//! Strategic air regions and airbase capacity for the air MVP.

use std::collections::HashMap;

use hoi4_state::{CountryId, StateId, World};

pub const NO_AIR_REGION: u32 = u32::MAX;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AirRegionDef {
    pub id: u32,
    pub name: String,
    pub states: Vec<StateId>,
}

#[derive(Debug, Clone, Default)]
pub struct AirRegionMap {
    pub regions: Vec<AirRegionDef>,
    pub state_region: HashMap<StateId, u32>,
}

impl AirRegionMap {
    /// MVP fallback: every loaded state is its own air region. This removes the
    /// implicit OOB state-id placeholder from logic callers while keeping region
    /// ids deterministic until vanilla air regions are parsed.
    pub fn from_world(world: &World) -> Self {
        let mut regions = Vec::with_capacity(world.states.count);
        let mut state_region = HashMap::with_capacity(world.states.count);
        for si in 0..world.states.count {
            let state = StateId(si as u16);
            let id = si as u32;
            state_region.insert(state, id);
            let name = world
                .states
                .names
                .get(si)
                .filter(|name| !name.is_empty())
                .cloned()
                .unwrap_or_else(|| format!("Air Region {}", id + 1));
            regions.push(AirRegionDef {
                id,
                name,
                states: vec![state],
            });
        }
        Self {
            regions,
            state_region,
        }
    }

    pub fn region_for_state(&self, state: StateId) -> Option<u32> {
        self.state_region.get(&state).copied()
    }
}

pub fn airbase_capacity(world: &World, owner: CountryId, state: StateId) -> u32 {
    if state.is_none() || state.0 as usize >= world.states.count {
        return 0;
    }
    if !owner.is_none() && world.states.controllers[state.0 as usize] != owner {
        return 0;
    }
    let mut capacity = 100u32;
    for building in &world.countries.buildings_v6.buildings {
        if building.state != state || building.level == 0 {
            continue;
        }
        if matches!(
            building.building_def_id.as_str(),
            "air_base" | "airbase" | "v6_air_base" | "airport"
        ) {
            capacity += building.level as u32 * 200;
        }
    }
    capacity
}

pub fn deployed_planes_in_state(world: &World, owner: CountryId, state: StateId) -> u32 {
    let raw = state.0;
    (0..world.air_wings.count)
        .filter(|&i| world.air_wings.owners[i] == owner && world.air_wings.base_state[i] == raw)
        .map(|i| world.air_wings.count_planes[i])
        .sum()
}

pub fn can_deploy_to_base(world: &World, owner: CountryId, state: StateId, planes: u32) -> bool {
    let capacity = airbase_capacity(world, owner, state);
    capacity >= deployed_planes_in_state(world, owner, state).saturating_add(planes)
}
