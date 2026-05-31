use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use hoi4_map::Heightmap;

use super::VanillaRuntimeTargetFrameParams;

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

pub fn generate_mud_snow(
    heightmap: &Heightmap,
    season_snow_offset: f32,
    season_blend: f32,
) -> Vec<u8> {
    let width = heightmap.width as usize;
    let height = heightmap.height as usize;
    let mut data = vec![0u8; width * height * 4];
    let winter = (0.55 + season_snow_offset * 1.8 + season_blend * 0.15).clamp(0.0, 1.0);
    for y in 0..height {
        let v = if height > 1 {
            y as f32 / (height - 1) as f32
        } else {
            0.5
        };
        let latitude = (v - 0.5).abs() * 2.0;
        let polar = smoothstep(0.72, 0.96, latitude);
        for x in 0..width {
            let i = y * width + x;
            let h = heightmap.pixels.get(i).copied().unwrap_or(0) as f32 / 255.0;
            let altitude = smoothstep(0.62 + season_snow_offset, 0.88 + season_snow_offset, h);
            let snow_now = (polar * winter + altitude * 0.85).clamp(0.0, 1.0);
            let snow_winter = snow_now.max(winter * smoothstep(0.46, 0.76, h));
            let lowland = 1.0 - smoothstep(0.42, 0.74, h);
            let no_snow = 1.0 - snow_now.max(snow_winter * 0.55);
            let mud_now = (0.35 + season_blend * 0.25) * lowland * no_snow;
            let mud_winter = (1.0 - winter) * 0.35 * lowland * no_snow;
            let o = i * 4;
            data[o] = to_u8(mud_now);
            data[o + 1] = to_u8(snow_winter);
            data[o + 2] = to_u8(snow_now);
            data[o + 3] = to_u8(mud_winter);
        }
    }
    data
}

pub fn mud_snow_signature(params: &VanillaRuntimeTargetFrameParams) -> u64 {
    let mut h = DefaultHasher::new();
    quantize(params.season_snow_offset).hash(&mut h);
    quantize(params.season_blend).hash(&mut h);
    h.finish()
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0).max(0.0001)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn to_u8(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn quantize(value: f32) -> i16 {
    (value.clamp(-1.0, 1.0) * 4096.0).round() as i16
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
    fn mud_snow_dimensions_match_heightmap() {
        let hm = Heightmap {
            width: 2,
            height: 2,
            pixels: vec![0, 96, 180, 255],
        };
        let data = generate_mud_snow(&hm, 0.0, 0.5);
        assert_eq!(data.len(), 2 * 2 * 4);
        assert!(data.iter().any(|&v| v > 0));
    }
}
