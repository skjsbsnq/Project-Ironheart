//! V6 法律面板 UI：6 大类法律切换 + 冷却显示 + PP 消耗。
//!
//! V6.A 验收要求：法律面板出现，能切换冷却显示但 modifier 还没接通。

use crate::i18n::tr;
use egui::Color32;

use hoi4_state::LawCategory;

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

pub fn law_category_label(cat: &LawCategory) -> &'static str {
    category_label(cat)
}

pub fn law_category_key(cat: LawCategory) -> &'static str {
    match cat {
        LawCategory::Conscription => "Conscription",
        LawCategory::Economy => "Economy",
        LawCategory::Trade => "Trade",
        LawCategory::Taxation => "Taxation",
        LawCategory::CivilRights => "CivilRights",
        LawCategory::InformationControl => "InformationControl",
    }
}

pub fn law_category_from_key(key: &str) -> Option<LawCategory> {
    match key {
        "Conscription" | "conscription" => Some(LawCategory::Conscription),
        "Economy" | "economy" => Some(LawCategory::Economy),
        "Trade" | "trade" => Some(LawCategory::Trade),
        "Taxation" | "taxation" => Some(LawCategory::Taxation),
        "CivilRights" | "civil_rights" | "civilRights" => Some(LawCategory::CivilRights),
        "InformationControl" | "information_control" | "informationControl" => {
            Some(LawCategory::InformationControl)
        }
        _ => None,
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

pub fn law_switch_available(
    slot: &LawSlotEntry,
    tier: &LawTierEntry,
    political_power: f32,
) -> bool {
    let is_pending = slot
        .pending
        .as_ref()
        .map_or(false, |(id, _, _)| id == &tier.id);
    tier.id != slot.current_id
        && !is_pending
        && slot.cooldown_days == 0
        && !slot.is_locked
        && political_power >= tier.pp_cost as f32
}

pub fn law_unavailable_reason(
    slot: &LawSlotEntry,
    tier: &LawTierEntry,
    political_power: f32,
) -> Option<String> {
    if law_switch_available(slot, tier, political_power) {
        return None;
    }
    let is_pending = slot
        .pending
        .as_ref()
        .map_or(false, |(id, _, _)| id == &tier.id);
    let has_pp = political_power >= tier.pp_cost as f32;
    Some(disabled_law_reason(slot, tier, is_pending, has_pp))
}

pub struct LawPanel;

impl LawPanel {
    pub fn show(ctx: &egui::Context, data: &LawPanelData) -> (bool, Vec<LawCommand>) {
        v9_show_law(ctx, data)
    }
}

fn v9_show_law(ctx: &egui::Context, data: &LawPanelData) -> (bool, Vec<LawCommand>) {
    use crate::v9::composites::panel_shell::{
        draw_summary_tiles, draw_tab_strip, PanelClass, PanelShell,
    };
    use crate::v9::tokens::palette;

    let selected_id = egui::Id::new("law_panel_v9_selected_overview_category");
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
        .footer("Q 关闭 | 选择法律组 / 切换")
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
