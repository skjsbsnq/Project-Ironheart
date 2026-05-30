//! V5 阶段 F.1：校验 `GER_events.ron` 解析 + 校验通过 + 满足规模要求。

use hoi4_content::EventDb;

#[test]
fn ger_events_parse_and_validate() {
    let ron = include_str!("../content/GER_events.ron");
    let db = EventDb::from_ron(ron).expect("parse GER_events.ron");

    let errors = db.validate();
    if !errors.is_empty() {
        for e in &errors {
            eprintln!("validation error: {e}");
        }
        panic!("GER_events.ron validation failed");
    }

    // F.1 路线图要求：30-50 个事件。1936 扩充后短期内可能 ≤ 60，
    // 留下 1937-1939 / F.2 决议 / F.3 AI 各阶段的扩展余地。
    assert!(
        db.events.len() >= 30,
        "F.1 requires >= 30 events, found {}",
        db.events.len()
    );
    assert!(
        db.events.len() <= 60,
        "F.1 soft cap at 60 events, found {}",
        db.events.len()
    );

    // 关键历史节点必须存在
    for required in [
        "germany.rhineland",
        "germany.spanish_civil_war",
        "germany.anschluss",
        "germany.munich_agreement",
        "germany.kristallnacht",
        "germany.molotov_ribbentrop",
        "germany.poland_war",
    ] {
        assert!(
            db.find(required).is_some(),
            "missing required event: {required}"
        );
    }
}

#[test]
fn ger_events_all_have_at_least_one_option() {
    let ron = include_str!("../content/GER_events.ron");
    let db = EventDb::from_ron(ron).expect("parse");
    for e in &db.events {
        assert!(!e.options.is_empty(), "event {} has no options", e.id);
    }
}

#[test]
fn ger_events_distribution_per_year() {
    use hoi4_content::Trigger;
    let ron = include_str!("../content/GER_events.ron");
    let db = EventDb::from_ron(ron).expect("parse");

    // Heuristic: count events whose Trigger directly mentions Date(year=...)
    // Helps catch huge year-skew (all events in 1939 = bug).
    fn first_year(t: &Trigger) -> Option<u16> {
        match t {
            Trigger::Date { year, .. } => Some(*year),
            Trigger::And(v) | Trigger::Or(v) => v.iter().find_map(first_year),
            Trigger::Not(b) => first_year(b),
            _ => None,
        }
    }

    let mut count_1936 = 0u32;
    let mut count_1937 = 0u32;
    let mut count_1938 = 0u32;
    let mut count_1939 = 0u32;
    for e in &db.events {
        match first_year(&e.trigger) {
            Some(1936) => count_1936 += 1,
            Some(1937) => count_1937 += 1,
            Some(1938) => count_1938 += 1,
            Some(1939) => count_1939 += 1,
            _ => {}
        }
    }

    // 1936 现在覆盖 7+ 个核心日期点（莱茵兰/公投/Stresa/Himmler/Wehrgesetz/
    // He-111/Madrid/HJ/Olympics/Spain/Axis/Anti-Comintern/DNVP/Rally）—— 至少 5。
    // 其余年份至少 2 个 date-pinned 事件，保证起码的历史节奏。
    assert!(
        count_1936 >= 5,
        "expected >=5 1936 events, got {count_1936}"
    );
    assert!(
        count_1937 >= 2,
        "expected >=2 1937 events, got {count_1937}"
    );
    assert!(
        count_1938 >= 2,
        "expected >=2 1938 events, got {count_1938}"
    );
    assert!(
        count_1939 >= 2,
        "expected >=2 1939 events, got {count_1939}"
    );
}
