use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use hoi4_data::naval::baseline_for_class;
use hoi4_data::{Color, Country, CountryTag, GameData, State};
use hoi4_logic::economy::EconomyState;
use hoi4_logic::naval::{missions, movement, spawn};
use hoi4_map::{
    GameMap, Heightmap, ProvinceDefinition, ProvinceMap, ProvinceType, TerrainBitmap,
    TerrainCatalog,
};
use hoi4_state::{NavalMission, StateId, World};

fn test_map() -> Arc<GameMap> {
    let mut definitions = vec![None; 5];
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
        r: 0,
        g: 0,
        b: 2,
        province_type: ProvinceType::Sea,
        coastal: false,
        terrain: "ocean".to_owned(),
        continent: 0,
    });
    definitions[3] = Some(ProvinceDefinition {
        id: 3,
        r: 0,
        g: 0,
        b: 3,
        province_type: ProvinceType::Sea,
        coastal: false,
        terrain: "ocean".to_owned(),
        continent: 0,
    });
    definitions[4] = Some(ProvinceDefinition {
        id: 4,
        r: 4,
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
        adjacencies: vec![vec![], vec![2], vec![1, 3], vec![2, 4], vec![3]],
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
    for tag in ["GER", "ENG"] {
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
                capital: 1,
                ruling_party: "neutrality".to_owned(),
                technologies: vec![],
            },
        );
    }
    data.states.push(State {
        id: 1,
        name: "Port A".to_owned(),
        manpower: 100_000,
        owner: CountryTag::new("GER"),
        cores: vec![CountryTag::new("GER")],
        provinces: vec![1],
        category: "city".to_owned(),
        infrastructure: 3,
        victory_points: vec![],
        resources: vec![],
    });
    data.ship_classes.insert(
        "destroyer".to_owned(),
        baseline_for_class("destroyer").unwrap(),
    );
    data.ship_classes.insert(
        "submarine".to_owned(),
        baseline_for_class("submarine").unwrap(),
    );
    Arc::new(data)
}

fn world() -> World {
    let mut world = World::new(test_map(), test_data());
    world.provinces.state_of[1] = StateId(0);
    world.provinces.state_of[4] = StateId(0);
    world
}

#[test]
fn fleet_moves_between_sea_regions() {
    let mut world = world();
    let ger = world.country("GER").unwrap();
    let fleet = spawn::create_fleet(&mut world, ger, 2, "Test Fleet").unwrap();
    movement::order_move_to_region(&mut world, fleet, 3, 0).unwrap();
    assert!(movement::is_moving(&world, fleet));
    world.date.hour = 23;
    world.date.advance_hour();
    movement::tick_fleet_movement(&mut world);
    assert_eq!(world.fleets.region_id[fleet.0 as usize], 3);
    assert!(!movement::is_moving(&world, fleet));
}

#[test]
fn escort_reduces_convoy_raiding_risk() {
    let mut world = world();
    let data = world.data.clone();
    let ger = world.country("GER").unwrap();
    let eng = world.country("ENG").unwrap();
    hoi4_logic::diplomacy::force_declare_war(&mut world, ger, eng).unwrap();

    let raider = spawn::create_fleet(&mut world, eng, 2, "U-boats").unwrap();
    spawn::spawn_ship(&mut world, &data, raider, "submarine", "U-1").unwrap();
    world.fleets.mission[raider.0 as usize] = NavalMission::ConvoyRaiding;

    let unescorted = missions::convoy_route_risk(&world, &data, ger, &[2]);

    let escort = spawn::create_fleet(&mut world, ger, 2, "Escort").unwrap();
    spawn::spawn_ship(&mut world, &data, escort, "destroyer", "Z-1").unwrap();
    world.fleets.mission[escort.0 as usize] = NavalMission::ConvoyEscort;
    let escorted = missions::convoy_route_risk(&world, &data, ger, &[2]);

    assert!(unescorted.loss_rate > escorted.loss_rate);
    assert!(escorted.escort_presence > 0.0);
}

#[test]
fn daily_tick_repairs_and_builds_reserve_ship() {
    let mut world = world();
    let data = world.data.clone();
    let mut econ = EconomyState::new(&world);
    let ger = world.country("GER").unwrap();
    let fleet = spawn::create_fleet(&mut world, ger, 2, "Port Fleet").unwrap();
    world.fleets.home_port[fleet.0 as usize] = 1;
    let ship = spawn::spawn_ship(&mut world, &data, fleet, "destroyer", "Damaged").unwrap();
    world.ships.hp[ship.0 as usize] = 10.0;
    econ.stockpile[ger.0 as usize].insert("naval_vessel".to_owned(), 1.0);

    let report = missions::tick_daily(&mut world, &mut econ, &data);
    assert!(report.repaired_hp > 0.0);
    assert_eq!(report.ships_built, 1);
    assert!(world.ships.count >= 2);
}

#[test]
fn convoy_losses_consume_stockpile() {
    let mut world = world();
    let data = world.data.clone();
    let mut econ = EconomyState::new(&world);
    let ger = world.country("GER").unwrap();
    let eng = world.country("ENG").unwrap();
    hoi4_logic::diplomacy::force_declare_war(&mut world, eng, ger).unwrap();
    let raider = spawn::create_fleet(&mut world, eng, 2, "Raiders").unwrap();
    spawn::spawn_ship(&mut world, &data, raider, "submarine", "U-1").unwrap();
    world.fleets.mission[raider.0 as usize] = NavalMission::ConvoyRaiding;
    econ.stockpile[ger.0 as usize].insert("convoy".to_owned(), 100.0);

    let lost = missions::consume_convoys_for_route(&mut world, &mut econ, ger, &[2], 50.0);
    assert!(lost > 0.0);
    assert!(econ.stockpile[ger.0 as usize]["convoy"] < 100.0);
}
