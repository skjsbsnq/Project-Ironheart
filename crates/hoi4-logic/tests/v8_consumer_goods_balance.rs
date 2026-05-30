use std::collections::HashMap;
use std::sync::Arc;

use hoi4_content::{inject_v6_into_world, V6Database};
use hoi4_data::{Color, Country, CountryTag, DivisionTemplate, GameData, State, SubunitDef};
use hoi4_logic::economy::{init_world, tick_daily_v6, EconomyState};
use hoi4_map::{GameMap, Heightmap, ProvinceDefinition, ProvinceMap, ProvinceType, TerrainBitmap};
use hoi4_state::{CountryId, LawCategory, LawSlot, World};

fn test_map(max_province: u16) -> Arc<GameMap> {
    let mut definitions = vec![None; max_province as usize + 1];
    for province_id in 1..=max_province {
        definitions[province_id as usize] = Some(ProvinceDefinition {
            id: province_id,
            r: (province_id & 0xff) as u8,
            g: ((province_id >> 8) & 0xff) as u8,
            b: 0,
            province_type: ProvinceType::Land,
            coastal: province_id % 2 == 0,
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
            ruling_party: "fascism".to_owned(),
            technologies: Vec::new(),
        },
    );
    let mut infantry_need = HashMap::new();
    infantry_need.insert("infantry_equipment".to_owned(), 100);
    data.subunits
        .entry("infantry".to_owned())
        .or_insert(SubunitDef {
            key: "infantry".to_owned(),
            manpower: 1_000,
            need: infantry_need,
            ..SubunitDef::default()
        });
    data.division_templates.insert(
        tag_str.to_owned(),
        vec![DivisionTemplate {
            name: "Infantry Division".to_owned(),
            country_tag: Some(tag_str.to_owned()),
            regiments: vec!["infantry".to_owned()],
            support: vec![],
            division_names_group: None,
        }],
    );
}

fn add_state(data: &mut GameData, tag: &str, state_id: u16, province_id: &mut u16) {
    let country = CountryTag::new(tag);
    data.states.push(State {
        id: state_id,
        name: format!("{tag} V8 Test State {state_id}"),
        manpower: 2_000_000,
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

fn germany_world() -> (World, V6Database) {
    let mut data = GameData::default();
    add_country(&mut data, "GER", 28);
    let mut province_id = 1u16;
    for state_id in [28, 51, 59, 64] {
        add_state(&mut data, "GER", state_id, &mut province_id);
    }

    let mut world = World::new(test_map(province_id), Arc::new(data));
    init_world(&mut world);
    let db = V6Database::load();
    inject_v6_into_world(&mut world, &db);
    (world, db)
}

fn replay(world: &mut World, db: &V6Database, days: i64) {
    let mut econ = EconomyState::new(world);
    for day in 1..=days {
        tick_daily_v6(world, &mut econ, db, day);
    }
}

fn shortage(world: &World, country: CountryId, good_id: &str) -> f32 {
    let market = &world.countries.market.markets[country.0 as usize];
    market
        .unmet_demand
        .get(good_id)
        .copied()
        .unwrap_or_else(|| {
            let demand = market.demand.get(good_id).copied().unwrap_or(0.0);
            let supply = market.supply.get(good_id).copied().unwrap_or(0.0);
            (demand - supply).max(0.0)
        })
}

#[test]
fn pop_need_table_loads_per_million_needs() {
    let db = V6Database::load();
    let worker_needs = db.pop_need_entries_for_class(hoi4_state::PopClass::Worker);

    assert!(worker_needs.iter().any(|need| need.good_id == "clothes"));
    assert!(worker_needs.iter().any(|need| need.good_id == "meat"));
    assert!(worker_needs
        .iter()
        .all(|need| need.amount_per_million > 0.0));
}

#[test]
fn germany_opening_clothes_and_meat_shortage_within_reasonable_band() {
    let (mut world, db) = germany_world();
    let ger = world.country("GER").unwrap();

    replay(&mut world, &db, 7);

    let clothes_shortage = shortage(&world, ger, "clothes");
    let meat_shortage = shortage(&world, ger, "meat");
    assert!(
        clothes_shortage < 1_000.0,
        "clothes shortage should not be thousands after 7 days, got {clothes_shortage:.1}"
    );
    assert!(
        meat_shortage < 1_000.0,
        "meat shortage should not be thousands after 7 days, got {meat_shortage:.1}"
    );
}

#[test]
fn market_tick_and_pop_need_table_use_same_scale() {
    let (mut world, db) = germany_world();
    let ger = world.country("GER").unwrap();
    replay(&mut world, &db, 1);

    let base_worker_clothes: f32 = world
        .countries
        .pops
        .groups
        .iter()
        .filter(|pop| {
            pop.class == hoi4_state::PopClass::Worker
                && world.states.owners[pop.state.0 as usize] == ger
        })
        .map(|pop| pop.size as f32 / 1_000_000.0)
        .sum::<f32>()
        * db.pop_need_entries_for_class(hoi4_state::PopClass::Worker)
            .iter()
            .find(|need| need.good_id == "clothes")
            .unwrap()
            .amount_per_million;
    let actual_clothes = world.countries.market.markets[ger.0 as usize]
        .demand
        .get("clothes")
        .copied()
        .unwrap_or(0.0);

    assert!(
        actual_clothes > 0.0,
        "clothes demand should be positive after tick, got {actual_clothes:.2}"
    );
    assert!(
        actual_clothes >= base_worker_clothes * 0.3,
        "clothes demand ({actual_clothes:.2}) should be at least 30% of base worker demand ({base_worker_clothes:.2})"
    );
}

#[test]
fn consumer_goods_factor_does_not_reduce_all_industrial_output() {
    let (mut baseline, db) = germany_world();
    let ger = baseline.country("GER").unwrap();
    let (mut war, _) = germany_world();
    baseline.countries.law_store.law_sets[ger.0 as usize].0[LawCategory::Economy.index()] =
        LawSlot::new(LawCategory::Economy, "interventionism");
    war.countries.law_store.law_sets[ger.0 as usize].0[LawCategory::Economy.index()] =
        LawSlot::new(LawCategory::Economy, "war_economy");

    replay(&mut baseline, &db, 1);
    replay(&mut war, &db, 1);

    let baseline_steel = baseline.countries.market.markets[ger.0 as usize]
        .supply
        .get("steel")
        .copied()
        .unwrap_or(0.0);
    let war_steel = war.countries.market.markets[ger.0 as usize]
        .supply
        .get("steel")
        .copied()
        .unwrap_or(0.0);

    assert!(baseline_steel > 0.0);
    assert!(
        war_steel >= baseline_steel * 0.95,
        "war economy should not globally cut steel output: baseline {baseline_steel:.1}, war {war_steel:.1}"
    );
}
