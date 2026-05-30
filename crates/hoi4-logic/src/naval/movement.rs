//! Fleet movement between strategic sea regions.

use hoi4_state::{FleetId, World};

use super::regions::NO_SEA_REGION;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FleetMoveError {
    InvalidFleet,
    InvalidRegion,
    AlreadyThere,
}

pub fn order_move_to_region(
    world: &mut World,
    fleet: FleetId,
    target_region: u32,
    current_hour: u64,
) -> Result<(), FleetMoveError> {
    if fleet.is_none() {
        return Err(FleetMoveError::InvalidFleet);
    }
    let fi = fleet.0 as usize;
    if fi >= world.fleets.count {
        return Err(FleetMoveError::InvalidFleet);
    }
    if target_region == NO_SEA_REGION {
        return Err(FleetMoveError::InvalidRegion);
    }
    let current = world.fleets.region_id[fi];
    if current == target_region && world.fleets.target_region_id[fi] == NO_SEA_REGION {
        return Err(FleetMoveError::AlreadyThere);
    }

    let distance = if current == NO_SEA_REGION {
        1
    } else {
        current.abs_diff(target_region).max(1)
    };
    let speed = world.fleets.speed_knots[fi].max(1.0);
    let travel_hours = ((distance as f32 / speed) * 24.0).ceil().max(1.0) as u64;

    world.fleets.target_region_id[fi] = target_region;
    world.fleets.arrival_hour[fi] = current_hour + travel_hours;
    world.fleets.home_port[fi] = u16::MAX;
    world.fleets.repair_state[fi] = hoi4_state::NavalRepairState::AtSea;
    Ok(())
}

pub fn tick_fleet_movement(world: &mut World) -> usize {
    let now = world.date.hours_since_epoch().max(0) as u64;
    let mut arrived = 0usize;
    for fi in 0..world.fleets.count {
        let target = world.fleets.target_region_id[fi];
        if target == NO_SEA_REGION || world.fleets.arrival_hour[fi] == 0 {
            continue;
        }
        if now >= world.fleets.arrival_hour[fi] {
            world.fleets.region_id[fi] = target;
            world.fleets.target_region_id[fi] = NO_SEA_REGION;
            world.fleets.arrival_hour[fi] = 0;
            arrived += 1;
        }
    }
    arrived
}

pub fn is_moving(world: &World, fleet: FleetId) -> bool {
    if fleet.is_none() {
        return false;
    }
    let fi = fleet.0 as usize;
    fi < world.fleets.count && world.fleets.target_region_id[fi] != NO_SEA_REGION
}
