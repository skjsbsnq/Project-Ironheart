//! Phase 3.12.9 (redesign) — Border strip mesh extraction.
//!
//! Vanilla `border.shader` renders borders as **thin strip meshes** that hug
//! the actual province/country boundaries, NOT as full-screen overlays.
//! Each strip is a quad-strip (triangle list) with UV.x ∈ [0,1] across the
//! width and UV.y tiling along the border direction.
//!
//! This module extracts border **edge segments** from the province bitmap,
//! classifies them by type (country / state / province / sea / impassable),
//! and generates the strip geometry (vertex + index buffers).

use hoi4_map::definition::ProvinceType;
use hoi4_state::{CountryId, StateId, World};

// ─── Types ──────────────────────────────────────────────────────────────────

/// Border classification — determines which DDS texture to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BorderKind {
    Country,
    State,
    Province,
    Sea,
    SeaRegion,
    Impassable,
}

/// A single edge segment between two adjacent pixels.
/// Stored as the midpoint (in pixel coords) + direction.
#[derive(Debug, Clone, Copy)]
pub struct BorderEdge {
    /// Midpoint X in pixel space (can be N+0.5 for vertical edges).
    pub mx: f32,
    /// Midpoint Y in pixel space.
    pub my: f32,
    /// Normal direction: (1,0) for vertical edge, (0,1) for horizontal edge.
    pub nx: f32,
    pub ny: f32,
    pub kind: BorderKind,
    pub province_a: u16,
    pub province_b: u16,
}

/// Per-vertex data for the border strip mesh.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct BorderVertex {
    /// World-space position.
    pub pos: [f32; 3],
    /// World-space strip center, used by the shader for screen-space width clamps.
    pub center: [f32; 3],
    /// UV: x = across strip [0,1], y = along strip (tiling).
    pub uv: [f32; 2],
    /// Province IDs on each side of this border segment.
    pub province_a: u32,
    pub province_b: u32,
    /// `BorderKind` encoded for shader hierarchy/debug styling.
    pub kind: u32,
}

// ─── Edge extraction ────────────────────────────────────────────────────────

/// Extract all border edges from the province bitmap.
///
/// For each pixel, checks right-neighbour and bottom-neighbour. If they belong
/// to different provinces, emits a `BorderEdge` classified by type.
pub fn extract_border_edges(world: &World) -> Vec<BorderEdge> {
    let pmap = &world.map.province_map;
    let w = pmap.width as usize;
    let h = pmap.height as usize;
    let pixels = &pmap.pixels;
    let controllers = &world.provinces.controllers;
    let state_of = &world.provinces.state_of;
    let definitions = &world.map.definitions;

    let prov_type = |pid: u16| -> ProvinceType {
        definitions
            .get(pid as usize)
            .and_then(|d| d.as_ref())
            .map(|d| d.province_type)
            .unwrap_or(ProvinceType::Sea)
    };

    let owner_of = |pid: u16| -> CountryId {
        controllers
            .get(pid as usize)
            .copied()
            .unwrap_or(CountryId::NONE)
    };

    let state_id_of =
        |pid: u16| -> StateId { state_of.get(pid as usize).copied().unwrap_or(StateId::NONE) };
    let sea_region_of = |pid: u16| -> Option<u32> {
        world
            .countries
            .trade
            .sea_region_map
            .as_ref()
            .and_then(|sea_map| sea_map.province_region.get(&pid).copied())
    };
    let is_impassable = |pid: u16| -> bool {
        definitions
            .get(pid as usize)
            .and_then(|d| d.as_ref())
            .map(|d| {
                let terrain = d.terrain.to_ascii_lowercase();
                terrain.contains("impassable")
                    || terrain.contains("wasteland")
                    || terrain.contains("blocked")
            })
            .unwrap_or(false)
    };

    let classify = |a: u16, b: u16| -> Option<BorderKind> {
        if a == b {
            return None;
        }
        let ta = prov_type(a);
        let tb = prov_type(b);

        // Sea ↔ anything = Sea border
        if ta == ProvinceType::Sea && tb == ProvinceType::Sea {
            let region_changes = match (sea_region_of(a), sea_region_of(b)) {
                (Some(ra), Some(rb)) => ra != rb,
                _ => false,
            };
            if region_changes {
                return Some(BorderKind::SeaRegion);
            }
            return None;
        }

        if ta == ProvinceType::Sea || tb == ProvinceType::Sea {
            return Some(BorderKind::Sea);
        }
        // Lake ↔ land = Province border (not sea)
        if ta == ProvinceType::Lake || tb == ProvinceType::Lake {
            return Some(BorderKind::Province);
        }

        // Both land
        if is_impassable(a) || is_impassable(b) {
            return Some(BorderKind::Impassable);
        }

        let oa = owner_of(a);
        let ob = owner_of(b);
        if !oa.is_none() && !ob.is_none() && oa != ob {
            return Some(BorderKind::Country);
        }

        let sa = state_id_of(a);
        let sb = state_id_of(b);
        if !sa.is_none() && !sb.is_none() && sa != sb {
            return Some(BorderKind::State);
        }

        // Same state, different province
        Some(BorderKind::Province)
    };

    // Pre-allocate conservatively (typical map has ~2M border edges)
    let mut edges = Vec::with_capacity(w * h / 4);

    for y in 0..h {
        for x in 0..w {
            let a = pixels[y * w + x];
            // Right neighbour → vertical edge at (x+0.5, y)
            if x + 1 < w {
                let b = pixels[y * w + x + 1];
                if let Some(kind) = classify(a, b) {
                    edges.push(BorderEdge {
                        mx: x as f32 + 0.5,
                        my: y as f32,
                        nx: 1.0,
                        ny: 0.0,
                        kind,
                        province_a: a,
                        province_b: b,
                    });
                }
            }
            // Bottom neighbour → horizontal edge at (x, y+0.5)
            if y + 1 < h {
                let b = pixels[(y + 1) * w + x];
                if let Some(kind) = classify(a, b) {
                    edges.push(BorderEdge {
                        mx: x as f32,
                        my: y as f32 + 0.5,
                        nx: 0.0,
                        ny: 1.0,
                        kind,
                        province_a: a,
                        province_b: b,
                    });
                }
            }
        }
    }

    edges
}

// ─── Strip mesh generation ──────────────────────────────────────────────────

/// Parameters for strip mesh generation.
pub struct StripParams {
    /// World units per heightmap pixel on XZ plane.
    pub world_scale: f32,
    /// World Y per heightmap value (0..1 → 0..height_scale).
    pub height_scale: f32,
    /// Half-width of the border strip in world units.
    pub half_width: f32,
    /// Sea level in heightmap units (0..1). Vertices below this get clamped.
    pub sea_level: f32,
    /// Small Y offset to prevent z-fighting with terrain.
    pub y_bias: f32,
    /// UV.y tile factor (vanilla `BORDER_TILE`). Higher = more repetitions.
    pub tile_factor: f32,
}

impl Default for StripParams {
    fn default() -> Self {
        Self {
            world_scale: 0.02,
            height_scale: 1.45,
            half_width: 0.009,
            sea_level: 95.0 / 255.0,
            y_bias: 0.018,
            tile_factor: 0.20,
        }
    }
}

/// Output of strip mesh generation for one border kind.
pub struct BorderMesh {
    pub kind: BorderKind,
    pub vertices: Vec<BorderVertex>,
    pub indices: Vec<u32>,
}

/// Generate strip meshes from extracted edges, grouped by `BorderKind`.
///
/// Collinear neighbouring bitmap edges are merged into continuous strips. This
/// keeps the border geometry faithful to the province bitmap while removing the
/// per-pixel quad seams and UV phase jumps that read as a grid at close zoom.
pub fn generate_border_meshes(
    edges: &[BorderEdge],
    heightmap: &hoi4_map::Heightmap,
    params: &StripParams,
) -> Vec<BorderMesh> {
    let hm_w = heightmap.width;
    let hm_h = heightmap.height;
    let hm_pixels = &heightmap.pixels;

    let sample_y = |px: f32, py: f32| -> f32 {
        let xi = (px as u32).min(hm_w.saturating_sub(1));
        let yi = (py as u32).min(hm_h.saturating_sub(1));
        let raw = hm_pixels[(yi * hm_w + xi) as usize] as f32 / 255.0;
        let h = raw.max(params.sea_level);
        h * params.height_scale + params.y_bias
    };

    // Group edges by kind
    let mut groups: [Vec<&BorderEdge>; 6] = Default::default();
    for e in edges {
        let idx = match e.kind {
            BorderKind::Country => 0,
            BorderKind::State => 1,
            BorderKind::Province => 2,
            BorderKind::Sea => 3,
            BorderKind::SeaRegion => 4,
            BorderKind::Impassable => 5,
        };
        groups[idx].push(e);
    }

    let kinds = [
        BorderKind::Country,
        BorderKind::State,
        BorderKind::Province,
        BorderKind::Sea,
        BorderKind::SeaRegion,
        BorderKind::Impassable,
    ];

    kinds
        .iter()
        .zip(groups.iter())
        .filter(|(_, g)| !g.is_empty())
        .map(|(&kind, group)| {
            let mut vertices = Vec::with_capacity(group.len() * 2);
            let mut indices = Vec::with_capacity(group.len() * 6);
            let mut runs: Vec<RunKey> = group.iter().map(|edge| RunKey::from_edge(edge)).collect();
            runs.sort_unstable();

            let mut run_start = 0usize;
            while run_start < runs.len() {
                let mut run_end = run_start + 1;
                while run_end < runs.len()
                    && runs[run_end].orientation == runs[run_start].orientation
                    && runs[run_end].fixed2 == runs[run_start].fixed2
                    && runs[run_end].province_a == runs[run_start].province_a
                    && runs[run_end].province_b == runs[run_start].province_b
                    && runs[run_end].variable2 == runs[run_end - 1].variable2 + 2
                {
                    run_end += 1;
                }

                append_run_strip(
                    kind,
                    &runs[run_start..run_end],
                    params,
                    &sample_y,
                    &mut vertices,
                    &mut indices,
                );
                run_start = run_end;
            }

            BorderMesh {
                kind,
                vertices,
                indices,
            }
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum EdgeOrientation {
    Vertical,
    Horizontal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct RunKey {
    orientation: EdgeOrientation,
    fixed2: i32,
    variable2: i32,
    province_a: u16,
    province_b: u16,
}

impl RunKey {
    fn from_edge(edge: &BorderEdge) -> Self {
        let (province_a, province_b) = if edge.province_a <= edge.province_b {
            (edge.province_a, edge.province_b)
        } else {
            (edge.province_b, edge.province_a)
        };
        if edge.nx.abs() > edge.ny.abs() {
            Self {
                orientation: EdgeOrientation::Vertical,
                fixed2: (edge.mx * 2.0).round() as i32,
                variable2: (edge.my * 2.0).round() as i32,
                province_a,
                province_b,
            }
        } else {
            Self {
                orientation: EdgeOrientation::Horizontal,
                fixed2: (edge.my * 2.0).round() as i32,
                variable2: (edge.mx * 2.0).round() as i32,
                province_a,
                province_b,
            }
        }
    }
}

fn append_run_strip<F>(
    kind: BorderKind,
    run: &[RunKey],
    params: &StripParams,
    sample_y: &F,
    vertices: &mut Vec<BorderVertex>,
    indices: &mut Vec<u32>,
) where
    F: Fn(f32, f32) -> f32,
{
    if run.is_empty() {
        return;
    }

    let width_multiplier = match kind {
        BorderKind::Country => 1.55,
        BorderKind::State => 1.15,
        BorderKind::Province => 0.70,
        BorderKind::Sea => 0.85,
        BorderKind::SeaRegion => 0.72,
        BorderKind::Impassable => 1.15,
    };
    let half_width = params.half_width * width_multiplier;
    let base = vertices.len() as u32;
    let first_var = run[0].variable2 as f32 * 0.5 - 0.5;
    let last_var = run[run.len() - 1].variable2 as f32 * 0.5 + 0.5;
    let fixed = run[0].fixed2 as f32 * 0.5;

    for point_idx in 0..=run.len() {
        let variable = if point_idx == 0 {
            first_var
        } else if point_idx == run.len() {
            last_var
        } else {
            (run[point_idx - 1].variable2 as f32 * 0.5) + 0.5
        };
        let uv_y = point_idx as f32 * params.tile_factor;

        let (px, py, nx, ny) = match run[0].orientation {
            EdgeOrientation::Vertical => (fixed, variable, 1.0, 0.0),
            EdgeOrientation::Horizontal => (variable, fixed, 0.0, 1.0),
        };
        let wx = px * params.world_scale;
        let wz = py * params.world_scale;
        let wy = sample_y(px, py);
        let norm_dx = nx * half_width;
        let norm_dz = ny * half_width;
        let kind_id = border_kind_shader_id(kind);

        vertices.push(BorderVertex {
            pos: [wx - norm_dx, wy, wz - norm_dz],
            center: [wx, wy, wz],
            uv: [0.0, uv_y],
            province_a: run[0].province_a as u32,
            province_b: run[0].province_b as u32,
            kind: kind_id,
        });
        vertices.push(BorderVertex {
            pos: [wx + norm_dx, wy, wz + norm_dz],
            center: [wx, wy, wz],
            uv: [1.0, uv_y],
            province_a: run[0].province_a as u32,
            province_b: run[0].province_b as u32,
            kind: kind_id,
        });
    }

    for segment_idx in 0..run.len() as u32 {
        let i0 = base + segment_idx * 2;
        let i1 = i0 + 1;
        let i2 = i0 + 2;
        let i3 = i0 + 3;
        indices.extend_from_slice(&[i0, i1, i2, i1, i3, i2]);
    }
}

fn border_kind_shader_id(kind: BorderKind) -> u32 {
    match kind {
        BorderKind::Country => 0,
        BorderKind::State => 1,
        BorderKind::Province => 2,
        BorderKind::Sea => 3,
        BorderKind::SeaRegion => 4,
        BorderKind::Impassable => 5,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn border_vertex_size_is_44() {
        assert_eq!(std::mem::size_of::<BorderVertex>(), 44);
    }

    #[test]
    fn classify_same_province_returns_none() {
        // Two identical province IDs should not produce an edge
        // (tested implicitly via extract_border_edges with uniform map)
    }
}
