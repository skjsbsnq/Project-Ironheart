//! J.2: Province InfoCard — left-click province info panel (bottom-left).
//!
//! V3-economy: show what this province/state produces — resources (actual output),
//! buildings, and construction projects.

use crate::{components, data_table, i18n::tr};

/// Snapshot of province/state data for display.
#[derive(Default)]
pub struct ProvinceInfoData {
    pub province: ProvinceTacticalInfo,
    pub state: StateEconomicInfo,
}

#[derive(Default)]
pub struct ProvinceTacticalInfo {
    pub province_id: u32,
    pub province_name: String,
    pub province_type: String,
    pub terrain: String,
    pub coastal: bool,
    pub owner_tag: String,
    pub controller_tag: String,
    pub state_name: String,
    pub supply: f32,
    pub victory_points: u8,
    pub strategic_nodes: Vec<String>,
    pub divisions: Vec<String>,
}

#[derive(Default)]
pub struct StateEconomicInfo {
    pub state_name: String,
    pub owner_tag: String,
    pub controller_tag: String,
    pub population: u64,
    pub state_category: String,
    pub infrastructure: u8,
    pub buildings: Vec<StateBuildingInfo>,
    pub construction_projects: Vec<StateConstructionProjectInfo>,
    pub resources: Vec<(String, f32)>,
    pub resources_output: Vec<(String, f32)>, // estimated daily output (base × infra bonus)
    pub slots_used: u8,
    pub slots_max: u8,
}

#[derive(Default)]
pub struct StateBuildingInfo {
    pub name: String,
    pub level: u8,
    pub employment_rate: f32,
    pub profit_rm_weekly: f64,
    pub warnings: Vec<String>,
}

#[derive(Default)]
pub struct StateConstructionProjectInfo {
    pub building_name: String,
    pub current_level: u8,
    pub target_level: u8,
    pub progress: f32,
    pub estimated_days_remaining: Option<u32>,
}

/// Province info card UI state.
pub struct ProvinceInfoCard {
    pub open: bool,
    pub bottom_bar_height: f32,
    last_rect: Option<egui::Rect>,
}

impl ProvinceInfoCard {
    pub fn new() -> Self {
        Self {
            open: false,
            bottom_bar_height: 60.0,
            last_rect: None,
        }
    }

    pub fn contains_point(&self, x: f32, y: f32) -> bool {
        self.open
            && self
                .last_rect
                .map_or(false, |rect| rect.contains(egui::pos2(x, y)))
    }

    fn show_v9(&mut self, ctx: &egui::Context, data: &ProvinceInfoData) {
        if !self.open {
            self.last_rect = None;
            return;
        }
        use crate::v9::{
            frame::{FrameStyle, PanelFrame},
            primitives::{Button, ButtonSize, ButtonVariant},
            tokens::{palette, spacing, Elevation, TextRole},
        };
        use egui::{Align2, Area, Order, Pos2, Rect, Sense, Vec2};

        let screen = ctx.screen_rect();
        let size = Vec2::new(
            470.0,
            (screen.height() - self.bottom_bar_height - 96.0).clamp(360.0, 640.0),
        );
        let pos = Pos2::new(
            spacing::S4,
            screen.bottom() - self.bottom_bar_height - size.y - spacing::S4,
        );
        let mut open = self.open;
        let mut last = None;
        Area::new(egui::Id::new("province_info_card_v9"))
            .order(Order::Foreground)
            .fixed_pos(pos)
            .show(ctx, |ui| {
                let (outer, _) = ui.allocate_exact_size(size, Sense::click_and_drag());
                last = Some(outer);
                let frame = PanelFrame::new(FrameStyle::Panel, outer)
                    .with_accent(palette::INFO)
                    .with_elevation(Elevation::E2);
                frame.draw(ui.painter());
                let inner = frame.inner_rect();
                let header = Rect::from_min_size(inner.left_top(), Vec2::new(inner.width(), 34.0));
                ui.painter().text(
                    header.left_center(),
                    Align2::LEFT_CENTER,
                    tr("state_province_info"),
                    TextRole::Heading.font_id(),
                    palette::BRASS_BRIGHT,
                );
                let close_rect = Rect::from_min_size(
                    Pos2::new(header.right() - 26.0, header.top() + 3.0),
                    Vec2::new(24.0, 24.0),
                );
                if Button::new("X")
                    .size(ButtonSize::Sm)
                    .variant(ButtonVariant::Ghost)
                    .show_at(ui, close_rect)
                    .clicked()
                {
                    open = false;
                }
                let body = Rect::from_min_max(
                    Pos2::new(inner.left(), inner.top() + 38.0),
                    inner.right_bottom(),
                );
                ui.allocate_ui_at_rect(body, |ui| {
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.set_width(body.width());
                            v9_province_tactical(ui, data);
                            v9_province_state(ui, data);
                            v9_province_buildings(ui, data);
                            v9_province_projects(ui, data);
                            v9_province_resources(ui, data);
                        });
                });
            });
        self.last_rect = last;
        self.open = open;
    }

    #[allow(unreachable_code)]
    pub fn show(&mut self, ctx: &egui::Context, data: &ProvinceInfoData) {
        return self.show_v9(ctx, data);

        if !self.open {
            self.last_rect = None;
            return;
        }
        let gold = egui::Color32::from_rgb(0xc9, 0xa5, 0x5b);
        let muted = egui::Color32::from_rgb(0x99, 0x99, 0x99);
        let mut open = self.open;
        let y_offset = -(8.0 + self.bottom_bar_height);
        let response = egui::Window::new(tr("state_province_info"))
            .open(&mut open)
            .anchor(egui::Align2::LEFT_BOTTOM, [8.0, y_offset])
            .order(egui::Order::Background)
            .interactable(true)
            .movable(false)
            .collapsible(false)
            .resizable(false)
            .default_width(430.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .max_height(ui.available_height().max(320.0))
                    .show(ui, |ui| {
                        section(ui, gold, tr("province_tactical_info"), |ui| {
                            egui::Grid::new("province_tactical_grid")
                                .num_columns(2)
                                .spacing([18.0, 4.0])
                                .striped(true)
                                .show(ui, |ui| {
                                    key_value(ui, tr("province"), format!("#{} {}", data.province.province_id, data.province.province_name));
                                    key_value(ui, tr("belongs_to_state"), &data.province.state_name);
                                    key_value(ui, tr("owner"), &data.province.owner_tag);
                                    if data.province.owner_tag != data.province.controller_tag {
                                        key_value(ui, tr("controller"), &data.province.controller_tag);
                                    }
                                    key_value(ui, tr("province_kind"), format!("{} / {}", data.province.province_type, data.province.terrain));
                                    key_value(ui, tr("supply"), format!("{:.0}", data.province.supply));
                                    if data.province.coastal {
                                        key_value(ui, "海岸", tr("coastal_province"));
                                    }
                                    if data.province.victory_points > 0 {
                                        key_value(ui, tr("victory_points_label"), data.province.victory_points.to_string());
                                    }
                                    if !data.province.strategic_nodes.is_empty() {
                                        key_value(ui, tr("strategic_nodes"), data.province.strategic_nodes.join("、"));
                                    }
                                });
                            ui.add_space(4.0);
                            ui.label("驻军与战术状态");
                            if data.province.divisions.is_empty() {
                                ui.colored_label(muted, tr("no_divisions"));
                            } else {
                                for name in &data.province.divisions {
                                    ui.label(format!("- {}", name));
                                }
                            }
                        });

                        section(ui, gold, tr("state_economic_info"), |ui| {
                            egui::Grid::new("state_economy_grid")
                                .num_columns(2)
                                .spacing([18.0, 4.0])
                                .striped(true)
                                .show(ui, |ui| {
                                    key_value(ui, "州", &data.state.state_name);
                                    key_value(ui, tr("owner"), &data.state.owner_tag);
                                    if data.state.owner_tag != data.state.controller_tag {
                                        key_value(ui, tr("controller"), &data.state.controller_tag);
                                    }
                                    key_value(ui, "人口", format_population(data.state.population));
                                    key_value(ui, tr("state_category_label"), &data.state.state_category);
                                    key_value(ui, tr("infrastructure_label"), format!("{}/10", data.state.infrastructure));
                                    key_value(ui, tr("slots_label"), format!("{}/{}", data.state.slots_used, data.state.slots_max));
                                });
                        });

                        section(ui, gold, tr("state_buildings"), |ui| {
                            if data.state.buildings.is_empty() {
                                ui.colored_label(muted, tr("no_buildings"));
                            } else {
                                egui::Grid::new("state_building_table")
                                    .num_columns(5)
                                    .spacing([12.0, 4.0])
                                    .striped(true)
                                    .show(ui, |ui| {
                                        table_header(ui, "建筑");
                                        table_header(ui, "等级");
                                        table_header(ui, tr("employment_rate"));
                                        table_header(ui, "利润");
                                        table_header(ui, "警告");
                                        ui.end_row();
                                        for building in &data.state.buildings {
                                            ui.label(&building.name);
                                            ui.label(format!("Lv {}", building.level));
                                            ui.label(format!("{:.0}%", building.employment_rate.clamp(0.0, 1.0) * 100.0));
                                            let profit_color = if building.profit_rm_weekly >= 0.0 { egui::Color32::LIGHT_GREEN } else { egui::Color32::LIGHT_RED };
                                            ui.colored_label(profit_color, format!("{:+.1}M RM/周", building.profit_rm_weekly / 1_000_000.0));
                                            if building.warnings.is_empty() {
                                                ui.colored_label(muted, "无");
                                            } else {
                                                ui.colored_label(egui::Color32::from_rgb(0xff, 0xc0, 0x60), building.warnings.join("；"));
                                            }
                                            ui.end_row();
                                        }
                                    });
                            }
                        });

                        section(ui, gold, tr("state_construction_projects"), |ui| {
                            if data.state.construction_projects.is_empty() {
                                ui.colored_label(muted, tr("no_state_construction"));
                            } else {
                                for item in &data.state.construction_projects {
                                    let progress = item.progress.clamp(0.0, 1.0);
                                    ui.horizontal(|ui| {
                                        ui.label(format!("{} Lv {} -> {}", item.building_name, item.current_level, item.target_level));
                                        let eta = item.estimated_days_remaining
                                            .map(|days| format!("预计 {} 天", days))
                                            .unwrap_or_else(|| "预计时间未知".to_owned());
                                        ui.colored_label(muted, eta);
                                    });
                                    ui.add(egui::ProgressBar::new(progress).desired_width(360.0).text(format!("{:.0}%", progress * 100.0)));
                                }
                            }
                        });

                        section(ui, gold, tr("resources_label"), |ui| {
                            ui.colored_label(muted, "本州资源与市场面板联动：产出进入全国市场，消费来源请在市场详情中查看。");
                            if data.state.resources.is_empty() {
                                ui.colored_label(muted, tr("no_resources"));
                            } else {
                                egui::Grid::new("state_resource_table")
                                    .num_columns(4)
                                    .spacing([16.0, 4.0])
                                    .striped(true)
                                    .show(ui, |ui| {
                                        table_header(ui, "资源");
                                        table_header(ui, "基础储量");
                                        table_header(ui, "估算产出");
                                        table_header(ui, "市场链接");
                                        ui.end_row();
                                        for ((name, level), (_, output)) in data.state.resources.iter().zip(data.state.resources_output.iter()) {
                                            ui.label(name);
                                            ui.label(format!("{:.0}", level));
                                            ui.label(format!("{:.0}/日", output));
                                            ui.colored_label(gold, "市场详情");
                                            ui.end_row();
                                        }
                                    });
                            }
                        });
                    });
            });
        self.last_rect = response.map(|inner| inner.response.rect);
        self.open = open;
    }
}

fn v9_province_tactical(ui: &mut egui::Ui, data: &ProvinceInfoData) {
    use crate::v9::tokens::spacing;

    let extra_rows = usize::from(data.province.coastal)
        + usize::from(data.province.victory_points > 0)
        + usize::from(!data.province.strategic_nodes.is_empty())
        + usize::from(data.province.owner_tag != data.province.controller_tag);
    let division_rows = data.province.divisions.len().max(1);
    let height = 172.0 + extra_rows as f32 * 20.0 + division_rows as f32 * 18.0;

    v9_section(ui, tr("province_tactical_info"), height, |ui, _| {
        egui::Grid::new("province_tactical_grid_v9")
            .num_columns(2)
            .spacing([18.0, spacing::S2])
            .striped(true)
            .show(ui, |ui| {
                v9_key_value(
                    ui,
                    tr("province"),
                    format!(
                        "#{} {}",
                        data.province.province_id, data.province.province_name
                    ),
                );
                v9_key_value(ui, tr("belongs_to_state"), &data.province.state_name);
                v9_key_value(ui, tr("owner"), &data.province.owner_tag);
                if data.province.owner_tag != data.province.controller_tag {
                    v9_key_value(ui, tr("controller"), &data.province.controller_tag);
                }
                v9_key_value(
                    ui,
                    tr("province_kind"),
                    format!(
                        "{} / {}",
                        data.province.province_type, data.province.terrain
                    ),
                );
                v9_key_value(ui, tr("supply"), format!("{:.0}", data.province.supply));
                if data.province.coastal {
                    v9_key_value(ui, "Coast", tr("coastal_province"));
                }
                if data.province.victory_points > 0 {
                    v9_key_value(
                        ui,
                        tr("victory_points_label"),
                        data.province.victory_points.to_string(),
                    );
                }
                if !data.province.strategic_nodes.is_empty() {
                    v9_key_value(
                        ui,
                        tr("strategic_nodes"),
                        data.province.strategic_nodes.join(", "),
                    );
                }
            });

        ui.add_space(spacing::S4);
        ui.label(v9_heading("驻军与战术状态"));
        if data.province.divisions.is_empty() {
            ui.colored_label(crate::v9::tokens::palette::MUTED, tr("no_divisions"));
        } else {
            for name in &data.province.divisions {
                ui.colored_label(crate::v9::tokens::palette::PARCHMENT_DIM, name);
            }
        }
    });
}

fn v9_province_state(ui: &mut egui::Ui, data: &ProvinceInfoData) {
    use crate::v9::tokens::spacing;

    let extra_rows = usize::from(data.state.owner_tag != data.state.controller_tag);
    let height = 140.0 + extra_rows as f32 * 20.0;
    v9_section(ui, tr("state_economic_info"), height, |ui, _| {
        egui::Grid::new("state_economy_grid_v9")
            .num_columns(2)
            .spacing([18.0, spacing::S2])
            .striped(true)
            .show(ui, |ui| {
                v9_key_value(ui, "State", &data.state.state_name);
                v9_key_value(ui, tr("owner"), &data.state.owner_tag);
                if data.state.owner_tag != data.state.controller_tag {
                    v9_key_value(ui, tr("controller"), &data.state.controller_tag);
                }
                v9_key_value(ui, "Population", format_population(data.state.population));
                v9_key_value(ui, tr("state_category_label"), &data.state.state_category);
                v9_key_value(
                    ui,
                    tr("infrastructure_label"),
                    format!("{}/10", data.state.infrastructure),
                );
                v9_key_value(
                    ui,
                    tr("slots_label"),
                    format!("{}/{}", data.state.slots_used, data.state.slots_max),
                );
            });
    });
}

fn v9_province_buildings(ui: &mut egui::Ui, data: &ProvinceInfoData) {
    use crate::v9::data_table::{DataTable, TableCell, TableColumn, TableRow};
    use crate::v9::tokens::palette;

    let row_count = data.state.buildings.len().max(1);
    let height = 52.0 + (row_count as f32 + 1.0) * 28.0;
    v9_section(ui, tr("state_buildings"), height, |ui, _| {
        if data.state.buildings.is_empty() {
            ui.colored_label(palette::MUTED, tr("no_buildings"));
            return;
        }

        let rows = data
            .state
            .buildings
            .iter()
            .map(|building| {
                let profit = building.profit_rm_weekly / 1_000_000.0;
                let warnings = if building.warnings.is_empty() {
                    TableCell::colored("None", palette::MUTED)
                } else {
                    TableCell::colored(building.warnings.join("; "), palette::WARN)
                };
                TableRow::new(vec![
                    TableCell::strong(building.name.clone()),
                    TableCell::new(format!("Lv {}", building.level)).center(),
                    TableCell::new(format!(
                        "{:.0}%",
                        building.employment_rate.clamp(0.0, 1.0) * 100.0
                    ))
                    .right(),
                    TableCell::colored(
                        format!("{:+.1}M", profit),
                        crate::v9::data_table::signed_color(profit),
                    )
                    .right(),
                    warnings,
                ])
            })
            .collect();

        DataTable::new(
            vec![
                TableColumn::new("Building", 1.8),
                TableColumn::new("Lv", 0.6).center(),
                TableColumn::new(tr("employment_rate"), 0.9).right(),
                TableColumn::new("Profit", 0.9).right(),
                TableColumn::new("Warnings", 1.5),
            ],
            rows,
        )
        .show(ui);
    });
}

fn v9_province_projects(ui: &mut egui::Ui, data: &ProvinceInfoData) {
    use crate::v9::primitives::draw_progress_bar;
    use crate::v9::tokens::{palette, spacing, TextRole};
    use egui::{Align2, Pos2, Rect, Sense, Vec2};

    let row_count = data.state.construction_projects.len().max(1);
    let height = 58.0 + row_count as f32 * 48.0;
    v9_section(ui, tr("state_construction_projects"), height, |ui, _| {
        if data.state.construction_projects.is_empty() {
            ui.colored_label(palette::MUTED, tr("no_state_construction"));
            return;
        }

        for item in &data.state.construction_projects {
            let (row, _) =
                ui.allocate_exact_size(Vec2::new(ui.available_width(), 44.0), Sense::hover());
            let label = format!(
                "{} Lv {} -> {}",
                item.building_name, item.current_level, item.target_level
            );
            ui.painter().text(
                row.left_top(),
                Align2::LEFT_TOP,
                label,
                TextRole::Body.font_id(),
                palette::PARCHMENT,
            );
            let eta = item
                .estimated_days_remaining
                .map(|days| format!("ETA {}d", days))
                .unwrap_or_else(|| "ETA unknown".to_owned());
            ui.painter().text(
                Pos2::new(row.right(), row.top()),
                Align2::RIGHT_TOP,
                eta,
                TextRole::Caption.font_id(),
                palette::MUTED,
            );
            let progress = item.progress.clamp(0.0, 1.0);
            let bar = Rect::from_min_max(
                Pos2::new(row.left(), row.top() + 22.0),
                Pos2::new(row.right(), row.top() + 34.0),
            );
            draw_progress_bar(ui, bar, progress, palette::INFO);
            ui.painter().text(
                bar.center(),
                Align2::CENTER_CENTER,
                format!("{:.0}%", progress * 100.0),
                TextRole::Small.font_id(),
                palette::PARCHMENT,
            );
            ui.add_space(spacing::S2);
        }
    });
}

fn v9_province_resources(ui: &mut egui::Ui, data: &ProvinceInfoData) {
    use crate::v9::data_table::{DataTable, TableCell, TableColumn, TableRow};
    use crate::v9::tokens::palette;

    let row_count = data.state.resources.len().max(1);
    let height = 74.0 + (row_count as f32 + 1.0) * 28.0;
    v9_section(ui, tr("resources_label"), height, |ui, _| {
        ui.colored_label(
            palette::MUTED,
            "State resource output feeds the national market. Consumption is shown in market details.",
        );
        if data.state.resources.is_empty() {
            ui.colored_label(palette::MUTED, tr("no_resources"));
            return;
        }

        let rows = data
            .state
            .resources
            .iter()
            .enumerate()
            .map(|(idx, (name, level))| {
                let output = data
                    .state
                    .resources_output
                    .get(idx)
                    .map(|(_, output)| *output)
                    .unwrap_or(*level);
                TableRow::new(vec![
                    TableCell::strong(name.clone()),
                    TableCell::new(format!("{:.0}", level)).right(),
                    TableCell::new(format!("{:.0}/day", output)).right(),
                    TableCell::colored("Market details", palette::BRASS_BRIGHT),
                ])
            })
            .collect();

        DataTable::new(
            vec![
                TableColumn::new("Resource", 1.2),
                TableColumn::new("Base", 0.8).right(),
                TableColumn::new("Output", 0.9).right(),
                TableColumn::new("Link", 1.1),
            ],
            rows,
        )
        .show(ui);
    });
}

fn v9_section(
    ui: &mut egui::Ui,
    title: &str,
    height: f32,
    add_contents: impl FnOnce(&mut egui::Ui, egui::Rect),
) {
    use crate::v9::primitives::Card;
    use crate::v9::tokens::{palette, spacing, TextRole};
    use egui::{Align2, Pos2, Rect, Sense, Vec2};

    let width = ui.available_width().max(120.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    let inner = Card::new().as_panel().show_at(ui, rect);
    ui.painter().text(
        inner.left_top(),
        Align2::LEFT_TOP,
        title,
        TextRole::Subheading.font_id(),
        palette::BRASS_BRIGHT,
    );
    let body = Rect::from_min_max(
        Pos2::new(inner.left(), inner.top() + spacing::S7),
        inner.right_bottom(),
    );
    ui.allocate_ui_at_rect(body, |ui| {
        ui.set_width(body.width());
        add_contents(ui, body);
    });
    ui.add_space(spacing::S4);
}

fn v9_key_value(ui: &mut egui::Ui, key: &str, value: impl ToString) {
    crate::v9::data_table::key_value(ui, key, value);
}

fn v9_heading(text: &str) -> egui::RichText {
    egui::RichText::new(text)
        .font(crate::v9::tokens::TextRole::Subheading.font_id())
        .color(crate::v9::tokens::palette::BRASS_BRIGHT)
}

fn section(
    ui: &mut egui::Ui,
    _color: egui::Color32,
    title: &str,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    components::section(ui, title, add_contents);
}

fn key_value(ui: &mut egui::Ui, key: &str, value: impl ToString) {
    data_table::key_value(ui, key, value);
}

fn table_header(ui: &mut egui::Ui, text: &str) {
    data_table::header(ui, text);
}

fn format_population(value: u64) -> String {
    if value >= 1_000_000 {
        format!("{:.1}M", value as f64 / 1_000_000.0)
    } else if value >= 1_000 {
        format!("{:.0}k", value as f64 / 1_000.0)
    } else {
        value.to_string()
    }
}
