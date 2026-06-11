use hoi4_content::{load_scenario_content, DecisionCategory, Effect};

#[test]
fn spanish_prewar_decisions_load_from_1936_manifest() {
    let content = load_scenario_content("1936").expect("1936 scenario should load");
    let expected = [
        "spr.prewar_rebalance_civil_governors",
        "spr.prewar_monitor_mola_network",
        "spr.prewar_legalize_land_committees",
        "spr.prewar_guard_church_property",
        "spr.prewar_prepare_armory_keys",
    ];

    for id in expected {
        let decision = content
            .decisions
            .find(id)
            .expect("missing SPR prewar decision");
        assert_eq!(decision.category, DecisionCategory::Crisis);
        assert!(decision.id.starts_with("spr.prewar_"));
        assert!(decision.days_re_enable > 0);
        assert!(
            decision.on_complete.iter().any(|effect| matches!(
                effect,
                Effect::AddToVariable { name, .. } if name.starts_with("spr_prewar_")
            )),
            "{id} should update the prewar minigame variables"
        );
    }
}
