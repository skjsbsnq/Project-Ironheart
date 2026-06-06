use crate::{
    components,
    vanilla_iron::{CommandPanelShell, VanillaIron},
    ActiveDetailPanel, ActivePrimaryPanel, FleetDetailTarget, PanelCommand,
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
pub enum NavalMissionUi {
    Idle,
    Patrol,
    ConvoyEscort,
    StrikeForce,
    ConvoyRaiding,
    MineLaying,
    MineSweeping,
    NavalInvasionSupport,
}

impl NavalMissionUi {
    pub const ALL: [Self; 8] = [
        Self::Idle,
        Self::Patrol,
        Self::ConvoyEscort,
        Self::StrikeForce,
        Self::ConvoyRaiding,
        Self::MineLaying,
        Self::MineSweeping,
        Self::NavalInvasionSupport,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Idle => "待命",
            Self::Patrol => "巡逻",
            Self::ConvoyEscort => "护航",
            Self::StrikeForce => "打击舰队",
            Self::ConvoyRaiding => "通商破坏",
            Self::MineLaying => "布雷",
            Self::MineSweeping => "扫雷",
            Self::NavalInvasionSupport => "登陆支援",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            Self::Idle => "停泊或待命，不主动提供海区存在。",
            Self::Patrol => "提高侦察和制海存在，辅助发现敌舰与护航。",
            Self::ConvoyEscort => "保护己方贸易与海运航线，降低运输船损失。",
            Self::StrikeForce => "主力舰队主动寻找交战机会。",
            Self::ConvoyRaiding => "袭击敌方运输船，制造贸易和海运损失。",
            Self::MineLaying => "MVP 中作为弱巡逻存在处理。",
            Self::MineSweeping => "MVP 中作为弱巡逻存在处理。",
            Self::NavalInvasionSupport => "为登陆方向提供海军支援存在。",
        }
    }
}

#[derive(Debug, Clone)]
pub struct FleetEntry {
    pub id: u32,
    pub name: String,
    pub region_id: u32,
    pub target_region_id: Option<u32>,
    pub mission: NavalMissionUi,
    pub ship_count: usize,
    pub damaged_ships: usize,
    pub hp_ratio: f32,
    pub convoy_risk_pct: f32,
    pub repair_state: String,
}

pub struct NavalData {
    pub fleets: Vec<FleetEntry>,
    pub convoys: f32,
    pub naval_vessels: f32,
    pub transfer_source_fleet: Option<u32>,
    pub pending_move_fleet: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavalCommand {
    SetMission {
        fleet_id: u32,
        mission: NavalMissionUi,
    },
    MoveToSelectedSeaRegion {
        fleet_id: u32,
    },
    SplitFleet {
        fleet_id: u32,
        count: usize,
    },
    DisbandEmptyFleet {
        fleet_id: u32,
    },
    SetTransferSource {
        fleet_id: u32,
    },
    ClearTransferSource,
    TransferShips {
        from_fleet_id: u32,
        to_fleet_id: u32,
        count: usize,
    },
    Panel(PanelCommand),
}

pub struct NavalPanel;

fn v9_show_naval(ctx: &egui::Context, data: &NavalData) -> (bool, Vec<NavalCommand>) {
    use crate::v9::composites::panel_shell::{
        draw_summary_tiles, draw_tab_strip, PanelClass, PanelShell,
    };
    use crate::v9::tokens::palette;

    let mut commands = Vec::new();
    let ship_count: usize = data.fleets.iter().map(|fleet| fleet.ship_count).sum();
    let damaged: usize = data.fleets.iter().map(|fleet| fleet.damaged_ships).sum();
    let moving = data
        .fleets
        .iter()
        .filter(|fleet| fleet.target_region_id.is_some())
        .count();
    let max_risk = data
        .fleets
        .iter()
        .map(|fleet| fleet.convoy_risk_pct)
        .fold(0.0_f32, f32::max);
    let accent = if max_risk >= 12.0 {
        palette::BAD
    } else if damaged > 0 {
        palette::WARN
    } else if data.fleets.is_empty() {
        palette::MUTED
    } else {
        palette::INFO
    };

    let (close, _) = PanelShell::new("naval_panel_v9", "海军司令部")
        .subtitle("舰队列表 / 任务指令")
        .class(PanelClass::MilitaryDiplomacy)
        .accent(accent)
        .footer("Q Close  |  Fleet DataTable / Mission orders")
        .show(ctx, |ui, layout| {
            draw_summary_tiles(
                ui,
                layout.summary,
                &[
                    ("舰队", data.fleets.len().to_string(), palette::GOLD),
                    ("舰船", ship_count.to_string(), palette::BRASS_BRIGHT),
                    (
                        "受损",
                        damaged.to_string(),
                        if damaged > 0 {
                            palette::WARN
                        } else {
                            palette::MUTED
                        },
                    ),
                    ("航行", moving.to_string(), palette::INFO),
                    (
                        "风险",
                        format!("{:.1}%", max_risk),
                        naval_risk_palette(max_risk),
                    ),
                    ("运输船", format!("{:.0}", data.convoys), palette::GOLD),
                ],
            );
            draw_tab_strip(ui, layout.tabs, "舰队表 / 任务指令", accent);
            v9_naval_body(ui, layout.body, data, &mut commands);
        });
    (close, commands)
}

fn v9_naval_body(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    data: &NavalData,
    commands: &mut Vec<NavalCommand>,
) {
    use crate::v9::layout::{GridLayout, Track};
    use crate::v9::tokens::spacing;

    ui.allocate_ui_at_rect(rect, |ui| {
        let grid = GridLayout::new(vec![Track::Fr(1.0)], vec![Track::Fr(0.62), Track::Fr(0.38)])
            .with_gutter(spacing::S5, 0.0);
        let cells = grid.measure(rect);
        v9_naval_table(ui, GridLayout::cell(&cells, 0, 0), data);
        v9_naval_order_ribbon(ui, GridLayout::cell(&cells, 0, 1), data, commands);
    });
}

fn v9_naval_table(ui: &mut egui::Ui, rect: egui::Rect, data: &NavalData) {
    use crate::v9::primitives::{Card, DataTable, TableCell, TableColumn, TableRow};
    use crate::v9::tokens::{palette, TextRole};
    use egui::{Align2, Pos2, Rect};

    let inner = Card::new().as_panel().show_at(ui, rect);
    ui.painter().text(
        inner.left_top(),
        Align2::LEFT_TOP,
        "舰队表",
        TextRole::Heading.font_id(),
        palette::BRASS_BRIGHT,
    );
    let table_rect = Rect::from_min_max(
        Pos2::new(inner.left(), inner.top() + 34.0),
        inner.right_bottom(),
    );
    if data.fleets.is_empty() {
        crate::v9::composites::panel_shell::draw_empty_state(
            ui,
            table_rect,
            "暂无舰队",
            "造船或加载海军 OOB 后会显示。",
        );
        return;
    }

    let rows: Vec<TableRow> = data
        .fleets
        .iter()
        .map(|fleet| {
            let accent = v9_fleet_accent(fleet);
            TableRow::new(vec![
                TableCell::strong(fleet.name.as_str()),
                TableCell::new(fleet.mission.label()),
                TableCell::new(format!("{}", fleet.region_id)).right(),
                TableCell::new(format!("{}", fleet.ship_count)).right(),
                TableCell::colored(
                    format!("{}", fleet.damaged_ships),
                    if fleet.damaged_ships > 0 {
                        palette::WARN
                    } else {
                        palette::MUTED
                    },
                )
                .right(),
                TableCell::colored(
                    format!("{:.0}%", fleet.hp_ratio * 100.0),
                    ratio_palette(fleet.hp_ratio),
                )
                .right(),
                TableCell::colored(
                    format!("{:.1}%", fleet.convoy_risk_pct),
                    naval_risk_palette(fleet.convoy_risk_pct),
                )
                .right(),
            ])
            .accent(accent)
        })
        .collect();

    DataTable::new(
        vec![
            TableColumn::new("舰队", 1.2),
            TableColumn::new("任务", 0.9),
            TableColumn::new("海区", 0.55).right(),
            TableColumn::new("舰船", 0.55).right(),
            TableColumn::new("受损", 0.55).right(),
            TableColumn::new("完整", 0.55).right(),
            TableColumn::new("风险", 0.55).right(),
        ],
        rows,
    )
    .row_height(28.0)
    .show_at(ui, table_rect);
}

fn v9_naval_order_ribbon(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    data: &NavalData,
    commands: &mut Vec<NavalCommand>,
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
                if data.fleets.is_empty() {
                    ui.label(RichText::new("暂无可下令舰队").color(palette::MUTED));
                    return;
                }
                if let Some(id) = data.transfer_source_fleet {
                    ui.label(
                        RichText::new(format!("转移来源：舰队 #{id}"))
                            .strong()
                            .color(palette::GOLD_HOT),
                    );
                    if Button::new("取消来源")
                        .size(ButtonSize::Sm)
                        .variant(ButtonVariant::Secondary)
                        .show(ui)
                        .clicked()
                    {
                        commands.push(NavalCommand::ClearTransferSource);
                    }
                    ui.add_space(spacing::S3);
                }
                for fleet in &data.fleets {
                    let card_inner = Card::new().show_at(
                        ui,
                        Rect::from_min_size(
                            ui.cursor().min,
                            egui::vec2(scroll_rect.width(), 188.0),
                        ),
                    );
                    ui.allocate_ui_at_rect(card_inner, |ui| {
                        ui.label(
                            RichText::new(&fleet.name)
                                .strong()
                                .color(palette::PARCHMENT),
                        );
                        ui.label(
                            RichText::new(format!(
                                "{}  |  海区 {}  |  {} 舰船  |  维修 {}",
                                fleet.mission.label(),
                                fleet.region_id,
                                fleet.ship_count,
                                repair_state_label(&fleet.repair_state)
                            ))
                            .small()
                            .color(palette::PARCHMENT_DIM),
                        );
                        ui.add_space(spacing::S2);
                        ui.push_id(("v9_naval_missions", fleet.id), |ui| {
                            ui.horizontal_wrapped(|ui| {
                                for mission in NavalMissionUi::ALL {
                                    let selected = fleet.mission == mission;
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
                                        commands.push(NavalCommand::SetMission {
                                            fleet_id: fleet.id,
                                            mission,
                                        });
                                    }
                                }
                            });
                        });
                        ui.add_space(spacing::S2);
                        ui.horizontal_wrapped(|ui| {
                            if Button::new("移动到选中海区")
                                .size(ButtonSize::Sm)
                                .variant(ButtonVariant::Secondary)
                                .show(ui)
                                .clicked()
                            {
                                commands.push(NavalCommand::MoveToSelectedSeaRegion {
                                    fleet_id: fleet.id,
                                });
                            }
                            let source_label = if data.transfer_source_fleet == Some(fleet.id) {
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
                                commands
                                    .push(NavalCommand::SetTransferSource { fleet_id: fleet.id });
                            }
                            for count in [1usize, 5] {
                                let enabled = fleet.ship_count >= count;
                                let label = format!("拆出{count}");
                                if Button::new(label.as_str())
                                    .size(ButtonSize::Sm)
                                    .variant(ButtonVariant::Ghost)
                                    .enabled(enabled)
                                    .show(ui)
                                    .clicked()
                                    && enabled
                                {
                                    commands.push(NavalCommand::SplitFleet {
                                        fleet_id: fleet.id,
                                        count,
                                    });
                                }
                            }
                            if Button::new("删除空舰队")
                                .size(ButtonSize::Sm)
                                .variant(ButtonVariant::Danger)
                                .enabled(fleet.ship_count == 0)
                                .show(ui)
                                .clicked()
                                && fleet.ship_count == 0
                            {
                                commands
                                    .push(NavalCommand::DisbandEmptyFleet { fleet_id: fleet.id });
                            }
                        });
                    });
                    ui.add_space(196.0);
                }
            });
    });
}

fn v9_fleet_accent(fleet: &FleetEntry) -> Color32 {
    if fleet.convoy_risk_pct >= 12.0 {
        crate::v9::tokens::palette::BAD
    } else if fleet.damaged_ships > 0 || fleet.target_region_id.is_some() {
        crate::v9::tokens::palette::WARN
    } else if fleet.mission == NavalMissionUi::Idle {
        crate::v9::tokens::palette::MUTED
    } else {
        crate::v9::tokens::palette::GOOD
    }
}

fn naval_risk_palette(value: f32) -> Color32 {
    if value >= 12.0 {
        crate::v9::tokens::palette::BAD
    } else if value >= 5.0 {
        crate::v9::tokens::palette::WARN
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

fn command_show_naval(ctx: &egui::Context, data: &NavalData) -> (bool, Vec<NavalCommand>) {
    let damaged: usize = data.fleets.iter().map(|fleet| fleet.damaged_ships).sum();
    let max_risk = data
        .fleets
        .iter()
        .map(|fleet| fleet.convoy_risk_pct)
        .fold(0.0_f32, f32::max);
    let accent = if max_risk >= 12.0 {
        VanillaIron::BAD
    } else if damaged > 0 {
        VanillaIron::WARN
    } else {
        VanillaIron::BRASS_BRIGHT
    };
    let (close, output) = CommandPanelShell::new("naval_command_panel", "海军司令部")
        .subtitle("舰队 / 任务海域 / 维修与补给")
        .footer("Q 关闭 | 点击舰队打开详情")
        .accent(accent)
        .show(ctx, |ui, layout| {
            let mut commands = Vec::new();
            naval_command_nav(ui, layout.nav, data, &mut commands);
            naval_command_main(ui, layout.main, data, damaged, max_risk, &mut commands);
            naval_command_strip(ui, layout.bottom_strip, data, &mut commands);
            commands
        });
    (close, output.unwrap_or_default())
}

fn naval_command_nav(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    data: &NavalData,
    commands: &mut Vec<NavalCommand>,
) {
    ui.allocate_ui_at_rect(rect.shrink2(egui::vec2(8.0, 7.0)), |ui| {
        VanillaIron::section_heading(ui, "舰队");
        VanillaIron::info_row(ui, "舰队数", data.fleets.len().to_string());
        VanillaIron::info_row(
            ui,
            "舰船",
            data.fleets
                .iter()
                .map(|fleet| fleet.ship_count)
                .sum::<usize>()
                .to_string(),
        );
        ui.add_space(8.0);
        if data.fleets.is_empty() {
            ui.label(
                RichText::new("暂无舰队。")
                    .small()
                    .color(VanillaIron::MUTED),
            );
            return;
        }
        egui::ScrollArea::vertical().show(ui, |ui| {
            for fleet in &data.fleets {
                let accent = if fleet.convoy_risk_pct >= 12.0 {
                    VanillaIron::BAD
                } else if fleet.damaged_ships > 0 || fleet.target_region_id.is_some() {
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
                            ui.label(RichText::new(&fleet.name).strong().color(VanillaIron::TEXT));
                            if ui.small_button("详情").clicked() {
                                commands.push(NavalCommand::Panel(PanelCommand::OpenDetail(
                                    ActiveDetailPanel::Fleet(FleetDetailTarget {
                                        fleet_id: fleet.id,
                                    }),
                                )));
                            }
                        });
                        ui.label(
                            RichText::new(format!(
                                "{} / 海区 {} / {} 艘",
                                fleet.mission.label(),
                                fleet.region_id,
                                fleet.ship_count
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

fn naval_command_main(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    data: &NavalData,
    damaged: usize,
    max_risk: f32,
    commands: &mut Vec<NavalCommand>,
) {
    ui.allocate_ui_at_rect(rect.shrink2(egui::vec2(8.0, 7.0)), |ui| {
        VanillaIron::section_heading(ui, "海军态势");
        ui.columns(4, |columns| {
            VanillaIron::info_row(&mut columns[0], "运输船", format!("{:.0}", data.convoys));
            VanillaIron::info_row(
                &mut columns[1],
                "舰船库存",
                format!("{:.0}", data.naval_vessels),
            );
            VanillaIron::value_row(
                &mut columns[2],
                "受损舰船",
                damaged.to_string(),
                if damaged > 0 {
                    VanillaIron::WARN
                } else {
                    VanillaIron::MUTED
                },
            );
            VanillaIron::value_row(
                &mut columns[3],
                "最高风险",
                format!("{max_risk:.0}%"),
                if max_risk >= 12.0 {
                    VanillaIron::BAD
                } else {
                    VanillaIron::GOOD
                },
            );
        });
        ui.add_space(8.0);
        if let Some(id) = data.transfer_source_fleet {
            VanillaIron::warning_row(ui, &format!("舰队 #{id} 已设为转移来源。"));
        }
        if let Some(id) = data.pending_move_fleet {
            VanillaIron::warning_row(ui, &format!("舰队 #{id} 正等待地图选择目标海区。"));
        }
        ui.add_space(8.0);
        VanillaIron::section_heading(ui, "舰队状态");
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Grid::new("naval_command_fleet_grid")
                    .striped(true)
                    .spacing(egui::vec2(8.0, 4.0))
                    .show(ui, |ui| {
                        for fleet in &data.fleets {
                            if ui.link(&fleet.name).clicked() {
                                commands.push(NavalCommand::Panel(PanelCommand::OpenDetail(
                                    ActiveDetailPanel::Fleet(FleetDetailTarget {
                                        fleet_id: fleet.id,
                                    }),
                                )));
                            }
                            ui.label(fleet.mission.label());
                            ui.label(format!("海区 {}", fleet.region_id));
                            ui.label(format!("{} 艘", fleet.ship_count));
                            ui.label(
                                RichText::new(format!("受损 {}", fleet.damaged_ships)).color(
                                    if fleet.damaged_ships > 0 {
                                        VanillaIron::WARN
                                    } else {
                                        VanillaIron::MUTED
                                    },
                                ),
                            );
                            ui.label(
                                RichText::new(format!("{:.0}% 风险", fleet.convoy_risk_pct)).color(
                                    if fleet.convoy_risk_pct >= 12.0 {
                                        VanillaIron::BAD
                                    } else {
                                        VanillaIron::MUTED
                                    },
                                ),
                            );
                            ui.end_row();
                        }
                    });
            });
    });
}

fn naval_command_strip(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    data: &NavalData,
    commands: &mut Vec<NavalCommand>,
) {
    ui.allocate_ui_at_rect(rect.shrink2(egui::vec2(8.0, 7.0)), |ui| {
        ui.horizontal_wrapped(|ui| {
            if let Some(fleet) = data.fleets.first() {
                if VanillaIron::compact_button(ui, "舰队详情").clicked() {
                    commands.push(NavalCommand::Panel(PanelCommand::OpenDetail(
                        ActiveDetailPanel::Fleet(FleetDetailTarget { fleet_id: fleet.id }),
                    )));
                }
                if VanillaIron::compact_button(ui, "移动到选中海区").clicked() {
                    commands.push(NavalCommand::MoveToSelectedSeaRegion { fleet_id: fleet.id });
                }
                if VanillaIron::compact_button(ui, "设为转移来源").clicked() {
                    commands.push(NavalCommand::SetTransferSource { fleet_id: fleet.id });
                }
                if VanillaIron::compact_button(ui, "巡逻").clicked() {
                    commands.push(NavalCommand::SetMission {
                        fleet_id: fleet.id,
                        mission: NavalMissionUi::Patrol,
                    });
                }
                if VanillaIron::compact_button(ui, "护航").clicked() {
                    commands.push(NavalCommand::SetMission {
                        fleet_id: fleet.id,
                        mission: NavalMissionUi::ConvoyEscort,
                    });
                }
            }
            if VanillaIron::compact_button(ui, "打开物流").clicked() {
                commands.push(NavalCommand::Panel(PanelCommand::OpenPrimary(
                    ActivePrimaryPanel::Logistics,
                )));
            }
        });
    });
}

impl NavalPanel {
    pub fn show(ctx: &egui::Context, data: &NavalData) -> (bool, Vec<NavalCommand>) {
        command_show_naval(ctx, data)
    }
}

fn render_selection_banner(ui: &mut egui::Ui, data: &NavalData, commands: &mut Vec<NavalCommand>) {
    if data.transfer_source_fleet.is_none() && data.pending_move_fleet.is_none() {
        return;
    }
    egui::Frame::new()
        .fill(Color32::from_rgba_premultiplied(0x33, 0x24, 0x10, 235))
        .stroke(egui::Stroke::new(1.0, GOLD_BRIGHT))
        .inner_margin(egui::Margin::symmetric(10, 7))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                if let Some(id) = data.transfer_source_fleet {
                    ui.label(
                        RichText::new(format!("转移来源：舰队 #{id}"))
                            .strong()
                            .color(GOLD_BRIGHT),
                    );
                    if components::action_button(ui, true, "取消来源").clicked() {
                        commands.push(NavalCommand::ClearTransferSource);
                    }
                }
                if let Some(id) = data.pending_move_fleet {
                    ui.label(
                        RichText::new(format!("等待选择海区：舰队 #{id}"))
                            .strong()
                            .color(WARN),
                    );
                    ui.label(
                        RichText::new("在地图点击海洋省份后会自动移动。ESC/再次点移动可取消。")
                            .small()
                            .color(MUTED),
                    );
                }
            });
        });
    ui.add_space(8.0);
}

fn render_summary(ui: &mut egui::Ui, data: &NavalData) {
    let fleet_count = data.fleets.len();
    let ship_count: usize = data.fleets.iter().map(|fleet| fleet.ship_count).sum();
    let damaged: usize = data.fleets.iter().map(|fleet| fleet.damaged_ships).sum();
    let moving = data
        .fleets
        .iter()
        .filter(|fleet| fleet.target_region_id.is_some())
        .count();
    let high_risk = data
        .fleets
        .iter()
        .filter(|fleet| fleet.convoy_risk_pct >= 8.0)
        .count();

    ui.add_space(6.0);
    components::summary_strip(
        ui,
        &[
            ("舰队", fleet_count.to_string()),
            ("舰船", ship_count.to_string()),
            ("受损", damaged.to_string()),
            ("航行中", moving.to_string()),
            ("高风险海区", high_risk.to_string()),
        ],
    );
    ui.add_space(6.0);
    components::summary_strip(
        ui,
        &[
            ("运输船库存", format!("{:.0}", data.convoys)),
            ("舰船库存", format!("{:.0}", data.naval_vessels)),
        ],
    );
    ui.add_space(8.0);
}

fn render_status_banner(ui: &mut egui::Ui, data: &NavalData) {
    let max_risk = data
        .fleets
        .iter()
        .map(|fleet| fleet.convoy_risk_pct)
        .fold(0.0f32, f32::max);
    let damaged = data.fleets.iter().any(|fleet| fleet.damaged_ships > 0);
    let (label, text, color) = if max_risk >= 12.0 {
        (
            "航线受袭",
            format!("最高运输风险 {:.1}%，需要护航或巡逻舰队。", max_risk),
            BAD,
        )
    } else if damaged {
        (
            "舰队待修",
            "存在受损舰船，停靠有维修能力的港口会逐日恢复。".to_owned(),
            WARN,
        )
    } else if data.fleets.is_empty() {
        (
            "无可用舰队",
            "当前国家没有已编成舰队。造船闭环会把抽象舰船补入预备舰队。".to_owned(),
            MUTED,
        )
    } else {
        (
            "海军待命",
            "舰队状态稳定。根据战区需要分配巡逻、护航或通商破坏任务。".to_owned(),
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

fn render_fleet_section(ui: &mut egui::Ui, data: &NavalData, commands: &mut Vec<NavalCommand>) {
    naval_card(ui, "舰队列表", |ui| {
        if data.fleets.is_empty() {
            components::empty_state(
                ui,
                "暂无舰队",
                "完成造船或加载海军 OOB 后会在这里显示。若有舰船库存，每日 tick 会建立预备舰队。",
            );
            return;
        }

        for fleet in &data.fleets {
            render_fleet_card(ui, fleet, data, commands);
            ui.add_space(7.0);
        }
    });
}

fn render_fleet_card(
    ui: &mut egui::Ui,
    fleet: &FleetEntry,
    data: &NavalData,
    commands: &mut Vec<NavalCommand>,
) {
    let selected_source = data.transfer_source_fleet == Some(fleet.id);
    let pending_move = data.pending_move_fleet == Some(fleet.id);
    let accent = if selected_source || pending_move {
        GOLD_BRIGHT
    } else if fleet.convoy_risk_pct >= 12.0 {
        BAD
    } else if fleet.damaged_ships > 0 || fleet.target_region_id.is_some() {
        WARN
    } else if fleet.mission == NavalMissionUi::Idle {
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
                ui.label(RichText::new(&fleet.name).strong().color(Color32::WHITE));
                if selected_source {
                    status_pill(ui, "转移来源", GOLD_BRIGHT);
                }
                if pending_move {
                    status_pill(ui, "选择海区中", WARN);
                }
                status_pill(ui, format!("{}", fleet.mission.label()), GOLD_BRIGHT);
                status_pill(ui, format!("海区 {}", fleet.region_id), BLUE);
                if let Some(target) = fleet.target_region_id {
                    status_pill(ui, format!("驶向 {}", target), WARN);
                }
            });
            ui.add_space(6.0);
            ui.columns(4, |columns| {
                small_metric(&mut columns[0], "舰船", fleet.ship_count.to_string());
                small_metric(&mut columns[1], "受损", fleet.damaged_ships.to_string());
                small_metric(
                    &mut columns[2],
                    "完整度",
                    format!("{:.0}%", fleet.hp_ratio.clamp(0.0, 1.0) * 100.0),
                );
                small_metric(
                    &mut columns[3],
                    "维修",
                    repair_state_label(&fleet.repair_state),
                );
            });
            ui.add_space(6.0);
            progress_bar(ui, fleet.hp_ratio, hp_color(fleet.hp_ratio), "舰体完整度");
            ui.add_space(5.0);
            let risk_color = risk_color(fleet.convoy_risk_pct);
            ui.label(
                RichText::new(format!("当前海区运输风险：{:.1}%", fleet.convoy_risk_pct))
                    .small()
                    .color(risk_color),
            );
            ui.add_space(7.0);
            render_mission_buttons(ui, fleet, commands);
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                let move_resp = components::action_button(ui, true, "移动到选中海区");
                if move_resp.clicked() {
                    commands.push(NavalCommand::MoveToSelectedSeaRegion { fleet_id: fleet.id });
                }
                ui.label(
                    RichText::new("先在地图选中海洋省份，再点击移动。")
                        .small()
                        .color(MUTED),
                );
            });
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("编制").small().strong().color(GOLD));
                if components::action_button(ui, fleet.ship_count >= 1, "拆出1").clicked() {
                    commands.push(NavalCommand::SplitFleet {
                        fleet_id: fleet.id,
                        count: 1,
                    });
                }
                if components::action_button(ui, fleet.ship_count >= 5, "拆出5").clicked() {
                    commands.push(NavalCommand::SplitFleet {
                        fleet_id: fleet.id,
                        count: 5,
                    });
                }
                if components::action_button(ui, fleet.ship_count >= 2, "拆出半数").clicked() {
                    commands.push(NavalCommand::SplitFleet {
                        fleet_id: fleet.id,
                        count: fleet.ship_count / 2,
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
                    commands.push(NavalCommand::SetTransferSource { fleet_id: fleet.id });
                }
                if let Some(from) = data.transfer_source_fleet {
                    if from != fleet.id {
                        for count in [1usize, 5, 10] {
                            let label = format!("调入{count}");
                            if components::action_button(ui, true, &label).clicked() {
                                commands.push(NavalCommand::TransferShips {
                                    from_fleet_id: from,
                                    to_fleet_id: fleet.id,
                                    count,
                                });
                            }
                        }
                    }
                }
                if components::action_button(ui, fleet.ship_count == 0, "删除空舰队").clicked()
                {
                    commands.push(NavalCommand::DisbandEmptyFleet { fleet_id: fleet.id });
                }
            });
        });
}

fn render_mission_buttons(ui: &mut egui::Ui, fleet: &FleetEntry, commands: &mut Vec<NavalCommand>) {
    ui.label(RichText::new("任务").small().strong().color(GOLD));
    ui.horizontal_wrapped(|ui| {
        for mission in NavalMissionUi::ALL {
            let selected = fleet.mission == mission;
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
                commands.push(NavalCommand::SetMission {
                    fleet_id: fleet.id,
                    mission,
                });
            }
        }
    });
}

fn render_help_card(ui: &mut egui::Ui) {
    naval_card(ui, "操作提示", |ui| {
        ui.label(
            RichText::new("巡逻提高发现与制海存在；护航降低己方运输船损失；通商破坏袭击敌方运输线；打击舰队适合主力舰队寻找海战。")
                .small()
                .color(Color32::from_rgb(0xe0, 0xd2, 0xa8)),
        );
        ui.add_space(5.0);
        ui.label(
            RichText::new(
                "当前为海军 MVP：布雷/扫雷为弱巡逻占位；完整登陆与陆军海运会在后续阶段接入。",
            )
            .small()
            .color(MUTED),
        );
    });
}

fn naval_card(ui: &mut egui::Ui, title: &str, add_contents: impl FnOnce(&mut egui::Ui)) {
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

fn risk_color(value: f32) -> Color32 {
    if value >= 12.0 {
        BAD
    } else if value >= 5.0 {
        WARN
    } else {
        GOOD
    }
}

fn repair_state_label(value: &str) -> String {
    match value {
        "AtSea" => "海上".to_owned(),
        "InPort" => "港内".to_owned(),
        "Repairing" => "维修中".to_owned(),
        other => other.to_owned(),
    }
}
