//! V5 阶段 F.1：事件调度器 / 每日 tick / 待弹窗队列。
//!
//! ## 责任分工
//!
//! - **调度器** [`EventScheduler`]：持有 `EventDb`、`fired_once` 集合、`pending`
//!   弹窗队列、`rng`（确定性 PCG，每日 advance）。
//! - **每日 tick** [`daily_event_tick`]：遍历所有非 `is_triggered_only` 事件，
//!   evaluate trigger（已 fire 过 `fire_only_once` 的跳过），过 → 按 MTTH 抛
//!   随机；命中 → 入 pending（先跑 immediate，再等 UI 弹）。
//! - **主动触发** [`EventScheduler::trigger`]：由 `Effect::TriggerEvent("X")`
//!   或测试代码调用，绕过 trigger / MTTH，直接入队（仍受 `fire_only_once`）。
//! - **选项执行** [`EventScheduler::resolve_option`]：玩家点 modal 按钮后，
//!   把选项 effects 跑进 World。
//!
//! ## 隐藏事件
//!
//! `hidden=true` 的事件不入 `pending`：调度器在 trigger 命中时直接跑
//! `immediate + options[0].effects`（AI 权重最高的那个），不打扰玩家。
//!
//! ## 玩家视角
//!
//! 当前 F.1 只为玩家国家（`country = world.player`）做 daily MTTH 评估。AI
//! 国家事件留给 F.3 阶段简化 AI 子系统按需扩展。

use hoi4_state::{CountryId, World};

use crate::eval::{eval_trigger, run_effects, GlobalFlags};
use crate::event::{Event, EventDb};
use crate::focus::Trigger;

/// 调度器：持有事件库 + 运行时状态。
#[derive(Debug, Clone)]
pub struct EventScheduler {
    /// 静态事件库（启动时加载 RON）。
    pub db: EventDb,
    /// 已触发过的 `fire_only_once` 事件 id 集合。
    pub fired_once: std::collections::HashSet<String>,
    /// 待玩家选择的事件队列（FIFO，UI 每帧弹最前一个）。
    pub pending: std::collections::VecDeque<PendingEvent>,
    /// 上一次 daily tick 的日期键（year * 400 + month * 32 + day），用于检测「日变化」。
    last_tick_key: u64,
    /// PCG-32 风格确定性 RNG 状态（基于 World.random_seed 初始化）。
    rng_state: u64,
    /// P2.7：隐藏事件触发日志。隐藏事件不弹窗，但需写入此处供调试和事件历史读取。
    /// 仅保留最近 256 条，避免长局游戏内存膨胀。
    pub hidden_log: std::collections::VecDeque<HiddenEventLogEntry>,
    country_event_cooldown_until: std::collections::HashMap<CountryId, i64>,
    deferred_triggers: std::collections::VecDeque<DeferredTrigger>,
}

/// P2.7：隐藏事件触发日志条目。
#[derive(Debug, Clone, PartialEq)]
pub struct HiddenEventLogEntry {
    /// 事件 id（如 `hidden.scw_madrid_status_router`）。
    pub event_id: String,
    /// 触发时游戏日（自 epoch 的天数）。
    pub day: i64,
    /// 触发时作用国家（用于按国家筛选日志）。
    pub country: CountryId,
}

#[derive(Debug, Clone, PartialEq)]
struct DeferredTrigger {
    event_id: String,
    effect_country: CountryId,
    display_country: CountryId,
    earliest_day: i64,
}

/// 入队事件实例：在 modal 上需要展示的内容。
#[derive(Debug, Clone, PartialEq)]
pub struct PendingEvent {
    pub event_id: String,
    /// 接收事件的国家。F.1 只为玩家入队，但保留字段以备 F.3 扩展。
    pub country: CountryId,
    /// J.5b: Event scope for UI rendering distinction.
    pub scope: crate::event::EventScope,
}

/// 调度器构造器输入：种子来自 `World.random_seed`，确保确定性回放。
impl EventScheduler {
    /// 用一组事件 + 起始 RNG 种子构造。
    pub fn new(db: EventDb, seed: u64) -> Self {
        Self {
            db,
            fired_once: Default::default(),
            pending: Default::default(),
            last_tick_key: 0,
            // 避免种子=0 → PCG 退化全 0 序列
            rng_state: seed.wrapping_add(0x9E3779B97F4A7C15),
            hidden_log: Default::default(),
            country_event_cooldown_until: Default::default(),
            deferred_triggers: Default::default(),
        }
    }

    /// P2.7：写入隐藏事件日志（保留最近 256 条）。
    fn push_hidden_log(&mut self, event_id: &str, world: &World, country: CountryId) {
        const MAX_LOG: usize = 256;
        if self.hidden_log.len() >= MAX_LOG {
            self.hidden_log.pop_front();
        }
        self.hidden_log.push_back(HiddenEventLogEntry {
            event_id: event_id.to_owned(),
            day: world.date.days_since_epoch(),
            country,
        });
    }

    /// 每日 tick：对所有国家评估非 triggered-only 事件的 trigger + MTTH。
    /// 玩家国家事件入队显示；AI 国家事件直接按 ai_chance 自动执行。
    /// `country` 是玩家国家，用于区分 UI 入队与 AI 自动选择。
    /// P1.3：返回 (fired_ids, effect_report)。
    pub fn daily_tick(
        &mut self,
        world: &mut World,
        country: CountryId,
        flags: &mut GlobalFlags,
    ) -> (Vec<String>, crate::eval::EffectReport) {
        let mut fired = Vec::new();
        let mut report = crate::eval::EffectReport::default();
        let key = day_key(world);
        if key == self.last_tick_key {
            return (fired, report);
        }
        self.last_tick_key = key;
        self.fire_due_deferred_triggers(world, country, flags, &mut fired, &mut report);

        let event_count = self.db.events.len();
        for event_idx in 0..event_count {
            if self.db.events[event_idx].is_triggered_only {
                continue;
            }
            if self.db.events[event_idx].fire_only_once
                && self.fired_once.contains(&self.db.events[event_idx].id)
            {
                continue;
            }
            if trigger_is_date_blocked(&self.db.events[event_idx].trigger, world) {
                continue;
            }

            let candidate_countries =
                candidate_countries_for_trigger(&self.db.events[event_idx].trigger, world);
            let event = self.db.events[event_idx].clone();

            for scoped_country in candidate_countries {
                if world.diplomacy.annexed_countries.contains(&scoped_country) {
                    continue;
                }
                if !eval_trigger(&event.trigger, world, scoped_country, flags) {
                    continue;
                }
                if self.is_country_event_on_cooldown(&event, world, scoped_country) {
                    continue;
                }

                let hit = if event.mean_time_to_happen_days == 0 {
                    true
                } else {
                    let r = self.next_f32();
                    r < (1.0 / event.mean_time_to_happen_days as f32)
                };
                if !hit {
                    continue;
                }

                if event.scope == crate::event::EventScope::News {
                    self.fire_news(&event, world, scoped_country, country, flags, &mut report);
                } else if scoped_country == country {
                    self.fire(&event, world, scoped_country, flags, &mut report);
                    self.note_country_event_fired(&event, world, scoped_country);
                } else {
                    self.fire_ai(&event, world, scoped_country, flags, &mut report);
                }
                let tag = world.country_tag(scoped_country).unwrap_or("???");
                fired.push(format!("{}@{tag}", event.id));
                if event.fire_only_once {
                    break;
                }
            }
        }
        (fired, report)
    }

    /// 主动触发事件（绕过 trigger / MTTH，但受 fire_only_once 限制）。
    ///
    /// 返回 `true` = 成功入队 / 立即执行；`false` = id 不存在或已触发过。
    /// P1.3：返回 (success, effect_report)，失败时 report 包含诊断 warning。
    pub fn trigger(
        &mut self,
        event_id: &str,
        world: &mut World,
        country: CountryId,
        flags: &mut GlobalFlags,
    ) -> (bool, crate::eval::EffectReport) {
        self.trigger_scoped(event_id, world, country, country, flags)
    }

    /// P1.2：带作用域的主动触发事件。
    ///
    /// `effect_country` 是效果执行上下文国家（如法国投降事件中法国）。
    /// `display_country` 是事件显示国家（新闻事件中可能不同于作用国家）。
    /// P1.3：返回 (success, effect_report)，失败时 report 包含诊断 warning。
    pub fn trigger_scoped(
        &mut self,
        event_id: &str,
        world: &mut World,
        effect_country: CountryId,
        display_country: CountryId,
        flags: &mut GlobalFlags,
    ) -> (bool, crate::eval::EffectReport) {
        let mut report = crate::eval::EffectReport::default();
        let Some(event) = self.db.find(event_id).cloned() else {
            push_trigger_warning(&mut report, event_id, "missing event");
            return (false, report);
        };
        if event.fire_only_once && self.fired_once.contains(event_id) {
            push_trigger_warning(&mut report, event_id, "fire_only_once already fired");
            return (false, report);
        }
        if self.defer_if_country_event_on_cooldown(&event, world, effect_country, display_country) {
            return (true, report);
        }
        if event.scope == crate::event::EventScope::News {
            self.fire_news(
                &event,
                world,
                effect_country,
                display_country,
                flags,
                &mut report,
            );
        } else {
            self.fire(&event, world, effect_country, flags, &mut report);
        }
        (true, report)
    }

    /// 玩家点 modal 上某个选项后调用。`option_idx` 是选项数组下标。
    ///
    /// 把选项 effects 跑进 World，然后从 pending 头部弹掉。
    /// 返回 `true` = 处理成功；`false` = pending 为空 / 选项 idx 越界 / event id 丢失。
    /// P1.3：返回效果报告，失败时 report 包含诊断 warning。
    pub fn trigger_scoped_for_player(
        &mut self,
        event_id: &str,
        world: &mut World,
        effect_country: CountryId,
        display_country: CountryId,
        player: CountryId,
        flags: &mut GlobalFlags,
    ) -> (bool, crate::eval::EffectReport) {
        let mut report = crate::eval::EffectReport::default();
        let Some(event) = self.db.find(event_id).cloned() else {
            push_trigger_warning(&mut report, event_id, "missing event");
            return (false, report);
        };
        if event.fire_only_once && self.fired_once.contains(event_id) {
            push_trigger_warning(&mut report, event_id, "fire_only_once already fired");
            return (false, report);
        }
        if self.defer_if_country_event_on_cooldown(&event, world, effect_country, display_country) {
            return (true, report);
        }
        if event.scope == crate::event::EventScope::News {
            self.fire_news(
                &event,
                world,
                effect_country,
                display_country,
                flags,
                &mut report,
            );
        } else if effect_country == player || display_country == player {
            self.fire(&event, world, effect_country, flags, &mut report);
            self.note_country_event_fired(&event, world, display_country);
        } else {
            self.fire_ai(&event, world, effect_country, flags, &mut report);
        }
        (true, report)
    }

    fn scw_country_event_spacing_days(
        &self,
        event: &Event,
        world: &World,
        display_country: CountryId,
    ) -> Option<i64> {
        if event.hidden || event.scope != crate::event::EventScope::Country {
            return None;
        }
        let tag = world.country_tag(display_country)?;
        if tag == "SPA" && event.id.starts_with("spa.scw_") {
            Some(14)
        } else {
            None
        }
    }

    fn is_country_event_on_cooldown(
        &self,
        event: &Event,
        world: &World,
        display_country: CountryId,
    ) -> bool {
        self.scw_country_event_spacing_days(event, world, display_country)
            .is_some()
            && self
                .country_event_cooldown_until
                .get(&display_country)
                .is_some_and(|&day| world.date.days_since_epoch() < day)
    }

    fn note_country_event_fired(
        &mut self,
        event: &Event,
        world: &World,
        display_country: CountryId,
    ) {
        if let Some(spacing) = self.scw_country_event_spacing_days(event, world, display_country) {
            self.country_event_cooldown_until
                .insert(display_country, world.date.days_since_epoch() + spacing);
        }
    }

    fn defer_if_country_event_on_cooldown(
        &mut self,
        event: &Event,
        world: &World,
        effect_country: CountryId,
        display_country: CountryId,
    ) -> bool {
        let Some(spacing) = self.scw_country_event_spacing_days(event, world, display_country)
        else {
            return false;
        };
        let today = world.date.days_since_epoch();
        let cooldown_until = self
            .country_event_cooldown_until
            .get(&display_country)
            .copied()
            .unwrap_or(today);
        if today >= cooldown_until {
            return false;
        }
        self.deferred_triggers.push_back(DeferredTrigger {
            event_id: event.id.clone(),
            effect_country,
            display_country,
            earliest_day: cooldown_until.max(today + spacing),
        });
        true
    }

    fn fire_due_deferred_triggers(
        &mut self,
        world: &mut World,
        player: CountryId,
        flags: &mut GlobalFlags,
        fired: &mut Vec<String>,
        report: &mut crate::eval::EffectReport,
    ) {
        if self.deferred_triggers.is_empty() {
            return;
        }
        let today = world.date.days_since_epoch();
        let mut remaining = std::collections::VecDeque::new();
        let mut fired_deferred = false;
        while let Some(mut trigger) = self.deferred_triggers.pop_front() {
            if fired_deferred || trigger.earliest_day > today {
                remaining.push_back(trigger);
                continue;
            }
            let Some(event) = self.db.find(&trigger.event_id).cloned() else {
                continue;
            };
            if self.is_country_event_on_cooldown(&event, world, trigger.display_country) {
                trigger.earliest_day = self
                    .country_event_cooldown_until
                    .get(&trigger.display_country)
                    .copied()
                    .unwrap_or(today + 1);
                remaining.push_back(trigger);
                continue;
            }
            if event.scope == crate::event::EventScope::News {
                self.fire_news(
                    &event,
                    world,
                    trigger.effect_country,
                    trigger.display_country,
                    flags,
                    report,
                );
            } else if trigger.effect_country == player || trigger.display_country == player {
                self.fire(&event, world, trigger.effect_country, flags, report);
                self.note_country_event_fired(&event, world, trigger.display_country);
            } else {
                self.fire_ai(&event, world, trigger.effect_country, flags, report);
            }
            let tag = world.country_tag(trigger.display_country).unwrap_or("???");
            fired.push(format!("{}@{}", trigger.event_id, tag));
            fired_deferred = true;
        }
        remaining.extend(std::mem::take(&mut self.deferred_triggers));
        self.deferred_triggers = remaining;
    }

    pub fn resolve_option(
        &mut self,
        option_idx: usize,
        world: &mut World,
        flags: &mut GlobalFlags,
    ) -> (bool, crate::eval::EffectReport) {
        let mut report = crate::eval::EffectReport::default();
        let Some(pending) = self.pending.front().cloned() else {
            report
                .warnings
                .push("ResolveEventOption skipped: no pending event".to_owned());
            return (false, report);
        };
        let Some(event) = self.db.find(&pending.event_id).cloned() else {
            self.pending.pop_front();
            report.warnings.push(format!(
                "ResolveEventOption skipped: missing event id={}",
                pending.event_id
            ));
            return (false, report);
        };
        let Some(opt) = event.options.get(option_idx) else {
            report.warnings.push(format!(
                "ResolveEventOption skipped: invalid option index {option_idx} for event id={} options={}",
                event.id,
                event.options.len()
            ));
            return (false, report);
        };
        report.merge(run_effects(&opt.effects, world, pending.country, flags));
        self.pending.pop_front();
        (true, report)
    }

    /// pending 队列首部事件（UI 每帧读这个）。
    pub fn front(&self) -> Option<&PendingEvent> {
        self.pending.front()
    }

    /// 取队列长度（用于显示「还有 N 个事件待处理」）。
    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }

    // --- 内部辅助 ---

    /// 实际触发：跑 immediate；hidden 直接跑第一个 option；非 hidden 入队。
    /// P1.3：累积效果报告。
    fn fire(
        &mut self,
        event: &Event,
        world: &mut World,
        country: CountryId,
        flags: &mut GlobalFlags,
        report: &mut crate::eval::EffectReport,
    ) {
        if event.fire_only_once {
            self.fired_once.insert(event.id.clone());
        }
        report.merge(run_effects(&event.immediate, world, country, flags));
        if event.hidden {
            // hidden 自动取第一个 option（AI 路径）；后续可按 ai_chance 加权。
            if let Some(opt) = event.options.first() {
                report.merge(run_effects(&opt.effects, world, country, flags));
            }
            // P2.7：隐藏事件不弹窗，但写日志便于调试和事件历史读取。
            self.push_hidden_log(&event.id, world, country);
        } else {
            self.pending.push_back(PendingEvent {
                event_id: event.id.clone(),
                country,
                scope: event.scope,
            });
        }
    }

    fn fire_news(
        &mut self,
        event: &Event,
        world: &mut World,
        effect_country: CountryId,
        display_country: CountryId,
        flags: &mut GlobalFlags,
        report: &mut crate::eval::EffectReport,
    ) {
        if event.fire_only_once {
            self.fired_once.insert(event.id.clone());
        }
        report.merge(run_effects(&event.immediate, world, effect_country, flags));
        if event.hidden {
            if let Some(opt) = event.options.first() {
                report.merge(run_effects(&opt.effects, world, effect_country, flags));
            }
            // P2.7：隐藏事件不弹窗，但写日志便于调试和事件历史读取。
            self.push_hidden_log(&event.id, world, effect_country);
        } else {
            self.pending.push_back(PendingEvent {
                event_id: event.id.clone(),
                country: display_country,
                scope: event.scope,
            });
        }
    }

    fn fire_ai(
        &mut self,
        event: &Event,
        world: &mut World,
        country: CountryId,
        flags: &mut GlobalFlags,
        report: &mut crate::eval::EffectReport,
    ) {
        if event.fire_only_once {
            self.fired_once.insert(event.id.clone());
        }
        report.merge(run_effects(&event.immediate, world, country, flags));
        if let Some(opt) = event
            .options
            .iter()
            .filter(|opt| eval_trigger(&opt.trigger, world, country, flags))
            .max_by(|a, b| {
                a.ai_chance
                    .partial_cmp(&b.ai_chance)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
        {
            report.merge(run_effects(&opt.effects, world, country, flags));
        }
    }

    /// PCG-32 步进，输出 [0.0, 1.0) f32。确定性可重放。
    fn next_f32(&mut self) -> f32 {
        // 经典 PCG-XSH-RR 32 输出（input = u64 state）
        let oldstate = self.rng_state;
        self.rng_state = oldstate
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let xorshifted = (((oldstate >> 18) ^ oldstate) >> 27) as u32;
        let rot = (oldstate >> 59) as u32;
        let r = xorshifted.rotate_right(rot);
        // 24 bit mantissa
        ((r >> 8) as f32) / (1u32 << 24) as f32
    }
}

/// P1.3：每日 tick 的便捷函数，返回 (fired_ids, effect_report)。
pub fn daily_event_tick(
    scheduler: &mut EventScheduler,
    world: &mut World,
    country: CountryId,
    flags: &mut GlobalFlags,
) -> (Vec<String>, crate::eval::EffectReport) {
    scheduler.daily_tick(world, country, flags)
}

fn day_key(world: &World) -> u64 {
    (world.date.year as u64) * 400 + (world.date.month as u64) * 32 + world.date.day as u64
}

fn push_trigger_warning(report: &mut crate::eval::EffectReport, event_id: &str, reason: &str) {
    report.warnings.push(format!(
        "TriggerEvent skipped: event id={event_id} reason={reason}"
    ));
}

fn trigger_is_date_blocked(trigger: &Trigger, world: &World) -> bool {
    match trigger {
        Trigger::Date { year, month, day } => {
            let target = hoi4_state::GameDate {
                year: *year,
                month: *month,
                day: *day,
                hour: 0,
            };
            world.date < target
        }
        Trigger::And(parts) => parts
            .iter()
            .any(|part| trigger_is_date_blocked(part, world)),
        Trigger::Or(parts) => {
            !parts.is_empty()
                && parts
                    .iter()
                    .all(|part| trigger_is_date_blocked(part, world))
        }
        Trigger::Not(_) => false,
        _ => false,
    }
}

fn candidate_countries_for_trigger(trigger: &Trigger, world: &World) -> Vec<CountryId> {
    let Some(tags) = trigger_country_candidates(trigger) else {
        return (0..world.countries.count)
            .map(|ci| CountryId(ci as u16))
            .collect();
    };

    tags.into_iter()
        .filter_map(|tag| world.tag_to_country.get(tag).copied())
        .collect()
}

fn trigger_country_candidates(trigger: &Trigger) -> Option<Vec<&str>> {
    match trigger {
        Trigger::Tag(tag) => Some(vec![tag.as_str()]),
        Trigger::And(parts) => {
            let mut out: Option<Vec<&str>> = None;
            for part in parts {
                let Some(mut part_tags) = trigger_country_candidates(part) else {
                    continue;
                };
                sort_dedup_tags(&mut part_tags);
                out = Some(match out {
                    Some(mut existing) => {
                        existing.retain(|tag| part_tags.contains(tag));
                        existing
                    }
                    None => part_tags,
                });
            }
            out
        }
        Trigger::Or(parts) => {
            let mut out = Vec::new();
            for part in parts {
                let part_tags = trigger_country_candidates(part)?;
                out.extend(part_tags);
            }
            sort_dedup_tags(&mut out);
            Some(out)
        }
        Trigger::Not(_) => None,
        _ => None,
    }
}

fn sort_dedup_tags(tags: &mut Vec<&str>) {
    tags.sort_unstable();
    tags.dedup();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{Event, EventOption};
    use crate::focus::{Effect, Trigger};

    fn test_world() -> World {
        // Reuse same minimal World as eval.rs tests.
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

        let mut countries = CountryStore::new(2);
        countries.tags[0] = "GER".to_owned();
        countries.tags[1] = "POL".to_owned();
        countries.political_power[0] = 50.0;
        countries.stability[0] = 0.5;
        countries.completed_focuses[0].insert("GER_rhineland".to_owned());

        let mut tag_to_country = HashMap::new();
        tag_to_country.insert("GER".to_owned(), CountryId(0));
        tag_to_country.insert("POL".to_owned(), CountryId(1));

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
            tag_to_country,
            state_id_lookup: HashMap::new(),
            player: CountryId(0),
            random_seed: 42,
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

    fn ev(id: &str, mtth: u32, fire_only_once: bool, trigger: Trigger) -> Event {
        Event {
            id: id.into(),
            title: id.into(),
            description: String::new(),
            picture: String::new(),
            scope: crate::event::EventScope::Country,
            is_triggered_only: false,
            fire_only_once,
            hidden: false,
            trigger,
            mean_time_to_happen_days: mtth,
            immediate: vec![],
            options: vec![EventOption {
                name: "OK".into(),
                trigger: Trigger::AlwaysTrue,
                effects: vec![Effect::AddPoliticalPower(10.0)],
                ai_chance: 1.0,
            }],
        }
    }

    fn make_second_country_spa(world: &mut World) -> CountryId {
        world.countries.tags[1] = "SPA".to_owned();
        world.tag_to_country.remove("POL");
        world.tag_to_country.insert("SPA".to_owned(), CountryId(1));
        CountryId(1)
    }

    #[test]
    fn mtth_zero_fires_immediately() {
        let db = EventDb {
            events: vec![ev("e1", 0, true, Trigger::AlwaysTrue)],
        };
        let mut sched = EventScheduler::new(db, 1);
        let mut world = test_world();
        let mut flags = GlobalFlags::default();
        let (fired, _) = sched.daily_tick(&mut world, CountryId(0), &mut flags);
        assert_eq!(fired, vec!["e1@GER".to_owned()]);
        assert_eq!(sched.pending_len(), 1);
        assert!(sched.fired_once.contains("e1"));
    }

    #[test]
    fn fire_only_once_blocks_second() {
        let db = EventDb {
            events: vec![ev("e1", 0, true, Trigger::AlwaysTrue)],
        };
        let mut sched = EventScheduler::new(db, 1);
        let mut world = test_world();
        let mut flags = GlobalFlags::default();
        let _ = sched.daily_tick(&mut world, CountryId(0), &mut flags);
        // Tick again on a different day — should NOT re-fire.
        world.date.day += 1;
        let (fired2, _) = sched.daily_tick(&mut world, CountryId(0), &mut flags);
        assert!(fired2.is_empty());
    }

    #[test]
    fn trigger_gates_event() {
        let db = EventDb {
            events: vec![ev(
                "blocked",
                0,
                true,
                Trigger::HasCompletedFocus("never_done".into()),
            )],
        };
        let mut sched = EventScheduler::new(db, 1);
        let mut world = test_world();
        let mut flags = GlobalFlags::default();
        let (fired, _) = sched.daily_tick(&mut world, CountryId(0), &mut flags);
        assert!(fired.is_empty());
        assert_eq!(sched.pending_len(), 0);
    }

    #[test]
    fn tag_trigger_only_evaluates_matching_country() {
        let db = EventDb {
            events: vec![ev("pol_only", 0, true, Trigger::Tag("POL".into()))],
        };
        let mut sched = EventScheduler::new(db, 1);
        let mut world = test_world();
        let mut flags = GlobalFlags::default();

        let (fired, _) = sched.daily_tick(&mut world, CountryId(0), &mut flags);

        assert_eq!(fired, vec!["pol_only@POL".to_owned()]);
        assert_eq!(world.countries.political_power[1], 10.0);
        assert_eq!(sched.pending_len(), 0);
    }

    #[test]
    fn future_date_trigger_skips_until_reached() {
        let db = EventDb {
            events: vec![ev(
                "future_pol",
                0,
                true,
                Trigger::And(vec![
                    Trigger::Tag("POL".into()),
                    Trigger::Date {
                        year: 1936,
                        month: 1,
                        day: 3,
                    },
                ]),
            )],
        };
        let mut sched = EventScheduler::new(db, 1);
        let mut world = test_world();
        let mut flags = GlobalFlags::default();

        let (before, _) = sched.daily_tick(&mut world, CountryId(0), &mut flags);
        assert!(before.is_empty());

        world.date.day = 3;
        let (after, _) = sched.daily_tick(&mut world, CountryId(0), &mut flags);
        assert_eq!(after, vec!["future_pol@POL".to_owned()]);
    }

    #[test]
    fn resolve_option_runs_effects() {
        let db = EventDb {
            events: vec![ev("e1", 0, true, Trigger::AlwaysTrue)],
        };
        let mut sched = EventScheduler::new(db, 1);
        let mut world = test_world();
        let mut flags = GlobalFlags::default();
        sched.daily_tick(&mut world, CountryId(0), &mut flags);
        let pp_before = world.countries.political_power[0];
        let (ok, _) = sched.resolve_option(0, &mut world, &mut flags);
        assert!(ok);
        assert_eq!(world.countries.political_power[0], pp_before + 10.0);
        assert_eq!(sched.pending_len(), 0);
    }

    #[test]
    fn manual_trigger_bypasses_mtth() {
        // Even with mtth=10000, manual trigger fires.
        let mut e = ev("manual", 10000, true, Trigger::AlwaysFalse);
        e.is_triggered_only = true;
        let db = EventDb { events: vec![e] };
        let mut sched = EventScheduler::new(db, 1);
        let mut world = test_world();
        let mut flags = GlobalFlags::default();
        // Daily tick respects is_triggered_only — does nothing.
        let (fired, _) = sched.daily_tick(&mut world, CountryId(0), &mut flags);
        assert!(fired.is_empty());
        // Manual trigger fires.
        let (ok, _) = sched.trigger("manual", &mut world, CountryId(0), &mut flags);
        assert!(ok);
        assert_eq!(sched.pending_len(), 1);
    }

    #[test]
    fn hidden_event_runs_first_option_immediately() {
        let mut e = ev("hidden", 0, true, Trigger::AlwaysTrue);
        e.hidden = true;
        e.options[0].effects = vec![Effect::SetGlobalFlag("h_done".into())];
        let db = EventDb { events: vec![e] };
        let mut sched = EventScheduler::new(db, 1);
        let mut world = test_world();
        let mut flags = GlobalFlags::default();
        sched.daily_tick(&mut world, CountryId(0), &mut flags);
        assert_eq!(sched.pending_len(), 0);
        assert!(flags.flags.contains("h_done"));
    }

    #[test]
    fn immediate_runs_before_modal() {
        let mut e = ev("imm", 0, true, Trigger::AlwaysTrue);
        e.immediate = vec![Effect::SetGlobalFlag("imm_run".into())];
        let db = EventDb { events: vec![e] };
        let mut sched = EventScheduler::new(db, 1);
        let mut world = test_world();
        let mut flags = GlobalFlags::default();
        sched.daily_tick(&mut world, CountryId(0), &mut flags);
        assert!(flags.flags.contains("imm_run"));
        // Modal still queued.
        assert_eq!(sched.pending_len(), 1);
    }

    #[test]
    fn one_tick_per_day() {
        let db = EventDb {
            events: vec![ev("e1", 0, false, Trigger::AlwaysTrue)],
        };
        let mut sched = EventScheduler::new(db, 1);
        let mut world = test_world();
        let mut flags = GlobalFlags::default();
        // Two ticks on same day → only one fires.
        let (f1, _) = sched.daily_tick(&mut world, CountryId(0), &mut flags);
        let (f2, _) = sched.daily_tick(&mut world, CountryId(0), &mut flags);
        assert_eq!(f1.len(), world.countries.count);
        assert_eq!(f2.len(), 0);
    }

    /// P1.2 验收：trigger_scoped 作用于正确国家而非玩家国家
    #[test]
    fn p12_trigger_scoped_uses_effect_country() {
        let mut e = ev("france_surrender", 0, true, Trigger::AlwaysTrue);
        e.scope = crate::event::EventScope::Country;
        let db = EventDb { events: vec![e] };
        let mut sched = EventScheduler::new(db, 1);
        let mut world = test_world();
        let mut flags = GlobalFlags::default();
        // 玩家国家是 GER(CountryId(0))，法国是 FRA(CountryId(1))
        // trigger_scoped 使用 effect_country=FRA，而非 player=GER
        let (ok, _) = sched.trigger_scoped(
            "france_surrender",
            &mut world,
            CountryId(1), // effect_country = FRA
            CountryId(0), // display_country = GER (玩家)
            &mut flags,
        );
        assert!(ok);
        assert_eq!(sched.pending_len(), 1);
        let pending = sched.front().unwrap();
        // 效果作用国家是 FRA，而非玩家 GER
        assert_eq!(pending.country, CountryId(1));
    }

    /// P1.2 验收：新闻事件 cascade 使用 display_country
    #[test]
    fn p12_trigger_scoped_news_event_uses_display_country() {
        let mut e = ev("news.france_surrenders", 0, true, Trigger::AlwaysTrue);
        e.scope = crate::event::EventScope::News;
        let db = EventDb { events: vec![e] };
        let mut sched = EventScheduler::new(db, 1);
        let mut world = test_world();
        let mut flags = GlobalFlags::default();
        // 新闻事件：effect_country=FRA，display_country=GER(玩家)
        let (ok, _) = sched.trigger_scoped(
            "news.france_surrenders",
            &mut world,
            CountryId(1), // effect_country = FRA
            CountryId(0), // display_country = GER (玩家)
            &mut flags,
        );
        assert!(ok);
        assert_eq!(sched.pending_len(), 1);
        let pending = sched.front().unwrap();
        // 新闻事件显示给玩家(GER)，scope 是 News
        assert_eq!(pending.country, CountryId(0)); // display_country
        assert_eq!(pending.scope, crate::event::EventScope::News);
    }

    /// P1.2 验收：级联触发 FIFO 顺序 — A 触发 B、C，处理顺序为 B 再 C
    #[test]
    fn p12_cascade_fifo_order() {
        let e_b = ev("cascade_b", 0, true, Trigger::AlwaysTrue);
        let e_c = ev("cascade_c", 0, true, Trigger::AlwaysTrue);
        let db = EventDb {
            events: vec![e_b, e_c],
        };
        let mut sched = EventScheduler::new(db, 1);
        let mut world = test_world();
        let mut flags = GlobalFlags::default();

        // 模拟级联：先推 B 再推 C（FIFO 队列）
        flags
            .pending_triggers
            .push_back(crate::eval::PendingTrigger {
                event_id: "cascade_b".into(),
                effect_country: CountryId(0),
                display_country: CountryId(0),
                source: "test".into(),
            });
        flags
            .pending_triggers
            .push_back(crate::eval::PendingTrigger {
                event_id: "cascade_c".into(),
                effect_country: CountryId(0),
                display_country: CountryId(0),
                source: "test".into(),
            });

        // 逐个 pop_front 处理（FIFO）
        let first = flags.pending_triggers.pop_front().unwrap();
        let (ok1, _) = sched.trigger_scoped(
            &first.event_id,
            &mut world,
            first.effect_country,
            first.display_country,
            &mut flags,
        );
        assert!(ok1);
        let second = flags.pending_triggers.pop_front().unwrap();
        let (ok2, _) = sched.trigger_scoped(
            &second.event_id,
            &mut world,
            second.effect_country,
            second.display_country,
            &mut flags,
        );
        assert!(ok2);

        // B 先入队，C 后入队 → pending 顺序为 B, C
        assert_eq!(sched.pending[0].event_id, "cascade_b");
        assert_eq!(sched.pending[1].event_id, "cascade_c");
    }

    /// P1.2 验收：国家事件出现时 UI 应暂停（检查 pending 队列中任意国家事件）
    #[test]
    fn p12_country_event_should_pause() {
        let mut e = ev("country_event", 0, true, Trigger::AlwaysTrue);
        e.scope = crate::event::EventScope::Country;
        let db = EventDb { events: vec![e] };
        let mut sched = EventScheduler::new(db, 1);
        let mut world = test_world();
        let mut flags = GlobalFlags::default();
        sched.daily_tick(&mut world, CountryId(0), &mut flags);
        // pending 队列中有国家事件，应暂停
        let should_pause = sched
            .pending
            .iter()
            .any(|p| p.scope == crate::event::EventScope::Country);
        assert!(should_pause);
    }

    #[test]
    fn spa_scw_daily_events_are_paced() {
        let db = EventDb {
            events: vec![
                ev("spa.scw_a", 0, true, Trigger::Tag("SPA".into())),
                ev("spa.scw_b", 0, true, Trigger::Tag("SPA".into())),
            ],
        };
        let mut sched = EventScheduler::new(db, 1);
        let mut world = test_world();
        let spa = make_second_country_spa(&mut world);
        let mut flags = GlobalFlags::default();

        let (first_day, _) = sched.daily_tick(&mut world, spa, &mut flags);
        assert_eq!(first_day, vec!["spa.scw_a@SPA".to_owned()]);
        assert_eq!(sched.pending_len(), 1);

        world.date.day += 1;
        let (next_day, _) = sched.daily_tick(&mut world, spa, &mut flags);
        assert!(next_day.is_empty());

        world.date.day += 13;
        let (after_cooldown, _) = sched.daily_tick(&mut world, spa, &mut flags);
        assert_eq!(after_cooldown, vec!["spa.scw_b@SPA".to_owned()]);
    }

    #[test]
    fn spa_scw_cascaded_player_events_are_deferred() {
        let mut first = ev("spa.scw_first", 0, true, Trigger::AlwaysTrue);
        first.is_triggered_only = true;
        let mut second = ev("spa.scw_second", 0, true, Trigger::AlwaysTrue);
        second.is_triggered_only = true;
        let db = EventDb {
            events: vec![first, second],
        };
        let mut sched = EventScheduler::new(db, 1);
        let mut world = test_world();
        let spa = make_second_country_spa(&mut world);
        let mut flags = GlobalFlags::default();

        let (ok_first, _) =
            sched.trigger_scoped_for_player("spa.scw_first", &mut world, spa, spa, spa, &mut flags);
        assert!(ok_first);
        assert_eq!(sched.pending_len(), 1);
        sched.pending.pop_front();

        let (ok_second, _) = sched.trigger_scoped_for_player(
            "spa.scw_second",
            &mut world,
            spa,
            spa,
            spa,
            &mut flags,
        );
        assert!(ok_second);
        assert_eq!(sched.pending_len(), 0);

        world.date.day += 14;
        let (fired, _) = sched.daily_tick(&mut world, spa, &mut flags);
        assert_eq!(fired, vec!["spa.scw_second@SPA".to_owned()]);
        assert_eq!(sched.pending_len(), 1);
    }
}
