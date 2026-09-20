use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU32, Ordering},
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use cpal::{
    FromSample, SizedSample, Stream, StreamConfig,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};

use super::{AudioControls, Synth};
use crate::app_state::{AppState, Status};

#[derive(Default)]
struct SharedControls {
    values: [AtomicU32; 5],
}

impl SharedControls {
    fn publish(&self, state: &AppState) {
        for (slot, value) in self.values.iter().zip([
            state.volume(),
            state.tone(),
            state.heat(),
            state.flow(),
            state.form(),
        ]) {
            slot.store(value.to_bits(), Ordering::Relaxed);
        }
    }

    fn snapshot(&self) -> AudioControls {
        let values = self
            .values
            .each_ref()
            .map(|value| f32::from_bits(value.load(Ordering::Relaxed)));
        AudioControls {
            volume: values[0],
            tone: values[1],
            heat: values[2],
            flow: values[3],
            form: values[4],
        }
    }
}

pub struct AudioService {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl AudioService {
    pub fn start(state: Arc<Mutex<AppState>>) -> std::io::Result<Self> {
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let thread = thread::Builder::new()
            .name("nova-audio".into())
            .spawn(move || {
                let controls = Arc::new(SharedControls::default());
                let fault = Arc::new(AtomicBool::new(false));
                let mut stream = None;
                let mut retry_at = Instant::now();
                let mut audio_status = Status::Unknown;
                while !thread_stop.load(Ordering::Relaxed) {
                    if let Ok(state) = state.lock() {
                        controls.publish(&state);
                    }
                    if fault.swap(false, Ordering::Relaxed) {
                        stream = None;
                        retry_at = Instant::now() + Duration::from_secs(2);
                        audio_status =
                            Status::Err("Audio interrupted; retrying output device.".into());
                        log::warn!("Audio stream interrupted; retrying output device");
                    }
                    if stream.is_none() && Instant::now() >= retry_at {
                        match open_stream(Arc::clone(&controls), Arc::clone(&fault)) {
                            Ok(opened) => {
                                stream = Some(opened);
                                audio_status = Status::Ok("Audio ready".into());
                                log::info!("Audio output ready");
                            }
                            Err(error) => {
                                let message = format!("Audio unavailable: {error}");
                                let new_status = Status::Err(message.clone());
                                if audio_status != new_status {
                                    log::warn!("{message}");
                                }
                                audio_status = new_status;
                                retry_at = Instant::now() + Duration::from_secs(5);
                            }
                        }
                    }
                    match state.lock() {
                        Ok(mut state) if state.audio_status() != &audio_status => {
                            state.set_audio_status(audio_status.clone());
                        }
                        _ => {}
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

impl Drop for AudioService {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            thread.thread().unpark();
            let _ = thread.join();
        }
    }
}

fn open_stream(controls: Arc<SharedControls>, fault: Arc<AtomicBool>) -> Result<Stream, String> {
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
        config.buffer_size = cpal::BufferSize::Fixed(512_u32.clamp(*min, *max));
    }
    match supported.sample_format() {
        cpal::SampleFormat::F32 => build_stream::<f32>(&device, &config, controls, fault),
        cpal::SampleFormat::F64 => build_stream::<f64>(&device, &config, controls, fault),
        cpal::SampleFormat::I8 => build_stream::<i8>(&device, &config, controls, fault),
        cpal::SampleFormat::I16 => build_stream::<i16>(&device, &config, controls, fault),
        cpal::SampleFormat::I24 => build_stream::<cpal::I24>(&device, &config, controls, fault),
        cpal::SampleFormat::I32 => build_stream::<i32>(&device, &config, controls, fault),
        cpal::SampleFormat::I64 => build_stream::<i64>(&device, &config, controls, fault),
        cpal::SampleFormat::U8 => build_stream::<u8>(&device, &config, controls, fault),
        cpal::SampleFormat::U16 => build_stream::<u16>(&device, &config, controls, fault),
        cpal::SampleFormat::U32 => build_stream::<u32>(&device, &config, controls, fault),
        cpal::SampleFormat::U64 => build_stream::<u64>(&device, &config, controls, fault),
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
    controls: Arc<SharedControls>,
    fault: Arc<AtomicBool>,
) -> Result<Stream, String> {
    let mut synth = Synth::new(config.sample_rate as f32, rand::random());
    let channels = config.channels as usize;
    let stream = device
        .build_output_stream(
            *config,
            move |output: &mut [Sample], _| {
                synth.set_controls(controls.snapshot());
                for frame in output.chunks_mut(channels) {
                    write_frame(frame, synth.next_frame());
                }
            },
            move |_| {
                fault.store(true, Ordering::Relaxed);
            },
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
    fn control_snapshot_preserves_order_and_mute() {
        let shared = SharedControls::default();
        let mut state = AppState::default();
        state.set_volume(0.4);
        state.set_tone(0.2);
        state.set_heat(0.3);
        state.set_flow(0.5);
        state.set_form(0.6);
        shared.publish(&state);
        let controls = shared.snapshot();
        assert_eq!(
            [
                controls.volume,
                controls.tone,
                controls.heat,
                controls.flow,
                controls.form
            ],
            [0.4, 0.2, 0.3, 0.5, 0.6]
        );
        state.set_volume(0.0);
        shared.publish(&state);
        assert_eq!(shared.snapshot().volume, 0.0);
    }

    #[test]
    #[ignore = "requires an available native audio output device; remains muted"]
    fn native_audio_output_smoke() {
        let controls = Arc::new(SharedControls::default());
        let fault = Arc::new(AtomicBool::new(false));
        let stream = open_stream(controls, Arc::clone(&fault)).unwrap();
        thread::park_timeout(Duration::from_millis(500));
        assert!(!fault.load(Ordering::Relaxed));
        drop(stream);
    }
}
