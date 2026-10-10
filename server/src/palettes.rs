//! Curated palettes, compiled into the binary.
//!
//! Colors are specified for light, as the display shows them, and converted to sRGB:
//! - OKLCh (lightness 0–1, chroma, hue in degrees) for most colors, so lightness, purity
//!   and hue can be tuned one at a time on the display.
//! - Color temperature in kelvin for whites, at full brightness.
//! - sRGB hex for Nueva York Buzz, Texas Sun Burn and Uluwatu Wipeout, which keep the
//!   Pantone colors closest to colors picked from reference images.
//!
//! A color that started from another, typically a Pantone color, names it as its source.
//! Judged on the display: warm dark colors vanish while bluish ones survive as dim color;
//! blue is strong, so a palette has at most one strong blue; pastels read as white; and
//! the green LEDs are so bright that greens need a lower lightness and little red or blue
//! to look lush rather than pale.
//!
//! Palettes are listed alphabetically. Colors run in the order Heat as a picker slides
//! through them (see the README), and the blends between neighbours are what the
//! wrapping modules show.

use glam::Vec3;
use palette::convert::FromColorUnclamped;
use palette::{LinSrgb, Oklch, Srgb, Xyz};

/// Palettes have between 1 and this many colors.
pub const MAX_COLORS: usize = 6;

/// How a palette color is specified.
#[derive(Clone, Copy, Debug)]
pub enum Spec {
    /// Lightness 0–1, chroma, hue in degrees; must lie within sRGB (checked by a test).
    Oklch(f32, f32, f32),
    /// A white of this color temperature, at full brightness.
    Kelvin(f32),
    /// sRGB, for colors taken over as they are.
    Hex(u32),
}

pub struct PaletteColor {
    pub name: &'static str,
    pub spec: Spec,
    /// The color this one started from, or empty.
    pub source: &'static str,
}

impl PaletteColor {
    const fn new(name: &'static str, spec: Spec) -> Self {
        Self {
            name,
            spec,
            source: "",
        }
    }

    const fn after(name: &'static str, spec: Spec, source: &'static str) -> Self {
        Self { name, spec, source }
    }

    /// Linear sRGB, unclamped, so a test can check the gamut.
    fn linear(&self) -> Vec3 {
        match self.spec {
            Spec::Oklch(l, c, h) => {
                let rgb = LinSrgb::from_color_unclamped(Oklch::new(l, c, h));
                Vec3::new(rgb.red, rgb.green, rgb.blue)
            }
            Spec::Kelvin(t) => kelvin(t),
            Spec::Hex(hex) => {
                let rgb = Srgb::new((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
                    .into_format::<f32>()
                    .into_linear();
                Vec3::new(rgb.red, rgb.green, rgb.blue)
            }
        }
    }

    /// sRGB in [0, 1].
    pub fn color(&self) -> Vec3 {
        let linear = self.linear().clamp(Vec3::ZERO, Vec3::ONE);
        let rgb = Srgb::from_linear(LinSrgb::new(linear.x, linear.y, linear.z));
        Vec3::new(rgb.red, rgb.green, rgb.blue)
    }

    /// CSS-style hex, e.g. `#F6D155`.
    pub fn hex(&self) -> String {
        let [r, g, b] = (self.color() * 255.0).round().to_array().map(|c| c as u8);
        format!("#{r:02X}{g:02X}{b:02X}")
    }

    /// The specification as written, e.g. `L 0.48 · C 0.150 · h 145` or `3000 K`.
    pub fn describe(&self) -> String {
        match self.spec {
            Spec::Oklch(l, c, h) => format!("L {l:.2} · C {c:.3} · h {h:.0}"),
            Spec::Kelvin(t) => format!("{t:.0} K"),
            Spec::Hex(_) => "sRGB".into(),
        }
    }
}

/// Linear sRGB of a blackbody at `t` kelvin (1667 to 25000), scaled to full brightness.
/// Its chromaticity follows Kim et al. (2002).
fn kelvin(t: f32) -> Vec3 {
    let x = if t <= 4000.0 {
        -0.266_123_9e9 / t.powi(3) - 0.234_358_9e6 / t.powi(2) + 0.877_695_6e3 / t + 0.179_910
    } else {
        -3.025_846_9e9 / t.powi(3) + 2.107_037_9e6 / t.powi(2) + 0.222_634_7e3 / t + 0.240_390
    };
    let y = if t <= 2222.0 {
        -1.106_381_4 * x.powi(3) - 1.348_110_2 * x.powi(2) + 2.185_558_3 * x - 0.202_196_83
    } else if t <= 4000.0 {
        -0.954_947_6 * x.powi(3) - 1.374_185_9 * x.powi(2) + 2.091_370_2 * x - 0.167_488_67
    } else {
        3.081_758 * x.powi(3) - 5.873_386_7 * x.powi(2) + 3.751_13 * x - 0.370_014_83
    };
    let rgb = LinSrgb::from_color_unclamped(Xyz::new(x / y, 1.0, (1.0 - x - y) / y));
    let rgb = Vec3::new(rgb.red, rgb.green, rgb.blue).max(Vec3::ZERO);
    rgb / rgb.max_element()
}

pub struct Palette {
    pub name: &'static str,
    pub colors: &'static [PaletteColor],
}

// Antarctica Freeze
const PACK_ICE: PaletteColor = PaletteColor::new("Pack Ice", Spec::Kelvin(9000.0));
const GLACIER: PaletteColor = PaletteColor::after(
    "Glacier",
    Spec::Oklch(0.70, 0.120, 217.0),
    "Pantone Blue Atoll",
);
const POLAR_SEA: PaletteColor = PaletteColor::after(
    "Polar Sea",
    Spec::Oklch(0.44, 0.130, 253.0),
    "Pantone Princess Blue",
);

// Avignon Breeze
const LAVENDER: PaletteColor = PaletteColor::new("Lavender", Spec::Oklch(0.64, 0.120, 320.0));
const OLIVE_GROVE: PaletteColor = PaletteColor::new("Olive Grove", Spec::Oklch(0.60, 0.100, 128.0));
const SUNFLOWER: PaletteColor = PaletteColor::after(
    "Sunflower",
    Spec::Oklch(0.85, 0.170, 86.0),
    "Pantone Lemon Chrome",
);

// Belize Ripple
const SAND: PaletteColor = PaletteColor::new("Sand", Spec::Oklch(0.93, 0.035, 85.0));
const SHALLOWS: PaletteColor = PaletteColor::new("Shallows", Spec::Oklch(0.82, 0.100, 190.0));
const REEF: PaletteColor = PaletteColor::after(
    "Reef",
    Spec::Oklch(0.70, 0.125, 178.0),
    "Pantone Aqua Green",
);
const BLUE_HOLE: PaletteColor = PaletteColor::after(
    "Blue Hole",
    Spec::Oklch(0.47, 0.080, 210.0),
    "Pantone Fanfare",
);

// Kiruna Glow
const SNOWFIELD: PaletteColor = PaletteColor::new("Snowfield", Spec::Kelvin(7000.0));
const AURORA: PaletteColor = PaletteColor::new("Aurora", Spec::Oklch(0.60, 0.170, 148.0));
const CORONA: PaletteColor = PaletteColor::new("Corona", Spec::Oklch(0.55, 0.170, 332.0));

// Lampung Blink
const NIGHT_CANOPY: PaletteColor =
    PaletteColor::new("Night Canopy", Spec::Oklch(0.40, 0.120, 145.0));
const FIREFLY: PaletteColor = PaletteColor::after(
    "Firefly",
    Spec::Oklch(0.78, 0.200, 136.0),
    "Pantone Acid Lime",
);

// Nueva York Buzz
const BLAZING_YELLOW: PaletteColor =
    PaletteColor::after("Blazing Yellow", Spec::Hex(0xFEE715), "Pantone 12-0643 TCX");
const DESERT_FLOWER: PaletteColor =
    PaletteColor::after("Desert Flower", Spec::Hex(0xFF9687), "Pantone 15-1435 TCX");
const BACHELOR_BUTTON: PaletteColor = PaletteColor::after(
    "Bachelor Button",
    Spec::Hex(0x4ABBD5),
    "Pantone 14-4522 TCX",
);
const CORNFLOWER_BLUE: PaletteColor = PaletteColor::after(
    "Cornflower Blue",
    Spec::Hex(0x7391C8),
    "Pantone 16-4031 TCX",
);
const ORCHID_BLOOM: PaletteColor =
    PaletteColor::after("Orchid Bloom", Spec::Hex(0xC5AECF), "Pantone 14-3612 TCX");

// Sahara Drift
const DUNE: PaletteColor =
    PaletteColor::after("Dune", Spec::Oklch(0.84, 0.150, 80.0), "Pantone Daffodil");
const SAFFRON: PaletteColor =
    PaletteColor::after("Saffron", Spec::Oklch(0.79, 0.170, 71.0), "Pantone Saffron");
const DUSK: PaletteColor = PaletteColor::after(
    "Dusk",
    Spec::Oklch(0.60, 0.160, 50.0),
    "Pantone Autumn Maple",
);

// Shenzhen Lantern Flicker
const PAPER_LANTERN: PaletteColor = PaletteColor::new("Paper Lantern", Spec::Kelvin(3000.0));
const AMBER: PaletteColor = PaletteColor::after(
    "Amber",
    Spec::Oklch(0.78, 0.160, 66.0),
    "Pantone Radiant Yellow",
);
const VERMILION: PaletteColor = PaletteColor::after(
    "Vermilion",
    Spec::Oklch(0.62, 0.210, 31.0),
    "Pantone Cherry Tomato",
);

// Texas Sun Burn
const BLUEJAY: PaletteColor =
    PaletteColor::after("Bluejay", Spec::Hex(0x157EA0), "Pantone 17-4427 TCX");
const GOLDEN_ROD: PaletteColor =
    PaletteColor::after("Golden Rod", Spec::Hex(0xE2A829), "Pantone 14-0951 TCX");
const ORANGE_PEPPER: PaletteColor =
    PaletteColor::after("Orange Pepper", Spec::Hex(0xDF7500), "Pantone 16-1164 TCX");
const VALIANT_POPPY: PaletteColor =
    PaletteColor::after("Valiant Poppy", Spec::Hex(0xBC322C), "Pantone 18-1549 TCX");

// Tikal Humm
const CANOPY: PaletteColor = PaletteColor::new("Canopy", Spec::Oklch(0.48, 0.150, 145.0));
const FERN: PaletteColor = PaletteColor::new("Fern", Spec::Oklch(0.56, 0.170, 144.0));
const LEAF: PaletteColor = PaletteColor::new("Leaf", Spec::Oklch(0.64, 0.170, 134.0));
const SUNFLECK: PaletteColor = PaletteColor::after(
    "Sunfleck",
    Spec::Oklch(0.80, 0.160, 93.0),
    "Pantone Sulphur",
);

// Tokyo Lights Flash
const NIGHT: PaletteColor = PaletteColor::new("Night", Spec::Oklch(0.30, 0.060, 275.0));
const NEON_CYAN: PaletteColor = PaletteColor::new("Neon Cyan", Spec::Oklch(0.72, 0.120, 208.0));
const NEON_MAGENTA: PaletteColor =
    PaletteColor::new("Neon Magenta", Spec::Oklch(0.62, 0.250, 340.0));
const NEON_YELLOW: PaletteColor = PaletteColor::after(
    "Neon Yellow",
    Spec::Oklch(0.90, 0.180, 100.0),
    "Pantone Vibrant Yellow",
);

// Uluwatu Wipeout
const BUFF_YELLOW: PaletteColor =
    PaletteColor::after("Buff Yellow", Spec::Hex(0xF1BF70), "Pantone 14-0847 TCX");
const FLAMINGO: PaletteColor =
    PaletteColor::after("Flamingo", Spec::Hex(0xDF7253), "Pantone 16-1450 TCX");
const LICHEN_BLUE: PaletteColor =
    PaletteColor::after("Lichen Blue", Spec::Hex(0x5D89B3), "Pantone 17-4032 TCX");
const CLEMATIS_BLUE: PaletteColor =
    PaletteColor::after("Clematis Blue", Spec::Hex(0x363B7C), "Pantone 19-3951 TCX");
const BLUE_DEPTHS: PaletteColor =
    PaletteColor::after("Blue Depths", Spec::Hex(0x263056), "Pantone 19-3940 TCX");

// Palette names are what settings and the API refer to: keep them unique.
pub const PALETTES: &[Palette] = &[
    Palette {
        name: "Antarctica Freeze",
        colors: &[PACK_ICE, GLACIER, POLAR_SEA],
    },
    Palette {
        name: "Avignon Breeze",
        colors: &[LAVENDER, OLIVE_GROVE, SUNFLOWER],
    },
    Palette {
        name: "Belize Ripple",
        colors: &[SAND, SHALLOWS, REEF, BLUE_HOLE],
    },
    Palette {
        name: "Kiruna Glow",
        colors: &[SNOWFIELD, AURORA, CORONA],
    },
    Palette {
        name: "Lampung Blink",
        colors: &[NIGHT_CANOPY, FIREFLY],
    },
    Palette {
        name: "Nueva York Buzz",
        colors: &[
            BLAZING_YELLOW,
            DESERT_FLOWER,
            BACHELOR_BUTTON,
            CORNFLOWER_BLUE,
            ORCHID_BLOOM,
        ],
    },
    Palette {
        name: "Sahara Drift",
        colors: &[DUNE, SAFFRON, DUSK],
    },
    Palette {
        name: "Shenzhen Lantern Flicker",
        colors: &[PAPER_LANTERN, AMBER, VERMILION],
    },
    Palette {
        name: "Texas Sun Burn",
        colors: &[BLUEJAY, GOLDEN_ROD, ORANGE_PEPPER, VALIANT_POPPY],
    },
    Palette {
        name: "Tikal Humm",
        colors: &[CANOPY, FERN, LEAF, SUNFLECK],
    },
    Palette {
        name: "Tokyo Lights Flash",
        colors: &[NIGHT, NEON_CYAN, NEON_MAGENTA, NEON_YELLOW],
    },
    Palette {
        name: "Uluwatu Wipeout",
        colors: &[
            BUFF_YELLOW,
            FLAMINGO,
            LICHEN_BLUE,
            CLEMATIS_BLUE,
            BLUE_DEPTHS,
        ],
    },
];

// Checked at compile time.
const _: () = {
    let mut index = 0;
    while index < PALETTES.len() {
        let count = PALETTES[index].colors.len();
        assert!(
            count >= 1 && count <= MAX_COLORS,
            "palettes need 1 to MAX_COLORS colors"
        );
        index += 1;
    }
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::common::PaletteMix;

    #[test]
    fn oklch_colors_lie_within_srgb() {
        for palette in PALETTES {
            for color in palette.colors {
                if let Spec::Oklch(..) = color.spec {
                    let linear = color.linear();
                    assert!(
                        linear.min_element() > -0.002 && linear.max_element() < 1.002,
                        "{} in {} lies outside sRGB: {linear}",
                        color.name,
                        palette.name
                    );
                }
            }
        }
    }

    #[test]
    fn kelvin_whites_warm_up_with_falling_temperature() {
        let daylight = kelvin(6500.0);
        assert!(
            daylight.min_element() > 0.9,
            "6500 K is near neutral: {daylight}"
        );
        let lantern = kelvin(3000.0);
        assert_eq!(lantern.x, 1.0);
        assert!(lantern.z < 0.5 * lantern.y, "3000 K is warm: {lantern}");
        assert!(kelvin(9000.0).z > kelvin(9000.0).x, "9000 K is cool");
    }

    fn escape(text: &str) -> String {
        text.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    }

    /// Documentation, not a check: writes `doc/palettes.svg`, an overview of all palettes
    /// in order, with each color's name, specification, hex value and source. Below the colors, a
    /// strip shows the palette as light: voxels on black, blended as the content modules
    /// blend them, with each pure color under its chip and the wrap from the last color back
    /// to the first split across both ends.
    #[test]
    #[ignore = "writes doc/palettes.svg"]
    fn palettes_overview_svg() {
        const MARGIN: usize = 48;
        const NAME_WIDTH: usize = 290;
        const CHIP: (usize, usize) = (150, 92); // color block
        const LABEL: usize = 82;
        const GAP: usize = 14;
        const STRIP: usize = 34; // height of the light strip
        const VOXELS_PER_COLOR: usize = 8;
        // Gamma of the display being previewed. Monitors show sRGB values with a gamma of
        // about 2.2, so at 2.2 the strip shows the values as they are; at 1, it shows the
        // uncalibrated LEDs, which drive values linearly and look lighter.
        const PREVIEW_GAMMA: f32 = 2.2;
        const ROW: usize = CHIP.1 + LABEL + GAP + STRIP + 28;
        const HEADER: usize = 126;
        let columns = PALETTES.iter().map(|p| p.colors.len()).max().unwrap();
        let width = 2 * MARGIN + NAME_WIDTH + columns * (CHIP.0 + GAP) - GAP;
        let height = HEADER + PALETTES.len() * ROW + MARGIN - 28;

        let mut svg = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}" font-family="-apple-system, 'Helvetica Neue', Arial, sans-serif">
<defs><radialGradient id="voxel"><stop offset="0.45" stop-opacity="0"/><stop offset="1" stop-opacity="0.6"/></radialGradient></defs>
<rect width="100%" height="100%" fill="#F3F2EE"/>
<text x="{MARGIN}" y="{y1}" font-size="30" font-weight="700" fill="#1C1B19">Nova palettes</text>
<text x="{MARGIN}" y="{y2}" font-size="15" fill="#6D6A62">{count} palettes in alphabetical order · colors in OKLCh, kelvin or sRGB · generated from server/src/palettes.rs</text>
<text x="{MARGIN}" y="{y3}" font-size="15" fill="#6D6A62">Below each palette: its colors as light on black, blended as on the display, previewed at gamma {PREVIEW_GAMMA}</text>
"##,
            y1 = MARGIN + 8,
            y2 = MARGIN + 36,
            y3 = MARGIN + 58,
            count = PALETTES.len(),
        );
        for (row, palette) in PALETTES.iter().enumerate() {
            let top = HEADER + row * ROW;
            svg += &format!(
                r##"<text x="{MARGIN}" y="{y1}" font-size="13" font-weight="600" fill="#9A968C">{number:02}</text>
<text x="{MARGIN}" y="{y2}" font-size="21" font-weight="700" fill="#1C1B19">{name}</text>
"##,
                y1 = top + 34,
                y2 = top + 60,
                number = row + 1,
                name = escape(palette.name),
            );
            for (column, color) in palette.colors.iter().enumerate() {
                let x = MARGIN + NAME_WIDTH + column * (CHIP.0 + GAP);
                let hex = color.hex();
                svg += &format!(
                    r##"<g transform="translate({x},{top})">
<rect width="{w}" height="{h}" rx="6" fill="#FFFFFF" stroke="#DCD9D0"/>
<path d="M0 6 a6 6 0 0 1 6 -6 h{inner} a6 6 0 0 1 6 6 v{chip} h-{w} z" fill="{hex}"/>
<text x="10" y="{t1}" font-size="13" font-weight="700" fill="#1C1B19">{name}</text>
<text x="10" y="{t2}" font-size="12" fill="#55524B">{spec}</text>
<text x="10" y="{t3}" font-size="12" fill="#55524B" font-family="Menlo, Consolas, monospace">{hex}</text>
<text x="10" y="{t4}" font-size="11" fill="#8A867C">{source}</text>
</g>
"##,
                    w = CHIP.0,
                    h = CHIP.1 + LABEL,
                    inner = CHIP.0 - 12,
                    chip = CHIP.1 - 6,
                    t1 = CHIP.1 + 20,
                    t2 = CHIP.1 + 37,
                    t3 = CHIP.1 + 54,
                    t4 = CHIP.1 + 71,
                    name = escape(color.name),
                    spec = escape(&color.describe()),
                    source = escape(color.source),
                );
            }

            let mut mix = PaletteMix::default();
            mix.select(row, "Overview");
            let left = MARGIN + NAME_WIDTH;
            let strip_top = top + CHIP.1 + LABEL + GAP;
            let spacing = (CHIP.0 + GAP) as f32 / VOXELS_PER_COLOR as f32;
            let radius = 0.42 * spacing;
            svg += &format!(
                r##"<rect x="{x}" y="{strip_top}" width="{w}" height="{STRIP}" rx="6" fill="#000000"/>
"##,
                x = left - GAP / 2,
                w = mix.len() * (CHIP.0 + GAP),
            );
            for voxel in 0..mix.len() * VOXELS_PER_COLOR {
                // Palette position, from half a color before the first to half after the last.
                let q = (voxel as f32 + 0.5) / VOXELS_PER_COLOR as f32 - 0.5;
                let light = mix.wrapped(q).powf(PREVIEW_GAMMA / 2.2) * 255.0;
                svg += &format!(
                    r##"<circle cx="{cx:.1}" cy="{cy}" r="{radius:.1}" fill="#{r:02X}{g:02X}{b:02X}"/><circle cx="{cx:.1}" cy="{cy}" r="{radius:.1}" fill="url(#voxel)"/>
"##,
                    cx = (left - GAP / 2) as f32 + (q + 0.5) * (CHIP.0 + GAP) as f32,
                    cy = strip_top + STRIP / 2,
                    r = light.x.round() as u8,
                    g = light.y.round() as u8,
                    b = light.z.round() as u8,
                );
            }
        }
        svg += "</svg>\n";
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../doc/palettes.svg");
        std::fs::write(path, svg).unwrap();
        println!("Palette overview: {path}");
    }
}
