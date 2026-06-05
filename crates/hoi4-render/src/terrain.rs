//! Terrain chunking + LOD selection.
//!
//! The world map is partitioned into a grid of `Chunk`s. Each chunk renders as
//! an `N×N` quad grid where `N` (the LOD step) depends on its distance from
//! the camera. Per-chunk AABBs (computed from the heightmap min/max) are used
//! for view-frustum culling.

use glam::{Vec2, Vec3, Vec4};

use crate::camera::Camera;
use hoi4_map::Heightmap;

/// LOD levels — number of quads per side. Index 0 = highest detail.
/// Phase 11.1: changed from [32, 16, 8] to [32, 24, 16] — 1.5× ratio
/// between adjacent levels means denser vertex alignment at LOD boundaries,
/// reducing visible seams.
/// 3.12.19 (2026-05-23): close-up quality pass. LOD0 now spans
/// ≈1.38 heightmap pixels per quad with CHUNKS_X=32, reducing visible
/// triangulation on mountains, coasts, and small islands.
pub const LOD_GRID: [u32; 3] = [64, 32, 16];

/// A single map chunk.
#[derive(Debug, Clone)]
pub struct Chunk {
    /// World-space origin (south-west corner on XZ plane).
    pub origin_xz: Vec2,
    /// World-space size on XZ plane (width, depth).
    pub size_xz: Vec2,
    /// Min/max world Y in this chunk (after height scale applied).
    pub min_y: f32,
    pub max_y: f32,
}

impl Chunk {
    pub fn aabb(&self) -> (Vec3, Vec3) {
        let min = Vec3::new(self.origin_xz.x, self.min_y, self.origin_xz.y);
        let max = Vec3::new(
            self.origin_xz.x + self.size_xz.x,
            self.max_y,
            self.origin_xz.y + self.size_xz.y,
        );
        (min, max)
    }

    pub fn center(&self) -> Vec3 {
        let (min, max) = self.aabb();
        (min + max) * 0.5
    }
}

/// All chunks for the map, plus per-frame culling.
#[derive(Debug, Clone)]
pub struct ChunkGrid {
    pub chunks: Vec<Chunk>,
    /// Number of chunks along X (kept for debug/inspection).
    #[allow(dead_code)]
    pub nx: u32,
    /// Number of chunks along Z (kept for debug/inspection).
    #[allow(dead_code)]
    pub nz: u32,
    pub world_size: Vec2,
}

impl ChunkGrid {
    /// Build chunks from a heightmap.
    /// `world_size` = world-space dimensions of the whole map (XZ plane).
    /// `height_scale` = world Y per (heightmap value / 255).
    /// `nx`, `nz` = number of chunks along X / Z.
    pub fn build(
        heightmap: &Heightmap,
        world_size: Vec2,
        height_scale: f32,
        nx: u32,
        nz: u32,
    ) -> Self {
        let mut chunks = Vec::with_capacity((nx * nz) as usize);
        let chunk_w = world_size.x / nx as f32;
        let chunk_d = world_size.y / nz as f32;

        for cz in 0..nz {
            for cx in 0..nx {
                let origin_xz = Vec2::new(cx as f32 * chunk_w, cz as f32 * chunk_d);
                let size_xz = Vec2::new(chunk_w, chunk_d);
                let (min_h, max_h) = heightmap_minmax(
                    heightmap,
                    cx as f32 / nx as f32,
                    (cx + 1) as f32 / nx as f32,
                    cz as f32 / nz as f32,
                    (cz + 1) as f32 / nz as f32,
                );
                chunks.push(Chunk {
                    origin_xz,
                    size_xz,
                    min_y: min_h * height_scale,
                    max_y: max_h * height_scale,
                });
            }
        }
        Self {
            chunks,
            nx,
            nz,
            world_size,
        }
    }

    /// Select visible chunks + LOD per chunk.
    /// Returns a vector with the same length as `chunks`; each entry is `Some(lod)` if visible, else `None`.
    pub fn cull_and_lod(&self, camera: &Camera) -> Vec<Option<usize>> {
        let planes = camera.frustum_planes();
        let eye = camera.eye();

        // LOD distance thresholds (world units).
        // We use map's larger dimension to scale them.
        // Phase 11.1: widened LOD0/LOD1 coverage so seams appear farther away.
        let map_extent = self.world_size.x.max(self.world_size.y);
        let lod0_dist = map_extent * 0.12;
        let lod1_dist = map_extent * 0.35;

        self.chunks
            .iter()
            .map(|c| {
                let (min, max) = c.aabb();
                if !crate::camera::aabb_in_frustum(&planes, min, max) {
                    return None;
                }
                let center = c.center();
                let dx = center.x - eye.x;
                let dz = center.z - eye.z;
                let dy = center.y - eye.y;
                let dist = (dx * dx + dy * dy + dz * dz).sqrt();
                let lod = if dist < lod0_dist {
                    0
                } else if dist < lod1_dist {
                    1
                } else {
                    2
                };
                Some(lod)
            })
            .collect()
    }
}

/// Min/max heightmap value in [u, v] window (UV-space). Result in [0,1].
fn heightmap_minmax(h: &Heightmap, u0: f32, u1: f32, v0: f32, v1: f32) -> (f32, f32) {
    let x0 = ((u0 * h.width as f32).floor() as u32).min(h.width.saturating_sub(1));
    let x1 = ((u1 * h.width as f32).ceil() as u32).min(h.width);
    let y0 = ((v0 * h.height as f32).floor() as u32).min(h.height.saturating_sub(1));
    let y1 = ((v1 * h.height as f32).ceil() as u32).min(h.height);

    if x1 <= x0 || y1 <= y0 {
        return (0.0, 0.0);
    }

    // Sample sparsely — a stride that gives ~32 samples per side is plenty.
    let stride_x = (((x1 - x0) / 32).max(1)) as usize;
    let stride_y = (((y1 - y0) / 32).max(1)) as usize;
    let w = h.width as usize;

    let mut lo = 255u8;
    let mut hi = 0u8;
    let mut y = y0 as usize;
    let y1u = y1 as usize;
    while y < y1u {
        let mut x = x0 as usize;
        let x1u = x1 as usize;
        while x < x1u {
            let v = h.pixels[y * w + x];
            if v < lo {
                lo = v;
            }
            if v > hi {
                hi = v;
            }
            x += stride_x;
        }
        y += stride_y;
    }
    (lo as f32 / 255.0, hi as f32 / 255.0)
}

/// Per-chunk instance attributes (vertex buffer, instance step).
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable, Debug)]
pub struct ChunkInstance {
    /// World-space SW corner XZ.
    pub origin_xz: [f32; 2],
    /// Chunk size XZ.
    pub size_xz: [f32; 2],
}

impl ChunkInstance {
    pub fn from_chunk(c: &Chunk) -> Self {
        Self {
            origin_xz: [c.origin_xz.x, c.origin_xz.y],
            size_xz: [c.size_xz.x, c.size_xz.y],
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TerrainBucketStats {
    pub counts: [u32; 3],
    pub signatures: [u64; 3],
}

/// Group culled chunks by LOD into instance buffers.
/// Returns three Vecs (one per LOD level).
pub fn build_instance_buckets(
    grid: &ChunkGrid,
    visibility: &[Option<usize>],
) -> [Vec<ChunkInstance>; 3] {
    let mut out: [Vec<ChunkInstance>; 3] = Default::default();
    for (chunk, vis) in grid.chunks.iter().zip(visibility.iter()) {
        if let Some(lod) = *vis {
            out[lod].push(ChunkInstance::from_chunk(chunk));
        }
    }
    out
}

/// Group visible chunks by LOD, including horizontal copies so the map wraps
/// seamlessly when the camera crosses the date-line edge.
pub fn build_wrapped_instance_buckets(
    grid: &ChunkGrid,
    camera: &Camera,
) -> [Vec<ChunkInstance>; 3] {
    let mut out: [Vec<ChunkInstance>; 3] = Default::default();
    build_wrapped_instance_buckets_into(grid, camera, &mut out);
    out
}

/// Rebuild wrapped visible chunk buckets in place and compute upload stats in
/// the same pass.
pub fn build_wrapped_instance_buckets_into(
    grid: &ChunkGrid,
    camera: &Camera,
    out: &mut [Vec<ChunkInstance>; 3],
) -> TerrainBucketStats {
    for bucket in out.iter_mut() {
        bucket.clear();
    }

    let mut stats = TerrainBucketStats::default();
    let planes = camera.frustum_planes();
    let eye = camera.eye();
    let map_extent = grid.world_size.x.max(grid.world_size.y);
    let lod0_dist = map_extent * 0.12;
    let lod1_dist = map_extent * 0.35;

    for shift in [-grid.world_size.x, 0.0, grid.world_size.x] {
        for chunk in &grid.chunks {
            let shifted = Chunk {
                origin_xz: Vec2::new(chunk.origin_xz.x + shift, chunk.origin_xz.y),
                size_xz: chunk.size_xz,
                min_y: chunk.min_y,
                max_y: chunk.max_y,
            };
            let (min, max) = shifted.aabb();
            if !crate::camera::aabb_in_frustum(&planes, min, max) {
                continue;
            }
            let center = shifted.center();
            let dist = (center - eye).length();
            let lod = if dist < lod0_dist {
                0
            } else if dist < lod1_dist {
                1
            } else {
                2
            };
            let instance = ChunkInstance::from_chunk(&shifted);
            stats.counts[lod] = stats.counts[lod].saturating_add(1);
            terrain_bucket_mix_instance(&mut stats.signatures[lod], instance);
            out[lod].push(instance);
        }
    }

    for lod in 0..3 {
        terrain_bucket_mix_u64(&mut stats.signatures[lod], stats.counts[lod] as u64);
    }

    stats
}

/// Number of triangles in a draw of `grid` quads × `grid` quads.
pub fn vertex_count_for_lod(lod: usize) -> u32 {
    let g = LOD_GRID[lod];
    g * g * 6
}

fn terrain_bucket_mix_instance(signature: &mut u64, instance: ChunkInstance) {
    terrain_bucket_mix_u32(signature, instance.origin_xz[0].to_bits());
    terrain_bucket_mix_u32(signature, instance.origin_xz[1].to_bits());
    terrain_bucket_mix_u32(signature, instance.size_xz[0].to_bits());
    terrain_bucket_mix_u32(signature, instance.size_xz[1].to_bits());
}

fn terrain_bucket_mix_u32(signature: &mut u64, value: u32) {
    terrain_bucket_mix_u64(signature, value as u64);
}

fn terrain_bucket_mix_u64(signature: &mut u64, value: u64) {
    *signature ^= value
        .wrapping_add(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(*signature << 6)
        .wrapping_add(*signature >> 2);
}

// `_` prevents unused warnings for crate consumers that only want LOD constants.
#[allow(dead_code)]
fn _vec4_unused(_: Vec4) {}

#[cfg(test)]
mod tests {
    use super::*;
    use hoi4_map::Heightmap;

    fn flat_heightmap(w: u32, h: u32, value: u8) -> Heightmap {
        Heightmap {
            width: w,
            height: h,
            pixels: vec![value; (w * h) as usize],
        }
    }

    fn ramp_heightmap(w: u32, h: u32) -> Heightmap {
        // x → 0..255 ramp
        let mut pixels = Vec::with_capacity((w * h) as usize);
        for _ in 0..h {
            for x in 0..w {
                pixels.push(((x * 255) / w.max(1)) as u8);
            }
        }
        Heightmap {
            width: w,
            height: h,
            pixels,
        }
    }

    #[test]
    fn build_creates_correct_chunk_count() {
        let h = flat_heightmap(64, 32, 100);
        let grid = ChunkGrid::build(&h, Vec2::new(64.0, 32.0), 1.0, 4, 2);
        assert_eq!(grid.chunks.len(), 8);
        assert_eq!(grid.nx, 4);
        assert_eq!(grid.nz, 2);
    }

    #[test]
    fn flat_chunks_have_uniform_height() {
        let h = flat_heightmap(64, 32, 128);
        let grid = ChunkGrid::build(&h, Vec2::new(64.0, 32.0), 10.0, 4, 2);
        for c in &grid.chunks {
            // 128/255 * 10.0 ≈ 5.02
            assert!((c.min_y - c.max_y).abs() < 1e-3);
            assert!((c.min_y - 5.019608).abs() < 1e-2);
        }
    }

    #[test]
    fn ramp_chunks_have_increasing_max() {
        let h = ramp_heightmap(64, 4);
        let grid = ChunkGrid::build(&h, Vec2::new(64.0, 4.0), 10.0, 4, 1);
        // Each chunk west-to-east should have a strictly higher max_y.
        let mut prev = -1.0;
        for c in &grid.chunks {
            assert!(
                c.max_y > prev,
                "chunks not monotonically increasing: {:?}",
                grid.chunks
            );
            prev = c.max_y;
        }
    }

    #[test]
    fn cull_and_lod_returns_some_for_centred_camera() {
        let h = flat_heightmap(64, 32, 100);
        let grid = ChunkGrid::build(&h, Vec2::new(64.0, 32.0), 1.0, 4, 2);
        let camera = Camera::new(Vec2::new(64.0, 32.0), 16.0 / 9.0);
        let vis = grid.cull_and_lod(&camera);
        // At least one chunk should be visible.
        assert!(vis.iter().any(Option::is_some));
        // Lengths match.
        assert_eq!(vis.len(), grid.chunks.len());
    }

    #[test]
    fn build_instance_buckets_partitions_by_lod() {
        let h = flat_heightmap(64, 32, 100);
        let grid = ChunkGrid::build(&h, Vec2::new(64.0, 32.0), 1.0, 4, 2);
        // Synthesise visibility: chunk 0 → LOD 0, chunk 1 → LOD 1, rest culled.
        let mut vis = vec![None; grid.chunks.len()];
        vis[0] = Some(0);
        vis[1] = Some(1);
        vis[2] = Some(2);
        let buckets = build_instance_buckets(&grid, &vis);
        assert_eq!(buckets[0].len(), 1);
        assert_eq!(buckets[1].len(), 1);
        assert_eq!(buckets[2].len(), 1);
    }

    #[test]
    fn wrapped_instance_buckets_include_shifted_edge_chunks() {
        let h = flat_heightmap(64, 32, 100);
        let grid = ChunkGrid::build(&h, Vec2::new(64.0, 32.0), 1.0, 4, 2);
        let mut camera = Camera::new(Vec2::new(64.0, 32.0), 16.0 / 9.0);
        camera.target.x = 1.0;
        camera.distance = 20.0;
        let buckets = build_wrapped_instance_buckets(&grid, &camera);
        let any_shifted = buckets
            .iter()
            .flatten()
            .any(|inst| inst.origin_xz[0] < 0.0 || inst.origin_xz[0] >= grid.world_size.x);
        assert!(
            any_shifted,
            "expected at least one wrapped terrain instance"
        );
    }

    #[test]
    fn vertex_count_for_lod_matches_grid() {
        assert_eq!(vertex_count_for_lod(0), LOD_GRID[0] * LOD_GRID[0] * 6);
        assert_eq!(vertex_count_for_lod(1), LOD_GRID[1] * LOD_GRID[1] * 6);
        assert_eq!(vertex_count_for_lod(2), LOD_GRID[2] * LOD_GRID[2] * 6);
    }
}
