use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use hoi4_data::naval::baseline_for_class;
use hoi4_data::{Color, Country, CountryTag, GameData, State};
use hoi4_logic::economy::EconomyState;
use hoi4_logic::military::movement::{
    daily_occupation_tick, daily_transport_tick, is_division_in_transport, order_naval_invasion,
    order_overseas_transport, NavalInvasionError, TransportOrderError,
};
use hoi4_logic::naval::{missions, spawn};
use hoi4_map::{
    GameMap, Heightmap, ProvinceDefinition, ProvinceMap, ProvinceType, TerrainBitmap,
    TerrainCatalog,
};
use hoi4_state::{
    Building, BuildingKind, BuildingOwner, CountryId, NavalMission, ProvinceId, World,
};

fn test_map() -> Arc<GameMap> {
    let mut definitions = vec![None; 6];
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
        continent: 2,
    });
    definitions[5] = Some(ProvinceDefinition {
        id: 5,
        r: 5,
        g: 0,
        b: 0,
        province_type: ProvinceType::Land,
        coastal: false,
        terrain: "plains".to_owned(),
        continent: 2,
    });
    Arc::new(GameMap {
        definitions,
        rgb_to_id: HashMap::new(),
        province_map: ProvinceMap {
            width: 1,
            height: 1,
            pixels: vec![0],
        },
        adjacencies: vec![vec![], vec![2], vec![1, 3], vec![2, 4], vec![3, 5], vec![4]],
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
    let tag = CountryTag::new("JAP");
    data.countries.insert(
        tag.clone(),
        Country {
            tag: tag.clone(),
            color: Color {
                r: 200,
                g: 200,
                b: 200,
            },
            graphical_culture: "asian_gfx".to_owned(),
            capital: 1,
            ruling_party: "neutrality".to_owned(),
            technologies: vec![],
        },
    );
    let enemy_tag = CountryTag::new("CHI");
    data.countries.insert(
        enemy_tag.clone(),
        Country {
            tag: enemy_tag.clone(),
            color: Color {
                r: 80,
                g: 160,
                b: 80,
            },
            graphical_culture: "asian_gfx".to_owned(),
            capital: 2,
            ruling_party: "neutrality".to_owned(),
            technologies: vec![],
        },
    );
    data.states.push(State {
        id: 1,
        name: "Home Port".to_owned(),
        manpower: 100_000,
        owner: tag.clone(),
        cores: vec![tag.clone()],
        provinces: vec![1],
        category: "city".to_owned(),
        infrastructure: 3,
        victory_points: vec![],
        resources: vec![],
    });
    data.states.push(State {
        id: 2,
        name: "Overseas Port".to_owned(),
        manpower: 100_000,
        owner: tag.clone(),
        cores: vec![tag],
        provinces: vec![4, 5],
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

fn port_building(state: hoi4_state::StateId) -> Building {
    let mut building = Building::runtime_defaults();
    building.kind = BuildingKind::MilitaryBase;
    building.building_def_id = "v6_naval_base".to_owned();
    building.state = state;
    building.level = 1;
    building.owner = BuildingOwner::State;
    building
}

fn world() -> (World, EconomyState, CountryId) {
    let mut world = World::new(test_map(), test_data());
    let jap = world.country("JAP").unwrap();
    world
        .countries
        .buildings_v6
        .buildings
        .push(port_building(hoi4_state::StateId(0)));
    world
        .countries
        .buildings_v6
        .buildings
        .push(port_building(hoi4_state::StateId(1)));
    let mut econ = EconomyState::new(&world);
    econ.stockpile[jap.0 as usize].insert("convoy".to_owned(), 20.0);
    (world, econ, jap)
}

fn make_enemy_controller(world: &mut World, province: ProvinceId) -> CountryId {
    let jap = world.country("JAP").unwrap();
    let chi = world.country("CHI").unwrap();
    hoi4_logic::diplomacy::force_declare_war(world, jap, chi).unwrap();
    world.provinces.controllers[province.0 as usize] = chi;
    chi
}

#[test]
fn division_transports_between_friendly_ports() {
    let (mut world, mut econ, jap) = world();
    let div = world
        .divisions
        .push(jap, ProvinceId(1), 0, 50.0, 1000.0, "1st Division".into()) as usize;

    order_overseas_transport(&mut world, &econ, div, ProvinceId(4)).unwrap();
    assert!(is_division_in_transport(&world, div));

    for _ in 0..3 {
        let end = world.divisions.transport[div]
            .as_ref()
            .unwrap()
            .phase_ends_at_hour;
        world.elapsed_hours = end;
        daily_transport_tick(&mut world, &mut econ);
    }

    assert_eq!(world.divisions.locations[div], ProvinceId(4));
    assert!(!is_division_in_transport(&world, div));
}

#[test]
fn insufficient_convoys_rejects_transport_order() {
    let (mut world, mut econ, jap) = world();
    econ.stockpile[jap.0 as usize].insert("convoy".to_owned(), 0.0);
    let div = world
        .divisions
        .push(jap, ProvinceId(1), 0, 50.0, 1000.0, "1st Division".into()) as usize;

    let err = order_overseas_transport(&mut world, &econ, div, ProvinceId(4)).unwrap_err();
    assert_eq!(err, TransportOrderError::NoConvoyCapacity);
}

#[test]
fn transporting_division_does_not_flip_controller() {
    let (mut world, econ, jap) = world();
    let div = world
        .divisions
        .push(jap, ProvinceId(1), 0, 50.0, 1000.0, "1st Division".into()) as usize;
    order_overseas_transport(&mut world, &econ, div, ProvinceId(4)).unwrap();
    world.divisions.locations[div] = ProvinceId(4);
    world.provinces.controllers[4] = CountryId::NONE;

    daily_occupation_tick(&mut world);

    assert_eq!(world.provinces.controllers[4], CountryId::NONE);
}

#[test]
fn invasion_requires_preparation_and_support() {
    let (mut world, econ, jap) = world();
    make_enemy_controller(&mut world, ProvinceId(4));
    let div = world
        .divisions
        .push(jap, ProvinceId(1), 0, 50.0, 1000.0, "1st Division".into()) as usize;

    let err = order_naval_invasion(&mut world, &econ, div, ProvinceId(4), 24).unwrap_err();
    assert_eq!(err, NavalInvasionError::NotPrepared);

    let err = order_naval_invasion(&mut world, &econ, div, ProvinceId(4), 7 * 24).unwrap_err();
    assert!(matches!(
        err,
        NavalInvasionError::InsufficientNavalSupport { .. }
    ));
}

#[test]
fn naval_invasion_support_allows_landing_order() {
    let (mut world, econ, jap) = world();
    make_enemy_controller(&mut world, ProvinceId(4));
    let data = world.data.clone();
    let fleet = spawn::create_fleet(&mut world, jap, 0, "Landing Support").unwrap();
    spawn::spawn_ship(&mut world, &data, fleet, "destroyer", "Escort-1").unwrap();
    world.fleets.mission[fleet.0 as usize] = NavalMission::NavalInvasionSupport;

    let div = world
        .divisions
        .push(jap, ProvinceId(1), 0, 50.0, 1000.0, "1st Division".into()) as usize;

    order_naval_invasion(&mut world, &econ, div, ProvinceId(4), 7 * 24).unwrap();

    assert!(is_division_in_transport(&world, div));
    let transport = world.divisions.transport[div].as_ref().unwrap();
    assert_eq!(transport.destination_port, ProvinceId(4));
    assert_eq!(transport.route_regions, vec![0]);
}

#[test]
fn convoy_raiding_and_escort_affect_strategic_region_route() {
    let (mut world, _econ, jap) = world();
    let chi = make_enemy_controller(&mut world, ProvinceId(4));
    let data = world.data.clone();

    let raider = spawn::create_fleet(&mut world, chi, 0, "Raiders").unwrap();
    spawn::spawn_ship(&mut world, &data, raider, "submarine", "Sub-1").unwrap();
    world.fleets.mission[raider.0 as usize] = NavalMission::ConvoyRaiding;
    let raided = missions::convoy_route_risk(&world, &data, jap, &[0]);

    let escort = spawn::create_fleet(&mut world, jap, 0, "Escorts").unwrap();
    spawn::spawn_ship(&mut world, &data, escort, "destroyer", "Escort-1").unwrap();
    world.fleets.mission[escort.0 as usize] = NavalMission::ConvoyEscort;
    let escorted = missions::convoy_route_risk(&world, &data, jap, &[0]);

    assert!(raided.loss_rate > 0.0);
    assert!(escorted.loss_rate < raided.loss_rate);
}
