//! V6 法律面板 UI：6 大类法律切换 + 冷却显示 + PP 消耗。
//!
//! V6.A 验收要求：法律面板出现，能切换冷却显示但 modifier 还没接通。

use crate::{components, i18n::tr};
use egui::{Color32, RichText};

use hoi4_state::LawCategory;

const GOLD: Color32 = Color32::from_rgb(0xc9, 0xa5, 0x5b);
const GOLD_BRIGHT: Color32 = Color32::from_rgb(0xe0, 0xc0, 0x78);
const MUTED: Color32 = Color32::from_gray(155);
const PANEL_CARD: Color32 = Color32::from_rgb(0x24, 0x1a, 0x12);
const STROKE_DARK: Color32 = Color32::from_rgb(0x5a, 0x44, 0x2c);
const GOOD: Color32 = Color32::from_rgb(0x70, 0xc8, 0x78);
const WARN: Color32 = Color32::from_rgb(0xff, 0xc0, 0x60);
const BAD: Color32 = Color32::from_rgb(0xe0, 0x60, 0x58);

fn category_label(cat: &LawCategory) -> &'static str {
    match cat {
        LawCategory::Conscription => tr("v6_law_conscription"),
        LawCategory::Economy => tr("v6_law_economy"),
        LawCategory::Trade => tr("v6_law_trade"),
        LawCategory::Taxation => tr("v6_law_taxation"),
        LawCategory::CivilRights => tr("v6_law_civil_rights"),
        LawCategory::InformationControl => tr("v6_law_information_control"),
    }
}

fn category_color(cat: &LawCategory) -> Color32 {
    match cat {
        LawCategory::Conscription => Color32::from_rgb(0xb0, 0x60, 0x40),
        LawCategory::Economy => Color32::from_rgb(0x60, 0x90, 0xb0),
        LawCategory::Trade => Color32::from_rgb(0x60, 0xa0, 0x60),
        LawCategory::Taxation => Color32::from_rgb(0xc0, 0xa0, 0x30),
        LawCategory::CivilRights => Color32::from_rgb(0x90, 0x70, 0xb0),
        LawCategory::InformationControl => Color32::from_rgb(0xa0, 0x50, 0x50),
    }
}

#[derive(Debug, Clone)]
pub struct LawTierEntry {
    pub id: String,
    pub name: String,
    pub pp_cost: u32,
    pub cooldown_days: u16,
    pub effects: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct LawSlotEntry {
    pub category: LawCategory,
    pub current_id: String,
    pub current_name: String,
    pub cooldown_days: u16,
    pub pending: Option<(String, String, u16)>,
    pub is_locked: bool,
    pub locked_reason: Option<String>,
    pub previous_before_lock: Option<String>,
    pub tiers: Vec<LawTierEntry>,
}

#[derive(Debug, Clone)]
pub struct LawPanelData {
    pub political_power: f32,
    pub slots: Vec<LawSlotEntry>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LawCommand {
    SwitchLaw {
        category: LawCategory,
        target_law_id: String,
    },
}

pub struct LawPanel;

impl LawPanel {
    #[allow(unreachable_code)]
    pub fn show(ctx: &egui::Context, data: &LawPanelData) -> (bool, Vec<LawCommand>) {
        return v9_show_law(ctx, data);

        let mut close = false;
        let mut cmds: Vec<LawCommand> = Vec::new();
        let selected_id = egui::Id::new("law_panel_selected_category");
        let mut selected_category = ctx
            .data_mut(|d| d.get_persisted::<Option<LawCategory>>(selected_id))
            .unwrap_or(None);

        egui::SidePanel::left("law_panel")
            .default_width(540.0)
            .min_width(460.0)
            .resizable(true)
            .show(ctx, |ui| {
                components::panel_header(ui, tr("v6_law_panel_title"), &mut close);
                render_law_summary(ui, data);
                render_law_status_banner(ui, data);

                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        render_law_overview_grid(ui, data, &mut selected_category);
                    });
            });

        if let Some(category) = selected_category {
            if let Some(slot) = data.slots.iter().find(|slot| slot.category == category) {
                render_law_picker_window(
                    ctx,
                    slot,
                    data.political_power,
                    &mut selected_category,
                    &mut cmds,
                );
            } else {
                selected_category = None;
            }
        }

        ctx.data_mut(|d| d.insert_persisted(selected_id, selected_category));

        (close, cmds)
    }
}

fn v9_show_law(ctx: &egui::Context, data: &LawPanelData) -> (bool, Vec<LawCommand>) {
    use crate::v9::composites::panel_shell::{
        draw_summary_tiles, draw_tab_strip, PanelClass, PanelShell,
    };
    use crate::v9::tokens::palette;

    let selected_id = egui::Id::new("law_panel_v9_selected_category");
    let mut selected_category = ctx
        .data_mut(|d| d.get_persisted::<Option<LawCategory>>(selected_id))
        .unwrap_or_else(|| data.slots.first().map(|slot| slot.category));
    if selected_category
        .map(|category| data.slots.iter().any(|slot| slot.category == category))
        .unwrap_or(false)
        == false
    {
        selected_category = data.slots.first().map(|slot| slot.category);
    }

    let locked = data.slots.iter().filter(|slot| slot.is_locked).count();
    let cooling = data
        .slots
        .iter()
        .filter(|slot| slot.cooldown_days > 0)
        .count();
    let pending = data
        .slots
        .iter()
        .filter(|slot| slot.pending.is_some())
        .count();
    let affordable = v9_law_affordable_count(data);
    let accent = if locked > 0 {
        palette::BAD
    } else if pending > 0 || cooling > 0 {
        palette::WARN
    } else {
        palette::GOLD
    };

    let (close, output) = PanelShell::new("law_panel_v9", tr("v6_law_panel_title"))
        .subtitle("法律类别 / 切换影响预览")
        .class(PanelClass::Economy)
        .accent(accent)
        .footer("Q Close  |  Select law group / Switch")
        .show(ctx, |ui, layout| {
            draw_summary_tiles(
                ui,
                layout.summary,
                &[
                    (
                        tr("political_power"),
                        format!("{:.0}", data.political_power),
                        palette::GOLD,
                    ),
                    ("类别", data.slots.len().to_string(), palette::INFO),
                    (
                        "可切换",
                        affordable.to_string(),
                        if affordable > 0 {
                            palette::GOOD
                        } else {
                            palette::MUTED
                        },
                    ),
                    (
                        tr("cooldown"),
                        cooling.to_string(),
                        if cooling > 0 {
                            palette::WARN
                        } else {
                            palette::GOOD
                        },
                    ),
                    (
                        tr("locked"),
                        locked.to_string(),
                        if locked > 0 {
                            palette::BAD
                        } else {
                            palette::GOOD
                        },
                    ),
                    (
                        tr("v6_law_pending"),
                        pending.to_string(),
                        if pending > 0 {
                            palette::WARN
                        } else {
                            palette::MUTED
                        },
                    ),
                ],
            );
            draw_tab_strip(ui, layout.tabs, "法律类别 / 影响预览", accent);
            let mut cmds = Vec::new();
            v9_law_body(ui, layout.body, data, &mut selected_category, &mut cmds);
            cmds
        });

    ctx.data_mut(|d| d.insert_persisted(selected_id, selected_category));
    (close, output.unwrap_or_default())
}

fn v9_law_body(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    data: &LawPanelData,
    selected_category: &mut Option<LawCategory>,
    cmds: &mut Vec<LawCommand>,
) {
    use crate::v9::layout::{GridLayout, Track};
    use crate::v9::tokens::spacing;

    ui.allocate_ui_at_rect(rect, |ui| {
        let grid = GridLayout::new(vec![Track::Fr(1.0)], vec![Track::Fr(0.35), Track::Fr(0.65)])
            .with_gutter(spacing::S5, 0.0);
        let cells = grid.measure(rect);
        v9_law_group_list(ui, GridLayout::cell(&cells, 0, 0), data, selected_category);
        let selected_slot = selected_category
            .and_then(|category| data.slots.iter().find(|slot| slot.category == category))
            .or_else(|| data.slots.first());
        v9_law_impact_preview(
            ui,
            GridLayout::cell(&cells, 0, 1),
            selected_slot,
            data.political_power,
            cmds,
        );
    });
}

fn v9_law_group_list(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    data: &LawPanelData,
    selected_category: &mut Option<LawCategory>,
) {
    use crate::v9::primitives::Card;
    use crate::v9::tokens::{palette, spacing, TextRole};
    use egui::{Align2, Pos2, Rect, Sense, Vec2};

    let inner = Card::new().as_panel().show_at(ui, rect);
    ui.painter().text(
        inner.left_top(),
        Align2::LEFT_TOP,
        "法律组",
        TextRole::Heading.font_id(),
        palette::BRASS_BRIGHT,
    );
    let mut y = inner.top() + 34.0;
    for slot in &data.slots {
        let row = Rect::from_min_size(Pos2::new(inner.left(), y), Vec2::new(inner.width(), 48.0));
        let response = ui.interact(
            row,
            ui.id()
                .with(("law_group_v9", category_label(&slot.category))),
            Sense::click(),
        );
        if response.clicked() {
            *selected_category = Some(slot.category);
        }
        let selected = *selected_category == Some(slot.category);
        let accent = if slot.is_locked {
            palette::BAD
        } else if slot.pending.is_some() || slot.cooldown_days > 0 {
            palette::WARN
        } else {
            v9_law_category_color(&slot.category)
        };
        crate::v9::paint::paint_bevel(
            ui.painter(),
            row,
            if selected {
                palette::IRON_DARK
            } else {
                palette::SOOT_BLACK
            },
            if selected {
                palette::BRASS_DARK
            } else {
                palette::EDGE_DARK
            },
            2.0,
        );
        ui.painter().rect_filled(
            Rect::from_min_size(row.left_top(), Vec2::new(4.0, row.height())),
            egui::epaint::CornerRadius::ZERO,
            accent,
        );
        ui.painter().text(
            Pos2::new(row.left() + spacing::S5, row.top() + spacing::S3),
            Align2::LEFT_TOP,
            category_label(&slot.category),
            TextRole::Subheading.font_id(),
            if selected {
                palette::GOLD_HOT
            } else {
                palette::PARCHMENT
            },
        );
        ui.painter().text(
            Pos2::new(row.left() + spacing::S5, row.bottom() - spacing::S3),
            Align2::LEFT_BOTTOM,
            slot.current_name.as_str(),
            TextRole::Caption.font_id(),
            palette::PARCHMENT_DIM,
        );
        let status = if slot.is_locked {
            tr("locked")
        } else if slot.pending.is_some() {
            tr("v6_law_pending")
        } else if slot.cooldown_days > 0 {
            tr("cooldown")
        } else {
            tr("available")
        };
        ui.painter().text(
            Pos2::new(row.right() - spacing::S4, row.center().y),
            Align2::RIGHT_CENTER,
            status,
            TextRole::Caption.font_id(),
            accent,
        );
        y += 48.0 + spacing::S3;
    }
}

fn v9_law_impact_preview(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    slot: Option<&LawSlotEntry>,
    political_power: f32,
    cmds: &mut Vec<LawCommand>,
) {
    use crate::v9::primitives::{Card, Pill, PillTone};
    use crate::v9::tokens::{palette, spacing, TextRole};
    use egui::{Align2, Pos2, Rect, Vec2};

    let inner = Card::new().as_panel().show_at(ui, rect);
    ui.painter().text(
        inner.left_top(),
        Align2::LEFT_TOP,
        "影响预览",
        TextRole::Heading.font_id(),
        palette::BRASS_BRIGHT,
    );
    let Some(slot) = slot else {
        crate::v9::composites::panel_shell::draw_empty_state(
            ui,
            Rect::from_min_max(
                Pos2::new(inner.left(), inner.top() + 34.0),
                inner.right_bottom(),
            ),
            "暂无法律",
            "当前没有可用法律类别。",
        );
        return;
    };

    let tone = if slot.is_locked {
        PillTone::Bad
    } else if slot.pending.is_some() || slot.cooldown_days > 0 {
        PillTone::Warn
    } else {
        PillTone::Good
    };
    Pill::new(category_label(&slot.category))
        .tone(tone)
        .show_at(
            ui,
            Rect::from_min_size(
                Pos2::new(inner.right() - 154.0, inner.top()),
                Vec2::new(150.0, 22.0),
            ),
        );

    let mut y = inner.top() + 34.0;
    ui.painter().text(
        Pos2::new(inner.left(), y),
        Align2::LEFT_TOP,
        format!("{}: {}", tr("current"), slot.current_name),
        TextRole::Subheading.font_id(),
        palette::PARCHMENT,
    );
    y += 24.0;
    if let Some(reason) = &slot.locked_reason {
        let galley = ui.painter().layout(
            reason.clone(),
            TextRole::Caption.font_id(),
            palette::BAD,
            inner.width(),
        );
        ui.painter()
            .galley(Pos2::new(inner.left(), y), galley, palette::BAD);
        y += 34.0;
    } else if let Some((_, target, days)) = &slot.pending {
        ui.painter().text(
            Pos2::new(inner.left(), y),
            Align2::LEFT_TOP,
            format!("{} {}: {}天", tr("v6_law_pending"), target, days),
            TextRole::Caption.font_id(),
            palette::WARN,
        );
        y += 28.0;
    } else if slot.cooldown_days > 0 {
        ui.painter().text(
            Pos2::new(inner.left(), y),
            Align2::LEFT_TOP,
            format!("{}: {}天", tr("cooldown"), slot.cooldown_days),
            TextRole::Caption.font_id(),
            palette::WARN,
        );
        y += 28.0;
    }

    let list_rect = Rect::from_min_max(Pos2::new(inner.left(), y), inner.right_bottom());
    ui.allocate_ui_at_rect(list_rect, |ui| {
        ui.set_min_size(list_rect.size());
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for tier in &slot.tiers {
                    v9_law_tier_row(ui, slot, tier, political_power, cmds);
                    ui.add_space(spacing::S3);
                }
            });
    });
}

fn v9_law_tier_row(
    ui: &mut egui::Ui,
    slot: &LawSlotEntry,
    tier: &LawTierEntry,
    political_power: f32,
    cmds: &mut Vec<LawCommand>,
) {
    use crate::v9::primitives::{Button, ButtonSize, ButtonVariant};
    use crate::v9::tokens::{palette, spacing, TextRole};
    use egui::{Align2, Pos2, Rect, Sense, Vec2};

    let height = 76.0 + (tier.effects.len().min(3) as f32 * 14.0);
    let (row, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::hover());
    let is_current = tier.id == slot.current_id;
    let is_pending = slot
        .pending
        .as_ref()
        .map_or(false, |(id, _, _)| id == &tier.id);
    let has_pp = political_power >= tier.pp_cost as f32;
    let can_switch =
        !is_current && !is_pending && slot.cooldown_days == 0 && !slot.is_locked && has_pp;
    let accent = if is_current {
        palette::GOOD
    } else if is_pending {
        palette::WARN
    } else if can_switch {
        palette::GOLD
    } else {
        palette::MUTED
    };
    crate::v9::paint::paint_bevel(
        ui.painter(),
        row,
        palette::SOOT_BLACK,
        palette::EDGE_DARK,
        2.0,
    );
    ui.painter().rect_filled(
        Rect::from_min_size(row.left_top(), Vec2::new(4.0, row.height())),
        egui::epaint::CornerRadius::ZERO,
        accent,
    );
    ui.painter().text(
        Pos2::new(row.left() + spacing::S5, row.top() + spacing::S3),
        Align2::LEFT_TOP,
        tier.name.as_str(),
        TextRole::Subheading.font_id(),
        if is_current {
            palette::GOOD
        } else {
            palette::PARCHMENT
        },
    );
    ui.painter().text(
        Pos2::new(row.left() + spacing::S5, row.top() + 28.0),
        Align2::LEFT_TOP,
        format!(
            "花费 {} PP  |  冷却 {} 天",
            tier.pp_cost, tier.cooldown_days
        ),
        TextRole::Caption.font_id(),
        palette::GOLD,
    );
    let mut effect_y = row.top() + 46.0;
    for effect in tier.effects.iter().take(3) {
        ui.painter().text(
            Pos2::new(row.left() + spacing::S5, effect_y),
            Align2::LEFT_TOP,
            effect.as_str(),
            TextRole::Caption.font_id(),
            palette::PARCHMENT_DIM,
        );
        effect_y += 14.0;
    }
    let button_rect = Rect::from_min_size(
        Pos2::new(row.right() - 112.0, row.center().y - 14.0),
        Vec2::new(104.0, ButtonSize::Sm.min_size().y),
    );
    if is_current {
        ui.painter().text(
            button_rect.center(),
            Align2::CENTER_CENTER,
            tr("current"),
            TextRole::Caption.font_id(),
            palette::GOOD,
        );
    } else if can_switch
        && Button::new(tr("v6_law_switch"))
            .size(ButtonSize::Sm)
            .variant(ButtonVariant::Primary)
            .show_at(ui, button_rect)
            .clicked()
    {
        cmds.push(LawCommand::SwitchLaw {
            category: slot.category,
            target_law_id: tier.id.clone(),
        });
    } else if !can_switch {
        Button::new(tr("locked"))
            .size(ButtonSize::Sm)
            .variant(ButtonVariant::Ghost)
            .enabled(false)
            .show_at(ui, button_rect);
        ui.painter().text(
            Pos2::new(row.left() + spacing::S5, row.bottom() - spacing::S3),
            Align2::LEFT_BOTTOM,
            disabled_law_reason(slot, tier, is_pending, has_pp),
            TextRole::Small.font_id(),
            palette::BAD,
        );
    }
}

fn v9_law_affordable_count(data: &LawPanelData) -> usize {
    data.slots
        .iter()
        .flat_map(|slot| slot.tiers.iter().map(move |tier| (slot, tier)))
        .filter(|(slot, tier)| {
            tier.id != slot.current_id
                && slot
                    .pending
                    .as_ref()
                    .map_or(true, |(id, _, _)| id != &tier.id)
                && slot.cooldown_days == 0
                && !slot.is_locked
                && data.political_power >= tier.pp_cost as f32
        })
        .count()
}

fn v9_law_category_color(cat: &LawCategory) -> Color32 {
    use crate::v9::tokens::palette;
    match cat {
        LawCategory::Conscription => palette::IDEO_FASCISM,
        LawCategory::Economy => palette::INFO,
        LawCategory::Trade => palette::GOOD,
        LawCategory::Taxation => palette::GOLD,
        LawCategory::CivilRights => palette::COLD_ATOMIC,
        LawCategory::InformationControl => palette::BAD,
    }
}

fn render_law_summary(ui: &mut egui::Ui, data: &LawPanelData) {
    let locked = data.slots.iter().filter(|slot| slot.is_locked).count();
    let cooling = data
        .slots
        .iter()
        .filter(|slot| slot.cooldown_days > 0)
        .count();
    let pending = data
        .slots
        .iter()
        .filter(|slot| slot.pending.is_some())
        .count();
    let affordable = data
        .slots
        .iter()
        .flat_map(|slot| slot.tiers.iter().map(move |tier| (slot, tier)))
        .filter(|(slot, tier)| {
            tier.id != slot.current_id
                && slot
                    .pending
                    .as_ref()
                    .map_or(true, |(id, _, _)| id != &tier.id)
                && slot.cooldown_days == 0
                && !slot.is_locked
                && data.political_power >= tier.pp_cost as f32
        })
        .count();

    ui.add_space(6.0);
    components::summary_strip(
        ui,
        &[
            (
                tr("political_power"),
                format!("{:.0}", data.political_power),
            ),
            ("法律类别", data.slots.len().to_string()),
            ("可切换", affordable.to_string()),
            ("冷却中", cooling.to_string()),
            ("锁定", locked.to_string()),
            ("进行中", pending.to_string()),
        ],
    );
    ui.add_space(6.0);
}

fn render_law_status_banner(ui: &mut egui::Ui, data: &LawPanelData) {
    let locked = data.slots.iter().filter(|slot| slot.is_locked).count();
    let pending = data
        .slots
        .iter()
        .filter(|slot| slot.pending.is_some())
        .count();
    let cooling = data
        .slots
        .iter()
        .filter(|slot| slot.cooldown_days > 0)
        .count();
    let (label, text, color) = if pending > 0 {
        (
            "法律切换中",
            format!("{} 项法律正在过渡，完成前不能重复切换。", pending),
            WARN,
        )
    } else if locked > 0 {
        (
            "部分法律锁定",
            format!("{} 个法律类别受当前政治或事件状态限制。", locked),
            BAD,
        )
    } else if cooling > 0 {
        (
            "法律冷却",
            format!("{} 个法律类别仍在冷却，等待冷却结束后再调整。", cooling),
            WARN,
        )
    } else if data.political_power < 50.0 {
        (
            "政治力量不足",
            "多数法律切换需要消耗 PP，建议先积累政治力量。".to_owned(),
            WARN,
        )
    } else {
        (
            "法律稳定",
            "当前没有锁定或冷却阻塞，可根据战争、财政和生产需要调整法律。".to_owned(),
            GOOD,
        )
    };

    egui::Frame::new()
        .fill(Color32::from_rgba_premultiplied(0x1d, 0x16, 0x10, 230))
        .stroke(egui::Stroke::new(1.0, color))
        .inner_margin(egui::Margin::symmetric(10, 7))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(label).strong().color(color));
                ui.label(
                    RichText::new(text)
                        .small()
                        .color(Color32::from_rgb(0xe0, 0xd2, 0xa8)),
                );
            });
        });
    ui.add_space(8.0);
}

fn render_law_overview_grid(
    ui: &mut egui::Ui,
    data: &LawPanelData,
    selected_category: &mut Option<LawCategory>,
) {
    ui.label(RichText::new("法律总览").strong().color(GOLD_BRIGHT));
    ui.label(
        RichText::new("点击一个法律类别，打开二级界面选择具体法律。")
            .small()
            .color(MUTED),
    );
    ui.add_space(8.0);

    egui::Grid::new("law_panel_vic3_grid")
        .num_columns(3)
        .spacing([8.0, 8.0])
        .show(ui, |ui| {
            for index in 0..9 {
                if let Some(slot) = data.slots.get(index) {
                    if law_overview_card(ui, slot, data.political_power).clicked() {
                        *selected_category = Some(slot.category);
                    }
                } else {
                    empty_law_grid_cell(ui);
                }

                if index % 3 == 2 {
                    ui.end_row();
                }
            }
        });
}

fn law_overview_card(
    ui: &mut egui::Ui,
    slot: &LawSlotEntry,
    political_power: f32,
) -> egui::Response {
    let size = egui::vec2(160.0, 124.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    if ui.is_rect_visible(rect) {
        let cat_color = category_color(&slot.category);
        let border = if response.hovered() {
            GOLD_BRIGHT
        } else if slot.is_locked {
            BAD
        } else if slot.pending.is_some() {
            GOLD
        } else if slot.cooldown_days > 0 {
            WARN
        } else {
            STROKE_DARK
        };
        let painter = ui.painter();
        painter.rect_filled(rect, 4.0, PANEL_CARD);
        painter.rect_stroke(
            rect,
            4.0,
            egui::Stroke::new(1.2, border),
            egui::epaint::StrokeKind::Inside,
        );
        painter.rect_filled(
            egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), 5.0)),
            3.0,
            cat_color,
        );

        let title_pos = rect.min + egui::vec2(10.0, 20.0);
        painter.text(
            title_pos,
            egui::Align2::LEFT_CENTER,
            category_label(&slot.category),
            egui::FontId::proportional(14.0),
            GOLD_BRIGHT,
        );
        painter.text(
            rect.min + egui::vec2(10.0, 45.0),
            egui::Align2::LEFT_CENTER,
            &slot.current_name,
            egui::FontId::proportional(16.0),
            Color32::from_rgb(0xe0, 0xd2, 0xa8),
        );

        let (status, color) = if slot.is_locked {
            ("锁定", BAD)
        } else if slot.pending.is_some() {
            ("切换中", GOLD)
        } else if slot.cooldown_days > 0 {
            ("冷却中", WARN)
        } else if slot
            .tiers
            .iter()
            .any(|tier| tier.id != slot.current_id && political_power >= tier.pp_cost as f32)
        {
            ("可调整", GOOD)
        } else {
            ("稳定", MUTED)
        };
        painter.text(
            rect.min + egui::vec2(10.0, 76.0),
            egui::Align2::LEFT_CENTER,
            status,
            egui::FontId::proportional(13.0),
            color,
        );

        let detail = if let Some((_, target_name, remaining)) = &slot.pending {
            format!("目标: {} / {}天", target_name, remaining)
        } else if slot.cooldown_days > 0 {
            format!("冷却: {}天", slot.cooldown_days)
        } else if slot.is_locked {
            slot.locked_reason
                .as_deref()
                .unwrap_or(tr("v6_law_locked"))
                .to_owned()
        } else {
            format!("{} 项可选法律", slot.tiers.len())
        };
        painter.text(
            rect.min + egui::vec2(10.0, 100.0),
            egui::Align2::LEFT_CENTER,
            detail,
            egui::FontId::proportional(12.0),
            MUTED,
        );
    }

    response.on_hover_text("点击打开法律选择")
}

fn empty_law_grid_cell(ui: &mut egui::Ui) {
    let size = egui::vec2(160.0, 124.0);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    if ui.is_rect_visible(rect) {
        ui.painter().rect_stroke(
            rect,
            4.0,
            egui::Stroke::new(1.0, Color32::from_rgba_premultiplied(0x5a, 0x44, 0x2c, 80)),
            egui::epaint::StrokeKind::Inside,
        );
    }
}

fn render_law_picker_window(
    ctx: &egui::Context,
    slot: &LawSlotEntry,
    political_power: f32,
    selected_category: &mut Option<LawCategory>,
    cmds: &mut Vec<LawCommand>,
) {
    let mut open = true;
    egui::Window::new(format!("{} 法律", category_label(&slot.category)))
        .id(egui::Id::new("law_picker_window"))
        .default_pos(egui::pos2(575.0, 96.0))
        .open(&mut open)
        .default_width(620.0)
        .default_height(560.0)
        .resizable(true)
        .show(ctx, |ui| {
            render_law_picker_header(ui, slot, political_power);
            ui.add_space(8.0);

            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for tier in &slot.tiers {
                        render_law_tier_choice(ui, slot, tier, political_power, cmds);
                        ui.add_space(6.0);
                    }
                });
        });

    if !open {
        *selected_category = None;
    }
}

fn render_law_picker_header(ui: &mut egui::Ui, slot: &LawSlotEntry, political_power: f32) {
    let cat_color = category_color(&slot.category);
    egui::Frame::new()
        .fill(Color32::from_rgba_premultiplied(0x1d, 0x16, 0x10, 230))
        .stroke(egui::Stroke::new(1.0, cat_color))
        .inner_margin(egui::Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    RichText::new(category_label(&slot.category))
                        .strong()
                        .color(cat_color),
                );
                ui.label(RichText::new(format!("当前: {}", slot.current_name)).strong());
                ui.label(
                    RichText::new(format!("PP {:.0}", political_power))
                        .small()
                        .color(GOLD),
                );
            });

            if slot.is_locked {
                ui.label(
                    RichText::new(slot.locked_reason.as_deref().unwrap_or(tr("v6_law_locked")))
                        .small()
                        .color(BAD),
                );
            }
            if slot.cooldown_days > 0 {
                ui.label(
                    RichText::new(format!("法律冷却中，还需 {} 天。", slot.cooldown_days))
                        .small()
                        .color(WARN),
                );
            }
            if let Some((target_id, target_name, remaining)) = &slot.pending {
                let target_cooldown = slot
                    .tiers
                    .iter()
                    .find(|tier| tier.id == *target_id)
                    .map(|tier| tier.cooldown_days)
                    .unwrap_or(60);
                let ratio = 1.0 - (*remaining as f32 / target_cooldown.max(1) as f32);
                ui.label(
                    RichText::new(format!(
                        "正在切换到 {}，剩余 {} 天。",
                        target_name, remaining
                    ))
                    .small()
                    .color(GOLD),
                );
                ui.add(egui::ProgressBar::new(ratio.clamp(0.0, 1.0)).fill(GOLD));
            }
        });
}

fn render_law_tier_choice(
    ui: &mut egui::Ui,
    slot: &LawSlotEntry,
    tier: &LawTierEntry,
    political_power: f32,
    cmds: &mut Vec<LawCommand>,
) {
    let is_current = tier.id == slot.current_id;
    let is_pending = slot
        .pending
        .as_ref()
        .map_or(false, |(id, _, _)| id == &tier.id);
    let has_pp = political_power >= tier.pp_cost as f32;
    let can_click =
        !is_current && !is_pending && slot.cooldown_days == 0 && !slot.is_locked && has_pp;
    let row_color = if is_current {
        GOOD
    } else if is_pending {
        GOLD
    } else {
        Color32::from_rgb(0xe0, 0xd2, 0xa8)
    };

    egui::Frame::new()
        .fill(Color32::from_rgba_premultiplied(0x31, 0x24, 0x18, 220))
        .stroke(egui::Stroke::new(
            1.0,
            if is_current || is_pending {
                row_color
            } else {
                STROKE_DARK
            },
        ))
        .inner_margin(egui::Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new(&tier.name).strong().color(row_color));
                    ui.label(
                        RichText::new(format!(
                            "消耗 {} PP，切换后冷却 {} 天",
                            tier.pp_cost, tier.cooldown_days
                        ))
                        .small()
                        .color(GOLD),
                    );
                    for effect in &tier.effects {
                        ui.label(RichText::new(effect).small().color(MUTED));
                    }
                    if !is_current && !can_click {
                        ui.label(
                            RichText::new(disabled_law_reason(slot, tier, is_pending, has_pp))
                                .small()
                                .color(BAD),
                        );
                    }
                });

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if is_current {
                        ui.label(RichText::new("当前法律").strong().color(GOOD));
                    } else if is_pending {
                        ui.label(RichText::new("切换中").strong().color(GOLD));
                    } else if ui
                        .add_enabled(
                            can_click,
                            egui::Button::new(
                                RichText::new(tr("v6_law_switch")).color(GOLD_BRIGHT),
                            )
                            .min_size(egui::vec2(96.0, 28.0)),
                        )
                        .clicked()
                    {
                        cmds.push(LawCommand::SwitchLaw {
                            category: slot.category,
                            target_law_id: tier.id.clone(),
                        });
                    }
                });
            });
        });
}

fn disabled_law_reason(
    slot: &LawSlotEntry,
    tier: &LawTierEntry,
    is_pending: bool,
    has_pp: bool,
) -> String {
    if tier.id == slot.current_id {
        return "当前正在使用该法律。".to_owned();
    }
    if is_pending {
        return "该法律已经在切换流程中。".to_owned();
    }
    if slot.is_locked {
        return slot
            .locked_reason
            .clone()
            .unwrap_or_else(|| "该法律类别当前被锁定。".to_owned());
    }
    if slot.cooldown_days > 0 {
        return format!("法律冷却中，还需 {} 天。", slot.cooldown_days);
    }
    if !has_pp {
        return format!("政治力量不足，需要 {} PP。", tier.pp_cost);
    }
    "暂不可切换。".to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_slot() -> LawSlotEntry {
        LawSlotEntry {
            category: LawCategory::Conscription,
            current_id: "volunteer_only".into(),
            current_name: "志愿兵役".into(),
            cooldown_days: 0,
            pending: None,
            is_locked: false,
            locked_reason: None,
            previous_before_lock: None,
            tiers: vec![
                LawTierEntry {
                    id: "volunteer_only".into(),
                    name: "志愿兵役".into(),
                    pp_cost: 50,
                    cooldown_days: 60,
                    effects: Vec::new(),
                },
                LawTierEntry {
                    id: "limited_conscription".into(),
                    name: "有限征兵".into(),
                    pp_cost: 100,
                    cooldown_days: 90,
                    effects: Vec::new(),
                },
                LawTierEntry {
                    id: "extensive_conscription".into(),
                    name: "广泛征兵".into(),
                    pp_cost: 200,
                    cooldown_days: 120,
                    effects: Vec::new(),
                },
                LawTierEntry {
                    id: "total_mobilization".into(),
                    name: "总动员".into(),
                    pp_cost: 400,
                    cooldown_days: 180,
                    effects: Vec::new(),
                },
            ],
        }
    }

    #[test]
    fn sample_slot_constructs() {
        let s = sample_slot();
        assert_eq!(s.tiers.len(), 4);
        assert_eq!(s.current_id, "volunteer_only");
        assert!(!s.is_locked);
    }

    #[test]
    fn law_command_equality() {
        let cmd = LawCommand::SwitchLaw {
            category: LawCategory::Economy,
            target_law_id: "war_economy".into(),
        };
        assert_eq!(
            cmd,
            LawCommand::SwitchLaw {
                category: LawCategory::Economy,
                target_law_id: "war_economy".into(),
            }
        );
    }
}
