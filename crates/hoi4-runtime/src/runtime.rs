use std::time::Instant;

use hoi4_logic::economy::EconomyState;
use hoi4_logic::feedback::FeedbackBus;
use hoi4_logic::politics::PoliticsCache;
use hoi4_logic::research::ResearchState;
use hoi4_state::{GameSpeed, World};

use crate::ai_runtime::AiState;
use crate::content_runtime::ContentRuntimeState;
use crate::schedule::SystemSchedule;
use crate::script_runtime::ScriptState;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeEvent {
    DayChanged(hoi4_state::GameDate),
    MonthChanged(hoi4_state::GameDate),
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RuntimeEvents {
    pub events: Vec<RuntimeEvent>,
}

impl RuntimeEvents {
    fn push_calendar_events(&mut self, before: hoi4_state::GameDate, after: hoi4_state::GameDate) {
        if before.day != after.day || before.month != after.month || before.year != after.year {
            self.events.push(RuntimeEvent::DayChanged(after));
        }
        if before.month != after.month || before.year != after.year {
            self.events.push(RuntimeEvent::MonthChanged(after));
        }
    }

    pub fn extend(&mut self, other: RuntimeEvents) {
        self.events.extend(other.events);
    }
}

pub struct HourlyRuntime<'a> {
    pub world: &'a mut World,
    pub econ: &'a mut EconomyState,
    pub research: &'a mut ResearchState,
    pub politics_cache: &'a mut PoliticsCache,
    pub script: &'a mut ScriptState,
    pub ai: &'a mut AiState,
    pub v6_db: &'a hoi4_content::V6Database,
    pub schedule: &'a mut SystemSchedule,
    pub feedback_bus: &'a mut FeedbackBus,
    pub content: &'a mut ContentRuntimeState,
}

pub fn tick_one_hour(runtime: &mut HourlyRuntime<'_>) -> RuntimeEvents {
    let before = runtime.world.date;
    runtime.schedule.tick_hour(
        runtime.world,
        runtime.econ,
        runtime.research,
        runtime.politics_cache,
        runtime.script,
        runtime.ai,
        runtime.v6_db,
        runtime.content,
    );
    runtime.feedback_bus.drain(runtime.world, runtime.econ);
    let mut events = RuntimeEvents::default();
    events.push_calendar_events(before, runtime.world.date);
    events
}

pub fn tick_one_hour_interactive(runtime: &mut HourlyRuntime<'_>) -> RuntimeEvents {
    let before = runtime.world.date;
    runtime.schedule.tick_hour_interactive(
        runtime.world,
        runtime.econ,
        runtime.research,
        runtime.politics_cache,
        runtime.script,
        runtime.ai,
        runtime.v6_db,
        runtime.content,
    );
    runtime.feedback_bus.drain(runtime.world, runtime.econ);
    let mut events = RuntimeEvents::default();
    events.push_calendar_events(before, runtime.world.date);
    events
}

pub fn run_pending_interactive(runtime: &mut HourlyRuntime<'_>, budget_secs: f32) -> bool {
    let ran = runtime.schedule.run_pending_interactive(
        runtime.world,
        runtime.econ,
        runtime.research,
        runtime.politics_cache,
        runtime.script,
        runtime.ai,
        runtime.v6_db,
        runtime.content,
        budget_secs,
    );
    if ran {
        runtime.feedback_bus.drain(runtime.world, runtime.econ);
    }
    ran
}

pub fn tick_hours(runtime: &mut HourlyRuntime<'_>, n: u32) -> RuntimeEvents {
    let mut out = RuntimeEvents::default();
    for _ in 0..n {
        out.extend(tick_one_hour(runtime));
    }
    out
}

pub fn tick_until_day_change(runtime: &mut HourlyRuntime<'_>) -> RuntimeEvents {
    let start_day = runtime.world.date.days_since_epoch();
    let mut out = RuntimeEvents::default();
    while runtime.world.date.days_since_epoch() == start_day {
        out.extend(tick_one_hour(runtime));
    }
    out
}

pub fn speed_tick_limits(speed: GameSpeed) -> (u32, f32) {
    let max_ticks = match speed {
        GameSpeed::Speed1 => 2,
        GameSpeed::Speed2 => 4,
        GameSpeed::Speed3 => 8,
        GameSpeed::Speed4 => 12,
        GameSpeed::Speed5 => 24,
        GameSpeed::Paused => 0,
    };
    let tick_budget = match speed {
        GameSpeed::Speed1 | GameSpeed::Speed2 => 0.004,
        GameSpeed::Speed3 => 0.006,
        GameSpeed::Speed4 => 0.008,
        GameSpeed::Speed5 => 0.010,
        GameSpeed::Paused => 0.0,
    };
    (max_ticks, tick_budget)
}

pub fn should_stop_hourly_catchup(started_at: Instant, budget_secs: f32) -> bool {
    budget_secs > 0.0 && started_at.elapsed().as_secs_f32() >= budget_secs
}
