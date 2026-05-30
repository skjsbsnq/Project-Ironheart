//! 陆军战术决策：进攻 / 防御 / 包围。
//!
//! 输入：[`crate::frontline::FrontLine`] + 双方师 / 工业实力。
//! 输出：每段前线的 [`GroundPosture`]（攻 / 守 / 拖延）与可选的包围目标列表。
//!
//! ## 评分模型
//! - 力量比 ratio = my_combat_power / enemy_combat_power（仅前线沿线参战的师）
//! - ratio >= [`OFFENSIVE_RATIO`] → Attack
//! - ratio <= [`RETREAT_RATIO`] → Retreat（实质等同于 Defend，但放低优先级）
//! - 中间 → Defend
//!
//! 包围检测：
//! - 把所有敌方控制的州按 "被我方控制的邻接州比例" 排序
//! - 比例 ≥ [`ENCIRCLEMENT_RATIO`] 即视为可被包围目标
//!
//! ## 没有的部分（延后）
//! - 调度具体师从 A → B：需要 movement / supply / order 系统
//! - 战线"宽度"匹配：当前没有 plan 系统
//! - 装甲突破链：需要 mobility 数据
//!
//! 本模块只输出"决心"（attack / defend / encircle），不下达具体行动指令——
//! 那部分要在 plan 系统就绪后再补。

use hoi4_state::{CountryId, ProvinceId, StateId, World};

use crate::china_theater;
use crate::constants::*;
use crate::frontline::{frontline_state_count_of, neighbor_ratio_controlled_by, FrontLine};
use crate::profile::AiProfile;

/// 一段前线的总体姿态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroundPosture {
    /// 全面进攻：力量优势 + 高士气
    Attack,
    /// 防御：力量均势或略劣
    Defend,
    /// 撤退 / 让步：力量明显劣势
    Retreat,
    /// 完全没有可战之兵：仅维持战线警戒
    Hold,
}

/// 一段前线的战术决策。
#[derive(Debug, Clone)]
pub struct FrontDecision {
    pub enemy: CountryId,
    pub posture: GroundPosture,
    /// 我方该前线的总战斗力（org × strength × max_strength 加权）
    pub my_power: f32,
    /// 敌方该前线的总战斗力（推算自师数 × 经验）
    pub enemy_power: f32,
    pub ratio: f32,
    /// 该前线上可优先尝试包围 / 突击的敌方州（按"被我们控制的邻接比例"排序）
    pub encirclement_targets: Vec<EncirclementTarget>,
    /// 该前线上有被包围风险的我方州（"被敌人控制的邻接比例 ≥ 阈值"）
    pub at_risk_states: Vec<EncirclementTarget>,
    /// 局部战术 sector 评分，按最佳突破机会排序。
    pub sectors: Vec<FrontSector>,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FrontSector {
    pub friendly_state: StateId,
    pub enemy_state: StateId,
    pub attack_from: Vec<ProvinceId>,
    pub target_provinces: Vec<ProvinceId>,
    pub friendly_power: f32,
    pub enemy_power: f32,
    pub ratio: f32,
    pub attack_width: u32,
    pub supply_score: f32,
    pub terrain_penalty: f32,
    pub value_score: f32,
    pub score: f32,
    pub reason: String,
}

/// 包围目标 / 风险州 — 共用同一结构。
#[derive(Debug, Clone, PartialEq)]
pub struct EncirclementTarget {
    pub state: StateId,
    /// 被对方控制的邻接州数
    pub controlled_neighbors: u32,
    /// 总邻接州数
    pub total_neighbors: u32,
    /// 比例 = controlled / total
    pub ratio: f32,
}

/// 整个国家的陆军战术评估结果。
#[derive(Debug, Clone)]
pub struct GroundEvaluation {
    pub country: CountryId,
    pub decisions: Vec<FrontDecision>,
}

impl GroundEvaluation {
    pub fn empty(country: CountryId) -> Self {
        Self {
            country,
            decisions: Vec::new(),
        }
    }

    pub fn against(&self, enemy: CountryId) -> Option<&FrontDecision> {
        self.decisions.iter().find(|d| d.enemy == enemy)
    }
}

/// 计算某国师的综合战斗力（粗估）。
fn country_division_power(world: &World, country: CountryId) -> f32 {
    let mut sum = 0.0f32;
    if let Some(indices) = country_division_indices(world, country) {
        for &di in indices {
            sum += division_power(world, di);
        }
    } else {
        for di in 0..world.divisions.count {
            if world.divisions.owners[di] != country {
                continue;
            }
            sum += division_power(world, di);
        }
    }
    sum
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

fn division_power(world: &World, div_idx: usize) -> f32 {
    let owner = world.divisions.owners[div_idx];
    let strength = world.divisions.strength[div_idx];
    let max_str = world.divisions.max_strength[div_idx].max(1.0);
    let max_org = world.divisions.max_organisation[div_idx].max(1.0);
    let org_ratio = (world.divisions.organisation[div_idx] / max_org).clamp(0.0, 1.0);
    let xp = world.divisions.experience[div_idx];
    let xp_mod = 1.0 + (xp / 900.0).clamp(0.0, 1.0) * 0.30;
    strength * org_ratio * max_str * xp_mod * country_ground_power_factor(world, owner)
}

fn country_ground_power_factor(world: &World, country: CountryId) -> f32 {
    let i = country.0 as usize;
    if i >= world.countries.ideas.len() {
        return 1.0;
    }

    let mut attack = 0.0f32;
    let mut defense = 0.0f32;
    let mut org = 0.0f32;
    for idea in &world.countries.ideas[i] {
        match idea.as_str() {
            "chi_united_front_coordination_difficulties" => {
                attack -= 0.12;
                org -= 0.06;
            }
            "chi_regional_command_autonomy" => {
                attack -= 0.08;
                org -= 0.04;
            }
            "chi_theater_command_delay" => {
                attack -= 0.06;
                defense -= 0.03;
            }
            "chi_national_war_mobilization" => {
                defense += 0.08;
                org += 0.04;
            }
            "chi_protracted_war_policy" => {
                defense += 0.10;
                org += 0.06;
            }
            "jap_continental_offensive_momentum" => {
                attack += 0.08;
                org += 0.03;
            }
            "jap_north_china_expeditionary_expansion" => {
                attack += 0.04;
            }
            "jap_china_incident_expansion" => {
                org += 0.02;
            }
            "jap_occupation_security_pressure" => {
                attack -= 0.04;
                org -= 0.02;
            }
            "jap_extended_continental_supply_lines" => {
                org -= 0.05;
            }
            "jap_forces_dispersed_in_china" => {
                attack -= 0.06;
                defense -= 0.02;
            }
            _ => {}
        }
    }

    if matches!(world.country_tag(country), Some("JAP")) {
        let occupied = china_theater::japan_controlled_chinese_core_states(world, country);
        if occupied >= 5 {
            attack -= 0.03;
            org -= 0.02;
        }
        if occupied >= 10 {
            attack -= 0.05;
            org -= 0.04;
        }
        if occupied >= 18 {
            attack -= 0.08;
            defense -= 0.03;
            org -= 0.04;
        }
        if world.date.year >= 1939 {
            attack -= 0.04;
            org -= 0.03;
        }
    }

    (1.0 + attack * 0.45 + defense * 0.35 + org * 0.50).clamp(0.55, 1.25)
}

pub fn evaluate_front_sectors(
    world: &World,
    country: CountryId,
    seg: &crate::frontline::FrontSegment,
) -> Vec<FrontSector> {
    let enemy_set: std::collections::HashSet<StateId> = seg.enemy_states.iter().copied().collect();
    let mut by_pair: std::collections::HashMap<
        (StateId, StateId),
        (Vec<ProvinceId>, Vec<ProvinceId>),
    > = std::collections::HashMap::new();

    for &friendly_state in &seg.friendly_states {
        let si = friendly_state.0 as usize;
        if si >= world.states.count {
            continue;
        }
        for &p in &world.states.provinces[si] {
            let pi = p.0 as usize;
            if pi >= world.provinces.count || pi >= world.map.adjacencies.len() {
                continue;
            }
            if !can_stage_from_province(world, country, p) || !is_land(world, p) {
                continue;
            }
            for &adj_raw in &world.map.adjacencies[pi] {
                let target = ProvinceId(adj_raw);
                let ti = target.0 as usize;
                if ti >= world.provinces.count || !is_land(world, target) {
                    continue;
                }
                let target_ctrl = world.provinces.controllers[ti];
                if target_ctrl != seg.enemy {
                    continue;
                }
                let enemy_state = world.provinces.state_of[ti];
                if !enemy_set.contains(&enemy_state) {
                    continue;
                }
                let entry = by_pair.entry((friendly_state, enemy_state)).or_default();
                if !entry.0.contains(&p) {
                    entry.0.push(p);
                }
                if !entry.1.contains(&target) {
                    entry.1.push(target);
                }
            }
        }
    }

    let mut sectors = Vec::new();
    for ((friendly_state, enemy_state), (attack_from, target_provinces)) in by_pair {
        let friendly_power = state_division_power(world, country, friendly_state);
        let enemy_power = state_division_power(world, seg.enemy, enemy_state)
            + target_province_enemy_power(world, country, &target_provinces) * 0.35;
        let ratio = if enemy_power > 0.0 {
            friendly_power / enemy_power
        } else if friendly_power > 0.0 {
            8.0
        } else {
            0.0
        };
        let attack_width = attack_from.len() as u32;
        let supply_score = average_supply(world, &attack_from);
        let terrain_penalty = average_terrain_penalty(world, &target_provinces);
        let value_score = state_value_score(world, enemy_state)
            + japan_china_area_bonus(world, country, enemy_state);
        let score =
            ratio * 45.0 + attack_width.min(4) as f32 * 8.0 + supply_score * 0.35 + value_score
                - terrain_penalty
                - enemy_power.sqrt() * 0.08;
        let reason = format!(
            "sector {:?}->{:?} ratio={:.2} width={} supply={:.0} terrain_pen={:.1} value={:.1} score={:.1}",
            friendly_state,
            enemy_state,
            ratio,
            attack_width,
            supply_score,
            terrain_penalty,
            value_score,
            score
        );
        sectors.push(FrontSector {
            friendly_state,
            enemy_state,
            attack_from,
            target_provinces,
            friendly_power,
            enemy_power,
            ratio,
            attack_width,
            supply_score,
            terrain_penalty,
            value_score,
            score,
            reason,
        });
    }

    sectors.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    sectors
}

fn is_land(world: &World, p: ProvinceId) -> bool {
    world
        .map
        .get_province(p.0)
        .is_some_and(|def| matches!(def.province_type, hoi4_map::ProvinceType::Land))
}

fn can_stage_from_province(world: &World, country: CountryId, p: ProvinceId) -> bool {
    let pi = p.0 as usize;
    if pi >= world.provinces.count {
        return false;
    }
    let controller = world.provinces.controllers[pi];
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

fn state_division_power(world: &World, country: CountryId, state: StateId) -> f32 {
    let mut sum = 0.0;
    if let Some(indices) = country_division_indices(world, country) {
        for &di in indices {
            let loc = world.divisions.locations[di].0 as usize;
            if loc < world.provinces.count && world.provinces.state_of[loc] == state {
                sum += division_power(world, di);
            }
        }
    } else {
        for di in 0..world.divisions.count {
            if world.divisions.owners[di] != country {
                continue;
            }
            let loc = world.divisions.locations[di].0 as usize;
            if loc < world.provinces.count && world.provinces.state_of[loc] == state {
                sum += division_power(world, di);
            }
        }
    }
    sum
}

fn target_province_enemy_power(world: &World, country: CountryId, targets: &[ProvinceId]) -> f32 {
    if !world.prov_div_index.is_empty() {
        let mut sum = 0.0;
        for &target in targets {
            for &di in world.divisions_in_province(target) {
                let owner = world.divisions.owners[di];
                if owner == country || !world.diplomacy.at_war_with(country, owner) {
                    continue;
                }
                sum += division_power(world, di);
            }
        }
        return sum;
    }

    let target_set: std::collections::HashSet<u16> = targets.iter().map(|p| p.0).collect();
    let mut sum = 0.0;
    if world.runtime_country_indexes_valid {
        for ci in 0..world.countries.count {
            let owner = CountryId(ci as u16);
            if owner == country || !world.diplomacy.at_war_with(country, owner) {
                continue;
            }
            if let Some(indices) = world.country_division_index.get(ci) {
                for &di in indices {
                    if target_set.contains(&world.divisions.locations[di].0) {
                        sum += division_power(world, di);
                    }
                }
            }
        }
    } else {
        for di in 0..world.divisions.count {
            let owner = world.divisions.owners[di];
            if owner == country || !world.diplomacy.at_war_with(country, owner) {
                continue;
            }
            if target_set.contains(&world.divisions.locations[di].0) {
                sum += division_power(world, di);
            }
        }
    }
    sum
}

fn average_supply(world: &World, provinces: &[ProvinceId]) -> f32 {
    if provinces.is_empty() {
        return 0.0;
    }
    let mut sum = 0.0;
    let mut count = 0.0;
    for &p in provinces {
        let pi = p.0 as usize;
        if pi < world.provinces.supply.len() {
            sum += world.provinces.supply[pi];
            count += 1.0;
        }
    }
    if count > 0.0 {
        sum / count
    } else {
        0.0
    }
}

fn average_terrain_penalty(world: &World, provinces: &[ProvinceId]) -> f32 {
    if provinces.is_empty() {
        return 0.0;
    }
    let mut sum = 0.0;
    let mut count = 0.0;
    for &p in provinces {
        if let Some(def) = world.map.get_province(p.0) {
            sum += terrain_penalty(&def.terrain);
            count += 1.0;
        }
    }
    if count > 0.0 {
        sum / count
    } else {
        0.0
    }
}

fn terrain_penalty(terrain: &str) -> f32 {
    match terrain {
        "mountain" => 24.0,
        "hills" => 14.0,
        "forest" => 10.0,
        "jungle" => 18.0,
        "marsh" => 20.0,
        "urban" => 16.0,
        "desert" => 8.0,
        _ => 0.0,
    }
}

fn state_value_score(world: &World, state: StateId) -> f32 {
    let si = state.0 as usize;
    let vp = if si < world.data.states.len() {
        world.data.states[si]
            .victory_points
            .iter()
            .map(|(_, v)| *v as f32)
            .sum::<f32>()
    } else {
        0.0
    };
    let factories = world.state_building_levels(state) as f32;
    vp * 1.8 + factories * 1.2
}

fn japan_china_area_bonus(world: &World, country: CountryId, state: StateId) -> f32 {
    china_theater::state_priority_bonus(world, country, state)
}

/// 评估某国家全部前线，返回每段前线的姿态决策。
pub fn evaluate_ground(
    world: &World,
    country: CountryId,
    front: &FrontLine,
    personality: &AiProfile,
) -> GroundEvaluation {
    let mut eval = GroundEvaluation::empty(country);
    if !front.has_front() {
        return eval;
    }

    let my_total_power = country_division_power(world, country);
    // 缓存敌方总前线州数，避免同一 evaluate 中对同一敌国重复扫描。
    let mut enemy_frontline_cache: std::collections::HashMap<CountryId, usize> =
        std::collections::HashMap::new();

    for seg in &front.segments {
        let enemy_total_power = country_division_power(world, seg.enemy);
        // 该段前线上分摊到的实际力量 ≈ 总力 × (前线州 / 该国前线州总数)
        // 但本国可能与多敌国交战，因此我们简化为"按段前线长度比"分摊。
        let my_share = if front.frontline_state_count() > 0 {
            seg.friendly_states.len() as f32 / front.frontline_state_count() as f32
        } else {
            1.0
        };
        let my_power = my_total_power * my_share.max(0.0).min(1.0);
        // 敌方对此段的投入 — 用敌方自己的前线占比估算（不对称）：
        //   enemy_share = enemy_states_in_this_segment / enemy_total_frontline_states
        // 这反映了敌方需要把兵力分散到所有交战国的事实，避免"敌方完全镜像我方分摊"
        // 的对称偏差。若敌方只与我方交战（且无内乱前线），share 自然趋近 1.0。
        let enemy_total_frontline_states = *enemy_frontline_cache
            .entry(seg.enemy)
            .or_insert_with(|| frontline_state_count_of(world, seg.enemy));
        let enemy_share = if enemy_total_frontline_states > 0 {
            seg.enemy_states.len() as f32 / enemy_total_frontline_states as f32
        } else {
            1.0
        };
        let enemy_power = enemy_total_power * enemy_share.clamp(0.0, 1.0);

        let ratio = if enemy_power > 0.0 {
            my_power / enemy_power
        } else if my_power > 0.0 {
            10.0
        } else {
            0.0
        };

        let sectors = evaluate_front_sectors(world, country, seg);
        let best_sector_ratio = sectors.first().map(|s| s.ratio).unwrap_or(ratio);
        let best_sector_score = sectors.first().map(|s| s.score).unwrap_or(0.0);

        // 包围目标：找敌方前线州中，被我方控制邻州比例最高的
        let mut targets: Vec<EncirclementTarget> = seg
            .enemy_states
            .iter()
            .map(|&s| {
                let (ctrl, total) = neighbor_ratio_controlled_by(world, s, country);
                let r = if total > 0 {
                    ctrl as f32 / total as f32
                } else {
                    0.0
                };
                EncirclementTarget {
                    state: s,
                    controlled_neighbors: ctrl,
                    total_neighbors: total,
                    ratio: r,
                }
            })
            .filter(|t| t.ratio >= ENCIRCLEMENT_RATIO)
            .collect();
        targets.sort_by(|a, b| {
            b.ratio
                .partial_cmp(&a.ratio)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // 风险州：我方前线州被敌方包围的
        let mut at_risk: Vec<EncirclementTarget> = seg
            .friendly_states
            .iter()
            .map(|&s| {
                let (ctrl, total) = neighbor_ratio_controlled_by(world, s, seg.enemy);
                let r = if total > 0 {
                    ctrl as f32 / total as f32
                } else {
                    0.0
                };
                EncirclementTarget {
                    state: s,
                    controlled_neighbors: ctrl,
                    total_neighbors: total,
                    ratio: r,
                }
            })
            .filter(|t| t.ratio >= AT_RISK_RATIO)
            .collect();
        at_risk.sort_by(|a, b| {
            b.ratio
                .partial_cmp(&a.ratio)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // 决定姿态
        let mut aggressive_threshold = OFFENSIVE_RATIO * (1.5 - personality.aggression);
        if china_theater::is_japan_china_war_active(world, country)
            && china_theater::is_china_war_enemy(world, seg.enemy)
        {
            aggressive_threshold *=
                china_theater::attack_threshold_multiplier(world, country, seg.enemy);
        }
        let posture = if my_power < 1.0 && enemy_power < 1.0 {
            GroundPosture::Hold
        } else if !targets.is_empty() && best_sector_ratio >= aggressive_threshold * 0.72 {
            // 有可包围目标 + 力量不算大劣 → 进攻
            GroundPosture::Attack
        } else if best_sector_score >= 58.0 && best_sector_ratio >= RETREAT_RATIO {
            GroundPosture::Attack
        } else if ratio >= aggressive_threshold {
            GroundPosture::Attack
        } else if ratio >= RETREAT_RATIO {
            // ratio ∈ [RETREAT_RATIO, aggressive_threshold) → 防守（兵力均衡或轻微劣势）
            GroundPosture::Defend
        } else {
            GroundPosture::Retreat
        };

        let reason = format!(
            "ratio={:.2} my_pow={:.0} enemy_pow={:.0} best_sector={:.2}/{:.1} sectors={} encircle={} at_risk={}",
            ratio,
            my_power,
            enemy_power,
            best_sector_ratio,
            best_sector_score,
            sectors.len(),
            targets.len(),
            at_risk.len(),
        );

        eval.decisions.push(FrontDecision {
            enemy: seg.enemy,
            posture,
            my_power,
            enemy_power,
            ratio,
            encirclement_targets: targets,
            at_risk_states: at_risk,
            sectors,
            reason,
        });
    }

    eval
}

/// 单独探测：针对全图，找出该国家*所有*被敌人控制邻接比例 ≥ 阈值的州。
/// 用于"主动出击"评估而不依赖现有前线（例如计划登陆/空降）。
pub fn detect_encirclement_opportunities(
    world: &World,
    country: CountryId,
    threshold: f32,
) -> Vec<EncirclementTarget> {
    let mut out = Vec::new();
    for si in 0..world.states.count {
        let controller = world.states.controllers[si];
        if controller == country || controller.is_none() {
            continue;
        }
        // 仅对交战敌国感兴趣
        if !world.diplomacy.at_war_with(country, controller) {
            continue;
        }
        let s = StateId(si as u16);
        let (ctrl, total) = neighbor_ratio_controlled_by(world, s, country);
        if total == 0 {
            continue;
        }
        let r = ctrl as f32 / total as f32;
        if r >= threshold {
            out.push(EncirclementTarget {
                state: s,
                controlled_neighbors: ctrl,
                total_neighbors: total,
                ratio: r,
            });
        }
    }
    out.sort_by(|a, b| {
        b.ratio
            .partial_cmp(&a.ratio)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    out
}

/// 借助 [`evaluate_ground`] 的结果，给出"摘要式"信息（用于 orchestrator 写报告）。
pub fn summarise(eval: &GroundEvaluation, world: &World) -> String {
    if eval.decisions.is_empty() {
        return "no front".to_string();
    }
    let mut parts = Vec::new();
    for d in &eval.decisions {
        let enemy_tag = world
            .country_tag(d.enemy)
            .map(|s| s.to_owned())
            .unwrap_or_else(|| format!("c{}", d.enemy.0));
        parts.push(format!("{}={:?}({:.2})", enemy_tag, d.posture, d.ratio));
    }
    parts.join(" | ")
}

/// 该决策摘要：返回 (number_of_attack_fronts, number_of_defend_fronts, encircle_count)。
pub fn decision_summary(eval: &GroundEvaluation) -> (usize, usize, usize) {
    let mut atk = 0;
    let mut def = 0;
    let mut enc = 0;
    for d in &eval.decisions {
        match d.posture {
            GroundPosture::Attack => atk += 1,
            GroundPosture::Defend | GroundPosture::Hold => def += 1,
            GroundPosture::Retreat => def += 1,
        }
        enc += d.encirclement_targets.len();
    }
    (atk, def, enc)
}
