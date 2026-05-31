//! V9 文本绘制 helper：从 `TextRole` 取 `FontId`，统一调用 `Painter::text`。
//!
//! 调用方禁止裸写 `RichText::size(...)`；必须通过 [`draw_text`] / [`measure_text`]
//! 或 [`TextRole::font_id`] 间接。

use egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Vec2};

use super::tokens::palette;
pub use super::tokens::TextRole;

/// 在 `pos` 处绘制单段文字。`anchor` 决定 pos 是文字哪个角的锚点。
///
/// 返回绘制后的 `Rect`（用于链式布局）。
pub fn draw_text(
    painter: &Painter,
    pos: Pos2,
    anchor: Align2,
    role: TextRole,
    text: &str,
    color: Color32,
) -> Rect {
    painter.text(pos, anchor, text, role.font_id(), color)
}

/// 默认 parchment 色绘制。
pub fn draw_body(painter: &Painter, pos: Pos2, anchor: Align2, text: &str) -> Rect {
    draw_text(
        painter,
        pos,
        anchor,
        TextRole::Body,
        text,
        palette::PARCHMENT,
    )
}

/// 黄铜亮色绘制标题。
pub fn draw_heading(painter: &Painter, pos: Pos2, anchor: Align2, text: &str) -> Rect {
    draw_text(
        painter,
        pos,
        anchor,
        TextRole::Heading,
        text,
        palette::BRASS_BRIGHT,
    )
}

/// 在 `Rect` 内居中绘制文字。
pub fn draw_centered(painter: &Painter, rect: Rect, role: TextRole, text: &str, color: Color32) {
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        text,
        role.font_id(),
        color,
    );
}

/// 在 `rect` 左侧绘制 label，右侧绘制 value（用于 KV 行）。
pub fn draw_kv(
    painter: &Painter,
    rect: Rect,
    label: &str,
    value: &str,
    label_color: Color32,
    value_color: Color32,
) {
    painter.text(
        Pos2::new(rect.min.x, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        TextRole::Body.font_id(),
        label_color,
    );
    painter.text(
        Pos2::new(rect.max.x, rect.center().y),
        Align2::RIGHT_CENTER,
        value,
        TextRole::Numeric.font_id(),
        value_color,
    );
}

/// 测量文字所占大小（用于布局回退场景；优先用 GridLayout 显式尺寸）。
pub fn measure_text(painter: &Painter, role: TextRole, text: &str) -> Vec2 {
    let galley = painter
        .ctx()
        .fonts(|f| f.layout_no_wrap(text.to_owned(), role.font_id(), Color32::WHITE));
    galley.size()
}

/// Cheap text-width estimate for fixed-rect custom painters.
///
/// We still prefer real galley measurement when layout is already in egui flow, but most V9
/// primitives paint directly into explicit rects. This keeps labels from spilling over their
/// allocated panel cells without forcing every draw path through font layout.
pub fn approximate_text_width(text: &str, font_size: f32) -> f32 {
    text.chars()
        .map(|ch| glyph_width_factor(ch) * font_size)
        .sum()
}

/// Shrink a font only when the text would overflow `available_w`.
pub fn fit_font_to_width(text: &str, mut font: FontId, available_w: f32, min_scale: f32) -> FontId {
    let available_w = available_w.max(1.0);
    let approx_w = approximate_text_width(text, font.size);
    if approx_w > available_w && approx_w > 0.0 {
        font.size *= (available_w / approx_w).clamp(min_scale.clamp(0.20, 1.0), 1.0);
    }
    font
}

fn glyph_width_factor(ch: char) -> f32 {
    if ch.is_ascii_whitespace() {
        0.34
    } else if ch.is_ascii() {
        match ch {
            'i' | 'l' | 'I' | '1' | '|' | '.' | ',' | ':' | ';' | '!' | '\'' => 0.32,
            'W' | 'M' | '@' | '#' | '%' | '&' => 0.82,
            'm' | 'w' => 0.72,
            _ => 0.56,
        }
    } else {
        0.96
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_role_to_font_id_size_matches() {
        assert_eq!(TextRole::Body.font_id().size, 12.0);
        assert_eq!(TextRole::Title.font_id().size, 32.0);
        assert_eq!(
            TextRole::Numeric.font_id().family,
            egui::FontFamily::Monospace
        );
    }

    #[test]
    fn fit_font_shrinks_long_cjk_labels() {
        let font = fit_font_to_width("建设部门需求", TextRole::Subheading.font_id(), 38.0, 0.60);
        assert!(font.size < TextRole::Subheading.font_id().size);
        assert!(font.size >= TextRole::Subheading.font_id().size * 0.60);
    }

    #[test]
    fn ascii_width_estimate_accounts_for_narrow_glyphs() {
        let wide = approximate_text_width("WWWW", 10.0);
        let narrow = approximate_text_width("iiii", 10.0);
        assert!(wide > narrow);
    }
}
