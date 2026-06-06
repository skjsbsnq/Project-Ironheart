//! 将领立绘 widget —— ROADMAP_MILITARY_UI_PARITY.md Phase A。
//!
//! 把 `military.rs:252-272` 用 `circle_filled + rect_filled` 拼出的小人头像
//! 抽成独立组件,统一供底栏军团卡片、左侧军团详情面板、右上军团摘要徽章使用。
//!
//! MVP 渲染 = 暗金渐变背景 + 姓首字大字 + 金边描边。无将领时降饱和 + "?"。
//! 后续可不破坏接口替换为真实立绘 PNG。

use egui::{Color32, FontId, Pos2, Rect, Response, Sense, Stroke, Ui, Vec2};

const PANEL_CARD: Color32 = Color32::from_rgb(0x0d, 0x10, 0x0f);
const PANEL_CARD_SOFT: Color32 = Color32::from_rgb(0x14, 0x18, 0x17);
const STROKE_DARK: Color32 = Color32::from_rgb(0x28, 0x31, 0x31);
const GOLD: Color32 = Color32::from_rgb(0x9f, 0xc1, 0xc8);
const GOLD_BRIGHT: Color32 = Color32::from_rgb(0xd1, 0xdf, 0xdd);
const MUTED: Color32 = Color32::from_gray(140);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PortraitStyle {
    Small,
    Medium,
    Large,
}

impl PortraitStyle {
    pub fn size(self) -> Vec2 {
        match self {
            PortraitStyle::Small => Vec2::new(28.0, 40.0),
            PortraitStyle::Medium => Vec2::new(36.0, 48.0),
            PortraitStyle::Large => Vec2::new(64.0, 80.0),
        }
    }

    pub fn initial_font_size(self) -> f32 {
        match self {
            PortraitStyle::Small => 20.0,
            PortraitStyle::Medium => 24.0,
            PortraitStyle::Large => 40.0,
        }
    }
}

pub fn draw_general_portrait(ui: &mut Ui, name: Option<&str>, style: PortraitStyle) -> Response {
    let size = style.size();
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    if ui.is_rect_visible(rect) {
        paint_portrait_at(ui.painter(), rect, name, style.initial_font_size());
    }
    response
}

pub fn paint_portrait_at(
    painter: &egui::Painter,
    rect: Rect,
    name: Option<&str>,
    initial_font_size: f32,
) {
    let has_name = name.map_or(false, |s| !s.trim().is_empty());
    let (top_color, bot_color, initial_color, stroke_color) = if has_name {
        (PANEL_CARD_SOFT, PANEL_CARD, GOLD_BRIGHT, GOLD)
    } else {
        (
            Color32::from_gray(56),
            Color32::from_gray(36),
            MUTED,
            STROKE_DARK,
        )
    };

    let top_h = rect.height() * 0.42;
    let top_rect = Rect::from_min_size(rect.min, Vec2::new(rect.width(), top_h));
    let bot_rect = Rect::from_min_max(Pos2::new(rect.min.x, rect.min.y + top_h), rect.max);
    painter.rect_filled(top_rect, 2.0, top_color);
    painter.rect_filled(bot_rect, 0.0, bot_color);
    painter.rect_stroke(
        rect,
        2.0,
        Stroke::new(1.2, stroke_color),
        egui::epaint::StrokeKind::Inside,
    );

    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        portrait_initial(name),
        FontId::proportional(initial_font_size),
        initial_color,
    );
}

pub fn portrait_initial(name: Option<&str>) -> String {
    match name {
        Some(s) if !s.trim().is_empty() => s
            .trim()
            .chars()
            .next()
            .map(|c| c.to_string())
            .unwrap_or_else(|| "?".to_string()),
        _ => "?".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portrait_styles_have_positive_size() {
        for s in [
            PortraitStyle::Small,
            PortraitStyle::Medium,
            PortraitStyle::Large,
        ] {
            let sz = s.size();
            assert!(sz.x > 0.0 && sz.y > 0.0, "{:?}", s);
            assert!(s.initial_font_size() > 0.0);
        }
    }

    #[test]
    fn initial_falls_back_to_question() {
        assert_eq!(portrait_initial(None), "?");
        assert_eq!(portrait_initial(Some("")), "?");
        assert_eq!(portrait_initial(Some("   ")), "?");
    }

    #[test]
    fn initial_takes_first_char() {
        assert_eq!(portrait_initial(Some("穆罕默德·梅齐亚纳")), "穆");
        assert_eq!(portrait_initial(Some("Franco")), "F");
        assert_eq!(portrait_initial(Some(" Lin Biao")), "L");
    }
}
