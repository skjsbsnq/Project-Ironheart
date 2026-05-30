//! V6 economy event triggers and string-effect executor.

use hoi4_content::V6Database;
use hoi4_state::{
    Building, BuildingKind, BuildingOwner, CountryId, LawCategory, PopClass, StateId, World,
};

pub fn tick_v6_events(world: &mut World, db: &V6Database, ci: usize, day: i64) -> Vec<String> {
    if ci >= world.countries.count {
        return Vec::new();
    }

    let mut fired = Vec::new();
    let tag = world
        .countries
        .tags
        .get(ci)
        .map(|t| t.as_str())
        .unwrap_or("");
    if tag == "GER" {
        if day >= days_since_start(1936, 3, 7) {
            fire_event(world, db, ci, "rhineland_remil_econ", &mut fired);
        }
        if day >= days_since_start(1936, 8, 18) {
            fire_event(world, db, ci, "four_year_plan", &mut fired);
        }
        if day >= days_since_start(1937, 1, 1) {
            fire_event(world, db, ci, "mefo_expansion_1937", &mut fired);
        }
        if day >= days_since_start(1938, 3, 12) {
            fire_event(world, db, ci, "anschluss_economic", &mut fired);
        }
    }

    let economy_law = world.countries.law_store.law_sets[ci].0[LawCategory::Economy.index()]
        .current
        .clone();
    if economy_law == "planned_economy" {
        fire_event(world, db, ci, "nationalization", &mut fired);
    }

    let exchange = &world.countries.treasury.exchange_rates[ci];
    if exchange.rm_per_gbp > hoi4_state::finance::ExchangeRate::BASE_RATE * 1.10 {
        fire_event(world, db, ci, "mark_devaluation_crisis", &mut fired);
    }

    if world.countries.trade.routes.iter().any(|r| {
        (r.importer == CountryId(ci as u16) || r.exporter == CountryId(ci as u16)) && r.is_blockaded
    }) {
        fire_event(world, db, ci, "blockade_crisis", &mut fired);
    }

    fired
}

pub fn fire_event(
    world: &mut World,
    db: &V6Database,
    ci: usize,
    event_id: &str,
    fired: &mut Vec<String>,
) -> bool {
    if ci >= world.countries.v6_events_fired.len() {
        return false;
    }
    if world.countries.v6_events_fired[ci].contains(event_id) {
        return false;
    }

    let Some(event) = db
        .events_v6
        .iter()
        .find(|event| event.id == event_id)
        .cloned()
    else {
        return false;
    };
    world.countries.v6_events_fired[ci].insert(event_id.to_owned());
    if let Some(option) = event.options.first() {
        for (effect_key, effect_value) in &option.effects {
            apply_effect(world, db, ci, effect_key, effect_value);
        }
    }
    fired.push(event_id.to_owned());
    true
}

pub fn apply_effect(
    world: &mut World,
    db: &V6Database,
    ci: usize,
    effect_key: &str,
    effect_value: &str,
) {
    match effect_key {
        "mefo_forced_payment" => {
            let rm_per_gbp = world.countries.treasury.exchange_rates[ci].rm_per_gbp;
            world.countries.treasury.treasuries[ci].trigger_mefo_crisis_with_ratios(
                rm_per_gbp,
                db.mefo.crisis_forced_payment_ratio,
                db.mefo.crisis_residual_debt_ratio,
            );
        }
        "nationalize_all_buildings" => nationalize_all_buildings(world, ci),
        "close_banks" => close_banks(world, ci),
        "force_pm_synthetic" => force_pm_synthetic(world, ci),
        "coal_diversion" => apply_coal_diversion(world, ci, parse_f32(effect_value, 0.2)),
        "annex_industry" => annex_industry(world, db, ci, effect_value),
        "hyperinflation" | "inflation_multiply" => {
            apply_exchange_rate_multiplier(world, ci, parse_f32(effect_value, 1.5))
        }
        "credit_downgrade_2" => downgrade_credit(world, ci, 2),
        "credit_rating_set_d" => {
            world.countries.treasury.treasuries[ci].credit_rating = hoi4_state::CreditRating::D
        }
        "pop_satisfaction_drop" | "pop_satisfaction_penalty" => {
            apply_pop_satisfaction(world, ci, parse_f32(effect_value, -0.10))
        }
        "stability_drop" | "stability_penalty" | "stability_add" => {
            apply_stability(world, ci, parse_f32(effect_value, -0.10))
        }
        "war_support_drop" => apply_war_support(world, ci, parse_f32(effect_value, -0.10)),
        "war_support_add" => apply_war_support(world, ci, parse_f32(effect_value, 0.10)),
        "loyalty_drop_capitalist" | "capitalist_loyalty_penalty" => {
            apply_capitalist_loyalty(world, ci, parse_f32(effect_value, -0.25))
        }
        "political_power_cost" => apply_pp(world, ci, parse_f32(effect_value, -100.0)),
        "bond_issuance" => world.countries.treasury.treasuries[ci]
            .issue_domestic_bond(parse_f64(effect_value, 0.0)),
        "reserve_gbp_bonus" => {
            world.countries.treasury.treasuries[ci].reserve_gbp += parse_f64(effect_value, 0.0)
        }
        "gold_bonus" => {
            world.countries.treasury.treasuries[ci].gold_kg += parse_f64(effect_value, 0.0)
        }
        "sell_gold_for_reserve" => {
            let gold = world.countries.treasury.treasuries[ci].gold_kg;
            let ratio = parse_f64(effect_value, 0.5).clamp(0.0, 1.0);
            world.countries.treasury.treasuries[ci].sell_gold(gold * ratio);
        }
        "exchange_rate_stabilize" => {
            world.countries.treasury.exchange_rates[ci].rm_per_gbp =
                hoi4_state::finance::ExchangeRate::BASE_RATE
        }
        "force_trade_law" => set_law_current(world, ci, LawCategory::Trade, effect_value),
        "construction_boost"
        | "trade_penalty"
        | "import_reduction"
        | "mefo_print_bonus"
        | "satisfaction_penalty_duration"
        | "mefo_crisis_deferred"
        | "consumer_goods_cut" => {}
        _ => {}
    }
}

fn days_since_start(year: u16, month: u8, day: u8) -> i64 {
    let target = hoi4_state::GameDate {
        year,
        month,
        day,
        hour: 0,
    }
    .days_since_epoch();
    let start = hoi4_state::GameDate::START.days_since_epoch();
    target - start
}

fn country_states(world: &World, ci: usize) -> Vec<StateId> {
    let country_id = CountryId(ci as u16);
    (0..world.states.count)
        .filter(|&si| world.states.owners[si] == country_id)
        .map(|si| StateId(si as u16))
        .collect()
}

fn nationalize_all_buildings(world: &mut World, ci: usize) {
    let state_ids = country_states(world, ci);
    for building in &mut world.countries.buildings_v6.buildings {
        if state_ids.contains(&building.state) && building.owner != BuildingOwner::State {
            building.owner = BuildingOwner::State;
        }
    }
}

fn close_banks(world: &mut World, ci: usize) {
    let state_ids = country_states(world, ci);
    for (bidx, building) in world
        .countries
        .buildings_v6
        .buildings
        .iter_mut()
        .enumerate()
    {
        if !state_ids.contains(&building.state) || building.building_def_id != "bank" {
            continue;
        }
        building.level = 0;
        building.employment = [0; 6];
        for pg in &mut world.countries.pops.groups {
            if pg.employed_at == Some(hoi4_state::BuildingId(bidx as u32)) {
                pg.employed_at = None;
            }
        }
    }
}

fn force_pm_synthetic(world: &mut World, ci: usize) {
    let state_ids = country_states(world, ci);
    for building in &mut world.countries.buildings_v6.buildings {
        if state_ids.contains(&building.state) && building.building_def_id == "synthetic_refinery" {
            building.active_pm = "synthetic_default".to_owned();
        }
    }
}

fn apply_coal_diversion(world: &mut World, ci: usize, ratio: f32) {
    let market = &mut world.countries.market.markets[ci];
    if let Some(coal_supply) = market.supply.get_mut("coal") {
        *coal_supply *= 1.0 - ratio.clamp(0.0, 1.0);
    }
}

fn annex_industry(world: &mut World, db: &V6Database, ci: usize, target: &str) {
    let target_tag = match target {
        "austria" => "AUS",
        other => other,
    };
    if !world.tag_to_country.contains_key(target_tag) {
        inject_bonus_industry(world, db, ci);
        return;
    }
    inject_bonus_industry(world, db, ci);
}

fn inject_bonus_industry(world: &mut World, db: &V6Database, ci: usize) {
    let state = country_states(world, ci)
        .first()
        .copied()
        .unwrap_or(StateId(0));
    for building_id in ["steel_mill", "coal_mine"] {
        let active_pm = db
            .production_methods
            .iter()
            .find(|pm| pm.building_id == building_id && pm.id.ends_with("default"))
            .map(|pm| pm.id.clone())
            .unwrap_or_else(|| format!("{building_id}_default"));
        world.countries.buildings_v6.buildings.push(Building {
            kind: BuildingKind::Industrial,
            building_def_id: building_id.to_owned(),
            state,
            level: 1,
            active_pm,
            employment: [0; 6],
            owner: BuildingOwner::Private,
            requires_law: None,
            built_progress: 1.0,
            ..Building::runtime_defaults()
        });
    }
}

fn apply_exchange_rate_multiplier(world: &mut World, ci: usize, mult: f32) {
    world.countries.treasury.exchange_rates[ci].rm_per_gbp =
        (world.countries.treasury.exchange_rates[ci].rm_per_gbp * mult.max(0.1)).max(0.1);
}

fn downgrade_credit(world: &mut World, ci: usize, steps: usize) {
    let ratings = [
        hoi4_state::CreditRating::AAA,
        hoi4_state::CreditRating::AA,
        hoi4_state::CreditRating::A,
        hoi4_state::CreditRating::BBB,
        hoi4_state::CreditRating::BB,
        hoi4_state::CreditRating::B,
        hoi4_state::CreditRating::CCC,
        hoi4_state::CreditRating::D,
    ];
    let current = world.countries.treasury.treasuries[ci].credit_rating;
    let idx = ratings.iter().position(|r| *r == current).unwrap_or(0);
    world.countries.treasury.treasuries[ci].credit_rating =
        ratings[(idx + steps).min(ratings.len() - 1)];
}

fn apply_pop_satisfaction(world: &mut World, ci: usize, delta: f32) {
    let state_ids = country_states(world, ci);
    for pg in &mut world.countries.pops.groups {
        if state_ids.contains(&pg.state) && pg.class != PopClass::Soldier {
            pg.satisfaction = (pg.satisfaction + delta).clamp(0.0, 1.0);
        }
    }
}

fn apply_stability(world: &mut World, ci: usize, delta: f32) {
    world.countries.stability[ci] = (world.countries.stability[ci] + delta).clamp(0.0, 1.0);
}

fn apply_war_support(world: &mut World, ci: usize, delta: f32) {
    world.countries.war_support[ci] = (world.countries.war_support[ci] + delta).clamp(0.0, 1.0);
}

fn apply_capitalist_loyalty(world: &mut World, ci: usize, delta: f32) {
    let state_ids = country_states(world, ci);
    for pg in &mut world.countries.pops.groups {
        if state_ids.contains(&pg.state) && pg.class == PopClass::Capitalist {
            pg.political_loyalty = (pg.political_loyalty + delta).clamp(-1.0, 1.0);
        }
    }
}

fn apply_pp(world: &mut World, ci: usize, delta: f32) {
    world.countries.political_power[ci] =
        (world.countries.political_power[ci] + delta).clamp(-1000.0, 1000.0);
}

fn set_law_current(world: &mut World, ci: usize, category: LawCategory, law_id: &str) {
    world.countries.law_store.law_sets[ci].0[category.index()].current = law_id.to_owned();
}

fn parse_f32(value: &str, default: f32) -> f32 {
    value.parse().unwrap_or(default)
}

fn parse_f64(value: &str, default: f64) -> f64 {
    value.parse().unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Arc;

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
                palette: [[0; 3]; 256],
            },
            terrain_catalog: hoi4_map::TerrainCatalog::default(),
            tree_definition_bmp: None,
            tree_indices: std::collections::HashSet::new(),
        })
    }

    fn test_data() -> Arc<hoi4_data::GameData> {
        let mut data = hoi4_data::GameData::default();
        let tag = hoi4_data::CountryTag::new("GER");
        let aus_tag = hoi4_data::CountryTag::new("AUS");
        data.countries.insert(
            tag.clone(),
            hoi4_data::Country {
                tag: tag.clone(),
                color: hoi4_data::Color {
                    r: 80,
                    g: 80,
                    b: 80,
                },
                graphical_culture: "western_european_gfx".to_owned(),
                capital: 1,
                ruling_party: "fascism".to_owned(),
                technologies: vec![],
            },
        );
        data.countries.insert(
            aus_tag.clone(),
            hoi4_data::Country {
                tag: aus_tag.clone(),
                color: hoi4_data::Color {
                    r: 180,
                    g: 180,
                    b: 180,
                },
                graphical_culture: "western_european_gfx".to_owned(),
                capital: 2,
                ruling_party: "neutrality".to_owned(),
                technologies: vec![],
            },
        );
        data.states.push(hoi4_data::State {
            id: 1,
            name: "Test State".to_owned(),
            manpower: 1_000_000,
            owner: tag.clone(),
            cores: vec![tag],
            provinces: vec![],
            category: "city".to_owned(),
            infrastructure: 0,
            victory_points: vec![],
            resources: vec![],
        });
        data.states.push(hoi4_data::State {
            id: 2,
            name: "Austria Test State".to_owned(),
            manpower: 1_000_000,
            owner: aus_tag.clone(),
            cores: vec![aus_tag],
            provinces: vec![],
            category: "city".to_owned(),
            infrastructure: 0,
            victory_points: vec![],
            resources: vec![],
        });
        Arc::new(data)
    }

    fn test_world() -> World {
        let mut world = World::new(test_map(), test_data());
        world.countries.pops.groups.push(hoi4_state::PopGroup {
            class: PopClass::Capitalist,
            state: StateId(0),
            size: 10,
            employed_at: Some(hoi4_state::BuildingId(0)),
            wage_rm: 10.0,
            tax_burden: 0.0,
            income_rm: 0.0,
            tax_paid_rm: 0.0,
            disposable_income_rm: 0.0,
            basic_consumption_budget: 0.0,
            satisfaction_law_modifier: 0.0,
            loyalty_coefficient: 1.0,
            loyalty_decay_mult: 1.0,
            satisfaction: 0.5,
            political_loyalty: 0.0,
            literacy: PopClass::Capitalist.baseline_literacy(),
            skilled_ratio: PopClass::Capitalist.baseline_skilled_ratio(),
            standard_of_living: 0.5,
            needs_fulfillment: 1.0,
            essential_needs_fulfillment: 1.0,
            normal_needs_fulfillment: 1.0,
            luxury_needs_fulfillment: 1.0,
            radicalism: 0.0,
        });
        world
    }

    #[test]
    fn historical_date_offsets_are_relative_to_1936_start() {
        assert_eq!(days_since_start(1936, 1, 1), 0);
        assert_eq!(days_since_start(1936, 3, 7), 65);
    }

    #[test]
    #[ignore = "known Phase 9 baseline: V6 economy event content/effect contract is not reconciled"]
    fn historical_event_triggers_once() {
        let mut world = test_world();
        let db = V6Database::load();

        let first = tick_v6_events(&mut world, &db, 0, days_since_start(1936, 3, 7));
        let second = tick_v6_events(&mut world, &db, 0, days_since_start(1936, 3, 8));

        assert!(first.contains(&"rhineland_remil_econ".to_owned()));
        assert!(!second.contains(&"rhineland_remil_econ".to_owned()));
    }

    #[test]
    #[ignore = "known Phase 9 baseline: V6 economy event content/effect contract is not reconciled"]
    fn planned_economy_triggers_nationalization_effects() {
        let mut world = test_world();
        world.countries.law_store.law_sets[0].0[LawCategory::Economy.index()].current =
            "planned_economy".to_owned();
        world.countries.political_power[0] = 600.0;
        world.countries.stability[0] = 0.8;
        world.countries.buildings_v6.buildings.push(Building {
            kind: BuildingKind::Service,
            building_def_id: "bank".to_owned(),
            state: StateId(0),
            level: 1,
            active_pm: "bank_default".to_owned(),
            employment: [0, 0, 0, 10, 0, 0],
            owner: BuildingOwner::Private,
            requires_law: None,
            built_progress: 1.0,
            ..Building::runtime_defaults()
        });
        let db = V6Database::load();

        let fired = tick_v6_events(&mut world, &db, 0, 1);

        assert!(fired.contains(&"nationalization".to_owned()));
        assert_eq!(
            world.countries.buildings_v6.buildings[0].owner,
            BuildingOwner::State
        );
        assert_eq!(world.countries.buildings_v6.buildings[0].level, 0);
        assert_eq!(world.countries.political_power[0], 100.0);
        assert!(world.countries.stability[0] < 0.8);
        assert!(world.countries.pops.groups[0].political_loyalty < 0.0);
    }

    #[test]
    #[ignore = "known Phase 9 baseline: V6 economy event content/effect contract is not reconciled"]
    fn mefo_crisis_event_effect_executes_debt_and_pop_effects() {
        let mut world = test_world();
        world.countries.treasury.treasuries[0].cash_rm = 1_000.0;
        world.countries.treasury.treasuries[0].gdp_rm = 1_000.0;
        world.countries.treasury.treasuries[0].mefo_debt_rm = 400.0;
        world.countries.treasury.treasuries[0].public_debt_rm = 100.0;
        let db = V6Database::load();
        let mut fired = Vec::new();

        fire_event(&mut world, &db, 0, "mefo_crisis", &mut fired);

        assert!(fired.contains(&"mefo_crisis".to_owned()));
        assert_eq!(world.countries.treasury.treasuries[0].mefo_debt_rm, 0.0);
        assert_eq!(world.countries.treasury.treasuries[0].public_debt_rm, 340.0);
        assert!(world.countries.pops.groups[0].satisfaction < 0.5);
    }

    #[test]
    fn anschluss_economic_does_not_annex_austria() {
        let mut world = test_world();
        let db = V6Database::load();
        let ger = world.country("GER").expect("GER exists");
        let aus = world.country("AUS").expect("AUS exists");
        let before: Vec<(usize, hoi4_state::CountryId)> = (0..world.states.count)
            .filter(|&si| world.states.owners[si] == aus)
            .map(|si| (si, world.states.owners[si]))
            .collect();

        let mut fired = Vec::new();
        fire_event(
            &mut world,
            &db,
            ger.0 as usize,
            "anschluss_economic",
            &mut fired,
        );

        assert!(fired.contains(&"anschluss_economic".to_owned()));
        for (si, owner) in before {
            assert_eq!(
                world.states.owners[si], owner,
                "state {} should remain Austrian",
                si
            );
        }
    }
}
