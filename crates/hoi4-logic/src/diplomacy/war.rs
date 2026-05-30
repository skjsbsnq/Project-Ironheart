//! 宣战 / 白和 / 投降。
//!
//! 简化模型：
//! - 宣战必须有至少一个 justified wargoal（target == defender）
//! - 宣战瞬间：把 attacker 阵营全部成员加入 attackers；defender 阵营成员加入 defenders
//! - 白和：双方协议结束战争，无领土转移
//! - 投降：根据首都、胜利点和州控制比例判断；触发后该国从 war 中移出，可能成为占领国的傀儡

use std::collections::{HashMap, HashSet};

use hoi4_state::{CountryId, War, WarJoinPolicy, WarSide, Wargoal, World};

use super::constants::TENSION_PER_DECLARATION;
use super::wargoal::drain_justified_into;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WarError {
    BadCountry,
    /// attacker 已与 defender 在交战
    AlreadyAtWar,
    /// 没有任何 justified wargoal 指向该 defender
    NoJustifiedWargoal,
    /// attacker == defender
    SelfWar,
}

/// Scripted war creation used by historical events and situations.
///
/// This intentionally skips wargoal validation, but preserves the same faction
/// propagation and `at_war` bookkeeping as normal declarations.
pub fn force_declare_war(
    world: &mut World,
    attacker: CountryId,
    defender: CountryId,
) -> Result<u32, WarError> {
    if attacker == defender {
        return Err(WarError::SelfWar);
    }
    if attacker.is_none() || attacker.0 as usize >= world.countries.count {
        return Err(WarError::BadCountry);
    }
    if defender.is_none() || defender.0 as usize >= world.countries.count {
        return Err(WarError::BadCountry);
    }
    if world.diplomacy.at_war_with(attacker, defender) {
        return Err(WarError::AlreadyAtWar);
    }

    let (attackers, defenders) = war_side_members(world, attacker, defender);
    let war_id = insert_war(world, attacker, defender, attackers, defenders, vec![]);
    world.diplomacy.world_tension = (world.diplomacy.world_tension + TENSION_PER_DECLARATION)
        .min(super::constants::TENSION_CAP);
    Ok(war_id)
}

/// Adds a scripted participant to the side of `war_leader` in that leader's war.
pub fn add_war_participant(
    world: &mut World,
    war_leader: CountryId,
    participant: CountryId,
) -> Result<(), WarError> {
    if war_leader.is_none()
        || war_leader.0 as usize >= world.countries.count
        || participant.is_none()
        || participant.0 as usize >= world.countries.count
    {
        return Err(WarError::BadCountry);
    }
    let Some((_war_id, war)) =
        world.diplomacy.wars.iter_mut().find(|(_, war)| {
            war.primary_attacker == war_leader || war.primary_defender == war_leader
        })
    else {
        return Err(WarError::BadCountry);
    };
    if war.attackers.contains(&war_leader) {
        war.attackers.insert(participant);
    } else {
        war.defenders.insert(participant);
    }
    world.countries.at_war[participant.0 as usize] = true;
    Ok(())
}

/// 宣战。
///
/// - 把 attacker 全部 justified 且 target=defender 的 wargoal 转入新 War
/// - 阵营连带：双方阵营成员全部入战
/// - 世界紧张度 += [`TENSION_PER_DECLARATION`]
///
/// 返回新建的 war_id。
pub fn declare_war(
    world: &mut World,
    attacker: CountryId,
    defender: CountryId,
) -> Result<u32, WarError> {
    if attacker == defender {
        return Err(WarError::SelfWar);
    }
    if attacker.is_none() || attacker.0 as usize >= world.countries.count {
        return Err(WarError::BadCountry);
    }
    if defender.is_none() || defender.0 as usize >= world.countries.count {
        return Err(WarError::BadCountry);
    }
    if world.diplomacy.at_war_with(attacker, defender) {
        return Err(WarError::AlreadyAtWar);
    }

    // 找到指向该 defender 的 justified wargoal
    let mut transferred: Vec<Wargoal> = Vec::new();
    {
        // 临时 drain，过滤后只把 target=defender 的留在 transferred；其它 justified 的留回 pending
        let mut justified_all: Vec<Wargoal> = Vec::new();
        drain_justified_into(world, attacker, &mut justified_all);
        for wg in justified_all {
            if wg.target == defender {
                transferred.push(wg);
            } else {
                // 留作下次用
                world
                    .diplomacy
                    .pending_wargoals
                    .entry(attacker)
                    .or_default()
                    .push(wg);
            }
        }
    }
    if transferred.is_empty() {
        return Err(WarError::NoJustifiedWargoal);
    }

    let (attackers, defenders) = war_side_members(world, attacker, defender);
    let war_id = insert_war(world, attacker, defender, attackers, defenders, transferred);

    // 紧张度
    let cap = super::constants::TENSION_CAP;
    world.diplomacy.world_tension =
        (world.diplomacy.world_tension + TENSION_PER_DECLARATION).min(cap);

    Ok(war_id)
}

fn war_side_members(
    world: &World,
    attacker: CountryId,
    defender: CountryId,
) -> (HashSet<CountryId>, HashSet<CountryId>) {
    let mut attackers: HashSet<CountryId> = HashSet::new();
    let mut defenders: HashSet<CountryId> = HashSet::new();
    attackers.insert(attacker);
    defenders.insert(defender);
    if let Some(fid) = world.diplomacy.faction_of(attacker) {
        if let Some(f) = world.diplomacy.faction(fid) {
            attackers.extend(f.members.iter().copied());
        }
    }
    if let Some(fid) = world.diplomacy.faction_of(defender) {
        if let Some(f) = world.diplomacy.faction(fid) {
            defenders.extend(f.members.iter().copied());
        }
    }
    (attackers, defenders)
}

fn insert_war(
    world: &mut World,
    attacker: CountryId,
    defender: CountryId,
    attackers: HashSet<CountryId>,
    defenders: HashSet<CountryId>,
    attacker_wargoals: Vec<Wargoal>,
) -> u32 {
    let war_id = world.diplomacy.next_war_id;
    world.diplomacy.next_war_id += 1;
    world.diplomacy.wars.insert(
        war_id,
        War {
            id: war_id,
            primary_attacker: attacker,
            primary_defender: defender,
            attackers: attackers.clone(),
            defenders: defenders.clone(),
            started_at_hour: world.elapsed_hours,
            attacker_war_score: 0.0,
            defender_war_score: 0.0,
            attacker_wargoals,
            defender_wargoals: vec![],
            war_join_policies: HashMap::new(),
        },
    );
    for &c in attackers.iter().chain(defenders.iter()) {
        if (c.0 as usize) < world.countries.count {
            world.countries.at_war[c.0 as usize] = true;
        }
    }
    war_id
}

/// 白和：双方同意结束战争，没有领土变更。任一方都可以发起；这里直接清除战争实体。
pub fn white_peace(world: &mut World, war_id: u32) -> Result<(), WarError> {
    let war = world
        .diplomacy
        .wars
        .remove(&war_id)
        .ok_or(WarError::BadCountry)?; // 简单化：不存在视为 BadCountry

    // 重置 at_war 标志：仅当国家不再有任何战争时才置 false
    let participants: Vec<CountryId> = war
        .attackers
        .iter()
        .chain(war.defenders.iter())
        .copied()
        .collect();
    for c in participants {
        if c.0 as usize >= world.countries.count {
            continue;
        }
        let still_at_war = world.diplomacy.is_at_war(c);
        world.countries.at_war[c.0 as usize] = still_at_war;
    }
    Ok(())
}

/// 检查某国是否已"投降"。
///
/// 先看首都和胜利点；没有胜利点数据时，再退回到州控制比例。
/// 这样避免小国或地方势力因为丢掉外围州就被每日清退。
pub fn is_capitulated(world: &World, country: CountryId) -> bool {
    if country.is_none() {
        return false;
    }
    let mut had_state = false;
    let mut owned_states = 0usize;
    let mut controlled_states = 0usize;
    let mut total_vp = 0u32;
    let mut controlled_vp = 0u32;
    let capital = world
        .countries
        .capitals
        .get(country.0 as usize)
        .copied()
        .unwrap_or(hoi4_state::StateId::NONE);
    let mut capital_owned = false;
    let mut capital_controlled = false;

    for i in 0..world.states.count {
        if world.states.owners[i] != country {
            continue;
        }
        had_state = true;
        owned_states += 1;
        let controls_state = world.states.controllers[i] == country;
        if controls_state {
            controlled_states += 1;
        }
        if capital.0 as usize == i {
            capital_owned = true;
            capital_controlled = controls_state;
        }
        if let Some(state_def) = world.data.states.get(i) {
            for &(_, vp) in &state_def.victory_points {
                let vp = vp as u32;
                total_vp += vp;
                if controls_state {
                    controlled_vp += vp;
                }
            }
        }
    }
    // 没有任何 state 也算投降（已被吞并）
    if !had_state {
        return world.diplomacy.annexed_countries.contains(&country);
    }

    if capital_owned && capital_controlled {
        return false;
    }

    if total_vp > 0 {
        return controlled_vp == 0 || (controlled_vp as f32 / total_vp as f32) < 0.20;
    }

    controlled_states == 0 || (controlled_states as f32 / owned_states as f32) < 0.20
}

/// P0.11：设置某国在某场战争中的参战策略。
///
/// 用于中日战争等场景：军阀加入统一战线阵营但应延迟参战，
/// 由后续事件或脚本显式触发入战。
pub fn set_war_join_policy(
    world: &mut World,
    war_id: u32,
    country: CountryId,
    policy: WarJoinPolicy,
) -> Result<(), WarError> {
    let war = world
        .diplomacy
        .wars
        .get_mut(&war_id)
        .ok_or(WarError::BadCountry)?;
    war.set_join_policy(country, policy);
    Ok(())
}

/// P0.11：将延迟参战的国家正式加入战争。
///
/// 将其从 war_join_policies 中移除（恢复默认 AutoJoin 行为），
/// 然后立即加入战争对应方（与 war_leader 同侧）。
/// 用于事件触发地方军参战的场景。
pub fn add_delayed_war_participant(
    world: &mut World,
    war_id: u32,
    war_leader: CountryId,
    participant: CountryId,
) -> Result<(), WarError> {
    let war = world
        .diplomacy
        .wars
        .get_mut(&war_id)
        .ok_or(WarError::BadCountry)?;

    if war.attackers.contains(&participant) || war.defenders.contains(&participant) {
        return Err(WarError::AlreadyAtWar);
    }

    if war.attackers.contains(&war_leader) {
        war.attackers.insert(participant);
    } else {
        war.defenders.insert(participant);
    }
    war.war_join_policies.remove(&participant);

    let ci = participant.0 as usize;
    if ci < world.countries.count {
        world.countries.at_war[ci] = world.diplomacy.is_at_war(participant);
    }
    Ok(())
}

/// 把已投降国家从所有进行中战争里移除。
pub fn remove_capitulated_from_wars(world: &mut World, country: CountryId) {
    for war in world.diplomacy.wars.values_mut() {
        war.attackers.remove(&country);
        war.defenders.remove(&country);
    }
    if (country.0 as usize) < world.countries.count {
        world.countries.at_war[country.0 as usize] = world.diplomacy.is_at_war(country);
    }
}

/// 每日"晚加入阵营"对账：[`declare_war`] 只在宣战瞬间快照阵营成员，之后才加入
/// （或新成立）的阵营成员不会自动入战。本函数遍历所有进行中的战争：
/// - 重新查 primary_attacker / primary_defender 当前的 faction
/// - 把 faction 中所有成员补进 `War.attackers` / `War.defenders` 集合
/// - **P0.11**：尊重 `War.war_join_policies`，Delayed/Forbidden 的阵营成员不会被自动拉入
/// - 同步更新 `world.countries.at_war`
///
/// 返回新加入战争的"国家次数"（一个国家进了 N 场战争算 N 次）。
pub fn reconcile_war_membership(world: &mut World) -> usize {
    let n_countries = world.countries.count;
    let mut newly_added = 0usize;

    let war_ids: Vec<u32> = world.diplomacy.wars.keys().copied().collect();
    for war_id in war_ids {
        let (attacker_faction, defender_faction) = {
            let Some(war) = world.diplomacy.wars.get(&war_id) else {
                continue;
            };
            (
                world.diplomacy.faction_of(war.primary_attacker),
                world.diplomacy.faction_of(war.primary_defender),
            )
        };

        let attacker_members: Vec<CountryId> = attacker_faction
            .and_then(|fid| world.diplomacy.faction(fid))
            .map(|f| f.members.clone())
            .unwrap_or_default();
        let defender_members: Vec<CountryId> = defender_faction
            .and_then(|fid| world.diplomacy.faction(fid))
            .map(|f| f.members.clone())
            .unwrap_or_default();

        if let Some(war) = world.diplomacy.wars.get_mut(&war_id) {
            for c in attacker_members {
                if !war.attackers.contains(&c) && !war.defenders.contains(&c) {
                    if war.is_auto_join_blocked(c) {
                        continue;
                    }
                    war.attackers.insert(c);
                    newly_added += 1;
                }
            }
            for c in defender_members {
                if !war.defenders.contains(&c) && !war.attackers.contains(&c) {
                    if war.is_auto_join_blocked(c) {
                        continue;
                    }
                    war.defenders.insert(c);
                    newly_added += 1;
                }
            }
        }
    }

    // 重新计算每国 at_war 标志
    for ci in 0..n_countries {
        let cid = CountryId(ci as u16);
        world.countries.at_war[ci] = world.diplomacy.is_at_war(cid);
    }

    newly_added
}

/// P1.1：统一战争清理函数。
///
/// 投降和和平共用此出口：删除指定战争，重算所有参战国 at_war 状态。
/// 同时清理战争成员残留的 war_join_policies 等附属数据。
///
/// 返回被移除的战争 id。
pub fn cleanup_war(world: &mut World, war_id: u32) -> Result<u32, WarError> {
    let war = world
        .diplomacy
        .wars
        .remove(&war_id)
        .ok_or(WarError::BadCountry)?;

    let participants: Vec<CountryId> = war
        .attackers
        .iter()
        .chain(war.defenders.iter())
        .copied()
        .collect();
    for c in participants {
        let ci = c.0 as usize;
        if ci < world.countries.count {
            world.countries.at_war[ci] = world.diplomacy.is_at_war(c);
        }
    }

    Ok(war_id)
}

/// P1.1：删除空战争（一方全部退出后没有攻击者或没有防御者的战争），
/// 并重算所有相关国家 at_war 状态。
///
/// 返回被删除的空战争数量。
pub fn remove_empty_wars(world: &mut World) -> usize {
    let mut empty_war_ids: Vec<u32> = Vec::new();
    for (&war_id, war) in &world.diplomacy.wars {
        if war.attackers.is_empty() || war.defenders.is_empty() {
            empty_war_ids.push(war_id);
        }
    }
    let count = empty_war_ids.len();
    for war_id in empty_war_ids {
        let _ = cleanup_war(world, war_id);
    }
    count
}

/// P1.1：每日和平结算入口。
///
/// 检查所有进行中的战争：
/// 1. 如果一方全部投降（被 `evict_capitulated_daily` 逐出），自动执行和平会议。
/// 2. 如果一方全部被吞并，自动删除空战争。
/// 3. 删除空战争（攻击方或防御方为空的残留战争）。
///
/// 返回本次结算的战争结果。
#[derive(Debug, Clone, Default)]
pub struct PeaceResolutionOutcome {
    /// 每场已结算战争的详情
    pub resolved_wars: Vec<ResolvedWarInfo>,
    /// 因攻击方或防御方为空而清理的战争数
    pub empty_wars_removed: u32,
}

#[derive(Debug, Clone)]
pub struct ResolvedWarInfo {
    /// 战争 id
    pub war_id: u32,
    /// 主攻方标签
    pub primary_attacker_tag: String,
    /// 主守方标签
    pub primary_defender_tag: String,
    /// 获胜方
    pub winning_side: WarSide,
    /// 和平会议结果
    pub peace_outcome: Option<super::peace::PeaceOutcome>,
}

pub fn tick_peace_resolution_daily(world: &mut World) -> PeaceResolutionOutcome {
    let mut outcome = PeaceResolutionOutcome::default();

    // 第一步：检查所有战争是否有一方全部投降或被吞并
    let war_ids: Vec<u32> = world.diplomacy.wars.keys().copied().collect();
    let mut to_resolve: Vec<(u32, WarSide, String, String)> = Vec::new();

    for war_id in war_ids {
        let Some(war) = world.diplomacy.wars.get(&war_id) else {
            continue;
        };

        let primary_attacker_tag = world
            .country_tag(war.primary_attacker)
            .unwrap_or("")
            .to_owned();
        let primary_defender_tag = world
            .country_tag(war.primary_defender)
            .unwrap_or("")
            .to_owned();

        let attackers_all_gone = war.attackers.is_empty()
            || war.attackers.iter().all(|&a| {
                is_capitulated(world, a) || world.diplomacy.annexed_countries.contains(&a)
            });
        let defenders_all_gone = war.defenders.is_empty()
            || war.defenders.iter().all(|&d| {
                is_capitulated(world, d) || world.diplomacy.annexed_countries.contains(&d)
            });

        if defenders_all_gone && !attackers_all_gone {
            to_resolve.push((
                war_id,
                WarSide::Attacker,
                primary_attacker_tag,
                primary_defender_tag,
            ));
        } else if attackers_all_gone && !defenders_all_gone {
            to_resolve.push((
                war_id,
                WarSide::Defender,
                primary_attacker_tag,
                primary_defender_tag,
            ));
        } else if attackers_all_gone && defenders_all_gone {
            let _ = cleanup_war(world, war_id);
        }
    }

    // 执行和平会议，收集结果
    for (war_id, winning_side, atk_tag, def_tag) in to_resolve {
        if world.diplomacy.wars.contains_key(&war_id) {
            let peace_outcome = super::peace::peace_conference(world, war_id, winning_side);
            outcome.resolved_wars.push(ResolvedWarInfo {
                war_id,
                primary_attacker_tag: atk_tag,
                primary_defender_tag: def_tag,
                winning_side,
                peace_outcome,
            });
        }
    }

    // 删除双方全部出局的战争（peace_conference 已处理大部分，这里清残留）
    let war_ids_again: Vec<u32> = world.diplomacy.wars.keys().copied().collect();
    for war_id in war_ids_again {
        let Some(war) = world.diplomacy.wars.get(&war_id) else {
            continue;
        };
        let attackers_all_gone = war.attackers.is_empty()
            || war.attackers.iter().all(|&a| {
                is_capitulated(world, a) || world.diplomacy.annexed_countries.contains(&a)
            });
        let defenders_all_gone = war.defenders.is_empty()
            || war.defenders.iter().all(|&d| {
                is_capitulated(world, d) || world.diplomacy.annexed_countries.contains(&d)
            });
        if attackers_all_gone && defenders_all_gone {
            let _ = cleanup_war(world, war_id);
        }
    }

    // 第二步：清理空战争（evict_capitulated_daily 可能导致某方变空）
    outcome.empty_wars_removed = remove_empty_wars(world) as u32;

    // 最终确保所有国家 at_war 状态正确
    for ci in 0..world.countries.count {
        let cid = CountryId(ci as u16);
        world.countries.at_war[ci] = world.diplomacy.is_at_war(cid);
    }

    outcome
}

/// 每日扫描所有国家：投降的从所有战争中移除，并把它们加入 `annexed_countries`
/// （供紧张度衰减/上升使用，简化为"投降即视为退场"）。
///
/// **不**自动建立傀儡关系（这是 `peace_conference` 的活）。
///
/// 返回本次新移除的（capitulated）国家数。
pub fn evict_capitulated_daily(world: &mut World) -> usize {
    let n = world.countries.count;
    // 先收集要移除的国家列表，避免在迭代中修改 world.diplomacy
    let mut to_evict: Vec<CountryId> = Vec::new();
    for ci in 0..n {
        let cid = CountryId(ci as u16);
        // 没参加任何战争的国家不会因为"丢光所有省"在 1.3 范围内被处理；
        // 其它系统会保护和平国家的 controllers。
        if !world.diplomacy.is_at_war(cid) {
            continue;
        }
        if is_capitulated(world, cid) {
            to_evict.push(cid);
        }
    }
    let count = to_evict.len();
    for cid in to_evict {
        remove_capitulated_from_wars(world, cid);
    }
    count
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use hoi4_data::{Color, Country, CountryTag, GameData, State};
    use hoi4_map::{
        GameMap, Heightmap, ProvinceDefinition, ProvinceMap, ProvinceType, TerrainBitmap,
        TerrainCatalog,
    };

    use super::*;

    #[test]
    fn capital_control_prevents_premature_capitulation() {
        let mut world = two_state_world();
        let aaa = world.country("AAA").unwrap();
        let bbb = world.country("BBB").unwrap();

        world.states.controllers[1] = bbb;
        world.provinces.controllers[2] = bbb;

        assert!(
            !is_capitulated(&world, aaa),
            "首都仍受控制时，不应因外围州丢失而投降"
        );
    }

    #[test]
    fn capital_loss_and_low_vp_control_capitulates() {
        let mut world = two_state_world();
        let aaa = world.country("AAA").unwrap();
        let bbb = world.country("BBB").unwrap();

        world.states.controllers[0] = bbb;
        world.provinces.controllers[1] = bbb;

        assert!(
            is_capitulated(&world, aaa),
            "首都和主要胜利点丢失后应判定投降"
        );
    }

    fn two_state_world() -> World {
        let aaa = CountryTag::new("AAA");
        let bbb = CountryTag::new("BBB");
        let mut data = GameData::default();
        data.countries.insert(
            aaa.clone(),
            Country {
                tag: aaa.clone(),
                color: Color { r: 1, g: 1, b: 1 },
                graphical_culture: String::new(),
                capital: 1,
                ruling_party: "neutrality".to_owned(),
                technologies: Vec::new(),
            },
        );
        data.countries.insert(
            bbb.clone(),
            Country {
                tag: bbb.clone(),
                color: Color { r: 2, g: 2, b: 2 },
                graphical_culture: String::new(),
                capital: 3,
                ruling_party: "neutrality".to_owned(),
                technologies: Vec::new(),
            },
        );
        data.states.push(State {
            id: 1,
            name: "AAA Capital".to_owned(),
            manpower: 0,
            owner: aaa.clone(),
            cores: vec![aaa.clone()],
            provinces: vec![1],
            category: String::new(),
            infrastructure: 0,
            victory_points: vec![(1, 10)],
            resources: Vec::new(),
        });
        data.states.push(State {
            id: 2,
            name: "AAA Border".to_owned(),
            manpower: 0,
            owner: aaa,
            cores: vec![CountryTag::new("AAA")],
            provinces: vec![2],
            category: String::new(),
            infrastructure: 0,
            victory_points: Vec::new(),
            resources: Vec::new(),
        });
        data.states.push(State {
            id: 3,
            name: "BBB Home".to_owned(),
            manpower: 0,
            owner: bbb.clone(),
            cores: vec![bbb],
            provinces: vec![3],
            category: String::new(),
            infrastructure: 0,
            victory_points: Vec::new(),
            resources: Vec::new(),
        });

        let map = Arc::new(GameMap {
            definitions: vec![
                None,
                Some(province(1)),
                Some(province(2)),
                Some(province(3)),
            ],
            rgb_to_id: std::collections::HashMap::new(),
            province_map: ProvinceMap {
                width: 0,
                height: 0,
                pixels: Vec::new(),
            },
            adjacencies: vec![Vec::new(), vec![2], vec![1, 3], vec![2]],
            special_adjacencies: Vec::new(),
            heightmap: Heightmap {
                width: 0,
                height: 0,
                pixels: Vec::new(),
            },
            terrain_bmp: TerrainBitmap {
                width: 0,
                height: 0,
                pixels: Vec::new(),
                palette: [[0; 3]; 256],
            },
            terrain_catalog: TerrainCatalog::default(),
            tree_definition_bmp: None,
            tree_indices: hoi4_map::DEFAULT_TREE_INDICES.iter().copied().collect(),
        });

        World::new(map, Arc::new(data))
    }

    fn province(id: u16) -> ProvinceDefinition {
        ProvinceDefinition {
            id,
            r: id as u8,
            g: 0,
            b: 0,
            province_type: ProvinceType::Land,
            coastal: false,
            terrain: "plains".to_owned(),
            continent: 0,
        }
    }
}
