//! V5 阶段 F.2：决议运行时状态 + 每日 tick + 玩家激活入口。
//!
//! ## 责任分工
//!
//! - **状态** [`DecisionState`]：跟踪 `active_missions`（mission 决议进行中：id +
//!   剩余天数）、`cooldowns`（id → 剩余冷却天数）、`fired_once`（fire_only_once
//!   命中过的 id 集合）、`last_tick_key`（防同日重复 tick）。
//! - **玩家激活** [`activate`]：校验可见 / 可点 / 冷却 / 一次性 / PP 充足 →
//!   扣 PP → 跑 `on_activation` → 即时决议直接跑 `on_complete` + 进冷却；mission
//!   决议入 `active_missions` 等 tick 推进。
//! - **每日 tick** [`daily_decision_tick`]：mission 倒计时；到期跑 `on_complete`
//!   + 进冷却；`cancel_trigger` 命中跑 `on_cancel` + 不进冷却；冷却 / once 状态
//!   清理。
//!
//! ## 与 F.1 事件的对比
//!
//! | 维度 | 事件（F.1） | 决议（F.2） |
//! |---|---|---|
//! | 触发 | 调度器 daily MTTH | 玩家点 / 测试代码 `activate` |
//! | 队列 | `pending` FIFO 弹 modal | `active_missions` 倒计时 |
//! | 一次性 | `fire_only_once` 永久禁用 | 同义 |
//! | 冷却 | 无 | `days_re_enable` |
//! | 取消 | n/a | `cancel_trigger` 中途打断 |

use hoi4_state::{CountryId, World};
use std::collections::{HashMap, HashSet};

use crate::decision::{Decision, DecisionDb};
use crate::eval::{eval_trigger, run_effects, GlobalFlags};

/// 玩家激活决议的失败原因。
#[derive(Debug, Clone, PartialEq)]
pub enum ActivateError {
    /// 决议 id 在 db 里没有。
    NotFound,
    /// `visible` trigger 没满足（用户不该能点到，但 API 防御）。
    NotVisible,
    /// `available` trigger 没满足。
    NotAvailable,
    /// 政治力量不足以支付 cost。
    InsufficientPoliticalPower { have: f32, need: f32 },
    /// 已在冷却期。
    OnCooldown { remaining_days: u32 },
    /// 已在进行中（mission 决议）。
    AlreadyActive,
    /// fire_only_once 已触发过。
    AlreadyFiredOnce,
}

impl std::fmt::Display for ActivateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound => write!(f, "decision not in db"),
            Self::NotVisible => write!(f, "decision not visible"),
            Self::NotAvailable => write!(f, "available trigger not satisfied"),
            Self::InsufficientPoliticalPower { have, need } => {
                write!(f, "need {need:.0} PP, have {have:.0}")
            }
            Self::OnCooldown { remaining_days } => {
                write!(f, "on cooldown for {remaining_days} more days")
            }
            Self::AlreadyActive => write!(f, "already active"),
            Self::AlreadyFiredOnce => write!(f, "already fired once (fire_only_once)"),
        }
    }
}

/// 进行中的 mission 决议实例。
#[derive(Debug, Clone, PartialEq)]
pub struct MissionInstance {
    pub decision_id: String,
    pub country: CountryId,
    /// 剩余天数（递减到 0 → 跑 on_complete）。
    pub days_remaining: u32,
    /// 总长度（only used by UI to compute progress %）。
    pub total_days: u32,
}

/// 决议运行时状态（每个 World 一个；跨国通用，但 F.2 只用玩家国）。
#[derive(Debug, Clone, Default)]
pub struct DecisionState {
    /// 活跃 mission 列表。
    pub active_missions: Vec<MissionInstance>,
    /// 决议 id → 剩余冷却天数。
    pub cooldowns: HashMap<String, u32>,
    /// fire_only_once 已触发过的 id 集合。
    pub fired_once: HashSet<String>,
    /// 防同日双 tick。
    last_tick_key: u64,
}

impl DecisionState {
    pub fn new() -> Self {
        Self::default()
    }

    /// 当前是否在某决议的冷却期。
    pub fn is_on_cooldown(&self, id: &str) -> bool {
        self.cooldowns.get(id).copied().unwrap_or(0) > 0
    }

    /// 当前是否有该决议的 mission 在进行中。
    pub fn is_active(&self, id: &str) -> Option<&MissionInstance> {
        self.active_missions.iter().find(|m| m.decision_id == id)
    }

    /// fire_only_once 是否已触发过。
    pub fn already_fired(&self, id: &str) -> bool {
        self.fired_once.contains(id)
    }

    /// 玩家激活某决议。
    pub fn activate(
        &mut self,
        decision_id: &str,
        db: &DecisionDb,
        world: &mut World,
        country: CountryId,
        flags: &mut GlobalFlags,
    ) -> Result<(), ActivateError> {
        flags.expire_country_flags(world);
        let Some(decision) = db.find(decision_id) else {
            return Err(ActivateError::NotFound);
        };

        // 重复防御
        if decision.fire_only_once && self.already_fired(decision_id) {
            return Err(ActivateError::AlreadyFiredOnce);
        }
        if let Some(remaining) = self.cooldowns.get(decision_id).copied().filter(|&v| v > 0) {
            return Err(ActivateError::OnCooldown {
                remaining_days: remaining,
            });
        }
        if self.is_active(decision_id).is_some() {
            return Err(ActivateError::AlreadyActive);
        }

        // Trigger 可见 / 可点
        if !eval_trigger(&decision.visible, world, country, flags) {
            return Err(ActivateError::NotVisible);
        }
        if !eval_trigger(&decision.available, world, country, flags) {
            return Err(ActivateError::NotAvailable);
        }

        // PP 成本
        let i = country.0 as usize;
        let have = world.countries.political_power[i];
        if have < decision.cost_political_power {
            return Err(ActivateError::InsufficientPoliticalPower {
                have,
                need: decision.cost_political_power,
            });
        }
        world.countries.political_power[i] = have - decision.cost_political_power;

        // 跑 on_activation
        // P1.3：收集效果报告
        let report = run_effects(&decision.on_activation, world, country, flags);
        if report.has_errors() {
            for e in &report.errors {
                println!("[decision] on_activation error for {decision_id}: {e}");
            }
        }

        if decision.is_instant() {
            // 即时决议：直接 on_complete + 冷却
            let report = run_effects(&decision.on_complete, world, country, flags);
            if report.has_errors() {
                for e in &report.errors {
                    println!("[decision] on_complete error for {decision_id}: {e}");
                }
            }
            self.start_cooldown(decision);
            if decision.fire_only_once {
                self.fired_once.insert(decision_id.to_owned());
            }
        } else {
            // mission 决议：入活跃列表
            self.active_missions.push(MissionInstance {
                decision_id: decision_id.to_owned(),
                country,
                days_remaining: decision.days_mission_timeout,
                total_days: decision.days_mission_timeout,
            });
        }
        Ok(())
    }

    fn start_cooldown(&mut self, decision: &Decision) {
        if decision.days_re_enable > 0 {
            self.cooldowns
                .insert(decision.id.clone(), decision.days_re_enable);
        }
    }
}

/// 决议每日 tick：推进 mission / 处理 cancel_trigger / 倒计时冷却。
///
/// 返回本日触发完成的决议 id 列表（既包含 on_complete 完成，也包含 cancel 的）。
pub fn daily_decision_tick(
    state: &mut DecisionState,
    db: &DecisionDb,
    world: &mut World,
    flags: &mut GlobalFlags,
) -> Vec<DecisionTickEvent> {
    flags.expire_country_flags(world);
    let key = day_key(world);
    if key == state.last_tick_key {
        return Vec::new();
    }
    state.last_tick_key = key;

    let mut events = Vec::new();

    // mission 推进 / 取消 / 完成
    let mut survivors: Vec<MissionInstance> = Vec::with_capacity(state.active_missions.len());
    let active = std::mem::take(&mut state.active_missions);
    for mut m in active {
        let Some(decision) = db.find(&m.decision_id) else {
            // db 漂移：丢掉（保留为 Cancelled 防止状态泄漏）
            events.push(DecisionTickEvent::Cancelled(m.decision_id));
            continue;
        };

        // cancel_trigger
        if eval_trigger(&decision.cancel_trigger, world, m.country, flags) {
            // P1.3：收集效果报告
            let report = run_effects(&decision.on_cancel, world, m.country, flags);
            if report.has_errors() {
                for e in &report.errors {
                    println!("[decision] on_cancel error for {}: {e}", m.decision_id);
                }
            }
            events.push(DecisionTickEvent::Cancelled(m.decision_id.clone()));
            continue;
        }

        // 倒计时
        m.days_remaining = m.days_remaining.saturating_sub(1);
        if m.days_remaining == 0 {
            // P1.3：收集效果报告
            let report = run_effects(&decision.on_complete, world, m.country, flags);
            if report.has_errors() {
                for e in &report.errors {
                    println!("[decision] on_complete error for {}: {e}", m.decision_id);
                }
            }
            if decision.fire_only_once {
                state.fired_once.insert(m.decision_id.clone());
            }
            state.start_cooldown(decision);
            events.push(DecisionTickEvent::Completed(m.decision_id));
        } else {
            survivors.push(m);
        }
    }
    state.active_missions = survivors;

    // 冷却倒计时
    state.cooldowns.retain(|_, days| {
        *days = days.saturating_sub(1);
        *days > 0
    });

    events
}

/// daily_decision_tick 的输出（按发生顺序）。
#[derive(Debug, Clone, PartialEq)]
pub enum DecisionTickEvent {
    /// mission 决议正常完成。
    Completed(String),
    /// mission 决议被 cancel_trigger 中途打断。
    Cancelled(String),
}

fn day_key(world: &World) -> u64 {
    (world.date.year as u64) * 400 + (world.date.month as u64) * 32 + world.date.day as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decision::{Decision, DecisionCategory, DecisionDb};
    use crate::focus::{Effect, Trigger};

    fn test_world() -> World {
        use hoi4_state::store::*;
        use hoi4_state::*;
        use std::collections::HashMap;
        use std::sync::Arc;

        let map = Arc::new(hoi4_map::GameMap {
            definitions: vec![],
            rgb_to_id: HashMap::new(),
            province_map: hoi4_map::ProvinceMap {
                width: 1,
                height: 1,
                pixels: vec![0],
            },
            adjacencies: vec![],
            special_adjacencies: vec![],
            heightmap: hoi4_map::Heightmap {
                width: 1,
                height: 1,
                pixels: vec![0],
            },
            terrain_bmp: hoi4_map::TerrainBitmap {
                width: 1,
                height: 1,
                pixels: vec![0],
                palette: [[0u8; 3]; 256],
            },
            terrain_catalog: hoi4_map::TerrainCatalog::default(),
            tree_definition_bmp: None,
            tree_indices: std::collections::HashSet::new(),
        });
        let data = Arc::new(hoi4_data::GameData {
            countries: HashMap::new(),
            states: vec![],
            province_owners: HashMap::new(),
            buildings: HashMap::new(),
            resources: HashMap::new(),
            equipment: HashMap::new(),
            technologies: HashMap::new(),
            tech_prereqs: HashMap::new(),
            ideologies: HashMap::new(),
            ideas: HashMap::new(),
            focus_trees: HashMap::new(),
            focus_to_tree: HashMap::new(),
            subunits: HashMap::new(),
            combat_tactics: HashMap::new(),
            division_templates: HashMap::new(),
            ship_classes: HashMap::new(),
            aircraft: HashMap::new(),
            oob_land: HashMap::new(),
            oob_naval: HashMap::new(),
            oob_air: HashMap::new(),
            country_histories: HashMap::new(),
            decision_categories: HashMap::new(),
            decisions: HashMap::new(),
            ..Default::default()
        });

        let mut countries = CountryStore::new(1);
        countries.tags[0] = "GER".to_owned();
        countries.political_power[0] = 100.0;
        countries.stability[0] = 0.5;
        countries.ruling_party[0] = "fascism".to_owned();

        World {
            date: GameDate::START,
            speed: GameSpeed::Paused,
            elapsed_hours: 0,
            provinces: ProvinceStore::new(0),
            states: StateStore::new(0),
            countries,
            divisions: DivisionStore::new(),
            ships: ShipStore::new(),
            fleets: FleetStore::new(),
            air_wings: AirWingStore::new(),
            diplomacy: DiplomacyState::new(),
            command: hoi4_state::CommandHierarchy::default(),
            map,
            data,
            tag_to_country: HashMap::new(),
            state_id_lookup: HashMap::new(),
            player: CountryId(0),
            random_seed: 0,
            game_unique_id: 0,
            path_cache: HashMap::new(),
            path_cache_day: 0,
            prov_div_index: HashMap::new(),
            country_state_index: Vec::new(),
            country_pop_index: Vec::new(),
            country_building_index: Vec::new(),
            country_division_index: Vec::new(),
            country_fleet_index: Vec::new(),
            country_air_wing_index: Vec::new(),
            runtime_country_indexes_valid: false,
            trade_export_surplus_index: HashMap::new(),
            player_armies: Vec::new(),
            player_locked_divisions: std::collections::HashSet::new(),
            next_army_id: 0,
            generals: Vec::new(),
            next_general_id: 0,
        }
    }

    fn dec(id: &str) -> Decision {
        Decision {
            id: id.into(),
            name: id.into(),
            description: String::new(),
            icon: String::new(),
            category: DecisionCategory::Industry,
            mechanic_kind: crate::decision::DecisionMechanicKind::Standard,
            visible: Trigger::AlwaysTrue,
            available: Trigger::AlwaysTrue,
            cost_political_power: 25.0,
            days_mission_timeout: 0,
            days_re_enable: 0,
            fire_only_once: false,
            on_activation: vec![],
            on_complete: vec![Effect::AddStability(0.05)],
            cancel_trigger: Trigger::AlwaysFalse,
            on_cancel: vec![],
        }
    }

    #[test]
    fn instant_decision_runs_all_effects() {
        let mut d = dec("inst");
        d.on_activation = vec![Effect::SetCountryFlag("act".into())];
        d.on_complete = vec![Effect::AddStability(0.1)];
        let db = DecisionDb { decisions: vec![d] };
        let mut state = DecisionState::new();
        let mut w = test_world();
        let mut f = GlobalFlags::default();
        state
            .activate("inst", &db, &mut w, CountryId(0), &mut f)
            .unwrap();
        // PP 扣了
        assert_eq!(w.countries.political_power[0], 75.0);
        // on_activation 跑了
        assert!(w.countries.ideas[0].iter().any(|x| x == "FLAG:act"));
        // on_complete 跑了
        assert!((w.countries.stability[0] - 0.6).abs() < 0.001);
    }

    #[test]
    fn insufficient_pp_blocks() {
        let mut d = dec("rich");
        d.cost_political_power = 200.0;
        let db = DecisionDb { decisions: vec![d] };
        let mut state = DecisionState::new();
        let mut w = test_world();
        let mut f = GlobalFlags::default();
        let err = state
            .activate("rich", &db, &mut w, CountryId(0), &mut f)
            .unwrap_err();
        assert!(matches!(
            err,
            ActivateError::InsufficientPoliticalPower { .. }
        ));
        // PP 没扣
        assert_eq!(w.countries.political_power[0], 100.0);
    }

    #[test]
    fn cooldown_enforced() {
        let mut d = dec("cd");
        d.days_re_enable = 3;
        let db = DecisionDb { decisions: vec![d] };
        let mut state = DecisionState::new();
        let mut w = test_world();
        let mut f = GlobalFlags::default();
        state
            .activate("cd", &db, &mut w, CountryId(0), &mut f)
            .unwrap();
        // 立刻 try again → cooldown
        let err = state
            .activate("cd", &db, &mut w, CountryId(0), &mut f)
            .unwrap_err();
        assert!(matches!(err, ActivateError::OnCooldown { .. }));
        // 推 3 天 → 解锁
        for i in 0..3 {
            w.date.day = (i + 2) as u8;
            daily_decision_tick(&mut state, &db, &mut w, &mut f);
        }
        assert!(!state.is_on_cooldown("cd"));
        // PP 充足才能再次激活
        w.countries.political_power[0] = 100.0;
        state
            .activate("cd", &db, &mut w, CountryId(0), &mut f)
            .unwrap();
    }

    #[test]
    fn fire_only_once_blocks_second() {
        let mut d = dec("once");
        d.fire_only_once = true;
        let db = DecisionDb { decisions: vec![d] };
        let mut state = DecisionState::new();
        let mut w = test_world();
        let mut f = GlobalFlags::default();
        state
            .activate("once", &db, &mut w, CountryId(0), &mut f)
            .unwrap();
        w.countries.political_power[0] = 100.0;
        let err = state
            .activate("once", &db, &mut w, CountryId(0), &mut f)
            .unwrap_err();
        assert_eq!(err, ActivateError::AlreadyFiredOnce);
    }

    #[test]
    fn mission_completes_after_timeout() {
        let mut d = dec("miss");
        d.days_mission_timeout = 3;
        d.on_complete = vec![Effect::AddStability(0.1)];
        let db = DecisionDb { decisions: vec![d] };
        let mut state = DecisionState::new();
        let mut w = test_world();
        let mut f = GlobalFlags::default();
        state
            .activate("miss", &db, &mut w, CountryId(0), &mut f)
            .unwrap();
        // 进入活跃，未完成
        assert_eq!(state.active_missions.len(), 1);
        assert!((w.countries.stability[0] - 0.5).abs() < 0.001);

        // 推 3 天
        for day_offset in 1..=3 {
            w.date.day = (day_offset + 1) as u8;
            let ev = daily_decision_tick(&mut state, &db, &mut w, &mut f);
            if day_offset < 3 {
                assert!(ev.is_empty());
            } else {
                assert_eq!(ev.len(), 1);
                assert_eq!(ev[0], DecisionTickEvent::Completed("miss".into()));
            }
        }
        assert_eq!(state.active_missions.len(), 0);
        assert!((w.countries.stability[0] - 0.6).abs() < 0.001);
    }

    #[test]
    fn cancel_trigger_aborts_mission() {
        let mut d = dec("warable");
        d.days_mission_timeout = 10;
        d.cancel_trigger = Trigger::HasGlobalFlag("at_war".into());
        d.on_complete = vec![Effect::AddStability(0.1)];
        d.on_cancel = vec![Effect::AddStability(-0.05)];
        let db = DecisionDb { decisions: vec![d] };
        let mut state = DecisionState::new();
        let mut w = test_world();
        let mut f = GlobalFlags::default();
        state
            .activate("warable", &db, &mut w, CountryId(0), &mut f)
            .unwrap();
        assert_eq!(state.active_missions.len(), 1);

        // 触发 cancel
        f.flags.insert("at_war".into());
        w.date.day = 2;
        let ev = daily_decision_tick(&mut state, &db, &mut w, &mut f);
        assert_eq!(ev.len(), 1);
        assert_eq!(ev[0], DecisionTickEvent::Cancelled("warable".into()));
        // on_cancel 跑了，on_complete 没跑
        assert!((w.countries.stability[0] - 0.45).abs() < 0.001);
    }

    #[test]
    fn already_active_blocks_re_activation() {
        let mut d = dec("ad");
        d.days_mission_timeout = 5;
        let db = DecisionDb { decisions: vec![d] };
        let mut state = DecisionState::new();
        let mut w = test_world();
        let mut f = GlobalFlags::default();
        state
            .activate("ad", &db, &mut w, CountryId(0), &mut f)
            .unwrap();
        let err = state
            .activate("ad", &db, &mut w, CountryId(0), &mut f)
            .unwrap_err();
        assert_eq!(err, ActivateError::AlreadyActive);
    }

    #[test]
    fn one_tick_per_day() {
        let mut d = dec("once_a_day");
        d.days_mission_timeout = 5;
        let db = DecisionDb { decisions: vec![d] };
        let mut state = DecisionState::new();
        let mut w = test_world();
        let mut f = GlobalFlags::default();
        state
            .activate("once_a_day", &db, &mut w, CountryId(0), &mut f)
            .unwrap();
        let m = &state.active_missions[0];
        assert_eq!(m.days_remaining, 5);
        // 同日两次 tick → 只推进一次
        w.date.day = 2;
        daily_decision_tick(&mut state, &db, &mut w, &mut f);
        daily_decision_tick(&mut state, &db, &mut w, &mut f);
        assert_eq!(state.active_missions[0].days_remaining, 4);
    }

    #[test]
    fn not_visible_blocks_activate() {
        let mut d = dec("hidden");
        d.visible = Trigger::HasGlobalFlag("never".into());
        let db = DecisionDb { decisions: vec![d] };
        let mut state = DecisionState::new();
        let mut w = test_world();
        let mut f = GlobalFlags::default();
        let err = state
            .activate("hidden", &db, &mut w, CountryId(0), &mut f)
            .unwrap_err();
        assert_eq!(err, ActivateError::NotVisible);
    }
}
