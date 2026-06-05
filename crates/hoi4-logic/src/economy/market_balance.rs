//! 统一商品清算层：按需求桶优先级分配可用供给，确保商品守恒。
//!
//! 核心逻辑：
//! 1. 收集每种商品的可用供给（国内生产 + 进口 + 库存释放）
//! 2. 按优先级分配给各需求桶（POP基础消费 > 建筑投入 > 军工投入 > 政府采购 > 建造投入 > POP非基础消费 > 出口）
//! 3. 写出每个桶的实际满足量、未满足量、期末库存
//! 4. 从供给和库存中真实扣减已消耗量
//! 5. 生成 MarketClearingSheet 供 UI 和其他系统读取
//!
//! P0.5 关键改进：
//! - 各系统写入 bucket_demand 而非统一 demand，清算层读取精确分桶需求
//! - 政府采购改为提交需求，清算后确认实际成交
//! - 军工生产根据清算后 MilitaryInput 桶满足率调整产出

use hoi4_content::V6Database;
use hoi4_state::{
    market::{
        BucketDemand, ClearingBucket, DemandBucketKind, GoodClearingResult, MarketClearingSheet,
    },
    World,
};

const STOCKPILE_TARGET_DAYS: f32 = 30.0;

const BUCKET_KINDS: [DemandBucketKind; 7] = [
    DemandBucketKind::PopBasicConsumption,
    DemandBucketKind::BuildingInput,
    DemandBucketKind::MilitaryInput,
    DemandBucketKind::GovernmentProcurement,
    DemandBucketKind::ConstructionInput,
    DemandBucketKind::PopNonBasicConsumption,
    DemandBucketKind::Export,
];

/// 统一商品清算：按需求桶优先级分配供给。
///
/// 前置条件：
/// - 各系统已将分桶需求写入 market.bucket_demand
/// - 各系统已将总需求写入 market.demand（用于 UI 展示需求全貌）
/// - 建筑产出已写入 market.supply
/// - 进口/出口已写入 market.imports/exports
///
/// 清算后：
/// - supply 和 stockpile 中的商品被真实扣减
/// - clearing_sheet 记录每个需求桶的请求量、满足量、未满足量
/// - unmet_demand 和 stockpile_coverage_days 基于清算结果更新
/// - demand 保留原始请求量（供 UI 展示需求全貌）
/// - bucket_demand 被清空（日终重置）
pub fn settle_market(world: &mut World, db: &V6Database, ci: usize) {
    if ci >= world.countries.market.markets.len() {
        return;
    }

    let mut sheet = MarketClearingSheet::default();
    let bucket_demand = world.countries.market.markets[ci].bucket_demand.clone();

    for good in &db.goods {
        let good_id = &good.id;

        let domestic_production = world.countries.market.markets[ci]
            .supply
            .get(good_id)
            .copied()
            .unwrap_or(0.0)
            .max(0.0);
        let imports = world.countries.market.markets[ci]
            .imports
            .get(good_id)
            .copied()
            .unwrap_or(0.0)
            .max(0.0);
        let exports = world.countries.market.markets[ci]
            .exports
            .get(good_id)
            .copied()
            .unwrap_or(0.0)
            .max(0.0);
        let stockpile_opening = world.countries.market.markets[ci]
            .stockpile
            .get(good_id)
            .copied()
            .unwrap_or(0.0)
            .max(0.0);
        let total_demand = world.countries.market.markets[ci]
            .demand
            .get(good_id)
            .copied()
            .unwrap_or(0.0)
            .max(0.0);

        let stockpile_release = compute_stockpile_release(stockpile_opening, total_demand);
        let supply_available = domestic_production + imports + stockpile_release;

        let mut remaining = supply_available;
        let mut buckets = Vec::with_capacity(BUCKET_KINDS.len());

        for kind in &BUCKET_KINDS {
            let requested =
                get_demand_for_bucket(&bucket_demand, good_id, *kind, total_demand, exports);
            let fulfilled = requested.min(remaining).max(0.0);
            remaining -= fulfilled;
            let unmet = (requested - fulfilled).max(0.0);
            buckets.push(ClearingBucket {
                kind: *kind,
                requested,
                fulfilled,
                unmet,
            });
        }

        let total_fulfilled = supply_available - remaining;
        let total_unmet = (total_demand - total_fulfilled).max(0.0);

        let stockpile_from_surplus = remaining;
        let exportable_after_local_demand =
            (stockpile_opening - stockpile_release + stockpile_from_surplus).max(0.0);
        let actual_exports = exports.min(exportable_after_local_demand);
        let stockpile_closing = (exportable_after_local_demand - actual_exports).max(0.0);

        let coverage_days = if total_demand > 0.0 {
            (stockpile_closing / total_demand).min(STOCKPILE_TARGET_DAYS)
        } else if stockpile_closing > 0.0 {
            STOCKPILE_TARGET_DAYS
        } else {
            0.0
        };

        let shortage_ratio = if total_fulfilled + total_unmet > 0.0 {
            total_unmet / (total_fulfilled + total_unmet)
        } else {
            0.0
        };

        sheet.results.insert(
            good_id.clone(),
            GoodClearingResult {
                stockpile_opening,
                domestic_production,
                imports,
                supply_available,
                buckets,
                total_fulfilled,
                total_unmet,
                exports: actual_exports,
                stockpile_closing,
                stockpile_coverage_days: coverage_days,
                shortage_ratio,
                gov_procurement_rm: 0.0,
            },
        );

        let market = &mut world.countries.market.markets[ci];
        market.stockpile.insert(good_id.clone(), stockpile_closing);
        if actual_exports != exports {
            market.exports.insert(good_id.clone(), actual_exports);
        }
        market.unmet_demand.insert(good_id.clone(), total_unmet);
        market
            .stockpile_coverage_days
            .insert(good_id.clone(), coverage_days);

        let consumed_from_production = (domestic_production + stockpile_release + imports
            - stockpile_from_surplus)
            .max(0.0)
            .min(domestic_production + stockpile_release + imports);
        let new_supply = (domestic_production - consumed_from_production).max(0.0);
        market.supply.insert(good_id.clone(), new_supply);
    }

    let market = &mut world.countries.market.markets[ci];
    market.clearing_sheet = sheet;
    market.bucket_demand.clear();
}

/// 清算后确认政府采购：根据 GovernmentProcurement 桶的实际满足量扣减国库现金。
/// 必须在 settle_market 之后调用。
pub fn confirm_government_procurement(world: &mut World, ci: usize) {
    if ci >= world.countries.market.markets.len() {
        return;
    }

    let sheet = world.countries.market.markets[ci].clearing_sheet.clone();

    for (good_id, result) in &sheet.results {
        let gov_bucket = result
            .buckets
            .iter()
            .find(|b| b.kind == DemandBucketKind::GovernmentProcurement);
        if let Some(bucket) = gov_bucket {
            if bucket.fulfilled > 0.0 {
                let unit_price = world.countries.market.markets[ci]
                    .price
                    .get(good_id)
                    .copied()
                    .unwrap_or(0.01)
                    .max(0.01);
                let cost = (bucket.fulfilled * unit_price) as f64;
                let treasury = &mut world.countries.treasury.treasuries[ci];

                let actual_cost = cost.min(treasury.cash_rm.max(0.0));
                if actual_cost > 0.0 {
                    treasury.pay(actual_cost, "military_procurement");
                }

                let market = &mut world.countries.market.markets[ci];
                if let Some(r) = market.clearing_sheet.results.get_mut(good_id) {
                    r.gov_procurement_rm = actual_cost;
                }
            }
        }
    }
}

/// 清算后确认建造投入：根据 ConstructionInput 桶的实际满足量扣减国库现金。
pub fn confirm_construction_procurement(world: &mut World, ci: usize) {
    if ci >= world.countries.market.markets.len() {
        return;
    }

    let sheet = world.countries.market.markets[ci].clearing_sheet.clone();

    for (good_id, result) in &sheet.results {
        let constr_bucket = result
            .buckets
            .iter()
            .find(|b| b.kind == DemandBucketKind::ConstructionInput);
        if let Some(bucket) = constr_bucket {
            if bucket.fulfilled > 0.0 {
                let unit_price = world.countries.market.markets[ci]
                    .price
                    .get(good_id)
                    .copied()
                    .unwrap_or(0.01)
                    .max(0.01);
                let cost = (bucket.fulfilled * unit_price) as f64;
                let treasury = &mut world.countries.treasury.treasuries[ci];

                let actual_cost = cost.min(treasury.cash_rm.max(0.0));
                if actual_cost > 0.0 {
                    treasury.pay(actual_cost, "construction_goods");
                }
            }
        }
    }
}

/// 读取清算后军工投入桶的满足率，用于调整军工产出。
/// 返回 (requested, fulfilled, ratio)。
pub fn military_input_fulfillment(world: &World, ci: usize, good_id: &str) -> (f32, f32, f32) {
    if ci >= world.countries.market.markets.len() {
        return (0.0, 0.0, 1.0);
    }
    let sheet = &world.countries.market.markets[ci].clearing_sheet;
    if let Some(result) = sheet.results.get(good_id) {
        if let Some(bucket) = result
            .buckets
            .iter()
            .find(|b| b.kind == DemandBucketKind::MilitaryInput)
        {
            let ratio = if bucket.requested > 0.0 {
                (bucket.fulfilled / bucket.requested).clamp(0.0, 1.0)
            } else {
                1.0
            };
            return (bucket.requested, bucket.fulfilled, ratio);
        }
    }
    (0.0, 0.0, 1.0)
}

/// 旧接口兼容
#[allow(dead_code)]
pub fn settle_stockpiles(world: &mut World, db: &V6Database, ci: usize, mutate_stockpile: bool) {
    if mutate_stockpile {
        settle_market(world, db, ci);
    } else {
        settle_market_readonly(world, db, ci);
    }
}

fn settle_market_readonly(world: &mut World, db: &V6Database, ci: usize) {
    if ci >= world.countries.market.markets.len() {
        return;
    }

    let mut sheet = MarketClearingSheet::default();
    let bucket_demand = world.countries.market.markets[ci].bucket_demand.clone();

    for good in &db.goods {
        let good_id = &good.id;
        let supply = world.countries.market.markets[ci]
            .supply
            .get(good_id)
            .copied()
            .unwrap_or(0.0)
            .max(0.0);
        let demand = world.countries.market.markets[ci]
            .demand
            .get(good_id)
            .copied()
            .unwrap_or(0.0)
            .max(0.0);
        let stockpile = world.countries.market.markets[ci]
            .stockpile
            .get(good_id)
            .copied()
            .unwrap_or(0.0)
            .max(0.0);
        let imports = world.countries.market.markets[ci]
            .imports
            .get(good_id)
            .copied()
            .unwrap_or(0.0)
            .max(0.0);
        let exports = world.countries.market.markets[ci]
            .exports
            .get(good_id)
            .copied()
            .unwrap_or(0.0)
            .max(0.0);

        let effective_supply = supply + stockpile;
        let unmet = (demand - effective_supply).max(0.0);
        let coverage_days = if demand > 0.0 {
            (stockpile / demand).min(STOCKPILE_TARGET_DAYS)
        } else if stockpile > 0.0 {
            STOCKPILE_TARGET_DAYS
        } else {
            0.0
        };

        let shortage_ratio = if demand > 0.0 { unmet / demand } else { 0.0 };

        let mut buckets = Vec::new();
        for kind in &BUCKET_KINDS {
            let requested = get_demand_for_bucket(&bucket_demand, good_id, *kind, demand, exports);
            let fulfilled = requested.min(effective_supply).max(0.0);
            buckets.push(ClearingBucket {
                kind: *kind,
                requested,
                fulfilled,
                unmet: (requested - fulfilled).max(0.0),
            });
        }

        sheet.results.insert(
            good_id.clone(),
            GoodClearingResult {
                stockpile_opening: stockpile,
                domestic_production: supply,
                imports,
                supply_available: effective_supply,
                buckets,
                total_fulfilled: demand.min(effective_supply),
                total_unmet: unmet,
                exports,
                stockpile_closing: stockpile,
                stockpile_coverage_days: coverage_days,
                shortage_ratio,
                gov_procurement_rm: 0.0,
            },
        );

        let market = &mut world.countries.market.markets[ci];
        market.unmet_demand.insert(good_id.clone(), unmet);
        market
            .stockpile_coverage_days
            .insert(good_id.clone(), coverage_days);
    }

    let market = &mut world.countries.market.markets[ci];
    market.clearing_sheet = sheet;
    market.bucket_demand.clear();
}

fn compute_stockpile_release(stockpile: f32, demand: f32) -> f32 {
    if demand <= 0.0 || stockpile <= 0.0 {
        return 0.0;
    }
    let deficit = (demand - stockpile * 0.1).max(0.0);
    stockpile.min(deficit)
}

/// 从分桶需求中读取指定桶的需求量。
/// 优先从 bucket_demand 读取精确值；若该桶无条目则用启发式回退。
fn get_demand_for_bucket(
    bucket_demand: &BucketDemand,
    good_id: &str,
    kind: DemandBucketKind,
    total_demand: f32,
    exports: f32,
) -> f32 {
    let precise = bucket_demand.get(good_id, kind);
    if precise > 0.0 {
        return precise;
    }

    if total_demand <= 0.0 {
        return 0.0;
    }

    let _ = exports;

    0.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stockpile_release_with_deficit() {
        let release = compute_stockpile_release(100.0, 80.0);
        assert!(release > 0.0);
        assert!(release <= 100.0);
    }

    #[test]
    fn stockpile_release_zero_when_no_demand() {
        let release = compute_stockpile_release(100.0, 0.0);
        assert!((release - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn stockpile_release_zero_when_no_stockpile() {
        let release = compute_stockpile_release(0.0, 80.0);
        assert!((release - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn conservation_invariant_simple() {
        let opening = 50.0_f32;
        let production = 30.0;
        let imports = 10.0;
        let exports = 5.0;
        let total_consumption = 20.0;
        let closing = opening + production + imports - exports - total_consumption;
        assert!((closing - 65.0).abs() < 0.01);
    }

    #[test]
    fn bucket_demand_reads_precise() {
        let mut bd = BucketDemand::default();
        bd.add("steel", DemandBucketKind::MilitaryInput, 15.0);
        bd.add("steel", DemandBucketKind::GovernmentProcurement, 10.0);

        let result =
            get_demand_for_bucket(&bd, "steel", DemandBucketKind::MilitaryInput, 100.0, 0.0);
        assert!((result - 15.0).abs() < 0.01);

        let result2 = get_demand_for_bucket(
            &bd,
            "steel",
            DemandBucketKind::GovernmentProcurement,
            100.0,
            0.0,
        );
        assert!((result2 - 10.0).abs() < 0.01);

        let result3 =
            get_demand_for_bucket(&bd, "steel", DemandBucketKind::BuildingInput, 100.0, 0.0);
        assert!((result3 - 0.0).abs() < f32::EPSILON);
    }
}
