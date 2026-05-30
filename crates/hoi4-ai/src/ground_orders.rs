//! AI Division Movement Execution Layer (J.10).
//!
//! 性能修复：战争全面打起来后非常卡。根因：
//! 1. compute_segment 遍历 states×divisions → O(N²) 热路径
//! 2. find_deep_enemy_provinces 同样 O(N²)
//! 3. bfs_distance_cached 名字有 cached 但实际没缓存，每次完整 BFS
//! 4. assign_garrison_role 对每个师×每个前线省做 BFS
//!
//! 修复：
//! - compute_segment 不再遍历 divisions，只看 controller 字段
//! - find_deep_enemy_provinces 用预计算的 enemy_div_locations 避免嵌套循环
//! - bfs_distance_cached 真正使用 World.path_cache
//! - assign_garrison_role 用一次 multi-source BFS 代替 per-div BFS
//! - partition_divisions_by_segment 用 segment index 缓存避免重复 BFS

use hoi4_logic::economy::EconomyState;
use hoi4_logic::military::command_executor;
use hoi4_logic::military::movement::{
    can_enter_province, clear_division_move_order, order_naval_invasion, order_overseas_transport,
    usable_ports,
};
use hoi4_state::{CountryId, DivisionAssignment, DivisionIntent, DivisionRole, ProvinceId, World};

use crate::china_theater;
use crate::frontline::{FrontLine, FrontSegment};
use crate::ground::{evaluate_front_sectors, GroundEvaluation, GroundPosture};
use crate::intent::NationalIntent;

const MAX_RALLY_DISTANCE: u32 = 40;
const ATTACK_FRONT_BREADTH: usize = 3;
const MIN_DIVS_PER_ATTACK_TARGET: usize = 3;
const MAX_GARRISON_DIVS_PER_FRONT_PROV: u32 = 3;
const MAX_ASSAULT_DIVS_PER_TARGET: usize = 4;
const OVERSEAS_REINFORCEMENTS_PER_TICK: usize = 3;
const PRIORITY_THEATER_REINFORCEMENTS_PER_TICK: usize = 8;
const NAVAL_INVASIONS_PER_TICK: usize = 2;

pub const TACTICAL_EVAL_CADENCE_DAYS: u32 = crate::constants::TACTICAL_EVAL_CADENCE;

pub fn execute_ground_orders(
    world: &mut World,
    econ: &EconomyState,
    country: CountryId,
    eval: &GroundEvaluation,
    _intent: &NationalIntent,
    front: &FrontLine,
) {
    if country == world.player {
        let has_active = world
            .player_armies
            .iter()
            .any(|a| a.owner == country && a.order.as_ref().is_some_and(|o| o.active));
        if has_active {
            return;
        }
    }

    let mut segments: Vec<FrontSegment> = Vec::new();
    let mut postures: Vec<GroundPosture> = Vec::new();
    let mut active_enemies: std::collections::HashSet<CountryId> = std::collections::HashSet::new();
    for decision in &eval.decisions {
        // 复用同轮 FrontLine 计算结果，避免冗余 compute_segment。
        let seg = match front.segment_against(decision.enemy) {
            Some(s) if s.is_active() => s.clone(),
            _ => continue,
        };
        segments.push(seg);
        postures.push(decision.posture);
        active_enemies.insert(decision.enemy);
    }

    let enemy_div_locations = build_enemy_div_locations(world, country, &active_enemies);

    if !segments.is_empty() {
        let assignments = partition_divisions_by_segment(world, country, &segments);
        for (seg_idx, seg) in segments.iter().enumerate() {
            let div_indices = match assignments.get(seg_idx) {
                Some(v) => v,
                None => continue,
            };
            let posture = postures[seg_idx];
            let enemy = eval.decisions.get(seg_idx).map(|d| d.enemy);

            match posture {
                GroundPosture::Attack => {
                    let mut targets = pick_attack_targets(world, country, seg);
                    if targets.is_empty() {
                        targets = find_deep_enemy_provinces(
                            world,
                            country,
                            seg.enemy,
                            &enemy_div_locations,
                        );
                    }
                    if targets.is_empty() {
                        if !div_indices.is_empty() {
                            assign_mop_up_role(
                                world,
                                div_indices,
                                country,
                                enemy,
                                &enemy_div_locations,
                            );
                        }
                        continue;
                    }
                    if div_indices.is_empty() {
                        continue;
                    }
                    assign_assault_role(world, div_indices, &targets, country, seg, enemy);
                }
                GroundPosture::Defend | GroundPosture::Hold => {
                    if div_indices.is_empty() {
                        continue;
                    }
                    assign_garrison_role(world, div_indices, seg, country, enemy);
                }
                GroundPosture::Retreat => {
                    if div_indices.is_empty() {
                        continue;
                    }
                    assign_garrison_role(world, div_indices, seg, country, enemy);
                }
            }
        }
    }

    let war_enemies: std::collections::HashSet<CountryId> = (0..world.countries.count)
        .filter_map(|ci| {
            let other = CountryId(ci as u16);
            if other != country
                && world.diplomacy.at_war_with(country, other)
                && !world.diplomacy.annexed_countries.contains(&other)
            {
                Some(other)
            } else {
                None
            }
        })
        .collect();
    let all_enemy_div_locations = build_enemy_div_locations(world, country, &war_enemies);
    let no_handled_enemies = std::collections::HashSet::new();
    mop_up_remaining_enemies(
        world,
        country,
        &no_handled_enemies,
        &all_enemy_div_locations,
    );

    clear_stale_assignments(world, country);
    execute_overseas_reinforcement_orders(world, econ, country);
    execute_naval_invasion_orders(world, econ, country);
}

fn issue_ai_move_assignment(
    world: &mut World,
    div_idx: usize,
    destination: ProvinceId,
    assignment: DivisionAssignment,
) -> bool {
    let intent = match assignment.role {
        DivisionRole::Assault => DivisionIntent::Assault,
        DivisionRole::Garrison => DivisionIntent::Garrison,
        DivisionRole::MopUp => DivisionIntent::MopUp,
        DivisionRole::Reserve => DivisionIntent::Reserve,
    };
    command_executor::issue_ai_ground_command(world, div_idx, intent, destination, assignment)
}

fn clear_ai_move_order(world: &mut World, div_idx: usize) {
    let _ = clear_division_move_order(world, div_idx);
    command_executor::clear_division_command(world, div_idx);
}

fn build_enemy_div_locations(
    world: &World,
    _country: CountryId,
    enemies: &std::collections::HashSet<CountryId>,
) -> std::collections::HashSet<u16> {
    let mut locs = std::collections::HashSet::new();
    if world.runtime_country_indexes_valid {
        for &enemy in enemies {
            let Some(indices) = country_division_indices(world, enemy) else {
                continue;
            };
            for &di in indices {
                add_healthy_division_location(world, di, &mut locs);
            }
        }
    } else {
        for di in 0..world.divisions.count {
            let owner = world.divisions.owners[di];
            if !enemies.contains(&owner) {
                continue;
            }
            add_healthy_division_location(world, di, &mut locs);
        }
    }
    locs
}

fn country_division_indices(world: &World, country: CountryId) -> Option<&[usize]> {
    if !world.runtime_country_indexes_valid || country.is_none() {
        return None;
    }
    world
        .country_division_index
        .get(country.0 as usize)
        .map(|indices| indices.as_slice())
}

fn add_healthy_division_location(
    world: &World,
    div_idx: usize,
    locs: &mut std::collections::HashSet<u16>,
) {
    if div_idx >= world.divisions.count {
        return;
    }
    let max_org = world.divisions.max_organisation[div_idx].max(1e-6);
    let org_ratio = world.divisions.organisation[div_idx] / max_org;
    if org_ratio < 0.05 || world.divisions.strength[div_idx] < 0.05 {
        return;
    }
    locs.insert(world.divisions.locations[div_idx].0);
}

fn find_deep_enemy_provinces(
    world: &World,
    country: CountryId,
    enemy: CountryId,
    enemy_div_locs: &std::collections::HashSet<u16>,
) -> Vec<ProvinceId> {
    let mut scored: Vec<(ProvinceId, i32)> = Vec::new();

    for si in 0..world.states.count {
        if world.states.owners[si] != enemy {
            continue;
        }
        let state_ctrl = world.states.controllers[si];
        if state_ctrl == country {
            continue;
        }

        let mut found = false;
        let mut enemy_garrison = 0i32;
        for &p in &world.states.provinces[si] {
            let pi = p.0 as usize;
            if pi >= world.provinces.count {
                continue;
            }
            if world.provinces.controllers[pi] != country {
                found = true;
            }
            if enemy_div_locs.contains(&p.0) {
                found = true;
                enemy_garrison += 1;
            }
        }
        if !found {
            continue;
        }

        let vp_score = if si < world.data.states.len() {
            world.data.states[si]
                .victory_points
                .iter()
                .map(|(_, v)| *v as i32)
                .sum::<i32>()
        } else {
            0
        };
        let factory_score = world.state_building_levels(hoi4_state::StateId(si as u16)) as i32;
        let score = 50 + vp_score * 3 + factory_score - enemy_garrison * 8;

        for &p in &world.states.provinces[si] {
            let pi = p.0 as usize;
            if pi >= world.provinces.count {
                continue;
            }
            if can_stage_from_province(world, country, p) {
                continue;
            }
            if !can_enter_province(world, country, p) {
                continue;
            }
            if let Some(def) = world.map.get_province(p.0) {
                if !matches!(def.province_type, hoi4_map::ProvinceType::Land) {
                    continue;
                }
            } else {
                continue;
            }
            scored.push((p, score));
            break;
        }
    }

    scored.sort_by(|a, b| b.1.cmp(&a.1));
    scored.into_iter().map(|(p, _)| p).collect()
}

fn should_reassign(world: &World, div_idx: usize, owner: CountryId) -> bool {
    match world
        .divisions
        .assignments
        .get(div_idx)
        .and_then(|a| a.as_ref())
    {
        None => true,
        Some(asgn) => {
            if is_broken(world, div_idx) {
                return true;
            }
            match asgn.role {
                DivisionRole::Assault => {
                    if let Some(target) = asgn.target {
                        let ctrl = world.provinces.controllers[target.0 as usize];
                        if ctrl == owner {
                            return true;
                        }
                        if !can_enter_province(world, owner, target) {
                            return true;
                        }
                        if has_arrived_or_stuck(world, div_idx) {
                            return true;
                        }
                    } else {
                        return true;
                    }
                    false
                }
                DivisionRole::MopUp => {
                    if let Some(target) = asgn.target {
                        let ctrl = world.provinces.controllers[target.0 as usize];
                        if ctrl == owner || !can_enter_province(world, owner, target) {
                            return true;
                        }
                    } else {
                        return true;
                    }
                    has_arrived_or_stuck(world, div_idx)
                }
                DivisionRole::Garrison => {
                    if let Some(target) = asgn.target {
                        let ctrl = world.provinces.controllers[target.0 as usize];
                        if ctrl != owner && world.diplomacy.at_war_with(owner, ctrl) {
                            return true;
                        }
                    }
                    match world.divisions.destinations[div_idx] {
                        None => true,
                        Some(dest) => {
                            let cur = world.divisions.locations[div_idx];
                            if cur == dest {
                                false
                            } else {
                                let cur_pi = cur.0 as usize;
                                if cur_pi < world.provinces.count {
                                    let cur_ctrl = world.provinces.controllers[cur_pi];
                                    if !cur_ctrl.is_none()
                                        && cur_ctrl != owner
                                        && world.diplomacy.at_war_with(owner, cur_ctrl)
                                    {
                                        return true;
                                    }
                                }
                                false
                            }
                        }
                    }
                }
                DivisionRole::Reserve => true,
            }
        }
    }
}

fn is_active_assault(world: &World, div_idx: usize, owner: CountryId) -> bool {
    let Some(asgn) = world
        .divisions
        .assignments
        .get(div_idx)
        .and_then(|a| a.as_ref())
    else {
        return false;
    };
    if asgn.role != DivisionRole::Assault || should_reassign(world, div_idx, owner) {
        return false;
    }
    let Some(target) = asgn.target else {
        return false;
    };
    let ti = target.0 as usize;
    ti < world.provinces.count
        && world.provinces.controllers[ti] != owner
        && world
            .diplomacy
            .at_war_with(owner, world.provinces.controllers[ti])
}

fn has_arrived_or_stuck(world: &World, div_idx: usize) -> bool {
    let cur = world.divisions.locations[div_idx];
    match world.divisions.destinations[div_idx] {
        None => true,
        Some(dest) => {
            if cur == dest {
                return true;
            }
            let cur_pi = cur.0 as usize;
            if cur_pi < world.provinces.count {
                let cur_ctrl = world.provinces.controllers[cur_pi];
                let owner = world.divisions.owners[div_idx];
                if !cur_ctrl.is_none()
                    && cur_ctrl != owner
                    && world.diplomacy.at_war_with(owner, cur_ctrl)
                {
                    return true;
                }
            }
            false
        }
    }
}

fn is_broken(world: &World, div_idx: usize) -> bool {
    let max_org = world.divisions.max_organisation[div_idx].max(1e-6);
    let org_ratio = world.divisions.organisation[div_idx] / max_org;
    org_ratio < 0.05 || world.divisions.strength[div_idx] < 0.05
}

fn assign_assault_role(
    world: &mut World,
    div_indices: &[usize],
    targets: &[ProvinceId],
    country: CountryId,
    seg: &FrontSegment,
    enemy: Option<CountryId>,
) {
    if targets.is_empty() || div_indices.is_empty() {
        return;
    }
    force_fill_frontline(world, div_indices, seg, country, enemy);
    let line_anchors = frontline_anchor_divisions(world, div_indices, seg, country);

    let div_indices: Vec<usize> = div_indices
        .iter()
        .copied()
        .filter(|&di| {
            if line_anchors.contains(&di) {
                return false;
            }
            if is_active_assault(world, di, country) {
                return false;
            }
            if world.player_locked_divisions.contains(&di) {
                if let Some(ref asgn) = world.divisions.assignments.get(di).and_then(|a| a.as_ref())
                {
                    if asgn.role == DivisionRole::Assault && !should_reassign(world, di, country) {
                        return false;
                    }
                }
            }
            true
        })
        .collect();
    if div_indices.is_empty() {
        return;
    }
    let div_indices = &div_indices[..];
    force_fill_frontline(world, div_indices, seg, country, enemy);

    let map = world.map.clone();

    let max_breadth_by_count = (div_indices.len() / MIN_DIVS_PER_ATTACK_TARGET).max(1);
    let breadth = ATTACK_FRONT_BREADTH
        .min(max_breadth_by_count)
        .min(targets.len());
    let working_targets: Vec<ProvinceId> = targets.iter().take(breadth).copied().collect();

    let mut target_dists: Vec<std::collections::HashMap<u16, u32>> = Vec::with_capacity(breadth);
    for &t in &working_targets {
        let dist = bfs_multi_source_dist(&map, &[t], MAX_RALLY_DISTANCE);
        target_dists.push(dist);
    }

    let per_target = ((div_indices.len() as f32) / (working_targets.len() as f32)).ceil() as usize;
    let per_target = per_target
        .max(MIN_DIVS_PER_ATTACK_TARGET)
        .min(MAX_ASSAULT_DIVS_PER_TARGET);

    let mut triplets: Vec<(u32, usize, usize)> = Vec::new();
    for &di in div_indices {
        if !should_reassign(world, di, country) {
            continue;
        }
        let cur = world.divisions.locations[di].0;
        for (ti, dist) in target_dists.iter().enumerate() {
            if let Some(&d) = dist.get(&cur) {
                triplets.push((d, ti, di));
            }
        }
    }
    triplets.sort_by_key(|&(d, _, _)| d);

    let mut assigned: std::collections::HashSet<usize> = std::collections::HashSet::new();
    let mut counts = vec![0usize; working_targets.len()];
    let now = world.elapsed_hours;

    for (_, ti, di) in triplets {
        if assigned.contains(&di) {
            continue;
        }
        if counts[ti] >= per_target {
            continue;
        }
        if !can_enter_province(world, country, working_targets[ti]) {
            continue;
        }
        if issue_ai_move_assignment(
            world,
            di,
            working_targets[ti],
            DivisionAssignment {
                front: enemy,
                role: DivisionRole::Assault,
                target: Some(working_targets[ti]),
                assigned_at: now,
            },
        ) {
            assigned.insert(di);
            counts[ti] += 1;
        }
    }

    for &di in div_indices {
        if assigned.contains(&di) {
            continue;
        }
        if !should_reassign(world, di, country) {
            continue;
        }
        if let Some(ti) = counts.iter().position(|&c| c < MAX_ASSAULT_DIVS_PER_TARGET) {
            if can_enter_province(world, country, working_targets[ti]) {
                if issue_ai_move_assignment(
                    world,
                    di,
                    working_targets[ti],
                    DivisionAssignment {
                        front: enemy,
                        role: DivisionRole::Assault,
                        target: Some(working_targets[ti]),
                        assigned_at: now,
                    },
                ) {
                    counts[ti] += 1;
                }
            }
        }
    }

    // Bug #11 fallback: orphaned divisions not reachable from any target BFS.
    // Send them toward the closest frontline province so they eventually enter range.
    let mut orphans: Vec<usize> = Vec::new();
    for &di in div_indices {
        if assigned.contains(&di) {
            continue;
        }
        if !should_reassign(world, di, country) {
            continue;
        }
        let cur = world.divisions.locations[di].0;
        let reachable = target_dists.iter().any(|dist| dist.contains_key(&cur));
        if !reachable {
            orphans.push(di);
        }
    }
    if !orphans.is_empty() {
        let fallback = seg
            .friendly_states
            .iter()
            .flat_map(|&sid| {
                let si = sid.0 as usize;
                if si < world.states.count {
                    world.states.provinces[si].clone()
                } else {
                    Vec::new()
                }
            })
            .find(|&p| can_enter_province(world, country, p))
            .unwrap_or_else(|| capital_province(world, country));
        for &di in &orphans {
            if can_enter_province(world, country, fallback) {
                if issue_ai_move_assignment(
                    world,
                    di,
                    fallback,
                    DivisionAssignment {
                        front: enemy,
                        role: DivisionRole::Reserve,
                        target: Some(fallback),
                        assigned_at: now,
                    },
                ) {
                    assigned.insert(di);
                }
            }
        }
    }
}

fn assign_garrison_role(
    world: &mut World,
    div_indices: &[usize],
    seg: &FrontSegment,
    country: CountryId,
    enemy: Option<CountryId>,
) {
    if div_indices.is_empty() {
        return;
    }
    let div_indices: Vec<usize> = div_indices
        .iter()
        .copied()
        .filter(|&di| {
            if world.player_locked_divisions.contains(&di) {
                if let Some(ref asgn) = world.divisions.assignments.get(di).and_then(|a| a.as_ref())
                {
                    if asgn.role == DivisionRole::Assault && !should_reassign(world, di, country) {
                        return false;
                    }
                }
            }
            true
        })
        .collect();
    if div_indices.is_empty() {
        return;
    }
    let div_indices = &div_indices[..];

    let map = world.map.clone();

    let frontline_provs = frontline_provinces(world, seg, country);

    if frontline_provs.is_empty() {
        let fallback = seg
            .friendly_states
            .first()
            .and_then(|&sid| {
                let si = sid.0 as usize;
                if si < world.states.count {
                    world.states.provinces[si].first().copied()
                } else {
                    None
                }
            })
            .unwrap_or_else(|| capital_province(world, country));
        let now = world.elapsed_hours;
        for &di in div_indices {
            if should_reassign(world, di, country) && can_enter_province(world, country, fallback) {
                issue_ai_move_assignment(
                    world,
                    di,
                    fallback,
                    DivisionAssignment {
                        front: enemy,
                        role: DivisionRole::Garrison,
                        target: Some(fallback),
                        assigned_at: now,
                    },
                );
            }
        }
        return;
    }

    let target_per_prov =
        ((div_indices.len() as f32) / (frontline_provs.len() as f32)).ceil() as u32;
    let target_per_prov = target_per_prov.max(1).min(MAX_GARRISON_DIVS_PER_FRONT_PROV);
    let frontline_set: std::collections::HashSet<u16> =
        frontline_provs.iter().map(|p| p.0).collect();

    let front_dist = bfs_multi_source_dist(&map, &frontline_provs, MAX_RALLY_DISTANCE);

    let mut assigned_count: std::collections::HashMap<u16, u32> = std::collections::HashMap::new();
    for &di in div_indices {
        let cur = world.divisions.locations[di];
        if frontline_set.contains(&cur.0) {
            *assigned_count.entry(cur.0).or_insert(0) += 1;
            continue;
        }
        if let Some(dest) = world.divisions.destinations[di] {
            if frontline_set.contains(&dest.0) {
                *assigned_count.entry(dest.0).or_insert(0) += 1;
            }
        }
    }

    let reachable_frontline_provs: Vec<ProvinceId> = frontline_provs
        .iter()
        .copied()
        .filter(|&p| can_enter_province(world, country, p))
        .collect();
    let reachable_frontline_set: std::collections::HashSet<u16> =
        reachable_frontline_provs.iter().map(|p| p.0).collect();
    let ensure_one_per_prov = !reachable_frontline_provs.is_empty()
        && div_indices.len() >= reachable_frontline_provs.len();

    let needs_frontline_cover = |assigned_count: &std::collections::HashMap<u16, u32>| -> bool {
        ensure_one_per_prov
            && reachable_frontline_provs
                .iter()
                .any(|p| assigned_count.get(&p.0).copied().unwrap_or(0) == 0)
    };

    let now = world.elapsed_hours;
    for &di in div_indices {
        let cur = world.divisions.locations[di];
        if frontline_set.contains(&cur.0) {
            let cur_count = assigned_count.get(&cur.0).copied().unwrap_or(0);
            if ((needs_frontline_cover(&assigned_count) && cur_count > 1)
                || cur_count > target_per_prov)
                && should_reassign(world, di, country)
            {
                if let Some(count) = assigned_count.get_mut(&cur.0) {
                    *count = count.saturating_sub(1);
                }
            } else {
                clear_ai_move_order(world, di);
                if should_reassign(world, di, country) {
                    world.divisions.assignments[di] = Some(DivisionAssignment {
                        front: enemy,
                        role: DivisionRole::Garrison,
                        target: Some(cur),
                        assigned_at: now,
                    });
                }
                continue;
            }
        }
        if let Some(dest) = world.divisions.destinations[di] {
            if frontline_set.contains(&dest.0)
                && assigned_count.get(&dest.0).copied().unwrap_or(0) <= target_per_prov
            {
                let dest_count = assigned_count.get(&dest.0).copied().unwrap_or(0);
                if ((needs_frontline_cover(&assigned_count) && dest_count > 1)
                    || dest_count > target_per_prov)
                    && should_reassign(world, di, country)
                {
                    if let Some(count) = assigned_count.get_mut(&dest.0) {
                        *count = count.saturating_sub(1);
                    }
                } else {
                    continue;
                }
            }
        }
        if !should_reassign(world, di, country) {
            continue;
        }

        let d = front_dist.get(&cur.0).copied().unwrap_or(u32::MAX);
        if d == u32::MAX {
            if let Some(&dest) = frontline_provs
                .iter()
                .find(|&&p| can_enter_province(world, country, p))
            {
                issue_ai_move_assignment(
                    world,
                    di,
                    dest,
                    DivisionAssignment {
                        front: enemy,
                        role: DivisionRole::Garrison,
                        target: Some(dest),
                        assigned_at: now,
                    },
                );
            } else {
                // Bug #11 fallback: unreachable from frontline → head to capital
                let cap = capital_province(world, country);
                if can_enter_province(world, country, cap) {
                    issue_ai_move_assignment(
                        world,
                        di,
                        cap,
                        DivisionAssignment {
                            front: enemy,
                            role: DivisionRole::Reserve,
                            target: Some(cap),
                            assigned_at: now,
                        },
                    );
                }
            }
            continue;
        }

        let mut best: Option<(ProvinceId, u32)> = None;
        for &p in &frontline_provs {
            if !reachable_frontline_set.contains(&p.0) {
                continue;
            }
            let cnt = assigned_count.get(&p.0).copied().unwrap_or(0);
            if ensure_one_per_prov && needs_frontline_cover(&assigned_count) && cnt > 0 {
                continue;
            }
            if cnt >= target_per_prov {
                continue;
            }
            let pd = front_dist.get(&p.0).copied().unwrap_or(u32::MAX);
            let cost = d + pd;
            match best {
                None => best = Some((p, cost)),
                Some((_, prev)) if cost < prev => best = Some((p, cost)),
                _ => {}
            }
        }
        if let Some((dest, _)) = best {
            if issue_ai_move_assignment(
                world,
                di,
                dest,
                DivisionAssignment {
                    front: enemy,
                    role: DivisionRole::Garrison,
                    target: Some(dest),
                    assigned_at: now,
                },
            ) {
                *assigned_count.entry(dest.0).or_insert(0) += 1;
            }
        } else if let Some(&dest) = frontline_provs
            .iter()
            .filter(|&&p| reachable_frontline_set.contains(&p.0))
            .min_by_key(|p| assigned_count.get(&p.0).copied().unwrap_or(0))
        {
            if assigned_count.get(&dest.0).copied().unwrap_or(0) < target_per_prov {
                if issue_ai_move_assignment(
                    world,
                    di,
                    dest,
                    DivisionAssignment {
                        front: enemy,
                        role: DivisionRole::Garrison,
                        target: Some(dest),
                        assigned_at: now,
                    },
                ) {
                    *assigned_count.entry(dest.0).or_insert(0) += 1;
                }
            } else {
                let cap = capital_province(world, country);
                if can_enter_province(world, country, cap) {
                    issue_ai_move_assignment(
                        world,
                        di,
                        cap,
                        DivisionAssignment {
                            front: enemy,
                            role: DivisionRole::Reserve,
                            target: Some(cap),
                            assigned_at: now,
                        },
                    );
                }
            }
        } else {
            // Bug #11 fallback: no reachable frontline province → head to capital
            let cap = capital_province(world, country);
            if can_enter_province(world, country, cap) {
                issue_ai_move_assignment(
                    world,
                    di,
                    cap,
                    DivisionAssignment {
                        front: enemy,
                        role: DivisionRole::Reserve,
                        target: Some(cap),
                        assigned_at: now,
                    },
                );
            }
        }
    }
}

fn force_fill_frontline(
    world: &mut World,
    div_indices: &[usize],
    seg: &FrontSegment,
    country: CountryId,
    enemy: Option<CountryId>,
) {
    if div_indices.is_empty() {
        return;
    }

    let frontline_provs = frontline_provinces(world, seg, country);
    let reachable_frontline_provs: Vec<ProvinceId> = frontline_provs
        .iter()
        .copied()
        .filter(|&p| can_enter_province(world, country, p))
        .collect();
    if reachable_frontline_provs.is_empty() || div_indices.len() < reachable_frontline_provs.len() {
        return;
    }

    let reachable_set: std::collections::HashSet<u16> =
        reachable_frontline_provs.iter().map(|p| p.0).collect();
    let mut assigned_count: std::collections::HashMap<u16, u32> = std::collections::HashMap::new();
    for &di in div_indices {
        let cur = world.divisions.locations[di];
        if reachable_set.contains(&cur.0) {
            *assigned_count.entry(cur.0).or_insert(0) += 1;
            continue;
        }
        if let Some(dest) = world.divisions.destinations[di] {
            if reachable_set.contains(&dest.0) {
                *assigned_count.entry(dest.0).or_insert(0) += 1;
            }
        }
    }

    let mut empty: Vec<ProvinceId> = reachable_frontline_provs
        .iter()
        .copied()
        .filter(|p| assigned_count.get(&p.0).copied().unwrap_or(0) == 0)
        .collect();
    if empty.is_empty() {
        return;
    }

    let map = world.map.clone();
    let mut empty_dist: Vec<std::collections::HashMap<u16, u32>> = empty
        .iter()
        .map(|&p| bfs_multi_source_dist(&map, &[p], MAX_RALLY_DISTANCE))
        .collect();
    let now = world.elapsed_hours;

    while !empty.is_empty() {
        let mut best: Option<(u32, usize, usize)> = None;
        for (target_idx, dist) in empty_dist.iter().enumerate() {
            for &di in div_indices {
                if world.player_locked_divisions.contains(&di) {
                    continue;
                }
                let cur = world.divisions.locations[di];
                if reachable_set.contains(&cur.0)
                    && assigned_count.get(&cur.0).copied().unwrap_or(0) <= 1
                {
                    continue;
                }
                if let Some(dest) = world.divisions.destinations[di] {
                    if reachable_set.contains(&dest.0)
                        && assigned_count.get(&dest.0).copied().unwrap_or(0) <= 1
                    {
                        continue;
                    }
                }
                let d = dist.get(&cur.0).copied().unwrap_or(u32::MAX / 2);
                match best {
                    None => best = Some((d, target_idx, di)),
                    Some((prev, _, _)) if d < prev => best = Some((d, target_idx, di)),
                    _ => {}
                }
            }
        }

        let Some((_, target_idx, di)) = best else {
            break;
        };
        let dest = empty.remove(target_idx);
        empty_dist.remove(target_idx);
        let cur = world.divisions.locations[di];
        if reachable_set.contains(&cur.0) {
            if let Some(count) = assigned_count.get_mut(&cur.0) {
                *count = count.saturating_sub(1);
            }
        }
        if let Some(old_dest) = world.divisions.destinations[di] {
            if reachable_set.contains(&old_dest.0) {
                if let Some(count) = assigned_count.get_mut(&old_dest.0) {
                    *count = count.saturating_sub(1);
                }
            }
        }
        if issue_ai_move_assignment(
            world,
            di,
            dest,
            DivisionAssignment {
                front: enemy,
                role: DivisionRole::Garrison,
                target: Some(dest),
                assigned_at: now,
            },
        ) {
            *assigned_count.entry(dest.0).or_insert(0) += 1;
        }
    }
}

fn frontline_anchor_divisions(
    world: &World,
    div_indices: &[usize],
    seg: &FrontSegment,
    country: CountryId,
) -> std::collections::HashSet<usize> {
    let frontline_provs = frontline_provinces(world, seg, country);
    let reachable_frontline_provs: Vec<ProvinceId> = frontline_provs
        .iter()
        .copied()
        .filter(|&p| can_enter_province(world, country, p))
        .collect();
    if reachable_frontline_provs.is_empty() || div_indices.len() < reachable_frontline_provs.len() {
        return std::collections::HashSet::new();
    }

    let mut anchors = std::collections::HashSet::new();
    for prov in reachable_frontline_provs {
        if let Some(&di) = div_indices
            .iter()
            .find(|&&di| world.divisions.locations[di] == prov)
        {
            anchors.insert(di);
            continue;
        }
        if let Some(&di) = div_indices.iter().find(|&&di| {
            world.divisions.destinations[di] == Some(prov)
                && world
                    .divisions
                    .assignments
                    .get(di)
                    .and_then(|a| a.as_ref())
                    .is_some_and(|a| a.role == DivisionRole::Garrison)
        }) {
            anchors.insert(di);
            continue;
        }
        if let Some(&di) = div_indices
            .iter()
            .find(|&&di| world.divisions.destinations[di] == Some(prov))
        {
            anchors.insert(di);
        }
    }
    anchors
}

fn frontline_provinces(world: &World, seg: &FrontSegment, country: CountryId) -> Vec<ProvinceId> {
    let mut frontline_provs: Vec<ProvinceId> = Vec::new();
    for &state_id in &seg.friendly_states {
        let si = state_id.0 as usize;
        if si >= world.states.count {
            continue;
        }
        for &p in &world.states.provinces[si] {
            let pi = p.0 as usize;
            if pi >= world.provinces.count || !can_stage_from_province(world, country, p) {
                continue;
            }
            if world
                .map
                .get_province(p.0)
                .is_none_or(|def| !matches!(def.province_type, hoi4_map::ProvinceType::Land))
            {
                continue;
            }
            if pi >= world.map.adjacencies.len() {
                continue;
            }
            let on_front = world.map.adjacencies[pi].iter().any(|&adj_raw| {
                let adj_pi = adj_raw as usize;
                if adj_pi >= world.provinces.count {
                    return false;
                }
                let adj_ctrl = world.provinces.controllers[adj_pi];
                !adj_ctrl.is_none()
                    && !can_stage_controller(world, country, adj_ctrl)
                    && world.diplomacy.at_war_with(country, adj_ctrl)
            });
            if on_front {
                frontline_provs.push(p);
            }
        }
    }
    frontline_provs.sort_by_key(|p| p.0);
    frontline_provs.dedup_by_key(|p| p.0);
    frontline_provs
}

fn can_stage_from_province(world: &World, country: CountryId, p: ProvinceId) -> bool {
    let pi = p.0 as usize;
    pi < world.provinces.count
        && can_stage_controller(world, country, world.provinces.controllers[pi])
}

fn can_stage_controller(world: &World, country: CountryId, controller: CountryId) -> bool {
    if controller == country {
        return true;
    }
    if controller.is_none() {
        return false;
    }
    if world.diplomacy.has_military_access(country, controller) {
        return true;
    }
    world
        .diplomacy
        .faction_of(country)
        .is_some_and(|f| world.diplomacy.faction_of(controller) == Some(f))
}

fn clear_stale_assignments(world: &mut World, country: CountryId) {
    for i in 0..world.divisions.count {
        if world.divisions.owners[i] != country {
            continue;
        }
        if world.player_locked_divisions.contains(&i) {
            continue;
        }
        let asgn = match world.divisions.assignments[i].as_ref() {
            Some(a) => a,
            None => continue,
        };
        if let Some(target) = asgn.target {
            if !can_enter_province(world, country, target) {
                world.divisions.assignments[i] = None;
                clear_ai_move_order(world, i);
                continue;
            }
        }
        if !world.countries.at_war[country.0 as usize] {
            world.divisions.assignments[i] = None;
        }
    }
}

fn assign_mop_up_role(
    world: &mut World,
    div_indices: &[usize],
    country: CountryId,
    enemy: Option<CountryId>,
    enemy_div_locs: &std::collections::HashSet<u16>,
) {
    if div_indices.is_empty() {
        return;
    }
    let div_indices: Vec<usize> = div_indices
        .iter()
        .copied()
        .filter(|&di| {
            if world.player_locked_divisions.contains(&di) {
                if let Some(ref asgn) = world.divisions.assignments.get(di).and_then(|a| a.as_ref())
                {
                    if asgn.role == DivisionRole::Assault && !should_reassign(world, di, country) {
                        return false;
                    }
                }
            }
            true
        })
        .collect();
    if div_indices.is_empty() {
        return;
    }

    let enemy_id = match enemy {
        Some(e) => e,
        None => return,
    };

    let enemy_locs = healthy_division_locations_for(world, enemy_id);
    let mut mop_targets: Vec<ProvinceId> = Vec::new();
    for &loc_raw in enemy_div_locs {
        if enemy_locs.contains(&loc_raw) && can_enter_province(world, country, ProvinceId(loc_raw))
        {
            mop_targets.push(ProvinceId(loc_raw));
        }
    }

    for si in 0..world.states.count {
        if world.states.owners[si] != enemy_id {
            continue;
        }
        if world.states.controllers[si] == country {
            continue;
        }
        for &p in &world.states.provinces[si] {
            let pi = p.0 as usize;
            if pi >= world.provinces.count {
                continue;
            }
            if world.provinces.controllers[pi] == country {
                continue;
            }
            if can_enter_province(world, country, p) {
                mop_targets.push(p);
            }
        }
    }

    mop_targets.sort_by_key(|p| {
        let pi = p.0 as usize;
        let has_enemy_div = enemy_div_locs.contains(&p.0);
        let supply = if pi < world.provinces.supply.len() {
            (100.0 - world.provinces.supply[pi]).max(0.0) as i32
        } else {
            0
        };
        let owner_mismatch = if pi < world.provinces.count && world.provinces.owners[pi] == enemy_id
        {
            0
        } else {
            10
        };
        (
            if has_enemy_div { 0 } else { 1 },
            owner_mismatch,
            supply,
            p.0,
        )
    });
    mop_targets.dedup();

    if mop_targets.is_empty() {
        let now = world.elapsed_hours;
        let fallback = capital_province(world, country);
        for &di in &div_indices {
            if should_reassign(world, di, country) && can_enter_province(world, country, fallback) {
                issue_ai_move_assignment(
                    world,
                    di,
                    fallback,
                    DivisionAssignment {
                        front: enemy,
                        role: DivisionRole::Reserve,
                        target: Some(fallback),
                        assigned_at: now,
                    },
                );
            }
        }
        return;
    }

    let map = world.map.clone();
    let now = world.elapsed_hours;
    let mut assigned_targets: std::collections::HashMap<u16, u32> =
        std::collections::HashMap::new();

    for &di in &div_indices {
        if !should_reassign(world, di, country) {
            continue;
        }
        let cur = world.divisions.locations[di];
        let best = bfs_nearest_target_with_capacity(
            &map,
            cur,
            &mop_targets,
            MAX_RALLY_DISTANCE,
            &assigned_targets,
            |p| {
                if enemy_div_locs.contains(&p.0) {
                    4
                } else {
                    2
                }
            },
            |p| can_enter_province(world, country, p),
        )
        .or_else(|| {
            bfs_nearest_target(&map, cur, &mop_targets, MAX_RALLY_DISTANCE, |p| {
                can_enter_province(world, country, p)
            })
        });
        if let Some((dest, _)) = best {
            if issue_ai_move_assignment(
                world,
                di,
                dest,
                DivisionAssignment {
                    front: enemy,
                    role: DivisionRole::MopUp,
                    target: Some(dest),
                    assigned_at: now,
                },
            ) {
                *assigned_targets.entry(dest.0).or_insert(0) += 1;
            }
        } else {
            // Bug #11 fallback: orphaned division → head to capital
            let cap = capital_province(world, country);
            if can_enter_province(world, country, cap) {
                issue_ai_move_assignment(
                    world,
                    di,
                    cap,
                    DivisionAssignment {
                        front: enemy,
                        role: DivisionRole::Reserve,
                        target: Some(cap),
                        assigned_at: now,
                    },
                );
            }
        }
    }
}

fn partition_divisions_by_segment(
    world: &World,
    country: CountryId,
    segments: &[FrontSegment],
) -> Vec<Vec<usize>> {
    let map = world.map.clone();
    let mut dists: Vec<std::collections::HashMap<u16, u32>> = Vec::with_capacity(segments.len());
    for seg in segments {
        let mut sources: Vec<ProvinceId> = seg
            .friendly_states
            .iter()
            .flat_map(|&sid| {
                let si = sid.0 as usize;
                if si < world.states.count {
                    world.states.provinces[si].clone()
                } else {
                    Vec::new()
                }
            })
            .collect();
        // Bug #14: include provinces from active army paths so that newly expanded
        // frontline provinces (daily tick_frontlines) are covered even when
        // partition runs only every 7 days.
        for army in &world.player_armies {
            if army.owner != country {
                continue;
            }
            if !army.order.as_ref().is_some_and(|o| o.active) {
                continue;
            }
            let army_seg_states: std::collections::HashSet<hoi4_state::ids::StateId> =
                seg.friendly_states.iter().copied().collect();
            if let Some(o) = &army.order {
                let overlaps = o.path.iter().any(|p| {
                    let pi = p.0 as usize;
                    pi < world.provinces.count
                        && army_seg_states.contains(&world.provinces.state_of[pi])
                });
                if overlaps {
                    for &p in &o.path {
                        sources.push(p);
                    }
                }
            }
        }
        let dist = bfs_multi_source_dist(&map, &sources, MAX_RALLY_DISTANCE);
        dists.push(dist);
    }

    let mut out: Vec<Vec<usize>> = vec![Vec::new(); segments.len()];
    for i in 0..world.divisions.count {
        if world.divisions.owners[i] != country {
            continue;
        }
        if world.player_locked_divisions.contains(&i) {
            if let Some(ref asgn) = world.divisions.assignments.get(i).and_then(|a| a.as_ref()) {
                if asgn.role == DivisionRole::Assault && !should_reassign(world, i, country) {
                    continue;
                }
            }
        }
        let cur = world.divisions.locations[i];
        let mut best: Option<(usize, f32)> = None;
        for (si, dist) in dists.iter().enumerate() {
            if let Some(&d) = dist.get(&cur.0) {
                let weight = china_theater::segment_weight(
                    world,
                    country,
                    segments[si].enemy,
                    &segments[si].enemy_states,
                );
                let weighted = d as f32 / weight.max(0.1);
                match best {
                    None => best = Some((si, weighted)),
                    Some((_, prev)) if weighted < prev => best = Some((si, weighted)),
                    _ => {}
                }
            }
        }
        if let Some((si, _)) = best {
            out[si].push(i);
        }
    }
    out
}

fn pick_attack_targets(world: &World, country: CountryId, seg: &FrontSegment) -> Vec<ProvinceId> {
    pick_attack_targets_impl(world, country, seg)
}

pub fn pick_attack_targets_export(
    world: &World,
    country: CountryId,
    seg: &FrontSegment,
) -> Vec<ProvinceId> {
    pick_attack_targets_impl(world, country, seg)
}

fn pick_attack_targets_impl(
    world: &World,
    country: CountryId,
    seg: &FrontSegment,
) -> Vec<ProvinceId> {
    let mut scored: Vec<(ProvinceId, i32)> = Vec::new();
    let enemy_garrison_by_prov = enemy_garrison_by_province(world, country);
    let sectors = evaluate_front_sectors(world, country, seg);
    let mut sector_bonus: std::collections::HashMap<u16, i32> = std::collections::HashMap::new();
    for (rank, sector) in sectors.iter().enumerate() {
        let rank_bonus = (sectors.len().saturating_sub(rank) as i32).min(8) * 4;
        let local = (sector.score.clamp(-50.0, 120.0) * 0.8) as i32 + rank_bonus;
        for &p in &sector.target_provinces {
            sector_bonus.insert(p.0, local);
        }
    }

    for &state_id in &seg.enemy_states {
        let si = state_id.0 as usize;
        if si >= world.states.count {
            continue;
        }
        let provs = &world.states.provinces[si];
        for &target_prov in provs {
            let pi = target_prov.0 as usize;
            if pi >= world.provinces.count
                || can_stage_from_province(world, country, target_prov)
                || !can_enter_province(world, country, target_prov)
            {
                continue;
            }
            let attack_width = count_friendly_attack_neighbors(world, country, target_prov) as i32;
            if attack_width == 0 {
                continue;
            }
            let enemy_garrison = enemy_garrison_by_prov
                .get(&target_prov.0)
                .copied()
                .unwrap_or(0);

            let vp_score = if si < world.data.states.len() {
                world.data.states[si]
                    .victory_points
                    .iter()
                    .map(|(_, v)| *v as i32)
                    .sum::<i32>()
            } else {
                0
            };
            let factory_score = world.state_building_levels(hoi4_state::StateId(si as u16)) as i32;
            let theater_bonus =
                china_theater::state_priority_bonus(world, country, state_id) as i32;
            let terrain_penalty = world
                .map
                .get_province(target_prov.0)
                .map(|def| target_terrain_penalty(&def.terrain))
                .unwrap_or(0);
            let score = 45
                + vp_score * 3
                + factory_score
                + theater_bonus
                + attack_width * 12
                + sector_bonus.get(&target_prov.0).copied().unwrap_or(0)
                - enemy_garrison * 14
                - terrain_penalty;
            scored.push((target_prov, score));
        }
    }

    scored.sort_by(|a, b| b.1.cmp(&a.1));
    scored.dedup_by_key(|(p, _)| p.0);
    scored.into_iter().map(|(p, _)| p).collect()
}

fn enemy_garrison_by_province(
    world: &World,
    country: CountryId,
) -> std::collections::HashMap<u16, i32> {
    let mut out = std::collections::HashMap::new();
    if world.runtime_country_indexes_valid {
        for ci in 0..world.countries.count {
            let owner = CountryId(ci as u16);
            if owner == country || !world.diplomacy.at_war_with(country, owner) {
                continue;
            }
            if let Some(indices) = world.country_division_index.get(ci) {
                for &di in indices {
                    add_healthy_garrison(world, di, &mut out);
                }
            }
        }
    } else {
        for di in 0..world.divisions.count {
            let owner = world.divisions.owners[di];
            if owner == country || !world.diplomacy.at_war_with(country, owner) {
                continue;
            }
            add_healthy_garrison(world, di, &mut out);
        }
    }
    out
}

fn add_healthy_garrison(
    world: &World,
    div_idx: usize,
    out: &mut std::collections::HashMap<u16, i32>,
) {
    if div_idx >= world.divisions.count {
        return;
    }
    let max_org = world.divisions.max_organisation[div_idx].max(1e-6);
    let org_ratio = world.divisions.organisation[div_idx] / max_org;
    if org_ratio < 0.05 || world.divisions.strength[div_idx] < 0.05 {
        return;
    }
    *out.entry(world.divisions.locations[div_idx].0).or_insert(0) += 1;
}

fn healthy_division_locations_for(
    world: &World,
    country: CountryId,
) -> std::collections::HashSet<u16> {
    let mut out = std::collections::HashSet::new();
    if let Some(indices) = country_division_indices(world, country) {
        for &di in indices {
            add_healthy_division_location(world, di, &mut out);
        }
    } else {
        for di in 0..world.divisions.count {
            if world.divisions.owners[di] != country {
                continue;
            }
            add_healthy_division_location(world, di, &mut out);
        }
    }
    out
}

fn count_friendly_attack_neighbors(world: &World, country: CountryId, target: ProvinceId) -> usize {
    let ti = target.0 as usize;
    if ti >= world.map.adjacencies.len() {
        return 0;
    }
    let mut count = 0;
    for &adj in &world.map.adjacencies[ti] {
        let ai = adj as usize;
        if ai >= world.provinces.count || !can_stage_from_province(world, country, ProvinceId(adj))
        {
            continue;
        }
        if world
            .map
            .get_province(adj)
            .is_some_and(|def| matches!(def.province_type, hoi4_map::ProvinceType::Land))
        {
            count += 1;
        }
    }
    count
}

fn target_terrain_penalty(terrain: &str) -> i32 {
    match terrain {
        "mountain" => 24,
        "hills" => 14,
        "forest" => 10,
        "jungle" => 18,
        "marsh" => 20,
        "urban" => 16,
        "desert" => 8,
        _ => 0,
    }
}

fn mop_up_remaining_enemies(
    world: &mut World,
    country: CountryId,
    handled_enemies: &std::collections::HashSet<CountryId>,
    enemy_div_locs: &std::collections::HashSet<u16>,
) {
    use std::collections::HashSet;

    let mut residual_enemies: Vec<CountryId> = Vec::new();
    for ci in 0..world.countries.count {
        let other = CountryId(ci as u16);
        if other == country {
            continue;
        }
        if !world.diplomacy.at_war_with(country, other) {
            continue;
        }
        if handled_enemies.contains(&other) {
            continue;
        }
        if world.diplomacy.annexed_countries.contains(&other) {
            continue;
        }
        residual_enemies.push(other);
    }
    if residual_enemies.is_empty() {
        return;
    }

    let enemy_set: HashSet<CountryId> = residual_enemies.iter().copied().collect();

    let mut residual_targets: HashSet<u16> = HashSet::new();
    for &enemy in &residual_enemies {
        for si in 0..world.states.count {
            if world.states.owners[si] != enemy {
                continue;
            }
            for &p in &world.states.provinces[si] {
                let pi = p.0 as usize;
                if pi >= world.provinces.count {
                    continue;
                }
                if world.provinces.controllers[pi] != country {
                    residual_targets.insert(p.0);
                }
            }
        }
    }
    let has_enemy_divs = world
        .divisions
        .owners
        .iter()
        .any(|&o| enemy_set.contains(&o));
    if has_enemy_divs {
        for &loc in enemy_div_locs {
            residual_targets.insert(loc);
        }
    }
    if residual_targets.is_empty() {
        return;
    }

    let mut idle_divs: Vec<usize> = Vec::new();
    let own_divisions: Vec<usize> = country_division_indices(world, country)
        .map(|indices| indices.to_vec())
        .unwrap_or_else(|| (0..world.divisions.count).collect());
    for i in own_divisions {
        if world.divisions.owners[i] != country {
            continue;
        }
        match world.divisions.assignments.get(i).and_then(|a| a.as_ref()) {
            Some(a) if a.role == DivisionRole::Assault && !should_reassign(world, i, country) => {
                continue
            }
            Some(a) if a.role == DivisionRole::MopUp && !should_reassign(world, i, country) => {
                continue
            }
            _ => {}
        }
        if world.player_locked_divisions.contains(&i) {
            let cur = world.divisions.locations[i];
            let cur_i = cur.0 as usize;
            if cur_i < world.provinces.count {
                let cur_ctrl = world.provinces.controllers[cur_i];
                if cur_ctrl != country {
                    continue;
                }
            }
        }
        match world.divisions.destinations[i] {
            None => idle_divs.push(i),
            Some(dest) => {
                let di = dest.0 as usize;
                if di < world.provinces.count
                    && world.provinces.controllers[di] == country
                    && !residual_targets.contains(&dest.0)
                {
                    idle_divs.push(i);
                }
            }
        }
    }
    if idle_divs.is_empty() {
        return;
    }

    let target_list: Vec<ProvinceId> = residual_targets.into_iter().map(ProvinceId).collect();
    let map = world.map.clone();
    let now = world.elapsed_hours;
    let mut assigned_targets: std::collections::HashMap<u16, u32> =
        std::collections::HashMap::new();

    for &di in &idle_divs {
        let cur = world.divisions.locations[di];
        if let Some((dest, _)) = bfs_nearest_target_with_capacity(
            &map,
            cur,
            &target_list,
            40,
            &assigned_targets,
            |p| {
                if enemy_div_locs.contains(&p.0) {
                    4
                } else {
                    2
                }
            },
            |p| can_enter_province(world, country, p),
        )
        .or_else(|| {
            bfs_nearest_target(&map, cur, &target_list, 40, |p| {
                can_enter_province(world, country, p)
            })
        }) {
            if issue_ai_move_assignment(
                world,
                di,
                dest,
                DivisionAssignment {
                    front: residual_enemies.first().copied(),
                    role: DivisionRole::MopUp,
                    target: Some(dest),
                    assigned_at: now,
                },
            ) {
                *assigned_targets.entry(dest.0).or_insert(0) += 1;
            }
        }
    }
}

pub fn execute_mop_up_only(world: &mut World, econ: &EconomyState, country: CountryId) {
    let empty: std::collections::HashSet<CountryId> = std::collections::HashSet::new();
    let enemies: std::collections::HashSet<CountryId> = (0..world.countries.count)
        .filter_map(|ci| {
            let other = CountryId(ci as u16);
            if other == country {
                return None;
            }
            if !world.diplomacy.at_war_with(country, other) {
                return None;
            }
            if world.diplomacy.annexed_countries.contains(&other) {
                return None;
            }
            Some(other)
        })
        .collect();
    let enemy_div_locs = build_enemy_div_locations(world, country, &enemies);
    mop_up_remaining_enemies(world, country, &empty, &enemy_div_locs);
    execute_overseas_reinforcement_orders(world, econ, country);
    execute_naval_invasion_orders(world, econ, country);
}

fn execute_overseas_reinforcement_orders(
    world: &mut World,
    econ: &EconomyState,
    country: CountryId,
) {
    let ports = usable_ports(world, country);
    if ports.len() < 2 {
        return;
    }

    let china_war = china_theater::is_japan_china_war_active(world, country);

    let destination_ports = if china_war {
        let strategy = china_theater::strategy_for(world, country);
        china_theater::resolve_reinforcement_ports(world, country, strategy.phase)
    } else {
        let mut enemy_front_provs = Vec::new();
        for pi in 0..world.provinces.count {
            let ctrl = world.provinces.controllers[pi];
            if ctrl.is_none() || !world.diplomacy.at_war_with(country, ctrl) {
                continue;
            }
            if let Some(def) = world.map.get_province(pi as u16) {
                if matches!(def.province_type, hoi4_map::ProvinceType::Land) {
                    enemy_front_provs.push(ProvinceId(pi as u16));
                }
            }
        }
        if enemy_front_provs.is_empty() {
            return;
        }
        let best = ports.iter().copied().min_by_key(|port| {
            enemy_front_provs
                .iter()
                .map(|p| port.0.abs_diff(p.0) as u32)
                .min()
                .unwrap_or(u32::MAX)
        });
        match best {
            Some(p) => vec![p],
            None => return,
        }
    };

    if destination_ports.is_empty() {
        return;
    }

    let max_issued = if china_war {
        PRIORITY_THEATER_REINFORCEMENTS_PER_TICK
    } else {
        OVERSEAS_REINFORCEMENTS_PER_TICK
    };

    let mut issued = 0usize;
    let own_divisions: Vec<usize> = country_division_indices(world, country)
        .map(|indices| indices.to_vec())
        .unwrap_or_else(|| (0..world.divisions.count).collect());
    for di in own_divisions {
        if issued >= max_issued {
            break;
        }
        if world.divisions.owners[di] != country
            || world.divisions.transport[di].is_some()
            || world.divisions.in_combat[di]
        {
            continue;
        }
        if !china_war && world.divisions.destinations[di].is_some() {
            continue;
        }
        if china_war && !is_available_for_overseas_reinforcement(world, di, country) {
            continue;
        }
        let cur = world.divisions.locations[di];

        let dest_port = if china_war {
            *destination_ports
                .iter()
                .min_by_key(|port| cur.0.abs_diff(port.0))
                .unwrap_or(&destination_ports[0])
        } else {
            destination_ports[0]
        };

        if cur == dest_port || cur.0.abs_diff(dest_port.0) < 8 {
            continue;
        }
        if order_overseas_transport(world, econ, di, dest_port).is_ok() {
            command_executor::issue_overseas_reinforce_command(
                world,
                di,
                dest_port,
                DivisionAssignment {
                    front: None,
                    role: DivisionRole::Reserve,
                    target: Some(dest_port),
                    assigned_at: world.elapsed_hours,
                },
            );
            issued += 1;
        }
    }
}

fn is_available_for_overseas_reinforcement(
    world: &World,
    div_idx: usize,
    country: CountryId,
) -> bool {
    if world.player_locked_divisions.contains(&div_idx) {
        return false;
    }
    match world
        .divisions
        .assignments
        .get(div_idx)
        .and_then(|a| a.as_ref())
    {
        Some(asgn)
            if asgn.role == DivisionRole::Assault && !should_reassign(world, div_idx, country) =>
        {
            false
        }
        Some(asgn)
            if asgn.role == DivisionRole::MopUp && !should_reassign(world, div_idx, country) =>
        {
            false
        }
        _ => true,
    }
}

fn execute_naval_invasion_orders(world: &mut World, econ: &EconomyState, country: CountryId) {
    let china_war = china_theater::is_japan_china_war_active(world, country);

    let mut targets = if china_war {
        let strategy = china_theater::strategy_for(world, country);
        let phase_targets =
            china_theater::invasion_targets_for_phase(world, country, strategy.phase);
        if phase_targets.is_empty() {
            coastal_invasion_targets(world, country)
        } else {
            phase_targets
                .into_iter()
                .map(|(p, score, _)| (p, score))
                .collect()
        }
    } else {
        coastal_invasion_targets(world, country)
    };

    if targets.is_empty() {
        return;
    }
    targets.sort_by(|a, b| b.1.cmp(&a.1));

    let mut issued = 0usize;
    let own_divisions: Vec<usize> = country_division_indices(world, country)
        .map(|indices| indices.to_vec())
        .unwrap_or_else(|| (0..world.divisions.count).collect());
    for di in own_divisions {
        if issued >= NAVAL_INVASIONS_PER_TICK {
            break;
        }
        if world.divisions.owners[di] != country
            || world.divisions.transport[di].is_some()
            || world.divisions.in_combat[di]
            || !is_available_for_overseas_reinforcement(world, di, country)
        {
            continue;
        }

        for &(target, _) in &targets {
            if order_naval_invasion(world, econ, di, target, 7 * 24).is_ok() {
                let asgn = DivisionAssignment {
                    front: Some(world.provinces.controllers[target.0 as usize]),
                    role: DivisionRole::Assault,
                    target: Some(target),
                    assigned_at: world.elapsed_hours,
                };
                command_executor::issue_invasion_command(
                    world,
                    di,
                    DivisionIntent::NavalInvasion,
                    target,
                    asgn,
                );
                issued += 1;
                break;
            }
        }
    }
}

fn coastal_invasion_targets(world: &World, country: CountryId) -> Vec<(ProvinceId, i32)> {
    let mut targets = Vec::new();
    for si in 0..world.states.count {
        let state = hoi4_state::StateId(si as u16);
        let controller = world.states.controllers[si];
        if controller == country || !world.diplomacy.at_war_with(country, controller) {
            continue;
        }

        let vp_score = world
            .data
            .states
            .get(si)
            .map(|s| s.victory_points.iter().map(|(_, vp)| *vp as i32).sum())
            .unwrap_or(0);
        let theater_bonus = if china_theater::is_japan_china_war_active(world, country)
            && china_theater::is_chinese_core_state(world, state)
        {
            china_theater::state_priority_bonus(world, country, state) as i32
        } else {
            0
        };
        let score = 50 + theater_bonus + vp_score * 3 + world.state_building_levels(state) as i32;

        for &p in &world.states.provinces[si] {
            let pi = p.0 as usize;
            if pi >= world.provinces.count || world.provinces.controllers[pi] == country {
                continue;
            }
            if !world
                .diplomacy
                .at_war_with(country, world.provinces.controllers[pi])
            {
                continue;
            }
            if world.map.get_province(p.0).is_some_and(|def| {
                matches!(def.province_type, hoi4_map::ProvinceType::Land) && def.coastal
            }) {
                targets.push((p, score));
            }
        }
    }
    targets
}

fn capital_province(world: &World, country: CountryId) -> ProvinceId {
    let ci = country.0 as usize;
    if ci >= world.countries.count {
        return ProvinceId(0);
    }
    let cap_state = world.countries.capitals[ci];
    if cap_state.is_none() {
        return ProvinceId(0);
    }
    let si = cap_state.0 as usize;
    if si >= world.states.count || world.states.provinces[si].is_empty() {
        return ProvinceId(0);
    }
    world.states.provinces[si][0]
}

fn bfs_multi_source_dist(
    map: &hoi4_map::GameMap,
    sources: &[ProvinceId],
    max_depth: u32,
) -> std::collections::HashMap<u16, u32> {
    let mut dist: std::collections::HashMap<u16, u32> = std::collections::HashMap::new();
    let mut queue: std::collections::VecDeque<(u16, u32)> = std::collections::VecDeque::new();
    for &s in sources {
        dist.insert(s.0, 0);
        queue.push_back((s.0, 0));
    }
    while let Some((node, depth)) = queue.pop_front() {
        if depth >= max_depth {
            continue;
        }
        let ni = node as usize;
        if ni >= map.adjacencies.len() {
            continue;
        }
        for &nb in &map.adjacencies[ni] {
            if let Some(def) = map.get_province(nb) {
                if !matches!(def.province_type, hoi4_map::ProvinceType::Land) {
                    continue;
                }
            } else {
                continue;
            }
            if dist.contains_key(&nb) {
                continue;
            }
            dist.insert(nb, depth + 1);
            queue.push_back((nb, depth + 1));
        }
    }
    dist
}

fn bfs_nearest_target<F>(
    map: &hoi4_map::GameMap,
    start: ProvinceId,
    targets: &[ProvinceId],
    max_depth: u32,
    mut can_enter: F,
) -> Option<(ProvinceId, u32)>
where
    F: FnMut(ProvinceId) -> bool,
{
    let target_set: std::collections::HashSet<u16> = targets.iter().map(|p| p.0).collect();
    if target_set.is_empty() {
        return None;
    }
    let mut visited: std::collections::HashSet<u16> = std::collections::HashSet::new();
    let mut queue: std::collections::VecDeque<(u16, u32)> = std::collections::VecDeque::new();
    visited.insert(start.0);
    queue.push_back((start.0, 0));

    while let Some((node, depth)) = queue.pop_front() {
        let p = ProvinceId(node);
        if target_set.contains(&node) && can_enter(p) {
            return Some((p, depth));
        }
        if depth >= max_depth {
            continue;
        }
        let ni = node as usize;
        if ni >= map.adjacencies.len() {
            continue;
        }
        for &nb in &map.adjacencies[ni] {
            if !visited.insert(nb) {
                continue;
            }
            if let Some(def) = map.get_province(nb) {
                if !matches!(def.province_type, hoi4_map::ProvinceType::Land) {
                    continue;
                }
            } else {
                continue;
            }
            queue.push_back((nb, depth + 1));
        }
    }
    None
}

fn bfs_nearest_target_with_capacity<F, C>(
    map: &hoi4_map::GameMap,
    start: ProvinceId,
    targets: &[ProvinceId],
    max_depth: u32,
    assigned: &std::collections::HashMap<u16, u32>,
    mut capacity: C,
    mut can_enter: F,
) -> Option<(ProvinceId, u32)>
where
    F: FnMut(ProvinceId) -> bool,
    C: FnMut(ProvinceId) -> u32,
{
    let filtered: Vec<ProvinceId> = targets
        .iter()
        .copied()
        .filter(|&p| assigned.get(&p.0).copied().unwrap_or(0) < capacity(p))
        .collect();
    bfs_nearest_target(map, start, &filtered, max_depth, |p| can_enter(p))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn active_assault_is_not_available_for_mop_up() {
        let mut world = test_world();
        world.provinces.controllers[1] = CountryId(0);
        world.provinces.controllers[2] = CountryId(1);
        let mut war = hoi4_state::War {
            id: 0,
            primary_attacker: CountryId(0),
            primary_defender: CountryId(1),
            attackers: std::collections::HashSet::new(),
            defenders: std::collections::HashSet::new(),
            started_at_hour: 0,
            attacker_war_score: 0.0,
            defender_war_score: 0.0,
            attacker_wargoals: vec![],
            defender_wargoals: vec![],
            war_join_policies: std::collections::HashMap::new(),
        };
        war.attackers.insert(CountryId(0));
        war.defenders.insert(CountryId(1));
        world.diplomacy.wars.insert(0, war);
        world.divisions.push(
            CountryId(0),
            ProvinceId(1),
            0,
            60.0,
            1000.0,
            "assault".into(),
        );
        world.divisions.destinations[0] = Some(ProvinceId(2));
        world.divisions.assignments[0] = Some(DivisionAssignment {
            front: Some(CountryId(1)),
            role: DivisionRole::Assault,
            target: Some(ProvinceId(2)),
            assigned_at: 0,
        });

        assert!(is_active_assault(&world, 0, CountryId(0)));
        assert!(!should_reassign(&world, 0, CountryId(0)));
    }

    #[test]
    fn overseas_reinforcement_can_pull_reserves_but_not_active_assaults() {
        let mut world = test_world();
        world.provinces.controllers[1] = CountryId(0);
        world.provinces.controllers[2] = CountryId(1);
        let mut war = hoi4_state::War {
            id: 0,
            primary_attacker: CountryId(0),
            primary_defender: CountryId(1),
            attackers: std::collections::HashSet::new(),
            defenders: std::collections::HashSet::new(),
            started_at_hour: 0,
            attacker_war_score: 0.0,
            defender_war_score: 0.0,
            attacker_wargoals: vec![],
            defender_wargoals: vec![],
            war_join_policies: std::collections::HashMap::new(),
        };
        war.attackers.insert(CountryId(0));
        war.defenders.insert(CountryId(1));
        world.diplomacy.wars.insert(0, war);

        let reserve = world.divisions.push(
            CountryId(0),
            ProvinceId(1),
            0,
            60.0,
            1000.0,
            "reserve".into(),
        ) as usize;
        world.divisions.destinations[reserve] = Some(ProvinceId(1));
        world.divisions.assignments[reserve] = Some(DivisionAssignment {
            front: Some(CountryId(1)),
            role: DivisionRole::Reserve,
            target: Some(ProvinceId(1)),
            assigned_at: 0,
        });

        let assault = world.divisions.push(
            CountryId(0),
            ProvinceId(1),
            0,
            60.0,
            1000.0,
            "assault".into(),
        ) as usize;
        world.divisions.destinations[assault] = Some(ProvinceId(2));
        world.divisions.assignments[assault] = Some(DivisionAssignment {
            front: Some(CountryId(1)),
            role: DivisionRole::Assault,
            target: Some(ProvinceId(2)),
            assigned_at: 0,
        });

        assert!(is_available_for_overseas_reinforcement(
            &world,
            reserve,
            CountryId(0)
        ));
        assert!(!is_available_for_overseas_reinforcement(
            &world,
            assault,
            CountryId(0)
        ));
    }

    fn test_world() -> World {
        let definitions = vec![
            None,
            Some(hoi4_map::ProvinceDefinition {
                id: 1,
                r: 0,
                g: 0,
                b: 0,
                province_type: hoi4_map::ProvinceType::Land,
                coastal: false,
                terrain: String::new(),
                continent: 0,
            }),
            Some(hoi4_map::ProvinceDefinition {
                id: 2,
                r: 0,
                g: 0,
                b: 0,
                province_type: hoi4_map::ProvinceType::Land,
                coastal: false,
                terrain: String::new(),
                continent: 0,
            }),
        ];
        World {
            date: hoi4_state::GameDate::START,
            speed: hoi4_state::GameSpeed::Paused,
            elapsed_hours: 0,
            provinces: hoi4_state::ProvinceStore::new(3),
            states: hoi4_state::StateStore::new(1),
            countries: hoi4_state::CountryStore::new(2),
            divisions: hoi4_state::DivisionStore::new(),
            ships: hoi4_state::ShipStore::new(),
            fleets: hoi4_state::FleetStore::new(),
            air_wings: hoi4_state::AirWingStore::new(),
            diplomacy: hoi4_state::DiplomacyState::default(),
            command: hoi4_state::command::CommandHierarchy::default(),
            generals: Vec::new(),
            next_general_id: 0,
            map: Arc::new(hoi4_map::GameMap {
                definitions,
                rgb_to_id: std::collections::HashMap::new(),
                province_map: hoi4_map::ProvinceMap {
                    width: 0,
                    height: 0,
                    pixels: vec![],
                },
                adjacencies: vec![vec![], vec![2], vec![1]],
                special_adjacencies: vec![],
                heightmap: hoi4_map::Heightmap {
                    width: 0,
                    height: 0,
                    pixels: vec![],
                },
                terrain_bmp: hoi4_map::TerrainBitmap {
                    width: 0,
                    height: 0,
                    pixels: vec![],
                    palette: [[0; 3]; 256],
                },
                terrain_catalog: hoi4_map::TerrainCatalog::default(),
                tree_definition_bmp: None,
                tree_indices: hoi4_map::DEFAULT_TREE_INDICES.iter().copied().collect(),
            }),
            data: Arc::new(hoi4_data::GameData::default()),
            tag_to_country: std::collections::HashMap::new(),
            state_id_lookup: std::collections::HashMap::new(),
            player: CountryId::NONE,
            random_seed: 0,
            game_unique_id: 0,
            path_cache: std::collections::HashMap::new(),
            path_cache_day: 0,
            prov_div_index: std::collections::HashMap::new(),
            country_state_index: vec![Vec::new(); 2],
            country_pop_index: vec![Vec::new(); 2],
            country_building_index: vec![Vec::new(); 2],
            country_division_index: vec![Vec::new(); 2],
            country_fleet_index: vec![Vec::new(); 2],
            country_air_wing_index: vec![Vec::new(); 2],
            runtime_country_indexes_valid: false,
            trade_export_surplus_index: std::collections::HashMap::new(),
            player_armies: Vec::new(),
            player_locked_divisions: std::collections::HashSet::new(),
            next_army_id: 0,
        }
    }
}
