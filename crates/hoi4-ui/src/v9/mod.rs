//! V9 自研前端模块。
//!
//! 路线见 `ROADMAP_V9_UI_FRONTEND_REDESIGN.md`。
//!
//! ## 层次
//!
//! - [`tokens`] — 设计常量（调色板 / 字号 / 间距 / 圆角 / 描边 / 海拔 / 动效 / Z 层）
//! - [`layout`] — 布局元件（GridLayout / SplitLayout / AnchorLayout）
//! - [`paint`] — 程序化绘制（draw_frame + 纹理 helper）
//! - [`frame`] — `PanelFrame` + `FrameStyle` 枚举
//! - [`text`] — 文本档位封装
//! - [`primitives`] — 30 个 UI 元件（Phase A 覆盖 button/card/tile/tabs/list）
//! - [`demo`] — F12 切换的演示页
//!
//! ## 使用约定
//!
//! - 调用方禁止裸写 `Color32::from_rgb`、`RichText::size`、`ui.horizontal/vertical`。
//! - 所有尺寸来自 `tokens::spacing` / `radius` / `border`，禁止裸数字。
//! - 面板内的子项布局走 `layout::GridLayout::cell`，不走 egui 流式排版。

pub mod accessibility;
pub mod composites;
pub mod data_table;
pub mod demo;
pub mod frame;
pub mod icons;
pub mod layout;
pub mod motion;
pub mod nato_icon;
pub mod paint;
pub mod primitives;
pub mod profiler;
pub mod sound;
pub mod text;
pub mod tokens;

pub use frame::{FrameStyle, PanelFrame};
pub use layout::{AnchorLayout, GridLayout, SplitAxis, SplitLayout, Track};
pub use text::{draw_text, TextRole};
pub use tokens::{border, palette, radius, spacing, Elevation, ZLayer};
