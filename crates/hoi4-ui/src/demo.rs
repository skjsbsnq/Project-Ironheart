//! UI demo 场景 — V5 阶段 B.6。
//!
//! 一个独立、自包含的 demo Window：用 TabBar 把 4 页子（Buttons / Labels /
//! List / Slider）拍平到 5 个 vanilla 调性必要控件类型上，配上 9-slice 木纹
//! 背景、vanilla theme 的暖金按钮三态。`hoi4-app::main` 通过 F2 toggle 显示。
//!
//! ## 5 个控件覆盖
//!
//! 1. **Button**（Buttons 页）—— 带 hover / pressed / 计数器
//! 2. **Label**（Labels 页）—— heading / body / monospace / 中英文混排
//! 3. **List**（List 页）—— `selectable_label` 列表 + 选中状态
//! 4. **Slider**（Slider 页）—— `Slider` + `DragValue` + `ProgressBar`
//! 5. **TabBar**（顶部）—— 页签切换本身计为第 5 个控件
//!
//! ## 状态字段
//!
//! 所有交互结果都缓存在 [`DemoWindow`] 字段，保证窗口隐藏 / 重开后状态保留。
//!
//! ## 用法
//!
//! ```ignore
//! // App 字段
//! demo: hoi4_ui::demo::DemoWindow,
//! demo_visible: bool,
//!
//! // window_event 中
//! KeyCode::F2 => self.demo_visible = !self.demo_visible;
//!
//! // render begin_frame 闭包内
//! if demo_visible {
//!     demo.show(ctx, nine_slice, icon_bank);
//! }
//! ```

use egui::{Color32, Context, Frame, Margin, ProgressBar, Slider, TextEdit, Window};

use crate::icons::{self, IconBank};
use crate::nine_slice::NineSlice;

/// 4 个页签名（控件 #5：TabBar 本身）。
const TAB_NAMES: &[&str] = &["Buttons", "Labels", "List", "Slider"];

/// demo Window 的全部交互状态。`Default` 给一组中性初值。
pub struct DemoWindow {
    pub tab_idx: usize,
    pub list_idx: usize,
    pub slider: f32,
    pub drag_value: i32,
    pub button_clicks: u32,
    pub text_field: String,
}

impl Default for DemoWindow {
    fn default() -> Self {
        Self {
            tab_idx: 0,
            list_idx: 0,
            slider: 0.45,
            drag_value: 1936,
            button_clicks: 0,
            text_field: "Project Ironheart V5 · 钢铁雄心".to_owned(),
        }
    }
}

impl DemoWindow {
    pub fn new() -> Self {
        Self::default()
    }

    /// 在 `ctx` 上画 demo Window。`nine_slice` 命中时关掉 Frame::fill 走木纹背景；
    /// `icon_bank` 用于 Buttons 页右侧示例 icon；`stats` 是上一帧 perf 探针，
    /// 显示在窗口底部 footer（B.7 性能预算可视化）。
    pub fn show(
        &mut self,
        ctx: &Context,
        nine_slice: Option<&NineSlice>,
        icon_bank: &mut IconBank,
        stats: crate::UiFrameStats,
    ) {
        let edge_px: f32 = nine_slice.map(|ns| ns.edges.left).unwrap_or(0.0);
        let frame = if nine_slice.is_some() {
            Frame::default().inner_margin(Margin::same(edge_px as i8))
        } else {
            Frame::window(&ctx.style())
        };

        Window::new("hoi4-ui demo (B.6) — 5 控件 / 9-slice / 暖金")
            .default_pos([520.0, 80.0]) // 与 B.5 always-on 状态面板错开
            .default_width(440.0)
            .resizable(true)
            .frame(frame)
            .show(ctx, |ui| {
                if let Some(ns) = nine_slice {
                    let outer = ui.max_rect().expand(edge_px);
                    ns.paint(ui.painter(), outer, Color32::WHITE);
                }

                // 控件 #5：TabBar —— 用 selectable_label 横排。
                ui.horizontal(|ui| {
                    for (i, name) in TAB_NAMES.iter().enumerate() {
                        if ui.selectable_label(self.tab_idx == i, *name).clicked() {
                            self.tab_idx = i;
                        }
                    }
                });
                ui.separator();

                match self.tab_idx {
                    0 => self.draw_buttons(ui, icon_bank),
                    1 => self.draw_labels(ui),
                    2 => self.draw_list(ui),
                    3 => self.draw_slider(ui),
                    _ => {}
                }

                // V5 阶段 B.7：perf footer —— 把 last_stats 显示在窗口底部，
                // 超预算（>2 ms）用红色，达标用暖金。
                ui.separator();
                let over = !stats.within_budget();
                let footer_color = if over {
                    Color32::from_rgb(0xff, 0x60, 0x60)
                } else {
                    Color32::from_rgb(0x9f, 0xc1, 0xc8)
                };
                let budget_us = crate::FRAME_BUDGET_US;
                ui.colored_label(
                    footer_color,
                    format!(
                        "perf: total={total} us (begin={b} + tess={t} + bufs={u} + paint={p}) · {prims} prims · {tris} tris · budget={budget} us {tag}",
                        total = stats.total_us,
                        b = stats.begin_us,
                        t = stats.tessellate_us,
                        u = stats.buffers_us,
                        p = stats.paint_us,
                        prims = stats.primitive_count,
                        tris = stats.triangle_count,
                        budget = budget_us,
                        tag = if over { "⚠ OVER" } else { "✅ OK" },
                    ),
                );
            });
    }

    /// 控件 #1：Button —— 三态色（默认 / 悬停 / 按下）+ 点击计数 + 暖金 icon。
    fn draw_buttons(&mut self, ui: &mut egui::Ui, icon_bank: &mut IconBank) {
        ui.label("vanilla theme 的按钮三态：默认 暗木 / 悬停 暖金棕 / 按下 亮金。");
        ui.horizontal(|ui| {
            if ui.button("点击 +1").clicked() {
                self.button_clicks = self.button_clicks.saturating_add(1);
            }
            if ui.button("Reset").clicked() {
                self.button_clicks = 0;
            }
            if ui.button("Quit (no-op)").clicked() {
                println!("[demo] Quit button clicked (no-op in B.6)");
            }
        });
        ui.label(format!("累计点击：{} 次", self.button_clicks));
        ui.separator();
        ui.label("配合 B.5 sprite icon helper：");
        ui.horizontal(|ui| {
            for name in &["GFX_focus_GER_anschluss", "GFX_focus_GER_afrikakorps"] {
                let resp = icons::show_icon(ui, icon_bank, name, Some(egui::vec2(64.0, 56.0)));
                if resp.is_none() {
                    ui.label(format!("⚠ {name} 缺失"));
                }
            }
        });
    }

    /// 控件 #2：Label —— heading / body / monospace + 中英文混排（验证 B.3 字体）。
    fn draw_labels(&mut self, ui: &mut egui::Ui) {
        ui.heading("Heading：钢铁雄心 V5 — 自研 UI");
        ui.label("Body 段：This is a Latin paragraph using Georgia (Garamond-alike).");
        ui.label("Body 段：这是一段中文，验证 msyh.ttc 中文 fallback 正常工作。");
        ui.label(egui::RichText::new("Monospace 段：1936-01-01  PP=180  Stab=70%").monospace());
        ui.separator();
        ui.label("可编辑文本：");
        ui.add(TextEdit::singleline(&mut self.text_field).hint_text("输入任意文本"));
        ui.label(format!(
            "当前长度：{} chars",
            self.text_field.chars().count()
        ));
    }

    /// 控件 #3：List —— `selectable_label` 列表 + ScrollArea + 选中状态。
    fn draw_list(&mut self, ui: &mut egui::Ui) {
        ui.label("可选列表（GER 历史 focus 节点占位）：");
        let entries = [
            "Rhineland",
            "4 Year Plan",
            "Anschluss",
            "Sudetenland",
            "Polish Question",
            "Sea Lion",
            "Barbarossa",
        ];
        egui::ScrollArea::vertical()
            .max_height(160.0)
            .show(ui, |ui| {
                for (i, name) in entries.iter().enumerate() {
                    if ui
                        .selectable_label(self.list_idx == i, format!("[{}] {}", i + 1, name))
                        .clicked()
                    {
                        self.list_idx = i;
                    }
                }
            });
        ui.separator();
        ui.label(format!(
            "已选：{} ({})",
            entries.get(self.list_idx).copied().unwrap_or("?"),
            self.list_idx
        ));
    }

    /// 控件 #4：Slider —— `Slider` + `DragValue` + `ProgressBar`。
    fn draw_slider(&mut self, ui: &mut egui::Ui) {
        ui.label("Slider（0.0 ~ 1.0，步进 0.01）：");
        ui.add(Slider::new(&mut self.slider, 0.0..=1.0).text("Stability 模拟"));
        ui.separator();
        ui.label("DragValue（拖动 / 输入：年份）：");
        ui.add(
            egui::DragValue::new(&mut self.drag_value)
                .range(1900..=1950)
                .speed(0.5),
        );
        ui.separator();
        ui.label("ProgressBar（与 Slider 同步显示）：");
        ui.add(ProgressBar::new(self.slider).show_percentage());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Default 给出合理初值。
    #[test]
    fn default_has_sensible_initial_state() {
        let d = DemoWindow::default();
        assert_eq!(d.tab_idx, 0);
        assert_eq!(d.list_idx, 0);
        assert!((0.0..=1.0).contains(&d.slider));
        assert_eq!(d.button_clicks, 0);
        assert!(d.text_field.contains("Ironheart"));
    }

    /// 4 个 tab 名都不为空（避免 selectable_label 接到空字符串导致点击不响应）。
    #[test]
    fn tab_names_nonempty() {
        for name in TAB_NAMES {
            assert!(!name.is_empty(), "tab name should not be empty");
        }
        assert_eq!(TAB_NAMES.len(), 4);
    }
}
