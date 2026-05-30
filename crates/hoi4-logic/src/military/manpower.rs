use hoi4_content::V6Database;
use hoi4_state::{CountryId, LawCategory, PopClass, StateIntegrationStatus, World};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RecruitableManpowerBreakdown {
    pub domestic: u64,
    pub colonial: u64,
    pub subject: u64,
    pub total: u64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConscriptionManpowerPolicy {
    pub soldier_ratio: f32,
    pub domestic_recruitable_ratio: f32,
    pub colonial_recruitable_ratio: f32,
    pub subject_force_contribution_ratio: f32,
    pub political_cost_multiplier: f32,
    pub radicalism_gain_multiplier: f32,
}

impl Default for ConscriptionManpowerPolicy {
    fn default() -> Self {
        Self {
            soldier_ratio: 0.0,
            domestic_recruitable_ratio: 1.0,
            colonial_recruitable_ratio: 0.0,
            subject_force_contribution_ratio: 0.0,
            political_cost_multiplier: 1.0,
            radicalism_gain_multiplier: 1.0,
        }
    }
}

pub fn conscription_policy(
    world: &World,
    db: &V6Database,
    country: CountryId,
) -> ConscriptionManpowerPolicy {
    if country.is_none() || country.0 as usize >= world.countries.count {
        return ConscriptionManpowerPolicy::default();
    }
    let current = &world.countries.law_store.law_sets[country.0 as usize].0
        [LawCategory::Conscription.index()]
    .current;
    db.conscription_laws
        .iter()
        .find(|law| &law.id == current)
        .map(|law| ConscriptionManpowerPolicy {
            soldier_ratio: law.soldier_ratio,
            domestic_recruitable_ratio: law.domestic_recruitable_ratio,
            colonial_recruitable_ratio: law.colonial_recruitable_ratio,
            subject_force_contribution_ratio: law.subject_force_contribution_ratio,
            political_cost_multiplier: law.political_cost_multiplier,
            radicalism_gain_multiplier: law.radicalism_gain_multiplier,
        })
        .unwrap_or_default()
}

pub fn recruitable_manpower_breakdown(
    world: &World,
    country: CountryId,
    policy: ConscriptionManpowerPolicy,
) -> RecruitableManpowerBreakdown {
    if country.is_none() || country.0 as usize >= world.countries.count {
        return RecruitableManpowerBreakdown::default();
    }

    let mut out = RecruitableManpowerBreakdown::default();
    for pg in &world.countries.pops.groups {
        let state_idx = pg.state.0 as usize;
        if state_idx >= world.states.count || pg.class == PopClass::Soldier {
            continue;
        }
        if world.states.owners[state_idx] != country {
            continue;
        }
        let is_core_state = world.states.cores[state_idx].contains(&country);
        let status = world.states.integration_status[state_idx];
        let ratio = match status {
            StateIntegrationStatus::Metropole if !is_core_state => {
                policy.colonial_recruitable_ratio
            }
            status if status.is_domestic() => policy.domestic_recruitable_ratio,
            status if status.is_colonial_or_occupied() => policy.colonial_recruitable_ratio,
            _ => policy.domestic_recruitable_ratio,
        };
        let recruitable = (pg.size as f32 * policy.soldier_ratio * ratio).floor() as u64;
        if status.is_domestic() && (status != StateIntegrationStatus::Metropole || is_core_state) {
            out.domestic = out.domestic.saturating_add(recruitable);
        } else {
            out.colonial = out.colonial.saturating_add(recruitable);
        }
    }

    for autonomy in world
        .diplomacy
        .autonomy
        .values()
        .filter(|autonomy| autonomy.master == country)
    {
        let subject_pop = world.country_governed_population(autonomy.subject);
        let share = policy
            .subject_force_contribution_ratio
            .min(autonomy.level.master_manpower_share());
        out.subject = out
            .subject
            .saturating_add((subject_pop as f32 * policy.soldier_ratio * share).floor() as u64);
    }
    out.total = out.domestic + out.colonial + out.subject;
    out
}
