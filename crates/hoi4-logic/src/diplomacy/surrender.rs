use hoi4_content::{ScriptedSurrenderDef, SituationEffect};
use hoi4_state::{CountryId, World};

use crate::scripted_effects::{apply_situation_diplomatic_effect, ScriptedEffectError};

const SURRENDER_MARKER_PREFIX: &str = "SURRENDER:";
const COUNTRY_FLAG_PREFIX: &str = "FLAG:";

#[derive(Debug, Clone, PartialEq)]
pub struct SurrenderEvaluation {
    pub def_id: String,
    pub target: String,
    pub capital_lost: bool,
    pub target_core_control_ratio: f32,
    pub threshold: f32,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SurrenderOutcome {
    pub resolved: Vec<SurrenderEvaluation>,
    /// P1.2：触发的事件附带作用国家信息。
    pub triggered_events: Vec<TriggeredEvent>,
    pub effect_errors: Vec<String>,
}

/// P1.2：投降触发的事件，保留作用国家。
#[derive(Debug, Clone, PartialEq)]
pub struct TriggeredEvent {
    pub event_id: String,
    /// 效果执行上下文国家（如法国投降事件中法国）。
    pub effect_country: String,
}

pub fn evaluate_scripted_surrender(
    world: &World,
    def: &ScriptedSurrenderDef,
) -> Option<SurrenderEvaluation> {
    let target = world.country(&def.target)?;
    if has_surrender_marker(world, target, &def.id)
        || world.diplomacy.annexed_countries.contains(&target)
    {
        return None;
    }
    if !world.diplomacy.is_at_war(target) {
        return None;
    }

    let enemy_countries: Vec<CountryId> = def
        .enemy_side
        .iter()
        .filter_map(|tag| world.country(tag))
        .collect();
    if enemy_countries.is_empty() {
        return None;
    }

    let capital_lost = capital_lost_to_enemy(world, target, &enemy_countries);
    if def.require_capital_lost && !capital_lost {
        return None;
    }

    let target_core_control_ratio = target_core_control_ratio(world, target);
    if target_core_control_ratio > def.max_target_core_control_ratio {
        return None;
    }

    Some(SurrenderEvaluation {
        def_id: def.id.clone(),
        target: def.target.clone(),
        capital_lost,
        target_core_control_ratio,
        threshold: def.max_target_core_control_ratio,
    })
}

pub fn apply_scripted_surrenders(
    world: &mut World,
    defs: &[ScriptedSurrenderDef],
) -> SurrenderOutcome {
    let mut outcome = SurrenderOutcome::default();
    for def in defs {
        let Some(eval) = evaluate_scripted_surrender(world, def) else {
            continue;
        };
        let Some(target) = world.country(&def.target) else {
            continue;
        };

        mark_surrender(world, target, &def.id);
        if def.remove_from_wars {
            // P1.1：使用统一结算出口，删除空战争并重算 at_war
            resolve_surrender_war_cleanup(world, target);
        }

        // P1.2：先推入 resolved，以便 apply_surrender_effect 能读到 target
        outcome.resolved.push(eval);

        for effect in &def.effects {
            apply_surrender_effect(world, effect, &mut outcome);
        }
    }
    outcome
}

fn apply_surrender_effect(
    world: &mut World,
    effect: &SituationEffect,
    outcome: &mut SurrenderOutcome,
) {
    if let Some(result) = apply_situation_diplomatic_effect(world, effect) {
        if let Err(err) = result {
            outcome.effect_errors.push(format_effect_error(err));
        }
        return;
    }

    match effect {
        SituationEffect::TriggerEvent(event_id) => {
            // P1.2：投降触发的事件，作用国家为投降目标
            let effect_country = outcome
                .resolved
                .last()
                .map(|r| r.target.clone())
                .unwrap_or_default();
            outcome.triggered_events.push(TriggeredEvent {
                event_id: event_id.clone(),
                effect_country,
            });
        }
        SituationEffect::AddIdea { country, idea } => {
            if let Some(cid) = world.country(country) {
                let ci = cid.0 as usize;
                if ci < world.countries.count && !world.countries.ideas[ci].contains(idea) {
                    world.countries.ideas[ci].push(idea.clone());
                }
            }
        }
        SituationEffect::AddStability { country, amount } => {
            if let Some(cid) = world.country(country) {
                let ci = cid.0 as usize;
                if ci < world.countries.count {
                    world.countries.stability[ci] =
                        (world.countries.stability[ci] + amount).clamp(0.0, 1.0);
                }
            }
        }
        SituationEffect::AddWarSupport { country, amount } => {
            if let Some(cid) = world.country(country) {
                let ci = cid.0 as usize;
                if ci < world.countries.count {
                    world.countries.war_support[ci] =
                        (world.countries.war_support[ci] + amount).clamp(0.0, 1.0);
                }
            }
        }
        SituationEffect::AddOpinion {
            country_a,
            country_b,
            amount,
        } => {
            if let (Some(a), Some(b)) = (world.country(country_a), world.country(country_b)) {
                world.diplomacy.opinions.modify(a, b, *amount);
                world.diplomacy.opinions.modify(b, a, *amount);
            }
        }
        SituationEffect::SetCountryFlag { country, flag } => {
            if let Some(cid) = world.country(country) {
                let ci = cid.0 as usize;
                if ci < world.countries.count {
                    let flag = format!("{COUNTRY_FLAG_PREFIX}{flag}");
                    if !world.countries.ideas[ci].contains(&flag) {
                        world.countries.ideas[ci].push(flag);
                    }
                }
            }
        }
        _ => {}
    }
}

fn capital_lost_to_enemy(world: &World, target: CountryId, enemy_countries: &[CountryId]) -> bool {
    let ci = target.0 as usize;
    if ci >= world.countries.count {
        return false;
    }
    let capital = world.countries.capitals[ci];
    if capital.is_none() {
        return false;
    }
    let si = capital.0 as usize;
    if si >= world.states.count {
        return false;
    }
    enemy_countries
        .iter()
        .any(|&enemy| world.states.controllers[si] == enemy)
}

fn target_core_control_ratio(world: &World, target: CountryId) -> f32 {
    let mut total = 0u32;
    let mut controlled = 0u32;
    for si in 0..world.states.count {
        if world.states.cores[si].contains(&target) {
            total += 1;
            if world.states.controllers[si] == target {
                controlled += 1;
            }
        }
    }

    if total == 0 {
        for si in 0..world.states.count {
            if world.states.owners[si] == target {
                total += 1;
                if world.states.controllers[si] == target {
                    controlled += 1;
                }
            }
        }
    }

    if total == 0 {
        0.0
    } else {
        controlled as f32 / total as f32
    }
}

fn remove_country_from_wars(world: &mut World, country: CountryId) {
    for war in world.diplomacy.wars.values_mut() {
        war.attackers.remove(&country);
        war.defenders.remove(&country);
    }
    let ci = country.0 as usize;
    if ci < world.countries.count {
        world.countries.at_war[ci] = world.diplomacy.is_at_war(country);
    }
}

/// P1.1：统一战争结算入口。
///
/// scripted surrender 和普通战争结算共用此出口。
/// 把国家从所有战争中移除后，调用统一清理删除空战争。
pub fn resolve_surrender_war_cleanup(
    world: &mut World,
    country: CountryId,
) -> SurrenderWarCleanupResult {
    let mut result = SurrenderWarCleanupResult::default();
    remove_country_from_wars(world, country);

    // 收集空战争（国家被移除后可能某方为空）
    let empty_war_ids: Vec<u32> = world
        .diplomacy
        .wars
        .iter()
        .filter(|(_, war)| war.attackers.is_empty() || war.defenders.is_empty())
        .map(|(&id, _)| id)
        .collect();
    result.wars_cleaned = empty_war_ids.len() as u32;

    for war_id in empty_war_ids {
        let _ = super::war::cleanup_war(world, war_id);
    }

    for ci in 0..world.countries.count {
        let cid = CountryId(ci as u16);
        world.countries.at_war[ci] = world.diplomacy.is_at_war(cid);
    }

    result
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SurrenderWarCleanupResult {
    pub wars_cleaned: u32,
}

fn has_surrender_marker(world: &World, country: CountryId, id: &str) -> bool {
    let ci = country.0 as usize;
    ci < world.countries.count
        && world.countries.ideas[ci].contains(&format!("{SURRENDER_MARKER_PREFIX}{id}"))
}

fn mark_surrender(world: &mut World, country: CountryId, id: &str) {
    let ci = country.0 as usize;
    if ci >= world.countries.count {
        return;
    }
    let marker = format!("{SURRENDER_MARKER_PREFIX}{id}");
    if !world.countries.ideas[ci].contains(&marker) {
        world.countries.ideas[ci].push(marker);
    }
}

fn format_effect_error(err: ScriptedEffectError) -> String {
    format!("{err:?}")
}
