//! V9 toast primitive.

use egui::{Align2, Color32, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2};

use crate::v9::{
    accessibility::{self, SemanticTone},
    frame::{FrameStyle, PanelFrame},
    paint, profiler, sound,
    tokens::{palette, spacing, Elevation, TextRole},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Info,
    Good,
    Warn,
    Bad,
}

impl ToastKind {
    pub fn accent(self) -> Color32 {
        match self {
            Self::Info => palette::INFO,
            Self::Good => palette::GOOD,
            Self::Warn => palette::WARN,
            Self::Bad => palette::BAD,
        }
    }

    fn semantic_tone(self) -> SemanticTone {
        match self {
            Self::Info => SemanticTone::Info,
            Self::Good => SemanticTone::Good,
            Self::Warn => SemanticTone::Warn,
            Self::Bad => SemanticTone::Bad,
        }
    }
}

pub struct ToastView<'a> {
    pub title: &'a str,
    pub body: &'a str,
    pub kind: ToastKind,
    pub progress: f32,
}

impl<'a> ToastView<'a> {
    pub fn new(title: &'a str, body: &'a str, kind: ToastKind) -> Self {
        Self {
            title,
            body,
            kind,
            progress: 1.0,
        }
    }

    pub fn progress(mut self, progress: f32) -> Self {
        self.progress = progress.clamp(0.0, 1.0);
        self
    }

    pub fn show_at(&self, ui: &mut Ui, rect: Rect, id_source: impl std::hash::Hash) -> Response {
        let response = ui.interact(rect, ui.id().with(id_source), Sense::hover());
        sound::hook_response_auto("toast", &response, true);
        let ctx = ui.ctx().clone();
        profiler::measure_ctx(&ctx, "toast", || self.draw(ui, rect, response.hovered()));
        response
    }

    pub fn draw(&self, ui: &mut Ui, rect: Rect, hovered: bool) {
        let accessible = accessibility::style_for(ui.ctx(), self.kind.semantic_tone());
        let accent = accessible.color;
        PanelFrame::new(FrameStyle::Card, rect)
            .with_accent(accent)
            .with_elevation(if hovered {
                Elevation::E2
            } else {
                Elevation::E1
            })
            .draw(ui.painter());

        let inner = rect.shrink2(Vec2::new(spacing::S4, spacing::S3));
        let bar = Rect::from_min_max(
            Pos2::new(inner.left(), inner.top() + spacing::S1),
            Pos2::new(inner.left() + 4.0, inner.bottom() - spacing::S1),
        );
        ui.painter().rect_filled(bar, 0.0, accent);
        accessibility::paint_marker(ui, bar.expand(5.0), accessible.marker, palette::PARCHMENT);

        let text_x = bar.right() + spacing::S4;
        ui.painter().text(
            Pos2::new(text_x, inner.top() + spacing::S1),
            Align2::LEFT_TOP,
            self.title,
            TextRole::Subheading.font_id(),
            accent,
        );

        let body_galley = ui.painter().layout(
            self.body.to_owned(),
            TextRole::Body.font_id(),
            palette::PARCHMENT,
            (inner.right() - text_x).max(80.0),
        );
        ui.painter().galley(
            Pos2::new(text_x, inner.top() + 23.0),
            body_galley,
            palette::PARCHMENT,
        );

        let track = Rect::from_min_max(
            Pos2::new(text_x, rect.bottom() - 5.0),
            Pos2::new(rect.right() - spacing::S4, rect.bottom() - 3.0),
        );
        ui.painter()
            .rect_filled(track, 0.0, Color32::from_black_alpha(160));
        let fill = Rect::from_min_max(
            track.min,
            Pos2::new(track.left() + track.width() * self.progress, track.bottom()),
        );
        ui.painter().rect_filled(fill, 0.0, accent);

        if hovered {
            paint::paint_top_highlight(ui.painter(), rect.shrink(2.0), accent);
            ui.painter().rect_stroke(
                rect.shrink(1.0),
                egui::epaint::CornerRadius::same(1),
                Stroke::new(1.0, accent),
                StrokeKind::Inside,
            );
        }
    }
}
