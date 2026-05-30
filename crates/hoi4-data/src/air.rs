//! 空军飞机类型（来自 `common/units/equipment/` 中的 `*_plane*.txt`，
//! 战斗属性来自内置 baseline）。
//!
//! 与 [`super::naval::ShipClassDef`] 类似，原版 vanilla naval/air DLCs 的真实属性来自
//! plane hull + modules（Battle for the Bosporus 后还有 air designer），
//! 我们用混合方案：检测原版有哪些飞机类，再注入内置 baseline。

/// 飞机大类
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AircraftKind {
    /// 战斗机（夺取制空，护航）
    Fighter,
    /// 重型战斗机（远程拦截）
    HeavyFighter,
    /// 近距支援（CAS）
    CloseAirSupport,
    /// 战术轰炸机（多用途）
    TacticalBomber,
    /// 战略轰炸机（轰炸工业）
    StrategicBomber,
    /// 海军轰炸机（反舰）
    NavalBomber,
    /// 运输机
    TransportPlane,
    Other,
}

impl AircraftKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Fighter => "fighter",
            Self::HeavyFighter => "heavy_fighter",
            Self::CloseAirSupport => "cas",
            Self::TacticalBomber => "tactical_bomber",
            Self::StrategicBomber => "strategic_bomber",
            Self::NavalBomber => "naval_bomber",
            Self::TransportPlane => "transport_plane",
            Self::Other => "other",
        }
    }
}

/// 飞机型号定义（"fighter" / "cas" / "tactical_bomber" 等）。
///
/// HOI4 中 air_unit 战斗依赖：
/// - `air_attack` 反敌机
/// - `air_defence` 抗反击
/// - `air_agility` 抓住目标的概率
/// - `air_speed` 影响相遇率（基本不在我们模型里）
/// - `air_bombing` 对地伤害（CAS / 战术）
/// - `naval_strike_attack` 反舰伤害（naval bomber）
/// - `air_ground_attack`：广义对地（包括摧毁建筑）
/// - 战略轰炸特殊伤害字段 `strategic_bombing`
#[derive(Debug, Clone)]
pub struct AircraftDef {
    pub key: String,
    pub kind: AircraftKind,
    /// 单架满 HP（HOI4 每架 25..40，此处给一个抽象 baseline）
    pub max_hp: f32,
    /// 单架满组织度（联队级；HOI4 中 air wing 的 max_org 30..70）
    pub max_organisation: f32,
    /// 反敌机攻击力
    pub air_attack: f32,
    /// 抗反击防御
    pub air_defense: f32,
    /// 敏捷性（追击/规避）
    pub agility: f32,
    /// 速度
    pub speed: f32,
    /// 作战半径（空区数；战斗机短，战略轰长）
    pub range: f32,
    /// 对地轰炸（CAS / tactical 攻击地面师）
    pub air_bombing: f32,
    /// 反舰攻击（naval bomber）
    pub naval_strike: f32,
    /// 战略轰炸伤害（对工厂/基建）
    pub strategic_bombing: f32,
    /// 单架 IC 生产成本
    pub build_cost_ic: f32,
    /// 单架人力（机组）
    pub manpower: u32,
    /// 单架日补给消耗
    pub supply_consumption: f32,
}

impl AircraftDef {
    pub fn is_fighter(&self) -> bool {
        matches!(
            self.kind,
            AircraftKind::Fighter | AircraftKind::HeavyFighter
        )
    }

    pub fn can_strategic_bomb(&self) -> bool {
        self.strategic_bombing > 0.0
    }

    pub fn can_close_air_support(&self) -> bool {
        self.air_bombing > 0.0
    }

    pub fn can_naval_strike(&self) -> bool {
        self.naval_strike > 0.0
    }
}

/// 内置 baseline：HOI4 1936-1939 飞机的近似属性（每架）
///
/// 字段顺序：
/// (kind, hp, org, air_atk, air_def, agility, speed, range,
///  air_bomb, naval_strike, strat_bomb, ic, manpower, supply)
pub fn baseline_for_aircraft(key: &str) -> Option<AircraftDef> {
    let (kind, hp, org, atk, def, agi, spd, rng, ab, ns, sb, ic, mp, sup) = match key {
        // (kind, hp, org, atk, def, agi, spd, rng, air_bomb, naval, strat, ic, mp, sup)
        "fighter" => (
            AircraftKind::Fighter,
            32.0,
            70.0,
            10.0,
            18.0,
            68.0,
            460.0,
            800.0,
            1.0,
            0.0,
            0.0,
            22.0,
            1,
            0.05,
        ),
        "heavy_fighter" => (
            AircraftKind::HeavyFighter,
            45.0,
            65.0,
            14.0,
            30.0,
            38.0,
            480.0,
            1200.0,
            2.0,
            1.0,
            1.0,
            34.0,
            2,
            0.07,
        ),
        "cas" => (
            AircraftKind::CloseAirSupport,
            28.0,
            60.0,
            2.0,
            14.0,
            55.0,
            320.0,
            700.0,
            18.0,
            1.0,
            0.0,
            24.0,
            1,
            0.06,
        ),
        "tactical_bomber" => (
            AircraftKind::TacticalBomber,
            55.0,
            65.0,
            4.0,
            20.0,
            42.0,
            380.0,
            1200.0,
            10.0,
            5.0,
            6.0,
            36.0,
            2,
            0.10,
        ),
        "strategic_bomber" => (
            AircraftKind::StrategicBomber,
            85.0,
            65.0,
            2.0,
            32.0,
            18.0,
            360.0,
            2400.0,
            6.0,
            0.0,
            22.0,
            68.0,
            4,
            0.18,
        ),
        "naval_bomber" => (
            AircraftKind::NavalBomber,
            40.0,
            60.0,
            3.0,
            14.0,
            48.0,
            340.0,
            900.0,
            2.0,
            18.0,
            0.0,
            26.0,
            1,
            0.07,
        ),
        "transport_plane" => (
            AircraftKind::TransportPlane,
            45.0,
            50.0,
            0.0,
            12.0,
            30.0,
            320.0,
            1500.0,
            0.0,
            0.0,
            0.0,
            30.0,
            2,
            0.10,
        ),
        _ => return None,
    };
    Some(AircraftDef {
        key: key.to_owned(),
        kind,
        max_hp: hp,
        max_organisation: org,
        air_attack: atk,
        air_defense: def,
        agility: agi,
        speed: spd,
        range: rng,
        air_bombing: ab,
        naval_strike: ns,
        strategic_bombing: sb,
        build_cost_ic: ic,
        manpower: mp,
        supply_consumption: sup,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fighter_baseline() {
        let f = baseline_for_aircraft("fighter").unwrap();
        assert!(f.is_fighter());
        assert!(f.air_attack > 0.0);
        assert!(f.agility > 50.0);
        assert!(!f.can_strategic_bomb());
    }

    #[test]
    fn cas_does_air_bombing() {
        let c = baseline_for_aircraft("cas").unwrap();
        assert!(!c.is_fighter());
        assert!(c.can_close_air_support());
        assert!(c.air_bombing > 10.0);
    }

    #[test]
    fn strategic_bomber() {
        let sb = baseline_for_aircraft("strategic_bomber").unwrap();
        assert!(sb.can_strategic_bomb());
        assert!(sb.strategic_bombing > 15.0);
        assert!(sb.range > 2000.0);
    }

    #[test]
    fn naval_bomber() {
        let nb = baseline_for_aircraft("naval_bomber").unwrap();
        assert!(nb.can_naval_strike());
        assert!(nb.naval_strike > 10.0);
    }

    #[test]
    fn unknown_returns_none() {
        assert!(baseline_for_aircraft("unicorn").is_none());
    }
}
