use clausewitz_parser::{parse, parse_defines, Value};
use std::fs;

fn hoi4_path() -> std::path::PathBuf {
    hoi4_paths::PathConfig::resolve(Default::default())
        .expect("set IRONHEART_HOI4_PATH or place HOI4 install at default Steam path")
        .game_path()
        .to_path_buf()
}

fn read_game_file(relative: &str) -> String {
    let path = hoi4_path().join(relative);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("Failed to read {}: {}", path.display(), e))
}

#[test]
fn test_parse_state_file() {
    let content = read_game_file("history/states/5-Germany.txt");
    let block = parse(&content);
    let state = block.get_block("state").unwrap();
    assert_eq!(state.get_int("id"), Some(5));
    assert_eq!(state.get_string("name"), Some("STATE_5"));
    assert_eq!(state.get_int("manpower"), Some(1238108));
    assert_eq!(state.get_string("state_category"), Some("town"));

    let history = state.get_block("history").unwrap();
    assert_eq!(history.get_string("owner"), Some("GER"));

    let provs = state.get_block("provinces").unwrap();
    assert!(provs.values.len() > 5);
}

#[test]
fn test_parse_country_file() {
    let content = read_game_file("common/countries/Germany.txt");
    let block = parse(&content);
    assert_eq!(
        block.get_string("graphical_culture"),
        Some("western_european_gfx")
    );
    assert_eq!(
        block.get_string("graphical_culture_2d"),
        Some("western_european_2d")
    );

    let color = block.get_block("color").unwrap();
    assert_eq!(color.values.len(), 3);
}

#[test]
fn test_parse_infantry_unit() {
    let content = read_game_file("common/units/infantry.txt");
    let block = parse(&content);
    let sub_units = block.get_block("sub_units").unwrap();
    let infantry = sub_units.get_block("infantry").unwrap();
    assert_eq!(infantry.get_string("abbreviation"), Some("INF"));
    assert_eq!(infantry.get_int("combat_width"), Some(2));
    assert_eq!(infantry.get_int("max_strength"), Some(25));
    assert_eq!(infantry.get_int("max_organisation"), Some(60));
    assert_eq!(infantry.get_int("manpower"), Some(1000));
    assert_eq!(infantry.get_bool("active"), Some(false));
}

#[test]
fn test_parse_default_map() {
    let content = read_game_file("map/default.map");
    let block = parse(&content);
    assert_eq!(block.get_string("definitions"), Some("definition.csv"));
    assert_eq!(block.get_string("provinces"), Some("provinces.bmp"));
    assert_eq!(block.get_string("terrain"), Some("terrain.bmp"));

    let tree = block.get_block("tree").unwrap();
    assert!(tree.values.len() >= 4);
}

#[test]
fn test_parse_ideology() {
    let content = read_game_file("common/ideologies/00_ideologies.txt");
    let block = parse(&content);
    let ideologies = block.get_block("ideologies").unwrap();
    let democratic = ideologies.get_block("democratic").unwrap();
    assert_eq!(
        democratic.get_bool("can_host_government_in_exile"),
        Some(true)
    );

    let color = democratic.get_block("color").unwrap();
    assert_eq!(color.values.len(), 3);
}

#[test]
fn test_parse_event_file() {
    let content = read_game_file("events/WUW_Hungary.txt");
    let block = parse(&content);
    // add_namespace = WW_hungary appears as entry
    let ns_values: Vec<&Value> = block.get_all("add_namespace");
    assert!(ns_values.len() >= 1);

    // country_event blocks
    let events = block.get_all("country_event");
    assert!(events.len() >= 3);

    if let Value::Block(ev) = events[0] {
        assert_eq!(ev.get_string("id"), Some("WW_hungary.1"));
        assert_eq!(ev.get_bool("fire_only_once"), Some(true));
        assert_eq!(ev.get_bool("is_triggered_only"), Some(true));
    } else {
        panic!("Expected block");
    }
}

#[test]
#[ignore = "V5 §3.4 vanilla focus 加载已停用，本 parser 测试参考 vanilla 文件"]
fn test_parse_large_focus_tree() {
    // Parse the generic focus tree (49KB) - tests performance and correctness
    let content = read_game_file("vanilla-focus-dir-removed/generic.txt");
    let block = parse(&content);

    let focus_tree = block.get_block("focus_tree").unwrap();
    assert_eq!(focus_tree.get_string("id"), Some("generic_focus"));
    assert_eq!(focus_tree.get_bool("default"), Some(true));

    // Should have many focus entries
    let focuses = focus_tree.get_all("focus");
    assert!(
        focuses.len() > 10,
        "Expected many focuses, got {}",
        focuses.len()
    );
}

#[test]
fn test_parse_real_defines() {
    let content = read_game_file("common/defines/00_defines.lua");
    let defines = parse_defines(&content);

    // NGame section
    assert_eq!(defines.get_str("NGame", "START_DATE"), Some("1936.1.1.12"));
    assert_eq!(defines.get_str("NGame", "END_DATE"), Some("1949.1.1.1"));
    assert_eq!(
        defines.get_float("NGame", "MAP_SCALE_PIXEL_TO_KM"),
        Some(7.114)
    );
    assert_eq!(defines.get_int("NGame", "SAVE_VERSION"), Some(31));

    let speeds = defines
        .get("NGame", "GAME_SPEED_SECONDS")
        .unwrap()
        .as_array()
        .unwrap();
    assert_eq!(speeds.len(), 5);
    assert_eq!(speeds[0], 2.0);

    // NDiplomacy section
    assert_eq!(
        defines.get_float("NDiplomacy", "BASE_SURRENDER_LEVEL"),
        Some(1.0)
    );
    assert_eq!(defines.get_int("NDiplomacy", "MAX_TRUST_VALUE"), Some(100));
    assert_eq!(defines.get_int("NDiplomacy", "MIN_TRUST_VALUE"), Some(-100));
    assert_eq!(
        defines.get_int("NDiplomacy", "BASE_TRUCE_PERIOD"),
        Some(180)
    );

    // NMilitary section should exist
    assert!(defines.sections.contains_key("NMilitary"));

    // NAir section should exist
    assert!(defines.sections.contains_key("NAir"));

    // NNavy section should exist
    assert!(defines.sections.contains_key("NNavy"));

    // NAI section should exist
    assert!(defines.sections.contains_key("NAI"));

    // NSupply section should exist
    assert!(defines.sections.contains_key("NSupply"));

    // Check total sections count (should be ~30)
    assert!(
        defines.sections.len() >= 20,
        "Expected 20+ sections, got {}",
        defines.sections.len()
    );
}

#[test]
fn test_parse_graphics_defines() {
    let content = read_game_file("common/defines/00_graphics.lua");
    let defines = parse_defines(&content);
    // Should have NMapMode or NWiki section
    assert!(defines.sections.contains_key("NWiki") || defines.sections.contains_key("NMapMode"));
}
