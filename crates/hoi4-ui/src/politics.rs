//! V5 阶段 C.2 + F.2：政治面板。
//!
//! - C.2：党派色块 + 民众支持率条 + ideas 列表 + 5 顾问槽位（空）
//! - F.2：决议列表 — 5 分类 tab，按可见性筛选，按按钮显示状态：
//!     - 可点（绿色）/ 灰显（条件不满足）/ 冷却中 / 进行中 / 已触发
//!     - 点击后返回命令，由调用方执行。
//!       `DecisionState::activate(...)` 真正执行。

#![allow(dead_code, deprecated)]

use crate::{components, i18n::tr};
use egui::{Color32, Pos2, Rect, RichText, Sense, Vec2};

use hoi4_content::{Decision, DecisionCategory, DecisionMechanicKind, Effect};

const PANEL_CARD: Color32 = Color32::from_rgb(0x24, 0x1a, 0x12);
const PANEL_CARD_SOFT: Color32 = Color32::from_rgb(0x31, 0x24, 0x18);
const STROKE_DARK: Color32 = Color32::from_rgb(0x5a, 0x44, 0x2c);
const GOOD: Color32 = Color32::from_rgb(0x70, 0xc8, 0x78);
const WARN: Color32 = Color32::from_rgb(0xff, 0xc0, 0x60);
const BAD: Color32 = Color32::from_rgb(0xe0, 0x60, 0x58);
const IDEA_SLOT_SIZE: f32 = 54.0;
const IDEA_ICON_SIZE: f32 = 44.0;

fn ideology_color(key: &str) -> Color32 {
    match key {
        // HOI4-style ideology palette: democratic blue, communist red,
        // fascist brown, non-aligned grey.
        "democratic" => Color32::from_rgb(60, 90, 170),
        "communism" => Color32::from_rgb(180, 30, 30),
        "fascism" => Color32::from_rgb(80, 60, 40),
        "neutrality" => Color32::from_rgb(140, 140, 140),
        _ => Color32::from_rgb(100, 100, 100),
    }
}

fn ideology_label(key: &str) -> &'static str {
    match key {
        "fascism" => tr("fascism"),
        "democratic" => tr("democratic"),
        "communism" => tr("communism"),
        "neutrality" => tr("neutrality"),
        _ => tr("unknown"),
    }
}

/// F.2：单个决议在面板上的运行时状态（caller 在快照里填好）。
#[derive(Debug, Clone)]
pub struct DecisionEntry {
    pub id: String,
    pub name: String,
    pub description: String,
    pub effect_preview: String,
    pub category: DecisionCategory,
    pub mechanic_kind: DecisionMechanicKind,
    pub cost_political_power: f32,
    /// `true` = 可见条件满足，决议出现在面板里。
    pub visible: bool,
    /// `true` = 可用条件满足且政治力量足够 + 不在冷却 + 不进行中 + 不一次性触发过。
    pub clickable: bool,
    /// `Some(remaining_days)` = 进行中（mission 决议）。
    pub mission_remaining: Option<u32>,
    /// `Some(total_days)` = 进行中的总长度（用于 progress bar）。
    pub mission_total: Option<u32>,
    /// `Some(remaining_days)` = 在冷却。
    pub cooldown_remaining: Option<u32>,
    /// `true` = fire_only_once 已触发过，永久禁用。
    pub already_fired: bool,
}

impl DecisionEntry {
    pub fn from_def(d: &Decision) -> Self {
        Self {
            id: d.id.clone(),
            name: d.name.clone(),
            description: d.description.clone(),
            effect_preview: decision_effect_preview(d),
            category: d.category,
            mechanic_kind: d.mechanic_kind,
            cost_political_power: d.cost_political_power,
            visible: true,
            clickable: true,
            mission_remaining: None,
            mission_total: None,
            cooldown_remaining: None,
            already_fired: false,
        }
    }
}

fn decision_effect_preview(d: &Decision) -> String {
    let mut summaries = d
        .on_complete
        .iter()
        .chain(d.on_activation.iter())
        .filter_map(decision_effect_summary)
        .collect::<Vec<_>>();
    if summaries.len() > 5 {
        summaries.truncate(5);
        summaries.push("...".to_owned());
    }
    summaries.join("  ·  ")
}

fn decision_effect_summary(effect: &Effect) -> Option<String> {
    match effect {
        Effect::ClampVariable { .. } | Effect::TriggerEvent(_) => None,
        _ => effect.effect_summary(),
    }
}

/// 政治面板每帧所需数据。
pub struct PoliticsData {
    pub ruling_party: String,
    pub party_popularity: Vec<(String, f32)>, // (ideology_key, 0..1)
    pub ideas: Vec<IdeaEntry>,
    /// 当前玩家 PP（用于面板顶部 hint 显示）。
    pub political_power: f32,
    pub stability: f32,
    pub war_support: f32,
    pub focus_available: bool,
    pub current_focus_name: Option<String>,
    pub current_focus_progress: f32,
    pub current_focus_cost_days: Option<u32>,

    // ─── J.1：国家元首 + 党派全称（顶部摘要区） ──────────────────
    /// 国家 tag（"GER" 等）。仅用于 IconBank 兜底。
    pub country_tag: String,
    /// 元首显示名（已按 `data.character_names` 解过 loc，例 "Adolf Hitler"）；
    /// 若元首不存在则为空串。
    pub leader_name: String,
    /// 元首肖像 GFX 名（例 `GFX_portrait_GER_adolf_hitler`）；`None` 时
    /// 由 PoliticsPanel 用国旗占位图兜底。
    pub leader_portrait_key: Option<String>,
    /// 党派完整名称（已解过 `<TAG>_<ideology>_party_long` loc，例 "Nationalsozialistische Deutsche Arbeiterpartei"）。
    /// 若本地化缺失则回退为 `ideology_label(ruling_party)`。
    pub party_full_name: String,
}

impl PoliticsData {
    /// 测试 / 无数据时的空构造（只填 4 项 C.2 字段，decisions 留空）。
    pub fn legacy(
        ruling_party: String,
        party_popularity: Vec<(String, f32)>,
        ideas: Vec<IdeaEntry>,
    ) -> Self {
        Self {
            ruling_party,
            party_popularity,
            ideas,
            political_power: 0.0,
            stability: 0.5,
            war_support: 0.0,
            focus_available: false,
            current_focus_name: None,
            current_focus_progress: 0.0,
            current_focus_cost_days: None,
            country_tag: String::new(),
            leader_name: String::new(),
            leader_portrait_key: None,
            party_full_name: String::new(),
        }
    }
}

/// 政治面板中的一条国家精神。
#[derive(Debug, Clone)]
pub struct IdeaEntry {
    pub key: String,
    pub name: String,
    pub category: String,
    pub picture: Option<String>,
    pub modifiers: Vec<(String, f32)>,
}

/// F.2：面板回写命令。
#[derive(Debug, Clone, PartialEq)]
pub enum DecisionCommand {
    /// 玩家点了某个决议的执行按钮。
    Activate(String),
    OpenFocusTree,
}

/// 政治面板（无状态）。
///
/// 返回 `(close, commands)`：
/// - `close = true` 用户点关闭按钮
/// - `commands` 用户本帧点击的所有决议命令
pub struct PoliticsPanel;

impl PoliticsPanel {
    pub fn show(
        ctx: &egui::Context,
        data: &PoliticsData,
        icon_bank: &mut crate::icons::IconBank,
    ) -> (bool, Vec<DecisionCommand>) {
        v9_show_politics(ctx, data, icon_bank)
    }
}

fn v9_show_politics(
    ctx: &egui::Context,
    data: &PoliticsData,
    icon_bank: &mut crate::icons::IconBank,
) -> (bool, Vec<DecisionCommand>) {
    use crate::v9::composites::panel_shell::{
        draw_summary_tiles, draw_tab_strip, PanelClass, PanelShell,
    };
    use crate::v9::tokens::palette;

    let accent = v9_ideology_color(&data.ruling_party);
    let ruling_support = ruling_party_support(data);
    let subtitle = if data.country_tag.is_empty() {
        tr("country")
    } else {
        data.country_tag.as_str()
    };
    let (close, output) = PanelShell::new("politics_panel_v9", tr("politics"))
        .subtitle(subtitle)
        .class(PanelClass::MilitaryDiplomacy)
        .accent(accent)
        .footer("Q Close  |  National politics")
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
                    (
                        tr("stability"),
                        format!("{:.0}%", data.stability * 100.0),
                        v9_percent_color(data.stability),
                    ),
                    (
                        tr("war_support"),
                        format!("{:.0}%", data.war_support * 100.0),
                        v9_percent_color(data.war_support),
                    ),
                    (
                        tr("ruling_party"),
                        format!("{:.0}%", ruling_support * 100.0),
                        accent,
                    ),
                ],
            );
            draw_tab_strip(ui, layout.tabs, "政权 / 意识形态 / 国家精神 / 顾问", accent);
            let mut cmds = Vec::new();
            v9_politics_body(ui, layout.body, data, icon_bank, &mut cmds);
            cmds
        });
    (close, output.unwrap_or_default())
}

fn v9_politics_body(
    ui: &mut egui::Ui,
    rect: Rect,
    data: &PoliticsData,
    icon_bank: &mut crate::icons::IconBank,
    cmds: &mut Vec<DecisionCommand>,
) {
    ui.allocate_ui_at_rect(rect, |ui| {
        ui.set_min_size(rect.size());
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                v9_leader_card(ui, data, icon_bank, cmds);
                v9_ideology_card(ui, data);
                v9_ideas_card(ui, data, icon_bank);
                v9_advisors_card(ui);
            });
    });
}

fn v9_card(ui: &mut egui::Ui, height: f32, add_contents: impl FnOnce(&mut egui::Ui, Rect)) {
    let width = ui.available_width().max(360.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    let inner = crate::v9::primitives::Card::new()
        .as_panel()
        .show_at(ui, rect);
    add_contents(ui, inner);
    ui.add_space(crate::v9::spacing::S4);
}

fn v9_leader_card(
    ui: &mut egui::Ui,
    data: &PoliticsData,
    icon_bank: &mut crate::icons::IconBank,
    cmds: &mut Vec<DecisionCommand>,
) {
    use crate::v9::{
        layout::{GridLayout, Track},
        primitives::{draw_progress_bar, Button, ButtonSize, ButtonVariant, PortraitFrame},
        tokens::{palette, spacing, TextRole},
    };
    v9_card(ui, 170.0, |ui, inner| {
        let grid = GridLayout::new(
            vec![Track::Fr(1.0)],
            vec![Track::Fixed(118.0), Track::Fr(1.0)],
        )
        .with_gutter(spacing::S5, 0.0);
        let cells = grid.measure(inner.shrink(2.0));
        let portrait_rect = GridLayout::cell(&cells, 0, 0).shrink2(Vec2::new(2.0, 10.0));
        let portrait = data
            .leader_portrait_key
            .as_deref()
            .and_then(|gfx| icon_bank.get_or_load(gfx));
        PortraitFrame::new(if data.country_tag.is_empty() {
            "?"
        } else {
            &data.country_tag
        })
        .accent(v9_ideology_color(&data.ruling_party))
        .show_at(ui, portrait_rect, portrait);

        let detail = GridLayout::cell(&cells, 0, 1).shrink(2.0);
        let leader = if data.leader_name.is_empty() {
            tr("leader_unknown").to_owned()
        } else {
            data.leader_name.clone()
        };
        let party = if data.party_full_name.is_empty() {
            ideology_label(&data.ruling_party).to_owned()
        } else {
            data.party_full_name.clone()
        };
        ui.painter().text(
            Pos2::new(detail.left(), detail.top() + 4.0),
            egui::Align2::LEFT_TOP,
            leader,
            TextRole::Display.font_id(),
            palette::GOLD_HOT,
        );
        ui.painter().text(
            Pos2::new(detail.left(), detail.top() + 32.0),
            egui::Align2::LEFT_TOP,
            party,
            TextRole::Subheading.font_id(),
            v9_ideology_color(&data.ruling_party),
        );
        v9_badge(
            ui,
            Rect::from_min_size(
                Pos2::new(detail.left(), detail.top() + 58.0),
                Vec2::new(152.0, 24.0),
            ),
            ideology_label(&data.ruling_party),
            v9_ideology_color(&data.ruling_party),
        );
        v9_badge(
            ui,
            Rect::from_min_size(
                Pos2::new(detail.left() + 162.0, detail.top() + 58.0),
                Vec2::new(92.0, 24.0),
            ),
            if data.country_tag.is_empty() {
                "TAG"
            } else {
                &data.country_tag
            },
            palette::BRASS_BRIGHT,
        );

        let focus_label = data.current_focus_name.as_deref().unwrap_or("未选择国策");
        ui.painter().text(
            Pos2::new(detail.left(), detail.top() + 96.0),
            egui::Align2::LEFT_TOP,
            focus_label,
            TextRole::Body.font_id(),
            palette::PARCHMENT,
        );
        let progress = data
            .current_focus_cost_days
            .filter(|days| *days > 0)
            .map(|days| (data.current_focus_progress / days as f32).clamp(0.0, 1.0))
            .unwrap_or(0.0);
        let bar = Rect::from_min_size(
            Pos2::new(detail.left(), detail.top() + 118.0),
            Vec2::new((detail.width() - 138.0).max(120.0), 10.0),
        );
        draw_progress_bar(ui, bar, progress, palette::GOLD);
        let btn_rect = Rect::from_min_size(
            Pos2::new(detail.right() - 124.0, detail.top() + 106.0),
            Vec2::new(120.0, 30.0),
        );
        if Button::new("国策")
            .size(ButtonSize::Md)
            .variant(ButtonVariant::Secondary)
            .enabled(data.focus_available)
            .show_at(ui, btn_rect)
            .clicked()
        {
            cmds.push(DecisionCommand::OpenFocusTree);
        }
    });
}

fn v9_ideology_card(ui: &mut egui::Ui, data: &PoliticsData) {
    use crate::v9::{
        primitives::draw_progress_bar,
        tokens::{palette, spacing, TextRole},
    };
    let height = 58.0 + data.party_popularity.len().max(1) as f32 * 34.0;
    v9_card(ui, height, |ui, inner| {
        ui.painter().text(
            Pos2::new(inner.left(), inner.top()),
            egui::Align2::LEFT_TOP,
            tr("party_popularity"),
            TextRole::Heading.font_id(),
            palette::BRASS_BRIGHT,
        );
        let mut y = inner.top() + 34.0;
        for (key, pop) in &data.party_popularity {
            let color = v9_ideology_color(key);
            ui.painter().text(
                Pos2::new(inner.left(), y),
                egui::Align2::LEFT_TOP,
                ideology_label(key),
                TextRole::Body.font_id(),
                color,
            );
            ui.painter().text(
                Pos2::new(inner.right(), y),
                egui::Align2::RIGHT_TOP,
                format!("{:.0}%", pop * 100.0),
                TextRole::Numeric.font_id(),
                palette::PARCHMENT,
            );
            let bar = Rect::from_min_size(
                Pos2::new(inner.left(), y + 18.0),
                Vec2::new(inner.width(), 8.0),
            );
            draw_progress_bar(ui, bar, *pop, color);
            y += spacing::S8 + 2.0;
        }
    });
}

fn v9_ideas_card(ui: &mut egui::Ui, data: &PoliticsData, icon_bank: &mut crate::icons::IconBank) {
    use crate::v9::{
        primitives::PortraitFrame,
        tokens::{palette, spacing, TextRole},
    };
    let cols = 8usize;
    let rows = data.ideas.len().max(1).div_ceil(cols);
    let height = 54.0 + rows as f32 * 64.0;
    v9_card(ui, height, |ui, inner| {
        ui.painter().text(
            Pos2::new(inner.left(), inner.top()),
            egui::Align2::LEFT_TOP,
            tr("national_spirits"),
            TextRole::Heading.font_id(),
            palette::BRASS_BRIGHT,
        );
        if data.ideas.is_empty() {
            ui.painter().text(
                inner.center(),
                egui::Align2::CENTER_CENTER,
                tr("idea_none"),
                TextRole::Body.font_id(),
                palette::MUTED,
            );
            return;
        }
        let slot = Vec2::splat(54.0);
        let gap = spacing::S4;
        let start = Pos2::new(inner.left(), inner.top() + 34.0);
        for (idx, idea) in data.ideas.iter().enumerate() {
            let col = idx % cols;
            let row = idx / cols;
            let rect = Rect::from_min_size(
                Pos2::new(
                    start.x + col as f32 * (slot.x + gap),
                    start.y + row as f32 * (slot.y + gap),
                ),
                slot,
            );
            let texture = idea_icon_gfx(idea).and_then(|gfx| icon_bank.get_or_load(&gfx));
            PortraitFrame::new(&idea.name)
                .accent(palette::BRASS_BRIGHT)
                .show_at(ui, rect, texture);
            let response = ui.interact(rect, ui.id().with(("v9_idea", &idea.key)), Sense::hover());
            response.on_hover_ui(|ui| render_idea_tooltip(ui, idea));
        }
    });
}

fn v9_advisors_card(ui: &mut egui::Ui) {
    use crate::v9::{
        primitives::PortraitFrame,
        tokens::{palette, spacing, TextRole},
    };
    v9_card(ui, 120.0, |ui, inner| {
        ui.painter().text(
            Pos2::new(inner.left(), inner.top()),
            egui::Align2::LEFT_TOP,
            tr("advisors"),
            TextRole::Heading.font_id(),
            palette::BRASS_BRIGHT,
        );
        let slot = Vec2::new(72.0, 72.0);
        let start = Pos2::new(inner.left(), inner.top() + 34.0);
        for idx in 0..5 {
            let rect = Rect::from_min_size(
                Pos2::new(start.x + idx as f32 * (slot.x + spacing::S5), start.y),
                slot,
            );
            PortraitFrame::new("?")
                .accent(palette::MUTED)
                .show_at(ui, rect, None);
        }
    });
}

fn v9_badge(ui: &mut egui::Ui, rect: Rect, text: &str, color: Color32) {
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
        text,
        crate::v9::TextRole::Caption.font_id(),
        color,
    );
}

fn v9_ideology_color(key: &str) -> Color32 {
    match key {
        "fascism" => crate::v9::palette::IDEO_FASCISM,
        "democratic" => crate::v9::palette::IDEO_DEMOCRATIC,
        "communism" => crate::v9::palette::IDEO_COMMUNISM,
        "neutrality" => crate::v9::palette::IDEO_NEUTRALITY,
        _ => crate::v9::palette::BRASS_BRIGHT,
    }
}

fn v9_percent_color(value: f32) -> Color32 {
    if value >= 0.60 {
        crate::v9::palette::GOOD
    } else if value >= 0.35 {
        crate::v9::palette::WARN
    } else {
        crate::v9::palette::BAD
    }
}

fn render_politics_summary(ui: &mut egui::Ui, data: &PoliticsData) {
    let ruling_support = ruling_party_support(data);
    ui.add_space(6.0);
    components::summary_strip(
        ui,
        &[
            (
                tr("political_power"),
                format!("{:.0}", data.political_power),
            ),
            (tr("stability"), format!("{:.0}%", data.stability * 100.0)),
            (
                tr("war_support"),
                format!("{:.0}%", data.war_support * 100.0),
            ),
            ("执政支持", format!("{:.0}%", ruling_support * 100.0)),
        ],
    );
    ui.add_space(6.0);
}

fn render_politics_status_banner(ui: &mut egui::Ui, data: &PoliticsData) {
    let ruling_support = ruling_party_support(data);
    let (label, text, color) = if data.stability < 0.35 {
        (
            "政权不稳",
            "稳定度偏低，罢工、激进化和政治事件风险会上升。".to_owned(),
            BAD,
        )
    } else if data.war_support < 0.35 {
        (
            "战争支持不足",
            "战争支持偏低，动员和长期战争承压。".to_owned(),
            WARN,
        )
    } else if ruling_support < 0.40 {
        (
            "执政基础薄弱",
            "执政党支持率不足 40%，意识形态竞争正在削弱政权。".to_owned(),
            WARN,
        )
    } else {
        (
            "政局稳定",
            "当前政治局势可控，继续积累政治力量并观察局势变化。".to_owned(),
            GOOD,
        )
    };

    components::status_banner(ui, color, label, &text);
    ui.add_space(4.0);
}

fn render_leader_overview(
    ui: &mut egui::Ui,
    data: &PoliticsData,
    icon_bank: &mut crate::icons::IconBank,
    cmds: &mut Vec<DecisionCommand>,
) {
    politics_card(ui, "政权总览", |ui| {
        ui.horizontal(|ui| {
            render_leader_portrait(ui, data, icon_bank, Vec2::new(96.0, 96.0));
            ui.add_space(8.0);
            ui.vertical(|ui| {
                let leader_text = if data.leader_name.is_empty() {
                    tr("leader_unknown").to_string()
                } else {
                    data.leader_name.clone()
                };
                let party_text = if data.party_full_name.is_empty() {
                    ideology_label(&data.ruling_party).to_string()
                } else {
                    data.party_full_name.clone()
                };
                let ruling_color = ideology_color(&data.ruling_party);
                ui.label(
                    RichText::new(leader_text)
                        .heading()
                        .color(components::GOLD_BRIGHT),
                );
                ui.label(RichText::new(party_text).strong().color(ruling_color));
                ui.horizontal(|ui| {
                    ideology_chip(ui, &data.ruling_party, ruling_party_support(data));
                    if !data.country_tag.is_empty() {
                        ui.label(
                            RichText::new(&data.country_tag)
                                .small()
                                .color(components::MUTED),
                        );
                    }
                });
                render_current_focus_strip(ui, data, cmds);
                ui.add_space(6.0);
                ui.columns(3, |columns| {
                    metric_tile(
                        &mut columns[0],
                        tr("political_power"),
                        format!("{:.0}", data.political_power),
                        components::GOLD,
                    );
                    metric_tile(
                        &mut columns[1],
                        tr("stability"),
                        format!("{:.0}%", data.stability * 100.0),
                        percent_color(data.stability),
                    );
                    metric_tile(
                        &mut columns[2],
                        tr("war_support"),
                        format!("{:.0}%", data.war_support * 100.0),
                        percent_color(data.war_support),
                    );
                });
            });
        });
    });
}

fn render_current_focus_strip(
    ui: &mut egui::Ui,
    data: &PoliticsData,
    cmds: &mut Vec<DecisionCommand>,
) {
    if !data.focus_available {
        return;
    }
    ui.add_space(6.0);
    let progress = if let Some(cost_days) = data.current_focus_cost_days {
        if cost_days > 0 {
            (data.current_focus_progress / cost_days as f32).clamp(0.0, 1.0)
        } else {
            0.0
        }
    } else {
        0.0
    };
    let focus_label = data.current_focus_name.as_deref().unwrap_or("选择国策");
    let button_text = if data.current_focus_name.is_some() {
        format!("国策：{}  {:.0}%", focus_label, progress * 100.0)
    } else {
        "国策：未选择".to_owned()
    };

    egui::Frame::new()
        .fill(PANEL_CARD_SOFT)
        .stroke(egui::Stroke::new(1.0, components::GOLD))
        .inner_margin(egui::Margin::symmetric(8, 5))
        .show(ui, |ui| {
            ui.set_min_width(250.0);
            let response = ui.add(
                egui::Button::new(
                    RichText::new(button_text)
                        .small()
                        .color(components::GOLD_BRIGHT),
                )
                .min_size(egui::vec2(250.0, 24.0)),
            );
            if response.clicked() {
                cmds.push(DecisionCommand::OpenFocusTree);
            }
            if data.current_focus_name.is_some() {
                ui.add(
                    egui::ProgressBar::new(progress)
                        .fill(components::GOLD)
                        .desired_width(250.0),
                );
            } else {
                ui.label(
                    RichText::new("点击打开国策树")
                        .small()
                        .color(components::MUTED),
                );
            }
        });
}

fn render_ideology_section(ui: &mut egui::Ui, data: &PoliticsData) {
    politics_card(ui, tr("party_popularity"), |ui| {
        ui.horizontal(|ui| {
            render_ideology_pie(ui, &data.party_popularity, 138.0);
            ui.add_space(12.0);
            ui.vertical(|ui| {
                for (key, pop) in &data.party_popularity {
                    render_ideology_popularity_row(ui, key, *pop);
                    let bar = egui::ProgressBar::new((*pop).clamp(0.0, 1.0))
                        .fill(ideology_color(key))
                        .desired_width(ui.available_width());
                    ui.add(bar);
                    ui.add_space(3.0);
                }
            });
        });
    });
}

fn render_ideas_section(
    ui: &mut egui::Ui,
    data: &PoliticsData,
    icon_bank: &mut crate::icons::IconBank,
) {
    politics_card(ui, tr("national_spirits"), |ui| {
        if data.ideas.is_empty() {
            components::empty_state(ui, tr("idea_none"), "当前国家没有国家精神。 ");
        } else {
            let available_width = ui.available_width().max(IDEA_SLOT_SIZE);
            let spacing = 6.0;
            let columns = ((available_width + spacing) / (IDEA_SLOT_SIZE + spacing))
                .floor()
                .max(1.0) as usize;

            egui::Grid::new("politics_ideas_grid")
                .num_columns(columns)
                .spacing([spacing, spacing])
                .show(ui, |ui| {
                    for (index, idea) in data.ideas.iter().enumerate() {
                        render_idea_slot(ui, idea, icon_bank);
                        if index % columns == columns - 1 {
                            ui.end_row();
                        }
                    }
                });
        }
    });
}

fn render_idea_slot(ui: &mut egui::Ui, idea: &IdeaEntry, icon_bank: &mut crate::icons::IconBank) {
    let (rect, response) =
        ui.allocate_exact_size(Vec2::splat(IDEA_SLOT_SIZE), egui::Sense::hover());
    if ui.is_rect_visible(rect) {
        let border = if response.hovered() {
            components::GOLD_BRIGHT
        } else {
            STROKE_DARK
        };
        let painter = ui.painter();
        painter.rect_filled(rect, 3.0, Color32::from_rgb(0x16, 0x11, 0x0c));
        painter.rect_stroke(
            rect,
            3.0,
            egui::Stroke::new(1.2, border),
            egui::epaint::StrokeKind::Inside,
        );

        let icon_rect = egui::Rect::from_center_size(rect.center(), Vec2::splat(IDEA_ICON_SIZE));
        let mut icon_drawn = false;
        if let Some(gfx) = idea_icon_gfx(idea) {
            if let Some(handle) = icon_bank.get_or_load(&gfx) {
                ui.put(
                    icon_rect,
                    egui::Image::from_texture(handle)
                        .fit_to_exact_size(Vec2::splat(IDEA_ICON_SIZE)),
                );
                icon_drawn = true;
            }
        }
        if !icon_drawn {
            draw_idea_placeholder(ui, icon_rect, idea);
        }
    }

    response.on_hover_ui(|ui| render_idea_tooltip(ui, idea));
}

fn idea_icon_gfx(idea: &IdeaEntry) -> Option<String> {
    if let Some(picture) = idea.picture.as_deref().filter(|p| !p.is_empty()) {
        Some(if picture.starts_with("GFX_") {
            picture.to_owned()
        } else {
            format!("GFX_{picture}")
        })
    } else if !idea.key.is_empty() {
        Some(format!("GFX_idea_{}", idea.key))
    } else {
        None
    }
}

fn draw_idea_placeholder(ui: &mut egui::Ui, rect: egui::Rect, idea: &IdeaEntry) {
    let painter = ui.painter();
    painter.rect_filled(rect, 2.0, PANEL_CARD_SOFT);
    painter.rect_stroke(
        rect,
        2.0,
        egui::Stroke::new(1.0, Color32::from_rgb(0x72, 0x58, 0x34)),
        egui::epaint::StrokeKind::Inside,
    );
    let letter = idea
        .name
        .chars()
        .find(|c| !c.is_whitespace())
        .unwrap_or('?')
        .to_string();
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        letter,
        egui::FontId::proportional(20.0),
        components::GOLD_BRIGHT,
    );
}

fn render_idea_tooltip(ui: &mut egui::Ui, idea: &IdeaEntry) {
    ui.set_max_width(300.0);
    ui.label(
        RichText::new(&idea.name)
            .strong()
            .color(components::GOLD_BRIGHT),
    );
    let category_key = format!("idea_category_{}", idea.category);
    let category_label = tr(&category_key);
    if category_label != category_key {
        ui.label(
            RichText::new(category_label)
                .small()
                .color(components::MUTED),
        );
    }
    ui.separator();
    if idea.modifiers.is_empty() {
        ui.label(
            RichText::new("暂无数据加成。")
                .small()
                .color(components::MUTED),
        );
    } else {
        ui.label(
            RichText::new(idea_modifier_summary(&idea.modifiers))
                .small()
                .color(Color32::from_rgb(0xd8, 0xc0, 0x8a)),
        );
    }
}

fn idea_modifier_summary(modifiers: &[(String, f32)]) -> String {
    modifiers
        .iter()
        .map(|(key, value)| match key.as_str() {
            "political_power_gain" => format!("政治力量 +{value:.2}/日"),
            "political_power_factor" | "political_power_gain_factor" => {
                format!("政治力量 {:+.0}%", value * 100.0)
            }
            "national_focus_progress" => format!("国策速度 {:+.0}%", value * 100.0),
            "production_speed_buildings_factor" => format!("建造速度 {:+.0}%", value * 100.0),
            "production_factory_max_efficiency_factor" => {
                format!("工厂效率上限 {:+.0}%", value * 100.0)
            }
            "research_speed_factor" => format!("科研速度 {:+.0}%", value * 100.0),
            "consumer_goods_factor" => format!("消费品工厂 {:+.0}%", value * 100.0),
            "army_attack_factor" => format!("陆军攻击 {:+.0}%", value * 100.0),
            "army_defence_factor" => format!("陆军防御 {:+.0}%", value * 100.0),
            "army_organisation_factor" => format!("陆军组织度 {:+.0}%", value * 100.0),
            "army_org_regain" => format!("组织恢复 {:+.0}%", value * 100.0),
            "reinforce_rate" => format!("增援率 {:+.0}%", value * 100.0),
            "front_demand_factor" => format!("前线需求 {:+.0}%", value * 100.0),
            "war_support_factor" => format!("战争支持度 {:+.0}%", value * 100.0),
            "attrition" => format!("损耗 {:+.0}%", value * 100.0),
            _ if value.fract().abs() < f32::EPSILON => format!("{key} {value:+.0}"),
            _ => format!("{key} {value:+.2}"),
        })
        .collect::<Vec<_>>()
        .join(" · ")
}

fn render_advisors_section(ui: &mut egui::Ui) {
    politics_card(ui, tr("advisors"), |ui| {
        ui.horizontal_wrapped(|ui| {
            for i in 0..5 {
                let (rect, response) =
                    ui.allocate_exact_size(Vec2::new(44.0, 44.0), egui::Sense::hover());
                ui.painter().rect_filled(rect, 4.0, PANEL_CARD_SOFT);
                ui.painter().rect_stroke(
                    rect,
                    4.0,
                    egui::Stroke::new(1.0, STROKE_DARK),
                    egui::epaint::StrokeKind::Outside,
                );
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    format!("{}", i + 1),
                    egui::FontId::proportional(13.0),
                    components::GOLD,
                );
                response.on_hover_text("顾问数据尚未接入：该槽位当前为空。");
            }
        });
        ui.label(
            RichText::new("顾问槽状态：暂无可任命顾问数据；不会消耗政治力量。")
                .small()
                .color(components::MUTED),
        );
    });
}

fn politics_card(ui: &mut egui::Ui, title: &str, add_contents: impl FnOnce(&mut egui::Ui)) {
    components::section_card(ui, title, add_contents);
}

fn metric_tile(ui: &mut egui::Ui, label: &str, value: String, color: Color32) {
    components::metric_tile(ui, label, value, color);
}

fn render_leader_portrait(
    ui: &mut egui::Ui,
    data: &PoliticsData,
    icon_bank: &mut crate::icons::IconBank,
    portrait_size: Vec2,
) {
    let mut portrait_drawn = false;
    if let Some(gfx) = data.leader_portrait_key.as_deref() {
        if let Some(handle) = icon_bank.get_or_load(gfx) {
            ui.add(
                egui::Image::from_texture(handle)
                    .fit_to_exact_size(portrait_size)
                    .corner_radius(2.0),
            );
            portrait_drawn = true;
        }
    }
    if portrait_drawn {
        return;
    }

    let flag_gfx = format!("GFX_flag_{}", data.country_tag);
    if !data.country_tag.is_empty() {
        if let Some(handle) = icon_bank.get_or_load(&flag_gfx) {
            ui.add(
                egui::Image::from_texture(handle)
                    .fit_to_exact_size(portrait_size)
                    .corner_radius(2.0),
            );
            return;
        }
    }

    let (rect, _) = ui.allocate_exact_size(portrait_size, egui::Sense::hover());
    ui.painter().rect_filled(rect, 2.0, Color32::from_gray(40));
    ui.painter().rect_stroke(
        rect,
        2.0,
        egui::Stroke::new(1.5, components::GOLD),
        egui::epaint::StrokeKind::Outside,
    );
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        if data.country_tag.is_empty() {
            "?"
        } else {
            data.country_tag.as_str()
        },
        egui::FontId::proportional(14.0),
        components::GOLD,
    );
}

fn render_ideology_pie(ui: &mut egui::Ui, popularity: &[(String, f32)], diameter: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(diameter), egui::Sense::hover());
    let center = rect.center();
    let radius = diameter * 0.5;
    let total: f32 = popularity.iter().map(|(_, pop)| pop.max(0.0)).sum();

    if total <= f32::EPSILON {
        ui.painter()
            .circle_filled(center, radius, Color32::from_rgb(70, 70, 70));
        ui.painter().circle_stroke(
            center,
            radius,
            egui::Stroke::new(2.0, Color32::from_rgb(40, 34, 26)),
        );
        return;
    }

    let mut start = -std::f32::consts::FRAC_PI_2;
    for (key, pop) in popularity {
        let frac = (pop.max(0.0) / total).clamp(0.0, 1.0);
        if frac <= 0.0 {
            continue;
        }
        let sweep = frac * std::f32::consts::TAU;
        let steps = ((sweep.abs() / std::f32::consts::TAU) * 72.0)
            .ceil()
            .max(3.0) as usize;
        let mut points = Vec::with_capacity(steps + 2);
        points.push(center);
        for i in 0..=steps {
            let t = start + sweep * (i as f32 / steps as f32);
            points.push(egui::pos2(
                center.x + t.cos() * radius,
                center.y + t.sin() * radius,
            ));
        }
        ui.painter().add(egui::Shape::convex_polygon(
            points,
            ideology_color(key),
            egui::Stroke::new(0.0, Color32::TRANSPARENT),
        ));
        start += sweep;
    }

    ui.painter().circle_stroke(
        center,
        radius,
        egui::Stroke::new(2.0, Color32::from_rgb(28, 22, 16)),
    );
    ui.painter().circle_stroke(
        center,
        radius - 3.0,
        egui::Stroke::new(1.0, Color32::from_black_alpha(120)),
    );
    ui.painter()
        .circle_filled(center, radius * 0.18, Color32::from_rgb(0x20, 0x18, 0x12));
    ui.painter().circle_stroke(
        center,
        radius * 0.18,
        egui::Stroke::new(1.0, components::GOLD),
    );
}

fn ideology_chip(ui: &mut egui::Ui, key: &str, support: f32) {
    let color = ideology_color(key);
    egui::Frame::new()
        .fill(Color32::from_rgba_premultiplied(
            color.r(),
            color.g(),
            color.b(),
            70,
        ))
        .stroke(egui::Stroke::new(1.0, color))
        .inner_margin(egui::Margin::symmetric(7, 3))
        .show(ui, |ui| {
            ui.label(
                RichText::new(format!("{} {:.0}%", ideology_label(key), support * 100.0))
                    .small()
                    .strong()
                    .color(color),
            );
        });
}

fn ruling_party_support(data: &PoliticsData) -> f32 {
    data.party_popularity
        .iter()
        .find(|(key, _)| key == &data.ruling_party)
        .map(|(_, pop)| *pop)
        .unwrap_or(0.0)
        .clamp(0.0, 1.0)
}

fn percent_color(v: f32) -> Color32 {
    if v >= 0.60 {
        GOOD
    } else if v >= 0.40 {
        WARN
    } else {
        BAD
    }
}

fn render_ideology_popularity_row(ui: &mut egui::Ui, key: &str, pop: f32) {
    let color = ideology_color(key);
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(Vec2::new(22.0, 14.0), egui::Sense::hover());
        ui.painter().rect_filled(rect, 1.0, color);
        ui.painter().rect_stroke(
            rect,
            1.0,
            egui::Stroke::new(1.0, Color32::from_black_alpha(170)),
            egui::epaint::StrokeKind::Outside,
        );
        ui.label(RichText::new(ideology_label(key)).strong().color(color));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                RichText::new(format!("{:.0}%", pop * 100.0))
                    .strong()
                    .color(color),
            );
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str) -> DecisionEntry {
        DecisionEntry {
            id: id.into(),
            name: id.into(),
            description: String::new(),
            effect_preview: String::new(),
            category: DecisionCategory::Industry,
            mechanic_kind: DecisionMechanicKind::Standard,
            cost_political_power: 50.0,
            visible: true,
            clickable: true,
            mission_remaining: None,
            mission_total: None,
            cooldown_remaining: None,
            already_fired: false,
        }
    }

    #[test]
    fn legacy_constructor_has_no_focus() {
        let d = PoliticsData::legacy("fascism".into(), vec![], vec![]);
        assert!(!d.focus_available);
        assert_eq!(d.political_power, 0.0);
    }

    #[test]
    fn entry_from_def_inherits_fields() {
        use hoi4_content::{Decision, DecisionCategory, Trigger};
        let d = Decision {
            id: "x".into(),
            name: "X".into(),
            description: "desc".into(),
            icon: String::new(),
            category: DecisionCategory::Military,
            mechanic_kind: DecisionMechanicKind::Standard,
            visible: Trigger::AlwaysTrue,
            available: Trigger::AlwaysTrue,
            cost_political_power: 75.0,
            days_mission_timeout: 0,
            days_re_enable: 0,
            fire_only_once: false,
            on_activation: vec![],
            on_complete: vec![],
            cancel_trigger: Trigger::AlwaysFalse,
            on_cancel: vec![],
        };
        let e = DecisionEntry::from_def(&d);
        assert_eq!(e.id, "x");
        assert_eq!(e.cost_political_power, 75.0);
        assert!(matches!(e.category, DecisionCategory::Military));
    }

    #[test]
    fn entry_status_flags_independent() {
        let mut e = entry("e");
        e.mission_remaining = Some(10);
        e.mission_total = Some(30);
        // 直接断字段：UI 渲染逻辑由 render_decision_status 消费这几个字段。
        assert_eq!(e.mission_remaining, Some(10));
        assert!(e.mission_total.is_some());
    }

    #[test]
    fn idea_modifier_summary_formats_known_modifiers() {
        let s = idea_modifier_summary(&[
            ("political_power_gain".to_owned(), 0.15),
            ("consumer_goods_factor".to_owned(), -0.10),
        ]);
        assert!(s.contains("政治力量 +0.15/日"));
        assert!(s.contains("消费品工厂 -10%"));
    }
}
