use hoi4_content::{
    load_scenario_content, Effect, Event, ProgressSource, SituationEffect, Trigger,
};

#[test]
fn phase12_sino_japanese_war_content_loads_from_1936_manifest() {
    let content = load_scenario_content("1936").expect("1936 scenario should load");

    assert!(content
        .events
        .find("news.marco_polo_bridge_incident")
        .is_some());
    assert!(content
        .events
        .find("japan.china_incident_escalated")
        .is_some());
    assert!(content.events.find("china.united_front_forms").is_some());
    assert!(content.events.find("japan.north_china_campaign").is_some());
    assert!(content.events.find("japan.shandong_campaign").is_some());
    assert!(content
        .events
        .find("japan.shandong_campaign_auto")
        .is_some());
    assert!(content.events.find("china.battle_of_shanghai").is_some());
    assert!(content.events.find("china.defense_of_nanjing").is_some());
    assert!(content
        .events
        .find("japan.shanghai_campaign_auto")
        .is_some());
    assert!(content.events.find("japan.nanjing_drive_auto").is_some());
    assert!(content.events.find("news.jinan_under_threat").is_some());
    assert!(content.events.find("news.fall_of_jinan").is_some());
    assert!(content
        .events
        .find("news.battle_of_shanghai_begins")
        .is_some());
    assert!(content.events.find("news.fall_of_nanjing").is_some());
    assert!(content
        .events
        .find("news.sino_japanese_war_chongqing_bombing")
        .is_some());

    let situation = content
        .situations
        .iter()
        .find(|def| def.id == "second_sino_japanese_war")
        .expect("second_sino_japanese_war situation");

    assert_eq!(situation.start_date, (1937, 7, 7));
    assert_eq!(situation.end_date, (1946, 1, 1));
    assert_eq!(situation.sides.len(), 2);
    assert_eq!(
        situation.sides[0].default_supporters,
        ["JAP", "MAN", "MEN", "HBC"]
    );
    assert!(situation.sides[1]
        .default_supporters
        .contains(&"CHI".to_owned()));
    assert!(situation.sides[1]
        .default_supporters
        .contains(&"SND".to_owned()));
    assert!(situation.sides[1]
        .default_supporters
        .contains(&"PRC".to_owned()));
    assert!(situation.sides[1]
        .default_supporters
        .contains(&"SIC".to_owned()));

    match &situation.progress_source {
        ProgressSource::TerritorialControl { side_country_tags } => {
            assert_eq!(side_country_tags[0], ["JAP", "MAN", "MEN", "HBC"]);
            for tag in [
                "CHI", "SND", "PRC", "SHX", "GXC", "GDC", "YUN", "XAJ", "SIC", "XSM", "SIK",
            ] {
                assert!(
                    side_country_tags[1].contains(&tag.to_owned()),
                    "missing Chinese side tag {tag}"
                );
            }
        }
        other => panic!("expected TerritorialControl, got {other:?}"),
    }
}

#[test]
fn phase12_shandong_campaign_auto_unblocks_ai_after_opening_month() {
    let content = load_scenario_content("1936").expect("1936 scenario should load");
    let event = content
        .events
        .find("japan.shandong_campaign_auto")
        .expect("hidden Shandong campaign auto event");

    assert!(!event.is_triggered_only);
    assert!(event.hidden);
    assert_eq!(event.mean_time_to_happen_days, 0);
    assert!(trigger_any(&event.trigger, &|trigger| {
        matches!(trigger, Trigger::Tag(tag) if tag == "JAP")
    }));
    assert!(trigger_any(&event.trigger, &|trigger| {
        matches!(trigger, Trigger::HasCountryFlag(flag) if flag == "china_incident_escalated")
    }));
    assert!(trigger_any(&event.trigger, &|trigger| {
        matches!(trigger, Trigger::HasWarWith(tag) if tag == "CHI")
    }));
    assert!(trigger_any(&event.trigger, &|trigger| {
        matches!(
            trigger,
            Trigger::Date {
                year: 1937,
                month: 8,
                day: 1,
            }
        )
    }));
    assert!(trigger_any(&event.trigger, &|trigger| {
        matches!(
            trigger,
            Trigger::Not(inner)
                if matches!(
                    inner.as_ref(),
                    Trigger::HasCountryFlag(flag) if flag == "jap_priority_shandong"
                )
        )
    }));
    assert!(event_sets_flag(event, "jap_priority_shandong"));
}

#[test]
fn phase12_historical_campaign_stage_autos_unblock_shanghai_and_nanjing() {
    let content = load_scenario_content("1936").expect("1936 scenario should load");
    let shanghai = content
        .events
        .find("japan.shanghai_campaign_auto")
        .expect("hidden Shanghai campaign auto event");

    assert!(!shanghai.is_triggered_only);
    assert!(shanghai.hidden);
    assert_eq!(shanghai.mean_time_to_happen_days, 0);
    assert!(trigger_any(&shanghai.trigger, &|trigger| {
        matches!(
            trigger,
            Trigger::Date {
                year: 1937,
                month: 8,
                day: 13,
            }
        )
    }));
    assert!(trigger_any(&shanghai.trigger, &|trigger| {
        matches!(trigger, Trigger::HasCountryFlag(flag) if flag == "jap_priority_shandong")
    }));
    assert!(event_sets_flag(shanghai, "battle_of_shanghai_fired"));
    assert!(event_sets_flag(shanghai, "jap_priority_shanghai_nanjing"));
    assert!(event_triggers(shanghai, "news.battle_of_shanghai_begins"));

    let nanjing = content
        .events
        .find("japan.nanjing_drive_auto")
        .expect("hidden Nanjing drive auto event");

    assert!(!nanjing.is_triggered_only);
    assert!(nanjing.hidden);
    assert_eq!(nanjing.mean_time_to_happen_days, 0);
    assert!(trigger_any(&nanjing.trigger, &|trigger| {
        matches!(
            trigger,
            Trigger::Date {
                year: 1937,
                month: 11,
                day: 15,
            }
        )
    }));
    assert!(trigger_any(&nanjing.trigger, &|trigger| {
        matches!(trigger, Trigger::HasCountryFlag(flag) if flag == "jap_priority_shanghai_nanjing")
    }));
    assert!(event_sets_flag(nanjing, "jap_advance_on_nanjing"));
    assert!(event_triggers(nanjing, "news.nanjing_under_threat"));
}

#[test]
fn phase12_sino_japanese_war_on_start_has_required_effects() {
    let content = load_scenario_content("1936").expect("1936 scenario should load");
    let situation = content
        .situations
        .iter()
        .find(|def| def.id == "second_sino_japanese_war")
        .expect("second_sino_japanese_war situation");

    assert!(has_create_war(&situation.on_start, "JAP", "CHI"));
    assert!(has_create_faction(
        &situation.on_start,
        "CHI",
        "中国统一战线"
    ));
    assert!(has_trigger(
        &situation.on_start,
        "news.marco_polo_bridge_incident"
    ));
    assert!(
        !has_trigger(&situation.on_start, "japan.china_incident_escalated")
            && !has_trigger(&situation.on_start, "china.united_front_forms")
            && !has_trigger(&situation.on_start, "china.regional_command_autonomy")
            && !has_trigger(&situation.on_start, "japan.north_china_campaign")
            && !has_trigger(&situation.on_start, "china.defense_of_north_china")
            && !has_trigger(&situation.on_start, "japan.shandong_campaign")
            && !has_trigger(&situation.on_start, "china.defense_of_shandong"),
        "opening the Sino-Japanese War should only queue news, not country event popups"
    );
    assert!(has_flag(
        &situation.on_start,
        "JAP",
        "china_incident_escalated"
    ));
    assert!(has_flag(
        &situation.on_start,
        "JAP",
        "jap_priority_north_china"
    ));
    assert!(
        !has_flag(&situation.on_start, "JAP", "jap_priority_shandong"),
        "Shandong should not be forced on the opening day; Japan should start in North China"
    );
    assert!(has_flag(
        &situation.on_start,
        "CHI",
        "anti_japanese_united_front"
    ));
    assert!(has_flag(
        &situation.on_start,
        "CHI",
        "regional_command_autonomy"
    ));
    assert!(has_flag(
        &situation.on_start,
        "CHI",
        "defense_of_north_china"
    ));
    assert!(has_flag(&situation.on_start, "CHI", "defense_of_shandong"));

    for tag in ["MAN", "MEN", "HBC"] {
        assert!(
            has_war_participant(&situation.on_start, "JAP", tag),
            "missing Japanese puppet participant {tag}"
        );
    }

    for tag in [
        "SND", "PRC", "SHX", "GXC", "GDC", "YUN", "XAJ", "SIC", "XSM", "SIK",
    ] {
        assert!(
            has_add_to_faction(&situation.on_start, "CHI", tag),
            "missing United Front faction member {tag}"
        );
        assert!(
            has_access(&situation.on_start, "CHI", tag),
            "missing CHI access grant to {tag}"
        );
        assert!(
            has_access(&situation.on_start, tag, "CHI"),
            "missing {tag} access grant to CHI"
        );
    }

    for tag in ["SND", "SHX"] {
        assert!(
            has_war_participant(&situation.on_start, "CHI", tag),
            "missing initial Chinese war participant {tag}"
        );
    }

    for tag in ["PRC", "GXC", "GDC", "YUN", "XAJ", "SIC", "XSM", "SIK"] {
        assert!(
            !has_war_participant(&situation.on_start, "CHI", tag),
            "{tag} should join the faction but not the war on 1937-07-07"
        );
        assert!(
            has_war_join_policy(&situation.on_start, "CHI", tag, "Delayed"),
            "{tag} should have delayed war join policy"
        );
    }
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

fn has_war_join_policy(
    effects: &[SituationEffect],
    war_leader: &str,
    country: &str,
    policy: &str,
) -> bool {
    effects.iter().any(|effect| {
        matches!(
            effect,
            SituationEffect::SetWarJoinPolicy { war_leader: l, country: c, policy: p }
                if l == war_leader && c == country && p == policy
        )
    })
}

fn has_access(effects: &[SituationEffect], grantor: &str, grantee: &str) -> bool {
    effects.iter().any(|effect| {
        matches!(
            effect,
            SituationEffect::GrantMilitaryAccess { grantor: a, grantee: b }
                if a == grantor && b == grantee
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

fn event_sets_flag(event: &Event, flag: &str) -> bool {
    event
        .immediate
        .iter()
        .chain(
            event
                .options
                .iter()
                .flat_map(|option| option.effects.iter()),
        )
        .any(|effect| matches!(effect, Effect::SetCountryFlag(name) if name == flag))
}

fn event_triggers(event: &Event, event_id: &str) -> bool {
    event
        .immediate
        .iter()
        .chain(
            event
                .options
                .iter()
                .flat_map(|option| option.effects.iter()),
        )
        .any(|effect| matches!(effect, Effect::TriggerEvent(id) if id == event_id))
}

fn trigger_any<F>(trigger: &Trigger, predicate: &F) -> bool
where
    F: Fn(&Trigger) -> bool,
{
    if predicate(trigger) {
        return true;
    }

    match trigger {
        Trigger::And(children) | Trigger::Or(children) => {
            children.iter().any(|child| trigger_any(child, predicate))
        }
        Trigger::Not(inner) => trigger_any(inner, predicate),
        _ => false,
    }
}
