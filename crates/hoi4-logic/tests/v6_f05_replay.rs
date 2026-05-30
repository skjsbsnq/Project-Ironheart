//! F0.5: 1936-1938 GER V6 economy replay gate.

use std::sync::Arc;

use hoi4_content::V6Database;
use hoi4_data::GameData;
use hoi4_logic::economy::{EconomicSystemTick, EconomyState, MarketTick};
use hoi4_map::GameMap;
use hoi4_state::{LawCategory, PopClass, World};

fn hoi4_path() -> std::path::PathBuf {
    hoi4_paths::PathConfig::resolve(Default::default())
        .expect("set IRONHEART_HOI4_PATH or place HOI4 install at default Steam path")
        .game_path()
        .to_path_buf()
}

fn build_ger_replay_world() -> (World, V6Database) {
    let game_path = hoi4_path();
    let map = Arc::new(GameMap::load(&game_path).unwrap());
    let data = Arc::new(GameData::load(&game_path).unwrap());
    let mut world = World::new(map, data);
    hoi4_logic::economy::init_world(&mut world);
    world.populate_from_history();

    let db = V6Database::load();
    db.assert_no_vanilla_conflict();
    hoi4_content::inject_v6_into_world(&mut world, &db);

    let ger = world.country("GER").expect("GER exists");
    let ci = ger.0 as usize;
    world.countries.law_store.law_sets[ci].0[LawCategory::Conscription.index()].current =
        "limited_conscription".to_owned();
    assert_eq!(
        world.countries.law_store.law_sets[ci].0[LawCategory::Economy.index()].current,
        "corporatist_war_economy",
        "GER should start in corporatist war economy for F0.5 replay",
    );

    (world, db)
}

fn average_civilian_satisfaction(world: &World, _ci: usize) -> f32 {
    let mut weighted = 0.0_f64;
    let mut total = 0_u64;
    for pg in &world.countries.pops.groups {
        if pg.class == PopClass::Soldier {
            continue;
        }
        weighted += pg.satisfaction as f64 * pg.size as f64;
        total += pg.size as u64;
    }

    if total == 0 {
        0.0
    } else {
        (weighted / total as f64) as f32
    }
}

fn civilian_pop_count(world: &World, _ci: usize) -> u64 {
    world
        .countries
        .pops
        .groups
        .iter()
        .filter(|pg| pg.class != PopClass::Soldier)
        .map(|pg| pg.size as u64)
        .sum()
}

fn replay(world: &mut World, db: &V6Database, days: i64) -> (f64, f64) {
    let mut econ = EconomyState::new(world);
    let ger = world.country("GER").expect("GER exists");
    let ci = ger.0 as usize;
    let mut income_sum = 0.0_f64;
    let mut expense_sum = 0.0_f64;

    for day in 1..=days {
        MarketTick::tick_daily(world, &mut econ, db, ci, day);
        let treasury = &world.countries.treasury.treasuries[ci];
        income_sum += treasury.daily_income_rm;
        expense_sum += treasury.daily_expense_rm;
    }

    (income_sum, expense_sum)
}

#[test]
fn ger_1936_1938_replay_hits_f05_gate() {
    let (mut world, db) = build_ger_replay_world();
    let ger = world.country("GER").unwrap();
    let ci = ger.0 as usize;

    let (income_sum, expense_sum) = replay(&mut world, &db, 730);
    let treasury = &world.countries.treasury.treasuries[ci];
    let mefo_ratio = if treasury.gdp_rm > 0.0 {
        treasury.mefo_debt_rm / treasury.gdp_rm
    } else {
        0.0
    };
    let satisfaction = average_civilian_satisfaction(&world, ci);
    let pop_count = civilian_pop_count(&world, ci);
    let fiscal_ratio = income_sum / expense_sum.max(1.0);
    println!(
        "730d GER: MEFO/GDP={:.3}, satisfaction={:.3}, civilian_pop={}, income/expense={:.3}, MEFO={:.0}, GDP={:.0}",
        mefo_ratio,
        satisfaction,
        pop_count,
        fiscal_ratio,
        treasury.mefo_debt_rm,
        treasury.gdp_rm,
    );

    assert!(
        (0.25..=0.30).contains(&mefo_ratio),
        "730d MEFO/GDP = {:.3}, expected 0.25..=0.30 (MEFO RM {:.0}, GDP RM {:.0})",
        mefo_ratio,
        treasury.mefo_debt_rm,
        treasury.gdp_rm,
    );
    assert!(
        satisfaction >= 0.45,
        "730d GER civilian satisfaction {:.3}, expected >= 0.45",
        satisfaction,
    );
    assert!(
        (0.85..=1.15).contains(&fiscal_ratio),
        "730d income/expense = {:.3}, expected 0.85..=1.15 (income {:.0}, expense {:.0})",
        fiscal_ratio,
        income_sum,
        expense_sum,
    );
}

#[test]
fn ger_1936_1939_replay_triggers_mefo_crisis() {
    let (mut world, db) = build_ger_replay_world();
    let ger = world.country("GER").unwrap();
    let ci = ger.0 as usize;

    replay(&mut world, &db, 1095);

    let treasury = &world.countries.treasury.treasuries[ci];
    assert!(
        treasury.mefo_debt_rm <= treasury.gdp_rm * 0.05,
        "1095d MEFO debt should be cleared or sharply reduced by crisis, got MEFO RM {:.0}, GDP RM {:.0}",
        treasury.mefo_debt_rm,
        treasury.gdp_rm,
    );
    assert!(
        treasury.public_debt_rm > 0.0,
        "1095d MEFO crisis should roll residual debt into public_debt_rm",
    );
}
