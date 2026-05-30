//! V9 notification stack built from toast primitives.

use std::time::Duration;

use egui::{Area, Context, Id, Order, Pos2, Rect, Vec2};

use crate::v9::{
    layout::AnchorLayout,
    motion,
    primitives::{ToastKind, ToastView},
    sound,
    tokens::spacing,
};

const TOAST_W: f32 = 320.0;
const TOAST_H: f32 = 58.0;
const TOAST_GAP: f32 = 8.0;
const DEFAULT_TTL: f64 = 4.0;

#[derive(Debug, Clone)]
struct Notification {
    id: u64,
    key: String,
    title: String,
    body: String,
    kind: ToastKind,
    created_at: Option<f64>,
    paused_at: Option<f64>,
    paused_total: f64,
    ttl: f64,
}

#[derive(Debug, Default)]
pub struct NotificationStack {
    next_id: u64,
    items: Vec<Notification>,
}

impl NotificationStack {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, title: impl Into<String>, body: impl Into<String>, kind: ToastKind) {
        self.push_keyed(
            format!("toast_{}", self.next_id),
            title,
            body,
            kind,
            DEFAULT_TTL,
        );
    }

    pub fn push_keyed(
        &mut self,
        key: impl Into<String>,
        title: impl Into<String>,
        body: impl Into<String>,
        kind: ToastKind,
        ttl: f64,
    ) {
        let key = key.into();
        if let Some(existing) = self.items.iter_mut().find(|item| item.key == key) {
            existing.title = title.into();
            existing.body = body.into();
            existing.kind = kind;
            existing.created_at = None;
            existing.paused_at = None;
            existing.paused_total = 0.0;
            existing.ttl = ttl.max(0.5);
            return;
        }

        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        self.items.push(Notification {
            id,
            key,
            title: title.into(),
            body: body.into(),
            kind,
            created_at: None,
            paused_at: None,
            paused_total: 0.0,
            ttl: ttl.max(0.5),
        });
    }

    pub fn show(&mut self, ctx: &Context) {
        let now = ctx.input(|i| i.time);
        self.items.retain_mut(|item| {
            let created_at = *item.created_at.get_or_insert(now);
            let age = now - created_at - item.paused_total;
            age < item.ttl || item.paused_at.is_some()
        });
        if self.items.is_empty() {
            return;
        }

        let screen = ctx.screen_rect();
        let count = self.items.len().min(5);
        let total_h = count as f32 * TOAST_H + count.saturating_sub(1) as f32 * TOAST_GAP;
        let stack_rect = AnchorLayout::BottomRight.place(
            screen,
            Vec2::new(TOAST_W, total_h),
            Vec2::new(spacing::S5, 220.0),
        );

        Area::new(Id::new("v9_notification_stack"))
            .order(Order::Foreground)
            .fixed_pos(stack_rect.min)
            .default_size(stack_rect.size())
            .show(ctx, |ui| {
                let (canvas, _) = ui.allocate_exact_size(stack_rect.size(), egui::Sense::hover());
                for (idx, item) in self.items.iter_mut().rev().take(count).enumerate() {
                    let y = idx as f32 * (TOAST_H + TOAST_GAP);
                    let rect = Rect::from_min_size(
                        Pos2::new(canvas.left(), canvas.top() + y),
                        Vec2::new(TOAST_W, TOAST_H),
                    );
                    let toast_id = Id::new(("v9_toast", item.id));
                    let rect = motion::toast_slide_rect(ui.ctx(), toast_id, rect);
                    sound::emit_once(
                        ui.ctx(),
                        toast_id.with("sound"),
                        "toast",
                        sound::V9SoundEvent::Page,
                    );
                    let created_at = item.created_at.unwrap_or(now);
                    let age = now - created_at - item.paused_total;
                    let progress = 1.0 - (age / item.ttl) as f32;
                    let response = ToastView::new(&item.title, &item.body, item.kind)
                        .progress(progress)
                        .show_at(ui, rect, item.id);

                    if response.hovered() {
                        if item.paused_at.is_none() {
                            item.paused_at = Some(now);
                        }
                    } else if let Some(paused_at) = item.paused_at.take() {
                        item.paused_total += now - paused_at;
                    }
                }
            });

        ctx.request_repaint_after(Duration::from_millis(100));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyed_push_updates_existing_item() {
        let mut stack = NotificationStack::new();
        stack.push_keyed("law", "A", "one", ToastKind::Bad, 4.0);
        stack.push_keyed("law", "B", "two", ToastKind::Warn, 4.0);
        assert_eq!(stack.items.len(), 1);
        assert_eq!(stack.items[0].title, "B");
        assert_eq!(stack.items[0].body, "two");
    }
}
