use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use crate::error::PennelError;
use crate::llm::ChatMessage;
use crate::provider::ProviderKind;

const AGENTS: [(ProviderKind, &str); 6] = [
    (ProviderKind::ClaudeCode, "claude"),
    (ProviderKind::Codex, "codex"),
    (ProviderKind::OpenCode, "opencode"),
    (ProviderKind::Copilot, "copilot"),
    (ProviderKind::CursorAgent, "cursor-agent"),
    (ProviderKind::GeminiCli, "gemini"),
];

/// Tools Claude Code must never use here: transcripts are untrusted text and must not be able to act.
const CLAUDE_BLOCKED_TOOLS: &str =
    "Bash,Edit,Write,Read,MultiEdit,NotebookEdit,WebFetch,WebSearch,Glob,Grep,Task,TodoWrite";
const ERROR_TAIL_CHARS: usize = 300;

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record, serde::Serialize)]
pub struct DetectedAgent {
    pub kind: ProviderKind,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Invocation {
    pub args: Vec<String>,
    pub stdin: Option<String>,
    /// Codex writes its final answer to a file; stdout carries progress.
    pub output_file: Option<PathBuf>,
}

/// GUI apps start with a minimal PATH, so node-based CLIs (installed via nvm, npm, bun) are found
/// through the user's login shell.
#[cfg(not(unix))]
fn login_shell_path() -> &'static str {
    static PATH: OnceLock<String> = OnceLock::new();
    PATH.get_or_init(|| std::env::var("PATH").unwrap_or_default())
}

#[cfg(unix)]
fn login_shell_path() -> &'static str {
    static PATH: OnceLock<String> = OnceLock::new();
    PATH.get_or_init(|| {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into());
        let from_shell = Command::new(shell)
            .args(["-lc", "printf %s \"$PATH\""])
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .ok()
            .filter(|out| out.status.success())
            .and_then(|out| String::from_utf8(out.stdout).ok())
            .filter(|path| !path.trim().is_empty());
        let home = std::env::var("HOME").unwrap_or_default();
        let extra = [".local/bin", ".bun/bin", ".npm-global/bin", ".cargo/bin"]
            .map(|d| format!("{home}/{d}"));
        let mut dirs: Vec<String> = from_shell
            .unwrap_or_else(|| std::env::var("PATH").unwrap_or_default())
            .split(':')
            .map(str::to_owned)
            .collect();
        for dir in extra
            .into_iter()
            .chain(["/opt/homebrew/bin".into(), "/usr/local/bin".into()])
        {
            if !dirs.contains(&dir) {
                dirs.push(dir);
            }
        }
        dirs.join(":")
    })
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.metadata()
        .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

#[cfg(unix)]
const PATH_SEPARATOR: char = ':';
#[cfg(not(unix))]
const PATH_SEPARATOR: char = ';';

/// npm installs Windows CLIs as `.cmd` shims next to the extension-less Unix script.
#[cfg(not(unix))]
const EXECUTABLE_SUFFIXES: [&str; 3] = [".exe", ".cmd", ""];
#[cfg(unix)]
const EXECUTABLE_SUFFIXES: [&str; 1] = [""];

pub(crate) fn detect_in(search_path: &str) -> Vec<DetectedAgent> {
    AGENTS
        .iter()
        .filter_map(|(kind, binary)| {
            search_path
                .split(PATH_SEPARATOR)
                .filter(|dir| !dir.is_empty())
                .flat_map(|dir| {
                    EXECUTABLE_SUFFIXES
                        .iter()
                        .map(move |suffix| Path::new(dir).join(format!("{binary}{suffix}")))
                })
                .find(|candidate| is_executable(candidate))
                .map(|path| DetectedAgent {
                    kind: *kind,
                    path: path.to_string_lossy().into_owned(),
                })
        })
        .collect()
}

/// Coding-agent CLIs installed on this computer that Pennel can use for cleanup and summaries.
#[uniffi::export]
pub fn detect_cli_agents() -> Vec<DetectedAgent> {
    detect_in(login_shell_path())
}

/// A one-line round trip, so the settings screen can show whether the CLI is logged in and answering.
#[uniffi::export]
pub fn check_cli_agent(
    kind: ProviderKind,
    path: String,
    model: String,
) -> Result<String, PennelError> {
    let messages = [ChatMessage {
        role: "user",
        content: "Reply with exactly: ok".into(),
    }];
    complete(kind, &path, &model, &messages, 90)
}

fn system_text(messages: &[ChatMessage]) -> String {
    messages
        .iter()
        .filter(|m| m.role == "system")
        .map(|m| m.content.as_str())
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// The conversation as one prompt: CLIs take a single message, so earlier turns are quoted above the last one.
fn conversation_text(messages: &[ChatMessage]) -> String {
    let turns: Vec<&ChatMessage> = messages.iter().filter(|m| m.role != "system").collect();
    match turns.as_slice() {
        [] => String::new(),
        [only] => only.content.clone(),
        [earlier @ .., last] => {
            let history: Vec<String> = earlier
                .iter()
                .map(|m| {
                    format!(
                        "{}: {}",
                        if m.role == "assistant" {
                            "Assistant"
                        } else {
                            "User"
                        },
                        m.content
                    )
                })
                .collect();
            format!(
                "Earlier in this conversation:\n{}\n\nNow:\n{}",
                history.join("\n\n"),
                last.content
            )
        }
    }
}

fn single_prompt(messages: &[ChatMessage]) -> String {
    let system = system_text(messages);
    let conversation = conversation_text(messages);
    if system.is_empty() {
        conversation
    } else {
        format!(
            "{system}\n\nAnswer with the requested text only, without tools, preamble or explanation.\n\n{conversation}"
        )
    }
}

pub(crate) fn invocation(
    kind: ProviderKind,
    model: &str,
    messages: &[ChatMessage],
    output_file: &Path,
) -> Invocation {
    let model_flag = |flag: &str| -> Vec<String> {
        if model.trim().is_empty() {
            Vec::new()
        } else {
            vec![flag.into(), model.trim().into()]
        }
    };
    let strings = |items: &[&str]| items.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    match kind {
        ProviderKind::ClaudeCode => {
            let mut args = strings(&[
                "-p",
                "--strict-mcp-config",
                "--no-session-persistence",
                "--setting-sources",
                "",
                "--disallowedTools",
                CLAUDE_BLOCKED_TOOLS,
                "--output-format",
                "text",
                "--system-prompt",
            ]);
            args.push(system_text(messages));
            args.extend(model_flag("--model"));
            Invocation {
                args,
                stdin: Some(conversation_text(messages)),
                output_file: None,
            }
        }
        ProviderKind::Codex => {
            let mut args = strings(&[
                "exec",
                "--sandbox",
                "read-only",
                "--skip-git-repo-check",
                "--ephemeral",
                "--color",
                "never",
                "-o",
            ]);
            args.push(output_file.to_string_lossy().into_owned());
            args.extend(model_flag("-m"));
            args.push("-".into());
            Invocation {
                args,
                stdin: Some(single_prompt(messages)),
                output_file: Some(output_file.to_owned()),
            }
        }
        ProviderKind::OpenCode => {
            let mut args = vec!["run".to_string()];
            args.extend(model_flag("-m"));
            args.push(single_prompt(messages));
            Invocation {
                args,
                stdin: None,
                output_file: None,
            }
        }
        ProviderKind::CursorAgent => {
            let mut args = vec![
                "-p".to_string(),
                single_prompt(messages),
                "--output-format".into(),
                "text".into(),
            ];
            args.extend(model_flag("--model"));
            Invocation {
                args,
                stdin: None,
                output_file: None,
            }
        }
        ProviderKind::Copilot => {
            let mut args = vec!["-p".to_string(), single_prompt(messages)];
            args.extend(model_flag("--model"));
            Invocation {
                args,
                stdin: None,
                output_file: None,
            }
        }
        _ => {
            let mut args = vec!["-p".to_string(), single_prompt(messages)];
            args.extend(model_flag("-m"));
            Invocation {
                args,
                stdin: None,
                output_file: None,
            }
        }
    }
}

pub(crate) fn strip_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            if chars.peek() == Some(&'[') {
                chars.next();
                for next in chars.by_ref() {
                    if next.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
            continue;
        }
        out.push(c);
    }
    out
}

fn tail(text: &str) -> String {
    let clean = strip_ansi(text);
    let trimmed = clean.trim();
    let start = trimmed
        .char_indices()
        .rev()
        .nth(ERROR_TAIL_CHARS)
        .map_or(0, |(i, _)| i);
    trimmed[start..].to_owned()
}

/// Runs one completion through a coding-agent CLI in an empty scratch folder, so it has no project
/// files to look at, and kills it when it exceeds `timeout_secs`.
pub(crate) fn complete(
    kind: ProviderKind,
    executable: &str,
    model: &str,
    messages: &[ChatMessage],
    timeout_secs: u64,
) -> Result<String, PennelError> {
    if executable.is_empty() || !is_executable(Path::new(executable)) {
        return Err(PennelError::NotFound);
    }
    let scratch =
        std::env::temp_dir().join(format!("pennel-agent-{}", uuid::Uuid::now_v7().simple()));
    std::fs::create_dir_all(&scratch).map_err(PennelError::storage)?;
    let result = run(kind, executable, model, messages, timeout_secs, &scratch);
    let _ = std::fs::remove_dir_all(&scratch);
    result
}

fn run(
    kind: ProviderKind,
    executable: &str,
    model: &str,
    messages: &[ChatMessage],
    timeout_secs: u64,
    scratch: &Path,
) -> Result<String, PennelError> {
    let call = invocation(kind, model, messages, &scratch.join("answer.txt"));
    let mut child = Command::new(executable)
        .args(&call.args)
        .current_dir(scratch)
        .env("PATH", login_shell_path())
        .env("NO_COLOR", "1")
        .stdin(if call.stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| PennelError::Network {
            message: format!("{executable}: {e}"),
        })?;

    if let (Some(input), Some(mut pipe)) = (call.stdin, child.stdin.take()) {
        std::thread::spawn(move || {
            let _ = pipe.write_all(input.as_bytes());
        });
    }
    let reader = |pipe: Option<Box<dyn Read + Send>>| {
        std::thread::spawn(move || {
            let mut text = String::new();
            if let Some(mut pipe) = pipe {
                let _ = pipe.read_to_string(&mut text);
            }
            text
        })
    };
    let stdout = reader(
        child
            .stdout
            .take()
            .map(|p| Box::new(p) as Box<dyn Read + Send>),
    );
    let stderr = reader(
        child
            .stderr
            .take()
            .map(|p| Box::new(p) as Box<dyn Read + Send>),
    );

    let deadline = Instant::now() + Duration::from_secs(timeout_secs);
    let status = loop {
        if let Some(status) = child.try_wait().map_err(PennelError::storage)? {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(PennelError::Network {
                message: format!("timed out after {timeout_secs} s"),
            });
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let stdout = stdout.join().unwrap_or_default();
    let stderr = stderr.join().unwrap_or_default();
    if !status.success() {
        let detail = if stderr.trim().is_empty() {
            tail(&stdout)
        } else {
            tail(&stderr)
        };
        return Err(PennelError::InvalidResponse { message: detail });
    }
    let answer = match call.output_file {
        Some(file) => std::fs::read_to_string(file).unwrap_or_default(),
        None => stdout,
    };
    let answer = strip_ansi(&answer).trim().to_owned();
    if answer.is_empty() {
        return Err(PennelError::InvalidResponse {
            message: tail(&stderr),
        });
    }
    Ok(answer)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn messages() -> Vec<ChatMessage> {
        vec![
            ChatMessage {
                role: "system",
                content: "Clean up.".into(),
            },
            ChatMessage {
                role: "user",
                content: "euh hallo".into(),
            },
        ]
    }

    fn script(dir: &Path, name: &str, body: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    #[test]
    fn detects_executables_on_the_search_path() {
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        script(a.path(), "claude", "true");
        script(b.path(), "codex", "true");
        std::fs::write(b.path().join("opencode"), "not executable").unwrap();
        let found = detect_in(&format!("{}:{}", a.path().display(), b.path().display()));
        let kinds: Vec<ProviderKind> = found.iter().map(|d| d.kind).collect();
        assert_eq!(kinds, [ProviderKind::ClaudeCode, ProviderKind::Codex]);
        assert!(found[0].path.ends_with("/claude"));
    }

    #[test]
    fn claude_gets_system_prompt_blocked_tools_and_stdin() {
        let call = invocation(
            ProviderKind::ClaudeCode,
            "haiku",
            &messages(),
            Path::new("/tmp/x"),
        );
        let args = call.args.join(" ");
        assert!(args.starts_with("-p --strict-mcp-config --no-session-persistence"));
        assert!(args.contains("--disallowedTools Bash,Edit,Write,Read"));
        assert!(args.contains("--system-prompt Clean up."));
        assert!(args.ends_with("--model haiku"));
        assert_eq!(call.stdin.as_deref(), Some("euh hallo"));
    }

    #[test]
    fn codex_is_read_only_and_writes_answer_to_file() {
        let call = invocation(
            ProviderKind::Codex,
            "",
            &messages(),
            Path::new("/tmp/answer.txt"),
        );
        assert_eq!(
            call.args,
            [
                "exec",
                "--sandbox",
                "read-only",
                "--skip-git-repo-check",
                "--ephemeral",
                "--color",
                "never",
                "-o",
                "/tmp/answer.txt",
                "-"
            ]
        );
        assert!(call.stdin.unwrap().ends_with("euh hallo"));
        assert_eq!(
            call.output_file.as_deref(),
            Some(Path::new("/tmp/answer.txt"))
        );
    }

    #[test]
    fn single_prompt_clis_get_system_and_turns_in_one_message() {
        let mut chat = messages();
        chat.push(ChatMessage {
            role: "assistant",
            content: "Hallo.".into(),
        });
        chat.push(ChatMessage {
            role: "user",
            content: "en nu?".into(),
        });
        let call = invocation(
            ProviderKind::Copilot,
            "gpt-5-mini",
            &chat,
            Path::new("/tmp/x"),
        );
        assert_eq!(call.args[0], "-p");
        assert!(call.args[1].starts_with("Clean up."));
        assert!(call.args[1].contains("User: euh hallo\n\nAssistant: Hallo.\n\nNow:\nen nu?"));
        assert_eq!(&call.args[2..], ["--model", "gpt-5-mini"]);
    }

    #[test]
    fn strips_terminal_colours() {
        assert_eq!(
            strip_ansi("\u{1b}[91m\u{1b}[1mError: \u{1b}[0mkey expired"),
            "Error: key expired"
        );
    }

    #[test]
    fn runs_agent_and_returns_stdout() {
        let dir = tempfile::tempdir().unwrap();
        let fake = script(dir.path(), "claude", "cat; printf '\\n'");
        let answer = complete(
            ProviderKind::ClaudeCode,
            fake.to_str().unwrap(),
            "",
            &messages(),
            10,
        )
        .unwrap();
        assert_eq!(answer, "euh hallo");
    }

    #[test]
    fn failing_agent_reports_its_error() {
        let dir = tempfile::tempdir().unwrap();
        let fake = script(
            dir.path(),
            "opencode",
            "echo 'This API key has expired' >&2; exit 1",
        );
        let err = complete(
            ProviderKind::OpenCode,
            fake.to_str().unwrap(),
            "",
            &messages(),
            10,
        )
        .unwrap_err();
        assert_eq!(
            err,
            PennelError::InvalidResponse {
                message: "This API key has expired".into()
            }
        );
    }

    #[test]
    fn slow_agent_is_killed() {
        let dir = tempfile::tempdir().unwrap();
        let fake = script(dir.path(), "gemini", "sleep 5");
        let started = Instant::now();
        let err = complete(
            ProviderKind::GeminiCli,
            fake.to_str().unwrap(),
            "",
            &messages(),
            1,
        )
        .unwrap_err();
        assert!(matches!(err, PennelError::Network { .. }));
        assert!(started.elapsed() < Duration::from_secs(3));
    }

    #[test]
    fn missing_executable_is_not_found() {
        assert_eq!(
            complete(ProviderKind::Codex, "/nope/codex", "", &messages(), 1),
            Err(PennelError::NotFound)
        );
    }
}

/// Uses the real Claude Code CLI on this machine; run with `cargo test -- --ignored`.
#[cfg(all(test, unix))]
mod real {
    use super::*;

    #[test]
    #[ignore]
    fn real_claude_cleans_up_dictation() {
        let claude = detect_cli_agents()
            .into_iter()
            .find(|a| a.kind == ProviderKind::ClaudeCode)
            .expect("claude installed");
        let messages = crate::cleanup::cleanup_messages(
            "euh dus ik wou eigenlijk zeggen dat de meeting naar dinsdag nee wacht donderdag verschuift",
            crate::settings::Style::Neutral,
            &[],
        );
        let started = Instant::now();
        let answer = complete(
            ProviderKind::ClaudeCode,
            &claude.path,
            "haiku",
            &messages,
            60,
        )
        .unwrap();
        eprintln!("{:?}: {answer}", started.elapsed());
        assert!(answer.to_lowercase().contains("donderdag"), "{answer}");
        assert!(!answer.to_lowercase().contains("euh"), "{answer}");
    }
}
