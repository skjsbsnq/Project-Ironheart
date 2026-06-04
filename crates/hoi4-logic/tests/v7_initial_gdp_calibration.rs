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

fn tick_days(world: &mut World, db: &V6Database, days: i64) {
    let mut econ = EconomyState::new(world);
    for day in 1..=days {
        tick_daily_v6(world, &mut econ, db, day);
    }
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
fn h4_initial_gdp_is_validation_only_until_runtime_tick() {
    let (world, db) = calibration_world();
    let tags = ["USA", "GER", "SOV", "ENG", "FRA", "JAP", "ITA", "CHI"];

    for tag in tags {
        let country = world.country(tag).expect("country exists");
        let profile = db
            .historical_countries
            .iter()
            .find(|profile| profile.tag == tag)
            .expect("historical profile exists");
        let treasury = &world.countries.treasury.treasuries[country.0 as usize];
        assert_eq!(treasury.gdp_gbp, 0.0, "{tag} runtime GDP starts unset");
        assert_eq!(treasury.gdp_rm, 0.0, "{tag} runtime GDP RM starts unset");
        assert_eq!(
            treasury.gdp_breakdown.historical_validation_gbp, profile.gdp_1936_gbp,
            "{tag} historical GDP should be retained only as validation target"
        );
    }
}

#[test]
fn h4_runtime_gdp_breakdown_is_generated_after_tick() {
    let (mut world, db) = calibration_world();
    tick_days(&mut world, &db, 30);

    for tag in ["USA", "GER", "SOV", "ENG", "FRA", "JAP", "ITA", "CHI"] {
        let country = world.country(tag).expect("country exists");
        let treasury = &world.countries.treasury.treasuries[country.0 as usize];
        assert!(
            treasury.gdp_gbp > 0.0,
            "{tag} runtime GDP should be produced by tick"
        );
        assert!(
            treasury.gdp_rm > 0.0,
            "{tag} runtime GDP RM should be produced by tick"
        );
        assert!(
            treasury.gdp_breakdown.runtime_total_rm() > 0.0,
            "{tag} GDP breakdown should have runtime components"
        );
        assert!(
            treasury.gdp_breakdown.building_primary_rm > 0.0
                || treasury.gdp_breakdown.building_secondary_rm > 0.0
                || treasury.gdp_breakdown.building_tertiary_rm > 0.0,
            "{tag} GDP should include building value-added"
        );
        assert!(
            treasury.gdp_breakdown.pop_income_rm > 0.0
                && treasury.gdp_breakdown.pop_consumption_rm > 0.0,
            "{tag} GDP should include POP income and consumption"
        );
    }
}

#[test]
fn h4_major_gdp_records_historical_validation_error_after_30_and_90_days() {
    let (mut world, db) = calibration_world();
    let tags = ["USA", "GER", "SOV", "ENG", "FRA", "JAP", "ITA", "CHI"];

    for tag in tags {
        let country = world.country(tag).expect("country exists");
        assert_eq!(
            world.countries.treasury.treasuries[country.0 as usize].gdp_gbp, 0.0,
            "{tag} initial GDP should not be seeded from historical target"
        );
    }

    tick_days(&mut world, &db, 30);
    for tag in tags {
        let country = world.country(tag).expect("country exists");
        let (actual_gbp, actual_rm, target, rate) = gdp_snapshot(&world, &db, tag);
        let treasury = &world.countries.treasury.treasuries[country.0 as usize];
        let signed_err = (actual_gbp - target) / target;
        assert!(
            signed_err.is_finite()
                && (treasury.gdp_breakdown.historical_validation_error_ratio - signed_err).abs()
                    < 0.0001,
            "{tag} 30d GDP validation error should be recorded from runtime GDP (actual_gbp={actual_gbp:.0}, actual_rm={actual_rm:.0}, target={target:.0}, rate={rate:.3})"
        );
    }

    tick_days(&mut world, &db, 60);
    for tag in tags {
        let country = world.country(tag).expect("country exists");
        let (actual_gbp, actual_rm, target, rate) = gdp_snapshot(&world, &db, tag);
        let treasury = &world.countries.treasury.treasuries[country.0 as usize];
        let signed_err = (actual_gbp - target) / target;
        assert!(
            signed_err.is_finite()
                && (treasury.gdp_breakdown.historical_validation_error_ratio - signed_err).abs()
                    < 0.0001,
            "{tag} 90d GDP validation error should be recorded from runtime GDP (actual_gbp={actual_gbp:.0}, actual_rm={actual_rm:.0}, target={target:.0}, rate={rate:.3})"
        );
    }
}

#[test]
fn h4_china_has_runtime_gdp_and_low_industrial_capacity() {
    let (mut world, db) = calibration_world();
    tick_days(&mut world, &db, 30);
    let chi = world.country("CHI").unwrap();
    let jap = world.country("JAP").unwrap();
    let chi_gdp = world.countries.treasury.treasuries[chi.0 as usize].gdp_gbp;
    let chi_industry_levels = building_levels(&world, chi, &["steel_mill", "machinery_workshop"]);
    let jap_industry_levels = building_levels(&world, jap, &["steel_mill", "machinery_workshop"]);

    assert!(chi_gdp > 0.0, "CHI runtime GDP should be produced by tick");
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
