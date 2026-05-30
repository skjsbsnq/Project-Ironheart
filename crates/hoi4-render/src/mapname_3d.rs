//! Phase 3.10.3 — 3D country-name labels.
//!
//! Replaces the screen-space HUD path (centroid → view-projection → fontdue
//! atlas at fixed 28 px) with a proper 3D pass:
//!
//! 1. Each country gets an oriented bounding box (OBB) computed by 2D PCA
//!    over the pixels of its owned provinces — major axis = "long way" of
//!    the country, perpendicular = "short way".
//! 2. A pre-baked R8 atlas (one entry per country, bake done by `hoi4-app`
//!    via fontdue) provides the rasterised text + outline for the label.
//! 3. Per-country `CountryNameInstance` carries world centre, world half-
//!    extents, the major-axis direction, and the atlas UV rect. The vertex
//!    shader spreads 6 verts into a quad along axis1 / axis2; the fragment
//!    shader samples the atlas (alpha) and applies day/night dimming + an
//!    approximate text/outline split.
//!
//! Atlas baking lives in the binary crate (`hoi4-app::mapname_atlas`) so
//! `hoi4-render` doesn't pull a TTF rasteriser into its dependency tree.

use hoi4_map::ProvinceMap;

/// A single country's oriented bounding box (in **pixel space**: same
/// coordinates as `ProvinceMap`).
#[derive(Debug, Clone, Copy)]
pub struct CountryObb {
    /// Pixel centroid of all owned-province pixels.
    pub centroid_px: (f32, f32),
    /// Major-axis direction in pixel space (unit length).
    pub axis1_dir: (f32, f32),
    /// Half-extent along axis1 (pixels). Approx 2σ of pixel distribution
    /// projected onto axis1 — gives a quad that comfortably fits the country
    /// without overlapping into neighbours.
    pub half_extent_1: f32,
    /// Half-extent along axis2 (perpendicular to axis1, in the XZ plane).
    pub half_extent_2: f32,
    /// Number of pixels owned by this country (used for LOD culling).
    pub pixel_count: u32,
}

/// Compute one OBB per country in `0..country_count` from the per-pixel
/// `province_map`. `province_owner_idx[pid]` is the owning country index for
/// province `pid` (or `None`).
///
/// Single pass O(map_pixels). On 5632×2048 vanilla maps this is ~11M pixels,
/// runs in <250 ms in release on a modern CPU. Output is sized
/// `country_count`; entries with no pixels are `None`.
pub fn compute_country_obbs(
    province_map: &ProvinceMap,
    province_owner_idx: &[Option<usize>],
    province_is_core: &[bool],
    country_count: usize,
) -> Vec<Option<CountryObb>> {
    if country_count == 0 {
        return Vec::new();
    }
    // Use f64 accumulators because (5632 * 2048)² overflows f32 mantissa
    // when summing (x*x) for big countries.
    #[derive(Default, Clone)]
    struct Acc {
        count: u64,
        sum_x: f64,
        sum_y: f64,
        sum_xx: f64,
        sum_yy: f64,
        sum_xy: f64,
    }
    let mut sums: Vec<Acc> = vec![Acc::default(); country_count];

    let w = province_map.width as usize;
    let h = province_map.height as usize;
    let owners_len = province_owner_idx.len();
    for y in 0..h {
        let yoff = y * w;
        for x in 0..w {
            let pid = province_map.pixels[yoff + x] as usize;
            if pid >= owners_len {
                continue;
            }
            if pid < province_is_core.len() && !province_is_core[pid] {
                continue;
            }
            let Some(owner_idx) = province_owner_idx[pid] else {
                continue;
            };
            if owner_idx >= country_count {
                continue;
            }
            let acc = &mut sums[owner_idx];
            acc.count += 1;
            let xf = x as f64;
            let yf = y as f64;
            acc.sum_x += xf;
            acc.sum_y += yf;
            acc.sum_xx += xf * xf;
            acc.sum_yy += yf * yf;
            acc.sum_xy += xf * yf;
        }
    }

    sums.into_iter()
        .map(|acc| {
            if acc.count < 4 {
                return None;
            }
            let n = acc.count as f64;
            let mx = acc.sum_x / n;
            let my = acc.sum_y / n;
            // Sample covariance (population is fine here — we want the
            // distribution shape, not an unbiased estimator).
            let cxx = acc.sum_xx / n - mx * mx;
            let cyy = acc.sum_yy / n - my * my;
            let cxy = acc.sum_xy / n - mx * my;

            // 2x2 eigenvalues: λ = (trace ± √(trace² - 4·det)) / 2.
            let trace = cxx + cyy;
            let det = cxx * cyy - cxy * cxy;
            let disc = (trace * trace * 0.25 - det).max(0.0).sqrt();
            let lam1 = trace * 0.5 + disc; // major
            let lam2 = (trace * 0.5 - disc).max(0.0); // minor

            // Eigenvector for λ₁: (cxy, λ₁ − cxx), normalised. If cxy ≈ 0
            // the matrix is already diagonal — pick whichever axis has the
            // larger variance.
            let (mut vx, mut vy) = if cxy.abs() > 1e-6 {
                let dx = cxy;
                let dy = lam1 - cxx;
                let mag = (dx * dx + dy * dy).sqrt();
                if mag > 1e-9 {
                    (dx / mag, dy / mag)
                } else {
                    (1.0, 0.0)
                }
            } else if cxx >= cyy {
                (1.0, 0.0)
            } else {
                (0.0, 1.0)
            };

            // PCA eigenvectors have a free sign — half the countries would
            // otherwise come out with axis1 pointing "leftward" and the
            // baked text strip would render mirrored. Pick the
            // canonical-positive direction: prefer +x; if vx ≈ 0, prefer
            // +y. This is purely a presentation choice — the OBB extents
            // are identical either way.
            if vx < -1e-9 || (vx.abs() < 1e-9 && vy < 0.0) {
                vx = -vx;
                vy = -vy;
            }

            // Half-extent: ~1.5σ along each axis. Earlier 2σ covered ~95%
            // of pixels which made labels span almost the entire country
            // and overlap into neighbours. 1.5σ ≈ 87% — labels stay inside
            // the core territory while still aligning along the long axis.
            let half_ext_1 = lam1.sqrt() * 1.5;
            let half_ext_2 = lam2.sqrt() * 1.5;
            Some(CountryObb {
                centroid_px: (mx as f32, my as f32),
                axis1_dir: (vx as f32, vy as f32),
                half_extent_1: half_ext_1 as f32,
                half_extent_2: half_ext_2 as f32,
                pixel_count: acc.count as u32,
            })
        })
        .collect()
}

/// GPU-bound per-instance label data. Layout matches the wgsl pipeline
/// expectations exactly (48 bytes, every field starts at a 4-byte boundary).
///
/// Vertex format mapping (instance-rate):
/// * `center`         Float32x3 @  0
/// * `width_world`    Float32   @ 12
/// * `axis1`          Float32x2 @ 16
/// * `height_world`   Float32   @ 24
/// * `_pad0`          Float32   @ 28 (kept zero — SoA alignment slot)
/// * `uv_min`         Float32x2 @ 32
/// * `uv_max`         Float32x2 @ 40
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable, Debug, Default)]
pub struct CountryNameInstance {
    /// World-space centre of the label (X, Y, Z). Y is typically a small
    /// constant lift above terrain so the quad doesn't z-fight the heightmap
    /// mesh.
    pub center: [f32; 3],
    /// World-space half-extent along `axis1` (= half the rendered text
    /// width). Determines the long axis of the visible quad.
    pub width_world: f32,
    /// Major-axis direction in world XZ plane (unit length).
    pub axis1: [f32; 2],
    /// World-space half-extent along axis2 (perpendicular to axis1). Pinned
    /// to atlas aspect ratio to avoid stretching text.
    pub height_world: f32,
    pub _pad0: f32,
    /// Atlas UV minimum (top-left of this country's letter strip).
    pub uv_min: [f32; 2],
    /// Atlas UV maximum (bottom-right).
    pub uv_max: [f32; 2],
}

const _: () = assert!(std::mem::size_of::<CountryNameInstance>() == 48);

#[cfg(test)]
mod tests {
    use super::*;
    use hoi4_map::ProvinceMap;

    fn map_with_pids(width: u32, height: u32, pids: Vec<u16>) -> ProvinceMap {
        assert_eq!(pids.len(), (width * height) as usize);
        ProvinceMap {
            width,
            height,
            pixels: pids,
        }
    }

    #[test]
    fn empty_country_count() {
        let pmap = map_with_pids(4, 2, vec![0; 8]);
        let obbs = compute_country_obbs(&pmap, &[], &[], 0);
        assert!(obbs.is_empty());
    }

    #[test]
    fn single_uniform_block_country() {
        // 8×4 map, all pixels belong to province 1 owned by country 0.
        let pmap = map_with_pids(8, 4, vec![1; 32]);
        let owners = vec![None, Some(0usize)];
        let is_core = vec![true, true];
        let obbs = compute_country_obbs(&pmap, &owners, &is_core, 1);
        let obb = obbs[0].expect("country 0 should have an OBB");
        // Centroid = (3.5, 1.5)
        assert!((obb.centroid_px.0 - 3.5).abs() < 1e-3);
        assert!((obb.centroid_px.1 - 1.5).abs() < 1e-3);
        // Wider in X than Y → axis1 should be (±1, 0).
        assert!(obb.axis1_dir.0.abs() > 0.99);
        assert!(obb.axis1_dir.1.abs() < 0.05);
        assert!(obb.half_extent_1 > obb.half_extent_2);
    }

    #[test]
    fn diagonal_strip_picks_diagonal_axis() {
        // 5×5 map with provinces along the main diagonal (pid 1).
        let mut pids = vec![0u16; 25];
        for i in 0..5 {
            pids[i * 5 + i] = 1;
        }
        let pmap = map_with_pids(5, 5, pids);
        let owners = vec![None, Some(0usize)];
        let is_core = vec![true, true];
        let obbs = compute_country_obbs(&pmap, &owners, &is_core, 1);
        let obb = obbs[0].unwrap();
        // axis1 should point along (1, 1) / √2 — both components |val| > 0.6
        // (allowing some 5-sample noise).
        let ax = obb.axis1_dir.0.abs();
        let ay = obb.axis1_dir.1.abs();
        assert!(ax > 0.6, "expected axis1.x large, got {}", ax);
        assert!(ay > 0.6, "expected axis1.y large, got {}", ay);
    }

    #[test]
    fn skips_countries_with_too_few_pixels() {
        // pid 1 → owner 0 with only 2 pixels — below the 4-pixel floor.
        let mut pids = vec![0u16; 16];
        pids[0] = 1;
        pids[15] = 1;
        let pmap = map_with_pids(4, 4, pids);
        let owners = vec![None, Some(0usize)];
        let is_core = vec![true, true];
        let obbs = compute_country_obbs(&pmap, &owners, &is_core, 1);
        assert!(obbs[0].is_none());
    }

    #[test]
    fn unowned_provinces_ignored() {
        // pid 1 covers everything but owner is None → no OBB anywhere.
        let pmap = map_with_pids(4, 4, vec![1; 16]);
        let owners: Vec<Option<usize>> = vec![None, None];
        let is_core = vec![true, true];
        let obbs = compute_country_obbs(&pmap, &owners, &is_core, 2);
        assert!(obbs.iter().all(|o| o.is_none()));
    }

    #[test]
    fn instance_size_is_48_bytes() {
        // Layout invariant — pipeline assumes 48-byte stride.
        assert_eq!(std::mem::size_of::<CountryNameInstance>(), 48);
    }

    #[test]
    fn axis1_dir_is_canonical_positive_x() {
        // Build a tilted thin diagonal country going /from/ low-x-high-y /to/ high-x-low-y
        // (i.e., the natural eigenvector could come out as (-1, 1)/√2 OR (1, -1)/√2;
        // we want the canonical one with vx ≥ 0).
        let mut pids = vec![0u16; 49];
        for i in 0..7 {
            // Diagonal from (0, 6) to (6, 0): vy decreases as x increases.
            pids[(6 - i) * 7 + i] = 1;
        }
        let pmap = map_with_pids(7, 7, pids);
        let owners = vec![None, Some(0usize)];
        let is_core = vec![true, true];
        let obbs = compute_country_obbs(&pmap, &owners, &is_core, 1);
        let obb = obbs[0].unwrap();
        // Canonical pick: vx ≥ 0 (text reads rightward). For this diagonal
        // axis1 ∝ (1, -1)/√2.
        assert!(
            obb.axis1_dir.0 >= 0.0,
            "axis1.x should be canonical-positive, got {}",
            obb.axis1_dir.0
        );
    }

    #[test]
    fn axis1_canonical_when_purely_vertical() {
        // 5×5 map, country occupies the central column (vertical strip).
        // Eigenvector along +Y or -Y are both valid; we want canonical +Y.
        let mut pids = vec![0u16; 25];
        for y in 0..5 {
            pids[y * 5 + 2] = 1;
        }
        let pmap = map_with_pids(5, 5, pids);
        let owners = vec![None, Some(0usize)];
        let is_core = vec![true, true];
        let obbs = compute_country_obbs(&pmap, &owners, &is_core, 1);
        let obb = obbs[0].unwrap();
        // axis1 ≈ (0, ±1); canonical pick: +y.
        assert!(
            obb.axis1_dir.1 >= 0.0,
            "axis1.y should be canonical-positive when vx≈0, got {:?}",
            obb.axis1_dir
        );
    }

    #[test]
    fn non_core_province_excluded() {
        // 4×4 map: province 1 (core, owned by country 0) in left half,
        // province 2 (non-core, owned by country 0) in right half.
        // With is_core filtering, only the left 8 pixels should contribute.
        let mut pids = vec![0u16; 16];
        for y in 0..4 {
            for x in 0..2 {
                pids[y * 4 + x] = 1; // core province
            }
            for x in 2..4 {
                pids[y * 4 + x] = 2; // non-core province
            }
        }
        let pmap = map_with_pids(4, 4, pids);
        let owners = vec![None, Some(0usize), Some(0usize)];
        let is_core = vec![true, true, false]; // pid 2 is not core
        let obbs = compute_country_obbs(&pmap, &owners, &is_core, 1);
        let obb = obbs[0].expect("country 0 should have an OBB from core provinces only");
        // Centroid should be at x=0.5 (only left half contributes: x in 0,1 → avg 0.5)
        assert!(
            (obb.centroid_px.0 - 0.5).abs() < 0.1,
            "centroid_x should be ~0.5, got {}",
            obb.centroid_px.0
        );
        // Centroid y should be 1.5 (all 4 rows)
        assert!(
            (obb.centroid_px.1 - 1.5).abs() < 0.1,
            "centroid_y should be ~1.5, got {}",
            obb.centroid_px.1
        );
    }
}
