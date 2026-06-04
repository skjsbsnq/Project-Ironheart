use std::collections::HashMap;

use hoi4_content::v6_loader::{
    BuildingDef, BuildingEmploymentProfileDef, BuildingGameplayClassDef, BuildingKindDef,
    ConstructionMaterialDef, ConstructionRecipeDef, EconomicSectorDef, EquipmentOutputDef,
    GoodCategoryDef, GoodDef, OwnerDef, ProductionMethodDef,
};
use hoi4_content::V6Database;
use hoi4_logic::economy::production_chain::{
    ProductionChainConsumerKind, ProductionChainGraph, ProductionChainOutputKind,
};
use hoi4_state::market::{BucketDemand, DemandBucketKind};
use hoi4_state::market::{ClearingBucket, GoodClearingResult, MarketClearingSheet, NationalMarket};

fn good(id: &str, category: GoodCategoryDef) -> GoodDef {
    GoodDef {
        id: id.to_owned(),
        name: id.to_owned(),
        category,
        base_price_rm: 1.0,
        unlocked_by: None,
    }
}

fn building(id: &str, name: &str, kind: BuildingKindDef, buildable: bool) -> BuildingDef {
    BuildingDef {
        id: id.to_owned(),
        name: name.to_owned(),
        description: format!("{name} description"),
        economic_sector: EconomicSectorDef::Secondary,
        gameplay_class: BuildingGameplayClassDef::Industrial,
        kind,
        max_level: 10,
        owner_default: OwnerDef::Private,
        buildable,
        group: "test".to_owned(),
        state_limit_kind: None,
        requires_law: None,
        employment_profile: BuildingEmploymentProfileDef {
            peasants: 0,
            workers: 100,
            clerks: 0,
            capitalists: 0,
            aristocrats: 0,
            soldiers: 0,
        },
        construction_recipe: ConstructionRecipeDef {
            cp_cost: 100.0,
            funds_rm: 1_000.0,
            materials: vec![ConstructionMaterialDef {
                good_id: "steel".to_owned(),
                amount: 5.0,
            }],
            labor: 10,
            engineering: 5,
            regional_restrictions: Vec::new(),
        },
    }
}

fn pm(
    id: &str,
    building_id: &str,
    inputs: &[(&str, f32)],
    outputs: &[(&str, f32)],
) -> ProductionMethodDef {
    ProductionMethodDef {
        id: id.to_owned(),
        name: id.to_owned(),
        building_id: building_id.to_owned(),
        group: "base".to_owned(),
        group_name: "base".to_owned(),
        input_good_ids: inputs.iter().map(|(good, _)| (*good).to_owned()).collect(),
        input_good_amounts: inputs.iter().map(|(_, amount)| *amount).collect(),
        output_good_ids: outputs.iter().map(|(good, _)| (*good).to_owned()).collect(),
        output_good_amounts: outputs.iter().map(|(_, amount)| *amount).collect(),
        employment_demand: [0, 100, 0, 0, 0, 0],
        unlocked_by: None,
        required_law: None,
        throughput_modifier: 1.0,
        automation_modifier: 1.0,
        required_literacy: 0.0,
        required_skilled_ratio: 0.0,
        equipment_output: None,
    }
}

fn test_db() -> V6Database {
    let mut arms_pm = pm("arms_default", "arms_industry", &[("steel", 3.0)], &[]);
    arms_pm.equipment_output = Some(EquipmentOutputDef {
        equipment_category: "infantry_equipment".to_owned(),
        daily_per_level: 2.0,
    });

    V6Database {
        goods: vec![
            good("coal", GoodCategoryDef::RawMaterial),
            good("steel", GoodCategoryDef::Intermediate),
            good("infantry_equipment", GoodCategoryDef::MilitaryIntermediate),
        ],
        buildings: vec![
            building("coal_mine", "Coal Mine", BuildingKindDef::Resource, true),
            building(
                "steel_mill",
                "Steel Mill",
                BuildingKindDef::Industrial,
                true,
            ),
            building(
                "arms_industry",
                "Arms Industry",
                BuildingKindDef::Military,
                true,
            ),
        ],
        production_methods: vec![
            pm("coal_default", "coal_mine", &[], &[("coal", 8.0)]),
            pm(
                "steel_default",
                "steel_mill",
                &[("coal", 4.0)],
                &[("steel", 6.0)],
            ),
            arms_pm,
        ],
        ..V6Database::default()
    }
}

fn market_with_coal_shortage() -> NationalMarket {
    let mut market = NationalMarket {
        supply: HashMap::from([("coal".to_owned(), 2.0)]),
        demand: HashMap::from([("coal".to_owned(), 10.0)]),
        unmet_demand: HashMap::from([("coal".to_owned(), 8.0)]),
        ..NationalMarket::default()
    };
    market.bucket_demand = BucketDemand {
        entries: HashMap::from([(("coal".to_owned(), DemandBucketKind::BuildingInput), 10.0)]),
    };
    market.clearing_sheet = MarketClearingSheet {
        results: HashMap::from([(
            "coal".to_owned(),
            GoodClearingResult {
                buckets: vec![ClearingBucket {
                    kind: DemandBucketKind::BuildingInput,
                    requested: 10.0,
                    fulfilled: 2.0,
                    unmet: 8.0,
                }],
                total_fulfilled: 2.0,
                total_unmet: 8.0,
                shortage_ratio: 0.8,
                ..GoodClearingResult::default()
            },
        )]),
    };
    market
}

#[test]
fn production_chain_graph_exists() {
    let graph = ProductionChainGraph::from_database(&test_db());

    assert!(graph.contains_good("steel"));
    assert!(graph.contains_good("infantry_equipment"));
}

#[test]
fn good_to_producers_query() {
    let graph = ProductionChainGraph::from_database(&test_db());

    let steel_producers = graph.producers_for_good("steel");
    assert!(steel_producers
        .iter()
        .any(|producer| producer.building_id == "steel_mill"
            && producer.production_method_id == "steel_default"
            && producer.amount_per_level == 6.0));

    let equipment_producers = graph.producers_for_good("infantry_equipment");
    assert!(equipment_producers
        .iter()
        .any(|producer| producer.building_id == "arms_industry"
            && producer.output_kind == ProductionChainOutputKind::EquipmentCategory));
}

#[test]
fn good_to_consumers_query() {
    let graph = ProductionChainGraph::from_database(&test_db());

    assert!(graph
        .consumers_for_good("coal")
        .iter()
        .any(|consumer| matches!(
            &consumer.kind,
            ProductionChainConsumerKind::ProductionMethodInput {
                building_id,
                production_method_id
            } if building_id == "steel_mill" && production_method_id == "steel_default"
        )));
    assert_eq!(graph.upstream_goods("steel"), vec!["coal".to_owned()]);
    assert_eq!(graph.downstream_goods("coal"), vec!["steel".to_owned()]);
}

#[test]
fn shortage_to_affected_buildings_query() {
    let db = test_db();
    let market = market_with_coal_shortage();
    let graph = ProductionChainGraph::from_database_and_market(&db, Some(&market));

    let impact = graph.shortage_impact("coal");
    assert_eq!(impact.shortage_amount, 8.0);
    assert_eq!(impact.shortage_ratio, 0.8);
    assert!(impact
        .affected_buildings
        .iter()
        .any(|building| building.building_id == "steel_mill"
            && building.production_method_ids == vec!["steel_default".to_owned()]));
    assert!(
        impact
            .affected_demand_buckets
            .iter()
            .any(|bucket| bucket.bucket == DemandBucketKind::BuildingInput
                && bucket.requested >= 10.0)
    );
}

#[test]
fn shortage_to_actions_query() {
    let graph = ProductionChainGraph::from_database(&test_db());

    let coal_actions = graph.buildable_actions_for_shortage("coal");
    assert!(coal_actions
        .iter()
        .any(|action| action.building_id == "coal_mine" && action.amount_per_level == 8.0));

    let equipment_actions = graph.buildable_actions_for_shortage("infantry_equipment");
    assert!(equipment_actions.iter().any(|action| {
        action.building_id == "arms_industry"
            && action.output_kind == ProductionChainOutputKind::EquipmentCategory
    }));
}
