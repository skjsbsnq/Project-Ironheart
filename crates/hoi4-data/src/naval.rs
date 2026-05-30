//! 海军舰船类型（来自 `common/units/<class>.txt`）。
//!
//! 由于 HOI4 vanilla naval DLC 系统真实属性来自 hull + modules 的复杂组合，
//! 我们采用混合方案：
//! - 从 `common/units/*.txt` 检测哪些舰类存在 + 取 max_organisation / supply_consumption
//! - 战斗属性使用内置 baseline（按 1936-1939 平均装备水准的典型值）

/// 舰种大类
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShipKind {
    /// 屏卫舰（驱逐舰、轻巡）
    Screen,
    /// 主力舰（重巡、战列、战巡）
    Capital,
    /// 航母
    Carrier,
    /// 潜艇
    Submarine,
    /// 运输舰 / convoy
    Transport,
    Other,
}

impl ShipKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Screen => "screen",
            Self::Capital => "capital",
            Self::Carrier => "carrier",
            Self::Submarine => "submarine",
            Self::Transport => "transport",
            Self::Other => "other",
        }
    }
}

/// 舰类定义（"destroyer" / "battleship" 等）
#[derive(Debug, Clone)]
pub struct ShipClassDef {
    pub key: String,
    pub kind: ShipKind,
    /// 满 HP（1936 装备的近似值，HOI4 默认 100..1500）
    pub max_hp: f32,
    /// 满组织度（HOI4 中 ship 也有 max_organisation；通常 30..50）
    pub max_organisation: f32,
    /// 反舰攻击（vs surface）
    pub naval_attack: f32,
    /// 鱼雷攻击（vs surface，潜艇/驱逐用）
    pub torpedo_attack: f32,
    /// 反潜攻击
    pub sub_attack: f32,
    /// 反空攻击（防空）
    pub anti_air_attack: f32,
    /// 装甲（仅大舰有）
    pub armor: f32,
    /// 装甲穿透
    pub armor_piercing: f32,
    /// 速度
    pub speed: f32,
    /// 视野（侦察）
    pub surface_visibility: f32,
    /// 潜艇视野（贴近被发现概率）
    pub sub_visibility: f32,
    /// 飞机搭载（航母）
    pub carrier_size: u32,
    /// 每日补给消耗
    pub supply_consumption: f32,
    /// 满员人力
    pub manpower: u32,
}

impl ShipClassDef {
    /// 是否对水面舰艇造成威胁
    pub fn can_attack_surface(&self) -> bool {
        self.naval_attack > 0.0 || self.torpedo_attack > 0.0
    }

    pub fn can_attack_subs(&self) -> bool {
        self.sub_attack > 0.0
    }

    pub fn is_capital(&self) -> bool {
        matches!(self.kind, ShipKind::Capital)
    }
}

/// 内置 baseline：HOI4 1936-1939 装备水准的典型舰艇属性。
///
/// vanilla 数据中 ship sub_unit 不直接含战斗属性（来自 modules），
/// 故这里手工给每个 known class 一组合理 baseline。
pub fn baseline_for_class(key: &str) -> Option<ShipClassDef> {
    let (kind, hp, org, na, ta, sa, aa, armor, ap, speed, sv, suv, cs, sup, mp) = match key {
        // (kind, hp, org, naval, torp, sub, AA, armor, ap, speed, surf_vis, sub_vis, carrier, supply, manpower)
        "destroyer" => (
            ShipKind::Screen,
            70.0,
            35.0,
            4.0,
            6.0,
            4.0,
            1.0,
            0.0,
            0.0,
            35.0,
            12.0,
            12.0,
            0,
            0.04,
            200,
        ),
        "light_cruiser" => (
            ShipKind::Screen,
            350.0,
            40.0,
            8.0,
            0.0,
            1.0,
            4.0,
            5.0,
            5.0,
            30.0,
            18.0,
            0.0,
            0,
            0.10,
            600,
        ),
        "heavy_cruiser" => (
            ShipKind::Capital,
            700.0,
            40.0,
            14.0,
            0.0,
            0.0,
            6.0,
            30.0,
            25.0,
            28.0,
            22.0,
            0.0,
            0,
            0.20,
            1500,
        ),
        "battlecruiser" | "battle_cruiser" => (
            ShipKind::Capital,
            1100.0,
            45.0,
            24.0,
            0.0,
            0.0,
            6.0,
            50.0,
            55.0,
            28.0,
            25.0,
            0.0,
            0,
            0.40,
            2200,
        ),
        "battleship" => (
            ShipKind::Capital,
            1400.0,
            50.0,
            30.0,
            0.0,
            0.0,
            8.0,
            80.0,
            80.0,
            24.0,
            28.0,
            0.0,
            0,
            0.80,
            4500,
        ),
        "carrier" => (
            ShipKind::Carrier,
            1500.0,
            50.0,
            4.0,
            0.0,
            0.0,
            6.0,
            35.0,
            0.0,
            26.0,
            28.0,
            0.0,
            90,
            0.50,
            4500,
        ),
        "submarine" => (
            ShipKind::Submarine,
            60.0,
            30.0,
            0.0,
            12.0,
            0.0,
            0.0,
            0.0,
            0.0,
            18.0,
            18.0,
            6.0,
            0,
            0.05,
            80,
        ),
        "convoy" => (
            ShipKind::Transport,
            60.0,
            25.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            16.0,
            16.0,
            16.0,
            0,
            0.01,
            100,
        ),
        _ => return None,
    };
    Some(ShipClassDef {
        key: key.to_owned(),
        kind,
        max_hp: hp,
        max_organisation: org,
        naval_attack: na,
        torpedo_attack: ta,
        sub_attack: sa,
        anti_air_attack: aa,
        armor,
        armor_piercing: ap,
        speed,
        surface_visibility: sv,
        sub_visibility: suv,
        carrier_size: cs,
        supply_consumption: sup,
        manpower: mp,
    })
}
