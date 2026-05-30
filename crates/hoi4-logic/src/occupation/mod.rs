use hoi4_data::GameData;
use hoi4_state::{CountryId, OccupationPolicy, StateId, StateIntegrationStatus, World};

#[derive(Debug, Clone, PartialEq)]
pub struct OccupationStateView {
    pub state: StateId,
    pub owner: CountryId,
    pub controller: CountryId,
    pub occupier: CountryId,
    pub policy: OccupationPolicy,
    pub garrison_template_id: Option<u32>,
    pub required_suppression: f32,
    pub provided_suppression: f32,
    pub resistance: f32,
    pub compliance: f32,
    pub last_sabotage_day: i64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OccupationPolicyEffects {
    pub resistance_growth: f32,
    pub compliance_growth: f32,
    pub suppression_need_mult: f32,
    pub output_share: f32,
    pub sabotage_risk_mult: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum OccupationError {
    InvalidState,
    NotOccupied,
}

pub fn policy_effects(policy: OccupationPolicy) -> OccupationPolicyEffects {
    match policy {
        OccupationPolicy::LenientOccupation => OccupationPolicyEffects {
            resistance_growth: -0.08,
            compliance_growth: 0.08,
            suppression_need_mult: 0.75,
            output_share: 0.35,
            sabotage_risk_mult: 0.60,
        },
        OccupationPolicy::CivilianOversight => OccupationPolicyEffects {
            resistance_growth: 0.02,
            compliance_growth: 0.05,
            suppression_need_mult: 1.00,
            output_share: 0.50,
            sabotage_risk_mult: 1.00,
        },
        OccupationPolicy::MilitaryGovernor => OccupationPolicyEffects {
            resistance_growth: 0.10,
            compliance_growth: 0.02,
            suppression_need_mult: 1.20,
            output_share: 0.65,
            sabotage_risk_mult: 1.20,
        },
        OccupationPolicy::HarshRepression => OccupationPolicyEffects {
            resistance_growth: -0.18,
            compliance_growth: -0.02,
            suppression_need_mult: 1.45,
            output_share: 0.70,
            sabotage_risk_mult: 1.50,
        },
        OccupationPolicy::LootingEconomy => OccupationPolicyEffects {
            resistance_growth: 0.25,
            compliance_growth: -0.05,
            suppression_need_mult: 1.70,
            output_share: 0.85,
            sabotage_risk_mult: 2.00,
        },
    }
}

pub fn sync_occupation_states(world: &mut World) {
    for si in 0..world.states.count {
        let owner = world.states.owners[si];
        let controller = world.states.controllers[si];
        if owner.is_none() || controller.is_none() || owner == controller {
            world.states.occupiers[si] = CountryId::NONE;
            world.states.required_suppression[si] = 0.0;
            world.states.provided_suppression[si] = 0.0;
            world.states.resistance[si] = 0.0;
            world.states.compliance[si] = 1.0;
            continue;
        }

        if world.states.occupiers[si] != controller {
            world.states.occupiers[si] = controller;
            world.states.occupation_policies[si] = OccupationPolicy::CivilianOversight;
            world.states.garrison_template_indices[si] = None;
            world.states.resistance[si] = world.states.resistance[si].max(10.0);
            world.states.compliance[si] = world.states.compliance[si].min(5.0);
            world.states.last_sabotage_day[si] = -1;
        }
    }
}

pub fn occupation_state(world: &World, state: StateId) -> Option<OccupationStateView> {
    let si = state.0 as usize;
    if si >= world.states.count || world.states.occupiers[si].is_none() {
        return None;
    }
    Some(OccupationStateView {
        state,
        owner: world.states.owners[si],
        controller: world.states.controllers[si],
        occupier: world.states.occupiers[si],
        policy: world.states.occupation_policies[si],
        garrison_template_id: world.states.garrison_template_indices[si],
        required_suppression: world.states.required_suppression[si],
        provided_suppression: world.states.provided_suppression[si],
        resistance: world.states.resistance[si],
        compliance: world.states.compliance[si],
        last_sabotage_day: world.states.last_sabotage_day[si],
    })
}

pub fn set_occupation_policy(
    world: &mut World,
    state: StateId,
    policy: OccupationPolicy,
) -> Result<(), OccupationError> {
    sync_occupation_states(world);
    let si = state.0 as usize;
    if si >= world.states.count {
        return Err(OccupationError::InvalidState);
    }
    if world.states.occupiers[si].is_none() {
        return Err(OccupationError::NotOccupied);
    }
    world.states.occupation_policies[si] = policy;
    Ok(())
}

pub fn set_garrison_template(
    world: &mut World,
    state: StateId,
    template_id: Option<u32>,
) -> Result<(), OccupationError> {
    sync_occupation_states(world);
    let si = state.0 as usize;
    if si >= world.states.count {
        return Err(OccupationError::InvalidState);
    }
    if world.states.occupiers[si].is_none() {
        return Err(OccupationError::NotOccupied);
    }
    world.states.garrison_template_indices[si] = template_id;
    Ok(())
}

pub fn tick_occupation_daily(world: &mut World, data: &GameData) {
    sync_occupation_states(world);
    let current_day = world.date.days_since_epoch();

    for si in 0..world.states.count {
        let occupier = world.states.occupiers[si];
        if occupier.is_none() {
            continue;
        }

        let policy = world.states.occupation_policies[si];
        let effects = policy_effects(policy);
        let required = required_suppression_for_state(world, StateId(si as u16), effects);
        let provided = provided_suppression_for_state(world, data, StateId(si as u16));
        world.states.required_suppression[si] = required;
        world.states.provided_suppression[si] = provided;

        let deficit = (required - provided).max(0.0);
        let surplus = (provided - required).max(0.0);
        let resistance_delta = effects.resistance_growth + deficit * 0.18 - surplus * 0.12;
        let compliance_delta = effects.compliance_growth - deficit * 0.015;
        world.states.resistance[si] =
            (world.states.resistance[si] + resistance_delta).clamp(0.0, 100.0);
        world.states.compliance[si] =
            (world.states.compliance[si] + compliance_delta).clamp(0.0, 100.0);

        let sabotage_threshold = 55.0 / effects.sabotage_risk_mult.max(0.1);
        let enough_time = world.states.last_sabotage_day[si] < 0
            || current_day - world.states.last_sabotage_day[si] >= 7;
        if enough_time && world.states.resistance[si] >= sabotage_threshold && deficit > 0.0 {
            if sabotage_state_building(world, StateId(si as u16)) {
                world.states.last_sabotage_day[si] = current_day;
            }
        }
    }
}

pub fn occupied_output_share(world: &World, state: StateId) -> f32 {
    let si = state.0 as usize;
    if si >= world.states.count || world.states.occupiers[si].is_none() {
        return 1.0;
    }
    let effects = policy_effects(world.states.occupation_policies[si]);
    let compliance_bonus = (world.states.compliance[si] / 100.0).clamp(0.0, 1.0) * 0.35;
    let resistance_penalty = (world.states.resistance[si] / 100.0).clamp(0.0, 1.0) * 0.45;
    (effects.output_share + compliance_bonus - resistance_penalty).clamp(0.05, 1.0)
}

pub fn state_governance_yield_factor(world: &World, state: StateId) -> f32 {
    let si = state.0 as usize;
    if si >= world.states.count {
        return 1.0;
    }
    if !world.states.occupiers[si].is_none() {
        return occupied_output_share(world, state);
    }

    let base = match world.states.integration_status[si] {
        StateIntegrationStatus::Metropole | StateIntegrationStatus::Incorporated => 1.0,
        StateIntegrationStatus::Colony => 0.85,
        StateIntegrationStatus::Protectorate | StateIntegrationStatus::Mandate => 0.75,
        StateIntegrationStatus::Concession => 0.80,
        StateIntegrationStatus::Occupied => 0.35,
    };
    let compliance_bonus = (world.states.compliance[si] / 100.0).clamp(0.0, 1.0) * 0.20;
    let resistance_penalty = (world.states.resistance[si] / 100.0).clamp(0.0, 1.0) * 0.65;
    (base + compliance_bonus - resistance_penalty).clamp(0.05, 1.0)
}

pub fn country_governance_market_access(world: &World, country: CountryId) -> f32 {
    if country.is_none() {
        return 0.0;
    }
    let mut weighted = 0.0f64;
    let mut total_pop = 0u64;
    for si in 0..world.states.count {
        if world.states.owners[si] != country {
            continue;
        }
        let state = StateId(si as u16);
        let pop = world.state_population(state).max(1);
        weighted += state_governance_yield_factor(world, state) as f64 * pop as f64;
        total_pop = total_pop.saturating_add(pop);
    }
    if total_pop == 0 {
        1.0
    } else {
        (weighted / total_pop as f64).clamp(0.05, 1.0) as f32
    }
}

fn required_suppression_for_state(
    world: &World,
    state: StateId,
    effects: OccupationPolicyEffects,
) -> f32 {
    let building_levels: u32 = world
        .countries
        .buildings_v6
        .buildings
        .iter()
        .filter(|building| building.state == state && building.level > 0)
        .map(|building| building.level as u32)
        .sum();
    let si = state.0 as usize;
    let base = 2.0 + building_levels as f32 * 0.60 + world.states.resistance[si] * 0.05;
    base * effects.suppression_need_mult
}

fn provided_suppression_for_state(world: &World, data: &GameData, state: StateId) -> f32 {
    let si = state.0 as usize;
    let Some(template_id) = world.states.garrison_template_indices[si] else {
        return 0.0;
    };
    let occupier = world.states.occupiers[si];
    let Some(tag) = world.country_tag(occupier) else {
        return 0.0;
    };
    let Some(template) = data
        .division_templates
        .get(tag)
        .and_then(|templates| templates.get(template_id as usize))
    else {
        return 0.0;
    };
    let stats = crate::military::stats::DivisionStats::aggregate(template, data);
    let manpower_ratio = if stats.manpower == 0 {
        1.0
    } else {
        (world.manpower(occupier) as f32 / stats.manpower as f32).clamp(0.25, 1.0)
    };
    stats.suppression * manpower_ratio
}

fn sabotage_state_building(world: &mut World, state: StateId) -> bool {
    if let Some(building) = world
        .countries
        .buildings_v6
        .buildings
        .iter_mut()
        .filter(|building| building.state == state && building.level > 0)
        .max_by_key(|building| building.level)
    {
        building.level = building.level.saturating_sub(1);
        building.production_rate *= 0.90;
        true
    } else {
        false
    }
}
