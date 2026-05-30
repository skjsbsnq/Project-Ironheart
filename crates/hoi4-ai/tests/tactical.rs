//! Phase 4.2 战术 AI 单元测试。
//!
//! 测试用最小化 World：手工构造 `GameMap` + `GameData`，然后绕过 `World::new` 直接
//! 通过结构体字面量构造，避免依赖真实 HOI4 安装路径。

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use hoi4_ai::{
    air, compute_front, compute_segment, detect_encirclement_opportunities, evaluate_air,
    evaluate_ground, evaluate_naval, AiProfile, AirDecision, FrontLine, GroundPosture,
};
use hoi4_data::air::AircraftDef;
use hoi4_data::naval::ShipClassDef;
use hoi4_data::{AircraftKind, GameData, ShipKind};
use hoi4_map::adjacency::Adjacency;
use hoi4_map::definition::{ProvinceDefinition, ProvinceType};
use hoi4_map::provinces::ProvinceMap;
use hoi4_map::terrain::Heightmap;
use hoi4_map::terrain_bmp::TerrainBitmap;
use hoi4_map::terrain_catalog::TerrainCatalog;
use hoi4_map::GameMap;
use hoi4_state::store::{
    AirWingStore, CountryStore, DivisionStore, FleetStore, ProvinceStore, ShipStore, StateStore,
};
use hoi4_state::{
    AirMission, CountryId, DiplomacyState, Faction, FactionId, GameDate, GameSpeed, NavalMission,
    ProvinceId, StateId, War, World,
};

// ─── 最小化构造工具 ────────────────────────────────────────

fn empty_data() -> GameData {
    GameData {
        countries: HashMap::new(),
        states: vec![],
        province_owners: HashMap::new(),
        buildings: HashMap::new(),
        resources: HashMap::new(),
        equipment: HashMap::new(),
        technologies: HashMap::new(),
        tech_prereqs: HashMap::new(),
        ideologies: HashMap::new(),
        ideas: HashMap::new(),
        focus_trees: HashMap::new(),
        focus_to_tree: HashMap::new(),
        subunits: HashMap::new(),
        combat_tactics: HashMap::new(),
        division_templates: HashMap::new(),
        ship_classes: HashMap::new(),
        aircraft: HashMap::new(),
        oob_land: HashMap::new(),
        oob_naval: HashMap::new(),
        oob_air: HashMap::new(),
        country_histories: HashMap::new(),
        decision_categories: HashMap::new(),
        decisions: HashMap::new(),
        ..Default::default()
    }
}

fn empty_map(province_count: usize) -> GameMap {
    let mut definitions: Vec<Option<ProvinceDefinition>> = (0..=province_count)
        .map(|i| {
            if i == 0 {
                None
            } else {
                Some(ProvinceDefinition {
                    id: i as u16,
                    r: 0,
                    g: 0,
                    b: 0,
                    province_type: ProvinceType::Land,
                    coastal: false,
                    terrain: String::new(),
                    continent: 0,
                })
            }
        })
        .collect();
    // ensure index province_count exists
    if definitions.len() < province_count + 1 {
        definitions.resize(province_count + 1, None);
    }

    GameMap {
        definitions,
        rgb_to_id: HashMap::new(),
        province_map: ProvinceMap {
            width: 0,
            height: 0,
            pixels: vec![],
        },
        adjacencies: vec![Vec::new(); province_count + 1],
        special_adjacencies: Vec::<Adjacency>::new(),
        heightmap: Heightmap {
            width: 0,
            height: 0,
            pixels: vec![],
        },
        terrain_bmp: TerrainBitmap {
            width: 0,
            height: 0,
            pixels: vec![],
            palette: [[0; 3]; 256],
        },
        terrain_catalog: TerrainCatalog::default(),
        tree_definition_bmp: None,
        tree_indices: hoi4_map::DEFAULT_TREE_INDICES.iter().copied().collect(),
    }
}

/// 构造一个测试 World：N 个 country，M 个 province，K 个 state。
/// 调用方需手动设置 ownership / controllers / adjacencies / 师 / 舰队 / 联队等。
fn make_world(country_count: usize, province_count: usize, state_count: usize) -> World {
    let map = Arc::new(empty_map(province_count));
    let data = Arc::new(empty_data());

    let provinces = ProvinceStore::new(province_count + 1);
    let mut states = StateStore::new(state_count);
    let mut countries = CountryStore::new(country_count);
    for i in 0..country_count {
        countries.tags[i] = format!("C{}", i);
    }
    for si in 0..state_count {
        states.names[si] = format!("S{}", si);
    }

    World {
        date: GameDate::START,
        speed: GameSpeed::Paused,
        elapsed_hours: 0,
        provinces,
        states,
        countries,
        divisions: DivisionStore::new(),
        ships: ShipStore::new(),
        fleets: FleetStore::new(),
        air_wings: AirWingStore::new(),
        diplomacy: DiplomacyState::new(),
        command: hoi4_state::CommandHierarchy::default(),
        generals: Vec::new(),
        next_general_id: 0,
        map,
        data,
        tag_to_country: HashMap::new(),
        state_id_lookup: HashMap::new(),
        player: CountryId::NONE,
        random_seed: 0xC0FFEE,
        game_unique_id: 1,
        path_cache: HashMap::new(),
        path_cache_day: 0,
        prov_div_index: HashMap::new(),
        country_state_index: vec![Vec::new(); country_count],
        country_pop_index: vec![Vec::new(); country_count],
        country_building_index: vec![Vec::new(); country_count],
        country_division_index: vec![Vec::new(); country_count],
        country_fleet_index: vec![Vec::new(); country_count],
        country_air_wing_index: vec![Vec::new(); country_count],
        runtime_country_indexes_valid: false,
        trade_export_surplus_index: HashMap::new(),
        player_armies: Vec::new(),
        player_locked_divisions: std::collections::HashSet::new(),
        next_army_id: 0,
    }
}

fn set_province(world: &mut World, prov: u16, state: u16, owner: CountryId) {
    let pi = prov as usize;
    world.provinces.owners[pi] = owner;
    world.provinces.controllers[pi] = owner;
    world.provinces.state_of[pi] = StateId(state);
    let si = state as usize;
    world.states.owners[si] = owner;
    world.states.controllers[si] = owner;
    if !world.states.provinces[si].iter().any(|p| p.0 == prov) {
        world.states.provinces[si].push(ProvinceId(prov));
    }
}

/// 设置陆地相邻：双向 a ↔ b。
fn add_adjacency(map: &mut GameMap, a: u16, b: u16) {
    if !map.adjacencies[a as usize].contains(&b) {
        map.adjacencies[a as usize].push(b);
    }
    if !map.adjacencies[b as usize].contains(&a) {
        map.adjacencies[b as usize].push(a);
    }
}

/// 加入一个简单战争：c1 vs c2。
fn put_at_war(world: &mut World, c1: CountryId, c2: CountryId) {
    let mut atk = HashSet::new();
    atk.insert(c1);
    let mut def = HashSet::new();
    def.insert(c2);
    let id = world.diplomacy.next_war_id;
    world.diplomacy.next_war_id += 1;
    world.diplomacy.wars.insert(
        id,
        War {
            id,
            primary_attacker: c1,
            primary_defender: c2,
            attackers: atk,
            defenders: def,
            started_at_hour: 0,
            attacker_war_score: 0.0,
            defender_war_score: 0.0,
            attacker_wargoals: vec![],
            defender_wargoals: vec![],
            war_join_policies: std::collections::HashMap::new(),
        },
    );
    world.countries.at_war[c1.0 as usize] = true;
    world.countries.at_war[c2.0 as usize] = true;
}

// ─── frontline 测试 ────────────────────────────────────────

#[test]
fn frontline_no_war_returns_empty() {
    let world = make_world(2, 4, 2);
    let f = compute_front(&world, CountryId(0));
    assert!(!f.has_front());
    assert!(f.segments.is_empty());
}

#[test]
fn frontline_detects_adjacent_states() {
    // 两个州，4 个省份，省份 1-2 在 state 0 (C0)；省份 3-4 在 state 1 (C1)。
    // 邻接：2 ↔ 3。
    let mut world = make_world(2, 4, 2);
    let c0 = CountryId(0);
    let c1 = CountryId(1);

    set_province(&mut world, 1, 0, c0);
    set_province(&mut world, 2, 0, c0);
    set_province(&mut world, 3, 1, c1);
    set_province(&mut world, 4, 1, c1);

    // 注意：map 在 Arc 中，需要 mutate 一份新的（更简单：直接 hack 进 mutable Arc::get_mut）
    let map_mut = Arc::get_mut(&mut world.map).expect("Arc owned");
    add_adjacency(map_mut, 2, 3);

    put_at_war(&mut world, c0, c1);

    let front = compute_front(&world, c0);
    assert!(front.has_front());
    assert_eq!(front.segments.len(), 1);
    let seg = &front.segments[0];
    assert_eq!(seg.enemy, c1);
    assert_eq!(seg.friendly_states, vec![StateId(0)]);
    assert_eq!(seg.enemy_states, vec![StateId(1)]);
}

#[test]
fn frontline_uses_military_access_staging_states() {
    let mut world = make_world(3, 2, 2);
    let jap = CountryId(0);
    let man = CountryId(1);
    let chi = CountryId(2);

    set_province(&mut world, 1, 0, man);
    set_province(&mut world, 2, 1, chi);
    let map_mut = Arc::get_mut(&mut world.map).expect("Arc owned");
    add_adjacency(map_mut, 1, 2);

    put_at_war(&mut world, jap, chi);
    world.diplomacy.grant_military_access(man, jap);

    let front = compute_front(&world, jap);
    assert!(front.has_front());
    let seg = front.segment_against(chi).expect("china front");
    assert_eq!(seg.friendly_states, vec![StateId(0)]);
    assert_eq!(seg.enemy_states, vec![StateId(1)]);

    let eval = evaluate_ground(&world, jap, &front, &AiProfile::default());
    let decision = eval.against(chi).expect("ground decision");
    assert!(!decision.sectors.is_empty());
    assert_eq!(decision.sectors[0].attack_from, vec![ProvinceId(1)]);
    assert_eq!(decision.sectors[0].target_provinces, vec![ProvinceId(2)]);
}

#[test]
fn frontline_reports_peace_border_for_non_war_neighbor() {
    // 两国相邻但未交战时仍返回和平边境，用于战前集结。
    let mut world = make_world(2, 4, 2);
    let c0 = CountryId(0);
    let c1 = CountryId(1);

    set_province(&mut world, 1, 0, c0);
    set_province(&mut world, 2, 0, c0);
    set_province(&mut world, 3, 1, c1);
    set_province(&mut world, 4, 1, c1);

    let map_mut = Arc::get_mut(&mut world.map).unwrap();
    add_adjacency(map_mut, 2, 3);

    let front = compute_front(&world, c0);
    assert!(front.has_front());
    let seg = front.segment_against(c1).expect("peace border front");
    assert_eq!(seg.friendly_states, vec![StateId(0)]);
    assert_eq!(seg.enemy_states, vec![StateId(1)]);
}

// ─── encirclement 检测 ────────────────────────────────────

#[test]
fn encirclement_detects_state_surrounded_by_us() {
    // 4 个州、9 个省份。中心 state(1) 被 C1 控制，四周 state(0/2/3/4) 都被 C0 控制。
    // 中心：1 个省份 5；四周：4 个省份 1,2,3,4 各属一个 state。
    // 邻接：5↔1, 5↔2, 5↔3, 5↔4。
    let mut world = make_world(2, 5, 5);
    let c0 = CountryId(0);
    let c1 = CountryId(1);

    set_province(&mut world, 1, 0, c0);
    set_province(&mut world, 5, 1, c1);
    set_province(&mut world, 2, 2, c0);
    set_province(&mut world, 3, 3, c0);
    set_province(&mut world, 4, 4, c0);

    {
        let map_mut = Arc::get_mut(&mut world.map).unwrap();
        for n in [1, 2, 3, 4] {
            add_adjacency(map_mut, 5, n);
        }
    }

    put_at_war(&mut world, c0, c1);

    // 用 ENCIRCLEMENT_RATIO 默认 0.5 — 中心 state 4/4 邻接被 C0 控制 → ratio = 1.0
    let opps = detect_encirclement_opportunities(&world, c0, 0.5);
    assert_eq!(opps.len(), 1);
    assert_eq!(opps[0].state, StateId(1));
    assert!((opps[0].ratio - 1.0).abs() < 1e-6);
}

// ─── ground 决策 ────────────────────────────────────────

#[test]
fn ground_no_war_no_decisions() {
    let world = make_world(2, 4, 2);
    let front = FrontLine::empty(CountryId(0));
    let eval = evaluate_ground(&world, CountryId(0), &front, &AiProfile::default());
    assert!(eval.decisions.is_empty());
}

#[test]
fn ground_attack_when_outnumbering() {
    // 与 frontline_detects_adjacent_states 相同的设置 + 加几个师让 C0 优势
    let mut world = make_world(2, 4, 2);
    let c0 = CountryId(0);
    let c1 = CountryId(1);

    set_province(&mut world, 1, 0, c0);
    set_province(&mut world, 2, 0, c0);
    set_province(&mut world, 3, 1, c1);
    set_province(&mut world, 4, 1, c1);

    {
        let map_mut = Arc::get_mut(&mut world.map).unwrap();
        add_adjacency(map_mut, 2, 3);
    }
    put_at_war(&mut world, c0, c1);

    // C0 拥有 5 个满员师；C1 1 个
    for _ in 0..5 {
        world
            .divisions
            .push(c0, ProvinceId(2), 0, 60.0, 1000.0, "atk".into());
    }
    world
        .divisions
        .push(c1, ProvinceId(3), 0, 60.0, 1000.0, "def".into());

    let front = compute_front(&world, c0);
    let eval = evaluate_ground(&world, c0, &front, &AiProfile::default());
    assert_eq!(eval.decisions.len(), 1);
    assert_eq!(eval.decisions[0].posture, GroundPosture::Attack);
    assert!(eval.decisions[0].ratio > 1.0);
    assert!(!eval.decisions[0].sectors.is_empty());
}

#[test]
fn ground_sector_prefers_local_breakthrough_over_global_average() {
    let mut world = make_world(2, 6, 4);
    let c0 = CountryId(0);
    let c1 = CountryId(1);

    set_province(&mut world, 1, 0, c0);
    set_province(&mut world, 2, 1, c0);
    set_province(&mut world, 3, 2, c1);
    set_province(&mut world, 4, 3, c1);
    set_province(&mut world, 5, 3, c1);
    set_province(&mut world, 6, 3, c1);

    {
        let map_mut = Arc::get_mut(&mut world.map).unwrap();
        add_adjacency(map_mut, 1, 3);
        add_adjacency(map_mut, 2, 4);
    }
    put_at_war(&mut world, c0, c1);

    for _ in 0..4 {
        world
            .divisions
            .push(c0, ProvinceId(1), 0, 60.0, 1000.0, "local_adv".into());
    }
    world
        .divisions
        .push(c1, ProvinceId(3), 0, 60.0, 1000.0, "thin".into());
    for _ in 0..6 {
        world
            .divisions
            .push(c1, ProvinceId(4), 0, 60.0, 1000.0, "strong".into());
    }

    let front = compute_front(&world, c0);
    let eval = evaluate_ground(&world, c0, &front, &AiProfile::default());
    assert_eq!(eval.decisions.len(), 1);
    let d = &eval.decisions[0];
    assert!(
        d.ratio < 1.0,
        "global ratio should remain weak: {}",
        d.ratio
    );
    assert_eq!(d.posture, GroundPosture::Attack, "reason: {}", d.reason);
    assert_eq!(d.sectors[0].enemy_state, StateId(2));
    assert!(d.sectors[0].ratio > 1.0, "best sector: {:?}", d.sectors[0]);
}

/// 回归测试 — Bug #1/#2 修复：力量比处于 [RETREAT_RATIO, aggressive_threshold)
/// 区间时应进入 Defend 姿态（修复前会被硬编码 0.60 阈值强制 Attack）。
#[test]
fn ground_defend_when_slightly_weaker() {
    // C0: 5 师 vs C1: 6 师 → ratio ≈ 0.833
    // 在默认 aggression=0.5 下 aggressive_threshold = 1.30，
    // 且 ratio < 1.04（encirclement 旁路阈值），ratio ≥ 0.70（RETREAT_RATIO）→ 应 Defend。
    let mut world = make_world(2, 4, 2);
    let c0 = CountryId(0);
    let c1 = CountryId(1);

    set_province(&mut world, 1, 0, c0);
    set_province(&mut world, 2, 0, c0);
    set_province(&mut world, 3, 1, c1);
    set_province(&mut world, 4, 1, c1);

    {
        let map_mut = Arc::get_mut(&mut world.map).unwrap();
        add_adjacency(map_mut, 2, 3);
    }
    put_at_war(&mut world, c0, c1);

    for _ in 0..5 {
        world
            .divisions
            .push(c0, ProvinceId(2), 0, 60.0, 1000.0, "def_c0".into());
    }
    for _ in 0..6 {
        world
            .divisions
            .push(c1, ProvinceId(3), 0, 60.0, 1000.0, "atk_c1".into());
    }

    let front = compute_front(&world, c0);
    let eval = evaluate_ground(&world, c0, &front, &AiProfile::default());
    assert_eq!(eval.decisions.len(), 1);
    let d = &eval.decisions[0];
    assert!(
        d.ratio >= 0.70 && d.ratio < 1.04,
        "ratio {} 应落在 Defend 区间内",
        d.ratio
    );
    assert_eq!(
        d.posture,
        GroundPosture::Defend,
        "ratio={} 期望 Defend，实际 {:?}（reason: {}）",
        d.ratio,
        d.posture,
        d.reason
    );
}

#[test]
fn ground_retreat_when_severely_outnumbered() {
    // C0: 1 师 vs C1: 5 师 → ratio = 0.2 < RETREAT_RATIO → Retreat
    let mut world = make_world(2, 4, 2);
    let c0 = CountryId(0);
    let c1 = CountryId(1);

    set_province(&mut world, 1, 0, c0);
    set_province(&mut world, 2, 0, c0);
    set_province(&mut world, 3, 1, c1);
    set_province(&mut world, 4, 1, c1);

    {
        let map_mut = Arc::get_mut(&mut world.map).unwrap();
        add_adjacency(map_mut, 2, 3);
    }
    put_at_war(&mut world, c0, c1);

    world
        .divisions
        .push(c0, ProvinceId(2), 0, 60.0, 1000.0, "weak".into());
    for _ in 0..5 {
        world
            .divisions
            .push(c1, ProvinceId(3), 0, 60.0, 1000.0, "strong".into());
    }

    let front = compute_front(&world, c0);
    let eval = evaluate_ground(&world, c0, &front, &AiProfile::default());
    assert_eq!(eval.decisions.len(), 1);
    assert_eq!(eval.decisions[0].posture, GroundPosture::Retreat);
}

// ─── naval 决策 ────────────────────────────────────────

#[test]
fn naval_submarine_fleet_raids_when_at_war() {
    let mut world = make_world(2, 1, 1);
    let c0 = CountryId(0);
    let c1 = CountryId(1);

    // 注册 ship class
    {
        let data_mut = Arc::get_mut(&mut world.data).unwrap();
        data_mut.ship_classes.insert(
            "submarine".to_string(),
            ShipClassDef {
                key: "submarine".into(),
                kind: ShipKind::Submarine,
                max_hp: 100.0,
                max_organisation: 30.0,
                naval_attack: 0.0,
                torpedo_attack: 5.0,
                sub_attack: 0.0,
                anti_air_attack: 0.0,
                armor: 0.0,
                armor_piercing: 0.0,
                speed: 18.0,
                surface_visibility: 5.0,
                sub_visibility: 1.0,
                carrier_size: 0,
                supply_consumption: 1.0,
                manpower: 50,
            },
        );
    }

    let fleet_idx = world.fleets.push(c0, 10, "U-Flotte".into());
    // 加 5 艘潜艇（直接操作 ShipStore）
    for i in 0..5 {
        let ship_idx = world.ships.push(
            c0,
            hoi4_state::FleetId(fleet_idx),
            "submarine".into(),
            100.0,
            30.0,
            format!("U{}", i),
        );
        world.fleets.ships[fleet_idx as usize].push(hoi4_state::ShipId(ship_idx));
    }

    put_at_war(&mut world, c0, c1);

    let decisions = evaluate_naval(&world, c0, &AiProfile::default());
    assert_eq!(decisions.len(), 1);
    assert_eq!(decisions[0].mission, NavalMission::ConvoyRaiding);
}

#[test]
fn naval_transport_fleet_supports_coastal_invasion() {
    let mut world = make_world(2, 2, 2);
    let c0 = CountryId(0);
    let c1 = CountryId(1);
    set_province(&mut world, 1, 0, c0);
    set_province(&mut world, 2, 1, c1);
    Arc::get_mut(&mut world.map).unwrap().definitions[2]
        .as_mut()
        .unwrap()
        .coastal = true;

    {
        let data_mut = Arc::get_mut(&mut world.data).unwrap();
        data_mut.ship_classes.insert(
            "transport".to_string(),
            ShipClassDef {
                key: "transport".into(),
                kind: ShipKind::Transport,
                max_hp: 50.0,
                max_organisation: 10.0,
                naval_attack: 0.0,
                torpedo_attack: 0.0,
                sub_attack: 0.0,
                anti_air_attack: 0.0,
                armor: 0.0,
                armor_piercing: 0.0,
                speed: 12.0,
                surface_visibility: 20.0,
                sub_visibility: 10.0,
                carrier_size: 0,
                supply_consumption: 1.0,
                manpower: 20,
            },
        );
    }

    let fleet_idx = world.fleets.push(c0, 10, "Transport Fleet".into());
    let ship_idx = world.ships.push(
        c0,
        hoi4_state::FleetId(fleet_idx),
        "transport".into(),
        50.0,
        10.0,
        "Transport 1".into(),
    );
    world.fleets.ships[fleet_idx as usize].push(hoi4_state::ShipId(ship_idx));

    put_at_war(&mut world, c0, c1);

    let decisions = evaluate_naval(&world, c0, &AiProfile::default());
    assert_eq!(decisions.len(), 1);
    assert_eq!(decisions[0].mission, NavalMission::NavalInvasionSupport);
}

// ─── air 决策 ────────────────────────────────────────

#[test]
fn air_strategic_bomber_strategic_bombing() {
    let mut world = make_world(2, 1, 1);
    let c0 = CountryId(0);
    let c1 = CountryId(1);

    {
        let data_mut = Arc::get_mut(&mut world.data).unwrap();
        data_mut.aircraft.insert(
            "strategic_bomber".to_string(),
            AircraftDef {
                key: "strategic_bomber".into(),
                kind: AircraftKind::StrategicBomber,
                max_hp: 35.0,
                max_organisation: 50.0,
                air_attack: 5.0,
                air_defense: 5.0,
                agility: 30.0,
                speed: 350.0,
                range: 4000.0,
                air_bombing: 5.0,
                naval_strike: 0.0,
                strategic_bombing: 30.0,
                build_cost_ic: 200.0,
                manpower: 60,
                supply_consumption: 0.6,
            },
        );
        data_mut.aircraft.insert(
            "fighter".to_string(),
            AircraftDef {
                key: "fighter".into(),
                kind: AircraftKind::Fighter,
                max_hp: 30.0,
                max_organisation: 50.0,
                air_attack: 30.0,
                air_defense: 20.0,
                agility: 80.0,
                speed: 450.0,
                range: 800.0,
                air_bombing: 0.0,
                naval_strike: 0.0,
                strategic_bombing: 0.0,
                build_cost_ic: 30.0,
                manpower: 20,
                supply_consumption: 0.2,
            },
        );
    }

    // 一个战略轰炸机联队 + 一个战斗机联队
    world
        .air_wings
        .push(c0, "strategic_bomber".into(), 5, 100, 50.0, "9.JG".into());
    world
        .air_wings
        .push(c0, "fighter".into(), 5, 100, 50.0, "1.JG".into());

    put_at_war(&mut world, c0, c1);

    let decisions: Vec<AirDecision> = evaluate_air(&world, c0, &AiProfile::default());
    assert_eq!(decisions.len(), 2);

    let strat = decisions.iter().find(|d| d.wing.0 == 0).unwrap();
    assert_eq!(strat.mission, AirMission::StrategicBombing);

    let fighter = decisions.iter().find(|d| d.wing.0 == 1).unwrap();
    // 没有敌军飞机 → 制空已 100% 但默认走 AirSuperiority （air_ctrl == 1.0 ≥ HIGH_THRESHOLD →
    // Interception）
    assert!(matches!(
        fighter.mission,
        AirMission::AirSuperiority | AirMission::Interception
    ));
}

#[test]
fn air_apply_writes_back_mission() {
    let mut world = make_world(1, 1, 1);
    let c0 = CountryId(0);
    {
        let data_mut = Arc::get_mut(&mut world.data).unwrap();
        data_mut.aircraft.insert(
            "fighter".to_string(),
            AircraftDef {
                key: "fighter".into(),
                kind: AircraftKind::Fighter,
                max_hp: 30.0,
                max_organisation: 50.0,
                air_attack: 30.0,
                air_defense: 20.0,
                agility: 80.0,
                speed: 450.0,
                range: 800.0,
                air_bombing: 0.0,
                naval_strike: 0.0,
                strategic_bombing: 0.0,
                build_cost_ic: 30.0,
                manpower: 20,
                supply_consumption: 0.2,
            },
        );
    }

    world
        .air_wings
        .push(c0, "fighter".into(), 1, 50, 50.0, "JG".into());

    let decisions = evaluate_air(&world, c0, &AiProfile::default());
    air::apply_air_decisions(&mut world, &decisions);
    assert_eq!(world.air_wings.mission[0], decisions[0].mission);
}

// 让 unused import 警告闭嘴：这两个被 frontline_detects_adjacent_states 用到。
#[allow(dead_code)]
fn _link_other(f: &Faction, _: FactionId) -> usize {
    f.members.len()
}

#[test]
fn compute_segment_consistent_with_compute_front() {
    let mut world = make_world(2, 4, 2);
    let c0 = CountryId(0);
    let c1 = CountryId(1);
    set_province(&mut world, 1, 0, c0);
    set_province(&mut world, 2, 0, c0);
    set_province(&mut world, 3, 1, c1);
    set_province(&mut world, 4, 1, c1);
    {
        let map_mut = Arc::get_mut(&mut world.map).unwrap();
        add_adjacency(map_mut, 2, 3);
    }
    put_at_war(&mut world, c0, c1);

    let front = compute_front(&world, c0);
    let seg = compute_segment(&world, c0, c1);
    assert_eq!(front.segments.len(), 1);
    assert_eq!(front.segments[0].friendly_states, seg.friendly_states);
    assert_eq!(front.segments[0].enemy_states, seg.enemy_states);
}
