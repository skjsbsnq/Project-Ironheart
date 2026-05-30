//! Naval task execution, convoy risk, repair, and abstract shipbuilding MVP.

use std::collections::HashMap;

use hoi4_data::{GameData, ShipKind};
use hoi4_state::{CountryId, FleetId, NavalMission, NavalRepairState, ProvinceId, World};

use crate::economy::EconomyState;

#[derive(Debug, Clone, Copy, Default)]
pub struct SeaRegionPresence {
    pub patrol: f32,
    pub strike_force: f32,
    pub escort: f32,
    pub raiding: f32,
    pub invasion_support: f32,
    pub recon: f32,
}

#[derive(Debug, Clone, Default)]
pub struct NavalTaskReport {
    pub convoy_losses: HashMap<CountryId, f32>,
    pub repaired_hp: f32,
    pub ships_built: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConvoyRouteRisk {
    pub escort_presence: f32,
    pub raider_presence: f32,
    pub loss_rate: f32,
    pub safe_capacity_mult: f32,
}

pub fn fleet_presence(world: &World, data: &GameData, fleet: FleetId) -> f32 {
    if fleet.is_none() {
        return 0.0;
    }
    let fi = fleet.0 as usize;
    if fi >= world.fleets.count {
        return 0.0;
    }
    let mut out = 0.0f32;
    for &ship in &world.fleets.ships[fi] {
        if ship.is_none() {
            continue;
        }
        let si = ship.0 as usize;
        if si >= world.ships.count || world.ships.hp[si] <= 0.0 {
            continue;
        }
        let hp_ratio = world.ships.hp[si] / world.ships.max_hp[si].max(1.0);
        let class = data.ship_classes.get(&world.ships.class_keys[si]);
        let base = match class.map(|c| c.kind).unwrap_or(ShipKind::Other) {
            ShipKind::Carrier | ShipKind::Capital => 4.0,
            ShipKind::Screen => 1.4,
            ShipKind::Submarine => 1.0,
            ShipKind::Transport => 0.1,
            ShipKind::Other => 0.8,
        };
        let speed = class.map(|c| c.speed).unwrap_or(20.0).max(1.0) / 20.0;
        out += base * hp_ratio.max(0.0) * speed;
    }
    out
}

pub fn mission_presence_by_region(
    world: &World,
    data: &GameData,
) -> HashMap<u32, HashMap<CountryId, SeaRegionPresence>> {
    let mut out: HashMap<u32, HashMap<CountryId, SeaRegionPresence>> = HashMap::new();
    for fi in 0..world.fleets.count {
        if world.fleets.target_region_id[fi] != super::regions::NO_SEA_REGION {
            continue;
        }
        let region = world.fleets.region_id[fi];
        if region == super::regions::NO_SEA_REGION {
            continue;
        }
        let owner = world.fleets.owners[fi];
        if owner.is_none() {
            continue;
        }
        let strength = fleet_presence(world, data, FleetId(fi as u32));
        if strength <= 0.0 {
            continue;
        }
        let entry = out.entry(region).or_default().entry(owner).or_default();
        match world.fleets.mission[fi] {
            NavalMission::Idle => {}
            NavalMission::Patrol => {
                entry.patrol += strength;
                entry.recon += strength * 0.8;
            }
            NavalMission::StrikeForce => entry.strike_force += strength,
            NavalMission::ConvoyEscort => {
                entry.escort += strength;
                entry.recon += strength * 0.4;
            }
            NavalMission::ConvoyRaiding => {
                entry.raiding += strength;
                entry.recon += strength * 0.6;
            }
            NavalMission::NavalInvasionSupport => entry.invasion_support += strength,
            NavalMission::MineLaying | NavalMission::MineSweeping => entry.patrol += strength * 0.5,
        }
    }
    out
}

pub fn convoy_route_risk(
    world: &World,
    data: &GameData,
    owner: CountryId,
    route: &[u32],
) -> ConvoyRouteRisk {
    let presence = mission_presence_by_region(world, data);
    let mut escort = 0.0f32;
    let mut raiders = 0.0f32;
    for region in route {
        if let Some(by_country) = presence.get(region) {
            for (&country, p) in by_country {
                if country == owner {
                    escort += p.escort + p.patrol * 0.25 + p.invasion_support * 0.5;
                } else if world.diplomacy.at_war_with(owner, country) {
                    raiders += p.raiding + p.patrol * 0.15;
                }
            }
        }
    }
    let pressure = raiders / (raiders + escort + 1.0);
    let loss_rate = (pressure * 0.12 * route.len().max(1) as f32).clamp(0.0, 0.8);
    ConvoyRouteRisk {
        escort_presence: escort,
        raider_presence: raiders,
        loss_rate,
        safe_capacity_mult: (1.0 - loss_rate).clamp(0.2, 1.0),
    }
}

pub fn naval_invasion_support_for_route(
    world: &World,
    data: &GameData,
    owner: CountryId,
    route: &[u32],
) -> f32 {
    let presence = mission_presence_by_region(world, data);
    let mut support = 0.0f32;
    for region in route {
        if let Some(by_country) = presence.get(region) {
            if let Some(p) = by_country.get(&owner) {
                support += p.invasion_support + p.escort * 0.25 + p.patrol * 0.1;
            }
        }
    }
    support
}

pub fn consume_convoys_for_route(
    world: &mut World,
    econ: &mut EconomyState,
    owner: CountryId,
    route: &[u32],
    convoys_required: f32,
) -> f32 {
    if owner.is_none() || convoys_required <= 0.0 {
        return 0.0;
    }
    let risk = convoy_route_risk(world, world.data.as_ref(), owner, route);
    let loss = convoys_required * risk.loss_rate;
    let ci = owner.0 as usize;
    econ.ensure_capacity(ci + 1);
    let stockpile = econ.stockpile[ci].entry("convoy".to_owned()).or_insert(0.0);
    let actual = loss.min(stockpile.max(0.0));
    *stockpile -= actual;
    if let Some(market) = world.countries.market.markets.get_mut(ci) {
        let market_stock = market.stockpile.entry("convoy".to_owned()).or_insert(0.0);
        *market_stock = (*market_stock - actual).max(0.0);
    }
    actual
}

pub fn tick_daily(world: &mut World, econ: &mut EconomyState, data: &GameData) -> NavalTaskReport {
    let mut report = NavalTaskReport::default();
    tick_repairs(world, &mut report);
    tick_shipbuilding(world, econ, data, &mut report);
    report
}

fn tick_repairs(world: &mut World, report: &mut NavalTaskReport) {
    let mut used_capacity: HashMap<(CountryId, u16), u32> = HashMap::new();
    for fi in 0..world.fleets.count {
        if world.fleets.home_port[fi] == u16::MAX {
            world.fleets.repair_state[fi] = NavalRepairState::AtSea;
            continue;
        }
        let owner = world.fleets.owners[fi];
        let port = world.fleets.home_port[fi];
        let capacity = port_repair_capacity(world, owner, ProvinceId(port));
        let used = used_capacity.entry((owner, port)).or_insert(0);
        if *used >= capacity {
            world.fleets.repair_state[fi] = NavalRepairState::InPort;
            continue;
        }
        let mut damaged = false;
        for &ship in &world.fleets.ships[fi] {
            if ship.is_none() {
                continue;
            }
            let si = ship.0 as usize;
            if si >= world.ships.count || world.ships.hp[si] >= world.ships.max_hp[si] {
                continue;
            }
            damaged = true;
            let repair = (world.ships.max_hp[si] * 0.04).max(1.0);
            let before = world.ships.hp[si];
            world.ships.hp[si] = (world.ships.hp[si] + repair).min(world.ships.max_hp[si]);
            report.repaired_hp += world.ships.hp[si] - before;
        }
        if damaged {
            *used += 1;
            world.fleets.repair_state[fi] = NavalRepairState::Repairing;
        } else {
            world.fleets.repair_state[fi] = NavalRepairState::InPort;
        }
    }
}

fn port_repair_capacity(world: &World, owner: CountryId, port: ProvinceId) -> u32 {
    if port.is_none() {
        return 0;
    }
    let pi = port.0 as usize;
    let state = world
        .provinces
        .state_of
        .get(pi)
        .copied()
        .unwrap_or(hoi4_state::StateId::NONE);
    let mut capacity = 1u32;
    for building in &world.countries.buildings_v6.buildings {
        if building.state != state || building.level == 0 {
            continue;
        }
        let state_owned = state.0 as usize >= world.states.count
            || world.states.owners[state.0 as usize] == owner;
        if !state_owned {
            continue;
        }
        if building.building_def_id == "shipyard"
            || building.building_def_id == "v6_naval_base"
            || building.building_def_id == "naval_base"
        {
            capacity += building.level as u32;
        }
    }
    capacity
}

fn tick_shipbuilding(
    world: &mut World,
    econ: &mut EconomyState,
    data: &GameData,
    report: &mut NavalTaskReport,
) {
    for ci in 0..world.countries.count {
        let country = CountryId(ci as u16);
        econ.ensure_capacity(ci + 1);
        let naval_vessels = econ.stockpile[ci]
            .entry("naval_vessel".to_owned())
            .or_insert(0.0);
        while *naval_vessels >= 1.0 {
            let fleet = find_or_create_reserve_fleet(world, country);
            let name = format!("Reserve Ship {}", world.ships.count + 1);
            if super::spawn::spawn_ship(world, data, fleet, "destroyer", name).is_some() {
                *naval_vessels -= 1.0;
                report.ships_built += 1;
            } else {
                break;
            }
        }
        let market_convoy = world
            .countries
            .market
            .markets
            .get(ci)
            .and_then(|m| m.stockpile.get("convoy").copied())
            .unwrap_or(0.0);
        let finished_convoys = econ.stockpile[ci].entry("convoy".to_owned()).or_insert(0.0);
        if market_convoy > *finished_convoys {
            *finished_convoys = market_convoy;
        }
    }
}

fn find_or_create_reserve_fleet(world: &mut World, country: CountryId) -> FleetId {
    for fi in 0..world.fleets.count {
        if world.fleets.owners[fi] == country && world.fleets.names[fi].contains("Reserve Fleet") {
            return FleetId(fi as u32);
        }
    }
    let region = first_owned_fleet_region(world, country).unwrap_or(0);
    let fleet_id = world
        .fleets
        .push(country, region, "Reserve Fleet".to_owned());
    FleetId(fleet_id)
}

fn first_owned_fleet_region(world: &World, country: CountryId) -> Option<u32> {
    (0..world.fleets.count)
        .find(|&fi| {
            world.fleets.owners[fi] == country
                && world.fleets.region_id[fi] != super::regions::NO_SEA_REGION
        })
        .map(|fi| world.fleets.region_id[fi])
}
