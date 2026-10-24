//! Windows call observations and audio archives. Native objects stay on their owning MTA thread.

use std::collections::{BTreeMap, HashMap, HashSet};

#[cfg(windows)]
#[path = "windows_meetings/aec.rs"]
mod aec;
#[cfg(windows)]
#[path = "windows_meetings/archive.rs"]
mod archive;
#[cfg(windows)]
#[path = "windows_meetings/observation.rs"]
mod observation;

#[cfg(windows)]
pub use aec::{EchoCapture, echo_cancellation_available};
#[cfg(windows)]
pub use archive::{AudioArchive, decode_audio};
#[cfg(windows)]
pub use observation::snapshot;

#[cfg(windows)]
struct ComApartment(std::marker::PhantomData<std::rc::Rc<()>>);

#[cfg(windows)]
impl ComApartment {
    fn new() -> Result<Self, String> {
        unsafe {
            windows::Win32::System::Com::CoInitializeEx(
                None,
                windows::Win32::System::Com::COINIT_MULTITHREADED,
            )
            .ok()
            .map_err(|error| error.to_string())?;
        }
        Ok(Self(std::marker::PhantomData))
    }
}

#[cfg(windows)]
impl Drop for ComApartment {
    fn drop(&mut self) {
        unsafe { windows::Win32::System::Com::CoUninitialize() };
    }
}

#[derive(Clone, Debug)]
struct Process {
    parent: u32,
    executable: String,
}

#[derive(Clone, Copy, Debug)]
struct AudioUse {
    pid: u32,
    input: bool,
    output: bool,
}

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

fn known_app(executable: &str) -> Option<(&'static str, bool)> {
    let executable = executable.to_ascii_lowercase();
    Some(match executable.trim_end_matches(".exe") {
        "teams" | "ms-teams" | "msteams" => ("com.microsoft.teams2", false),
        "zoom" => ("us.zoom.xos", false),
        "slack" => ("com.tinyspeck.slackmacgap", false),
        "whatsapp" => ("net.whatsapp.WhatsApp", false),
        "discord" => ("com.hnc.Discord", false),
        "webex" | "ciscocollabhost" | "webexmta" => ("Cisco-Systems.Spark", false),
        "chrome" => ("com.google.Chrome", true),
        "msedge" => ("com.microsoft.edgemac", true),
        "firefox" => ("org.mozilla.firefox", true),
        "brave" => ("com.brave.Browser", true),
        _ => return None,
    })
}

fn is_own_process(pid: u32, own_pid: u32, processes: &HashMap<u32, Process>) -> bool {
    let mut current = pid;
    let mut seen = HashSet::new();
    while current != 0 && seen.insert(current) {
        if current == own_pid {
            return true;
        }
        let Some(process) = processes.get(&current) else {
            return false;
        };
        // A previous Pennel process may still be shutting down during an upgrade.
        if process.executable.eq_ignore_ascii_case("pennel.exe") {
            return true;
        }
        current = process.parent;
    }
    false
}

fn app_identity(pid: u32, processes: &HashMap<u32, Process>) -> Option<(String, bool)> {
    let fallback = processes.get(&pid)?.executable.to_ascii_lowercase();
    let mut current = pid;
    let mut seen = HashSet::new();
    while current != 0 && seen.insert(current) {
        let Some(process) = processes.get(&current) else {
            break;
        };
        if let Some((id, browser)) = known_app(&process.executable) {
            return Some((id.to_owned(), browser));
        }
        current = process.parent;
    }
    Some((fallback, false))
}

fn observations(
    uses: &[AudioUse],
    processes: &HashMap<u32, Process>,
    titles: &HashMap<u32, Vec<String>>,
    own_pid: u32,
) -> Vec<pennel_core::CallObservation> {
    let mut call_browsers = HashSet::new();
    for (pid, windows) in titles {
        let Some((id, true)) = app_identity(*pid, processes) else {
            continue;
        };
        if windows.iter().any(|title| {
            let title = title.to_lowercase();
            TITLE_HINTS.iter().any(|hint| title.contains(hint))
        }) {
            call_browsers.insert(id);
        }
    }

    let mut by_app: BTreeMap<String, (bool, bool, bool)> = BTreeMap::new();
    for usage in uses {
        if is_own_process(usage.pid, own_pid, processes) {
            continue;
        }
        let Some((id, browser)) = app_identity(usage.pid, processes) else {
            continue;
        };
        let recognized = if browser {
            call_browsers.contains(&id)
        } else {
            pennel_core::known_call_apps().contains(&id)
        };
        let entry = by_app.entry(id).or_default();
        entry.0 |= usage.input;
        entry.1 |= usage.output;
        entry.2 |= recognized;
    }
    by_app
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

/// Linear interpolation keeps the previous endpoint between batches. The Windows AAC encoder
/// accepts 48 kHz PCM16, while the shared transcription/capture boundary is 16 kHz Float32.
#[derive(Default)]
struct AacPcm {
    previous: Option<f32>,
}

impl AacPcm {
    fn convert(&mut self, samples: &[f32]) -> Vec<i16> {
        let mut output = Vec::with_capacity(samples.len() * 3);
        for &sample in samples {
            let sample = if sample.is_finite() {
                sample.clamp(-1.0, 1.0)
            } else {
                0.0
            };
            let previous = self.previous.unwrap_or(sample);
            for fraction in [0.0, 1.0 / 3.0, 2.0 / 3.0] {
                let value = previous + (sample - previous) * fraction;
                output.push((value * f32::from(i16::MAX)).round() as i16);
            }
            self.previous = Some(sample);
        }
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn process(parent: u32, executable: &str) -> Process {
        Process {
            parent,
            executable: executable.into(),
        }
    }

    #[test]
    fn call_helpers_share_the_existing_rule_identity() {
        let processes = HashMap::from([
            (10, process(0, "ms-teams.exe")),
            (11, process(10, "msedgewebview2.exe")),
        ]);
        let users = observations(
            &[
                AudioUse {
                    pid: 10,
                    input: true,
                    output: false,
                },
                AudioUse {
                    pid: 11,
                    input: false,
                    output: true,
                },
            ],
            &processes,
            &HashMap::new(),
            99,
        );
        assert_eq!(users.len(), 1);
        assert_eq!(users[0].app_id, "com.microsoft.teams2");
        assert!(users[0].is_call && users[0].is_playing);
    }

    #[test]
    fn browsers_need_a_call_title_and_keep_input_observations_after_tab_change() {
        let processes = HashMap::from([(10, process(0, "chrome.exe"))]);
        let uses = [AudioUse {
            pid: 10,
            input: true,
            output: true,
        }];
        let titles = HashMap::from([(10, vec!["Meet - abc-defg-hij - Google Chrome".into()])]);
        assert!(observations(&uses, &processes, &titles, 99)[0].is_call);
        let titles = HashMap::from([(10, vec!["Meeting notes - Google Chrome".into()])]);
        let users = observations(&uses, &processes, &titles, 99);
        assert_eq!(users.len(), 1);
        assert!(!users[0].is_call);
        assert!(users[0].is_playing);
    }

    #[test]
    fn own_process_tree_and_output_only_apps_are_excluded() {
        let processes = HashMap::from([
            (1, process(0, "pennel.exe")),
            (2, process(1, "msedgewebview2.exe")),
            (3, process(0, "zoom.exe")),
        ]);
        let users = observations(
            &[
                AudioUse {
                    pid: 2,
                    input: true,
                    output: true,
                },
                AudioUse {
                    pid: 3,
                    input: false,
                    output: true,
                },
            ],
            &processes,
            &HashMap::new(),
            1,
        );
        assert!(users.is_empty());
    }

    #[test]
    fn cyclic_or_missing_parent_processes_do_not_loop() {
        let processes =
            HashMap::from([(1, process(2, "unknown.exe")), (2, process(1, "other.exe"))]);
        assert_eq!(
            app_identity(1, &processes),
            Some(("unknown.exe".into(), false))
        );
        assert!(!is_own_process(1, 99, &processes));
        assert_eq!(app_identity(99, &processes), None);
    }

    #[test]
    fn encoder_resampling_is_continuous_between_batches() {
        let mut whole = AacPcm::default();
        let expected = whole.convert(&[0.0, 0.5, 1.0, -0.5]);
        let mut batches = AacPcm::default();
        let mut actual = batches.convert(&[0.0, 0.5]);
        actual.extend(batches.convert(&[1.0, -0.5]));
        assert_eq!(actual, expected);
        assert_eq!(actual.len(), 12);
        assert!(actual.windows(2).any(|p| p[0] != p[1]));
    }

    #[test]
    fn encoder_sanitizes_nonfinite_samples() {
        let mut converter = AacPcm::default();
        assert_eq!(converter.convert(&[f32::NAN, f32::INFINITY]), vec![0; 6]);
    }
}
