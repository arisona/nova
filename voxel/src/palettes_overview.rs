//! Documentation, not a check: an overview of the palettes with their colors as the content
//! blends them (see `palettes_overview_svg`).

use nova_core::palettes::PALETTES;

use crate::content::common::PaletteMix;

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
<text x="{MARGIN}" y="{y2}" font-size="15" fill="#6D6A62">{count} palettes in alphabetical order · colors in OKLCh, kelvin or sRGB · generated from server/nova-core/src/palettes.rs</text>
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
