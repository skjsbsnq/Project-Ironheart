use hoi4_data::ReinforcementPriority;
use hoi4_logic::economy::EconomyState;
use hoi4_logic::military::training::enqueue_training;
use hoi4_state::{CountryId, StateId, World};

use crate::intent::NationalIntent;
use crate::profile::AiProfile;

#[cfg(test)]
mod tests {
    use super::*;
    use hoi4_data::{Country, CountryTag, DivisionTemplate, GameData, State, SubunitDef};
    use hoi4_logic::economy::EconomyState;
    use hoi4_map::{GameMap, Heightmap, ProvinceMap, TerrainBitmap, TerrainCatalog};
    use hoi4_state::{CountryId, PopClass, PopGroup, ProvinceId, StateId, World};
    use std::collections::HashMap;
    use std::sync::Arc;

    fn test_world() -> World {
        let mut data = GameData::default();
        let tag = CountryTag::new("GER");
        data.countries.insert(
            tag.clone(),
            Country {
                tag: tag.clone(),
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
        data.states.push(State {
            id: 1,
            name: "Test State".to_owned(),
            manpower: 1_000_000,
            owner: tag.clone(),
            cores: vec![tag.clone()],
            provinces: vec![0],
            category: "city".to_owned(),
            infrastructure: 3,
            victory_points: vec![],
            resources: vec![],
        });
        data.subunits.insert(
            "infantry".to_owned(),
            SubunitDef {
                key: "infantry".to_owned(),
                group: "infantry".to_owned(),
                combat_width: 2.0,
                manpower: 1_000,
                training_time: 4,
                need: HashMap::from([("infantry_equipment".to_owned(), 100)]),
                ..SubunitDef::default()
            },
        );
        data.division_templates.insert(
            "GER".to_owned(),
            vec![DivisionTemplate {
                name: "Infantry Division".to_owned(),
                country_tag: Some("GER".to_owned()),
                regiments: vec!["infantry".to_owned()],
                support: vec![],
                division_names_group: None,
            }],
        );

        let mut world = World::new(
            Arc::new(GameMap {
                definitions: vec![],
                rgb_to_id: HashMap::new(),
                province_map: ProvinceMap {
                    width: 1,
                    height: 1,
                    pixels: vec![0],
                },
                adjacencies: vec![vec![]],
                special_adjacencies: vec![],
                heightmap: Heightmap {
                    width: 1,
                    height: 1,
                    pixels: vec![0],
                },
                terrain_bmp: TerrainBitmap {
                    width: 1,
                    height: 1,
                    pixels: vec![0],
                    palette: [[0; 3]; 256],
                },
                terrain_catalog: TerrainCatalog::default(),
                tree_definition_bmp: None,
                tree_indices: std::collections::HashSet::new(),
            }),
            Arc::new(data),
        );
        world.states.owners[0] = CountryId(0);
        world.states.controllers[0] = CountryId(0);
        world.states.provinces[0] = vec![ProvinceId(0)];
        world.countries.capitals[0] = StateId(0);
        world.countries.unlocked_equipments[0].insert("infantry_equipment_1".to_owned());
        world.countries.pops.groups.push(PopGroup {
            class: PopClass::Soldier,
            state: StateId(0),
            size: 50_000,
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
        world
    }

    #[test]
    fn recruit_queues_training_instead_of_spawning_division() {
        let mut world = test_world();
        let mut econ = EconomyState::new(&world);
        econ.stockpile[0].insert("infantry_equipment".to_owned(), 1_000.0);
        let profile = AiProfile::default();
        let intent = NationalIntent::default_at_peace(&profile);

        let result = try_recruit(&mut world, &mut econ, CountryId(0), &profile, &intent);

        assert!(result.is_some());
        assert_eq!(
            world.divisions.count, 0,
            "recruit should not spawn division immediately"
        );
        assert_eq!(econ.training_queues[0].len(), 1);
    }
}

pub const RECRUIT_CADENCE_DAYS: u32 = 7;
pub const RECRUIT_CADENCE_DAYS_WAR: u32 = 3;

const MIN_INFANTRY_STOCKPILE_TO_RECRUIT: f32 = 200.0;
const MIN_MANPOWER_TO_RECRUIT: u64 = 10_000;
const AI_SOFT_DIVISION_CAP: u32 = 96;

pub fn try_recruit(
    world: &mut World,
    econ: &mut EconomyState,
    country: CountryId,
    _profile: &AiProfile,
    intent: &NationalIntent,
) -> Option<u32> {
    let ci = country.0 as usize;
    if ci >= world.countries.count {
        return None;
    }
    if world.diplomacy.annexed_countries.contains(&country) {
        return None;
    }

    let current_divs = world
        .divisions
        .owners
        .iter()
        .take(world.divisions.count)
        .filter(|&&o| o == country)
        .count() as u32;

    let target = intent.target_divisions.min(AI_SOFT_DIVISION_CAP);
    if current_divs >= target {
        return None;
    }

    let tag = match world.country_tag(country) {
        Some(t) => t.to_owned(),
        None => return None,
    };
    let templates = match world.data.division_templates.get(&tag) {
        Some(t) if !t.is_empty() => t,
        _ => return None,
    };
    let template_idx = templates
        .iter()
        .position(|t| {
            t.name.to_lowercase().contains("infantry")
                || t.regiments.iter().any(|r| r.contains("infantry"))
        })
        .unwrap_or(0) as u32;

    let manpower = world.manpower(country);
    if manpower < MIN_MANPOWER_TO_RECRUIT {
        return None;
    }
    if ci < econ.stockpile.len() {
        let infantry_eq = econ.stockpile[ci]
            .get("infantry_equipment")
            .copied()
            .unwrap_or(0.0);
        if infantry_eq < MIN_INFANTRY_STOCKPILE_TO_RECRUIT {
            return None;
        }
    } else {
        return None;
    }

    let deploy_state = find_deploy_state(world, country, intent)?;
    let data_clone = world.data.clone();
    let queue_id = enqueue_training(
        world,
        econ,
        &data_clone,
        country,
        template_idx,
        1,
        deploy_state,
        ReinforcementPriority::Normal,
    )
    .ok()?;

    Some(queue_id)
}

fn find_deploy_state(
    world: &World,
    country: CountryId,
    intent: &NationalIntent,
) -> Option<StateId> {
    let ci = country.0 as usize;
    if !world.countries.at_war[ci] || intent.front_assignments.is_empty() {
        return capital_fallback_state(world, country);
    }

    let mut best: Option<(StateId, u32)> = None;
    for (&enemy, fa) in &intent.front_assignments {
        if fa.posture != crate::ground::GroundPosture::Attack
            && fa.posture != crate::ground::GroundPosture::Defend
        {
            continue;
        }
        let seg = crate::frontline::compute_segment(world, country, enemy);
        for &sid in &seg.friendly_states {
            let si = sid.0 as usize;
            if si >= world.states.count {
                continue;
            }
            if world.states.controllers[si] != country {
                continue;
            }
            let is_front = world.states.provinces[si].iter().any(|&p| {
                let pi = p.0 as usize;
                if pi >= world.provinces.count || world.provinces.controllers[pi] != country {
                    return false;
                }
                world.map.adjacencies.get(pi).map_or(false, |adj| {
                    adj.iter().any(|&nb| {
                        if (nb as usize) >= world.provinces.count {
                            return false;
                        }
                        let nb_ctrl = world.provinces.controllers[nb as usize];
                        !nb_ctrl.is_none()
                            && nb_ctrl != country
                            && world.diplomacy.at_war_with(country, nb_ctrl)
                    })
                })
            });
            if is_front {
                let infra = world.states.infrastructure[si];
                match best {
                    None => best = Some((sid, infra as u32)),
                    Some((_, prev_infra)) if infra as u32 > prev_infra => {
                        best = Some((sid, infra as u32))
                    }
                    _ => {}
                }
            }
        }
    }

    best.map(|(s, _)| s)
        .or_else(|| capital_fallback_state(world, country))
}

fn capital_fallback_state(world: &World, country: CountryId) -> Option<StateId> {
    let ci = country.0 as usize;
    if ci >= world.countries.count {
        return None;
    }
    let cap_state = world.countries.capitals[ci];
    if cap_state.is_none() {
        return None;
    }
    let cap_si = cap_state.0 as usize;
    if cap_si >= world.states.count {
        return None;
    }
    Some(StateId(cap_si as u16))
}
