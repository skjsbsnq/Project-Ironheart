use std::collections::{HashMap, HashSet, VecDeque};

use glam::{Vec3, Vec3Swizzles};

const WORLD_SCALE: f32 = 0.02;
const HEIGHT_SCALE: f32 = 1.45;

#[derive(Debug, Clone, Copy)]
struct VisualPathPoint {
    pos: Vec3,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ProvincePixelBounds {
    min_x: usize,
    min_y: usize,
    max_x: usize,
    max_y: usize,
}

impl ProvincePixelBounds {
    fn new(x: usize, y: usize) -> Self {
        Self {
            min_x: x,
            min_y: y,
            max_x: x,
            max_y: y,
        }
    }

    fn include(&mut self, x: usize, y: usize) {
        self.min_x = self.min_x.min(x);
        self.min_y = self.min_y.min(y);
        self.max_x = self.max_x.max(x);
        self.max_y = self.max_y.max(y);
    }
}

pub fn collect_order_arrows(
    world: &hoi4_state::World,
    player_country: usize,
    show_all_units: bool,
    hide_ai_frontlines: bool,
    frontline_overlay_visible: bool,
    selected_divisions: &[usize],
    painter_samples: &[hoi4_state::ProvinceId],
    painter_frontline_owner: Option<hoi4_state::CountryId>,
    centroids: &[(f32, f32)],
    province_bounds: &[Option<ProvincePixelBounds>],
) -> Vec<crate::passes::maparrow::ArrowInstance> {
    let mut instances = Vec::new();
    let player_cid = hoi4_state::CountryId(player_country as u16);
    let heightmap = &world.map.heightmap;
    let province_map = &world.map.province_map;
    let hm_w = heightmap.width;
    let hm_h = heightmap.height;
    let height_x_scale = if province_map.width > 1 && hm_w > 1 {
        (hm_w - 1) as f32 / (province_map.width - 1) as f32
    } else {
        1.0
    };
    let height_y_scale = if province_map.height > 1 && hm_h > 1 {
        (hm_h - 1) as f32 / (province_map.height - 1) as f32
    } else {
        1.0
    };
    let sample_y = |px: f32, py: f32| -> f32 {
        let hx = (px * height_x_scale).clamp(0.0, hm_w.saturating_sub(1) as f32);
        let hy = (py * height_y_scale).clamp(0.0, hm_h.saturating_sub(1) as f32);
        let xi = hx as u32;
        let yi = hy as u32;
        let raw = heightmap.pixels[(yi * hm_w + xi) as usize] as f32 / 255.0;
        raw * HEIGHT_SCALE
    };
    let sample_y_world = |wx: f32, wz: f32| -> f32 {
        let px = (wx / WORLD_SCALE).clamp(0.0, province_map.width.saturating_sub(1) as f32);
        let py = (wz / WORLD_SCALE).clamp(0.0, province_map.height.saturating_sub(1) as f32);
        sample_y(px, py)
    };
    let frontline_width = 0.02;
    let arrow_width = 0.017;
    let move_arrow_width = 0.012;
    let preview_width = 0.012;
    let min_visual_segment = 0.045;
    let max_terrain_segment = 0.08;
    let max_line_points = 2048;

    for army in &world.player_armies {
        if army.owner != player_cid && !show_all_units {
            continue;
        }
        if army.owner != player_cid && hide_ai_frontlines {
            continue;
        }
        if army.owner != player_cid && !frontline_overlay_visible {
            continue;
        }
        if let Some(ref order) = army.order {
            let path = &order.path;
            if path.len() >= 2 {
                let boundary_segments = collect_player_frontline_boundary_segments(
                    world,
                    army.owner,
                    path,
                    province_bounds,
                    &sample_y,
                );
                if boundary_segments.is_empty() {
                    let points =
                        visual_path_from_provinces(path, centroids, &sample_y, min_visual_segment);
                    let points = smooth_visual_path_points(&points, 4);
                    let points = resample_visual_path_heights(
                        &points,
                        &sample_y_world,
                        max_terrain_segment,
                        max_line_points,
                    );
                    push_visual_path_instances(
                        &mut instances,
                        &points,
                        frontline_width,
                        false,
                        2.0,
                    );
                } else {
                    for segment in boundary_segments {
                        let segment = resample_visual_path_heights(
                            &segment,
                            &sample_y_world,
                            max_terrain_segment * 0.75,
                            8,
                        );
                        push_visual_path_instances(
                            &mut instances,
                            &segment,
                            frontline_width * 0.55,
                            false,
                            2.0,
                        );
                    }
                }
            }
            if let Some(ref arrow) = order.arrow {
                let anchor_pid = order
                    .anchor
                    .unwrap_or_else(|| path.last().copied().unwrap_or(hoi4_state::ProvinceId(0)));
                let mut arrow_path: Vec<hoi4_state::ProvinceId> = vec![anchor_pid];
                arrow_path.extend_from_slice(&arrow.provinces);
                if arrow_path.len() >= 2 {
                    let points = visual_path_from_provinces(
                        &arrow_path,
                        centroids,
                        &sample_y,
                        min_visual_segment,
                    );
                    let points = smooth_visual_path_points(&points, 7);
                    let points = resample_visual_path_heights(
                        &points,
                        &sample_y_world,
                        max_terrain_segment,
                        max_line_points,
                    );
                    push_battleplan_arrow_instances(
                        &mut instances,
                        &points,
                        arrow_width * 1.35,
                        order.executing,
                    );
                }
            }
        }
    }

    let mut movement_routes: HashSet<(hoi4_state::ProvinceId, hoi4_state::ProvinceId)> =
        HashSet::new();
    for &div_idx in selected_divisions {
        if div_idx >= world.divisions.count {
            continue;
        }
        if world.divisions.owners[div_idx] != player_cid {
            continue;
        }
        let Some(dest) = world.divisions.destinations[div_idx] else {
            continue;
        };
        let cur = world.divisions.locations[div_idx];
        if cur == dest || !movement_routes.insert((cur, dest)) {
            continue;
        }
        let route = find_land_visual_path(&world.map, cur, dest, 32);
        let points =
            visual_path_from_provinces(&route, centroids, &sample_y, min_visual_segment * 0.75);
        let points = smooth_visual_path_points(&points, 3);
        let points = resample_visual_path_heights(
            &points,
            &sample_y_world,
            max_terrain_segment,
            max_line_points / 2,
        );
        push_move_arrow_instances(&mut instances, &points, move_arrow_width);
    }

    if painter_samples.len() >= 2 {
        if let Some(owner) = painter_frontline_owner {
            let preview_path = if let Ok(path) =
                hoi4_logic::military::frontline::frontline_snapper(world, owner, painter_samples)
            {
                path
            } else {
                painter_samples.to_vec()
            };
            let boundary_segments = collect_player_frontline_boundary_segments(
                world,
                owner,
                &preview_path,
                province_bounds,
                &sample_y,
            );
            if boundary_segments.is_empty() {
                let points = visual_path_from_provinces(
                    &preview_path,
                    centroids,
                    &sample_y,
                    min_visual_segment * 0.75,
                );
                let points = smooth_visual_path_points(&points, 4);
                let points = resample_visual_path_heights(
                    &points,
                    &sample_y_world,
                    max_terrain_segment,
                    max_line_points / 2,
                );
                push_visual_path_instances(&mut instances, &points, preview_width, false, 0.0);
            } else {
                for segment in boundary_segments {
                    let segment = resample_visual_path_heights(
                        &segment,
                        &sample_y_world,
                        max_terrain_segment * 0.75,
                        8,
                    );
                    push_visual_path_instances(&mut instances, &segment, preview_width, false, 0.0);
                }
            }
        } else {
            let points = visual_path_from_provinces(
                painter_samples,
                centroids,
                &sample_y,
                min_visual_segment * 0.75,
            );
            let points = smooth_visual_path_points(&points, 4);
            let points = resample_visual_path_heights(
                &points,
                &sample_y_world,
                max_terrain_segment,
                max_line_points / 2,
            );
            push_visual_path_instances(&mut instances, &points, preview_width, false, 0.0);
        }
    }

    instances
}

fn visual_path_from_provinces(
    provinces: &[hoi4_state::ProvinceId],
    centroids: &[(f32, f32)],
    sample_y: &impl Fn(f32, f32) -> f32,
    min_segment_len: f32,
) -> Vec<VisualPathPoint> {
    let mut raw = Vec::with_capacity(provinces.len());
    for &pid in provinces {
        let Some(&(cx, cy)) = centroids.get(pid.0 as usize) else {
            continue;
        };
        if (cx + cy) == 0.0 {
            continue;
        }
        raw.push(VisualPathPoint {
            pos: Vec3::new(cx * WORLD_SCALE, sample_y(cx, cy), cy * WORLD_SCALE),
        });
    }

    filter_visual_path_points(&raw, min_segment_len)
}

fn filter_visual_path_points(
    points: &[VisualPathPoint],
    min_segment_len: f32,
) -> Vec<VisualPathPoint> {
    if points.len() < 2 {
        return points.to_vec();
    }

    let mut filtered = Vec::with_capacity(points.len());
    filtered.push(points[0]);
    for &point in points.iter().skip(1) {
        let last = filtered.last().copied().unwrap_or(point);
        if (point.pos.xz() - last.pos.xz()).length() >= min_segment_len {
            filtered.push(point);
        }
    }

    let last_raw = *points.last().unwrap();
    if filtered.len() == 1 {
        filtered.push(last_raw);
    } else if let Some(last_filtered) = filtered.last_mut() {
        if (last_raw.pos.xz() - last_filtered.pos.xz()).length() >= min_segment_len * 0.5 {
            filtered.push(last_raw);
        } else {
            *last_filtered = last_raw;
        }
    }

    filtered
}

fn smooth_visual_path_points(
    points: &[VisualPathPoint],
    samples_per_segment: usize,
) -> Vec<VisualPathPoint> {
    if points.len() < 3 {
        return points.to_vec();
    }

    let mut smoothed = Vec::with_capacity((points.len() - 1) * samples_per_segment + 1);
    smoothed.push(points[0]);

    for i in 0..points.len() - 1 {
        let p0 = points[i.saturating_sub(1)].pos;
        let p1 = points[i].pos;
        let p2 = points[i + 1].pos;
        let p3 = points[(i + 2).min(points.len() - 1)].pos;
        for step in 1..=samples_per_segment {
            let t = step as f32 / samples_per_segment as f32;
            smoothed.push(VisualPathPoint {
                pos: catmull_rom_limited(p0, p1, p2, p3, t),
            });
        }
    }

    smoothed
}

fn resample_visual_path_heights(
    points: &[VisualPathPoint],
    sample_y_world: &impl Fn(f32, f32) -> f32,
    max_segment_len: f32,
    max_points: usize,
) -> Vec<VisualPathPoint> {
    if points.len() < 2 {
        return points.to_vec();
    }

    let mut out = Vec::with_capacity(points.len().min(max_points));
    out.push(VisualPathPoint {
        pos: Vec3::new(
            points[0].pos.x,
            sample_y_world(points[0].pos.x, points[0].pos.z),
            points[0].pos.z,
        ),
    });

    for w in points.windows(2) {
        let start = w[0].pos;
        let end = w[1].pos;
        let dist = (end.xz() - start.xz()).length();
        let steps = ((dist / max_segment_len).ceil() as usize).clamp(1, 24);
        for step in 1..=steps {
            if out.len() >= max_points {
                return out;
            }
            let t = step as f32 / steps as f32;
            let x = start.x + (end.x - start.x) * t;
            let z = start.z + (end.z - start.z) * t;
            out.push(VisualPathPoint {
                pos: Vec3::new(x, sample_y_world(x, z), z),
            });
        }
    }

    out
}

fn catmull_rom_limited(p0: Vec3, p1: Vec3, p2: Vec3, p3: Vec3, t: f32) -> Vec3 {
    let t2 = t * t;
    let t3 = t2 * t;
    let curved = 0.5
        * ((2.0 * p1)
            + (-p0 + p2) * t
            + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
            + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t3);
    let linear = p1.lerp(p2, t);
    let max_offset = (p2.xz() - p1.xz()).length() * 0.35;
    let offset = curved.xz() - linear.xz();
    if offset.length() <= max_offset || max_offset <= f32::EPSILON {
        curved
    } else {
        let limited_xz = linear.xz() + offset.normalize() * max_offset;
        Vec3::new(limited_xz.x, curved.y, limited_xz.y)
    }
}

fn push_visual_path_instances(
    instances: &mut Vec<crate::passes::maparrow::ArrowInstance>,
    points: &[VisualPathPoint],
    width: f32,
    head_last_segment: bool,
    segment_type: f32,
) {
    if points.len() < 2 {
        return;
    }

    for (i, w) in points.windows(2).enumerate() {
        let is_last = i == points.len() - 2;
        let draw_type = if head_last_segment && is_last {
            1.0
        } else {
            segment_type
        };
        instances.push(crate::passes::maparrow::ArrowInstance {
            start_xz: [w[0].pos.x, w[0].pos.z],
            end_xz: [w[1].pos.x, w[1].pos.z],
            width,
            segment_type: draw_type,
            start_y: w[0].pos.y,
            end_y: w[1].pos.y,
        });
    }
}

fn push_battleplan_arrow_instances(
    instances: &mut Vec<crate::passes::maparrow::ArrowInstance>,
    points: &[VisualPathPoint],
    width: f32,
    executing: bool,
) {
    if points.len() < 2 {
        return;
    }

    let mut seg_lengths = Vec::with_capacity(points.len() - 1);
    let mut total_len = 0.0;
    for w in points.windows(2) {
        let len = (w[1].pos.xz() - w[0].pos.xz()).length();
        seg_lengths.push(len);
        total_len += len;
    }
    if total_len <= f32::EPSILON {
        return;
    }

    let head_len = (total_len * 0.24).clamp(width * 4.0, width * 9.0);
    let tail_len = (total_len * 0.10).clamp(width * 1.8, width * 4.5);
    let body_type = if executing { 9.0 } else { 6.0 };
    let head_type = if executing { 10.0 } else { 7.0 };
    let tail_type = if executing { 11.0 } else { 8.0 };

    let mut dist = 0.0;
    for (i, w) in points.windows(2).enumerate() {
        let mid = dist + seg_lengths[i] * 0.5;
        let segment_type = if mid < tail_len {
            tail_type
        } else if mid > total_len - head_len {
            head_type
        } else {
            body_type
        };
        let segment_width = if segment_type == tail_type {
            width * 0.75
        } else if segment_type == head_type {
            width * 1.45
        } else {
            width
        };
        instances.push(crate::passes::maparrow::ArrowInstance {
            start_xz: [w[0].pos.x, w[0].pos.z],
            end_xz: [w[1].pos.x, w[1].pos.z],
            width: segment_width,
            segment_type,
            start_y: w[0].pos.y,
            end_y: w[1].pos.y,
        });
        dist += seg_lengths[i];
    }

    let last = points[points.len() - 1].pos;
    let prev = points[points.len() - 2].pos;
    let dir = (last.xz() - prev.xz()).normalize_or_zero();
    if dir.length_squared() > 0.0 {
        let start = last.xz() - dir * head_len;
        instances.push(crate::passes::maparrow::ArrowInstance {
            start_xz: [start.x, start.y],
            end_xz: [last.x, last.z],
            width: width * 2.15,
            segment_type: head_type,
            start_y: prev.y + (last.y - prev.y) * 0.5,
            end_y: last.y,
        });
    }
}

fn push_move_arrow_instances(
    instances: &mut Vec<crate::passes::maparrow::ArrowInstance>,
    points: &[VisualPathPoint],
    width: f32,
) {
    if points.len() < 2 {
        return;
    }

    for w in points.windows(2) {
        instances.push(crate::passes::maparrow::ArrowInstance {
            start_xz: [w[0].pos.x, w[0].pos.z],
            end_xz: [w[1].pos.x, w[1].pos.z],
            width,
            segment_type: 4.0,
            start_y: w[0].pos.y,
            end_y: w[1].pos.y,
        });
    }

    let last = points[points.len() - 1].pos;
    let prev = points[points.len() - 2].pos;
    let dir = (last.xz() - prev.xz()).normalize_or_zero();
    if dir.length_squared() <= f32::EPSILON {
        return;
    }

    let head_len =
        ((last.xz() - points[0].pos.xz()).length() * 0.18).clamp(width * 5.0, width * 10.0);
    let start = last.xz() - dir * head_len;
    instances.push(crate::passes::maparrow::ArrowInstance {
        start_xz: [start.x, start.y],
        end_xz: [last.x, last.z],
        width: width * 2.35,
        segment_type: 5.0,
        start_y: prev.y + (last.y - prev.y) * 0.55,
        end_y: last.y,
    });
}

fn find_land_visual_path(
    map: &hoi4_map::GameMap,
    start: hoi4_state::ProvinceId,
    end: hoi4_state::ProvinceId,
    max_depth: usize,
) -> Vec<hoi4_state::ProvinceId> {
    if start == end {
        return vec![start, end];
    }

    let start_idx = start.0 as usize;
    let end_idx = end.0 as usize;
    if start_idx >= map.adjacencies.len() || end_idx >= map.adjacencies.len() {
        return vec![start, end];
    }

    let mut prev: HashMap<u16, u16> = HashMap::new();
    let mut visited: HashSet<u16> = HashSet::new();
    let mut queue = VecDeque::new();
    visited.insert(start.0);
    queue.push_back((start.0, 0usize));

    while let Some((node, depth)) = queue.pop_front() {
        if depth >= max_depth {
            continue;
        }
        let node_idx = node as usize;
        if node_idx >= map.adjacencies.len() {
            continue;
        }
        for &nb in &map.adjacencies[node_idx] {
            if !visited.insert(nb) {
                continue;
            }
            if map
                .get_province(nb)
                .is_some_and(|province| province.province_type != hoi4_map::ProvinceType::Land)
            {
                continue;
            }
            prev.insert(nb, node);
            if nb == end.0 {
                let mut path = vec![end];
                let mut cur = end.0;
                while cur != start.0 {
                    let Some(&p) = prev.get(&cur) else {
                        return vec![start, end];
                    };
                    cur = p;
                    path.push(hoi4_state::ProvinceId(cur));
                }
                path.reverse();
                return path;
            }
            queue.push_back((nb, depth + 1));
        }
    }

    vec![start, end]
}

fn collect_player_frontline_boundary_segments(
    world: &hoi4_state::World,
    owner: hoi4_state::CountryId,
    path: &[hoi4_state::ProvinceId],
    province_bounds: &[Option<ProvincePixelBounds>],
    sample_y: &impl Fn(f32, f32) -> f32,
) -> Vec<[VisualPathPoint; 2]> {
    if path.is_empty() {
        return Vec::new();
    }

    let path_set: HashSet<u16> = path.iter().map(|p| p.0).collect();
    let pmap = &world.map.province_map;
    let w = pmap.width as usize;
    let h = pmap.height as usize;
    if w == 0 || h == 0 {
        return Vec::new();
    }

    let Some((min_x, min_y, max_x, max_y)) = path_scan_bounds(path, province_bounds, w, h) else {
        return Vec::new();
    };

    let is_land = |pid: u16| -> bool {
        world
            .map
            .get_province(pid)
            .is_some_and(|province| province.province_type == hoi4_map::ProvinceType::Land)
    };
    let is_hostile = |pid: u16| -> bool {
        let pi = pid as usize;
        if pi >= world.provinces.count || !is_land(pid) {
            return false;
        }
        let ctrl = world.provinces.controllers[pi];
        if ctrl.is_none() || ctrl == owner {
            return false;
        }
        if world.diplomacy.at_war_with(owner, ctrl) {
            return true;
        }
        let cobel = hoi4_logic::military::frontline::co_belligerent_set(world, owner);
        !cobel.contains(&ctrl)
    };
    let is_front_contact = |a: u16, b: u16| -> bool {
        a != b
            && ((path_set.contains(&a) && is_hostile(b))
                || (path_set.contains(&b) && is_hostile(a)))
    };
    let point = |px: f32, py: f32| VisualPathPoint {
        pos: Vec3::new(px * WORLD_SCALE, sample_y(px, py), py * WORLD_SCALE),
    };

    let mut out = Vec::new();
    let pixels = &pmap.pixels;
    for y in min_y..=max_y {
        let row = y * w;
        for x in min_x..=max_x {
            let a = pixels[row + x];
            if x + 1 < w {
                let b = pixels[row + x + 1];
                if is_front_contact(a, b) {
                    out.push([
                        point(x as f32 + 0.5, y as f32 - 0.5),
                        point(x as f32 + 0.5, y as f32 + 0.5),
                    ]);
                }
            }
            if y + 1 < h {
                let b = pixels[row + w + x];
                if is_front_contact(a, b) {
                    out.push([
                        point(x as f32 - 0.5, y as f32 + 0.5),
                        point(x as f32 + 0.5, y as f32 + 0.5),
                    ]);
                }
            }
        }
    }

    out
}

pub(crate) fn build_province_pixel_bounds(
    pmap: &hoi4_map::ProvinceMap,
) -> Vec<Option<ProvincePixelBounds>> {
    let w = pmap.width as usize;
    let h = pmap.height as usize;
    let max_pid = pmap.pixels.iter().copied().max().unwrap_or(0) as usize;
    let mut bounds: Vec<Option<ProvincePixelBounds>> = vec![None; max_pid.saturating_add(1)];
    for y in 0..h {
        let row = y * w;
        for x in 0..w {
            let pid = pmap.pixels[row + x] as usize;
            if pid == 0 {
                continue;
            }
            if let Some(slot) = bounds.get_mut(pid) {
                match slot {
                    Some(existing) => existing.include(x, y),
                    None => *slot = Some(ProvincePixelBounds::new(x, y)),
                }
            }
        }
    }
    bounds
}

fn path_scan_bounds(
    path: &[hoi4_state::ProvinceId],
    province_bounds: &[Option<ProvincePixelBounds>],
    w: usize,
    h: usize,
) -> Option<(usize, usize, usize, usize)> {
    let mut combined: Option<ProvincePixelBounds> = None;
    for pid in path {
        let Some(Some(bounds)) = province_bounds.get(pid.0 as usize) else {
            continue;
        };
        match &mut combined {
            Some(existing) => {
                existing.include(bounds.min_x, bounds.min_y);
                existing.include(bounds.max_x, bounds.max_y);
            }
            None => combined = Some(*bounds),
        }
    }
    let bounds = combined?;
    let margin = 8usize;
    Some((
        bounds.min_x.saturating_sub(margin),
        bounds.min_y.saturating_sub(margin),
        (bounds.max_x + margin).min(w.saturating_sub(1)),
        (bounds.max_y + margin).min(h.saturating_sub(1)),
    ))
}
