//! V9 设计 token：调色板 / 字号 / 间距 / 圆角 / 描边 / 海拔 / 动效 / Z 层。
//!
//! 所有 v9 视觉常量必须经此模块；调用方不允许裸写 `Color32::from_rgb` /
//! `RichText::size` / 裸数字 spacing。详见 `ROADMAP_V9_UI_FRONTEND_REDESIGN.md` §3。

use std::sync::atomic::{AtomicU32, Ordering};

use egui::{FontFamily, FontId};

static TEXT_SCALE_BITS: AtomicU32 = AtomicU32::new(1.0f32.to_bits());

pub fn set_text_scale(scale: f32) {
    TEXT_SCALE_BITS.store(scale.clamp(1.0, 1.30).to_bits(), Ordering::Relaxed);
}

pub fn text_scale() -> f32 {
    f32::from_bits(TEXT_SCALE_BITS.load(Ordering::Relaxed)).clamp(1.0, 1.30)
}

// ─── Palette ─────────────────────────────────────────────────────

pub mod palette {
    use egui::Color32;

    // Graphite steel base. These replace the old warm wood/parchment foundation.
    pub const CANVAS_DEEP: Color32 = Color32::from_rgb(0x03, 0x04, 0x04);
    pub const CANVAS: Color32 = Color32::from_rgb(0x07, 0x08, 0x08);
    pub const PANEL: Color32 = Color32::from_rgb(0x10, 0x12, 0x11);
    pub const PANEL_SOFT: Color32 = Color32::from_rgb(0x19, 0x1c, 0x1a);
    pub const PANEL_DEEP: Color32 = Color32::from_rgb(0x05, 0x06, 0x06);
    pub const ZEBRA_DARK: Color32 = Color32::from_rgb(0x0b, 0x0d, 0x0c);
    pub const ZEBRA_LIGHT: Color32 = Color32::from_rgb(0x11, 0x13, 0x12);
    pub const HAIRLINE: Color32 = Color32::from_rgb(0x24, 0x26, 0x23);
    pub const STROKE_DARK: Color32 = Color32::from_rgb(0x30, 0x28, 0x19);
    pub const STROKE_MED: Color32 = Color32::from_rgb(0x5b, 0x49, 0x2a);
    pub const PARCHMENT: Color32 = Color32::from_rgb(0xd7, 0xcc, 0xa7);
    pub const PARCHMENT_DIM: Color32 = Color32::from_rgb(0x9f, 0x97, 0x82);
    pub const MUTED: Color32 = Color32::from_rgb(0x73, 0x76, 0x70);

    // Dark iron material tokens for the reference-2 HUD style.
    pub const SOOT_BLACK: Color32 = Color32::from_rgb(0x01, 0x02, 0x02);
    pub const OIL_BLACK: Color32 = Color32::from_rgb(0x06, 0x07, 0x06);
    pub const IRON_BLACK: Color32 = Color32::from_rgb(0x03, 0x04, 0x04);
    pub const IRON_DARK: Color32 = Color32::from_rgb(0x09, 0x0b, 0x0b);
    pub const IRON: Color32 = Color32::from_rgb(0x15, 0x18, 0x16);
    pub const IRON_LIGHT: Color32 = Color32::from_rgb(0x2c, 0x30, 0x2b);
    pub const GUNMETAL: Color32 = Color32::from_rgb(0x3e, 0x42, 0x3b);
    pub const STEEL_FACE: Color32 = Color32::from_rgb(0x4d, 0x52, 0x49);
    pub const EDGE_LIGHT: Color32 = Color32::from_rgb(0x80, 0x72, 0x53);
    pub const EDGE_DARK: Color32 = Color32::from_rgb(0x12, 0x0f, 0x09);
    pub const RUST: Color32 = Color32::from_rgb(0x5a, 0x2d, 0x20);

    // Muted brass / old gold.
    pub const BRASS_SHADOW: Color32 = Color32::from_rgb(0x2f, 0x24, 0x14);
    pub const BRASS_DARK: Color32 = Color32::from_rgb(0x43, 0x32, 0x1d);
    pub const BRASS: Color32 = Color32::from_rgb(0x72, 0x58, 0x2e);
    pub const BRASS_WORN: Color32 = Color32::from_rgb(0x9b, 0x7a, 0x3f);
    pub const BRASS_BRIGHT: Color32 = Color32::from_rgb(0xb0, 0x8a, 0x43);
    pub const GOLD: Color32 = Color32::from_rgb(0xcc, 0xa2, 0x49);
    pub const GOLD_HOT: Color32 = Color32::from_rgb(0xe3, 0xc1, 0x66);

    // 派系
    pub const IDEO_FASCISM: Color32 = Color32::from_rgb(0x9e, 0x32, 0x32);
    pub const IDEO_DEMOCRATIC: Color32 = Color32::from_rgb(0x2e, 0x60, 0xa8);
    pub const IDEO_COMMUNISM: Color32 = Color32::from_rgb(0xb0, 0x2a, 0x2a);
    pub const IDEO_NEUTRALITY: Color32 = Color32::from_rgb(0xc0, 0x8a, 0x3e);

    // 冷战派系点缀
    pub const COLD_STEEL: Color32 = Color32::from_rgb(0x5a, 0x66, 0x75);
    pub const COLD_SIGNAL: Color32 = Color32::from_rgb(0xd0, 0x40, 0x30);
    pub const COLD_ATOMIC: Color32 = Color32::from_rgb(0x2c, 0xb4, 0xa8);

    // 状态语义色
    pub const GOOD: Color32 = Color32::from_rgb(0x6c, 0xc0, 0x70);
    pub const WARN: Color32 = Color32::from_rgb(0xf0, 0xb8, 0x50);
    pub const BAD: Color32 = Color32::from_rgb(0xd8, 0x58, 0x4c);
    pub const INFO: Color32 = Color32::from_rgb(0x68, 0x96, 0xc8);

    // 统计色
    pub const STAT_PRIMARY: Color32 = Color32::from_rgb(0xc9, 0xa5, 0x5b);
    pub const STAT_SECONDARY: Color32 = Color32::from_rgb(0x68, 0x96, 0xc8);
    pub const STAT_TERTIARY: Color32 = Color32::from_rgb(0x8a, 0x8a, 0x8a);
}

// ─── Typography ──────────────────────────────────────────────────

/// 文本角色。每个 role 锁定字号 / 字族 / 字重，调用方不得直接传 `size`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextRole {
    /// 主菜单大标题 32px，Serif Bold。
    Title,
    /// 面板 / 模态标题 22px，Serif Bold。
    Display,
    /// 章节标题 / 摘要数值 17px，Serif SemiBold。
    Heading,
    /// tab 标题 / 列表标题 14px，Sans SemiBold。
    Subheading,
    /// 正文 / 列表行 12px，Sans Regular。
    Body,
    /// 次说明 / 单位 11px，Sans Regular。
    Caption,
    /// 角标 / debug 10px，Sans Regular。
    Small,
    /// 表格数值 13px，Mono Medium。
    Numeric,
    /// 调试 ID / 日志 11px，Mono Regular。
    Code,
}

impl TextRole {
    /// 字号（逻辑像素）。
    pub fn base_size(self) -> f32 {
        match self {
            Self::Title => 32.0,
            Self::Display => 22.0,
            Self::Heading => 17.0,
            Self::Subheading => 14.0,
            Self::Body => 12.0,
            Self::Caption => 11.0,
            Self::Small => 10.0,
            Self::Numeric => 13.0,
            Self::Code => 11.0,
        }
    }

    /// 字族。
    pub fn size(self) -> f32 {
        self.base_size() * text_scale()
    }

    pub fn family(self) -> FontFamily {
        match self {
            Self::Numeric | Self::Code => FontFamily::Monospace,
            _ => FontFamily::Proportional,
        }
    }

    /// 转换为 egui `FontId`。
    pub fn font_id(self) -> FontId {
        FontId::new(self.size(), self.family())
    }
}

// ─── Spacing ─────────────────────────────────────────────────────

/// 间距档位（逻辑像素）。所有 `inner_margin` / `add_space` 必须从这里取值。
pub mod spacing {
    pub const S0: f32 = 0.0;
    pub const S1: f32 = 2.0;
    pub const S2: f32 = 4.0;
    pub const S3: f32 = 6.0;
    pub const S4: f32 = 8.0;
    pub const S5: f32 = 12.0;
    pub const S6: f32 = 16.0;
    pub const S7: f32 = 24.0;
    pub const S8: f32 = 32.0;
    pub const S9: f32 = 48.0;
    pub const S10: f32 = 64.0;
}

// ─── Radius ──────────────────────────────────────────────────────

/// 圆角档位。
pub mod radius {
    pub const R0: f32 = 0.0;
    pub const R1: f32 = 2.0;
    pub const R2: f32 = 4.0;
    pub const R3: f32 = 6.0;
    pub const R4: f32 = 8.0;
}

// ─── Border ──────────────────────────────────────────────────────

/// 描边档位。
pub mod border {
    pub const B0: f32 = 0.0;
    pub const B1: f32 = 1.0;
    pub const B2: f32 = 1.5;
    pub const B3: f32 = 2.0;
    pub const B4: f32 = 3.0;
}

// ─── Elevation ───────────────────────────────────────────────────

/// 海拔档位：决定阴影 alpha 与偏移。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Elevation {
    /// 扁平（嵌入式 — 列表行 / chip）。
    E0,
    /// 轻浮（卡片）。
    E1,
    /// 中浮（面板）。
    E2,
    /// 高浮（模态 / 弹窗）。
    E3,
    /// 最高（全屏弹窗 / 错误对话框）。
    E4,
}

impl Elevation {
    pub fn shadow_alpha(self) -> f32 {
        match self {
            Self::E0 => 0.0,
            Self::E1 => 0.18,
            Self::E2 => 0.32,
            Self::E3 => 0.48,
            Self::E4 => 0.62,
        }
    }

    pub fn shadow_offset(self) -> f32 {
        match self {
            Self::E0 => 0.0,
            Self::E1 => 4.0,
            Self::E2 => 8.0,
            Self::E3 => 12.0,
            Self::E4 => 16.0,
        }
    }
}

// ─── Motion ──────────────────────────────────────────────────────

/// 动画时长（秒）。
pub mod motion {
    pub const FAST: f32 = 0.080;
    pub const NORMAL: f32 = 0.160;
    pub const SLOW: f32 = 0.280;
    pub const CINEMA: f32 = 0.480;
}

// ─── Z Order ─────────────────────────────────────────────────────

/// Z 层。`order()` 返回数值越大越靠上。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ZLayer {
    Background,
    HudOverlay,
    Panel,
    Tooltip,
    Modal,
    Event,
    Toast,
    FullscreenOverlay,
    DebugOverlay,
}

impl ZLayer {
    pub fn order(self) -> u32 {
        match self {
            Self::Background => 0,
            Self::HudOverlay => 100,
            Self::Panel => 200,
            Self::Tooltip => 300,
            Self::Modal => 400,
            Self::Event => 500,
            Self::Toast => 600,
            Self::FullscreenOverlay => 700,
            Self::DebugOverlay => 1000,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_roles_have_correct_sizes() {
        assert_eq!(TextRole::Title.size(), 32.0);
        assert_eq!(TextRole::Body.size(), 12.0);
        assert_eq!(TextRole::Small.size(), 10.0);
    }

    #[test]
    fn mono_roles_use_monospace_family() {
        assert_eq!(TextRole::Numeric.family(), FontFamily::Monospace);
        assert_eq!(TextRole::Code.family(), FontFamily::Monospace);
        assert_eq!(TextRole::Body.family(), FontFamily::Proportional);
    }

    #[test]
    fn elevation_monotone() {
        let levels = [
            Elevation::E0,
            Elevation::E1,
            Elevation::E2,
            Elevation::E3,
            Elevation::E4,
        ];
        for w in levels.windows(2) {
            assert!(w[0].shadow_alpha() < w[1].shadow_alpha());
            assert!(w[0].shadow_offset() < w[1].shadow_offset());
        }
    }

    #[test]
    fn spacing_strictly_increases() {
        use spacing::*;
        let scale = [S0, S1, S2, S3, S4, S5, S6, S7, S8, S9, S10];
        for w in scale.windows(2) {
            assert!(w[0] < w[1]);
        }
    }

    #[test]
    fn z_order_strictly_increases() {
        let layers = [
            ZLayer::Background,
            ZLayer::HudOverlay,
            ZLayer::Panel,
            ZLayer::Tooltip,
            ZLayer::Modal,
            ZLayer::Event,
            ZLayer::Toast,
            ZLayer::FullscreenOverlay,
            ZLayer::DebugOverlay,
        ];
        for w in layers.windows(2) {
            assert!(w[0].order() < w[1].order());
        }
    }
}
