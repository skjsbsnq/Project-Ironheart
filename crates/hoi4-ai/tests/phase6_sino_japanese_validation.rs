use std::collections::HashMap;
use std::sync::Arc;

use hoi4_ai::china_theater::{
    attack_threshold_multiplier, generate_invasion_plans, invasion_staging_ports,
    japan_controlled_chinese_core_states, segment_weight, state_priority_bonus, strategy_for,
    ChinaWarPhase,
};
use hoi4_data::{CountryTag, GameData, State};
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
    Building, BuildingKind, BuildingOwner, CountryId, DiplomacyState, GameDate, GameSpeed,
    ProvinceId, StateId, War, World,
};

const JAP: CountryId = CountryId(0);
const CHI: CountryId = CountryId(1);
const SND: CountryId = CountryId(2);
const SHX: CountryId = CountryId(3);
const PRC: CountryId = CountryId(4);

fn test_world(year: u16) -> World {
    let mut data = GameData::default();
    data.states = vec![
        data_state(608, "beiping", "CHI"),
        data_state(597, "shandong", "SND"),
        data_state(613, "shanghai", "CHI"),
        data_state(598, "nanjing", "CHI"),
        data_state(900, "chongqing", "CHI"),
        data_state(300, "kanto", "JAP"),
    ];

    let province_count = 30;
    let state_count = data.states.len();
    let mut map = empty_map(province_count);
    for p in 1..=province_count {
        map.definitions[p] = Some(ProvinceDefinition {
            id: p as u16,
            r: 0,
            g: 0,
            b: 0,
            province_type: ProvinceType::Land,
            coastal: false,
            terrain: String::new(),
            continent: 0,
        });
    }

    let mut provinces = ProvinceStore::new(province_count + 1);
    let mut states = StateStore::new(state_count);
    let mut countries = CountryStore::new(5);
    for (idx, tag) in ["JAP", "CHI", "SND", "SHX", "PRC"].iter().enumerate() {
        countries.tags[idx] = (*tag).to_owned();
    }
    countries.ideas[JAP.0 as usize].push("FLAG:china_incident_escalated".to_owned());

    let mut tag_to_country = HashMap::new();
    tag_to_country.insert("JAP".to_owned(), JAP);
    tag_to_country.insert("CHI".to_owned(), CHI);
    tag_to_country.insert("SND".to_owned(), SND);
    tag_to_country.insert("SHX".to_owned(), SHX);
    tag_to_country.insert("PRC".to_owned(), PRC);

    let mut state_id_lookup = HashMap::new();
    for (idx, state) in data.states.iter().enumerate() {
        let state_id = StateId(idx as u16);
        states.names[idx] = state.name.clone();
        states.owners[idx] = owner_id(&state.owner.0);
        states.controllers[idx] = states.owners[idx];
        states.cores[idx] = state.cores.iter().map(|tag| owner_id(&tag.0)).collect();
        states.provinces[idx] = vec![ProvinceId((idx as u16) + 1)];
        provinces.owners[(idx + 1) as usize] = states.owners[idx];
        provinces.controllers[(idx + 1) as usize] = states.controllers[idx];
        provinces.state_of[(idx + 1) as usize] = state_id;
        state_id_lookup.insert(state.id, state_id);
    }

    let mut world = World {
        date: GameDate {
            year,
            month: 7,
            day: 1,
            hour: 0,
        },
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
        map: Arc::new(map),
        data: Arc::new(data),
        tag_to_country,
        state_id_lookup,
        player: CountryId::NONE,
        random_seed: 0x19370707,
        game_unique_id: 1,
        path_cache: HashMap::new(),
        path_cache_day: 0,
        prov_div_index: HashMap::new(),
        country_state_index: vec![Vec::new(); 5],
        country_pop_index: vec![Vec::new(); 5],
        country_building_index: vec![Vec::new(); 5],
        country_division_index: vec![Vec::new(); 5],
        country_fleet_index: vec![Vec::new(); 5],
        country_air_wing_index: vec![Vec::new(); 5],
        runtime_country_indexes_valid: false,
        trade_export_surplus_index: HashMap::new(),
        player_armies: Vec::new(),
        player_locked_divisions: std::collections::HashSet::new(),
        next_army_id: 0,
    };

    put_at_war(&mut world, JAP, CHI);
    put_at_war(&mut world, JAP, SND);
    put_at_war(&mut world, JAP, SHX);
    put_at_war(&mut world, JAP, PRC);
    world
}

#[test]
fn phase6_1937_to_1938_prioritizes_north_china_and_shandong() {
    let mut world = test_world(1937);
    add_flag(&mut world, "jap_priority_north_china");
    add_flag(&mut world, "jap_priority_shandong");
    add_idea(&mut world, "jap_continental_offensive_momentum");

    let strategy = strategy_for(&world, JAP);
    assert!(strategy.active);
    assert_eq!(strategy.phase, ChinaWarPhase::NorthChina);

    world.date.month = 8;
    let strategy = strategy_for(&world, JAP);
    assert_eq!(strategy.phase, ChinaWarPhase::Shandong);

    let shandong = world.state_id_lookup[&597];
    let north_china = world.state_id_lookup[&608];
    assert!(state_priority_bonus(&world, JAP, shandong) >= 70.0);
    assert!(state_priority_bonus(&world, JAP, north_china) >= 45.0);
    assert!(
        !strategy.priority_enemy_tags.contains(&"PRC"),
        "1937 早期日本不应把 PRC 作为华北/山东正面优先目标"
    );
    assert!(attack_threshold_multiplier(&world, JAP, SND) < 0.85);
    assert!(segment_weight(&world, JAP, SND, &[shandong]) > 1.25);
}

#[test]
fn phase6_1938_shanghai_nanjing_depends_on_actual_stage_flags() {
    let mut world = test_world(1938);
    add_flag(&mut world, "jap_priority_shanghai_nanjing");

    let strategy = strategy_for(&world, JAP);
    assert_eq!(strategy.phase, ChinaWarPhase::ShanghaiNanjing);

    let shanghai = world.state_id_lookup[&613];
    let nanjing = world.state_id_lookup[&598];
    let shandong = world.state_id_lookup[&597];
    assert!(state_priority_bonus(&world, JAP, shanghai) >= 55.0);
    assert!(state_priority_bonus(&world, JAP, nanjing) >= 55.0);
    assert!(segment_weight(&world, JAP, CHI, &[shanghai, nanjing]) > 1.25);
    assert!(state_priority_bonus(&world, JAP, shandong) > 0.0);
}

#[test]
fn phase6_1939_to_1941_stalemate_slows_deep_advance() {
    let mut early = test_world(1937);
    add_flag(&mut early, "jap_priority_shandong");
    add_idea(&mut early, "jap_continental_offensive_momentum");
    let shandong = early.state_id_lookup[&597];
    let early_bonus = state_priority_bonus(&early, JAP, shandong);

    let mut late = test_world(1939);
    add_flag(&mut late, "jap_priority_shandong");
    add_idea(&mut late, "jap_occupation_security_pressure");
    add_idea(&mut late, "jap_extended_continental_supply_lines");
    add_idea(&mut late, "jap_forces_dispersed_in_china");
    for si in 0..late.states.count {
        late.states.controllers[si] = JAP;
    }

    let strategy = strategy_for(&late, JAP);
    assert_eq!(strategy.phase, ChinaWarPhase::Stalemate);
    assert!(strategy.min_front_ratio > 1.0);
    assert!(attack_threshold_multiplier(&late, JAP, CHI) > 1.0);
    assert!(state_priority_bonus(&late, JAP, shandong) < early_bonus);
    assert!(japan_controlled_chinese_core_states(&late, JAP) >= 5);
}

#[test]
fn phase6_china_invasion_plans_use_japanese_home_ports() {
    let mut world = test_world(1937);
    add_flag(&mut world, "jap_priority_north_china");

    let kanto = world.state_id_lookup[&300];
    let shandong = world.state_id_lookup[&597];
    let kanto_port = world.states.provinces[kanto.0 as usize][0];
    let shandong_target = world.states.provinces[shandong.0 as usize][0];
    Arc::get_mut(&mut world.map).unwrap().definitions[kanto_port.0 as usize]
        .as_mut()
        .unwrap()
        .coastal = true;
    Arc::get_mut(&mut world.map).unwrap().definitions[shandong_target.0 as usize]
        .as_mut()
        .unwrap()
        .coastal = true;
    Arc::get_mut(&mut world.map).unwrap().definitions[30]
        .as_mut()
        .unwrap()
        .province_type = ProvinceType::Sea;
    Arc::get_mut(&mut world.map).unwrap().adjacencies[kanto_port.0 as usize].push(30);
    Arc::get_mut(&mut world.map).unwrap().adjacencies[shandong_target.0 as usize].push(30);
    world.countries.buildings_v6.buildings.push(Building {
        kind: BuildingKind::MilitaryBase,
        building_def_id: "naval_base".to_owned(),
        state: kanto,
        level: 3,
        active_pm: String::new(),
        active_pm_by_group: Vec::new(),
        employment: [0; 6],
        owner: BuildingOwner::State,
        ownership_shares: Vec::new(),
        requires_law: None,
        production_rate: 0.0,
        output_value_gbp: 0.0,
        wage_rm: 0.0,
        profit_rm: 0.0,
        input_cost_rm: 0.0,
        estimated_profit_rm: 0.0,
        value_added_rm: 0.0,
        cp_cost: 0.0,
        max_level: 3,
        built_progress: 1.0,
    });

    let staging_ports = invasion_staging_ports(&world, JAP);
    let usable_ports = hoi4_logic::military::movement::usable_ports(&world, JAP);
    assert!(
        staging_ports.contains(&kanto_port),
        "Kanto should count as a Japanese invasion staging port; states_count={} state_provs={:?} usable={usable_ports:?} staging={staging_ports:?} kanto_port={kanto_port:?} state={kanto:?} owner={:?} controller={:?} coastal={:?} buildings={:?}",
        world.states.count,
        world.states.provinces[kanto.0 as usize],
        world.states.owners[kanto.0 as usize],
        world.states.controllers[kanto.0 as usize],
        world.map.definitions[kanto_port.0 as usize].as_ref().map(|d| d.coastal),
        world.countries.buildings_v6.buildings
    );

    let mut next_id = 0;
    let plans = generate_invasion_plans(&world, JAP, &[], &mut next_id);
    assert!(
        plans
            .iter()
            .any(|plan| plan.embark_port == Some(kanto_port)),
        "Japan should generate a China invasion plan with a real embark port: {plans:?}"
    );
}

fn empty_map(province_count: usize) -> GameMap {
    GameMap {
        definitions: vec![None; province_count + 1],
        rgb_to_id: HashMap::new(),
        province_map: ProvinceMap {
            width: 0,
            height: 0,
            pixels: Vec::new(),
        },
        adjacencies: vec![Vec::new(); province_count + 1],
        special_adjacencies: Vec::<Adjacency>::new(),
        heightmap: Heightmap {
            width: 0,
            height: 0,
            pixels: Vec::new(),
        },
        terrain_bmp: TerrainBitmap {
            width: 0,
            height: 0,
            pixels: Vec::new(),
            palette: [[0; 3]; 256],
        },
        terrain_catalog: TerrainCatalog::default(),
        tree_definition_bmp: None,
        tree_indices: hoi4_map::DEFAULT_TREE_INDICES.iter().copied().collect(),
    }
}

fn data_state(id: u16, name: &str, owner: &str) -> State {
    State {
        id,
        name: name.to_owned(),
        manpower: 0,
        owner: CountryTag::new(owner),
        cores: vec![CountryTag::new(owner)],
        provinces: Vec::new(),
        category: String::new(),
        infrastructure: 0,
        victory_points: Vec::new(),
        resources: Vec::new(),
    }
}

fn owner_id(tag: &str) -> CountryId {
    match tag {
        "JAP" => JAP,
        "CHI" => CHI,
        "SND" => SND,
        "SHX" => SHX,
        "PRC" => PRC,
        _ => CountryId::NONE,
    }
}

fn add_flag(world: &mut World, flag: &str) {
    world.countries.ideas[JAP.0 as usize].push(format!("FLAG:{flag}"));
}

fn add_idea(world: &mut World, idea: &str) {
    world.countries.ideas[JAP.0 as usize].push(idea.to_owned());
}

fn put_at_war(world: &mut World, c1: CountryId, c2: CountryId) {
    let mut atk = std::collections::HashSet::new();
    atk.insert(c1);
    let mut def = std::collections::HashSet::new();
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
