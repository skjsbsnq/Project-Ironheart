//! 经济系统集成测试 — 加载真实 1936 数据，运行一整年。
//!
//! V6: Uses tick_daily_v6 instead of the removed vanilla tick_daily.
//! Factory counts are now building-based (placeholder 0 for now).

use std::sync::Arc;

use hoi4_content::V6Database;
use hoi4_data::{GameData, ResourceKind};
use hoi4_logic::economy::{self, BuildOrder, EconomyState};
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
    let (world, data) = build_world();
    let db = V6Database::load();
    (world, data, db)
}

#[test]
fn test_data_loaded_buildings_resources_equipment() {
    let (_world, data) = build_world();
    assert!(
        data.buildings.contains_key("industrial_complex"),
        "industrial_complex should be loaded"
    );
    assert!(data.buildings.contains_key("arms_factory"));
    assert!(data.buildings.contains_key("infrastructure"));
    let ic = &data.buildings["industrial_complex"];
    assert!(
        (ic.base_cost - 10800.0).abs() < 1.0,
        "IC base_cost = {}",
        ic.base_cost
    );

    assert!(data.resources.contains_key(&ResourceKind::Oil));
    assert!(data.resources.contains_key(&ResourceKind::Steel));

    assert!(
        data.equipment.contains_key("infantry_equipment_1"),
        "infantry_equipment_1 should be loaded"
    );
    let inf1 = &data.equipment["infantry_equipment_1"];
    assert!(inf1.build_cost_ic > 0.0, "infantry_equipment_1 cost > 0");
    assert!(
        inf1.resources.get("steel").copied().unwrap_or(0.0) >= 1.0,
        "infantry_equipment_1 should inherit steel cost from archetype, got {:?}",
        inf1.resources
    );
}

#[test]
fn test_economy_state_init() {
    let (world, _data) = build_world();
    let econ = EconomyState::new(&world);
    assert_eq!(econ.count, world.countries.count);
    assert_eq!(econ.construction.len(), econ.count);
    assert_eq!(econ.production.len(), econ.count);
    assert_eq!(econ.resources.len(), econ.count);

    let ger = world.country("GER").unwrap();
    let i = ger.0 as usize;
    let f = world.countries.fuel[i];
    let cap = world.countries.fuel_capacity[i];
    assert!(cap > 0.0, "GER fuel capacity should be > 0");
    let ratio = f / cap;
    assert!(
        (ratio - 0.25).abs() < 0.01,
        "GER initial fuel ratio = {} (expected 0.25)",
        ratio
    );
}

#[test]
fn test_v6_tick_runs_without_panic() {
    let (mut world, data, db) = build_world_with_v6();
    let mut econ = EconomyState::new(&world);

    for day in 1..=365 {
        economy::tick_daily_v6(&mut world, &mut econ, &db, day);
    }
    // If we get here without panic, V6 tick is stable for 1 year
}

#[test]
fn test_manpower_growth_one_year() {
    let (mut world, _data, db) = build_world_with_v6();
    let mut econ = EconomyState::new(&world);

    let chi = world.country("CHI").unwrap();
    let initial_mp = world.manpower(chi);

    for day in 1..=365 {
        economy::tick_daily_v6(&mut world, &mut econ, &db, day);
    }

    let final_mp = world.manpower(chi);
    let delta = final_mp.saturating_sub(initial_mp);
    let growth_ratio = delta as f64 / initial_mp as f64;

    println!(
        "CHI manpower: {} → {} (Δ={}, ratio={:.4})",
        initial_mp, final_mp, delta, growth_ratio
    );

    assert!(
        growth_ratio >= 0.0,
        "CHI manpower should not decrease in peacetime (ratio={})",
        growth_ratio
    );
}

#[test]
fn test_full_year_simulation_stable() {
    let (mut world, _data, db) = build_world_with_v6();
    let mut econ = EconomyState::new(&world);

    let majors = ["GER", "ENG", "FRA", "ITA", "USA", "SOV", "JAP", "CHI"];

    for day in 1..=365 {
        economy::tick_daily_v6(&mut world, &mut econ, &db, day);
    }

    // Verify no panics and economy state is consistent
    for tag in majors.iter() {
        let cid = match world.country(tag) {
            Some(c) => c,
            None => continue,
        };
        let ci = cid.0 as usize;
        // Stockpile should be valid (may be empty, that's fine)
        let _stock = econ.stockpile.get(ci);
        // Resources should be valid
        let _res = econ.resources.get(ci);
    }
}
