//! 通用 UI 组件：调色板、面板标题、章节卡片、metric tile、状态横幅、tab chip
//! 以及空状态 / 摘要条 / 操作按钮。
//!
//! 所有共享视觉 helpers 集中于此，面板源码不要再各自重复 PANEL_CARD / STROKE_DARK
//! 之类的常量；通过 `components::PANEL_CARD` 等方式引用，统一全游戏风格。

use egui::{Color32, RichText, Sense};

// ─── 调色板（vanilla HoI4 金棕 + Vic3 报表色阶） ─────────────────

pub const GOLD: Color32 = Color32::from_rgb(0xc9, 0xa5, 0x5b);
pub const GOLD_BRIGHT: Color32 = Color32::from_rgb(0xe0, 0xc0, 0x78);
pub const GOLD_DIM: Color32 = Color32::from_rgb(0x8b, 0x6f, 0x3e);
pub const BRONZE: Color32 = Color32::from_rgb(0x5a, 0x44, 0x2c);
pub const PARCHMENT: Color32 = Color32::from_rgb(0xe0, 0xd2, 0xa8);
pub const MUTED: Color32 = Color32::from_gray(155);

pub const PANEL_CARD: Color32 = Color32::from_rgb(0x24, 0x1a, 0x12);
pub const PANEL_CARD_SOFT: Color32 = Color32::from_rgb(0x31, 0x24, 0x18);
pub const PANEL_CARD_DEEP: Color32 = Color32::from_rgb(0x1a, 0x12, 0x0a);
pub const HERO_FILL: Color32 = Color32::from_rgb(0x16, 0x0e, 0x08);
pub const ZEBRA_DARK: Color32 = Color32::from_rgb(0x28, 0x1d, 0x14);
pub const ZEBRA_LIGHT: Color32 = Color32::from_rgb(0x2e, 0x22, 0x18);
pub const STROKE_TILE: Color32 = Color32::from_rgb(0x48, 0x36, 0x24);

pub const GOOD: Color32 = Color32::from_rgb(0x70, 0xc8, 0x78);
pub const WARN: Color32 = Color32::from_rgb(0xff, 0xc0, 0x60);
pub const BAD: Color32 = Color32::from_rgb(0xe0, 0x60, 0x58);
pub const BLUE: Color32 = Color32::from_rgb(0x68, 0xa0, 0xd8);

// 旧别名 — situation_panel 等仍在使用，保留兼容。
pub const WARNING: Color32 = WARN;
pub const DANGER: Color32 = BAD;
pub const SUCCESS: Color32 = GOOD;

// ─── 面板顶部 ──────────────────────────────────────────────────

pub fn panel_header(ui: &mut egui::Ui, title: &str, close: &mut bool) {
    ui.horizontal(|ui| {
        let title_width = (ui.available_width() - 34.0).max(120.0);
        ui.add_sized(
            egui::vec2(title_width, 0.0),
            egui::Label::new(RichText::new(title).color(GOLD_BRIGHT).strong().size(20.0)).wrap(),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add(
                    egui::Button::new(RichText::new("✕").color(PARCHMENT).strong().size(14.0))
                        .min_size(egui::vec2(26.0, 22.0)),
                )
                .clicked()
            {
                *close = true;
            }
        });
    });
    // 顶部金棕分隔线
    let rect = ui.max_rect();
    let y = ui.cursor().top() - 2.0;
    ui.painter().hline(
        rect.left()..=rect.right(),
        y,
        egui::Stroke::new(1.0, GOLD_DIM),
    );
    ui.add_space(2.0);
}

pub fn panel_hint(ui: &mut egui::Ui, text: impl Into<String>) {
    ui.label(RichText::new(text.into()).size(11.0).color(MUTED));
}

// ─── 章节卡片（升级版 — 兼容旧 section 调用） ────────────────────

/// 主要章节卡片：金棕外框 + ◆ 居中压花标题 + 分隔线 + 内容。
///
/// 这是 `components::section` 的新实现，所有原调用点自动受益。
pub fn section(ui: &mut egui::Ui, title: &str, add_contents: impl FnOnce(&mut egui::Ui)) {
    section_card(ui, title, add_contents);
}

pub fn section_card(ui: &mut egui::Ui, title: &str, add_contents: impl FnOnce(&mut egui::Ui)) {
    ui.add_space(6.0);
    egui::Frame::new()
        .fill(PANEL_CARD)
        .stroke(egui::Stroke::new(1.0, BRONZE))
        .inner_margin(egui::Margin {
            left: 12,
            right: 12,
            top: 7,
            bottom: 10,
        })
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.vertical_centered(|ui| {
                ui.label(
                    RichText::new(format!("◆  {}  ◆", title))
                        .strong()
                        .color(GOLD_BRIGHT)
                        .size(14.0),
                );
            });
            ui.add_space(2.0);
            ui.separator();
            ui.add_space(2.0);
            add_contents(ui);
        });
}

/// 子卡片（无 ◆ 装饰，左对齐金色标题），适合在 section 内部嵌套使用。
pub fn subcard(ui: &mut egui::Ui, title: &str, add_contents: impl FnOnce(&mut egui::Ui)) {
    ui.add_space(4.0);
    egui::Frame::new()
        .fill(PANEL_CARD_SOFT)
        .stroke(egui::Stroke::new(1.0, STROKE_TILE))
        .inner_margin(egui::Margin::symmetric(10, 7))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(title).strong().color(GOLD).size(12.0));
            ui.add_space(2.0);
            add_contents(ui);
        });
}

// ─── 状态横幅 ──────────────────────────────────────────────────

/// Vic3 风格状态横幅：左侧 4px 实色块 + 半透暗底 + 强调色标题 + 小字描述。
pub fn status_banner(ui: &mut egui::Ui, accent: Color32, title: &str, body: &str) {
    let inner = egui::Frame::new()
        .fill(Color32::from_rgba_premultiplied(0x18, 0x12, 0x0c, 235))
        .stroke(egui::Stroke::new(1.0, accent))
        .inner_margin(egui::Margin {
            left: 14,
            right: 12,
            top: 8,
            bottom: 8,
        })
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.add(egui::Label::new(RichText::new(title).strong().color(accent).size(13.0)).wrap());
            if !body.is_empty() {
                ui.add_space(2.0);
                ui.add(egui::Label::new(RichText::new(body).size(12.0).color(PARCHMENT)).wrap());
            }
        });
    let rect = inner.response.rect;
    let bar = egui::Rect::from_min_size(rect.left_top(), egui::vec2(4.0, rect.height()));
    ui.painter().rect_filled(bar, 0.0, accent);
}

// ─── Metric / KPI / 行 ─────────────────────────────────────────

/// 标准 metric tile：上小灰标签 + 下大粗体数值 + 左侧 3px 强调色条。
pub fn metric_tile(ui: &mut egui::Ui, label: &str, value: impl Into<String>, color: Color32) {
    let value = value.into();
    let inner = egui::Frame::new()
        .fill(PANEL_CARD_DEEP)
        .stroke(egui::Stroke::new(1.0, STROKE_TILE))
        .inner_margin(egui::Margin {
            left: 12,
            right: 10,
            top: 7,
            bottom: 7,
        })
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(label).size(11.0).color(MUTED));
            ui.add_space(2.0);
            ui.label(RichText::new(value).strong().size(17.0).color(color));
        });
    let rect = inner.response.rect;
    let bar = egui::Rect::from_min_size(
        rect.left_top() + egui::vec2(1.0, 1.0),
        egui::vec2(3.0, rect.height() - 2.0),
    );
    ui.painter().rect_filled(bar, 0.0, color);
}

/// 含描述的 tile（标题/数值/小字描述），左侧强调色条。
pub fn pressure_tile(
    ui: &mut egui::Ui,
    title: &str,
    value: impl Into<String>,
    desc: &str,
    color: Color32,
) {
    let value = value.into();
    let inner = egui::Frame::new()
        .fill(PANEL_CARD_SOFT)
        .stroke(egui::Stroke::new(1.0, STROKE_TILE))
        .inner_margin(egui::Margin {
            left: 12,
            right: 10,
            top: 7,
            bottom: 7,
        })
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(title).strong().size(12.0).color(color));
            ui.label(RichText::new(value).strong().size(16.0).color(color));
            ui.label(RichText::new(desc).size(10.0).color(MUTED));
        });
    let rect = inner.response.rect;
    let bar = egui::Rect::from_min_size(
        rect.left_top() + egui::vec2(1.0, 1.0),
        egui::vec2(3.0, rect.height() - 2.0),
    );
    ui.painter().rect_filled(bar, 0.0, color);
}

/// Hero 顶部小型 KPI（居中标签 + 居中数值）。
pub fn hero_kpi(ui: &mut egui::Ui, label: &str, value: impl Into<String>, color: Color32) {
    let value = value.into();
    egui::Frame::new()
        .fill(PANEL_CARD)
        .stroke(egui::Stroke::new(1.0, BRONZE))
        .inner_margin(egui::Margin::symmetric(8, 6))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.vertical_centered(|ui| {
                ui.label(RichText::new(label).size(10.0).color(MUTED));
                ui.add_space(1.0);
                ui.label(RichText::new(value).strong().size(15.0).color(color));
            });
        });
}

/// 左标签 / 右值的小行（用于详细字段）。
pub fn kv_row(ui: &mut egui::Ui, label: &str, value: impl Into<String>, color: Color32) {
    let value = value.into();
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).size(12.0).color(MUTED));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new(value).size(12.0).color(color).strong());
        });
    });
}

/// 斑马纹列表行：交替深浅底色 + 左标签 + 右值。
pub fn zebra_row(
    ui: &mut egui::Ui,
    idx: usize,
    label: &str,
    value: impl Into<String>,
    value_color: Color32,
) {
    let value = value.into();
    let fill = if idx % 2 == 0 {
        ZEBRA_DARK
    } else {
        ZEBRA_LIGHT
    };
    egui::Frame::new()
        .fill(fill)
        .inner_margin(egui::Margin {
            left: 10,
            right: 10,
            top: 3,
            bottom: 3,
        })
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new(label).size(12.0).color(PARCHMENT));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new(value).size(12.0).color(value_color).strong());
                });
            });
        });
}

/// 合计行（金棕边框 + 加粗）。
pub fn total_row(ui: &mut egui::Ui, label: &str, sum: impl Into<String>, ok: bool) {
    let sum = sum.into();
    let value_color = if ok { GOLD_BRIGHT } else { BAD };
    egui::Frame::new()
        .fill(PANEL_CARD_DEEP)
        .stroke(egui::Stroke::new(1.0, GOLD_DIM))
        .inner_margin(egui::Margin {
            left: 10,
            right: 10,
            top: 4,
            bottom: 4,
        })
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new(label).size(12.0).color(GOLD).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new(sum).size(13.0).color(value_color).strong());
                });
            });
        });
}

// ─── Tab Bar ───────────────────────────────────────────────────

/// 单个 tab chip：激活态金色边框 + 亮金底部下划线。返回是否被点击。
pub fn tab_chip(ui: &mut egui::Ui, active: bool, label: &str) -> bool {
    let fill = if active {
        PANEL_CARD_SOFT
    } else {
        PANEL_CARD_DEEP
    };
    let stroke_color = if active { GOLD } else { BRONZE };
    let text_color = if active { GOLD_BRIGHT } else { PARCHMENT };

    let inner = egui::Frame::new()
        .fill(fill)
        .stroke(egui::Stroke::new(1.0, stroke_color))
        .inner_margin(egui::Margin::symmetric(14, 6))
        .show(ui, |ui| {
            ui.label(RichText::new(label).size(13.0).color(text_color).strong());
        });

    let resp = ui.interact(inner.response.rect, inner.response.id, Sense::click());
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    if active {
        let rect = inner.response.rect;
        let y = rect.bottom() - 1.5;
        ui.painter().hline(
            (rect.left() + 3.0)..=(rect.right() - 3.0),
            y,
            egui::Stroke::new(2.0, GOLD_BRIGHT),
        );
    }
    resp.clicked()
}

// ─── 装饰分隔 ──────────────────────────────────────────────────

/// 金棕 ◆ 居中装饰横线（用于 hero 内分隔）。
pub fn ornament_divider(ui: &mut egui::Ui) {
    let avail = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(avail, 10.0), Sense::hover());
    let painter = ui.painter();
    let cy = rect.center().y;
    let gap = 12.0;
    painter.hline(
        rect.left()..=(rect.center().x - gap),
        cy,
        egui::Stroke::new(1.0, GOLD_DIM),
    );
    painter.hline(
        (rect.center().x + gap)..=rect.right(),
        cy,
        egui::Stroke::new(1.0, GOLD_DIM),
    );
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        "◆",
        egui::FontId::proportional(11.0),
        GOLD,
    );
}

// ─── 其余原有 helpers（已升级） ──────────────────────────────────

pub fn empty_state(ui: &mut egui::Ui, title: &str, hint: &str) {
    egui::Frame::new()
        .fill(PANEL_CARD_DEEP)
        .stroke(egui::Stroke::new(1.0, STROKE_TILE))
        .inner_margin(egui::Margin::symmetric(14, 12))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.vertical_centered(|ui| {
                ui.label(RichText::new("◌").size(20.0).color(BRONZE));
                ui.add_space(2.0);
                ui.label(RichText::new(title).strong().color(MUTED).size(13.0));
                if !hint.is_empty() {
                    ui.add_space(2.0);
                    ui.label(RichText::new(hint).size(11.0).color(MUTED));
                }
            });
        });
}

pub fn warning_pill(ui: &mut egui::Ui, text: &str) {
    egui::Frame::new()
        .fill(Color32::from_rgba_premultiplied(0x2a, 0x1d, 0x0c, 220))
        .stroke(egui::Stroke::new(1.0, WARN))
        .inner_margin(egui::Margin::symmetric(8, 3))
        .show(ui, |ui| {
            ui.label(RichText::new(text).size(11.0).strong().color(WARN));
        });
}

/// 升级版：胶囊条由暗底卡片 + 金值替换原 `ui.group` 默认外观。
pub fn summary_strip(ui: &mut egui::Ui, items: &[(&str, String)]) {
    ui.horizontal_wrapped(|ui| {
        for (label, value) in items {
            egui::Frame::new()
                .fill(PANEL_CARD_DEEP)
                .stroke(egui::Stroke::new(1.0, BRONZE))
                .inner_margin(egui::Margin::symmetric(9, 4))
                .show(ui, |ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new(*label).size(10.0).color(MUTED));
                        ui.label(RichText::new(value).size(13.0).strong().color(GOLD_BRIGHT));
                    });
                });
        }
    });
}

/// 操作按钮：金棕风格、强制 32px 高、金色文字。
pub fn action_button(ui: &mut egui::Ui, enabled: bool, label: &str) -> egui::Response {
    let text = RichText::new(label).color(GOLD).strong().size(13.0);
    ui.add_enabled(
        enabled,
        egui::Button::new(text).min_size(egui::vec2(120.0, 30.0)),
    )
}

/// 操作按钮（自定义强调色），适合危险/中性/英镑等按钮。
pub fn action_button_colored(
    ui: &mut egui::Ui,
    enabled: bool,
    label: &str,
    accent: Color32,
) -> egui::Response {
    let text = RichText::new(label).color(accent).strong().size(13.0);
    ui.add_enabled(
        enabled,
        egui::Button::new(text).min_size(egui::vec2(120.0, 30.0)),
    )
}
