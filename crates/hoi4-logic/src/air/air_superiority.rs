//! 制空权计算：每空区每国"presence"占比。
//!
//! 简化模型：
//! - 一个空区里所有联队按 (plane_count × kind_weight × org_ratio) 累加为该国 presence
//! - 国家 i 的制空权 = i.presence / Σ(presence)
//! - 战斗机权重最高，CAS / 战略轰炸次之；运输机几乎不贡献

use std::collections::HashMap;

use hoi4_data::{AircraftKind, GameData};
use hoi4_state::{CountryId, World};

/// 单空区制空权状态
#[derive(Debug, Clone, Default)]
pub struct AirControl {
    /// region_id → 国家 → 制空权 0..1
    pub by_region: HashMap<u32, HashMap<CountryId, f32>>,
}

impl AirControl {
    /// 重新计算所有空区制空权。
    pub fn recompute(world: &World, data: &GameData) -> Self {
        let mut by_region: HashMap<u32, HashMap<CountryId, f32>> = HashMap::new();

        for i in 0..world.air_wings.count {
            if world.air_wings.transfer_arrival_hour[i] != 0 {
                continue;
            }
            let region = if world.air_wings.target_region[i] == super::regions::NO_AIR_REGION {
                world.air_wings.region_id[i]
            } else {
                world.air_wings.target_region[i]
            };
            if region == u32::MAX {
                continue;
            }
            let owner = world.air_wings.owners[i];
            if owner.is_none() {
                continue;
            }
            let planes = world.air_wings.count_planes[i];
            if planes == 0 {
                continue;
            }
            let key = &world.air_wings.aircraft_keys[i];
            let def = match data.aircraft.get(key) {
                Some(d) => d,
                None => continue,
            };
            let weight = match def.kind {
                // 战斗机 = 制空主力
                AircraftKind::Fighter => 4.0,
                AircraftKind::HeavyFighter => 3.0,
                // 多用途轰炸机有自卫，能争夺制空
                AircraftKind::TacticalBomber => 1.5,
                AircraftKind::CloseAirSupport => 1.0,
                AircraftKind::NavalBomber => 0.8,
                AircraftKind::StrategicBomber => 1.0,
                AircraftKind::TransportPlane => 0.1,
                AircraftKind::Other => 1.0,
            };
            let max_org = world.air_wings.max_organisation[i].max(1e-6);
            let org_ratio = (world.air_wings.organisation[i] / max_org).clamp(0.0, 1.0);
            let presence = (planes as f32) * weight * org_ratio.max(0.1);

            *by_region
                .entry(region)
                .or_default()
                .entry(owner)
                .or_insert(0.0) += presence;
        }

        // 归一化
        for entries in by_region.values_mut() {
            let total: f32 = entries.values().copied().sum();
            if total > 0.0 {
                for v in entries.values_mut() {
                    *v /= total;
                }
            }
        }

        Self { by_region }
    }

    pub fn control(&self, region: u32, country: CountryId) -> f32 {
        self.by_region
            .get(&region)
            .and_then(|m| m.get(&country).copied())
            .unwrap_or(0.0)
    }

    /// 谁在该空区占主导（>= 50%）
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
        let ac = AirControl::default();
        assert_eq!(ac.control(1, CountryId(0)), 0.0);
        assert!(ac.dominant(1).is_none());
    }
}
