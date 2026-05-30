//! V9 event modal composite.

use egui::{Align2, Color32, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};
use hoi4_content::Event as ContentEvent;

use crate::{
    i18n::tr,
    v9::{
        layout::{GridLayout, Track},
        paint,
        primitives::{Modal, Tooltip},
        sound,
        tokens::{palette, spacing, TextRole},
    },
};

pub fn show_event_modal(
    ctx: &egui::Context,
    event: &ContentEvent,
    queue_extra: usize,
    mut option_trigger_satisfied: impl FnMut(usize) -> bool,
) -> Option<usize> {
    let title = localized_event_title(event);
    let desc = localized_event_description(event);
    let screen = ctx.screen_rect();
    let modal_w = screen
        .width()
        .mul_add(0.0, 720.0)
        .min(screen.width() * 0.82);
    let option_h = 46.0 * event.options.len().max(1) as f32;
    let modal_h = (292.0 + option_h).clamp(420.0, (screen.height() * 0.82).max(420.0));
    let mut picked = None;

    Modal::new(("v9_event_modal", &event.id), Vec2::new(modal_w, modal_h))
        .title(&title)
        .accent(if matches!(event.scope, hoi4_content::EventScope::News) {
            palette::INFO
        } else {
            palette::BRASS_BRIGHT
        })
        .show(ctx, |ui, body| {
            let rows = GridLayout::new(
                vec![
                    Track::Fixed(26.0),
                    Track::Fixed(138.0),
                    Track::Fr(1.0),
                    Track::Fixed(22.0),
                ],
                vec![Track::Fr(1.0)],
            )
            .with_gutter(0.0, spacing::S4);
            let cells = rows.measure(body);
            draw_meta(ui, GridLayout::cell(&cells, 0, 0), event, queue_extra);
            draw_picture_and_description(ui, GridLayout::cell(&cells, 1, 0), event, &desc);
            draw_options(
                ui,
                GridLayout::cell(&cells, 2, 0),
                event,
                &mut option_trigger_satisfied,
                &mut picked,
            );
            draw_footer(ui, GridLayout::cell(&cells, 3, 0), event);
        });

    picked
}

fn draw_meta(ui: &mut egui::Ui, rect: Rect, event: &ContentEvent, queue_extra: usize) {
    let painter = ui.painter();
    let is_news = matches!(event.scope, hoi4_content::EventScope::News);
    let label = if is_news {
        tr("world_news")
    } else {
        tr("event")
    };
    let fill = if is_news {
        Color32::from_rgba_premultiplied(0x10, 0x18, 0x24, 210)
    } else {
        Color32::from_rgba_premultiplied(0x10, 0x12, 0x10, 210)
    };
    paint::paint_bevel(
        painter,
        rect,
        fill,
        if is_news {
            palette::INFO
        } else {
            palette::BRASS_DARK
        },
        1.0,
    );
    painter.text(
        Pos2::new(rect.left() + spacing::S4, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        TextRole::Caption.font_id(),
        if is_news {
            palette::INFO
        } else {
            palette::BRASS_BRIGHT
        },
    );
    if queue_extra > 0 {
        painter.text(
            Pos2::new(rect.right() - spacing::S4, rect.center().y),
            Align2::RIGHT_CENTER,
            format!("+{} {}", queue_extra, tr("more_events")),
            TextRole::Caption.font_id(),
            palette::WARN,
        );
    }
}

fn draw_picture_and_description(
    ui: &mut egui::Ui,
    rect: Rect,
    event: &ContentEvent,
    description: &str,
) {
    let grid = GridLayout::new(
        vec![Track::Fr(1.0)],
        vec![Track::Fixed(168.0), Track::Fr(1.0)],
    )
    .with_gutter(spacing::S5, 0.0);
    let cells = grid.measure(rect);
    let picture = GridLayout::cell(&cells, 0, 0);
    let desc = GridLayout::cell(&cells, 0, 1);
    let painter = ui.painter();

    paint::paint_recessed_panel(painter, picture, 1.0);
    painter.rect_stroke(
        picture,
        egui::epaint::CornerRadius::same(1),
        Stroke::new(1.0, palette::EDGE_DARK),
        StrokeKind::Inside,
    );
    paint::paint_plate_grain(painter, picture.shrink(4.0), 4.0, 2);
    let picture_label = if event.picture.is_empty() {
        "EVENT PICTURE"
    } else {
        event.picture.as_str()
    };
    painter.text(
        picture.center(),
        Align2::CENTER_CENTER,
        picture_label,
        TextRole::Code.font_id(),
        palette::MUTED,
    );

    paint::paint_recessed_panel(painter, desc, 1.0);
    let text_rect = desc.shrink2(Vec2::new(spacing::S4, spacing::S4));
    let galley = painter.layout(
        description.to_owned(),
        TextRole::Body.font_id(),
        palette::PARCHMENT,
        text_rect.width(),
    );
    painter.galley(text_rect.min, galley, palette::PARCHMENT);
}

fn draw_options(
    ui: &mut egui::Ui,
    rect: Rect,
    event: &ContentEvent,
    option_trigger_satisfied: &mut impl FnMut(usize) -> bool,
    picked: &mut Option<usize>,
) {
    if rect.height() <= 8.0 {
        return;
    }
    let row_h = 40.0;
    let max_visible = ((rect.height() + spacing::S2) / (row_h + spacing::S2)) as usize;
    for (idx, option) in event.options.iter().take(max_visible).enumerate() {
        let row = Rect::from_min_max(
            Pos2::new(rect.left(), rect.top() + idx as f32 * (row_h + spacing::S2)),
            Pos2::new(
                rect.right(),
                rect.top() + idx as f32 * (row_h + spacing::S2) + row_h,
            ),
        );
        let enabled = option_trigger_satisfied(idx);
        let opt_name = tr(&option.name);
        let effect_preview = option
            .effects
            .iter()
            .filter_map(|effect| effect.effect_summary())
            .collect::<Vec<_>>()
            .join(" | ");
        if option_row(ui, row, idx, enabled, opt_name, &effect_preview) {
            *picked = Some(idx);
        }
    }
}

fn option_row(
    ui: &mut egui::Ui,
    rect: Rect,
    idx: usize,
    enabled: bool,
    label: &str,
    effect_preview: &str,
) -> bool {
    let response = ui.interact(rect, ui.id().with(("v9_event_option", idx)), Sense::click());
    sound::hook_response_auto(&format!("event_option:{}", idx), &response, enabled);
    let hovered = response.hovered() && enabled;
    let fill = if !enabled {
        palette::PANEL_DEEP
    } else if hovered {
        palette::IRON_DARK
    } else {
        palette::SOOT_BLACK
    };
    let stroke = if hovered {
        palette::GOLD_HOT
    } else if enabled {
        palette::BRASS_DARK
    } else {
        palette::HAIRLINE
    };
    paint::paint_bevel(ui.painter(), rect, fill, stroke, 1.0);
    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    ui.painter().text(
        Pos2::new(rect.left() + spacing::S5, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        TextRole::Subheading.font_id(),
        if enabled {
            palette::PARCHMENT
        } else {
            palette::MUTED
        },
    );
    if !effect_preview.is_empty() {
        ui.painter().text(
            Pos2::new(rect.right() - spacing::S5, rect.center().y),
            Align2::RIGHT_CENTER,
            effect_preview,
            TextRole::Caption.font_id(),
            if enabled {
                palette::BRASS_BRIGHT
            } else {
                palette::MUTED
            },
        );
        Tooltip::new(effect_preview).show_for_response(ui, &response);
    }
    enabled && response.clicked()
}

fn draw_footer(ui: &mut egui::Ui, rect: Rect, event: &ContentEvent) {
    ui.painter().hline(
        rect.left()..=rect.right(),
        rect.top(),
        Stroke::new(1.0, Color32::from_black_alpha(220)),
    );
    ui.painter().text(
        Pos2::new(rect.left(), rect.center().y),
        Align2::LEFT_CENTER,
        tr("event_id_footer").replace("{}", &event.id),
        TextRole::Code.font_id(),
        palette::MUTED,
    );
}

fn localized_event_title(event: &ContentEvent) -> String {
    let translated = tr(&event.id);
    if translated == event.id {
        event.title.clone()
    } else {
        translated.to_owned()
    }
}

fn localized_event_description(event: &ContentEvent) -> String {
    let desc_key = format!("desc.{}", event.id);
    let translated = tr(&desc_key);
    if translated == desc_key {
        event.description.clone()
    } else {
        translated.to_owned()
    }
}
