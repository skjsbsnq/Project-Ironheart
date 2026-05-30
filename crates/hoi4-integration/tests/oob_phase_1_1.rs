//! Phase 1.1 验收：1936-01-01 启动时主要国家的师数与 vanilla `history/units/`
//! 文件实际师数 ±10% 相符。
//!
//! 注：ROADMAP_V3 引用的 wiki 数字（GER ≈24 / ENG ≈13 / SOV ≈107）来自旧版
//! HOI4。当前 vanilla（含历史补丁后的内容更新）`history/units/<TAG>_1936.txt` 实
//! 际师数为：
//!   GER 30, ENG 36, SOV 138, FRA 75, USA 36, JAP 60, ITA 46, CHI 57。
//! 我们以**当前 vanilla 文件**为唯一事实来源（grep `division\s*=\s*\{`），
//! 把 V3 验收范围按 ±10% 重新校准。

use hoi4_integration::{load_world, ReexportWorld as World};

fn count_divisions_for(world: &World, tag: &str) -> usize {
    let cid = match world.country(tag) {
        Some(c) => c,
        None => return 0,
    };
    let mut n = 0usize;
    for i in 0..world.divisions.count {
        if world.divisions.owners[i] == cid {
            n += 1;
        }
    }
    n
}

#[test]
fn phase_1_1_oob_division_counts() {
    let world = load_world().expect("HOI4 install required for Phase 1.1 acceptance");

    let tags = ["GER", "ENG", "SOV", "FRA", "USA", "JAP", "ITA", "CHI"];
    println!("\n[Phase 1.1] 1936-01-01 division count per country:");
    for tag in &tags {
        let n = count_divisions_for(&world, tag);
        println!("  {}: {} divisions", tag, n);
    }

    let ger = count_divisions_for(&world, "GER");
    let eng = count_divisions_for(&world, "ENG");
    let sov = count_divisions_for(&world, "SOV");

    // 当前 vanilla `history/units/<TAG>_1936.txt` 师数（精确 grep 计数）：
    //   GER 30, ENG 36, SOV 138。允差 ±10%。
    assert!(
        (27..=33).contains(&ger),
        "GER divisions = {}，预期 27..=33（vanilla 30 ±10%）",
        ger,
    );
    assert!(
        (32..=39).contains(&eng),
        "ENG divisions = {}，预期 32..=39（vanilla 36 ±10%）",
        eng,
    );
    assert!(
        (124..=151).contains(&sov),
        "SOV divisions = {}，预期 124..=151（vanilla 138 ±10%）",
        sov,
    );

    // 全球总师数：vanilla 1936 文件聚合 700+
    assert!(
        world.divisions.count > 600,
        "全球师数 {} 太少；OOB 加载可能未生效",
        world.divisions.count,
    );

    // 七大国应全部 > 0（avoids 退化到"只解析了某个全局子集"的 bug）
    for tag in &tags {
        let n = count_divisions_for(&world, tag);
        assert!(n > 0, "{} 师数 {} 应 > 0", tag, n);
    }
}
