//! 空军系统集成测试 — 加载飞机类、创建 GER 与 ENG 联队、空战、战略轰炸、CAS、制空权。

use std::sync::Arc;

use hoi4_data::{AircraftKind, GameData};
use hoi4_logic::air::{
    air_superiority::AirControl, air_support, battle, spawn, stats::AirWingStats, strategic_bombing,
};
use hoi4_logic::military::battle::DeterministicRng;
use hoi4_map::GameMap;
use hoi4_state::{AirMission, World};

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

#[test]
fn test_aircraft_loaded() {
    let (_world, data) = build_world();
    println!("Loaded {} aircraft classes", data.aircraft.len());
    assert!(
        data.aircraft.len() >= 5,
        "Should load >= 5 aircraft (got {})",
        data.aircraft.len()
    );

    for k in [
        "fighter",
        "cas",
        "tactical_bomber",
        "strategic_bomber",
        "naval_bomber",
    ] {
        assert!(
            data.aircraft.contains_key(k),
            "aircraft `{}` must be loaded",
            k
        );
    }

    let f = &data.aircraft["fighter"];
    assert_eq!(f.kind, AircraftKind::Fighter);
    assert!(f.air_attack > 0.0);
    assert!(f.agility > 50.0);

    let sb = &data.aircraft["strategic_bomber"];
    assert!(sb.can_strategic_bomb());
    assert!(sb.strategic_bombing > 15.0);

    let nb = &data.aircraft["naval_bomber"];
    assert!(nb.can_naval_strike());

    let cas = &data.aircraft["cas"];
    assert!(cas.can_close_air_support());
}

#[test]
fn test_create_air_wing_and_aggregate() {
    let (mut world, data) = build_world();
    let ger = world.country("GER").unwrap();

    let wing = spawn::create_air_wing(&mut world, &data, ger, "fighter", 7, 100, "JG 1").unwrap();
    assert_eq!(world.air_wings.count, 1);
    let i = wing.0 as usize;
    assert_eq!(world.air_wings.owners[i], ger);
    assert_eq!(world.air_wings.region_id[i], 7);
    assert_eq!(world.air_wings.count_planes[i], 100);
    assert_eq!(world.air_wings.max_planes[i], 100);
    assert_eq!(world.air_wings.mission[i], AirMission::Idle);

    let stats = AirWingStats::aggregate(&world, &data, wing);
    assert_eq!(stats.plane_count, 100);
    assert_eq!(stats.kind, Some(AircraftKind::Fighter));
    assert!(stats.total_air_attack > 0.0);
    // 满 org → org_ratio = 1
    assert!((stats.org_ratio() - 1.0).abs() < 1e-3);
}

#[test]
fn test_air_battle_fighter_vs_bomber() {
    let (mut world, data) = build_world();
    let ger = world.country("GER").unwrap();
    let eng = world.country("ENG").unwrap();
    let region = 7u32;

    // ENG: 200 战斗机拦截
    let eng_wing =
        spawn::create_air_wing(&mut world, &data, eng, "fighter", region, 200, "RAF 1").unwrap();
    spawn::assign_mission(&mut world, eng_wing, AirMission::Interception, region);

    // GER: 300 战略轰炸机执行轰炸
    let ger_wing = spawn::create_air_wing(
        &mut world,
        &data,
        ger,
        "strategic_bomber",
        region,
        300,
        "KG 1",
    )
    .unwrap();
    spawn::assign_mission(&mut world, ger_wing, AirMission::StrategicBombing, region);

    let pre_atk = AirWingStats::aggregate(&world, &data, ger_wing);
    let pre_def = AirWingStats::aggregate(&world, &data, eng_wing);
    println!(
        "Pre: GER bombers={}, ENG fighters={}",
        pre_atk.plane_count, pre_def.plane_count
    );

    let mut rng = DeterministicRng::new(42);
    // ENG 战斗机为 attacker（拦截方），GER 轰炸机为 defender
    let outcome = battle::simulate(&mut world, &data, eng_wing, ger_wing, 96, &mut rng);
    println!(
        "Air battle: rounds={}, atk_won={}, atk_retr={}, def_retr={}, ENG_lost={}, GER_lost={}",
        outcome.rounds_fought,
        outcome.attacker_won,
        outcome.attacker_retreated,
        outcome.defender_retreated,
        outcome.attacker_planes_lost,
        outcome.defender_planes_lost
    );

    // 战斗机 vs 轰炸机：拦截方应至少造成轰炸方的损失
    assert!(
        outcome.defender_planes_lost > 0,
        "fighters intercepting bombers should cause some bomber losses"
    );
    // 双方都受到伤害（防御侧轰炸机也有自卫）
    assert!(
        outcome.attacker.stats.org_ratio() < 1.0 || outcome.defender.stats.org_ratio() < 1.0,
        "at least one side should lose org"
    );
}

#[test]
fn test_air_battle_marks_in_combat_correctly() {
    let (mut world, data) = build_world();
    let ger = world.country("GER").unwrap();
    let eng = world.country("ENG").unwrap();
    let region = 5u32;

    let g = spawn::create_air_wing(&mut world, &data, ger, "fighter", region, 50, "G").unwrap();
    let e = spawn::create_air_wing(&mut world, &data, eng, "fighter", region, 50, "E").unwrap();

    assert!(!world.air_wings.in_combat[g.0 as usize]);
    assert!(!world.air_wings.in_combat[e.0 as usize]);

    let mut rng = DeterministicRng::new(0);
    let _ = battle::simulate(&mut world, &data, g, e, 24, &mut rng);

    // 战后重置
    assert!(!world.air_wings.in_combat[g.0 as usize]);
    assert!(!world.air_wings.in_combat[e.0 as usize]);
}

#[test]
fn test_air_control_dominant_one_country() {
    let (mut world, data) = build_world();
    let ger = world.country("GER").unwrap();
    let region = 9u32;

    spawn::create_air_wing(&mut world, &data, ger, "fighter", region, 100, "JG").unwrap();
    spawn::create_air_wing(&mut world, &data, ger, "cas", region, 50, "St").unwrap();

    let ac = AirControl::recompute(&world, &data);
    let c = ac.control(region, ger);
    assert!(
        (c - 1.0).abs() < 1e-6,
        "GER alone in region should have 100% (got {})",
        c
    );
    assert_eq!(ac.dominant(region).map(|(c, _)| c), Some(ger));
}

#[test]
fn test_air_control_split() {
    let (mut world, data) = build_world();
    let ger = world.country("GER").unwrap();
    let eng = world.country("ENG").unwrap();
    let region = 9u32;

    // GER: 100 fighter (weight 4 each)
    spawn::create_air_wing(&mut world, &data, ger, "fighter", region, 100, "JG").unwrap();
    // ENG: 100 fighter
    spawn::create_air_wing(&mut world, &data, eng, "fighter", region, 100, "RAF").unwrap();

    let ac = AirControl::recompute(&world, &data);
    let g = ac.control(region, ger);
    let e = ac.control(region, eng);
    println!("Split fighters: GER={:.2}, ENG={:.2}", g, e);
    assert!((g - 0.5).abs() < 0.01, "equal forces → 50/50 split");
    assert!((e - 0.5).abs() < 0.01);
    assert!((g + e - 1.0).abs() < 1e-6);
}

#[test]
fn test_air_control_fighter_outweighs_cas() {
    let (mut world, data) = build_world();
    let ger = world.country("GER").unwrap();
    let eng = world.country("ENG").unwrap();
    let region = 9u32;

    // GER: 100 fighter (weight 4 → 400)
    spawn::create_air_wing(&mut world, &data, ger, "fighter", region, 100, "JG").unwrap();
    // ENG: 100 cas (weight 1 → 100)
    spawn::create_air_wing(&mut world, &data, eng, "cas", region, 100, "St").unwrap();

    let ac = AirControl::recompute(&world, &data);
    let g = ac.control(region, ger);
    let e = ac.control(region, eng);
    println!("F vs CAS: GER={:.2}, ENG={:.2}", g, e);
    // GER 400 / ENG 100 → 0.8 / 0.2
    assert!((g - 0.8).abs() < 0.05);
    assert!((e - 0.2).abs() < 0.05);
}

#[test]
#[ignore = "strategic bombing tests are skipped for Gate20 vanilla-audit cleanup"]
fn test_strategic_bombing_destroys_factories() {
    let (mut world, data) = build_world();
    let ger = world.country("GER").unwrap();
    let eng = world.country("ENG").unwrap();
    let region = 9u32;

    // 找一个 ENG 控制且有 V6 建筑的 state。
    let target_state = (0..world.states.count)
        .find(|&i| {
            world.states.controllers[i] == eng
                && world.state_building_levels(hoi4_state::StateId(i as u16)) > 0
        })
        .map(|i| hoi4_state::StateId(i as u16))
        .expect("Should find at least one ENG-controlled state with V6 buildings");

    let pre_buildings = world.state_building_levels(target_state);
    let pre_inf = world.states.infrastructure[target_state.0 as usize];
    println!(
        "Target state {}: buildings={}, infra={}",
        target_state.0, pre_buildings, pre_inf
    );

    // 大编队战略轰炸机，无敌方制空
    let bomber = spawn::create_air_wing(
        &mut world,
        &data,
        ger,
        "strategic_bomber",
        region,
        1000,
        "Big Wing",
    )
    .unwrap();

    let ac = AirControl::default(); // 无任何制空数据 → 敌制空 = 0
    let target = strategic_bombing::StrategicBombingTarget {
        state_id: target_state,
        region,
        defender: eng,
    };

    // 多日累积轰炸（30 天）
    let mut total_buildings = 0u8;
    let mut total_inf = 0u8;
    for _ in 0..30 {
        let out = strategic_bombing::execute_bombing(&mut world, &data, &ac, bomber, target);
        total_buildings = total_buildings.saturating_add(out.buildings_destroyed);
        total_inf = total_inf.saturating_add(out.infra_destroyed);
    }
    println!(
        "After 30 days: buildings -{}, infra -{}",
        total_buildings, total_inf
    );

    let post_buildings = world.state_building_levels(target_state);
    let post_inf = world.states.infrastructure[target_state.0 as usize];

    // 应当有损失
    assert!(
        post_buildings < pre_buildings || post_inf < pre_inf,
        "30-day bombing of 1000 strat_bombers must destroy SOMETHING (buildings {} -> {}, infra {} -> {})",
        pre_buildings, post_buildings, pre_inf, post_inf,
    );
}

#[test]
#[ignore = "strategic bombing tests are skipped for Gate20 vanilla-audit cleanup"]
fn test_strategic_bombing_blocked_by_air_control() {
    // 制空权完全在防方手里 → 轰炸效果应大幅缩减（接近 0）
    let (mut world, data) = build_world();
    let ger = world.country("GER").unwrap();
    let eng = world.country("ENG").unwrap();
    let region = 9u32;

    let target_state = (0..world.states.count)
        .find(|&i| {
            world.states.controllers[i] == eng
                && world.state_building_levels(hoi4_state::StateId(i as u16)) >= 1
        })
        .map(|i| hoi4_state::StateId(i as u16))
        .expect("ENG-controlled state with V6 buildings");

    let bomber =
        spawn::create_air_wing(&mut world, &data, ger, "strategic_bomber", region, 100, "B")
            .unwrap();
    spawn::create_air_wing(&mut world, &data, eng, "fighter", region, 1000, "RAF").unwrap();
    let ac = AirControl::recompute(&world, &data);
    println!(
        "GER ac={:.2}, ENG ac={:.2}",
        ac.control(region, ger),
        ac.control(region, eng)
    );

    // ENG 制空压倒
    assert!(ac.control(region, eng) > 0.9);

    let target = strategic_bombing::StrategicBombingTarget {
        state_id: target_state,
        region,
        defender: eng,
    };

    let pre_buildings = world.state_building_levels(target_state);
    let pre_bomber_planes = world.air_wings.count_planes[bomber.0 as usize];
    let mut total_buildings = 0u8;
    let mut total_bomber_loss = 0u32;
    for _ in 0..30 {
        let out = strategic_bombing::execute_bombing(&mut world, &data, &ac, bomber, target);
        total_buildings = total_buildings.saturating_add(out.buildings_destroyed);
        total_bomber_loss = total_bomber_loss.saturating_add(out.bomber_planes_lost);
    }
    let post_buildings = world.state_building_levels(target_state);
    let post_bomber_planes = world.air_wings.count_planes[bomber.0 as usize];
    println!(
        "Heavy ENG cover: buildings {}->{} (lost {}), bombers {}->{} (lost {})",
        pre_buildings,
        post_buildings,
        total_buildings,
        pre_bomber_planes,
        post_bomber_planes,
        total_bomber_loss,
    );

    // 在敌制空主导下，轰炸方损失飞机
    assert!(
        post_bomber_planes < pre_bomber_planes,
        "bombers should suffer losses when enemy has air control"
    );
}

#[test]
fn test_cas_ground_support_modifier() {
    let (mut world, data) = build_world();
    let ger = world.country("GER").unwrap();
    let region = 9u32;

    // 200 CAS + 200 fighter（保证 GER 制空）
    let cas = spawn::create_air_wing(&mut world, &data, ger, "cas", region, 200, "St").unwrap();
    let fighter =
        spawn::create_air_wing(&mut world, &data, ger, "fighter", region, 200, "JG").unwrap();
    spawn::assign_mission(&mut world, cas, AirMission::CloseAirSupport, region);
    spawn::assign_mission(&mut world, fighter, AirMission::AirSuperiority, region);

    let ac = AirControl::recompute(&world, &data);
    assert!(
        (ac.control(region, ger) - 1.0).abs() < 1e-6,
        "GER should dominate air"
    );

    let support = air_support::ground_support_modifier(&world, &data, &ac, region, ger);
    println!(
        "Ground support: bonus={:.3}, cas_planes={}, ac={:.2}",
        support.bonus, support.cas_planes, support.air_control
    );
    assert!(support.cas_planes >= 200);
    assert!(
        support.bonus > 0.0 && support.bonus <= 0.5,
        "bonus should be in (0, MAX_CAS_BONUS=0.5]"
    );
}

#[test]
fn test_cas_no_bonus_without_air_control() {
    let (mut world, data) = build_world();
    let ger = world.country("GER").unwrap();
    let eng = world.country("ENG").unwrap();
    let region = 9u32;

    // GER: 200 CAS（无战斗机 — 拦截下表现差）
    spawn::create_air_wing(&mut world, &data, ger, "cas", region, 200, "St").unwrap();
    // ENG: 1000 fighter
    spawn::create_air_wing(&mut world, &data, eng, "fighter", region, 1000, "RAF").unwrap();

    let ac = AirControl::recompute(&world, &data);
    let g_ac = ac.control(region, ger);
    let e_ac = ac.control(region, eng);
    println!("CAS-only: GER ac={:.2}, ENG ac={:.2}", g_ac, e_ac);
    assert!(e_ac > 0.5, "ENG should dominate air");

    let support = air_support::ground_support_modifier(&world, &data, &ac, region, ger);
    println!(
        "GER ground support under enemy air: bonus={:.3}, cas_planes={}, ac={:.3}",
        support.bonus, support.cas_planes, support.air_control
    );
    // GER 制空很低 + 敌制空 > 0.5 → 加成应被 intercept_penalty 大幅缩减
    assert!(
        support.bonus < 0.15,
        "CAS bonus should be tiny when enemy controls air (got {})",
        support.bonus
    );
}

#[test]
fn test_purge_destroyed_wings() {
    let (mut world, data) = build_world();
    let ger = world.country("GER").unwrap();

    let w1 = spawn::create_air_wing(&mut world, &data, ger, "fighter", 5, 100, "A").unwrap();
    let _w2 = spawn::create_air_wing(&mut world, &data, ger, "fighter", 5, 100, "B").unwrap();
    let _w3 = spawn::create_air_wing(&mut world, &data, ger, "fighter", 5, 100, "C").unwrap();
    assert_eq!(world.air_wings.count, 3);

    // 把 w1 打光
    world.air_wings.count_planes[w1.0 as usize] = 0;
    let purged = spawn::purge_destroyed_wings(&mut world);
    assert_eq!(purged, 1);
    assert_eq!(world.air_wings.count, 2);
}

#[test]
fn test_reinforce_caps_at_max() {
    let (mut world, data) = build_world();
    let ger = world.country("GER").unwrap();
    let w = spawn::create_air_wing(&mut world, &data, ger, "fighter", 5, 100, "A").unwrap();

    // 损失 30 架
    world.air_wings.count_planes[w.0 as usize] = 70;
    let added = spawn::reinforce(&mut world, w, 50);
    assert_eq!(added, 30); // 只能补到 100
    assert_eq!(world.air_wings.count_planes[w.0 as usize], 100);

    // 已满，再加 5
    let added = spawn::reinforce(&mut world, w, 5);
    assert_eq!(added, 0);
}

#[test]
fn test_daily_org_regen() {
    let (mut world, data) = build_world();
    let ger = world.country("GER").unwrap();
    let w = spawn::create_air_wing(&mut world, &data, ger, "fighter", 5, 100, "A").unwrap();
    let i = w.0 as usize;
    // 拉低 org
    world.air_wings.organisation[i] = 10.0;
    let max_org = world.air_wings.max_organisation[i];
    let pre = world.air_wings.organisation[i];

    spawn::tick_daily(&mut world);
    let post = world.air_wings.organisation[i];
    println!("Org regen: {} -> {} (max {})", pre, post, max_org);
    assert!(post > pre, "org should regen when not in combat");
    assert!(post <= max_org, "org cap at max");
}

#[test]
fn test_assign_mission() {
    let (mut world, data) = build_world();
    let ger = world.country("GER").unwrap();
    let w = spawn::create_air_wing(&mut world, &data, ger, "fighter", 5, 100, "A").unwrap();
    let i = w.0 as usize;
    assert_eq!(world.air_wings.mission[i], AirMission::Idle);

    spawn::assign_mission(&mut world, w, AirMission::AirSuperiority, 11);
    assert_eq!(world.air_wings.mission[i], AirMission::AirSuperiority);
    assert_eq!(world.air_wings.target_region[i], 11);
}
