use std::collections::HashMap;
use std::sync::Arc;

use hoi4_content::{inject_v6_into_world, V6Database};
use hoi4_data::{Color, Country, CountryTag, GameData, State};
use hoi4_map::{GameMap, Heightmap, ProvinceDefinition, ProvinceMap, ProvinceType, TerrainBitmap};
use hoi4_state::{StateIntegrationStatus, World};

fn test_map(max_province: u16) -> Arc<GameMap> {
    let mut definitions = vec![None; max_province as usize + 1];
    for province_id in 1..=max_province {
        definitions[province_id as usize] = Some(ProvinceDefinition {
            id: province_id,
            r: (province_id & 0xff) as u8,
            g: 0,
            b: 0,
            province_type: ProvinceType::Land,
            coastal: province_id % 3 == 0,
            terrain: "plains".to_owned(),
            continent: 1,
        });
    }
    Arc::new(GameMap {
        definitions,
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
        terrain_catalog: hoi4_map::TerrainCatalog::default(),
        tree_definition_bmp: None,
        tree_indices: std::collections::HashSet::new(),
    })
}

fn add_country(data: &mut GameData, tag_str: &str, capital: u16) {
    let tag = CountryTag::new(tag_str);
    data.countries.insert(
        tag.clone(),
        Country {
            tag,
            color: Color {
                r: 80,
                g: 80,
                b: 80,
            },
            graphical_culture: "western_european_gfx".to_owned(),
            capital,
            ruling_party: "neutrality".to_owned(),
            technologies: Vec::new(),
        },
    );
}

fn add_state(
    data: &mut GameData,
    owner: &str,
    state_id: u16,
    manpower: u64,
    infrastructure: u8,
    province_id: &mut u16,
) {
    let country = CountryTag::new(owner);
    data.states.push(State {
        id: state_id,
        name: format!("{owner} Phase5 State {state_id}"),
        manpower,
        owner: country.clone(),
        cores: vec![country],
        provinces: vec![*province_id],
        category: "metropolis".to_owned(),
        infrastructure,
        victory_points: vec![],
        resources: vec![],
    });
    *province_id += 1;
}

fn colonial_building_world() -> World {
    let mut data = GameData::default();
    add_country(&mut data, "ENG", 91);
    add_country(&mut data, "FRA", 105);
    add_country(&mut data, "MAL", 702);

    let mut province_id = 1u16;
    add_state(&mut data, "ENG", 91, 1_000_000, 4, &mut province_id);
    add_state(&mut data, "ENG", 92, 1_000_000, 4, &mut province_id);
    add_state(&mut data, "ENG", 702, 9_000_000, 9, &mut province_id);
    add_state(&mut data, "FRA", 105, 1_000_000, 4, &mut province_id);
    add_state(&mut data, "FRA", 106, 1_000_000, 4, &mut province_id);
    add_state(&mut data, "FRA", 303, 12_000_000, 9, &mut province_id);

    let mut world = World::new(test_map(province_id), Arc::new(data));
    let db = V6Database::load();
    inject_v6_into_world(&mut world, &db);
    world
}

fn modern_initial_building(id: &str) -> bool {
    matches!(
        id,
        "steel_mill"
            | "aluminium_plant"
            | "machinery_workshop"
            | "chemical_plant"
            | "electrical_works"
            | "machine_tool_works"
            | "engine_plant"
            | "rubber_factory"
            | "oil_refinery"
            | "textile_mill"
            | "furniture_factory"
            | "distillery"
            | "bank"
            | "university"
            | "telegraph_office"
            | "arms_industry"
            | "munition_plant"
            | "tank_factory"
            | "vehicle_factory"
            | "aircraft_factory"
            | "shipyard"
            | "synthetic_refinery"
            | "construction_sector"
    )
}

#[test]
fn industrial_buildings_prefer_metropole() {
    let world = colonial_building_world();
    let modern_buildings: Vec<_> = world
        .countries
        .buildings_v6
        .buildings
        .iter()
        .filter(|building| building.level > 0 && modern_initial_building(&building.building_def_id))
        .collect();

    assert!(
        modern_buildings.iter().any(|building| {
            let si = building.state.0 as usize;
            world.states.integration_status[si].is_domestic()
        }),
        "test setup should create modern buildings in domestic states"
    );

    for building in modern_buildings {
        let si = building.state.0 as usize;
        assert!(
            world.states.integration_status[si].is_domestic(),
            "{} should not be allocated to {:?} state {}",
            building.building_def_id,
            world.states.integration_status[si],
            world.states.names[si]
        );
    }
}

#[test]
fn colonial_resource_and_agriculture_buildings_remain_allowed() {
    let world = colonial_building_world();
    let colonial_rural_levels: u16 = world
        .countries
        .buildings_v6
        .buildings
        .iter()
        .filter(|building| {
            let si = building.state.0 as usize;
            world.states.integration_status[si] != StateIntegrationStatus::Metropole
                && matches!(
                    building.building_def_id.as_str(),
                    "grain_farm" | "livestock_ranch" | "rubber_plantation"
                )
        })
        .map(|building| building.level as u16)
        .sum();

    assert!(
        colonial_rural_levels > 0,
        "colonial agriculture/resource allocation should remain available"
    );
}
