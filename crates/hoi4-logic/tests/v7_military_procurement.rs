use std::collections::HashMap;
use std::sync::Arc;

use hoi4_data::{Color, Country, CountryTag, DivisionTemplate, GameData, State, SubunitDef};
use hoi4_logic::economy::{finance_tick, EconomyState};
use hoi4_logic::military::collect_military_demand;
use hoi4_map::{GameMap, Heightmap, ProvinceMap, TerrainBitmap, TerrainCatalog};
use hoi4_state::{CountryId, ProvinceId, World};

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
    let tag = CountryTag::new("GER");
    data.countries.insert(
        tag.clone(),
        Country {
            tag: tag.clone(),
            color: Color {
                r: 80,
                g: 80,
                b: 80,
            },
            graphical_culture: "western_european_gfx".to_owned(),
            capital: 1,
            ruling_party: "fascism".to_owned(),
            technologies: vec![],
        },
    );
    data.states.push(State {
        id: 1,
        name: "Test State".to_owned(),
        manpower: 1_000_000,
        owner: tag,
        cores: vec![CountryTag::new("GER")],
        provinces: vec![],
        category: "city".to_owned(),
        infrastructure: 3,
        victory_points: vec![],
        resources: vec![],
    });

    let mut infantry_need = HashMap::new();
    infantry_need.insert("infantry_equipment".to_owned(), 100);
    data.subunits.insert(
        "infantry".to_owned(),
        SubunitDef {
            key: "infantry".to_owned(),
            manpower: 1_000,
            need: infantry_need,
            ..SubunitDef::default()
        },
    );
    data.division_templates.insert(
        "GER".to_owned(),
        vec![DivisionTemplate {
            name: "Infantry Division".to_owned(),
            country_tag: Some("GER".to_owned()),
            regiments: vec!["infantry".to_owned()],
            support: vec![],
            division_names_group: None,
        }],
    );
    Arc::new(data)
}

fn world_with_damaged_division() -> World {
    let mut world = World::new(test_map(), test_data());
    world.divisions.push(
        CountryId(0),
        ProvinceId(0),
        0,
        30.0,
        100.0,
        "Damaged Infantry".to_owned(),
    );
    world.divisions.strength[0] = 0.50;
    world.countries.treasury.treasuries[0].cash_rm = 10_000_000.0;
    world
}

#[test]
fn h7_military_system_outputs_demand() {
    let world = world_with_damaged_division();
    let data = world.data.clone();

    let demand = collect_military_demand(&world, data.as_ref(), CountryId(0));

    assert_eq!(demand.manpower_needed, 1_000);
    assert!(demand.equipment_needed["infantry_equipment"] > 0.0);
    assert!(demand.fuel_needed > 0.0);
    assert!(demand.maintenance_rm > 0.0);
    assert!(demand.training_goods.contains_key("fuel"));
}

#[test]
fn h7_procurement_creates_orders_and_budget_breakdown() {
    let mut world = world_with_damaged_division();
    let mut econ = EconomyState::new(&world);

    finance_tick::step_pay_military_upkeep(&mut world, &mut econ, 0);

    let treasury = &world.countries.treasury.treasuries[0];
    assert!(treasury.daily_budget.expense_military_wages_rm > 0.0);
    assert_eq!(treasury.daily_budget.expense_military_procurement_rm, 0.0);
    assert!(treasury.daily_budget.expense_military_maintenance_rm > 0.0);
    assert!(treasury.daily_budget.expense_military_training_rm > 0.0);
    assert!(
        econ.government_orders[0]
            .iter()
            .any(|order| order.equipment_category == "infantry_equipment"
                && order.daily_budget_rm > 0.0),
        "equipment shortfall should create a government procurement order"
    );
}

#[test]
fn h7_procurement_order_creation_does_not_charge_cash() {
    let mut world = world_with_damaged_division();
    let cash_before = world.countries.treasury.treasuries[0].cash_rm;
    let mut econ = EconomyState::new(&world);

    finance_tick::step_pay_military_upkeep(&mut world, &mut econ, 0);

    let treasury = &world.countries.treasury.treasuries[0];
    assert_eq!(treasury.daily_budget.expense_military_procurement_rm, 0.0);
    assert!(treasury.cash_rm < cash_before);
    assert!(econ.government_orders[0]
        .iter()
        .any(|order| order.equipment_category == "infantry_equipment"));
}

#[test]
fn h7_existing_stockpile_reduces_procurement_order() {
    let mut world = world_with_damaged_division();
    let mut low_stock = EconomyState::new(&world);
    let mut high_stock = EconomyState::new(&world);
    high_stock.stockpile[0].insert("infantry_equipment".to_owned(), 10_000.0);

    finance_tick::step_pay_military_upkeep(&mut world, &mut low_stock, 0);
    let low_budget = low_stock.government_orders[0]
        .iter()
        .find(|order| order.equipment_category == "infantry_equipment")
        .map(|order| order.daily_budget_rm)
        .unwrap_or(0.0);

    let mut world = world_with_damaged_division();
    finance_tick::step_pay_military_upkeep(&mut world, &mut high_stock, 0);
    let high_budget = high_stock.government_orders[0]
        .iter()
        .find(|order| order.equipment_category == "infantry_equipment")
        .map(|order| order.daily_budget_rm)
        .unwrap_or(0.0);

    assert!(low_budget > 0.0);
    assert_eq!(high_budget, 0.0);
}
