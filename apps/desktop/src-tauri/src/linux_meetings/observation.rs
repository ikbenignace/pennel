use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::Read;
use std::time::{Duration, Instant};

use serde_json::Value;

use super::process::output;

const TIMEOUT: Duration = Duration::from_secs(2);
const LIMIT: usize = 1024 * 1024;
const TITLE_HINTS: &[&str] = &[
    "meet - ",
    "google meet",
    "microsoft teams",
    "| teams",
    "zoom meeting",
    "zoom workplace",
    "whereby",
    "webex",
];

#[derive(Default, Clone)]
struct Process {
    parent: u32,
    binary: String,
}
#[derive(Clone)]
struct Usage {
    pid: u32,
    binary: String,
    input: bool,
}

pub fn detection_support() -> Result<(), String> {
    pulse("info").map(|_| ()).map_err(|_| "Meetingdetectie vereist een bereikbare PulseAudio- of PipeWire-server en pactl met JSON-ondersteuning (pakket pulseaudio-utils).".into())
}

pub fn browser_detection_support() -> Result<(), String> {
    if std::env::var("XDG_SESSION_TYPE").is_ok_and(|session| session == "wayland")
        || std::env::var_os("DISPLAY").is_none()
    {
        return Err("Browsercalls herkennen vereist een X11-sessie. Op Wayland kun je browsermeetings handmatig opnemen; native call-apps blijven herkenbaar via de audioserver.".into());
    }
    output("xprop", &["-root", "_NET_CLIENT_LIST"], TIMEOUT, LIMIT)
        .map(|_| ())
        .map_err(|_| {
            "Browsercalls herkennen vereist xprop en toegang tot de X11-sessie (pakket x11-utils)."
                .into()
        })
}

fn pulse(command: &str) -> Result<Value, String> {
    let arguments = if command == "info" {
        vec!["--format=json", "info"]
    } else {
        vec!["--format=json", "list", command]
    };
    let bytes = output("pactl", &arguments, TIMEOUT, LIMIT)?;
    serde_json::from_slice(&bytes).map_err(|_| "De audioserver gaf ongeldige metadata terug".into())
}

pub fn snapshot() -> Result<Vec<pennel_core::CallObservation>, String> {
    let clients = pulse("clients")?;
    let input = pulse("source-outputs")?;
    let output = pulse("sink-inputs")?;
    let uses = usages(&clients, &input, &output)?;
    let mut processes = HashMap::new();
    for usage in &uses {
        load_process(usage.pid, &mut processes);
        if !usage.binary.is_empty() {
            processes.entry(usage.pid).or_default().binary = usage.binary.clone();
        }
    }
    let titles = if std::env::var("XDG_SESSION_TYPE").is_ok_and(|session| session == "wayland") {
        HashSet::new()
    } else {
        browser_titles(&processes).unwrap_or_default()
    };
    Ok(observations(&uses, &processes, &titles, std::process::id()))
}

fn number(value: Option<&Value>) -> Option<u32> {
    value.and_then(|value| {
        value
            .as_u64()
            .and_then(|number| u32::try_from(number).ok())
            .or_else(|| value.as_str()?.parse().ok())
    })
}

fn property<'a>(value: &'a Value, name: &str) -> Option<&'a Value> {
    value.get("properties")?.get(name)
}

fn usages(clients: &Value, input: &Value, output: &Value) -> Result<Vec<Usage>, String> {
    let clients = clients
        .as_array()
        .ok_or("Ongeldige lijst met audioclients")?;
    let clients: HashMap<_, _> = clients
        .iter()
        .filter_map(|client| Some((number(client.get("index"))?, client)))
        .collect();
    let mut uses = Vec::new();
    for (streams, is_input) in [(input, true), (output, false)] {
        for stream in streams
            .as_array()
            .ok_or("Ongeldige lijst met audiostromen")?
        {
            if stream
                .get("corked")
                .is_some_and(|value| value.as_bool() == Some(true) || value.as_str() == Some("yes"))
            {
                continue;
            }
            let client = number(stream.get("client"))
                .and_then(|index| clients.get(&index))
                .copied();
            let pid = number(property(stream, "application.process.id")).or_else(|| {
                number(client.and_then(|client| property(client, "application.process.id")))
            });
            let Some(pid) = pid.filter(|pid| *pid != 0) else {
                continue;
            };
            let binary = property(stream, "application.process.binary")
                .or_else(|| {
                    client.and_then(|client| property(client, "application.process.binary"))
                })
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
            uses.push(Usage {
                pid,
                binary,
                input: is_input,
            });
        }
    }
    Ok(uses)
}

fn known(binary: &str) -> Option<(&'static str, bool)> {
    let binary = binary
        .rsplit('/')
        .next()
        .unwrap_or(binary)
        .to_ascii_lowercase();
    Some(match binary.as_str() {
        "teams" | "teams-for-linux" | "teams-for-linu" | "ms-teams" => {
            ("com.microsoft.teams2", false)
        }
        "zoom" | "zoom.real" => ("us.zoom.xos", false),
        "slack" => ("com.tinyspeck.slackmacgap", false),
        "discord" => ("com.hnc.Discord", false),
        "whatsapp-for-linux" | "whatsapp-for-li" => ("net.whatsapp.WhatsApp", false),
        "webex" | "ciscocollabhost" => ("Cisco-Systems.Spark", false),
        "chrome" | "google-chrome" | "google-chrome-stable" | "chromium" | "chromium-browser" => {
            ("com.google.Chrome", true)
        }
        "msedge" | "microsoft-edge" | "microsoft-edge-stable" => ("com.microsoft.edgemac", true),
        "firefox" | "firefox-bin" => ("org.mozilla.firefox", true),
        "brave" | "brave-browser" => ("com.brave.Browser", true),
        _ => return None,
    })
}

fn identity(pid: u32, processes: &HashMap<u32, Process>) -> Option<(String, bool)> {
    let fallback = processes.get(&pid)?.binary.clone();
    let mut current = pid;
    let mut seen = HashSet::new();
    while current != 0 && seen.insert(current) {
        let Some(process) = processes.get(&current) else {
            break;
        };
        if let Some((id, browser)) = known(&process.binary) {
            return Some((id.into(), browser));
        }
        current = process.parent;
    }
    Some((fallback, false))
}

fn own_process(pid: u32, own: u32, processes: &HashMap<u32, Process>) -> bool {
    let mut current = pid;
    let mut seen = HashSet::new();
    while current != 0 && seen.insert(current) {
        if current == own {
            return true;
        }
        let Some(process) = processes.get(&current) else {
            break;
        };
        if process.binary == "pennel" {
            return true;
        }
        current = process.parent;
    }
    false
}

fn observations(
    uses: &[Usage],
    processes: &HashMap<u32, Process>,
    browser_calls: &HashSet<String>,
    own: u32,
) -> Vec<pennel_core::CallObservation> {
    let mut grouped: BTreeMap<String, (bool, bool, bool)> = BTreeMap::new();
    for usage in uses {
        if own_process(usage.pid, own, processes) {
            continue;
        }
        let Some((id, browser)) = identity(usage.pid, processes) else {
            continue;
        };
        let call = if browser {
            browser_calls.contains(&id)
        } else {
            pennel_core::known_call_apps().contains(&id)
        };
        let entry = grouped.entry(id).or_default();
        if usage.input {
            entry.0 = true;
        } else {
            entry.1 = true;
        }
        entry.2 |= call;
    }
    grouped
        .into_iter()
        .filter(|(_, (input, _, _))| *input)
        .map(
            |(app_id, (_, is_playing, is_call))| pennel_core::CallObservation {
                app_id,
                is_call,
                is_playing,
            },
        )
        .collect()
}

fn load_process(pid: u32, processes: &mut HashMap<u32, Process>) {
    let mut current = pid;
    for _ in 0..64 {
        if current == 0 || processes.contains_key(&current) {
            break;
        }
        let mut stat = String::new();
        if std::fs::File::open(format!("/proc/{current}/stat"))
            .and_then(|file| file.take(4096).read_to_string(&mut stat))
            .is_err()
        {
            break;
        }
        let Some(open) = stat.find('(') else {
            break;
        };
        let Some(close) = stat.rfind(')') else {
            break;
        };
        let parent = stat[close + 1..]
            .split_whitespace()
            .nth(1)
            .and_then(|parent| parent.parse().ok())
            .unwrap_or(0);
        let binary = stat[open + 1..close].to_owned();
        processes.insert(current, Process { parent, binary });
        current = parent;
    }
}

fn browser_titles(processes: &HashMap<u32, Process>) -> Result<HashSet<String>, String> {
    let bytes = output(
        "xprop",
        &["-root", "_NET_CLIENT_LIST"],
        Duration::from_millis(250),
        LIMIT,
    )?;
    let root = String::from_utf8(bytes).map_err(|_| "Ongeldige venstermetadata")?;
    let deadline = Instant::now() + TIMEOUT;
    let mut calls = HashSet::new();
    for window in root
        .split(|character: char| character.is_whitespace() || character == ',')
        .filter(|window| {
            window.starts_with("0x")
                && window.len() <= 18
                && window[2..]
                    .chars()
                    .all(|character| character.is_ascii_hexdigit())
        })
        .take(128)
    {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        let Ok(bytes) = output(
            "xprop",
            &["-id", window, "_NET_WM_PID", "_NET_WM_NAME", "WM_NAME"],
            remaining.min(Duration::from_millis(200)),
            16_384,
        ) else {
            continue;
        };
        let properties = String::from_utf8_lossy(&bytes);
        let pid = properties
            .lines()
            .find(|line| line.starts_with("_NET_WM_PID("))
            .and_then(|line| line.split_once('='))
            .and_then(|(_, pid)| pid.trim().parse().ok());
        let Some((id, true)) = pid.and_then(|pid| identity(pid, processes)) else {
            continue;
        };
        let title = properties
            .lines()
            .filter(|line| line.starts_with("_NET_WM_NAME(") || line.starts_with("WM_NAME("))
            .filter_map(|line| line.split_once('=').map(|(_, title)| title))
            .collect::<String>()
            .to_lowercase();
        if TITLE_HINTS.iter().any(|hint| title.contains(hint)) {
            calls.insert(id);
        }
    }
    Ok(calls)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn pulse_clients_supply_missing_stream_properties_and_corked_streams_are_ignored() {
        let clients = json!([{"index": 7, "properties":{"application.process.id":"42", "application.process.binary":"zoom"}}]);
        let input = json!([{"client":7,"corked":false,"properties":{}}]);
        let output = json!([{"client":7,"corked":true,"properties":{}}]);
        let uses = usages(&clients, &input, &output).unwrap();
        assert_eq!(uses.len(), 1);
        assert_eq!(uses[0].pid, 42);
        assert_eq!(uses[0].binary, "zoom");
    }

    #[test]
    fn helpers_merge_and_own_recording_never_detects_itself() {
        let processes = HashMap::from([
            (
                10,
                Process {
                    parent: 0,
                    binary: "zoom".into(),
                },
            ),
            (
                11,
                Process {
                    parent: 10,
                    binary: "helper".into(),
                },
            ),
            (
                99,
                Process {
                    parent: 0,
                    binary: "pennel".into(),
                },
            ),
        ]);
        let uses = [
            Usage {
                pid: 10,
                binary: "zoom".into(),
                input: true,
            },
            Usage {
                pid: 11,
                binary: "helper".into(),
                input: false,
            },
            Usage {
                pid: 99,
                binary: "pennel".into(),
                input: true,
            },
        ];
        let users = observations(&uses, &processes, &HashSet::new(), 99);
        assert_eq!(users.len(), 1);
        assert_eq!(users[0].app_id, "us.zoom.xos");
        assert!(users[0].is_call && users[0].is_playing);
    }

    #[test]
    fn browser_requires_call_title_but_observation_survives_tab_change() {
        let processes = HashMap::from([(
            10,
            Process {
                parent: 0,
                binary: "chromium".into(),
            },
        )]);
        let uses = [
            Usage {
                pid: 10,
                binary: "chromium".into(),
                input: true,
            },
            Usage {
                pid: 10,
                binary: "chromium".into(),
                input: false,
            },
        ];
        assert!(!observations(&uses, &processes, &HashSet::new(), 99)[0].is_call);
        assert!(
            observations(
                &uses,
                &processes,
                &HashSet::from(["com.google.Chrome".into()]),
                99
            )[0]
            .is_call
        );
    }
}
