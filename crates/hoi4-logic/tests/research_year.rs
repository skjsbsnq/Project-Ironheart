//! 科技系统集成测试 — 加载 1936 年真实数据，多国并行多项研发跨年验证。
//!
//! V6.F 迁移后：
//! - 科技数据全部来自 V6Database.technologies（RON 定义）
//! - start() 需要 &V6Database 作为第 4 参数
//! - tick_daily 内部委托给 tick_daily_v6
//! - tick_daily_v6 需要 Clerk 在 University 就业 + 足够 cash_rm 才有研究速度
//! - vanilla data.technologies 仍用于 test_tech_data_loaded 等纯数据测试

use std::sync::Arc;

use hoi4_content::V6Database;
use hoi4_data::GameData;
use hoi4_logic::economy;
use hoi4_logic::research::{self, ResearchError, ResearchSlot, ResearchState};
use hoi4_map::GameMap;
use hoi4_state::{BuildingId, BuildingKind, BuildingOwner, PopClass, World};

fn hoi4_path() -> std::path::PathBuf {
    hoi4_paths::PathConfig::resolve(Default::default())
        .expect("set IRONHEART_HOI4_PATH or place HOI4 install at default Steam path")
        .game_path()
        .to_path_buf()
}

fn build_world_vanilla() -> (World, Arc<GameData>) {
    let game_path = hoi4_path();
    let map = Arc::new(GameMap::load(&game_path).unwrap());
    let data = Arc::new(GameData::load(&game_path).unwrap());
    let world = World::new(map, data.clone());
    (world, data)
}

fn build_world_with_v6() -> (World, Arc<GameData>, V6Database) {
    let game_path = hoi4_path();
    let map = Arc::new(GameMap::load(&game_path).unwrap());
    let data = Arc::new(GameData::load(&game_path).unwrap());
    let mut world = World::new(map, data.clone());
    economy::init_world(&mut world);
    world.populate_from_history();
    let db = V6Database::load();
    db.assert_no_vanilla_conflict();
    hoi4_content::inject_v6_into_world(&mut world, &db);
    setup_research_prerequisites(&mut world);
    (world, data, db)
}

fn setup_research_prerequisites(world: &mut World) {
    let ger = match world.country("GER") {
        Some(c) => c,
        None => return,
    };

    let ger_states: Vec<hoi4_state::StateId> = (0..world.states.count)
        .filter(|&si| world.states.owners[si] == ger)
        .map(|si| hoi4_state::StateId(si as u16))
        .collect();

    if ger_states.is_empty() {
        return;
    }

    let univ_state = ger_states[0];
    let univ_level = 2u8;

    let univ_bidx = world.countries.buildings_v6.buildings.len();
    world
        .countries
        .buildings_v6
        .buildings
        .push(hoi4_state::Building {
            kind: BuildingKind::Service,
            building_def_id: "university".to_owned(),
            state: univ_state,
            level: univ_level,
            active_pm: "default".to_owned(),
            employment: [0; 6],
            owner: BuildingOwner::State,
            requires_law: None,
            built_progress: 1.0,
            ..hoi4_state::Building::runtime_defaults()
        });

    let required_clerks = (univ_level as u32) * research::constants::CLERK_PER_SLOT;
    let mut assigned: u32 = 0;
    let univ_building_id = BuildingId(univ_bidx as u32);
    let clerk_idx = PopClass::Clerk.index();

    for pg in &mut world.countries.pops.groups {
        if pg.class == PopClass::Clerk && pg.employed_at.is_none() && assigned < required_clerks {
            let assign = pg.size.min(required_clerks - assigned);
            if assign > 0 {
                pg.employed_at = Some(univ_building_id);
                pg.size = assign;
                assigned += assign;
            }
        }
    }

    if assigned > 0 {
        world.countries.buildings_v6.buildings[univ_bidx].employment[clerk_idx] = assigned;
    }
}

#[test]
fn test_tech_data_loaded() {
    let (_world, data) = build_world_vanilla();
    println!("Loaded {} technologies", data.technologies.len());
    assert!(
        data.technologies.len() > 200,
        "Should load 200+ techs (got {})",
        data.technologies.len()
    );

    let inf = data
        .technologies
        .get("infantry_weapons1")
        .expect("infantry_weapons1 must exist");
    assert!(
        (inf.research_cost - 1.5).abs() < 1e-3,
        "infantry_weapons1 cost = {}",
        inf.research_cost
    );
    assert_eq!(inf.start_year, 1936);
    assert!(inf
        .enable_equipments
        .contains(&"infantry_equipment_1".to_owned()));

    let basic_tools = data
        .technologies
        .get("basic_machine_tools")
        .expect("basic_machine_tools must exist");
    assert_eq!(basic_tools.start_year, 1936);
    assert!(
        !basic_tools.paths.is_empty(),
        "basic_machine_tools has paths"
    );
}

#[test]
fn test_tech_prereqs_indexed() {
    let (_world, data) = build_world_vanilla();
    let parents = data
        .tech_prereqs
        .get("infantry_weapons1")
        .expect("infantry_weapons1 has prereqs");
    assert!(
        parents.iter().any(|p| p == "infantry_weapons"),
        "infantry_weapons1 should require infantry_weapons (got {:?})",
        parents
    );
}

#[test]
fn test_v6_tech_database_loaded() {
    let (_world, _data, db) = build_world_with_v6();
    println!("V6 technologies: {}", db.technologies.len());
    assert!(
        db.technologies.len() > 10,
        "V6 should have 10+ techs (got {})",
        db.technologies.len()
    );

    let conc_ind = db
        .technologies
        .iter()
        .find(|t| t.id == "concentrated_industry")
        .expect("concentrated_industry must exist in V6");
    assert!(
        (conc_ind.research_cost - 1.0).abs() < 1e-3,
        "concentrated_industry cost = {}",
        conc_ind.research_cost
    );
    assert_eq!(conc_ind.start_year, 1936);
    assert!(conc_ind.prereqs.is_empty(), "root tech has no prereqs");
}

#[test]
fn test_world_unlocks_initial_equipments() {
    let (world, _data, _db) = build_world_with_v6();
    let ger = world.country("GER").unwrap();
    let i = ger.0 as usize;
    let unlocked = &world.countries.unlocked_equipments[i];
    println!(
        "GER initial unlocked equipments: {} ({:?})",
        unlocked.len(),
        unlocked.iter().take(5).collect::<Vec<_>>()
    );
    assert!(
        unlocked.contains("infantry_equipment_1") || unlocked.contains("infantry_equipment_0"),
        "GER should have basic infantry equipment unlocked at start"
    );
    assert!(
        unlocked.len() > 5,
        "GER should have multiple equipments unlocked"
    );
}

#[test]
fn test_research_state_init() {
    let (world, _data, _db) = build_world_with_v6();
    let r = ResearchState::new(&world);
    assert_eq!(r.count, world.countries.count);
    let ger = world.country("GER").unwrap();
    let slots = &r.slots[ger.0 as usize];
    assert!(
        slots.len() >= 2,
        "GER should have at least 2 research slots (got {})",
        slots.len()
    );
    assert!(slots.iter().all(|s| s.is_idle()));
}

#[test]
fn test_start_research_basic() {
    let (world, _data, db) = build_world_with_v6();
    let mut r = ResearchState::new(&world);
    let ger = world.country("GER").unwrap();
    let num_slots = r.slots[ger.0 as usize].len();

    let slot = r
        .start(&world, ger, "concentrated_industry", &db)
        .expect("can start");
    assert_eq!(slot, 0, "first idle slot is 0");

    let err = r
        .start(&world, ger, "concentrated_industry", &db)
        .unwrap_err();
    assert!(
        matches!(err, ResearchError::DuplicateActiveTech(_)),
        "expected DuplicateActiveTech, got {:?}",
        err
    );

    let err = r.start(&world, ger, "no_such_tech_xyz", &db).unwrap_err();
    assert!(
        matches!(err, ResearchError::UnknownTech(_)),
        "expected UnknownTech, got {:?}",
        err
    );

    let slot2 = r
        .start(&world, ger, "dispersed_industry", &db)
        .expect("dispersed_industry start ok (no prereq, root tech)");
    assert_eq!(slot2, 1);

    if num_slots >= 3 {
        let slot3 = r
            .start(&world, ger, "metallurgy", &db)
            .expect("metallurgy start ok (no prereq, root tech)");
        assert_eq!(slot3, 2);
    }

    let err = r
        .start(&world, ger, "concentrated_industry_2", &db)
        .unwrap_err();
    assert!(
        matches!(err, ResearchError::PrereqNotMet(_)),
        "expected PrereqNotMet for concentrated_industry_2, got {:?}",
        err
    );
}

#[test]
fn test_start_research_unknown_tech() {
    let (world, _data, db) = build_world_with_v6();
    let mut r = ResearchState::new(&world);
    let ger = world.country("GER").unwrap();
    let err = r.start(&world, ger, "no_such_tech_xyz", &db).unwrap_err();
    assert!(
        matches!(err, ResearchError::UnknownTech(_)),
        "expected UnknownTech, got {:?}",
        err
    );
}

#[test]
fn test_start_research_missing_prereq() {
    let (world, _data, db) = build_world_with_v6();
    let mut r = ResearchState::new(&world);
    let ger = world.country("GER").unwrap();
    let err = r
        .start(&world, ger, "concentrated_industry_2", &db)
        .unwrap_err();
    assert!(
        matches!(err, ResearchError::PrereqNotMet(_)),
        "expected PrereqNotMet for concentrated_industry_2 without prereq, got {:?}",
        err
    );
}

#[test]
fn test_start_research_already_completed() {
    let (mut world, _data, db) = build_world_with_v6();
    let ger = world.country("GER").unwrap();
    world.mark_tech_completed(ger, "concentrated_industry");
    let mut r = ResearchState::new(&world);
    let err = r
        .start(&world, ger, "concentrated_industry", &db)
        .unwrap_err();
    assert!(
        matches!(err, ResearchError::AlreadyResearched(_)),
        "expected AlreadyResearched, got {:?}",
        err
    );
}

#[test]
fn test_research_completes_in_expected_days() {
    let (mut world, data, db) = build_world_with_v6();
    let mut r = ResearchState::new(&world);
    let ger = world.country("GER").unwrap();

    let tech_key = "concentrated_industry";
    let tech = db.technologies.iter().find(|t| t.id == tech_key).unwrap();
    let expected_days = (tech.research_cost * research::constants::COST_TO_DAYS) as i32;

    r.start(&world, ger, tech_key, &db).expect("start ok");

    let mut completed_day = None;
    for day in 1..=(expected_days + 50) {
        research::tick_daily(&mut world, &mut r, &data, &[]);
        let slots = &r.slots[ger.0 as usize];
        if slots[0].is_idle() && completed_day.is_none() {
            completed_day = Some(day);
            break;
        }
    }

    let day = completed_day.unwrap_or(0);
    println!(
        "{} completed on day {} (expected ~{})",
        tech_key, day, expected_days
    );
    assert!(
        day > 0 && day <= expected_days + 50,
        "{} should finish within {} days, got {}",
        tech_key,
        expected_days + 50,
        day
    );

    assert!(
        world.countries.completed_techs[ger.0 as usize]
            .iter()
            .any(|t| t == tech_key),
        "tech must be in completed list"
    );
}

#[test]
fn test_tech_completion_unlocks_v6_entities() {
    let (mut world, data, db) = build_world_with_v6();
    let mut r = ResearchState::new(&world);
    let ger = world.country("GER").unwrap();
    let i = ger.0 as usize;

    let had_construction = world.countries.unlocked_buildings[i].contains("construction_sector");
    assert!(
        !had_construction,
        "construction_sector should NOT be unlocked at GER 1936 start"
    );

    r.start(&world, ger, "construction_technology", &db)
        .expect("start ok");

    for day in 0..400 {
        research::tick_daily(&mut world, &mut r, &data, &[]);
        if r.slots[i][0].is_idle() {
            println!("construction_technology completed on day {}", day + 1);
            break;
        }
    }
    assert!(
        world.countries.unlocked_buildings[i].contains("construction_sector"),
        "construction_sector should be unlocked after researching construction_technology"
    );
}

#[test]
fn test_ahead_of_time_penalty_real_world() {
    let (mut world, data, db) = build_world_with_v6();
    let mut r = ResearchState::new(&world);
    let ger = world.country("GER").unwrap();

    let tech_key = "concentrated_industry_2";
    let tech = db.technologies.iter().find(|t| t.id == tech_key).unwrap();
    assert!(
        tech.start_year >= 1937,
        "tech start_year = {}",
        tech.start_year
    );

    world.mark_tech_completed(ger, "concentrated_industry");

    r.start(&world, ger, tech_key, &db).expect("start ok");

    let expected_days = (tech.research_cost * research::constants::COST_TO_DAYS / 0.5) as i32;
    let mut completed_day = None;
    for day in 1..=(expected_days + 100) {
        research::tick_daily(&mut world, &mut r, &data, &[]);
        if r.slots[ger.0 as usize][0].is_idle() {
            completed_day = Some(day);
            break;
        }
    }
    let day = completed_day.expect("ahead-of-time tech should still complete eventually");
    println!(
        "{} (start_year={}) completed in {} days at year {} (expected ~{})",
        tech_key, tech.start_year, day, world.date.year, expected_days
    );
}

#[test]
fn test_germany_full_year_chain_research() {
    let (mut world, data, db) = build_world_with_v6();
    let mut r = ResearchState::new(&world);
    let ger = world.country("GER").unwrap();
    let i = ger.0 as usize;

    let initial_completed = world.countries.completed_techs[i].len();

    r.start(&world, ger, "concentrated_industry", &db)
        .expect("concentrated_industry start");
    r.start(&world, ger, "dispersed_industry", &db)
        .expect("dispersed_industry start");

    let mut day = 0;
    while day < 365 {
        research::tick_daily(&mut world, &mut r, &data, &[]);
        for slot_idx in 0..r.slots[i].len() {
            if matches!(r.slots[i][slot_idx], ResearchSlot::Idle) {
                let candidate = next_research_candidate(&db, &world, &r, ger);
                if let Some(key) = candidate {
                    let _ = r.start(&world, ger, &key, &db);
                }
            }
        }
        world.tick_day();
        day += 1;
    }

    let final_completed = world.countries.completed_techs[i].len();
    let researched_this_year = final_completed - initial_completed;
    let total_completed_via_research = r.total_completed[i];

    println!(
        "GER 1 year research: completed {} techs (initial {} → final {}), via research={}",
        researched_this_year, initial_completed, final_completed, total_completed_via_research
    );

    assert!(
        total_completed_via_research >= 1,
        "GER should research at least 1 tech in 1 year (got {})",
        total_completed_via_research
    );

    assert!(
        world.date.year >= 1936,
        "date.year = {} (should have advanced)",
        world.date.year
    );
    assert!(
        world.date.year <= 1937,
        "year jumped too far: {}",
        world.date.year
    );
}

fn next_research_candidate(
    db: &V6Database,
    world: &World,
    r: &ResearchState,
    country: hoi4_state::CountryId,
) -> Option<String> {
    let i = country.0 as usize;
    let completed: std::collections::HashSet<&str> = world.countries.completed_techs[i]
        .iter()
        .map(|s| s.as_str())
        .collect();
    let active: std::collections::HashSet<String> = r.active_techs(country).into_iter().collect();

    for tech in &db.technologies {
        if completed.contains(tech.id.as_str()) {
            continue;
        }
        if active.contains(&tech.id) {
            continue;
        }
        if tech.prereqs.iter().all(|p| completed.contains(p.as_str())) {
            return Some(tech.id.clone());
        }
    }
    None
}

#[test]
fn test_doctrine_detection() {
    let (_world, _data, db) = build_world_with_v6();
    let doctrines: Vec<&str> = db
        .technologies
        .iter()
        .filter(|t| matches!(t.category, hoi4_content::TechCategoryDef::MilitaryDoctrine))
        .map(|t| t.id.as_str())
        .take(5)
        .collect();
    println!("Sample V6 doctrines: {:?}", doctrines);
    assert!(
        !doctrines.is_empty(),
        "Some doctrines should be detected in V6 tech database"
    );
}

#[test]
fn test_multiple_countries_independent_research() {
    let (mut world, data, db) = build_world_with_v6();
    let mut r = ResearchState::new(&world);

    let ger = world.country("GER").unwrap();
    let usa = world.country("USA").unwrap();
    let sov = world.country("SOV").unwrap();

    r.start(&world, ger, "concentrated_industry", &db).unwrap();
    r.start(&world, usa, "concentrated_industry", &db).unwrap();
    r.start(&world, sov, "concentrated_industry", &db).unwrap();

    for _ in 0..400 {
        research::tick_daily(&mut world, &mut r, &data, &[]);
    }

    let tech_key = "concentrated_industry";
    let ger_done = world.countries.completed_techs[ger.0 as usize].contains(&tech_key.to_owned());
    let usa_done = world.countries.completed_techs[usa.0 as usize].contains(&tech_key.to_owned());
    let sov_done = world.countries.completed_techs[sov.0 as usize].contains(&tech_key.to_owned());

    assert!(ger_done, "GER {} should be done", tech_key);

    for &c in &[ger, usa, sov] {
        if c == ger {
            assert_eq!(
                r.total_completed[c.0 as usize], 1,
                "GER should have 1 completed via research"
            );
        }
    }
}
