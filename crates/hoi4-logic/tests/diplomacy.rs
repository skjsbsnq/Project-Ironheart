//! 外交系统集成测试 — 阵营、宣战、wargoal、和平、傀儡、世界紧张度。

use std::sync::Arc;

use hoi4_data::GameData;
use hoi4_logic::diplomacy::{
    evaluate_action, execute_action, factions, peace, puppet, tension, war, wargoal,
    DiplomacyError, DiplomaticAction, UnavailableReason,
};
use hoi4_map::GameMap;
use hoi4_state::{AutonomyLevel, CountryId, StateId, WarJoinPolicy, WarSide, WargoalType, World};

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

fn major(world: &World) -> (CountryId, CountryId, CountryId, CountryId) {
    let ger = world.country("GER").unwrap();
    let eng = world.country("ENG").unwrap();
    let fra = world.country("FRA").unwrap();
    let ita = world.country("ITA").unwrap();
    (ger, eng, fra, ita)
}

// ─── 阵营 ────────────────────────────────────────────────────

#[test]
fn test_create_and_join_faction() {
    let (mut world, _data) = build_world();
    let (ger, _eng, _fra, ita) = major(&world);

    let f = factions::create_faction(&mut world, ger, "Axis").unwrap();
    assert_eq!(world.diplomacy.factions.len(), 1);
    assert_eq!(world.diplomacy.factions[0].leader, ger);
    assert_eq!(world.diplomacy.faction_of(ger), Some(f));

    factions::join_faction(&mut world, f, ita).unwrap();
    assert_eq!(world.diplomacy.factions[0].members.len(), 2);
    assert_eq!(world.diplomacy.faction_of(ita), Some(f));
}

#[test]
fn test_cannot_join_two_factions() {
    let (mut world, _data) = build_world();
    let (ger, eng, _fra, ita) = major(&world);

    let axis = factions::create_faction(&mut world, ger, "Axis").unwrap();
    let allies = factions::create_faction(&mut world, eng, "Allies").unwrap();
    factions::join_faction(&mut world, axis, ita).unwrap();
    let err = factions::join_faction(&mut world, allies, ita);
    assert_eq!(err, Err(factions::FactionError::AlreadyInFaction));
}

#[test]
fn test_kick_member() {
    let (mut world, _data) = build_world();
    let (ger, _eng, _fra, ita) = major(&world);

    let f = factions::create_faction(&mut world, ger, "Axis").unwrap();
    factions::join_faction(&mut world, f, ita).unwrap();
    factions::kick_member(&mut world, f, ger, ita).unwrap();
    assert!(world.diplomacy.faction_of(ita).is_none());
    // 不能踢自己
    let err = factions::kick_member(&mut world, f, ger, ger);
    assert_eq!(err, Err(factions::FactionError::NotMember));
}

#[test]
fn test_dissolve_faction() {
    let (mut world, _data) = build_world();
    let (ger, _eng, _fra, ita) = major(&world);

    let f = factions::create_faction(&mut world, ger, "Axis").unwrap();
    factions::join_faction(&mut world, f, ita).unwrap();
    factions::dissolve_faction(&mut world, f, ger).unwrap();
    assert!(world.diplomacy.factions.is_empty());
    assert!(world.diplomacy.faction_of(ger).is_none());
    assert!(world.diplomacy.faction_of(ita).is_none());
}

#[test]
fn test_leader_leaves_dissolves() {
    let (mut world, _data) = build_world();
    let (ger, _eng, _fra, ita) = major(&world);

    let f = factions::create_faction(&mut world, ger, "Axis").unwrap();
    factions::join_faction(&mut world, f, ita).unwrap();
    factions::leave_faction(&mut world, f, ger).unwrap();
    assert!(world.diplomacy.factions.is_empty());
}

#[test]
fn test_transfer_leadership() {
    let (mut world, _data) = build_world();
    let (ger, _eng, _fra, ita) = major(&world);

    let f = factions::create_faction(&mut world, ger, "Axis").unwrap();
    factions::join_faction(&mut world, f, ita).unwrap();
    factions::transfer_leadership(&mut world, f, ger, ita).unwrap();
    assert_eq!(world.diplomacy.factions[0].leader, ita);
}

// ─── Wargoal ────────────────────────────────────────────────

#[test]
fn test_justify_wargoal_pp_cost_and_progress() {
    let (mut world, _data) = build_world();
    let (ger, _eng, _fra, _ita) = major(&world);
    let pol = world.country("POL").unwrap();

    let pre_pp = 200.0f32;
    world.countries.political_power[ger.0 as usize] = pre_pp;

    wargoal::start_justification(&mut world, ger, pol, WargoalType::Annex, None).unwrap();
    let post_pp = world.countries.political_power[ger.0 as usize];
    assert!(
        post_pp < pre_pp,
        "PP should decrease (cost {} → {})",
        pre_pp,
        post_pp
    );

    let pending = wargoal::all_wargoals(&world, ger);
    assert_eq!(pending.len(), 1);
    assert!(!pending[0].justified);

    // 推进 100 天，annex 需 70 × 1.5 = 105 天，故还没好
    for _ in 0..100 {
        wargoal::advance_justification(&mut world);
    }
    let pending = wargoal::all_wargoals(&world, ger);
    assert!(!pending[0].justified, "annex needs >100 days to complete");

    for _ in 0..10 {
        wargoal::advance_justification(&mut world);
    }
    // 现在应已完成
    let justified = wargoal::justified_wargoals(&world, ger);
    assert_eq!(justified.len(), 1);
    assert_eq!(justified[0].kind, WargoalType::Annex);
}

#[test]
fn test_insufficient_pp_blocks_justify() {
    let (mut world, _data) = build_world();
    let (ger, _eng, _fra, _ita) = major(&world);
    let pol = world.country("POL").unwrap();
    world.countries.political_power[ger.0 as usize] = 5.0;

    let res = wargoal::start_justification(&mut world, ger, pol, WargoalType::Annex, None);
    assert_eq!(res, Err(wargoal::WargoalError::InsufficientPP));
}

#[test]
fn test_take_state_requires_state_id() {
    let (mut world, _data) = build_world();
    let (ger, _eng, _fra, _ita) = major(&world);
    let pol = world.country("POL").unwrap();
    world.countries.political_power[ger.0 as usize] = 200.0;

    let err = wargoal::start_justification(&mut world, ger, pol, WargoalType::TakeState, None);
    assert_eq!(err, Err(wargoal::WargoalError::MissingTargetState));

    // 给一个有效 state
    let sid = StateId(0);
    wargoal::start_justification(&mut world, ger, pol, WargoalType::TakeState, Some(sid)).unwrap();
}

// ─── 宣战 / 白和 ─────────────────────────────────────────

#[test]
fn test_declare_war_no_wargoal_fails() {
    let (mut world, _data) = build_world();
    let (ger, eng, _fra, _ita) = major(&world);
    // 无 wargoal
    let res = war::declare_war(&mut world, ger, eng);
    assert_eq!(res, Err(war::WarError::NoJustifiedWargoal));
}

#[test]
fn test_action_declare_war_explains_missing_wargoal() {
    let (world, _data) = build_world();
    let (ger, eng, _fra, _ita) = major(&world);

    let action = DiplomaticAction::DeclareWar { target: eng };
    let availability = evaluate_action(&world, ger, &action);

    assert!(!availability.available);
    assert_eq!(
        availability.reason,
        Some(UnavailableReason::MissingJustifiedWargoal)
    );
}

#[test]
fn test_action_request_military_access_requires_opinion() {
    let (mut world, _data) = build_world();
    let (ger, eng, _fra, _ita) = major(&world);

    let action = DiplomaticAction::RequestMilitaryAccess { target: eng };
    let availability = evaluate_action(&world, ger, &action);
    assert!(!availability.available);
    assert_eq!(
        availability.reason,
        Some(UnavailableReason::OpinionTooLow {
            current: 0,
            required: 51
        })
    );

    world.diplomacy.opinions.set(eng, ger, 80);
    let outcome = execute_action(&mut world, ger, action).unwrap();
    assert_eq!(outcome.summary, "military access requested");
    assert!(!world.diplomacy.has_military_access(ger, eng));
    assert_eq!(world.diplomacy.diplomatic_requests.len(), 1);

    hoi4_logic::diplomacy::tick_diplomatic_requests(&mut world);
    assert!(world.diplomacy.has_military_access(ger, eng));
}

#[test]
fn test_action_invite_to_faction_uses_same_join_rule() {
    let (mut world, _data) = build_world();
    let (ger, _eng, _fra, ita) = major(&world);

    let no_faction = execute_action(
        &mut world,
        ger,
        DiplomaticAction::InviteToFaction { target: ita },
    );
    assert_eq!(
        no_faction,
        Err(DiplomacyError::Unavailable(
            UnavailableReason::NoFactionToInviteFrom
        ))
    );

    factions::create_faction(&mut world, ger, "Axis").unwrap();
    let outcome = execute_action(
        &mut world,
        ger,
        DiplomaticAction::InviteToFaction { target: ita },
    )
    .unwrap();
    assert_eq!(outcome.summary, "faction invitation sent");
    assert_eq!(world.diplomacy.diplomatic_requests.len(), 1);
    assert!(world.diplomacy.faction_of(ita).is_none());

    hoi4_logic::diplomacy::tick_diplomatic_requests(&mut world);
    assert_eq!(
        world.diplomacy.faction_of(ita),
        world.diplomacy.faction_of(ger)
    );
}

#[test]
fn test_declare_war_with_justified_wargoal() {
    let (mut world, _data) = build_world();
    let (ger, _eng, _fra, _ita) = major(&world);
    let pol = world.country("POL").unwrap();
    world.countries.political_power[ger.0 as usize] = 500.0;

    wargoal::start_justification(&mut world, ger, pol, WargoalType::Annex, None).unwrap();
    // 跑足天数
    for _ in 0..120 {
        wargoal::advance_justification(&mut world);
    }
    let pre_tension = world.diplomacy.world_tension;

    let war_id = war::declare_war(&mut world, ger, pol).unwrap();
    assert_eq!(war_id, 0);
    assert!(world.diplomacy.at_war_with(ger, pol));
    assert!(world.diplomacy.is_at_war(ger));
    assert!(world.countries.at_war[ger.0 as usize]);
    assert!(world.countries.at_war[pol.0 as usize]);
    // pending wargoals 已转入 War
    assert!(wargoal::justified_wargoals(&world, ger).is_empty());
    let war = world.diplomacy.wars.get(&war_id).unwrap();
    assert_eq!(war.attacker_wargoals.len(), 1);
    // 紧张度增加
    assert!(world.diplomacy.world_tension > pre_tension);
}

#[test]
fn test_faction_members_auto_join_war() {
    let (mut world, _data) = build_world();
    let (ger, eng, fra, ita) = major(&world);
    let pol = world.country("POL").unwrap();

    // GER + ITA = Axis；ENG + FRA = Allies；POL 独立
    let axis = factions::create_faction(&mut world, ger, "Axis").unwrap();
    factions::join_faction(&mut world, axis, ita).unwrap();
    let allies = factions::create_faction(&mut world, eng, "Allies").unwrap();
    factions::join_faction(&mut world, allies, fra).unwrap();

    // GER 正当化对 POL 的 wargoal
    world.countries.political_power[ger.0 as usize] = 500.0;
    wargoal::start_justification(&mut world, ger, pol, WargoalType::Annex, None).unwrap();
    for _ in 0..120 {
        wargoal::advance_justification(&mut world);
    }

    let _war_id = war::declare_war(&mut world, ger, pol).unwrap();
    // GER 阵营连带：ITA 入战。POL 没有阵营，ENG/FRA 不入战。
    let war = world.diplomacy.wars.values().next().unwrap();
    assert!(war.attackers.contains(&ger));
    assert!(war.attackers.contains(&ita));
    assert!(war.defenders.contains(&pol));
    assert!(!war.defenders.contains(&eng));
    assert!(!war.defenders.contains(&fra));
}

#[test]
fn test_white_peace_clears_war() {
    let (mut world, _data) = build_world();
    let (ger, _eng, _fra, _ita) = major(&world);
    let pol = world.country("POL").unwrap();
    world.countries.political_power[ger.0 as usize] = 500.0;
    wargoal::start_justification(&mut world, ger, pol, WargoalType::Annex, None).unwrap();
    for _ in 0..120 {
        wargoal::advance_justification(&mut world);
    }
    let war_id = war::declare_war(&mut world, ger, pol).unwrap();
    assert!(world.diplomacy.at_war_with(ger, pol));

    war::white_peace(&mut world, war_id).unwrap();
    assert!(!world.diplomacy.at_war_with(ger, pol));
    assert!(!world.countries.at_war[ger.0 as usize]);
    assert!(!world.countries.at_war[pol.0 as usize]);
    assert!(world.diplomacy.wars.is_empty());
}

#[test]
fn test_action_resolve_peace_requires_score_or_capitulation() {
    let (mut world, _data) = build_world();
    let (ger, _eng, _fra, _ita) = major(&world);
    let pol = world.country("POL").unwrap();
    world.countries.political_power[ger.0 as usize] = 500.0;
    wargoal::start_justification(&mut world, ger, pol, WargoalType::Annex, None).unwrap();
    for _ in 0..120 {
        wargoal::advance_justification(&mut world);
    }
    let war_id = war::declare_war(&mut world, ger, pol).unwrap();

    let action = DiplomaticAction::ResolvePeace {
        war_id,
        winning_side: WarSide::Attacker,
    };
    let availability = evaluate_action(&world, ger, &action);
    assert_eq!(availability.reason, Some(UnavailableReason::PeaceNotReady));

    world
        .diplomacy
        .wars
        .get_mut(&war_id)
        .unwrap()
        .attacker_war_score = 55.0;
    let availability = evaluate_action(&world, ger, &action);
    assert!(availability.available);
}

#[test]
fn test_action_resolve_peace_available_after_target_capitulated_and_evicted() {
    let (mut world, _data) = build_world();
    let (ger, _eng, _fra, _ita) = major(&world);
    let aus = world.country("AUS").unwrap();
    world.countries.political_power[ger.0 as usize] = 500.0;
    wargoal::start_justification(&mut world, ger, aus, WargoalType::Annex, None).unwrap();
    for _ in 0..120 {
        wargoal::advance_justification(&mut world);
    }
    let war_id = war::declare_war(&mut world, ger, aus).unwrap();

    for si in 0..world.states.count {
        if world.states.owners[si] == aus {
            world.states.controllers[si] = ger;
            for province in world.states.provinces[si].clone() {
                world.provinces.controllers[province.0 as usize] = ger;
            }
        }
    }
    let evicted = war::evict_capitulated_daily(&mut world);
    assert!(
        evicted > 0,
        "AUS should capitulate after all states are controlled"
    );
    assert!(
        !world.diplomacy.wars[&war_id].defenders.contains(&aus),
        "evicted target is no longer in current defender set"
    );

    let availability = evaluate_action(
        &world,
        ger,
        &DiplomaticAction::ResolvePeace {
            war_id,
            winning_side: WarSide::Attacker,
        },
    );
    assert!(
        availability.available,
        "peace should be available after wargoal target capitulates even if evicted from active side: {:?}",
        availability.reason
    );
}

// ─── 世界紧张度 ────────────────────────────────────────

#[test]
fn test_world_tension_rises_with_wars() {
    let (mut world, _data) = build_world();
    let (ger, _eng, _fra, _ita) = major(&world);
    let pol = world.country("POL").unwrap();
    world.countries.political_power[ger.0 as usize] = 500.0;
    wargoal::start_justification(&mut world, ger, pol, WargoalType::Annex, None).unwrap();
    for _ in 0..120 {
        wargoal::advance_justification(&mut world);
    }
    let pre = world.diplomacy.world_tension;
    war::declare_war(&mut world, ger, pol).unwrap();
    let after_decl = world.diplomacy.world_tension;
    assert!(after_decl > pre + 4.0, "declaration should add ~5 tension");

    // 推 30 天，紧张度继续增长
    for _ in 0..30 {
        tension::tick_world_tension_daily(&mut world);
    }
    println!(
        "Tension after 30 days war: {:.2}",
        world.diplomacy.world_tension
    );
    assert!(
        world.diplomacy.world_tension > after_decl,
        "ongoing war should raise tension over time"
    );
}

#[test]
fn test_world_tension_decay_when_quiet() {
    let (mut world, _data) = build_world();
    world.diplomacy.world_tension = 50.0;
    for _ in 0..30 {
        tension::tick_world_tension_daily(&mut world);
    }
    // 30 天没新事件 → 紧张度应下降
    assert!(
        world.diplomacy.world_tension < 50.0,
        "tension should decay when no wars or wargoals (got {})",
        world.diplomacy.world_tension
    );
}

#[test]
fn test_pending_wargoals_contribute_to_tension() {
    let (mut world, _data) = build_world();
    let (ger, _eng, _fra, _ita) = major(&world);
    let pol = world.country("POL").unwrap();
    world.countries.political_power[ger.0 as usize] = 500.0;
    wargoal::start_justification(&mut world, ger, pol, WargoalType::Annex, None).unwrap();

    let c = tension::tick_world_tension_daily(&mut world);
    println!("Tension contributors: {:?}", c);
    assert!(
        c.from_pending_wargoals > 0.0,
        "pending wargoal should add tension"
    );
}

// ─── 和平会议 ────────────────────────────────────────

#[test]
fn test_peace_conference_annex() {
    let (mut world, _data) = build_world();
    let (ger, _eng, _fra, _ita) = major(&world);
    let pol = world.country("POL").unwrap();
    world.countries.political_power[ger.0 as usize] = 500.0;
    wargoal::start_justification(&mut world, ger, pol, WargoalType::Annex, None).unwrap();
    for _ in 0..120 {
        wargoal::advance_justification(&mut world);
    }
    let war_id = war::declare_war(&mut world, ger, pol).unwrap();

    // 数 POL 拥有的 state
    let pre_pol_states = (0..world.states.count)
        .filter(|&i| world.states.owners[i] == pol)
        .count();
    assert!(pre_pol_states > 0, "POL should have some states");

    let outcome = peace::peace_conference(&mut world, war_id, WarSide::Attacker).unwrap();
    assert_eq!(outcome.annexed_countries, 1);
    assert!(outcome.states_transferred > 0);

    // POL 已被吞并
    let post_pol_states = (0..world.states.count)
        .filter(|&i| world.states.owners[i] == pol)
        .count();
    assert_eq!(post_pol_states, 0);
    assert!(world.diplomacy.annexed_countries.contains(&pol));

    // GER 工业增加
    let (ger_civ, ger_mil, _) = world.country_industry(ger);
    println!("GER industry after annex: civ={}, mil={}", ger_civ, ger_mil);
}

#[test]
fn test_peace_conference_take_state() {
    let (mut world, _data) = build_world();
    let (ger, _eng, _fra, _ita) = major(&world);
    let pol = world.country("POL").unwrap();
    world.countries.political_power[ger.0 as usize] = 500.0;

    // 找一个 POL 的 state
    let pol_state = (0..world.states.count)
        .find(|&i| world.states.owners[i] == pol)
        .map(|i| StateId(i as u16))
        .expect("POL should own at least one state");

    wargoal::start_justification(
        &mut world,
        ger,
        pol,
        WargoalType::TakeState,
        Some(pol_state),
    )
    .unwrap();
    for _ in 0..80 {
        wargoal::advance_justification(&mut world);
    }
    let war_id = war::declare_war(&mut world, ger, pol).unwrap();

    let outcome = peace::peace_conference(&mut world, war_id, WarSide::Attacker).unwrap();
    assert_eq!(outcome.annexed_countries, 0);
    assert_eq!(outcome.states_transferred, 1);
    assert_eq!(world.states.owners[pol_state.0 as usize], ger);
}

#[test]
fn test_peace_conference_puppet() {
    let (mut world, _data) = build_world();
    let (ger, _eng, _fra, _ita) = major(&world);
    let pol = world.country("POL").unwrap();
    world.countries.political_power[ger.0 as usize] = 500.0;

    wargoal::start_justification(&mut world, ger, pol, WargoalType::Puppet, None).unwrap();
    for _ in 0..120 {
        wargoal::advance_justification(&mut world);
    }
    let war_id = war::declare_war(&mut world, ger, pol).unwrap();

    let outcome = peace::peace_conference(&mut world, war_id, WarSide::Attacker).unwrap();
    assert_eq!(outcome.puppets_created, 1);
    assert!(world.diplomacy.is_subject_of(pol, ger));
    assert_eq!(
        world.diplomacy.autonomy.get(&pol).unwrap().level,
        AutonomyLevel::Puppet
    );
}

// ─── 傀儡 / 自治度 ────────────────────────────────────

#[test]
fn test_set_puppet_creates_relation() {
    let (mut world, _data) = build_world();
    let (ger, _eng, _fra, _ita) = major(&world);
    let pol = world.country("POL").unwrap();

    puppet::set_puppet(&mut world, ger, pol, AutonomyLevel::Puppet).unwrap();
    assert!(world.diplomacy.is_subject_of(pol, ger));
    let a = world.diplomacy.autonomy.get(&pol).unwrap();
    assert_eq!(a.master, ger);
    assert_eq!(a.level, AutonomyLevel::Puppet);
}

#[test]
fn test_cannot_puppet_self() {
    let (mut world, _data) = build_world();
    let (ger, _eng, _fra, _ita) = major(&world);
    let err = puppet::set_puppet(&mut world, ger, ger, AutonomyLevel::Puppet);
    assert_eq!(err, Err(puppet::AutonomyError::SelfPuppet));
}

#[test]
fn test_autonomy_progress_advances_daily() {
    let (mut world, _data) = build_world();
    let (ger, _eng, _fra, _ita) = major(&world);
    let pol = world.country("POL").unwrap();

    puppet::set_puppet(&mut world, ger, pol, AutonomyLevel::IntegratedPuppet).unwrap();
    let pre_progress = world.diplomacy.autonomy.get(&pol).unwrap().progress;

    for _ in 0..30 {
        puppet::tick_autonomy_daily(&mut world);
    }
    let post_progress = world.diplomacy.autonomy.get(&pol).unwrap().progress;
    println!(
        "Autonomy progress 30d: {} → {}",
        pre_progress, post_progress
    );
    assert!(
        post_progress > pre_progress,
        "autonomy progress should increase daily"
    );
}

#[test]
fn test_autonomy_upgrades_after_threshold() {
    let (mut world, _data) = build_world();
    let (ger, _eng, _fra, _ita) = major(&world);
    let pol = world.country("POL").unwrap();

    puppet::set_puppet(&mut world, ger, pol, AutonomyLevel::IntegratedPuppet).unwrap();
    // 直接把 progress 设到阈值（任何正向 daily 增量都会触发升级）
    {
        let a = world.diplomacy.autonomy.get_mut(&pol).unwrap();
        a.progress = AutonomyLevel::IntegratedPuppet.upgrade_threshold();
    }
    // 推进一天 → 应升级
    puppet::tick_autonomy_daily(&mut world);
    let a = world.diplomacy.autonomy.get(&pol).unwrap();
    println!("After tick: level={:?}, progress={}", a.level, a.progress);
    assert_eq!(a.level, AutonomyLevel::Puppet);
    assert_eq!(a.progress, 0.0);
}

#[test]
fn test_release_puppet_removes_relation() {
    let (mut world, _data) = build_world();
    let (ger, _eng, _fra, _ita) = major(&world);
    let pol = world.country("POL").unwrap();
    puppet::set_puppet(&mut world, ger, pol, AutonomyLevel::Puppet).unwrap();
    puppet::release_puppet(&mut world, ger, pol).unwrap();
    assert!(world.diplomacy.autonomy.get(&pol).is_none());
}

#[test]
fn test_integrate_subject_with_low_progress() {
    let (mut world, _data) = build_world();
    let (ger, _eng, _fra, _ita) = major(&world);
    let pol = world.country("POL").unwrap();
    puppet::set_puppet(&mut world, ger, pol, AutonomyLevel::IntegratedPuppet).unwrap();
    // progress 默认 0 — 应能整合
    let pre_pol_states = (0..world.states.count)
        .filter(|&i| world.states.owners[i] == pol)
        .count();
    puppet::integrate_subject(&mut world, ger, pol).unwrap();
    let post_pol_states = (0..world.states.count)
        .filter(|&i| world.states.owners[i] == pol)
        .count();
    println!(
        "POL states {} → {} after integration",
        pre_pol_states, post_pol_states
    );
    assert_eq!(post_pol_states, 0);
    assert!(world.diplomacy.annexed_countries.contains(&pol));
    assert!(world.diplomacy.autonomy.get(&pol).is_none());
}

// ─── 综合：完整外交场景 ────────────────────────────────

#[test]
fn test_full_diplomacy_scenario() {
    // 场景：GER 创建 Axis（含 ITA），ENG 创建 Allies（含 FRA），
    // GER 正当化 → 宣战 POL → 和平会议吞并 POL。验证所有副作用。
    let (mut world, _data) = build_world();
    let (ger, eng, fra, ita) = major(&world);
    let pol = world.country("POL").unwrap();

    // 建阵营
    let axis = factions::create_faction(&mut world, ger, "Axis").unwrap();
    factions::join_faction(&mut world, axis, ita).unwrap();
    let allies = factions::create_faction(&mut world, eng, "Allies").unwrap();
    factions::join_faction(&mut world, allies, fra).unwrap();
    assert_eq!(world.diplomacy.factions.len(), 2);

    // GER 正当化
    world.countries.political_power[ger.0 as usize] = 500.0;
    wargoal::start_justification(&mut world, ger, pol, WargoalType::Annex, None).unwrap();
    for _ in 0..120 {
        wargoal::advance_justification(&mut world);
        tension::tick_world_tension_daily(&mut world);
    }
    let pre_war_tension = world.diplomacy.world_tension;
    println!("Pre-war tension: {:.2}", pre_war_tension);

    // 宣战
    let war_id = war::declare_war(&mut world, ger, pol).unwrap();
    let war_count_attackers = world.diplomacy.wars[&war_id].attackers.len();
    assert!(war_count_attackers >= 2, "Axis members should be in war");

    // 30 天战争
    for _ in 0..30 {
        tension::tick_world_tension_daily(&mut world);
    }
    let mid_war_tension = world.diplomacy.world_tension;
    println!("After 30d war: {:.2}", mid_war_tension);
    assert!(mid_war_tension > pre_war_tension);

    // 和平会议吞并
    let outcome = peace::peace_conference(&mut world, war_id, WarSide::Attacker).unwrap();
    assert_eq!(outcome.annexed_countries, 1);
    assert!(world.diplomacy.annexed_countries.contains(&pol));
    assert!(!world.countries.at_war[ger.0 as usize]);
    assert!(world.diplomacy.wars.is_empty());

    // 紧张度因吞并继续增加
    println!("Final tension: {:.2}", world.diplomacy.world_tension);
    assert!(
        world.diplomacy.world_tension > mid_war_tension,
        "annexation should add tension"
    );
}

// ─── P0.11：军阀延迟参战测试 ────────────────────────────

#[test]
fn test_reconcile_respects_delayed_war_join_policy() {
    let (mut world, _data) = build_world();
    let (ger, eng, fra, ita) = major(&world);
    let pol = world.country("POL").unwrap();

    // GER+ITA=Axis, ENG+FRA=Allies
    let axis = factions::create_faction(&mut world, ger, "Axis").unwrap();
    factions::join_faction(&mut world, axis, ita).unwrap();
    let allies = factions::create_faction(&mut world, eng, "Allies").unwrap();
    factions::join_faction(&mut world, allies, fra).unwrap();

    // GER 正当化并宣战 POL
    world.countries.political_power[ger.0 as usize] = 500.0;
    wargoal::start_justification(&mut world, ger, pol, WargoalType::Annex, None).unwrap();
    for _ in 0..120 {
        wargoal::advance_justification(&mut world);
    }
    let war_id = war::declare_war(&mut world, ger, pol).unwrap();

    // ITA 已在 Axis 并已在战争中（宣战时连带加入）
    let war = world.diplomacy.wars.get(&war_id).unwrap();
    assert!(war.attackers.contains(&ita), "ITA 应在攻方中");

    // 现在让 HUN 加入 Axis
    let hun = world.country("HUN").unwrap_or_else(|| {
        // 如果 HUN 不存在，用 SPA 作为替代
        world
            .country("SPA")
            .unwrap_or(world.country("ROM").unwrap())
    });
    factions::join_faction(&mut world, axis, hun).unwrap();

    // 设置 HUN 为延迟参战
    war::set_war_join_policy(&mut world, war_id, hun, WarJoinPolicy::Delayed).unwrap();

    // reconcile 应该跳过 HUN
    let newly_added = war::reconcile_war_membership(&mut world);
    // HUN 在阵营但不在战争中
    let war = world.diplomacy.wars.get(&war_id).unwrap();
    assert!(
        !war.attackers.contains(&hun),
        "延迟参战的阵营成员不应被自动拉入战争"
    );
    assert!(
        !war.defenders.contains(&hun),
        "延迟参战的阵营成员不应在守方中"
    );
    assert!(
        !world.countries.at_war[hun.0 as usize],
        "延迟参战国家不应标记为交战"
    );
}

#[test]
fn test_reconcile_auto_joins_normal_faction_members() {
    let (mut world, _data) = build_world();
    let (ger, eng, fra, ita) = major(&world);
    let pol = world.country("POL").unwrap();

    let axis = factions::create_faction(&mut world, ger, "Axis").unwrap();
    factions::join_faction(&mut world, axis, ita).unwrap();

    world.countries.political_power[ger.0 as usize] = 500.0;
    wargoal::start_justification(&mut world, ger, pol, WargoalType::Annex, None).unwrap();
    for _ in 0..120 {
        wargoal::advance_justification(&mut world);
    }
    let war_id = war::declare_war(&mut world, ger, pol).unwrap();

    // HUN 加入 Axis（无延迟策略）
    let hun = world.country("HUN").unwrap_or_else(|| {
        world
            .country("SPA")
            .unwrap_or(world.country("ROM").unwrap())
    });
    factions::join_faction(&mut world, axis, hun).unwrap();

    let _newly_added = war::reconcile_war_membership(&mut world);
    let war = world.diplomacy.wars.get(&war_id).unwrap();
    assert!(
        war.attackers.contains(&hun),
        "无延迟策略的阵营成员应被自动拉入战争"
    );
    assert!(
        world.countries.at_war[hun.0 as usize],
        "自动参战国家应标记为交战"
    );
}

#[test]
fn test_add_delayed_war_participant() {
    let (mut world, _data) = build_world();
    let (ger, eng, fra, ita) = major(&world);
    let pol = world.country("POL").unwrap();

    let axis = factions::create_faction(&mut world, ger, "Axis").unwrap();
    factions::join_faction(&mut world, axis, ita).unwrap();

    world.countries.political_power[ger.0 as usize] = 500.0;
    wargoal::start_justification(&mut world, ger, pol, WargoalType::Annex, None).unwrap();
    for _ in 0..120 {
        wargoal::advance_justification(&mut world);
    }
    let war_id = war::declare_war(&mut world, ger, pol).unwrap();

    // HUN 加入 Axis 但延迟参战
    let hun = world.country("HUN").unwrap_or_else(|| {
        world
            .country("SPA")
            .unwrap_or(world.country("ROM").unwrap())
    });
    factions::join_faction(&mut world, axis, hun).unwrap();
    war::set_war_join_policy(&mut world, war_id, hun, WarJoinPolicy::Delayed).unwrap();

    // reconcile 跳过 HUN
    war::reconcile_war_membership(&mut world);
    let war = world.diplomacy.wars.get(&war_id).unwrap();
    assert!(!war.attackers.contains(&hun));

    // 事件触发后，HUN 正式参战
    war::add_delayed_war_participant(&mut world, war_id, ger, hun).unwrap();
    let war = world.diplomacy.wars.get(&war_id).unwrap();
    assert!(
        war.attackers.contains(&hun),
        "事件触发后延迟参战国家应加入攻方"
    );
    assert!(!war.is_auto_join_blocked(hun), "参战后延迟策略应被移除");
    assert!(world.countries.at_war[hun.0 as usize], "参战后应标记为交战");
}

#[test]
fn test_delayed_join_survives_multiple_reconcile() {
    let (mut world, _data) = build_world();
    let (ger, _eng, _fra, ita) = major(&world);
    let pol = world.country("POL").unwrap();

    let axis = factions::create_faction(&mut world, ger, "Axis").unwrap();
    factions::join_faction(&mut world, axis, ita).unwrap();

    world.countries.political_power[ger.0 as usize] = 500.0;
    wargoal::start_justification(&mut world, ger, pol, WargoalType::Annex, None).unwrap();
    for _ in 0..120 {
        wargoal::advance_justification(&mut world);
    }
    let war_id = war::declare_war(&mut world, ger, pol).unwrap();

    let hun = world.country("HUN").unwrap_or_else(|| {
        world
            .country("SPA")
            .unwrap_or(world.country("ROM").unwrap())
    });
    factions::join_faction(&mut world, axis, hun).unwrap();
    war::set_war_join_policy(&mut world, war_id, hun, WarJoinPolicy::Delayed).unwrap();

    // 多次 reconcile 不应破坏延迟状态
    for _ in 0..10 {
        war::reconcile_war_membership(&mut world);
    }
    let war = world.diplomacy.wars.get(&war_id).unwrap();
    assert!(
        !war.attackers.contains(&hun),
        "多次 reconcile 后延迟参战状态不应被破坏"
    );
    assert!(war.is_auto_join_blocked(hun), "延迟策略应保持有效");
}

// ─── P1.1：战争投降与和平自动结算测试 ────────────────────────────

#[test]
fn test_p11_war_deleted_after_full_side_capitulation() {
    // 战争一方全部投降后，战争应被删除或进入和平会议
    let (mut world, _data) = build_world();
    let (ger, _eng, _fra, _ita) = major(&world);
    let aus = world.country("AUS").unwrap();
    world.countries.political_power[ger.0 as usize] = 500.0;
    wargoal::start_justification(&mut world, ger, aus, WargoalType::Annex, None).unwrap();
    for _ in 0..120 {
        wargoal::advance_justification(&mut world);
    }
    let war_id = war::declare_war(&mut world, ger, aus).unwrap();
    assert!(world.diplomacy.wars.contains_key(&war_id));

    // AUS 丢失所有州控制权
    for si in 0..world.states.count {
        if world.states.owners[si] == aus {
            world.states.controllers[si] = ger;
            for province in world.states.provinces[si].clone() {
                world.provinces.controllers[province.0 as usize] = ger;
            }
        }
    }

    // 投降驱逐
    let evicted = war::evict_capitulated_daily(&mut world);
    assert!(evicted > 0, "AUS 应被判定投降并驱逐");

    // 和平自动结算
    let outcome = war::tick_peace_resolution_daily(&mut world);
    assert!(
        !outcome.resolved_wars.is_empty() || outcome.empty_wars_removed > 0,
        "一方全部投降后应自动结算或删除战争"
    );

    // 战争应被删除
    assert!(
        !world.diplomacy.wars.contains_key(&war_id),
        "战争应在投降结算后删除"
    );

    // 双方 at_war 状态正确
    assert!(
        !world.countries.at_war[ger.0 as usize],
        "GER 战后应不在交战状态"
    );
    assert!(
        !world.countries.at_war[aus.0 as usize],
        "AUS 投降后应不在交战状态"
    );
}

#[test]
fn test_p11_at_war_correct_after_capitulation_and_cleanup() {
    // 所有参战国 at_war 状态在战争清理后正确
    let (mut world, _data) = build_world();
    let (ger, eng, fra, ita) = major(&world);
    let aus = world.country("AUS").unwrap();

    // GER+ITA=Axis, ENG+FRA=Allies
    let axis = factions::create_faction(&mut world, ger, "Axis").unwrap();
    factions::join_faction(&mut world, axis, ita).unwrap();
    let allies = factions::create_faction(&mut world, eng, "Allies").unwrap();
    factions::join_faction(&mut world, allies, fra).unwrap();

    // GER 对 AUS 宣战（Axis 连带加入）
    world.countries.political_power[ger.0 as usize] = 500.0;
    wargoal::start_justification(&mut world, ger, aus, WargoalType::Annex, None).unwrap();
    for _ in 0..120 {
        wargoal::advance_justification(&mut world);
    }
    let _war_id = war::declare_war(&mut world, ger, aus).unwrap();

    // AUS 全部丢失
    for si in 0..world.states.count {
        if world.states.owners[si] == aus {
            world.states.controllers[si] = ger;
            for province in world.states.provinces[si].clone() {
                world.provinces.controllers[province.0 as usize] = ger;
            }
        }
    }

    let _evicted = war::evict_capitulated_daily(&mut world);
    let _outcome = war::tick_peace_resolution_daily(&mut world);

    // 验证所有国家 at_war 正确
    for ci in 0..world.countries.count {
        let cid = hoi4_state::CountryId(ci as u16);
        let expected = world.diplomacy.is_at_war(cid);
        let actual = world.countries.at_war[ci];
        assert_eq!(
            actual, expected,
            "国家 {} at_war 应为 {}，实际为 {}",
            ci, expected, actual
        );
    }
}

#[test]
fn test_p11_province_transfer_after_peace_conference() {
    // 省份转移、吞并结果可测试
    let (mut world, _data) = build_world();
    let (ger, _eng, _fra, _ita) = major(&world);
    let aus = world.country("AUS").unwrap();
    world.countries.political_power[ger.0 as usize] = 500.0;
    wargoal::start_justification(&mut world, ger, aus, WargoalType::Annex, None).unwrap();
    for _ in 0..120 {
        wargoal::advance_justification(&mut world);
    }
    let _war_id = war::declare_war(&mut world, ger, aus).unwrap();

    let pre_aus_states: Vec<usize> = (0..world.states.count)
        .filter(|&i| world.states.owners[i] == aus)
        .collect();
    assert!(!pre_aus_states.is_empty(), "AUS 应有省份");

    // AUS 投降
    for si in 0..world.states.count {
        if world.states.owners[si] == aus {
            world.states.controllers[si] = ger;
            for province in world.states.provinces[si].clone() {
                world.provinces.controllers[province.0 as usize] = ger;
            }
        }
    }
    let _evicted = war::evict_capitulated_daily(&mut world);
    let _outcome = war::tick_peace_resolution_daily(&mut world);

    // 吞并后 AUS 所有省份应转给 GER
    for &si in &pre_aus_states {
        assert_eq!(
            world.states.owners[si], ger,
            "AUS 州 {} 吞并后应归 GER 所有",
            si
        );
        assert_eq!(
            world.states.controllers[si], ger,
            "AUS 州 {} 吞并后应归 GER 控制",
            si
        );
    }
    assert!(
        world.diplomacy.annexed_countries.contains(&aus),
        "AUS 应被标记为已吞并"
    );
}

#[test]
fn test_p11_puppet_result_after_peace_conference() {
    // 傀儡化结果可测试
    let (mut world, _data) = build_world();
    let (ger, _eng, _fra, _ita) = major(&world);
    let aus = world.country("AUS").unwrap();
    world.countries.political_power[ger.0 as usize] = 500.0;
    wargoal::start_justification(&mut world, ger, aus, WargoalType::Puppet, None).unwrap();
    for _ in 0..120 {
        wargoal::advance_justification(&mut world);
    }
    let _war_id = war::declare_war(&mut world, ger, aus).unwrap();

    // AUS 投降
    for si in 0..world.states.count {
        if world.states.owners[si] == aus {
            world.states.controllers[si] = ger;
            for province in world.states.provinces[si].clone() {
                world.provinces.controllers[province.0 as usize] = ger;
            }
        }
    }
    let _evicted = war::evict_capitulated_daily(&mut world);
    let _outcome = war::tick_peace_resolution_daily(&mut world);

    // AUS 应成为 GER 的傀儡
    assert!(
        world.diplomacy.is_subject_of(aus, ger),
        "AUS 应成为 GER 的傀儡"
    );
    assert_eq!(
        world.diplomacy.autonomy.get(&aus).unwrap().level,
        AutonomyLevel::Puppet,
        "AUS 应为标准傀儡等级"
    );
    assert!(
        !world.countries.at_war[ger.0 as usize],
        "GER 战后应不在交战状态"
    );
}

#[test]
fn test_p11_cleanup_war_removes_war_and_updates_at_war() {
    // cleanup_war 函数直接测试
    let (mut world, _data) = build_world();
    let (ger, _eng, _fra, _ita) = major(&world);
    let pol = world.country("POL").unwrap();
    world.countries.political_power[ger.0 as usize] = 500.0;
    wargoal::start_justification(&mut world, ger, pol, WargoalType::Annex, None).unwrap();
    for _ in 0..120 {
        wargoal::advance_justification(&mut world);
    }
    let war_id = war::declare_war(&mut world, ger, pol).unwrap();
    assert!(world.countries.at_war[ger.0 as usize]);
    assert!(world.countries.at_war[pol.0 as usize]);

    let result = war::cleanup_war(&mut world, war_id);
    assert!(result.is_ok(), "cleanup_war 应成功");
    assert!(!world.diplomacy.wars.contains_key(&war_id), "战争应被删除");
    assert!(
        !world.countries.at_war[ger.0 as usize],
        "GER at_war 应为 false"
    );
    assert!(
        !world.countries.at_war[pol.0 as usize],
        "POL at_war 应为 false"
    );
}

#[test]
fn test_p11_remove_empty_wars_after_eviction() {
    // evict 后如果某方为空，remove_empty_wars 应删除该战争
    let (mut world, _data) = build_world();
    let (ger, _eng, _fra, _ita) = major(&world);
    let aus = world.country("AUS").unwrap();
    world.countries.political_power[ger.0 as usize] = 500.0;
    wargoal::start_justification(&mut world, ger, aus, WargoalType::Annex, None).unwrap();
    for _ in 0..120 {
        wargoal::advance_justification(&mut world);
    }
    let war_id = war::declare_war(&mut world, ger, aus).unwrap();

    // AUS 全部丢失
    for si in 0..world.states.count {
        if world.states.owners[si] == aus {
            world.states.controllers[si] = ger;
            for province in world.states.provinces[si].clone() {
                world.provinces.controllers[province.0 as usize] = ger;
            }
        }
    }

    // 驱逐投降国
    let _evicted = war::evict_capitulated_daily(&mut world);

    // 检查 AUS 被驱逐后防守方可能为空
    let war = world.diplomacy.wars.get(&war_id);
    if let Some(w) = war {
        if w.defenders.is_empty() || w.attackers.is_empty() {
            let removed = war::remove_empty_wars(&mut world);
            assert!(removed > 0, "空战争应被删除");
            assert!(
                !world.diplomacy.wars.contains_key(&war_id),
                "空战争应被删除"
            );
        }
    }
}

#[test]
fn test_p11_two_country_war_both_sides_gone() {
    // 双方全部被吞并时，战争直接删除（无和平会议）
    let (mut world, _data) = build_world();
    let (ger, _eng, _fra, _ita) = major(&world);
    let aus = world.country("AUS").unwrap();
    world.countries.political_power[ger.0 as usize] = 500.0;
    wargoal::start_justification(&mut world, ger, aus, WargoalType::Annex, None).unwrap();
    for _ in 0..120 {
        wargoal::advance_justification(&mut world);
    }
    let war_id = war::declare_war(&mut world, ger, aus).unwrap();

    // AUS 全部丢失 → 被驱逐
    for si in 0..world.states.count {
        if world.states.owners[si] == aus {
            world.states.controllers[si] = ger;
            for province in world.states.provinces[si].clone() {
                world.provinces.controllers[province.0 as usize] = ger;
            }
        }
    }
    let _evicted = war::evict_capitulated_daily(&mut world);

    // 现在把 GER 也标记为已吞并（极端场景）
    world.diplomacy.annexed_countries.insert(ger);

    let _outcome = war::tick_peace_resolution_daily(&mut world);
    // 双方全部出局，战争应被直接删除
    assert!(
        !world.diplomacy.wars.contains_key(&war_id),
        "双方全部出局后战争应被删除"
    );
}
