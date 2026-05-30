//! V9 文本绘制 helper：从 `TextRole` 取 `FontId`，统一调用 `Painter::text`。
//!
//! 调用方禁止裸写 `RichText::size(...)`；必须通过 [`draw_text`] / [`measure_text`]
//! 或 [`TextRole::font_id`] 间接。

use egui::{Align2, Color32, Painter, Pos2, Rect, Vec2};

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
}
