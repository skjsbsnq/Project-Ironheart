//! V9 procedural paint primitives.
//!
//! The Phase A API is kept intact, but the material language now targets the
//! reference-2 style: blackened steel, worn brass, recessed wells, rivets, and
//! chipped edges instead of warm wood/parchment panels.

use egui::{
    epaint::{CornerRadius, Mesh, PathShape, PathStroke, Vertex, WHITE_UV},
    Color32, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, Vec2,
};

use super::tokens::{border, palette, spacing, Elevation};

pub fn paint_shadow(painter: &Painter, rect: Rect, elev: Elevation, corner: f32) {
    let alpha = elev.shadow_alpha();
    let offset = elev.shadow_offset();
    if alpha <= 0.0 || offset <= 0.0 {
        return;
    }

    let cr = CornerRadius::same(corner as u8);
    for i in 0..7 {
        let t = (i as f32 + 1.0) / 7.0;
        let expand = offset * (0.7 + t * 1.6);
        let a = (alpha * (1.0 - t * 0.48) * 255.0) as u8;
        let shadow_rect = Rect::from_min_max(
            Pos2::new(rect.min.x - expand * 0.55, rect.min.y + expand * 0.15),
            Pos2::new(rect.max.x + expand * 0.55, rect.max.y + expand * 1.25),
        );
        painter.rect_filled(shadow_rect, cr, Color32::from_black_alpha(a));
    }

    let contact = Rect::from_min_max(
        Pos2::new(rect.min.x - offset * 0.30, rect.max.y - offset * 0.08),
        Pos2::new(rect.max.x + offset * 0.30, rect.max.y + offset * 0.34),
    );
    painter.rect_filled(
        contact,
        CornerRadius::same((corner + 2.0).max(0.0) as u8),
        Color32::from_black_alpha((alpha * 160.0).clamp(0.0, 210.0) as u8),
    );
}

pub fn paint_bevel(painter: &Painter, rect: Rect, fill: Color32, edge: Color32, corner: f32) {
    let cr = CornerRadius::same(corner as u8);
    if fill.a() > 0 {
        painter.rect_filled(rect, cr, fill);
    }
    paint_inset_lip(painter, rect, corner);
    paint_vertical_gradient_mesh(
        painter,
        rect.shrink(1.0),
        Color32::from_rgba_premultiplied(0x28, 0x2b, 0x25, 10),
        Color32::from_black_alpha(206),
    );
    paint_horizontal_gradient_mesh(
        painter,
        rect.shrink(1.0),
        Color32::from_black_alpha(116),
        Color32::from_white_alpha(3),
    );

    painter.rect_stroke(rect, cr, Stroke::new(border::B1, edge), StrokeKind::Inside);
    painter.rect_stroke(
        rect.shrink(1.0),
        CornerRadius::same((corner - 1.0).max(0.0) as u8),
        Stroke::new(1.0, Color32::from_white_alpha(2)),
        StrokeKind::Inside,
    );
    painter.rect_stroke(
        rect.shrink(2.0),
        CornerRadius::same((corner - 1.0).max(0.0) as u8),
        Stroke::new(1.0, Color32::from_black_alpha(228)),
        StrokeKind::Inside,
    );

    painter.hline(
        (rect.left() + 2.0)..=(rect.right() - 2.0),
        rect.top() + 1.0,
        Stroke::new(1.0, Color32::from_white_alpha(5)),
    );
    painter.hline(
        (rect.left() + 2.0)..=(rect.right() - 2.0),
        rect.bottom() - 1.0,
        Stroke::new(1.0, Color32::from_black_alpha(220)),
    );
    painter.line_segment(
        [
            Pos2::new(rect.left() + 1.0, rect.top() + 2.0),
            Pos2::new(rect.left() + 1.0, rect.bottom() - 2.0),
        ],
        Stroke::new(1.0, Color32::from_white_alpha(6)),
    );
    painter.line_segment(
        [
            Pos2::new(rect.right() - 1.0, rect.top() + 2.0),
            Pos2::new(rect.right() - 1.0, rect.bottom() - 2.0),
        ],
        Stroke::new(1.0, Color32::from_black_alpha(150)),
    );
    paint_edge_wear(painter, rect.shrink(1.0), palette::EDGE_LIGHT, 4);
    paint_micro_scratches(painter, rect.shrink(5.0), 2);
    paint_speckle(painter, rect.shrink(4.0), 8, 2);
}

pub fn paint_wood(painter: &Painter, rect: Rect, corner: f32) {
    paint_blackened_steel(painter, rect, corner);
}

pub fn paint_blackened_steel(painter: &Painter, rect: Rect, corner: f32) {
    painter.rect_filled(rect, CornerRadius::same(corner as u8), palette::IRON_DARK);
    paint_multistop_gradient(
        painter,
        rect,
        &[
            (0.00, Color32::from_rgba_premultiplied(0x28, 0x2a, 0x26, 22)),
            (
                0.08,
                Color32::from_rgba_premultiplied(0x13, 0x15, 0x13, 100),
            ),
            (
                0.46,
                Color32::from_rgba_premultiplied(0x08, 0x09, 0x08, 232),
            ),
            (0.82, Color32::from_black_alpha(242)),
            (1.00, Color32::from_black_alpha(250)),
        ],
    );
    paint_radial_corner_vignette(painter, rect, 124);
    paint_horizontal_gradient_mesh(
        painter,
        rect,
        Color32::from_black_alpha(142),
        Color32::from_black_alpha(58),
    );
    paint_burnished_steel_surface(painter, rect.shrink(2.0), 0.13);
    paint_plate_grain(painter, rect.shrink(2.0), 3.0, 4);
    paint_oil_stains(painter, rect.shrink(5.0), 2);
    paint_micro_scratches(painter, rect.shrink(6.0), 4);
    paint_plate_seams(painter, rect.shrink(5.0), 280.0, 190.0);
    paint_speckle(painter, rect.shrink(5.0), 28, 3);
    paint_edge_wear(painter, rect.shrink(1.0), palette::EDGE_LIGHT, 5);
}

pub fn paint_plate_grain(painter: &Painter, rect: Rect, stride: f32, alpha_base: u8) {
    if rect.width() <= 2.0 || rect.height() <= 2.0 {
        return;
    }

    let seed = (rect.min.x as i32).wrapping_mul(31) ^ (rect.min.y as i32);
    let n = ((rect.height() / stride) as i32).max(1);
    for i in 0..n {
        let y = rect.min.y + i as f32 * stride + 0.5;
        if y >= rect.max.y - 1.0 {
            break;
        }
        let h = seed
            .wrapping_add(i.wrapping_mul(1_103_515_245))
            .wrapping_mul(12_345) as u32;
        let alpha = (alpha_base / 2).saturating_add((h & 0x03) as u8);
        let col = if i % 4 == 0 {
            Color32::from_white_alpha(alpha / 5)
        } else {
            Color32::from_black_alpha(alpha)
        };
        painter.hline(
            (rect.min.x + 1.0)..=(rect.max.x - 1.0),
            y,
            Stroke::new(0.45, col),
        );
    }
}

pub fn paint_plate_seams(painter: &Painter, rect: Rect, x_stride: f32, y_stride: f32) {
    if rect.width() <= 32.0 || rect.height() <= 24.0 {
        return;
    }

    let seed = (rect.min.x as i32).wrapping_mul(97) ^ (rect.min.y as i32).wrapping_mul(53);
    let x_count = ((rect.width() / x_stride) as i32).clamp(0, 1);
    for i in 1..=x_count {
        let jitter = ((seed.wrapping_add(i * 19) & 0x0f) as f32) - 7.0;
        let x = rect.min.x + x_stride * i as f32 + jitter;
        if x >= rect.max.x - 12.0 {
            break;
        }
        painter.line_segment(
            [
                Pos2::new(x, rect.min.y + 8.0),
                Pos2::new(x, rect.max.y - 8.0),
            ],
            Stroke::new(0.6, Color32::from_black_alpha(16)),
        );
    }

    let y_count = ((rect.height() / y_stride) as i32).clamp(0, 1);
    for i in 1..=y_count {
        let jitter = ((seed.wrapping_add(i * 31) & 0x0b) as f32) - 5.0;
        let y = rect.min.y + y_stride * i as f32 + jitter;
        if y >= rect.max.y - 10.0 {
            break;
        }
        painter.hline(
            (rect.min.x + 8.0)..=(rect.max.x - 8.0),
            y,
            Stroke::new(0.6, Color32::from_black_alpha(14)),
        );
    }
}

pub fn paint_burnished_steel_surface(painter: &Painter, rect: Rect, strength: f32) {
    if rect.width() <= 8.0 || rect.height() <= 8.0 {
        return;
    }

    let s = strength.clamp(0.0, 1.6);
    let top = Rect::from_min_max(
        rect.min,
        Pos2::new(rect.max.x, rect.min.y + rect.height() * 0.18),
    );
    paint_vertical_gradient_mesh(
        painter,
        top,
        Color32::from_white_alpha(scaled_alpha(4, s)),
        Color32::from_white_alpha(0),
    );

    let shoulder = Rect::from_min_max(
        Pos2::new(rect.min.x, rect.min.y + rect.height() * 0.10),
        Pos2::new(rect.max.x, rect.min.y + rect.height() * 0.26),
    );
    paint_vertical_gradient_mesh(
        painter,
        shoulder,
        Color32::from_rgba_premultiplied(0x4c, 0x50, 0x49, scaled_alpha(3, s)),
        Color32::from_white_alpha(0),
    );

    let lower = Rect::from_min_max(
        Pos2::new(rect.min.x, rect.min.y + rect.height() * 0.50),
        rect.max,
    );
    paint_vertical_gradient_mesh(
        painter,
        lower,
        Color32::from_black_alpha(0),
        Color32::from_black_alpha(scaled_alpha(88, s)),
    );

    let left = Rect::from_min_size(rect.min, Vec2::new(rect.width() * 0.16, rect.height()));
    paint_horizontal_gradient_mesh(
        painter,
        left,
        Color32::from_black_alpha(scaled_alpha(48, s)),
        Color32::from_black_alpha(0),
    );

    let right = Rect::from_min_max(
        Pos2::new(rect.max.x - rect.width() * 0.14, rect.min.y),
        rect.max,
    );
    paint_horizontal_gradient_mesh(
        painter,
        right,
        Color32::from_black_alpha(0),
        Color32::from_black_alpha(scaled_alpha(50, s)),
    );

    let y = rect.min.y + rect.height() * 0.22;
    painter.hline(
        (rect.min.x + 4.0)..=(rect.max.x - 4.0),
        y + 1.0,
        Stroke::new(0.8, Color32::from_black_alpha(scaled_alpha(24, s))),
    );
}

pub fn paint_oil_stains(painter: &Painter, rect: Rect, count: usize) {
    if rect.width() <= 10.0 || rect.height() <= 10.0 || count == 0 {
        return;
    }

    let seed = (rect.min.x as i32).wrapping_mul(131) ^ (rect.min.y as i32).wrapping_mul(71);
    for i in 0..count {
        let h = seed
            .wrapping_add((i as i32).wrapping_mul(1_664_525))
            .wrapping_mul(1_013_904_223) as u32;
        let tx = ((h & 0xff) as f32) / 255.0;
        let ty = (((h >> 8) & 0xff) as f32) / 255.0;
        let w = 18.0 + ((h >> 16) & 0x1f) as f32;
        let hgt = 7.0 + ((h >> 21) & 0x0f) as f32;
        let center = Pos2::new(
            rect.min.x + rect.width() * tx,
            rect.min.y + rect.height() * ty,
        );
        let stain = Rect::from_center_size(center, Vec2::new(w, hgt));
        let alpha = 7 + ((h >> 25) & 0x0f) as u8;
        let radius = (hgt * 0.5).clamp(3.0, 9.0) as u8;
        painter.rect_filled(
            stain.intersect(rect),
            CornerRadius::same(radius),
            Color32::from_black_alpha(alpha),
        );
        let core = stain
            .shrink2(Vec2::new(w * 0.18, hgt * 0.22))
            .intersect(rect);
        if core.is_positive() {
            painter.rect_filled(
                core,
                CornerRadius::same(radius.saturating_sub(1)),
                Color32::from_black_alpha(alpha.saturating_add(5)),
            );
        }
    }
}

pub fn paint_edge_wear(painter: &Painter, rect: Rect, color: Color32, alpha: u8) {
    let c = Color32::from_rgba_premultiplied(color.r(), color.g(), color.b(), alpha);
    let dark = Color32::from_black_alpha(alpha.saturating_mul(2));
    let segments = [
        (
            Pos2::new(rect.left() + 8.0, rect.top() + 1.0),
            Pos2::new(rect.left() + rect.width() * 0.28, rect.top() + 1.0),
            c,
        ),
        (
            Pos2::new(rect.right() - rect.width() * 0.30, rect.top() + 1.0),
            Pos2::new(rect.right() - 10.0, rect.top() + 1.0),
            c,
        ),
        (
            Pos2::new(rect.left() + 10.0, rect.bottom() - 1.0),
            Pos2::new(rect.left() + rect.width() * 0.22, rect.bottom() - 1.0),
            dark,
        ),
        (
            Pos2::new(rect.right() - rect.width() * 0.26, rect.bottom() - 1.0),
            Pos2::new(rect.right() - 8.0, rect.bottom() - 1.0),
            dark,
        ),
        (
            Pos2::new(rect.left() + 1.0, rect.top() + rect.height() * 0.18),
            Pos2::new(rect.left() + 1.0, rect.top() + rect.height() * 0.42),
            c,
        ),
        (
            Pos2::new(rect.right() - 1.0, rect.bottom() - rect.height() * 0.38),
            Pos2::new(rect.right() - 1.0, rect.bottom() - rect.height() * 0.16),
            dark,
        ),
    ];
    for (a, b, col) in segments {
        painter.line_segment([a, b], Stroke::new(1.0, col));
    }
}

pub fn paint_parchment(painter: &Painter, rect: Rect, corner: f32) {
    painter.rect_filled(rect, CornerRadius::same(corner as u8), palette::IRON_DARK);
    paint_multistop_gradient(
        painter,
        rect,
        &[
            (0.00, Color32::from_rgba_premultiplied(0x22, 0x24, 0x20, 78)),
            (
                0.18,
                Color32::from_rgba_premultiplied(0x0d, 0x0e, 0x0c, 202),
            ),
            (0.72, Color32::from_black_alpha(236)),
            (1.00, Color32::from_black_alpha(248)),
        ],
    );
    paint_burnished_steel_surface(painter, rect.shrink(2.0), 0.08);
    paint_plate_grain(painter, rect.shrink(2.0), 3.0, 3);
    paint_oil_stains(painter, rect.shrink(6.0), 2);
    paint_micro_scratches(painter, rect.shrink(5.0), 3);
    paint_speckle(painter, rect.shrink(4.0), 14, 2);
}

pub fn paint_recessed_panel(painter: &Painter, rect: Rect, corner: f32) {
    painter.rect_filled(rect, CornerRadius::same(corner as u8), palette::SOOT_BLACK);
    paint_multistop_gradient(
        painter,
        rect,
        &[
            (0.00, Color32::from_black_alpha(248)),
            (0.08, Color32::from_black_alpha(206)),
            (
                0.30,
                Color32::from_rgba_premultiplied(0x07, 0x08, 0x07, 238),
            ),
            (
                0.70,
                Color32::from_rgba_premultiplied(0x04, 0x05, 0x05, 244),
            ),
            (1.00, Color32::from_black_alpha(250)),
        ],
    );
    painter.rect_stroke(
        rect,
        CornerRadius::same(corner as u8),
        Stroke::new(border::B1, Color32::from_black_alpha(245)),
        StrokeKind::Inside,
    );
    painter.rect_stroke(
        rect.shrink(1.0),
        CornerRadius::same((corner - 1.0).max(0.0) as u8),
        Stroke::new(border::B1, Color32::from_white_alpha(4)),
        StrokeKind::Inside,
    );
    painter.rect_stroke(
        rect.shrink(3.0),
        CornerRadius::same((corner - 1.0).max(0.0) as u8),
        Stroke::new(border::B1, Color32::from_black_alpha(185)),
        StrokeKind::Inside,
    );
    paint_inner_lip_shadow(painter, rect, corner);
    paint_burnished_steel_surface(painter, rect.shrink(4.0), 0.06);
    paint_plate_grain(painter, rect.shrink(4.0), 3.0, 3);
    paint_oil_stains(painter, rect.shrink(6.0), 1);
    paint_micro_scratches(painter, rect.shrink(6.0), 2);
}

pub fn paint_inset_lip(painter: &Painter, rect: Rect, corner: f32) {
    if rect.width() <= 8.0 || rect.height() <= 8.0 {
        return;
    }

    let cr = CornerRadius::same(corner as u8);
    painter.rect_stroke(
        rect.translate(Vec2::new(0.0, 1.0)),
        cr,
        Stroke::new(1.0, Color32::from_black_alpha(180)),
        StrokeKind::Inside,
    );
    painter.rect_stroke(
        rect.shrink(1.0),
        CornerRadius::same((corner - 1.0).max(0.0) as u8),
        Stroke::new(1.0, Color32::from_white_alpha(3)),
        StrokeKind::Inside,
    );
    let top_band = Rect::from_min_max(
        Pos2::new(rect.min.x + 2.0, rect.min.y + 1.0),
        Pos2::new(
            rect.max.x - 2.0,
            rect.min.y + (rect.height() * 0.16).clamp(3.0, 10.0),
        ),
    );
    paint_vertical_gradient_mesh(
        painter,
        top_band,
        Color32::from_white_alpha(4),
        Color32::from_white_alpha(0),
    );
}

pub fn paint_inner_lip_shadow(painter: &Painter, rect: Rect, corner: f32) {
    if rect.width() <= 10.0 || rect.height() <= 10.0 {
        return;
    }

    let cr = CornerRadius::same(corner as u8);
    painter.rect_stroke(
        rect,
        cr,
        Stroke::new(1.0, Color32::from_black_alpha(245)),
        StrokeKind::Inside,
    );
    painter.rect_stroke(
        rect.shrink(1.0),
        CornerRadius::same((corner - 1.0).max(0.0) as u8),
        Stroke::new(1.0, Color32::from_black_alpha(170)),
        StrokeKind::Inside,
    );
    painter.hline(
        (rect.left() + 3.0)..=(rect.right() - 3.0),
        rect.top() + 2.0,
        Stroke::new(2.0, Color32::from_black_alpha(138)),
    );
    painter.hline(
        (rect.left() + 3.0)..=(rect.right() - 3.0),
        rect.bottom() - 2.0,
        Stroke::new(1.0, Color32::from_white_alpha(5)),
    );
}

pub fn paint_vertical_gradient_mesh(painter: &Painter, rect: Rect, top: Color32, bottom: Color32) {
    let mut mesh = Mesh::default();
    mesh.vertices.push(Vertex {
        pos: rect.left_top(),
        uv: WHITE_UV,
        color: top,
    });
    mesh.vertices.push(Vertex {
        pos: rect.right_top(),
        uv: WHITE_UV,
        color: top,
    });
    mesh.vertices.push(Vertex {
        pos: rect.right_bottom(),
        uv: WHITE_UV,
        color: bottom,
    });
    mesh.vertices.push(Vertex {
        pos: rect.left_bottom(),
        uv: WHITE_UV,
        color: bottom,
    });
    mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
    painter.add(Shape::mesh(mesh));
}

pub fn paint_horizontal_gradient_mesh(
    painter: &Painter,
    rect: Rect,
    left: Color32,
    right: Color32,
) {
    let mut mesh = Mesh::default();
    mesh.vertices.push(Vertex {
        pos: rect.left_top(),
        uv: WHITE_UV,
        color: left,
    });
    mesh.vertices.push(Vertex {
        pos: rect.right_top(),
        uv: WHITE_UV,
        color: right,
    });
    mesh.vertices.push(Vertex {
        pos: rect.right_bottom(),
        uv: WHITE_UV,
        color: right,
    });
    mesh.vertices.push(Vertex {
        pos: rect.left_bottom(),
        uv: WHITE_UV,
        color: left,
    });
    mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
    painter.add(Shape::mesh(mesh));
}

pub fn paint_multistop_gradient(painter: &Painter, rect: Rect, stops: &[(f32, Color32)]) {
    if stops.len() < 2 {
        return;
    }

    let mut mesh = Mesh::default();
    for (i, (t, color)) in stops.iter().enumerate() {
        let y = rect.min.y + rect.height() * t.clamp(0.0, 1.0);
        mesh.vertices.push(Vertex {
            pos: Pos2::new(rect.min.x, y),
            uv: WHITE_UV,
            color: *color,
        });
        mesh.vertices.push(Vertex {
            pos: Pos2::new(rect.max.x, y),
            uv: WHITE_UV,
            color: *color,
        });
        if i > 0 {
            let base = (i - 1) as u32 * 2;
            mesh.indices
                .extend_from_slice(&[base, base + 1, base + 3, base, base + 3, base + 2]);
        }
    }
    painter.add(Shape::mesh(mesh));
}

pub fn paint_brass_ribbon(painter: &Painter, rect: Rect, height: f32, accent: Color32) {
    let band = Rect::from_min_max(
        Pos2::new(rect.min.x + spacing::S3, rect.min.y + spacing::S2),
        Pos2::new(rect.max.x - spacing::S3, rect.min.y + spacing::S2 + height),
    );
    paint_multistop_gradient(
        painter,
        band,
        &[
            (0.00, palette::GOLD_HOT),
            (0.20, palette::BRASS),
            (0.52, accent),
            (1.00, palette::BRASS_SHADOW),
        ],
    );
    painter.hline(
        band.min.x..=band.max.x,
        band.min.y + 0.5,
        Stroke::new(1.0, Color32::from_white_alpha(22)),
    );
    painter.hline(
        band.min.x..=band.max.x,
        band.min.y + 2.0,
        Stroke::new(1.0, palette::BRASS_DARK),
    );
    painter.hline(
        band.min.x..=band.max.x,
        band.max.y - 0.5,
        Stroke::new(1.0, Color32::from_black_alpha(210)),
    );
    paint_edge_wear(painter, band, palette::BRASS_WORN, 10);
    paint_brass_patina(painter, band, 2);
    paint_micro_scratches(painter, band.shrink(2.0), 2);
}

pub fn paint_brass_patina(painter: &Painter, rect: Rect, count: usize) {
    if rect.width() <= 8.0 || rect.height() <= 4.0 || count == 0 {
        return;
    }

    let seed = (rect.min.x as i32).wrapping_mul(191) ^ (rect.min.y as i32).wrapping_mul(83);
    for i in 0..count {
        let h = seed
            .wrapping_add((i as i32).wrapping_mul(22_695_477))
            .wrapping_mul(1_103_515_245) as u32;
        let tx = ((h & 0xff) as f32) / 255.0;
        let ty = (((h >> 8) & 0xff) as f32) / 255.0;
        let w = 5.0 + ((h >> 16) & 0x0f) as f32;
        let y = rect.min.y + rect.height() * ty;
        let x = rect.min.x + rect.width() * tx;
        let alpha = 18 + ((h >> 24) & 0x17) as u8;
        painter.line_segment(
            [
                Pos2::new(x, y),
                Pos2::new((x + w).clamp(rect.min.x, rect.max.x), y + 0.8),
            ],
            Stroke::new(0.8, Color32::from_black_alpha(alpha)),
        );
        if i % 3 == 0 {
            painter.line_segment(
                [
                    Pos2::new(x, (y - 1.0).clamp(rect.min.y, rect.max.y)),
                    Pos2::new((x + w * 0.55).clamp(rect.min.x, rect.max.x), y - 0.4),
                ],
                Stroke::new(0.6, Color32::from_white_alpha(alpha / 3)),
            );
        }
    }
}

pub fn paint_brass_border(painter: &Painter, rect: Rect, corner: f32) {
    let cr = CornerRadius::same(corner as u8);
    painter.rect_stroke(
        rect.translate(Vec2::new(1.0, 1.0)),
        cr,
        Stroke::new(2.0, Color32::from_black_alpha(220)),
        StrokeKind::Inside,
    );
    painter.rect_stroke(
        rect,
        cr,
        Stroke::new(border::B3, palette::EDGE_DARK),
        StrokeKind::Inside,
    );
    painter.rect_stroke(
        rect.shrink(1.0),
        CornerRadius::same((corner - 1.0).max(0.0) as u8),
        Stroke::new(border::B1, palette::GUNMETAL),
        StrokeKind::Inside,
    );
    painter.rect_stroke(
        rect.shrink(2.0),
        CornerRadius::same((corner - 1.0).max(0.0) as u8),
        Stroke::new(border::B1, Color32::from_black_alpha(232)),
        StrokeKind::Inside,
    );
    painter.hline(
        (rect.left() + 8.0)..=(rect.right() - 8.0),
        rect.top() + 2.0,
        Stroke::new(1.0, Color32::from_white_alpha(7)),
    );
    painter.hline(
        (rect.left() + 8.0)..=(rect.right() - 8.0),
        rect.bottom() - 2.0,
        Stroke::new(1.0, Color32::from_black_alpha(240)),
    );
    let brass = Color32::from_rgba_premultiplied(
        palette::BRASS_DARK.r(),
        palette::BRASS_DARK.g(),
        palette::BRASS_DARK.b(),
        120,
    );
    painter.hline(
        (rect.left() + 10.0)..=(rect.right() - 10.0),
        rect.top() + 3.0,
        Stroke::new(1.0, brass),
    );
    painter.hline(
        (rect.left() + 10.0)..=(rect.right() - 10.0),
        rect.bottom() - 3.0,
        Stroke::new(1.0, Color32::from_black_alpha(230)),
    );
    paint_edge_wear(painter, rect.shrink(1.0), palette::BRASS_WORN, 8);
}

pub fn paint_steel_rails(painter: &Painter, rect: Rect, accent: Color32) {
    if rect.width() <= 32.0 || rect.height() <= 24.0 {
        return;
    }

    let rail_h = rect.height().mul_add(0.035, 3.0).clamp(6.0, 12.0);
    let top = Rect::from_min_max(
        Pos2::new(rect.left() + 5.0, rect.top() + 4.0),
        Pos2::new(rect.right() - 5.0, rect.top() + 4.0 + rail_h),
    );
    let bottom = Rect::from_min_max(
        Pos2::new(rect.left() + 5.0, rect.bottom() - 4.0 - rail_h),
        Pos2::new(rect.right() - 5.0, rect.bottom() - 4.0),
    );

    for (rail, invert) in [(top, false), (bottom, true)] {
        let (a, b) = if invert {
            (
                Color32::from_black_alpha(246),
                Color32::from_rgba_premultiplied(0x12, 0x14, 0x11, 226),
            )
        } else {
            (
                Color32::from_rgba_premultiplied(0x20, 0x23, 0x1e, 140),
                Color32::from_black_alpha(242),
            )
        };
        paint_vertical_gradient_mesh(painter, rail, a, b);
        painter.rect_stroke(
            rail,
            CornerRadius::same(1),
            Stroke::new(1.0, Color32::from_black_alpha(230)),
            StrokeKind::Inside,
        );
        painter.hline(
            (rail.left() + 4.0)..=(rail.right() - 4.0),
            rail.top() + 1.0,
            Stroke::new(1.0, Color32::from_white_alpha(if invert { 1 } else { 4 })),
        );
        painter.hline(
            (rail.left() + 8.0)..=(rail.right() - 8.0),
            rail.bottom() - 1.0,
            Stroke::new(1.0, Color32::from_black_alpha(238)),
        );
    }

    let accent = Color32::from_rgba_premultiplied(accent.r(), accent.g(), accent.b(), 95);
    painter.hline(
        (top.left() + 12.0)..=(top.right() - 12.0),
        top.bottom() + 1.0,
        Stroke::new(1.0, accent),
    );
}

pub fn paint_corner_caps(painter: &Painter, rect: Rect) {
    if rect.width() <= 48.0 || rect.height() <= 36.0 {
        return;
    }

    let cap = Vec2::new(22.0, 18.0);
    let caps = [
        Rect::from_min_size(rect.min + Vec2::new(3.0, 3.0), cap),
        Rect::from_min_size(Pos2::new(rect.right() - cap.x - 3.0, rect.top() + 3.0), cap),
        Rect::from_min_size(
            Pos2::new(rect.left() + 3.0, rect.bottom() - cap.y - 3.0),
            cap,
        ),
        Rect::from_min_size(
            Pos2::new(rect.right() - cap.x - 3.0, rect.bottom() - cap.y - 3.0),
            cap,
        ),
    ];
    for cap_rect in caps {
        paint_vertical_gradient_mesh(
            painter,
            cap_rect,
            Color32::from_rgba_premultiplied(0x19, 0x1b, 0x17, 212),
            Color32::from_black_alpha(230),
        );
        painter.rect_stroke(
            cap_rect,
            CornerRadius::same(1),
            Stroke::new(1.0, Color32::from_black_alpha(238)),
            StrokeKind::Inside,
        );
        painter.hline(
            (cap_rect.left() + 2.0)..=(cap_rect.right() - 2.0),
            cap_rect.top() + 1.0,
            Stroke::new(1.0, Color32::from_white_alpha(4)),
        );
        paint_slot_screw(painter, cap_rect.center(), 2.1);
    }
}

pub fn paint_double_border(painter: &Painter, rect: Rect, corner: f32) {
    paint_brass_border(painter, rect, corner);
    painter.rect_stroke(
        rect.shrink(4.0),
        CornerRadius::same((corner - 1.0).max(0.0) as u8),
        Stroke::new(border::B1, Color32::from_white_alpha(5)),
        StrokeKind::Inside,
    );
    painter.rect_stroke(
        rect.shrink(6.0),
        CornerRadius::same((corner - 2.0).max(0.0) as u8),
        Stroke::new(border::B1, Color32::from_black_alpha(238)),
        StrokeKind::Inside,
    );
}

pub fn paint_rivets(painter: &Painter, rect: Rect) {
    let inset = spacing::S4;
    let r = 3.2_f32;
    for (x, y) in [
        (rect.min.x + inset, rect.min.y + inset),
        (rect.max.x - inset, rect.min.y + inset),
        (rect.min.x + inset, rect.max.y - inset),
        (rect.max.x - inset, rect.max.y - inset),
    ] {
        painter.circle_filled(
            Pos2::new(x + 1.0, y + 1.5),
            r + 1.2,
            Color32::from_black_alpha(190),
        );
        painter.circle_filled(Pos2::new(x, y), r + 0.4, palette::SOOT_BLACK);
        painter.circle_filled(Pos2::new(x, y), r, palette::EDGE_DARK);
        painter.circle_filled(Pos2::new(x - 0.2, y - 0.2), r - 1.1, palette::IRON);
        painter.circle_filled(Pos2::new(x - 0.8, y - 0.8), r * 0.34, palette::GUNMETAL);
        painter.circle_filled(
            Pos2::new(x - 1.35, y - 1.35),
            r * 0.10,
            Color32::from_white_alpha(24),
        );
        painter.line_segment(
            [Pos2::new(x - 1.5, y), Pos2::new(x + 1.5, y)],
            Stroke::new(0.7, Color32::from_black_alpha(168)),
        );
        painter.circle_stroke(
            Pos2::new(x, y),
            r + 0.2,
            Stroke::new(0.8, Color32::from_black_alpha(180)),
        );
    }
}

pub fn paint_side_rivets(painter: &Painter, rect: Rect, count_per_side: usize) {
    if count_per_side == 0 || rect.height() <= 56.0 {
        return;
    }

    let count = count_per_side.min(6);
    let top = rect.top() + spacing::S7;
    let bottom = rect.bottom() - spacing::S7;
    if bottom <= top {
        return;
    }
    for i in 0..count {
        let t = if count == 1 {
            0.5
        } else {
            i as f32 / (count - 1) as f32
        };
        let y = top + (bottom - top) * t;
        paint_slot_screw(painter, Pos2::new(rect.left() + spacing::S4, y), 2.5);
        paint_slot_screw(painter, Pos2::new(rect.right() - spacing::S4, y), 2.5);
    }
}

pub fn paint_slot_screw(painter: &Painter, center: Pos2, r: f32) {
    painter.circle_filled(
        center + Vec2::new(1.0, 1.2),
        r + 1.0,
        Color32::from_black_alpha(178),
    );
    painter.circle_filled(center, r + 0.5, palette::EDGE_DARK);
    painter.circle_filled(center - Vec2::splat(0.4), r - 0.4, palette::IRON_LIGHT);
    painter.circle_stroke(
        center,
        r + 0.4,
        Stroke::new(0.7, Color32::from_black_alpha(200)),
    );
    painter.line_segment(
        [
            Pos2::new(center.x - r * 0.70, center.y),
            Pos2::new(center.x + r * 0.70, center.y),
        ],
        Stroke::new(0.8, Color32::from_black_alpha(200)),
    );
    painter.line_segment(
        [
            Pos2::new(center.x - r * 0.55, center.y - 0.7),
            Pos2::new(center.x, center.y - 0.7),
        ],
        Stroke::new(0.6, Color32::from_white_alpha(20)),
    );
}

pub fn paint_corner_rosettes(painter: &Painter, rect: Rect, color: Color32) {
    paint_corner_brackets(painter, rect, color);
}

pub fn paint_corner_brackets(painter: &Painter, rect: Rect, color: Color32) {
    let l = 14.0;
    let o = spacing::S5;
    let s = Stroke::new(
        1.0,
        Color32::from_rgba_premultiplied(color.r(), color.g(), color.b(), 70),
    );
    let inner = Stroke::new(0.7, Color32::from_white_alpha(8));
    let shadow = Stroke::new(2.0, Color32::from_black_alpha(210));
    for (x, y, sx, sy) in [
        (rect.left() + o, rect.top() + o, 1.0, 1.0),
        (rect.right() - o, rect.top() + o, -1.0, 1.0),
        (rect.left() + o, rect.bottom() - o, 1.0, -1.0),
        (rect.right() - o, rect.bottom() - o, -1.0, -1.0),
    ] {
        let p = Pos2::new(x, y);
        let hx = Pos2::new(x + sx * l, y);
        let vy = Pos2::new(x, y + sy * l);
        painter.line_segment([p + Vec2::splat(1.0), hx + Vec2::splat(1.0)], shadow);
        painter.line_segment([p + Vec2::splat(1.0), vy + Vec2::splat(1.0)], shadow);
        painter.line_segment([p, hx], s);
        painter.line_segment([p, vy], s);
        painter.line_segment([p + Vec2::new(sx * 3.0, sy * 3.0), hx], inner);
        painter.line_segment([p + Vec2::new(sx * 3.0, sy * 3.0), vy], inner);
    }
}

pub fn paint_pressed_panel_lines(painter: &Painter, rect: Rect) {
    if rect.width() <= 20.0 || rect.height() <= 18.0 {
        return;
    }

    let inner = rect.shrink(4.0);
    painter.hline(
        inner.left()..=inner.right(),
        inner.top(),
        Stroke::new(1.0, Color32::from_black_alpha(220)),
    );
    painter.hline(
        inner.left()..=inner.right(),
        inner.top() + 1.0,
        Stroke::new(1.0, Color32::from_white_alpha(4)),
    );
    painter.hline(
        inner.left()..=inner.right(),
        inner.bottom(),
        Stroke::new(1.0, Color32::from_black_alpha(235)),
    );
    painter.line_segment(
        [inner.left_top(), inner.left_bottom()],
        Stroke::new(1.0, Color32::from_black_alpha(210)),
    );
    painter.line_segment(
        [inner.right_top(), inner.right_bottom()],
        Stroke::new(1.0, Color32::from_black_alpha(235)),
    );
}

pub fn paint_inner_glow(painter: &Painter, rect: Rect, top_alpha: u8, bot_alpha: u8) {
    let top_band = Rect::from_min_max(
        rect.min,
        Pos2::new(rect.max.x, rect.min.y + rect.height() * 0.45),
    );
    paint_vertical_gradient_mesh(
        painter,
        top_band,
        Color32::from_white_alpha(top_alpha),
        Color32::from_white_alpha(0),
    );

    let bot_band = Rect::from_min_max(
        Pos2::new(rect.min.x, rect.max.y - rect.height() * 0.45),
        rect.max,
    );
    paint_vertical_gradient_mesh(
        painter,
        bot_band,
        Color32::from_black_alpha(0),
        Color32::from_black_alpha(bot_alpha),
    );
}

pub fn paint_micro_scratches(painter: &Painter, rect: Rect, count: usize) {
    if rect.width() <= 2.0 || rect.height() <= 2.0 || count == 0 {
        return;
    }

    let seed = (rect.min.x as i32).wrapping_mul(73) ^ (rect.min.y as i32).wrapping_mul(151);
    let visible_count = ((count as f32) * 0.48).ceil() as usize;
    for i in 0..visible_count {
        let h = seed
            .wrapping_add((i as i32).wrapping_mul(1_103_515_245))
            .wrapping_mul(12_345) as u32;
        let tx = ((h & 0xff) as f32) / 255.0;
        let ty = (((h >> 8) & 0xff) as f32) / 255.0;
        let len = 5.0 + (((h >> 16) & 0x0f) as f32) * 0.85;
        let x = rect.min.x + rect.width() * tx;
        let y = rect.min.y + rect.height() * ty;
        let dx = if i % 4 == 0 { -len * 0.45 } else { len };
        let dy = (((h >> 20) & 0x07) as f32 - 3.0) * 0.22;
        let dark_alpha = 16 + ((h >> 24) & 0x0b) as u8;
        let light_alpha = 4 + ((h >> 28) & 0x07) as u8;
        let end = Pos2::new(
            (x + dx).clamp(rect.min.x, rect.max.x),
            (y + dy).clamp(rect.min.y, rect.max.y),
        );
        painter.line_segment(
            [Pos2::new(x, y + 0.7), Pos2::new(end.x, end.y + 0.7)],
            Stroke::new(0.8, Color32::from_black_alpha(dark_alpha)),
        );
        painter.line_segment(
            [Pos2::new(x, y), end],
            Stroke::new(0.55, Color32::from_white_alpha(light_alpha)),
        );
    }
}

pub fn paint_panel_vignette(painter: &Painter, rect: Rect) {
    let left = Rect::from_min_size(rect.min, Vec2::new(rect.width() * 0.20, rect.height()));
    paint_horizontal_gradient_mesh(
        painter,
        left,
        Color32::from_black_alpha(110),
        Color32::from_black_alpha(0),
    );
    let right = Rect::from_min_max(
        Pos2::new(rect.max.x - rect.width() * 0.20, rect.min.y),
        rect.max,
    );
    paint_horizontal_gradient_mesh(
        painter,
        right,
        Color32::from_black_alpha(0),
        Color32::from_black_alpha(115),
    );
    let bottom = Rect::from_min_max(
        Pos2::new(rect.min.x, rect.max.y - rect.height() * 0.24),
        rect.max,
    );
    paint_vertical_gradient_mesh(
        painter,
        bottom,
        Color32::from_black_alpha(0),
        Color32::from_black_alpha(145),
    );
}

pub fn paint_radial_corner_vignette(painter: &Painter, rect: Rect, alpha: u8) {
    if rect.width() <= 16.0 || rect.height() <= 16.0 || alpha == 0 {
        return;
    }

    let w = (rect.width() * 0.24).clamp(10.0, 80.0);
    let h = (rect.height() * 0.30).clamp(10.0, 80.0);
    let corners = [
        Rect::from_min_size(rect.min, Vec2::new(w, h)),
        Rect::from_min_size(Pos2::new(rect.max.x - w, rect.min.y), Vec2::new(w, h)),
        Rect::from_min_size(Pos2::new(rect.min.x, rect.max.y - h), Vec2::new(w, h)),
        Rect::from_min_size(Pos2::new(rect.max.x - w, rect.max.y - h), Vec2::new(w, h)),
    ];
    for corner_rect in corners {
        painter.rect_filled(
            corner_rect,
            CornerRadius::same(0),
            Color32::from_black_alpha(alpha / 5),
        );
    }
}

pub fn paint_speckle(painter: &Painter, rect: Rect, count: usize, alpha_base: u8) {
    if rect.width() <= 4.0 || rect.height() <= 4.0 || count == 0 {
        return;
    }

    let seed = (rect.min.x as i32).wrapping_mul(211) ^ (rect.min.y as i32).wrapping_mul(127);
    for i in 0..count {
        let h = seed
            .wrapping_add((i as i32).wrapping_mul(1_664_525))
            .wrapping_mul(1_013_904_223) as u32;
        let tx = ((h & 0xff) as f32) / 255.0;
        let ty = (((h >> 8) & 0xff) as f32) / 255.0;
        let x = rect.min.x + rect.width() * tx;
        let y = rect.min.y + rect.height() * ty;
        let r = 0.45 + (((h >> 16) & 0x03) as f32) * 0.15;
        let alpha = alpha_base.saturating_add(((h >> 24) & 0x05) as u8);
        let col = if i % 3 == 0 {
            Color32::from_white_alpha(alpha / 2)
        } else {
            Color32::from_black_alpha(alpha.saturating_mul(3))
        };
        painter.circle_filled(Pos2::new(x, y), r, col);
    }
}

pub fn paint_top_highlight(painter: &Painter, rect: Rect, accent: Color32) {
    painter.hline(
        (rect.min.x + spacing::S3)..=(rect.max.x - spacing::S3),
        rect.min.y + 1.0,
        Stroke::new(1.0, accent),
    );
}

pub fn paint_hero_gradient(painter: &Painter, rect: Rect, corner: f32) {
    painter.rect_filled(rect, CornerRadius::same(corner as u8), palette::IRON_BLACK);
    paint_multistop_gradient(
        painter,
        rect,
        &[
            (
                0.00,
                Color32::from_rgba_premultiplied(0x22, 0x24, 0x20, 218),
            ),
            (
                0.12,
                Color32::from_rgba_premultiplied(0x12, 0x14, 0x12, 230),
            ),
            (
                0.38,
                Color32::from_rgba_premultiplied(0x08, 0x0a, 0x09, 240),
            ),
            (
                0.68,
                Color32::from_rgba_premultiplied(0x04, 0x05, 0x05, 246),
            ),
            (1.00, Color32::from_black_alpha(252)),
        ],
    );
    paint_burnished_steel_surface(painter, rect.shrink(3.0), 0.10);
    paint_plate_grain(painter, rect.shrink(3.0), 3.0, 3);
    paint_speckle(painter, rect.shrink(6.0), 26, 3);
    paint_inner_glow(painter, rect, 6, 96);
}

pub fn paint_hero_seals(painter: &Painter, rect: Rect) {
    let inset = spacing::S6;
    painter.hline(
        (rect.min.x + inset)..=(rect.max.x - inset),
        rect.min.y + spacing::S4,
        Stroke::new(1.0, Color32::from_white_alpha(12)),
    );
    painter.hline(
        (rect.min.x + inset + 6.0)..=(rect.max.x - inset - 6.0),
        rect.min.y + spacing::S4 + 3.0,
        Stroke::new(2.0, Color32::from_black_alpha(210)),
    );
    painter.hline(
        (rect.min.x + inset + 6.0)..=(rect.max.x - inset - 6.0),
        rect.max.y - spacing::S4 - 3.0,
        Stroke::new(2.0, Color32::from_black_alpha(224)),
    );
    painter.hline(
        (rect.min.x + inset)..=(rect.max.x - inset),
        rect.max.y - spacing::S4,
        Stroke::new(1.0, palette::BRASS_DARK),
    );
}

pub fn paint_ribbon(painter: &Painter, rect: Rect, accent: Color32) {
    let h = rect.height();
    let chevron = h * 0.5;
    let body = Rect::from_min_max(
        Pos2::new(rect.min.x + chevron, rect.min.y),
        Pos2::new(rect.max.x - chevron, rect.max.y),
    );

    paint_vertical_gradient_mesh(painter, body, accent, darker(accent, 0.58));
    let left = vec![
        Pos2::new(rect.min.x, rect.min.y + h * 0.5),
        Pos2::new(rect.min.x + chevron, rect.min.y),
        Pos2::new(rect.min.x + chevron, rect.max.y),
    ];
    painter.add(Shape::Path(PathShape {
        points: left,
        closed: true,
        fill: darker(accent, 0.72),
        stroke: PathStroke::NONE,
    }));
    let right = vec![
        Pos2::new(rect.max.x, rect.min.y + h * 0.5),
        Pos2::new(rect.max.x - chevron, rect.min.y),
        Pos2::new(rect.max.x - chevron, rect.max.y),
    ];
    painter.add(Shape::Path(PathShape {
        points: right,
        closed: true,
        fill: darker(accent, 0.72),
        stroke: PathStroke::NONE,
    }));
    painter.hline(
        body.min.x..=body.max.x,
        rect.min.y + 1.0,
        Stroke::new(1.0, Color32::from_white_alpha(24)),
    );
    painter.hline(
        body.min.x..=body.max.x,
        rect.max.y - 1.0,
        Stroke::new(1.0, Color32::from_black_alpha(170)),
    );
}

fn darker(c: Color32, t: f32) -> Color32 {
    let r = (c.r() as f32 * t).clamp(0.0, 255.0) as u8;
    let g = (c.g() as f32 * t).clamp(0.0, 255.0) as u8;
    let b = (c.b() as f32 * t).clamp(0.0, 255.0) as u8;
    Color32::from_rgba_premultiplied(r, g, b, c.a())
}

fn scaled_alpha(alpha: u8, strength: f32) -> u8 {
    ((alpha as f32) * strength).round().clamp(0.0, 255.0) as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::Vec2;

    #[test]
    fn shadow_skipped_when_e0() {
        assert_eq!(Elevation::E0.shadow_alpha(), 0.0);
        assert_eq!(Elevation::E0.shadow_offset(), 0.0);
    }

    #[test]
    fn ribbon_geometry_is_consistent() {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(200.0, 30.0));
        let chevron = rect.height() * 0.5;
        assert_eq!(chevron, 15.0);
        assert_eq!(rect.width() - 2.0 * chevron, 170.0);
    }

    #[test]
    fn darker_preserves_alpha() {
        let c = Color32::from_rgba_premultiplied(0xc0, 0x80, 0x40, 0xff);
        let d = darker(c, 0.5);
        assert_eq!(d.a(), 0xff);
        assert!(d.r() < c.r());
    }
}
