//! Shared black-steel/brass panel primitives extracted from the politics panel.
//!
//! `VanillaIron` is the baseline for newly migrated panels.  Legacy gold-brown
//! `components::PANEL_CARD*` and ornate V9 `PanelShell` remain for panels that
//! have not moved yet, but new migrated panel bodies should start here.

use egui::{Color32, Context, Id, Order, Pos2, Rect, RichText, Sense, Stroke, Vec2};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VanillaIron;

impl VanillaIron {
    pub const BLACK: Color32 = Color32::from_rgb(0x08, 0x0a, 0x09);
    pub const CARD: Color32 = Color32::from_rgb(0x0d, 0x0f, 0x0d);
    pub const CARD_SOFT: Color32 = Color32::from_rgb(0x12, 0x14, 0x12);
    pub const CARD_DEEP: Color32 = Color32::from_rgb(0x05, 0x06, 0x05);
    pub const EDGE: Color32 = Color32::from_rgb(0x55, 0x48, 0x31);
    pub const EDGE_DARK: Color32 = Color32::from_rgb(0x28, 0x22, 0x18);
    pub const BRASS: Color32 = Color32::from_rgb(0x9d, 0x84, 0x48);
    pub const BRASS_BRIGHT: Color32 = Color32::from_rgb(0xd0, 0xb0, 0x6a);
    pub const TEXT: Color32 = Color32::from_rgb(0xd6, 0xca, 0x9b);
    pub const MUTED: Color32 = Color32::from_rgb(0x8d, 0x8b, 0x80);
    pub const GOOD: Color32 = Color32::from_rgb(0x70, 0xc8, 0x78);
    pub const WARN: Color32 = Color32::from_rgb(0xff, 0xc0, 0x60);
    pub const BAD: Color32 = Color32::from_rgb(0xe0, 0x60, 0x58);

    pub fn paint_panel(ui: &mut egui::Ui, rect: Rect, accent: Color32) {
        let painter = ui.painter();
        painter.rect_filled(rect, 1.0, Self::BLACK);
        crate::v9::paint::paint_vertical_gradient_mesh(
            painter,
            rect.shrink(2.0),
            Color32::from_rgba_premultiplied(0x18, 0x18, 0x15, 244),
            Color32::from_rgba_premultiplied(0x06, 0x07, 0x06, 252),
        );
        crate::v9::paint::paint_plate_grain(painter, rect.shrink(4.0), 3.0, 3);
        Self::paint_border(painter, rect);
        Self::separator(
            painter,
            rect,
            rect.top() + 3.0,
            Color32::from_white_alpha(18),
        );
        Self::separator(
            painter,
            rect,
            rect.bottom() - 4.0,
            Color32::from_black_alpha(235),
        );
        for corner in [
            rect.left_top() + Vec2::new(14.0, 14.0),
            rect.right_top() + Vec2::new(-14.0, 14.0),
        ] {
            painter.circle_filled(corner, 2.2, Color32::from_black_alpha(210));
            painter.circle_stroke(
                corner,
                2.2,
                Stroke::new(
                    1.0,
                    Color32::from_rgba_premultiplied(accent.r(), accent.g(), accent.b(), 90),
                ),
            );
        }
    }

    pub fn paint_border(painter: &egui::Painter, rect: Rect) {
        painter.rect_stroke(
            rect.translate(Vec2::new(1.0, 1.0)),
            egui::epaint::CornerRadius::same(1),
            Stroke::new(1.0, Color32::from_black_alpha(230)),
            egui::epaint::StrokeKind::Inside,
        );
        painter.rect_stroke(
            rect,
            egui::epaint::CornerRadius::same(1),
            Stroke::new(1.0, Self::EDGE),
            egui::epaint::StrokeKind::Inside,
        );
        painter.rect_stroke(
            rect.shrink(2.0),
            egui::epaint::CornerRadius::same(0),
            Stroke::new(1.0, Color32::from_black_alpha(220)),
            egui::epaint::StrokeKind::Inside,
        );
    }

    pub fn paint_region(painter: &egui::Painter, rect: Rect, fill: Color32) {
        if rect.width() <= 2.0 || rect.height() <= 2.0 {
            return;
        }
        painter.rect_filled(rect, 1.0, fill);
        painter.rect_stroke(
            rect,
            egui::epaint::CornerRadius::same(1),
            Stroke::new(1.0, Self::EDGE_DARK),
            egui::epaint::StrokeKind::Inside,
        );
    }

    pub fn section_title_at(ui: &mut egui::Ui, rect: Rect, title: &str) {
        ui.painter()
            .rect_filled(rect, 0.0, Color32::from_rgb(0x10, 0x11, 0x0f));
        crate::v9::paint::paint_vertical_gradient_mesh(
            ui.painter(),
            rect,
            Color32::from_rgba_premultiplied(0x44, 0x43, 0x38, 95),
            Color32::from_black_alpha(205),
        );
        ui.painter().rect_stroke(
            rect,
            egui::epaint::CornerRadius::same(0),
            Stroke::new(1.0, Color32::from_black_alpha(210)),
            egui::epaint::StrokeKind::Inside,
        );
        Self::separator(
            ui.painter(),
            rect,
            rect.top() + 1.0,
            Color32::from_white_alpha(20),
        );
        Self::separator(ui.painter(), rect, rect.bottom() - 1.0, Self::EDGE);
        ui.painter().text(
            Pos2::new(rect.left() + 6.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            title,
            crate::v9::TextRole::Subheading.font_id(),
            Self::BRASS_BRIGHT,
        );
    }

    pub fn close_button(
        ui: &mut egui::Ui,
        rect: Rect,
        id_source: impl std::hash::Hash,
    ) -> egui::Response {
        let response = ui.interact(rect, Id::new(id_source), Sense::click());
        let fill = if response.hovered() {
            Color32::from_rgb(0x27, 0x28, 0x23)
        } else {
            Color32::from_rgb(0x12, 0x13, 0x10)
        };
        ui.painter().rect_filled(rect, 1.0, fill);
        Self::paint_border(ui.painter(), rect);
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "X",
            crate::v9::TextRole::Caption.font_id(),
            Self::MUTED,
        );
        response
    }

    pub fn compact_button(ui: &mut egui::Ui, text: &str) -> egui::Response {
        ui.add(
            egui::Button::new(RichText::new(text).strong().color(Self::TEXT))
                .fill(Self::CARD_SOFT)
                .stroke(Stroke::new(1.0, Self::EDGE))
                .min_size(Vec2::new(72.0, 24.0)),
        )
    }

    pub fn section_heading(ui: &mut egui::Ui, title: &str) {
        ui.label(
            RichText::new(title)
                .strong()
                .color(Self::BRASS_BRIGHT)
                .size(14.0),
        );
    }

    pub fn info_row(ui: &mut egui::Ui, label: &str, value: impl Into<String>) {
        Self::value_row(ui, label, value, Self::TEXT);
    }

    pub fn value_row(ui: &mut egui::Ui, label: &str, value: impl Into<String>, color: Color32) {
        let value = value.into();
        ui.horizontal(|ui| {
            ui.label(RichText::new(label).small().color(Self::MUTED));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new(value).color(color));
            });
        });
    }

    pub fn warning_row(ui: &mut egui::Ui, text: &str) {
        egui::Frame::new()
            .fill(Color32::from_rgba_premultiplied(0x18, 0x12, 0x0c, 235))
            .stroke(Stroke::new(1.0, Self::WARN))
            .inner_margin(egui::Margin::symmetric(8, 6))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.add(egui::Label::new(RichText::new(text).color(Self::WARN)).wrap());
            });
    }

    fn separator(painter: &egui::Painter, rect: Rect, y: f32, color: Color32) {
        painter.hline(
            (rect.left() + 8.0)..=(rect.right() - 8.0),
            y,
            Stroke::new(1.0, color),
        );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IronPanelPrototype {
    Ledger,
    Workbench,
    Dossier,
    Command,
    Journal,
    Utility,
}

impl IronPanelPrototype {
    pub fn purpose(self) -> &'static str {
        match self {
            Self::Ledger => "dense numeric ledger with aligned rows and limited controls",
            Self::Workbench => "left navigation with a large central work area",
            Self::Dossier => "object detail with header, metrics, sources, and links",
            Self::Command => "force tree, selected unit status, and command strip",
            Self::Journal => "filters, timeline entries, status, and executable actions",
            Self::Utility => "forms, lists, confirmations, and explicit actions",
        }
    }

    fn width(self) -> f32 {
        match self {
            Self::Ledger => 1080.0,
            Self::Workbench => 1120.0,
            Self::Dossier => 430.0,
            Self::Command => 980.0,
            Self::Journal => 920.0,
            Self::Utility => 680.0,
        }
    }

    fn height_ratio(self) -> f32 {
        match self {
            Self::Dossier => 0.74,
            Self::Utility => 0.62,
            Self::Ledger | Self::Workbench | Self::Command | Self::Journal => 0.78,
        }
    }

    fn height_bounds(self) -> (f32, f32) {
        match self {
            Self::Dossier => (320.0, 700.0),
            Self::Utility => (360.0, 680.0),
            Self::Ledger | Self::Workbench | Self::Command | Self::Journal => (520.0, 880.0),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct IronPanelLayout {
    pub outer: Rect,
    pub inner: Rect,
    pub header: Rect,
    pub body: Rect,
    pub footer: Rect,
    pub nav: Rect,
    pub main: Rect,
    pub side: Rect,
    pub top_strip: Rect,
    pub bottom_strip: Rect,
}

#[derive(Debug, Clone)]
struct IronShell<'a> {
    id: Id,
    title: &'a str,
    subtitle: Option<&'a str>,
    footer: Option<&'a str>,
    prototype: IronPanelPrototype,
    accent: Color32,
    position: Option<IronPanelPosition>,
    fixed_size: Option<Vec2>,
}

impl<'a> IronShell<'a> {
    fn new(id_source: impl std::hash::Hash, title: &'a str, prototype: IronPanelPrototype) -> Self {
        Self {
            id: Id::new(id_source),
            title,
            subtitle: None,
            footer: None,
            prototype,
            accent: VanillaIron::BRASS_BRIGHT,
            position: None,
            fixed_size: None,
        }
    }

    fn subtitle(mut self, subtitle: &'a str) -> Self {
        self.subtitle = Some(subtitle);
        self
    }

    fn footer(mut self, footer: &'a str) -> Self {
        self.footer = Some(footer);
        self
    }

    fn accent(mut self, accent: Color32) -> Self {
        self.accent = accent;
        self
    }

    fn left_bottom(mut self, offset: Vec2) -> Self {
        self.position = Some(IronPanelPosition::LeftBottom { offset });
        self
    }

    fn fixed_size(mut self, size: Vec2) -> Self {
        self.fixed_size = Some(size);
        self
    }
}

#[derive(Debug, Clone, Copy)]
enum IronPanelPosition {
    LeftBottom { offset: Vec2 },
}

macro_rules! iron_shell_wrapper {
    ($name:ident, $prototype:expr) => {
        #[derive(Debug, Clone)]
        pub struct $name<'a> {
            core: IronShell<'a>,
        }

        impl<'a> $name<'a> {
            pub fn new(id_source: impl std::hash::Hash, title: &'a str) -> Self {
                Self {
                    core: IronShell::new(id_source, title, $prototype),
                }
            }

            pub fn subtitle(mut self, subtitle: &'a str) -> Self {
                self.core = self.core.subtitle(subtitle);
                self
            }

            pub fn footer(mut self, footer: &'a str) -> Self {
                self.core = self.core.footer(footer);
                self
            }

            pub fn accent(mut self, accent: Color32) -> Self {
                self.core = self.core.accent(accent);
                self
            }

            pub fn left_bottom(mut self, offset: Vec2) -> Self {
                self.core = self.core.left_bottom(offset);
                self
            }

            pub fn fixed_size(mut self, size: Vec2) -> Self {
                self.core = self.core.fixed_size(size);
                self
            }

            pub fn show<R>(
                self,
                ctx: &Context,
                add_contents: impl FnOnce(&mut egui::Ui, IronPanelLayout) -> R,
            ) -> (bool, Option<R>) {
                show_iron_shell(self.core, ctx, add_contents)
            }
        }
    };
}

iron_shell_wrapper!(LedgerPanelShell, IronPanelPrototype::Ledger);
iron_shell_wrapper!(WorkbenchShell, IronPanelPrototype::Workbench);
iron_shell_wrapper!(DossierPanelShell, IronPanelPrototype::Dossier);
iron_shell_wrapper!(CommandPanelShell, IronPanelPrototype::Command);
iron_shell_wrapper!(JournalPanelShell, IronPanelPrototype::Journal);
iron_shell_wrapper!(UtilityWindowShell, IronPanelPrototype::Utility);

fn show_iron_shell<R>(
    core: IronShell<'_>,
    ctx: &Context,
    add_contents: impl FnOnce(&mut egui::Ui, IronPanelLayout) -> R,
) -> (bool, Option<R>) {
    let screen = ctx.screen_rect();
    let size = core
        .fixed_size
        .map(|size| clamp_panel_size(size, screen))
        .unwrap_or_else(|| panel_size(core.prototype, screen));
    let pos = panel_pos_with_override(core.prototype, screen, size, core.position);

    let mut close = false;
    let mut output = None;
    egui::Area::new(core.id)
        .order(Order::Foreground)
        .fixed_pos(pos)
        .default_size(size)
        .show(ctx, |ui| {
            let (outer, _) = ui.allocate_exact_size(size, Sense::click_and_drag());
            VanillaIron::paint_panel(ui, outer, core.accent);
            let inner = outer.shrink2(Vec2::new(12.0, 10.0));
            let layout = build_layout(core.prototype, inner, core.subtitle.is_some(), core.footer);
            draw_layout_regions(ui, core.prototype, layout);
            close = draw_shell_header(ui, &core, layout.header);
            draw_shell_footer(ui, core.footer, layout.footer);

            let mut content_ui = ui.new_child(
                egui::UiBuilder::new()
                    .id_salt(core.id.with("content"))
                    .max_rect(inner)
                    .layout(egui::Layout::top_down(egui::Align::Min))
                    .sense(Sense::hover()),
            );
            content_ui.set_clip_rect(inner);
            output = Some(add_contents(&mut content_ui, layout));
        });

    (close, output)
}

fn panel_pos_with_override(
    prototype: IronPanelPrototype,
    screen: Rect,
    size: Vec2,
    position: Option<IronPanelPosition>,
) -> Pos2 {
    match position {
        Some(IronPanelPosition::LeftBottom { offset }) => Pos2::new(
            screen.left() + offset.x,
            (screen.bottom() - size.y - offset.y).max(screen.top() + 12.0),
        ),
        None => panel_pos(prototype, screen, size),
    }
}

fn panel_size(prototype: IronPanelPrototype, screen: Rect) -> Vec2 {
    let edge = 12.0;
    let usable_w = (screen.width() - edge * 2.0).max(1.0);
    let usable_h = (screen.height() - edge * 2.0).max(1.0);
    let (min_h, max_h) = prototype.height_bounds();
    let height = (screen.height() * prototype.height_ratio())
        .clamp(min_h, max_h)
        .min(usable_h);
    Vec2::new(prototype.width().min(usable_w), height)
}

fn clamp_panel_size(size: Vec2, screen: Rect) -> Vec2 {
    let edge = 12.0;
    Vec2::new(
        size.x.min((screen.width() - edge * 2.0).max(1.0)),
        size.y.min((screen.height() - edge * 2.0).max(1.0)),
    )
}

fn panel_pos(prototype: IronPanelPrototype, screen: Rect, size: Vec2) -> Pos2 {
    let top = if screen.height() >= 760.0 { 96.0 } else { 72.0 };
    match prototype {
        IronPanelPrototype::Dossier => Pos2::new(
            (screen.right() - size.x - 22.0).max(screen.left() + 18.0),
            (screen.top() + top + 8.0).min(screen.bottom() - size.y - 12.0),
        ),
        IronPanelPrototype::Utility => Pos2::new(
            screen.center().x - size.x * 0.5,
            screen.center().y - size.y * 0.5,
        ),
        _ => Pos2::new(
            screen.left() + 112.0_f32.min(screen.width() * 0.12),
            screen.top() + top,
        ),
    }
}

fn build_layout(
    prototype: IronPanelPrototype,
    inner: Rect,
    has_subtitle: bool,
    footer: Option<&str>,
) -> IronPanelLayout {
    let header_h = if has_subtitle { 52.0 } else { 42.0 };
    let footer_h = if footer.is_some() { 28.0 } else { 0.0 };
    let header = Rect::from_min_max(
        inner.left_top(),
        Pos2::new(inner.right(), (inner.top() + header_h).min(inner.bottom())),
    );
    let footer_rect = Rect::from_min_max(
        Pos2::new(
            inner.left(),
            (inner.bottom() - footer_h).max(inner.top() + header_h),
        ),
        inner.right_bottom(),
    );
    let body = Rect::from_min_max(
        Pos2::new(inner.left(), header.bottom() + 8.0),
        Pos2::new(inner.right(), footer_rect.top() - 8.0),
    );

    let mut layout = IronPanelLayout {
        outer: inner.expand2(Vec2::new(12.0, 10.0)),
        inner,
        header,
        body,
        footer: footer_rect,
        nav: zero_rect(body.left_top()),
        main: body,
        side: zero_rect(body.right_top()),
        top_strip: zero_rect(body.left_top()),
        bottom_strip: zero_rect(body.left_bottom()),
    };

    match prototype {
        IronPanelPrototype::Ledger => {
            let top_h = 66.0_f32.min(body.height() * 0.22);
            let side_w = 230.0_f32.min(body.width() * 0.28);
            layout.top_strip =
                Rect::from_min_max(body.left_top(), Pos2::new(body.right(), body.top() + top_h));
            let ledger_top = layout.top_strip.bottom() + 8.0;
            layout.side = Rect::from_min_max(
                Pos2::new(body.right() - side_w, ledger_top),
                body.right_bottom(),
            );
            layout.main = Rect::from_min_max(
                Pos2::new(body.left(), ledger_top),
                Pos2::new(layout.side.left() - 8.0, body.bottom()),
            );
        }
        IronPanelPrototype::Workbench => {
            let nav_w = 190.0_f32.min(body.width() * 0.24);
            let side_w = 250.0_f32.min(body.width() * 0.28);
            layout.nav = Rect::from_min_max(
                body.left_top(),
                Pos2::new(body.left() + nav_w, body.bottom()),
            );
            layout.side = Rect::from_min_max(
                Pos2::new(body.right() - side_w, body.top()),
                body.right_bottom(),
            );
            layout.main = Rect::from_min_max(
                Pos2::new(layout.nav.right() + 8.0, body.top()),
                Pos2::new(layout.side.left() - 8.0, body.bottom()),
            );
        }
        IronPanelPrototype::Dossier => {
            let top_h = 72.0_f32.min(body.height() * 0.25);
            layout.top_strip =
                Rect::from_min_max(body.left_top(), Pos2::new(body.right(), body.top() + top_h));
            layout.bottom_strip = zero_rect(body.left_bottom());
            layout.main = Rect::from_min_max(
                Pos2::new(body.left(), layout.top_strip.bottom() + 8.0),
                body.right_bottom(),
            );
        }
        IronPanelPrototype::Command => {
            let nav_w = 260.0_f32.min(body.width() * 0.32);
            let bottom_h = 58.0_f32.min(body.height() * 0.16);
            layout.nav = Rect::from_min_max(
                body.left_top(),
                Pos2::new(body.left() + nav_w, body.bottom()),
            );
            layout.bottom_strip = Rect::from_min_max(
                Pos2::new(layout.nav.right() + 8.0, body.bottom() - bottom_h),
                body.right_bottom(),
            );
            layout.main = Rect::from_min_max(
                Pos2::new(layout.nav.right() + 8.0, body.top()),
                Pos2::new(body.right(), layout.bottom_strip.top() - 8.0),
            );
        }
        IronPanelPrototype::Journal => {
            let nav_w = 180.0_f32.min(body.width() * 0.24);
            let side_w = 260.0_f32.min(body.width() * 0.30);
            layout.nav = Rect::from_min_max(
                body.left_top(),
                Pos2::new(body.left() + nav_w, body.bottom()),
            );
            layout.side = Rect::from_min_max(
                Pos2::new(body.right() - side_w, body.top()),
                body.right_bottom(),
            );
            layout.main = Rect::from_min_max(
                Pos2::new(layout.nav.right() + 8.0, body.top()),
                Pos2::new(layout.side.left() - 8.0, body.bottom()),
            );
        }
        IronPanelPrototype::Utility => {
            let bottom_h = 48.0_f32.min(body.height() * 0.18);
            layout.main = Rect::from_min_max(
                body.left_top(),
                Pos2::new(body.right(), body.bottom() - bottom_h - 8.0),
            );
            layout.bottom_strip = Rect::from_min_max(
                Pos2::new(body.left(), layout.main.bottom() + 8.0),
                body.right_bottom(),
            );
        }
    }

    layout
}

fn draw_layout_regions(ui: &mut egui::Ui, prototype: IronPanelPrototype, layout: IronPanelLayout) {
    match prototype {
        IronPanelPrototype::Ledger => {
            VanillaIron::paint_region(ui.painter(), layout.top_strip, VanillaIron::CARD_SOFT);
            VanillaIron::paint_region(ui.painter(), layout.main, VanillaIron::CARD_DEEP);
            VanillaIron::paint_region(ui.painter(), layout.side, VanillaIron::CARD);
        }
        IronPanelPrototype::Workbench => {
            VanillaIron::paint_region(ui.painter(), layout.nav, VanillaIron::CARD_DEEP);
            VanillaIron::paint_region(ui.painter(), layout.main, VanillaIron::CARD);
            VanillaIron::paint_region(ui.painter(), layout.side, VanillaIron::CARD_SOFT);
        }
        IronPanelPrototype::Dossier => {
            VanillaIron::paint_region(ui.painter(), layout.top_strip, VanillaIron::CARD_SOFT);
            VanillaIron::paint_region(ui.painter(), layout.main, VanillaIron::CARD_DEEP);
        }
        IronPanelPrototype::Command => {
            VanillaIron::paint_region(ui.painter(), layout.nav, VanillaIron::CARD_DEEP);
            VanillaIron::paint_region(ui.painter(), layout.main, VanillaIron::CARD);
            VanillaIron::paint_region(ui.painter(), layout.bottom_strip, VanillaIron::CARD_SOFT);
        }
        IronPanelPrototype::Journal => {
            VanillaIron::paint_region(ui.painter(), layout.nav, VanillaIron::CARD_DEEP);
            VanillaIron::paint_region(ui.painter(), layout.main, VanillaIron::CARD);
            VanillaIron::paint_region(ui.painter(), layout.side, VanillaIron::CARD_SOFT);
        }
        IronPanelPrototype::Utility => {
            VanillaIron::paint_region(ui.painter(), layout.main, VanillaIron::CARD_DEEP);
            VanillaIron::paint_region(ui.painter(), layout.bottom_strip, VanillaIron::CARD);
        }
    }
}

fn draw_shell_header(ui: &mut egui::Ui, core: &IronShell<'_>, rect: Rect) -> bool {
    ui.painter().text(
        Pos2::new(rect.left() + 3.0, rect.top() + 5.0),
        egui::Align2::LEFT_TOP,
        core.title,
        crate::v9::TextRole::Display.font_id(),
        VanillaIron::TEXT,
    );
    if let Some(subtitle) = core.subtitle {
        ui.painter().text(
            Pos2::new(rect.left() + 4.0, rect.top() + 33.0),
            egui::Align2::LEFT_TOP,
            subtitle,
            crate::v9::TextRole::Caption.font_id(),
            VanillaIron::MUTED,
        );
    }
    ui.painter().hline(
        rect.left()..=rect.right(),
        rect.bottom() - 1.0,
        Stroke::new(1.0, VanillaIron::EDGE),
    );
    let close_rect = Rect::from_min_size(
        Pos2::new(rect.right() - 28.0, rect.top() + 1.0),
        Vec2::splat(23.0),
    );
    VanillaIron::close_button(ui, close_rect, core.id.with("close"))
        .on_hover_text("关闭")
        .clicked()
}

fn draw_shell_footer(ui: &mut egui::Ui, footer: Option<&str>, rect: Rect) {
    let Some(footer) = footer else {
        return;
    };
    ui.painter().hline(
        rect.left()..=rect.right(),
        rect.top(),
        Stroke::new(1.0, VanillaIron::EDGE_DARK),
    );
    ui.painter().text(
        Pos2::new(rect.left() + 4.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        footer,
        crate::v9::TextRole::Caption.font_id(),
        VanillaIron::MUTED,
    );
}

fn zero_rect(pos: Pos2) -> Rect {
    Rect::from_min_size(pos, Vec2::ZERO)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gate2_shells_have_distinct_purposes() {
        let purposes = [
            IronPanelPrototype::Ledger.purpose(),
            IronPanelPrototype::Workbench.purpose(),
            IronPanelPrototype::Dossier.purpose(),
            IronPanelPrototype::Command.purpose(),
            IronPanelPrototype::Journal.purpose(),
            IronPanelPrototype::Utility.purpose(),
        ];
        assert_eq!(purposes.len(), 6);
        assert!(purposes.iter().all(|purpose| !purpose.is_empty()));
        assert_ne!(
            IronPanelPrototype::Ledger.purpose(),
            IronPanelPrototype::Dossier.purpose()
        );
    }

    #[test]
    fn gate2_ledger_and_dossier_layouts_are_not_dashboard_clones() {
        let inner = Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 640.0));
        let ledger = build_layout(IronPanelPrototype::Ledger, inner, true, Some("Esc 返回"));
        let dossier = build_layout(IronPanelPrototype::Dossier, inner, true, Some("Esc 返回"));

        assert!(ledger.side.width() > 0.0);
        assert!(ledger.main.width() > ledger.side.width());
        assert!(dossier.top_strip.height() > 0.0);
        assert_eq!(dossier.bottom_strip.height(), 0.0);
        assert_ne!(ledger.main, dossier.main);
    }
}
