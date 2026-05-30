//! Card primitive。简单卡片容器，把 [`FrameStyle::Card`] 包装成单调用接口。

use egui::{Painter, Rect, Ui};

use crate::v9::{
    frame::{FrameStyle, PanelFrame},
    sound,
};

/// 卡片。提供 `show_at(ui, rect)` 在指定 `Rect` 中绘制外框，调用方再用 `inner_rect`
/// 拿到内部可用区域。
pub struct Card {
    style: FrameStyle,
}

impl Card {
    pub fn new() -> Self {
        Self {
            style: FrameStyle::Card,
        }
    }

    /// 切换为 `Panel` 风格（更厚边框 + 角部 ◆）。
    pub fn as_panel(mut self) -> Self {
        self.style = FrameStyle::Panel;
        self
    }

    /// 切换为 `Glass` 风格（半透 HUD 用）。
    pub fn as_glass(mut self) -> Self {
        self.style = FrameStyle::Glass;
        self
    }

    /// 切换为 `Ornate` 风格（强调区 + 铜钉头）。
    pub fn as_ornate(mut self) -> Self {
        self.style = FrameStyle::Ornate;
        self
    }

    /// 在 `painter` 上以 `rect` 绘制。返回 `inner_rect`，调用方继续在其中布局。
    pub fn draw(&self, painter: &Painter, rect: Rect) -> Rect {
        let frame = PanelFrame::new(self.style, rect);
        frame.draw(painter);
        frame.inner_rect()
    }

    /// 在 `ui` 中分配 `rect`，并绘制外框。
    pub fn show_at(&self, ui: &mut Ui, rect: Rect) -> Rect {
        let inner = self.draw(ui.painter(), rect);
        // 拿 sense::hover() 以便上层 tooltip / cursor 能用
        let response = ui.interact(rect, ui.id().with("card"), egui::Sense::hover());
        sound::hook_response_auto("card", &response, true);
        inner
    }
}

impl Default for Card {
    fn default() -> Self {
        Self::new()
    }
}
