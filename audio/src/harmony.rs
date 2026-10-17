//! The tonal world of a palette: palettes as the sound reads them, in OKLCh, the harmony they
//! give, and the bass notes its progression moves through.

pub use nova_core::palettes::MAX_COLORS;

/// A palette color as the sound reads it: OKLCh lightness (0 to 1), chroma, and hue in
/// degrees.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Color {
    pub lightness: f32,
    pub chroma: f32,
    pub hue: f32,
}

/// The colors of a palette, which set the tonal world (see `harmony.rs`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    colors: [Color; MAX_COLORS],
    count: usize,
}

impl Palette {
    /// Takes up to `MAX_COLORS` colors.
    pub fn new(colors: &[Color]) -> Self {
        let count = colors.len().min(MAX_COLORS);
        let mut palette = Self {
            colors: [Color::default(); MAX_COLORS],
            count,
        };
        palette.colors[..count].copy_from_slice(&colors[..count]);
        palette
    }

    /// A palette of Nova's (see `nova_core::PALETTES`), as the sound reads it.
    pub fn from_core(palette: &nova_core::Palette) -> Self {
        let colors: Vec<Color> = palette
            .colors
            .iter()
            .map(|color| {
                let (lightness, chroma, hue) = color.oklch();
                Color {
                    lightness,
                    chroma,
                    hue,
                }
            })
            .collect();
        Self::new(&colors)
    }

    /// Every palette of Nova's, in the order of `nova_core::PALETTES`.
    pub fn all() -> Vec<Self> {
        nova_core::PALETTES.iter().map(Self::from_core).collect()
    }

    pub fn colors(&self) -> &[Color] {
        &self.colors[..self.count]
    }
}

/// Maximum number of tones in a harmony: the root and one per palette color.
pub const MAX_TONES: usize = 1 + MAX_COLORS;

/// Seconds over which a new harmony replaces the old one; the next palette change waits
/// until it is done.
pub const CROSSFADE_SECONDS: f32 = 5.0;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Tone {
    pub frequency: f32,
    pub level: f32,
    /// Hue in degrees of the color the tone stands for, if it has one: not the root, and
    /// not whites and grays.
    pub hue: Option<f32>,
}

/// The tonal world of a palette.
///
/// The chroma-weighted mean hue picks the root on the circle of fifths, between C1 and B1
/// (33 to 62 Hz). Every color adds a tone: its hue distance from the mean gives the
/// interval in steps on the circle of fifths (close hues give fifths and fourths, contrasting
/// hues thirds and sixths, all in just intonation), its lightness the octave, and its chroma
/// the level. Whites add an octave.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Harmony {
    pub(crate) tones: [Tone; MAX_TONES],
    pub(crate) count: usize,
}

/// Bass notes away from home, as ratios to the root, by preference. Only the palette's own
/// pitch classes are used, so the chord above stays consonant; palettes whose tones are all
/// octaves of the root use `BASS_FALLBACK`.
const BASS_PREFERENCE: [f32; 8] = [
    3.0 / 2.0,
    4.0 / 3.0,
    5.0 / 4.0,
    5.0 / 3.0,
    6.0 / 5.0,
    8.0 / 5.0,
    9.0 / 8.0,
    7.0 / 4.0,
];

/// Bass notes away from home for palettes without pitch classes of their own: the fourth
/// and the fifth, which give the chord above a suspended color.
const BASS_FALLBACK: [f32; 2] = [4.0 / 3.0, 3.0 / 2.0];

/// Just ratios by steps on the circle of fifths from the root, from −4 to 4.
const FIFTHS: [f32; 9] = [
    8.0 / 5.0,
    6.0 / 5.0,
    7.0 / 4.0,
    4.0 / 3.0,
    1.0,
    3.0 / 2.0,
    9.0 / 8.0,
    5.0 / 3.0,
    5.0 / 4.0,
];

impl Harmony {
    pub fn from_palette(palette: &Palette) -> Self {
        let (mut x, mut y) = (0.0_f32, 0.0_f32);
        for color in palette.colors() {
            x += color.chroma * color.hue.to_radians().cos();
            y += color.chroma * color.hue.to_radians().sin();
        }
        let mean = if x.hypot(y) > 1e-3 {
            y.atan2(x).to_degrees().rem_euclid(360.0)
        } else {
            0.0
        };
        // Neighbouring hues lie a fifth apart: 30° per step around the circle of fifths.
        let pitch_class = ((mean / 30.0).round() as i32 * 7).rem_euclid(12);
        let root = 440.0 * 2.0_f32.powf((24 + pitch_class - 69) as f32 / 12.0);
        let mut tones = [Tone::default(); MAX_TONES];
        tones[0] = Tone {
            frequency: root,
            level: 1.0,
            hue: None,
        };
        let mut count = 1;
        for color in palette.colors() {
            let ratio = if color.chroma < 0.03 {
                2.0
            } else {
                let distance = (color.hue - mean + 540.0).rem_euclid(360.0) - 180.0;
                FIFTHS[(((distance / 40.0).round() as i32).clamp(-4, 4) + 4) as usize]
            };
            let octave = (1 + (color.lightness * 3.2) as i32).clamp(1, 3);
            let mut frequency = root * ratio * 2.0_f32.powi(octave);
            // Colors that land on the same tone move up an octave rather than double it.
            for _ in 0..2 {
                if tones[..count]
                    .iter()
                    .any(|tone| (tone.frequency / frequency - 1.0).abs() < 0.01)
                {
                    frequency *= 2.0;
                }
            }
            tones[count] = Tone {
                frequency,
                level: (0.3 + 2.0 * color.chroma).min(0.9),
                hue: (color.chroma >= 0.03).then_some(color.hue),
            };
            count += 1;
        }
        Self { tones, count }
    }

    /// The root first, then one tone per palette color.
    pub fn tones(&self) -> &[Tone] {
        &self.tones[..self.count]
    }

    pub fn root(&self) -> f32 {
        self.tones[0].frequency
    }

    /// The bass at `step` of the progression: home (the root) at even steps, otherwise the
    /// palette's pitch classes in turn by `BASS_PREFERENCE`, as close to the root as
    /// possible and at least 30 Hz.
    pub fn bass(&self, step: usize) -> f32 {
        let root = self.root();
        let mut away = [0.0; BASS_PREFERENCE.len()];
        let mut count = 0;
        for &ratio in &BASS_PREFERENCE {
            let used = self.tones()[1..].iter().any(|tone| {
                let mut class = tone.frequency / root;
                while class >= 2.0 - 1e-3 {
                    class *= 0.5;
                }
                (class / ratio - 1.0).abs() < 1e-3
            });
            if used {
                away[count] = ratio;
                count += 1;
            }
        }
        if count == 0 {
            away[..BASS_FALLBACK.len()].copy_from_slice(&BASS_FALLBACK);
            count = BASS_FALLBACK.len();
        }
        if step.is_multiple_of(2) {
            return root;
        }
        let ratio = away[(step / 2) % count];
        let mut frequency = root
            * if ratio > std::f32::consts::SQRT_2 {
                0.5 * ratio
            } else {
                ratio
            };
        while frequency < 30.0 {
            frequency *= 2.0;
        }
        frequency
    }

    /// This harmony over another bass.
    pub fn with_bass(&self, frequency: f32) -> Self {
        let mut harmony = *self;
        harmony.tones[0].frequency = frequency;
        harmony
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::dsp::Rng;

    /// A palette of four colors.
    pub fn palette() -> Palette {
        let color = |lightness, chroma, hue| Color {
            lightness,
            chroma,
            hue,
        };
        Palette::new(&[
            color(0.45, 0.12, 200.0),
            color(0.7, 0.1, 160.0),
            color(0.85, 0.02, 0.0),
            color(0.6, 0.18, 30.0),
        ])
    }

    /// Palettes across hues, chromas, lightnesses and sizes, with whites and single colors.
    pub fn palettes() -> Vec<Palette> {
        let mut rng = Rng::new(5);
        let mut palettes = vec![
            Palette::new(&[Color {
                lightness: 0.9,
                chroma: 0.0,
                hue: 0.0,
            }]),
            Palette::new(&[Color {
                lightness: 0.6,
                chroma: 0.15,
                hue: 250.0,
            }]),
        ];
        for _ in 0..200 {
            let colors: Vec<Color> = (0..1 + rng.below(MAX_COLORS))
                .map(|_| Color {
                    lightness: rng.range(0.2, 1.0),
                    chroma: if rng.unit() < 0.15 {
                        0.01
                    } else {
                        rng.range(0.03, 0.3)
                    },
                    hue: rng.range(0.0, 360.0),
                })
                .collect();
            palettes.push(Palette::new(&colors));
        }
        palettes
    }

    #[test]
    fn harmonies_stay_in_range_and_in_just_intonation() {
        for palette in palettes() {
            let harmony = Harmony::from_palette(&palette);
            assert_eq!(harmony.count, 1 + palette.colors().len());
            assert!((32.0..62.0).contains(&harmony.root()), "{palette:?}");
            for tone in &harmony.tones()[1..] {
                assert!(tone.frequency > harmony.root() * 1.9, "{palette:?}");
                assert!(tone.frequency < harmony.root() * 128.0, "{palette:?}");
                assert!((0.3..=0.9).contains(&tone.level));
                let ratio = tone.frequency / harmony.root();
                let octaves = ratio / 2.0_f32.powi(ratio.log2().floor() as i32);
                assert!(
                    FIFTHS
                        .iter()
                        .chain([2.0].iter())
                        .any(|just| (just - octaves).abs() < 1e-4 || (octaves - 1.0).abs() < 1e-4),
                    "{palette:?}: {ratio}"
                );
            }
        }
    }

    #[test]
    fn bass_alternates_home_and_the_palettes_pitch_classes() {
        for palette in palettes() {
            let harmony = Harmony::from_palette(&palette);
            let root = harmony.root();
            for step in 0..12 {
                let bass = harmony.bass(step);
                if step % 2 == 0 {
                    assert_eq!(bass, root);
                    continue;
                }
                assert!((30.0..90.0).contains(&bass), "{palette:?}: {bass}");
                let mut class = bass / root;
                while class < 1.0 - 1e-3 {
                    class *= 2.0;
                }
                while class >= 2.0 - 1e-3 {
                    class *= 0.5;
                }
                assert!(
                    (class - 1.0).abs() > 1e-3,
                    "{palette:?}: the bass must move"
                );
                let classes: Vec<f32> = harmony.tones()[1..]
                    .iter()
                    .map(|tone| {
                        let mut tone_class = tone.frequency / root;
                        while tone_class >= 2.0 - 1e-3 {
                            tone_class *= 0.5;
                        }
                        tone_class
                    })
                    .collect();
                let own = classes
                    .iter()
                    .any(|tone_class| (tone_class / class - 1.0).abs() < 1e-3);
                let only_octaves = classes
                    .iter()
                    .all(|tone_class| (tone_class - 1.0).abs() < 1e-3);
                let fallback = BASS_FALLBACK
                    .iter()
                    .any(|ratio| (ratio / class - 1.0).abs() < 1e-3);
                assert!(own || (only_octaves && fallback), "{palette:?}: {class}");
            }
        }
    }

    #[test]
    fn every_palette_of_novas_gives_a_harmony_in_range() {
        for (palette, sound) in nova_core::PALETTES.iter().zip(Palette::all()) {
            assert_eq!(
                sound.colors().len(),
                palette.colors.len(),
                "{}",
                palette.name
            );
            let harmony = Harmony::from_palette(&sound);
            assert_eq!(harmony.tones().len(), 1 + palette.colors.len());
            assert!((32.0..62.0).contains(&harmony.root()), "{}", palette.name);
            for tone in &harmony.tones()[1..] {
                let ratio = tone.frequency / harmony.root();
                assert!((1.9..128.0).contains(&ratio), "{}: {ratio}", palette.name);
            }
        }
    }
}
