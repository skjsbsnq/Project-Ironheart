//! Parser for HOI4's `map/rivers.bmp` — an 8-bit palette-indexed BMP encoding
//! the world's river network as per-pixel river type indices.
//!
//! ## Vanilla palette semantics (per HOI4 modding wiki)
//! | Index | Meaning |
//! |-------|---------|
//! | 0     | Mouth (river end / source)         — start point |
//! | 1     | Reserved (start)                   — flow source |
//! | 2     | Smallest river (1px wide)          — small-1 |
//! | 3     | Small river                        — small-2 |
//! | 4     | Medium-small river                 — small-3 |
//! | 5     | Medium river                       — medium-1 |
//! | 6     | Wider medium river                 — medium-2 |
//! | 7     | Wide river                         — large-1 |
//! | 8     | Wider river                        — large-2 |
//! | 9     | Widest river                       — large-3 |
//! | 10–11 | Reserved / flow markers (visual same as adjacent index) |
//! | 254   | No river (background, white in palette)  |
//! | 255   | Out-of-map / lake fill |
//!
//! For rendering we collapse the type to a "river level" 0..=4 where higher
//! = wider; level 0 means *no river*. This is what the shader samples for
//! width / LOD.
//!
//! Format note: Like `terrain.bmp`, rivers is a standard 8-bit indexed BMP
//! (BITMAPINFOHEADER, BI_RGB, optionally bottom-up). We reuse the parser
//! from [`crate::terrain_bmp::parse_indexed_bmp`].

use crate::terrain_bmp::{parse_indexed_bmp, TerrainBitmap};
use std::path::Path;

/// Per-pixel river type map.
#[derive(Debug, Clone)]
pub struct RiverBitmap {
    pub width: u32,
    pub height: u32,
    /// Flat row-major (top-down). Each value is a vanilla river-palette index
    /// (see module docs). 254/255 = no river.
    pub pixels: Vec<u8>,
    /// 256-entry RGB palette read from the BMP (mostly white background +
    /// blue gradient for river types).
    pub palette: [[u8; 3]; 256],
}

impl RiverBitmap {
    #[inline]
    pub fn get(&self, x: u32, y: u32) -> u8 {
        let x = x.min(self.width.saturating_sub(1));
        let y = y.min(self.height.saturating_sub(1));
        self.pixels[(y * self.width + x) as usize]
    }

    /// Map vanilla river palette index → coarse "river level" 0..=4.
    /// Level 0 = no river; 4 = widest river. Used as direct shader input
    /// for width / LOD selection.
    #[inline]
    pub fn level_at(&self, x: u32, y: u32) -> u8 {
        Self::index_to_level(self.get(x, y))
    }

    /// Map raw palette index to coarse level. Pure helper, no allocation.
    #[inline]
    pub fn index_to_level(idx: u8) -> u8 {
        match idx {
            0 | 1 => 1,           // sources / mouths — render as level 1
            2 | 3 => 1,           // small-1/2
            4 | 5 => 2,           // small-3 / medium-1
            6 | 7 => 3,           // medium-2 / large-1
            8 | 9 | 10 | 11 => 4, // large-2/3 + flow markers
            _ => 0,               // 254/255/other = no river
        }
    }

    /// Total non-zero river pixels (debug / metrics).
    pub fn river_pixel_count(&self) -> usize {
        self.pixels
            .iter()
            .filter(|&&p| Self::index_to_level(p) > 0)
            .count()
    }

    /// Build a packed `R8Unorm`-friendly byte buffer where each byte =
    /// `level * 64` (so 0 / 64 / 128 / 192 / 255). Renderer can sample as
    /// f32 in [0, 1] and threshold per LOD.
    pub fn to_r8_normalised(&self) -> Vec<u8> {
        self.pixels
            .iter()
            .map(|&p| match Self::index_to_level(p) {
                0 => 0u8,
                1 => 64,
                2 => 128,
                3 => 192,
                _ => 255,
            })
            .collect()
    }
}

/// Load `rivers.bmp` from disk.
pub fn load_rivers_bmp(path: &Path) -> Result<RiverBitmap, String> {
    let bytes =
        std::fs::read(path).map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
    parse_rivers_bmp(&bytes)
}

/// Parse `rivers.bmp` from a byte slice. Internally reuses the generic
/// indexed-BMP parser; this wrapper exists for API symmetry with terrain.
pub fn parse_rivers_bmp(bytes: &[u8]) -> Result<RiverBitmap, String> {
    let terrain: TerrainBitmap = parse_indexed_bmp(bytes)?;
    Ok(RiverBitmap {
        width: terrain.width,
        height: terrain.height,
        pixels: terrain.pixels,
        palette: terrain.palette,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_to_level_ranges() {
        assert_eq!(RiverBitmap::index_to_level(0), 1);
        assert_eq!(RiverBitmap::index_to_level(1), 1);
        assert_eq!(RiverBitmap::index_to_level(2), 1);
        assert_eq!(RiverBitmap::index_to_level(3), 1);
        assert_eq!(RiverBitmap::index_to_level(5), 2);
        assert_eq!(RiverBitmap::index_to_level(7), 3);
        assert_eq!(RiverBitmap::index_to_level(9), 4);
        assert_eq!(RiverBitmap::index_to_level(11), 4);
        assert_eq!(RiverBitmap::index_to_level(50), 0);
        assert_eq!(RiverBitmap::index_to_level(254), 0);
        assert_eq!(RiverBitmap::index_to_level(255), 0);
    }

    fn synth_bmp_8bit(w: u32, h: u32, pixels: &[u8]) -> Vec<u8> {
        // Minimal 8-bit indexed BMP synthesizer for tests.
        let row_padded = (w as usize + 3) & !3;
        let pixel_data_size = row_padded * h as usize;
        let palette_size = 256 * 4;
        let pixel_offset = 14 + 40 + palette_size;
        let file_size = pixel_offset + pixel_data_size;

        let mut out = Vec::with_capacity(file_size);
        // BITMAPFILEHEADER (14 bytes)
        out.extend_from_slice(b"BM");
        out.extend_from_slice(&(file_size as u32).to_le_bytes());
        out.extend_from_slice(&[0u8; 4]); // reserved
        out.extend_from_slice(&(pixel_offset as u32).to_le_bytes());
        // BITMAPINFOHEADER (40 bytes)
        out.extend_from_slice(&40u32.to_le_bytes()); // info size
        out.extend_from_slice(&(w as i32).to_le_bytes());
        out.extend_from_slice(&(h as i32).to_le_bytes()); // bottom-up
        out.extend_from_slice(&1u16.to_le_bytes()); // planes
        out.extend_from_slice(&8u16.to_le_bytes()); // bpp
        out.extend_from_slice(&0u32.to_le_bytes()); // BI_RGB
        out.extend_from_slice(&(pixel_data_size as u32).to_le_bytes()); // image size
        out.extend_from_slice(&[0u8; 16]); // x/y resolution + clr_used + clr_important
                                           // Palette (256 BGRA entries)
        for i in 0..256u16 {
            out.push(i as u8); // B
            out.push(i as u8); // G
            out.push(i as u8); // R
            out.push(0); // _
        }
        // Pixel data, bottom-up
        assert_eq!(pixels.len(), (w * h) as usize);
        for y in (0..h).rev() {
            let row = &pixels[(y * w) as usize..((y + 1) * w) as usize];
            out.extend_from_slice(row);
            // pad
            for _ in 0..(row_padded - w as usize) {
                out.push(0);
            }
        }
        out
    }

    #[test]
    fn parses_synthetic_river_bmp() {
        let pixels = vec![254u8, 4, 7, 254, 9, 254, 254, 254, 0];
        let bmp = synth_bmp_8bit(3, 3, &pixels);
        let r = parse_rivers_bmp(&bmp).expect("parse ok");
        assert_eq!((r.width, r.height), (3, 3));
        // top-down expected order (input was bottom-up source)
        assert_eq!(r.pixels.len(), 9);
        // Counts
        let count = r.river_pixel_count();
        assert!(count >= 4, "expected >=4 river pixels, got {count}");
    }

    #[test]
    fn r8_normalised_thresholds() {
        let pixels = vec![0u8, 2, 5, 7, 9, 254];
        let bmp = synth_bmp_8bit(6, 1, &pixels);
        let r = parse_rivers_bmp(&bmp).unwrap();
        let bytes = r.to_r8_normalised();
        assert_eq!(bytes.len(), 6);
        // Mapping: 0→64 (idx 0 = mouth, level 1), 2→64, 5→128, 7→192, 9→255, 254→0
        assert_eq!(bytes[0], 64);
        assert_eq!(bytes[1], 64);
        assert_eq!(bytes[2], 128);
        assert_eq!(bytes[3], 192);
        assert_eq!(bytes[4], 255);
        assert_eq!(bytes[5], 0);
    }
}
