use std::collections::HashMap;

use hoi4_content::v6_loader::LawCategoryDef;
use hoi4_logic::economy::EconomyState;
use hoi4_state::{CountryId, World};

use super::names::{DisplayNameKind, DisplayNameResolver};

pub fn build_logistics_panel_data(
    world: &World,
    db: &hoi4_content::V6Database,
    econ: &mut EconomyState,
    player: usize,
) -> Option<hoi4_ui::logistics_panel::LogisticsData> {
    if player >= econ.stockpile.len() || player >= world.countries.treasury.treasuries.len() {
        return None;
    }

    hoi4_logic::economy::stockpile::normalize_stockpile_keys(&mut econ.stockpile[player]);

    let stockpile = &econ.stockpile[player];
    let (daily_outputs, production_sources) = logistics_v6_military_outputs(world, db, player);
    let (force_need, replenishment_need) = logistics_force_needs(world, player);
    let training_shortfalls = logistics_training_shortfalls(world, econ, player);
    let treasury = &world.countries.treasury.treasuries[player];
    let mut ids = logistics_equipment_ids(db);
    for id in stockpile
        .keys()
        .chain(daily_outputs.keys())
        .chain(force_need.keys())
        .chain(training_shortfalls.keys())
    {
        if !ids.contains(id) {
            ids.push(id.clone());
        }
    }
    let total_shortfall_value: f64 = ids
        .iter()
        .map(|id| {
            let daily_prod = daily_outputs.get(id).copied().unwrap_or(0.0);
            let daily_replenishment_need = replenishment_need.get(id).copied().unwrap_or(0.0);
            let total_equipment_need = force_need.get(id).copied().unwrap_or(0.0);
            let daily_maintenance_need = total_equipment_need * 0.0005;
            let daily_consumption = daily_replenishment_need + daily_maintenance_need;
            let daily_deficit = (daily_consumption - daily_prod).max(0.0);
            daily_deficit as f64 * logistics_equipment_unit_cost_rm(id)
        })
        .sum();
    let mut entries: Vec<hoi4_ui::logistics_panel::LogisticsEntry> = ids
        .into_iter()
        .map(|id| {
            let qty = stockpile.get(&id).copied().unwrap_or(0.0);
            let daily_prod = daily_outputs.get(&id).copied().unwrap_or(0.0);
            let daily_replenishment_need = replenishment_need.get(&id).copied().unwrap_or(0.0);
            let total_equipment_need = force_need.get(&id).copied().unwrap_or(0.0);
            let training_shortfall = training_shortfalls.get(&id).copied().unwrap_or(0.0);
            let display_qty = qty - training_shortfall;
            let daily_training_need = training_shortfall / 30.0;
            let daily_maintenance_need = total_equipment_need * 0.0005;
            let daily_consumption =
                daily_replenishment_need + daily_training_need + daily_maintenance_need;
            let net_change = daily_prod - daily_consumption;
            let deficit = (daily_consumption - daily_prod).max(0.0);
            let days_until_empty = if net_change < 0.0 && display_qty > 0.0 {
                Some(display_qty / -net_change)
            } else if deficit > 0.0 && qty <= 0.0 {
                None
            } else {
                None
            };
            let shortfall_value = deficit as f64 * logistics_equipment_unit_cost_rm(&id);
            let procurement_rm = if total_shortfall_value > 0.0 {
                treasury.daily_budget.expense_military_procurement_rm * shortfall_value
                    / total_shortfall_value
            } else {
                0.0
            };
            hoi4_ui::logistics_panel::LogisticsEntry {
                name: equipment_display_name(&id),
                stockpile: display_qty,
                daily_production: daily_prod,
                daily_replenishment_need,
                daily_training_need,
                daily_maintenance_need,
                daily_consumption,
                net_change,
                deficit,
                days_until_empty,
                procurement_rm,
                production_sources: production_sources.get(&id).cloned().unwrap_or_default(),
            }
        })
        .collect();
    entries.sort_by(|a, b| {
        b.deficit
            .partial_cmp(&a.deficit)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.name.cmp(&b.name))
    });
    let deficit_types = entries.iter().filter(|e| e.deficit > 0.0).count();
    let total_daily_production = entries.iter().map(|e| e.daily_production).sum();
    let total_daily_need = entries.iter().map(|e| e.daily_consumption).sum();
    let resources = logistics_resource_entries(world, player);
    Some(hoi4_ui::logistics_panel::LogisticsData {
        total_types: entries.len(),
        deficit_types,
        total_daily_production,
        total_daily_need,
        military_procurement_rm: treasury.daily_budget.expense_military_procurement_rm,
        military_maintenance_rm: treasury.daily_budget.expense_military_maintenance_rm,
        entries,
        resources,
    })
}

pub fn panel_data(
    world: &World,
    db: &hoi4_content::V6Database,
    econ: &mut EconomyState,
    player: usize,
) -> Option<hoi4_ui::logistics_panel::LogisticsData> {
    build_logistics_panel_data(world, db, econ, player)
}

fn equipment_display_name(equipment_id: &str) -> String {
    match equipment_id {
        "infantry_equipment" => "步兵装备".to_owned(),
        "artillery" => "火炮".to_owned(),
        "anti_tank" => "反坦克炮".to_owned(),
        "anti_air" => "防空炮".to_owned(),
        "support_equipment" => "支援装备".to_owned(),
        "motorized" => "摩托化装备".to_owned(),
        "mechanized" => "机械化装备".to_owned(),
        "armor" => "装甲车辆".to_owned(),
        "aircraft" => "飞机".to_owned(),
        "naval_vessel" => "舰艇".to_owned(),
        "convoy" => "运输船".to_owned(),
        "train" => "火车".to_owned(),
        _ => equipment_id.to_owned(),
    }
}

fn logistics_equipment_ids(db: &hoi4_content::V6Database) -> Vec<String> {
    const ORDER: [&str; 12] = [
        "infantry_equipment",
        "artillery",
        "anti_tank",
        "anti_air",
        "support_equipment",
        "motorized",
        "mechanized",
        "armor",
        "aircraft",
        "naval_vessel",
        "convoy",
        "train",
    ];
    let mut ids: Vec<String> = ORDER.into_iter().map(str::to_owned).collect();
    for pm in &db.production_methods {
        if let Some(eq) = &pm.equipment_output {
            if !ids.contains(&eq.equipment_category) {
                ids.push(eq.equipment_category.clone());
            }
        }
    }
    ids
}

fn logistics_equipment_unit_cost_rm(equipment_id: &str) -> f64 {
    match equipment_id.to_ascii_lowercase().as_str() {
        e if e.contains("tank") || e.contains("armor") => 80_000.0,
        e if e.contains("fighter")
            || e.contains("bomber")
            || e.contains("plane")
            || e.contains("aircraft") =>
        {
            120_000.0
        }
        e if e.contains("artillery") => 18_000.0,
        e if e.contains("truck") || e.contains("motorized") => 12_000.0,
        e if e.contains("support") => 8_000.0,
        e if e.contains("ship") || e.contains("naval") => 250_000.0,
        e if e.contains("convoy") => 60_000.0,
        e if e.contains("train") => 40_000.0,
        _ => 4_000.0,
    }
}

fn logistics_building_output_ratio(
    building: &hoi4_state::Building,
    pms: &[&hoi4_content::ProductionMethodDef],
    market: &hoi4_state::market::NationalMarket,
) -> f32 {
    if building.level == 0 {
        return 0.0;
    }
    let mut total_needed = 0.0;
    let mut total_filled = 0.0;
    for class_idx in 0..hoi4_state::PopClass::COUNT {
        let needed = pms
            .iter()
            .map(|pm| pm.employment_demand.get(class_idx).copied().unwrap_or(0) as f32)
            .sum::<f32>()
            * building.level as f32;
        let filled = building.employment.get(class_idx).copied().unwrap_or(0) as f32;
        total_needed += needed;
        total_filled += filled.min(needed);
    }
    let employment_ratio = if total_needed <= 0.0 {
        1.0
    } else if total_filled <= 0.0 {
        1.0
    } else {
        (total_filled / total_needed).clamp(0.0, 1.0)
    };
    let market_empty = market.supply.values().all(|v| v.abs() <= f32::EPSILON)
        && market.stockpile.values().all(|v| v.abs() <= f32::EPSILON);
    let input_ratio = pms
        .iter()
        .flat_map(|pm| {
            pm.input_good_ids
                .iter()
                .enumerate()
                .map(move |(i, good_id)| (pm, i, good_id))
        })
        .fold(1.0_f32, |ratio, (pm, i, good_id)| {
            let demand = pm.input_good_amounts.get(i).copied().unwrap_or(0.0)
                * building.level as f32
                * employment_ratio;
            if demand <= 0.0 || market_empty {
                ratio
            } else {
                let available = market.supply.get(good_id).copied().unwrap_or(0.0)
                    + market.stockpile.get(good_id).copied().unwrap_or(0.0);
                ratio.min((available / demand).clamp(0.0, 1.0))
            }
        });
    employment_ratio * input_ratio
}

fn logistics_resource_entries(
    world: &World,
    player: usize,
) -> Vec<hoi4_ui::logistics_panel::ResourceEntry> {
    let market = world.countries.market.markets.get(player);
    let country_id = CountryId(player as u16);
    hoi4_data::ResourceKind::all()
        .iter()
        .map(|k| {
            let key = k.as_str();
            let market_produced = market
                .and_then(|m| m.supply.get(key).copied())
                .unwrap_or(0.0)
                + market
                    .and_then(|m| m.imports.get(key).copied())
                    .unwrap_or(0.0);
            let static_produced = world
                .data
                .states
                .iter()
                .enumerate()
                .filter(|(idx, _)| world.states.owners.get(*idx) == Some(&country_id))
                .flat_map(|(_, state)| state.resources.iter())
                .filter(|(kind, _)| kind == k)
                .map(|(_, amount)| *amount)
                .sum::<f32>();
            let produced = if market_produced > 0.0 {
                market_produced
            } else {
                static_produced
            };
            let consumed = market
                .and_then(|m| m.demand.get(key).copied())
                .unwrap_or(0.0)
                + market
                    .and_then(|m| m.exports.get(key).copied())
                    .unwrap_or(0.0);
            let stored = market
                .and_then(|m| m.stockpile.get(key).copied())
                .unwrap_or(0.0);
            hoi4_ui::logistics_panel::ResourceEntry {
                name: key.to_owned(),
                produced,
                consumed,
                stored,
            }
        })
        .collect()
}

fn logistics_v6_military_outputs(
    world: &World,
    db: &hoi4_content::V6Database,
    player: usize,
) -> (HashMap<String, f32>, HashMap<String, Vec<String>>) {
    let country_id = CountryId(player as u16);
    let completed_techs = world.countries.completed_techs[player].clone();
    let Some(market) = world.countries.market.markets.get(player) else {
        return (HashMap::new(), HashMap::new());
    };
    let mut outputs: HashMap<String, f32> = HashMap::new();
    let mut sources: HashMap<String, Vec<String>> = HashMap::new();
    for building in &world.countries.buildings_v6.buildings {
        let state_idx = building.state.0 as usize;
        if state_idx >= world.states.count
            || world.states.owners[state_idx] != country_id
            || building.level == 0
        {
            continue;
        }
        let pms = hoi4_content::active_pms_for_building(building, db)
            .into_iter()
            .filter(|pm| {
                let tech_ok = pm
                    .unlocked_by
                    .as_ref()
                    .map(|tech| completed_techs.contains(tech))
                    .unwrap_or(true);
                let law_ok = pm
                    .required_law
                    .as_ref()
                    .map(|(cat, law_id)| {
                        world.countries.law_store.law_sets[player].0[law_category_index(*cat)]
                            .current
                            == *law_id
                    })
                    .unwrap_or(true);
                tech_ok && law_ok
            })
            .collect::<Vec<_>>();
        if pms.iter().all(|pm| pm.equipment_output.is_none()) {
            continue;
        }
        let ratio = logistics_building_output_ratio(building, &pms, market);
        let state_name = world
            .states
            .names
            .get(state_idx)
            .cloned()
            .unwrap_or_default();
        let building_name = building_name(db, &building.building_def_id);
        for pm in pms {
            let Some(eq) = &pm.equipment_output else {
                continue;
            };
            let daily = eq.daily_per_level
                * pm.throughput_modifier.max(0.0)
                * building.level as f32
                * ratio;
            if daily <= 0.0 {
                continue;
            }
            *outputs.entry(eq.equipment_category.clone()).or_insert(0.0) += daily;
            let source = if state_name.is_empty() {
                format!("{} Lv{} +{:.1}/day", building_name, building.level, daily)
            } else {
                format!(
                    "{} {} Lv{} +{:.1}/day",
                    state_name, building_name, building.level, daily
                )
            };
            sources
                .entry(eq.equipment_category.clone())
                .or_default()
                .push(source);
        }
    }
    for rows in sources.values_mut() {
        rows.truncate(3);
    }
    (outputs, sources)
}

fn logistics_force_needs(
    world: &World,
    player: usize,
) -> (HashMap<String, f32>, HashMap<String, f32>) {
    let country_id = CountryId(player as u16);
    let tag = world.country_tag(country_id).map(|s| s.to_owned());
    let templates = tag
        .as_deref()
        .and_then(|t| world.data.division_templates.get(t));
    let mut total_need: HashMap<String, f32> = HashMap::new();
    let mut replenishment_need: HashMap<String, f32> = HashMap::new();
    let Some(templates) = templates else {
        return (total_need, replenishment_need);
    };
    for div_idx in 0..world.divisions.count {
        if world.divisions.owners[div_idx] != country_id {
            continue;
        }
        let tpl_idx = world.divisions.template_indices[div_idx] as usize;
        let Some(template) = templates.get(tpl_idx) else {
            continue;
        };
        let strength = world
            .divisions
            .strength
            .get(div_idx)
            .copied()
            .unwrap_or(1.0)
            .clamp(0.0, 1.0);
        let gap = 1.0 - strength;
        for (equipment, qty) in
            hoi4_logic::economy::stockpile::template_equipment_needs(template, &world.data)
        {
            let qty = qty as f32;
            *total_need.entry(equipment.clone()).or_insert(0.0) += qty;
            if gap > 0.0 && !world.divisions.in_combat[div_idx] {
                *replenishment_need.entry(equipment).or_insert(0.0) += qty * gap.min(0.01);
            }
        }
    }
    (total_need, replenishment_need)
}

fn logistics_training_shortfalls(
    world: &World,
    econ: &EconomyState,
    player: usize,
) -> HashMap<String, f32> {
    let mut shortfalls: HashMap<String, f32> = HashMap::new();
    let country_id = CountryId(player as u16);
    let Some(tag) = world.country_tag(country_id) else {
        return shortfalls;
    };
    let Some(templates) = world.data.division_templates.get(tag) else {
        return shortfalls;
    };
    let Some(queue) = econ.training_queues.get(player) else {
        return shortfalls;
    };

    for item in queue {
        let Some(template) = templates.get(item.template_id as usize) else {
            continue;
        };
        for (equipment, qty) in
            hoi4_logic::economy::stockpile::template_equipment_needs(template, &world.data)
        {
            let needed = qty as f32 * item.count.max(1) as f32;
            let allocated = item
                .equipment_allocated
                .get(&equipment)
                .copied()
                .unwrap_or(0.0);
            let missing = (needed - allocated).max(0.0);
            if missing > 0.0 {
                *shortfalls.entry(equipment).or_insert(0.0) += missing;
            }
        }
    }
    shortfalls
}

fn law_category_index(category: LawCategoryDef) -> usize {
    match category {
        LawCategoryDef::Conscription => hoi4_state::LawCategory::Conscription.index(),
        LawCategoryDef::Economy => hoi4_state::LawCategory::Economy.index(),
        LawCategoryDef::Trade => hoi4_state::LawCategory::Trade.index(),
        LawCategoryDef::Taxation => hoi4_state::LawCategory::Taxation.index(),
        LawCategoryDef::CivilRights => hoi4_state::LawCategory::CivilRights.index(),
        LawCategoryDef::InformationControl => hoi4_state::LawCategory::InformationControl.index(),
    }
}

fn building_name(db: &hoi4_content::V6Database, building_id: &str) -> String {
    let name_resolver = DisplayNameResolver::new(None);
    db.buildings
        .iter()
        .find(|bd| bd.id == building_id)
        .map(|bd| name_resolver.content_name(DisplayNameKind::Building, &bd.id, &bd.name))
        .unwrap_or_else(|| {
            name_resolver.content_name(DisplayNameKind::Building, building_id, building_id)
        })
}
