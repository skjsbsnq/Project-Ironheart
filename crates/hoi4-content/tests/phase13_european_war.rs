use hoi4_content::{load_scenario_content, ProgressSource, SituationEffect};

#[test]
fn phase13_european_war_content_loads_from_1936_manifest() {
    let content = load_scenario_content("1936").expect("1936 scenario should load");

    for event_id in [
        "england.war_with_germany",
        "france.battle_for_france",
        "soviet.operation_barbarossa",
        "news.invasion_of_poland",
        "news.britain_france_declare_war",
        "news.weserubung",
        "news.battle_of_france_begins",
        "news.italy_enters_war",
        "news.greece_invaded",
        "news.operation_barbarossa",
    ] {
        assert!(
            content.events.find(event_id).is_some(),
            "missing Phase 13 event {event_id}"
        );
    }

    for situation_id in [
        "poland_campaign",
        "weserubung_and_western_campaign",
        "mediterranean_war",
        "greco_italian_war",
        "barbarossa",
    ] {
        assert!(
            content.situations.iter().any(|def| def.id == situation_id),
            "missing Phase 13 situation {situation_id}"
        );
    }
}

#[test]
fn poland_campaign_starts_european_war_without_scripted_partition() {
    let content = load_scenario_content("1936").expect("1936 scenario should load");
    let situation = find_situation(&content, "poland_campaign");

    assert_eq!(situation.start_date, (1939, 9, 1));
    assert_eq!(situation.end_date, (1940, 1, 1));
    assert!(has_trigger(&situation.on_start, "news.invasion_of_poland"));
    assert!(has_create_war(&situation.on_start, "GER", "POL"));
    assert!(has_create_faction(&situation.on_start, "ENG", "Allies"));
    assert!(has_add_to_faction(&situation.on_start, "ENG", "FRA"));
    assert!(has_war_participant(&situation.on_start, "POL", "ENG"));
    assert!(has_war_participant(&situation.on_start, "POL", "FRA"));
    assert!(has_flag(&situation.on_start, "GER", "polish_campaign"));
    assert!(has_flag(&situation.on_start, "ENG", "ww2_started"));
    assert!(has_flag(&situation.on_start, "FRA", "ww2_started"));
    assert!(
        !situation.on_start.iter().any(|effect| matches!(
            effect,
            SituationEffect::TransferState { .. } | SituationEffect::AnnexCountry { .. }
        )),
        "Poland campaign start should create a playable war, not immediately partition Poland"
    );

    match &situation.progress_source {
        ProgressSource::TerritorialControl { side_country_tags } => {
            assert_eq!(side_country_tags[0], ["GER"]);
            assert_eq!(side_country_tags[1], ["POL", "ENG", "FRA"]);
        }
        other => panic!("expected TerritorialControl, got {other:?}"),
    }
}

#[test]
fn barbarossa_creates_long_running_eastern_front() {
    let content = load_scenario_content("1936").expect("1936 scenario should load");
    let situation = find_situation(&content, "barbarossa");

    assert_eq!(situation.start_date, (1941, 6, 22));
    assert_eq!(situation.end_date, (1945, 1, 1));
    assert!(has_trigger(
        &situation.on_start,
        "news.operation_barbarossa"
    ));
    assert!(has_create_war(&situation.on_start, "GER", "SOV"));
    assert!(has_war_participant(&situation.on_start, "GER", "ITA"));
    assert!(has_war_participant(&situation.on_start, "GER", "ROM"));
    assert!(has_flag(&situation.on_start, "GER", "barbarossa_started"));
    assert!(has_flag(&situation.on_start, "SOV", "great_patriotic_war"));

    match &situation.progress_source {
        ProgressSource::TerritorialControl { side_country_tags } => {
            assert_eq!(side_country_tags[0], ["GER", "ITA", "ROM"]);
            assert_eq!(side_country_tags[1], ["SOV"]);
        }
        other => panic!("expected TerritorialControl, got {other:?}"),
    }
}

fn find_situation<'a>(
    content: &'a hoi4_content::ScenarioContent,
    id: &str,
) -> &'a hoi4_content::SituationDef {
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
