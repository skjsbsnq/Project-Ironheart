use std::collections::HashMap;
use std::sync::Arc;

use hoi4_content::{inject_v6_into_world, V6Database};
use hoi4_data::{Color, Country, CountryTag, DivisionTemplate, GameData, State, SubunitDef};
use hoi4_logic::trade::step_trade_matching;
use hoi4_map::{GameMap, Heightmap, ProvinceDefinition, ProvinceMap, ProvinceType, TerrainBitmap};
use hoi4_state::{AutonomyLevel, CountryId, PopClass, PopGroup, StateId, TradeRouteKind, World};

fn test_map(max_province: u16) -> Arc<GameMap> {
    let mut definitions = vec![None; max_province as usize + 1];
    for province_id in 1..=max_province {
        definitions[province_id as usize] = Some(ProvinceDefinition {
            id: province_id,
            r: (province_id & 0xff) as u8,
            g: ((province_id >> 8) & 0xff) as u8,
            b: 0,
            province_type: ProvinceType::Land,
            coastal: true,
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
        name: format!("{tag} P4 Test State"),
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

fn trade_world() -> (World, V6Database) {
    let mut data = GameData::default();
    let countries = [
        ("ENG", 126),
        ("CAN", 703),
        ("AST", 704),
        ("NZL", 705),
        ("SAF", 706),
        ("RAJ", 707),
        ("MAL", 702),
        ("USA", 195),
        ("GER", 28),
        ("ROM", 700),
        ("SWE", 701),
        ("JAP", 536),
        ("MAN", 710),
        ("MEN", 711),
    ];
    for (tag, capital) in countries {
        add_country(&mut data, tag, capital);
    }
    let mut province_id = 1u16;
    for (tag, state_id) in countries {
        add_state(&mut data, tag, state_id, &mut province_id);
    }

    let mut world = World::new(test_map(province_id), Arc::new(data));
    hoi4_logic::economy::init_world(&mut world);
    let db = V6Database::load();
    inject_v6_into_world(&mut world, &db);
    world.countries.trade.routes.clear();
    world.countries.trade.agreements.clear();
    for treasury in &mut world.countries.treasury.treasuries {
        treasury.reserve_gbp = 1_000_000_000.0;
    }
    for market in &mut world.countries.market.markets {
        market.imports.clear();
        market.exports.clear();
        market.supply.clear();
        market.demand.clear();
        market.price.clear();
        market.unmet_demand.clear();
    }
    (world, db)
}

fn set_shortage(world: &mut World, country: CountryId, good_id: &str, demand: f32, supply: f32) {
    let market = &mut world.countries.market.markets[country.0 as usize];
    market.demand.insert(good_id.to_owned(), demand);
    market.supply.insert(good_id.to_owned(), supply);
    market
        .unmet_demand
        .insert(good_id.to_owned(), (demand - supply).max(0.0));
    market.price.insert(good_id.to_owned(), 10.0);
}

fn set_surplus(world: &mut World, country: CountryId, good_id: &str, supply: f32) {
    let market = &mut world.countries.market.markets[country.0 as usize];
    market.supply.insert(good_id.to_owned(), supply);
    market.demand.insert(good_id.to_owned(), 0.0);
    market.price.insert(good_id.to_owned(), 8.0);
}

fn test_pop(class: PopClass, state: StateId, size: u32) -> PopGroup {
    PopGroup {
        class,
        state,
        size,
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
        literacy: class.baseline_literacy(),
        skilled_ratio: class.baseline_skilled_ratio(),
        standard_of_living: 0.5,
        needs_fulfillment: 1.0,
        essential_needs_fulfillment: 1.0,
        normal_needs_fulfillment: 1.0,
        luxury_needs_fulfillment: 1.0,
        radicalism: 0.0,
    }
}

#[test]
fn market_bloc_supply_has_priority_over_neutral_partner() {
    let (mut world, db) = trade_world();
    let eng = world.country("ENG").unwrap();
    let can = world.country("CAN").unwrap();
    let usa = world.country("USA").unwrap();
    set_shortage(&mut world, eng, "steel", 100.0, 0.0);
    set_surplus(&mut world, can, "steel", 20.0);
    set_surplus(&mut world, usa, "steel", 200.0);
    world.diplomacy.opinions.set(eng, usa, 200);

    step_trade_matching(&mut world, &db, eng.0 as usize);

    assert!(
        world.countries.market.markets[can.0 as usize]
            .exports
            .get("steel")
            .copied()
            .unwrap_or(0.0)
            > 0.0,
        "bloc member CAN should export steel to ENG before neutral USA"
    );
    assert_eq!(
        world.countries.market.markets[usa.0 as usize]
            .exports
            .get("steel")
            .copied()
            .unwrap_or(0.0),
        0.0
    );
}

#[test]
fn world_spot_market_is_limited_not_infinite() {
    let (mut world, db) = trade_world();
    let eng = world.country("ENG").unwrap();
    set_shortage(&mut world, eng, "oil", 1_000.0, 0.0);
    world
        .countries
        .market
        .world_spot
        .daily_supply_caps
        .insert("oil".to_owned(), 4.0);

    step_trade_matching(&mut world, &db, eng.0 as usize);

    let imports = world.countries.market.markets[eng.0 as usize]
        .imports
        .get("oil")
        .copied()
        .unwrap_or(0.0);
    assert!(imports > 0.0, "small spot import should be possible");
    assert!(
        imports <= 4.01,
        "spot import should be capped, got {imports}"
    );
    assert!(world.countries.trade.routes.iter().any(|route| {
        route.importer == eng
            && route.exporter == CountryId::NONE
            && route.good_id.as_deref() == Some("oil")
    }));
}

#[test]
fn puppet_resource_share_feeds_master_market_and_reduces_autonomy() {
    let (mut world, db) = trade_world();
    let eng = world.country("ENG").unwrap();
    let mal = world.country("MAL").unwrap();
    set_shortage(&mut world, eng, "rubber", 100.0, 0.0);
    set_surplus(&mut world, mal, "rubber", 40.0);
    let before_progress = world.diplomacy.autonomy.get(&mal).unwrap().progress;

    step_trade_matching(&mut world, &db, eng.0 as usize);

    let imports = world.countries.market.markets[eng.0 as usize]
        .imports
        .get("rubber")
        .copied()
        .unwrap_or(0.0);
    assert!(imports > 0.0, "ENG should receive MAL rubber");
    assert!(
        world.countries.market.markets[mal.0 as usize]
            .exports
            .get("rubber")
            .copied()
            .unwrap_or(0.0)
            > 0.0,
        "MAL should record rubber exports"
    );
    let after_progress = world.diplomacy.autonomy.get(&mal).unwrap().progress;
    assert!(
        after_progress < before_progress,
        "forced extraction should reduce subject autonomy progress"
    );
    assert!(world.countries.trade.routes.iter().any(|route| {
        route.importer == eng
            && route.exporter == mal
            && route.good_id.as_deref() == Some("rubber")
            && route.kind == TradeRouteKind::ImperialPreference
    }));
}

#[test]
fn forced_subject_extraction_raises_subject_radicalism() {
    let (mut world, db) = trade_world();
    let eng = world.country("ENG").unwrap();
    let mal = world.country("MAL").unwrap();
    let mal_state = world.country_state_ids(mal)[0];
    world
        .countries
        .pops
        .groups
        .push(test_pop(PopClass::Worker, mal_state, 100_000));
    set_shortage(&mut world, eng, "rubber", 100.0, 0.0);
    set_surplus(&mut world, mal, "rubber", 40.0);

    step_trade_matching(&mut world, &db, eng.0 as usize);

    let radicalism = world
        .countries
        .pops
        .groups
        .iter()
        .find(|pg| pg.state == mal_state && pg.class == PopClass::Worker)
        .unwrap()
        .radicalism;
    assert!(
        radicalism > 0.0,
        "forced extraction should raise subject radicalism"
    );
}

#[test]
fn high_resistance_reduces_subject_resource_extraction() {
    let (mut stable, db) = trade_world();
    let eng = stable.country("ENG").unwrap();
    let mal = stable.country("MAL").unwrap();
    set_shortage(&mut stable, eng, "rubber", 100.0, 0.0);
    set_surplus(&mut stable, mal, "rubber", 40.0);
    step_trade_matching(&mut stable, &db, eng.0 as usize);
    let stable_imports = stable.countries.market.markets[eng.0 as usize]
        .imports
        .get("rubber")
        .copied()
        .unwrap_or(0.0);

    let (mut unrest, db) = trade_world();
    let eng = unrest.country("ENG").unwrap();
    let mal = unrest.country("MAL").unwrap();
    for state in unrest.country_state_ids(mal) {
        unrest.states.resistance[state.0 as usize] = 80.0;
        unrest.states.compliance[state.0 as usize] = 0.0;
    }
    set_shortage(&mut unrest, eng, "rubber", 100.0, 0.0);
    set_surplus(&mut unrest, mal, "rubber", 40.0);
    step_trade_matching(&mut unrest, &db, eng.0 as usize);
    let unrest_imports = unrest.countries.market.markets[eng.0 as usize]
        .imports
        .get("rubber")
        .copied()
        .unwrap_or(0.0);

    assert!(stable_imports > 0.0);
    assert!(
        unrest_imports < stable_imports,
        "high resistance should reduce subject resource extraction: stable={stable_imports}, unrest={unrest_imports}"
    );
}

#[test]
fn dominion_priority_does_not_force_full_resource_extraction() {
    let (mut world, db) = trade_world();
    let eng = world.country("ENG").unwrap();
    let can = world.country("CAN").unwrap();
    assert_eq!(
        world.diplomacy.autonomy.get(&can).unwrap().level,
        AutonomyLevel::Dominion
    );
    set_shortage(&mut world, eng, "grain", 100.0, 0.0);
    set_surplus(&mut world, can, "grain", 40.0);

    step_trade_matching(&mut world, &db, eng.0 as usize);

    let exports = world.countries.market.markets[can.0 as usize]
        .exports
        .get("grain")
        .copied()
        .unwrap_or(0.0);
    assert!(exports > 0.0, "Dominion CAN should be a priority supplier");
    assert!(
        exports < 40.0,
        "Dominion priority should not forcibly drain all available surplus"
    );
}

#[test]
fn imperial_preference_routes_remain_blockadable_sea_lanes() {
    let (mut world, _db) = trade_world();
    let eng = world.country("ENG").unwrap();
    let mal = world.country("MAL").unwrap();
    let route_id = world.countries.trade.add_route_for_good(
        eng,
        mal,
        Some("rubber".to_owned()),
        TradeRouteKind::ImperialPreference,
        Some(hoi4_state::StateId(0)),
        18.0,
        true,
    );
    let route = world
        .countries
        .trade
        .routes
        .iter()
        .find(|route| route.id == route_id)
        .expect("newly added route should exist");
    assert!(route.kind.uses_sea_lanes());
}

#[test]
fn japan_subject_resource_share_enters_imperial_preference_route() {
    let (mut world, db) = trade_world();
    let jap = world.country("JAP").unwrap();
    let man = world.country("MAN").unwrap();
    assert_eq!(
        world.diplomacy.autonomy.get(&man).unwrap().level,
        AutonomyLevel::Puppet
    );
    set_shortage(&mut world, jap, "coal", 100.0, 0.0);
    set_surplus(&mut world, man, "coal", 40.0);
    let before_progress = world.diplomacy.autonomy.get(&man).unwrap().progress;

    step_trade_matching(&mut world, &db, jap.0 as usize);

    let imports = world.countries.market.markets[jap.0 as usize]
        .imports
        .get("coal")
        .copied()
        .unwrap_or(0.0);
    assert!(imports > 0.0, "JAP should receive MAN coal");
    assert!(
        imports <= 20.01,
        "Puppet resource extraction should respect master share cap, got {imports}"
    );
    assert!(world.countries.trade.routes.iter().any(|route| {
        route.importer == jap
            && route.exporter == man
            && route.good_id.as_deref() == Some("coal")
            && route.kind == TradeRouteKind::ImperialPreference
    }));
    let after_progress = world.diplomacy.autonomy.get(&man).unwrap().progress;
    assert!(after_progress < before_progress);
}
