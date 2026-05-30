//! 政治系统集成测试 — 加载 1936 真实数据，验证 PP / 国策 / 国家精神 / 意识形态。

use std::sync::Arc;

use hoi4_data::GameData;
use hoi4_logic::politics::{self, PoliticsCache, PoliticsError};
use hoi4_map::GameMap;
use hoi4_state::{CountryId, World};

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
#[ignore = "V5: vanilla focus loading stubbed"]
fn test_political_data_loaded() {
    let (_world, data) = build_world();

    // 4 种基础意识形态
    println!(
        "Ideologies: {:?}",
        data.ideologies.keys().collect::<Vec<_>>()
    );
    for k in ["democratic", "communism", "fascism", "neutrality"] {
        assert!(data.ideologies.contains_key(k), "ideology `{}` missing", k);
    }

    // 应有大量 idea
    println!("Total ideas loaded: {}", data.ideas.len());
    assert!(data.ideas.len() > 100, "should load 100+ ideas");

    // 应有大量 focus tree（76 个国家树）
    println!("Total focus trees: {}", data.focus_trees.len());
    assert!(data.focus_trees.len() > 50, "should load 50+ trees");

    // generic tree 应有 political_effort focus
    let generic = data
        .focus_trees
        .get("generic_focus")
        .expect("generic_focus tree exists");
    assert!(
        generic.focuses.contains_key("political_effort"),
        "political_effort focus exists in generic tree"
    );
    let pe = &generic.focuses["political_effort"];
    assert_eq!(pe.cost_weeks as i32, 10, "cost_weeks=10");
    assert_eq!(pe.cost_days(), 70.0, "70-day completion");
    assert!(
        pe.completion_reward.is_some(),
        "completion_reward block exists"
    );

    // 反向索引
    assert_eq!(
        data.focus_to_tree.get("political_effort"),
        Some(&"generic_focus".to_owned())
    );
}

#[test]
fn test_pp_daily_gain() {
    let (mut world, data) = build_world();
    let cache = PoliticsCache::new(world.countries.count);

    let ger = world.country("GER").unwrap();
    let i = ger.0 as usize;
    let initial_pp = world.countries.political_power[i];

    politics::tick_daily(&mut world, &data, &cache);
    let after_one_day = world.countries.political_power[i];

    // 默认每日 +1
    assert!(
        (after_one_day - initial_pp - 1.0).abs() < 0.01,
        "PP gain after 1 day = {} (expected ~+1)",
        after_one_day - initial_pp
    );

    // 30 天累计
    for _ in 0..30 {
        politics::tick_daily(&mut world, &data, &cache);
    }
    let after_31_days = world.countries.political_power[i];
    assert!(
        (after_31_days - initial_pp - 31.0).abs() < 0.5,
        "PP after 31 days = {} (expected ~31)",
        after_31_days - initial_pp
    );
}

#[test]
#[ignore = "V5: vanilla focus loading stubbed"]
fn test_assign_focus_basic_validation() {
    let (mut world, data) = build_world();
    let ger = world.country("GER").unwrap();

    // 未知 focus
    let err = politics::assign_focus(&mut world, ger, "no_such_focus_xyz", &data).unwrap_err();
    assert!(matches!(err, PoliticsError::FocusNotInTree(_)), "{:?}", err);

    // 合法 focus（generic 树的 political_effort）
    let ok = politics::assign_focus(&mut world, ger, "political_effort", &data);
    assert!(
        ok.is_ok(),
        "GER should be able to start political_effort: {:?}",
        ok
    );
    assert_eq!(
        world.countries.current_focus[ger.0 as usize].as_deref(),
        Some("political_effort")
    );
    assert_eq!(world.countries.focus_progress[ger.0 as usize], 0.0);
}

#[test]
#[ignore = "V5: vanilla focus loading stubbed"]
fn test_cannot_redo_completed_focus() {
    let (mut world, data) = build_world();
    let ger = world.country("GER").unwrap();
    let i = ger.0 as usize;

    // 模拟已完成
    world.countries.completed_focuses[i].insert("political_effort".to_owned());

    let err = politics::assign_focus(&mut world, ger, "political_effort", &data).unwrap_err();
    assert!(
        matches!(err, PoliticsError::AlreadyCompleted(_)),
        "expected AlreadyCompleted, got {:?}",
        err
    );
}

#[test]
#[ignore = "V5: vanilla focus loading stubbed"]
fn test_focus_completion_runs_effects() {
    let (mut world, data) = build_world();
    let cache = PoliticsCache::new(world.countries.count);
    let ger = world.country("GER").unwrap();
    let i = ger.0 as usize;

    let initial_pp = world.countries.political_power[i];

    politics::assign_focus(&mut world, ger, "political_effort", &data).unwrap();

    // 跑 70+ 天（每天 +1 day progress + 每天 +1 PP base）
    for _ in 0..72 {
        politics::tick_daily(&mut world, &data, &cache);
    }

    // 应已完成
    assert!(
        world.countries.completed_focuses[i].contains("political_effort"),
        "political_effort should be in completed_focuses"
    );
    assert_eq!(world.countries.current_focus[i], None);

    // completion_reward 是 add_political_power = 120
    // PP = initial + ~72 (daily base) + 120 (reward) = initial + 192
    let final_pp = world.countries.political_power[i];
    let delta = final_pp - initial_pp;
    println!(
        "GER PP after political_effort: {} → {} (Δ={})",
        initial_pp, final_pp, delta
    );
    // PP 在 tick 中被 cap 到 1000，所以容许更大下界
    assert!(
        delta > 150.0,
        "PP should grow by 70 (daily) + 120 (reward) - already-clamped, got Δ={}",
        delta
    );
}

#[test]
fn test_idea_modifier_aggregation() {
    let (mut world, data) = build_world();
    let mut cache = PoliticsCache::new(world.countries.count);

    let ger = world.country("GER").unwrap();
    let i = ger.0 as usize;

    // 找一个 GER 已有的、且影响 PP 的 idea；如果没有就跳过
    // 通常 GER 起始 ideas 列表是空的（CountryStore::new 默认）；让我们手动加一个
    // triumphant_will idea: political_power_gain=1
    if data.ideas.contains_key("triumphant_will") {
        politics::add_idea(&mut world, ger, "triumphant_will", &data).unwrap();
        politics::recompute_modifiers(&world, &data, &mut cache);
        let bonus = cache.pp_gain_flat[i];
        assert!(
            (bonus - 1.0).abs() < 1e-3,
            "triumphant_will should give +1 PP gain, got {}",
            bonus
        );

        // 之后每日 PP +2（base 1 + flat 1）
        let pp_before = world.countries.political_power[i];
        politics::tick_daily(&mut world, &data, &cache);
        let pp_after = world.countries.political_power[i];
        assert!(
            (pp_after - pp_before - 2.0).abs() < 0.01,
            "with triumphant_will daily PP = {} (expected 2.0)",
            pp_after - pp_before
        );

        // 移除 idea 后 PP gain 复原
        politics::remove_idea(&mut world, ger, "triumphant_will");
        politics::recompute_modifiers(&world, &data, &mut cache);
        assert!(
            (cache.pp_gain_flat[i]).abs() < 1e-3,
            "after removal pp_gain_flat should be 0"
        );
    } else {
        println!("Skipping idea modifier test (triumphant_will not in vanilla data)");
    }
}

#[test]
fn test_add_remove_popularity() {
    let (mut world, _data) = build_world();
    let ger = world.country("GER").unwrap();
    let i = ger.0 as usize;

    politics::add_popularity(&mut world, ger, "fascism", 0.3);
    politics::add_popularity(&mut world, ger, "communism", 0.1);

    let pp = &world.countries.party_popularity[i];
    assert!((pp.get("fascism").copied().unwrap_or(0.0) - 0.3).abs() < 1e-6);
    assert!((pp.get("communism").copied().unwrap_or(0.0) - 0.1).abs() < 1e-6);

    // 上限 clamp
    politics::add_popularity(&mut world, ger, "fascism", 1.0);
    assert!((world.countries.party_popularity[i]["fascism"] - 1.0).abs() < 1e-6);
    // 下限 clamp
    politics::add_popularity(&mut world, ger, "fascism", -2.0);
    assert!((world.countries.party_popularity[i]["fascism"]).abs() < 1e-6);
}

#[test]
#[ignore = "V5: vanilla focus loading stubbed"]
fn test_focus_speed_modifier_from_idea() {
    let (mut world, data) = build_world();
    let mut cache = PoliticsCache::new(world.countries.count);

    let ger = world.country("GER").unwrap();
    let i = ger.0 as usize;

    politics::assign_focus(&mut world, ger, "political_effort", &data).unwrap();

    // 直接注入一个虚拟"焦点速度+100%"修饰：把 cache 改写后 70 天应在 35 天完成
    cache.focus_speed_factor[i] = 1.0; // +100%

    let mut day = 0;
    let mut completed_at = None;
    while day < 100 {
        politics::tick_daily(&mut world, &data, &cache);
        day += 1;
        if world.countries.current_focus[i].is_none()
            && world.countries.completed_focuses[i].contains("political_effort")
        {
            completed_at = Some(day);
            break;
        }
    }
    let completed_at = completed_at.expect("should complete within 100 days");
    println!("With +100% focus speed completed on day {}", completed_at);
    assert!(
        completed_at >= 33 && completed_at <= 38,
        "with +100% speed, 70-day focus should finish ~35d (got {})",
        completed_at
    );
}

#[test]
#[ignore = "V5: vanilla focus loading stubbed"]
fn test_germany_industrial_focus_grants_effects() {
    // 验证德国的国策完成会触发 effect。我们选 GER_lower_taxes（generic 风格的简单 effect）
    let (mut world, data) = build_world();
    let cache = PoliticsCache::new(world.countries.count);
    let ger = world.country("GER").unwrap();
    let i = ger.0 as usize;

    let germany_tree = data
        .focus_trees
        .values()
        .find(|t| t.country_tag.as_deref() == Some("GER"))
        .expect("GER focus tree should exist");
    println!(
        "GER tree id={}, focus count={}",
        germany_tree.id,
        germany_tree.focuses.len()
    );
    assert!(
        germany_tree.focuses.len() > 50,
        "GER should have many focuses"
    );

    // 选一个无前置且 cost 较低的 GER 焦点。
    // GER_prioritize_economic_growth (cost=5) 是分支的 root（没有 prereq）。
    let target = "GER_prioritize_economic_growth";
    if !germany_tree.focuses.contains_key(target) {
        println!("{} not found, test skipped", target);
        return;
    }

    let initial_stability = world.countries.stability[i];
    let initial_pp = world.countries.political_power[i];

    politics::assign_focus(&mut world, ger, target, &data).unwrap();

    // 5 cost × 7 = 35 天即可完成
    for _ in 0..40 {
        politics::tick_daily(&mut world, &data, &cache);
    }

    assert!(
        world.countries.completed_focuses[i].contains(target),
        "{} should be completed",
        target
    );

    // 验证 effect 被运行：stability -0.1 + add_ideas civilian_economy + add_popularity ROOT -0.1
    println!(
        "GER stability: {} → {}",
        initial_stability, world.countries.stability[i]
    );
    println!(
        "GER pp: {} → {}",
        initial_pp, world.countries.political_power[i]
    );
    println!("GER ideas now: {:?}", world.countries.ideas[i]);

    // stability 应下降 0.1（被 clamp 到 0..1）
    assert!(
        world.countries.stability[i] < initial_stability,
        "stability should decrease from {} (got {})",
        initial_stability,
        world.countries.stability[i]
    );

    // 应已加入 civilian_economy idea
    assert!(
        world.countries.ideas[i]
            .iter()
            .any(|id| id == "civilian_economy"),
        "civilian_economy idea should be added"
    );
}

#[test]
fn test_set_politics_changes_ruling_party() {
    use clausewitz_parser::parser::parse;
    let (mut world, data) = build_world();
    let ger = world.country("GER").unwrap();
    let i = ger.0 as usize;

    let original = world.countries.ruling_party[i].clone();
    println!("GER original ruling_party = {:?}", original);

    // 用 effect runner 直接执行一段构造的 set_politics 块
    let block = parse("set_politics = { ruling_party = communism }");
    politics::run_effect_block(&mut world, ger, &block, &data);

    assert_eq!(world.countries.ruling_party[i], "communism");
}

#[test]
#[ignore = "V5: vanilla focus loading stubbed"]
fn test_full_political_simulation_quarter_year() {
    // 综合：8 大国并行跑 90 天，验证 PP 累计 + 至少有一国能完成 1 个国策
    let (mut world, data) = build_world();
    let mut cache = PoliticsCache::new(world.countries.count);
    politics::recompute_modifiers(&world, &data, &mut cache);

    let majors = ["GER", "ENG", "FRA", "ITA", "USA", "SOV", "JAP", "CHI"];

    // 给每国分配 generic 树的 political_effort（70 天完成；快于 90 天）
    let mut assigned = Vec::new();
    for tag in &majors {
        let cid = world.country(tag).expect(tag);
        if politics::assign_focus(&mut world, cid, "political_effort", &data).is_ok() {
            assigned.push(cid);
        }
    }

    let initial_pp_sum: f32 = majors
        .iter()
        .map(|t| {
            let c = world.country(t).unwrap();
            world.countries.political_power[c.0 as usize]
        })
        .sum();

    for _ in 0..90 {
        politics::tick_daily(&mut world, &data, &cache);
    }

    let final_pp_sum: f32 = majors
        .iter()
        .map(|t| {
            let c = world.country(t).unwrap();
            world.countries.political_power[c.0 as usize]
        })
        .sum();

    println!("8 majors PP total: {} → {}", initial_pp_sum, final_pp_sum);
    assert!(
        final_pp_sum > initial_pp_sum,
        "PP should accumulate over 90 days for all majors"
    );

    let completed: Vec<&CountryId> = assigned
        .iter()
        .filter(|c| world.countries.completed_focuses[c.0 as usize].contains("political_effort"))
        .collect();
    println!(
        "Majors that completed political_effort: {} of {}",
        completed.len(),
        assigned.len()
    );
    assert!(
        !completed.is_empty(),
        "at least one major should complete political_effort in 90 days"
    );
}
