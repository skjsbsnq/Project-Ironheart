//! Pre-computed signed distance fields for province and country borders.
//!
//! At startup we run a **two-pass Chamfer distance transform** over the
//! `province_map` (top-down), producing one `u8` value per pixel: the
//! pixel distance to the nearest border, clamped to 255.
//!
//! Two fields are produced:
//! * **province SDF** — distance to the nearest pixel where the
//!   neighbouring province ID is different.
//! * **country SDF** — same, but the neighbour predicate uses province
//!   ownership (`controllers` array, since occupied territory paints the
//!   *controller* country's border colour in HOI4).
//!
//! At ~5632×2048 = 11.5 M pixels this takes ~0.5 s in release / ~3 s in
//! debug. The two `Vec<u8>`s become R8Unorm textures sampled in the
//! fragment shader for soft border rendering.
//!
//! ## Algorithm
//! Chamfer 4-distance:
//! 1. Pass 0: every border pixel gets distance 0; everything else `MAX`.
//! 2. Pass 1 (forward TL → BR): `d[x,y] = min(d, d[x-1,y]+1, d[x,y-1]+1)`.
//! 3. Pass 2 (backward BR → TL): `d[x,y] = min(d, d[x+1,y]+1, d[x,y+1]+1)`.
//! 4. Clamp to 255 and downcast to `u8`.
//!
//! This gives a manhattan-ish distance which is plenty for soft borders.

use hoi4_map::ProvinceMap;
use hoi4_state::CountryId;

/// Compute a clamped distance field. `border_at(x, y)` should return
/// `true` for any pixel that sits on a border (one or more 4-neighbours
/// differ from this pixel under the desired predicate).
///
/// `width` × `height` must match the input pixel grid. Output length is
/// `width * height`.
pub fn chamfer_distance_field(
    width: u32,
    height: u32,
    border_at: impl Fn(u32, u32) -> bool,
) -> Vec<u8> {
    let w = width as usize;
    let h = height as usize;
    let mut dist = vec![u16::MAX; w * h];

    // Pass 0: mark border pixels.
    for y in 0..h {
        for x in 0..w {
            if border_at(x as u32, y as u32) {
                dist[y * w + x] = 0;
            }
        }
    }

    // Pass 1: forward sweep (TL → BR).
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let mut d = dist[i];
            if x > 0 {
                d = d.min(dist[i - 1].saturating_add(1));
            }
            if y > 0 {
                d = d.min(dist[i - w].saturating_add(1));
            }
            dist[i] = d;
        }
    }

    // Pass 2: backward sweep (BR → TL).
    for y in (0..h).rev() {
        for x in (0..w).rev() {
            let i = y * w + x;
            let mut d = dist[i];
            if x + 1 < w {
                d = d.min(dist[i + 1].saturating_add(1));
            }
            if y + 1 < h {
                d = d.min(dist[i + w].saturating_add(1));
            }
            dist[i] = d;
        }
    }

    dist.into_iter().map(|d| d.min(255) as u8).collect()
}

/// Province-border distance: neighbour pixel has different province ID.
pub fn compute_province_sdf(province_map: &ProvinceMap) -> Vec<u8> {
    let w = province_map.width;
    let h = province_map.height;
    let pixels = &province_map.pixels;
    chamfer_distance_field(w, h, |x, y| {
        let me = pixels[(y * w + x) as usize];
        // 4-neighbour check
        if x + 1 < w && pixels[(y * w + x + 1) as usize] != me {
            return true;
        }
        if y + 1 < h && pixels[((y + 1) * w + x) as usize] != me {
            return true;
        }
        if x > 0 && pixels[(y * w + x - 1) as usize] != me {
            return true;
        }
        if y > 0 && pixels[((y - 1) * w + x) as usize] != me {
            return true;
        }
        false
    })
}

/// Country-border distance: neighbour pixel has different province *controller*.
/// Sea↔land doesn't count as a border (controller of sea is `NONE`, land may be
/// owned, but we don't want a black line around every coastline — the SDF would
/// be dominated by it). So borders are only marked when both pixels are owned
/// AND owners differ. Coastline gets the natural land/water tinting from the
/// terrain palette instead.
pub fn compute_country_sdf(province_map: &ProvinceMap, controllers: &[CountryId]) -> Vec<u8> {
    let w = province_map.width;
    let h = province_map.height;
    let pixels = &province_map.pixels;

    let owner_of = |pid: u16| -> CountryId {
        controllers
            .get(pid as usize)
            .copied()
            .unwrap_or(CountryId::NONE)
    };

    chamfer_distance_field(w, h, |x, y| {
        let me = pixels[(y * w + x) as usize];
        let oa = owner_of(me);
        if oa.is_none() {
            return false;
        }
        let neighbour_differs = |nid_idx: usize| -> bool {
            let n_pid = pixels[nid_idx];
            let ob = owner_of(n_pid);
            !ob.is_none() && ob != oa
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
/// Coast distance: neighbour pixel is on the *other side of sea level*.
/// Operates directly on the heightmap pixel array — no province IDs needed.
/// Returns clamped pixel distance to the nearest land↔water boundary.
///
/// `sea_level` is the threshold in raw heightmap units (vanilla ≈ 95 / 255).
pub fn compute_coast_sdf(heightmap: &hoi4_map::Heightmap, sea_level: u8) -> Vec<u8> {
    let w = heightmap.width;
    let h = heightmap.height;
    let pixels = &heightmap.pixels;
    let is_land = |idx: usize| pixels[idx] > sea_level;
    chamfer_distance_field(w, h, |x, y| {
        let me = is_land((y * w + x) as usize);
        if x + 1 < w && is_land((y * w + x + 1) as usize) != me {
            return true;
        }
        if y + 1 < h && is_land(((y + 1) * w + x) as usize) != me {
            return true;
        }
        if x > 0 && is_land((y * w + x - 1) as usize) != me {
            return true;
        }
        if y > 0 && is_land(((y - 1) * w + x) as usize) != me {
            return true;
        }
        false
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use hoi4_map::ProvinceMap;

    fn pmap_from_grid(grid: &[&[u16]]) -> ProvinceMap {
        let h = grid.len() as u32;
        let w = grid[0].len() as u32;
        let mut pixels = Vec::with_capacity((w * h) as usize);
        for row in grid {
            assert_eq!(row.len() as u32, w);
            pixels.extend_from_slice(row);
        }
        ProvinceMap {
            width: w,
            height: h,
            pixels,
        }
    }

    #[test]
    fn province_sdf_zero_on_border_increasing_inward() {
        // 6×3 grid: left half province 1, right half province 2.
        let pmap = pmap_from_grid(&[
            &[1, 1, 1, 2, 2, 2],
            &[1, 1, 1, 2, 2, 2],
            &[1, 1, 1, 2, 2, 2],
        ]);
        let sdf = compute_province_sdf(&pmap);
        // Column 2 (last province-1 col) and column 3 (first province-2 col) are borders → 0
        assert_eq!(sdf[0 * 6 + 2], 0);
        assert_eq!(sdf[0 * 6 + 3], 0);
        // Column 0 is 2 pixels from border → 2
        assert_eq!(sdf[0 * 6 + 0], 2);
        // Column 5 is 2 pixels from border → 2
        assert_eq!(sdf[0 * 6 + 5], 2);
    }

    #[test]
    fn province_sdf_constant_for_uniform_map() {
        // 4×4 all the same province → no borders at all → distances saturate.
        let pmap = pmap_from_grid(&[&[5, 5, 5, 5], &[5, 5, 5, 5], &[5, 5, 5, 5], &[5, 5, 5, 5]]);
        let sdf = compute_province_sdf(&pmap);
        for &d in &sdf {
            assert_eq!(d, 255);
        }
    }

    #[test]
    fn country_sdf_ignores_unowned_neighbours() {
        // Map: province 1 (owned by A), province 2 (unowned), province 3 (owned by B)
        let pmap = pmap_from_grid(&[&[1, 1, 2, 3, 3], &[1, 1, 2, 3, 3]]);
        let mut controllers = vec![CountryId::NONE; 10];
        controllers[1] = CountryId(0); // A
        controllers[2] = CountryId::NONE; // unowned
        controllers[3] = CountryId(1); // B

        let sdf = compute_country_sdf(&pmap, &controllers);
        // Province 1 ↔ province 2 boundary: neighbour is unowned so NOT a country border
        // → those pixels should all have dist > 0. (None of the cells are border pixels.)
        for &d in &sdf {
            assert!(
                d > 0,
                "expected no country borders when one side unowned, got dist={d}"
            );
        }
    }

    #[test]
    fn country_sdf_marks_owned_vs_owned_border() {
        // 1 ↔ 3 directly adjacent, both owned (different countries) → border.
        let pmap = pmap_from_grid(&[&[1, 1, 3, 3], &[1, 1, 3, 3]]);
        let mut controllers = vec![CountryId::NONE; 5];
        controllers[1] = CountryId(0);
        controllers[3] = CountryId(1);

        let sdf = compute_country_sdf(&pmap, &controllers);
        // Pixels at columns 1 & 2 are on the border
        assert_eq!(sdf[0 * 4 + 1], 0);
        assert_eq!(sdf[0 * 4 + 2], 0);
        // Column 0 → distance 1
        assert_eq!(sdf[0 * 4 + 0], 1);
    }

    #[test]
    fn chamfer_field_dimensions() {
        let v = chamfer_distance_field(7, 5, |_, _| false);
        assert_eq!(v.len(), 7 * 5);
        // No borders at all → all distances saturate to 255
        assert!(v.iter().all(|&d| d == 255));
    }

    #[test]
    fn coast_sdf_zero_at_land_water_boundary() {
        // 6×2 heightmap, half land (200), half water (50). Sea level 95.
        let heightmap = hoi4_map::Heightmap {
            width: 6,
            height: 2,
            pixels: vec![50, 50, 50, 200, 200, 200, 50, 50, 50, 200, 200, 200],
        };
        let sdf = compute_coast_sdf(&heightmap, 95);
        // The two pixels on either side of the boundary (cols 2 and 3) are on the coast → 0
        assert_eq!(sdf[0 * 6 + 2], 0);
        assert_eq!(sdf[0 * 6 + 3], 0);
        // Far end (col 0, far water) → 2 px from coast
        assert_eq!(sdf[0 * 6 + 0], 2);
    }
}
