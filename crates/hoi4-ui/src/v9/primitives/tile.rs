//! KPI tile primitive: label + value + left accent strip.

use egui::{Align2, Color32, Painter, Pos2, Rect, Stroke, StrokeKind, Ui, Vec2};

use crate::v9::{
    accessibility, profiler, sound,
    tokens::{palette, radius, spacing, TextRole},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TileTrend {
    Up,
    Down,
    Flat,
    None,
}

pub struct Tile<'a> {
    label: &'a str,
    value: &'a str,
    accent: Color32,
    trend: TileTrend,
}

impl<'a> Tile<'a> {
    pub fn new(label: &'a str, value: &'a str) -> Self {
        Self {
            label,
            value,
            accent: palette::STAT_PRIMARY,
            trend: TileTrend::None,
        }
    }

    pub fn accent(mut self, color: Color32) -> Self {
        self.accent = color;
        self
    }

    pub fn trend(mut self, trend: TileTrend) -> Self {
        self.trend = trend;
        self
    }

    pub fn show_at(&self, ui: &mut Ui, rect: Rect) {
        let ctx = ui.ctx().clone();
        profiler::measure_ctx(&ctx, "tile", || self.draw(ui.painter(), rect));
        let response = ui.interact(rect, ui.id().with(self.label), egui::Sense::hover());
        sound::hook_response_auto(&format!("tile:{}", self.label), &response, true);
    }

    pub fn draw(&self, painter: &Painter, rect: Rect) {
        let accessible = accessibility::semantic_tone_for_color(self.accent)
            .map(|tone| accessibility::style_for(painter.ctx(), tone));
        let accent = accessible.map(|style| style.color).unwrap_or(self.accent);
        let corner = radius::R1;
        let cr = egui::epaint::CornerRadius::same(corner as u8);

        crate::v9::paint::paint_bevel(
            painter,
            rect,
            palette::SOOT_BLACK,
            palette::EDGE_DARK,
            corner,
        );

        let well = rect.shrink2(Vec2::new(4.0, 5.0));
        painter.rect_filled(well, cr, palette::OIL_BLACK);
        crate::v9::paint::paint_multistop_gradient(
            painter,
            well,
            &[
                (0.00, Color32::from_rgba_premultiplied(0x24, 0x27, 0x22, 18)),
                (
                    0.22,
                    Color32::from_rgba_premultiplied(0x09, 0x0a, 0x09, 210),
                ),
                (1.00, Color32::from_black_alpha(246)),
            ],
        );
        crate::v9::paint::paint_plate_grain(painter, well.shrink(2.0), 3.0, 2);
        crate::v9::paint::paint_speckle(painter, well.shrink(3.0), 8, 2);

        painter.rect_stroke(
            rect,
            cr,
            Stroke::new(1.0, palette::EDGE_DARK),
            StrokeKind::Inside,
        );
        painter.rect_stroke(
            well,
            cr,
            Stroke::new(1.0, Color32::from_black_alpha(235)),
            StrokeKind::Inside,
        );
        painter.hline(
            (well.left() + 4.0)..=(well.right() - 4.0),
            well.top() + 1.0,
            Stroke::new(1.0, Color32::from_white_alpha(5)),
        );
        painter.hline(
            (well.left() + 4.0)..=(well.right() - 4.0),
            well.bottom() - 1.0,
            Stroke::new(1.0, Color32::from_black_alpha(235)),
        );

        let bar = Rect::from_min_max(
            Pos2::new(well.left() + 2.0, well.top() + 4.0),
            Pos2::new(well.left() + 5.0, well.bottom() - 4.0),
        );
        painter.rect_filled(bar, egui::epaint::CornerRadius::ZERO, accent);
        if let Some(style) = accessible {
            let marker_rect = Rect::from_center_size(
                Pos2::new(bar.center().x, well.center().y),
                Vec2::new(14.0, 14.0),
            );
            accessibility::paint_marker_with_painter(
                painter,
                marker_rect,
                style.marker,
                palette::PARCHMENT,
            );
        }
        painter.line_segment(
            [bar.left_top(), bar.left_bottom()],
            Stroke::new(1.0, Color32::from_white_alpha(10)),
        );
        painter.line_segment(
            [bar.right_top(), bar.right_bottom()],
            Stroke::new(1.0, Color32::from_black_alpha(190)),
        );

        let text_x = well.left() + spacing::S6;
        painter.text(
            Pos2::new(text_x, well.top() + spacing::S3),
            Align2::LEFT_TOP,
            self.label,
            TextRole::Caption.font_id(),
            palette::PARCHMENT_DIM,
        );
        painter.text(
            Pos2::new(text_x, well.bottom() - spacing::S3),
            Align2::LEFT_BOTTOM,
            self.value,
            TextRole::Heading.font_id(),
            accent,
        );

        if rect.width() >= 112.0 {
            crate::v9::paint::paint_slot_screw(
                painter,
                Pos2::new(rect.right() - 8.0, rect.top() + 8.0),
                1.9,
            );
        }

        if self.trend != TileTrend::None {
            let (sym, col) = match self.trend {
                TileTrend::Up => ("+", palette::GOOD),
                TileTrend::Down => ("-", palette::BAD),
                TileTrend::Flat => ("=", palette::MUTED),
                TileTrend::None => unreachable!(),
            };
            painter.text(
                Pos2::new(well.right() - spacing::S3, well.top() + spacing::S3),
                Align2::RIGHT_TOP,
                sym,
                TextRole::Small.font_id(),
                col,
            );
        }
    }
}
