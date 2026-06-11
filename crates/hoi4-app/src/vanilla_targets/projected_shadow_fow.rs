use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use hoi4_map::ProvinceType;
use hoi4_state::World;

use super::fow::{
    VANILLA_INTEL_MAP_HEIGHT, VANILLA_INTEL_MAP_WIDTH, VANILLA_PROJECTED_SHADOW_HEIGHT,
    VANILLA_PROJECTED_SHADOW_WIDTH,
};
use super::VanillaRuntimeTargetFrameParams;

pub const PRODUCER_STATUS: &str =
    "ShadowMap_ProjectFOW=runtime_generated_equivalent_projected_blur";
pub const PRODUCER_TRACE: &str = "reverse_out/exports/dynamic_target_lifecycle.tsv ShadowMap_projectedFOW; reverse_out/exports/tree_shadow_pipeline.tsv";
pub const PRODUCER_DETAIL: &str = "Runtime generated packed projected shadow/FOW target: order 77 tree/projected stamps tree/object occluders, order 78 terrainunlit/projected combines height, terrain, IntelMap, FOW, and SnowMudData, orders 79/80 run full-resolution horizontal/vertical blur";

#[derive(Debug, Clone)]
pub struct ProjectedShadowFowTargets {
    pub shadow_map: Vec<u8>,
    pub blur_temp: Vec<u8>,
}

pub fn generate(
    world: &World,
    params: &VanillaRuntimeTargetFrameParams,
    fow_data: &[u8],
    intel_map_data: &[u8],
    mud_snow_data: &[u8],
) -> ProjectedShadowFowTargets {
    let mut base = generate_projected_base(world, params, fow_data, intel_map_data, mud_snow_data);
    apply_tree_projected_stage(world, &mut base);
    let blur_temp = blur_horizontal(&base);
    let shadow_map = blur_vertical(&blur_temp);
    ProjectedShadowFowTargets {
        shadow_map,
        blur_temp,
    }
}

pub fn signature(world: &World, params: &VanillaRuntimeTargetFrameParams) -> u64 {
    let mut h = DefaultHasher::new();
    params
        .player_country
        .map(|country| country.raw())
        .unwrap_or(hoi4_state::CountryId::NONE.raw())
        .hash(&mut h);
    quantize_4k(params.season_snow_offset).hash(&mut h);
    quantize_4k(params.season_blend).hash(&mut h);
    world.date.year.hash(&mut h);
    world.date.month.hash(&mut h);
    world.date.day.hash(&mut h);
    world.map.province_map.width.hash(&mut h);
    world.map.province_map.height.hash(&mut h);
    world.map.heightmap.width.hash(&mut h);
    world.map.heightmap.height.hash(&mut h);
    for owner in &world.provinces.owners {
        owner.raw().hash(&mut h);
    }
    for controller in &world.provinces.controllers {
        controller.raw().hash(&mut h);
    }
    for px in world.map.heightmap.pixels.iter().step_by(251) {
        px.hash(&mut h);
    }
    if let Some(tree_bmp) = &world.map.tree_definition_bmp {
        tree_bmp.width.hash(&mut h);
        tree_bmp.height.hash(&mut h);
        for px in tree_bmp.pixels.iter().step_by(157) {
            px.hash(&mut h);
        }
        for idx in &world.map.tree_indices {
            idx.hash(&mut h);
        }
    }
    h.finish()
}

fn generate_projected_base(
    world: &World,
    params: &VanillaRuntimeTargetFrameParams,
    fow_data: &[u8],
    intel_map_data: &[u8],
    mud_snow_data: &[u8],
) -> Vec<u8> {
    let pmap = &world.map.province_map;
    let mut data =
        vec![0u8; (VANILLA_PROJECTED_SHADOW_WIDTH * VANILLA_PROJECTED_SHADOW_HEIGHT * 4) as usize];

    for y in 0..VANILLA_PROJECTED_SHADOW_HEIGHT {
        let src_y = scaled_coord(y, VANILLA_PROJECTED_SHADOW_HEIGHT, pmap.height);
        for x in 0..VANILLA_PROJECTED_SHADOW_WIDTH {
            let src_x = scaled_coord(x, VANILLA_PROJECTED_SHADOW_WIDTH, pmap.width);
            let pmap_idx = (src_y * pmap.width + src_x) as usize;
            let pid = pmap.pixels.get(pmap_idx).copied().unwrap_or(0) as usize;
            let def = world.map.definitions.get(pid).and_then(|def| def.as_ref());
            let height = sample_height(world, src_x, src_y);
            let terrain_shadow = terrainunlit_projected_shadow(def, height);
            let weather_shadow =
                snow_mud_shadow(src_x, src_y, pmap.width, pmap.height, mud_snow_data);
            let fow = fow_sample(src_x, src_y, pmap.width, fow_data);
            let intel = intel_sample(x, y, intel_map_data);
            let explored = fow[0] as f32 / 255.0;
            let visible = fow[1] as f32 / 255.0;
            let enemy_spotted = fow[2] as f32 / 255.0;
            let intel_factor = intel as f32 / 255.0;

            let projected_shadow =
                (terrain_shadow - weather_shadow * 0.08 - (1.0 - visible) * 0.12).clamp(0.0, 1.0);
            let visible_factor = (0.35 + visible * 0.65).clamp(0.0, 1.0);
            let explored_factor = (0.25 + explored.max(intel_factor) * 0.75).clamp(0.0, 1.0);
            let spotted_factor = enemy_spotted.max(intel_factor * 0.50);

            let o = ((y * VANILLA_PROJECTED_SHADOW_WIDTH + x) * 4) as usize;
            // BGRA storage sampled as RGBA by wgpu. Existing shaders use R for
            // projected shadow, G/B for visible/explored FOW factors.
            data[o] = to_u8(projected_shadow);
            data[o + 1] = to_u8(visible_factor);
            data[o + 2] = to_u8(explored_factor.max(spotted_factor * 0.6));
            data[o + 3] = 255;
        }
    }

    let _ = params;
    data
}

fn apply_tree_projected_stage(world: &World, data: &mut [u8]) {
    let Some(tree_bmp) = &world.map.tree_definition_bmp else {
        return;
    };
    if tree_bmp.width == 0 || tree_bmp.height == 0 {
        return;
    }

    for y in 0..VANILLA_PROJECTED_SHADOW_HEIGHT {
        let ty = scaled_coord(y, VANILLA_PROJECTED_SHADOW_HEIGHT, tree_bmp.height);
        for x in 0..VANILLA_PROJECTED_SHADOW_WIDTH {
            let tx = scaled_coord(x, VANILLA_PROJECTED_SHADOW_WIDTH, tree_bmp.width);
            let tree_idx = tree_bmp.get(tx, ty);
            if !world.map.tree_indices.contains(&tree_idx) {
                continue;
            }
            let o = ((y * VANILLA_PROJECTED_SHADOW_WIDTH + x) * 4) as usize;
            let species_bias = match tree_idx {
                7 => 0.82,
                10 => 0.86,
                _ => 0.78,
            };
            data[o] = ((data[o] as f32) * species_bias).round() as u8;
        }
    }
}

fn terrainunlit_projected_shadow(def: Option<&hoi4_map::ProvinceDefinition>, height: f32) -> f32 {
    let slope = height_slope(height);
    let terrain_bias = match def.map(|def| def.province_type) {
        Some(ProvinceType::Sea | ProvinceType::Lake) => 0.02,
        Some(ProvinceType::Land) => match def.map(|def| def.terrain.as_str()).unwrap_or_default() {
            "mountain" => 0.22,
            "hills" => 0.14,
            "forest" => 0.10,
            "jungle" => 0.12,
            "urban" => 0.06,
            _ => 0.04,
        },
        None => 0.0,
    };
    (1.0 - terrain_bias - slope * 0.36).clamp(0.35, 1.0)
}

fn height_slope(height: f32) -> f32 {
    ((height - 0.42).abs() * 1.35).clamp(0.0, 1.0)
}

fn sample_height(world: &World, x: u32, y: u32) -> f32 {
    let heightmap = &world.map.heightmap;
    if heightmap.width == 0 || heightmap.height == 0 {
        return 0.0;
    }
    let hx = scaled_coord(x, world.map.province_map.width, heightmap.width);
    let hy = scaled_coord(y, world.map.province_map.height, heightmap.height);
    heightmap.pixels[(hy * heightmap.width + hx) as usize] as f32 / 255.0
}

fn fow_sample(x: u32, y: u32, map_width: u32, fow_data: &[u8]) -> [u8; 4] {
    if map_width == 0 || fow_data.is_empty() {
        return [255, 255, 0, 255];
    }
    let idx = ((y * map_width + x) * 4) as usize;
    if idx + 3 >= fow_data.len() {
        return [255, 255, 0, 255];
    }
    [
        fow_data[idx],
        fow_data[idx + 1],
        fow_data[idx + 2],
        fow_data[idx + 3],
    ]
}

fn intel_sample(x: u32, y: u32, intel_map_data: &[u8]) -> u8 {
    if intel_map_data.len() != (VANILLA_INTEL_MAP_WIDTH * VANILLA_INTEL_MAP_HEIGHT) as usize {
        return 255;
    }
    let ix = scaled_coord(x, VANILLA_PROJECTED_SHADOW_WIDTH, VANILLA_INTEL_MAP_WIDTH);
    let iy = scaled_coord(y, VANILLA_PROJECTED_SHADOW_HEIGHT, VANILLA_INTEL_MAP_HEIGHT);
    intel_map_data[(iy * VANILLA_INTEL_MAP_WIDTH + ix) as usize]
}

fn snow_mud_shadow(x: u32, y: u32, map_width: u32, map_height: u32, mud_snow_data: &[u8]) -> f32 {
    let width = (map_width / 4).max(1);
    let height = (map_height / 4).max(1);
    if mud_snow_data.len() != (width * height * 4) as usize {
        return 0.0;
    }
    let sx = scaled_coord(x, map_width, width);
    let sy = scaled_coord(y, map_height, height);
    let o = ((sy * width + sx) * 4) as usize;
    let snow = mud_snow_data[o + 1].max(mud_snow_data[o]) as f32 / 255.0;
    let mud = mud_snow_data[o + 3].max(mud_snow_data[o + 2]) as f32 / 255.0;
    (snow * 0.35 + mud * 0.18).clamp(0.0, 1.0)
}

fn blur_horizontal(src: &[u8]) -> Vec<u8> {
    blur(src, 1, 0)
}

fn blur_vertical(src: &[u8]) -> Vec<u8> {
    blur(src, 0, 1)
}

fn blur(src: &[u8], dx: i32, dy: i32) -> Vec<u8> {
    let width = VANILLA_PROJECTED_SHADOW_WIDTH as i32;
    let height = VANILLA_PROJECTED_SHADOW_HEIGHT as i32;
    let mut dst = vec![0u8; src.len()];
    for y in 0..height {
        for x in 0..width {
            let o = ((y * width + x) * 4) as usize;
            for c in 0..4 {
                let mut acc = 0.0;
                let mut weight_sum = 0.0;
                for (tap, weight) in [(-2, 1.0), (-1, 4.0), (0, 6.0), (1, 4.0), (2, 1.0)] {
                    let sx = (x + dx * tap).clamp(0, width - 1);
                    let sy = (y + dy * tap).clamp(0, height - 1);
                    let so = ((sy * width + sx) * 4) as usize + c;
                    acc += src[so] as f32 * weight;
                    weight_sum += weight;
                }
                dst[o + c] = (acc / weight_sum).round() as u8;
            }
        }
    }
    dst
}

fn scaled_coord(coord: u32, dst_extent: u32, src_extent: u32) -> u32 {
    if src_extent == 0 || dst_extent == 0 {
        return 0;
    }
    ((coord as u64 * src_extent as u64) / dst_extent as u64)
        .min(src_extent.saturating_sub(1) as u64) as u32
}

fn to_u8(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
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
        TreeBitmap,
    };
    use hoi4_state::World;

    fn test_world(with_trees: bool) -> World {
        let mut definitions = vec![None; 4];
        for (id, terrain, province_type) in [
            (1u16, "plains", ProvinceType::Land),
            (2u16, "mountain", ProvinceType::Land),
            (3u16, "sea", ProvinceType::Sea),
        ] {
            definitions[id as usize] = Some(ProvinceDefinition {
                id,
                r: id as u8,
                g: 0,
                b: 0,
                province_type,
                coastal: province_type == ProvinceType::Sea,
                terrain: terrain.to_owned(),
                continent: 1,
            });
        }
        let tree_definition_bmp = with_trees.then(|| TreeBitmap {
            width: 3,
            height: 2,
            pixels: vec![254, 3, 254, 254, 7, 254],
            palette: [[0; 3]; 256],
        });
        let map = Arc::new(GameMap {
            definitions,
            rgb_to_id: HashMap::new(),
            province_map: ProvinceMap {
                width: 3,
                height: 2,
                pixels: vec![1, 2, 3, 1, 2, 3],
            },
            adjacencies: vec![vec![], vec![2], vec![1, 3], vec![2]],
            special_adjacencies: Vec::new(),
            heightmap: Heightmap {
                width: 3,
                height: 2,
                pixels: vec![100, 230, 20, 105, 220, 20],
            },
            terrain_bmp: TerrainBitmap {
                width: 3,
                height: 2,
                pixels: vec![0; 6],
                palette: [[0; 3]; 256],
            },
            terrain_catalog: TerrainCatalog::default(),
            tree_definition_bmp,
            tree_indices: [3u8, 4, 7, 10].into_iter().collect::<HashSet<_>>(),
        });
        let mut data = GameData::default();
        for tag in ["AAA", "BBB"] {
            let country_tag = CountryTag::new(tag);
            data.countries.insert(
                country_tag.clone(),
                Country {
                    tag: country_tag,
                    color: Color {
                        r: 80,
                        g: 90,
                        b: 100,
                    },
                    graphical_culture: "western_european_gfx".to_owned(),
                    capital: 1,
                    ruling_party: "neutrality".to_owned(),
                    technologies: Vec::new(),
                },
            );
        }
        data.states.push(State {
            id: 1,
            name: "A".to_owned(),
            manpower: 0,
            owner: CountryTag::new("AAA"),
            cores: vec![CountryTag::new("AAA")],
            provinces: vec![1],
            category: String::new(),
            infrastructure: 5,
            victory_points: Vec::new(),
            resources: Vec::new(),
        });
        World::new(map, Arc::new(data))
    }

    #[test]
    fn projected_shadow_fow_is_full_res_and_state_driven() {
        let world = test_world(false);
        let fow = vec![255, 255, 0, 255, 200, 160, 0, 255, 120, 80, 0, 255].repeat(2);
        let intel = vec![180; (VANILLA_INTEL_MAP_WIDTH * VANILLA_INTEL_MAP_HEIGHT) as usize];
        let mud_snow = vec![
            0;
            ((world.map.province_map.width / 4).max(1)
                * (world.map.province_map.height / 4).max(1)
                * 4) as usize
        ];
        let targets = generate(
            &world,
            &VanillaRuntimeTargetFrameParams::default(),
            &fow,
            &intel,
            &mud_snow,
        );
        let expected =
            (VANILLA_PROJECTED_SHADOW_WIDTH * VANILLA_PROJECTED_SHADOW_HEIGHT * 4) as usize;
        assert_eq!(targets.shadow_map.len(), expected);
        assert_eq!(targets.blur_temp.len(), expected);
        assert!(targets.shadow_map.chunks_exact(4).any(|px| px[0] < 255));
        assert!(targets.shadow_map.chunks_exact(4).any(|px| px[1] < 255));
    }

    #[test]
    fn tree_projected_stage_darkens_tree_pixels_before_blur() {
        let world_without_trees = test_world(false);
        let world_with_trees = test_world(true);
        let fow = vec![
            255;
            world_without_trees.map.province_map.width as usize
                * world_without_trees.map.province_map.height as usize
                * 4
        ];
        let intel = vec![255; (VANILLA_INTEL_MAP_WIDTH * VANILLA_INTEL_MAP_HEIGHT) as usize];
        let mud_snow = vec![0; 4];
        let without = generate(
            &world_without_trees,
            &VanillaRuntimeTargetFrameParams::default(),
            &fow,
            &intel,
            &mud_snow,
        );
        let with = generate(
            &world_with_trees,
            &VanillaRuntimeTargetFrameParams::default(),
            &fow,
            &intel,
            &mud_snow,
        );
        let without_sum: u64 = without
            .shadow_map
            .iter()
            .step_by(4)
            .map(|v| *v as u64)
            .sum();
        let with_sum: u64 = with.shadow_map.iter().step_by(4).map(|v| *v as u64).sum();
        assert!(with_sum < without_sum);
    }
}
