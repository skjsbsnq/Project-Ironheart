//! P0.1：统一内容运行时。
//!
//! `ContentRuntimeState` 集中保存 focus、event、decision、situation、global flags、
//! scripted surrender、last content day 等所有内容状态。
//!
//! `tick_content_daily` 从 app 层移入 runtime day-change 流程，
//! 在 `SystemSchedule` 的 Content daily 系统中执行。
//! GUI、headless、测试都走同一条日更路径。
//!
//! 高速推进多天时，`SystemSchedule` 每天触发 daily 系统，
//! 因此 content daily 逐日执行，不会跳过中间日期。

use hoi4_ai::StrategicProfileReport;
use hoi4_content::{DecisionDb, DecisionTickEvent, TickResult};
use hoi4_logic::diplomacy::{PeaceResolutionOutcome, SurrenderOutcome};
use hoi4_state::{CountryId, GameSpeed, World};

/// 统一内容运行状态。集中保存所有 RON/self-authored 内容的运行时数据。
/// GUI、headless、测试共享同一套状态。
#[derive(Debug, Clone)]
pub struct ContentRuntimeState {
    pub focus_tree: hoi4_content::FocusTree,
    pub global_flags: hoi4_content::GlobalFlags,
    pub event_scheduler: hoi4_content::EventScheduler,
    pub decision_state: hoi4_content::DecisionState,
    pub decision_db: DecisionDb,
    pub situation_state: hoi4_content::SituationState,
    pub scripted_surrenders: Vec<hoi4_content::ScriptedSurrenderDef>,
    /// 上次 content daily 执行的游戏日
    pub last_content_day: i64,
    /// 上次 strategic weekly tick 的 epoch days
    pub last_strategic_week: i64,
    /// 玩家国家
    pub player: CountryId,
    /// 最近一次 content daily 的结果（供 app 层读取）
    pub last_tick_events: ContentTickEvents,
    /// P1.1：diplomacy_daily 写入的和平结算结果（供 content_daily 读取并合并到 last_tick_events）
    pub pending_peace_resolution: PeaceResolutionOutcome,
    pub elections: std::collections::HashMap<String, hoi4_content::Election1936>,
}

impl ContentRuntimeState {
    /// 从 ScenarioContent 构造统一内容运行状态。
    pub fn new(
        scenario_content: &hoi4_content::ScenarioContent,
        player: CountryId,
        initial_day: i64,
    ) -> Self {
        let focus_tree = scenario_content
            .primary_focus_tree()
            .expect("1936 scenario has focus tree")
            .clone();
        let event_scheduler =
            hoi4_content::EventScheduler::new(scenario_content.events.clone(), 0xC0FFEE);
        let situation_state = {
            let mut state = hoi4_content::SituationState::new();
            for def in &scenario_content.situations {
                state.add_def(def.clone());
            }
            state
        };
        Self {
            focus_tree,
            global_flags: hoi4_content::GlobalFlags::default(),
            event_scheduler,
            decision_state: hoi4_content::DecisionState::new(),
            decision_db: scenario_content.decisions.clone(),
            situation_state,
            scripted_surrenders: scenario_content.surrenders.clone(),
            last_content_day: initial_day - 1,
            last_strategic_week: -1,
            player,
            last_tick_events: ContentTickEvents::default(),
            pending_peace_resolution: PeaceResolutionOutcome::default(),
            elections: scenario_content.elections.clone(),
        }
    }

    /// 构造空内容运行状态（用于测试，不需要 ScenarioContent）。
    pub fn empty_for_test(player: CountryId, initial_day: i64) -> Self {
        Self {
            focus_tree: hoi4_content::FocusTree {
                country: String::new(),
                focuses: Vec::new(),
            },
            global_flags: hoi4_content::GlobalFlags::default(),
            event_scheduler: hoi4_content::EventScheduler::new(
                hoi4_content::EventDb::default(),
                0xC0FFEE,
            ),
            decision_state: hoi4_content::DecisionState::new(),
            decision_db: hoi4_content::DecisionDb::default(),
            situation_state: hoi4_content::SituationState::new(),
            scripted_surrenders: Vec::new(),
            last_content_day: initial_day - 1,
            last_strategic_week: -1,
            player,
            last_tick_events: ContentTickEvents::default(),
            pending_peace_resolution: PeaceResolutionOutcome::default(),
            elections: std::collections::HashMap::new(),
        }
    }

    /// 执行一次 content daily tick。
    /// 返回本次 tick 产生的事件。
    /// P0.3：focus_speed_factor 从 PoliticsCache 传入，确保 RON focus 使用正确的速度。
    pub fn tick_daily(&mut self, world: &mut World, focus_speed_factor: f32) -> ContentTickEvents {
        let current_day = world.date.days_since_epoch();
        let day_changed = current_day != self.last_content_day && world.speed != GameSpeed::Paused;
        if !day_changed {
            return ContentTickEvents::default();
        }

        self.last_content_day = current_day;

        let mut out = ContentTickEvents {
            day_changed: true,
            ..ContentTickEvents::default()
        };

        let result = hoi4_content::daily_focus_tick(
            world,
            self.player,
            &self.focus_tree,
            &mut self.global_flags,
            focus_speed_factor,
        );
        out.focus_completed = matches!(result, TickResult::Completed(_));

        let (fired, event_report) = hoi4_content::daily_event_tick(
            &mut self.event_scheduler,
            world,
            self.player,
            &mut self.global_flags,
        );
        out.fired_events = fired;
        // P1.3：汇总事件效果报告
        out.effect_report.merge(event_report);
        // P1.2：暂停判断检查 pending 队列中是否存在任意国家事件
        out.pause_for_country_event = self
            .event_scheduler
            .pending
            .iter()
            .any(|p| p.scope == hoi4_content::EventScope::Country);

        out.decision_events = hoi4_content::daily_decision_tick(
            &mut self.decision_state,
            &self.decision_db,
            world,
            &mut self.global_flags,
        );

        self.situation_state.check_start(world);

        let now_days = world.date.days_since_epoch();
        if now_days - self.last_strategic_week >= 7 {
            self.last_strategic_week = now_days;
            out.strategic_report = Some(hoi4_ai::weekly_strategic_tick(world));
            self.situation_state.ai_auto_intervene(world);
        }

        self.situation_state.daily_tick(world);

        out.surrender_outcome =
            hoi4_logic::diplomacy::apply_scripted_surrenders(world, &self.scripted_surrenders);
        if !out.surrender_outcome.resolved.is_empty() {
            for triggered in &out.surrender_outcome.triggered_events {
                // P1.2：投降触发事件时保留作用国家
                let effect_country = if let Some(cid) = world.country(&triggered.effect_country) {
                    cid
                } else {
                    self.player
                };
                self.global_flags
                    .pending_triggers
                    .push_back(hoi4_content::eval::PendingTrigger {
                        event_id: triggered.event_id.clone(),
                        effect_country,
                        display_country: self.player,
                        source: format!("surrender:{}", triggered.effect_country),
                    });
            }
        }

        let cascaded = self.process_pending_triggers(world);
        out.cascaded_triggers.extend(cascaded.cascaded_triggers);
        out.effect_report.merge(cascaded.effect_report);
        out.pause_for_country_event |= cascaded.pause_for_country_event;

        self.last_tick_events = out.clone();

        // P1.1：将 diplomacy_daily 写入的和平结算结果合并到 tick events
        std::mem::swap(
            &mut self.last_tick_events.peace_resolution,
            &mut self.pending_peace_resolution,
        );
        self.pending_peace_resolution = PeaceResolutionOutcome::default();

        out
    }

    /// Immediately process queued TriggerEvent requests into event popups/effects.
    /// Used after situation effects and after resolving event options, so country events pause now
    /// instead of waiting for the next unpaused daily tick.
    pub fn process_pending_triggers(&mut self, world: &mut World) -> ContentTickEvents {
        let mut out = ContentTickEvents::default();
        for _ in 0..16 {
            let Some(trigger) = self.global_flags.pending_triggers.pop_front() else {
                break;
            };
            let (triggered, cascade_report) = self.event_scheduler.trigger_scoped_for_player(
                &trigger.event_id,
                world,
                trigger.effect_country,
                trigger.display_country,
                self.player,
                &mut self.global_flags,
            );
            if triggered {
                out.cascaded_triggers.push(trigger.event_id.clone());
            } else {
                out.effect_report.warnings.push(format!(
                    "Pending TriggerEvent skipped: event id={} source={} effect_country={:?} display_country={:?}",
                    trigger.event_id, trigger.source, trigger.effect_country, trigger.display_country
                ));
            }
            out.effect_report.merge(cascade_report);
        }
        out.pause_for_country_event = self
            .event_scheduler
            .pending
            .iter()
            .any(|p| p.scope == hoi4_content::EventScope::Country);
        out
    }
}

#[derive(Debug, Default, Clone)]
pub struct ContentTickEvents {
    pub day_changed: bool,
    pub focus_completed: bool,
    pub fired_events: Vec<String>,
    pub pause_for_country_event: bool,
    pub decision_events: Vec<DecisionTickEvent>,
    pub strategic_report: Option<StrategicProfileReport>,
    pub surrender_outcome: SurrenderOutcome,
    pub cascaded_triggers: Vec<String>,
    /// P1.1：每日和平结算结果（一方全部投降后自动执行）
    pub peace_resolution: PeaceResolutionOutcome,
    /// P1.3：效果执行报告（包含 warnings、errors）
    pub effect_report: hoi4_content::eval::EffectReport,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, HashSet};
    use std::sync::Arc;

    fn minimal_world() -> World {
        let map = Arc::new(hoi4_map::GameMap {
            definitions: Vec::new(),
            rgb_to_id: HashMap::new(),
            province_map: hoi4_map::ProvinceMap {
                width: 0,
                height: 0,
                pixels: Vec::new(),
            },
            adjacencies: Vec::new(),
            special_adjacencies: Vec::new(),
            heightmap: hoi4_map::Heightmap {
                width: 0,
                height: 0,
                pixels: Vec::new(),
            },
            terrain_bmp: hoi4_map::TerrainBitmap {
                width: 0,
                height: 0,
                pixels: Vec::new(),
                palette: [[0; 3]; 256],
            },
            terrain_catalog: hoi4_map::TerrainCatalog::default(),
            tree_definition_bmp: None,
            tree_indices: HashSet::new(),
        });
        World::new(map, Arc::new(hoi4_data::GameData::default()))
    }

    #[test]
    fn pending_trigger_failure_reports_warning() {
        let mut runtime = ContentRuntimeState::empty_for_test(CountryId::NONE, 0);
        runtime
            .global_flags
            .pending_triggers
            .push_back(hoi4_content::eval::PendingTrigger {
                event_id: "missing.event".to_owned(),
                effect_country: CountryId::NONE,
                display_country: CountryId::NONE,
                source: "unit-test".to_owned(),
            });

        let mut world = minimal_world();
        let out = runtime.process_pending_triggers(&mut world);

        assert!(out.cascaded_triggers.is_empty());
        assert!(out.effect_report.has_warnings());
        assert!(out
            .effect_report
            .warnings
            .iter()
            .any(|warning| { warning.contains("missing.event") && warning.contains("unit-test") }));
    }
}
