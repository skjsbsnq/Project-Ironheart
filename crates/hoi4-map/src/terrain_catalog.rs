//! Parser for `common/terrain/00_terrain.txt` — extracts the
//! `terrain.bmp` palette-index → terrain category mapping.
//!
//! HOI4's terrain file has two top-level blocks:
//! * `categories = { plains = { ... }, forest = { ... }, ... }`
//!   — gameplay properties keyed by category name.
//! * `terrain = { terrain_0 = { type = plains color = { 0 } texture = 1 }, ... }`
//!   — per-`terrain.bmp`-index entries that map an index to a category.
//!
//! For rendering we only need the index → category map and a "perm snow" flag.

use std::collections::HashMap;
use std::path::Path;

use clausewitz_parser::{parse, Value};

/// Built-in terrain categories used by HOI4 vanilla. Mods may add more —
/// unknown names land in `Other`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TerrainCategory {
    Plains,
    Forest,
    Hills,
    Mountain,
    Desert,
    Marsh,
    Jungle,
    Urban,
    Ocean,
    Lakes,
    WaterFjords,
    WaterShallowSea,
    WaterDeepOcean,
    Unknown,
    Other(String),
}

impl TerrainCategory {
    pub fn parse(s: &str) -> Self {
        match s {
            "plains" => Self::Plains,
            "forest" => Self::Forest,
            "hills" => Self::Hills,
            "mountain" => Self::Mountain,
            "desert" => Self::Desert,
            "marsh" => Self::Marsh,
            "jungle" => Self::Jungle,
            "urban" => Self::Urban,
            "ocean" => Self::Ocean,
            "lakes" => Self::Lakes,
            "water_fjords" => Self::WaterFjords,
            "water_shallow_sea" => Self::WaterShallowSea,
            "water_deep_ocean" => Self::WaterDeepOcean,
            "unknown" => Self::Unknown,
            other => Self::Other(other.to_string()),
        }
    }

    pub fn is_water(&self) -> bool {
        matches!(
            self,
            Self::Ocean
                | Self::Lakes
                | Self::WaterFjords
                | Self::WaterShallowSea
                | Self::WaterDeepOcean
        )
    }
}

/// Per-index entry from the `terrain` block.
#[derive(Debug, Clone)]
pub struct TerrainEntry {
    pub category: TerrainCategory,
    pub perm_snow: bool,
    pub atlas_idx: Option<u8>,
}

/// Parsed terrain catalog (indexed by `terrain.bmp` palette index).
#[derive(Debug, Clone, Default)]
pub struct TerrainCatalog {
    /// index 0..=255 → entry. Unmapped indices fall through to `Unknown`.
    pub by_index: HashMap<u8, TerrainEntry>,
}

impl TerrainCatalog {
    pub fn category_at(&self, index: u8) -> TerrainCategory {
        self.by_index
            .get(&index)
            .map(|e| e.category.clone())
            .unwrap_or(TerrainCategory::Unknown)
    }

    pub fn perm_snow_at(&self, index: u8) -> bool {
        self.by_index
            .get(&index)
            .map(|e| e.perm_snow)
            .unwrap_or(false)
    }

    pub fn len(&self) -> usize {
        self.by_index.len()
    }

    /// Build a 16-entry LUT: `terrain.bmp` palette index -> atlas tile index.
    /// Unmapped or out-of-range indices fall back to identity (idx -> idx).
    pub fn atlas_idx_array(&self) -> [u8; 16] {
        let mut arr = [0u8; 16];
        for i in 0..16u8 {
            arr[i as usize] = self.by_index.get(&i).and_then(|e| e.atlas_idx).unwrap_or(i);
        }
        arr
    }

    /// Build the full shader LUT: `terrain.bmp` palette index -> atlas tile.
    ///
    /// Vanilla can use indices above 15 for snow/variant entries while still
    /// pointing them at one of the 4x4 atlas tiles through `texture = N`.
    /// Keeping all 256 entries prevents the shader from masking those indices
    /// down and losing `perm_snow` semantics.
    pub fn atlas_idx_array_256(&self) -> [u8; 256] {
        let mut arr = [0u8; 256];
        for i in 0..=255u8 {
            arr[i as usize] = self
                .by_index
                .get(&i)
                .and_then(|e| e.atlas_idx)
                .unwrap_or(i & 15);
        }
        arr
    }

    /// Build a compact 256-entry flag LUT for shader-side terrain metadata.
    ///
    /// bit 0 = `perm_snow`, bit 1 = water terrain category.
    pub fn terrain_flags_array_256(&self) -> [u8; 256] {
        let mut arr = [0u8; 256];
        for i in 0..=255u8 {
            if let Some(entry) = self.by_index.get(&i) {
                let mut flags = 0u8;
                if entry.perm_snow {
                    flags |= 1;
                }
                if entry.category.is_water() {
                    flags |= 2;
                }
                arr[i as usize] = flags;
            }
        }
        arr
    }
}

/// Load `common/terrain/00_terrain.txt`.
pub fn load_terrain_catalog(path: &Path) -> Result<TerrainCatalog, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
    parse_terrain_catalog(&content)
}

/// Parse a raw `00_terrain.txt` string.
pub fn parse_terrain_catalog(content: &str) -> Result<TerrainCatalog, String> {
    let block = parse(content);

    // Find the second top-level "terrain = { ... }" block (the per-index list).
    // The first top-level block is "categories"; the bottom one is "terrain".
    let terrain_block_val = block
        .get("terrain")
        .ok_or_else(|| "Missing top-level 'terrain' block".to_string())?;
    let terrain_block = match terrain_block_val {
        Value::Block(b) => b,
        _ => return Err("'terrain' is not a block".into()),
    };

    let mut by_index: HashMap<u8, TerrainEntry> = HashMap::new();

    for entry in &terrain_block.entries {
        let inner = match &entry.value {
            Value::Block(b) => b,
            _ => continue,
        };
        // type = <name>
        let type_name = inner.get_string("type").unwrap_or("unknown");
        // color = { N }
        let color_val = match inner.get("color") {
            Some(Value::Block(b)) => b,
            _ => continue,
        };
        let index_i64 = match color_val.values.first() {
            Some(Value::Integer(n)) => *n,
            _ => continue,
        };
        if !(0..=255).contains(&index_i64) {
            continue;
        }
        let index = index_i64 as u8;
        let perm_snow = inner.get_bool("perm_snow").unwrap_or(false);
        let atlas_idx = inner.get_int("texture").and_then(|n| {
            if (0..=15).contains(&n) {
                Some(n as u8)
            } else {
                None
            }
        });
        // Last write wins — vanilla file has duplicate keys (e.g. `desert` repeated).
        by_index.insert(
            index,
            TerrainEntry {
                category: TerrainCategory::parse(type_name),
                perm_snow,
                atlas_idx,
            },
        );
    }

    Ok(TerrainCatalog { by_index })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
categories = {
    plains = { color = { 255 129 66 } movement_cost = 1.0 }
    forest = { color = { 89 199 85 } movement_cost = 1.5 }
    mountain = { color = { 157 192 208 } movement_cost = 2.0 }
}

terrain = {
    terrain_0 = { type = plains   color = { 0 } texture = 1 }
    terrain_1 = { type = forest   color = { 1 } texture = 4 }
    terrain_6 = { type = mountain color = { 6 } texture = 11 }
    snow_16   = { type = mountain color = { 16 } texture = 11 perm_snow = yes }
}
"#;

    #[test]
    fn parses_basic_indexes() {
        let cat = parse_terrain_catalog(SAMPLE).unwrap();
        assert_eq!(cat.category_at(0), TerrainCategory::Plains);
        assert_eq!(cat.category_at(1), TerrainCategory::Forest);
        assert_eq!(cat.category_at(6), TerrainCategory::Mountain);
        assert_eq!(cat.category_at(16), TerrainCategory::Mountain);
    }

    #[test]
    fn perm_snow_flag() {
        let cat = parse_terrain_catalog(SAMPLE).unwrap();
        assert!(!cat.perm_snow_at(6));
        assert!(cat.perm_snow_at(16));
    }

    #[test]
    fn unknown_index_falls_through() {
        let cat = parse_terrain_catalog(SAMPLE).unwrap();
        assert_eq!(cat.category_at(99), TerrainCategory::Unknown);
        assert!(!cat.perm_snow_at(99));
    }

    #[test]
    fn category_water_classification() {
        assert!(TerrainCategory::Ocean.is_water());
        assert!(TerrainCategory::Lakes.is_water());
        assert!(TerrainCategory::WaterDeepOcean.is_water());
        assert!(!TerrainCategory::Mountain.is_water());
        assert!(!TerrainCategory::Plains.is_water());
    }

    #[test]
    fn atlas_idx_array_parsed() {
        let cat = parse_terrain_catalog(SAMPLE).unwrap();
        let arr = cat.atlas_idx_array();
        assert_eq!(arr[0], 1);
        assert_eq!(arr[1], 4);
        assert_eq!(arr[6], 11);
        assert_eq!(arr[2], 2);
    }

    #[test]
    fn full_shader_luts_keep_variant_metadata() {
        let cat = parse_terrain_catalog(SAMPLE).unwrap();
        let atlas = cat.atlas_idx_array_256();
        let flags = cat.terrain_flags_array_256();
        assert_eq!(atlas[16], 11);
        assert_eq!(flags[16] & 1, 1);
        assert_eq!(atlas[200], 8);
    }

    #[test]
    fn missing_terrain_block_errors() {
        let err = parse_terrain_catalog("categories = { plains = { } }").unwrap_err();
        assert!(err.contains("terrain"), "got: {err}");
    }
}
