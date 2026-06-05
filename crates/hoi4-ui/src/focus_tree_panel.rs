//! Focus tree panel — node-line rendering with scroll/zoom/tooltip/selection.

#![allow(deprecated)]

use crate::{i18n::tr, ActiveDetailPanel, FocusDetailTarget, PanelCommand};
use egui::{CornerRadius, Pos2, Rect, Stroke, Vec2};
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
    Panel(PanelCommand),
}

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
        v9_show_focus_tree(
            self,
            ctx,
            tree,
            completed,
            current_focus,
            current_progress,
            available_focus_ids,
        )
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
                    ("总数", tree.focuses.len().to_string(), palette::PARCHMENT),
                    (tr("completed"), completed.len().to_string(), palette::GOOD),
                    (tr("available"), available_count.to_string(), palette::GOLD),
                    (
                        "缩放",
                        format!("{:.0}%", panel.zoom * 100.0),
                        palette::BRASS_BRIGHT,
                    ),
                ],
            );
            draw_tab_strip(ui, layout.tabs, "国策树 / 节点状态", palette::GOLD);
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
                                *cmd = Some(FocusCommand::Panel(PanelCommand::OpenDetail(
                                    ActiveDetailPanel::Focus(FocusDetailTarget {
                                        focus_id: focus.id.clone(),
                                    }),
                                )));
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
