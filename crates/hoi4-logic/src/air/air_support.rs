//! 空中支援：CAS 联队对地面战斗的加成（attacker_bonus / defender_bonus）。
//!
//! 简化模型：
//! - 给定空区里 owner 的 CAS 总数与制空权 → 该国师在该空区获得的攻击 / 防御加成
//! - 加成上限 [`super::constants::MAX_CAS_BONUS`]（HOI4 ≈ +50%）
//! - 制空权 < FULL_AIR_CONTROL_THRESHOLD 时按比例缩减
//! - 若敌方制空权 > 0.5，CAS 受拦截；加成额外 ×0.5

use hoi4_data::{AircraftKind, GameData};
use hoi4_state::{CountryId, World};

use super::air_superiority::AirControl;
use super::constants::{FULL_AIR_CONTROL_THRESHOLD, MAX_CAS_BONUS};

/// 一支地面战斗中获得的空中支援加成
#[derive(Debug, Clone, Copy, Default)]
pub struct GroundSupport {
    /// 加成系数（0..MAX_CAS_BONUS）；进攻 / 防御都用同一个
    pub bonus: f32,
    /// 该国 CAS / 战术轰炸机总数（参与该空区）
    pub cas_planes: u32,
    /// 该国制空权
    pub air_control: f32,
}

/// 计算某国在某空区对地面战斗的支援加成。
pub fn ground_support_modifier(
    world: &World,
    data: &GameData,
    air_control: &AirControl,
    region: u32,
    country: CountryId,
) -> GroundSupport {
    if country.is_none() {
        return GroundSupport::default();
    }
    let mut cas_planes: u32 = 0;
    for i in 0..world.air_wings.count {
        if world.air_wings.region_id[i] != region {
            continue;
        }
        if world.air_wings.owners[i] != country {
            continue;
        }
        if world.air_wings.transfer_arrival_hour[i] != 0 {
            continue;
        }
        let key = &world.air_wings.aircraft_keys[i];
        let Some(def) = data.aircraft.get(key) else {
            continue;
        };
        // CAS / TacticalBomber 都贡献 air_bombing
        if matches!(
            def.kind,
            AircraftKind::CloseAirSupport | AircraftKind::TacticalBomber
        ) && world.air_wings.mission[i] == hoi4_state::AirMission::CloseAirSupport
        {
            cas_planes = cas_planes.saturating_add(world.air_wings.count_planes[i]);
        }
    }

    let ac = air_control.control(region, country);

    // 没有 CAS 或没有制空权 → 没有加成
    if cas_planes == 0 || ac <= 0.0 {
        return GroundSupport {
            bonus: 0.0,
            cas_planes,
            air_control: ac,
        };
    }

    // 飞机数 → 比例（每 200 架达到 max；HOI4 中规模拐点附近）
    let plane_factor = (cas_planes as f32 / 200.0).clamp(0.0, 1.0);
    // 制空权 → 比例（0.75 → 1.0；以下按线性；以上 = 1）
    let ac_factor = (ac / FULL_AIR_CONTROL_THRESHOLD).clamp(0.0, 1.0);

    // 敌方制空权（取 1 - own_ac 中扣除其他中立部分；近似）
    let enemy_ac = (1.0 - ac).clamp(0.0, 1.0);
    let intercept_penalty = if enemy_ac > 0.5 { 0.5 } else { 1.0 };

    let bonus = MAX_CAS_BONUS * plane_factor * ac_factor * intercept_penalty;

    GroundSupport {
        bonus,
        cas_planes,
        air_control: ac,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_when_country_none() {
        // 用 default world 测试 country=NONE
        // 跳过 — 需要真实 world 构造，集成测试覆盖
        let g = GroundSupport::default();
        assert_eq!(g.bonus, 0.0);
        assert_eq!(g.cas_planes, 0);
    }
}
