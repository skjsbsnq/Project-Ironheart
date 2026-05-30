//! NationalIntent 战略层：每 7 天评估一次国家的"战略意图"。
//!
//! 这是两层 AI 的上层：读取 AiProfile + 世界状态 → 产出 NationalIntent，
//! 指导下层（production / recruit / ground）的目标导向决策。

use std::collections::HashMap;

use hoi4_state::{CountryId, World};

use crate::ground::GroundPosture;
use crate::profile::AiProfile;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NationalStance {
    BuildingUp,
    Defending { primary_threat: CountryId },
    Attacking { primary_target: CountryId },
    MoppingUp,
}

#[derive(Debug, Clone)]
pub enum BuildFocus {
    Industry,
    Military,
    Balanced,
}

#[derive(Debug, Clone)]
pub struct FrontAssignment {
    pub posture: GroundPosture,
    pub target_div_count: u32,
    pub assigned_divs: u32,
}

#[derive(Debug, Clone)]
pub struct NationalIntent {
    pub stance: NationalStance,
    pub build_focus: BuildFocus,
    pub target_divisions: u32,
    pub target_civ_factories: u32,
    pub target_mil_factories: u32,
    pub front_assignments: HashMap<CountryId, FrontAssignment>,
    pub last_updated: i64,
}

impl NationalIntent {
    pub fn default_at_peace(profile: &AiProfile) -> Self {
        Self {
            stance: NationalStance::BuildingUp,
            build_focus: BuildFocus::Balanced,
            target_divisions: profile.target_div_count_peace,
            target_civ_factories: profile.target_civ_factories,
            target_mil_factories: profile.target_mil_factories,
            front_assignments: HashMap::new(),
            last_updated: 0,
        }
    }
}

pub fn evaluate_intent(
    world: &World,
    country: CountryId,
    profile: &AiProfile,
    last_postures: &HashMap<u16, (GroundPosture, i64)>,
    day: i64,
) -> NationalIntent {
    let ci = country.0 as usize;
    if ci >= world.countries.count {
        return NationalIntent::default_at_peace(profile);
    }

    let at_war = world.countries.at_war[ci];
    let current_divs = count_divisions(world, country);
    let (civ, mil, _dock) = world.country_industry(country);

    let (stance, target_divs) = if !at_war {
        (NationalStance::BuildingUp, profile.target_div_count_peace)
    } else if last_postures.is_empty() {
        (NationalStance::MoppingUp, profile.target_div_count_war)
    } else {
        let any_attack = last_postures
            .values()
            .any(|(p, _)| *p == GroundPosture::Attack);
        let force_attack_enemy = last_postures.keys().find(|&eid| {
            let enemy_tag = world
                .countries
                .tags
                .get(*eid as usize)
                .map(|s| s.as_str())
                .unwrap_or("");
            profile.force_attack_against.iter().any(|t| t == enemy_tag)
        });

        if force_attack_enemy.is_some() {
            let enemy = CountryId(*force_attack_enemy.unwrap());
            (
                NationalStance::Attacking {
                    primary_target: enemy,
                },
                profile.target_div_count_war,
            )
        } else if any_attack {
            let primary = last_postures
                .iter()
                .filter(|(_, (p, _))| *p == GroundPosture::Attack)
                .max_by_key(|(_, (_, d))| *d)
                .map(|(&eid, _)| CountryId(eid));
            match primary {
                Some(enemy) => (
                    NationalStance::Attacking {
                        primary_target: enemy,
                    },
                    profile.target_div_count_war,
                ),
                None => (
                    NationalStance::Defending {
                        primary_threat: CountryId(*last_postures.keys().next().unwrap()),
                    },
                    profile.target_div_count_war,
                ),
            }
        } else {
            let primary = last_postures
                .iter()
                .min_by_key(|(_, (p, _))| match p {
                    GroundPosture::Retreat => 0,
                    GroundPosture::Defend => 1,
                    _ => 2,
                })
                .map(|(&eid, _)| CountryId(eid));
            match primary {
                Some(enemy) => (
                    NationalStance::Defending {
                        primary_threat: enemy,
                    },
                    profile.target_div_count_war,
                ),
                None => (NationalStance::MoppingUp, profile.target_div_count_war),
            }
        }
    };

    let build_focus = if at_war {
        BuildFocus::Military
    } else if world.date.year >= profile.mil_focus_year {
        BuildFocus::Military
    } else if civ as f32 / (mil as f32).max(1.0) < profile.civ_to_mil_ratio {
        BuildFocus::Industry
    } else {
        BuildFocus::Balanced
    };

    let target_civ = if at_war {
        (profile.target_civ_factories as f32 * 0.8) as u32
    } else {
        profile.target_civ_factories
    };
    let target_mil = if at_war {
        (profile.target_mil_factories as f32 * 1.5) as u32
    } else {
        profile.target_mil_factories
    };

    let mut front_assignments = HashMap::new();
    if at_war && !last_postures.is_empty() {
        let n_enemies = last_postures.len().max(1);
        let divs_per_front = (target_divs as f32 / n_enemies as f32).ceil() as u32;
        // 修复 #6：assigned_divs 必须在所有前线上累计 ≤ current_divs，
        // 否则下游模块据此决策时会高估总兵力。按前线数均分当前师，
        // 整除剩余的余数发给迭代到的前几个前线。
        let base_per_front = current_divs / n_enemies as u32;
        let mut leftover = current_divs - base_per_front * n_enemies as u32;

        for (&enemy_id, &(posture, _)) in last_postures.iter() {
            let enemy = CountryId(enemy_id);
            let is_force_attack = profile.force_attack_against.iter().any(|t| {
                world
                    .countries
                    .tags
                    .get(enemy_id as usize)
                    .map(|s| s.as_str())
                    == Some(t.as_str())
            });
            let assigned = if is_force_attack {
                divs_per_front * 2
            } else {
                divs_per_front
            };
            let extra = if leftover > 0 { 1 } else { 0 };
            leftover = leftover.saturating_sub(extra);
            let assigned_now = base_per_front + extra;
            front_assignments.insert(
                enemy,
                FrontAssignment {
                    posture,
                    target_div_count: assigned,
                    assigned_divs: assigned_now,
                },
            );
        }
    }

    NationalIntent {
        stance,
        build_focus,
        target_divisions: target_divs,
        target_civ_factories: target_civ,
        target_mil_factories: target_mil,
        front_assignments,
        last_updated: day,
    }
}

fn count_divisions(world: &World, country: CountryId) -> u32 {
    world
        .divisions
        .owners
        .iter()
        .take(world.divisions.count)
        .filter(|&&o| o == country)
        .count() as u32
}
