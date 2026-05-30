use std::collections::HashMap;
use std::sync::Arc;

use hoi4_content::{inject_v6_into_world, V6Database};
use hoi4_data::{Color, Country, CountryTag, DivisionTemplate, GameData, State, SubunitDef};
use hoi4_logic::economy::{
    construction_tick, finance_tick, BuildOrder, ConstructionFundingSource, EconomyState,
};
use hoi4_map::{GameMap, Heightmap, ProvinceDefinition, ProvinceMap, ProvinceType, TerrainBitmap};
use hoi4_state::{
    BuildingOwner, CountryId, InvestmentAccountKind, OwnershipAccount, OwnershipShare, StateId,
    World,
};

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
            ruling_party: "fascism".to_owned(),
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

fn ownership_world() -> (World, V6Database, CountryId, StateId) {
    let mut data = GameData::default();
    add_country(&mut data, "GER", 28);
    data.states.push(State {
        id: 28,
        name: "Ownership Test State".to_owned(),
        manpower: 2_000_000,
        owner: CountryTag::new("GER"),
        cores: vec![CountryTag::new("GER")],
        provinces: vec![1],
        category: "metropolis".to_owned(),
        infrastructure: 8,
        victory_points: vec![],
        resources: vec![],
    });

    let mut world = World::new(test_map(1), Arc::new(data));
    hoi4_logic::economy::init_world(&mut world);
    let db = V6Database::load();
    inject_v6_into_world(&mut world, &db);
    world.countries.buildings_v6.buildings.clear();
    for slots in &mut world.states.category_slots {
        *slots = 100;
    }
    let country = world.country("GER").expect("GER should exist");
    (world, db, country, StateId(0))
}

fn complete_first_item(
    world: &mut World,
    econ: &mut EconomyState,
    db: &V6Database,
    country: CountryId,
) {
    let item = &mut econ.construction[country.0 as usize].items[0];
    item.cost = 1.0;
    item.progress = 1.0;
    construction_tick::run(world, econ, db, country.0 as usize);
}

fn completed_owner(world: &World, building_id: &str, state: StateId) -> BuildingOwner {
    world
        .countries
        .buildings_v6
        .buildings
        .iter()
        .find(|building| building.building_def_id == building_id && building.state == state)
        .map(|building| building.owner)
        .expect("completed building should exist")
}

fn completed_building_mut<'a>(
    world: &'a mut World,
    building_id: &str,
    state: StateId,
) -> &'a mut hoi4_state::Building {
    world
        .countries
        .buildings_v6
        .buildings
        .iter_mut()
        .find(|building| building.building_def_id == building_id && building.state == state)
        .expect("completed building should exist")
}

#[test]
fn construction_item_records_funding_source_and_completion_owner() {
    let (world, _db, country, state) = ownership_world();
    let mut econ = EconomyState::new(&world);

    econ.enqueue_construction(country, BuildOrder::new("steel_mill", state), &world);
    let item = &econ.construction[country.0 as usize].items[0];

    assert_eq!(item.funding_source, ConstructionFundingSource::Government);
    assert_eq!(item.owner_on_completion, BuildingOwner::State);
    assert_eq!(item.reserved_funds_rm, 0.0);
}

#[test]
fn government_project_completes_as_state_owner() {
    let (mut world, db, country, state) = ownership_world();
    let mut econ = EconomyState::new(&world);

    econ.enqueue_construction(country, BuildOrder::new("steel_mill", state), &world);
    complete_first_item(&mut world, &mut econ, &db, country);

    assert_eq!(
        completed_owner(&world, "steel_mill", state),
        BuildingOwner::State
    );
}

#[test]
fn private_pool_project_completes_as_private_owner() {
    let (mut world, db, country, state) = ownership_world();
    let mut econ = EconomyState::new(&world);
    let order = BuildOrder::new("arms_industry", state).with_funding(
        ConstructionFundingSource::PrivatePool,
        BuildingOwner::Private,
        250_000_000.0,
    );

    econ.enqueue_construction(country, order, &world);
    complete_first_item(&mut world, &mut econ, &db, country);

    assert_eq!(
        completed_owner(&world, "arms_industry", state),
        BuildingOwner::Private
    );
}

#[test]
fn cartel_project_completes_as_cartel_owner() {
    let (mut world, db, country, state) = ownership_world();
    let mut econ = EconomyState::new(&world);
    let order = BuildOrder::new("arms_industry", state).with_funding(
        ConstructionFundingSource::CartelPool,
        BuildingOwner::Cartel,
        250_000_000.0,
    );

    econ.enqueue_construction(country, order, &world);
    complete_first_item(&mut world, &mut econ, &db, country);

    assert_eq!(
        completed_owner(&world, "arms_industry", state),
        BuildingOwner::Cartel
    );
}

#[test]
fn country_store_has_v8_investment_accounts() {
    let (world, _db, country, _state) = ownership_world();

    let accounts: Vec<_> = world
        .countries
        .investment_accounts
        .iter()
        .filter(|account| account.country == country)
        .map(|account| account.account_kind)
        .collect();

    assert!(accounts.contains(&InvestmentAccountKind::Private));
    assert!(accounts.contains(&InvestmentAccountKind::Cartel));
    assert!(accounts.contains(&InvestmentAccountKind::StateDevelopmentBank));
    assert!(accounts.contains(&InvestmentAccountKind::ColonialExtraction));
    assert!(accounts.contains(&InvestmentAccountKind::ForeignCapital));
}

#[test]
fn completed_private_project_gets_default_private_ownership_share() {
    let (mut world, db, country, state) = ownership_world();
    let mut econ = EconomyState::new(&world);
    let order = BuildOrder::new("textile_mill", state).with_funding(
        ConstructionFundingSource::PrivatePool,
        BuildingOwner::Private,
        250_000_000.0,
    );

    econ.enqueue_construction(country, order, &world);
    complete_first_item(&mut world, &mut econ, &db, country);

    let building = completed_building_mut(&mut world, "textile_mill", state);
    assert_eq!(building.ownership_shares.len(), 1);
    assert_eq!(
        building.ownership_shares[0].account,
        OwnershipAccount::DomesticPrivate { country }
    );
    assert!((building.ownership_shares[0].share - 1.0).abs() < f32::EPSILON);
}

#[test]
fn profit_distribution_follows_ownership_shares() {
    let (mut world, db, country, state) = ownership_world();
    let mut econ = EconomyState::new(&world);
    let order = BuildOrder::new("textile_mill", state).with_funding(
        ConstructionFundingSource::PrivatePool,
        BuildingOwner::Private,
        250_000_000.0,
    );

    econ.enqueue_construction(country, order, &world);
    complete_first_item(&mut world, &mut econ, &db, country);
    let building = completed_building_mut(&mut world, "textile_mill", state);
    building.level = 5;
    building.employment = [100_000; 6];
    building.ownership_shares = vec![
        OwnershipShare {
            account: OwnershipAccount::DomesticPrivate { country },
            share: 0.60,
        },
        OwnershipShare {
            account: OwnershipAccount::State { country },
            share: 0.40,
        },
    ];

    let treasury_before = world.countries.treasury.treasuries[country.0 as usize].cash_rm;
    let private_before = world
        .countries
        .investment_balance_rm(country, InvestmentAccountKind::Private);

    finance_tick::step_collect_taxes(&mut world, &db, country.0 as usize);

    let treasury_after = world.countries.treasury.treasuries[country.0 as usize].cash_rm;
    let private_after = world
        .countries
        .investment_balance_rm(country, InvestmentAccountKind::Private);

    assert!(treasury_after > treasury_before);
    assert!(private_after > private_before);
}
