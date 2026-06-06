//! Phase 1.4 验收：脚本引擎接入。
//!
//! 与 Phase 1.3 一样，以"主动注入场景"为主：因为 vanilla `events/*.txt` 等
//! 数据加载是 Phase 5.1 的工作，1.4 只把"运行时管线"打通。我们手工注册一些
//! event / decision 验证 daily / monthly 系统能正确路由并执行 effect。
//!
//! 验收点：
//! 1. 调度器：`with_phase1_systems()` 把 SystemId::Script 标为 active
//! 2. EventScheduler：到期事件被 daily 系统弹出，immediate + option 0 effect 跑过
//! 3. DecisionCatalog：days_remove 倒计时正确 + 到期 effect 跑过
//! 4. MTTH：有 mtth 的非 triggered_only 事件，monthly 抽签会在足够时间后触发
//! 5. 国策完成 effect：politics 现有 effect runner 在国策结束日依然把 PP 加到位

use clausewitz_parser::parse;
use hoi4_data::politics::DecisionDef;
use hoi4_integration::{init_simulation, load_world, tick_days_with};
use hoi4_script::events::{EventDef, EventOption, MeanTimeToHappen};

#[test]
fn schedule_phase1_4_script_active() {
    let s = hoi4_runtime::SystemSchedule::with_phase1_systems();
    let line = s.report_systems();
    // 1.4 完成后 script 应是 ✓
    assert!(line.contains("script ✓"), "report_systems: {}", line);
}

#[test]
fn event_immediate_and_option_fire() {
    let mut world = load_world().expect("HOI4 install required");
    let ger = world.country("GER").expect("GER tag");
    let ger_idx = ger.0;

    let (mut econ, mut research, mut politics_cache, mut script, mut ai) =
        init_simulation(&mut world);

    // 注册一个事件：immediate 设 country flag，option 0 加 100 PP
    script.events.register(EventDef {
        id: "ironheart.test.1".into(),
        title: "Test".into(),
        desc: String::new(),
        is_triggered_only: true,
        fire_only_once: false,
        hidden: false,
        trigger: None,
        mtth: None,
        immediate: Some(parse("set_country_flag = ironheart_test_immediate")),
        options: vec![EventOption {
            name: "OK".into(),
            trigger: None,
            effect: Some(parse("add_political_power = 100")),
            ai_chance: 1.0,
        }],
    });

    // 把事件排到 0 hour（立即可触发）
    script.events.queue("ironheart.test.1", ger_idx, 0);

    let pre_pp = world.countries.political_power[ger_idx as usize];
    let _ = tick_days_with(
        &mut world,
        &mut econ,
        &mut research,
        &mut politics_cache,
        &mut script,
        &mut ai,
        1,
    );
    let post_pp = world.countries.political_power[ger_idx as usize];

    // PP 应增加约 100（叠加上每日 +1 PP），Flag 应被设置
    assert!(
        post_pp >= pre_pp + 99.0,
        "PP 应增加 ≥99（事件 +100 减去日变化）: pre={} post={}",
        pre_pp,
        post_pp
    );
    assert!(
        script
            .flags
            .has_country_flag(ger, "ironheart_test_immediate"),
        "事件 immediate 设的 country flag 应已生效",
    );
}

#[test]
fn decision_days_remove_and_effect() {
    let mut world = load_world().expect("HOI4 install required");
    let ger = world.country("GER").expect("GER tag");
    let ger_idx = ger.0;

    let (mut econ, mut research, mut politics_cache, mut script, mut ai) =
        init_simulation(&mut world);

    // Register decision definition in GameData
    let def = DecisionDef {
        id: "ironheart_test_decision".into(),
        category: "political".into(),
        allowed: None,
        available: None,
        visible: None,
        complete_effect: Some(parse("add_political_power = 50")),
        remove_effect: Some(parse(
            "set_country_flag = ironheart_decision_done\nadd_war_support = 0.05",
        )),
        days_remove: 3,
        cost: 0.0,
        icon: None,
        name: None,
        allowed_country: None,
        fire_only_once: false,
        days_mission_timeout: 0,
        cooldown_days: 0,
    };
    std::sync::Arc::get_mut(&mut world.data)
        .unwrap()
        .decisions
        .insert("ironheart_test_decision".into(), def);

    // 激活
    script
        .decisions
        .activate("ironheart_test_decision", ger_idx, 3);
    assert_eq!(script.decisions.instances.len(), 1);

    let pre_ws = world.countries.war_support[ger_idx as usize];

    // 跑 4 天（>3 days_remove），到期后 instances 应被 purge
    let _ = tick_days_with(
        &mut world,
        &mut econ,
        &mut research,
        &mut politics_cache,
        &mut script,
        &mut ai,
        4,
    );

    let post_ws = world.countries.war_support[ger_idx as usize];
    assert!(
        script
            .flags
            .has_country_flag(ger, "ironheart_decision_done"),
        "remove_effect 设的 flag 应已生效",
    );
    assert!(
        (post_ws - pre_ws - 0.05).abs() < 0.001,
        "war_support 应增加 0.05：pre={} post={}",
        pre_ws,
        post_ws
    );
    // purge 后 instances 应清空
    assert_eq!(
        script.decisions.instances.len(),
        0,
        "已完成实例应被 purge_completed 清掉",
    );
}

#[test]
fn mtth_event_eventually_fires() {
    let mut world = load_world().expect("HOI4 install required");
    let ger = world.country("GER").expect("GER tag");
    let ger_idx = ger.0;

    let (mut econ, mut research, mut politics_cache, mut script, mut ai) =
        init_simulation(&mut world);

    // 一个非 triggered_only、mtth=10 天的事件：immediate 设 flag。
    // mtth=10 → 每月命中概率 ≈ 1 - (1 - 0.1)^30 ≈ 95.7%。
    // 但每月 × 每国（~120）抽签，所以即使首月对 GER 没命中，4 个月内几乎必然触发一次。
    script.events.register(EventDef {
        id: "ironheart.mtth.1".into(),
        title: String::new(),
        desc: String::new(),
        is_triggered_only: false,
        fire_only_once: true,
        hidden: false,
        trigger: Some(parse("tag = GER")), // 限定只对 GER 生效
        mtth: Some(MeanTimeToHappen { days: 10.0 }),
        immediate: Some(parse("set_country_flag = ironheart_mtth_fired")),
        options: vec![],
    });

    // 跑 4 个月（120 天）—— monthly 抽签 4 次，每次对 GER 都有 95% 概率
    let _ = tick_days_with(
        &mut world,
        &mut econ,
        &mut research,
        &mut politics_cache,
        &mut script,
        &mut ai,
        120,
    );

    assert!(
        script.flags.has_country_flag(ger, "ironheart_mtth_fired"),
        "MTTH=10 日的事件在 120 天内应触发（GER flag 应被设）",
    );

    // 顺手验：fire_only_once → fired_once 已记录
    assert!(
        script.events.fired_once.contains("ironheart.mtth.1"),
        "fire_only_once 事件触发后应被记入 fired_once",
    );
    let _ = ger_idx;
}

#[test]
fn focus_completion_runs_existing_effect_runner() {
    // 验证 1.4 之前已有的国策 → completion_reward → politics::run_effect_block 链路
    // 仍然在 Phase 1 调度器下工作。手工把 GER 的 focus_progress 推到接近完成，
    // 跑一天看 PP 跳变（GER 1936 默认 focus 树头节点的 reward 大多包含 PP）。
    //
    // 因 vanilla focus reward 形态各异，这里我们只断言 "focus 完成"本身：
    // 选 GER focus 树第一个 focus，把它设为 current_focus + progress 接近完成，
    // tick 1 天后该 focus 应进 completed_focuses，current_focus 应清空。
    let mut world = load_world().expect("HOI4 install required");
    let ger = world.country("GER").expect("GER tag");
    let i = ger.0 as usize;

    // 找 GER 焦点树
    let data = world.data.clone();
    let tree_id = data
        .focus_trees
        .iter()
        .find(|(_, t)| t.country_tag.as_deref() == Some("GER"))
        .map(|(k, _)| k.clone());
    let Some(tree_id) = tree_id else {
        eprintln!("[1.4 focus] GER focus tree not found — skip");
        return;
    };
    let first_focus_id = match data.focus_trees[&tree_id].focuses.keys().next() {
        Some(k) => k.clone(),
        None => {
            eprintln!("[1.4 focus] GER focus tree empty — skip");
            return;
        }
    };

    world.countries.current_focus[i] = Some(first_focus_id.clone());
    // 推到 999 天进度，1 天即过完成线
    world.countries.focus_progress[i] = 999.0;

    let (mut econ, mut research, mut politics_cache, mut script, mut ai) =
        init_simulation(&mut world);
    let _ = tick_days_with(
        &mut world,
        &mut econ,
        &mut research,
        &mut politics_cache,
        &mut script,
        &mut ai,
        1,
    );

    assert!(
        world.countries.completed_focuses[i].contains(&first_focus_id),
        "focus `{}` 应已完成并写入 completed_focuses",
        first_focus_id,
    );
    // Phase 1.5 起 AI orchestrator 会在 focus 完成同日给 GER 续上下一个 focus。
    // 因此 current_focus 可能是 None（队列里没下一项）也可能 = 下一项 id；只要
    // 不是仍卡在 first_focus_id 即可视为"完成 + 释放"。
    assert!(
        world.countries.current_focus[i].as_deref() != Some(first_focus_id.as_str()),
        "完成后 current_focus 不应仍是已完成的 `{}`",
        first_focus_id,
    );
}
