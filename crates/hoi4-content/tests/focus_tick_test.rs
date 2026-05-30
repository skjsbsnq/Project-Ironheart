//! Tests for focus_tick module.
#[cfg(test)]
mod tests {
    use hoi4_content::eval::GlobalFlags;
    use hoi4_content::focus::*;
    use hoi4_content::focus_tick::*;
    use hoi4_state::CountryId;

    // Reuse the test_world from eval.rs via a minimal inline version
    fn test_world() -> hoi4_state::World {
        use hoi4_state::store::*;
        use hoi4_state::*;
        use std::collections::HashMap;
        use std::sync::Arc;

        let map = Arc::new(hoi4_map::GameMap {
            definitions: vec![],
            rgb_to_id: HashMap::new(),
            province_map: hoi4_map::ProvinceMap {
                width: 1,
                height: 1,
                pixels: vec![0],
            },
            adjacencies: vec![],
            special_adjacencies: vec![],
            heightmap: hoi4_map::Heightmap {
                width: 1,
                height: 1,
                pixels: vec![0],
            },
            terrain_bmp: hoi4_map::TerrainBitmap {
                width: 1,
                height: 1,
                pixels: vec![0],
                palette: [[0u8; 3]; 256],
            },
            terrain_catalog: hoi4_map::TerrainCatalog::default(),
            tree_definition_bmp: None,
            tree_indices: std::collections::HashSet::new(),
        });
        let data = Arc::new(hoi4_data::GameData {
            countries: HashMap::new(),
            states: vec![],
            province_owners: HashMap::new(),
            buildings: HashMap::new(),
            resources: HashMap::new(),
            equipment: HashMap::new(),
            technologies: HashMap::new(),
            tech_prereqs: HashMap::new(),
            ideologies: HashMap::new(),
            ideas: HashMap::new(),
            focus_trees: HashMap::new(),
            focus_to_tree: HashMap::new(),
            subunits: HashMap::new(),
            combat_tactics: HashMap::new(),
            division_templates: HashMap::new(),
            ship_classes: HashMap::new(),
            aircraft: HashMap::new(),
            oob_land: HashMap::new(),
            oob_naval: HashMap::new(),
            oob_air: HashMap::new(),
            country_histories: HashMap::new(),
            decision_categories: HashMap::new(),
            decisions: HashMap::new(),
            ..Default::default()
        });
        let mut countries = CountryStore::new(1);
        countries.tags[0] = "GER".to_owned();
        countries.political_power[0] = 100.0;
        countries.stability[0] = 0.5;
        let mut tag_to_country = HashMap::new();
        tag_to_country.insert("GER".to_owned(), CountryId(0));
        World {
            date: GameDate::START,
            speed: GameSpeed::Paused,
            elapsed_hours: 0,
            provinces: ProvinceStore::new(0),
            states: StateStore::new(0),
            countries,
            divisions: DivisionStore::new(),
            ships: ShipStore::new(),
            fleets: FleetStore::new(),
            air_wings: AirWingStore::new(),
            diplomacy: DiplomacyState::new(),
            command: hoi4_state::command::CommandHierarchy::default(),
            map,
            data,
            tag_to_country,
            state_id_lookup: HashMap::new(),
            player: CountryId(0),
            random_seed: 0,
            game_unique_id: 0,
            path_cache: HashMap::new(),
            path_cache_day: 0,
            prov_div_index: HashMap::new(),
            country_state_index: Vec::new(),
            country_pop_index: Vec::new(),
            country_building_index: Vec::new(),
            country_division_index: Vec::new(),
            country_fleet_index: Vec::new(),
            country_air_wing_index: Vec::new(),
            runtime_country_indexes_valid: false,
            trade_export_surplus_index: HashMap::new(),
            player_armies: Vec::new(),
            player_locked_divisions: std::collections::HashSet::new(),
            next_army_id: 0,
            generals: Vec::new(),
            next_general_id: 0,
        }
    }

    fn mini_tree() -> FocusTree {
        FocusTree {
            country: "GER".to_owned(),
            focuses: vec![
                Focus {
                    id: "a".into(),
                    name: "A".into(),
                    icon: "".into(),
                    position: (0, 0),
                    cost_days: 3,
                    prerequisites: vec![],
                    mutually_exclusive: vec![],
                    available: Trigger::AlwaysTrue,
                    completion_effect: vec![Effect::AddPoliticalPower(50.0)],
                },
                Focus {
                    id: "b".into(),
                    name: "B".into(),
                    icon: "".into(),
                    position: (1, 0),
                    cost_days: 2,
                    prerequisites: vec![vec!["a".into()]],
                    mutually_exclusive: vec![],
                    available: Trigger::AlwaysTrue,
                    completion_effect: vec![Effect::AddStability(0.1)],
                },
            ],
        }
    }

    #[test]
    fn tick_idle_when_no_focus() {
        let mut w = test_world();
        let mut f = GlobalFlags::default();
        let tree = mini_tree();
        assert_eq!(
            daily_focus_tick(&mut w, CountryId(0), &tree, &mut f, 0.0),
            TickResult::Idle
        );
    }

    #[test]
    fn start_and_complete_focus() {
        let mut w = test_world();
        let mut f = GlobalFlags::default();
        let tree = mini_tree();
        let ger = CountryId(0);

        assert!(start_focus(&mut w, ger, &tree, "a", &f));
        assert_eq!(w.countries.current_focus[0], Some("a".to_owned()));

        // Tick 3 days
        assert_eq!(
            daily_focus_tick(&mut w, ger, &tree, &mut f, 0.0),
            TickResult::InProgress {
                id: "a".into(),
                progress: 1.0,
                cost: 3
            }
        );
        assert_eq!(
            daily_focus_tick(&mut w, ger, &tree, &mut f, 0.0),
            TickResult::InProgress {
                id: "a".into(),
                progress: 2.0,
                cost: 3
            }
        );
        assert_eq!(
            daily_focus_tick(&mut w, ger, &tree, &mut f, 0.0),
            TickResult::Completed("a".into())
        );

        // PP should have increased by 50
        assert_eq!(w.countries.political_power[0], 150.0);
        // Focus marked completed
        assert!(w.countries.completed_focuses[0].contains("a"));
        assert_eq!(w.countries.current_focus[0], None);
    }

    #[test]
    fn cannot_start_without_prereqs() {
        let mut w = test_world();
        let f = GlobalFlags::default();
        let tree = mini_tree();
        // "b" requires "a" completed
        assert!(!start_focus(&mut w, CountryId(0), &tree, "b", &f));
    }

    #[test]
    fn can_start_after_prereq_done() {
        let mut w = test_world();
        let f = GlobalFlags::default();
        let tree = mini_tree();
        let ger = CountryId(0);
        w.countries.completed_focuses[0].insert("a".to_owned());
        assert!(start_focus(&mut w, ger, &tree, "b", &f));
    }

    #[test]
    fn cannot_start_twice() {
        let mut w = test_world();
        let f = GlobalFlags::default();
        let tree = mini_tree();
        let ger = CountryId(0);
        assert!(start_focus(&mut w, ger, &tree, "a", &f));
        assert!(!start_focus(&mut w, ger, &tree, "a", &f)); // already active
    }
}
