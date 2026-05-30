//! P1.4 建筑 tick：每日更新建筑运行时字段（单一权威源）。
//!
//! 使用 `valuation` 模块统一计算产出、投入、工资、利润、价值增量，
//! 写入 `Building` 结构体的运行时字段。财政、GDP、UI 均读取这些字段，
//! 不再各自重新推导。

use hoi4_content::V6Database;
use hoi4_state::{CountryId, World};

const DEFAULT_BUILD_COST: f32 = 7_200.0;

pub fn update(world: &mut World, db: &V6Database, ci: usize) {
    if ci >= world.countries.count {
        return;
    }

    let country = CountryId(ci as u16);

    let stats: Vec<_> = world
        .countries
        .buildings_v6
        .buildings
        .iter()
        .enumerate()
        .filter(|(_, building)| {
            let state_idx = building.state.0 as usize;
            state_idx < world.states.count && world.states.owners[state_idx] == country
        })
        .map(|(idx, building)| {
            let max_level = db
                .buildings
                .iter()
                .find(|def| def.id == building.building_def_id)
                .map(|def| def.max_level)
                .unwrap_or(building.level);
            let cp_cost = construction_cost(max_level);
            let val = super::valuation::compute_building_valuation(building, world, db, ci);
            (idx, val, cp_cost, max_level)
        })
        .collect();

    for (idx, val, cp_cost, max_level) in stats {
        if let Some(building) = world.countries.buildings_v6.buildings.get_mut(idx) {
            building.production_rate = val.employment_ratio;
            building.output_value_gbp = val.output_value_gbp;
            building.wage_rm = val.wage_rm;
            building.profit_rm = val.actual_profit_rm.max(0.0);
            building.input_cost_rm = val.input_cost_rm;
            building.estimated_profit_rm = val.estimated_profit_rm;
            building.value_added_rm = val.value_added_rm;
            building.cp_cost = cp_cost;
            building.max_level = max_level;
        }
    }
}

fn construction_cost(max_level: u8) -> f32 {
    DEFAULT_BUILD_COST * (1.0 + max_level as f32 * 0.02)
}
