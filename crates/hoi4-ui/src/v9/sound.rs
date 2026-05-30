//! V9 UI sound event hook.

use egui::{Context, Id, Response};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum V9SoundEvent {
    Hover,
    Click,
    Error,
    Page,
    Modal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueuedSoundEvent {
    pub source: String,
    pub event: V9SoundEvent,
}

#[derive(Debug, Default, Clone)]
struct SoundQueue {
    events: Vec<QueuedSoundEvent>,
}

#[derive(Debug, Clone, Copy, Default)]
struct LastResponseState {
    hovered: bool,
    clicked: bool,
}

pub fn emit(ctx: &Context, source: impl Into<String>, event: V9SoundEvent) {
    ctx.data_mut(|data| {
        data.get_temp_mut_or_default::<SoundQueue>(Id::new("v9_sound_queue"))
            .events
            .push(QueuedSoundEvent {
                source: source.into(),
                event,
            });
    });
}

pub fn emit_error(ctx: &Context, source: impl Into<String>) {
    emit(ctx, source, V9SoundEvent::Error);
}

pub fn emit_once(ctx: &Context, id: Id, source: impl Into<String>, event: V9SoundEvent) {
    let already_emitted = ctx.data_mut(|data| data.get_temp::<bool>(id).unwrap_or(false));
    if already_emitted {
        return;
    }
    emit(ctx, source, event);
    ctx.data_mut(|data| data.insert_temp(id, true));
}

pub fn hook_response_auto(source: &str, response: &Response, enabled: bool) {
    hook_response(
        &response.ctx,
        response.id.with("v9_sound"),
        source,
        response,
        enabled,
    );
}

pub fn hook_response(ctx: &Context, id: Id, source: &str, response: &Response, enabled: bool) {
    let previous = ctx.data_mut(|data| {
        data.get_temp_mut_or::<LastResponseState>(id, LastResponseState::default())
            .to_owned()
    });
    let current = LastResponseState {
        hovered: response.hovered() && enabled,
        clicked: response.clicked() && enabled,
    };

    if current.hovered && !previous.hovered {
        emit(ctx, source.to_owned(), V9SoundEvent::Hover);
    }
    if current.clicked && !previous.clicked {
        emit(ctx, source.to_owned(), V9SoundEvent::Click);
    }

    if response.hovered() && !enabled {
        let disabled_error_id = id.with("disabled_error");
        let was_down = ctx.data_mut(|data| {
            data.get_temp_mut_or::<bool>(disabled_error_id, false)
                .to_owned()
        });
        let is_down = response.is_pointer_button_down_on();
        if is_down && !was_down {
            emit(ctx, source.to_owned(), V9SoundEvent::Error);
        }
        ctx.data_mut(|data| data.insert_temp(disabled_error_id, is_down));
    }

    ctx.data_mut(|data| data.insert_temp(id, current));
}

pub fn drain(ctx: &Context) -> Vec<QueuedSoundEvent> {
    ctx.data_mut(|data| {
        let queue = data.get_temp_mut_or_default::<SoundQueue>(Id::new("v9_sound_queue"));
        std::mem::take(&mut queue.events)
    })
}

pub fn pending_count(ctx: &Context) -> usize {
    ctx.data_mut(|data| {
        data.get_temp_mut_or_default::<SoundQueue>(Id::new("v9_sound_queue"))
            .events
            .len()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queue_round_trip() {
        let ctx = Context::default();
        emit(&ctx, "button", V9SoundEvent::Click);
        emit_error(&ctx, "form");
        let events = drain(&ctx);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event, V9SoundEvent::Click);
        assert_eq!(events[1].event, V9SoundEvent::Error);
        assert_eq!(pending_count(&ctx), 0);
    }

    #[test]
    fn emit_once_deduplicates_by_id() {
        let ctx = Context::default();
        let id = Id::new("modal");
        emit_once(&ctx, id, "modal", V9SoundEvent::Modal);
        emit_once(&ctx, id, "modal", V9SoundEvent::Modal);
        assert_eq!(drain(&ctx).len(), 1);
    }
}
