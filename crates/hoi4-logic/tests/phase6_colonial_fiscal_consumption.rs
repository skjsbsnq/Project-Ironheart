use std::collections::HashMap;
use std::sync::Arc;

use hoi4_content::{inject_v6_into_world, V6Database};
use hoi4_data::{Color, Country, CountryTag, GameData, State};
use hoi4_logic::economy::{tick_daily_v6, EconomyState};
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

fn add_state(data: &mut GameData, owner: &str, state_id: u16, province_id: &mut u16) {
    let country = CountryTag::new(owner);
    data.states.push(State {
        id: state_id,
        name: format!("{owner} Phase6 State {state_id}"),
        manpower: 4_000_000,
        owner: country.clone(),
        cores: vec![country],
        provinces: vec![*province_id],
        category: "metropolis".to_owned(),
        infrastructure: 6,
        victory_points: vec![],
        resources: vec![],
    });
    *province_id += 1;
}

fn phase6_world(colonial: bool) -> (World, V6Database) {
    let mut data = GameData::default();
    add_country(&mut data, "FRA", 105);
    let mut province_id = 1u16;
    add_state(&mut data, "FRA", 105, &mut province_id);
    add_state(&mut data, "FRA", 303, &mut province_id);

    let mut world = World::new(test_map(province_id), Arc::new(data));
    let db = V6Database::load();
    inject_v6_into_world(&mut world, &db);
    if !colonial {
        for status in &mut world.states.integration_status {
            *status = StateIntegrationStatus::Metropole;
        }
    }
    (world, db)
}

fn run_week(world: &mut World, db: &V6Database) {
    let mut econ = EconomyState::new(world);
    for day in 1..=7 {
        tick_daily_v6(world, &mut econ, db, day);
    }
}

#[test]
fn colonial_population_does_not_scale_tax_and_consumption_like_domestic() {
    let (mut domestic_world, db) = phase6_world(false);
    let (mut colonial_world, _) = phase6_world(true);

    run_week(&mut domestic_world, &db);
    run_week(&mut colonial_world, &db);

    let fra_domestic = domestic_world.country("FRA").unwrap().0 as usize;
    let fra_colonial = colonial_world.country("FRA").unwrap().0 as usize;
    let domestic_taxes = domestic_world.countries.treasury.treasuries[fra_domestic]
        .daily_budget
        .income_taxes_rm;
    let colonial_taxes = colonial_world.countries.treasury.treasuries[fra_colonial]
        .daily_budget
        .income_taxes_rm;
    let domestic_demand: f32 = domestic_world.countries.market.markets[fra_domestic]
        .demand
        .values()
        .sum();
    let colonial_demand: f32 = colonial_world.countries.market.markets[fra_colonial]
        .demand
        .values()
        .sum();

    assert!(domestic_taxes > 0.0, "control setup should collect taxes");
    assert!(domestic_demand > 0.0, "control setup should create demand");
    assert!(
        colonial_taxes < domestic_taxes * 0.80,
        "colonial tax base should be reduced: colonial={colonial_taxes}, domestic={domestic_taxes}"
    );
    assert!(
        colonial_demand < domestic_demand * 0.90,
        "colonial consumption should not fully enter owner market: colonial={colonial_demand}, domestic={domestic_demand}"
    );
}

#[test]
fn gdp_statistics_split_domestic_colonial_and_extracted_value() {
    let (mut world, db) = phase6_world(true);
    run_week(&mut world, &db);

    let fra = world.country("FRA").unwrap().0 as usize;
    let treasury = &world.countries.treasury.treasuries[fra];

    assert!(treasury.gdp_rm > 0.0);
    assert!(treasury.domestic_gdp_rm > 0.0);
    assert!(treasury.colonial_gdp_rm > 0.0);
    assert!(treasury.colonial_extracted_value_rm > 0.0);
    assert!(treasury.colonial_extracted_value_rm < treasury.colonial_gdp_rm);
    assert!((treasury.domestic_gdp_rm + treasury.colonial_gdp_rm - treasury.gdp_rm).abs() < 1.0);
}
