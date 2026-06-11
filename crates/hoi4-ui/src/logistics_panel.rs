//! J.4b: 后勤面板。快捷键 L 开关。
//!
//! 面板结构来自 HOI4 原版 `countrylogisticsview` runtime，数据仍由项目后勤数据链路提供。

use crate::{i18n::tr, icons::IconBank, vanilla_gui::VanillaPanelProfile, PanelCommand};
use egui::{Color32, Pos2, Rect, Sense, Vec2};

pub use crate::logistics_profile::CountryLogisticsProfile;

pub const LOGISTICS_VANILLA_SNAPSHOT_1080P: &str =
    "crates/hoi4-ui/tests/snapshots/logistics_vanilla_1080p.png";
const LOGISTICS_MATERIEL_ROW_HEIGHT: f32 = 58.0;

/// 单条装备库存条目。
#[derive(Clone, Debug, PartialEq)]
pub struct LogisticsEntry {
    pub id: String,
    pub name: String,
    pub kind: LogisticsEntryKind,
    pub equipment_icon_sprite: Option<String>,
    pub stockpile: f32,
    pub daily_production: f32,
    pub daily_replenishment_need: f32,
    pub daily_training_need: f32,
    pub daily_maintenance_need: f32,
    pub daily_consumption: f32,
    pub net_change: f32,
    pub deficit: f32,
    pub days_until_empty: Option<f32>,
    pub procurement_rm: f64,
    pub production_sources: Vec<String>,
    pub resource_inputs: Vec<ResourceInputEntry>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogisticsEntryKind {
    Land,
    Naval,
    Air,
    Other,
}

pub type LogisticsVanillaEntryKind = LogisticsEntryKind;

#[derive(Clone, Debug, PartialEq)]
pub struct ResourceInputEntry {
    pub id: String,
    pub name: String,
    pub amount: f32,
}

pub fn logistics_entry_kind_for_id(equipment_id: &str) -> LogisticsEntryKind {
    match equipment_id {
        "aircraft" | "飞机" => LogisticsEntryKind::Air,
        "naval_vessel" | "convoy" | "舰艇" | "运输船" => LogisticsEntryKind::Naval,
        "infantry_equipment" | "artillery" | "anti_tank" | "anti_air" | "support_equipment"
        | "motorized" | "mechanized" | "armor" | "train" | "步兵装备" | "火炮" | "反坦克炮"
        | "防空炮" | "支援装备" | "摩托化装备" | "机械化装备" | "装甲车辆" | "火车" => {
            LogisticsEntryKind::Land
        }
        _ => LogisticsEntryKind::Other,
    }
}

pub fn logistics_vanilla_entry_kind(equipment_id: &str) -> LogisticsEntryKind {
    logistics_entry_kind_for_id(equipment_id)
}

pub fn logistics_equipment_icon_sprite_for_id(equipment_id: &str) -> Option<&'static str> {
    match equipment_id {
        "infantry_equipment" | "步兵装备" => Some("GFX_archetype_infantry_equipment_medium"),
        "artillery" | "火炮" => Some("GFX_archetype_artillery_equipment_medium"),
        "anti_tank" | "反坦克炮" => Some("GFX_archetype_anti_tank_equipment_medium"),
        "anti_air" | "防空炮" => Some("GFX_archetype_anti_air_equipment_medium"),
        "support_equipment" | "支援装备" => Some("GFX_archetype_support_equipment_medium"),
        "motorized" | "摩托化装备" => Some("GFX_archetype_motorized_equipment_medium"),
        "mechanized" | "机械化装备" => Some("GFX_mechanised_infantry_medium"),
        "armor" | "装甲车辆" => Some("GFX_archetype_medium_tank_equipment_medium"),
        "train" | "火车" => Some("GFX_archetype_train_medium"),
        "aircraft" | "飞机" => Some("GFX_archetype_fighter_equipment_medium"),
        "naval_vessel" | "舰艇" => Some("GFX_early_destroyer_medium"),
        "convoy" | "运输船" => Some("GFX_archetype_convoy_medium"),
        _ => None,
    }
}

/// 单条资源条目。
#[derive(Clone, Debug, PartialEq)]
pub struct ResourceEntry {
    pub name: String,
    pub produced: f32,
    pub consumed: f32,
    /// 累积仓储量
    pub stored: f32,
}

/// 面板数据快照。
#[derive(Clone, Debug, PartialEq)]
pub struct LogisticsData {
    pub entries: Vec<LogisticsEntry>,
    pub total_types: usize,
    pub deficit_types: usize,
    pub total_daily_production: f32,
    pub total_daily_need: f32,
    pub military_procurement_rm: f64,
    pub military_maintenance_rm: f64,
    /// 战略资源收支
    pub resources: Vec<ResourceEntry>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LogisticsVanillaEntryPlan {
    pub entry_index: usize,
    pub template_name: &'static str,
    pub model_key: String,
    pub kind: LogisticsEntryKind,
}

pub fn logistics_vanilla_summary_text(data: &LogisticsData) -> String {
    let net = data.total_daily_production - data.total_daily_need;
    format!(
        "类型 {} | 缺口 {} | 日产 {} | 需求 -{:.1}/日 | 净变化 {}",
        data.total_types,
        data.deficit_types,
        signed_one_decimal(data.total_daily_production),
        data.total_daily_need,
        signed_one_decimal(net)
    )
}

pub fn logistics_vanilla_entry_plan(data: &LogisticsData) -> Vec<LogisticsVanillaEntryPlan> {
    let mut indexed: Vec<(usize, &LogisticsEntry)> = data.entries.iter().enumerate().collect();
    indexed.sort_by(|(_, a), (_, b)| {
        b.deficit
            .partial_cmp(&a.deficit)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.name.cmp(&b.name))
    });
    indexed
        .into_iter()
        .map(|(entry_index, entry)| {
            let kind = entry.kind;
            LogisticsVanillaEntryPlan {
                entry_index,
                template_name: logistics_template_for_kind(kind),
                model_key: entry_model_key(entry),
                kind,
            }
        })
        .collect()
}

pub fn logistics_vanilla_entry_instance_specs(
    data: &LogisticsData,
    parent_path: crate::vanilla_gui::GuiNodePath,
    parent_grid_node: &crate::vanilla_gui::GuiNode,
    parent_rect: crate::vanilla_gui::GuiRect,
) -> Vec<crate::vanilla_gui::GuiRuntimeInstanceSpec> {
    let mut groups: Vec<(&'static str, Vec<crate::vanilla_gui::GuiRect>, Vec<String>)> = Vec::new();
    let plan = logistics_vanilla_entry_plan(data);
    let slots = logistics_scroll_content_grid_slots(parent_grid_node, parent_rect, plan.len());
    for (plan, rect) in plan.into_iter().zip(slots) {
        if let Some((_, rects, model_keys)) = groups
            .iter_mut()
            .find(|(template_name, _, _)| *template_name == plan.template_name)
        {
            rects.push(rect);
            model_keys.push(plan.model_key);
        } else {
            groups.push((plan.template_name, vec![rect], vec![plan.model_key]));
        }
    }
    groups
        .into_iter()
        .map(|(template_name, rects, model_keys)| {
            crate::vanilla_gui::GuiRuntimeInstanceSpec::absolute_rects(
                template_name,
                parent_path.clone(),
                rects,
            )
            .with_options(
                crate::vanilla_gui::TemplateInstanceOptions::default().template_size(true),
            )
            .with_semantic_role("logistics_equipment_entry")
            .with_model_keys(model_keys)
        })
        .collect()
}

fn logistics_scroll_content_grid_slots(
    parent_grid_node: &crate::vanilla_gui::GuiNode,
    parent_rect: crate::vanilla_gui::GuiRect,
    count: usize,
) -> Vec<crate::vanilla_gui::GuiRect> {
    if count == 0 {
        return Vec::new();
    }
    let Some(first_slot) = crate::vanilla_gui::grid_slots(parent_grid_node, parent_rect, 1)
        .into_iter()
        .next()
    else {
        return Vec::new();
    };
    let expanded_rect = crate::vanilla_gui::GuiRect::new(
        parent_rect.x,
        parent_rect.y,
        parent_rect.width,
        parent_rect
            .height
            .max(first_slot.height.max(1.0) * count as f32),
    );
    crate::vanilla_gui::grid_slots(parent_grid_node, expanded_rect, count)
}

pub fn logistics_vanilla_resource_strip_instance_specs(
    data: &LogisticsData,
    parent_path: crate::vanilla_gui::GuiNodePath,
    parent_grid_node: &crate::vanilla_gui::GuiNode,
    parent_rect: crate::vanilla_gui::GuiRect,
) -> Vec<crate::vanilla_gui::GuiRuntimeInstanceSpec> {
    if data.resources.is_empty() {
        return Vec::new();
    }
    let rects = crate::vanilla_gui::grid_slots(parent_grid_node, parent_rect, data.resources.len());
    vec![crate::vanilla_gui::GuiRuntimeInstanceSpec::absolute_rects(
        "logistics_overview_resource_item",
        parent_path,
        rects,
    )
    .with_options(crate::vanilla_gui::TemplateInstanceOptions::default().template_size(true))
    .with_semantic_role("logistics_resource_strip_item")
    .with_model_keys(data.resources.iter().map(|resource| resource.name.clone()))]
}

pub fn logistics_vanilla_entry_resource_instance_specs(
    entry: &LogisticsEntry,
    parent_path: crate::vanilla_gui::GuiNodePath,
    parent_grid_node: &crate::vanilla_gui::GuiNode,
    parent_rect: crate::vanilla_gui::GuiRect,
) -> Vec<crate::vanilla_gui::GuiRuntimeInstanceSpec> {
    if entry.resource_inputs.is_empty() {
        return Vec::new();
    }
    let rects =
        crate::vanilla_gui::grid_slots(parent_grid_node, parent_rect, entry.resource_inputs.len());
    vec![crate::vanilla_gui::GuiRuntimeInstanceSpec::absolute_rects(
        "logistics_entry_resource_item",
        parent_path,
        rects,
    )
    .with_semantic_role("logistics_entry_resource_item")
    .with_model_keys(entry.resource_inputs.iter().map(|input| input.id.clone()))]
}

pub fn logistics_vanilla_runtime_frame(
    root: &crate::vanilla_gui::GuiNode,
    data: &LogisticsData,
    viewport: crate::vanilla_gui::GuiRect,
    runtime_state: crate::vanilla_gui::GuiRuntimeState,
    registry: crate::vanilla_gui::GuiTemplateRegistry<'_>,
) -> crate::vanilla_gui::GuiRuntimeFrame {
    logistics_vanilla_runtime_frame_parts(root, data, viewport, runtime_state, registry, None, None)
        .frame
}

#[derive(Debug, Clone)]
pub struct LogisticsVanillaRuntimeFrameParts {
    pub frame: crate::vanilla_gui::GuiRuntimeFrame,
    pub bindings: crate::vanilla_gui::GuiBindingMap,
}

pub fn logistics_vanilla_runtime_frame_parts(
    root: &crate::vanilla_gui::GuiNode,
    data: &LogisticsData,
    viewport: crate::vanilla_gui::GuiRect,
    runtime_state: crate::vanilla_gui::GuiRuntimeState,
    registry: crate::vanilla_gui::GuiTemplateRegistry<'_>,
    gfx_index: Option<&crate::vanilla_gui::GfxIndex>,
    icon_bank: Option<&mut IconBank>,
) -> LogisticsVanillaRuntimeFrameParts {
    let mut bindings = crate::vanilla_gui::bind_profile_tree(&CountryLogisticsProfile, root, data);
    let base = crate::vanilla_gui::GuiRuntimeFrame::build(
        crate::vanilla_gui::GuiRuntimeFrameInput::new(
            root,
            viewport,
            crate::vanilla_gui::COUNTRY_LOGISTICS_PROFILE_ID,
            &bindings,
        )
        .with_runtime_state(runtime_state.clone()),
    );
    let mut specs = Vec::new();
    if let Some(resources_grid) = base.root_layout.find_by_name("resources_grid") {
        let resources_grid_node = find_gui_node_by_layout_path(root, &resources_grid.path)
            .or_else(|| root.find_node_by_name("resources_grid"));
        if let Some(resources_grid_node) = resources_grid_node {
            specs.extend(logistics_vanilla_resource_strip_instance_specs(
                data,
                resources_grid.path.clone(),
                resources_grid_node,
                resources_grid.rect,
            ));
        }
    }
    if let Some(materiel_grid) = base.root_layout.find_by_name("materiel_grid") {
        let materiel_grid_node = find_gui_node_by_layout_path(root, &materiel_grid.path)
            .or_else(|| root.find_node_by_name("materiel_grid"));
        if let Some(materiel_grid_node) = materiel_grid_node {
            specs.extend(logistics_vanilla_entry_instance_specs(
                data,
                materiel_grid.path.clone(),
                materiel_grid_node,
                materiel_grid.rect,
            ));
        }
    }
    let mut first_pass_bindings = bindings.clone();
    first_pass_bindings.extend(logistics_vanilla_instance_bindings(data, &specs, &registry));
    let first_pass = crate::vanilla_gui::GuiRuntimeFrame::build(
        crate::vanilla_gui::GuiRuntimeFrameInput::new(
            root,
            viewport,
            crate::vanilla_gui::COUNTRY_LOGISTICS_PROFILE_ID,
            &first_pass_bindings,
        )
        .with_runtime_state(runtime_state.clone())
        .with_template_registry(registry.clone())
        .with_instance_specs(specs.clone()),
    );
    for instance in &first_pass.generated_instances {
        if instance.context.semantic_role.as_deref() != Some("logistics_equipment_entry") {
            continue;
        }
        let Some(model_key) = instance.context.model_key.as_deref() else {
            continue;
        };
        let Some(entry) = data
            .entries
            .iter()
            .find(|entry| entry_model_key(entry) == model_key)
        else {
            continue;
        };
        let Some(resources_grid) = instance.layout.find_by_name("resources_grid") else {
            continue;
        };
        let Some(template) = registry.get(instance.template_name) else {
            continue;
        };
        let Some(resources_grid_node) = template.node.find_node_by_name("resources_grid") else {
            continue;
        };
        specs.extend(logistics_vanilla_entry_resource_instance_specs(
            entry,
            resources_grid.path.clone(),
            resources_grid_node,
            resources_grid.rect,
        ));
    }
    bindings.extend(logistics_vanilla_instance_bindings(data, &specs, &registry));
    let mut input = crate::vanilla_gui::GuiRuntimeFrameInput::new(
        root,
        viewport,
        crate::vanilla_gui::COUNTRY_LOGISTICS_PROFILE_ID,
        &bindings,
    )
    .with_runtime_state(runtime_state)
    .with_template_registry(registry)
    .with_instance_specs(specs);
    if let Some(gfx_index) = gfx_index {
        input = input.with_gfx_index(gfx_index);
    }
    if let Some(icon_bank) = icon_bank {
        input = input.with_icon_bank(icon_bank);
    }
    LogisticsVanillaRuntimeFrameParts {
        frame: crate::vanilla_gui::GuiRuntimeFrame::build(input),
        bindings,
    }
}

fn find_gui_node_by_layout_path<'a>(
    node: &'a crate::vanilla_gui::GuiNode,
    path: &crate::vanilla_gui::GuiNodePath,
) -> Option<&'a crate::vanilla_gui::GuiNode> {
    fn visit<'a>(
        node: &'a crate::vanilla_gui::GuiNode,
        path: &crate::vanilla_gui::GuiNodePath,
        depth: usize,
    ) -> Option<&'a crate::vanilla_gui::GuiNode> {
        if depth >= path.0.len() {
            return Some(node);
        }
        node.children.iter().enumerate().find_map(|(index, child)| {
            (child.path_label(index) == path.0[depth])
                .then(|| visit(child, path, depth + 1))
                .flatten()
        })
    }

    if path
        .0
        .first()
        .is_some_and(|label| label == &node.path_label(0))
    {
        visit(node, path, 1)
    } else {
        None
    }
}

fn logistics_vanilla_instance_bindings(
    data: &LogisticsData,
    specs: &[crate::vanilla_gui::GuiRuntimeInstanceSpec],
    registry: &crate::vanilla_gui::GuiTemplateRegistry<'_>,
) -> crate::vanilla_gui::GuiBindingMap {
    let mut bindings = crate::vanilla_gui::GuiBindingMap::default();
    for spec in specs {
        let Some(template) = registry.get(spec.template_name) else {
            continue;
        };
        let count = match &spec.source {
            crate::vanilla_gui::GuiRuntimeInstanceSource::Descriptor { count }
            | crate::vanilla_gui::GuiRuntimeInstanceSource::Grid { count } => *count,
            crate::vanilla_gui::GuiRuntimeInstanceSource::Absolute { rects } => rects.len(),
        };
        for index in 0..count {
            let path = crate::vanilla_gui::template_instance_path(
                &spec.parent_path,
                spec.template_name,
                index,
            );
            let mut context = crate::vanilla_gui::GuiInstanceContext::new(
                spec.template_name,
                index,
                spec.parent_path.clone(),
            );
            if let Some(role) = spec.semantic_role.clone() {
                context = context.with_semantic_role(role);
            }
            if let Some(model_key) = spec.model_keys.get(index).cloned() {
                context = context.with_model_key(model_key);
            }
            bindings.extend(crate::vanilla_gui::bind_profile_tree_with_path_and_context(
                &CountryLogisticsProfile,
                template.node,
                data,
                path,
                Some(&context),
            ));
        }
    }
    bindings
}

fn logistics_template_for_kind(kind: LogisticsEntryKind) -> &'static str {
    match kind {
        LogisticsEntryKind::Naval => "logistics_overview_naval_equipment_entry",
        LogisticsEntryKind::Air => "logistics_overview_air_equipment_entry",
        LogisticsEntryKind::Land | LogisticsEntryKind::Other => {
            "logistics_overview_land_equipment_entry"
        }
    }
}

fn entry_model_key(entry: &LogisticsEntry) -> String {
    if entry.id.is_empty() {
        entry.name.clone()
    } else {
        entry.id.clone()
    }
}

pub struct LogisticsPanel;

impl LogisticsPanel {
    /// 返回 (close_requested, panel_commands)。
    pub fn show(ctx: &egui::Context, data: &LogisticsData) -> (bool, Vec<PanelCommand>) {
        let Some(context) = crate::vanilla_gui::country_logistics_runtime_context() else {
            return vanilla_logistics_runtime_unavailable_panel(ctx);
        };
        let mut icon_bank = IconBank::new(ctx.clone(), context.path_cfg.clone());
        Self::show_with_icon_bank(ctx, data, &mut icon_bank)
    }

    /// 返回 (close_requested, panel_commands)。
    pub fn show_with_icon_bank(
        ctx: &egui::Context,
        data: &LogisticsData,
        icon_bank: &mut IconBank,
    ) -> (bool, Vec<PanelCommand>) {
        vanilla_show_logistics(ctx, data, icon_bank)
    }
}

fn vanilla_show_logistics(
    ctx: &egui::Context,
    data: &LogisticsData,
    icon_bank: &mut IconBank,
) -> (bool, Vec<PanelCommand>) {
    let profile = CountryLogisticsProfile;
    icon_bank.add_profile_search_dirs(profile.profile_id());

    let Some(context) = crate::vanilla_gui::country_logistics_runtime_context() else {
        return vanilla_logistics_runtime_unavailable_panel(ctx);
    };
    let Some(root) = context.root_template(
        crate::vanilla_gui::COUNTRY_LOGISTICS_GUI_FILE,
        profile.root_template(),
    ) else {
        return vanilla_logistics_runtime_unavailable_panel(ctx);
    };

    let screen = ctx.screen_rect();
    let viewport = crate::vanilla_gui::GuiRect::new(
        screen.left(),
        screen.top(),
        screen.width(),
        screen.height(),
    );
    let spec = crate::vanilla_gui::AnimationSpec::from_node(root);
    let update = crate::vanilla_gui::update_panel_animation(ctx, profile.profile_id(), spec);
    if update.close_finished {
        crate::vanilla_gui::clear_panel_close_request(ctx, profile.profile_id());
        clear_logistics_scroll_offset(ctx);
        return (true, Vec::new());
    }
    if !update.visible {
        return (false, Vec::new());
    }

    let registry = crate::vanilla_gui::GuiTemplateRegistry::from_documents(context.documents());
    let mut runtime_state = crate::vanilla_gui::GuiRuntimeState::shown(viewport)
        .with_pixels_per_point(ctx.pixels_per_point());
    runtime_state.root_position =
        crate::vanilla_gui::GuiRuntimeRootPosition::Current(update.position);
    runtime_state.phase = Some(update.phase);
    runtime_state.visible = update.visible;
    runtime_state = apply_logistics_scroll_input(ctx, root, data, viewport, runtime_state);

    let parts = logistics_vanilla_runtime_frame_parts(
        root,
        data,
        viewport,
        runtime_state,
        registry.clone(),
        Some(&context.gfx_index),
        Some(&mut *icon_bank),
    );

    let mut close_requested = ctx.input(|input| input.key_pressed(egui::Key::Escape));
    let mut stats = crate::vanilla_gui::RenderStats::default();
    egui::Area::new(egui::Id::new("countrylogisticsview_vanilla_runtime"))
        .order(egui::Order::Foreground)
        .fixed_pos(screen.min)
        .show(ctx, |ui| {
            let outer: Rect = parts.frame.root_layout.rect.into();
            let visible_outer = outer.intersect(screen);
            let _ = ui.allocate_rect(visible_outer, Sense::hover());
            ui.interact(
                visible_outer,
                ui.id().with("countrylogisticsview_drag_region"),
                Sense::click_and_drag(),
            );
            crate::v9::paint::paint_shadow(ui.painter(), outer, crate::v9::Elevation::E2, 1.0);
            let renderer = crate::vanilla_gui::VanillaGuiRenderer::new(&context.gfx_index);
            stats.merge(renderer.paint_tree(
                ui,
                root,
                &parts.frame.root_layout,
                &parts.bindings,
                icon_bank,
            ));
            for instance in &parts.frame.generated_instances {
                let Some(template) = registry.get(instance.template_name) else {
                    continue;
                };
                stats.merge(renderer.paint_tree(
                    ui,
                    template.node,
                    &instance.layout,
                    &parts.bindings,
                    icon_bank,
                ));
            }
        });

    if logistics_close_requested_from_render_stats(&stats) {
        close_requested = true;
    }
    let commands = logistics_commands_from_render_stats(&profile, data, &stats);
    log_logistics_render_stats(ctx, &stats, icon_bank);

    if close_requested {
        crate::vanilla_gui::request_panel_close(ctx, profile.profile_id());
    }

    (false, commands)
}

fn apply_logistics_scroll_input(
    ctx: &egui::Context,
    root: &crate::vanilla_gui::GuiNode,
    data: &LogisticsData,
    viewport: crate::vanilla_gui::GuiRect,
    mut runtime_state: crate::vanilla_gui::GuiRuntimeState,
) -> crate::vanilla_gui::GuiRuntimeState {
    let bindings = crate::vanilla_gui::bind_profile_tree(&CountryLogisticsProfile, root, data);
    let probe = crate::vanilla_gui::GuiRuntimeFrame::build(
        crate::vanilla_gui::GuiRuntimeFrameInput::new(
            root,
            viewport,
            crate::vanilla_gui::COUNTRY_LOGISTICS_PROFILE_ID,
            &bindings,
        )
        .with_runtime_state(runtime_state.clone()),
    );
    let Some(scroll_state) = probe
        .scroll_states
        .iter()
        .find(|state| state.node_name.as_deref() == Some("materiel"))
    else {
        return runtime_state;
    };

    let max_scroll = (data.entries.len() as f32 * LOGISTICS_MATERIEL_ROW_HEIGHT
        - scroll_state.content_clip_rect.height)
        .max(0.0);
    let scroll_id = logistics_scroll_offset_id();
    let mut offset = ctx
        .data_mut(|data| data.get_persisted::<f32>(scroll_id))
        .unwrap_or(0.0)
        .clamp(0.0, max_scroll);
    let pointer = ctx.input(|input| input.pointer.latest_pos());
    let clip: Rect = scroll_state.content_clip_rect.into();
    let wheel = ctx.input(|input| {
        if input.smooth_scroll_delta.y.abs() > f32::EPSILON {
            input.smooth_scroll_delta.y
        } else {
            input.raw_scroll_delta.y
        }
    });
    if pointer.is_some_and(|pos| clip.contains(pos)) && wheel.abs() > f32::EPSILON {
        let delta = if scroll_state.spec.smooth_scrolling {
            -wheel
        } else {
            -wheel.signum() * scroll_state.spec.scroll_wheel_factor
        };
        let next = (offset + delta).clamp(0.0, max_scroll);
        if (next - offset).abs() > f32::EPSILON {
            offset = next;
            ctx.data_mut(|data| data.insert_persisted(scroll_id, offset));
            ctx.input_mut(|input| {
                input.smooth_scroll_delta = Vec2::ZERO;
                input.raw_scroll_delta = Vec2::ZERO;
            });
            ctx.request_repaint();
        }
    } else {
        ctx.data_mut(|data| data.insert_persisted(scroll_id, offset));
    }
    runtime_state = runtime_state.with_scroll_offset(
        scroll_state.path.clone(),
        crate::vanilla_gui::GuiPoint { x: 0.0, y: offset },
    );
    runtime_state
}

fn logistics_scroll_offset_id() -> egui::Id {
    egui::Id::new("countrylogisticsview_materiel_scroll_offset")
}

fn clear_logistics_scroll_offset(ctx: &egui::Context) {
    ctx.data_mut(|data| data.insert_persisted(logistics_scroll_offset_id(), 0.0));
}

fn logistics_close_requested_from_render_stats(stats: &crate::vanilla_gui::RenderStats) -> bool {
    stats
        .clicked_commands
        .iter()
        .any(|command| command == "close")
}

fn logistics_commands_from_render_stats(
    profile: &CountryLogisticsProfile,
    data: &LogisticsData,
    stats: &crate::vanilla_gui::RenderStats,
) -> Vec<PanelCommand> {
    stats
        .clicked_commands
        .iter()
        .filter(|command| command.as_str() != "close")
        .filter_map(|command| {
            profile.handle_action(
                crate::vanilla_gui::GuiAction {
                    node_path: crate::vanilla_gui::GuiNodePath::root(command.clone()),
                    kind: crate::vanilla_gui::GuiActionKind::Click,
                },
                data,
            )
        })
        .collect()
}

fn log_logistics_render_stats(
    ctx: &egui::Context,
    stats: &crate::vanilla_gui::RenderStats,
    icon_bank: &IconBank,
) {
    let id = egui::Id::new("countrylogisticsview_render_stats_logged");
    let already_logged = ctx
        .data_mut(|data| data.get_persisted::<bool>(id))
        .unwrap_or(false);
    if already_logged {
        return;
    }
    println!(
        "[ui][logistics] render nodes={}/{} sprites={} fallback={} text={} buttons={} progress={} icon_missing_cache={} fallback_labels={:?}",
        stats.nodes_painted,
        stats.nodes_seen,
        stats.sprites_painted,
        stats.fallback_painted,
        stats.text_painted,
        stats.buttons,
        stats.progress_bars,
        icon_bank.missing_count(),
        stats.fallback_labels
    );
    ctx.data_mut(|data| data.insert_persisted(id, true));
}

fn vanilla_logistics_runtime_unavailable_panel(ctx: &egui::Context) -> (bool, Vec<PanelCommand>) {
    let screen = ctx.screen_rect();
    let panel = Rect::from_min_size(
        Pos2::new(screen.left() + 16.0, screen.top() + 92.0),
        Vec2::new(550.0_f32.min((screen.width() - 32.0).max(260.0)), 210.0),
    );
    let report = crate::vanilla_gui::VanillaGuiRuntimeUnavailableReport::for_profile(
        &crate::vanilla_gui::COUNTRY_LOGISTICS_DESCRIPTOR,
    );
    let text_color = Color32::from_rgb(0xe8, 0xe2, 0xd4);
    let muted_color = Color32::from_rgb(0xa5, 0xa0, 0x94);
    let mut close = ctx.input(|input| input.key_pressed(egui::Key::Escape));
    egui::Area::new(egui::Id::new("countrylogisticsview_runtime_unavailable"))
        .order(egui::Order::Foreground)
        .fixed_pos(panel.min)
        .show(ctx, |ui| {
            let local = Rect::from_min_size(Pos2::ZERO, panel.size());
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
                "Vanilla logistics runtime unavailable",
                crate::v9::TextRole::Heading.font_id(),
                text_color,
            );
            painter.text(
                Pos2::new(local.left() + 16.0, local.top() + 54.0),
                egui::Align2::LEFT_TOP,
                report.reason,
                crate::v9::TextRole::Body.font_id(),
                muted_color,
            );
            painter.text(
                Pos2::new(local.left() + 16.0, local.top() + 88.0),
                egui::Align2::LEFT_TOP,
                format!(
                    "required: {}",
                    crate::vanilla_gui::COUNTRY_LOGISTICS_GUI_FILE
                ),
                crate::v9::TextRole::Caption.font_id(),
                muted_color,
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
    (close, Vec::new())
}

fn signed_one_decimal(value: f32) -> String {
    format!("{:+.1}/日", value)
}
