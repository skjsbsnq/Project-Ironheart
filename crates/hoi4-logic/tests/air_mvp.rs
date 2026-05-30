use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use hoi4_data::air::baseline_for_aircraft;
use hoi4_data::naval::baseline_for_class;
use hoi4_data::{Color, Country, CountryTag, GameData, State};
use hoi4_logic::air::{operations, regions, spawn};
use hoi4_logic::economy::EconomyState;
use hoi4_logic::naval;
use hoi4_map::{
    GameMap, Heightmap, ProvinceDefinition, ProvinceMap, ProvinceType, TerrainBitmap,
    TerrainCatalog,
};
use hoi4_state::{AirMission, Building, BuildingKind, BuildingOwner, StateId, World};

fn test_map() -> Arc<GameMap> {
    let mut definitions = vec![None; 3];
    definitions[1] = Some(ProvinceDefinition {
        id: 1,
        r: 1,
        g: 0,
        b: 0,
        province_type: ProvinceType::Land,
        coastal: true,
        terrain: "plains".to_owned(),
        continent: 1,
    });
    definitions[2] = Some(ProvinceDefinition {
        id: 2,
        r: 2,
        g: 0,
        b: 0,
        province_type: ProvinceType::Land,
        coastal: true,
        terrain: "plains".to_owned(),
        continent: 1,
    });
    Arc::new(GameMap {
        definitions,
        rgb_to_id: HashMap::new(),
        province_map: ProvinceMap {
            width: 1,
            height: 1,
            pixels: vec![0],
        },
        adjacencies: vec![vec![], vec![2], vec![1]],
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
        tree_indices: HashSet::new(),
    })
}

fn test_data() -> Arc<GameData> {
    let mut data = GameData::default();
    for (idx, tag) in ["GER", "ENG"].iter().enumerate() {
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
        data.states.push(State {
            id: (idx + 1) as u16,
            name: format!("{tag} State"),
            manpower: 100_000,
            owner: ctag.clone(),
            cores: vec![ctag],
            provinces: vec![(idx + 1) as u16],
            category: "city".to_owned(),
            infrastructure: 3,
            victory_points: vec![],
            resources: vec![],
        });
    }
    for key in ["fighter", "cas", "strategic_bomber", "naval_bomber"] {
        data.aircraft
            .insert(key.to_owned(), baseline_for_aircraft(key).unwrap());
    }
    data.ship_classes.insert(
        "destroyer".to_owned(),
        baseline_for_class("destroyer").unwrap(),
    );
    Arc::new(data)
}

fn world() -> World {
    let mut world = World::new(test_map(), test_data());
    world.provinces.state_of[1] = StateId(0);
    world.provinces.state_of[2] = StateId(1);
    world.states.controllers[0] = world.country("GER").unwrap();
    world.states.controllers[1] = world.country("ENG").unwrap();
    world.countries.buildings_v6.buildings.push(Building {
        kind: BuildingKind::MilitaryBase,
        building_def_id: "air_base".to_owned(),
        state: StateId(0),
        level: 1,
        active_pm: String::new(),
        active_pm_by_group: Vec::new(),
        employment: [0; 6],
        owner: BuildingOwner::State,
        ownership_shares: Vec::new(),
        requires_law: None,
        production_rate: 0.0,
        output_value_gbp: 0.0,
        wage_rm: 0.0,
        profit_rm: 0.0,
        input_cost_rm: 0.0,
        estimated_profit_rm: 0.0,
        value_added_rm: 0.0,
        cp_cost: 0.0,
        max_level: 1,
        built_progress: 1.0,
    });
    world.countries.buildings_v6.buildings.push(Building {
        state: StateId(1),
        level: 2,
        ..Building::runtime_defaults()
    });
    world
}

#[test]
fn airbase_capacity_limits_deployment() {
    let world = world();
    let ger = world.country("GER").unwrap();
    assert_eq!(regions::airbase_capacity(&world, ger, StateId(0)), 300);
    assert!(regions::can_deploy_to_base(&world, ger, StateId(0), 100));
}

#[test]
fn wing_transfers_to_new_air_region() {
    let mut world = world();
    let data = world.data.clone();
    let ger = world.country("GER").unwrap();
    let wing = spawn::create_air_wing(&mut world, &data, ger, "fighter", 0, 100, "JG").unwrap();
    operations::order_transfer_to_base(&mut world, wing, StateId(0), 1, 0).unwrap();
    assert!(world.air_wings.transfer_arrival_hour[wing.0 as usize] > 0);
    world.date.hour = 23;
    world.date.advance_hour();
    operations::tick_transfers(&mut world);
    assert_eq!(world.air_wings.region_id[wing.0 as usize], 1);
}

#[test]
fn daily_missions_bomb_buildings_and_reinforce_aircraft() {
    let mut world = world();
    let data = world.data.clone();
    let mut econ = EconomyState::new(&world);
    let ger = world.country("GER").unwrap();
    let eng = world.country("ENG").unwrap();
    hoi4_logic::diplomacy::force_declare_war(&mut world, ger, eng).unwrap();
    let bomber =
        spawn::create_air_wing(&mut world, &data, ger, "strategic_bomber", 1, 200, "KG").unwrap();
    world.air_wings.mission[bomber.0 as usize] = AirMission::StrategicBombing;
    world.air_wings.target_region[bomber.0 as usize] = 1;
    world.air_wings.count_planes[bomber.0 as usize] = 180;
    econ.stockpile[ger.0 as usize].insert("aircraft".to_owned(), 120.0);

    let report = operations::tick_daily(&mut world, &mut econ, &data);
    assert!(report.planes_reinforced > 0);
    assert!(report.buildings_destroyed > 0 || report.infrastructure_destroyed > 0);
}

#[test]
fn aircraft_stockpile_creates_reserve_wing() {
    let mut world = world();
    let data = world.data.clone();
    let mut econ = EconomyState::new(&world);
    let ger = world.country("GER").unwrap();
    econ.stockpile[ger.0 as usize].insert("aircraft".to_owned(), 100.0);
    let report = operations::tick_daily(&mut world, &mut econ, &data);
    assert_eq!(report.wings_created, 1);
    assert_eq!(world.air_wings.count, 1);
}

#[test]
fn naval_strike_damages_enemy_fleet() {
    let mut world = world();
    let data = world.data.clone();
    let mut econ = EconomyState::new(&world);
    let ger = world.country("GER").unwrap();
    let eng = world.country("ENG").unwrap();
    hoi4_logic::diplomacy::force_declare_war(&mut world, ger, eng).unwrap();
    let fleet = naval::spawn::create_fleet(&mut world, eng, 0, "Target").unwrap();
    let ship = naval::spawn::spawn_ship(&mut world, &data, fleet, "destroyer", "DD").unwrap();
    let wing =
        spawn::create_air_wing(&mut world, &data, ger, "naval_bomber", 0, 100, "Nav").unwrap();
    world.air_wings.mission[wing.0 as usize] = AirMission::NavalStrike;
    let before = world.ships.hp[ship.0 as usize];
    let report = operations::tick_daily(&mut world, &mut econ, &data);
    assert!(report.naval_damage > 0.0);
    assert!(world.ships.hp[ship.0 as usize] < before);
}
