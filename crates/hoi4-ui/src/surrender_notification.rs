//! P1.1：投降与和平结算通知弹窗。
//!
//! 当战争一方全部投降、或每日和平自动结算产生结果时，
//! 本面板向玩家弹出一个 modal 通知，暂停游戏直到玩家确认。
//!
//! ## 行为
//!
//! - 收到通知队列后，逐个显示居中 modal。
//! - 每条通知显示：战争双方标签、结算类型（投降/和平/空战争清理）、结果（吞并/傀儡/割地/白和）。
//! - 玩家点击「确认」后关闭当前通知，队列非空时显示下一条。
//! - 通知全部确认后恢复游戏速度。

use crate::i18n::tr;

/// 单条投降/和平通知。
#[derive(Debug, Clone)]
pub struct SurrenderNotification {
    /// 投降/被结算方标签（如 "AUS"）
    pub target_tag: String,
    pub target_name: String,
    /// 胜利方标签（如 "GER"）
    pub winner_tag: String,
    pub winner_name: String,
    /// 结算类型
    pub kind: SurrenderKind,
    /// 具体结果列表
    pub results: Vec<SurrenderResultEntry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurrenderKind {
    /// 脚本投降（scripted surrender）
    ScriptedSurrender,
    /// 每日和平自动结算（一方全部投降）
    AutoPeaceConference,
    /// 空战争清理（双方全部出局）
    EmptyWarCleanup,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurrenderResultKind {
    Annexed,
    Puppeted,
    StateTransferred,
    GovernmentToppled,
    WhitePeace,
    WarRemoved,
}

#[derive(Debug, Clone)]
pub struct SurrenderResultEntry {
    pub kind: SurrenderResultKind,
    pub target_tag: String,
    pub target_name: String,
    pub winner_tag: String,
    pub winner_name: String,
    pub detail: String,
}

/// modal 给 caller 的命令。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SurrenderNotificationCommand {
    /// 玩家确认当前通知
    Acknowledge,
}

fn show_surrender_notification_v9(
    ctx: &egui::Context,
    notification: &SurrenderNotification,
    queue_remaining: usize,
    index: usize,
) -> Option<SurrenderNotificationCommand> {
    use crate::v9::primitives::{Button, ButtonSize, ButtonVariant, Card, Modal};
    use crate::v9::tokens::{palette, spacing, TextRole};
    use egui::{Align2, Pos2, Rect, Sense, Vec2};

    let kind_label = surrender_kind_label(notification.kind);
    let title = surrender_title(notification, queue_remaining, kind_label);
    let screen = ctx.screen_rect();
    let size = Vec2::new(
        600.0_f32.min(screen.width() * 0.86),
        430.0_f32.min(screen.height() * 0.82),
    );

    Modal::new(("surrender_notification_v9", index), size)
        .title(&title)
        .accent(palette::BAD)
        .show(ctx, |ui, body| {
            let mut cmd = None;
            let header_rect = Rect::from_min_size(body.min, Vec2::new(body.width(), 74.0));
            let results_rect = Rect::from_min_max(
                Pos2::new(body.left(), header_rect.bottom() + spacing::S5),
                Pos2::new(body.right(), body.bottom() - 48.0),
            );
            let actions_rect = Rect::from_min_max(
                Pos2::new(body.left(), body.bottom() - 36.0),
                body.right_bottom(),
            );

            let header_inner = Card::new().as_panel().show_at(ui, header_rect);
            let summary = if notification.winner_tag.is_empty() {
                format!("{} {}", notification.target_name, tr("surrendered"))
            } else {
                format!(
                    "{} {} {} {}",
                    notification.winner_name,
                    tr("defeated"),
                    notification.target_name,
                    tr("surrender_long")
                )
            };
            ui.painter().text(
                header_inner.left_top(),
                Align2::LEFT_TOP,
                summary,
                TextRole::Heading.font_id(),
                palette::GOLD_HOT,
            );
            ui.painter().text(
                Pos2::new(header_inner.left(), header_inner.top() + 28.0),
                Align2::LEFT_TOP,
                format!(
                    "{}: {}  |  {} -> {}",
                    kind_label,
                    notification.results.len(),
                    notification.target_name,
                    notification.winner_name
                ),
                TextRole::Caption.font_id(),
                palette::PARCHMENT_DIM,
            );

            let results_inner = Card::new().as_panel().show_at(ui, results_rect);
            ui.painter().text(
                results_inner.left_top(),
                Align2::LEFT_TOP,
                tr("peace_results"),
                TextRole::Subheading.font_id(),
                palette::BRASS_BRIGHT,
            );
            let list_rect = Rect::from_min_max(
                Pos2::new(results_inner.left(), results_inner.top() + spacing::S7),
                results_inner.right_bottom(),
            );
            ui.allocate_ui_at_rect(list_rect, |ui| {
                ui.set_width(list_rect.width());
                if notification.results.is_empty() {
                    ui.colored_label(palette::MUTED, tr("war_concluded_no_territorial_change"));
                    return;
                }
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for (row_idx, entry) in notification.results.iter().enumerate() {
                            let (row, _) = ui.allocate_exact_size(
                                Vec2::new(ui.available_width(), 34.0),
                                Sense::hover(),
                            );
                            let fill = if row_idx % 2 == 0 {
                                palette::ZEBRA_DARK
                            } else {
                                palette::ZEBRA_LIGHT
                            };
                            ui.painter()
                                .rect_filled(row, egui::epaint::CornerRadius::ZERO, fill);
                            let color = surrender_result_color(entry.kind);
                            let label = surrender_result_label(entry.kind);
                            ui.painter().text(
                                Pos2::new(row.left() + spacing::S3, row.center().y),
                                Align2::LEFT_CENTER,
                                label,
                                TextRole::Caption.font_id(),
                                color,
                            );
                            ui.painter().text(
                                Pos2::new(row.left() + 96.0, row.center().y),
                                Align2::LEFT_CENTER,
                                format!("{} -> {}", entry.target_name, entry.winner_name),
                                TextRole::Code.font_id(),
                                palette::PARCHMENT_DIM,
                            );
                            ui.painter().text(
                                Pos2::new(row.left() + 194.0, row.center().y),
                                Align2::LEFT_CENTER,
                                &entry.detail,
                                TextRole::Body.font_id(),
                                color,
                            );
                        }
                    });
            });

            let button_rect = Rect::from_center_size(
                Pos2::new(actions_rect.center().x, actions_rect.center().y),
                Vec2::new(200.0, 32.0),
            );
            if Button::new(tr("acknowledge"))
                .size(ButtonSize::Md)
                .variant(ButtonVariant::Primary)
                .show_at(ui, button_rect)
                .clicked()
            {
                cmd = Some(SurrenderNotificationCommand::Acknowledge);
            }
            cmd
        })
        .flatten()
}

fn surrender_kind_label(kind: SurrenderKind) -> &'static str {
    match kind {
        SurrenderKind::ScriptedSurrender => "投降",
        SurrenderKind::AutoPeaceConference => "和平会议",
        SurrenderKind::EmptyWarCleanup => "战争结束",
    }
}

fn surrender_title(
    notification: &SurrenderNotification,
    queue_remaining: usize,
    kind_label: &str,
) -> String {
    if queue_remaining > 0 {
        if notification.target_tag.is_empty() {
            format!(
                "{}  (+{} {})",
                kind_label,
                queue_remaining,
                tr("more_events")
            )
        } else {
            format!(
                "{}: {} {}  (+{} {})",
                kind_label,
                notification.target_name,
                tr("surrendered"),
                queue_remaining,
                tr("more_events")
            )
        }
    } else if notification.target_tag.is_empty() {
        kind_label.to_owned()
    } else {
        format!(
            "{}: {} {}",
            kind_label,
            notification.target_name,
            tr("surrendered")
        )
    }
}

fn surrender_result_label(kind: SurrenderResultKind) -> &'static str {
    match kind {
        SurrenderResultKind::Annexed => "吞并",
        SurrenderResultKind::Puppeted => "傀儡",
        SurrenderResultKind::StateTransferred => "割让州",
        SurrenderResultKind::GovernmentToppled => "推翻政府",
        SurrenderResultKind::WhitePeace => "无条件停战",
        SurrenderResultKind::WarRemoved => "战争移除",
    }
}

fn surrender_result_color(kind: SurrenderResultKind) -> egui::Color32 {
    use crate::v9::tokens::palette;

    match kind {
        SurrenderResultKind::Annexed => palette::BAD,
        SurrenderResultKind::Puppeted => palette::INFO,
        SurrenderResultKind::StateTransferred => palette::GOLD,
        SurrenderResultKind::GovernmentToppled => palette::WARN,
        SurrenderResultKind::WhitePeace => palette::GOOD,
        SurrenderResultKind::WarRemoved => palette::MUTED,
    }
}

/// 显示投降/和平通知 modal。
///
/// - `ctx`: egui Context
/// - `notification`: 当前要显示的通知（来自队列前端）
///
/// 返回 `Some(Acknowledge)` 表示玩家已确认；`None` = 未交互。
pub fn show_surrender_notification(
    ctx: &egui::Context,
    notification: &SurrenderNotification,
    queue_remaining: usize,
    index: usize,
) -> Option<SurrenderNotificationCommand> {
    show_surrender_notification_v9(ctx, notification, queue_remaining, index)
}
