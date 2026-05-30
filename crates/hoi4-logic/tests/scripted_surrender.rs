use std::collections::HashMap;
use std::sync::Arc;

use hoi4_content::{ScriptedSurrenderDef, SituationEffect};
use hoi4_data::{Color, Country, CountryTag, GameData, State};
use hoi4_logic::diplomacy::{apply_scripted_surrenders, evaluate_scripted_surrender};
use hoi4_logic::scripted_effects::ScriptedDiplomaticEffect;
use hoi4_map::{GameMap, Heightmap, ProvinceMap, TerrainBitmap, TerrainCatalog};
use hoi4_state::World;

fn test_map() -> Arc<GameMap> {
    Arc::new(GameMap {
        definitions: vec![],
        rgb_to_id: HashMap::new(),
        province_map: ProvinceMap {
            width: 1,
            height: 1,
            pixels: vec![0],
        },
        adjacencies: vec![],
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
    })
}

fn test_data() -> Arc<GameData> {
    let mut data = GameData::default();
    for (idx, tag) in ["GER", "FRA"].iter().enumerate() {
        let ctag = CountryTag::new(tag);
        data.countries.insert(
            ctag.clone(),
            Country {
                tag: ctag.clone(),
                color: Color {
                    r: 80,
                    g: 80,
                    b: 80,
                },
                graphical_culture: "western_european_gfx".to_owned(),
                capital: (idx + 1) as u16,
                ruling_party: "neutrality".to_owned(),
                technologies: vec![],
            },
        );
    }
    for (id, owner) in [(1, "FRA"), (2, "FRA"), (3, "FRA"), (4, "GER")] {
        let ctag = CountryTag::new(owner);
        data.states.push(State {
            id,
            name: format!("State {id}"),
            manpower: 100_000,
            owner: ctag.clone(),
            cores: vec![ctag],
            provinces: vec![],
            category: "city".to_owned(),
            infrastructure: 3,
            victory_points: vec![],
            resources: vec![],
        });
    }
    Arc::new(data)
}

fn world() -> World {
    World::new(test_map(), test_data())
}

fn fra_surrender_def() -> ScriptedSurrenderDef {
    ScriptedSurrenderDef {
        id: "fra_surrender".to_owned(),
        target: "FRA".to_owned(),
        enemy_side: vec!["GER".to_owned()],
        require_capital_lost: true,
        max_target_core_control_ratio: 0.35,
        remove_from_wars: true,
        effects: vec![
            SituationEffect::TriggerEvent("news.france_surrenders".to_owned()),
            SituationEffect::SetCountryFlag {
                country: "FRA".to_owned(),
                flag: "surrendered_to_axis".to_owned(),
            },
            SituationEffect::SetCountryFlag {
                country: "GER".to_owned(),
                flag: "france_defeated".to_owned(),
            },
        ],
    }
}

#[test]
fn scripted_surrender_requires_capital_and_core_collapse() {
    let mut world = world();
    let ger = world.country("GER").unwrap();
    let fra = world.country("FRA").unwrap();
    hoi4_logic::scripted_effects::apply_diplomatic_effect(
        &mut world,
        &ScriptedDiplomaticEffect::DeclareWar {
            attacker: "GER".to_owned(),
            defender: "FRA".to_owned(),
        },
    )
    .unwrap();

    let def = fra_surrender_def();
    assert!(evaluate_scripted_surrender(&world, &def).is_none());

    for si in 0..world.states.count {
        if world.states.owners[si] == fra && world.states.cores[si].contains(&fra) {
            world.states.controllers[si] = ger;
        }
    }

    let eval = evaluate_scripted_surrender(&world, &def).expect("France should surrender");
    assert!(eval.capital_lost);
    assert_eq!(eval.target_core_control_ratio, 0.0);
}

#[test]
fn scripted_surrender_applies_once_and_removes_from_war() {
    let mut world = world();
    let ger = world.country("GER").unwrap();
    let fra = world.country("FRA").unwrap();
    hoi4_logic::scripted_effects::apply_diplomatic_effect(
        &mut world,
        &ScriptedDiplomaticEffect::DeclareWar {
            attacker: "GER".to_owned(),
            defender: "FRA".to_owned(),
        },
    )
    .unwrap();
    for si in 0..world.states.count {
        if world.states.owners[si] == fra && world.states.cores[si].contains(&fra) {
            world.states.controllers[si] = ger;
        }
    }

    let def = fra_surrender_def();
    let outcome = apply_scripted_surrenders(&mut world, &[def.clone()]);
    assert_eq!(outcome.resolved.len(), 1);
    assert_eq!(outcome.triggered_events.len(), 1);
    assert_eq!(
        outcome.triggered_events[0].event_id,
        "news.france_surrenders"
    );
    assert_eq!(outcome.triggered_events[0].effect_country, "FRA");
    assert!(!world.diplomacy.is_at_war(fra));
    assert!(world.countries.ideas[fra.0 as usize].contains(&"SURRENDER:fra_surrender".to_owned()));
    assert!(world.countries.ideas[fra.0 as usize].contains(&"FLAG:surrendered_to_axis".to_owned()));
    assert!(world.countries.ideas[ger.0 as usize].contains(&"FLAG:france_defeated".to_owned()));

    let second = apply_scripted_surrenders(&mut world, &[def]);
    assert!(second.resolved.is_empty());
}
