//! Curated Pantone palettes, compiled into the binary.
//!
//! Sources:
//! - Pantone Connect palette generator exports. Colors listed on the exported color-code
//!   sheet use the values published there.
//! - Colors marked `dataset` use the values of a public Pantone FHI dataset
//!   (github.com/Margaret2/pantone-colors). Where it overlaps with Pantone Connect, it
//!   matches within ΔE 0.5 (Oklab distance × 100).
//! - Colors marked `sampled` are not in that dataset; they were measured from the
//!   Connect screenshots after converting Display P3 to sRGB (accurate to ±1 per channel).
//! - Island Vibes, Nueva York, Texas Sun and Uluwatu Wipeout are based on reference
//!   images: they use the Pantone colors from the dataset closest to colors picked from
//!   each image. Colors marked `out of gamut`
//!   have no close Pantone equivalent; the nearest one is used.
//!
//! Palettes are listed by the hue of their most saturated color, so moving through them
//! goes around the color wheel. Colors keep the order of the source palette.

use glam::Vec3;

/// Palettes have between 1 and this many colors.
pub const MAX_COLORS: usize = 6;

pub struct PantoneColor {
    pub name: &'static str,
    pub code: &'static str,
    pub rgb: [u8; 3],
}

impl PantoneColor {
    const fn new(name: &'static str, code: &'static str, hex: u32) -> Self {
        Self {
            name,
            code,
            rgb: [(hex >> 16) as u8, (hex >> 8) as u8, hex as u8],
        }
    }

    /// CSS-style hex, e.g. `#F6D155`.
    pub fn hex(&self) -> String {
        let [r, g, b] = self.rgb;
        format!("#{r:02X}{g:02X}{b:02X}")
    }

    /// sRGB in [0, 1].
    pub fn color(&self) -> Vec3 {
        Vec3::new(self.rgb[0] as f32, self.rgb[1] as f32, self.rgb[2] as f32) / 255.0
    }
}

pub struct Palette {
    pub name: &'static str,
    pub colors: &'static [PantoneColor],
}

// Brilliant Abyss
const PRIMROSE_YELLOW: PantoneColor = PantoneColor::new("Primrose Yellow", "13-0755 TCX", 0xF6D155);
const OUTER_SPACE: PantoneColor = PantoneColor::new("Outer Space", "19-4009 TCX", 0x2F3441);
const AQUA_ESQUE: PantoneColor = PantoneColor::new("Aqua-esque", "13-5411 TCX", 0xA3CCD6);
const PLANTATION: PantoneColor = PantoneColor::new("Plantation", "18-0832 TCX", 0x7A6332);
const CATTLEYA_ORCHID: PantoneColor = PantoneColor::new("Cattleya Orchid", "18-3223 TCX", 0x9C4C8D);

// Spectrum Twist
const LEMON_VERBENA: PantoneColor = PantoneColor::new("Lemon Verbena", "12-0742 TCX", 0xF4E87A);
const PURPLE_VELVET: PantoneColor = PantoneColor::new("Purple Velvet", "19-3725 TCX", 0x41354D);
const PEACH_QUARTZ: PantoneColor = PantoneColor::new("Peach Quartz", "13-1125 TCX", 0xF5B895);
const ETHEREAL_BLUE: PantoneColor = PantoneColor::new("Ethereal Blue", "15-4323 TCX", 0x5CA6CE);
const FUCHSIA_PURPLE: PantoneColor = PantoneColor::new("Fuchsia Purple", "18-2436 TCX", 0xD4367A);

// Horizon Beam
const DARK_CITRON: PantoneColor = PantoneColor::new("Dark Citron", "16-0435 TCX", 0xA0AC4F);
const ESTATE_BLUE: PantoneColor = PantoneColor::new("Estate Blue", "19-4027 TCX", 0x233658);
const MUREX_SHELL: PantoneColor = PantoneColor::new("Murex Shell", "15-1712 TCX", 0xF8A3A4);
const PURPLE_HEATHER: PantoneColor = PantoneColor::new("Purple Heather", "14-3911 TCX", 0xBAB8D3);
const KOMBU_GREEN: PantoneColor = PantoneColor::new("Kombu Green", "19-0417 TCX", 0x3A4132);

// Infinite Radiance
const ARTISANS_GOLD: PantoneColor = PantoneColor::new("Artisan's Gold", "15-1049 TCX", 0xF2AB46);
const CUMULUS_CLOUD: PantoneColor = PantoneColor::new("Cumulus Cloud", "14-0207 TCX", 0xB5B0AB);
const BLACK_BEAN: PantoneColor = PantoneColor::new("Black Bean", "19-3909 TCX", 0x2E272A);

// Voxel Vision (also uses Peach Quartz)
const BLUE_ATOLL: PantoneColor = PantoneColor::new("Blue Atoll", "16-4535 TCX", 0x00B1D2); // dataset
const CARBON: PantoneColor = PantoneColor::new("Carbon", "19-4012 TCX", 0x272F38); // dataset
const SAP_GREEN: PantoneColor = PantoneColor::new("Sap Green", "13-0331 TCX", 0xAFCB80); // dataset
const FLAMINGO_PINK: PantoneColor = PantoneColor::new("Flamingo Pink", "15-1821 TCX", 0xF7969E); // dataset

// Electric Escape (also uses Carbon)
const DIRECTOIRE_BLUE: PantoneColor = PantoneColor::new("Directoire Blue", "18-4244 TCX", 0x0061A3); // dataset
const FREESIA: PantoneColor = PantoneColor::new("Freesia", "14-0852 TCX", 0xF3C12C); // dataset
const WILD_ORCHID: PantoneColor = PantoneColor::new("Wild Orchid", "16-2120 TCX", 0xD979A2); // dataset
const SIMPLY_GREEN: PantoneColor = PantoneColor::new("Simply Green", "17-5936 TCX", 0x009B75); // dataset

// Playful Voxel Glow
const ICY_MORN: PantoneColor = PantoneColor::new("Icy Morn", "13-5306 TCX", 0xB0D3D1); // dataset
const FRUIT_DOVE: PantoneColor = PantoneColor::new("Fruit Dove", "17-1926 TCX", 0xCE5B78); // dataset
const GREEN_BEE: PantoneColor = PantoneColor::new("Green Bee", "17-6154 TCX", 0x008C4E); // sampled
const STAR_SAPPHIRE: PantoneColor = PantoneColor::new("Star Sapphire", "18-4041 TCX", 0x386192); // dataset
const AURORA_PINK: PantoneColor = PantoneColor::new("Aurora Pink", "15-2217 TCX", 0xE881A6); // dataset

// Dazzling Dimensions
const AURORA: PantoneColor = PantoneColor::new("Aurora", "12-0642 TCX", 0xEDDD59); // dataset
const LYONS_BLUE: PantoneColor = PantoneColor::new("Lyons Blue", "19-4340 TCX", 0x005871); // dataset
const PRIMROSE_PINK: PantoneColor = PantoneColor::new("Primrose Pink", "12-2904 TCX", 0xEED4D9); // dataset
const DAMSON: PantoneColor = PantoneColor::new("Damson", "18-1716 TCX", 0x854C65); // dataset
const POPPY_RED: PantoneColor = PantoneColor::new("Poppy Red", "17-1664 TCX", 0xDC343B); // dataset

// Island Vibes
const MANGO_MOJITO: PantoneColor = PantoneColor::new("Mango Mojito", "15-0960 TCX", 0xD69C2F);
const OCHRE: PantoneColor = PantoneColor::new("Ochre", "14-1036 TCX", 0xD6AF66);
const PALACE_BLUE: PantoneColor = PantoneColor::new("Palace Blue", "18-4043 TCX", 0x346CB0);
const SURF_THE_WEB: PantoneColor = PantoneColor::new("Surf The Web", "19-3952 TCX", 0x203C7F); // out of gamut
const BLUE_DEPTHS: PantoneColor = PantoneColor::new("Blue Depths", "19-3940 TCX", 0x263056); // out of gamut

// Nueva York
const BLAZING_YELLOW: PantoneColor = PantoneColor::new("Blazing Yellow", "12-0643 TCX", 0xFEE715);
const DESERT_FLOWER: PantoneColor = PantoneColor::new("Desert Flower", "15-1435 TCX", 0xFF9687);
const BACHELOR_BUTTON: PantoneColor = PantoneColor::new("Bachelor Button", "14-4522 TCX", 0x4ABBD5); // out of gamut
const CORNFLOWER_BLUE: PantoneColor = PantoneColor::new("Cornflower Blue", "16-4031 TCX", 0x7391C8);
const ORCHID_BLOOM: PantoneColor = PantoneColor::new("Orchid Bloom", "14-3612 TCX", 0xC5AECF);

// Texas Sun
const BLUEJAY: PantoneColor = PantoneColor::new("Bluejay", "17-4427 TCX", 0x157EA0);
const GOLDEN_ROD: PantoneColor = PantoneColor::new("Golden Rod", "14-0951 TCX", 0xE2A829);
const ORANGE_PEPPER: PantoneColor = PantoneColor::new("Orange Pepper", "16-1164 TCX", 0xDF7500);
const VALIANT_POPPY: PantoneColor = PantoneColor::new("Valiant Poppy", "18-1549 TCX", 0xBC322C);
const DAHLIA: PantoneColor = PantoneColor::new("Dahlia", "18-3324 TCX", 0x843E83); // out of gamut

// Uluwatu Wipeout (also uses Blue Depths)
const BUFF_YELLOW: PantoneColor = PantoneColor::new("Buff Yellow", "14-0847 TCX", 0xF1BF70);
const FLAMINGO: PantoneColor = PantoneColor::new("Flamingo", "16-1450 TCX", 0xDF7253);
const LICHEN_BLUE: PantoneColor = PantoneColor::new("Lichen Blue", "17-4032 TCX", 0x5D89B3);
const CLEMATIS_BLUE: PantoneColor = PantoneColor::new("Clematis Blue", "19-3951 TCX", 0x363B7C);

// Palette names are what settings and the API refer to: keep them unique.
pub const PALETTES: &[Palette] = &[
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
    Palette {
        name: "Texas Sun",
        colors: &[BLUEJAY, GOLDEN_ROD, ORANGE_PEPPER, VALIANT_POPPY, DAHLIA],
    },
    Palette {
        name: "Infinite Radiance",
        colors: &[ARTISANS_GOLD, CUMULUS_CLOUD, BLACK_BEAN],
    },
    Palette {
        name: "Island Vibes",
        colors: &[MANGO_MOJITO, OCHRE, PALACE_BLUE, SURF_THE_WEB, BLUE_DEPTHS],
    },
    Palette {
        name: "Electric Escape",
        colors: &[DIRECTOIRE_BLUE, FREESIA, WILD_ORCHID, CARBON, SIMPLY_GREEN],
    },
    Palette {
        name: "Brilliant Abyss",
        colors: &[
            PRIMROSE_YELLOW,
            OUTER_SPACE,
            AQUA_ESQUE,
            PLANTATION,
            CATTLEYA_ORCHID,
        ],
    },
    Palette {
        name: "Nueva York",
        colors: &[
            BLAZING_YELLOW,
            DESERT_FLOWER,
            BACHELOR_BUTTON,
            CORNFLOWER_BLUE,
            ORCHID_BLOOM,
        ],
    },
    Palette {
        name: "Horizon Beam",
        colors: &[
            DARK_CITRON,
            ESTATE_BLUE,
            MUREX_SHELL,
            PURPLE_HEATHER,
            KOMBU_GREEN,
        ],
    },
    Palette {
        name: "Playful Voxel Glow",
        colors: &[ICY_MORN, FRUIT_DOVE, GREEN_BEE, STAR_SAPPHIRE, AURORA_PINK],
    },
    Palette {
        name: "Voxel Vision",
        colors: &[BLUE_ATOLL, CARBON, SAP_GREEN, FLAMINGO_PINK, PEACH_QUARTZ],
    },
    Palette {
        name: "Spectrum Twist",
        colors: &[
            LEMON_VERBENA,
            PURPLE_VELVET,
            PEACH_QUARTZ,
            ETHEREAL_BLUE,
            FUCHSIA_PURPLE,
        ],
    },
    Palette {
        name: "Dazzling Dimensions",
        colors: &[AURORA, LYONS_BLUE, PRIMROSE_PINK, DAMSON, POPPY_RED],
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

    fn escape(text: &str) -> String {
        text.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    }

    /// Documentation, not a check: writes `doc/palettes.svg`, an overview of all palettes
    /// in order, with each color's name, Pantone code and hex value.
    #[test]
    #[ignore = "writes doc/palettes.svg"]
    fn palettes_overview_svg() {
        const MARGIN: usize = 48;
        const NAME_WIDTH: usize = 230;
        const CHIP: (usize, usize) = (150, 92); // color block
        const LABEL: usize = 66;
        const GAP: usize = 14;
        const ROW: usize = CHIP.1 + LABEL + 28;
        const HEADER: usize = 104;
        let columns = PALETTES.iter().map(|p| p.colors.len()).max().unwrap();
        let width = 2 * MARGIN + NAME_WIDTH + columns * (CHIP.0 + GAP) - GAP;
        let height = HEADER + PALETTES.len() * ROW + MARGIN - 28;

        let mut svg = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}" font-family="-apple-system, 'Helvetica Neue', Arial, sans-serif">
<rect width="100%" height="100%" fill="#F3F2EE"/>
<text x="{MARGIN}" y="{y1}" font-size="30" font-weight="700" fill="#1C1B19">Nova palettes</text>
<text x="{MARGIN}" y="{y2}" font-size="15" fill="#6D6A62">{count} palettes, ordered by the hue of their most saturated color · Pantone FHI (TCX) colors · generated from server/src/content/palettes.rs</text>
"##,
            y1 = MARGIN + 8,
            y2 = MARGIN + 36,
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
<text x="10" y="{t2}" font-size="12" fill="#55524B">{code}</text>
<text x="10" y="{t3}" font-size="12" fill="#55524B" font-family="Menlo, Consolas, monospace">{hex}</text>
</g>
"##,
                    w = CHIP.0,
                    h = CHIP.1 + LABEL,
                    inner = CHIP.0 - 12,
                    chip = CHIP.1 - 6,
                    t1 = CHIP.1 + 20,
                    t2 = CHIP.1 + 37,
                    t3 = CHIP.1 + 54,
                    name = escape(color.name),
                    code = escape(color.code),
                );
            }
        }
        svg += "</svg>\n";
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../doc/palettes.svg");
        std::fs::write(path, svg).unwrap();
        println!("Palette overview: {path}");
    }
}
