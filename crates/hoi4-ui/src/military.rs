//! V5 阶段 C.6 + 11.2：陆军管理 UI。
//!
//! 模仿 HOI4 vanilla 交互流程：
//! - 选中师团 → 底栏显示已选师团 + "创建集团军" (+) 按钮
//! - 选中集团军 → 底栏显示集团军详情 + 画线/画箭头按钮
//! - 画线模式 → 底栏显示模式提示 + Esc/右键取消

use crate::{
    components,
    i18n::tr,
    portrait::{self, PortraitStyle},
    vanilla_iron::{CommandPanelShell, VanillaIron},
    ActiveDetailPanel, ActivePrimaryPanel, ArmyDetailTarget, PanelCommand,
};
use egui::{Color32, RichText};

const GOLD: Color32 = Color32::from_rgb(0xc9, 0xa5, 0x5b);
const GOLD_BRIGHT: Color32 = Color32::from_rgb(0xe0, 0xc0, 0x78);
const MUTED: Color32 = Color32::from_gray(155);
const PANEL_CARD: Color32 = Color32::from_rgb(0x24, 0x1a, 0x12);
const PANEL_CARD_SOFT: Color32 = Color32::from_rgb(0x31, 0x24, 0x18);
const STROKE_DARK: Color32 = Color32::from_rgb(0x5a, 0x44, 0x2c);
const GOOD: Color32 = Color32::from_rgb(0x70, 0xc8, 0x78);
const WARN: Color32 = Color32::from_rgb(0xff, 0xc0, 0x60);
const BAD: Color32 = Color32::from_rgb(0xe0, 0x60, 0x58);

pub struct DivisionEntry {
    pub index: usize,
    pub name: String,
    pub organisation: f32,
    pub max_organisation: f32,
    pub experience: f32,
    pub strength: f32,
    pub in_combat: bool,
    pub province_id: u16,
    pub province_name: String,
    pub equipment_ratio: f32,
    pub army_id: Option<u32>,
    pub army_name: Option<String>,
}

pub struct TemplateEntry {
    pub index: u16,
    pub name: String,
    pub battalion_count: usize,
    pub combat_width: f32,
    pub manpower: u32,
    pub training_days: f32,
    pub stockpile_satisfied_divisions: f32,
}

pub struct TemplateEditorData {
    pub selected_template: Option<u16>,
    pub name: String,
    pub line_battalions: Vec<Vec<Option<String>>>,
    pub support_companies: Vec<Option<String>>,
    pub line_choices: Vec<String>,
    pub support_choices: Vec<String>,
    pub combat_width: f32,
    pub manpower: u32,
    pub max_organisation: f32,
    pub soft_attack: f32,
    pub hard_attack: f32,
    pub defense: f32,
    pub breakthrough: f32,
    pub suppression: f32,
    pub supply_consumption: f32,
    pub training_days: f32,
    pub equipment_needed: Vec<(String, u32)>,
    pub stockpile_satisfied_divisions: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TemplatePickerTarget {
    Line { template: u16, row: u8, col: u8 },
    Support { template: u16, slot: u8 },
}

pub struct TemplateSubunitPickerData {
    pub target: Option<TemplatePickerTarget>,
    pub title: String,
    pub current: Option<String>,
    pub choices: Vec<String>,
}

pub struct TrainingQueueEntry {
    pub id: u32,
    pub template_name: String,
    pub count: u8,
    pub progress: f32,
    pub manpower_allocated: u32,
    pub required_manpower: u32,
}

pub struct ArmyEntry {
    pub id: u32,
    pub name: String,
    pub member_count: usize,
    pub has_path: bool,
    pub has_arrow: bool,
    pub active: bool,
    pub executing: bool,
    pub commander: Option<u32>,
    pub commander_name: Option<String>,
    pub command_limit: Option<u16>,
    pub command_efficiency: f32,
    pub attack_bonus_pct: f32,
    pub defense_bonus_pct: f32,
    pub planning_bonus_pct: f32,
    pub org_recovery_bonus_pct: f32,
    pub supply_reduction_pct: f32,
}

pub struct GeneralEntry {
    pub id: u32,
    pub name: String,
    pub skill: u8,
    pub attack: u8,
    pub defense: u8,
    pub planning: u8,
    pub logistics: u8,
    pub command_limit: u16,
    pub assigned_army_id: Option<u32>,
}

pub struct MilitaryData {
    pub divisions: Vec<DivisionEntry>,
    pub templates: Vec<TemplateEntry>,
    pub template_editor_open: bool,
    pub template_editor: TemplateEditorData,
    pub template_subunit_picker: TemplateSubunitPickerData,
    pub training_queue: Vec<TrainingQueueEntry>,
    pub player_capital_province: u16,
    pub armies: Vec<ArmyEntry>,
    pub generals: Vec<GeneralEntry>,
    pub frontline_overlay_visible: bool,
    pub selected_division_count: usize,
    pub active_army_count: usize,
    pub max_armies_per_country: usize,
    pub selected_army_id: Option<u32>,
    pub painter_mode: PainterModeInfo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PainterModeInfo {
    Idle,
    ArmyPainter(u32),
    ArrowPainter(u32),
}

#[derive(Debug)]
pub enum MilitaryCommand {
    Train(u16),
    NewTemplate,
    OpenTemplateEditor(u16),
    CloseTemplateEditor,
    OpenLineSubunitPicker {
        template: u16,
        row: u8,
        col: u8,
    },
    OpenSupportSubunitPicker {
        template: u16,
        slot: u8,
    },
    CloseSubunitPicker,
    SelectTemplate(u16),
    RenameTemplate(u16, String),
    CloneTemplate(u16),
    DeleteTemplate(u16),
    AddLineBattalion(u16, String),
    AddSupportCompany(u16, String),
    SetLineBattalion {
        template: u16,
        row: u8,
        col: u8,
        subunit: Option<String>,
    },
    SetSupportCompany {
        template: u16,
        slot: u8,
        subunit: Option<String>,
    },
    CreateArmy,
    DissolveArmy(u32),
    AddMembers(u32),
    RemoveMembers(u32),
    DrawFrontline(u32),
    ClearFrontline(u32),
    DrawArrow(u32),
    ClearArrow(u32),
    ExecutePlan(u32),
    HaltPlan(u32),
    AssignGeneral {
        army_id: u32,
        general_id: u32,
    },
    UnassignGeneral(u32),
    ToggleOverlay,
    SelectArmy(u32),
    ClearArmySelection,
    AddSelectedDivisionsToArmy(u32),
    ToggleDivisionSelection(usize),
    SelectAllDivisions,
    Panel(PanelCommand),
}

pub struct MilitaryPanel;

const ARMY_CARD_W: f32 = 84.0;
const ARMY_CARD_H: f32 = 64.0;
const ADD_SLOT_W: f32 = 58.0;
const CARD_GAP_W: f32 = 6.0;
const TOOLBAR_W: f32 = 580.0;
const TRAY_MAX_W: f32 = 760.0;
const BOTTOM_MARGIN: f32 = 30.0;

fn army_card(ui: &mut egui::Ui, army: &ArmyEntry, selected: bool) -> egui::Response {
    let size = egui::vec2(ARMY_CARD_W, ARMY_CARD_H);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let over_limit = army
        .command_limit
        .is_some_and(|limit| army.member_count > limit as usize);
    if ui.is_rect_visible(rect) {
        let fill = if selected {
            PANEL_CARD_SOFT
        } else {
            PANEL_CARD
        };
        let border = if selected {
            GOLD_BRIGHT
        } else if response.hovered() {
            GOLD
        } else {
            STROKE_DARK
        };
        let painter = ui.painter();
        painter.rect_filled(rect, 2.0, fill);
        painter.rect_stroke(
            rect,
            2.0,
            egui::Stroke::new(1.2, border),
            egui::epaint::StrokeKind::Inside,
        );

        let color_bar = egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), 4.0));
        let bar_color = if over_limit {
            BAD
        } else if army.executing {
            GOOD
        } else if army.has_arrow {
            WARN
        } else if army.has_path {
            GOLD_BRIGHT
        } else {
            MUTED
        };
        painter.rect_filled(color_bar, 1.0, bar_color);

        let portrait_rect =
            egui::Rect::from_min_size(rect.min + egui::vec2(8.0, 9.0), PortraitStyle::Small.size());
        portrait::paint_portrait_at(
            painter,
            portrait_rect,
            army.commander_name.as_deref(),
            PortraitStyle::Small.initial_font_size(),
        );

        let badge =
            egui::Rect::from_min_size(rect.min + egui::vec2(42.0, 43.0), egui::vec2(32.0, 15.0));
        painter.rect_filled(badge, 2.0, Color32::from_rgb(0x1d, 0x16, 0x10));
        painter.rect_stroke(
            badge,
            2.0,
            egui::Stroke::new(1.0, STROKE_DARK),
            egui::epaint::StrokeKind::Inside,
        );
        painter.text(
            badge.center(),
            egui::Align2::CENTER_CENTER,
            format!("{} 师", army.member_count),
            egui::FontId::proportional(10.0),
            Color32::WHITE,
        );

        painter.text(
            rect.min + egui::vec2(42.0, 17.0),
            egui::Align2::LEFT_CENTER,
            army.commander_name.as_deref().unwrap_or(&army.name),
            egui::FontId::proportional(12.0),
            GOLD_BRIGHT,
        );
        let order = if over_limit {
            "超限"
        } else if army.executing {
            "执行中"
        } else if army.has_arrow {
            "计划"
        } else if army.has_path {
            "前线"
        } else {
            "待命"
        };
        painter.text(
            rect.min + egui::vec2(42.0, 35.0),
            egui::Align2::LEFT_CENTER,
            order,
            egui::FontId::proportional(11.0),
            bar_color,
        );
    }
    let mut tooltip = format!(
        "{}\n师团: {}",
        army.commander_name.as_deref().unwrap_or("无将领"),
        army.member_count
    );
    tooltip.push_str("\n左键：选中  右键：加入选中师团");
    if let Some(limit) = army.command_limit {
        tooltip.push_str(&format!(
            "\n指挥上限: {}/{}  效率: {:.0}%",
            army.member_count,
            limit,
            army.command_efficiency * 100.0
        ));
        if over_limit {
            tooltip.push_str("\n超过指挥上限：将领加成被按比例削弱");
        }
        tooltip.push_str(&format!(
            "\n加成来源: 攻击 +{:.0}%  防御 +{:.0}%  计划 +{:.0}%\n后勤: 组织恢复 +{:.0}%  补给/装备消耗 -{:.0}%",
            army.attack_bonus_pct,
            army.defense_bonus_pct,
            army.planning_bonus_pct,
            army.org_recovery_bonus_pct,
            army.supply_reduction_pct
        ));
    } else {
        tooltip.push_str("\n无将领加成");
    }
    response.on_hover_text(tooltip)
}

fn tool_button(ui: &mut egui::Ui, enabled: bool, label: &str, tooltip: &str) -> egui::Response {
    ui.add_enabled(
        enabled,
        egui::Button::new(RichText::new(label).strong().color(GOLD))
            .fill(PANEL_CARD)
            .stroke(egui::Stroke::new(1.0, STROKE_DARK))
            .min_size(egui::vec2(44.0, 28.0)),
    )
    .on_hover_text(tooltip)
}

fn add_army_slot(ui: &mut egui::Ui, data: &MilitaryData) -> egui::Response {
    let can_create =
        data.selected_division_count > 0 && data.active_army_count < data.max_armies_per_country;
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ADD_SLOT_W, ARMY_CARD_H), egui::Sense::click());
    let response = response.on_hover_text(if can_create {
        "创建集团军（Ctrl多选师团后点击）"
    } else if data.selected_division_count == 0 {
        "请先选择师团（Ctrl+点击多选）"
    } else {
        "已达到集团军上限"
    });
    let painter = ui.painter();
    let border = if can_create { GOLD } else { STROKE_DARK };
    painter.rect_filled(rect, 2.0, PANEL_CARD);
    painter.rect_stroke(
        rect,
        2.0,
        egui::Stroke::new(1.1, border),
        egui::epaint::StrokeKind::Inside,
    );
    painter.text(
        rect.center() - egui::vec2(0.0, 6.0),
        egui::Align2::CENTER_CENTER,
        "+",
        egui::FontId::proportional(30.0),
        if can_create { GOLD } else { Color32::GRAY },
    );
    painter.text(
        rect.center_bottom() - egui::vec2(0.0, 10.0),
        egui::Align2::CENTER_CENTER,
        format!("{} 师", data.selected_division_count),
        egui::FontId::proportional(10.0),
        Color32::GRAY,
    );

    response
}

fn show_template_editor(ui: &mut egui::Ui, data: &MilitaryData, cmds: &mut Vec<MilitaryCommand>) {
    let editor = &data.template_editor;
    ui.label(RichText::new("师模板编辑器").strong().color(GOLD));
    let Some(template_idx) = editor.selected_template else {
        ui.label("选择一个模板，或点击新建空模板。");
        return;
    };

    ui.horizontal_wrapped(|ui| {
        ui.label("模板名");
        let mut name = editor.name.clone();
        let response = ui.text_edit_singleline(&mut name);
        if response.changed() && name != editor.name {
            cmds.push(MilitaryCommand::RenameTemplate(template_idx, name));
        }
        if ui.small_button("复制").clicked() {
            cmds.push(MilitaryCommand::CloneTemplate(template_idx));
        }
        if ui.small_button("训练").clicked() {
            cmds.push(MilitaryCommand::Train(template_idx));
        }
        if ui.small_button("删除").clicked() {
            cmds.push(MilitaryCommand::DeleteTemplate(template_idx));
        }
    });

    ui.add_space(4.0);
    ui.label(RichText::new("战斗营 5×5").strong());
    egui::Grid::new("template_line_grid")
        .spacing([4.0, 4.0])
        .show(ui, |ui| {
            for row in 0..5 {
                for col in 0..5 {
                    let current = editor
                        .line_battalions
                        .get(row)
                        .and_then(|r| r.get(col))
                        .cloned()
                        .flatten();
                    let label = current.as_deref().unwrap_or("+");
                    if ui.button(label).clicked() {
                        cmds.push(MilitaryCommand::OpenLineSubunitPicker {
                            template: template_idx,
                            row: row as u8,
                            col: col as u8,
                        });
                    }
                    if ui.small_button("x").clicked() {
                        cmds.push(MilitaryCommand::SetLineBattalion {
                            template: template_idx,
                            row: row as u8,
                            col: col as u8,
                            subunit: None,
                        });
                    }
                }
                ui.end_row();
            }
        });

    ui.add_space(4.0);
    ui.label(RichText::new("支援连").strong());
    ui.horizontal_wrapped(|ui| {
        for slot in 0..5 {
            let current = editor.support_companies.get(slot).cloned().flatten();
            let label = current.as_deref().unwrap_or("+");
            if ui.button(label).clicked() {
                cmds.push(MilitaryCommand::OpenSupportSubunitPicker {
                    template: template_idx,
                    slot: slot as u8,
                });
            }
            if ui.small_button("x").clicked() {
                cmds.push(MilitaryCommand::SetSupportCompany {
                    template: template_idx,
                    slot: slot as u8,
                    subunit: None,
                });
            }
        }
    });

    ui.add_space(4.0);
    ui.label(format!(
        "宽 {:.0} / 人力 {} / 组织 {:.1} / 软攻 {:.1} / 硬攻 {:.1} / 防御 {:.1} / 突破 {:.1}",
        editor.combat_width,
        editor.manpower,
        editor.max_organisation,
        editor.soft_attack,
        editor.hard_attack,
        editor.defense,
        editor.breakthrough,
    ));
    ui.label(format!(
        "镇压 {:.1} / 补给 {:.1} / 训练 {:.0} 天 / 当前库存可训 {:.1} 个",
        editor.suppression,
        editor.supply_consumption,
        editor.training_days,
        editor.stockpile_satisfied_divisions,
    ));
    let equipment = if editor.equipment_needed.is_empty() {
        "无装备需求".to_owned()
    } else {
        editor
            .equipment_needed
            .iter()
            .map(|(id, qty)| format!("{} {}", id, qty))
            .collect::<Vec<_>>()
            .join(" / ")
    };
    ui.label(format!("装备需求：{}", equipment));
}

fn show_template_editor_window(
    ctx: &egui::Context,
    data: &MilitaryData,
    cmds: &mut Vec<MilitaryCommand>,
) {
    let mut open = true;
    egui::Window::new("师模板编辑器")
        .open(&mut open)
        .default_width(620.0)
        .default_height(620.0)
        .resizable(true)
        .show(ctx, |ui| {
            show_template_editor(ui, data, cmds);
        });
    if !open {
        cmds.push(MilitaryCommand::CloseTemplateEditor);
    }
}

fn show_subunit_picker_window(
    ctx: &egui::Context,
    data: &MilitaryData,
    cmds: &mut Vec<MilitaryCommand>,
) {
    let picker = &data.template_subunit_picker;
    let Some(target) = picker.target else {
        return;
    };
    let mut open = true;
    egui::Window::new(&picker.title)
        .open(&mut open)
        .default_width(320.0)
        .default_height(420.0)
        .resizable(true)
        .show(ctx, |ui| {
            ui.label(format!(
                "当前：{}",
                picker.current.as_deref().unwrap_or("空")
            ));
            if ui.button("清空此槽").clicked() {
                match target {
                    TemplatePickerTarget::Line { template, row, col } => {
                        cmds.push(MilitaryCommand::SetLineBattalion {
                            template,
                            row,
                            col,
                            subunit: None,
                        });
                    }
                    TemplatePickerTarget::Support { template, slot } => {
                        cmds.push(MilitaryCommand::SetSupportCompany {
                            template,
                            slot,
                            subunit: None,
                        });
                    }
                }
                cmds.push(MilitaryCommand::CloseSubunitPicker);
            }
            ui.separator();
            egui::ScrollArea::vertical().show(ui, |ui| {
                for choice in &picker.choices {
                    let selected = picker.current.as_deref() == Some(choice.as_str());
                    if ui.selectable_label(selected, choice).clicked() {
                        match target {
                            TemplatePickerTarget::Line { template, row, col } => {
                                cmds.push(MilitaryCommand::SetLineBattalion {
                                    template,
                                    row,
                                    col,
                                    subunit: Some(choice.clone()),
                                });
                            }
                            TemplatePickerTarget::Support { template, slot } => {
                                cmds.push(MilitaryCommand::SetSupportCompany {
                                    template,
                                    slot,
                                    subunit: Some(choice.clone()),
                                });
                            }
                        }
                        cmds.push(MilitaryCommand::CloseSubunitPicker);
                    }
                }
            });
        });
    if !open {
        cmds.push(MilitaryCommand::CloseSubunitPicker);
    }
}

fn render_military_summary(ui: &mut egui::Ui, data: &MilitaryData) {
    let queued = data
        .training_queue
        .iter()
        .map(|item| item.count as usize)
        .sum::<usize>();
    let executing = data.armies.iter().filter(|army| army.executing).count();

    ui.add_space(6.0);
    components::summary_strip(
        ui,
        &[
            ("师团", data.divisions.len().to_string()),
            (
                "集团军",
                format!("{}/{}", data.active_army_count, data.max_armies_per_country),
            ),
            ("选中", data.selected_division_count.to_string()),
            ("训练中", queued.to_string()),
            ("执行计划", executing.to_string()),
        ],
    );
    ui.add_space(6.0);
}

fn render_military_status_banner(ui: &mut egui::Ui, data: &MilitaryData) {
    let selected_army = data
        .selected_army_id
        .and_then(|id| data.armies.iter().find(|army| army.id == id));
    let (label, text, color) = if matches!(data.painter_mode, PainterModeInfo::ArmyPainter(_)) {
        (
            "绘制前线",
            "在地图上拖拽经过省份，松开鼠标确认；Esc 或右键取消。".to_owned(),
            WARN,
        )
    } else if matches!(data.painter_mode, PainterModeInfo::ArrowPainter(_)) {
        (
            "绘制进攻箭头",
            "从前线方向拖拽到目标区域，松开鼠标确认；Esc 或右键取消。".to_owned(),
            WARN,
        )
    } else if let Some(army) = selected_army {
        if army.executing {
            (
                "计划执行中",
                format!("{} 正在执行作战计划，可在底栏停止或调整命令。", army.name),
                GOOD,
            )
        } else if !army.has_path {
            (
                "需要前线",
                format!("{} 尚未设置前线。先画前线，再画进攻箭头。", army.name),
                WARN,
            )
        } else if !army.has_arrow {
            (
                "需要箭头",
                format!("{} 已有前线，下一步绘制进攻箭头。", army.name),
                WARN,
            )
        } else {
            (
                "计划就绪",
                format!("{} 已有前线和进攻箭头，可执行计划。", army.name),
                GOOD,
            )
        }
    } else if data.selected_division_count > 0 {
        (
            "可创建集团军",
            format!(
                "已选中 {} 个师，可在底栏创建或加入集团军。",
                data.selected_division_count
            ),
            GOOD,
        )
    } else {
        (
            "待命",
            "选择地图上的师团后，可创建集团军并绘制作战计划。".to_owned(),
            MUTED,
        )
    };

    components::status_banner(ui, color, label, &text);
    ui.add_space(4.0);
}

fn v9_show_military(ctx: &egui::Context, data: &MilitaryData) -> (bool, Vec<MilitaryCommand>) {
    use crate::v9::composites::panel_shell::{
        draw_summary_tiles, draw_tab_strip, PanelClass, PanelShell,
    };
    use crate::v9::tokens::palette;

    let mut cmds = Vec::new();
    let queued = data
        .training_queue
        .iter()
        .map(|item| item.count as usize)
        .sum::<usize>();
    let executing = data.armies.iter().filter(|army| army.executing).count();
    let accent = if matches!(
        data.painter_mode,
        PainterModeInfo::ArmyPainter(_) | PainterModeInfo::ArrowPainter(_)
    ) {
        palette::WARN
    } else if executing > 0 {
        palette::GOOD
    } else if data.selected_division_count > 0 || data.selected_army_id.is_some() {
        palette::GOLD
    } else {
        palette::BRASS_BRIGHT
    };

    let (close, _) = PanelShell::new("military_panel_v9", tr("military"))
        .subtitle("军团列表 / 师团表 / 命令条")
        .class(PanelClass::MilitaryDiplomacy)
        .accent(accent)
        .footer("Q Close  |  Ctrl click division  |  Right click army to attach")
        .show(ctx, |ui, layout| {
            draw_summary_tiles(
                ui,
                layout.summary,
                &[
                    ("师团", data.divisions.len().to_string(), palette::GOLD),
                    (
                        "集团军",
                        format!("{}/{}", data.active_army_count, data.max_armies_per_country),
                        palette::BRASS_BRIGHT,
                    ),
                    (
                        "选中",
                        data.selected_division_count.to_string(),
                        if data.selected_division_count > 0 {
                            palette::GOOD
                        } else {
                            palette::MUTED
                        },
                    ),
                    ("训练中", queued.to_string(), palette::INFO),
                    (
                        "执行计划",
                        executing.to_string(),
                        if executing > 0 {
                            palette::GOOD
                        } else {
                            palette::MUTED
                        },
                    ),
                ],
            );
            draw_tab_strip(ui, layout.tabs, "军团列表 / 师团表 / 命令条", accent);
            v9_military_body(ui, layout.body, data, &mut cmds);
        });

    if data.template_editor_open {
        show_template_editor_window(ctx, data, &mut cmds);
    }
    show_subunit_picker_window(ctx, data, &mut cmds);

    (close, cmds)
}

fn v9_military_body(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    data: &MilitaryData,
    cmds: &mut Vec<MilitaryCommand>,
) {
    use crate::v9::layout::{GridLayout, Track};
    use crate::v9::tokens::spacing;

    ui.allocate_ui_at_rect(rect, |ui| {
        let grid = GridLayout::new(
            vec![Track::Fr(1.0)],
            vec![Track::Fr(0.30), Track::Fr(0.46), Track::Fr(0.24)],
        )
        .with_gutter(spacing::S5, 0.0);
        let cells = grid.measure(rect);
        v9_military_army_list(ui, GridLayout::cell(&cells, 0, 0), data, cmds);
        v9_military_division_table(ui, GridLayout::cell(&cells, 0, 1), data, cmds);
        v9_military_order_ribbon(ui, GridLayout::cell(&cells, 0, 2), data, cmds);
    });
}

fn v9_military_army_list(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    data: &MilitaryData,
    cmds: &mut Vec<MilitaryCommand>,
) {
    use crate::nato_icon::NatoArchetype;
    use crate::v9::primitives::{Card, CounterIcon};
    use crate::v9::tokens::{palette, spacing, TextRole};
    use egui::{Align2, Pos2, Rect, Sense, Vec2};

    let inner = Card::new().as_panel().show_at(ui, rect);
    ui.painter().text(
        inner.left_top(),
        Align2::LEFT_TOP,
        "集团军",
        TextRole::Heading.font_id(),
        palette::BRASS_BRIGHT,
    );

    let list_rect = Rect::from_min_max(
        Pos2::new(inner.left(), inner.top() + 34.0),
        inner.right_bottom(),
    );
    if data.armies.is_empty() {
        crate::v9::composites::panel_shell::draw_empty_state(
            ui,
            list_rect,
            "暂无集团军",
            "选择师团后可在命令条创建。",
        );
        return;
    }

    let row_h = 58.0;
    let max_rows = (list_rect.height() / (row_h + spacing::S2))
        .floor()
        .max(1.0) as usize;
    for (idx, army) in data.armies.iter().take(max_rows).enumerate() {
        let top = list_rect.top() + idx as f32 * (row_h + spacing::S2);
        let row = Rect::from_min_size(
            Pos2::new(list_rect.left(), top),
            Vec2::new(list_rect.width(), row_h),
        );
        let selected = data.selected_army_id == Some(army.id);
        let accent = army_status_color(army);
        crate::v9::paint::paint_bevel(
            ui.painter(),
            row,
            if selected {
                palette::IRON
            } else {
                palette::SOOT_BLACK
            },
            if selected {
                palette::GOLD_HOT
            } else {
                palette::EDGE_DARK
            },
            2.0,
        );
        let response = ui
            .interact(row, ui.id().with(("v9_army_row", army.id)), Sense::click())
            .on_hover_text("左键选中集团军；右键把当前选中师团加入集团军");
        if response.clicked() {
            cmds.push(MilitaryCommand::SelectArmy(army.id));
        }
        if response.secondary_clicked() && data.selected_division_count > 0 {
            cmds.push(MilitaryCommand::AddSelectedDivisionsToArmy(army.id));
        }

        let counter_label = format!("{} 师", army.member_count);
        CounterIcon::new(counter_label.as_str(), NatoArchetype::Infantry)
            .accent(accent)
            .selected(selected)
            .show_at(
                ui,
                Rect::from_min_size(
                    row.left_top() + Vec2::new(spacing::S3, spacing::S3),
                    Vec2::new(70.0, 24.0),
                ),
            );

        let painter = ui
            .painter()
            .with_clip_rect(row.shrink2(Vec2::new(84.0, 0.0)));
        painter.text(
            Pos2::new(row.left() + 84.0, row.top() + 14.0),
            Align2::LEFT_CENTER,
            army.name.as_str(),
            TextRole::Subheading.font_id(),
            palette::PARCHMENT,
        );
        painter.text(
            Pos2::new(row.left() + 84.0, row.top() + 34.0),
            Align2::LEFT_CENTER,
            army.commander_name.as_deref().unwrap_or("无将领"),
            TextRole::Caption.font_id(),
            palette::PARCHMENT_DIM,
        );
        painter.text(
            Pos2::new(row.right() - spacing::S3, row.center().y),
            Align2::RIGHT_CENTER,
            army_status_label(army),
            TextRole::Caption.font_id(),
            accent,
        );
    }
}

fn v9_military_division_table(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    data: &MilitaryData,
    cmds: &mut Vec<MilitaryCommand>,
) {
    use crate::v9::primitives::{Card, DataTable, TableCell, TableColumn, TableRow};
    use crate::v9::tokens::{palette, TextRole};
    use egui::{Align2, Pos2, Rect, Sense};

    let inner = Card::new().as_panel().show_at(ui, rect);
    ui.painter().text(
        inner.left_top(),
        Align2::LEFT_TOP,
        "师团表",
        TextRole::Heading.font_id(),
        palette::BRASS_BRIGHT,
    );

    let table_rect = Rect::from_min_max(
        Pos2::new(inner.left(), inner.top() + 34.0),
        inner.right_bottom(),
    );
    if data.divisions.is_empty() {
        crate::v9::composites::panel_shell::draw_empty_state(
            ui,
            table_rect,
            "暂无师团",
            "地图上选中己方师团后会显示。",
        );
        return;
    }

    let row_h = 28.0;
    let max_rows = ((table_rect.height() - row_h) / row_h).floor().max(0.0) as usize;
    let visible: Vec<&DivisionEntry> = data.divisions.iter().take(max_rows).collect();
    let rows: Vec<TableRow> = visible
        .iter()
        .map(|div| {
            let org_ratio = org_ratio(div);
            let accent = if div.in_combat {
                palette::BAD
            } else if org_ratio < 0.5 || div.strength < 0.65 {
                palette::WARN
            } else {
                palette::GOOD
            };
            TableRow::new(vec![
                TableCell::strong(tr(&div.name)),
                TableCell::new(div.army_name.as_deref().unwrap_or("未编入")),
                TableCell::colored(format!("{:.0}%", org_ratio * 100.0), accent).right(),
                TableCell::colored(
                    format!("{:.0}%", div.equipment_ratio * 100.0),
                    ratio_color(div.equipment_ratio),
                )
                .right(),
                TableCell::colored(
                    format!("{:.0}%", div.strength * 100.0),
                    ratio_color(div.strength),
                )
                .right(),
                TableCell::new(&div.province_name).right(),
            ])
            .accent(accent)
        })
        .collect();

    DataTable::new(
        vec![
            TableColumn::new("师团", 1.28),
            TableColumn::new("集团军", 0.85),
            TableColumn::new("组织", 0.55).right(),
            TableColumn::new("装备", 0.55).right(),
            TableColumn::new("兵力", 0.55).right(),
            TableColumn::new("省份", 0.50).right(),
        ],
        rows,
    )
    .row_height(row_h)
    .show_at(ui, table_rect);

    for (idx, div) in visible.iter().enumerate() {
        let top = table_rect.top() + row_h * (idx as f32 + 1.0);
        let row = Rect::from_min_max(
            Pos2::new(table_rect.left(), top),
            Pos2::new(table_rect.right(), top + row_h),
        );
        let response = ui
            .interact(
                row,
                ui.id().with(("v9_division_row", div.index)),
                Sense::click(),
            )
            .on_hover_text("点击切换师团选择；Shift 点击选择全部");
        if response.clicked() {
            let modifiers = ui.ctx().input(|i| i.modifiers);
            if modifiers.shift {
                cmds.push(MilitaryCommand::SelectAllDivisions);
            } else {
                cmds.push(MilitaryCommand::ToggleDivisionSelection(div.index));
            }
        }
    }
}

fn v9_military_order_ribbon(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    data: &MilitaryData,
    cmds: &mut Vec<MilitaryCommand>,
) {
    use crate::v9::primitives::{Button, ButtonSize, ButtonVariant, Card};
    use crate::v9::tokens::{palette, spacing, TextRole};
    use egui::{Align2, Pos2, Rect, Vec2};

    let inner = Card::new().as_ornate().show_at(ui, rect);
    ui.painter().text(
        inner.left_top(),
        Align2::LEFT_TOP,
        "命令 Ribbon",
        TextRole::Heading.font_id(),
        palette::BRASS_BRIGHT,
    );

    let selected_army = data
        .selected_army_id
        .and_then(|id| data.armies.iter().find(|army| army.id == id));
    let mut y = inner.top() + 32.0;
    let line_h = 22.0;

    match data.painter_mode {
        PainterModeInfo::ArmyPainter(_) => {
            v9_ribbon_text(
                ui,
                inner,
                &mut y,
                "绘制前线",
                "在地图上拖拽省份，松开确认。",
                palette::WARN,
            );
        }
        PainterModeInfo::ArrowPainter(_) => {
            v9_ribbon_text(
                ui,
                inner,
                &mut y,
                "绘制进攻箭头",
                "从前线拖向目标区域。",
                palette::WARN,
            );
        }
        PainterModeInfo::Idle => {
            if let Some(army) = selected_army {
                v9_ribbon_text(
                    ui,
                    inner,
                    &mut y,
                    army.name.as_str(),
                    army_status_label(army),
                    army_status_color(army),
                );
            } else if data.selected_division_count > 0 {
                v9_ribbon_text(
                    ui,
                    inner,
                    &mut y,
                    "已选中师团",
                    "可创建集团军或加入现有集团军。",
                    palette::GOOD,
                );
            } else {
                v9_ribbon_text(
                    ui,
                    inner,
                    &mut y,
                    "待命",
                    "选择地图上的师团或集团军。",
                    palette::MUTED,
                );
            }
        }
    }

    y += spacing::S2;
    if let Some(army) = selected_army {
        let button_w = (inner.width() - spacing::S3) * 0.5;
        let left = inner.left();
        let right = left + button_w + spacing::S3;
        let rect = Rect::from_min_size(Pos2::new(left, y), Vec2::new(button_w, 28.0));
        if Button::new("画前线")
            .size(ButtonSize::Sm)
            .variant(ButtonVariant::Primary)
            .show_at(ui, rect)
            .on_hover_text("绘制或重绘集团军前线")
            .clicked()
        {
            cmds.push(MilitaryCommand::DrawFrontline(army.id));
        }
        let rect = Rect::from_min_size(Pos2::new(right, y), Vec2::new(button_w, 28.0));
        let response = Button::new("画箭头")
            .size(ButtonSize::Sm)
            .variant(ButtonVariant::Primary)
            .enabled(army.has_path)
            .show_at(ui, rect)
            .on_hover_text("已有前线后绘制进攻箭头");
        if army.has_path && response.clicked() {
            cmds.push(MilitaryCommand::DrawArrow(army.id));
        }
        y += 32.0;

        let exec_label = if army.executing {
            "停止计划"
        } else {
            "执行计划"
        };
        let exec_enabled = army.executing || army.has_arrow;
        let exec_variant = if army.executing {
            ButtonVariant::Danger
        } else {
            ButtonVariant::Primary
        };
        let rect = Rect::from_min_size(Pos2::new(left, y), Vec2::new(button_w, 28.0));
        let response = Button::new(exec_label)
            .size(ButtonSize::Sm)
            .variant(exec_variant)
            .enabled(exec_enabled)
            .show_at(ui, rect)
            .on_hover_text("执行或停止当前作战计划");
        if exec_enabled && response.clicked() {
            if army.executing {
                cmds.push(MilitaryCommand::HaltPlan(army.id));
            } else {
                cmds.push(MilitaryCommand::ExecutePlan(army.id));
            }
        }
        let rect = Rect::from_min_size(Pos2::new(right, y), Vec2::new(button_w, 28.0));
        let response = Button::new("清除命令")
            .size(ButtonSize::Sm)
            .variant(ButtonVariant::Secondary)
            .enabled(army.has_path || army.has_arrow)
            .show_at(ui, rect)
            .on_hover_text("清除前线和进攻箭头");
        if (army.has_path || army.has_arrow) && response.clicked() {
            if army.has_arrow {
                cmds.push(MilitaryCommand::ClearArrow(army.id));
            }
            if army.has_path {
                cmds.push(MilitaryCommand::ClearFrontline(army.id));
            }
        }
        y += 36.0;

        for (label, value, color) in [
            (
                "攻击",
                format!("+{:.0}%", army.attack_bonus_pct),
                palette::GOLD,
            ),
            (
                "防御",
                format!("+{:.0}%", army.defense_bonus_pct),
                palette::GOLD,
            ),
            (
                "计划",
                format!("+{:.0}%", army.planning_bonus_pct),
                palette::INFO,
            ),
            (
                "后勤",
                format!("-{:.0}%", army.supply_reduction_pct),
                palette::GOOD,
            ),
        ] {
            ui.painter().text(
                Pos2::new(inner.left(), y),
                Align2::LEFT_TOP,
                label,
                TextRole::Caption.font_id(),
                palette::MUTED,
            );
            ui.painter().text(
                Pos2::new(inner.right(), y),
                Align2::RIGHT_TOP,
                value,
                TextRole::Numeric.font_id(),
                color,
            );
            y += line_h;
        }
        y += spacing::S2;

        let rect = Rect::from_min_size(Pos2::new(inner.left(), y), Vec2::new(inner.width(), 28.0));
        let response = Button::new("加入选中师团")
            .size(ButtonSize::Sm)
            .variant(ButtonVariant::Secondary)
            .enabled(data.selected_division_count > 0)
            .show_at(ui, rect)
            .on_hover_text("把当前选中的师团加入该集团军");
        if data.selected_division_count > 0 && response.clicked() {
            cmds.push(MilitaryCommand::AddMembers(army.id));
        }
        y += 32.0;
        let rect = Rect::from_min_size(Pos2::new(inner.left(), y), Vec2::new(inner.width(), 28.0));
        if Button::new("解散集团军")
            .size(ButtonSize::Sm)
            .variant(ButtonVariant::Danger)
            .show_at(ui, rect)
            .on_hover_text("解散该集团军")
            .clicked()
        {
            cmds.push(MilitaryCommand::DissolveArmy(army.id));
        }
    } else {
        let can_create = data.selected_division_count > 0
            && data.active_army_count < data.max_armies_per_country;
        let rect = Rect::from_min_size(Pos2::new(inner.left(), y), Vec2::new(inner.width(), 30.0));
        let response = Button::new("创建集团军")
            .size(ButtonSize::Md)
            .variant(ButtonVariant::Primary)
            .enabled(can_create)
            .show_at(ui, rect)
            .on_hover_text("需要先选中至少一个师团");
        if can_create && response.clicked() {
            cmds.push(MilitaryCommand::CreateArmy);
        }
    }

    let template_top = (inner.bottom() - 126.0).max(y + spacing::S5);
    ui.painter().hline(
        inner.left()..=inner.right(),
        template_top - spacing::S3,
        egui::Stroke::new(1.0, palette::HAIRLINE),
    );
    ui.painter().text(
        Pos2::new(inner.left(), template_top),
        Align2::LEFT_TOP,
        "模板 / 训练",
        TextRole::Subheading.font_id(),
        palette::BRASS_BRIGHT,
    );
    let mut ty = template_top + 24.0;
    let selected_template = data.template_editor.selected_template;
    let selected_template_ref =
        selected_template.and_then(|idx| data.templates.iter().find(|t| t.index == idx));
    let label = selected_template_ref
        .map(|t| tr(&t.name))
        .unwrap_or("未选择模板");
    ui.painter().text(
        Pos2::new(inner.left(), ty),
        Align2::LEFT_TOP,
        label,
        TextRole::Caption.font_id(),
        palette::PARCHMENT_DIM,
    );
    ty += 22.0;
    let third = (inner.width() - spacing::S3 * 2.0) / 3.0;
    let new_rect = Rect::from_min_size(Pos2::new(inner.left(), ty), Vec2::new(third, 26.0));
    if Button::new("新建")
        .size(ButtonSize::Sm)
        .variant(ButtonVariant::Secondary)
        .show_at(ui, new_rect)
        .clicked()
    {
        cmds.push(MilitaryCommand::NewTemplate);
    }
    let train_rect = Rect::from_min_size(
        Pos2::new(inner.left() + third + spacing::S3, ty),
        Vec2::new(third, 26.0),
    );
    let train_resp = Button::new("训练")
        .size(ButtonSize::Sm)
        .variant(ButtonVariant::Primary)
        .enabled(selected_template.is_some())
        .show_at(ui, train_rect);
    if let Some(idx) = selected_template {
        if train_resp.clicked() {
            cmds.push(MilitaryCommand::Train(idx));
        }
    }
    let edit_rect = Rect::from_min_size(
        Pos2::new(inner.left() + (third + spacing::S3) * 2.0, ty),
        Vec2::new(third, 26.0),
    );
    let edit_resp = Button::new("编辑")
        .size(ButtonSize::Sm)
        .variant(ButtonVariant::Secondary)
        .enabled(selected_template.is_some())
        .show_at(ui, edit_rect);
    if let Some(idx) = selected_template {
        if edit_resp.clicked() {
            cmds.push(MilitaryCommand::OpenTemplateEditor(idx));
        }
    }
}

fn v9_ribbon_text(
    ui: &mut egui::Ui,
    inner: egui::Rect,
    y: &mut f32,
    title: &str,
    body: &str,
    color: Color32,
) {
    use crate::v9::tokens::{palette, TextRole};
    use egui::{Align2, Pos2};

    let painter = ui.painter().with_clip_rect(inner);
    painter.text(
        Pos2::new(inner.left(), *y),
        Align2::LEFT_TOP,
        title,
        TextRole::Subheading.font_id(),
        color,
    );
    *y += 20.0;
    painter.text(
        Pos2::new(inner.left(), *y),
        Align2::LEFT_TOP,
        body,
        TextRole::Caption.font_id(),
        palette::PARCHMENT_DIM,
    );
    *y += 24.0;
}

fn army_status_label(army: &ArmyEntry) -> &'static str {
    let over_limit = army
        .command_limit
        .is_some_and(|limit| army.member_count > limit as usize);
    if over_limit {
        "超限"
    } else if army.executing {
        "执行中"
    } else if army.has_arrow {
        "计划就绪"
    } else if army.has_path {
        "已有前线"
    } else if army.active {
        "行动中"
    } else {
        "待命"
    }
}

fn army_status_color(army: &ArmyEntry) -> Color32 {
    use crate::v9::tokens::palette;
    let over_limit = army
        .command_limit
        .is_some_and(|limit| army.member_count > limit as usize);
    if over_limit {
        palette::BAD
    } else if army.executing {
        palette::GOOD
    } else if army.has_arrow {
        palette::GOLD_HOT
    } else if army.has_path {
        palette::GOLD
    } else if army.active {
        palette::INFO
    } else {
        palette::MUTED
    }
}

fn org_ratio(div: &DivisionEntry) -> f32 {
    if div.max_organisation <= 0.0 {
        0.0
    } else {
        (div.organisation / div.max_organisation).clamp(0.0, 1.0)
    }
}

fn ratio_color(value: f32) -> Color32 {
    use crate::v9::tokens::palette;
    if value < 0.5 {
        palette::BAD
    } else if value < 0.8 {
        palette::WARN
    } else {
        palette::GOOD
    }
}

fn command_show_military(ctx: &egui::Context, data: &MilitaryData) -> (bool, Vec<MilitaryCommand>) {
    let queued = data
        .training_queue
        .iter()
        .map(|item| item.count as usize)
        .sum::<usize>();
    let executing = data.armies.iter().filter(|army| army.executing).count();
    let accent = if matches!(
        data.painter_mode,
        PainterModeInfo::ArmyPainter(_) | PainterModeInfo::ArrowPainter(_)
    ) {
        VanillaIron::WARN
    } else if executing > 0 {
        VanillaIron::GOOD
    } else {
        VanillaIron::BRASS_BRIGHT
    };
    let (close, output) = CommandPanelShell::new("military_command_panel", tr("military"))
        .subtitle("战区 / 集团军 / 师团命令")
        .footer("Q 关闭 | 点击集团军打开详情 | 底部执行命令")
        .accent(accent)
        .show(ctx, |ui, layout| {
            let mut cmds = Vec::new();
            military_command_nav(ui, layout.nav, data, &mut cmds);
            military_command_main(ui, layout.main, data, queued, executing, &mut cmds);
            military_command_strip(ui, layout.bottom_strip, data, &mut cmds);
            cmds
        });

    let mut cmds = output.unwrap_or_default();
    if data.template_editor_open {
        show_template_editor_window(ctx, data, &mut cmds);
    }
    show_subunit_picker_window(ctx, data, &mut cmds);
    (close, cmds)
}

fn military_command_nav(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    data: &MilitaryData,
    cmds: &mut Vec<MilitaryCommand>,
) {
    ui.allocate_ui_at_rect(rect.shrink2(egui::vec2(8.0, 7.0)), |ui| {
        VanillaIron::section_heading(ui, "集团军结构");
        VanillaIron::info_row(
            ui,
            "集团军",
            format!("{}/{}", data.active_army_count, data.max_armies_per_country),
        );
        VanillaIron::info_row(ui, "已选师团", data.selected_division_count.to_string());
        if data.armies.is_empty() {
            ui.add_space(8.0);
            ui.label(
                RichText::new("暂无集团军。选择师团后可在命令条创建。")
                    .small()
                    .color(VanillaIron::MUTED),
            );
            return;
        }
        ui.add_space(8.0);
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for army in &data.armies {
                    let selected = data.selected_army_id == Some(army.id);
                    egui::Frame::new()
                        .fill(if selected {
                            VanillaIron::CARD_SOFT
                        } else {
                            VanillaIron::CARD_DEEP
                        })
                        .stroke(egui::Stroke::new(
                            1.0,
                            if selected {
                                VanillaIron::BRASS_BRIGHT
                            } else {
                                VanillaIron::EDGE_DARK
                            },
                        ))
                        .inner_margin(egui::Margin::symmetric(7, 5))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                let status_color = if army.executing {
                                    VanillaIron::GOOD
                                } else if army.has_arrow {
                                    VanillaIron::BRASS_BRIGHT
                                } else if army.has_path {
                                    VanillaIron::WARN
                                } else {
                                    VanillaIron::MUTED
                                };
                                ui.label(RichText::new("●").color(status_color));
                                if ui
                                    .selectable_label(
                                        selected,
                                        format!("{}（{} 师）", army.name, army.member_count),
                                    )
                                    .on_hover_text("左键选中，右键把当前师团加入")
                                    .clicked()
                                {
                                    cmds.push(MilitaryCommand::SelectArmy(army.id));
                                }
                                if ui.small_button("详情").clicked() {
                                    cmds.push(MilitaryCommand::Panel(PanelCommand::OpenDetail(
                                        ActiveDetailPanel::Army(ArmyDetailTarget {
                                            army_id: army.id,
                                        }),
                                    )));
                                }
                            });
                            ui.label(
                                RichText::new(
                                    army.commander_name.as_deref().unwrap_or("未任命将领"),
                                )
                                .small()
                                .color(VanillaIron::MUTED),
                            );
                        })
                        .response
                        .context_menu(|ui| {
                            if data.selected_division_count > 0
                                && ui.button("加入选中师团").clicked()
                            {
                                cmds.push(MilitaryCommand::AddSelectedDivisionsToArmy(army.id));
                                ui.close_menu();
                            }
                        });
                    ui.add_space(4.0);
                }
            });
    });
}

fn military_command_main(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    data: &MilitaryData,
    queued: usize,
    executing: usize,
    cmds: &mut Vec<MilitaryCommand>,
) {
    ui.allocate_ui_at_rect(rect.shrink2(egui::vec2(8.0, 7.0)), |ui| {
        VanillaIron::section_heading(ui, "作战状态");
        ui.columns(4, |columns| {
            VanillaIron::info_row(&mut columns[0], "师团", data.divisions.len().to_string());
            VanillaIron::info_row(&mut columns[1], "训练中", queued.to_string());
            VanillaIron::value_row(
                &mut columns[2],
                "执行计划",
                executing.to_string(),
                if executing > 0 {
                    VanillaIron::GOOD
                } else {
                    VanillaIron::MUTED
                },
            );
            VanillaIron::info_row(&mut columns[3], "模板", data.templates.len().to_string());
        });
        ui.add_space(8.0);
        if let Some(army_id) = data.selected_army_id {
            if let Some(army) = data.armies.iter().find(|army| army.id == army_id) {
                VanillaIron::section_heading(ui, "选中集团军");
                key_value_military(ui, "名称", &army.name);
                key_value_military(
                    ui,
                    "状态",
                    if army.executing {
                        "执行计划中"
                    } else if army.active {
                        "已激活"
                    } else {
                        "待命"
                    },
                );
                key_value_military(ui, "组织", &format!("{} 个师", army.member_count));
                key_value_military(ui, "前线", if army.has_path { "已有" } else { "无" });
                key_value_military(ui, "进攻箭头", if army.has_arrow { "已有" } else { "无" });
                if VanillaIron::compact_button(ui, "打开军队详情").clicked() {
                    cmds.push(MilitaryCommand::Panel(PanelCommand::OpenDetail(
                        ActiveDetailPanel::Army(ArmyDetailTarget { army_id: army.id }),
                    )));
                }
            }
        } else {
            VanillaIron::warning_row(
                ui,
                "尚未选中集团军。左侧选择集团军，或在地图/底栏选择师团后创建。",
            );
        }
        ui.add_space(8.0);
        VanillaIron::section_heading(ui, "师团状态");
        if data.divisions.is_empty() {
            ui.label(
                RichText::new("暂无师团数据。")
                    .small()
                    .color(VanillaIron::MUTED),
            );
        } else {
            egui::ScrollArea::vertical()
                .max_height((rect.height() - 210.0).max(120.0))
                .show(ui, |ui| {
                    egui::Grid::new("military_command_divisions")
                        .striped(true)
                        .spacing(egui::vec2(8.0, 3.0))
                        .show(ui, |ui| {
                            for div in data.divisions.iter().take(18) {
                                if ui.selectable_label(false, tr(&div.name)).clicked() {
                                    let modifiers = ui.ctx().input(|i| i.modifiers);
                                    if modifiers.shift {
                                        cmds.push(MilitaryCommand::SelectAllDivisions);
                                    } else if modifiers.ctrl {
                                        cmds.push(MilitaryCommand::ToggleDivisionSelection(
                                            div.index,
                                        ));
                                    }
                                }
                                ui.label(
                                    RichText::new(div.army_name.as_deref().unwrap_or("未编入"))
                                        .small()
                                        .color(VanillaIron::MUTED),
                                );
                                ui.label(
                                    RichText::new(format!("组织 {:.0}", div.organisation))
                                        .small()
                                        .color(if div.organisation < div.max_organisation * 0.5 {
                                            VanillaIron::WARN
                                        } else {
                                            VanillaIron::TEXT
                                        }),
                                );
                                ui.label(
                                    RichText::new(format!(
                                        "装备 {:.0}%",
                                        div.equipment_ratio * 100.0
                                    ))
                                    .small()
                                    .color(
                                        if div.equipment_ratio < 0.8 {
                                            VanillaIron::BAD
                                        } else {
                                            VanillaIron::GOOD
                                        },
                                    ),
                                );
                                ui.label(
                                    RichText::new(&div.province_name)
                                        .small()
                                        .color(VanillaIron::MUTED),
                                );
                                ui.end_row();
                            }
                        });
                });
        }
    });
}

fn military_command_strip(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    data: &MilitaryData,
    cmds: &mut Vec<MilitaryCommand>,
) {
    ui.allocate_ui_at_rect(rect.shrink2(egui::vec2(8.0, 7.0)), |ui| {
        ui.horizontal_wrapped(|ui| {
            if VanillaIron::compact_button(ui, "新建模板").clicked() {
                cmds.push(MilitaryCommand::NewTemplate);
            }
            if let Some(idx) = data.template_editor.selected_template {
                if VanillaIron::compact_button(ui, "编辑模板").clicked() {
                    cmds.push(MilitaryCommand::OpenTemplateEditor(idx));
                }
            }
            let can_create = data.selected_division_count > 0
                && data.active_army_count < data.max_armies_per_country;
            if ui
                .add_enabled(can_create, egui::Button::new("创建集团军"))
                .clicked()
            {
                cmds.push(MilitaryCommand::CreateArmy);
            }
            if let Some(army_id) = data.selected_army_id {
                if let Some(army) = data.armies.iter().find(|army| army.id == army_id) {
                    if VanillaIron::compact_button(
                        ui,
                        if army.has_path {
                            "重画前线"
                        } else {
                            "画前线"
                        },
                    )
                    .clicked()
                    {
                        cmds.push(MilitaryCommand::DrawFrontline(army.id));
                    }
                    if ui
                        .add_enabled(army.has_path, egui::Button::new("画进攻箭头"))
                        .clicked()
                    {
                        cmds.push(MilitaryCommand::DrawArrow(army.id));
                    }
                    if army.executing {
                        if VanillaIron::compact_button(ui, "停止计划").clicked() {
                            cmds.push(MilitaryCommand::HaltPlan(army.id));
                        }
                    } else if ui
                        .add_enabled(army.has_arrow, egui::Button::new("执行计划"))
                        .clicked()
                    {
                        cmds.push(MilitaryCommand::ExecutePlan(army.id));
                    }
                    if VanillaIron::compact_button(ui, "清除前线").clicked() {
                        cmds.push(MilitaryCommand::ClearFrontline(army.id));
                    }
                    if VanillaIron::compact_button(ui, "军队详情").clicked() {
                        cmds.push(MilitaryCommand::Panel(PanelCommand::OpenDetail(
                            ActiveDetailPanel::Army(ArmyDetailTarget { army_id: army.id }),
                        )));
                    }
                }
            }
            if VanillaIron::compact_button(ui, "物流").clicked() {
                cmds.push(MilitaryCommand::Panel(PanelCommand::OpenPrimary(
                    ActivePrimaryPanel::Logistics,
                )));
            }
            if VanillaIron::compact_button(
                ui,
                if data.frontline_overlay_visible {
                    "隐藏叠层"
                } else {
                    "显示叠层"
                },
            )
            .clicked()
            {
                cmds.push(MilitaryCommand::ToggleOverlay);
            }
        });
    });
}

fn key_value_military(ui: &mut egui::Ui, key: &str, value: &str) {
    VanillaIron::info_row(ui, key, value.to_owned());
}

impl MilitaryPanel {
    pub fn show_side_panel(
        ctx: &egui::Context,
        data: &MilitaryData,
    ) -> (bool, Vec<MilitaryCommand>) {
        command_show_military(ctx, data)
    }

    pub fn show_bottom_bar(ctx: &egui::Context, data: &MilitaryData) -> Vec<MilitaryCommand> {
        let mut cmds = Vec::new();

        let screen = ctx.screen_rect();
        let visible_armies = data.armies.len().min(8);
        let tray_content_width = if visible_armies == 0 {
            ADD_SLOT_W
        } else {
            visible_armies as f32 * ARMY_CARD_W + (visible_armies as f32 * CARD_GAP_W) + ADD_SLOT_W
        };
        let tray_width = (tray_content_width + 24.0).clamp(112.0, TRAY_MAX_W);
        let area_width = tray_width.max(TOOLBAR_W);
        let area_height = if data.selected_army_id.is_some() {
            116.0
        } else {
            92.0
        };
        let area_pos = egui::pos2(
            screen.center().x - area_width * 0.5,
            screen.bottom() - area_height - BOTTOM_MARGIN,
        );

        egui::Area::new(egui::Id::new("army_bottom_bar"))
            .order(egui::Order::Foreground)
            .fixed_pos(area_pos)
            .show(ctx, |ui| {
                ui.set_min_width(area_width);
                ui.set_max_width(area_width);
                ui.vertical_centered(|ui| {
                    match data.painter_mode {
                        PainterModeInfo::ArmyPainter(_) => {
                            egui::Frame::new()
                                .fill(Color32::from_rgba_premultiplied(0x24, 0x1a, 0x12, 235))
                                .stroke(egui::Stroke::new(1.0, WARN))
                                .inner_margin(egui::Margin::symmetric(8, 4))
                                .show(ui, |ui| {
                                    ui.colored_label(
                                        GOLD,
                                        "正在绘制前线：在地图上拖拽，松开确认（Esc / 右键取消）",
                                    );
                                });
                        }
                        PainterModeInfo::ArrowPainter(_) => {
                            egui::Frame::new()
                                .fill(Color32::from_rgba_premultiplied(0x24, 0x1a, 0x12, 235))
                                .stroke(egui::Stroke::new(1.0, WARN))
                                .inner_margin(egui::Margin::symmetric(8, 4))
                                .show(ui, |ui| {
                                    ui.colored_label(
                                    GOLD,
                                    "正在绘制进攻箭头：在地图上拖拽，松开确认（Esc / 右键取消）",
                                );
                                });
                        }
                        PainterModeInfo::Idle => {}
                    }

                    if let Some(army_id) = data.selected_army_id {
                        if let Some(army) = data.armies.iter().find(|a| a.id == army_id) {
                            ui.horizontal_centered(|ui| {
                                egui::Frame::new()
                                    .fill(Color32::from_rgba_premultiplied(0x24, 0x1a, 0x12, 235))
                                    .stroke(egui::Stroke::new(
                                        1.0,
                                        STROKE_DARK,
                                    ))
                                    .inner_margin(egui::Margin::symmetric(6, 4))
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            ui.set_height(28.0);
                                            ui.spacing_mut().interact_size.y = 28.0;
                                            egui::ComboBox::from_id_salt(("army_commander", army.id))
                                                .width(118.0)
                                                .height(220.0)
                                                .selected_text(
                                                    army.commander_name
                                                        .as_deref()
                                                        .unwrap_or("任命将领"),
                                                )
                                                .show_ui(ui, |ui| {
                                                    if ui
                                                        .selectable_label(
                                                            army.commander.is_none(),
                                                            "无将领",
                                                        )
                                                        .clicked()
                                                    {
                                                        cmds.push(MilitaryCommand::UnassignGeneral(
                                                            army.id,
                                                        ));
                                                    }
                                                    for general in &data.generals {
                                                        let assigned_elsewhere = general
                                                            .assigned_army_id
                                                            .is_some_and(|id| id != army.id);
                                                        let label = format!(
                                                            "{}  Lv{}  {}/{}",
                                                            general.name,
                                                            general.skill,
                                                            army.member_count,
                                                            general.command_limit
                                                        );
                                                        if ui
                                                            .add_enabled(
                                                                !assigned_elsewhere,
                                                                egui::SelectableLabel::new(
                                                                    army.commander == Some(general.id),
                                                                    label,
                                                                ),
                                                            )
                                                            .on_disabled_hover_text(
                                                                "该将领已指挥其他集团军",
                                                            )
                                                        .on_hover_text(format!(
                                                                "攻{} 防{} 计划{} 后勤{}\n指挥上限 {}，超限会降低将领加成",
                                                                general.attack,
                                                                general.defense,
                                                                general.planning,
                                                                general.logistics,
                                                                general.command_limit
                                                            ))
                                                            .clicked()
                                                        {
                                                            cmds.push(
                                                                MilitaryCommand::AssignGeneral {
                                                                    army_id: army.id,
                                                                    general_id: general.id,
                                                                },
                                                            );
                                                        }
                                                    }
                                                });
                                            ui.separator();
                                            if tool_button(
                                                ui,
                                                true,
                                                "画线",
                                                if army.has_path {
                                                    "重绘前线"
                                                } else {
                                                    "画前线"
                                                },
                                            )
                                            .clicked()
                                            {
                                                cmds.push(MilitaryCommand::DrawFrontline(army.id));
                                            }
                                            if tool_button(
                                                ui,
                                                army.has_path,
                                                "箭头",
                                                if army.has_arrow {
                                                    "重绘进攻箭头"
                                                } else {
                                                    "画进攻箭头"
                                                },
                                            )
                                            .on_disabled_hover_text("请先绘制前线")
                                            .clicked()
                                            {
                                                cmds.push(MilitaryCommand::DrawArrow(army.id));
                                            }
                                            if army.executing {
                                                if tool_button(ui, true, "停止", "停止计划").clicked()
                                                {
                                                    cmds.push(MilitaryCommand::HaltPlan(army.id));
                                                }
                                            } else if tool_button(
                                                ui,
                                                army.has_arrow,
                                                "执行",
                                                "执行计划",
                                            )
                                            .on_disabled_hover_text("请先绘制进攻箭头")
                                            .clicked()
                                            {
                                                cmds.push(MilitaryCommand::ExecutePlan(army.id));
                                            }
                                            if tool_button(ui, army.has_path, "清线", "清除前线")
                                                .clicked()
                                            {
                                                cmds.push(MilitaryCommand::ClearFrontline(army.id));
                                            }
                                            if tool_button(ui, army.has_arrow, "清箭", "清除箭头")
                                                .clicked()
                                            {
                                                cmds.push(MilitaryCommand::ClearArrow(army.id));
                                            }
                                            ui.separator();
                                            if tool_button(
                                                ui,
                                                data.selected_division_count > 0,
                                                "加师",
                                                "加入选中师团",
                                            )
                                            .on_disabled_hover_text("请先选择师团")
                                            .clicked()
                                            {
                                                cmds.push(MilitaryCommand::AddMembers(army.id));
                                            }
                                            if tool_button(
                                                ui,
                                                data.selected_division_count > 0,
                                                "移师",
                                                "移出选中师团",
                                            )
                                            .on_disabled_hover_text("请先选择师团")
                                            .clicked()
                                            {
                                                cmds.push(MilitaryCommand::RemoveMembers(army.id));
                                            }
                                            if tool_button(ui, true, "解散", "解散集团军").clicked()
                                            {
                                                cmds.push(MilitaryCommand::DissolveArmy(army.id));
                                            }
                                        });
                                    });
                            });
                        }
                    } else {
                        ui.add_space(24.0);
                    }

                    ui.add_space(2.0);
                    ui.horizontal_centered(|ui| {
                        egui::Frame::new()
                            .fill(Color32::from_rgba_premultiplied(0x24, 0x1a, 0x12, 230))
                            .stroke(egui::Stroke::new(1.0, STROKE_DARK))
                            .inner_margin(egui::Margin::symmetric(8, 5))
                            .show(ui, |ui| {
                                ui.spacing_mut().item_spacing.x = CARD_GAP_W;
                                ui.horizontal(|ui| {
                                    for army in data.armies.iter().take(8) {
                                        let selected = data.selected_army_id == Some(army.id);
                                        let resp = army_card(ui, army, selected);
                                        if resp.clicked() {
                                            cmds.push(MilitaryCommand::SelectArmy(army.id));
                                        }
                                        if resp.secondary_clicked() && data.selected_division_count > 0 {
                                            cmds.push(MilitaryCommand::AddSelectedDivisionsToArmy(army.id));
                                        }
                                    }
                                    let can_create = data.selected_division_count > 0
                                        && data.active_army_count < data.max_armies_per_country;
                                    if add_army_slot(ui, data).clicked() && can_create {
                                        cmds.push(MilitaryCommand::CreateArmy);
                                    }
                                });
                            });

                        let overlay_text = if data.frontline_overlay_visible {
                            "ORD"
                        } else {
                            "ord"
                        };
                        if ui
                            .small_button(overlay_text)
                            .on_hover_text("显示/隐藏作战叠层")
                            .clicked()
                        {
                            cmds.push(MilitaryCommand::ToggleOverlay);
                        }
                    });
                });
            });

        cmds
    }
}
