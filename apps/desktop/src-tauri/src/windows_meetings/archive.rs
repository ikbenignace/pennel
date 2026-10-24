use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use windows::Win32::Media::MediaFoundation::*;
use windows::core::PCWSTR;

use super::{AacPcm, ComApartment};
use crate::resample::MonoResampler;

const INPUT_RATE: u64 = 16_000;
const ENCODE_RATE: u64 = 48_000;
const MAX_SAMPLES: usize = INPUT_RATE as usize * 3 * 3600;
const QUEUE_BATCHES: usize = 32;
const BATCH_SAMPLES: usize = INPUT_RATE as usize;

/// The queue is bounded; a slow/full disk cannot consume an unbounded amount of recording memory.
pub struct AudioArchive {
    sender: Option<SyncSender<WriteCommand>>,
    worker: Option<JoinHandle<Result<(), String>>>,
    error: Arc<Mutex<Option<String>>>,
}

enum WriteCommand {
    Samples(Vec<f32>),
    Silence(u64),
}

impl AudioArchive {
    pub fn create(path: &Path) -> Result<Self, String> {
        if path.exists() {
            return Err("Het audiobestand bestaat al".into());
        }
        let path = path.to_owned();
        let (sender, receiver) = mpsc::sync_channel::<WriteCommand>(QUEUE_BATCHES);
        let (ready_sender, ready) = mpsc::sync_channel(1);
        let error = Arc::new(Mutex::new(None));
        let worker_error = error.clone();
        let worker = std::thread::Builder::new()
            .name("pennel-audio-archive".into())
            .spawn(move || {
                let result = (|| {
                    let _media = MediaFoundation::new()?;
                    let mut writer = AacWriter::create(&path)?;
                    let _ = ready_sender.send(Ok(()));
                    let mut failure = None;
                    for command in receiver {
                        if failure.is_none()
                            && let Err(error) = match command {
                                WriteCommand::Samples(samples) => writer.write(&samples),
                                WriteCommand::Silence(samples) => writer.write_silence(samples),
                            }
                        {
                            *worker_error.lock().unwrap() = Some(error.clone());
                            failure = Some(error);
                        }
                    }
                    let finalized = writer.finish();
                    failure.map_or(finalized, Err)
                })();
                if let Err(error) = &result {
                    *worker_error.lock().unwrap() = Some(error.clone());
                    let _ = ready_sender.send(Err(error.clone()));
                }
                result
            })
            .map_err(|error| error.to_string())?;
        match ready.recv() {
            Ok(Ok(())) => Ok(Self {
                sender: Some(sender),
                worker: Some(worker),
                error,
            }),
            result => {
                drop(sender);
                let _ = worker.join();
                Err(match result {
                    Ok(Err(error)) => error,
                    _ => "Audio-opslag kon niet worden gestart".into(),
                })
            }
        }
    }

    /// Accepts normalized 16 kHz mono Float32. Errors latch until finish, even if a caller ignores
    /// a write result, so a truncated archive is never reported as a successfully saved recording.
    pub fn write(&mut self, samples: &[f32]) -> Result<(), String> {
        if let Some(error) = self.error.lock().unwrap().as_ref() {
            return Err(error.clone());
        }
        for batch in samples.chunks(BATCH_SAMPLES) {
            self.send(WriteCommand::Samples(batch.to_vec()))?;
        }
        Ok(())
    }

    /// Gaps are queued as a sample count, so a suspended/disconnected device never creates a
    /// correspondingly large allocation or floods the bounded queue with individual silence blocks.
    pub fn write_silence(&mut self, samples: u64) -> Result<(), String> {
        if samples == 0 {
            return Ok(());
        }
        self.send(WriteCommand::Silence(samples))
    }

    fn send(&mut self, command: WriteCommand) -> Result<(), String> {
        if let Some(error) = self.error.lock().unwrap().as_ref() {
            return Err(error.clone());
        }
        let Some(sender) = self.sender.as_ref() else {
            return Err("Audio-opslag is gesloten".into());
        };
        sender.try_send(command).map_err(|error| {
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
        let finalized = self.worker.take().map_or(Ok(()), |worker| {
            worker
                .join()
                .map_err(|_| "Audio-opslag is onverwacht gestopt".to_owned())?
        });
        self.error.lock().unwrap().clone().map_or(finalized, Err)
    }
}

impl Drop for AudioArchive {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

struct MediaFoundation {
    _apartment: ComApartment,
}

impl MediaFoundation {
    fn new() -> Result<Self, String> {
        let apartment = ComApartment::new()?;
        unsafe { MFStartup(MF_VERSION, MFSTARTUP_FULL) }
            .map_err(|error| format!("Windows-mediaondersteuning is niet beschikbaar: {error}"))?;
        Ok(Self {
            _apartment: apartment,
        })
    }
}

impl Drop for MediaFoundation {
    fn drop(&mut self) {
        unsafe {
            let _ = MFShutdown();
        }
    }
}

fn wide_path(path: &Path) -> Result<Vec<u16>, String> {
    use std::os::windows::ffi::OsStrExt;
    let path: Vec<_> = path.as_os_str().encode_wide().collect();
    if path.contains(&0) {
        return Err("Ongeldig audiopad".into());
    }
    Ok(path.into_iter().chain([0]).collect())
}

struct AacWriter {
    writer: IMFSinkWriter,
    stream: u32,
    frames: u64,
    converter: AacPcm,
}

impl AacWriter {
    fn create(path: &Path) -> Result<Self, String> {
        let path = wide_path(path)?;
        unsafe {
            let writer = MFCreateSinkWriterFromURL(
                PCWSTR(path.as_ptr()),
                None::<&IMFByteStream>,
                None::<&IMFAttributes>,
            )
            .map_err(|error| format!("M4A-opslag kon niet starten: {error}"))?;
            let output = MFCreateMediaType().map_err(|error| error.to_string())?;
            set_audio_type(&output, &MFAudioFormat_AAC, ENCODE_RATE as u32, 16)?;
            output
                .SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, 12_000)
                .map_err(|error| error.to_string())?;
            output
                .SetUINT32(&MF_MT_AAC_PAYLOAD_TYPE, 0)
                .map_err(|error| error.to_string())?;
            let stream = writer
                .AddStream(&output)
                .map_err(|error| error.to_string())?;
            let input = MFCreateMediaType().map_err(|error| error.to_string())?;
            set_audio_type(&input, &MFAudioFormat_PCM, ENCODE_RATE as u32, 16)?;
            input
                .SetUINT32(&MF_MT_AUDIO_BLOCK_ALIGNMENT, 2)
                .map_err(|error| error.to_string())?;
            input
                .SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, ENCODE_RATE as u32 * 2)
                .map_err(|error| error.to_string())?;
            writer
                .SetInputMediaType(stream, &input, None::<&IMFAttributes>)
                .map_err(|error| error.to_string())?;
            writer.BeginWriting().map_err(|error| error.to_string())?;
            Ok(Self {
                writer,
                stream,
                frames: 0,
                converter: AacPcm::default(),
            })
        }
    }

    fn write(&mut self, samples: &[f32]) -> Result<(), String> {
        if samples.is_empty() {
            return Ok(());
        }
        let pcm = self.converter.convert(samples);
        let bytes: Vec<_> = pcm.iter().flat_map(|sample| sample.to_le_bytes()).collect();
        let length = u32::try_from(bytes.len()).map_err(|_| "Audioblok is te groot")?;
        unsafe {
            let buffer = MFCreateMemoryBuffer(length).map_err(|error| error.to_string())?;
            {
                let locked = LockedBuffer::new(&buffer)?;
                if locked.capacity < length || locked.data.is_null() {
                    return Err("Windows gaf een ongeldig audiobuffer terug".into());
                }
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), locked.data, bytes.len());
            }
            buffer
                .SetCurrentLength(length)
                .map_err(|error| error.to_string())?;
            let sample = MFCreateSample().map_err(|error| error.to_string())?;
            sample
                .AddBuffer(&buffer)
                .map_err(|error| error.to_string())?;
            let end = self
                .frames
                .checked_add(pcm.len() as u64)
                .ok_or("Opname is te lang")?;
            let clock = |frames: u64| -> Result<i64, String> {
                i64::try_from(u128::from(frames) * 10_000_000 / u128::from(ENCODE_RATE))
                    .map_err(|_| "Opname is te lang".into())
            };
            let start_time = clock(self.frames)?;
            sample
                .SetSampleTime(start_time)
                .map_err(|error| error.to_string())?;
            sample
                .SetSampleDuration(clock(end)? - start_time)
                .map_err(|error| error.to_string())?;
            self.writer
                .WriteSample(self.stream, &sample)
                .map_err(|error| error.to_string())?;
            self.frames = end;
        }
        Ok(())
    }

    fn finish(self) -> Result<(), String> {
        unsafe { self.writer.Finalize() }.map_err(|error| format!("M4A afronden mislukt: {error}"))
    }

    fn write_silence(&mut self, mut samples: u64) -> Result<(), String> {
        static SILENCE: [f32; BATCH_SAMPLES] = [0.0; BATCH_SAMPLES];
        while samples != 0 {
            let length = samples.min(BATCH_SAMPLES as u64) as usize;
            self.write(&SILENCE[..length])?;
            samples -= length as u64;
        }
        Ok(())
    }
}

unsafe fn set_audio_type(
    media_type: &IMFMediaType,
    subtype: &windows::core::GUID,
    rate: u32,
    bits: u32,
) -> Result<(), String> {
    unsafe {
        media_type
            .SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)
            .map_err(|error| error.to_string())?;
        media_type
            .SetGUID(&MF_MT_SUBTYPE, subtype)
            .map_err(|error| error.to_string())?;
        media_type
            .SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, 1)
            .map_err(|error| error.to_string())?;
        media_type
            .SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, rate)
            .map_err(|error| error.to_string())?;
        media_type
            .SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, bits)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

struct LockedBuffer<'a> {
    buffer: &'a IMFMediaBuffer,
    data: *mut u8,
    capacity: u32,
    length: u32,
}

impl<'a> LockedBuffer<'a> {
    fn new(buffer: &'a IMFMediaBuffer) -> Result<Self, String> {
        let mut data = std::ptr::null_mut();
        let mut capacity = 0;
        let mut length = 0;
        unsafe { buffer.Lock(&mut data, Some(&mut capacity), Some(&mut length)) }
            .map_err(|error| error.to_string())?;
        Ok(Self {
            buffer,
            data,
            capacity,
            length,
        })
    }
}

impl Drop for LockedBuffer<'_> {
    fn drop(&mut self) {
        unsafe {
            let _ = self.buffer.Unlock();
        }
    }
}

/// Decode historical Mac M4A and Windows M4A into the core's normalized format. The three-hour
/// bound matches the native Mac diarization limit and is checked before extending the result.
pub fn decode_audio(path: &Path) -> Result<Vec<f32>, String> {
    let path = path.to_owned();
    std::thread::Builder::new()
        .name("pennel-audio-decode".into())
        .spawn(move || decode_mta(path))
        .map_err(|error| error.to_string())?
        .join()
        .map_err(|_| "Audiodecodering is onverwacht gestopt".to_owned())?
}

fn decode_mta(path: PathBuf) -> Result<Vec<f32>, String> {
    let _media = MediaFoundation::new()?;
    let path = wide_path(&path)?;
    unsafe {
        let reader = MFCreateSourceReaderFromURL(PCWSTR(path.as_ptr()), None::<&IMFAttributes>)
            .map_err(|error| format!("Audio openen mislukt: {error}"))?;
        let stream = MF_SOURCE_READER_FIRST_AUDIO_STREAM.0 as u32;
        reader
            .SetStreamSelection(MF_SOURCE_READER_ALL_STREAMS.0 as u32, false)
            .map_err(|error| error.to_string())?;
        reader
            .SetStreamSelection(stream, true)
            .map_err(|error| error.to_string())?;
        let output = MFCreateMediaType().map_err(|error| error.to_string())?;
        output
            .SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)
            .map_err(|error| error.to_string())?;
        output
            .SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_Float)
            .map_err(|error| error.to_string())?;
        reader
            .SetCurrentMediaType(stream, None, &output)
            .map_err(|error| error.to_string())?;
        let format = reader
            .GetCurrentMediaType(stream)
            .map_err(|error| error.to_string())?;
        let rate = format
            .GetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND)
            .map_err(|error| error.to_string())?;
        let channels = format
            .GetUINT32(&MF_MT_AUDIO_NUM_CHANNELS)
            .map_err(|error| error.to_string())?;
        if rate == 0 || channels == 0 || channels > u16::MAX.into() {
            return Err("Ongeldig audioformaat".into());
        }
        let mut resampler = MonoResampler::new(rate, channels as u16);
        let mut samples = Vec::new();
        loop {
            let mut flags = 0u32;
            let mut sample = None;
            reader
                .ReadSample(stream, 0, None, Some(&mut flags), None, Some(&mut sample))
                .map_err(|error| error.to_string())?;
            if flags & MF_SOURCE_READERF_ERROR.0 as u32 != 0 {
                return Err("Audio bevat een onleesbaar blok".into());
            }
            if flags & MF_SOURCE_READERF_CURRENTMEDIATYPECHANGED.0 as u32 != 0 {
                return Err("Het audioformaat verandert tijdens het bestand".into());
            }
            if let Some(sample) = sample {
                let buffer = sample
                    .ConvertToContiguousBuffer()
                    .map_err(|error| error.to_string())?;
                let locked = LockedBuffer::new(&buffer)?;
                if locked.length > locked.capacity
                    || locked.length % 4 != 0
                    || (locked.length != 0 && locked.data.is_null())
                {
                    return Err("Windows gaf een ongeldig audiobuffer terug".into());
                }
                if locked.length != 0 {
                    let bytes = std::slice::from_raw_parts(locked.data, locked.length as usize);
                    let decoded: Vec<_> = bytes
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .map(|bytes| f32::from_le_bytes(*bytes))
                        .collect();
                    let normalized = resampler.push(&decoded);
                    if samples.len().saturating_add(normalized.len()) > MAX_SAMPLES {
                        return Err("Audio langer dan drie uur kan niet worden verwerkt".into());
                    }
                    samples.extend(normalized);
                }
            }
            if flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
                break;
            }
        }
        Ok(samples)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_native_mac_16k_aac_archive() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ik.m4a");
        std::fs::write(&path, include_bytes!("fixtures/mac16k.m4a")).unwrap();
        let decoded = decode_audio(&path).unwrap();
        assert!((15_000..19_000).contains(&decoded.len()));
        assert!(decoded.iter().any(|sample| sample.abs() > 0.2));
    }

    #[test]
    fn m4a_archive_roundtrip_keeps_duration_and_audio() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("anderen.m4a");
        let samples: Vec<_> = (0..INPUT_RATE as usize * 2)
            .map(|index| {
                (index as f32 * std::f32::consts::TAU * 440.0 / INPUT_RATE as f32).sin() * 0.4
            })
            .collect();
        let mut archive = AudioArchive::create(&path).unwrap();
        for batch in samples.chunks(1024) {
            archive.write(batch).unwrap();
        }
        archive.finish().unwrap();
        let decoded = decode_audio(&path).unwrap();
        assert!((28_000..36_000).contains(&decoded.len()));
        assert!(decoded.iter().any(|sample| sample.abs() > 0.2));
        assert!(AudioArchive::create(&path).is_err());
    }

    #[test]
    fn m4a_archive_preserves_a_device_interruption_as_silence() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ik.m4a");
        let tone: Vec<_> = (0..8_000)
            .map(|index| {
                (index as f32 * std::f32::consts::TAU * 440.0 / INPUT_RATE as f32).sin() * 0.4
            })
            .collect();
        let mut archive = AudioArchive::create(&path).unwrap();
        archive.write(&tone).unwrap();
        archive.write_silence(16_000).unwrap();
        archive.write(&tone).unwrap();
        archive.finish().unwrap();
        let decoded = decode_audio(&path).unwrap();
        assert!((28_000..36_000).contains(&decoded.len()));
        let middle_energy = decoded[12_000..20_000]
            .iter()
            .map(|sample| sample * sample)
            .sum::<f32>()
            / 8_000.0;
        assert!(middle_energy < 0.001);
        assert!(decoded[27_000..].iter().any(|sample| sample.abs() > 0.2));
    }
}
