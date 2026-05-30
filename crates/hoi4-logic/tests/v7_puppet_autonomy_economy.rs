use std::collections::HashMap;
use std::sync::Arc;

use hoi4_data::{Color, Country, CountryTag, GameData, State};
use hoi4_logic::diplomacy::puppet::{country_industry_score, set_puppet, tick_autonomy_daily};
use hoi4_map::{GameMap, Heightmap, ProvinceMap, TerrainBitmap, TerrainCatalog};
use hoi4_state::{AutonomyLevel, Building, BuildingKind, BuildingOwner, StateId, World};

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
    for tag in ["GER", "SLO"] {
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
                capital: if tag == "GER" { 1 } else { 2 },
                ruling_party: "neutrality".to_owned(),
                technologies: vec![],
            },
        );
    }
    data.states.push(State {
        id: 1,
        name: "Master State".to_owned(),
        manpower: 1_000_000,
        owner: CountryTag::new("GER"),
        cores: vec![CountryTag::new("GER")],
        provinces: vec![],
        category: "city".to_owned(),
        infrastructure: 3,
        victory_points: vec![],
        resources: vec![],
    });
    data.states.push(State {
        id: 2,
        name: "Subject State".to_owned(),
        manpower: 500_000,
        owner: CountryTag::new("SLO"),
        cores: vec![CountryTag::new("SLO")],
        provinces: vec![],
        category: "city".to_owned(),
        infrastructure: 3,
        victory_points: vec![],
        resources: vec![],
    });
    Arc::new(data)
}

fn add_building(world: &mut World, state: StateId, kind: BuildingKind, level: u8) {
    let mut building = Building::runtime_defaults();
    building.kind = kind;
    building.building_def_id = format!("{:?}", kind);
    building.state = state;
    building.level = level;
    building.owner = BuildingOwner::State;
    world.countries.buildings_v6.buildings.push(building);
}

#[test]
fn puppet_autonomy_uses_real_industry() {
    let mut world = World::new(test_map(), test_data());
    let master = world.tag_to_country["GER"];
    let subject = world.tag_to_country["SLO"];
    add_building(&mut world, StateId(0), BuildingKind::Industrial, 2);
    add_building(&mut world, StateId(1), BuildingKind::Military, 6);

    assert!(country_industry_score(&world, subject) > country_industry_score(&world, master));
    set_puppet(&mut world, master, subject, AutonomyLevel::Puppet).unwrap();
    tick_autonomy_daily(&mut world);

    let autonomy = world.diplomacy.autonomy.get(&subject).unwrap();
    assert!(
        autonomy.progress > 0.0,
        "subject industry should create autonomy progress"
    );
}
