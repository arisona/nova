//! Offline sketches of the instruments (see `doc/audio_concept.md`).
//!
//! `cargo run --release -p nova-audio --example sketches -- [filter]` renders
//! every scenario whose name contains the filter to a 32-bit float WAV and a spectrogram,
//! and writes `report.md` with measurements, to `NOVA_AUDIO_DIR`, or `nova-audio-sketches`
//! in the OS temporary directory. `… -- ducking [heat] [flow]` prints how Strata's drone
//! dips with the thumps, `… -- features` what every content module hands to the sound.
//!
//! Scripted scenarios move the controls like the sliders, with Nova's palettes, and hand
//! over a neutral light. The content modules are rendered by the real renderer through a
//! bus (`nova-voxel`, a dependency of this example only), as Nova renders them.

mod analysis;
mod scenarios;

use std::fmt::Write as _;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use nova_audio::{Activity, Engine, Preset};
use nova_core::{Features, HUES};
use scenarios::{FRAME_SECONDS, Scenario};

const RATE: f32 = 48_000.0;

fn fail(message: &str) -> ! {
    eprintln!("{message}");
    std::process::exit(1);
}

fn directory() -> PathBuf {
    std::env::var_os("NOVA_AUDIO_DIR")
        .map(Into::into)
        .unwrap_or_else(|| std::env::temp_dir().join("nova-audio-sketches"))
}

/// The names of Nova's palettes, in order.
fn palettes() -> Vec<String> {
    nova_core::PALETTES
        .iter()
        .map(|palette| palette.name.to_string())
        .collect()
}

/// An engine that plays `frame`.
fn play(engine: &mut Engine, frame: &scenarios::Frame) {
    engine.set_controls(frame.controls);
    engine.set_clock(frame.clock);
    engine.set_light(frame.light);
}

struct Rendering {
    left: Vec<f32>,
    right: Vec<f32>,
    synth_time: Duration,
    activity: (f32, f32),
    pulses: [u64; 3],
}

fn render(scenario: &Scenario) -> Rendering {
    let mut engine = Engine::new(RATE, 7, &scenario.preset);
    let per_frame = (RATE * FRAME_SECONDS) as usize;
    let frames = scenario.frames.len();
    let mut rendering = Rendering {
        left: Vec::with_capacity(frames * per_frame),
        right: Vec::with_capacity(frames * per_frame),
        synth_time: Duration::ZERO,
        activity: (0.0, 0.0),
        pulses: [0; 3],
    };
    for frame in &scenario.frames {
        let activity = frame.light.activity;
        rendering.activity.0 += activity / frames as f32;
        rendering.activity.1 = rendering.activity.1.max(activity);
        play(&mut engine, frame);
        let started = Instant::now();
        for _ in 0..per_frame {
            let [left, right] = engine.next_frame();
            rendering.left.push(left);
            rendering.right.push(right);
        }
        rendering.synth_time += started.elapsed();
    }
    rendering.pulses = engine.steps();
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

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let directory = directory();
    let palettes = palettes();
    match args.first().map(String::as_str) {
        Some("ducking") => ducking(&palettes, &args[1..]),
        Some("features") => features(),
        filter => sketches(&directory, &palettes, filter.unwrap_or_default()),
    }
}

fn sketches(directory: &Path, names: &[String], filter: &str) {
    let mut report = String::from(
        "# Audio sketches\n\nRendered by the `sketches` example of `nova-audio` (see \
         `doc/audio_concept.md`). Loudness per ITU-R BS.1770 (LUFS) with a simplified \
         loudness range (LRA), sample peak, crest factor (peak to RMS), share of energy per \
         band, side-to-mid energy below 80 Hz (the mono check for subwoofers), speed relative \
         to real time, mean and maximum visual activity, and the steps played by thumps, \
         ticks and stabs.\n\n",
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

    let scenarios: Vec<Scenario> = scenarios::all(names)
        .into_iter()
        .filter(|scenario| scenario.name.contains(filter))
        .collect();
    if scenarios.is_empty() {
        fail(&format!("No scenario's name contains {filter:?}"));
    }
    std::fs::create_dir_all(directory).unwrap();
    for scenario in scenarios {
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

        let seconds = scenario.frames.len() as f32 * FRAME_SECONDS;
        let speed = seconds / rendering.synth_time.as_secs_f32();
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
            "- **{}** ({seconds:.0} s): {}",
            scenario.name, scenario.description
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

/// Averages over all thumps of a minute of Strata at Form 0.8 (a thump on every beat), dry
/// at Void 0, Belize Ripple: the drone's gain from the signals (once per block), and the
/// rendered level of the chord band (300 to 1500 Hz) relative to its highest point, in
/// 20 ms steps after the thump. Heat and Flow are 0.5 unless given.
fn ducking(names: &[String], args: &[String]) {
    use nova_audio::dsp::Svf;
    let setting = |index: usize| {
        args.get(index).map_or(0.5, |value| {
            value
                .parse()
                .unwrap_or_else(|_| fail(&format!("Not a number: {value}")))
        })
    };
    let (heat, flow) = (setting(0), setting(1));
    let frames = scenarios::script(names, 60.0, |_, state| {
        state.set_palette("Belize Ripple");
        state.set_form(0.8);
        state.set_void(0.0);
        state.set_heat(heat);
        state.set_flow(flow);
    });
    let mut engine = Engine::new(RATE, 7, &Preset::default());
    let mut high = [Svf::new(300.0, 0.541, RATE), Svf::new(300.0, 1.307, RATE)];
    let mut low = [Svf::new(1500.0, 0.541, RATE), Svf::new(1500.0, 1.307, RATE)];

    const STEP: usize = 960; // 20 ms
    const STEPS: usize = 50;
    let (mut gains, mut powers, mut counts) = ([0.0_f64; STEPS], [0.0_f64; STEPS], [0_u32; STEPS]);
    let (mut since, mut thumps, mut window, mut gain_sum) = (usize::MAX, 0, 0.0_f64, 0.0_f64);
    let per_frame = (RATE * FRAME_SECONDS) as usize;
    for (frame, input) in frames.iter().enumerate() {
        play(&mut engine, input);
        for _ in 0..per_frame {
            let [left, right] = engine.next_frame();
            let count = engine.steps()[0];
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
            gain_sum += (1.0 - engine.signals().dip) as f64;
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
        "Heat {heat}, Flow {flow}, {:.0} BPM, {} thumps",
        engine.signals().tempo,
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

/// What every content module hands to the sound over 45 s with every control at 0.5: the
/// mean of each feature, the activity also as the level the sound hears it at, from 0 to 1,
/// with its onsets per minute, and the hue with the largest share.
fn features() {
    const SECONDS: f32 = 45.0;
    println!(
        "| Module | Activity mean / max | Heard | Onsets/min | Level | Balance | Width | Height | \
         Density | Strongest hue |\n| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |"
    );
    for (index, content) in nova_voxel::all_content_names().iter().enumerate() {
        let frames = scenarios::module(index, SECONDS);
        let mut activity = Activity::default();
        let (mut heard, mut onsets, mut peak) = (0.0, 0, 0.0_f32);
        for frame in &frames {
            let (level, onset) = activity.update(frame.light.activity, FRAME_SECONDS);
            heard += level;
            onsets += onset as u32;
            peak = peak.max(frame.light.activity);
        }
        let count = frames.len() as f32;
        let mean = |feature: fn(&Features) -> f32| {
            frames
                .iter()
                .map(|frame| feature(&frame.light))
                .sum::<f32>()
                / count
        };
        let mut hues = [0.0; HUES];
        for frame in &frames {
            for (sum, share) in hues.iter_mut().zip(frame.light.hues) {
                *sum += share / count;
            }
        }
        let strongest = (0..HUES)
            .max_by(|a, b| hues[*a].total_cmp(&hues[*b]))
            .unwrap();
        println!(
            "| {content} | {:.3} / {peak:.2} | {:.2} | {:.1} | {:.2} | {:.2} | {:.2} | {:.2} | {:.2} | {}° ({:.0}%) |",
            mean(|features| features.activity),
            heard / count,
            onsets as f32 * 60.0 / SECONDS,
            mean(|features| features.level),
            mean(|features| features.balance),
            mean(|features| features.width),
            mean(|features| features.height),
            mean(|features| features.density),
            strongest * 30,
            100.0 * hues[strongest],
        );
    }
}
