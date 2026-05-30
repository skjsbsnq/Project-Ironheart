use std::collections::HashMap;
use std::sync::Arc;

use hoi4_content::v6_loader::{ConscriptionDef, PopModifiers, V6Database};
use hoi4_data::{
    Color, Country, CountryTag, DivisionTemplate, GameData, ReinforcementPriority, State,
    SubunitDef,
};
use hoi4_logic::economy::{law_modifiers, EconomyState};
use hoi4_logic::military::manpower::{conscription_policy, recruitable_manpower_breakdown};
use hoi4_logic::military::training::{enqueue_training, tick_training_queues};
use hoi4_map::{GameMap, Heightmap, ProvinceMap, TerrainBitmap, TerrainCatalog};
use hoi4_state::{
    Autonomy, AutonomyLevel, CountryId, LawCategory, LawSlot, PopClass, PopGroup, ProvinceId,
    StateId, StateIntegrationStatus, World,
};

fn test_map() -> Arc<GameMap> {
    Arc::new(GameMap {
        definitions: vec![],
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
        terrain_catalog: TerrainCatalog::default(),
        tree_definition_bmp: None,
        tree_indices: std::collections::HashSet::new(),
    })
}

fn test_data() -> Arc<GameData> {
    let mut data = GameData::default();
    for (tag, capital, color) in [("ENG", 1, [60, 80, 120]), ("RAJ", 3, [120, 80, 60])] {
        let country_tag = CountryTag::new(tag);
        data.countries.insert(
            country_tag.clone(),
            Country {
                tag: country_tag,
                color: Color {
                    r: color[0],
                    g: color[1],
                    b: color[2],
                },
                graphical_culture: "western_european_gfx".to_owned(),
                capital,
                ruling_party: "democratic".to_owned(),
                technologies: vec![],
            },
        );
    }
    data.states.push(State {
        id: 1,
        name: "Home".to_owned(),
        manpower: 1_000_000,
        owner: CountryTag::new("ENG"),
        cores: vec![CountryTag::new("ENG")],
        provinces: vec![1],
        category: "city".to_owned(),
        infrastructure: 5,
        victory_points: vec![],
        resources: vec![],
    });
    data.states.push(State {
        id: 2,
        name: "Colony".to_owned(),
        manpower: 1_000_000,
        owner: CountryTag::new("ENG"),
        cores: vec![],
        provinces: vec![2],
        category: "rural".to_owned(),
        infrastructure: 2,
        victory_points: vec![],
        resources: vec![],
    });
    data.states.push(State {
        id: 3,
        name: "Subject".to_owned(),
        manpower: 1_000_000,
        owner: CountryTag::new("RAJ"),
        cores: vec![CountryTag::new("RAJ")],
        provinces: vec![3],
        category: "rural".to_owned(),
        infrastructure: 2,
        victory_points: vec![],
        resources: vec![],
    });

    let mut infantry_need = HashMap::new();
    infantry_need.insert("infantry_equipment".to_owned(), 100);
    data.subunits.insert(
        "infantry".to_owned(),
        SubunitDef {
            key: "infantry".to_owned(),
            group: "infantry".to_owned(),
            combat_width: 2.0,
            manpower: 1_000,
            training_time: 4,
            need: infantry_need,
            ..SubunitDef::default()
        },
    );
    data.division_templates.insert(
        "ENG".to_owned(),
        vec![DivisionTemplate {
            name: "Infantry Division".to_owned(),
            country_tag: Some("ENG".to_owned()),
            regiments: vec!["infantry".to_owned()],
            support: vec![],
            division_names_group: None,
        }],
    );
    Arc::new(data)
}

fn conscription_db() -> V6Database {
    V6Database {
        conscription_laws: vec![ConscriptionDef {
            id: "limited_conscription".to_owned(),
            name: "Limited".to_owned(),
            pp_cost: 0,
            soldier_ratio: 0.10,
            domestic_recruitable_ratio: 1.0,
            colonial_recruitable_ratio: 0.25,
            subject_force_contribution_ratio: 0.0,
            political_cost_multiplier: 1.0,
            radicalism_gain_multiplier: 1.0,
            cooldown_days: 0,
            conscription_conversion_rate: 365.0,
            pop_modifiers: PopModifiers {
                satisfaction: 0.0,
                loyalty_coefficient: 1.0,
            },
        }],
        ..V6Database::default()
    }
}

fn pop(class: PopClass, state: StateId, size: u32) -> PopGroup {
    PopGroup {
        class,
        state,
        size,
        employed_at: None,
        wage_rm: 0.0,
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
        literacy: class.baseline_literacy(),
        skilled_ratio: class.baseline_skilled_ratio(),
        standard_of_living: 0.5,
        needs_fulfillment: 1.0,
        essential_needs_fulfillment: 1.0,
        normal_needs_fulfillment: 1.0,
        luxury_needs_fulfillment: 1.0,
        radicalism: 0.0,
    }
}

fn world() -> World {
    let mut world = World::new(test_map(), test_data());
    world.states.provinces[0] = vec![ProvinceId(0)];
    world.states.provinces[1] = vec![ProvinceId(1)];
    world.states.provinces[2] = vec![ProvinceId(2)];
    world.states.controllers[0] = CountryId(0);
    world.states.controllers[1] = CountryId(0);
    world.states.controllers[2] = CountryId(1);
    world.states.integration_status[0] = StateIntegrationStatus::Metropole;
    world.states.integration_status[1] = StateIntegrationStatus::Colony;
    world.states.integration_status[2] = StateIntegrationStatus::Metropole;
    world.countries.law_store.law_sets[0].0[LawCategory::Conscription.index()] =
        LawSlot::new(LawCategory::Conscription, "limited_conscription");
    world.countries.pops.groups.clear();
    world
        .countries
        .pops
        .groups
        .push(pop(PopClass::Peasant, StateId(0), 100_000));
    world
        .countries
        .pops
        .groups
        .push(pop(PopClass::Peasant, StateId(1), 100_000));
    world
        .countries
        .pops
        .groups
        .push(pop(PopClass::Peasant, StateId(2), 1_000_000));
    world.diplomacy.autonomy.insert(
        CountryId(1),
        Autonomy {
            master: CountryId(0),
            subject: CountryId(1),
            level: AutonomyLevel::Puppet,
            progress: 0.0,
            since_hour: 0,
        },
    );
    world
}

#[test]
fn subject_population_not_master_manpower() {
    let world = world();
    let db = conscription_db();
    let policy = conscription_policy(&world, &db, CountryId(0));
    let breakdown = recruitable_manpower_breakdown(&world, CountryId(0), policy);

    assert_eq!(breakdown.domestic, 10_000);
    assert_eq!(breakdown.colonial, 2_500);
    assert_eq!(breakdown.subject, 0);
    assert_eq!(world.manpower(CountryId(0)), 0);
}

#[test]
fn colonial_conscription_has_political_cost() {
    let mut world = world();
    let db = conscription_db();
    law_modifiers::apply(&mut world, &db, 0);

    let home_soldiers: u32 = world
        .countries
        .pops
        .groups
        .iter()
        .filter(|pg| pg.class == PopClass::Soldier && pg.state == StateId(0))
        .map(|pg| pg.size)
        .sum();
    let colonial_soldiers: u32 = world
        .countries
        .pops
        .groups
        .iter()
        .filter(|pg| pg.class == PopClass::Soldier && pg.state == StateId(1))
        .map(|pg| pg.size)
        .sum();
    let colony_radicalism = world
        .countries
        .pops
        .groups
        .iter()
        .find(|pg| pg.class == PopClass::Peasant && pg.state == StateId(1))
        .unwrap()
        .radicalism;
    let subject_soldiers: u32 = world
        .countries
        .pops
        .groups
        .iter()
        .filter(|pg| pg.class == PopClass::Soldier && pg.state == StateId(2))
        .map(|pg| pg.size)
        .sum();

    assert_eq!(home_soldiers, 10_000);
    assert_eq!(colonial_soldiers, 2_500);
    assert_eq!(subject_soldiers, 0);
    assert!(
        colony_radicalism > 0.0,
        "colonial draft should raise radicalism"
    );
}

#[test]
fn training_queue_records_domestic_and_colonial_manpower_sources() {
    let mut world = world();
    world.countries.pops.groups.clear();
    world
        .countries
        .pops
        .groups
        .push(pop(PopClass::Soldier, StateId(0), 500));
    world
        .countries
        .pops
        .groups
        .push(pop(PopClass::Soldier, StateId(1), 800));
    let data = world.data.clone();
    let mut econ = EconomyState::new(&world);
    econ.stockpile[0].insert("infantry_equipment".to_owned(), 100.0);

    enqueue_training(
        &world,
        &mut econ,
        data.as_ref(),
        CountryId(0),
        0,
        1,
        StateId(0),
        ReinforcementPriority::Normal,
    )
    .unwrap();
    for _ in 0..3 {
        tick_training_queues(&mut world, &mut econ, data.as_ref());
    }

    let item = &econ.training_queues[0][0];
    assert_eq!(item.domestic_manpower_allocated, 500);
    assert!(item.colonial_manpower_allocated > 0);
    assert_eq!(item.subject_manpower_allocated, 0);
}
