//! V5 阶段 C.2 + F.2：政治面板。
//!
//! - C.2：党派色块 + 民众支持率条 + ideas 列表 + 5 顾问槽位（空）
//! - F.2：决议列表 — 5 分类 tab，按可见性筛选，按按钮显示状态：
//!     - 可点（绿色）/ 灰显（条件不满足）/ 冷却中 / 进行中 / 已触发
//!     - 点击后返回命令，由调用方执行。
//!       `DecisionState::activate(...)` 真正执行。

#![allow(dead_code, deprecated)]

use crate::{
    components, i18n::tr, law_panel, vanilla_iron::VanillaIron, ActiveDetailPanel, PanelCommand,
};
use egui::{Color32, Pos2, Rect, RichText, Sense, Vec2};

use hoi4_content::{Decision, DecisionCategory, DecisionMechanicKind, Effect};
use hoi4_state::LawCategory;

const PANEL_CARD: Color32 = Color32::from_rgb(0x24, 0x1a, 0x12);
const PANEL_CARD_SOFT: Color32 = Color32::from_rgb(0x31, 0x24, 0x18);
const STROKE_DARK: Color32 = Color32::from_rgb(0x5a, 0x44, 0x2c);
const GOOD: Color32 = Color32::from_rgb(0x70, 0xc8, 0x78);
const WARN: Color32 = Color32::from_rgb(0xff, 0xc0, 0x60);
const BAD: Color32 = Color32::from_rgb(0xe0, 0x60, 0x58);
const IDEA_SLOT_SIZE: f32 = 54.0;
const IDEA_ICON_SIZE: f32 = 44.0;
const VANILLA_CARD_GAP: f32 = 6.0;

fn ideology_color(key: &str) -> Color32 {
    hoi4_ideology_color(key)
}

fn hoi4_ideology_color(key: &str) -> Color32 {
    match key {
        "democratic" => Color32::from_rgb(0x32, 0x68, 0xa6),
        "communism" => Color32::from_rgb(0xa8, 0x2b, 0x25),
        "fascism" => Color32::from_rgb(0x83, 0x55, 0x32),
        "neutrality" => Color32::from_rgb(0x8f, 0x8d, 0x80),
        _ => Color32::from_rgb(0x66, 0x66, 0x60),
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
    pub government_posts: Vec<GovernmentPostEntry>,
    pub law_slots: Vec<PoliticsLawEntry>,
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
            government_posts: Vec::new(),
            law_slots: Vec::new(),
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

#[derive(Debug, Clone)]
pub struct GovernmentPostEntry {
    pub office: String,
    pub name: String,
    pub detail: String,
}

#[derive(Debug, Clone)]
pub struct PoliticsLawEntry {
    pub category: LawCategory,
    pub current_name: String,
    pub cooldown_days: u16,
    pub pending: Option<(String, u16)>,
    pub is_locked: bool,
}

/// F.2：面板回写命令。
#[derive(Debug, Clone, PartialEq)]
pub enum DecisionCommand {
    /// 玩家点了某个决议的执行按钮。
    Activate(String),
    OpenFocusTree,
    Panel(PanelCommand),
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
        vanilla_show_politics(ctx, data, icon_bank)
    }
}

fn vanilla_show_politics(
    ctx: &egui::Context,
    data: &PoliticsData,
    icon_bank: &mut crate::icons::IconBank,
) -> (bool, Vec<DecisionCommand>) {
    use crate::v9::{
        composites::side_rail::{SIDE_RAIL_PANEL_LEFT, SIDE_RAIL_TOP_OFFSET},
        paint,
        tokens::TextRole,
    };

    let accent = v9_ideology_color(&data.ruling_party);
    let screen = ctx.screen_rect();
    let left_gap = if screen.width() >= 980.0 {
        SIDE_RAIL_PANEL_LEFT
    } else {
        8.0
    };
    let top_gap = if screen.height() >= 680.0 {
        SIDE_RAIL_TOP_OFFSET
    } else {
        72.0
    };
    let panel_w = 820.0_f32.min((screen.width() - left_gap - 8.0).max(420.0));
    let panel_h = (screen.height() - top_gap - 8.0).max(360.0);
    let panel_pos = Pos2::new(screen.left() + left_gap, screen.top() + top_gap);
    let panel_size = Vec2::new(panel_w, panel_h);

    let mut close = false;
    let mut output = Vec::new();
    egui::Area::new(egui::Id::new("politics_panel_vanilla_1936"))
        .order(egui::Order::Foreground)
        .fixed_pos(panel_pos)
        .show(ctx, |ui| {
            let (outer, _) = ui.allocate_exact_size(panel_size, Sense::click_and_drag());
            paint::paint_shadow(ui.painter(), outer, crate::v9::Elevation::E2, 1.0);
            vanilla_politics_shell(ui, outer, accent);

            let inner = outer.shrink2(Vec2::new(14.0, 12.0));
            ui.painter().text(
                Pos2::new(inner.left() + 2.0, inner.top() + 5.0),
                egui::Align2::LEFT_TOP,
                tr("politics"),
                TextRole::Display.font_id(),
                vanilla_text(),
            );

            let close_rect = Rect::from_min_size(
                Pos2::new(inner.right() - 28.0, inner.top() - 2.0),
                Vec2::splat(23.0),
            );
            if vanilla_close_button(ui, close_rect)
                .on_hover_text(tr("panel_close_hint"))
                .clicked()
            {
                close = true;
            }

            let body = Rect::from_min_max(
                Pos2::new(inner.left(), inner.top() + 52.0),
                Pos2::new(inner.right(), inner.bottom() - 8.0),
            );

            let mut cmds = Vec::new();
            v9_politics_body(ui, body, data, icon_bank, &mut cmds);
            output = cmds;
        });

    (close, output)
}

fn v9_show_politics(
    ctx: &egui::Context,
    data: &PoliticsData,
    icon_bank: &mut crate::icons::IconBank,
) -> (bool, Vec<DecisionCommand>) {
    use crate::v9::{
        composites::side_rail::{SIDE_RAIL_PANEL_LEFT, SIDE_RAIL_TOP_OFFSET},
        paint,
        primitives::{Button, ButtonSize, ButtonVariant},
        tokens::{palette, spacing, Elevation, TextRole},
    };

    let accent = v9_ideology_color(&data.ruling_party);

    let screen = ctx.screen_rect();
    let left_gap = if screen.width() >= 980.0 {
        SIDE_RAIL_PANEL_LEFT
    } else {
        8.0
    };
    let top_gap = if screen.height() >= 680.0 {
        SIDE_RAIL_TOP_OFFSET
    } else {
        72.0
    };
    let panel_w = 820.0_f32.min((screen.width() - left_gap - 8.0).max(420.0));
    let available_h = (screen.height() - top_gap - 8.0).max(360.0);
    let panel_h = available_h.min(820.0);
    let panel_pos = Pos2::new(screen.left() + left_gap, screen.top() + top_gap);
    let panel_size = Vec2::new(panel_w, panel_h);

    let mut close = false;
    let mut output = Vec::new();
    egui::Area::new(egui::Id::new("politics_panel_hoi4_rebuild"))
        .order(egui::Order::Foreground)
        .fixed_pos(panel_pos)
        .show(ctx, |ui| {
            let (outer, _) = ui.allocate_exact_size(panel_size, Sense::click_and_drag());
            paint::paint_shadow(ui.painter(), outer, Elevation::E3, 2.0);
            paint_politics_shell(ui, outer, accent);

            let inner = outer.shrink2(Vec2::new(spacing::S5, spacing::S4));
            let header = Rect::from_min_size(inner.min, Vec2::new(inner.width(), 66.0));
            let tab = Rect::from_min_size(
                Pos2::new(inner.left(), header.bottom() + spacing::S4),
                Vec2::new(inner.width(), 36.0),
            );
            let body = Rect::from_min_max(
                Pos2::new(inner.left(), tab.bottom() + spacing::S4),
                Pos2::new(inner.right(), inner.bottom() - spacing::S2),
            );

            paint::paint_recessed_panel(ui.painter(), header, 1.0);
            ui.painter().rect_filled(
                Rect::from_min_max(
                    header.left_top(),
                    Pos2::new(header.left() + 4.0, header.bottom()),
                ),
                0.0,
                accent,
            );
            ui.painter().text(
                Pos2::new(header.left() + spacing::S5, header.top() + spacing::S3),
                egui::Align2::LEFT_TOP,
                tr("politics"),
                TextRole::Display.font_id(),
                palette::GOLD_HOT,
            );
            ui.painter().text(
                Pos2::new(header.left() + spacing::S5, header.bottom() - spacing::S2),
                egui::Align2::LEFT_BOTTOM,
                if data.country_tag.is_empty() {
                    tr("country")
                } else {
                    data.country_tag.as_str()
                },
                TextRole::Caption.font_id(),
                palette::PARCHMENT_DIM,
            );

            let close_rect = Rect::from_min_size(
                Pos2::new(header.right() - 30.0, header.top() + spacing::S3),
                Vec2::splat(24.0),
            );
            if Button::new("X")
                .size(ButtonSize::Sm)
                .variant(ButtonVariant::Ghost)
                .show_at(ui, close_rect)
                .on_hover_text(tr("panel_close_hint"))
                .clicked()
            {
                close = true;
            }

            draw_politics_tab_strip(ui, tab, "政权总览 / 意识形态 / 国家精神 / 政府制度", accent);
            let mut cmds = Vec::new();
            v9_politics_body(ui, body, data, icon_bank, &mut cmds);
            output = cmds;
        });
    (close, output)
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
            .id_salt("politics_vanilla_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let width = ui.available_width().max(360.0);
                if width >= 760.0 {
                    let gap = 10.0;
                    let right_w = 360.0_f32.min((width - gap) * 0.46);
                    let left_w = (width - gap - right_w).max(360.0);
                    let top_h =
                        vanilla_leader_card_height().max(vanilla_government_card_height(data));
                    let ideas_h = vanilla_ideas_card_height(data);
                    let base_ideology_h = vanilla_ideology_card_height(data);
                    let lower_stack_h = ideas_h + VANILLA_CARD_GAP + base_ideology_h;
                    let target_lower_h =
                        (rect.height() - top_h - VANILLA_CARD_GAP).max(lower_stack_h);
                    let aligned_lower_h = lower_stack_h
                        .max(vanilla_law_systems_card_height(data))
                        .max(target_lower_h);
                    let ideology_h = base_ideology_h + (aligned_lower_h - lower_stack_h);
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing.x = gap;
                        ui.allocate_ui_with_layout(
                            Vec2::new(left_w, 0.0),
                            egui::Layout::top_down(egui::Align::Min),
                            |ui| {
                                ui.set_width(left_w);
                                vanilla_leader_card(ui, data, icon_bank, cmds, top_h);
                                vanilla_ideas_card(ui, data, icon_bank, ideas_h);
                                vanilla_ideology_card(ui, data, ideology_h);
                            },
                        );
                        ui.allocate_ui_with_layout(
                            Vec2::new(right_w, 0.0),
                            egui::Layout::top_down(egui::Align::Min),
                            |ui| {
                                ui.set_width(right_w);
                                vanilla_government_card(ui, data, top_h);
                                vanilla_law_systems_card(ui, data, cmds, aligned_lower_h);
                            },
                        );
                    });
                } else {
                    vanilla_leader_card(ui, data, icon_bank, cmds, vanilla_leader_card_height());
                    vanilla_government_card(ui, data, vanilla_government_card_height(data));
                    vanilla_ideas_card(ui, data, icon_bank, vanilla_ideas_card_height(data));
                    vanilla_law_systems_card(ui, data, cmds, vanilla_law_systems_card_height(data));
                    vanilla_ideology_card(ui, data, vanilla_ideology_card_height(data));
                }
            });
    });
}

fn paint_politics_shell(ui: &mut egui::Ui, rect: Rect, accent: Color32) {
    use crate::v9::{paint, tokens::palette};

    ui.painter().rect_filled(rect, 1.0, palette::SOOT_BLACK);
    paint::paint_vertical_gradient_mesh(
        ui.painter(),
        rect.shrink(2.0),
        Color32::from_rgba_premultiplied(0x18, 0x1b, 0x17, 245),
        Color32::from_black_alpha(252),
    );
    ui.painter().rect_stroke(
        rect,
        1.0,
        egui::Stroke::new(2.0, palette::EDGE_DARK),
        egui::epaint::StrokeKind::Inside,
    );
    ui.painter().rect_stroke(
        rect.shrink(2.0),
        1.0,
        egui::Stroke::new(1.0, palette::BRASS_DARK),
        egui::epaint::StrokeKind::Inside,
    );
    ui.painter().rect_stroke(
        rect.shrink(6.0),
        0.0,
        egui::Stroke::new(1.0, Color32::from_black_alpha(230)),
        egui::epaint::StrokeKind::Inside,
    );
    ui.painter().hline(
        (rect.left() + 22.0)..=(rect.right() - 22.0),
        rect.top() + 9.0,
        egui::Stroke::new(1.0, accent),
    );
    ui.painter().hline(
        (rect.left() + 22.0)..=(rect.right() - 22.0),
        rect.bottom() - 9.0,
        egui::Stroke::new(1.0, palette::BRASS_DARK),
    );
}

fn draw_politics_tab_strip(ui: &mut egui::Ui, rect: Rect, label: &str, accent: Color32) {
    use crate::v9::{
        paint,
        tokens::{palette, spacing, TextRole},
    };

    paint::paint_recessed_panel(ui.painter(), rect, 1.0);
    ui.painter().rect_filled(
        Rect::from_min_max(rect.left_top(), Pos2::new(rect.left() + 4.0, rect.bottom())),
        0.0,
        accent,
    );
    ui.painter().hline(
        (rect.left() + spacing::S4)..=(rect.right() - spacing::S4),
        rect.top() + 1.0,
        egui::Stroke::new(1.0, palette::BRASS_DARK),
    );
    ui.painter().text(
        Pos2::new(rect.left() + spacing::S5, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        TextRole::Subheading.font_id(),
        accent,
    );
}

fn vanilla_politics_shell(ui: &mut egui::Ui, rect: Rect, accent: Color32) {
    VanillaIron::paint_panel(ui, rect, accent);
}

fn paint_vanilla_border(painter: &egui::Painter, rect: Rect) {
    VanillaIron::paint_border(painter, rect);
}

fn vanilla_close_button(ui: &mut egui::Ui, rect: Rect) -> egui::Response {
    VanillaIron::close_button(ui, rect, ui.id().with("politics_vanilla_close"))
}

fn vanilla_black() -> Color32 {
    VanillaIron::BLACK
}

fn vanilla_card() -> Color32 {
    VanillaIron::CARD
}

fn vanilla_edge() -> Color32 {
    VanillaIron::EDGE
}

fn vanilla_text() -> Color32 {
    VanillaIron::TEXT
}

fn vanilla_muted() -> Color32 {
    VanillaIron::MUTED
}

fn v9_card(ui: &mut egui::Ui, height: f32, add_contents: impl FnOnce(&mut egui::Ui, Rect)) {
    let width = ui.available_width().max(360.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    paint_politics_card_frame(ui, rect);
    let inner = rect.shrink2(Vec2::new(9.0, 7.0));
    add_contents(ui, inner);
    ui.add_space(VANILLA_CARD_GAP);
}

fn paint_politics_card_frame(ui: &mut egui::Ui, rect: Rect) {
    let painter = ui.painter();
    painter.rect_filled(rect, 1.0, Color32::from_rgb(0x08, 0x09, 0x08));
    crate::v9::paint::paint_vertical_gradient_mesh(
        painter,
        rect.shrink(2.0),
        Color32::from_rgba_premultiplied(0x19, 0x1b, 0x17, 238),
        Color32::from_rgba_premultiplied(0x03, 0x04, 0x03, 252),
    );
    crate::v9::paint::paint_horizontal_gradient_mesh(
        painter,
        rect.shrink(2.0),
        Color32::from_black_alpha(120),
        Color32::from_white_alpha(3),
    );
    crate::v9::paint::paint_plate_grain(painter, rect.shrink(4.0), 3.0, 2);
    paint_vanilla_border(painter, rect);
    painter.rect_stroke(
        rect.shrink(3.0),
        egui::epaint::CornerRadius::same(0),
        egui::Stroke::new(1.0, Color32::from_black_alpha(190)),
        egui::epaint::StrokeKind::Inside,
    );
    painter.hline(
        (rect.left() + 8.0)..=(rect.right() - 8.0),
        rect.top() + 3.0,
        egui::Stroke::new(1.0, Color32::from_white_alpha(12)),
    );
    painter.hline(
        (rect.left() + 8.0)..=(rect.right() - 8.0),
        rect.bottom() - 3.0,
        egui::Stroke::new(1.0, Color32::from_black_alpha(230)),
    );
}

fn vanilla_leader_card_height() -> f32 {
    238.0
}

fn vanilla_ideas_card_height(_data: &PoliticsData) -> f32 {
    122.0
}

fn vanilla_ideology_card_height(data: &PoliticsData) -> f32 {
    (76.0 + data.party_popularity.len().max(1) as f32 * 24.0).max(206.0)
}

fn vanilla_government_card_height(data: &PoliticsData) -> f32 {
    let rows = data.government_posts.len().max(1);
    (48.0 + rows as f32 * 43.0).max(238.0)
}

fn vanilla_law_systems_card_height(data: &PoliticsData) -> f32 {
    let rows = data.law_slots.len().max(1);
    (48.0 + rows as f32 * 43.0).max(294.0)
}

fn vanilla_leader_card(
    ui: &mut egui::Ui,
    data: &PoliticsData,
    icon_bank: &mut crate::icons::IconBank,
    cmds: &mut Vec<DecisionCommand>,
    height: f32,
) {
    v9_card(ui, height, |ui, inner| {
        vanilla_section_title(ui, inner, "\u{6267}\u{653f}\u{515a}");

        let content = Rect::from_min_max(
            Pos2::new(inner.left(), inner.top() + 34.0),
            inner.right_bottom(),
        );
        let portrait_rect = Rect::from_min_size(
            content.left_top() + Vec2::new(2.0, 4.0),
            Vec2::new(102.0, 148.0),
        );
        draw_vanilla_portrait(ui, portrait_rect, data, icon_bank);

        let text_left = portrait_rect.right() + 18.0;
        let text_rect = Rect::from_min_max(
            Pos2::new(text_left, portrait_rect.top() + 2.0),
            Pos2::new(content.right() - 4.0, portrait_rect.bottom()),
        );
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
        let painter = ui.painter().with_clip_rect(text_rect);
        painter.text(
            text_rect.left_top(),
            egui::Align2::LEFT_TOP,
            leader.as_str(),
            fit_text_font(
                leader.as_str(),
                crate::v9::TextRole::Heading.font_id(),
                text_rect.width(),
            ),
            vanilla_gold(),
        );
        painter.text(
            Pos2::new(text_rect.left(), text_rect.top() + 30.0),
            egui::Align2::LEFT_TOP,
            party.as_str(),
            fit_text_font(
                party.as_str(),
                crate::v9::TextRole::Body.font_id(),
                text_rect.width(),
            ),
            v9_ideology_color(&data.ruling_party),
        );
        vanilla_info_line(
            ui,
            Pos2::new(text_rect.left(), text_rect.top() + 70.0),
            "\u{610f}\u{8bc6}\u{5f62}\u{6001}:",
            ideology_label(&data.ruling_party),
            v9_ideology_color(&data.ruling_party),
        );
        vanilla_info_line(
            ui,
            Pos2::new(text_rect.left(), text_rect.top() + 96.0),
            "\u{6267}\u{653f}\u{5730}\u{4f4d}:",
            "\u{6267}\u{653f}\u{515a}",
            vanilla_gold(),
        );
        vanilla_info_line(
            ui,
            Pos2::new(text_rect.left(), text_rect.top() + 122.0),
            "\u{4e0b}\u{4e00}\u{6b21}\u{9009}\u{4e3e}:",
            "1940\u{5e74}1\u{6708}",
            vanilla_text(),
        );

        if data.focus_available {
            let button = Rect::from_min_size(
                Pos2::new(content.right() - 126.0, content.bottom() - 34.0),
                Vec2::new(118.0, 28.0),
            );
            if vanilla_action_button(ui, button, "\u{56fd}\u{7b56}") {
                cmds.push(DecisionCommand::OpenFocusTree);
            }
        }
    });
}

fn vanilla_ideas_card(
    ui: &mut egui::Ui,
    data: &PoliticsData,
    icon_bank: &mut crate::icons::IconBank,
    height: f32,
) {
    v9_card(ui, height, |ui, inner| {
        vanilla_section_title(ui, inner, tr("national_spirits"));
        let content = Rect::from_min_max(
            Pos2::new(inner.left(), inner.top() + 34.0),
            inner.right_bottom(),
        );
        if data.ideas.is_empty() {
            let slot = Rect::from_min_size(
                Pos2::new(content.left() + 12.0, content.top() + 8.0),
                Vec2::splat(50.0),
            );
            vanilla_slot(ui, slot, Color32::from_rgb(0x34, 0x35, 0x30));
            ui.painter().text(
                slot.center(),
                egui::Align2::CENTER_CENTER,
                "?",
                crate::v9::TextRole::Heading.font_id(),
                Color32::from_rgb(0x4c, 0x4b, 0x43),
            );
            ui.painter().text(
                Pos2::new(slot.right() + 14.0, slot.center().y),
                egui::Align2::LEFT_CENTER,
                tr("idea_none"),
                crate::v9::TextRole::Body.font_id(),
                vanilla_muted(),
            );
            return;
        }

        let slot = Vec2::splat(54.0);
        let gap = 8.0;
        let cols = (((content.width() + gap) / (slot.x + gap)).floor() as usize).clamp(1, 7);
        for (idx, idea) in data.ideas.iter().enumerate() {
            let col = idx % cols;
            let row = idx / cols;
            let rect = Rect::from_min_size(
                content.left_top()
                    + Vec2::new(col as f32 * (slot.x + gap), row as f32 * (slot.y + gap)),
                slot,
            );
            vanilla_slot(ui, rect, vanilla_edge());
            if let Some(handle) = idea_icon_gfx(idea).and_then(|gfx| icon_bank.get_or_load(&gfx)) {
                ui.put(
                    rect.shrink(5.0),
                    egui::Image::from_texture(handle).fit_to_exact_size(rect.shrink(5.0).size()),
                );
            } else {
                let letter = idea
                    .name
                    .chars()
                    .find(|c| !c.is_whitespace())
                    .unwrap_or('?');
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    letter.to_string(),
                    crate::v9::TextRole::Heading.font_id(),
                    vanilla_gold(),
                );
            }
            ui.interact(
                rect,
                ui.id().with(("vanilla_idea", &idea.key)),
                Sense::hover(),
            )
            .on_hover_ui(|ui| render_idea_tooltip(ui, idea));
        }
    });
}

fn vanilla_ideology_card(ui: &mut egui::Ui, data: &PoliticsData, height: f32) {
    v9_card(ui, height, |ui, inner| {
        vanilla_section_title(ui, inner, "\u{610f}\u{8bc6}\u{5f62}\u{6001}");
        let content = Rect::from_min_max(
            Pos2::new(inner.left(), inner.top() + 40.0),
            inner.right_bottom(),
        );
        let donut = Rect::from_min_size(
            Pos2::new(content.left() + 26.0, content.top() + 10.0),
            Vec2::splat(128.0),
        );
        draw_vanilla_ideology_donut(ui, donut, &data.party_popularity);

        let legend = Rect::from_min_max(
            Pos2::new(donut.right() + 26.0, content.top() + 4.0),
            content.right_bottom(),
        );
        let mut y = legend.top();
        for (key, pop) in &data.party_popularity {
            let color = v9_ideology_color(key);
            let row =
                Rect::from_min_size(Pos2::new(legend.left(), y), Vec2::new(legend.width(), 24.0));
            ui.painter()
                .circle_filled(Pos2::new(row.left() + 7.0, row.center().y), 5.0, color);
            ui.painter().text(
                Pos2::new(row.left() + 20.0, row.center().y),
                egui::Align2::LEFT_CENTER,
                ideology_label(key),
                crate::v9::TextRole::Body.font_id(),
                vanilla_text(),
            );
            ui.painter().text(
                Pos2::new(row.right(), row.center().y),
                egui::Align2::RIGHT_CENTER,
                format!("{:.0}%", pop * 100.0),
                crate::v9::TextRole::Body.font_id(),
                vanilla_text(),
            );
            y += 28.0;
        }
    });
}

fn vanilla_government_card(ui: &mut egui::Ui, data: &PoliticsData, height: f32) {
    v9_card(ui, height, |ui, inner| {
        vanilla_section_title(ui, inner, "\u{653f}\u{5e9c}");
        if data.government_posts.is_empty() {
            vanilla_empty_text(
                ui,
                inner,
                "\u{6682}\u{65e0}\u{653f}\u{5e9c}\u{804c}\u{4f4d}\u{6570}\u{636e}",
            );
            return;
        }
        let mut y = inner.top() + 36.0;
        for (idx, post) in data.government_posts.iter().enumerate() {
            let rect =
                Rect::from_min_size(Pos2::new(inner.left(), y), Vec2::new(inner.width(), 37.0));
            let accent = if idx == 0 {
                v9_ideology_color(&data.ruling_party)
            } else {
                vanilla_edge()
            };
            vanilla_person_row(ui, rect, &post.office, &post.name, &post.detail, accent);
            y += 43.0;
        }
    });
}

fn vanilla_law_systems_card(
    ui: &mut egui::Ui,
    data: &PoliticsData,
    cmds: &mut Vec<DecisionCommand>,
    height: f32,
) {
    v9_card(ui, height, |ui, inner| {
        vanilla_section_title(ui, inner, "\u{6cd5}\u{5f8b}\u{4e0e}\u{5236}\u{5ea6}");
        if data.law_slots.is_empty() {
            vanilla_empty_text(
                ui,
                inner,
                "\u{6682}\u{65e0}\u{6cd5}\u{5f8b}\u{5236}\u{5ea6}\u{6570}\u{636e}",
            );
            return;
        }
        let mut y = inner.top() + 36.0;
        for slot in &data.law_slots {
            let rect =
                Rect::from_min_size(Pos2::new(inner.left(), y), Vec2::new(inner.width(), 37.0));
            let response = ui.interact(
                rect,
                ui.id().with((
                    "politics_law_detail",
                    law_panel::law_category_key(slot.category),
                )),
                Sense::click(),
            );
            if response.clicked() {
                cmds.push(DecisionCommand::Panel(PanelCommand::OpenDetail(
                    ActiveDetailPanel::Law {
                        category: law_panel::law_category_key(slot.category).to_owned(),
                        law_id: None,
                    },
                )));
            }
            response.on_hover_text("点击打开法律详情");
            let status = politics_law_status_text(slot);
            vanilla_law_row(ui, rect, slot, &status);
            y += 43.0;
        }
    });
}

fn vanilla_section_title(ui: &mut egui::Ui, inner: Rect, title: &str) {
    let title_rect = Rect::from_min_size(inner.left_top(), Vec2::new(inner.width(), 26.0));
    VanillaIron::section_title_at(ui, title_rect, title);
}

fn vanilla_info_line(ui: &mut egui::Ui, pos: Pos2, label: &str, value: &str, value_color: Color32) {
    ui.painter().text(
        pos,
        egui::Align2::LEFT_TOP,
        label,
        crate::v9::TextRole::Body.font_id(),
        vanilla_muted(),
    );
    ui.painter().text(
        pos + Vec2::new(92.0, 0.0),
        egui::Align2::LEFT_TOP,
        value,
        crate::v9::TextRole::Body.font_id(),
        value_color,
    );
}

fn draw_vanilla_portrait(
    ui: &mut egui::Ui,
    rect: Rect,
    data: &PoliticsData,
    icon_bank: &mut crate::icons::IconBank,
) {
    vanilla_slot(ui, rect, v9_ideology_color(&data.ruling_party));
    let mut drawn = false;
    let image_rect = rect.shrink(2.0);
    if let Some(gfx) = data.leader_portrait_key.as_deref() {
        if let Some(handle) = icon_bank.get_or_load(gfx) {
            ui.put(
                image_rect,
                egui::Image::from_texture(handle).fit_to_exact_size(image_rect.size()),
            );
            drawn = true;
        }
    }
    if !drawn && !data.country_tag.is_empty() {
        let gfx = format!("GFX_flag_{}", data.country_tag);
        if let Some(handle) = icon_bank.get_or_load(&gfx) {
            ui.put(
                image_rect,
                egui::Image::from_texture(handle).fit_to_exact_size(image_rect.size()),
            );
            drawn = true;
        }
    }
    if !drawn {
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            if data.country_tag.is_empty() {
                "?"
            } else {
                data.country_tag.as_str()
            },
            crate::v9::TextRole::Heading.font_id(),
            v9_ideology_color(&data.ruling_party),
        );
    }
    ui.painter().rect_stroke(
        image_rect,
        egui::epaint::CornerRadius::same(0),
        egui::Stroke::new(1.0, Color32::from_black_alpha(230)),
        egui::epaint::StrokeKind::Inside,
    );
    ui.painter().rect_stroke(
        rect,
        egui::epaint::CornerRadius::same(1),
        egui::Stroke::new(1.0, v9_ideology_color(&data.ruling_party)),
        egui::epaint::StrokeKind::Inside,
    );
}

fn vanilla_person_row(
    ui: &mut egui::Ui,
    rect: Rect,
    office: &str,
    name: &str,
    detail: &str,
    accent: Color32,
) {
    vanilla_row_frame(ui, rect);
    let icon = Rect::from_min_size(rect.left_top() + Vec2::new(7.0, 5.0), Vec2::splat(27.0));
    vanilla_slot(ui, icon, accent);
    draw_panel_svg_icon(ui, icon.shrink(5.0), PanelSvgIcon::Person, vanilla_muted());
    let text_left = icon.right() + 10.0;
    let action = Rect::from_min_size(
        Pos2::new(rect.right() - 34.0, rect.top() + 4.0),
        Vec2::splat(29.0),
    );
    vanilla_icon_button(ui, action, PanelSvgIcon::Government, accent);
    ui.painter().text(
        Pos2::new(text_left, rect.top() + 3.0),
        egui::Align2::LEFT_TOP,
        office,
        crate::v9::TextRole::Caption.font_id(),
        vanilla_muted(),
    );
    ui.painter().text(
        Pos2::new(text_left, rect.top() + 18.0),
        egui::Align2::LEFT_TOP,
        name,
        fit_text_font(
            name,
            crate::v9::TextRole::Body.font_id(),
            action.left() - text_left - 58.0,
        ),
        vanilla_text(),
    );
    if !detail.is_empty() {
        ui.painter().text(
            Pos2::new(action.left() - 7.0, rect.center().y),
            egui::Align2::RIGHT_CENTER,
            detail,
            crate::v9::TextRole::Caption.font_id(),
            accent,
        );
    }
}

fn vanilla_law_row(ui: &mut egui::Ui, rect: Rect, slot: &PoliticsLawEntry, status: &str) {
    vanilla_row_frame(ui, rect);
    let icon = Rect::from_min_size(rect.left_top() + Vec2::new(7.0, 5.0), Vec2::splat(27.0));
    vanilla_slot(ui, icon, Color32::from_rgb(0x4d, 0x45, 0x35));
    draw_panel_svg_icon(
        ui,
        icon.shrink(5.0),
        PanelSvgIcon::from_law(&slot.category),
        Color32::from_rgb(0xb7, 0xb1, 0x9a),
    );
    let text_left = icon.right() + 10.0;
    let status_rect = Rect::from_min_size(
        Pos2::new(rect.right() - 73.0, rect.top() + 7.0),
        Vec2::new(66.0, 23.0),
    );
    ui.painter().text(
        Pos2::new(text_left, rect.top() + 3.0),
        egui::Align2::LEFT_TOP,
        politics_law_category_label(&slot.category),
        crate::v9::TextRole::Caption.font_id(),
        vanilla_muted(),
    );
    ui.painter().text(
        Pos2::new(text_left, rect.top() + 18.0),
        egui::Align2::LEFT_TOP,
        slot.current_name.as_str(),
        fit_text_font(
            slot.current_name.as_str(),
            crate::v9::TextRole::Body.font_id(),
            status_rect.left() - text_left - 8.0,
        ),
        vanilla_text(),
    );
    vanilla_status_pill(ui, status_rect, status, politics_law_status_color(slot));
}

fn vanilla_row_frame(ui: &mut egui::Ui, rect: Rect) {
    ui.painter()
        .rect_filled(rect, 1.0, Color32::from_rgb(0x12, 0x14, 0x12));
    crate::v9::paint::paint_vertical_gradient_mesh(
        ui.painter(),
        rect.shrink(1.0),
        Color32::from_rgba_premultiplied(0x25, 0x27, 0x22, 150),
        Color32::from_rgba_premultiplied(0x08, 0x09, 0x08, 235),
    );
    paint_vanilla_border(ui.painter(), rect);
    ui.painter().hline(
        (rect.left() + 4.0)..=(rect.right() - 4.0),
        rect.top() + 1.0,
        egui::Stroke::new(1.0, Color32::from_white_alpha(6)),
    );
}

fn vanilla_slot(ui: &mut egui::Ui, rect: Rect, accent: Color32) {
    ui.painter()
        .rect_filled(rect, 1.0, Color32::from_rgb(0x0d, 0x10, 0x10));
    ui.painter().rect_stroke(
        rect,
        egui::epaint::CornerRadius::same(1),
        egui::Stroke::new(1.0, accent),
        egui::epaint::StrokeKind::Inside,
    );
    ui.painter().rect_stroke(
        rect.shrink(3.0),
        egui::epaint::CornerRadius::same(0),
        egui::Stroke::new(1.0, Color32::from_black_alpha(210)),
        egui::epaint::StrokeKind::Inside,
    );
}

fn vanilla_icon_button(ui: &mut egui::Ui, rect: Rect, icon: PanelSvgIcon, accent: Color32) {
    ui.painter()
        .rect_filled(rect, 1.0, Color32::from_rgb(0x18, 0x19, 0x15));
    crate::v9::paint::paint_vertical_gradient_mesh(
        ui.painter(),
        rect.shrink(1.0),
        Color32::from_white_alpha(12),
        Color32::from_black_alpha(165),
    );
    paint_vanilla_border(ui.painter(), rect);
    draw_panel_svg_icon(ui, rect.shrink(6.0), icon, accent);
}

fn vanilla_status_pill(ui: &mut egui::Ui, rect: Rect, text: &str, color: Color32) {
    ui.painter()
        .rect_filled(rect, 1.0, Color32::from_rgb(0x0a, 0x0b, 0x09));
    ui.painter().rect_stroke(
        rect,
        egui::epaint::CornerRadius::same(1),
        egui::Stroke::new(1.0, Color32::from_black_alpha(220)),
        egui::epaint::StrokeKind::Inside,
    );
    ui.painter().rect_stroke(
        rect.shrink(1.0),
        egui::epaint::CornerRadius::same(0),
        egui::Stroke::new(1.0, color),
        egui::epaint::StrokeKind::Inside,
    );
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        text,
        fit_text_font(
            text,
            crate::v9::TextRole::Caption.font_id(),
            rect.width() - 6.0,
        ),
        color,
    );
}

#[derive(Clone, Copy)]
enum PanelSvgIcon {
    Person,
    Government,
    Helmet,
    Factory,
    Globe,
    Coin,
    Scales,
    Microphone,
}

impl PanelSvgIcon {
    fn from_law(category: &LawCategory) -> Self {
        match category {
            LawCategory::Conscription => Self::Helmet,
            LawCategory::Economy => Self::Factory,
            LawCategory::Trade => Self::Globe,
            LawCategory::Taxation => Self::Coin,
            LawCategory::CivilRights => Self::Scales,
            LawCategory::InformationControl => Self::Microphone,
        }
    }
}

fn draw_panel_svg_icon(ui: &mut egui::Ui, rect: Rect, icon: PanelSvgIcon, color: Color32) {
    let stroke = egui::Stroke::new(1.7, color);
    let thin = egui::Stroke::new(1.25, color);
    let p = |x: f32, y: f32| -> Pos2 {
        Pos2::new(
            rect.left() + rect.width() * x / 24.0,
            rect.top() + rect.height() * y / 24.0,
        )
    };
    let rr =
        |x: f32, y: f32, w: f32, h: f32| -> Rect { Rect::from_min_max(p(x, y), p(x + w, y + h)) };
    let painter = ui.painter();

    match icon {
        PanelSvgIcon::Person => {
            painter.circle_stroke(p(12.0, 7.5), rect.width() * 3.0 / 24.0, stroke);
            panel_svg_polyline(
                painter,
                &[
                    p(5.0, 21.0),
                    p(7.0, 15.0),
                    p(12.0, 12.5),
                    p(17.0, 15.0),
                    p(19.0, 21.0),
                ],
                stroke,
            );
            painter.line_segment([p(7.0, 21.0), p(17.0, 21.0)], thin);
        }
        PanelSvgIcon::Government => {
            panel_svg_polyline(painter, &[p(3.0, 9.0), p(12.0, 4.5), p(21.0, 9.0)], stroke);
            painter.line_segment([p(5.0, 10.0), p(19.0, 10.0)], stroke);
            for x in [7.0, 11.0, 15.0] {
                painter.line_segment([p(x, 11.0), p(x, 18.0)], thin);
            }
            painter.line_segment([p(5.0, 19.0), p(19.0, 19.0)], stroke);
            painter.line_segment([p(3.5, 21.0), p(20.5, 21.0)], thin);
        }
        PanelSvgIcon::Helmet => {
            panel_svg_polyline(
                painter,
                &[
                    p(4.0, 13.0),
                    p(6.0, 8.0),
                    p(12.0, 5.5),
                    p(18.0, 8.0),
                    p(20.0, 13.0),
                ],
                stroke,
            );
            painter.line_segment([p(4.0, 13.0), p(20.0, 13.0)], stroke);
            painter.line_segment([p(7.0, 16.5), p(17.0, 16.5)], thin);
            painter.line_segment([p(9.0, 20.0), p(15.0, 20.0)], thin);
        }
        PanelSvgIcon::Factory => {
            painter.rect_stroke(
                rr(4.0, 11.0, 16.0, 9.0),
                egui::epaint::CornerRadius::same(1),
                stroke,
                egui::epaint::StrokeKind::Inside,
            );
            panel_svg_polyline(
                painter,
                &[
                    p(4.0, 11.0),
                    p(8.0, 8.0),
                    p(12.0, 11.0),
                    p(16.0, 8.0),
                    p(20.0, 11.0),
                ],
                stroke,
            );
            painter.rect_stroke(
                rr(6.0, 5.0, 3.0, 6.0),
                egui::epaint::CornerRadius::same(0),
                thin,
                egui::epaint::StrokeKind::Inside,
            );
            for x in [8.0, 12.0, 16.0] {
                painter.line_segment([p(x, 15.0), p(x, 20.0)], thin);
            }
        }
        PanelSvgIcon::Globe => {
            painter.circle_stroke(p(12.0, 12.0), rect.width() * 7.0 / 24.0, stroke);
            painter.line_segment([p(5.0, 12.0), p(19.0, 12.0)], thin);
            painter.line_segment([p(12.0, 5.0), p(12.0, 19.0)], thin);
            painter.circle_stroke(p(12.0, 12.0), rect.width() * 3.8 / 24.0, thin);
        }
        PanelSvgIcon::Coin => {
            painter.circle_stroke(p(12.0, 12.0), rect.width() * 7.0 / 24.0, stroke);
            painter.line_segment([p(12.0, 6.0), p(12.0, 18.0)], thin);
            panel_svg_polyline(
                painter,
                &[
                    p(15.5, 8.5),
                    p(10.0, 8.5),
                    p(8.5, 10.5),
                    p(10.0, 12.0),
                    p(14.0, 12.0),
                    p(15.5, 13.5),
                    p(14.0, 15.5),
                    p(8.5, 15.5),
                ],
                thin,
            );
        }
        PanelSvgIcon::Scales => {
            painter.line_segment([p(12.0, 5.0), p(12.0, 20.0)], stroke);
            painter.line_segment([p(6.0, 8.0), p(18.0, 8.0)], stroke);
            painter.line_segment([p(8.0, 8.0), p(5.0, 15.0)], thin);
            painter.line_segment([p(8.0, 8.0), p(11.0, 15.0)], thin);
            painter.line_segment([p(16.0, 8.0), p(13.0, 15.0)], thin);
            painter.line_segment([p(16.0, 8.0), p(19.0, 15.0)], thin);
            panel_svg_polyline(
                painter,
                &[
                    p(4.5, 15.0),
                    p(11.5, 15.0),
                    p(10.0, 17.0),
                    p(6.0, 17.0),
                    p(4.5, 15.0),
                ],
                thin,
            );
            panel_svg_polyline(
                painter,
                &[
                    p(12.5, 15.0),
                    p(19.5, 15.0),
                    p(18.0, 17.0),
                    p(14.0, 17.0),
                    p(12.5, 15.0),
                ],
                thin,
            );
            painter.line_segment([p(8.0, 21.0), p(16.0, 21.0)], stroke);
        }
        PanelSvgIcon::Microphone => {
            painter.rect_stroke(
                rr(9.0, 4.5, 6.0, 10.0),
                egui::epaint::CornerRadius::same(3),
                stroke,
                egui::epaint::StrokeKind::Inside,
            );
            painter.line_segment([p(12.0, 14.5), p(12.0, 20.0)], stroke);
            painter.line_segment([p(8.0, 20.0), p(16.0, 20.0)], stroke);
            panel_svg_polyline(
                painter,
                &[
                    p(6.5, 11.0),
                    p(6.5, 14.0),
                    p(9.0, 17.0),
                    p(12.0, 17.5),
                    p(15.0, 17.0),
                    p(17.5, 14.0),
                    p(17.5, 11.0),
                ],
                thin,
            );
            painter.line_segment([p(10.5, 7.0), p(13.5, 7.0)], thin);
            painter.line_segment([p(10.5, 10.0), p(13.5, 10.0)], thin);
        }
    }
}

fn panel_svg_polyline(painter: &egui::Painter, points: &[Pos2], stroke: egui::Stroke) {
    painter.add(egui::Shape::line(points.to_vec(), stroke));
}

fn vanilla_action_button(ui: &mut egui::Ui, rect: Rect, label: &str) -> bool {
    let response = ui.interact(
        rect,
        ui.id().with(("vanilla_action", label)),
        Sense::click(),
    );
    ui.painter().rect_filled(
        rect,
        1.0,
        if response.hovered() {
            Color32::from_rgb(0x31, 0x32, 0x2a)
        } else {
            Color32::from_rgb(0x21, 0x22, 0x1c)
        },
    );
    paint_vanilla_border(ui.painter(), rect);
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        crate::v9::TextRole::Body.font_id(),
        vanilla_text(),
    );
    response.clicked()
}

fn draw_vanilla_ideology_donut(ui: &mut egui::Ui, rect: Rect, popularity: &[(String, f32)]) {
    let center = rect.center();
    let radius = rect.width().min(rect.height()) * 0.5;
    let total: f32 = popularity.iter().map(|(_, pop)| pop.max(0.0)).sum();
    if total <= f32::EPSILON {
        ui.painter()
            .circle_filled(center, radius, Color32::from_rgb(0x3d, 0x3d, 0x38));
        ui.painter()
            .circle_filled(center, radius * 0.56, vanilla_black());
        return;
    }
    let mut start = -std::f32::consts::FRAC_PI_2;
    for (key, pop) in popularity {
        let frac = (pop.max(0.0) / total).clamp(0.0, 1.0);
        if frac <= 0.0 {
            continue;
        }
        let sweep = frac * std::f32::consts::TAU;
        let steps = ((sweep / std::f32::consts::TAU) * 80.0).ceil().max(4.0) as usize;
        let mut points = Vec::with_capacity(steps + 2);
        points.push(center);
        for i in 0..=steps {
            let t = start + sweep * (i as f32 / steps as f32);
            points.push(Pos2::new(
                center.x + t.cos() * radius,
                center.y + t.sin() * radius,
            ));
        }
        ui.painter().add(egui::Shape::convex_polygon(
            points,
            v9_ideology_color(key),
            egui::Stroke::NONE,
        ));
        start += sweep;
    }
    ui.painter()
        .circle_filled(center, radius * 0.56, vanilla_black());
    ui.painter().circle_stroke(
        center,
        radius,
        egui::Stroke::new(2.0, Color32::from_black_alpha(220)),
    );
    ui.painter().circle_stroke(
        center,
        radius * 0.56,
        egui::Stroke::new(1.0, vanilla_edge()),
    );
}

fn vanilla_empty_text(ui: &mut egui::Ui, inner: Rect, text: &str) {
    ui.painter().text(
        inner.center(),
        egui::Align2::CENTER_CENTER,
        text,
        crate::v9::TextRole::Body.font_id(),
        vanilla_muted(),
    );
}

fn fit_text_font(text: &str, mut font: egui::FontId, max_width: f32) -> egui::FontId {
    let estimated = text.chars().count() as f32 * font.size * 0.56;
    if estimated > max_width && estimated > 1.0 {
        font.size *= (max_width / estimated).clamp(0.70, 1.0);
    }
    font
}

fn vanilla_gold() -> Color32 {
    VanillaIron::BRASS_BRIGHT
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
        let leader_clip = Rect::from_min_max(
            detail.left_top(),
            Pos2::new(detail.right(), detail.top() + 56.0),
        );
        let leader_painter = ui.painter().with_clip_rect(leader_clip);
        let mut leader_font = TextRole::Display.font_id();
        let leader_w = leader.chars().count() as f32 * leader_font.size * 0.56;
        if leader_w > detail.width() {
            leader_font.size *= (detail.width() / leader_w).clamp(0.78, 1.0);
        }
        leader_painter.text(
            Pos2::new(detail.left(), detail.top() + 4.0),
            egui::Align2::LEFT_TOP,
            leader,
            leader_font,
            palette::GOLD_HOT,
        );
        let mut party_font = TextRole::Subheading.font_id();
        let party_w = party.chars().count() as f32 * party_font.size * 0.56;
        if party_w > detail.width() {
            party_font.size *= (detail.width() / party_w).clamp(0.72, 1.0);
        }
        leader_painter.text(
            Pos2::new(detail.left(), detail.top() + 32.0),
            egui::Align2::LEFT_TOP,
            party,
            party_font,
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
        let focus_clip = Rect::from_min_max(
            Pos2::new(detail.left(), detail.top() + 90.0),
            Pos2::new(detail.right() - 132.0, detail.top() + 114.0),
        );
        ui.painter().with_clip_rect(focus_clip).text(
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
    use crate::v9::tokens::{palette, spacing, TextRole};
    let height = (92.0 + data.party_popularity.len().max(1) as f32 * 25.0).max(188.0);
    v9_card(ui, height, |ui, inner| {
        ui.painter().text(
            Pos2::new(inner.left(), inner.top()),
            egui::Align2::LEFT_TOP,
            tr("party_popularity"),
            TextRole::Heading.font_id(),
            palette::BRASS_BRIGHT,
        );
        let content = Rect::from_min_max(
            Pos2::new(inner.left(), inner.top() + 34.0),
            inner.right_bottom(),
        );
        let donut_size = content
            .height()
            .min(content.width() * 0.42)
            .clamp(108.0, 148.0);
        let donut_rect = Rect::from_min_size(
            Pos2::new(
                content.left(),
                content.top() + (content.height() - donut_size) * 0.5,
            ),
            Vec2::splat(donut_size),
        );
        draw_v9_ideology_donut(ui, donut_rect, &data.party_popularity);

        let legend_left = donut_rect.right() + spacing::S5;
        let legend = Rect::from_min_max(
            Pos2::new(legend_left, content.top()),
            content.right_bottom(),
        );
        let mut y = legend.top();
        for (key, pop) in &data.party_popularity {
            let color = v9_ideology_color(key);
            let row =
                Rect::from_min_size(Pos2::new(legend.left(), y), Vec2::new(legend.width(), 22.0));
            ui.painter().rect_filled(
                Rect::from_min_size(
                    Pos2::new(row.left(), row.center().y - 5.0),
                    Vec2::splat(10.0),
                ),
                1.0,
                color,
            );
            let text_clip = Rect::from_min_max(
                Pos2::new(row.left() + 16.0, row.top()),
                Pos2::new(row.right() - 48.0, row.bottom()),
            );
            ui.painter().text(
                Pos2::new(text_clip.left(), row.center().y),
                egui::Align2::LEFT_CENTER,
                ideology_label(key),
                TextRole::Body.font_id(),
                palette::PARCHMENT,
            );
            ui.painter().text(
                Pos2::new(row.right(), row.center().y),
                egui::Align2::RIGHT_CENTER,
                format!("{:.0}%", pop * 100.0),
                TextRole::Numeric.font_id(),
                color,
            );
            let bar = Rect::from_min_size(
                Pos2::new(row.left() + 16.0, row.bottom() - 3.0),
                Vec2::new((row.width() - 64.0).max(32.0), 3.0),
            );
            ui.painter().rect_filled(bar, 1.0, palette::SOOT_BLACK);
            ui.painter().rect_filled(
                Rect::from_min_size(
                    bar.min,
                    Vec2::new(bar.width() * pop.clamp(0.0, 1.0), bar.height()),
                ),
                1.0,
                color,
            );
            y += spacing::S7;
        }
    });
}

fn draw_v9_ideology_donut(ui: &mut egui::Ui, rect: Rect, popularity: &[(String, f32)]) {
    use crate::v9::tokens::palette;

    let center = rect.center();
    let radius = rect.width().min(rect.height()) * 0.5;
    let total: f32 = popularity.iter().map(|(_, pop)| pop.max(0.0)).sum();
    if total <= f32::EPSILON {
        ui.painter().circle_filled(center, radius, palette::IRON);
        ui.painter()
            .circle_filled(center, radius * 0.55, palette::SOOT_BLACK);
        ui.painter()
            .circle_stroke(center, radius, egui::Stroke::new(1.0, palette::EDGE_DARK));
        return;
    }

    let mut start = -std::f32::consts::FRAC_PI_2;
    for (key, pop) in popularity {
        let frac = (pop.max(0.0) / total).clamp(0.0, 1.0);
        if frac <= 0.0 {
            continue;
        }
        let sweep = frac * std::f32::consts::TAU;
        let steps = ((sweep / std::f32::consts::TAU) * 80.0).ceil().max(4.0) as usize;
        let mut points = Vec::with_capacity(steps + 2);
        points.push(center);
        for i in 0..=steps {
            let t = start + sweep * (i as f32 / steps as f32);
            points.push(Pos2::new(
                center.x + t.cos() * radius,
                center.y + t.sin() * radius,
            ));
        }
        ui.painter().add(egui::Shape::convex_polygon(
            points,
            v9_ideology_color(key),
            egui::Stroke::new(0.0, Color32::TRANSPARENT),
        ));
        start += sweep;
    }
    ui.painter()
        .circle_filled(center, radius * 0.55, palette::SOOT_BLACK);
    ui.painter()
        .circle_stroke(center, radius, egui::Stroke::new(2.0, palette::EDGE_DARK));
    ui.painter().circle_stroke(
        center,
        radius * 0.55,
        egui::Stroke::new(1.0, palette::BRASS_DARK),
    );
}

fn v9_ideas_card(ui: &mut egui::Ui, data: &PoliticsData, icon_bank: &mut crate::icons::IconBank) {
    use crate::v9::{
        primitives::PortraitFrame,
        tokens::{palette, spacing, TextRole},
    };
    let content_w = (ui.available_width().max(360.0) - 32.0).max(IDEA_SLOT_SIZE);
    let cols = (((content_w + spacing::S4) / (54.0 + spacing::S4)).floor() as usize).clamp(1, 8);
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

fn v9_government_card(ui: &mut egui::Ui, data: &PoliticsData) {
    use crate::v9::tokens::{palette, spacing, TextRole};

    let rows = data.government_posts.len().max(1);
    let height = 58.0 + rows as f32 * 48.0;
    v9_card(ui, height, |ui, inner| {
        ui.painter().text(
            Pos2::new(inner.left(), inner.top()),
            egui::Align2::LEFT_TOP,
            "政府",
            TextRole::Heading.font_id(),
            palette::BRASS_BRIGHT,
        );
        if data.government_posts.is_empty() {
            draw_v9_empty_text(ui, inner, "暂无政府职位数据");
            return;
        }
        let mut y = inner.top() + 34.0;
        for (idx, post) in data.government_posts.iter().enumerate() {
            let row =
                Rect::from_min_size(Pos2::new(inner.left(), y), Vec2::new(inner.width(), 40.0));
            let accent = if idx == 0 {
                v9_ideology_color(&data.ruling_party)
            } else {
                palette::BRASS_BRIGHT
            };
            draw_v9_detail_row(ui, row, &post.office, &post.name, &post.detail, accent);
            y += 40.0 + spacing::S3;
        }
    });
}

fn v9_law_systems_card(ui: &mut egui::Ui, data: &PoliticsData) {
    use crate::v9::tokens::{palette, spacing, TextRole};

    let rows = data.law_slots.len().max(1);
    let height = 58.0 + rows as f32 * 48.0;
    v9_card(ui, height, |ui, inner| {
        ui.painter().text(
            Pos2::new(inner.left(), inner.top()),
            egui::Align2::LEFT_TOP,
            "法律与制度",
            TextRole::Heading.font_id(),
            palette::BRASS_BRIGHT,
        );
        if data.law_slots.is_empty() {
            draw_v9_empty_text(ui, inner, "暂无法律制度数据");
            return;
        }
        let mut y = inner.top() + 34.0;
        for slot in &data.law_slots {
            let row =
                Rect::from_min_size(Pos2::new(inner.left(), y), Vec2::new(inner.width(), 40.0));
            let accent = politics_law_category_color(&slot.category);
            let status = politics_law_status_text(slot);
            draw_v9_law_row(ui, row, slot, &status, accent);
            y += 40.0 + spacing::S3;
        }
    });
}

fn draw_v9_detail_row(
    ui: &mut egui::Ui,
    rect: Rect,
    label: &str,
    value: &str,
    detail: &str,
    accent: Color32,
) {
    use crate::v9::tokens::{palette, spacing, TextRole};

    crate::v9::paint::paint_bevel(
        ui.painter(),
        rect,
        palette::SOOT_BLACK,
        palette::EDGE_DARK,
        2.0,
    );
    ui.painter().rect_filled(
        Rect::from_min_size(rect.left_top(), Vec2::new(3.0, rect.height())),
        0.0,
        accent,
    );
    let icon = Rect::from_min_size(
        Pos2::new(rect.left() + spacing::S3, rect.top() + spacing::S3),
        Vec2::splat(28.0),
    );
    crate::v9::paint::paint_bevel(ui.painter(), icon, palette::IRON_DARK, accent, 2.0);
    let glyph = label.chars().next().unwrap_or('?').to_string();
    ui.painter().text(
        icon.center(),
        egui::Align2::CENTER_CENTER,
        glyph,
        TextRole::Subheading.font_id(),
        accent,
    );
    let text = Rect::from_min_max(
        Pos2::new(icon.right() + spacing::S4, rect.top() + spacing::S2),
        Pos2::new(rect.right() - spacing::S4, rect.bottom() - spacing::S2),
    );
    let painter = ui.painter().with_clip_rect(text);
    painter.text(
        Pos2::new(text.left(), text.top()),
        egui::Align2::LEFT_TOP,
        label,
        TextRole::Caption.font_id(),
        palette::PARCHMENT_DIM,
    );
    painter.text(
        Pos2::new(text.left(), text.top() + 14.0),
        egui::Align2::LEFT_TOP,
        value,
        TextRole::Body.font_id(),
        palette::PARCHMENT,
    );
    if !detail.is_empty() {
        painter.text(
            Pos2::new(text.right(), text.top() + 14.0),
            egui::Align2::RIGHT_TOP,
            detail,
            TextRole::Caption.font_id(),
            accent,
        );
    }
}

fn draw_v9_law_row(
    ui: &mut egui::Ui,
    rect: Rect,
    slot: &PoliticsLawEntry,
    status: &str,
    accent: Color32,
) {
    use crate::v9::tokens::{palette, spacing, TextRole};

    crate::v9::paint::paint_bevel(
        ui.painter(),
        rect,
        palette::SOOT_BLACK,
        palette::EDGE_DARK,
        2.0,
    );
    ui.painter().rect_filled(
        Rect::from_min_size(rect.left_top(), Vec2::new(3.0, rect.height())),
        0.0,
        accent,
    );
    let icon = Rect::from_min_size(
        Pos2::new(rect.left() + spacing::S3, rect.top() + spacing::S3),
        Vec2::splat(28.0),
    );
    crate::v9::paint::paint_bevel(
        ui.painter(),
        icon,
        palette::IRON_DARK,
        palette::EDGE_DARK,
        2.0,
    );
    draw_panel_svg_icon(
        ui,
        icon.shrink(6.0),
        PanelSvgIcon::from_law(&slot.category),
        palette::PARCHMENT_DIM,
    );
    let text = Rect::from_min_max(
        Pos2::new(icon.right() + spacing::S4, rect.top() + spacing::S2),
        Pos2::new(rect.right() - spacing::S4, rect.bottom() - spacing::S2),
    );
    let painter = ui.painter().with_clip_rect(text);
    painter.text(
        Pos2::new(text.left(), text.top()),
        egui::Align2::LEFT_TOP,
        politics_law_category_label(&slot.category),
        TextRole::Caption.font_id(),
        palette::PARCHMENT_DIM,
    );
    painter.text(
        Pos2::new(text.left(), text.top() + 14.0),
        egui::Align2::LEFT_TOP,
        slot.current_name.as_str(),
        TextRole::Body.font_id(),
        palette::PARCHMENT,
    );
    painter.text(
        Pos2::new(text.right(), text.top() + 14.0),
        egui::Align2::RIGHT_TOP,
        status,
        TextRole::Caption.font_id(),
        politics_law_status_color(slot),
    );
}

fn draw_v9_empty_text(ui: &mut egui::Ui, inner: Rect, text: &str) {
    ui.painter().text(
        inner.center(),
        egui::Align2::CENTER_CENTER,
        text,
        crate::v9::TextRole::Body.font_id(),
        crate::v9::palette::MUTED,
    );
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
    hoi4_ideology_color(key)
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

fn politics_law_category_label(category: &LawCategory) -> &'static str {
    match category {
        LawCategory::Conscription => tr("v6_law_conscription"),
        LawCategory::Economy => tr("v6_law_economy"),
        LawCategory::Trade => tr("v6_law_trade"),
        LawCategory::Taxation => tr("v6_law_taxation"),
        LawCategory::CivilRights => tr("v6_law_civil_rights"),
        LawCategory::InformationControl => tr("v6_law_information_control"),
    }
}

fn politics_law_category_glyph(category: &LawCategory) -> &'static str {
    match category {
        LawCategory::Conscription => "C",
        LawCategory::Economy => "E",
        LawCategory::Trade => "T",
        LawCategory::Taxation => "$",
        LawCategory::CivilRights => "R",
        LawCategory::InformationControl => "I",
    }
}

fn politics_law_category_color(category: &LawCategory) -> Color32 {
    use crate::v9::palette;
    match category {
        LawCategory::Conscription => palette::IDEO_FASCISM,
        LawCategory::Economy => palette::INFO,
        LawCategory::Trade => palette::GOOD,
        LawCategory::Taxation => palette::GOLD,
        LawCategory::CivilRights => palette::COLD_ATOMIC,
        LawCategory::InformationControl => palette::BAD,
    }
}

fn politics_law_status_text(slot: &PoliticsLawEntry) -> String {
    if slot.is_locked {
        tr("locked").to_owned()
    } else if let Some((target, days)) = &slot.pending {
        format!("{}: {} / {}天", tr("v6_law_pending"), target, days)
    } else if slot.cooldown_days > 0 {
        format!("{} {}天", tr("cooldown"), slot.cooldown_days)
    } else {
        tr("current").to_owned()
    }
}

fn politics_law_status_color(slot: &PoliticsLawEntry) -> Color32 {
    use crate::v9::palette;
    if slot.is_locked {
        palette::BAD
    } else if slot.pending.is_some() || slot.cooldown_days > 0 {
        palette::WARN
    } else {
        palette::GOOD
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
