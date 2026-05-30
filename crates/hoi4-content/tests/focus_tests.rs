#[cfg(test)]
mod tests {
    use hoi4_content::focus::*;
    use hoi4_content::validate::*;

    fn minimal_focus(id: &str, pos: (i32, i32)) -> Focus {
        Focus {
            id: id.to_owned(),
            name: format!("FOCUS_{}", id.to_uppercase()),
            icon: "GFX_focus_generic".to_owned(),
            position: pos,
            cost_days: 70,
            prerequisites: vec![],
            mutually_exclusive: vec![],
            available: Trigger::AlwaysTrue,
            completion_effect: vec![],
        }
    }

    // ─── Parsing ──

    #[test]
    fn parse_minimal_tree() {
        let ron = r#"
FocusTree(
    country: "GER",
    focuses: [
        Focus(
            id: "GER_rhineland",
            name: "FOCUS_GER_RHINELAND",
            icon: "GFX_focus_generic_anschluss",
            position: (0, 0),
            cost_days: 70,
            available: AlwaysTrue,
        ),
    ],
)
"#;
        let tree = FocusTree::from_ron(ron).unwrap();
        assert_eq!(tree.country, "GER");
        assert_eq!(tree.focuses.len(), 1);
        assert_eq!(tree.focuses[0].id, "GER_rhineland");
    }

    #[test]
    fn parse_trigger_variants() {
        let ron = r#"
FocusTree(
    country: "GER",
    focuses: [
        Focus(
            id: "test",
            name: "TEST",
            icon: "GFX_test",
            position: (0, 0),
            cost_days: 70,
            available: And([
                HasCompletedFocus("GER_rhineland"),
                Or([HasGovernment("fascism"), HasWar(true)]),
                Not(HasCountryFlag("peace")),
                Date(year: 1938, month: 3, day: 1),
                NumDivisions(50),
                IsInFactionWith("ITA"),
            ]),
        ),
    ],
)
"#;
        let tree = FocusTree::from_ron(ron).unwrap();
        match &tree.focuses[0].available {
            Trigger::And(v) => assert_eq!(v.len(), 6),
            _ => panic!("expected And"),
        }
    }

    #[test]
    fn parse_effects_expanded() {
        let ron = r#"
FocusTree(
    country: "GER",
    focuses: [
        Focus(
            id: "test",
            name: "TEST",
            icon: "GFX_test",
            position: (0, 0),
            cost_days: 70,
            completion_effect: [
                AddPoliticalPower(25.0),
                AddStability(0.05),
                SetCountryFlag("done"),
                AddResearchSlot(1),
                SwapIdea(remove: "old", add: "new"),
                TransferState(50),
                AddCoreTo(state: 50, country: "GER"),
                If(trigger: HasWar(true), effects: [AddWarSupport(0.1)]),
            ],
        ),
    ],
)
"#;
        let tree = FocusTree::from_ron(ron).unwrap();
        assert_eq!(tree.focuses[0].completion_effect.len(), 8);
    }

    #[test]
    fn roundtrip_serialize() {
        let tree = FocusTree {
            country: "GER".to_owned(),
            focuses: vec![minimal_focus("GER_rhineland", (0, 0))],
        };
        let s = tree.to_ron().unwrap();
        let parsed = FocusTree::from_ron(&s).unwrap();
        assert_eq!(parsed.focuses[0].id, "GER_rhineland");
    }

    // ─── Validation ──

    #[test]
    fn validate_empty_tree() {
        let tree = FocusTree {
            country: "GER".to_owned(),
            focuses: vec![],
        };
        let errs = validate_focus_tree(&tree);
        assert_eq!(errs, vec![ValidationError::EmptyTree]);
    }

    #[test]
    fn validate_valid_tree() {
        let mut f1 = minimal_focus("a", (0, 0));
        let mut f2 = minimal_focus("b", (1, 0));
        f2.prerequisites = vec![vec!["a".to_owned()]];
        f1.mutually_exclusive = vec!["b".to_owned()];
        f2.mutually_exclusive = vec!["a".to_owned()];
        let tree = FocusTree {
            country: "GER".to_owned(),
            focuses: vec![f1, f2],
        };
        assert!(validate_focus_tree(&tree).is_empty());
    }

    #[test]
    fn validate_duplicate_id() {
        let tree = FocusTree {
            country: "GER".to_owned(),
            focuses: vec![minimal_focus("a", (0, 0)), minimal_focus("a", (1, 0))],
        };
        assert!(validate_focus_tree(&tree)
            .iter()
            .any(|e| matches!(e, ValidationError::DuplicateId(_))));
    }

    #[test]
    fn validate_missing_prerequisite() {
        let mut f = minimal_focus("a", (0, 0));
        f.prerequisites = vec![vec!["ghost".to_owned()]];
        let tree = FocusTree {
            country: "GER".to_owned(),
            focuses: vec![f],
        };
        assert!(validate_focus_tree(&tree)
            .iter()
            .any(|e| matches!(e, ValidationError::MissingPrerequisite { .. })));
    }

    #[test]
    fn validate_cyclic_prerequisites() {
        let mut f1 = minimal_focus("a", (0, 0));
        let mut f2 = minimal_focus("b", (1, 0));
        f1.prerequisites = vec![vec!["b".to_owned()]];
        f2.prerequisites = vec![vec!["a".to_owned()]];
        let tree = FocusTree {
            country: "GER".to_owned(),
            focuses: vec![f1, f2],
        };
        assert!(validate_focus_tree(&tree)
            .iter()
            .any(|e| matches!(e, ValidationError::CyclicPrerequisite(_))));
    }

    #[test]
    fn validate_trigger_refs_missing() {
        let ids: std::collections::HashSet<&str> = ["a", "b"].into_iter().collect();
        let trigger = Trigger::And(vec![
            Trigger::HasCompletedFocus("a".to_owned()),
            Trigger::HasCompletedFocus("nonexistent".to_owned()),
        ]);
        let missing = validate_trigger_refs(&trigger, &ids);
        assert_eq!(missing, vec!["nonexistent".to_owned()]);
    }
}
