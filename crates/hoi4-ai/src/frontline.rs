//! 前线探测：找出某国与每个敌国之间的"前线"——
//! 即己方控制的、与敌方控制的省份相邻的州集合。
//!
//! 战术 AI 的所有决策（进攻 / 防御 / 包围 / 空中 CAS / 海军支援）都建立在
//! "我和谁在哪些州交锋" 这个事实之上。本模块只输出事实，不做决策。
//!
//! ## 关键概念
//! - **owner vs controller**：HOI4 中"占领"是 controller 字段。前线指的是当前
//!   *实际*控制（controller），不是法理（owner）。
//! - **陆地相邻**：通过 `GameMap.adjacencies` 取邻居省份；海邻接也会包含在该列表，
//!   但被对方占领的海省份才视作"敌前线"是不合理的，因此本模块只看陆地省份对陆地省份。
//! - **战争状态**：仅当某国与本国 `at_war_with` 才被视为敌人；和平时期相邻的他国不算。

use hoi4_state::{CountryId, ProvinceId, StateId, World};

/// 一段前线：本国某州 vs 某敌国（多个敌方控制的相邻州）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontSegment {
    /// 本国持有方
    pub owner: CountryId,
    /// 这段前线的敌国
    pub enemy: CountryId,
    /// 本国控制的、紧贴敌方的州
    pub friendly_states: Vec<StateId>,
    /// 敌方控制的、紧贴本国的州
    pub enemy_states: Vec<StateId>,
}

impl FrontSegment {
    /// 该前线段是否仍然存在
    pub fn is_active(&self) -> bool {
        !self.friendly_states.is_empty() && !self.enemy_states.is_empty()
    }
}

/// 整个战争前线 — 按敌国分组的前线段集合。
#[derive(Debug, Clone)]
pub struct FrontLine {
    pub country: CountryId,
    /// 按敌国 → 前线段
    pub segments: Vec<FrontSegment>,
}

impl FrontLine {
    pub fn empty(country: CountryId) -> Self {
        Self {
            country,
            segments: Vec::new(),
        }
    }

    /// 该国是否有任何前线
    pub fn has_front(&self) -> bool {
        self.segments.iter().any(|s| s.is_active())
    }

    /// 总的前线州数（去重）
    pub fn frontline_state_count(&self) -> usize {
        let mut set = std::collections::HashSet::new();
        for seg in &self.segments {
            for &s in &seg.friendly_states {
                set.insert(s);
            }
        }
        set.len()
    }

    /// 取与某敌国的前线段
    pub fn segment_against(&self, enemy: CountryId) -> Option<&FrontSegment> {
        self.segments.iter().find(|s| s.enemy == enemy)
    }
}

/// 估算某国总前线州数 — 用于在不调用 compute_front 的情况下推算
/// 对方对一段前线的力量投入占比。
///
/// 定义为：该国控制的、至少有一条相邻陆地省份被任意交战国控制的州数。
/// 复杂度 O(states × provinces_per_state × adjacencies)，与 compute_segment 同阶，
/// 但不需要构造 FrontLine。
pub fn frontline_state_count_of(world: &World, country: CountryId) -> usize {
    let mut count = 0usize;
    for si in 0..world.states.count {
        let state_controller = world.states.controllers[si];
        if !is_friendly_staging_controller(world, country, state_controller) {
            continue;
        }
        let mut on_front = false;
        'outer: for &p in &world.states.provinces[si] {
            if !is_land_province(world, p) {
                continue;
            }
            let pi = p.0 as usize;
            if pi >= world.map.adjacencies.len() {
                continue;
            }
            for &adj_raw in &world.map.adjacencies[pi] {
                let adj = ProvinceId(adj_raw);
                if !is_land_province(world, adj) {
                    continue;
                }
                let adj_pi = adj.0 as usize;
                if adj_pi >= world.provinces.count {
                    continue;
                }
                let adj_ctrl = world.provinces.controllers[adj_pi];
                if adj_ctrl.is_none() || adj_ctrl == country {
                    continue;
                }
                if world.diplomacy.at_war_with(country, adj_ctrl) {
                    on_front = true;
                    break 'outer;
                }
            }
        }
        if on_front {
            count += 1;
        }
    }
    count
}

/// 仅看陆地相邻：跳过海洋 / 湖泊省份。
fn is_land_province(world: &World, p: ProvinceId) -> bool {
    if p.is_none() {
        return false;
    }
    let pi = p.0 as usize;
    if pi >= world.provinces.count {
        return false;
    }
    if let Some(Some(def)) = world.map.definitions.get(pi) {
        matches!(def.province_type, hoi4_map::ProvinceType::Land)
    } else {
        false
    }
}

/// 计算某国家针对所有交战敌国的前线。
pub fn compute_front(world: &World, country: CountryId) -> FrontLine {
    let mut front = FrontLine::empty(country);

    if country.is_none() || (country.0 as usize) >= world.countries.count {
        return front;
    }
    if !world.diplomacy.is_at_war(country) {
        return compute_peace_border(world, country);
    }

    // 找出所有交战敌国
    let mut enemies: Vec<CountryId> = Vec::new();
    for ci in 0..world.countries.count {
        let other = CountryId(ci as u16);
        if other == country {
            continue;
        }
        if world.diplomacy.at_war_with(country, other) {
            enemies.push(other);
        }
    }

    for enemy in enemies {
        let seg = compute_segment(world, country, enemy);
        if seg.is_active() {
            front.segments.push(seg);
        }
    }

    front
}

/// 计算 country 与单个敌国 enemy 之间的前线段。
///
/// 修复深处敌方省份不可见 + 性能优化：
/// - enemy_states 不仅包含"与我方控制的省份直接相邻的敌方州"
/// - 还包含"enemy owned 且 controller 不是我方的所有州"
/// - 还包含"有 enemy 健康师团驻扎的州"（用预计算避免 O(N²)）
pub fn compute_segment(world: &World, country: CountryId, enemy: CountryId) -> FrontSegment {
    let mut friendly: std::collections::HashSet<StateId> = std::collections::HashSet::new();
    let mut hostile: std::collections::HashSet<StateId> = std::collections::HashSet::new();

    for si in 0..world.states.count {
        if !is_friendly_staging_controller(world, country, world.states.controllers[si]) {
            continue;
        }
        let mut on_front = false;
        for &p in &world.states.provinces[si] {
            if !is_land_province(world, p) {
                continue;
            }
            let pi = p.0 as usize;
            if pi >= world.map.adjacencies.len() {
                continue;
            }
            for &adj_raw in &world.map.adjacencies[pi] {
                let adj = ProvinceId(adj_raw);
                if !is_land_province(world, adj) {
                    continue;
                }
                let adj_pi = adj.0 as usize;
                if adj_pi >= world.provinces.count {
                    continue;
                }
                let adj_ctrl = world.provinces.controllers[adj_pi];
                if adj_ctrl == enemy {
                    on_front = true;
                    let adj_state = world.provinces.state_of[adj_pi];
                    if !adj_state.is_none() {
                        hostile.insert(adj_state);
                    }
                }
            }
        }
        if on_front {
            friendly.insert(StateId(si as u16));
        }
    }

    let mut enemy_div_states: std::collections::HashSet<StateId> = std::collections::HashSet::new();
    for di in 0..world.divisions.count {
        if world.divisions.owners[di] != enemy {
            continue;
        }
        let max_org = world.divisions.max_organisation[di].max(1e-6);
        let org_ratio = world.divisions.organisation[di] / max_org;
        if org_ratio < 0.05 || world.divisions.strength[di] < 0.05 {
            continue;
        }
        let loc = world.divisions.locations[di];
        let si = loc.0 as usize;
        if si < world.provinces.count {
            let state = world.provinces.state_of[si];
            if !state.is_none() {
                enemy_div_states.insert(state);
            }
        }
    }

    for si in 0..world.states.count {
        if world.states.owners[si] != enemy {
            continue;
        }
        let state_ctrl = world.states.controllers[si];
        if state_ctrl == country {
            continue;
        }
        if state_ctrl != enemy && !enemy_div_states.contains(&StateId(si as u16)) {
            continue;
        }
        hostile.insert(StateId(si as u16));
    }

    for &sid in &enemy_div_states {
        hostile.insert(sid);
    }

    let mut friendly_states: Vec<StateId> = friendly.into_iter().collect();
    let mut enemy_states: Vec<StateId> = hostile.into_iter().collect();
    friendly_states.sort_by_key(|s| s.0);
    enemy_states.sort_by_key(|s| s.0);

    FrontSegment {
        owner: country,
        enemy,
        friendly_states,
        enemy_states,
    }
}

fn is_friendly_staging_controller(
    world: &World,
    country: CountryId,
    controller: CountryId,
) -> bool {
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

/// 计算一个州周围，被某国 `who` 控制的邻接州数 / 总陆地邻接州数。
/// 用于包围检测："邻居有多少被我们控制" → 越接近 1.0 越倾向被包围。
pub fn neighbor_ratio_controlled_by(world: &World, state: StateId, who: CountryId) -> (u32, u32) {
    if state.is_none() {
        return (0, 0);
    }
    let si = state.0 as usize;
    if si >= world.states.count {
        return (0, 0);
    }
    let mut neighbor_states: std::collections::HashSet<StateId> = std::collections::HashSet::new();
    for &p in &world.states.provinces[si] {
        if !is_land_province(world, p) {
            continue;
        }
        let pi = p.0 as usize;
        if pi >= world.map.adjacencies.len() {
            continue;
        }
        for &adj_raw in &world.map.adjacencies[pi] {
            let adj = ProvinceId(adj_raw);
            if !is_land_province(world, adj) {
                continue;
            }
            let api = adj.0 as usize;
            if api >= world.provinces.count {
                continue;
            }
            let s = world.provinces.state_of[api];
            if s == state || s.is_none() {
                continue;
            }
            neighbor_states.insert(s);
        }
    }
    let total = neighbor_states.len() as u32;
    let mut by_who = 0u32;
    for s in neighbor_states {
        let other_si = s.0 as usize;
        if other_si < world.states.count && world.states.controllers[other_si] == who {
            by_who += 1;
        }
    }
    (by_who, total)
}

fn compute_peace_border(world: &World, country: CountryId) -> FrontLine {
    let mut front = FrontLine::empty(country);

    let mut foreign_ctrls: std::collections::HashSet<CountryId> = std::collections::HashSet::new();
    for si in 0..world.states.count {
        if !is_friendly_staging_controller(world, country, world.states.controllers[si]) {
            continue;
        }
        for &p in &world.states.provinces[si] {
            if !is_land_province(world, p) {
                continue;
            }
            let pi = p.0 as usize;
            if pi >= world.map.adjacencies.len() {
                continue;
            }
            for &adj_raw in &world.map.adjacencies[pi] {
                let adj = ProvinceId(adj_raw);
                if !is_land_province(world, adj) {
                    continue;
                }
                let adj_pi = adj.0 as usize;
                if adj_pi >= world.provinces.count {
                    continue;
                }
                let adj_ctrl = world.provinces.controllers[adj_pi];
                if !adj_ctrl.is_none()
                    && adj_ctrl != country
                    && !is_friendly_staging_controller(world, country, adj_ctrl)
                {
                    foreign_ctrls.insert(adj_ctrl);
                }
            }
        }
    }

    for foreign in foreign_ctrls {
        let seg = compute_segment_peace(world, country, foreign);
        if seg.is_active() {
            front.segments.push(seg);
        }
    }

    front
}

fn compute_segment_peace(world: &World, country: CountryId, neighbor: CountryId) -> FrontSegment {
    let mut friendly: std::collections::HashSet<StateId> = std::collections::HashSet::new();
    let mut enemy: std::collections::HashSet<StateId> = std::collections::HashSet::new();

    for si in 0..world.states.count {
        if !is_friendly_staging_controller(world, country, world.states.controllers[si]) {
            continue;
        }
        let mut on_front = false;
        for &p in &world.states.provinces[si] {
            if !is_land_province(world, p) {
                continue;
            }
            let pi = p.0 as usize;
            if pi >= world.map.adjacencies.len() {
                continue;
            }
            for &adj_raw in &world.map.adjacencies[pi] {
                let adj = ProvinceId(adj_raw);
                if !is_land_province(world, adj) {
                    continue;
                }
                let adj_pi = adj.0 as usize;
                if adj_pi >= world.provinces.count {
                    continue;
                }
                let adj_ctrl = world.provinces.controllers[adj_pi];
                if adj_ctrl == neighbor {
                    on_front = true;
                    let adj_state = world.provinces.state_of[adj_pi];
                    if !adj_state.is_none() {
                        enemy.insert(adj_state);
                    }
                }
            }
        }
        if on_front {
            friendly.insert(StateId(si as u16));
        }
    }

    FrontSegment {
        owner: country,
        enemy: neighbor,
        friendly_states: friendly.into_iter().collect(),
        enemy_states: enemy.into_iter().collect(),
    }
}
