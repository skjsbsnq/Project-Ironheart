use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use hoi4_map::Heightmap;
use hoi4_render::railways::compute_province_centroids;
use hoi4_state::{BuildingKind, ProvinceId, StateId, World};

use super::VanillaRuntimeTargetInputs;

pub const MAX_POINT_LIGHTS: usize = 192;
pub const LIGHT_DATA_TEXELS_PER_LIGHT: u32 = 2;
pub const LIGHT_INDEX_TILE_PX: u32 = 16;
pub const LIGHT_INDEX_SENTINEL: u8 = 255;

#[derive(Debug, Clone)]
pub struct PointLightTargetData {
    pub light_data: Vec<u8>,
    pub light_index: Vec<u8>,
    pub light_data_width: u32,
    pub light_index_width: u32,
    pub light_index_height: u32,
    pub light_count: usize,
}

#[derive(Debug, Clone)]
struct CpuPointLight {
    province_id: u16,
    map_px: [f32; 2],
    world_pos: [f32; 3],
    radius: f32,
    color: [f32; 3],
    falloff: f32,
    priority: f32,
}

pub fn generate(inputs: VanillaRuntimeTargetInputs<'_>) -> PointLightTargetData {
    let centroids = compute_province_centroids(&inputs.world.map.province_map);
    generate_with_centroids(inputs, &centroids)
}

pub fn generate_with_centroids(
    inputs: VanillaRuntimeTargetInputs<'_>,
    centroids: &[(f32, f32)],
) -> PointLightTargetData {
    let width = inputs.world.map.province_map.width;
    let height = inputs.world.map.province_map.height;
    let index_width = align_to(width.div_ceil(LIGHT_INDEX_TILE_PX).max(1), 64);
    let index_height = height.div_ceil(LIGHT_INDEX_TILE_PX).max(1);
    generate_with_dimensions(
        inputs.world,
        centroids,
        inputs.world_scale,
        inputs.height_scale,
        index_width,
        index_height,
    )
}

pub fn signature(world: &World) -> u64 {
    let mut hasher = DefaultHasher::new();
    world.data.states.len().hash(&mut hasher);
    for state in &world.data.states {
        state.id.hash(&mut hasher);
        state.infrastructure.hash(&mut hasher);
        state.victory_points.hash(&mut hasher);
    }

    world
        .countries
        .buildings_v6
        .buildings
        .len()
        .hash(&mut hasher);
    for building in &world.countries.buildings_v6.buildings {
        building.state.0.hash(&mut hasher);
        building.level.hash(&mut hasher);
        building.kind.hash(&mut hasher);
        building.building_def_id.hash(&mut hasher);
    }

    world.divisions.count.hash(&mut hasher);
    for idx in 0..world.divisions.count {
        if !world.divisions.in_combat[idx] {
            continue;
        }
        idx.hash(&mut hasher);
        world.divisions.locations[idx].0.hash(&mut hasher);
        world.divisions.strength[idx].to_bits().hash(&mut hasher);
    }
    hasher.finish()
}

fn generate_with_dimensions(
    world: &World,
    centroids: &[(f32, f32)],
    world_scale: f32,
    height_scale: f32,
    index_width: u32,
    index_height: u32,
) -> PointLightTargetData {
    let mut lights = collect_point_lights(world, centroids, world_scale, height_scale);
    lights.sort_by(|a, b| {
        b.priority
            .total_cmp(&a.priority)
            .then_with(|| a.province_id.cmp(&b.province_id))
    });
    lights.truncate(MAX_POINT_LIGHTS);

    let light_data_width = MAX_POINT_LIGHTS as u32 * LIGHT_DATA_TEXELS_PER_LIGHT;
    let mut light_texels = vec![[0.0f32; 4]; light_data_width as usize];
    for (idx, light) in lights.iter().enumerate() {
        let base = idx * LIGHT_DATA_TEXELS_PER_LIGHT as usize;
        light_texels[base] = [
            light.world_pos[0],
            light.world_pos[1],
            light.world_pos[2],
            light.radius,
        ];
        light_texels[base + 1] = [
            light.color[0],
            light.color[1],
            light.color[2],
            light.falloff,
        ];
    }
    let light_data = bytemuck::cast_slice(&light_texels).to_vec();

    let mut light_index =
        vec![LIGHT_INDEX_SENTINEL; index_width as usize * index_height as usize * 4];
    for (idx, light) in lights.iter().enumerate() {
        write_light_index(
            idx as u8,
            light,
            index_width,
            index_height,
            world_scale,
            &mut light_index,
        );
    }

    PointLightTargetData {
        light_data,
        light_index,
        light_data_width,
        light_index_width: index_width,
        light_index_height: index_height,
        light_count: lights.len(),
    }
}

fn collect_point_lights(
    world: &World,
    centroids: &[(f32, f32)],
    world_scale: f32,
    height_scale: f32,
) -> Vec<CpuPointLight> {
    let mut lights = Vec::new();

    for state in &world.data.states {
        for &(province_id, value) in &state.victory_points {
            if value == 0 {
                continue;
            }
            if let Some(world_pos) = province_world_pos(
                world,
                centroids,
                ProvinceId(province_id),
                world_scale,
                height_scale,
            ) {
                let value_f = value as f32;
                lights.push(CpuPointLight {
                    province_id,
                    map_px: [world_pos[0] / world_scale, world_pos[2] / world_scale],
                    world_pos: [world_pos[0], world_pos[1] + 0.12, world_pos[2]],
                    radius: 0.42 + value_f.sqrt() * 0.09,
                    color: [
                        0.95 + value_f.min(20.0) * 0.010,
                        0.62 + value_f.min(20.0) * 0.006,
                        0.34,
                    ],
                    falloff: 0.28,
                    priority: 200.0 + value_f,
                });
            }
        }
    }

    for (idx, building) in world.countries.buildings_v6.buildings.iter().enumerate() {
        if building.level == 0 || building.state.is_none() {
            continue;
        }
        let Some(world_pos) =
            state_world_pos(world, centroids, building.state, world_scale, height_scale)
        else {
            continue;
        };
        let id = building.building_def_id.as_str();
        let level = building.level as f32;
        let (offset, color, radius, priority) = if is_port_light(id) {
            (
                [0.12, 0.10],
                [0.45, 0.70, 1.0],
                0.32 + level * 0.030,
                150.0 + level,
            )
        } else if is_air_light(id) {
            (
                [-0.10, -0.08],
                [0.62, 0.75, 1.0],
                0.28 + level * 0.025,
                130.0 + level,
            )
        } else if building.kind == BuildingKind::Military {
            (
                [0.0, 0.08],
                [0.82, 0.48, 0.35],
                0.25 + level * 0.020,
                95.0 + level,
            )
        } else if id == "industrial_complex" || id == "arms_factory" || id == "shipyard" {
            (
                [-0.08, 0.0],
                [0.78, 0.60, 0.42],
                0.24 + level * 0.020,
                80.0 + level,
            )
        } else {
            continue;
        };

        let px = world_pos[0] / world_scale + offset[0] / world_scale;
        let py = world_pos[2] / world_scale + offset[1] / world_scale;
        lights.push(CpuPointLight {
            province_id: state_anchor_province(world, building.state)
                .unwrap_or(ProvinceId(idx as u16))
                .0,
            map_px: [px, py],
            world_pos: [
                world_pos[0] + offset[0],
                world_pos[1] + 0.10,
                world_pos[2] + offset[1],
            ],
            radius,
            color,
            falloff: 0.24,
            priority,
        });
    }

    for idx in 0..world.divisions.count {
        if !world.divisions.in_combat[idx] {
            continue;
        }
        let province = world.divisions.locations[idx];
        if let Some(world_pos) =
            province_world_pos(world, centroids, province, world_scale, height_scale)
        {
            let strength = world.divisions.strength[idx].clamp(0.0, 1.0);
            lights.push(CpuPointLight {
                province_id: province.0,
                map_px: [world_pos[0] / world_scale, world_pos[2] / world_scale],
                world_pos: [world_pos[0], world_pos[1] + 0.16, world_pos[2]],
                radius: 0.46 + strength * 0.16,
                color: [1.0, 0.34, 0.16],
                falloff: 0.20,
                priority: 180.0 + idx as f32 * 0.001,
            });
        }
    }

    lights
}

fn write_light_index(
    light_idx: u8,
    light: &CpuPointLight,
    index_width: u32,
    index_height: u32,
    world_scale: f32,
    light_index: &mut [u8],
) {
    let radius_px = light.radius / world_scale.max(0.0001);
    let min_tx = ((light.map_px[0] - radius_px) / LIGHT_INDEX_TILE_PX as f32)
        .floor()
        .max(0.0) as u32;
    let max_tx = ((light.map_px[0] + radius_px) / LIGHT_INDEX_TILE_PX as f32)
        .ceil()
        .min(index_width.saturating_sub(1) as f32) as u32;
    let min_ty = ((light.map_px[1] - radius_px) / LIGHT_INDEX_TILE_PX as f32)
        .floor()
        .max(0.0) as u32;
    let max_ty = ((light.map_px[1] + radius_px) / LIGHT_INDEX_TILE_PX as f32)
        .ceil()
        .min(index_height.saturating_sub(1) as f32) as u32;

    for ty in min_ty..=max_ty {
        for tx in min_tx..=max_tx {
            let base = ((ty * index_width + tx) * 4) as usize;
            let slot = &mut light_index[base..base + 4];
            if slot.contains(&light_idx) {
                continue;
            }
            if let Some(empty) = slot
                .iter_mut()
                .find(|value| **value == LIGHT_INDEX_SENTINEL)
            {
                *empty = light_idx;
            }
        }
    }
}

fn is_port_light(id: &str) -> bool {
    matches!(
        id,
        "naval_base" | "v6_naval_base" | "port" | "shipyard" | "dockyard"
    )
}

fn is_air_light(id: &str) -> bool {
    matches!(id, "air_base" | "airbase" | "v6_air_base" | "airport")
}

fn align_to(value: u32, alignment: u32) -> u32 {
    if alignment == 0 {
        return value;
    }
    value.div_ceil(alignment) * alignment
}

fn state_world_pos(
    world: &World,
    centroids: &[(f32, f32)],
    state: StateId,
    world_scale: f32,
    height_scale: f32,
) -> Option<[f32; 3]> {
    let province = state_anchor_province(world, state)?;
    province_world_pos(world, centroids, province, world_scale, height_scale)
}

fn state_anchor_province(world: &World, state: StateId) -> Option<ProvinceId> {
    let state_idx = state.0 as usize;
    world.states.provinces.get(state_idx)?.first().copied()
}

fn province_world_pos(
    world: &World,
    centroids: &[(f32, f32)],
    province: ProvinceId,
    world_scale: f32,
    height_scale: f32,
) -> Option<[f32; 3]> {
    let pid = province.0 as usize;
    let (px, py) = *centroids.get(pid)?;
    if px == 0.0 && py == 0.0 {
        return None;
    }
    let heightmap = &world.map.heightmap;
    let xi = (px as u32).min(heightmap.width.saturating_sub(1));
    let yi = (py as u32).min(heightmap.height.saturating_sub(1));
    let raw_h = heightmap.get(xi, yi);
    let base_h = raw_h.max(Heightmap::SEA_LEVEL) as f32 / 255.0;
    Some([px * world_scale, base_h * height_scale, py * world_scale])
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    use hoi4_data::{Color, Country, CountryTag, GameData, State};
    use hoi4_map::{GameMap, ProvinceMap, TerrainBitmap, TerrainCatalog};
    use hoi4_state::{Building, BuildingOwner, CountryId, World};

    fn test_world() -> World {
        let map = Arc::new(GameMap {
            definitions: vec![None; 4],
            rgb_to_id: Default::default(),
            province_map: ProvinceMap {
                width: 4,
                height: 4,
                pixels: vec![
                    1, 1, 2, 2, //
                    1, 1, 2, 2, //
                    3, 3, 3, 3, //
                    3, 3, 3, 3,
                ],
            },
            adjacencies: vec![Vec::new(); 4],
            special_adjacencies: Vec::new(),
            heightmap: Heightmap {
                width: 4,
                height: 4,
                pixels: vec![120; 16],
            },
            terrain_bmp: TerrainBitmap {
                width: 4,
                height: 4,
                pixels: vec![0; 16],
                palette: [[0; 3]; 256],
            },
            terrain_catalog: TerrainCatalog::default(),
            tree_definition_bmp: None,
            tree_indices: Default::default(),
        });
        let mut data = GameData::default();
        data.countries.insert(
            CountryTag::new("TST"),
            Country {
                tag: CountryTag::new("TST"),
                color: Color {
                    r: 128,
                    g: 128,
                    b: 128,
                },
                graphical_culture: "western_european_gfx".to_string(),
                capital: 1,
                ruling_party: "neutrality".to_string(),
                technologies: Vec::new(),
            },
        );
        data.states.push(State {
            id: 1,
            name: "STATE_TEST".to_string(),
            manpower: 0,
            owner: CountryTag::new("TST"),
            cores: vec![CountryTag::new("TST")],
            provinces: vec![1, 2],
            category: String::new(),
            infrastructure: 5,
            victory_points: vec![(1, 10)],
            resources: Vec::new(),
        });

        let mut world = World::new(map, Arc::new(data));
        world.countries.buildings_v6.buildings.push(Building {
            kind: BuildingKind::Military,
            building_def_id: "air_base".to_string(),
            state: StateId(0),
            level: 2,
            owner: BuildingOwner::State,
            ownership_shares: Building::default_ownership_shares(
                BuildingOwner::State,
                CountryId(0),
            ),
            ..Building::runtime_defaults()
        });
        world.divisions.push(
            CountryId(0),
            ProvinceId(2),
            0,
            10.0,
            10.0,
            "test".to_string(),
        );
        world.divisions.in_combat[0] = true;
        world
    }

    #[test]
    fn light_targets_have_fixed_data_and_tiled_index() {
        let world = test_world();
        let inputs = VanillaRuntimeTargetInputs {
            world: &world,
            country_sdf: &[],
            province_sdf: &[],
            coast_sdf: &[],
            world_scale: 0.02,
            height_scale: 1.45,
            default_map_mode_code: 0,
        };
        let data = generate(inputs);
        assert_eq!(data.light_data_width, MAX_POINT_LIGHTS as u32 * 2);
        assert_eq!(data.light_data.len(), data.light_data_width as usize * 16);
        assert_eq!(data.light_index_width, 64);
        assert_eq!(data.light_index_height, 1);
        assert_eq!(data.light_index.len(), 64 * 4);
        assert!(data.light_count >= 2);
        assert!(data
            .light_index
            .iter()
            .any(|&idx| idx != LIGHT_INDEX_SENTINEL));
    }

    #[test]
    fn signature_changes_when_combat_state_changes() {
        let mut world = test_world();
        let before = signature(&world);
        world.divisions.in_combat[0] = false;
        assert_ne!(before, signature(&world));
    }
}
