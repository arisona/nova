//! Native output: plays the engine on the OS default device, from the bus.
//!
//! A service thread opens the stream, rebuilds it when it breaks, retries a missing device,
//! counts dropouts and leaves the sound's status on the bus. The stream uses the device's sample
//! rate and format, with mono downmix when needed; every new stream starts a new engine,
//! which fades in from silence with the instrument on the bus, and mixes over to any other
//! the bus asks for.

use std::hash::{BuildHasher, RandomState};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU32, Ordering},
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use cpal::{
    ErrorKind, FromSample, SizedSample, Stream, StreamConfig,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use nova_core::{Bus, Part, Status};

use crate::engine::Engine;
use crate::instruments::{INSTRUMENTS, Preset};

/// Seconds between reports of dropouts in the log.
const DROPOUT_REPORT_SECONDS: u64 = 10;

/// Frames per callback. ALSA keeps two of them buffered, so 2048 frames give the audio
/// thread about 40 ms of slack at 48 kHz, enough for the scheduling delays of a busy
/// Raspberry Pi; 512 underran there. Nothing is played live, so the latency of about 85 ms
/// only delays the response to the sliders slightly.
const PERIOD_FRAMES: u32 = 2048;

/// Whether the stream must be rebuilt after an error. Dropouts, a reroute to a new default
/// device and refused real-time scheduling leave it running.
fn is_fatal(kind: ErrorKind) -> bool {
    !matches!(
        kind,
        ErrorKind::Xrun | ErrorKind::DeviceChanged | ErrorKind::RealtimeDenied
    )
}

/// What the stream's error callback hands to the service thread. Dropouts are reported on
/// the real-time thread, so they only count.
#[derive(Default)]
struct Faults {
    dropouts: AtomicU32,
    /// Set when the stream must be rebuilt.
    fatal: AtomicBool,
    /// The latest error other than a dropout, for the log.
    latest: Mutex<Option<cpal::Error>>,
}

impl Faults {
    fn report(&self, error: cpal::Error) {
        let kind = error.kind();
        if kind == ErrorKind::Xrun {
            self.dropouts.fetch_add(1, Ordering::Relaxed);
            return;
        }
        if let Ok(mut latest) = self.latest.try_lock() {
            latest.get_or_insert(error);
        }
        if is_fatal(kind) {
            self.fatal.store(true, Ordering::Relaxed);
        }
    }
}

pub struct Output {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Output {
    /// Starts the service thread, which plays `INSTRUMENTS` from the bus as soon as a device
    /// is available, until the output is dropped.
    pub fn start(bus: Arc<Bus>) -> std::io::Result<Self> {
        let presets: Vec<Preset> = INSTRUMENTS.iter().map(|(_, preset)| *preset).collect();
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let thread = thread::Builder::new()
            .name("nova-audio".into())
            .spawn(move || {
                let faults = Arc::new(Faults::default());
                let mut stream = None;
                let mut retry_at = Instant::now();
                let mut dropouts_at = Instant::now();
                let mut status = Status::Unknown;
                while !thread_stop.load(Ordering::Relaxed) {
                    let latest = faults
                        .latest
                        .lock()
                        .ok()
                        .and_then(|mut latest| latest.take());
                    if let Some(error) = latest {
                        if is_fatal(error.kind()) {
                            log::warn!("Audio error: {error}");
                        } else {
                            log::info!("Audio: {error}");
                        }
                    }
                    if dropouts_at.elapsed() >= Duration::from_secs(DROPOUT_REPORT_SECONDS) {
                        dropouts_at = Instant::now();
                        let dropouts = faults.dropouts.swap(0, Ordering::Relaxed);
                        if dropouts > 0 {
                            log::warn!(
                                "Audio dropped out {dropouts} times in {DROPOUT_REPORT_SECONDS} s; \
                                 the engine may be too slow for this machine"
                            );
                        }
                    }
                    if faults.fatal.swap(false, Ordering::Relaxed) {
                        stream = None;
                        retry_at = Instant::now() + Duration::from_secs(2);
                        status = Status::Err("Audio interrupted; retrying output device.".into());
                        log::warn!("Audio stream interrupted; retrying output device");
                    }
                    if stream.is_none() && Instant::now() >= retry_at {
                        match open_stream(Arc::clone(&bus), &presets, Arc::clone(&faults)) {
                            Ok(opened) => {
                                stream = Some(opened);
                                status = Status::Ok("Audio ready".into());
                                log::info!("Audio output ready");
                            }
                            Err(error) => {
                                let message = format!("Audio unavailable: {error}");
                                let new_status = Status::Err(message.clone());
                                if status != new_status {
                                    log::warn!("{message}");
                                }
                                status = new_status;
                                retry_at = Instant::now() + Duration::from_secs(5);
                            }
                        }
                    }
                    if bus.status(Part::Audio) != status {
                        bus.report(Part::Audio, status.clone());
                    }
                    thread::park_timeout(Duration::from_millis(20));
                }
            })?;
        Ok(Self {
            stop,
            thread: Some(thread),
        })
    }
}

impl Drop for Output {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            thread.thread().unpark();
            let _ = thread.join();
        }
    }
}

fn open_stream(bus: Arc<Bus>, presets: &[Preset], faults: Arc<Faults>) -> Result<Stream, String> {
    let device = cpal::default_host()
        .default_output_device()
        .ok_or("No output device")?;
    let supported = device
        .default_output_config()
        .map_err(|error| error.to_string())?;
    let mut config = supported.config();
    if config.channels == 0 || config.sample_rate < 8000 {
        return Err("Unsupported channel count or sample rate".into());
    }
    if let cpal::SupportedBufferSize::Range { min, max } = supported.buffer_size() {
        config.buffer_size = cpal::BufferSize::Fixed(PERIOD_FRAMES.clamp(*min, *max));
    }
    match supported.sample_format() {
        cpal::SampleFormat::F32 => build_stream::<f32>(&device, &config, bus, presets, faults),
        cpal::SampleFormat::F64 => build_stream::<f64>(&device, &config, bus, presets, faults),
        cpal::SampleFormat::I8 => build_stream::<i8>(&device, &config, bus, presets, faults),
        cpal::SampleFormat::I16 => build_stream::<i16>(&device, &config, bus, presets, faults),
        cpal::SampleFormat::I24 => {
            build_stream::<cpal::I24>(&device, &config, bus, presets, faults)
        }
        cpal::SampleFormat::I32 => build_stream::<i32>(&device, &config, bus, presets, faults),
        cpal::SampleFormat::I64 => build_stream::<i64>(&device, &config, bus, presets, faults),
        cpal::SampleFormat::U8 => build_stream::<u8>(&device, &config, bus, presets, faults),
        cpal::SampleFormat::U16 => build_stream::<u16>(&device, &config, bus, presets, faults),
        cpal::SampleFormat::U32 => build_stream::<u32>(&device, &config, bus, presets, faults),
        cpal::SampleFormat::U64 => build_stream::<u64>(&device, &config, bus, presets, faults),
        format => Err(format!("Unsupported sample format: {format}")),
    }
}

fn write_frame<Sample: SizedSample + FromSample<f32>>(output: &mut [Sample], frame: [f32; 2]) {
    let channels = output.len();
    for (channel, sample) in output.iter_mut().enumerate() {
        let value = match (channels, channel) {
            (1, _) => (frame[0] + frame[1]) * 0.5,
            (_, 0 | 1) => frame[channel],
            _ => 0.0,
        };
        *sample = Sample::from_sample(value);
    }
}

fn build_stream<Sample: SizedSample + FromSample<f32>>(
    device: &cpal::Device,
    config: &StreamConfig,
    bus: Arc<Bus>,
    presets: &[Preset],
    faults: Arc<Faults>,
) -> Result<Stream, String> {
    let seed = RandomState::new().hash_one(Instant::now());
    let first = bus.instrument().min(presets.len() - 1);
    let mut engine = Engine::with_instruments(config.sample_rate as f32, seed, presets, first);
    let channels = config.channels as usize;
    let stream = device
        .build_output_stream(
            *config,
            move |output: &mut [Sample], _| {
                engine.set_controls(bus.controls());
                engine.set_instrument(bus.instrument());
                engine.set_clock(bus.clock());
                engine.set_light(bus.light());
                for frame in output.chunks_mut(channels) {
                    write_frame(frame, engine.next_frame());
                }
            },
            move |error| faults.report(error),
            Some(Duration::from_secs(2)),
        )
        .map_err(|error| error.to_string())?;
    stream.play().map_err(|error| error.to_string())?;
    Ok(stream)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_formats_and_channel_layouts() {
        let mut mono = [0.0_f32];
        write_frame(&mut mono, [0.2, 0.6]);
        assert_eq!(mono, [0.4]);
        let mut surround = [1.0_f32; 6];
        write_frame(&mut surround, [0.2, 0.6]);
        assert_eq!(surround, [0.2, 0.6, 0.0, 0.0, 0.0, 0.0]);
        let mut unsigned = [0_u16; 2];
        write_frame(&mut unsigned, [0.0; 2]);
        assert_eq!(unsigned, [32768; 2]);
        let mut signed = [0_i16; 2];
        write_frame(&mut signed, [-1.0, 0.5]);
        assert_eq!(signed, [i16::MIN, 16384]);
    }

    #[test]
    fn only_errors_that_break_the_stream_rebuild_it() {
        let faults = Faults::default();
        faults.report(ErrorKind::Xrun.into());
        faults.report(ErrorKind::Xrun.into());
        faults.report(ErrorKind::DeviceChanged.into());
        assert_eq!(faults.dropouts.load(Ordering::Relaxed), 2);
        assert!(!faults.fatal.load(Ordering::Relaxed));
        let latest = faults
            .latest
            .lock()
            .unwrap()
            .take()
            .map(|error| error.kind());
        assert_eq!(latest, Some(ErrorKind::DeviceChanged));
        faults.report(ErrorKind::DeviceNotAvailable.into());
        assert!(faults.fatal.load(Ordering::Relaxed));
    }

    #[test]
    #[ignore = "requires an available native audio output device; remains muted"]
    fn native_audio_output_smoke() {
        // Volume 0: the bus starts with every control in the middle and Volume muted.
        let bus = Arc::new(Bus::new(&Default::default()));
        let faults = Arc::new(Faults::default());
        let stream = open_stream(bus, &[Preset::default()], Arc::clone(&faults)).unwrap();
        thread::park_timeout(Duration::from_millis(500));
        assert!(!faults.fatal.load(Ordering::Relaxed));
        drop(stream);
    }
}
