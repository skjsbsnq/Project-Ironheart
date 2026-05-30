use hoi4_data::{CountryTag, GameData};

fn hoi4_path() -> std::path::PathBuf {
    hoi4_paths::PathConfig::resolve(Default::default())
        .expect("set IRONHEART_HOI4_PATH or place HOI4 install at default Steam path")
        .game_path()
        .to_path_buf()
}

#[test]
fn test_load_game_data() {
    let data = GameData::load(&hoi4_path()).expect("Failed to load game data");

    // Should have many countries
    assert!(
        data.countries.len() > 50,
        "Expected 50+ countries, got {}",
        data.countries.len()
    );

    // Germany should exist
    let ger = data
        .countries
        .get(&CountryTag::new("GER"))
        .expect("Germany should exist");
    assert_eq!(ger.tag.as_str(), "GER");
    assert!(ger.capital > 0, "Germany should have a capital");

    // Should have many states
    assert!(
        data.states.len() > 900,
        "Expected 900+ states, got {}",
        data.states.len()
    );

    // Check a known state (state 5 = East Prussia)
    let state5 = data
        .states
        .iter()
        .find(|s| s.id == 5)
        .expect("State 5 should exist");
    assert_eq!(state5.owner.as_str(), "GER");
    assert!(state5.manpower > 0);
    assert!(!state5.provinces.is_empty());

    // Province owners should be populated
    assert!(
        data.province_owners.len() > 10000,
        "Expected 10000+ province owners, got {}",
        data.province_owners.len()
    );

    // Check some major countries have colors
    let usa = data
        .countries
        .get(&CountryTag::new("USA"))
        .expect("USA should exist");
    let eng = data
        .countries
        .get(&CountryTag::new("ENG"))
        .expect("ENG should exist");
    let sov = data
        .countries
        .get(&CountryTag::new("SOV"))
        .expect("SOV should exist");
    let jap = data
        .countries
        .get(&CountryTag::new("JAP"))
        .expect("JAP should exist");

    // All major powers should have capitals
    assert!(usa.capital > 0);
    assert!(eng.capital > 0);
    assert!(sov.capital > 0);
    assert!(jap.capital > 0);

    // Check state data is populated; V6 building setup is no longer stored on vanilla states.
    assert!(
        data.states.len() > 500,
        "Expected many vanilla states, got {}",
        data.states.len()
    );
}
