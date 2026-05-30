use std::collections::HashMap;
use std::sync::Arc;

use hoi4_content::{inject_v6_into_world, V6Database};
use hoi4_data::{Color, Country, CountryTag, GameData, State};
use hoi4_logic::economy::{tick_daily_v6, EconomyState};
use hoi4_map::{GameMap, Heightmap, ProvinceDefinition, ProvinceMap, ProvinceType, TerrainBitmap};
use hoi4_state::{CountryId, World};

fn test_map(max_province: u16) -> Arc<GameMap> {
    let mut definitions = vec![None; max_province as usize + 1];
    for province_id in 1..=max_province {
        definitions[province_id as usize] = Some(ProvinceDefinition {
            id: province_id,
            r: (province_id & 0xff) as u8,
            g: 0,
            b: 0,
            province_type: ProvinceType::Land,
            coastal: province_id % 3 == 0,
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

fn calibration_world() -> (World, V6Database) {
    let tags = ["USA", "GER", "SOV", "ENG", "FRA", "JAP", "ITA", "CHI"];
    let state_ids: [[u16; 3]; 8] = [
        [195, 202, 276],
        [28, 51, 29],
        [229, 230, 231],
        [91, 92, 93],
        [105, 106, 107],
        [300, 301, 302],
        [212, 213, 214],
        [613, 614, 615],
    ];
    let mut data = GameData::default();
    let mut province_id = 1u16;
    for (tag_idx, tag_str) in tags.iter().enumerate() {
        let tag = CountryTag::new(tag_str);
        data.countries.insert(
            tag.clone(),
            Country {
                tag: tag.clone(),
                color: Color {
                    r: 40 + tag_idx as u8 * 20,
                    g: 80,
                    b: 120,
                },
                graphical_culture: "western_european_gfx".to_owned(),
                capital: state_ids[tag_idx][0],
                ruling_party: "neutrality".to_owned(),
                technologies: Vec::new(),
            },
        );
        for (local_idx, state_id) in state_ids[tag_idx].iter().enumerate() {
            data.states.push(State {
                id: *state_id,
                name: format!("{tag_str} State {local_idx}"),
                manpower: 2_000_000 + tag_idx as u64 * 300_000 + local_idx as u64 * 150_000,
                owner: tag.clone(),
                cores: vec![tag.clone()],
                provinces: vec![province_id],
                category: "metropolis".to_owned(),
                infrastructure: 5 + local_idx as u8,
                victory_points: vec![],
                resources: vec![],
            });
            province_id += 1;
        }
    }

    let mut world = World::new(test_map(province_id), Arc::new(data));
    hoi4_logic::economy::init_world(&mut world);
    let db = V6Database::load();
    inject_v6_into_world(&mut world, &db);
    (world, db)
}

fn gdp_error(world: &World, db: &V6Database, tag: &str) -> f64 {
    let country = world.country(tag).expect("country exists");
    let target = db
        .historical_countries
        .iter()
        .find(|profile| profile.tag == tag)
        .expect("historical profile exists")
        .gdp_1936_gbp;
    let actual = world.countries.treasury.treasuries[country.0 as usize].gdp_gbp;
    ((actual / target) - 1.0).abs()
}

fn gdp_snapshot(world: &World, db: &V6Database, tag: &str) -> (f64, f64, f64, f32) {
    let country = world.country(tag).expect("country exists");
    let target = db
        .historical_countries
        .iter()
        .find(|profile| profile.tag == tag)
        .expect("historical profile exists")
        .gdp_1936_gbp;
    let ci = country.0 as usize;
    let treasury = &world.countries.treasury.treasuries[ci];
    (
        treasury.gdp_gbp,
        treasury.gdp_rm,
        target,
        world.countries.treasury.exchange_rates[ci].rm_per_gbp,
    )
}

#[test]
fn h4_gdp_order_matches_1936_targets() {
    let (world, _db) = calibration_world();
    let usa = world.country("USA").unwrap().0 as usize;
    let ger = world.country("GER").unwrap().0 as usize;
    let sov = world.country("SOV").unwrap().0 as usize;
    let eng = world.country("ENG").unwrap().0 as usize;
    let fra = world.country("FRA").unwrap().0 as usize;
    let jap = world.country("JAP").unwrap().0 as usize;
    let ita = world.country("ITA").unwrap().0 as usize;

    let treasury = &world.countries.treasury.treasuries;
    assert!(treasury[usa].gdp_gbp > treasury[ger].gdp_gbp);
    assert!(treasury[ger].gdp_gbp >= treasury[sov].gdp_gbp * 0.9);
    assert!(treasury[sov].gdp_gbp > treasury[eng].gdp_gbp);
    assert!(treasury[eng].gdp_gbp > treasury[fra].gdp_gbp);
    assert!(treasury[fra].gdp_gbp > treasury[jap].gdp_gbp);
    assert!(treasury[jap].gdp_gbp > treasury[ita].gdp_gbp);
}

#[test]
fn h4_major_gdp_stays_calibrated_after_30_and_90_days() {
    let (mut world, db) = calibration_world();
    let mut econ = EconomyState::new(&world);
    let tags = ["USA", "GER", "SOV", "ENG", "FRA", "JAP", "ITA", "CHI"];

    for tag in tags {
        assert!(
            gdp_error(&world, &db, tag) < 0.001,
            "{tag} initial GDP should match target"
        );
    }

    for day in 1..=30 {
        tick_daily_v6(&mut world, &mut econ, &db, day);
    }
    for tag in tags {
        let err = gdp_error(&world, &db, tag);
        let (actual_gbp, actual_rm, target, rate) = gdp_snapshot(&world, &db, tag);
        assert!(
            err <= 0.10,
            "{tag} 30d GDP error {err:.3} exceeds 10% (actual_gbp={actual_gbp:.0}, actual_rm={actual_rm:.0}, target={target:.0}, rate={rate:.3})"
        );
    }

    for day in 31..=90 {
        tick_daily_v6(&mut world, &mut econ, &db, day);
    }
    for tag in tags {
        let err = gdp_error(&world, &db, tag);
        let (actual_gbp, actual_rm, target, rate) = gdp_snapshot(&world, &db, tag);
        assert!(
            err <= 0.08,
            "{tag} 90d GDP error {err:.3} exceeds 8% (actual_gbp={actual_gbp:.0}, actual_rm={actual_rm:.0}, target={target:.0}, rate={rate:.3})"
        );
    }
}

#[test]
fn h4_china_has_high_total_gdp_low_industrial_capacity() {
    let (world, _db) = calibration_world();
    let chi = world.country("CHI").unwrap();
    let jap = world.country("JAP").unwrap();
    let chi_gdp = world.countries.treasury.treasuries[chi.0 as usize].gdp_gbp;
    let jap_gdp = world.countries.treasury.treasuries[jap.0 as usize].gdp_gbp;
    let chi_industry_levels = building_levels(&world, chi, &["steel_mill", "machinery_workshop"]);
    let jap_industry_levels = building_levels(&world, jap, &["steel_mill", "machinery_workshop"]);

    assert!(
        chi_gdp >= jap_gdp * 0.85,
        "CHI total GDP should not be too low"
    );
    assert!(
        chi_industry_levels < jap_industry_levels,
        "CHI industrial capacity should remain below JAP despite comparable total GDP"
    );
}

fn building_levels(world: &World, country: CountryId, ids: &[&str]) -> u16 {
    world
        .countries
        .buildings_v6
        .buildings
        .iter()
        .filter(|building| {
            let si = building.state.0 as usize;
            si < world.states.count
                && world.states.owners[si] == country
                && ids.contains(&building.building_def_id.as_str())
        })
        .map(|building| building.level as u16)
        .sum()
}
