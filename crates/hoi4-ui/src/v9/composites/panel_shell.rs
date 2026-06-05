//! Legacy V9 ornate in-game panel shell.
//!
//! Gate 2.3 boundary: this shell is kept for panels that have not migrated.
//! New migrated panels should use `crate::vanilla_iron::{LedgerPanelShell,
//! WorkbenchShell, DossierPanelShell, CommandPanelShell, JournalPanelShell,
//! UtilityWindowShell}` instead of adding new V9 `PanelShell` bodies or new
//! `*_v9_secondary_tabs` flows.

use egui::{Align2, Area, Color32, Context, Id, Order, Pos2, Rect, Sense, Stroke, Vec2};

use crate::{
    i18n::{current_language, tr, Language},
    v9::{
        frame::{FrameStyle, PanelFrame},
        layout::{GridLayout, Track},
        paint,
        primitives::{Button, ButtonSize, ButtonVariant, Tile, TileTrend},
        text::fit_font_to_width,
        tokens::{palette, spacing, Elevation, TextRole},
    },
};

const DESKTOP_LEFT_RAIL_RESERVE: f32 = 112.0;
const DESKTOP_TOP_GAP: f32 = 96.0;
const COMPACT_TOP_GAP: f32 = 72.0;
const MIN_SUMMARY_TILE_W: f32 = 92.0;

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
                let rows = self.layout_rows(inner.height());
                let grid =
                    GridLayout::new(rows, vec![Track::Fr(1.0)]).with_gutter(0.0, spacing::S3);
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
                self.draw_body_backdrop(&mut content_ui, layout.body);
                self.draw_footer(&mut content_ui, layout.footer);
                output = Some(add_contents(&mut content_ui, layout));
            });

        (close, output)
    }

    fn panel_size(&self, screen: Rect) -> Vec2 {
        let edge = edge_margin(screen);
        let horizontal_reserve = if self.class.left_docked() && screen.width() >= 900.0 {
            DESKTOP_LEFT_RAIL_RESERVE
        } else {
            0.0
        };
        let usable_w = (screen.width() - edge * 2.0).max(1.0);
        let reserved_w = (screen.width() - horizontal_reserve - spacing::S5 - edge)
            .max(1.0)
            .min(usable_w);
        let usable_h = (screen.height() - edge * 2.0).max(1.0);
        let (min_h, max_h_class) = self.class.height_bounds();
        let h = (screen.height() * self.class.height_ratio())
            .clamp(min_h, max_h_class)
            .min(usable_h);
        Vec2::new(self.class.width().min(reserved_w), h)
    }

    fn layout_rows(&self, inner_h: f32) -> Vec<Track> {
        let compact = inner_h < 320.0;
        let header_h = match (self.subtitle.is_some(), compact) {
            (true, true) => 46.0,
            (true, false) => 54.0,
            (false, true) => 42.0,
            (false, false) => 48.0,
        };
        let summary_h = if compact { 50.0 } else { 64.0 };
        let tabs_h = if compact { 30.0 } else { 36.0 };
        let footer_h = if compact { 24.0 } else { 32.0 };
        vec![
            Track::Fixed(header_h),
            Track::Fixed(summary_h),
            Track::Fixed(tabs_h),
            Track::Fr(1.0),
            Track::Fixed(footer_h),
        ]
    }

    fn panel_pos(&self, screen: Rect, size: Vec2) -> Pos2 {
        let edge = edge_margin(screen);
        let top_gap = if screen.height() >= 760.0 {
            DESKTOP_TOP_GAP
        } else {
            COMPACT_TOP_GAP
        };
        let left_gap = if screen.width() >= 900.0 { 88.0 } else { edge };
        let ideal = if self.class.left_docked() {
            Pos2::new(screen.left() + left_gap, screen.top() + top_gap)
        } else {
            Pos2::new(
                screen.center().x - size.x * 0.5,
                screen.center().y - size.y * 0.5,
            )
        };
        Pos2::new(
            clamp_panel_axis(ideal.x, screen.left(), screen.right(), size.x, edge),
            clamp_panel_axis(ideal.y, screen.top(), screen.bottom(), size.y, edge),
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
        let title_available_w = (text_clip.width() - spacing::S5 * 2.0).max(24.0);
        let text_painter = ui.painter().with_clip_rect(text_clip);
        let title_font = fit_font_to_width(
            self.title,
            TextRole::Display.font_id(),
            title_available_w,
            0.70,
        );
        if self.subtitle.is_some() {
            text_painter.text(
                Pos2::new(rect.left() + spacing::S5, rect.top() + spacing::S3),
                Align2::LEFT_TOP,
                self.title,
                title_font,
                palette::GOLD_HOT,
            );
        } else {
            text_painter.text(
                Pos2::new(rect.left() + spacing::S5, rect.center().y - 1.0),
                Align2::LEFT_CENTER,
                self.title,
                title_font,
                palette::GOLD_HOT,
            );
        }
        if let Some(subtitle) = self.subtitle {
            let subtitle_font = fit_font_to_width(
                subtitle,
                TextRole::Caption.font_id(),
                title_available_w,
                0.76,
            );
            text_painter.text(
                Pos2::new(rect.left() + spacing::S5, rect.bottom() - spacing::S2),
                Align2::LEFT_BOTTOM,
                subtitle,
                subtitle_font,
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
        let font = fit_font_to_width(
            footer,
            TextRole::Caption.font_id(),
            (rect.width() - spacing::S6).max(12.0),
            0.72,
        );
        text_painter.text(
            Pos2::new(rect.left() + spacing::S3, rect.center().y),
            Align2::LEFT_CENTER,
            footer,
            font,
            palette::MUTED,
        );
    }

    fn draw_body_backdrop(&self, ui: &mut egui::Ui, rect: Rect) {
        if !rect.is_positive() {
            return;
        }
        paint::paint_recessed_panel(ui.painter(), rect, 1.0);
        if rect.width() > 8.0 && rect.height() > spacing::S6 {
            ui.painter().rect_filled(
                Rect::from_min_max(
                    rect.left_top() + Vec2::new(1.0, spacing::S3),
                    Pos2::new(rect.left() + 3.0, rect.bottom() - spacing::S3),
                ),
                0.0,
                Color32::from_rgba_premultiplied(
                    self.accent.r(),
                    self.accent.g(),
                    self.accent.b(),
                    92,
                ),
            );
        }
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

fn edge_margin(screen: Rect) -> f32 {
    if screen.width() < 360.0 || screen.height() < 260.0 {
        spacing::S2
    } else {
        spacing::S4
    }
}

fn clamp_panel_axis(ideal: f32, min: f32, max: f32, size: f32, margin: f32) -> f32 {
    let lo = min + margin;
    let hi = max - size - margin;
    if lo <= hi {
        ideal.clamp(lo, hi)
    } else {
        min + (max - min - size) * 0.5
    }
}

pub fn draw_summary_tiles(ui: &mut egui::Ui, rect: Rect, items: &[(&str, String, Color32)]) {
    paint::paint_recessed_panel(ui.painter(), rect, 1.0);
    if items.is_empty() {
        return;
    }
    let inner = rect.shrink2(Vec2::new(spacing::S4, spacing::S3));
    let max_tiles = (((inner.width() + spacing::S4) / (MIN_SUMMARY_TILE_W + spacing::S4)).floor()
        as usize)
        .max(1);
    let visible_items: Vec<(&str, String, Color32)> = if items.len() > max_tiles && max_tiles >= 2 {
        let keep = max_tiles - 1;
        let mut visible = items.iter().take(keep).cloned().collect::<Vec<_>>();
        visible.push((
            tr("panel_summary_more"),
            format!("+{}", items.len() - keep),
            palette::MUTED,
        ));
        visible
    } else {
        items.iter().take(max_tiles).cloned().collect()
    };
    let cols = vec![Track::Fr(1.0); visible_items.len()];
    let grid = GridLayout::new(vec![Track::Fr(1.0)], cols).with_gutter(spacing::S4, 0.0);
    let cells = grid.measure(inner);
    for (idx, (label, value, color)) in visible_items.iter().enumerate() {
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
    ui.painter().rect_filled(
        Rect::from_min_max(rect.left_top(), Pos2::new(rect.left() + 3.0, rect.bottom())),
        0.0,
        accent,
    );
    let text_painter = ui.painter().with_clip_rect(rect);
    let font = fit_font_to_width(
        label,
        TextRole::Subheading.font_id(),
        (rect.width() - spacing::S8).max(12.0),
        0.72,
    );
    text_painter.text(
        Pos2::new(rect.left() + spacing::S5, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        font,
        accent,
    );
}

pub fn draw_empty_state(ui: &mut egui::Ui, rect: Rect, title: &str, body: &str) {
    paint::paint_recessed_panel(ui.painter(), rect, 1.0);
    let text_rect = rect.shrink2(Vec2::new(spacing::S6, spacing::S4));
    let text_painter = ui.painter().with_clip_rect(text_rect);
    let title_font = fit_font_to_width(title, TextRole::Heading.font_id(), text_rect.width(), 0.72);
    let body_font = fit_font_to_width(body, TextRole::Body.font_id(), text_rect.width(), 0.78);
    text_painter.text(
        Pos2::new(rect.center().x, rect.center().y - spacing::S4),
        Align2::CENTER_CENTER,
        title,
        title_font,
        palette::BRASS_BRIGHT,
    );
    text_painter.text(
        Pos2::new(rect.center().x, rect.center().y + spacing::S5),
        Align2::CENTER_CENTER,
        body,
        body_font,
        palette::PARCHMENT_DIM,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen(w: f32, h: f32) -> Rect {
        Rect::from_min_size(Pos2::ZERO, Vec2::new(w, h))
    }

    #[test]
    fn panel_size_stays_inside_compact_screen() {
        let shell = PanelShell::new("small_screen", "Treasury").class(PanelClass::Economy);
        let screen = screen(320.0, 220.0);
        let size = shell.panel_size(screen);
        assert!(size.x <= screen.width());
        assert!(size.y <= screen.height());

        let pos = shell.panel_pos(screen, size);
        assert!(pos.x.is_finite());
        assert!(pos.y.is_finite());
    }

    #[test]
    fn panel_position_is_clamped_on_desktop() {
        let shell = PanelShell::new("desktop_screen", "Research").class(PanelClass::Detail);
        let screen = screen(1280.0, 720.0);
        let size = shell.panel_size(screen);
        let pos = shell.panel_pos(screen, size);
        assert!(pos.x >= screen.left() + spacing::S4);
        assert!(pos.y >= screen.top() + spacing::S4);
        assert!(pos.x + size.x <= screen.right() - spacing::S4 + 0.01);
        assert!(pos.y + size.y <= screen.bottom() - spacing::S4 + 0.01);
    }

    #[test]
    fn compact_rows_reduce_fixed_chrome() {
        let compact = PanelShell::new("compact_rows", "Politics").layout_rows(280.0);
        let normal = PanelShell::new("normal_rows", "Politics").layout_rows(640.0);
        let compact_fixed: f32 = compact
            .iter()
            .map(|track| match track {
                Track::Fixed(px) => *px,
                Track::Fr(_) => 0.0,
            })
            .sum();
        let normal_fixed: f32 = normal
            .iter()
            .map(|track| match track {
                Track::Fixed(px) => *px,
                Track::Fr(_) => 0.0,
            })
            .sum();
        assert!(compact_fixed < normal_fixed);
    }
}
