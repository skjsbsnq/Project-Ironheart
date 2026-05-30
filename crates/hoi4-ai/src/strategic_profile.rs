//! V5 strategic profile: faction building / wartime supply / peacetime PP adjustments.
//!
//! Data moved to profile.rs AiProfile. This module keeps execution logic + tests.
use hoi4_logic::diplomacy::{
    execute_action, factions::create_faction, tick_diplomatic_requests, DiplomaticAction,
};
use hoi4_state::{CountryId, PopClass, World};

use crate::profile::{AiProfile, ProfileRegistry};

pub use crate::profile::FactionRole;

/// Report for a weekly strategic tick (for logging / test assertions).
#[derive(Debug, Default, Clone, PartialEq)]
pub struct StrategicProfileReport {
    pub factions_created: Vec<String>,
    pub factions_joined: Vec<(String, String)>,
    pub members_invited: Vec<(String, String)>,
    pub defensive_boosts: Vec<String>,
    pub peacetime_pp_boosts: Vec<String>,
}

/// List of faction-forming country tags.
const FACTION_TAGS: &[&str] = &["ENG", "FRA", "SOV", "POL", "CZE", "ITA", "JAP", "GER"];

/// Weekly strategic tick entry point.
pub fn weekly_strategic_tick(world: &mut World) -> StrategicProfileReport {
    weekly_strategic_tick_with_registry(world, ProfileRegistry::global())
}

pub fn weekly_strategic_tick_with_registry(
    world: &mut World,
    registry: &ProfileRegistry,
) -> StrategicProfileReport {
    let mut report = StrategicProfileReport::default();
    let date = world.date;

    let profiles: Vec<(&str, AiProfile)> = FACTION_TAGS
        .iter()
        .filter_map(|&tag| {
            if world.tag_to_country.contains_key(tag) {
                Some((tag, registry.get(tag)))
            } else {
                None
            }
        })
        .collect();

    // Phase 1: Faction leaders create factions
    for (tag, profile) in &profiles {
        let Some(country) = world.tag_to_country.get(*tag).copied() else {
            continue;
        };
        if world.diplomacy.annexed_countries.contains(&country) {
            continue;
        }

        if let FactionRole::LeadFaction {
            name,
            by_year,
            by_month,
            by_day,
        } = &profile.faction_role
        {
            if world.diplomacy.faction_of(country).is_some() {
                continue;
            }
            if world.diplomacy.factions.iter().any(|f| f.name == *name) {
                continue;
            }
            if !date_reached(date, *by_year, *by_month, *by_day) {
                continue;
            }

            if create_faction(world, country, name).is_ok() {
                report.factions_created.push(name.clone());
            }
        }
    }

    // Phase 2: Invite members
    for (tag, profile) in &profiles {
        let Some(leader) = world.tag_to_country.get(*tag).copied() else {
            continue;
        };
        let Some(faction_id) = world.diplomacy.faction_of(leader) else {
            continue;
        };
        let FactionRole::LeadFaction {
            name: faction_name, ..
        } = &profile.faction_role
        else {
            continue;
        };
        let Some(faction) = world.diplomacy.faction(faction_id) else {
            continue;
        };
        if faction.leader != leader {
            continue;
        }

        for invitee_tag in &profile.invite_targets {
            let Some(invitee) = world.tag_to_country.get(invitee_tag.as_str()).copied() else {
                continue;
            };
            if world.diplomacy.annexed_countries.contains(&invitee) {
                continue;
            }
            if world.diplomacy.faction_of(invitee).is_some() {
                continue;
            }
            if execute_action(
                world,
                leader,
                DiplomaticAction::InviteToFaction { target: invitee },
            )
            .is_ok()
            {
                report
                    .members_invited
                    .push((tag.to_string(), invitee_tag.clone()));
            }
            let _ = faction_name;
        }
    }

    // Phase 3: JoinFaction role
    for (tag, profile) in &profiles {
        let Some(country) = world.tag_to_country.get(*tag).copied() else {
            continue;
        };
        if world.diplomacy.annexed_countries.contains(&country) {
            continue;
        }
        if world.diplomacy.faction_of(country).is_some() {
            continue;
        }

        let FactionRole::JoinFaction {
            faction_name,
            by_year,
            by_month,
            by_day,
        } = &profile.faction_role
        else {
            continue;
        };
        if !date_reached(date, *by_year, *by_month, *by_day) {
            continue;
        }

        let target_fid = world
            .diplomacy
            .factions
            .iter()
            .find(|f| f.name == *faction_name)
            .map(|f| f.id);
        let Some(fid) = target_fid else {
            continue;
        };
        let Some(leader) = world.diplomacy.faction(fid).map(|f| f.leader) else {
            continue;
        };
        if execute_action(
            world,
            leader,
            DiplomaticAction::InviteToFaction { target: country },
        )
        .is_ok()
        {
            report
                .factions_joined
                .push((tag.to_string(), faction_name.clone()));
        }
    }

    let _ = tick_diplomatic_requests(world);

    // Phase 4: wartime supply + peacetime PP adjustments
    for (tag, profile) in &profiles {
        let Some(country) = world.tag_to_country.get(*tag).copied() else {
            continue;
        };
        if world.diplomacy.annexed_countries.contains(&country) {
            continue;
        }
        let i = country.0 as usize;
        if i >= world.countries.count {
            continue;
        }

        if world.diplomacy.is_at_war(country) {
            let (mp, ws, pp) = profile.defensive_weekly_boost;
            apply_defensive_boost(world, country, mp, ws, pp);
            report.defensive_boosts.push(tag.to_string());
        } else if profile.peacetime_weekly_pp > 0.0 {
            world.countries.political_power[i] = (world.countries.political_power[i]
                + profile.peacetime_weekly_pp)
                .clamp(-1000.0, 1000.0);
            report.peacetime_pp_boosts.push(tag.to_string());
        }
    }

    report
}

fn date_reached(date: hoi4_state::GameDate, year: u16, month: u8, day: u8) -> bool {
    (date.year, date.month, date.day) >= (year, month, day)
}

fn apply_defensive_boost(
    world: &mut World,
    country: CountryId,
    mp_boost: i64,
    ws_boost: f32,
    pp_boost: f32,
) {
    let i = country.0 as usize;
    if i >= world.countries.count {
        return;
    }
    let state_ids = world.country_state_ids(country);
    let soldier_indices = world
        .countries
        .pops
        .pops_by_class_in_country_mut(PopClass::Soldier, &state_ids);
    if let Some(&first) = soldier_indices.first() {
        let add = mp_boost.max(0) as u32;
        world.countries.pops.groups[first].size =
            world.countries.pops.groups[first].size.saturating_add(add);
    }
    world.countries.war_support[i] = (world.countries.war_support[i] + ws_boost).clamp(0.0, 1.0);
    world.countries.political_power[i] =
        (world.countries.political_power[i] + pp_boost).clamp(-1000.0, 1000.0);
}

#[cfg(test)]
mod tests {
    use super::*;
    use hoi4_data::GameData;
    use hoi4_map::GameMap;
    use hoi4_state::StateId;
    use std::collections::HashMap;
    use std::sync::Arc;

    fn test_map() -> Arc<GameMap> {
        Arc::new(GameMap {
            definitions: vec![],
            rgb_to_id: HashMap::new(),
            province_map: hoi4_map::ProvinceMap {
                width: 1,
                height: 1,
                pixels: vec![0],
            },
            adjacencies: vec![vec![]],
            special_adjacencies: vec![],
            heightmap: hoi4_map::Heightmap {
                width: 1,
                height: 1,
                pixels: vec![0],
            },
            terrain_bmp: hoi4_map::TerrainBitmap {
                width: 1,
                height: 1,
                pixels: vec![0],
                palette: [[0u8; 3]; 256],
            },
            terrain_catalog: hoi4_map::TerrainCatalog::default(),
            tree_definition_bmp: None,
            tree_indices: std::collections::HashSet::new(),
        })
    }

    fn test_data() -> Arc<GameData> {
        let mut data = GameData::default();
        let tag = hoi4_data::CountryTag::new("GER");
        data.countries.insert(
            tag.clone(),
            hoi4_data::Country {
                tag,
                color: hoi4_data::Color {
                    r: 80,
                    g: 80,
                    b: 80,
                },
                graphical_culture: "western_european_gfx".to_owned(),
                capital: 1,
                ruling_party: "fascism".to_owned(),
                technologies: vec![],
            },
        );
        data.states.push(hoi4_data::State {
            id: 1,
            name: "Test State".to_owned(),
            manpower: 1_000_000,
            owner: hoi4_data::CountryTag::new("GER"),
            cores: vec![hoi4_data::CountryTag::new("GER")],
            provinces: vec![],
            category: "city".to_owned(),
            infrastructure: 0,
            victory_points: vec![],
            resources: vec![],
        });
        Arc::new(data)
    }

    fn test_world() -> World {
        let mut world = World::new(test_map(), test_data());
        world.countries.pops.groups.push(hoi4_state::PopGroup {
            class: PopClass::Soldier,
            state: StateId(0),
            size: 100_000,
            employed_at: None,
            wage_rm: 0.0,
            tax_burden: 0.0,
            income_rm: 0.0,
            tax_paid_rm: 0.0,
            disposable_income_rm: 0.0,
            basic_consumption_budget: 0.0,
            satisfaction_law_modifier: 0.0,
            loyalty_coefficient: 1.0,
            loyalty_decay_mult: 1.0,
            satisfaction: 1.0,
            political_loyalty: 0.5,
            literacy: PopClass::Soldier.baseline_literacy(),
            skilled_ratio: PopClass::Soldier.baseline_skilled_ratio(),
            standard_of_living: 0.5,
            needs_fulfillment: 1.0,
            essential_needs_fulfillment: 1.0,
            normal_needs_fulfillment: 1.0,
            luxury_needs_fulfillment: 1.0,
            radicalism: 0.0,
        });
        world
    }

    #[test]
    fn test_date_reached() {
        let d = hoi4_state::GameDate {
            year: 1939,
            month: 9,
            day: 1,
            hour: 0,
        };
        assert!(date_reached(d, 1939, 9, 1));
        assert!(date_reached(d, 1939, 8, 31));
        assert!(!date_reached(d, 1939, 9, 2));
    }

    #[test]
    fn test_defensive_boost() {
        let mut world = test_world();
        let ger = CountryId(0);
        let before_ws = world.countries.war_support[0];
        let before_pp = world.countries.political_power[0];
        apply_defensive_boost(&mut world, ger, 500, 0.05, 10.0);
        assert!(world.countries.war_support[0] > before_ws);
        assert!(world.countries.political_power[0] > before_pp);
    }
}
