//! J.4: Stockpile 闭环 — 装备需求计算、师 strength 基于装备满足度、训练消耗。
//!
//! 每日 tick 在 production 之后执行：
//! 1. 计算每个师的装备需求总量
//! 2. 从 stockpile 分配装备给师（按需求比例）
//! 3. 师 strength = min(equipment_ratio, current_strength_target)
//! 4. 训练中的师消耗 stockpile + manpower

use std::collections::HashMap;

use hoi4_data::GameData;
use hoi4_state::World;

use super::EconomyState;

/// 每日 strength 收敛速率（朝 target 靠拢 1%/天）
const STRENGTH_CONVERGENCE_RATE: f32 = 0.01;
/// 每日维护损耗。用于常规磨损、故障、训练外消耗；不足时形成负库存缺口。
const MAINTENANCE_LOSS_RATE: f32 = 0.0005;

/// V6 keeps finished equipment in `econ.stockpile` using the 12-category schema.
/// Vanilla template needs can still reference concrete equipment IDs, so normalize
/// them at every stockpile boundary.
pub fn normalize_equipment_id(id: &str) -> String {
    let key = id.to_ascii_lowercase();
    match key.as_str() {
        "infantry_equipment"
        | "infantry_equipment_0"
        | "infantry_equipment_1"
        | "infantry_equipment_2"
        | "infantry_equipment_3" => "infantry_equipment".to_owned(),
        "support_equipment" | "support_equipment_1" => "support_equipment".to_owned(),
        "artillery"
        | "artillery_equipment"
        | "artillery_equipment_1"
        | "artillery_equipment_2"
        | "artillery_equipment_3" => "artillery".to_owned(),
        "anti_tank"
        | "anti_tank_equipment"
        | "anti_tank_equipment_1"
        | "anti_tank_equipment_2"
        | "anti_tank_equipment_3" => "anti_tank".to_owned(),
        "anti_air"
        | "anti_air_equipment"
        | "anti_air_equipment_1"
        | "anti_air_equipment_2"
        | "anti_air_equipment_3" => "anti_air".to_owned(),
        "motorized" | "motorized_equipment" | "motorized_equipment_1" => "motorized".to_owned(),
        "mechanized"
        | "mechanized_equipment"
        | "mechanized_equipment_1"
        | "mechanized_equipment_2"
        | "mechanized_equipment_3" => "mechanized".to_owned(),
        "convoy" | "convoy_1" => "convoy".to_owned(),
        "train" | "train_equipment" | "train_equipment_1" => "train".to_owned(),
        _ if key.contains("tank")
            || key.contains("armor")
            || key.contains("armour")
            || key.contains("chassis") =>
        {
            "armor".to_owned()
        }
        _ if key.contains("fighter")
            || key.contains("bomber")
            || key.contains("cas")
            || key.contains("aircraft")
            || key.contains("plane") =>
        {
            "aircraft".to_owned()
        }
        _ if key.contains("ship") || key.contains("naval") => "naval_vessel".to_owned(),
        _ => id.to_owned(),
    }
}

pub fn normalize_stockpile_keys(stockpile: &mut HashMap<String, f32>) {
    let old = std::mem::take(stockpile);
    for (id, qty) in old {
        *stockpile.entry(normalize_equipment_id(&id)).or_insert(0.0) += qty;
    }
}

/// 计算一个师模板的总装备需求（所有 battalion 的 need 求和）
pub fn template_equipment_needs(
    template: &hoi4_data::DivisionTemplate,
    data: &GameData,
) -> HashMap<String, u32> {
    let mut needs: HashMap<String, u32> = HashMap::new();
    for sub_key in template.regiments.iter().chain(template.support.iter()) {
        if let Some(sub) = data.subunits.get(sub_key) {
            for (equip, &qty) in &sub.need {
                *needs.entry(normalize_equipment_id(equip)).or_insert(0) += qty;
            }
        }
    }
    needs
}

/// 每日 tick：受损师从库存补充装备，恢复 strength。
///
/// 关键设计：strength=1.0 的师视为"已满编"，不消耗库存。
/// 只有 strength < 1.0 的师才尝试从库存补充。
/// 补充速率 = 每日最多恢复 1% strength（如果库存足够）。
pub fn tick(world: &mut World, econ: &mut EconomyState, data: &GameData) {
    for stockpile in &mut econ.stockpile {
        normalize_stockpile_keys(stockpile);
    }

    let mut needs_cache: HashMap<(u16, u16), HashMap<String, u32>> = HashMap::new();
    for i in 0..world.divisions.count {
        let country_id = world.divisions.owners[i];
        if country_id.is_none() {
            continue;
        }
        let ci = country_id.0 as usize;
        if ci >= econ.stockpile.len() || ci >= world.countries.count {
            continue;
        }
        let tpl_idx = world.divisions.template_indices[i];
        let cache_key = (country_id.0, tpl_idx);
        let needs = if let Some(needs) = needs_cache.get(&cache_key) {
            needs
        } else {
            let Some(tag) = world.country_tag(country_id) else {
                continue;
            };
            let Some(templates) = data.division_templates.get(tag) else {
                continue;
            };
            let Some(template) = templates.get(tpl_idx as usize) else {
                continue;
            };
            needs_cache.insert(cache_key, template_equipment_needs(template, data));
            needs_cache.get(&cache_key).expect("inserted needs cache")
        };

        // All fielded divisions consume a small amount of equipment for maintenance.
        // Negative stockpile is intentional: it records unmet upkeep demand for procurement.
        {
            let stockpile = &mut econ.stockpile[ci];
            for (equip, qty) in needs {
                let loss = *qty as f32 * MAINTENANCE_LOSS_RATE;
                if loss > 0.0 {
                    *stockpile.entry(equip.clone()).or_insert(0.0) -= loss;
                }
            }
        }

        let cur_str = world.divisions.strength[i];
        if cur_str >= 1.0 - 0.001 {
            continue; // Already full, no replenishment needed
        }
        if world.divisions.in_combat[i] {
            continue; // Can't replenish while in combat
        }

        // Check if stockpile can support replenishment
        let stockpile = &econ.stockpile[ci];
        let mut can_replenish = true;
        for (equip, qty) in needs {
            let needed_for_tick = *qty as f32 * STRENGTH_CONVERGENCE_RATE;
            let have = stockpile.get(equip).copied().unwrap_or(0.0);
            if have < needed_for_tick {
                can_replenish = false;
                break;
            }
        }

        if can_replenish {
            // Deduct equipment for this tick's replenishment
            let stockpile = &mut econ.stockpile[ci];
            for (equip, qty) in needs {
                let consume = *qty as f32 * STRENGTH_CONVERGENCE_RATE;
                *stockpile.entry(equip.clone()).or_insert(0.0) -= consume;
            }
            // Increase strength
            world.divisions.strength[i] = (cur_str + STRENGTH_CONVERGENCE_RATE).min(1.0);
        }
        // If can't replenish, strength stays where it is (doesn't drop further)
    }
}

/// Deduct equipment from stockpile after a battle.
/// Called by the combat arbiter after each round.
///
/// `strength_loss` is the delta in strength (0..1) that the division lost this round.
/// We deduct proportional equipment from the country's stockpile.
pub fn deduct_combat_losses(
    world: &mut World,
    econ: &mut EconomyState,
    data: &GameData,
    division_idx: usize,
    strength_loss: f32,
) {
    if strength_loss <= 0.0 {
        return;
    }
    let owner = world.divisions.owners[division_idx];
    if owner.is_none() {
        return;
    }
    let ci = owner.0 as usize;
    if let Some(stockpile) = econ.stockpile.get_mut(ci) {
        normalize_stockpile_keys(stockpile);
    }
    let tag = match world.country_tag(owner) {
        Some(t) => t.to_owned(),
        None => return,
    };
    let templates = match data.division_templates.get(&tag) {
        Some(t) => t,
        None => return,
    };
    let tpl_idx = world.divisions.template_indices[division_idx] as usize;
    let Some(template) = templates.get(tpl_idx) else {
        return;
    };

    let needs = template_equipment_needs(template, data);
    let stockpile = &mut econ.stockpile[ci];

    // Deduct equipment proportional to strength loss
    for (equip, qty) in &needs {
        let loss = *qty as f32 * strength_loss;
        *stockpile.entry(equip.clone()).or_insert(0.0) -= loss;
    }

    let casualties = (super::stats_manpower(template, data) as f32 * strength_loss).round() as u32;
    if casualties > 0 {
        apply_casualty_pressure(world, owner, casualties);
    }
}

fn apply_casualty_pressure(world: &mut World, owner: hoi4_state::CountryId, casualties: u32) {
    let ci = owner.0 as usize;
    if ci >= world.countries.count {
        return;
    }
    let state_ids = world.country_state_ids(owner);
    let mut remaining_casualties = casualties;
    for pg in &mut world.countries.pops.groups {
        if remaining_casualties == 0 {
            break;
        }
        if !state_ids.contains(&pg.state) || pg.class != hoi4_state::PopClass::Soldier {
            continue;
        }
        let take = pg.size.min(remaining_casualties);
        pg.size -= take;
        remaining_casualties -= take;
    }
    let civilian_population: u64 = world
        .countries
        .pops
        .groups
        .iter()
        .filter(|pg| state_ids.contains(&pg.state) && pg.class != hoi4_state::PopClass::Soldier)
        .map(|pg| pg.size as u64)
        .sum();
    if civilian_population == 0 {
        return;
    }
    let casualty_rate = casualties as f32 / civilian_population as f32;
    let stability_hit = (casualty_rate * 2.0).clamp(0.0001, 0.02);
    let war_support_hit = (casualty_rate * 3.0).clamp(0.0001, 0.03);
    world.countries.stability[ci] = (world.countries.stability[ci] - stability_hit).max(0.0);
    world.countries.war_support[ci] = (world.countries.war_support[ci] - war_support_hit).max(0.0);
    for pg in &mut world.countries.pops.groups {
        if !state_ids.contains(&pg.state) || pg.class == hoi4_state::PopClass::Soldier {
            continue;
        }
        let pressure = (casualties as f32 / civilian_population as f32 * 8.0).clamp(0.0, 0.05);
        pg.satisfaction = (pg.satisfaction - pressure).max(0.0);
        pg.radicalism = (pg.radicalism + pressure * 0.75).min(1.0);
    }
}

/// Consume stockpile + manpower when training a new division.
/// Returns the achieved strength ratio (0..1). If stockpile is insufficient,
/// the division starts at partial strength.
pub fn consume_training_resources(
    world: &mut World,
    econ: &mut EconomyState,
    data: &GameData,
    division_idx: usize,
) -> f32 {
    let owner = world.divisions.owners[division_idx];
    if owner.is_none() {
        return 0.0;
    }
    let ci = owner.0 as usize;
    if let Some(stockpile) = econ.stockpile.get_mut(ci) {
        normalize_stockpile_keys(stockpile);
    }
    let tag = match world.country_tag(owner) {
        Some(t) => t.to_owned(),
        None => return 0.0,
    };
    let templates = match data.division_templates.get(&tag) {
        Some(t) => t,
        None => return 0.0,
    };
    let tpl_idx = world.divisions.template_indices[division_idx] as usize;
    let Some(template) = templates.get(tpl_idx) else {
        return 0.0;
    };

    let needs = template_equipment_needs(template, data);
    let stockpile = &mut econ.stockpile[ci];

    // Compute min satisfaction ratio across all equipment types
    let mut min_ratio: f32 = 1.0;
    for (equip, qty) in &needs {
        if *qty == 0 {
            continue;
        }
        let have = stockpile.get(equip).copied().unwrap_or(0.0);
        let ratio = (have / *qty as f32).clamp(0.0, 1.0);
        min_ratio = min_ratio.min(ratio);
    }

    // Record the full equipment shortfall as negative stockpile so procurement demand grows.
    for (equip, qty) in &needs {
        *stockpile.entry(equip.clone()).or_insert(0.0) -= *qty as f32;
    }

    // Deduct manpower (P5: via PopGroup)
    let manpower_need = super::stats_manpower(template, data);
    let available_mp = world.manpower(owner);
    let mp_ratio = if manpower_need > 0 {
        (available_mp as f32 / manpower_need as f32).clamp(0.0, 1.0)
    } else {
        1.0
    };
    let mp_consume_f = manpower_need as f32 * mp_ratio.min(min_ratio);
    if mp_consume_f > 0.0 {
        let state_ids = world.country_state_ids(owner);
        let pop_indices = world
            .countries
            .pops
            .pops_by_class_in_country_mut(hoi4_state::PopClass::Soldier, &state_ids);
        let mut to_deduct = mp_consume_f as u64;
        for &idx in &pop_indices {
            if to_deduct == 0 {
                break;
            }
            let pg = &mut world.countries.pops.groups[idx];
            if pg.employed_at.is_none() {
                let deduct = to_deduct.min(pg.size as u64);
                pg.size = pg.size.saturating_sub(deduct as u32);
                to_deduct = to_deduct.saturating_sub(deduct);
            }
        }
    }

    min_ratio.min(mp_ratio)
}
