use hoi4_map::definition::ProvinceType;
use hoi4_map::GameMap;

fn hoi4_path() -> std::path::PathBuf {
    hoi4_paths::PathConfig::resolve(Default::default())
        .expect("set IRONHEART_HOI4_PATH or place HOI4 install at default Steam path")
        .game_path()
        .to_path_buf()
}

#[test]
fn test_load_full_map() {
    let map = GameMap::load(&hoi4_path()).expect("Failed to load map");

    // Should have ~13000+ provinces
    let count = map.province_count();
    assert!(count > 13000, "Expected 13000+ provinces, got {}", count);

    // Province 1 should exist (first real province)
    let p1 = map.get_province(1).expect("Province 1 should exist");
    assert_eq!(p1.id, 1);

    // Check province map dimensions (5632×2048 for HOI4)
    assert_eq!(map.province_map.width, 5632);
    assert_eq!(map.province_map.height, 2048);

    // Province map should have valid IDs
    let center_id = map.province_map.get(2816, 1024);
    assert!(center_id > 0, "Center of map should have a valid province");

    // Adjacency should be populated
    let neighbors = map.neighbors(center_id);
    assert!(
        !neighbors.is_empty(),
        "Province {} should have neighbors",
        center_id
    );

    // Check some known province types
    let sea_count = map
        .definitions
        .iter()
        .filter_map(|d| d.as_ref())
        .filter(|d| d.province_type == ProvinceType::Sea)
        .count();
    assert!(
        sea_count > 1000,
        "Expected 1000+ sea provinces, got {}",
        sea_count
    );

    let land_count = map
        .definitions
        .iter()
        .filter_map(|d| d.as_ref())
        .filter(|d| d.province_type == ProvinceType::Land)
        .count();
    assert!(
        land_count > 10000,
        "Expected 10000+ land provinces, got {}",
        land_count
    );

    // Special adjacencies (straits/canals)
    assert!(
        !map.special_adjacencies.is_empty(),
        "Should have special adjacencies"
    );
    // Panama canal should be in there
    let has_panama = map
        .special_adjacencies
        .iter()
        .any(|a| a.rule_name.contains("PANAMA"));
    assert!(has_panama, "Should have Panama Canal adjacency");
}
