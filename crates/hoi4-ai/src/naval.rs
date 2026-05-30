//! 海军战术决策：为每个舰队分配 [`NavalMission`]。
//!
//! 简化模型——基于舰队组成 + 战争状态 + 海区的启发式：
//! - 主力为运输舰：保持 Idle（等候 invasion 计划）
//! - 主力为潜艇：ConvoyRaiding（攻击敌商船）
//! - 主力舰 / 航母 + 战争 + 同海区有敌：StrikeForce；否则 Patrol
//! - 屏卫主力：ConvoyEscort
//! - 其它在战时：Patrol；和平：Idle

use hoi4_data::ShipKind;
use hoi4_state::{CountryId, FleetId, NavalMission, World};

use crate::profile::AiProfile;

/// 一个舰队的任务决定。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavalDecision {
    pub fleet: FleetId,
    pub mission: NavalMission,
    pub target_region: Option<u32>,
    pub reason: &'static str,
}

#[derive(Debug, Clone, Default)]
struct FleetComposition {
    capital: u32,
    carrier: u32,
    screen: u32,
    submarine: u32,
    transport: u32,
    other: u32,
}

impl FleetComposition {
    fn total(&self) -> u32 {
        self.capital + self.carrier + self.screen + self.submarine + self.transport + self.other
    }
    fn combat(&self) -> u32 {
        self.capital + self.carrier + self.screen + self.submarine
    }
}

fn fleet_composition(world: &World, fleet_idx: usize) -> FleetComposition {
    let mut c = FleetComposition::default();
    for &s in &world.fleets.ships[fleet_idx] {
        if s.is_none() {
            continue;
        }
        let si = s.0 as usize;
        if si >= world.ships.count {
            continue;
        }
        let class_key = &world.ships.class_keys[si];
        let kind = world
            .data
            .ship_classes
            .get(class_key)
            .map(|cls| cls.kind)
            .unwrap_or(ShipKind::Other);
        match kind {
            ShipKind::Capital => c.capital += 1,
            ShipKind::Carrier => c.carrier += 1,
            ShipKind::Screen => c.screen += 1,
            ShipKind::Submarine => c.submarine += 1,
            ShipKind::Transport => c.transport += 1,
            ShipKind::Other => c.other += 1,
        }
    }
    c
}

/// 同海区是否存在交战敌方舰队。
fn enemy_present_in_region(world: &World, country: CountryId, region: u32) -> bool {
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

fn enemy_regions(world: &World, country: CountryId) -> Vec<u32> {
    let mut regions = Vec::new();
    for fi in 0..world.fleets.count {
        let owner = world.fleets.owners[fi];
        let region = world.fleets.region_id[fi];
        if owner == country || owner.is_none() || region == u32::MAX {
            continue;
        }
        if world.diplomacy.at_war_with(country, owner) && !regions.contains(&region) {
            regions.push(region);
        }
    }
    regions.sort_unstable();
    regions
}

fn nearby_enemy_region(world: &World, country: CountryId, current_region: u32) -> Option<u32> {
    enemy_regions(world, country)
        .into_iter()
        .min_by_key(|region| current_region.abs_diff(*region))
}

/// 评估该国所有舰队的任务决定。
pub fn evaluate_naval(
    world: &World,
    country: CountryId,
    personality: &AiProfile,
) -> Vec<NavalDecision> {
    let mut out = Vec::new();
    if country.is_none() || (country.0 as usize) >= world.countries.count {
        return out;
    }
    let at_war = world.diplomacy.is_at_war(country);
    let china_war = crate::china_theater::is_japan_china_war_active(world, country);
    let invasion_route_regions = if china_war {
        let staging_ports = crate::china_theater::invasion_staging_ports(world, country);
        crate::china_theater::invasion_targets_for_phase(
            world,
            country,
            crate::china_theater::strategy_for(world, country).phase,
        )
        .iter()
        .flat_map(|(target, _, _)| {
            let embark = staging_ports.iter().copied().min_by_key(|p| {
                hoi4_logic::military::movement::land_distance_approx(world, *p, *target)
            });
            match embark {
                Some(ep) => {
                    hoi4_logic::military::movement::port_route_regions_pub(world, ep, *target)
                }
                None => Vec::new(),
            }
        })
        .collect::<std::collections::HashSet<u32>>()
        .into_iter()
        .collect::<Vec<u32>>()
    } else {
        Vec::new()
    };

    for fi in 0..world.fleets.count {
        if world.fleets.owners[fi] != country {
            continue;
        }
        let comp = fleet_composition(world, fi);
        if comp.total() == 0 {
            continue;
        }
        let region = world.fleets.region_id[fi];

        let mission = if china_war
            && !invasion_route_regions.is_empty()
            && comp.capital + comp.carrier > 0
            && at_war
        {
            if invasion_route_regions.contains(&region) {
                NavalMission::NavalInvasionSupport
            } else {
                NavalMission::Patrol
            }
        } else if should_support_naval_invasion(world, country) && at_war {
            NavalMission::NavalInvasionSupport
        } else if comp.transport > comp.combat() {
            NavalMission::Idle
        } else if comp.submarine as f32 / comp.total().max(1) as f32
            >= crate::constants::SUBMARINE_RAIDING_RATIO
        {
            if at_war {
                NavalMission::ConvoyRaiding
            } else {
                NavalMission::Patrol
            }
        } else if comp.capital + comp.carrier > 0 && at_war {
            if enemy_present_in_region(world, country, region) {
                NavalMission::StrikeForce
            } else {
                NavalMission::Patrol
            }
        } else if comp.screen >= comp.combat().saturating_sub(comp.screen) && at_war {
            NavalMission::ConvoyEscort
        } else if at_war {
            NavalMission::Patrol
        } else {
            NavalMission::Idle
        };

        let target_region = if at_war
            && personality.naval_focus >= 0.5
            && !enemy_present_in_region(world, country, region)
            && !hoi4_logic::naval::movement::is_moving(world, FleetId(fi as u32))
        {
            if china_war && !invasion_route_regions.is_empty() {
                invasion_route_regions
                    .iter()
                    .min_by_key(|r| region.abs_diff(**r))
                    .copied()
            } else {
                nearby_enemy_region(world, country, region)
            }
        } else {
            None
        };

        out.push(NavalDecision {
            fleet: FleetId(fi as u32),
            mission,
            target_region,
            reason: mission_reason(mission),
        });
    }
    out
}

fn should_support_naval_invasion(world: &World, country: CountryId) -> bool {
    for si in 0..world.states.count {
        let controller = world.states.controllers[si];
        if controller == country || controller.is_none() {
            continue;
        }
        if !world.diplomacy.at_war_with(country, controller) {
            continue;
        }
        if world.states.provinces[si].iter().any(|&p| {
            world.map.get_province(p.0).is_some_and(|def| {
                matches!(def.province_type, hoi4_map::ProvinceType::Land) && def.coastal
            })
        }) {
            return true;
        }
    }
    false
}

/// 写回 World：更新每个 fleet 的 mission 字段。
pub fn apply_naval_decisions(world: &mut World, decisions: &[NavalDecision]) {
    for d in decisions {
        if d.fleet.is_none() {
            continue;
        }
        let i = d.fleet.0 as usize;
        if i < world.fleets.count {
            world.fleets.mission[i] = d.mission;
            if let Some(region) = d.target_region {
                let now = world.date.hours_since_epoch().max(0) as u64;
                let _ =
                    hoi4_logic::naval::movement::order_move_to_region(world, d.fleet, region, now);
            }
        }
    }
}

fn mission_reason(m: NavalMission) -> &'static str {
    match m {
        NavalMission::Idle => "no enemy / not at war",
        NavalMission::Patrol => "patrol home waters",
        NavalMission::ConvoyEscort => "screen-heavy escort",
        NavalMission::StrikeForce => "capital force seek battle",
        NavalMission::ConvoyRaiding => "submarine raiding",
        NavalMission::MineLaying => "mining",
        NavalMission::MineSweeping => "sweeping",
        NavalMission::NavalInvasionSupport => "invasion support",
    }
}
