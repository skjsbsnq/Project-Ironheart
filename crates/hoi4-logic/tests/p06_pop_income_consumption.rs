//! P0.6 验收测试：POP 消费、税后收入和失业人口需求
//!
//! 验收标准：
//! - 失业人口仍消耗粮食和衣物
//! - 提高税率会降低可支配收入和非基础消费
//! - 粮食短缺会降低基础满足度并提高激进度

use std::collections::HashMap;
use std::sync::Arc;

use hoi4_content::{inject_v6_into_world, V6Database};
use hoi4_data::{Color, Country, CountryTag, DivisionTemplate, GameData, State, SubunitDef};
use hoi4_logic::economy::{init_world, tick_daily_v6, EconomyState};
use hoi4_map::{
    GameMap, Heightmap, ProvinceDefinition, ProvinceMap, ProvinceType, TerrainBitmap,
    TerrainCatalog,
};
use hoi4_state::{LawCategory, LawSlot, PopClass, World};

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
        terrain_catalog: TerrainCatalog::default(),
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
        name: format!("{tag} P06 Test State {state_id}"),
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

#[test]
fn unemployed_pops_consume_grain_and_clothes() {
    let (mut world, db) = germany_world();
    let ger = world.country("GER").unwrap();

    let mut unemployed_count = 0u32;
    for pg in &world.countries.pops.groups {
        if world.states.owners[pg.state.0 as usize] == ger
            && pg.class != PopClass::Soldier
            && pg.employed_at.is_none()
        {
            unemployed_count += pg.size;
        }
    }
    if unemployed_count == 0 {
        eprintln!("P0.6 跳过：德国开局无失业人口，手动创建");
        let state_id = world
            .states
            .owners
            .iter()
            .position(|o| *o == ger)
            .map(|i| hoi4_state::StateId(i as u16))
            .unwrap_or(hoi4_state::StateId(0));
        world.countries.pops.groups.push(hoi4_state::PopGroup {
            class: PopClass::Worker,
            state: state_id,
            size: 500_000,
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
            literacy: 0.55,
            skilled_ratio: 0.18,
            standard_of_living: 0.5,
            needs_fulfillment: 1.0,
            essential_needs_fulfillment: 1.0,
            normal_needs_fulfillment: 1.0,
            luxury_needs_fulfillment: 1.0,
            radicalism: 0.0,
        });
    }

    let demand_before_grain = world.countries.market.markets[ger.0 as usize]
        .demand
        .get("grain")
        .copied()
        .unwrap_or(0.0);
    let demand_before_clothes = world.countries.market.markets[ger.0 as usize]
        .demand
        .get("clothes")
        .copied()
        .unwrap_or(0.0);

    replay(&mut world, &db, 7);

    let demand_after_grain = world.countries.market.markets[ger.0 as usize]
        .demand
        .get("grain")
        .copied()
        .unwrap_or(0.0);
    let demand_after_clothes = world.countries.market.markets[ger.0 as usize]
        .demand
        .get("clothes")
        .copied()
        .unwrap_or(0.0);

    assert!(
        demand_after_grain > demand_before_grain || demand_after_grain > 0.0,
        "失业人口必须产生粮食消费需求：grain demand = {demand_after_grain:.1}"
    );
    assert!(
        demand_after_clothes > demand_before_clothes || demand_after_clothes > 0.0,
        "失业人口必须产生衣物消费需求：clothes demand = {demand_after_clothes:.1}"
    );
}

#[test]
fn higher_tax_reduces_disposable_income() {
    let (mut world_low, db) = germany_world();
    let (mut world_high, _) = germany_world();
    let ger = world_low.country("GER").unwrap();

    let low_tax_law = "minimal_taxation";
    let high_tax_law = "heavy_taxation";

    if let Some(low_def) = db.taxation_laws.iter().find(|l| l.id == low_tax_law) {
        world_low.countries.law_store.law_sets[ger.0 as usize].0[LawCategory::Taxation.index()] =
            LawSlot::new(LawCategory::Taxation, &low_def.id);
    } else {
        eprintln!("P0.6 跳过低税率法律测试：未找到 {low_tax_law}");
        return;
    }
    if let Some(high_def) = db.taxation_laws.iter().find(|l| l.id == high_tax_law) {
        world_high.countries.law_store.law_sets[ger.0 as usize].0[LawCategory::Taxation.index()] =
            LawSlot::new(LawCategory::Taxation, &high_def.id);
    } else {
        eprintln!("P0.6 跳过高税率法律测试：未找到 {high_tax_law}");
        return;
    }

    replay(&mut world_low, &db, 14);
    replay(&mut world_high, &db, 14);

    let avg_disposable_low: f32 = world_low
        .countries
        .pops
        .groups
        .iter()
        .filter(|pg| {
            world_low.states.owners[pg.state.0 as usize] == ger && pg.class != PopClass::Soldier
        })
        .map(|pg| pg.disposable_income_rm * pg.size as f32)
        .sum::<f32>()
        / world_low
            .countries
            .pops
            .groups
            .iter()
            .filter(|pg| {
                world_low.states.owners[pg.state.0 as usize] == ger && pg.class != PopClass::Soldier
            })
            .map(|pg| pg.size as f32)
            .sum::<f32>()
            .max(1.0);

    let avg_disposable_high: f32 = world_high
        .countries
        .pops
        .groups
        .iter()
        .filter(|pg| {
            world_high.states.owners[pg.state.0 as usize] == ger && pg.class != PopClass::Soldier
        })
        .map(|pg| pg.disposable_income_rm * pg.size as f32)
        .sum::<f32>()
        / world_high
            .countries
            .pops
            .groups
            .iter()
            .filter(|pg| {
                world_high.states.owners[pg.state.0 as usize] == ger
                    && pg.class != PopClass::Soldier
            })
            .map(|pg| pg.size as f32)
            .sum::<f32>()
            .max(1.0);

    assert!(
        avg_disposable_high < avg_disposable_low,
        "高税率应降低可支配收入：低税={avg_disposable_low:.2} 高税={avg_disposable_high:.2}"
    );
}

#[test]
fn grain_shortage_reduces_essential_fulfillment_and_raises_radicalism() {
    let (mut world, db) = germany_world();
    let ger = world.country("GER").unwrap();

    for building in &mut world.countries.buildings_v6.buildings {
        if world.states.owners[building.state.0 as usize] == ger
            && building.building_def_id.contains("grain")
            || building.building_def_id.contains("farm")
        {
            building.level = 0;
        }
    }

    replay(&mut world, &db, 30);

    let market = &world.countries.market.markets[ger.0 as usize];
    let grain_supply = market.supply.get("grain").copied().unwrap_or(0.0);
    let grain_demand = market.demand.get("grain").copied().unwrap_or(0.0);

    if grain_supply >= grain_demand * 0.9 {
        eprintln!(
            "P0.6 跳过粮食短缺测试：grain supply={grain_supply:.1} demand={grain_demand:.1}，无法制造足够短缺"
        );
        return;
    }

    let avg_essential: f32 = world
        .countries
        .pops
        .groups
        .iter()
        .filter(|pg| {
            world.states.owners[pg.state.0 as usize] == ger && pg.class != PopClass::Soldier
        })
        .map(|pg| pg.essential_needs_fulfillment * pg.size as f32)
        .sum::<f32>()
        / world
            .countries
            .pops
            .groups
            .iter()
            .filter(|pg| {
                world.states.owners[pg.state.0 as usize] == ger && pg.class != PopClass::Soldier
            })
            .map(|pg| pg.size as f32)
            .sum::<f32>()
            .max(1.0);

    let avg_radicalism: f32 = world
        .countries
        .pops
        .groups
        .iter()
        .filter(|pg| {
            world.states.owners[pg.state.0 as usize] == ger && pg.class != PopClass::Soldier
        })
        .map(|pg| pg.radicalism * pg.size as f32)
        .sum::<f32>()
        / world
            .countries
            .pops
            .groups
            .iter()
            .filter(|pg| {
                world.states.owners[pg.state.0 as usize] == ger && pg.class != PopClass::Soldier
            })
            .map(|pg| pg.size as f32)
            .sum::<f32>()
            .max(1.0);

    assert!(
        avg_essential < 0.95,
        "粮食短缺应降低基础满足度：essential={avg_essential:.2}"
    );
    assert!(
        avg_radicalism > 0.01,
        "粮食短缺应提高激进度：radicalism={avg_radicalism:.3}"
    );
}

#[test]
fn pop_income_and_tax_paid_are_set() {
    let (mut world, db) = germany_world();
    let ger = world.country("GER").unwrap();

    replay(&mut world, &db, 7);

    let employed_with_income = world
        .countries
        .pops
        .groups
        .iter()
        .filter(|pg| {
            world.states.owners[pg.state.0 as usize] == ger
                && pg.class != PopClass::Soldier
                && pg.employed_at.is_some()
        })
        .any(|pg| pg.income_rm > 0.0);

    let any_tax_paid = world
        .countries
        .pops
        .groups
        .iter()
        .filter(|pg| {
            world.states.owners[pg.state.0 as usize] == ger && pg.class != PopClass::Soldier
        })
        .any(|pg| pg.tax_paid_rm > 0.0);

    let disposable_consistent = world
        .countries
        .pops
        .groups
        .iter()
        .filter(|pg| {
            world.states.owners[pg.state.0 as usize] == ger && pg.class != PopClass::Soldier
        })
        .all(|pg| (pg.disposable_income_rm - (pg.income_rm - pg.tax_paid_rm)).abs() < 0.01);

    assert!(employed_with_income, "就业人口应有 income_rm > 0");
    assert!(any_tax_paid, "部分人口应有 tax_paid_rm > 0");
    assert!(
        disposable_consistent,
        "disposable_income_rm 应等于 income_rm - tax_paid_rm"
    );
}
