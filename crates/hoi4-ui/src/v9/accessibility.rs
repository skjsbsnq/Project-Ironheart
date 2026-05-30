//! V9 accessibility controls.

use egui::{Color32, Context, FontFamily, FontId, Id, Stroke, TextStyle};

use crate::{
    i18n::{current_language, Language},
    v9::tokens::{self, palette, TextRole},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorBlindMode {
    Off,
    Deuteranopia,
    Protanopia,
    Tritanopia,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontScale {
    Normal,
    Large,
    ExtraLarge,
}

impl FontScale {
    pub fn factor(self) -> f32 {
        match self {
            Self::Normal => 1.0,
            Self::Large => 1.15,
            Self::ExtraLarge => 1.30,
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Self::Normal => "1.00",
            Self::Large => "1.15",
            Self::ExtraLarge => "1.30",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Normal => "x1.00",
            Self::Large => "x1.15",
            Self::ExtraLarge => "x1.30",
        }
    }

    pub fn from_code(code: &str) -> Self {
        match code.trim() {
            "1.15" | "large" | "Large" => Self::Large,
            "1.30" | "extra_large" | "ExtraLarge" => Self::ExtraLarge,
            _ => Self::Normal,
        }
    }

    pub fn all() -> &'static [Self] {
        &[Self::Normal, Self::Large, Self::ExtraLarge]
    }
}

impl ColorBlindMode {
    pub fn code(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Deuteranopia => "deuteranopia",
            Self::Protanopia => "protanopia",
            Self::Tritanopia => "tritanopia",
        }
    }

    pub fn label(self) -> &'static str {
        match (current_language(), self) {
            (Language::Chinese, Self::Off) => "关闭",
            (Language::Chinese, Self::Deuteranopia) => "绿色弱辅助",
            (Language::Chinese, Self::Protanopia) => "红色弱辅助",
            (Language::Chinese, Self::Tritanopia) => "蓝黄色弱辅助",
            (_, Self::Off) => "Off",
            (_, Self::Deuteranopia) => "Deuteranopia",
            (_, Self::Protanopia) => "Protanopia",
            (_, Self::Tritanopia) => "Tritanopia",
        }
    }

    pub fn from_code(code: &str) -> Self {
        match code.trim().to_ascii_lowercase().as_str() {
            "deuteranopia" | "deuter" => Self::Deuteranopia,
            "protanopia" | "protan" => Self::Protanopia,
            "tritanopia" | "tritan" => Self::Tritanopia,
            _ => Self::Off,
        }
    }

    pub fn all() -> &'static [Self] {
        &[
            Self::Off,
            Self::Deuteranopia,
            Self::Protanopia,
            Self::Tritanopia,
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemanticTone {
    Neutral,
    Good,
    Warn,
    Bad,
    Info,
    Fascism,
    Democratic,
    Communism,
    Neutrality,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShapeMarker {
    None,
    Plus,
    Triangle,
    Cross,
    Circle,
    Chevron,
    Square,
    Diamond,
    Slash,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AccessibleStyle {
    pub color: Color32,
    pub marker: ShapeMarker,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AccessibilitySettings {
    pub color_blind_mode: ColorBlindMode,
    pub font_scale: FontScale,
}

impl Default for AccessibilitySettings {
    fn default() -> Self {
        Self {
            color_blind_mode: ColorBlindMode::Off,
            font_scale: FontScale::Normal,
        }
    }
}

pub fn set_settings(ctx: &Context, settings: AccessibilitySettings) {
    ctx.data_mut(|data| data.insert_persisted(settings_id(), settings));
    tokens::set_text_scale(settings.font_scale.factor());
    apply_font_scale(ctx, settings.font_scale);
}

pub fn settings(ctx: &Context) -> AccessibilitySettings {
    ctx.data_mut(|data| data.get_persisted(settings_id()).unwrap_or_default())
}

pub fn scaled_font_id(ctx: &Context, role: TextRole) -> egui::FontId {
    let mut font = role.font_id();
    font.size *= settings(ctx).font_scale.factor();
    font
}

pub fn style_for(ctx: &Context, tone: SemanticTone) -> AccessibleStyle {
    style_for_settings(settings(ctx), tone)
}

pub fn style_for_settings(settings: AccessibilitySettings, tone: SemanticTone) -> AccessibleStyle {
    let color = match tone {
        SemanticTone::Neutral => palette::PARCHMENT_DIM,
        SemanticTone::Good => palette::GOOD,
        SemanticTone::Warn => palette::WARN,
        SemanticTone::Bad => palette::BAD,
        SemanticTone::Info => palette::INFO,
        SemanticTone::Fascism => palette::IDEO_FASCISM,
        SemanticTone::Democratic => palette::IDEO_DEMOCRATIC,
        SemanticTone::Communism => palette::IDEO_COMMUNISM,
        SemanticTone::Neutrality => palette::IDEO_NEUTRALITY,
    };
    let marker = match settings.color_blind_mode {
        ColorBlindMode::Off => ShapeMarker::None,
        _ => marker_for(tone),
    };
    AccessibleStyle {
        color: remap_color(settings.color_blind_mode, color),
        marker,
    }
}

pub fn semantic_tone_for_color(color: Color32) -> Option<SemanticTone> {
    match color {
        c if c == palette::GOOD => Some(SemanticTone::Good),
        c if c == palette::WARN => Some(SemanticTone::Warn),
        c if c == palette::BAD => Some(SemanticTone::Bad),
        c if c == palette::INFO => Some(SemanticTone::Info),
        c if c == palette::IDEO_FASCISM => Some(SemanticTone::Fascism),
        c if c == palette::IDEO_DEMOCRATIC => Some(SemanticTone::Democratic),
        c if c == palette::IDEO_COMMUNISM => Some(SemanticTone::Communism),
        c if c == palette::IDEO_NEUTRALITY => Some(SemanticTone::Neutrality),
        _ => None,
    }
}

pub fn paint_marker(ui: &mut egui::Ui, rect: egui::Rect, marker: ShapeMarker, color: Color32) {
    paint_marker_with_painter(ui.painter(), rect, marker, color);
}

pub fn paint_marker_with_painter(
    painter: &egui::Painter,
    rect: egui::Rect,
    marker: ShapeMarker,
    color: Color32,
) {
    use egui::{Pos2, Shape};

    let c = rect.center();
    let s = rect.width().min(rect.height()) * 0.36;
    let stroke = Stroke::new(1.4, color);
    match marker {
        ShapeMarker::None => {}
        ShapeMarker::Plus => {
            painter.line_segment([Pos2::new(c.x - s, c.y), Pos2::new(c.x + s, c.y)], stroke);
            painter.line_segment([Pos2::new(c.x, c.y - s), Pos2::new(c.x, c.y + s)], stroke);
        }
        ShapeMarker::Triangle => {
            painter.add(Shape::convex_polygon(
                vec![
                    Pos2::new(c.x, c.y - s),
                    Pos2::new(c.x + s, c.y + s),
                    Pos2::new(c.x - s, c.y + s),
                ],
                Color32::TRANSPARENT,
                stroke,
            ));
        }
        ShapeMarker::Cross => {
            painter.line_segment(
                [Pos2::new(c.x - s, c.y - s), Pos2::new(c.x + s, c.y + s)],
                stroke,
            );
            painter.line_segment(
                [Pos2::new(c.x + s, c.y - s), Pos2::new(c.x - s, c.y + s)],
                stroke,
            );
        }
        ShapeMarker::Circle => {
            painter.circle_stroke(c, s, stroke);
        }
        ShapeMarker::Chevron => {
            painter.line_segment([Pos2::new(c.x - s, c.y - s), Pos2::new(c.x, c.y)], stroke);
            painter.line_segment([Pos2::new(c.x, c.y), Pos2::new(c.x - s, c.y + s)], stroke);
        }
        ShapeMarker::Square => {
            painter.rect_stroke(rect.shrink(s * 0.55), 0.0, stroke, egui::StrokeKind::Inside);
        }
        ShapeMarker::Diamond => {
            painter.add(Shape::convex_polygon(
                vec![
                    Pos2::new(c.x, c.y - s),
                    Pos2::new(c.x + s, c.y),
                    Pos2::new(c.x, c.y + s),
                    Pos2::new(c.x - s, c.y),
                ],
                Color32::TRANSPARENT,
                stroke,
            ));
        }
        ShapeMarker::Slash => {
            painter.line_segment(
                [Pos2::new(c.x - s, c.y + s), Pos2::new(c.x + s, c.y - s)],
                stroke,
            );
        }
    }
}

fn apply_font_scale(ctx: &Context, scale: FontScale) {
    let factor = scale.factor();
    ctx.all_styles_mut(|style| {
        style.text_styles.insert(
            TextStyle::Small,
            FontId::new(10.0 * factor, FontFamily::Proportional),
        );
        style.text_styles.insert(
            TextStyle::Body,
            FontId::new(12.0 * factor, FontFamily::Proportional),
        );
        style.text_styles.insert(
            TextStyle::Button,
            FontId::new(12.0 * factor, FontFamily::Proportional),
        );
        style.text_styles.insert(
            TextStyle::Heading,
            FontId::new(17.0 * factor, FontFamily::Proportional),
        );
        style.text_styles.insert(
            TextStyle::Monospace,
            FontId::new(13.0 * factor, FontFamily::Monospace),
        );
    });
}

fn marker_for(tone: SemanticTone) -> ShapeMarker {
    match tone {
        SemanticTone::Neutral => ShapeMarker::Circle,
        SemanticTone::Good => ShapeMarker::Plus,
        SemanticTone::Warn => ShapeMarker::Triangle,
        SemanticTone::Bad => ShapeMarker::Cross,
        SemanticTone::Info => ShapeMarker::Square,
        SemanticTone::Fascism => ShapeMarker::Chevron,
        SemanticTone::Democratic => ShapeMarker::Circle,
        SemanticTone::Communism => ShapeMarker::Diamond,
        SemanticTone::Neutrality => ShapeMarker::Slash,
    }
}

fn remap_color(mode: ColorBlindMode, color: Color32) -> Color32 {
    match mode {
        ColorBlindMode::Off => color,
        ColorBlindMode::Deuteranopia => Color32::from_rgb(
            ((color.r() as u16 + color.g() as u16) / 2) as u8,
            ((color.r() as u16 + color.g() as u16) / 2) as u8,
            color.b(),
        ),
        ColorBlindMode::Protanopia => Color32::from_rgb(
            color.g(),
            color.g(),
            ((color.r() as u16 + color.b() as u16) / 2) as u8,
        ),
        ColorBlindMode::Tritanopia => Color32::from_rgb(
            color.r(),
            ((color.g() as u16 + color.b() as u16) / 2) as u8,
            ((color.g() as u16 + color.b() as u16) / 2) as u8,
        ),
    }
}

fn settings_id() -> Id {
    Id::new("v9_accessibility_settings")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn font_scale_values_match_phase_h() {
        assert_eq!(FontScale::Normal.factor(), 1.0);
        assert_eq!(FontScale::Large.factor(), 1.15);
        assert_eq!(FontScale::ExtraLarge.factor(), 1.30);
    }

    #[test]
    fn font_scale_codes_round_trip() {
        for scale in FontScale::all() {
            assert_eq!(FontScale::from_code(scale.code()), *scale);
        }
    }

    #[test]
    fn color_blind_mode_codes_round_trip() {
        for mode in ColorBlindMode::all() {
            assert_eq!(ColorBlindMode::from_code(mode.code()), *mode);
        }
    }

    #[test]
    fn color_blind_mode_adds_shape_markers() {
        let settings = AccessibilitySettings {
            color_blind_mode: ColorBlindMode::Deuteranopia,
            font_scale: FontScale::Normal,
        };
        assert_ne!(
            style_for_settings(settings, SemanticTone::Fascism).marker,
            style_for_settings(settings, SemanticTone::Democratic).marker
        );
        assert_ne!(
            style_for_settings(settings, SemanticTone::Good).marker,
            ShapeMarker::None
        );
    }
}
