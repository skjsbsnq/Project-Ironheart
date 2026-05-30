//! Standard V9 in-game panel shell.

use egui::{Align2, Area, Color32, Context, Id, Order, Pos2, Rect, Sense, Stroke, Vec2};

use crate::{
    i18n::{current_language, tr, Language},
    v9::{
        frame::{FrameStyle, PanelFrame},
        layout::{GridLayout, Track},
        paint,
        primitives::{Button, ButtonSize, ButtonVariant, Tile, TileTrend},
        tokens::{palette, spacing, Elevation, TextRole},
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelClass {
    Compact,
    MilitaryDiplomacy,
    Economy,
    Detail,
    Settings,
}

impl PanelClass {
    fn width(self) -> f32 {
        match self {
            Self::Compact => 560.0,
            Self::MilitaryDiplomacy => 960.0,
            Self::Economy => 1080.0,
            Self::Detail => 1200.0,
            Self::Settings => 720.0,
        }
    }

    fn height_ratio(self) -> f32 {
        match self {
            Self::Compact => 0.58,
            Self::MilitaryDiplomacy => 0.72,
            Self::Economy => 0.74,
            Self::Detail => 0.80,
            Self::Settings => 0.66,
        }
    }

    fn height_bounds(self) -> (f32, f32) {
        match self {
            Self::Compact => (360.0, 620.0),
            Self::MilitaryDiplomacy => (500.0, 820.0),
            Self::Economy => (540.0, 860.0),
            Self::Detail => (560.0, 920.0),
            Self::Settings => (460.0, 720.0),
        }
    }

    fn left_docked(self) -> bool {
        matches!(
            self,
            Self::Compact | Self::MilitaryDiplomacy | Self::Economy
        )
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PanelShellLayout {
    pub outer: Rect,
    pub inner: Rect,
    pub header: Rect,
    pub summary: Rect,
    pub tabs: Rect,
    pub body: Rect,
    pub footer: Rect,
}

pub struct PanelShell<'a> {
    id: Id,
    title: &'a str,
    subtitle: Option<&'a str>,
    footer: Option<&'a str>,
    class: PanelClass,
    accent: Color32,
}

impl<'a> PanelShell<'a> {
    pub fn new(id_source: impl std::hash::Hash, title: &'a str) -> Self {
        Self {
            id: Id::new(id_source),
            title,
            subtitle: None,
            footer: None,
            class: PanelClass::MilitaryDiplomacy,
            accent: palette::BRASS_BRIGHT,
        }
    }

    pub fn subtitle(mut self, subtitle: &'a str) -> Self {
        self.subtitle = Some(subtitle);
        self
    }

    pub fn footer(mut self, footer: &'a str) -> Self {
        self.footer = Some(footer);
        self
    }

    pub fn class(mut self, class: PanelClass) -> Self {
        self.class = class;
        self
    }

    pub fn accent(mut self, accent: Color32) -> Self {
        self.accent = accent;
        self
    }

    pub fn show<R>(
        self,
        ctx: &Context,
        add_contents: impl FnOnce(&mut egui::Ui, PanelShellLayout) -> R,
    ) -> (bool, Option<R>) {
        let screen = ctx.screen_rect();
        let size = self.panel_size(screen);
        let pos = self.panel_pos(screen, size);

        let mut close = false;
        let mut output = None;
        Area::new(self.id)
            .order(Order::Foreground)
            .fixed_pos(pos)
            .default_size(size)
            .show(ctx, |ui| {
                let (outer, _) = ui.allocate_exact_size(size, Sense::click_and_drag());
                let frame = PanelFrame::new(FrameStyle::Ornate, outer)
                    .with_accent(self.accent)
                    .with_elevation(Elevation::E3);
                frame.draw(ui.painter());

                let inner = frame.inner_rect();
                let grid = GridLayout::new(
                    vec![
                        Track::Fixed(48.0),
                        Track::Fixed(64.0),
                        Track::Fixed(36.0),
                        Track::Fr(1.0),
                        Track::Fixed(32.0),
                    ],
                    vec![Track::Fr(1.0)],
                )
                .with_gutter(0.0, spacing::S3);
                let cells = grid.measure(inner);
                let layout = PanelShellLayout {
                    outer,
                    inner,
                    header: GridLayout::cell(&cells, 0, 0),
                    summary: GridLayout::cell(&cells, 1, 0),
                    tabs: GridLayout::cell(&cells, 2, 0),
                    body: GridLayout::cell(&cells, 3, 0),
                    footer: GridLayout::cell(&cells, 4, 0),
                };

                let mut content_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .id_salt(self.id.with("content"))
                        .max_rect(inner)
                        .layout(egui::Layout::top_down(egui::Align::Min))
                        .sense(Sense::hover()),
                );
                content_ui.set_clip_rect(inner);

                close = self.draw_header(&mut content_ui, layout.header);
                self.draw_footer(&mut content_ui, layout.footer);
                output = Some(add_contents(&mut content_ui, layout));
            });

        (close, output)
    }

    fn panel_size(&self, screen: Rect) -> Vec2 {
        let horizontal_reserve = if self.class.left_docked() && screen.width() >= 900.0 {
            112.0
        } else {
            spacing::S8
        };
        let max_w = (screen.width() - horizontal_reserve - spacing::S5).max(360.0);
        let max_h = (screen.height() - 132.0).max(360.0);
        let (min_h, max_h_class) = self.class.height_bounds();
        let h = (screen.height() * self.class.height_ratio())
            .clamp(min_h, max_h_class)
            .min(max_h);
        Vec2::new(self.class.width().min(max_w), h)
    }

    fn panel_pos(&self, screen: Rect, size: Vec2) -> Pos2 {
        let top_gap = if screen.height() >= 760.0 { 96.0 } else { 72.0 };
        let left_gap = if screen.width() >= 900.0 {
            88.0
        } else {
            spacing::S4
        };
        let ideal = if self.class.left_docked() {
            Pos2::new(screen.left() + left_gap, screen.top() + top_gap)
        } else {
            Pos2::new(
                screen.center().x - size.x * 0.5,
                screen.center().y - size.y * 0.5,
            )
        };
        Pos2::new(
            ideal.x.clamp(
                screen.left() + spacing::S4,
                screen.right() - size.x - spacing::S4,
            ),
            ideal.y.clamp(
                screen.top() + spacing::S4,
                screen.bottom() - size.y - spacing::S4,
            ),
        )
    }

    fn draw_header(&self, ui: &mut egui::Ui, rect: Rect) -> bool {
        paint::paint_recessed_panel(ui.painter(), rect, 1.0);
        ui.painter().hline(
            rect.left()..=rect.right(),
            rect.bottom() - 1.0,
            Stroke::new(1.0, self.accent),
        );
        ui.painter().rect_filled(
            Rect::from_min_max(rect.left_top(), Pos2::new(rect.left() + 4.0, rect.bottom())),
            0.0,
            self.accent,
        );
        let close_rect = Rect::from_min_size(
            Pos2::new(rect.right() - 32.0, rect.top() + 8.0),
            Vec2::new(24.0, 24.0),
        );
        let text_clip = Rect::from_min_max(
            rect.left_top(),
            Pos2::new(close_rect.left() - spacing::S3, rect.bottom()),
        );
        let text_painter = ui.painter().with_clip_rect(text_clip);
        text_painter.text(
            Pos2::new(rect.left() + spacing::S5, rect.center().y - 2.0),
            Align2::LEFT_CENTER,
            self.title,
            TextRole::Display.font_id(),
            palette::GOLD_HOT,
        );
        if let Some(subtitle) = self.subtitle {
            text_painter.text(
                Pos2::new(rect.left() + spacing::S5, rect.bottom() - spacing::S2),
                Align2::LEFT_BOTTOM,
                subtitle,
                TextRole::Caption.font_id(),
                palette::PARCHMENT_DIM,
            );
        }
        Button::new("×")
            .size(ButtonSize::Sm)
            .variant(ButtonVariant::Ghost)
            .show_at(ui, close_rect)
            .on_hover_text(tr("panel_close_hint"))
            .clicked()
    }

    fn draw_footer(&self, ui: &mut egui::Ui, rect: Rect) {
        ui.painter().hline(
            rect.left()..=rect.right(),
            rect.top(),
            Stroke::new(1.0, Color32::from_black_alpha(220)),
        );
        let footer = localized_footer(self.footer);
        let text_painter = ui.painter().with_clip_rect(rect);
        text_painter.text(
            Pos2::new(rect.left() + spacing::S3, rect.center().y),
            Align2::LEFT_CENTER,
            footer,
            TextRole::Caption.font_id(),
            palette::MUTED,
        );
    }
}

fn localized_footer(custom: Option<&str>) -> &str {
    if current_language() == Language::Chinese {
        return match custom {
            Some("Q Close  |  Apply writes settings.toml") => tr("panel_footer_settings"),
            _ => tr("panel_footer_default"),
        };
    }
    custom.unwrap_or_else(|| tr("panel_footer_default"))
}

pub fn draw_summary_tiles(ui: &mut egui::Ui, rect: Rect, items: &[(&str, String, Color32)]) {
    paint::paint_recessed_panel(ui.painter(), rect, 1.0);
    if items.is_empty() {
        return;
    }
    let cols = vec![Track::Fr(1.0); items.len()];
    let grid = GridLayout::new(vec![Track::Fr(1.0)], cols).with_gutter(spacing::S4, 0.0);
    let cells = grid.measure(rect.shrink2(Vec2::new(spacing::S4, spacing::S3)));
    for (idx, (label, value, color)) in items.iter().enumerate() {
        let cell = GridLayout::cell(&cells, 0, idx);
        Tile::new(label, value)
            .accent(*color)
            .trend(TileTrend::None)
            .show_at(ui, cell);
    }
}

pub fn draw_tab_strip(ui: &mut egui::Ui, rect: Rect, label: &str, accent: Color32) {
    paint::paint_bevel(
        ui.painter(),
        rect,
        palette::SOOT_BLACK,
        palette::EDGE_DARK,
        1.0,
    );
    let text_painter = ui.painter().with_clip_rect(rect);
    text_painter.text(
        Pos2::new(rect.left() + spacing::S5, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        TextRole::Subheading.font_id(),
        accent,
    );
}

pub fn draw_empty_state(ui: &mut egui::Ui, rect: Rect, title: &str, body: &str) {
    paint::paint_recessed_panel(ui.painter(), rect, 1.0);
    let text_painter = ui.painter().with_clip_rect(rect);
    text_painter.text(
        Pos2::new(rect.center().x, rect.center().y - spacing::S4),
        Align2::CENTER_CENTER,
        title,
        TextRole::Heading.font_id(),
        palette::BRASS_BRIGHT,
    );
    text_painter.text(
        Pos2::new(rect.center().x, rect.center().y + spacing::S5),
        Align2::CENTER_CENTER,
        body,
        TextRole::Body.font_id(),
        palette::PARCHMENT_DIM,
    );
}
