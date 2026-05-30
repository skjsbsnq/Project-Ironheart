//! 存档系统集成测试 — 圆 trip 验证（save → load → 对比）

use std::collections::HashSet;
use std::sync::Arc;

use hoi4_data::GameData;
use hoi4_map::GameMap;
use hoi4_state::save::{self, SaveKind};
use hoi4_state::{
    ArmyId, Autonomy, AutonomyLevel, Building, BuildingKind, BuildingOwner, CountryId,
    DiplomaticRequestKind, Faction, FactionId, FrontlineOrder, GameDate, GameSpeed, OffensiveArrow,
    PlayerArmy, ProvinceId, StateId, StateIntegrationStatus, TreatyKind, War, Wargoal, WargoalType,
    World,
};

fn hoi4_path() -> std::path::PathBuf {
    hoi4_paths::PathConfig::resolve(Default::default())
        .expect("set IRONHEART_HOI4_PATH or place HOI4 install at default Steam path")
        .game_path()
        .to_path_buf()
}

fn build_world() -> World {
    let game_path = hoi4_path();
    let map = Arc::new(GameMap::load(&game_path).unwrap());
    let data = Arc::new(GameData::load(&game_path).unwrap());
    let mut world = World::new(map, data);
    world.populate_from_history();
    world
}

/// 给世界做一些可观察的改动，使其偏离初始状态
fn mutate_world(world: &mut World) {
    // 推进时间
    world.tick_day();
    world.tick_day();
    world.tick_hour();

    // 设玩家、种子、unique id
    world.player = world.country("GER").unwrap();
    world.random_seed = 0xCAFEBABE_DEADBEEF;
    world.game_unique_id = 42;
    world.speed = GameSpeed::Speed3;

    // 改一些国家状态
    let ger = world.country("GER").unwrap();
    let i = ger.0 as usize;
    world.countries.political_power[i] = 123.5;
    world.countries.stability[i] = 0.75;
    world.countries.war_support[i] = 0.42;
    world.countries.fuel[i] = 999.5;
    world.countries.army_xp[i] = 150.25;
    world.countries.at_war[i] = true;
    world.countries.completed_techs[i].push("save_test_tech".into());
    world.countries.ideas[i].push("save_test_idea".into());

    // 改一个州的运行时状态
    let cap = world.countries.capitals[i];
    if !cap.is_none() {
        let si = cap.0 as usize;
        world.states.resistance[si] = 0.33;
        world.states.integration_status[si] = StateIntegrationStatus::Colony;
    }

    // 模拟一次省份"被占领"：把柏林改成苏联控制
    let sov = world.country("SOV").unwrap();
    world.provinces.controllers[6521] = sov;
    world.provinces.supply[6521] = 12.5;

    // V6: recalc_country_caches is now a no-op (building-based calculations replace cached factory counts).
    // Keeping the call for API compatibility; it no longer modifies state.
    world.recalc_country_caches();

    mutate_diplomacy(world);

    // ─── player_armies (frontline-orders R10/R12) ───
    // 找两个相邻的陆地省作为 path（柏林附近）
    let ger = world.country("GER").unwrap();
    let ger_divs: Vec<usize> = (0..world.divisions.count)
        .filter(|&i| world.divisions.owners[i] == ger)
        .take(3)
        .collect();
    if ger_divs.len() >= 2 && world.map.adjacencies.len() > 6521 {
        let neighbors: Vec<u16> = world.map.adjacencies[6521].iter().copied().collect();
        if neighbors.len() >= 2 {
            let p0 = ProvinceId(6521);
            let p1 = ProvinceId(neighbors[0]);
            let path = vec![p0, p1];
            let a1 = ProvinceId(neighbors[1]);
            let arrow = OffensiveArrow {
                provinces: vec![p1, a1],
            };
            let anchor = Some(p0);

            world.player_armies.push(PlayerArmy {
                id: ArmyId(10),
                name: "1. Armee".to_owned(),
                owner: ger,
                commander: world.generals.iter().find(|g| g.owner == ger).map(|g| g.id),
                members: ger_divs.clone(),
                order: Some(FrontlineOrder {
                    path,
                    arrow: Some(arrow),
                    anchor,
                    active: true,
                    executing: false,
                }),
            });

            world.player_armies.push(PlayerArmy {
                id: ArmyId(20),
                name: "2. Armee".to_owned(),
                owner: ger,
                commander: None,
                members: vec![],
                order: None,
            });

            world.next_army_id = 21;
        }
    }
}

fn mutate_diplomacy(world: &mut World) {
    let ger = world.country("GER").unwrap();
    let ita = world.country("ITA").unwrap();
    let eng = world.country("ENG").unwrap();
    let fra = world.country("FRA").unwrap();
    let pol = world.country("POL").unwrap();
    let sov = world.country("SOV").unwrap();

    world.diplomacy.world_tension = 12.5;
    world.diplomacy.next_war_id = 8;
    world.diplomacy.next_faction_id = 2;
    world.diplomacy.factions = vec![Faction {
        id: FactionId(1),
        name: "Axis Save Test".to_owned(),
        leader: ger,
        members: vec![ger, ita],
        created_at_hour: world.elapsed_hours,
    }];

    let annex_goal = Wargoal {
        claimant: ger,
        target: pol,
        kind: WargoalType::Annex,
        target_state: None,
        justified: true,
        justify_progress: 105.0,
        justify_total_days: 105.0,
    };
    let take_state_goal = Wargoal {
        claimant: eng,
        target: fra,
        kind: WargoalType::TakeState,
        target_state: Some(StateId(0)),
        justified: false,
        justify_progress: 12.0,
        justify_total_days: 70.0,
    };

    let mut attackers = HashSet::new();
    attackers.insert(ger);
    attackers.insert(ita);
    let mut defenders = HashSet::new();
    defenders.insert(pol);
    world.diplomacy.wars.insert(
        7,
        War {
            id: 7,
            primary_attacker: ger,
            primary_defender: pol,
            attackers,
            defenders,
            started_at_hour: world.elapsed_hours,
            attacker_war_score: 33.0,
            defender_war_score: 12.0,
            attacker_wargoals: vec![annex_goal.clone()],
            defender_wargoals: vec![],
            war_join_policies: std::collections::HashMap::new(),
        },
    );

    world.diplomacy.autonomy.insert(
        fra,
        Autonomy {
            master: eng,
            subject: fra,
            level: AutonomyLevel::Dominion,
            progress: 42.0,
            since_hour: world.elapsed_hours,
        },
    );
    world
        .diplomacy
        .pending_wargoals
        .insert(eng, vec![take_state_goal]);
    world.diplomacy.opinions.set(ger, ita, 88);
    world.diplomacy.opinions.set(ita, ger, 75);
    world.diplomacy.annexed_countries.insert(sov);
    world.diplomacy.grant_military_access(eng, ger);
    world.diplomacy.next_treaty_id = world.diplomacy.next_treaty_id.max(2);
    world.diplomacy.create_request(
        ger,
        ita,
        DiplomaticRequestKind::InviteToFaction {
            faction_id: FactionId(1),
        },
        world.elapsed_hours,
        Some(world.elapsed_hours + 24 * 30),
    );
}

/// 比较两个 world 在保存范围内的字段相等
fn assert_world_equiv(a: &World, b: &World) {
    assert_eq!(a.date, b.date, "date mismatch");
    assert_eq!(a.elapsed_hours, b.elapsed_hours, "elapsed_hours mismatch");
    assert_eq!(a.random_seed, b.random_seed, "random_seed mismatch");
    assert_eq!(
        a.game_unique_id, b.game_unique_id,
        "game_unique_id mismatch"
    );
    assert_eq!(a.player, b.player, "player mismatch");
    assert_eq!(a.speed, b.speed, "speed mismatch");

    assert_eq!(a.countries.count, b.countries.count, "country count");
    assert_eq!(a.states.count, b.states.count, "state count");
    assert_eq!(a.provinces.count, b.provinces.count, "province count");

    // 抽样检查 GER 的关键字段
    let ger_a = a.country("GER").unwrap();
    let ger_b = b.country("GER").unwrap();
    let ia = ger_a.0 as usize;
    let ib = ger_b.0 as usize;

    assert_eq!(
        a.countries.political_power[ia], b.countries.political_power[ib],
        "GER political_power"
    );
    assert_eq!(
        a.countries.stability[ia], b.countries.stability[ib],
        "GER stability"
    );
    assert_eq!(
        a.countries.war_support[ia], b.countries.war_support[ib],
        "GER war_support"
    );
    assert_eq!(a.countries.fuel[ia], b.countries.fuel[ib], "GER fuel");
    assert_eq!(
        a.countries.army_xp[ia], b.countries.army_xp[ib],
        "GER army_xp"
    );
    assert_eq!(a.countries.at_war[ia], b.countries.at_war[ib], "GER at_war");
    assert_eq!(
        a.countries.completed_techs[ia], b.countries.completed_techs[ib],
        "GER completed_techs"
    );
    assert_eq!(a.countries.ideas[ia], b.countries.ideas[ib], "GER ideas");

    // GER 首都 state
    let cap_a = a.countries.capitals[ia];
    let cap_b = b.countries.capitals[ib];
    let sa = cap_a.0 as usize;
    let sb = cap_b.0 as usize;
    assert_eq!(
        a.states.resistance[sa], b.states.resistance[sb],
        "GER cap resistance"
    );

    // 柏林控制者
    assert_eq!(
        a.provinces.controllers[6521], b.provinces.controllers[6521],
        "Berlin controller"
    );
    assert_eq!(
        a.provinces.supply[6521], b.provinces.supply[6521],
        "Berlin supply"
    );

    // 比较所有 state 的 owner / manpower
    for i in 0..a.states.count {
        assert_eq!(
            a.states.owners[i], b.states.owners[i],
            "state[{}] owner mismatch",
            i
        );
        assert_eq!(
            a.states.manpower_pool[i], b.states.manpower_pool[i],
            "state[{}] manpower_pool",
            i
        );
        assert_eq!(
            a.states.integration_status[i], b.states.integration_status[i],
            "state[{}] integration_status",
            i
        );
    }

    // 抽样比较国家工厂 cached
    let (civ_a, mil_a, dock_a) = a.country_industry(ger_a);
    let (civ_b, mil_b, dock_b) = b.country_industry(ger_b);
    assert_eq!(civ_a, civ_b, "GER civ cached");
    assert_eq!(mil_a, mil_b, "GER mil cached");
    assert_eq!(dock_a, dock_b, "GER dock cached");

    // player_armies round-trip（_R10.1, R10.2_）
    assert_eq!(
        a.player_armies.len(),
        b.player_armies.len(),
        "player_armies count"
    );
    for (ai, bi) in a.player_armies.iter().zip(b.player_armies.iter()) {
        assert_eq!(ai.id, bi.id, "ArmyId mismatch");
        assert_eq!(ai.name, bi.name, "army name mismatch");
        assert_eq!(ai.owner, bi.owner, "army owner mismatch");
        assert_eq!(ai.commander, bi.commander, "army commander mismatch");
        assert_eq!(ai.members, bi.members, "army members mismatch");
        assert_eq!(ai.order, bi.order, "army order mismatch");
    }
    assert_eq!(a.next_army_id, b.next_army_id, "next_army_id mismatch");
    assert_eq!(a.generals, b.generals, "generals mismatch");
    assert_eq!(
        a.next_general_id, b.next_general_id,
        "next_general_id mismatch"
    );

    assert_diplomacy_equiv(a, b);
}

fn assert_diplomacy_equiv(a: &World, b: &World) {
    assert_eq!(
        a.diplomacy.factions.len(),
        b.diplomacy.factions.len(),
        "faction count"
    );
    assert_eq!(
        a.diplomacy.next_faction_id, b.diplomacy.next_faction_id,
        "next_faction_id"
    );
    for (af, bf) in a.diplomacy.factions.iter().zip(b.diplomacy.factions.iter()) {
        assert_eq!(af.id, bf.id, "faction id");
        assert_eq!(af.name, bf.name, "faction name");
        assert_eq!(af.leader, bf.leader, "faction leader");
        assert_eq!(af.members, bf.members, "faction members");
        assert_eq!(
            af.created_at_hour, bf.created_at_hour,
            "faction created_at_hour"
        );
    }

    assert_eq!(
        a.diplomacy.next_war_id, b.diplomacy.next_war_id,
        "next_war_id"
    );
    assert_eq!(a.diplomacy.wars.len(), b.diplomacy.wars.len(), "war count");
    for (id, aw) in &a.diplomacy.wars {
        let bw = b.diplomacy.wars.get(id).expect("war id roundtrips");
        assert_eq!(aw.id, bw.id, "war id");
        assert_eq!(
            aw.primary_attacker, bw.primary_attacker,
            "war primary_attacker"
        );
        assert_eq!(
            aw.primary_defender, bw.primary_defender,
            "war primary_defender"
        );
        assert_eq!(aw.attackers, bw.attackers, "war attackers");
        assert_eq!(aw.defenders, bw.defenders, "war defenders");
        assert_eq!(
            aw.started_at_hour, bw.started_at_hour,
            "war started_at_hour"
        );
        assert_eq!(
            aw.attacker_war_score, bw.attacker_war_score,
            "war attacker score"
        );
        assert_eq!(
            aw.defender_war_score, bw.defender_war_score,
            "war defender score"
        );
        assert_wargoals_eq(
            &aw.attacker_wargoals,
            &bw.attacker_wargoals,
            "war attacker goals",
        );
        assert_wargoals_eq(
            &aw.defender_wargoals,
            &bw.defender_wargoals,
            "war defender goals",
        );
    }

    assert_eq!(
        a.diplomacy.autonomy.len(),
        b.diplomacy.autonomy.len(),
        "autonomy count"
    );
    for (subject, aa) in &a.diplomacy.autonomy {
        let ba = b
            .diplomacy
            .autonomy
            .get(subject)
            .expect("autonomy subject roundtrips");
        assert_eq!(aa.master, ba.master, "autonomy master");
        assert_eq!(aa.subject, ba.subject, "autonomy subject");
        assert_eq!(aa.level, ba.level, "autonomy level");
        assert_eq!(aa.progress, ba.progress, "autonomy progress");
        assert_eq!(aa.since_hour, ba.since_hour, "autonomy since_hour");
    }

    assert_eq!(
        a.diplomacy.pending_wargoals.len(),
        b.diplomacy.pending_wargoals.len(),
        "pending wargoal owner count"
    );
    for (claimant, goals) in &a.diplomacy.pending_wargoals {
        assert_wargoals_eq(
            goals,
            b.diplomacy
                .pending_wargoals
                .get(claimant)
                .expect("pending claimant roundtrips"),
            "pending wargoals",
        );
    }

    assert_eq!(
        a.diplomacy.opinions.opinions, b.diplomacy.opinions.opinions,
        "opinions"
    );
    assert_eq!(
        a.diplomacy.world_tension, b.diplomacy.world_tension,
        "world_tension"
    );
    assert_eq!(
        a.diplomacy.annexed_countries, b.diplomacy.annexed_countries,
        "annexed countries"
    );
    assert_eq!(
        a.diplomacy.military_access, b.diplomacy.military_access,
        "military access"
    );
    assert_eq!(
        a.diplomacy.next_treaty_id, b.diplomacy.next_treaty_id,
        "next_treaty_id"
    );
    assert_eq!(
        a.diplomacy.treaties.len(),
        b.diplomacy.treaties.len(),
        "treaty count"
    );
    for (at, bt) in a.diplomacy.treaties.iter().zip(b.diplomacy.treaties.iter()) {
        assert_eq!(at.id, bt.id, "treaty id");
        assert_eq!(at.kind, bt.kind, "treaty kind");
        assert_eq!(at.parties, bt.parties, "treaty parties");
        assert_eq!(at.since_hour, bt.since_hour, "treaty since_hour");
        assert_eq!(
            at.expires_at_hour, bt.expires_at_hour,
            "treaty expires_at_hour"
        );
    }
    assert_eq!(
        a.diplomacy.next_diplomatic_request_id, b.diplomacy.next_diplomatic_request_id,
        "next_diplomatic_request_id"
    );
    assert_eq!(
        a.diplomacy.diplomatic_requests, b.diplomacy.diplomatic_requests,
        "diplomatic requests"
    );
    assert!(
        a.diplomacy
            .treaties
            .iter()
            .any(|t| t.kind == TreatyKind::MilitaryAccess),
        "military access treaty should be present"
    );
}

fn assert_wargoals_eq(a: &[Wargoal], b: &[Wargoal], label: &str) {
    assert_eq!(a.len(), b.len(), "{} len", label);
    for (ag, bg) in a.iter().zip(b.iter()) {
        assert_eq!(ag.claimant, bg.claimant, "{} claimant", label);
        assert_eq!(ag.target, bg.target, "{} target", label);
        assert_eq!(ag.kind, bg.kind, "{} kind", label);
        assert_eq!(ag.target_state, bg.target_state, "{} target_state", label);
        assert_eq!(ag.justified, bg.justified, "{} justified", label);
        assert_eq!(
            ag.justify_progress, bg.justify_progress,
            "{} progress",
            label
        );
        assert_eq!(
            ag.justify_total_days, bg.justify_total_days,
            "{} total",
            label
        );
    }
}

#[test]
fn test_text_save_roundtrip() {
    let mut original = build_world();
    mutate_world(&mut original);

    // 序列化
    let saved = save::text::write_string(&original);
    assert!(
        saved.starts_with("HOI4txt"),
        "text save must start with HOI4txt magic"
    );

    // 元数据应能独立解析
    let meta = save::text::read_meta_str(&saved).expect("read_meta_str");
    assert_eq!(meta.kind, SaveKind::Text);
    assert_eq!(meta.player_tag, "GER");
    assert_eq!(meta.random_seed, original.random_seed);
    assert_eq!(meta.game_unique_id, original.game_unique_id);
    assert_eq!(meta.date, original.date);
    assert_eq!(meta.elapsed_hours, original.elapsed_hours);

    // 反序列化到新 world
    let mut loaded = build_world();
    save::text::read_str(&saved, &mut loaded).expect("read_str");

    assert_world_equiv(&original, &loaded);
}

#[test]
fn test_binary_save_roundtrip() {
    let mut original = build_world();
    mutate_world(&mut original);

    // 序列化
    let bytes = save::binary::write_bytes(&original);
    assert!(
        bytes.starts_with(b"IRHRT001"),
        "binary save must start with IRHRT001 magic"
    );

    // 元数据
    let meta = save::binary::read_meta_bytes(&bytes).expect("binary read_meta");
    assert_eq!(meta.kind, SaveKind::Binary);
    assert_eq!(meta.random_seed, original.random_seed);
    assert_eq!(meta.game_unique_id, original.game_unique_id);
    assert_eq!(meta.date, original.date);
    assert_eq!(meta.elapsed_hours, original.elapsed_hours);

    // 反序列化
    let mut loaded = build_world();
    save::binary::read_bytes(&bytes, &mut loaded).expect("binary read_bytes");

    assert_world_equiv(&original, &loaded);
}

#[test]
fn test_binary_save_compactness() {
    // 二进制存档应远小于文本存档
    let mut world = build_world();
    mutate_world(&mut world);

    let txt = save::text::write_string(&world);
    let bin = save::binary::write_bytes(&world);

    println!(
        "save sizes: text={} bytes ({:.1} KB), binary={} bytes ({:.1} KB), ratio={:.2}x",
        txt.len(),
        txt.len() as f64 / 1024.0,
        bin.len(),
        bin.len() as f64 / 1024.0,
        txt.len() as f64 / bin.len() as f64
    );
    assert!(
        bin.len() < txt.len(),
        "binary save ({} bytes) should be smaller than text ({} bytes)",
        bin.len(),
        txt.len()
    );
}

#[test]
fn test_auto_dispatch() {
    use std::env;
    use std::fs;

    let mut original = build_world();
    mutate_world(&mut original);

    let tmp_dir = env::temp_dir();
    let txt_path = tmp_dir.join("hoi4_test_save.txt");
    let bin_path = tmp_dir.join("hoi4_test_save.bin");

    save::write(&txt_path, &original).expect("write text");
    save::write(&bin_path, &original).expect("write binary");

    // 自动检测：txt 文件按文本读
    let mut w_txt = build_world();
    save::read(&txt_path, &mut w_txt).expect("auto read text");
    assert_world_equiv(&original, &w_txt);

    // 自动检测：bin 文件按二进制读
    let mut w_bin = build_world();
    save::read(&bin_path, &mut w_bin).expect("auto read binary");
    assert_world_equiv(&original, &w_bin);

    // 元数据
    let m_txt = save::read_meta(&txt_path).unwrap();
    assert_eq!(m_txt.kind, SaveKind::Text);
    let m_bin = save::read_meta(&bin_path).unwrap();
    assert_eq!(m_bin.kind, SaveKind::Binary);

    let _ = fs::remove_file(&txt_path);
    let _ = fs::remove_file(&bin_path);
}

#[test]
fn test_text_save_human_readable() {
    let mut world = build_world();
    mutate_world(&mut world);
    let saved = save::text::write_string(&world);

    // 关键标记应出现
    assert!(saved.contains("HOI4txt"));
    assert!(saved.contains("date="));
    assert!(saved.contains("player=\"GER\""));
    assert!(saved.contains("states={"));
    assert!(saved.contains("countries={"));
    assert!(saved.contains("GER={"));
    assert!(saved.contains("ruling_party="));
    assert!(saved.contains("province_overrides={"));
}

#[test]
fn test_bad_magic_rejected() {
    let mut world = build_world();
    let bad_text = "NOTAGOODSAVE\nfoo=1";
    let err = save::text::read_str(bad_text, &mut world);
    assert!(
        err.is_err(),
        "should reject text save without HOI4txt magic"
    );

    let bad_bin = vec![0u8; 16];
    let err = save::binary::read_bytes(&bad_bin, &mut world);
    assert!(err.is_err(), "should reject binary save with wrong magic");
}

#[test]
fn test_partial_text_save_loads() {
    // 用户也可能写一份 minimal 存档（只有元数据，没有 states）
    let minimal = "HOI4txt\n\
        version=\"hand-rolled\"\n\
        date=1940.6.22.4\n\
        player=\"SOV\"\n\
        ironman=no\n\
        random_seed=7777\n\
        game_unique_id=1\n\
        elapsed_hours=39000\n\
        speed=2\n";

    let mut world = build_world();
    save::text::read_str(minimal, &mut world).expect("minimal save loads");

    assert_eq!(
        world.date,
        GameDate {
            year: 1940,
            month: 6,
            day: 22,
            hour: 4
        }
    );
    assert_eq!(world.elapsed_hours, 39000);
    assert_eq!(world.random_seed, 7777);
    assert_eq!(world.game_unique_id, 1);
    assert_eq!(world.speed, GameSpeed::Speed2);
    assert_eq!(world.country_tag(world.player), Some("SOV"));
}

#[test]
fn test_unknown_player_tag_becomes_none() {
    let s = "HOI4txt\n\
        version=\"x\"\n\
        date=1936.1.1.0\n\
        player=\"ZZZ_NOT_REAL\"\n\
        random_seed=0\n\
        game_unique_id=0\n\
        elapsed_hours=0\n\
        speed=0\n";
    let mut world = build_world();
    save::text::read_str(s, &mut world).expect("loads with unknown player");
    assert!(
        world.player.is_none(),
        "unknown player tag should map to NONE"
    );
}

#[test]
fn test_province_override_only_for_changes() {
    // 没有改任何 province：override 段应为空
    let world = build_world();
    let saved = save::text::write_string(&world);
    let after_overrides = saved
        .split_once("province_overrides={")
        .expect("section exists")
        .1;
    let block = after_overrides.split_once("}\n").unwrap().0;
    let trimmed = block.trim();
    assert!(
        trimmed.is_empty(),
        "expected empty province_overrides for fresh world, got:\n{}",
        block
    );
}

#[test]
fn test_state_owner_change_propagates_to_provinces() {
    // 模拟"史实剧本"：苏联兼并波兰东部，把波兰一个州的 owner 改成 SOV
    let mut original = build_world();
    let pol = original.country("POL").unwrap();
    let sov = original.country("SOV").unwrap();

    // 找到第一个属于 POL 的 state
    let pol_state_idx = (0..original.states.count)
        .find(|&i| original.states.owners[i] == pol)
        .expect("POL has at least one state");

    original.states.owners[pol_state_idx] = sov;
    // 同时也要更新该 state 的所有省份 owner
    for &p in &original.states.provinces[pol_state_idx].clone() {
        original.provinces.owners[p.0 as usize] = sov;
    }

    let saved = save::text::write_string(&original);
    let mut loaded = build_world();
    save::text::read_str(&saved, &mut loaded).unwrap();

    assert_eq!(
        loaded.states.owners[pol_state_idx], sov,
        "state owner should be SOV after load"
    );
    // 省份 owner 也要跟着州走（reader 自动同步）
    for &p in &loaded.states.provinces[pol_state_idx] {
        assert_eq!(
            loaded.province_owner(p),
            sov,
            "province {} should be owned by SOV after load",
            p.0
        );
    }
}

#[test]
fn save_roundtrip_state_integration() {
    let mut original = build_world();
    let ger = original.country("GER").unwrap();
    let state_idx = (0..original.states.count)
        .find(|&i| original.states.owners[i] == ger)
        .expect("GER owns states");
    original.states.integration_status[state_idx] = StateIntegrationStatus::Protectorate;

    let saved = save::text::write_string(&original);
    assert!(
        saved.contains("integration=\"protectorate\""),
        "text save should include state integration status"
    );

    let mut text_loaded = build_world();
    save::text::read_str(&saved, &mut text_loaded).unwrap();
    assert_eq!(
        text_loaded.states.integration_status[state_idx],
        StateIntegrationStatus::Protectorate,
        "text save should roundtrip state integration"
    );

    let bytes = save::binary::write_bytes(&original);
    let mut binary_loaded = build_world();
    save::binary::read_bytes(&bytes, &mut binary_loaded).unwrap();
    assert_eq!(
        binary_loaded.states.integration_status[state_idx],
        StateIntegrationStatus::Protectorate,
        "binary save should roundtrip state integration"
    );
}

#[test]
fn test_country_cache_recomputed_after_load() {
    // 修改 V6 建筑后保存再读取，国家工业统计应该重新累加
    let mut original = build_world();
    let usa = original.country("USA").unwrap();
    let (civ_before, _, _) = original.country_industry(usa);

    // 找一个 USA 拥有的 state，加 5 个工厂
    let usa_state = (0..original.states.count)
        .find(|&i| original.states.owners[i] == usa)
        .expect("USA owns states");
    original.countries.buildings_v6.buildings.push(Building {
        kind: BuildingKind::Industrial,
        building_def_id: "steel_mill".to_owned(),
        state: StateId(usa_state as u16),
        level: 5,
        active_pm: "default".to_owned(),
        employment: [0; 6],
        owner: BuildingOwner::State,
        requires_law: None,
        built_progress: 1.0,
        ..Building::runtime_defaults()
    });

    let saved = save::text::write_string(&original);
    let mut loaded = build_world();
    save::text::read_str(&saved, &mut loaded).unwrap();

    let (civ_after, _, _) = loaded.country_industry(loaded.country("USA").unwrap());
    assert_eq!(
        civ_after,
        civ_before + 5,
        "USA V6 industry should reflect +5 (before={}, after={})",
        civ_before,
        civ_after
    );
}

#[test]
fn test_text_save_idempotent() {
    // save -> load -> save 应得到完全相同的字符串
    let mut original = build_world();
    mutate_world(&mut original);

    let s1 = save::text::write_string(&original);

    let mut loaded = build_world();
    save::text::read_str(&s1, &mut loaded).unwrap();

    let s2 = save::text::write_string(&loaded);

    assert_eq!(
        s1, s2,
        "text save should be idempotent under save->load->save"
    );
}

#[test]
fn test_binary_save_idempotent() {
    let mut original = build_world();
    mutate_world(&mut original);

    let b1 = save::binary::write_bytes(&original);

    let mut loaded = build_world();
    save::binary::read_bytes(&b1, &mut loaded).unwrap();

    let b2 = save::binary::write_bytes(&loaded);

    assert_eq!(
        b1, b2,
        "binary save should be idempotent under save->load->save"
    );
}

// 防止 unused warnings on imports the test doesn't reach if HOI4 path is missing
#[allow(dead_code)]
fn _suppress(_p: ProvinceId, _c: CountryId) {}

// ─── 任务 2.4 单元测试：旧档兼容 + 越界成员 + 非法路径 ───

#[test]
fn test_old_text_save_no_armies() {
    // R10.3：不含 armies={…} 的旧存档读后 player_armies.is_empty()
    let mut world = build_world();
    let ger = world.country("GER").unwrap();
    world.player = ger;

    let old_save = format!(
        "HOI4txt\n\
         version=\"Ironheart 0.1.0\"\n\
         date=1936.1.1.12\n\
         player=\"GER\"\n\
         ironman=no\n\
         random_seed=0\n\
         game_unique_id=0\n\
         elapsed_hours=0\n\
         speed=0\n\
         countries={{\n\
         \tGER={{\n\
         \t\tpolitical_power=0.0\n\
         \t\tstability=0.5\n\
         \t\twar_support=0.5\n\
         \t\truling_party=\"democratic\"\n\
         \t\tmanpower=0\n\
         \t\tfuel=0.0\n\
         \t\tfuel_capacity=1000.0\n\
         \t\tarmy_xp=0.0\n\
         \t\tnavy_xp=0.0\n\
         \t\tair_xp=0.0\n\
         \t\tat_war=no\n\
         \t\tcompleted_techs={{ }}\n\
         \t\tideas={{ }}\n\
         \t}}\n\
         }}\n"
    );

    save::text::read_str(&old_save, &mut world).expect("old text save loads");
    assert!(
        world.player_armies.is_empty(),
        "old save should have no armies"
    );
    assert_eq!(world.next_army_id, 0, "next_army_id should be 0");
}

#[test]
fn test_out_of_range_members_discarded() {
    // R10.4：含越界成员的 armies 读后该成员被丢且仍含其它
    let mut world = build_world();
    let div_count = world.divisions.count;

    let valid_idx = if div_count > 0 { 0usize } else { 9999usize };
    let oob_idx = div_count + 42;

    let save_with_oob = format!(
        "HOI4txt\n\
         version=\"Ironheart 0.1.0\"\n\
         date=1936.1.1.12\n\
         player=\"GER\"\n\
         ironman=no\n\
         random_seed=0\n\
         game_unique_id=0\n\
         elapsed_hours=0\n\
         speed=0\n\
         countries={{\n\
         \tGER={{\n\
         \t\tpolitical_power=0.0\n\
         \t\tstability=0.5\n\
         \t\twar_support=0.5\n\
         \t\truling_party=\"democratic\"\n\
         \t\tmanpower=0\n\
         \t\tfuel=0.0\n\
         \t\tfuel_capacity=1000.0\n\
         \t\tarmy_xp=0.0\n\
         \t\tnavy_xp=0.0\n\
         \t\tair_xp=0.0\n\
         \t\tat_war=no\n\
         \t\tcompleted_techs={{ }}\n\
         \t\tideas={{ }}\n\
         \t\tarmies={{\n\
         \t\t\t100={{\n\
         \t\t\t\tname=\"Test Army\"\n\
         \t\t\t\tmembers={{ {valid} {oob} }}\n\
         \t\t\t}}\n\
         \t\t}}\n\
         \t}}\n\
         }}\n",
        valid = valid_idx,
        oob = oob_idx,
    );

    save::text::read_str(&save_with_oob, &mut world).expect("save with OOB members loads");
    assert_eq!(world.player_armies.len(), 1, "should have 1 army");
    let army = &world.player_armies[0];
    assert_eq!(army.id, ArmyId(100));

    if div_count > 0 {
        assert_eq!(
            army.members,
            vec![valid_idx],
            "OOB member should be discarded, valid kept"
        );
    } else {
        assert!(army.members.is_empty(), "all members OOB when div_count=0");
    }
}

#[test]
fn test_invalid_path_order_reset() {
    // R12.3：含不相邻 path 的 armies 读后 order==None && PlayerArmy 仍在
    let mut world = build_world();

    // 用两个绝对不相邻的省 id（0 和 65534）
    let save_with_bad_path = format!(
        "HOI4txt\n\
         version=\"Ironheart 0.1.0\"\n\
         date=1936.1.1.12\n\
         player=\"GER\"\n\
         ironman=no\n\
         random_seed=0\n\
         game_unique_id=0\n\
         elapsed_hours=0\n\
         speed=0\n\
         countries={{\n\
         \tGER={{\n\
         \t\tpolitical_power=0.0\n\
         \t\tstability=0.5\n\
         \t\twar_support=0.5\n\
         \t\truling_party=\"democratic\"\n\
         \t\tmanpower=0\n\
         \t\tfuel=0.0\n\
         \t\tfuel_capacity=1000.0\n\
         \t\tarmy_xp=0.0\n\
         \t\tnavy_xp=0.0\n\
         \t\tair_xp=0.0\n\
         \t\tat_war=no\n\
         \t\tcompleted_techs={{ }}\n\
         \t\tideas={{ }}\n\
         \t\tarmies={{\n\
         \t\t\t200={{\n\
         \t\t\t\tname=\"Bad Path Army\"\n\
         \t\t\t\tmembers={{ }}\n\
         \t\t\t\tpath={{ 0 65534 }}\n\
         \t\t\t\tactive=yes\n\
         \t\t\t}}\n\
         \t\t}}\n\
         \t}}\n\
         }}\n"
    );

    save::text::read_str(&save_with_bad_path, &mut world).expect("save with bad path loads");
    assert_eq!(world.player_armies.len(), 1, "army should still exist");
    let army = &world.player_armies[0];
    assert_eq!(army.id, ArmyId(200));
    assert_eq!(army.name, "Bad Path Army");
    assert!(
        army.order.is_none(),
        "order should be None after invalid path"
    );
}

#[test]
fn test_old_binary_save_no_armies() {
    // R10.3（二进制版）：无 FRARM chunk 的旧二进制存档读后 player_armies.is_empty()
    let mut world = build_world();
    let ger = world.country("GER").unwrap();
    world.player = ger;
    world.random_seed = 123;
    world.game_unique_id = 456;

    // 先写一份完整二进制存档（含 armies），然后手动裁掉 FRARM chunk
    // 简化方案：直接确保 player_armies 为空时二进制不写 FRARM chunk，
    // 再读回来。
    world.player_armies.clear();
    world.next_army_id = 0;

    let bytes = save::binary::write_bytes(&world);
    let mut loaded = build_world();
    save::binary::read_bytes(&bytes, &mut loaded).expect("binary read back");

    assert!(
        loaded.player_armies.is_empty(),
        "no-army binary save should have no armies"
    );
    assert_eq!(loaded.next_army_id, 0, "next_army_id should be 0");
}
