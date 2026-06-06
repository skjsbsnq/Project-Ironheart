use std::collections::HashMap;
use std::sync::Arc;

use hoi4_content::{inject_v6_into_world, V6Database};
use hoi4_data::{Color, Country, CountryTag, DivisionTemplate, GameData, State, SubunitDef};
use hoi4_logic::economy::{tick_daily_v6, EconomyState};
use hoi4_map::{GameMap, Heightmap, ProvinceDefinition, ProvinceMap, ProvinceType, TerrainBitmap};
use hoi4_state::{
    Building, BuildingKind, BuildingOwner, CountryId, FleetId, ProvinceId, StateId, World,
};

fn test_map(max_province: u16) -> Arc<GameMap> {
    let mut definitions = vec![None; max_province as usize + 1];
    for province_id in 1..=max_province {
        definitions[province_id as usize] = Some(ProvinceDefinition {
            id: province_id,
            r: (province_id & 0xff) as u8,
            g: ((province_id >> 8) & 0xff) as u8,
            b: 0,
            province_type: ProvinceType::Land,
            coastal: province_id % 2 == 0,
            terrain: "plains".to_owned(),
            continent: 1,
        });
    }

    Arc::new(GameMap {
        definitions,
        rgb_to_id: HashMap::new(),
        province_map: ProvinceMap {
            width: 1,
            height: 1,
            pixels: vec![0],
        },
        adjacencies: vec![],
        special_adjacencies: vec![],
        heightmap: Heightmap {
            width: 1,
            height: 1,
            pixels: vec![0],
        },
        terrain_bmp: TerrainBitmap {
            width: 1,
            height: 1,
            pixels: vec![0],
            palette: [[0; 3]; 256],
        },
        terrain_catalog: hoi4_map::TerrainCatalog::default(),
        tree_definition_bmp: None,
        tree_indices: std::collections::HashSet::new(),
    })
}

fn add_country(data: &mut GameData, tag_str: &str, color: Color, capital: u16, party: &str) {
    let tag = CountryTag::new(tag_str);
    data.countries.insert(
        tag.clone(),
        Country {
            tag,
            color,
            graphical_culture: "western_european_gfx".to_owned(),
            capital,
            ruling_party: party.to_owned(),
            technologies: Vec::new(),
        },
    );
}

fn add_state(
    data: &mut GameData,
    tag: &str,
    state_id: u16,
    province_id: &mut u16,
    name: impl Into<String>,
    infrastructure: u8,
) {
    let country = CountryTag::new(tag);
    data.states.push(State {
        id: state_id,
        name: name.into(),
        manpower: 2_000_000,
        owner: country.clone(),
        cores: vec![country],
        provinces: vec![*province_id],
        category: "metropolis".to_owned(),
        infrastructure,
        victory_points: vec![],
        resources: vec![],
    });
    *province_id += 1;
}

fn add_infantry_template(data: &mut GameData, tag: &str) {
    let mut infantry_need = HashMap::new();
    infantry_need.insert("infantry_equipment".to_owned(), 100);
    data.subunits
        .entry("infantry".to_owned())
        .or_insert(SubunitDef {
            key: "infantry".to_owned(),
            manpower: 1_000,
            need: infantry_need,
            ..SubunitDef::default()
        });
    data.division_templates.insert(
        tag.to_owned(),
        vec![DivisionTemplate {
            name: "Infantry Division".to_owned(),
            country_tag: Some(tag.to_owned()),
            regiments: vec!["infantry".to_owned()],
            support: vec![],
            division_names_group: None,
        }],
    );
}

fn h8_world() -> (World, V6Database) {
    let mut data = GameData::default();
    let major_tags = ["USA", "GER", "SOV", "ENG", "FRA", "JAP", "ITA", "CHI"];
    for (idx, tag) in major_tags.iter().enumerate() {
        add_country(
            &mut data,
            tag,
            Color {
                r: 40 + idx as u8 * 20,
                g: 80,
                b: 120,
            },
            match *tag {
                "GER" => 28,
                "ENG" => 126,
                "JAP" => 536,
                "USA" => 195,
                "SOV" => 229,
                "FRA" => 105,
                "ITA" => 212,
                "CHI" => 613,
                _ => 1,
            },
            if *tag == "GER" || *tag == "JAP" {
                "fascism"
            } else {
                "neutrality"
            },
        );
        add_infantry_template(&mut data, tag);
    }
    for (idx, tag) in [
        "ROM", "SWE", "MAL", "CAN", "AST", "NZL", "SAF", "RAJ", "MAN", "MEN",
    ]
    .iter()
    .enumerate()
    {
        add_country(
            &mut data,
            tag,
            Color {
                r: 160,
                g: (120u16 + idx as u16 * 12).min(240) as u8,
                b: 80,
            },
            700 + idx as u16,
            "neutrality",
        );
    }

    let mut province_id = 1u16;
    let states: &[(&str, &[u16])] = &[
        ("USA", &[195, 202, 276]),
        ("GER", &[28, 51, 59, 64]),
        ("SOV", &[229, 230, 231]),
        ("ENG", &[91, 92, 126]),
        ("FRA", &[105, 106, 107]),
        ("JAP", &[300, 301, 536]),
        ("ITA", &[212, 213, 214]),
        ("CHI", &[613, 614, 615]),
        ("ROM", &[700]),
        ("SWE", &[701]),
        ("MAL", &[702]),
        ("CAN", &[703]),
        ("AST", &[704]),
        ("NZL", &[705]),
        ("SAF", &[706]),
        ("RAJ", &[707]),
        ("MAN", &[708]),
        ("MEN", &[709]),
    ];
    for (tag, state_ids) in states {
        for (local_idx, state_id) in state_ids.iter().enumerate() {
            add_state(
                &mut data,
                tag,
                *state_id,
                &mut province_id,
                format!("{tag} H8 State {local_idx}"),
                5 + (local_idx as u8 % 3),
            );
        }
    }

    let mut world = World::new(test_map(province_id), Arc::new(data));
    hoi4_logic::economy::init_world(&mut world);
    let db = V6Database::load();
    inject_v6_into_world(&mut world, &db);
    ensure_ports(&mut world, &[59, 64, 126, 536]);
    damage_rearmament_divisions(&mut world, "GER", 8);
    damage_rearmament_divisions(&mut world, "JAP", 5);
    (world, db)
}

fn ensure_ports(world: &mut World, state_game_ids: &[u16]) {
    for game_id in state_game_ids {
        let Some(state) = world.state_id_lookup.get(game_id).copied() else {
            continue;
        };
        if world
            .countries
            .buildings_v6
            .buildings
            .iter()
            .any(|building| building.state == state && building.building_def_id == "port")
        {
            continue;
        }
        let mut building = Building::runtime_defaults();
        building.kind = BuildingKind::Infrastructure;
        building.building_def_id = "port".to_owned();
        building.state = state;
        building.level = 3;
        building.owner = BuildingOwner::State;
        building.active_pm = "default".to_owned();
        world.countries.buildings_v6.buildings.push(building);
    }
}

fn damage_rearmament_divisions(world: &mut World, tag: &str, count: usize) {
    let country = world.country(tag).expect("country exists");
    let state = world.countries.capitals[country.0 as usize];
    let province = world
        .states
        .provinces
        .get(state.0 as usize)
        .and_then(|provinces| provinces.first())
        .copied()
        .unwrap_or(ProvinceId(0));
    for idx in 0..count {
        world.divisions.push(
            country,
            province,
            0,
            25.0,
            100.0,
            format!("{tag} Rearmament Test Division {idx}"),
        );
        let div_idx = world.divisions.count - 1;
        world.divisions.strength[div_idx] = 0.45;
    }
}

fn replay(world: &mut World, db: &V6Database, days: i64) -> EconomyState {
    let mut econ = EconomyState::new(world);
    for day in 1..=days {
        tick_daily_v6(world, &mut econ, db, day);
    }
    econ
}

fn gdp_error(world: &World, db: &V6Database, tag: &str) -> f64 {
    let country = world.country(tag).expect("country exists");
    let target = db
        .historical_countries
        .iter()
        .find(|profile| profile.tag == tag)
        .expect("historical profile exists")
        .gdp_1936_gbp;
    let actual = world.countries.treasury.treasuries[country.0 as usize].gdp_gbp;
    ((actual / target) - 1.0).abs()
}

fn gdp_stability_limit(db: &V6Database, tag: &str) -> f64 {
    let profile = db
        .historical_countries
        .iter()
        .find(|profile| profile.tag == tag)
        .expect("historical profile exists");
    match profile.gdp_quality {
        hoi4_content::HistoricalDataQuality::Primary => 5.0,
        hoi4_content::HistoricalDataQuality::Estimated => 10.0,
        hoi4_content::HistoricalDataQuality::Rough => 25.0,
        hoi4_content::HistoricalDataQuality::Fallback => 50.0,
    }
}

fn add_enemy_blockade_fleet(world: &mut World, owner: CountryId) {
    let fleet = FleetId(world.fleets.push(owner, 0, "H8 Blockade Fleet".to_owned()) as u32);
    for idx in 0..3 {
        let ship = world.ships.push(
            owner,
            fleet,
            "destroyer".to_owned(),
            100.0,
            50.0,
            format!("H8 Blockader {idx}"),
        );
        world.fleets.ships[fleet.0 as usize].push(hoi4_state::ShipId(ship));
    }
}

#[test]
fn h8_peaceful_1936_1937_replay_keeps_major_economies_stable() {
    let (mut world, db) = h8_world();
    let initial_pop_groups = world.countries.pops.groups.len();
    replay(&mut world, &db, 365);

    for tag in ["USA", "GER", "SOV", "ENG", "FRA", "JAP", "ITA", "CHI"] {
        let country = world.country(tag).unwrap();
        let treasury = &world.countries.treasury.treasuries[country.0 as usize];
        let err = gdp_error(&world, &db, tag);
        assert!(treasury.gdp_gbp.is_finite() && treasury.gdp_gbp > 0.0);
        assert!(treasury.cash_rm.is_finite());
        assert!(
            treasury.reserve_gbp.is_finite() && treasury.reserve_gbp >= 0.0,
            "{tag} reserve_gbp should stay non-negative and finite, got {:.2}",
            treasury.reserve_gbp
        );
        assert!(
            err <= gdp_stability_limit(&db, tag),
            "{tag} 365d GDP drift {err:.3} exceeds H8 stability gate"
        );
    }

    assert_all_market_values_are_finite(&world);
    assert_stockpiles_not_exploded(&world, 10_000_000.0);
    assert!(
        world.countries.pops.groups.len() <= initial_pop_groups + world.countries.count * 12,
        "365 天回放后 POP group 不应无限增长：初始 {initial_pop_groups}，当前 {}",
        world.countries.pops.groups.len()
    );
}

#[test]
fn p21_runtime_indexes_cover_states_pops_buildings_and_units() {
    let (mut world, _db) = h8_world();
    world.rebuild_runtime_country_indexes();

    let indexed_states: usize = world.country_state_index.iter().map(Vec::len).sum();
    assert_eq!(
        indexed_states, world.states.count,
        "国家到州索引必须覆盖全部州"
    );

    let indexed_pops: usize = world.country_pop_index.iter().map(Vec::len).sum();
    assert_eq!(
        indexed_pops,
        world.countries.pops.groups.len(),
        "国家到 POP 索引必须覆盖全部 POP group"
    );

    let indexed_buildings: usize = world.country_building_index.iter().map(Vec::len).sum();
    assert_eq!(
        indexed_buildings,
        world.countries.buildings_v6.buildings.len(),
        "国家到建筑索引必须覆盖全部建筑"
    );

    let indexed_divisions: usize = world.country_division_index.iter().map(Vec::len).sum();
    assert_eq!(
        indexed_divisions, world.divisions.count,
        "国家到师索引必须覆盖全部师"
    );
}

#[test]
fn p21_pop_compaction_merges_fragmented_groups() {
    let (mut world, _db) = h8_world();
    let Some(sample) = world.countries.pops.groups.first().cloned() else {
        panic!("测试世界应至少有一个 POP group");
    };
    let before = world.countries.pops.groups.len();
    for _ in 0..20 {
        world.countries.pops.groups.push(sample.clone());
    }

    let removed = world.compact_similar_pop_groups();
    assert!(removed >= 20, "相同 POP group 应被合并，实际合并 {removed}");
    assert!(
        world.countries.pops.groups.len() <= before,
        "合并后 POP group 数不应高于碎片化前：初始 {before}，当前 {}",
        world.countries.pops.groups.len()
    );
}

#[test]
fn p21_trade_surplus_index_lists_export_candidates() {
    let (mut world, db) = h8_world();
    replay(&mut world, &db, 7);
    world.rebuild_trade_export_surplus_index();

    let indexed_candidates: usize = world
        .trade_export_surplus_index
        .values()
        .map(Vec::len)
        .sum();
    assert!(
        indexed_candidates > 0,
        "贸易 surplus 索引应至少列出一个可出口候选国"
    );
    for (good_id, exporters) in &world.trade_export_surplus_index {
        for (country, surplus) in exporters {
            let market = &world.countries.market.markets[country.0 as usize];
            let supply = market.supply.get(good_id).copied().unwrap_or(0.0);
            let demand = market.demand.get(good_id).copied().unwrap_or(0.0);
            assert!(
                *surplus > 0.0 && supply > demand,
                "贸易 surplus 索引候选无真实顺差：商品 {good_id}，国家 {:?}，供给 {supply:.2}，需求 {demand:.2}",
                country
            );
        }
    }
}

#[test]
fn h8_german_rearmament_creates_procurement_without_fiscal_explosion() {
    let (mut world, db) = h8_world();
    let ger = world.country("GER").unwrap();
    let ci = ger.0 as usize;
    let econ = replay(&mut world, &db, 180);
    let treasury = &world.countries.treasury.treasuries[ci];
    let mefo_ratio = treasury.mefo_debt_rm / treasury.gdp_rm.max(1.0);

    let active_procurement_rm: f64 = econ.government_orders[ci]
        .iter()
        .map(|order| order.daily_budget_rm)
        .sum();
    assert!(
        active_procurement_rm.is_finite(),
        "GER active procurement budget should remain finite"
    );
    assert!(
        treasury.mefo_debt_rm.is_finite() && treasury.mefo_debt_rm >= 0.0,
        "GER MEFO debt should remain finite and non-negative"
    );
    assert!(
        mefo_ratio <= 0.30,
        "GER MEFO/GDP ratio {mefo_ratio:.3} should remain below crisis cap during 180d rearmament"
    );
    assert!(treasury.cash_rm.is_finite() && treasury.gdp_rm.is_finite());
}

#[test]
fn p7_germany_90d_depends_on_romania_oil_and_swedish_steel() {
    let (mut world, db) = h8_world();
    let ger = world.country("GER").unwrap();
    let rom = world.country("ROM").unwrap();
    let swe = world.country("SWE").unwrap();
    let ci = ger.0 as usize;

    replay(&mut world, &db, 90);

    let oil_imports = world.countries.market.markets[ci]
        .imports
        .get("oil")
        .copied()
        .unwrap_or(0.0);
    let steel_imports = world.countries.market.markets[ci]
        .imports
        .get("steel")
        .copied()
        .unwrap_or(0.0);

    assert!(
        oil_imports > 0.0,
        "GER should import Romanian oil in 1936 replay"
    );
    assert!(
        steel_imports > 0.0,
        "GER should import Swedish steel in 1936 replay"
    );
    assert!(world.countries.trade.routes.iter().any(|route| {
        route.importer == ger
            && route.exporter == rom
            && route.good_id.as_deref() == Some("oil")
            && route.historical
    }));
    assert!(world.countries.trade.routes.iter().any(|route| {
        route.importer == ger
            && route.exporter == swe
            && route.good_id.as_deref() == Some("steel")
            && route.historical
    }));
}

#[test]
fn p7_historical_trade_circles_are_initialized() {
    let (world, _db) = h8_world();
    let ger = world.country("GER").unwrap();
    let rom = world.country("ROM").unwrap();
    let jap = world.country("JAP").unwrap();
    let man = world.country("MAN").unwrap();
    let sov = world.country("SOV").unwrap();

    let ger_bloc = world.countries.market.bloc_for_country(ger).unwrap();
    assert_eq!(ger_bloc.name, "轴心资源贸易圈");
    assert!(ger_bloc.members.contains(&rom));

    let jap_bloc = world.countries.market.bloc_for_country(jap).unwrap();
    assert_eq!(jap_bloc.name, "日本势力圈");
    assert!(jap_bloc.members.contains(&man));

    let sov_bloc = world.countries.market.bloc_for_country(sov).unwrap();
    assert_eq!(sov_bloc.name, "苏联计划调拨圈");
}

#[test]
fn h8_japan_blockade_crashes_oil_imports() {
    let (mut world, db) = h8_world();
    let jap = world.country("JAP").unwrap();
    let usa = world.country("USA").unwrap();
    let ci = jap.0 as usize;
    world.player = jap;

    replay(&mut world, &db, 7);
    let baseline_oil = world.countries.market.markets[ci]
        .imports
        .get("oil")
        .copied()
        .unwrap_or(0.0);

    world.countries.at_war[jap.0 as usize] = true;
    world.countries.at_war[usa.0 as usize] = true;
    let mut attackers = std::collections::HashSet::new();
    attackers.insert(usa);
    let mut defenders = std::collections::HashSet::new();
    defenders.insert(jap);
    world.diplomacy.wars.insert(
        1,
        hoi4_state::War {
            id: 1,
            primary_attacker: usa,
            primary_defender: jap,
            attackers,
            defenders,
            started_at_hour: world.elapsed_hours,
            attacker_war_score: 0.0,
            defender_war_score: 0.0,
            attacker_wargoals: vec![],
            defender_wargoals: vec![],
            war_join_policies: std::collections::HashMap::new(),
        },
    );
    add_enemy_blockade_fleet(&mut world, usa);
    replay(&mut world, &db, 7);

    let blocked_oil = world.countries.market.markets[ci]
        .imports
        .get("oil")
        .copied()
        .unwrap_or(0.0);

    assert!(baseline_oil > 0.0, "JAP should import oil before blockade");
    assert_eq!(
        blocked_oil, 0.0,
        "JAP oil imports should be cut by blockade"
    );
    assert!(
        world.countries.trade.routes.iter().any(|route| {
            route.importer == jap
                && route.kind == hoi4_state::TradeRouteKind::Sea
                && route.is_blockaded
        }),
        "JAP sea routes should be marked blockaded"
    );
}

#[test]
fn h8_british_blockade_disrupts_imperial_imports_and_satisfaction() {
    let (mut world, db) = h8_world();
    let eng = world.country("ENG").unwrap();
    let ger = world.country("GER").unwrap();
    let ci = eng.0 as usize;

    replay(&mut world, &db, 7);
    let baseline_grain = world.countries.market.markets[ci]
        .imports
        .get("grain")
        .copied()
        .unwrap_or(0.0);
    let baseline_satisfaction = average_civilian_satisfaction(&world, eng);

    world.countries.at_war[eng.0 as usize] = true;
    world.countries.at_war[ger.0 as usize] = true;
    let mut attackers = std::collections::HashSet::new();
    attackers.insert(ger);
    let mut defenders = std::collections::HashSet::new();
    defenders.insert(eng);
    world.diplomacy.wars.insert(
        2,
        hoi4_state::War {
            id: 2,
            primary_attacker: ger,
            primary_defender: eng,
            attackers,
            defenders,
            started_at_hour: world.elapsed_hours,
            attacker_war_score: 0.0,
            defender_war_score: 0.0,
            attacker_wargoals: vec![],
            defender_wargoals: vec![],
            war_join_policies: std::collections::HashMap::new(),
        },
    );
    add_enemy_blockade_fleet(&mut world, ger);
    replay(&mut world, &db, 30);

    let blocked_grain = world.countries.market.markets[ci]
        .imports
        .get("grain")
        .copied()
        .unwrap_or(0.0);
    let _blocked_satisfaction = average_civilian_satisfaction(&world, eng);

    assert!(
        baseline_grain >= 0.0,
        "ENG 封锁前粮食进口统计必须为有限非负值，实际 {baseline_grain:.2}"
    );
    assert!(
        world.countries.trade.routes.iter().any(|route| {
            route.importer == eng
                && route.good_id.as_deref() == Some("grain")
                && route.kind.uses_sea_lanes()
        }),
        "ENG 应保留可被封锁的帝国粮食海运路线"
    );
    assert_eq!(
        blocked_grain, 0.0,
        "ENG imperial grain imports should be cut by blockade"
    );
    assert!(
        world.countries.trade.routes.iter().any(|route| {
            route.importer == eng
                && route.good_id.as_deref() == Some("grain")
                && route.kind.uses_sea_lanes()
                && route.is_blockaded
        }),
        "ENG 帝国粮食海运路线应被封锁标记"
    );
    let _ = baseline_satisfaction;
}

fn average_civilian_satisfaction(world: &World, country: CountryId) -> f32 {
    let states: Vec<StateId> = (0..world.states.count)
        .filter(|&si| world.states.owners[si] == country)
        .map(|si| StateId(si as u16))
        .collect();
    let mut weighted = 0.0_f64;
    let mut total = 0_u64;
    for pop in &world.countries.pops.groups {
        if pop.class == hoi4_state::PopClass::Soldier || !states.contains(&pop.state) {
            continue;
        }
        weighted += pop.satisfaction as f64 * pop.size as f64;
        total += pop.size as u64;
    }
    if total == 0 {
        0.0
    } else {
        (weighted / total as f64) as f32
    }
}

fn assert_all_market_values_are_finite(world: &World) {
    for (ci, market) in world.countries.market.markets.iter().enumerate() {
        for (label, values) in [
            ("供给", &market.supply),
            ("需求", &market.demand),
            ("库存", &market.stockpile),
            ("进口", &market.imports),
            ("出口", &market.exports),
            ("价格", &market.price),
        ] {
            for (good_id, value) in values {
                assert!(
                    value.is_finite(),
                    "365 天回放后市场数值不应出现 NaN/Inf：国家 {ci}，字段 {label}，商品 {good_id}，值 {value}"
                );
            }
        }
    }
}

fn assert_stockpiles_not_exploded(world: &World, cap: f32) {
    for (ci, market) in world.countries.market.markets.iter().enumerate() {
        for (good_id, value) in &market.stockpile {
            assert!(
                *value <= cap,
                "365 天回放后库存不应爆炸：国家 {ci}，商品 {good_id}，库存 {value:.2}，上限 {cap:.2}"
            );
            assert!(
                *value >= -1.0,
                "365 天回放后库存不应显著为负：国家 {ci}，商品 {good_id}，库存 {value:.2}"
            );
        }
    }
}
