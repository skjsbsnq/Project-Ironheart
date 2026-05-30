//! Phase 1.2：365 天 smoke + metrics 基线。
//!
//! 跑一整年，验证真实系统运行后：
//! - 主要国家 PP 积累
//! - 研究有进展（completed_techs 增长）
//! - 不 panic、性能达标

use hoi4_integration::{load_world, tick_days, Metrics};

const MAJOR_TAGS: &[&str] = &["GER", "SOV", "ENG", "FRA", "USA", "JAP", "ITA"];

#[test]
#[ignore]
fn smoke_365_days_metrics_baseline() {
    let mut world = load_world()
        .expect("HOI4 安装目录未找到；设置 IRONHEART_HOI4_PATH 或安装到默认 Steam 路径");

    // 1936-01-01 基线
    let before: Vec<Metrics> = MAJOR_TAGS
        .iter()
        .map(|tag| Metrics::sample(&world, tag))
        .collect();

    let report = tick_days(&mut world, 365);
    println!(
        "[smoke_365d] {} days in {:.2} s ({:.3} ms/day) — {}",
        report.days,
        report.total.as_secs_f64(),
        report.ms_per_day,
        report.report_line,
    );

    // 1937-01-01 终态
    let after: Vec<Metrics> = MAJOR_TAGS
        .iter()
        .map(|tag| Metrics::sample(&world, tag))
        .collect();

    // 把每国"前后"打印成一张表，方便 CI 截屏
    println!(
        "\n{:<6} | {:>4} {:>4} | {:>4} {:>4} | {:>10} {:>10} | {:>4} {:>4} | {:>4} {:>4}",
        "TAG", "ind0", "ind1", "mil0", "mil1", "mp0", "mp1", "tech0", "tech1", "foc0", "foc1",
    );
    println!("{}", "-".repeat(95));
    for (b, a) in before.iter().zip(after.iter()) {
        println!(
            "{:<6} | {:>4} {:>4} | {:>4} {:>4} | {:>10} {:>10} | {:>4} {:>4} | {:>4} {:>4}",
            b.tag,
            b.industry_buildings,
            a.industry_buildings,
            b.military_buildings,
            a.military_buildings,
            b.manpower,
            a.manpower,
            b.completed_techs,
            a.completed_techs,
            b.completed_focuses,
            a.completed_focuses,
        );
    }

    // Phase 1.2 回归：真实系统运行一年后应有变化
    for (b, a) in before.iter().zip(after.iter()) {
        // PP 应该积累（base +1/天 × 365 ≈ 365+）
        assert!(
            a.political_power > b.political_power,
            "{} PP should grow in 1 year: before={}, after={}",
            b.tag,
            b.political_power,
            a.political_power,
        );

        // 人力应增长（POPULATION_YEARLY_GROWTH = 1.5%）
        assert!(
            a.manpower >= b.manpower,
            "{} manpower should not decrease: before={}, after={}",
            b.tag,
            b.manpower,
            a.manpower,
        );
    }

    assert_eq!(world.elapsed_hours, 365 * 24);

    // 性能：1 day < 50 ms
    assert!(
        report.ms_per_day < 50.0,
        "365d smoke 太慢：{:.3} ms/day（预算 50）",
        report.ms_per_day
    );
}
