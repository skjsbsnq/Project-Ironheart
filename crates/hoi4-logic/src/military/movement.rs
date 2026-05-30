//! Army movement: daily tick moves divisions toward their destination.

use hoi4_state::{ArmyTransportPhase, ArmyTransportState, CountryId, ProvinceId, StateId, World};
use std::collections::{HashMap, VecDeque};

use crate::economy::EconomyState;

const EMBARK_HOURS: u64 = 24;
const DISEMBARK_HOURS: u64 = 24;
const SEA_HOURS_PER_REGION: u64 = 24;
const CONVOYS_PER_DIVISION: f32 = 5.0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportOrderError {
    InvalidDivision,
    DivisionBusy,
    NoEmbarkPort,
    NoDebarkPort,
    NoConvoyCapacity,
    CannotEnterDestination,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NavalInvasionError {
    InvalidDivision,
    DivisionBusy,
    NoEmbarkPort,
    TargetNotCoastalLand,
    TargetNotEnemyControlled,
    NotPrepared,
    NoConvoyCapacity,
    InsufficientNavalSupport { required: f32, actual: f32 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DivisionMoveOrderError {
    InvalidDivision,
    UnownedDivision,
    CannotEnterDestination,
}

/// Check whether a division owned by `division_owner` may enter `target_province`.
pub fn can_enter_province(
    world: &World,
    division_owner: CountryId,
    target_province: ProvinceId,
) -> bool {
    let controller = world.provinces.controllers[target_province.0 as usize];
    if controller.is_none() {
        return true;
    }
    if controller == division_owner {
        return true;
    }
    if world.diplomacy.at_war_with(division_owner, controller) {
        return true;
    }
    if world
        .diplomacy
        .has_military_access(division_owner, controller)
    {
        return true;
    }
    if world.diplomacy.is_subject_of(controller, division_owner)
        || world.diplomacy.is_subject_of(division_owner, controller)
    {
        return true;
    }
    // Same faction check
    if let Some(f) = world.diplomacy.faction_of(division_owner) {
        if world.diplomacy.faction_of(controller) == Some(f) {
            return true;
        }
    }
    false
}

/// Authoritative API for assigning normal division movement destinations.
///
/// AI should emit move intent through this function instead of mutating
/// `World.divisions.destinations` directly. The movement system remains the
/// owner of destination validation and final writes.
pub fn issue_division_move_order(
    world: &mut World,
    division_idx: usize,
    destination: ProvinceId,
) -> Result<(), DivisionMoveOrderError> {
    if division_idx >= world.divisions.count {
        return Err(DivisionMoveOrderError::InvalidDivision);
    }
    let owner = world.divisions.owners[division_idx];
    if owner.is_none() {
        return Err(DivisionMoveOrderError::UnownedDivision);
    }
    if !can_enter_province(world, owner, destination) {
        return Err(DivisionMoveOrderError::CannotEnterDestination);
    }
    world.divisions.destinations[division_idx] = Some(destination);
    Ok(())
}

/// Return the first legal land step for a normal division move order.
///
/// This uses the same validation and path cache as [`daily_movement_tick`], so
/// UI previews can animate toward the movement system's actual next province
/// instead of following a purely visual path.
pub fn next_step_toward_destination(
    world: &mut World,
    division_idx: usize,
    destination: ProvinceId,
) -> Result<Option<ProvinceId>, DivisionMoveOrderError> {
    if division_idx >= world.divisions.count {
        return Err(DivisionMoveOrderError::InvalidDivision);
    }
    let owner = world.divisions.owners[division_idx];
    if owner.is_none() {
        return Err(DivisionMoveOrderError::UnownedDivision);
    }
    if !can_enter_province(world, owner, destination) {
        return Err(DivisionMoveOrderError::CannotEnterDestination);
    }

    let cur = world.divisions.locations[division_idx];
    let next = next_step_cached(world, owner, cur, destination);
    Ok((next != cur).then_some(next))
}

pub fn clear_division_move_order(
    world: &mut World,
    division_idx: usize,
) -> Result<(), DivisionMoveOrderError> {
    if division_idx >= world.divisions.count {
        return Err(DivisionMoveOrderError::InvalidDivision);
    }
    world.divisions.destinations[division_idx] = None;
    world.divisions.move_progress[division_idx] = 0.0;
    Ok(())
}

/// Speed: divisions move 1 province per day (vanilla HOI4 simplified).
/// Call once per day for all divisions.
pub fn daily_movement_tick(world: &mut World) {
    for i in 0..world.divisions.count {
        if world.divisions.transport[i].is_some() {
            continue;
        }
        let dest = match world.divisions.destinations[i] {
            Some(d) => d,
            None => continue,
        };
        let cur = world.divisions.locations[i];
        if cur == dest {
            world.divisions.destinations[i] = None;
            world.divisions.move_progress[i] = 0.0;
            continue;
        }

        let owner = world.divisions.owners[i];
        let next = next_step_cached(world, owner, cur, dest);
        if next == cur {
            // Bug #21 fix: BFS cannot find a path to destination. Do NOT clear
            // the destination — that causes should_reassign → AI reassigns the
            // same target → BFS fails again → clear → infinite loop. Instead,
            // keep the destination and wait; the path may become reachable later
            // (e.g., after occupying an intermediate province).
            continue;
        }
        if !can_enter_province(world, owner, next) {
            world.divisions.destinations[i] = None;
            continue;
        }

        let next_idx = next.0 as usize;
        let prov_ctrl = world.provinces.controllers[next_idx];
        let mut blocked_by_combat = false;
        if !prov_ctrl.is_none()
            && prov_ctrl != owner
            && world.diplomacy.at_war_with(owner, prov_ctrl)
        {
            let divs_in_next: Vec<usize> = world.divisions_in_province(next).to_vec();
            // 修复 #8：单只残兵不应阻挡整军推进。要求敌方驻军聚合战斗力
            // (sum of strength × org_ratio) 达到 BLOCKING_GARRISON_THRESHOLD 才视为
            // 真正能形成阻挡的守备力量；低于此阈值的零散残兵直接被穿过。
            // 阈值与"破碎"下限 0.05 对齐——任何未破碎的师都应拦截入侵，
            // 防止疲劳但仍可作战的守军被无视。
            const BLOCKING_GARRISON_THRESHOLD: f32 = 0.05;
            let mut enemy_combat_power = 0.0f32;
            for &j in &divs_in_next {
                if j == i {
                    continue;
                }
                let other_owner = world.divisions.owners[j];
                if other_owner != owner && world.diplomacy.at_war_with(owner, other_owner) {
                    let max_org = world.divisions.max_organisation[j].max(1e-6);
                    let org_ratio = (world.divisions.organisation[j] / max_org).clamp(0.0, 1.0);
                    let str_now = world.divisions.strength[j].clamp(0.0, 1.0);
                    if org_ratio < 0.05 || str_now < 0.05 {
                        continue;
                    }
                    enemy_combat_power += str_now * org_ratio;
                }
            }
            if enemy_combat_power >= BLOCKING_GARRISON_THRESHOLD {
                world.divisions.in_combat[i] = true;
                for j in divs_in_next {
                    let other_owner = world.divisions.owners[j];
                    if other_owner != owner && world.diplomacy.at_war_with(owner, other_owner) {
                        world.divisions.in_combat[j] = true;
                    }
                }
                blocked_by_combat = true;
            }
        }

        if blocked_by_combat {
            continue;
        }

        world.divisions.locations[i] = next;
        world.divisions.in_combat[i] = false;
        if next == dest {
            world.divisions.destinations[i] = None;
            world.divisions.move_progress[i] = 0.0;
        }
    }

    let map = world.map.clone();
    let mut need_retreat: Vec<usize> = Vec::new();
    for i in 0..world.divisions.count {
        if world.divisions.transport[i].is_some() {
            continue;
        }
        let owner = world.divisions.owners[i];
        if owner.is_none() {
            continue;
        }
        let cur = world.divisions.locations[i];
        let pi = cur.0 as usize;
        if pi >= map.adjacencies.len() {
            continue;
        }
        let cur_ctrl = world.provinces.controllers[pi];

        if cur_ctrl == owner {
            let max_org = world.divisions.max_organisation[i].max(1e-6);
            let org_ratio = world.divisions.organisation[i] / max_org;
            let str_now = world.divisions.strength[i];
            let is_broken = org_ratio < 0.05 || str_now < 0.05;
            if is_broken {
                if let Some(dest) = world.divisions.destinations[i] {
                    let di = dest.0 as usize;
                    if di < world.provinces.count && world.provinces.controllers[di] != owner {
                        world.divisions.destinations[i] = None;
                    }
                }
            }
            continue;
        }

        let max_org = world.divisions.max_organisation[i].max(1e-6);
        let org_ratio = world.divisions.organisation[i] / max_org;
        let str_now = world.divisions.strength[i];
        let is_broken = org_ratio < 0.05 || str_now < 0.05;

        if is_broken || world.divisions.destinations[i].is_none() {
            need_retreat.push(i);
        }
    }

    if !need_retreat.is_empty() {
        let mut by_owner: std::collections::HashMap<CountryId, Vec<usize>> =
            std::collections::HashMap::new();
        for &di in &need_retreat {
            let owner = world.divisions.owners[di];
            by_owner.entry(owner).or_default().push(di);
        }
        for (&owner, divs) in &by_owner {
            let sources: Vec<ProvinceId> = divs
                .iter()
                .map(|&di| world.divisions.locations[di])
                .collect();
            let dist = bfs_multi_source_retreat_dist(&map, world, owner, &sources);
            for &di in divs {
                let cur = world.divisions.locations[di];
                if let Some(&next) = dist.get(&cur.0) {
                    world.divisions.destinations[di] = Some(ProvinceId(next));
                }
            }
        }
    }
}

pub fn order_overseas_transport(
    world: &mut World,
    econ: &EconomyState,
    division_idx: usize,
    destination: ProvinceId,
) -> Result<(), TransportOrderError> {
    if division_idx >= world.divisions.count {
        return Err(TransportOrderError::InvalidDivision);
    }
    if world.divisions.transport[division_idx].is_some() || world.divisions.in_combat[division_idx]
    {
        return Err(TransportOrderError::DivisionBusy);
    }

    let owner = world.divisions.owners[division_idx];
    if !can_enter_province(world, owner, destination) {
        return Err(TransportOrderError::CannotEnterDestination);
    }
    if available_convoys(econ, owner) < CONVOYS_PER_DIVISION {
        return Err(TransportOrderError::NoConvoyCapacity);
    }

    let origin = world.divisions.locations[division_idx];
    let origin_port =
        nearest_usable_port(world, owner, origin).ok_or(TransportOrderError::NoEmbarkPort)?;
    let destination_port =
        nearest_usable_port(world, owner, destination).ok_or(TransportOrderError::NoDebarkPort)?;
    let route_regions = port_route_regions(world, origin_port, destination_port);

    world.divisions.destinations[division_idx] = None;
    world.divisions.move_progress[division_idx] = 0.0;
    world.divisions.in_combat[division_idx] = false;
    world.divisions.transport[division_idx] = Some(ArmyTransportState {
        phase: ArmyTransportPhase::Embarking,
        origin_port,
        destination_port,
        route_regions,
        phase_ends_at_hour: world.elapsed_hours
            + EMBARK_HOURS
            + land_delay(world, owner, origin, origin_port),
    });
    Ok(())
}

pub fn order_naval_invasion(
    world: &mut World,
    econ: &EconomyState,
    division_idx: usize,
    target: ProvinceId,
    prepared_hours: u64,
) -> Result<(), NavalInvasionError> {
    const REQUIRED_PREP_HOURS: u64 = 7 * 24;
    const REQUIRED_SUPPORT: f32 = 0.5;

    if division_idx >= world.divisions.count {
        return Err(NavalInvasionError::InvalidDivision);
    }
    if world.divisions.transport[division_idx].is_some() || world.divisions.in_combat[division_idx]
    {
        return Err(NavalInvasionError::DivisionBusy);
    }
    if !is_coastal_land(world, target) {
        return Err(NavalInvasionError::TargetNotCoastalLand);
    }

    let owner = world.divisions.owners[division_idx];
    let target_controller = world.provinces.controllers[target.0 as usize];
    if target_controller.is_none()
        || target_controller == owner
        || !world.diplomacy.at_war_with(owner, target_controller)
    {
        return Err(NavalInvasionError::TargetNotEnemyControlled);
    }
    if prepared_hours < REQUIRED_PREP_HOURS {
        return Err(NavalInvasionError::NotPrepared);
    }
    if available_convoys(econ, owner) < CONVOYS_PER_DIVISION {
        return Err(NavalInvasionError::NoConvoyCapacity);
    }

    let origin = world.divisions.locations[division_idx];
    let origin_port =
        nearest_usable_port(world, owner, origin).ok_or(NavalInvasionError::NoEmbarkPort)?;
    let route_regions = port_route_regions(world, origin_port, target);
    let support = crate::naval::missions::naval_invasion_support_for_route(
        world,
        world.data.as_ref(),
        owner,
        &route_regions,
    );
    if support < REQUIRED_SUPPORT {
        return Err(NavalInvasionError::InsufficientNavalSupport {
            required: REQUIRED_SUPPORT,
            actual: support,
        });
    }

    world.divisions.destinations[division_idx] = None;
    world.divisions.move_progress[division_idx] = 0.0;
    world.divisions.in_combat[division_idx] = false;
    world.divisions.transport[division_idx] = Some(ArmyTransportState {
        phase: ArmyTransportPhase::Embarking,
        origin_port,
        destination_port: target,
        route_regions,
        phase_ends_at_hour: world.elapsed_hours
            + EMBARK_HOURS
            + land_delay(world, owner, origin, origin_port),
    });
    Ok(())
}

pub fn daily_transport_tick(world: &mut World, econ: &mut EconomyState) {
    let now = world.elapsed_hours;
    for i in 0..world.divisions.count {
        let Some(mut transport) = world.divisions.transport[i].clone() else {
            continue;
        };
        if now < transport.phase_ends_at_hour {
            continue;
        }
        match transport.phase {
            ArmyTransportPhase::Embarking => {
                world.divisions.locations[i] = transport.origin_port;
                transport.phase = ArmyTransportPhase::AtSea;
                let owner = world.divisions.owners[i];
                let risk = crate::naval::missions::convoy_route_risk(
                    world,
                    world.data.as_ref(),
                    owner,
                    &transport.route_regions,
                );
                let delay = (transport.route_regions.len().max(1) as f32
                    * SEA_HOURS_PER_REGION as f32
                    / risk.safe_capacity_mult.max(0.2)) as u64;
                crate::naval::missions::consume_convoys_for_route(
                    world,
                    econ,
                    owner,
                    &transport.route_regions,
                    CONVOYS_PER_DIVISION,
                );
                world.divisions.organisation[i] *= (1.0 - risk.loss_rate * 0.5).clamp(0.25, 1.0);
                transport.phase_ends_at_hour = now + delay.max(SEA_HOURS_PER_REGION);
                world.divisions.transport[i] = Some(transport);
            }
            ArmyTransportPhase::AtSea => {
                transport.phase = ArmyTransportPhase::Disembarking;
                transport.phase_ends_at_hour = now + DISEMBARK_HOURS;
                world.divisions.transport[i] = Some(transport);
            }
            ArmyTransportPhase::Disembarking => {
                let owner = world.divisions.owners[i];
                if can_enter_province(world, owner, transport.destination_port) {
                    world.divisions.locations[i] = transport.destination_port;
                }
                world.divisions.transport[i] = None;
                world.divisions.in_combat[i] = false;
            }
        }
    }
}

pub fn is_division_in_transport(world: &World, division_idx: usize) -> bool {
    division_idx < world.divisions.count && world.divisions.transport[division_idx].is_some()
}

pub fn usable_ports(world: &World, owner: CountryId) -> Vec<ProvinceId> {
    let mut ports = Vec::new();
    for si in 0..world.states.count {
        let state = StateId(si as u16);
        if !state_has_port_building(world, state) {
            continue;
        }
        let controller = world.states.controllers[si];
        let same_faction = world
            .diplomacy
            .faction_of(owner)
            .map(|f| world.diplomacy.faction_of(controller) == Some(f))
            .unwrap_or(false);
        if world.states.owners[si] != owner
            && controller != owner
            && !world.diplomacy.has_military_access(owner, controller)
            && !same_faction
        {
            continue;
        }
        if let Some(port) = world.states.provinces[si]
            .iter()
            .copied()
            .find(|p| is_coastal_land(world, *p))
        {
            ports.push(port);
        }
    }
    ports.sort_by_key(|p| p.0);
    ports.dedup();
    ports
}

fn nearest_usable_port(world: &World, owner: CountryId, from: ProvinceId) -> Option<ProvinceId> {
    usable_ports(world, owner)
        .into_iter()
        .filter(|p| can_enter_province(world, owner, *p))
        .min_by_key(|p| land_distance(world, from, *p).unwrap_or(u32::MAX / 2))
}

fn available_convoys(econ: &EconomyState, owner: CountryId) -> f32 {
    if owner.is_none() {
        return 0.0;
    }
    econ.stockpile
        .get(owner.0 as usize)
        .and_then(|s| s.get("convoy"))
        .copied()
        .unwrap_or(0.0)
}

fn state_has_port_building(world: &World, state: StateId) -> bool {
    world
        .countries
        .buildings_v6
        .buildings
        .iter()
        .any(|building| {
            building.state == state
                && building.level > 0
                && matches!(
                    building.building_def_id.as_str(),
                    "port" | "naval_base" | "v6_naval_base" | "shipyard"
                )
        })
}

pub fn port_route_regions_pub(
    world: &World,
    origin: ProvinceId,
    destination: ProvinceId,
) -> Vec<u32> {
    port_route_regions(world, origin, destination)
}

fn port_route_regions(world: &World, origin: ProvinceId, destination: ProvinceId) -> Vec<u32> {
    let sea_regions = crate::naval::regions::SeaRegionMap::from_world(world);
    let mut regions = Vec::new();
    if let Some(region) = sea_regions
        .region_for_port(origin)
        .or_else(|| sea_regions.nearest_region_for_port(world, origin))
    {
        regions.push(region);
    }
    if let Some(region) = sea_regions
        .region_for_port(destination)
        .or_else(|| sea_regions.nearest_region_for_port(world, destination))
    {
        regions.push(region);
    }
    if regions.is_empty() {
        regions.push(crate::naval::regions::NO_SEA_REGION);
    }
    regions.sort_unstable();
    regions.dedup();
    regions
}

fn land_delay(world: &World, owner: CountryId, from: ProvinceId, to: ProvinceId) -> u64 {
    if from == to {
        return 0;
    }
    if !can_enter_province(world, owner, to) {
        return 0;
    }
    land_distance(world, from, to).unwrap_or(0) as u64 * 24
}

pub fn land_distance_approx(world: &World, from: ProvinceId, to: ProvinceId) -> u32 {
    land_distance(world, from, to).unwrap_or(u32::MAX / 2)
}

fn land_distance(world: &World, from: ProvinceId, to: ProvinceId) -> Option<u32> {
    if from == to {
        return Some(0);
    }
    let map = &world.map;
    let mut visited = std::collections::HashSet::new();
    let mut queue = std::collections::VecDeque::new();
    visited.insert(from.0);
    queue.push_back((from.0, 0u32));
    while let Some((node, depth)) = queue.pop_front() {
        if depth >= 80 || node as usize >= map.adjacencies.len() {
            continue;
        }
        for &nb in &map.adjacencies[node as usize] {
            if !is_land(map, nb) || !visited.insert(nb) {
                continue;
            }
            if nb == to.0 {
                return Some(depth + 1);
            }
            queue.push_back((nb, depth + 1));
        }
    }
    None
}

fn is_coastal_land(world: &World, province: ProvinceId) -> bool {
    let Some(def) = world.map.get_province(province.0) else {
        return false;
    };
    if !matches!(def.province_type, hoi4_map::ProvinceType::Land) || !def.coastal {
        return false;
    }
    let pi = province.0 as usize;
    pi < world.map.adjacencies.len()
        && world.map.adjacencies[pi]
            .iter()
            .any(|p| !is_land(&world.map, *p))
}

/// 从多个 source 出发 BFS，找到每个 source 到最近的 controller==owner 省份。
/// 返回 HashMap<当前省 raw_id, 目标友省 raw_id>，用于批量撤退。
fn bfs_multi_source_retreat_dist(
    map: &hoi4_map::GameMap,
    world: &World,
    owner: CountryId,
    sources: &[ProvinceId],
) -> HashMap<u16, u16> {
    use std::collections::{HashMap, HashSet};
    const MAX_DEPTH: u32 = 30;
    let mut result: HashMap<u16, u16> = HashMap::new();
    let mut visited: HashSet<u16> = HashSet::new();
    let mut queue: VecDeque<(u16, u32, u16)> = VecDeque::new();
    for &s in sources {
        visited.insert(s.0);
        queue.push_back((s.0, 0, s.0));
    }
    while let Some((node, depth, source)) = queue.pop_front() {
        if result.len() == sources.len() {
            break;
        }
        if result.contains_key(&source) {
            continue;
        }
        if depth >= MAX_DEPTH {
            continue;
        }
        if (node as usize) >= map.adjacencies.len() {
            continue;
        }
        for &nb in &map.adjacencies[node as usize] {
            if !is_land(map, nb) {
                continue;
            }
            if !visited.insert(nb) {
                continue;
            }
            let nb_idx = nb as usize;
            if nb_idx >= world.provinces.count {
                continue;
            }
            if world.provinces.controllers[nb_idx] == owner {
                if !result.contains_key(&source) {
                    result.insert(source, nb);
                }
            } else {
                queue.push_back((nb, depth + 1, source));
            }
        }
    }
    result
}

/// Phase 1.5: 占领判定 — 每天扫一遍所有 land 省份，按 vanilla HOI4 模型翻 controller。
///
/// 规则（与 vanilla HOI4 一致）：
/// 1. 该省内有原 controller 阵营的师 → 不变（驻军在守）
/// 2. 该省内只有敌方师（与原 controller 在战争）→ 立即翻 controller
/// 3. 该省完全空 → 不变（保持现状，类似 vanilla 的"无人区"）
///
/// 这样"经过即占领"的 vanilla 行为得以实现，但占领不会自己消失（除非
/// 原 controller 派师反推回来 + 把敌方守军赶走）。
pub fn daily_occupation_tick(world: &mut World) {
    let prov_count = world.provinces.count;
    let mut controller_changed = false;
    let mut occupied_provinces: HashMap<u16, HashMap<CountryId, u32>> = HashMap::new();
    for di in 0..world.divisions.count {
        if world.divisions.transport[di].is_some() {
            continue;
        }
        let prov = world.divisions.locations[di];
        let pi = prov.0 as usize;
        if pi >= prov_count {
            continue;
        }
        let owner = world.divisions.owners[di];
        if owner.is_none() {
            continue;
        }
        let max_org = world.divisions.max_organisation[di].max(1e-6);
        let org_ratio = world.divisions.organisation[di] / max_org;
        let str_now = world.divisions.strength[di];
        if org_ratio < 0.05 || str_now < 0.05 {
            continue;
        }
        *occupied_provinces
            .entry(prov.0)
            .or_default()
            .entry(owner)
            .or_insert(0) += 1;
    }

    let map = world.map.clone();

    // 第二步：只判定有有效部队驻扎的省份；空省不会翻 controller。
    for (&raw_id, occupants) in &occupied_provinces {
        let pi = raw_id as usize;
        if pi >= prov_count {
            continue;
        }
        let cur_ctrl = world.provinces.controllers[pi];
        if cur_ctrl.is_none() {
            continue;
        }
        // 只处理陆地省（不让海域 controller 翻飞）
        if (raw_id as usize) >= map.adjacencies.len() {
            continue;
        }
        if let Some(def) = map.get_province(raw_id) {
            if !matches!(def.province_type, hoi4_map::ProvinceType::Land) {
                continue;
            }
        } else {
            continue;
        }

        // 当前 controller 阵营是否还有师驻扎？
        let mut has_controller_garrison = false;
        let mut strongest_enemy: Option<(CountryId, u32)> = None;
        for (&ctry, &cnt) in occupants {
            if ctry == cur_ctrl {
                has_controller_garrison = true;
            } else if world.diplomacy.at_war_with(ctry, cur_ctrl) {
                if let Some((_, prev)) = strongest_enemy {
                    if cnt > prev {
                        strongest_enemy = Some((ctry, cnt));
                    }
                } else {
                    strongest_enemy = Some((ctry, cnt));
                }
            }
        }

        // 翻 controller：当前 controller 无驻军，且有敌方驻军
        if !has_controller_garrison {
            if let Some((new_ctrl, _)) = strongest_enemy {
                world.provinces.controllers[pi] = new_ctrl;
                controller_changed = true;
                // 检查 state 全翻？
                let state = world.provinces.state_of[pi];
                if !state.is_none() {
                    let si = state.0 as usize;
                    if si < world.states.count {
                        let all_flipped = world.states.provinces[si]
                            .iter()
                            .all(|&p| world.provinces.controllers[p.0 as usize] == new_ctrl);
                        if all_flipped {
                            world.states.controllers[si] = new_ctrl;
                        }
                    }
                }
            }
        }
    }

    if controller_changed {
        world.path_cache.clear();
    }
}

/// P4: next_step with World.path_cache. Caches BFS next-step results
/// so repeated calls for the same (from, dest) pair are O(1).
pub(crate) fn next_step_cached(
    world: &mut World,
    owner: CountryId,
    cur: ProvinceId,
    dest: ProvinceId,
) -> ProvinceId {
    if cur == dest {
        return dest;
    }
    let key = (owner.0, cur.0, dest.0);
    if let Some(&next_raw) = world.path_cache.get(&key) {
        return ProvinceId(next_raw);
    }
    let result = next_step(world, owner, cur, dest);
    if world.path_cache.len() > 200_000 {
        world.path_cache.clear();
    }
    world.path_cache.insert(key, result.0);
    result
}

/// Find next step toward `dest` from `cur` using BFS with limited depth.
///
/// **真正的最短路径 BFS**（仅走陆地省）。深度上限 50 跳——足够覆盖一战区
/// 范围。如 BFS 找不到路径，返回 `cur` 表示原地不动，等待 AI 重派目标，
/// 而不是走 ID-heuristic 跳进海里。
pub(crate) fn next_step(
    world: &World,
    owner: CountryId,
    cur: ProvinceId,
    dest: ProvinceId,
) -> ProvinceId {
    if cur == dest {
        return dest;
    }
    let map = &world.map;
    let raw_cur = cur.0;
    if raw_cur as usize >= map.adjacencies.len() {
        return cur;
    }

    // 直接邻接命中（dest 是陆地）：最快路径
    if map.adjacencies[raw_cur as usize]
        .iter()
        .any(|&n| n == dest.0)
        && is_land(map, dest.0)
        && can_enter_province(world, owner, dest)
    {
        return dest;
    }

    // BFS（仅陆地省）：从 cur 出发找 dest，记录 first_step。
    const MAX_DEPTH: u32 = 40;
    use std::collections::{HashMap, VecDeque};
    let mut first_step: HashMap<u16, u16> = HashMap::new();
    let mut queue: VecDeque<(u16, u32)> = VecDeque::new();
    let mut visited: std::collections::HashSet<u16> = std::collections::HashSet::new();
    visited.insert(raw_cur);
    for &n in &map.adjacencies[raw_cur as usize] {
        if !is_land(map, n) {
            continue;
        }
        if !can_enter_province(world, owner, ProvinceId(n)) {
            continue;
        }
        if !visited.insert(n) {
            continue;
        }
        first_step.insert(n, n);
        if n == dest.0 {
            return ProvinceId(n);
        }
        queue.push_back((n, 1));
    }

    while let Some((node, depth)) = queue.pop_front() {
        if depth >= MAX_DEPTH {
            continue;
        }
        if (node as usize) >= map.adjacencies.len() {
            continue;
        }
        let first = first_step[&node];
        for &nb in &map.adjacencies[node as usize] {
            if !is_land(map, nb) {
                continue;
            }
            if !can_enter_province(world, owner, ProvinceId(nb)) {
                continue;
            }
            if !visited.insert(nb) {
                continue;
            }
            first_step.insert(nb, first);
            if nb == dest.0 {
                return ProvinceId(first);
            }
            queue.push_back((nb, depth + 1));
        }
    }

    // BFS 50 跳内没找到陆路 → 原地不动（等 AI 重派 / dest 自己被占领后清空）
    cur
}

/// Land province check.
fn is_land(map: &hoi4_map::GameMap, raw_id: u16) -> bool {
    match map.get_province(raw_id) {
        Some(def) => matches!(def.province_type, hoi4_map::ProvinceType::Land),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, HashSet};
    use std::sync::Arc;

    use hoi4_data::{Color, Country, CountryTag, GameData, State};
    use hoi4_map::{
        GameMap, Heightmap, ProvinceDefinition, ProvinceMap, ProvinceType, TerrainBitmap,
        TerrainCatalog,
    };
    use hoi4_state::diplomacy::War;
    use hoi4_state::{Autonomy, AutonomyLevel};

    #[cfg(feature = "vanilla-audit")]
    fn hoi4_path() -> std::path::PathBuf {
        hoi4_paths::PathConfig::resolve(Default::default())
            .expect("set IRONHEART_HOI4_PATH or place HOI4 install at default Steam path")
            .game_path()
            .to_path_buf()
    }

    #[cfg(feature = "vanilla-audit")]
    fn build_world() -> World {
        let game_path = hoi4_path();
        let map = Arc::new(GameMap::load(&game_path).unwrap());
        let data = Arc::new(GameData::load(&game_path).unwrap());
        let mut world = World::new(map, data);
        world.populate_from_history();
        world
    }

    fn make_war(world: &mut World, a: CountryId, b: CountryId) {
        let war = War {
            id: world.diplomacy.next_war_id,
            primary_attacker: a,
            primary_defender: b,
            attackers: HashSet::from([a]),
            defenders: HashSet::from([b]),
            started_at_hour: world.elapsed_hours,
            attacker_war_score: 0.0,
            defender_war_score: 0.0,
            attacker_wargoals: vec![],
            defender_wargoals: vec![],
            war_join_policies: std::collections::HashMap::new(),
        };
        world
            .diplomacy
            .wars
            .insert(world.diplomacy.next_war_id, war);
        world.diplomacy.next_war_id += 1;
        world.countries.at_war[a.0 as usize] = true;
        world.countries.at_war[b.0 as usize] = true;
    }

    fn synthetic_world() -> World {
        let definitions = vec![
            None,
            Some(ProvinceDefinition {
                id: 1,
                r: 1,
                g: 0,
                b: 0,
                province_type: ProvinceType::Land,
                coastal: false,
                terrain: "plains".to_owned(),
                continent: 1,
            }),
            Some(ProvinceDefinition {
                id: 2,
                r: 2,
                g: 0,
                b: 0,
                province_type: ProvinceType::Land,
                coastal: false,
                terrain: "plains".to_owned(),
                continent: 1,
            }),
        ];
        let map = Arc::new(GameMap {
            definitions,
            rgb_to_id: HashMap::new(),
            province_map: ProvinceMap {
                width: 1,
                height: 1,
                pixels: vec![1],
            },
            adjacencies: vec![Vec::new(), vec![2], vec![1]],
            special_adjacencies: Vec::new(),
            heightmap: Heightmap {
                width: 1,
                height: 1,
                pixels: vec![0],
            },
            terrain_bmp: TerrainBitmap {
                width: 1,
                height: 1,
                pixels: vec![0],
                palette: [[0; 3]; 256],
            },
            terrain_catalog: TerrainCatalog::default(),
            tree_definition_bmp: None,
            tree_indices: Default::default(),
        });

        let mut data = GameData::default();
        let ger = CountryTag::new("GER");
        let aus = CountryTag::new("AUS");
        for tag in [ger.clone(), aus.clone()] {
            data.countries.insert(
                tag.clone(),
                Country {
                    tag,
                    color: Color { r: 1, g: 1, b: 1 },
                    graphical_culture: "western_european_gfx".to_owned(),
                    capital: 1,
                    ruling_party: "neutrality".to_owned(),
                    technologies: Vec::new(),
                },
            );
        }
        data.states.push(State {
            id: 1,
            name: "GER State".to_owned(),
            manpower: 1000,
            owner: ger.clone(),
            cores: vec![ger],
            provinces: vec![1],
            category: "city".to_owned(),
            infrastructure: 0,
            victory_points: Vec::new(),
            resources: Vec::new(),
        });
        data.states.push(State {
            id: 2,
            name: "AUS State".to_owned(),
            manpower: 1000,
            owner: aus.clone(),
            cores: vec![aus],
            provinces: vec![2],
            category: "city".to_owned(),
            infrastructure: 0,
            victory_points: Vec::new(),
            resources: Vec::new(),
        });

        World::new(map, Arc::new(data))
    }

    #[cfg(feature = "vanilla-audit")]
    fn unconstrained_next_step(world: &World, cur: ProvinceId, dest: ProvinceId) -> ProvinceId {
        if cur == dest {
            return dest;
        }
        let map = &world.map;
        let raw_cur = cur.0;
        if raw_cur as usize >= map.adjacencies.len() {
            return cur;
        }
        if map.adjacencies[raw_cur as usize]
            .iter()
            .any(|&n| n == dest.0)
            && is_land(map, dest.0)
        {
            return dest;
        }

        let mut first_step: HashMap<u16, u16> = HashMap::new();
        let mut queue: std::collections::VecDeque<(u16, u32)> = std::collections::VecDeque::new();
        let mut visited: HashSet<u16> = HashSet::new();
        visited.insert(raw_cur);
        for &n in &map.adjacencies[raw_cur as usize] {
            if !is_land(map, n) || !visited.insert(n) {
                continue;
            }
            first_step.insert(n, n);
            if n == dest.0 {
                return ProvinceId(n);
            }
            queue.push_back((n, 1));
        }

        while let Some((node, depth)) = queue.pop_front() {
            if depth >= 40 || (node as usize) >= map.adjacencies.len() {
                continue;
            }
            let first = first_step[&node];
            for &nb in &map.adjacencies[node as usize] {
                if !is_land(map, nb) || !visited.insert(nb) {
                    continue;
                }
                first_step.insert(nb, first);
                if nb == dest.0 {
                    return ProvinceId(first);
                }
                queue.push_back((nb, depth + 1));
            }
        }
        cur
    }

    #[cfg(feature = "vanilla-audit")]
    #[test]
    fn movement_path_avoids_neutral_shortest_route() {
        let mut world = build_world();
        let ger = world.country("GER").unwrap();
        let aus = world.country("AUS").unwrap();
        make_war(&mut world, ger, aus);

        let ger_provs: Vec<ProvinceId> = (0..world.provinces.count)
            .filter_map(|i| {
                let p = ProvinceId(i as u16);
                (world.provinces.controllers[i] == ger && is_land(&world.map, p.0)).then_some(p)
            })
            .collect();
        let aus_provs: Vec<ProvinceId> = (0..world.provinces.count)
            .filter_map(|i| {
                let p = ProvinceId(i as u16);
                (world.provinces.controllers[i] == aus && is_land(&world.map, p.0)).then_some(p)
            })
            .collect();

        let mut case = None;
        'outer: for &src in &ger_provs {
            for &dest in &aus_provs {
                let old_next = unconstrained_next_step(&world, src, dest);
                if old_next == src || can_enter_province(&world, ger, old_next) {
                    continue;
                }
                let legal_next = next_step(&world, ger, src, dest);
                if legal_next != src && can_enter_province(&world, ger, legal_next) {
                    case = Some((src, dest, legal_next));
                    break 'outer;
                }
            }
        }

        let (src, dest, expected_next) =
            case.expect("expected GER->AUS route with neutral shortest first step");
        let div = (0..world.divisions.count)
            .find(|&i| world.divisions.owners[i] == ger)
            .expect("GER should have at least one division");
        world.divisions.locations[div] = src;
        world.divisions.destinations[div] = Some(dest);
        world.rebuild_province_div_index();

        daily_movement_tick(&mut world);

        assert_eq!(world.divisions.locations[div], expected_next);
        if expected_next != dest {
            assert_eq!(world.divisions.destinations[div], Some(dest));
        }
    }

    #[test]
    fn division_move_order_api_validates_and_clears_destinations() {
        let mut world = synthetic_world();
        let ger = world.country("GER").unwrap();
        let aus = world.country("AUS").unwrap();
        let div = world
            .divisions
            .push(ger, ProvinceId(1), 0, 10.0, 1.0, "test division".to_owned())
            as usize;

        let rejected = issue_division_move_order(&mut world, div, ProvinceId(2));
        assert_eq!(
            rejected,
            Err(DivisionMoveOrderError::CannotEnterDestination)
        );
        assert_eq!(world.divisions.destinations[div], None);

        make_war(&mut world, ger, aus);
        assert_eq!(
            issue_division_move_order(&mut world, div, ProvinceId(2)),
            Ok(())
        );
        assert_eq!(world.divisions.destinations[div], Some(ProvinceId(2)));

        world.divisions.move_progress[div] = 0.5;
        assert_eq!(clear_division_move_order(&mut world, div), Ok(()));
        assert_eq!(world.divisions.destinations[div], None);
        assert_eq!(world.divisions.move_progress[div], 0.0);
    }

    #[test]
    fn next_step_toward_destination_matches_daily_movement() {
        let mut world = synthetic_world();
        let ger = world.country("GER").unwrap();
        let aus = world.country("AUS").unwrap();
        make_war(&mut world, ger, aus);
        let div = world
            .divisions
            .push(ger, ProvinceId(1), 0, 10.0, 1.0, "test division".to_owned())
            as usize;

        assert_eq!(
            next_step_toward_destination(&mut world, div, ProvinceId(2)),
            Ok(Some(ProvinceId(2)))
        );

        issue_division_move_order(&mut world, div, ProvinceId(2)).unwrap();
        daily_movement_tick(&mut world);

        assert_eq!(world.divisions.locations[div], ProvinceId(2));
    }

    #[test]
    fn master_and_subject_have_military_access() {
        let mut world = synthetic_world();
        let ger = world.country("GER").unwrap();
        let aus = world.country("AUS").unwrap();

        assert!(!can_enter_province(&world, ger, ProvinceId(2)));
        world.diplomacy.autonomy.insert(
            aus,
            Autonomy {
                master: ger,
                subject: aus,
                level: AutonomyLevel::Puppet,
                progress: 0.0,
                since_hour: 0,
            },
        );

        assert!(can_enter_province(&world, ger, ProvinceId(2)));
        assert!(can_enter_province(&world, aus, ProvinceId(1)));
    }
}
