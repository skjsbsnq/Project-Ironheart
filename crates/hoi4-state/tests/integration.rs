use hoi4_data::GameData;
use hoi4_map::GameMap;
use hoi4_state::{GameDate, World};
use std::sync::Arc;

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
    World::new(map, data)
}

#[test]
fn test_world_initialization() {
    let world = build_world();

    // 时间从 1936.1.1.12 开始
    assert_eq!(world.date, GameDate::START);
    assert_eq!(world.elapsed_hours, 0);

    // 应该有 400+ 国家
    assert!(
        world.countries.count > 400,
        "Got {} countries",
        world.countries.count
    );

    // 应该有 1000+ states
    assert!(
        world.states.count > 1000,
        "Got {} states",
        world.states.count
    );

    // 省份数量等于地图定义数
    assert_eq!(world.provinces.count, world.map.definitions.len());

    println!("World stats:");
    println!("  Countries: {}", world.countries.count);
    println!("  States: {}", world.states.count);
    println!("  Provinces: {}", world.provinces.count);
}

#[test]
fn test_country_lookup() {
    let world = build_world();

    // 主要大国都应该存在
    let ger = world.country("GER").expect("GER exists");
    let sov = world.country("SOV").expect("SOV exists");
    let eng = world.country("ENG").expect("ENG exists");
    let usa = world.country("USA").expect("USA exists");

    // ID 应该不同
    assert_ne!(ger, sov);
    assert_ne!(eng, usa);

    // 反向查找
    assert_eq!(world.country_tag(ger), Some("GER"));
    assert_eq!(world.country_tag(sov), Some("SOV"));
}

#[test]
fn test_country_industry_cached() {
    let world = build_world();

    // 主要工业国的工厂总数应该正确累加
    let ger = world.country("GER").unwrap();
    let (civ, mil, dock) = world.country_industry(ger);
    println!("GER industry: {} civ / {} mil / {} dock", civ, mil, dock);
    assert!(civ > 20, "GER civ factories: {}", civ);
    assert!(mil > 10);

    let usa = world.country("USA").unwrap();
    let (civ, mil, dock) = world.country_industry(usa);
    println!("USA industry: {} civ / {} mil / {} dock", civ, mil, dock);
    assert!(civ > 100, "USA civ factories: {}", civ);

    let sov = world.country("SOV").unwrap();
    let (civ, mil, dock) = world.country_industry(sov);
    println!("SOV industry: {} civ / {} mil / {} dock", civ, mil, dock);
    assert!(civ > 30);
}

#[test]
fn test_country_manpower() {
    let world = build_world();

    let ger = world.country("GER").unwrap();
    let chi = world.country("CHI").unwrap();
    let sov = world.country("SOV").unwrap();

    let ger_mp = world.manpower(ger);
    let chi_mp = world.manpower(chi);
    let sov_mp = world.manpower(sov);

    println!("Manpower pool:");
    println!("  GER: {}", ger_mp);
    println!("  CHI: {}", chi_mp);
    println!("  SOV: {}", sov_mp);

    // 中国和苏联是人力大国
    assert!(chi_mp > 100_000_000, "CHI manpower: {}", chi_mp);
    assert!(sov_mp > 100_000_000, "SOV manpower: {}", sov_mp);
}

#[test]
fn test_province_ownership_set() {
    let world = build_world();

    // 检查 Berlin (province 6521) 属于德国
    let ger = world.country("GER").unwrap();
    let berlin_owner = world.province_owner(hoi4_state::ProvinceId(6521));
    assert_eq!(berlin_owner, ger, "Berlin (6521) should be owned by GER");

    // 海洋省份应该是 NONE
    for def in world.map.definitions.iter().flatten() {
        if def.province_type == hoi4_map::definition::ProvinceType::Sea {
            let owner = world.province_owner(hoi4_state::ProvinceId(def.id));
            assert!(
                owner.is_none(),
                "Sea province {} should have no owner",
                def.id
            );
            break;
        }
    }
}

#[test]
fn test_time_advancement() {
    let mut world = build_world();
    assert_eq!(world.elapsed_hours, 0);

    world.tick_hour();
    assert_eq!(world.elapsed_hours, 1);
    assert_eq!(world.date.hour, 13);

    world.tick_day();
    assert_eq!(world.elapsed_hours, 25);
    // 12 + 24 = 36 → 12:00 next day
    assert_eq!(world.date.hour, 13);
    assert_eq!(world.date.day, 2);

    // Run a full year
    for _ in 0..365 {
        world.tick_day();
    }
    assert_eq!(world.date.year, 1937);
}

#[test]
fn test_capitals() {
    let world = build_world();

    let ger = world.country("GER").unwrap();
    let cap_state = world.countries.capitals[ger.0 as usize];
    assert!(!cap_state.is_none(), "GER should have a capital state");

    // 首都所在 state 的 owner 应该是 GER 自己
    let cap_owner = world.state_owner(cap_state);
    assert_eq!(cap_owner, ger);

    // 首都所在 state 的名称应该是 STATE_64 (Brandenburg)
    let cap_name = &world.states.names[cap_state.0 as usize];
    assert_eq!(
        cap_name, "STATE_64",
        "GER capital state should be Brandenburg"
    );
}

#[test]
fn test_state_provinces_consistency() {
    let world = build_world();

    // 所有 state 的省份归属应该和 province store 一致
    for state_idx in 0..world.states.count {
        let state_owner = world.states.owners[state_idx];
        for &prov in &world.states.provinces[state_idx] {
            let prov_owner = world.province_owner(prov);
            assert_eq!(
                prov_owner, state_owner,
                "Province {} owner mismatch with state {}",
                prov.0, state_idx
            );
        }
    }
}
