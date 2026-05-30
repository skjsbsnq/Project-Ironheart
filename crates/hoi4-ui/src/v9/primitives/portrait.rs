//! Portrait frame primitive.

use egui::{Align2, Color32, Pos2, Rect, Stroke, StrokeKind, TextureHandle, Ui, Vec2};

use crate::v9::{
    paint, sound,
    tokens::{palette, spacing, TextRole},
};

pub struct PortraitFrame<'a> {
    label: &'a str,
    accent: Color32,
}

impl<'a> PortraitFrame<'a> {
    pub fn new(label: &'a str) -> Self {
        Self {
            label,
            accent: palette::BRASS_BRIGHT,
        }
    }

    pub fn accent(mut self, accent: Color32) -> Self {
        self.accent = accent;
        self
    }

    pub fn show_at(self, ui: &mut Ui, rect: Rect, texture: Option<&TextureHandle>) {
        paint::paint_bevel(
            ui.painter(),
            rect,
            palette::SOOT_BLACK,
            palette::EDGE_DARK,
            2.0,
        );
        let image_rect = rect.shrink2(Vec2::new(spacing::S3, spacing::S3));
        ui.painter().rect_filled(
            image_rect,
            egui::epaint::CornerRadius::same(1),
            palette::IRON_DARK,
        );
        if let Some(handle) = texture {
            ui.put(
                image_rect,
                egui::Image::from_texture(handle).fit_to_exact_size(image_rect.size()),
            );
        } else {
            paint::paint_plate_grain(ui.painter(), image_rect.shrink(2.0), 3.0, 3);
            ui.painter().text(
                image_rect.center(),
                Align2::CENTER_CENTER,
                fallback_label(self.label),
                TextRole::Heading.font_id(),
                self.accent,
            );
        }
        ui.painter().rect_stroke(
            image_rect,
            egui::epaint::CornerRadius::same(1),
            Stroke::new(1.0, self.accent),
            StrokeKind::Inside,
        );
        paint::paint_slot_screw(
            ui.painter(),
            Pos2::new(rect.left() + 8.0, rect.top() + 8.0),
            2.0,
        );
        paint::paint_slot_screw(
            ui.painter(),
            Pos2::new(rect.right() - 8.0, rect.bottom() - 8.0),
            2.0,
        );
        let response = ui.interact(
            rect,
            ui.id().with(("portrait_frame", self.label)),
            egui::Sense::hover(),
        );
        sound::hook_response_auto(&format!("portrait:{}", self.label), &response, true);
    }
}

fn fallback_label(label: &str) -> &str {
    if label.trim().is_empty() {
        "?"
    } else {
        label
    }
}
