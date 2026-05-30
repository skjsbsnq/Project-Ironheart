use hoi4_map::definition::ProvinceType;
use hoi4_map::ProvinceMap;

#[derive(Debug, Clone, Copy)]
pub struct ProvinceLabel {
    pub province_id: u16,
    pub centroid_px: (f32, f32),
    pub axis1_dir: (f32, f32),
    pub half_extent_1: f32,
    pub half_extent_2: f32,
    pub pixel_count: u32,
    pub is_land: bool,
}

pub fn compute_province_labels(
    province_map: &ProvinceMap,
    definitions: &[Option<hoi4_map::definition::ProvinceDefinition>],
    max_provinces: usize,
) -> Vec<Option<ProvinceLabel>> {
    let max_id = province_map.pixels.iter().copied().max().unwrap_or(0) as usize;

    #[derive(Default, Clone)]
    struct Acc {
        count: u64,
        sum_x: f64,
        sum_y: f64,
        sum_xx: f64,
        sum_yy: f64,
        sum_xy: f64,
    }

    let mut sums: Vec<Acc> = vec![Acc::default(); max_id + 1];
    let w = province_map.width as usize;
    let h = province_map.height as usize;

    for y in 0..h {
        let yoff = y * w;
        for x in 0..w {
            let id = province_map.pixels[yoff + x] as usize;
            if id == 0 || id > max_id {
                continue;
            }
            let acc = &mut sums[id];
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

    let mut labels: Vec<Option<ProvinceLabel>> = vec![None; max_id + 1];
    let mut land_count = 0usize;

    for id in 1..=max_id {
        let acc = &sums[id];
        if acc.count < 4 {
            continue;
        }

        let is_land = if id < definitions.len() {
            definitions[id]
                .as_ref()
                .map(|d| d.province_type == ProvinceType::Land)
                .unwrap_or(false)
        } else {
            false
        };

        if !is_land {
            continue;
        }

        let n = acc.count as f64;
        let mx = acc.sum_x / n;
        let my = acc.sum_y / n;
        let cxx = acc.sum_xx / n - mx * mx;
        let cyy = acc.sum_yy / n - my * my;
        let cxy = acc.sum_xy / n - mx * my;

        let trace = cxx + cyy;
        let det = cxx * cyy - cxy * cxy;
        let disc = (trace * trace * 0.25 - det).max(0.0).sqrt();
        let lam1 = trace * 0.5 + disc;
        let lam2 = (trace * 0.5 - disc).max(0.0);

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

        if vx < -1e-9 || (vx.abs() < 1e-9 && vy < 0.0) {
            vx = -vx;
            vy = -vy;
        }

        let half_ext_1 = lam1.sqrt() * 1.2f64;
        let half_ext_2 = lam2.sqrt() * 1.2f64;

        labels[id] = Some(ProvinceLabel {
            province_id: id as u16,
            centroid_px: (mx as f32, my as f32),
            axis1_dir: (vx as f32, vy as f32),
            half_extent_1: half_ext_1 as f32,
            half_extent_2: half_ext_2 as f32,
            pixel_count: acc.count as u32,
            is_land,
        });

        land_count += 1;
        if land_count >= max_provinces {
            break;
        }
    }

    labels
}
