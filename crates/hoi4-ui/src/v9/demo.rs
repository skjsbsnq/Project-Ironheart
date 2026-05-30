//! V9 demo 页：同屏展示 token 调色板 + 7 种 FrameStyle + 9 种 TextRole + 5 个 primitive。
//!
//! 入口：F12 切换 [`V9Demo::open`]。外壳走自绘 `egui::Area`，内部走自研
//! GridLayout 锁死布局，验证 V9 反"egui 决定尺寸"的设计原则。

use egui::{
    Align2, Area, Color32, Context, Id, Order, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2,
};

use super::frame::{FrameStyle, PanelFrame};
use super::layout::{GridLayout, Track};
use super::paint;
use super::primitives::{
    Button, ButtonSize, ButtonVariant, Card, ListItem, ListView, TabBar, TabItem, Tile, TileTrend,
};
use super::tokens::{palette, radius, spacing, Elevation, TextRole};

/// V9 demo 页的持久 UI 状态。
pub struct V9Demo {
    pub open: bool,
    active_tab: &'static str,
    list_selected: Option<&'static str>,
}

impl Default for V9Demo {
    fn default() -> Self {
        Self {
            open: false,
            active_tab: "tab_overview",
            list_selected: Some("ger"),
        }
    }
}

impl V9Demo {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn show(&mut self, ctx: &Context) {
        if !self.open {
            return;
        }
        let screen = ctx.screen_rect();
        let size = demo_size(screen);
        let pos = Pos2::new(screen.left() + spacing::S7, screen.top() + spacing::S7);
        let mut should_close = false;

        Area::new(Id::new("v9_design_system_demo"))
            .order(Order::Foreground)
            .fixed_pos(pos)
            .default_size(size)
            .show(ctx, |ui| {
                let (canvas, _) = ui.allocate_exact_size(size, Sense::hover());
                self.draw_shell(ui, canvas, &mut should_close);
            });

        if should_close {
            self.open = false;
        }
    }

    fn draw_shell(&mut self, ui: &mut egui::Ui, canvas: Rect, should_close: &mut bool) {
        let frame = PanelFrame::new(FrameStyle::Hero, canvas)
            .with_accent(palette::BRASS)
            .with_elevation(Elevation::E3);
        frame.draw(ui.painter());

        let inner = frame.inner_rect();
        let title_h = 40.0;
        let title_rect =
            Rect::from_min_max(inner.min, Pos2::new(inner.max.x, inner.min.y + title_h));
        self.draw_title_bar(ui, title_rect, should_close);

        let body_outer = Rect::from_min_max(
            Pos2::new(inner.min.x, title_rect.max.y + spacing::S4),
            inner.max,
        );
        paint::paint_recessed_panel(ui.painter(), body_outer, radius::R2);
        let canvas = body_outer.shrink2(Vec2::new(spacing::S5, spacing::S5));

        // 整体 3 行：调色 + 字号 / 外框 / primitive
        let palette_h = (canvas.height() * 0.27).clamp(150.0, 180.0);
        let frame_h = (canvas.height() * 0.36).clamp(220.0, 280.0);
        let outer = GridLayout::new(
            vec![
                Track::Fixed(palette_h),
                Track::Fixed(frame_h),
                Track::Fr(1.0),
            ],
            vec![Track::Fr(1.0)],
        )
        .with_gutter(0.0, spacing::S5);
        let cells = outer.measure(canvas);
        self.draw_palette_and_typography(ui, GridLayout::cell(&cells, 0, 0));
        self.draw_frame_showcase(ui, GridLayout::cell(&cells, 1, 0));
        self.draw_primitives(ui, GridLayout::cell(&cells, 2, 0));
    }

    fn draw_title_bar(&self, ui: &mut egui::Ui, rect: Rect, should_close: &mut bool) {
        paint::paint_vertical_gradient_mesh(
            ui.painter(),
            rect,
            Color32::from_rgba_premultiplied(0x28, 0x2a, 0x25, 226),
            Color32::from_rgba_premultiplied(0x05, 0x06, 0x05, 248),
        );
        paint::paint_plate_grain(ui.painter(), rect.shrink(2.0), 3.0, 3);
        paint::paint_speckle(ui.painter(), rect.shrink(3.0), 16, 2);
        ui.painter().rect_stroke(
            rect,
            egui::epaint::CornerRadius::same(radius::R1 as u8),
            Stroke::new(1.0, palette::EDGE_DARK),
            StrokeKind::Inside,
        );
        ui.painter().rect_stroke(
            rect.shrink(2.0),
            egui::epaint::CornerRadius::same(radius::R1 as u8),
            Stroke::new(1.0, Color32::from_black_alpha(230)),
            StrokeKind::Inside,
        );
        ui.painter().hline(
            (rect.left() + spacing::S3)..=(rect.right() - spacing::S3),
            rect.top() + 1.0,
            Stroke::new(1.0, Color32::from_white_alpha(8)),
        );
        ui.painter().hline(
            (rect.left() + spacing::S3)..=(rect.right() - spacing::S3),
            rect.bottom() - 1.0,
            Stroke::new(1.5, palette::BRASS_DARK),
        );

        let flag = Rect::from_min_max(
            rect.min + Vec2::new(spacing::S4, spacing::S3),
            Pos2::new(rect.min.x + 58.0, rect.max.y - spacing::S3),
        );
        paint::paint_bevel(
            ui.painter(),
            flag,
            palette::PANEL_DEEP,
            palette::BRASS,
            radius::R1,
        );
        let stripe_w = flag.width() / 3.0;
        let stripe = Rect::from_min_size(
            flag.min + Vec2::splat(3.0),
            Vec2::new(stripe_w - 2.0, flag.height() - 6.0),
        );
        ui.painter().rect_filled(
            stripe,
            egui::epaint::CornerRadius::ZERO,
            palette::IDEO_FASCISM,
        );
        ui.painter().rect_filled(
            stripe.translate(Vec2::new(stripe_w, 0.0)),
            egui::epaint::CornerRadius::ZERO,
            palette::PARCHMENT,
        );
        ui.painter().rect_filled(
            stripe.translate(Vec2::new(stripe_w * 2.0, 0.0)),
            egui::epaint::CornerRadius::ZERO,
            palette::IDEO_FASCISM,
        );

        ui.painter().text(
            Pos2::new(flag.max.x + spacing::S4, rect.center().y),
            Align2::LEFT_CENTER,
            "V9 Frontend - Design System Demo",
            TextRole::Display.font_id(),
            palette::BRASS_BRIGHT,
        );
        ui.painter().text(
            Pos2::new(rect.right() - 52.0, rect.center().y),
            Align2::RIGHT_CENTER,
            "F12",
            TextRole::Code.font_id(),
            palette::PARCHMENT_DIM,
        );

        let close_rect = Rect::from_min_size(
            Pos2::new(rect.right() - 34.0, rect.top() + 6.0),
            Vec2::new(26.0, 26.0),
        );
        let resp = ui.interact(close_rect, ui.id().with("v9_demo_close"), Sense::click());
        let fill = if resp.hovered() {
            palette::BRASS_DARK
        } else {
            palette::PANEL_DEEP
        };
        paint::paint_bevel(ui.painter(), close_rect, fill, palette::BRASS, radius::R1);
        ui.painter().text(
            close_rect.center(),
            Align2::CENTER_CENTER,
            "X",
            TextRole::Subheading.font_id(),
            if resp.hovered() {
                palette::GOLD_HOT
            } else {
                palette::PARCHMENT_DIM
            },
        );
        if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if resp.clicked() {
            *should_close = true;
        }
    }

    fn draw_palette_and_typography(&self, ui: &mut egui::Ui, rect: Rect) {
        // 左右两半：palette / typography
        let split = GridLayout::new(vec![Track::Fr(1.0)], vec![Track::Fr(1.0), Track::Fr(1.0)])
            .with_gutter(spacing::S5, 0.0);
        let cells = split.measure(rect);

        // 调色板
        let pal_rect = GridLayout::cell(&cells, 0, 0);
        Card::new().draw(ui.painter(), pal_rect);
        ui.painter().text(
            Pos2::new(pal_rect.min.x + spacing::S5, pal_rect.min.y + spacing::S4),
            Align2::LEFT_TOP,
            "Palette",
            TextRole::Display.font_id(),
            palette::BRASS_BRIGHT,
        );
        let swatches: &[(&str, Color32)] = &[
            ("CANVAS_DEEP", palette::CANVAS_DEEP),
            ("PANEL", palette::PANEL),
            ("PANEL_SOFT", palette::PANEL_SOFT),
            ("PARCHMENT", palette::PARCHMENT),
            ("BRASS_DARK", palette::BRASS_DARK),
            ("BRASS", palette::BRASS),
            ("BRASS_BRIGHT", palette::BRASS_BRIGHT),
            ("GOLD", palette::GOLD),
            ("GOLD_HOT", palette::GOLD_HOT),
            ("FASCISM", palette::IDEO_FASCISM),
            ("DEMOCRATIC", palette::IDEO_DEMOCRATIC),
            ("COMMUNISM", palette::IDEO_COMMUNISM),
            ("NEUTRALITY", palette::IDEO_NEUTRALITY),
            ("COLD_STEEL", palette::COLD_STEEL),
            ("COLD_SIGNAL", palette::COLD_SIGNAL),
            ("COLD_ATOMIC", palette::COLD_ATOMIC),
            ("GOOD", palette::GOOD),
            ("WARN", palette::WARN),
            ("BAD", palette::BAD),
            ("INFO", palette::INFO),
        ];
        let cols = 5;
        let rows = (swatches.len() + cols - 1) / cols;
        let grid = GridLayout::new(
            (0..rows).map(|_| Track::Fr(1.0)).collect(),
            (0..cols).map(|_| Track::Fr(1.0)).collect(),
        )
        .with_gutter(spacing::S2, spacing::S2);
        let inner = Rect::from_min_max(
            Pos2::new(pal_rect.min.x + spacing::S5, pal_rect.min.y + spacing::S8),
            Pos2::new(pal_rect.max.x - spacing::S5, pal_rect.max.y - spacing::S4),
        );
        let pal_cells = grid.measure(inner);
        for (i, (name, color)) in swatches.iter().enumerate() {
            let cell = GridLayout::cell(&pal_cells, i / cols, i % cols);
            ui.painter()
                .rect_filled(cell, egui::epaint::CornerRadius::same(2), *color);
            ui.painter().text(
                Pos2::new(cell.min.x + 4.0, cell.min.y + 2.0),
                Align2::LEFT_TOP,
                name,
                TextRole::Small.font_id(),
                if color.r() as u16 + color.g() as u16 + color.b() as u16 > 380 {
                    palette::CANVAS_DEEP
                } else {
                    palette::PARCHMENT
                },
            );
        }

        // 字号档位
        let type_rect = GridLayout::cell(&cells, 0, 1);
        Card::new().draw(ui.painter(), type_rect);
        ui.painter().text(
            Pos2::new(type_rect.min.x + spacing::S5, type_rect.min.y + spacing::S4),
            Align2::LEFT_TOP,
            "Typography",
            TextRole::Display.font_id(),
            palette::BRASS_BRIGHT,
        );
        let roles = [
            (TextRole::Title, "Title 32"),
            (TextRole::Display, "Display 22"),
            (TextRole::Heading, "Heading 17"),
            (TextRole::Subheading, "Subheading 14"),
            (TextRole::Body, "Body 12 — 钢铁雄心"),
            (TextRole::Caption, "Caption 11"),
            (TextRole::Small, "Small 10"),
            (TextRole::Numeric, "Numeric 13 · 1234.56"),
            (TextRole::Code, "Code 11 · GFX_flag_DEU"),
        ];
        let mut y = type_rect.min.y + spacing::S8;
        for (role, sample) in roles {
            ui.painter().text(
                Pos2::new(type_rect.min.x + spacing::S5, y),
                Align2::LEFT_TOP,
                sample,
                role.font_id(),
                palette::PARCHMENT,
            );
            y += role.size() + 4.0;
        }
    }

    fn draw_frame_showcase(&self, ui: &mut egui::Ui, rect: Rect) {
        // 标题
        ui.painter().text(
            Pos2::new(rect.min.x, rect.min.y),
            Align2::LEFT_TOP,
            "FrameStyle × 7",
            TextRole::Display.font_id(),
            palette::BRASS_BRIGHT,
        );
        let body = Rect::from_min_max(Pos2::new(rect.min.x, rect.min.y + spacing::S7), rect.max);
        // 7 个外框 — 一行 4 + 一行 3
        let grid = GridLayout::new(
            vec![Track::Fr(1.0), Track::Fr(1.0)],
            vec![
                Track::Fr(1.0),
                Track::Fr(1.0),
                Track::Fr(1.0),
                Track::Fr(1.0),
            ],
        )
        .with_gutter(spacing::S5, spacing::S5);
        let cells = grid.measure(body);

        let styles = [
            (FrameStyle::Card, "Card"),
            (FrameStyle::Panel, "Panel"),
            (FrameStyle::Ornate, "Ornate"),
            (FrameStyle::Glass, "Glass"),
            (FrameStyle::Modal, "Modal"),
            (FrameStyle::Ribbon, "Ribbon"),
            (FrameStyle::Hero, "Hero"),
        ];
        for (i, (style, name)) in styles.iter().enumerate() {
            let cell = GridLayout::cell(&cells, i / 4, i % 4);
            let frame = if *style == FrameStyle::Ribbon {
                // ribbon 不能整格画，只画一条横幅
                let stripe = Rect::from_min_max(
                    Pos2::new(cell.min.x, cell.center().y - 14.0),
                    Pos2::new(cell.max.x, cell.center().y + 14.0),
                );
                PanelFrame::new(*style, stripe).with_accent(palette::IDEO_FASCISM)
            } else {
                PanelFrame::new(*style, cell)
            };
            frame.draw(ui.painter());
            ui.painter().text(
                cell.center(),
                Align2::CENTER_CENTER,
                name,
                TextRole::Subheading.font_id(),
                if *style == FrameStyle::Ribbon {
                    palette::PARCHMENT
                } else {
                    palette::PARCHMENT
                },
            );
        }
    }

    fn draw_primitives(&mut self, ui: &mut egui::Ui, rect: Rect) {
        ui.painter().text(
            Pos2::new(rect.min.x, rect.min.y),
            Align2::LEFT_TOP,
            "Primitives — Button / Tile / Tabs / List / Card",
            TextRole::Display.font_id(),
            palette::BRASS_BRIGHT,
        );
        let body = Rect::from_min_max(Pos2::new(rect.min.x, rect.min.y + spacing::S7), rect.max);
        // 左右 split：左 = 按钮 + tile + tabs，右 = list + card
        let grid = GridLayout::new(vec![Track::Fr(1.0)], vec![Track::Fr(1.0), Track::Fr(1.0)])
            .with_gutter(spacing::S5, 0.0);
        let cells = grid.measure(body);
        let left = GridLayout::cell(&cells, 0, 0);
        let right = GridLayout::cell(&cells, 0, 1);

        self.draw_left_column(ui, left);
        self.draw_right_column(ui, right);
    }

    fn draw_left_column(&mut self, ui: &mut egui::Ui, rect: Rect) {
        // 3 行：按钮 / tile 网格 / tabs
        let grid = GridLayout::new(
            vec![Track::Fixed(96.0), Track::Fixed(72.0), Track::Fixed(48.0)],
            vec![Track::Fr(1.0)],
        )
        .with_gutter(0.0, spacing::S4);
        let cells = grid.measure(rect);

        // 按钮区
        let btn_rect = GridLayout::cell(&cells, 0, 0);
        let btn_grid = GridLayout::new(
            vec![Track::Fr(1.0), Track::Fr(1.0)],
            vec![
                Track::Fixed(120.0),
                Track::Fixed(120.0),
                Track::Fixed(120.0),
                Track::Fixed(120.0),
            ],
        )
        .with_gutter(spacing::S3, spacing::S2);
        let btn_cells = btn_grid.measure(btn_rect);
        let combos = [
            (ButtonVariant::Primary, true, "Primary"),
            (ButtonVariant::Secondary, true, "Secondary"),
            (ButtonVariant::Danger, true, "Danger"),
            (ButtonVariant::Ghost, true, "Ghost"),
            (ButtonVariant::Primary, false, "Disabled"),
            (ButtonVariant::Secondary, false, "Disabled"),
            (ButtonVariant::Danger, false, "Disabled"),
            (ButtonVariant::Ghost, false, "Disabled"),
        ];
        for (i, (variant, enabled, label)) in combos.iter().enumerate() {
            let cell = GridLayout::cell(&btn_cells, i / 4, i % 4);
            Button::new(label)
                .variant(*variant)
                .size(ButtonSize::Md)
                .enabled(*enabled)
                .show_at(ui, cell);
        }

        // tile 区
        let tile_rect = GridLayout::cell(&cells, 1, 0);
        let tile_grid = GridLayout::new(
            vec![Track::Fr(1.0)],
            vec![
                Track::Fr(1.0),
                Track::Fr(1.0),
                Track::Fr(1.0),
                Track::Fr(1.0),
            ],
        )
        .with_gutter(spacing::S3, 0.0);
        let tcells = tile_grid.measure(tile_rect);
        let tiles = [
            ("人力", "68.0M", palette::STAT_PRIMARY, TileTrend::Up),
            ("GDP", "£58B", palette::BRASS_BRIGHT, TileTrend::Up),
            ("稳定", "73%", palette::GOOD, TileTrend::Flat),
            ("增速", "+1.8%", palette::INFO, TileTrend::Up),
        ];
        for (i, (label, value, color, trend)) in tiles.iter().enumerate() {
            let cell = GridLayout::cell(&tcells, 0, i);
            Tile::new(label, value)
                .accent(*color)
                .trend(*trend)
                .show_at(ui, cell);
        }

        // tab 区
        let tab_rect = GridLayout::cell(&cells, 2, 0);
        let items = [
            TabItem::new("tab_overview", "总览"),
            TabItem::new("tab_decisions", "决议").with_badge(3),
            TabItem::new("tab_advisors", "顾问"),
            TabItem::new("tab_party", "议会"),
        ];
        TabBar::show_at(ui, tab_rect, &items, &mut self.active_tab);
    }

    fn draw_right_column(&mut self, ui: &mut egui::Ui, rect: Rect) {
        // 上下 split：list / card showcase
        let grid = GridLayout::new(
            vec![Track::Fr(1.0), Track::Fixed(80.0)],
            vec![Track::Fr(1.0)],
        )
        .with_gutter(0.0, spacing::S4);
        let cells = grid.measure(rect);

        // List
        let list_outer = GridLayout::cell(&cells, 0, 0);
        Card::new().draw(ui.painter(), list_outer);
        let list_inner = Rect::from_min_max(
            list_outer.min + Vec2::new(spacing::S2, spacing::S2),
            list_outer.max - Vec2::new(spacing::S2, spacing::S2),
        );
        let items = [
            ListItem::new("ger", "DEU 德意志国")
                .with_trailing("民用 32")
                .with_accent(palette::IDEO_FASCISM),
            ListItem::new("spr", "SPR 西班牙")
                .with_trailing("民用 14")
                .with_accent(palette::IDEO_NEUTRALITY),
            ListItem::new("fra", "FRA 法兰西")
                .with_trailing("民用 26")
                .with_accent(palette::IDEO_DEMOCRATIC),
            ListItem::new("gbr", "GBR 大不列颠")
                .with_trailing("民用 35")
                .with_accent(palette::IDEO_DEMOCRATIC),
            ListItem::new("usa", "USA 美利坚")
                .with_trailing("民用 48")
                .with_accent(palette::IDEO_DEMOCRATIC),
            ListItem::new("sov", "SOV 苏联")
                .with_trailing("民用 38")
                .with_accent(palette::IDEO_COMMUNISM),
            ListItem::new("jap", "JAP 日本")
                .with_trailing("民用 22")
                .with_accent(palette::IDEO_FASCISM),
            ListItem::new("ita", "ITA 意大利")
                .with_trailing("民用 18")
                .with_accent(palette::IDEO_FASCISM),
        ];
        ListView::new().show_at(ui, list_inner, &items, &mut self.list_selected);

        // Card showcase 底部条
        let footer = GridLayout::cell(&cells, 1, 0);
        Card::new().as_glass().draw(ui.painter(), footer);
        ui.painter().text(
            footer.center(),
            Align2::CENTER_CENTER,
            "V9 Demo — F12 关闭 · 视觉验收：颜色 / 字号 / 间距 / 边框 / 阴影",
            TextRole::Body.font_id(),
            palette::PARCHMENT,
        );
    }
}

fn demo_size(screen: Rect) -> Vec2 {
    let width = (screen.width() - spacing::S9).clamp(960.0, 1280.0);
    let height = (screen.height() - spacing::S10).clamp(600.0, 780.0);
    Vec2::new(width, height)
}
