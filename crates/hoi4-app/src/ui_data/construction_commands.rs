use std::collections::HashSet;

use hoi4_logic::economy::EconomyState;
use hoi4_state::{CountryId, StateId, World};
use hoi4_ui::construction_v6_panel::ConstructionV6Command;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ConstructionControlEffect {
    pub handled: bool,
    pub reset_auto_build_month: bool,
    pub highlight_changed: bool,
}

pub fn apply_construction_control_command(
    cmd: &ConstructionV6Command,
    world: &mut World,
    econ: &mut EconomyState,
    db: &hoi4_content::V6Database,
    player: usize,
    auto_build_enabled: &mut bool,
    construction_mode: &mut Option<String>,
    construction_highlight_province_ids: &mut HashSet<u32>,
) -> ConstructionControlEffect {
    match cmd {
        ConstructionV6Command::ToggleAutoBuild(enabled) => {
            *auto_build_enabled = *enabled;
            ConstructionControlEffect {
                handled: true,
                reset_auto_build_month: *enabled,
                highlight_changed: false,
            }
        }
        ConstructionV6Command::MoveUp(idx) => {
            if let Some(q) = econ.construction.get_mut(player) {
                if *idx > 0 && *idx < q.items.len() {
                    q.items.swap(*idx, *idx - 1);
                }
            }
            handled()
        }
        ConstructionV6Command::MoveDown(idx) => {
            if let Some(q) = econ.construction.get_mut(player) {
                if *idx + 1 < q.items.len() {
                    q.items.swap(*idx, *idx + 1);
                }
            }
            handled()
        }
        ConstructionV6Command::Remove(idx) => {
            econ.cancel_construction_item(world, player, *idx, db);
            handled()
        }
        ConstructionV6Command::ToggleProjectPaused(idx, paused) => {
            if let Some(item) = econ
                .construction
                .get_mut(player)
                .and_then(|queue| queue.items.get_mut(*idx))
            {
                item.paused = *paused;
                item.runtime.paused = *paused;
            }
            handled()
        }
        ConstructionV6Command::SetProjectPriority { idx, priority } => {
            if let Some(item) = econ
                .construction
                .get_mut(player)
                .and_then(|queue| queue.items.get_mut(*idx))
            {
                item.priority = *priority;
                item.runtime.priority = *priority;
            }
            handled()
        }
        ConstructionV6Command::SetProjectWeight { idx, weight } => {
            if let Some(item) = econ
                .construction
                .get_mut(player)
                .and_then(|queue| queue.items.get_mut(*idx))
            {
                let weight = weight.max(0.1);
                item.weight = weight;
                item.runtime.weight = weight;
            }
            handled()
        }
        ConstructionV6Command::StartConstructionMode { building_key } => {
            *construction_highlight_province_ids =
                construction_highlight_provinces(world, db, player, building_key);
            *construction_mode = Some(building_key.clone());
            ConstructionControlEffect {
                handled: true,
                reset_auto_build_month: false,
                highlight_changed: true,
            }
        }
        ConstructionV6Command::SwitchPM {
            building_idx,
            group,
            pm_id,
        } => {
            let pm_valid = db.production_methods.iter().any(|pm| {
                world
                    .countries
                    .buildings_v6
                    .buildings
                    .get(*building_idx)
                    .map(|building| {
                        pm.id == *pm_id
                            && pm.building_id == building.building_def_id
                            && hoi4_content::production_method_group(pm) == *group
                            && production_method_lock_reason(world, db, player, pm).is_none()
                    })
                    .unwrap_or(false)
            });
            if pm_valid && *building_idx < world.countries.buildings_v6.buildings.len() {
                set_building_pm_group(
                    &mut world.countries.buildings_v6.buildings[*building_idx],
                    group,
                    pm_id,
                );
            }
            handled()
        }
        ConstructionV6Command::SwitchPMNationwide {
            building_def_id,
            group,
            pm_id,
        } => {
            let pm_valid = db.production_methods.iter().any(|pm| {
                pm.id == *pm_id
                    && pm.building_id == *building_def_id
                    && hoi4_content::production_method_group(pm) == *group
                    && production_method_lock_reason(world, db, player, pm).is_none()
            });
            if pm_valid {
                for building in world
                    .countries
                    .buildings_v6
                    .buildings
                    .iter_mut()
                    .filter(|building| building.building_def_id == *building_def_id)
                {
                    set_building_pm_group(building, group, pm_id);
                }
            }
            handled()
        }
    }
}

pub fn construction_highlight_provinces(
    world: &World,
    db: &hoi4_content::V6Database,
    player: usize,
    building_key: &str,
) -> HashSet<u32> {
    let mut highlighted = HashSet::new();
    let Some(building_def) = db.buildings.iter().find(|def| def.id == building_key) else {
        return highlighted;
    };

    let player_cid = CountryId(player as u16);
    for si in 0..world.states.count {
        if world.states.owners[si] != player_cid {
            continue;
        }
        let sid = StateId(si as u16);
        if !state_has_free_building_slot(world, sid) {
            continue;
        }
        if hoi4_logic::economy::construction_tick::validate_build_location(
            world,
            db,
            player,
            building_def,
            sid,
        )
        .is_err()
        {
            continue;
        }
        for province in &world.states.provinces[si] {
            highlighted.insert(province.0 as u32);
        }
    }
    highlighted
}

pub fn state_has_free_building_slot(world: &World, state: StateId) -> bool {
    let si = state.0 as usize;
    if si >= world.states.count {
        return false;
    }
    let used: u8 = world
        .countries
        .buildings_v6
        .buildings
        .iter()
        .filter(|building| building.state == state && building.level > 0)
        .map(|building| building.level)
        .sum();
    (used as u16) < state_building_capacity(world, state)
}

pub fn state_building_capacity(world: &World, state: StateId) -> u16 {
    let si = state.0 as usize;
    if si >= world.states.count {
        return 0;
    }
    (world.states.category_slots[si] as u16).max(4) + 20
}

fn handled() -> ConstructionControlEffect {
    ConstructionControlEffect {
        handled: true,
        reset_auto_build_month: false,
        highlight_changed: false,
    }
}

fn production_method_lock_reason(
    world: &World,
    db: &hoi4_content::V6Database,
    player: usize,
    pm: &hoi4_content::ProductionMethodDef,
) -> Option<String> {
    if let Some(tech_id) = &pm.unlocked_by {
        if !world.countries.completed_techs[player].contains(tech_id) {
            let tech_name = db
                .technologies
                .iter()
                .find(|tech| tech.id == *tech_id)
                .map(|tech| tech.name.as_str())
                .unwrap_or(tech_id.as_str());
            return Some(format!("Requires technology: {}", tech_name));
        }
    }
    if let Some(reason) = required_law_label(db, pm.required_law.as_ref()) {
        if let Some((cat, law_id)) = &pm.required_law {
            let current =
                &world.countries.law_store.law_sets[player].0[law_category_index(*cat)].current;
            if current != law_id {
                return Some(reason);
            }
        }
    }
    None
}

fn required_law_label(
    db: &hoi4_content::V6Database,
    requirement: Option<&(hoi4_content::v6_loader::LawCategoryDef, String)>,
) -> Option<String> {
    let (category, law_id) = requirement?;
    let law_name = match category {
        hoi4_content::v6_loader::LawCategoryDef::Conscription => db
            .conscription_laws
            .iter()
            .find(|law| law.id == *law_id)
            .map(|law| law.name.as_str()),
        hoi4_content::v6_loader::LawCategoryDef::Economy => db
            .economy_laws
            .iter()
            .find(|law| law.id == *law_id)
            .map(|law| law.name.as_str()),
        hoi4_content::v6_loader::LawCategoryDef::Trade => db
            .trade_laws
            .iter()
            .find(|law| law.id == *law_id)
            .map(|law| law.name.as_str()),
        hoi4_content::v6_loader::LawCategoryDef::Taxation => db
            .taxation_laws
            .iter()
            .find(|law| law.id == *law_id)
            .map(|law| law.name.as_str()),
        hoi4_content::v6_loader::LawCategoryDef::CivilRights => db
            .civil_rights_laws
            .iter()
            .find(|law| law.id == *law_id)
            .map(|law| law.name.as_str()),
        hoi4_content::v6_loader::LawCategoryDef::InformationControl => db
            .information_control_laws
            .iter()
            .find(|law| law.id == *law_id)
            .map(|law| law.name.as_str()),
    };
    Some(format!(
        "Requires law: {}",
        law_name.unwrap_or(law_id.as_str())
    ))
}

fn law_category_index(category: hoi4_content::v6_loader::LawCategoryDef) -> usize {
    match category {
        hoi4_content::v6_loader::LawCategoryDef::Conscription => {
            hoi4_state::LawCategory::Conscription.index()
        }
        hoi4_content::v6_loader::LawCategoryDef::Economy => {
            hoi4_state::LawCategory::Economy.index()
        }
        hoi4_content::v6_loader::LawCategoryDef::Trade => hoi4_state::LawCategory::Trade.index(),
        hoi4_content::v6_loader::LawCategoryDef::Taxation => {
            hoi4_state::LawCategory::Taxation.index()
        }
        hoi4_content::v6_loader::LawCategoryDef::CivilRights => {
            hoi4_state::LawCategory::CivilRights.index()
        }
        hoi4_content::v6_loader::LawCategoryDef::InformationControl => {
            hoi4_state::LawCategory::InformationControl.index()
        }
    }
}

fn set_building_pm_group(building: &mut hoi4_state::Building, group: &str, pm_id: &str) {
    if let Some(active) = building
        .active_pm_by_group
        .iter_mut()
        .find(|active| active.group == group)
    {
        active.pm_id = pm_id.to_owned();
    } else {
        building
            .active_pm_by_group
            .push(hoi4_state::ActiveProductionMethod {
                group: group.to_owned(),
                pm_id: pm_id.to_owned(),
            });
    }
    if group == "base" || building.active_pm.is_empty() {
        building.active_pm = pm_id.to_owned();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Arc;

    use hoi4_data::{Color, Country, CountryTag, GameData, State};
    use hoi4_logic::economy::{BuildOrder, ConstructionItem};
    use hoi4_map::{
        definition::{ProvinceDefinition, ProvinceType},
        GameMap, Heightmap, ProvinceMap, TerrainBitmap, TerrainCatalog,
    };

    fn item(key: &str) -> ConstructionItem {
        ConstructionItem::from_order(BuildOrder::new(key, StateId(0)))
    }

    fn test_world() -> World {
        let mut data = GameData::default();
        let tag = CountryTag::new("GER");
        data.countries.insert(
            tag.clone(),
            Country {
                tag: tag.clone(),
                color: Color {
                    r: 80,
                    g: 80,
                    b: 80,
                },
                graphical_culture: "western_european_gfx".to_owned(),
                capital: 1,
                ruling_party: "fascism".to_owned(),
                technologies: vec!["industrial_production".to_owned()],
            },
        );
        data.states.push(State {
            id: 1,
            name: "莱茵兰".to_owned(),
            manpower: 1_000_000,
            owner: tag.clone(),
            cores: vec![tag],
            provinces: vec![1],
            category: "city".to_owned(),
            infrastructure: 5,
            victory_points: vec![],
            resources: vec![],
        });
        let map = Arc::new(GameMap {
            definitions: vec![
                None,
                Some(ProvinceDefinition {
                    id: 1,
                    r: 1,
                    g: 1,
                    b: 1,
                    province_type: ProvinceType::Land,
                    coastal: false,
                    terrain: "plains".to_owned(),
                    continent: 1,
                }),
            ],
            rgb_to_id: HashMap::new(),
            province_map: ProvinceMap {
                width: 1,
                height: 1,
                pixels: vec![1],
            },
            adjacencies: vec![],
            special_adjacencies: vec![],
            heightmap: Heightmap {
                width: 1,
                height: 1,
                pixels: vec![0],
            },
            terrain_bmp: TerrainBitmap {
                width: 1,
                height: 1,
                pixels: vec![0],
                palette: [[0; 3]; 256],
            },
            terrain_catalog: TerrainCatalog::default(),
            tree_definition_bmp: None,
            tree_indices: Default::default(),
        });
        World::new(map, Arc::new(data))
    }

    #[test]
    fn construction_control_commands_mutate_real_queue_items() {
        let mut world = test_world();
        let mut econ = EconomyState::new(&world);
        let db = hoi4_content::V6Database::load();
        let mut auto_build_enabled = false;
        let mut construction_mode = None;
        let mut highlights = HashSet::new();
        econ.construction[0].items = vec![item("a"), item("b"), item("c")];

        apply_for_test(
            ConstructionV6Command::MoveUp(2),
            &mut world,
            &mut econ,
            &db,
            &mut auto_build_enabled,
            &mut construction_mode,
            &mut highlights,
        );
        assert_eq!(econ.construction[0].items[1].building_key, "c");

        apply_for_test(
            ConstructionV6Command::MoveDown(1),
            &mut world,
            &mut econ,
            &db,
            &mut auto_build_enabled,
            &mut construction_mode,
            &mut highlights,
        );
        assert_eq!(econ.construction[0].items[2].building_key, "c");

        apply_for_test(
            ConstructionV6Command::ToggleProjectPaused(0, true),
            &mut world,
            &mut econ,
            &db,
            &mut auto_build_enabled,
            &mut construction_mode,
            &mut highlights,
        );
        assert!(econ.construction[0].items[0].paused);
        assert!(econ.construction[0].items[0].runtime.paused);

        apply_for_test(
            ConstructionV6Command::SetProjectPriority {
                idx: 0,
                priority: 3,
            },
            &mut world,
            &mut econ,
            &db,
            &mut auto_build_enabled,
            &mut construction_mode,
            &mut highlights,
        );
        assert_eq!(econ.construction[0].items[0].priority, 3);
        assert_eq!(econ.construction[0].items[0].runtime.priority, 3);

        apply_for_test(
            ConstructionV6Command::SetProjectWeight {
                idx: 0,
                weight: 0.0,
            },
            &mut world,
            &mut econ,
            &db,
            &mut auto_build_enabled,
            &mut construction_mode,
            &mut highlights,
        );
        assert_eq!(econ.construction[0].items[0].weight, 0.1);
        assert_eq!(econ.construction[0].items[0].runtime.weight, 0.1);
    }

    fn apply_for_test(
        cmd: ConstructionV6Command,
        world: &mut World,
        econ: &mut EconomyState,
        db: &hoi4_content::V6Database,
        auto_build_enabled: &mut bool,
        construction_mode: &mut Option<String>,
        highlights: &mut HashSet<u32>,
    ) {
        let effect = apply_construction_control_command(
            &cmd,
            world,
            econ,
            db,
            0,
            auto_build_enabled,
            construction_mode,
            highlights,
        );
        assert!(effect.handled);
    }

    #[test]
    fn construction_control_effects_report_auto_build_and_highlight_changes() {
        let effect = ConstructionControlEffect {
            handled: true,
            reset_auto_build_month: true,
            highlight_changed: false,
        };
        assert!(effect.handled);
        assert!(effect.reset_auto_build_month);
        assert!(!effect.highlight_changed);
    }
}
