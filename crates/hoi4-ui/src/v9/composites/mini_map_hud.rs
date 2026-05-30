//! V9 bottom-right mini map HUD and secondary time controls.

use egui::{
    Align2, Area, Color32, Context, Id, Order, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2,
};

use crate::{
    topbar::SpeedCommand,
    v9::{
        frame::{FrameStyle, PanelFrame},
        paint,
        primitives::Tooltip,
        sound,
        tokens::{palette, spacing, Elevation, TextRole},
    },
};

const HUD_W: f32 = 336.0;
const HUD_H: f32 = 188.0;
const SPEED_COUNT: usize = 6;

#[derive(Debug, Clone, PartialEq)]
pub struct MiniMapHudData {
    pub date: String,
    pub speed_index: u8,
    pub map_mode: String,
    pub selected_label: Option<String>,
}

pub struct MiniMapHud;

impl MiniMapHud {
    pub fn show(ctx: &Context, data: &MiniMapHudData) -> Option<SpeedCommand> {
        let screen = ctx.screen_rect();
        let size = Vec2::new(HUD_W.min(screen.width() - 24.0), HUD_H);
        let pos = Pos2::new(
            screen.right() - size.x - spacing::S5,
            screen.bottom() - size.y - spacing::S5,
        );
        let mut cmd = None;

        Area::new(Id::new("v9_mini_map_hud"))
            .order(Order::Foreground)
            .fixed_pos(pos)
            .default_size(size)
            .show(ctx, |ui| {
                let (outer, _) = ui.allocate_exact_size(size, Sense::hover());
                PanelFrame::new(FrameStyle::Panel, outer)
                    .with_accent(palette::BRASS_DARK)
                    .with_elevation(Elevation::E2)
                    .draw(ui.painter());

                let inner = outer.shrink2(Vec2::new(spacing::S5, spacing::S4));
                let map_rect = Rect::from_min_max(
                    inner.left_top(),
                    Pos2::new(inner.right(), inner.top() + 104.0),
                );
                draw_mini_map(ui, map_rect, data);

                let strip = Rect::from_min_max(
                    Pos2::new(inner.left(), map_rect.bottom() + spacing::S4),
                    inner.right_bottom(),
                );
                draw_time_strip(ui, strip, data, &mut cmd);
            });

        cmd
    }
}

fn draw_mini_map(ui: &mut egui::Ui, rect: Rect, data: &MiniMapHudData) {
    let painter = ui.painter();
    let response = ui.interact(rect, ui.id().with("v9_mini_map_canvas"), Sense::hover());
    paint::paint_recessed_panel(painter, rect, 1.0);
    painter.rect_stroke(
        rect,
        egui::epaint::CornerRadius::same(1),
        Stroke::new(1.0, palette::EDGE_DARK),
        StrokeKind::Inside,
    );
    paint::paint_plate_grain(painter, rect.shrink(2.0), 3.0, 2);

    let water = rect.shrink2(Vec2::new(7.0, 7.0));
    painter.rect_filled(
        water,
        0.0,
        Color32::from_rgba_premultiplied(18, 27, 31, 120),
    );

    let land_a = vec![
        Pos2::new(
            water.left() + water.width() * 0.08,
            water.top() + water.height() * 0.62,
        ),
        Pos2::new(
            water.left() + water.width() * 0.18,
            water.top() + water.height() * 0.30,
        ),
        Pos2::new(
            water.left() + water.width() * 0.38,
            water.top() + water.height() * 0.22,
        ),
        Pos2::new(
            water.left() + water.width() * 0.54,
            water.top() + water.height() * 0.45,
        ),
        Pos2::new(
            water.left() + water.width() * 0.43,
            water.top() + water.height() * 0.78,
        ),
        Pos2::new(
            water.left() + water.width() * 0.20,
            water.top() + water.height() * 0.80,
        ),
    ];
    let land_b = vec![
        Pos2::new(
            water.left() + water.width() * 0.61,
            water.top() + water.height() * 0.20,
        ),
        Pos2::new(
            water.left() + water.width() * 0.88,
            water.top() + water.height() * 0.28,
        ),
        Pos2::new(
            water.left() + water.width() * 0.93,
            water.top() + water.height() * 0.56,
        ),
        Pos2::new(
            water.left() + water.width() * 0.75,
            water.top() + water.height() * 0.76,
        ),
        Pos2::new(
            water.left() + water.width() * 0.58,
            water.top() + water.height() * 0.58,
        ),
    ];
    painter.add(egui::Shape::convex_polygon(
        land_a,
        Color32::from_rgba_premultiplied(73, 82, 64, 210),
        Stroke::new(1.0, Color32::from_black_alpha(180)),
    ));
    painter.add(egui::Shape::convex_polygon(
        land_b,
        Color32::from_rgba_premultiplied(86, 76, 58, 210),
        Stroke::new(1.0, Color32::from_black_alpha(180)),
    ));

    for i in 1..4 {
        let x = water.left() + water.width() * i as f32 / 4.0;
        painter.line_segment(
            [Pos2::new(x, water.top()), Pos2::new(x, water.bottom())],
            Stroke::new(0.6, Color32::from_black_alpha(80)),
        );
    }
    for i in 1..3 {
        let y = water.top() + water.height() * i as f32 / 3.0;
        painter.hline(
            water.left()..=water.right(),
            y,
            Stroke::new(0.6, Color32::from_black_alpha(80)),
        );
    }

    let viewport = Rect::from_center_size(
        Pos2::new(
            water.left() + water.width() * 0.50,
            water.top() + water.height() * 0.53,
        ),
        Vec2::new(water.width() * 0.34, water.height() * 0.38),
    );
    painter.rect_stroke(
        viewport,
        egui::epaint::CornerRadius::ZERO,
        Stroke::new(1.5, palette::GOLD_HOT),
        StrokeKind::Inside,
    );

    let label = data
        .selected_label
        .as_deref()
        .unwrap_or("No province selected");
    painter.text(
        Pos2::new(rect.left() + spacing::S4, rect.bottom() - spacing::S3),
        Align2::LEFT_BOTTOM,
        label,
        TextRole::Caption.font_id(),
        palette::PARCHMENT_DIM,
    );
    painter.text(
        Pos2::new(rect.right() - spacing::S4, rect.top() + spacing::S3),
        Align2::RIGHT_TOP,
        &data.map_mode,
        TextRole::Code.font_id(),
        palette::BRASS_BRIGHT,
    );
    Tooltip::titled("Mini map", "Strategic overview and current map mode.")
        .show_for_response(ui, &response);
}

fn draw_time_strip(
    ui: &mut egui::Ui,
    rect: Rect,
    data: &MiniMapHudData,
    cmd: &mut Option<SpeedCommand>,
) {
    let painter = ui.painter();
    paint::paint_bevel(painter, rect, palette::SOOT_BLACK, palette::EDGE_DARK, 1.0);
    let date_rect = Rect::from_min_max(
        rect.left_top() + Vec2::new(spacing::S3, spacing::S3),
        Pos2::new(rect.left() + 118.0, rect.bottom() - spacing::S3),
    );
    paint::paint_recessed_panel(painter, date_rect, 1.0);
    painter.text(
        date_rect.center(),
        Align2::CENTER_CENTER,
        &data.date,
        TextRole::Subheading.font_id(),
        palette::PARCHMENT,
    );

    let speed_start = date_rect.right() + spacing::S4;
    let speed_gap = 4.0;
    let speed_w = ((rect.right() - spacing::S3 - speed_start)
        - speed_gap * (SPEED_COUNT.saturating_sub(1) as f32))
        / SPEED_COUNT as f32;
    for (idx, speed_cmd) in [
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
        let r = Rect::from_min_max(
            Pos2::new(
                speed_start + idx as f32 * (speed_w + speed_gap),
                date_rect.top(),
            ),
            Pos2::new(
                speed_start + idx as f32 * (speed_w + speed_gap) + speed_w,
                date_rect.bottom(),
            ),
        );
        if speed_button(ui, r, idx as u8, data.speed_index == idx as u8) {
            *cmd = Some(*speed_cmd);
        }
    }
}

fn speed_button(ui: &mut egui::Ui, rect: Rect, speed: u8, active: bool) -> bool {
    let response = ui.interact(
        rect,
        ui.id().with(("v9_minimap_speed", speed)),
        Sense::click(),
    );
    sound::hook_response_auto(&format!("speed:{}", speed), &response, true);
    let painter = ui.painter();
    let fill = if active {
        palette::IRON
    } else if response.hovered() {
        palette::IRON_DARK
    } else {
        palette::PANEL_DEEP
    };
    paint::paint_bevel(
        painter,
        rect,
        fill,
        if active || response.hovered() {
            palette::BRASS_BRIGHT
        } else {
            palette::EDGE_DARK
        },
        1.0,
    );
    if speed == 0 {
        for x in [-4.0, 4.0] {
            painter.rect_filled(
                Rect::from_center_size(rect.center() + Vec2::new(x, 0.0), Vec2::new(3.0, 13.0)),
                0.0,
                palette::PARCHMENT,
            );
        }
    } else {
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            speed.to_string(),
            TextRole::Numeric.font_id(),
            if active {
                palette::GOLD_HOT
            } else {
                palette::PARCHMENT_DIM
            },
        );
    }
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    Tooltip::new(if speed == 0 {
        "Pause"
    } else {
        "Set game speed"
    })
    .show_for_response(ui, &response);
    response.clicked()
}
