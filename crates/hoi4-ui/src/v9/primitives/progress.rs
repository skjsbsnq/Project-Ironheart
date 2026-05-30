//! Progress primitives used by Phase D panels.

use egui::{Align2, Color32, Pos2, Rect, Shape, Stroke, Ui, Vec2};

use crate::v9::{
    sound,
    tokens::{palette, TextRole},
};

pub struct ProgressRing<'a> {
    value: f32,
    label: &'a str,
    accent: Color32,
}

impl<'a> ProgressRing<'a> {
    pub fn new(value: f32, label: &'a str) -> Self {
        Self {
            value,
            label,
            accent: palette::BRASS_BRIGHT,
        }
    }

    pub fn accent(mut self, accent: Color32) -> Self {
        self.accent = accent;
        self
    }

    pub fn show_at(self, ui: &mut Ui, rect: Rect) {
        let center = rect.center();
        let radius = rect.width().min(rect.height()) * 0.34;
        ui.painter()
            .circle_filled(center, radius + 5.0, Color32::from_black_alpha(150));
        ui.painter()
            .circle_stroke(center, radius, Stroke::new(4.0, palette::EDGE_DARK));
        draw_arc(
            ui.painter(),
            center,
            radius,
            -std::f32::consts::FRAC_PI_2,
            self.value.clamp(0.0, 1.0) * std::f32::consts::TAU,
            Stroke::new(4.0, self.accent),
        );
        ui.painter().text(
            center,
            Align2::CENTER_CENTER,
            format!("{:.0}%", self.value.clamp(0.0, 1.0) * 100.0),
            TextRole::Numeric.font_id(),
            palette::PARCHMENT,
        );
        ui.painter().text(
            Pos2::new(center.x, rect.bottom() - 2.0),
            Align2::CENTER_BOTTOM,
            self.label,
            TextRole::Caption.font_id(),
            palette::PARCHMENT_DIM,
        );
        let response = ui.interact(
            rect,
            ui.id().with(("progress_ring", self.label)),
            egui::Sense::hover(),
        );
        sound::hook_response_auto(&format!("progress:{}", self.label), &response, true);
    }
}

pub fn draw_progress_bar(ui: &mut Ui, rect: Rect, value: f32, accent: Color32) {
    ui.painter().rect_filled(
        rect,
        egui::epaint::CornerRadius::same(1),
        palette::SOOT_BLACK,
    );
    let fill = Rect::from_min_size(
        rect.min,
        Vec2::new(rect.width() * value.clamp(0.0, 1.0), rect.height()),
    );
    ui.painter()
        .rect_filled(fill, egui::epaint::CornerRadius::same(1), accent);
    ui.painter().rect_stroke(
        rect,
        egui::epaint::CornerRadius::same(1),
        Stroke::new(1.0, palette::EDGE_DARK),
        egui::StrokeKind::Inside,
    );
}

fn draw_arc(
    painter: &egui::Painter,
    center: Pos2,
    radius: f32,
    start: f32,
    sweep: f32,
    stroke: Stroke,
) {
    if sweep <= 0.001 {
        return;
    }
    let steps = ((sweep.abs() / std::f32::consts::TAU) * 72.0)
        .ceil()
        .max(4.0) as usize;
    let mut points = Vec::with_capacity(steps + 1);
    for i in 0..=steps {
        let t = start + sweep * (i as f32 / steps as f32);
        points.push(Pos2::new(
            center.x + t.cos() * radius,
            center.y + t.sin() * radius,
        ));
    }
    painter.add(Shape::line(points, stroke));
}
