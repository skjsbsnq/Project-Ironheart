//! J.4.8: Integration test — stockpile behavior via V6 tick.
//!
//! V6: Uses tick_daily_v6 instead of the removed vanilla tick_daily.
//! Factory counts are now building-based (placeholder 0 for now).

use std::sync::Arc;

use hoi4_content::V6Database;
use hoi4_data::GameData;
use hoi4_logic::economy::{self, EconomyState};
use hoi4_map::GameMap;
use hoi4_state::World;

fn hoi4_path() -> std::path::PathBuf {
    hoi4_paths::PathConfig::resolve(Default::default())
        .expect("set IRONHEART_HOI4_PATH or place HOI4 install at default Steam path")
        .game_path()
        .to_path_buf()
}

fn build_world() -> (World, Arc<GameData>) {
    let game_path = hoi4_path();
    let map = Arc::new(GameMap::load(&game_path).unwrap());
    let data = Arc::new(GameData::load(&game_path).unwrap());
    let mut world = World::new(map, data.clone());
    economy::init_world(&mut world);
    (world, data)
}

fn build_world_with_v6() -> (World, Arc<GameData>, V6Database) {
    let (mut world, data) = build_world();
    let db = V6Database::load();
    hoi4_content::inject_v6_into_world(&mut world, &db);
    (world, data, db)
}

#[test]
fn combat_deducts_stockpile() {
    let (mut world, data) = build_world();
    let mut econ = EconomyState::new(&world);

    let ger = world.country("GER").unwrap();

    // Give GER stockpile using archetype key (what SubunitDef.need references)
    econ.stockpile[ger.0 as usize].insert("infantry_equipment".to_string(), 10000.0);
    // Also add variant key (what production lines produce)
    econ.stockpile[ger.0 as usize].insert("infantry_equipment_1".to_string(), 10000.0);

    // Spawn a division if none exist
    let ger_div_idx = (0..world.divisions.count).find(|&i| world.divisions.owners[i] == ger);

    let div_idx = if let Some(idx) = ger_div_idx {
        idx
    } else {
        let tag = "GER".to_string();
        let templates = data.division_templates.get(&tag).expect("GER templates");
        let capital_state = world.countries.capitals[ger.0 as usize];
        let loc = world.states.provinces[capital_state.0 as usize][0];
        let div_id = hoi4_logic::military::spawn::spawn_from_template(
            &mut world,
            &data,
            ger,
            loc,
            0,
            "Test Div".to_string(),
        )
        .expect("spawn");
        div_id.0 as usize
    };

    let before = econ.stockpile_of(ger, "infantry_equipment")
        + econ.stockpile_of(ger, "infantry_equipment_1");
    hoi4_logic::economy::stockpile::deduct_combat_losses(
        &mut world, &mut econ, &data, div_idx, 0.05,
    );
    let after = econ.stockpile_of(ger, "infantry_equipment")
        + econ.stockpile_of(ger, "infantry_equipment_1");

    assert!(
        after < before,
        "Stockpile should decrease after combat losses (before={:.0}, after={:.0})",
        before,
        after
    );
    let loss = before - after;
    println!(
        "GER stockpile loss from 5% strength loss: {:.0} units",
        loss
    );
    assert!(loss > 0.0, "Loss should be positive");
}

#[test]
fn training_consumes_stockpile_and_manpower() {
    let (mut world, data) = build_world();
    let mut econ = EconomyState::new(&world);

    let ger = world.country("GER").unwrap();

    // Give GER stockpile using both archetype and variant keys
    econ.stockpile[ger.0 as usize].insert("infantry_equipment".to_string(), 50000.0);
    econ.stockpile[ger.0 as usize].insert("support_equipment".to_string(), 10000.0);
    econ.stockpile[ger.0 as usize].insert("artillery_equipment".to_string(), 5000.0);

    let mp_before = world.manpower(ger);
    let stock_before = econ.stockpile_of(ger, "infantry_equipment");

    // Spawn a division
    let tag = "GER".to_string();
    let templates = data.division_templates.get(&tag).unwrap();
    assert!(!templates.is_empty(), "GER should have templates");

    let capital_state = world.countries.capitals[ger.0 as usize];
    let loc = world.states.provinces[capital_state.0 as usize][0];
    let div_id = hoi4_logic::military::spawn::spawn_from_template(
        &mut world,
        &data,
        ger,
        loc,
        0,
        "Test Division".to_string(),
    )
    .expect("spawn should succeed");

    // Consume training resources
    let ratio = hoi4_logic::economy::stockpile::consume_training_resources(
        &mut world,
        &mut econ,
        &data,
        div_id.0 as usize,
    );

    let mp_after = world.manpower(ger);
    let stock_after = econ.stockpile_of(ger, "infantry_equipment");

    println!(
        "Training: ratio={:.2}, mp {}->{}, stock {:.0}->{:.0}",
        ratio, mp_before, mp_after, stock_before, stock_after
    );

    let consumed_something = mp_after < mp_before || stock_after < stock_before;
    assert!(
        consumed_something,
        "Training should consume stockpile or manpower"
    );
    assert!(
        ratio > 0.0,
        "Should have some equipment to train with (ratio={:.4})",
        ratio
    );
}

#[test]
fn division_strength_converges_to_equipment_ratio() {
    let (mut world, data) = build_world();
    let mut econ = EconomyState::new(&world);

    let ger = world.country("GER").unwrap();

    // Give GER plenty of stockpile
    econ.stockpile[ger.0 as usize].insert("infantry_equipment".to_string(), 100000.0);
    econ.stockpile[ger.0 as usize].insert("support_equipment".to_string(), 50000.0);
    econ.stockpile[ger.0 as usize].insert("artillery_equipment".to_string(), 50000.0);

    // Find a GER division and damage it
    let ger_div_idx = (0..world.divisions.count).find(|&i| world.divisions.owners[i] == ger);

    if let Some(idx) = ger_div_idx {
        // Damage the division to 50% strength
        world.divisions.strength[idx] = 0.50;
        let initial_str = world.divisions.strength[idx];

        // Run stockpile tick for 100 days — strength should recover
        for _ in 0..100 {
            hoi4_logic::economy::stockpile::tick(&mut world, &mut econ, &data);
        }
        let final_str = world.divisions.strength[idx];
        println!(
            "GER div strength: {:.3} -> {:.3} (with ample stockpile, should recover)",
            initial_str, final_str
        );
        assert!(
            final_str > initial_str,
            "Damaged division should recover strength with ample stockpile"
        );
    }
}

#[test]
fn v6_tick_produces_equipment() {
    let (mut world, _data, db) = build_world_with_v6();
    let mut econ = EconomyState::new(&world);

    let ger = world.country("GER").unwrap();
    world.player = ger;
    world.countries.at_war[ger.0 as usize] = true;

    // Run 30 days with V6 tick
    for day in 1..=30 {
        economy::tick_daily_v6(&mut world, &mut econ, &db, day);
    }

    // V6 tick should populate stockpile via building-based production.
    let output = econ.stockpile_of(ger, "infantry_equipment");
    println!(
        "GER 30-day infantry_equipment output via V6: {:.0} ({:.1}/day)",
        output,
        output / 30.0
    );
    assert!(
        output >= 45.0 * 30.0,
        "GER 1936 infantry equipment production is too low: {:.1}/day, expected at least 45/day",
        output / 30.0
    );
}

#[test]
fn v6_tick_produces_equipment_spain() {
    let (mut world, _data, db) = build_world_with_v6();
    let mut econ = EconomyState::new(&world);

    let spr = world.country("SPR").unwrap();
    world.player = spr;
    world.countries.at_war[spr.0 as usize] = true;

    for day in 1..=30 {
        economy::tick_daily_v6(&mut world, &mut econ, &db, day);
    }

    let output = econ.stockpile_of(spr, "infantry_equipment");
    println!(
        "SPR 30-day infantry_equipment output via V6: {:.0} ({:.1}/day)",
        output,
        output / 30.0
    );
    assert!(
        output > 0.0,
        "SPR 1936 infantry equipment production should be positive"
    );
}
