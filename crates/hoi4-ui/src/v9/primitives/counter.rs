//! V9 NATO counter icon primitive used by military panels.

use egui::{Align2, Color32, Pos2, Rect, Response, Sense, Stroke, Ui, Vec2};

use crate::v9::{
    nato_icon::{self, NatoArchetype},
    sound,
    tokens::{palette, radius, spacing, TextRole},
};

pub struct CounterIcon<'a> {
    label: &'a str,
    archetype: NatoArchetype,
    accent: Color32,
    selected: bool,
}

impl<'a> CounterIcon<'a> {
    pub fn new(label: &'a str, archetype: NatoArchetype) -> Self {
        Self {
            label,
            archetype,
            accent: palette::BRASS_BRIGHT,
            selected: false,
        }
    }

    pub fn accent(mut self, accent: Color32) -> Self {
        self.accent = accent;
        self
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn show_at(self, ui: &mut Ui, rect: Rect) -> Response {
        let response = ui.interact(
            rect,
            ui.id().with((
                "counter_icon",
                self.label,
                rect.min.x.to_bits(),
                rect.min.y.to_bits(),
            )),
            Sense::click(),
        );
        sound::hook_response_auto(&format!("counter:{}", self.label), &response, true);
        let border = if self.selected || response.hovered() {
            palette::GOLD_HOT
        } else {
            self.accent
        };
        let fill = if self.selected {
            palette::IRON
        } else {
            palette::SOOT_BLACK
        };

        ui.painter().rect_filled(
            rect,
            egui::epaint::CornerRadius::same(radius::R1 as u8),
            fill,
        );
        ui.painter().rect_stroke(
            rect,
            egui::epaint::CornerRadius::same(radius::R1 as u8),
            Stroke::new(1.0, border),
            egui::StrokeKind::Inside,
        );
        crate::v9::paint::paint_plate_grain(ui.painter(), rect.shrink(2.0), 3.0, 2);

        let symbol = Rect::from_min_size(
            rect.left_top() + Vec2::new(spacing::S2, spacing::S2),
            Vec2::new(
                (rect.height() - spacing::S4) * 1.38,
                rect.height() - spacing::S4,
            ),
        );
        nato_icon::draw_nato_symbol(ui.painter(), symbol, self.archetype, self.accent);

        let text_x = symbol.right() + spacing::S3;
        let clip = Rect::from_min_max(Pos2::new(text_x, rect.top()), rect.right_bottom());
        let painter = ui.painter().with_clip_rect(clip);
        painter.text(
            Pos2::new(text_x, rect.center().y),
            Align2::LEFT_CENTER,
            self.label,
            TextRole::Caption.font_id(),
            palette::PARCHMENT,
        );
        response
    }
}
