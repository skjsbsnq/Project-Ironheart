//! 制海权计算：每海区每国家的"presence weight"占比。
//!
//! 简化模型：
//! - 一个海区里所有 fleet 的所有 ship 按 hp_ratio × class_weight 累加为该国 presence
//! - 国家 i 的制海权 = i.presence / Σ(presence)（不打折，单海区竞争）
//! - 没有任何 fleet 时制海权全为 0（无人争夺）

use std::collections::HashMap;

use hoi4_data::{GameData, ShipKind};
use hoi4_state::{CountryId, World};

use super::constants::CONVOY_PRESENCE_WEIGHT;

/// 单海区制海权状态
#[derive(Debug, Clone, Default)]
pub struct SeaControl {
    /// region_id → 国家 → 制海权 0..1
    pub by_region: HashMap<u32, HashMap<CountryId, f32>>,
}

impl SeaControl {
    /// 从全局 world 重新计算所有海区制海权
    pub fn recompute(world: &World, data: &GameData) -> Self {
        let mut by_region: HashMap<u32, HashMap<CountryId, f32>> = HashMap::new();

        // 第一遍：累计每海区每国 presence
        for fi in 0..world.fleets.count {
            if world.fleets.target_region_id[fi] != super::regions::NO_SEA_REGION {
                continue;
            }
            let region = world.fleets.region_id[fi];
            if region == u32::MAX {
                continue;
            }
            let owner = world.fleets.owners[fi];
            if owner.is_none() {
                continue;
            }

            let mut presence = 0.0f32;
            for &s in &world.fleets.ships[fi] {
                if s.is_none() {
                    continue;
                }
                let si = s.0 as usize;
                if si >= world.ships.count {
                    continue;
                }
                let hp = world.ships.hp[si];
                let max_hp = world.ships.max_hp[si].max(1e-6);
                let hp_ratio = hp / max_hp;
                if hp_ratio <= 0.0 {
                    continue;
                }
                let class_key = &world.ships.class_keys[si];
                let weight = match data.ship_classes.get(class_key) {
                    Some(c) => match c.kind {
                        ShipKind::Capital => 4.0,
                        ShipKind::Carrier => 4.0,
                        ShipKind::Screen => 1.0,
                        ShipKind::Submarine => 0.7,
                        ShipKind::Transport => CONVOY_PRESENCE_WEIGHT,
                        ShipKind::Other => 1.0,
                    },
                    None => 1.0,
                };
                presence += weight * hp_ratio;
            }

            *by_region
                .entry(region)
                .or_default()
                .entry(owner)
                .or_insert(0.0) += presence;
        }

        // 第二遍：归一化为 0..1
        for (_, entries) in by_region.iter_mut() {
            let total: f32 = entries.values().copied().sum();
            if total > 0.0 {
                for v in entries.values_mut() {
                    *v /= total;
                }
            }
        }

        Self { by_region }
    }

    /// 查询某国在某海区的制海权
    pub fn control(&self, region: u32, country: CountryId) -> f32 {
        self.by_region
            .get(&region)
            .and_then(|m| m.get(&country).copied())
            .unwrap_or(0.0)
    }

    /// 谁在该海区占主导（>= 50%）
    pub fn dominant(&self, region: u32) -> Option<(CountryId, f32)> {
        let entries = self.by_region.get(&region)?;
        entries
            .iter()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(&c, &v)| (c, v))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_returns_zero_control() {
        let sc = SeaControl::default();
        assert_eq!(sc.control(1, CountryId(0)), 0.0);
        assert!(sc.dominant(1).is_none());
    }
}
