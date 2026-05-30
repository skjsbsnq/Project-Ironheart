pub fn load_scenario_content(scenario: &str) -> hoi4_content::ScenarioContent {
    hoi4_content::load_scenario_content(scenario)
        .unwrap_or_else(|error| panic!("failed to load scenario content '{scenario}': {error}"))
}

pub fn build_situation_state(
    scenario_content: &hoi4_content::ScenarioContent,
) -> hoi4_content::SituationState {
    let mut state = hoi4_content::SituationState::new();
    for def in &scenario_content.situations {
        state.add_def(def.clone());
    }
    state
}
