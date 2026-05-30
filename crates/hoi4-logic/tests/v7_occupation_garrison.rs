use std::collections::HashMap;
use std::sync::Arc;

use hoi4_data::{Color, Country, CountryTag, DivisionTemplate, GameData, State, SubunitDef};
use hoi4_logic::occupation::{
    occupation_state, set_garrison_template, set_occupation_policy, tick_occupation_daily,
};
use hoi4_map::{GameMap, Heightmap, ProvinceMap, TerrainBitmap, TerrainCatalog};
use hoi4_state::{Building, BuildingKind, BuildingOwner, OccupationPolicy, StateId, World};

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
    for tag in ["FRA", "GER"] {
        let ctag = CountryTag::new(tag);
        data.countries.insert(
            ctag.clone(),
            Country {
                tag: ctag,
                color: Color {
                    r: 80,
                    g: 80,
                    b: 80,
                },
                graphical_culture: "western_european_gfx".to_owned(),
                capital: 1,
                ruling_party: "neutrality".to_owned(),
                technologies: vec![],
            },
        );
    }
    data.states.push(State {
        id: 1,
        name: "Occupied State".to_owned(),
        manpower: 1_000_000,
        owner: CountryTag::new("FRA"),
        cores: vec![CountryTag::new("FRA")],
        provinces: vec![],
        category: "city".to_owned(),
        infrastructure: 3,
        victory_points: vec![],
        resources: vec![],
    });

    data.subunits.insert(
        "cavalry".to_owned(),
        SubunitDef {
            key: "cavalry".to_owned(),
            group: "cavalry".to_owned(),
            manpower: 1_000,
            suppression: 8.0,
            ..SubunitDef::default()
        },
    );
    data.division_templates.insert(
        "GER".to_owned(),
        vec![DivisionTemplate {
            name: "Garrison".to_owned(),
            country_tag: Some("GER".to_owned()),
            regiments: vec!["cavalry".to_owned()],
            support: vec![],
            division_names_group: None,
        }],
    );
    Arc::new(data)
}

fn occupied_world() -> World {
    let mut world = World::new(test_map(), test_data());
    let ger = world.tag_to_country["GER"];
    world.states.controllers[0] = ger;
    let mut building = Building::runtime_defaults();
    building.kind = BuildingKind::Industrial;
    building.building_def_id = "steel_mill".to_owned();
    building.state = StateId(0);
    building.level = 4;
    building.owner = BuildingOwner::State;
    building.production_rate = 1.0;
    world.countries.buildings_v6.buildings.push(building);
    world
}

#[test]
fn garrison_suppression_reduces_resistance_growth() {
    let mut no_garrison = occupied_world();
    let data = no_garrison.data.clone();
    tick_occupation_daily(&mut no_garrison, data.as_ref());
    let no_garrison_resistance = no_garrison.states.resistance[0];

    let mut with_garrison = occupied_world();
    set_garrison_template(&mut with_garrison, StateId(0), Some(0)).unwrap();
    let data = with_garrison.data.clone();
    tick_occupation_daily(&mut with_garrison, data.as_ref());

    let view = occupation_state(&with_garrison, StateId(0)).unwrap();
    assert_eq!(view.occupier, with_garrison.tag_to_country["GER"]);
    assert!(view.provided_suppression > 0.0);
    assert!(with_garrison.states.resistance[0] < no_garrison_resistance);
}

#[test]
fn resistance_sabotage_damages_buildings_and_rails() {
    let mut world = occupied_world();
    set_occupation_policy(&mut world, StateId(0), OccupationPolicy::LootingEconomy).unwrap();
    world.states.resistance[0] = 80.0;
    let before_level = world.countries.buildings_v6.buildings[0].level;
    let data = world.data.clone();

    tick_occupation_daily(&mut world, data.as_ref());

    assert!(world.states.last_sabotage_day[0] >= 0);
    assert!(world.countries.buildings_v6.buildings[0].level < before_level);
}
