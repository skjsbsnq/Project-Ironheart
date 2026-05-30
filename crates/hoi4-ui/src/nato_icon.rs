//! NATO 师符 widget —— ROADMAP_MILITARY_UI_PARITY.md Phase B。
//!
//! 程序化绘制(`egui::Painter`)师团 NATO 标识小图。供左侧军团详情面板的
//! 师列表使用,后续也可用于其他需要 NATO 符号的 UI 位置。
//!
//! 没有引入额外资产或 GPU 纹理,所有图形都是直线/矩形/椭圆/小字组合。

use egui::{Color32, FontId, Pos2, Rect, Stroke, Vec2};

const STROKE_W: f32 = 1.2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NatoArchetype {
    Infantry,
    Cavalry,
    Motorized,
    Mechanized,
    Armor,
    Mountain,
    Marine,
    Paratrooper,
    Artillery,
    AntiTank,
    AntiAir,
    Garrison,
    Unknown,
}

/// 按师模板名匹配 archetype。中英西多语言常见词都覆盖。
pub fn archetype_from_template_name(name: &str) -> NatoArchetype {
    let n = name.to_lowercase();
    // 顺序:更具体的优先于更宽泛的(例如"装甲"优先于"步兵")
    if contains_any(
        &n,
        &[
            "装甲师",
            "装甲",
            "armor",
            "armour",
            "tank",
            "panzer",
            "blindad",
        ],
    ) {
        NatoArchetype::Armor
    } else if contains_any(&n, &["机械化", "mechan"]) {
        NatoArchetype::Mechanized
    } else if contains_any(&n, &["摩托化", "motor", "truck", "motoriz"]) {
        NatoArchetype::Motorized
    } else if contains_any(&n, &["山地", "mountain", "alpin", "montaña", "montana"]) {
        NatoArchetype::Mountain
    } else if contains_any(&n, &["伞兵", "para", "fallschirm"]) {
        NatoArchetype::Paratrooper
    } else if contains_any(&n, &["海军陆战", "marine", "infantería de marina"]) {
        NatoArchetype::Marine
    } else if contains_any(&n, &["骑兵", "cavalry", "cavalería", "caballería"]) {
        NatoArchetype::Cavalry
    } else if contains_any(&n, &["反坦克", "anti-tank", "antitank"]) {
        NatoArchetype::AntiTank
    } else if contains_any(&n, &["防空", "anti-air", "antiair", "anti-aircraft"]) {
        NatoArchetype::AntiAir
    } else if contains_any(&n, &["火炮", "artiller", "artillería"]) {
        NatoArchetype::Artillery
    } else if contains_any(&n, &["驻防", "garrison", "comandancia"]) {
        NatoArchetype::Garrison
    } else if contains_any(
        &n,
        &[
            "步兵",
            "infantry",
            "infanter",
            "división",
            "division",
            "división",
            "brigada",
            "brigade",
        ],
    ) {
        NatoArchetype::Infantry
    } else {
        NatoArchetype::Unknown
    }
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|n| haystack.contains(n))
}

pub fn draw_nato_symbol(
    painter: &egui::Painter,
    rect: Rect,
    archetype: NatoArchetype,
    country_color: Color32,
) {
    let fill = tint(country_color, 0.55);
    let stroke = Stroke::new(STROKE_W, darken(country_color, 0.5));
    painter.rect_filled(rect, 1.5, fill);
    painter.rect_stroke(rect, 1.5, stroke, egui::epaint::StrokeKind::Inside);

    let inner_color = darken(country_color, 0.7);
    let inner_stroke = Stroke::new(STROKE_W, inner_color);
    let inset = rect.shrink2(Vec2::new(rect.width() * 0.18, rect.height() * 0.22));

    match archetype {
        NatoArchetype::Infantry => {
            painter.line_segment([inset.left_top(), inset.right_bottom()], inner_stroke);
            painter.line_segment([inset.left_bottom(), inset.right_top()], inner_stroke);
        }
        NatoArchetype::Cavalry => {
            painter.line_segment([inset.left_bottom(), inset.right_top()], inner_stroke);
        }
        NatoArchetype::Motorized => {
            // 一条横线 + 上方一个圆点
            let mid_y = inset.center().y + inset.height() * 0.18;
            painter.line_segment(
                [
                    Pos2::new(inset.left(), mid_y),
                    Pos2::new(inset.right(), mid_y),
                ],
                inner_stroke,
            );
            painter.circle_filled(
                Pos2::new(inset.center().x, inset.center().y - inset.height() * 0.18),
                inset.height() * 0.13,
                inner_color,
            );
        }
        NatoArchetype::Mechanized => {
            // X + 底部一条粗履带线
            painter.line_segment([inset.left_top(), inset.right_bottom()], inner_stroke);
            painter.line_segment([inset.left_bottom(), inset.right_top()], inner_stroke);
            let bottom_y = inset.bottom() + 0.6;
            painter.line_segment(
                [
                    Pos2::new(inset.left(), bottom_y),
                    Pos2::new(inset.right(), bottom_y),
                ],
                Stroke::new(STROKE_W + 0.6, inner_color),
            );
        }
        NatoArchetype::Armor => {
            // 椭圆 = 矩形 + 圆角接近椭圆
            let cx = inset.center().x;
            let cy = inset.center().y;
            let rx = inset.width() * 0.40;
            let ry = inset.height() * 0.28;
            draw_ellipse(painter, Pos2::new(cx, cy), rx, ry, inner_stroke, 18);
        }
        NatoArchetype::Mountain => {
            // ^
            let mid_x = inset.center().x;
            let peak_y = inset.top() + inset.height() * 0.15;
            let base_y = inset.bottom() - inset.height() * 0.05;
            painter.line_segment(
                [
                    Pos2::new(inset.left() + inset.width() * 0.10, base_y),
                    Pos2::new(mid_x, peak_y),
                ],
                inner_stroke,
            );
            painter.line_segment(
                [
                    Pos2::new(mid_x, peak_y),
                    Pos2::new(inset.right() - inset.width() * 0.10, base_y),
                ],
                inner_stroke,
            );
        }
        NatoArchetype::Marine => {
            // 锚:竖线 + 半圆底 + 顶部横线
            let cx = inset.center().x;
            let top_y = inset.top() + inset.height() * 0.15;
            let bot_y = inset.bottom() - inset.height() * 0.20;
            painter.line_segment([Pos2::new(cx, top_y), Pos2::new(cx, bot_y)], inner_stroke);
            painter.line_segment(
                [
                    Pos2::new(cx - inset.width() * 0.20, top_y),
                    Pos2::new(cx + inset.width() * 0.20, top_y),
                ],
                inner_stroke,
            );
            painter.line_segment(
                [
                    Pos2::new(cx - inset.width() * 0.25, bot_y),
                    Pos2::new(cx + inset.width() * 0.25, bot_y),
                ],
                inner_stroke,
            );
        }
        NatoArchetype::Paratrooper => {
            // 倒三角(降落伞简化)
            painter.line_segment(
                [
                    Pos2::new(inset.left(), inset.top() + inset.height() * 0.20),
                    Pos2::new(inset.center().x, inset.bottom() - inset.height() * 0.10),
                ],
                inner_stroke,
            );
            painter.line_segment(
                [
                    Pos2::new(inset.right(), inset.top() + inset.height() * 0.20),
                    Pos2::new(inset.center().x, inset.bottom() - inset.height() * 0.10),
                ],
                inner_stroke,
            );
            painter.line_segment(
                [
                    Pos2::new(inset.left(), inset.top() + inset.height() * 0.20),
                    Pos2::new(inset.right(), inset.top() + inset.height() * 0.20),
                ],
                inner_stroke,
            );
        }
        NatoArchetype::Artillery => {
            painter.circle_filled(inset.center(), inset.height() * 0.22, inner_color);
        }
        NatoArchetype::AntiTank => {
            painter.text(
                inset.center(),
                egui::Align2::CENTER_CENTER,
                "AT",
                FontId::proportional(inset.height() * 0.72),
                inner_color,
            );
        }
        NatoArchetype::AntiAir => {
            // ^ + 圆点
            let mid_x = inset.center().x;
            painter.line_segment(
                [
                    Pos2::new(inset.left() + inset.width() * 0.10, inset.center().y),
                    Pos2::new(mid_x, inset.top() + inset.height() * 0.15),
                ],
                inner_stroke,
            );
            painter.line_segment(
                [
                    Pos2::new(mid_x, inset.top() + inset.height() * 0.15),
                    Pos2::new(inset.right() - inset.width() * 0.10, inset.center().y),
                ],
                inner_stroke,
            );
            painter.circle_filled(
                Pos2::new(mid_x, inset.bottom() - inset.height() * 0.18),
                inset.height() * 0.12,
                inner_color,
            );
        }
        NatoArchetype::Garrison => {
            // 实心填充内部
            let inner_fill = Rect::from_center_size(
                inset.center(),
                Vec2::new(inset.width() * 0.55, inset.height() * 0.55),
            );
            painter.rect_filled(inner_fill, 0.0, inner_color);
        }
        NatoArchetype::Unknown => {
            // 仅空框,无内符
        }
    }
}

fn draw_ellipse(
    painter: &egui::Painter,
    center: Pos2,
    rx: f32,
    ry: f32,
    stroke: Stroke,
    segments: usize,
) {
    let segments = segments.max(8);
    let mut prev = Pos2::new(center.x + rx, center.y);
    for i in 1..=segments {
        let theta = (i as f32) * std::f32::consts::TAU / (segments as f32);
        let next = Pos2::new(center.x + rx * theta.cos(), center.y + ry * theta.sin());
        painter.line_segment([prev, next], stroke);
        prev = next;
    }
}

fn tint(c: Color32, amount: f32) -> Color32 {
    let amount = amount.clamp(0.0, 1.0);
    let blend = |x: u8| {
        (x as f32 + (255.0 - x as f32) * amount)
            .round()
            .clamp(0.0, 255.0) as u8
    };
    Color32::from_rgb(blend(c.r()), blend(c.g()), blend(c.b()))
}

fn darken(c: Color32, amount: f32) -> Color32 {
    let amount = amount.clamp(0.0, 1.0);
    let dim = |x: u8| ((x as f32) * (1.0 - amount)).round().clamp(0.0, 255.0) as u8;
    Color32::from_rgb(dim(c.r()), dim(c.g()), dim(c.b()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_common_template_names() {
        assert_eq!(
            archetype_from_template_name("步兵师"),
            NatoArchetype::Infantry
        );
        assert_eq!(archetype_from_template_name("装甲师"), NatoArchetype::Armor);
        assert_eq!(
            archetype_from_template_name("摩托化师"),
            NatoArchetype::Motorized
        );
        assert_eq!(
            archetype_from_template_name("机械化师"),
            NatoArchetype::Mechanized
        );
        assert_eq!(
            archetype_from_template_name("山地师"),
            NatoArchetype::Mountain
        );
        assert_eq!(
            archetype_from_template_name("骑兵旅"),
            NatoArchetype::Cavalry
        );
        assert_eq!(
            archetype_from_template_name("海军陆战队"),
            NatoArchetype::Marine
        );
        assert_eq!(
            archetype_from_template_name("伞兵师"),
            NatoArchetype::Paratrooper
        );
        assert_eq!(
            archetype_from_template_name("驻防旅"),
            NatoArchetype::Garrison
        );
    }

    #[test]
    fn maps_english_and_spanish_names() {
        assert_eq!(
            archetype_from_template_name("Infantry Division"),
            NatoArchetype::Infantry
        );
        assert_eq!(
            archetype_from_template_name("División de Infantería"),
            NatoArchetype::Infantry
        );
        assert_eq!(
            archetype_from_template_name("Panzer Division"),
            NatoArchetype::Armor
        );
        assert_eq!(
            archetype_from_template_name("Comandancia de Baleares"),
            NatoArchetype::Garrison
        );
        assert_eq!(
            archetype_from_template_name("Brigada Mixta Montaña"),
            NatoArchetype::Mountain
        );
    }

    #[test]
    fn unknown_falls_back() {
        assert_eq!(
            archetype_from_template_name("某种神秘部队"),
            NatoArchetype::Unknown
        );
        assert_eq!(archetype_from_template_name(""), NatoArchetype::Unknown);
    }

    #[test]
    fn tint_darken_round_trip_safe() {
        let c = Color32::from_rgb(100, 150, 200);
        let t = tint(c, 0.5);
        assert!(t.r() >= c.r() && t.g() >= c.g() && t.b() >= c.b());
        let d = darken(c, 0.5);
        assert!(d.r() <= c.r() && d.g() <= c.g() && d.b() <= c.b());
    }
}
