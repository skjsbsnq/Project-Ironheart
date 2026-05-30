//! V6.D 计划经济专属 UI：五年计划面板（目标 vs 实际 vs 配给率）。

use crate::i18n::tr;
use egui::{Color32, RichText};

#[derive(Debug, Clone)]
pub struct PlanTargetEntry {
    pub good_id: String,
    pub good_name: String,
    pub target_output: f32,
    pub actual_output: f32,
    pub ration_rate: f32,
}

#[derive(Debug, Clone)]
pub struct PyatiletkaPanelData {
    pub plan_name: String,
    pub plan_period: String,
    pub is_planned_economy: bool,
    pub targets: Vec<PlanTargetEntry>,
    pub focus_directions: Vec<String>,
    pub focus_bonus: f32,
    pub off_focus_penalty: f32,
}

pub struct PyatiletkaPanel;

impl PyatiletkaPanel {
    #[allow(unreachable_code)]
    pub fn show(ctx: &egui::Context, data: &PyatiletkaPanelData) -> bool {
        return v9_show_pyatiletka(ctx, data);

        let mut close = false;

        if !data.is_planned_economy {
            return close;
        }

        egui::SidePanel::left("pyatiletka_panel")
            .default_width(380.0)
            .resizable(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.heading(
                        RichText::new(tr("v6_pyatiletka_title"))
                            .color(Color32::from_rgb(0xcc, 0x33, 0x33)),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("✕").clicked() {
                            close = true;
                        }
                    });
                });

                ui.label(RichText::new(&data.plan_name).color(Color32::from_rgb(0xff, 0xcc, 0x80)));
                ui.label(
                    RichText::new(&data.plan_period)
                        .small()
                        .color(Color32::from_gray(160)),
                );
                ui.add_space(4.0);
                ui.separator();

                ui.label(
                    RichText::new(tr("v6_pyatiletka_targets"))
                        .color(Color32::from_rgb(0xe0, 0xc0, 0x78)),
                );
                ui.add_space(2.0);

                egui::ScrollArea::vertical().show(ui, |ui| {
                    for entry in &data.targets {
                        render_plan_target(ui, entry);
                    }
                });

                ui.add_space(4.0);
                ui.separator();

                ui.label(
                    RichText::new(tr("v6_pyatiletka_research"))
                        .color(Color32::from_rgb(0xe0, 0xc0, 0x78)),
                );
                ui.add_space(2.0);

                for dir in &data.focus_directions {
                    ui.horizontal(|ui| {
                        ui.colored_label(Color32::from_rgb(0x60, 0xc0, 0x60), "★");
                        ui.label(RichText::new(dir).small());
                    });
                }

                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!(
                            "{}: +{:.0}%",
                            tr("v6_pyatiletka_focus_bonus"),
                            data.focus_bonus * 100.0
                        ))
                        .small()
                        .color(Color32::from_rgb(0x60, 0xc0, 0x60)),
                    );
                    ui.label(
                        RichText::new(format!(
                            "{}: {:.0}%",
                            tr("v6_pyatiletka_off_penalty"),
                            data.off_focus_penalty * 100.0
                        ))
                        .small()
                        .color(Color32::from_rgb(0xc0, 0x60, 0x60)),
                    );
                });
            });

        close
    }
}

fn v9_show_pyatiletka(ctx: &egui::Context, data: &PyatiletkaPanelData) -> bool {
    use crate::v9::composites::panel_shell::{
        draw_summary_tiles, draw_tab_strip, PanelClass, PanelShell,
    };
    use crate::v9::tokens::palette;

    if !data.is_planned_economy {
        return false;
    }

    let average_progress = v9_plan_average_progress(data);
    let shortage_count = data
        .targets
        .iter()
        .filter(|entry| entry.ration_rate < 1.0)
        .count();
    let accent = if average_progress >= 0.90 && shortage_count == 0 {
        palette::GOOD
    } else if average_progress >= 0.55 {
        palette::WARN
    } else {
        palette::BAD
    };

    let (close, _) = PanelShell::new("pyatiletka_panel_v9", tr("v6_pyatiletka_title"))
        .subtitle(data.plan_name.as_str())
        .class(PanelClass::Compact)
        .accent(accent)
        .footer("Q Close  |  Inspect targets / research focus")
        .show(ctx, |ui, layout| {
            draw_summary_tiles(
                ui,
                layout.summary,
                &[
                    ("Period", data.plan_period.clone(), palette::GOLD),
                    ("Targets", data.targets.len().to_string(), palette::INFO),
                    ("Progress", v9_plan_percent(average_progress), accent),
                    (
                        "Shortages",
                        shortage_count.to_string(),
                        if shortage_count > 0 {
                            palette::WARN
                        } else {
                            palette::GOOD
                        },
                    ),
                    (
                        "Focus bonus",
                        v9_plan_signed_percent(data.focus_bonus),
                        palette::GOOD,
                    ),
                    (
                        "Off focus",
                        v9_plan_signed_percent(data.off_focus_penalty),
                        palette::BAD,
                    ),
                ],
            );
            draw_tab_strip(
                ui,
                layout.tabs,
                "Target DataTable / Focus ImpactPreview",
                accent,
            );
            v9_pyatiletka_body(ui, layout.body, data);
        });

    close
}

fn v9_pyatiletka_body(ui: &mut egui::Ui, rect: egui::Rect, data: &PyatiletkaPanelData) {
    use crate::v9::layout::{GridLayout, Track};
    use crate::v9::tokens::spacing;

    ui.allocate_ui_at_rect(rect, |ui| {
        let grid = GridLayout::new(vec![Track::Fr(1.0)], vec![Track::Fr(0.64), Track::Fr(0.36)])
            .with_gutter(spacing::S5, 0.0);
        let cells = grid.measure(rect);
        v9_plan_target_table(ui, GridLayout::cell(&cells, 0, 0), data);
        v9_plan_focus_panel(ui, GridLayout::cell(&cells, 0, 1), data);
    });
}

fn v9_plan_target_table(ui: &mut egui::Ui, rect: egui::Rect, data: &PyatiletkaPanelData) {
    use crate::v9::primitives::{Card, DataTable, TableCell, TableColumn, TableRow};
    use crate::v9::tokens::{palette, TextRole};
    use egui::{Align2, Pos2, Rect};

    let inner = Card::new().as_panel().show_at(ui, rect);
    ui.painter().text(
        inner.left_top(),
        Align2::LEFT_TOP,
        "Production targets",
        TextRole::Heading.font_id(),
        palette::BRASS_BRIGHT,
    );

    if data.targets.is_empty() {
        crate::v9::composites::panel_shell::draw_empty_state(
            ui,
            Rect::from_min_max(
                Pos2::new(inner.left(), inner.top() + 34.0),
                inner.right_bottom(),
            ),
            "No plan targets",
            "The planned economy has no active target goods.",
        );
        return;
    }

    let rows: Vec<TableRow> = data
        .targets
        .iter()
        .take(18)
        .map(|entry| {
            let progress = v9_plan_target_progress(entry);
            let accent = v9_plan_progress_color(progress, entry.ration_rate);
            TableRow::new(vec![
                TableCell::strong(entry.good_name.as_str()),
                TableCell::new(format!("{:.0}", entry.actual_output)).right(),
                TableCell::new(format!("{:.0}", entry.target_output)).right(),
                TableCell::colored(v9_plan_percent(progress), accent).right(),
                TableCell::colored(
                    v9_plan_percent(entry.ration_rate),
                    v9_plan_ration_color(entry.ration_rate),
                )
                .right(),
            ])
            .accent(accent)
        })
        .collect();

    DataTable::new(
        vec![
            TableColumn::new("Good", 1.35),
            TableColumn::new("Actual", 0.72).right(),
            TableColumn::new("Target", 0.72).right(),
            TableColumn::new("Done", 0.58).right(),
            TableColumn::new("Ration", 0.62).right(),
        ],
        rows,
    )
    .row_height(28.0)
    .show_at(
        ui,
        Rect::from_min_max(
            Pos2::new(inner.left(), inner.top() + 34.0),
            inner.right_bottom(),
        ),
    );
}

fn v9_plan_focus_panel(ui: &mut egui::Ui, rect: egui::Rect, data: &PyatiletkaPanelData) {
    use crate::v9::primitives::{
        draw_progress_bar, Card, DataTable, TableCell, TableColumn, TableRow,
    };
    use crate::v9::tokens::{palette, spacing, TextRole};
    use egui::{Align2, Pos2, Rect, Vec2};

    let inner = Card::new().as_panel().show_at(ui, rect);
    ui.painter().text(
        inner.left_top(),
        Align2::LEFT_TOP,
        "Focus preview",
        TextRole::Heading.font_id(),
        palette::BRASS_BRIGHT,
    );

    let mut y = inner.top() + 34.0;
    let progress_rows: Vec<&PlanTargetEntry> = data.targets.iter().take(7).collect();
    if progress_rows.is_empty() {
        crate::v9::composites::panel_shell::draw_empty_state(
            ui,
            Rect::from_min_size(Pos2::new(inner.left(), y), Vec2::new(inner.width(), 110.0)),
            "No targets",
            "Target progress bars will appear here.",
        );
        y += 122.0;
    } else {
        for entry in progress_rows {
            let progress = v9_plan_target_progress(entry);
            let color = v9_plan_progress_color(progress, entry.ration_rate);
            ui.painter().text(
                Pos2::new(inner.left(), y),
                Align2::LEFT_TOP,
                entry.good_name.as_str(),
                TextRole::Caption.font_id(),
                palette::PARCHMENT,
            );
            ui.painter().text(
                Pos2::new(inner.right(), y),
                Align2::RIGHT_TOP,
                v9_plan_percent(progress),
                TextRole::Numeric.font_id(),
                color,
            );
            draw_progress_bar(
                ui,
                Rect::from_min_size(
                    Pos2::new(inner.left(), y + 17.0),
                    Vec2::new(inner.width(), 9.0),
                ),
                progress,
                color,
            );
            y += 36.0;
        }
    }

    let impact_rows = vec![
        TableRow::new(vec![
            TableCell::strong(tr("v6_pyatiletka_focus_bonus")),
            TableCell::colored(v9_plan_signed_percent(data.focus_bonus), palette::GOOD).right(),
        ])
        .accent(palette::GOOD),
        TableRow::new(vec![
            TableCell::strong(tr("v6_pyatiletka_off_penalty")),
            TableCell::colored(v9_plan_signed_percent(data.off_focus_penalty), palette::BAD)
                .right(),
        ])
        .accent(palette::BAD),
    ];
    DataTable::new(
        vec![
            TableColumn::new("Modifier", 1.2),
            TableColumn::new("Value", 0.7).right(),
        ],
        impact_rows,
    )
    .row_height(28.0)
    .show_at(
        ui,
        Rect::from_min_size(
            Pos2::new(inner.left(), y + spacing::S2),
            Vec2::new(inner.width(), 84.0),
        ),
    );
    y += 96.0;

    ui.painter().text(
        Pos2::new(inner.left(), y),
        Align2::LEFT_TOP,
        tr("v6_pyatiletka_research"),
        TextRole::Subheading.font_id(),
        palette::BRASS_BRIGHT,
    );
    y += 28.0;
    if data.focus_directions.is_empty() {
        ui.painter().text(
            Pos2::new(inner.left(), y),
            Align2::LEFT_TOP,
            "No research focus directions selected.",
            TextRole::Caption.font_id(),
            palette::PARCHMENT_DIM,
        );
    } else {
        for direction in data.focus_directions.iter().take(8) {
            let row =
                Rect::from_min_size(Pos2::new(inner.left(), y), Vec2::new(inner.width(), 24.0));
            crate::v9::paint::paint_bevel(
                ui.painter(),
                row,
                palette::SOOT_BLACK,
                palette::EDGE_DARK,
                1.0,
            );
            ui.painter().rect_filled(
                Rect::from_min_size(row.left_top(), Vec2::new(4.0, row.height())),
                egui::epaint::CornerRadius::ZERO,
                palette::GOOD,
            );
            ui.painter().text(
                Pos2::new(row.left() + spacing::S5, row.center().y),
                Align2::LEFT_CENTER,
                direction.as_str(),
                TextRole::Caption.font_id(),
                palette::PARCHMENT,
            );
            y += 28.0;
        }
    }
}

fn v9_plan_average_progress(data: &PyatiletkaPanelData) -> f32 {
    if data.targets.is_empty() {
        return 1.0;
    }
    data.targets
        .iter()
        .map(v9_plan_target_progress)
        .sum::<f32>()
        / data.targets.len() as f32
}

fn v9_plan_target_progress(entry: &PlanTargetEntry) -> f32 {
    if entry.target_output > 0.0 {
        (entry.actual_output / entry.target_output).clamp(0.0, 1.0)
    } else {
        1.0
    }
}

fn v9_plan_progress_color(progress: f32, ration_rate: f32) -> Color32 {
    use crate::v9::tokens::palette;
    if ration_rate < 0.85 || progress < 0.50 {
        palette::BAD
    } else if ration_rate < 1.0 || progress < 0.90 {
        palette::WARN
    } else {
        palette::GOOD
    }
}

fn v9_plan_ration_color(value: f32) -> Color32 {
    use crate::v9::tokens::palette;
    if value >= 1.0 {
        palette::GOOD
    } else if value >= 0.85 {
        palette::WARN
    } else {
        palette::BAD
    }
}

fn v9_plan_percent(value: f32) -> String {
    format!("{:.0}%", value.clamp(0.0, 9.99) * 100.0)
}

fn v9_plan_signed_percent(value: f32) -> String {
    format!("{:+.0}%", value * 100.0)
}

fn render_plan_target(ui: &mut egui::Ui, entry: &PlanTargetEntry) {
    let progress = if entry.target_output > 0.0 {
        (entry.actual_output / entry.target_output).min(1.0)
    } else {
        1.0
    };

    let bar_color = if progress >= 0.9 {
        Color32::from_rgb(0x60, 0xc0, 0x60)
    } else if progress >= 0.5 {
        Color32::from_rgb(0xc0, 0xc0, 0x40)
    } else {
        Color32::from_rgb(0xc0, 0x60, 0x40)
    };

    ui.horizontal(|ui| {
        ui.label(RichText::new(&entry.good_name).color(Color32::from_gray(220)));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                RichText::new(format!(
                    "{:.0}/{:.0}",
                    entry.actual_output, entry.target_output
                ))
                .small()
                .color(Color32::from_gray(160)),
            );
        });
    });

    ui.add(
        egui::ProgressBar::new(progress)
            .fill(bar_color)
            .show_percentage(),
    );

    if entry.ration_rate < 1.0 {
        ui.horizontal(|ui| {
            ui.colored_label(Color32::from_rgb(0xff, 0xa0, 0x40), "⚠");
            ui.label(
                RichText::new(format!(
                    "{}: {:.0}%",
                    tr("v6_pyatiletka_ration_rate"),
                    entry.ration_rate * 100.0
                ))
                .small()
                .color(Color32::from_rgb(0xff, 0xa0, 0x40)),
            );
        });
    }

    ui.add_space(2.0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panel_data_constructs() {
        let data = PyatiletkaPanelData {
            plan_name: "苏联第一个五年计划".into(),
            plan_period: "1928-1932".into(),
            is_planned_economy: true,
            targets: vec![PlanTargetEntry {
                good_id: "steel".into(),
                good_name: "钢材".into(),
                target_output: 5000.0,
                actual_output: 3500.0,
                ration_rate: 0.85,
            }],
            focus_directions: vec!["industry_1".into()],
            focus_bonus: 0.30,
            off_focus_penalty: -0.20,
        };
        assert_eq!(data.targets.len(), 1);
        assert!(data.is_planned_economy);
    }
}
