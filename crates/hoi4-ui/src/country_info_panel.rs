//! Right-click foreign country diplomacy panel.
//!
//! The right-click path now renders the vanilla `countrydiplomacyview` runtime
//! profile. The data and commands remain project-owned; vanilla files only
//! provide layout, sprites, animation, hit regions, and scroll semantics.

#![allow(deprecated)]

use egui::{Color32, Pos2, Rect, Sense, Vec2};

use crate::diplomacy::{
    DiplomacyActionEntry, DiplomacyRelationEntry, DiplomaticActionView, RelationFactorEntry,
    WargoalDetailEntry,
};
use crate::diplomacy_profile::{
    CountryDiplomacyProfile, COUNTRY_DIPLOMACY_CLOSE_COMMAND, COUNTRY_DIPLOMACY_ESCAPE_COMMAND,
};
use crate::i18n::tr;
use crate::icons::IconBank;
use crate::vanilla_gui::VanillaPanelProfile;

const DIPLOMACY_RELATION_ROW_HEIGHT: f32 = 50.0;
const DIPLOMACY_ACTION_ROW_HEIGHT: f32 = 29.0;

pub struct CountryInfoData {
    pub target_tag: String,
    pub tag: String,
    pub display_name: String,
    pub flag_gfx: String,
    pub player_flag_gfx: String,
    pub leader_name: String,
    pub leader_portrait_key: Option<String>,
    pub ruling_party: String,
    pub ruling_party_label: String,
    pub party_full_name: String,
    pub party_popularity: Vec<(String, f32)>,
    pub gdp_gbp: f64,
    pub industrial_level: u32,
    pub military_industrial_level: u32,
    pub construction_points: u32,
    pub division_count: u32,
    pub population: u64,
    pub domestic_population: u64,
    pub colonial_population: u64,
    pub governed_population: u64,
    pub subject_population: u64,
    pub imperial_population: u64,
    pub manpower: u64,
    pub stability: f32,
    pub war_support: f32,
    pub opinion: i16,
    pub our_opinion_of_target: i16,
    pub their_opinion_of_us: i16,
    pub at_war: bool,
    pub same_faction: bool,
    pub faction_name: Option<String>,
    pub overlord_name: Option<String>,
    pub subject_names: Vec<String>,
    pub autonomy_level_name: Option<String>,
    pub has_wargoal: bool,
    pub justifying_wargoal: bool,
    pub justify_progress: f32,
    pub justify_days_remaining: u32,
    pub wargoals: Vec<WargoalDetailEntry>,
    pub relation_factors: Vec<RelationFactorEntry>,
    pub relations: Vec<DiplomacyRelationEntry>,
    pub actions: Vec<DiplomacyActionEntry>,
    pub justify_action: DiplomaticActionView,
    pub declare_war_action: DiplomaticActionView,
    pub invite_to_faction_action: DiplomaticActionView,
    pub request_access_action: DiplomaticActionView,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CountryInfoCommand {
    JustifyWargoal { target_tag: String },
    DeclareWar { target_tag: String },
    InviteToFaction { target_tag: String },
    RequestMilitaryAccess { target_tag: String },
    Close,
}

pub struct CountryInfoPanel {
    pub open: bool,
    pub data: Option<CountryInfoData>,
}

impl Default for CountryInfoPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl CountryInfoPanel {
    pub fn new() -> Self {
        Self {
            open: false,
            data: None,
        }
    }

    pub fn open_with(&mut self, data: CountryInfoData) {
        self.data = Some(data);
        self.open = true;
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    pub fn show(
        &mut self,
        ctx: &egui::Context,
        icon_bank: &mut IconBank,
        _player_in_faction: bool,
    ) -> Vec<CountryInfoCommand> {
        self.show_with_icon_bank(ctx, icon_bank)
    }

    pub fn show_with_icon_bank(
        &mut self,
        ctx: &egui::Context,
        icon_bank: &mut IconBank,
    ) -> Vec<CountryInfoCommand> {
        if !self.open {
            return Vec::new();
        }
        let Some(data) = self.data.as_ref() else {
            self.open = false;
            return Vec::new();
        };

        let (close, commands) = vanilla_show_country_diplomacy(ctx, data, icon_bank);
        if close {
            self.open = false;
        }
        commands
    }
}

fn vanilla_show_country_diplomacy(
    ctx: &egui::Context,
    data: &CountryInfoData,
    icon_bank: &mut IconBank,
) -> (bool, Vec<CountryInfoCommand>) {
    let profile = CountryDiplomacyProfile;
    icon_bank.add_profile_search_dirs(profile.profile_id());

    let Some(context) = crate::vanilla_gui::country_diplomacy_runtime_context() else {
        return vanilla_country_diplomacy_runtime_unavailable_panel(ctx);
    };
    let Some(root) = context.root_template(
        crate::vanilla_gui::COUNTRY_DIPLOMACY_GUI_FILE,
        profile.root_template(),
    ) else {
        return vanilla_country_diplomacy_runtime_unavailable_panel(ctx);
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
        clear_country_diplomacy_scroll_offsets(ctx);
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
    runtime_state = apply_country_diplomacy_scroll_input(ctx, root, data, viewport, runtime_state);

    let parts = crate::diplomacy_profile::country_diplomacy_vanilla_runtime_frame_parts(
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
    egui::Area::new(egui::Id::new("countrydiplomacyview_vanilla_runtime"))
        .order(egui::Order::Foreground)
        .fixed_pos(screen.min)
        .show(ctx, |ui| {
            let outer: Rect = parts.frame.root_layout.rect.into();
            let visible_outer = outer.intersect(screen);
            let _ = ui.allocate_rect(visible_outer, Sense::hover());
            ui.interact(
                visible_outer,
                ui.id().with("countrydiplomacyview_drag_region"),
                Sense::click_and_drag(),
            );
            crate::v9::paint::paint_shadow(ui.painter(), outer, crate::v9::Elevation::E2, 1.0);
            paint_vanilla_diplomacy_panel_backing(ui.painter(), visible_outer);

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

    if country_diplomacy_close_requested_from_render_stats(&stats) {
        close_requested = true;
    }
    let commands = country_diplomacy_commands_from_render_stats(&profile, data, &stats);
    log_country_diplomacy_render_stats(ctx, &stats, icon_bank);

    if close_requested {
        crate::vanilla_gui::request_panel_close(ctx, profile.profile_id());
    }

    (false, commands)
}

fn paint_vanilla_diplomacy_panel_backing(painter: &egui::Painter, rect: Rect) {
    if rect.is_positive() {
        painter.rect_filled(rect, 0.0, Color32::from_rgb(0x39, 0x36, 0x2f));
    }
}

fn apply_country_diplomacy_scroll_input(
    ctx: &egui::Context,
    root: &crate::vanilla_gui::GuiNode,
    data: &CountryInfoData,
    viewport: crate::vanilla_gui::GuiRect,
    mut runtime_state: crate::vanilla_gui::GuiRuntimeState,
) -> crate::vanilla_gui::GuiRuntimeState {
    let bindings = crate::vanilla_gui::bind_profile_tree(&CountryDiplomacyProfile, root, data);
    let probe = crate::vanilla_gui::GuiRuntimeFrame::build(
        crate::vanilla_gui::GuiRuntimeFrameInput::new(
            root,
            viewport,
            crate::vanilla_gui::COUNTRY_DIPLOMACY_PROFILE_ID,
            &bindings,
        )
        .with_runtime_state(runtime_state.clone()),
    );

    let pointer = ctx.input(|input| input.pointer.latest_pos());
    let wheel = ctx.input(|input| {
        if input.smooth_scroll_delta.y.abs() > f32::EPSILON {
            input.smooth_scroll_delta.y
        } else {
            input.raw_scroll_delta.y
        }
    });
    let mut wheel_consumed = false;

    for scroll_state in &probe.scroll_states {
        let Some(node_name) = scroll_state.node_name.as_deref() else {
            continue;
        };
        let Some((row_count, row_height)) = country_diplomacy_scroll_model(data, node_name) else {
            continue;
        };

        let max_scroll =
            (row_count as f32 * row_height - scroll_state.content_clip_rect.height).max(0.0);
        let scroll_id = country_diplomacy_scroll_offset_id(node_name);
        let mut offset = ctx
            .data_mut(|data| data.get_persisted::<f32>(scroll_id))
            .unwrap_or(0.0)
            .clamp(0.0, max_scroll);
        let clip: Rect = scroll_state.content_clip_rect.into();
        let hovered = pointer.is_some_and(|pos| clip.contains(pos));
        if !wheel_consumed && hovered && wheel.abs() > f32::EPSILON {
            let delta = if scroll_state.spec.smooth_scrolling {
                -wheel
            } else {
                -wheel.signum() * scroll_state.spec.scroll_wheel_factor
            };
            offset = (offset + delta).clamp(0.0, max_scroll);
            ctx.request_repaint();
            wheel_consumed = true;
        }
        ctx.data_mut(|data| data.insert_persisted(scroll_id, offset));
        runtime_state = runtime_state.with_scroll_offset(
            scroll_state.path.clone(),
            crate::vanilla_gui::GuiPoint { x: 0.0, y: offset },
        );
    }

    if wheel_consumed {
        ctx.input_mut(|input| {
            input.smooth_scroll_delta = Vec2::ZERO;
            input.raw_scroll_delta = Vec2::ZERO;
        });
    }

    runtime_state
}

fn country_diplomacy_scroll_model(data: &CountryInfoData, node_name: &str) -> Option<(usize, f32)> {
    match node_name {
        "relations_info" => Some((data.relations.len(), DIPLOMACY_RELATION_ROW_HEIGHT)),
        "diplomatic_actions" => Some((data.actions.len(), DIPLOMACY_ACTION_ROW_HEIGHT)),
        _ => None,
    }
}

fn country_diplomacy_scroll_offset_id(node_name: &str) -> egui::Id {
    egui::Id::new(format!("countrydiplomacyview_{node_name}_scroll_offset"))
}

fn clear_country_diplomacy_scroll_offsets(ctx: &egui::Context) {
    for node_name in ["relations_info", "diplomatic_actions"] {
        ctx.data_mut(|data| {
            data.insert_persisted(country_diplomacy_scroll_offset_id(node_name), 0.0)
        });
    }
}

fn country_diplomacy_close_requested_from_render_stats(
    stats: &crate::vanilla_gui::RenderStats,
) -> bool {
    stats
        .clicked_commands
        .iter()
        .any(|command| is_country_diplomacy_close_command(command))
}

fn country_diplomacy_commands_from_render_stats(
    profile: &CountryDiplomacyProfile,
    data: &CountryInfoData,
    stats: &crate::vanilla_gui::RenderStats,
) -> Vec<CountryInfoCommand> {
    stats
        .clicked_commands
        .iter()
        .filter(|command| !is_country_diplomacy_close_command(command))
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

fn is_country_diplomacy_close_command(command: &str) -> bool {
    command == COUNTRY_DIPLOMACY_CLOSE_COMMAND
        || command == COUNTRY_DIPLOMACY_ESCAPE_COMMAND
        || command.eq_ignore_ascii_case("ESCAPE")
        || command.ends_with("close_button")
}

fn log_country_diplomacy_render_stats(
    ctx: &egui::Context,
    stats: &crate::vanilla_gui::RenderStats,
    icon_bank: &IconBank,
) {
    let id = egui::Id::new("countrydiplomacyview_render_stats_logged");
    let already_logged = ctx
        .data_mut(|data| data.get_persisted::<bool>(id))
        .unwrap_or(false);
    if already_logged {
        return;
    }
    println!(
        "[ui][country_diplomacy] render nodes={}/{} sprites={} fallback={} text={} buttons={} progress={} icon_missing_cache={} fallback_labels={:?}",
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

fn vanilla_country_diplomacy_runtime_unavailable_panel(
    ctx: &egui::Context,
) -> (bool, Vec<CountryInfoCommand>) {
    let screen = ctx.screen_rect();
    let panel = Rect::from_min_size(
        Pos2::new(screen.left() + 16.0, screen.top() + 92.0),
        Vec2::new(550.0_f32.min((screen.width() - 32.0).max(260.0)), 232.0),
    );
    let report = crate::vanilla_gui::VanillaGuiRuntimeUnavailableReport::for_profile(
        &crate::vanilla_gui::COUNTRY_DIPLOMACY_DESCRIPTOR,
    );
    let text_color = Color32::from_rgb(0xe8, 0xe2, 0xd4);
    let muted_color = Color32::from_rgb(0xa5, 0xa0, 0x94);
    let mut close = ctx.input(|input| input.key_pressed(egui::Key::Escape));
    egui::Area::new(egui::Id::new("countrydiplomacyview_runtime_unavailable"))
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
                "Vanilla diplomacy runtime unavailable",
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
                Pos2::new(local.left() + 16.0, local.top() + 92.0),
                egui::Align2::LEFT_TOP,
                format!(
                    "required: {}, {}",
                    crate::vanilla_gui::COUNTRY_DIPLOMACY_GUI_FILE,
                    crate::vanilla_gui::COUNTRY_DIPLOMACY_GFX_FILE
                ),
                crate::v9::TextRole::Caption.font_id(),
                muted_color,
            );
            painter.text(
                Pos2::new(local.left() + 16.0, local.top() + 124.0),
                egui::Align2::LEFT_TOP,
                "No legacy V9 fallback is used for this right-click panel.",
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
