use hoi4_map::ProvinceType;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use hoi4_render::sdf::{chamfer_distance_field, compute_country_sdf};
use hoi4_state::{CountryId, StateId, World};

use super::VanillaRuntimeTargetInputs;

pub struct GradientBorderCpuTargets {
    pub ch1: Vec<u8>,
    pub ch2: Vec<u8>,
    pub ch3: Vec<u8>,
}

pub fn generate(inputs: VanillaRuntimeTargetInputs<'_>) -> GradientBorderCpuTargets {
    let width = inputs.world.map.province_map.width;
    let height = inputs.world.map.province_map.height;
    GradientBorderCpuTargets {
        ch1: copy_sdf_or_far(inputs.country_sdf, width, height),
        ch2: copy_sdf_or_far(inputs.province_sdf, width, height),
        ch3: compute_channel3_distance(inputs.world, inputs.coast_sdf),
    }
}

pub fn generate_country_channel(world: &World) -> Vec<u8> {
    compute_country_sdf(&world.map.province_map, &world.provinces.controllers)
}

pub fn country_signature(world: &World) -> u64 {
    controllers_signature(&world.provinces.controllers)
}

fn controllers_signature(controllers: &[CountryId]) -> u64 {
    let mut h = DefaultHasher::new();
    controllers.len().hash(&mut h);
    for controller in controllers {
        controller.raw().hash(&mut h);
    }
    h.finish()
}

fn copy_sdf_or_far(source: &[u8], width: u32, height: u32) -> Vec<u8> {
    let expected = width as usize * height as usize;
    if source.len() == expected {
        source.to_vec()
    } else {
        vec![255; expected]
    }
}

fn compute_channel3_distance(world: &World, coast_sdf: &[u8]) -> Vec<u8> {
    let pmap = &world.map.province_map;
    let width = pmap.width;
    let height = pmap.height;
    let pixels = &pmap.pixels;
    let definitions = &world.map.definitions;
    let state_of = &world.provinces.state_of;

    let province_type = |pid: u16| -> ProvinceType {
        definitions
            .get(pid as usize)
            .and_then(|d| d.as_ref())
            .map(|d| d.province_type)
            .unwrap_or(ProvinceType::Sea)
    };
    let state_id =
        |pid: u16| -> StateId { state_of.get(pid as usize).copied().unwrap_or(StateId::NONE) };
    let impassable = |pid: u16| -> bool {
        definitions
            .get(pid as usize)
            .and_then(|d| d.as_ref())
            .map(|d| is_impassable_terrain(&d.terrain))
            .unwrap_or(false)
    };
    let semantic_border = |a: u16, b: u16| -> bool {
        if a == b {
            return false;
        }
        let ta = province_type(a);
        let tb = province_type(b);
        if ta == ProvinceType::Sea && tb == ProvinceType::Sea {
            return false;
        }
        if ta == ProvinceType::Sea || tb == ProvinceType::Sea {
            return true;
        }
        if impassable(a) || impassable(b) {
            return true;
        }
        let sa = state_id(a);
        let sb = state_id(b);
        !sa.is_none() && !sb.is_none() && sa != sb
    };

    let mut dist = chamfer_distance_field(width, height, |x, y| {
        let idx = (y * width + x) as usize;
        let me = pixels[idx];
        if x + 1 < width && semantic_border(me, pixels[(y * width + x + 1) as usize]) {
            return true;
        }
        if y + 1 < height && semantic_border(me, pixels[((y + 1) * width + x) as usize]) {
            return true;
        }
        if x > 0 && semantic_border(me, pixels[(y * width + x - 1) as usize]) {
            return true;
        }
        if y > 0 && semantic_border(me, pixels[((y - 1) * width + x) as usize]) {
            return true;
        }
        false
    });

    if coast_sdf.len() == dist.len() {
        for (d, coast) in dist.iter_mut().zip(coast_sdf.iter().copied()) {
            *d = (*d).min(coast);
        }
    }
    dist
}

fn is_impassable_terrain(terrain: &str) -> bool {
    let terrain = terrain.to_ascii_lowercase();
    terrain.contains("impassable") || terrain.contains("wasteland") || terrain.contains("blocked")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_sdf_or_far_preserves_matching_data() {
        assert_eq!(copy_sdf_or_far(&[0, 3, 9, 255], 2, 2), vec![0, 3, 9, 255]);
    }

    #[test]
    fn copy_sdf_or_far_fills_far_on_mismatch() {
        assert_eq!(copy_sdf_or_far(&[0, 1], 2, 2), vec![255, 255, 255, 255]);
    }

    #[test]
    fn impassable_terrain_detection_covers_vanilla_style_names() {
        assert!(is_impassable_terrain("mountain_impassable"));
        assert!(is_impassable_terrain("wasteland"));
        assert!(!is_impassable_terrain("plains"));
    }

    #[test]
    fn country_signature_tracks_control_changes() {
        let mut controllers = vec![CountryId::NONE, CountryId(1), CountryId(1)];
        let before = controllers_signature(&controllers);
        controllers[2] = CountryId(2);
        assert_ne!(before, controllers_signature(&controllers));
    }
}
