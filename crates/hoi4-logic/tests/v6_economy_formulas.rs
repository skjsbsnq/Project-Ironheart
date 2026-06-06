use std::collections::HashMap;
use std::sync::Arc;

use hoi4_content::v6_loader::{
    BuildingDef, BuildingEmploymentProfileDef, BuildingGameplayClassDef, BuildingGdpComponentDef,
    BuildingGdpRuleDef, BuildingKindDef, CivilRightsDef, ConscriptionDef, ConstructionMaterialDef,
    ConstructionRecipeDef, EconomicSectorDef, EconomyDef, EquipmentOutputDef, GoodCategoryDef,
    GoodDef, InformationControlDef, MefoDef, OwnerDef, PopClassNeedsDef, PopModifiers,
    PopNeedEntryDef, PopNeedTierDef, ProductionMethodDef, PyatiletkaDef, PyatiletkaTargetDef,
    TaxationDef, TradeDef, V6Database, V6EventDef, V6EventOption,
};
use hoi4_content::{ResourceDepositDef, StateResourceDepositDef};
use hoi4_data::{Color, Country, CountryTag, DivisionTemplate, GameData, State, SubunitDef};
use hoi4_logic::economy::{tick_daily_v6, BuildOrder, EconomyState};
use hoi4_map::{GameMap, Heightmap, ProvinceMap, TerrainBitmap, TerrainCatalog};
use hoi4_state::{
    Building, BuildingId, BuildingKind, BuildingOwner, LawCategory, LawSlot, PopClass, PopGroup,
    ProvinceId, StateId, World,
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
    let tag = CountryTag::new("GER");
    data.countries.insert(
        tag.clone(),
        Country {
            tag: tag.clone(),
            color: Color {
                r: 80,
                g: 80,
                b: 80,
            },
            graphical_culture: "western_european_gfx".to_owned(),
            capital: 1,
            ruling_party: "fascism".to_owned(),
            technologies: vec!["industrial_production".to_owned()],
        },
    );
    data.states.push(State {
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
    let mut infantry_need = HashMap::new();
    infantry_need.insert("infantry_equipment".to_owned(), 100);
    data.subunits.insert(
        "infantry".to_owned(),
        SubunitDef {
            key: "infantry".to_owned(),
            manpower: 1_000,
            need: infantry_need,
            ..SubunitDef::default()
        },
    );
    data.division_templates.insert(
        "GER".to_owned(),
        vec![DivisionTemplate {
            name: "步兵师".to_owned(),
            country_tag: Some("GER".to_owned()),
            regiments: vec!["infantry".to_owned()],
            support: vec![],
            division_names_group: None,
        }],
    );
    Arc::new(data)
}

fn test_pm(id: &str, name: &str, building_id: &str) -> ProductionMethodDef {
    ProductionMethodDef {
        id: id.to_owned(),
        name: name.to_owned(),
        building_id: building_id.to_owned(),
        group: "base".to_owned(),
        group_name: "基础工艺".to_owned(),
        input_good_ids: vec![],
        input_good_amounts: vec![],
        output_good_ids: vec![],
        output_good_amounts: vec![],
        employment_demand: [0; 6],
        unlocked_by: None,
        required_law: None,
        throughput_modifier: 1.0,
        automation_modifier: 1.0,
        required_literacy: 0.0,
        required_skilled_ratio: 0.0,
        equipment_output: None,
    }
}

fn test_construction_recipe(good_id: &str) -> ConstructionRecipeDef {
    ConstructionRecipeDef {
        cp_cost: 1_000.0,
        funds_rm: 50_000_000.0,
        materials: vec![ConstructionMaterialDef {
            good_id: good_id.to_owned(),
            amount: 20.0,
        }],
        labor: 10,
        engineering: 5,
        regional_restrictions: Vec::new(),
    }
}

fn base_db() -> V6Database {
    V6Database {
        goods: vec![
            GoodDef {
                id: "steel".to_owned(),
                name: "Steel".to_owned(),
                category: GoodCategoryDef::RawMaterial,
                base_price_rm: 1.0,
                unlocked_by: None,
            },
            GoodDef {
                id: "coal".to_owned(),
                name: "Coal".to_owned(),
                category: GoodCategoryDef::RawMaterial,
                base_price_rm: 1.0,
                unlocked_by: None,
            },
            GoodDef {
                id: "small_arms".to_owned(),
                name: "Small Arms".to_owned(),
                category: GoodCategoryDef::MilitaryIntermediate,
                base_price_rm: 1.0,
                unlocked_by: None,
            },
        ],
        buildings: vec![BuildingDef {
            id: "steel_mill".to_owned(),
            name: "Steel Mill".to_owned(),
            description: "Test steel mill".to_owned(),
            economic_sector: EconomicSectorDef::Secondary,
            gameplay_class: BuildingGameplayClassDef::HeavyIndustry,
            gdp_rule: BuildingGdpRuleDef {
                component: BuildingGdpComponentDef::SecondaryOutput,
                value_added_multiplier: 1.0,
            },
            kind: BuildingKindDef::Industrial,
            max_level: 15,
            owner_default: OwnerDef::Private,
            buildable: true,
            group: "城市工业".to_owned(),
            state_limit_kind: None,
            requires_law: None,
            employment_profile: BuildingEmploymentProfileDef {
                peasants: 0,
                workers: 10,
                clerks: 0,
                capitalists: 0,
                aristocrats: 0,
                soldiers: 0,
            },
            construction_recipe: test_construction_recipe("steel"),
        }],
        production_methods: vec![
            ProductionMethodDef {
                input_good_ids: vec!["coal".to_owned()],
                input_good_amounts: vec![10.0],
                output_good_ids: vec!["steel".to_owned()],
                output_good_amounts: vec![20.0],
                employment_demand: [0, 10, 0, 0, 0, 0],
                ..test_pm("steel_mill_default", "Steel", "steel_mill")
            },
            ProductionMethodDef {
                group: "military".to_owned(),
                group_name: "军工型号".to_owned(),
                input_good_ids: vec!["steel".to_owned()],
                input_good_amounts: vec![10.0],
                output_good_ids: vec![],
                output_good_amounts: vec![],
                employment_demand: [0, 10, 0, 0, 0, 0],
                equipment_output: Some(EquipmentOutputDef {
                    equipment_category: "infantry_equipment".to_owned(),
                    daily_per_level: 4.5,
                }),
                ..test_pm("arms_industry_default", "Arms", "arms_industry")
            },
        ],
        conscription_laws: vec![
            ConscriptionDef {
                id: "volunteer_only".to_owned(),
                name: "Volunteer".to_owned(),
                pp_cost: 0,
                soldier_ratio: 0.01,
                domestic_recruitable_ratio: 1.0,
                colonial_recruitable_ratio: 0.05,
                subject_force_contribution_ratio: 0.0,
                political_cost_multiplier: 0.25,
                radicalism_gain_multiplier: 0.25,
                cooldown_days: 0,
                conscription_conversion_rate: 0.05,
                pop_modifiers: PopModifiers {
                    satisfaction: 0.05,
                    loyalty_coefficient: 1.0,
                },
            },
            ConscriptionDef {
                id: "total_mobilization".to_owned(),
                name: "Total Mobilization".to_owned(),
                pp_cost: 0,
                soldier_ratio: 0.10,
                domestic_recruitable_ratio: 1.0,
                colonial_recruitable_ratio: 0.50,
                subject_force_contribution_ratio: 0.10,
                political_cost_multiplier: 2.0,
                radicalism_gain_multiplier: 2.0,
                cooldown_days: 0,
                conscription_conversion_rate: 0.15,
                pop_modifiers: PopModifiers {
                    satisfaction: -0.15,
                    loyalty_coefficient: 1.0,
                },
            },
        ],
        economy_laws: vec![
            EconomyDef {
                id: "interventionism".to_owned(),
                name: "Intervention".to_owned(),
                pp_cost: 0,
                cooldown_days: 0,
                wage_multiplier_worker: 1.0,
                wage_multiplier_capitalist: 1.0,
                consumer_goods_factor: 1.0,
                construction_speed_modifier: 1.0,
                forces_trade_law: None,
                pop_modifiers: PopModifiers {
                    satisfaction: 0.0,
                    loyalty_coefficient: 1.0,
                },
            },
            EconomyDef {
                id: "planned_economy".to_owned(),
                name: "Planned".to_owned(),
                pp_cost: 0,
                cooldown_days: 0,
                wage_multiplier_worker: 1.0,
                wage_multiplier_capitalist: 1.0,
                consumer_goods_factor: 1.0,
                construction_speed_modifier: 1.0,
                forces_trade_law: None,
                pop_modifiers: PopModifiers {
                    satisfaction: 0.0,
                    loyalty_coefficient: 1.0,
                },
            },
            EconomyDef {
                id: "corporatist_war_economy".to_owned(),
                name: "Corporatist War Economy".to_owned(),
                pp_cost: 0,
                cooldown_days: 0,
                wage_multiplier_worker: 1.0,
                wage_multiplier_capitalist: 1.0,
                consumer_goods_factor: 1.0,
                construction_speed_modifier: 1.0,
                forces_trade_law: None,
                pop_modifiers: PopModifiers {
                    satisfaction: 0.0,
                    loyalty_coefficient: 1.0,
                },
            },
        ],
        trade_laws: vec![],
        taxation_laws: vec![
            TaxationDef {
                id: "medium_taxation".to_owned(),
                name: "Medium".to_owned(),
                pp_cost: 0,
                cooldown_days: 0,
                income_tax_rate: 0.20,
                consumption_tax_rate: 0.10,
                corporate_tax_rate: 0.20,
                pop_modifiers: PopModifiers {
                    satisfaction: 0.0,
                    loyalty_coefficient: 1.0,
                },
            },
            TaxationDef {
                id: "war_taxation".to_owned(),
                name: "War Taxation".to_owned(),
                pp_cost: 0,
                cooldown_days: 0,
                income_tax_rate: 0.50,
                consumption_tax_rate: 0.30,
                corporate_tax_rate: 0.45,
                pop_modifiers: PopModifiers {
                    satisfaction: -0.15,
                    loyalty_coefficient: 0.8,
                },
            },
        ],
        civil_rights_laws: vec![
            CivilRightsDef {
                id: "limited_rights".to_owned(),
                name: "Limited".to_owned(),
                pp_cost: 0,
                cooldown_days: 0,
                research_slots: 4,
                welfare_rate: 0.001,
                pop_modifiers: PopModifiers {
                    satisfaction: 0.0,
                    loyalty_coefficient: 1.0,
                },
            },
            CivilRightsDef {
                id: "police_state".to_owned(),
                name: "Police State".to_owned(),
                pp_cost: 0,
                cooldown_days: 0,
                research_slots: 2,
                welfare_rate: 0.0,
                pop_modifiers: PopModifiers {
                    satisfaction: -0.15,
                    loyalty_coefficient: 0.3,
                },
            },
        ],
        information_control_laws: vec![
            InformationControlDef {
                id: "regulated_press".to_owned(),
                name: "Regulated Press".to_owned(),
                pp_cost: 0,
                cooldown_days: 0,
                loyalty_decay_multiplier: 1.0,
                pop_modifiers: PopModifiers {
                    satisfaction: 0.0,
                    loyalty_coefficient: 1.0,
                },
            },
            InformationControlDef {
                id: "total_propaganda".to_owned(),
                name: "Total Propaganda".to_owned(),
                pp_cost: 0,
                cooldown_days: 0,
                loyalty_decay_multiplier: 0.2,
                pop_modifiers: PopModifiers {
                    satisfaction: -0.05,
                    loyalty_coefficient: 0.6,
                },
            },
        ],
        pop_needs: vec![PopClassNeedsDef {
            class: PopClass::Worker,
            needs: vec![PopNeedEntryDef {
                good_id: "steel".to_owned(),
                tier: PopNeedTierDef::Essential,
                amount_per_million: 10.0,
            }],
        }],
        initial_pops: HashMap::new(),
        pyatiletka_plans: vec![],
        events_v6: vec![],
        mefo: MefoDef::default(),
        technologies: vec![],
        state_resource_deposits: vec![],
        state_populations: vec![],
        historical_countries: vec![],
        historical_trade_routes: vec![],
    }
}

fn db_with_trade() -> V6Database {
    let mut db = base_db();
    db.trade_laws.push(TradeDef {
        id: "export_focus".to_owned(),
        name: "Export Focus".to_owned(),
        pp_cost: 0,
        cooldown_days: 0,
        import_tariff_rate: 0.20,
        export_tariff_rate: 0.10,
        import_efficiency: 1.0,
        export_efficiency: 1.0,
        foreign_exchange_control: false,
        trade_law_modifier: 1.0,
        pop_modifiers: PopModifiers {
            satisfaction: 0.0,
            loyalty_coefficient: 1.0,
        },
    });
    db
}

fn add_resource_building_def(db: &mut V6Database, building_id: &str, deposit_kind: &str) {
    db.buildings.push(BuildingDef {
        id: building_id.to_owned(),
        name: building_id.to_owned(),
        description: format!("Test resource building {building_id}"),
        economic_sector: EconomicSectorDef::Primary,
        gameplay_class: BuildingGameplayClassDef::ResourceExtraction,
        gdp_rule: BuildingGdpRuleDef {
            component: BuildingGdpComponentDef::PrimaryOutput,
            value_added_multiplier: 1.0,
        },
        kind: BuildingKindDef::Resource,
        max_level: 10,
        owner_default: OwnerDef::Private,
        buildable: true,
        group: "资源与农业".to_owned(),
        state_limit_kind: Some(deposit_kind.to_owned()),
        requires_law: None,
        employment_profile: BuildingEmploymentProfileDef {
            peasants: 0,
            workers: 10,
            clerks: 0,
            capitalists: 0,
            aristocrats: 0,
            soldiers: 0,
        },
        construction_recipe: test_construction_recipe("steel"),
    });
    db.production_methods.push(test_pm(
        &format!("{building_id}_default"),
        building_id,
        building_id,
    ));
}

fn world_with_law(economy_law: &str) -> World {
    let mut world = World::new(test_map(), test_data());
    world.player = hoi4_state::CountryId(0);
    let ci = 0;
    world.countries.law_store.law_sets[ci].0[LawCategory::Economy.index()] =
        LawSlot::new(LawCategory::Economy, economy_law);
    world.countries.law_store.law_sets[ci].0[LawCategory::Taxation.index()] =
        LawSlot::new(LawCategory::Taxation, "medium_taxation");
    world.countries.law_store.law_sets[ci].0[LawCategory::CivilRights.index()] =
        LawSlot::new(LawCategory::CivilRights, "limited_rights");
    world.countries.law_store.law_sets[ci].0[LawCategory::Trade.index()] =
        LawSlot::new(LawCategory::Trade, "export_focus");
    world.countries.treasury.treasuries[ci].cash_rm = 1_000_000.0;
    world.countries.research_slots[ci] = 0;
    world.countries.pops.groups.push(PopGroup {
        class: PopClass::Worker,
        state: StateId(0),
        size: 100,
        employed_at: Some(BuildingId(0)),
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
        literacy: PopClass::Worker.baseline_literacy(),
        skilled_ratio: PopClass::Worker.baseline_skilled_ratio(),
        standard_of_living: 0.5,
        needs_fulfillment: 1.0,
        essential_needs_fulfillment: 1.0,
        normal_needs_fulfillment: 1.0,
        luxury_needs_fulfillment: 1.0,
        radicalism: 0.0,
    });
    world.countries.pops.groups.push(PopGroup {
        class: PopClass::Capitalist,
        state: StateId(0),
        size: 10,
        employed_at: Some(BuildingId(0)),
        wage_rm: 30.0,
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

fn add_building(world: &mut World, building_def_id: &str, active_pm: &str, owner: BuildingOwner) {
    world.countries.buildings_v6.buildings.push(Building {
        kind: BuildingKind::Industrial,
        building_def_id: building_def_id.to_owned(),
        state: StateId(0),
        level: 1,
        active_pm: active_pm.to_owned(),
        employment: [0, 10, 0, 0, 0, 0],
        owner,
        requires_law: None,
        built_progress: 1.0,
        ..Building::runtime_defaults()
    });
}

#[test]
fn daily_accumulator_zeroes() {
    let mut baseline = world_with_law("interventionism");
    add_building(
        &mut baseline,
        "steel_mill",
        "steel_mill_default",
        BuildingOwner::State,
    );
    let mut world = world_with_law("interventionism");
    add_building(
        &mut world,
        "steel_mill",
        "steel_mill_default",
        BuildingOwner::State,
    );
    world.countries.treasury.treasuries[0].daily_income_rm = 365_000_000.0;
    world.countries.treasury.treasuries[0].daily_expense_rm = 730_000_000.0;
    let db = base_db();
    let mut baseline_econ = EconomyState::new(&baseline);
    let mut econ = EconomyState::new(&world);

    tick_daily_v6(&mut baseline, &mut baseline_econ, &db, 1);
    tick_daily_v6(&mut world, &mut econ, &db, 1);

    assert_eq!(
        world.countries.treasury.treasuries[0].daily_income_rm,
        baseline.countries.treasury.treasuries[0].daily_income_rm
    );
    assert_eq!(
        world.countries.treasury.treasuries[0].daily_expense_rm,
        baseline.countries.treasury.treasuries[0].daily_expense_rm
    );
}

#[test]
fn consumption_tax_real_rate() {
    let mut world = world_with_law("interventionism");
    add_building(
        &mut world,
        "steel_mill",
        "steel_mill_default",
        BuildingOwner::State,
    );
    let db = base_db();
    let mut econ = EconomyState::new(&world);

    tick_daily_v6(&mut world, &mut econ, &db, 1);

    let income_tax = 100.0 * 3.0 * 0.20 + 10.0 * 15.0 * 0.20;
    let consumption_tax = 100.0 * 3.0 * 0.8 * 0.10 + 10.0 * 15.0 * 2.0 * 0.10;
    let actual = world.countries.treasury.treasuries[0].daily_income_rm;
    assert!(
        (actual - (income_tax + consumption_tax)).abs() < 0.01,
        "actual={actual}"
    );
}

#[test]
fn planned_no_income_tax() {
    let mut world = world_with_law("planned_economy");
    let db = base_db();
    let mut econ = EconomyState::new(&world);

    tick_daily_v6(&mut world, &mut econ, &db, 1);

    assert_eq!(world.countries.treasury.treasuries[0].daily_income_rm, 0.0);
    assert_eq!(world.countries.pops.groups[0].tax_burden, 0.0);
}

#[test]
fn tax_burden_varies_by_class() {
    let mut world = world_with_law("interventionism");
    let db = base_db();
    let mut econ = EconomyState::new(&world);

    tick_daily_v6(&mut world, &mut econ, &db, 1);

    let worker = world
        .countries
        .pops
        .groups
        .iter()
        .find(|p| p.class == PopClass::Worker)
        .unwrap();
    let capitalist = world
        .countries
        .pops
        .groups
        .iter()
        .find(|p| p.class == PopClass::Capitalist)
        .unwrap();
    assert!(capitalist.tax_burden > worker.tax_burden);
}

#[test]
fn pop_needs_shortage_lowers_satisfaction() {
    let mut world = world_with_law("interventionism");
    world
        .countries
        .pops
        .groups
        .retain(|pg| pg.class == PopClass::Worker);
    world.countries.pops.groups[0].satisfaction = 0.8;
    world.countries.market.markets[0]
        .supply
        .insert("grain".to_owned(), 0.0);
    world.countries.market.markets[0]
        .supply
        .insert("clothes".to_owned(), 0.0);
    world.countries.market.markets[0]
        .stockpile
        .insert("grain".to_owned(), 0.0);
    world.countries.market.markets[0]
        .stockpile
        .insert("clothes".to_owned(), 0.0);
    let db = base_db();
    let mut econ = EconomyState::new(&world);

    tick_daily_v6(&mut world, &mut econ, &db, 7);

    let worker = world
        .countries
        .pops
        .groups
        .iter()
        .find(|p| p.class == PopClass::Worker)
        .unwrap();
    assert!(
        worker.essential_needs_fulfillment < 0.1,
        "essential needs should collapse under grain/clothes shortage"
    );
    assert!(
        worker.satisfaction < 0.8,
        "satisfaction should move down when essential needs are unmet"
    );
}

#[test]
fn pop_shortage_and_unemployment_raise_radicalism_and_reduce_stability() {
    let mut world = world_with_law("interventionism");
    world
        .countries
        .pops
        .groups
        .retain(|pg| pg.class == PopClass::Worker);
    world.countries.pops.groups[0].employed_at = None;
    world.countries.pops.groups[0].satisfaction = 0.1;
    world.countries.pops.groups[0].essential_needs_fulfillment = 0.0;
    world.countries.pops.groups[0].tax_burden = 0.8;
    world.countries.stability[0] = 0.8;
    world.countries.market.markets[0]
        .supply
        .insert("grain".to_owned(), 0.0);
    world.countries.market.markets[0]
        .supply
        .insert("clothes".to_owned(), 0.0);
    world.countries.market.markets[0]
        .stockpile
        .insert("grain".to_owned(), 0.0);
    world.countries.market.markets[0]
        .stockpile
        .insert("clothes".to_owned(), 0.0);
    let db = base_db();
    let mut econ = EconomyState::new(&world);

    for day in (7..=84).step_by(7) {
        tick_daily_v6(&mut world, &mut econ, &db, day);
    }

    let worker = world
        .countries
        .pops
        .groups
        .iter()
        .find(|p| p.class == PopClass::Worker)
        .unwrap();
    assert!(worker.radicalism > 0.05, "radicalism={}", worker.radicalism);
    assert!(
        world.countries.stability[0] < 0.8,
        "stability should fall under sustained radicalism"
    );
}

#[test]
fn i17_conscription_law_changes_soldier_ratio_and_recruits_same_tick() {
    let mut world = world_with_law("interventionism");
    world.countries.law_store.law_sets[0].0[LawCategory::Conscription.index()] =
        LawSlot::new(LawCategory::Conscription, "total_mobilization");
    world.countries.pops.groups.clear();
    world.countries.pops.groups.push(PopGroup {
        class: PopClass::Worker,
        state: StateId(0),
        size: 10_000,
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
        literacy: PopClass::Worker.baseline_literacy(),
        skilled_ratio: PopClass::Worker.baseline_skilled_ratio(),
        standard_of_living: 0.5,
        needs_fulfillment: 1.0,
        essential_needs_fulfillment: 1.0,
        normal_needs_fulfillment: 1.0,
        luxury_needs_fulfillment: 1.0,
        radicalism: 0.0,
    });
    let db = base_db();
    let mut econ = EconomyState::new(&world);

    tick_daily_v6(&mut world, &mut econ, &db, 1);

    assert_eq!(world.countries.conscription_max_ratio[0], 0.10);
    assert_eq!(world.countries.conscription_recruit_speed_mult[0], 0.15);
    let soldiers: u32 = world
        .countries
        .pops
        .groups
        .iter()
        .filter(|pg| pg.class == PopClass::Soldier)
        .map(|pg| pg.size)
        .sum();
    assert!(
        soldiers > 0,
        "conscription should convert civilian POPs into Soldier POPs on the same tick"
    );
}

#[test]
fn mobilization_reduces_available_workers() {
    let mut world = world_with_law("interventionism");
    world.countries.law_store.law_sets[0].0[LawCategory::Conscription.index()] =
        LawSlot::new(LawCategory::Conscription, "total_mobilization");
    world.countries.pops.groups.clear();
    world.countries.buildings_v6.buildings.clear();
    world.countries.buildings_v6.buildings.push(Building {
        kind: BuildingKind::Industrial,
        building_def_id: "steel_mill".to_owned(),
        state: StateId(0),
        level: 1,
        active_pm: "steel_mill_default".to_owned(),
        employment: [0, 100, 0, 0, 0, 0],
        owner: BuildingOwner::State,
        requires_law: None,
        built_progress: 1.0,
        ..Building::runtime_defaults()
    });
    world.countries.pops.groups.push(PopGroup {
        class: PopClass::Worker,
        state: StateId(0),
        size: 100,
        employed_at: Some(BuildingId(0)),
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
        literacy: PopClass::Worker.baseline_literacy(),
        skilled_ratio: PopClass::Worker.baseline_skilled_ratio(),
        standard_of_living: 0.5,
        needs_fulfillment: 1.0,
        essential_needs_fulfillment: 1.0,
        normal_needs_fulfillment: 1.0,
        luxury_needs_fulfillment: 1.0,
        radicalism: 0.0,
    });
    let db = base_db();
    let mut econ = EconomyState::new(&world);

    tick_daily_v6(&mut world, &mut econ, &db, 1);

    let employed = world.countries.buildings_v6.buildings[0].employment[PopClass::Worker.index()];
    let soldiers: u32 = world
        .countries
        .pops
        .groups
        .iter()
        .filter(|pg| pg.class == PopClass::Soldier)
        .map(|pg| pg.size)
        .sum();
    assert!(
        employed < 100,
        "mobilization should pull workers from factory jobs"
    );
    assert!(soldiers > 0, "mobilization should create soldier POP");
}

#[test]
fn combat_casualties_reduce_stability_and_pop_satisfaction() {
    let mut world = world_with_law("interventionism");
    world.countries.war_support[0] = 0.5;
    world.countries.pops.groups.clear();
    world.countries.pops.groups.push(PopGroup {
        class: PopClass::Worker,
        state: StateId(0),
        size: 10_000,
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
        satisfaction: 0.7,
        political_loyalty: 0.0,
        literacy: PopClass::Worker.baseline_literacy(),
        skilled_ratio: PopClass::Worker.baseline_skilled_ratio(),
        standard_of_living: 0.7,
        needs_fulfillment: 1.0,
        essential_needs_fulfillment: 1.0,
        normal_needs_fulfillment: 1.0,
        luxury_needs_fulfillment: 1.0,
        radicalism: 0.0,
    });
    world.countries.pops.groups.push(PopGroup {
        class: PopClass::Soldier,
        state: StateId(0),
        size: 1_000,
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
        satisfaction: 0.7,
        political_loyalty: 0.0,
        literacy: PopClass::Soldier.baseline_literacy(),
        skilled_ratio: PopClass::Soldier.baseline_skilled_ratio(),
        standard_of_living: 0.7,
        needs_fulfillment: 1.0,
        essential_needs_fulfillment: 1.0,
        normal_needs_fulfillment: 1.0,
        luxury_needs_fulfillment: 1.0,
        radicalism: 0.0,
    });
    world.divisions.push(
        hoi4_state::CountryId(0),
        ProvinceId(0),
        0,
        30.0,
        100.0,
        "Loss Test".to_owned(),
    );
    let data = world.data.clone();
    let mut econ = EconomyState::new(&world);
    let stability_before = world.countries.stability[0];
    let war_support_before = world.countries.war_support[0];
    let satisfaction_before = world.countries.pops.groups[0].satisfaction;
    let soldier_before = world.countries.pops.groups[1].size;

    hoi4_logic::economy::stockpile::deduct_combat_losses(
        &mut world,
        &mut econ,
        data.as_ref(),
        0,
        0.5,
    );

    assert!(world.countries.stability[0] < stability_before);
    assert!(world.countries.war_support[0] < war_support_before);
    assert!(world.countries.pops.groups[0].satisfaction < satisfaction_before);
    assert!(world.countries.pops.groups[1].size < soldier_before);
}

#[test]
fn i18_war_taxation_changes_daily_income_same_tick() {
    let mut medium_world = world_with_law("interventionism");
    add_building(
        &mut medium_world,
        "steel_mill",
        "steel_mill_default",
        BuildingOwner::State,
    );
    let mut war_world = world_with_law("interventionism");
    war_world.countries.law_store.law_sets[0].0[LawCategory::Taxation.index()] =
        LawSlot::new(LawCategory::Taxation, "war_taxation");
    add_building(
        &mut war_world,
        "steel_mill",
        "steel_mill_default",
        BuildingOwner::State,
    );
    let db = base_db();
    let mut medium_econ = EconomyState::new(&medium_world);
    let mut war_econ = EconomyState::new(&war_world);

    tick_daily_v6(&mut medium_world, &mut medium_econ, &db, 1);
    tick_daily_v6(&mut war_world, &mut war_econ, &db, 1);

    let medium_income = medium_world.countries.treasury.treasuries[0].daily_income_rm;
    let war_income = war_world.countries.treasury.treasuries[0].daily_income_rm;
    assert_eq!(
        war_world.countries.treasury.treasuries[0].tax_rates,
        [0.50, 0.30, 0.45]
    );
    assert!(
        war_income >= medium_income * 1.30,
        "war income {war_income} should be at least 30% above medium {medium_income}"
    );
}

#[test]
fn i19_civil_rights_police_state_changes_pop_law_fields_same_tick() {
    let mut world = world_with_law("interventionism");
    world.countries.law_store.law_sets[0].0[LawCategory::CivilRights.index()] =
        LawSlot::new(LawCategory::CivilRights, "police_state");
    let db = base_db();
    let mut econ = EconomyState::new(&world);

    tick_daily_v6(&mut world, &mut econ, &db, 1);

    let worker = world
        .countries
        .pops
        .groups
        .iter()
        .find(|pg| pg.class == PopClass::Worker)
        .unwrap();
    assert_eq!(world.countries.research_slots[0], 2);
    assert!(worker.satisfaction_law_modifier < 0.0);
    assert_eq!(worker.loyalty_coefficient, 0.3);
}

#[test]
fn i20_total_propaganda_applies_loyalty_decay_multiplier_same_tick() {
    let mut world = world_with_law("interventionism");
    world.countries.law_store.law_sets[0].0[LawCategory::InformationControl.index()] =
        LawSlot::new(LawCategory::InformationControl, "total_propaganda");
    world.countries.pops.groups[0].satisfaction = 0.0;
    let db = base_db();
    let mut econ = EconomyState::new(&world);

    tick_daily_v6(&mut world, &mut econ, &db, 1);

    let worker = world
        .countries
        .pops
        .groups
        .iter()
        .find(|pg| pg.class == PopClass::Worker)
        .unwrap();
    assert_eq!(worker.loyalty_decay_mult, 0.2);
    assert!(worker.political_loyalty > -0.0015 && worker.political_loyalty < 0.0);
}

#[test]
fn pm_input_fulfillment() {
    let mut world = world_with_law("interventionism");
    world.countries.pops.groups.clear();
    add_building(
        &mut world,
        "arms_industry",
        "arms_industry_default",
        BuildingOwner::State,
    );
    world.countries.market.markets[0]
        .supply
        .insert("steel".to_owned(), 0.0);
    world.countries.market.markets[0]
        .stockpile
        .insert("steel".to_owned(), 0.0);
    let db = base_db();
    let mut econ = EconomyState::new(&world);

    tick_daily_v6(&mut world, &mut econ, &db, 1);

    assert_eq!(
        world.countries.market.markets[0]
            .supply
            .get("small_arms")
            .copied()
            .unwrap_or(0.0),
        0.0
    );
}

#[test]
fn planned_quota_multiplier_applies() {
    let mut world = world_with_law("planned_economy");
    world.countries.pops.groups.clear();
    add_building(
        &mut world,
        "steel_mill",
        "steel_mill_default",
        BuildingOwner::State,
    );
    world.countries.market.markets[0]
        .supply
        .insert("coal".to_owned(), 100.0);
    let mut db = base_db();
    db.pyatiletka_plans = vec![PyatiletkaDef {
        id: "test_plan".to_owned(),
        name: "Test Plan".to_owned(),
        year_start: 1936,
        year_end: 1936,
        focus_tech_directions: vec![],
        focus_bonus: 1.0,
        off_focus_penalty: 1.0,
        targets: vec![PyatiletkaTargetDef {
            good_id: "steel".to_owned(),
            target_daily_output: 40.0,
        }],
        forced_pms: vec![],
    }];
    let mut econ = EconomyState::new(&world);

    tick_daily_v6(&mut world, &mut econ, &db, 1);

    let steel_stockpile = world.countries.market.markets[0]
        .stockpile
        .get("steel")
        .copied()
        .unwrap_or(0.0);
    assert!(
        steel_stockpile > 0.0,
        "planned quota should produce steel, got {steel_stockpile}"
    );
}

#[test]
fn planned_rationing_uses_world_year() {
    fn remaining_steel_after_tick(year: u16) -> f32 {
        let mut world = world_with_law("planned_economy");
        world.date.year = year;
        world.countries.pops.groups[0].size = 1_000_000;
        world.countries.market.markets[0]
            .stockpile
            .insert("steel".to_owned(), 100.0);
        let mut db = base_db();
        db.pyatiletka_plans = vec![
            PyatiletkaDef {
                id: "plan_1936".to_owned(),
                name: "Plan 1936".to_owned(),
                year_start: 1936,
                year_end: 1936,
                focus_tech_directions: vec![],
                focus_bonus: 1.0,
                off_focus_penalty: 1.0,
                targets: vec![PyatiletkaTargetDef {
                    good_id: "steel".to_owned(),
                    target_daily_output: 2.0,
                }],
                forced_pms: vec![],
            },
            PyatiletkaDef {
                id: "plan_1938".to_owned(),
                name: "Plan 1938".to_owned(),
                year_start: 1938,
                year_end: 1938,
                focus_tech_directions: vec![],
                focus_bonus: 1.0,
                off_focus_penalty: 1.0,
                targets: vec![PyatiletkaTargetDef {
                    good_id: "steel".to_owned(),
                    target_daily_output: 6.0,
                }],
                forced_pms: vec![],
            },
            PyatiletkaDef {
                id: "plan_1941".to_owned(),
                name: "Plan 1941".to_owned(),
                year_start: 1941,
                year_end: 1941,
                focus_tech_directions: vec![],
                focus_bonus: 1.0,
                off_focus_penalty: 1.0,
                targets: vec![PyatiletkaTargetDef {
                    good_id: "steel".to_owned(),
                    target_daily_output: 9.0,
                }],
                forced_pms: vec![],
            },
        ];
        let mut econ = EconomyState::new(&world);

        tick_daily_v6(&mut world, &mut econ, &db, 1);

        world.countries.market.markets[0]
            .stockpile
            .get("steel")
            .copied()
            .unwrap_or(0.0)
    }

    assert!(
        (remaining_steel_after_tick(1936) - 97.8).abs() < 1.0,
        "1936 plan steel ~97.8, got {}",
        remaining_steel_after_tick(1936)
    );
    assert!(
        (remaining_steel_after_tick(1938) - 94.0).abs() < 2.0,
        "1938 plan steel ~94, got {}",
        remaining_steel_after_tick(1938)
    );
    assert!(
        (remaining_steel_after_tick(1941) - 91.0).abs() < 2.0,
        "1941 plan steel ~91, got {}",
        remaining_steel_after_tick(1941)
    );
}

#[test]
fn daily_and_hourly_spread_do_not_double_tick_same_day() {
    let mut world = world_with_law("planned_economy");
    world.countries.pops.groups.clear();
    add_building(
        &mut world,
        "steel_mill",
        "steel_mill_default",
        BuildingOwner::State,
    );
    world.countries.market.markets[0]
        .supply
        .insert("coal".to_owned(), 100.0);
    let mut db = base_db();
    db.pyatiletka_plans = vec![PyatiletkaDef {
        id: "test_plan".to_owned(),
        name: "Test Plan".to_owned(),
        year_start: 1936,
        year_end: 1936,
        focus_tech_directions: vec![],
        focus_bonus: 1.0,
        off_focus_penalty: 1.0,
        targets: vec![PyatiletkaTargetDef {
            good_id: "steel".to_owned(),
            target_daily_output: 40.0,
        }],
        forced_pms: vec![],
    }];
    let mut econ = EconomyState::new(&world);
    let day = world.date.days_since_epoch();

    tick_daily_v6(&mut world, &mut econ, &db, day);
    let stockpile_after_daily = world.countries.market.markets[0]
        .stockpile
        .get("steel")
        .copied()
        .unwrap_or(0.0);
    hoi4_logic::economy::tick_hourly_spread_v6(&mut world, &mut econ, &db);

    assert!(
        stockpile_after_daily > 0.0,
        "daily tick should produce steel: {stockpile_after_daily}"
    );
    assert_eq!(
        world.countries.market.markets[0]
            .stockpile
            .get("steel")
            .copied()
            .unwrap_or(0.0),
        stockpile_after_daily
    );
}

#[test]
fn equipment_category_12() {
    assert_eq!(hoi4_data::EquipmentCategory::COMBAT_CATEGORY_COUNT, 12);
    assert_eq!(
        hoi4_data::EquipmentCategory::from_str("naval_vessel"),
        hoi4_data::EquipmentCategory::Ship
    );
}

#[test]
fn unemployed_wage_zero() {
    let mut world = world_with_law("interventionism");
    world.countries.pops.groups[0].employed_at = None;
    let db = base_db();
    let mut econ = EconomyState::new(&world);

    tick_daily_v6(&mut world, &mut econ, &db, 1);

    assert_eq!(world.countries.pops.groups[0].wage_rm, 0.0);
}

#[test]
fn blocked_building_releases_employment() {
    let mut world = world_with_law("interventionism");
    world.countries.pops.groups.clear();
    world.countries.pops.groups.push(PopGroup {
        class: PopClass::Capitalist,
        state: StateId(0),
        size: 10,
        employed_at: Some(BuildingId(0)),
        wage_rm: 25.0,
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
    world.countries.buildings_v6.buildings.push(Building {
        kind: BuildingKind::Service,
        building_def_id: "bank".to_owned(),
        state: StateId(0),
        level: 1,
        active_pm: "bank_default".to_owned(),
        employment: [0, 0, 0, 10, 0, 0],
        owner: BuildingOwner::Private,
        requires_law: Some((LawCategory::Economy, "laissez_faire".to_owned())),
        built_progress: 1.0,
        ..Building::runtime_defaults()
    });
    let mut db = base_db();
    db.production_methods.push(ProductionMethodDef {
        employment_demand: [0, 0, 0, 10, 0, 0],
        ..test_pm("bank_default", "Bank", "bank")
    });
    let mut econ = EconomyState::new(&world);

    tick_daily_v6(&mut world, &mut econ, &db, 1);

    assert_eq!(world.countries.pops.groups[0].employed_at, None);
    assert_eq!(world.countries.buildings_v6.buildings[0].employment, [0; 6]);
}

#[test]
fn cartel_profit_splits_state_share_and_capitalist_income() {
    let mut world = world_with_law("interventionism");
    world.countries.pops.groups.clear();
    world.countries.pops.groups.push(PopGroup {
        class: PopClass::Capitalist,
        state: StateId(0),
        size: 10,
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
        literacy: PopClass::Capitalist.baseline_literacy(),
        skilled_ratio: PopClass::Capitalist.baseline_skilled_ratio(),
        standard_of_living: 0.5,
        needs_fulfillment: 1.0,
        essential_needs_fulfillment: 1.0,
        normal_needs_fulfillment: 1.0,
        luxury_needs_fulfillment: 1.0,
        radicalism: 0.0,
    });
    world.countries.buildings_v6.buildings.push(Building {
        kind: BuildingKind::Industrial,
        building_def_id: "steel_mill".to_owned(),
        state: StateId(0),
        level: 1,
        active_pm: "steel_mill_default".to_owned(),
        employment: [0, 10, 0, 0, 0, 0],
        owner: BuildingOwner::Cartel,
        requires_law: None,
        built_progress: 1.0,
        ..Building::runtime_defaults()
    });
    world.countries.market.markets[0]
        .price
        .insert("steel".to_owned(), 2.0);
    world.countries.market.markets[0]
        .price
        .insert("coal".to_owned(), 1.0);
    let db = base_db();
    let mut econ = EconomyState::new(&world);

    tick_daily_v6(&mut world, &mut econ, &db, 1);

    let profit = 20.0 * 2.0 - 10.0 * 1.0;
    let expected_state_income = profit * 0.30 + profit * 0.30 * 0.20;
    assert!(
        (world.countries.treasury.treasuries[0].daily_income_rm - expected_state_income).abs()
            < 0.01
    );
    assert!((world.countries.pops.groups[0].wage_rm - (profit * 0.70 / 10.0) as f32).abs() < 0.01);
}

#[test]
fn tariffs_use_import_and_export_rates_separately() {
    let mut world = world_with_law("interventionism");
    world.countries.pops.groups.clear();
    let market = &mut world.countries.market.markets[0];
    market.price.insert("steel".to_owned(), 10.0);
    market.supply.insert("steel".to_owned(), 0.0);
    market.demand.insert("steel".to_owned(), 100.0);
    market.price.insert("coal".to_owned(), 10.0);
    market.supply.insert("coal".to_owned(), 100.0);
    market.demand.insert("coal".to_owned(), 0.0);
    world.countries.treasury.treasuries[0].reserve_gbp = 1_000.0;
    world.countries.treasury.exchange_rates[0].rm_per_gbp = 10.0;
    let db = db_with_trade();

    hoi4_logic::trade::step_trade_matching(&mut world, &db, 0);

    let import_amount = world.countries.market.markets[0]
        .imports
        .get("steel")
        .copied()
        .unwrap_or(0.0) as f64;
    let export_amount = world.countries.market.markets[0]
        .exports
        .get("coal")
        .copied()
        .unwrap_or(0.0) as f64;
    let import_tariff_rm = import_amount * 10.0 * 0.20;
    let export_tariff_gbp = export_amount * 10.0 / 10.0 * 0.10;
    assert!(
        (world.countries.treasury.treasuries[0].cash_rm - (1_000_000.0 + import_tariff_rm)).abs()
            < 0.01
    );
    assert!(
        (world.countries.treasury.treasuries[0].reserve_gbp
            - (1_000.0 - import_amount + import_amount * 0.20 + export_amount - export_tariff_gbp))
            .abs()
            < 0.01
    );
}

#[test]
fn h5_imports_cap_at_available_foreign_exchange() {
    let mut world = world_with_law("interventionism");
    world.countries.pops.groups.clear();
    let market = &mut world.countries.market.markets[0];
    market.price.insert("steel".to_owned(), 10.0);
    market.supply.insert("steel".to_owned(), 0.0);
    market.demand.insert("steel".to_owned(), 100.0);
    world.countries.treasury.treasuries[0].reserve_gbp = 2.0;
    world.countries.treasury.exchange_rates[0].rm_per_gbp = 10.0;
    let db = db_with_trade();

    hoi4_logic::trade::step_trade_matching(&mut world, &db, 0);

    let imports = world.countries.market.markets[0]
        .imports
        .get("steel")
        .copied()
        .unwrap_or(0.0);
    assert!(
        (imports - 2.0).abs() < 0.01,
        "imports should be capped by FX"
    );
    assert!(
        world.countries.treasury.treasuries[0].reserve_gbp >= -0.001,
        "foreign exchange reserve should not go negative"
    );
    assert_eq!(
        world
            .countries
            .trade
            .import_failures
            .get("steel")
            .map(String::as_str),
        Some("partial_due_to_foreign_exchange")
    );
}

#[test]
fn h5_dynamic_trade_preserves_historical_routes() {
    let mut world = world_with_law("interventionism");
    world.countries.trade.add_route_for_good(
        hoi4_state::CountryId(0),
        hoi4_state::CountryId::NONE,
        Some("oil".to_owned()),
        hoi4_state::TradeRouteKind::Sea,
        Some(StateId(0)),
        18.0,
        true,
    );
    let market = &mut world.countries.market.markets[0];
    market.price.insert("steel".to_owned(), 10.0);
    market.supply.insert("steel".to_owned(), 0.0);
    market.demand.insert("steel".to_owned(), 100.0);
    world.countries.treasury.treasuries[0].reserve_gbp = 1_000.0;
    world.countries.treasury.exchange_rates[0].rm_per_gbp = 10.0;
    let db = db_with_trade();

    hoi4_logic::trade::step_trade_matching(&mut world, &db, 0);

    assert!(world.countries.trade.routes.iter().any(|route| {
        route.historical && route.good_id.as_deref() == Some("oil") && route.throughput == 18.0
    }));
    assert!(world.countries.trade.routes.iter().any(|route| {
        !route.historical && route.good_id.as_deref() == Some("steel") && route.throughput > 0.0
    }));
}

#[test]
fn zero_trade_price_keeps_last_price() {
    let mut world = world_with_law("interventionism");
    world.countries.pops.groups.clear();
    world.countries.market.markets[0]
        .price
        .insert("steel".to_owned(), 3.0);
    world.countries.market.markets[0]
        .supply
        .insert("steel".to_owned(), 0.0);
    world.countries.market.markets[0]
        .demand
        .insert("steel".to_owned(), 0.0);
    let db = base_db();
    let mut econ = EconomyState::new(&world);

    tick_daily_v6(&mut world, &mut econ, &db, 7);

    assert_eq!(
        world.countries.market.markets[0]
            .price
            .get("steel")
            .copied()
            .unwrap(),
        3.0
    );
}

#[test]
fn ron_military_equipment_outputs_cover_12_categories() {
    use std::collections::HashSet;

    let db = V6Database::load();
    let categories: HashSet<_> = db
        .production_methods
        .iter()
        .filter_map(|pm| pm.equipment_output.as_ref())
        .map(|eq| hoi4_data::EquipmentCategory::from_str(&eq.equipment_category))
        .filter(|cat| *cat != hoi4_data::EquipmentCategory::Other)
        .collect();

    assert_eq!(
        categories.len(),
        hoi4_data::EquipmentCategory::COMBAT_CATEGORY_COUNT
    );
    assert!(categories.contains(&hoi4_data::EquipmentCategory::Infantry));
    assert!(categories.contains(&hoi4_data::EquipmentCategory::Artillery));
    assert!(categories.contains(&hoi4_data::EquipmentCategory::AntiTank));
    assert!(categories.contains(&hoi4_data::EquipmentCategory::AntiAir));
    assert!(categories.contains(&hoi4_data::EquipmentCategory::SupportEquipment));
    assert!(categories.contains(&hoi4_data::EquipmentCategory::Motorized));
    assert!(categories.contains(&hoi4_data::EquipmentCategory::Mechanized));
    assert!(categories.contains(&hoi4_data::EquipmentCategory::Tank));
    assert!(categories.contains(&hoi4_data::EquipmentCategory::Plane));
    assert!(categories.contains(&hoi4_data::EquipmentCategory::Ship));
    assert!(categories.contains(&hoi4_data::EquipmentCategory::Convoy));
    assert!(categories.contains(&hoi4_data::EquipmentCategory::Train));
}

#[test]
fn single_building_does_not_monopolize_state_workers() {
    let mut world = world_with_law("interventionism");
    world.countries.pops.groups.clear();
    world.countries.pops.groups.push(PopGroup {
        class: PopClass::Worker,
        state: StateId(0),
        size: 100,
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
        literacy: PopClass::Worker.baseline_literacy(),
        skilled_ratio: PopClass::Worker.baseline_skilled_ratio(),
        standard_of_living: 0.5,
        needs_fulfillment: 1.0,
        essential_needs_fulfillment: 1.0,
        normal_needs_fulfillment: 1.0,
        luxury_needs_fulfillment: 1.0,
        radicalism: 0.0,
    });
    for _ in 0..3 {
        world.countries.buildings_v6.buildings.push(Building {
            kind: BuildingKind::Industrial,
            building_def_id: "steel_mill".to_owned(),
            state: StateId(0),
            level: 1,
            active_pm: "steel_mill_default".to_owned(),
            employment: [0; 6],
            owner: BuildingOwner::State,
            requires_law: None,
            built_progress: 1.0,
            ..Building::runtime_defaults()
        });
    }
    let db = base_db();
    let mut econ = EconomyState::new(&world);

    tick_daily_v6(&mut world, &mut econ, &db, 1);

    let first = world.countries.buildings_v6.buildings[0].employment[PopClass::Worker.index()];
    let second = world.countries.buildings_v6.buildings[1].employment[PopClass::Worker.index()];
    let third = world.countries.buildings_v6.buildings[2].employment[PopClass::Worker.index()];
    assert!(first <= 50, "first building monopolized {first} workers");
    assert!(second > 0, "second building got no workers");
    assert!(third > 0, "third building got no workers");
}

#[test]
fn private_payroll_depends_on_profit() {
    let mut world = world_with_law("interventionism");
    world.countries.pops.groups.clear();
    world.countries.pops.groups.push(PopGroup {
        class: PopClass::Worker,
        state: StateId(0),
        size: 10,
        employed_at: Some(BuildingId(0)),
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
        literacy: PopClass::Worker.baseline_literacy(),
        skilled_ratio: PopClass::Worker.baseline_skilled_ratio(),
        standard_of_living: 0.5,
        needs_fulfillment: 1.0,
        essential_needs_fulfillment: 1.0,
        normal_needs_fulfillment: 1.0,
        luxury_needs_fulfillment: 1.0,
        radicalism: 0.0,
    });
    add_building(
        &mut world,
        "arms_industry",
        "arms_industry_default",
        BuildingOwner::Private,
    );
    world.countries.market.markets[0]
        .price
        .insert("small_arms".to_owned(), 2.0);
    world.countries.market.markets[0]
        .price
        .insert("steel".to_owned(), 1.0);
    let db = base_db();
    let mut econ = EconomyState::new(&world);

    tick_daily_v6(&mut world, &mut econ, &db, 1);
    assert!(world.countries.pops.groups[0].wage_rm > 0.0);

    world.countries.market.markets[0]
        .price
        .insert("small_arms".to_owned(), 0.0);
    tick_daily_v6(&mut world, &mut econ, &db, 2);
    assert_eq!(world.countries.pops.groups[0].wage_rm, 0.0);
}

#[test]
fn construction_queue_completes_into_real_building_and_produces() {
    let mut world = world_with_law("interventionism");
    world.countries.pops.groups.clear();
    world.countries.pops.groups.push(PopGroup {
        class: PopClass::Worker,
        state: StateId(0),
        size: 100,
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
        literacy: PopClass::Worker.baseline_literacy(),
        skilled_ratio: PopClass::Worker.baseline_skilled_ratio(),
        standard_of_living: 0.5,
        needs_fulfillment: 1.0,
        essential_needs_fulfillment: 1.0,
        normal_needs_fulfillment: 1.0,
        luxury_needs_fulfillment: 1.0,
        radicalism: 0.0,
    });
    world.countries.market.markets[0]
        .supply
        .insert("coal".to_owned(), 1_000.0);
    world.countries.market.markets[0]
        .stockpile
        .insert("coal".to_owned(), 1_000.0);
    let db = base_db();
    let mut econ = EconomyState::new(&world);
    econ.enqueue_construction(
        hoi4_state::CountryId(0),
        BuildOrder::new("steel_mill", StateId(0)),
        &world,
    );

    for day in 1..=90 {
        tick_daily_v6(&mut world, &mut econ, &db, day);
    }

    assert!(
        econ.construction[0].items.is_empty(),
        "construction item should complete within 90 days"
    );
    assert!(world
        .countries
        .buildings_v6
        .buildings
        .iter()
        .any(|building| {
            building.building_def_id == "steel_mill"
                && building.state == StateId(0)
                && building.level > 0
        }));

    tick_daily_v6(&mut world, &mut econ, &db, 91);

    assert!(
        world.countries.market.markets[0]
            .supply
            .get("steel")
            .copied()
            .unwrap_or(0.0)
            > 0.0
    );
}

#[test]
fn construction_queue_progresses_and_increases_existing_building_level() {
    let mut world = world_with_law("interventionism");
    world.countries.buildings_v6.buildings.push(Building {
        kind: BuildingKind::Industrial,
        building_def_id: "steel_mill".to_owned(),
        state: StateId(0),
        level: 1,
        active_pm: "steel_mill_default".to_owned(),
        employment: [0; 6],
        owner: BuildingOwner::Private,
        requires_law: None,
        built_progress: 1.0,
        max_level: 15,
        ..Building::runtime_defaults()
    });
    let db = base_db();
    let mut econ = EconomyState::new(&world);
    econ.enqueue_construction(
        hoi4_state::CountryId(0),
        BuildOrder::new("steel_mill", StateId(0)).with_level(2),
        &world,
    );

    tick_daily_v6(&mut world, &mut econ, &db, 1);
    assert!(
        econ.construction[0].items[0].progress > 0.0,
        "tick should increase construction progress"
    );

    for day in 2..=90 {
        tick_daily_v6(&mut world, &mut econ, &db, day);
        if econ.construction[0].items.is_empty() {
            break;
        }
    }

    assert!(
        econ.construction[0].items.is_empty(),
        "construction should finish"
    );
    let building = world
        .countries
        .buildings_v6
        .buildings
        .iter()
        .find(|building| building.building_def_id == "steel_mill" && building.state == StateId(0))
        .expect("steel mill should exist");
    assert_eq!(
        building.level, 2,
        "completed construction should increase level"
    );
}

#[test]
fn construction_queue_uses_building_recipe() {
    let mut world = world_with_law("interventionism");
    let db = base_db();
    let mut econ = EconomyState::new(&world);
    econ.enqueue_construction(
        hoi4_state::CountryId(0),
        BuildOrder::new("steel_mill", StateId(0)),
        &world,
    );

    assert_eq!(econ.construction[0].items[0].cost, 0.0);
    tick_daily_v6(&mut world, &mut econ, &db, 1);

    let item = &econ.construction[0].items[0];
    assert_eq!(item.cost, 1_000.0);
    assert_eq!(item.budget_needed_rm, 50_000_000.0);
    assert_eq!(item.material_needs.len(), 1);
    assert_eq!(item.material_needs[0].good_id, "steel");
    assert_eq!(item.material_needs[0].total_needed, 20.0);
}

#[test]
fn construction_queue_advances_multiple_projects_and_reports_capacity() {
    let mut world = world_with_law("interventionism");
    world.countries.pops.groups.clear();
    world.countries.pops.groups.push(PopGroup {
        class: PopClass::Worker,
        state: StateId(0),
        size: 10_000,
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
        literacy: PopClass::Worker.baseline_literacy(),
        skilled_ratio: PopClass::Worker.baseline_skilled_ratio(),
        standard_of_living: 0.5,
        needs_fulfillment: 1.0,
        essential_needs_fulfillment: 1.0,
        normal_needs_fulfillment: 1.0,
        luxury_needs_fulfillment: 1.0,
        radicalism: 0.0,
    });
    world.countries.treasury.treasuries[0].cash_rm = 10_000_000.0;
    let db = base_db();
    let mut econ = EconomyState::new(&world);
    let cid = hoi4_state::CountryId(0);
    econ.enqueue_construction(
        cid,
        BuildOrder::new("steel_mill", StateId(0)).with_level(1),
        &world,
    );
    econ.enqueue_construction(
        cid,
        BuildOrder::new("steel_mill", StateId(0)).with_level(2),
        &world,
    );

    tick_daily_v6(&mut world, &mut econ, &db, 1);

    let queue = &econ.construction[0];
    assert_eq!(queue.items.len(), 2);
    assert!(queue.capacity.total_cp > 0.0);
    assert!(queue.capacity.national_admin_cp > 0.0);
    assert!(queue.capacity.construction_sector_cp >= 0.0);
    assert!(queue.capacity.regional_labor_cp > 0.0);
    assert!(queue.capacity.engineering_equipment_cp > 0.0);
    assert!(queue.capacity.finance_cp > 0.0);
    assert!(queue.capacity.material_cp > 0.0);
    let physical_cp = queue.capacity.national_admin_cp
        + queue.capacity.construction_sector_cp
        + queue.capacity.regional_labor_cp
        + queue.capacity.engineering_equipment_cp;
    let expected_total_cp = physical_cp
        .min(queue.capacity.finance_cp)
        .min(queue.capacity.material_cp);
    assert!(
        (queue.capacity.total_cp - expected_total_cp).abs() < 0.01,
        "construction total CP must be the capped capacity breakdown"
    );
    assert!(queue.capacity.allocated_cp > 0.0);
    assert!(queue.capacity.idle_cp >= 0.0);
    assert!(queue.capacity.blocked_cp >= 0.0);
    assert!(queue.items[0].progress > 0.0);
    assert!(queue.items[1].progress > 0.0);
    for item in &queue.items {
        assert!(item.runtime.allocated_cp > 0.0);
        assert!(item.runtime.effective_cp > 0.0);
        assert!(item.runtime.estimated_days.is_some());
        assert!(!item.runtime.bottleneck.is_empty());
    }
}

#[test]
fn construction_queue_pause_priority_and_weight_affect_runtime() {
    let mut world = world_with_law("interventionism");
    world.countries.pops.groups.clear();
    world.countries.pops.groups.push(PopGroup {
        class: PopClass::Worker,
        state: StateId(0),
        size: 10_000,
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
        literacy: PopClass::Worker.baseline_literacy(),
        skilled_ratio: PopClass::Worker.baseline_skilled_ratio(),
        standard_of_living: 0.5,
        needs_fulfillment: 1.0,
        essential_needs_fulfillment: 1.0,
        normal_needs_fulfillment: 1.0,
        luxury_needs_fulfillment: 1.0,
        radicalism: 0.0,
    });
    world.countries.treasury.treasuries[0].cash_rm = 10_000_000.0;
    let db = base_db();
    let mut econ = EconomyState::new(&world);
    let cid = hoi4_state::CountryId(0);
    econ.enqueue_construction(
        cid,
        BuildOrder::new("steel_mill", StateId(0))
            .with_level(1)
            .paused(true),
        &world,
    );
    econ.enqueue_construction(
        cid,
        BuildOrder::new("steel_mill", StateId(0))
            .with_level(2)
            .with_priority(4)
            .with_weight(2.0),
        &world,
    );

    tick_daily_v6(&mut world, &mut econ, &db, 1);

    let queue = &econ.construction[0];
    assert_eq!(queue.items[0].progress, 0.0);
    assert_eq!(queue.items[0].runtime.bottleneck, "paused");
    assert!(queue.items[1].progress > 0.0);
    assert!(queue.items[1].runtime.allocated_cp > 0.0);
    assert_eq!(queue.items[1].runtime.priority, 4);
    assert!((queue.items[1].runtime.weight - 2.0).abs() < f32::EPSILON);
}

#[test]
fn construction_queue_reorder_and_cancel_mutate_real_items() {
    let world = world_with_law("interventionism");
    let mut econ = EconomyState::new(&world);
    let cid = hoi4_state::CountryId(0);
    econ.enqueue_construction(
        cid,
        BuildOrder::new("steel_mill", StateId(0)).with_level(1),
        &world,
    );
    econ.enqueue_construction(
        cid,
        BuildOrder::new("arms_industry", StateId(0)).with_level(1),
        &world,
    );
    econ.enqueue_construction(
        cid,
        BuildOrder::new("steel_mill", StateId(0)).with_level(2),
        &world,
    );

    econ.construction[0].items.swap(1, 0);
    assert_eq!(econ.construction[0].items[0].building_key, "arms_industry");

    econ.construction[0].items.swap(0, 1);
    assert_eq!(econ.construction[0].items[1].building_key, "arms_industry");

    econ.construction[0].items.remove(1);
    assert_eq!(econ.construction[0].items.len(), 2);
    assert!(econ.construction[0]
        .items
        .iter()
        .all(|item| item.building_key == "steel_mill"));
}

#[test]
fn resource_building_requires_deposit() {
    let mut world = world_with_law("interventionism");
    let mut db = base_db();
    add_resource_building_def(&mut db, "coal_mine", "coal");
    let mut econ = EconomyState::new(&world);
    let cid = hoi4_state::CountryId(0);
    let order = BuildOrder::new("coal_mine", StateId(0)).with_level(1);

    let err = econ
        .enqueue_construction_checked(cid, order.clone(), &world, &db)
        .expect_err("coal mine without deposit should be rejected");
    assert_eq!(
        err,
        hoi4_logic::economy::construction_tick::BuildBlockReason::NoResourceDeposit
    );

    econ.enqueue_construction(cid, order, &world);
    tick_daily_v6(&mut world, &mut econ, &db, 1);
    assert_eq!(
        econ.construction[0].items[0].progress, 0.0,
        "queued illegal resource construction must not progress"
    );
}

#[test]
fn resource_building_caps_at_potential_level() {
    let world = {
        let mut world = world_with_law("interventionism");
        world.countries.buildings_v6.buildings.push(Building {
            kind: BuildingKind::Resource,
            building_def_id: "coal_mine".to_owned(),
            state: StateId(0),
            level: 1,
            active_pm: "coal_mine_default".to_owned(),
            employment: [0; 6],
            owner: BuildingOwner::Private,
            requires_law: None,
            built_progress: 1.0,
            max_level: 10,
            ..Building::runtime_defaults()
        });
        world
    };
    let mut db = base_db();
    add_resource_building_def(&mut db, "coal_mine", "coal");
    db.state_resource_deposits.push(StateResourceDepositDef {
        state_id: 1,
        deposits: vec![ResourceDepositDef {
            good_id: "coal".to_owned(),
            discovered_level: 1,
            potential_level: 1,
            extraction_difficulty: 1.0,
            requires_tech: None,
        }],
    });
    let mut econ = EconomyState::new(&world);

    let err = econ
        .enqueue_construction_checked(
            hoi4_state::CountryId(0),
            BuildOrder::new("coal_mine", StateId(0)).with_level(2),
            &world,
            &db,
        )
        .expect_err("coal mine at potential cap should be rejected");
    assert_eq!(
        err,
        hoi4_logic::economy::construction_tick::BuildBlockReason::DepositExhausted
    );
}

#[test]
fn f06_auto_mefo_covers_deficit_without_public_debt() {
    let mut world = world_with_law("corporatist_war_economy");
    world.countries.law_store.law_sets[0].0[LawCategory::Conscription.index()] =
        LawSlot::new(LawCategory::Conscription, "limited_conscription");
    let treasury = &mut world.countries.treasury.treasuries[0];
    treasury.daily_income_rm = 100.0;
    treasury.operating_income_rm = 100.0;
    treasury.operating_expense_rm = 350.0;
    treasury.daily_expense_rm = 350.0;
    treasury.public_debt_rm = 1_000.0;

    let db = base_db();
    let printed = hoi4_logic::economy::finance_tick::step_auto_mefo(&mut world, &db, 0);
    let treasury = &world.countries.treasury.treasuries[0];

    assert_eq!(printed, 250.0);
    assert_eq!(treasury.mefo_debt_rm, 250.0);
    assert_eq!(treasury.public_debt_rm, 1_000.0);
}

#[test]
fn f06_corporatist_procurement_creates_military_demand_jobs_and_wages() {
    let mut world = world_with_law("corporatist_war_economy");
    world.countries.law_store.law_sets[0].0[LawCategory::Conscription.index()] =
        LawSlot::new(LawCategory::Conscription, "limited_conscription");
    world.countries.pops.groups.clear();
    world.countries.pops.groups.push(PopGroup {
        class: PopClass::Worker,
        state: StateId(0),
        size: 100,
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
        literacy: PopClass::Worker.baseline_literacy(),
        skilled_ratio: PopClass::Worker.baseline_skilled_ratio(),
        standard_of_living: 0.5,
        needs_fulfillment: 1.0,
        essential_needs_fulfillment: 1.0,
        normal_needs_fulfillment: 1.0,
        luxury_needs_fulfillment: 1.0,
        radicalism: 0.0,
    });
    world.countries.buildings_v6.buildings.push(Building {
        kind: BuildingKind::Military,
        building_def_id: "arms_industry".to_owned(),
        state: StateId(0),
        level: 1,
        active_pm: "arms_industry_default".to_owned(),
        employment: [0; 6],
        owner: BuildingOwner::Private,
        requires_law: None,
        built_progress: 1.0,
        ..Building::runtime_defaults()
    });
    let market = &mut world.countries.market.markets[0];
    market.supply.insert("steel".to_owned(), 1_000.0);
    market.stockpile.insert("steel".to_owned(), 1_000.0);
    market.price.insert("steel".to_owned(), 1.0);
    market.price.insert("small_arms".to_owned(), 2.0);
    world.countries.treasury.treasuries[0].gdp_rm = 1_000_000.0;
    let db = base_db();
    let mut econ = EconomyState::new(&world);

    tick_daily_v6(&mut world, &mut econ, &db, 7);
    tick_daily_v6(&mut world, &mut econ, &db, 8);

    let demand = world.countries.market.markets[0]
        .demand
        .get("small_arms")
        .copied()
        .unwrap_or(0.0);
    let infantry_equipment = econ.stockpile[0]
        .get("infantry_equipment")
        .copied()
        .unwrap_or(0.0);
    let employed = world.countries.buildings_v6.buildings[0].employment[PopClass::Worker.index()];
    assert!(
        demand > 0.0,
        "corporatist government procurement should create military demand"
    );
    assert!(
        infantry_equipment > 0.0,
        "employed arms industry should produce infantry equipment"
    );
    assert!(employed > 0, "arms industry should hire workers");
}

#[test]
fn building_runtime_fields_are_written_by_tick() {
    let mut world = world_with_law("interventionism");
    world.countries.pops.groups.clear();
    world.countries.pops.groups.push(PopGroup {
        class: PopClass::Worker,
        state: StateId(0),
        size: 100,
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
        literacy: PopClass::Worker.baseline_literacy(),
        skilled_ratio: PopClass::Worker.baseline_skilled_ratio(),
        standard_of_living: 0.5,
        needs_fulfillment: 1.0,
        essential_needs_fulfillment: 1.0,
        normal_needs_fulfillment: 1.0,
        luxury_needs_fulfillment: 1.0,
        radicalism: 0.0,
    });
    add_building(
        &mut world,
        "steel_mill",
        "steel_mill_default",
        BuildingOwner::State,
    );
    world.countries.buildings_v6.buildings.push(Building {
        kind: BuildingKind::Infrastructure,
        building_def_id: "construction_sector".to_owned(),
        state: StateId(0),
        level: 1,
        active_pm: "construction_sector_default".to_owned(),
        employment: [0; 6],
        owner: BuildingOwner::State,
        requires_law: None,
        built_progress: 1.0,
        ..Building::runtime_defaults()
    });
    world.countries.market.markets[0]
        .price
        .insert("steel".to_owned(), 20.0);
    world.countries.market.markets[0]
        .price
        .insert("coal".to_owned(), 1.0);
    world.countries.market.markets[0]
        .supply
        .insert("coal".to_owned(), 1_000.0);
    world.countries.market.markets[0]
        .stockpile
        .insert("coal".to_owned(), 1_000.0);
    let db = base_db();
    let mut econ = EconomyState::new(&world);

    tick_daily_v6(&mut world, &mut econ, &db, 1);

    let building = &world.countries.buildings_v6.buildings[0];
    assert!(building.production_rate > 0.0);
    assert!(building.output_value_gbp > 0.0);
    assert!(building.wage_rm > 0.0);
    assert!(building.profit_rm > 0.0);
    assert!(building.cp_cost > 0.0);
    assert_eq!(building.max_level, 15);
    assert_eq!(building.built_progress, 1.0);
}

#[test]
fn f06_mefo_crisis_rolls_hidden_debt_into_public_debt_and_hits_pops() {
    let mut world = world_with_law("corporatist_war_economy");
    let treasury = &mut world.countries.treasury.treasuries[0];
    treasury.cash_rm = 1_000.0;
    treasury.gdp_rm = 1_000.0;
    treasury.mefo_debt_rm = 400.0;
    treasury.public_debt_rm = 100.0;
    world.countries.treasury.exchange_rates[0].rm_per_gbp = 10.0;
    let satisfaction_before = world.countries.pops.groups[0].satisfaction;

    let mut db = base_db();
    db.events_v6.push(V6EventDef {
        id: "mefo_crisis".to_owned(),
        title: "MEFO Crisis".to_owned(),
        body: "MEFO crisis".to_owned(),
        options: vec![V6EventOption {
            name: "Default".to_owned(),
            effects: vec![
                ("mefo_forced_payment".to_owned(), "".to_owned()),
                ("pop_satisfaction_penalty".to_owned(), "-0.20".to_owned()),
            ],
        }],
    });
    let triggered =
        hoi4_logic::economy::finance_tick::step_check_mefo_crisis(&mut world, &db, 0, 1095);
    let treasury = &world.countries.treasury.treasuries[0];

    assert!(triggered);
    assert_eq!(treasury.mefo_debt_rm, 0.0);
    assert_eq!(treasury.public_debt_rm, 340.0);
    assert_eq!(treasury.cash_rm, 840.0);
    assert!(world.countries.pops.groups[0].satisfaction < satisfaction_before);
}

#[test]
fn f06_military_spending_breaks_into_wages_procurement_and_maintenance() {
    let mut world = world_with_law("interventionism");
    let mut econ = EconomyState::new(&world);
    world.divisions.push(
        hoi4_state::CountryId(0),
        ProvinceId(0),
        0,
        30.0,
        100.0,
        "测试步兵师".to_owned(),
    );
    world.divisions.strength[0] = 0.5;

    hoi4_logic::economy::finance_tick::step_pay_military_upkeep(&mut world, &mut econ, 0);

    let treasury = &world.countries.treasury.treasuries[0];
    assert!(treasury.daily_budget.expense_military_wages_rm > 0.0);
    assert_eq!(treasury.daily_budget.expense_military_procurement_rm, 0.0);
    assert!(treasury.daily_budget.expense_military_maintenance_rm > 0.0);
    assert_ne!(
        treasury.daily_expense_rm, 500_000.0,
        "military spending must not be division_count * 500_000"
    );
}

#[test]
fn f06_finance_budget_breakdown_sums_to_daily_totals() {
    let mut world = world_with_law("interventionism");
    let db = base_db();
    let mut econ = EconomyState::new(&world);
    econ.enqueue_construction(
        hoi4_state::CountryId(0),
        BuildOrder::new("steel_mill", StateId(0)).with_level(1),
        &world,
    );
    add_building(
        &mut world,
        "steel_mill",
        "steel_mill_default",
        BuildingOwner::State,
    );
    world.countries.buildings_v6.buildings.push(Building {
        kind: BuildingKind::Infrastructure,
        building_def_id: "construction_sector".to_owned(),
        state: StateId(0),
        level: 1,
        active_pm: "construction_sector_default".to_owned(),
        employment: [0; 6],
        owner: BuildingOwner::State,
        requires_law: None,
        built_progress: 1.0,
        ..Building::runtime_defaults()
    });
    world.countries.market.markets[0]
        .price
        .insert("machinery".to_owned(), 1.0);
    world.countries.market.markets[0]
        .supply
        .insert("machinery".to_owned(), 1_000.0);

    hoi4_logic::economy::finance_tick::step_construction_cost(&mut world, &db, 0);

    let treasury = &world.countries.treasury.treasuries[0];
    assert!(treasury.daily_budget.expense_construction_wages_rm > 0.0);
    let machinery_demand = world.countries.market.markets[0].bucket_demand.get(
        "machinery",
        hoi4_state::market::DemandBucketKind::ConstructionInput,
    );
    assert!(
        machinery_demand > 0.0,
        "construction should demand machinery via bucket_demand"
    );
    assert!(
        (treasury.daily_budget.total_income_rm() - treasury.daily_income_rm).abs() < 0.01,
        "income breakdown must sum to total"
    );
    assert!(
        (treasury.daily_budget.total_expense_rm() - treasury.daily_expense_rm).abs() < 0.01,
        "expense breakdown must sum to total"
    );
}

#[test]
fn f06_auto_mefo_records_daily_coverage_breakdown() {
    let mut world = world_with_law("corporatist_war_economy");
    world.countries.law_store.law_sets[0].0[LawCategory::Conscription.index()] =
        LawSlot::new(LawCategory::Conscription, "limited_conscription");
    let treasury = &mut world.countries.treasury.treasuries[0];
    treasury.daily_income_rm = 100.0;
    treasury.operating_income_rm = 100.0;
    treasury.operating_expense_rm = 350.0;
    treasury.daily_expense_rm = 350.0;

    let db = base_db();
    let printed = hoi4_logic::economy::finance_tick::step_auto_mefo(&mut world, &db, 0);

    let treasury = &world.countries.treasury.treasuries[0];
    assert_eq!(printed, 250.0);
    assert_eq!(treasury.mefo_coverage_rm, 250.0);
    assert_eq!(treasury.daily_financing.mefo_issued_rm, 250.0);
    assert_eq!(treasury.daily_income_rm, 350.0);
}

#[test]
fn f06_mefo_crisis_disables_future_issuance() {
    let mut world = world_with_law("corporatist_war_economy");
    world.countries.law_store.law_sets[0].0[LawCategory::Conscription.index()] =
        LawSlot::new(LawCategory::Conscription, "limited_conscription");
    let treasury = &mut world.countries.treasury.treasuries[0];
    treasury.gdp_rm = 1_000.0;
    treasury.mefo_debt_rm = 400.0;

    let mut db = base_db();
    db.events_v6.push(V6EventDef {
        id: "mefo_crisis".to_owned(),
        title: "MEFO Crisis".to_owned(),
        body: "MEFO crisis".to_owned(),
        options: vec![V6EventOption {
            name: "Default".to_owned(),
            effects: vec![("mefo_forced_payment".to_owned(), "".to_owned())],
        }],
    });

    assert!(hoi4_logic::economy::finance_tick::step_check_mefo_crisis(
        &mut world, &db, 0, 1095
    ));
    let treasury = &mut world.countries.treasury.treasuries[0];
    treasury.daily_income_rm = 100.0;
    treasury.daily_expense_rm = 350.0;

    let printed = hoi4_logic::economy::finance_tick::step_auto_mefo(&mut world, &db, 0);

    assert_eq!(printed, 0.0);
    assert!(world.countries.treasury.treasuries[0].mefo_disabled);
}

#[test]
fn f06_mefo_is_germany_only() {
    let mut world = world_with_law("corporatist_war_economy");
    world.countries.tags[0] = "USA".to_owned();
    world.countries.law_store.law_sets[0].0[LawCategory::Conscription.index()] =
        LawSlot::new(LawCategory::Conscription, "limited_conscription");
    let treasury = &mut world.countries.treasury.treasuries[0];
    treasury.daily_income_rm = 100.0;
    treasury.daily_expense_rm = 350.0;
    let db = base_db();

    let printed = hoi4_logic::economy::finance_tick::step_auto_mefo(&mut world, &db, 0);

    assert_eq!(printed, 0.0);
    assert_eq!(world.countries.treasury.treasuries[0].mefo_debt_rm, 0.0);
}

#[test]
fn p14_building_runtime_fields_match_valuation_helper() {
    let mut world = world_with_law("interventionism");
    add_building(
        &mut world,
        "steel_mill",
        "steel_mill_default",
        BuildingOwner::Private,
    );
    world.countries.market.markets[0]
        .price
        .insert("steel".to_owned(), 2.0);
    world.countries.market.markets[0]
        .price
        .insert("coal".to_owned(), 1.0);
    let db = base_db();
    let mut econ = EconomyState::new(&world);

    tick_daily_v6(&mut world, &mut econ, &db, 1);

    let building = &world.countries.buildings_v6.buildings[0];
    let val = hoi4_logic::economy::valuation::compute_building_valuation(building, &world, &db, 0);

    assert!(
        (building.profit_rm - val.actual_profit_rm.max(0.0)).abs() < 0.01,
        "building.profit_rm ({}) 应与 valuation actual_profit_rm ({}) 一致",
        building.profit_rm,
        val.actual_profit_rm,
    );
    assert!(
        (building.input_cost_rm - val.input_cost_rm).abs() < 0.01,
        "building.input_cost_rm ({}) 应与 valuation input_cost_rm ({}) 一致",
        building.input_cost_rm,
        val.input_cost_rm,
    );
    assert!(
        (building.estimated_profit_rm - val.estimated_profit_rm).abs() < 0.01,
        "building.estimated_profit_rm ({}) 应与 valuation estimated_profit_rm ({}) 一致",
        building.estimated_profit_rm,
        val.estimated_profit_rm,
    );
    assert!(
        (building.value_added_rm - val.value_added_rm).abs() < 0.01,
        "building.value_added_rm ({}) 应与 valuation value_added_rm ({}) 一致",
        building.value_added_rm,
        val.value_added_rm,
    );
    assert!(
        (building.wage_rm - val.wage_rm).abs() < 0.01,
        "building.wage_rm ({}) 应与 valuation wage_rm ({}) 一致",
        building.wage_rm,
        val.wage_rm,
    );
}

#[test]
fn p14_gdp_uses_building_value_added_not_separate_estimate() {
    let mut world = world_with_law("interventionism");
    add_building(
        &mut world,
        "steel_mill",
        "steel_mill_default",
        BuildingOwner::State,
    );
    world.countries.market.markets[0]
        .price
        .insert("steel".to_owned(), 2.0);
    world.countries.market.markets[0]
        .price
        .insert("coal".to_owned(), 1.0);
    world.countries.treasury.treasuries[0].cash_rm = 1_000_000_000.0;
    let db = base_db();
    hoi4_logic::economy::building_runtime::update(&mut world, &db, 0);
    hoi4_logic::economy::finance_tick::step_update_gdp(&mut world, &db, 0, 7);

    let building = &world.countries.buildings_v6.buildings[0];
    let treasury = &world.countries.treasury.treasuries[0];
    assert!(treasury.gdp_rm > 0.0, "GDP 应在 8 天后（含一次周更）大于 0",);
    assert!(
        building.value_added_rm > 0.0 || building.level == 0,
        "建筑 value_added_rm 应 > 0（有生产时）",
    );
}

#[test]
fn g13_gdp_replaces_profile_anchor_with_runtime_components() {
    let mut world = world_with_law("interventionism");
    add_building(
        &mut world,
        "steel_mill",
        "steel_mill_default",
        BuildingOwner::State,
    );
    let db = base_db();
    let rm_per_gbp = world.countries.treasury.exchange_rates[0].rm_per_gbp as f64;
    {
        let building = &mut world.countries.buildings_v6.buildings[0];
        building.value_added_rm = 100.0;
    }
    {
        let pop = &mut world.countries.pops.groups[0];
        pop.income_rm = 5.0;
        pop.basic_consumption_budget = 3.0;
    }
    {
        let treasury = &mut world.countries.treasury.treasuries[0];
        treasury.gdp_rm = 9_999_999.0;
        treasury.gdp_gbp = 9_999.0;
        treasury.gdp_breakdown.historical_validation_gbp = 1_000.0;
        treasury.daily_budget.expense_state_payroll_rm = 7.0;
        treasury.daily_budget.expense_construction_wages_rm = 2.0;
        treasury.daily_budget.expense_welfare_rm = 1.0;
        treasury.daily_budget.expense_military_procurement_rm = 4.0;
        treasury.daily_trade_balance_gbp = 2.0;
    }

    hoi4_logic::economy::finance_tick::step_update_gdp(&mut world, &db, 0, 7);

    let treasury = &world.countries.treasury.treasuries[0];
    let expected_rm = (100.0 + (3.0 * 100.0) + 10.0 + 4.0 + (2.0 * rm_per_gbp)) * 365.0;
    assert!(
        (treasury.gdp_rm - expected_rm).abs() < 0.01,
        "GDP should be written from runtime components, got {} expected {}",
        treasury.gdp_rm,
        expected_rm
    );
    assert_eq!(treasury.gdp_breakdown.building_secondary_rm, 36_500.0);
    assert_eq!(treasury.gdp_breakdown.pop_income_rm, 182_500.0);
    assert_eq!(treasury.gdp_breakdown.pop_consumption_rm, 109_500.0);
    assert_eq!(treasury.gdp_breakdown.government_services_rm, 3_650.0);
    assert_eq!(treasury.gdp_breakdown.military_procurement_rm, 1_460.0);
    assert!((treasury.gdp_gbp - treasury.gdp_rm / rm_per_gbp).abs() < 0.01);
    assert!(treasury
        .gdp_breakdown
        .historical_validation_error_ratio
        .is_finite());
}

#[test]
fn p14_loss_building_not_masked_by_estimated_profit() {
    let mut world = world_with_law("interventionism");
    add_building(
        &mut world,
        "steel_mill",
        "steel_mill_default",
        BuildingOwner::Private,
    );
    world.countries.market.markets[0]
        .price
        .insert("steel".to_owned(), 0.01);
    world.countries.market.markets[0]
        .price
        .insert("coal".to_owned(), 100.0);
    let db = base_db();
    let mut econ = EconomyState::new(&world);

    tick_daily_v6(&mut world, &mut econ, &db, 1);

    let building = &world.countries.buildings_v6.buildings[0];
    assert!(
        building.profit_rm <= building.estimated_profit_rm,
        "实际利润 ({}) 不应超过估算利润 ({})",
        building.profit_rm,
        building.estimated_profit_rm,
    );
    assert!(building.estimated_profit_rm >= 0.0, "估算利润应 >= 0",);
}

#[test]
fn p14_single_goods_rm_scale_authority() {
    let scale = hoi4_logic::economy::valuation::GOODS_RM_SCALE;
    let gdp_scale = hoi4_logic::economy::valuation::GDP_OUTPUT_RM_SCALE;
    assert!((scale - 1_200.0).abs() < 0.01, "GOODS_RM_SCALE 应为 1200",);
    assert!(
        (gdp_scale - 1_200.0).abs() < 0.01,
        "GDP_OUTPUT_RM_SCALE 应与 GOODS_RM_SCALE 一致 (= 1200)",
    );
    assert!(
        (scale - gdp_scale).abs() < 0.01,
        "GOODS_RM_SCALE 与 GDP_OUTPUT_RM_SCALE 必须相等",
    );
}

#[test]
fn p15_private_building_produces_under_planned_economy() {
    let mut world = world_with_law("planned_economy");
    add_building(
        &mut world,
        "steel_mill",
        "steel_mill_default",
        BuildingOwner::Private,
    );
    world.countries.market.markets[0]
        .price
        .insert("steel".to_owned(), 2.0);
    world.countries.market.markets[0]
        .price
        .insert("coal".to_owned(), 1.0);
    world.countries.market.markets[0]
        .supply
        .insert("coal".to_owned(), 100.0);
    world.countries.market.markets[0]
        .stockpile
        .insert("coal".to_owned(), 100.0);
    let db = base_db();
    let mut econ = EconomyState::new(&world);

    tick_daily_v6(&mut world, &mut econ, &db, 1);

    let building = &world.countries.buildings_v6.buildings[0];
    assert!(
        building.output_value_gbp > 0.0 || building.production_rate <= 0.0,
        "计划经济下私有建筑应有产出（production_rate={}, output_gbp={}）",
        building.production_rate,
        building.output_value_gbp,
    );
    let steel_supply = world.countries.market.markets[0]
        .supply
        .get("steel")
        .copied()
        .unwrap_or(0.0);
    assert!(
        steel_supply > 0.0,
        "计划经济下私有建筑应在市场产出钢（steel supply={}）",
        steel_supply,
    );
}

#[test]
fn p15_rationing_no_double_penalty_on_satisfaction() {
    let mut world = world_with_law("planned_economy");
    add_building(
        &mut world,
        "steel_mill",
        "steel_mill_default",
        BuildingOwner::State,
    );
    world.countries.market.markets[0]
        .price
        .insert("steel".to_owned(), 2.0);
    world.countries.market.markets[0]
        .price
        .insert("coal".to_owned(), 1.0);
    world.countries.market.markets[0]
        .supply
        .insert("coal".to_owned(), 100.0);
    world.countries.market.markets[0]
        .stockpile
        .insert("coal".to_owned(), 100.0);
    world.countries.market.markets[0]
        .supply
        .insert("steel".to_owned(), 50.0);
    world.countries.market.markets[0]
        .stockpile
        .insert("steel".to_owned(), 50.0);
    let db = base_db();
    let mut econ = EconomyState::new(&world);

    tick_daily_v6(&mut world, &mut econ, &db, 1);

    let pg = world
        .countries
        .pops
        .groups
        .iter()
        .find(|pg| pg.class == PopClass::Worker)
        .unwrap();
    assert!(
        pg.essential_needs_fulfillment > 0.0,
        "配给下基础满足度应 > 0（实际={:.3}），不应被二次惩罚至 0",
        pg.essential_needs_fulfillment,
    );
    assert!(
        pg.satisfaction > 0.0,
        "配给下满意度应 > 0（实际={:.3}）",
        pg.satisfaction,
    );
}
