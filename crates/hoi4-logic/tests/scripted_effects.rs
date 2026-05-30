use std::collections::HashMap;
use std::sync::Arc;

use hoi4_data::{Color, Country, CountryTag, GameData, State};
use hoi4_logic::diplomacy::peace_conference;
use hoi4_logic::scripted_effects::{
    apply_diplomatic_effect, apply_situation_diplomatic_effect, ScriptedDiplomaticEffect,
};
use hoi4_map::{
    GameMap, Heightmap, ProvinceDefinition, ProvinceMap, ProvinceType, TerrainBitmap,
    TerrainCatalog,
};
use hoi4_state::{AutonomyLevel, WarSide, Wargoal, WargoalType, World};

fn test_map() -> Arc<GameMap> {
    let definitions = (0..=4)
        .map(|id| {
            (id > 0).then_some(ProvinceDefinition {
                id,
                r: id as u8,
                g: 0,
                b: 0,
                province_type: ProvinceType::Land,
                coastal: false,
                terrain: "plains".to_owned(),
                continent: 1,
            })
        })
        .collect();
    Arc::new(GameMap {
        definitions,
        rgb_to_id: HashMap::new(),
        province_map: ProvinceMap {
            width: 1,
            height: 1,
            pixels: vec![0],
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
        tree_indices: std::collections::HashSet::new(),
    })
}

fn test_data() -> Arc<GameData> {
    let mut data = GameData::default();
    for (idx, tag) in ["GER", "ITA", "ENG", "FRA"].iter().enumerate() {
        let ctag = CountryTag::new(tag);
        data.countries.insert(
            ctag.clone(),
            Country {
                tag: ctag.clone(),
                color: Color {
                    r: 80,
                    g: 80,
                    b: 80,
                },
                graphical_culture: "western_european_gfx".to_owned(),
                capital: (idx + 1) as u16,
                ruling_party: "neutrality".to_owned(),
                technologies: vec![],
            },
        );
        data.states.push(State {
            id: (idx + 1) as u16,
            name: format!("{tag} State"),
            manpower: 100_000,
            owner: ctag.clone(),
            cores: vec![ctag],
            provinces: vec![(idx + 1) as u16],
            category: "city".to_owned(),
            infrastructure: 3,
            victory_points: vec![],
            resources: vec![],
        });
    }
    Arc::new(data)
}

fn world() -> World {
    World::new(test_map(), test_data())
}

#[test]
fn create_and_join_faction_use_logic_api() {
    let mut world = world();
    apply_diplomatic_effect(
        &mut world,
        &ScriptedDiplomaticEffect::CreateFaction {
            leader: "GER".into(),
            name: "Axis".into(),
        },
    )
    .unwrap();
    apply_diplomatic_effect(
        &mut world,
        &ScriptedDiplomaticEffect::AddToFaction {
            faction_leader: "GER".into(),
            member: "ITA".into(),
        },
    )
    .unwrap();

    let ger = world.country("GER").unwrap();
    let ita = world.country("ITA").unwrap();
    let fid = world.diplomacy.faction_of(ger).unwrap();
    let faction = world.diplomacy.faction(fid).unwrap();
    assert_eq!(faction.leader, ger);
    assert!(faction.contains(ita));
}

#[test]
fn declare_war_propagates_faction_members_and_at_war_flags() {
    let mut world = world();
    apply_diplomatic_effect(
        &mut world,
        &ScriptedDiplomaticEffect::CreateFaction {
            leader: "GER".into(),
            name: "Axis".into(),
        },
    )
    .unwrap();
    apply_diplomatic_effect(
        &mut world,
        &ScriptedDiplomaticEffect::AddToFaction {
            faction_leader: "GER".into(),
            member: "ITA".into(),
        },
    )
    .unwrap();
    apply_diplomatic_effect(
        &mut world,
        &ScriptedDiplomaticEffect::DeclareWar {
            attacker: "GER".into(),
            defender: "ENG".into(),
        },
    )
    .unwrap();

    let ger = world.country("GER").unwrap();
    let ita = world.country("ITA").unwrap();
    let eng = world.country("ENG").unwrap();
    let war = world.diplomacy.wars.values().next().unwrap();
    assert!(war.attackers.contains(&ger));
    assert!(war.attackers.contains(&ita));
    assert!(war.defenders.contains(&eng));
    assert!(world.countries.at_war[ger.0 as usize]);
    assert!(world.countries.at_war[ita.0 as usize]);
    assert!(world.countries.at_war[eng.0 as usize]);
}

#[test]
fn access_autonomy_transfer_core_and_annex_effects_work() {
    let mut world = world();
    apply_diplomatic_effect(
        &mut world,
        &ScriptedDiplomaticEffect::GrantMilitaryAccess {
            grantor: "GER".into(),
            grantee: "ITA".into(),
        },
    )
    .unwrap();
    let ger = world.country("GER").unwrap();
    let ita = world.country("ITA").unwrap();
    assert!(world.diplomacy.has_military_access(ita, ger));

    apply_diplomatic_effect(
        &mut world,
        &ScriptedDiplomaticEffect::SetAutonomy {
            master: "GER".into(),
            subject: "ITA".into(),
            level: "puppet".into(),
        },
    )
    .unwrap();
    assert_eq!(
        world.diplomacy.autonomy.get(&ita).unwrap().level,
        AutonomyLevel::Puppet
    );

    apply_diplomatic_effect(
        &mut world,
        &ScriptedDiplomaticEffect::TransferState {
            state: 3,
            owner: "GER".into(),
        },
    )
    .unwrap();
    let state = world.state_id_lookup[&3];
    assert_eq!(world.states.owners[state.0 as usize], ger);

    apply_diplomatic_effect(
        &mut world,
        &ScriptedDiplomaticEffect::AddCore {
            state: 3,
            country: "GER".into(),
        },
    )
    .unwrap();
    assert!(world.states.cores[state.0 as usize].contains(&ger));

    let fra = world.country("FRA").unwrap();
    apply_diplomatic_effect(
        &mut world,
        &ScriptedDiplomaticEffect::AnnexCountry {
            annexer: "GER".into(),
            target: "FRA".into(),
        },
    )
    .unwrap();
    assert!(world.diplomacy.annexed_countries.contains(&fra));
    assert!(world.country("FRA").is_none());
}

#[test]
fn annex_country_restores_target_occupied_states_to_legal_owner() {
    let mut world = world();
    let eng = world.country("ENG").unwrap();
    let fra = world.country("FRA").unwrap();
    let eng_state = world.state_id_lookup[&3];
    let eng_province = world.states.provinces[eng_state.0 as usize][0];

    world.states.controllers[eng_state.0 as usize] = fra;
    world.provinces.controllers[eng_province.0 as usize] = fra;

    apply_diplomatic_effect(
        &mut world,
        &ScriptedDiplomaticEffect::AnnexCountry {
            annexer: "ITA".into(),
            target: "FRA".into(),
        },
    )
    .unwrap();

    assert_eq!(world.states.owners[eng_state.0 as usize], eng);
    assert_eq!(world.states.controllers[eng_state.0 as usize], eng);
    assert_eq!(world.provinces.owners[eng_province.0 as usize], eng);
    assert_eq!(world.provinces.controllers[eng_province.0 as usize], eng);
    assert!(!world
        .states
        .controllers
        .iter()
        .any(|&controller| controller == fra));
    assert!(!world
        .provinces
        .controllers
        .iter()
        .any(|&controller| controller == fra));
    assert!(world.diplomacy.annexed_countries.contains(&fra));
}

#[test]
fn peace_annex_restores_target_occupied_states_to_legal_owner() {
    let mut world = world();
    let ita = world.country("ITA").unwrap();
    let eng = world.country("ENG").unwrap();
    let fra = world.country("FRA").unwrap();
    let war_id = apply_diplomatic_effect(
        &mut world,
        &ScriptedDiplomaticEffect::DeclareWar {
            attacker: "ITA".into(),
            defender: "FRA".into(),
        },
    )
    .map(|_| *world.diplomacy.wars.keys().next().unwrap())
    .unwrap();
    world
        .diplomacy
        .wars
        .get_mut(&war_id)
        .unwrap()
        .attacker_wargoals
        .push(Wargoal {
            claimant: ita,
            target: fra,
            kind: WargoalType::Annex,
            target_state: None,
            justified: true,
            justify_progress: 1.0,
            justify_total_days: 1.0,
        });

    let eng_state = world.state_id_lookup[&3];
    let eng_province = world.states.provinces[eng_state.0 as usize][0];
    world.states.controllers[eng_state.0 as usize] = fra;
    world.provinces.controllers[eng_province.0 as usize] = fra;

    let outcome = peace_conference(&mut world, war_id, WarSide::Attacker).unwrap();

    assert_eq!(outcome.annexed_countries, 1);
    assert_eq!(world.states.owners[eng_state.0 as usize], eng);
    assert_eq!(world.states.controllers[eng_state.0 as usize], eng);
    assert_eq!(world.provinces.owners[eng_province.0 as usize], eng);
    assert_eq!(world.provinces.controllers[eng_province.0 as usize], eng);
    assert!(!world
        .states
        .controllers
        .iter()
        .any(|&controller| controller == fra));
    assert!(!world
        .provinces
        .controllers
        .iter()
        .any(|&controller| controller == fra));
}

#[test]
fn focus_event_and_situation_diplomatic_effects_share_results() {
    let mut from_focus = world();
    let mut flags = hoi4_content::GlobalFlags::default();
    let ger = from_focus.country("GER").unwrap();
    hoi4_content::run_effects(
        &[
            hoi4_content::Effect::CreateFaction("Axis".into()),
            hoi4_content::Effect::AddToFaction("ITA".into()),
            hoi4_content::Effect::DeclareWarOn("ENG".into()),
            hoi4_content::Effect::GiveMilitaryAccess("ITA".into()),
            hoi4_content::Effect::PuppetCountry("FRA".into()),
            hoi4_content::Effect::TransferState(3),
            hoi4_content::Effect::AddCoreTo {
                state: 3,
                country: "GER".into(),
            },
        ],
        &mut from_focus,
        ger,
        &mut flags,
    );

    let mut from_situation = world();
    for effect in [
        hoi4_content::SituationEffect::CreateFaction {
            leader: "GER".into(),
            name: "Axis".into(),
        },
        hoi4_content::SituationEffect::AddToFaction {
            faction_leader: "GER".into(),
            member: "ITA".into(),
        },
        hoi4_content::SituationEffect::CreateWar {
            attacker: "GER".into(),
            defender: "ENG".into(),
        },
        hoi4_content::SituationEffect::GrantMilitaryAccess {
            grantor: "GER".into(),
            grantee: "ITA".into(),
        },
        hoi4_content::SituationEffect::SetAutonomy {
            master: "GER".into(),
            subject: "FRA".into(),
            level: "puppet".into(),
        },
        hoi4_content::SituationEffect::TransferState {
            state: 3,
            owner: "GER".into(),
        },
        hoi4_content::SituationEffect::AddCore {
            state: 3,
            country: "GER".into(),
        },
    ] {
        apply_situation_diplomatic_effect(&mut from_situation, &effect)
            .unwrap()
            .unwrap();
    }

    assert_diplomatic_core_state_matches(&from_focus, &from_situation);
}

fn assert_diplomatic_core_state_matches(left: &World, right: &World) {
    for tag in ["GER", "ITA", "ENG", "FRA"] {
        assert_eq!(
            left.country(tag).is_some(),
            right.country(tag).is_some(),
            "{tag}"
        );
    }
    let left_ger = left.country("GER").unwrap();
    let right_ger = right.country("GER").unwrap();
    let left_ita = left.country("ITA").unwrap();
    let right_ita = right.country("ITA").unwrap();
    let left_eng = left.country("ENG").unwrap();
    let right_eng = right.country("ENG").unwrap();
    let left_fra = left.country("FRA").unwrap();
    let right_fra = right.country("FRA").unwrap();

    assert_eq!(
        left.diplomacy.factions.len(),
        right.diplomacy.factions.len()
    );
    assert_eq!(
        left.diplomacy.faction_of(left_ger).is_some(),
        right.diplomacy.faction_of(right_ger).is_some()
    );
    assert_eq!(
        left.diplomacy.faction_of(left_ita).is_some(),
        right.diplomacy.faction_of(right_ita).is_some()
    );
    assert_eq!(
        left.diplomacy.at_war_with(left_ger, left_eng),
        right.diplomacy.at_war_with(right_ger, right_eng)
    );
    assert_eq!(
        left.diplomacy.is_at_war(left_ita),
        right.diplomacy.is_at_war(right_ita)
    );
    assert_eq!(
        left.diplomacy.has_military_access(left_ita, left_ger),
        right.diplomacy.has_military_access(right_ita, right_ger)
    );
    assert_eq!(
        left.diplomacy.autonomy.get(&left_fra).unwrap().level,
        right.diplomacy.autonomy.get(&right_fra).unwrap().level
    );

    let left_state = left.state_id_lookup[&3];
    let right_state = right.state_id_lookup[&3];
    assert_eq!(left.states.owners[left_state.0 as usize], left_ger);
    assert_eq!(right.states.owners[right_state.0 as usize], right_ger);
    assert!(left.states.cores[left_state.0 as usize].contains(&left_ger));
    assert!(right.states.cores[right_state.0 as usize].contains(&right_ger));
}
