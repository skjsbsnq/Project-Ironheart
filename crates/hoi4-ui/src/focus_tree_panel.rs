//! Focus tree panel — node-line rendering with scroll/zoom/tooltip/selection.

#![allow(deprecated)]

use crate::icons::IconBank;
use crate::vanilla_gui::VanillaPanelProfile;
use crate::{i18n::tr, ActiveDetailPanel, FocusDetailTarget, PanelCommand};
use egui::{Color32, CornerRadius, Pos2, Rect, Sense, Stroke, Vec2};
use hoi4_content::focus::{Effect, Focus, FocusTree};
use std::collections::{BTreeSet, HashMap, HashSet};

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
const NATIONAL_FOCUS_GUI_FILE: &str = crate::vanilla_gui::NATIONAL_FOCUS_GUI_FILE;
const NATIONAL_FOCUS_PROFILE_ID: &str = crate::vanilla_gui::NATIONAL_FOCUS_PROFILE_ID;
const NATIONAL_FOCUS_ROOT: &str = crate::vanilla_gui::NATIONAL_FOCUS_ROOT;
const NATIONAL_FOCUS_ITEM_TEMPLATE: &str = "national_focus_item";
const NATIONAL_FOCUS_LINK_TEMPLATE: &str = "national_focus_link";
const NATIONAL_FOCUS_EXCLUSIVE_TEMPLATE: &str = "national_focus_exclusive_item";
const NATIONAL_FOCUS_DETAIL_TEMPLATE: &str = "national_focus_detail_view";
const FOCUS_SELECT_PREFIX: &str = "focus:select:";
const FOCUS_START_PREFIX: &str = "focus:start:";
const FOCUS_DETAIL_CLOSE_COMMAND: &str = "focus:detail:close";
const FOCUS_CLOSE_COMMAND: &str = "close";
const FOCUS_ZOOM_IN_COMMAND: &str = "focus:zoom:in";
const FOCUS_ZOOM_OUT_COMMAND: &str = "focus:zoom:out";
const FOCUS_SCROLL_ID: &str = "nationalfocusview_tree_scroll";
const FOCUS_ITEM_BG_TOP: f32 = 40.0;
const FOCUS_ITEM_BG_HEIGHT: f32 = 84.0;
const FOCUS_ICON_LINK_RADIUS: f32 = 40.0;
const FOCUS_CANVAS_PADDING_X: f32 = 180.0;
const FOCUS_CANVAS_PADDING_TOP: f32 = 72.0;
const FOCUS_CANVAS_PADDING_BOTTOM: f32 = 140.0;
const FOCUS_NODE_GAP_X: f32 = 32.0;
const FOCUS_NODE_GAP_Y: f32 = 28.0;

type VanillaFocusGuiContext = crate::vanilla_gui::VanillaGuiRuntimeContext;

/// Focus tree panel state.
pub struct FocusTreePanel {
    pub open: bool,
    pub zoom: f32,
    pub pan: Vec2,
    /// Currently selected (highlighted) focus id, awaiting confirm.
    pub selected: Option<String>,
}

impl FocusTreePanel {
    pub fn new() -> Self {
        Self {
            open: false,
            zoom: 1.0,
            pan: Vec2::ZERO,
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
        let Some(context) = focus_runtime_context() else {
            return vanilla_focus_runtime_unavailable_panel(self, ctx);
        };
        let mut icon_bank = IconBank::new(ctx.clone(), context.path_cfg.clone());
        icon_bank.add_profile_search_dirs(NATIONAL_FOCUS_PROFILE_ID);
        self.show_with_icon_bank(
            ctx,
            tree,
            completed,
            current_focus,
            current_progress,
            available_focus_ids,
            &mut icon_bank,
        )
    }

    pub fn show_with_icon_bank(
        &mut self,
        ctx: &egui::Context,
        tree: &FocusTree,
        completed: &HashSet<String>,
        current_focus: Option<&str>,
        current_progress: f32,
        available_focus_ids: &HashSet<String>,
        icon_bank: &mut IconBank,
    ) -> Option<FocusCommand> {
        if !self.open {
            return None;
        }
        vanilla_show_focus_tree(
            self,
            ctx,
            tree,
            completed,
            current_focus,
            current_progress,
            available_focus_ids,
            icon_bank,
        )
    }
}

pub struct NationalFocusProfile;

impl crate::vanilla_gui::VanillaPanelProfile for NationalFocusProfile {
    type Data = FocusTree;
    type Command = FocusCommand;

    fn profile_id(&self) -> &'static str {
        NATIONAL_FOCUS_PROFILE_ID
    }

    fn root_template(&self) -> &'static str {
        NATIONAL_FOCUS_ROOT
    }

    fn required_gui_files(&self) -> &'static [&'static str] {
        &[NATIONAL_FOCUS_GUI_FILE]
    }

    fn key_templates(&self) -> &'static [&'static str] {
        &[
            NATIONAL_FOCUS_ITEM_TEMPLATE,
            NATIONAL_FOCUS_LINK_TEMPLATE,
            NATIONAL_FOCUS_EXCLUSIVE_TEMPLATE,
            NATIONAL_FOCUS_DETAIL_TEMPLATE,
        ]
    }

    fn required_sprites(&self) -> &'static [&'static str] {
        crate::vanilla_gui::FOCUS_REQUIRED_SPRITES
    }

    fn bind_node(
        &self,
        node_path: &crate::vanilla_gui::GuiNodePath,
        data: &Self::Data,
    ) -> crate::vanilla_gui::GuiBinding {
        let name = node_path.0.last().map(String::as_str).unwrap_or_default();
        match name {
            "national_focus_title" => crate::vanilla_gui::GuiBinding::default().text(format!(
                "{} - {}",
                tr("national_focus"),
                data.country
            )),
            "close_button" => crate::vanilla_gui::GuiBinding::default()
                .tooltip(tr("panel_close_hint"))
                .click(FOCUS_CLOSE_COMMAND),
            "zoom_in" => crate::vanilla_gui::GuiBinding::default()
                .tooltip("Zoom in")
                .click(FOCUS_ZOOM_IN_COMMAND),
            "zoom_out" => crate::vanilla_gui::GuiBinding::default()
                .tooltip("Zoom out")
                .click(FOCUS_ZOOM_OUT_COMMAND),
            "to_find_text" => crate::vanilla_gui::GuiBinding::default()
                .input_text("")
                .enabled(false),
            "type_to_search_text" | "found_text" => {
                crate::vanilla_gui::GuiBinding::default().visible(false)
            }
            "filter_container"
            | "filter_grid_main_container"
            | "shortcut_grid_container"
            | "toggle_shortcuts"
            | "continuous_focus_window"
            | "search_tooltip"
            | "tutorial_glow_1"
            | "tutorial_glow_2"
            | "tutorial_glow_3"
            | "tutorial_glow_4" => crate::vanilla_gui::GuiBinding::default().visible(false),
            _ => crate::vanilla_gui::GuiBinding::default(),
        }
    }

    fn handle_action(
        &self,
        action: crate::vanilla_gui::GuiAction,
        _data: &Self::Data,
    ) -> Option<Self::Command> {
        if action.kind != crate::vanilla_gui::GuiActionKind::Click {
            return None;
        }
        focus_command_from_click(action.node_path.to_string())
    }
}

fn focus_runtime_context() -> Option<&'static VanillaFocusGuiContext> {
    static CACHE: std::sync::OnceLock<Option<VanillaFocusGuiContext>> = std::sync::OnceLock::new();
    CACHE
        .get_or_init(|| VanillaFocusGuiContext::load(&[NATIONAL_FOCUS_GUI_FILE]))
        .as_ref()
}

pub fn warm_national_focus_runtime(icon_bank: &mut IconBank, tree: Option<&FocusTree>) {
    let context = focus_runtime_context();
    let profile = NationalFocusProfile;
    icon_bank.add_profile_search_dirs(profile.profile_id());
    let _ = icon_bank.diagnose_sprites(crate::vanilla_gui::FOCUS_REQUIRED_SPRITES.iter().copied());
    if let Some(context) = context {
        let sprites = crate::vanilla_gui::collect_profile_gfx_references(
            context,
            &crate::vanilla_gui::NATIONAL_FOCUS_DESCRIPTOR,
        );
        let _ = icon_bank.diagnose_sprites(sprites.iter().map(String::as_str));
    }
    let mut icons: Vec<&str> = tree
        .into_iter()
        .flat_map(|tree| tree.focuses.iter())
        .map(|focus| focus.icon.as_str())
        .filter(|icon| !icon.trim().is_empty())
        .collect();
    icons.push("GFX_goal_unknown");
    icons.sort_unstable();
    icons.dedup();
    let _ = icon_bank.diagnose_sprites(icons);
}

#[allow(clippy::too_many_arguments)]
fn vanilla_show_focus_tree(
    panel: &mut FocusTreePanel,
    ctx: &egui::Context,
    tree: &FocusTree,
    completed: &HashSet<String>,
    current_focus: Option<&str>,
    current_progress: f32,
    available_focus_ids: &HashSet<String>,
    icon_bank: &mut IconBank,
) -> Option<FocusCommand> {
    let profile = NationalFocusProfile;
    let Some(context) = focus_runtime_context() else {
        return vanilla_focus_runtime_unavailable_panel(panel, ctx);
    };
    let Some(root) = context.root_template(NATIONAL_FOCUS_GUI_FILE, profile.root_template()) else {
        return vanilla_focus_runtime_unavailable_panel(panel, ctx);
    };

    let screen = ctx.screen_rect();
    let viewport = crate::vanilla_gui::GuiRect::new(
        screen.left(),
        screen.top(),
        screen.width(),
        screen.height(),
    );
    let root_layout = crate::vanilla_gui::compute_layout_tree(
        root,
        &crate::vanilla_gui::LayoutOptions::new(viewport).shown_position(true),
    );
    icon_bank.add_profile_search_dirs(profile.profile_id());

    let mut command = None;
    let mut close = false;
    egui::Area::new(egui::Id::new("nationalfocusview_vanilla_runtime"))
        .order(egui::Order::Foreground)
        .fixed_pos(screen.min)
        .show(ctx, |ui| {
            let outer: Rect = root_layout.rect.into();
            ui.interact(
                outer.intersect(screen),
                ui.id().with("nationalfocusview_drag_region"),
                Sense::hover(),
            );
            crate::v9::paint::paint_shadow(ui.painter(), outer, crate::v9::Elevation::E2, 1.0);
            let renderer = crate::vanilla_gui::VanillaGuiRenderer::new(&context.gfx_index);
            let bindings = crate::vanilla_gui::bind_profile_tree(&profile, root, tree);
            let mut stats = renderer.paint_tree(ui, root, &root_layout, &bindings, icon_bank);
            stats.merge(paint_focus_template_instances(
                ui,
                &renderer,
                context,
                root,
                &root_layout,
                panel,
                tree,
                completed,
                current_focus,
                current_progress,
                available_focus_ids,
                icon_bank,
            ));

            for clicked in &stats.clicked_commands {
                match clicked.as_str() {
                    FOCUS_CLOSE_COMMAND => close = true,
                    FOCUS_ZOOM_IN_COMMAND => panel.zoom = (panel.zoom + 0.1).min(MAX_ZOOM),
                    FOCUS_ZOOM_OUT_COMMAND => panel.zoom = (panel.zoom - 0.1).max(MIN_ZOOM),
                    FOCUS_DETAIL_CLOSE_COMMAND => panel.selected = None,
                    _ => {
                        if let Some(id) = clicked.strip_prefix(FOCUS_SELECT_PREFIX) {
                            panel.selected = Some(id.to_owned());
                            continue;
                        }
                        if let Some(next) = profile.handle_action(
                            crate::vanilla_gui::GuiAction {
                                node_path: crate::vanilla_gui::GuiNodePath::root(clicked.clone()),
                                kind: crate::vanilla_gui::GuiActionKind::Click,
                            },
                            tree,
                        ) {
                            command = Some(next);
                        }
                    }
                }
            }
            log_focus_render_stats(ctx, &stats, icon_bank);
        });

    if close {
        panel.open = false;
    }
    command
}

fn vanilla_focus_runtime_unavailable_panel(
    panel: &mut FocusTreePanel,
    ctx: &egui::Context,
) -> Option<FocusCommand> {
    let screen = ctx.screen_rect();
    let panel_rect = Rect::from_min_size(
        Pos2::new(screen.left() + 16.0, screen.top() + 92.0),
        Vec2::new(620.0_f32.min((screen.width() - 32.0).max(280.0)), 214.0),
    );
    let report = crate::vanilla_gui::VanillaGuiRuntimeUnavailableReport::for_profile(
        &crate::vanilla_gui::NATIONAL_FOCUS_DESCRIPTOR,
    );
    let mut close = false;
    egui::Area::new(egui::Id::new("nationalfocusview_runtime_unavailable"))
        .order(egui::Order::Foreground)
        .fixed_pos(panel_rect.min)
        .show(ctx, |ui| {
            let local = Rect::from_min_size(Pos2::ZERO, panel_rect.size());
            let response = ui.allocate_rect(local, Sense::click_and_drag());
            let painter = ui.painter();
            painter.rect_filled(local, 1.0, Color32::from_rgb(0x0a, 0x0d, 0x0c));
            painter.rect_stroke(
                local,
                1.0,
                egui::Stroke::new(1.0, Color32::from_rgb(0x6e, 0x5a, 0x35)),
                egui::StrokeKind::Inside,
            );
            painter.text(
                Pos2::new(local.left() + 16.0, local.top() + 16.0),
                egui::Align2::LEFT_TOP,
                "Vanilla national focus runtime unavailable",
                crate::v9::TextRole::Heading.font_id(),
                crate::vanilla_iron::VanillaIron::TEXT,
            );
            painter.text(
                Pos2::new(local.left() + 16.0, local.top() + 56.0),
                egui::Align2::LEFT_TOP,
                report.reason,
                crate::v9::TextRole::Body.font_id(),
                crate::vanilla_iron::VanillaIron::MUTED,
            );
            painter.text(
                Pos2::new(local.left() + 16.0, local.top() + 92.0),
                egui::Align2::LEFT_TOP,
                format!("required: {}", NATIONAL_FOCUS_GUI_FILE),
                crate::v9::TextRole::Caption.font_id(),
                crate::vanilla_iron::VanillaIron::MUTED,
            );
            let close_rect = Rect::from_min_size(
                Pos2::new(local.right() - 38.0, local.top() + 10.0),
                Vec2::splat(26.0),
            );
            if ui
                .put(close_rect, egui::Button::new("X"))
                .on_hover_text(tr("panel_close_hint"))
                .clicked()
            {
                close = true;
            }
            response.on_hover_cursor(egui::CursorIcon::Grab);
        });
    if close {
        panel.open = false;
    }
    None
}

#[allow(clippy::too_many_arguments)]
fn paint_focus_template_instances(
    ui: &mut egui::Ui,
    renderer: &crate::vanilla_gui::VanillaGuiRenderer<'_>,
    context: &VanillaFocusGuiContext,
    root: &crate::vanilla_gui::GuiNode,
    root_layout: &crate::vanilla_gui::LayoutNode,
    panel: &mut FocusTreePanel,
    tree: &FocusTree,
    completed: &HashSet<String>,
    current_focus: Option<&str>,
    current_progress: f32,
    available_focus_ids: &HashSet<String>,
    icon_bank: &mut IconBank,
) -> crate::vanilla_gui::RenderStats {
    let Some(grid_layout) = root_layout
        .find_by_name("grid_window")
        .and_then(|grid_window| grid_window.find_by_name("grid"))
    else {
        return crate::vanilla_gui::RenderStats::default();
    };
    let Some(document) = context.document(NATIONAL_FOCUS_GUI_FILE) else {
        return crate::vanilla_gui::RenderStats::default();
    };

    let metrics = FocusTreeMetrics::new(root, document, tree, grid_layout.rect);
    handle_focus_tree_view_input(ui, panel, &metrics, grid_layout.rect);
    let canvas_size = metrics.canvas_size(panel.zoom, grid_layout.rect);
    let mut total = crate::vanilla_gui::RenderStats::default();

    let grid_rect: Rect = grid_layout.rect.into();
    let tree_rect = grid_rect.intersect(Rect::from(grid_layout.clip_rect));
    let clip = tree_rect;
    ui.allocate_ui_at_rect(tree_rect, |ui| {
        ui.set_clip_rect(clip);
        let output = egui::ScrollArea::both()
            .id_salt(FOCUS_SCROLL_ID)
            .auto_shrink([false, false])
            .scroll_offset(panel.pan)
            .max_width(tree_rect.width().max(1.0))
            .max_height(tree_rect.height().max(1.0))
            .show_viewport(ui, |ui, viewport| {
                ui.set_clip_rect(clip);
                let _ = ui.allocate_exact_size(canvas_size, Sense::hover());
                let scroll = gui_point_from_pos(viewport.min);
                total.merge(paint_focus_links(
                    ui,
                    renderer,
                    document,
                    root,
                    grid_layout,
                    tree,
                    completed,
                    &metrics,
                    panel.zoom,
                    scroll,
                    icon_bank,
                ));
                total.merge(paint_focus_exclusive_links(
                    ui,
                    renderer,
                    document,
                    grid_layout,
                    tree,
                    &metrics,
                    panel.zoom,
                    scroll,
                    icon_bank,
                ));
                total.merge(paint_focus_items(
                    ui,
                    renderer,
                    document,
                    grid_layout,
                    panel,
                    tree,
                    completed,
                    current_focus,
                    current_progress,
                    available_focus_ids,
                    &metrics,
                    panel.zoom,
                    scroll,
                    icon_bank,
                ));
            });
        panel.pan = clamp_focus_pan(
            output.state.offset,
            output.content_size,
            output.inner_rect.size(),
        );
        if panel.pan != output.state.offset {
            let mut state = output.state;
            state.offset = panel.pan;
            state.store(ui.ctx(), output.id);
        }
    });

    total.merge(paint_focus_detail_view(
        ui,
        renderer,
        document,
        panel,
        tree,
        completed,
        current_focus,
        current_progress,
        available_focus_ids,
        icon_bank,
    ));
    total
}

fn handle_focus_tree_view_input(
    ui: &mut egui::Ui,
    panel: &mut FocusTreePanel,
    metrics: &FocusTreeMetrics,
    grid_rect: crate::vanilla_gui::GuiRect,
) {
    let grid: Rect = grid_rect.into();
    let Some(pointer) = ui.ctx().input(|input| input.pointer.latest_pos()) else {
        return;
    };
    if !grid.contains(pointer) {
        return;
    }

    let wheel = ui.ctx().input(|input| {
        if input.smooth_scroll_delta.y.abs() > f32::EPSILON {
            input.smooth_scroll_delta.y
        } else {
            input.raw_scroll_delta.y
        }
    });
    if wheel.abs() <= f32::EPSILON {
        return;
    }

    let old_zoom = panel.zoom;
    let new_zoom = (old_zoom * (wheel * 0.001).exp()).clamp(MIN_ZOOM, MAX_ZOOM);
    if (new_zoom - old_zoom).abs() <= f32::EPSILON {
        ui.ctx().input_mut(|input| {
            input.smooth_scroll_delta = Vec2::ZERO;
            input.raw_scroll_delta = Vec2::ZERO;
        });
        return;
    }

    let origin = Pos2::new(grid_rect.x, grid_rect.y);
    let pointer_from_origin = pointer - origin;
    let zoom_ratio = new_zoom / old_zoom;
    panel.pan = (pointer_from_origin + panel.pan) * zoom_ratio - pointer_from_origin;
    panel.zoom = new_zoom;
    let canvas_size = metrics.canvas_size(panel.zoom, grid_rect);
    panel.pan = clamp_focus_pan(panel.pan, canvas_size, grid.size());
    ui.ctx().input_mut(|input| {
        input.smooth_scroll_delta = Vec2::ZERO;
        input.raw_scroll_delta = Vec2::ZERO;
    });
}

fn clamp_focus_pan(pan: Vec2, content_size: Vec2, viewport_size: Vec2) -> Vec2 {
    let max_x = (content_size.x - viewport_size.x).max(0.0);
    let max_y = (content_size.y - viewport_size.y).max(0.0);
    Vec2::new(pan.x.clamp(0.0, max_x), pan.y.clamp(0.0, max_y))
}

fn gui_rect_origin(rect: crate::vanilla_gui::GuiRect) -> crate::vanilla_gui::GuiPoint {
    crate::vanilla_gui::GuiPoint {
        x: rect.x,
        y: rect.y,
    }
}

fn gui_point_from_pos(pos: Pos2) -> crate::vanilla_gui::GuiPoint {
    crate::vanilla_gui::GuiPoint { x: pos.x, y: pos.y }
}

#[allow(clippy::too_many_arguments)]
fn paint_focus_items(
    ui: &mut egui::Ui,
    renderer: &crate::vanilla_gui::VanillaGuiRenderer<'_>,
    document: &crate::vanilla_gui::GuiDocument,
    grid_layout: &crate::vanilla_gui::LayoutNode,
    panel: &FocusTreePanel,
    tree: &FocusTree,
    completed: &HashSet<String>,
    current_focus: Option<&str>,
    current_progress: f32,
    available_focus_ids: &HashSet<String>,
    metrics: &FocusTreeMetrics,
    zoom: f32,
    scroll: crate::vanilla_gui::GuiPoint,
    icon_bank: &mut IconBank,
) -> crate::vanilla_gui::RenderStats {
    let Some(template) = document.template_index().get(NATIONAL_FOCUS_ITEM_TEMPLATE) else {
        return crate::vanilla_gui::RenderStats::default();
    };
    let instancer =
        crate::vanilla_gui::TemplateInstancer::new(NATIONAL_FOCUS_ITEM_TEMPLATE, template);
    let rects = tree
        .focuses
        .iter()
        .map(|focus| metrics.template_rect(focus.position));
    let instances = instancer.rect_instances_with_options(
        &grid_layout.path,
        rects,
        crate::vanilla_gui::TemplateInstanceOptions::default()
            .template_size(true)
            .zoom(zoom)
            .transform_origin(gui_rect_origin(grid_layout.rect))
            .scroll_offset(scroll),
    );
    let focus_by_index: Vec<&Focus> = tree.focuses.iter().collect();
    instancer.paint_instances_with_bindings(
        ui,
        renderer,
        instances,
        |instance| {
            let focus = focus_by_index[instance.index];
            let state = focus_state(
                focus,
                completed,
                current_focus,
                current_progress,
                available_focus_ids.contains(&focus.id),
            );
            focus_item_bindings(
                &instance.path,
                focus,
                state,
                panel.selected.as_deref() == Some(focus.id.as_str()),
            )
        },
        icon_bank,
    )
}

#[allow(clippy::too_many_arguments)]
fn paint_focus_links(
    ui: &mut egui::Ui,
    renderer: &crate::vanilla_gui::VanillaGuiRenderer<'_>,
    document: &crate::vanilla_gui::GuiDocument,
    root: &crate::vanilla_gui::GuiNode,
    grid_layout: &crate::vanilla_gui::LayoutNode,
    tree: &FocusTree,
    completed: &HashSet<String>,
    metrics: &FocusTreeMetrics,
    zoom: f32,
    scroll: crate::vanilla_gui::GuiPoint,
    icon_bank: &mut IconBank,
) -> crate::vanilla_gui::RenderStats {
    let Some(template) = document.template_index().get(NATIONAL_FOCUS_LINK_TEMPLATE) else {
        return crate::vanilla_gui::RenderStats::default();
    };
    let id_to_focus: HashMap<&str, &Focus> = tree
        .focuses
        .iter()
        .map(|focus| (focus.id.as_str(), focus))
        .collect();
    let mut paths = Vec::new();
    let mut links = Vec::new();
    for child in &tree.focuses {
        for group in &child.prerequisites {
            for parent_id in group {
                let Some(parent) = id_to_focus.get(parent_id.as_str()) else {
                    continue;
                };
                let path = focus_link_path(
                    root,
                    metrics,
                    parent.position,
                    child.position,
                    completed.contains(parent_id),
                );
                links.extend(focus_link_segments_from_path(root, &path));
                paths.push(path);
            }
        }
    }
    paint_focus_link_backbones(ui, grid_layout, &paths, zoom, scroll);
    let instancer =
        crate::vanilla_gui::TemplateInstancer::new(NATIONAL_FOCUS_LINK_TEMPLATE, template);
    let rects = links.iter().map(|link| link.rect);
    let instances = instancer.rect_instances_with_options(
        &grid_layout.path,
        rects,
        crate::vanilla_gui::TemplateInstanceOptions::default()
            .zoom(zoom)
            .transform_origin(gui_rect_origin(grid_layout.rect))
            .scroll_offset(scroll),
    );
    instancer.paint_instances_with_bindings(
        ui,
        renderer,
        instances,
        |instance| focus_link_bindings(&instance.path, &links[instance.index]),
        icon_bank,
    )
}

#[allow(clippy::too_many_arguments)]
fn paint_focus_exclusive_links(
    ui: &mut egui::Ui,
    renderer: &crate::vanilla_gui::VanillaGuiRenderer<'_>,
    document: &crate::vanilla_gui::GuiDocument,
    grid_layout: &crate::vanilla_gui::LayoutNode,
    tree: &FocusTree,
    metrics: &FocusTreeMetrics,
    zoom: f32,
    scroll: crate::vanilla_gui::GuiPoint,
    icon_bank: &mut IconBank,
) -> crate::vanilla_gui::RenderStats {
    let Some(template) = document
        .template_index()
        .get(NATIONAL_FOCUS_EXCLUSIVE_TEMPLATE)
    else {
        return crate::vanilla_gui::RenderStats::default();
    };
    let id_to_focus: HashMap<&str, &Focus> = tree
        .focuses
        .iter()
        .map(|focus| (focus.id.as_str(), focus))
        .collect();
    let mut seen = BTreeSet::new();
    let mut links = Vec::new();
    for focus in &tree.focuses {
        for other_id in &focus.mutually_exclusive {
            let Some(other) = id_to_focus.get(other_id.as_str()) else {
                continue;
            };
            let mut pair = [focus.id.as_str(), other.id.as_str()];
            pair.sort();
            if !seen.insert((pair[0].to_owned(), pair[1].to_owned())) {
                continue;
            }
            links.push(focus_exclusive_link_rect(
                metrics,
                focus.position,
                other.position,
            ));
        }
    }
    let instancer =
        crate::vanilla_gui::TemplateInstancer::new(NATIONAL_FOCUS_EXCLUSIVE_TEMPLATE, template);
    let rects = links.iter().copied();
    let instances = instancer.rect_instances_with_options(
        &grid_layout.path,
        rects,
        crate::vanilla_gui::TemplateInstanceOptions::default()
            .zoom(zoom)
            .transform_origin(gui_rect_origin(grid_layout.rect))
            .scroll_offset(scroll),
    );
    instancer.paint_instances_with_bindings(
        ui,
        renderer,
        instances,
        |instance| focus_exclusive_bindings(&instance.path, links[instance.index]),
        icon_bank,
    )
}

#[allow(clippy::too_many_arguments)]
fn paint_focus_detail_view(
    ui: &mut egui::Ui,
    renderer: &crate::vanilla_gui::VanillaGuiRenderer<'_>,
    document: &crate::vanilla_gui::GuiDocument,
    panel: &FocusTreePanel,
    tree: &FocusTree,
    completed: &HashSet<String>,
    current_focus: Option<&str>,
    current_progress: f32,
    available_focus_ids: &HashSet<String>,
    icon_bank: &mut IconBank,
) -> crate::vanilla_gui::RenderStats {
    let Some(selected_id) = panel.selected.as_deref() else {
        return crate::vanilla_gui::RenderStats::default();
    };
    let Some(focus) = tree.focuses.iter().find(|focus| focus.id == selected_id) else {
        return crate::vanilla_gui::RenderStats::default();
    };
    let state = focus_state(
        focus,
        completed,
        current_focus,
        current_progress,
        available_focus_ids.contains(&focus.id),
    );
    let _ = renderer;
    let _ = document;
    paint_focus_detail_overlay(ui, focus, state, current_focus, icon_bank)
}

fn ensure_named_layout_size(
    layout: &mut crate::vanilla_gui::LayoutNode,
    name: &str,
    width: f32,
    height: f32,
) {
    if layout.name.as_deref() == Some(name) {
        if layout.rect.width <= 0.0 {
            layout.rect.width = width;
        }
        if layout.rect.height <= 0.0 {
            layout.rect.height = height;
        }
        if layout.clip_rect.width <= 0.0 {
            layout.clip_rect.width = width;
        }
        if layout.clip_rect.height <= 0.0 {
            layout.clip_rect.height = height;
        }
    }
    for child in &mut layout.children {
        ensure_named_layout_size(child, name, width, height);
    }
}

fn paint_focus_detail_overlay(
    ui: &mut egui::Ui,
    focus: &Focus,
    state: FocusState,
    current_focus: Option<&str>,
    icon_bank: &mut IconBank,
) -> crate::vanilla_gui::RenderStats {
    let mut stats = crate::vanilla_gui::RenderStats::default();
    let screen = ui.ctx().screen_rect();
    let size = Vec2::new(550.0, 550.0);
    let pos = Pos2::new(
        (screen.left() + 500.0).min(screen.right() - size.x - 18.0),
        (screen.top() + 100.0).min(screen.bottom() - size.y - 18.0),
    );
    let rect = Rect::from_min_size(pos, size);
    let painter = ui.painter().clone();

    painter.rect_filled(rect, 0.0, Color32::from_rgb(0x0b, 0x0d, 0x0b));
    painter.rect_stroke(
        rect,
        egui::epaint::CornerRadius::same(1),
        Stroke::new(1.0, Color32::from_rgb(0x35, 0x46, 0x4b)),
        egui::epaint::StrokeKind::Inside,
    );
    painter.rect_stroke(
        rect.shrink(2.0),
        egui::epaint::CornerRadius::same(0),
        Stroke::new(1.0, Color32::from_black_alpha(230)),
        egui::epaint::StrokeKind::Inside,
    );
    let header = Rect::from_min_max(rect.left_top(), Pos2::new(rect.right(), rect.top() + 44.0));
    painter.rect_filled(header, 0.0, Color32::from_rgb(0x13, 0x14, 0x11));
    painter.hline(
        (rect.left() + 8.0)..=(rect.right() - 8.0),
        header.bottom(),
        Stroke::new(1.0, Color32::from_rgb(0x35, 0x46, 0x4b)),
    );

    let close_rect = Rect::from_min_size(
        Pos2::new(rect.right() - 42.0, rect.top() + 9.0),
        Vec2::splat(28.0),
    );
    let close = ui
        .put(close_rect, egui::Button::new("X"))
        .on_hover_text(tr("panel_close_hint"));
    stats.buttons += 1;
    if close.clicked() {
        stats
            .clicked_commands
            .push(FOCUS_DETAIL_CLOSE_COMMAND.to_owned());
    }

    paint_focus_detail_text(
        &painter,
        Rect::from_min_size(
            Pos2::new(rect.left() + 30.0, rect.top() + 12.0),
            Vec2::new(450.0, 30.0),
        ),
        &focus_display_name(focus),
        crate::v9::TextRole::Heading.font_id(),
        Color32::from_rgb(0xd8, 0xd6, 0xc8),
        egui::Align2::CENTER_CENTER,
    );
    stats.text_painted += 1;

    let icon_rect = Rect::from_min_size(
        Pos2::new(rect.left() + 42.0, rect.top() + 88.0),
        Vec2::splat(86.0),
    );
    paint_focus_detail_region(
        &painter,
        icon_rect.expand(6.0),
        Color32::from_rgb(0x10, 0x12, 0x10),
    );
    paint_focus_detail_icon(
        &painter,
        icon_bank,
        icon_rect,
        focus_icon_sprite(focus),
        &mut stats,
    );

    let can_start = matches!(state, FocusState::Available) && current_focus.is_none();
    let start_rect = Rect::from_min_size(
        Pos2::new(rect.left() + 390.0, rect.top() + 53.0),
        Vec2::new(123.0, 34.0),
    );
    let start = ui
        .put(start_rect, egui::Button::new("START_FOCUS"))
        .on_hover_text(focus_tooltip(focus, state));
    stats.buttons += 1;
    if can_start && start.clicked() {
        stats
            .clicked_commands
            .push(format!("{FOCUS_START_PREFIX}{}", focus.id));
    }
    if !can_start {
        painter.rect_filled(start_rect, 0.0, Color32::from_black_alpha(125));
    }

    paint_focus_detail_region(
        &painter,
        Rect::from_min_size(
            Pos2::new(rect.left() + 205.0, rect.top() + 92.0),
            Vec2::new(307.0, 113.0),
        ),
        Color32::from_rgb(0x12, 0x13, 0x10),
    );
    paint_focus_detail_region(
        &painter,
        Rect::from_min_size(
            Pos2::new(rect.left() + 22.0, rect.top() + 220.0),
            Vec2::new(506.0, 132.0),
        ),
        Color32::from_rgb(0x0f, 0x11, 0x0f),
    );
    paint_focus_detail_region(
        &painter,
        Rect::from_min_size(
            Pos2::new(rect.left() + 84.0, rect.top() + 363.0),
            Vec2::new(382.0, 172.0),
        ),
        Color32::from_rgb(0x10, 0x12, 0x10),
    );

    paint_focus_detail_text(
        &painter,
        Rect::from_min_size(
            Pos2::new(rect.left() + 208.0, rect.top() + 60.0),
            Vec2::new(170.0, 18.0),
        ),
        &focus_status_text(state),
        crate::v9::TextRole::Body.font_id(),
        Color32::from_rgb(0xd8, 0xd6, 0xc8),
        egui::Align2::CENTER_CENTER,
    );
    paint_focus_detail_text(
        &painter,
        Rect::from_min_size(
            Pos2::new(rect.left() + 215.0, rect.top() + 100.0),
            Vec2::new(287.0, 100.0),
        ),
        &localized_prerequisites(focus),
        crate::v9::TextRole::Caption.font_id(),
        Color32::from_rgb(0xd8, 0xd6, 0xc8),
        egui::Align2::LEFT_TOP,
    );
    paint_focus_detail_text(
        &painter,
        Rect::from_min_size(
            Pos2::new(rect.left() + 34.0, rect.top() + 250.0),
            Vec2::new(485.0, 70.0),
        ),
        &focus_description(focus),
        crate::v9::TextRole::Body.font_id(),
        Color32::from_rgb(0xd8, 0xd6, 0xc8),
        egui::Align2::CENTER_CENTER,
    );
    paint_focus_detail_text(
        &painter,
        Rect::from_min_size(
            Pos2::new(rect.left() + 34.0, rect.top() + 370.0),
            Vec2::new(485.0, 24.0),
        ),
        "Reward",
        crate::v9::TextRole::Body.font_id(),
        Color32::from_rgb(0xd8, 0xd6, 0xc8),
        egui::Align2::CENTER_CENTER,
    );
    paint_focus_detail_text(
        &painter,
        Rect::from_min_size(
            Pos2::new(rect.left() + 50.0, rect.top() + 397.0),
            Vec2::new(455.0, 138.0),
        ),
        &focus_reward_text(focus),
        crate::v9::TextRole::Caption.font_id(),
        Color32::from_rgb(0xd8, 0xd6, 0xc8),
        egui::Align2::LEFT_TOP,
    );
    stats.text_painted += 5;
    stats
}

fn paint_focus_detail_icon(
    painter: &egui::Painter,
    icon_bank: &mut IconBank,
    rect: Rect,
    sprite: &str,
    stats: &mut crate::vanilla_gui::RenderStats,
) {
    if let Some(handle) = icon_bank.get_or_load(sprite) {
        painter.image(
            handle.id(),
            rect,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            Color32::WHITE,
        );
        stats.sprites_painted += 1;
    } else {
        painter.rect_filled(rect, 0.0, Color32::from_rgb(0x16, 0x17, 0x14));
        painter.rect_stroke(
            rect,
            egui::epaint::CornerRadius::same(1),
            Stroke::new(1.0, Color32::from_rgb(0x4d, 0x54, 0x52)),
            egui::epaint::StrokeKind::Inside,
        );
        stats.record_fallback(sprite);
    }
}

fn paint_focus_detail_text(
    painter: &egui::Painter,
    rect: Rect,
    text: &str,
    font: egui::FontId,
    color: Color32,
    align: egui::Align2,
) {
    if text.trim().is_empty() {
        return;
    }
    let galley = painter.layout(text.to_owned(), font, color, rect.width().max(1.0));
    let pos = match align {
        egui::Align2::LEFT_TOP => rect.left_top(),
        egui::Align2::CENTER_CENTER => rect.center() - galley.size() * 0.5,
        _ => rect.center() - galley.size() * 0.5,
    };
    painter.with_clip_rect(rect).galley(pos, galley, color);
}

fn paint_focus_detail_region(painter: &egui::Painter, rect: Rect, fill: Color32) {
    painter.rect_filled(rect, 0.0, fill);
    painter.rect_stroke(
        rect,
        egui::epaint::CornerRadius::same(0),
        Stroke::new(1.0, Color32::from_rgb(0x25, 0x31, 0x34)),
        egui::epaint::StrokeKind::Inside,
    );
    painter.hline(
        (rect.left() + 1.0)..=(rect.right() - 1.0),
        rect.top() + 1.0,
        Stroke::new(1.0, Color32::from_white_alpha(18)),
    );
}

fn focus_item_bindings(
    path: &crate::vanilla_gui::GuiNodePath,
    focus: &Focus,
    state: FocusState,
    selected: bool,
) -> crate::vanilla_gui::GuiBindingMap {
    let mut bindings = crate::vanilla_gui::GuiBindingMap::default();
    bindings.insert_path(
        path.child("bg"),
        crate::vanilla_gui::GuiBinding::default()
            .sprite(focus_bg_sprite(state))
            .click(format!("{FOCUS_SELECT_PREFIX}{}", focus.id))
            .tooltip(focus_tooltip(focus, state)),
    );
    bindings.insert_path(
        path.child("symbol"),
        crate::vanilla_gui::GuiBinding::default()
            .sprite(focus_icon_sprite(focus))
            .click(format!("{FOCUS_SELECT_PREFIX}{}", focus.id))
            .tooltip(focus_tooltip(focus, state)),
    );
    bindings.insert_path(
        path.child("name"),
        crate::vanilla_gui::GuiBinding::default()
            .text(focus_display_name(focus))
            .text_color(match state {
                FocusState::Locked => Color32::from_rgb(0x9f, 0xa1, 0x9a),
                _ => Color32::WHITE,
            }),
    );
    bindings.insert_path(
        path.child("highlight_glow"),
        crate::vanilla_gui::GuiBinding::default().visible(selected),
    );
    bindings.insert_path(
        path.child("continuous_glow"),
        crate::vanilla_gui::GuiBinding::default()
            .visible(matches!(state, FocusState::InProgress(_))),
    );
    for name in [
        "overlay",
        "historical",
        "viewing_flag",
        "viewing_flag_border",
    ] {
        bindings.insert_path(
            path.child(name),
            crate::vanilla_gui::GuiBinding::default().visible(false),
        );
    }
    bindings
}

fn focus_link_bindings(
    path: &crate::vanilla_gui::GuiNodePath,
    link: &FocusLinkSegment,
) -> crate::vanilla_gui::GuiBindingMap {
    let mut bindings = crate::vanilla_gui::GuiBindingMap::default();
    bindings.insert_path(
        path.child("link"),
        crate::vanilla_gui::GuiBinding::default()
            .sprite(link.sprite)
            .frame(link.frame)
            .tint(if link.completed {
                Color32::WHITE
            } else {
                Color32::from_rgb(0x92, 0x96, 0x90)
            }),
    );
    bindings
}

fn paint_focus_link_backbones(
    ui: &mut egui::Ui,
    grid_layout: &crate::vanilla_gui::LayoutNode,
    links: &[FocusLinkPath],
    zoom: f32,
    scroll: crate::vanilla_gui::GuiPoint,
) {
    if links.is_empty() {
        return;
    }
    let origin = gui_rect_origin(grid_layout.rect);
    let painter = ui.painter();
    for link in links {
        let color = if link.completed {
            Color32::from_rgba_premultiplied(0x7e, 0xc8, 0x86, 150)
        } else {
            Color32::from_rgba_premultiplied(0x53, 0x6d, 0x7e, 135)
        };
        let stroke = Stroke::new((2.0 * zoom).clamp(1.0, 3.0), color);
        let points = [
            transform_focus_link_point(link.start, origin, zoom, scroll),
            transform_focus_link_point(link.corner_a, origin, zoom, scroll),
            transform_focus_link_point(link.corner_b, origin, zoom, scroll),
            transform_focus_link_point(link.finish, origin, zoom, scroll),
        ];
        for pair in points.windows(2) {
            if pair[0].distance(pair[1]) >= 1.0 {
                painter.line_segment([pair[0], pair[1]], stroke);
            }
        }
        let joint_radius = (1.25 * zoom).clamp(1.0, 2.0);
        painter.circle_filled(points[1], joint_radius, color);
        painter.circle_filled(points[2], joint_radius, color);
    }
}

fn transform_focus_link_point(
    point: crate::vanilla_gui::GuiPoint,
    origin: crate::vanilla_gui::GuiPoint,
    zoom: f32,
    scroll: crate::vanilla_gui::GuiPoint,
) -> Pos2 {
    Pos2::new(
        origin.x + (point.x - origin.x) * zoom - scroll.x,
        origin.y + (point.y - origin.y) * zoom - scroll.y,
    )
}

fn focus_exclusive_bindings(
    path: &crate::vanilla_gui::GuiNodePath,
    rect: crate::vanilla_gui::GuiRect,
) -> crate::vanilla_gui::GuiBindingMap {
    let mut bindings = crate::vanilla_gui::GuiBindingMap::default();
    bindings.insert_path(
        path.child("left"),
        crate::vanilla_gui::GuiBinding::default()
            .sprite("GFX_focus_link_exclusive")
            .frame(2)
            .tint(Color32::from_rgb(0xd8, 0xb6, 0x52)),
    );
    bindings.insert_path(
        path.child("mid"),
        crate::vanilla_gui::GuiBinding::default()
            .sprite("GFX_focus_link_exclusive")
            .frame(1)
            .tint(Color32::from_rgb(0xd8, 0xb6, 0x52))
            .visible(rect.width >= 24.0),
    );
    bindings.insert_path(
        path.child("right"),
        crate::vanilla_gui::GuiBinding::default()
            .sprite("GFX_focus_link_exclusive")
            .frame(3)
            .tint(Color32::from_rgb(0xd8, 0xb6, 0x52)),
    );
    bindings.insert_path(
        path.child("link1"),
        crate::vanilla_gui::GuiBinding::default()
            .sprite("GFX_focus_exclusive_line1")
            .tint(Color32::from_rgb(0xd8, 0xb6, 0x52)),
    );
    bindings.insert_path(
        path.child("link2"),
        crate::vanilla_gui::GuiBinding::default()
            .sprite("GFX_focus_exclusive_line2")
            .tint(Color32::from_rgb(0xd8, 0xb6, 0x52)),
    );
    bindings
}

fn focus_detail_bindings(
    path: &crate::vanilla_gui::GuiNodePath,
    focus: &Focus,
    state: FocusState,
    current_focus: Option<&str>,
) -> crate::vanilla_gui::GuiBindingMap {
    let mut bindings = crate::vanilla_gui::GuiBindingMap::default();
    bindings.insert_path(
        path.child("close"),
        crate::vanilla_gui::GuiBinding::default()
            .tooltip(tr("panel_close_hint"))
            .click(FOCUS_DETAIL_CLOSE_COMMAND),
    );
    bindings.insert_path(
        path.child("name"),
        crate::vanilla_gui::GuiBinding::default().text(focus_display_name(focus)),
    );
    bindings.insert_path(
        path.child("symbol"),
        crate::vanilla_gui::GuiBinding::default().sprite(focus_icon_sprite(focus)),
    );
    bindings.insert_path(
        path.child("info_top_win"),
        crate::vanilla_gui::GuiBinding::default().visible(false),
    );
    bindings.insert_path(
        path.child("prerequisites"),
        crate::vanilla_gui::GuiBinding::default().text(localized_prerequisites(focus)),
    );
    bindings.insert_path(
        path.child("desc"),
        crate::vanilla_gui::GuiBinding::default().text(focus_description(focus)),
    );
    bindings.insert_path(
        path.child("reward_label"),
        crate::vanilla_gui::GuiBinding::default().text("Reward"),
    );
    bindings.insert_path(
        path.child("reward"),
        crate::vanilla_gui::GuiBinding::default().text(focus_reward_text(focus)),
    );
    bindings.insert_path(
        path.child("start"),
        crate::vanilla_gui::GuiBinding::default()
            .enabled(matches!(state, FocusState::Available) && current_focus.is_none())
            .click(format!("{FOCUS_START_PREFIX}{}", focus.id)),
    );
    bindings.insert_path(
        path.child("research_progressbar"),
        crate::vanilla_gui::GuiBinding::default()
            .visible(matches!(state, FocusState::InProgress(_)))
            .progress(match state {
                FocusState::InProgress(progress) => progress,
                _ => 0.0,
            }),
    );
    bindings.insert_path(
        path.child("research_progressbar_frame"),
        crate::vanilla_gui::GuiBinding::default()
            .visible(matches!(state, FocusState::InProgress(_))),
    );
    bindings.insert_path(
        path.child("status"),
        crate::vanilla_gui::GuiBinding::default().text(focus_status_text(state)),
    );
    bindings
}

#[derive(Debug, Clone, Copy)]
struct FocusLinkSegment {
    rect: crate::vanilla_gui::GuiRect,
    sprite: &'static str,
    frame: u32,
    completed: bool,
}

#[derive(Debug, Clone, Copy)]
struct FocusLinkPath {
    start: crate::vanilla_gui::GuiPoint,
    corner_a: crate::vanilla_gui::GuiPoint,
    corner_b: crate::vanilla_gui::GuiPoint,
    finish: crate::vanilla_gui::GuiPoint,
    completed: bool,
}

#[derive(Debug, Clone)]
struct FocusTreeMetrics {
    bounds: (i32, i32, i32, i32),
    origin: crate::vanilla_gui::GuiPoint,
    marker: crate::vanilla_gui::GuiPoint,
    spacing: crate::vanilla_gui::GuiPoint,
    template_size: crate::vanilla_gui::GuiSize,
    symbol_center: crate::vanilla_gui::GuiPoint,
    canvas_size_unscaled: Vec2,
}

impl FocusTreeMetrics {
    fn new(
        root: &crate::vanilla_gui::GuiNode,
        document: &crate::vanilla_gui::GuiDocument,
        tree: &FocusTree,
        grid_rect: crate::vanilla_gui::GuiRect,
    ) -> Self {
        if tree.focuses.is_empty() {
            return Self {
                bounds: (0, 0, 0, 0),
                origin: gui_rect_origin(grid_rect),
                marker: crate::vanilla_gui::national_focus_center_marker(root).position(),
                spacing: crate::vanilla_gui::focus_spacing_marker(root).position(),
                template_size: focus_template_size(document),
                symbol_center: focus_symbol_center(document),
                canvas_size_unscaled: Vec2::new(
                    grid_rect.width.max(1.0),
                    grid_rect.height.max(1.0),
                ),
            };
        }

        let bounds = tree_bounds(tree);
        let (min_x, max_x, min_y, max_y) = bounds;
        let raw_spacing = crate::vanilla_gui::focus_spacing_marker(root).position();
        let marker = crate::vanilla_gui::national_focus_center_marker(root).position();
        let template_size = focus_template_size(document);
        let symbol_center = focus_symbol_center(document);
        let spacing = effective_focus_spacing(raw_spacing, template_size);
        let content_width =
            ((max_x - min_x) as f32 * spacing.x + template_size.width).max(template_size.width);
        let content_height =
            ((max_y - min_y) as f32 * spacing.y + template_size.height).max(template_size.height);
        let origin = crate::vanilla_gui::GuiPoint {
            x: grid_rect.x + FOCUS_CANVAS_PADDING_X + marker.x,
            y: grid_rect.y + FOCUS_CANVAS_PADDING_TOP + marker.y,
        };
        let canvas_size_unscaled = Vec2::new(
            (FOCUS_CANVAS_PADDING_X * 2.0 + content_width).max(grid_rect.width.max(1.0)),
            (FOCUS_CANVAS_PADDING_TOP + FOCUS_CANVAS_PADDING_BOTTOM + content_height)
                .max(grid_rect.height.max(1.0)),
        );

        Self {
            bounds,
            origin,
            marker,
            spacing,
            template_size,
            symbol_center,
            canvas_size_unscaled,
        }
    }

    fn canvas_size(&self, zoom: f32, fallback: crate::vanilla_gui::GuiRect) -> Vec2 {
        Vec2::new(
            (self.canvas_size_unscaled.x * zoom).max(fallback.width.max(1.0)),
            (self.canvas_size_unscaled.y * zoom).max(fallback.height.max(1.0)),
        )
    }

    fn center(&self, position: (i32, i32)) -> crate::vanilla_gui::GuiPoint {
        let (min_x, _, min_y, _) = self.bounds;
        crate::vanilla_gui::GuiPoint {
            x: self.origin.x + (position.0 - min_x) as f32 * self.spacing.x,
            y: self.origin.y + (position.1 - min_y) as f32 * self.spacing.y,
        }
    }

    fn template_rect(&self, position: (i32, i32)) -> crate::vanilla_gui::GuiRect {
        let center = self.center(position);
        crate::vanilla_gui::GuiRect::new(
            center.x - self.marker.x,
            center.y - self.marker.y,
            self.template_size.width,
            self.template_size.height,
        )
    }

    fn symbol_center(&self, position: (i32, i32)) -> crate::vanilla_gui::GuiPoint {
        let rect = self.template_rect(position);
        crate::vanilla_gui::GuiPoint {
            x: rect.x + self.symbol_center.x,
            y: rect.y + self.symbol_center.y,
        }
    }
}

fn effective_focus_spacing(
    raw: crate::vanilla_gui::GuiPoint,
    template_size: crate::vanilla_gui::GuiSize,
) -> crate::vanilla_gui::GuiPoint {
    crate::vanilla_gui::GuiPoint {
        x: raw.x.max(template_size.width + FOCUS_NODE_GAP_X),
        y: raw.y.max(template_size.height + FOCUS_NODE_GAP_Y),
    }
}

fn focus_link_path(
    root: &crate::vanilla_gui::GuiNode,
    metrics: &FocusTreeMetrics,
    parent_pos: (i32, i32),
    child_pos: (i32, i32),
    completed: bool,
) -> FocusLinkPath {
    let parent_rect = metrics.template_rect(parent_pos);
    let child_rect = metrics.template_rect(child_pos);
    let parent_symbol = metrics.symbol_center(parent_pos);
    let child_symbol = metrics.symbol_center(child_pos);
    let _ = crate::vanilla_gui::link_begin_marker(root).position();
    let _ = crate::vanilla_gui::link_end_marker(root).position();

    // Anchor prerequisite lines on the focus icon itself. The node template is
    // painted after links, so the part under the icon is covered and the
    // visible line touches the icon/title stack without floating gaps.
    let start = crate::vanilla_gui::GuiPoint {
        x: parent_symbol.x,
        y: parent_symbol.y + FOCUS_ICON_LINK_RADIUS,
    };
    let finish = crate::vanilla_gui::GuiPoint {
        x: child_symbol.x,
        y: child_symbol.y - FOCUS_ICON_LINK_RADIUS,
    };
    if (start.x - finish.x).abs() < 1.0 {
        return FocusLinkPath {
            start,
            corner_a: finish,
            corner_b: finish,
            finish,
            completed,
        };
    }

    let step_h = crate::vanilla_gui::link_spacing_marker(root)
        .position()
        .y
        .max(8.0);
    let parent_clear = parent_rect.y + FOCUS_ITEM_BG_TOP + FOCUS_ITEM_BG_HEIGHT;
    let child_clear = child_rect.y + FOCUS_ITEM_BG_TOP;
    let clear_y = if child_clear > parent_clear + step_h {
        parent_clear + ((child_clear - parent_clear) * 0.5)
    } else {
        start.y + (finish.y - start.y) * 0.5
    };
    FocusLinkPath {
        start,
        corner_a: crate::vanilla_gui::GuiPoint {
            x: start.x,
            y: clear_y,
        },
        corner_b: crate::vanilla_gui::GuiPoint {
            x: finish.x,
            y: clear_y,
        },
        finish,
        completed,
    }
}

fn focus_link_segments(
    root: &crate::vanilla_gui::GuiNode,
    metrics: &FocusTreeMetrics,
    parent_pos: (i32, i32),
    child_pos: (i32, i32),
    completed: bool,
) -> Vec<FocusLinkSegment> {
    let path = focus_link_path(root, metrics, parent_pos, child_pos, completed);
    focus_link_segments_from_path(root, &path)
}

fn focus_link_segments_from_path(
    root: &crate::vanilla_gui::GuiNode,
    path: &FocusLinkPath,
) -> Vec<FocusLinkSegment> {
    let link_spacing = crate::vanilla_gui::link_spacing_marker(root).position();
    let step_w = link_spacing.x.max(8.0);
    let step_h = link_spacing.y.max(8.0);
    let mut segments = Vec::new();
    push_vertical_link_tiles(
        &mut segments,
        path.start.x,
        path.start.y,
        path.corner_a.y,
        step_w,
        step_h,
        path.completed,
    );
    push_horizontal_link_tiles(
        &mut segments,
        path.corner_a.x,
        path.corner_b.x,
        path.corner_a.y,
        step_w,
        step_h,
        path.completed,
    );
    push_vertical_link_tiles(
        &mut segments,
        path.finish.x,
        path.corner_b.y,
        path.finish.y,
        step_w,
        step_h,
        path.completed,
    );
    segments
}

fn push_vertical_link_tiles(
    segments: &mut Vec<FocusLinkSegment>,
    x: f32,
    from_y: f32,
    to_y: f32,
    step_w: f32,
    step_h: f32,
    completed: bool,
) {
    let y0 = from_y.min(to_y);
    let y1 = from_y.max(to_y);
    if y1 - y0 < 1.0 {
        return;
    }
    let mut y = y0;
    while y < y1 {
        let height = (y1 - y).min(step_h);
        segments.push(FocusLinkSegment {
            rect: crate::vanilla_gui::GuiRect::new(x - step_w * 0.5, y, step_w, height),
            sprite: "GFX_focus_link_up_down",
            frame: focus_link_frame(completed),
            completed,
        });
        y += height;
    }
}

fn push_horizontal_link_tiles(
    segments: &mut Vec<FocusLinkSegment>,
    from_x: f32,
    to_x: f32,
    y: f32,
    step_w: f32,
    step_h: f32,
    completed: bool,
) {
    let x0 = from_x.min(to_x);
    let x1 = from_x.max(to_x);
    if x1 - x0 < 1.0 {
        return;
    }
    let mut x = x0;
    while x < x1 {
        let width = (x1 - x).min(step_w);
        segments.push(FocusLinkSegment {
            rect: crate::vanilla_gui::GuiRect::new(x, y - step_h * 0.5, width, step_h),
            sprite: "GFX_focus_link_left_right",
            frame: focus_link_frame(completed),
            completed,
        });
        x += width;
    }
}

fn focus_link_frame(completed: bool) -> u32 {
    if completed {
        1
    } else {
        3
    }
}

fn focus_exclusive_link_rect(
    metrics: &FocusTreeMetrics,
    first_pos: (i32, i32),
    second_pos: (i32, i32),
) -> crate::vanilla_gui::GuiRect {
    let first = metrics.center(first_pos);
    let second = metrics.center(second_pos);
    let x = first.x.min(second.x);
    let y = first.y.min(second.y) + 24.0;
    let w = (first.x - second.x).abs().max(32.0);
    crate::vanilla_gui::GuiRect::new(x, y, w, 12.0)
}

fn focus_template_size(document: &crate::vanilla_gui::GuiDocument) -> crate::vanilla_gui::GuiSize {
    document
        .template_index()
        .get(NATIONAL_FOCUS_ITEM_TEMPLATE)
        .map(|template| {
            let layout = crate::vanilla_gui::compute_layout_tree(
                template,
                &crate::vanilla_gui::LayoutOptions::new(crate::vanilla_gui::GuiRect::new(
                    0.0, 0.0, 1.0, 1.0,
                )),
            );
            layout.rect.size()
        })
        .unwrap_or(crate::vanilla_gui::GuiSize {
            width: 165.0,
            height: 128.0,
        })
}

fn focus_symbol_center(document: &crate::vanilla_gui::GuiDocument) -> crate::vanilla_gui::GuiPoint {
    document
        .template_index()
        .get(NATIONAL_FOCUS_ITEM_TEMPLATE)
        .and_then(|template| {
            let layout = crate::vanilla_gui::compute_layout_tree(
                template,
                &crate::vanilla_gui::LayoutOptions::new(crate::vanilla_gui::GuiRect::new(
                    0.0, 0.0, 1.0, 1.0,
                )),
            );
            layout.find_by_name("symbol").map(|symbol| {
                let rect: Rect = symbol.rect.into();
                crate::vanilla_gui::GuiPoint {
                    x: rect.center().x,
                    y: rect.center().y,
                }
            })
        })
        .unwrap_or(crate::vanilla_gui::GuiPoint { x: 5.0, y: -44.0 })
}

fn focus_icon_sprite(focus: &Focus) -> &str {
    if focus.icon.trim().is_empty() {
        "GFX_goal_unknown"
    } else {
        focus.icon.as_str()
    }
}

fn focus_bg_sprite(state: FocusState) -> &'static str {
    match state {
        FocusState::Completed => "GFX_focus_completed",
        FocusState::InProgress(_) | FocusState::Available => "GFX_focus_can_start",
        FocusState::Locked => "GFX_focus_unavailable",
    }
}

fn focus_status_text(state: FocusState) -> String {
    match state {
        FocusState::Completed => tr("focus_completed").to_owned(),
        FocusState::InProgress(p) => format!("{:.0}%", p * 100.0),
        FocusState::Available => tr("available").to_owned(),
        FocusState::Locked => tr("locked").to_owned(),
    }
}

fn focus_tooltip(focus: &Focus, state: FocusState) -> String {
    let mut lines = vec![
        focus_display_name(focus),
        tr("cost_days").replace("{}", &focus.cost_days.to_string()),
        focus_status_text(state),
    ];
    let prerequisites = localized_prerequisites(focus);
    if !prerequisites.is_empty() {
        lines.push(tr("requires").replace("{}", &prerequisites));
    }
    lines.join("\n")
}

fn focus_description(focus: &Focus) -> String {
    if focus.name.trim().is_empty() {
        focus.id.clone()
    } else {
        focus.name.clone()
    }
}

fn focus_reward_text(focus: &Focus) -> String {
    let summaries: Vec<String> = focus
        .completion_effect
        .iter()
        .filter_map(Effect::effect_summary)
        .collect();
    if summaries.is_empty() {
        "No immediate effect".to_owned()
    } else {
        summaries.join("\n")
    }
}

fn focus_command_from_click(command: String) -> Option<FocusCommand> {
    if command.starts_with(FOCUS_SELECT_PREFIX) {
        return None;
    }
    if let Some(id) = command.strip_prefix(FOCUS_START_PREFIX) {
        return Some(FocusCommand::Start(id.to_owned()));
    }
    if command == FOCUS_DETAIL_CLOSE_COMMAND {
        return Some(FocusCommand::Panel(PanelCommand::CloseDetail));
    }
    None
}

fn log_focus_render_stats(
    ctx: &egui::Context,
    stats: &crate::vanilla_gui::RenderStats,
    icon_bank: &IconBank,
) {
    let id = egui::Id::new("nationalfocusview_render_stats_logged");
    let already_logged = ctx
        .data_mut(|d| d.get_persisted::<bool>(id))
        .unwrap_or(false);
    if already_logged {
        return;
    }
    println!(
        "[ui][focus] render nodes={}/{} sprites={} fallback={} text={} buttons={} icon_missing_cache={} fallback_labels={:?}",
        stats.nodes_painted,
        stats.nodes_seen,
        stats.sprites_painted,
        stats.fallback_painted,
        stats.text_painted,
        stats.buttons,
        icon_bank.missing_count(),
        stats.fallback_labels
    );
    ctx.data_mut(|d| d.insert_persisted(id, true));
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

#[cfg(test)]
mod tests {
    use super::*;

    fn focus_marker_root() -> crate::vanilla_gui::GuiNode {
        let doc = crate::vanilla_gui::parse_gui_str(
            None,
            r#"
guiTypes = {
    containerWindowType = {
        name = "nationalfocusview"
        positionType = {
            name = "focus_spacing"
            position = { x = 96 y = 130 }
        }
        positionType = {
            name = "national_focus_center"
            position = { x = 130 y = 32 }
        }
        positionType = {
            name = "link_spacing"
            position = { x = 16 y = 16 }
        }
        positionType = {
            name = "link_begin"
            position = { x = 80 y = 64 }
        }
        positionType = {
            name = "link_end"
            position = { x = 80 y = 0 }
        }
    }
}
"#,
        );
        doc.template_index()
            .get("nationalfocusview")
            .expect("focus root")
            .clone()
    }

    fn focus_marker_document() -> crate::vanilla_gui::GuiDocument {
        crate::vanilla_gui::parse_gui_str(
            None,
            r#"
guiTypes = {
    containerWindowType = {
        name = "national_focus_item"
        size = { width = 165 height = 128 }
    }
}
"#,
        )
    }

    fn metric_tree() -> FocusTree {
        FocusTree {
            country: "TST".to_owned(),
            focuses: vec![
                Focus {
                    id: "parent".to_owned(),
                    name: "Parent".to_owned(),
                    icon: "GFX_goal_unknown".to_owned(),
                    position: (0, 0),
                    cost_days: 70,
                    prerequisites: Vec::new(),
                    mutually_exclusive: Vec::new(),
                    available: hoi4_content::focus::Trigger::AlwaysTrue,
                    completion_effect: Vec::new(),
                },
                Focus {
                    id: "child".to_owned(),
                    name: "Child".to_owned(),
                    icon: "GFX_goal_unknown".to_owned(),
                    position: (0, 1),
                    cost_days: 70,
                    prerequisites: vec![vec!["parent".to_owned()]],
                    mutually_exclusive: Vec::new(),
                    available: hoi4_content::focus::Trigger::AlwaysTrue,
                    completion_effect: Vec::new(),
                },
            ],
        }
    }

    #[test]
    fn focus_links_start_at_icon_anchor() {
        let root = focus_marker_root();
        let document = focus_marker_document();
        let tree = metric_tree();
        let grid = crate::vanilla_gui::GuiRect::new(47.0, 175.0, 1920.0, 1080.0);
        let metrics = FocusTreeMetrics::new(&root, &document, &tree, grid);
        let parent = metrics.template_rect((0, 0));
        let child_anchor = metrics.symbol_center((0, 1));
        let start = metrics.symbol_center((0, 0));
        let segments = focus_link_segments(&root, &metrics, (0, 0), (0, 1), true);

        assert!(!segments.is_empty());
        assert_eq!(segments[0].rect.y, start.y + FOCUS_ICON_LINK_RADIUS);
        assert_eq!(segments[0].sprite, "GFX_focus_link_up_down");
        assert!(segments
            .iter()
            .any(|segment| segment.rect.bottom() >= child_anchor.y - FOCUS_ICON_LINK_RADIUS));
        assert!(parent.x > grid.x + FOCUS_CANVAS_PADDING_X - 1.0);
        assert!(parent.y > grid.y + FOCUS_CANVAS_PADDING_TOP - 1.0);
    }

    #[test]
    fn focus_link_tiles_do_not_overshoot_or_backtrack_into_nodes() {
        let root = focus_marker_root();
        let document = focus_marker_document();
        let tree = FocusTree {
            country: "TST".to_owned(),
            focuses: vec![
                Focus {
                    id: "parent".to_owned(),
                    name: "Parent".to_owned(),
                    icon: "GFX_goal_unknown".to_owned(),
                    position: (0, 0),
                    cost_days: 70,
                    prerequisites: Vec::new(),
                    mutually_exclusive: Vec::new(),
                    available: hoi4_content::focus::Trigger::AlwaysTrue,
                    completion_effect: Vec::new(),
                },
                Focus {
                    id: "child".to_owned(),
                    name: "Child".to_owned(),
                    icon: "GFX_goal_unknown".to_owned(),
                    position: (1, 1),
                    cost_days: 70,
                    prerequisites: vec![vec!["parent".to_owned()]],
                    mutually_exclusive: Vec::new(),
                    available: hoi4_content::focus::Trigger::AlwaysTrue,
                    completion_effect: Vec::new(),
                },
            ],
        };
        let grid = crate::vanilla_gui::GuiRect::new(47.0, 175.0, 1920.0, 1080.0);
        let metrics = FocusTreeMetrics::new(&root, &document, &tree, grid);
        let start = metrics.symbol_center((0, 0));
        let finish = metrics.symbol_center((1, 1));
        let start_x = start.x;
        let finish_x = finish.x;
        let finish_y = finish.y - FOCUS_ICON_LINK_RADIUS;
        let segments = focus_link_segments(&root, &metrics, (0, 0), (1, 1), false);

        let horizontal = segments
            .iter()
            .find(|segment| segment.sprite == "GFX_focus_link_left_right")
            .expect("horizontal link segment");
        assert!(horizontal.rect.x >= start_x - f32::EPSILON);
        assert!(horizontal.rect.right() <= finish_x + f32::EPSILON);
        assert!(segments
            .iter()
            .filter(|segment| {
                segment.sprite == "GFX_focus_link_up_down"
                    && (segment.rect.x + segment.rect.width * 0.5 - finish_x).abs() < 0.5
            })
            .all(|segment| segment.rect.bottom() <= finish_y + f32::EPSILON));
    }

    #[test]
    fn focus_tree_metrics_prevents_template_overlap_when_vanilla_spacing_is_tight() {
        let root = focus_marker_root();
        let document = focus_marker_document();
        let tree = FocusTree {
            country: "TST".to_owned(),
            focuses: vec![
                Focus {
                    id: "left".to_owned(),
                    name: "Left".to_owned(),
                    icon: "GFX_goal_unknown".to_owned(),
                    position: (0, 0),
                    cost_days: 70,
                    prerequisites: Vec::new(),
                    mutually_exclusive: Vec::new(),
                    available: hoi4_content::focus::Trigger::AlwaysTrue,
                    completion_effect: Vec::new(),
                },
                Focus {
                    id: "right".to_owned(),
                    name: "Right".to_owned(),
                    icon: "GFX_goal_unknown".to_owned(),
                    position: (1, 0),
                    cost_days: 70,
                    prerequisites: Vec::new(),
                    mutually_exclusive: Vec::new(),
                    available: hoi4_content::focus::Trigger::AlwaysTrue,
                    completion_effect: Vec::new(),
                },
            ],
        };
        let grid = crate::vanilla_gui::GuiRect::new(47.0, 175.0, 1920.0, 1080.0);
        let metrics = FocusTreeMetrics::new(&root, &document, &tree, grid);
        let left = metrics.template_rect((0, 0));
        let right = metrics.template_rect((1, 0));

        assert!(right.x >= left.right() + FOCUS_NODE_GAP_X - 0.5);
        assert!(metrics.spacing.x >= metrics.template_size.width + FOCUS_NODE_GAP_X);
    }

    #[test]
    fn focus_select_click_only_selects_in_tree_detail() {
        assert_eq!(
            focus_command_from_click("focus:select:phase5_army".to_owned()),
            None
        );
        assert_eq!(
            focus_command_from_click("focus:start:phase5_army".to_owned()),
            Some(FocusCommand::Start("phase5_army".to_owned()))
        );
    }

    #[test]
    fn focus_node_background_sprite_tracks_state() {
        assert_eq!(
            focus_bg_sprite(FocusState::Completed),
            "GFX_focus_completed"
        );
        assert_eq!(
            focus_bg_sprite(FocusState::InProgress(0.4)),
            "GFX_focus_can_start"
        );
        assert_eq!(
            focus_bg_sprite(FocusState::Available),
            "GFX_focus_can_start"
        );
        assert_eq!(focus_bg_sprite(FocusState::Locked), "GFX_focus_unavailable");
    }

    #[test]
    fn focus_tree_pan_clamps_to_canvas_bounds() {
        let clamped = clamp_focus_pan(
            Vec2::new(480.0, -20.0),
            Vec2::new(900.0, 600.0),
            Vec2::new(640.0, 480.0),
        );

        assert_eq!(clamped, Vec2::new(260.0, 0.0));
    }
}
