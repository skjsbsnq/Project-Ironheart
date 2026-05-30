use std::collections::HashMap;
use std::sync::Arc;

use hoi4_content::{inject_v6_into_world, V6Database};
use hoi4_data::{Color, Country, CountryTag, GameData, State};
use hoi4_map::{GameMap, Heightmap, ProvinceDefinition, ProvinceMap, ProvinceType, TerrainBitmap};
use hoi4_state::World;

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

fn add_state(data: &mut GameData, owner: &str, state_id: u16, province_id: &mut u16) {
    add_state_with_cores(data, owner, &[owner], state_id, province_id);
}

fn add_state_with_cores(
    data: &mut GameData,
    owner: &str,
    cores: &[&str],
    state_id: u16,
    province_id: &mut u16,
) {
    let country = CountryTag::new(owner);
    data.states.push(State {
        id: state_id,
        name: format!("{owner} Phase4 State {state_id}"),
        manpower: 2_000_000,
        owner: country.clone(),
        cores: cores.iter().map(|tag| CountryTag::new(tag)).collect(),
        provinces: vec![*province_id],
        category: "metropolis".to_owned(),
        infrastructure: 6,
        victory_points: vec![],
        resources: vec![],
    });
    *province_id += 1;
}

fn population_world() -> World {
    let mut data = GameData::default();
    add_country(&mut data, "ENG", 91);
    add_country(&mut data, "MAL", 702);
    add_country(&mut data, "RAJ", 304);
    add_country(&mut data, "FRA", 105);
    add_country(&mut data, "ITA", 212);
    add_country(&mut data, "LBA", 550);

    let mut province_id = 1u16;
    add_state(&mut data, "ENG", 91, &mut province_id);
    add_state(&mut data, "MAL", 702, &mut province_id);
    add_state(&mut data, "FRA", 105, &mut province_id);
    add_state(&mut data, "FRA", 303, &mut province_id);
    add_state(&mut data, "ITA", 212, &mut province_id);
    add_state_with_cores(&mut data, "ITA", &["LBA"], 550, &mut province_id);

    let mut world = World::new(test_map(province_id), Arc::new(data));
    let db = V6Database::load();
    inject_v6_into_world(&mut world, &db);
    world
}

#[test]
fn eng_domestic_vs_imperial_population() {
    let world = population_world();
    let eng = world.country("ENG").expect("ENG exists");
    let breakdown = world.country_population_breakdown(eng);

    assert!(
        breakdown.domestic > 0,
        "ENG should have domestic population"
    );
    assert_eq!(
        breakdown.colonial, 0,
        "ENG owns no colony state in this setup"
    );
    assert!(
        breakdown.subject > 0,
        "ENG should count MAL subject population"
    );
    assert_eq!(breakdown.governed, breakdown.domestic + breakdown.colonial);
    assert_eq!(breakdown.imperial, breakdown.governed + breakdown.subject);
    assert!(breakdown.imperial > breakdown.domestic);
}

#[test]
fn fra_colonial_population_not_domestic() {
    let world = population_world();
    let fra = world.country("FRA").expect("FRA exists");
    let breakdown = world.country_population_breakdown(fra);

    assert!(
        breakdown.domestic > 0,
        "FRA should have metropole population"
    );
    assert!(
        breakdown.colonial > 0,
        "FRA should have colonial/protectorate population"
    );
    assert_eq!(breakdown.governed, breakdown.domestic + breakdown.colonial);
    assert!(breakdown.governed > breakdown.domestic);
    assert_eq!(breakdown.subject, 0);
}

#[test]
fn non_core_owned_state_defaults_to_colonial_population() {
    let world = population_world();
    let ita = world.country("ITA").expect("ITA exists");
    let breakdown = world.country_population_breakdown(ita);

    assert!(
        breakdown.domestic > 0,
        "ITA should keep core metropole population domestic"
    );
    assert!(
        breakdown.colonial > 0,
        "owned non-core overseas territory should not be reported as metropole"
    );
    assert_eq!(breakdown.governed, breakdown.domestic + breakdown.colonial);
}
