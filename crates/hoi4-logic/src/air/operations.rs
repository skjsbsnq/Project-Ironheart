//! Airbase transfer, mission execution, and aircraft reinforcement MVP.

use std::collections::HashMap;

use hoi4_data::{AircraftKind, GameData};
use hoi4_state::{AirMission, AirWingId, CountryId, FleetId, StateId, World};

use crate::economy::EconomyState;

use super::air_superiority::AirControl;
use super::regions::{airbase_capacity, can_deploy_to_base, NO_AIR_REGION};
use super::strategic_bombing::{execute_bombing, StrategicBombingTarget};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AirTransferError {
    InvalidWing,
    InvalidBase,
    OverCapacity,
    OutOfRange,
}

#[derive(Debug, Clone, Default)]
pub struct AirTaskReport {
    pub wings_transferred: usize,
    pub planes_reinforced: u32,
    pub wings_created: u32,
    pub buildings_destroyed: u32,
    pub infrastructure_destroyed: u32,
    pub naval_damage: f32,
    pub port_damage: f32,
}

pub fn order_transfer_to_base(
    world: &mut World,
    wing: AirWingId,
    target_base: StateId,
    target_region: u32,
    current_hour: u64,
) -> Result<(), AirTransferError> {
    if wing.is_none() {
        return Err(AirTransferError::InvalidWing);
    }
    let wi = wing.0 as usize;
    if wi >= world.air_wings.count {
        return Err(AirTransferError::InvalidWing);
    }
    if target_base.is_none() || target_base.0 as usize >= world.states.count {
        return Err(AirTransferError::InvalidBase);
    }
    let owner = world.air_wings.owners[wi];
    if !can_deploy_to_base(world, owner, target_base, world.air_wings.count_planes[wi]) {
        return Err(AirTransferError::OverCapacity);
    }
    let current_region = world.air_wings.region_id[wi];
    let distance = if current_region == NO_AIR_REGION {
        1
    } else {
        current_region.abs_diff(target_region).max(1)
    };
    let range = world.air_wings.range_km[wi].max(300.0);
    let max_region_hops = (range / 600.0).ceil().max(1.0) as u32;
    if distance > max_region_hops.saturating_mul(3) {
        return Err(AirTransferError::OutOfRange);
    }
    world.air_wings.target_region[wi] = target_region;
    world.air_wings.base_state[wi] = target_base.0;
    world.air_wings.transfer_arrival_hour[wi] = current_hour + distance as u64 * 6;
    Ok(())
}

pub fn tick_transfers(world: &mut World) -> usize {
    let now = world.date.hours_since_epoch().max(0) as u64;
    let mut moved = 0usize;
    for wi in 0..world.air_wings.count {
        let arrival = world.air_wings.transfer_arrival_hour[wi];
        if arrival == 0 || now < arrival {
            continue;
        }
        let target = world.air_wings.target_region[wi];
        if target != NO_AIR_REGION {
            world.air_wings.region_id[wi] = target;
        }
        world.air_wings.transfer_arrival_hour[wi] = 0;
        moved += 1;
    }
    moved
}

pub fn tick_daily(world: &mut World, econ: &mut EconomyState, data: &GameData) -> AirTaskReport {
    let mut report = AirTaskReport::default();
    report.wings_transferred += tick_transfers(world);
    reinforce_from_stockpile(world, econ, data, &mut report);
    create_wings_from_stockpile(world, econ, data, &mut report);
    execute_daily_missions(world, data, &mut report);
    report
}

fn reinforce_from_stockpile(
    world: &mut World,
    econ: &mut EconomyState,
    data: &GameData,
    report: &mut AirTaskReport,
) {
    for wi in 0..world.air_wings.count {
        if !world.air_wings.reinforce_enabled[wi] {
            continue;
        }
        let owner = world.air_wings.owners[wi];
        if owner.is_none() {
            continue;
        }
        let ci = owner.0 as usize;
        econ.ensure_capacity(ci + 1);
        let missing =
            world.air_wings.max_planes[wi].saturating_sub(world.air_wings.count_planes[wi]);
        if missing == 0 {
            continue;
        }
        let stock = econ.stockpile[ci]
            .entry("aircraft".to_owned())
            .or_insert(0.0);
        let add = missing.min(stock.max(0.0).floor() as u32).min(10);
        if add == 0 {
            continue;
        }
        *stock -= add as f32;
        world.air_wings.count_planes[wi] += add;
        report.planes_reinforced += add;
        if let Some(def) = data.aircraft.get(&world.air_wings.aircraft_keys[wi]) {
            world.air_wings.range_km[wi] = def.range;
        }
    }
}

fn create_wings_from_stockpile(
    world: &mut World,
    econ: &mut EconomyState,
    data: &GameData,
    report: &mut AirTaskReport,
) {
    for ci in 0..world.countries.count {
        let country = CountryId(ci as u16);
        econ.ensure_capacity(ci + 1);
        let stock = econ.stockpile[ci]
            .entry("aircraft".to_owned())
            .or_insert(0.0);
        while *stock >= 100.0 {
            let Some(base) = best_airbase_for_country(world, country) else {
                break;
            };
            if !can_deploy_to_base(world, country, base, 100) {
                break;
            }
            let aircraft_key = preferred_aircraft_key(data);
            let Some(def) = data.aircraft.get(aircraft_key) else {
                break;
            };
            let wing_id = world.air_wings.push(
                country,
                aircraft_key.to_owned(),
                base.0 as u32,
                100,
                def.max_organisation,
                format!("Reserve Air Wing {}", world.air_wings.count + 1),
            );
            let wi = wing_id as usize;
            world.air_wings.base_state[wi] = base.0;
            world.air_wings.range_km[wi] = def.range;
            *stock -= 100.0;
            report.wings_created += 1;
        }
    }
}

fn best_airbase_for_country(world: &World, country: CountryId) -> Option<StateId> {
    (0..world.states.count)
        .map(|si| StateId(si as u16))
        .filter(|&state| airbase_capacity(world, country, state) > 0)
        .max_by_key(|&state| airbase_capacity(world, country, state))
}

fn preferred_aircraft_key(data: &GameData) -> &str {
    if data.aircraft.contains_key("fighter") {
        "fighter"
    } else {
        data.aircraft
            .keys()
            .next()
            .map(String::as_str)
            .unwrap_or("fighter")
    }
}

fn execute_daily_missions(world: &mut World, data: &GameData, report: &mut AirTaskReport) {
    let air_control = AirControl::recompute(world, data);
    let mut bomb_targets = strategic_targets_by_region(world);
    for wi in 0..world.air_wings.count {
        if world.air_wings.count_planes[wi] == 0 || world.air_wings.transfer_arrival_hour[wi] != 0 {
            continue;
        }
        match world.air_wings.mission[wi] {
            AirMission::StrategicBombing | AirMission::LogisticalStrike => {
                let region = effective_region(world, wi);
                if let Some(target) = bomb_targets.remove(&region) {
                    let out =
                        execute_bombing(world, data, &air_control, AirWingId(wi as u32), target);
                    report.buildings_destroyed += out.buildings_destroyed as u32;
                    report.infrastructure_destroyed += out.infra_destroyed as u32;
                }
            }
            AirMission::NavalStrike | AirMission::NavalPatrol => {
                report.naval_damage += execute_naval_strike(world, data, wi);
            }
            AirMission::PortStrike => {
                report.port_damage += execute_port_strike(world, data, wi);
            }
            AirMission::CloseAirSupport
            | AirMission::AirSuperiority
            | AirMission::Interception
            | AirMission::Drop
            | AirMission::Idle => {}
        }
    }
}

fn strategic_targets_by_region(world: &World) -> HashMap<u32, StrategicBombingTarget> {
    let mut targets = HashMap::new();
    for si in 0..world.states.count {
        let defender = world.states.controllers[si];
        if defender.is_none() {
            continue;
        }
        let region = si as u32;
        targets.entry(region).or_insert(StrategicBombingTarget {
            state_id: StateId(si as u16),
            region,
            defender,
        });
    }
    targets
}

fn execute_naval_strike(world: &mut World, data: &GameData, wing_idx: usize) -> f32 {
    let owner = world.air_wings.owners[wing_idx];
    let region = effective_region(world, wing_idx);
    let strike = aircraft_mission_power(world, data, wing_idx, |def| def.naval_strike);
    if strike <= 0.0 {
        return 0.0;
    }
    let Some(fleet) = enemy_fleet_in_region(world, owner, region) else {
        return 0.0;
    };
    let damage = strike * 0.02;
    apply_fleet_air_damage(world, fleet, damage)
}

fn execute_port_strike(world: &mut World, data: &GameData, wing_idx: usize) -> f32 {
    let owner = world.air_wings.owners[wing_idx];
    let region = effective_region(world, wing_idx);
    let strike = aircraft_mission_power(world, data, wing_idx, |def| {
        def.naval_strike + def.air_bombing * 0.25
    });
    if strike <= 0.0 {
        return 0.0;
    }
    let mut total = 0.0;
    let targets: Vec<FleetId> = (0..world.fleets.count)
        .filter(|&fi| {
            world.fleets.region_id[fi] == region
                && world.fleets.home_port[fi] != u16::MAX
                && world.diplomacy.at_war_with(owner, world.fleets.owners[fi])
        })
        .map(|fi| FleetId(fi as u32))
        .collect();
    for fleet in targets {
        total += apply_fleet_air_damage(world, fleet, strike * 0.01);
    }
    total
}

fn effective_region(world: &World, wing_idx: usize) -> u32 {
    let target = world.air_wings.target_region[wing_idx];
    if target == NO_AIR_REGION {
        world.air_wings.region_id[wing_idx]
    } else {
        target
    }
}

fn aircraft_mission_power(
    world: &World,
    data: &GameData,
    wing_idx: usize,
    value: impl Fn(&hoi4_data::AircraftDef) -> f32,
) -> f32 {
    let Some(def) = data.aircraft.get(&world.air_wings.aircraft_keys[wing_idx]) else {
        return 0.0;
    };
    let org = world.air_wings.organisation[wing_idx]
        / world.air_wings.max_organisation[wing_idx].max(1.0);
    value(def) * world.air_wings.count_planes[wing_idx] as f32 * org.clamp(0.1, 1.0)
}

fn enemy_fleet_in_region(world: &World, owner: CountryId, region: u32) -> Option<FleetId> {
    (0..world.fleets.count)
        .find(|&fi| {
            world.fleets.region_id[fi] == region
                && world.diplomacy.at_war_with(owner, world.fleets.owners[fi])
                && !world.fleets.ships[fi].is_empty()
        })
        .map(|fi| FleetId(fi as u32))
}

fn apply_fleet_air_damage(world: &mut World, fleet: FleetId, damage: f32) -> f32 {
    if fleet.is_none() || damage <= 0.0 {
        return 0.0;
    }
    let fi = fleet.0 as usize;
    if fi >= world.fleets.count || world.fleets.ships[fi].is_empty() {
        return 0.0;
    }
    let mut total_applied = 0.0;
    let per_ship = damage / world.fleets.ships[fi].len().max(1) as f32;
    for &ship in &world.fleets.ships[fi] {
        let si = ship.0 as usize;
        if si >= world.ships.count || world.ships.hp[si] <= 0.0 {
            continue;
        }
        let before = world.ships.hp[si];
        world.ships.hp[si] = (world.ships.hp[si] - per_ship).max(0.0);
        total_applied += before - world.ships.hp[si];
    }
    total_applied
}

pub fn mission_efficiency(world: &World, wing_idx: usize) -> f32 {
    if wing_idx >= world.air_wings.count {
        return 0.0;
    }
    if world.air_wings.transfer_arrival_hour[wing_idx] != 0 {
        return 0.0;
    }
    let region = effective_region(world, wing_idx);
    let base_region = world.air_wings.region_id[wing_idx];
    let distance = base_region.abs_diff(region) as f32;
    let range_regions = (world.air_wings.range_km[wing_idx].max(300.0) / 600.0).max(1.0);
    (1.0 - distance / (range_regions * 3.0)).clamp(0.1, 1.0)
}

pub fn is_air_to_air_mission(mission: AirMission) -> bool {
    matches!(
        mission,
        AirMission::AirSuperiority | AirMission::Interception
    )
}

pub fn contributes_to_cas(mission: AirMission, kind: AircraftKind) -> bool {
    mission == AirMission::CloseAirSupport
        && matches!(
            kind,
            AircraftKind::CloseAirSupport | AircraftKind::TacticalBomber
        )
}
