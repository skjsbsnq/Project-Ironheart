//! 外交决策：评估正当化战争目标、宣战、加入阵营等。

use hoi4_state::{CountryId, WargoalType, World};

use crate::constants::*;
use crate::profile::AiProfile;
use crate::scoring::{rank, Scored, Scorer};

/// AI 做出的外交决策。
#[derive(Debug, Clone)]
pub enum DiplomacyDecision {
    /// 正当化战争目标
    JustifyWargoal {
        target: CountryId,
        kind: WargoalType,
    },
    /// 宣战
    DeclareWar { target: CountryId },
    /// 创建阵营
    CreateFaction { name: String },
}

/// 外交评估详情。
#[derive(Debug, Clone)]
pub struct DiplomacyEvaluation {
    pub decision: DiplomacyDecision,
    pub score: f32,
    pub reason: String,
}

/// 评估外交选项，返回按分数降序排列的列表。
///
/// 策略：
/// - 未开战 + 有足够 PP + 紧张度够 → 考虑正当化
/// - 已有 justified wargoal → 考虑宣战
/// - 不在阵营 + 有意识形态相同的强大国家 → 考虑创建阵营
pub fn evaluate_diplomacy(
    world: &World,
    country: CountryId,
    profile: &AiProfile,
) -> Vec<Scored<DiplomacyDecision>> {
    let ci = country.0 as usize;
    if ci >= world.countries.count {
        return Vec::new();
    }

    let is_at_war = world.countries.at_war[ci];
    let pp = world.countries.political_power[ci];
    let tension = world.diplomacy.world_tension;
    let scorer = Scorer::new(world.random_seed.wrapping_add(ci as u64 + 7777));
    let mut results = Vec::new();

    // ─── 正当化战争目标 ───
    if !is_at_war && pp >= 50.0 && tension >= WAR_TENSION_THRESHOLD {
        let my_industry = world.country_industry(country);
        let my_factories = my_industry.0 + my_industry.1 + my_industry.2;
        let my_divisions = count_divisions(world, country);

        for target_ci in 0..world.countries.count {
            let target = CountryId(target_ci as u16);
            if target == country || target == world.player {
                continue;
            }
            if world.countries.at_war[target_ci] {
                continue; // 已在战争中，不作为正当化目标
            }
            if world.diplomacy.at_war_with(country, target) {
                continue;
            }
            if world
                .diplomacy
                .pending_wargoals
                .get(&country)
                .map_or(false, |wgs| {
                    wgs.iter().any(|w| w.target == target && !w.justified)
                })
            {
                continue; // 已有未完成的正当化
            }

            let target_industry = world.country_industry(target);
            let target_factories = target_industry.0 + target_industry.1 + target_industry.2;
            let target_divisions = count_divisions(world, target);

            // 评分：我们比目标强 → 高分
            let strength_ratio = if target_divisions > 0 {
                my_divisions as f32 / target_divisions as f32
            } else {
                5.0
            };
            let industry_ratio = if target_factories > 0 {
                my_factories as f32 / target_factories as f32
            } else {
                3.0
            };

            // 只有足够强大时才正当化
            if strength_ratio < 1.2
                || my_divisions < WAR_MIN_DIVISIONS
                || my_factories < WAR_MIN_FACTORIES
            {
                continue;
            }

            // 意识形态不同 → 更容易正当化
            let ideology_bonus =
                if world.countries.ruling_party[ci] != world.countries.ruling_party[target_ci] {
                    10.0
                } else {
                    0.0
                };

            let opinion = world.diplomacy.opinions.get(country, target);
            let opinion_factor = if opinion < 0 {
                (-opinion as f32 / 100.0).min(1.0) * 10.0
            } else {
                0.0
            };

            let mut score = 20.0
                + ideology_bonus
                + opinion_factor
                + (strength_ratio - 1.0) * 15.0
                + (industry_ratio - 1.0) * 5.0;

            // 好战性格加成
            score *= 0.5 + profile.aggression;

            // 选择 wargoal 类型
            let kind = if profile.aggression > 0.7 && strength_ratio > 2.0 {
                WargoalType::Annex
            } else if profile.aggression > 0.4 {
                WargoalType::Puppet
            } else {
                WargoalType::TakeState
            };

            score = scorer.jitter(score, &format!("justify_{}", target_ci));
            results.push(Scored::new(
                DiplomacyDecision::JustifyWargoal { target, kind },
                score,
                format!(
                    "justify vs {} (str={:.1} ind={:.1} ideol={:.0})",
                    world.countries.tags[target_ci], strength_ratio, industry_ratio, ideology_bonus
                ),
            ));
        }
    }

    // ─── 宣战 ───
    if !is_at_war {
        // 检查是否有已正当化的 wargoal
        if let Some(wgs) = world.diplomacy.pending_wargoals.get(&country) {
            for wg in wgs {
                if !wg.justified {
                    continue;
                }
                let target = wg.target;
                let target_ci = target.0 as usize;
                if target_ci >= world.countries.count {
                    continue;
                }

                let my_industry = world.country_industry(country);
                let my_factories = my_industry.0 + my_industry.1 + my_industry.2;
                let my_divisions = count_divisions(world, country);

                if my_divisions < WAR_MIN_DIVISIONS || my_factories < WAR_MIN_FACTORIES {
                    continue;
                }

                let war_support = world.countries.war_support[ci];
                let mut score = 30.0 + war_support * 20.0;
                score *= 0.5 + profile.aggression;

                score = scorer.jitter(score, &format!("war_{}", target_ci));
                results.push(Scored::new(
                    DiplomacyDecision::DeclareWar { target },
                    score,
                    format!(
                        "declare war on {} (ws={:.2} divs={})",
                        world.countries.tags[target_ci], war_support, my_divisions
                    ),
                ));
            }
        }
    }

    // ─── 创建阵营 ───
    if !is_at_war && world.diplomacy.faction_of(country).is_none() {
        let war_support = world.countries.war_support[ci];
        let my_industry = world.country_industry(country);
        let my_factories = my_industry.0 + my_industry.1 + my_industry.2;

        if my_factories >= 30 && war_support > 0.4 {
            let mut score = 15.0 + war_support * 10.0 + profile.aggression * 10.0;
            score = scorer.jitter(score, "create_faction");
            results.push(Scored::new(
                DiplomacyDecision::CreateFaction {
                    name: format!("{}_faction", world.countries.tags[ci]),
                },
                score,
                format!("create faction (factories={})", my_factories),
            ));
        }
    }

    rank(results)
}

/// 执行外交决策。
pub fn apply_diplomacy_decision(
    world: &mut World,
    country: CountryId,
    decision: &DiplomacyDecision,
) {
    match decision {
        DiplomacyDecision::JustifyWargoal { target, kind } => {
            let _ = hoi4_logic::diplomacy::start_justification(
                world,
                country,
                *target,
                kind.clone(),
                None,
            );
        }
        DiplomacyDecision::DeclareWar { target } => {
            let _ = hoi4_logic::diplomacy::declare_war(world, country, *target);
        }
        DiplomacyDecision::CreateFaction { name } => {
            let _ = hoi4_logic::diplomacy::factions::create_faction(world, country, name.clone());
        }
    }
}

/// 统计某国的师数量。
fn count_divisions(world: &World, country: CountryId) -> u32 {
    let mut count = 0;
    for i in 0..world.divisions.count {
        if world.divisions.owners[i] == country {
            count += 1;
        }
    }
    count
}
