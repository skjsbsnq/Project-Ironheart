//! Phase 1 综合自检 — 验证所有子模块加载的数据正确性

use clausewitz_parser::{parse, parse_defines, Value};
use hoi4_data::{CountryTag, GameData};
use hoi4_map::GameMap;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

fn hoi4_path() -> std::path::PathBuf {
    hoi4_paths::PathConfig::resolve(Default::default())
        .expect("set IRONHEART_HOI4_PATH or place HOI4 install at default Steam path")
        .game_path()
        .to_path_buf()
}

#[test]
fn audit_1_1_parser_typed_blocks() {
    // Check: rgb { 201 56 93 } should parse as Block { rgb: Block { values: [201, 56, 93] } }
    let input = "color = rgb { 201 56 93 }";
    let block = parse(input);
    let color = block.get_block("color").expect("color should be a block");
    let rgb = color.get_block("rgb").expect("rgb should be a sub-block");
    assert_eq!(rgb.values.len(), 3);
    assert_eq!(rgb.values[0], Value::Integer(201));
    assert_eq!(rgb.values[1], Value::Integer(56));
    assert_eq!(rgb.values[2], Value::Integer(93));

    // Check HSV
    let input = "color = HSV { 0.1 0.15 0.4 }";
    let block = parse(input);
    let color = block.get_block("color").unwrap();
    let hsv = color.get_block("HSV").unwrap();
    assert_eq!(hsv.values.len(), 3);
}

#[test]
fn audit_1_1_parser_did_not_break_normal_blocks() {
    // Make sure normal blocks still work after typed block fix
    let input = r#"
state = {
    id = 5
    provinces = { 1 2 3 }
    history = { owner = GER }
}
"#;
    let block = parse(input);
    let s = block.get_block("state").unwrap();
    assert_eq!(s.get_int("id"), Some(5));
    assert_eq!(s.get_block("provinces").unwrap().values.len(), 3);
    assert_eq!(
        s.get_block("history").unwrap().get_string("owner"),
        Some("GER")
    );
}

#[test]
fn audit_1_2_defines_loaded_correctly() {
    let path = hoi4_path().join("common/defines/00_defines.lua");
    let content = std::fs::read_to_string(&path).unwrap();
    let defines = parse_defines(&content);

    // Verify well-known values
    assert_eq!(defines.get_str("NGame", "START_DATE"), Some("1936.1.1.12"));
    assert_eq!(defines.get_int("NGame", "SAVE_VERSION"), Some(31));

    // NMilitary should have many fields
    let mil = defines
        .sections
        .get("NMilitary")
        .expect("NMilitary section");
    assert!(
        mil.len() > 50,
        "NMilitary should have 50+ fields, got {}",
        mil.len()
    );

    // NAI is the largest section
    let ai = defines.sections.get("NAI").expect("NAI section");
    assert!(
        ai.len() > 100,
        "NAI should have 100+ fields, got {}",
        ai.len()
    );

    println!("Defines stats:");
    println!("  Total sections: {}", defines.sections.len());
    let mut sec_names: Vec<&String> = defines.sections.keys().collect();
    sec_names.sort();
    for name in &sec_names {
        let count = defines.sections[*name].len();
        println!("    {}: {} entries", name, count);
    }
}

#[test]
fn audit_1_3_map_structure() {
    let map = GameMap::load(&hoi4_path()).unwrap();

    // Check dimensions
    assert_eq!(map.province_map.width, 5632);
    assert_eq!(map.province_map.height, 2048);

    // Check definition coverage
    let total = map.province_count();
    assert!(total >= 13000, "Got {} provinces", total);

    // Sample some known provinces
    // Province 64 = Berlin (Germany capital)
    let berlin = map.get_province(64).expect("Province 64 exists");
    assert_eq!(berlin.id, 64);

    // Check land/sea/lake distribution
    let sea = map
        .definitions
        .iter()
        .flatten()
        .filter(|p| p.province_type == hoi4_map::definition::ProvinceType::Sea)
        .count();
    let land = map
        .definitions
        .iter()
        .flatten()
        .filter(|p| p.province_type == hoi4_map::definition::ProvinceType::Land)
        .count();
    let lake = map
        .definitions
        .iter()
        .flatten()
        .filter(|p| p.province_type == hoi4_map::definition::ProvinceType::Lake)
        .count();

    println!("Map stats:");
    println!("  Total: {} provinces", total);
    println!("  Land: {}", land);
    println!("  Sea: {}", sea);
    println!("  Lake: {}", lake);

    assert!(land > 8000);
    assert!(sea > 2000);

    // Check adjacency completeness
    let mut total_edges = 0;
    let mut isolated = 0;
    for (id, neighbors) in map.adjacencies.iter().enumerate() {
        if id == 0 {
            continue;
        }
        if !neighbors.is_empty() {
            total_edges += neighbors.len();
        } else if map.get_province(id as u16).is_some() {
            isolated += 1;
        }
    }
    println!("  Total adjacency edges (counted twice): {}", total_edges);
    println!("  Isolated provinces (no neighbors): {}", isolated);

    // Sanity: most provinces should have neighbors
    assert!(total_edges > 50000, "Too few edges: {}", total_edges);
}

#[test]
fn audit_1_4_data_country_colors() {
    let data = GameData::load(&hoi4_path()).unwrap();

    // Verify known country colors from common/countries/colors.txt
    let ger = data.countries.get(&CountryTag::new("GER")).unwrap();
    println!(
        "GER color: ({}, {}, {})",
        ger.color.r, ger.color.g, ger.color.b
    );
    // GER uses HSV(0.1, 0.15, 0.4) → should be roughly (102, 96, 87) — gray-greenish
    assert!(ger.color.r > 50 && ger.color.r < 150);

    let eng = data.countries.get(&CountryTag::new("ENG")).unwrap();
    println!(
        "ENG color: ({}, {}, {})",
        eng.color.r, eng.color.g, eng.color.b
    );
    // ENG = rgb { 201 56 93 } — red
    assert_eq!(eng.color.r, 201);
    assert_eq!(eng.color.g, 56);
    assert_eq!(eng.color.b, 93);

    let sov = data.countries.get(&CountryTag::new("SOV")).unwrap();
    println!(
        "SOV color: ({}, {}, {})",
        sov.color.r, sov.color.g, sov.color.b
    );
    // SOV = rgb { 125 13 24 } — dark red
    assert_eq!(sov.color.r, 125);
    assert_eq!(sov.color.g, 13);
    assert_eq!(sov.color.b, 24);

    let fra = data.countries.get(&CountryTag::new("FRA")).unwrap();
    println!(
        "FRA color: ({}, {}, {})",
        fra.color.r, fra.color.g, fra.color.b
    );
    assert_eq!(fra.color.r, 57);
    assert_eq!(fra.color.g, 113);
    assert_eq!(fra.color.b, 228);

    let usa = data.countries.get(&CountryTag::new("USA")).unwrap();
    println!(
        "USA color: ({}, {}, {})",
        usa.color.r, usa.color.g, usa.color.b
    );

    // Count countries with non-default color
    let with_color = data
        .countries
        .values()
        .filter(|c| !(c.color.r == 128 && c.color.g == 128 && c.color.b == 128))
        .count();
    println!(
        "\nCountries with custom color: {}/{}",
        with_color,
        data.countries.len()
    );
    assert!(
        with_color > 200,
        "Expected 200+ countries with custom colors"
    );
}

#[test]
fn audit_1_4_capital_assignments() {
    let data = GameData::load(&hoi4_path()).unwrap();

    // Major powers should have known capitals
    // GER capital = 64 (Berlin)
    let ger = data.countries.get(&CountryTag::new("GER")).unwrap();
    assert_eq!(ger.capital, 64, "GER capital should be 64 (Berlin)");

    // SOV capital should be Moscow region (state 219)
    let sov = data.countries.get(&CountryTag::new("SOV")).unwrap();
    println!("SOV capital: {}", sov.capital);
    assert!(sov.capital > 0);

    // ENG, FRA, USA, JAP, ITA, CHI all should have capitals
    for tag in &["ENG", "FRA", "USA", "JAP", "ITA", "CHI"] {
        let c = data.countries.get(&CountryTag::new(tag)).unwrap();
        println!("{} capital: {}", tag, c.capital);
        assert!(c.capital > 0, "{} should have a capital", tag);
    }
}

#[test]
fn audit_1_4_state_ownership() {
    let data = GameData::load(&hoi4_path()).unwrap();

    // Count states by owner
    let mut owners: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for s in &data.states {
        *owners.entry(s.owner.as_str().to_owned()).or_insert(0) += 1;
    }

    println!("State counts by major owners:");
    for tag in &[
        "GER", "SOV", "ENG", "FRA", "USA", "JAP", "ITA", "CHI", "RAJ",
    ] {
        let count = owners.get(*tag).copied().unwrap_or(0);
        println!("  {}: {} states", tag, count);
    }

    assert!(
        owners.get("GER").copied().unwrap_or(0) > 5,
        "GER should own multiple states"
    );
    assert!(
        owners.get("SOV").copied().unwrap_or(0) > 50,
        "SOV should own many states"
    );
    assert!(
        owners.get("USA").copied().unwrap_or(0) > 20,
        "USA should own many states"
    );

    // Total province ownership
    println!(
        "Total province ownership entries: {}",
        data.province_owners.len()
    );
    assert!(data.province_owners.len() > 8000);
}

#[test]
fn audit_1_4_austria_starts_owned_by_austria() {
    let data = GameData::load(&hoi4_path()).unwrap();

    for state_id in [4_u16, 152_u16, 153_u16] {
        let state = data
            .states
            .iter()
            .find(|state| state.id == state_id)
            .unwrap_or_else(|| panic!("state {} should exist", state_id));
        assert_eq!(
            state.owner.as_str(),
            "AUS",
            "state {} should start owned by Austria, not later dated history owner",
            state_id
        );
        assert!(
            state.cores.iter().any(|core| core.as_str() == "AUS"),
            "state {} should start as Austrian core",
            state_id
        );
    }
}

#[test]
fn audit_1_4_own_1936_overrides_replace_later_vanilla_history() {
    let data = GameData::load(&hoi4_path()).unwrap();

    let expected = [
        (4_u16, "AUS"),
        (152, "AUS"),
        (153, "AUS"),
        (848, "AUS"),
        (975, "AUS"),
        (976, "AUS"),
        (69, "CZE"),
        (74, "CZE"),
        (75, "CZE"),
        (972, "CZE"),
        (188, "LIT"),
        (622, "PRC"),
        (608, "HBC"),
        (1039, "HBC"),
        (597, "SND"),
        (743, "SND"),
        (1038, "SND"),
        (615, "SHX"),
        (746, "SHX"),
        (592, "GDC"),
        (599, "GXC"),
        (325, "YUN"),
        (744, "XAJ"),
        (747, "YUN"),
        (283, "XSM"),
        (756, "XSM"),
        (617, "SIK"),
        (760, "SIK"),
        (620, "CHI"),
        (751, "SIC"),
        (1037, "SIC"),
        (1041, "SIC"),
        (322, "TIB"),
        (757, "TIB"),
        (714, "MAN"),
        (761, "MAN"),
        (611, "MEN"),
    ];

    for (state_id, owner) in expected {
        let state = data
            .states
            .iter()
            .find(|state| state.id == state_id)
            .unwrap_or_else(|| panic!("state {} should exist", state_id));
        assert_eq!(state.owner.as_str(), owner, "state {} owner", state_id);
    }
}

#[test]
fn audit_1_4_china_1936_overrides_keep_regional_cores() {
    let data = GameData::load(&hoi4_path()).unwrap();
    let expected = [
        (622_u16, "PRC"),
        (608, "HBC"),
        (597, "SND"),
        (615, "SHX"),
        (592, "GDC"),
        (325, "YUN"),
        (744, "XAJ"),
        (283, "XSM"),
        (617, "SIK"),
        (1037, "SIC"),
        (322, "TIB"),
        (714, "MAN"),
        (611, "MEN"),
    ];

    for (state_id, core) in expected {
        let state = data
            .states
            .iter()
            .find(|state| state.id == state_id)
            .unwrap_or_else(|| panic!("state {} should exist", state_id));
        assert!(
            state.cores.iter().any(|tag| tag.as_str() == core),
            "state {} should keep {} core",
            state_id,
            core
        );
    }
}

#[test]
fn audit_1_4_custom_1936_china_countries_load() {
    let data = GameData::load(&hoi4_path()).unwrap();

    for (tag, capital) in [("GDC", 592_u16), ("XAJ", 744_u16)] {
        let country = data
            .countries
            .get(&CountryTag::new(tag))
            .unwrap_or_else(|| panic!("custom 1936 country {tag} should load"));
        assert_eq!(country.capital, capital, "{tag} capital");
        assert_eq!(country.ruling_party, "neutrality", "{tag} ruling party");
    }

    for tag in ["SIC", "HBC", "SND"] {
        let country = data
            .countries
            .get(&CountryTag::new(tag))
            .unwrap_or_else(|| panic!("vanilla {tag} country should load"));
        assert!(country.capital > 0, "{tag} should have a vanilla capital");
    }
}

#[test]
fn audit_1_4_state_coverage() {
    let data = GameData::load(&hoi4_path()).unwrap();

    let mut states_by_country: std::collections::HashMap<String, u32> =
        std::collections::HashMap::new();
    for s in &data.states {
        *states_by_country
            .entry(s.owner.as_str().to_owned())
            .or_insert(0) += 1;
    }

    println!("State coverage by major powers:");
    for tag in &["GER", "SOV", "ENG", "FRA", "USA", "JAP", "ITA"] {
        let n = states_by_country.get(*tag).copied().unwrap_or(0);
        println!("  {}: {} states", tag, n);
    }

    assert!(
        states_by_country.get("GER").copied().unwrap_or(0) > 5,
        "GER should have multiple states"
    );
}

#[derive(Debug, Deserialize)]
struct TagOnlyProfile {
    tag: String,
}

#[derive(Debug, Deserialize)]
struct FinanceProfileForCoverage {
    country: String,
    year: u16,
    quarter: u8,
}

#[test]
fn audit_g04_1936_country_economy_coverage_sets() {
    let data = GameData::load(&hoi4_path()).expect("Failed to load game data");
    let economy_tags = load_history_country_profile_tags();
    let pop_tags = load_initial_pop_profile_tags();
    let finance_profiles: Vec<FinanceProfileForCoverage> =
        ron::from_str(&ron_without_directives(include_str!(
            "../../hoi4-content/content/economy_v6/finance/historical_finance_profiles.ron"
        )))
        .expect("historical finance profiles should load");

    let existing_by_tag = existing_state_owner_counts(&data);
    let existing_tags: BTreeSet<String> = existing_by_tag.keys().cloned().collect();
    let building_tags = economy_tags.clone();
    let finance_tags: BTreeSet<String> = finance_profiles
        .iter()
        .filter(|profile| profile.year == 1936 && (1..=4).contains(&profile.quarter))
        .map(|profile| profile.country.clone())
        .collect();

    let missing_economy = missing(&existing_tags, &economy_tags);
    let missing_pops = missing(&existing_tags, &pop_tags);
    let missing_buildings = missing(&existing_tags, &building_tags);
    let missing_finance = missing(&existing_tags, &finance_tags);

    println!(
        "G04 existing countries with owned 1936 states: {}",
        existing_tags.len()
    );
    println!("G04 existing country tags: {}", join_set(&existing_tags));
    println!("G04 state owner counts:");
    for (tag, count) in &existing_by_tag {
        println!("  {tag}: {count}");
    }
    println!(
        "G04 missing economy profiles: {}",
        join_vec(&missing_economy)
    );
    println!("G04 missing POP profiles: {}", join_vec(&missing_pops));
    println!(
        "G04 missing building profiles: {}",
        join_vec(&missing_buildings)
    );
    println!(
        "G04 missing finance profiles: {}",
        join_vec(&missing_finance)
    );

    assert!(
        !existing_tags.is_empty(),
        "existing country set must not be empty"
    );
    assert!(
        existing_tags.contains("GER")
            && existing_tags.contains("CHI")
            && existing_tags.contains("AUS"),
        "1936 owner set should include vanilla majors and 1936 override countries"
    );
    assert!(
        missing_economy.iter().any(|tag| tag == "AUS"),
        "AUS should remain visible as missing economy coverage until G06 fills it"
    );
    assert!(
        missing_pops.iter().any(|tag| tag == "AUS"),
        "AUS should remain visible as missing POP coverage until G06/G05 fills it"
    );
    assert!(
        missing_finance.iter().any(|tag| tag == "AUS"),
        "AUS should remain visible as missing finance coverage until G06 fills it"
    );
}

fn existing_state_owner_counts(data: &GameData) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for state in &data.states {
        let tag = state.owner.as_str();
        if tag.is_empty() || tag == "---" {
            continue;
        }
        *counts.entry(tag.to_owned()).or_insert(0) += 1;
    }
    counts
}

fn load_history_country_profile_tags() -> BTreeSet<String> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../hoi4-content/content/history_1936/countries");
    let mut tags = BTreeSet::new();
    for entry in std::fs::read_dir(&dir)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", dir.display()))
    {
        let path = entry.expect("country profile dir entry").path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("ron") {
            continue;
        }
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));
        let profile: TagOnlyProfile = ron::from_str(&ron_without_directives(&text))
            .unwrap_or_else(|err| panic!("failed to parse {}: {err}", path.display()));
        tags.insert(profile.tag);
    }
    tags
}

fn ron_without_directives(text: &str) -> String {
    text.trim_start_matches('\u{feff}')
        .lines()
        .filter(|line| !line.trim_start().starts_with("#!["))
        .collect::<Vec<_>>()
        .join("\n")
}

fn load_initial_pop_profile_tags() -> BTreeSet<String> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../hoi4-content/content/economy_v6/pops");
    let mut tags = BTreeSet::new();
    for entry in std::fs::read_dir(&dir)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", dir.display()))
    {
        let path = entry.expect("POP profile dir entry").path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("ron") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        let Some(tag) = stem.strip_prefix("initial_") else {
            continue;
        };
        tags.insert(tag.to_ascii_uppercase());
    }
    tags
}

fn missing(existing: &BTreeSet<String>, covered: &BTreeSet<String>) -> Vec<String> {
    existing.difference(covered).cloned().collect()
}

fn join_set(values: &BTreeSet<String>) -> String {
    values.iter().cloned().collect::<Vec<_>>().join(", ")
}

fn join_vec(values: &[String]) -> String {
    if values.is_empty() {
        "无".to_owned()
    } else {
        values.join(", ")
    }
}
