//! J.1.10：Country Leader & Portrait 集成测试。
//!
//! 验证：
//! 1. GER 的 leader 名 = "Adolf Hitler"（通过 character_names loc 解析）
//! 2. SOV 的 leader = "Iosif Stalin"
//! 3. ENG 的 leader = "Stanley Baldwin"（1936 起手 democratic → liberalism）
//! 4. 缺失肖像时 leader_portrait_key = None 不 panic（用一个不存在的 tag 测试）

#[test]
fn ger_leader_is_adolf_hitler() {
    let world =
        hoi4_integration::load_world().expect("HOI4 安装目录未找到；设置 IRONHEART_HOI4_PATH");

    let ger = world.country("GER").expect("GER tag missing");
    let leader = world
        .country_leader(ger)
        .expect("GER should have a country_leader");

    // name_loc_key 应为 "GER_adolf_hitler"
    assert_eq!(leader.name_loc_key, "GER_adolf_hitler");
    // character_names 应解析出 "Adolf Hitler"
    let display_name = world.data.character_names.get(&leader.name_loc_key);
    assert!(
        display_name.is_some(),
        "GER_adolf_hitler should be in character_names"
    );
    assert_eq!(display_name.unwrap(), "Adolf Hitler");
    // portrait 应存在
    assert!(
        leader.portrait_large.is_some(),
        "GER leader should have portrait_large"
    );
    assert!(leader
        .portrait_large
        .as_deref()
        .unwrap()
        .contains("adolf_hitler"));
}

#[test]
fn sov_leader_is_iosif_stalin() {
    let world =
        hoi4_integration::load_world().expect("HOI4 安装目录未找到；设置 IRONHEART_HOI4_PATH");

    let sov = world.country("SOV").expect("SOV tag missing");
    let leader = world
        .country_leader(sov)
        .expect("SOV should have a country_leader");

    assert_eq!(leader.name_loc_key, "SOV_iosif_stalin");
    let display_name = world.data.character_names.get(&leader.name_loc_key);
    assert!(display_name.is_some());
    assert!(
        display_name.unwrap().contains("Stalin"),
        "SOV leader display name should contain 'Stalin', got: {:?}",
        display_name
    );
    assert!(leader.portrait_large.is_some());
}

#[test]
fn eng_leader_is_stanley_baldwin() {
    let world =
        hoi4_integration::load_world().expect("HOI4 安装目录未找到；设置 IRONHEART_HOI4_PATH");

    // ENG 1936 ruling_party = democratic → subtypes include liberalism
    let eng = world.country("ENG").expect("ENG tag missing");
    let leader = world
        .country_leader(eng)
        .expect("ENG should have a country_leader");

    // Vanilla 1936 starts with Stanley Baldwin. Neville Chamberlain is also a
    // democratic leader in ENG.txt, but appears later and should not be picked
    // at scenario start.
    assert_eq!(leader.name_loc_key, "ENG_stanley_baldwin");
    let display_name = world.data.character_names.get(&leader.name_loc_key);
    assert!(display_name.is_some());
    let name = display_name.unwrap();
    assert!(
        name.contains("Baldwin"),
        "ENG leader should be Baldwin, got: {name}"
    );
}

#[test]
fn major_1936_leaders_match_start_date() {
    let world =
        hoi4_integration::load_world().expect("HOI4 安装目录未找到；设置 IRONHEART_HOI4_PATH");

    let expected = [
        ("GER", "GER_adolf_hitler"),
        ("SOV", "SOV_iosif_stalin"),
        ("ENG", "ENG_stanley_baldwin"),
        ("FRA", "FRA_pierre_laval"),
        ("ITA", "ITA_benito_mussolini"),
        ("JAP", "JAP_keisuke_okada"),
        ("USA", "USA_franklin_delano_roosevelt"),
        ("POL", "POL_ignacy_moscicki"),
        ("ROM", "ROM_gheorghe_tatarescu"),
        ("CAN", "CAN_mackenzie_king"),
        ("AST", "AST_john_curtin"),
        ("NZL", "NZL_michael_joseph_savage"),
    ];

    let mut mismatches = Vec::new();
    for (tag, expected_key) in expected {
        let cid = world
            .country(tag)
            .unwrap_or_else(|| panic!("{tag} tag missing"));
        let leader = world
            .country_leader(cid)
            .unwrap_or_else(|| panic!("{tag} should have a country_leader"));
        if leader.name_loc_key != expected_key {
            mismatches.push(format!(
                "{tag}: got {}, expected {expected_key}",
                leader.name_loc_key
            ));
        }
        assert!(
            leader.portrait_large.is_some(),
            "{tag} 1936 leader should define portrait_large"
        );
    }

    assert!(
        mismatches.is_empty(),
        "1936 leader mismatches:\n{}",
        mismatches.join("\n")
    );
}

#[test]
fn reported_countries_have_leader_names_and_portraits() {
    let world =
        hoi4_integration::load_world().expect("HOI4 安装目录未找到；设置 IRONHEART_HOI4_PATH");

    let expected_tags = ["SPR", "TUR", "SOV"];

    for tag in expected_tags {
        let cid = world
            .country(tag)
            .unwrap_or_else(|| panic!("{tag} tag missing"));
        let leader = world
            .country_leader(cid)
            .unwrap_or_else(|| panic!("{tag} should not display unknown leader"));
        assert!(
            !leader.name_loc_key.is_empty(),
            "{tag} leader name should not be empty"
        );
        assert!(
            leader.portrait_large.is_some(),
            "{tag} leader should define portrait_large"
        );
    }
}

#[test]
fn owned_countries_have_leader_names_and_portrait_keys() {
    let world =
        hoi4_integration::load_world().expect("HOI4 安装目录未找到；设置 IRONHEART_HOI4_PATH");

    let mut missing = Vec::new();
    for i in 0..world.countries.count {
        let cid = hoi4_state::CountryId(i as u16);
        let Some(tag) = world.country_tag(cid) else {
            continue;
        };
        let owns_state = world.states.owners.iter().any(|owner| *owner == cid);
        if !owns_state {
            continue;
        }
        match world.country_leader(cid) {
            Some(leader) if !leader.name_loc_key.is_empty() && leader.portrait_large.is_some() => {}
            Some(leader) => missing.push(format!(
                "{tag}: leader={}, portrait={:?}",
                leader.name_loc_key, leader.portrait_large
            )),
            None => missing.push(format!("{tag}: unknown leader")),
        }
    }

    assert!(
        missing.is_empty(),
        "owned countries with missing leader/portrait:\n{}",
        missing.join("\n")
    );
}

#[test]
fn missing_tag_returns_none_no_panic() {
    let world =
        hoi4_integration::load_world().expect("HOI4 安装目录未找到；设置 IRONHEART_HOI4_PATH");

    // 不存在的 tag → country() 返回 None
    assert!(world.country("ZZZ").is_none());

    // 即使手动构造一个 CountryId::NONE，country_leader 也应返回 None
    let none_leader = world.country_leader(hoi4_state::CountryId::NONE);
    assert!(none_leader.is_none());
}

#[test]
fn party_names_loaded() {
    let world =
        hoi4_integration::load_world().expect("HOI4 安装目录未找到；设置 IRONHEART_HOI4_PATH");

    // GER fascism party long name
    let key = "GER_fascism_party_long";
    let val = world.data.party_names.get(key);
    assert!(val.is_some(), "party_names should contain {key}");
    assert!(val.unwrap().contains("Nationalsozialistische"));
}
