use std::collections::HashMap;

use hoi4_data::{GameData, ReinforcementPriority};
use hoi4_state::{CountryId, DivisionId, PopClass, ProvinceId, StateId, World};

use crate::economy::EconomyState;

use super::{spawn, stats::DivisionStats, templates::template_training_days};

#[derive(Debug, Clone, PartialEq)]
pub struct TrainingQueueItem {
    pub id: u32,
    pub owner: CountryId,
    pub template_id: u32,
    pub count: u8,
    pub deploy_state: StateId,
    pub progress_days: f32,
    pub required_days: f32,
    pub manpower_allocated: u32,
    pub domestic_manpower_allocated: u32,
    pub colonial_manpower_allocated: u32,
    pub subject_manpower_allocated: u32,
    pub equipment_allocated: HashMap<String, f32>,
    pub priority: ReinforcementPriority,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TrainingManpowerAllocation {
    pub domestic: u32,
    pub colonial: u32,
    pub subject: u32,
}

impl TrainingManpowerAllocation {
    fn total(self) -> u32 {
        self.domestic + self.colonial + self.subject
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TrainingError {
    InvalidCountry,
    MissingTemplate,
    InvalidDeployState,
    DeployStateNotControlled,
    NoDeployProvince,
}

pub fn enqueue_training(
    world: &World,
    econ: &mut EconomyState,
    data: &GameData,
    owner: CountryId,
    template_id: u32,
    count: u8,
    deploy_state: StateId,
    priority: ReinforcementPriority,
) -> Result<u32, TrainingError> {
    if owner.is_none() {
        return Err(TrainingError::InvalidCountry);
    }
    let ci = owner.0 as usize;
    econ.ensure_capacity(ci + 1);
    validate_deploy_state(world, owner, deploy_state)?;

    let tag = world
        .country_tag(owner)
        .ok_or(TrainingError::InvalidCountry)?
        .to_owned();
    let template = data
        .division_templates
        .get(&tag)
        .and_then(|templates| templates.get(template_id as usize))
        .ok_or(TrainingError::MissingTemplate)?;

    let next_id = econ.training_queues[ci]
        .iter()
        .map(|item| item.id)
        .max()
        .unwrap_or(0)
        .saturating_add(1);
    econ.training_queues[ci].push(TrainingQueueItem {
        id: next_id,
        owner,
        template_id,
        count: count.max(1),
        deploy_state,
        progress_days: 0.0,
        required_days: template_training_days(template, data),
        manpower_allocated: 0,
        domestic_manpower_allocated: 0,
        colonial_manpower_allocated: 0,
        subject_manpower_allocated: 0,
        equipment_allocated: HashMap::new(),
        priority,
    });
    Ok(next_id)
}

pub fn tick_training_queues(
    world: &mut World,
    econ: &mut EconomyState,
    data: &GameData,
) -> Vec<DivisionId> {
    let mut deployed = Vec::new();
    econ.ensure_capacity(world.countries.count);

    for ci in 0..econ.training_queues.len() {
        let owner = CountryId(ci as u16);
        if owner.is_none() {
            continue;
        }
        if let Some(stockpile) = econ.stockpile.get_mut(ci) {
            crate::economy::stockpile::normalize_stockpile_keys(stockpile);
        }

        let mut queue = std::mem::take(&mut econ.training_queues[ci]);
        queue.sort_by_key(|item| match item.priority {
            ReinforcementPriority::High => 0,
            ReinforcementPriority::Normal => 1,
            ReinforcementPriority::Low => 2,
        });

        let mut remaining = Vec::new();
        for mut item in queue {
            allocate_training_day(world, econ, data, ci, &mut item);
            let manpower_ratio = manpower_ratio(world, data, &item);
            let equipment_ratio = equipment_ratio(world, data, &item);
            let allocation_ratio = manpower_ratio.min(equipment_ratio);
            if allocation_ratio > 0.0 {
                item.progress_days += allocation_ratio.clamp(0.10, 1.0);
            }

            if item.progress_days >= item.required_days {
                match deploy_one(world, data, &mut item, allocation_ratio) {
                    Ok(div_id) => deployed.push(div_id),
                    Err(_) => {
                        remaining.push(item);
                        continue;
                    }
                }
                item.count = item.count.saturating_sub(1);
                item.progress_days = 0.0;
                item.manpower_allocated = 0;
                item.domestic_manpower_allocated = 0;
                item.colonial_manpower_allocated = 0;
                item.subject_manpower_allocated = 0;
                item.equipment_allocated.clear();
            }

            if item.count > 0 {
                remaining.push(item);
            }
        }
        econ.training_queues[ci] = remaining;
    }

    deployed
}

fn validate_deploy_state(
    world: &World,
    owner: CountryId,
    deploy_state: StateId,
) -> Result<(), TrainingError> {
    let si = deploy_state.0 as usize;
    if si >= world.states.count {
        return Err(TrainingError::InvalidDeployState);
    }
    if world.states.controllers[si] != owner {
        return Err(TrainingError::DeployStateNotControlled);
    }
    if world.states.provinces[si].is_empty() {
        return Err(TrainingError::NoDeployProvince);
    }
    Ok(())
}

fn allocate_training_day(
    world: &mut World,
    econ: &mut EconomyState,
    data: &GameData,
    ci: usize,
    item: &mut TrainingQueueItem,
) {
    let Some(template) = template_for_item(world, data, item) else {
        return;
    };
    let stats = DivisionStats::aggregate(template, data);
    let daily_manpower =
        ((stats.manpower as f32 / item.required_days.max(1.0)).ceil() as u32).max(1);
    let manpower_remaining = stats.manpower.saturating_sub(item.manpower_allocated);
    let request = daily_manpower.min(manpower_remaining);
    let allocated = consume_soldier_pop(world, item.owner, request);
    item.manpower_allocated = item.manpower_allocated.saturating_add(allocated.total());
    item.domestic_manpower_allocated = item
        .domestic_manpower_allocated
        .saturating_add(allocated.domestic);
    item.colonial_manpower_allocated = item
        .colonial_manpower_allocated
        .saturating_add(allocated.colonial);
    item.subject_manpower_allocated = item
        .subject_manpower_allocated
        .saturating_add(allocated.subject);

    let needs = crate::economy::stockpile::template_equipment_needs(template, data);
    let stockpile = &mut econ.stockpile[ci];
    for (equipment, total_needed) in needs {
        let total_needed = total_needed as f32;
        let already = item
            .equipment_allocated
            .get(&equipment)
            .copied()
            .unwrap_or(0.0);
        let remaining = (total_needed - already).max(0.0);
        if remaining <= 0.0 {
            continue;
        }
        let daily = (total_needed / item.required_days.max(1.0))
            .max(1.0)
            .min(remaining);
        *stockpile.entry(equipment.clone()).or_insert(0.0) -= daily;
        *item.equipment_allocated.entry(equipment).or_insert(0.0) += daily;
    }
}

fn consume_soldier_pop(
    world: &mut World,
    owner: CountryId,
    amount: u32,
) -> TrainingManpowerAllocation {
    if amount == 0 {
        return TrainingManpowerAllocation::default();
    }
    let state_ids = world.country_state_ids(owner);
    let mut remaining = amount;
    let mut consumed = TrainingManpowerAllocation::default();
    let has_country_pops = world
        .countries
        .pops
        .groups
        .iter()
        .any(|pg| state_ids.contains(&pg.state));
    for colonial_source in [false, true] {
        for pg in &mut world.countries.pops.groups {
            if remaining == 0 {
                break;
            }
            if pg.class != PopClass::Soldier
                || pg.employed_at.is_some()
                || !state_ids.contains(&pg.state)
            {
                continue;
            }
            let state_idx = pg.state.0 as usize;
            if state_idx >= world.states.count {
                continue;
            }
            let status = world.states.integration_status[state_idx];
            let is_colonial = (status == hoi4_state::StateIntegrationStatus::Metropole
                && !world.states.cores[state_idx].contains(&owner))
                || status.is_colonial_or_occupied();
            if is_colonial != colonial_source {
                continue;
            }
            let take = pg.size.min(remaining);
            pg.size -= take;
            if is_colonial {
                pg.radicalism = (pg.radicalism + 0.000_003 * take as f32).clamp(0.0, 1.0);
                consumed.colonial = consumed.colonial.saturating_add(take);
            } else {
                consumed.domestic = consumed.domestic.saturating_add(take);
            }
            remaining -= take;
        }
        if remaining == 0 {
            break;
        }
    }
    if consumed.total() == 0 && !has_country_pops {
        let fallback_pool: u32 = state_ids
            .iter()
            .map(|state| world.states.manpower_pool[state.0 as usize])
            .sum();
        consumed.domestic = fallback_pool.min(amount);
    }
    consumed
}

fn deploy_one(
    world: &mut World,
    data: &GameData,
    item: &mut TrainingQueueItem,
    allocation_ratio: f32,
) -> Result<DivisionId, TrainingError> {
    validate_deploy_state(world, item.owner, item.deploy_state)?;
    let location = deploy_province(world, item.deploy_state)?;
    let name = format!("New Division {}", world.divisions.count + 1);
    let div = spawn::spawn_from_template(
        world,
        data,
        item.owner,
        location,
        item.template_id as u16,
        name,
    )
    .ok_or(TrainingError::MissingTemplate)?;
    let idx = div.0 as usize;
    world.divisions.strength[idx] = allocation_ratio.clamp(0.0, 1.0);
    Ok(div)
}

fn deploy_province(world: &World, state: StateId) -> Result<ProvinceId, TrainingError> {
    world
        .states
        .provinces
        .get(state.0 as usize)
        .and_then(|provinces| provinces.first().copied())
        .ok_or(TrainingError::NoDeployProvince)
}

fn template_for_item<'a>(
    world: &World,
    data: &'a GameData,
    item: &TrainingQueueItem,
) -> Option<&'a hoi4_data::DivisionTemplate> {
    let tag = world.country_tag(item.owner)?;
    data.division_templates
        .get(tag)?
        .get(item.template_id as usize)
}

fn manpower_ratio(world: &World, data: &GameData, item: &TrainingQueueItem) -> f32 {
    let Some(template) = template_for_item(world, data, item) else {
        return 0.0;
    };
    let need = DivisionStats::aggregate(template, data).manpower;
    if need == 0 {
        1.0
    } else {
        (item.manpower_allocated as f32 / need as f32).clamp(0.0, 1.0)
    }
}

fn equipment_ratio(world: &World, data: &GameData, item: &TrainingQueueItem) -> f32 {
    let Some(template) = template_for_item(world, data, item) else {
        return 0.0;
    };
    let needs = crate::economy::stockpile::template_equipment_needs(template, data);
    let mut ratio: f32 = 1.0;
    for (equipment, qty) in needs {
        if qty == 0 {
            continue;
        }
        let allocated = item
            .equipment_allocated
            .get(&equipment)
            .copied()
            .unwrap_or(0.0);
        ratio = ratio.min(allocated / qty as f32);
    }
    ratio.clamp(0.0, 1.0)
}
