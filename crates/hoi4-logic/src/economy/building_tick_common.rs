use std::collections::HashMap;

use hoi4_content::{ProductionMethodDef, V6Database};
use hoi4_state::{CountryId, LawCategory, NationalMarket, World};

#[derive(Clone, Copy, Default)]
pub(crate) struct QualificationTotals {
    pub(crate) total: f32,
    pub(crate) literacy: f32,
    pub(crate) skilled: f32,
}

pub(crate) fn good_is_unlocked(db: &V6Database, completed_techs: &[String], good_id: &str) -> bool {
    match db
        .goods
        .iter()
        .find(|g| g.id == good_id)
        .and_then(|gd| gd.unlocked_by.as_ref())
    {
        Some(unlock_tech) => completed_techs.contains(unlock_tech),
        None => true,
    }
}

pub(crate) fn demand_fulfillment(market: &NationalMarket, good_id: &str) -> f32 {
    let demand = market.demand.get(good_id).copied().unwrap_or(0.0);
    if demand <= 0.0 {
        return 1.0;
    }
    let supply = market.supply.get(good_id).copied().unwrap_or(0.0)
        + market.stockpile.get(good_id).copied().unwrap_or(0.0);
    (supply / demand).clamp(0.0, 1.0)
}

#[cfg(test)]
pub(crate) fn compute_qualification_ratio(
    world: &World,
    building: &hoi4_state::Building,
    pms: &[&ProductionMethodDef],
) -> f32 {
    let building_id = world
        .countries
        .buildings_v6
        .buildings
        .iter()
        .position(|candidate| std::ptr::eq(candidate, building))
        .map(|idx| hoi4_state::BuildingId(idx as u32));
    let Some(building_id) = building_id else {
        return 1.0;
    };
    compute_qualification_ratio_for_id(world, building_id, pms)
}

#[cfg(test)]
pub(crate) fn compute_qualification_ratio_for_id(
    world: &World,
    building_id: hoi4_state::BuildingId,
    pms: &[&ProductionMethodDef],
) -> f32 {
    let mut totals = QualificationTotals::default();
    for pg in &world.countries.pops.groups {
        if pg.employed_at != Some(building_id) {
            continue;
        }
        let weight = pg.size as f32;
        totals.total += weight;
        totals.literacy += pg.literacy * weight;
        totals.skilled += pg.skilled_ratio * weight;
    }
    compute_qualification_ratio_from_total(totals, pms)
}

pub(crate) fn collect_qualification_totals(world: &World) -> Vec<QualificationTotals> {
    let mut totals =
        vec![QualificationTotals::default(); world.countries.buildings_v6.buildings.len()];
    for pg in &world.countries.pops.groups {
        let Some(building_id) = pg.employed_at else {
            continue;
        };
        let Some(total) = totals.get_mut(building_id.0 as usize) else {
            continue;
        };
        let weight = pg.size as f32;
        total.total += weight;
        total.literacy += pg.literacy * weight;
        total.skilled += pg.skilled_ratio * weight;
    }
    totals
}

pub(crate) fn country_building_indices(world: &World, ci: usize) -> Vec<usize> {
    if world.runtime_country_indexes_valid {
        if let Some(indices) = world.country_building_index.get(ci) {
            return indices.clone();
        }
    }

    let country_id = CountryId(ci as u16);
    world
        .countries
        .buildings_v6
        .buildings
        .iter()
        .enumerate()
        .filter_map(|(idx, building)| {
            let state_idx = building.state.0 as usize;
            (state_idx < world.states.count && world.states.owners[state_idx] == country_id)
                .then_some(idx)
        })
        .collect()
}

pub(crate) fn country_pop_indices(world: &World, ci: usize) -> Vec<usize> {
    if world.runtime_country_indexes_valid {
        if let Some(indices) = world.country_pop_index.get(ci) {
            return indices.clone();
        }
    }

    let country_id = CountryId(ci as u16);
    world
        .countries
        .pops
        .groups
        .iter()
        .enumerate()
        .filter_map(|(idx, pg)| {
            state_owned_by_parts(
                world.states.count,
                &world.states.owners,
                pg.state,
                country_id,
            )
            .then_some(idx)
        })
        .collect()
}

pub(crate) fn compute_qualification_ratio_from_totals(
    totals: &[QualificationTotals],
    building_idx: usize,
    pms: &[&ProductionMethodDef],
) -> f32 {
    compute_qualification_ratio_from_total(
        totals.get(building_idx).copied().unwrap_or_default(),
        pms,
    )
}

pub(crate) fn compute_qualification_ratio_from_total(
    totals: QualificationTotals,
    pms: &[&ProductionMethodDef],
) -> f32 {
    let required_literacy = pms
        .iter()
        .map(|pm| pm.required_literacy)
        .fold(0.0_f32, f32::max);
    let required_skilled_ratio = pms
        .iter()
        .map(|pm| pm.required_skilled_ratio)
        .fold(0.0_f32, f32::max);
    if required_literacy <= 0.0 && required_skilled_ratio <= 0.0 {
        return 1.0;
    }

    if totals.total <= 0.0 {
        return 0.35;
    }
    let avg_literacy = totals.literacy / totals.total;
    let avg_skilled = totals.skilled / totals.total;
    let literacy_ratio = if required_literacy > 0.0 {
        avg_literacy / required_literacy
    } else {
        1.0
    };
    let skilled_ratio = if required_skilled_ratio > 0.0 {
        avg_skilled / required_skilled_ratio
    } else {
        1.0
    };
    literacy_ratio.min(skilled_ratio).clamp(0.35, 1.0)
}

pub(crate) fn compute_input_fulfillment_ratio(
    pms: &[&ProductionMethodDef],
    building_level: u8,
    employment_ratio: f32,
    previous_supply: &HashMap<String, f32>,
    previous_stockpile: &HashMap<String, f32>,
) -> f32 {
    if pms.iter().all(|pm| pm.input_good_ids.is_empty()) {
        return 1.0;
    }
    pms.iter()
        .flat_map(|pm| {
            pm.input_good_ids
                .iter()
                .enumerate()
                .map(move |(i, good_id)| (pm, i, good_id))
        })
        .fold(1.0, |ratio, (pm, i, good_id)| {
            let demand = pm.input_good_amounts.get(i).copied().unwrap_or(0.0)
                * building_level as f32
                * employment_ratio;
            if demand <= 0.0 {
                ratio
            } else {
                let available = previous_supply.get(good_id).copied().unwrap_or(0.0)
                    + previous_stockpile.get(good_id).copied().unwrap_or(0.0);
                ratio.min((available / demand).clamp(0.0, 1.0))
            }
        })
}

pub(crate) fn compute_employment_ratio(
    building: &hoi4_state::Building,
    pms: &[&ProductionMethodDef],
) -> f32 {
    if building.level == 0 {
        return 0.0;
    }
    let mut total_needed: f32 = 0.0;
    let mut total_filled: f32 = 0.0;
    for class_idx in 0..6 {
        let needed = pms
            .iter()
            .map(|pm| pm.employment_demand.get(class_idx).copied().unwrap_or(0) as f32)
            .sum::<f32>()
            * building.level as f32;
        let filled = building.employment.get(class_idx).copied().unwrap_or(0) as f32;
        total_needed += needed;
        total_filled += filled.min(needed);
    }
    if total_needed <= 0.0 {
        1.0
    } else {
        total_filled / total_needed
    }
}

pub(crate) fn active_available_pms<'a>(
    building: &hoi4_state::Building,
    db: &'a V6Database,
    ci: usize,
    world: &World,
    completed_techs: &[String],
) -> Vec<&'a ProductionMethodDef> {
    hoi4_content::active_pms_for_building(building, db)
        .into_iter()
        .filter(|pm| pm_unlocked(pm, ci, world, completed_techs))
        .collect()
}

pub(crate) fn pm_unlocked(
    pm: &ProductionMethodDef,
    ci: usize,
    world: &World,
    completed_techs: &[String],
) -> bool {
    if let Some(unlock_tech) = &pm.unlocked_by {
        if !completed_techs.contains(unlock_tech) {
            return false;
        }
    }
    if let Some((cat, law_id)) = &pm.required_law {
        let current =
            &world.countries.law_store.law_sets[ci].0[map_content_law_category(*cat)].current;
        if current != law_id {
            return false;
        }
    }
    true
}

pub(crate) fn map_content_law_category(cat: hoi4_content::v6_loader::LawCategoryDef) -> usize {
    match cat {
        hoi4_content::v6_loader::LawCategoryDef::Conscription => LawCategory::Conscription.index(),
        hoi4_content::v6_loader::LawCategoryDef::Economy => LawCategory::Economy.index(),
        hoi4_content::v6_loader::LawCategoryDef::Trade => LawCategory::Trade.index(),
        hoi4_content::v6_loader::LawCategoryDef::Taxation => LawCategory::Taxation.index(),
        hoi4_content::v6_loader::LawCategoryDef::CivilRights => LawCategory::CivilRights.index(),
        hoi4_content::v6_loader::LawCategoryDef::InformationControl => {
            LawCategory::InformationControl.index()
        }
    }
}

pub(crate) fn state_owned_by_parts(
    state_count: usize,
    state_owners: &[hoi4_state::CountryId],
    state: hoi4_state::StateId,
    country_id: hoi4_state::CountryId,
) -> bool {
    let state_idx = state.0 as usize;
    state_idx < state_count && state_owners[state_idx] == country_id
}

pub(crate) fn state_owned_by_world(
    world: &World,
    state: hoi4_state::StateId,
    country_id: hoi4_state::CountryId,
) -> bool {
    state_owned_by_parts(world.states.count, &world.states.owners, state, country_id)
}
