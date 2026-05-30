//! Focus daily tick: advance progress, fire completion effects.

use crate::eval::{run_effects, GlobalFlags};
use crate::focus::FocusTree;
use hoi4_state::{CountryId, World};

/// Result of a daily tick for one country's focus.
#[derive(Debug, Clone, PartialEq)]
pub enum TickResult {
    /// No focus active.
    Idle,
    /// Focus still in progress.
    InProgress {
        id: String,
        progress: f32,
        cost: u32,
    },
    /// Focus just completed this tick.
    Completed(String),
}

/// Advance the active focus for `country` by one day.
/// If completed, runs completion_effect and marks focus done.
/// P0.3：focus_speed_factor 来自 PoliticsCache，确保 modifier 生效。
pub fn daily_focus_tick(
    world: &mut World,
    country: CountryId,
    tree: &FocusTree,
    flags: &mut GlobalFlags,
    focus_speed_factor: f32,
) -> TickResult {
    let i = country.0 as usize;

    let focus_id = match &world.countries.current_focus[i] {
        Some(id) => id.clone(),
        None => return TickResult::Idle,
    };

    // P0.3：进度 = BASE(1.0) * (1 + focus_speed_factor)
    let speed = 1.0 * (1.0 + focus_speed_factor);
    world.countries.focus_progress[i] += speed;

    // Find the focus definition
    let Some(focus) = tree.focuses.iter().find(|f| f.id == focus_id) else {
        // Focus not found in tree, clear it
        world.countries.current_focus[i] = None;
        world.countries.focus_progress[i] = 0.0;
        return TickResult::Idle;
    };

    let cost = focus.cost_days;
    let progress = world.countries.focus_progress[i];

    if progress >= cost as f32 {
        // Complete: run effects, mark done, clear current
        // P1.3：收集效果报告（目前只记录错误到日志）
        let report = run_effects(&focus.completion_effect, world, country, flags);
        if report.has_errors() || report.has_warnings() {
            for w in &report.warnings {
                println!("[focus] warning: {w}");
            }
            for e in &report.errors {
                println!("[focus] error: {e}");
            }
        }
        world.countries.completed_focuses[i].insert(focus_id.clone());
        world.countries.current_focus[i] = None;
        world.countries.focus_progress[i] = 0.0;
        TickResult::Completed(focus_id)
    } else {
        TickResult::InProgress {
            id: focus_id,
            progress,
            cost,
        }
    }
}

/// Start a focus for a country (sets current_focus + resets progress).
/// Returns false if focus cannot be started (already active, prereqs not met, available false, etc).
/// P0.3：新增 available 条件检查。
pub fn start_focus(
    world: &mut World,
    country: CountryId,
    tree: &FocusTree,
    focus_id: &str,
    flags: &GlobalFlags,
) -> bool {
    let i = country.0 as usize;

    // Can't start if already doing one
    if world.countries.current_focus[i].is_some() {
        return false;
    }

    // Find focus
    let Some(focus) = tree.focuses.iter().find(|f| f.id == focus_id) else {
        return false;
    };

    // Check already completed
    if world.countries.completed_focuses[i].contains(focus_id) {
        return false;
    }

    // Check prerequisites
    for group in &focus.prerequisites {
        if !group
            .iter()
            .any(|p| world.countries.completed_focuses[i].contains(p))
        {
            return false;
        }
    }

    // Check mutual exclusive not already taken
    for me in &focus.mutually_exclusive {
        if world.countries.completed_focuses[i].contains(me) {
            return false;
        }
    }

    // P0.3：检查 available 条件
    if !crate::eval::eval_trigger(&focus.available, world, country, flags) {
        return false;
    }

    world.countries.current_focus[i] = Some(focus_id.to_owned());
    world.countries.focus_progress[i] = 0.0;
    true
}
