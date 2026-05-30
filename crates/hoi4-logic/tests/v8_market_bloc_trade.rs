use std::collections::HashMap;
use std::sync::Arc;

use hoi4_content::{inject_v6_into_world, V6Database};
use hoi4_data::{Color, Country, CountryTag, DivisionTemplate, GameData, State, SubunitDef};
use hoi4_map::{GameMap, Heightmap, ProvinceDefinition, ProvinceMap, ProvinceType, TerrainBitmap};
use hoi4_state::{MarketBlocKind, TradeRouteKind, World};

fn test_map(max_province: u16) -> Arc<GameMap> {
    let mut definitions = vec![None; max_province as usize + 1];
    for province_id in 1..=max_province {
        definitions[province_id as usize] = Some(ProvinceDefinition {
            id: province_id,
            r: (province_id & 0xff) as u8,
            g: ((province_id >> 8) & 0xff) as u8,
            b: 0,
            province_type: ProvinceType::Land,
            coastal: true,
            terrain: "plains".to_owned(),
            continent: 1,
        });
    }

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
        terrain_catalog: hoi4_map::TerrainCatalog::default(),
        tree_definition_bmp: None,
        tree_indices: std::collections::HashSet::new(),
    })
}

fn add_country(data: &mut GameData, tag_str: &str, capital: u16) {
    let tag = CountryTag::new(tag_str);
    data.countries.insert(
        tag.clone(),
        Country {
            tag,
            color: Color {
                r: 80,
                g: 80,
                b: 80,
            },
            graphical_culture: "western_european_gfx".to_owned(),
            capital,
            ruling_party: "neutrality".to_owned(),
            technologies: Vec::new(),
        },
    );
    let mut infantry_need = HashMap::new();
    infantry_need.insert("infantry_equipment".to_owned(), 100);
    data.subunits
        .entry("infantry".to_owned())
        .or_insert(SubunitDef {
            key: "infantry".to_owned(),
            manpower: 1_000,
            need: infantry_need,
            ..SubunitDef::default()
        });
    data.division_templates.insert(
        tag_str.to_owned(),
        vec![DivisionTemplate {
            name: "Infantry Division".to_owned(),
            country_tag: Some(tag_str.to_owned()),
            regiments: vec!["infantry".to_owned()],
            support: vec![],
            division_names_group: None,
        }],
    );
}

fn add_state(data: &mut GameData, tag: &str, state_id: u16, province_id: &mut u16) {
    let country = CountryTag::new(tag);
    data.states.push(State {
        id: state_id,
        name: format!("{tag} V8 Market Bloc State"),
        manpower: 2_000_000,
        owner: country.clone(),
        cores: vec![country],
        provinces: vec![*province_id],
        category: "metropolis".to_owned(),
        infrastructure: 6,
        victory_points: vec![],
        resources: vec![],
    });
    *province_id += 1;
}

fn empire_world() -> (World, V6Database) {
    let mut data = GameData::default();
    let countries = [
        ("ENG", 126),
        ("CAN", 703),
        ("AST", 704),
        ("NZL", 705),
        ("SAF", 706),
        ("RAJ", 707),
        ("MAL", 702),
        ("GER", 28),
        ("ROM", 700),
        ("SWE", 701),
        ("JAP", 536),
        ("USA", 195),
    ];
    for (tag, capital) in countries {
        add_country(&mut data, tag, capital);
    }
    let mut province_id = 1u16;
    for (tag, state_id) in countries {
        add_state(&mut data, tag, state_id, &mut province_id);
    }

    let mut world = World::new(test_map(province_id), Arc::new(data));
    hoi4_logic::economy::init_world(&mut world);
    let db = V6Database::load();
    inject_v6_into_world(&mut world, &db);
    (world, db)
}

#[test]
fn british_empire_market_bloc_is_initialized() {
    let (world, _db) = empire_world();
    let eng = world.country("ENG").expect("ENG exists");
    let bloc = world
        .countries
        .market
        .bloc_for_country(eng)
        .expect("ENG should belong to a market bloc");

    assert_eq!(bloc.name, "英帝国优惠体系");
    assert_eq!(bloc.kind, MarketBlocKind::ImperialPreference);
    assert_eq!(bloc.leader, eng);
    for tag in ["ENG", "CAN", "AST", "NZL", "SAF", "RAJ", "MAL"] {
        let country = world.country(tag).expect("empire member exists");
        assert!(
            bloc.members.contains(&country),
            "{tag} should be a bloc member"
        );
        assert_eq!(
            world.countries.market.bloc_for_country(country).unwrap().id,
            bloc.id
        );
    }
}

#[test]
fn imperial_preference_routes_keep_runtime_semantics() {
    let (world, _db) = empire_world();
    let eng = world.country("ENG").expect("ENG exists");
    let mal = world.country("MAL").expect("MAL exists");

    let rubber_route = world
        .countries
        .trade
        .routes
        .iter()
        .find(|route| {
            route.importer == eng
                && route.exporter == mal
                && route.good_id.as_deref() == Some("rubber")
                && route.historical
        })
        .expect("ENG-MAL rubber route should exist");

    assert_eq!(rubber_route.kind, TradeRouteKind::ImperialPreference);
    assert!(rubber_route.kind.uses_sea_lanes());
}

#[test]
fn market_bloc_members_are_visible_for_panel_snapshots() {
    let (mut world, _db) = empire_world();
    let eng = world.country("ENG").expect("ENG exists");
    let can = world.country("CAN").expect("CAN exists");
    let mal = world.country("MAL").expect("MAL exists");
    world.countries.market.markets[can.0 as usize]
        .supply
        .insert("grain".to_owned(), 20.0);
    world.countries.market.markets[mal.0 as usize]
        .supply
        .insert("rubber".to_owned(), 18.0);

    let bloc = world
        .countries
        .market
        .bloc_for_country(eng)
        .expect("ENG should belong to a market bloc");
    let tags: Vec<&str> = bloc
        .members
        .iter()
        .filter_map(|member| {
            world
                .countries
                .tags
                .get(member.0 as usize)
                .map(String::as_str)
        })
        .collect();

    assert!(tags.contains(&"CAN"));
    assert!(tags.contains(&"MAL"));
    assert_eq!(
        world.countries.market.markets[can.0 as usize]
            .supply
            .get("grain")
            .copied(),
        Some(20.0)
    );
    assert_eq!(
        world.countries.market.markets[mal.0 as usize]
            .supply
            .get("rubber")
            .copied(),
        Some(18.0)
    );
}
