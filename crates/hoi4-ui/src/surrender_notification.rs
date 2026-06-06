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

fn show_surrender_notification_iron(
    ctx: &egui::Context,
    notification: &SurrenderNotification,
    queue_remaining: usize,
    index: usize,
) -> Option<SurrenderNotificationCommand> {
    use crate::{
        v9::{
            text::fit_font_to_width,
            tokens::{spacing, TextRole},
        },
        vanilla_iron::VanillaIron,
    };
    use egui::{Align2, Area, Color32, Order, Pos2, Rect, Sense, Stroke, Vec2};

    let kind_label = surrender_kind_label(notification.kind);
    let title = surrender_title(notification, queue_remaining, kind_label);
    let screen = ctx.screen_rect();
    let size = Vec2::new(
        720.0_f32.min(screen.width() * 0.86),
        460.0_f32.min(screen.height() * 0.82),
    );
    let pos = Pos2::new(
        screen.center().x - size.x * 0.5,
        screen.center().y - size.y * 0.5,
    );
    let accent = surrender_result_color(SurrenderResultKind::Annexed);
    let mut cmd = None;

    Area::new(egui::Id::new((
        "surrender_notification_iron_backdrop",
        index,
    )))
    .order(Order::Foreground)
    .fixed_pos(screen.min)
    .show(ctx, |ui| {
        let (rect, _) = ui.allocate_exact_size(screen.size(), Sense::click());
        ui.painter().rect_filled(
            rect,
            egui::epaint::CornerRadius::ZERO,
            Color32::from_black_alpha(126),
        );
    });

    Area::new(egui::Id::new(("surrender_notification_iron", index)))
        .order(Order::Foreground)
        .fixed_pos(pos)
        .default_size(size)
        .show(ctx, |ui| {
            let (outer, _) = ui.allocate_exact_size(size, Sense::click_and_drag());
            VanillaIron::paint_panel(ui, outer, accent);

            let inner = outer.shrink2(Vec2::new(14.0, 10.0));
            let header = Rect::from_min_max(
                inner.left_top(),
                Pos2::new(inner.right(), inner.top() + 50.0),
            );
            let title_font = fit_font_to_width(
                &title,
                TextRole::Display.font_id(),
                (header.width() - 48.0).max(1.0),
                0.70,
            );
            ui.painter().with_clip_rect(header).text(
                header.center(),
                Align2::CENTER_CENTER,
                &title,
                title_font,
                VanillaIron::TEXT,
            );
            ui.painter().hline(
                header.left()..=header.right(),
                header.bottom(),
                Stroke::new(1.0, VanillaIron::EDGE),
            );

            let summary_rect = Rect::from_min_max(
                Pos2::new(inner.left(), header.bottom() + spacing::S4),
                Pos2::new(inner.right(), header.bottom() + spacing::S4 + 78.0),
            );
            draw_surrender_summary(ui, summary_rect, notification, kind_label);

            let actions_h = 46.0;
            let results_rect = Rect::from_min_max(
                Pos2::new(inner.left(), summary_rect.bottom() + spacing::S4),
                Pos2::new(inner.right(), inner.bottom() - actions_h - spacing::S4),
            );
            draw_surrender_results(ui, results_rect, notification);

            let actions_rect = Rect::from_min_max(
                Pos2::new(inner.left(), results_rect.bottom() + spacing::S4),
                inner.right_bottom(),
            );
            let button_rect = Rect::from_center_size(actions_rect.center(), Vec2::new(190.0, 30.0));
            if VanillaIron::compact_button_at(
                ui,
                button_rect,
                tr("acknowledge"),
                ("surrender_acknowledge", index),
            )
            .clicked()
            {
                cmd = Some(SurrenderNotificationCommand::Acknowledge);
            }
        });

    cmd
}

fn draw_surrender_summary(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    notification: &SurrenderNotification,
    kind_label: &str,
) {
    use crate::{
        v9::{
            text::fit_font_to_width,
            tokens::{spacing, TextRole},
        },
        vanilla_iron::VanillaIron,
    };
    use egui::{Align2, Pos2};

    VanillaIron::paint_region(ui.painter(), rect, VanillaIron::CARD_SOFT);
    let inner = rect.shrink2(egui::Vec2::new(spacing::S5, spacing::S4));
    let summary = if notification.winner_tag.is_empty() {
        format!("{} {}", notification.target_name, tr("surrendered"))
    } else {
        format!(
            "{} {} {} -- {}",
            notification.winner_name,
            tr("defeated"),
            notification.target_name,
            tr("surrender_long")
        )
    };
    let summary_font =
        fit_font_to_width(&summary, TextRole::Heading.font_id(), inner.width(), 0.72);
    ui.painter().with_clip_rect(inner).text(
        inner.left_top(),
        Align2::LEFT_TOP,
        summary,
        summary_font,
        VanillaIron::BRASS_BRIGHT,
    );

    let winner = if notification.winner_name.is_empty() {
        tr("war_concluded_no_territorial_change").to_owned()
    } else {
        notification.winner_name.clone()
    };
    let meta = format!(
        "{}: {}  |  {} -> {}",
        kind_label,
        notification.results.len(),
        notification.target_name,
        winner
    );
    ui.painter().with_clip_rect(inner).text(
        Pos2::new(inner.left(), inner.bottom() - 4.0),
        Align2::LEFT_BOTTOM,
        &meta,
        fit_font_to_width(&meta, TextRole::Caption.font_id(), inner.width(), 0.70),
        VanillaIron::MUTED,
    );
}

fn draw_surrender_results(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    notification: &SurrenderNotification,
) {
    use crate::{
        v9::tokens::{spacing, TextRole},
        vanilla_iron::VanillaIron,
    };
    use egui::{Align2, Pos2, Rect, Sense, Vec2};

    VanillaIron::paint_region(ui.painter(), rect, VanillaIron::CARD_DEEP);
    let title_rect =
        Rect::from_min_max(rect.left_top(), Pos2::new(rect.right(), rect.top() + 28.0));
    VanillaIron::section_title_at(ui, title_rect, tr("peace_results"));
    let list_rect = Rect::from_min_max(
        Pos2::new(rect.left() + spacing::S4, title_rect.bottom() + spacing::S3),
        rect.right_bottom() - Vec2::new(spacing::S4, spacing::S4),
    );

    ui.allocate_ui_at_rect(list_rect, |ui| {
        ui.set_width(list_rect.width());
        if notification.results.is_empty() {
            ui.painter().text(
                list_rect.center(),
                Align2::CENTER_CENTER,
                tr("war_concluded_no_territorial_change"),
                TextRole::Body.font_id(),
                VanillaIron::MUTED,
            );
            return;
        }

        egui::ScrollArea::vertical()
            .id_salt("surrender_results_iron_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_width(list_rect.width());
                let compact = ui.available_width() < 420.0;
                let row_h = if compact { 58.0 } else { 42.0 };
                for (row_idx, entry) in notification.results.iter().enumerate() {
                    let (row, _) = ui.allocate_exact_size(
                        Vec2::new(ui.available_width(), row_h),
                        Sense::hover(),
                    );
                    draw_surrender_result_row(ui, row, row_idx, entry, compact);
                    ui.add_space(spacing::S2);
                }
            });
    });
}

fn draw_surrender_result_row(
    ui: &mut egui::Ui,
    row: egui::Rect,
    row_idx: usize,
    entry: &SurrenderResultEntry,
    compact: bool,
) {
    use crate::{
        v9::tokens::{spacing, TextRole},
        vanilla_iron::VanillaIron,
    };
    use egui::{Pos2, Rect, Stroke, Vec2};

    let fill = if row_idx % 2 == 0 {
        VanillaIron::CARD
    } else {
        VanillaIron::CARD_SOFT
    };
    ui.painter().rect_filled(row, 1.0, fill);
    ui.painter().rect_stroke(
        row,
        egui::epaint::CornerRadius::same(1),
        Stroke::new(1.0, VanillaIron::EDGE_DARK),
        egui::epaint::StrokeKind::Inside,
    );

    let color = surrender_result_color(entry.kind);
    let label = surrender_result_label(entry.kind);
    let route = if entry.winner_name.is_empty() {
        entry.target_name.clone()
    } else {
        format!("{} -> {}", entry.target_name, entry.winner_name)
    };
    let inner = row.shrink2(Vec2::new(spacing::S4, spacing::S3));

    if compact {
        let top = Rect::from_min_max(
            inner.left_top(),
            Pos2::new(inner.right(), inner.top() + 20.0),
        );
        let label_rect =
            Rect::from_min_max(top.left_top(), Pos2::new(top.left() + 86.0, top.bottom()));
        let route_rect = Rect::from_min_max(
            Pos2::new(label_rect.right() + spacing::S3, top.top()),
            top.right_bottom(),
        );
        draw_clipped_single_line(ui, label_rect, label, TextRole::Caption, color, true);
        draw_clipped_single_line(
            ui,
            route_rect,
            &route,
            TextRole::Code,
            VanillaIron::MUTED,
            true,
        );
        let detail_rect = Rect::from_min_max(
            Pos2::new(inner.left(), top.bottom() + spacing::S1),
            inner.right_bottom(),
        );
        draw_wrapped_text(ui, detail_rect, &entry.detail, TextRole::Body, color);
        return;
    }

    let label_w = 84.0_f32.min(inner.width() * 0.20);
    let route_w = 190.0_f32.min(inner.width() * 0.34).max(118.0);
    let label_rect = Rect::from_min_max(
        inner.left_top(),
        Pos2::new(inner.left() + label_w, inner.bottom()),
    );
    let route_rect = Rect::from_min_max(
        Pos2::new(label_rect.right() + spacing::S4, inner.top()),
        Pos2::new(
            (label_rect.right() + spacing::S4 + route_w).min(inner.right()),
            inner.bottom(),
        ),
    );
    let detail_rect = Rect::from_min_max(
        Pos2::new(route_rect.right() + spacing::S4, inner.top()),
        inner.right_bottom(),
    );

    draw_clipped_single_line(ui, label_rect, label, TextRole::Caption, color, true);
    draw_clipped_single_line(
        ui,
        route_rect,
        &route,
        TextRole::Code,
        VanillaIron::MUTED,
        true,
    );
    draw_wrapped_text(ui, detail_rect, &entry.detail, TextRole::Body, color);
}

fn draw_clipped_single_line(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    text: &str,
    role: crate::v9::TextRole,
    color: egui::Color32,
    left: bool,
) {
    let font = crate::v9::text::fit_font_to_width(text, role.font_id(), rect.width(), 0.66);
    let painter = ui.painter().with_clip_rect(rect);
    let (pos, align) = if left {
        (rect.left_center(), egui::Align2::LEFT_CENTER)
    } else {
        (rect.right_center(), egui::Align2::RIGHT_CENTER)
    };
    painter.text(pos, align, text, font, color);
}

fn draw_wrapped_text(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    text: &str,
    role: crate::v9::TextRole,
    color: egui::Color32,
) {
    if rect.width() <= 1.0 || rect.height() <= 1.0 {
        return;
    }
    let painter = ui.painter().with_clip_rect(rect);
    let galley = painter.layout(text.to_owned(), role.font_id(), color, rect.width());
    let y = (rect.center().y - galley.size().y * 0.5).max(rect.top());
    painter.galley(egui::Pos2::new(rect.left(), y), galley, color);
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
    use crate::vanilla_iron::VanillaIron;

    match kind {
        SurrenderResultKind::Annexed => VanillaIron::BAD,
        SurrenderResultKind::Puppeted => VanillaIron::BRASS_BRIGHT,
        SurrenderResultKind::StateTransferred => VanillaIron::BRASS,
        SurrenderResultKind::GovernmentToppled => VanillaIron::WARN,
        SurrenderResultKind::WhitePeace => VanillaIron::GOOD,
        SurrenderResultKind::WarRemoved => VanillaIron::MUTED,
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
    show_surrender_notification_iron(ctx, notification, queue_remaining, index)
}
