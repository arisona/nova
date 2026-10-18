//! Measurements for the sketches: loudness, peaks, spectrum, mono bass, and a spectrogram.
//! They catch technical problems; listening decides whether it sounds good.

use std::f32::consts::TAU;
use std::io::Write;
use std::path::Path;

/// Frequency bands for the energy distribution, in Hz.
pub const BANDS: [(f32, f32, &str); 6] = [
    (0.0, 40.0, "< 40"),
    (40.0, 120.0, "40–120"),
    (120.0, 500.0, "120–500"),
    (500.0, 2000.0, "500–2k"),
    (2000.0, 8000.0, "2k–8k"),
    (8000.0, f32::INFINITY, "> 8k"),
];

/// Octave band centres, in Hz.
pub const OCTAVES: [f32; 10] = [
    31.5, 63.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0,
];

pub struct Analysis {
    /// Integrated loudness (ITU-R BS.1770-4), in LUFS.
    pub loudness: f32,
    /// Loudness range (EBU Tech 3342, simplified), in LU.
    pub range: f32,
    /// The 95th percentile of the momentary loudness (400 ms), in LUFS: how loud the loud
    /// moments are.
    pub momentary: f32,
    /// The median and the 95th percentile of the level between 1 and 4 kHz over 50 ms, in
    /// dBFS: where the ear is most sensitive, and how far hits stand out from the gaps.
    pub presence: [f32; 2],
    /// Sample peak, in dBFS.
    pub peak: f32,
    /// Peak to RMS, in dB.
    pub crest: f32,
    /// Share of the energy per band, in percent.
    pub bands: [f32; BANDS.len()],
    /// Energy per octave band relative to the total, in dB: the spectral balance.
    pub octaves: [f32; OCTAVES.len()],
    /// Energy of the side signal relative to the mid signal below 80 Hz, where subwoofers
    /// sum the channels, in dB.
    pub side_to_mid_bass: f32,
    /// Largest DC offset of a channel.
    pub dc: f32,
}

fn decibels(power: f64) -> f32 {
    (10.0 * power.max(1e-20).log10()) as f32
}

/// In-place radix-2 FFT.
struct Fft {
    cos: Vec<f32>,
    sin: Vec<f32>,
    window: Vec<f32>,
}

impl Fft {
    fn new(size: usize) -> Self {
        assert!(size.is_power_of_two());
        Self {
            cos: (0..size / 2)
                .map(|k| (TAU * k as f32 / size as f32).cos())
                .collect(),
            sin: (0..size / 2)
                .map(|k| -(TAU * k as f32 / size as f32).sin())
                .collect(),
            window: (0..size)
                .map(|k| 0.5 - 0.5 * (TAU * k as f32 / size as f32).cos())
                .collect(),
        }
    }

    fn size(&self) -> usize {
        self.window.len()
    }

    /// Windowed power spectrum of `samples[start..start + size]`, scaled so a full-scale
    /// sine reads 1 (0 dB) in its bin.
    fn power(&self, samples: &[f32], start: usize, power: &mut [f32]) {
        let size = self.size();
        let mut re: Vec<f32> = (0..size)
            .map(|k| samples.get(start + k).copied().unwrap_or(0.0) * self.window[k])
            .collect();
        let mut im = vec![0.0; size];
        let bits = size.trailing_zeros();
        for k in 0..size {
            let j = k.reverse_bits() >> (usize::BITS - bits);
            if j > k {
                re.swap(k, j);
            }
        }
        let mut length = 2;
        while length <= size {
            let stride = size / length;
            for start in (0..size).step_by(length) {
                for k in 0..length / 2 {
                    let (c, s) = (self.cos[k * stride], self.sin[k * stride]);
                    let (a, b) = (start + k, start + k + length / 2);
                    let (tr, ti) = (re[b] * c - im[b] * s, re[b] * s + im[b] * c);
                    re[b] = re[a] - tr;
                    im[b] = im[a] - ti;
                    re[a] += tr;
                    im[a] += ti;
                }
            }
            length *= 2;
        }
        let scale = 16.0 / (size * size) as f32;
        for (bin, value) in power.iter_mut().enumerate() {
            *value = (re[bin] * re[bin] + im[bin] * im[bin]) * scale;
        }
    }
}

/// K-weighting at 48 kHz, as published in ITU-R BS.1770-4.
fn k_weighted(samples: &[f32]) -> Vec<f64> {
    let stages: [([f64; 3], [f64; 2]); 2] = [
        (
            [
                1.535_124_859_586_97,
                -2.691_696_189_406_38,
                1.198_392_810_852_85,
            ],
            [-1.690_659_293_182_41, 0.732_480_774_215_85],
        ),
        (
            [1.0, -2.0, 1.0],
            [-1.990_047_454_833_98, 0.990_072_250_366_21],
        ),
    ];
    let mut signal: Vec<f64> = samples.iter().map(|&sample| sample as f64).collect();
    for (b, a) in stages {
        let (mut x1, mut x2, mut y1, mut y2) = (0.0, 0.0, 0.0, 0.0);
        for value in &mut signal {
            let x = *value;
            let y = b[0] * x + b[1] * x1 + b[2] * x2 - a[0] * y1 - a[1] * y2;
            (x2, x1, y2, y1) = (x1, x, y1, y);
            *value = y;
        }
    }
    signal
}

/// A band-pass around 2 kHz with a Q of 0.7, about 1 to 4 kHz (RBJ cookbook).
fn presence_band(samples: &[f32], sample_rate: f32) -> Vec<f64> {
    let w = std::f64::consts::TAU * 2000.0 / sample_rate as f64;
    let alpha = w.sin() / (2.0 * 0.7);
    let a0 = 1.0 + alpha;
    let (b0, b2, a1, a2) = (
        alpha / a0,
        -alpha / a0,
        -2.0 * w.cos() / a0,
        (1.0 - alpha) / a0,
    );
    let (mut x1, mut x2, mut y1, mut y2) = (0.0, 0.0, 0.0, 0.0);
    samples
        .iter()
        .map(|&sample| {
            let x = sample as f64;
            let y = b0 * x + b2 * x2 - a1 * y1 - a2 * y2;
            (x2, x1, y2, y1) = (x1, x, y1, y);
            y
        })
        .collect()
}

/// The value at `fraction` of sorted `values`, in dB.
fn percentile_of(values: &[f64], fraction: f64) -> f32 {
    values
        .get(((values.len() as f64 - 1.0) * fraction).round() as usize)
        .map_or(-200.0, |&power| decibels(power))
}

/// Mean square of both K-weighted channels over windows of `window` samples, every `hop`.
fn window_powers(left: &[f64], right: &[f64], window: usize, hop: usize) -> Vec<f64> {
    let mut powers = Vec::new();
    let mut start = 0;
    while start + window <= left.len() {
        let sum: f64 = (start..start + window)
            .map(|k| left[k] * left[k] + right[k] * right[k])
            .sum();
        powers.push(sum / window as f64);
        start += hop;
    }
    powers
}

fn gated_loudness(powers: &[f64], relative: f32) -> (f64, Vec<f64>) {
    let loudness = |power: f64| -0.691 + decibels(power);
    let absolute: Vec<f64> = powers
        .iter()
        .copied()
        .filter(|&power| loudness(power) > -70.0)
        .collect();
    if absolute.is_empty() {
        return (0.0, absolute);
    }
    let threshold = loudness(absolute.iter().sum::<f64>() / absolute.len() as f64) + relative;
    let gated: Vec<f64> = absolute
        .into_iter()
        .filter(|&power| loudness(power) > threshold)
        .collect();
    let mean = gated.iter().sum::<f64>() / gated.len().max(1) as f64;
    (mean, gated)
}

pub fn analyse(left: &[f32], right: &[f32], sample_rate: f32) -> Analysis {
    assert_eq!(
        sample_rate, 48_000.0,
        "the K-weighting coefficients are for 48 kHz"
    );
    let rate = sample_rate as usize;
    let (k_left, k_right) = (k_weighted(left), k_weighted(right));
    let momentary = window_powers(&k_left, &k_right, rate * 4 / 10, rate / 10);
    let (integrated, _) = gated_loudness(&momentary, -10.0);
    let mut sorted = momentary.clone();
    sorted.sort_by(f64::total_cmp);
    let momentary_loud = -0.691 + percentile_of(&sorted, 0.95);
    let (band_left, band_right) = (
        presence_band(left, sample_rate),
        presence_band(right, sample_rate),
    );
    // Both channels, as for the loudness: a full-scale sine in one reads −3 dB.
    let mut presence = window_powers(&band_left, &band_right, rate / 20, rate / 40);
    presence.sort_by(f64::total_cmp);
    let short_term = window_powers(&k_left, &k_right, rate * 3, rate);
    let (_, mut gated) = gated_loudness(&short_term, -20.0);
    gated.sort_by(f64::total_cmp);
    let percentile = |fraction: f64| {
        gated
            .get(((gated.len() as f64 - 1.0) * fraction).round() as usize)
            .map_or(0.0, |&power| decibels(power))
    };

    let peak = left
        .iter()
        .chain(right)
        .fold(0.0_f32, |peak, sample| peak.max(sample.abs()));
    let mean_square = left
        .iter()
        .chain(right)
        .map(|&sample| (sample as f64).powi(2))
        .sum::<f64>()
        / (2 * left.len()).max(1) as f64;
    let dc = [left, right]
        .map(|channel| {
            (channel.iter().map(|&sample| sample as f64).sum::<f64>() / channel.len().max(1) as f64)
                .abs() as f32
        })
        .into_iter()
        .fold(0.0, f32::max);

    let fft = Fft::new(16_384);
    let size = fft.size();
    let bin_width = sample_rate / size as f32;
    let mid: Vec<f32> = left.iter().zip(right).map(|(l, r)| 0.5 * (l + r)).collect();
    let side: Vec<f32> = left.iter().zip(right).map(|(l, r)| 0.5 * (l - r)).collect();
    let mut totals = [
        vec![0.0_f64; size / 2],
        vec![0.0; size / 2],
        vec![0.0; size / 2],
    ];
    let mut power = vec![0.0; size / 2];
    let mut start = 0;
    while start + size <= left.len() {
        for (channel, total) in [left, right, &mid, &side].iter().zip([0, 0, 1, 2]) {
            fft.power(channel, start, &mut power);
            for (sum, &value) in totals[total].iter_mut().zip(&power) {
                *sum += value as f64;
            }
        }
        start += size / 2;
    }
    let all: f64 = totals[0].iter().skip(1).sum();
    let mut bands = [0.0; BANDS.len()];
    for (band, &(low, high, _)) in bands.iter_mut().zip(&BANDS) {
        let energy: f64 = (1..size / 2)
            .filter(|&bin| (low..high).contains(&(bin as f32 * bin_width)))
            .map(|bin| totals[0][bin])
            .sum();
        *band = (100.0 * energy / all.max(1e-30)) as f32;
    }
    let mut octaves = [0.0; OCTAVES.len()];
    for (octave, &centre) in octaves.iter_mut().zip(&OCTAVES) {
        let (low, high) = (
            centre / std::f32::consts::SQRT_2,
            centre * std::f32::consts::SQRT_2,
        );
        let energy: f64 = (1..size / 2)
            .filter(|&bin| (low..high).contains(&(bin as f32 * bin_width)))
            .map(|bin| totals[0][bin])
            .sum();
        *octave = decibels(energy / all.max(1e-30));
    }
    let bass = 1..=(80.0 / bin_width) as usize;
    let side_bass: f64 = bass.clone().map(|bin| totals[2][bin]).sum();
    let mid_bass: f64 = bass.map(|bin| totals[1][bin]).sum();

    Analysis {
        loudness: (-0.691 + decibels(integrated) as f64) as f32,
        range: percentile(0.95) - percentile(0.10),
        momentary: momentary_loud,
        presence: [
            percentile_of(&presence, 0.5),
            percentile_of(&presence, 0.95),
        ],
        peak: decibels((peak as f64).powi(2)),
        crest: decibels((peak as f64).powi(2) / mean_square.max(1e-30)),
        bands,
        octaves,
        side_to_mid_bass: decibels(side_bass / mid_bass.max(1e-30)),
        dc,
    }
}

fn color(value: f32) -> [u8; 3] {
    const STOPS: [[f32; 3]; 5] = [
        [0.0, 0.0, 0.0],
        [20.0, 24.0, 110.0],
        [150.0, 30.0, 140.0],
        [245.0, 130.0, 30.0],
        [255.0, 250.0, 205.0],
    ];
    let position = value.clamp(0.0, 1.0) * (STOPS.len() - 1) as f32;
    let index = (position as usize).min(STOPS.len() - 2);
    let fraction = position - index as f32;
    std::array::from_fn(|channel| {
        (STOPS[index][channel] + (STOPS[index + 1][channel] - STOPS[index][channel]) * fraction)
            as u8
    })
}

/// Writes a spectrogram of the mid signal as a PPM: time to the right (8 columns per
/// second, a tick every 10 s), 20 Hz to 20 kHz upwards on a logarithmic axis, with lines
/// at 50, 100, 500, 1k, 5k and 10k Hz, and −110 to −10 dB from black to pale yellow.
pub fn spectrogram(
    left: &[f32],
    right: &[f32],
    sample_rate: f32,
    path: &Path,
) -> std::io::Result<()> {
    const HEIGHT: usize = 400;
    const COLUMNS_PER_SECOND: usize = 8;
    let fft = Fft::new(16_384);
    let size = fft.size();
    let bin_width = sample_rate / size as f32;
    let hop = sample_rate as usize / COLUMNS_PER_SECOND;
    let mid: Vec<f32> = left.iter().zip(right).map(|(l, r)| 0.5 * (l + r)).collect();
    let width = (mid.len() / hop).max(1);
    let (low, high) = (20.0_f32, 20_000.0_f32);
    let row_frequency = |row: f32| low * (high / low).powf(row / HEIGHT as f32);
    let mut pixels = vec![0u8; width * HEIGHT * 3];
    let mut power = vec![0.0; size / 2];
    for column in 0..width {
        let center = column * hop + hop / 2;
        fft.power(&mid, center.saturating_sub(size / 2), &mut power);
        for row in 0..HEIGHT {
            let from = row_frequency(row as f32) / bin_width;
            let to = row_frequency(row as f32 + 1.0) / bin_width;
            let first = from.floor() as usize;
            let last = (to.ceil() as usize).max(first + 1).min(size / 2);
            let value = power[first.min(size / 2 - 1)..last]
                .iter()
                .fold(0.0_f32, |max, &value| max.max(value));
            let level = (decibels(value as f64) + 110.0) / 100.0;
            let y = HEIGHT - 1 - row;
            pixels[(y * width + column) * 3..][..3].copy_from_slice(&color(level));
        }
    }
    for frequency in [50.0_f32, 100.0, 500.0, 1000.0, 5000.0, 10_000.0] {
        let row = ((frequency / low).ln() / (high / low).ln() * HEIGHT as f32) as usize;
        let y = HEIGHT - 1 - row.min(HEIGHT - 1);
        for column in (0..width).step_by(3) {
            pixels[(y * width + column) * 3..][..3].copy_from_slice(&[90, 90, 90]);
        }
    }
    for column in (0..width).step_by(10 * COLUMNS_PER_SECOND) {
        for y in HEIGHT - 8..HEIGHT {
            pixels[(y * width + column) * 3..][..3].copy_from_slice(&[200, 200, 200]);
        }
    }
    let mut file = std::io::BufWriter::new(std::fs::File::create(path)?);
    write!(file, "P6\n{width} {HEIGHT}\n255\n")?;
    file.write_all(&pixels)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analysis_measures_a_sine_correctly() {
        let rate = 48_000.0;
        let sine: Vec<f32> = (0..48_000 * 4)
            .map(|k| 0.5 * (TAU * 1000.0 * k as f32 / rate).sin())
            .collect();
        let analysis = analyse(&sine, &sine, rate);
        // A full-scale 1 kHz sine in one channel reads −3.01 LUFS; at −6.02 dBFS in both
        // channels, −6.02 LUFS.
        assert!(
            (analysis.loudness + 6.02).abs() < 0.1,
            "{}",
            analysis.loudness
        );
        assert!((analysis.peak + 6.02).abs() < 0.05);
        assert!((analysis.crest - 3.01).abs() < 0.05);
        assert!(analysis.bands[3] > 99.0);
        assert!(analysis.side_to_mid_bass < -100.0);
    }
}
