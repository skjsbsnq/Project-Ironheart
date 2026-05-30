//! V5 阶段 F.2：自研决议（Decision）RON 系统。
//!
//! ## 与 F.1 事件的区别
//!
//! - **事件 = 自动**：调度器 daily tick MTTH，命中即弹 modal，玩家被动处理。
//! - **决议 = 主动**：玩家在政治面板按按钮才执行；schema 含 `cost_political_power`、
//!   `days_re_enable`（冷却）、`days_mission_timeout`（任务持续期）、`fire_only_once`。
//!
//! ## 三种决议形态
//!
//! 1. **Instant**（`days_mission_timeout = 0`）：点 → 扣 PP → 跑 `on_activation` →
//!    进入冷却（如果 `days_re_enable > 0`）。例：Speech / Diplomatic Pressure。
//! 2. **Mission**（`days_mission_timeout > 0`）：点 → 扣 PP → 跑 `on_activation` →
//!    进入「进行中」状态 → N 天后跑 `on_complete` → 进入冷却。例：Volkswagen Factory
//!    建造 90 天 → 完成发 idea / 加工厂。
//! 2.5 **`cancel_trigger`** 命中时 mission 中途终止，跑 `on_cancel`，不进冷却。
//! 3. **Once-only**（`fire_only_once = true`）：触发后永久禁用，不冷却。
//!
//! ## RON 雏形
//!
//! ```ron
//! DecisionDb(
//!     decisions: [
//!         Decision(
//!             id: "ger.volkswagen_factory",
//!             name: "Build Volkswagen Factory",
//!             description: "...",
//!             icon: "GFX_decision_volkswagen",
//!             category: Industry,
//!             visible: HasGovernment("fascism"),
//!             available: PoliticalPower(50.0),
//!             cost_political_power: 50.0,
//!             days_mission_timeout: 90,
//!             days_re_enable: 365,
//!             fire_only_once: true,
//!             on_activation: [SetCountryFlag("vw_in_progress")],
//!             on_complete: [
//!                 ClearCountryFlag("vw_in_progress"),
//!                 AddBuildingInState(state: 64, building: "industrial_complex", level: 2),
//!             ],
//!             cancel_trigger: HasWar(true),
//!             on_cancel: [ClearCountryFlag("vw_in_progress")],
//!         ),
//!     ],
//! )
//! ```

use serde::{Deserialize, Serialize};

use crate::focus::{Effect, Trigger};

/// 决议分类（用于政治面板 tab 分组）。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum DecisionCategory {
    /// 工业 / 建造（Volkswagen, Hermann Goering Werke, Synthetic Oil ...）
    Industry,
    /// 外交 / 阵营（Improve Relations, NAP, Demand Territory ...）
    Diplomacy,
    /// 军事 / 训练（Train Army, Reservist Drill, Partial Mobilisation ...）
    Military,
    /// 内政 / 宣传（Goebbels Speech, Suppress Opposition, Strength Rally ...）
    Internal,
    /// 危机 / 一次性（Burn the Reichstag Once-only ...）
    Crisis,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
pub enum DecisionMechanicKind {
    #[default]
    Standard,
    Progress,
    Timed,
    Cooldown,
    Gauge,
    BalanceOfPower,
    MapTargets,
    SpainRepublicAuthority,
    SpainSovietInfluence,
    SpainCntTension,
    SpainMadridDefense,
    SpainFrancoAuthority,
    SpainRightBalance,
    SpainForeignDependency,
    SpainOccupationOrder,
    SpainCntRevolutionaryCommittee,
    SpainBarcelonaTension,
    SpainRevolutionOrCompromise,
}

impl DecisionMechanicKind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Standard => "普通决议",
            Self::Progress => "进度决议",
            Self::Timed => "时限决议",
            Self::Cooldown => "冷却决议",
            Self::Gauge => "数值机制",
            Self::BalanceOfPower => "权力平衡",
            Self::MapTargets => "地图目标",
            Self::SpainRepublicAuthority => "共和国权威",
            Self::SpainSovietInfluence => "苏联影响",
            Self::SpainCntTension => "CNT 紧张度",
            Self::SpainMadridDefense => "马德里防御",
            Self::SpainFrancoAuthority => "佛朗哥权威",
            Self::SpainRightBalance => "右翼派系平衡",
            Self::SpainForeignDependency => "外援依赖",
            Self::SpainOccupationOrder => "占领区秩序",
            Self::SpainCntRevolutionaryCommittee => "革命委员会",
            Self::SpainBarcelonaTension => "巴塞罗那紧张度",
            Self::SpainRevolutionOrCompromise => "革命输出 vs 生存妥协",
        }
    }
}

impl DecisionCategory {
    /// 用于面板 tab 标签的人类可读名。
    pub fn label(&self) -> &'static str {
        match self {
            Self::Industry => "Industry",
            Self::Diplomacy => "Diplomacy",
            Self::Military => "Military",
            Self::Internal => "Internal",
            Self::Crisis => "Crisis",
        }
    }
}

/// 单个决议定义。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Decision {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub icon: String,
    pub category: DecisionCategory,
    #[serde(default)]
    pub mechanic_kind: DecisionMechanicKind,

    // ── 可见 / 可点 ──
    /// 决议在面板中可见的条件（`AlwaysTrue` = 始终显示）。
    #[serde(default = "Trigger::always_true")]
    pub visible: Trigger,
    /// 按钮可点击的条件。即便不满足也会显示，但灰显。
    #[serde(default = "Trigger::always_true")]
    pub available: Trigger,

    // ── 成本 ──
    /// 激活消耗的政治力量（< 当前 PP 才能点）。
    #[serde(default)]
    pub cost_political_power: f32,

    // ── 持续 / 冷却 / 一次性 ──
    /// 任务持续天数。0 = 即时决议（点 → 立刻跑 on_activation → 完成）；
    /// > 0 = mission 决议（点 → 进入活跃 → N 天后跑 on_complete）。
    #[serde(default)]
    pub days_mission_timeout: u32,
    /// 完成 / 取消后的冷却天数（再次启用前需等待）。0 = 无冷却。
    #[serde(default)]
    pub days_re_enable: u32,
    /// `true` = 全局只能触发一次。
    #[serde(default)]
    pub fire_only_once: bool,

    // ── 效果块 ──
    /// 激活时立即跑（扣 PP 之后）。
    #[serde(default)]
    pub on_activation: Vec<Effect>,
    /// mission 决议到期时跑。即时决议（timeout=0）也用此字段，
    /// 为单一 instant 决议把效果都写在这里。
    #[serde(default)]
    pub on_complete: Vec<Effect>,
    /// mission 进行中若该 trigger 满足，立即取消，跑 on_cancel，不进冷却。
    /// `AlwaysFalse` = 永不取消。
    #[serde(default = "Trigger::always_false")]
    pub cancel_trigger: Trigger,
    #[serde(default)]
    pub on_cancel: Vec<Effect>,
}

impl Trigger {
    /// 决议 `cancel_trigger` 默认值；与 `always_true` 对应。
    pub fn always_false() -> Self {
        Self::AlwaysFalse
    }
}

impl Decision {
    /// `true` = 即时决议（无 mission 阶段）。
    pub fn is_instant(&self) -> bool {
        self.days_mission_timeout == 0
    }
}

/// 决议库（一组定义的容器，RON 顶层节点）。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DecisionDb {
    pub decisions: Vec<Decision>,
}

impl DecisionDb {
    pub fn from_ron(s: &str) -> Result<Self, ron::error::SpannedError> {
        ron::from_str(s)
    }

    pub fn to_ron(&self) -> Result<String, ron::Error> {
        ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default())
    }

    pub fn find(&self, id: &str) -> Option<&Decision> {
        self.decisions.iter().find(|d| d.id == id)
    }

    /// 按分类排序后的引用列表（用于政治面板分 tab）。
    pub fn by_category(&self, cat: DecisionCategory) -> Vec<&Decision> {
        self.decisions
            .iter()
            .filter(|d| d.category == cat)
            .collect()
    }

    /// 校验决议库结构。
    pub fn validate(&self) -> Vec<DecisionValidationError> {
        let mut errors = Vec::new();

        if self.decisions.is_empty() {
            errors.push(DecisionValidationError::EmptyDb);
            return errors;
        }

        let mut seen = std::collections::HashSet::new();
        for d in &self.decisions {
            if !seen.insert(d.id.as_str()) {
                errors.push(DecisionValidationError::DuplicateId(d.id.clone()));
            }
            if d.cost_political_power < 0.0 {
                errors.push(DecisionValidationError::NegativeCost(d.id.clone()));
            }
            if d.is_instant() && d.on_activation.is_empty() && d.on_complete.is_empty() {
                errors.push(DecisionValidationError::EmptyEffects(d.id.clone()));
            }
        }
        errors
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum DecisionValidationError {
    EmptyDb,
    DuplicateId(String),
    NegativeCost(String),
    /// 即时决议（timeout=0）但 on_activation 与 on_complete 都为空 → 无意义。
    EmptyEffects(String),
}

impl std::fmt::Display for DecisionValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyDb => write!(f, "decision db is empty"),
            Self::DuplicateId(id) => write!(f, "duplicate decision id: {id}"),
            Self::NegativeCost(id) => write!(f, "decision '{id}' has negative cost"),
            Self::EmptyEffects(id) => {
                write!(
                    f,
                    "decision '{id}' is instant but has no on_activation/on_complete effects"
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_decision(id: &str) -> Decision {
        Decision {
            id: id.into(),
            name: id.into(),
            description: String::new(),
            icon: String::new(),
            category: DecisionCategory::Industry,
            mechanic_kind: DecisionMechanicKind::Standard,
            visible: Trigger::AlwaysTrue,
            available: Trigger::AlwaysTrue,
            cost_political_power: 25.0,
            days_mission_timeout: 0,
            days_re_enable: 0,
            fire_only_once: false,
            on_activation: vec![Effect::AddPoliticalPower(-25.0)],
            on_complete: vec![],
            cancel_trigger: Trigger::AlwaysFalse,
            on_cancel: vec![],
        }
    }

    #[test]
    fn parse_minimal_decision() {
        let ron = r#"
            DecisionDb(
                decisions: [
                    Decision(
                        id: "ger.test",
                        name: "Test",
                        category: Industry,
                        cost_political_power: 50.0,
                        on_complete: [AddPoliticalPower(-50.0), AddStability(0.05)],
                    ),
                ],
            )
        "#;
        let db = DecisionDb::from_ron(ron).expect("parse");
        assert_eq!(db.decisions.len(), 1);
        assert_eq!(db.decisions[0].id, "ger.test");
        assert!(db.decisions[0].is_instant());
        assert_eq!(db.decisions[0].cost_political_power, 50.0);
        // Defaults
        assert_eq!(db.decisions[0].days_mission_timeout, 0);
        assert_eq!(db.decisions[0].days_re_enable, 0);
        assert!(!db.decisions[0].fire_only_once);
    }

    #[test]
    fn parse_mission_decision() {
        let ron = r#"
            DecisionDb(
                decisions: [
                    Decision(
                        id: "ger.vw",
                        name: "Volkswagen",
                        category: Industry,
                        cost_political_power: 75.0,
                        days_mission_timeout: 90,
                        days_re_enable: 365,
                        fire_only_once: true,
                        on_activation: [SetCountryFlag("vw_in_progress")],
                        on_complete: [
                            ClearCountryFlag("vw_in_progress"),
                            AddBuildingAllStates(building: "industrial_complex", level: 1),
                        ],
                        cancel_trigger: HasWar(true),
                        on_cancel: [ClearCountryFlag("vw_in_progress")],
                    ),
                ],
            )
        "#;
        let db = DecisionDb::from_ron(ron).expect("parse");
        let d = &db.decisions[0];
        assert!(!d.is_instant());
        assert_eq!(d.days_mission_timeout, 90);
        assert_eq!(d.days_re_enable, 365);
        assert!(d.fire_only_once);
    }

    #[test]
    fn validate_empty_db() {
        let db = DecisionDb::default();
        let errs = db.validate();
        assert_eq!(errs, vec![DecisionValidationError::EmptyDb]);
    }

    #[test]
    fn validate_duplicate_id() {
        let db = DecisionDb {
            decisions: vec![sample_decision("dup"), sample_decision("dup")],
        };
        let errs = db.validate();
        assert!(errs.contains(&DecisionValidationError::DuplicateId("dup".into())));
    }

    #[test]
    fn validate_negative_cost() {
        let mut d = sample_decision("ng");
        d.cost_political_power = -10.0;
        let db = DecisionDb { decisions: vec![d] };
        let errs = db.validate();
        assert!(errs.contains(&DecisionValidationError::NegativeCost("ng".into())));
    }

    #[test]
    fn validate_empty_effects_instant() {
        let mut d = sample_decision("empty");
        d.on_activation = vec![];
        d.on_complete = vec![];
        let db = DecisionDb { decisions: vec![d] };
        let errs = db.validate();
        assert!(errs.contains(&DecisionValidationError::EmptyEffects("empty".into())));
    }

    #[test]
    fn by_category_filters() {
        let mut a = sample_decision("a");
        let mut b = sample_decision("b");
        b.category = DecisionCategory::Diplomacy;
        let db = DecisionDb {
            decisions: vec![a.clone(), b.clone()],
        };
        a.id = "x".into(); // not in db, just for shape
        let industry = db.by_category(DecisionCategory::Industry);
        assert_eq!(industry.len(), 1);
        assert_eq!(industry[0].id, "a");
        let diplo = db.by_category(DecisionCategory::Diplomacy);
        assert_eq!(diplo.len(), 1);
        assert_eq!(diplo[0].id, "b");
    }

    #[test]
    fn round_trip_ron() {
        let original = DecisionDb {
            decisions: vec![Decision {
                id: "ger.rt".into(),
                name: "RT".into(),
                description: "round trip".into(),
                icon: "GFX_x".into(),
                category: DecisionCategory::Military,
                mechanic_kind: DecisionMechanicKind::Standard,
                visible: Trigger::HasGovernment("fascism".into()),
                available: Trigger::PoliticalPower(50.0),
                cost_political_power: 50.0,
                days_mission_timeout: 60,
                days_re_enable: 180,
                fire_only_once: false,
                on_activation: vec![Effect::SetCountryFlag("active".into())],
                on_complete: vec![Effect::ArmyExperience(15.0)],
                cancel_trigger: Trigger::HasWar(true),
                on_cancel: vec![Effect::ClearCountryFlag("active".into())],
            }],
        };
        let s = original.to_ron().expect("to_ron");
        let restored = DecisionDb::from_ron(&s).expect("from_ron");
        assert_eq!(original.decisions, restored.decisions);
    }
}
