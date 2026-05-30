use std::collections::HashMap;
use std::sync::Arc;

use hoi4_data::{
    Color, Country, CountryTag, DivisionTemplate, GameData, ReinforcementPriority, State,
    SubunitDef,
};
use hoi4_logic::economy::stockpile;
use hoi4_logic::economy::EconomyState;
use hoi4_logic::military::training::{enqueue_training, tick_training_queues};
use hoi4_map::{GameMap, Heightmap, ProvinceMap, TerrainBitmap, TerrainCatalog};
use hoi4_state::{CountryId, PopClass, PopGroup, ProvinceId, StateId, World};

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
    let tag = CountryTag::new("GER");
    data.countries.insert(
        tag.clone(),
        Country {
            tag: tag.clone(),
            color: Color {
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
        owner: tag,
        cores: vec![CountryTag::new("GER")],
        provinces: vec![1],
        category: "city".to_owned(),
        infrastructure: 3,
        victory_points: vec![],
        resources: vec![],
    });

    let mut infantry_need = HashMap::new();
    infantry_need.insert("infantry_equipment".to_owned(), 100);
    data.subunits.insert(
        "infantry".to_owned(),
        SubunitDef {
            key: "infantry".to_owned(),
            group: "infantry".to_owned(),
            combat_width: 2.0,
            manpower: 1_000,
            training_time: 4,
            need: infantry_need,
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
    Arc::new(data)
}

fn world_and_econ() -> (World, EconomyState) {
    let data = test_data();
    let mut world = World::new(test_map(), data);
    world.states.provinces[0] = vec![ProvinceId(0)];
    world.states.controllers[0] = CountryId(0);
    world.states.manpower_pool[0] = 100_000;
    let econ = EconomyState::new(&world);
    (world, econ)
}

#[test]
fn training_queue_records_equipment_shortfall_as_negative_stockpile() {
    let (world, mut econ) = world_and_econ();
    let data = world.data.clone();

    enqueue_training(
        &world,
        &mut econ,
        data.as_ref(),
        CountryId(0),
        0,
        1,
        hoi4_state::StateId(0),
        ReinforcementPriority::Normal,
    )
    .unwrap();

    let mut world = world;
    let deployed = tick_training_queues(&mut world, &mut econ, data.as_ref());

    assert!(deployed.is_empty());
    assert_eq!(world.divisions.count, 0);
    assert!(econ.training_queues[0][0].progress_days > 0.0);
    assert!(econ.stockpile[0]["infantry_equipment"] < 0.0);
}

#[test]
fn training_queue_deploys_after_time_and_equipment() {
    let (mut world, mut econ) = world_and_econ();
    let data = world.data.clone();
    world.countries.pops.groups.clear();
    world.countries.pops.groups.push(PopGroup {
        class: PopClass::Soldier,
        state: StateId(0),
        size: 1_500,
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
    let manpower_before = world.manpower(CountryId(0));
    econ.stockpile[0].insert("infantry_equipment".to_owned(), 100.0);
    enqueue_training(
        &world,
        &mut econ,
        data.as_ref(),
        CountryId(0),
        0,
        1,
        hoi4_state::StateId(0),
        ReinforcementPriority::Normal,
    )
    .unwrap();

    for _ in 0..8 {
        tick_training_queues(&mut world, &mut econ, data.as_ref());
    }

    assert_eq!(world.divisions.count, 1);
    assert!(econ.training_queues[0].is_empty());
    assert!(world.divisions.strength[0] > 0.0);
    assert!(
        world.manpower(CountryId(0)) < manpower_before,
        "training should consume soldier POP manpower"
    );
}

#[test]
fn fielded_division_maintenance_consumes_stockpile() {
    let (mut world, mut econ) = world_and_econ();
    let data = world.data.clone();
    world.divisions.push(
        CountryId(0),
        ProvinceId(0),
        0,
        30.0,
        100.0,
        "Field Division".to_owned(),
    );
    world.divisions.strength[0] = 1.0;
    econ.stockpile[0].insert("infantry_equipment".to_owned(), 0.0);

    stockpile::tick(&mut world, &mut econ, data.as_ref());

    assert!(econ.stockpile[0]["infantry_equipment"] < 0.0);
}
