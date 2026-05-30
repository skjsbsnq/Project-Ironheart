//! V9 tooltip primitive.

use egui::{Area, Color32, Context, Id, Order, Pos2, Rect, Response, Sense, Stroke, Ui, Vec2};

use crate::v9::{
    frame::{FrameStyle, PanelFrame},
    tokens::{palette, spacing, Elevation, TextRole},
};

const DEFAULT_WIDTH: f32 = 280.0;

pub struct Tooltip<'a> {
    title: Option<&'a str>,
    body: &'a str,
    accent: Color32,
    max_width: f32,
}

impl<'a> Tooltip<'a> {
    pub fn new(body: &'a str) -> Self {
        Self {
            title: None,
            body,
            accent: palette::BRASS_BRIGHT,
            max_width: DEFAULT_WIDTH,
        }
    }

    pub fn titled(title: &'a str, body: &'a str) -> Self {
        Self::new(body).title(title)
    }

    pub fn title(mut self, title: &'a str) -> Self {
        self.title = Some(title);
        self
    }

    pub fn accent(mut self, accent: Color32) -> Self {
        self.accent = accent;
        self
    }

    pub fn max_width(mut self, max_width: f32) -> Self {
        self.max_width = max_width.clamp(160.0, 420.0);
        self
    }

    pub fn show_for_response(&self, ui: &Ui, response: &Response) {
        if !response.hovered() {
            return;
        }
        let pos = ui
            .ctx()
            .pointer_hover_pos()
            .unwrap_or_else(|| response.rect.right_top())
            + Vec2::new(14.0, 18.0);
        self.show_at(ui.ctx(), response.id.with("v9_tooltip"), pos);
    }

    pub fn show_at(&self, ctx: &Context, id: Id, pos: Pos2) {
        let screen = ctx.screen_rect();
        let width = self
            .max_width
            .min((screen.width() - spacing::S8).max(160.0));
        let pos = Pos2::new(
            pos.x
                .min(screen.right() - width - spacing::S4)
                .max(screen.left() + spacing::S4),
            pos.y
                .min(screen.bottom() - 120.0)
                .max(screen.top() + spacing::S4),
        );

        Area::new(id)
            .order(Order::Tooltip)
            .fixed_pos(pos)
            .show(ctx, |ui| {
                let text_w = width - spacing::S5 * 2.0;
                let (title_galley, body_galley, height) = {
                    let painter = ui.painter();
                    let title_galley = self.title.map(|title| {
                        painter.layout_no_wrap(
                            title.to_owned(),
                            TextRole::Subheading.font_id(),
                            palette::GOLD_HOT,
                        )
                    });
                    let body_galley = painter.layout(
                        self.body.to_owned(),
                        TextRole::Body.font_id(),
                        palette::PARCHMENT,
                        text_w,
                    );
                    let title_h = title_galley
                        .as_ref()
                        .map(|g| g.size().y + spacing::S2)
                        .unwrap_or(0.0);
                    let height =
                        (spacing::S4 * 2.0 + title_h + body_galley.size().y).clamp(42.0, 260.0);
                    (title_galley, body_galley, height)
                };
                let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
                let painter = ui.painter();
                PanelFrame::new(FrameStyle::Glass, rect)
                    .with_accent(self.accent)
                    .with_elevation(Elevation::E2)
                    .draw(painter);

                let accent_line = Rect::from_min_max(
                    rect.left_top() + Vec2::new(spacing::S3, spacing::S3),
                    Pos2::new(rect.left() + spacing::S3 + 3.0, rect.bottom() - spacing::S3),
                );
                painter.rect_filled(accent_line, 0.0, self.accent);
                painter.line_segment(
                    [accent_line.right_top(), accent_line.right_bottom()],
                    Stroke::new(1.0, Color32::from_black_alpha(180)),
                );

                let text_pos = rect.min + Vec2::new(spacing::S5, spacing::S4);
                let mut y = text_pos.y;
                if let Some(galley) = title_galley {
                    painter.galley(Pos2::new(text_pos.x, y), galley.clone(), palette::GOLD_HOT);
                    y += galley.size().y + spacing::S2;
                }
                painter.galley(Pos2::new(text_pos.x, y), body_galley, palette::PARCHMENT);
            });
    }
}
