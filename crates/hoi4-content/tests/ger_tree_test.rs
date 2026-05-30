#[test]
fn load_and_validate_ger_focus_tree() {
    let ron_str = include_str!("../content/GER_focus_tree.ron");
    let tree = hoi4_content::FocusTree::from_ron(ron_str).expect("GER_focus_tree.ron should parse");
    assert_eq!(tree.country, "GER");
    assert!(
        tree.focuses.len() >= 30,
        "got {} focuses",
        tree.focuses.len()
    );

    let errors = hoi4_content::validate_focus_tree(&tree);
    assert!(errors.is_empty(), "validation errors: {:?}", errors);
}

#[test]
fn h6_ger_focus_tree_uses_v7_economic_effects() {
    let ron_str = include_str!("../content/GER_focus_tree.ron");
    let tree = hoi4_content::FocusTree::from_ron(ron_str).expect("GER_focus_tree.ron should parse");

    let mut has_change_law = false;
    let mut has_government_order = false;
    let mut has_v7_building = false;

    for focus in &tree.focuses {
        for effect in &focus.completion_effect {
            match effect {
                hoi4_content::Effect::AddBuildingInState { building, .. }
                | hoi4_content::Effect::AddBuildingAllStates { building, .. } => {
                    assert!(
                        !matches!(
                            building.as_str(),
                            "industrial_complex" | "arms_factory" | "dockyard"
                        ),
                        "{} still uses legacy factory effect {}",
                        focus.id,
                        building
                    );
                }
                hoi4_content::Effect::ChangeLaw { .. } => has_change_law = true,
                hoi4_content::Effect::AddGovernmentOrder { .. } => has_government_order = true,
                hoi4_content::Effect::AddBuildingLevel { .. } => has_v7_building = true,
                _ => {}
            }
        }
    }

    assert!(
        has_change_law,
        "GER focus tree should change laws via V7 effects"
    );
    assert!(
        has_government_order,
        "GER focus tree should create V7 government orders"
    );
    assert!(
        has_v7_building,
        "GER focus tree should use V7 building effects"
    );
}
