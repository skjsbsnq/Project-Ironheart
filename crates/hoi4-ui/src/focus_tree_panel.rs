//! Focus tree panel — node-line rendering with scroll/zoom/tooltip/selection.

#![allow(deprecated)]

use crate::i18n::tr;
use egui::{Color32, CornerRadius, Pos2, Rect, Stroke, Vec2};
use hoi4_content::focus::{Focus, FocusTree};
use std::collections::HashSet;

/// Node visual state.
#[derive(Clone, Copy, PartialEq)]
pub enum FocusState {
    Completed,
    InProgress(f32),
    Available,
    Locked,
}

/// Command emitted by the panel for the game to execute.
#[derive(Debug, Clone, PartialEq)]
pub enum FocusCommand {
    /// Player confirmed starting this focus.
    Start(String),
    /// Player cancelled the current focus.
    Cancel,
}

const NODE_W: f32 = 90.0;
const NODE_H: f32 = 90.0;
const GRID_SPACING_X: f32 = 110.0;
const GRID_SPACING_Y: f32 = 110.0;
const ICON_SIZE: f32 = 52.0;
const RING_RADIUS: f32 = 30.0;
const PROGRESS_H: f32 = 6.0;
const MIN_ZOOM: f32 = 0.5;
const MAX_ZOOM: f32 = 2.0;

/// Focus tree panel state.
pub struct FocusTreePanel {
    pub open: bool,
    pub zoom: f32,
    /// Currently selected (highlighted) focus id, awaiting confirm.
    pub selected: Option<String>,
}

impl FocusTreePanel {
    pub fn new() -> Self {
        Self {
            open: false,
            zoom: 1.0,
            selected: None,
        }
    }

    /// Show the focus tree window. Returns a command if the player acts.
    /// P0.3：available_focus_ids 包含通过 available 条件的 focus id，
    /// 不在此集合中的 focus 显示为锁定。
    #[allow(unreachable_code)]
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        tree: &FocusTree,
        completed: &HashSet<String>,
        current_focus: Option<&str>,
        current_progress: f32,
        available_focus_ids: &HashSet<String>,
    ) -> Option<FocusCommand> {
        if !self.open {
            return None;
        }
        return v9_show_focus_tree(
            self,
            ctx,
            tree,
            completed,
            current_focus,
            current_progress,
            available_focus_ids,
        );

        let mut cmd: Option<FocusCommand> = None;
        let mut open = self.open;

        egui::Window::new(tr("national_focus"))
            .open(&mut open)
            .default_size([800.0, 600.0])
            .show(ctx, |ui| {
                // Zoom controls
                ui.horizontal(|ui| {
                    if ui.button("−").clicked() {
                        self.zoom = (self.zoom - 0.1).max(MIN_ZOOM);
                    }
                    ui.label(format!("{:.0}%", self.zoom * 100.0));
                    if ui.button("+").clicked() {
                        self.zoom = (self.zoom + 0.1).min(MAX_ZOOM);
                    }
                    // Cancel button
                    if current_focus.is_some() {
                        if ui.button(tr("cancel_focus")).clicked() {
                            cmd = Some(FocusCommand::Cancel);
                        }
                    }
                    // Confirm button
                    if let Some(sel) = &self.selected {
                        let selected_focus = tree.focuses.iter().find(|f| f.id == *sel);
                        let state = selected_focus
                            .map(|f| {
                                focus_state(
                                    f,
                                    completed,
                                    current_focus,
                                    current_progress,
                                    available_focus_ids.contains(&f.id),
                                )
                            })
                            .unwrap_or(FocusState::Locked);
                        if state == FocusState::Available && current_focus.is_none() {
                            let label = selected_focus
                                .map(focus_display_name)
                                .unwrap_or_else(|| sel.clone());
                            if ui
                                .button(format!("{}: {label}", tr("start_focus")))
                                .clicked()
                            {
                                cmd = Some(FocusCommand::Start(sel.clone()));
                                self.selected = None;
                            }
                        }
                    }
                });

                // Scroll area with zoom
                egui::ScrollArea::both().show(ui, |ui| {
                    let z = self.zoom;
                    let (min_x, max_x, min_y, max_y) = tree_bounds(tree);
                    let canvas_w = ((max_x - min_x + 1) as f32 * GRID_SPACING_X + NODE_W) * z;
                    let canvas_h = ((max_y - min_y + 1) as f32 * GRID_SPACING_Y + NODE_H) * z;
                    let (response, painter) =
                        ui.allocate_painter(Vec2::new(canvas_w, canvas_h), egui::Sense::click());
                    let origin = response.rect.min;

                    // Handle scroll-wheel zoom
                    if response.hovered() {
                        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
                        if scroll != 0.0 {
                            self.zoom = (self.zoom + scroll * 0.001).clamp(MIN_ZOOM, MAX_ZOOM);
                        }
                    }

                    draw_lines(&painter, tree, origin, min_x, min_y, z);
                    draw_mutual_exclusive(&painter, tree, origin, min_x, min_y, z);

                    // Draw nodes + detect click + tooltip
                    for focus in &tree.focuses {
                        let state = focus_state(
                            focus,
                            completed,
                            current_focus,
                            current_progress,
                            available_focus_ids.contains(&focus.id),
                        );
                        let center = node_center_z(focus, origin, min_x, min_y, z);
                        let selected = self.selected.as_deref() == Some(focus.id.as_str());
                        draw_node(&painter, focus, center, state, z, selected);

                        // Hit test for click
                        let node_rect =
                            Rect::from_center_size(center, Vec2::splat(RING_RADIUS * 2.0 * z));
                        if response.clicked() {
                            if let Some(pos) = response.interact_pointer_pos() {
                                if node_rect.contains(pos) {
                                    self.selected = Some(focus.id.clone());
                                }
                            }
                        }

                        // Tooltip on hover
                        if ui.rect_contains_pointer(node_rect) {
                            egui::show_tooltip(
                                ui.ctx(),
                                ui.layer_id(),
                                ui.id().with(&focus.id),
                                |ui| {
                                    ui.strong(focus_display_name(focus));
                                    ui.label(
                                        tr("cost_days").replace("{}", &focus.cost_days.to_string()),
                                    );
                                    match state {
                                        FocusState::Completed => {
                                            ui.label(tr("focus_completed"));
                                        }
                                        FocusState::InProgress(p) => {
                                            ui.label(
                                                tr("focus_in_progress")
                                                    .replace("{:.0}", &format!("{:.0}", p * 100.0)),
                                            );
                                        }
                                        FocusState::Available => {
                                            ui.label(tr("available"));
                                        }
                                        FocusState::Locked => {
                                            ui.label(tr("locked"));
                                        }
                                    }
                                    let prerequisites = localized_prerequisites(focus);
                                    if !prerequisites.is_empty() {
                                        ui.label(tr("requires").replace("{}", &prerequisites));
                                    }
                                },
                            );
                        }
                    }
                });
            });
        self.open = open;
        cmd
    }
}

fn v9_show_focus_tree(
    panel: &mut FocusTreePanel,
    ctx: &egui::Context,
    tree: &FocusTree,
    completed: &HashSet<String>,
    current_focus: Option<&str>,
    current_progress: f32,
    available_focus_ids: &HashSet<String>,
) -> Option<FocusCommand> {
    use crate::v9::composites::panel_shell::{
        draw_summary_tiles, draw_tab_strip, PanelClass, PanelShell,
    };
    use crate::v9::tokens::palette;
    let available_count = tree
        .focuses
        .iter()
        .filter(|f| {
            focus_state(
                f,
                completed,
                current_focus,
                current_progress,
                available_focus_ids.contains(&f.id),
            ) == FocusState::Available
        })
        .count();
    let mut cmd = None;
    let (close, _) = PanelShell::new("focus_tree_panel_v9", tr("national_focus"))
        .class(PanelClass::Detail)
        .accent(palette::GOLD)
        .footer("Q Close  |  Mouse wheel zoom")
        .show(ctx, |ui, layout| {
            draw_summary_tiles(
                ui,
                layout.summary,
                &[
                    ("Total", tree.focuses.len().to_string(), palette::PARCHMENT),
                    ("Completed", completed.len().to_string(), palette::GOOD),
                    ("Available", available_count.to_string(), palette::GOLD),
                    (
                        "Zoom",
                        format!("{:.0}%", panel.zoom * 100.0),
                        palette::BRASS_BRIGHT,
                    ),
                ],
            );
            draw_tab_strip(
                ui,
                layout.tabs,
                "TreeLayout / 32x32 nodes / Status outline",
                palette::GOLD,
            );
            v9_focus_tree_body(
                ui,
                layout.body,
                panel,
                tree,
                completed,
                current_focus,
                current_progress,
                available_focus_ids,
                &mut cmd,
            );
        });
    if close {
        panel.open = false;
    }
    cmd
}

fn v9_focus_tree_body(
    ui: &mut egui::Ui,
    rect: Rect,
    panel: &mut FocusTreePanel,
    tree: &FocusTree,
    completed: &HashSet<String>,
    current_focus: Option<&str>,
    current_progress: f32,
    available_focus_ids: &HashSet<String>,
    cmd: &mut Option<FocusCommand>,
) {
    use crate::v9::{
        layout::{GridLayout, Track},
        primitives::{Button, ButtonSize, ButtonVariant, TreeLayout},
        tokens::{palette, spacing, TextRole},
    };
    ui.allocate_ui_at_rect(rect, |ui| {
        let grid = GridLayout::new(
            vec![Track::Fixed(42.0), Track::Fr(1.0)],
            vec![Track::Fr(1.0)],
        )
        .with_gutter(0.0, spacing::S4);
        let cells = grid.measure(rect);
        let controls = GridLayout::cell(&cells, 0, 0);
        crate::v9::paint::paint_recessed_panel(ui.painter(), controls, 1.0);
        let minus = Rect::from_min_size(
            Pos2::new(controls.left() + 8.0, controls.top() + 7.0),
            Vec2::new(32.0, 28.0),
        );
        if Button::new("-")
            .size(ButtonSize::Sm)
            .variant(ButtonVariant::Secondary)
            .show_at(ui, minus)
            .clicked()
        {
            panel.zoom = (panel.zoom - 0.1).max(MIN_ZOOM);
        }
        ui.painter().text(
            Pos2::new(controls.left() + 58.0, controls.center().y),
            egui::Align2::LEFT_CENTER,
            format!("{:.0}%", panel.zoom * 100.0),
            TextRole::Numeric.font_id(),
            palette::PARCHMENT,
        );
        let plus = Rect::from_min_size(
            Pos2::new(controls.left() + 116.0, controls.top() + 7.0),
            Vec2::new(32.0, 28.0),
        );
        if Button::new("+")
            .size(ButtonSize::Sm)
            .variant(ButtonVariant::Secondary)
            .show_at(ui, plus)
            .clicked()
        {
            panel.zoom = (panel.zoom + 0.1).min(MAX_ZOOM);
        }
        let mut right_x = controls.right() - 132.0;
        if current_focus.is_some() {
            if Button::new(tr("cancel_focus"))
                .size(ButtonSize::Md)
                .variant(ButtonVariant::Danger)
                .show_at(
                    ui,
                    Rect::from_min_size(
                        Pos2::new(right_x, controls.top() + 5.0),
                        Vec2::new(126.0, 32.0),
                    ),
                )
                .clicked()
            {
                *cmd = Some(FocusCommand::Cancel);
            }
            right_x -= 136.0;
        }
        if let Some(sel) = panel.selected.as_deref() {
            if let Some(focus) = tree.focuses.iter().find(|f| f.id == sel) {
                let state = focus_state(
                    focus,
                    completed,
                    current_focus,
                    current_progress,
                    available_focus_ids.contains(&focus.id),
                );
                if state == FocusState::Available && current_focus.is_none() {
                    if Button::new(tr("start_focus"))
                        .size(ButtonSize::Md)
                        .variant(ButtonVariant::Primary)
                        .show_at(
                            ui,
                            Rect::from_min_size(
                                Pos2::new(right_x, controls.top() + 5.0),
                                Vec2::new(126.0, 32.0),
                            ),
                        )
                        .clicked()
                    {
                        *cmd = Some(FocusCommand::Start(focus.id.clone()));
                        panel.selected = None;
                    }
                }
            }
        }

        let canvas_rect = GridLayout::cell(&cells, 1, 0);
        ui.allocate_ui_at_rect(canvas_rect, |ui| {
            egui::ScrollArea::both().show(ui, |ui| {
                let z = panel.zoom;
                let (min_x, max_x, min_y, max_y) = tree_bounds(tree);
                let layout = TreeLayout::phase_d();
                let canvas = layout.canvas_size(min_x, max_x, min_y, max_y, z);
                let (response, painter) = ui.allocate_painter(canvas, egui::Sense::click());
                let origin = response.rect.min + Vec2::new(24.0, 24.0) * z;
                if response.hovered() {
                    let scroll = ui.input(|i| i.smooth_scroll_delta.y);
                    if scroll != 0.0 {
                        panel.zoom = (panel.zoom + scroll * 0.001).clamp(MIN_ZOOM, MAX_ZOOM);
                    }
                }
                v9_draw_focus_lines(&painter, &layout, tree, origin, min_x, min_y, z, completed);
                for focus in &tree.focuses {
                    let state = focus_state(
                        focus,
                        completed,
                        current_focus,
                        current_progress,
                        available_focus_ids.contains(&focus.id),
                    );
                    let node_rect = layout.node_rect(origin, min_x, min_y, focus.position, z);
                    let selected = panel.selected.as_deref() == Some(focus.id.as_str());
                    v9_draw_focus_node(&painter, focus, node_rect, state, selected, z);
                    if response.clicked() {
                        if let Some(pos) = response.interact_pointer_pos() {
                            if node_rect.expand(8.0 * z).contains(pos) {
                                panel.selected = Some(focus.id.clone());
                            }
                        }
                    }
                    if ui.rect_contains_pointer(node_rect.expand(8.0 * z)) {
                        egui::show_tooltip(
                            ui.ctx(),
                            ui.layer_id(),
                            ui.id().with(("v9_focus_tip", &focus.id)),
                            |ui| {
                                ui.strong(focus_display_name(focus));
                                ui.label(
                                    tr("cost_days").replace("{}", &focus.cost_days.to_string()),
                                );
                                match state {
                                    FocusState::Completed => {
                                        ui.label(tr("focus_completed"));
                                    }
                                    FocusState::InProgress(p) => {
                                        ui.label(format!("{:.0}%", p * 100.0));
                                    }
                                    FocusState::Available => {
                                        ui.label(tr("available"));
                                    }
                                    FocusState::Locked => {
                                        ui.label(tr("locked"));
                                    }
                                }
                                let prerequisites = localized_prerequisites(focus);
                                if !prerequisites.is_empty() {
                                    ui.label(tr("requires").replace("{}", &prerequisites));
                                }
                            },
                        );
                    }
                }
            });
        });
    });
}

fn v9_draw_focus_lines(
    painter: &egui::Painter,
    layout: &crate::v9::primitives::TreeLayout,
    tree: &FocusTree,
    origin: Pos2,
    min_x: i32,
    min_y: i32,
    z: f32,
    completed: &HashSet<String>,
) {
    let id_to_focus: std::collections::HashMap<&str, &Focus> =
        tree.focuses.iter().map(|f| (f.id.as_str(), f)).collect();
    for focus in &tree.focuses {
        let child = layout
            .node_rect(origin, min_x, min_y, focus.position, z)
            .center();
        for group in &focus.prerequisites {
            for pid in group {
                if let Some(parent_focus) = id_to_focus.get(pid.as_str()) {
                    let parent = layout
                        .node_rect(origin, min_x, min_y, parent_focus.position, z)
                        .center();
                    let col = if completed.contains(pid) {
                        crate::v9::palette::GOLD
                    } else {
                        crate::v9::palette::HAIRLINE
                    };
                    painter.line_segment([parent, child], Stroke::new(1.4 * z.max(0.75), col));
                }
            }
        }
    }
}

fn v9_draw_focus_node(
    painter: &egui::Painter,
    focus: &Focus,
    rect: Rect,
    state: FocusState,
    selected: bool,
    z: f32,
) {
    let color = match state {
        FocusState::Completed => crate::v9::palette::GOOD,
        FocusState::InProgress(_) => crate::v9::palette::GOLD,
        FocusState::Available => crate::v9::palette::PARCHMENT,
        FocusState::Locked => crate::v9::palette::MUTED,
    };
    crate::v9::paint::paint_bevel(painter, rect, crate::v9::palette::SOOT_BLACK, color, 1.0);
    painter.rect_stroke(
        rect.expand(if selected { 4.0 } else { 1.0 } * z),
        CornerRadius::same(1),
        Stroke::new(if selected { 2.0 } else { 1.0 } * z.max(0.75), color),
        egui::StrokeKind::Inside,
    );
    if let FocusState::InProgress(pct) = state {
        let bar = Rect::from_min_size(
            Pos2::new(rect.left(), rect.bottom() + 4.0 * z),
            Vec2::new(rect.width() * pct.clamp(0.0, 1.0), 4.0 * z),
        );
        painter.rect_filled(bar, CornerRadius::same(1), crate::v9::palette::GOLD);
    }
    let label = focus_display_name(focus);
    painter.text(
        Pos2::new(rect.center().x, rect.bottom() + 10.0 * z),
        egui::Align2::CENTER_TOP,
        label,
        crate::v9::TextRole::Small.font_id(),
        color,
    );
}

fn tree_bounds(tree: &FocusTree) -> (i32, i32, i32, i32) {
    let mut min_x = i32::MAX;
    let mut max_x = i32::MIN;
    let mut min_y = i32::MAX;
    let mut max_y = i32::MIN;
    for f in &tree.focuses {
        min_x = min_x.min(f.position.0);
        max_x = max_x.max(f.position.0);
        min_y = min_y.min(f.position.1);
        max_y = max_y.max(f.position.1);
    }
    if min_x > max_x {
        (0, 0, 0, 0)
    } else {
        (min_x, max_x, min_y, max_y)
    }
}

fn node_center_z(focus: &Focus, origin: Pos2, min_x: i32, min_y: i32, z: f32) -> Pos2 {
    let gx = (focus.position.0 - min_x) as f32;
    let gy = (focus.position.1 - min_y) as f32;
    Pos2::new(
        origin.x + (gx * GRID_SPACING_X + NODE_W * 0.5) * z,
        origin.y + (gy * GRID_SPACING_Y + NODE_H * 0.5) * z,
    )
}

fn focus_state(
    focus: &Focus,
    completed: &HashSet<String>,
    current: Option<&str>,
    progress: f32,
    available_ok: bool,
) -> FocusState {
    if completed.contains(&focus.id) {
        return FocusState::Completed;
    }
    if current == Some(focus.id.as_str()) {
        return FocusState::InProgress(progress / focus.cost_days as f32);
    }
    for group in &focus.prerequisites {
        if !group.iter().any(|p| completed.contains(p)) {
            return FocusState::Locked;
        }
    }
    for me in &focus.mutually_exclusive {
        if completed.contains(me) {
            return FocusState::Locked;
        }
    }
    // P0.3：available 条件不满足时显示锁定
    if !available_ok {
        return FocusState::Locked;
    }
    FocusState::Available
}

fn draw_node(
    painter: &egui::Painter,
    focus: &Focus,
    center: Pos2,
    state: FocusState,
    z: f32,
    selected: bool,
) {
    let r = RING_RADIUS * z;
    let color = match state {
        FocusState::Completed => Color32::from_rgb(80, 200, 80),
        FocusState::InProgress(_) => Color32::from_rgb(220, 180, 50),
        FocusState::Available => Color32::from_rgb(200, 200, 200),
        FocusState::Locked => Color32::from_rgb(100, 100, 100),
    };
    let ring_w = if selected { 5.0 } else { 3.0 };
    painter.circle_stroke(center, r, Stroke::new(ring_w, color));
    if selected {
        painter.circle_stroke(
            center,
            r + 3.0,
            Stroke::new(1.5, Color32::from_rgb(255, 220, 100)),
        );
    }
    painter.circle_filled(center, ICON_SIZE * 0.4 * z, Color32::from_rgb(60, 50, 40));

    if let FocusState::InProgress(pct) = state {
        let bar_top = center.y + r + 4.0;
        let bar_left = center.x - r;
        let bar_w = r * 2.0;
        let bg = Rect::from_min_size(
            Pos2::new(bar_left, bar_top),
            Vec2::new(bar_w, PROGRESS_H * z),
        );
        painter.rect_filled(bg, CornerRadius::same(2), Color32::from_rgb(40, 40, 40));
        let fg = Rect::from_min_size(
            Pos2::new(bar_left, bar_top),
            Vec2::new(bar_w * pct.clamp(0.0, 1.0), PROGRESS_H * z),
        );
        painter.rect_filled(fg, CornerRadius::same(2), Color32::from_rgb(220, 180, 50));
    }

    let label_pos = Pos2::new(center.x, center.y + r + 14.0);
    let focus_name = focus_display_name(focus);
    let galley = painter.layout_no_wrap(
        focus_name,
        egui::FontId::proportional(10.0 * z),
        Color32::from_rgb(200, 190, 170),
    );
    let text_rect = egui::Align2::CENTER_TOP.anchor_size(label_pos, galley.size());
    painter.galley(text_rect.min, galley, Color32::from_rgb(200, 190, 170));
}

fn focus_display_name(focus: &Focus) -> String {
    let t = tr(&focus.id);
    if t == focus.id {
        focus.name.clone()
    } else {
        t.to_string()
    }
}

fn localized_prerequisites(focus: &Focus) -> String {
    focus
        .prerequisites
        .iter()
        .map(|group| {
            group
                .iter()
                .map(|id| {
                    let t = tr(id);
                    if t == id {
                        id.as_str()
                    } else {
                        t
                    }
                })
                .collect::<Vec<_>>()
                .join(" / ")
        })
        .collect::<Vec<_>>()
        .join("; ")
}

fn draw_lines(
    painter: &egui::Painter,
    tree: &FocusTree,
    origin: Pos2,
    min_x: i32,
    min_y: i32,
    z: f32,
) {
    let id_to_focus: std::collections::HashMap<&str, &Focus> =
        tree.focuses.iter().map(|f| (f.id.as_str(), f)).collect();
    let r = RING_RADIUS * z;
    for focus in &tree.focuses {
        let child = node_center_z(focus, origin, min_x, min_y, z);
        for group in &focus.prerequisites {
            for pid in group {
                if let Some(p) = id_to_focus.get(pid.as_str()) {
                    let parent = node_center_z(p, origin, min_x, min_y, z);
                    painter.line_segment(
                        [
                            Pos2::new(parent.x, parent.y + r),
                            Pos2::new(child.x, child.y - r),
                        ],
                        Stroke::new(1.5, Color32::from_rgb(150, 140, 120)),
                    );
                }
            }
        }
    }
}

fn draw_mutual_exclusive(
    painter: &egui::Painter,
    tree: &FocusTree,
    origin: Pos2,
    min_x: i32,
    min_y: i32,
    z: f32,
) {
    let id_to_focus: std::collections::HashMap<&str, &Focus> =
        tree.focuses.iter().map(|f| (f.id.as_str(), f)).collect();
    let mut drawn: HashSet<(&str, &str)> = HashSet::new();
    for focus in &tree.focuses {
        for me_id in &focus.mutually_exclusive {
            let pair = if focus.id.as_str() < me_id.as_str() {
                (focus.id.as_str(), me_id.as_str())
            } else {
                (me_id.as_str(), focus.id.as_str())
            };
            if !drawn.insert(pair) {
                continue;
            }
            if let Some(other) = id_to_focus.get(me_id.as_str()) {
                let a = node_center_z(focus, origin, min_x, min_y, z);
                let b = node_center_z(other, origin, min_x, min_y, z);
                let mid = Pos2::new((a.x + b.x) * 0.5, (a.y + b.y) * 0.5);
                let sz = 8.0 * z;
                let red = Color32::from_rgb(200, 60, 60);
                painter.line_segment(
                    [
                        Pos2::new(mid.x - sz, mid.y - sz),
                        Pos2::new(mid.x + sz, mid.y + sz),
                    ],
                    Stroke::new(2.5, red),
                );
                painter.line_segment(
                    [
                        Pos2::new(mid.x + sz, mid.y - sz),
                        Pos2::new(mid.x - sz, mid.y + sz),
                    ],
                    Stroke::new(2.5, red),
                );
            }
        }
    }
}
