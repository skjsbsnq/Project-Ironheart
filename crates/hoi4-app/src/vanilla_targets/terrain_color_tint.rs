use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use hoi4_map::definition::ProvinceType;
use hoi4_render::map_mode::{color_lut_entry, MapMode};
use hoi4_state::{CountryId, ProvinceId, World};

use super::VanillaRuntimeTargetFrameParams;

pub const VANILLA_TERRAIN_COLOR_TINT_WIDTH: u32 = 2816;
pub const VANILLA_TERRAIN_COLOR_TINT_HEIGHT: u32 = 1024;

pub fn generate(world: &World, params: &VanillaRuntimeTargetFrameParams) -> Vec<u8> {
    let pmap = &world.map.province_map;
    let mut per_province = vec![[0u8; 4]; world.map.definitions.len().max(world.provinces.count)];

    for pid in 0..per_province.len() {
        let entry = color_lut_entry(world, params.map_mode, params.player_country, pid);
        per_province[pid] = [
            entry[0],
            entry[1],
            entry[2],
            city_emissive_alpha(world, ProvinceId(pid as u16)),
        ];
    }

    let mut data = vec![
        0u8;
        (VANILLA_TERRAIN_COLOR_TINT_WIDTH * VANILLA_TERRAIN_COLOR_TINT_HEIGHT * 4)
            as usize
    ];
    for y in 0..VANILLA_TERRAIN_COLOR_TINT_HEIGHT {
        let src_y = scaled_coord(y, VANILLA_TERRAIN_COLOR_TINT_HEIGHT, pmap.height);
        for x in 0..VANILLA_TERRAIN_COLOR_TINT_WIDTH {
            let src_x = scaled_coord(x, VANILLA_TERRAIN_COLOR_TINT_WIDTH, pmap.width);
            let src_idx = (src_y * pmap.width + src_x) as usize;
            let pid = pmap.pixels.get(src_idx).copied().unwrap_or(0) as usize;
            let rgba = per_province.get(pid).copied().unwrap_or([0, 0, 0, 0]);
            let o = ((y * VANILLA_TERRAIN_COLOR_TINT_WIDTH + x) * 4) as usize;
            // Texture format is BGRA8; shader sampling exposes this as RGBA.
            data[o] = rgba[2];
            data[o + 1] = rgba[1];
            data[o + 2] = rgba[0];
            data[o + 3] = rgba[3];
        }
    }
    data
}

pub fn signature(world: &World, params: &VanillaRuntimeTargetFrameParams) -> u64 {
    let mut h = DefaultHasher::new();
    map_mode_code(params.map_mode).hash(&mut h);
    params
        .player_country
        .map(|id| id.raw())
        .unwrap_or(CountryId::NONE.raw())
        .hash(&mut h);
    quantize_opacity(params.map_mode_overlay_opacity).hash(&mut h);

    for owner in &world.provinces.owners {
        owner.raw().hash(&mut h);
    }
    for controller in &world.provinces.controllers {
        controller.raw().hash(&mut h);
    }
    for supply in &world.provinces.supply {
        quantize_4k(*supply).hash(&mut h);
    }
    for state_idx in 0..world.states.count {
        world.states.owners[state_idx].raw().hash(&mut h);
        world.states.controllers[state_idx].raw().hash(&mut h);
        world.states.infrastructure[state_idx].hash(&mut h);
        world.states.manpower_pool[state_idx].hash(&mut h);
        quantize_4k(world.states.resistance[state_idx]).hash(&mut h);
        quantize_4k(world.states.compliance[state_idx]).hash(&mut h);
    }
    for party in &world.countries.ruling_party {
        party.hash(&mut h);
    }
    for state in &world.data.states {
        state.id.hash(&mut h);
        state.victory_points.hash(&mut h);
    }
    h.finish()
}

fn city_emissive_alpha(world: &World, province: ProvinceId) -> u8 {
    let pid = province.0 as usize;
    let Some(def) = world.map.definitions.get(pid).and_then(|def| def.as_ref()) else {
        return 0;
    };
    if def.province_type != ProvinceType::Land {
        return 0;
    }

    let mut intensity = if def.terrain == "urban" { 0.18 } else { 0.0 };
    let state_idx = world
        .provinces
        .state_of
        .get(pid)
        .copied()
        .filter(|state| !state.is_none())
        .map(|state| state.0 as usize);

    if let Some(state_idx) = state_idx {
        if state_idx < world.states.count {
            intensity += (world.states.infrastructure[state_idx] as f32 / 10.0) * 0.10;
            intensity +=
                (world.state_building_levels(hoi4_state::StateId(state_idx as u16)) as f32 / 40.0)
                    .clamp(0.0, 0.22);
        }
    }

    for state in &world.data.states {
        for &(vp_province, value) in &state.victory_points {
            if vp_province == province.0 {
                intensity += (value as f32 / 40.0).sqrt().clamp(0.0, 0.45);
            }
        }
    }

    (intensity.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn scaled_coord(coord: u32, dst_extent: u32, src_extent: u32) -> u32 {
    if src_extent == 0 || dst_extent == 0 {
        return 0;
    }
    ((coord as u64 * src_extent as u64) / dst_extent as u64)
        .min(src_extent.saturating_sub(1) as u64) as u32
}

fn map_mode_code(mode: MapMode) -> u8 {
    match mode {
        MapMode::Political => 0,
        MapMode::Terrain => 1,
        MapMode::Manpower => 2,
        MapMode::Factories => 3,
        MapMode::Cores => 4,
        MapMode::Infrastructure => 5,
        MapMode::Ideology => 6,
        MapMode::Supply => 7,
        MapMode::Resistance => 8,
    }
}

fn quantize_opacity(value: f32) -> u16 {
    (value.clamp(0.0, 1.0) * 1024.0).round() as u16
}

fn quantize_4k(value: f32) -> i32 {
    (value.clamp(-1_000_000.0, 1_000_000.0) * 4096.0).round() as i32
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, HashSet};
    use std::sync::Arc;

    use hoi4_data::{Color, Country, CountryTag, GameData, State};
    use hoi4_map::{
        GameMap, Heightmap, ProvinceDefinition, ProvinceMap, TerrainBitmap, TerrainCatalog,
    };
    use hoi4_state::World;

    fn test_world() -> World {
        let mut definitions = vec![None; 3];
        for id in 1..=2u16 {
            definitions[id as usize] = Some(ProvinceDefinition {
                id,
                r: id as u8,
                g: 0,
                b: 0,
                province_type: ProvinceType::Land,
                coastal: false,
                terrain: if id == 1 { "urban" } else { "plains" }.to_owned(),
                continent: 1,
            });
        }
        let map = Arc::new(GameMap {
            definitions,
            rgb_to_id: HashMap::new(),
            province_map: ProvinceMap {
                width: 2,
                height: 1,
                pixels: vec![1, 2],
            },
            adjacencies: vec![vec![], vec![2], vec![1]],
            special_adjacencies: Vec::new(),
            heightmap: Heightmap {
                width: 2,
                height: 1,
                pixels: vec![120, 120],
            },
            terrain_bmp: TerrainBitmap {
                width: 2,
                height: 1,
                pixels: vec![0, 0],
                palette: [[0; 3]; 256],
            },
            terrain_catalog: TerrainCatalog::default(),
            tree_definition_bmp: None,
            tree_indices: HashSet::new(),
        });
        let mut data = GameData::default();
        let tag = CountryTag::new("TST");
        data.countries.insert(
            tag.clone(),
            Country {
                tag: tag.clone(),
                color: Color {
                    r: 20,
                    g: 80,
                    b: 160,
                },
                graphical_culture: "western_european_gfx".to_owned(),
                capital: 1,
                ruling_party: "democratic".to_owned(),
                technologies: Vec::new(),
            },
        );
        data.states.push(State {
            id: 1,
            name: "STATE_TEST".to_owned(),
            manpower: 100,
            owner: tag.clone(),
            cores: vec![tag],
            provinces: vec![1, 2],
            category: "city".to_owned(),
            infrastructure: 8,
            victory_points: vec![(1, 10)],
            resources: Vec::new(),
        });
        World::new(map, Arc::new(data))
    }

    #[test]
    fn terrain_color_tint_uses_vanilla_dimensions_and_bgra_storage() {
        let world = test_world();
        let data = generate(&world, &VanillaRuntimeTargetFrameParams::default());
        assert_eq!(
            data.len(),
            (VANILLA_TERRAIN_COLOR_TINT_WIDTH * VANILLA_TERRAIN_COLOR_TINT_HEIGHT * 4) as usize
        );
        assert_eq!(&data[0..3], &[160, 80, 20]);
        assert!(data[3] > 0);
    }

    #[test]
    fn signature_tracks_controller_changes() {
        let mut world = test_world();
        let before = signature(&world, &VanillaRuntimeTargetFrameParams::default());
        world.provinces.controllers[1] = CountryId::NONE;
        assert_ne!(
            before,
            signature(&world, &VanillaRuntimeTargetFrameParams::default())
        );
    }
}
