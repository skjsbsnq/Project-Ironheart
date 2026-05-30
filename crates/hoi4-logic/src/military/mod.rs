//! 军事系统 — 陆军（Phase 3.4）。
//!
//! 子模块：
//! - [`stats`]：师属性汇总（按模板把所有 battalion 的属性求和/聚合）
//! - [`organisation`]：每日组织度恢复
//! - [`battle`]：简化战斗解析器（随机战术选择 + 损失计算）
//! - [`spawn`]：从模板生成新师，写入 World.divisions

pub mod arbiter;
pub mod battle;
pub mod cleanup;
pub mod command_executor;
pub mod frontline;
pub mod general;
pub mod manpower;
pub mod movement;
pub mod organisation;
pub mod spawn;
pub mod stats;
pub mod templates;
pub mod training;

pub use battle::{BattleOutcome, BattleSide};
pub use stats::DivisionStats;
pub use training::TrainingQueueItem;

use std::collections::HashMap;

use hoi4_data::GameData;
use hoi4_state::{CountryId, World};

/// Economic demand emitted by the military system for one country on one day.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MilitaryDemand {
    pub manpower_needed: u32,
    pub equipment_needed: HashMap<String, f32>,
    pub fuel_needed: f32,
    pub maintenance_rm: f64,
    pub training_goods: HashMap<String, f32>,
}

/// Summarize active armed forces into economic requirements.
pub fn collect_military_demand(
    world: &World,
    data: &GameData,
    country_id: CountryId,
) -> MilitaryDemand {
    let mut demand = MilitaryDemand::default();
    if country_id.is_none() {
        return demand;
    }

    let tag = world.country_tag(country_id).map(|tag| tag.to_owned());
    let templates = tag
        .as_deref()
        .and_then(|tag| data.division_templates.get(tag));

    let country_idx = country_id.0 as usize;
    if world.runtime_country_indexes_valid {
        if let Some(indices) = world.country_division_index.get(country_idx) {
            for &div_idx in indices {
                add_division_demand(world, data, country_id, templates, div_idx, &mut demand);
            }
        }
    } else {
        for div_idx in 0..world.divisions.count {
            if world.divisions.owners[div_idx] == country_id {
                add_division_demand(world, data, country_id, templates, div_idx, &mut demand);
            }
        }
    }

    if world.runtime_country_indexes_valid {
        if let Some(indices) = world.country_air_wing_index.get(country_idx) {
            for &i in indices {
                if i < world.air_wings.count && world.air_wings.owners[i] == country_id {
                    demand.fuel_needed += world.air_wings.count_planes[i] as f32 * 0.05;
                    demand.maintenance_rm += world.air_wings.count_planes[i] as f64 * 120.0;
                }
            }
        }
    } else {
        for i in 0..world.air_wings.count {
            if world.air_wings.owners[i] == country_id {
                demand.fuel_needed += world.air_wings.count_planes[i] as f32 * 0.05;
                demand.maintenance_rm += world.air_wings.count_planes[i] as f64 * 120.0;
            }
        }
    }

    let naval_maintenance: f64 = (0..world.ships.count)
        .filter(|&i| world.ships.owners[i] == country_id)
        .map(|_| {
            demand.fuel_needed += 5.0;
            5_000.0
        })
        .sum();
    demand.maintenance_rm += naval_maintenance;

    let equipment_maintenance: f64 = demand
        .equipment_needed
        .iter()
        .map(|(equipment, qty)| *qty as f64 * equipment_unit_cost_rm(equipment) * 0.0005)
        .sum();
    demand.maintenance_rm += equipment_maintenance + demand.manpower_needed as f64 * 0.35;

    if demand.fuel_needed > 0.0 {
        demand
            .training_goods
            .insert("fuel".to_owned(), demand.fuel_needed * 0.1);
    }
    demand
}

fn add_division_demand(
    world: &World,
    data: &GameData,
    country_id: CountryId,
    templates: Option<&Vec<hoi4_data::DivisionTemplate>>,
    div_idx: usize,
    demand: &mut MilitaryDemand,
) {
    if div_idx >= world.divisions.count || world.divisions.owners[div_idx] != country_id {
        return;
    }
    let strength = world.divisions.strength[div_idx].clamp(0.0, 1.0);
    let replenishment_ratio = (1.0 - strength).max(0.01);
    let supply_mult = crate::military::general::modifier_for_division(world, div_idx)
        .map(|modifier| modifier.supply_mult)
        .unwrap_or(1.0);
    demand.fuel_needed += (0.2 + (1.0 - strength) * 0.3) * supply_mult;

    let Some(templates) = templates else {
        return;
    };
    let Some(template) = templates.get(world.divisions.template_indices[div_idx] as usize) else {
        return;
    };

    demand.manpower_needed = demand
        .manpower_needed
        .saturating_add(crate::economy::stats_manpower(template, data));
    for (equipment, qty) in crate::economy::stockpile::template_equipment_needs(template, data) {
        let normalized = crate::economy::stockpile::normalize_equipment_id(&equipment);
        let needed = qty as f32 * replenishment_ratio * supply_mult;
        *demand.equipment_needed.entry(normalized).or_insert(0.0) += needed;
    }
}

pub fn equipment_unit_cost_rm(equipment: &str) -> f64 {
    match equipment.to_ascii_lowercase().as_str() {
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
        _ => 4_000.0,
    }
}

pub mod constants {
    /// 战斗按 4 小时分一轮（HOI4: 1 day = 6 rounds）
    pub const HOURS_PER_ROUND: u32 = 4;

    /// 每日组织度恢复比例（占 max_org 的百分比）。
    /// HOI4 vanilla ≈ 30%/天；我们用 18% 是因为简化模型下双方师团很容易"互打→互停→互恢复"
    /// 卡死稳态。降低恢复让破碎/分胜负更易达成，同时给守军一定喘息空间避免被一次性击溃。
    pub const ORG_REGEN_PER_DAY: f32 = 0.18;

    /// 每轮战斗对组织度的伤害系数。
    /// 0.40 让普通陆战在 ~10-25 轮（2-4 天）出现明显失败方。
    /// 比之前 0.15 高，因为简化战斗模型缺少 CAS / 战宽 / 地形等放大因子。
    pub const ORG_DAMAGE_FACTOR: f32 = 0.40;

    /// 每轮战斗对人力 / 装备（strength）的伤害系数
    pub const STRENGTH_DAMAGE_FACTOR: f32 = 0.001;

    /// 没有任何战术匹配时使用的默认权重
    pub const DEFAULT_TACTIC_WEIGHT: f32 = 4.0;

    /// "克制"战术触发时的额外权重倍数
    pub const COUNTER_TACTIC_WEIGHT_BONUS: f32 = 4.0;
}
