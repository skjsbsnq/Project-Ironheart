//! Parser for HOI4's `map/trees.bmp` — an 8-bit palette-indexed BMP that the
//! art team hand-paints to mark which spots on the world map should receive
//! trees and which species.
//!
//! Unlike `terrain.bmp` (5632×2048, one byte = terrain category), trees.bmp is
//! a *separate*, lower-resolution map (vanilla 1650×600) used purely for
//! billboard / mesh placement. Most pixels are 254/255 (no tree). The
//! "active" indices in vanilla are 3, 4, 7, 10 (declared in `default.map`'s
//! `tree = { 3 4 7 10 }` block).
//!
//! Coarse semantic mapping that this project uses for tree-type routing:
//! | Index | Tree species               | Engine `tree_type` |
//! |-------|----------------------------|--------------------|
//! | 3     | Beech / temperate broadleaf | 0 (deciduous) |
//! | 4     | Mixed forest (beech-heavy)  | 0 (deciduous) |
//! | 7     | Pine / boreal               | 1 (conifer) |
//! | 10    | Palm / jungle               | 2 (tropical) |
//! | other | No tree                     | — |
//!
//! Format note: like `terrain.bmp`, this is a standard 8-bit indexed BMP
//! (BITMAPINFOHEADER + BI_RGB). We reuse the parser from
//! [`crate::terrain_bmp::parse_indexed_bmp`] to avoid duplicating BMP code.

use crate::terrain_bmp::{parse_indexed_bmp, TerrainBitmap};
use std::collections::HashSet;
use std::path::Path;

/// Default fall-back set of "active" tree palette indices when `default.map`
/// is missing or its `tree` block fails to parse. Matches vanilla HOI4.
pub const DEFAULT_TREE_INDICES: &[u8] = &[3, 4, 7, 10];

/// Per-pixel "this spot has a tree of species X" map.
#[derive(Debug, Clone)]
pub struct TreeBitmap {
    pub width: u32,
    pub height: u32,
    /// Flat row-major (top-down). Each value is a vanilla tree-palette index.
    pub pixels: Vec<u8>,
    /// 256-entry RGB palette read from the BMP.
    pub palette: [[u8; 3]; 256],
}

impl TreeBitmap {
    #[inline]
    pub fn get(&self, x: u32, y: u32) -> u8 {
        let x = x.min(self.width.saturating_sub(1));
        let y = y.min(self.height.saturating_sub(1));
        self.pixels[(y * self.width + x) as usize]
    }

    /// Map raw palette index → coarse engine tree-type:
    /// `Some(0)` deciduous, `Some(1)` conifer, `Some(2)` tropical, `None` no
    /// tree (or index not in the active set).
    #[inline]
    pub fn type_for(idx: u8, active: &HashSet<u8>) -> Option<u8> {
        if !active.contains(&idx) {
            return None;
        }
        Some(match idx {
            3 | 4 => 0, // beech / mixed temperate
            7 => 1,     // pine / boreal
            10 => 2,    // palm / tropical
            _ => 0,     // unknown active index → default to deciduous
        })
    }

    /// Total number of pixels whose palette index is in the active set
    /// (debug / metrics).
    pub fn active_pixel_count(&self, active: &HashSet<u8>) -> usize {
        self.pixels.iter().filter(|&&p| active.contains(&p)).count()
    }
}

/// Load `map/trees.bmp` from disk.
pub fn load_trees_bmp(path: &Path) -> Result<TreeBitmap, String> {
    let bytes =
        std::fs::read(path).map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
    parse_trees_bmp(&bytes)
}

/// Parse `trees.bmp` from a byte slice. Internally reuses the generic
/// indexed-BMP parser; this wrapper exists for API symmetry with terrain
/// and rivers.
pub fn parse_trees_bmp(bytes: &[u8]) -> Result<TreeBitmap, String> {
    let inner: TerrainBitmap = parse_indexed_bmp(bytes)?;
    Ok(TreeBitmap {
        width: inner.width,
        height: inner.height,
        pixels: inner.pixels,
        palette: inner.palette,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synth_8bit_bmp(w: u32, h: u32, pixels: &[u8]) -> Vec<u8> {
        let row_padded = (w as usize + 3) & !3;
        let pixel_data_size = row_padded * h as usize;
        let palette_size = 256 * 4;
        let pixel_offset = 14 + 40 + palette_size;
        let file_size = pixel_offset + pixel_data_size;

        let mut out = Vec::with_capacity(file_size);
        out.extend_from_slice(b"BM");
        out.extend_from_slice(&(file_size as u32).to_le_bytes());
        out.extend_from_slice(&[0u8; 4]);
        out.extend_from_slice(&(pixel_offset as u32).to_le_bytes());
        out.extend_from_slice(&40u32.to_le_bytes());
        out.extend_from_slice(&(w as i32).to_le_bytes());
        out.extend_from_slice(&(h as i32).to_le_bytes()); // bottom-up
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&8u16.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&(pixel_data_size as u32).to_le_bytes());
        out.extend_from_slice(&[0u8; 16]);
        // Palette: index = grey level
        for i in 0..256u16 {
            out.push(i as u8);
            out.push(i as u8);
            out.push(i as u8);
            out.push(0);
        }
        // pixel data, bottom-up
        assert_eq!(pixels.len(), (w * h) as usize);
        for y in (0..h).rev() {
            let row = &pixels[(y * w) as usize..((y + 1) * w) as usize];
            out.extend_from_slice(row);
            for _ in 0..(row_padded - w as usize) {
                out.push(0);
            }
        }
        out
    }

    #[test]
    fn parses_synthetic_trees_bmp() {
        // 4×2 image: top row [3, 4, 7, 10], bottom row [254, 254, 0, 5]
        let pixels = vec![3u8, 4, 7, 10, 254, 254, 0, 5];
        let bmp = synth_8bit_bmp(4, 2, &pixels);
        let t = parse_trees_bmp(&bmp).expect("ok");
        assert_eq!((t.width, t.height), (4, 2));
        // Top-down output
        assert_eq!(t.pixels, pixels);
    }

    #[test]
    fn type_for_routes_to_correct_species() {
        let active: HashSet<u8> = DEFAULT_TREE_INDICES.iter().copied().collect();
        assert_eq!(TreeBitmap::type_for(3, &active), Some(0));
        assert_eq!(TreeBitmap::type_for(4, &active), Some(0));
        assert_eq!(TreeBitmap::type_for(7, &active), Some(1));
        assert_eq!(TreeBitmap::type_for(10, &active), Some(2));
        // Inactive index → None
        assert_eq!(TreeBitmap::type_for(254, &active), None);
        assert_eq!(TreeBitmap::type_for(0, &active), None);
        assert_eq!(TreeBitmap::type_for(5, &active), None);
    }

    #[test]
    fn type_for_respects_custom_active_set() {
        // mod that adds palette index 5 as deciduous
        let mut active: HashSet<u8> = DEFAULT_TREE_INDICES.iter().copied().collect();
        active.insert(5);
        // 5 is not in the special enum branches, should default to deciduous (0)
        assert_eq!(TreeBitmap::type_for(5, &active), Some(0));
        // Anything outside still None
        assert_eq!(TreeBitmap::type_for(99, &active), None);
    }

    #[test]
    fn active_pixel_count_filters_correctly() {
        let pixels = vec![3u8, 4, 7, 10, 254, 254, 0, 5];
        let bmp = synth_8bit_bmp(4, 2, &pixels);
        let t = parse_trees_bmp(&bmp).unwrap();
        let active: HashSet<u8> = DEFAULT_TREE_INDICES.iter().copied().collect();
        assert_eq!(t.active_pixel_count(&active), 4); // 3, 4, 7, 10
    }

    #[test]
    fn default_indices_match_vanilla_default_map() {
        // Sanity: default `tree = { 3 4 7 10 }` block in vanilla `default.map`.
        assert_eq!(DEFAULT_TREE_INDICES, &[3, 4, 7, 10]);
    }
}
