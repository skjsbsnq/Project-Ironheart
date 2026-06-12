//! RON focus tree schema types.

use serde::{Deserialize, Serialize};

/// 一棵完整的国策树。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FocusTree {
    pub country: String,
    pub focuses: Vec<Focus>,
}

/// 单个国策节点。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Focus {
    pub id: String,
    pub name: String,
    pub icon: String,
    /// 网格坐标 (x, y)，用于 UI 布局。
    pub position: (i32, i32),
    /// 完成所需天数（通常 70）。
    pub cost_days: u32,
    /// 前置国策 id 列表。外层 Vec = AND，内层 Vec = OR（任一满足即可）。
    #[serde(default)]
    pub prerequisites: Vec<Vec<String>>,
    /// 互斥国策 id 列表。
    #[serde(default)]
    pub mutually_exclusive: Vec<String>,
    /// 可选条件（为空 / AlwaysTrue 表示无条件可选）。
    #[serde(default = "Trigger::always_true")]
    pub available: Trigger,
    /// 完成后执行的效果列表。
    #[serde(default)]
    pub completion_effect: Vec<Effect>,
}

// ═══════════════════════════════════════════════════════════════════
// Trigger — ~30 variants
// ═══════════════════════════════════════════════════════════════════

/// 自研 trigger 枚举（RON 可序列化，不依赖 clausewitz-parser）。
/// 30 个实质变体 + And/Or/Not 组合。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Trigger {
    // ── 通用 ──
    AlwaysTrue,
    AlwaysFalse,
    /// 当前事件/国策作用域国家 tag 等于指定值。
    Tag(String),

    // ── 国家 flag / focus / idea / tech ──
    HasCountryFlag(String),
    HasGlobalFlag(String),
    HasCompletedFocus(String),
    HasIdea(String),
    HasTech(String),

    // ── 政治 ──
    HasGovernment(String),
    /// 某意识形态支持率 >= amount
    PartyPopularity {
        ideology: String,
        amount: f32,
    },

    // ── 数值比较 ──
    PoliticalPower(f32),
    Stability(f32),
    WarSupport(f32),
    NumOfFactories(u32),
    NumOfCivilianFactories(u32),
    NumOfMilitaryFactories(u32),
    Manpower(u64),

    // ── 军事 ──
    HasWar(bool),
    HasWarWith(String),
    /// 师数量 >= n
    NumDivisions(u32),
    ArmyExperience(f32),
    NavyExperience(f32),
    AirExperience(f32),

    // ── 外交 ──
    IsInFaction(bool),
    IsInFactionWith(String),
    IsFactionLeader(bool),
    IsSubjectOf(String),
    IsPuppet(bool),
    /// opinion of target >= value
    OpinionOf {
        target: String,
        value: i16,
    },
    WorldTension(f32),

    // ── 地理 / 州 ──
    OwnsState(u16),
    ControlsState(u16),
    /// 国家 tag 存在（用于检查某国是否已被吞并）
    CountryExists(String),

    // ── 时间 ──
    Date {
        year: u16,
        month: u8,
        day: u8,
    },

    // ── P2.5 国家变量 ──
    /// 当前作用国家变量 `name` >= `value`。
    VariableAtLeast {
        name: String,
        value: f32,
    },
    /// 当前作用国家变量 `name` <= `value`。
    VariableAtMost {
        name: String,
        value: f32,
    },
    /// 当前作用国家变量 `name` 严格落在 [min, max] 内（含端点）。
    VariableInRange {
        name: String,
        min: f32,
        max: f32,
    },
    /// 当前作用国家变量 `name` 等于 `value`（容差 1e-4）。
    VariableEquals {
        name: String,
        value: f32,
    },

    // ── P2.6 人物状态 ──
    /// 当前作用国家人物 `key` 已死亡。
    CharacterDead(String),
    /// 当前作用国家人物 `key` 当前可用（不死、不被捕、不流亡）。
    CharacterAvailable(String),
    /// 当前作用国家人物 `key` 当前掌权（InOffice）。
    CharacterIsLeader(String),

    // ── 逻辑组合 ──
    And(Vec<Trigger>),
    Or(Vec<Trigger>),
    Not(Box<Trigger>),
}

impl Trigger {
    pub fn always_true() -> Self {
        Self::AlwaysTrue
    }
}

impl Default for Trigger {
    fn default() -> Self {
        Self::AlwaysTrue
    }
}

// ═══════════════════════════════════════════════════════════════════
// Effect — ~50 variants
// ═══════════════════════════════════════════════════════════════════

/// 自研 effect 枚举（RON 可序列化）。
/// 50 个实质变体 + If 条件执行。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Effect {
    // ── 政治力量 / 稳定 / 战争支持 ──
    AddPoliticalPower(f32),
    AddStability(f32),
    AddWarSupport(f32),

    // ── 人力 / 燃油 ──
    AddManpower(i64),
    AddFuel(f32),

    // ── 经验 ──
    ArmyExperience(f32),
    NavyExperience(f32),
    AirExperience(f32),

    // ── Flag ──
    SetCountryFlag(String),
    SetCountryFlagForDays {
        flag: String,
        days: u32,
    },
    ClearCountryFlag(String),
    SetGlobalFlag(String),
    ClearGlobalFlag(String),

    // ── 国家精神 / Ideas ──
    AddIdea(String),
    RemoveIdea(String),
    /// 交换 idea（移除 old，添加 new）
    SwapIdea {
        remove: String,
        add: String,
    },

    // ── 政治 ──
    SetPolitics {
        ruling_party: String,
    },
    AddPopularity {
        ideology: String,
        amount: f32,
    },
    /// 设置选举（开启/关闭）
    SetPartyName {
        ideology: String,
        name: String,
    },

    // ── 科研 ──
    AddResearchSlot(i8),
    AddTechBonus {
        category: String,
        bonus: f32,
    },
    /// 直接完成某科技
    SetTechnology(String),

    // ── 工厂 / 建筑 ──
    AddBuildingInState {
        state: u16,
        building: String,
        level: i32,
    },
    /// 给所有拥有州加建筑
    AddBuildingAllStates {
        building: String,
        level: i32,
    },
    /// V7 building effect: directly changes a V6/V7 building level in a state.
    AddBuildingLevel {
        state: u16,
        building_id: String,
        level: i32,
    },
    /// 增加建筑槽位
    AddExtraBuildingSlots {
        state: u16,
        slots: u32,
    },

    // ── 资源 ──
    AddResource {
        state: u16,
        resource: String,
        amount: i32,
    },
    /// V7 resource exploration effect: raises discovered deposit level up to potential.
    AddResourceDiscovery {
        state: u16,
        good_id: String,
        discovered_level: u8,
    },
    AddProductionMethodUnlock {
        pm_id: String,
    },
    AddGovernmentOrder {
        equipment_category: String,
        daily_budget_rm: f64,
        duration_days: u32,
    },
    AddTradeAgreement {
        partner: String,
        good_id: String,
        daily_quantity: f32,
    },
    ChangeLaw {
        category: String,
        law_id: String,
        lock_days: u16,
    },
    AddMefoCapacity {
        amount_rm: f64,
    },
    AddPrivateInvestmentPool {
        amount_rm: f64,
    },
    AddConstructionCapacity {
        amount: i32,
    },
    AddMilitarySpendingShare {
        delta: f32,
    },

    // ── 军事 ──
    /// 增加征兵法律等级（简化为直接加人力比例）
    AddConscription(f32),
    /// 创建师（按模板名）
    CreateDivision {
        template: String,
        province: u16,
    },

    // ── 外交 ──
    AddNamedThreat(f32),
    AddOpinion {
        target: String,
        amount: i16,
    },
    CreateFaction(String),
    AddToFaction(String),
    LeaveFaction,
    DeclareWarOn(String),
    AnnexCountry(String),
    PuppetCountry(String),
    /// 给予独立
    FreeCountry(String),
    /// 给予军事通行权
    GiveMilitaryAccess(String),
    /// 签订互不侵犯条约（简化为 opinion boost）
    NonAggressionPact(String),
    /// 保证独立
    GuaranteeIndependence(String),

    // ── 州转移 / 核心 ──
    TransferState(u16),
    /// 把指定州转给指定国家（区别于 `TransferState`，后者只能转给当前作用域国家）。
    /// 释放傀儡国时用：先 `CreateCountry` → `TransferStateTo` → `PuppetCountry`。
    TransferStateTo {
        state: u16,
        country: String,
    },
    /// 动态创建国家（如果该 tag 不存在）。tag 已存在则 no-op。
    /// 用于事件 / 决议中释放历史上尚未独立的国家，例如波西米亚-摩拉维亚保护国、斯洛伐克。
    CreateCountry {
        tag: String,
        color: [u8; 3],
        ruling_party: String,
    },
    /// 添加核心
    AddCoreTo {
        state: u16,
        country: String,
    },
    /// 移除核心
    RemoveCoreFrom {
        state: u16,
        country: String,
    },
    /// 添加宣称
    AddClaim {
        state: u16,
        country: String,
    },

    // ── 决议 ──
    UnlockDecision(String),
    SpawnDivisionsInStates {
        country: String,
        states: Vec<u16>,
        count_per_state: u32,
        name_prefix: String,
    },

    RemoveDecision(String),

    // ── 变量 ──
    SetVariable {
        name: String,
        value: f32,
    },
    AddToVariable {
        name: String,
        value: f32,
    },
    AddToCountryVariable {
        tag: String,
        name: String,
        value: f32,
    },
    /// P2.5：限制变量在 [min, max] 区间内（已存在的变量被钳制；不存在则不创建）。
    ClampVariable {
        name: String,
        min: f32,
        max: f32,
    },
    ClampCountryVariable {
        tag: String,
        name: String,
        min: f32,
        max: f32,
    },
    ComputeSpanishPrewarSettlement {
        republic_tag: String,
        nationalist_tag: String,
    },

    // ── P2.6 人物效果 ──
    /// 标记人物 `key` 在当前作用国家死亡（不可恢复）。
    KillCharacter(String),
    /// 标记人物 `key` 在当前作用国家流亡（不在任，不可掌权）。
    ExileCharacter(String),
    /// 标记人物 `key` 在当前作用国家被捕。
    ImprisonCharacter(String),
    /// 招募人物 `key` 进入当前作用国家可用人物名册（不会复活已死人物）。
    RecruitCharacter(String),
    /// 显式设置人物 `key` 在当前作用国家的运行时状态。
    SetCharacterStatus {
        key: String,
        status: hoi4_state::CharacterRuntimeStatus,
    },
    /// 让人物 `key` 在当前作用国家就任领导人（同时清除其他 InOffice 状态）。
    /// 仅在人物当前可用时生效；不会复活死亡或流亡人物。
    SetLeader(String),

    // ── 杂项 ──
    /// 触发事件（事件 id）
    TriggerEvent(String),
    /// Request app-side player country switch by tag.
    SwitchPlayerCountry(String),
    /// 修改国家 modifier（简化：key + value，运行时按 key 查表）
    AddModifier {
        name: String,
        duration_days: i32,
    },
    /// 移除 modifier
    RemoveModifier(String),

    // ── 条件执行 ──
    If {
        trigger: Trigger,
        effects: Vec<Effect>,
    },
}

fn variable_display_name(name: &str) -> &str {
    match name {
        "spa_franco_authority" => "佛朗哥权威",
        "spa_falange_power" => "长枪党权力",
        "spa_army_loyalty" => "军队忠诚",
        "spa_church_influence" => "教会影响",
        "spa_carlist_anger" => "卡洛斯派怨恨",
        "spa_foreign_dependency" => "外援依赖",
        "spa_rear_supply" => "后方补给",
        "spa_junta_legitimacy" => "军政府合法性",
        "spa_occupation_resistance" => "占领区抵抗",
        "spa_international_outrage" => "国际愤怒",
        "spa_syndicalist_institutionalization" => "工团制度化",
        "spa_postwar_debt_pressure" => "战后债务压力",
        "spa_demobilization_pressure" => "复员压力",
        "spr_government_authority" => "共和国政府权威",
        "spr_militia_autonomy" => "民兵自治",
        "spr_revolutionary_pressure" => "革命压力",
        "spr_soviet_dependency" => "苏联依赖",
        "spr_cnt_tension" => "CNT 紧张度",
        other => other,
    }
}

impl Effect {
    /// Generate a brief human-readable summary string for tooltip / detail panel display.
    /// Returns `None` for internal-only effects (flags, variables) that aren't meaningful to players.
    pub fn effect_summary(&self) -> Option<String> {
        match self {
            // ── 政治力量 / 稳定 / 战争支持 ──
            Self::AddPoliticalPower(v) => Some(format!("政治点数 {:+.0}", v)),
            Self::AddStability(v) => Some(format!("稳定度 {:+.0}%", v * 100.0)),
            Self::AddWarSupport(v) => Some(format!("战争支持度 {:+.0}%", v * 100.0)),

            // ── 人力 / 燃油 ──
            Self::AddManpower(v) => Some(format!("人力 {:+}", v)),
            Self::AddFuel(v) => Some(format!("燃油 {:+.0}", v)),

            // ── 经验 ──
            Self::ArmyExperience(v) => Some(format!("陆军经验 {:+.0}", v)),
            Self::NavyExperience(v) => Some(format!("海军经验 {:+.0}", v)),
            Self::AirExperience(v) => Some(format!("空军经验 {:+.0}", v)),

            // ── Flags (internal, don't show) ──
            Self::SetCountryFlag(_)
            | Self::SetCountryFlagForDays { .. }
            | Self::ClearCountryFlag(_)
            | Self::SetGlobalFlag(_)
            | Self::ClearGlobalFlag(_) => None,

            // ── Ideas ──
            Self::AddIdea(id) => Some(format!("Add idea: {}", id)),
            Self::RemoveIdea(id) => Some(format!("Remove idea: {}", id)),
            Self::SwapIdea { remove, add } => Some(format!("Swap idea: {} → {}", remove, add)),

            // ── 政治 ──
            Self::SetPolitics { ruling_party } => Some(format!("Ruling party → {}", ruling_party)),
            Self::AddPopularity { ideology, amount } => {
                Some(format!("{} popularity {:+.0}%", ideology, amount * 100.0))
            }
            Self::SetPartyName { ideology, name } => {
                Some(format!("Set {} party name: {}", ideology, name))
            }

            // ── 科研 ──
            Self::AddResearchSlot(v) => Some(format!("Research slots {:+}", v)),
            Self::AddTechBonus { category, bonus } => Some(format!(
                "{} research bonus {:+.0}%",
                category,
                bonus * 100.0
            )),
            Self::SetTechnology(t) => Some(format!("Unlock tech: {}", t)),

            // ── 工厂 / 建筑 ──
            Self::AddBuildingInState {
                state,
                building,
                level,
            } => {
                let bname = match building.as_str() {
                    "industrial_complex" => "Civilian factories",
                    "arms_factory" => "Military factories",
                    "dockyard" => "Dockyards",
                    "infrastructure" => "Infrastructure",
                    "air_base" => "Air base",
                    "anti_air_building" => "Anti-air",
                    "radar_station" => "Radar",
                    _ => building.as_str(),
                };
                Some(format!("{} {:+} (state {})", bname, level, state))
            }
            Self::AddBuildingAllStates { building, level } => {
                let bname = match building.as_str() {
                    "industrial_complex" => "Civilian factories",
                    "arms_factory" => "Military factories",
                    "infrastructure" => "Infrastructure",
                    _ => building.as_str(),
                };
                Some(format!("{} {:+} (all states)", bname, level))
            }
            Self::AddBuildingLevel {
                state,
                building_id,
                level,
            } => Some(format!("{} {:+} (state {})", building_id, level, state)),
            Self::AddExtraBuildingSlots { state, slots } => {
                Some(format!("Building slots +{} (state {})", slots, state))
            }

            // ── 资源 ──
            Self::AddResource {
                state,
                resource,
                amount,
            } => Some(format!("{} {:+} (state {})", resource, amount, state)),
            Self::AddResourceDiscovery {
                state,
                good_id,
                discovered_level,
            } => Some(format!(
                "Discover {} +{} levels (state {})",
                good_id, discovered_level, state
            )),
            Self::AddProductionMethodUnlock { pm_id } => {
                Some(format!("Unlock production method: {}", pm_id))
            }
            Self::AddGovernmentOrder {
                equipment_category,
                daily_budget_rm,
                duration_days,
            } => Some(format!(
                "政府军工订单：{} RM {:.0}/日，持续 {} 天",
                equipment_category, daily_budget_rm, duration_days
            )),
            Self::AddTradeAgreement {
                partner,
                good_id,
                daily_quantity,
            } => Some(format!(
                "贸易协定：与 {} 交易 {} {:.1}/日",
                partner, good_id, daily_quantity
            )),
            Self::ChangeLaw {
                category,
                law_id,
                lock_days,
            } => Some(format!(
                "{} law -> {} (locked {} days)",
                category, law_id, lock_days
            )),
            Self::AddMefoCapacity { amount_rm } => {
                Some(format!("MEFO capacity RM {:+.0}", amount_rm))
            }
            Self::AddPrivateInvestmentPool { amount_rm } => {
                Some(format!("Private investment pool RM {:+.0}", amount_rm))
            }
            Self::AddConstructionCapacity { amount } => {
                Some(format!("Construction capacity {:+}", amount))
            }
            Self::AddMilitarySpendingShare { delta } => {
                Some(format!("Military spending share {:+.1}%", delta * 100.0))
            }

            // ── 军事 ──
            Self::AddConscription(v) => Some(format!("Conscription {:+.1}%", v * 100.0)),
            Self::CreateDivision { template, province } => {
                Some(format!("Create {} (prov {})", template, province))
            }

            // ── 外交 ──
            Self::AddNamedThreat(v) => Some(format!("World tension {:+.1}%", v * 100.0)),
            Self::AddOpinion { target, amount } => Some(format!("{} 关系 {:+}", target, amount)),
            Self::CreateFaction(name) => Some(format!("Create faction: {}", name)),
            Self::AddToFaction(tag) => Some(format!("Add {} to faction", tag)),
            Self::LeaveFaction => Some("Leave faction".to_string()),
            Self::DeclareWarOn(tag) => Some(format!("Declare war on {}", tag)),
            Self::AnnexCountry(tag) => Some(format!("Annex {}", tag)),
            Self::PuppetCountry(tag) => Some(format!("Puppet {}", tag)),
            Self::FreeCountry(tag) => Some(format!("Free {}", tag)),
            Self::GiveMilitaryAccess(tag) => Some(format!("Military access to {}", tag)),
            Self::NonAggressionPact(tag) => Some(format!("Non-aggression pact with {}", tag)),
            Self::GuaranteeIndependence(tag) => Some(format!("Guarantee {}", tag)),

            // ── 州转移 / 核心 ──
            Self::TransferState(s) => Some(format!("Gain state {}", s)),
            Self::TransferStateTo { state, country } => {
                Some(format!("Transfer state {} to {}", state, country))
            }
            Self::CreateCountry { tag, .. } => Some(format!("Create country: {}", tag)),
            Self::AddCoreTo { state, country } => {
                Some(format!("{} gains core on state {}", country, state))
            }
            Self::RemoveCoreFrom { state, country } => {
                Some(format!("{} loses core on state {}", country, state))
            }
            Self::AddClaim { state, country } => {
                Some(format!("{} claims state {}", country, state))
            }

            // ── 决议 ──
            Self::SpawnDivisionsInStates {
                country,
                states,
                count_per_state,
                ..
            } => Some(format!(
                "Spawn {} divisions for {} in {} states",
                count_per_state.saturating_mul(states.len() as u32),
                country,
                states.len()
            )),
            Self::UnlockDecision(id) => Some(format!("解锁决议：{}", id)),
            Self::RemoveDecision(id) => Some(format!("移除决议：{}", id)),

            // ── 变量 (P2.7：第一轮事件后果预览) ──
            Self::SetVariable { name, value } => {
                Some(format!("{} = {:.1}", variable_display_name(name), value))
            }
            Self::AddToVariable { name, value } => {
                Some(format!("{} {:+.1}", variable_display_name(name), value))
            }
            Self::AddToCountryVariable { tag, name, value } => Some(format!(
                "{} {} {:+.1}",
                tag,
                variable_display_name(name),
                value
            )),
            Self::ClampVariable { name, min, max } => Some(format!(
                "{} 限制在 [{:.1}, {:.1}]",
                variable_display_name(name),
                min,
                max
            )),
            Self::ClampCountryVariable {
                tag,
                name,
                min,
                max,
            } => Some(format!(
                "{} {} 限制在 [{:.1}, {:.1}]",
                tag,
                variable_display_name(name),
                min,
                max
            )),
            Self::ComputeSpanishPrewarSettlement { .. } => None,

            // ── 人物 (P2.7：第一轮事件后果预览) ──
            Self::KillCharacter(key) => Some(format!("人物死亡：{key}")),
            Self::ExileCharacter(key) => Some(format!("人物流亡：{key}")),
            Self::ImprisonCharacter(key) => Some(format!("人物被捕：{key}")),
            Self::RecruitCharacter(key) => Some(format!("招募人物：{key}")),
            Self::SetCharacterStatus { key, status } => Some(format!("人物 {key} → {:?}", status)),
            Self::SetLeader(key) => Some(format!("领导人 → {key}")),

            // ── 杂项 ──
            Self::TriggerEvent(id) => Some(format!("Trigger event: {}", id)),
            Self::SwitchPlayerCountry(tag) => Some(format!("Switch player country: {}", tag)),
            Self::AddModifier {
                name,
                duration_days,
            } => {
                if *duration_days > 0 {
                    Some(format!("Modifier: {} ({} days)", name, duration_days))
                } else {
                    Some(format!("Modifier: {} (permanent)", name))
                }
            }
            Self::RemoveModifier(name) => Some(format!("Remove modifier: {}", name)),

            // ── 条件执行 ──
            Self::If { effects, .. } => {
                let summaries: Vec<String> =
                    effects.iter().filter_map(|e| e.effect_summary()).collect();
                if summaries.is_empty() {
                    None
                } else {
                    Some(format!("(conditional) {}", summaries.join(", ")))
                }
            }
        }
    }
}

impl FocusTree {
    /// 从 RON 字符串解析。
    pub fn from_ron(s: &str) -> Result<Self, ron::error::SpannedError> {
        ron::from_str(s)
    }

    /// 序列化为 RON 字符串。
    pub fn to_ron(&self) -> Result<String, ron::Error> {
        ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ron_round_trip_dynamic_country_effects() {
        let effects = vec![
            Effect::CreateCountry {
                tag: "CZE".to_owned(),
                color: [54, 167, 156],
                ruling_party: "fascism".to_owned(),
            },
            Effect::TransferStateTo {
                state: 75,
                country: "CZE".to_owned(),
            },
        ];
        let text = ron::ser::to_string(&effects).expect("serialize effects");
        println!("{text}");
        let parsed: Vec<Effect> = ron::from_str(&text).expect("parse effects");
        assert_eq!(parsed, effects);
    }
}
