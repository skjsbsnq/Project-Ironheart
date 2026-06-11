use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use hoi4_map::ProvinceMap;
use hoi4_render::sdf::chamfer_distance_field;
use hoi4_state::{StateId, World};

use super::VanillaRuntimeTargetInputs;

pub const MAP_EDGE_SDF_CHANNELS: usize = 4;

pub struct MapEdgeSdfCpuTarget {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

pub fn generate(inputs: VanillaRuntimeTargetInputs<'_>) -> MapEdgeSdfCpuTarget {
    let width = inputs.world.map.province_map.width;
    let height = inputs.world.map.province_map.height;
    let state_sdf = compute_state_sdf(inputs.world);
    MapEdgeSdfCpuTarget {
        width,
        height,
        data: pack_edge_sdfs(
            width,
            height,
            inputs.country_sdf,
            &state_sdf,
            inputs.province_sdf,
            inputs.coast_sdf,
        ),
    }
}

pub fn signature(world: &World) -> u64 {
    let mut h = DefaultHasher::new();
    world.map.province_map.width.hash(&mut h);
    world.map.province_map.height.hash(&mut h);
    world.provinces.controllers.len().hash(&mut h);
    for id in &world.provinces.controllers {
        id.0.hash(&mut h);
    }
    world.provinces.state_of.len().hash(&mut h);
    for id in &world.provinces.state_of {
        id.0.hash(&mut h);
    }
    h.finish()
}

pub fn compute_state_sdf(world: &World) -> Vec<u8> {
    compute_state_sdf_for_map(&world.map.province_map, &world.provinces.state_of)
}

fn compute_state_sdf_for_map(pmap: &ProvinceMap, state_of: &[StateId]) -> Vec<u8> {
    let w = pmap.width;
    let h = pmap.height;
    let pixels = &pmap.pixels;
    let state_id_of =
        |pid: u16| -> StateId { state_of.get(pid as usize).copied().unwrap_or(StateId::NONE) };

    chamfer_distance_field(w, h, |x, y| {
        let me = pixels[(y * w + x) as usize];
        let sa = state_id_of(me);
        if sa.is_none() {
            return false;
        }

        let neighbour_differs = |idx: usize| -> bool {
            let sb = state_id_of(pixels[idx]);
            !sb.is_none() && sb != sa
        };

        if x + 1 < w && neighbour_differs((y * w + x + 1) as usize) {
            return true;
        }
        if y + 1 < h && neighbour_differs(((y + 1) * w + x) as usize) {
            return true;
        }
        if x > 0 && neighbour_differs((y * w + x - 1) as usize) {
            return true;
        }
        if y > 0 && neighbour_differs(((y - 1) * w + x) as usize) {
            return true;
        }
        false
    })
}

fn pack_edge_sdfs(
    width: u32,
    height: u32,
    country_sdf: &[u8],
    state_sdf: &[u8],
    province_sdf: &[u8],
    coast_sdf: &[u8],
) -> Vec<u8> {
    let expected = width as usize * height as usize;
    let mut out = vec![255u8; expected * MAP_EDGE_SDF_CHANNELS];
    for i in 0..expected {
        let o = i * MAP_EDGE_SDF_CHANNELS;
        out[o] = source_value(country_sdf, i);
        out[o + 1] = source_value(state_sdf, i);
        out[o + 2] = source_value(province_sdf, i);
        out[o + 3] = source_value(coast_sdf, i);
    }
    out
}

fn source_value(source: &[u8], idx: usize) -> u8 {
    source.get(idx).copied().unwrap_or(255)
}

#[cfg(test)]
mod tests {
    use super::*;
    use hoi4_map::ProvinceMap;

    #[test]
    fn packed_edge_sdf_uses_stable_channel_semantics() {
        let data = pack_edge_sdfs(2, 1, &[0, 10], &[20, 30], &[40, 50], &[60, 70]);
        assert_eq!(data, vec![0, 20, 40, 60, 10, 30, 50, 70]);
    }

    #[test]
    fn missing_source_channels_fall_back_to_far_distance() {
        let data = pack_edge_sdfs(1, 1, &[], &[7], &[9], &[]);
        assert_eq!(data, vec![255, 7, 9, 255]);
    }

    #[test]
    fn state_sdf_marks_valid_state_boundaries_only() {
        let pmap = ProvinceMap {
            width: 4,
            height: 1,
            pixels: vec![1, 1, 2, 3],
        };
        let mut state_of = vec![StateId::NONE; 4];
        state_of[1] = StateId(1);
        state_of[2] = StateId(2);
        state_of[3] = StateId::NONE;

        let sdf = compute_state_sdf_for_map(&pmap, &state_of);
        assert_eq!(sdf[1], 0);
        assert_eq!(sdf[2], 0);
        assert!(sdf[3] > 0);
    }
}
