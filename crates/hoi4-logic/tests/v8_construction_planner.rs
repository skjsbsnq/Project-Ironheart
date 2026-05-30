use std::collections::HashMap;
use std::sync::Arc;

use hoi4_content::{inject_v6_into_world, V6Database};
use hoi4_data::{Color, Country, CountryTag, DivisionTemplate, GameData, State, SubunitDef};
use hoi4_logic::economy::construction_planner::{
    plan_construction, target_queue_len_from_cp, ConstructionPlanningMode,
};
use hoi4_logic::economy::{init_world, EconomyState};
use hoi4_map::{GameMap, Heightmap, ProvinceDefinition, ProvinceMap, ProvinceType, TerrainBitmap};
use hoi4_state::World;

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
        name: format!("{tag} Planner Test State {state_id}"),
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

fn planner_world() -> (World, V6Database) {
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
    for slots in &mut world.states.category_slots {
        *slots = 100;
    }
    (world, db)
}

fn set_shortage(world: &mut World, country_idx: usize, good_id: &str, demand: f32, supply: f32) {
    let market = &mut world.countries.market.markets[country_idx];
    market.demand.insert(good_id.to_owned(), demand);
    market.supply.insert(good_id.to_owned(), supply);
    market
        .unmet_demand
        .insert(good_id.to_owned(), (demand - supply).max(0.0));
    market.price.insert(good_id.to_owned(), 1.5);
}

#[test]
fn planner_prioritizes_textiles_when_clothes_shortage_is_real() {
    let (mut world, db) = planner_world();
    let ger = world.country("GER").unwrap();
    set_shortage(&mut world, ger.0 as usize, "clothes", 100.0, 10.0);
    let econ = EconomyState::new(&world);

    let plan = plan_construction(
        &world,
        &econ,
        &db,
        ger,
        ConstructionPlanningMode::PlayerAutoBuild,
        75,
    );

    let textile = plan
        .candidates
        .iter()
        .find(|candidate| candidate.building_id == "textile_mill")
        .expect("textile candidate");
    assert!(textile
        .reasons
        .iter()
        .any(|reason| reason.contains("衣物短缺")));
    assert!(plan
        .candidates
        .iter()
        .take(8)
        .any(|candidate| candidate.building_id == "textile_mill"));
}

#[test]
fn planner_prioritizes_grain_before_livestock_when_meat_chain_lacks_feed() {
    let (mut world, db) = planner_world();
    let ger = world.country("GER").unwrap();
    set_shortage(&mut world, ger.0 as usize, "meat", 100.0, 15.0);
    set_shortage(&mut world, ger.0 as usize, "grain", 100.0, 20.0);
    let econ = EconomyState::new(&world);

    let plan = plan_construction(
        &world,
        &econ,
        &db,
        ger,
        ConstructionPlanningMode::PlayerAutoBuild,
        75,
    );
    let grain = plan
        .candidates
        .iter()
        .find(|candidate| candidate.building_id == "grain_farm")
        .expect("grain farm candidate");
    let livestock = plan
        .candidates
        .iter()
        .find(|candidate| candidate.building_id == "livestock_ranch")
        .expect("livestock candidate");

    assert!(grain.score > livestock.score);
    assert!(grain
        .reasons
        .iter()
        .any(|reason| reason.contains("先补粮食")));
}

#[test]
fn planner_explains_rejected_candidates() {
    let (mut world, db) = planner_world();
    let ger = world.country("GER").unwrap();
    for slots in &mut world.states.category_slots {
        *slots = 0;
    }
    let econ = EconomyState::new(&world);

    let plan = plan_construction(
        &world,
        &econ,
        &db,
        ger,
        ConstructionPlanningMode::PlayerAutoBuild,
        75,
    );

    assert!(plan.candidates.is_empty());
    assert!(plan
        .rejected
        .iter()
        .any(|candidate| candidate.reason.contains("建筑槽已满")));
}

#[test]
fn planner_is_testable_without_app_layer() {
    let (world, db) = planner_world();
    let ger = world.country("GER").unwrap();
    let econ = EconomyState::new(&world);

    let plan = plan_construction(
        &world,
        &econ,
        &db,
        ger,
        ConstructionPlanningMode::PlayerAutoBuild,
        75,
    );

    assert_eq!(target_queue_len_from_cp(75), 8);
    assert!(
        !plan.candidates.is_empty(),
        "no candidates; rejected examples: {:?}",
        plan.rejected.iter().take(5).collect::<Vec<_>>()
    );
    assert!(plan
        .candidates
        .iter()
        .all(|candidate| !candidate.reasons.is_empty()));
}
