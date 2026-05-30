//! Terrain heightmap loading.
//!
//! HOI4 stores terrain elevation in `map/heightmap.bmp`, an 8-bit greyscale
//! BMP at the same dimensions as `provinces.bmp` (5632 × 2048 in vanilla).
//! Pixel value 0..=255 maps roughly to "low → high" elevation; sea level is
//! around 95 in vanilla.

use std::path::Path;

/// Greyscale heightmap. Pixel value 0..=255.
#[derive(Debug, Clone)]
pub struct Heightmap {
    pub width: u32,
    pub height: u32,
    /// Flat array, row-major, top-down. `pixels[y * width + x] = grayscale`.
    pub pixels: Vec<u8>,
}

impl Heightmap {
    /// Sea level threshold in vanilla HOI4 heightmap (≈ 94..96).
    /// Pixels with value ≤ this are water.
    pub const SEA_LEVEL: u8 = 95;

    /// Get raw greyscale value at pixel coordinate (clamped).
    #[inline]
    pub fn get(&self, x: u32, y: u32) -> u8 {
        let x = x.min(self.width.saturating_sub(1));
        let y = y.min(self.height.saturating_sub(1));
        self.pixels[(y * self.width + x) as usize]
    }

    /// Sample height in [0,1] using bilinear interpolation. UV in [0,1].
    pub fn sample_uv(&self, u: f32, v: f32) -> f32 {
        let u = u.clamp(0.0, 1.0);
        let v = v.clamp(0.0, 1.0);
        let fx = u * (self.width.saturating_sub(1)) as f32;
        let fy = v * (self.height.saturating_sub(1)) as f32;
        let x0 = fx.floor() as u32;
        let y0 = fy.floor() as u32;
        let x1 = (x0 + 1).min(self.width - 1);
        let y1 = (y0 + 1).min(self.height - 1);
        let tx = fx - x0 as f32;
        let ty = fy - y0 as f32;

        let h00 = self.get(x0, y0) as f32;
        let h10 = self.get(x1, y0) as f32;
        let h01 = self.get(x0, y1) as f32;
        let h11 = self.get(x1, y1) as f32;

        let h0 = h00 + (h10 - h00) * tx;
        let h1 = h01 + (h11 - h01) * tx;
        (h0 + (h1 - h0) * ty) / 255.0
    }

    /// Number of pixels above sea level (rough land-ratio sanity check).
    pub fn land_pixel_count(&self) -> usize {
        self.pixels.iter().filter(|&&v| v > Self::SEA_LEVEL).count()
    }
}

/// Load a greyscale heightmap from `path`. Accepts any image format `image`
/// supports; converts to luminance.
pub fn load_heightmap(path: &Path) -> Result<Heightmap, String> {
    let img = image::open(path)
        .map_err(|e| format!("Failed to open {}: {}", path.display(), e))?
        .into_luma8();
    let width = img.width();
    let height = img.height();
    let pixels = img.into_raw();
    Ok(Heightmap {
        width,
        height,
        pixels,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic() -> Heightmap {
        // 4×3 ramp: each row goes 0,85,170,255
        let row = [0u8, 85, 170, 255];
        let mut pixels = Vec::new();
        for _ in 0..3 {
            pixels.extend_from_slice(&row);
        }
        Heightmap {
            width: 4,
            height: 3,
            pixels,
        }
    }

    #[test]
    fn get_clamps_out_of_range() {
        let h = synthetic();
        assert_eq!(h.get(0, 0), 0);
        assert_eq!(h.get(3, 2), 255);
        // Out of range clamps to last pixel
        assert_eq!(h.get(99, 99), 255);
    }

    #[test]
    fn sample_uv_corners() {
        let h = synthetic();
        // top-left
        assert!((h.sample_uv(0.0, 0.0) - 0.0).abs() < 1e-4);
        // top-right (last column, first row)
        assert!((h.sample_uv(1.0, 0.0) - 1.0).abs() < 1e-4);
    }

    #[test]
    fn sample_uv_interpolates() {
        let h = synthetic();
        // Row is [0, 85, 170, 255]. At u=0.5 (fx = 1.5) it interpolates
        // between pixels 1 (85) and 2 (170) → (85+170)/2 / 255 ≈ 0.5.
        let mid = h.sample_uv(0.5, 0.0);
        assert!((mid - 0.5).abs() < 0.01, "expected ~0.5, got {mid}");

        // u just past 0 should still be near 0 (between pixel 0 and 1).
        let near0 = h.sample_uv(0.05, 0.0);
        assert!(near0 < 0.1, "expected near 0, got {near0}");
    }

    #[test]
    fn land_pixel_count_works() {
        let h = synthetic();
        // 170 and 255 are above SEA_LEVEL=95 → 2 cols × 3 rows = 6
        assert_eq!(h.land_pixel_count(), 6);
    }
}
