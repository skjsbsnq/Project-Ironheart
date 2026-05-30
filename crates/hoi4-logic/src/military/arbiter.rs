//! 陆战仲裁器：每 4 小时（HOURS_PER_ROUND）扫描所有处于"对立 controller"邻接关系
//! 的省份对，挑代表师跑一轮 [`battle::simulate`]。
//!
//! Phase 1.3 v1 简化策略：
//! - 一对 (province_a, province_b) 内最多挑一个进攻师 + 一个防御师（org × strength
//!   最高的）一对一交火
//! - 不实施战宽 / 地形修正 / CAS / 多师堆叠（留给 Phase 6 军事深度）
//! - 不实施占领翻转 controller（崩溃后师留在原省，下一轮再战或自然 org 恢复）
//! - 海省 / 湖省被过滤掉（按 `ProvinceType::Land`）
//! - "对立"判定：双方 controller 不为 NONE、不相同，且
//!   `world.diplomacy.at_war_with(ctrl_a, ctrl_b)`
//!
//! ## 调度
//! 由 `hoi4-app` 的 `military_hourly` adapter 在 hourly cadence 下统一驱动；本仲
//! 裁器内部用 `world.elapsed_hours % HOURS_PER_ROUND == 0` 自动 gating，避免每小
//! 时都跑（与海/空对齐：4 小时一轮）。
//!
//! ## 确定性
//! RNG seed 由 `(elapsed_hours, prov_a, prov_b)` 哈希得出，保证同一存档下每对每
//! 轮的战术抽签结果一致；[`battle::simulate`] 本身已是确定的（DeterministicRng）。

use hoi4_data::GameData;
use hoi4_map::ProvinceType;
use hoi4_state::{CountryId, DivisionId, ProvinceId, World};

use super::battle::{self, BattleSide, DeterministicRng};
use super::constants::HOURS_PER_ROUND;
use super::stats::DivisionStats;

/// 同省份单方最多上前线的师数（仿 vanilla "战宽" 概念，简化为固定值）。
/// 多余的师作为预备队（暂不实施 reinforcement，但不会被战斗系统忽略——
/// 它们仍占位，下一轮 4h tick 同样会进入对战的轮换池）。
const MAX_FRONTLINE_DIVISIONS: usize = 8;

use crate::economy::EconomyState;

/// 每 4 小时（HOURS_PER_ROUND）扫一次。其他小时直接 return。
/// P4: 分批处理——每小时只处理 1/4 的对立省份对（4h 一轮 = 一天全扫一次）。
pub fn tick_hourly(world: &mut World, data: &GameData, econ: Option<&mut EconomyState>) {
    if world.elapsed_hours % HOURS_PER_ROUND as u64 != 0 {
        return;
    }
    run_round(world, data, econ);
}

/// 强制跑一轮（测试 / 集成验证用）。生产代码请用 [`tick_hourly`]。
pub fn run_round(world: &mut World, data: &GameData, mut econ: Option<&mut EconomyState>) {
    // 第一遍：清掉上一轮的 in_combat 标记。本仲裁器是唯一陆军 in_combat 触发器，
    // 集中重置避免崩溃师永远卡在 in_combat=true。
    for i in 0..world.divisions.count {
        world.divisions.in_combat[i] = false;
    }

    // 收集对立省份对。我们用 raw province u16 做比较（map.adjacencies 用 u16 索引）。
    // 排序约束 a < b 防止 (A,B) 与 (B,A) 双计。
    let pairs = collect_opposing_pairs(world);
    if pairs.is_empty() {
        return;
    }
    let attack_edges = collect_attack_edges(world, &pairs);

    for (prov_a, prov_b, ctrl_a, ctrl_b) in pairs {
        let a_attacks_b = attack_edges.contains(&(prov_a.0, prov_b.0));
        let b_attacks_a = attack_edges.contains(&(prov_b.0, prov_a.0));
        if !a_attacks_b && !b_attacks_a {
            continue;
        }

        if a_attacks_b {
            run_pair(
                world,
                data,
                econ.as_deref_mut(),
                prov_a,
                prov_b,
                ctrl_a,
                ctrl_b,
            );
        } else {
            run_pair(
                world,
                data,
                econ.as_deref_mut(),
                prov_b,
                prov_a,
                ctrl_b,
                ctrl_a,
            );
        }
    }
}

/// 扫一遍 map.adjacencies，返回所有 `(prov_a, prov_b, ctrl_a, ctrl_b)` 对：
/// - 双方都是 Land province
/// - 双方 controller 都非 NONE 且不同
/// - 两 controller 在战争中
fn collect_opposing_pairs(world: &World) -> Vec<(ProvinceId, ProvinceId, CountryId, CountryId)> {
    use std::collections::HashSet;

    let map = &world.map;
    let prov_count = world.provinces.count;
    let mut out: Vec<(ProvinceId, ProvinceId, CountryId, CountryId)> = Vec::new();
    let mut seen: HashSet<(u16, u16)> = HashSet::new();

    for raw_a in world.prov_div_index.keys().copied() {
        if raw_a as usize >= map.adjacencies.len().min(prov_count) {
            continue;
        }
        let ctrl_a = world.provinces.controllers[raw_a as usize];
        if ctrl_a.is_none() {
            continue;
        }
        // 只考虑陆地省（避免给海/湖跑陆战）
        if !is_land_province(&map, raw_a) {
            continue;
        }

        for &raw_b in map.neighbors(raw_a) {
            if (raw_b as usize) >= prov_count {
                continue;
            }
            let key = if raw_a < raw_b {
                (raw_a, raw_b)
            } else {
                (raw_b, raw_a)
            };
            if !seen.insert(key) {
                continue;
            }
            if !is_land_province(&map, raw_b) {
                continue;
            }
            let ctrl_b = world.provinces.controllers[raw_b as usize];
            if ctrl_b.is_none() || ctrl_b == ctrl_a {
                continue;
            }
            if !world.diplomacy.at_war_with(ctrl_a, ctrl_b) {
                continue;
            }
            out.push((ProvinceId(raw_a), ProvinceId(raw_b), ctrl_a, ctrl_b));
        }
    }

    out
}

fn is_land_province(map: &hoi4_map::GameMap, raw_id: u16) -> bool {
    match map.get_province(raw_id) {
        Some(def) => matches!(def.province_type, ProvinceType::Land),
        None => false,
    }
}

fn collect_attack_edges(
    world: &mut World,
    pairs: &[(ProvinceId, ProvinceId, CountryId, CountryId)],
) -> std::collections::HashSet<(u16, u16)> {
    let mut frontier_provinces = std::collections::HashSet::new();
    for &(prov_a, prov_b, _, _) in pairs {
        frontier_provinces.insert(prov_a);
        frontier_provinces.insert(prov_b);
    }

    let mut edges = std::collections::HashSet::new();
    for source in frontier_provinces {
        let divs = world.divisions_in_province(source).to_vec();
        for di in divs {
            if di >= world.divisions.count || world.divisions.transport[di].is_some() {
                continue;
            }
            let owner = world.divisions.owners[di];
            if owner.is_none() || !division_can_fight(world, di) {
                continue;
            }
            let Some(dest) = world.divisions.destinations[di] else {
                continue;
            };
            let next = super::movement::next_step_cached(world, owner, source, dest);
            if next == source {
                continue;
            };
            let next_i = next.0 as usize;
            if next_i >= world.provinces.count {
                continue;
            }
            let next_ctrl = world.provinces.controllers[next_i];
            if !next_ctrl.is_none()
                && next_ctrl != owner
                && world.diplomacy.at_war_with(owner, next_ctrl)
            {
                edges.insert((source.0, next.0));
            }
        }
    }
    edges
}

fn division_can_fight(world: &World, di: usize) -> bool {
    let max_org = world.divisions.max_organisation[di].max(1e-6);
    let org_ratio = world.divisions.organisation[di] / max_org;
    org_ratio >= 0.05 && world.divisions.strength[di] >= 0.05
}

/// 运行一对省份对的一轮战斗。
///
/// **多师团参战**（仿 vanilla 战宽）：每方最多 [`MAX_FRONTLINE_DIVISIONS`] 个师同时上前线，
/// 后续作为预备队（暂不实施 reinforcement）。这样 25 个 ITA 师压 1 个 ETH 省时
/// 真的有 4 个师轮番打而不是只挑最强的一个，避免"几十个师挤一个目标却进展缓慢"的卡战。
fn run_pair(
    world: &mut World,
    data: &GameData,
    mut econ: Option<&mut EconomyState>,
    prov_a: ProvinceId,
    prov_b: ProvinceId,
    ctrl_a: CountryId,
    ctrl_b: CountryId,
) {
    let attackers = pick_top_divisions_in(world, prov_a, ctrl_a, MAX_FRONTLINE_DIVISIONS);
    let defenders = pick_top_divisions_in(world, prov_b, ctrl_b, MAX_FRONTLINE_DIVISIONS);
    if attackers.is_empty() || defenders.is_empty() {
        return;
    }

    // 公平配对：每方上 min(attackers, defenders) 个师 1v1 打。
    // 多余的师作为预备队不参战（不挨打也不输出）—— 仿 vanilla 战宽溢出。
    // 这样 ITA 4 vs ETH 23 → 实际只有 4v4 在打，ETH 多余的 19 守军不能"群殴"4 个 ITA。
    let n_pairs = attackers.len().min(defenders.len());
    if n_pairs == 0 {
        return;
    }

    // 累积每个 div 的损失，战斗结束后批量扣装备（避免 mut borrow 竞争）
    let mut losses: Vec<(DivisionId, f32)> = Vec::new();

    for k in 0..n_pairs {
        let div_a = attackers[k];
        let div_b = defenders[k];

        let Some(side_a) = build_battle_side(world, data, div_a, true) else {
            continue;
        };
        let Some(side_b) = build_battle_side(world, data, div_b, false) else {
            continue;
        };

        let str_a_before = side_a.strength;
        let str_b_before = side_b.strength;

        let seed = derive_seed(world.elapsed_hours, prov_a.0, prov_b.0)
            .wrapping_add(k as u32 * 0x9E37_79B1);
        let mut rng = DeterministicRng::new(seed);
        let outcome = battle::simulate(side_a, side_b, data, HOURS_PER_ROUND, &mut rng);

        if outcome.rounds_fought > 0 {
            set_in_combat(world, div_a, true);
            set_in_combat(world, div_b, true);
        }

        super::spawn::apply_battle_result(world, div_a, &outcome.attacker);
        super::spawn::apply_battle_result(world, div_b, &outcome.defender);

        let loss_a = (str_a_before - outcome.attacker.strength).max(0.0);
        let loss_b = (str_b_before - outcome.defender.strength).max(0.0);
        if loss_a > 0.0 {
            losses.push((div_a, loss_a));
        }
        if loss_b > 0.0 {
            losses.push((div_b, loss_b));
        }
    }

    // 装备损失批量扣除
    if let Some(ref mut econ) = econ {
        for (div, loss) in losses {
            crate::economy::stockpile::deduct_combat_losses(
                world,
                econ,
                data,
                div.0 as usize,
                loss,
            );
        }
    }
}

/// 在 `province` 内、属于 `owner` 的师中，挑 `strength × org_ratio` 最高的一个。
/// 在 `province` 内、属于 `owner` 的师中，按 `strength × org_ratio` 取前 N 个。
/// 已破碎（org<5%）/ 已清零（strength<5%）的师不参战。
/// 使用 World.prov_div_index 索引避免全扫描。
fn pick_top_divisions_in(
    world: &World,
    province: ProvinceId,
    owner: CountryId,
    n: usize,
) -> Vec<DivisionId> {
    let mut scored: Vec<(usize, f32)> = Vec::new();
    for &di in world.divisions_in_province(province) {
        if world.divisions.transport[di].is_some() {
            continue;
        }
        if world.divisions.owners[di] != owner {
            continue;
        }
        let max_org = world.divisions.max_organisation[di].max(1e-6);
        let org_ratio = world.divisions.organisation[di] / max_org;
        if org_ratio < 0.05 || world.divisions.strength[di] < 0.05 {
            continue;
        }
        let score = world.divisions.strength[di] * org_ratio;
        scored.push((di, score));
    }
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(n);
    scored
        .into_iter()
        .map(|(i, _)| DivisionId(i as u32))
        .collect()
}

/// 从一个 [`DivisionId`] 构造 [`BattleSide`]：先 `DivisionStats::aggregate`，
/// 再把当前 strength / org 覆盖进去。如果模板查不到则返回 None。
fn build_battle_side(
    world: &World,
    data: &GameData,
    div: DivisionId,
    is_attacker: bool,
) -> Option<BattleSide> {
    if div.is_none() {
        return None;
    }
    let i = div.0 as usize;
    if i >= world.divisions.count {
        return None;
    }
    let owner = world.divisions.owners[i];
    let tag = world.country_tag(owner)?.to_owned();
    let templates = data.division_templates.get(&tag)?;
    let tpl_idx = world.divisions.template_indices[i] as usize;
    let template = templates.get(tpl_idx)?;
    let mut stats = DivisionStats::aggregate(template, data);
    let modifier = china_war_idea_modifier(world, owner);
    stats.max_organisation *= modifier.org_mult;
    if let Some(modifier) = super::general::modifier_for_division(world, i) {
        if is_attacker {
            stats.soft_attack *= modifier.attack_mult;
            stats.hard_attack *= modifier.attack_mult;
            stats.breakthrough *= modifier.attack_mult;
        } else {
            stats.defense *= modifier.defense_mult;
        }
    }
    if is_attacker {
        stats.soft_attack *= modifier.attack_mult;
        stats.hard_attack *= modifier.attack_mult;
        stats.breakthrough *= modifier.attack_mult;
    } else {
        stats.defense *= modifier.defense_mult;
    }
    let mut side = BattleSide::from_stats(stats);
    side.strength = world.divisions.strength[i];
    side.organisation = (world.divisions.organisation[i] * modifier.org_mult)
        .min(side.stats.max_organisation.max(1.0));
    Some(side)
}

#[derive(Debug, Clone, Copy)]
struct ChinaWarIdeaModifier {
    attack_mult: f32,
    defense_mult: f32,
    org_mult: f32,
}

fn china_war_idea_modifier(world: &World, country: CountryId) -> ChinaWarIdeaModifier {
    let i = country.0 as usize;
    if i >= world.countries.ideas.len() {
        return ChinaWarIdeaModifier {
            attack_mult: 1.0,
            defense_mult: 1.0,
            org_mult: 1.0,
        };
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
        let occupied = japan_controlled_chinese_core_states(world, country);
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

    ChinaWarIdeaModifier {
        attack_mult: (1.0 + attack).clamp(0.65, 1.20),
        defense_mult: (1.0 + defense).clamp(0.75, 1.25),
        org_mult: (1.0 + org).clamp(0.75, 1.20),
    }
}

fn is_chinese_core_state(world: &World, state_idx: usize) -> bool {
    if state_idx >= world.states.cores.len() {
        return false;
    }
    world.states.cores[state_idx].iter().any(|&core| {
        world.country_tag(core).is_some_and(|tag| {
            matches!(
                tag,
                "CHI"
                    | "SND"
                    | "PRC"
                    | "SHX"
                    | "GXC"
                    | "GDC"
                    | "YUN"
                    | "XAJ"
                    | "SIC"
                    | "XSM"
                    | "SIK"
            )
        })
    })
}

fn japan_controlled_chinese_core_states(world: &World, country: CountryId) -> usize {
    if !matches!(world.country_tag(country), Some("JAP")) {
        return 0;
    }
    (0..world.states.count)
        .filter(|&si| {
            world.states.controllers[si] == country
                && world.states.owners[si] != country
                && is_chinese_core_state(world, si)
        })
        .count()
}

fn set_in_combat(world: &mut World, div: DivisionId, value: bool) {
    if div.is_none() {
        return;
    }
    let i = div.0 as usize;
    if i < world.divisions.count {
        world.divisions.in_combat[i] = value;
        if value {
            world.divisions.last_combat_hour[i] = world.elapsed_hours;
        }
    }
}

/// 由 `(elapsed_hours, prov_a, prov_b)` 派生 32-bit 种子（xorshift 输入）。
/// 保证同一对、同一时刻、同一存档的战术抽签结果可重现。
fn derive_seed(hour: u64, a: u16, b: u16) -> u32 {
    let h = hour as u32;
    let mut s = h ^ ((a as u32) << 16) ^ (b as u32);
    // 多搅一次降低对小数字的敏感（hour=0、a=0、b=0 等场景）
    s = s.wrapping_mul(0x9E37_79B1).wrapping_add(0xDEAD_BEEF);
    if s == 0 {
        0xCAFE_F00D
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hoi4_data::{Color, Country, CountryTag, DivisionTemplate, GameData, State, SubunitDef};
    use hoi4_map::{
        GameMap, Heightmap, ProvinceDefinition, ProvinceMap, ProvinceType, TerrainBitmap,
        TerrainCatalog,
    };
    use hoi4_state::diplomacy::War;
    use std::collections::{HashMap, HashSet};
    use std::sync::Arc;

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
        data.subunits.insert(
            "infantry".to_owned(),
            SubunitDef {
                key: "infantry".to_owned(),
                group: "infantry".to_owned(),
                types: vec!["infantry".to_owned()],
                combat_width: 2.0,
                max_strength: 1.0,
                max_organisation: 30.0,
                manpower: 1000,
                ..Default::default()
            },
        );
        let template = DivisionTemplate {
            name: "Infantry".to_owned(),
            country_tag: None,
            regiments: vec!["infantry".to_owned()],
            support: Vec::new(),
            division_names_group: None,
        };
        let ger = CountryTag::new("GER");
        let aus = CountryTag::new("AUS");
        for tag in [ger.clone(), aus.clone()] {
            data.countries.insert(
                tag.clone(),
                Country {
                    tag: tag.clone(),
                    color: Color { r: 1, g: 1, b: 1 },
                    graphical_culture: "western_european_gfx".to_owned(),
                    capital: if tag.as_str() == "GER" { 1 } else { 2 },
                    ruling_party: "neutrality".to_owned(),
                    technologies: Vec::new(),
                },
            );
            data.division_templates
                .insert(tag.as_str().to_owned(), vec![template.clone()]);
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
            war_join_policies: HashMap::new(),
        };
        world
            .diplomacy
            .wars
            .insert(world.diplomacy.next_war_id, war);
        world.diplomacy.next_war_id += 1;
        world.countries.at_war[a.0 as usize] = true;
        world.countries.at_war[b.0 as usize] = true;
    }

    fn add_opposing_divisions(world: &mut World) -> (usize, usize, CountryId, CountryId) {
        let ger = world.country("GER").unwrap();
        let aus = world.country("AUS").unwrap();
        make_war(world, ger, aus);
        let ger_div = world
            .divisions
            .push(ger, ProvinceId(1), 0, 30.0, 1.0, "GER div".to_owned())
            as usize;
        let aus_div = world
            .divisions
            .push(aus, ProvinceId(2), 0, 30.0, 1.0, "AUS div".to_owned())
            as usize;
        world.rebuild_province_div_index();
        (ger_div, aus_div, ger, aus)
    }

    #[test]
    fn seed_is_stable_and_distinct() {
        let s1 = derive_seed(1, 100, 200);
        let s2 = derive_seed(1, 100, 200);
        assert_eq!(s1, s2);
        let s3 = derive_seed(2, 100, 200);
        assert_ne!(s1, s3);
        let s4 = derive_seed(1, 200, 100);
        assert_ne!(s1, s4);
    }

    #[test]
    fn seed_never_zero() {
        // 全零输入产生的 seed 仍非 0（DeterministicRng 不喜欢 0）
        let s = derive_seed(0, 0, 0);
        assert_ne!(s, 0);
    }

    #[test]
    fn adjacent_defenders_do_not_auto_start_combat() {
        let mut world = synthetic_world();
        let (ger_div, aus_div, _, _) = add_opposing_divisions(&mut world);
        let data = world.data.clone();

        run_round(&mut world, data.as_ref(), None);

        assert!(!world.divisions.in_combat[ger_div]);
        assert!(!world.divisions.in_combat[aus_div]);
        assert_eq!(world.divisions.organisation[ger_div], 30.0);
        assert_eq!(world.divisions.organisation[aus_div], 30.0);
    }

    #[test]
    fn attack_destination_starts_combat() {
        let mut world = synthetic_world();
        let (ger_div, aus_div, _, _) = add_opposing_divisions(&mut world);
        world.divisions.destinations[ger_div] = Some(ProvinceId(2));
        let data = world.data.clone();

        run_round(&mut world, data.as_ref(), None);

        assert!(world.divisions.in_combat[ger_div]);
        assert!(world.divisions.in_combat[aus_div]);
        assert!(world.divisions.organisation[ger_div] < 30.0);
        assert!(world.divisions.organisation[aus_div] < 30.0);
    }
}
