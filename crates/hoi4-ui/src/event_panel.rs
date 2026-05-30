//! Event modal bridge.
//!
//! The scheduler and command contract stay in this legacy module so callers do
//! not need to change. The visual shell is rendered by the V9 event composite.

use hoi4_content::{Event as ContentEvent, EventScheduler};

#[derive(Debug, Clone, PartialEq)]
pub enum EventCommand {
    PickOption { event_id: String, option_idx: usize },
}

pub fn show_event_modal(
    ctx: &egui::Context,
    scheduler: &EventScheduler,
    mut option_trigger_satisfied: impl FnMut(&str, usize) -> bool,
) -> Option<EventCommand> {
    show_event_modal_inner(ctx, scheduler, None, |id, idx| {
        option_trigger_satisfied(id, idx)
    })
}

pub fn show_event_modal_with_icons(
    ctx: &egui::Context,
    scheduler: &EventScheduler,
    icon_bank: &mut crate::icons::IconBank,
    mut option_trigger_satisfied: impl FnMut(&str, usize) -> bool,
) -> Option<EventCommand> {
    show_event_modal_inner(ctx, scheduler, Some(icon_bank), |id, idx| {
        option_trigger_satisfied(id, idx)
    })
}

fn show_event_modal_inner(
    ctx: &egui::Context,
    scheduler: &EventScheduler,
    icon_bank: Option<&mut crate::icons::IconBank>,
    mut option_trigger_satisfied: impl FnMut(&str, usize) -> bool,
) -> Option<EventCommand> {
    let pending = scheduler.front()?;
    let event = scheduler.db.find(&pending.event_id)?;
    let queue_extra = scheduler.pending_len().saturating_sub(1);

    crate::v9::composites::modal_event::show_event_modal(
        ctx,
        event,
        queue_extra,
        icon_bank,
        |idx| option_trigger_satisfied(&event.id, idx),
    )
    .map(|option_idx| EventCommand::PickOption {
        event_id: event.id.clone(),
        option_idx,
    })
}

#[doc(hidden)]
pub fn _option_count(event: &ContentEvent) -> usize {
    event.options.len()
}

#[cfg(test)]
mod tests {
    use hoi4_content::{Effect, Event, EventDb, EventOption, EventScheduler, Trigger};

    fn sample_db() -> EventDb {
        EventDb {
            events: vec![Event {
                id: "test.modal".into(),
                title: "Test".into(),
                description: "desc".into(),
                picture: String::new(),
                scope: hoi4_content::EventScope::Country,
                is_triggered_only: true,
                fire_only_once: true,
                hidden: false,
                trigger: Trigger::AlwaysTrue,
                mean_time_to_happen_days: 1,
                immediate: vec![],
                options: vec![
                    EventOption {
                        name: "A".into(),
                        trigger: Trigger::AlwaysTrue,
                        effects: vec![Effect::AddPoliticalPower(5.0)],
                        ai_chance: 1.0,
                    },
                    EventOption {
                        name: "B".into(),
                        trigger: Trigger::AlwaysFalse,
                        effects: vec![],
                        ai_chance: 0.5,
                    },
                ],
            }],
        }
    }

    #[test]
    fn front_when_pending_empty() {
        let s = EventScheduler::new(sample_db(), 1);
        assert!(s.front().is_none());
    }

    #[test]
    fn option_count_helper() {
        let db = sample_db();
        assert_eq!(super::_option_count(&db.events[0]), 2);
    }
}
