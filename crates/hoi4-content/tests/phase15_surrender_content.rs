use hoi4_content::{load_scenario_content, SituationEffect};

#[test]
fn phase15_surrender_content_loads_from_1936_manifest() {
    let content = load_scenario_content("1936").expect("1936 scenario should load");

    for event_id in [
        "news.france_surrenders",
        "news.italy_surrenders",
        "news.germany_surrenders",
        "news.japan_surrenders",
    ] {
        assert!(
            content.events.find(event_id).is_some(),
            "missing {event_id}"
        );
    }

    for id in [
        "fra_surrender",
        "ita_surrender",
        "ger_surrender",
        "jap_surrender",
    ] {
        assert!(
            content.surrenders.iter().any(|def| def.id == id),
            "missing surrender def {id}"
        );
    }
}

#[test]
fn france_surrender_has_armistice_shape() {
    let content = load_scenario_content("1936").expect("1936 scenario should load");
    let def = content
        .surrenders
        .iter()
        .find(|def| def.id == "fra_surrender")
        .expect("fra_surrender");

    assert_eq!(def.target, "FRA");
    assert_eq!(def.enemy_side, ["GER", "ITA"]);
    assert!(def.require_capital_lost);
    assert!(def.remove_from_wars);
    assert!(def.max_target_core_control_ratio <= 0.35);
    assert!(has_trigger(&def.effects, "news.france_surrenders"));
    assert!(has_flag(&def.effects, "FRA", "surrendered_to_axis"));
    assert!(has_flag(&def.effects, "GER", "france_defeated"));
    assert!(
        !def.effects.iter().any(|effect| matches!(
            effect,
            SituationEffect::AnnexCountry { .. } | SituationEffect::TransferState { .. }
        )),
        "France surrender MVP should not hard-annex or hard-transfer states"
    );
}

fn has_trigger(effects: &[SituationEffect], event_id: &str) -> bool {
    effects
        .iter()
        .any(|effect| matches!(effect, SituationEffect::TriggerEvent(id) if id == event_id))
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
