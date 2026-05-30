//! 玩家划线战线 — 纯算法模块（frontline-orders）。
//!
//! 正确性属性摘要（design.md § 正确性属性）：
//!
//! | # | 属性 | 关键需求 |
//! |---|---|---|
//! | 1 | 分布方案均衡不变量 | R3.2, R3.3, R3.4 |
//! | 2 | 分布器确定性 | R11.1, R11.2 |
//! | 3 | 输入顺序置换汇合 | R11.3 |
//! | 4 | 战线路径合法性 | R2.3–R2.5, R7.1, R7.2, R7.5 |
//! | 5 | 存档 round-trip | R10.1–R10.4, R12.1–R12.3, R15.10 |
//! | 6 | 成员互斥 | R1.4–R1.6 |
//! | 7 | 玩家/AI 集团军 destinations 保护 | R8.1–R8.5, R15.4 |
//! | 8 | 战线塌陷终止 | R4.4, R7.3 |
//! | 9 | 吸附器幂等 | R2 总体 |
//! | 10 | 玩家与 AI 分布等价 | R15.1, R15.5 |
//! | 11 | 每国战线上限 | R15.2 |
//! | 12 | destinations 写入语义 | R3.6, R3.8, R4.5, R7.4, R14.5 |
//! | 13 | 箭头执行者选择 + 防御保留 | R6.1–R6.4 |
//! | 14 | 箭头完成 → 清 arrow 保 path | R6.5 |
//! | 15 | anchor 滚动 | R6.6 |
//! | 16 | 解散保留 destinations | R1.3 |
//! | 17 | 渲染叠加层尊重 owner 与 show_all_units | R9.6, R15.9 |

use std::collections::{HashMap, HashSet, VecDeque};

use hoi4_state::frontline::{ArmyId, FrontlineOrder, OffensiveArrow, PlayerArmy};
use hoi4_state::ids::{CountryId, ProvinceId};
use hoi4_state::World;
use hoi4_state::{CommandSource, DivisionAssignment, DivisionRole};

pub const MAX_PATH_LEN: usize = 512;
pub const MAX_ARROW_LEN: usize = 32;
pub const MAX_FRONTLINES_PER_COUNTRY: usize = 4;
pub const MAX_BRIDGE_DEPTH: u32 = 6;
pub const MAX_SAMPLES: usize = 1024;
pub const BFS_MAX_DEPTH: u32 = 40;
const FRONTLINE_PATH_ADVANCE_DEPTH: u32 = 8;
const MAX_INTERIOR_BRIDGE: u32 = 3;
const FRONTLINE_PATH_ADVANCE_CANDIDATE_CAP: usize = MAX_PATH_LEN;
const ARROW_TARGET_CAPACITY: usize = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrontlineError {
    NoSelectedDivisions,
    DivisionNotOwned,
    NoEligibleProvince,
    AnchorNotAdjacent,
    BridgeUnreachable,
    PathTruncated(usize),
    ArrowTruncated(usize),
    StaleMember(usize),
    ArmyCapReached,
    UnknownArmy,
}

impl std::fmt::Display for FrontlineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoSelectedDivisions => write!(f, "no selected divisions"),
            Self::DivisionNotOwned => write!(f, "division not owned by caller"),
            Self::NoEligibleProvince => write!(f, "no eligible province"),
            Self::AnchorNotAdjacent => write!(f, "anchor not adjacent to arrow start"),
            Self::BridgeUnreachable => write!(f, "bridge unreachable within 6 hops"),
            Self::PathTruncated(n) => write!(f, "path truncated from {} to {}", n, MAX_PATH_LEN),
            Self::ArrowTruncated(n) => write!(f, "arrow truncated from {} to {}", n, MAX_ARROW_LEN),
            Self::StaleMember(idx) => write!(f, "stale member index {}", idx),
            Self::ArmyCapReached => write!(f, "army cap reached ({})", MAX_FRONTLINES_PER_COUNTRY),
            Self::UnknownArmy => write!(f, "unknown army id"),
        }
    }
}

impl std::error::Error for FrontlineError {}

fn is_land(map: &hoi4_map::GameMap, raw_id: u16) -> bool {
    match map.get_province(raw_id) {
        Some(def) => matches!(def.province_type, hoi4_map::ProvinceType::Land),
        None => false,
    }
}

pub fn co_belligerent_set(world: &World, owner: CountryId) -> HashSet<CountryId> {
    let mut set = HashSet::new();
    set.insert(owner);
    if let Some(fid) = world.diplomacy.faction_of(owner) {
        if let Some(faction) = world.diplomacy.faction(fid) {
            for &m in &faction.members {
                if world.diplomacy.is_at_war(m) {
                    set.insert(m);
                }
            }
        }
    }
    for treaty in &world.diplomacy.treaties {
        if treaty.kind != hoi4_state::TreatyKind::MilitaryAccess || treaty.parties.len() < 2 {
            continue;
        }
        let grantor = treaty.parties[0];
        let grantee = treaty.parties[1];
        if grantee == owner {
            set.insert(grantor);
        }
        if grantor == owner {
            set.insert(grantee);
        }
    }
    for autonomy in world.diplomacy.autonomy.values() {
        if autonomy.master == owner {
            set.insert(autonomy.subject);
        }
        if autonomy.subject == owner {
            set.insert(autonomy.master);
            for peer in world.diplomacy.autonomy.values() {
                if peer.master == autonomy.master {
                    set.insert(peer.subject);
                }
            }
        }
    }
    set
}

fn is_hostile_to(world: &World, owner: CountryId, controller: CountryId) -> bool {
    if controller.is_none() {
        return false;
    }
    if controller == owner {
        return false;
    }
    world.diplomacy.at_war_with(owner, controller)
}

fn is_foreign_to(world: &World, owner: CountryId, controller: CountryId) -> bool {
    if controller.is_none() {
        return false;
    }
    let cobel = co_belligerent_set(world, owner);
    !cobel.contains(&controller)
}

fn eligible_frontline(world: &World, owner: CountryId, pid: ProvinceId) -> bool {
    eligible_frontline_against(world, owner, pid, None)
}

fn eligible_frontline_against(
    world: &World,
    owner: CountryId,
    pid: ProvinceId,
    target_country: Option<CountryId>,
) -> bool {
    let pi = pid.0 as usize;
    if pi >= world.provinces.count {
        return false;
    }
    if !is_land(&world.map, pid.0) {
        return false;
    }
    let ctrl = world.provinces.controllers[pi];
    let cobel = co_belligerent_set(world, owner);
    if !cobel.contains(&ctrl) {
        return false;
    }
    let neighs = world
        .map
        .adjacencies
        .get(pi)
        .map(|v| v.as_slice())
        .unwrap_or(&[]);
    let has_foreign_neighbor = neighs.iter().any(|&nb| {
        let nbi = nb as usize;
        if nbi >= world.provinces.count {
            return false;
        }
        if !is_land(&world.map, nb) {
            return false;
        }
        let nb_ctrl = world.provinces.controllers[nbi];
        if nb_ctrl.is_none() || cobel.contains(&nb_ctrl) {
            return false;
        }
        if world.diplomacy.is_at_war(owner) && !is_hostile_to(world, owner, nb_ctrl) {
            return false;
        }
        if let Some(target) = target_country {
            nb_ctrl == target
        } else {
            true
        }
    });
    has_foreign_neighbor
}

fn frontline_target_countries(world: &World, owner: CountryId, pid: ProvinceId) -> Vec<CountryId> {
    let pi = pid.0 as usize;
    if pi >= world.provinces.count || !is_land(&world.map, pid.0) {
        return Vec::new();
    }
    let cobel = co_belligerent_set(world, owner);
    let ctrl = world.provinces.controllers[pi];
    if !cobel.contains(&ctrl) {
        return Vec::new();
    }
    let mut targets = Vec::new();
    let neighs = world
        .map
        .adjacencies
        .get(pi)
        .map(|v| v.as_slice())
        .unwrap_or(&[]);
    for &nb in neighs {
        let nbi = nb as usize;
        if nbi >= world.provinces.count || !is_land(&world.map, nb) {
            continue;
        }
        let nb_ctrl = world.provinces.controllers[nbi];
        if !nb_ctrl.is_none()
            && !cobel.contains(&nb_ctrl)
            && (!world.diplomacy.is_at_war(owner) || is_hostile_to(world, owner, nb_ctrl))
            && !targets.contains(&nb_ctrl)
        {
            targets.push(nb_ctrl);
        }
    }
    targets
}

fn dominant_frontline_target_country(
    world: &World,
    owner: CountryId,
    samples: &[ProvinceId],
) -> Option<CountryId> {
    let mut counts: HashMap<CountryId, (usize, usize)> = HashMap::new();
    for (idx, &pid) in samples.iter().enumerate() {
        for target in frontline_target_countries(world, owner, pid) {
            let entry = counts.entry(target).or_insert((0, idx));
            entry.0 += 1;
            entry.1 = entry.1.min(idx);
        }
    }
    counts
        .into_iter()
        .max_by(
            |(a_country, (a_count, a_first)), (b_country, (b_count, b_first))| {
                a_count
                    .cmp(b_count)
                    .then_with(|| b_first.cmp(a_first))
                    .then_with(|| b_country.0.cmp(&a_country.0))
            },
        )
        .map(|(country, _)| country)
}

fn validate_path_against(
    world: &World,
    owner: CountryId,
    path: &[ProvinceId],
    target_country: Option<CountryId>,
) -> Vec<ProvinceId> {
    let validated = validate_path(world, owner, path);
    let Some(target) = target_country else {
        return validated;
    };
    validated
        .into_iter()
        .filter(|&pid| {
            let contacts = frontline_target_countries(world, owner, pid);
            contacts.is_empty() || contacts.contains(&target)
        })
        .collect()
}

fn eligible_frontline_or_bridge(
    world: &World,
    owner: CountryId,
    cobel: &HashSet<CountryId>,
    pid: ProvinceId,
) -> bool {
    if eligible_frontline(world, owner, pid) {
        return true;
    }
    let pi = pid.0 as usize;
    if pi >= world.provinces.count || !is_land(&world.map, pid.0) {
        return false;
    }
    let ctrl = world.provinces.controllers[pi];
    if !cobel.contains(&ctrl) {
        return false;
    }
    if !world.diplomacy.is_at_war(owner) {
        return true;
    }
    true
}

pub fn validate_path(world: &World, owner: CountryId, path: &[ProvinceId]) -> Vec<ProvinceId> {
    let cobel = co_belligerent_set(world, owner);
    path.iter()
        .filter(|&&pid| {
            let pi = pid.0 as usize;
            if pi >= world.provinces.count {
                return false;
            }
            if !is_land(&world.map, pid.0) {
                return false;
            }
            let ctrl = world.provinces.controllers[pi];
            if !cobel.contains(&ctrl) {
                return false;
            }
            true
        })
        .copied()
        .collect()
}

pub fn frontline_snapper(
    world: &World,
    owner: CountryId,
    samples: &[ProvinceId],
) -> Result<Vec<ProvinceId>, FrontlineError> {
    if samples.is_empty() {
        return Err(FrontlineError::NoEligibleProvince);
    }
    let cobel = co_belligerent_set(world, owner);
    let target_country = dominant_frontline_target_country(world, owner, samples);
    let mut projected: Vec<ProvinceId> = samples
        .iter()
        .filter_map(|&pid| {
            project_sample_to_frontline_against(world, owner, &cobel, pid, target_country)
        })
        .collect();
    projected = dedup_consecutive(&projected);
    if projected.is_empty() {
        return Err(FrontlineError::NoEligibleProvince);
    }
    projected =
        border_span_from_projected_samples_against(world, owner, &projected, target_country);
    if projected.is_empty() {
        return Err(FrontlineError::NoEligibleProvince);
    }
    let orig_len = projected.len();
    if orig_len > MAX_PATH_LEN {
        projected.truncate(MAX_PATH_LEN);
        let validated = validate_path_against(world, owner, &projected, target_country);
        return if validated.is_empty() {
            Err(FrontlineError::NoEligibleProvince)
        } else {
            Err(FrontlineError::PathTruncated(orig_len))
        };
    }
    let validated = validate_path_against(world, owner, &projected, target_country);
    if validated.is_empty() {
        Err(FrontlineError::NoEligibleProvince)
    } else {
        Ok(validated)
    }
}

fn border_span_from_projected_samples(
    world: &World,
    owner: CountryId,
    projected: &[ProvinceId],
) -> Vec<ProvinceId> {
    if projected.len() <= 1 {
        return projected.to_vec();
    }
    let start = projected[0];
    let end = *projected.last().unwrap();
    if start == end {
        return vec![start];
    }
    let cobel = co_belligerent_set(world, owner);
    if let Some(path) = bfs_frontline_path(world, owner, start, end, BFS_MAX_DEPTH) {
        return path;
    }
    if let Some(path) = bfs_cobelligerent_land_path(world, &cobel, start, end, BFS_MAX_DEPTH) {
        return path;
    }
    stitch_gaps(world, projected)
}

fn border_span_from_projected_samples_against(
    world: &World,
    owner: CountryId,
    projected: &[ProvinceId],
    target_country: Option<CountryId>,
) -> Vec<ProvinceId> {
    if projected.len() <= 1 {
        return projected.to_vec();
    }
    let start = projected[0];
    let end = *projected.last().unwrap();
    if start == end {
        return vec![start];
    }
    let cobel = co_belligerent_set(world, owner);
    if let Some(path) =
        bfs_frontline_path_against(world, owner, start, end, BFS_MAX_DEPTH, target_country)
    {
        return path;
    }
    if let Some(path) = bfs_cobelligerent_land_path(world, &cobel, start, end, BFS_MAX_DEPTH) {
        return path;
    }
    stitch_gaps(world, projected)
}

fn project_sample_to_frontline_against(
    world: &World,
    owner: CountryId,
    cobel: &HashSet<CountryId>,
    pid: ProvinceId,
    target_country: Option<CountryId>,
) -> Option<ProvinceId> {
    if eligible_frontline_against(world, owner, pid, target_country) {
        return Some(pid);
    }
    if !eligible_frontline_or_bridge(world, owner, cobel, pid) {
        return nearest_frontline_province_against(world, owner, cobel, pid, 8, target_country);
    }
    nearest_frontline_province_against(world, owner, cobel, pid, 8, target_country).or_else(|| {
        if world.diplomacy.is_at_war(owner) && target_country.is_some() {
            None
        } else {
            Some(pid)
        }
    })
}

fn nearest_frontline_province_against(
    world: &World,
    owner: CountryId,
    cobel: &HashSet<CountryId>,
    start: ProvinceId,
    max_depth: u32,
    target_country: Option<CountryId>,
) -> Option<ProvinceId> {
    let raw_start = start.0 as usize;
    if raw_start >= world.map.adjacencies.len()
        || raw_start >= world.provinces.count
        || !is_land(&world.map, start.0)
    {
        return None;
    }
    if !cobel.contains(&world.provinces.controllers[raw_start]) {
        return None;
    }

    let mut visited: HashSet<u16> = HashSet::new();
    let mut queue: VecDeque<(u16, u32)> = VecDeque::new();
    visited.insert(start.0);
    queue.push_back((start.0, 0));

    while let Some((node, depth)) = queue.pop_front() {
        let pid = ProvinceId(node);
        if eligible_frontline_against(world, owner, pid, target_country) {
            return Some(pid);
        }
        if depth >= max_depth || (node as usize) >= world.map.adjacencies.len() {
            continue;
        }
        for &nb in &world.map.adjacencies[node as usize] {
            let nbi = nb as usize;
            if nbi >= world.provinces.count || !is_land(&world.map, nb) {
                continue;
            }
            if !cobel.contains(&world.provinces.controllers[nbi]) {
                continue;
            }
            if visited.insert(nb) {
                queue.push_back((nb, depth + 1));
            }
        }
    }
    None
}

fn dedup_consecutive(path: &[ProvinceId]) -> Vec<ProvinceId> {
    let mut out = Vec::with_capacity(path.len());
    for &p in path {
        if out.last() != Some(&p) {
            out.push(p);
        }
    }
    out
}

fn stitch_gaps(world: &World, path: &[ProvinceId]) -> Vec<ProvinceId> {
    if path.len() <= 1 {
        return path.to_vec();
    }
    let mut result = vec![path[0]];
    for &pid in &path[1..] {
        let prev = *result.last().unwrap();
        if are_adjacent(world, prev, pid) {
            result.push(pid);
        } else if let Some(bridge) = bfs_land_path(world, prev, pid, MAX_BRIDGE_DEPTH) {
            result.extend_from_slice(&bridge[1..]);
        } else {
            result.push(pid);
        }
    }
    result
}

fn repair_frontline_path(world: &World, owner: CountryId, path: &[ProvinceId]) -> Vec<ProvinceId> {
    let cobel = co_belligerent_set(world, owner);
    let mut valid: Vec<ProvinceId> = path
        .iter()
        .filter(|&&pid| {
            let pi = pid.0 as usize;
            pi < world.provinces.count
                && is_land(&world.map, pid.0)
                && cobel.contains(&world.provinces.controllers[pi])
        })
        .copied()
        .collect();
    valid = dedup_consecutive(&valid);
    valid = stitch_cobelligerent_gaps(world, owner, &cobel, &valid, MAX_BRIDGE_DEPTH);
    if valid.len() > MAX_PATH_LEN {
        valid.truncate(MAX_PATH_LEN);
    }
    validate_path(world, owner, &valid)
}

fn stitch_cobelligerent_gaps(
    world: &World,
    owner: CountryId,
    cobel: &HashSet<CountryId>,
    path: &[ProvinceId],
    max_depth: u32,
) -> Vec<ProvinceId> {
    if path.len() <= 1 {
        return path.to_vec();
    }
    let mut result = vec![path[0]];
    for &pid in &path[1..] {
        let prev = *result.last().unwrap();
        if prev == pid {
            continue;
        }
        if are_adjacent(world, prev, pid) {
            result.push(pid);
        } else if let Some(bridge) =
            bfs_shared_enemy_frontline_path(world, owner, cobel, prev, pid, BFS_MAX_DEPTH)
        {
            result.extend_from_slice(&bridge[1..]);
        } else if let Some(bridge) = bfs_cobelligerent_land_path(world, cobel, prev, pid, max_depth)
        {
            result.extend_from_slice(&bridge[1..]);
        } else {
            result.push(pid);
        }
    }
    result
}

fn merge_frontline_contacts(
    world: &World,
    cobel: &HashSet<CountryId>,
    path: &[ProvinceId],
    contacts: &[ProvinceId],
) -> Vec<ProvinceId> {
    let mut merged = path.to_vec();
    for &contact in contacts {
        if merged.contains(&contact) {
            continue;
        }
        let best_bridge = merged
            .iter()
            .enumerate()
            .filter_map(|(idx, &anchor)| {
                bfs_cobelligerent_land_path(
                    world,
                    cobel,
                    anchor,
                    contact,
                    FRONTLINE_PATH_ADVANCE_DEPTH,
                )
                .map(|bridge| (idx, bridge))
            })
            .min_by(|(_, a), (_, b)| a.len().cmp(&b.len()));

        if let Some((idx, bridge)) = best_bridge {
            let mut insert_at = idx + 1;
            for pid in bridge.into_iter().skip(1) {
                if merged.contains(&pid) {
                    continue;
                }
                merged.insert(insert_at, pid);
                insert_at += 1;
            }
        } else {
            merged.push(contact);
        }
    }
    dedup_consecutive(&merged)
}

fn hostile_neighbor_set(world: &World, owner: CountryId, pid: ProvinceId) -> HashSet<CountryId> {
    let mut enemies = HashSet::new();
    let pi = pid.0 as usize;
    if pi >= world.map.adjacencies.len() {
        return enemies;
    }
    for &nb in &world.map.adjacencies[pi] {
        let nbi = nb as usize;
        if nbi >= world.provinces.count || !is_land(&world.map, nb) {
            continue;
        }
        let ctrl = world.provinces.controllers[nbi];
        if is_hostile_to(world, owner, ctrl) {
            enemies.insert(ctrl);
        }
    }
    enemies
}

fn shared_hostile_neighbors(
    world: &World,
    owner: CountryId,
    a: ProvinceId,
    b: ProvinceId,
) -> HashSet<CountryId> {
    let a_enemies = hostile_neighbor_set(world, owner, a);
    if a_enemies.is_empty() {
        return HashSet::new();
    }
    hostile_neighbor_set(world, owner, b)
        .into_iter()
        .filter(|enemy| a_enemies.contains(enemy))
        .collect()
}

fn borders_any_enemy_in_set(
    world: &World,
    owner: CountryId,
    pid: ProvinceId,
    enemies: &HashSet<CountryId>,
) -> bool {
    if enemies.is_empty() {
        return false;
    }
    hostile_neighbor_set(world, owner, pid)
        .into_iter()
        .any(|enemy| enemies.contains(&enemy))
}

fn bfs_shared_enemy_frontline_path(
    world: &World,
    owner: CountryId,
    cobel: &HashSet<CountryId>,
    from: ProvinceId,
    to: ProvinceId,
    max_depth: u32,
) -> Option<Vec<ProvinceId>> {
    if from == to {
        return Some(vec![from]);
    }
    let shared_enemies = shared_hostile_neighbors(world, owner, from, to);
    if shared_enemies.is_empty() && !world.diplomacy.is_at_war(owner) {
        return None;
    }
    let shared_enemies = shared_hostile_neighbors(world, owner, from, to);
    if shared_enemies.is_empty() {
        return None;
    }
    let raw_from = from.0;
    let raw_to = to.0;
    if (raw_from as usize) >= world.map.adjacencies.len()
        || (raw_to as usize) >= world.map.adjacencies.len()
    {
        return None;
    }

    let mut parent: HashMap<u16, u16> = HashMap::new();
    let mut visited: HashSet<u16> = HashSet::new();
    let mut queue: VecDeque<(u16, u32)> = VecDeque::new();
    visited.insert(raw_from);
    queue.push_back((raw_from, 0));

    while let Some((node, depth)) = queue.pop_front() {
        if depth >= max_depth || (node as usize) >= world.map.adjacencies.len() {
            continue;
        }
        for &nb in &world.map.adjacencies[node as usize] {
            let nbi = nb as usize;
            if nbi >= world.provinces.count || !is_land(&world.map, nb) {
                continue;
            }
            if !cobel.contains(&world.provinces.controllers[nbi]) {
                continue;
            }
            let nb_id = ProvinceId(nb);
            if !borders_any_enemy_in_set(world, owner, nb_id, &shared_enemies) {
                continue;
            }
            if !visited.insert(nb) {
                continue;
            }
            parent.insert(nb, node);
            if nb == raw_to {
                return Some(reconstruct_path(&parent, raw_from, raw_to));
            }
            queue.push_back((nb, depth + 1));
        }
    }
    None
}

fn bfs_frontline_path_against(
    world: &World,
    owner: CountryId,
    from: ProvinceId,
    to: ProvinceId,
    max_depth: u32,
    target_country: Option<CountryId>,
) -> Option<Vec<ProvinceId>> {
    if from == to {
        return Some(vec![from]);
    }
    let cobel = co_belligerent_set(world, owner);
    if !cobel.contains(&world.provinces.controllers[from.0 as usize])
        || !cobel.contains(&world.provinces.controllers[to.0 as usize])
    {
        return None;
    }
    let raw_from = from.0;
    let raw_to = to.0;
    if (raw_from as usize) >= world.map.adjacencies.len()
        || (raw_to as usize) >= world.map.adjacencies.len()
    {
        return None;
    }

    let mut parent: HashMap<u16, u16> = HashMap::new();
    let mut visited: HashSet<u16> = HashSet::new();
    let mut interior_depth: HashMap<u16, u32> = HashMap::new();
    let mut queue: VecDeque<(u16, u32)> = VecDeque::new();
    visited.insert(raw_from);
    let from_on_front = eligible_frontline_against(world, owner, from, target_country);
    interior_depth.insert(raw_from, if from_on_front { 0 } else { 1 });
    queue.push_back((raw_from, 0));

    while let Some((node, depth)) = queue.pop_front() {
        if depth >= max_depth || (node as usize) >= world.map.adjacencies.len() {
            continue;
        }
        let cur_interior = *interior_depth.get(&node).unwrap_or(&0);
        for &nb in &world.map.adjacencies[node as usize] {
            let nbi = nb as usize;
            if nbi >= world.provinces.count || !is_land(&world.map, nb) {
                continue;
            }
            let nb_ctrl = world.provinces.controllers[nbi];
            if !cobel.contains(&nb_ctrl) {
                continue;
            }
            let nb_on_front =
                eligible_frontline_against(world, owner, ProvinceId(nb), target_country);
            let next_interior = if nb_on_front { 0 } else { cur_interior + 1 };
            if next_interior > MAX_INTERIOR_BRIDGE {
                continue;
            }
            if !visited.insert(nb) {
                continue;
            }
            interior_depth.insert(nb, next_interior);
            parent.insert(nb, node);
            if nb == raw_to {
                return Some(reconstruct_path(&parent, raw_from, raw_to));
            }
            queue.push_back((nb, depth + 1));
        }
    }
    None
}

fn bfs_cobelligerent_land_path(
    world: &World,
    cobel: &HashSet<CountryId>,
    from: ProvinceId,
    to: ProvinceId,
    max_depth: u32,
) -> Option<Vec<ProvinceId>> {
    if from == to {
        return Some(vec![from]);
    }
    let raw_from = from.0;
    let raw_to = to.0;
    if (raw_from as usize) >= world.map.adjacencies.len()
        || (raw_to as usize) >= world.map.adjacencies.len()
    {
        return None;
    }
    let from_i = raw_from as usize;
    let to_i = raw_to as usize;
    if from_i >= world.provinces.count
        || to_i >= world.provinces.count
        || !is_land(&world.map, raw_from)
        || !is_land(&world.map, raw_to)
        || !cobel.contains(&world.provinces.controllers[from_i])
        || !cobel.contains(&world.provinces.controllers[to_i])
    {
        return None;
    }

    let mut parent: HashMap<u16, u16> = HashMap::new();
    let mut visited: HashSet<u16> = HashSet::new();
    let mut queue: VecDeque<(u16, u32)> = VecDeque::new();
    visited.insert(raw_from);
    queue.push_back((raw_from, 0));

    while let Some((node, depth)) = queue.pop_front() {
        if depth >= max_depth || (node as usize) >= world.map.adjacencies.len() {
            continue;
        }
        for &nb in &world.map.adjacencies[node as usize] {
            let nbi = nb as usize;
            if nbi >= world.provinces.count || !is_land(&world.map, nb) {
                continue;
            }
            if !cobel.contains(&world.provinces.controllers[nbi]) {
                continue;
            }
            if !visited.insert(nb) {
                continue;
            }
            parent.insert(nb, node);
            if nb == raw_to {
                return Some(reconstruct_path(&parent, raw_from, raw_to));
            }
            queue.push_back((nb, depth + 1));
        }
    }
    None
}

fn bfs_frontline_path(
    world: &World,
    owner: CountryId,
    from: ProvinceId,
    to: ProvinceId,
    max_depth: u32,
) -> Option<Vec<ProvinceId>> {
    if from == to {
        return Some(vec![from]);
    }
    let cobel = co_belligerent_set(world, owner);
    if !cobel.contains(&world.provinces.controllers[from.0 as usize])
        || !cobel.contains(&world.provinces.controllers[to.0 as usize])
    {
        return None;
    }
    let raw_from = from.0;
    let raw_to = to.0;
    if (raw_from as usize) >= world.map.adjacencies.len()
        || (raw_to as usize) >= world.map.adjacencies.len()
    {
        return None;
    }

    let mut parent: HashMap<u16, u16> = HashMap::new();
    let mut visited: HashSet<u16> = HashSet::new();
    let mut interior_depth: HashMap<u16, u32> = HashMap::new();
    let mut queue: VecDeque<(u16, u32)> = VecDeque::new();
    visited.insert(raw_from);
    let from_on_front = eligible_frontline(world, owner, from);
    interior_depth.insert(raw_from, if from_on_front { 0 } else { 1 });
    queue.push_back((raw_from, 0));

    while let Some((node, depth)) = queue.pop_front() {
        if depth >= max_depth || (node as usize) >= world.map.adjacencies.len() {
            continue;
        }
        let cur_interior = *interior_depth.get(&node).unwrap_or(&0);
        for &nb in &world.map.adjacencies[node as usize] {
            let nbi = nb as usize;
            if nbi >= world.provinces.count || !is_land(&world.map, nb) {
                continue;
            }
            let nb_ctrl = world.provinces.controllers[nbi];
            if !cobel.contains(&nb_ctrl) {
                continue;
            }
            let nb_on_front = eligible_frontline(world, owner, ProvinceId(nb));
            let next_interior = if nb_on_front { 0 } else { cur_interior + 1 };
            if next_interior > MAX_INTERIOR_BRIDGE {
                continue;
            }
            if !visited.insert(nb) {
                continue;
            }
            interior_depth.insert(nb, next_interior);
            parent.insert(nb, node);
            if nb == raw_to {
                return Some(reconstruct_path(&parent, raw_from, raw_to));
            }
            queue.push_back((nb, depth + 1));
        }
    }
    None
}

fn are_adjacent(world: &World, a: ProvinceId, b: ProvinceId) -> bool {
    world
        .map
        .adjacencies
        .get(a.0 as usize)
        .map(|v| v.contains(&b.0))
        .unwrap_or(false)
}

pub fn arrow_snapper(
    world: &World,
    owner: CountryId,
    anchor: ProvinceId,
    samples: &[ProvinceId],
) -> Result<Vec<ProvinceId>, FrontlineError> {
    if samples.is_empty() {
        return Err(FrontlineError::NoEligibleProvince);
    }
    let mut projected: Vec<ProvinceId> = samples
        .iter()
        .filter(|&&pid| {
            let pi = pid.0 as usize;
            if pi >= world.provinces.count {
                return false;
            }
            if !is_land(&world.map, pid.0) {
                return false;
            }
            let ctrl = world.provinces.controllers[pi];
            is_hostile_to(world, owner, ctrl)
        })
        .copied()
        .collect();
    projected = dedup_consecutive(&projected);
    if projected.is_empty() {
        return Err(FrontlineError::NoEligibleProvince);
    }
    let first = projected[0];
    if !are_adjacent(world, anchor, first) {
        let Some(bridge) = bfs_land_path(world, anchor, first, BFS_MAX_DEPTH) else {
            return Err(FrontlineError::AnchorNotAdjacent);
        };
        let mut bridged: Vec<ProvinceId> = bridge
            .into_iter()
            .skip(1)
            .filter(|&pid| {
                let pi = pid.0 as usize;
                pi < world.provinces.count
                    && is_land(&world.map, pid.0)
                    && is_hostile_to(world, owner, world.provinces.controllers[pi])
            })
            .collect();
        if bridged.is_empty() {
            return Err(FrontlineError::AnchorNotAdjacent);
        }
        bridged.extend_from_slice(&projected[1..]);
        projected = dedup_consecutive(&bridged);
    }
    projected = stitch_gaps_limited(world, &projected, BFS_MAX_DEPTH);
    if projected.is_empty() {
        return Err(FrontlineError::BridgeUnreachable);
    }
    let orig_len = projected.len();
    if orig_len > MAX_ARROW_LEN {
        projected.truncate(MAX_ARROW_LEN);
        return Err(FrontlineError::ArrowTruncated(orig_len));
    }
    Ok(projected)
}

fn stitch_gaps_limited(world: &World, path: &[ProvinceId], max_depth: u32) -> Vec<ProvinceId> {
    if path.len() <= 1 {
        return path.to_vec();
    }
    let mut result = vec![path[0]];
    for &pid in &path[1..] {
        let prev = *result.last().unwrap();
        if are_adjacent(world, prev, pid) {
            result.push(pid);
        } else if let Some(bridge) = bfs_land_path(world, prev, pid, max_depth) {
            result.extend_from_slice(&bridge[1..]);
        } else {
            return Vec::new();
        }
    }
    result
}

pub(crate) fn bfs_land_path(
    world: &World,
    from: ProvinceId,
    to: ProvinceId,
    max_depth: u32,
) -> Option<Vec<ProvinceId>> {
    if from == to {
        return Some(vec![from]);
    }
    let raw_from = from.0;
    let raw_to = to.0;
    if (raw_from as usize) >= world.map.adjacencies.len()
        || (raw_to as usize) >= world.map.adjacencies.len()
    {
        return None;
    }

    let mut parent: HashMap<u16, u16> = HashMap::new();
    let mut visited: HashSet<u16> = HashSet::new();
    let mut queue: VecDeque<(u16, u32)> = VecDeque::new();
    visited.insert(raw_from);

    for &nb in &world.map.adjacencies[raw_from as usize] {
        if !is_land(&world.map, nb) {
            continue;
        }
        if !visited.insert(nb) {
            continue;
        }
        parent.insert(nb, raw_from);
        if nb == raw_to {
            return Some(reconstruct_path(&parent, raw_from, raw_to));
        }
        queue.push_back((nb, 1));
    }

    while let Some((node, depth)) = queue.pop_front() {
        if depth >= max_depth {
            continue;
        }
        if (node as usize) >= world.map.adjacencies.len() {
            continue;
        }
        for &nb in &world.map.adjacencies[node as usize] {
            if !is_land(&world.map, nb) {
                continue;
            }
            if !visited.insert(nb) {
                continue;
            }
            parent.insert(nb, node);
            if nb == raw_to {
                return Some(reconstruct_path(&parent, raw_from, raw_to));
            }
            queue.push_back((nb, depth + 1));
        }
    }
    None
}

fn reconstruct_path(parent: &HashMap<u16, u16>, from: u16, to: u16) -> Vec<ProvinceId> {
    let mut path = Vec::new();
    let mut cur = to;
    path.push(ProvinceId(cur));
    while cur != from {
        cur = *parent.get(&cur).unwrap_or(&from);
        path.push(ProvinceId(cur));
    }
    path.reverse();
    path
}

pub(crate) fn bfs_land_dist(
    world: &World,
    from: ProvinceId,
    to: ProvinceId,
    max_depth: u32,
) -> Option<u32> {
    if from == to {
        return Some(0);
    }
    let raw_from = from.0;
    let raw_to = to.0;
    if (raw_from as usize) >= world.map.adjacencies.len()
        || (raw_to as usize) >= world.map.adjacencies.len()
    {
        return None;
    }

    let mut visited: HashSet<u16> = HashSet::new();
    let mut queue: VecDeque<(u16, u32)> = VecDeque::new();
    visited.insert(raw_from);

    for &nb in &world.map.adjacencies[raw_from as usize] {
        if !is_land(&world.map, nb) {
            continue;
        }
        if !visited.insert(nb) {
            continue;
        }
        if nb == raw_to {
            return Some(1);
        }
        queue.push_back((nb, 1));
    }

    while let Some((node, depth)) = queue.pop_front() {
        if depth >= max_depth {
            continue;
        }
        if (node as usize) >= world.map.adjacencies.len() {
            continue;
        }
        for &nb in &world.map.adjacencies[node as usize] {
            if !is_land(&world.map, nb) {
                continue;
            }
            if !visited.insert(nb) {
                continue;
            }
            if nb == raw_to {
                return Some(depth + 1);
            }
            queue.push_back((nb, depth + 1));
        }
    }
    None
}

fn is_div_alive(world: &World, idx: usize) -> bool {
    if idx >= world.divisions.count {
        return false;
    }
    let max_org = world.divisions.max_organisation[idx].max(1e-6);
    let org_ratio = world.divisions.organisation[idx] / max_org;
    let str_now = world.divisions.strength[idx];
    let broken = org_ratio < 0.05 || str_now < 0.05;
    !broken
}

pub fn frontline_distributor(world: &World, army: &PlayerArmy) -> Vec<(usize, ProvinceId)> {
    let path = match &army.order {
        Some(o) if o.active && !o.path.is_empty() => &o.path,
        _ => return Vec::new(),
    };
    let owner = army.owner;
    let cobel = co_belligerent_set(world, owner);

    let mut visited: HashSet<ProvinceId> = path.iter().copied().collect();
    let mut all_frontline: Vec<ProvinceId> = path.clone();
    let mut queue: VecDeque<ProvinceId> = path.iter().copied().collect();
    while let Some(p) = queue.pop_front() {
        let pi = p.0 as usize;
        if pi >= world.map.adjacencies.len() {
            continue;
        }
        for &nb in &world.map.adjacencies[pi] {
            let nb_id = ProvinceId(nb);
            let nbi = nb as usize;
            if nbi >= world.provinces.count || !is_land(&world.map, nb) {
                continue;
            }
            if visited.contains(&nb_id) {
                continue;
            }
            let nb_ctrl = world.provinces.controllers[nbi];
            if !cobel.contains(&nb_ctrl) {
                continue;
            }
            let nb_neighs = world
                .map
                .adjacencies
                .get(nbi)
                .map(|v| v.as_slice())
                .unwrap_or(&[]);
            let has_foreign = nb_neighs.iter().any(|&nn| {
                let nni = nn as usize;
                if nni >= world.provinces.count || !is_land(&world.map, nn) {
                    return false;
                }
                is_foreign_to(world, owner, world.provinces.controllers[nni])
            });
            if has_foreign {
                visited.insert(nb_id);
                all_frontline.push(nb_id);
                queue.push_back(nb_id);
            }
        }
    }

    let m = all_frontline.len();
    let n_alive: Vec<usize> = army
        .members
        .iter()
        .filter(|&&i| is_div_alive(world, i))
        .copied()
        .collect();
    let n = n_alive.len();
    if n == 0 || m == 0 {
        return Vec::new();
    }

    // Keep the line covered first. Offensive concentration is handled by the
    // AI assault assignment/arrow executor path; using the frontline
    // distributor for concentration leaves holes that the enemy can walk through.
    let mut enemy_density: Vec<u32> = vec![0; m];
    for (fi, &fp) in all_frontline.iter().enumerate() {
        let fpi = fp.0 as usize;
        let neighs = world
            .map
            .adjacencies
            .get(fpi)
            .map(|v| v.as_slice())
            .unwrap_or(&[]);
        for &nb in neighs {
            let nbi = nb as usize;
            if nbi >= world.provinces.count || !is_land(&world.map, nb) {
                continue;
            }
            let nb_ctrl = world.provinces.controllers[nbi];
            if is_hostile_to(world, owner, nb_ctrl) {
                for &di in world.divisions_in_province(ProvinceId(nb)) {
                    if world.divisions.owners[di] == nb_ctrl {
                        enemy_density[fi] += 1;
                    }
                }
            }
        }
    }

    // Sort frontline provinces by enemy pressure (descending) so limited forces
    // cover threatened tiles first. Keep original index order as tiebreaker for
    // determinism.
    let mut ranked: Vec<(u32, usize)> = (0..m).map(|i| (enemy_density[i], i)).collect();
    ranked.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));

    let target_idxs: Vec<usize>;
    if n < m {
        // Not enough divisions for 1:1 coverage. Spreading one defender per
        // top-n tile loses to AI 3-4-division assaults (combat resolver pairs
        // up to min(attackers, defenders) per battle). Concentrate on tiles
        // with current enemy pressure and bare the quiet BFS-expanded ones.
        let hot: Vec<usize> = ranked
            .iter()
            .filter(|&&(density, _)| density > 0)
            .map(|&(_, idx)| idx)
            .collect();
        if hot.is_empty() {
            target_idxs = ranked.iter().take(n).map(|&(_, i)| i).collect();
        } else if n <= hot.len() {
            target_idxs = hot.into_iter().take(n).collect();
        } else {
            let k = hot.len();
            let base = n / k;
            let extra = n % k;
            target_idxs = hot
                .iter()
                .enumerate()
                .flat_map(|(j, &slot)| {
                    let cap = if j < extra { base + 1 } else { base };
                    std::iter::repeat(slot).take(cap)
                })
                .collect();
        }
    } else {
        // n >= m: enough divisions to cover the whole frontline.
        let base = n / m;
        let extra = n % m;
        target_idxs = ranked
            .iter()
            .enumerate()
            .flat_map(|j| {
                let cap = if j.0 < extra { base + 1 } else { base };
                std::iter::repeat(j.1 .1).take(cap)
            })
            .collect();
    }

    let mut pairs: Vec<(u32, usize, usize)> = Vec::with_capacity(n * m.min(8));
    for &di in &n_alive {
        let loc = world.divisions.locations[di];
        for (slot, &path_idx) in target_idxs.iter().enumerate() {
            let dist = bfs_land_dist(world, loc, all_frontline[path_idx], BFS_MAX_DEPTH)
                .unwrap_or(u32::MAX);
            pairs.push((dist, di, slot));
        }
    }
    pairs.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));

    let mut used_divs: HashSet<usize> = HashSet::with_capacity(n);
    let mut used_slots: HashSet<usize> = HashSet::with_capacity(n);
    let mut result: Vec<(usize, ProvinceId)> = Vec::with_capacity(n);

    for (_, di, slot) in pairs {
        if used_divs.contains(&di) || used_slots.contains(&slot) {
            continue;
        }
        let path_idx = target_idxs[slot];
        used_divs.insert(di);
        used_slots.insert(slot);
        result.push((di, all_frontline[path_idx]));
    }

    result
}

pub fn pick_arrow_executors(world: &World, army: &PlayerArmy) -> Vec<usize> {
    let Some(order) = army.order.as_ref().filter(|o| o.active) else {
        return Vec::new();
    };

    let alive: Vec<usize> = army
        .members
        .iter()
        .filter(|&&i| is_div_alive(world, i))
        .copied()
        .collect();
    if alive.len() <= 1 {
        return Vec::new();
    }

    // Keep enough divisions on the frontline before sending surplus into the
    // arrow. This prevents AI armies from abandoning every line province when an
    // offensive plan is active.
    let plan_efficiency = crate::military::general::modifier_for_division(world, alive[0])
        .map(|modifier| modifier.plan_efficiency_mult)
        .unwrap_or(1.0);
    let line_need = order.path.len().max(1).min((alive.len() + 1) / 2);
    let executor_count = alive.len().saturating_sub(line_need);
    if executor_count == 0 {
        return Vec::new();
    }
    let planned_count = ((executor_count as f32) * plan_efficiency).ceil() as usize;
    alive
        .into_iter()
        .take(planned_count.min(executor_count))
        .collect()
}

fn collect_arrow_attack_targets(
    world: &World,
    owner: CountryId,
    path: &[ProvinceId],
    arrow: &OffensiveArrow,
) -> Vec<ProvinceId> {
    let path_set: HashSet<ProvinceId> = path.iter().copied().collect();
    let mut targets = Vec::new();

    for &target in &arrow.provinces {
        let ti = target.0 as usize;
        if ti >= world.provinces.count || !is_land(&world.map, target.0) {
            continue;
        }
        let ctrl = world.provinces.controllers[ti];
        if !is_hostile_to(world, owner, ctrl) {
            continue;
        }
        if !crate::military::movement::can_enter_province(world, owner, target) {
            continue;
        }
        let adjacent_to_front = world
            .map
            .adjacencies
            .get(ti)
            .map(|neighs| neighs.iter().any(|&nb| path_set.contains(&ProvinceId(nb))))
            .unwrap_or(false);
        if adjacent_to_front && !targets.contains(&target) {
            targets.push(target);
        }
    }

    // Fallback: any path province not adjacent to an arrow-derived target
    // contributes its own hostile-adjacent province. Ensures every disjoint
    // frontline segment has at least one local assault target when the user's
    // arrow stroke only covered part of the line (e.g. the path was extended
    // by repair / merge after the arrow was drawn).
    let mut covered_path: HashSet<ProvinceId> = HashSet::new();
    for &t in &targets {
        let ti = t.0 as usize;
        if let Some(neighs) = world.map.adjacencies.get(ti) {
            for &nb in neighs {
                let nb_id = ProvinceId(nb);
                if path_set.contains(&nb_id) {
                    covered_path.insert(nb_id);
                }
            }
        }
    }
    for &p in path {
        if covered_path.contains(&p) {
            continue;
        }
        let pi = p.0 as usize;
        if pi >= world.map.adjacencies.len() {
            continue;
        }
        for &nb in &world.map.adjacencies[pi] {
            let nbi = nb as usize;
            if nbi >= world.provinces.count || !is_land(&world.map, nb) {
                continue;
            }
            let nb_id = ProvinceId(nb);
            if path_set.contains(&nb_id) {
                continue;
            }
            let ctrl = world.provinces.controllers[nbi];
            if !is_hostile_to(world, owner, ctrl) {
                continue;
            }
            if !crate::military::movement::can_enter_province(world, owner, nb_id) {
                continue;
            }
            if !targets.contains(&nb_id) {
                targets.push(nb_id);
            }
            covered_path.insert(p);
            break;
        }
    }

    targets
}

fn assign_arrow_executor_targets(
    world: &World,
    owner: CountryId,
    executors: &[usize],
    path: &[ProvinceId],
    arrow: &OffensiveArrow,
) -> HashMap<usize, ProvinceId> {
    let targets = collect_arrow_attack_targets(world, owner, path, arrow);
    if targets.is_empty() {
        return HashMap::new();
    }

    let mut assigned_load: HashMap<ProvinceId, usize> = targets.iter().map(|&t| (t, 0)).collect();
    let mut current_stack: HashMap<ProvinceId, usize> = targets.iter().map(|&t| (t, 0)).collect();
    for &target in &targets {
        let stack = world
            .divisions_in_province(target)
            .iter()
            .filter(|&&di| di < world.divisions.count && world.divisions.owners[di] == owner)
            .count();
        current_stack.insert(target, stack);
    }

    let mut result = HashMap::new();
    let mut ordered_executors: Vec<usize> = executors
        .iter()
        .copied()
        .filter(|&di| di < world.divisions.count)
        .collect();
    ordered_executors.sort_unstable();

    for di in ordered_executors {
        let loc = world.divisions.locations[di];
        let mut best: Option<(u32, usize, ProvinceId)> = None;
        for &target in &targets {
            let load = current_stack.get(&target).copied().unwrap_or(0)
                + assigned_load.get(&target).copied().unwrap_or(0);
            if load >= ARROW_TARGET_CAPACITY {
                continue;
            }
            let distance = bfs_land_dist(world, loc, target, BFS_MAX_DEPTH).unwrap_or(u32::MAX);
            match best {
                Some((best_distance, best_load, best_target))
                    if (distance, load, target.0) >= (best_distance, best_load, best_target.0) => {}
                _ => best = Some((distance, load, target)),
            }
        }
        if let Some((_, _, target)) = best {
            *assigned_load.entry(target).or_default() += 1;
            result.insert(di, target);
        }
    }

    result
}

fn rebuild_locked_divisions(world: &mut World) {
    world.player_locked_divisions.clear();
    for army in &world.player_armies {
        if army.order.as_ref().is_some_and(|o| o.active) {
            for &m in &army.members {
                world.player_locked_divisions.insert(m);
            }
        }
    }
}

fn has_player_manual_command(world: &World, div_idx: usize) -> bool {
    matches!(
        world
            .divisions
            .commands
            .get(div_idx)
            .and_then(|cmd| cmd.as_ref())
            .map(|cmd| cmd.source),
        Some(CommandSource::PlayerManual)
    )
}

fn find_nearest_friendly(world: &World, owner: CountryId, from: ProvinceId) -> Option<ProvinceId> {
    let raw = from.0;
    if (raw as usize) >= world.map.adjacencies.len() {
        return None;
    }
    for &nb in &world.map.adjacencies[raw as usize] {
        let nbi = nb as usize;
        if nbi >= world.provinces.count || !is_land(&world.map, nb) {
            continue;
        }
        if world.provinces.controllers[nbi] == owner {
            return Some(ProvinceId(nb));
        }
    }
    let mut visited: HashSet<u16> = HashSet::new();
    let mut queue: VecDeque<(u16, u32)> = VecDeque::new();
    visited.insert(raw);
    queue.push_back((raw, 0));
    const MAX_DEPTH: u32 = 20;
    while let Some((node, depth)) = queue.pop_front() {
        if depth >= MAX_DEPTH {
            continue;
        }
        if (node as usize) >= world.map.adjacencies.len() {
            continue;
        }
        for &nb in &world.map.adjacencies[node as usize] {
            if !is_land(&world.map, nb) {
                continue;
            }
            if !visited.insert(nb) {
                continue;
            }
            let nbi = nb as usize;
            if nbi >= world.provinces.count {
                continue;
            }
            if world.provinces.controllers[nbi] == owner {
                return Some(ProvinceId(nb));
            }
            queue.push_back((nb, depth + 1));
        }
    }
    None
}

fn find_army_mut(world: &mut World, id: ArmyId) -> Option<usize> {
    world.player_armies.iter().position(|a| a.id == id)
}

fn active_army_count(world: &World, owner: CountryId) -> usize {
    world
        .player_armies
        .iter()
        .filter(|a| a.owner == owner && a.order.as_ref().is_some_and(|o| o.active))
        .count()
}

fn remove_members_from_other_armies(world: &mut World, members: &[usize], exclude_id: ArmyId) {
    let to_remove: HashSet<usize> = members.iter().copied().collect();
    for army in &mut world.player_armies {
        if army.id == exclude_id {
            continue;
        }
        army.members.retain(|m| !to_remove.contains(m));
    }
}

pub fn create_army(
    world: &mut World,
    owner: CountryId,
    members: Vec<usize>,
    name: String,
) -> Result<ArmyId, FrontlineError> {
    if members.is_empty() {
        return Err(FrontlineError::NoSelectedDivisions);
    }
    for &i in &members {
        if i >= world.divisions.count || world.divisions.owners[i] != owner {
            return Err(FrontlineError::DivisionNotOwned);
        }
    }
    if active_army_count(world, owner) >= MAX_FRONTLINES_PER_COUNTRY {
        return Err(FrontlineError::ArmyCapReached);
    }
    remove_members_from_other_armies(world, &members, ArmyId::NONE);
    let id = ArmyId(world.next_army_id);
    world.next_army_id += 1;
    world.player_armies.push(PlayerArmy {
        id,
        name,
        owner,
        commander: None,
        members,
        order: None,
    });
    rebuild_locked_divisions(world);
    Ok(id)
}

pub fn dissolve_army(world: &mut World, id: ArmyId) -> Result<(), FrontlineError> {
    let idx = find_army_mut(world, id).ok_or(FrontlineError::UnknownArmy)?;
    let army_members = world.player_armies[idx].members.clone();
    world.player_armies.remove(idx);
    for m in army_members {
        world.player_locked_divisions.remove(&m);
    }
    Ok(())
}

pub fn add_members(world: &mut World, id: ArmyId, members: &[usize]) -> Result<(), FrontlineError> {
    let idx = find_army_mut(world, id).ok_or(FrontlineError::UnknownArmy)?;
    let owner = world.player_armies[idx].owner;
    for &i in members {
        if i >= world.divisions.count || world.divisions.owners[i] != owner {
            return Err(FrontlineError::DivisionNotOwned);
        }
    }
    remove_members_from_other_armies(world, members, id);
    let army = &mut world.player_armies[idx];
    for &i in members {
        if !army.members.contains(&i) {
            army.members.push(i);
        }
    }
    rebuild_locked_divisions(world);
    Ok(())
}

pub fn remove_members(
    world: &mut World,
    id: ArmyId,
    members: &[usize],
) -> Result<(), FrontlineError> {
    let idx = find_army_mut(world, id).ok_or(FrontlineError::UnknownArmy)?;
    let to_remove: HashSet<usize> = members.iter().copied().collect();
    let army = &mut world.player_armies[idx];
    army.members.retain(|m| !to_remove.contains(m));
    rebuild_locked_divisions(world);
    Ok(())
}

pub fn set_frontline_path(
    world: &mut World,
    id: ArmyId,
    samples: &[ProvinceId],
) -> Result<(), FrontlineError> {
    let idx = find_army_mut(world, id).ok_or(FrontlineError::UnknownArmy)?;
    let owner = world.player_armies[idx].owner;
    match frontline_snapper(world, owner, samples) {
        Ok(path) => {
            let army = &mut world.player_armies[idx];
            army.order = Some(FrontlineOrder {
                path,
                arrow: None,
                anchor: None,
                active: true,
                executing: false,
            });
            rebuild_locked_divisions(world);
            Ok(())
        }
        Err(FrontlineError::PathTruncated(orig_len)) => {
            let path = frontline_snapper_internal_truncated(world, owner, samples);
            tracing::warn!(
                target: "frontline_api",
                army_id = id.0,
                orig_len = orig_len,
                "路径已截断"
            );
            let army = &mut world.player_armies[idx];
            army.order = Some(FrontlineOrder {
                path,
                arrow: None,
                anchor: None,
                active: true,
                executing: false,
            });
            rebuild_locked_divisions(world);
            Ok(())
        }
        Err(e) => Err(e),
    }
}

fn frontline_snapper_internal_truncated(
    world: &World,
    owner: CountryId,
    samples: &[ProvinceId],
) -> Vec<ProvinceId> {
    let cobel = co_belligerent_set(world, owner);
    let target_country = dominant_frontline_target_country(world, owner, samples);
    let mut projected: Vec<ProvinceId> = samples
        .iter()
        .filter_map(|&pid| {
            project_sample_to_frontline_against(world, owner, &cobel, pid, target_country)
        })
        .collect();
    projected = dedup_consecutive(&projected);
    projected =
        border_span_from_projected_samples_against(world, owner, &projected, target_country);
    if projected.len() > MAX_PATH_LEN {
        projected.truncate(MAX_PATH_LEN);
    }
    let validated = validate_path_against(world, owner, &projected, target_country);
    if validated.is_empty() {
        projected
    } else {
        validated
    }
}

pub fn clear_frontline_path(world: &mut World, id: ArmyId) -> Result<(), FrontlineError> {
    let idx = find_army_mut(world, id).ok_or(FrontlineError::UnknownArmy)?;
    let army = &mut world.player_armies[idx];
    army.order = None;
    rebuild_locked_divisions(world);
    Ok(())
}

pub fn set_arrow(
    world: &mut World,
    id: ArmyId,
    anchor: ProvinceId,
    samples: &[ProvinceId],
) -> Result<(), FrontlineError> {
    let idx = find_army_mut(world, id).ok_or(FrontlineError::UnknownArmy)?;
    {
        let army = &world.player_armies[idx];
        if army.order.is_none() || !army.order.as_ref().unwrap().active {
            return Err(FrontlineError::NoEligibleProvince);
        }
    }
    let owner = world.player_armies[idx].owner;
    let path = world.player_armies[idx]
        .order
        .as_ref()
        .unwrap()
        .path
        .clone();
    let centroid = path
        .get(path.len() / 2)
        .copied()
        .unwrap_or_else(|| ProvinceId(0));
    let effective_anchor =
        choose_input_arrow_anchor(world, owner, &path, samples, anchor, centroid);
    match arrow_snapper(world, owner, effective_anchor, samples) {
        Ok(provinces) => {
            let chosen_anchor =
                choose_arrow_anchor(world, &path, &provinces, effective_anchor, centroid);
            let army = &mut world.player_armies[idx];
            army.order.as_mut().unwrap().arrow = Some(OffensiveArrow { provinces });
            army.order.as_mut().unwrap().anchor = chosen_anchor;
            Ok(())
        }
        Err(FrontlineError::ArrowTruncated(orig_len)) => {
            let provinces = arrow_snapper_truncated(world, owner, effective_anchor, samples);
            tracing::warn!(
                target: "frontline_api",
                army_id = id.0,
                orig_len = orig_len,
                "箭头已截断"
            );
            let chosen_anchor =
                choose_arrow_anchor(world, &path, &provinces, effective_anchor, centroid);
            let army = &mut world.player_armies[idx];
            army.order.as_mut().unwrap().arrow = Some(OffensiveArrow { provinces });
            army.order.as_mut().unwrap().anchor = chosen_anchor;
            Ok(())
        }
        Err(e) => Err(e),
    }
}

fn choose_input_arrow_anchor(
    world: &World,
    owner: CountryId,
    path: &[ProvinceId],
    samples: &[ProvinceId],
    fallback: ProvinceId,
    centroid: ProvinceId,
) -> ProvinceId {
    let first_enemy = samples.iter().copied().find(|&pid| {
        let pi = pid.0 as usize;
        pi < world.provinces.count
            && is_land(&world.map, pid.0)
            && is_hostile_to(world, owner, world.provinces.controllers[pi])
    });
    let Some(first_enemy) = first_enemy else {
        return fallback;
    };

    let mut candidates: Vec<(u32, ProvinceId)> = path
        .iter()
        .copied()
        .filter(|&p| are_adjacent(world, p, first_enemy))
        .filter_map(|p| bfs_land_dist(world, p, centroid, BFS_MAX_DEPTH).map(|d| (d, p)))
        .collect();
    candidates.sort_by_key(|(d, p)| (*d, p.0));
    candidates.first().map(|(_, p)| *p).unwrap_or(fallback)
}

fn choose_arrow_anchor(
    world: &World,
    path: &[ProvinceId],
    arrow_provs: &[ProvinceId],
    fallback: ProvinceId,
    centroid: ProvinceId,
) -> Option<ProvinceId> {
    let first_arrow = arrow_provs.first().copied();
    first_arrow
        .and_then(|fa| {
            let mut candidates: Vec<(u32, ProvinceId)> = path
                .iter()
                .filter(|&&p| are_adjacent(world, p, fa))
                .filter_map(|&p| bfs_land_dist(world, p, centroid, BFS_MAX_DEPTH).map(|d| (d, p)))
                .collect();
            candidates.sort_by_key(|(d, p)| (*d, p.0));
            candidates.first().map(|(_, p)| *p)
        })
        .or(Some(fallback))
}

fn arrow_snapper_truncated(
    world: &World,
    owner: CountryId,
    _anchor: ProvinceId,
    samples: &[ProvinceId],
) -> Vec<ProvinceId> {
    let mut projected: Vec<ProvinceId> = samples
        .iter()
        .filter(|&&pid| {
            let pi = pid.0 as usize;
            if pi >= world.provinces.count {
                return false;
            }
            if !is_land(&world.map, pid.0) {
                return false;
            }
            let ctrl = world.provinces.controllers[pi];
            is_hostile_to(world, owner, ctrl)
        })
        .copied()
        .collect();
    projected = dedup_consecutive(&projected);
    projected = stitch_gaps_limited(world, &projected, BFS_MAX_DEPTH);
    if projected.len() > MAX_ARROW_LEN {
        projected.truncate(MAX_ARROW_LEN);
    }
    projected
}

pub fn clear_arrow(world: &mut World, id: ArmyId) -> Result<(), FrontlineError> {
    let idx = find_army_mut(world, id).ok_or(FrontlineError::UnknownArmy)?;
    let army = &mut world.player_armies[idx];
    if let Some(o) = &mut army.order {
        o.arrow = None;
        o.executing = false;
    }
    Ok(())
}

pub fn execute_plan(world: &mut World, id: ArmyId) -> Result<(), FrontlineError> {
    let idx = find_army_mut(world, id).ok_or(FrontlineError::UnknownArmy)?;
    let army = &mut world.player_armies[idx];
    if let Some(o) = &mut army.order {
        if o.arrow.is_some() {
            o.executing = true;
        }
    }
    Ok(())
}

pub fn halt_plan(world: &mut World, id: ArmyId) -> Result<(), FrontlineError> {
    let idx = find_army_mut(world, id).ok_or(FrontlineError::UnknownArmy)?;
    let army = &mut world.player_armies[idx];
    if let Some(o) = &mut army.order {
        o.executing = false;
    }
    Ok(())
}

pub fn tick_frontlines(world: &mut World) {
    let at_war_set: std::collections::HashSet<CountryId> = world
        .player_armies
        .iter()
        .filter_map(|a| {
            if a.order.is_some() {
                Some(a.owner)
            } else {
                None
            }
        })
        .filter(|&owner| !world.diplomacy.is_at_war(owner))
        .collect();

    for army in world.player_armies.iter_mut() {
        if army.order.is_none() || !at_war_set.contains(&army.owner) {
            continue;
        }
        let o = army.order.as_mut().unwrap();
        if o.executing {
            o.executing = false;
        }
        if let Some(ref mut arrow) = o.arrow {
            arrow.provinces.clear();
        }
    }

    for mi in 0..world.divisions.count {
        let owner = world.divisions.owners[mi];
        if !owner.is_none() && !world.diplomacy.is_at_war(owner) {
            if has_player_manual_command(world, mi) {
                continue;
            }
            if let Some(dest) = world.divisions.destinations[mi].take() {
                let _ = dest;
            }
            crate::military::command_executor::clear_division_command(world, mi);
        }
    }

    let mut armies_to_process: Vec<(usize, ArmyId, CountryId, Vec<usize>, Option<FrontlineOrder>)> =
        world
            .player_armies
            .iter()
            .enumerate()
            .filter_map(|(i, a)| {
                if a.order.as_ref().is_some_and(|o| o.active) {
                    Some((i, a.id, a.owner, a.members.clone(), a.order.clone()))
                } else {
                    None
                }
            })
            .collect();

    for (army_idx, _army_id, owner, _members, order) in &mut armies_to_process {
        if let Some(o) = order {
            let original_len = o.path.len();
            o.path = repair_frontline_path(world, *owner, &o.path);
            if o.path.len() < original_len {
                tracing::warn!(
                    target: "frontline_tick",
                    army_idx = army_idx,
                    removed = original_len - o.path.len(),
                    "战线省份校验移除非法省"
                );
            }
            if o.path.is_empty() {
                let member_ids = world.player_armies[*army_idx].members.clone();
                for &mi in &member_ids {
                    if mi < world.divisions.count {
                        if has_player_manual_command(world, mi) {
                            continue;
                        }
                        world.divisions.destinations[mi] = None;
                        crate::military::command_executor::clear_division_command(world, mi);
                    }
                }
                o.active = false;
                o.arrow = None;
                o.anchor = None;
                tracing::warn!(
                    target: "frontline_tick",
                    army_idx = army_idx,
                    "前线已崩溃"
                );
                world.player_armies[*army_idx].order = Some(o.clone());
                continue;
            }

            let cobel = co_belligerent_set(world, *owner);

            // Arrow advance: remove conquered provinces, extend deeper
            if let Some(ref mut arrow) = o.arrow {
                arrow.provinces.retain(|p| {
                    let pi = p.0 as usize;
                    if pi >= world.provinces.count {
                        return false;
                    }
                    !cobel.contains(&world.provinces.controllers[pi])
                });
                if arrow.provinces.is_empty() {
                    o.arrow = None;
                    o.executing = false;
                } else {
                    // Extend arrow: BFS from last arrow province to find more enemy provinces
                    let mut depth = 0;
                    while arrow.provinces.len() < MAX_ARROW_LEN && depth < 3 {
                        depth += 1;
                        let last = *arrow.provinces.last().unwrap();
                        let li = last.0 as usize;
                        if li >= world.map.adjacencies.len() {
                            break;
                        }
                        let mut found = false;
                        for &nb in &world.map.adjacencies[li] {
                            let nbi = nb as usize;
                            if nbi >= world.provinces.count || !is_land(&world.map, nb) {
                                continue;
                            }
                            let nb_id = ProvinceId(nb);
                            if arrow.provinces.contains(&nb_id) || o.path.contains(&nb_id) {
                                continue;
                            }
                            let nb_ctrl = world.provinces.controllers[nbi];
                            if is_hostile_to(world, *owner, nb_ctrl) {
                                arrow.provinces.push(nb_id);
                                found = true;
                                if arrow.provinces.len() >= MAX_ARROW_LEN {
                                    break;
                                }
                            }
                        }
                        if !found {
                            break;
                        }
                    }
                }
            }

            // Path advance: BFS through conquered (co-belligerent) territory to find
            // all provinces that now border enemies. Previous single-hop logic couldn't
            // reach provinces deeper behind the front, causing the path to lag behind
            // retreating enemies (Bug #10).
            let path_set: HashSet<ProvinceId> = o.path.iter().copied().collect();
            let mut new_provinces: Vec<ProvinceId> = Vec::new();
            let mut bfs_visited: HashSet<ProvinceId> = path_set.clone();
            let mut bfs_queue: VecDeque<(ProvinceId, u32)> =
                o.path.iter().copied().map(|p| (p, 0)).collect();
            'path_advance: while let Some((p, depth)) = bfs_queue.pop_front() {
                if depth >= FRONTLINE_PATH_ADVANCE_DEPTH {
                    continue;
                }
                let pi = p.0 as usize;
                if pi >= world.map.adjacencies.len() {
                    continue;
                }
                for &nb in &world.map.adjacencies[pi] {
                    let nb_id = ProvinceId(nb);
                    let nbi = nb as usize;
                    if nbi >= world.provinces.count || !is_land(&world.map, nb) {
                        continue;
                    }
                    if !bfs_visited.insert(nb_id) {
                        continue;
                    }
                    let nb_ctrl = world.provinces.controllers[nbi];
                    if !cobel.contains(&nb_ctrl) {
                        continue;
                    }
                    let nb_neighs = world
                        .map
                        .adjacencies
                        .get(nbi)
                        .map(|v| v.as_slice())
                        .unwrap_or(&[]);
                    let has_enemy = nb_neighs.iter().any(|&nn| {
                        let nni = nn as usize;
                        if nni >= world.provinces.count || !is_land(&world.map, nn) {
                            return false;
                        }
                        is_hostile_to(world, *owner, world.provinces.controllers[nni])
                    });
                    if has_enemy {
                        new_provinces.push(nb_id);
                        if new_provinces.len() >= FRONTLINE_PATH_ADVANCE_CANDIDATE_CAP {
                            break 'path_advance;
                        }
                    }
                    bfs_queue.push_back((nb_id, depth + 1));
                }
            }

            if !new_provinces.is_empty() {
                o.path = merge_frontline_contacts(world, &cobel, &o.path, &new_provinces);
            }
            o.path = repair_frontline_path(world, *owner, &o.path);

            if o.path.is_empty() {
                let member_ids = world.player_armies[*army_idx].members.clone();
                for &mi in &member_ids {
                    if mi < world.divisions.count {
                        if has_player_manual_command(world, mi) {
                            continue;
                        }
                        world.divisions.destinations[mi] = None;
                        crate::military::command_executor::clear_division_command(world, mi);
                    }
                }
                o.active = false;
                o.arrow = None;
                o.anchor = None;
                tracing::warn!(
                    target: "frontline_tick",
                    army_idx = army_idx,
                    "前线已崩溃(推进后)"
                );
                world.player_armies[*army_idx].order = Some(o.clone());
                continue;
            }

            let army_snapshot = PlayerArmy {
                id: world.player_armies[*army_idx].id,
                name: world.player_armies[*army_idx].name.clone(),
                owner: *owner,
                commander: world.player_armies[*army_idx].commander,
                members: world.player_armies[*army_idx].members.clone(),
                order: Some(o.clone()),
            };

            let executors = if o.executing {
                pick_arrow_executors(world, &army_snapshot)
            } else {
                Vec::new()
            };
            let executor_set: HashSet<usize> = executors.iter().copied().collect();

            // Distribute only the non-executor members. Including executors here would
            // assign them line slots that get overridden by the Assault command below,
            // leaving those slots uncovered and letting the enemy walk through (Bug B).
            let garrison_snapshot = PlayerArmy {
                id: army_snapshot.id,
                name: army_snapshot.name.clone(),
                owner: army_snapshot.owner,
                commander: army_snapshot.commander,
                members: army_snapshot
                    .members
                    .iter()
                    .copied()
                    .filter(|m| !executor_set.contains(m))
                    .collect(),
                order: Some(o.clone()),
            };
            let distribution = frontline_distributor(world, &garrison_snapshot);

            let executor_targets: HashMap<usize, ProvinceId> = if let Some(ref arrow) = o.arrow {
                if !o.executing {
                    HashMap::new()
                } else {
                    assign_arrow_executor_targets(world, *owner, &executors, &o.path, arrow)
                }
            } else if o.executing {
                let mut enemy_targets: Vec<ProvinceId> = Vec::new();
                for &pp in &o.path {
                    let ppi = pp.0 as usize;
                    if ppi >= world.map.adjacencies.len() {
                        continue;
                    }
                    for &nb in &world.map.adjacencies[ppi] {
                        let nbi = nb as usize;
                        if nbi >= world.provinces.count || !is_land(&world.map, nb) {
                            continue;
                        }
                        let nb_id = ProvinceId(nb);
                        if o.path.contains(&nb_id) {
                            continue;
                        }
                        let nb_ctrl = world.provinces.controllers[nbi];
                        if is_hostile_to(world, *owner, nb_ctrl) {
                            if !enemy_targets.contains(&nb_id) {
                                enemy_targets.push(nb_id);
                            }
                        }
                    }
                }
                if enemy_targets.is_empty() {
                    HashMap::new()
                } else {
                    executors
                        .iter()
                        .filter_map(|&di| {
                            if di < world.divisions.count {
                                let loc = world.divisions.locations[di];
                                let mut best: Option<(u32, ProvinceId)> = None;
                                for &et in &enemy_targets {
                                    if !crate::military::movement::can_enter_province(
                                        world, *owner, et,
                                    ) {
                                        continue;
                                    }
                                    let d = bfs_land_dist(world, loc, et, BFS_MAX_DEPTH)
                                        .unwrap_or(u32::MAX);
                                    match best {
                                        Some((prev, _)) if d >= prev => {}
                                        _ => best = Some((d, et)),
                                    }
                                }
                                best.map(|(_, t)| (di, t))
                            } else {
                                None
                            }
                        })
                        .collect()
                }
            } else {
                HashMap::new()
            };

            let dist_map: HashMap<usize, ProvinceId> = distribution.into_iter().collect();

            let member_ids = world.player_armies[*army_idx].members.clone();
            for &mi in &member_ids {
                if mi >= world.divisions.count {
                    continue;
                }
                let max_org = world.divisions.max_organisation[mi].max(1e-6);
                let org_ratio = world.divisions.organisation[mi] / max_org;
                let str_now = world.divisions.strength[mi];
                let broken = org_ratio < 0.05 || str_now < 0.05;

                if broken {
                    if let Some(dest) = world.divisions.destinations[mi] {
                        let di = dest.0 as usize;
                        if di < world.provinces.count {
                            let dest_ctrl = world.provinces.controllers[di];
                            if dest_ctrl == *owner {
                                continue;
                            }
                        }
                    }
                    let cur = world.divisions.locations[mi];
                    let cur_i = cur.0 as usize;
                    if cur_i < world.provinces.count {
                        let cur_ctrl = world.provinces.controllers[cur_i];
                        if cur_ctrl != *owner {
                            let retreat = find_nearest_friendly(world, *owner, cur);
                            if let Some(r) = retreat {
                                crate::military::command_executor::issue_retreat_command(
                                    world, mi, r,
                                );
                            }
                        }
                    }
                    continue;
                }

                if let Some(&arrow_target) = executor_targets.get(&mi) {
                    crate::military::command_executor::issue_player_frontline_command(
                        world,
                        mi,
                        hoi4_state::DivisionIntent::Assault,
                        arrow_target,
                        DivisionAssignment {
                            front: None,
                            role: DivisionRole::Assault,
                            target: Some(arrow_target),
                            assigned_at: world.elapsed_hours,
                        },
                    );
                } else if let Some(&target) = dist_map.get(&mi) {
                    if !o.executing && world.divisions.destinations[mi].is_some() {
                        continue;
                    }
                    crate::military::command_executor::issue_player_frontline_command(
                        world,
                        mi,
                        hoi4_state::DivisionIntent::Garrison,
                        target,
                        DivisionAssignment {
                            front: None,
                            role: DivisionRole::Garrison,
                            target: Some(target),
                            assigned_at: world.elapsed_hours,
                        },
                    );
                }
            }

            if let Some(ref mut arrow) = o.arrow {
                if o.executing {
                    let all_conquered = arrow.provinces.iter().all(|&p| {
                        let pi = p.0 as usize;
                        if pi >= world.provinces.count {
                            return true;
                        }
                        let ctrl = world.provinces.controllers[pi];
                        ctrl == *owner
                    });
                    if all_conquered {
                        let mut extended = false;
                        if let Some(&last_conquered) = arrow.provinces.last() {
                            let lci = last_conquered.0 as usize;
                            if lci < world.map.adjacencies.len() {
                                for &nb in &world.map.adjacencies[lci] {
                                    let nbi = nb as usize;
                                    if nbi >= world.provinces.count || !is_land(&world.map, nb) {
                                        continue;
                                    }
                                    let nb_id = ProvinceId(nb);
                                    if arrow.provinces.contains(&nb_id) || o.path.contains(&nb_id) {
                                        continue;
                                    }
                                    let nb_ctrl = world.provinces.controllers[nbi];
                                    if is_hostile_to(world, *owner, nb_ctrl) {
                                        arrow.provinces.push(nb_id);
                                        extended = true;
                                        break;
                                    }
                                }
                            }
                        }
                        if !extended {
                            if let Some(&first_conquered) = arrow.provinces.first() {
                                let fci = first_conquered.0 as usize;
                                if fci < world.map.adjacencies.len() {
                                    for &nb in &world.map.adjacencies[fci] {
                                        let nbi = nb as usize;
                                        if nbi >= world.provinces.count || !is_land(&world.map, nb)
                                        {
                                            continue;
                                        }
                                        let nb_id = ProvinceId(nb);
                                        if arrow.provinces.contains(&nb_id)
                                            || o.path.contains(&nb_id)
                                        {
                                            continue;
                                        }
                                        let nb_ctrl = world.provinces.controllers[nbi];
                                        if is_hostile_to(world, *owner, nb_ctrl) {
                                            arrow.provinces.insert(0, nb_id);
                                            extended = true;
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                        if !extended {
                            for &pp in &o.path {
                                let ppi = pp.0 as usize;
                                if ppi >= world.map.adjacencies.len() {
                                    continue;
                                }
                                for &nb in &world.map.adjacencies[ppi] {
                                    let nbi = nb as usize;
                                    if nbi >= world.provinces.count || !is_land(&world.map, nb) {
                                        continue;
                                    }
                                    let nb_id = ProvinceId(nb);
                                    if arrow.provinces.contains(&nb_id) || o.path.contains(&nb_id) {
                                        continue;
                                    }
                                    let nb_ctrl = world.provinces.controllers[nbi];
                                    if is_hostile_to(world, *owner, nb_ctrl) {
                                        arrow.provinces = vec![nb_id];
                                        extended = true;
                                        break;
                                    }
                                }
                                if extended {
                                    break;
                                }
                            }
                        }
                        if !extended {
                            o.arrow = None;
                        }
                        tracing::warn!(
                            target: "frontline_tick",
                            army_idx = army_idx,
                            extended = extended,
                            "箭头已完成,{}",
                            if extended { "已延伸" } else { "无法延伸" }
                        );
                    }
                }
            }

            if let Some(ref arrow) = o.arrow {
                if let Some(anchor) = o.anchor {
                    if !o.path.contains(&anchor) {
                        if !o.path.is_empty() {
                            let best = o
                                .path
                                .iter()
                                .filter_map(|&p| {
                                    bfs_land_dist(world, anchor, p, BFS_MAX_DEPTH).map(|d| (d, p))
                                })
                                .min_by(|(d1, p1), (d2, p2)| d1.cmp(d2).then(p1.0.cmp(&p2.0)));
                            o.anchor = best.map(|(_, p)| p);
                        } else {
                            o.arrow = None;
                            o.anchor = None;
                        }
                    }
                } else if let Some(first_arrow) = arrow.provinces.first() {
                    let best = o
                        .path
                        .iter()
                        .filter(|&&p| are_adjacent(world, p, *first_arrow))
                        .filter_map(|&p| {
                            let centroid = o.path.get(o.path.len() / 2).copied();
                            centroid.and_then(|c| {
                                bfs_land_dist(world, p, c, BFS_MAX_DEPTH).map(|d| (d, p))
                            })
                        })
                        .min_by(|(d1, p1), (d2, p2)| d1.cmp(d2).then(p1.0.cmp(&p2.0)));
                    o.anchor = best.map(|(_, p)| p);
                }
            }

            world.player_armies[*army_idx].order = Some(o.clone());
        }
    }

    for army in &mut world.player_armies {
        if let Some(o) = army.order.as_mut() {
            if o.active {
                army.members.retain(|&mi| mi < world.divisions.count);
            }
        }
    }

    rebuild_locked_divisions(world);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn peacetime_frontline_tick_preserves_player_manual_move_orders() {
        let mut world = test_line_world();
        let ger = world.country("GER").expect("GER country");
        let div = world
            .divisions
            .push(ger, ProvinceId(1), 0, 60.0, 1000.0, "Manual move".into())
            as usize;

        assert!(
            crate::military::command_executor::issue_player_manual_command(
                &mut world,
                div,
                ProvinceId(3),
            )
        );
        world.divisions.destinations[div] = Some(ProvinceId(3));

        tick_frontlines(&mut world);

        assert_eq!(world.divisions.destinations[div], Some(ProvinceId(3)));
        assert!(has_player_manual_command(&world, div));

        crate::military::command_executor::tick_division_commands_daily(&mut world);
        crate::military::movement::daily_movement_tick(&mut world);
        assert_eq!(world.divisions.locations[div], ProvinceId(2));
        assert_eq!(world.divisions.destinations[div], Some(ProvinceId(3)));

        world.elapsed_hours += 24;
        tick_frontlines(&mut world);
        crate::military::command_executor::tick_division_commands_daily(&mut world);
        crate::military::movement::daily_movement_tick(&mut world);
        assert_eq!(world.divisions.locations[div], ProvinceId(3));
        assert_eq!(world.divisions.destinations[div], None);
    }

    #[test]
    fn path_advance_is_bounded_to_local_front_movement() {
        let mut world = test_line_world();
        let ger = world.country("GER").expect("GER country");
        let sov = world.country("SOV").expect("SOV country");
        make_war(&mut world, ger, sov);

        let div = world
            .divisions
            .push(ger, ProvinceId(1), 0, 60.0, 1000.0, "Front test".into())
            as usize;
        let army_id =
            create_army(&mut world, ger, vec![div], "Bounded Front".into()).expect("army");
        world.player_armies[0].order = Some(FrontlineOrder {
            path: vec![ProvinceId(1)],
            arrow: None,
            anchor: None,
            active: true,
            executing: false,
        });

        tick_frontlines(&mut world);

        let path = &world.player_armies[0]
            .order
            .as_ref()
            .expect("frontline order")
            .path;
        assert!(
            !path.contains(&ProvinceId(10)),
            "frontline should not jump across the whole friendly corridor in one tick: {path:?}"
        );

        world.player_armies[0].order.as_mut().unwrap().path = vec![ProvinceId(4)];
        tick_frontlines(&mut world);

        let path = &world.player_armies[0]
            .order
            .as_ref()
            .expect("frontline order")
            .path;
        assert!(
            path.contains(&ProvinceId(10)),
            "frontline should still advance when the new contact is within the local search bound: {path:?}"
        );

        assert_eq!(world.player_armies[0].id, army_id);
    }

    #[test]
    fn tick_reconnects_disconnected_frontline_path() {
        let mut world = test_line_world();
        let ger = world.country("GER").expect("GER country");

        let div = world
            .divisions
            .push(ger, ProvinceId(6), 0, 60.0, 1000.0, "Reconnect test".into())
            as usize;
        create_army(&mut world, ger, vec![div], "Reconnect Front".into()).expect("army");
        world.player_armies[0].order = Some(FrontlineOrder {
            path: vec![ProvinceId(6), ProvinceId(9)],
            arrow: None,
            anchor: None,
            active: true,
            executing: false,
        });

        tick_frontlines(&mut world);

        let path = &world.player_armies[0]
            .order
            .as_ref()
            .expect("frontline order")
            .path;
        assert_eq!(
            path,
            &vec![ProvinceId(6), ProvinceId(7), ProvinceId(8), ProvinceId(9)],
            "frontline tick should reconnect local gaps through controlled land: {path:?}"
        );
        assert!(
            path.windows(2)
                .all(|pair| are_adjacent(&world, pair[0], pair[1])),
            "repaired frontline path should be contiguous: {path:?}"
        );
    }

    #[test]
    fn wartime_tick_reconnects_long_gaps_on_same_enemy_border() {
        let mut world = test_parallel_front_world();
        let ger = world.country("GER").expect("GER country");
        let sov = world.country("SOV").expect("SOV country");
        make_war(&mut world, ger, sov);

        let div = world
            .divisions
            .push(ger, ProvinceId(1), 0, 60.0, 1000.0, "Long reconnect".into())
            as usize;
        create_army(&mut world, ger, vec![div], "Long Reconnect Front".into()).expect("army");
        world.player_armies[0].order = Some(FrontlineOrder {
            path: vec![ProvinceId(1), ProvinceId(10)],
            arrow: None,
            anchor: None,
            active: true,
            executing: false,
        });

        tick_frontlines(&mut world);

        let path = &world.player_armies[0]
            .order
            .as_ref()
            .expect("frontline order")
            .path;
        let expected: Vec<ProvinceId> = (1..=10).map(ProvinceId).collect();
        assert_eq!(
            path, &expected,
            "wartime frontline should immediately fill same-enemy border gaps: {path:?}"
        );
    }

    #[test]
    fn path_advance_keeps_captured_corridor_to_new_contact() {
        let mut world = test_line_world();
        let ger = world.country("GER").expect("GER country");
        let sov = world.country("SOV").expect("SOV country");
        make_war(&mut world, ger, sov);

        world.provinces.controllers[11] = ger;

        let div = world.divisions.push(
            ger,
            ProvinceId(9),
            0,
            60.0,
            1000.0,
            "Captured corridor".into(),
        ) as usize;
        create_army(&mut world, ger, vec![div], "Captured Corridor Front".into()).expect("army");
        world.player_armies[0].order = Some(FrontlineOrder {
            path: vec![ProvinceId(9)],
            arrow: None,
            anchor: None,
            active: true,
            executing: false,
        });

        tick_frontlines(&mut world);

        let path = &world.player_armies[0]
            .order
            .as_ref()
            .expect("frontline order")
            .path;
        assert!(
            path.contains(&ProvinceId(10)) && path.contains(&ProvinceId(11)),
            "frontline should cover the captured corridor up to the new enemy contact: {path:?}"
        );
    }

    #[test]
    fn snapper_ignores_neutral_foreign_border_during_war() {
        let mut world = test_three_country_border_world();
        let ger = world.country("GER").expect("GER country");
        let cze = world.country("CZE").expect("CZE country");
        make_war(&mut world, ger, cze);

        let path = frontline_snapper(&world, ger, &[ProvinceId(2), ProvinceId(1)])
            .expect("frontline should snap to the active war border");

        assert!(
            path.contains(&ProvinceId(1)),
            "frontline should include the GER-CZE border province: {path:?}"
        );
        assert!(
            !path.contains(&ProvinceId(2)),
            "frontline should not snap onto the neutral GER-POL border: {path:?}"
        );
    }

    #[test]
    fn snapper_locks_to_dominant_drawn_target_country() {
        let world = test_dual_border_world();
        let ger = world.country("GER").expect("GER country");

        let path = frontline_snapper(
            &world,
            ger,
            &[ProvinceId(4), ProvinceId(3), ProvinceId(2), ProvinceId(1)],
        )
        .expect("frontline should snap to the country covered by most samples");

        assert!(
            path.contains(&ProvinceId(1))
                && path.contains(&ProvinceId(2))
                && path.contains(&ProvinceId(3)),
            "frontline should follow the GER-CZE border: {path:?}"
        );
        assert!(
            !path.contains(&ProvinceId(4)),
            "frontline should not keep the minor AUS-only border contact: {path:?}"
        );
    }

    fn test_line_world() -> World {
        let mut adjacencies = vec![Vec::new(); 13];
        for raw in 1..12 {
            adjacencies[raw].push((raw + 1) as u16);
            adjacencies[raw + 1].push(raw as u16);
        }

        let map = hoi4_map::GameMap {
            definitions: (0..=12)
                .map(|raw| {
                    if raw == 0 {
                        None
                    } else {
                        Some(hoi4_map::ProvinceDefinition {
                            id: raw as u16,
                            r: 0,
                            g: 0,
                            b: 0,
                            province_type: hoi4_map::ProvinceType::Land,
                            coastal: false,
                            terrain: String::new(),
                            continent: 0,
                        })
                    }
                })
                .collect(),
            rgb_to_id: std::collections::HashMap::new(),
            province_map: hoi4_map::ProvinceMap {
                width: 0,
                height: 0,
                pixels: vec![],
            },
            adjacencies,
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
        };

        let ger = hoi4_data::CountryTag::new("GER");
        let sov = hoi4_data::CountryTag::new("SOV");
        let mut data = hoi4_data::GameData::default();
        data.countries.insert(
            ger.clone(),
            hoi4_data::Country {
                tag: ger.clone(),
                color: hoi4_data::Color { r: 0, g: 0, b: 0 },
                graphical_culture: String::new(),
                capital: 1,
                ruling_party: "fascism".into(),
                technologies: vec![],
            },
        );
        data.countries.insert(
            sov.clone(),
            hoi4_data::Country {
                tag: sov.clone(),
                color: hoi4_data::Color { r: 0, g: 0, b: 0 },
                graphical_culture: String::new(),
                capital: 11,
                ruling_party: "communism".into(),
                technologies: vec![],
            },
        );
        for raw in 1..=12 {
            let owner = if raw <= 10 { ger.clone() } else { sov.clone() };
            data.states.push(hoi4_data::State {
                id: raw as u16,
                name: format!("S{raw}"),
                manpower: 0,
                owner: owner.clone(),
                cores: vec![owner],
                provinces: vec![raw as u16],
                category: String::new(),
                infrastructure: 1,
                victory_points: vec![],
                resources: vec![],
            });
        }

        World::new(Arc::new(map), Arc::new(data))
    }

    fn test_parallel_front_world() -> World {
        let mut adjacencies = vec![Vec::new(); 21];
        for raw in 1..10 {
            adjacencies[raw].push((raw + 1) as u16);
            adjacencies[raw + 1].push(raw as u16);
        }
        for raw in 11..20 {
            adjacencies[raw].push((raw + 1) as u16);
            adjacencies[raw + 1].push(raw as u16);
        }
        for raw in 1..=10 {
            let enemy = raw + 10;
            adjacencies[raw].push(enemy as u16);
            adjacencies[enemy].push(raw as u16);
        }

        let map = hoi4_map::GameMap {
            definitions: (0..=20)
                .map(|raw| {
                    if raw == 0 {
                        None
                    } else {
                        Some(hoi4_map::ProvinceDefinition {
                            id: raw as u16,
                            r: 0,
                            g: 0,
                            b: 0,
                            province_type: hoi4_map::ProvinceType::Land,
                            coastal: false,
                            terrain: String::new(),
                            continent: 0,
                        })
                    }
                })
                .collect(),
            rgb_to_id: std::collections::HashMap::new(),
            province_map: hoi4_map::ProvinceMap {
                width: 0,
                height: 0,
                pixels: vec![],
            },
            adjacencies,
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
        };

        let ger = hoi4_data::CountryTag::new("GER");
        let sov = hoi4_data::CountryTag::new("SOV");
        let mut data = hoi4_data::GameData::default();
        data.countries.insert(
            ger.clone(),
            hoi4_data::Country {
                tag: ger.clone(),
                color: hoi4_data::Color { r: 0, g: 0, b: 0 },
                graphical_culture: String::new(),
                capital: 1,
                ruling_party: "fascism".into(),
                technologies: vec![],
            },
        );
        data.countries.insert(
            sov.clone(),
            hoi4_data::Country {
                tag: sov.clone(),
                color: hoi4_data::Color { r: 0, g: 0, b: 0 },
                graphical_culture: String::new(),
                capital: 11,
                ruling_party: "communism".into(),
                technologies: vec![],
            },
        );
        for raw in 1..=20 {
            let owner = if raw <= 10 { ger.clone() } else { sov.clone() };
            data.states.push(hoi4_data::State {
                id: raw as u16,
                name: format!("S{raw}"),
                manpower: 0,
                owner: owner.clone(),
                cores: vec![owner],
                provinces: vec![raw as u16],
                category: String::new(),
                infrastructure: 1,
                victory_points: vec![],
                resources: vec![],
            });
        }

        World::new(Arc::new(map), Arc::new(data))
    }

    fn test_three_country_border_world() -> World {
        let mut adjacencies = vec![Vec::new(); 5];
        adjacencies[1].push(3);
        adjacencies[3].push(1);
        adjacencies[2].push(4);
        adjacencies[4].push(2);

        let map = hoi4_map::GameMap {
            definitions: (0..=4)
                .map(|raw| {
                    if raw == 0 {
                        None
                    } else {
                        Some(hoi4_map::ProvinceDefinition {
                            id: raw as u16,
                            r: 0,
                            g: 0,
                            b: 0,
                            province_type: hoi4_map::ProvinceType::Land,
                            coastal: false,
                            terrain: String::new(),
                            continent: 0,
                        })
                    }
                })
                .collect(),
            rgb_to_id: std::collections::HashMap::new(),
            province_map: hoi4_map::ProvinceMap {
                width: 0,
                height: 0,
                pixels: vec![],
            },
            adjacencies,
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
        };

        let ger = hoi4_data::CountryTag::new("GER");
        let cze = hoi4_data::CountryTag::new("CZE");
        let pol = hoi4_data::CountryTag::new("POL");
        let mut data = hoi4_data::GameData::default();
        for tag in [&ger, &cze, &pol] {
            data.countries.insert(
                tag.clone(),
                hoi4_data::Country {
                    tag: tag.clone(),
                    color: hoi4_data::Color { r: 0, g: 0, b: 0 },
                    graphical_culture: String::new(),
                    capital: 1,
                    ruling_party: "neutrality".into(),
                    technologies: vec![],
                },
            );
        }
        for (raw, owner) in [(1, &ger), (2, &ger), (3, &cze), (4, &pol)] {
            data.states.push(hoi4_data::State {
                id: raw as u16,
                name: format!("S{raw}"),
                manpower: 0,
                owner: owner.clone(),
                cores: vec![owner.clone()],
                provinces: vec![raw as u16],
                category: String::new(),
                infrastructure: 1,
                victory_points: vec![],
                resources: vec![],
            });
        }

        World::new(Arc::new(map), Arc::new(data))
    }

    fn test_dual_border_world() -> World {
        let mut adjacencies = vec![Vec::new(); 22];
        for raw in 1..4 {
            adjacencies[raw].push((raw + 1) as u16);
            adjacencies[raw + 1].push(raw as u16);
        }
        for (friendly, foreign) in [(1, 11), (2, 12), (3, 13), (4, 21)] {
            adjacencies[friendly].push(foreign as u16);
            adjacencies[foreign].push(friendly as u16);
        }

        let map = hoi4_map::GameMap {
            definitions: (0..=21)
                .map(|raw| {
                    if raw == 0 || (5..=10).contains(&raw) || (14..=20).contains(&raw) {
                        None
                    } else {
                        Some(hoi4_map::ProvinceDefinition {
                            id: raw as u16,
                            r: 0,
                            g: 0,
                            b: 0,
                            province_type: hoi4_map::ProvinceType::Land,
                            coastal: false,
                            terrain: String::new(),
                            continent: 0,
                        })
                    }
                })
                .collect(),
            rgb_to_id: std::collections::HashMap::new(),
            province_map: hoi4_map::ProvinceMap {
                width: 0,
                height: 0,
                pixels: vec![],
            },
            adjacencies,
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
        };

        let ger = hoi4_data::CountryTag::new("GER");
        let cze = hoi4_data::CountryTag::new("CZE");
        let aus = hoi4_data::CountryTag::new("AUS");
        let mut data = hoi4_data::GameData::default();
        for tag in [&ger, &cze, &aus] {
            data.countries.insert(
                tag.clone(),
                hoi4_data::Country {
                    tag: tag.clone(),
                    color: hoi4_data::Color { r: 0, g: 0, b: 0 },
                    graphical_culture: String::new(),
                    capital: 1,
                    ruling_party: "neutrality".into(),
                    technologies: vec![],
                },
            );
        }
        for raw in 1..=4 {
            data.states.push(hoi4_data::State {
                id: raw as u16,
                name: format!("GER {raw}"),
                manpower: 0,
                owner: ger.clone(),
                cores: vec![ger.clone()],
                provinces: vec![raw as u16],
                category: String::new(),
                infrastructure: 1,
                victory_points: vec![],
                resources: vec![],
            });
        }
        for raw in 11..=13 {
            data.states.push(hoi4_data::State {
                id: raw as u16,
                name: format!("CZE {raw}"),
                manpower: 0,
                owner: cze.clone(),
                cores: vec![cze.clone()],
                provinces: vec![raw as u16],
                category: String::new(),
                infrastructure: 1,
                victory_points: vec![],
                resources: vec![],
            });
        }
        data.states.push(hoi4_data::State {
            id: 21,
            name: "AUS 21".into(),
            manpower: 0,
            owner: aus.clone(),
            cores: vec![aus],
            provinces: vec![21],
            category: String::new(),
            infrastructure: 1,
            victory_points: vec![],
            resources: vec![],
        });

        World::new(Arc::new(map), Arc::new(data))
    }

    fn make_war(world: &mut World, attacker: CountryId, defender: CountryId) {
        let war_id = world.diplomacy.next_war_id;
        let mut attackers = std::collections::HashSet::new();
        attackers.insert(attacker);
        let mut defenders = std::collections::HashSet::new();
        defenders.insert(defender);
        world.diplomacy.wars.insert(
            war_id,
            hoi4_state::War {
                id: war_id,
                primary_attacker: attacker,
                primary_defender: defender,
                attackers,
                defenders,
                started_at_hour: world.elapsed_hours,
                attacker_war_score: 0.0,
                defender_war_score: 0.0,
                attacker_wargoals: vec![],
                defender_wargoals: vec![],
                war_join_policies: std::collections::HashMap::new(),
            },
        );
        world.diplomacy.next_war_id += 1;
        world.countries.at_war[attacker.0 as usize] = true;
        world.countries.at_war[defender.0 as usize] = true;
    }
}
