//! Front-line rendering.
//!
//! Generates strip geometry along the actual province-bitmap contact boundary
//! between countries that are at war. This avoids the old centroid-to-centroid
//! front lines that cut through province interiors.

use hoi4_map::definition::ProvinceType;
use hoi4_state::{CountryId, World};

/// Per-vertex data for front-line strip triangles.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable, Debug)]
pub struct FrontVertex {
    pub pos: [f32; 3],
    /// Country colour (attacker side).
    pub color: [u8; 4],
}

/// Generate front-line vertices from the current world state.
///
/// Scans the province bitmap: if two adjacent land pixels belong to provinces
/// controlled by countries at war with each other, emit a small quad on that
/// exact contact edge. The caller renders the output as `TriangleList`.
pub fn generate_frontline_vertices(
    world: &World,
    _centroids: &[(f32, f32)],
    world_scale: f32,
    height_scale: f32,
) -> Vec<FrontVertex> {
    let heightmap = &world.map.heightmap;
    let pmap = &world.map.province_map;
    let w = pmap.width as usize;
    let h = pmap.height as usize;
    let pixels = &pmap.pixels;
    let mut out = Vec::with_capacity(w * h / 256);

    let sample_y = |px: f32, py: f32| -> f32 {
        let xi = (px as u32).min(heightmap.width.saturating_sub(1));
        let yi = (py as u32).min(heightmap.height.saturating_sub(1));
        heightmap.pixels[(yi * heightmap.width + xi) as usize] as f32 / 255.0
    };

    let province_type = |pid: u16| -> ProvinceType {
        world
            .map
            .definitions
            .get(pid as usize)
            .and_then(|d| d.as_ref())
            .map(|d| d.province_type)
            .unwrap_or(ProvinceType::Sea)
    };

    let is_land = |pid: u16| -> bool { matches!(province_type(pid), ProvinceType::Land) };

    let contact_color = |a: u16, b: u16| -> Option<[u8; 4]> {
        if a == b || a == 0 || b == 0 {
            return None;
        }
        let ai = a as usize;
        let bi = b as usize;
        if ai >= world.provinces.count || bi >= world.provinces.count {
            return None;
        }
        if !is_land(a) || !is_land(b) {
            return None;
        }

        let ctrl_a = world.provinces.controllers[ai];
        let ctrl_b = world.provinces.controllers[bi];
        if ctrl_a.is_none() || ctrl_b.is_none() || ctrl_a == ctrl_b {
            return None;
        }
        if !world.diplomacy.at_war_with(ctrl_a, ctrl_b) {
            return None;
        }

        Some(country_color(world, ctrl_a))
    };

    let half_width = world_scale * 0.9;
    let lift = 0.105;

    for y in 0..h {
        for x in 0..w {
            let a = pixels[y * w + x];

            if x + 1 < w {
                let b = pixels[y * w + x + 1];
                if let Some(color) = contact_color(a, b) {
                    emit_frontline_edge_quad(
                        &mut out,
                        x as f32 + 0.5,
                        y as f32 - 0.5,
                        x as f32 + 0.5,
                        y as f32 + 0.5,
                        1.0,
                        0.0,
                        half_width,
                        world_scale,
                        height_scale,
                        lift,
                        color,
                        &sample_y,
                    );
                }
            }

            if y + 1 < h {
                let b = pixels[(y + 1) * w + x];
                if let Some(color) = contact_color(a, b) {
                    emit_frontline_edge_quad(
                        &mut out,
                        x as f32 - 0.5,
                        y as f32 + 0.5,
                        x as f32 + 0.5,
                        y as f32 + 0.5,
                        0.0,
                        1.0,
                        half_width,
                        world_scale,
                        height_scale,
                        lift,
                        color,
                        &sample_y,
                    );
                }
            }
        }
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn emit_frontline_edge_quad(
    out: &mut Vec<FrontVertex>,
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    nx: f32,
    ny: f32,
    half_width: f32,
    world_scale: f32,
    height_scale: f32,
    lift: f32,
    color: [u8; 4],
    sample_y: &impl Fn(f32, f32) -> f32,
) {
    let sx0 = x0.max(0.0);
    let sy0 = y0.max(0.0);
    let sx1 = x1.max(0.0);
    let sy1 = y1.max(0.0);
    let z0 = sample_y(sx0, sy0) * height_scale + lift;
    let z1 = sample_y(sx1, sy1) * height_scale + lift;
    let ox = nx * half_width;
    let oy = ny * half_width;

    let v0 = FrontVertex {
        pos: [x0 * world_scale - ox, z0, y0 * world_scale - oy],
        color,
    };
    let v1 = FrontVertex {
        pos: [x0 * world_scale + ox, z0, y0 * world_scale + oy],
        color,
    };
    let v2 = FrontVertex {
        pos: [x1 * world_scale - ox, z1, y1 * world_scale - oy],
        color,
    };
    let v3 = FrontVertex {
        pos: [x1 * world_scale + ox, z1, y1 * world_scale + oy],
        color,
    };

    out.extend_from_slice(&[v0, v1, v2, v1, v3, v2]);
}

fn country_color(world: &World, cid: CountryId) -> [u8; 4] {
    if !cid.is_none() && (cid.0 as usize) < world.countries.count {
        let c = world.countries.colors[cid.0 as usize];
        [c[0], c[1], c[2], 255]
    } else {
        [200, 50, 50, 255]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vertex_size() {
        assert_eq!(std::mem::size_of::<FrontVertex>(), 16);
    }
}
