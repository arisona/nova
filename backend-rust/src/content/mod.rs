use std::sync::LazyLock;

use palette::{IntoColor, Oklch, Srgb};
use serde_json::{Value, json};

use crate::renderer::RenderState;
use crate::voxel_image::VoxelImage;

pub mod fill;
pub mod ramp;
pub mod simplex;

pub trait Content {
    fn name(&self) -> &str;
    fn render(
        &mut self,
        state: &RenderState,
        elapsed: f32,
        delta: f32,
        prev: &VoxelImage,
        next: &mut VoxelImage,
    );
}

pub fn get_all_content() -> Vec<Box<dyn Content>> {
    vec![
        Box::new(fill::Fill::new()),
        Box::new(ramp::Ramp::new()),
        Box::new(simplex::Simplex::new()),
    ]
}

pub fn get_all_content_names() -> Vec<String> {
    assert_ne!(get_all_content().len(), 0);
    get_all_content()
        .iter()
        .map(|c| c.name().to_string())
        .collect()
}

static PALETTES: LazyLock<Value> = LazyLock::new(|| {
    json!(
        [
            {
                "movie": "The Grand Budapest Hotel (2014)",
                "palette": [
                  { "color": "Pink", "rgb": [244, 194, 194] },
                  { "color": "Purple", "rgb": [204, 153, 204] },
                  { "color": "Red", "rgb": [255, 102, 102] },
                  { "color": "Gold", "rgb": [255, 204, 102] },
                  { "color": "Brown", "rgb": [153, 102, 51] }
                ]
            },
            {
                "movie": "Mad Max: Fury Road (2015)",
                "palette": [
                  { "color": "Teal", "rgb": [14, 115, 115] },
                  { "color": "Orange", "rgb": [217, 112, 20] },
                  { "color": "Gray", "rgb": [128, 128, 128] },
                  { "color": "Brown", "rgb": [102, 51, 0] },
                  { "color": "Black", "rgb": [0, 0, 0] }
                ]
            },
            {
            "movie": "The Shining (1980)",
            "palette": [
                { "color": "Muted Red", "rgb": [210, 117, 117] },
                { "color": "Brown", "rgb": [103, 90, 85] },
                { "color": "Teal", "rgb": [82, 155, 156] },
                { "color": "Olive", "rgb": [156, 186, 143] },
                { "color": "Beige", "rgb": [234, 195, 146] }
            ]
            },
            {
            "movie": "Amélie (2001)",
            "palette": [
                { "color": "Warm Yellow", "rgb": [255, 223, 186] },
                { "color": "Green", "rgb": [153, 204, 102] },
                { "color": "Red", "rgb": [204, 51, 51] },
                { "color": "Brown", "rgb": [102, 51, 0] },
                { "color": "Cream", "rgb": [255, 245, 238] }
            ]
            },
            {
            "movie": "Blade Runner 2049 (2017)",
            "palette": [
                { "color": "Deep Orange", "rgb": [255, 140, 0] },
                { "color": "Dark Gray", "rgb": [64, 64, 64] },
                { "color": "Teal", "rgb": [0, 128, 128] },
                { "color": "Brown", "rgb": [139, 69, 19] },
                { "color": "Black", "rgb": [0, 0, 0] }
            ]
            },
            {
            "movie": "The Matrix (1999)",
            "palette": [
                { "color": "Green", "rgb": [0, 102, 0] },
                { "color": "Black", "rgb": [0, 0, 0] },
                { "color": "Gray", "rgb": [128, 128, 128] },
                { "color": "White", "rgb": [255, 255, 255] },
                { "color": "Dark Green", "rgb": [0, 51, 0] }
            ]
            },
            {
            "movie": "La La Land (2016)",
            "palette": [
                { "color": "Blue", "rgb": [60, 105, 231] },
                { "color": "Yellow", "rgb": [255, 223, 0] },
                { "color": "Red", "rgb": [255, 61, 61] },
                { "color": "Purple", "rgb": [150, 123, 182] },
                { "color": "White", "rgb": [255, 255, 255] }
            ]
            },
            {
            "movie": "Inception (2010)",
            "palette": [
                { "color": "Blue", "rgb": [52, 73, 94] },
                { "color": "Gray", "rgb": [149, 165, 166] },
                { "color": "Beige", "rgb": [241, 196, 15] },
                { "color": "Brown", "rgb": [211, 84, 0] },
                { "color": "Dark Blue", "rgb": [44, 62, 80] }
            ]
            },
            {
            "movie": "The Dark Knight (2008)",
            "palette": [
                { "color": "Dark Blue", "rgb": [44, 62, 80] },
                { "color": "Gray", "rgb": [127, 140, 141] },
                { "color": "Black", "rgb": [0, 0, 0] },
                { "color": "White", "rgb": [236, 240, 241] },
                { "color": "Red", "rgb": [192, 57, 43] }
            ]
            },
            {
            "movie": "Pulp Fiction (1994)",
            "palette": [
                { "color": "Yellow", "rgb": [241, 196, 15] },
                { "color": "Brown", "rgb": [211, 84, 0] },
                { "color": "Red", "rgb": [192, 57, 43] },
                { "color": "Black", "rgb": [0, 0, 0] },
                { "color": "White", "rgb": [236, 240, 241] }
            ]
            },
            {
            "movie": "The Godfather (1972)",
            "palette": [
                { "color": "Dark Brown", "rgb": [67, 24, 17] },
                { "color": "Muted Gold", "rgb": [182, 143, 64] },
                { "color": "Olive Green", "rgb": [107, 94, 53] },
                { "color": "Deep Red", "rgb": [123, 22, 22] },
                { "color": "Charcoal", "rgb": [54, 69, 79] }
            ]
            },
            {
            "movie": "Schindler's List (1993)",
            "palette": [
                { "color": "Black", "rgb": [0, 0, 0] },
                { "color": "Dark Gray", "rgb": [51, 51, 51] },
                { "color": "Gray", "rgb": [102, 102, 102] },
                { "color": "Light Gray", "rgb": [153, 153, 153] },
                { "color": "White", "rgb": [255, 255, 255] }
            ]
            },
            {
            "movie": "Her (2013)",
            "palette": [
                { "color": "Soft Red", "rgb": [215, 94, 86] },
                { "color": "Muted Pink", "rgb": [232, 138, 134] },
                { "color": "Warm Beige", "rgb": [244, 216, 190] },
                { "color": "Pale Orange", "rgb": [255, 178, 154] },
                { "color": "Light Brown", "rgb": [198, 134, 107] }
            ]
            },
            {
            "movie": "The Great Gatsby (2013)",
            "palette": [
                { "color": "Gold", "rgb": [212, 175, 55] },
                { "color": "Black", "rgb": [0, 0, 0] },
                { "color": "White", "rgb": [255, 255, 255] },
                { "color": "Emerald Green", "rgb": [80, 200, 120] },
                { "color": "Deep Blue", "rgb": [1, 70, 130] }
            ]
            },
            {
            "movie": "Moonlight (2016)",
            "palette": [
                { "color": "Deep Blue", "rgb": [25, 25, 112] },
                { "color": "Teal", "rgb": [0, 128, 128] },
                { "color": "Purple", "rgb": [128, 0, 128] },
                { "color": "Magenta", "rgb": [255, 0, 255] },
                { "color": "Soft Pink", "rgb": [255, 182, 193] }
            ]
            },
            {
            "movie": "The Revenant (2015)",
            "palette": [
                { "color": "Cold Blue", "rgb": [52, 73, 94] },
                { "color": "Snow White", "rgb": [255, 250, 250] },
                { "color": "Earth Brown", "rgb": [139, 69, 19] },
                { "color": "Forest Green", "rgb": [34, 139, 34] },
                { "color": "Ash Gray", "rgb": [178, 190, 181] }
            ]
            },
            {
            "movie": "Drive (2011)",
            "palette": [
                { "color": "Neon Pink", "rgb": [255, 20, 147] },
                { "color": "Deep Purple", "rgb": [75, 0, 130] },
                { "color": "Midnight Blue", "rgb": [25, 25, 112] },
                { "color": "Gunmetal Gray", "rgb": [42, 52, 57] },
                { "color": "Soft White", "rgb": [245, 245, 245] }
            ]
            },
            {
            "movie": "Black Panther (2018)",
            "palette": [
                { "color": "Vibrant Purple", "rgb": [128, 0, 128] },
                { "color": "Royal Blue", "rgb": [65, 105, 225] },
                { "color": "Jet Black", "rgb": [52, 52, 52] },
                { "color": "Silver", "rgb": [192, 192, 192] },
                { "color": "Forest Green", "rgb": [34, 139, 34] }
            ]
            },
            {
            "movie": "The Shape of Water (2017)",
            "palette": [
                { "color": "Sea Green", "rgb": [46, 139, 87] },
                { "color": "Aqua", "rgb": [0, 255, 255] },
                { "color": "Teal", "rgb": [0, 128, 128] },
                { "color": "Deep Blue", "rgb": [0, 0, 139] },
                { "color": "Muted Gold", "rgb": [212, 175, 55] }
            ]
            },
            {
            "movie": "Joker (2019)",
            "palette": [
                { "color": "Deep Red", "rgb": [139, 0, 0] },
                { "color": "Mustard Yellow", "rgb": [255, 219, 88] },
                { "color": "Teal Green", "rgb": [0, 128, 128] },
                { "color": "Rust Brown", "rgb": [150, 75, 0] },
                { "color": "Dark Gray", "rgb": [64, 64, 64] }
            ]
            }
        ]
    )
});

pub fn get_palette(name: &str) -> Vec<Oklch> {
    let palette = PALETTES
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["movie"].as_str().unwrap() == name)
        .unwrap();
    let colors = palette["palette"].as_array().unwrap();
    let mut oklch_colors = Vec::new();
    for color in colors {
        let rgb = color["rgb"].as_array().unwrap();
        let r = rgb[0].as_u64().unwrap() as f32 / 255.0;
        let g = rgb[1].as_u64().unwrap() as f32 / 255.0;
        let b = rgb[2].as_u64().unwrap() as f32 / 255.0;
        let rgb = Srgb::new(r, g, b);
        oklch_colors.push(rgb.into_color());
    }
    oklch_colors
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_all_content_names() {
        let names = get_all_content_names();
        assert!(!names.is_empty());
        assert!(names.contains(&"Fill".to_string()));
        assert!(names.contains(&"Ramp".to_string()));
        assert!(names.contains(&"Simplex".to_string()));
    }

    #[test]
    fn test_get_palette_valid_movie() {
        let palette = get_palette("The Grand Budapest Hotel (2014)");
        assert!(!palette.is_empty());
        let first_color = palette[0];
        let rgb: Srgb<f32> = first_color.into_color();
        let rgb8: Srgb<u8> = rgb.into_format();
        assert_eq!(rgb8.red, 244);
        assert_eq!(rgb8.green, 194);
        assert_eq!(rgb8.blue, 194);
    }

    #[test]
    fn test_get_palette_invalid_movie() {
        let result = std::panic::catch_unwind(|| {
            get_palette("Nonexistent Movie");
        });
        assert!(result.is_err());
    }
}
