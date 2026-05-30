use std::collections::HashMap;
use std::sync::Arc;

use hoi4_data::{Color, Country, CountryTag, DivisionTemplate, GameData, State, SubunitDef};
use hoi4_logic::military::templates::{
    preview_template, set_line_battalion, switch_division_template,
};
use hoi4_map::{GameMap, Heightmap, ProvinceMap, TerrainBitmap, TerrainCatalog};
use hoi4_state::{CountryId, ProvinceId, World};

fn test_data() -> Arc<GameData> {
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
            technologies: vec![],
        },
    );
    data.states.push(State {
        id: 1,
        name: "Test State".to_owned(),
        manpower: 1_000_000,
        owner: tag,
        cores: vec![CountryTag::new("GER")],
        provinces: vec![],
        category: "city".to_owned(),
        infrastructure: 3,
        victory_points: vec![],
        resources: vec![],
    });

    let mut infantry_need = HashMap::new();
    infantry_need.insert("infantry_equipment".to_owned(), 100);
    data.subunits.insert(
        "infantry".to_owned(),
        SubunitDef {
            key: "infantry".to_owned(),
            group: "infantry".to_owned(),
            combat_width: 2.0,
            manpower: 1_000,
            training_time: 90,
            need: infantry_need,
            ..SubunitDef::default()
        },
    );
    let mut artillery_need = HashMap::new();
    artillery_need.insert("artillery".to_owned(), 36);
    data.subunits.insert(
        "artillery".to_owned(),
        SubunitDef {
            key: "artillery".to_owned(),
            group: "artillery".to_owned(),
            combat_width: 3.0,
            soft_attack: 20.0,
            manpower: 500,
            training_time: 120,
            need: artillery_need,
            ..SubunitDef::default()
        },
    );
    data.division_templates.insert(
        "GER".to_owned(),
        vec![
            DivisionTemplate {
                name: "Infantry Division".to_owned(),
                country_tag: Some("GER".to_owned()),
                regiments: vec!["infantry".to_owned()],
                support: vec![],
                division_names_group: None,
            },
            DivisionTemplate {
                name: "Infantry Artillery Division".to_owned(),
                country_tag: Some("GER".to_owned()),
                regiments: vec!["infantry".to_owned(), "artillery".to_owned()],
                support: vec![],
                division_names_group: None,
            },
        ],
    );
    Arc::new(data)
}

fn test_map() -> Arc<GameMap> {
    Arc::new(GameMap {
        definitions: vec![],
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

#[test]
fn template_editor_recomputes_stats_and_equipment() {
    let data = test_data();
    let mut templates = data.division_templates["GER"].clone();
    let stockpile = HashMap::from([
        ("infantry_equipment".to_owned(), 250.0),
        ("artillery".to_owned(), 72.0),
    ]);

    set_line_battalion(&mut templates, 0, 0, 1, Some("artillery".to_owned())).unwrap();
    let preview = preview_template(&templates[0], data.as_ref(), Some(&stockpile));

    assert_eq!(preview.combat_width, 5.0);
    assert_eq!(preview.manpower, 1_500);
    assert_eq!(preview.equipment_needed["infantry_equipment"], 100);
    assert_eq!(preview.equipment_needed["artillery"], 36);
    assert!(preview.training_days >= 120.0);
    assert!(preview.stockpile_satisfied_divisions > 0.0);
}

#[test]
fn switching_template_creates_equipment_deficit() {
    let data = test_data();
    let mut world = World::new(test_map(), data.clone());
    world
        .divisions
        .push(CountryId(0), ProvinceId(0), 0, 30.0, 1.0, "Test".to_owned());
    world.divisions.organisation[0] = 30.0;

    let result = switch_division_template(&mut world, data.as_ref(), 0, 1).unwrap();

    assert_eq!(world.divisions.template_indices[0], 1);
    assert!(result.equipment_deficit["artillery"] > 0.0);
    assert!(world.divisions.organisation[0] < 30.0);
    assert!(world.divisions.strength[0] < 1.0);
}
