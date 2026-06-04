use hoi4_content::{
    Historical1936Database, StateIntegrationDef, TradeRouteKindDef, V6Database, WorkforceProfileDef,
};
use std::collections::{HashMap, HashSet};

#[test]
fn h0_history_1936_loads_all_required_profiles() {
    let db = Historical1936Database::load().expect("history_1936 RON should load");

    let tags: Vec<&str> = db
        .countries
        .iter()
        .map(|country| country.tag.as_str())
        .collect();
    assert_eq!(
        tags,
        [
            "USA", "GER", "SOV", "ENG", "FRA", "CZE", "AUS", "LIT", "POL", "JAP", "ITA", "SPR",
            "CHI", "HBC", "SND", "PRC", "SHX", "GXC", "GDC", "YUN", "XAJ", "SIC", "XSM", "SIK",
            "TIB", "CAN", "AST", "NZL", "SAF", "RAJ", "MAL", "ROM", "SWE", "MAN", "MEN",
        ]
    );
    assert_eq!(db.military_profiles.len(), db.countries.len());
    assert!(!db.state_populations.is_empty());
    assert!(!db.state_deposits.is_empty());
    assert!(!db.trade_routes.is_empty());
    assert!(!db.head_of_states.is_empty());
}

#[test]
fn p3_warehouse_visible_country_profiles_are_complete() {
    let history = Historical1936Database::load().expect("history_1936 RON should load");
    let db = V6Database::load();
    let required_tags = [
        "AST", "AUS", "CAN", "CHI", "CZE", "ENG", "FRA", "GDC", "GER", "GXC", "HBC", "ITA", "JAP",
        "LIT", "MAL", "MAN", "MEN", "NZL", "POL", "PRC", "RAJ", "ROM", "SAF", "SHX", "SIC", "SIK",
        "SND", "SOV", "SPR", "SWE", "TIB", "USA", "XAJ", "XSM", "YUN",
    ];

    for tag in required_tags {
        assert!(
            history.country(tag).is_some(),
            "missing {tag} economy profile"
        );
        assert!(
            history.head_of_state(tag).is_some(),
            "missing {tag} head of state profile"
        );
        assert!(
            history
                .military_profiles
                .iter()
                .any(|profile| profile.tag == tag),
            "missing {tag} military profile"
        );
        assert!(
            db.initial_pops.contains_key(tag),
            "missing {tag} POP profile"
        );
    }
}

#[test]
fn p3_state_owner_overrides_have_population_profiles() {
    let db = Historical1936Database::load().expect("history_1936 RON should load");
    let required_states = [
        4, 69, 74, 75, 152, 153, 188, 610, 611, 612, 714, 715, 716, 717, 761, 848, 972, 975, 976,
    ];

    for state_id in required_states {
        assert!(
            db.state_population(state_id).is_some(),
            "missing Phase 3 state owner override population for state {state_id}"
        );
    }
}

#[test]
fn h0_head_of_state_1936_overrides_load() {
    let db = Historical1936Database::load().expect("history_1936 RON should load");
    let mut seen = HashSet::new();

    for entry in &db.head_of_states {
        assert!(
            seen.insert(entry.tag.as_str()),
            "duplicate head_of_state tag {}",
            entry.tag
        );
        assert!(
            !entry.character_key.is_empty() || !entry.name.is_empty(),
            "{} must define character_key or name",
            entry.tag
        );
    }

    let chi = db.head_of_state("CHI").expect("CHI head of state");
    assert_eq!(chi.character_key, "CHI_chiang_kaishek");
    assert_eq!(chi.name, "Chiang Kai-Shek");
    assert_eq!(chi.portrait_gfx, "GFX_portrait_CHI_chiang_kaishek");

    let jap = db.head_of_state("JAP").expect("JAP head of state");
    assert_eq!(jap.character_key, "JAP_emperor_hirohito");
    assert_eq!(jap.name, "Hirohito");

    let eng = db.head_of_state("ENG").expect("ENG displayed leader");
    assert_eq!(eng.character_key, "ENG_neville_chamberlain");

    for country in &db.countries {
        assert!(
            db.head_of_state(&country.tag).is_some(),
            "missing 1936 head_of_state override for {}",
            country.tag
        );
    }

    let expected_character_keys = [
        ("RAJ", "RAJ_lord_linlithgow"),
        ("SAF", "SAF_j_b_m_hertzog"),
        ("MAL", "MAL_shenton_thomas"),
        ("SWE", "SWE_per_albin_hansson"),
        ("HBC", "HBC_yin_jugeng"),
        ("SND", "SND_han_fuqu"),
        ("PRC", "PRC_mao_zedong"),
        ("SHX", "SHX_yan_xishan"),
        ("GXC", "GXC_li_zongren"),
        ("GDC", "GDC_chen_jitang"),
        ("YUN", "YUN_long_yun"),
        ("XAJ", "XAJ_zhang_xueliang"),
        ("SIC", "SIC_liu_xiang"),
        ("XSM", "XSM_ma_bufang"),
        ("SIK", "SIK_sheng_shicai"),
        ("TIB", "TIB_reting_rinpoche"),
        ("MAN", "MAN_aisin_gioro_puyi"),
        ("MEN", "MEN_prince_demchugdongrub"),
    ];
    for (tag, expected_key) in expected_character_keys {
        assert_eq!(
            db.head_of_state(tag)
                .unwrap_or_else(|| panic!("{tag} head of state"))
                .character_key,
            expected_key
        );
    }
}

#[test]
fn p1_state_population_1936_loads_and_has_unique_ids() {
    let db = Historical1936Database::load().expect("history_1936 RON should load");
    let mut seen = HashSet::new();

    for state in &db.state_populations {
        assert!(state.population > 0, "state {} population", state.state_id);
        assert!(
            seen.insert(state.state_id),
            "duplicate state_population entry for state {}",
            state.state_id
        );
        if let Some(urbanization) = state.urbanization {
            assert!(
                (0.0..=1.0).contains(&urbanization),
                "state {} urbanization",
                state.state_id
            );
        }
        if let Some(literacy) = state.literacy {
            assert!(
                (0.0..=1.0).contains(&literacy),
                "state {} literacy",
                state.state_id
            );
        }
    }
}

#[test]
fn p1_state_population_covers_first_batch_targets() {
    let db = Historical1936Database::load().expect("history_1936 RON should load");
    let required_states = [
        6, 64, 8, 54, 9, 11, 51, 7, 10, 15, 55, 13, 28, 29, 59, // GER
        91, 92, 93, 126, // ENG metropole
        105, 106, 107, // FRA metropole
        303, 702, 703, 304, 307, 306, // RAJ, MAL, CAN, AST, SAF, NZL
    ];

    for state_id in required_states {
        assert!(
            db.state_population(state_id).is_some(),
            "missing Phase 1 state population for state {}",
            state_id
        );
    }

    assert_eq!(
        db.state_population(91).expect("London/SE").integration,
        StateIntegrationDef::Metropole
    );
    assert_eq!(
        db.state_population(303).expect("British Raj").integration,
        StateIntegrationDef::Protectorate
    );
    assert_eq!(
        db.state_population(703).expect("Canada").integration,
        StateIntegrationDef::Incorporated
    );
}

#[test]
fn p10_state_population_covers_authored_population_batches() {
    let db = Historical1936Database::load().expect("history_1936 RON should load");
    let required_states = [
        // Batch A: GER, ENG metropole, FRA metropole, USA, SOV, ITA, JAP, CHI.
        6, 64, 8, 54, 9, 11, 51, 7, 10, 15, 55, 13, 28, 29, 59, 91, 92, 93, 126, 105, 106, 107, 195,
        202, 276, 283, 285, 290, 293, 294, 229, 230, 231, 232, 212, 213, 214, 300, 301, 302, 613,
        614, 615, // Batch B: RAJ, MAL, CAN, AST, NZL, SAF, MAN, MEN.
        303, 702, 703, 304, 306, 307, 305, 308,
    ];

    for state_id in required_states {
        assert!(
            db.state_population(state_id).is_some(),
            "missing Phase 10 state population for state {}",
            state_id
        );
    }

    assert!(
        db.state_populations.len() >= required_states.len(),
        "Phase 10 data should not regress below authored batch coverage"
    );
    assert_eq!(
        db.state_population(702).expect("MAL").integration,
        StateIntegrationDef::Protectorate
    );
    assert_eq!(
        db.state_population(305).expect("MAN").integration,
        StateIntegrationDef::Protectorate
    );
    assert_eq!(
        db.state_population(195).expect("USA oil state").integration,
        StateIntegrationDef::Metropole
    );
}

#[test]
fn p10_japanese_colonial_populations_are_reasonable() {
    let db = Historical1936Database::load().expect("history_1936 RON should load");

    let taiwan = db.state_population(524).expect("Taiwan population");
    assert!(
        (5_000_000..=6_500_000).contains(&taiwan.population),
        "Taiwan population should be around 1936-1942 historical scale, got {}",
        taiwan.population
    );
    assert_eq!(taiwan.integration, StateIntegrationDef::Colony);

    let southern_korea = db.state_population(525).expect("Southern Korea population");
    let northern_korea = db.state_population(527).expect("Northern Korea population");
    let korea_total = southern_korea.population + northern_korea.population;
    assert!(
        (22_000_000..=26_000_000).contains(&korea_total),
        "Korea population should stay near late-1930s scale, got {}",
        korea_total
    );
}

#[test]
fn p10_batch_c_state_population_covers_colonial_empires() {
    let db = Historical1936Database::load().expect("history_1936 RON should load");
    let required_states = [
        // French colonial empire: North Africa, West/Central Africa, Levant, Indochina.
        458, 459, 461, 462, 513, 514, 543, 553, 677, 670, 671, 741, // Dutch East Indies.
        334, 335, 672, 673, 669, // Belgian Congo.
        718, 719, // Portuguese Africa and Asian enclaves.
        540, 544, 296, 721, 729,
    ];

    for state_id in required_states {
        assert!(
            db.state_population(state_id).is_some(),
            "missing Phase 10 Batch C state population for state {}",
            state_id
        );
    }

    assert_eq!(
        db.state_population(459)
            .expect("French Algeria")
            .integration,
        StateIntegrationDef::Colony
    );
    assert_eq!(
        db.state_population(671)
            .expect("French Indochina")
            .integration,
        StateIntegrationDef::Protectorate
    );
    assert_eq!(
        db.state_population(335).expect("Java").integration,
        StateIntegrationDef::Colony
    );
    assert_eq!(
        db.state_population(718).expect("Belgian Congo").integration,
        StateIntegrationDef::Colony
    );
    assert_eq!(
        db.state_population(729).expect("Macau").integration,
        StateIntegrationDef::Concession
    );
}

#[test]
fn h0_country_profiles_have_valid_core_economic_shape() {
    let db = Historical1936Database::load().expect("history_1936 RON should load");
    let usa = db.country("USA").expect("USA profile");
    let ger = db.country("GER").expect("GER profile");
    let sov = db.country("SOV").expect("SOV profile");
    let chi = db.country("CHI").expect("CHI profile");

    assert!(usa.gdp_1936_gbp > ger.gdp_1936_gbp);
    assert!(ger.gdp_1936_gbp >= sov.gdp_1936_gbp * 0.9);
    assert!(chi.population > usa.population);
    assert!(chi.industrial_capacity_index < ger.industrial_capacity_index);

    for country in &db.countries {
        assert!(country.population > 0, "{} population", country.tag);
        assert!(country.gdp_1936_gbp > 0.0, "{} GDP", country.tag);
        assert!(
            (country.sector_shares.total() - 1.0).abs() < 0.001,
            "{} sector shares must sum to 1.0",
            country.tag
        );
    }
}

#[test]
fn h0_state_deposits_and_trade_routes_are_readable() {
    let db = Historical1936Database::load().expect("history_1936 RON should load");

    assert!(db.state_deposits.iter().any(|state| state
        .deposits
        .iter()
        .any(|deposit| deposit.good_id == "oil" && deposit.discovered_level > 0)));
    assert!(db.state_deposits.iter().all(|state| state
        .deposits
        .iter()
        .all(|deposit| deposit.discovered_level <= deposit.potential_level)));

    assert!(db.trade_routes.iter().any(|route| route.importer == "JAP"
        && route.good_id == "oil"
        && route.route_kind == TradeRouteKindDef::Sea));
    assert!(db.trade_routes.iter().any(|route| route.importer == "ENG"
        && route.route_kind == TradeRouteKindDef::ImperialPreference));
    assert!(db
        .trade_routes
        .iter()
        .any(|route| route.importer == "GER" && route.exporter == "ROM" && route.good_id == "oil"));
    assert!(db.trade_routes.iter().any(|route| route.importer == "JAP"
        && route.exporter == "MAN"
        && route.good_id == "iron"));
}

#[test]
fn h0_resource_deposits_cover_authored_mining_and_plantation_states() {
    let db = Historical1936Database::load().expect("history_1936 RON should load");
    let deposits_by_state = deposits_by_state(&db);

    for state in &db.state_populations {
        if matches!(state.workforce_profile, Some(WorkforceProfileDef::Mining)) {
            assert!(
                deposits_by_state.contains_key(&state.state_id),
                "mining workforce state {} lacks resource deposits",
                state.state_id
            );
            assert!(
                state_has_any_deposit(
                    &deposits_by_state,
                    state.state_id,
                    &["coal", "iron", "oil", "chromium", "bauxite", "tungsten"],
                ),
                "mining workforce state {} lacks strategic mineral deposits",
                state.state_id
            );
        }
        if matches!(
            state.workforce_profile,
            Some(WorkforceProfileDef::Plantation)
        ) {
            assert!(
                state_has_any_deposit(&deposits_by_state, state.state_id, &["rubber", "oil"]),
                "plantation workforce state {} lacks plantation/extraction deposits",
                state.state_id
            );
        }
    }

    let expected = [
        (8, "coal"),
        (54, "coal"),
        (51, "coal"),
        (28, "coal"),
        (29, "coal"),
        (91, "coal"),
        (92, "coal"),
        (93, "coal"),
        (195, "oil"),
        (202, "iron"),
        (276, "coal"),
        (229, "oil"),
        (307, "coal"),
        (305, "coal"),
        (718, "chromium"),
        (702, "rubber"),
        (543, "rubber"),
        (671, "rubber"),
        (334, "oil"),
        (335, "rubber"),
        (672, "oil"),
        (673, "rubber"),
    ];
    for (state_id, good_id) in expected {
        assert!(
            state_has_deposit(&deposits_by_state, state_id, good_id),
            "state {} should expose {} deposit",
            state_id,
            good_id
        );
    }
}

fn deposits_by_state(db: &Historical1936Database) -> HashMap<u16, HashSet<&str>> {
    db.state_deposits
        .iter()
        .map(|state| {
            (
                state.state_id,
                state
                    .deposits
                    .iter()
                    .map(|deposit| deposit.good_id.as_str())
                    .collect(),
            )
        })
        .collect()
}

fn state_has_deposit(
    deposits_by_state: &HashMap<u16, HashSet<&str>>,
    state_id: u16,
    good_id: &str,
) -> bool {
    deposits_by_state
        .get(&state_id)
        .map(|goods| goods.contains(good_id))
        .unwrap_or(false)
}

fn state_has_any_deposit(
    deposits_by_state: &HashMap<u16, HashSet<&str>>,
    state_id: u16,
    good_ids: &[&str],
) -> bool {
    deposits_by_state
        .get(&state_id)
        .map(|goods| good_ids.iter().any(|good| goods.contains(good)))
        .unwrap_or(false)
}

#[test]
fn h0_military_profiles_expose_budget_and_demand() {
    let db = Historical1936Database::load().expect("history_1936 RON should load");

    for country in &db.countries {
        let military = db
            .military_profiles
            .iter()
            .find(|profile| profile.tag == country.tag)
            .unwrap_or_else(|| panic!("{} military profile", country.tag));
        assert!(
            military.active_personnel > 0,
            "{} active personnel",
            country.tag
        );
        assert!(
            military.military_spending_gbp > 0.0,
            "{} military budget",
            country.tag
        );
        assert!(
            !military.equipment_demands.is_empty(),
            "{} demands",
            country.tag
        );
    }
}

#[test]
fn p11_china_1936_regional_profiles_are_complete() {
    let db = Historical1936Database::load().expect("history_1936 RON should load");
    let required_tags = [
        "CHI", "HBC", "SND", "PRC", "SHX", "GXC", "GDC", "YUN", "XAJ", "SIC", "XSM", "SIK", "TIB",
        "MAN", "MEN",
    ];

    for tag in required_tags {
        assert!(db.country(tag).is_some(), "missing {tag} country profile");
        assert!(
            db.head_of_state(tag).is_some(),
            "missing {tag} head of state"
        );
        assert!(
            db.military_profiles
                .iter()
                .any(|profile| profile.tag == tag),
            "missing {tag} military profile"
        );
    }

    let required_states = [
        283, 287, 322, 325, 591, 592, 593, 594, 597, 599, 601, 604, 608, 613, 614, 615, 616, 617,
        618, 619, 621, 622, 743, 744, 746, 747, 751, 753, 754, 755, 756, 757, 758, 759, 760, 1037,
        1038, 1039, 1041,
    ];
    for state_id in required_states {
        assert!(
            db.state_population(state_id).is_some(),
            "missing Phase 11 China population for state {state_id}"
        );
    }

    assert!(
        db.state_deposits.iter().any(|state| state.state_id == 615
            && state
                .deposits
                .iter()
                .any(|deposit| deposit.good_id == "coal")),
        "Shanxi should expose coal deposits"
    );
    assert!(
        db.state_deposits.iter().any(|state| state.state_id == 325
            && state
                .deposits
                .iter()
                .any(|deposit| deposit.good_id == "tungsten")),
        "Yunnan should expose tungsten deposits"
    );
}

#[test]
fn p11_initial_equipment_stockpiles_are_seeded_from_force_profiles() {
    let db = Historical1936Database::load().expect("history_1936 RON should load");
    let stockpiles = db.initial_equipment_stockpiles();

    for tag in ["GER", "SOV", "ENG", "FRA", "JAP", "ITA", "USA"] {
        let stock = stockpiles
            .get(tag)
            .unwrap_or_else(|| panic!("{tag} stockpile"));
        let infantry = stock.get("infantry_equipment").copied().unwrap_or(0.0);
        assert!(infantry >= 9_000.0, "{tag} infantry stockpile {infantry}");
    }

    let chi = stockpiles["CHI"]["infantry_equipment"];
    let prc = stockpiles["PRC"]["infantry_equipment"];
    assert!(chi > prc, "central China should outstock PRC");
    assert!(prc < 3_000.0, "PRC should still start equipment-poor");

    let japanese = stockpiles["JAP"]["infantry_equipment"];
    let chinese_front_total: f32 = [
        "CHI", "SND", "PRC", "SHX", "GXC", "GDC", "YUN", "XAJ", "SIC", "XSM", "SIK",
    ]
    .iter()
    .map(|tag| stockpiles[*tag]["infantry_equipment"])
    .sum();
    assert!(
        chinese_front_total < japanese * 0.55,
        "Chinese United Front infantry equipment {chinese_front_total} should stay well below Japan {japanese}"
    );
}

#[test]
fn h6_initial_law_profiles_reference_known_laws() {
    let history = Historical1936Database::load().expect("history_1936 RON should load");
    let db = V6Database::load();

    for country in &history.countries {
        assert!(
            db.conscription_laws
                .iter()
                .any(|law| law.id == country.initial_laws.conscription),
            "{} unknown conscription law {}",
            country.tag,
            country.initial_laws.conscription
        );
        assert!(
            db.economy_laws
                .iter()
                .any(|law| law.id == country.initial_laws.economy),
            "{} unknown economy law {}",
            country.tag,
            country.initial_laws.economy
        );
        assert!(
            db.trade_laws
                .iter()
                .any(|law| law.id == country.initial_laws.trade),
            "{} unknown trade law {}",
            country.tag,
            country.initial_laws.trade
        );
        assert!(
            db.taxation_laws
                .iter()
                .any(|law| law.id == country.initial_laws.taxation),
            "{} unknown taxation law {}",
            country.tag,
            country.initial_laws.taxation
        );
        assert!(
            db.civil_rights_laws
                .iter()
                .any(|law| law.id == country.initial_laws.civil_rights),
            "{} unknown civil rights law {}",
            country.tag,
            country.initial_laws.civil_rights
        );
        assert!(
            db.information_control_laws
                .iter()
                .any(|law| law.id == country.initial_laws.information_control),
            "{} unknown information control law {}",
            country.tag,
            country.initial_laws.information_control
        );
    }
}

#[test]
fn h6_major_country_laws_match_1936_systems() {
    let db = Historical1936Database::load().expect("history_1936 RON should load");

    assert_eq!(
        db.country("SOV").expect("SOV").initial_laws.economy,
        "planned_economy"
    );
    assert_eq!(
        db.country("SOV").expect("SOV").initial_laws.trade,
        "state_trade_monopoly"
    );
    assert_eq!(
        db.country("GER").expect("GER").initial_laws.economy,
        "corporatist_war_economy"
    );
    assert_eq!(
        db.country("USA").expect("USA").initial_laws.trade,
        "free_trade"
    );
    assert_eq!(
        db.country("ENG").expect("ENG").initial_laws.civil_rights,
        "open_society"
    );
}
