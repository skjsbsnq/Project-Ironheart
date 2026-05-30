//! V5 阶段 F.1：自研事件 RON 系统。
//!
//! ## 设计目标
//!
//! - **不解析 vanilla `events/*.txt`**：V3 时代 `hoi4_script::events::EventDef`
//!   仍保留为骨架（基于 `clausewitz_parser::Block`），但本 crate 不依赖它，
//!   F.1 走纯 RON 路线，与 D 阶段 focus 树同样的范式。
//! - **复用 `crate::focus::Trigger` / `crate::focus::Effect`**：所有 trigger /
//!   effect 走 D.2 已交付的 36/51 变体，不引入新枚举，保证 schema 一致性。
//! - **支持两种触发**：`is_triggered_only=true` 仅由 `Effect::TriggerEvent`
//!   主动触发；`is_triggered_only=false` 由调度器每日按 `mean_time_to_happen`
//!   随机评估并入队。
//! - **支持选项**：每个事件 1..N 个 `EventOption`，每个 option 自带可选
//!   `trigger`（不满足则灰显）+ `effects` 列表 + `ai_chance` 权重。
//!
//! ## RON 雏形
//!
//! ```ron
//! EventDb(
//!     events: [
//!         Event(
//!             id: "germany.rhineland",
//!             title: "Remilitarisation of the Rhineland",
//!             description: "...",
//!             is_triggered_only: false,
//!             fire_only_once: true,
//!             trigger: HasCompletedFocus("GER_rhineland"),
//!             mean_time_to_happen_days: 30,
//!             immediate: [],
//!             options: [
//!                 EventOption(
//!                     name: "Glory to the Reich!",
//!                     trigger: AlwaysTrue,
//!                     effects: [AddPoliticalPower(25.0), AddStability(0.05)],
//!                     ai_chance: 1.0,
//!                 ),
//!             ],
//!         ),
//!     ],
//! )
//! ```

use serde::{Deserialize, Serialize};

use crate::focus::{Effect, Trigger};

/// J.5b: Event scope — country (private) vs news (global broadcast).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum EventScope {
    /// Only the target country sees this event.
    #[default]
    Country,
    /// All countries see this event (newspaper style).
    News,
}

/// 单个事件定义。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Event {
    /// 事件 id（如 `"germany.rhineland"`）。
    pub id: String,
    /// 显示标题。
    pub title: String,
    /// 显示 flavor 描述。
    #[serde(default)]
    pub description: String,
    /// 事件 picture（GFX 名）；目前仅作为 metadata，UI 留扩展口。
    #[serde(default)]
    pub picture: String,
    /// J.5b: Event scope (Country or News). Default = Country.
    #[serde(default)]
    pub scope: EventScope,
    /// `true` = 只能由 `Effect::TriggerEvent` 主动触发，调度器不评估 MTTH。
    #[serde(default)]
    pub is_triggered_only: bool,
    /// `true` = 一局游戏中最多触发一次（`fire_only_once`）。
    #[serde(default = "Event::default_fire_only_once")]
    pub fire_only_once: bool,
    /// `true` = 隐藏事件，立即跑 immediate + 第一个 option 的 effects，不显示 modal。
    #[serde(default)]
    pub hidden: bool,
    /// 触发条件（满足才入队）。`AlwaysTrue` = 无前置条件。
    #[serde(default = "Trigger::always_true")]
    pub trigger: Trigger,
    /// 平均触发天数。trigger 通过后每天有 `1/mtth` 概率入队。0 = 立即入队。
    #[serde(default = "Event::default_mtth")]
    pub mean_time_to_happen_days: u32,
    /// 入队 → 弹窗显示前立即执行的 effects（如设置 country flag）。
    #[serde(default)]
    pub immediate: Vec<Effect>,
    /// 玩家可选选项，至少一个。
    pub options: Vec<EventOption>,
}

impl Event {
    fn default_fire_only_once() -> bool {
        true
    }
    fn default_mtth() -> u32 {
        30
    }
}

/// 事件选项（modal 上的一个按钮）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EventOption {
    /// 按钮文本（玩家看到的）。
    pub name: String,
    /// 选项可选条件：不满足则按钮灰显（仍显示）。`AlwaysTrue` = 始终可选。
    #[serde(default = "Trigger::always_true")]
    pub trigger: Trigger,
    /// 选中后执行的 effects。
    #[serde(default)]
    pub effects: Vec<Effect>,
    /// AI 选择此选项的相对权重（>= 0.0），归一化后用作概率。
    #[serde(default = "EventOption::default_ai_chance")]
    pub ai_chance: f32,
}

impl EventOption {
    fn default_ai_chance() -> f32 {
        1.0
    }
}

/// 事件库：一组事件定义的容器。RON 顶层节点。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EventDb {
    pub events: Vec<Event>,
}

impl EventDb {
    /// 从 RON 字符串解析。
    pub fn from_ron(s: &str) -> Result<Self, ron::error::SpannedError> {
        ron::from_str(s)
    }

    /// 序列化为 RON 字符串。
    pub fn to_ron(&self) -> Result<String, ron::Error> {
        ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default())
    }

    /// 按 id 查找事件（线性扫描；事件总数 < 100 不需要 HashMap）。
    pub fn find(&self, id: &str) -> Option<&Event> {
        self.events.iter().find(|e| e.id == id)
    }

    /// 校验事件库结构。返回所有错误，空 = 合法。
    pub fn validate(&self) -> Vec<EventValidationError> {
        let mut errors = Vec::new();

        if self.events.is_empty() {
            errors.push(EventValidationError::EmptyDb);
            return errors;
        }

        // Duplicate id
        let mut seen = std::collections::HashSet::new();
        for e in &self.events {
            if !seen.insert(e.id.as_str()) {
                errors.push(EventValidationError::DuplicateId(e.id.clone()));
            }
            if e.options.is_empty() {
                errors.push(EventValidationError::NoOptions(e.id.clone()));
            }
        }

        // TriggerEvent references existing event ids
        let ids: std::collections::HashSet<&str> =
            self.events.iter().map(|e| e.id.as_str()).collect();
        for e in &self.events {
            collect_trigger_event_refs(&e.immediate, &mut |target| {
                if !ids.contains(target) {
                    errors.push(EventValidationError::MissingTriggerTarget {
                        from: e.id.clone(),
                        target: target.to_owned(),
                    });
                }
            });
            for opt in &e.options {
                collect_trigger_event_refs(&opt.effects, &mut |target| {
                    if !ids.contains(target) {
                        errors.push(EventValidationError::MissingTriggerTarget {
                            from: format!("{}::{}", e.id, opt.name),
                            target: target.to_owned(),
                        });
                    }
                });
            }
        }

        errors
    }
}

/// 事件库校验错误。
#[derive(Debug, Clone, PartialEq)]
pub enum EventValidationError {
    EmptyDb,
    DuplicateId(String),
    NoOptions(String),
    /// `Effect::TriggerEvent("X")` 引用的事件 id `X` 不存在。
    MissingTriggerTarget {
        from: String,
        target: String,
    },
}

impl std::fmt::Display for EventValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyDb => write!(f, "event db is empty"),
            Self::DuplicateId(id) => write!(f, "duplicate event id: {id}"),
            Self::NoOptions(id) => write!(f, "event '{id}' has no options"),
            Self::MissingTriggerTarget { from, target } => {
                write!(f, "event '{from}' triggers missing event '{target}'")
            }
        }
    }
}

/// 递归收集 effects 中所有 `TriggerEvent("...")` 引用的事件 id。
fn collect_trigger_event_refs(effects: &[Effect], visit: &mut impl FnMut(&str)) {
    for e in effects {
        match e {
            Effect::TriggerEvent(id) => visit(id),
            Effect::If { effects, .. } => collect_trigger_event_refs(effects, visit),
            Effect::SwitchPlayerCountry(_) => {}
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_event_db() {
        let ron = r#"
            EventDb(
                events: [
                    Event(
                        id: "test.1",
                        title: "Hello",
                        options: [
                            EventOption(name: "OK", effects: [AddPoliticalPower(10.0)]),
                        ],
                    ),
                ],
            )
        "#;
        let db = EventDb::from_ron(ron).expect("parse");
        assert_eq!(db.events.len(), 1);
        assert_eq!(db.events[0].id, "test.1");
        assert_eq!(db.events[0].title, "Hello");
        assert_eq!(db.events[0].options.len(), 1);
        // Defaults wired
        assert!(db.events[0].fire_only_once);
        assert_eq!(db.events[0].mean_time_to_happen_days, 30);
        assert_eq!(db.events[0].options[0].ai_chance, 1.0);
    }

    #[test]
    fn parse_full_event_db() {
        let ron = r#"
            EventDb(
                events: [
                    Event(
                        id: "ger.rhineland",
                        title: "Rhineland",
                        description: "Our troops march into Rhineland.",
                        picture: "GFX_event_rhineland",
                        is_triggered_only: true,
                        fire_only_once: true,
                        hidden: false,
                        trigger: HasCompletedFocus("GER_rhineland"),
                        mean_time_to_happen_days: 1,
                        immediate: [SetGlobalFlag("rhineland_in_progress")],
                        options: [
                            EventOption(
                                name: "Glory to the Reich!",
                                trigger: AlwaysTrue,
                                effects: [
                                    AddPoliticalPower(50.0),
                                    AddStability(0.05),
                                    AddNamedThreat(2.0),
                                ],
                                ai_chance: 1.0,
                            ),
                        ],
                    ),
                ],
            )
        "#;
        let db = EventDb::from_ron(ron).expect("parse");
        let e = &db.events[0];
        assert_eq!(e.id, "ger.rhineland");
        assert!(e.is_triggered_only);
        assert_eq!(e.options[0].effects.len(), 3);
    }

    #[test]
    fn validate_empty_db() {
        let db = EventDb::default();
        let errs = db.validate();
        assert_eq!(errs, vec![EventValidationError::EmptyDb]);
    }

    #[test]
    fn validate_duplicate_id() {
        let db = EventDb {
            events: vec![
                Event {
                    id: "x".into(),
                    title: "a".into(),
                    description: String::new(),
                    picture: String::new(),
                    scope: EventScope::Country,
                    is_triggered_only: false,
                    fire_only_once: true,
                    hidden: false,
                    trigger: Trigger::AlwaysTrue,
                    mean_time_to_happen_days: 10,
                    immediate: vec![],
                    options: vec![EventOption {
                        name: "ok".into(),
                        trigger: Trigger::AlwaysTrue,
                        effects: vec![],
                        ai_chance: 1.0,
                    }],
                },
                Event {
                    id: "x".into(),
                    title: "b".into(),
                    description: String::new(),
                    picture: String::new(),
                    scope: EventScope::Country,
                    is_triggered_only: false,
                    fire_only_once: true,
                    hidden: false,
                    trigger: Trigger::AlwaysTrue,
                    mean_time_to_happen_days: 10,
                    immediate: vec![],
                    options: vec![EventOption {
                        name: "ok".into(),
                        trigger: Trigger::AlwaysTrue,
                        effects: vec![],
                        ai_chance: 1.0,
                    }],
                },
            ],
        };
        let errs = db.validate();
        assert!(errs.contains(&EventValidationError::DuplicateId("x".into())));
    }

    #[test]
    fn validate_missing_trigger_target() {
        let db = EventDb {
            events: vec![Event {
                id: "a".into(),
                title: "a".into(),
                description: String::new(),
                picture: String::new(),
                scope: EventScope::Country,
                is_triggered_only: false,
                fire_only_once: true,
                hidden: false,
                trigger: Trigger::AlwaysTrue,
                mean_time_to_happen_days: 10,
                immediate: vec![],
                options: vec![EventOption {
                    name: "go".into(),
                    trigger: Trigger::AlwaysTrue,
                    effects: vec![Effect::TriggerEvent("nonexistent".into())],
                    ai_chance: 1.0,
                }],
            }],
        };
        let errs = db.validate();
        assert!(errs.iter().any(|e| matches!(e,
            EventValidationError::MissingTriggerTarget { target, .. } if target == "nonexistent"
        )));
    }

    #[test]
    fn validate_no_options() {
        let db = EventDb {
            events: vec![Event {
                id: "x".into(),
                title: "x".into(),
                description: String::new(),
                picture: String::new(),
                scope: EventScope::Country,
                is_triggered_only: false,
                fire_only_once: true,
                hidden: false,
                trigger: Trigger::AlwaysTrue,
                mean_time_to_happen_days: 10,
                immediate: vec![],
                options: vec![],
            }],
        };
        let errs = db.validate();
        assert!(errs.contains(&EventValidationError::NoOptions("x".into())));
    }

    #[test]
    fn find_by_id() {
        let db = EventDb {
            events: vec![Event {
                id: "found".into(),
                title: "x".into(),
                description: String::new(),
                picture: String::new(),
                scope: EventScope::Country,
                is_triggered_only: false,
                fire_only_once: true,
                hidden: false,
                trigger: Trigger::AlwaysTrue,
                mean_time_to_happen_days: 10,
                immediate: vec![],
                options: vec![EventOption {
                    name: "ok".into(),
                    trigger: Trigger::AlwaysTrue,
                    effects: vec![],
                    ai_chance: 1.0,
                }],
            }],
        };
        assert!(db.find("found").is_some());
        assert!(db.find("missing").is_none());
    }

    #[test]
    fn round_trip_ron() {
        let original = EventDb {
            events: vec![Event {
                id: "rt.1".into(),
                title: "Round trip".into(),
                description: "desc".into(),
                picture: "GFX_x".into(),
                scope: EventScope::Country,
                is_triggered_only: true,
                fire_only_once: true,
                hidden: false,
                trigger: Trigger::And(vec![
                    Trigger::HasCompletedFocus("GER_rhineland".into()),
                    Trigger::Date {
                        year: 1936,
                        month: 3,
                        day: 7,
                    },
                ]),
                mean_time_to_happen_days: 1,
                immediate: vec![Effect::SetCountryFlag("did_it".into())],
                options: vec![EventOption {
                    name: "Yes".into(),
                    trigger: Trigger::AlwaysTrue,
                    effects: vec![Effect::AddPoliticalPower(25.0)],
                    ai_chance: 0.7,
                }],
            }],
        };
        let s = original.to_ron().expect("to_ron");
        let restored = EventDb::from_ron(&s).expect("from_ron");
        assert_eq!(original.events, restored.events);
    }
}
