use hoi4_content::V6Database;
use hoi4_state::{BuildingId, CountryId, LawCategory, PopClass, StateId, World};

use super::building_tick_common::country_pop_indices;

#[derive(Clone, Copy, Debug)]
pub struct AccumulatedModifiers {
    pub conscription_max_ratio: f32,
    pub domestic_recruitable_ratio: f32,
    pub colonial_recruitable_ratio: f32,
    pub subject_force_contribution_ratio: f32,
    pub political_cost_multiplier: f32,
    pub radicalism_gain_multiplier: f32,
    pub recruit_speed_mult: f32,
    pub income_tax_rate: f32,
    pub consumption_tax_rate: f32,
    pub corporate_tax_rate: f32,
    pub satisfaction_law_modifier: f32,
    pub loyalty_coefficient: f32,
    pub research_slots_max: u8,
    pub loyalty_decay_mult: f32,
}

impl Default for AccumulatedModifiers {
    fn default() -> Self {
        Self {
            conscription_max_ratio: 0.0,
            domestic_recruitable_ratio: 1.0,
            colonial_recruitable_ratio: 0.0,
            subject_force_contribution_ratio: 0.0,
            political_cost_multiplier: 1.0,
            radicalism_gain_multiplier: 1.0,
            recruit_speed_mult: 1.0,
            income_tax_rate: 0.20,
            consumption_tax_rate: 0.10,
            corporate_tax_rate: 0.20,
            satisfaction_law_modifier: 0.0,
            loyalty_coefficient: 1.0,
            research_slots_max: 2,
            loyalty_decay_mult: 1.0,
        }
    }
}

pub fn accum_modifiers(world: &World, db: &V6Database, ci: usize) -> AccumulatedModifiers {
    let mut out = AccumulatedModifiers::default();
    if ci >= world.countries.count {
        return out;
    }
    let laws = &world.countries.law_store.law_sets[ci].0;

    let conscription_id = &laws[LawCategory::Conscription.index()].current;
    if let Some(def) = db
        .conscription_laws
        .iter()
        .find(|l| &l.id == conscription_id)
    {
        out.conscription_max_ratio = def.soldier_ratio;
        out.domestic_recruitable_ratio = def.domestic_recruitable_ratio;
        out.colonial_recruitable_ratio = def.colonial_recruitable_ratio;
        out.subject_force_contribution_ratio = def.subject_force_contribution_ratio;
        out.political_cost_multiplier = def.political_cost_multiplier;
        out.radicalism_gain_multiplier = def.radicalism_gain_multiplier;
        out.recruit_speed_mult = def.conscription_conversion_rate;
        if conscription_id == "volunteer_only" {
            out.recruit_speed_mult = 0.0;
        }
        out.satisfaction_law_modifier += def.pop_modifiers.satisfaction;
    }

    let taxation_id = &laws[LawCategory::Taxation.index()].current;
    if let Some(def) = db.taxation_laws.iter().find(|l| &l.id == taxation_id) {
        out.income_tax_rate = def.income_tax_rate;
        out.consumption_tax_rate = def.consumption_tax_rate;
        out.corporate_tax_rate = def.corporate_tax_rate;
        out.satisfaction_law_modifier += def.pop_modifiers.satisfaction;
    }

    let civil_rights_id = &laws[LawCategory::CivilRights.index()].current;
    if let Some(def) = db
        .civil_rights_laws
        .iter()
        .find(|l| &l.id == civil_rights_id)
    {
        out.satisfaction_law_modifier += def.pop_modifiers.satisfaction;
        out.loyalty_coefficient = def.pop_modifiers.loyalty_coefficient;
        out.research_slots_max = def.research_slots.max(1);
    }

    let info_id = &laws[LawCategory::InformationControl.index()].current;
    if let Some(def) = db
        .information_control_laws
        .iter()
        .find(|l| &l.id == info_id)
    {
        out.satisfaction_law_modifier += def.pop_modifiers.satisfaction;
        out.loyalty_decay_mult = def.loyalty_decay_multiplier;
    }

    out
}

pub fn apply(world: &mut World, db: &V6Database, ci: usize) -> AccumulatedModifiers {
    let modifiers = accum_modifiers(world, db, ci);
    if ci >= world.countries.count {
        return modifiers;
    }

    world.countries.conscription_max_ratio[ci] = modifiers.conscription_max_ratio;
    world.countries.conscription_recruit_speed_mult[ci] = modifiers.recruit_speed_mult;
    world.countries.research_slots[ci] = modifiers.research_slots_max;
    world.countries.treasury.treasuries[ci].tax_rates = [
        modifiers.income_tax_rate,
        modifiers.consumption_tax_rate,
        modifiers.corporate_tax_rate,
    ];

    let country_id = CountryId(ci as u16);
    let state_count = world.states.count;
    let state_owners = &world.states.owners;
    let first_state = (0..state_count)
        .find(|&si| state_owners[si] == country_id)
        .map(|si| hoi4_state::StateId(si as u16));
    for pop_idx in country_pop_indices(world, ci) {
        let Some(pg) = world.countries.pops.groups.get_mut(pop_idx) else {
            continue;
        };
        pg.satisfaction_law_modifier = modifiers.satisfaction_law_modifier;
        pg.loyalty_coefficient = modifiers.loyalty_coefficient;
        pg.loyalty_decay_mult = modifiers.loyalty_decay_mult;
    }

    apply_conscription_conversion(world, ci, country_id, first_state, modifiers);

    modifiers
}

fn apply_conscription_conversion(
    world: &mut World,
    ci: usize,
    country_id: CountryId,
    first_state: Option<hoi4_state::StateId>,
    modifiers: AccumulatedModifiers,
) {
    if modifiers.conscription_max_ratio <= 0.0 || modifiers.recruit_speed_mult <= 0.0 {
        return;
    }

    let mut domestic_civilian_capacity = 0f32;
    let mut colonial_civilian_capacity = 0f32;
    let mut domestic_soldier_total = 0u32;
    let mut colonial_soldier_total = 0u32;
    let pop_indices = country_pop_indices(world, ci);
    for &pop_idx in &pop_indices {
        let Some(pg) = world.countries.pops.groups.get(pop_idx) else {
            continue;
        };
        let is_colonial = state_counts_as_colonial(world, pg.state, country_id);
        match pg.class {
            PopClass::Soldier if is_colonial => {
                colonial_soldier_total = colonial_soldier_total.saturating_add(pg.size)
            }
            PopClass::Soldier => {
                domestic_soldier_total = domestic_soldier_total.saturating_add(pg.size)
            }
            _ if is_colonial => {
                colonial_civilian_capacity += pg.size as f32 * modifiers.colonial_recruitable_ratio
            }
            _ => {
                domestic_civilian_capacity += pg.size as f32 * modifiers.domestic_recruitable_ratio
            }
        }
    }

    if domestic_civilian_capacity <= 0.0 && colonial_civilian_capacity <= 0.0 {
        return;
    }

    let domestic_target = (domestic_civilian_capacity * modifiers.conscription_max_ratio) as u32;
    let colonial_target = (colonial_civilian_capacity * modifiers.conscription_max_ratio) as u32;
    let domestic_deficit = domestic_target.saturating_sub(domestic_soldier_total);
    let colonial_deficit = colonial_target.saturating_sub(colonial_soldier_total);
    let domestic_daily_cap = ((domestic_civilian_capacity * modifiers.recruit_speed_mult) / 365.0)
        .ceil()
        .max(1.0) as u32;
    let colonial_daily_cap = ((colonial_civilian_capacity * modifiers.recruit_speed_mult) / 365.0)
        .ceil()
        .max(1.0) as u32;

    convert_civilians_to_soldiers(
        world,
        country_id,
        ci,
        first_state,
        modifiers,
        false,
        domestic_deficit.min(domestic_daily_cap),
    );
    convert_civilians_to_soldiers(
        world,
        country_id,
        ci,
        first_state,
        modifiers,
        true,
        colonial_deficit.min(colonial_daily_cap),
    );
}

fn convert_civilians_to_soldiers(
    world: &mut World,
    country_id: CountryId,
    ci: usize,
    first_state: Option<StateId>,
    modifiers: AccumulatedModifiers,
    colonial_source: bool,
    mut remaining: u32,
) {
    if remaining == 0 {
        return;
    }

    for allow_employed in [false, true] {
        for class in [PopClass::Peasant, PopClass::Worker, PopClass::Clerk] {
            let pop_indices = country_pop_indices(world, ci);
            for pop_idx in pop_indices {
                if remaining == 0 {
                    break;
                }
                let Some(pg) = world.countries.pops.groups.get(pop_idx) else {
                    continue;
                };
                if pg.class != class
                    || (!allow_employed && pg.employed_at.is_some())
                    || state_counts_as_colonial(world, pg.state, country_id) != colonial_source
                {
                    continue;
                }
                let take = pg.size.min(remaining);
                if take == 0 {
                    continue;
                }
                let employed_at = world.countries.pops.groups[pop_idx].employed_at;
                let state = world.countries.pops.groups[pop_idx].state;
                world.countries.pops.groups[pop_idx].size -= take;
                if colonial_source {
                    world.countries.pops.groups[pop_idx].radicalism =
                        (world.countries.pops.groups[pop_idx].radicalism
                            + 0.000_002 * take as f32 * modifiers.radicalism_gain_multiplier)
                            .clamp(0.0, 1.0);
                }
                if let Some(building_id) = employed_at {
                    reduce_building_employment(world, building_id, class, take);
                }
                remaining -= take;
                add_soldiers_to_state(world, state, take, modifiers);
            }
            if remaining == 0 {
                break;
            }
        }
        if remaining == 0 {
            break;
        }
    }

    if remaining == 0 {
        return;
    }
    let fallback_state = first_state.unwrap_or(StateId(0));
    add_soldiers_to_state(world, fallback_state, 0, modifiers);
}

fn add_soldiers_to_state(
    world: &mut World,
    state: StateId,
    added_soldiers: u32,
    modifiers: AccumulatedModifiers,
) {
    if let Some(pg) =
        world.countries.pops.groups.iter_mut().find(|pg| {
            pg.class == PopClass::Soldier && pg.state == state && pg.employed_at.is_none()
        })
    {
        pg.size = pg.size.saturating_add(added_soldiers);
    } else {
        world.push_pop_group(hoi4_state::PopGroup {
            class: PopClass::Soldier,
            state,
            size: added_soldiers,
            employed_at: None,
            wage_rm: 0.0,
            tax_burden: 0.0,
            income_rm: 0.0,
            tax_paid_rm: 0.0,
            disposable_income_rm: 0.0,
            basic_consumption_budget: 0.0,
            satisfaction_law_modifier: modifiers.satisfaction_law_modifier,
            loyalty_coefficient: modifiers.loyalty_coefficient,
            loyalty_decay_mult: modifiers.loyalty_decay_mult,
            satisfaction: 0.5,
            political_loyalty: 0.0,
            literacy: PopClass::Soldier.baseline_literacy(),
            skilled_ratio: PopClass::Soldier.baseline_skilled_ratio(),
            standard_of_living: 0.5,
            needs_fulfillment: 1.0,
            essential_needs_fulfillment: 1.0,
            normal_needs_fulfillment: 1.0,
            luxury_needs_fulfillment: 1.0,
            radicalism: 0.0,
        });
    }
}

fn reduce_building_employment(
    world: &mut World,
    building_id: BuildingId,
    class: PopClass,
    amount: u32,
) {
    let Some(building) = world
        .countries
        .buildings_v6
        .buildings
        .get_mut(building_id.0 as usize)
    else {
        return;
    };
    let class_idx = class.index();
    building.employment[class_idx] = building.employment[class_idx].saturating_sub(amount);
}

fn state_counts_as_colonial(world: &World, state: StateId, country_id: CountryId) -> bool {
    let state_idx = state.0 as usize;
    if state_idx >= world.states.count {
        return false;
    }
    let status = world.states.integration_status[state_idx];
    (status == hoi4_state::StateIntegrationStatus::Metropole
        && !world.states.cores[state_idx].contains(&country_id))
        || status.is_colonial_or_occupied()
}
