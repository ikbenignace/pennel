//! Audio capture as 16 kHz mono f32. cpal streams are not `Send` everywhere, so each capture
//! owns a thread that builds, runs and drops its stream.

use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SizedSample};

use crate::diagnostics::Log;
use crate::resample::{MonoResampler, rms};

const LOG: Log = Log::new("audio");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Microphone,
    /// What the default output device plays (WASAPI loopback, PulseAudio/PipeWire monitor).
    SystemOutput,
}

#[derive(Debug)]
pub enum CaptureError {
    NoDevice,
    Failed(String),
}

impl std::fmt::Display for CaptureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CaptureError::NoDevice => write!(f, "geen apparaat gevonden"),
            CaptureError::Failed(message) => write!(f, "{message}"),
        }
    }
}

pub struct Capture {
    stop: Sender<()>,
    thread: Option<JoinHandle<()>>,
}

impl Capture {
    /// `sink` receives 16 kHz mono chunks on the audio thread; keep it cheap.
    pub fn start(
        source: Source,
        sink: impl FnMut(&[f32]) + Send + 'static,
    ) -> Result<Self, CaptureError> {
        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<(), CaptureError>>();
        let thread = std::thread::Builder::new()
            .name("pennel-audio".into())
            .spawn(move || match open(source, sink) {
                Ok(stream) => {
                    let _ = ready_tx.send(Ok(()));
                    let _ = stop_rx.recv();
                    drop(stream);
                }
                Err(e) => {
                    let _ = ready_tx.send(Err(e));
                }
            })
            .map_err(|e| CaptureError::Failed(e.to_string()))?;
        ready_rx
            .recv()
            .map_err(|e| CaptureError::Failed(e.to_string()))??;
        Ok(Self {
            stop: stop_tx,
            thread: Some(thread),
        })
    }

    pub fn stop(mut self) {
        self.finish();
    }

    fn finish(&mut self) {
        let _ = self.stop.send(());
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        self.finish();
    }
}

fn device(source: Source) -> Result<cpal::Device, CaptureError> {
    let host = cpal::default_host();
    match source {
        Source::Microphone => host.default_input_device(),
        #[cfg(target_os = "linux")]
        Source::SystemOutput => linux_monitor(&host),
        #[cfg(not(target_os = "linux"))]
        Source::SystemOutput => host.default_output_device(),
    }
    .ok_or(CaptureError::NoDevice)
}

/// PulseAudio and PipeWire expose what a sink plays as a "<sink>.monitor" input source.
#[cfg(target_os = "linux")]
fn linux_monitor(host: &cpal::Host) -> Option<cpal::Device> {
    let wanted = host
        .default_output_device()
        .and_then(|d| d.id().ok())
        .map(|id| format!("{}.monitor", id.id()));
    let monitors: Vec<cpal::Device> = host
        .input_devices()
        .ok()?
        .filter(|d| d.id().is_ok_and(|id| id.id().ends_with(".monitor")))
        .collect();
    let matching = wanted.and_then(|wanted| {
        monitors
            .iter()
            .find(|d| d.id().is_ok_and(|id| id.id() == wanted))
    });
    matching.or(monitors.first()).cloned()
}

fn open(
    source: Source,
    sink: impl FnMut(&[f32]) + Send + 'static,
) -> Result<cpal::Stream, CaptureError> {
    let device = device(source)?;
    let supported = match source {
        Source::Microphone => device.default_input_config(),
        Source::SystemOutput => device
            .default_input_config()
            .or_else(|_| device.default_output_config()),
    }
    .map_err(|e| CaptureError::Failed(e.to_string()))?;
    let config = supported.config();
    if config.sample_rate == 0 || config.channels == 0 {
        return Err(CaptureError::NoDevice);
    }
    LOG.info(format!(
        "{source:?}: {} Hz, {} kanalen, {:?}",
        config.sample_rate,
        config.channels,
        supported.sample_format()
    ));
    let stream = match supported.sample_format() {
        cpal::SampleFormat::F32 => build::<f32>(&device, config, sink),
        cpal::SampleFormat::I16 => build::<i16>(&device, config, sink),
        cpal::SampleFormat::U16 => build::<u16>(&device, config, sink),
        cpal::SampleFormat::I32 => build::<i32>(&device, config, sink),
        cpal::SampleFormat::F64 => build::<f64>(&device, config, sink),
        cpal::SampleFormat::U8 => build::<u8>(&device, config, sink),
        other => return Err(CaptureError::Failed(format!("formaat {other:?}"))),
    }
    .map_err(|e| CaptureError::Failed(e.to_string()))?;
    stream
        .play()
        .map_err(|e| CaptureError::Failed(e.to_string()))?;
    Ok(stream)
}

fn build<T>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    mut sink: impl FnMut(&[f32]) + Send + 'static,
) -> Result<cpal::Stream, cpal::Error>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let mut resampler = MonoResampler::new(config.sample_rate, config.channels);
    let mut floats = Vec::new();
    device.build_input_stream::<T, _, _>(
        config,
        move |data: &[T], _| {
            floats.clear();
            floats.extend(data.iter().map(|s| s.to_sample::<f32>()));
            let mono = resampler.push(&floats);
            if !mono.is_empty() {
                sink(&mono);
            }
        },
        |e| LOG.error(format!("audiostroom: {e}")),
        None,
    )
}

/// A dictation recording: all samples plus a level for the pill.
pub struct Recorder {
    capture: Capture,
    samples: Arc<Mutex<Vec<f32>>>,
}

impl Recorder {
    pub fn start(on_level: impl Fn(f32) + Send + 'static) -> Result<Self, CaptureError> {
        let samples = Arc::new(Mutex::new(Vec::with_capacity(16_000 * 30)));
        let sink_samples = samples.clone();
        let capture = Capture::start(Source::Microphone, move |chunk| {
            sink_samples.lock().unwrap().extend_from_slice(chunk);
            on_level(rms(chunk));
        })?;
        Ok(Self { capture, samples })
    }

    pub fn stop(self) -> Vec<f32> {
        self.capture.stop();
        std::mem::take(&mut *self.samples.lock().unwrap())
    }
}

/// Exact zeros only: a quiet room always carries some noise, so this means a broken capture path.
pub fn is_digital_silence(samples: &[f32]) -> bool {
    !samples.is_empty() && samples.iter().all(|s| *s == 0.0)
}
