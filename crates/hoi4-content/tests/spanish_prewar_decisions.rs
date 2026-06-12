use hoi4_content::{
    daily_decision_tick, load_scenario_content, run_effects, DecisionCategory, DecisionDb,
    DecisionState, Effect, GlobalFlags, ScenarioContent, SituationEffect, Trigger,
};
use hoi4_state::{
    AirWingStore, CommandHierarchy, CountryId, CountryStore, DiplomacyState, DivisionStore,
    FleetStore, GameDate, GameSpeed, ProvinceStore, ShipStore, StateStore, World,
};
use std::collections::HashMap;
use std::sync::Arc;

const SPR_AXES: &[&str] = &[
    "spr_government_authority",
    "spr_street_mobilization",
    "spr_officer_conspiracy",
    "spr_church_right_alarm",
    "spr_armory_control",
];

const SPA_AXES: &[&str] = &[
    "spa_conspiracy_network",
    "spa_garrison_loyalty",
    "spa_carlist_support",
    "spa_falange_mobilization",
    "spa_foreign_precommit",
];

fn effect_touches_any_axis(effect: &Effect, axes: &[&str]) -> bool {
    match effect {
        Effect::SetVariable { name, .. }
        | Effect::AddToVariable { name, .. }
        | Effect::ClampVariable { name, .. } => axes.contains(&name.as_str()),
        Effect::AddToCountryVariable { name, .. } | Effect::ClampCountryVariable { name, .. } => {
            axes.contains(&name.as_str())
        }
        Effect::If { effects, .. } => effects.iter().any(|e| effect_touches_any_axis(e, axes)),
        _ => false,
    }
}

fn effect_uses_old_delta(effect: &Effect) -> bool {
    match effect {
        Effect::SetVariable { name, .. }
        | Effect::AddToVariable { name, .. }
        | Effect::ClampVariable { name, .. }
        | Effect::AddToCountryVariable { name, .. }
        | Effect::ClampCountryVariable { name, .. } => name.ends_with("_delta"),
        Effect::If { effects, .. } => effects.iter().any(effect_uses_old_delta),
        _ => false,
    }
}

fn trigger_has_lock(trigger: &Trigger, flag: &str) -> bool {
    match trigger {
        Trigger::Not(inner) => matches!(inner.as_ref(), Trigger::HasCountryFlag(f) if f == flag),
        Trigger::And(parts) | Trigger::Or(parts) => parts.iter().any(|t| trigger_has_lock(t, flag)),
        _ => false,
    }
}

fn effect_sets_timed_lock(effect: &Effect, flag: &str) -> bool {
    match effect {
        Effect::SetCountryFlagForDays { flag: f, days } => f == flag && *days == 7,
        Effect::If { effects, .. } => effects.iter().any(|e| effect_sets_timed_lock(e, flag)),
        _ => false,
    }
}

fn test_map() -> Arc<hoi4_map::GameMap> {
    Arc::new(hoi4_map::GameMap {
        definitions: vec![],
        rgb_to_id: HashMap::new(),
        province_map: hoi4_map::ProvinceMap {
            width: 1,
            height: 1,
            pixels: vec![0],
        },
        adjacencies: vec![],
        special_adjacencies: vec![],
        heightmap: hoi4_map::Heightmap {
            width: 1,
            height: 1,
            pixels: vec![0],
        },
        terrain_bmp: hoi4_map::TerrainBitmap {
            width: 1,
            height: 1,
            pixels: vec![0],
            palette: [[0u8; 3]; 256],
        },
        terrain_catalog: hoi4_map::TerrainCatalog::default(),
        tree_definition_bmp: None,
        tree_indices: std::collections::HashSet::new(),
    })
}

fn prewar_test_world() -> World {
    let mut countries = CountryStore::new(2);
    countries.tags[0] = "SPR".to_owned();
    countries.tags[1] = "SPA".to_owned();
    countries.political_power[0] = 500.0;
    countries.political_power[1] = 500.0;
    countries.stability[0] = 0.50;
    countries.stability[1] = 0.50;
    countries.ruling_party[0] = "democratic".to_owned();
    countries.ruling_party[1] = "fascist".to_owned();

    let mut tag_to_country = HashMap::new();
    tag_to_country.insert("SPR".to_owned(), CountryId(0));
    tag_to_country.insert("SPA".to_owned(), CountryId(1));

    World {
        date: GameDate::START,
        speed: GameSpeed::Paused,
        elapsed_hours: 0,
        provinces: ProvinceStore::new(0),
        states: StateStore::new(0),
        countries,
        divisions: DivisionStore::new(),
        ships: ShipStore::new(),
        fleets: FleetStore::new(),
        air_wings: AirWingStore::new(),
        diplomacy: DiplomacyState::new(),
        command: CommandHierarchy::default(),
        map: test_map(),
        data: Arc::new(hoi4_data::GameData::default()),
        tag_to_country,
        state_id_lookup: HashMap::new(),
        player: CountryId(0),
        random_seed: 0,
        game_unique_id: 0,
        path_cache: HashMap::new(),
        path_cache_day: 0,
        prov_div_index: HashMap::new(),
        country_state_index: Vec::new(),
        country_pop_index: Vec::new(),
        country_building_index: Vec::new(),
        country_division_index: Vec::new(),
        country_fleet_index: Vec::new(),
        country_air_wing_index: Vec::new(),
        runtime_country_indexes_valid: false,
        trade_export_surplus_index: HashMap::new(),
        player_armies: Vec::new(),
        player_locked_divisions: std::collections::HashSet::new(),
        next_army_id: 0,
        generals: Vec::new(),
        next_general_id: 0,
    }
}

fn date(month: u8, day: u8) -> GameDate {
    GameDate {
        year: 1936,
        month,
        day,
        hour: 12,
    }
}

fn advance_to(
    world: &mut World,
    state: &mut DecisionState,
    decisions: &DecisionDb,
    flags: &mut GlobalFlags,
    target: GameDate,
) {
    while world.date < target {
        world.tick_day();
        daily_decision_tick(state, decisions, world, flags);
    }
}

fn activate_on(
    world: &mut World,
    state: &mut DecisionState,
    decisions: &DecisionDb,
    flags: &mut GlobalFlags,
    tag: &str,
    decision_id: &str,
    target: GameDate,
) {
    advance_to(world, state, decisions, flags, target);
    let country = world.country(tag).expect("test country should exist");
    state
        .activate(decision_id, decisions, world, country, flags)
        .unwrap_or_else(|err| panic!("{decision_id} should activate on {:?}: {err}", world.date));
}

#[derive(Debug)]
struct ScwPayoff {
    rebellion_strength: f32,
    republic_readiness: f32,
    rebel_states_tier: f32,
    manpower_fraction: f32,
    spr_density: f32,
    spa_density: f32,
    sanjurjo_survives: f32,
    militia_autonomy: f32,
}

fn payoff_var(world: &World, name: &str) -> f32 {
    let spr = world.country("SPR").expect("SPR should exist");
    world.countries.variables[spr.0 as usize]
        .get(name)
        .copied()
        .unwrap_or_else(|| panic!("missing payoff variable {name}"))
}

fn run_prewar_settlement(content: &ScenarioContent, world: &mut World) -> ScwPayoff {
    let mut flags = GlobalFlags::default();
    let settlement = content
        .events
        .find("hidden.scw_prewar_settlement")
        .expect("missing SCW prewar settlement event");
    let spr = world.country("SPR").expect("SPR should exist");
    let report = run_effects(&settlement.options[0].effects, world, spr, &mut flags);
    assert!(!report.has_errors(), "{:?}", report.errors);
    assert!(!report.has_warnings(), "{:?}", report.warnings);

    ScwPayoff {
        rebellion_strength: payoff_var(world, "scw_rebellion_strength"),
        republic_readiness: payoff_var(world, "scw_republic_readiness"),
        rebel_states_tier: payoff_var(world, "scw_rebel_states_tier"),
        manpower_fraction: payoff_var(world, "scw_rebel_manpower_fraction"),
        spr_density: payoff_var(world, "scw_spr_frontline_density"),
        spa_density: payoff_var(world, "scw_spa_frontline_density"),
        sanjurjo_survives: payoff_var(world, "scw_sanjurjo_survives"),
        militia_autonomy: payoff_var(world, "scw_spr_militia_autonomy"),
    }
}

#[test]
fn spanish_prewar_decisions_load_from_1936_manifest() {
    let content = load_scenario_content("1936").expect("1936 scenario should load");
    let spr_expected = [
        "spr.prewar_rebalance_civil_governors",
        "spr.prewar_monitor_mola_network",
        "spr.prewar_legalize_land_committees",
        "spr.prewar_guard_church_property",
        "spr.prewar_prepare_armory_keys",
    ];
    let spa_expected = [
        "spa.prewar_expand_mola_network",
        "spa.prewar_suborn_garrison_commanders",
        "spa.prewar_negotiate_carlist_support",
        "spa.prewar_mobilize_falange",
        "spa.prewar_contact_axis",
    ];

    for id in spr_expected {
        let decision = content
            .decisions
            .find(id)
            .expect("missing SPR prewar decision");
        assert_eq!(decision.category, DecisionCategory::Crisis);
        assert!(decision.id.starts_with("spr.prewar_"));
        assert!(decision.days_re_enable > 0);
        assert!(trigger_has_lock(
            &decision.available,
            "SPR_prewar_action_taken"
        ));
        assert!(decision
            .on_complete
            .iter()
            .any(|effect| effect_sets_timed_lock(effect, "SPR_prewar_action_taken")));
        assert!(!decision.on_complete.iter().any(effect_uses_old_delta));
        assert!(decision
            .on_complete
            .iter()
            .any(|effect| effect_touches_any_axis(effect, SPR_AXES)));
    }

    for id in spa_expected {
        let decision = content
            .decisions
            .find(id)
            .expect("missing SPA prewar decision");
        assert_eq!(decision.category, DecisionCategory::Crisis);
        assert!(decision.id.starts_with("spa.prewar_"));
        assert!(decision.days_re_enable > 0);
        assert!(trigger_has_lock(
            &decision.available,
            "SPA_prewar_action_taken"
        ));
        assert!(decision
            .on_complete
            .iter()
            .any(|effect| effect_sets_timed_lock(effect, "SPA_prewar_action_taken")));
        assert!(decision
            .on_complete
            .iter()
            .any(|effect| effect_touches_any_axis(effect, SPA_AXES)));
    }
}

#[test]
fn spanish_prewar_settlement_is_wired_to_situation_start() {
    let content = load_scenario_content("1936").expect("1936 scenario should load");

    let settlement = content
        .events
        .find("hidden.scw_prewar_settlement")
        .expect("missing SCW prewar settlement event");
    assert!(settlement.hidden);
    assert!(settlement.is_triggered_only);
    assert!(settlement.options[0].effects.iter().any(|effect| matches!(
        effect,
        Effect::ComputeSpanishPrewarSettlement {
            republic_tag,
            nationalist_tag,
        } if republic_tag == "SPR" && nationalist_tag == "SPA"
    )));

    let scw = content
        .situations
        .iter()
        .find(|s| s.id == "spanish_civil_war")
        .expect("missing Spanish Civil War situation");
    assert!(matches!(
        scw.on_start.first(),
        Some(SituationEffect::TriggerEvent(id)) if id == "hidden.scw_prewar_settlement"
    ));
    assert!(scw.on_start.iter().any(|effect| matches!(
        effect,
        SituationEffect::SplitCountryByStates {
            rebel_states_var: Some(var),
            manpower_fraction_var: Some(mp_var),
            ..
        } if var == "scw_rebel_states_tier" && mp_var == "scw_rebel_manpower_fraction"
    )));
    assert!(scw.on_start.iter().any(|effect| matches!(
        effect,
        SituationEffect::SplitDivisions {
            fraction_var: Some(var),
            ..
        } if var == "scw_rebel_division_fraction"
    )));
    assert!(scw.on_start.iter().any(|effect| matches!(
        effect,
        SituationEffect::SpawnFrontlineDivisions {
            tag,
            density_var: Some(var),
            ..
        } if tag == "SPR" && var == "scw_spr_frontline_density"
    )));
    assert!(scw.on_start.iter().any(|effect| matches!(
        effect,
        SituationEffect::SpawnFrontlineDivisions {
            tag,
            density_var: Some(var),
            ..
        } if tag == "SPA" && var == "scw_spa_frontline_density"
    )));
    assert!(!scw.on_start.iter().any(|effect| matches!(
        effect,
        SituationEffect::SetCountryFlag { country, flag }
            if country == "SPA" && flag == "spa_sanjurjo_dead"
    )));
}

#[test]
fn spanish_prewar_routes_drive_different_july_17_payoffs() {
    let content = load_scenario_content("1936").expect("1936 scenario should load");

    let mut republic_world = prewar_test_world();
    let mut republic_state = DecisionState::new();
    let mut republic_flags = GlobalFlags::default();
    activate_on(
        &mut republic_world,
        &mut republic_state,
        &content.decisions,
        &mut republic_flags,
        "SPR",
        "spr.prewar_rebalance_civil_governors",
        date(2, 17),
    );
    activate_on(
        &mut republic_world,
        &mut republic_state,
        &content.decisions,
        &mut republic_flags,
        "SPR",
        "spr.prewar_monitor_mola_network",
        date(3, 1),
    );
    activate_on(
        &mut republic_world,
        &mut republic_state,
        &content.decisions,
        &mut republic_flags,
        "SPR",
        "spr.prewar_legalize_land_committees",
        date(3, 10),
    );
    activate_on(
        &mut republic_world,
        &mut republic_state,
        &content.decisions,
        &mut republic_flags,
        "SPR",
        "spr.prewar_guard_church_property",
        date(4, 1),
    );
    activate_on(
        &mut republic_world,
        &mut republic_state,
        &content.decisions,
        &mut republic_flags,
        "SPR",
        "spr.prewar_monitor_mola_network",
        date(4, 8),
    );
    activate_on(
        &mut republic_world,
        &mut republic_state,
        &content.decisions,
        &mut republic_flags,
        "SPR",
        "spr.prewar_prepare_armory_keys",
        date(5, 1),
    );
    activate_on(
        &mut republic_world,
        &mut republic_state,
        &content.decisions,
        &mut republic_flags,
        "SPR",
        "spr.prewar_monitor_mola_network",
        date(5, 10),
    );
    activate_on(
        &mut republic_world,
        &mut republic_state,
        &content.decisions,
        &mut republic_flags,
        "SPR",
        "spr.prewar_prepare_armory_keys",
        date(6, 3),
    );
    activate_on(
        &mut republic_world,
        &mut republic_state,
        &content.decisions,
        &mut republic_flags,
        "SPR",
        "spr.prewar_prepare_armory_keys",
        date(7, 6),
    );
    advance_to(
        &mut republic_world,
        &mut republic_state,
        &content.decisions,
        &mut republic_flags,
        date(7, 17),
    );
    let republic_payoff = run_prewar_settlement(&content, &mut republic_world);

    let mut nationalist_world = prewar_test_world();
    let mut nationalist_state = DecisionState::new();
    let mut nationalist_flags = GlobalFlags::default();
    activate_on(
        &mut nationalist_world,
        &mut nationalist_state,
        &content.decisions,
        &mut nationalist_flags,
        "SPA",
        "spa.prewar_expand_mola_network",
        date(2, 17),
    );
    activate_on(
        &mut nationalist_world,
        &mut nationalist_state,
        &content.decisions,
        &mut nationalist_flags,
        "SPA",
        "spa.prewar_suborn_garrison_commanders",
        date(3, 1),
    );
    activate_on(
        &mut nationalist_world,
        &mut nationalist_state,
        &content.decisions,
        &mut nationalist_flags,
        "SPA",
        "spa.prewar_negotiate_carlist_support",
        date(3, 10),
    );
    activate_on(
        &mut nationalist_world,
        &mut nationalist_state,
        &content.decisions,
        &mut nationalist_flags,
        "SPA",
        "spa.prewar_expand_mola_network",
        date(3, 17),
    );
    activate_on(
        &mut nationalist_world,
        &mut nationalist_state,
        &content.decisions,
        &mut nationalist_flags,
        "SPA",
        "spa.prewar_expand_mola_network",
        date(4, 11),
    );
    activate_on(
        &mut nationalist_world,
        &mut nationalist_state,
        &content.decisions,
        &mut nationalist_flags,
        "SPA",
        "spa.prewar_contact_axis",
        date(5, 1),
    );
    activate_on(
        &mut nationalist_world,
        &mut nationalist_state,
        &content.decisions,
        &mut nationalist_flags,
        "SPA",
        "spa.prewar_contact_axis",
        date(6, 3),
    );
    activate_on(
        &mut nationalist_world,
        &mut nationalist_state,
        &content.decisions,
        &mut nationalist_flags,
        "SPA",
        "spa.prewar_contact_axis",
        date(7, 6),
    );
    advance_to(
        &mut nationalist_world,
        &mut nationalist_state,
        &content.decisions,
        &mut nationalist_flags,
        date(7, 17),
    );
    let nationalist_payoff = run_prewar_settlement(&content, &mut nationalist_world);

    assert!(republic_payoff.rebellion_strength < nationalist_payoff.rebellion_strength);
    assert_eq!(republic_payoff.rebel_states_tier, -1.0);
    assert_eq!(nationalist_payoff.rebel_states_tier, 1.0);
    assert!(republic_payoff.manpower_fraction < 0.45);
    assert!(nationalist_payoff.manpower_fraction > 0.45);
    assert_eq!(republic_payoff.spr_density, 3.0);
    assert_eq!(nationalist_payoff.spa_density, 2.0);
    assert_eq!(republic_payoff.sanjurjo_survives, 0.0);
    assert_eq!(nationalist_payoff.sanjurjo_survives, 1.0);
    assert_eq!(republic_payoff.militia_autonomy, 1.0);
    assert!(republic_payoff.republic_readiness > nationalist_payoff.republic_readiness);
}
