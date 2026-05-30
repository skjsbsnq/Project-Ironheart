//! 师属性汇总：根据模板把所有 battalion 的属性聚合。
//!
//! HOI4 中 sub_unit 的 `soft_attack` / `defense` 等数值通常表示**相对装备的乘子**
//! （可正可负），实际战斗力来自该 sub_unit 装备的 `infantry_equipment_X` 等。
//! 我们没有完整加载装备战斗值，故采用：
//! 1. 给每个 sub_unit 按其 `group` 推断一个"baseline"（一战末-1936 装备的典型值）
//! 2. 再叠加 sub_unit 自身声明的 *正* 加成（负数当 0 处理）
//! 这样汇总出的结果在数量级上和原版相符，足以驱动战斗模拟。

use hoi4_data::{DivisionTemplate, GameData};

/// 一个师在战斗中暴露的全部属性
#[derive(Debug, Clone, Default)]
pub struct DivisionStats {
    pub combat_width: f32,
    pub soft_attack: f32,
    pub hard_attack: f32,
    pub defense: f32,
    pub breakthrough: f32,
    pub armor_value: f32,
    pub ap_attack: f32,
    /// 加权平均硬度（按 max_strength 加权）
    pub hardness: f32,
    pub max_strength: f32,
    pub max_organisation: f32,
    pub default_morale: f32,
    pub manpower: u32,
    pub suppression: f32,
    pub supply_consumption: f32,
    /// 单位数量
    pub battalion_count: usize,
}

/// 给定 subunit 类型推断的基础战斗值（HOI4 用 equipment 提供基础，但我们没载入装备战斗值，
/// 故按"满员装备 _1 型号"的典型值给一个推断基线）。
///
/// 返回 `(soft, hard, defense, breakthrough, armor, ap, hardness)`
fn baseline_for(sub: &hoi4_data::SubunitDef) -> (f32, f32, f32, f32, f32, f32, f32) {
    let category_hint = if !sub.group.is_empty() {
        sub.group.as_str()
    } else if let Some(t) = sub.types.first() {
        t.as_str()
    } else {
        ""
    };

    match category_hint {
        "infantry" => (6.0, 0.5, 22.0, 18.0, 0.0, 4.0, 0.0),
        "cavalry" => (5.0, 0.5, 18.0, 14.0, 0.0, 4.0, 0.0),
        "motorized" => (6.0, 1.0, 14.0, 11.0, 0.0, 5.0, 0.0),
        "mechanized" => (8.0, 2.0, 17.0, 14.0, 5.0, 7.0, 0.7),
        "armor" => (8.0, 12.0, 4.0, 20.0, 10.0, 10.0, 0.9),
        // 支援/特殊：无基线，让自身声明值起作用
        _ => (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0),
    }
}

impl DivisionStats {
    /// 聚合：sum for attack/defense/strength, max for armor/AP, weighted avg for hardness, avg for max_org
    pub fn aggregate(template: &DivisionTemplate, data: &GameData) -> Self {
        let mut s = Self::default();
        let mut org_sum = 0.0;
        let mut org_count = 0;
        let mut hardness_weighted = 0.0;
        let mut hardness_total_w = 0.0;
        let mut morale_max: f32 = 0.0;

        for sub_key in template.regiments.iter().chain(template.support.iter()) {
            let Some(sub) = data.subunits.get(sub_key) else {
                continue;
            };

            let (b_soft, b_hard, b_def, b_brk, b_armor, b_ap, b_hard_target) = baseline_for(sub);

            s.combat_width += sub.combat_width;
            s.soft_attack += b_soft + sub.soft_attack.max(0.0);
            s.hard_attack += b_hard + sub.hard_attack.max(0.0);
            s.defense += b_def + sub.defense.max(0.0);
            s.breakthrough += b_brk + sub.breakthrough.max(0.0);
            s.armor_value = s.armor_value.max(b_armor).max(sub.armor_value);
            s.ap_attack = s.ap_attack.max(b_ap).max(sub.ap_attack);
            s.max_strength += sub.max_strength;
            s.suppression += sub.suppression;
            s.manpower = s.manpower.saturating_add(sub.manpower);
            s.supply_consumption += sub.supply_consumption;
            s.battalion_count += 1;

            let effective_hardness = sub.hardness.max(b_hard_target);
            hardness_weighted += effective_hardness * sub.max_strength;
            hardness_total_w += sub.max_strength;

            org_sum += sub.max_organisation;
            org_count += 1;

            morale_max = morale_max.max(sub.default_morale);
        }

        s.max_organisation = if org_count > 0 {
            org_sum / org_count as f32
        } else {
            0.0
        };
        s.hardness = if hardness_total_w > 0.0 {
            hardness_weighted / hardness_total_w
        } else {
            0.0
        };
        s.default_morale = morale_max;
        s
    }

    /// 当前有效软攻 = soft_attack × strength × org_ratio
    pub fn effective_soft(&self, strength: f32, org: f32) -> f32 {
        self.soft_attack * strength * (org / self.max_organisation.max(1e-6))
    }

    pub fn effective_hard(&self, strength: f32, org: f32) -> f32 {
        self.hard_attack * strength * (org / self.max_organisation.max(1e-6))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_template_zero_stats() {
        let data = GameData::default();
        let tmpl = DivisionTemplate {
            name: "x".into(),
            country_tag: None,
            regiments: vec![],
            support: vec![],
            division_names_group: None,
        };
        let s = DivisionStats::aggregate(&tmpl, &data);
        assert_eq!(s.battalion_count, 0);
        assert_eq!(s.soft_attack, 0.0);
        assert_eq!(s.max_organisation, 0.0);
    }
}
