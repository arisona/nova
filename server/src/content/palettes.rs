//! Curated Pantone palettes, compiled into the binary.
//!
//! Source: Pantone Connect palette generator exports in `doc/palettes/`. Values for
//! colors listed on `00_color_codes.png` are as published there. The remaining colors
//! (marked `sampled`) were measured from the palette screenshots after converting
//! Display P3 to sRGB; checked against the listed colors, this is accurate to ±1 per
//! channel.
//!
//! Colors keep the order of the source palette. How a palette maps onto structure
//! values (source order, sorted by lightness, black anchor) is decided in Flux.

use glam::Vec3;

pub struct PantoneColor {
    pub name: &'static str,
    #[allow(dead_code)] // for the palette picker (Step 4)
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
const BLUE_ATOLL: PantoneColor = PantoneColor::new("Blue Atoll", "16-4535 TCX", 0x02B1D2); // sampled
const CARBON: PantoneColor = PantoneColor::new("Carbon", "19-4012 TCX", 0x262F38); // sampled
const SAP_GREEN: PantoneColor = PantoneColor::new("Sap Green", "13-0331 TCX", 0xAFCB81); // sampled
const FLAMINGO_PINK: PantoneColor = PantoneColor::new("Flamingo Pink", "15-1821 TCX", 0xF7969D); // sampled

// Electric Escape (also uses Carbon)
const DIRECTOIRE_BLUE: PantoneColor = PantoneColor::new("Directoire Blue", "18-4244 TCX", 0x0161A4); // sampled
const FREESIA: PantoneColor = PantoneColor::new("Freesia", "14-0852 TCX", 0xF4C12D); // sampled
const WILD_ORCHID: PantoneColor = PantoneColor::new("Wild Orchid", "16-2120 TCX", 0xD879A1); // sampled
const SIMPLY_GREEN: PantoneColor = PantoneColor::new("Simply Green", "17-5936 TCX", 0x009A74); // sampled

// Playful Voxel Glow
const ICY_MORN: PantoneColor = PantoneColor::new("Icy Morn", "13-5306 TCX", 0xB1D3D1); // sampled
const FRUIT_DOVE: PantoneColor = PantoneColor::new("Fruit Dove", "17-1926 TCX", 0xCF5D78); // sampled
const GREEN_BEE: PantoneColor = PantoneColor::new("Green Bee", "17-6154 TCX", 0x008C4E); // sampled
const STAR_SAPPHIRE: PantoneColor = PantoneColor::new("Star Sapphire", "18-4041 TCX", 0x376192); // sampled
const AURORA_PINK: PantoneColor = PantoneColor::new("Aurora Pink", "15-2217 TCX", 0xE882A6); // sampled

// Dazzling Dimensions
const AURORA: PantoneColor = PantoneColor::new("Aurora", "12-0642 TCX", 0xEDDD58); // sampled
const LYONS_BLUE: PantoneColor = PantoneColor::new("Lyons Blue", "19-4340 TCX", 0x025772); // sampled
const PRIMROSE_PINK: PantoneColor = PantoneColor::new("Primrose Pink", "12-2904 TCX", 0xEED5D8); // sampled
const DAMSON: PantoneColor = PantoneColor::new("Damson", "18-1716 TCX", 0x844C65); // sampled
const POPPY_RED: PantoneColor = PantoneColor::new("Poppy Red", "17-1664 TCX", 0xDC353C); // sampled

pub const PALETTES: &[Palette] = &[
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
        name: "Infinite Radiance",
        colors: &[ARTISANS_GOLD, CUMULUS_CLOUD, BLACK_BEAN],
    },
    Palette {
        name: "Voxel Vision",
        colors: &[BLUE_ATOLL, CARBON, SAP_GREEN, FLAMINGO_PINK, PEACH_QUARTZ],
    },
    Palette {
        name: "Electric Escape",
        colors: &[DIRECTOIRE_BLUE, FREESIA, WILD_ORCHID, CARBON, SIMPLY_GREEN],
    },
    Palette {
        name: "Playful Voxel Glow",
        colors: &[ICY_MORN, FRUIT_DOVE, GREEN_BEE, STAR_SAPPHIRE, AURORA_PINK],
    },
    Palette {
        name: "Dazzling Dimensions",
        colors: &[AURORA, LYONS_BLUE, PRIMROSE_PINK, DAMSON, POPPY_RED],
    },
];
