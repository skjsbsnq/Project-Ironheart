//! V5 收口（2026-05-18）：菜单场景类型枚举。
//!
//! V3 时代本模块构造程序化 `GuiNode` 树，喂给 `GuiRt-removed` → `UI-pass-removed` 渲染。
//! V5 放弃 vanilla GUI 路线后，菜单渲染统一走 [`crate::menu_pass`] 的程序化
//! 路径（`PanelPass` 圆角矩形 + `TextPass` 直接绘字），不再需要 `GuiNode`。
//!
//! 本文件仅保留 `MenuKind` 枚举供 `main.rs` 区分主菜单 / 国家选择 / 加载画面。

/// 菜单场景类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuKind {
    MainMenu,
    CountrySelect,
    Loading,
}
