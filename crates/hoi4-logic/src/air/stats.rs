//! 空军联队属性汇总：把单个 wing 的飞机属性 × 飞机数量 聚合为 [`AirWingStats`]。

use hoi4_data::{AircraftDef, AircraftKind, GameData};
use hoi4_state::{AirWingId, World};

/// 联队的战斗 / 任务属性汇总
#[derive(Debug, Clone, Default)]
pub struct AirWingStats {
    /// 联队当前飞机数
    pub plane_count: u32,
    /// 联队满员飞机数
    pub max_plane_count: u32,
    /// 联队总 HP（plane_count × hp_per_plane）
    pub total_hp: f32,
    /// 联队总反敌机攻击
    pub total_air_attack: f32,
    /// 联队总反敌机防御
    pub total_air_defense: f32,
    /// 平均敏捷（不取累加，按架数加权）
    pub avg_agility: f32,
    /// 总对地轰炸（CAS）
    pub total_air_bombing: f32,
    /// 总反舰
    pub total_naval_strike: f32,
    /// 总战略轰炸
    pub total_strategic_bombing: f32,
    /// 飞机大类（联队是单一型号，故只有一种）
    pub kind: Option<AircraftKind>,
    /// 当前 org / max org（来自 store 直接读）
    pub current_org: f32,
    pub max_org: f32,
}

impl AirWingStats {
    pub fn org_ratio(&self) -> f32 {
        if self.max_org > 0.0 {
            (self.current_org / self.max_org).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    pub fn plane_ratio(&self) -> f32 {
        if self.max_plane_count > 0 {
            (self.plane_count as f32 / self.max_plane_count as f32).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    pub fn aggregate(world: &World, data: &GameData, wing: AirWingId) -> Self {
        let mut s = Self::default();
        if wing.is_none() {
            return s;
        }
        let i = wing.0 as usize;
        if i >= world.air_wings.count {
            return s;
        }
        let key = &world.air_wings.aircraft_keys[i];
        let def: &AircraftDef = match data.aircraft.get(key) {
            Some(d) => d,
            None => return s,
        };

        let n = world.air_wings.count_planes[i];
        let n_f = n as f32;
        s.plane_count = n;
        s.max_plane_count = world.air_wings.max_planes[i];
        s.kind = Some(def.kind);
        s.current_org = world.air_wings.organisation[i];
        s.max_org = world.air_wings.max_organisation[i];

        // 战斗属性按架数 × 单架 × org_ratio（org 低则不发挥）
        let org_ratio = s.org_ratio().max(0.05); // 最低 5% 战力
        s.total_hp = def.max_hp * n_f;
        s.total_air_attack = def.air_attack * n_f * org_ratio;
        s.total_air_defense = def.air_defense * n_f * org_ratio;
        s.avg_agility = def.agility;
        s.total_air_bombing = def.air_bombing * n_f * org_ratio;
        s.total_naval_strike = def.naval_strike * n_f * org_ratio;
        s.total_strategic_bombing = def.strategic_bombing * n_f * org_ratio;
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_wing_zero_stats() {
        let s = AirWingStats::default();
        assert_eq!(s.plane_count, 0);
        assert_eq!(s.org_ratio(), 0.0);
    }
}
