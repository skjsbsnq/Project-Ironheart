//! Standard V9 in-game panel shell.

use egui::{Align2, Area, Color32, Context, Id, Order, Pos2, Rect, Sense, Stroke, Vec2};

use crate::v9::{
    frame::{FrameStyle, PanelFrame},
    layout::{GridLayout, Track},
    paint,
    primitives::{Button, ButtonSize, ButtonVariant, Tile, TileTrend},
    tokens::{palette, spacing, Elevation, TextRole},
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
            Self::Compact => 720.0,
            Self::MilitaryDiplomacy => 960.0,
            Self::Economy => 1080.0,
            Self::Detail => 1200.0,
            Self::Settings => 720.0,
        }
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
        let pos = Pos2::new(
            screen.center().x - size.x * 0.5,
            screen.center().y - size.y * 0.5,
        );

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
                        Track::Fixed(56.0),
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

                close = self.draw_header(ui, layout.header);
                self.draw_footer(ui, layout.footer);
                output = Some(add_contents(ui, layout));
            });

        (close, output)
    }

    fn panel_size(&self, screen: Rect) -> Vec2 {
        let max_w = (screen.width() - spacing::S8).max(360.0);
        let max_h = (screen.height() - spacing::S8).max(360.0);
        let h = (screen.height() * 0.78).clamp(540.0, 920.0).min(max_h);
        Vec2::new(self.class.width().min(max_w), h)
    }

    fn draw_header(&self, ui: &mut egui::Ui, rect: Rect) -> bool {
        paint::paint_recessed_panel(ui.painter(), rect, 1.0);
        ui.painter().hline(
            rect.left()..=rect.right(),
            rect.bottom() - 1.0,
            Stroke::new(1.0, self.accent),
        );
        ui.painter().text(
            Pos2::new(rect.left() + spacing::S5, rect.center().y - 2.0),
            Align2::LEFT_CENTER,
            self.title,
            TextRole::Display.font_id(),
            palette::GOLD_HOT,
        );
        if let Some(subtitle) = self.subtitle {
            ui.painter().text(
                Pos2::new(rect.left() + spacing::S5, rect.bottom() - spacing::S2),
                Align2::LEFT_BOTTOM,
                subtitle,
                TextRole::Caption.font_id(),
                palette::PARCHMENT_DIM,
            );
        }
        let close_rect = Rect::from_min_size(
            Pos2::new(rect.right() - 32.0, rect.top() + 8.0),
            Vec2::new(24.0, 24.0),
        );
        Button::new("X")
            .size(ButtonSize::Sm)
            .variant(ButtonVariant::Ghost)
            .show_at(ui, close_rect)
            .clicked()
    }

    fn draw_footer(&self, ui: &mut egui::Ui, rect: Rect) {
        ui.painter().hline(
            rect.left()..=rect.right(),
            rect.top(),
            Stroke::new(1.0, Color32::from_black_alpha(220)),
        );
        let footer = self.footer.unwrap_or("Q Close  |  Tab Switch");
        ui.painter().text(
            Pos2::new(rect.left() + spacing::S3, rect.center().y),
            Align2::LEFT_CENTER,
            footer,
            TextRole::Caption.font_id(),
            palette::MUTED,
        );
    }
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
    ui.painter().text(
        Pos2::new(rect.left() + spacing::S5, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        TextRole::Subheading.font_id(),
        accent,
    );
}

pub fn draw_empty_state(ui: &mut egui::Ui, rect: Rect, title: &str, body: &str) {
    paint::paint_recessed_panel(ui.painter(), rect, 1.0);
    ui.painter().text(
        Pos2::new(rect.center().x, rect.center().y - spacing::S4),
        Align2::CENTER_CENTER,
        title,
        TextRole::Heading.font_id(),
        palette::BRASS_BRIGHT,
    );
    ui.painter().text(
        Pos2::new(rect.center().x, rect.center().y + spacing::S5),
        Align2::CENTER_CENTER,
        body,
        TextRole::Body.font_id(),
        palette::PARCHMENT_DIM,
    );
}
