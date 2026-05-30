//! Builds a 256×1 RGBA palette texture mapping `terrain.bmp` indices
//! to display colours.
//!
//! Colours are chosen to be "natural map" tints — green for forest, sand for
//! desert, snow-white for `perm_snow` variants, etc. They sit in the shader's
//! mix between political tint and pure terrain tone.

use hoi4_map::{TerrainCatalog, TerrainCategory};

/// 256 RGBA bytes (4 × 256 = 1024 bytes).
pub fn build_terrain_palette(catalog: &TerrainCatalog) -> Vec<u8> {
    let mut palette = vec![0u8; 256 * 4];
    for index in 0..=255u32 {
        let idx = index as u8;
        let entry = catalog.by_index.get(&idx);
        let (cat, perm_snow) = match entry {
            Some(e) => (e.category.clone(), e.perm_snow),
            None => (TerrainCategory::Unknown, false),
        };
        let (mut r, mut g, mut b) = base_color(&cat);
        if perm_snow {
            // Mix toward snow white (75% snow / 25% terrain).
            r = ((r as u16 * 25 + 245 * 75) / 100) as u8;
            g = ((g as u16 * 25 + 250 * 75) / 100) as u8;
            b = ((b as u16 * 25 + 252 * 75) / 100) as u8;
        }
        let off = index as usize * 4;
        palette[off] = r;
        palette[off + 1] = g;
        palette[off + 2] = b;
        palette[off + 3] = 255;
    }
    palette
}

fn base_color(cat: &TerrainCategory) -> (u8, u8, u8) {
    match cat {
        TerrainCategory::Plains => (140, 175, 95),
        TerrainCategory::Forest => (60, 105, 60),
        TerrainCategory::Hills => (155, 145, 95),
        TerrainCategory::Mountain => (130, 120, 110),
        TerrainCategory::Desert => (220, 200, 140),
        TerrainCategory::Marsh => (95, 115, 75),
        TerrainCategory::Jungle => (45, 100, 50),
        TerrainCategory::Urban => (140, 130, 130),
        TerrainCategory::Ocean => (30, 65, 110),
        TerrainCategory::Lakes => (60, 100, 145),
        TerrainCategory::WaterFjords => (75, 135, 165),
        TerrainCategory::WaterShallowSea => (60, 120, 165),
        TerrainCategory::WaterDeepOcean => (15, 50, 105),
        TerrainCategory::Unknown | TerrainCategory::Other(_) => (120, 120, 120),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hoi4_map::{TerrainCatalog, TerrainEntry};
    use std::collections::HashMap;

    fn catalog_with(entries: &[(u8, TerrainCategory, bool)]) -> TerrainCatalog {
        let mut by_index = HashMap::new();
        for (idx, cat, snow) in entries {
            by_index.insert(
                *idx,
                TerrainEntry {
                    category: cat.clone(),
                    perm_snow: *snow,
                    atlas_idx: None,
                },
            );
        }
        TerrainCatalog { by_index }
    }

    #[test]
    fn palette_is_1024_bytes() {
        let cat = catalog_with(&[]);
        let pal = build_terrain_palette(&cat);
        assert_eq!(pal.len(), 1024);
        // Every alpha byte should be 255
        for i in 0..256 {
            assert_eq!(pal[i * 4 + 3], 255);
        }
    }

    #[test]
    fn known_indices_get_category_color() {
        let cat = catalog_with(&[
            (0, TerrainCategory::Plains, false),
            (5, TerrainCategory::Mountain, false),
            (15, TerrainCategory::Ocean, false),
        ]);
        let pal = build_terrain_palette(&cat);
        // Plains
        assert_eq!(&pal[0..3], &[140, 175, 95]);
        // Mountain
        assert_eq!(&pal[5 * 4..5 * 4 + 3], &[130, 120, 110]);
        // Ocean
        assert_eq!(&pal[15 * 4..15 * 4 + 3], &[30, 65, 110]);
    }

    #[test]
    fn perm_snow_tints_white() {
        let cat = catalog_with(&[(16, TerrainCategory::Mountain, true)]);
        let pal = build_terrain_palette(&cat);
        let r = pal[16 * 4];
        let g = pal[16 * 4 + 1];
        let b = pal[16 * 4 + 2];
        // Should be much brighter than vanilla mountain (130,120,110).
        assert!(r > 200, "expected snow R > 200, got {r}");
        assert!(g > 200, "expected snow G > 200, got {g}");
        assert!(b > 200, "expected snow B > 200, got {b}");
    }

    #[test]
    fn unmapped_index_falls_to_neutral_gray() {
        let cat = catalog_with(&[]);
        let pal = build_terrain_palette(&cat);
        assert_eq!(&pal[100 * 4..100 * 4 + 3], &[120, 120, 120]);
    }
}
