use crate::{
    components,
    vanilla_iron::{CommandPanelShell, VanillaIron},
    ActiveDetailPanel, ActivePrimaryPanel, AirWingDetailTarget, PanelCommand,
};
use egui::{Color32, RichText};

const GOLD: Color32 = Color32::from_rgb(0x9f, 0xc1, 0xc8);
const GOLD_BRIGHT: Color32 = Color32::from_rgb(0xd1, 0xdf, 0xdd);
const MUTED: Color32 = Color32::from_gray(155);
const PANEL_CARD: Color32 = Color32::from_rgb(0x0d, 0x10, 0x0f);
const PANEL_CARD_SOFT: Color32 = Color32::from_rgb(0x14, 0x18, 0x17);
const STROKE_DARK: Color32 = Color32::from_rgb(0x28, 0x31, 0x31);
const GOOD: Color32 = Color32::from_rgb(0x70, 0xc8, 0x78);
const WARN: Color32 = Color32::from_rgb(0xff, 0xc0, 0x60);
const BAD: Color32 = Color32::from_rgb(0xe0, 0x60, 0x58);
const BLUE: Color32 = Color32::from_rgb(0x68, 0xa0, 0xd8);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AirMissionUi {
    Idle,
    AirSuperiority,
    Interception,
    CloseAirSupport,
    StrategicBombing,
    PortStrike,
    NavalStrike,
    NavalPatrol,
    LogisticalStrike,
    Drop,
}

impl AirMissionUi {
    pub const ALL: [Self; 10] = [
        Self::Idle,
        Self::AirSuperiority,
        Self::Interception,
        Self::CloseAirSupport,
        Self::StrategicBombing,
        Self::PortStrike,
        Self::NavalStrike,
        Self::NavalPatrol,
        Self::LogisticalStrike,
        Self::Drop,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Idle => "待命",
            Self::AirSuperiority => "夺取制空",
            Self::Interception => "拦截",
            Self::CloseAirSupport => "近距支援",
            Self::StrategicBombing => "战略轰炸",
            Self::PortStrike => "港口打击",
            Self::NavalStrike => "海军打击",
            Self::NavalPatrol => "海上巡逻",
            Self::LogisticalStrike => "后勤打击",
            Self::Drop => "空投",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            Self::Idle => "联队待命，不执行任务。",
            Self::AirSuperiority => "战斗机争夺空区制空权。",
            Self::Interception => "优先拦截敌方轰炸机。",
            Self::CloseAirSupport => "CAS 和战术轰炸机支援陆战。",
            Self::StrategicBombing => "攻击敌方工业建筑和基础设施。",
            Self::PortStrike => "攻击港口内敌方舰队。",
            Self::NavalStrike => "攻击目标海区敌方舰队。",
            Self::NavalPatrol => "提高海上发现并辅助海军打击。",
            Self::LogisticalStrike => "MVP 中按战略轰炸执行。",
            Self::Drop => "空投任务占位，后续接伞兵/补给。",
        }
    }
}

#[derive(Debug, Clone)]
pub struct AirWingEntry {
    pub id: u32,
    pub name: String,
    pub aircraft_key: String,
    pub base_state: u16,
    pub region_id: u32,
    pub target_region: Option<u32>,
    pub mission: AirMissionUi,
    pub planes: u32,
    pub max_planes: u32,
    pub organisation: f32,
    pub max_organisation: f32,
    pub range_km: f32,
    pub air_control_pct: f32,
    pub mission_efficiency_pct: f32,
    pub transferring: bool,
    pub reinforce_enabled: bool,
}

pub struct AirData {
    pub wings: Vec<AirWingEntry>,
    pub aircraft_stockpile: f32,
    pub total_planes: u32,
    pub active_wings: usize,
    pub over_capacity_bases: usize,
    pub transfer_source_wing: Option<u32>,
    pub pending_transfer_wing: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AirCommand {
    SetMission {
        wing_id: u32,
        mission: AirMissionUi,
    },
    TransferToSelectedState {
        wing_id: u32,
    },
    ToggleReinforce {
        wing_id: u32,
    },
    SplitWing {
        wing_id: u32,
        planes: u32,
    },
    DisbandEmptyWing {
        wing_id: u32,
    },
    SetTransferSource {
        wing_id: u32,
    },
    ClearTransferSource,
    TransferPlanes {
        from_wing_id: u32,
        to_wing_id: u32,
        planes: u32,
    },
    Panel(PanelCommand),
}

pub struct AirPanel;

fn v9_show_air(ctx: &egui::Context, data: &AirData) -> (bool, Vec<AirCommand>) {
    use crate::v9::composites::panel_shell::{
        draw_summary_tiles, draw_tab_strip, PanelClass, PanelShell,
    };
    use crate::v9::tokens::palette;

    let mut commands = Vec::new();
    let low_org = data.wings.iter().any(|wing| wing.org_ratio() < 0.35);
    let low_planes = data.wings.iter().any(|wing| wing.plane_ratio() < 0.5);
    let accent = if data.over_capacity_bases > 0 || low_planes {
        palette::BAD
    } else if low_org {
        palette::WARN
    } else if data.wings.is_empty() {
        palette::MUTED
    } else {
        palette::INFO
    };

    let (close, _) = PanelShell::new("air_panel_v9", "空军司令部")
        .subtitle("联队列表 / 任务指令")
        .class(PanelClass::MilitaryDiplomacy)
        .accent(accent)
        .footer("Q Close  |  Wing DataTable / Mission orders")
        .show(ctx, |ui, layout| {
            draw_summary_tiles(
                ui,
                layout.summary,
                &[
                    ("联队", data.wings.len().to_string(), palette::GOLD),
                    ("任务", data.active_wings.to_string(), palette::INFO),
                    ("飞机", data.total_planes.to_string(), palette::BRASS_BRIGHT),
                    (
                        "库存",
                        format!("{:.0}", data.aircraft_stockpile),
                        palette::GOLD,
                    ),
                    (
                        "超载基地",
                        data.over_capacity_bases.to_string(),
                        if data.over_capacity_bases > 0 {
                            palette::BAD
                        } else {
                            palette::MUTED
                        },
                    ),
                ],
            );
            draw_tab_strip(ui, layout.tabs, "联队表 / 任务指令", accent);
            v9_air_body(ui, layout.body, data, &mut commands);
        });
    (close, commands)
}

fn v9_air_body(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    data: &AirData,
    commands: &mut Vec<AirCommand>,
) {
    use crate::v9::layout::{GridLayout, Track};
    use crate::v9::tokens::spacing;

    ui.allocate_ui_at_rect(rect, |ui| {
        let grid = GridLayout::new(vec![Track::Fr(1.0)], vec![Track::Fr(0.62), Track::Fr(0.38)])
            .with_gutter(spacing::S5, 0.0);
        let cells = grid.measure(rect);
        v9_air_table(ui, GridLayout::cell(&cells, 0, 0), data);
        v9_air_order_ribbon(ui, GridLayout::cell(&cells, 0, 1), data, commands);
    });
}

fn v9_air_table(ui: &mut egui::Ui, rect: egui::Rect, data: &AirData) {
    use crate::v9::primitives::{Card, DataTable, TableCell, TableColumn, TableRow};
    use crate::v9::tokens::{palette, TextRole};
    use egui::{Align2, Pos2, Rect};

    let inner = Card::new().as_panel().show_at(ui, rect);
    ui.painter().text(
        inner.left_top(),
        Align2::LEFT_TOP,
        "联队表",
        TextRole::Heading.font_id(),
        palette::BRASS_BRIGHT,
    );
    let table_rect = Rect::from_min_max(
        Pos2::new(inner.left(), inner.top() + 34.0),
        inner.right_bottom(),
    );
    if data.wings.is_empty() {
        crate::v9::composites::panel_shell::draw_empty_state(
            ui,
            table_rect,
            "暂无联队",
            "加载空军 OOB 或积累飞机库存后会显示。",
        );
        return;
    }

    let rows: Vec<TableRow> = data
        .wings
        .iter()
        .map(|wing| {
            let accent = v9_wing_accent(wing);
            TableRow::new(vec![
                TableCell::strong(wing.name.as_str()),
                TableCell::new(wing.mission.label()),
                TableCell::new(wing.aircraft_key.as_str()),
                TableCell::new(format!("{}", wing.base_state)).right(),
                TableCell::new(format!("{}/{}", wing.planes, wing.max_planes)).right(),
                TableCell::colored(
                    format!("{:.0}%", wing.org_ratio() * 100.0),
                    ratio_palette(wing.org_ratio()),
                )
                .right(),
                TableCell::colored(
                    format!("{:.0}%", wing.mission_efficiency_pct),
                    efficiency_palette(wing.mission_efficiency_pct),
                )
                .right(),
            ])
            .accent(accent)
        })
        .collect();

    DataTable::new(
        vec![
            TableColumn::new("联队", 1.2),
            TableColumn::new("任务", 0.9),
            TableColumn::new("机型", 0.8),
            TableColumn::new("基地", 0.55).right(),
            TableColumn::new("飞机", 0.65).right(),
            TableColumn::new("组织", 0.55).right(),
            TableColumn::new("效率", 0.55).right(),
        ],
        rows,
    )
    .row_height(28.0)
    .show_at(ui, table_rect);
}

fn v9_air_order_ribbon(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    data: &AirData,
    commands: &mut Vec<AirCommand>,
) {
    use crate::v9::primitives::{Button, ButtonSize, ButtonVariant, Card};
    use crate::v9::tokens::{palette, spacing, TextRole};
    use egui::{Align2, Pos2, Rect};

    let inner = Card::new().as_ornate().show_at(ui, rect);
    ui.painter().text(
        inner.left_top(),
        Align2::LEFT_TOP,
        "任务 Ribbon",
        TextRole::Heading.font_id(),
        palette::BRASS_BRIGHT,
    );
    let scroll_rect = Rect::from_min_max(
        Pos2::new(inner.left(), inner.top() + 34.0),
        inner.right_bottom(),
    );
    ui.allocate_ui_at_rect(scroll_rect, |ui| {
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_width(scroll_rect.width());
                if data.wings.is_empty() {
                    ui.label(RichText::new("暂无可下令联队").color(palette::MUTED));
                    return;
                }
                if let Some(id) = data.transfer_source_wing {
                    ui.label(
                        RichText::new(format!("转移来源：联队 #{id}"))
                            .strong()
                            .color(palette::GOLD_HOT),
                    );
                    if Button::new("取消来源")
                        .size(ButtonSize::Sm)
                        .variant(ButtonVariant::Secondary)
                        .show(ui)
                        .clicked()
                    {
                        commands.push(AirCommand::ClearTransferSource);
                    }
                    ui.add_space(spacing::S3);
                }
                for wing in &data.wings {
                    let card_inner = Card::new().show_at(
                        ui,
                        Rect::from_min_size(
                            ui.cursor().min,
                            egui::vec2(scroll_rect.width(), 210.0),
                        ),
                    );
                    ui.allocate_ui_at_rect(card_inner, |ui| {
                        ui.label(RichText::new(&wing.name).strong().color(palette::PARCHMENT));
                        ui.label(
                            RichText::new(format!(
                                "{}  |  基地 {}  |  空区 {}  |  航程 {:.0}km",
                                wing.mission.label(),
                                wing.base_state,
                                wing.region_id,
                                wing.range_km
                            ))
                            .small()
                            .color(palette::PARCHMENT_DIM),
                        );
                        ui.add_space(spacing::S2);
                        ui.push_id(("v9_air_missions", wing.id), |ui| {
                            ui.horizontal_wrapped(|ui| {
                                for mission in AirMissionUi::ALL {
                                    let selected = wing.mission == mission;
                                    let response = Button::new(mission.label())
                                        .size(ButtonSize::Sm)
                                        .variant(if selected {
                                            ButtonVariant::Primary
                                        } else {
                                            ButtonVariant::Ghost
                                        })
                                        .show(ui)
                                        .on_hover_text(mission.hint());
                                    if response.clicked() && !selected {
                                        commands.push(AirCommand::SetMission {
                                            wing_id: wing.id,
                                            mission,
                                        });
                                    }
                                }
                            });
                        });
                        ui.add_space(spacing::S2);
                        ui.horizontal_wrapped(|ui| {
                            if Button::new("调动到选中州")
                                .size(ButtonSize::Sm)
                                .variant(ButtonVariant::Secondary)
                                .show(ui)
                                .clicked()
                            {
                                commands
                                    .push(AirCommand::TransferToSelectedState { wing_id: wing.id });
                            }
                            let reinforce_label = if wing.reinforce_enabled {
                                "关闭补员"
                            } else {
                                "开启补员"
                            };
                            if Button::new(reinforce_label)
                                .size(ButtonSize::Sm)
                                .variant(ButtonVariant::Secondary)
                                .show(ui)
                                .clicked()
                            {
                                commands.push(AirCommand::ToggleReinforce { wing_id: wing.id });
                            }
                            let source_label = if data.transfer_source_wing == Some(wing.id) {
                                "来源已选"
                            } else {
                                "设为来源"
                            };
                            if Button::new(source_label)
                                .size(ButtonSize::Sm)
                                .variant(ButtonVariant::Secondary)
                                .show(ui)
                                .clicked()
                            {
                                commands.push(AirCommand::SetTransferSource { wing_id: wing.id });
                            }
                            for planes in [10u32, 50] {
                                let enabled = wing.planes >= planes;
                                let label = format!("拆出{planes}");
                                if Button::new(label.as_str())
                                    .size(ButtonSize::Sm)
                                    .variant(ButtonVariant::Ghost)
                                    .enabled(enabled)
                                    .show(ui)
                                    .clicked()
                                    && enabled
                                {
                                    commands.push(AirCommand::SplitWing {
                                        wing_id: wing.id,
                                        planes,
                                    });
                                }
                            }
                            if Button::new("删除空联队")
                                .size(ButtonSize::Sm)
                                .variant(ButtonVariant::Danger)
                                .enabled(wing.planes == 0)
                                .show(ui)
                                .clicked()
                                && wing.planes == 0
                            {
                                commands.push(AirCommand::DisbandEmptyWing { wing_id: wing.id });
                            }
                        });
                    });
                    ui.add_space(218.0);
                }
            });
    });
}

fn v9_wing_accent(wing: &AirWingEntry) -> Color32 {
    if wing.transferring {
        crate::v9::tokens::palette::WARN
    } else if wing.plane_ratio() < 0.5 || wing.org_ratio() < 0.35 {
        crate::v9::tokens::palette::BAD
    } else if wing.mission == AirMissionUi::Idle {
        crate::v9::tokens::palette::MUTED
    } else {
        crate::v9::tokens::palette::GOOD
    }
}

fn ratio_palette(value: f32) -> Color32 {
    if value < 0.35 {
        crate::v9::tokens::palette::BAD
    } else if value < 0.75 {
        crate::v9::tokens::palette::WARN
    } else {
        crate::v9::tokens::palette::GOOD
    }
}

fn efficiency_palette(value: f32) -> Color32 {
    if value < 45.0 {
        crate::v9::tokens::palette::BAD
    } else if value < 70.0 {
        crate::v9::tokens::palette::WARN
    } else {
        crate::v9::tokens::palette::GOOD
    }
}

fn command_show_air(ctx: &egui::Context, data: &AirData) -> (bool, Vec<AirCommand>) {
    let low_planes = data.wings.iter().any(|wing| wing.plane_ratio() < 0.5);
    let low_org = data.wings.iter().any(|wing| wing.org_ratio() < 0.35);
    let accent = if data.over_capacity_bases > 0 || low_planes {
        VanillaIron::BAD
    } else if low_org {
        VanillaIron::WARN
    } else {
        VanillaIron::BRASS_BRIGHT
    };
    let (close, output) = CommandPanelShell::new("air_command_panel", "空军司令部")
        .subtitle("空域 / 联队 / 机场容量")
        .footer("Q 关闭 | 点击联队打开详情")
        .accent(accent)
        .show(ctx, |ui, layout| {
            let mut commands = Vec::new();
            air_command_nav(ui, layout.nav, data, &mut commands);
            air_command_main(ui, layout.main, data, &mut commands);
            air_command_strip(ui, layout.bottom_strip, data, &mut commands);
            commands
        });
    (close, output.unwrap_or_default())
}

fn air_command_nav(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    data: &AirData,
    commands: &mut Vec<AirCommand>,
) {
    ui.allocate_ui_at_rect(rect.shrink2(egui::vec2(8.0, 7.0)), |ui| {
        VanillaIron::section_heading(ui, "联队");
        VanillaIron::info_row(ui, "联队数", data.wings.len().to_string());
        VanillaIron::info_row(ui, "执行任务", data.active_wings.to_string());
        ui.add_space(8.0);
        if data.wings.is_empty() {
            ui.label(
                RichText::new("暂无联队。")
                    .small()
                    .color(VanillaIron::MUTED),
            );
            return;
        }
        egui::ScrollArea::vertical().show(ui, |ui| {
            for wing in &data.wings {
                let accent = if wing.plane_ratio() < 0.5 || wing.org_ratio() < 0.35 {
                    VanillaIron::BAD
                } else if wing.transferring {
                    VanillaIron::WARN
                } else {
                    VanillaIron::BRASS_BRIGHT
                };
                egui::Frame::new()
                    .fill(VanillaIron::CARD_DEEP)
                    .stroke(egui::Stroke::new(1.0, accent))
                    .inner_margin(egui::Margin::symmetric(7, 5))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(&wing.name).strong().color(VanillaIron::TEXT));
                            if ui.small_button("详情").clicked() {
                                commands.push(AirCommand::Panel(PanelCommand::OpenDetail(
                                    ActiveDetailPanel::AirWing(AirWingDetailTarget {
                                        air_wing_id: wing.id,
                                    }),
                                )));
                            }
                        });
                        ui.label(
                            RichText::new(format!(
                                "{} / 空域 {} / {} 架",
                                wing.mission.label(),
                                wing.region_id,
                                wing.planes
                            ))
                            .small()
                            .color(VanillaIron::MUTED),
                        );
                    });
                ui.add_space(4.0);
            }
        });
    });
}

fn air_command_main(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    data: &AirData,
    commands: &mut Vec<AirCommand>,
) {
    ui.allocate_ui_at_rect(rect.shrink2(egui::vec2(8.0, 7.0)), |ui| {
        VanillaIron::section_heading(ui, "空军态势");
        ui.columns(4, |columns| {
            VanillaIron::info_row(&mut columns[0], "飞机", data.total_planes.to_string());
            VanillaIron::info_row(
                &mut columns[1],
                "库存",
                format!("{:.0}", data.aircraft_stockpile),
            );
            VanillaIron::value_row(
                &mut columns[2],
                "超载基地",
                data.over_capacity_bases.to_string(),
                if data.over_capacity_bases > 0 {
                    VanillaIron::BAD
                } else {
                    VanillaIron::MUTED
                },
            );
            VanillaIron::info_row(
                &mut columns[3],
                "转移来源",
                data.transfer_source_wing
                    .map(|id| id.to_string())
                    .unwrap_or_else(|| "无".to_owned()),
            );
        });
        if let Some(id) = data.pending_transfer_wing {
            ui.add_space(8.0);
            VanillaIron::warning_row(ui, &format!("联队 #{id} 正等待地图选择机场州。"));
        }
        ui.add_space(8.0);
        VanillaIron::section_heading(ui, "联队状态");
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Grid::new("air_command_wing_grid")
                    .striped(true)
                    .spacing(egui::vec2(8.0, 4.0))
                    .show(ui, |ui| {
                        for wing in &data.wings {
                            if ui.link(&wing.name).clicked() {
                                commands.push(AirCommand::Panel(PanelCommand::OpenDetail(
                                    ActiveDetailPanel::AirWing(AirWingDetailTarget {
                                        air_wing_id: wing.id,
                                    }),
                                )));
                            }
                            ui.label(wing.mission.label());
                            ui.label(format!("基地 {}", wing.base_state));
                            ui.label(format!("空域 {}", wing.region_id));
                            ui.label(
                                RichText::new(format!("{}/{}", wing.planes, wing.max_planes))
                                    .color(if wing.plane_ratio() < 0.5 {
                                        VanillaIron::BAD
                                    } else {
                                        VanillaIron::TEXT
                                    }),
                            );
                            ui.label(
                                RichText::new(format!("{:.0}% 效率", wing.mission_efficiency_pct))
                                    .color(if wing.mission_efficiency_pct < 60.0 {
                                        VanillaIron::WARN
                                    } else {
                                        VanillaIron::MUTED
                                    }),
                            );
                            ui.end_row();
                        }
                    });
            });
    });
}

fn air_command_strip(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    data: &AirData,
    commands: &mut Vec<AirCommand>,
) {
    ui.allocate_ui_at_rect(rect.shrink2(egui::vec2(8.0, 7.0)), |ui| {
        ui.horizontal_wrapped(|ui| {
            if let Some(wing) = data.wings.first() {
                if VanillaIron::compact_button(ui, "联队详情").clicked() {
                    commands.push(AirCommand::Panel(PanelCommand::OpenDetail(
                        ActiveDetailPanel::AirWing(AirWingDetailTarget {
                            air_wing_id: wing.id,
                        }),
                    )));
                }
                if VanillaIron::compact_button(ui, "调动到选中州").clicked() {
                    commands.push(AirCommand::TransferToSelectedState { wing_id: wing.id });
                }
                if VanillaIron::compact_button(ui, "切换补员").clicked() {
                    commands.push(AirCommand::ToggleReinforce { wing_id: wing.id });
                }
                if VanillaIron::compact_button(ui, "制空").clicked() {
                    commands.push(AirCommand::SetMission {
                        wing_id: wing.id,
                        mission: AirMissionUi::AirSuperiority,
                    });
                }
                if VanillaIron::compact_button(ui, "近距支援").clicked() {
                    commands.push(AirCommand::SetMission {
                        wing_id: wing.id,
                        mission: AirMissionUi::CloseAirSupport,
                    });
                }
            }
            if VanillaIron::compact_button(ui, "打开物流").clicked() {
                commands.push(AirCommand::Panel(PanelCommand::OpenPrimary(
                    ActivePrimaryPanel::Logistics,
                )));
            }
        });
    });
}

impl AirPanel {
    pub fn show(ctx: &egui::Context, data: &AirData) -> (bool, Vec<AirCommand>) {
        command_show_air(ctx, data)
    }
}

fn render_selection_banner(ui: &mut egui::Ui, data: &AirData, commands: &mut Vec<AirCommand>) {
    if data.transfer_source_wing.is_none() && data.pending_transfer_wing.is_none() {
        return;
    }
    egui::Frame::new()
        .fill(Color32::from_rgba_premultiplied(0x33, 0x24, 0x10, 235))
        .stroke(egui::Stroke::new(1.0, GOLD_BRIGHT))
        .inner_margin(egui::Margin::symmetric(10, 7))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                if let Some(id) = data.transfer_source_wing {
                    ui.label(
                        RichText::new(format!("转移来源：联队 #{id}"))
                            .strong()
                            .color(GOLD_BRIGHT),
                    );
                    if components::action_button(ui, true, "取消来源").clicked() {
                        commands.push(AirCommand::ClearTransferSource);
                    }
                }
                if let Some(id) = data.pending_transfer_wing {
                    ui.label(
                        RichText::new(format!("等待选择机场州：联队 #{id}"))
                            .strong()
                            .color(WARN),
                    );
                    ui.label(
                        RichText::new("在地图点击目标州后会自动调动。ESC/再次点调动可取消。")
                            .small()
                            .color(MUTED),
                    );
                }
            });
        });
    ui.add_space(8.0);
}

fn render_summary(ui: &mut egui::Ui, data: &AirData) {
    ui.add_space(6.0);
    components::summary_strip(
        ui,
        &[
            ("联队", data.wings.len().to_string()),
            ("执行任务", data.active_wings.to_string()),
            ("飞机", data.total_planes.to_string()),
            ("飞机库存", format!("{:.0}", data.aircraft_stockpile)),
            ("超容量基地", data.over_capacity_bases.to_string()),
        ],
    );
    ui.add_space(8.0);
}

fn render_status_banner(ui: &mut egui::Ui, data: &AirData) {
    let low_org = data.wings.iter().any(|wing| wing.org_ratio() < 0.35);
    let low_planes = data.wings.iter().any(|wing| wing.plane_ratio() < 0.5);
    let (label, text, color) = if data.over_capacity_bases > 0 {
        (
            "机场超载",
            format!(
                "{} 个基地超过容量，调动或拆分联队可恢复效率。",
                data.over_capacity_bases
            ),
            BAD,
        )
    } else if low_planes {
        (
            "飞机缺口",
            "有联队飞机不足，开启补员并保持 aircraft 库存。".to_owned(),
            WARN,
        )
    } else if low_org {
        (
            "组织度偏低",
            "部分联队需要休整，待命或减少连续作战可恢复组织度。".to_owned(),
            WARN,
        )
    } else if data.wings.is_empty() {
        (
            "无可用联队",
            "飞机库存达到 100 后每日 tick 可建立预备联队。".to_owned(),
            MUTED,
        )
    } else {
        (
            "空军待命",
            "联队状态稳定。按战区需要分配制空、支援或轰炸任务。".to_owned(),
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

fn render_wing_section(ui: &mut egui::Ui, data: &AirData, commands: &mut Vec<AirCommand>) {
    air_card(ui, "联队列表", |ui| {
        if data.wings.is_empty() {
            components::empty_state(
                ui,
                "暂无联队",
                "加载空军 OOB 或积累飞机库存后会在这里显示。月/日 tick 会补充预备联队。",
            );
            return;
        }
        for wing in &data.wings {
            render_wing_card(ui, wing, data, commands);
            ui.add_space(7.0);
        }
    });
}

fn render_wing_card(
    ui: &mut egui::Ui,
    wing: &AirWingEntry,
    data: &AirData,
    commands: &mut Vec<AirCommand>,
) {
    let selected_source = data.transfer_source_wing == Some(wing.id);
    let pending_transfer = data.pending_transfer_wing == Some(wing.id);
    let accent = if selected_source || pending_transfer {
        GOLD_BRIGHT
    } else if wing.transferring {
        WARN
    } else if wing.plane_ratio() < 0.5 || wing.org_ratio() < 0.35 {
        BAD
    } else if wing.mission == AirMissionUi::Idle {
        MUTED
    } else {
        GOOD
    };
    egui::Frame::new()
        .fill(PANEL_CARD_SOFT)
        .stroke(egui::Stroke::new(1.0, accent))
        .inner_margin(egui::Margin::symmetric(9, 8))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(&wing.name).strong().color(Color32::WHITE));
                if selected_source {
                    status_pill(ui, "转移来源", GOLD_BRIGHT);
                }
                if pending_transfer {
                    status_pill(ui, "选择机场中", WARN);
                }
                status_pill(ui, wing.mission.label(), GOLD_BRIGHT);
                status_pill(ui, format!("基地州 {}", wing.base_state), BLUE);
                status_pill(ui, format!("空区 {}", wing.region_id), BLUE);
                if let Some(target) = wing.target_region {
                    status_pill(ui, format!("目标 {}", target), WARN);
                }
                if wing.transferring {
                    status_pill(ui, "调动中", WARN);
                }
            });
            ui.add_space(6.0);
            ui.columns(4, |columns| {
                small_metric(&mut columns[0], "机型", &wing.aircraft_key);
                small_metric(
                    &mut columns[1],
                    "飞机",
                    format!("{}/{}", wing.planes, wing.max_planes),
                );
                small_metric(&mut columns[2], "航程", format!("{:.0}km", wing.range_km));
                small_metric(
                    &mut columns[3],
                    "制空",
                    format!("{:.0}%", wing.air_control_pct),
                );
            });
            ui.add_space(6.0);
            progress_bar(
                ui,
                wing.plane_ratio(),
                hp_color(wing.plane_ratio()),
                "飞机数量",
            );
            ui.add_space(4.0);
            progress_bar(ui, wing.org_ratio(), hp_color(wing.org_ratio()), "组织度");
            ui.add_space(5.0);
            ui.label(
                RichText::new(format!("任务效率：{:.0}%", wing.mission_efficiency_pct))
                    .small()
                    .color(if wing.mission_efficiency_pct >= 70.0 {
                        GOOD
                    } else {
                        WARN
                    }),
            );
            ui.add_space(7.0);
            render_mission_buttons(ui, wing, commands);
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                if components::action_button(ui, true, "调动到选中州").clicked() {
                    commands.push(AirCommand::TransferToSelectedState { wing_id: wing.id });
                }
                if components::action_button(
                    ui,
                    true,
                    if wing.reinforce_enabled {
                        "关闭补员"
                    } else {
                        "开启补员"
                    },
                )
                .clicked()
                {
                    commands.push(AirCommand::ToggleReinforce { wing_id: wing.id });
                }
            });
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("编制").small().strong().color(GOLD));
                if components::action_button(ui, wing.planes >= 1, "拆出1").clicked() {
                    commands.push(AirCommand::SplitWing {
                        wing_id: wing.id,
                        planes: 1,
                    });
                }
                if components::action_button(ui, wing.planes >= 10, "拆出10").clicked() {
                    commands.push(AirCommand::SplitWing {
                        wing_id: wing.id,
                        planes: 10,
                    });
                }
                if components::action_button(ui, wing.planes >= 2, "拆出半数").clicked() {
                    commands.push(AirCommand::SplitWing {
                        wing_id: wing.id,
                        planes: wing.planes / 2,
                    });
                }
                if components::action_button(
                    ui,
                    true,
                    if selected_source {
                        "来源已选"
                    } else {
                        "设为转移来源"
                    },
                )
                .clicked()
                {
                    commands.push(AirCommand::SetTransferSource { wing_id: wing.id });
                }
                if let Some(from) = data.transfer_source_wing {
                    if from != wing.id {
                        for planes in [10u32, 50, 100] {
                            let label = format!("调入{planes}");
                            if components::action_button(ui, true, &label).clicked() {
                                commands.push(AirCommand::TransferPlanes {
                                    from_wing_id: from,
                                    to_wing_id: wing.id,
                                    planes,
                                });
                            }
                        }
                    }
                }
                if components::action_button(ui, wing.planes == 0, "删除空联队").clicked() {
                    commands.push(AirCommand::DisbandEmptyWing { wing_id: wing.id });
                }
            });
        });
}

fn render_mission_buttons(ui: &mut egui::Ui, wing: &AirWingEntry, commands: &mut Vec<AirCommand>) {
    ui.label(RichText::new("任务").small().strong().color(GOLD));
    ui.horizontal_wrapped(|ui| {
        for mission in AirMissionUi::ALL {
            let selected = wing.mission == mission;
            let text = if selected {
                RichText::new(mission.label())
                    .strong()
                    .color(Color32::WHITE)
            } else {
                RichText::new(mission.label()).color(GOLD)
            };
            let response = ui
                .add(egui::SelectableLabel::new(selected, text))
                .on_hover_text(mission.hint());
            if response.clicked() && !selected {
                commands.push(AirCommand::SetMission {
                    wing_id: wing.id,
                    mission,
                });
            }
        }
    });
}

fn render_help_card(ui: &mut egui::Ui) {
    air_card(ui, "操作提示", |ui| {
        ui.label(RichText::new("战斗机执行夺取制空/拦截；CAS 执行近距支援；战略轰炸会每日破坏敌方建筑或基建；海军打击会伤害目标海区敌舰。")
            .small()
            .color(Color32::from_rgb(0xe0, 0xd2, 0xa8)));
        ui.add_space(5.0);
        ui.label(
            RichText::new("先在地图选中目标州，再点击“调动到选中州”。机场容量会限制部署规模。")
                .small()
                .color(MUTED),
        );
    });
}

fn air_card(ui: &mut egui::Ui, title: &str, add_contents: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(PANEL_CARD)
        .stroke(egui::Stroke::new(1.0, STROKE_DARK))
        .inner_margin(egui::Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(title).strong().color(GOLD_BRIGHT));
            ui.separator();
            add_contents(ui);
        });
    ui.add_space(8.0);
}

fn status_pill(ui: &mut egui::Ui, text: impl Into<String>, color: Color32) {
    egui::Frame::new()
        .fill(Color32::from_rgba_premultiplied(0x1d, 0x16, 0x10, 210))
        .stroke(egui::Stroke::new(1.0, color))
        .inner_margin(egui::Margin::symmetric(6, 2))
        .show(ui, |ui| {
            ui.label(RichText::new(text.into()).small().strong().color(color));
        });
}

fn small_metric(ui: &mut egui::Ui, label: &str, value: impl Into<String>) {
    ui.vertical(|ui| {
        ui.label(RichText::new(label).small().color(MUTED));
        ui.label(
            RichText::new(value.into())
                .small()
                .strong()
                .color(GOLD_BRIGHT),
        );
    });
}

fn progress_bar(ui: &mut egui::Ui, value: f32, color: Color32, label: &str) {
    let value = value.clamp(0.0, 1.0);
    let desired = egui::vec2(ui.available_width(), 10.0);
    let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, 1.0, Color32::from_rgb(0x18, 0x12, 0x0d));
    let fill = egui::Rect::from_min_size(rect.min, egui::vec2(rect.width() * value, rect.height()));
    painter.rect_filled(fill, 1.0, color);
    painter.rect_stroke(
        rect,
        1.0,
        egui::Stroke::new(1.0, STROKE_DARK),
        egui::epaint::StrokeKind::Inside,
    );
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        egui::TextStyle::Small.resolve(ui.style()),
        Color32::from_rgb(0xe0, 0xd2, 0xa8),
    );
}

fn hp_color(value: f32) -> Color32 {
    if value < 0.35 {
        BAD
    } else if value < 0.75 {
        WARN
    } else {
        GOOD
    }
}

impl AirWingEntry {
    fn plane_ratio(&self) -> f32 {
        if self.max_planes == 0 {
            0.0
        } else {
            self.planes as f32 / self.max_planes as f32
        }
    }

    fn org_ratio(&self) -> f32 {
        if self.max_organisation <= 0.0 {
            0.0
        } else {
            self.organisation / self.max_organisation
        }
    }
}
