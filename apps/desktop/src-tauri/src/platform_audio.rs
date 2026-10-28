//! Audio files stay in the backend; webviews address a meeting and track, never arbitrary files.

use std::path::Path;

pub fn decode_audio(path: &Path) -> Result<Vec<f32>, String> {
    #[cfg(target_os = "macos")]
    return crate::macos::decode_audio(path);
    #[cfg(windows)]
    return crate::windows_meetings::decode_audio(path);
    #[cfg(target_os = "linux")]
    return crate::linux_meetings::decode_audio(path);
    #[cfg(not(any(target_os = "macos", windows, target_os = "linux")))]
    {
        let _ = path;
        Err("Reading M4A audio is not available on this platform yet".into())
    }
}

pub fn play(path: &Path, start_ms: u64) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    return crate::macos::play_audio(path, start_ms);
    #[cfg(not(target_os = "macos"))]
    {
        playback::start(decode_audio(path)?, start_ms)
    }
}

pub fn stop() {
    #[cfg(target_os = "macos")]
    crate::macos::stop_audio();
    #[cfg(not(target_os = "macos"))]
    playback::stop();
}

#[cfg(not(target_os = "macos"))]
mod playback {
    use std::sync::{Mutex, OnceLock, mpsc};
    use std::thread::JoinHandle;

    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use cpal::{FromSample, SizedSample};

    struct Playback {
        stop: mpsc::Sender<()>,
        thread: JoinHandle<()>,
    }

    fn player() -> &'static Mutex<Option<Playback>> {
        static PLAYER: OnceLock<Mutex<Option<Playback>>> = OnceLock::new();
        PLAYER.get_or_init(|| Mutex::new(None))
    }

    pub fn stop() {
        if let Some(playback) = player().lock().unwrap().take() {
            let _ = playback.stop.send(());
            let _ = playback.thread.join();
        }
    }

    pub fn start(samples: Vec<f32>, start_ms: u64) -> Result<(), String> {
        stop();
        let (stop_tx, stop_rx) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let thread = std::thread::Builder::new()
            .name("pennel-playback".into())
            .spawn(move || {
                let result = open(samples, start_ms);
                match result {
                    Ok(stream) => {
                        let _ = ready_tx.send(Ok(()));
                        let _ = stop_rx.recv();
                        drop(stream);
                    }
                    Err(error) => {
                        let _ = ready_tx.send(Err(error));
                    }
                }
            })
            .map_err(|e| e.to_string())?;
        match ready_rx.recv().map_err(|e| e.to_string())? {
            Ok(()) => {
                *player().lock().unwrap() = Some(Playback {
                    stop: stop_tx,
                    thread,
                });
                Ok(())
            }
            Err(error) => {
                let _ = thread.join();
                Err(error)
            }
        }
    }

    fn open(samples: Vec<f32>, start_ms: u64) -> Result<cpal::Stream, String> {
        let device = cpal::default_host()
            .default_output_device()
            .ok_or("No audio output found")?;
        let supported = device.default_output_config().map_err(|e| e.to_string())?;
        let config = supported.config();
        let stream = match supported.sample_format() {
            cpal::SampleFormat::F32 => build::<f32>(&device, config, samples, start_ms),
            cpal::SampleFormat::I16 => build::<i16>(&device, config, samples, start_ms),
            cpal::SampleFormat::U16 => build::<u16>(&device, config, samples, start_ms),
            other => {
                return Err(format!("Audio output format {other:?} is unsupported"));
            }
        }
        .map_err(|e| e.to_string())?;
        stream.play().map_err(|e| e.to_string())?;
        Ok(stream)
    }

    fn build<T: SizedSample + FromSample<f32>>(
        device: &cpal::Device,
        config: cpal::StreamConfig,
        samples: Vec<f32>,
        start_ms: u64,
    ) -> Result<cpal::Stream, cpal::Error> {
        let mut position = start_ms as f64 * 16.0;
        let step = 16_000.0 / f64::from(config.sample_rate);
        let channels = usize::from(config.channels);
        device.build_output_stream::<T, _, _>(
            config,
            move |output: &mut [T], _| {
                for frame in output.chunks_mut(channels) {
                    let index = position as usize;
                    let a = samples.get(index).copied().unwrap_or(0.0);
                    let b = samples.get(index + 1).copied().unwrap_or(a);
                    let value = a + (b - a) * position.fract() as f32;
                    frame.fill(T::from_sample(value));
                    position += step;
                }
            },
            |error| {
                crate::diagnostics::Log::new("playback").error(format!("audio output: {error}"))
            },
            None,
        )
    }
}
