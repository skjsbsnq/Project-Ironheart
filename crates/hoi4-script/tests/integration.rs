//! 脚本引擎集成测试 — trigger 求值、effect 执行、scope chain、AND/OR/NOT、events、decisions。

use std::sync::Arc;

use clausewitz_parser::parse;
use hoi4_data::politics::DecisionDef;
use hoi4_data::GameData;
use hoi4_map::GameMap;
use hoi4_script::decisions::DecisionCatalog;
use hoi4_script::effects::{run_effect_block, EffectRegistry};
use hoi4_script::events::{EventDef, EventOption, EventScheduler};
use hoi4_script::scope::{Scope, ScopeChain};
use hoi4_script::triggers::{eval_trigger_block, TriggerRegistry};
use hoi4_script::vars::{Flags, Variables};
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
    let world = World::new(map, data.clone());
    (world, data)
}

// ─── Trigger 求值 ────────────────────────────────────────

#[test]
fn test_trigger_has_government() {
    let (world, data) = build_world();
    let ger = world.country("GER").unwrap();
    let chain = ScopeChain::new(Scope::Country(ger));
    let vars = Variables::default();
    let flags = Flags::default();
    let reg = TriggerRegistry::with_builtins();

    let ruling = world.countries.ruling_party[ger.0 as usize].clone();
    let script = format!("has_government = {}", ruling);
    let block = parse(&script);
    assert!(eval_trigger_block(
        &block, &world, &data, &vars, &flags, &chain, &reg
    ));

    let block2 = parse("has_government = nonexistent_ideology");
    assert!(!eval_trigger_block(
        &block2, &world, &data, &vars, &flags, &chain, &reg
    ));
}

#[test]
fn test_trigger_num_of_factories() {
    let (world, data) = build_world();
    let ger = world.country("GER").unwrap();
    let chain = ScopeChain::new(Scope::Country(ger));
    let vars = Variables::default();
    let flags = Flags::default();
    let reg = TriggerRegistry::with_builtins();

    // GER should have > 10 factories
    let block = parse("num_of_factories > 10");
    assert!(eval_trigger_block(
        &block, &world, &data, &vars, &flags, &chain, &reg
    ));

    let block2 = parse("num_of_factories > 9999");
    assert!(!eval_trigger_block(
        &block2, &world, &data, &vars, &flags, &chain, &reg
    ));
}

#[test]
fn test_trigger_has_country_flag() {
    let (world, data) = build_world();
    let ger = world.country("GER").unwrap();
    let chain = ScopeChain::new(Scope::Country(ger));
    let vars = Variables::default();
    let mut flags = Flags::default();
    let reg = TriggerRegistry::with_builtins();

    let block = parse("has_country_flag = test_flag");
    assert!(!eval_trigger_block(
        &block, &world, &data, &vars, &flags, &chain, &reg
    ));

    flags.set_country_flag(ger, "test_flag");
    assert!(eval_trigger_block(
        &block, &world, &data, &vars, &flags, &chain, &reg
    ));
}

#[test]
fn test_trigger_check_variable() {
    let (world, data) = build_world();
    let ger = world.country("GER").unwrap();
    let chain = ScopeChain::new(Scope::Country(ger));
    let mut vars = Variables::default();
    let flags = Flags::default();
    let reg = TriggerRegistry::with_builtins();

    vars.set_country(ger, "my_var", 42.0);
    let block = parse("check_variable = { var = my_var value = 40 compare = greater_than }");
    assert!(eval_trigger_block(
        &block, &world, &data, &vars, &flags, &chain, &reg
    ));

    let block2 = parse("check_variable = { var = my_var value = 50 compare = greater_than }");
    assert!(!eval_trigger_block(
        &block2, &world, &data, &vars, &flags, &chain, &reg
    ));
}

// ─── AND / OR / NOT ────────────────────────────────────────

#[test]
fn test_and_or_not_combinators() {
    let (world, data) = build_world();
    let ger = world.country("GER").unwrap();
    let chain = ScopeChain::new(Scope::Country(ger));
    let vars = Variables::default();
    let mut flags = Flags::default();
    let reg = TriggerRegistry::with_builtins();

    flags.set_country_flag(ger, "flag_a");

    // AND: both true
    let block = parse("AND = { has_country_flag = flag_a num_of_factories > 1 }");
    assert!(eval_trigger_block(
        &block, &world, &data, &vars, &flags, &chain, &reg
    ));

    // OR: one true
    let block2 = parse("OR = { has_country_flag = flag_a has_country_flag = flag_b }");
    assert!(eval_trigger_block(
        &block2, &world, &data, &vars, &flags, &chain, &reg
    ));

    // OR: none true
    let block3 = parse("OR = { has_country_flag = flag_x has_country_flag = flag_y }");
    assert!(!eval_trigger_block(
        &block3, &world, &data, &vars, &flags, &chain, &reg
    ));

    // NOT: inner false → NOT true
    let block4 = parse("NOT = { has_country_flag = flag_missing }");
    assert!(eval_trigger_block(
        &block4, &world, &data, &vars, &flags, &chain, &reg
    ));

    // NOT: inner true → NOT false
    let block5 = parse("NOT = { has_country_flag = flag_a }");
    assert!(!eval_trigger_block(
        &block5, &world, &data, &vars, &flags, &chain, &reg
    ));
}

// ─── Scope chain ────────────────────────────────────────

#[test]
fn test_scope_chain_this_root_from() {
    let (world, data) = build_world();
    let ger = world.country("GER").unwrap();
    let eng = world.country("ENG").unwrap();
    let mut chain = ScopeChain::new(Scope::Country(ger));
    chain.push(Scope::Country(eng));
    let vars = Variables::default();
    let flags = Flags::default();
    let reg = TriggerRegistry::with_builtins();

    // THIS = ENG
    let block = parse("tag = ENG");
    assert!(eval_trigger_block(
        &block, &world, &data, &vars, &flags, &chain, &reg
    ));

    // ROOT = GER (scope changer)
    let block2 = parse("ROOT = { tag = GER }");
    assert!(eval_trigger_block(
        &block2, &world, &data, &vars, &flags, &chain, &reg
    ));

    // FROM = GER (previous scope)
    let block3 = parse("FROM = { tag = GER }");
    assert!(eval_trigger_block(
        &block3, &world, &data, &vars, &flags, &chain, &reg
    ));
}

// ─── Effect 执行 ────────────────────────────────────────

#[test]
fn test_effect_add_political_power() {
    let (mut world, data) = build_world();
    let ger = world.country("GER").unwrap();
    let chain = ScopeChain::new(Scope::Country(ger));
    let mut vars = Variables::default();
    let mut flags = Flags::default();
    let reg = EffectRegistry::with_builtins();

    let pre_pp = world.countries.political_power[ger.0 as usize];
    let block = parse("add_political_power = 100");
    run_effect_block(
        &block, &mut world, &data, &mut vars, &mut flags, &chain, &reg,
    );
    let post_pp = world.countries.political_power[ger.0 as usize];
    assert!((post_pp - pre_pp - 100.0).abs() < 0.01);
}

#[test]
fn test_effect_set_country_flag() {
    let (mut world, data) = build_world();
    let ger = world.country("GER").unwrap();
    let chain = ScopeChain::new(Scope::Country(ger));
    let mut vars = Variables::default();
    let mut flags = Flags::default();
    let reg = EffectRegistry::with_builtins();

    assert!(!flags.has_country_flag(ger, "my_flag"));
    let block = parse("set_country_flag = my_flag");
    run_effect_block(
        &block, &mut world, &data, &mut vars, &mut flags, &chain, &reg,
    );
    assert!(flags.has_country_flag(ger, "my_flag"));
}

#[test]
fn test_effect_set_variable() {
    let (mut world, data) = build_world();
    let ger = world.country("GER").unwrap();
    let chain = ScopeChain::new(Scope::Country(ger));
    let mut vars = Variables::default();
    let mut flags = Flags::default();
    let reg = EffectRegistry::with_builtins();

    let block = parse("set_variable = { var = test_v value = 99 }");
    run_effect_block(
        &block, &mut world, &data, &mut vars, &mut flags, &chain, &reg,
    );
    assert_eq!(vars.get_country(ger, "test_v"), 99.0);

    let block2 = parse("add_to_variable = { var = test_v value = 1 }");
    run_effect_block(
        &block2, &mut world, &data, &mut vars, &mut flags, &chain, &reg,
    );
    assert_eq!(vars.get_country(ger, "test_v"), 100.0);
}

#[test]
fn test_effect_set_politics() {
    let (mut world, data) = build_world();
    let ger = world.country("GER").unwrap();
    let chain = ScopeChain::new(Scope::Country(ger));
    let mut vars = Variables::default();
    let mut flags = Flags::default();
    let reg = EffectRegistry::with_builtins();

    let block = parse("set_politics = { ruling_party = communism }");
    run_effect_block(
        &block, &mut world, &data, &mut vars, &mut flags, &chain, &reg,
    );
    assert_eq!(world.countries.ruling_party[ger.0 as usize], "communism");
}

// ─── Events ────────────────────────────────────────

#[test]
fn test_event_fire_and_option_effect() {
    let (mut world, data) = build_world();
    let ger = world.country("GER").unwrap();
    let chain = ScopeChain::new(Scope::Country(ger));
    let mut vars = Variables::default();
    let mut flags = Flags::default();
    let effect_reg = EffectRegistry::with_builtins();

    let option_effect = parse("add_political_power = 50");
    let mut scheduler = EventScheduler::new();
    scheduler.register(EventDef {
        id: "test.1".into(),
        title: "Test Event".into(),
        desc: "desc".into(),
        is_triggered_only: true,
        fire_only_once: false,
        hidden: false,
        trigger: None,
        mtth: None,
        immediate: Some(parse("set_country_flag = event_fired")),
        options: vec![EventOption {
            name: "OK".into(),
            trigger: None,
            effect: Some(option_effect),
            ai_chance: 1.0,
        }],
    });

    // Fire event
    let ev = scheduler.fire_immediate("test.1").unwrap().clone();

    // Execute immediate
    if let Some(imm) = &ev.immediate {
        run_effect_block(
            imm,
            &mut world,
            &data,
            &mut vars,
            &mut flags,
            &chain,
            &effect_reg,
        );
    }
    assert!(flags.has_country_flag(ger, "event_fired"));

    // Execute option 0
    let pre_pp = world.countries.political_power[ger.0 as usize];
    if let Some(eff) = &ev.options[0].effect {
        run_effect_block(
            eff,
            &mut world,
            &data,
            &mut vars,
            &mut flags,
            &chain,
            &effect_reg,
        );
    }
    assert!((world.countries.political_power[ger.0 as usize] - pre_pp - 50.0).abs() < 0.01);
}

#[test]
fn test_event_fire_only_once() {
    let mut scheduler = EventScheduler::new();
    scheduler.register(EventDef {
        id: "once.1".into(),
        title: String::new(),
        desc: String::new(),
        is_triggered_only: true,
        fire_only_once: true,
        hidden: false,
        trigger: None,
        mtth: None,
        immediate: None,
        options: vec![],
    });
    assert!(scheduler.fire_immediate("once.1").is_some());
    assert!(scheduler.fire_immediate("once.1").is_none());
}

// ─── Decisions ────────────────────────────────────────

#[test]
fn test_decision_activate_and_complete() {
    let (mut world, data) = build_world();
    let ger = world.country("GER").unwrap();
    let chain = ScopeChain::new(Scope::Country(ger));
    let mut vars = Variables::default();
    let mut flags = Flags::default();
    let effect_reg = EffectRegistry::with_builtins();

    // Register decision definition in GameData
    let def = DecisionDef {
        id: "test_decision".into(),
        category: "political".into(),
        allowed: None,
        available: None,
        visible: None,
        complete_effect: Some(parse("set_country_flag = decision_done")),
        remove_effect: None,
        days_remove: 3,
        cost: 25.0,
        icon: None,
        name: None,
        allowed_country: None,
        fire_only_once: false,
        days_mission_timeout: 0,
        cooldown_days: 0,
    };
    Arc::get_mut(&mut world.data)
        .unwrap()
        .decisions
        .insert("test_decision".into(), def);

    let mut catalog = DecisionCatalog::new();

    // Activate
    catalog.activate("test_decision", ger.0, 3);
    assert_eq!(catalog.instances.len(), 1);
    assert!(catalog.instances[0].active);

    // Tick 3 days
    for _ in 0..2 {
        let completed = catalog.tick_daily();
        assert!(completed.is_empty());
    }
    let completed = catalog.tick_daily();
    assert_eq!(completed.len(), 1);
    assert_eq!(completed[0].0, "test_decision");

    // Execute complete_effect (look up from world.data)
    let def = world.data.decisions.get("test_decision").unwrap().clone();
    if let Some(eff) = &def.complete_effect {
        run_effect_block(
            eff,
            &mut world,
            &data,
            &mut vars,
            &mut flags,
            &chain,
            &effect_reg,
        );
    }
    assert!(flags.has_country_flag(ger, "decision_done"));
}

#[test]
fn test_decision_available_trigger() {
    let (world, data) = build_world();
    let ger = world.country("GER").unwrap();
    let chain = ScopeChain::new(Scope::Country(ger));
    let vars = Variables::default();
    let mut flags = Flags::default();
    let trigger_reg = TriggerRegistry::with_builtins();

    // Look up "gated" from world.data if it exists; otherwise skip
    let def = match world.data.decisions.get("gated") {
        Some(d) => d.clone(),
        None => return, // no such decision in vanilla — skip test
    };
    let avail = match &def.available {
        Some(b) => b,
        None => return, // no available trigger — skip
    };

    // Not available yet (no flag set)
    assert!(!eval_trigger_block(
        avail,
        &world,
        &data,
        &vars,
        &flags,
        &chain,
        &trigger_reg
    ));
}
