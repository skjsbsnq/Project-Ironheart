//! Foreign country intelligence card opened from right-clicking foreign provinces.
//!
//! V7 视觉重做：Hero 国家头（国旗+头像+国名+党派+◆ 摘要 strip） + Vic3 状态横幅
//! + 共享 components 卡片 / metric_tile / status_banner。

#![allow(deprecated)]

use crate::diplomacy::{
    render_country_diplomacy_detail, CountryDiplomacyDetail, DiplomacyCommand,
    DiplomaticActionView, RelationFactorEntry, WargoalDetailEntry,
};
use crate::{components, i18n::tr};
use egui::{Color32, Pos2, Rect, RichText, Vec2};

use components::{BAD, GOLD, GOLD_BRIGHT, GOLD_DIM, GOOD, HERO_FILL, MUTED, PARCHMENT, WARN};

pub struct CountryInfoData {
    pub tag: String,
    pub display_name: String,
    pub flag_gfx: String,
    pub leader_name: String,
    pub leader_portrait_key: Option<String>,
    pub ruling_party_label: String,
    pub party_full_name: String,
    pub gdp_gbp: f64,
    pub industrial_level: u32,
    pub military_industrial_level: u32,
    pub construction_points: u32,
    pub division_count: u32,
    pub population: u64,
    pub domestic_population: u64,
    pub colonial_population: u64,
    pub governed_population: u64,
    pub subject_population: u64,
    pub imperial_population: u64,
    pub manpower: u64,
    pub stability: f32,
    pub war_support: f32,
    pub opinion: i16,
    pub at_war: bool,
    pub same_faction: bool,
    pub faction_name: Option<String>,
    pub overlord_name: Option<String>,
    pub subject_names: Vec<String>,
    pub autonomy_level_name: Option<String>,
    pub has_wargoal: bool,
    pub justifying_wargoal: bool,
    pub justify_progress: f32,
    pub justify_days_remaining: u32,
    pub wargoals: Vec<WargoalDetailEntry>,
    pub relation_factors: Vec<RelationFactorEntry>,
    pub justify_action: DiplomaticActionView,
    pub declare_war_action: DiplomaticActionView,
    pub invite_to_faction_action: DiplomaticActionView,
    pub request_access_action: DiplomaticActionView,
}

#[derive(Debug, Clone)]
pub enum CountryInfoCommand {
    JustifyWargoal { target_tag: String },
    DeclareWar { target_tag: String },
    InviteToFaction { target_tag: String },
    RequestMilitaryAccess { target_tag: String },
    Close,
}

pub struct CountryInfoPanel {
    pub open: bool,
    pub data: Option<CountryInfoData>,
}

impl Default for CountryInfoPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl CountryInfoPanel {
    pub fn new() -> Self {
        Self {
            open: false,
            data: None,
        }
    }

    pub fn open_with(&mut self, data: CountryInfoData) {
        self.data = Some(data);
        self.open = true;
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    #[allow(unreachable_code, unused_mut)]
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        icon_bank: &mut crate::icons::IconBank,
        _player_in_faction: bool,
    ) -> Vec<CountryInfoCommand> {
        let mut cmds = Vec::new();
        if !self.open {
            return cmds;
        }
        let Some(data) = self.data.as_ref() else {
            self.open = false;
            return cmds;
        };

        return v9_show_country_info(ctx, data, icon_bank, &mut self.open);

        let mut close = false;
        egui::SidePanel::left("country_info_panel")
            .default_width(420.0)
            .min_width(360.0)
            .max_width(460.0)
            .resizable(false)
            .frame(egui::Frame::new().fill(components::PANEL_CARD))
            .show(ctx, |ui| {
                ui.set_max_width(440.0);
                components::panel_header(
                    ui,
                    &format!("{} {}", &data.display_name, tr("country_info_title")),
                    &mut close,
                );
                ui.add_space(4.0);
                render_hero_country(ui, data, icon_bank);
                ui.add_space(6.0);
                render_status_banner(ui, data);

                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        render_state_card_responsive(ui, data);
                        render_diplomacy_card(ui, data, &mut cmds);
                    });
            });
        if close {
            self.open = false;
        }
        cmds
    }
}

fn v9_show_country_info(
    ctx: &egui::Context,
    data: &CountryInfoData,
    icon_bank: &mut crate::icons::IconBank,
    open: &mut bool,
) -> Vec<CountryInfoCommand> {
    use crate::v9::composites::panel_shell::{
        draw_summary_tiles, draw_tab_strip, PanelClass, PanelShell,
    };
    use crate::v9::tokens::palette;

    let mut cmds = Vec::new();
    let (close, output) = PanelShell::new("country_info_panel_v9", &data.display_name)
        .subtitle(&data.tag)
        .class(PanelClass::Compact)
        .accent(country_relation_accent(data))
        .footer("Q Close  |  Country detail")
        .show(ctx, |ui, layout| {
            draw_summary_tiles(
                ui,
                layout.summary,
                &[
                    (
                        tr("opinion"),
                        format!("{:+}", data.opinion),
                        opinion_color(data.opinion),
                    ),
                    (tr("gdp"), format_gbp(data.gdp_gbp), palette::GOLD),
                    (
                        tr("divisions"),
                        data.division_count.to_string(),
                        palette::WARN,
                    ),
                    (
                        tr("stability"),
                        format!("{:.0}%", data.stability * 100.0),
                        stability_color(data.stability),
                    ),
                ],
            );
            draw_tab_strip(
                ui,
                layout.tabs,
                "国家 / 工业 / 外交",
                country_relation_accent(data),
            );
            let mut inner_cmds = Vec::new();
            v9_country_info_body(ui, layout.body, data, icon_bank, &mut inner_cmds);
            inner_cmds
        });
    if close {
        *open = false;
    }
    cmds.extend(output.unwrap_or_default());
    cmds
}

fn v9_country_info_body(
    ui: &mut egui::Ui,
    rect: Rect,
    data: &CountryInfoData,
    icon_bank: &mut crate::icons::IconBank,
    cmds: &mut Vec<CountryInfoCommand>,
) {
    ui.allocate_ui_at_rect(rect, |ui| {
        ui.set_min_size(rect.size());
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                v9_country_hero(ui, data, icon_bank);
                v9_country_metrics(ui, data);
                v9_country_diplomacy(ui, data, cmds);
            });
    });
}

fn v9_country_hero(
    ui: &mut egui::Ui,
    data: &CountryInfoData,
    icon_bank: &mut crate::icons::IconBank,
) {
    use crate::v9::{
        layout::{GridLayout, Track},
        primitives::{Card, FlagFrame, PortraitFrame},
        tokens::{palette, spacing, TextRole},
    };
    let (rect, _) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 150.0), egui::Sense::hover());
    let inner = Card::new().as_ornate().show_at(ui, rect);
    let grid = GridLayout::new(
        vec![Track::Fr(1.0)],
        vec![Track::Fixed(92.0), Track::Fixed(92.0), Track::Fr(1.0)],
    )
    .with_gutter(spacing::S5, 0.0);
    let cells = grid.measure(inner);
    let flag = icon_bank.get_or_load(&data.flag_gfx);
    FlagFrame::new(&data.tag)
        .accent(country_relation_accent(data))
        .show_at(
            ui,
            GridLayout::cell(&cells, 0, 0).shrink2(Vec2::new(0.0, 30.0)),
            flag,
        );
    let portrait = data
        .leader_portrait_key
        .as_deref()
        .and_then(|gfx| icon_bank.get_or_load(gfx));
    PortraitFrame::new(&data.tag)
        .accent(palette::BRASS_BRIGHT)
        .show_at(
            ui,
            GridLayout::cell(&cells, 0, 1).shrink2(Vec2::new(2.0, 18.0)),
            portrait,
        );
    let text_rect = GridLayout::cell(&cells, 0, 2);
    ui.painter().text(
        Pos2::new(text_rect.left(), text_rect.top() + 4.0),
        egui::Align2::LEFT_TOP,
        &data.display_name,
        TextRole::Display.font_id(),
        palette::GOLD_HOT,
    );
    let leader = if data.leader_name.is_empty() {
        tr("leader_unknown")
    } else {
        &data.leader_name
    };
    ui.painter().text(
        Pos2::new(text_rect.left(), text_rect.top() + 36.0),
        egui::Align2::LEFT_TOP,
        leader,
        TextRole::Subheading.font_id(),
        palette::PARCHMENT,
    );
    let party = if data.party_full_name.is_empty() {
        &data.ruling_party_label
    } else {
        &data.party_full_name
    };
    let galley = ui.painter().layout(
        party.to_owned(),
        TextRole::Body.font_id(),
        palette::PARCHMENT_DIM,
        text_rect.width(),
    );
    ui.painter().galley(
        Pos2::new(text_rect.left(), text_rect.top() + 62.0),
        galley,
        palette::PARCHMENT_DIM,
    );
    let status = if data.at_war {
        "交战"
    } else if data.same_faction {
        "同阵营"
    } else if data.has_wargoal {
        "战争目标"
    } else {
        "情报"
    };
    v9_country_badge(
        ui,
        Rect::from_min_size(
            Pos2::new(text_rect.left(), text_rect.bottom() - 30.0),
            Vec2::new(132.0, 24.0),
        ),
        status,
        country_relation_accent(data),
    );
}

fn v9_country_metrics(ui: &mut egui::Ui, data: &CountryInfoData) {
    use crate::v9::{
        layout::{GridLayout, Track},
        primitives::Card,
        tokens::{palette, spacing, TextRole},
    };
    let (rect, _) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 188.0), egui::Sense::hover());
    let inner = Card::new().as_panel().show_at(ui, rect);
    ui.painter().text(
        Pos2::new(inner.left(), inner.top()),
        egui::Align2::LEFT_TOP,
        tr("country_info_industry"),
        TextRole::Heading.font_id(),
        palette::BRASS_BRIGHT,
    );
    let grid = GridLayout::new(
        vec![Track::Fixed(52.0), Track::Fixed(52.0)],
        vec![Track::Fr(1.0), Track::Fr(1.0), Track::Fr(1.0)],
    )
    .with_gutter(spacing::S4, spacing::S4);
    let cells = grid.measure(Rect::from_min_max(
        Pos2::new(inner.left(), inner.top() + 34.0),
        inner.right_bottom(),
    ));
    let items = [
        ("工业", data.industrial_level.to_string(), palette::GOOD),
        (
            "军工",
            data.military_industrial_level.to_string(),
            palette::WARN,
        ),
        ("建设", data.construction_points.to_string(), palette::GOLD),
        (
            tr("manpower"),
            format_manpower(data.manpower),
            palette::GOOD,
        ),
        (
            "人口",
            format_population(data.population),
            palette::PARCHMENT,
        ),
        (
            "帝国人口",
            format_population(data.imperial_population),
            palette::BRASS_BRIGHT,
        ),
    ];
    for (idx, (label, value, color)) in items.iter().enumerate() {
        let row = idx / 3;
        let col = idx % 3;
        crate::v9::primitives::Tile::new(label, value)
            .accent(*color)
            .show_at(ui, GridLayout::cell(&cells, row, col));
    }
}

fn v9_country_diplomacy(
    ui: &mut egui::Ui,
    data: &CountryInfoData,
    cmds: &mut Vec<CountryInfoCommand>,
) {
    use crate::v9::{
        primitives::{draw_progress_bar, Button, ButtonSize, ButtonVariant, Card},
        tokens::{palette, TextRole},
    };
    let (rect, _) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 246.0), egui::Sense::hover());
    let inner = Card::new().as_panel().show_at(ui, rect);
    ui.painter().text(
        Pos2::new(inner.left(), inner.top()),
        egui::Align2::LEFT_TOP,
        tr("country_info_actions"),
        TextRole::Heading.font_id(),
        palette::BRASS_BRIGHT,
    );
    let mut y = inner.top() + 34.0;
    if let Some(faction) = &data.faction_name {
        v9_country_line(
            ui,
            inner.left(),
            y,
            tr("faction"),
            faction,
            if data.same_faction {
                palette::GOOD
            } else {
                palette::BRASS_BRIGHT
            },
        );
        y += 22.0;
    }
    if let Some(overlord) = &data.overlord_name {
        v9_country_line(ui, inner.left(), y, "宗主国", overlord, palette::WARN);
        y += 22.0;
    }
    if data.justifying_wargoal {
        v9_country_line(
            ui,
            inner.left(),
            y,
            tr("justifying_wargoal"),
            &format!("{}d", data.justify_days_remaining),
            palette::WARN,
        );
        let bar = Rect::from_min_size(
            Pos2::new(inner.left(), y + 22.0),
            Vec2::new(inner.width() * 0.75, 10.0),
        );
        draw_progress_bar(ui, bar, data.justify_progress, palette::WARN);
        y += 44.0;
    } else if data.has_wargoal {
        v9_country_line(
            ui,
            inner.left(),
            y,
            "战争目标",
            tr("wargoal_ready"),
            palette::GOOD,
        );
        y += 24.0;
    }
    for factor in data.relation_factors.iter().take(4) {
        v9_country_line(
            ui,
            inner.left(),
            y,
            &factor.label,
            &factor.value,
            if factor.positive {
                palette::GOOD
            } else {
                palette::WARN
            },
        );
        y += 20.0;
    }

    let action_y = inner.bottom() - 38.0;
    let mut x = inner.left();
    if !data.has_wargoal && !data.justifying_wargoal {
        if Button::new(tr("justify_wargoal"))
            .size(ButtonSize::Md)
            .variant(ButtonVariant::Secondary)
            .enabled(data.justify_action.enabled)
            .show_at(
                ui,
                Rect::from_min_size(Pos2::new(x, action_y), Vec2::new(118.0, 32.0)),
            )
            .on_hover_text(action_hover(&data.justify_action))
            .clicked()
        {
            cmds.push(CountryInfoCommand::JustifyWargoal {
                target_tag: data.tag.clone(),
            });
        }
        x += 126.0;
    }
    if Button::new(tr("declare_war"))
        .size(ButtonSize::Md)
        .variant(ButtonVariant::Danger)
        .enabled(data.declare_war_action.enabled)
        .show_at(
            ui,
            Rect::from_min_size(Pos2::new(x, action_y), Vec2::new(108.0, 32.0)),
        )
        .on_hover_text(action_hover(&data.declare_war_action))
        .clicked()
    {
        cmds.push(CountryInfoCommand::DeclareWar {
            target_tag: data.tag.clone(),
        });
    }
    x += 116.0;
    if Button::new(tr("invite_to_faction"))
        .size(ButtonSize::Md)
        .variant(ButtonVariant::Secondary)
        .enabled(data.invite_to_faction_action.enabled)
        .show_at(
            ui,
            Rect::from_min_size(Pos2::new(x, action_y), Vec2::new(126.0, 32.0)),
        )
        .on_hover_text(action_hover(&data.invite_to_faction_action))
        .clicked()
    {
        cmds.push(CountryInfoCommand::InviteToFaction {
            target_tag: data.tag.clone(),
        });
    }
    x += 134.0;
    if Button::new(tr("request_access"))
        .size(ButtonSize::Md)
        .variant(ButtonVariant::Secondary)
        .enabled(data.request_access_action.enabled)
        .show_at(
            ui,
            Rect::from_min_size(Pos2::new(x, action_y), Vec2::new(116.0, 32.0)),
        )
        .on_hover_text(action_hover(&data.request_access_action))
        .clicked()
    {
        cmds.push(CountryInfoCommand::RequestMilitaryAccess {
            target_tag: data.tag.clone(),
        });
    }
}

fn v9_country_badge(ui: &mut egui::Ui, rect: Rect, label: &str, color: Color32) {
    crate::v9::paint::paint_bevel(
        ui.painter(),
        rect,
        crate::v9::palette::SOOT_BLACK,
        color,
        1.0,
    );
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        crate::v9::TextRole::Caption.font_id(),
        color,
    );
}

fn v9_country_line(ui: &mut egui::Ui, x: f32, y: f32, label: &str, value: &str, color: Color32) {
    ui.painter().text(
        Pos2::new(x, y),
        egui::Align2::LEFT_TOP,
        label,
        crate::v9::TextRole::Caption.font_id(),
        crate::v9::palette::MUTED,
    );
    ui.painter().text(
        Pos2::new(x + 128.0, y),
        egui::Align2::LEFT_TOP,
        value,
        crate::v9::TextRole::Body.font_id(),
        color,
    );
}

fn country_relation_accent(data: &CountryInfoData) -> Color32 {
    if data.at_war {
        BAD
    } else if data.same_faction {
        GOOD
    } else if data.has_wargoal || data.justifying_wargoal {
        WARN
    } else {
        GOLD
    }
}

fn action_hover(action: &DiplomaticActionView) -> String {
    if action.enabled {
        action.preview.clone()
    } else {
        action
            .reason
            .clone()
            .unwrap_or_else(|| action.preview.clone())
    }
}

fn render_hero_country(
    ui: &mut egui::Ui,
    data: &CountryInfoData,
    icon_bank: &mut crate::icons::IconBank,
) {
    egui::Frame::new()
        .fill(HERO_FILL)
        .stroke(egui::Stroke::new(1.5, GOLD_DIM))
        .inner_margin(egui::Margin::symmetric(14, 10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                render_flag(ui, data, icon_bank);
                ui.add_space(4.0);
                render_portrait(ui, data, icon_bank);
                ui.add_space(6.0);
                ui.vertical(|ui| {
                    ui.add(
                        egui::Label::new(
                            RichText::new(&data.display_name)
                                .strong()
                                .color(GOLD_BRIGHT)
                                .size(20.0),
                        )
                        .wrap(),
                    );
                    let leader = if data.leader_name.is_empty() {
                        tr("leader_unknown").to_owned()
                    } else {
                        data.leader_name.clone()
                    };
                    ui.add(
                        egui::Label::new(RichText::new(leader).size(12.5).color(PARCHMENT)).wrap(),
                    );
                    let party = if data.party_full_name.is_empty() {
                        data.ruling_party_label.clone()
                    } else {
                        data.party_full_name.clone()
                    };
                    ui.add(
                        egui::Label::new(RichText::new(party).size(11.0).color(MUTED).italics())
                            .wrap(),
                    );
                });
            });

            ui.add_space(8.0);
            components::ornament_divider(ui);
            ui.add_space(6.0);

            compact_metric_stack(
                ui,
                &[
                    (
                        tr("opinion").to_owned(),
                        format!("{:+}", data.opinion),
                        opinion_color(data.opinion),
                    ),
                    (
                        tr("divisions").to_owned(),
                        data.division_count.to_string(),
                        WARN,
                    ),
                    ("GDP".to_owned(), format_gbp(data.gdp_gbp), GOLD_BRIGHT),
                ],
            );
        });
}

fn render_status_banner(ui: &mut egui::Ui, data: &CountryInfoData) {
    let (label, text, color) = if data.at_war {
        ("交战中", "我国正在与该国交战。".to_owned(), BAD)
    } else if data.same_faction {
        ("同阵营", "该国与我国同属一个阵营。".to_owned(), GOOD)
    } else if data.has_wargoal {
        (
            "战争目标就绪",
            "我国已有可用于宣战的正当化战争目标。".to_owned(),
            WARN,
        )
    } else if data.opinion > 50 {
        (
            "关系良好",
            "该国对我国态度较好，外交行动更可能可用。".to_owned(),
            GOOD,
        )
    } else {
        (
            "常规情报",
            "该国暂无直接军事冲突，外交动作取决于关系与战争目标。".to_owned(),
            GOLD,
        )
    };

    components::status_banner(ui, color, label, &text);
}

#[allow(dead_code)]
fn render_state_card(ui: &mut egui::Ui, data: &CountryInfoData) {
    let colonial_or_subject_population = data.colonial_population + data.subject_population;
    components::section(ui, tr("country_info_industry"), |ui| {
        ui.columns(4, |c| {
            components::metric_tile(&mut c[0], "GDP", format_gbp(data.gdp_gbp), GOLD);
            components::metric_tile(
                &mut c[1],
                "工业等级",
                data.industrial_level.to_string(),
                GOOD,
            );
            components::metric_tile(
                &mut c[2],
                "军工等级",
                data.military_industrial_level.to_string(),
                WARN,
            );
            components::metric_tile(
                &mut c[3],
                "建造力",
                data.construction_points.to_string(),
                GOLD_BRIGHT,
            );
        });
        ui.add_space(6.0);
        ui.columns(3, |c| {
            components::metric_tile(
                &mut c[0],
                tr("stability"),
                format!("{:.0}%", data.stability * 100.0),
                stability_color(data.stability),
            );
            components::metric_tile(
                &mut c[1],
                tr("war_support"),
                format!("{:.0}%", data.war_support * 100.0),
                warsupport_color(data.war_support),
            );
            components::metric_tile(
                &mut c[2],
                tr("manpower"),
                format_manpower(data.manpower),
                GOOD,
            );
        });
        ui.add_space(6.0);
        ui.columns(3, |c| {
            components::metric_tile(
                &mut c[0],
                "本土人口",
                format_population(data.domestic_population),
                GOOD,
            );
            components::metric_tile(
                &mut c[1],
                "殖民/属国人口",
                format_population(colonial_or_subject_population),
                WARN,
            );
            components::metric_tile(
                &mut c[2],
                "总人口",
                format_population(data.population),
                GOLD,
            );
        });
        ui.add_space(4.0);
        components::kv_row(
            ui,
            "管辖人口",
            format_population(data.governed_population),
            PARCHMENT,
        );
        components::kv_row(
            ui,
            "其中属国",
            format_population(data.subject_population),
            WARN,
        );
        components::kv_row(
            ui,
            "帝国人口",
            format_population(data.imperial_population),
            GOLD,
        );
    });
}

fn render_state_card_responsive(ui: &mut egui::Ui, data: &CountryInfoData) {
    let colonial_or_subject_population = data.colonial_population + data.subject_population;
    components::section(ui, tr("country_info_industry"), |ui| {
        compact_metric_stack(
            ui,
            &[
                ("GDP".to_owned(), format_gbp(data.gdp_gbp), GOLD),
                (
                    "工业等级".to_owned(),
                    data.industrial_level.to_string(),
                    GOOD,
                ),
                (
                    "军工等级".to_owned(),
                    data.military_industrial_level.to_string(),
                    WARN,
                ),
                (
                    "建造力".to_owned(),
                    data.construction_points.to_string(),
                    GOLD_BRIGHT,
                ),
            ],
        );
        ui.add_space(6.0);
        compact_metric_stack(
            ui,
            &[
                (
                    tr("stability").to_owned(),
                    format!("{:.0}%", data.stability * 100.0),
                    stability_color(data.stability),
                ),
                (
                    tr("war_support").to_owned(),
                    format!("{:.0}%", data.war_support * 100.0),
                    warsupport_color(data.war_support),
                ),
                (
                    tr("manpower").to_owned(),
                    format_manpower(data.manpower),
                    GOOD,
                ),
            ],
        );
        ui.add_space(6.0);
        compact_metric_stack(
            ui,
            &[
                (
                    "本土人口".to_owned(),
                    format_population(data.domestic_population),
                    GOOD,
                ),
                (
                    "殖民/属国".to_owned(),
                    format_population(colonial_or_subject_population),
                    WARN,
                ),
                (
                    "总人口".to_owned(),
                    format_population(data.population),
                    GOLD,
                ),
            ],
        );
        ui.add_space(4.0);
        components::kv_row(
            ui,
            "管辖人口",
            format_population(data.governed_population),
            PARCHMENT,
        );
        components::kv_row(
            ui,
            "其中属国",
            format_population(data.subject_population),
            WARN,
        );
        components::kv_row(
            ui,
            "帝国人口",
            format_population(data.imperial_population),
            GOLD,
        );
    });
}

fn compact_metric_stack(ui: &mut egui::Ui, items: &[(String, String, Color32)]) {
    for row in items.chunks(2) {
        ui.columns(2, |cols| {
            for (col, (label, value, color)) in row.iter().enumerate() {
                compact_metric_tile(&mut cols[col], label, value, *color);
            }
        });
        ui.add_space(4.0);
    }
}

fn compact_metric_tile(ui: &mut egui::Ui, label: &str, value: &str, color: Color32) {
    let inner = egui::Frame::new()
        .fill(components::PANEL_CARD_DEEP)
        .stroke(egui::Stroke::new(1.0, components::STROKE_TILE))
        .inner_margin(egui::Margin {
            left: 12,
            right: 10,
            top: 7,
            bottom: 7,
        })
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.add(egui::Label::new(RichText::new(label).size(11.0).color(MUTED)).wrap());
            ui.add_space(2.0);
            ui.add(egui::Label::new(RichText::new(value).strong().size(16.0).color(color)).wrap());
        });
    let rect = inner.response.rect;
    let bar = egui::Rect::from_min_size(
        rect.left_top() + egui::vec2(1.0, 1.0),
        egui::vec2(3.0, rect.height() - 2.0),
    );
    ui.painter().rect_filled(bar, 0.0, color);
}

fn render_diplomacy_card(
    ui: &mut egui::Ui,
    data: &CountryInfoData,
    cmds: &mut Vec<CountryInfoCommand>,
) {
    components::section(ui, tr("country_info_actions"), |ui| {
        let mut diplomacy_cmds = Vec::new();
        render_country_diplomacy_detail(ui, &data.diplomacy_detail(), &mut diplomacy_cmds);
        for cmd in diplomacy_cmds {
            cmds.push(country_command_from_diplomacy(cmd));
        }
    });
}

impl CountryInfoData {
    fn diplomacy_detail(&self) -> CountryDiplomacyDetail {
        CountryDiplomacyDetail {
            tag: self.tag.clone(),
            display_name: self.display_name.clone(),
            opinion: self.opinion,
            at_war: self.at_war,
            same_faction: self.same_faction,
            faction_name: self.faction_name.clone(),
            overlord_name: self.overlord_name.clone(),
            subject_names: self.subject_names.clone(),
            autonomy_level_name: self.autonomy_level_name.clone(),
            domestic_population: self.domestic_population,
            colonial_population: self.colonial_population,
            governed_population: self.governed_population,
            subject_population: self.subject_population,
            imperial_population: self.imperial_population,
            has_wargoal: self.has_wargoal,
            justifying_wargoal: self.justifying_wargoal,
            justify_progress: self.justify_progress,
            justify_days_remaining: self.justify_days_remaining,
            wargoals: self.wargoals.clone(),
            relation_factors: self.relation_factors.clone(),
            justify_action: self.justify_action.clone(),
            declare_war_action: self.declare_war_action.clone(),
            invite_to_faction_action: self.invite_to_faction_action.clone(),
            request_access_action: self.request_access_action.clone(),
        }
    }
}

fn country_command_from_diplomacy(cmd: DiplomacyCommand) -> CountryInfoCommand {
    match cmd {
        DiplomacyCommand::JustifyWargoal(tag) => {
            CountryInfoCommand::JustifyWargoal { target_tag: tag }
        }
        DiplomacyCommand::DeclareWar(tag) => CountryInfoCommand::DeclareWar { target_tag: tag },
        DiplomacyCommand::InviteToFaction(tag) => {
            CountryInfoCommand::InviteToFaction { target_tag: tag }
        }
        DiplomacyCommand::RequestMilitaryAccess(tag) => {
            CountryInfoCommand::RequestMilitaryAccess { target_tag: tag }
        }
        DiplomacyCommand::CreateFaction
        | DiplomacyCommand::LeaveFaction
        | DiplomacyCommand::ResolvePeace { .. } => CountryInfoCommand::Close,
    }
}

fn render_flag(ui: &mut egui::Ui, data: &CountryInfoData, icon_bank: &mut crate::icons::IconBank) {
    let size = egui::Vec2::new(72.0, 44.0);
    if let Some(handle) = icon_bank.get_or_load(&data.flag_gfx) {
        ui.add(
            egui::Image::from_texture(handle)
                .fit_to_exact_size(size)
                .corner_radius(2.0),
        );
    } else {
        placeholder(ui, size, &data.tag, 13.0);
    }
}

fn render_portrait(
    ui: &mut egui::Ui,
    data: &CountryInfoData,
    icon_bank: &mut crate::icons::IconBank,
) {
    let size = egui::Vec2::new(72.0, 72.0);
    if let Some(gfx) = data.leader_portrait_key.as_deref() {
        if let Some(handle) = icon_bank.get_or_load(gfx) {
            ui.add(
                egui::Image::from_texture(handle)
                    .fit_to_exact_size(size)
                    .corner_radius(2.0),
            );
            return;
        }
    }
    placeholder(ui, size, "?", 26.0);
}

fn placeholder(ui: &mut egui::Ui, size: egui::Vec2, label: &str, font_size: f32) {
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    ui.painter().rect_filled(rect, 2.0, Color32::from_gray(45));
    ui.painter().rect_stroke(
        rect,
        2.0,
        egui::Stroke::new(1.0, GOLD),
        egui::epaint::StrokeKind::Outside,
    );
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::proportional(font_size),
        GOLD,
    );
}

fn opinion_color(opinion: i16) -> Color32 {
    if opinion >= 30 {
        GOOD
    } else if opinion >= -30 {
        GOLD
    } else {
        BAD
    }
}

fn stability_color(v: f32) -> Color32 {
    if v >= 0.6 {
        GOOD
    } else if v >= 0.3 {
        WARN
    } else {
        BAD
    }
}

fn warsupport_color(v: f32) -> Color32 {
    if v >= 0.5 {
        GOOD
    } else if v >= 0.25 {
        WARN
    } else {
        BAD
    }
}

fn format_manpower(mp: u64) -> String {
    if mp >= 1_000_000 {
        format!("{:.1}M", mp as f64 / 1_000_000.0)
    } else if mp >= 1_000 {
        format!("{:.0}k", mp as f64 / 1_000.0)
    } else {
        mp.to_string()
    }
}

fn format_population(population: u64) -> String {
    if population >= 1_000_000_000 {
        format!("{:.2}B", population as f64 / 1_000_000_000.0)
    } else if population >= 1_000_000 {
        format!("{:.1}M", population as f64 / 1_000_000.0)
    } else if population >= 1_000 {
        format!("{:.0}k", population as f64 / 1_000.0)
    } else {
        population.to_string()
    }
}

fn format_gbp(value: f64) -> String {
    if value.abs() >= 1_000_000_000.0 {
        format!("£{:.1}B", value / 1_000_000_000.0)
    } else if value.abs() >= 1_000_000.0 {
        format!("£{:.0}M", value / 1_000_000.0)
    } else if value.abs() >= 1_000.0 {
        format!("£{:.0}K", value / 1_000.0)
    } else {
        format!("£{:.0}", value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_manpower_in_millions() {
        assert_eq!(format_manpower(5_230_000), "5.2M");
    }

    #[test]
    fn formats_population_in_billions() {
        assert_eq!(format_population(1_250_000_000), "1.25B");
    }

    #[test]
    fn formats_population_in_millions() {
        assert_eq!(format_population(430_000_000), "430.0M");
    }
}
