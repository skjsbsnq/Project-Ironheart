//! P0.4 商品清算守恒测试。
//!
//! 验收标准：
//! - 商品守恒：期初库存 + 生产 + 进口 - 消费 - 投入 - 出口 = 期末库存
//! - POP 消费会扣库存
//! - 建筑投入会扣库存或当日供给
//! - 政府采购会占用商品
//! - 军工生产会消耗投入品

use std::sync::Arc;

use hoi4_content::V6Database;
use hoi4_data::GameData;
use hoi4_logic::economy::{tick_daily_v6, EconomyState};
use hoi4_map::GameMap;
use hoi4_state::World;

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
    if let Some(ger) = world.country("GER") {
        world.player = ger;
    }
    (world, data, db)
}

/// 核心守恒测试：每种商品满足 期初库存 + 生产 + 进口 - 总消耗 = 期末库存
/// 总消耗 = 各需求桶满足量之和 + 出口
#[test]
fn commodity_conservation_per_good() {
    let (mut world, _data, db) = build_world_with_v6();
    let mut econ = EconomyState::new(&world);

    let ger = world.country("GER").unwrap();
    let ci = ger.0 as usize;

    tick_daily_v6(&mut world, &mut econ, &db, 1);

    let market = &world.countries.market.markets[ci];
    let sheet = &market.clearing_sheet;

    for (good_id, result) in &sheet.results {
        let opening = result.stockpile_opening;
        let production = result.domestic_production;
        let imports = result.imports;
        let exports = result.exports;
        let closing = result.stockpile_closing;

        let total_consumed: f32 = result.buckets.iter().map(|b| b.fulfilled).sum();

        let expected_closing = opening + production + imports - total_consumed - exports;

        let tolerance = (expected_closing.abs() * 0.05).max(1.0);
        let diff = (closing - expected_closing).abs();
        assert!(
            diff <= tolerance,
            "商品守恒违反 [{}]：期初({:.2}) + 生产({:.2}) + 进口({:.2}) - 消耗({:.2}) - 出口({:.2}) = 期望期末({:.2})，实际期末({:.2})，差值({:.2})",
            good_id, opening, production, imports, total_consumed, exports, expected_closing, closing, diff
        );
    }
}

/// POP 消费会扣库存：清算后 POP 消费桶有满足量时，库存应减少
#[test]
fn pop_consumption_reduces_stockpile() {
    let (mut world, _data, db) = build_world_with_v6();
    let mut econ = EconomyState::new(&world);

    let ger = world.country("GER").unwrap();
    let ci = ger.0 as usize;

    tick_daily_v6(&mut world, &mut econ, &db, 1);

    let sheet = &world.countries.market.markets[ci].clearing_sheet;

    let mut found_pop_consumption = false;
    for (good_id, result) in &sheet.results {
        let pop_basic = result
            .buckets
            .iter()
            .find(|b| b.kind == hoi4_state::market::DemandBucketKind::PopBasicConsumption);
        let pop_nonbasic = result
            .buckets
            .iter()
            .find(|b| b.kind == hoi4_state::market::DemandBucketKind::PopNonBasicConsumption);

        let pop_fulfilled = pop_basic.map(|b| b.fulfilled).unwrap_or(0.0)
            + pop_nonbasic.map(|b| b.fulfilled).unwrap_or(0.0);

        if pop_fulfilled > 0.0 {
            found_pop_consumption = true;
            let opening = result.stockpile_opening;
            let closing = result.stockpile_closing;
            let production = result.domestic_production;
            let imports = result.imports;

            if production + imports < pop_fulfilled {
                let stock_draw = pop_fulfilled - production - imports;
                let actual_draw = opening - closing + production + imports;
                let tolerance = stock_draw.abs() * 0.1 + 1.0;
                assert!(
                    actual_draw >= stock_draw - tolerance,
                    "POP 消费未真实扣库存 [{}]：POP消耗 {:.2}，生产+进口仅 {:.2}，应从库存提取 {:.2}，实际提取 {:.2}",
                    good_id, pop_fulfilled, production + imports, stock_draw, actual_draw
                );
            }
        }
    }

    assert!(
        found_pop_consumption,
        "测试未能发现任何 POP 消费满足，可能数据异常"
    );
}

/// 建筑投入会扣库存：清算后建筑投入桶有满足量时，对应商品供给/库存减少
#[test]
fn building_input_reduces_supply_or_stockpile() {
    let (mut world, _data, db) = build_world_with_v6();
    let mut econ = EconomyState::new(&world);

    let ger = world.country("GER").unwrap();
    let ci = ger.0 as usize;

    tick_daily_v6(&mut world, &mut econ, &db, 1);

    let sheet = &world.countries.market.markets[ci].clearing_sheet;

    let mut found_building_input = false;
    for (good_id, result) in &sheet.results {
        let building_input = result
            .buckets
            .iter()
            .find(|b| b.kind == hoi4_state::market::DemandBucketKind::BuildingInput);

        if let Some(bucket) = building_input {
            if bucket.fulfilled > 0.0 {
                found_building_input = true;

                assert!(
                    bucket.fulfilled <= result.supply_available,
                    "建筑投入满足量 [{}]：满足 {:.2} 超过可用供给 {:.2}",
                    good_id,
                    bucket.fulfilled,
                    result.supply_available
                );
            }
        }
    }

    assert!(
        found_building_input,
        "测试未能发现任何建筑投入满足，可能数据异常"
    );
}

/// 政府采购会占用商品：清算后政府采购桶有满足量时，商品被真实消耗
#[test]
fn government_procurement_consumes_goods() {
    let (mut world, _data, db) = build_world_with_v6();
    let mut econ = EconomyState::new(&world);

    let ger = world.country("GER").unwrap();
    let ci = ger.0 as usize;

    tick_daily_v6(&mut world, &mut econ, &db, 1);

    let sheet = &world.countries.market.markets[ci].clearing_sheet;

    let mut found_procurement = false;
    for (good_id, result) in &sheet.results {
        let gov_bucket = result
            .buckets
            .iter()
            .find(|b| b.kind == hoi4_state::market::DemandBucketKind::GovernmentProcurement);

        if let Some(bucket) = gov_bucket {
            if bucket.fulfilled > 0.0 {
                found_procurement = true;

                assert!(
                    bucket.fulfilled <= result.supply_available,
                    "政府采购满足量 [{}]：满足 {:.2} 超过可用供给 {:.2}",
                    good_id,
                    bucket.fulfilled,
                    result.supply_available
                );

                let total_consumed: f32 = result.buckets.iter().map(|b| b.fulfilled).sum();
                assert!(
                    total_consumed <= result.supply_available + 0.01,
                    "政府采购后总消耗 [{}]：总消耗 {:.2} 超过可用供给 {:.2}",
                    good_id,
                    total_consumed,
                    result.supply_available
                );
            }
        }
    }

    assert!(
        found_procurement,
        "测试未能发现任何政府采购满足，可能数据异常"
    );
}

/// 军工生产消耗投入品：清算后军工投入桶有满足量时，投入品被真实消耗
#[test]
fn military_production_consumes_inputs() {
    let (mut world, _data, db) = build_world_with_v6();
    let mut econ = EconomyState::new(&world);

    let ger = world.country("GER").unwrap();
    let ci = ger.0 as usize;

    tick_daily_v6(&mut world, &mut econ, &db, 1);

    let sheet = &world.countries.market.markets[ci].clearing_sheet;

    let mut found_military_input = false;
    for (good_id, result) in &sheet.results {
        let mil_bucket = result
            .buckets
            .iter()
            .find(|b| b.kind == hoi4_state::market::DemandBucketKind::MilitaryInput);

        if let Some(bucket) = mil_bucket {
            if bucket.fulfilled > 0.0 {
                found_military_input = true;

                assert!(
                    bucket.fulfilled <= result.supply_available,
                    "军工投入满足量 [{}]：满足 {:.2} 超过可用供给 {:.2}",
                    good_id,
                    bucket.fulfilled,
                    result.supply_available
                );
            }
        }
    }

    assert!(
        found_military_input,
        "测试未能发现任何军工投入消耗，可能数据异常"
    );
}

/// 同一商品不能被多个需求桶重复使用：总满足量不超过可用供给
#[test]
fn no_double_counting_across_buckets() {
    let (mut world, _data, db) = build_world_with_v6();
    let mut econ = EconomyState::new(&world);

    let ger = world.country("GER").unwrap();
    let ci = ger.0 as usize;

    tick_daily_v6(&mut world, &mut econ, &db, 1);

    let sheet = &world.countries.market.markets[ci].clearing_sheet;

    for (good_id, result) in &sheet.results {
        let total_fulfilled: f32 = result.buckets.iter().map(|b| b.fulfilled).sum();
        assert!(
            total_fulfilled <= result.supply_available + 0.01,
            "需求桶重复消耗 [{}]：总满足 {:.2} 超过可用供给 {:.2}，同一商品被多桶重复使用",
            good_id,
            total_fulfilled,
            result.supply_available
        );
    }
}

/// 清算表非空：确保清算层实际运行并产生结果
#[test]
fn clearing_sheet_not_empty() {
    let (mut world, _data, db) = build_world_with_v6();
    let mut econ = EconomyState::new(&world);

    let ger = world.country("GER").unwrap();
    let ci = ger.0 as usize;

    tick_daily_v6(&mut world, &mut econ, &db, 1);

    let sheet = &world.countries.market.markets[ci].clearing_sheet;
    assert!(
        !sheet.results.is_empty(),
        "清算表不应为空——至少应该有部分商品的清算结果"
    );

    let has_any_activity = sheet.results.values().any(|r| {
        r.domestic_production > 0.0 || r.total_fulfilled > 0.0 || r.stockpile_opening > 0.0
    });
    assert!(
        has_any_activity,
        "清算表中应至少有一个商品存在生产、满足或库存活动"
    );
}

/// 30 天守恒测试：连续 30 天回放后守恒仍成立
#[test]
fn commodity_conservation_30_days() {
    let (mut world, _data, db) = build_world_with_v6();
    let mut econ = EconomyState::new(&world);

    let ger = world.country("GER").unwrap();
    let ci = ger.0 as usize;

    for day in 0..30 {
        tick_daily_v6(&mut world, &mut econ, &db, day + 1);
    }

    let sheet = &world.countries.market.markets[ci].clearing_sheet;

    for (good_id, result) in &sheet.results {
        let opening = result.stockpile_opening;
        let production = result.domestic_production;
        let imports = result.imports;
        let exports = result.exports;
        let closing = result.stockpile_closing;

        let total_consumed: f32 = result.buckets.iter().map(|b| b.fulfilled).sum();

        let expected_closing = opening + production + imports - total_consumed - exports;

        let tolerance = (expected_closing.abs() * 0.05).max(1.0);
        let diff = (closing - expected_closing).abs();
        assert!(
            diff <= tolerance,
            "30天守恒违反 [{}]：期初({:.2}) + 生产({:.2}) + 进口({:.2}) - 消耗({:.2}) - 出口({:.2}) = 期望期末({:.2})，实际({:.2})",
            good_id, opening, production, imports, total_consumed, exports, expected_closing, closing
        );
    }

    let _ = econ;
}
