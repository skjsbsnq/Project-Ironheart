pub mod adjacency;
pub mod definition;
pub mod provinces;
pub mod rivers;
pub mod seasons;
pub mod terrain;
pub mod terrain_bmp;
pub mod terrain_catalog;
pub mod trees_bmp;

use std::collections::{HashMap, HashSet};
use std::path::Path;

use hoi4_paths::PathConfig;

pub use adjacency::Adjacency;
pub use definition::{ProvinceDefinition, ProvinceType};
pub use provinces::ProvinceMap;
pub use rivers::{load_rivers_bmp, parse_rivers_bmp, RiverBitmap};
pub use seasons::{
    load_seasons_txt, parse_seasons_txt, SeasonResult, SeasonsTxt, TreeSeasonRange, SEASON_COLUMNS,
};
pub use terrain::{load_heightmap, Heightmap};
pub use terrain_bmp::{load_terrain_bmp, TerrainBitmap};
pub use terrain_catalog::{load_terrain_catalog, TerrainCatalog, TerrainCategory, TerrainEntry};
pub use trees_bmp::{load_trees_bmp, parse_trees_bmp, TreeBitmap, DEFAULT_TREE_INDICES};

/// Complete loaded map data
pub struct GameMap {
    /// Province definitions indexed by province ID
    pub definitions: Vec<Option<ProvinceDefinition>>,
    /// RGB → Province ID lookup
    pub rgb_to_id: HashMap<(u8, u8, u8), u16>,
    /// Province pixel map (width × height, each pixel = province_id)
    pub province_map: ProvinceMap,
    /// Adjacency list: province_id → list of neighbor province_ids
    pub adjacencies: Vec<Vec<u16>>,
    /// Special adjacencies (straits, canals)
    pub special_adjacencies: Vec<Adjacency>,
    /// Greyscale heightmap (terrain elevation), same dims as province_map.
    pub heightmap: Heightmap,
    /// Per-pixel terrain category index (from terrain.bmp).
    pub terrain_bmp: TerrainBitmap,
    /// Catalog mapping `terrain.bmp` palette indices → categories
    /// (parsed from `common/terrain/00_terrain.txt`).
    pub terrain_catalog: TerrainCatalog,
    /// Per-pixel "this is a tree" map (vanilla 1650×600). When the file is
    /// missing or fails to parse, this is `None` and the renderer falls back
    /// to terrain-based placement.
    pub tree_definition_bmp: Option<TreeBitmap>,
    /// Active tree palette indices declared by `default.map`'s
    /// `tree = { 3 4 7 10 }` block. Defaults to vanilla 3/4/7/10 when the
    /// block is missing.
    pub tree_indices: HashSet<u8>,
}

impl GameMap {
    /// Load all map data through the unified path resolver.
    pub fn load_from_paths(paths: &PathConfig) -> Result<Self, String> {
        let required = |relative: &str| {
            paths
                .find(relative)
                .ok_or_else(|| format!("missing required map file `{relative}`"))
        };

        // 1. Parse definition.csv
        let def_path = required("map/definition.csv")?;
        let (definitions, rgb_to_id, max_id) = definition::load_definitions(&def_path)?;

        // 2. Load provinces.bmp -> province index map
        let bmp_path = required("map/provinces.bmp")?;
        let province_map = provinces::load_province_map(&bmp_path, &rgb_to_id)?;

        // 3. Build adjacency from pixel neighbors
        let mut adjacencies = vec![Vec::new(); max_id as usize + 1];
        province_map.build_adjacency(&mut adjacencies);

        // 4. Parse special adjacencies (straits, canals)
        let adj_path = required("map/adjacencies.csv")?;
        let special_adjacencies = adjacency::load_adjacencies(&adj_path)?;

        for adj in &special_adjacencies {
            if (adj.from as usize) < adjacencies.len() && (adj.to as usize) < adjacencies.len() {
                if !adjacencies[adj.from as usize].contains(&adj.to) {
                    adjacencies[adj.from as usize].push(adj.to);
                }
                if !adjacencies[adj.to as usize].contains(&adj.from) {
                    adjacencies[adj.to as usize].push(adj.from);
                }
            }
        }

        // 5. Load heightmap.bmp (8-bit greyscale, same dims as province_map)
        let height_path = required("map/heightmap.bmp")?;
        let heightmap = terrain::load_heightmap(&height_path)?;

        // 6. Load terrain.bmp (8-bit indexed, terrain category index per pixel)
        let terrain_path = required("map/terrain.bmp")?;
        let terrain_bmp = terrain_bmp::load_terrain_bmp(&terrain_path)?;

        // 7. Optional terrain catalogue.
        let terrain_catalog =
            if let Some(catalog_path) = paths.find("common/terrain/00_terrain.txt") {
                terrain_catalog::load_terrain_catalog(&catalog_path)?
            } else {
                terrain_catalog::TerrainCatalog::default()
            };

        // 8. Parse `map/default.map` for tree_definition path + active tree indices.
        let (tree_def_filename, tree_indices) =
            if let Some(default_map_path) = paths.find("map/default.map") {
                parse_default_map_tree_block(&default_map_path)
            } else {
                (None, default_tree_indices())
            };

        // 9. Load `map/<tree_def_filename>` if available.
        let tree_def_filename = tree_def_filename.unwrap_or_else(|| "trees.bmp".to_string());
        let trees_bmp_rel = format!("map/{tree_def_filename}");
        let tree_definition_bmp = if let Some(trees_bmp_path) = paths.find(&trees_bmp_rel) {
            match trees_bmp::load_trees_bmp(&trees_bmp_path) {
                Ok(b) => Some(b),
                Err(e) => {
                    eprintln!("[map] failed to load {}: {}", trees_bmp_path.display(), e);
                    None
                }
            }
        } else {
            None
        };

        Ok(Self {
            definitions,
            rgb_to_id,
            province_map,
            adjacencies,
            special_adjacencies,
            heightmap,
            terrain_bmp,
            terrain_catalog,
            tree_definition_bmp,
            tree_indices,
        })
    }

    /// Load all map data from the game directory.
    pub fn load(game_path: &Path) -> Result<Self, String> {
        Self::load_from_paths(&PathConfig::with_game_path(game_path))
    }

    pub fn province_count(&self) -> usize {
        self.definitions.iter().filter(|d| d.is_some()).count()
    }

    pub fn get_province(&self, id: u16) -> Option<&ProvinceDefinition> {
        self.definitions.get(id as usize)?.as_ref()
    }

    pub fn neighbors(&self, id: u16) -> &[u16] {
        self.adjacencies
            .get(id as usize)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }
}

// ─── default.map helpers ───────────────────────────────────────────────────────

/// Vanilla `tree = { 3 4 7 10 }` set, used when the file is missing or its
/// `tree` block fails to parse.
fn default_tree_indices() -> HashSet<u8> {
    DEFAULT_TREE_INDICES.iter().copied().collect()
}

/// Parse `map/default.map` for two fields:
/// 1. `tree_definition = "trees.bmp"` — the BMP filename to load relative to
///    `map/`.
/// 2. `tree = { 3 4 7 10 }` — palette indices that count as "tree" pixels.
///
/// Both have safe fallbacks; this function never errors. On parse failure or
/// missing fields it returns `(None, default_tree_indices())`.
fn parse_default_map_tree_block(path: &Path) -> (Option<String>, HashSet<u8>) {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("[map] cannot read {}: {}", path.display(), e);
            return (None, default_tree_indices());
        }
    };
    // default.map is plain Latin-1/UTF-8; lossy decode is fine for the two
    // ASCII keys we care about.
    let text = String::from_utf8_lossy(&bytes);
    parse_default_map_tree_block_str(&text)
}

/// Inner parser that operates on a `&str` — keeps the file IO out of unit
/// tests so they can run in parallel without temp-file collisions.
fn parse_default_map_tree_block_str(text: &str) -> (Option<String>, HashSet<u8>) {
    let block = clausewitz_parser::parse(text);

    let tree_def = block.get_string("tree_definition").map(|s| s.to_string());

    let mut indices: HashSet<u8> = HashSet::new();
    if let Some(tree_block) = block.get_block("tree") {
        for v in &tree_block.values {
            if let clausewitz_parser::Value::Integer(i) = v {
                if (0..=255).contains(i) {
                    indices.insert(*i as u8);
                }
            }
        }
    }
    if indices.is_empty() {
        indices = default_tree_indices();
    }
    (tree_def, indices)
}

#[cfg(test)]
mod default_map_tests {
    use super::*;

    #[test]
    fn extracts_tree_definition_and_indices() {
        let (def, idx) = parse_default_map_tree_block_str(
            r#"
definitions = "definition.csv"
provinces = "provinces.bmp"
tree_definition = "trees.bmp"
tree = { 3 4 7 10 }
"#,
        );
        assert_eq!(def.as_deref(), Some("trees.bmp"));
        assert_eq!(idx.len(), 4);
        for v in [3, 4, 7, 10] {
            assert!(idx.contains(&v));
        }
    }

    #[test]
    fn falls_back_when_block_missing() {
        let (def, idx) = parse_default_map_tree_block_str("definitions = \"definition.csv\"\n");
        assert!(
            def.is_none(),
            "tree_definition not declared → def should be None"
        );
        assert_eq!(idx, default_tree_indices());
    }

    #[test]
    fn falls_back_when_tree_block_empty() {
        let (_, idx) = parse_default_map_tree_block_str("tree = { }\n");
        assert_eq!(idx, default_tree_indices());
    }

    #[test]
    fn ignores_out_of_byte_range_indices() {
        let (_, idx) = parse_default_map_tree_block_str("tree = { 3 999 -1 7 }\n");
        assert!(idx.contains(&3));
        assert!(idx.contains(&7));
        // 999 wraps would land on 231; -1 wraps to 255. Neither should
        // be present because the parser rejects values outside [0, 255].
        assert!(!idx.contains(&231));
        assert!(!idx.contains(&255));
    }
}
