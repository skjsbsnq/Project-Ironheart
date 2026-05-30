//! V6 construction queue progress: queued orders become real `buildings_v6` buildings.
//!
//! P0.9: 建造进度受 CP、资金、材料三重约束。
//! - 政府项目每日从国库扣建造资金，现金不足时降速或停止
//! - 私人项目每日从投资池扣建造资金，余额不足时降速或停止
//! - 材料由市场清算层提供，短缺降低有效 CP
//! - 取消项目按进度退款或转为沉没成本

use hoi4_content::v6_loader::{BuildingDef, BuildingKindDef, V6Database};
use hoi4_state::{
    Building, BuildingKind, CountryId, InvestmentAccountKind, LawCategory, StateId, World,
};

use super::{ConstructionFundingSource, ConstructionItem, EconomyState, MaterialNeed};

const DEFAULT_BUILD_COST: f32 = 7_200.0;
const BASE_CP_POOL: f32 = 75.0;
const CP_PER_CONSTRUCTION_SECTOR_LEVEL: f32 = 30.0;
const DAILY_FUND_RATIO: f64 = 0.015;
const STEEL_PER_CP: f32 = 0.02;
const MACHINERY_PER_CP: f32 = 0.01;

pub fn run(world: &mut World, econ: &mut EconomyState, db: &V6Database, ci: usize) {
    if ci >= econ.construction.len() || ci >= world.countries.count {
        return;
    }
    if econ.construction[ci].items.is_empty() {
        return;
    }

    let Some(building_def_id) = econ.construction[ci]
        .items
        .first()
        .map(|item| item.building_key.clone())
    else {
        return;
    };
    let Some(building_def) = db.buildings.iter().find(|def| def.id == building_def_id) else {
        econ.construction[ci].items.remove(0);
        return;
    };

    let target_state = econ.construction[ci].items[0].target_state;
    if validate_build_location(world, db, ci, building_def, target_state).is_err() {
        return;
    }

    let item = &mut econ.construction[ci].items[0];
    if item.cost <= 0.0 {
        item.cost = construction_cost(building_def.max_level);
    }
    if item.budget_needed_rm <= 0.0 {
        item.budget_needed_rm = construction_budget(item.cost);
    }
    if item.material_needs.is_empty() {
        item.material_needs = compute_material_needs(item.cost);
    }

    let base_cp_progress = daily_construction_progress(world, db, ci, target_state);
    if base_cp_progress <= 0.0 {
        return;
    }

    let material_ratio = compute_material_fulfillment(world, ci, &econ.construction[ci].items[0]);
    request_construction_materials(world, ci, &econ.construction[ci].items[0], material_ratio);

    let fund_ratio = pay_daily_construction_funds(world, econ, ci);
    let effective_cp = base_cp_progress * material_ratio.min(fund_ratio);

    if effective_cp <= 0.0 {
        return;
    }

    econ.construction[ci].items[0].progress += effective_cp;

    update_material_consumption(
        world,
        ci,
        &mut econ.construction[ci].items[0],
        material_ratio,
        effective_cp,
        base_cp_progress,
    );

    if !econ.construction[ci].items[0].is_complete() {
        return;
    }

    let item = econ.construction[ci].items.remove(0);
    complete_item(world, db, ci, item);
    econ.construction[ci].total_completed += 1;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BuildBlockReason {
    InvalidCountry,
    InvalidState,
    NotOwned,
    LawBlocked,
    NoResourceDeposit,
    DepositExhausted,
    RequiresTech,
    RequiresCoast,
    NoUrbanCapacity,
}

pub fn validate_build_location(
    world: &World,
    db: &V6Database,
    ci: usize,
    building_def: &BuildingDef,
    state: StateId,
) -> Result<(), BuildBlockReason> {
    if ci >= world.countries.count {
        return Err(BuildBlockReason::InvalidCountry);
    }
    let state_idx = state.0 as usize;
    if state_idx >= world.states.count {
        return Err(BuildBlockReason::InvalidState);
    }
    if !country_owns_state(world, ci, state) {
        return Err(BuildBlockReason::NotOwned);
    }
    if !requires_law_satisfied(world, ci, building_def.requires_law.as_ref()) {
        return Err(BuildBlockReason::LawBlocked);
    }

    match building_def.state_limit_kind.as_deref() {
        None => Ok(()),
        Some("coastal") => {
            if state_is_coastal(world, state) {
                Ok(())
            } else {
                Err(BuildBlockReason::RequiresCoast)
            }
        }
        Some("urban") => {
            if world.states.category_slots[state_idx] >= 6 {
                Ok(())
            } else {
                Err(BuildBlockReason::NoUrbanCapacity)
            }
        }
        Some(resource_kind) => {
            validate_resource_deposit(world, db, ci, building_def, state, resource_kind)
        }
    }
}

fn validate_resource_deposit(
    world: &World,
    db: &V6Database,
    ci: usize,
    building_def: &BuildingDef,
    state: StateId,
    resource_kind: &str,
) -> Result<(), BuildBlockReason> {
    let game_state_id = state_game_id(world, state);
    let deposit = db
        .state_resource_deposits
        .iter()
        .find(|entry| entry.state_id == game_state_id || entry.state_id == state.0)
        .and_then(|entry| {
            entry
                .deposits
                .iter()
                .find(|deposit| deposit.good_id == resource_kind)
        })
        .ok_or(BuildBlockReason::NoResourceDeposit)?;

    if let Some(required_tech) = &deposit.requires_tech {
        if !world.countries.completed_techs[ci].contains(required_tech) {
            return Err(BuildBlockReason::RequiresTech);
        }
    }

    let existing_level: u16 = world
        .countries
        .buildings_v6
        .buildings
        .iter()
        .filter(|building| {
            building.state == state
                && building.building_def_id == building_def.id
                && building.level > 0
        })
        .map(|building| building.level as u16)
        .sum();
    if existing_level >= deposit.potential_level as u16 {
        return Err(BuildBlockReason::DepositExhausted);
    }

    Ok(())
}

fn state_game_id(world: &World, state: StateId) -> u16 {
    world
        .state_id_lookup
        .iter()
        .find_map(|(game_id, internal)| (*internal == state).then_some(*game_id))
        .unwrap_or(state.0)
}

fn state_is_coastal(world: &World, state: StateId) -> bool {
    let state_idx = state.0 as usize;
    if state_idx >= world.states.count {
        return false;
    }
    world.states.provinces[state_idx].iter().any(|province| {
        world
            .map
            .definitions
            .get(province.0 as usize)
            .and_then(|definition| definition.as_ref())
            .map(|definition| definition.coastal)
            .unwrap_or(false)
    })
}

fn daily_construction_progress(world: &World, db: &V6Database, ci: usize, state: StateId) -> f32 {
    let state_idx = state.0 as usize;
    let infra = if state_idx < world.states.count {
        world.states.infrastructure[state_idx] as f32
    } else {
        0.0
    };
    let infra_mult = 1.0 + 0.08 * infra;
    let law_mult = economy_construction_speed(world, db, ci);
    construction_cp_pool(world, ci) * infra_mult * law_mult
}

pub fn construction_cp_pool(world: &World, ci: usize) -> f32 {
    let country = CountryId(ci as u16);
    let mut cp = BASE_CP_POOL;
    for building in &world.countries.buildings_v6.buildings {
        let state_idx = building.state.0 as usize;
        if state_idx >= world.states.count || world.states.owners[state_idx] != country {
            continue;
        }
        if building.building_def_id == "construction_sector" {
            cp += building.level as f32 * CP_PER_CONSTRUCTION_SECTOR_LEVEL;
        }
    }
    cp
}

fn economy_construction_speed(world: &World, db: &V6Database, ci: usize) -> f32 {
    let law = world.countries.law_store.law_sets[ci].0[LawCategory::Economy.index()]
        .current
        .as_str();
    1.0 + db
        .economy_laws
        .iter()
        .find(|def| def.id == law)
        .map(|def| def.construction_speed_modifier)
        .unwrap_or(0.0)
}

pub fn construction_cost(max_level: u8) -> f32 {
    DEFAULT_BUILD_COST * (1.0 + max_level as f32 * 0.02)
}

pub fn construction_budget(cp_cost: f32) -> f64 {
    cp_cost as f64 * 50_000.0
}

pub fn compute_material_needs(cp_cost: f32) -> Vec<MaterialNeed> {
    vec![
        MaterialNeed {
            good_id: "steel".to_owned(),
            total_needed: cp_cost * STEEL_PER_CP,
            consumed: 0.0,
        },
        MaterialNeed {
            good_id: "machinery".to_owned(),
            total_needed: cp_cost * MACHINERY_PER_CP,
            consumed: 0.0,
        },
    ]
}

fn compute_material_fulfillment(world: &World, ci: usize, item: &ConstructionItem) -> f32 {
    if item.material_needs.is_empty() {
        return 1.0;
    }
    let sheet = &world.countries.market.markets[ci].clearing_sheet;
    let mut total_weight = 0.0_f32;
    let mut total_fulfilled_weight = 0.0_f32;
    for need in &item.material_needs {
        if need.total_needed <= 0.0 {
            continue;
        }
        let remaining_need = (need.total_needed - need.consumed).max(0.0);
        if remaining_need <= 0.0 {
            total_weight += 1.0;
            total_fulfilled_weight += 1.0;
            continue;
        }
        let clearing_ratio = if let Some(result) = sheet.results.get(&need.good_id) {
            let constr_bucket = result
                .buckets
                .iter()
                .find(|b| b.kind == hoi4_state::market::DemandBucketKind::ConstructionInput);
            if let Some(bucket) = constr_bucket {
                if bucket.requested > 0.0 {
                    (bucket.fulfilled / bucket.requested).clamp(0.0, 1.0)
                } else {
                    1.0
                }
            } else if result.shortage_ratio > 0.0 {
                1.0 - result.shortage_ratio
            } else {
                1.0
            }
        } else {
            1.0
        };
        let needed_ratio = remaining_need / need.total_needed;
        total_weight += needed_ratio;
        total_fulfilled_weight += needed_ratio * clearing_ratio;
    }
    if total_weight <= 0.0 {
        1.0
    } else {
        (total_fulfilled_weight / total_weight).clamp(0.0, 1.0)
    }
}

fn request_construction_materials(
    world: &mut World,
    ci: usize,
    item: &ConstructionItem,
    _fulfillment: f32,
) {
    for need in &item.material_needs {
        let remaining = (need.total_needed - need.consumed).max(0.0);
        if remaining <= 0.0 {
            continue;
        }
        let daily_need = remaining / ((item.cost - item.progress).max(1.0) / 5.0).max(1.0);
        let market = &mut world.countries.market.markets[ci];
        *market.demand.entry(need.good_id.clone()).or_insert(0.0) += daily_need;
        market.bucket_demand.add(
            &need.good_id,
            hoi4_state::market::DemandBucketKind::ConstructionInput,
            daily_need,
        );
    }
}

fn pay_daily_construction_funds(world: &mut World, econ: &EconomyState, ci: usize) -> f32 {
    if ci >= econ.construction.len() || econ.construction[ci].items.is_empty() {
        return 1.0;
    }
    let item = &econ.construction[ci].items[0];
    let remaining_budget = item.funds_remaining_rm();
    if remaining_budget <= 0.0 {
        return 1.0;
    }
    let daily_payment = (remaining_budget * DAILY_FUND_RATIO).max(1.0);

    match item.funding_source {
        ConstructionFundingSource::Government | ConstructionFundingSource::Mefo => {
            let treasury = &world.countries.treasury.treasuries[ci];
            let available = treasury.cash_rm.max(0.0);
            if available <= 0.0 {
                return 0.0;
            }
            (available / daily_payment).min(1.0) as f32
        }
        ConstructionFundingSource::PrivatePool => {
            let pool_balance = world
                .countries
                .investment_balance_rm(CountryId(ci as u16), InvestmentAccountKind::Private);
            if pool_balance <= 0.0 {
                return 0.0;
            }
            (pool_balance / daily_payment).min(1.0) as f32
        }
        ConstructionFundingSource::CartelPool => {
            let pool_balance = world
                .countries
                .investment_balance_rm(CountryId(ci as u16), InvestmentAccountKind::Cartel);
            if pool_balance <= 0.0 {
                return 0.0;
            }
            (pool_balance / daily_payment).min(1.0) as f32
        }
        ConstructionFundingSource::OverlordInvestment { .. }
        | ConstructionFundingSource::ForeignInvestment { .. } => {
            let treasury = &world.countries.treasury.treasuries[ci];
            let available = treasury.cash_rm.max(0.0);
            if available <= 0.0 {
                return 0.0;
            }
            (available / daily_payment).min(1.0) as f32
        }
    }
}

fn update_material_consumption(
    world: &World,
    ci: usize,
    item: &mut ConstructionItem,
    material_ratio: f32,
    effective_cp: f32,
    base_cp: f32,
) {
    if material_ratio <= 0.0 || base_cp <= 0.0 {
        return;
    }
    let progress_share = effective_cp / base_cp;
    for need in &mut item.material_needs {
        let remaining = (need.total_needed - need.consumed).max(0.0);
        if remaining <= 0.0 {
            continue;
        }
        let daily_consumption = remaining * progress_share * 0.05;
        let sheet = &world.countries.market.markets[ci].clearing_sheet;
        let actual_ratio = if let Some(result) = sheet.results.get(&need.good_id) {
            let constr_bucket = result
                .buckets
                .iter()
                .find(|b| b.kind == hoi4_state::market::DemandBucketKind::ConstructionInput);
            if let Some(bucket) = constr_bucket {
                if bucket.requested > 0.0 {
                    (bucket.fulfilled / bucket.requested).clamp(0.0, 1.0)
                } else {
                    1.0
                }
            } else {
                1.0
            }
        } else {
            1.0
        };
        need.consumed = (need.consumed + daily_consumption * actual_ratio).min(need.total_needed);
    }
}

fn complete_item(world: &mut World, db: &V6Database, ci: usize, item: ConstructionItem) {
    let Some(building_def) = db.buildings.iter().find(|def| def.id == item.building_key) else {
        return;
    };
    let target_level = if item.target_level == 0 {
        1
    } else {
        item.target_level
    };

    if let Some(existing) = world
        .countries
        .buildings_v6
        .buildings
        .iter_mut()
        .find(|building| {
            building.state == item.target_state && building.building_def_id == item.building_key
        })
    {
        if existing.level < building_def.max_level {
            existing.level = existing
                .level
                .saturating_add(1)
                .min(target_level.max(existing.level + 1))
                .min(building_def.max_level);
        }
        return;
    }

    world.countries.buildings_v6.buildings.push(Building {
        kind: map_building_kind(building_def.kind),
        building_def_id: building_def.id.clone(),
        state: item.target_state,
        level: 1.min(building_def.max_level),
        active_pm: default_pm_id(db, &building_def.id),
        active_pm_by_group: default_active_pms(db, &building_def.id),
        employment: [0; 6],
        owner: item.owner_on_completion,
        ownership_shares: Building::default_ownership_shares(
            item.owner_on_completion,
            CountryId(ci as u16),
        ),
        requires_law: building_def
            .requires_law
            .as_ref()
            .map(|(cat, law)| (map_law_category(*cat), law.clone())),
        max_level: building_def.max_level,
        cp_cost: construction_cost(building_def.max_level),
        built_progress: 1.0,
        ..Building::runtime_defaults()
    });

    let _ = ci;
}

fn default_pm_id(db: &V6Database, building_id: &str) -> String {
    db.production_methods
        .iter()
        .find(|pm| pm.building_id == building_id && pm.id.ends_with("default"))
        .map(|pm| pm.id.clone())
        .unwrap_or_else(|| format!("{building_id}_default"))
}

fn default_active_pms(
    db: &V6Database,
    building_id: &str,
) -> Vec<hoi4_state::ActiveProductionMethod> {
    hoi4_content::default_pms_for_building(db, building_id)
        .into_iter()
        .map(|pm| hoi4_state::ActiveProductionMethod {
            group: hoi4_content::production_method_group(pm).to_owned(),
            pm_id: pm.id.clone(),
        })
        .collect()
}

fn country_owns_state(world: &World, ci: usize, state: StateId) -> bool {
    let state_idx = state.0 as usize;
    state_idx < world.states.count && world.states.owners[state_idx] == CountryId(ci as u16)
}

fn requires_law_satisfied(
    world: &World,
    ci: usize,
    requirement: Option<&(hoi4_content::v6_loader::LawCategoryDef, String)>,
) -> bool {
    let Some((category, law_id)) = requirement else {
        return true;
    };
    world.countries.law_store.law_sets[ci].0[map_law_category(*category).index()].current == *law_id
}

fn map_building_kind(kind: BuildingKindDef) -> BuildingKind {
    match kind {
        BuildingKindDef::Resource => BuildingKind::Resource,
        BuildingKindDef::Industrial => BuildingKind::Industrial,
        BuildingKindDef::Agriculture => BuildingKind::Agriculture,
        BuildingKindDef::ConsumerGoods => BuildingKind::ConsumerGoods,
        BuildingKindDef::Service => BuildingKind::Service,
        BuildingKindDef::Military => BuildingKind::Military,
        BuildingKindDef::Infrastructure => BuildingKind::Infrastructure,
        BuildingKindDef::MilitaryBase => BuildingKind::MilitaryBase,
    }
}

fn map_law_category(category: hoi4_content::v6_loader::LawCategoryDef) -> LawCategory {
    match category {
        hoi4_content::v6_loader::LawCategoryDef::Conscription => LawCategory::Conscription,
        hoi4_content::v6_loader::LawCategoryDef::Economy => LawCategory::Economy,
        hoi4_content::v6_loader::LawCategoryDef::Trade => LawCategory::Trade,
        hoi4_content::v6_loader::LawCategoryDef::Taxation => LawCategory::Taxation,
        hoi4_content::v6_loader::LawCategoryDef::CivilRights => LawCategory::CivilRights,
        hoi4_content::v6_loader::LawCategoryDef::InformationControl => {
            LawCategory::InformationControl
        }
    }
}
