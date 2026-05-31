//! Procedural tree placement.
//!
//! Phase 3.10.2: tree positions come from `map/trees.bmp` (the artist-painted
//! 1650×600 BMP referenced by `default.map::tree_definition`), not the
//! 5632×2048 `terrain.bmp` category index. Vanilla declares the active
//! palette indices in `default.map::tree = { 3 4 7 10 }`. Resolution ratio
//! is ~0.293 trees-px per terrain-px, so each tree is jittered around the
//! pixel-mapped heightmap centre.
//!
//! Each instance carries:
//! * world position (X, Y, Z) — Y sampled from the heightmap and scaled to
//!   world units so trees sit on the terrain mesh,
//! * world-space scale (varies slightly per tree),
//! * RGBA tint — varies by species (forest dark green, jungle deep green,
//!   marsh dull olive) plus a per-tree noise offset,
//! * tree_type 0/1/2 → deciduous (beech), conifer (pine), tropical (palm).
//! * slope (dh/dx, dh/dz) packed as i8 — vSlopes equivalent, makes tree
//!   bases follow terrain instead of one-sided floating / sinking.
//!
//! Phase 3.10.1: `inst.scale` is now multiplied by `world_scale` so the
//! mesh, which lives in "1 unit ≈ 1 vanilla map pixel" space, ends up the
//! correct fraction of the in-engine compressed map (target ~0.24% of map
//! Y, vs the previous ~0.85%).

use hoi4_map::{Heightmap, TreeBitmap};
use std::collections::HashSet;

/// One tree's data. `repr(C)` so it can be pushed straight to a GPU vertex
/// buffer with `instance` step mode.
///
/// 24 bytes total (multiple of 8 for vertex buffer). Layout chosen so each
/// field can be loaded with a wgpu vertex format whose alignment naturally
/// fits its byte offset:
/// * `pos`        Float32x3 @ 0  (alignment 4)
/// * `scale`      Float32   @ 12 (alignment 4)
/// * `tint`       Unorm8x4  @ 16 (alignment 4)
/// * `tree_type`  Uint8x2   @ 20 (alignment 2; only `.x` used)
/// * `slope`      Snorm8x2  @ 22 (alignment 2; auto-normalised to [-1, 1])
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable, Debug)]
pub struct TreeInstance {
    /// World-space XYZ. Y is `heightmap_value * height_scale`.
    pub pos: [f32; 3],
    /// World-space tree size (height + width). Trees are taller than wide;
    /// the shader spreads this over the billboard quad.
    pub scale: f32,
    /// RGBA tint (Unorm8x4).
    pub tint: [u8; 4],
    /// Tree type: 0=deciduous (beech), 1=conifer (pine), 2=tropical (palm).
    /// Used to select texture / mesh in the trees pipelines.
    pub tree_type: u8,
    /// 1 byte pad so `tree_type+pad` form a Uint8x2 attribute @ offset 20.
    pub _pad0: u8,
    /// Phase 3.10.2: terrain slope ∂h/∂X packed as i8 (Snorm8 ±127).
    /// Shader reads as Snorm8x2 → vec2<f32> in [-1, 1] and applies
    /// `world_pos.y += scaled_pos.x * slope.x + scaled_pos.z * slope.y`.
    pub slope_x: i8,
    /// Slope ∂h/∂Z packed as i8.
    pub slope_z: i8,
}

/// Sea level threshold (heightmap raw 0..=255). Matches the shader constant.
pub const SEA_LEVEL: u8 = 95;

/// CPU-side accounting for the `trees.bmp` -> tree instance conversion.
///
/// This is intentionally tied to the same walk used by [`generate_trees`], so
/// parity reports can prove that instance density came from the vanilla tree
/// mask instead of a terrain-category approximation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TreeDistributionStats {
    pub tree_bitmap_size: [u32; 2],
    pub heightmap_size: [u32; 2],
    pub active_pixels_total: usize,
    pub active_pixels_by_type: [usize; 3],
    pub generated_instances_total: usize,
    pub generated_instances_by_type: [usize; 3],
    pub skipped_by_stride: usize,
    pub skipped_below_sea: usize,
    pub forest_stride: u32,
    pub jungle_stride: u32,
}

impl TreeDistributionStats {
    pub fn placement_ratio(self) -> f32 {
        if self.active_pixels_total == 0 {
            return 0.0;
        }
        self.generated_instances_total as f32 / self.active_pixels_total as f32
    }
}

/// Generate a tree instance buffer.
///
/// `tree_bmp` — Paradox `map/trees.bmp` (the artist mask).
/// `tree_indices` — palette indices treated as "tree pixel" (vanilla {3,4,7,10}).
/// `heightmap` — `map/heightmap.bmp`, used both for Y position and to skip
/// underwater pixels.
/// `world_scale` — units per **heightmap** pixel on the XZ plane.
/// `height_scale` — world-Y units when heightmap raw == 1.0 (255).
/// `forest_stride` — sample one cell every N **trees.bmp** pixels in
///   deciduous/conifer regions (smaller = more trees). Vanilla recommended
///   ~3 → ~25K trees in the temperate forest belt.
/// `jungle_stride` — same for tropical (denser).
pub fn generate_trees(
    tree_bmp: &TreeBitmap,
    tree_indices: &HashSet<u8>,
    heightmap: &Heightmap,
    world_scale: f32,
    height_scale: f32,
    forest_stride: u32,
    jungle_stride: u32,
) -> Vec<TreeInstance> {
    generate_trees_with_stats(
        tree_bmp,
        tree_indices,
        heightmap,
        world_scale,
        height_scale,
        forest_stride,
        jungle_stride,
    )
    .0
}

pub fn generate_trees_with_stats(
    tree_bmp: &TreeBitmap,
    tree_indices: &HashSet<u8>,
    heightmap: &Heightmap,
    world_scale: f32,
    height_scale: f32,
    forest_stride: u32,
    jungle_stride: u32,
) -> (Vec<TreeInstance>, TreeDistributionStats) {
    let tw = tree_bmp.width;
    let th = tree_bmp.height;
    let hw = heightmap.width;
    let hh = heightmap.height;
    let mut out: Vec<TreeInstance> = Vec::new();
    let mut stats = TreeDistributionStats {
        tree_bitmap_size: [tw, th],
        heightmap_size: [hw, hh],
        forest_stride,
        jungle_stride,
        ..TreeDistributionStats::default()
    };

    if tw == 0 || th == 0 || hw == 0 || hh == 0 {
        return (out, stats);
    }

    for &idx in &tree_bmp.pixels {
        if let Some(tree_type) = TreeBitmap::type_for(idx, tree_indices) {
            stats.active_pixels_total += 1;
            if let Some(slot) = stats.active_pixels_by_type.get_mut(tree_type as usize) {
                *slot += 1;
            }
        }
    }

    // Resolution ratio: trees.bmp pixel → heightmap pixel. Vanilla
    // 1650/5632 ≈ 0.293, 600/2048 ≈ 0.293, both axes match.
    let scale_x = hw as f32 / tw as f32;
    let scale_y = hh as f32 / th as f32;

    // Walk the trees.bmp once. Stride depends on species — tropical pixels
    // produce denser placement so jungles look filled.
    let mut y = 0u32;
    while y < th {
        let mut x = 0u32;
        while x < tw {
            let pix_idx = (y * tw + x) as usize;
            let idx = tree_bmp.pixels[pix_idx];

            let Some(tree_type) = TreeBitmap::type_for(idx, tree_indices) else {
                x += 1;
                continue;
            };

            // Use species stride: 0/1 deciduous + conifer = forest_stride,
            // 2 tropical = jungle_stride.
            let stride = if tree_type == 2 {
                jungle_stride
            } else {
                forest_stride
            };
            // Skip if this pixel isn't on the species-grid.
            if stride > 1 && (x % stride != 0 || y % stride != 0) {
                x += 1;
                continue;
            }

            // Map this trees.bmp pixel to a heightmap pixel for Y / sea check.
            let hx = ((x as f32 + 0.5) * scale_x).floor() as u32;
            let hy = ((y as f32 + 0.5) * scale_y).floor() as u32;
            let hx = hx.min(hw - 1);
            let hy = hy.min(hh - 1);
            let raw_h = heightmap.pixels[(hy * hw + hx) as usize];
            if raw_h <= SEA_LEVEL {
                stats.skipped_below_sea += 1;
                x += 1;
                continue;
            }

            // Deterministic hash for jitter + scale + tint variety.
            let seed = hash2(x, y);
            // Jitter within ±0.5 trees.bmp pixel (= scale_x * 0.5 heightmap px).
            let jitter_tx = ((seed & 0xFF) as f32 / 255.0 - 0.5) * 0.6;
            let jitter_ty = (((seed >> 8) & 0xFF) as f32 / 255.0 - 0.5) * 0.6;
            let scale_n = ((seed >> 16) & 0xFF) as f32 / 255.0;
            let tint_n = ((seed >> 24) & 0xFF) as f32 / 255.0;

            // Convert (tree-pixel + jitter) → heightmap-pixel coordinate, then
            // → world (so XZ is in heightmap-pixel space scaled by world_scale).
            let final_hx = ((x as f32 + 0.5 + jitter_tx) * scale_x).clamp(0.0, (hw - 1) as f32);
            let final_hy = ((y as f32 + 0.5 + jitter_ty) * scale_y).clamp(0.0, (hh - 1) as f32);

            let world_x = final_hx * world_scale;
            let world_z = final_hy * world_scale;
            let world_y = (raw_h as f32 / 255.0) * height_scale;

            // Per-species base colour, modulated by tint_n.
            let base = match tree_type {
                0 => [44u8, 90, 38], // beech / temperate
                1 => [38, 78, 32],   // pine / boreal
                2 => [30, 100, 38],  // palm / jungle
                _ => [60, 60, 60],
            };
            let tn = (tint_n * 40.0) as i16 - 20;
            let tint = [
                (base[0] as i16 + tn).clamp(0, 255) as u8,
                (base[1] as i16 + tn).clamp(0, 255) as u8,
                (base[2] as i16 + (tn / 2)).clamp(0, 255) as u8,
                255,
            ];

            // Phase 3.10.1: shrink trees vs original ~3× (target the
            // roadmap's "比原版大 3.5×" reduction, NOT a full 50× squash by
            // WORLD_SCALE). Empirically:
            // * 0.06 (original) → trunk + canopy occupies ~0.7% map Z, too
            //   tall vs vanilla's ~0.24%.
            // * 0.06 × 0.02 (= world_scale) gave 0.001×, invisible.
            // * 0.020 (this constant, ≈ 0.06 / 3) lands close to 0.24%
            //   while still being a visible silhouette at moderate zoom.
            // Mesh y-range for vanilla beech is 1.21 mesh units, so
            // tree_height_world ≈ 1.21 × 0.020 × jitter[0.75..1.45]
            //                  ≈ 0.018..0.035 world units.
            let base_scale = match tree_type {
                0 => 0.020, // deciduous (beech)
                1 => 0.020, // conifer (pine)
                2 => 0.027, // tropical (palm — denser canopy → slightly bigger)
                _ => 0.018,
            };
            // Final per-tree scale = base × random jitter [0.75, 1.45].
            // (We DON'T multiply by world_scale here — the base constants
            // above are already calibrated for this project's compressed
            // world. See `scale_shrinks_with_world_scale` test note.)
            let scale = base_scale * (0.75 + scale_n * 0.7);

            // Phase 3.10.2: slope = central-difference on the heightmap.
            // dh/dx_pixel ≈ (h(x+1) - h(x-1)) / 2; ditto Z. We scale the
            // result so a 1-unit-tall tree slants by ~slope world units of
            // its own height — packed as signed byte ±127.
            let slope_x_world = sample_slope_x(heightmap, hx, hy, height_scale, world_scale);
            let slope_z_world = sample_slope_z(heightmap, hx, hy, height_scale, world_scale);
            let pack = |v: f32| -> i8 {
                // ±0.5 world Y per world XZ unit → maps to ±127.
                let clipped = (v * 254.0).clamp(-127.0, 127.0);
                clipped as i8
            };

            out.push(TreeInstance {
                pos: [world_x, world_y, world_z],
                scale,
                tint,
                tree_type,
                _pad0: 0,
                slope_x: pack(slope_x_world),
                slope_z: pack(slope_z_world),
            });
            stats.generated_instances_total += 1;
            if let Some(slot) = stats
                .generated_instances_by_type
                .get_mut(tree_type as usize)
            {
                *slot += 1;
            }

            x += stride.max(1);
        }
        // forest_stride drives Y also; jungle pixels we still hit row-by-row
        // because tropical isn't present everywhere — keeps temperate sparse
        // and tropical full.
        y += 1;
    }
    stats.skipped_by_stride = stats
        .active_pixels_total
        .saturating_sub(stats.generated_instances_total)
        .saturating_sub(stats.skipped_below_sea);
    (out, stats)
}

/// Cheap deterministic 2D hash → u32 (so jitter + tint stay stable across runs).
fn hash2(x: u32, y: u32) -> u32 {
    let mut h: u32 = x.wrapping_mul(0x9E3779B1);
    h ^= y.wrapping_mul(0x85EBCA6B);
    h = h.wrapping_mul(0xC2B2AE35);
    h ^= h >> 16;
    h
}

/// Central-difference ∂h/∂X in **world units**. `(hx, hy)` is the heightmap
/// pixel. Returns ratio: meters_y per meter_x (ish). Used as the i8-packed
/// `slope_x`.
fn sample_slope_x(h: &Heightmap, hx: u32, hy: u32, height_scale: f32, world_scale: f32) -> f32 {
    let w = h.width;
    let xl = hx.saturating_sub(1);
    let xr = (hx + 1).min(w - 1);
    let pl = h.pixels[(hy * w + xl) as usize] as f32 / 255.0;
    let pr = h.pixels[(hy * w + xr) as usize] as f32 / 255.0;
    // dh/dx_pixel = (pr - pl) / 2. World-X distance between sample points
    // = 2 * world_scale. World-Y change = (pr - pl) * height_scale.
    let dhx_world = ((pr - pl) * height_scale) / (2.0 * world_scale.max(1e-6));
    dhx_world
}

/// Central-difference ∂h/∂Z in **world units**.
fn sample_slope_z(h: &Heightmap, hx: u32, hy: u32, height_scale: f32, world_scale: f32) -> f32 {
    let w = h.width;
    let yh = h.height;
    let yu = hy.saturating_sub(1);
    let yd = (hy + 1).min(yh - 1);
    let pu = h.pixels[(yu * w + hx) as usize] as f32 / 255.0;
    let pd = h.pixels[(yd * w + hx) as usize] as f32 / 255.0;
    let dhz_world = ((pd - pu) * height_scale) / (2.0 * world_scale.max(1e-6));
    dhz_world
}

#[cfg(test)]
mod tests {
    use super::*;
    use hoi4_map::Heightmap;
    use std::collections::HashSet;

    fn flat_heightmap(w: u32, h: u32, value: u8) -> Heightmap {
        Heightmap {
            width: w,
            height: h,
            pixels: vec![value; (w * h) as usize],
        }
    }

    fn tree_bmp(w: u32, h: u32, value: u8) -> TreeBitmap {
        TreeBitmap {
            width: w,
            height: h,
            pixels: vec![value; (w * h) as usize],
            palette: [[0; 3]; 256],
        }
    }

    fn vanilla_indices() -> HashSet<u8> {
        hoi4_map::DEFAULT_TREE_INDICES.iter().copied().collect()
    }

    #[test]
    fn produces_no_trees_on_inactive_index() {
        let t = tree_bmp(64, 32, 254); // 254 = no tree
        let h = flat_heightmap(64, 32, 200);
        let trees = generate_trees(&t, &vanilla_indices(), &h, 0.02, 6.0, 4, 2);
        assert!(
            trees.is_empty(),
            "no-tree pixels should produce 0 trees, got {}",
            trees.len()
        );
    }

    #[test]
    fn forest_pixels_produce_trees() {
        let t = tree_bmp(64, 32, 3); // beech
        let h = flat_heightmap(64, 32, 200);
        let trees = generate_trees(&t, &vanilla_indices(), &h, 0.02, 6.0, 4, 2);
        // 64/4 × 32/4 = 16 × 8 = 128 cells, all forest above sea level → 128 trees.
        assert_eq!(trees.len(), 128);
        // All within world XZ bounds (with jitter) and tree_type 0.
        for tree in &trees {
            assert_eq!(tree.tree_type, 0);
            assert!(tree.pos[0] >= -1.0 && tree.pos[0] <= (64.0 * 0.02) + 1.0);
            assert!(tree.pos[2] >= -1.0 && tree.pos[2] <= (32.0 * 0.02) + 1.0);
        }
    }

    #[test]
    fn tree_distribution_stats_track_vanilla_mask_conversion() {
        let t = tree_bmp(64, 32, 3); // beech
        let h = flat_heightmap(64, 32, 200);
        let (trees, stats) = generate_trees_with_stats(&t, &vanilla_indices(), &h, 0.02, 6.0, 4, 2);
        assert_eq!(trees.len(), 128);
        assert_eq!(stats.tree_bitmap_size, [64, 32]);
        assert_eq!(stats.heightmap_size, [64, 32]);
        assert_eq!(stats.active_pixels_total, 64 * 32);
        assert_eq!(stats.active_pixels_by_type, [64 * 32, 0, 0]);
        assert_eq!(stats.generated_instances_total, 128);
        assert_eq!(stats.generated_instances_by_type, [128, 0, 0]);
        assert_eq!(stats.skipped_by_stride, 64 * 32 - 128);
        assert_eq!(stats.skipped_below_sea, 0);
        assert!((stats.placement_ratio() - 128.0 / (64.0 * 32.0)).abs() < 1e-6);
    }

    #[test]
    fn skips_below_sea_level() {
        let t = tree_bmp(64, 32, 3);
        let h = flat_heightmap(64, 32, 50); // below SEA_LEVEL=95
        let trees = generate_trees(&t, &vanilla_indices(), &h, 0.02, 6.0, 4, 2);
        assert!(
            trees.is_empty(),
            "underwater forests should produce no trees"
        );
    }

    #[test]
    fn palette_index_drives_tree_type() {
        // Build 4 zones: cols 0..16=3 (beech), 16..32=4 (mixed), 32..48=7 (pine), 48..64=10 (palm)
        let mut pixels = vec![0u8; 64 * 8];
        for y in 0..8 {
            for x in 0..16 {
                pixels[y * 64 + x] = 3;
            }
            for x in 16..32 {
                pixels[y * 64 + x] = 4;
            }
            for x in 32..48 {
                pixels[y * 64 + x] = 7;
            }
            for x in 48..64 {
                pixels[y * 64 + x] = 10;
            }
        }
        let t = TreeBitmap {
            width: 64,
            height: 8,
            pixels,
            palette: [[0; 3]; 256],
        };
        let h = flat_heightmap(64, 8, 200);
        let trees = generate_trees(&t, &vanilla_indices(), &h, 0.02, 6.0, 1, 1);
        let zone_for = |x: f32| {
            let px = x / 0.02;
            if px < 16.0 {
                0
            } else if px < 32.0 {
                0
            } else if px < 48.0 {
                1
            } else {
                2
            }
        };
        for tree in &trees {
            let expected = zone_for(tree.pos[0]);
            assert_eq!(
                tree.tree_type, expected,
                "tree at world_x {:.2} should be species {}",
                tree.pos[0], expected
            );
        }
    }

    #[test]
    fn jungle_denser_than_forest() {
        // Same area, beech vs palm → palm should be ~jungle_stride² less
        // restricted than beech's forest_stride².
        let t_b = tree_bmp(32, 32, 3); // beech
        let t_p = tree_bmp(32, 32, 10); // palm
        let h = flat_heightmap(32, 32, 200);
        let n_forest = generate_trees(&t_b, &vanilla_indices(), &h, 0.02, 6.0, 4, 2).len();
        let n_jungle = generate_trees(&t_p, &vanilla_indices(), &h, 0.02, 6.0, 4, 2).len();
        // forest stride 4 → 32/4 × 32/4 = 64; palm stride 2 → 32/2 × 32/2 = 256
        assert!(
            n_jungle > n_forest * 2,
            "jungle (palm/stride 2) should be denser than forest (beech/stride 4); got jungle={} forest={}",
            n_jungle, n_forest
        );
    }

    #[test]
    fn instance_size_is_24_bytes() {
        // GPU layout assumption.
        assert_eq!(std::mem::size_of::<TreeInstance>(), 24);
    }

    #[test]
    fn deterministic_hash() {
        // Same coords → same hash, different coords → different hash.
        assert_eq!(hash2(7, 9), hash2(7, 9));
        assert_ne!(hash2(7, 9), hash2(7, 10));
        assert_ne!(hash2(7, 9), hash2(8, 9));
    }

    #[test]
    fn slope_packs_in_correct_direction() {
        // Build a 1D ramp heightmap: pixels[x] increases with x.
        let mut pixels = vec![0u8; 32 * 8];
        for y in 0..8 {
            for x in 0..32 {
                pixels[y * 32 + x] = (x * 8) as u8; // 0..248
            }
        }
        let h = Heightmap {
            width: 32,
            height: 8,
            pixels,
        };
        let t = tree_bmp(32, 8, 3); // all beech
        let trees = generate_trees(&t, &vanilla_indices(), &h, 0.02, 6.0, 1, 1);
        // Ramp goes +X → slope_x must be > 0 for every tree, slope_z ≈ 0.
        // Drop trees on the very left edge (x=0) where the central-difference
        // is asymmetric and can be 0.
        for tr in trees.iter().filter(|t| t.pos[0] > 0.05) {
            assert!(
                tr.slope_x > 0,
                "ramp going +x must give positive slope_x at world_x={}",
                tr.pos[0]
            );
        }
    }

    #[test]
    fn scale_independent_of_world_scale() {
        // Phase 3.10.1: tree size is calibrated by the `base_scale` constants
        // directly, **not** by `world_scale`. Two runs with different
        // `world_scale` must produce the same `inst.scale` (they only differ
        // in tree XZ position).
        let t = tree_bmp(16, 16, 3);
        let h = flat_heightmap(16, 16, 200);
        let trees_a = generate_trees(&t, &vanilla_indices(), &h, 0.02, 6.0, 1, 1);
        let trees_b = generate_trees(&t, &vanilla_indices(), &h, 0.04, 6.0, 1, 1);
        assert_eq!(trees_a.len(), trees_b.len());
        for (a, b) in trees_a.iter().zip(trees_b.iter()) {
            assert!(
                (a.scale - b.scale).abs() < 1e-6,
                "scale must not depend on world_scale; got a.scale={}, b.scale={}",
                a.scale,
                b.scale
            );
        }
    }
}
