//! V9 compact status pill primitive.

use egui::{Align2, Color32, Pos2, Rect, Sense, Stroke, Ui, Vec2};

use crate::v9::{
    accessibility::{self, SemanticTone},
    sound,
    tokens::{palette, radius, spacing, TextRole},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PillTone {
    Neutral,
    Good,
    Warn,
    Bad,
    Info,
    Brass,
}

impl PillTone {
    pub fn color(self) -> Color32 {
        match self {
            Self::Neutral => palette::PARCHMENT_DIM,
            Self::Good => palette::GOOD,
            Self::Warn => palette::WARN,
            Self::Bad => palette::BAD,
            Self::Info => palette::INFO,
            Self::Brass => palette::GOLD,
        }
    }

    fn semantic_tone(self) -> SemanticTone {
        match self {
            Self::Neutral | Self::Brass => SemanticTone::Neutral,
            Self::Good => SemanticTone::Good,
            Self::Warn => SemanticTone::Warn,
            Self::Bad => SemanticTone::Bad,
            Self::Info => SemanticTone::Info,
        }
    }
}

pub struct Pill<'a> {
    label: &'a str,
    tone: PillTone,
}

impl<'a> Pill<'a> {
    pub fn new(label: &'a str) -> Self {
        Self {
            label,
            tone: PillTone::Neutral,
        }
    }

    pub fn tone(mut self, tone: PillTone) -> Self {
        self.tone = tone;
        self
    }

    pub fn show(self, ui: &mut Ui) -> egui::Response {
        let width =
            (self.label.chars().count() as f32 * 7.0 + spacing::S6 * 2.0).clamp(48.0, 190.0);
        let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 22.0), Sense::hover());
        sound::hook_response_auto(&format!("pill:{}", self.label), &response, true);
        self.draw_at(ui, rect);
        response
    }

    pub fn show_at(self, ui: &mut Ui, rect: Rect) {
        let response = ui.interact(rect, ui.id().with(("pill", self.label)), Sense::hover());
        sound::hook_response_auto(&format!("pill:{}", self.label), &response, true);
        self.draw_at(ui, rect);
    }

    fn draw_at(self, ui: &mut Ui, rect: Rect) {
        let style = accessibility::style_for(ui.ctx(), self.tone.semantic_tone());
        let color = if self.tone == PillTone::Brass {
            self.tone.color()
        } else {
            style.color
        };
        let fill =
            Color32::from_rgba_premultiplied(color.r() / 6, color.g() / 6, color.b() / 6, 210);
        ui.painter().rect_filled(
            rect,
            egui::epaint::CornerRadius::same(radius::R1 as u8),
            fill,
        );
        ui.painter().rect_stroke(
            rect,
            egui::epaint::CornerRadius::same(radius::R1 as u8),
            Stroke::new(1.0, color),
            egui::StrokeKind::Inside,
        );
        ui.painter().text(
            Pos2::new(rect.center().x, rect.center().y),
            Align2::CENTER_CENTER,
            self.label,
            TextRole::Caption.font_id(),
            color,
        );
        accessibility::paint_marker(ui, rect.shrink2(Vec2::new(4.0, 4.0)), style.marker, color);
    }
}
