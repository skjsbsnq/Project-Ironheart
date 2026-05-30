//! Phase 1.2：1 天 smoke。
//!
//! 跑 1 个 game-day（24 hour ticks）。
//! - 加载真实 vanilla
//! - 不 panic
//! - PP 应增长（base +1/天 + idea modifier）
//! - 工厂 / 研究 / focus 1 天内不变（建造需 >70 天，研究需 ~150 天）。

use hoi4_integration::{load_world, tick_days, Metrics};

#[test]
fn smoke_1_day() {
    let mut world = load_world()
        .expect("HOI4 安装目录未找到；设置 IRONHEART_HOI4_PATH 或安装到默认 Steam 路径");

    // 抽样 GER 作为参考国
    let before = Metrics::sample(&world, "GER");
    println!("[smoke_1d] before: {:#?}", before);

    let report = tick_days(&mut world, 1);
    println!(
        "[smoke_1d] {} day in {:.2} ms ({:.2} ms/day) — {}",
        report.days,
        report.total.as_secs_f64() * 1000.0,
        report.ms_per_day,
        report.report_line,
    );

    let after = Metrics::sample(&world, "GER");
    println!("[smoke_1d] after:  {:#?}", after);

    // 1 天建造不应改变 V6 建筑数。
    assert_eq!(before.industry_buildings, after.industry_buildings);
    assert_eq!(before.military_buildings, after.military_buildings);
    assert_eq!(before.shipyards, after.shipyards);

    // PP 应该增长（base +1/天 + idea modifier）
    assert!(
        after.political_power > before.political_power,
        "GER PP should grow daily: before={}, after={}",
        before.political_power,
        after.political_power,
    );

    // 时间确实推进了 1 天 = 24 小时。
    assert_eq!(world.elapsed_hours, 24);
}
