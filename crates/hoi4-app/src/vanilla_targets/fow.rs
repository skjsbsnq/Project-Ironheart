use super::VanillaRuntimeTargetFrameParams;

pub const VANILLA_INTEL_MAP_WIDTH: u32 = 938;
pub const VANILLA_INTEL_MAP_HEIGHT: u32 = 341;
pub const VANILLA_PROJECTED_SHADOW_WIDTH: u32 = 2560;
pub const VANILLA_PROJECTED_SHADOW_HEIGHT: u32 = 1600;

pub fn generate_default_fow(width: u32, height: u32) -> Vec<u8> {
    let mut data = vec![0u8; width as usize * height as usize * 4];
    for px in data.chunks_exact_mut(4) {
        px[0] = 255; // explored
        px[1] = 255; // visible
        px[2] = 0; // enemy spotted/reserved
        px[3] = 255; // target alpha/reserved
    }
    data
}

pub fn generate_default_intel_map() -> Vec<u8> {
    vec![255; (VANILLA_INTEL_MAP_WIDTH * VANILLA_INTEL_MAP_HEIGHT) as usize]
}

pub fn generate_neutral_projected_shadow_fow() -> Vec<u8> {
    let mut data =
        vec![0u8; (VANILLA_PROJECTED_SHADOW_WIDTH * VANILLA_PROJECTED_SHADOW_HEIGHT * 4) as usize];
    for px in data.chunks_exact_mut(4) {
        px[0] = 255; // B: neutral shadow scale in BGRA storage
        px[1] = 255; // G: visible FOW factor
        px[2] = 255; // R: explored FOW factor
        px[3] = 255;
    }
    data
}

pub fn snow_mud_dimensions(map_width: u32, map_height: u32) -> (u32, u32) {
    ((map_width / 4).max(1), (map_height / 4).max(1))
}

pub fn generate_neutral_snow_mud(map_width: u32, map_height: u32) -> Vec<u8> {
    let (width, height) = snow_mud_dimensions(map_width, map_height);
    vec![0u8; (width * height * 4) as usize]
}

pub fn mud_snow_signature(_params: &VanillaRuntimeTargetFrameParams) -> u64 {
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_fow_is_full_visible_rgba() {
        let fow = generate_default_fow(2, 1);
        assert_eq!(fow, vec![255, 255, 0, 255, 255, 255, 0, 255]);
    }

    #[test]
    fn intel_map_matches_vanilla_map_size_div_six() {
        let intel = generate_default_intel_map();
        assert_eq!(
            intel.len(),
            (VANILLA_INTEL_MAP_WIDTH * VANILLA_INTEL_MAP_HEIGHT) as usize
        );
        assert!(intel.iter().all(|&v| v == 255));
    }

    #[test]
    fn projected_shadow_fow_is_full_res_neutral_bgra() {
        let shadow = generate_neutral_projected_shadow_fow();
        assert_eq!(
            shadow.len(),
            (VANILLA_PROJECTED_SHADOW_WIDTH * VANILLA_PROJECTED_SHADOW_HEIGHT * 4) as usize
        );
        assert_eq!(&shadow[0..4], &[255, 255, 255, 255]);
    }

    #[test]
    fn snow_mud_dimensions_are_quarter_map_size() {
        assert_eq!(snow_mud_dimensions(5632, 2048), (1408, 512));
        assert_eq!(snow_mud_dimensions(2, 2), (1, 1));
    }

    #[test]
    fn neutral_snow_mud_is_zero_quarter_size() {
        let data = generate_neutral_snow_mud(5632, 2048);
        assert_eq!(data.len(), 1408 * 512 * 4);
        assert!(data.iter().all(|&v| v == 0));
    }
}
