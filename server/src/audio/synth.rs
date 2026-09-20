use fundsp::prelude32::*;
use rand::{RngExt, SeedableRng, rngs::StdRng};

#[derive(Clone, Copy, Default)]
pub struct AudioControls {
    pub volume: f32,
    pub tone: f32,
    pub heat: f32,
    pub flow: f32,
    pub form: f32,
}

impl AudioControls {
    fn sanitized(self) -> Self {
        fn level(value: f32) -> f32 {
            if value.is_finite() {
                value.clamp(0.0, 1.0)
            } else {
                0.0
            }
        }
        Self {
            volume: level(self.volume),
            tone: level(self.tone),
            heat: level(self.heat),
            flow: level(self.flow),
            form: level(self.form),
        }
    }
}

fn smooth(value: f32) -> f32 {
    let value = value.clamp(0.0, 1.0);
    value * value * (3.0 - 2.0 * value)
}

fn pitch_frequency(note: f32) -> f32 {
    440.0 * 2.0_f32.powf((note - 69.0) / 12.0)
}

fn pitch_center(tone: f32) -> i32 {
    ((tone.rem_euclid(1.0) * 12.0).round() as i32 * 7) % 12
}

struct Voice {
    oscillator: Box<dyn AudioUnit>,
    filter: Box<dyn AudioUnit>,
    sample_rate: f32,
    age: f32,
    duration: f32,
    attack: f32,
    release: f32,
    note: f32,
    pan: [f32; 2],
    arp: bool,
    arp_step: f32,
    slide: f32,
    color_phase: f32,
    frequency: f32,
    width: f32,
    cutoff: f32,
    cutoff_target: f32,
    control_clock: usize,
}

impl Voice {
    fn new(sample_rate: f32) -> Self {
        let mut oscillator: Box<dyn AudioUnit> = Box::new(triangle() | pulse() | white());
        let mut filter: Box<dyn AudioUnit> = Box::new(lowpass());
        oscillator.set_sample_rate(sample_rate as f64);
        filter.set_sample_rate(sample_rate as f64);
        oscillator.allocate();
        filter.allocate();
        oscillator.tick(&[220.0, 220.0, 0.5], &mut [0.0; 3]);
        Self {
            oscillator,
            filter,
            sample_rate,
            age: 0.0,
            duration: 0.0,
            attack: 0.0,
            release: 0.0,
            note: 60.0,
            pan: [0.707; 2],
            arp: false,
            arp_step: 0.04,
            slide: 0.0,
            color_phase: 0.0,
            frequency: 220.0,
            width: 0.5,
            cutoff: 800.0,
            cutoff_target: 800.0,
            control_clock: 0,
        }
    }

    fn sample(&mut self, frequency: f32, width: f32, richness: f32, cutoff: f32) -> f32 {
        let mut waves = [0.0; 3];
        self.oscillator
            .tick(&[frequency, frequency, width], &mut waves);
        let input = waves[0] * (1.0 - richness * 0.8)
            + waves[1] * richness * 0.48
            + waves[2] * richness * 0.008;
        let mut output = [0.0];
        self.filter.tick(
            &[
                input,
                cutoff.clamp(40.0, self.sample_rate * 0.2),
                0.7 + richness * 0.6,
            ],
            &mut output,
        );
        output[0]
    }

    fn active(&self) -> bool {
        self.age < self.duration
    }

    fn start(&mut self, note: f32, controls: AudioControls, arp: bool, rng: &mut StdRng) {
        self.age = 0.0;
        self.duration = rng.random_range(4.5..8.0) + 3.0 * (1.0 - controls.flow);
        self.attack = rng.random_range(0.12..0.7) + 0.8 * (1.0 - controls.flow);
        self.release = self.duration * 0.65;
        self.note = note;
        let pan_angle = rng.random_range(0.25..0.75) * std::f32::consts::FRAC_PI_2;
        self.pan = [pan_angle.cos(), pan_angle.sin()];
        self.arp = arp;
        self.arp_step = rng.random_range(0.025..0.060);
        self.slide = if rng.random::<f32>() < 0.25 {
            -0.7
        } else {
            0.0
        };
        self.color_phase = rng.random_range(0.0..std::f32::consts::TAU);
        self.control_clock = 0;
    }

    fn next(&mut self, heat: f32) -> [f32; 2] {
        if !self.active() {
            return [0.0; 2];
        }
        let envelope =
            smooth(self.age / self.attack) * smooth((self.duration - self.age) / self.release);
        if self.control_clock == 0 {
            let arp_interval = if self.arp {
                [0.0, 7.0, 12.0][(self.age / self.arp_step) as usize % 3]
            } else {
                0.0
            };
            let bend = self.slide * (1.0 - smooth(self.age / 0.5));
            let drift = (self.age * 0.9 + self.color_phase).sin();
            self.frequency = pitch_frequency(self.note + arp_interval + bend + drift * 0.035);
            self.width = 0.38 + 0.1 * (self.age * 0.37 + self.color_phase).sin();
            let base = pitch_frequency(self.note);
            self.cutoff_target =
                (base * (2.0 + 5.0 * heat) * (1.0 + envelope * 0.7 + drift * 0.12))
                    .clamp(400.0, 4500.0);
        }
        self.control_clock = (self.control_clock + 1) % 64;
        self.cutoff += (self.cutoff_target - self.cutoff) * (1.0 / (0.02 * self.sample_rate));
        let sample = self.sample(self.frequency, self.width, heat, self.cutoff)
            * envelope
            * if self.arp { 0.105 } else { 0.13 };
        self.age += 1.0 / self.sample_rate;
        [sample * self.pan[0], sample * self.pan[1]]
    }
}

pub struct Synth {
    voices: [Voice; 3],
    rng: StdRng,
    sample_rate: f32,
    controls: AudioControls,
    target: AudioControls,
    gain: f32,
    event_phase: f64,
    event_spacing: f64,
    motif: [i32; 4],
    motif_position: usize,
    root: i32,
    delays: [Vec<f32>; 2],
    delay_positions: [usize; 2],
    echo_lowpass: [f32; 2],
    echo_coefficient: f32,
    dc_input: [f32; 2],
    dc_output: [f32; 2],
    dc_coefficient: f32,
    events: u64,
}

impl Synth {
    pub fn new(sample_rate: f32, seed: u64) -> Self {
        assert!(sample_rate.is_finite() && sample_rate >= 8000.0);
        Self {
            voices: std::array::from_fn(|_| Voice::new(sample_rate)),
            rng: StdRng::seed_from_u64(seed),
            sample_rate,
            controls: AudioControls::default(),
            target: AudioControls::default(),
            gain: 0.0,
            event_phase: 1.0,
            event_spacing: 1.0,
            motif: [0, 7, 4, 2],
            motif_position: 0,
            root: 0,
            delays: [
                vec![0.0; (sample_rate * 0.431) as usize],
                vec![0.0; (sample_rate * 0.647) as usize],
            ],
            delay_positions: [0; 2],
            echo_lowpass: [0.0; 2],
            echo_coefficient: 1.0 - (-std::f32::consts::TAU * 1800.0 / sample_rate).exp(),
            dc_input: [0.0; 2],
            dc_output: [0.0; 2],
            dc_coefficient: (-std::f32::consts::TAU * 15.0 / sample_rate).exp(),
            events: 0,
        }
    }

    pub fn set_controls(&mut self, controls: AudioControls) {
        self.target = controls.sanitized();
    }

    fn trigger(&mut self) {
        let active = self.voices.iter().filter(|voice| voice.active()).count();
        let wanted_root = pitch_center(self.target.tone);
        if wanted_root != self.root {
            if active > 0 {
                return;
            }
            self.root = wanted_root;
            self.motif_position = 0;
        }
        let voice_limit = 1 + (self.controls.form * 2.99) as usize;
        if active >= voice_limit {
            return;
        }
        let arp_active = self.voices.iter().any(|voice| voice.active() && voice.arp);
        let Some(voice) = self.voices.iter_mut().find(|voice| !voice.active()) else {
            return;
        };
        if self.motif_position == 0 {
            self.motif = [[0, 7, 4, 2], [4, 2, 0, 7], [0, 2, 9, 7]][self.rng.random_range(0..3)];
        }
        let note = 48 + self.root + self.motif[self.motif_position];
        let arp = !arp_active && self.rng.random::<f32>() < self.controls.form * 0.38;
        voice.start(note as f32, self.controls, arp, &mut self.rng);
        self.motif_position = (self.motif_position + 1) % 4;
        self.events += 1;
    }

    pub fn next_frame(&mut self) -> [f32; 2] {
        let slew = 1.0 / (0.08 * self.sample_rate);
        self.controls.heat += (self.target.heat - self.controls.heat) * slew;
        self.controls.flow += (self.target.flow - self.controls.flow) * slew;
        self.controls.form += (self.target.form - self.controls.form) * slew;
        let target_gain = self.target.volume * self.target.volume;
        self.gain += (target_gain - self.gain).clamp(
            -1.0 / (0.02 * self.sample_rate),
            1.0 / (0.02 * self.sample_rate),
        );
        self.event_phase += (0.095 + 0.48 * self.controls.flow as f64) / self.sample_rate as f64;
        if self.event_phase >= self.event_spacing {
            self.event_phase -= self.event_spacing;
            self.trigger();
            self.event_spacing = self.rng.random_range(0.8..1.3);
            if self.motif_position != 0 {
                self.event_spacing *= 1.0 - self.controls.form as f64 * 0.45;
            }
        }
        let mut mixed = [0.0; 2];
        for voice in &mut self.voices {
            let frame = voice.next(self.controls.heat);
            mixed[0] += frame[0];
            mixed[1] += frame[1];
        }
        for (channel, filtered) in self.echo_lowpass.iter_mut().enumerate() {
            let echo = self.delays[channel][self.delay_positions[channel]];
            *filtered += self.echo_coefficient * (echo - *filtered);
        }
        for (channel, sample) in mixed.iter_mut().enumerate() {
            self.delays[channel][self.delay_positions[channel]] =
                *sample + self.echo_lowpass[1 - channel] * 0.28;
            self.delay_positions[channel] =
                (self.delay_positions[channel] + 1) % self.delays[channel].len();
            let wet = *sample + self.echo_lowpass[channel] * 0.24;
            let blocked =
                wet - self.dc_input[channel] + self.dc_coefficient * self.dc_output[channel];
            self.dc_input[channel] = wet;
            self.dc_output[channel] = blocked;
            *sample = blocked.tanh() * self.gain;
        }
        mixed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filtered_voice_is_finite_and_bounded() {
        for sample_rate in [44_100.0, 48_000.0] {
            let mut voice = Voice::new(sample_rate);
            let mut energy = 0.0;
            for frame in 0..sample_rate as usize {
                let progress = frame as f32 / sample_rate;
                let sample = voice.sample(220.0, 0.35, progress, 400.0 + 4100.0 * progress);
                assert!(sample.is_finite() && sample.abs() < 2.0);
                energy += sample * sample;
            }
            assert!(energy > 10.0);
        }
    }

    #[test]
    fn lowpass_preserves_body_and_attenuates_highs() {
        for sample_rate in [44_100.0, 48_000.0] {
            let mut filter = lowpass_hz(1000.0, 0.8);
            filter.set_sample_rate(sample_rate);
            let body = filter.response(0, 200.0).unwrap().norm();
            let highs = filter.response(0, 8000.0).unwrap().norm();
            assert!(body > 0.9);
            assert!(highs < body * 0.03);
        }
    }

    #[test]
    fn zero_flow_evolves_and_mute_includes_echoes() {
        let mut synth = Synth::new(8000.0, 42);
        synth.set_controls(AudioControls {
            volume: 1.0,
            ..Default::default()
        });
        let mut energy = [0.0; 4];
        for frame in 0..480_000 {
            let sample = synth.next_frame()[0];
            assert!(sample.is_finite() && sample.abs() < 1.0);
            energy[frame / 120_000] += sample * sample;
        }
        assert!(synth.events >= 4);
        assert!(energy.iter().all(|energy| *energy > 1.0));
        synth.set_controls(AudioControls::default());
        for _ in 0..200 {
            synth.next_frame();
        }
        for _ in 0..8000 {
            assert_eq!(synth.next_frame(), [0.0; 2]);
        }
    }

    #[test]
    fn control_extremes_and_changes_remain_bounded() {
        for sample_rate in [44_100.0, 48_000.0] {
            let mut synth = Synth::new(sample_rate, 7);
            let mut peak = 0.0_f32;
            for frame in 0..(sample_rate as usize * 16) {
                if frame % 4000 == 0 {
                    let value = if frame % 8000 == 0 { 1.0 } else { 0.0 };
                    synth.set_controls(AudioControls {
                        volume: 1.0,
                        tone: value,
                        heat: value,
                        flow: 1.0,
                        form: 1.0,
                    });
                }
                for sample in synth.next_frame() {
                    assert!(sample.is_finite());
                    peak = peak.max(sample.abs());
                }
                assert!(
                    synth
                        .voices
                        .iter()
                        .filter(|voice| voice.active() && voice.arp)
                        .count()
                        <= 1
                );
            }
            assert!(peak > 0.01 && peak < 0.65, "peak {peak}");
        }
    }

    #[test]
    fn arp_and_envelope_timing_do_not_follow_live_flow() {
        let mut synth = Synth::new(8000.0, 9);
        let controls = AudioControls {
            volume: 1.0,
            form: 1.0,
            ..Default::default()
        };
        synth.voices[0].start(60.0, controls, true, &mut synth.rng);
        let duration = synth.voices[0].duration;
        let step = synth.voices[0].arp_step;
        synth.set_controls(AudioControls {
            flow: 1.0,
            ..controls
        });
        for _ in 0..8000 {
            synth.next_frame();
        }
        assert_eq!(synth.voices[0].duration, duration);
        assert_eq!(synth.voices[0].arp_step, step);
        assert!((synth.voices[0].age - 1.0).abs() < 0.001);
        assert_eq!(pitch_center(0.0), pitch_center(1.0));
    }

    #[test]
    fn invalid_controls_are_silent_and_finite() {
        let mut synth = Synth::new(8000.0, 1);
        synth.set_controls(AudioControls {
            volume: f32::NAN,
            heat: f32::INFINITY,
            flow: -1.0,
            form: 2.0,
            tone: f32::NEG_INFINITY,
        });
        for _ in 0..8000 {
            assert_eq!(synth.next_frame(), [0.0; 2]);
        }
    }

    #[test]
    #[ignore = "writes listening previews to the system temporary directory"]
    fn audio_listening_previews() {
        let directory = std::env::temp_dir().join("nova-audio");
        std::fs::create_dir_all(&directory).unwrap();
        for (name, controls) in [
            (
                "slow",
                AudioControls {
                    volume: 1.0,
                    tone: 0.0,
                    heat: 0.35,
                    flow: 0.0,
                    form: 0.5,
                },
            ),
            (
                "bleeps",
                AudioControls {
                    volume: 1.0,
                    tone: 0.25,
                    heat: 0.7,
                    flow: 0.65,
                    form: 0.65,
                },
            ),
            (
                "arpeggios",
                AudioControls {
                    volume: 1.0,
                    tone: 0.5,
                    heat: 1.0,
                    flow: 1.0,
                    form: 1.0,
                },
            ),
        ] {
            let mut synth = Synth::new(48_000.0, 42);
            synth.set_controls(controls);
            let mut wave = Wave::new(2, 48_000.0);
            let started = std::time::Instant::now();
            let mut peak = 0.0_f32;
            let mut jump = 0.0_f32;
            let mut previous = [0.0; 2];
            let mut arp_frames = 0;
            for frame_index in 0..1_440_000 {
                if frame_index == 1_416_000 {
                    synth.set_controls(AudioControls {
                        volume: 0.0,
                        ..controls
                    });
                }
                let frame = synth.next_frame();
                for channel in 0..2 {
                    assert!(frame[channel].is_finite());
                    peak = peak.max(frame[channel].abs());
                    jump = jump.max((frame[channel] - previous[channel]).abs());
                }
                if synth.voices.iter().any(|voice| voice.active() && voice.arp) {
                    arp_frames += 1;
                }
                previous = frame;
                wave.push((frame[0], frame[1]));
            }
            assert!(peak > 0.01 && peak < 0.65);
            assert!(jump < 0.15, "sample jump {jump}");
            if name == "arpeggios" {
                assert!(arp_frames > 0);
            }
            let path = directory.join(format!("{name}.wav"));
            wave.save_wav16(&path).unwrap();
            println!(
                "{}: peak {peak:.3}, max sample step {jump:.3}, {} events, {:.1}s arpeggiated, rendered in {:?}",
                path.display(),
                synth.events,
                arp_frames as f32 / 48_000.0,
                started.elapsed()
            );
        }
    }
}
