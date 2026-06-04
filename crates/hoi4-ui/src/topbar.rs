//! V9 in-game top bar.
//!
//! The bar is intentionally painted as a continuous HUD fixture: blackened
//! steel backplate, brass rails, recessed stat cells, an inset flag frame, and
//! a single date/speed control cluster. egui is only used for hit testing and
//! text/texture painting.

use crate::{
    i18n::{current_language, tr, Language},
    icons::IconBank,
    v9::{layout::snap_rect, paint, palette, spacing, TextRole},
};
use egui::{Align2, Color32, Pos2, Rect, Sense, Stroke, StrokeKind, TopBottomPanel, Ui, Vec2};

const TOPBAR_HEIGHT: f32 = 82.0;
const STAT_COUNT: usize = 7;
const SPEED_COUNT: usize = 6;
const GBP: &str = "\u{00a3}";

/// Speed change command emitted by the top bar. The caller writes it back to
/// `World.speed`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpeedCommand {
    Pause,
    Speed1,
    Speed2,
    Speed3,
    Speed4,
    Speed5,
}

/// Read-only top bar snapshot for one frame.
#[derive(Debug, Clone, PartialEq)]
pub struct TopBarData {
    pub country_tag: String,
    pub country_name: String,
    pub ruling_party: String,
    pub political_power: f32,
    pub stability: f32,
    pub war_support: f32,
    pub manpower: u64,
    pub gdp_gbp: f64,
    pub gdp_growth_yoy: f32,
    pub construction_points: f32,
    pub date: String,
    pub date_year: u16,
    pub date_month: u8,
    pub date_day: u8,
    pub speed_index: u8, // 0=paused, 1-5=speed
}

struct StatTile {
    key: &'static str,
    value: String,
    tooltip_key: &'static str,
}

/// Stateless top bar renderer.
pub struct TopBar;

impl TopBar {
    pub fn show(
        ctx: &egui::Context,
        data: &TopBarData,
        icon_bank: &mut IconBank,
    ) -> (Option<SpeedCommand>, bool) {
        let mut cmd: Option<SpeedCommand> = None;
        let resource_clicked = false;

        TopBottomPanel::top("topbar_v9")
            .exact_height(TOPBAR_HEIGHT)
            .frame(
                egui::Frame::new()
                    .fill(Color32::TRANSPARENT)
                    .inner_margin(egui::Margin::same(0)),
            )
            .show(ctx, |ui| {
                let full = snap_rect(ui.max_rect(), ui.ctx().pixels_per_point());
                paint_topbar_backplate(ui, full);

                let content = full.shrink2(Vec2::new(spacing::S4, spacing::S3));
                let metrics = TopBarMetrics::measure(content.width());
                let row = Rect::from_min_max(
                    Pos2::new(content.left(), content.top() + 2.0),
                    Pos2::new(content.right(), content.bottom() - 4.0),
                );

                let left_group = Rect::from_min_size(
                    row.left_top(),
                    Vec2::new(metrics.left_group_w, row.height()),
                );
                let gear_rect = Rect::from_min_max(
                    Pos2::new(row.right() - metrics.gear_w, row.top() + 3.0),
                    Pos2::new(row.right(), row.bottom() - 3.0),
                );
                let controls_x = (left_group.right() + metrics.group_gap)
                    .min(gear_rect.left() - metrics.controls_w - spacing::S5)
                    .max(left_group.right() + spacing::S4);
                let controls = Rect::from_min_size(
                    Pos2::new(controls_x, row.top() + 4.0),
                    Vec2::new(metrics.controls_w, row.height() - 8.0),
                );

                paint_slanted_bridge(
                    ui,
                    Rect::from_min_max(
                        Pos2::new(left_group.right() - 2.0, row.top() + 5.0),
                        Pos2::new(controls.left() + 7.0, row.bottom() - 5.0),
                    ),
                );
                paint_center_rail(
                    ui,
                    Rect::from_min_max(
                        Pos2::new(controls.right() + spacing::S3, row.top() + 8.0),
                        Pos2::new(gear_rect.left() - spacing::S3, row.bottom() - 8.0),
                    ),
                );

                paint_group_frame(ui, left_group, palette::BRASS_DARK, true);
                paint_group_frame(ui, controls, palette::BRASS_DARK, false);

                let flag_rect = Rect::from_min_max(
                    Pos2::new(left_group.left() + 8.0, left_group.top() + 5.0),
                    Pos2::new(
                        left_group.left() + metrics.flag_w - 9.0,
                        left_group.bottom() - 5.0,
                    ),
                );
                Self::flag_plate_at(ui, flag_rect, data, icon_bank);

                let stats = Self::stats(data);
                let stats_left = left_group.left() + metrics.flag_w;
                paint_flag_separator(ui, stats_left, left_group);
                for (idx, stat) in stats.iter().enumerate() {
                    let x = stats_left + idx as f32 * metrics.stat_w;
                    let rect = Rect::from_min_max(
                        Pos2::new(x, left_group.top() + 6.0),
                        Pos2::new(x + metrics.stat_w, left_group.bottom() - 6.0),
                    );
                    Self::stat_tile_at(ui, rect, idx, stat);
                }

                Self::date_block_at(
                    ui,
                    Rect::from_min_max(
                        Pos2::new(controls.left() + 8.0, controls.top() + 7.0),
                        Pos2::new(
                            controls.left() + 8.0 + metrics.date_w,
                            controls.bottom() - 7.0,
                        ),
                    ),
                    Self::fmt_date(data.date_year, data.date_month, data.date_day),
                );

                let speed_start = controls.left() + 8.0 + metrics.date_w + metrics.speed_gap;
                for (idx, speed) in [
                    SpeedCommand::Pause,
                    SpeedCommand::Speed1,
                    SpeedCommand::Speed2,
                    SpeedCommand::Speed3,
                    SpeedCommand::Speed4,
                    SpeedCommand::Speed5,
                ]
                .iter()
                .enumerate()
                {
                    let x = speed_start + idx as f32 * (metrics.speed_w + metrics.speed_gap);
                    let rect = Rect::from_min_size(
                        Pos2::new(x, controls.top() + 7.0),
                        Vec2::new(metrics.speed_w, controls.height() - 14.0),
                    );
                    if speed_chip_at(ui, rect, idx as u8, idx as u8 == data.speed_index) {
                        cmd = Some(*speed);
                    }
                }

                gear_button_at(ui, gear_rect);
            });

        (cmd, resource_clicked)
    }

    fn stats(data: &TopBarData) -> [StatTile; STAT_COUNT] {
        [
            StatTile {
                key: "political_power",
                value: format!("{:.0}", data.political_power),
                tooltip_key: "tooltip_political_power",
            },
            StatTile {
                key: "stability",
                value: format!("{:.0}%", data.stability * 100.0),
                tooltip_key: "tooltip_stability",
            },
            StatTile {
                key: "war_support",
                value: format!("{:.0}%", data.war_support * 100.0),
                tooltip_key: "tooltip_war_support",
            },
            StatTile {
                key: "manpower",
                value: Self::fmt_manpower(data.manpower),
                tooltip_key: "tooltip_manpower",
            },
            StatTile {
                key: "gdp",
                value: Self::fmt_gbp_short(data.gdp_gbp),
                tooltip_key: "tooltip_gdp",
            },
            StatTile {
                key: "gdp_growth",
                value: Self::fmt_pct_signed(data.gdp_growth_yoy),
                tooltip_key: "tooltip_gdp_growth",
            },
            StatTile {
                key: "construction_points",
                value: format!("{:.0}", data.construction_points),
                tooltip_key: "tooltip_construction_points",
            },
        ]
    }

    fn flag_plate_at(ui: &mut Ui, rect: Rect, data: &TopBarData, icon_bank: &mut IconBank) {
        if !rect.is_positive() {
            return;
        }

        let resp = ui.interact(rect, ui.id().with("topbar_flag"), Sense::hover());

        let pixel = ui.ctx().pixels_per_point();
        let aperture = flag_aperture(rect, pixel);
        let frame = snap_rect(aperture.expand2(Vec2::new(4.0, 3.0)), pixel);
        let shadow = snap_rect(frame.expand2(Vec2::new(3.0, 3.0)), pixel);

        let painter = ui.painter().clone();
        paint::paint_recessed_panel(&painter, shadow, 1.0);
        paint::paint_plate_grain(&painter, shadow.shrink(3.0), 3.0, 2);
        paint_topbar_border(&painter, frame, 1.0);

        let flag_inner = aperture;
        painter.rect_filled(flag_inner.expand(1.0), corner(0.0), palette::OIL_BLACK);

        let gfx_name = if data.ruling_party.is_empty() {
            format!("GFX_flag_{}", data.country_tag)
        } else {
            format!("GFX_flag_{}_{}", data.country_tag, data.ruling_party)
        };

        if !data.country_tag.is_empty() {
            if let Some(handle) = icon_bank.get_or_load(&gfx_name) {
                let img = egui::Image::from_texture(handle)
                    .fit_to_exact_size(flag_inner.size())
                    .sense(Sense::hover());
                ui.put(flag_inner, img);
            } else {
                painter.text(
                    flag_inner.center(),
                    Align2::CENTER_CENTER,
                    &data.country_name,
                    TextRole::Heading.font_id(),
                    muted_value_color(),
                );
            }
        }

        paint_flag_integration(&painter, flag_inner, resp.hovered());
        painter.rect_stroke(
            flag_inner,
            corner(0.0),
            Stroke::new(2.0, Color32::from_black_alpha(235)),
            StrokeKind::Inside,
        );
        painter.rect_stroke(
            flag_inner.expand(1.0),
            corner(1.0),
            Stroke::new(1.0, translucent(palette::BRASS_DARK, 70)),
            StrokeKind::Inside,
        );
        resp.on_hover_text(format!("{} {}", data.country_name, data.ruling_party));
    }

    fn stat_tile_at(ui: &mut Ui, rect: Rect, idx: usize, stat: &StatTile) {
        if !rect.is_positive() {
            return;
        }

        let resp = ui.interact(rect, ui.id().with(("topbar_stat", idx)), Sense::hover());
        let painter = ui.painter();
        let cell = rect.shrink2(Vec2::new(3.0, 4.0));

        if resp.hovered() {
            painter.rect_filled(cell, corner(1.0), Color32::from_white_alpha(6));
            painter.hline(
                (cell.left() + 7.0)..=(cell.right() - 7.0),
                cell.bottom() - 2.0,
                Stroke::new(1.0, translucent(palette::BRASS_WORN, 105)),
            );
        } else {
            painter.rect_filled(cell, corner(1.0), Color32::from_black_alpha(28));
        }

        painter.rect_stroke(
            cell,
            corner(1.0),
            Stroke::new(1.0, Color32::from_black_alpha(205)),
            StrokeKind::Inside,
        );
        painter.hline(
            (cell.left() + 6.0)..=(cell.right() - 6.0),
            cell.top() + 1.0,
            Stroke::new(1.0, Color32::from_white_alpha(5)),
        );
        painter.hline(
            (cell.left() + 6.0)..=(cell.right() - 6.0),
            cell.bottom() - 1.0,
            Stroke::new(1.0, Color32::from_black_alpha(220)),
        );

        painter.text(
            Pos2::new(rect.center().x, rect.top() + 10.0),
            Align2::CENTER_TOP,
            topbar_label(stat.key),
            TextRole::Caption.font_id(),
            palette::PARCHMENT_DIM,
        );
        painter.text(
            Pos2::new(rect.center().x, rect.bottom() - 9.0),
            Align2::CENTER_BOTTOM,
            &stat.value,
            value_font(&stat.value, rect.width()),
            if resp.hovered() {
                palette::PARCHMENT
            } else {
                muted_value_color()
            },
        );

        draw_vertical_divider(ui, rect.right(), rect);
        resp.on_hover_text(tr(stat.tooltip_key));
    }

    fn date_block_at(ui: &mut Ui, rect: Rect, text: String) {
        if !rect.is_positive() {
            return;
        }

        let resp = ui.interact(rect, ui.id().with("topbar_date"), Sense::hover());
        let painter = ui.painter();
        paint::paint_recessed_panel(painter, rect, 1.0);
        paint_topbar_border(painter, rect, 1.0);
        painter.hline(
            (rect.left() + 8.0)..=(rect.right() - 8.0),
            rect.top() + 4.0,
            Stroke::new(1.0, translucent(palette::BRASS_DARK, 50)),
        );
        painter.hline(
            (rect.left() + 8.0)..=(rect.right() - 8.0),
            rect.bottom() - 4.0,
            Stroke::new(1.0, Color32::from_black_alpha(230)),
        );
        paint::paint_slot_screw(painter, rect.left_center() + Vec2::new(9.0, 0.0), 2.0);
        paint::paint_slot_screw(painter, rect.right_center() - Vec2::new(9.0, 0.0), 2.0);
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            text,
            TextRole::Subheading.font_id(),
            palette::PARCHMENT,
        );
        resp.on_hover_text(tr("date"));
    }

    fn fmt_manpower(m: u64) -> String {
        if m >= 1_000_000 {
            format!("{:.2}M", m as f64 / 1_000_000.0)
        } else if m >= 1_000 {
            format!("{:.1}K", m as f64 / 1_000.0)
        } else {
            m.to_string()
        }
    }

    fn fmt_gbp_short(value: f64) -> String {
        let abs = value.abs();
        if abs >= 1_000_000_000.0 {
            format!("{}{:.1}B", GBP, value / 1_000_000_000.0)
        } else if abs >= 1_000_000.0 {
            format!("{}{:.0}M", GBP, value / 1_000_000.0)
        } else if abs >= 1_000.0 {
            format!("{}{:.0}K", GBP, value / 1_000.0)
        } else {
            format!("{}{:.0}", GBP, value)
        }
    }

    fn fmt_pct_signed(value: f32) -> String {
        format!("{:+.1}%", value)
    }

    fn fmt_date(year: u16, month: u8, day: u8) -> String {
        if current_language() == Language::Chinese {
            return format!("{}年{}月{}日", year, month, day);
        }

        let month_key = match month {
            1 => "month_january",
            2 => "month_february",
            3 => "month_march",
            4 => "month_april",
            5 => "month_may",
            6 => "month_june",
            7 => "month_july",
            8 => "month_august",
            9 => "month_september",
            10 => "month_october",
            11 => "month_november",
            12 => "month_december",
            _ => "month_january",
        };
        format!("{} {}, {}", tr(month_key), day, year)
    }
}

#[derive(Debug, Clone, Copy)]
struct TopBarMetrics {
    flag_w: f32,
    stat_w: f32,
    left_group_w: f32,
    controls_w: f32,
    date_w: f32,
    speed_w: f32,
    speed_gap: f32,
    gear_w: f32,
    group_gap: f32,
}

impl TopBarMetrics {
    fn measure(width: f32) -> Self {
        let wide = width >= 1600.0;
        let flag_w = if wide { 124.0 } else { 108.0 };
        let date_w = if wide { 184.0 } else { 154.0 };
        let speed_w = if wide { 44.0 } else { 38.0 };
        let speed_gap = if wide { 6.0 } else { 4.0 };
        let gear_w = if wide { 50.0 } else { 42.0 };
        let group_gap = if wide { 26.0 } else { 18.0 };
        let controls_w =
            16.0 + date_w + speed_gap + SPEED_COUNT as f32 * speed_w + 5.0 * speed_gap + 6.0;

        let stat_budget = width - flag_w - controls_w - gear_w - group_gap - spacing::S10;
        let max_stat = if wide { 98.0 } else { 88.0 };
        let stat_w = (stat_budget / STAT_COUNT as f32).clamp(74.0, max_stat);
        let left_group_w = flag_w + STAT_COUNT as f32 * stat_w + 6.0;

        Self {
            flag_w,
            stat_w,
            left_group_w,
            controls_w,
            date_w,
            speed_w,
            speed_gap,
            gear_w,
            group_gap,
        }
    }
}

fn speed_chip_at(ui: &mut Ui, rect: Rect, speed: u8, active: bool) -> bool {
    if !rect.is_positive() {
        return false;
    }

    let resp = ui.interact(rect, ui.id().with(("topbar_speed", speed)), Sense::click());
    let painter = ui.painter();
    let fill = if active {
        palette::IRON
    } else if resp.hovered() {
        palette::IRON_DARK
    } else {
        palette::SOOT_BLACK
    };
    let stroke_color = if active || resp.hovered() {
        translucent(palette::BRASS_WORN, 115)
    } else {
        palette::EDGE_DARK
    };
    let glyph_color = if active {
        palette::PARCHMENT
    } else if resp.hovered() {
        muted_value_color()
    } else {
        palette::PARCHMENT_DIM
    };

    paint::paint_bevel(painter, rect, fill, stroke_color, 1.0);
    painter.rect_stroke(
        rect.shrink(3.0),
        corner(1.0),
        Stroke::new(1.0, Color32::from_black_alpha(220)),
        StrokeKind::Inside,
    );
    if active {
        painter.hline(
            (rect.left() + 7.0)..=(rect.right() - 7.0),
            rect.top() + 4.0,
            Stroke::new(1.0, translucent(palette::BRASS_WORN, 130)),
        );
        painter.hline(
            (rect.left() + 7.0)..=(rect.right() - 7.0),
            rect.bottom() - 4.0,
            Stroke::new(1.0, translucent(palette::BRASS_DARK, 125)),
        );
    }

    paint_speed_glyph(ui, rect, speed, glyph_color);

    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp.clone().on_hover_text(if speed == 0 {
        tr("paused").to_owned()
    } else {
        format!("{} {}", tr("speed"), speed)
    });
    resp.clicked()
}

fn paint_speed_glyph(ui: &mut Ui, rect: Rect, speed: u8, color: Color32) {
    let painter = ui.painter();
    let center = rect.center();
    if speed == 0 {
        let bar_w = (rect.width() * 0.085).clamp(3.0, 4.2);
        let bar_h = (rect.height() * 0.42).clamp(14.0, 18.0);
        for x in [-5.0, 5.0] {
            let bar = Rect::from_center_size(center + Vec2::new(x, 0.0), Vec2::new(bar_w, bar_h));
            painter.rect_filled(bar, 0.0, color);
        }
        return;
    }

    let count = speed.min(3);
    let tri_w = (rect.width() * 0.22).clamp(7.0, 9.5);
    let tri_h = (rect.height() * 0.36).clamp(12.0, 15.0);
    let gap = 3.0;
    let total_w = count as f32 * tri_w + (count.saturating_sub(1)) as f32 * gap;
    let start_x = center.x - total_w * 0.5;
    for i in 0..count {
        let x = start_x + i as f32 * (tri_w + gap);
        let points = vec![
            Pos2::new(x, center.y - tri_h * 0.5),
            Pos2::new(x, center.y + tri_h * 0.5),
            Pos2::new(x + tri_w, center.y),
        ];
        painter.add(egui::Shape::convex_polygon(points, color, Stroke::NONE));
    }

    if speed > 3 {
        painter.text(
            rect.right_bottom() - Vec2::new(8.0, 7.0),
            Align2::CENTER_CENTER,
            speed.to_string(),
            TextRole::Small.font_id(),
            color,
        );
    }
}

fn gear_button_at(ui: &mut Ui, rect: Rect) {
    if !rect.is_positive() {
        return;
    }

    let resp = ui.interact(rect, ui.id().with("topbar_gear"), Sense::hover());
    let painter = ui.painter();
    paint::paint_bevel(
        painter,
        rect,
        if resp.hovered() {
            palette::IRON_DARK
        } else {
            palette::SOOT_BLACK
        },
        if resp.hovered() {
            palette::BRASS_WORN
        } else {
            palette::EDGE_DARK
        },
        1.0,
    );
    paint_gear_glyph(
        painter,
        rect.center(),
        rect.height().min(rect.width()) * 0.23,
    );
    resp.on_hover_text(tr("settings_title"));
}

fn paint_gear_glyph(painter: &egui::Painter, center: Pos2, radius: f32) {
    let color = palette::PARCHMENT_DIM;
    for i in 0..8 {
        let a = i as f32 * std::f32::consts::TAU / 8.0;
        let dir = Vec2::angled(a);
        painter.line_segment(
            [
                center + dir * (radius * 0.85),
                center + dir * (radius * 1.22),
            ],
            Stroke::new(2.0, color),
        );
    }
    painter.circle_stroke(center, radius, Stroke::new(2.0, color));
    painter.circle_stroke(center, radius * 0.42, Stroke::new(1.5, color));
}

fn paint_topbar_backplate(ui: &mut Ui, rect: Rect) {
    let painter = ui.painter();
    painter.rect_filled(rect, corner(0.0), palette::IRON_BLACK);
    paint::paint_multistop_gradient(
        painter,
        rect,
        &[
            (0.00, palette::IRON_LIGHT),
            (0.05, palette::IRON_DARK),
            (0.30, palette::IRON_BLACK),
            (0.72, palette::OIL_BLACK),
            (1.00, palette::SOOT_BLACK),
        ],
    );
    paint::paint_burnished_steel_surface(painter, rect.shrink(2.0), 0.18);
    paint::paint_plate_grain(painter, rect.shrink(1.0), 3.0, 4);
    paint::paint_speckle(painter, rect.shrink(3.0), 80, 3);

    let top_rail = Rect::from_min_max(rect.left_top(), Pos2::new(rect.right(), rect.top() + 8.0));
    paint::paint_vertical_gradient_mesh(
        painter,
        top_rail,
        Color32::from_white_alpha(9),
        Color32::from_black_alpha(230),
    );
    let bottom_shadow = Rect::from_min_max(
        Pos2::new(rect.left(), rect.bottom() - 7.0),
        rect.right_bottom(),
    );
    paint::paint_vertical_gradient_mesh(
        painter,
        bottom_shadow,
        Color32::from_black_alpha(20),
        Color32::from_black_alpha(242),
    );

    painter.hline(
        rect.left()..=rect.right(),
        rect.top() + 1.0,
        Stroke::new(1.0, Color32::from_white_alpha(18)),
    );
    painter.hline(
        rect.left()..=rect.right(),
        rect.top() + 6.0,
        Stroke::new(1.0, translucent(palette::BRASS_DARK, 36)),
    );
    painter.hline(
        rect.left()..=rect.right(),
        rect.bottom() - 4.0,
        Stroke::new(1.0, translucent(palette::BRASS_SHADOW, 95)),
    );
    painter.hline(
        rect.left()..=rect.right(),
        rect.bottom() - 1.0,
        Stroke::new(1.0, Color32::from_black_alpha(245)),
    );
}

fn paint_group_frame(ui: &mut Ui, rect: Rect, accent: Color32, heavy: bool) {
    if !rect.is_positive() {
        return;
    }

    let painter = ui.painter();
    paint::paint_blackened_steel(painter, rect, 1.0);
    paint_topbar_border(painter, rect, 1.0);
    if heavy {
        paint::paint_corner_caps(painter, rect);
        paint::paint_rivets(painter, rect);
    } else {
        paint::paint_slot_screw(painter, rect.left_top() + Vec2::new(8.0, 8.0), 2.2);
        paint::paint_slot_screw(painter, rect.right_top() + Vec2::new(-8.0, 8.0), 2.2);
        paint::paint_slot_screw(painter, rect.left_bottom() + Vec2::new(8.0, -8.0), 2.2);
        paint::paint_slot_screw(painter, rect.right_bottom() + Vec2::new(-8.0, -8.0), 2.2);
    }
    painter.rect_stroke(
        rect.shrink(5.0),
        corner(1.0),
        Stroke::new(1.0, Color32::from_black_alpha(225)),
        StrokeKind::Inside,
    );
    painter.hline(
        (rect.left() + 14.0)..=(rect.right() - 14.0),
        rect.top() + 4.0,
        Stroke::new(1.0, translucent(accent, 48)),
    );
}

fn paint_topbar_border(painter: &egui::Painter, rect: Rect, corner_radius: f32) {
    let cr = corner(corner_radius);
    painter.rect_stroke(
        rect.translate(Vec2::new(1.0, 1.0)),
        cr,
        Stroke::new(2.0, Color32::from_black_alpha(220)),
        StrokeKind::Inside,
    );
    painter.rect_stroke(
        rect,
        cr,
        Stroke::new(2.0, palette::EDGE_DARK),
        StrokeKind::Inside,
    );
    painter.rect_stroke(
        rect.shrink(1.0),
        corner((corner_radius - 1.0).max(0.0)),
        Stroke::new(1.0, translucent(palette::GUNMETAL, 150)),
        StrokeKind::Inside,
    );
    painter.rect_stroke(
        rect.shrink(2.0),
        corner((corner_radius - 1.0).max(0.0)),
        Stroke::new(1.0, Color32::from_black_alpha(230)),
        StrokeKind::Inside,
    );
    painter.hline(
        (rect.left() + 10.0)..=(rect.right() - 10.0),
        rect.top() + 2.0,
        Stroke::new(1.0, Color32::from_white_alpha(5)),
    );
    painter.hline(
        (rect.left() + 10.0)..=(rect.right() - 10.0),
        rect.top() + 3.0,
        Stroke::new(1.0, translucent(palette::BRASS_DARK, 38)),
    );
    painter.hline(
        (rect.left() + 10.0)..=(rect.right() - 10.0),
        rect.bottom() - 3.0,
        Stroke::new(1.0, Color32::from_black_alpha(232)),
    );
}

fn paint_slanted_bridge(ui: &mut Ui, rect: Rect) {
    if rect.width() <= 14.0 || rect.height() <= 12.0 {
        return;
    }

    let painter = ui.painter();
    let cut = (rect.height() * 0.36).min(rect.width() * 0.35);
    let points = vec![
        rect.left_top(),
        Pos2::new(rect.right() - cut, rect.top()),
        rect.right_bottom(),
        Pos2::new(rect.left() + cut, rect.bottom()),
    ];
    painter.add(egui::Shape::convex_polygon(
        points,
        palette::IRON_BLACK,
        Stroke::new(1.0, palette::EDGE_DARK),
    ));
    paint::paint_plate_grain(painter, rect.shrink(3.0), 4.0, 2);
    painter.line_segment(
        [
            Pos2::new(rect.left() + 4.0, rect.top() + 2.0),
            Pos2::new(rect.right() - cut, rect.top() + 2.0),
        ],
        Stroke::new(1.0, Color32::from_white_alpha(5)),
    );
    painter.line_segment(
        [
            Pos2::new(rect.left() + cut, rect.bottom() - 2.0),
            Pos2::new(rect.right() - 4.0, rect.bottom() - 2.0),
        ],
        Stroke::new(1.0, Color32::from_black_alpha(220)),
    );
}

fn paint_center_rail(ui: &mut Ui, rect: Rect) {
    if rect.width() <= 16.0 || rect.height() <= 12.0 {
        return;
    }

    let painter = ui.painter();
    paint::paint_bevel(painter, rect, palette::SOOT_BLACK, palette::EDGE_DARK, 1.0);
    painter.hline(
        (rect.left() + 8.0)..=(rect.right() - 8.0),
        rect.top() + 3.0,
        Stroke::new(1.0, Color32::from_white_alpha(4)),
    );
    painter.hline(
        (rect.left() + 8.0)..=(rect.right() - 8.0),
        rect.bottom() - 3.0,
        Stroke::new(1.0, Color32::from_black_alpha(230)),
    );
    let step = 52.0;
    let mut x = rect.left() + 24.0;
    while x < rect.right() - 18.0 {
        paint::paint_slot_screw(painter, Pos2::new(x, rect.center().y), 1.8);
        x += step;
    }
}

fn paint_flag_separator(ui: &mut Ui, x: f32, group: Rect) {
    let painter = ui.painter();
    painter.line_segment(
        [
            Pos2::new(x, group.top() + 6.0),
            Pos2::new(x, group.bottom() - 6.0),
        ],
        Stroke::new(2.0, Color32::from_black_alpha(230)),
    );
    painter.line_segment(
        [
            Pos2::new(x + 1.0, group.top() + 10.0),
            Pos2::new(x + 1.0, group.bottom() - 10.0),
        ],
        Stroke::new(1.0, Color32::from_white_alpha(8)),
    );
}

fn draw_vertical_divider(ui: &mut Ui, x: f32, rect: Rect) {
    let painter = ui.painter();
    painter.line_segment(
        [
            Pos2::new(x, rect.top() + 4.0),
            Pos2::new(x, rect.bottom() - 4.0),
        ],
        Stroke::new(1.0, Color32::from_black_alpha(220)),
    );
    painter.line_segment(
        [
            Pos2::new(x + 1.0, rect.top() + 8.0),
            Pos2::new(x + 1.0, rect.bottom() - 8.0),
        ],
        Stroke::new(1.0, Color32::from_white_alpha(4)),
    );
}

fn flag_aperture(bounds: Rect, pixels_per_point: f32) -> Rect {
    let available = bounds.shrink2(Vec2::new(5.0, 3.0));
    let aspect = 82.0 / 52.0;
    let mut w = available.width();
    let mut h = w / aspect;
    if h > available.height() {
        h = available.height();
        w = h * aspect;
    }

    let size = Vec2::new(
        (w * pixels_per_point).floor(),
        (h * pixels_per_point).floor(),
    ) / pixels_per_point;
    snap_rect(
        Rect::from_center_size(available.center(), size),
        pixels_per_point,
    )
}

fn paint_flag_integration(painter: &egui::Painter, rect: Rect, hovered: bool) {
    let top_shadow = Rect::from_min_max(rect.left_top(), Pos2::new(rect.right(), rect.top() + 5.0));
    paint::paint_vertical_gradient_mesh(
        painter,
        top_shadow,
        Color32::from_black_alpha(125),
        Color32::from_black_alpha(0),
    );

    let bottom_shadow = Rect::from_min_max(
        Pos2::new(rect.left(), rect.bottom() - 6.0),
        rect.right_bottom(),
    );
    paint::paint_vertical_gradient_mesh(
        painter,
        bottom_shadow,
        Color32::from_black_alpha(0),
        Color32::from_black_alpha(150),
    );

    let left_shadow =
        Rect::from_min_max(rect.left_top(), Pos2::new(rect.left() + 6.0, rect.bottom()));
    paint::paint_horizontal_gradient_mesh(
        painter,
        left_shadow,
        Color32::from_black_alpha(120),
        Color32::from_black_alpha(0),
    );

    let right_shadow = Rect::from_min_max(
        Pos2::new(rect.right() - 6.0, rect.top()),
        rect.right_bottom(),
    );
    paint::paint_horizontal_gradient_mesh(
        painter,
        right_shadow,
        Color32::from_black_alpha(0),
        Color32::from_black_alpha(120),
    );

    painter.rect_filled(
        rect,
        corner(0.0),
        if hovered {
            Color32::from_black_alpha(18)
        } else {
            Color32::from_black_alpha(34)
        },
    );
    painter.hline(
        (rect.left() + 3.0)..=(rect.right() - 3.0),
        rect.top() + 1.0,
        Stroke::new(1.0, Color32::from_white_alpha(12)),
    );
    painter.hline(
        (rect.left() + 3.0)..=(rect.right() - 3.0),
        rect.bottom() - 1.0,
        Stroke::new(1.0, Color32::from_black_alpha(205)),
    );
}

fn value_font(value: &str, width: f32) -> egui::FontId {
    if width < 80.0 || value.chars().count() > 7 {
        TextRole::Subheading.font_id()
    } else {
        TextRole::Heading.font_id()
    }
}

fn muted_value_color() -> Color32 {
    Color32::from_rgb(0xc7, 0xba, 0x8f)
}

fn translucent(color: Color32, alpha: u8) -> Color32 {
    Color32::from_rgba_premultiplied(color.r(), color.g(), color.b(), alpha)
}

fn topbar_label(key: &str) -> &'static str {
    match (current_language(), key) {
        (Language::Chinese, "political_power") => "\u{653f}\u{6cbb}\u{529b}",
        (Language::Chinese, "stability") => "\u{7a33}\u{5b9a}\u{5ea6}",
        (Language::Chinese, "war_support") => "\u{6218}\u{4e89}\u{652f}\u{6301}",
        (Language::Chinese, "manpower") => "\u{4eba}\u{529b}",
        (Language::Chinese, "gdp") => "GDP",
        (Language::Chinese, "gdp_growth") => "GDP \u{589e}\u{5e45}",
        (Language::Chinese, "construction_points") => "\u{5efa}\u{9020}\u{529b}",
        (_, "political_power") => "POL",
        (_, "stability") => "Stability",
        (_, "war_support") => "War",
        (_, "manpower") => "Manpower",
        (_, "gdp") => "GDP",
        (_, "gdp_growth") => "GDP +",
        (_, "construction_points") => "Build",
        _ => "Stat",
    }
}

fn corner(r: f32) -> egui::epaint::CornerRadius {
    egui::epaint::CornerRadius::same(r as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fmt_manpower_millions() {
        assert_eq!(TopBar::fmt_manpower(5_230_000), "5.23M");
    }

    #[test]
    fn fmt_manpower_thousands() {
        assert_eq!(TopBar::fmt_manpower(42_500), "42.5K");
    }

    #[test]
    fn fmt_gbp_billions() {
        assert_eq!(TopBar::fmt_gbp_short(12_300_000_000.0), "\u{00a3}12.3B");
    }

    #[test]
    fn fmt_pct_signed_positive() {
        assert_eq!(TopBar::fmt_pct_signed(3.25), "+3.2%");
    }

    #[test]
    fn metrics_fit_1280() {
        let m = TopBarMetrics::measure(1264.0);
        assert!(m.stat_w >= 74.0);
        assert!(m.left_group_w + m.controls_w + m.gear_w < 1264.0);
    }
}
