//! Evaluate RON Trigger / execute RON Effect against World state.

use crate::focus::{Effect, Trigger};
use hoi4_state::{
    CountryId, LawCategory, ScriptedDiplomaticEffect, StateId, TradeDirection, TradeRouteKind,
    World,
};

/// P1.2：事件级联触发描述结构体，保存事件 id、作用国家、显示国家和来源。
#[derive(Debug, Clone, PartialEq)]
pub struct PendingTrigger {
    pub event_id: String,
    /// 效果作用的国家（effects 以此国家为上下文执行）。
    pub effect_country: CountryId,
    /// 事件显示的国家（UI modal 展示目标）。
    /// 新闻事件中 effect_country 可能与 display_country 不同。
    pub display_country: CountryId,
    /// 触发来源描述（如 "surrender:FRA"、effect 名称等），用于日志和调试。
    pub source: String,
}

/// P1.3：效果执行报告，记录成功应用、警告和错误。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EffectReport {
    /// 成功执行的效果描述列表。
    pub applied: Vec<String>,
    /// 执行中产生的警告（如国家不存在但效果被跳过）。
    pub warnings: Vec<String>,
    /// 执行中产生的错误（如外交操作失败）。
    pub errors: Vec<String>,
    /// App-side requests that cannot be applied inside hoi4-content.
    pub switch_player_country: Vec<String>,
}

impl EffectReport {
    /// 合并另一个报告到当前报告。
    pub fn merge(&mut self, other: EffectReport) {
        self.applied.extend(other.applied);
        self.warnings.extend(other.warnings);
        self.errors.extend(other.errors);
        self.switch_player_country
            .extend(other.switch_player_country);
    }

    /// 是否有任何错误。
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// 是否有任何警告。
    pub fn has_warnings(&self) -> bool {
        !self.warnings.is_empty()
    }
}

/// Global flags storage (simple set of strings).
#[derive(Debug, Default, Clone)]
pub struct GlobalFlags {
    pub flags: std::collections::HashSet<String>,
    /// P1.2：级联触发队列改为 VecDeque<PendingTrigger>，FIFO 顺序处理。
    /// 效果执行完毕后 drain，逐个调用 `EventScheduler::trigger_scoped` 入队 modal。
    pub pending_triggers: std::collections::VecDeque<PendingTrigger>,
}

/// Evaluate a RON Trigger against World state for a given country.
pub fn eval_trigger(t: &Trigger, world: &World, country: CountryId, flags: &GlobalFlags) -> bool {
    let i = country.0 as usize;
    match t {
        Trigger::AlwaysTrue => true,
        Trigger::AlwaysFalse => false,
        Trigger::Tag(tag) => world
            .countries
            .tags
            .get(i)
            .is_some_and(|country_tag| country_tag == tag),
        Trigger::HasCountryFlag(f) => world.countries.ideas[i]
            .iter()
            .any(|x| x.starts_with("FLAG:") && &x[5..] == f.as_str()),
        Trigger::HasGlobalFlag(f) => flags.flags.contains(f),
        Trigger::HasCompletedFocus(f) => world.countries.completed_focuses[i].contains(f),
        Trigger::HasIdea(idea) => world.countries.ideas[i].iter().any(|x| x == idea),
        Trigger::HasTech(tech) => world.countries.completed_techs[i].iter().any(|x| x == tech),
        Trigger::HasGovernment(g) => world.countries.ruling_party[i] == *g,
        Trigger::PartyPopularity { ideology, amount } => {
            world.countries.party_popularity[i]
                .get(ideology)
                .copied()
                .unwrap_or(0.0)
                >= *amount
        }
        Trigger::PoliticalPower(v) => world.countries.political_power[i] >= *v,
        Trigger::Stability(v) => world.countries.stability[i] >= *v,
        Trigger::WarSupport(v) => world.countries.war_support[i] >= *v,

        Trigger::NumOfFactories(v) => count_v6_factories(world, country, None) >= *v,
        Trigger::NumOfCivilianFactories(v) => count_v6_factories(world, country, Some(false)) >= *v,
        Trigger::NumOfMilitaryFactories(v) => count_v6_factories(world, country, Some(true)) >= *v,
        Trigger::Manpower(v) => world.manpower(country) >= *v,
        Trigger::HasWar(want) => world.diplomacy.is_at_war(country) == *want,
        Trigger::HasWarWith(tag) => world
            .tag_to_country
            .get(tag)
            .map(|&other| world.diplomacy.at_war_with(country, other))
            .unwrap_or(false),
        Trigger::NumDivisions(n) => {
            let count = world
                .divisions
                .owners
                .iter()
                .filter(|&&o| o == country)
                .count() as u32;
            count >= *n
        }
        Trigger::ArmyExperience(v) => world.countries.army_xp[i] >= *v,
        Trigger::NavyExperience(v) => world.countries.navy_xp[i] >= *v,
        Trigger::AirExperience(v) => world.countries.air_xp[i] >= *v,
        Trigger::IsInFaction(want) => world.diplomacy.faction_of(country).is_some() == *want,
        Trigger::IsInFactionWith(tag) => {
            let my_f = world.diplomacy.faction_of(country);
            let other = world
                .tag_to_country
                .get(tag)
                .and_then(|&c| world.diplomacy.faction_of(c));
            my_f.is_some() && my_f == other
        }
        Trigger::IsFactionLeader(want) => {
            let leader = world
                .diplomacy
                .faction_of(country)
                .and_then(|fid| world.diplomacy.faction(fid))
                .map(|f| f.leader == country)
                .unwrap_or(false);
            leader == *want
        }

        Trigger::IsSubjectOf(tag) => world
            .tag_to_country
            .get(tag)
            .map(|&master| world.diplomacy.is_subject_of(country, master))
            .unwrap_or(false),
        Trigger::IsPuppet(want) => world.diplomacy.autonomy.contains_key(&country) == *want,
        Trigger::OpinionOf { target, value } => world
            .tag_to_country
            .get(target)
            .map(|&other| world.diplomacy.opinions.get(country, other) >= *value)
            .unwrap_or(false),
        Trigger::WorldTension(v) => world.diplomacy.world_tension >= *v,
        Trigger::OwnsState(sid) => world
            .state_id_lookup
            .get(sid)
            .map(|&s| world.states.owners[s.0 as usize] == country)
            .unwrap_or(false),
        Trigger::ControlsState(sid) => world
            .state_id_lookup
            .get(sid)
            .map(|&s| world.states.controllers[s.0 as usize] == country)
            .unwrap_or(false),
        Trigger::CountryExists(tag) => world
            .tag_to_country
            .get(tag)
            .map(|&c| !world.diplomacy.annexed_countries.contains(&c))
            .unwrap_or(false),
        Trigger::Date { year, month, day } => {
            let target = hoi4_state::GameDate {
                year: *year,
                month: *month,
                day: *day,
                hour: 0,
            };
            world.date >= target
        }

        // ── P2.5 国家变量比较 ──
        Trigger::VariableAtLeast { name, value } => {
            country_variable(world, country, name) >= *value
        }
        Trigger::VariableAtMost { name, value } => country_variable(world, country, name) <= *value,
        Trigger::VariableInRange { name, min, max } => {
            let v = country_variable(world, country, name);
            v >= *min && v <= *max
        }
        Trigger::VariableEquals { name, value } => {
            (country_variable(world, country, name) - *value).abs() < 1e-4
        }

        // ── P2.6 人物状态 ──
        Trigger::CharacterDead(key) => character_status(world, country, key)
            .map(|s| matches!(s, hoi4_state::CharacterRuntimeStatus::Dead))
            .unwrap_or(false),
        Trigger::CharacterAvailable(key) => match character_status(world, country, key) {
            Some(hoi4_state::CharacterRuntimeStatus::Available)
            | Some(hoi4_state::CharacterRuntimeStatus::InOffice) => true,
            _ => false,
        },
        Trigger::CharacterIsLeader(key) => character_status(world, country, key)
            .map(|s| matches!(s, hoi4_state::CharacterRuntimeStatus::InOffice))
            .unwrap_or(false),

        Trigger::And(v) => v.iter().all(|sub| eval_trigger(sub, world, country, flags)),
        Trigger::Or(v) => v.iter().any(|sub| eval_trigger(sub, world, country, flags)),
        Trigger::Not(sub) => !eval_trigger(sub, world, country, flags),
    }
}

fn count_v6_factories(world: &World, country: CountryId, military: Option<bool>) -> u32 {
    world
        .countries
        .buildings_v6
        .buildings
        .iter()
        .filter(|building| {
            let si = building.state.0 as usize;
            if si >= world.states.count || world.states.owners[si] != country || building.level == 0
            {
                return false;
            }
            match military {
                Some(true) => matches!(building.kind, hoi4_state::BuildingKind::Military),
                Some(false) => !matches!(building.kind, hoi4_state::BuildingKind::Military),
                None => true,
            }
        })
        .map(|building| building.level as u32)
        .sum()
}

/// P2.5：读取国家级变量值。变量未定义视为 0.0。
fn country_variable(world: &World, country: CountryId, name: &str) -> f32 {
    if country.is_none() {
        return 0.0;
    }
    let i = country.0 as usize;
    world
        .countries
        .variables
        .get(i)
        .and_then(|m| m.get(name))
        .copied()
        .unwrap_or(0.0)
}

/// P2.6：读取国家级人物状态。人物未注册视为 None。
fn character_status(
    world: &World,
    country: CountryId,
    key: &str,
) -> Option<hoi4_state::CharacterRuntimeStatus> {
    if country.is_none() {
        return None;
    }
    let i = country.0 as usize;
    world
        .countries
        .characters
        .get(i)
        .and_then(|m| m.get(key))
        .copied()
}

/// P1.3：执行效果列表，返回效果报告（包含成功、警告、错误）。
pub fn run_effects(
    effects: &[Effect],
    world: &mut World,
    country: CountryId,
    flags: &mut GlobalFlags,
) -> EffectReport {
    let mut report = EffectReport::default();
    for effect in effects {
        let sub = run_one(effect, world, country, flags);
        report.merge(sub);
    }
    report
}

fn run_one(
    e: &Effect,
    world: &mut World,
    country: CountryId,
    flags: &mut GlobalFlags,
) -> EffectReport {
    let mut report = EffectReport::default();
    let i = country.0 as usize;
    match e {
        Effect::AddPoliticalPower(v) => {
            world.countries.political_power[i] =
                (world.countries.political_power[i] + v).clamp(-1000.0, 1000.0);
        }
        Effect::AddStability(v) => {
            world.countries.stability[i] = (world.countries.stability[i] + v).clamp(0.0, 1.0);
        }
        Effect::AddWarSupport(v) => {
            world.countries.war_support[i] = (world.countries.war_support[i] + v).clamp(0.0, 1.0);
        }
        Effect::AddManpower(v) => {
            if *v >= 0 {
                let state_ids = world.country_state_ids(country);
                let pop_indices = world
                    .countries
                    .pops
                    .pops_by_class_in_country_mut(hoi4_state::PopClass::Soldier, &state_ids);
                let mut remaining = *v as u64;
                for &idx in &pop_indices {
                    if remaining == 0 {
                        break;
                    }
                    let pg = &mut world.countries.pops.groups[idx];
                    if pg.employed_at.is_none() {
                        let add = remaining.min(u32::MAX as u64) as u32;
                        pg.size = pg.size.saturating_add(add);
                        remaining = remaining.saturating_sub(add as u64);
                    }
                }
            } else {
                let state_ids = world.country_state_ids(country);
                let pop_indices = world
                    .countries
                    .pops
                    .pops_by_class_in_country_mut(hoi4_state::PopClass::Soldier, &state_ids);
                let mut to_deduct = (-*v) as u64;
                for &idx in &pop_indices {
                    if to_deduct == 0 {
                        break;
                    }
                    let pg = &mut world.countries.pops.groups[idx];
                    if pg.employed_at.is_none() {
                        let deduct = to_deduct.min(pg.size as u64);
                        pg.size = pg.size.saturating_sub(deduct as u32);
                        to_deduct = to_deduct.saturating_sub(deduct);
                    }
                }
            }
        }
        Effect::AddFuel(v) => {
            world.countries.fuel[i] = (world.countries.fuel[i] + v).max(0.0);
        }
        Effect::ArmyExperience(v) => {
            world.countries.army_xp[i] += v;
        }
        Effect::NavyExperience(v) => {
            world.countries.navy_xp[i] += v;
        }
        Effect::AirExperience(v) => {
            world.countries.air_xp[i] += v;
        }

        Effect::SetCountryFlag(f) => {
            let key = format!("FLAG:{f}");
            if !world.countries.ideas[i].contains(&key) {
                world.countries.ideas[i].push(key);
            }
        }
        Effect::ClearCountryFlag(f) => {
            let key = format!("FLAG:{f}");
            world.countries.ideas[i].retain(|x| x != &key);
        }
        Effect::SetGlobalFlag(f) => {
            flags.flags.insert(f.clone());
        }
        Effect::ClearGlobalFlag(f) => {
            flags.flags.remove(f);
        }
        Effect::AddIdea(idea) => {
            if !world.countries.ideas[i].contains(idea) {
                world.countries.ideas[i].push(idea.clone());
            }
        }
        Effect::RemoveIdea(idea) => {
            world.countries.ideas[i].retain(|x| x != idea);
        }
        Effect::SwapIdea { remove, add } => {
            world.countries.ideas[i].retain(|x| x != remove);
            if !world.countries.ideas[i].contains(add) {
                world.countries.ideas[i].push(add.clone());
            }
        }
        Effect::SetPolitics { ruling_party } => {
            world.countries.ruling_party[i] = ruling_party.clone();
            // J.1.8锛氭墽鏀垮厷鍙樺寲 鈫?閲嶉€夊厓棣栬倴鍍?
            world.refresh_country_leader(country);
        }
        Effect::AddPopularity { ideology, amount } => {
            let entry = world.countries.party_popularity[i]
                .entry(ideology.clone())
                .or_insert(0.0);
            *entry = (*entry + amount).clamp(0.0, 1.0);
        }
        Effect::SetPartyName { .. } => { /* cosmetic only, no state change */ }

        Effect::AddResearchSlot(n) => {
            world.countries.research_slots[i] =
                (world.countries.research_slots[i] as i8 + n).max(1) as u8;
        }
        Effect::AddTechBonus { category, bonus } => {
            let entry = world.countries.tech_bonus[i]
                .entry(category.clone())
                .or_insert(0.0);
            *entry += bonus;
        }
        Effect::SetTechnology(tech) => {
            if !world.countries.completed_techs[i].contains(tech) {
                world.countries.completed_techs[i].push(tech.clone());
            }
        }
        Effect::AddBuildingInState {
            state,
            building,
            level,
        } => {
            if let Some(&sid) = world.state_id_lookup.get(state) {
                let si = sid.0 as usize;
                match building.as_str() {
                    "industrial_complex" => add_v6_building(
                        world,
                        sid,
                        "steel_mill",
                        hoi4_state::BuildingKind::Industrial,
                        *level,
                    ),
                    "arms_factory" => add_v6_building(
                        world,
                        sid,
                        "arms_industry",
                        hoi4_state::BuildingKind::Military,
                        *level,
                    ),
                    "dockyard" => add_v6_building(
                        world,
                        sid,
                        "shipyard",
                        hoi4_state::BuildingKind::Military,
                        *level,
                    ),
                    "infrastructure" => {
                        world.states.infrastructure[si] =
                            (world.states.infrastructure[si] as i32 + level).max(0) as u8
                    }
                    _ => {}
                }
            }
        }
        Effect::AddBuildingAllStates { building, level } => {
            for si in 0..world.states.count {
                if world.states.owners[si] == country {
                    match building.as_str() {
                        "industrial_complex" => add_v6_building(
                            world,
                            hoi4_state::StateId(si as u16),
                            "steel_mill",
                            hoi4_state::BuildingKind::Industrial,
                            *level,
                        ),
                        "arms_factory" => add_v6_building(
                            world,
                            hoi4_state::StateId(si as u16),
                            "arms_industry",
                            hoi4_state::BuildingKind::Military,
                            *level,
                        ),
                        "infrastructure" => {
                            world.states.infrastructure[si] =
                                (world.states.infrastructure[si] as i32 + level).max(0) as u8
                        }
                        _ => {}
                    }
                }
            }
        }
        Effect::AddBuildingLevel {
            state,
            building_id,
            level,
        } => {
            if let Some(&sid) = world.state_id_lookup.get(state) {
                add_v6_building(
                    world,
                    sid,
                    building_id,
                    infer_building_kind(building_id),
                    *level,
                );
            }
        }
        Effect::AddExtraBuildingSlots { .. } => { /* tracked as modifier */ }
        Effect::AddResource { .. } => { /* tracked as modifier */ }
        Effect::AddResourceDiscovery {
            state,
            good_id,
            discovered_level,
        } => {
            let marker = format!("RESOURCE_DISCOVERY:{state}:{good_id}:{discovered_level}");
            if !world.countries.ideas[i].contains(&marker) {
                world.countries.ideas[i].push(marker);
            }
        }
        Effect::AddProductionMethodUnlock { pm_id } => {
            let marker = format!("PM_UNLOCK:{pm_id}");
            if !world.countries.ideas[i].contains(&marker) {
                world.countries.ideas[i].push(marker);
            }
        }
        Effect::AddGovernmentOrder {
            equipment_category,
            daily_budget_rm,
            duration_days,
        } => {
            let total_rm = (*daily_budget_rm).max(0.0) * (*duration_days as f64).max(1.0);
            world.countries.treasury.treasuries[i].pay(total_rm, "military_procurement");
            let marker = format!(
                "GOV_ORDER:{equipment_category}:{:.0}:{}",
                daily_budget_rm, duration_days
            );
            if !world.countries.ideas[i].contains(&marker) {
                world.countries.ideas[i].push(marker);
            }
        }
        Effect::AddTradeAgreement {
            partner,
            good_id,
            daily_quantity,
        } => {
            if let Some(&other) = world.tag_to_country.get(partner) {
                world.countries.trade.add_route_for_good(
                    country,
                    other,
                    Some(good_id.clone()),
                    TradeRouteKind::Land,
                    None,
                    *daily_quantity,
                    false,
                );
                world.countries.trade.add_agreement(
                    country,
                    other,
                    good_id.clone(),
                    *daily_quantity,
                    TradeDirection::AImportsFromB,
                );
            }
        }
        Effect::ChangeLaw {
            category,
            law_id,
            lock_days,
        } => {
            if let Some(category) = parse_law_category(category) {
                let slot = &mut world.countries.law_store.law_sets[i].0[category.index()];
                if !slot.is_locked || slot.current == *law_id {
                    slot.current = law_id.clone();
                    slot.cooldown_days = *lock_days;
                }
            }
        }
        Effect::AddMefoCapacity { amount_rm } => {
            let can_add_mefo = world.countries.tags.get(i).map(|tag| tag.as_str()) == Some("GER")
                && !world.countries.treasury.treasuries[i].mefo_disabled;
            let treasury = &mut world.countries.treasury.treasuries[i];
            if can_add_mefo {
                let actual = (*amount_rm).max(0.0);
                treasury.mefo_debt_rm += actual;
                treasury.cash_rm += actual;
                treasury.daily_income_rm += actual;
                treasury.mefo_coverage_rm += actual;
                treasury.daily_financing.mefo_issued_rm += actual;
            }
        }
        Effect::AddPrivateInvestmentPool { amount_rm } => {
            world.countries.private_investment_pool_rm[i] =
                (world.countries.private_investment_pool_rm[i] + amount_rm).max(0.0);
            if let Some(account) = world
                .countries
                .investment_account_mut(country, hoi4_state::InvestmentAccountKind::Private)
            {
                account.balance_rm = (account.balance_rm + *amount_rm).max(0.0);
                account.last_income_rm += (*amount_rm).max(0.0);
            }
        }
        Effect::AddConstructionCapacity { amount } => {
            if let Some(sid) = first_owned_state(world, country) {
                add_v6_building(
                    world,
                    sid,
                    "construction_sector",
                    hoi4_state::BuildingKind::Industrial,
                    *amount,
                );
            }
        }
        Effect::AddMilitarySpendingShare { delta } => {
            let marker = format!("MIL_SPENDING_SHARE:{delta:+.4}");
            if !world.countries.ideas[i].contains(&marker) {
                world.countries.ideas[i].push(marker);
            }
        }
        Effect::AddConscription(v) => {
            let target = if *v <= 0.015 {
                "volunteer_only"
            } else if *v <= 0.035 {
                "limited_conscription"
            } else if *v <= 0.075 {
                "extensive_conscription"
            } else {
                "total_mobilization"
            };
            let law_set = &mut world.countries.law_store.law_sets[i];
            let slot = &mut law_set.0[hoi4_state::LawCategory::Conscription.index()];
            if slot.current != target && slot.cooldown_days == 0 && !slot.is_locked {
                slot.current = target.to_owned();
            }
        }
        Effect::CreateDivision { .. } => { /* requires template lookup, deferred to D.5 */ }

        Effect::AddNamedThreat(v) => {
            world.diplomacy.world_tension = (world.diplomacy.world_tension + v).clamp(0.0, 100.0);
        }
        Effect::AddOpinion { target, amount } => {
            if let Some(&other) = world.tag_to_country.get(target) {
                world.diplomacy.opinions.modify(country, other, *amount);
            } else {
                report
                    .warnings
                    .push(format!("AddOpinion: 目标国家 {target} 不存在"));
            }
        }
        Effect::CreateFaction(name) => {
            if let Some(leader) = world.country_tag(country).map(str::to_owned) {
                let effect = ScriptedDiplomaticEffect::CreateFaction {
                    leader,
                    name: name.clone(),
                };
                apply_diplomatic_effect_report(&mut report, "CreateFaction", world, &effect);
            }
        }
        Effect::AddToFaction(tag) => {
            if let Some(leader) = world.country_tag(country).map(str::to_owned) {
                let effect = ScriptedDiplomaticEffect::AddToFaction {
                    faction_leader: leader,
                    member: tag.clone(),
                };
                apply_diplomatic_effect_report(&mut report, "AddToFaction", world, &effect);
            }
        }
        Effect::LeaveFaction => {
            if let Some(fid) = world.diplomacy.faction_of(country) {
                if let Some(faction) = world.diplomacy.faction_mut(fid) {
                    faction.members.retain(|&c| c != country);
                }
            }
        }
        Effect::DeclareWarOn(tag) => {
            if let Some(attacker) = world.country_tag(country).map(str::to_owned) {
                let effect = ScriptedDiplomaticEffect::DeclareWar {
                    attacker,
                    defender: tag.clone(),
                };
                apply_diplomatic_effect_report(&mut report, "DeclareWarOn", world, &effect);
            }
        }
        Effect::AnnexCountry(tag) => {
            if let Some(annexer) = world.country_tag(country).map(str::to_owned) {
                let effect = ScriptedDiplomaticEffect::AnnexCountry {
                    annexer,
                    target: tag.clone(),
                };
                apply_diplomatic_effect_report(&mut report, "AnnexCountry", world, &effect);
            }
        }
        Effect::PuppetCountry(tag) => {
            if let Some(master) = world.country_tag(country).map(str::to_owned) {
                let effect = ScriptedDiplomaticEffect::SetAutonomy {
                    master,
                    subject: tag.clone(),
                    level: "puppet".into(),
                };
                apply_diplomatic_effect_report(&mut report, "PuppetCountry", world, &effect);
            }
        }
        Effect::FreeCountry(tag) => {
            if let Some(&other) = world.tag_to_country.get(tag) {
                world.diplomacy.autonomy.remove(&other);
            }
        }
        Effect::GiveMilitaryAccess(tag) => {
            if let Some(grantor) = world.country_tag(country).map(str::to_owned) {
                let effect = ScriptedDiplomaticEffect::GrantMilitaryAccess {
                    grantor,
                    grantee: tag.clone(),
                };
                apply_diplomatic_effect_report(&mut report, "GiveMilitaryAccess", world, &effect);
            }
        }
        Effect::NonAggressionPact(_) | Effect::GuaranteeIndependence(_) => {
            /* diplomatic agreements tracked as modifiers */
        }
        Effect::TransferState(sid) => {
            if let Some(owner) = world.country_tag(country).map(str::to_owned) {
                let effect = ScriptedDiplomaticEffect::TransferState { state: *sid, owner };
                apply_diplomatic_effect_report(&mut report, "TransferState", world, &effect);
            }
        }
        Effect::TransferStateTo {
            state,
            country: tag,
        } => {
            let effect = ScriptedDiplomaticEffect::TransferState {
                state: *state,
                owner: tag.clone(),
            };
            apply_diplomatic_effect_report(&mut report, "TransferStateTo", world, &effect);
        }
        Effect::CreateCountry {
            tag,
            color,
            ruling_party,
        } => {
            let existed = world.tag_to_country.contains_key(tag);
            let cid = world.spawn_country(tag, *color, ruling_party);
            if existed {
                report.applied.push(format!("country exists: {tag}"));
            } else {
                report
                    .applied
                    .push(format!("country created: {tag} ({cid:?})"));
            }
        }
        Effect::AddCoreTo {
            state,
            country: tag,
        } => {
            let effect = ScriptedDiplomaticEffect::AddCore {
                state: *state,
                country: tag.clone(),
            };
            apply_diplomatic_effect_report(&mut report, "AddCoreTo", world, &effect);
        }
        Effect::RemoveCoreFrom {
            state,
            country: tag,
        } => {
            if let Some(&s) = world.state_id_lookup.get(state) {
                if let Some(&c) = world.tag_to_country.get(tag) {
                    world.states.cores[s.0 as usize].retain(|&x| x != c);
                }
            }
        }
        Effect::AddClaim { .. } => { /* claims not yet tracked in StateStore */ }
        Effect::SpawnDivisionsInStates {
            country: tag,
            states,
            count_per_state,
            name_prefix,
        } => {
            let Some(owner) = world.country(tag) else {
                report
                    .warnings
                    .push(format!("SpawnDivisionsInStates: target not found: {tag}"));
                return report;
            };
            let mut spawned = 0u32;
            for state_game_id in states {
                let Some(&state_id) = world.state_id_lookup.get(state_game_id) else {
                    report.warnings.push(format!(
                        "SpawnDivisionsInStates: state not found: {state_game_id}"
                    ));
                    continue;
                };
                let si = state_id.0 as usize;
                let Some(&province) = world.states.provinces.get(si).and_then(|p| p.first()) else {
                    continue;
                };
                for _ in 0..*count_per_state {
                    spawned += 1;
                    world.divisions.push(
                        owner,
                        province,
                        0,
                        18.0,
                        1.0,
                        format!("{name_prefix} {spawned}"),
                    );
                }
            }
            report
                .applied
                .push(format!("spawned {spawned} divisions for {tag}"));
        }
        Effect::UnlockDecision(_) | Effect::RemoveDecision(_) => { /* decision system in F.2 */ }

        // ── P2.5 国家变量 ──
        Effect::SetVariable { name, value } => {
            if let Some(map) = world.countries.variables.get_mut(i) {
                map.insert(name.clone(), *value);
                report
                    .applied
                    .push(format!("variable {}={:.3}", name, value));
            }
        }
        Effect::AddToVariable { name, value } => {
            if let Some(map) = world.countries.variables.get_mut(i) {
                let entry = map.entry(name.clone()).or_insert(0.0);
                *entry += value;
                report
                    .applied
                    .push(format!("variable {} {:+.3} -> {:.3}", name, value, entry));
            }
        }
        Effect::ClampVariable { name, min, max } => {
            if let Some(map) = world.countries.variables.get_mut(i) {
                if let Some(v) = map.get_mut(name) {
                    *v = v.clamp(*min, *max);
                }
            }
        }

        // ── P2.6 人物效果 ──
        Effect::KillCharacter(key) => {
            apply_character_status(
                world,
                country,
                key,
                hoi4_state::CharacterRuntimeStatus::Dead,
            );
            report.applied.push(format!("character dead: {key}"));
        }
        Effect::ExileCharacter(key) => {
            apply_character_status(
                world,
                country,
                key,
                hoi4_state::CharacterRuntimeStatus::Exiled,
            );
            report.applied.push(format!("character exiled: {key}"));
        }
        Effect::ImprisonCharacter(key) => {
            apply_character_status(
                world,
                country,
                key,
                hoi4_state::CharacterRuntimeStatus::Imprisoned,
            );
            report.applied.push(format!("character imprisoned: {key}"));
        }
        Effect::RecruitCharacter(key) => {
            // 不会复活已死亡或已流亡/被捕人物。
            if let Some(map) = world.countries.characters.get_mut(i) {
                map.entry(key.clone())
                    .or_insert(hoi4_state::CharacterRuntimeStatus::Available);
                report.applied.push(format!("character recruited: {key}"));
            }
        }
        Effect::SetCharacterStatus { key, status } => {
            apply_character_status(world, country, key, *status);
            report
                .applied
                .push(format!("character {key} -> {:?}", status));
        }
        Effect::SetLeader(key) => {
            // 当前可用 → 设为 InOffice，并清除其他 InOffice 人物。
            // 死亡/流亡/被捕则记录警告，不强行掌权。
            if country.is_none() {
                report.warnings.push(format!("SetLeader 无作用国家: {key}"));
            } else if let Some(map) = world.countries.characters.get_mut(i) {
                let current = map
                    .get(key)
                    .copied()
                    .unwrap_or(hoi4_state::CharacterRuntimeStatus::Available);
                let blocking = matches!(
                    current,
                    hoi4_state::CharacterRuntimeStatus::Dead
                        | hoi4_state::CharacterRuntimeStatus::Exiled
                        | hoi4_state::CharacterRuntimeStatus::Imprisoned
                );
                if blocking {
                    report
                        .warnings
                        .push(format!("SetLeader 跳过：{key} 处于 {:?}", current));
                } else {
                    for (other_key, status) in map.iter_mut() {
                        if other_key != key
                            && matches!(*status, hoi4_state::CharacterRuntimeStatus::InOffice)
                        {
                            *status = hoi4_state::CharacterRuntimeStatus::Available;
                        }
                    }
                    map.insert(key.clone(), hoi4_state::CharacterRuntimeStatus::InOffice);
                    report.applied.push(format!("set leader: {key}"));
                }
            }
        }

        Effect::TriggerEvent(id) => {
            // P1.2：推入包含作用域的 PendingTrigger，FIFO 逐个由 scheduler.trigger_scoped 入队。
            let effect_country = infer_event_effect_country(id, world, country);
            let display_country = if event_should_display_on_current_country(id) {
                country
            } else {
                effect_country
            };
            flags.pending_triggers.push_back(PendingTrigger {
                event_id: id.clone(),
                effect_country,
                display_country,
                source: format!("effect:{}", id),
            });
        }
        Effect::SwitchPlayerCountry(tag) => {
            if world.tag_to_country.contains_key(tag) {
                report.switch_player_country.push(tag.clone());
            } else {
                report
                    .warnings
                    .push(format!("SwitchPlayerCountry target not found: {tag}"));
            }
        }
        Effect::AddModifier { .. } | Effect::RemoveModifier(_) => { /* modifier system deferred */ }
        Effect::If { trigger, effects } => {
            if eval_trigger(trigger, world, country, flags) {
                report.merge(run_effects(effects, world, country, flags));
            }
        }
    }
    report
}

/// P2.6：通用人物状态写入辅助。
fn apply_character_status(
    world: &mut World,
    country: CountryId,
    key: &str,
    status: hoi4_state::CharacterRuntimeStatus,
) {
    if country.is_none() {
        return;
    }
    let i = country.0 as usize;
    if let Some(map) = world.countries.characters.get_mut(i) {
        map.insert(key.to_owned(), status);
    }
}

/// Infer the effect target for cascaded `TriggerEvent` effects from an event id prefix.
/// Falls back to the current event country for neutral ids like `news.*`, `hidden.*`, or `spain.*`.
fn infer_event_effect_country(event_id: &str, world: &World, current: CountryId) -> CountryId {
    let Some((prefix, _)) = event_id.split_once('.') else {
        return current;
    };
    if matches!(prefix, "news" | "hidden" | "spain") {
        return current;
    }
    if let Some(cid) = world.country(prefix) {
        return cid;
    }
    let upper = prefix.to_uppercase();
    world.country(&upper).unwrap_or(current)
}

fn event_should_display_on_current_country(event_id: &str) -> bool {
    let Some((prefix, _)) = event_id.split_once('.') else {
        return true;
    };
    matches!(prefix, "news" | "hidden" | "spain")
}

/// P1.3：执行外交效果并收集错误到报告。
fn apply_diplomatic_effect_report(
    report: &mut EffectReport,
    label: &str,
    world: &mut World,
    effect: &ScriptedDiplomaticEffect,
) {
    if let Err(err) = hoi4_state::apply_diplomatic_effect(world, effect) {
        report.errors.push(format!("{label}: {err:?}"));
    }
}

fn add_v6_building(
    world: &mut World,
    state: hoi4_state::StateId,
    def_id: &str,
    kind: hoi4_state::BuildingKind,
    level: i32,
) {
    if level == 0 {
        return;
    }
    if let Some(existing) = world
        .countries
        .buildings_v6
        .buildings
        .iter_mut()
        .find(|building| building.state == state && building.building_def_id == def_id)
    {
        let next = (existing.level as i32 + level).max(0) as u8;
        existing.level = next;
        return;
    }
    if level < 0 {
        return;
    }
    world
        .countries
        .buildings_v6
        .buildings
        .push(hoi4_state::Building {
            kind,
            building_def_id: def_id.to_owned(),
            state,
            level: level as u8,
            active_pm: "default".to_owned(),
            employment: [0; 6],
            owner: hoi4_state::BuildingOwner::State,
            requires_law: None,
            built_progress: 1.0,
            ..hoi4_state::Building::runtime_defaults()
        });
}

fn infer_building_kind(def_id: &str) -> hoi4_state::BuildingKind {
    match def_id {
        "arms_industry" | "munition_plant" | "shipyard" | "aircraft_factory" => {
            hoi4_state::BuildingKind::Military
        }
        "coal_mine" | "iron_mine" | "oil_rig" | "rubber_plantation" | "bauxite_mine"
        | "chromium_mine" | "tungsten_mine" => hoi4_state::BuildingKind::Resource,
        "grain_farm" | "livestock_ranch" => hoi4_state::BuildingKind::Agriculture,
        "textile_mill" | "food_processing" => hoi4_state::BuildingKind::ConsumerGoods,
        "bank" | "office" => hoi4_state::BuildingKind::Service,
        "railway" | "port" => hoi4_state::BuildingKind::Infrastructure,
        _ => hoi4_state::BuildingKind::Industrial,
    }
}

fn parse_law_category(category: &str) -> Option<LawCategory> {
    match category {
        "Conscription" | "conscription" => Some(LawCategory::Conscription),
        "Economy" | "economy" => Some(LawCategory::Economy),
        "Trade" | "trade" => Some(LawCategory::Trade),
        "Taxation" | "taxation" => Some(LawCategory::Taxation),
        "CivilRights" | "civil_rights" => Some(LawCategory::CivilRights),
        "InformationControl" | "information_control" => Some(LawCategory::InformationControl),
        _ => None,
    }
}

fn first_owned_state(world: &World, country: CountryId) -> Option<StateId> {
    (0..world.states.count)
        .find(|&si| world.states.owners[si] == country)
        .map(|si| StateId(si as u16))
}

#[cfg(test)]
mod tests {
    use super::*;
    use hoi4_state::store::*;
    use hoi4_state::*;
    use std::collections::HashMap;
    use std::sync::Arc;

    fn test_world() -> World {
        // Minimal GameMap
        let map = Arc::new(hoi4_map::GameMap {
            definitions: vec![],
            rgb_to_id: HashMap::new(),
            province_map: hoi4_map::ProvinceMap {
                width: 1,
                height: 1,
                pixels: vec![0],
            },
            adjacencies: vec![],
            special_adjacencies: vec![],
            heightmap: hoi4_map::Heightmap {
                width: 1,
                height: 1,
                pixels: vec![0],
            },
            terrain_bmp: hoi4_map::TerrainBitmap {
                width: 1,
                height: 1,
                pixels: vec![0],
                palette: [[0u8; 3]; 256],
            },
            terrain_catalog: hoi4_map::TerrainCatalog::default(),
            tree_definition_bmp: None,
            tree_indices: std::collections::HashSet::new(),
        });

        let data = Arc::new(hoi4_data::GameData {
            countries: HashMap::new(),
            states: vec![],
            province_owners: HashMap::new(),
            buildings: HashMap::new(),
            resources: HashMap::new(),
            equipment: HashMap::new(),
            technologies: HashMap::new(),
            tech_prereqs: HashMap::new(),
            ideologies: HashMap::new(),
            ideas: HashMap::new(),
            focus_trees: HashMap::new(),
            focus_to_tree: HashMap::new(),
            subunits: HashMap::new(),
            combat_tactics: HashMap::new(),
            division_templates: HashMap::new(),
            ship_classes: HashMap::new(),
            aircraft: HashMap::new(),
            oob_land: HashMap::new(),
            oob_naval: HashMap::new(),
            oob_air: HashMap::new(),
            country_histories: HashMap::new(),
            decision_categories: HashMap::new(),
            decisions: HashMap::new(),
            ..Default::default()
        });

        let mut countries = CountryStore::new(2);
        countries.tags[0] = "GER".to_owned();
        countries.tags[1] = "POL".to_owned();
        countries.political_power[0] = 100.0;
        countries.stability[0] = 0.6;
        countries.war_support[0] = 0.4;
        countries.ruling_party[0] = "fascism".to_owned();
        countries.pops.groups.push(hoi4_state::PopGroup {
            class: hoi4_state::PopClass::Soldier,
            state: StateId(0),
            size: 500_000,
            employed_at: None,
            wage_rm: 0.0,
            tax_burden: 0.0,
            income_rm: 0.0,
            tax_paid_rm: 0.0,
            disposable_income_rm: 0.0,
            basic_consumption_budget: 0.0,
            satisfaction_law_modifier: 0.0,
            loyalty_coefficient: 1.0,
            loyalty_decay_mult: 1.0,
            satisfaction: 1.0,
            political_loyalty: 0.5,
            literacy: hoi4_state::PopClass::Soldier.baseline_literacy(),
            skilled_ratio: hoi4_state::PopClass::Soldier.baseline_skilled_ratio(),
            standard_of_living: 0.5,
            needs_fulfillment: 1.0,
            essential_needs_fulfillment: 1.0,
            normal_needs_fulfillment: 1.0,
            luxury_needs_fulfillment: 1.0,
            radicalism: 0.0,
        });
        countries.army_xp[0] = 10.0;

        countries.research_slots[0] = 3;
        countries.party_popularity[0].insert("fascism".to_owned(), 0.7);
        countries.completed_focuses[0].insert("GER_rhineland".to_owned());
        countries.ideas[0].push("military_staff".to_owned());

        let mut states = StateStore::new(1);
        states.owners[0] = CountryId(0);
        states.controllers[0] = CountryId(0);
        countries.buildings_v6.buildings.push(Building {
            kind: BuildingKind::Industrial,
            building_def_id: "steel_mill".to_owned(),
            state: StateId(0),
            level: 3,
            active_pm: "default".to_owned(),
            employment: [0; 6],
            owner: BuildingOwner::State,
            requires_law: None,
            built_progress: 1.0,
            ..Building::runtime_defaults()
        });
        countries.buildings_v6.buildings.push(Building {
            kind: BuildingKind::Military,
            building_def_id: "arms_industry".to_owned(),
            state: StateId(0),
            level: 2,
            active_pm: "default".to_owned(),
            employment: [0; 6],
            owner: BuildingOwner::State,
            requires_law: None,
            built_progress: 1.0,
            ..Building::runtime_defaults()
        });

        let mut tag_to_country = HashMap::new();
        tag_to_country.insert("GER".to_owned(), CountryId(0));
        tag_to_country.insert("POL".to_owned(), CountryId(1));

        let mut state_id_lookup = HashMap::new();
        state_id_lookup.insert(50u16, StateId(0));

        World {
            date: GameDate::START,
            speed: GameSpeed::Paused,
            elapsed_hours: 0,
            provinces: ProvinceStore::new(0),
            states,
            countries,
            divisions: DivisionStore::new(),
            ships: ShipStore::new(),
            fleets: FleetStore::new(),
            air_wings: AirWingStore::new(),
            diplomacy: DiplomacyState::new(),
            command: hoi4_state::CommandHierarchy::default(),
            map,
            data,
            tag_to_country,
            state_id_lookup,
            player: CountryId(0),
            random_seed: 0,
            game_unique_id: 0,
            path_cache: HashMap::new(),
            path_cache_day: 0,
            prov_div_index: HashMap::new(),
            country_state_index: Vec::new(),
            country_pop_index: Vec::new(),
            country_building_index: Vec::new(),
            country_division_index: Vec::new(),
            country_fleet_index: Vec::new(),
            country_air_wing_index: Vec::new(),
            runtime_country_indexes_valid: false,
            trade_export_surplus_index: HashMap::new(),
            player_armies: Vec::new(),
            player_locked_divisions: std::collections::HashSet::new(),
            next_army_id: 0,
            generals: Vec::new(),
            next_general_id: 0,
        }
    }

    #[test]
    fn trigger_always() {
        let w = test_world();
        let f = GlobalFlags::default();
        let ger = CountryId(0);
        assert!(eval_trigger(&Trigger::AlwaysTrue, &w, ger, &f));
        assert!(!eval_trigger(&Trigger::AlwaysFalse, &w, ger, &f));
    }

    #[test]
    fn trigger_political_power() {
        let w = test_world();
        let f = GlobalFlags::default();
        let ger = CountryId(0);
        assert!(eval_trigger(&Trigger::PoliticalPower(50.0), &w, ger, &f));
        assert!(!eval_trigger(&Trigger::PoliticalPower(200.0), &w, ger, &f));
    }

    #[test]
    fn trigger_has_completed_focus() {
        let w = test_world();
        let f = GlobalFlags::default();
        let ger = CountryId(0);
        assert!(eval_trigger(
            &Trigger::HasCompletedFocus("GER_rhineland".into()),
            &w,
            ger,
            &f
        ));
        assert!(!eval_trigger(
            &Trigger::HasCompletedFocus("GER_anschluss".into()),
            &w,
            ger,
            &f
        ));
    }

    #[test]
    fn trigger_and_or_not() {
        let w = test_world();
        let f = GlobalFlags::default();
        let ger = CountryId(0);
        let t = Trigger::And(vec![
            Trigger::PoliticalPower(50.0),
            Trigger::Or(vec![Trigger::HasWar(true), Trigger::Stability(0.5)]),
            Trigger::Not(Box::new(Trigger::HasGovernment("democratic".into()))),
        ]);
        assert!(eval_trigger(&t, &w, ger, &f));
    }

    #[test]
    fn trigger_date() {
        let w = test_world();
        let f = GlobalFlags::default();
        let ger = CountryId(0);
        // World date is 1936.1.1
        assert!(eval_trigger(
            &Trigger::Date {
                year: 1936,
                month: 1,
                day: 1
            },
            &w,
            ger,
            &f
        ));
        assert!(!eval_trigger(
            &Trigger::Date {
                year: 1937,
                month: 1,
                day: 1
            },
            &w,
            ger,
            &f
        ));
    }

    #[test]
    fn trigger_num_factories() {
        let w = test_world();
        let f = GlobalFlags::default();
        let ger = CountryId(0);
        assert!(eval_trigger(&Trigger::NumOfFactories(5), &w, ger, &f));
        assert!(!eval_trigger(&Trigger::NumOfFactories(6), &w, ger, &f));
        assert!(eval_trigger(
            &Trigger::NumOfCivilianFactories(3),
            &w,
            ger,
            &f
        ));
        assert!(!eval_trigger(
            &Trigger::NumOfCivilianFactories(4),
            &w,
            ger,
            &f
        ));
        assert!(eval_trigger(
            &Trigger::NumOfMilitaryFactories(2),
            &w,
            ger,
            &f
        ));
        assert!(!eval_trigger(
            &Trigger::NumOfMilitaryFactories(3),
            &w,
            ger,
            &f
        ));
    }

    #[test]
    fn trigger_global_flag() {
        let w = test_world();
        let mut f = GlobalFlags::default();
        let ger = CountryId(0);
        assert!(!eval_trigger(
            &Trigger::HasGlobalFlag("test".into()),
            &w,
            ger,
            &f
        ));
        f.flags.insert("test".to_owned());
        assert!(eval_trigger(
            &Trigger::HasGlobalFlag("test".into()),
            &w,
            ger,
            &f
        ));
    }

    #[test]
    fn effect_add_pp() {
        let mut w = test_world();
        let mut f = GlobalFlags::default();
        let ger = CountryId(0);
        run_effects(&[Effect::AddPoliticalPower(50.0)], &mut w, ger, &mut f);
        assert_eq!(w.countries.political_power[0], 150.0);
    }

    #[test]
    fn effect_add_stability() {
        let mut w = test_world();
        let mut f = GlobalFlags::default();
        let ger = CountryId(0);
        run_effects(&[Effect::AddStability(0.1)], &mut w, ger, &mut f);
        assert!((w.countries.stability[0] - 0.7).abs() < 0.001);
    }

    #[test]
    fn effect_flags() {
        let mut w = test_world();
        let mut f = GlobalFlags::default();
        let ger = CountryId(0);
        run_effects(&[Effect::SetGlobalFlag("test".into())], &mut w, ger, &mut f);
        assert!(f.flags.contains("test"));
        run_effects(
            &[Effect::ClearGlobalFlag("test".into())],
            &mut w,
            ger,
            &mut f,
        );
        assert!(!f.flags.contains("test"));
    }

    #[test]
    fn effect_ideas() {
        let mut w = test_world();
        let mut f = GlobalFlags::default();
        let ger = CountryId(0);
        run_effects(&[Effect::AddIdea("new_idea".into())], &mut w, ger, &mut f);
        assert!(w.countries.ideas[0].contains(&"new_idea".to_owned()));
        run_effects(
            &[Effect::RemoveIdea("new_idea".into())],
            &mut w,
            ger,
            &mut f,
        );
        assert!(!w.countries.ideas[0].contains(&"new_idea".to_owned()));
    }

    #[test]
    fn effect_transfer_state() {
        let mut w = test_world();
        let mut f = GlobalFlags::default();
        let pol = CountryId(1);
        // state 50 is owned by GER(0), transfer to POL(1)
        run_effects(&[Effect::TransferState(50)], &mut w, pol, &mut f);
        assert_eq!(w.states.owners[0], pol);
    }

    #[test]
    fn effect_transfer_state_to() {
        let mut w = test_world();
        let mut f = GlobalFlags::default();
        let ger = CountryId(0);
        run_effects(
            &[Effect::TransferStateTo {
                state: 50,
                country: "POL".into(),
            }],
            &mut w,
            ger,
            &mut f,
        );
        assert_eq!(w.states.owners[0], CountryId(1));
    }

    #[test]
    fn effect_create_country() {
        let mut w = test_world();
        let mut f = GlobalFlags::default();
        let ger = CountryId(0);
        let report = run_effects(
            &[Effect::CreateCountry {
                tag: "MOL".into(),
                color: [120, 80, 40],
                ruling_party: "neutrality".into(),
            }],
            &mut w,
            ger,
            &mut f,
        );
        let mol = w.country("MOL").expect("new country should be registered");
        let mi = mol.0 as usize;
        assert_eq!(w.countries.tags[mi], "MOL");
        assert_eq!(w.countries.colors[mi], [120, 80, 40]);
        assert_eq!(w.countries.ruling_party[mi], "neutrality");
        assert!(!report.has_errors());
    }

    #[test]
    fn effect_research_slot() {
        let mut w = test_world();
        let mut f = GlobalFlags::default();
        let ger = CountryId(0);
        run_effects(&[Effect::AddResearchSlot(1)], &mut w, ger, &mut f);
        assert_eq!(w.countries.research_slots[0], 4);
    }

    #[test]
    fn effect_if_conditional() {
        let mut w = test_world();
        let mut f = GlobalFlags::default();
        let ger = CountryId(0);
        let e = Effect::If {
            trigger: Trigger::PoliticalPower(50.0),
            effects: vec![Effect::AddStability(0.1)],
        };
        run_effects(&[e], &mut w, ger, &mut f);
        assert!((w.countries.stability[0] - 0.7).abs() < 0.001);
    }

    #[test]
    fn effect_create_faction() {
        let mut w = test_world();
        let mut f = GlobalFlags::default();
        let ger = CountryId(0);
        run_effects(&[Effect::CreateFaction("Axis".into())], &mut w, ger, &mut f);
        assert_eq!(w.diplomacy.factions.len(), 1);
        assert_eq!(w.diplomacy.factions[0].name, "Axis");
        assert!(w.diplomacy.faction_of(ger).is_some());
    }

    /// P1.3 验收：无效 tag 的外交效果产生错误报告
    #[test]
    fn p13_invalid_tag_effect_produces_error_report() {
        let mut w = test_world();
        let mut f = GlobalFlags::default();
        let ger = CountryId(0);
        // AddOpinion 指向不存在的国家 → 产生警告
        let report = run_effects(
            &[Effect::AddOpinion {
                target: "NONEXISTENT".into(),
                amount: 50,
            }],
            &mut w,
            ger,
            &mut f,
        );
        assert!(report.has_warnings());
        assert!(report.warnings.iter().any(|w| w.contains("NONEXISTENT")));
    }

    /// P1.3 验收：无效 tag 的 AnnexCountry 产生错误报告
    #[test]
    fn p13_invalid_tag_annex_produces_error_report() {
        let mut w = test_world();
        let mut f = GlobalFlags::default();
        let ger = CountryId(0);
        let report = run_effects(
            &[Effect::AnnexCountry("NONEXISTENT".into())],
            &mut w,
            ger,
            &mut f,
        );
        assert!(report.has_errors());
    }

    /// P1.3 验收：成功效果不产生错误
    #[test]
    fn p13_successful_effect_no_errors() {
        let mut w = test_world();
        let mut f = GlobalFlags::default();
        let ger = CountryId(0);
        let report = run_effects(&[Effect::AddPoliticalPower(50.0)], &mut w, ger, &mut f);
        assert!(!report.has_errors());
        assert!(!report.has_warnings());
    }

    /// P1.3 验收：PP 不足切法律返回中文错误
    #[test]
    fn p13_insufficient_pp_law_returns_chinese_error() {
        let mut w = test_world();
        let db = crate::V6Database::load();
        let result = crate::set_law(
            &mut w,
            CountryId(0),
            hoi4_state::LawCategory::Economy,
            "planned_economy",
            &db,
        );
        assert!(result.is_err());
        let err_msg = result.unwrap_err();
        assert!(err_msg.contains("政治力量不足") || err_msg.contains("PP"));
    }
}
