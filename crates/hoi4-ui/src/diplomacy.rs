//! Diplomacy panel and shared country diplomacy detail rendering.

#![allow(dead_code, deprecated)]

use crate::{components, i18n::tr, ActiveDetailPanel, CountryDetailTarget, PanelCommand};
use egui::{Color32, Pos2, Rect, RichText, Sense, Vec2};

const GOLD: Color32 = Color32::from_rgb(0x9f, 0xc1, 0xc8);
const PANEL_CARD: Color32 = Color32::from_rgb(0x0d, 0x10, 0x0f);
const PANEL_CARD_SOFT: Color32 = Color32::from_rgb(0x14, 0x18, 0x17);
const STROKE_DARK: Color32 = Color32::from_rgb(0x28, 0x31, 0x31);
const GOOD: Color32 = Color32::from_rgb(0x70, 0xc8, 0x78);
const WARN: Color32 = Color32::from_rgb(0xff, 0xc0, 0x60);
const BAD: Color32 = Color32::from_rgb(0xe0, 0x60, 0x58);

#[derive(Clone)]
pub struct CountryEntry {
    pub target_tag: String,
    pub tag: String,
    pub display_name: String,
    pub flag_gfx: String,
    pub ruling_party: String,
    pub ruling_party_label: String,
    pub party_full_name: String,
    pub party_popularity: Vec<(String, f32)>,
    pub opinion: i16,
    pub our_opinion_of_target: i16,
    pub their_opinion_of_us: i16,
    pub at_war: bool,
    pub same_faction: bool,
    pub relations: Vec<DiplomacyRelationEntry>,
    pub selected: bool,
    pub autonomy_summary: Option<String>,
    pub leader_name: String,
    pub leader_portrait_key: Option<String>,
    pub detail: Option<CountryDiplomacyDetail>,
}

#[derive(Clone, Debug)]
pub struct DiplomaticActionView {
    pub enabled: bool,
    pub reason: Option<String>,
    pub preview: String,
}

impl DiplomaticActionView {
    pub fn enabled(preview: impl Into<String>) -> Self {
        Self {
            enabled: true,
            reason: None,
            preview: preview.into(),
        }
    }

    pub fn disabled(preview: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            enabled: false,
            reason: Some(reason.into()),
            preview: preview.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DiplomacyRelationKind {
    AtWar,
    SameFaction,
    DifferentFaction,
    Subject,
    Overlord,
    Wargoal,
    JustifyingWargoal,
    MilitaryAccess,
    PendingRequest,
}

#[derive(Clone, Debug)]
pub struct DiplomacyRelationEntry {
    pub id: String,
    pub kind: DiplomacyRelationKind,
    pub label: String,
    pub sprite: String,
    pub tooltip: String,
    pub positive: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DiplomacyActionCommand {
    JustifyWargoal { target_tag: String },
    DeclareWar { target_tag: String },
    InviteToFaction { target_tag: String },
    RequestMilitaryAccess { target_tag: String },
    Unavailable { action_id: String },
}

#[derive(Clone, Debug)]
pub struct DiplomacyActionEntry {
    pub id: String,
    pub label: String,
    pub enabled: bool,
    pub preview: String,
    pub reason: Option<String>,
    pub cost_text: Option<String>,
    pub sprite: String,
    pub command: DiplomacyActionCommand,
}

#[derive(Clone, Debug)]
pub struct CountryDiplomacyDetail {
    pub target_tag: String,
    pub tag: String,
    pub display_name: String,
    pub ruling_party: String,
    pub ruling_party_label: String,
    pub party_full_name: String,
    pub party_popularity: Vec<(String, f32)>,
    pub opinion: i16,
    pub our_opinion_of_target: i16,
    pub their_opinion_of_us: i16,
    pub at_war: bool,
    pub same_faction: bool,
    pub faction_name: Option<String>,
    pub overlord_name: Option<String>,
    pub subject_names: Vec<String>,
    pub autonomy_level_name: Option<String>,
    pub domestic_population: u64,
    pub colonial_population: u64,
    pub governed_population: u64,
    pub subject_population: u64,
    pub imperial_population: u64,
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

#[derive(Clone, Debug)]
pub struct WargoalDetailEntry {
    pub kind: String,
    pub target_state: Option<u16>,
    pub status: String,
    pub progress: f32,
    pub days_remaining: u32,
    pub source: String,
}

#[derive(Clone, Debug)]
pub struct RelationFactorEntry {
    pub label: String,
    pub value: String,
    pub positive: bool,
}

#[derive(Clone)]
pub struct FactionEntry {
    pub name: String,
    pub leader_tag: String,
    pub member_tags: Vec<String>,
}

#[derive(Clone)]
pub struct DiplomaticRequestEntry {
    pub from_tag: String,
    pub to_tag: String,
    pub kind: String,
    pub status: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeaceSide {
    Attacker,
    Defender,
}

#[derive(Clone)]
pub struct PeaceWargoalEntry {
    pub claimant_tag: String,
    pub claimant_name: String,
    pub target_tag: String,
    pub target_name: String,
    pub kind: String,
    pub target_state: Option<u16>,
}

#[derive(Clone)]
pub struct PeaceWarEntry {
    pub id: u32,
    pub primary_attacker_tag: String,
    pub primary_defender_tag: String,
    pub attacker_tags: Vec<String>,
    pub defender_tags: Vec<String>,
    pub attacker_score: f32,
    pub defender_score: f32,
    pub player_side: Option<PeaceSide>,
    pub wargoals: Vec<PeaceWargoalEntry>,
    pub attacker_peace_action: DiplomaticActionView,
    pub defender_peace_action: DiplomaticActionView,
}

#[derive(Clone)]
pub struct DiplomacyData {
    pub player_tag: String,
    pub player_flag_gfx: String,
    pub player_faction: Option<FactionEntry>,
    pub all_factions: Vec<FactionEntry>,
    pub countries: Vec<CountryEntry>,
    pub active_wars: Vec<PeaceWarEntry>,
    pub requests: Vec<DiplomaticRequestEntry>,
    pub world_tension: f32,
}

#[derive(Debug)]
pub enum DiplomacyCommand {
    JustifyWargoal(String),
    DeclareWar(String),
    CreateFaction,
    LeaveFaction,
    InviteToFaction(String),
    RequestMilitaryAccess(String),
    ResolvePeace {
        war_id: u32,
        winning_side: PeaceSide,
    },
    Panel(PanelCommand),
}

pub struct DiplomacyPanel;

impl DiplomacyPanel {
    pub fn show(
        ctx: &egui::Context,
        data: &DiplomacyData,
        sort_by_opinion: &mut bool,
        selected_tag: &mut Option<String>,
        icon_bank: &mut crate::icons::IconBank,
    ) -> (bool, Vec<DiplomacyCommand>) {
        Self::show_with_icon_bank(ctx, data, sort_by_opinion, selected_tag, icon_bank)
    }

    pub fn show_with_icon_bank(
        ctx: &egui::Context,
        data: &DiplomacyData,
        sort_by_opinion: &mut bool,
        selected_tag: &mut Option<String>,
        icon_bank: &mut crate::icons::IconBank,
    ) -> (bool, Vec<DiplomacyCommand>) {
        vanilla_show_diplomacy_panel(ctx, data, sort_by_opinion, selected_tag, icon_bank)
    }
}

const DIPLOMACY_PANEL_COUNTRY_ROW_HEIGHT: f32 = 45.0;
const DIPLOMACY_RELATION_ROW_HEIGHT: f32 = 50.0;
const DIPLOMACY_ACTION_ROW_HEIGHT: f32 = 29.0;

fn vanilla_show_diplomacy_panel(
    ctx: &egui::Context,
    data: &DiplomacyData,
    sort_by_opinion: &mut bool,
    selected_tag: &mut Option<String>,
    icon_bank: &mut crate::icons::IconBank,
) -> (bool, Vec<DiplomacyCommand>) {
    use crate::vanilla_gui::VanillaPanelProfile;

    let profile = crate::diplomacy_profile::DiplomacyPanelProfile;
    icon_bank.add_profile_search_dirs(profile.profile_id());

    let Some(context) = crate::vanilla_gui::country_diplomacy_runtime_context() else {
        return vanilla_diplomacy_panel_runtime_unavailable_panel(ctx);
    };
    let Some(root) = context.root_template(
        crate::vanilla_gui::COUNTRY_DIPLOMACY_GUI_FILE,
        profile.root_template(),
    ) else {
        return vanilla_diplomacy_panel_runtime_unavailable_panel(ctx);
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
        clear_diplomacy_panel_scroll_offsets(ctx);
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
    runtime_state = apply_diplomacy_panel_scroll_input(ctx, root, data, viewport, runtime_state);

    let parts = crate::diplomacy_profile::diplomacy_panel_vanilla_runtime_frame_parts(
        root,
        data,
        viewport,
        runtime_state,
        registry.clone(),
        Some(&context.gfx_index),
        Some(&mut *icon_bank),
        *sort_by_opinion,
    );

    let mut close_requested = ctx.input(|input| input.key_pressed(egui::Key::Escape));
    let mut stats = crate::vanilla_gui::RenderStats::default();
    egui::Area::new(egui::Id::new("diplomacy_panel_vanilla_runtime"))
        .order(egui::Order::Foreground)
        .fixed_pos(screen.min)
        .show(ctx, |ui| {
            let outer: Rect = parts.frame.root_layout.rect.into();
            let visible_outer = outer.intersect(screen);
            let _ = ui.allocate_rect(visible_outer, Sense::hover());
            ui.interact(
                visible_outer,
                ui.id().with("diplomacy_panel_drag_region"),
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

    if diplomacy_panel_close_requested_from_render_stats(&stats) {
        close_requested = true;
    }
    let commands = diplomacy_panel_commands_from_render_stats(
        &profile,
        data,
        &stats,
        sort_by_opinion,
        selected_tag,
    );
    log_diplomacy_panel_render_stats(ctx, &stats, icon_bank);

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

fn apply_diplomacy_panel_scroll_input(
    ctx: &egui::Context,
    root: &crate::vanilla_gui::GuiNode,
    data: &DiplomacyData,
    viewport: crate::vanilla_gui::GuiRect,
    mut runtime_state: crate::vanilla_gui::GuiRuntimeState,
) -> crate::vanilla_gui::GuiRuntimeState {
    let profile = crate::diplomacy_profile::DiplomacyPanelProfile;
    let bindings = crate::vanilla_gui::bind_profile_tree(&profile, root, data);
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
        let Some((row_count, row_height)) = diplomacy_panel_scroll_model(data, node_name) else {
            continue;
        };

        let max_scroll =
            (row_count as f32 * row_height - scroll_state.content_clip_rect.height).max(0.0);
        let scroll_id = diplomacy_panel_scroll_offset_id(node_name);
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

fn diplomacy_panel_scroll_model(data: &DiplomacyData, node_name: &str) -> Option<(usize, f32)> {
    let selected_detail = data
        .countries
        .iter()
        .find(|country| country.selected)
        .and_then(|country| country.detail.as_ref());
    match node_name {
        "countries" => Some((data.countries.len(), DIPLOMACY_PANEL_COUNTRY_ROW_HEIGHT)),
        "relations_info" => Some((
            selected_detail
                .map(|detail| detail.relations.len())
                .unwrap_or(0),
            DIPLOMACY_RELATION_ROW_HEIGHT,
        )),
        "diplomatic_actions" => Some((
            selected_detail
                .map(|detail| detail.actions.len())
                .unwrap_or(0),
            DIPLOMACY_ACTION_ROW_HEIGHT,
        )),
        _ => None,
    }
}

fn diplomacy_panel_scroll_offset_id(node_name: &str) -> egui::Id {
    egui::Id::new(format!("diplomacy_panel_{node_name}_scroll_offset"))
}

fn clear_diplomacy_panel_scroll_offsets(ctx: &egui::Context) {
    for node_name in ["countries", "relations_info", "diplomatic_actions"] {
        ctx.data_mut(|data| {
            data.insert_persisted(diplomacy_panel_scroll_offset_id(node_name), 0.0)
        });
    }
}

fn diplomacy_panel_close_requested_from_render_stats(
    stats: &crate::vanilla_gui::RenderStats,
) -> bool {
    stats
        .clicked_commands
        .iter()
        .any(|command| is_diplomacy_panel_close_command(command))
}

fn diplomacy_panel_commands_from_render_stats(
    profile: &crate::diplomacy_profile::DiplomacyPanelProfile,
    data: &DiplomacyData,
    stats: &crate::vanilla_gui::RenderStats,
    sort_by_opinion: &mut bool,
    selected_tag: &mut Option<String>,
) -> Vec<DiplomacyCommand> {
    use crate::vanilla_gui::VanillaPanelProfile;

    let mut commands = Vec::new();
    for command in &stats.clicked_commands {
        if is_diplomacy_panel_close_command(command) {
            continue;
        }
        match command.as_str() {
            crate::diplomacy_profile::DIPLOMACY_PANEL_SORT_NAME_COMMAND => {
                *sort_by_opinion = false;
                continue;
            }
            crate::diplomacy_profile::DIPLOMACY_PANEL_SORT_OPINION_COMMAND => {
                *sort_by_opinion = true;
                continue;
            }
            crate::diplomacy_profile::DIPLOMACY_PANEL_BACK_COMMAND => {
                *selected_tag = None;
                commands.push(DiplomacyCommand::Panel(PanelCommand::CloseDetail));
                continue;
            }
            _ => {}
        }
        if let Some(tag) = command
            .as_str()
            .strip_prefix(crate::diplomacy_profile::DIPLOMACY_PANEL_SELECT_COUNTRY_PREFIX)
        {
            let tag = tag.to_owned();
            *selected_tag = Some(tag.clone());
            commands.push(DiplomacyCommand::Panel(PanelCommand::OpenDetail(
                ActiveDetailPanel::Country(CountryDetailTarget { tag }),
            )));
            continue;
        }
        if let Some(command) = profile.handle_action(
            crate::vanilla_gui::GuiAction {
                node_path: crate::vanilla_gui::GuiNodePath::root(command.clone()),
                kind: crate::vanilla_gui::GuiActionKind::Click,
            },
            data,
        ) {
            commands.push(command);
        }
    }
    commands
}

fn is_diplomacy_panel_close_command(command: &str) -> bool {
    command == crate::diplomacy_profile::COUNTRY_DIPLOMACY_CLOSE_COMMAND
        || command == crate::diplomacy_profile::COUNTRY_DIPLOMACY_ESCAPE_COMMAND
        || command.eq_ignore_ascii_case("ESCAPE")
        || command.ends_with("close_button")
}

fn log_diplomacy_panel_render_stats(
    ctx: &egui::Context,
    stats: &crate::vanilla_gui::RenderStats,
    icon_bank: &crate::icons::IconBank,
) {
    let id = egui::Id::new("diplomacy_panel_vanilla_render_stats_logged");
    let already_logged = ctx
        .data_mut(|data| data.get_persisted::<bool>(id))
        .unwrap_or(false);
    if already_logged {
        return;
    }
    println!(
        "[ui][diplomacy_panel] render nodes={}/{} sprites={} fallback={} text={} buttons={} progress={} icon_missing_cache={} fallback_labels={:?}",
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

fn vanilla_diplomacy_panel_runtime_unavailable_panel(
    ctx: &egui::Context,
) -> (bool, Vec<DiplomacyCommand>) {
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
    egui::Area::new(egui::Id::new("diplomacy_panel_runtime_unavailable"))
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
                "No legacy V9 fallback is used for the main diplomacy panel.",
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

fn legacy_show_diplomacy_unused(
    ctx: &egui::Context,
    data: &DiplomacyData,
    sort_by_opinion: &mut bool,
    selected_tag: &mut Option<String>,
    icon_bank: &mut crate::icons::IconBank,
) -> (bool, Vec<DiplomacyCommand>) {
    use crate::v9::composites::panel_shell::{
        draw_summary_tiles, draw_tab_strip, PanelClass, PanelShell,
    };
    use crate::v9::tokens::palette;

    let faction = data
        .player_faction
        .as_ref()
        .map(|f| f.name.clone())
        .unwrap_or_else(|| tr("none").to_owned());
    let (close, output) = PanelShell::new("diplomacy_panel_v9", tr("diplomacy"))
        .subtitle(&data.player_tag)
        .class(PanelClass::MilitaryDiplomacy)
        .accent(palette::INFO)
        .footer("Q Close  |  Diplomacy")
        .show(ctx, |ui, layout| {
            draw_summary_tiles(
                ui,
                layout.summary,
                &[
                    (
                        tr("world_tension"),
                        format!("{:.0}%", data.world_tension),
                        palette::WARN,
                    ),
                    (
                        "战争",
                        data.active_wars.len().to_string(),
                        if data.active_wars.is_empty() {
                            palette::GOOD
                        } else {
                            palette::BAD
                        },
                    ),
                    (tr("faction"), faction, palette::BRASS_BRIGHT),
                    (
                        tr("countries"),
                        data.countries.len().to_string(),
                        palette::PARCHMENT,
                    ),
                ],
            );
            draw_tab_strip(ui, layout.tabs, "国家详情 / 国旗 / 阵营标识", palette::INFO);
            let mut cmds = Vec::new();
            legacy_diplomacy_body_unused(
                ui,
                layout.body,
                data,
                sort_by_opinion,
                selected_tag,
                icon_bank,
                &mut cmds,
            );
            cmds
        });
    (close, output.unwrap_or_default())
}

fn legacy_diplomacy_body_unused(
    ui: &mut egui::Ui,
    rect: Rect,
    data: &DiplomacyData,
    sort_by_opinion: &mut bool,
    selected_tag: &mut Option<String>,
    icon_bank: &mut crate::icons::IconBank,
    cmds: &mut Vec<DiplomacyCommand>,
) {
    use crate::v9::layout::{GridLayout, Track};
    use crate::v9::tokens::spacing;
    ui.allocate_ui_at_rect(rect, |ui| {
        let grid = GridLayout::new(vec![Track::Fr(1.0)], vec![Track::Fr(0.38), Track::Fr(0.62)])
            .with_gutter(spacing::S5, 0.0);
        let cells = grid.measure(rect);
        legacy_diplomacy_country_list_unused(
            ui,
            GridLayout::cell(&cells, 0, 0),
            data,
            sort_by_opinion,
            selected_tag,
            icon_bank,
            cmds,
        );
        legacy_diplomacy_detail_unused(
            ui,
            GridLayout::cell(&cells, 0, 1),
            data,
            selected_tag,
            icon_bank,
            cmds,
        );
    });
}

fn legacy_diplomacy_country_list_unused(
    ui: &mut egui::Ui,
    rect: Rect,
    data: &DiplomacyData,
    sort_by_opinion: &mut bool,
    selected_tag: &mut Option<String>,
    icon_bank: &mut crate::icons::IconBank,
    cmds: &mut Vec<DiplomacyCommand>,
) {
    use crate::v9::{
        primitives::{Button, ButtonSize, ButtonVariant, Card},
        tokens::{palette, spacing, TextRole},
    };
    let inner = Card::new().as_panel().show_at(ui, rect);
    ui.painter().text(
        Pos2::new(inner.left(), inner.top()),
        egui::Align2::LEFT_TOP,
        "国家",
        TextRole::Heading.font_id(),
        palette::BRASS_BRIGHT,
    );
    let alpha_rect = Rect::from_min_size(
        Pos2::new(inner.right() - 168.0, inner.top()),
        Vec2::new(76.0, 24.0),
    );
    if Button::new("名称")
        .size(ButtonSize::Sm)
        .variant(if *sort_by_opinion {
            ButtonVariant::Ghost
        } else {
            ButtonVariant::Secondary
        })
        .show_at(ui, alpha_rect)
        .clicked()
    {
        *sort_by_opinion = false;
    }
    let opinion_rect = Rect::from_min_size(
        Pos2::new(inner.right() - 84.0, inner.top()),
        Vec2::new(80.0, 24.0),
    );
    if Button::new("关系")
        .size(ButtonSize::Sm)
        .variant(if *sort_by_opinion {
            ButtonVariant::Secondary
        } else {
            ButtonVariant::Ghost
        })
        .show_at(ui, opinion_rect)
        .clicked()
    {
        *sort_by_opinion = true;
    }

    let list_rect = Rect::from_min_max(
        Pos2::new(inner.left(), inner.top() + 34.0),
        inner.right_bottom(),
    );
    ui.allocate_ui_at_rect(list_rect, |ui| {
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let mut sorted: Vec<&CountryEntry> = data.countries.iter().collect();
                if *sort_by_opinion {
                    sorted.sort_by(|a, b| b.opinion.cmp(&a.opinion));
                } else {
                    sorted.sort_by(|a, b| a.tag.cmp(&b.tag));
                }
                for country in sorted {
                    let (row_rect, response) = ui
                        .allocate_exact_size(Vec2::new(ui.available_width(), 58.0), Sense::click());
                    let selected = selected_tag.as_deref() == Some(country.tag.as_str());
                    v9_country_row(ui, row_rect, country, selected, icon_bank);
                    if response.clicked() {
                        *selected_tag = Some(country.tag.clone());
                        cmds.push(DiplomacyCommand::Panel(PanelCommand::OpenDetail(
                            ActiveDetailPanel::Country(CountryDetailTarget {
                                tag: country.tag.clone(),
                            }),
                        )));
                    }
                    ui.add_space(spacing::S2);
                }
            });
    });
}

fn v9_country_row(
    ui: &mut egui::Ui,
    rect: Rect,
    country: &CountryEntry,
    selected: bool,
    icon_bank: &mut crate::icons::IconBank,
) {
    use crate::v9::{
        paint,
        primitives::FlagFrame,
        tokens::{palette, TextRole},
    };
    let fill = if selected {
        palette::OIL_BLACK
    } else {
        palette::SOOT_BLACK
    };
    paint::paint_bevel(
        ui.painter(),
        rect,
        fill,
        if selected {
            palette::GOLD
        } else {
            palette::EDGE_DARK
        },
        1.0,
    );
    let flag_rect = Rect::from_min_size(
        Pos2::new(rect.left() + 6.0, rect.top() + 9.0),
        Vec2::new(62.0, 40.0),
    );
    let flag = icon_bank.get_or_load(&country.flag_gfx);
    FlagFrame::new(&country.tag).show_at(ui, flag_rect, flag);
    let name_color = country_color(country);
    ui.painter().text(
        Pos2::new(flag_rect.right() + 10.0, rect.top() + 9.0),
        egui::Align2::LEFT_TOP,
        if country.display_name.is_empty() {
            tr(&country.tag)
        } else {
            &country.display_name
        },
        TextRole::Subheading.font_id(),
        name_color,
    );
    ui.painter().text(
        Pos2::new(flag_rect.right() + 10.0, rect.top() + 31.0),
        egui::Align2::LEFT_TOP,
        if country.leader_name.is_empty() {
            "-"
        } else {
            &country.leader_name
        },
        TextRole::Caption.font_id(),
        palette::PARCHMENT_DIM,
    );
    ui.painter().text(
        Pos2::new(rect.right() - 10.0, rect.center().y),
        egui::Align2::RIGHT_CENTER,
        format!("{:+}", country.opinion),
        TextRole::Numeric.font_id(),
        if country.opinion >= 0 {
            palette::GOOD
        } else {
            palette::BAD
        },
    );
}

fn legacy_diplomacy_detail_unused(
    ui: &mut egui::Ui,
    rect: Rect,
    data: &DiplomacyData,
    selected_tag: &mut Option<String>,
    icon_bank: &mut crate::icons::IconBank,
    cmds: &mut Vec<DiplomacyCommand>,
) {
    use crate::v9::{
        primitives::{draw_progress_bar, ButtonVariant, Card, FlagFrame},
        tokens::{palette, spacing, TextRole},
    };
    let inner = Card::new().as_panel().show_at(ui, rect);
    if selected_tag.is_none() {
        *selected_tag = data.countries.first().map(|c| c.tag.clone());
    }
    let selected_country = data
        .countries
        .iter()
        .find(|c| selected_tag.as_deref() == Some(c.tag.as_str()));
    let Some(country) = selected_country else {
        crate::v9::composites::panel_shell::draw_empty_state(
            ui,
            inner,
            "未选择国家",
            "请从列表中选择一个国家。",
        );
        return;
    };
    let Some(detail) = country.detail.as_ref() else {
        crate::v9::composites::panel_shell::draw_empty_state(
            ui,
            inner,
            &country.tag,
            "暂无外交详情。",
        );
        return;
    };

    let flag_rect = Rect::from_min_size(inner.left_top(), Vec2::new(96.0, 62.0));
    let flag = icon_bank.get_or_load(&country.flag_gfx);
    FlagFrame::new(&country.tag)
        .accent(relation_color(detail))
        .show_at(ui, flag_rect, flag);
    ui.painter().text(
        Pos2::new(flag_rect.right() + spacing::S5, inner.top() + 2.0),
        egui::Align2::LEFT_TOP,
        &detail.display_name,
        TextRole::Display.font_id(),
        palette::GOLD_HOT,
    );
    v9_diplomacy_badge(
        ui,
        Rect::from_min_size(
            Pos2::new(flag_rect.right() + spacing::S5, inner.top() + 34.0),
            Vec2::new(140.0, 24.0),
        ),
        if detail.at_war { "交战" } else { "和平" },
        if detail.at_war {
            palette::BAD
        } else {
            palette::GOOD
        },
    );
    v9_diplomacy_badge(
        ui,
        Rect::from_min_size(
            Pos2::new(flag_rect.right() + spacing::S5 + 150.0, inner.top() + 34.0),
            Vec2::new(180.0, 24.0),
        ),
        detail.faction_name.as_deref().unwrap_or("无阵营"),
        if detail.same_faction {
            palette::GOOD
        } else {
            palette::BRASS_BRIGHT
        },
    );

    let detail_button_rect = Rect::from_min_size(
        Pos2::new(inner.right() - 118.0, inner.top() + 2.0),
        Vec2::new(112.0, 28.0),
    );
    if crate::v9::primitives::Button::new("打开详情")
        .size(crate::v9::primitives::ButtonSize::Sm)
        .variant(crate::v9::primitives::ButtonVariant::Primary)
        .show_at(ui, detail_button_rect)
        .clicked()
    {
        cmds.push(DiplomacyCommand::Panel(PanelCommand::OpenDetail(
            ActiveDetailPanel::Country(CountryDetailTarget {
                tag: detail.tag.clone(),
            }),
        )));
    }

    let metric_y = inner.top() + 82.0;
    v9_detail_metric(
        ui,
        Rect::from_min_size(Pos2::new(inner.left(), metric_y), Vec2::new(130.0, 48.0)),
        tr("opinion"),
        &format!("{:+}", detail.opinion),
        relation_color(detail),
    );
    v9_detail_metric(
        ui,
        Rect::from_min_size(
            Pos2::new(inner.left() + 140.0, metric_y),
            Vec2::new(150.0, 48.0),
        ),
        "本土人口",
        &format_population(detail.domestic_population),
        palette::GOOD,
    );
    v9_detail_metric(
        ui,
        Rect::from_min_size(
            Pos2::new(inner.left() + 300.0, metric_y),
            Vec2::new(150.0, 48.0),
        ),
        "统治人口",
        &format_population(detail.governed_population),
        palette::GOLD,
    );

    let mut y = metric_y + 68.0;
    if let Some(overlord) = &detail.overlord_name {
        v9_text_line(ui, inner.left(), y, "宗主国", overlord, palette::WARN);
        y += 22.0;
    }
    if !detail.subject_names.is_empty() {
        v9_text_line(
            ui,
            inner.left(),
            y,
            "附属国",
            &detail.subject_names.join(", "),
            palette::INFO,
        );
        y += 22.0;
    }
    if detail.justifying_wargoal {
        ui.painter().text(
            Pos2::new(inner.left(), y),
            egui::Align2::LEFT_TOP,
            tr("justifying_wargoal"),
            TextRole::Subheading.font_id(),
            palette::WARN,
        );
        let bar = Rect::from_min_size(
            Pos2::new(inner.left(), y + 24.0),
            Vec2::new(inner.width() * 0.66, 10.0),
        );
        draw_progress_bar(ui, bar, detail.justify_progress, palette::WARN);
    } else if detail.has_wargoal {
        v9_text_line(
            ui,
            inner.left(),
            y,
            "战争目标",
            tr("wargoal_ready"),
            palette::GOOD,
        );
    } else {
        v9_text_line(
            ui,
            inner.left(),
            y,
            "外交动作",
            "可从下方按钮执行",
            palette::INFO,
        );
    }

    let action_y = inner.bottom() - 40.0;
    let mut x = inner.left();
    if !detail.has_wargoal && !detail.justifying_wargoal {
        if v9_action_button(
            ui,
            Rect::from_min_size(Pos2::new(x, action_y), Vec2::new(118.0, 32.0)),
            tr("justify_wargoal"),
            &detail.justify_action,
            ButtonVariant::Secondary,
        ) {
            cmds.push(DiplomacyCommand::JustifyWargoal(detail.tag.clone()));
        }
        x += 126.0;
    }
    if v9_action_button(
        ui,
        Rect::from_min_size(Pos2::new(x, action_y), Vec2::new(106.0, 32.0)),
        tr("declare_war"),
        &detail.declare_war_action,
        ButtonVariant::Danger,
    ) {
        cmds.push(DiplomacyCommand::DeclareWar(detail.tag.clone()));
    }
    x += 114.0;
    if v9_action_button(
        ui,
        Rect::from_min_size(Pos2::new(x, action_y), Vec2::new(124.0, 32.0)),
        tr("invite_to_faction"),
        &detail.invite_to_faction_action,
        ButtonVariant::Secondary,
    ) {
        cmds.push(DiplomacyCommand::InviteToFaction(detail.tag.clone()));
    }
    x += 132.0;
    if v9_action_button(
        ui,
        Rect::from_min_size(Pos2::new(x, action_y), Vec2::new(116.0, 32.0)),
        tr("request_access"),
        &detail.request_access_action,
        ButtonVariant::Secondary,
    ) {
        cmds.push(DiplomacyCommand::RequestMilitaryAccess(detail.tag.clone()));
    }
}

fn v9_action_button(
    ui: &mut egui::Ui,
    rect: Rect,
    label: &str,
    action: &DiplomaticActionView,
    variant: crate::v9::primitives::ButtonVariant,
) -> bool {
    let response = crate::v9::primitives::Button::new(label)
        .size(crate::v9::primitives::ButtonSize::Md)
        .variant(variant)
        .enabled(action.enabled)
        .show_at(ui, rect);
    let response = response.on_hover_text(if action.enabled {
        action.preview.clone()
    } else {
        action
            .reason
            .clone()
            .unwrap_or_else(|| action.preview.clone())
    });
    response.clicked()
}

fn v9_diplomacy_badge(ui: &mut egui::Ui, rect: Rect, label: &str, color: Color32) {
    crate::v9::paint::paint_bevel(
        ui.painter(),
        rect,
        crate::v9::palette::SOOT_BLACK,
        color,
        1.0,
    );
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        crate::v9::TextRole::Caption.font_id(),
        color,
    );
}

fn v9_detail_metric(ui: &mut egui::Ui, rect: Rect, label: &str, value: &str, color: Color32) {
    crate::v9::primitives::Tile::new(label, value)
        .accent(color)
        .show_at(ui, rect);
}

fn v9_text_line(ui: &mut egui::Ui, x: f32, y: f32, label: &str, value: &str, color: Color32) {
    ui.painter().text(
        Pos2::new(x, y),
        egui::Align2::LEFT_TOP,
        label,
        crate::v9::TextRole::Caption.font_id(),
        crate::v9::palette::MUTED,
    );
    ui.painter().text(
        Pos2::new(x + 120.0, y),
        egui::Align2::LEFT_TOP,
        value,
        crate::v9::TextRole::Body.font_id(),
        color,
    );
}

fn render_home_card(ui: &mut egui::Ui, data: &DiplomacyData, cmds: &mut Vec<DiplomacyCommand>) {
    diplomacy_card(ui, "我国外交", |ui| {
        if let Some(f) = &data.player_faction {
            ui.label(RichText::new(&f.name).strong().color(GOLD));
            ui.label(tr("leader_label").replace("{}", &tr(&f.leader_tag)));
            let members: Vec<String> = f.member_tags.iter().map(|t| tr(t).to_string()).collect();
            ui.label(tr("faction_members_list").replace("{}", &members.join(", ")));
            if ui.small_button(tr("leave_faction")).clicked() {
                cmds.push(DiplomacyCommand::LeaveFaction);
            }
        } else {
            ui.label(RichText::new(tr("not_in_faction")).color(components::MUTED));
            if ui.small_button(tr("create_faction")).clicked() {
                cmds.push(DiplomacyCommand::CreateFaction);
            }
        }
    });
}

fn render_requests(ui: &mut egui::Ui, data: &DiplomacyData) {
    diplomacy_card(ui, "待处理外交请求", |ui| {
        if data.requests.is_empty() {
            ui.label(RichText::new("当前没有外交请求。").color(components::MUTED));
            return;
        }
        for request in &data.requests {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(&request.kind).strong().color(GOLD));
                ui.label(format!(
                    "{} -> {}",
                    tr(&request.from_tag),
                    tr(&request.to_tag)
                ));
                ui.label(
                    RichText::new(&request.status)
                        .small()
                        .color(components::MUTED),
                );
            });
        }
    });
}

fn render_country_list(
    ui: &mut egui::Ui,
    data: &DiplomacyData,
    sort_by_opinion: &mut bool,
    selected_tag: &mut Option<String>,
    icon_bank: &mut crate::icons::IconBank,
) {
    diplomacy_card(ui, "国家列表", |ui| {
        ui.horizontal(|ui| {
            ui.label(format!("{}:", tr("sort_alpha")));
            if ui
                .selectable_label(!*sort_by_opinion, tr("alphabetical"))
                .clicked()
            {
                *sort_by_opinion = false;
            }
            if ui
                .selectable_label(*sort_by_opinion, tr("by_opinion"))
                .clicked()
            {
                *sort_by_opinion = true;
            }
        });
        ui.add_space(4.0);

        egui::ScrollArea::vertical()
            .max_height(260.0)
            .show(ui, |ui| {
                let mut sorted: Vec<&CountryEntry> = data.countries.iter().collect();
                if *sort_by_opinion {
                    sorted.sort_by(|a, b| b.opinion.cmp(&a.opinion));
                } else {
                    sorted.sort_by(|a, b| a.tag.cmp(&b.tag));
                }
                for c in sorted {
                    let selected = selected_tag.as_deref() == Some(c.tag.as_str());
                    let response = ui
                        .horizontal(|ui| {
                            render_country_portrait(ui, c, icon_bank);
                            ui.vertical(|ui| {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(tr(&c.tag)).color(country_color(c)).strong(),
                                    );
                                    ui.label(format!("({:+})", c.opinion));
                                });
                                if !c.leader_name.is_empty() {
                                    ui.label(
                                        RichText::new(&c.leader_name)
                                            .small()
                                            .color(Color32::from_gray(200)),
                                    );
                                }
                                if let Some(summary) = &c.autonomy_summary {
                                    ui.label(RichText::new(summary).small().color(WARN));
                                }
                            });
                        })
                        .response;
                    if response.clicked() {
                        *selected_tag = Some(c.tag.clone());
                    }
                    if selected {
                        response.highlight();
                    }
                    ui.separator();
                }
            });
    });
}

fn render_country_portrait(
    ui: &mut egui::Ui,
    c: &CountryEntry,
    icon_bank: &mut crate::icons::IconBank,
) {
    let size = egui::Vec2::new(48.0, 48.0);
    if let Some(gfx) = c.leader_portrait_key.as_deref() {
        if let Some(handle) = icon_bank.get_or_load(gfx) {
            ui.add(
                egui::Image::from_texture(handle)
                    .fit_to_exact_size(size)
                    .corner_radius(2.0),
            );
            return;
        }
    }
    if let Some(handle) = icon_bank.get_or_load(&c.flag_gfx) {
        ui.add(
            egui::Image::from_texture(handle)
                .fit_to_exact_size(size)
                .corner_radius(2.0),
        );
        return;
    }
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    ui.painter().rect_filled(rect, 2.0, Color32::from_gray(40));
    ui.painter().rect_stroke(
        rect,
        2.0,
        egui::Stroke::new(1.0, GOLD),
        egui::epaint::StrokeKind::Outside,
    );
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        &c.tag,
        egui::FontId::proportional(11.0),
        GOLD,
    );
}

fn selected_detail<'a>(
    data: &'a DiplomacyData,
    selected_tag: &mut Option<String>,
) -> Option<&'a CountryDiplomacyDetail> {
    if selected_tag.is_none() {
        *selected_tag = data.countries.first().map(|c| c.tag.clone());
    }
    data.countries
        .iter()
        .find(|c| selected_tag.as_deref() == Some(c.tag.as_str()))
        .and_then(|c| c.detail.as_ref())
}

pub fn render_country_diplomacy_detail(
    ui: &mut egui::Ui,
    detail: &CountryDiplomacyDetail,
    cmds: &mut Vec<DiplomacyCommand>,
) {
    let colonial_or_subject_population = detail.colonial_population + detail.subject_population;
    metric_tile_grid(
        ui,
        &[
            (
                tr("opinion").to_owned(),
                format!("{:+}", detail.opinion),
                relation_color(detail),
            ),
            (
                tr("faction").to_owned(),
                detail
                    .faction_name
                    .clone()
                    .unwrap_or_else(|| "无".to_owned()),
                if detail.same_faction { GOOD } else { GOLD },
            ),
            (
                "战争".to_owned(),
                if detail.at_war { "交战" } else { "和平" }.to_owned(),
                if detail.at_war { BAD } else { GOOD },
            ),
        ],
    );
    ui.add_space(6.0);
    ui.add(egui::Label::new(RichText::new(&detail.display_name).strong().color(GOLD)).wrap());
    if let Some(overlord) = &detail.overlord_name {
        let level = detail.autonomy_level_name.as_deref().unwrap_or("自治附庸");
        ui.label(RichText::new(format!("宗主国: {} ({})", overlord, level)).color(WARN));
    }
    if !detail.subject_names.is_empty() {
        ui.label(
            RichText::new(format!("傀儡/自治领: {}", detail.subject_names.join("、")))
                .color(Color32::from_rgb(0xc0, 0xe0, 0xff)),
        );
    }
    metric_tile_grid(
        ui,
        &[
            (
                "本土".to_owned(),
                format_population(detail.domestic_population),
                GOOD,
            ),
            (
                "殖民/属国".to_owned(),
                format_population(colonial_or_subject_population),
                WARN,
            ),
            (
                "管辖".to_owned(),
                format_population(detail.governed_population),
                GOLD,
            ),
            (
                "其中属国".to_owned(),
                format_population(detail.subject_population),
                WARN,
            ),
            (
                "帝国".to_owned(),
                format_population(detail.imperial_population),
                GOLD,
            ),
        ],
    );
    if detail.justifying_wargoal {
        ui.add(
            egui::ProgressBar::new(detail.justify_progress.clamp(0.0, 1.0))
                .desired_width(ui.available_width().min(280.0))
                .text(format!(
                    "{} ({}d)",
                    tr("justifying_wargoal"),
                    detail.justify_days_remaining
                )),
        );
    } else if detail.has_wargoal {
        ui.label(RichText::new(tr("wargoal_ready")).color(GOOD).strong());
    }
    if !detail.wargoals.is_empty() {
        ui.add_space(4.0);
        ui.label(RichText::new("战争目标详情").strong().color(GOLD));
        for goal in &detail.wargoals {
            let state = goal
                .target_state
                .map(|s| format!(" 州 {}", s))
                .unwrap_or_default();
            ui.horizontal_wrapped(|ui| {
                ui.label(format!("{}{}", goal.kind, state));
                ui.label(
                    RichText::new(&goal.status)
                        .small()
                        .color(if goal.progress >= 1.0 { GOOD } else { WARN }),
                );
                if goal.progress < 1.0 {
                    ui.label(
                        RichText::new(format!("剩余 {} 天", goal.days_remaining))
                            .small()
                            .color(components::MUTED),
                    );
                }
                ui.label(
                    RichText::new(format!("来源: {}", goal.source))
                        .small()
                        .color(components::MUTED),
                );
            });
        }
    }
    if !detail.relation_factors.is_empty() {
        ui.add_space(4.0);
        ui.label(RichText::new("关系构成").strong().color(GOLD));
        for factor in &detail.relation_factors {
            ui.horizontal_wrapped(|ui| {
                ui.label(&factor.label);
                ui.label(
                    RichText::new(&factor.value)
                        .small()
                        .color(if factor.positive { GOOD } else { WARN }),
                );
            });
        }
    }
    ui.separator();
    ui.horizontal_wrapped(|ui| {
        if !detail.has_wargoal && !detail.justifying_wargoal {
            let btn = ui.add_enabled(
                detail.justify_action.enabled,
                egui::Button::new(format!("📋 {}", tr("justify_wargoal"))),
            );
            if add_action_hover(btn, &detail.justify_action).clicked() {
                cmds.push(DiplomacyCommand::JustifyWargoal(detail.tag.clone()));
            }
        }
        let btn = ui.add_enabled(
            detail.declare_war_action.enabled,
            egui::Button::new(format!("⚔ {}", tr("declare_war"))),
        );
        if add_action_hover(btn, &detail.declare_war_action).clicked() {
            cmds.push(DiplomacyCommand::DeclareWar(detail.tag.clone()));
        }
        let btn = ui.add_enabled(
            detail.invite_to_faction_action.enabled,
            egui::Button::new(format!("🤝 {}", tr("invite_to_faction"))),
        );
        if add_action_hover(btn, &detail.invite_to_faction_action).clicked() {
            cmds.push(DiplomacyCommand::InviteToFaction(detail.tag.clone()));
        }
        let btn = ui.add_enabled(
            detail.request_access_action.enabled,
            egui::Button::new(format!("🛂 {}", tr("request_access"))),
        );
        if add_action_hover(btn, &detail.request_access_action).clicked() {
            cmds.push(DiplomacyCommand::RequestMilitaryAccess(detail.tag.clone()));
        }
    });
    render_action_reason(ui, tr("justify_wargoal"), &detail.justify_action);
    render_action_reason(ui, tr("declare_war"), &detail.declare_war_action);
    render_action_reason(
        ui,
        tr("invite_to_faction"),
        &detail.invite_to_faction_action,
    );
    render_action_reason(ui, tr("request_access"), &detail.request_access_action);
}

fn render_action_reason(ui: &mut egui::Ui, label: &str, action: &DiplomaticActionView) {
    if action.enabled {
        return;
    }
    let Some(reason) = &action.reason else {
        return;
    };
    ui.add(
        egui::Label::new(
            RichText::new(format!("{}：{}", label, reason))
                .small()
                .color(components::MUTED),
        )
        .wrap(),
    );
}

fn render_factions(ui: &mut egui::Ui, data: &DiplomacyData) {
    diplomacy_card(
        ui,
        &tr("all_factions").replace("{}", &data.all_factions.len().to_string()),
        |ui| {
            if data.all_factions.is_empty() {
                ui.label(RichText::new(tr("no_factions_formed")).color(components::MUTED));
            } else {
                for f in &data.all_factions {
                    ui.label(
                        RichText::new(format!("{} ({})", f.name, f.member_tags.len()))
                            .strong()
                            .color(GOLD),
                    );
                    ui.label(tr("leader_label").replace("{}", &tr(&f.leader_tag)));
                    let members: Vec<String> =
                        f.member_tags.iter().map(|t| tr(t).to_string()).collect();
                    ui.label(
                        RichText::new(tr("members_label").replace("{}", &members.join(" · ")))
                            .small(),
                    );
                    ui.add_space(4.0);
                }
            }
        },
    );
}

fn render_wars(ui: &mut egui::Ui, data: &DiplomacyData, cmds: &mut Vec<DiplomacyCommand>) {
    diplomacy_card(
        ui,
        &format!("战争与和平 ({})", data.active_wars.len()),
        |ui| {
            if data.active_wars.is_empty() {
                ui.label(
                    RichText::new("当前没有活跃战争。和平会议按钮会在参战后显示。")
                        .color(components::MUTED),
                );
            } else {
                for war in &data.active_wars {
                    ui.group(|ui| {
                        ui.label(
                            RichText::new(format!(
                                "War #{}: {} vs {}",
                                war.id,
                                tr(&war.primary_attacker_tag),
                                tr(&war.primary_defender_tag)
                            ))
                            .strong()
                            .color(GOLD),
                        );
                        ui.add(
                            egui::Label::new(format!(
                                "进攻方: {}",
                                war.attacker_tags
                                    .iter()
                                    .map(|tag| tr(tag).to_string())
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            ))
                            .wrap(),
                        );
                        ui.add(
                            egui::Label::new(format!(
                                "防守方: {}",
                                war.defender_tags
                                    .iter()
                                    .map(|tag| tr(tag).to_string())
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            ))
                            .wrap(),
                        );
                        ui.add(
                            egui::Label::new(format!(
                                "战争分数: 进攻方 {:.1} / 防守方 {:.1}",
                                war.attacker_score, war.defender_score
                            ))
                            .wrap(),
                        );
                        if !war.wargoals.is_empty() {
                            ui.label(RichText::new("和平条款预览:").strong());
                            for goal in &war.wargoals {
                                let state = goal
                                    .target_state
                                    .map(|s| format!(" state {}", s))
                                    .unwrap_or_default();
                                ui.add(
                                    egui::Label::new(format!(
                                        "- {} 将执行 {} 于 {}{}",
                                        goal.claimant_name, goal.kind, goal.target_name, state
                                    ))
                                    .wrap(),
                                );
                            }
                        }
                        ui.horizontal(|ui| {
                            let btn = ui.add_enabled(
                                war.attacker_peace_action.enabled,
                                egui::Button::new("执行进攻方和平"),
                            );
                            if add_action_hover(btn, &war.attacker_peace_action).clicked() {
                                cmds.push(DiplomacyCommand::ResolvePeace {
                                    war_id: war.id,
                                    winning_side: PeaceSide::Attacker,
                                });
                            }
                            let btn = ui.add_enabled(
                                war.defender_peace_action.enabled,
                                egui::Button::new("执行防守方和平"),
                            );
                            if add_action_hover(btn, &war.defender_peace_action).clicked() {
                                cmds.push(DiplomacyCommand::ResolvePeace {
                                    war_id: war.id,
                                    winning_side: PeaceSide::Defender,
                                });
                            }
                        });
                    });
                }
            }
        },
    );
}

fn diplomacy_card(ui: &mut egui::Ui, title: &str, add_contents: impl FnOnce(&mut egui::Ui)) {
    components::section_card(ui, title, add_contents);
}

fn metric_tile(ui: &mut egui::Ui, label: &str, value: String, color: Color32) {
    let h_margin = 8.0 + 8.0;
    let total = ui.available_width().max(40.0);
    let inner_width = (total - h_margin).max(20.0);
    let inner = egui::Frame::new()
        .fill(components::PANEL_CARD_DEEP)
        .stroke(egui::Stroke::new(1.0, components::STROKE_TILE))
        .inner_margin(egui::Margin {
            left: 8,
            right: 8,
            top: 4,
            bottom: 4,
        })
        .show(ui, |ui| {
            ui.set_min_width(inner_width);
            ui.set_max_width(inner_width);
            ui.add(
                egui::Label::new(RichText::new(label).size(10.5).color(components::MUTED)).wrap(),
            );
            ui.add_space(1.0);
            ui.add(egui::Label::new(RichText::new(value).strong().size(13.5).color(color)).wrap());
        });
    let rect = inner.response.rect;
    let bar = egui::Rect::from_min_size(
        rect.left_top() + egui::vec2(1.0, 1.0),
        egui::vec2(3.0, rect.height() - 2.0),
    );
    ui.painter().rect_filled(bar, 0.0, color);
}

fn metric_tile_grid(ui: &mut egui::Ui, items: &[(String, String, Color32)]) {
    if items.is_empty() {
        return;
    }
    let min_tile_width = 110.0;
    let available = ui.available_width();
    let fit = ((available / min_tile_width).floor() as usize).max(1);
    let per_row = fit.min(items.len()).max(1);
    for chunk in items.chunks(per_row) {
        ui.columns(chunk.len(), |cols| {
            for (i, (label, value, color)) in chunk.iter().enumerate() {
                metric_tile(&mut cols[i], label, value.clone(), *color);
            }
        });
        ui.add_space(4.0);
    }
}

fn format_population(value: u64) -> String {
    if value >= 1_000_000_000 {
        format!("{:.2}B", value as f64 / 1_000_000_000.0)
    } else if value >= 1_000_000 {
        format!("{:.1}M", value as f64 / 1_000_000.0)
    } else if value >= 1_000 {
        format!("{:.1}K", value as f64 / 1_000.0)
    } else {
        value.to_string()
    }
}

fn country_color(c: &CountryEntry) -> Color32 {
    if c.at_war {
        BAD
    } else if c.same_faction {
        GOOD
    } else if c.opinion > 0 {
        Color32::LIGHT_GREEN
    } else if c.opinion < -50 {
        Color32::from_rgb(0xff, 0x80, 0x80)
    } else {
        Color32::GRAY
    }
}

fn relation_color(detail: &CountryDiplomacyDetail) -> Color32 {
    if detail.at_war {
        BAD
    } else if detail.same_faction {
        GOOD
    } else if detail.opinion > 50 {
        Color32::LIGHT_GREEN
    } else if detail.opinion < -50 {
        Color32::from_rgb(0xff, 0x80, 0x80)
    } else {
        Color32::GRAY
    }
}

pub fn add_action_hover(response: egui::Response, action: &DiplomaticActionView) -> egui::Response {
    if let Some(reason) = &action.reason {
        response.on_hover_text(format!("{}\n{}", action.preview, reason))
    } else if !action.preview.is_empty() {
        response.on_hover_text(&action.preview)
    } else {
        response
    }
}
