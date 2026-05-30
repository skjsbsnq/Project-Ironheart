//! V5 阶段 C.4：科研面板。
//!
//! 多类别科技树：节点、进度条、年份惩罚和解锁预览。

use crate::{components, i18n::tr};
use egui::{Color32, RichText};

use components::{MUTED, PANEL_CARD_SOFT, PARCHMENT, STROKE_TILE, WARN};

/// 科研类别。
pub const CATEGORIES: &[&str] = &[
    "industry",
    "chemistry",
    "electrical",
    "metallurgy",
    "military_doctrine",
    "air",
    "naval",
    "social_science",
    "information_control",
];

fn category_label(category: &str) -> &'static str {
    match category {
        "industry" => "工业",
        "chemistry" => "化工",
        "electrical" => "电气",
        "metallurgy" => "冶金",
        "military_doctrine" => "军事学说",
        "air" => "航空",
        "naval" => "海军",
        "social_science" => "社会科学",
        "information_control" => "信息管制",
        _ => "科技",
    }
}

/// 单个科技节点 UI 数据。
pub struct TechNode {
    pub key: String,
    pub name: String,
    pub category: String,
    pub start_year: u16,
    pub completed: bool,
    pub researching: bool,
    pub progress: f32, // 0..1, only meaningful if researching
    pub prerequisites: Vec<String>,
    pub unlock_summary: Vec<String>,
}

/// 研究槽位状态。
pub struct SlotEntry {
    pub tech_key: String,
    pub progress: f32, // 0..1
}

/// 科研面板每帧数据。
pub struct ResearchData {
    pub current_year: u16,
    pub slots: Vec<SlotEntry>,
    pub slot_count: usize,
    pub techs: Vec<TechNode>,
}

/// 科研面板命令。
#[derive(Debug)]
pub enum ResearchCommand {
    /// 开始研究某科技
    StartResearch(String),
}

pub struct ResearchPanel;

impl ResearchPanel {
    /// 返回 (close_requested, commands)。
    #[allow(unreachable_code)]
    pub fn show(ctx: &egui::Context, data: &ResearchData) -> (bool, Vec<ResearchCommand>) {
        return v9_show_research(ctx, data);

        let mut close = false;
        let mut cmds = Vec::new();
        let search_id = egui::Id::new("research_panel_search");
        let mut search = ctx
            .data_mut(|d| d.get_persisted::<String>(search_id))
            .unwrap_or_default();
        egui::SidePanel::left("research_panel")
            .default_width(620.0)
            .min_width(480.0)
            .resizable(true)
            .show(ctx, |ui| {
                components::panel_header(ui, tr("research"), &mut close);

                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let idle_slots = data.slot_count.saturating_sub(data.slots.len());
                        components::section(ui, tr("active_research"), |ui| {
                            if data.slots.is_empty() {
                                components::panel_hint(
                                    ui,
                                    tr("slots_idle").replace("{}", &data.slot_count.to_string()),
                                );
                            } else {
                                for slot in &data.slots {
                                    let tech_name = tr(&slot.tech_key);
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            RichText::new(tech_name)
                                                .size(12.0)
                                                .color(PARCHMENT)
                                                .strong(),
                                        );
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                ui.add(
                                                    egui::ProgressBar::new(slot.progress)
                                                        .desired_width(160.0),
                                                );
                                            },
                                        );
                                    });
                                }
                            }
                            if idle_slots > 0 {
                                ui.add_space(2.0);
                                ui.label(
                                    RichText::new(
                                        tr("slot_idle").replace("{}", &idle_slots.to_string()),
                                    )
                                    .size(11.0)
                                    .color(WARN),
                                );
                            }
                        });

                        ui.add_space(4.0);
                        ui.horizontal_wrapped(|ui| {
                            ui.label(RichText::new("搜索").size(11.0).color(MUTED));
                            ui.add(
                                egui::TextEdit::singleline(&mut search)
                                    .hint_text("科技、类别、前置或解锁内容")
                                    .desired_width(240.0),
                            );
                            if !search.is_empty() && ui.small_button("清空").clicked() {
                                search.clear();
                            }
                        });

                        // 按类别显示科技树
                        for cat in CATEGORIES {
                            let cat_techs: Vec<&TechNode> = data
                                .techs
                                .iter()
                                .filter(|t| t.category == *cat && tech_matches_search(t, &search))
                                .collect();
                            if cat_techs.is_empty() {
                                continue;
                            }

                            components::section(ui, category_label(cat), |ui| {
                                for tech in cat_techs {
                                    render_tech_node(
                                        ui,
                                        tech,
                                        data.current_year,
                                        idle_slots,
                                        &mut cmds,
                                    );
                                    ui.add_space(3.0);
                                }
                            });
                        }
                    });
            });
        ctx.data_mut(|d| d.insert_persisted(search_id, search));
        (close, cmds)
    }
}

fn v9_show_research(ctx: &egui::Context, data: &ResearchData) -> (bool, Vec<ResearchCommand>) {
    use crate::v9::composites::panel_shell::{
        draw_summary_tiles, draw_tab_strip, PanelClass, PanelShell,
    };
    use crate::v9::tokens::palette;

    let selected_id = egui::Id::new("research_panel_v9_selected");
    let mut selected_key = ctx
        .data_mut(|d| d.get_persisted::<Option<String>>(selected_id))
        .unwrap_or(None);
    if selected_key
        .as_ref()
        .map(|key| data.techs.iter().any(|tech| &tech.key == key))
        .unwrap_or(false)
        == false
    {
        selected_key = data
            .techs
            .iter()
            .find(|tech| tech.researching)
            .or_else(|| data.techs.iter().find(|tech| !tech.completed))
            .or_else(|| data.techs.first())
            .map(|tech| tech.key.clone());
    }

    let idle_slots = data.slot_count.saturating_sub(data.slots.len());
    let completed = data.techs.iter().filter(|tech| tech.completed).count();
    let ahead = data
        .techs
        .iter()
        .filter(|tech| !tech.completed && tech.start_year > data.current_year)
        .count();
    let accent = if idle_slots > 0 {
        palette::WARN
    } else {
        palette::INFO
    };

    let (close, output) = PanelShell::new("research_panel_v9", tr("research"))
        .subtitle("科研树 / 解锁影响预览")
        .class(PanelClass::Detail)
        .accent(accent)
        .footer("Q 关闭  |  选择科技 / 开始科研")
        .show(ctx, |ui, layout| {
            draw_summary_tiles(
                ui,
                layout.summary,
                &[
                    ("年份", data.current_year.to_string(), palette::GOLD),
                    (
                        "槽位",
                        format!("{}/{}", data.slots.len(), data.slot_count),
                        if idle_slots > 0 {
                            palette::WARN
                        } else {
                            palette::GOOD
                        },
                    ),
                    (
                        "空闲",
                        idle_slots.to_string(),
                        if idle_slots > 0 {
                            palette::WARN
                        } else {
                            palette::GOOD
                        },
                    ),
                    (
                        tr("completed"),
                        format!("{}/{}", completed, data.techs.len()),
                        palette::GOOD,
                    ),
                    (
                        "超前",
                        ahead.to_string(),
                        if ahead > 0 {
                            palette::WARN
                        } else {
                            palette::MUTED
                        },
                    ),
                    (
                        tr("tech_categories"),
                        CATEGORIES.len().to_string(),
                        palette::INFO,
                    ),
                ],
            );
            draw_tab_strip(ui, layout.tabs, "科研树 / 影响预览", accent);
            let mut cmds = Vec::new();
            v9_research_body(
                ui,
                layout.body,
                data,
                idle_slots,
                &mut selected_key,
                &mut cmds,
            );
            cmds
        });
    ctx.data_mut(|d| d.insert_persisted(selected_id, selected_key));
    (close, output.unwrap_or_default())
}

fn v9_research_body(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    data: &ResearchData,
    idle_slots: usize,
    selected_key: &mut Option<String>,
    cmds: &mut Vec<ResearchCommand>,
) {
    use crate::v9::layout::{GridLayout, Track};
    use crate::v9::tokens::spacing;

    ui.allocate_ui_at_rect(rect, |ui| {
        let grid = GridLayout::new(vec![Track::Fr(1.0)], vec![Track::Fr(0.68), Track::Fr(0.32)])
            .with_gutter(spacing::S5, 0.0);
        let cells = grid.measure(rect);
        v9_research_tree(ui, GridLayout::cell(&cells, 0, 0), data, selected_key);
        let selected = selected_key
            .as_ref()
            .and_then(|key| data.techs.iter().find(|tech| &tech.key == key))
            .or_else(|| data.techs.iter().find(|tech| tech.researching))
            .or_else(|| data.techs.first());
        v9_research_detail(
            ui,
            GridLayout::cell(&cells, 0, 1),
            data,
            selected,
            idle_slots,
            cmds,
        );
    });
}

fn v9_research_tree(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    data: &ResearchData,
    selected_key: &mut Option<String>,
) {
    use crate::v9::primitives::{Card, TreeLayout};
    use crate::v9::tokens::{palette, spacing, TextRole};
    use egui::{Align2, Pos2, Rect, Sense, Stroke, Vec2};

    let inner = Card::new().as_panel().show_at(ui, rect);
    ui.painter().text(
        inner.left_top(),
        Align2::LEFT_TOP,
        "科技树",
        TextRole::Heading.font_id(),
        palette::BRASS_BRIGHT,
    );

    let tree = TreeLayout::new(Vec2::new(118.0, 42.0), Vec2::new(132.0, 52.0));
    let origin = Pos2::new(inner.left() + 116.0, inner.top() + 42.0);
    for (cat_idx, cat) in CATEGORIES.iter().enumerate() {
        let y = origin.y + cat_idx as f32 * tree.gap.y;
        ui.painter().text(
            Pos2::new(inner.left(), y + 16.0),
            Align2::LEFT_CENTER,
            category_label(cat),
            TextRole::Caption.font_id(),
            palette::PARCHMENT_DIM,
        );
        let cat_techs: Vec<&TechNode> = data
            .techs
            .iter()
            .filter(|tech| tech.category == *cat)
            .take(5)
            .collect();
        for (idx, tech) in cat_techs.iter().enumerate() {
            let node = tree.node_rect(origin, 0, 0, (idx as i32, cat_idx as i32), 1.0);
            if node.right() > inner.right() || node.bottom() > inner.bottom() {
                continue;
            }
            if idx > 0 {
                let prev = tree.node_rect(origin, 0, 0, (idx as i32 - 1, cat_idx as i32), 1.0);
                ui.painter().line_segment(
                    [prev.right_center(), node.left_center()],
                    Stroke::new(1.0, palette::BRASS_DARK),
                );
            }
            let response = ui.interact(
                node,
                ui.id().with(("research_node_v9", tech.key.as_str())),
                Sense::click(),
            );
            if response.clicked() {
                *selected_key = Some(tech.key.clone());
            }
            let selected = selected_key.as_deref() == Some(tech.key.as_str());
            let accent = if tech.completed {
                palette::GOOD
            } else if tech.researching {
                palette::WARN
            } else if tech.start_year > data.current_year {
                palette::BAD
            } else {
                palette::INFO
            };
            crate::v9::paint::paint_bevel(
                ui.painter(),
                node,
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
                Rect::from_min_size(node.left_top(), Vec2::new(4.0, node.height())),
                egui::epaint::CornerRadius::ZERO,
                accent,
            );
            ui.painter().text(
                Pos2::new(node.left() + spacing::S4, node.top() + spacing::S2),
                Align2::LEFT_TOP,
                tech.name.as_str(),
                TextRole::Caption.font_id(),
                if selected {
                    palette::GOLD_HOT
                } else {
                    palette::PARCHMENT
                },
            );
            ui.painter().text(
                Pos2::new(node.left() + spacing::S4, node.bottom() - spacing::S2),
                Align2::LEFT_BOTTOM,
                format!("{}  {:.0}%", tech.start_year, tech.progress * 100.0),
                TextRole::Small.font_id(),
                accent,
            );
        }
    }
}

fn v9_research_detail(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    data: &ResearchData,
    tech: Option<&TechNode>,
    idle_slots: usize,
    cmds: &mut Vec<ResearchCommand>,
) {
    use crate::v9::primitives::{
        draw_progress_bar, Button, ButtonSize, ButtonVariant, Card, DataTable, TableCell,
        TableColumn, TableRow,
    };
    use crate::v9::tokens::{palette, TextRole};
    use egui::{Align2, Pos2, Rect, Vec2};

    let inner = Card::new().as_panel().show_at(ui, rect);
    ui.painter().text(
        inner.left_top(),
        Align2::LEFT_TOP,
        "科研预览",
        TextRole::Heading.font_id(),
        palette::BRASS_BRIGHT,
    );
    let Some(tech) = tech else {
        crate::v9::composites::panel_shell::draw_empty_state(
            ui,
            Rect::from_min_max(
                Pos2::new(inner.left(), inner.top() + 34.0),
                inner.right_bottom(),
            ),
            "未选择科技",
            "请选择一个科技节点查看详情。",
        );
        return;
    };

    let mut y = inner.top() + 34.0;
    ui.painter().text(
        Pos2::new(inner.left(), y),
        Align2::LEFT_TOP,
        tech.name.as_str(),
        TextRole::Subheading.font_id(),
        palette::GOLD_HOT,
    );
    y += 26.0;
    let status = if tech.completed {
        "已完成"
    } else if tech.researching {
        "研究中"
    } else if tech.start_year > data.current_year {
        "超前"
    } else {
        "可研究"
    };
    ui.painter().text(
        Pos2::new(inner.left(), y),
        Align2::LEFT_TOP,
        format!(
            "{} / {} / {}",
            category_label(&tech.category),
            tech.start_year,
            status
        ),
        TextRole::Caption.font_id(),
        palette::PARCHMENT_DIM,
    );
    y += 26.0;

    draw_progress_bar(
        ui,
        Rect::from_min_size(Pos2::new(inner.left(), y), Vec2::new(inner.width(), 10.0)),
        if tech.completed { 1.0 } else { tech.progress },
        if tech.completed {
            palette::GOOD
        } else if tech.researching {
            palette::WARN
        } else {
            palette::INFO
        },
    );
    y += 26.0;

    let prereq = if tech.prerequisites.is_empty() {
        "前置科技：无".to_owned()
    } else {
        format!("前置科技：{}", tech.prerequisites.join(", "))
    };
    let galley = ui.painter().layout(
        prereq,
        TextRole::Caption.font_id(),
        palette::PARCHMENT_DIM,
        inner.width(),
    );
    ui.painter()
        .galley(Pos2::new(inner.left(), y), galley, palette::PARCHMENT_DIM);
    y += 46.0;

    let unlock_rows: Vec<TableRow> = if tech.unlock_summary.is_empty() {
        vec![TableRow::new(vec![
            TableCell::strong("解锁"),
            TableCell::new("无直接解锁预览"),
        ])]
    } else {
        tech.unlock_summary
            .iter()
            .take(7)
            .map(|unlock| {
                TableRow::new(vec![
                    TableCell::strong("解锁"),
                    TableCell::new(unlock.as_str()),
                ])
            })
            .collect()
    };
    DataTable::new(
        vec![
            TableColumn::new("类型", 0.55),
            TableColumn::new("效果", 1.45),
        ],
        unlock_rows,
    )
    .row_height(27.0)
    .show_at(
        ui,
        Rect::from_min_size(Pos2::new(inner.left(), y), Vec2::new(inner.width(), 222.0)),
    );
    y += 236.0;

    let can_start = !tech.completed && !tech.researching && idle_slots > 0;
    let button_rect = Rect::from_min_size(
        Pos2::new(inner.left(), y),
        Vec2::new(inner.width(), ButtonSize::Md.min_size().y),
    );
    if can_start
        && Button::new(tr("start_research"))
            .size(ButtonSize::Md)
            .variant(ButtonVariant::Primary)
            .show_at(ui, button_rect)
            .clicked()
    {
        cmds.push(ResearchCommand::StartResearch(tech.key.clone()));
    } else if !can_start {
        Button::new(if tech.completed {
            "已完成"
        } else if tech.researching {
            "研究中"
        } else {
            "无空闲槽位"
        })
        .size(ButtonSize::Md)
        .variant(ButtonVariant::Ghost)
        .enabled(false)
        .show_at(ui, button_rect);
    }

    let slot_rows: Vec<TableRow> = data
        .slots
        .iter()
        .take(4)
        .map(|slot| {
            TableRow::new(vec![
                TableCell::strong(tr(&slot.tech_key)),
                TableCell::new(format!("{:.0}%", slot.progress * 100.0)).right(),
            ])
        })
        .collect();
    DataTable::new(
        vec![
            TableColumn::new(tr("active_research"), 1.2),
            TableColumn::new(tr("progress"), 0.8).right(),
        ],
        slot_rows,
    )
    .row_height(27.0)
    .show_at(
        ui,
        Rect::from_min_max(
            Pos2::new(inner.left(), inner.bottom() - 136.0),
            Pos2::new(inner.right(), inner.bottom()),
        ),
    );
}

fn render_tech_node(
    ui: &mut egui::Ui,
    tech: &TechNode,
    current_year: u16,
    idle_slots: usize,
    cmds: &mut Vec<ResearchCommand>,
) {
    egui::Frame::new()
        .fill(PANEL_CARD_SOFT)
        .stroke(egui::Stroke::new(1.0, STROKE_TILE))
        .inner_margin(egui::Margin::symmetric(10, 7))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let (status, color) = if tech.completed {
                    ("✓", components::GOOD)
                } else if tech.researching {
                    ("⟳", WARN)
                } else {
                    ("○", MUTED)
                };
                ui.label(RichText::new(status).color(color).size(14.0).strong());
                ui.label(
                    RichText::new(&tech.name)
                        .size(12.5)
                        .color(PARCHMENT)
                        .strong(),
                );
                ui.label(
                    RichText::new(format!("({})", tech.start_year))
                        .size(10.5)
                        .color(MUTED),
                );

                if !tech.completed && tech.start_year > current_year {
                    let penalty = (tech.start_year - current_year) as f32 * 2.0;
                    ui.label(
                        RichText::new(
                            tr("aot_penalty").replace("{:.0}", &format!("{:.0}", penalty)),
                        )
                        .size(10.5)
                        .color(components::BAD),
                    );
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if !tech.completed && !tech.researching && idle_slots > 0 {
                        if components::action_button(ui, true, tr("start_research")).clicked() {
                            cmds.push(ResearchCommand::StartResearch(tech.key.clone()));
                        }
                    } else if !tech.completed && !tech.researching && idle_slots == 0 {
                        ui.label(RichText::new("无空闲研究槽").size(10.5).color(WARN));
                    }
                    if tech.researching {
                        ui.add(egui::ProgressBar::new(tech.progress).desired_width(80.0));
                    }
                });
            });
            // 前置 / 解锁
            ui.add_space(2.0);
            let prereq_text = if tech.prerequisites.is_empty() {
                "前置：暂无前置要求".to_owned()
            } else {
                format!("前置：{}", tech.prerequisites.join("、"))
            };
            ui.label(RichText::new(prereq_text).size(10.5).color(MUTED));
            let unlock_text = if tech.unlock_summary.is_empty() {
                "解锁：无直接解锁项，主要提供科研链路或数值修正。".to_owned()
            } else {
                format!("解锁：{}", tech.unlock_summary.join("、"))
            };
            ui.label(RichText::new(unlock_text).size(10.5).color(MUTED));
            ui.label(
                RichText::new(format!(
                    "预期：开始后占用 1 个研究槽；年份 {}，当前年份 {}。",
                    tech.start_year, current_year
                ))
                .size(10.0)
                .color(Color32::from_gray(140)),
            );
        });
}

fn tech_matches_search(tech: &TechNode, search: &str) -> bool {
    let query = search.trim().to_lowercase();
    if query.is_empty() {
        return true;
    }
    tech.name.to_lowercase().contains(&query)
        || category_label(&tech.category)
            .to_lowercase()
            .contains(&query)
        || tech
            .prerequisites
            .iter()
            .any(|p| p.to_lowercase().contains(&query))
        || tech
            .unlock_summary
            .iter()
            .any(|u| u.to_lowercase().contains(&query))
}
