//! Offline sketches of the instrument (see `doc/audio_concept.md`).
//!
//! `cargo test --release audio_sketches -- --ignored --nocapture` renders every scenario,
//! driven by the real content modules, tides and image features, to a 32-bit float WAV and a
//! spectrogram, and writes `report.md` with measurements. Output goes to `NOVA_AUDIO_DIR`,
//! or `nova-audio-sketches` in the OS temporary directory; `NOVA_SKETCH` renders only the
//! scenarios whose name contains its value.

mod analysis;

use std::fmt::Write as _;
use std::io::Write as _;
use std::path::Path;
use std::time::{Duration, Instant};

use std::sync::Arc;

use super::feed::{AudioFeed, FeedPublisher};
use super::instrument::{Instrument, Rhythm};
use crate::app_state::AppState;
use crate::content::all_content_names;
use crate::palettes::PALETTES;
use crate::renderer::{RenderState, Renderer};

const RATE: f32 = 48_000.0;
/// The display's frame time.
const FRAME_SECONDS: f32 = 0.04;

struct Scenario {
    name: String,
    description: String,
    content: String,
    seconds: f32,
    rhythm: Rhythm,
    /// Sets the controls at a point of the scenario, from 0 to 1.
    set: fn(f32, &mut AppState),
}

fn scenario(
    name: &str,
    description: &str,
    seconds: f32,
    rhythm: Rhythm,
    set: fn(f32, &mut AppState),
) -> Scenario {
    Scenario {
        name: name.into(),
        description: description.into(),
        content: "Orion's Belt".into(),
        seconds,
        rhythm,
        set,
    }
}

fn scenarios() -> Vec<Scenario> {
    use Rhythm::{Euclidean, Polymeter};
    let mut scenarios = vec![
        scenario(
            "01_cold_to_warm",
            "Pure drone (Form 0.05), Heat from 0 to 1",
            90.0,
            Euclidean,
            |t, state| {
                state.set_form(0.05);
                state.set_heat(t);
            },
        ),
        scenario(
            "02_ambient_to_rhythmic_euclidean",
            "Form from 0 to 1, Euclidean patterns on a 16-step bar",
            120.0,
            Euclidean,
            |t, state| state.set_form(t),
        ),
        scenario(
            "03_ambient_to_rhythmic_polymeter",
            "Form from 0 to 1, loops of 16, 12 and 7 steps",
            120.0,
            Polymeter,
            |t, state| state.set_form(t),
        ),
        scenario(
            "04_void_sweep",
            "Void from 0 to 1 (Form 0.3)",
            60.0,
            Euclidean,
            |t, state| {
                state.set_form(0.3);
                state.set_void(t);
            },
        ),
        scenario(
            "05_flow_sweep",
            "Flow from 0 to 1 (Form 0.6)",
            60.0,
            Euclidean,
            |t, state| {
                state.set_form(0.6);
                state.set_flow(t);
            },
        ),
        scenario(
            "06_palettes",
            "Every palette for 10 s (Form 0.35)",
            10.0 * PALETTES.len() as f32,
            Euclidean,
            |t, state| {
                let index = ((t * PALETTES.len() as f32) as usize).min(PALETTES.len() - 1);
                state.set_palette(PALETTES[index].name);
                state.set_form(0.35);
            },
        ),
        scenario(
            "07_cold_ambient",
            "Antarctica Freeze, Heat 0.1, Form 0.1, Flow 0.4, Void 0.6",
            60.0,
            Euclidean,
            |_, state| {
                state.set_palette("Antarctica Freeze");
                state.set_heat(0.1);
                state.set_form(0.1);
                state.set_flow(0.4);
                state.set_void(0.6);
            },
        ),
        scenario(
            "08_warm_ambient",
            "Sahara Drift, Heat 0.9, Form 0.1, Flow 0.4, Void 0.4",
            60.0,
            Euclidean,
            |_, state| {
                state.set_palette("Sahara Drift");
                state.set_heat(0.9);
                state.set_form(0.1);
                state.set_flow(0.4);
                state.set_void(0.4);
            },
        ),
        scenario(
            "09_cold_rhythmic",
            "Kiruna Glow, Heat 0.15, Form 0.8, Flow 0.6 (Euclidean)",
            60.0,
            Euclidean,
            |_, state| {
                state.set_palette("Kiruna Glow");
                state.set_heat(0.15);
                state.set_form(0.8);
                state.set_flow(0.6);
            },
        ),
        scenario(
            "10_warm_rhythmic",
            "Shenzhen Lantern Flicker, Heat 0.85, Form 0.8, Flow 0.55 (polymeter)",
            60.0,
            Polymeter,
            |_, state| {
                state.set_palette("Shenzhen Lantern Flicker");
                state.set_heat(0.85);
                state.set_form(0.8);
                state.set_flow(0.55);
            },
        ),
        scenario(
            "11_harmonic_motion",
            "Belize Ripple, Heat 0.6, Form 0.3, Flow 0.6 for 10 minutes: the bass moves about \
             every minute, a chord tone changes its octave halfway",
            600.0,
            Euclidean,
            |_, state| {
                state.set_palette("Belize Ripple");
                state.set_heat(0.6);
                state.set_form(0.3);
                state.set_flow(0.6);
            },
        ),
    ];
    for (index, content) in all_content_names().into_iter().enumerate() {
        let slug: String = content
            .to_lowercase()
            .chars()
            .filter_map(|c| match c {
                'a'..='z' => Some(c),
                ' ' => Some('_'),
                'ä' => Some('a'),
                _ => None,
            })
            .collect();
        scenarios.push(Scenario {
            name: format!("{:02}_module_{slug}", 12 + index),
            description: format!("{content} with every control at 0.5"),
            content,
            seconds: 45.0,
            rhythm: Euclidean,
            set: |_, _| {},
        });
    }
    scenarios
}

struct Rendering {
    left: Vec<f32>,
    right: Vec<f32>,
    synth_time: Duration,
    activity: (f32, f32),
    pulses: [u64; 3],
}

fn render(scenario: &Scenario) -> Rendering {
    let mut state = AppState::default();
    assert!(state.set_selected_content(&scenario.content));
    state.set_volume(1.0);
    let feed = Arc::new(AudioFeed::new(&state));
    let mut renderer = Renderer::new(state.dim());
    renderer.feed_audio(FeedPublisher::new(Arc::clone(&feed)));
    let mut instrument = Instrument::new(RATE, 7, scenario.rhythm);
    let frames = (scenario.seconds / FRAME_SECONDS).round() as usize;
    let per_frame = (RATE * FRAME_SECONDS) as usize;
    let mut rendering = Rendering {
        left: Vec::with_capacity(frames * per_frame),
        right: Vec::with_capacity(frames * per_frame),
        synth_time: Duration::ZERO,
        activity: (0.0, 0.0),
        pulses: [0; 3],
    };
    for frame in 0..frames {
        (scenario.set)(frame as f32 / (frames - 1).max(1) as f32, &mut state);
        let mut render_state = RenderState::from(&state);
        renderer.render_step(&mut render_state, FRAME_SECONDS);
        let features = feed.features();
        rendering.activity.0 += features.activity / frames as f32;
        rendering.activity.1 = rendering.activity.1.max(features.activity);
        instrument.set_features(features);
        instrument.set_controls(feed.controls());
        let started = Instant::now();
        for _ in 0..per_frame {
            let [left, right] = instrument.next_frame();
            rendering.left.push(left);
            rendering.right.push(right);
        }
        rendering.synth_time += started.elapsed();
    }
    rendering.pulses = instrument.pulse_counts();
    rendering
}

/// Writes a stereo WAV file of 32-bit floats.
fn save_wav(left: &[f32], right: &[f32], path: &Path) -> std::io::Result<()> {
    let frames = left.len().min(right.len()) as u32;
    let data = frames * 8;
    let mut file = std::io::BufWriter::new(std::fs::File::create(path)?);
    file.write_all(b"RIFF")?;
    file.write_all(&(4 + 26 + 12 + 8 + data).to_le_bytes())?;
    file.write_all(b"WAVEfmt ")?;
    file.write_all(&18_u32.to_le_bytes())?;
    file.write_all(&3_u16.to_le_bytes())?; // IEEE float
    file.write_all(&2_u16.to_le_bytes())?;
    file.write_all(&(RATE as u32).to_le_bytes())?;
    file.write_all(&(RATE as u32 * 8).to_le_bytes())?;
    file.write_all(&8_u16.to_le_bytes())?;
    file.write_all(&32_u16.to_le_bytes())?;
    file.write_all(&0_u16.to_le_bytes())?;
    file.write_all(b"fact")?;
    file.write_all(&4_u32.to_le_bytes())?;
    file.write_all(&frames.to_le_bytes())?;
    file.write_all(b"data")?;
    file.write_all(&data.to_le_bytes())?;
    for (left, right) in left.iter().zip(right) {
        file.write_all(&left.to_le_bytes())?;
        file.write_all(&right.to_le_bytes())?;
    }
    file.flush()
}

#[test]
#[ignore = "renders listening sketches and measurements to a directory"]
fn audio_sketches() {
    let directory = std::env::var_os("NOVA_AUDIO_DIR")
        .map(Into::into)
        .unwrap_or_else(|| std::env::temp_dir().join("nova-audio-sketches"));
    std::fs::create_dir_all(&directory).unwrap();
    let filter = std::env::var("NOVA_SKETCH").unwrap_or_default();

    let mut report = String::from(
        "# Audio sketches\n\nRendered by `audio_sketches` (see `doc/audio_concept.md`). \
         Loudness per ITU-R BS.1770 (LUFS) with a simplified loudness range (LRA), sample \
         peak, crest factor (peak to RMS), share of energy per band, side-to-mid energy below \
         80 Hz (the mono check for subwoofers), speed relative to real time, mean and maximum visual \
         activity, and the steps played by thumps, ticks and stabs.\n\n",
    );
    let bands: Vec<&str> = analysis::BANDS.iter().map(|band| band.2).collect();
    writeln!(
        report,
        "| Scenario | LUFS | LRA | Peak dBFS | Crest dB | {} Hz (%) | Side/mid < 80 Hz dB | × real time | Activity | Steps |",
        bands.join(" / ")
    )
    .unwrap();
    report.push_str("| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |\n");
    let mut descriptions = String::new();
    let octaves: Vec<String> = analysis::OCTAVES
        .iter()
        .map(|&centre| {
            if centre >= 1000.0 {
                format!("{:.0}k", centre / 1000.0)
            } else {
                format!("{centre:.0}")
            }
        })
        .collect();
    let mut balance = format!(
        "| Scenario | {} |\n|{}\n",
        octaves.join(" | "),
        " --- |".repeat(octaves.len() + 1)
    );

    for scenario in scenarios()
        .into_iter()
        .filter(|scenario| scenario.name.contains(&filter))
    {
        let rendering = render(&scenario);
        let (left, right) = (&rendering.left, &rendering.right);
        assert!(left.iter().chain(right).all(|sample| sample.is_finite()));
        let analysis = analysis::analyse(left, right, RATE);
        assert!(
            analysis.peak < -0.5,
            "{}: peak {}",
            scenario.name,
            analysis.peak
        );

        save_wav(
            left,
            right,
            &directory.join(format!("{}.wav", scenario.name)),
        )
        .unwrap();
        analysis::spectrogram(
            left,
            right,
            RATE,
            &directory.join(format!("{}.ppm", scenario.name)),
        )
        .unwrap();

        let speed = scenario.seconds / rendering.synth_time.as_secs_f32();
        let shares: Vec<String> = analysis
            .bands
            .iter()
            .map(|share| format!("{share:.0}"))
            .collect();
        writeln!(
            report,
            "| {} | {:.1} | {:.1} | {:.1} | {:.1} | {} | {:.0} | {:.0} | {:.3} / {:.2} | {} |",
            scenario.name,
            analysis.loudness,
            analysis.range,
            analysis.peak,
            analysis.crest,
            shares.join(" / "),
            analysis.side_to_mid_bass,
            speed,
            rendering.activity.0,
            rendering.activity.1,
            rendering
                .pulses
                .iter()
                .map(u64::to_string)
                .collect::<Vec<_>>()
                .join(" / "),
        )
        .unwrap();
        let levels: Vec<String> = analysis
            .octaves
            .iter()
            .map(|level| format!("{level:.0}"))
            .collect();
        writeln!(balance, "| {} | {} |", scenario.name, levels.join(" | ")).unwrap();
        writeln!(
            descriptions,
            "- **{}** ({:.0} s): {}",
            scenario.name, scenario.seconds, scenario.description
        )
        .unwrap();
        println!(
            "{}: {:.1} LUFS, peak {:.1} dBFS, side/mid bass {:.0} dB, DC {:.5}, {speed:.0}× real time",
            scenario.name, analysis.loudness, analysis.peak, analysis.side_to_mid_bass, analysis.dc
        );
    }
    report.push_str(
        "\n## Spectral balance\n\nEnergy per octave band relative to the total, in dB; \
         columns are the band centres in Hz.\n\n",
    );
    report.push_str(&balance);
    report.push_str("\n## Scenarios\n\n");
    report.push_str(&descriptions);
    std::fs::write(directory.join("report.md"), report).unwrap();
    println!("Wrote {}", directory.display());
}

/// Averages over all thumps of a minute at Form 0.8 (a thump on every beat), dry at Void 0:
/// the drone's gain from the instrument, and the rendered level of the chord band (300 to
/// 1500 Hz) relative to its highest point, in 20 ms steps after the thump. `NOVA_HEAT` sets
/// Heat (0.5), `NOVA_FLOW` Flow (0.5).
#[test]
#[ignore = "prints how the drone dips with the thumps"]
fn audio_ducking_profile() {
    use crate::audio::dsp::Svf;
    let setting = |name: &str, default: f32| {
        std::env::var(name)
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(default)
    };
    let mut state = AppState::default();
    state.set_volume(1.0);
    state.set_palette("Belize Ripple");
    state.set_form(0.8);
    state.set_void(0.0);
    state.set_heat(setting("NOVA_HEAT", 0.5));
    state.set_flow(setting("NOVA_FLOW", 0.5));
    let feed = Arc::new(AudioFeed::new(&state));
    let mut renderer = Renderer::new(state.dim());
    renderer.feed_audio(FeedPublisher::new(Arc::clone(&feed)));
    let mut instrument = Instrument::new(RATE, 7, Rhythm::Euclidean);
    let mut high = [Svf::new(300.0, 0.541, RATE), Svf::new(300.0, 1.307, RATE)];
    let mut low = [Svf::new(1500.0, 0.541, RATE), Svf::new(1500.0, 1.307, RATE)];

    const STEP: usize = 960; // 20 ms
    const STEPS: usize = 50;
    let (mut gains, mut powers, mut counts) = ([0.0_f64; STEPS], [0.0_f64; STEPS], [0_u32; STEPS]);
    let (mut since, mut thumps, mut window, mut gain_sum) = (usize::MAX, 0, 0.0_f64, 0.0_f64);
    let per_frame = (RATE * FRAME_SECONDS) as usize;
    for frame in 0..(60.0 / FRAME_SECONDS) as usize {
        renderer.render_step(&mut RenderState::from(&state), FRAME_SECONDS);
        instrument.set_features(feed.features());
        instrument.set_controls(feed.controls());
        for _ in 0..per_frame {
            let [left, right] = instrument.next_frame();
            let count = instrument.pulse_counts()[0];
            if count > thumps {
                thumps = count;
                since = 0;
                window = 0.0;
                gain_sum = 0.0;
            }
            let mid = 0.5 * (left + right);
            let high = high
                .iter_mut()
                .fold(mid, |value, filter| filter.highpass(value));
            let value = low
                .iter_mut()
                .fold(high, |value, filter| filter.lowpass(value));
            if since == usize::MAX {
                continue;
            }
            window += (value as f64).powi(2);
            gain_sum += (1.0 - instrument.duck()) as f64;
            since += 1;
            // Skip the first 5 s, while the sound settles.
            if since.is_multiple_of(STEP)
                && since / STEP <= STEPS
                && frame as f32 * FRAME_SECONDS > 5.0
            {
                let index = since / STEP - 1;
                powers[index] += window / STEP as f64;
                gains[index] += gain_sum / STEP as f64;
                counts[index] += 1;
                window = 0.0;
                gain_sum = 0.0;
            }
        }
    }
    let rows: Vec<(usize, f64, f64)> = (0..STEPS)
        .filter(|&index| counts[index] > counts[0] / 2)
        .map(|index| {
            let count = counts[index] as f64;
            (index, gains[index] / count, powers[index] / count)
        })
        .collect();
    let loudest = rows.iter().fold(0.0_f64, |max, row| max.max(row.2));
    println!(
        "Heat {}, Flow {}, {:.0} BPM, {} thumps",
        state.heat(),
        state.flow(),
        instrument.tempo(),
        counts[0]
    );
    println!("  ms after thump | drone gain dB | chord band dB");
    for (index, gain, power) in rows {
        println!(
            "  {:>14} | {:>13.1} | {:>13.1}",
            (index + 1) * 20,
            20.0 * gain.max(1e-6).log10(),
            10.0 * (power / loudest).log10()
        );
    }
}
