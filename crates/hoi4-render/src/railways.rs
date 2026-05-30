//! Railway parsing + line-vertex generation.
//!
//! HOI4's `map/railways.txt` is a whitespace-delimited list, one route per line:
//!
//! ```text
//! <level> <capacity> <num_provinces> <prov_id_1> <prov_id_2> ...
//! ```
//!
//! Each route traverses the listed provinces in order. We parse the file,
//! compute pixel-space centroids for each province ID once, then build a
//! flat `Vec<RailVertex>` of line segment endpoints (suitable for a `LineList`
//! draw). The Y coordinate is sampled from the heightmap and lifted slightly
//! to avoid Z-fighting with the terrain.

use hoi4_map::{Heightmap, ProvinceMap};

/// Per-vertex data for the railway line pipeline.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable, Debug)]
pub struct RailVertex {
    /// World-space XYZ.
    pub pos: [f32; 3],
    /// Track level 1.0..3.0 — drives shader colour intensity.
    pub level: f32,
}

/// Per-frame railway decal controls.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable, Debug)]
pub struct RailwayParams {
    pub alpha: f32,
    pub zoom_factor: f32,
    pub _pad: [f32; 2],
}

impl Default for RailwayParams {
    fn default() -> Self {
        Self {
            alpha: 0.42,
            zoom_factor: 0.5,
            _pad: [0.0; 2],
        }
    }
}

/// One parsed railway route.
#[derive(Debug, Clone, PartialEq)]
pub struct RailwayRoute {
    pub level: u8,
    pub capacity: u32,
    pub province_ids: Vec<u16>,
}

/// Parse the `railways.txt` content.
/// Format: `<level> <num_provinces> <prov_id_1> <prov_id_2> ...`
pub fn parse_railways(text: &str) -> Vec<RailwayRoute> {
    let mut out = Vec::new();
    for raw in text.lines() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 3 {
            continue;
        }
        let level = match parts[0].parse::<u8>() {
            Ok(v) => v,
            Err(_) => continue,
        };
        let n = match parts[1].parse::<usize>() {
            Ok(v) => v,
            Err(_) => continue,
        };
        if n < 2 || parts.len() < 2 + n {
            continue;
        }
        let pids: Vec<u16> = parts[2..2 + n]
            .iter()
            .filter_map(|s| s.parse::<u16>().ok())
            .collect();
        if pids.len() < 2 {
            continue;
        }
        out.push(RailwayRoute {
            level,
            capacity: 0,
            province_ids: pids,
        });
    }
    out
}

/// Compute pixel-space centroid (mean position of all member pixels) for every
/// province ID. Output is indexed by province ID; entries with no pixels stay
/// (0.0, 0.0).
pub fn compute_province_centroids(province_map: &ProvinceMap) -> Vec<(f32, f32)> {
    let max_id = province_map.pixels.iter().copied().max().unwrap_or(0) as usize;
    let mut sum_x = vec![0u64; max_id + 1];
    let mut sum_y = vec![0u64; max_id + 1];
    let mut count = vec![0u32; max_id + 1];
    let w = province_map.width;
    for y in 0..province_map.height {
        for x in 0..w {
            let id = province_map.pixels[(y * w + x) as usize] as usize;
            sum_x[id] += x as u64;
            sum_y[id] += y as u64;
            count[id] += 1;
        }
    }
    let mut centroids = vec![(0.0, 0.0); max_id + 1];
    for id in 0..=max_id {
        if count[id] > 0 {
            centroids[id] = (
                sum_x[id] as f32 / count[id] as f32,
                sum_y[id] as f32 / count[id] as f32,
            );
        }
    }
    centroids
}

/// Build a `LineList` vertex buffer (each consecutive pair of vertices forms
/// one line segment). `world_scale` and `height_scale` match the terrain
/// pipeline's transform so railways line up with the mesh.
///
/// `lift` is added to each endpoint's Y to avoid z-fighting (in world units).
pub fn build_railway_vertices(
    routes: &[RailwayRoute],
    centroids: &[(f32, f32)],
    heightmap: &Heightmap,
    world_scale: f32,
    height_scale: f32,
    lift: f32,
) -> Vec<RailVertex> {
    let w = heightmap.width;
    let h = heightmap.height;
    let mut verts = Vec::new();
    let sample_y = |px: f32, py: f32| -> f32 {
        let xi = (px as u32).min(w.saturating_sub(1));
        let yi = (py as u32).min(h.saturating_sub(1));
        heightmap.pixels[(yi * w + xi) as usize] as f32 / 255.0
    };
    for route in routes {
        let lvl = route.level as f32;
        for win in route.province_ids.windows(2) {
            let a = win[0] as usize;
            let b = win[1] as usize;
            if a >= centroids.len() || b >= centroids.len() {
                continue;
            }
            let (ax, ay) = centroids[a];
            let (bx, by) = centroids[b];
            if (ax + ay + bx + by) == 0.0 {
                continue;
            }
            let av = RailVertex {
                pos: [
                    ax * world_scale,
                    sample_y(ax, ay) * height_scale + lift,
                    ay * world_scale,
                ],
                level: lvl,
            };
            let bv = RailVertex {
                pos: [
                    bx * world_scale,
                    sample_y(bx, by) * height_scale + lift,
                    by * world_scale,
                ],
                level: lvl,
            };
            verts.push(av);
            verts.push(bv);
        }
    }
    verts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_basic_lines() {
        let txt = "3 6 1 2 3 4 5 6\n2 4 1 2 3 4\n";
        let routes = parse_railways(txt);
        assert_eq!(routes.len(), 2);
        assert_eq!(routes[0].level, 3);
        assert_eq!(routes[0].province_ids, vec![1, 2, 3, 4, 5, 6]);
        assert_eq!(routes[1].level, 2);
        assert_eq!(routes[1].province_ids.len(), 4);
    }

    #[test]
    fn skips_too_short_routes() {
        let txt = "1 1 5\n";
        assert!(parse_railways(txt).is_empty());
    }

    #[test]
    fn skips_malformed_lines() {
        let txt = "garbage\nabc 2 1 2\n2 2 1 2\n";
        let routes = parse_railways(txt);
        assert_eq!(routes.len(), 1);
        assert_eq!(routes[0].province_ids, vec![1, 2]);
    }

    #[test]
    fn skips_blank_and_comment_lines() {
        let txt = "\n# comment\n  \n2 2 1 2\n";
        let routes = parse_railways(txt);
        assert_eq!(routes.len(), 1);
    }

    #[test]
    fn centroids_for_simple_map() {
        // 4×2 map with two provinces split left/right.
        let pmap = ProvinceMap {
            width: 4,
            height: 2,
            pixels: vec![1, 1, 2, 2, 1, 1, 2, 2],
        };
        let cs = compute_province_centroids(&pmap);
        // Province 1 at average (0.5, 0.5).
        assert!((cs[1].0 - 0.5).abs() < 1e-3);
        assert!((cs[1].1 - 0.5).abs() < 1e-3);
        // Province 2 at (2.5, 0.5).
        assert!((cs[2].0 - 2.5).abs() < 1e-3);
        assert!((cs[2].1 - 0.5).abs() < 1e-3);
    }

    #[test]
    fn build_vertices_emits_pairs() {
        let pmap = ProvinceMap {
            width: 4,
            height: 2,
            pixels: vec![1, 1, 2, 2, 1, 1, 2, 2],
        };
        let h = Heightmap {
            width: 4,
            height: 2,
            pixels: vec![100; 8],
        };
        let centroids = compute_province_centroids(&pmap);
        let route = RailwayRoute {
            level: 2,
            capacity: 5,
            province_ids: vec![1, 2],
        };
        let verts = build_railway_vertices(&[route], &centroids, &h, 1.0, 10.0, 0.05);
        assert_eq!(verts.len(), 2); // one segment = two vertices
        assert_eq!(verts[0].level, 2.0);
        // Y should be height_value (100/255) * 10 + lift 0.05 ≈ 3.97.
        assert!((verts[0].pos[1] - (100.0 / 255.0 * 10.0 + 0.05)).abs() < 1e-3);
    }

    #[test]
    fn railway_params_size_is_16() {
        assert_eq!(std::mem::size_of::<RailwayParams>(), 16);
    }

    #[test]
    fn railway_wgsl_validates() {
        let module = match naga::front::wgsl::parse_str(crate::SHADER_RAILWAYS_WGSL) {
            Ok(module) => module,
            Err(err) => {
                eprintln!(
                    "[shader-validate] railways WGSL parse failed:\n{}",
                    err.emit_to_string(crate::SHADER_RAILWAYS_WGSL)
                );
                panic!("railways WGSL parse failed");
            }
        };
        let mut validator = naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        );
        if let Err(err) = validator.validate(&module) {
            eprintln!(
                "[shader-validate] railways WGSL validation failed:\n{}",
                err.emit_to_string(crate::SHADER_RAILWAYS_WGSL)
            );
            panic!("railways WGSL validation failed");
        }
    }
}
