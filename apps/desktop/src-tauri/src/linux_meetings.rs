//! Linux system adapters. Audio uses fixed GStreamer pipelines over file descriptors, and call
//! detection reads bounded PulseAudio/PipeWire metadata without logging window titles or content.

#[path = "linux_meetings/archive.rs"]
mod archive;
#[path = "linux_meetings/observation.rs"]
mod observation;
#[path = "linux_meetings/process.rs"]
mod process;

pub use archive::{AudioArchive, archive_support, decode_audio};
pub use observation::{browser_detection_support, detection_support, snapshot};
