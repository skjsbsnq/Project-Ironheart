//! V5 阶段 G.5：1940-12-31 战役结束界面。
//!
//! 当游戏日期推进到 1940-12-31 时，自动暂停并展示战役统计：
//!
//! - 已完成 focus 数（带累计天数估计）
//! - V6 建筑总数（工业 + 军工 + 造船）
//! - 师数（owned divisions）
//! - 占领的省数（玩家 controller / non-original-owner）
//! - 战争 / 阵营 / 紧张度数据
//!
//! 一致的「数据 + UI 解耦」架构：
//!
//! - [`EndStats`] 是纯数据快照，由 caller（main.rs）从 World / EconomyState
//!   / DiplomacyState 拉取
//! - [`EndScreen`] 是 egui modal，仅负责渲染 + 触发 [`EndCommand`]
//!
//! 触发条件：[`EndScreen::should_trigger`]，caller 每日 tick 后判断。

use crate::egui;
use crate::i18n::tr;

/// G.5：战役结束日期（GameDate 的 `(year, month, day)`）。
pub const END_YEAR: u16 = 1940;
pub const END_MONTH: u8 = 12;
pub const END_DAY: u8 = 31;

/// G.5：战役统计数据。所有字段都是从 caller 注入。
#[derive(Debug, Clone, Default)]
pub struct EndStats {
    /// 玩家国家 tag（用于标题显示）。
    pub player_tag: String,
    /// 玩家国家显示名（本地化）。
    pub player_name: String,
    /// 战役结束日期字符串（"December 31, 1940"）。
    pub date_str: String,
    /// 已完成的焦点 / 国策数。
    pub focuses_completed: u32,
    /// 民用工厂数。
    pub civ_factories: u32,
    /// 军用工厂数。
    pub mil_factories: u32,
    /// 造船建筑数。
    pub shipyards: u32,
    /// 师数（玩家拥有，未解散）。
    pub divisions: u32,
    /// 占领的省数（玩家是 controller，但不是 original owner）。
    pub provinces_occupied: u32,
    /// 自身战争的对手国 tag 列表。
    pub at_war_with: Vec<String>,
    /// 玩家所在阵营名（如果有）。
    pub faction_name: Option<String>,
    /// 阵营成员 tag 列表。
    pub faction_members: Vec<String>,
    /// 已完成科技数。
    pub techs_researched: u32,
    /// 当前世界紧张度 0..=1。
    pub world_tension: f32,
}

impl EndStats {
    /// G.5：所有工厂总数。
    pub fn total_factories(&self) -> u32 {
        self.civ_factories + self.mil_factories + self.shipyards
    }
}

/// G.5：副作用命令。
#[derive(Debug, Clone, PartialEq)]
pub enum EndCommand {
    /// 玩家选择「Continue Playing」 — 关闭 modal 但游戏继续（沙盒模式）。
    Continue,
    /// 玩家选择「Return to Main Menu」 — caller 切到主菜单 + reset world。
    ReturnToMainMenu,
    /// 玩家选择「Quit」 — caller event_loop.exit()。
    Quit,
}

/// G.5：结束界面状态。`triggered` 在 caller 第一次调用 [`EndScreen::trigger`]
/// 后变 true，之后每帧都画 modal。`continued` 在玩家点 Continue 后变 true，
/// 阻止下一帧重新触发。
pub struct EndScreen {
    pub triggered: bool,
    pub continued: bool,
    pub stats: EndStats,
}

impl Default for EndScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl EndScreen {
    pub fn new() -> Self {
        Self {
            triggered: false,
            continued: false,
            stats: EndStats::default(),
        }
    }

    /// G.5：判断是否到结束日期 / 之后。`(year, month, day)` 是 [`hoi4_state::GameDate`]
    /// 字段；caller 每日 tick 后传入。
    pub fn should_trigger(year: u16, month: u8, day: u8) -> bool {
        (year, month, day) >= (END_YEAR, END_MONTH, END_DAY)
    }

    /// G.5：触发结束界面。仅在第一次到达终点日期且 `continued = false` 时调用。
    pub fn trigger(&mut self, stats: EndStats) {
        self.stats = stats;
        self.triggered = true;
    }

    fn show_v9(&mut self, ctx: &egui::Context) -> Option<EndCommand> {
        if !self.triggered || self.continued {
            return None;
        }
        use crate::v9::layout::{GridLayout, Track};
        use crate::v9::primitives::{Button, ButtonSize, ButtonVariant, Modal};
        use crate::v9::tokens::{palette, spacing, TextRole};
        use egui::{Align2, Pos2, Rect, Vec2};

        let screen = ctx.screen_rect();
        let size = Vec2::new(
            720.0_f32.min(screen.width() * 0.86),
            600.0_f32.min(screen.height() * 0.86),
        );
        let title = tr("campaign_complete");
        let output = Modal::new("end_screen_modal_v9", size)
            .title(title)
            .accent(palette::GOLD_HOT)
            .show(ctx, |ui, body| {
                let mut cmd = None;
                let grid = GridLayout::new(
                    vec![Track::Fixed(62.0), Track::Fr(1.0), Track::Fixed(44.0)],
                    vec![Track::Fr(1.0)],
                )
                .with_gutter(0.0, spacing::S5);
                let cells = grid.measure(body);
                let header = GridLayout::cell(&cells, 0, 0);
                let stats_rect = GridLayout::cell(&cells, 1, 0);
                let actions = GridLayout::cell(&cells, 2, 0);

                ui.painter().text(
                    header.left_top(),
                    Align2::LEFT_TOP,
                    format!("{} - {}", self.stats.player_name, self.stats.date_str),
                    TextRole::Heading.font_id(),
                    palette::PARCHMENT,
                );
                ui.painter().text(
                    Pos2::new(header.left(), header.top() + 26.0),
                    Align2::LEFT_TOP,
                    format!("Player: {}", self.stats.player_tag),
                    TextRole::Caption.font_id(),
                    palette::PARCHMENT_DIM,
                );
                ui.painter().text(
                    Pos2::new(header.right(), header.top() + 26.0),
                    Align2::RIGHT_TOP,
                    format!(
                        "{} {:.0}%",
                        tr("world_tension"),
                        self.stats.world_tension * 100.0
                    ),
                    TextRole::Caption.font_id(),
                    if self.stats.world_tension >= 0.5 {
                        palette::WARN
                    } else {
                        palette::INFO
                    },
                );

                let stats_grid = GridLayout::new(
                    vec![Track::Fr(1.0), Track::Fr(1.0)],
                    vec![Track::Fr(1.0), Track::Fr(1.0)],
                )
                .with_gutter(spacing::S5, spacing::S5);
                let stat_cells = stats_grid.measure(stats_rect);
                v9_end_card(
                    ui,
                    GridLayout::cell(&stat_cells, 0, 0),
                    tr("politics"),
                    &[(
                        tr("focuses_completed"),
                        self.stats.focuses_completed.to_string(),
                    )],
                );
                v9_end_card(
                    ui,
                    GridLayout::cell(&stat_cells, 0, 1),
                    tr("production"),
                    &[
                        (tr("civ_factories"), self.stats.civ_factories.to_string()),
                        (tr("mil_factories"), self.stats.mil_factories.to_string()),
                        (tr("shipyards"), self.stats.shipyards.to_string()),
                        (tr("total"), self.stats.total_factories().to_string()),
                    ],
                );
                v9_end_card(
                    ui,
                    GridLayout::cell(&stat_cells, 1, 0),
                    tr("military"),
                    &[
                        (tr("divisions_list"), self.stats.divisions.to_string()),
                        (
                            tr("provinces_occupied"),
                            self.stats.provinces_occupied.to_string(),
                        ),
                        (
                            tr("techs_researched"),
                            self.stats.techs_researched.to_string(),
                        ),
                    ],
                );
                let faction = self
                    .stats
                    .faction_name
                    .clone()
                    .unwrap_or_else(|| "-".to_owned());
                let wars = if self.stats.at_war_with.is_empty() {
                    "-".to_owned()
                } else {
                    self.stats.at_war_with.join(", ")
                };
                v9_end_card(
                    ui,
                    GridLayout::cell(&stat_cells, 1, 1),
                    tr("diplomacy"),
                    &[
                        (tr("faction"), faction),
                        (
                            tr("faction_members"),
                            self.stats.faction_members.len().to_string(),
                        ),
                        (tr("at_war_with"), wars),
                    ],
                );

                let button_w = 168.0;
                let gap = spacing::S4;
                let total_w = button_w * 3.0 + gap * 2.0;
                let start_x = actions.center().x - total_w * 0.5;
                if Button::new(tr("continue_playing"))
                    .size(ButtonSize::Md)
                    .variant(ButtonVariant::Primary)
                    .show_at(
                        ui,
                        Rect::from_min_size(
                            Pos2::new(start_x, actions.top() + 4.0),
                            Vec2::new(button_w, 32.0),
                        ),
                    )
                    .clicked()
                {
                    cmd = Some(EndCommand::Continue);
                }
                if Button::new(tr("main_menu"))
                    .size(ButtonSize::Md)
                    .variant(ButtonVariant::Secondary)
                    .show_at(
                        ui,
                        Rect::from_min_size(
                            Pos2::new(start_x + button_w + gap, actions.top() + 4.0),
                            Vec2::new(button_w, 32.0),
                        ),
                    )
                    .clicked()
                {
                    cmd = Some(EndCommand::ReturnToMainMenu);
                }
                if Button::new(tr("quit"))
                    .size(ButtonSize::Md)
                    .variant(ButtonVariant::Danger)
                    .show_at(
                        ui,
                        Rect::from_min_size(
                            Pos2::new(start_x + (button_w + gap) * 2.0, actions.top() + 4.0),
                            Vec2::new(button_w, 32.0),
                        ),
                    )
                    .clicked()
                {
                    cmd = Some(EndCommand::Quit);
                }
                cmd
            })
            .flatten();

        if matches!(output, Some(EndCommand::Continue)) {
            self.continued = true;
        }
        output
    }

    /// 画 modal。返回命令（如有）。`triggered = false` 或 `continued = true` 时
    /// no-op（不画 modal）。
    #[allow(unreachable_code)]
    pub fn show(&mut self, ctx: &egui::Context) -> Option<EndCommand> {
        return self.show_v9(ctx);

        if !self.triggered || self.continued {
            return None;
        }
        let mut cmd: Option<EndCommand> = None;

        // modal 用 Area + 屏幕中央 fixed_pos，加 collapsible/movable/resizable=false。
        let screen = ctx.screen_rect();
        let modal_w = 560.0_f32;
        let modal_h = 540.0_f32;
        let pos = egui::pos2(
            (screen.width() - modal_w) * 0.5,
            (screen.height() - modal_h) * 0.5,
        );

        // 半透明遮罩 — 用一个全屏 Area + 暗色矩形。
        egui::Area::new(egui::Id::new("end_screen_overlay"))
            .order(egui::Order::Background)
            .fixed_pos(egui::pos2(0.0, 0.0))
            .show(ctx, |ui| {
                let painter = ui.painter();
                painter.rect_filled(screen, 0.0, egui::Color32::from_black_alpha(160));
            });

        egui::Window::new(tr("campaign_complete"))
            .id(egui::Id::new("end_screen_modal"))
            .fixed_pos(pos)
            .default_size([modal_w, modal_h])
            .collapsible(false)
            .movable(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.heading(format!(
                    "{} — {}",
                    self.stats.player_name, self.stats.date_str
                ));
                ui.label(format!("Player: {}", self.stats.player_tag));
                ui.separator();

                ui.heading(tr("politics"));
                stat_row(
                    ui,
                    tr("focuses_completed"),
                    &self.stats.focuses_completed.to_string(),
                );
                ui.separator();

                ui.heading(tr("production"));
                stat_row(
                    ui,
                    tr("civ_factories"),
                    &self.stats.civ_factories.to_string(),
                );
                stat_row(
                    ui,
                    tr("mil_factories"),
                    &self.stats.mil_factories.to_string(),
                );
                stat_row(ui, tr("shipyards"), &self.stats.shipyards.to_string());
                stat_row(ui, tr("total"), &self.stats.total_factories().to_string());
                ui.separator();

                ui.heading(tr("military"));
                stat_row(ui, tr("divisions_list"), &self.stats.divisions.to_string());
                stat_row(
                    ui,
                    tr("provinces_occupied"),
                    &self.stats.provinces_occupied.to_string(),
                );
                ui.separator();

                ui.heading(tr("research"));
                stat_row(
                    ui,
                    tr("techs_researched"),
                    &self.stats.techs_researched.to_string(),
                );
                ui.separator();

                ui.heading(tr("diplomacy"));
                stat_row(
                    ui,
                    tr("world_tension"),
                    &format!("{:.0}%", self.stats.world_tension * 100.0),
                );
                if let Some(f) = &self.stats.faction_name {
                    stat_row(
                        ui,
                        tr("faction"),
                        &format!(
                            "{} ({} {})",
                            f,
                            self.stats.faction_members.len(),
                            tr("faction_members")
                        ),
                    );
                } else {
                    stat_row(ui, tr("faction"), "—");
                }
                if self.stats.at_war_with.is_empty() {
                    stat_row(ui, tr("at_war_with"), "—");
                } else {
                    stat_row(ui, tr("at_war_with"), &self.stats.at_war_with.join(", "));
                }
                ui.separator();

                ui.horizontal(|ui| {
                    if ui.button(tr("continue_playing")).clicked() {
                        cmd = Some(EndCommand::Continue);
                        self.continued = true;
                    }
                    if ui.button(tr("main_menu")).clicked() {
                        cmd = Some(EndCommand::ReturnToMainMenu);
                    }
                    if ui.button(tr("quit")).clicked() {
                        cmd = Some(EndCommand::Quit);
                    }
                });
            });

        cmd
    }
}

fn stat_row(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(label);
        let avail = ui.available_width();
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(avail.max(0.0).min(4.0));
            ui.strong(value);
        });
    });
}

fn v9_end_card(ui: &mut egui::Ui, rect: egui::Rect, title: &str, rows: &[(&str, String)]) {
    use crate::v9::primitives::Card;
    use crate::v9::tokens::{palette, spacing, TextRole};
    use egui::{Align2, Pos2};

    let inner = Card::new().as_panel().show_at(ui, rect);
    ui.painter().text(
        inner.left_top(),
        Align2::LEFT_TOP,
        title,
        TextRole::Subheading.font_id(),
        palette::BRASS_BRIGHT,
    );
    let mut y = inner.top() + spacing::S7;
    for (label, value) in rows {
        ui.painter().text(
            Pos2::new(inner.left(), y),
            Align2::LEFT_TOP,
            *label,
            TextRole::Caption.font_id(),
            palette::MUTED,
        );
        ui.painter().text(
            Pos2::new(inner.right(), y),
            Align2::RIGHT_TOP,
            value,
            TextRole::Numeric.font_id(),
            palette::PARCHMENT,
        );
        y += 24.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn end_year_constants() {
        assert_eq!(END_YEAR, 1940);
        assert_eq!(END_MONTH, 12);
        assert_eq!(END_DAY, 31);
    }

    #[test]
    fn should_trigger_at_or_after_end() {
        assert!(EndScreen::should_trigger(1940, 12, 31));
        assert!(EndScreen::should_trigger(1941, 1, 1));
        assert!(EndScreen::should_trigger(1945, 5, 8));
    }

    #[test]
    fn should_not_trigger_before_end() {
        assert!(!EndScreen::should_trigger(1940, 12, 30));
        assert!(!EndScreen::should_trigger(1940, 11, 31));
        assert!(!EndScreen::should_trigger(1939, 9, 1));
        assert!(!EndScreen::should_trigger(1936, 1, 1));
    }

    #[test]
    fn end_screen_starts_inert() {
        let s = EndScreen::new();
        assert!(!s.triggered);
        assert!(!s.continued);
    }

    #[test]
    fn trigger_sets_state() {
        let mut s = EndScreen::new();
        let stats = EndStats {
            player_tag: "GER".into(),
            focuses_completed: 7,
            civ_factories: 30,
            ..Default::default()
        };
        s.trigger(stats);
        assert!(s.triggered);
        assert_eq!(s.stats.player_tag, "GER");
    }

    #[test]
    fn total_factories_sums_three_types() {
        let st = EndStats {
            civ_factories: 40,
            mil_factories: 30,
            shipyards: 10,
            ..Default::default()
        };
        assert_eq!(st.total_factories(), 80);
    }
}
