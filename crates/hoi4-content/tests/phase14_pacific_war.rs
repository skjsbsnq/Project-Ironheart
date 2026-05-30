use hoi4_content::{
    load_scenario_content, ProgressSource, ScenarioContent, SituationDef, SituationEffect,
};

#[test]
fn phase14_pacific_war_content_loads_from_1936_manifest() {
    let content = load_scenario_content("1936").expect("1936 scenario should load");

    for event_id in [
        "japan.pacific_war_plan",
        "japan.pearl_harbor_strike",
        "japan.malaya_operation",
        "japan.philippines_operation",
        "japan.dutch_east_indies_operation",
        "usa.pearl_harbor_attacked",
        "usa.arsenal_of_democracy",
        "usa.island_hopping_strategy",
        "news.pearl_harbor",
        "news.japan_declares_southern_advance",
        "news.fall_of_singapore",
        "news.battle_of_midway",
        "news.guadalcanal_campaign",
    ] {
        assert!(
            content.events.find(event_id).is_some(),
            "missing Phase 14 event {event_id}"
        );
    }

    for situation_id in ["pacific_war", "southern_resource_area"] {
        assert!(
            content.situations.iter().any(|def| def.id == situation_id),
            "missing Phase 14 situation {situation_id}"
        );
    }
}

#[test]
fn pacific_war_starts_us_japan_and_commonwealth_war() {
    let content = load_scenario_content("1936").expect("1936 scenario should load");
    let situation = find_situation(&content, "pacific_war");

    assert_eq!(situation.start_date, (1941, 12, 7));
    assert_eq!(situation.end_date, (1946, 1, 1));
    assert!(has_trigger(&situation.on_start, "news.pearl_harbor"));
    assert!(has_trigger(
        &situation.on_start,
        "japan.pearl_harbor_strike"
    ));
    assert!(has_trigger(
        &situation.on_start,
        "usa.pearl_harbor_attacked"
    ));
    assert!(has_create_war(&situation.on_start, "JAP", "USA"));
    assert!(has_create_war(&situation.on_start, "JAP", "ENG"));
    assert!(has_create_faction(
        &situation.on_start,
        "USA",
        "United Nations"
    ));
    assert!(has_add_to_faction(&situation.on_start, "USA", "ENG"));

    for tag in ["AST", "NZL", "RAJ", "MAL", "PHI", "HOL", "INS"] {
        assert!(
            has_war_participant(&situation.on_start, "USA", tag),
            "missing Pacific Allied participant {tag}"
        );
    }

    assert!(has_flag(&situation.on_start, "JAP", "pacific_war_started"));
    assert!(has_flag(&situation.on_start, "USA", "pacific_war_started"));
    assert!(has_flag(
        &situation.on_start,
        "USA",
        "pearl_harbor_attacked"
    ));

    match &situation.progress_source {
        ProgressSource::TerritorialControl { side_country_tags } => {
            assert_eq!(side_country_tags[0], ["JAP", "MAN", "MEN"]);
            for tag in [
                "USA", "ENG", "AST", "NZL", "RAJ", "MAL", "PHI", "HOL", "INS",
            ] {
                assert!(
                    side_country_tags[1].contains(&tag.to_owned()),
                    "missing Pacific Allied side tag {tag}"
                );
            }
        }
        other => panic!("expected TerritorialControl, got {other:?}"),
    }
}

#[test]
fn southern_resource_area_sets_landing_targets_without_free_annexation() {
    let content = load_scenario_content("1936").expect("1936 scenario should load");
    let situation = find_situation(&content, "southern_resource_area");

    assert_eq!(situation.start_date, (1941, 12, 8));
    assert_eq!(situation.end_date, (1943, 1, 1));
    assert!(has_trigger(
        &situation.on_start,
        "news.japan_declares_southern_advance"
    ));
    assert!(has_trigger(&situation.on_start, "japan.malaya_operation"));
    assert!(has_trigger(
        &situation.on_start,
        "japan.philippines_operation"
    ));
    assert!(has_trigger(
        &situation.on_start,
        "japan.dutch_east_indies_operation"
    ));
    assert!(has_create_war(&situation.on_start, "JAP", "MAL"));
    assert!(has_create_war(&situation.on_start, "JAP", "PHI"));
    assert!(has_create_war(&situation.on_start, "JAP", "HOL"));
    assert!(has_war_participant(&situation.on_start, "HOL", "INS"));
    assert!(has_flag(
        &situation.on_start,
        "JAP",
        "jap_priority_malaya_singapore"
    ));
    assert!(has_flag(
        &situation.on_start,
        "JAP",
        "jap_priority_philippines"
    ));
    assert!(has_flag(
        &situation.on_start,
        "JAP",
        "jap_priority_dutch_east_indies"
    ));
    assert!(
        !situation.on_start.iter().any(|effect| matches!(
            effect,
            SituationEffect::TransferState { .. } | SituationEffect::AnnexCountry { .. }
        )),
        "Southern advance should rely on naval transport/landing systems, not instant annexation"
    );
}

fn find_situation<'a>(content: &'a ScenarioContent, id: &str) -> &'a SituationDef {
    content
        .situations
        .iter()
        .find(|def| def.id == id)
        .unwrap_or_else(|| panic!("missing situation {id}"))
}

fn has_create_war(effects: &[SituationEffect], attacker: &str, defender: &str) -> bool {
    effects.iter().any(|effect| {
        matches!(
            effect,
            SituationEffect::CreateWar { attacker: a, defender: d }
                if a == attacker && d == defender
        )
    })
}

fn has_create_faction(effects: &[SituationEffect], leader: &str, name: &str) -> bool {
    effects.iter().any(|effect| {
        matches!(
            effect,
            SituationEffect::CreateFaction { leader: l, name: n }
                if l == leader && n == name
        )
    })
}

fn has_add_to_faction(effects: &[SituationEffect], leader: &str, member: &str) -> bool {
    effects.iter().any(|effect| {
        matches!(
            effect,
            SituationEffect::AddToFaction { faction_leader, member: m }
                if faction_leader == leader && m == member
        )
    })
}

fn has_war_participant(effects: &[SituationEffect], war_leader: &str, participant: &str) -> bool {
    effects.iter().any(|effect| {
        matches!(
            effect,
            SituationEffect::AddWarParticipant { war_leader: l, participant: p }
                if l == war_leader && p == participant
        )
    })
}

fn has_flag(effects: &[SituationEffect], country: &str, flag: &str) -> bool {
    effects.iter().any(|effect| {
        matches!(
            effect,
            SituationEffect::SetCountryFlag { country: c, flag: f }
                if c == country && f == flag
        )
    })
}

fn has_trigger(effects: &[SituationEffect], event_id: &str) -> bool {
    effects
        .iter()
        .any(|effect| matches!(effect, SituationEffect::TriggerEvent(id) if id == event_id))
}
