use std::io::{Read, Write};
use std::path::Path;
use std::process::{Child, Stdio};
use std::sync::mpsc::{self, SyncSender, TrySendError};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use super::process::{command, output, terminate, wait};

const RATE: usize = 16_000;
const MAX_SAMPLES: usize = RATE * 3 * 3600;
const PROCESS_TIMEOUT: Duration = Duration::from_secs(120);
const FINISH_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone, Copy)]
struct Codecs {
    encoder: &'static str,
    decoder: &'static str,
}

fn codecs() -> Result<Codecs, String> {
    static CODECS: OnceLock<Result<Codecs, String>> = OnceLock::new();
    CODECS.get_or_init(|| {
        output("gst-launch-1.0", &["--version"], Duration::from_secs(2), 16_384)
            .map_err(|_| "M4A-audio vereist GStreamer (gstreamer1.0-tools). Installeer de audiopakketten en herstart Pennel.".to_owned())?;
        for plugin in ["fdsrc", "fdsink", "rawaudioparse", "audioconvert", "audioresample", "aacparse", "mp4mux", "qtdemux"] {
            if !has_plugin(plugin) { return Err(format!("GStreamer-element {plugin} ontbreekt. Installeer gstreamer1.0-plugins-base en gstreamer1.0-plugins-good en herstart Pennel.")); }
        }
        let encoder = ["voaacenc", "avenc_aac"].into_iter().find(|plugin| has_plugin(plugin))
            .ok_or("Een AAC-encoder ontbreekt. Installeer gstreamer1.0-libav en herstart Pennel.")?;
        let decoder = ["avdec_aac", "faad"].into_iter().find(|plugin| has_plugin(plugin))
            .ok_or("Een AAC-decoder ontbreekt. Installeer gstreamer1.0-libav en herstart Pennel.")?;
        Ok(Codecs { encoder, decoder })
    }).clone()
}

fn has_plugin(plugin: &str) -> bool {
    output(
        "gst-inspect-1.0",
        &["--exists", plugin],
        Duration::from_secs(2),
        16_384,
    )
    .is_ok()
}

pub fn archive_support() -> Result<(), String> {
    codecs().map(|_| ())
}

fn encoder_pipeline(codec: &str) -> Vec<&str> {
    let format = if codec == "avenc_aac" {
        "audio/x-raw,format=F32LE,rate=48000,channels=1"
    } else {
        "audio/x-raw,format=S16LE,rate=48000,channels=1"
    };
    vec![
        "-q",
        "-e",
        "fdsrc",
        "fd=0",
        "!",
        "rawaudioparse",
        "format=pcm",
        "pcm-format=f32le",
        "sample-rate=16000",
        "num-channels=1",
        "!",
        "audioconvert",
        "!",
        "audioresample",
        "!",
        format,
        "!",
        codec,
        "bitrate=96000",
        "!",
        "aacparse",
        "!",
        "mp4mux",
        "faststart=true",
        "!",
        "fdsink",
        "fd=1",
        "sync=false",
    ]
}

fn decoder_pipeline(codec: &str) -> Vec<&str> {
    vec![
        "-q",
        "fdsrc",
        "fd=0",
        "!",
        "qtdemux",
        "!",
        "aacparse",
        "!",
        codec,
        "!",
        "audioconvert",
        "!",
        "audioresample",
        "!",
        "audio/x-raw,format=F32LE,rate=16000,channels=1,layout=interleaved",
        "!",
        "fdsink",
        "fd=1",
        "sync=false",
    ]
}

enum Block {
    Audio(Vec<f32>),
    Silence(u64),
}

pub struct AudioArchive {
    sender: Option<SyncSender<Block>>,
    writer: Option<JoinHandle<Result<(), String>>>,
    child: Arc<Mutex<Child>>,
    error: Arc<Mutex<Option<String>>>,
}

impl AudioArchive {
    pub fn create(path: &Path) -> Result<Self, String> {
        let codecs = codecs()?;
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|error| error.to_string())?;
        // No paths are interpolated into GStreamer's launch language. The only inputs to that
        // parser are fixed tokens and a codec from the compile-time allowlist above.
        let mut child = match command("gst-launch-1.0")
            .args(encoder_pipeline(codecs.encoder))
            .stdin(Stdio::piped())
            .stdout(Stdio::from(file))
            .stderr(Stdio::null())
            .env("GST_DEBUG", "0")
            .env_remove("GST_DEBUG_DUMP_DOT_DIR")
            .spawn()
        {
            Ok(child) => child,
            Err(error) => {
                let _ = std::fs::remove_file(path);
                return Err(error.to_string());
            }
        };
        let mut input = child.stdin.take().ok_or("Audio-invoer ontbreekt")?;
        let (sender, receiver) = mpsc::sync_channel::<Block>(32);
        let error = Arc::new(Mutex::new(None));
        let writer_error = error.clone();
        let writer = match std::thread::Builder::new()
            .name("pennel-gstreamer-archive".into())
            .spawn(move || {
                let result = (|| {
                    static SILENCE: [u8; RATE * 4] = [0; RATE * 4];
                    for block in receiver {
                        match block {
                            Block::Audio(samples) => {
                                let bytes: Vec<_> = samples
                                    .into_iter()
                                    .flat_map(|sample| {
                                        (if sample.is_finite() {
                                            sample.clamp(-1.0, 1.0)
                                        } else {
                                            0.0
                                        })
                                        .to_le_bytes()
                                    })
                                    .collect();
                                input.write_all(&bytes).map_err(|error| error.to_string())?;
                            }
                            Block::Silence(mut samples) => {
                                while samples != 0 {
                                    let count = samples.min(RATE as u64) as usize;
                                    input
                                        .write_all(&SILENCE[..count * 4])
                                        .map_err(|error| error.to_string())?;
                                    samples -= count as u64;
                                }
                            }
                        }
                    }
                    input.flush().map_err(|error| error.to_string())
                })();
                if let Err(error) = &result {
                    *writer_error.lock().unwrap() = Some(error.clone());
                }
                result
            }) {
            Ok(writer) => writer,
            Err(error) => {
                terminate(&mut child);
                return Err(error.to_string());
            }
        };
        Ok(Self {
            sender: Some(sender),
            writer: Some(writer),
            child: Arc::new(Mutex::new(child)),
            error,
        })
    }

    pub fn write(&mut self, samples: &[f32]) -> Result<(), String> {
        for batch in samples.chunks(RATE) {
            self.send(Block::Audio(batch.to_vec()))?;
        }
        Ok(())
    }

    pub fn write_silence(&mut self, samples: u64) -> Result<(), String> {
        if samples == 0 {
            return Ok(());
        }
        self.send(Block::Silence(samples))
    }

    fn send(&mut self, block: Block) -> Result<(), String> {
        if let Some(error) = self.error.lock().unwrap().as_ref() {
            return Err(error.clone());
        }
        let Some(sender) = &self.sender else {
            return Err("Audio-opslag is gesloten".into());
        };
        sender.try_send(block).map_err(|error| {
            let message = match error {
                TrySendError::Full(_) => "Audio-opslag houdt de opname niet bij",
                TrySendError::Disconnected(_) => "Audio-opslag is onverwacht gestopt",
            };
            *self.error.lock().unwrap() = Some(message.into());
            message.into()
        })
    }

    pub fn finish(mut self) -> Result<(), String> {
        self.close()
    }

    fn close(&mut self) -> Result<(), String> {
        self.sender.take();
        let Some(writer) = self.writer.take() else {
            return self.error.lock().unwrap().clone().map_or(Ok(()), Err);
        };
        let deadline = Instant::now() + FINISH_TIMEOUT;
        let mut timeout = false;
        while !writer.is_finished() {
            if Instant::now() >= deadline {
                terminate(&mut self.child.lock().unwrap());
                timeout = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let written = writer
            .join()
            .map_err(|_| "Audio-opslag is onverwacht gestopt".to_owned())
            .and_then(|result| result);
        let status = wait(
            &mut self.child.lock().unwrap(),
            deadline.saturating_duration_since(Instant::now()),
        );
        if timeout {
            return Err("Audio-opslag reageerde niet op tijd".into());
        }
        written?;
        if let Some(error) = self.error.lock().unwrap().clone() {
            return Err(error);
        }
        if !status?.success() {
            return Err(
                "M4A-opslag is mislukt. Controleer GStreamer en de beschikbare opslagruimte."
                    .into(),
            );
        }
        Ok(())
    }
}

impl Drop for AudioArchive {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

pub fn decode_audio(path: &Path) -> Result<Vec<f32>, String> {
    let codecs = codecs()?;
    let file = std::fs::File::open(path).map_err(|error| error.to_string())?;
    let mut child = command("gst-launch-1.0")
        .args(decoder_pipeline(codecs.decoder))
        .stdin(Stdio::from(file))
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .env("GST_DEBUG", "0")
        .env_remove("GST_DEBUG_DUMP_DOT_DIR")
        .spawn()
        .map_err(|error| error.to_string())?;
    let stdout = child.stdout.take().ok_or("Audio-uitvoer ontbreekt")?;
    let (done, result) = mpsc::sync_channel(1);
    let reader = match std::thread::Builder::new()
        .name("pennel-gstreamer-decode".into())
        .spawn(move || {
            let decoded = read_pcm(stdout);
            let _ = done.send(decoded);
        }) {
        Ok(reader) => reader,
        Err(error) => {
            terminate(&mut child);
            return Err(error.to_string());
        }
    };
    let deadline = Instant::now() + PROCESS_TIMEOUT;
    let mut decoded = None;
    let status = loop {
        if decoded.is_none()
            && let Ok(result) = result.try_recv()
        {
            if result.is_err() {
                terminate(&mut child);
            }
            decoded = Some(result);
        }
        if let Some(status) = child.try_wait().map_err(|error| error.to_string())? {
            break status;
        }
        if Instant::now() >= deadline {
            terminate(&mut child);
            let _ = reader.join();
            return Err("Audiodecodering reageerde niet op tijd".into());
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let _ = reader.join();
    let decoded = decoded
        .or_else(|| result.try_recv().ok())
        .ok_or("Audiodecodering is onverwacht gestopt")??;
    if !status.success() {
        return Err("Audio kon niet worden gelezen. Controleer het M4A-bestand en de GStreamer AAC-decoder.".into());
    }
    if decoded.is_empty() {
        return Err("Het audiobestand bevat geen audiosamples".into());
    }
    Ok(decoded)
}

fn read_pcm(mut reader: impl Read) -> Result<Vec<f32>, String> {
    let mut samples = Vec::new();
    let mut block = [0u8; 8192];
    let mut pending = Vec::new();
    loop {
        let read = reader.read(&mut block).map_err(|error| error.to_string())?;
        if read == 0 {
            break;
        }
        pending.extend_from_slice(&block[..read]);
        let complete = pending.len() / 4 * 4;
        if samples.len().saturating_add(complete / 4) > MAX_SAMPLES {
            return Err("Audio langer dan drie uur kan niet worden verwerkt".into());
        }
        samples.extend(
            pending[..complete]
                .as_chunks::<4>()
                .0
                .iter()
                .map(|bytes| f32::from_le_bytes(*bytes)),
        );
        pending.drain(..complete);
    }
    if !pending.is_empty() {
        return Err("De decoder gaf een onvolledig audioblok terug".into());
    }
    Ok(samples)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pipelines_use_only_static_properties_and_file_descriptors() {
        let pipeline = encoder_pipeline("avenc_aac");
        assert!(pipeline.contains(&"fd=0") && pipeline.contains(&"fd=1"));
        assert!(!pipeline.iter().any(|token| token.starts_with("location=")));
        assert!(
            decoder_pipeline("avdec_aac")
                .contains(&"audio/x-raw,format=F32LE,rate=16000,channels=1,layout=interleaved")
        );
    }

    #[test]
    fn pcm_decoder_reassembles_partial_reads_and_rejects_incomplete_sample() {
        struct Fragmented {
            bytes: Vec<u8>,
            offset: usize,
        }
        impl Read for Fragmented {
            fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
                let take = (self.bytes.len() - self.offset).min(3).min(output.len());
                output[..take].copy_from_slice(&self.bytes[self.offset..self.offset + take]);
                self.offset += take;
                Ok(take)
            }
        }
        let bytes = [0.5f32, -0.25]
            .into_iter()
            .flat_map(f32::to_le_bytes)
            .collect();
        assert_eq!(
            read_pcm(Fragmented { bytes, offset: 0 }).unwrap(),
            [0.5, -0.25]
        );
        assert!(read_pcm(&b"bad"[..]).is_err());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn gstreamer_archive_roundtrip_and_mac_compatibility() {
        // CI installs the same packages as the Linux installer. A missing runtime must fail
        // explicitly instead of silently skipping the codec integration test.
        archive_support().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ik.m4a");
        let tone: Vec<_> = (0..8_000)
            .map(|index| (index as f32 * std::f32::consts::TAU * 440.0 / RATE as f32).sin() * 0.4)
            .collect();
        let mut archive = AudioArchive::create(&path).unwrap();
        archive.write(&tone).unwrap();
        archive.write_silence(16_000).unwrap();
        archive.write(&tone).unwrap();
        archive.finish().unwrap();
        let decoded = decode_audio(&path).unwrap();
        assert!((28_000..36_000).contains(&decoded.len()));
        assert!(decoded[27_000..].iter().any(|sample| sample.abs() > 0.2));
        assert!(
            decoded[12_000..20_000]
                .iter()
                .map(|sample| sample * sample)
                .sum::<f32>()
                / 8_000.0
                < 0.001
        );
        assert!(AudioArchive::create(&path).is_err());
        let mac_path = dir.path().join("mac.m4a");
        std::fs::write(
            &mac_path,
            include_bytes!("../windows_meetings/fixtures/mac16k.m4a"),
        )
        .unwrap();
        let decoded = decode_audio(&mac_path).unwrap();
        assert!((15_000..19_000).contains(&decoded.len()));
        assert!(decoded.iter().any(|sample| sample.abs() > 0.2));
    }
}
