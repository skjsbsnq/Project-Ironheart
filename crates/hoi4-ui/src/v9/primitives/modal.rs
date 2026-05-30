//! V9 modal primitive.

use egui::{Area, Color32, Context, Id, Order, Pos2, Rect, Sense, Ui, Vec2};

use crate::v9::{
    frame::{FrameStyle, PanelFrame},
    layout::AnchorLayout,
    motion, sound,
    tokens::{palette, spacing, Elevation, TextRole},
};

pub struct Modal<'a> {
    id: Id,
    size: Vec2,
    title: Option<&'a str>,
    accent: Color32,
    dim_background: bool,
    open_sound: bool,
}

impl<'a> Modal<'a> {
    pub fn new(id_source: impl std::hash::Hash, size: Vec2) -> Self {
        Self {
            id: Id::new(id_source),
            size,
            title: None,
            accent: palette::BRASS_BRIGHT,
            dim_background: true,
            open_sound: true,
        }
    }

    pub fn title(mut self, title: &'a str) -> Self {
        self.title = Some(title);
        self
    }

    pub fn accent(mut self, accent: Color32) -> Self {
        self.accent = accent;
        self
    }

    pub fn dim_background(mut self, dim_background: bool) -> Self {
        self.dim_background = dim_background;
        self
    }

    pub fn open_sound(mut self, open_sound: bool) -> Self {
        self.open_sound = open_sound;
        self
    }

    pub fn show<R>(
        self,
        ctx: &Context,
        add_contents: impl FnOnce(&mut Ui, Rect) -> R,
    ) -> Option<R> {
        let screen = ctx.screen_rect();
        let modal_rect = motion::modal_enter_rect(ctx, self.id, centered_rect(screen, self.size));
        if self.open_sound {
            sound::emit_once(
                ctx,
                self.id.with("open"),
                "modal",
                sound::V9SoundEvent::Modal,
            );
        }

        if self.dim_background {
            Area::new(self.id.with("backdrop"))
                .order(Order::Foreground)
                .fixed_pos(screen.min)
                .show(ctx, |ui| {
                    let (rect, _) = ui.allocate_exact_size(screen.size(), Sense::click());
                    ui.painter().rect_filled(
                        rect,
                        egui::epaint::CornerRadius::ZERO,
                        Color32::from_black_alpha(126),
                    );
                });
        }

        let mut output = None;
        Area::new(self.id)
            .order(Order::Foreground)
            .fixed_pos(modal_rect.min)
            .default_size(self.size)
            .show(ctx, |ui| {
                let (rect, _) = ui.allocate_exact_size(self.size, Sense::click_and_drag());
                let frame = PanelFrame::new(FrameStyle::Modal, rect)
                    .with_accent(self.accent)
                    .with_elevation(Elevation::E3);
                frame.draw(ui.painter());

                if let Some(title) = self.title {
                    let title_pos = Pos2::new(rect.center().x, rect.top() + 15.0);
                    ui.painter().text(
                        title_pos,
                        egui::Align2::CENTER_CENTER,
                        title,
                        TextRole::Display.font_id(),
                        palette::GOLD_HOT,
                    );
                }

                let mut body = frame.inner_rect();
                body.min.y = rect.top() + 42.0;
                body.max.y -= spacing::S2;
                output = Some(add_contents(ui, body));
            });

        output
    }
}

pub fn centered_rect(screen: Rect, size: Vec2) -> Rect {
    AnchorLayout::Center.place(screen, size, Vec2::ZERO)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn centered_rect_places_modal_in_middle() {
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 800.0));
        let rect = centered_rect(screen, Vec2::new(400.0, 300.0));
        assert_eq!(rect.min, Pos2::new(300.0, 250.0));
        assert_eq!(rect.size(), Vec2::new(400.0, 300.0));
    }
}
