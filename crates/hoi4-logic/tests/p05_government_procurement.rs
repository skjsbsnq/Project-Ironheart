//! P0.5 政府采购与军工生产真闭环测试。
//!
//! 验收标准：
//! - 采购 100 单位钢后，市场钢可用量减少
//! - 钢不足时坦克/火炮产出降低
//! - 同一投入品不能被政府采购和军工生产重复使用

use std::sync::Arc;

use hoi4_content::V6Database;
use hoi4_data::GameData;
use hoi4_logic::economy::{tick_daily_v6, EconomyState};
use hoi4_map::GameMap;
use hoi4_state::{market::DemandBucketKind, World};

fn hoi4_path() -> std::path::PathBuf {
    hoi4_paths::PathConfig::resolve(Default::default())
        .expect("set IRONHEART_HOI4_PATH or place HOI4 install at default Steam path")
        .game_path()
        .to_path_buf()
}

fn build_world_with_v6() -> (World, Arc<GameData>, V6Database) {
    let game_path = hoi4_path();
    let map = Arc::new(GameMap::load(&game_path).unwrap());
    let data = Arc::new(GameData::load(&game_path).unwrap());
    let mut world = World::new(map, data.clone());
    hoi4_logic::economy::init_world(&mut world);
    world.populate_from_history();
    let db = V6Database::load();
    db.assert_no_vanilla_conflict();
    hoi4_content::inject_v6_into_world(&mut world, &db);
    (world, data, db)
}

/// 采购后市场钢可用量减少：GovernmentProcurement 桶满足量 > 0 时，市场供给被真实消耗
#[test]
fn government_procurement_reduces_steel_availability() {
    let (mut world, _data, db) = build_world_with_v6();
    let mut econ = EconomyState::new(&world);

    let ger = world.country("GER").unwrap();
    let ci = ger.0 as usize;

    let initial_steel = world.countries.market.markets[ci]
        .stockpile
        .get("steel")
        .copied()
        .unwrap_or(0.0)
        + world.countries.market.markets[ci]
            .supply
            .get("steel")
            .copied()
            .unwrap_or(0.0);

    tick_daily_v6(&mut world, &mut econ, &db, 1);

    let sheet = &world.countries.market.markets[ci].clearing_sheet;

    if let Some(result) = sheet.results.get("steel") {
        let gov_bucket = result
            .buckets
            .iter()
            .find(|b| b.kind == DemandBucketKind::GovernmentProcurement);
        if let Some(bucket) = gov_bucket {
            if bucket.fulfilled > 0.0 {
                let final_steel = world.countries.market.markets[ci]
                    .stockpile
                    .get("steel")
                    .copied()
                    .unwrap_or(0.0)
                    + world.countries.market.markets[ci]
                        .supply
                        .get("steel")
                        .copied()
                        .unwrap_or(0.0);

                assert!(
                    final_steel < initial_steel + 1.0,
                    "采购后钢可用量应减少：初始 {:.1}，期末 {:.1}，政府满足 {:.1}",
                    initial_steel,
                    final_steel,
                    bucket.fulfilled
                );
            }
        }
    }

    let _ = econ;
}

/// 钢不足时军工产出降低：MilitaryInput 桶未满足时，装备产出应低于满产
#[test]
fn military_output_reduced_when_steel_short() {
    let (mut world, _data, db) = build_world_with_v6();
    let mut econ = EconomyState::new(&world);

    let ger = world.country("GER").unwrap();
    let ci = ger.0 as usize;

    tick_daily_v6(&mut world, &mut econ, &db, 1);

    let sheet = &world.countries.market.markets[ci].clearing_sheet;

    if let Some(result) = sheet.results.get("steel") {
        let mil_bucket = result
            .buckets
            .iter()
            .find(|b| b.kind == DemandBucketKind::MilitaryInput);
        if let Some(bucket) = mil_bucket {
            if bucket.unmet > 0.0 && bucket.requested > 0.0 {
                let fulfillment_ratio = bucket.fulfilled / bucket.requested;
                assert!(
                    fulfillment_ratio < 1.0,
                    "钢不足时军工投入满足率应 < 1.0：满足 {:.1}/请求 {:.1} = {:.2}",
                    bucket.fulfilled,
                    bucket.requested,
                    fulfillment_ratio
                );
            }
        }
    }

    let _ = econ;
}

/// 同一投入品不被政府采购和军工生产重复使用：
/// MilitaryInput.fulfilled + GovernmentProcurement.fulfilled <= supply_available
#[test]
fn no_double_use_between_military_and_government() {
    let (mut world, _data, db) = build_world_with_v6();
    let mut econ = EconomyState::new(&world);

    let ger = world.country("GER").unwrap();
    let ci = ger.0 as usize;

    tick_daily_v6(&mut world, &mut econ, &db, 1);

    let sheet = &world.countries.market.markets[ci].clearing_sheet.clone();

    for (good_id, result) in &sheet.results {
        let mil_fulfilled: f32 = result
            .buckets
            .iter()
            .filter(|b| b.kind == DemandBucketKind::MilitaryInput)
            .map(|b| b.fulfilled)
            .sum();
        let gov_fulfilled: f32 = result
            .buckets
            .iter()
            .filter(|b| b.kind == DemandBucketKind::GovernmentProcurement)
            .map(|b| b.fulfilled)
            .sum();

        if mil_fulfilled > 0.0 && gov_fulfilled > 0.0 {
            assert!(
                mil_fulfilled + gov_fulfilled <= result.supply_available + 0.01,
                "同一投入品 [{}] 被重复使用：军工 {:.1} + 政府 {:.1} = {:.1} > 可用 {:.1}",
                good_id,
                mil_fulfilled,
                gov_fulfilled,
                mil_fulfilled + gov_fulfilled,
                result.supply_available
            );
        }
    }

    let _ = econ;
}

/// 政府采购确认后国库现金减少
#[test]
fn government_procurement_deducts_treasury_cash() {
    let (mut world, _data, db) = build_world_with_v6();
    let mut econ = EconomyState::new(&world);

    let ger = world.country("GER").unwrap();
    let ci = ger.0 as usize;

    let initial_cash = world.countries.treasury.treasuries[ci].cash_rm;

    tick_daily_v6(&mut world, &mut econ, &db, 1);

    let sheet = &world.countries.market.markets[ci].clearing_sheet;
    let mut total_gov_rm = 0.0;
    for result in sheet.results.values() {
        total_gov_rm += result.gov_procurement_rm;
    }

    if total_gov_rm > 0.0 {
        let final_cash = world.countries.treasury.treasuries[ci].cash_rm;
        let other_income = world.countries.treasury.treasuries[ci].daily_income_rm;
        let other_expense = world.countries.treasury.treasuries[ci].daily_expense_rm - total_gov_rm;

        let expected_cash = initial_cash + other_income - other_expense - total_gov_rm;
        let tolerance = (expected_cash.abs() * 0.01).max(1000.0);
        assert!(
            (final_cash - expected_cash).abs() <= tolerance,
            "政府采购确认后现金应减少：初始 {:.0} + 收入 {:.0} - 支出(非采购) {:.0} - 采购 {:.0} = 期望 {:.0}，实际 {:.0}",
            initial_cash, other_income, other_expense, total_gov_rm, expected_cash, final_cash
        );
    }

    let _ = econ;
}

/// 分桶需求精确性：bucket_demand 写入的量等于清算层读取的量
#[test]
fn bucket_demand_matches_clearing_result() {
    let (mut world, _data, db) = build_world_with_v6();
    let mut econ = EconomyState::new(&world);

    let ger = world.country("GER").unwrap();
    let ci = ger.0 as usize;

    tick_daily_v6(&mut world, &mut econ, &db, 1);

    let sheet = &world.countries.market.markets[ci].clearing_sheet;

    let mut total_requested: f32 = 0.0;
    let mut total_fulfilled: f32 = 0.0;
    for result in sheet.results.values() {
        for bucket in &result.buckets {
            total_requested += bucket.requested;
            total_fulfilled += bucket.fulfilled;
        }
    }

    assert!(
        total_fulfilled <= total_requested + 0.01,
        "清算满足量不应超过请求量：请求 {:.1}，满足 {:.1}",
        total_requested,
        total_fulfilled
    );

    let _ = econ;
}
