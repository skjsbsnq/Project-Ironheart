use std::collections::{BTreeMap, BTreeSet};

use hoi4_content::{
    v6_loader::{BuildingDef, ProductionMethodDef},
    V6Database,
};
use hoi4_state::market::{DemandBucketKind, NationalMarket};

#[derive(Debug, Clone, PartialEq)]
pub struct ProductionChainGraph {
    goods: BTreeSet<String>,
    producers_by_good: BTreeMap<String, Vec<ProductionChainProducer>>,
    consumers_by_good: BTreeMap<String, Vec<ProductionChainConsumer>>,
    shortage_by_good: BTreeMap<String, ProductionShortage>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProductionChainProducer {
    pub good_id: String,
    pub building_id: String,
    pub building_name: String,
    pub production_method_id: String,
    pub production_method_name: String,
    pub amount_per_level: f32,
    pub output_kind: ProductionChainOutputKind,
    pub buildable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProductionChainOutputKind {
    Good,
    EquipmentCategory,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProductionChainConsumer {
    pub good_id: String,
    pub consumer_id: String,
    pub consumer_name: String,
    pub kind: ProductionChainConsumerKind,
    pub amount: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ProductionChainConsumerKind {
    ProductionMethodInput {
        building_id: String,
        production_method_id: String,
    },
    ConstructionRecipe {
        building_id: String,
    },
    DemandBucket {
        bucket: DemandBucketKind,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProductionShortage {
    pub good_id: String,
    pub amount: f32,
    pub ratio: f32,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProductionShortageImpact {
    pub good_id: String,
    pub shortage_amount: f32,
    pub shortage_ratio: f32,
    pub affected_buildings: Vec<ProductionShortageAffectedBuilding>,
    pub affected_demand_buckets: Vec<ProductionShortageAffectedBucket>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProductionShortageAffectedBuilding {
    pub building_id: String,
    pub building_name: String,
    pub amount_per_level: f32,
    pub production_method_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProductionShortageAffectedBucket {
    pub bucket: DemandBucketKind,
    pub requested: f32,
    pub unmet: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProductionChainAction {
    pub good_id: String,
    pub building_id: String,
    pub building_name: String,
    pub production_method_id: String,
    pub production_method_name: String,
    pub amount_per_level: f32,
    pub output_kind: ProductionChainOutputKind,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProductionShortageContext {
    pub demand: f32,
    pub supply: f32,
    pub imports: f32,
    pub exports: f32,
    pub stockpile: f32,
    pub stockpile_coverage_days: f32,
    pub domestic_production: f32,
    pub building_input_demand: f32,
    pub pop_consumption_demand: f32,
    pub military_order_demand: f32,
    pub construction_demand: f32,
    pub is_blockaded: bool,
    pub has_market_bloc_supply: bool,
    pub has_subject_supply: bool,
    pub world_spot_available: f32,
    pub has_active_construction_queue: bool,
}

impl ProductionShortageContext {
    pub fn shortage_amount(&self) -> f32 {
        (self.demand - self.supply).max(0.0)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProductionShortageDiagnosis {
    pub good_id: String,
    pub shortage_amount: f32,
    pub shortage_ratio: f32,
    pub primary_bucket: Option<DemandBucketKind>,
    pub primary_pressure: ProductionShortagePressureKind,
    pub supply_condition: ProductionShortageSupplyCondition,
    pub affected_building_count: usize,
    pub affected_bucket_count: usize,
    pub actions: Vec<ProductionChainRecommendedAction>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProductionShortagePressureKind {
    BuildingInput,
    PopConsumption,
    MilitaryOrders,
    Construction,
    ExportOrders,
    GeneralDemand,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProductionShortageSupplyCondition {
    Stable,
    DomesticMissing,
    ImportsBlocked,
    ImportsInsufficient,
    StockpileBuffering,
    ProductionInsufficient,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProductionChainRecommendedAction {
    pub kind: ProductionChainRecommendedActionKind,
    pub good_id: String,
    pub building_id: Option<String>,
    pub building_name: Option<String>,
    pub production_method_id: Option<String>,
    pub production_method_name: Option<String>,
    pub amount_per_level: f32,
    pub priority: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProductionChainRecommendedActionKind {
    BuildProducer,
    RestoreImportRoute,
    OpenImport,
    UseMarketBloc,
    UseSubjectSupply,
    ReleaseStockpile,
    PauseConstruction,
    ProtectPopConsumption,
    CutExports,
}

impl ProductionChainGraph {
    pub fn from_database(db: &V6Database) -> Self {
        Self::from_database_and_market(db, None)
    }

    pub fn from_database_and_market(db: &V6Database, market: Option<&NationalMarket>) -> Self {
        let mut graph = Self {
            goods: db.goods.iter().map(|good| good.id.clone()).collect(),
            producers_by_good: BTreeMap::new(),
            consumers_by_good: BTreeMap::new(),
            shortage_by_good: BTreeMap::new(),
        };

        for building in &db.buildings {
            graph.index_building_construction_recipe(building);
        }
        for pm in &db.production_methods {
            graph.index_production_method(db, pm);
        }
        if let Some(market) = market {
            graph.index_market(market);
        }

        graph.sort_and_dedup();
        graph
    }

    pub fn contains_good(&self, good_id: &str) -> bool {
        self.goods.contains(good_id)
            || self.producers_by_good.contains_key(good_id)
            || self.consumers_by_good.contains_key(good_id)
    }

    pub fn producers_for_good(&self, good_id: &str) -> &[ProductionChainProducer] {
        self.producers_by_good
            .get(good_id)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    pub fn consumers_for_good(&self, good_id: &str) -> &[ProductionChainConsumer] {
        self.consumers_by_good
            .get(good_id)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    pub fn upstream_goods(&self, good_id: &str) -> Vec<String> {
        let mut goods = BTreeSet::new();
        for producer in self.producers_for_good(good_id) {
            for consumer in self.consumers_by_good.values().flatten() {
                let ProductionChainConsumerKind::ProductionMethodInput {
                    production_method_id,
                    ..
                } = &consumer.kind
                else {
                    continue;
                };
                if production_method_id == &producer.production_method_id {
                    goods.insert(consumer.good_id.clone());
                }
            }
        }
        goods.into_iter().collect()
    }

    pub fn downstream_goods(&self, good_id: &str) -> Vec<String> {
        let mut goods = BTreeSet::new();
        for consumer in self.consumers_for_good(good_id) {
            let ProductionChainConsumerKind::ProductionMethodInput {
                production_method_id,
                ..
            } = &consumer.kind
            else {
                continue;
            };
            for (output_good, producers) in &self.producers_by_good {
                if producers
                    .iter()
                    .any(|producer| &producer.production_method_id == production_method_id)
                {
                    goods.insert(output_good.clone());
                }
            }
        }
        goods.into_iter().collect()
    }

    pub fn shortage_for_good(&self, good_id: &str) -> Option<&ProductionShortage> {
        self.shortage_by_good.get(good_id)
    }

    pub fn shortage_impact(&self, good_id: &str) -> ProductionShortageImpact {
        let shortage = self.shortage_for_good(good_id);
        let mut buildings: BTreeMap<String, ProductionShortageAffectedBuilding> = BTreeMap::new();
        let mut buckets: BTreeMap<DemandBucketKindKey, ProductionShortageAffectedBucket> =
            BTreeMap::new();

        for consumer in self.consumers_for_good(good_id) {
            match &consumer.kind {
                ProductionChainConsumerKind::ProductionMethodInput {
                    building_id,
                    production_method_id,
                } => {
                    let entry = buildings.entry(building_id.clone()).or_insert_with(|| {
                        ProductionShortageAffectedBuilding {
                            building_id: building_id.clone(),
                            building_name: consumer.consumer_name.clone(),
                            amount_per_level: 0.0,
                            production_method_ids: Vec::new(),
                        }
                    });
                    entry.amount_per_level += consumer.amount;
                    if !entry.production_method_ids.contains(production_method_id) {
                        entry
                            .production_method_ids
                            .push(production_method_id.clone());
                    }
                }
                ProductionChainConsumerKind::ConstructionRecipe { building_id } => {
                    let entry = buildings.entry(building_id.clone()).or_insert_with(|| {
                        ProductionShortageAffectedBuilding {
                            building_id: building_id.clone(),
                            building_name: consumer.consumer_name.clone(),
                            amount_per_level: 0.0,
                            production_method_ids: Vec::new(),
                        }
                    });
                    entry.amount_per_level += consumer.amount;
                }
                ProductionChainConsumerKind::DemandBucket { bucket } => {
                    let entry = buckets
                        .entry(DemandBucketKindKey(*bucket))
                        .or_insert_with(|| ProductionShortageAffectedBucket {
                            bucket: *bucket,
                            requested: 0.0,
                            unmet: 0.0,
                        });
                    entry.requested += consumer.amount;
                }
            }
        }

        if let Some(shortage) = shortage {
            for bucket in buckets.values_mut() {
                bucket.unmet = bucket.requested.min(shortage.amount);
            }
        }

        ProductionShortageImpact {
            good_id: good_id.to_owned(),
            shortage_amount: shortage.map(|s| s.amount).unwrap_or(0.0),
            shortage_ratio: shortage.map(|s| s.ratio).unwrap_or(0.0),
            affected_buildings: buildings.into_values().collect(),
            affected_demand_buckets: buckets.into_values().collect(),
        }
    }

    pub fn buildable_actions_for_shortage(&self, good_id: &str) -> Vec<ProductionChainAction> {
        self.producers_for_good(good_id)
            .iter()
            .filter(|producer| producer.buildable)
            .map(|producer| ProductionChainAction {
                good_id: producer.good_id.clone(),
                building_id: producer.building_id.clone(),
                building_name: producer.building_name.clone(),
                production_method_id: producer.production_method_id.clone(),
                production_method_name: producer.production_method_name.clone(),
                amount_per_level: producer.amount_per_level,
                output_kind: producer.output_kind,
            })
            .collect()
    }

    pub fn diagnose_shortage(
        &self,
        good_id: &str,
        context: ProductionShortageContext,
    ) -> ProductionShortageDiagnosis {
        let impact = self.shortage_impact(good_id);
        let shortage_amount = impact
            .shortage_amount
            .max(context.shortage_amount())
            .max(0.0);
        let demand = context.demand.max(impact.shortage_amount).max(0.0);
        let shortage_ratio = if demand > 0.0 {
            impact.shortage_ratio.max(shortage_amount / demand)
        } else {
            impact.shortage_ratio
        };
        let primary_bucket = impact
            .affected_demand_buckets
            .iter()
            .max_by(|a, b| {
                a.unmet
                    .max(a.requested)
                    .partial_cmp(&b.unmet.max(b.requested))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|bucket| bucket.bucket);
        let primary_pressure = primary_pressure(primary_bucket, &context);
        let supply_condition = supply_condition(&context, shortage_amount);
        let mut actions = Vec::new();

        if shortage_amount > 0.0 {
            if context.is_blockaded && context.imports > 0.0 {
                push_recommendation(
                    &mut actions,
                    ProductionChainRecommendedActionKind::RestoreImportRoute,
                    good_id,
                );
            }

            for action in self
                .buildable_actions_for_shortage(good_id)
                .into_iter()
                .take(4)
            {
                actions.push(ProductionChainRecommendedAction {
                    kind: ProductionChainRecommendedActionKind::BuildProducer,
                    good_id: good_id.to_owned(),
                    building_id: Some(action.building_id),
                    building_name: Some(action.building_name),
                    production_method_id: Some(action.production_method_id),
                    production_method_name: Some(action.production_method_name),
                    amount_per_level: action.amount_per_level,
                    priority: actions.len() as u8,
                });
            }

            if context.world_spot_available > 0.0
                || (context.imports <= 0.0 && context.domestic_production < context.demand)
            {
                push_recommendation(
                    &mut actions,
                    ProductionChainRecommendedActionKind::OpenImport,
                    good_id,
                );
            }
            if context.has_market_bloc_supply {
                push_recommendation(
                    &mut actions,
                    ProductionChainRecommendedActionKind::UseMarketBloc,
                    good_id,
                );
            }
            if context.has_subject_supply {
                push_recommendation(
                    &mut actions,
                    ProductionChainRecommendedActionKind::UseSubjectSupply,
                    good_id,
                );
            }
            if context.stockpile > 0.0 {
                push_recommendation(
                    &mut actions,
                    ProductionChainRecommendedActionKind::ReleaseStockpile,
                    good_id,
                );
            }
            if primary_pressure == ProductionShortagePressureKind::Construction
                && context.has_active_construction_queue
            {
                push_recommendation(
                    &mut actions,
                    ProductionChainRecommendedActionKind::PauseConstruction,
                    good_id,
                );
            }
            if primary_pressure == ProductionShortagePressureKind::PopConsumption {
                push_recommendation(
                    &mut actions,
                    ProductionChainRecommendedActionKind::ProtectPopConsumption,
                    good_id,
                );
            }
            if primary_bucket == Some(DemandBucketKind::Export) || context.exports > 0.0 {
                push_recommendation(
                    &mut actions,
                    ProductionChainRecommendedActionKind::CutExports,
                    good_id,
                );
            }
        }

        ProductionShortageDiagnosis {
            good_id: good_id.to_owned(),
            shortage_amount,
            shortage_ratio,
            primary_bucket,
            primary_pressure,
            supply_condition,
            affected_building_count: impact.affected_buildings.len(),
            affected_bucket_count: impact.affected_demand_buckets.len(),
            actions,
        }
    }

    fn index_building_construction_recipe(&mut self, building: &BuildingDef) {
        for material in &building.construction_recipe.materials {
            if material.amount <= 0.0 {
                continue;
            }
            self.goods.insert(material.good_id.clone());
            self.consumers_by_good
                .entry(material.good_id.clone())
                .or_default()
                .push(ProductionChainConsumer {
                    good_id: material.good_id.clone(),
                    consumer_id: building.id.clone(),
                    consumer_name: building.name.clone(),
                    kind: ProductionChainConsumerKind::ConstructionRecipe {
                        building_id: building.id.clone(),
                    },
                    amount: material.amount,
                });
        }
    }

    fn index_production_method(&mut self, db: &V6Database, pm: &ProductionMethodDef) {
        let building = db
            .buildings
            .iter()
            .find(|building| building.id == pm.building_id);
        let building_name = building
            .map(|building| building.name.clone())
            .unwrap_or_else(|| pm.building_id.clone());
        let buildable = building.map(|building| building.buildable).unwrap_or(false);

        for (index, good_id) in pm.output_good_ids.iter().enumerate() {
            let amount = pm.output_good_amounts.get(index).copied().unwrap_or(0.0);
            if amount <= 0.0 {
                continue;
            }
            self.goods.insert(good_id.clone());
            self.producers_by_good
                .entry(good_id.clone())
                .or_default()
                .push(ProductionChainProducer {
                    good_id: good_id.clone(),
                    building_id: pm.building_id.clone(),
                    building_name: building_name.clone(),
                    production_method_id: pm.id.clone(),
                    production_method_name: pm.name.clone(),
                    amount_per_level: amount,
                    output_kind: ProductionChainOutputKind::Good,
                    buildable,
                });
        }

        if let Some(equipment) = &pm.equipment_output {
            if equipment.daily_per_level > 0.0 {
                self.producers_by_good
                    .entry(equipment.equipment_category.clone())
                    .or_default()
                    .push(ProductionChainProducer {
                        good_id: equipment.equipment_category.clone(),
                        building_id: pm.building_id.clone(),
                        building_name: building_name.clone(),
                        production_method_id: pm.id.clone(),
                        production_method_name: pm.name.clone(),
                        amount_per_level: equipment.daily_per_level,
                        output_kind: ProductionChainOutputKind::EquipmentCategory,
                        buildable,
                    });
            }
        }

        for (index, good_id) in pm.input_good_ids.iter().enumerate() {
            let amount = pm.input_good_amounts.get(index).copied().unwrap_or(0.0);
            if amount <= 0.0 {
                continue;
            }
            self.goods.insert(good_id.clone());
            self.consumers_by_good
                .entry(good_id.clone())
                .or_default()
                .push(ProductionChainConsumer {
                    good_id: good_id.clone(),
                    consumer_id: pm.id.clone(),
                    consumer_name: building_name.clone(),
                    kind: ProductionChainConsumerKind::ProductionMethodInput {
                        building_id: pm.building_id.clone(),
                        production_method_id: pm.id.clone(),
                    },
                    amount,
                });
        }
    }

    fn index_market(&mut self, market: &NationalMarket) {
        let mut clearing_bucket_keys: BTreeSet<(String, DemandBucketKindKey)> = BTreeSet::new();
        for (good_id, result) in &market.clearing_sheet.results {
            for bucket in &result.buckets {
                clearing_bucket_keys.insert((good_id.clone(), DemandBucketKindKey(bucket.kind)));
            }
        }

        for ((good_id, bucket), amount) in &market.bucket_demand.entries {
            if *amount <= 0.0 {
                continue;
            }
            if clearing_bucket_keys.contains(&(good_id.clone(), DemandBucketKindKey(*bucket))) {
                continue;
            }
            self.index_demand_bucket(good_id, *bucket, *amount);
        }

        for (good_id, result) in &market.clearing_sheet.results {
            let demand = result.total_fulfilled + result.total_unmet;
            if result.total_unmet > 0.01 || result.shortage_ratio > 0.0 {
                self.shortage_by_good.insert(
                    good_id.clone(),
                    ProductionShortage {
                        good_id: good_id.clone(),
                        amount: result.total_unmet.max(0.0),
                        ratio: result.shortage_ratio.max(if demand > 0.0 {
                            result.total_unmet / demand
                        } else {
                            0.0
                        }),
                    },
                );
            }
            for bucket in &result.buckets {
                if bucket.requested > 0.0 {
                    self.index_demand_bucket(good_id, bucket.kind, bucket.requested);
                }
            }
        }

        for (good_id, demand) in &market.demand {
            let supply = market.supply.get(good_id).copied().unwrap_or(0.0);
            let unmet = market.unmet_demand.get(good_id).copied().unwrap_or(0.0);
            let shortage_amount = unmet.max((*demand - supply).max(0.0));
            if shortage_amount > 0.01 {
                self.shortage_by_good
                    .entry(good_id.clone())
                    .or_insert_with(|| ProductionShortage {
                        good_id: good_id.clone(),
                        amount: shortage_amount,
                        ratio: shortage_amount / demand.max(1.0),
                    });
            }
        }
    }

    fn index_demand_bucket(&mut self, good_id: &str, bucket: DemandBucketKind, amount: f32) {
        self.goods.insert(good_id.to_owned());
        let consumers = self
            .consumers_by_good
            .entry(good_id.to_owned())
            .or_default();
        let consumer_id = format!("demand_bucket:{bucket:?}");
        if let Some(existing) = consumers.iter_mut().find(|consumer| {
            consumer.consumer_id == consumer_id
                && consumer.kind == ProductionChainConsumerKind::DemandBucket { bucket }
        }) {
            existing.amount += amount;
            return;
        }
        consumers.push(ProductionChainConsumer {
            good_id: good_id.to_owned(),
            consumer_id,
            consumer_name: format!("{bucket:?}"),
            kind: ProductionChainConsumerKind::DemandBucket { bucket },
            amount,
        });
    }

    fn sort_and_dedup(&mut self) {
        for producers in self.producers_by_good.values_mut() {
            producers.sort_by(|a, b| {
                a.building_id
                    .cmp(&b.building_id)
                    .then_with(|| a.production_method_id.cmp(&b.production_method_id))
                    .then_with(|| a.good_id.cmp(&b.good_id))
            });
            producers.dedup_by(|a, b| {
                a.good_id == b.good_id
                    && a.building_id == b.building_id
                    && a.production_method_id == b.production_method_id
                    && a.output_kind == b.output_kind
            });
        }

        for consumers in self.consumers_by_good.values_mut() {
            consumers.sort_by(|a, b| {
                a.consumer_id
                    .cmp(&b.consumer_id)
                    .then_with(|| {
                        consumer_kind_sort_key(&a.kind).cmp(&consumer_kind_sort_key(&b.kind))
                    })
                    .then_with(|| a.good_id.cmp(&b.good_id))
            });
            consumers.dedup_by(|a, b| {
                a.good_id == b.good_id && a.consumer_id == b.consumer_id && a.kind == b.kind
            });
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DemandBucketKindKey(DemandBucketKind);

impl DemandBucketKindKey {
    fn sort_value(self) -> u8 {
        self.0.priority()
    }
}

impl Ord for DemandBucketKindKey {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.sort_value()
            .cmp(&other.sort_value())
            .then_with(|| format!("{:?}", self.0).cmp(&format!("{:?}", other.0)))
    }
}

impl PartialOrd for DemandBucketKindKey {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

fn consumer_kind_sort_key(kind: &ProductionChainConsumerKind) -> String {
    match kind {
        ProductionChainConsumerKind::ProductionMethodInput {
            building_id,
            production_method_id,
        } => format!("0:{building_id}:{production_method_id}"),
        ProductionChainConsumerKind::ConstructionRecipe { building_id } => {
            format!("1:{building_id}")
        }
        ProductionChainConsumerKind::DemandBucket { bucket } => {
            format!("2:{}:{bucket:?}", bucket.priority())
        }
    }
}

fn primary_pressure(
    primary_bucket: Option<DemandBucketKind>,
    context: &ProductionShortageContext,
) -> ProductionShortagePressureKind {
    match primary_bucket {
        Some(DemandBucketKind::BuildingInput) => ProductionShortagePressureKind::BuildingInput,
        Some(DemandBucketKind::PopBasicConsumption)
        | Some(DemandBucketKind::PopNonBasicConsumption) => {
            ProductionShortagePressureKind::PopConsumption
        }
        Some(DemandBucketKind::GovernmentProcurement) | Some(DemandBucketKind::MilitaryInput) => {
            ProductionShortagePressureKind::MilitaryOrders
        }
        Some(DemandBucketKind::ConstructionInput) => ProductionShortagePressureKind::Construction,
        Some(DemandBucketKind::Export) => ProductionShortagePressureKind::ExportOrders,
        None => {
            let pressures = [
                (
                    context.building_input_demand,
                    ProductionShortagePressureKind::BuildingInput,
                ),
                (
                    context.pop_consumption_demand,
                    ProductionShortagePressureKind::PopConsumption,
                ),
                (
                    context.military_order_demand,
                    ProductionShortagePressureKind::MilitaryOrders,
                ),
                (
                    context.construction_demand,
                    ProductionShortagePressureKind::Construction,
                ),
                (
                    context.exports,
                    ProductionShortagePressureKind::ExportOrders,
                ),
            ];
            pressures
                .into_iter()
                .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal))
                .filter(|(amount, _)| *amount > 0.0)
                .map(|(_, kind)| kind)
                .unwrap_or(ProductionShortagePressureKind::GeneralDemand)
        }
    }
}

fn supply_condition(
    context: &ProductionShortageContext,
    shortage_amount: f32,
) -> ProductionShortageSupplyCondition {
    if shortage_amount <= 0.0 {
        return ProductionShortageSupplyCondition::Stable;
    }
    if context.is_blockaded && context.imports > 0.0 {
        return ProductionShortageSupplyCondition::ImportsBlocked;
    }
    if context.stockpile > 0.0 && context.stockpile_coverage_days > 0.0 {
        return ProductionShortageSupplyCondition::StockpileBuffering;
    }
    if context.domestic_production <= 0.0 && context.imports <= 0.0 {
        return ProductionShortageSupplyCondition::DomesticMissing;
    }
    if context.imports > 0.0 {
        return ProductionShortageSupplyCondition::ImportsInsufficient;
    }
    ProductionShortageSupplyCondition::ProductionInsufficient
}

fn push_recommendation(
    actions: &mut Vec<ProductionChainRecommendedAction>,
    kind: ProductionChainRecommendedActionKind,
    good_id: &str,
) {
    if actions.iter().any(|action| action.kind == kind) {
        return;
    }
    actions.push(ProductionChainRecommendedAction {
        kind,
        good_id: good_id.to_owned(),
        building_id: None,
        building_name: None,
        production_method_id: None,
        production_method_name: None,
        amount_per_level: 0.0,
        priority: actions.len() as u8,
    });
}
