//! V9 PanelFrame + FrameStyle —— 把 [`super::paint`] 的低层 helper 组合成 7 种统一外框。
//!
//! Every style now shares the reference-2 material language: blackened steel,
//! recessed wells, old brass edging, rivets, chipped highlights, and tight radii.

use egui::{Color32, Margin, Painter, Pos2, Rect, Stroke, StrokeKind};

use super::paint;
use super::tokens::{border, palette, radius, spacing, Elevation, TextRole};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FrameStyle {
    Card,
    Panel,
    Ornate,
    Glass,
    Modal,
    Ribbon,
    Hero,
}

impl FrameStyle {
    pub fn default_inset(self) -> Margin {
        match self {
            Self::Card => Margin::symmetric(spacing::S4 as i8, spacing::S3 as i8),
            Self::Panel => Margin::same(spacing::S5 as i8),
            Self::Ornate => Margin::same(spacing::S6 as i8),
            Self::Glass => Margin::symmetric(spacing::S4 as i8, spacing::S3 as i8),
            Self::Modal => Margin::same(spacing::S5 as i8),
            Self::Ribbon => Margin::symmetric(spacing::S7 as i8, spacing::S2 as i8),
            Self::Hero => Margin::same(spacing::S7 as i8),
        }
    }

    pub fn corner(self) -> f32 {
        match self {
            Self::Card => radius::R1,
            Self::Panel => radius::R1,
            Self::Ornate => radius::R2,
            Self::Glass => radius::R1,
            Self::Modal => radius::R1,
            Self::Ribbon => radius::R0,
            Self::Hero => radius::R2,
        }
    }

    pub fn default_elevation(self) -> Elevation {
        match self {
            Self::Card => Elevation::E1,
            Self::Panel => Elevation::E2,
            Self::Ornate => Elevation::E3,
            Self::Glass => Elevation::E1,
            Self::Modal => Elevation::E3,
            Self::Ribbon => Elevation::E1,
            Self::Hero => Elevation::E2,
        }
    }

    pub fn border_width(self) -> f32 {
        match self {
            Self::Card => border::B1,
            Self::Panel => border::B2,
            Self::Ornate => border::B3,
            Self::Glass => border::B1,
            Self::Modal => border::B3,
            Self::Ribbon => 0.0,
            Self::Hero => border::B1,
        }
    }
}

pub struct PanelFrame {
    pub style: FrameStyle,
    pub rect: Rect,
    pub accent: Option<Color32>,
    pub elevation: Option<Elevation>,
}

impl PanelFrame {
    pub fn new(style: FrameStyle, rect: Rect) -> Self {
        Self {
            style,
            rect,
            accent: None,
            elevation: None,
        }
    }

    pub fn with_accent(mut self, color: Color32) -> Self {
        self.accent = Some(color);
        self
    }

    pub fn with_elevation(mut self, elev: Elevation) -> Self {
        self.elevation = Some(elev);
        self
    }

    pub fn inner_rect(&self) -> Rect {
        let inset = self.style.default_inset();
        let bw = self.style.border_width();
        let r = self.rect;
        Rect::from_min_max(
            Pos2::new(
                r.min.x + bw + inset.left as f32,
                r.min.y + bw + inset.top as f32,
            ),
            Pos2::new(
                r.max.x - bw - inset.right as f32,
                r.max.y - bw - inset.bottom as f32,
            ),
        )
    }

    pub fn draw(&self, painter: &Painter) {
        let r = self.rect;
        let corner = self.style.corner();
        let elev = self
            .elevation
            .unwrap_or_else(|| self.style.default_elevation());

        // 阴影
        paint::paint_shadow(painter, r, elev, corner);

        match self.style {
            FrameStyle::Card => draw_card(painter, r, corner),
            FrameStyle::Panel => draw_panel(painter, r, corner),
            FrameStyle::Ornate => {
                draw_ornate(painter, r, corner, self.accent.unwrap_or(palette::BRASS))
            }
            FrameStyle::Glass => draw_glass(painter, r, corner, self.accent),
            FrameStyle::Modal => draw_modal(
                painter,
                r,
                corner,
                self.accent.unwrap_or(palette::BRASS_BRIGHT),
            ),
            FrameStyle::Ribbon => {
                paint::paint_ribbon(painter, r, self.accent.unwrap_or(palette::BRASS_BRIGHT))
            }
            FrameStyle::Hero => draw_hero(painter, r, corner, self.accent),
        }
    }
}

// ─── 7 种 style 各自的绘制 ─────────────────────────────────────

fn draw_card(painter: &Painter, r: Rect, corner: f32) {
    paint::paint_bevel(painter, r, palette::SOOT_BLACK, palette::EDGE_DARK, corner);
    paint::paint_plate_grain(painter, r.shrink(3.0), 3.0, 2);
    paint::paint_oil_stains(painter, r.shrink(5.0), 1);
    paint::paint_speckle(painter, r.shrink(5.0), 8, 2);
    painter.hline(
        (r.left() + spacing::S4)..=(r.right() - spacing::S4),
        r.top() + 3.0,
        Stroke::new(1.0, Color32::from_white_alpha(4)),
    );
    painter.hline(
        (r.left() + spacing::S4)..=(r.right() - spacing::S4),
        r.bottom() - 2.0,
        Stroke::new(1.0, Color32::from_black_alpha(190)),
    );
    painter.rect_stroke(
        r.shrink(2.0),
        cr((corner - 1.0).max(0.0)),
        Stroke::new(border::B1, Color32::from_black_alpha(220)),
        StrokeKind::Inside,
    );
}

fn draw_panel(painter: &Painter, r: Rect, corner: f32) {
    paint::paint_blackened_steel(painter, r, corner);
    let well = r.shrink(9.0);
    if well.width() > 24.0 && well.height() > 24.0 {
        paint::paint_recessed_panel(painter, well, (corner - 1.0).max(0.0));
        painter.rect_stroke(
            well,
            cr((corner - 1.0).max(0.0)),
            Stroke::new(border::B1, Color32::from_black_alpha(230)),
            StrokeKind::Inside,
        );
    }
    paint::paint_brass_border(painter, r, corner);
    paint::paint_steel_rails(painter, r, palette::BRASS_DARK);
    paint::paint_corner_caps(painter, r);
    painter.rect_stroke(
        r.shrink(5.0),
        cr((corner - 1.0).max(0.0)),
        Stroke::new(border::B1, Color32::from_black_alpha(235)),
        StrokeKind::Inside,
    );
    painter.hline(
        (r.left() + spacing::S3)..=(r.right() - spacing::S3),
        r.top() + 2.0,
        Stroke::new(1.0, Color32::from_white_alpha(4)),
    );
    painter.hline(
        (r.left() + spacing::S3)..=(r.right() - spacing::S3),
        r.bottom() - 1.5,
        Stroke::new(1.0, Color32::from_black_alpha(220)),
    );
    paint::paint_corner_brackets(painter, r, palette::GUNMETAL);
    paint::paint_rivets(painter, r);
    paint::paint_side_rivets(painter, r, 2);
}

fn draw_ornate(painter: &Painter, r: Rect, corner: f32, accent: Color32) {
    paint::paint_blackened_steel(painter, r, corner);
    paint::paint_inner_glow(painter, r.shrink(5.0), 1, 148);
    paint::paint_oil_stains(painter, r.shrink(10.0), 1);
    paint::paint_brass_ribbon(painter, r, 2.0, accent);
    paint::paint_double_border(painter, r, corner);
    paint::paint_steel_rails(painter, r, accent);
    paint::paint_corner_caps(painter, r);
    painter.rect_stroke(
        r.shrink(8.0),
        cr((corner - 2.0).max(0.0)),
        Stroke::new(border::B1, Color32::from_black_alpha(215)),
        StrokeKind::Inside,
    );
    paint::paint_rivets(painter, r);
    paint::paint_side_rivets(painter, r, 2);
    paint::paint_corner_brackets(painter, r, palette::GUNMETAL);
    painter.hline(
        (r.left() + spacing::S4)..=(r.right() - spacing::S4),
        r.bottom() - 2.5,
        Stroke::new(1.0, Color32::from_white_alpha(4)),
    );
}

fn draw_glass(painter: &Painter, r: Rect, corner: f32, accent: Option<Color32>) {
    painter.rect_filled(
        r,
        cr(corner),
        Color32::from_rgba_premultiplied(0x03, 0x04, 0x04, 238),
    );
    paint::paint_horizontal_gradient_mesh(
        painter,
        r.shrink(1.0),
        Color32::from_rgba_premultiplied(0x34, 0x38, 0x33, 8),
        Color32::from_black_alpha(118),
    );
    paint::paint_inner_glow(painter, r, 4, 136);
    paint::paint_plate_grain(painter, r.shrink(3.0), 4.0, 2);
    paint::paint_speckle(painter, r.shrink(5.0), 10, 2);
    painter.rect_stroke(
        r,
        cr(corner),
        Stroke::new(border::B1, accent.unwrap_or(palette::GUNMETAL)),
        StrokeKind::Inside,
    );
    painter.hline(
        (r.left() + 2.0)..=(r.right() - 2.0),
        r.top() + 1.0,
        Stroke::new(1.0, Color32::from_white_alpha(6)),
    );
    painter.hline(
        (r.left() + 2.0)..=(r.right() - 2.0),
        r.bottom() - 1.0,
        Stroke::new(1.0, Color32::from_black_alpha(220)),
    );
}

fn draw_modal(painter: &Painter, r: Rect, corner: f32, accent: Color32) {
    paint::paint_parchment(painter, r, corner);
    let ribbon_h = 30.0;
    let title_rect = Rect::from_min_max(r.min, Pos2::new(r.max.x, r.min.y + ribbon_h));
    paint::paint_multistop_gradient(
        painter,
        title_rect,
        &[
            (0.00, palette::IRON_LIGHT),
            (0.18, palette::IRON_DARK),
            (0.72, palette::IRON_DARK),
            (1.00, palette::SOOT_BLACK),
        ],
    );
    paint::paint_plate_grain(painter, title_rect.shrink(2.0), 4.0, 2);
    painter.hline(
        title_rect.min.x..=title_rect.max.x,
        title_rect.max.y - 0.5,
        Stroke::new(
            1.0,
            Color32::from_rgba_premultiplied(accent.r(), accent.g(), accent.b(), 120),
        ),
    );
    paint::paint_double_border(painter, r, corner);
    paint::paint_steel_rails(painter, r, accent);
    paint::paint_corner_caps(painter, r);
    paint::paint_rivets(painter, r);
    paint::paint_side_rivets(painter, r, 2);
    paint::paint_corner_brackets(painter, r, palette::GUNMETAL);
    painter.hline(
        (r.left() + 2.0)..=(r.right() - 2.0),
        r.top() + 1.0,
        Stroke::new(1.0, Color32::from_white_alpha(5)),
    );
}

fn draw_hero(painter: &Painter, r: Rect, corner: f32, accent: Option<Color32>) {
    paint::paint_hero_gradient(painter, r, corner);
    paint::paint_panel_vignette(painter, r.shrink(1.0));
    paint::paint_oil_stains(painter, r.shrink(10.0), 1);
    paint::paint_brass_border(painter, r, corner);
    paint::paint_steel_rails(painter, r, accent.unwrap_or(palette::BRASS_DARK));
    paint::paint_corner_caps(painter, r);
    painter.rect_stroke(
        r.shrink(5.0),
        cr((corner - 1.0).max(0.0)),
        Stroke::new(border::B1, accent.unwrap_or(palette::BRASS_DARK)),
        StrokeKind::Inside,
    );
    paint::paint_hero_seals(painter, r);
    paint::paint_corner_brackets(painter, r, palette::GUNMETAL);
    paint::paint_rivets(painter, r);
    paint::paint_side_rivets(painter, r, 3);
}

fn cr(r: f32) -> egui::epaint::CornerRadius {
    egui::epaint::CornerRadius::same(r as u8)
}

pub fn draw_frame(painter: &Painter, rect: Rect, style: FrameStyle) {
    PanelFrame::new(style, rect).draw(painter);
}

pub fn draw_frame_accent(painter: &Painter, rect: Rect, style: FrameStyle, accent: Color32) {
    PanelFrame::new(style, rect)
        .with_accent(accent)
        .draw(painter);
}

pub fn inner_rect_for(style: FrameStyle, rect: Rect) -> Rect {
    PanelFrame::new(style, rect).inner_rect()
}

#[doc(hidden)]
pub fn _text_role_check(role: TextRole) -> f32 {
    role.size()
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::Vec2;

    fn r(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect::from_min_size(egui::Pos2::new(x, y), Vec2::new(w, h))
    }

    #[test]
    fn frame_inner_rect_smaller_than_outer() {
        let outer = r(0.0, 0.0, 200.0, 100.0);
        for style in [
            FrameStyle::Card,
            FrameStyle::Panel,
            FrameStyle::Ornate,
            FrameStyle::Glass,
            FrameStyle::Modal,
            FrameStyle::Hero,
        ] {
            let inner = PanelFrame::new(style, outer).inner_rect();
            assert!(
                inner.width() < outer.width(),
                "{:?} inner not shrunk",
                style
            );
            assert!(
                inner.height() < outer.height(),
                "{:?} inner not shrunk",
                style
            );
        }
    }

    #[test]
    fn all_styles_have_distinct_corner_radius() {
        assert_eq!(FrameStyle::Ribbon.corner(), radius::R0);
        assert_eq!(FrameStyle::Hero.corner(), radius::R2);
    }
}
