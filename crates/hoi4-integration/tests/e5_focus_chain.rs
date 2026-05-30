//! E.5 闭环验收：headless 跑通 GER 1936→1940 国策链。
//!
//! 验证：选 GER → 完成 Rhineland → Anschluss → Danzig or War → 不 panic。
//! 需要 HOI4 安装路径（IRONHEART_HOI4_PATH 环境变量）。

use hoi4_content::{daily_focus_tick, start_focus, FocusTree, GlobalFlags, TickResult};
use hoi4_state::{CountryId, GameSpeed, World};
use std::sync::Arc;

fn build_world() -> World {
    let game_path = hoi4_paths::PathConfig::resolve(Default::default())
        .expect("set IRONHEART_HOI4_PATH")
        .game_path()
        .to_path_buf();
    let map = Arc::new(hoi4_map::GameMap::load(&game_path).unwrap());
    let data = Arc::new(hoi4_data::GameData::load(&game_path).unwrap());
    let mut world = World::new(map, data);
    world.populate_from_history();
    world.speed = GameSpeed::Speed5;
    world
}

fn ger_id(world: &World) -> CountryId {
    *world.tag_to_country.get("GER").expect("GER not found")
}

/// Advance world by N days, ticking focus each day.
fn advance_days(
    world: &mut World,
    ger: CountryId,
    tree: &FocusTree,
    flags: &mut GlobalFlags,
    days: u32,
) {
    for _ in 0..days {
        // Advance 24 hours
        for _ in 0..24 {
            world.tick_hour();
        }
        daily_focus_tick(world, ger, tree, flags, 0.0);
    }
}

#[test]
#[ignore] // requires HOI4 install
fn e5_ger_focus_chain_1936_to_1940() {
    let mut world = build_world();
    let ger = ger_id(&world);
    let tree = FocusTree::from_ron(include_str!(
        "../../hoi4-content/content/GER_focus_tree.ron"
    ))
    .unwrap();
    let mut flags = GlobalFlags::default();

    // Start Rhineland (70 days)
    assert!(start_focus(&mut world, ger, &tree, "GER_rhineland", &flags));
    advance_days(&mut world, ger, &tree, &mut flags, 70);
    assert!(world.countries.completed_focuses[ger.0 as usize].contains("GER_rhineland"));

    // Start Four Year Plan (70 days)
    assert!(start_focus(
        &mut world,
        ger,
        &tree,
        "GER_four_year_plan",
        &flags
    ));
    advance_days(&mut world, ger, &tree, &mut flags, 70);
    assert!(world.countries.completed_focuses[ger.0 as usize].contains("GER_four_year_plan"));

    // Advance to mid-1937 for Anschluss date gate
    advance_days(&mut world, ger, &tree, &mut flags, 200);

    // Start Anschluss (70 days)
    assert!(start_focus(&mut world, ger, &tree, "GER_anschluss", &flags));
    advance_days(&mut world, ger, &tree, &mut flags, 70);
    assert!(world.countries.completed_focuses[ger.0 as usize].contains("GER_anschluss"));

    // Advance to 1939 for Danzig date gate
    advance_days(&mut world, ger, &tree, &mut flags, 500);

    // Demand Sudetenland → End of Czechoslovakia → Danzig or War
    assert!(start_focus(
        &mut world,
        ger,
        &tree,
        "GER_demand_sudetenland",
        &flags
    ));
    advance_days(&mut world, ger, &tree, &mut flags, 70);
    assert!(start_focus(
        &mut world,
        ger,
        &tree,
        "GER_end_of_czechoslovakia",
        &flags
    ));
    advance_days(&mut world, ger, &tree, &mut flags, 70);
    assert!(start_focus(
        &mut world,
        ger,
        &tree,
        "GER_danzig_or_war",
        &flags
    ));
    advance_days(&mut world, ger, &tree, &mut flags, 70);
    assert!(world.countries.completed_focuses[ger.0 as usize].contains("GER_danzig_or_war"));

    // Verify no panic, world date should be around 1940
    assert!(
        world.date.year >= 1939,
        "Expected year >= 1939, got {}",
        world.date.year
    );
    println!(
        "E.5 PASS: GER focus chain completed, date = {}.{}.{}",
        world.date.year, world.date.month, world.date.day
    );
}

fn ger_id(world: &World) -> CountryId {
    *world.tag_to_country.get("GER").expect("GER not found")
}

/// Advance world by N days, ticking focus each day.
fn advance_days(
    world: &mut World,
    ger: CountryId,
    tree: &FocusTree,
    flags: &mut GlobalFlags,
    days: u32,
) {
    for _ in 0..days {
        // Advance 24 hours
        for _ in 0..24 {
            world.tick_hour();
        }
        daily_focus_tick(world, ger, tree, flags);
    }
}

#[test]
#[ignore] // requires HOI4 install
fn e5_ger_focus_chain_1936_to_1940() {
    let mut world = build_world();
    let ger = ger_id(&world);
    let tree = FocusTree::from_ron(include_str!(
        "../../hoi4-content/content/GER_focus_tree.ron"
    ))
    .unwrap();
    let mut flags = GlobalFlags::default();

    // Start Rhineland (70 days)
    assert!(start_focus(&mut world, ger, &tree, "GER_rhineland"));
    advance_days(&mut world, ger, &tree, &mut flags, 70);
    assert!(world.countries.completed_focuses[ger.0 as usize].contains("GER_rhineland"));

    // Start Four Year Plan (70 days)
    assert!(start_focus(&mut world, ger, &tree, "GER_four_year_plan"));
    advance_days(&mut world, ger, &tree, &mut flags, 70);
    assert!(world.countries.completed_focuses[ger.0 as usize].contains("GER_four_year_plan"));

    // Advance to mid-1937 for Anschluss date gate
    advance_days(&mut world, ger, &tree, &mut flags, 200);

    // Start Anschluss (70 days)
    assert!(start_focus(&mut world, ger, &tree, "GER_anschluss"));
    advance_days(&mut world, ger, &tree, &mut flags, 70);
    assert!(world.countries.completed_focuses[ger.0 as usize].contains("GER_anschluss"));

    // Advance to 1939 for Danzig date gate
    advance_days(&mut world, ger, &tree, &mut flags, 500);

    // Demand Sudetenland → End of Czechoslovakia → Danzig or War
    assert!(start_focus(
        &mut world,
        ger,
        &tree,
        "GER_demand_sudetenland"
    ));
    advance_days(&mut world, ger, &tree, &mut flags, 70);
    assert!(start_focus(
        &mut world,
        ger,
        &tree,
        "GER_end_of_czechoslovakia"
    ));
    advance_days(&mut world, ger, &tree, &mut flags, 70);
    assert!(start_focus(&mut world, ger, &tree, "GER_danzig_or_war"));
    advance_days(&mut world, ger, &tree, &mut flags, 70);
    assert!(world.countries.completed_focuses[ger.0 as usize].contains("GER_danzig_or_war"));

    // Verify no panic, world date should be around 1940
    assert!(
        world.date.year >= 1939,
        "Expected year >= 1939, got {}",
        world.date.year
    );
    println!(
        "E.5 PASS: GER focus chain completed, date = {}.{}.{}",
        world.date.year, world.date.month, world.date.day
    );
}
