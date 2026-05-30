//! Diplomacy panel and shared country diplomacy detail rendering.

#![allow(dead_code, deprecated)]

use crate::{components, i18n::tr};
use egui::{Color32, Pos2, Rect, RichText, Sense, Vec2};

const GOLD: Color32 = Color32::from_rgb(0xc9, 0xa5, 0x5b);
const PANEL_CARD: Color32 = Color32::from_rgb(0x24, 0x1a, 0x12);
const PANEL_CARD_SOFT: Color32 = Color32::from_rgb(0x31, 0x24, 0x18);
const STROKE_DARK: Color32 = Color32::from_rgb(0x5a, 0x44, 0x2c);
const GOOD: Color32 = Color32::from_rgb(0x70, 0xc8, 0x78);
const WARN: Color32 = Color32::from_rgb(0xff, 0xc0, 0x60);
const BAD: Color32 = Color32::from_rgb(0xe0, 0x60, 0x58);

#[derive(Clone)]
pub struct CountryEntry {
    pub tag: String,
    pub flag_gfx: String,
    pub opinion: i16,
    pub at_war: bool,
    pub same_faction: bool,
    pub autonomy_summary: Option<String>,
    pub leader_name: String,
    pub leader_portrait_key: Option<String>,
    pub detail: Option<CountryDiplomacyDetail>,
}

#[derive(Clone, Debug)]
pub struct DiplomaticActionView {
    pub enabled: bool,
    pub reason: Option<String>,
    pub preview: String,
}

impl DiplomaticActionView {
    pub fn enabled(preview: impl Into<String>) -> Self {
        Self {
            enabled: true,
            reason: None,
            preview: preview.into(),
        }
    }

    pub fn disabled(preview: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            enabled: false,
            reason: Some(reason.into()),
            preview: preview.into(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct CountryDiplomacyDetail {
    pub tag: String,
    pub display_name: String,
    pub opinion: i16,
    pub at_war: bool,
    pub same_faction: bool,
    pub faction_name: Option<String>,
    pub overlord_name: Option<String>,
    pub subject_names: Vec<String>,
    pub autonomy_level_name: Option<String>,
    pub domestic_population: u64,
    pub colonial_population: u64,
    pub governed_population: u64,
    pub subject_population: u64,
    pub imperial_population: u64,
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

#[derive(Clone, Debug)]
pub struct WargoalDetailEntry {
    pub kind: String,
    pub target_state: Option<u16>,
    pub status: String,
    pub progress: f32,
    pub days_remaining: u32,
    pub source: String,
}

#[derive(Clone, Debug)]
pub struct RelationFactorEntry {
    pub label: String,
    pub value: String,
    pub positive: bool,
}

#[derive(Clone)]
pub struct FactionEntry {
    pub name: String,
    pub leader_tag: String,
    pub member_tags: Vec<String>,
}

#[derive(Clone)]
pub struct DiplomaticRequestEntry {
    pub from_tag: String,
    pub to_tag: String,
    pub kind: String,
    pub status: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeaceSide {
    Attacker,
    Defender,
}

#[derive(Clone)]
pub struct PeaceWargoalEntry {
    pub claimant_tag: String,
    pub target_tag: String,
    pub kind: String,
    pub target_state: Option<u16>,
}

#[derive(Clone)]
pub struct PeaceWarEntry {
    pub id: u32,
    pub primary_attacker_tag: String,
    pub primary_defender_tag: String,
    pub attacker_tags: Vec<String>,
    pub defender_tags: Vec<String>,
    pub attacker_score: f32,
    pub defender_score: f32,
    pub player_side: Option<PeaceSide>,
    pub wargoals: Vec<PeaceWargoalEntry>,
    pub attacker_peace_action: DiplomaticActionView,
    pub defender_peace_action: DiplomaticActionView,
}

#[derive(Clone)]
pub struct DiplomacyData {
    pub player_tag: String,
    pub player_faction: Option<FactionEntry>,
    pub all_factions: Vec<FactionEntry>,
    pub countries: Vec<CountryEntry>,
    pub active_wars: Vec<PeaceWarEntry>,
    pub requests: Vec<DiplomaticRequestEntry>,
    pub world_tension: f32,
}

#[derive(Debug)]
pub enum DiplomacyCommand {
    JustifyWargoal(String),
    DeclareWar(String),
    CreateFaction,
    LeaveFaction,
    InviteToFaction(String),
    RequestMilitaryAccess(String),
    ResolvePeace {
        war_id: u32,
        winning_side: PeaceSide,
    },
}

pub struct DiplomacyPanel;

impl DiplomacyPanel {
    #[allow(unreachable_code)]
    pub fn show(
        ctx: &egui::Context,
        data: &DiplomacyData,
        sort_by_opinion: &mut bool,
        selected_tag: &mut Option<String>,
        icon_bank: &mut crate::icons::IconBank,
    ) -> (bool, Vec<DiplomacyCommand>) {
        return v9_show_diplomacy(ctx, data, sort_by_opinion, selected_tag, icon_bank);

        let mut close = false;
        let mut cmds = Vec::new();

        egui::SidePanel::left("diplomacy_panel")
            .default_width(380.0)
            .min_width(320.0)
            .max_width(460.0)
            .resizable(true)
            .show(ctx, |ui| {
                ui.set_max_width(440.0);
                components::panel_header(ui, tr("diplomacy"), &mut close);
                render_summary(ui, data);
                render_status_banner(ui, data);

                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        render_wars(ui, data, &mut cmds);
                        render_home_card(ui, data, &mut cmds);
                        render_requests(ui, data);
                        render_country_list(ui, data, sort_by_opinion, selected_tag, icon_bank);
                        if let Some(detail) = selected_detail(data, selected_tag) {
                            diplomacy_card(ui, "选中国家详情", |ui| {
                                render_country_diplomacy_detail(ui, detail, &mut cmds);
                            });
                        }
                        render_factions(ui, data);
                    });
            });

        (close, cmds)
    }
}

fn v9_show_diplomacy(
    ctx: &egui::Context,
    data: &DiplomacyData,
    sort_by_opinion: &mut bool,
    selected_tag: &mut Option<String>,
    icon_bank: &mut crate::icons::IconBank,
) -> (bool, Vec<DiplomacyCommand>) {
    use crate::v9::composites::panel_shell::{
        draw_summary_tiles, draw_tab_strip, PanelClass, PanelShell,
    };
    use crate::v9::tokens::palette;

    let faction = data
        .player_faction
        .as_ref()
        .map(|f| f.name.clone())
        .unwrap_or_else(|| tr("none").to_owned());
    let (close, output) = PanelShell::new("diplomacy_panel_v9", tr("diplomacy"))
        .subtitle(&data.player_tag)
        .class(PanelClass::MilitaryDiplomacy)
        .accent(palette::INFO)
        .footer("Q Close  |  Diplomacy")
        .show(ctx, |ui, layout| {
            draw_summary_tiles(
                ui,
                layout.summary,
                &[
                    (
                        tr("world_tension"),
                        format!("{:.0}%", data.world_tension),
                        palette::WARN,
                    ),
                    (
                        "战争",
                        data.active_wars.len().to_string(),
                        if data.active_wars.is_empty() {
                            palette::GOOD
                        } else {
                            palette::BAD
                        },
                    ),
                    (tr("faction"), faction, palette::BRASS_BRIGHT),
                    (
                        tr("countries"),
                        data.countries.len().to_string(),
                        palette::PARCHMENT,
                    ),
                ],
            );
            draw_tab_strip(ui, layout.tabs, "国家详情 / 国旗 / 阵营标识", palette::INFO);
            let mut cmds = Vec::new();
            v9_diplomacy_body(
                ui,
                layout.body,
                data,
                sort_by_opinion,
                selected_tag,
                icon_bank,
                &mut cmds,
            );
            cmds
        });
    (close, output.unwrap_or_default())
}

fn v9_diplomacy_body(
    ui: &mut egui::Ui,
    rect: Rect,
    data: &DiplomacyData,
    sort_by_opinion: &mut bool,
    selected_tag: &mut Option<String>,
    icon_bank: &mut crate::icons::IconBank,
    cmds: &mut Vec<DiplomacyCommand>,
) {
    use crate::v9::layout::{GridLayout, Track};
    use crate::v9::tokens::spacing;
    ui.allocate_ui_at_rect(rect, |ui| {
        let grid = GridLayout::new(vec![Track::Fr(1.0)], vec![Track::Fr(0.38), Track::Fr(0.62)])
            .with_gutter(spacing::S5, 0.0);
        let cells = grid.measure(rect);
        v9_diplomacy_country_list(
            ui,
            GridLayout::cell(&cells, 0, 0),
            data,
            sort_by_opinion,
            selected_tag,
            icon_bank,
        );
        v9_diplomacy_detail(
            ui,
            GridLayout::cell(&cells, 0, 1),
            data,
            selected_tag,
            icon_bank,
            cmds,
        );
    });
}

fn v9_diplomacy_country_list(
    ui: &mut egui::Ui,
    rect: Rect,
    data: &DiplomacyData,
    sort_by_opinion: &mut bool,
    selected_tag: &mut Option<String>,
    icon_bank: &mut crate::icons::IconBank,
) {
    use crate::v9::{
        primitives::{Button, ButtonSize, ButtonVariant, Card},
        tokens::{palette, spacing, TextRole},
    };
    let inner = Card::new().as_panel().show_at(ui, rect);
    ui.painter().text(
        Pos2::new(inner.left(), inner.top()),
        egui::Align2::LEFT_TOP,
        "国家",
        TextRole::Heading.font_id(),
        palette::BRASS_BRIGHT,
    );
    let alpha_rect = Rect::from_min_size(
        Pos2::new(inner.right() - 168.0, inner.top()),
        Vec2::new(76.0, 24.0),
    );
    if Button::new("名称")
        .size(ButtonSize::Sm)
        .variant(if *sort_by_opinion {
            ButtonVariant::Ghost
        } else {
            ButtonVariant::Secondary
        })
        .show_at(ui, alpha_rect)
        .clicked()
    {
        *sort_by_opinion = false;
    }
    let opinion_rect = Rect::from_min_size(
        Pos2::new(inner.right() - 84.0, inner.top()),
        Vec2::new(80.0, 24.0),
    );
    if Button::new("关系")
        .size(ButtonSize::Sm)
        .variant(if *sort_by_opinion {
            ButtonVariant::Secondary
        } else {
            ButtonVariant::Ghost
        })
        .show_at(ui, opinion_rect)
        .clicked()
    {
        *sort_by_opinion = true;
    }

    let list_rect = Rect::from_min_max(
        Pos2::new(inner.left(), inner.top() + 34.0),
        inner.right_bottom(),
    );
    ui.allocate_ui_at_rect(list_rect, |ui| {
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let mut sorted: Vec<&CountryEntry> = data.countries.iter().collect();
                if *sort_by_opinion {
                    sorted.sort_by(|a, b| b.opinion.cmp(&a.opinion));
                } else {
                    sorted.sort_by(|a, b| a.tag.cmp(&b.tag));
                }
                for country in sorted {
                    let (row_rect, response) = ui
                        .allocate_exact_size(Vec2::new(ui.available_width(), 58.0), Sense::click());
                    let selected = selected_tag.as_deref() == Some(country.tag.as_str());
                    v9_country_row(ui, row_rect, country, selected, icon_bank);
                    if response.clicked() {
                        *selected_tag = Some(country.tag.clone());
                    }
                    ui.add_space(spacing::S2);
                }
            });
    });
}

fn v9_country_row(
    ui: &mut egui::Ui,
    rect: Rect,
    country: &CountryEntry,
    selected: bool,
    icon_bank: &mut crate::icons::IconBank,
) {
    use crate::v9::{
        paint,
        primitives::FlagFrame,
        tokens::{palette, TextRole},
    };
    let fill = if selected {
        palette::OIL_BLACK
    } else {
        palette::SOOT_BLACK
    };
    paint::paint_bevel(
        ui.painter(),
        rect,
        fill,
        if selected {
            palette::GOLD
        } else {
            palette::EDGE_DARK
        },
        1.0,
    );
    let flag_rect = Rect::from_min_size(
        Pos2::new(rect.left() + 6.0, rect.top() + 9.0),
        Vec2::new(62.0, 40.0),
    );
    let flag = icon_bank.get_or_load(&country.flag_gfx);
    FlagFrame::new(&country.tag).show_at(ui, flag_rect, flag);
    let name_color = country_color(country);
    ui.painter().text(
        Pos2::new(flag_rect.right() + 10.0, rect.top() + 9.0),
        egui::Align2::LEFT_TOP,
        tr(&country.tag),
        TextRole::Subheading.font_id(),
        name_color,
    );
    ui.painter().text(
        Pos2::new(flag_rect.right() + 10.0, rect.top() + 31.0),
        egui::Align2::LEFT_TOP,
        if country.leader_name.is_empty() {
            "-"
        } else {
            &country.leader_name
        },
        TextRole::Caption.font_id(),
        palette::PARCHMENT_DIM,
    );
    ui.painter().text(
        Pos2::new(rect.right() - 10.0, rect.center().y),
        egui::Align2::RIGHT_CENTER,
        format!("{:+}", country.opinion),
        TextRole::Numeric.font_id(),
        if country.opinion >= 0 {
            palette::GOOD
        } else {
            palette::BAD
        },
    );
}

fn v9_diplomacy_detail(
    ui: &mut egui::Ui,
    rect: Rect,
    data: &DiplomacyData,
    selected_tag: &mut Option<String>,
    icon_bank: &mut crate::icons::IconBank,
    cmds: &mut Vec<DiplomacyCommand>,
) {
    use crate::v9::{
        primitives::{draw_progress_bar, ButtonVariant, Card, FlagFrame},
        tokens::{palette, spacing, TextRole},
    };
    let inner = Card::new().as_panel().show_at(ui, rect);
    if selected_tag.is_none() {
        *selected_tag = data.countries.first().map(|c| c.tag.clone());
    }
    let selected_country = data
        .countries
        .iter()
        .find(|c| selected_tag.as_deref() == Some(c.tag.as_str()));
    let Some(country) = selected_country else {
        crate::v9::composites::panel_shell::draw_empty_state(
            ui,
            inner,
            "未选择国家",
            "请从列表中选择一个国家。",
        );
        return;
    };
    let Some(detail) = country.detail.as_ref() else {
        crate::v9::composites::panel_shell::draw_empty_state(
            ui,
            inner,
            &country.tag,
            "暂无外交详情。",
        );
        return;
    };

    let flag_rect = Rect::from_min_size(inner.left_top(), Vec2::new(96.0, 62.0));
    let flag = icon_bank.get_or_load(&country.flag_gfx);
    FlagFrame::new(&country.tag)
        .accent(relation_color(detail))
        .show_at(ui, flag_rect, flag);
    ui.painter().text(
        Pos2::new(flag_rect.right() + spacing::S5, inner.top() + 2.0),
        egui::Align2::LEFT_TOP,
        &detail.display_name,
        TextRole::Display.font_id(),
        palette::GOLD_HOT,
    );
    v9_diplomacy_badge(
        ui,
        Rect::from_min_size(
            Pos2::new(flag_rect.right() + spacing::S5, inner.top() + 34.0),
            Vec2::new(140.0, 24.0),
        ),
        if detail.at_war { "交战" } else { "和平" },
        if detail.at_war {
            palette::BAD
        } else {
            palette::GOOD
        },
    );
    let faction_label = detail.faction_name.as_deref().unwrap_or("无阵营");
    v9_diplomacy_badge(
        ui,
        Rect::from_min_size(
            Pos2::new(flag_rect.right() + spacing::S5 + 150.0, inner.top() + 34.0),
            Vec2::new(180.0, 24.0),
        ),
        faction_label,
        if detail.same_faction {
            palette::GOOD
        } else {
            palette::BRASS_BRIGHT
        },
    );

    let metric_y = inner.top() + 82.0;
    v9_detail_metric(
        ui,
        Rect::from_min_size(Pos2::new(inner.left(), metric_y), Vec2::new(130.0, 48.0)),
        tr("opinion"),
        &format!("{:+}", detail.opinion),
        relation_color(detail),
    );
    v9_detail_metric(
        ui,
        Rect::from_min_size(
            Pos2::new(inner.left() + 140.0, metric_y),
            Vec2::new(150.0, 48.0),
        ),
        "本土人口",
        &format_population(detail.domestic_population),
        palette::GOOD,
    );
    v9_detail_metric(
        ui,
        Rect::from_min_size(
            Pos2::new(inner.left() + 300.0, metric_y),
            Vec2::new(150.0, 48.0),
        ),
        "统治人口",
        &format_population(detail.governed_population),
        palette::GOLD,
    );

    let mut y = metric_y + 64.0;
    if let Some(overlord) = &detail.overlord_name {
        v9_text_line(ui, inner.left(), y, "宗主国", overlord, palette::WARN);
        y += 22.0;
    }
    if !detail.subject_names.is_empty() {
        v9_text_line(
            ui,
            inner.left(),
            y,
            "附属国",
            &detail.subject_names.join(", "),
            palette::INFO,
        );
        y += 22.0;
    }
    if detail.justifying_wargoal {
        ui.painter().text(
            Pos2::new(inner.left(), y),
            egui::Align2::LEFT_TOP,
            tr("justifying_wargoal"),
            TextRole::Subheading.font_id(),
            palette::WARN,
        );
        let bar = Rect::from_min_size(
            Pos2::new(inner.left(), y + 24.0),
            Vec2::new(inner.width() * 0.66, 10.0),
        );
        draw_progress_bar(ui, bar, detail.justify_progress, palette::WARN);
        y += 48.0;
    } else if detail.has_wargoal {
        v9_text_line(
            ui,
            inner.left(),
            y,
            "战争目标",
            tr("wargoal_ready"),
            palette::GOOD,
        );
        y += 28.0;
    }
    if !detail.wargoals.is_empty() {
        ui.painter().text(
            Pos2::new(inner.left(), y),
            egui::Align2::LEFT_TOP,
            "战争目标",
            TextRole::Heading.font_id(),
            palette::BRASS_BRIGHT,
        );
        y += 28.0;
        for goal in detail.wargoals.iter().take(4) {
            let state = goal
                .target_state
                .map(|s| format!(" 州 {}", s))
                .unwrap_or_default();
            v9_text_line(
                ui,
                inner.left(),
                y,
                &goal.kind,
                &format!("{}{}", goal.status, state),
                if goal.progress >= 1.0 {
                    palette::GOOD
                } else {
                    palette::WARN
                },
            );
            y += 22.0;
        }
    }
    if !detail.relation_factors.is_empty() {
        ui.painter().text(
            Pos2::new(inner.left(), y + 4.0),
            egui::Align2::LEFT_TOP,
            "关系因素",
            TextRole::Heading.font_id(),
            palette::BRASS_BRIGHT,
        );
        y += 34.0;
        for factor in detail.relation_factors.iter().take(5) {
            v9_text_line(
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
            y += 21.0;
        }
    }

    let action_y = inner.bottom() - 40.0;
    let mut x = inner.left();
    if !detail.has_wargoal && !detail.justifying_wargoal {
        if v9_action_button(
            ui,
            Rect::from_min_size(Pos2::new(x, action_y), Vec2::new(118.0, 32.0)),
            tr("justify_wargoal"),
            &detail.justify_action,
            ButtonVariant::Secondary,
        ) {
            cmds.push(DiplomacyCommand::JustifyWargoal(detail.tag.clone()));
        }
        x += 126.0;
    }
    if v9_action_button(
        ui,
        Rect::from_min_size(Pos2::new(x, action_y), Vec2::new(106.0, 32.0)),
        tr("declare_war"),
        &detail.declare_war_action,
        ButtonVariant::Danger,
    ) {
        cmds.push(DiplomacyCommand::DeclareWar(detail.tag.clone()));
    }
    x += 114.0;
    if v9_action_button(
        ui,
        Rect::from_min_size(Pos2::new(x, action_y), Vec2::new(124.0, 32.0)),
        tr("invite_to_faction"),
        &detail.invite_to_faction_action,
        ButtonVariant::Secondary,
    ) {
        cmds.push(DiplomacyCommand::InviteToFaction(detail.tag.clone()));
    }
    x += 132.0;
    if v9_action_button(
        ui,
        Rect::from_min_size(Pos2::new(x, action_y), Vec2::new(116.0, 32.0)),
        tr("request_access"),
        &detail.request_access_action,
        ButtonVariant::Secondary,
    ) {
        cmds.push(DiplomacyCommand::RequestMilitaryAccess(detail.tag.clone()));
    }
}

fn v9_action_button(
    ui: &mut egui::Ui,
    rect: Rect,
    label: &str,
    action: &DiplomaticActionView,
    variant: crate::v9::primitives::ButtonVariant,
) -> bool {
    let response = crate::v9::primitives::Button::new(label)
        .size(crate::v9::primitives::ButtonSize::Md)
        .variant(variant)
        .enabled(action.enabled)
        .show_at(ui, rect);
    let response = response.on_hover_text(if action.enabled {
        action.preview.clone()
    } else {
        action
            .reason
            .clone()
            .unwrap_or_else(|| action.preview.clone())
    });
    response.clicked()
}

fn v9_diplomacy_badge(ui: &mut egui::Ui, rect: Rect, label: &str, color: Color32) {
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

fn v9_detail_metric(ui: &mut egui::Ui, rect: Rect, label: &str, value: &str, color: Color32) {
    crate::v9::primitives::Tile::new(label, value)
        .accent(color)
        .show_at(ui, rect);
}

fn v9_text_line(ui: &mut egui::Ui, x: f32, y: f32, label: &str, value: &str, color: Color32) {
    ui.painter().text(
        Pos2::new(x, y),
        egui::Align2::LEFT_TOP,
        label,
        crate::v9::TextRole::Caption.font_id(),
        crate::v9::palette::MUTED,
    );
    ui.painter().text(
        Pos2::new(x + 120.0, y),
        egui::Align2::LEFT_TOP,
        value,
        crate::v9::TextRole::Body.font_id(),
        color,
    );
}

fn render_summary(ui: &mut egui::Ui, data: &DiplomacyData) {
    let faction = data
        .player_faction
        .as_ref()
        .map(|f| f.name.clone())
        .unwrap_or_else(|| "无阵营".to_owned());
    let ready_wars = data
        .countries
        .iter()
        .filter_map(|c| c.detail.as_ref())
        .filter(|detail| detail.declare_war_action.enabled)
        .count();
    ui.add_space(6.0);
    components::summary_strip(
        ui,
        &[
            (tr("world_tension"), format!("{:.0}%", data.world_tension)),
            ("当前战争", data.active_wars.len().to_string()),
            (tr("faction"), faction),
            ("请求", data.requests.len().to_string()),
            ("可宣战目标", ready_wars.to_string()),
        ],
    );
    ui.add_space(6.0);
}

fn render_status_banner(ui: &mut egui::Ui, data: &DiplomacyData) {
    let ready_wars = data
        .countries
        .iter()
        .filter_map(|c| c.detail.as_ref())
        .filter(|detail| detail.declare_war_action.enabled)
        .count();
    let pending_requests = data
        .requests
        .iter()
        .filter(|r| r.status == "Pending")
        .count();
    let (label, text, color) = if !data.active_wars.is_empty() {
        (
            "战争进行中",
            format!(
                "当前有 {} 场战争，和平会议集中在战争卡。",
                data.active_wars.len()
            ),
            BAD,
        )
    } else if pending_requests > 0 {
        (
            "请求待处理",
            format!("当前有 {} 个外交请求等待处理或结果。", pending_requests),
            WARN,
        )
    } else if ready_wars > 0 {
        (
            "战争目标可用",
            format!("已有 {} 个目标可宣战。", ready_wars),
            WARN,
        )
    } else if data.player_faction.is_none() {
        (
            "外交孤立",
            "我国尚未加入阵营，可创建阵营或改善关系后邀请盟友。".to_owned(),
            WARN,
        )
    } else {
        (
            "外交稳定",
            "当前没有直接战争风险，继续维护阵营和通行权。".to_owned(),
            GOOD,
        )
    };

    components::status_banner(ui, color, label, &text);
    ui.add_space(4.0);
}

fn render_home_card(ui: &mut egui::Ui, data: &DiplomacyData, cmds: &mut Vec<DiplomacyCommand>) {
    diplomacy_card(ui, "我国外交", |ui| {
        if let Some(f) = &data.player_faction {
            ui.label(RichText::new(&f.name).strong().color(GOLD));
            ui.label(tr("leader_label").replace("{}", &tr(&f.leader_tag)));
            let members: Vec<String> = f.member_tags.iter().map(|t| tr(t).to_string()).collect();
            ui.label(tr("faction_members_list").replace("{}", &members.join(", ")));
            if ui.small_button(tr("leave_faction")).clicked() {
                cmds.push(DiplomacyCommand::LeaveFaction);
            }
        } else {
            ui.label(RichText::new(tr("not_in_faction")).color(components::MUTED));
            if ui.small_button(tr("create_faction")).clicked() {
                cmds.push(DiplomacyCommand::CreateFaction);
            }
        }
    });
}

fn render_requests(ui: &mut egui::Ui, data: &DiplomacyData) {
    diplomacy_card(ui, "待处理外交请求", |ui| {
        if data.requests.is_empty() {
            ui.label(RichText::new("当前没有外交请求。").color(components::MUTED));
            return;
        }
        for request in &data.requests {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(&request.kind).strong().color(GOLD));
                ui.label(format!(
                    "{} -> {}",
                    tr(&request.from_tag),
                    tr(&request.to_tag)
                ));
                ui.label(
                    RichText::new(&request.status)
                        .small()
                        .color(components::MUTED),
                );
            });
        }
    });
}

fn render_country_list(
    ui: &mut egui::Ui,
    data: &DiplomacyData,
    sort_by_opinion: &mut bool,
    selected_tag: &mut Option<String>,
    icon_bank: &mut crate::icons::IconBank,
) {
    diplomacy_card(ui, "国家列表", |ui| {
        ui.horizontal(|ui| {
            ui.label(format!("{}:", tr("sort_alpha")));
            if ui
                .selectable_label(!*sort_by_opinion, tr("alphabetical"))
                .clicked()
            {
                *sort_by_opinion = false;
            }
            if ui
                .selectable_label(*sort_by_opinion, tr("by_opinion"))
                .clicked()
            {
                *sort_by_opinion = true;
            }
        });
        ui.add_space(4.0);

        egui::ScrollArea::vertical()
            .max_height(260.0)
            .show(ui, |ui| {
                let mut sorted: Vec<&CountryEntry> = data.countries.iter().collect();
                if *sort_by_opinion {
                    sorted.sort_by(|a, b| b.opinion.cmp(&a.opinion));
                } else {
                    sorted.sort_by(|a, b| a.tag.cmp(&b.tag));
                }
                for c in sorted {
                    let selected = selected_tag.as_deref() == Some(c.tag.as_str());
                    let response = ui
                        .horizontal(|ui| {
                            render_country_portrait(ui, c, icon_bank);
                            ui.vertical(|ui| {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(tr(&c.tag)).color(country_color(c)).strong(),
                                    );
                                    ui.label(format!("({:+})", c.opinion));
                                });
                                if !c.leader_name.is_empty() {
                                    ui.label(
                                        RichText::new(&c.leader_name)
                                            .small()
                                            .color(Color32::from_gray(200)),
                                    );
                                }
                                if let Some(summary) = &c.autonomy_summary {
                                    ui.label(RichText::new(summary).small().color(WARN));
                                }
                            });
                        })
                        .response;
                    if response.clicked() {
                        *selected_tag = Some(c.tag.clone());
                    }
                    if selected {
                        response.highlight();
                    }
                    ui.separator();
                }
            });
    });
}

fn render_country_portrait(
    ui: &mut egui::Ui,
    c: &CountryEntry,
    icon_bank: &mut crate::icons::IconBank,
) {
    let size = egui::Vec2::new(48.0, 48.0);
    if let Some(gfx) = c.leader_portrait_key.as_deref() {
        if let Some(handle) = icon_bank.get_or_load(gfx) {
            ui.add(
                egui::Image::from_texture(handle)
                    .fit_to_exact_size(size)
                    .corner_radius(2.0),
            );
            return;
        }
    }
    if let Some(handle) = icon_bank.get_or_load(&c.flag_gfx) {
        ui.add(
            egui::Image::from_texture(handle)
                .fit_to_exact_size(size)
                .corner_radius(2.0),
        );
        return;
    }
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    ui.painter().rect_filled(rect, 2.0, Color32::from_gray(40));
    ui.painter().rect_stroke(
        rect,
        2.0,
        egui::Stroke::new(1.0, GOLD),
        egui::epaint::StrokeKind::Outside,
    );
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        &c.tag,
        egui::FontId::proportional(11.0),
        GOLD,
    );
}

fn selected_detail<'a>(
    data: &'a DiplomacyData,
    selected_tag: &mut Option<String>,
) -> Option<&'a CountryDiplomacyDetail> {
    if selected_tag.is_none() {
        *selected_tag = data.countries.first().map(|c| c.tag.clone());
    }
    data.countries
        .iter()
        .find(|c| selected_tag.as_deref() == Some(c.tag.as_str()))
        .and_then(|c| c.detail.as_ref())
}

pub fn render_country_diplomacy_detail(
    ui: &mut egui::Ui,
    detail: &CountryDiplomacyDetail,
    cmds: &mut Vec<DiplomacyCommand>,
) {
    let colonial_or_subject_population = detail.colonial_population + detail.subject_population;
    metric_tile_grid(
        ui,
        &[
            (
                tr("opinion").to_owned(),
                format!("{:+}", detail.opinion),
                relation_color(detail),
            ),
            (
                tr("faction").to_owned(),
                detail
                    .faction_name
                    .clone()
                    .unwrap_or_else(|| "无".to_owned()),
                if detail.same_faction { GOOD } else { GOLD },
            ),
            (
                "战争".to_owned(),
                if detail.at_war { "交战" } else { "和平" }.to_owned(),
                if detail.at_war { BAD } else { GOOD },
            ),
        ],
    );
    ui.add_space(6.0);
    ui.add(egui::Label::new(RichText::new(&detail.display_name).strong().color(GOLD)).wrap());
    if let Some(overlord) = &detail.overlord_name {
        let level = detail.autonomy_level_name.as_deref().unwrap_or("自治附庸");
        ui.label(RichText::new(format!("宗主国: {} ({})", overlord, level)).color(WARN));
    }
    if !detail.subject_names.is_empty() {
        ui.label(
            RichText::new(format!("傀儡/自治领: {}", detail.subject_names.join("、")))
                .color(Color32::from_rgb(0xc0, 0xe0, 0xff)),
        );
    }
    metric_tile_grid(
        ui,
        &[
            (
                "本土".to_owned(),
                format_population(detail.domestic_population),
                GOOD,
            ),
            (
                "殖民/属国".to_owned(),
                format_population(colonial_or_subject_population),
                WARN,
            ),
            (
                "管辖".to_owned(),
                format_population(detail.governed_population),
                GOLD,
            ),
            (
                "其中属国".to_owned(),
                format_population(detail.subject_population),
                WARN,
            ),
            (
                "帝国".to_owned(),
                format_population(detail.imperial_population),
                GOLD,
            ),
        ],
    );
    if detail.justifying_wargoal {
        ui.add(
            egui::ProgressBar::new(detail.justify_progress.clamp(0.0, 1.0))
                .desired_width(ui.available_width().min(280.0))
                .text(format!(
                    "{} ({}d)",
                    tr("justifying_wargoal"),
                    detail.justify_days_remaining
                )),
        );
    } else if detail.has_wargoal {
        ui.label(RichText::new(tr("wargoal_ready")).color(GOOD).strong());
    }
    if !detail.wargoals.is_empty() {
        ui.add_space(4.0);
        ui.label(RichText::new("战争目标详情").strong().color(GOLD));
        for goal in &detail.wargoals {
            let state = goal
                .target_state
                .map(|s| format!(" 州 {}", s))
                .unwrap_or_default();
            ui.horizontal_wrapped(|ui| {
                ui.label(format!("{}{}", goal.kind, state));
                ui.label(
                    RichText::new(&goal.status)
                        .small()
                        .color(if goal.progress >= 1.0 { GOOD } else { WARN }),
                );
                if goal.progress < 1.0 {
                    ui.label(
                        RichText::new(format!("剩余 {} 天", goal.days_remaining))
                            .small()
                            .color(components::MUTED),
                    );
                }
                ui.label(
                    RichText::new(format!("来源: {}", goal.source))
                        .small()
                        .color(components::MUTED),
                );
            });
        }
    }
    if !detail.relation_factors.is_empty() {
        ui.add_space(4.0);
        ui.label(RichText::new("关系构成").strong().color(GOLD));
        for factor in &detail.relation_factors {
            ui.horizontal_wrapped(|ui| {
                ui.label(&factor.label);
                ui.label(
                    RichText::new(&factor.value)
                        .small()
                        .color(if factor.positive { GOOD } else { WARN }),
                );
            });
        }
    }
    ui.separator();
    ui.horizontal_wrapped(|ui| {
        if !detail.has_wargoal && !detail.justifying_wargoal {
            let btn = ui.add_enabled(
                detail.justify_action.enabled,
                egui::Button::new(format!("📋 {}", tr("justify_wargoal"))),
            );
            if add_action_hover(btn, &detail.justify_action).clicked() {
                cmds.push(DiplomacyCommand::JustifyWargoal(detail.tag.clone()));
            }
        }
        let btn = ui.add_enabled(
            detail.declare_war_action.enabled,
            egui::Button::new(format!("⚔ {}", tr("declare_war"))),
        );
        if add_action_hover(btn, &detail.declare_war_action).clicked() {
            cmds.push(DiplomacyCommand::DeclareWar(detail.tag.clone()));
        }
        let btn = ui.add_enabled(
            detail.invite_to_faction_action.enabled,
            egui::Button::new(format!("🤝 {}", tr("invite_to_faction"))),
        );
        if add_action_hover(btn, &detail.invite_to_faction_action).clicked() {
            cmds.push(DiplomacyCommand::InviteToFaction(detail.tag.clone()));
        }
        let btn = ui.add_enabled(
            detail.request_access_action.enabled,
            egui::Button::new(format!("🛂 {}", tr("request_access"))),
        );
        if add_action_hover(btn, &detail.request_access_action).clicked() {
            cmds.push(DiplomacyCommand::RequestMilitaryAccess(detail.tag.clone()));
        }
    });
    render_action_reason(ui, tr("justify_wargoal"), &detail.justify_action);
    render_action_reason(ui, tr("declare_war"), &detail.declare_war_action);
    render_action_reason(
        ui,
        tr("invite_to_faction"),
        &detail.invite_to_faction_action,
    );
    render_action_reason(ui, tr("request_access"), &detail.request_access_action);
}

fn render_action_reason(ui: &mut egui::Ui, label: &str, action: &DiplomaticActionView) {
    if action.enabled {
        return;
    }
    let Some(reason) = &action.reason else {
        return;
    };
    ui.add(
        egui::Label::new(
            RichText::new(format!("{}：{}", label, reason))
                .small()
                .color(components::MUTED),
        )
        .wrap(),
    );
}

fn render_factions(ui: &mut egui::Ui, data: &DiplomacyData) {
    diplomacy_card(
        ui,
        &tr("all_factions").replace("{}", &data.all_factions.len().to_string()),
        |ui| {
            if data.all_factions.is_empty() {
                ui.label(RichText::new(tr("no_factions_formed")).color(components::MUTED));
            } else {
                for f in &data.all_factions {
                    ui.label(
                        RichText::new(format!("{} ({})", f.name, f.member_tags.len()))
                            .strong()
                            .color(GOLD),
                    );
                    ui.label(tr("leader_label").replace("{}", &tr(&f.leader_tag)));
                    let members: Vec<String> =
                        f.member_tags.iter().map(|t| tr(t).to_string()).collect();
                    ui.label(
                        RichText::new(tr("members_label").replace("{}", &members.join(" · ")))
                            .small(),
                    );
                    ui.add_space(4.0);
                }
            }
        },
    );
}

fn render_wars(ui: &mut egui::Ui, data: &DiplomacyData, cmds: &mut Vec<DiplomacyCommand>) {
    diplomacy_card(
        ui,
        &format!("战争与和平 ({})", data.active_wars.len()),
        |ui| {
            if data.active_wars.is_empty() {
                ui.label(
                    RichText::new("当前没有活跃战争。和平会议按钮会在参战后显示。")
                        .color(components::MUTED),
                );
            } else {
                for war in &data.active_wars {
                    ui.group(|ui| {
                        ui.label(
                            RichText::new(format!(
                                "War #{}: {} vs {}",
                                war.id,
                                tr(&war.primary_attacker_tag),
                                tr(&war.primary_defender_tag)
                            ))
                            .strong()
                            .color(GOLD),
                        );
                        ui.add(
                            egui::Label::new(format!(
                                "进攻方: {}",
                                war.attacker_tags
                                    .iter()
                                    .map(|tag| tr(tag).to_string())
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            ))
                            .wrap(),
                        );
                        ui.add(
                            egui::Label::new(format!(
                                "防守方: {}",
                                war.defender_tags
                                    .iter()
                                    .map(|tag| tr(tag).to_string())
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            ))
                            .wrap(),
                        );
                        ui.add(
                            egui::Label::new(format!(
                                "战争分数: 进攻方 {:.1} / 防守方 {:.1}",
                                war.attacker_score, war.defender_score
                            ))
                            .wrap(),
                        );
                        if !war.wargoals.is_empty() {
                            ui.label(RichText::new("和平条款预览:").strong());
                            for goal in &war.wargoals {
                                let state = goal
                                    .target_state
                                    .map(|s| format!(" state {}", s))
                                    .unwrap_or_default();
                                ui.add(
                                    egui::Label::new(format!(
                                        "- {} 将执行 {} 于 {}{}",
                                        tr(&goal.claimant_tag),
                                        goal.kind,
                                        tr(&goal.target_tag),
                                        state
                                    ))
                                    .wrap(),
                                );
                            }
                        }
                        ui.horizontal(|ui| {
                            let btn = ui.add_enabled(
                                war.attacker_peace_action.enabled,
                                egui::Button::new("执行进攻方和平"),
                            );
                            if add_action_hover(btn, &war.attacker_peace_action).clicked() {
                                cmds.push(DiplomacyCommand::ResolvePeace {
                                    war_id: war.id,
                                    winning_side: PeaceSide::Attacker,
                                });
                            }
                            let btn = ui.add_enabled(
                                war.defender_peace_action.enabled,
                                egui::Button::new("执行防守方和平"),
                            );
                            if add_action_hover(btn, &war.defender_peace_action).clicked() {
                                cmds.push(DiplomacyCommand::ResolvePeace {
                                    war_id: war.id,
                                    winning_side: PeaceSide::Defender,
                                });
                            }
                        });
                    });
                }
            }
        },
    );
}

fn diplomacy_card(ui: &mut egui::Ui, title: &str, add_contents: impl FnOnce(&mut egui::Ui)) {
    components::section_card(ui, title, add_contents);
}

fn metric_tile(ui: &mut egui::Ui, label: &str, value: String, color: Color32) {
    let h_margin = 8.0 + 8.0;
    let total = ui.available_width().max(40.0);
    let inner_width = (total - h_margin).max(20.0);
    let inner = egui::Frame::new()
        .fill(components::PANEL_CARD_DEEP)
        .stroke(egui::Stroke::new(1.0, components::STROKE_TILE))
        .inner_margin(egui::Margin {
            left: 8,
            right: 8,
            top: 4,
            bottom: 4,
        })
        .show(ui, |ui| {
            ui.set_min_width(inner_width);
            ui.set_max_width(inner_width);
            ui.add(
                egui::Label::new(RichText::new(label).size(10.5).color(components::MUTED)).wrap(),
            );
            ui.add_space(1.0);
            ui.add(egui::Label::new(RichText::new(value).strong().size(13.5).color(color)).wrap());
        });
    let rect = inner.response.rect;
    let bar = egui::Rect::from_min_size(
        rect.left_top() + egui::vec2(1.0, 1.0),
        egui::vec2(3.0, rect.height() - 2.0),
    );
    ui.painter().rect_filled(bar, 0.0, color);
}

fn metric_tile_grid(ui: &mut egui::Ui, items: &[(String, String, Color32)]) {
    if items.is_empty() {
        return;
    }
    let min_tile_width = 110.0;
    let available = ui.available_width();
    let fit = ((available / min_tile_width).floor() as usize).max(1);
    let per_row = fit.min(items.len()).max(1);
    for chunk in items.chunks(per_row) {
        ui.columns(chunk.len(), |cols| {
            for (i, (label, value, color)) in chunk.iter().enumerate() {
                metric_tile(&mut cols[i], label, value.clone(), *color);
            }
        });
        ui.add_space(4.0);
    }
}

fn format_population(value: u64) -> String {
    if value >= 1_000_000_000 {
        format!("{:.2}B", value as f64 / 1_000_000_000.0)
    } else if value >= 1_000_000 {
        format!("{:.1}M", value as f64 / 1_000_000.0)
    } else if value >= 1_000 {
        format!("{:.1}K", value as f64 / 1_000.0)
    } else {
        value.to_string()
    }
}

fn country_color(c: &CountryEntry) -> Color32 {
    if c.at_war {
        BAD
    } else if c.same_faction {
        GOOD
    } else if c.opinion > 0 {
        Color32::LIGHT_GREEN
    } else if c.opinion < -50 {
        Color32::from_rgb(0xff, 0x80, 0x80)
    } else {
        Color32::GRAY
    }
}

fn relation_color(detail: &CountryDiplomacyDetail) -> Color32 {
    if detail.at_war {
        BAD
    } else if detail.same_faction {
        GOOD
    } else if detail.opinion > 50 {
        Color32::LIGHT_GREEN
    } else if detail.opinion < -50 {
        Color32::from_rgb(0xff, 0x80, 0x80)
    } else {
        Color32::GRAY
    }
}

pub fn add_action_hover(response: egui::Response, action: &DiplomaticActionView) -> egui::Response {
    if let Some(reason) = &action.reason {
        response.on_hover_text(format!("{}\n{}", action.preview, reason))
    } else if !action.preview.is_empty() {
        response.on_hover_text(&action.preview)
    } else {
        response
    }
}
