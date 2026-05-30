//! 空军战术决策：为每个空军联队分配 [`AirMission`]。
//!
//! 启发式：根据飞机种类和当前战争 / 制空权状态决定任务。
//! - Fighter / HeavyFighter：制空权劣势 → AirSuperiority；优势 → 跟随同区任务（默认 AirSuperiority）
//! - CAS：有前线 → CloseAirSupport；否则 Idle
//! - TacticalBomber：根据制空权决定（劣势 → AirSuperiority，优势 → CloseAirSupport）
//! - StrategicBomber：StrategicBombing
//! - NavalBomber：NavalStrike（前提是同区有敌舰）；否则 NavalPatrol
//! - TransportPlane：Idle（仅在登陆 / 空投计划时切换）
//!
//! 没有空区的真实地图，target_region 暂时与 region_id 相同。

use hoi4_data::AircraftKind;
use hoi4_state::{AirMission, AirWingId, CountryId, World};

use crate::constants::*;
use crate::profile::AiProfile;

/// 单个空军联队的任务决定。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AirDecision {
    pub wing: AirWingId,
    pub mission: AirMission,
    pub target_region: u32,
    pub reason: &'static str,
}

/// 同区是否存在交战敌方空军（空军联队仍有飞机）。
fn enemy_air_present(world: &World, country: CountryId, region: u32) -> bool {
    if region == u32::MAX {
        return false;
    }
    for i in 0..world.air_wings.count {
        if world.air_wings.region_id[i] != region {
            continue;
        }
        let owner = world.air_wings.owners[i];
        if owner == country || owner.is_none() {
            continue;
        }
        if !world.diplomacy.at_war_with(country, owner) {
            continue;
        }
        if world.air_wings.count_planes[i] > 0 {
            return true;
        }
    }
    false
}

/// 同区是否存在交战敌方舰队（用于 NavalStrike 决策）。
fn enemy_fleet_in_region(world: &World, country: CountryId, region: u32) -> bool {
    if region == u32::MAX {
        return false;
    }
    for fi in 0..world.fleets.count {
        if world.fleets.region_id[fi] != region {
            continue;
        }
        let owner = world.fleets.owners[fi];
        if owner == country || owner.is_none() {
            continue;
        }
        if !world.diplomacy.at_war_with(country, owner) {
            continue;
        }
        for &s in &world.fleets.ships[fi] {
            if s.is_none() {
                continue;
            }
            let si = s.0 as usize;
            if si < world.ships.count && world.ships.hp[si] > 0.0 {
                return true;
            }
        }
    }
    false
}

/// 粗略估算本国在某 region 的制空权（presence-weighted）。
/// 没有真正调用 `AirControl::recompute`（避免拉入完整 GameData 计算），
/// 用一个简化版：本国 plane_count_in_region / 总 plane_count_in_region。
fn local_air_control(world: &World, country: CountryId, region: u32) -> f32 {
    if region == u32::MAX {
        return 0.0;
    }
    let mut mine = 0u32;
    let mut total = 0u32;
    for i in 0..world.air_wings.count {
        if world.air_wings.region_id[i] != region {
            continue;
        }
        let planes = world.air_wings.count_planes[i];
        total += planes;
        if world.air_wings.owners[i] == country {
            mine += planes;
        }
    }
    if total == 0 {
        0.0
    } else {
        mine as f32 / total as f32
    }
}

/// 评估该国全部联队的任务。
pub fn evaluate_air(
    world: &World,
    country: CountryId,
    _personality: &AiProfile,
) -> Vec<AirDecision> {
    let mut out = Vec::new();
    if country.is_none() || (country.0 as usize) >= world.countries.count {
        return out;
    }
    let at_war = world.diplomacy.is_at_war(country);

    for i in 0..world.air_wings.count {
        if world.air_wings.owners[i] != country {
            continue;
        }
        if world.air_wings.count_planes[i] == 0 {
            continue;
        }
        let key = &world.air_wings.aircraft_keys[i];
        let kind = world
            .data
            .aircraft
            .get(key)
            .map(|d| d.kind)
            .unwrap_or(AircraftKind::Other);
        let region = world.air_wings.region_id[i];
        let air_ctrl = local_air_control(world, country, region);
        let enemy_air = enemy_air_present(world, country, region);
        let enemy_fleet = enemy_fleet_in_region(world, country, region);

        let mission = match kind {
            AircraftKind::Fighter | AircraftKind::HeavyFighter => {
                if !at_war {
                    AirMission::Idle
                } else if enemy_air || air_ctrl < AIR_CONTROL_LOW_THRESHOLD {
                    AirMission::AirSuperiority
                } else if air_ctrl >= AIR_CONTROL_HIGH_THRESHOLD {
                    // 优势制空 → 拦截敌轰炸 / 守备
                    AirMission::Interception
                } else {
                    AirMission::AirSuperiority
                }
            }
            AircraftKind::CloseAirSupport => {
                if at_war {
                    AirMission::CloseAirSupport
                } else {
                    AirMission::Idle
                }
            }
            AircraftKind::TacticalBomber => {
                if !at_war {
                    AirMission::Idle
                } else if air_ctrl < AIR_CONTROL_LOW_THRESHOLD {
                    AirMission::AirSuperiority
                } else {
                    AirMission::CloseAirSupport
                }
            }
            AircraftKind::StrategicBomber => {
                if at_war {
                    AirMission::StrategicBombing
                } else {
                    AirMission::Idle
                }
            }
            AircraftKind::NavalBomber => {
                if at_war && enemy_fleet {
                    AirMission::NavalStrike
                } else if at_war {
                    AirMission::NavalPatrol
                } else {
                    AirMission::Idle
                }
            }
            AircraftKind::TransportPlane => AirMission::Idle,
            AircraftKind::Other => {
                if at_war {
                    AirMission::AirSuperiority
                } else {
                    AirMission::Idle
                }
            }
        };

        out.push(AirDecision {
            wing: AirWingId(i as u32),
            mission,
            target_region: region,
            reason: mission_reason(mission),
        });
    }
    out
}

/// 写回 World：更新每个 air_wing 的 mission / target_region。
pub fn apply_air_decisions(world: &mut World, decisions: &[AirDecision]) {
    for d in decisions {
        if d.wing.is_none() {
            continue;
        }
        let i = d.wing.0 as usize;
        if i < world.air_wings.count {
            world.air_wings.mission[i] = d.mission;
            world.air_wings.target_region[i] = d.target_region;
        }
    }
}

fn mission_reason(m: AirMission) -> &'static str {
    match m {
        AirMission::Idle => "no theater / not at war",
        AirMission::AirSuperiority => "contest air zone",
        AirMission::Interception => "intercept enemy bombers",
        AirMission::CloseAirSupport => "support ground front",
        AirMission::StrategicBombing => "bomb enemy industry",
        AirMission::PortStrike => "port strike",
        AirMission::NavalStrike => "naval strike",
        AirMission::NavalPatrol => "naval patrol",
        AirMission::LogisticalStrike => "logistical strike",
        AirMission::Drop => "airdrop",
    }
}
