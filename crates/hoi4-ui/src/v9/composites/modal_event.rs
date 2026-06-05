//! V9 event modal composite.

use egui::{Align2, Area, Color32, Order, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};
use hoi4_content::Event as ContentEvent;

use crate::{
    i18n::tr,
    v9::{
        layout::{GridLayout, Track},
        paint,
        primitives::Tooltip,
        sound,
        tokens::{palette, spacing, TextRole},
    },
    vanilla_iron::VanillaIron,
};

pub fn show_event_modal(
    ctx: &egui::Context,
    event: &ContentEvent,
    queue_extra: usize,
    mut icon_bank: Option<&mut crate::icons::IconBank>,
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

    let size = Vec2::new(modal_w, modal_h);
    let pos = Pos2::new(
        screen.center().x - size.x * 0.5,
        screen.center().y - size.y * 0.5,
    );
    let accent = if matches!(event.scope, hoi4_content::EventScope::News) {
        palette::INFO
    } else {
        VanillaIron::BRASS_BRIGHT
    };

    Area::new(egui::Id::new(("event_modal_iron_backdrop", &event.id)))
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

    Area::new(egui::Id::new(("event_modal_iron", &event.id)))
        .order(Order::Foreground)
        .fixed_pos(pos)
        .default_size(size)
        .show(ctx, |ui| {
            let (outer, _) = ui.allocate_exact_size(size, Sense::click_and_drag());
            VanillaIron::paint_panel(ui, outer, accent);
            let header = Rect::from_min_max(
                outer.left_top() + Vec2::new(14.0, 10.0),
                Pos2::new(outer.right() - 14.0, outer.top() + 48.0),
            );
            ui.painter().text(
                header.center(),
                Align2::CENTER_CENTER,
                &title,
                TextRole::Display.font_id(),
                VanillaIron::TEXT,
            );
            ui.painter().hline(
                header.left()..=header.right(),
                header.bottom(),
                Stroke::new(1.0, VanillaIron::EDGE),
            );
            let body = Rect::from_min_max(
                Pos2::new(outer.left() + 14.0, header.bottom() + spacing::S4),
                outer.right_bottom() - Vec2::new(14.0, 12.0),
            );
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
            draw_picture_and_description(
                ui,
                GridLayout::cell(&cells, 1, 0),
                event,
                icon_bank.as_deref_mut(),
                &desc,
            );
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
    let _ = fill;
    VanillaIron::paint_region(painter, rect, VanillaIron::CARD_DEEP);
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
    icon_bank: Option<&mut crate::icons::IconBank>,
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
    let painter = ui.painter().clone();

    VanillaIron::paint_region(&painter, picture, VanillaIron::CARD_DEEP);
    painter.rect_stroke(
        picture,
        egui::epaint::CornerRadius::same(1),
        Stroke::new(1.0, palette::EDGE_DARK),
        StrokeKind::Inside,
    );
    let image_rect = picture.shrink(5.0);
    let texture = if event.picture.is_empty() {
        None
    } else {
        icon_bank.and_then(|bank| bank.get_or_load(&event.picture))
    };
    if let Some(handle) = texture {
        ui.put(
            image_rect,
            egui::Image::from_texture(handle).fit_to_exact_size(image_rect.size()),
        );
    } else {
        draw_event_picture_fallback(ui, image_rect, event);
    }

    VanillaIron::paint_region(&painter, desc, VanillaIron::CARD_DEEP);
    let text_rect = desc.shrink2(Vec2::new(spacing::S4, spacing::S4));
    let galley = painter.layout(
        description.to_owned(),
        TextRole::Body.font_id(),
        palette::PARCHMENT,
        text_rect.width(),
    );
    painter.galley(text_rect.min, galley, palette::PARCHMENT);
}

fn draw_event_picture_fallback(ui: &mut egui::Ui, rect: Rect, event: &ContentEvent) {
    let painter = ui.painter();
    paint::paint_plate_grain(painter, rect, 4.0, 3);
    let tint = if matches!(event.scope, hoi4_content::EventScope::News) {
        palette::INFO
    } else {
        palette::BRASS_DARK
    };
    painter.rect_filled(
        Rect::from_min_max(
            rect.min,
            Pos2::new(rect.left() + rect.width() * 0.34, rect.bottom()),
        ),
        egui::epaint::CornerRadius::same(1),
        Color32::from_rgba_premultiplied(tint.r(), tint.g(), tint.b(), 36),
    );
    painter.line_segment(
        [
            Pos2::new(rect.left() + rect.width() * 0.12, rect.bottom() - 8.0),
            Pos2::new(rect.right() - 10.0, rect.top() + 10.0),
        ],
        Stroke::new(1.0, Color32::from_white_alpha(28)),
    );
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        if matches!(event.scope, hoi4_content::EventScope::News) {
            tr("news_picture")
        } else {
            tr("event_picture")
        },
        TextRole::Caption.font_id(),
        palette::MUTED,
    );
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
    let _ = (fill, stroke);
    VanillaIron::paint_region(
        ui.painter(),
        rect,
        if hovered {
            VanillaIron::CARD_SOFT
        } else {
            VanillaIron::CARD_DEEP
        },
    );
    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    let preview_left = if effect_preview.is_empty() {
        rect.right() - spacing::S5
    } else {
        rect.left() + rect.width() * 0.46
    };
    let label_rect = Rect::from_min_max(
        Pos2::new(rect.left() + spacing::S5, rect.top()),
        Pos2::new(preview_left - spacing::S3, rect.bottom()),
    );
    let label_painter = ui.painter().with_clip_rect(label_rect);
    label_painter.text(
        Pos2::new(label_rect.left(), label_rect.center().y),
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
        let preview_rect = Rect::from_min_max(
            Pos2::new(preview_left, rect.top()),
            Pos2::new(rect.right() - spacing::S5, rect.bottom()),
        );
        let preview_painter = ui.painter().with_clip_rect(preview_rect);
        preview_painter.text(
            Pos2::new(preview_rect.right(), preview_rect.center().y),
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
