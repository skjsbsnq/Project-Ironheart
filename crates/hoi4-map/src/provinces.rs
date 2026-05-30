use std::collections::HashMap;
use std::path::Path;

/// Province index map: each pixel stores a province ID (u16)
pub struct ProvinceMap {
    pub width: u32,
    pub height: u32,
    /// Flat array: pixels[y * width + x] = province_id
    pub pixels: Vec<u16>,
}

impl ProvinceMap {
    /// Get province ID at pixel coordinate
    pub fn get(&self, x: u32, y: u32) -> u16 {
        self.pixels[(y * self.width + x) as usize]
    }

    /// Build adjacency graph by scanning neighboring pixels
    pub fn build_adjacency(&self, adjacencies: &mut Vec<Vec<u16>>) {
        let w = self.width as usize;
        let h = self.height as usize;

        for y in 0..h {
            for x in 0..w {
                let id = self.pixels[y * w + x];
                if id == 0 {
                    continue;
                }

                // Check right neighbor
                if x + 1 < w {
                    let neighbor = self.pixels[y * w + x + 1];
                    if neighbor != 0 && neighbor != id {
                        add_neighbor(adjacencies, id, neighbor);
                    }
                }
                // Check bottom neighbor
                if y + 1 < h {
                    let neighbor = self.pixels[(y + 1) * w + x];
                    if neighbor != 0 && neighbor != id {
                        add_neighbor(adjacencies, id, neighbor);
                    }
                }
            }
        }
    }
}

fn add_neighbor(adjacencies: &mut Vec<Vec<u16>>, a: u16, b: u16) {
    let a_idx = a as usize;
    let b_idx = b as usize;
    if a_idx < adjacencies.len() && !adjacencies[a_idx].contains(&b) {
        adjacencies[a_idx].push(b);
    }
    if b_idx < adjacencies.len() && !adjacencies[b_idx].contains(&a) {
        adjacencies[b_idx].push(a);
    }
}

/// Load provinces.bmp and convert RGB pixels to province IDs using the lookup table
pub fn load_province_map(
    path: &Path,
    rgb_to_id: &HashMap<(u8, u8, u8), u16>,
) -> Result<ProvinceMap, String> {
    let img = image::open(path)
        .map_err(|e| format!("Failed to open {}: {}", path.display(), e))?
        .into_rgb8();

    let width = img.width();
    let height = img.height();
    let mut pixels = vec![0u16; (width * height) as usize];

    for (i, pixel) in img.pixels().enumerate() {
        let rgb = (pixel[0], pixel[1], pixel[2]);
        if let Some(&id) = rgb_to_id.get(&rgb) {
            pixels[i] = id;
        }
    }

    Ok(ProvinceMap {
        width,
        height,
        pixels,
    })
}
