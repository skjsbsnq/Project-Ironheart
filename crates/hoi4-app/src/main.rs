// Phase 0.2: Entry point (moved from hoi4-render). Render module is now a library.
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::time::Instant;

use glam::{Vec2, Vec4};
use winit::keyboard::KeyCode;
use winit::window::{Fullscreen, Window};

use hoi4_paths::PathConfig;
use hoi4_state::{CountryId, GameSpeed, PopClass, World};

use hoi4_audio::{AudioVolumes, MusicPlayer, UiSound, UiSoundBank};
use hoi4_render::buildings::{generate_buildings, generate_poi_icons, PoiIconInstance};
use hoi4_render::camera::{Camera, CameraUniform, RenderParams};
use hoi4_render::counter_layout::{
    build_hit_regions, hit_test, layout_screen_space, HitRegion, LayoutCounter,
};
use hoi4_render::counter_v3::{
    flag_bits, generate_hoi3_counters_cr3, project_counter_anchor_screen,
    project_counter_screen_pos, CounterMotionOverride, Hoi3CounterInstance,
};
use hoi4_render::defines::VanillaMapSpace;
use hoi4_render::frontlines::{generate_frontline_vertices, FrontVertex};
use hoi4_render::map_mode::{build_color_lut, color_lut_entry, MapMode};
use hoi4_render::railways::{
    build_railway_vertices_with_bridges, compute_province_centroids, parse_railways, RailVertex,
    RailwayParams,
};
use hoi4_render::sdf::{compute_coast_sdf, compute_country_sdf, compute_province_sdf};
use hoi4_render::terrain::{
    build_wrapped_instance_buckets_into, ChunkGrid, ChunkInstance, LOD_GRID,
};
use hoi4_render::trees::generate_trees_with_stats;
use passes::counter_v3::Hoi3CounterPass;
use passes::PoiIconPass;

use hoi4_logic::economy::EconomyState;
use hoi4_logic::politics::PoliticsCache;
use hoi4_logic::research::ResearchState;
use hoi4_runtime::{init_simulation, AiState, ScriptState, SystemSchedule};

mod text_pass;
use text_pass::{TextAlign, TextPass, TextSize};

mod glyphon_text;

mod app_helpers;
mod app_shell;
mod binding;
mod bootstrap;
mod camera_control;
mod combat_overlay;
mod content_bootstrap;
mod debug_commands;
mod edge_pan_test;
mod flag_bank;
mod gpu_readback;
mod gpu_utils;
mod map_baseline;
mod map_draw;
mod map_frame;
mod map_image_diff;
mod map_interaction;
mod map_perf;
mod map_phase0_run;
mod map_refresh;
mod map_renderer;
mod map_trade_routes;
mod mapname_atlas;
mod menu_pass;
mod menu_scene;
mod military_ui_data;
mod panel_pass;
mod passes;
mod province_name_atlas;
mod render_assets;
mod render_collect;
mod render_init;
mod render_state;
mod runtime;
mod ui_binding;
mod ui_driver;
mod update_loop;
mod world_overlays;
use app_helpers::{
    decision_id_matches_player_tag, estimate_construction_days_remaining, event_modal_sound,
    intervention_expected_impact, map_mode_from_capture_name, map_mode_terrain_blend_for,
    postprocess_lut_selection_for, surrender_notification_sound_key,
    terrain_debug_view_for_baseline_layer,
};
use combat_overlay::show_combat_bubble_overlay;
use edge_pan_test::{EdgePanTestConfig, EdgePanTestRun};
use flag_bank::FlagBank;
use gpu_readback::{enqueue_png_readback, finish_png_readback};
use gpu_utils::{
    make_depth_view, parity_required_limits, MIN_FRAGMENT_SAMPLED_TEXTURES_FOR_PARITY,
};
use hoi4_app::ui_data::cache::{UiPanelCache, UiPanelCacheKind};
use hoi4_app::ui_data::names::localized_content_name;
pub use hoi4_app::vanilla_resource_views;
pub use hoi4_app::vanilla_targets;
use hoi4_render::global_uniform::GlobalFrameUniform;
use map_interaction::{FrontlinePainterState, PainterMode, SelectionBoxState};
use map_perf::{
    estimate_frame_texture_memory_bytes, phase10_overlay_lines, GpuProfilerStatus,
    GpuTimestampProfiler, MapQualityPreset, Phase10OverlayInput,
};
use map_phase0_run::MapPhase0Run;
use map_refresh::upload_lut;
use map_renderer::{MapPrepareFrameInput, MapRenderer, WorldObjectPlan, WorldObjectSystem};
use menu_pass::{CountryEntry, MenuButton};
use menu_scene::MenuKind;
use military_ui_data::{
    air_mission_from_ui, air_mission_to_ui, build_template_editor_data,
    build_template_subunit_picker_data, empty_template_editor_data,
    empty_template_subunit_picker_data, first_open_line_slot, first_open_support_slot,
    naval_mission_from_ui, naval_mission_to_ui,
};
use panel_pass::PanelPass;
use passes::{
    ColorCubeSource, DebugOverlay, GlobalUniformBuffer, HdrTarget, PassRegistry, PostProcessChain,
    PostProcessDebugView, PostProcessMode, SimpleBlitPass, TerrainPass, WaterRefractionPass,
    WaterRefractionTarget, HDR_FORMAT,
};
use render_assets::{
    load_colormap, load_colormap_phase1, load_rivers_texture, load_rivers_texture_phase1,
    load_terrain_atlas, load_terrain_atlas_phase1, load_tree_atlas, setup_tree_mesh_pipeline,
};
use render_state::RenderState;
use vanilla_resource_views::VanillaResourceViews;
use vanilla_targets::{
    VanillaRuntimeTargetFrameParams, VanillaRuntimeTargetInputs, VanillaRuntimeTargets,
};
use world_overlays::{CounterVisibilityCache, VisualDivisionMotion};
/// World units per heightmap pixel (XZ). Smaller = "denser" world.
const WORLD_SCALE: f32 = 0.02;
/// World Y for full-white heightmap pixel (255 -> this height).
/// HOI4 vanilla heightmap goes 0..255; ~95 = sea level, mountains ~180-220.
/// With WORLD_SCALE=0.02 the world is 112??1 units, so we want a height_scale
/// that gives readable relief without making the strategic map look spiky.
const HEIGHT_SCALE: f32 = 1.45;
/// Latitude squash factor (0..1). 0=no correction.
/// HOI4's source map already uses a partial projection correction; adding our
/// own parabolic squash on top distorts shapes more than it helps, so default
/// to off. Keep the plumbing for experimentation.
const LAT_CORRECTION: f32 = 0.0;
/// Number of chunks across the map (X ??Z).
const CHUNKS_X: u32 = 32;
const CHUNKS_Z: u32 = 12;

/// Phase 4.1: Topbar hit regions for tooltip detection.
/// Each region: (x_start, x_end, label_index).
/// The topbar is ~46px tall; we check y < 46.
const TOPBAR_HEIGHT: f32 = 46.0;
const MAP_CLICK_DRAG_THRESHOLD_PX: f32 = 12.0;
const UNIT_BOX_SELECT_THRESHOLD_PX: f32 = 24.0;
const UNIT_BOX_SELECT_HOLD_MS: u128 = 180;
const EDGE_PAN_MARGIN_PX: f32 = 12.0;
const EDGE_PAN_SPEED_SCALE: f32 = 0.65;
const MAX_INTERACTION_DT_SECS: f32 = 1.0 / 30.0;
const INTERACTIVE_RENDER_QUALITY_HOLD_SECS: f32 = 0.18;
const TARGET_UI_FRAME_SECS: f32 = 1.0 / 60.0;
const REDRAW_GUARD_SECS: f32 = 0.002;
const MIN_SIM_SLICE_SECS: f32 = 0.001;
const REDRAW_OVERDUE_SIM_BUDGET_SECS: f32 = 0.002;
const INTERACTIVE_FAST_SIM_BUDGET_SECS: f32 = 0.002;
const FAST_VISUAL_REBUILD_INTERVAL_SECS: f32 = 1.0 / 20.0;
const SMOOTH_ZOOM_RESPONSE: f32 = 18.0;
const HOVER_PICK_INTERVAL_MS: u128 = 33;
const TOOLTIP_DELAY_MS: u128 = 400; // ms before tooltip appears

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct FrontlineParams {
    opacity: f32,
    _pad: [f32; 3],
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct BuildingParams {
    opacity: f32,
    scale: f32,
    brightness: f32,
    _pad0: f32,
}

impl Default for BuildingParams {
    fn default() -> Self {
        Self {
            opacity: 1.0,
            scale: 1.0,
            brightness: 1.0,
            _pad0: 0.0,
        }
    }
}

const _: () = assert!(std::mem::size_of::<BuildingParams>() == 16);

fn build_law_tiers(
    cat: hoi4_state::LawCategory,
    db: &hoi4_content::V6Database,
) -> Vec<hoi4_ui::law_panel::LawTierEntry> {
    match cat {
        hoi4_state::LawCategory::Conscription => db
            .conscription_laws
            .iter()
            .map(|l| hoi4_ui::law_panel::LawTierEntry {
                id: l.id.clone(),
                name: localized_content_name(&l.id, &l.name),
                pp_cost: l.pp_cost,
                cooldown_days: l.cooldown_days,
                effects: vec![
                    format!("可征兵比例 {:.1}%", l.soldier_ratio * 100.0),
                    format!("征兵转化 {:.1}%/日", l.conscription_conversion_rate * 100.0),
                ],
            })
            .collect(),
        hoi4_state::LawCategory::Economy => db
            .economy_laws
            .iter()
            .map(|l| hoi4_ui::law_panel::LawTierEntry {
                id: l.id.clone(),
                name: localized_content_name(&l.id, &l.name),
                pp_cost: l.pp_cost,
                cooldown_days: l.cooldown_days,
                effects: {
                    let mut effects = vec![
                        format!("工人工资 x{:.2}", l.wage_multiplier_worker),
                        format!("建设速度 {:+.0}%", l.construction_speed_modifier * 100.0),
                        format!("消费品需求 x{:.2}", l.consumer_goods_factor),
                    ];
                    if l.id == "corporatist_war_economy" {
                        effects.push("军工获得政府订单".to_owned());
                        effects.push("MEFO 自动融资直到风险上限".to_owned());
                    }
                    if let Some(trade_law) = &l.forces_trade_law {
                        effects.push(format!("强制贸易法律：{}", trade_law));
                    }
                    effects
                },
            })
            .collect(),
        hoi4_state::LawCategory::Trade => db
            .trade_laws
            .iter()
            .map(|l| hoi4_ui::law_panel::LawTierEntry {
                id: l.id.clone(),
                name: localized_content_name(&l.id, &l.name),
                pp_cost: l.pp_cost,
                cooldown_days: l.cooldown_days,
                effects: vec![
                    format!("进口效率 {:.0}%", l.import_efficiency * 100.0),
                    format!("出口效率 {:.0}%", l.export_efficiency * 100.0),
                    format!("进口关税 {:.0}%", l.import_tariff_rate * 100.0),
                    if l.foreign_exchange_control {
                        "外汇管制：启用".to_owned()
                    } else {
                        "外汇管制：关闭".to_owned()
                    },
                ],
            })
            .collect(),
        hoi4_state::LawCategory::Taxation => db
            .taxation_laws
            .iter()
            .map(|l| hoi4_ui::law_panel::LawTierEntry {
                id: l.id.clone(),
                name: localized_content_name(&l.id, &l.name),
                pp_cost: l.pp_cost,
                cooldown_days: l.cooldown_days,
                effects: vec![
                    format!("所得税 {:.0}%", l.income_tax_rate * 100.0),
                    format!("消费税 {:.0}%", l.consumption_tax_rate * 100.0),
                    format!("企业税 {:.0}%", l.corporate_tax_rate * 100.0),
                ],
            })
            .collect(),
        hoi4_state::LawCategory::CivilRights => db
            .civil_rights_laws
            .iter()
            .map(|l| hoi4_ui::law_panel::LawTierEntry {
                id: l.id.clone(),
                name: localized_content_name(&l.id, &l.name),
                pp_cost: l.pp_cost,
                cooldown_days: l.cooldown_days,
                effects: vec![
                    format!("科研槽 {}", l.research_slots),
                    format!("福利率 {:.0}%", l.welfare_rate * 100.0),
                ],
            })
            .collect(),
        hoi4_state::LawCategory::InformationControl => db
            .information_control_laws
            .iter()
            .map(|l| hoi4_ui::law_panel::LawTierEntry {
                id: l.id.clone(),
                name: localized_content_name(&l.id, &l.name),
                pp_cost: l.pp_cost,
                cooldown_days: l.cooldown_days,
                effects: vec![format!("忠诚衰减 x{:.2}", l.loyalty_decay_multiplier)],
            })
            .collect(),
    }
}

fn build_law_slot_entries(
    law_set: Option<&hoi4_state::LawSet>,
    db: &hoi4_content::V6Database,
) -> Vec<hoi4_ui::law_panel::LawSlotEntry> {
    let Some(ls) = law_set else {
        return Vec::new();
    };
    let categories = [
        hoi4_state::LawCategory::Conscription,
        hoi4_state::LawCategory::Economy,
        hoi4_state::LawCategory::Trade,
        hoi4_state::LawCategory::Taxation,
        hoi4_state::LawCategory::CivilRights,
        hoi4_state::LawCategory::InformationControl,
    ];
    categories
        .iter()
        .map(|&cat| {
            let slot = &ls.0[cat.index()];
            let tiers = build_law_tiers(cat, db);
            let current_name = tiers
                .iter()
                .find(|t| t.id == slot.current)
                .map(|t| t.name.clone())
                .unwrap_or_else(|| localized_content_name(&slot.current, &slot.current));
            hoi4_ui::law_panel::LawSlotEntry {
                category: cat,
                current_id: slot.current.clone(),
                current_name,
                cooldown_days: slot.cooldown_days,
                pending: slot.pending.as_ref().map(|(id, rem)| {
                    let name = tiers
                        .iter()
                        .find(|t| t.id == *id)
                        .map(|t| t.name.clone())
                        .unwrap_or_else(|| localized_content_name(id, id));
                    (id.clone(), name, *rem)
                }),
                is_locked: slot.is_locked,
                locked_reason: if slot.is_locked {
                    Some("计划经济锁定了该法律类别。".to_owned())
                } else {
                    None
                },
                previous_before_lock: slot.previous_before_lock.clone(),
                tiers,
            }
        })
        .collect()
}

fn build_politics_law_entries(
    law_set: Option<&hoi4_state::LawSet>,
    db: &hoi4_content::V6Database,
) -> Vec<hoi4_ui::politics::PoliticsLawEntry> {
    build_law_slot_entries(law_set, db)
        .into_iter()
        .map(|slot| hoi4_ui::politics::PoliticsLawEntry {
            category: slot.category,
            current_name: slot.current_name,
            cooldown_days: slot.cooldown_days,
            pending: slot
                .pending
                .map(|(_, target_name, remaining)| (target_name, remaining)),
            is_locked: slot.is_locked,
        })
        .collect()
}
/// Phase 4.2: Game phase state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum GamePhase {
    /// Main menu - show title + New Game / Quit buttons.
    MainMenu,
    /// Country selection - show list of major countries to pick.
    CountrySelect,
    /// Playing - normal map view with simulation running.
    Playing,
}

/// Phase 4.3: In-game overlay panel (opened by topbar button clicks).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InGamePanel {
    Politics,
    Decisions,
    NationalFocus,
    Research,
    Diplomacy,
    Military,
    Naval,
    Air,
    /// V6 ?????????
    Laws,
    /// V6 ??????????????
    Market,
    /// V7.I1 POP read-only panel.
    Pops,
    ConstructionV6,
    /// V6 ?????????
    Finance,
    /// V6 ?????????
    Trade,
    Logistics,
    Situation,
    Settings,
    Saves,
}

// 4.3 Step B (2026-05-18): ?????main ???????`PoliticsTab` ????????ab ?????????????// ??.3 (`tabbedWindowType`) ???????GuiRt-removed ????????????????????????????main ???????
/// Topbar region definitions: (x_min, x_max, region_id)
/// region_id: 0=PP, 1=Stability, 2=WarSupport, 3=Manpower, 4=Factories, 5=Experience, 6=Date
const TOPBAR_REGIONS: [(f32, f32, usize); 7] = [
    (100.0, 170.0, 0), // Political Power
    (170.0, 240.0, 1), // Stability
    (240.0, 330.0, 2), // War Support
    (330.0, 420.0, 3), // Manpower
    (420.0, 520.0, 4), // Factories
    (520.0, 650.0, 5), // Experience
    // Date region is dynamic (right-aligned), handled separately
    (0.0, 0.0, 6), // placeholder, computed at runtime
];

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct ChunkUniform {
    grid: u32,
    _pad: [u32; 3],
}

struct App {
    state: Option<RenderState>,
    camera: Camera,
    smooth_zoom_target_distance: Option<f32>,
    smooth_zoom_anchor_mouse: [f32; 2],
    world: World,
    map_mode: MapMode,
    time_accumulator: f32,
    last_frame: Instant,
    last_redraw_at: Instant,
    last_status_print: Instant,
    last_perf_diag: Instant,
    last_render_profile_log: Instant,
    perf_last_hours: u64,
    perf_counter_rebuilds: u32,
    perf_counter_cache_hits: u32,
    perf_counter_instances: usize,
    perf_render_us: u64,
    perf_render_frames: u32,
    perf_counter_update_us: u64,
    perf_arrow_update_us: u64,
    counter_visibility_cache: CounterVisibilityCache,
    cached_topbar_sig: u64,
    cached_topbar_data: Option<hoi4_ui::topbar::TopBarData>,
    last_topbar_rebuild_at: Instant,
    start_time: Instant,
    dragging: bool,
    heightmap_r16_supported: bool,
    /// Total pixels the mouse moved while LMB was held - used to distinguish
    /// click vs drag for province picking.
    drag_pixels: f32,
    /// Set once a left-button gesture has actually moved the map or painted an
    /// order, so releasing the button cannot also open a province card.
    suppress_next_map_click: bool,
    last_mouse: [f32; 2],
    keys_held: HashSet<KeyCode>,
    /// Selected province ID; `u32::MAX` means no selection.
    selected_province_id: u32,
    hovered_province_id: u32,
    last_hover_pick_at: Instant,
    /// Phase 4.1: Tooltip hover tracking.
    hover_region: Option<usize>, // which topbar region is hovered (index into TOPBAR_REGIONS)
    hover_start: Instant,  // when the hover started
    tooltip_visible: bool, // whether tooltip is currently showing
    /// 4.1.bis.6 diag (2026-05-16): F1 toggles GUI debug overlay.
    /// When true, every visible widget gets a 1-px coloured outline + a
    /// small label showing its name, so misalignment is observable directly.
    debug_overlay: bool,
    terrain_debug_view: passes::TerrainDebugView,
    water_debug_view: passes::WaterDebugView,
    border_debug_view: passes::BorderDebugView,
    postprocess_debug_view: PostProcessDebugView,
    map_quality_preset: MapQualityPreset,
    last_viewport_interaction_at: Option<Instant>,
    force_water_pass: bool,
    last_frame_cpu_ms: f32,
    last_map_prepare_cpu_ms: f32,
    demo_visible: bool,
    b5_demo_visible: bool,
    demo_window: hoi4_ui::demo::DemoWindow,
    v9_demo: hoi4_ui::v9::demo::V9Demo,
    v9_notifications: hoi4_ui::v9::composites::NotificationStack,
    /// Phase 4.2: Game state (menu / country select / playing).
    game_phase: GamePhase,
    /// Phase 4.2: Selected player country index (0 = first country, usually GER).
    player_country: usize,
    /// Phase 4.2 (redesign): cursor in the `available_countries` list (separate from world index).
    country_select_idx: usize,
    /// Phase 0: path config (HOI4 root + mod chain).
    path_cfg: PathConfig,
    /// C.7: localisation catalog.
    loc_catalog: hoi4_ui::loc::LocCatalog,
    /// Phase 1.2: companion states (build queue / research / idea modifier cache).
    econ: EconomyState,
    research: ResearchState,
    politics_cache: PoliticsCache,
    /// Phase 1.4: script runtime (events / decisions / variables / flags / registry).
    script: ScriptState,
    /// Phase 1.5: strategic AI state (per-country cooldown / personality).
    ai: AiState,
    /// J.7: Feedback Bus (domain event bus for cross-system loops).
    feedback_bus: hoi4_logic::FeedbackBus,
    /// Phase 0.3: system scheduler. Each tick_hour routes to registered hourly/daily/weekly/monthly systems.
    schedule: SystemSchedule,
    /// Phase 2.9: music player.
    music_player: MusicPlayer,
    /// Phase 4.2: ????????????????????????laying ????????????    
    menu_kind: Option<MenuKind>,
    /// Phase 4.2 (redesign): ?????hover ????????????id???btn_new_game" ??????    
    menu_hovered_btn: Option<&'static str>,
    /// Phase 4.2 (redesign): ?????hover ?????????????? index??    
    menu_hovered_row: Option<usize>,
    /// Phase 4.2 (redesign): ??????????????????????????????Phase 4.2 ????????? GER ?????????    
    available_countries: Vec<CountryEntry>,
    /// ???????????????????????+ ????????? click ????????    
    last_main_buttons: Vec<MenuButton>,
    last_country_layout: Option<menu_pass::CountrySelectLayout>,
    _cached_hoi3_counter_upload: Vec<Hoi3CounterInstance>,
    cached_hoi3_counter_sig: u64,
    cached_hoi3_counter_layout_sig: u64,
    cached_hoi3_counter_layout_offsets: HashMap<(u16, u16), [f32; 2]>,
    /// CR-4: hit regions for screen-space counter click detection.
    _cached_hoi3_hit_regions: Vec<HitRegion>,
    division_motion: HashMap<usize, VisualDivisionMotion>,
    last_division_locations: Vec<hoi4_state::ProvinceId>,
    /// Phase 3.12.8: parsed seasons.txt for tree season computation.
    seasons: hoi4_map::SeasonsTxt,
    /// Toggle: show province name labels (F7).
    show_province_names: bool,
    /// Phase 4.3: currently open in-game panel (None = no panel).
    open_panel: Option<InGamePanel>,
    /// Gate 1 panel router: currently open object/detail panel.
    active_detail_panel: Option<hoi4_ui::ActiveDetailPanel>,
    /// Gate 1 panel router: currently open short-lived popup.
    active_popup: Option<hoi4_ui::ActivePopup>,
    /// C.5: diplomacy panel sort toggle.
    diplomacy_sort_by_opinion: bool,
    diplomacy_selected_country_tag: Option<String>,
    /// Construction placement mode: Some(building_key) = waiting for province click.
    construction_mode: Option<String>,
    /// Provinces tinted while construction placement mode is active.
    construction_highlight_province_ids: HashSet<u32>,
    /// Player-facing auto-build toggle; when enabled it tops up the queue monthly.
    auto_build_enabled: bool,
    last_auto_build_month: Option<(u16, u8)>,
    last_auto_build_explanations:
        Vec<hoi4_logic::economy::construction_planner::ConstructionCandidateScore>,
    // V5 ???????026-05-18????????? gui_rt_removed / menu_runtime / menu_hovered_id /
    // menu_pressed_id ????????????????????????????????? vanilla GUI ???????????    // (`hoi4_assets::GuiRt-removed` / topbar.gui / countrypoliticsview.gui)??    // ?????hit-test ???????`last_main_buttons` / `last_country_layout`??    // topbar / ?????????????????????????B (egui)??
    content: hoi4_runtime::ContentRuntimeState,
    focus_panel: hoi4_ui::focus_tree_panel::FocusTreePanel,
    /// F.1: ???????????????????????
    pre_event_speed: Option<GameSpeed>,
    /// Last event modal id that played the popup sound, to avoid replaying every frame.
    last_event_sound_id: Option<String>,
    /// P1.1????????????????????
    pending_surrender_notifications: Vec<hoi4_ui::surrender_notification::SurrenderNotification>,
    /// Last surrender/peace notification that played its popup sound.
    last_surrender_sound_key: Option<String>,
    /// Cached snapshot of last logged country label province counts (for delta logging).
    last_label_provinces: Vec<u32>,
    map_refresh_owners: Vec<hoi4_state::CountryId>,
    map_refresh_controllers: Vec<hoi4_state::CountryId>,
    /// E.2: track war count for auto-pause on new war.
    last_war_count: usize,
    country_info_panel: hoi4_ui::country_info_panel::CountryInfoPanel,
    /// J.2: province left-click info card.
    province_info_card: hoi4_ui::province_info::ProvinceInfoCard,
    /// J.2: cached info data for the card.
    province_info_data: hoi4_ui::province_info::ProvinceInfoData,
    /// Selected division indices for movement commands.
    selected_divisions: Vec<usize>,
    selection_box: SelectionBoxState,
    /// CR-4.3: Multi-select province set.
    selected_province_ids: HashSet<u32>,
    /// CR-4.4: Expanded stacks (fan-out).
    expanded_stacks: HashSet<u32>,
    /// CR-4.5: Right-click counter menu province.
    counter_right_click_province: Option<u32>,
    /// CR-4.5: Pending move command (next left-click sets destination).
    pending_move_command: bool,
    selected_combat_bubble: Option<u64>,
    pending_naval_move_fleet: Option<u32>,
    naval_transfer_source_fleet: Option<u32>,
    pending_air_transfer_wing: Option<u32>,
    air_transfer_source_wing: Option<u32>,
    ui_sounds: UiSoundBank,
    settings: hoi4_ui::settings::Settings,
    settings_panel: hoi4_ui::settings::SettingsPanel,
    save_browser: hoi4_ui::save_browser::SaveBrowser,
    end_screen: hoi4_ui::end_screen::EndScreen,
    prev_focuses_completed: u32,
    /// 11.1???rontline painter state (draw frontline / arrow by mouse drag).
    frontline_painter: FrontlinePainterState,
    /// 11.1???hether frontline overlay is visible (toggle).
    frontline_overlay_visible: bool,
    /// 11.4???irty hash for frontline arrow instances.
    prev_armies_hash: u64,
    frontline_overlay_hash: u64,
    trade_routes_hash: u64,
    last_frontline_arrow_rebuild_at: Instant,
    last_frontline_overlay_rebuild_at: Instant,
    /// 11.2???urrently selected army (click frontline on map or select in bottom bar).
    selected_army_id: Option<hoi4_state::ArmyId>,
    template_editor_open: bool,
    selected_template_idx: Option<u16>,
    template_picker_target: Option<hoi4_ui::military::TemplatePickerTarget>,
    v6_db: hoi4_content::V6Database,
    historical_1936: hoi4_content::Historical1936Database,
    /// P1.3?????????????????????UI ??????
    law_error_message: Option<String>,
    last_law_error_toast: Option<String>,
    ui_panel_cache: UiPanelCache,
    map_phase0: Option<MapPhase0Run>,
    edge_pan_test: Option<EdgePanTestRun>,
}

impl App {
    fn new(
        mut world: World,
        path_cfg: PathConfig,
        edge_pan_test: Option<EdgePanTestConfig>,
    ) -> Self {
        let map_w = world.map.province_map.width as f32 * WORLD_SCALE;
        let map_d = world.map.province_map.height as f32 * WORLD_SCALE;
        let camera = Camera::new(Vec2::new(map_w, map_d), 16.0 / 9.0);

        // Default player_country = index of "GER". Keep World::player in sync
        // before AI state initialization, otherwise the AI can control Germany.
        let default_player = world
            .countries
            .tags
            .iter()
            .position(|t| t == "GER")
            .unwrap_or(0);
        world.player = hoi4_state::CountryId(default_player as u16);

        let (econ, research, politics_cache, script, ai) = init_simulation(&mut world);

        // Phase 2.9 / V5 G.2: music player + UI sound bank
        let mut music_player = MusicPlayer::new();
        music_player.load_playlist_from_paths(&path_cfg);
        if music_player.track_count() > 0 {
            music_player.play();
            println!(
                "[audio] loaded {} tracks, playing",
                music_player.track_count()
            );
        }

        // Load UI sounds with a silent fallback.
        let mut ui_sounds = UiSoundBank::new();
        let ui_loaded = ui_sounds.load_vanilla_from_paths(&path_cfg);
        if ui_loaded > 0 {
            println!("[audio] loaded {} UI sound categories", ui_loaded);
            for sound in [
                UiSound::Click,
                UiSound::OptionClick,
                UiSound::Hover,
                UiSound::PageFlip,
                UiSound::EventPopup,
                UiSound::WorldNews,
                UiSound::WorldDefeat,
                UiSound::WarDeclaration,
            ] {
                println!(
                    "[audio] UI sound {}: {}",
                    sound.label(),
                    if ui_sounds.has_loaded(sound) {
                        "loaded"
                    } else {
                        "missing"
                    }
                );
            }
        } else {
            println!("[audio] UI sounds not found in vanilla install (silent fallback)");
        }

        // Load settings from disk or use defaults on first run.
        let settings = hoi4_ui::settings::Settings::load_or_default();
        // Apply the configured UI language.
        hoi4_ui::i18n::set_language(settings.language);
        // Load localisation catalog. Prefer the selected language so state and
        // victory-point names can be shown in Chinese when available.
        let loc_catalog = load_loc_catalog_for_language(&path_cfg, settings.language);
        // Apply volume settings to music and UI sound banks.
        let initial_volumes = AudioVolumes {
            master: settings.master_volume,
            music: settings.music_volume,
            ui: settings.ui_volume,
        };
        music_player.set_volumes(initial_volumes);
        ui_sounds.set_volumes(initial_volumes);
        let settings_panel = hoi4_ui::settings::SettingsPanel::new(settings.clone());

        // Save browser defaults to the user's Ironheart saves directory.
        let saves_dir = hoi4_ui::save_browser::default_saves_dir();
        let save_browser = hoi4_ui::save_browser::SaveBrowser::new(saves_dir);

        // End screen starts inert.
        let end_screen = hoi4_ui::end_screen::EndScreen::new();

        // Phase 4.2 (redesign): build country selection list.
        // Phase 4.2 ????????? GER ???????????? majors ????????????????????
        let available_countries = build_country_select_list(&world);

        // Phase 3.12.8: load map/seasons.txt for tree season computation.
        let seasons_data =
            hoi4_map::load_seasons_txt(&path_cfg.game_path().join("map/seasons.txt"));

        let scenario_content = content_bootstrap::load_scenario_content("1936");
        let map_refresh_owners = world.provinces.owners.clone();
        let map_refresh_controllers = world.provinces.controllers.clone();

        // P0.1??? world move ????????????
        let initial_day = world.date.days_since_epoch();

        Self {
            state: None,
            camera,
            smooth_zoom_target_distance: None,
            smooth_zoom_anchor_mouse: [0.0; 2],
            world,
            map_mode: MapMode::Political,
            time_accumulator: 0.0,
            last_frame: Instant::now(),
            last_redraw_at: Instant::now(),
            last_status_print: Instant::now(),
            last_perf_diag: Instant::now(),
            last_render_profile_log: Instant::now(),
            perf_last_hours: 0,
            perf_counter_rebuilds: 0,
            perf_counter_cache_hits: 0,
            perf_counter_instances: 0,
            perf_render_us: 0,
            perf_render_frames: 0,
            perf_counter_update_us: 0,
            perf_arrow_update_us: 0,
            counter_visibility_cache: CounterVisibilityCache::default(),
            cached_topbar_sig: 0,
            cached_topbar_data: None,
            last_topbar_rebuild_at: Instant::now(),
            start_time: Instant::now(),
            dragging: false,
            heightmap_r16_supported: false,
            drag_pixels: 0.0,
            suppress_next_map_click: false,
            last_mouse: [0.0; 2],
            keys_held: HashSet::new(),
            selected_province_id: u32::MAX,
            hovered_province_id: u32::MAX,
            last_hover_pick_at: Instant::now(),
            hover_region: None,
            hover_start: Instant::now(),
            tooltip_visible: false,
            debug_overlay: false,
            terrain_debug_view: passes::TerrainDebugView::Off,
            water_debug_view: passes::WaterDebugView::Off,
            border_debug_view: passes::BorderDebugView::Off,
            postprocess_debug_view: PostProcessDebugView::Final,
            map_quality_preset: MapQualityPreset::High,
            last_viewport_interaction_at: None,
            force_water_pass: false,
            last_frame_cpu_ms: 0.0,
            last_map_prepare_cpu_ms: 0.0,
            demo_visible: false,
            b5_demo_visible: false,
            demo_window: hoi4_ui::demo::DemoWindow::new(),
            v9_demo: hoi4_ui::v9::demo::V9Demo::new(),
            v9_notifications: hoi4_ui::v9::composites::NotificationStack::new(),
            game_phase: GamePhase::MainMenu,
            player_country: default_player,
            country_select_idx: 0,
            path_cfg,
            loc_catalog,
            econ,
            research,
            politics_cache,
            script,
            ai,
            feedback_bus: hoi4_logic::FeedbackBus::new(),
            schedule: SystemSchedule::with_phase1_systems(),
            music_player,
            menu_kind: Some(MenuKind::MainMenu),
            menu_hovered_btn: None,
            menu_hovered_row: None,
            available_countries,
            last_main_buttons: Vec::new(),
            last_country_layout: None,
            _cached_hoi3_counter_upload: Vec::new(),
            cached_hoi3_counter_sig: 0,
            cached_hoi3_counter_layout_sig: 0,
            cached_hoi3_counter_layout_offsets: HashMap::new(),
            _cached_hoi3_hit_regions: Vec::new(),
            division_motion: HashMap::new(),
            last_division_locations: Vec::new(),
            seasons: seasons_data,
            show_province_names: true,
            open_panel: None,
            active_detail_panel: None,
            active_popup: None,
            diplomacy_sort_by_opinion: false,
            diplomacy_selected_country_tag: None,
            construction_mode: None,
            construction_highlight_province_ids: HashSet::new(),
            auto_build_enabled: false,
            last_auto_build_month: None,
            last_auto_build_explanations: Vec::new(),
            // V5 ????????ui_rt_removed / menu_runtime / menu_hovered_id /
            // menu_pressed_id / politics_tab / politics_scroll ??????????????
            content: hoi4_runtime::ContentRuntimeState::new(
                &scenario_content,
                hoi4_state::CountryId(default_player as u16),
                initial_day,
            ),
            focus_panel: hoi4_ui::focus_tree_panel::FocusTreePanel::new(),
            pre_event_speed: None,
            last_event_sound_id: None,
            pending_surrender_notifications: Vec::new(),
            last_surrender_sound_key: None,
            last_label_provinces: Vec::new(),
            map_refresh_owners,
            map_refresh_controllers,
            last_war_count: 0,
            country_info_panel: hoi4_ui::country_info_panel::CountryInfoPanel::new(),
            province_info_card: hoi4_ui::province_info::ProvinceInfoCard::new(),
            province_info_data: hoi4_ui::province_info::ProvinceInfoData::default(),
            selected_divisions: Vec::new(),
            selection_box: SelectionBoxState::default(),
            selected_province_ids: HashSet::new(),
            expanded_stacks: HashSet::new(),
            counter_right_click_province: None,
            pending_move_command: false,
            selected_combat_bubble: None,
            pending_naval_move_fleet: None,
            naval_transfer_source_fleet: None,
            pending_air_transfer_wing: None,
            air_transfer_source_wing: None,
            ui_sounds,
            settings,
            settings_panel,
            save_browser,
            end_screen,
            prev_focuses_completed: 0,
            frontline_painter: FrontlinePainterState::default(),
            frontline_overlay_visible: true,
            prev_armies_hash: 0,
            frontline_overlay_hash: 0,
            trade_routes_hash: 0,
            last_frontline_arrow_rebuild_at: Instant::now(),
            last_frontline_overlay_rebuild_at: Instant::now(),
            selected_army_id: None,
            template_editor_open: false,
            selected_template_idx: None,
            template_picker_target: None,
            v6_db: hoi4_content::V6Database::load(),
            historical_1936: hoi4_content::Historical1936Database::load()
                .expect("history_1936 RON should load"),
            law_error_message: None,
            last_law_error_toast: None,
            ui_panel_cache: UiPanelCache::default(),
            map_phase0: None,
            edge_pan_test: edge_pan_test.map(EdgePanTestRun::new),
        }
    }

    fn split_fleet_data(
        world: &mut World,
        player: hoi4_state::CountryId,
        fleet_id: u32,
        count: usize,
    ) {
        let fi = fleet_id as usize;
        if fi >= world.fleets.count || world.fleets.owners[fi] != player || count == 0 {
            return;
        }
        let available = world.fleets.ships[fi].len();
        let move_count = count.min(available.saturating_sub(1));
        if move_count == 0 {
            return;
        }

        let new_name = format!("{} Detachment", world.fleets.names[fi]);
        let new_id = world
            .fleets
            .push(player, world.fleets.region_id[fi], new_name) as usize;
        world.fleets.home_port[new_id] = world.fleets.home_port[fi];
        world.fleets.target_region_id[new_id] = world.fleets.target_region_id[fi];
        world.fleets.arrival_hour[new_id] = world.fleets.arrival_hour[fi];
        world.fleets.speed_knots[new_id] = world.fleets.speed_knots[fi];
        world.fleets.repair_state[new_id] = world.fleets.repair_state[fi];
        world.fleets.mission[new_id] = world.fleets.mission[fi];

        let split_at = world.fleets.ships[fi].len() - move_count;
        let moved = world.fleets.ships[fi].split_off(split_at);
        for ship in &moved {
            let si = ship.0 as usize;
            if si < world.ships.count {
                world.ships.fleet_id[si] = hoi4_state::FleetId(new_id as u32);
            }
        }
        world.fleets.ships[new_id] = moved;
        world.rebuild_runtime_country_indexes();
    }

    fn transfer_ships_between_fleets_data(
        world: &mut World,
        player: hoi4_state::CountryId,
        from_fleet_id: u32,
        to_fleet_id: u32,
        count: usize,
    ) {
        let from = from_fleet_id as usize;
        let to = to_fleet_id as usize;
        if from == to
            || from >= world.fleets.count
            || to >= world.fleets.count
            || world.fleets.owners[from] != player
            || world.fleets.owners[to] != player
            || count == 0
        {
            return;
        }
        let move_count = count.min(world.fleets.ships[from].len());
        if move_count == 0 {
            return;
        }
        let split_at = world.fleets.ships[from].len() - move_count;
        let moved = world.fleets.ships[from].split_off(split_at);
        for ship in &moved {
            let si = ship.0 as usize;
            if si < world.ships.count {
                world.ships.fleet_id[si] = hoi4_state::FleetId(to_fleet_id);
            }
        }
        world.fleets.ships[to].extend(moved);
    }

    fn split_air_wing_data(
        world: &mut World,
        player: hoi4_state::CountryId,
        wing_id: u32,
        planes: u32,
    ) {
        let wi = wing_id as usize;
        if wi >= world.air_wings.count || world.air_wings.owners[wi] != player || planes == 0 {
            return;
        }
        let move_planes = planes.min(world.air_wings.count_planes[wi].saturating_sub(1));
        if move_planes == 0 {
            return;
        }

        world.air_wings.count_planes[wi] -= move_planes;
        world.air_wings.max_planes[wi] = world.air_wings.max_planes[wi]
            .saturating_sub(move_planes)
            .max(world.air_wings.count_planes[wi]);

        let new_id = world.air_wings.push(
            player,
            world.air_wings.aircraft_keys[wi].clone(),
            world.air_wings.region_id[wi],
            move_planes,
            world.air_wings.max_organisation[wi],
            format!("{} Detachment", world.air_wings.names[wi]),
        ) as usize;
        world.air_wings.base_state[new_id] = world.air_wings.base_state[wi];
        world.air_wings.target_region[new_id] = world.air_wings.target_region[wi];
        world.air_wings.transfer_arrival_hour[new_id] = world.air_wings.transfer_arrival_hour[wi];
        world.air_wings.range_km[new_id] = world.air_wings.range_km[wi];
        world.air_wings.reinforce_enabled[new_id] = world.air_wings.reinforce_enabled[wi];
        world.air_wings.mission[new_id] = world.air_wings.mission[wi];
        world.air_wings.organisation[new_id] = world.air_wings.organisation[wi];
        world.air_wings.experience[new_id] = world.air_wings.experience[wi];
        world.rebuild_runtime_country_indexes();
    }

    fn transfer_planes_between_wings_data(
        world: &mut World,
        player: hoi4_state::CountryId,
        from_wing_id: u32,
        to_wing_id: u32,
        planes: u32,
    ) {
        let from = from_wing_id as usize;
        let to = to_wing_id as usize;
        if from == to
            || from >= world.air_wings.count
            || to >= world.air_wings.count
            || world.air_wings.owners[from] != player
            || world.air_wings.owners[to] != player
            || world.air_wings.aircraft_keys[from] != world.air_wings.aircraft_keys[to]
            || planes == 0
        {
            return;
        }
        let move_planes = planes.min(world.air_wings.count_planes[from]);
        if move_planes == 0 {
            return;
        }
        world.air_wings.count_planes[from] -= move_planes;
        world.air_wings.count_planes[to] += move_planes;
        world.air_wings.max_planes[from] = world.air_wings.max_planes[from]
            .saturating_sub(move_planes)
            .max(world.air_wings.count_planes[from]);
        world.air_wings.max_planes[to] =
            world.air_wings.max_planes[to].max(world.air_wings.count_planes[to]);
    }

    fn v6_industry_counts(world: &World, country: CountryId) -> (u32, u32, u32) {
        if country.is_none() {
            return (0, 0, 0);
        }
        let mut civilian = 0u32;
        let mut military = 0u32;
        let mut shipyards = 0u32;
        for building in &world.countries.buildings_v6.buildings {
            let state_idx = building.state.0 as usize;
            if state_idx >= world.states.count || world.states.owners[state_idx] != country {
                continue;
            }
            if building.level == 0 {
                continue;
            }
            match building.building_def_id.as_str() {
                "shipyard" => shipyards += building.level as u32,
                _ if building.kind == hoi4_state::BuildingKind::Military => {
                    military += building.level as u32
                }
                _ if building.kind == hoi4_state::BuildingKind::MilitaryBase => {}
                _ => civilian += building.level as u32,
            }
        }
        (civilian, military, shipyards)
    }

    fn v6_construction_points(world: &World, country: CountryId) -> u32 {
        if country.is_none() {
            return 0;
        }
        hoi4_logic::economy::construction_tick::construction_cp_pool(world, country.0 as usize)
            as u32
    }

    fn auto_enqueue_player_construction(
        world: &World,
        econ: &mut hoi4_logic::economy::EconomyState,
        v6_db: &hoi4_content::V6Database,
        player: usize,
    ) -> Vec<hoi4_logic::economy::construction_planner::ConstructionCandidateScore> {
        if player >= world.countries.count || player >= econ.construction.len() {
            return Vec::new();
        }

        let country = CountryId(player as u16);
        let cp = Self::v6_construction_points(world, country).max(1) as usize;
        let target_queue_len =
            hoi4_logic::economy::construction_planner::target_queue_len_from_cp(cp as u32);
        let current_queue_len = econ.construction[player].items.len();
        if current_queue_len >= target_queue_len {
            return Vec::new();
        }

        let mut queued_by_state = vec![0u16; world.states.count];
        let mut queued_by_building_state: HashMap<(u16, String), u8> = HashMap::new();
        for item in &econ.construction[player].items {
            let si = item.target_state.0 as usize;
            if si < queued_by_state.len() {
                queued_by_state[si] = queued_by_state[si].saturating_add(1);
            }
            *queued_by_building_state
                .entry((item.target_state.0, item.building_key.clone()))
                .or_default() += 1;
        }

        let plan = hoi4_logic::economy::construction_planner::plan_construction(
            world,
            econ,
            v6_db,
            country,
            hoi4_logic::economy::construction_planner::ConstructionPlanningMode::PlayerAutoBuild,
            cp as u32,
        );

        let mut added: Vec<hoi4_logic::economy::construction_planner::ConstructionCandidateScore> =
            Vec::new();
        let max_add = target_queue_len.saturating_sub(current_queue_len);
        for candidate in plan.candidates {
            if added.len() >= max_add {
                break;
            }
            let si = candidate.state.0 as usize;
            if si >= queued_by_state.len() {
                continue;
            }
            let used_projected = world
                .state_building_levels(candidate.state)
                .saturating_add(queued_by_state[si]);
            if used_projected >= Self::v6_state_building_capacity(world, candidate.state) {
                continue;
            }
            let Some(def) = v6_db
                .buildings
                .iter()
                .find(|def| def.id == candidate.building_id)
            else {
                continue;
            };
            let queued_same = queued_by_building_state
                .get(&(candidate.state.0, candidate.building_id.clone()))
                .copied()
                .unwrap_or(0);
            let target_level =
                Self::v6_current_building_level(world, &candidate.building_id, candidate.state)
                    .saturating_add(queued_same)
                    .saturating_add(1);
            if target_level > def.max_level {
                continue;
            }
            let order =
                hoi4_logic::economy::BuildOrder::new(&candidate.building_id, candidate.state)
                    .with_level(target_level);
            if econ
                .enqueue_construction_checked(country, order, world, v6_db)
                .is_ok()
            {
                queued_by_state[si] = queued_by_state[si].saturating_add(1);
                *queued_by_building_state
                    .entry((candidate.state.0, candidate.building_id.clone()))
                    .or_default() += 1;
                added.push(candidate);
            }
        }
        added
    }

    fn v6_tech_category_key(category: hoi4_content::v6_loader::TechCategoryDef) -> &'static str {
        match category {
            hoi4_content::v6_loader::TechCategoryDef::Industry => "industry",
            hoi4_content::v6_loader::TechCategoryDef::Chemistry => "chemistry",
            hoi4_content::v6_loader::TechCategoryDef::Electrical => "electrical",
            hoi4_content::v6_loader::TechCategoryDef::Metallurgy => "metallurgy",
            hoi4_content::v6_loader::TechCategoryDef::MilitaryDoctrine => "military_doctrine",
            hoi4_content::v6_loader::TechCategoryDef::Aviation => "air",
            hoi4_content::v6_loader::TechCategoryDef::Naval => "naval",
            hoi4_content::v6_loader::TechCategoryDef::SocialScience => "social_science",
            hoi4_content::v6_loader::TechCategoryDef::InformationControl => "information_control",
        }
    }

    fn v6_tech_unlock_summary(
        db: &hoi4_content::V6Database,
        tech: &hoi4_content::TechDef,
    ) -> Vec<String> {
        tech.unlocks
            .iter()
            .map(|unlock| match unlock {
                hoi4_content::v6_loader::TechUnlockDef::Good(id) => {
                    format!("??? {}", Self::v6_good_name(db, id))
                }
                hoi4_content::v6_loader::TechUnlockDef::PM(id) => {
                    format!("?????? {}", Self::v6_pm_name(db, id))
                }
                hoi4_content::v6_loader::TechUnlockDef::Building(id) => {
                    format!("??? {}", Self::v6_building_name(db, id))
                }
                hoi4_content::v6_loader::TechUnlockDef::Law(cat, law_id) => {
                    let label = Self::v6_required_law_label(db, Some(&(*cat, law_id.clone())))
                        .unwrap_or_else(|| law_id.clone());
                    format!("??? {}", label)
                }
            })
            .collect()
    }

    fn v6_required_law_label(
        db: &hoi4_content::V6Database,
        requirement: Option<&(hoi4_content::v6_loader::LawCategoryDef, String)>,
    ) -> Option<String> {
        let (category, law_id) = requirement?;
        let law_name = match category {
            hoi4_content::v6_loader::LawCategoryDef::Conscription => db
                .conscription_laws
                .iter()
                .find(|law| law.id == *law_id)
                .map(|law| law.name.as_str()),
            hoi4_content::v6_loader::LawCategoryDef::Economy => db
                .economy_laws
                .iter()
                .find(|law| law.id == *law_id)
                .map(|law| law.name.as_str()),
            hoi4_content::v6_loader::LawCategoryDef::Trade => db
                .trade_laws
                .iter()
                .find(|law| law.id == *law_id)
                .map(|law| law.name.as_str()),
            hoi4_content::v6_loader::LawCategoryDef::Taxation => db
                .taxation_laws
                .iter()
                .find(|law| law.id == *law_id)
                .map(|law| law.name.as_str()),
            hoi4_content::v6_loader::LawCategoryDef::CivilRights => db
                .civil_rights_laws
                .iter()
                .find(|law| law.id == *law_id)
                .map(|law| law.name.as_str()),
            hoi4_content::v6_loader::LawCategoryDef::InformationControl => db
                .information_control_laws
                .iter()
                .find(|law| law.id == *law_id)
                .map(|law| law.name.as_str()),
        };
        Some(format!(
            "Requires law: {}",
            law_name.unwrap_or(law_id.as_str())
        ))
    }

    fn exit_construction_mode(&mut self) {
        self.construction_mode = None;
        self.construction_highlight_province_ids.clear();
    }

    fn v6_state_building_capacity(world: &World, state: hoi4_state::StateId) -> u16 {
        let si = state.0 as usize;
        if si >= world.states.count {
            return 0;
        }
        (world.states.category_slots[si] as u16).max(4) + 20
    }

    fn v6_current_building_level(
        world: &World,
        building_key: &str,
        state: hoi4_state::StateId,
    ) -> u8 {
        world
            .countries
            .buildings_v6
            .buildings
            .iter()
            .find(|building| building.state == state && building.building_def_id == building_key)
            .map(|building| building.level)
            .unwrap_or(0)
    }

    fn v6_next_construction_level(
        world: &World,
        building_key: &str,
        state: hoi4_state::StateId,
    ) -> u8 {
        Self::v6_current_building_level(world, building_key, state).saturating_add(1)
    }

    fn v6_good_name(db: &hoi4_content::V6Database, good_id: &str) -> String {
        let name_resolver = hoi4_app::ui_data::names::DisplayNameResolver::new(None);
        db.goods
            .iter()
            .find(|good| good.id == good_id)
            .map(|good| {
                name_resolver.content_name(
                    hoi4_app::ui_data::names::DisplayNameKind::Good,
                    &good.id,
                    &good.name,
                )
            })
            .unwrap_or_else(|| {
                name_resolver.content_name(
                    hoi4_app::ui_data::names::DisplayNameKind::Good,
                    good_id,
                    good_id,
                )
            })
    }

    fn localize_key(&self, key: &str) -> String {
        let localized = self.loc_catalog.tr(key);
        if localized == key {
            key.to_owned()
        } else {
            localized.to_owned()
        }
    }

    fn state_display_name(&self, state_idx: usize) -> String {
        if state_idx >= self.world.states.count {
            return hoi4_ui::i18n::tr("unknown").to_owned();
        }
        let raw = self.world.states.names[state_idx].as_str();
        hoi4_app::ui_data::names::DisplayNameResolver::new(Some(&self.loc_catalog))
            .state_name(raw, state_idx)
    }

    fn country_display_name(&self, country: hoi4_state::CountryId) -> String {
        let name_resolver = hoi4_app::ui_data::names::DisplayNameResolver::new(None);
        self.world
            .countries
            .tags
            .get(country.0 as usize)
            .map(|tag| name_resolver.country_name(tag, tag))
            .unwrap_or_else(|| hoi4_ui::i18n::tr("unknown").to_owned())
    }

    fn province_display_name(
        &self,
        province_id: u32,
        vp_name: Option<&str>,
        state_name: Option<&str>,
    ) -> String {
        hoi4_app::ui_data::names::DisplayNameResolver::new(Some(&self.loc_catalog)).province_name(
            province_id,
            vp_name,
            state_name,
        )
    }

    fn province_display_name_by_id(&self, province_id: u16) -> String {
        let vp_key = format!("VICTORY_POINTS_{}", province_id);
        let vp_loc = self.localize_key(&vp_key);
        let vp_name = (vp_loc != vp_key).then_some(vp_loc.as_str());
        let state_name = self
            .world
            .provinces
            .state_of
            .get(province_id as usize)
            .copied()
            .and_then(|state| {
                let state_idx = state.0 as usize;
                (state_idx < self.world.states.count).then(|| self.state_display_name(state_idx))
            });
        self.province_display_name(province_id as u32, vp_name, state_name.as_deref())
    }

    fn province_type_name(def: Option<&hoi4_map::ProvinceDefinition>) -> String {
        match def.map(|d| d.province_type) {
            Some(hoi4_map::ProvinceType::Land) => "陆地".to_owned(),
            Some(hoi4_map::ProvinceType::Sea) => "海域".to_owned(),
            Some(hoi4_map::ProvinceType::Lake) => "湖泊".to_owned(),
            None => hoi4_ui::i18n::tr("unknown").to_owned(),
        }
    }

    fn terrain_display_name(terrain: &str) -> String {
        match terrain {
            "plains" => "平原".to_owned(),
            "forest" => "森林".to_owned(),
            "hills" => "丘陵".to_owned(),
            "mountain" => "山地".to_owned(),
            "desert" => "沙漠".to_owned(),
            "marsh" => "沼泽".to_owned(),
            "jungle" => "丛林".to_owned(),
            "urban" => "城市".to_owned(),
            "ocean" => "海洋".to_owned(),
            "lakes" => "湖泊".to_owned(),
            "water_fjords" => "峡湾".to_owned(),
            "water_shallow_sea" => "浅海".to_owned(),
            "water_deep_ocean" => "深海".to_owned(),
            "unknown" | "" => hoi4_ui::i18n::tr("unknown").to_owned(),
            other => other.to_owned(),
        }
    }

    fn v6_pm_name(db: &hoi4_content::V6Database, pm_id: &str) -> String {
        db.production_methods
            .iter()
            .find(|pm| pm.id == pm_id)
            .map(|pm| localized_content_name(&pm.id, &pm.name))
            .unwrap_or_else(|| pm_id.to_owned())
    }

    fn v6_building_name(db: &hoi4_content::V6Database, building_id: &str) -> String {
        let name_resolver = hoi4_app::ui_data::names::DisplayNameResolver::new(None);
        db.buildings
            .iter()
            .find(|bd| bd.id == building_id)
            .map(|bd| {
                name_resolver.content_name(
                    hoi4_app::ui_data::names::DisplayNameKind::Building,
                    &bd.id,
                    &bd.name,
                )
            })
            .unwrap_or_else(|| {
                name_resolver.content_name(
                    hoi4_app::ui_data::names::DisplayNameKind::Building,
                    building_id,
                    building_id,
                )
            })
    }

    fn v6_employment_gap_for_building(
        db: &hoi4_content::V6Database,
        building: &hoi4_state::Building,
    ) -> [u32; 6] {
        let mut employment_gap = [0u32; 6];
        let pms = hoi4_content::active_pms_for_building(building, db);
        for ci in 0..6 {
            let needed_per_level: u32 = pms
                .iter()
                .map(|pm| pm.employment_demand.get(ci).copied().unwrap_or(0))
                .sum();
            let needed = needed_per_level * building.level as u32;
            let filled = building.employment.get(ci).copied().unwrap_or(0);
            employment_gap[ci] = needed.saturating_sub(filled);
        }
        employment_gap
    }

    fn update_title(&self) {
        let s = match &self.state {
            Some(s) => s,
            None => return,
        };
        let speed = match self.world.speed {
            GameSpeed::Paused => "Paused",
            GameSpeed::Speed1 => "Speed 1",
            GameSpeed::Speed2 => "Speed 2",
            GameSpeed::Speed3 => "Speed 3",
            GameSpeed::Speed4 => "Speed 4",
            GameSpeed::Speed5 => "Speed 5",
        };
        let title = format!(
            "HOI4 Rust 3D | {} | {} | {} | {} | edge/arrows pan, wheel zoom, M map mode, ESC quit",
            self.world.date,
            speed,
            self.map_mode.name(),
            self.schedule.report_systems(),
        );
        s.window.set_title(&title);
    }

    fn issue_manual_move_to_selected_divisions(&mut self, dest: hoi4_state::ProvinceId) -> bool {
        let player_cid = hoi4_state::CountryId(self.player_country as u16);
        if !hoi4_logic::military::movement::can_enter_province(&self.world, player_cid, dest) {
            return false;
        }

        let selected = self.selected_divisions.clone();
        let mut issued = false;
        for div_idx in selected {
            if div_idx >= self.world.divisions.count
                || self.world.divisions.owners[div_idx] != player_cid
            {
                continue;
            }

            let next = hoi4_logic::military::movement::next_step_toward_destination(
                &mut self.world,
                div_idx,
                dest,
            )
            .ok()
            .flatten();

            if !hoi4_logic::military::command_executor::issue_player_manual_command(
                &mut self.world,
                div_idx,
                dest,
            ) {
                continue;
            }

            self.world.divisions.destinations[div_idx] = Some(dest);
            self.world.divisions.assignments[div_idx] = None;
            issued = true;

            if let Some(next) = next {
                let cur = self.world.divisions.locations[div_idx];
                if self.world.speed != GameSpeed::Paused
                    && cur != next
                    && self.should_animate_division_step(cur, next)
                {
                    self.division_motion.insert(
                        div_idx,
                        VisualDivisionMotion {
                            from: cur,
                            to: next,
                            elapsed: 0.0,
                            duration: self.manual_move_preview_duration(),
                        },
                    );
                }
            }
        }

        if issued {
            self.cached_hoi3_counter_sig = 0;
        }
        issued
    }

    fn manual_move_preview_duration(&self) -> f32 {
        let secs_per_hour = self.world.speed.seconds_per_hour();
        if !secs_per_hour.is_finite() {
            return 0.6;
        }
        let hours_until_daily_tick = if self.world.date.hour == 0 {
            24.0
        } else {
            (24 - self.world.date.hour as u32) as f32
        };
        (secs_per_hour * hours_until_daily_tick).clamp(0.08, 5.0)
    }

    fn should_animate_division_step(
        &self,
        from: hoi4_state::ProvinceId,
        to: hoi4_state::ProvinceId,
    ) -> bool {
        let Some(state) = self.state.as_ref() else {
            return false;
        };
        let Some(&(ax, ay)) = state.unit_counter_centroids.get(from.0 as usize) else {
            return false;
        };
        let Some(&(bx, by)) = state.unit_counter_centroids.get(to.0 as usize) else {
            return false;
        };
        if (ax + ay) == 0.0 || (bx + by) == 0.0 {
            return false;
        }
        let adjacent = self
            .world
            .map
            .adjacencies
            .get(from.0 as usize)
            .is_some_and(|nbs| nbs.contains(&to.0));
        let dist = Vec2::new((bx - ax) * WORLD_SCALE, (by - ay) * WORLD_SCALE).length();
        adjacent || dist <= 1.8
    }

    fn print_perf_diag(&mut self, now: Instant) {
        if (now - self.last_perf_diag).as_secs_f32() < 1.0 {
            return;
        }
        let elapsed = (now - self.last_perf_diag).as_secs_f64().max(0.001);
        let hours_delta = self
            .world
            .elapsed_hours
            .saturating_sub(self.perf_last_hours);
        let days_per_sec = hours_delta as f64 / 24.0 / elapsed;
        let moving_divs = self
            .world
            .divisions
            .destinations
            .iter()
            .take(self.world.divisions.count)
            .filter(|d| d.is_some())
            .count();
        let wars = self.world.diplomacy.wars.len();
        let armies = self.world.player_armies.len();
        let path_cache = self.world.path_cache.len();
        let prov_div_index = self.world.prov_div_index.len();
        let counter_rebuilds = self.perf_counter_rebuilds;
        let counter_hits = self.perf_counter_cache_hits;
        let counter_instances = self.perf_counter_instances;
        let render_frames = self.perf_render_frames.max(1);
        let render_avg_ms = self.perf_render_us as f64 / render_frames as f64 / 1000.0;
        let counter_avg_ms = self.perf_counter_update_us as f64 / render_frames as f64 / 1000.0;
        let arrow_avg_ms = self.perf_arrow_update_us as f64 / render_frames as f64 / 1000.0;
        let ui_stats = self
            .state
            .as_ref()
            .map(|s| s.ui.last_stats)
            .unwrap_or_default();
        let speed = match self.world.speed {
            GameSpeed::Paused => "P",
            GameSpeed::Speed1 => "1",
            GameSpeed::Speed2 => "2",
            GameSpeed::Speed3 => "3",
            GameSpeed::Speed4 => "4",
            GameSpeed::Speed5 => "5",
        };
        println!(
            "[perf] speed={} date={} sim={:.2}d/s acc={:.3}s divs={} moving={} wars={} armies={} path={} pdi={} counters={} rebuild/s={} cache/s={} render={:.2}ms counter={:.2}ms arrows={:.2}ms ui={:.2}ms(begin={:.2} input={:.2} run={:.2} tess={:.2} buf={:.2} paint={:.2} prim={} tris={}) panels={} {}",
            speed,
            self.world.date,
            days_per_sec,
            self.time_accumulator,
            self.world.divisions.count,
            moving_divs,
            wars,
            armies,
            path_cache,
            prov_div_index,
            counter_instances,
            counter_rebuilds,
            counter_hits,
            render_avg_ms,
            counter_avg_ms,
            arrow_avg_ms,
            ui_stats.total_us as f64 / 1000.0,
            ui_stats.begin_us as f64 / 1000.0,
            ui_stats.input_us as f64 / 1000.0,
            ui_stats.run_us as f64 / 1000.0,
            ui_stats.tessellate_us as f64 / 1000.0,
            ui_stats.buffers_us as f64 / 1000.0,
            ui_stats.paint_us as f64 / 1000.0,
            ui_stats.primitive_count,
            ui_stats.triangle_count,
            self.ui_panel_cache
                .perf_report()
                .unwrap_or_else(|| "none".to_owned()),
            self.schedule.timing_report(),
        );
        self.last_perf_diag = now;
        self.perf_last_hours = self.world.elapsed_hours;
        self.perf_counter_rebuilds = 0;
        self.perf_counter_cache_hits = 0;
        self.perf_render_us = 0;
        self.perf_render_frames = 0;
        self.perf_counter_update_us = 0;
        self.perf_arrow_update_us = 0;
    }

    /// Builds data for the country info panel.
    fn build_country_info_data(
        &self,
        target: hoi4_state::CountryId,
        has_wargoal: bool,
    ) -> Option<hoi4_ui::country_info_panel::CountryInfoData> {
        let player = hoi4_state::CountryId(self.player_country as u16);
        hoi4_app::ui_data::country::build_country_info_data(
            &self.world,
            &self.historical_1936,
            &self.v6_db,
            player,
            target,
            has_wargoal,
            self.settings.instant_war,
        )
    }

    fn state_population(&self, state: hoi4_state::StateId) -> u64 {
        self.world.state_population(state)
    }

    /// Open or close an in-game panel from a single entry point.
    /// Opening any main panel hides the province info card so a stray map click
    /// cannot leave it layered on top of the new panel.
    fn toggle_in_game_panel(&mut self, panel: InGamePanel) {
        if self.open_panel == Some(panel) {
            self.close_primary_panel();
        } else {
            self.open_primary_panel(panel);
        }
    }

    fn open_primary_panel(&mut self, panel: InGamePanel) {
        self.open_panel = Some(panel);
        self.province_info_card.open = false;
        self.country_info_panel.close();
    }

    fn close_primary_panel(&mut self) {
        self.open_panel = None;
        self.active_detail_panel = None;
    }

    fn set_player_country_by_tag(&mut self, tag: &str) -> bool {
        let Some(&cid) = self.world.tag_to_country.get(tag) else {
            println!("[player] cannot switch to missing country tag: {tag}");
            return false;
        };
        let idx = cid.0 as usize;
        if idx >= self.world.countries.count {
            println!("[player] cannot switch to invalid country index for tag: {tag}");
            return false;
        }

        self.player_country = idx;
        self.world.player = cid;
        self.content.player = cid;
        self.ui_panel_cache.clear();
        self.counter_visibility_cache = CounterVisibilityCache::default();
        self.cached_hoi3_counter_sig = 0;
        self.cached_hoi3_counter_layout_sig = 0;
        self.cached_hoi3_counter_layout_offsets.clear();
        self.selected_divisions.clear();
        self.selected_army_id = None;
        self.selected_province_ids.clear();
        self.active_detail_panel = None;
        self.active_popup = None;
        self.province_info_card.open = false;
        self.country_info_panel.close();
        self.refresh_lut();
        self.rebuild_country_labels_and_refresh();
        self.update_title();
        println!("[player] switched to {tag}");
        true
    }

    fn apply_effect_report_app_requests(&mut self, report: &hoi4_content::EffectReport) {
        for tag in &report.switch_player_country {
            self.set_player_country_by_tag(tag);
        }
    }

    fn build_decision_entries(&self, player: usize) -> Vec<hoi4_ui::politics::DecisionEntry> {
        let player_id = hoi4_state::CountryId(player as u16);
        let player_tag = self
            .world
            .countries
            .tags
            .get(player)
            .map(|tag| tag.to_ascii_lowercase())
            .unwrap_or_default();
        let pp = self
            .world
            .countries
            .political_power
            .get(player)
            .copied()
            .unwrap_or(0.0);
        self.content
            .decision_db
            .decisions
            .iter()
            .filter(|d| decision_id_matches_player_tag(&d.id, &player_tag))
            .map(|d| {
                let mut e = hoi4_ui::politics::DecisionEntry::from_def(d);
                e.name = localized_content_name(&d.id, &d.name);
                let desc_key = format!("desc.{}", d.id);
                e.description = localized_content_name(&desc_key, &d.description);
                e.visible = hoi4_content::eval_trigger(
                    &d.visible,
                    &self.world,
                    player_id,
                    &self.content.global_flags,
                );
                let avail = hoi4_content::eval_trigger(
                    &d.available,
                    &self.world,
                    player_id,
                    &self.content.global_flags,
                );
                let pp_ok = pp >= d.cost_political_power;
                e.cooldown_remaining = self
                    .content
                    .decision_state
                    .cooldowns
                    .get(&d.id)
                    .copied()
                    .filter(|&v| v > 0);
                if let Some(m) = self.content.decision_state.is_active(&d.id) {
                    e.mission_remaining = Some(m.days_remaining);
                    e.mission_total = Some(m.total_days);
                }
                e.already_fired =
                    d.fire_only_once && self.content.decision_state.already_fired(&d.id);
                e.clickable = avail
                    && pp_ok
                    && e.cooldown_remaining.is_none()
                    && e.mission_remaining.is_none()
                    && !e.already_fired;
                e
            })
            .collect()
    }

    fn build_decision_mechanics(
        &self,
        player: usize,
    ) -> Vec<hoi4_ui::decisions_panel::MechanicGauge> {
        let Some(tag) = self.world.countries.tags.get(player) else {
            return Vec::new();
        };
        if tag != "SPA" {
            return Vec::new();
        }
        let vars = self
            .world
            .countries
            .variables
            .get(player)
            .cloned()
            .unwrap_or_default();
        let value = |key: &str| vars.get(key).copied().unwrap_or(0.0);
        vec![
            hoi4_ui::decisions_panel::MechanicGauge {
                label: "Label".to_owned(),
                value: value("spa_franco_authority"),
                max: 15.0,
                detail: "Authority affects nationalist command decisions.".to_owned(),
            },
            hoi4_ui::decisions_panel::MechanicGauge {
                label: "Label".to_owned(),
                value: value("spa_falange_power"),
                max: 15.0,
                detail: "Falange power tracks faction momentum.".to_owned(),
            },
            hoi4_ui::decisions_panel::MechanicGauge {
                label: "Label".to_owned(),
                value: value("spa_army_loyalty"),
                max: 15.0,
                detail: "Army loyalty affects command stability.".to_owned(),
            },
            hoi4_ui::decisions_panel::MechanicGauge {
                label: "Label".to_owned(),
                value: value("spa_church_influence"),
                max: 15.0,
                detail: "Church influence affects political support.".to_owned(),
            },
            hoi4_ui::decisions_panel::MechanicGauge {
                label: "Label".to_owned(),
                value: value("spa_carlist_anger"),
                max: 15.0,
                detail: "Carlist anger raises internal tension.".to_owned(),
            },
            hoi4_ui::decisions_panel::MechanicGauge {
                label: "Label".to_owned(),
                value: value("spa_foreign_dependency"),
                max: 15.0,
                detail: "Foreign dependency tracks outside support.".to_owned(),
            },
            hoi4_ui::decisions_panel::MechanicGauge {
                label: "Label".to_owned(),
                value: value("spa_occupation_resistance"),
                max: 15.0,
                detail: "Resistance affects occupied areas.".to_owned(),
            },
        ]
    }

    /// Reset menu hover and cached layout state when changing game phase.
    fn reset_menu_state(&mut self) {
        self.menu_kind = match self.game_phase {
            GamePhase::MainMenu => Some(MenuKind::MainMenu),
            GamePhase::CountrySelect => Some(MenuKind::CountrySelect),
            GamePhase::Playing => None,
        };
        self.menu_hovered_btn = None;
        self.menu_hovered_row = None;
        self.last_main_buttons.clear();
        self.last_country_layout = None;
    }

    /// Handle mouse clicks on menu screens.
    fn handle_menu_mouse_click(&mut self, mx: f32, my: f32) {
        match self.game_phase {
            GamePhase::MainMenu => {
                let last = self.last_main_buttons.clone();
                for btn in &last {
                    if !btn.enabled || !btn.contains(mx, my) {
                        continue;
                    }
                    match btn.id {
                        "btn_new_game" => {
                            self.game_phase = GamePhase::CountrySelect;
                            self.country_select_idx = self
                                .available_countries
                                .iter()
                                .position(|c| c.tag == "GER")
                                .unwrap_or(0);
                            self.reset_menu_state();
                        }
                        "btn_settings" => {
                            self.settings_panel.open_with(self.settings.clone());
                        }
                        "btn_quit" => std::process::exit(0),
                        _ => {}
                    }
                    break;
                }
            }
            GamePhase::CountrySelect => {
                if let Some(layout) = self.last_country_layout.take() {
                    // List row click.
                    for (rect, idx) in &layout.list_rows {
                        let (x, y, w, h) = *rect;
                        if mx >= x && mx < x + w && my >= y && my < y + h {
                            self.country_select_idx = *idx;
                            self.last_country_layout = Some(layout);
                            return;
                        }
                    }
                    if layout.back_button.contains(mx, my) {
                        self.game_phase = GamePhase::MainMenu;
                        self.reset_menu_state();
                        return;
                    }
                    if layout.start_button.contains(mx, my) {
                        if !layout.start_button.enabled {
                            println!("[menu] selected country not playable");
                        } else if let Some(entry) =
                            self.available_countries.get(self.country_select_idx)
                        {
                            let tag = entry.tag.clone();
                            self.set_player_country_by_tag(&tag);
                            self.game_phase = GamePhase::Playing;
                            self.world.speed = GameSpeed::Paused;
                            self.reset_menu_state();
                            // 3.12.15: now that a country is picked, apply
                            // the fog-of-war filter to on-map counters.
                            println!("[game] Playing as {}", tag);
                        }
                        return;
                    }
                    self.last_country_layout = Some(layout);
                }
            }
            GamePhase::Playing => {}
        }
        self.update_title();
        if let Some(s) = &self.state {
            s.window.request_redraw();
        }
    }

    fn render(&mut self) {
        let render_started = Instant::now();
        let render_quality_preset = self.effective_render_quality_preset(render_started);
        let mut profile_mark = render_started;
        let prepare_started = Instant::now();
        self.prepare_map_phase0_capture();
        let map_layer_mask = self.current_map_layer_mask();
        let map_phase0_active = self.map_phase0.is_some();
        let map_phase0_debug_lines = if map_layer_mask.asset_fallback_debug {
            self.map_phase0_debug_lines()
        } else {
            Vec::new()
        };
        let map_phase0_capture_path = if self.map_phase0_capture_ready() {
            self.map_phase0_capture_path()
        } else {
            None
        };
        let pre_frame_world_objects = WorldObjectSystem::plan(
            map_frame::MapFrameInput {
                draw_3d_map: self.game_phase == GamePhase::Playing,
                enable_3d_terrain: self.settings.enable_3d_terrain,
                map_mode: self.map_mode,
                date: self.world.date,
                selected_province_id: self.selected_province_id,
                hovered_province_id: self.hovered_province_id,
                zoom_factor: {
                    let world_extent = self.camera.world_size.x.max(self.camera.world_size.y);
                    let max_dist = world_extent * 4.0;
                    let zoom_fade_dist = max_dist * 0.7;
                    (1.0 - self.camera.distance / zoom_fade_dist).clamp(0.0, 1.0)
                },
                time_seconds: self.start_time.elapsed().as_secs_f32(),
                screen_size: self
                    .state
                    .as_ref()
                    .map(|s| [s.config.width as f32, s.config.height as f32])
                    .unwrap_or([1.0, 1.0]),
                layer_mask: map_layer_mask,
                quality_preset: render_quality_preset,
            }
            .context(),
        );
        let profile_world_plan_ms = profile_mark.elapsed().as_secs_f32() * 1000.0;
        profile_mark = Instant::now();
        let counter_started = Instant::now();
        self.update_hoi3_counter_pass(pre_frame_world_objects);
        let counter_update_ms = counter_started.elapsed().as_secs_f32() * 1000.0;
        self.perf_counter_update_us = self
            .perf_counter_update_us
            .saturating_add((counter_update_ms * 1000.0) as u64);
        let arrow_started = Instant::now();
        self.update_frontline_overlay();
        self.update_frontline_arrows();
        self.update_trade_routes_overlay();
        let arrow_update_ms = arrow_started.elapsed().as_secs_f32() * 1000.0;
        self.perf_arrow_update_us = self
            .perf_arrow_update_us
            .saturating_add((arrow_update_ms * 1000.0) as u64);
        let profile_visual_updates_ms = profile_mark.elapsed().as_secs_f32() * 1000.0;
        profile_mark = Instant::now();

        let app_ui_enabled = !map_phase0_active || map_layer_mask.ui;
        let ui_data = ui_driver::build::build_ui_data(self, app_ui_enabled);
        let ui_driver::build::UiBuildOutput {
            elapsed_secs,
            game_phase,
            player_country,
            demo_visible,
            topbar_data,
            open_panel_kind,
            active_detail_panel,
            event_badge_count,
            surrender_badge_count,
            open_panel,
            politics_data,
            decisions_panel_data,
            law_panel_data,
            pop_panel_data,
            market_panel_data,
            finance_panel_data,
            trade_panel_data,
            construction_v6_data,
            research_data,
            diplomacy_data,
            combat_bubbles,
            military_data,
            air_data,
            naval_data,
            logistics_data,
            situation_panel_data,
            player_cid,
            completed_focuses,
            current_focus_id,
            current_focus_progress,
            available_focus_ids,
            player_in_faction,
            province_info_bottom_bar_height,
            counter_rclick_prov,
            counter_menu_pos,
            settings_panel_open_cmd,
            saves_open_cmd,
            law_error_toast,
        } = ui_data;
        let current_focus_ref = current_focus_id.as_deref();
        self.province_info_card.bottom_bar_height = province_info_bottom_bar_height;
        let mut deferred_switch_player_country: Vec<String> = Vec::new();
        let mut topbar_speed_cmd: Option<hoi4_ui::topbar::SpeedCommand> = None;
        let mut side_rail_panel_cmd: Option<hoi4_ui::PanelKind> = None;
        let mut panel_commands: Vec<hoi4_ui::PanelCommand> = Vec::new();
        let mut politics_close = false;
        let mut politics_decision_cmds: Vec<hoi4_ui::politics::DecisionCommand> = Vec::new();
        let mut decisions_close = false;
        let mut decisions_cmds: Vec<hoi4_ui::politics::DecisionCommand> = Vec::new();
        let law_close = false;
        let mut law_cmds: Vec<hoi4_ui::law_panel::LawCommand> = Vec::new();
        let mut pop_panel_close = false;
        let mut market_close = false;
        let mut finance_close = false;
        let mut finance_cmds: Vec<hoi4_ui::finance_panel::FinanceCommand> = Vec::new();
        let mut trade_close = false;
        let mut construction_v6_close = false;
        let mut construction_v6_cmds: Vec<hoi4_ui::construction_v6_panel::ConstructionV6Command> =
            Vec::new();
        let mut research_close = false;
        let mut research_cmds: Vec<hoi4_ui::research::ResearchCommand> = Vec::new();
        let mut diplomacy_close = false;
        let mut diplomacy_cmds: Vec<hoi4_ui::diplomacy::DiplomacyCommand> = Vec::new();
        let mut military_close = false;
        let mut military_cmds: Vec<hoi4_ui::military::MilitaryCommand> = Vec::new();
        let mut air_close = false;
        let mut air_cmds: Vec<hoi4_ui::air::AirCommand> = Vec::new();
        let mut naval_close = false;
        let mut naval_cmds: Vec<hoi4_ui::naval::NavalCommand> = Vec::new();
        let mut logistics_close = false;
        let mut situation_close = false;
        let mut situation_cmds: Vec<hoi4_ui::situation_panel::SituationCommand> = Vec::new();
        let focus_tree = &self.content.focus_tree;
        let focus_panel = &mut self.focus_panel;
        let mut focus_cmd: Option<hoi4_ui::focus_tree_panel::FocusCommand> = None;
        let country_info_panel = &mut self.country_info_panel;
        let mut country_info_cmds: Vec<hoi4_ui::country_info_panel::CountryInfoCommand> =
            Vec::new();
        let province_info_card = &mut self.province_info_card;
        let province_info_data = &self.province_info_data;
        let mut counter_menu_cmd: Option<&'static str> = None;
        let event_scheduler = &self.content.event_scheduler;
        let event_world = &self.world;
        let event_flags = &self.content.global_flags;
        let event_country = hoi4_state::CountryId(self.player_country as u16);
        let mut event_cmd: Option<hoi4_ui::event_panel::EventCommand> = None;
        let mut surrender_notif_cmd: Option<
            hoi4_ui::surrender_notification::SurrenderNotificationCommand,
        > = None;
        if settings_panel_open_cmd && !self.settings_panel.open {
            self.settings_panel.open_with(self.settings.clone());
        }
        if saves_open_cmd && !self.save_browser.open {
            self.save_browser.open = true;
            let saves_dir = self.save_browser.saves_dir.clone();
            let entries = hoi4_ui::save_browser::scan_saves(&saves_dir, |p| {
                hoi4_state::save::read_meta(p)
                    .ok()
                    .map(|m| (format!("{}", m.date), m.player_tag))
            });
            self.save_browser.set_saves(entries);
        }
        let settings_panel = &mut self.settings_panel;
        let save_browser = &mut self.save_browser;
        let end_screen = &mut self.end_screen;
        let pending_surrender_notifications = &mut self.pending_surrender_notifications;
        self.last_law_error_toast = self.law_error_message.clone();
        let v9_notifications = &mut self.v9_notifications;
        let mut settings_close = false;
        let mut settings_cmds: Vec<hoi4_ui::settings::SettingsCommand> = Vec::new();
        let mut saves_close = false;
        let mut save_cmds: Vec<hoi4_ui::save_browser::SaveCommand> = Vec::new();
        let mut end_cmd: Option<hoi4_ui::end_screen::EndCommand> = None;
        let profile_ui_data_ms = profile_mark.elapsed().as_secs_f32() * 1000.0;
        profile_mark = Instant::now();
        let s = match self.state.as_mut() {
            Some(s) => s,
            None => return,
        };
        if let Some(profiler) = s.gpu_profiler.as_mut() {
            profiler.poll_readback(&s.device, &mut s.pass_registry);
        }
        s.pass_registry.begin_frame_stats();
        s.pass_registry
            .record_cpu_ms("hoi3_counter_v3", counter_update_ms);
        s.pass_registry
            .record_cpu_ms("3d_maparrow", arrow_update_ms);
        if let Some(profiler) = s.gpu_profiler.as_mut() {
            profiler.begin_frame();
        }
        let nine_slice = s.nine_slice_window.as_ref();
        let edge_px: f32 = nine_slice.map(|ns| ns.edges.left).unwrap_or(0.0);
        let icon_bank = &mut s.icon_bank;
        let b5_open = &mut self.b5_demo_visible;
        let demo_window = &mut self.demo_window;
        let v9_demo = &mut self.v9_demo;
        let ui_last_stats = s.ui.last_stats;
        let accessibility_settings = self.settings.accessibility();
        let mut v9_frame_profile: Option<hoi4_ui::v9::profiler::V9FrameProfile> = None;
        s.ui.begin_frame(&s.window, |ctx| {
            hoi4_ui::v9::accessibility::set_settings(ctx, accessibility_settings);
            hoi4_ui::v9::profiler::begin_frame_ctx(ctx);
            let frame = if nine_slice.is_some() {
                hoi4_ui::egui::Frame::default()
                    .inner_margin(hoi4_ui::egui::Margin::same(edge_px as i8))
            } else {
                hoi4_ui::egui::Frame::window(&ctx.style())
            };
            hoi4_ui::egui::Window::new("hoi4-ui demo")
                .open(b5_open)
                .default_pos([20.0, 80.0])
                .default_width(420.0)
                .resizable(true)
                .frame(frame)
                .show(ctx, |ui| {
                    if let Some(ns) = nine_slice {
                        let outer = ui.max_rect().expand(edge_px);
                        ns.paint(ui.painter(), outer, hoi4_ui::egui::Color32::WHITE);
                    }

                    ui.heading("Project Ironheart V5");
                    ui.label("This is a Latin paragraph rendered with Georgia (Garamond-alike).");
                    ui.label("CJK fallback font is enabled.");
                    ui.label(format!(
                        "elapsed: {elapsed_secs:.2} s ??phase: {game_phase:?} ??player_country: {player_country}"
                    ));
                    ui.separator();

                    ui.label("Vanilla focus icons (B.5):");
                    ui.horizontal(|ui| {
                        for name in &["GFX_focus_GER_anschluss", "GFX_focus_GER_afrikakorps"] {
                            ui.vertical(|ui| {
                                let resp = hoi4_ui::icons::show_icon(
                                    ui,
                                    icon_bank,
                                    name,
                                    Some(hoi4_ui::egui::vec2(80.0, 70.0)),
                                );
                                if resp.is_none() {
                                    ui.label(format!("Missing {name}"));
                                }
                                ui.label(name.trim_start_matches("GFX_focus_GER_"));
                            });
                        }
                    });
                    ui.separator();

                    ui.label("Button state preview: default, hover, and pressed.");
                    if ui.button("Test button").clicked() {
                        println!("[ui] demo button clicked");
                    }
                    if nine_slice.is_some() {
                        ui.label("Using vanilla tiled_window.dds 9-slice background.");
                    } else {
                        ui.label("9-slice not loaded; using Frame::fill fallback.");
                    }
                    ui.separator();
                    ui.label("Press F2 to toggle this UI demo.");
                });

            if demo_visible {
                demo_window.show(ctx, nine_slice, icon_bank, ui_last_stats);
            }

            // V9 frontend demo (F12)
            v9_demo.show(ctx);

            if let Some(ref data) = topbar_data {
                let (speed, _res_click) = hoi4_ui::topbar::TopBar::show(ctx, data, icon_bank);
                topbar_speed_cmd = speed;

                let badge = (event_badge_count + surrender_badge_count).min(u8::MAX as usize) as u8;
                let rail_data =
                    hoi4_ui::v9::composites::SideRailData::gameplay(open_panel_kind)
                        .with_badge(hoi4_ui::PanelKind::Situation, badge);
                side_rail_panel_cmd = hoi4_ui::v9::composites::SideRail::show(ctx, &rail_data);
            }

            if let Some(ref data) = politics_data {
                let (close, cmds) = hoi4_ui::politics::PoliticsPanel::show(ctx, data, icon_bank);
                if close {
                    politics_close = true;
                }
                politics_decision_cmds = cmds;
            }

            if let Some(ref data) = decisions_panel_data {
                let (close, cmds) = hoi4_ui::decisions_panel::DecisionsPanel::show(ctx, data);
                if close {
                    decisions_close = true;
                }
                decisions_cmds = cmds;
            }

            // Gate 5.2: laws are rendered inside the politics workbench. `law_panel_data`
            // remains available for LawDetailPanel and no longer opens an isolated V9 report.

            if let Some(ref data) = pop_panel_data {
                let (close, cmds) = hoi4_ui::pop_panel::PopPanel::show(ctx, data);
                if close {
                    pop_panel_close = true;
                }
                panel_commands.extend(cmds);
            }

            if let Some(ref data) = market_panel_data {
                let (close, cmds) = hoi4_ui::market_panel::MarketPanel::show(ctx, data);
                if close { market_close = true; }
                panel_commands.extend(cmds);
            }

            if let Some(ref data) = finance_panel_data {
                let (close, cmds) = hoi4_ui::finance_panel::FinancePanel::show(ctx, data);
                if close { finance_close = true; }
                finance_cmds = cmds;
            }

            if let Some(ref data) = trade_panel_data {
                let (close, cmds) = hoi4_ui::trade_panel::TradePanel::show(ctx, data);
                if close { trade_close = true; }
                panel_commands.extend(cmds);
            }

            if let Some(ref data) = construction_v6_data {
                let (close, cmds) = hoi4_ui::construction_v6_panel::ConstructionV6Panel::show(ctx, data);
                if close { construction_v6_close = true; }
                construction_v6_cmds = cmds;
            }

            if let Some(ref data) = research_data {
                let (close, cmds) = hoi4_ui::research::ResearchPanel::show(ctx, data);
                if close { research_close = true; }
                research_cmds = cmds;
            }

            if let Some(ref data) = diplomacy_data {
                let (close, cmds) = hoi4_ui::diplomacy::DiplomacyPanel::show(
                    ctx,
                    data,
                    &mut self.diplomacy_sort_by_opinion,
                    &mut self.diplomacy_selected_country_tag,
                    icon_bank,
                );
                if close { diplomacy_close = true; }
                diplomacy_cmds = cmds;
            }

            if open_panel == Some(InGamePanel::Military) {
                if let Some(ref data) = military_data {
                    let (close, cmds) = hoi4_ui::military::MilitaryPanel::show_side_panel(ctx, data);
                    if close { military_close = true; }
                    military_cmds = cmds;
                }
            }

            if let Some(ref data) = military_data {
                let bottom_cmds = hoi4_ui::military::MilitaryPanel::show_bottom_bar(ctx, data);
                military_cmds.extend(bottom_cmds);
                let detail_cmds = hoi4_ui::army_detail_panel::ArmyDetailPanel::show(ctx, data);
                military_cmds.extend(detail_cmds);
                let badge_cmds = hoi4_ui::army_badge::ArmyBadge::show(ctx, data);
                military_cmds.extend(badge_cmds);
            }

            if let Some(ref data) = naval_data {
                let (close, cmds) = hoi4_ui::naval::NavalPanel::show(ctx, data);
                if close { naval_close = true; }
                naval_cmds = cmds;
            }

            if let Some(ref data) = air_data {
                let (close, cmds) = hoi4_ui::air::AirPanel::show(ctx, data);
                if close { air_close = true; }
                air_cmds = cmds;
            }

            if let Some(ref data) = logistics_data {
                let (close, cmds) = hoi4_ui::logistics_panel::LogisticsPanel::show(ctx, data);
                if close {
                    logistics_close = true;
                }
                panel_commands.extend(cmds);
            }

            if let Some(ref data) = situation_panel_data {
                let (close, cmds) = hoi4_ui::situation_panel::SituationPanel::show(ctx, data);
                if close { situation_close = true; }
                situation_cmds = cmds;
            }

            if focus_tree
                .country
                .eq_ignore_ascii_case(self.world.country_tag(player_cid).unwrap_or_default())
            {
                focus_cmd = focus_panel.show(
                    ctx,
                    focus_tree,
                    &completed_focuses,
                    current_focus_ref,
                    current_focus_progress,
                    &available_focus_ids,
                );
            } else {
                focus_panel.open = false;
            }

            country_info_cmds = country_info_panel.show(ctx, icon_bank, player_in_faction);

            // J.2: Province info card.
            province_info_card.show(ctx, province_info_data);

            if let Some(cmd) = hoi4_ui::detail_panel::DetailPanelHost::show(
                ctx,
                active_detail_panel.as_ref(),
                market_panel_data.as_ref(),
                finance_panel_data.as_ref(),
                law_panel_data.as_ref(),
                construction_v6_data.as_ref(),
                pop_panel_data.as_ref(),
                diplomacy_data.as_ref(),
                Some(province_info_data),
                military_data.as_ref(),
                naval_data.as_ref(),
                air_data.as_ref(),
                logistics_data.as_ref(),
                research_data.as_ref(),
                decisions_panel_data.as_ref(),
                Some(focus_tree),
                Some(&completed_focuses),
                current_focus_ref,
                current_focus_progress,
                Some(&available_focus_ids),
            ) {
                if let Some(panel_cmd) = cmd.panel_command {
                    panel_commands.push(panel_cmd);
                }
                law_cmds.extend(cmd.law_commands);
                diplomacy_cmds.extend(cmd.diplomacy_commands);
                politics_decision_cmds.extend(cmd.decision_commands);
            }

            show_combat_bubble_overlay(ctx, &combat_bubbles, &mut self.selected_combat_bubble);

            // CR-4.5: Counter right-click menu.
            if counter_rclick_prov.is_some() {
                hoi4_ui::egui::Window::new("Counter Menu")
                    .fixed_pos([counter_menu_pos[0], counter_menu_pos[1]])
                    .collapsible(false)
                    .title_bar(false)
                    .resizable(false)
                    .show(ctx, |ui| {
                        if ui.button("Move to...").clicked() {
                            counter_menu_cmd = Some("move");
                        }
                        if ui.button("Attack...").clicked() {
                            counter_menu_cmd = Some("move");
                        }
                        if ui.button("Cancel orders").clicked() {
                            counter_menu_cmd = Some("cancel");
                        }
                        if ui.button("Disband").clicked() {
                            counter_menu_cmd = Some("disband");
                        }
                    });
            }

            let front_event_id = event_scheduler.front().map(|pending| pending.event_id.clone());
            if self.last_event_sound_id.as_deref() != front_event_id.as_deref() {
                if let Some(ref event_id) = front_event_id {
                    let popup_sound = event_scheduler
                        .db
                        .find(event_id)
                        .map(event_modal_sound)
                        .unwrap_or(UiSound::EventPopup);
                    if !self.ui_sounds.play_with_fallback(popup_sound, UiSound::EventPopup) {
                        println!("[audio] event popup sound requested but no UI sound sample is loaded");
                    }
                }
                self.last_event_sound_id = front_event_id.clone();
            }

            event_cmd = hoi4_ui::event_panel::show_event_modal_with_icons(
                ctx,
                event_scheduler,
                icon_bank,
                |id, idx| {
                    let Some(ev) = event_scheduler.db.find(id) else {
                        return false;
                    };
                    let Some(opt) = ev.options.get(idx) else {
                        return false;
                    };
                    hoi4_content::eval_trigger(
                        &opt.trigger,
                        event_world,
                        event_country,
                        event_flags,
                    )
                },
            );

            if !pending_surrender_notifications.is_empty() {
                let remaining = pending_surrender_notifications.len().saturating_sub(1);
                let current = pending_surrender_notifications.first().unwrap();
                let surrender_sound_key = surrender_notification_sound_key(current);
                if self.last_surrender_sound_key.as_deref() != Some(surrender_sound_key.as_str()) {
                    if !self
                        .ui_sounds
                        .play_with_fallback(UiSound::WorldDefeat, UiSound::EventPopup)
                    {
                        println!("[audio] surrender notification sound requested but no UI sound sample is loaded");
                    }
                    self.last_surrender_sound_key = Some(surrender_sound_key);
                }
                surrender_notif_cmd = hoi4_ui::surrender_notification::show_surrender_notification(
                    ctx, current, remaining, 0,
                );
            }

            {
                let (close, cmds) = settings_panel.show(ctx);
                if close { settings_close = true; }
                settings_cmds = cmds;
            }

            {
                let (close, cmds) = save_browser.show(ctx);
                if close { saves_close = true; }
                save_cmds = cmds;
            }

            end_cmd = end_screen.show(ctx);

            if let Some(ref err) = law_error_toast {
                v9_notifications.push_keyed(
                    "law_error",
                    "Law change failed",
                    err.clone(),
                    hoi4_ui::v9::primitives::ToastKind::Bad,
                    4.0,
                );
            }
            v9_notifications.show(ctx);
            v9_frame_profile = hoi4_ui::v9::profiler::finish_frame_ctx(ctx);
        });
        let egui_current_stats = s.ui.last_stats;
        let profile_egui_begin_ms = profile_mark.elapsed().as_secs_f32() * 1000.0;
        profile_mark = Instant::now();

        for event in hoi4_ui::v9::sound::drain(&s.ui.ctx) {
            let sound = match event.event {
                hoi4_ui::v9::sound::V9SoundEvent::Hover => UiSound::Hover,
                hoi4_ui::v9::sound::V9SoundEvent::Click => UiSound::Click,
                hoi4_ui::v9::sound::V9SoundEvent::Error => UiSound::Click,
                hoi4_ui::v9::sound::V9SoundEvent::Page => UiSound::PageFlip,
                hoi4_ui::v9::sound::V9SoundEvent::Modal => UiSound::EventPopup,
            };
            self.ui_sounds.play_with_fallback(sound, UiSound::Click);
        }
        if let Some(profile) = v9_frame_profile {
            if !profile.within_frame_budget() && self.perf_render_frames % 60 == 0 {
                println!("[v9-perf] {}", profile.report());
            }
        }

        let topbar_action = topbar_speed_cmd.map(hoi4_ui::TopbarAction::SetSpeed);

        if politics_close {
            self.open_panel = None;
            self.active_detail_panel = None;
        }

        if decisions_close {
            self.open_panel = None;
            self.active_detail_panel = None;
        }

        if law_close {
            self.open_panel = None;
            self.active_detail_panel = None;
            self.law_error_message = None;
        }

        if pop_panel_close {
            self.open_panel = None;
            self.active_detail_panel = None;
        }

        if market_close {
            self.open_panel = None;
            self.active_detail_panel = None;
        }

        if finance_close {
            self.open_panel = None;
            self.active_detail_panel = None;
        }

        if trade_close {
            self.open_panel = None;
            self.active_detail_panel = None;
        }

        if !finance_cmds.is_empty() {
            let player = self.player_country;
            for cmd in &finance_cmds {
                let content_cmd = match cmd {
                    hoi4_ui::finance_panel::FinanceCommand::IssueDomesticBond { amount_rm } => {
                        Some(hoi4_content::FinanceCommand::IssueDomesticBond {
                            amount_rm: *amount_rm,
                        })
                    }
                    hoi4_ui::finance_panel::FinanceCommand::IssueForeignBond { amount_gbp } => {
                        Some(hoi4_content::FinanceCommand::IssueForeignBond {
                            amount_gbp: *amount_gbp,
                        })
                    }
                    hoi4_ui::finance_panel::FinanceCommand::PrintMefo => {
                        Some(hoi4_content::FinanceCommand::PrintMefo)
                    }
                    hoi4_ui::finance_panel::FinanceCommand::SellGold { kg } => {
                        Some(hoi4_content::FinanceCommand::SellGold { kg: *kg })
                    }
                    hoi4_ui::finance_panel::FinanceCommand::BuyForeignCurrency { gbp_amount } => {
                        Some(hoi4_content::FinanceCommand::BuyForeignCurrency {
                            gbp_amount: *gbp_amount,
                        })
                    }
                    hoi4_ui::finance_panel::FinanceCommand::Panel(panel_cmd) => {
                        panel_commands.push(panel_cmd.clone());
                        None
                    }
                };
                if let Some(content_cmd) = content_cmd {
                    let _ = hoi4_content::execute_finance_command(
                        &mut self.world,
                        &self.v6_db,
                        player,
                        &content_cmd,
                    );
                }
            }
        }

        let mut construction_highlight_changed = false;
        if construction_v6_close {
            self.open_panel = None;
            self.active_detail_panel = None;
            if self.construction_mode.is_some() {
                self.construction_mode = None;
                self.construction_highlight_province_ids.clear();
                construction_highlight_changed = true;
            }
        }
        for cmd in construction_v6_cmds {
            if let hoi4_ui::construction_v6_panel::ConstructionV6Command::Panel(panel_cmd) = &cmd {
                panel_commands.push(panel_cmd.clone());
                continue;
            }
            let effect =
                hoi4_app::ui_data::construction_commands::apply_construction_control_command(
                    &cmd,
                    &mut self.world,
                    &mut self.econ,
                    &self.v6_db,
                    self.player_country,
                    &mut self.auto_build_enabled,
                    &mut self.construction_mode,
                    &mut self.construction_highlight_province_ids,
                );
            if effect.handled {
                if effect.reset_auto_build_month {
                    self.last_auto_build_month = None;
                }
                construction_highlight_changed |= effect.highlight_changed;
            }
        }

        if !law_cmds.is_empty() {
            let player_id = hoi4_state::CountryId(self.player_country as u16);
            for cmd in law_cmds {
                use hoi4_ui::law_panel::LawCommand;
                match cmd {
                    LawCommand::SwitchLaw {
                        category,
                        target_law_id,
                    } => {
                        match hoi4_content::set_law(
                            &mut self.world,
                            player_id,
                            category,
                            &target_law_id,
                            &self.v6_db,
                        ) {
                            Ok(()) => {
                                self.law_error_message = None;
                            }
                            Err(e) => {
                                println!(
                                    "[law] ?????????: {:?} ??{}: {}",
                                    category, target_law_id, e
                                );
                                self.law_error_message = Some(format!("?????????: {}", e));
                            }
                        }
                    }
                }
            }
        }

        politics_decision_cmds.extend(decisions_cmds);
        if !politics_decision_cmds.is_empty() {
            let player_id = hoi4_state::CountryId(self.player_country as u16);
            for cmd in politics_decision_cmds {
                use hoi4_ui::politics::DecisionCommand;
                match cmd {
                    DecisionCommand::Activate(id) => {
                        match self.content.decision_state.activate(
                            &id,
                            &self.content.decision_db,
                            &mut self.world,
                            player_id,
                            &mut self.content.global_flags,
                        ) {
                            Ok(()) => println!("[decision] activated: {id}"),
                            Err(e) => println!("[decision] activate failed: {id}: {e}"),
                        }
                    }
                    DecisionCommand::OpenFocusTree => {
                        self.focus_panel.open = true;
                    }
                    DecisionCommand::Panel(panel_cmd) => {
                        panel_commands.push(panel_cmd);
                    }
                }
            }
        }

        if construction_highlight_changed {
            let player_cid = if self.player_country < self.world.countries.count {
                Some(hoi4_state::CountryId(self.player_country as u16))
            } else {
                None
            };
            let mut color_lut = build_color_lut(&self.world, self.map_mode, player_cid);
            for pid in self
                .selected_province_ids
                .iter()
                .chain(self.construction_highlight_province_ids.iter())
            {
                let o = *pid as usize * 4;
                if o + 3 < color_lut.len() {
                    color_lut[o] = color_lut[o].saturating_add(42);
                    color_lut[o + 1] = color_lut[o + 1].saturating_add(32);
                    color_lut[o + 2] = color_lut[o + 2].saturating_sub(18);
                }
            }
            color_lut.resize((s.lut_width * s.lut_height * 4) as usize, 0);
            upload_lut(
                &s.queue,
                &s.lut_texture,
                &color_lut,
                s.lut_width,
                s.lut_height,
            );
            s.window.request_redraw();
        }

        if research_close {
            self.open_panel = None;
            self.active_detail_panel = None;
        }
        for cmd in research_cmds {
            use hoi4_ui::research::ResearchCommand;
            match cmd {
                ResearchCommand::StartResearch(tech_key) => {
                    let _ = self.research.start(
                        &self.world,
                        hoi4_state::CountryId(self.player_country as u16),
                        &tech_key,
                        &self.v6_db,
                    );
                }
                ResearchCommand::Panel(panel_cmd) => {
                    panel_commands.push(panel_cmd);
                }
            }
        }

        if diplomacy_close {
            self.open_panel = None;
            self.active_detail_panel = None;
        }
        for cmd in diplomacy_cmds {
            use hoi4_logic::diplomacy::{execute_action, DiplomaticAction};
            use hoi4_ui::diplomacy::DiplomacyCommand;
            let player_cid = hoi4_state::CountryId(self.player_country as u16);
            match cmd {
                DiplomacyCommand::JustifyWargoal(tag) => {
                    if let Some(&target) = self.world.tag_to_country.get(&tag) {
                        if let Err(err) = execute_action(
                            &mut self.world,
                            player_cid,
                            DiplomaticAction::StartJustification {
                                target,
                                kind: hoi4_state::WargoalType::Annex,
                                target_state: None,
                            },
                        ) {
                            println!("[diplomacy] Justify wargoal against {tag} failed: {err:?}");
                        }
                    }
                }
                DiplomacyCommand::DeclareWar(tag) => {
                    if let Some(&target) = self.world.tag_to_country.get(&tag) {
                        if let Err(err) = execute_action(
                            &mut self.world,
                            player_cid,
                            DiplomaticAction::DeclareWar { target },
                        ) {
                            println!("[diplomacy] Declare war on {tag} failed: {err:?}");
                        } else {
                            self.frontline_overlay_hash = 0;
                        }
                    }
                }
                DiplomacyCommand::CreateFaction => {
                    if let Err(err) = execute_action(
                        &mut self.world,
                        player_cid,
                        DiplomaticAction::CreateFaction {
                            name: "Player Faction".to_owned(),
                        },
                    ) {
                        println!("[diplomacy] Create faction failed: {err:?}");
                    }
                }
                DiplomacyCommand::LeaveFaction => {
                    if let Err(err) =
                        execute_action(&mut self.world, player_cid, DiplomaticAction::LeaveFaction)
                    {
                        println!("[diplomacy] Leave faction failed: {err:?}");
                    }
                }
                DiplomacyCommand::InviteToFaction(tag) => {
                    if let Some(&target) = self.world.tag_to_country.get(&tag) {
                        if let Err(err) = execute_action(
                            &mut self.world,
                            player_cid,
                            DiplomaticAction::InviteToFaction { target },
                        ) {
                            println!("[diplomacy] Invite {tag} to faction failed: {err:?}");
                        }
                    }
                }
                DiplomacyCommand::RequestMilitaryAccess(tag) => {
                    if let Some(&target) = self.world.tag_to_country.get(&tag) {
                        if let Err(err) = execute_action(
                            &mut self.world,
                            player_cid,
                            DiplomaticAction::RequestMilitaryAccess { target },
                        ) {
                            println!("[diplomacy] Request access from {tag} failed: {err:?}");
                        }
                    }
                }
                DiplomacyCommand::ResolvePeace {
                    war_id,
                    winning_side,
                } => {
                    let side = match winning_side {
                        hoi4_ui::diplomacy::PeaceSide::Attacker => hoi4_state::WarSide::Attacker,
                        hoi4_ui::diplomacy::PeaceSide::Defender => hoi4_state::WarSide::Defender,
                    };
                    if execute_action(
                        &mut self.world,
                        player_cid,
                        DiplomaticAction::ResolvePeace {
                            war_id,
                            winning_side: side,
                        },
                    )
                    .is_ok()
                    {
                        println!("[peace] war #{war_id} resolved through unified diplomacy action");
                        self.frontline_overlay_hash = 0;
                        let player_cid = if self.player_country < self.world.countries.count {
                            Some(hoi4_state::CountryId(self.player_country as u16))
                        } else {
                            None
                        };
                        let mut color_lut = build_color_lut(&self.world, self.map_mode, player_cid);
                        color_lut.resize((s.lut_width * s.lut_height * 4) as usize, 0);
                        upload_lut(
                            &s.queue,
                            &s.lut_texture,
                            &color_lut,
                            s.lut_width,
                            s.lut_height,
                        );
                        s.window.request_redraw();
                    }
                }
                DiplomacyCommand::Panel(panel_cmd) => {
                    panel_commands.push(panel_cmd);
                }
            }
        }

        if military_close {
            self.open_panel = None;
            self.active_detail_panel = None;
        }
        if naval_close {
            self.open_panel = None;
            self.active_detail_panel = None;
        }
        if air_close {
            self.open_panel = None;
            self.active_detail_panel = None;
        }
        if logistics_close {
            self.open_panel = None;
            self.active_detail_panel = None;
        }
        if situation_close {
            self.open_panel = None;
            self.active_detail_panel = None;
        }
        for cmd in situation_cmds {
            use hoi4_ui::situation_panel::SituationCommand;
            match cmd {
                SituationCommand::Intervene {
                    situation_id,
                    intervention_id,
                } => {
                    let player = hoi4_state::CountryId(self.player_country as u16);
                    let ci = self.player_country;
                    let stockpile = &mut self.econ.stockpile[ci];
                    self.content.situation_state.intervene(
                        &situation_id,
                        &intervention_id,
                        player,
                        &mut self.world,
                        Some(stockpile),
                    );
                }
            }
        }
        for cmd in naval_cmds {
            match cmd {
                hoi4_ui::naval::NavalCommand::SetMission { fleet_id, mission } => {
                    let fi = fleet_id as usize;
                    if fi < self.world.fleets.count
                        && self.world.fleets.owners[fi]
                            == hoi4_state::CountryId(self.player_country as u16)
                    {
                        self.world.fleets.mission[fi] = naval_mission_from_ui(mission);
                    }
                }
                hoi4_ui::naval::NavalCommand::MoveToSelectedSeaRegion { fleet_id } => {
                    let fi = fleet_id as usize;
                    if fi < self.world.fleets.count
                        && self.world.fleets.owners[fi]
                            == hoi4_state::CountryId(self.player_country as u16)
                    {
                        let selected_sea_region = if self.selected_province_id != u32::MAX {
                            self.world
                                .map
                                .definitions
                                .get(self.selected_province_id as usize)
                                .and_then(|def| def.as_ref())
                                .filter(|def| def.province_type == hoi4_map::ProvinceType::Sea)
                                .map(|def| def.id as u32)
                        } else {
                            None
                        };
                        if let Some(region) = selected_sea_region {
                            let now = self.world.date.hours_since_epoch().max(0) as u64;
                            let _ = hoi4_logic::naval::movement::order_move_to_region(
                                &mut self.world,
                                hoi4_state::FleetId(fleet_id),
                                region,
                                now,
                            );
                            self.pending_naval_move_fleet = None;
                        } else if self.pending_naval_move_fleet == Some(fleet_id) {
                            self.pending_naval_move_fleet = None;
                        } else {
                            self.pending_naval_move_fleet = Some(fleet_id);
                        }
                    }
                }
                hoi4_ui::naval::NavalCommand::SplitFleet { fleet_id, count } => {
                    Self::split_fleet_data(
                        &mut self.world,
                        hoi4_state::CountryId(self.player_country as u16),
                        fleet_id,
                        count,
                    );
                }
                hoi4_ui::naval::NavalCommand::DisbandEmptyFleet { fleet_id } => {
                    let fi = fleet_id as usize;
                    let player = hoi4_state::CountryId(self.player_country as u16);
                    if fi < self.world.fleets.count
                        && self.world.fleets.owners[fi] == player
                        && self.world.fleets.ships[fi].is_empty()
                    {
                        self.world.fleets.owners[fi] = hoi4_state::CountryId::NONE;
                        if self.naval_transfer_source_fleet == Some(fleet_id) {
                            self.naval_transfer_source_fleet = None;
                        }
                        if self.pending_naval_move_fleet == Some(fleet_id) {
                            self.pending_naval_move_fleet = None;
                        }
                    }
                }
                hoi4_ui::naval::NavalCommand::SetTransferSource { fleet_id } => {
                    let fi = fleet_id as usize;
                    if fi < self.world.fleets.count
                        && self.world.fleets.owners[fi]
                            == hoi4_state::CountryId(self.player_country as u16)
                    {
                        self.naval_transfer_source_fleet = Some(fleet_id);
                    }
                }
                hoi4_ui::naval::NavalCommand::ClearTransferSource => {
                    self.naval_transfer_source_fleet = None;
                }
                hoi4_ui::naval::NavalCommand::TransferShips {
                    from_fleet_id,
                    to_fleet_id,
                    count,
                } => {
                    Self::transfer_ships_between_fleets_data(
                        &mut self.world,
                        hoi4_state::CountryId(self.player_country as u16),
                        from_fleet_id,
                        to_fleet_id,
                        count,
                    );
                }
                hoi4_ui::naval::NavalCommand::Panel(panel_cmd) => {
                    panel_commands.push(panel_cmd);
                }
            }
        }
        for cmd in air_cmds {
            match cmd {
                hoi4_ui::air::AirCommand::SetMission { wing_id, mission } => {
                    let wi = wing_id as usize;
                    if wi < self.world.air_wings.count
                        && self.world.air_wings.owners[wi]
                            == hoi4_state::CountryId(self.player_country as u16)
                    {
                        self.world.air_wings.mission[wi] = air_mission_from_ui(mission);
                        if self.world.air_wings.target_region[wi] == u32::MAX {
                            self.world.air_wings.target_region[wi] =
                                self.world.air_wings.region_id[wi];
                        }
                    }
                }
                hoi4_ui::air::AirCommand::TransferToSelectedState { wing_id } => {
                    let wi = wing_id as usize;
                    if wi < self.world.air_wings.count
                        && self.world.air_wings.owners[wi]
                            == hoi4_state::CountryId(self.player_country as u16)
                    {
                        let selected_state = if self.selected_province_id != u32::MAX {
                            self.world
                                .provinces
                                .state_of
                                .get(self.selected_province_id as usize)
                                .copied()
                                .unwrap_or(hoi4_state::StateId::NONE)
                        } else {
                            hoi4_state::StateId::NONE
                        };
                        if !selected_state.is_none() {
                            let now = self.world.date.hours_since_epoch().max(0) as u64;
                            let _ = hoi4_logic::air::operations::order_transfer_to_base(
                                &mut self.world,
                                hoi4_state::AirWingId(wing_id),
                                selected_state,
                                selected_state.0 as u32,
                                now,
                            );
                            self.pending_air_transfer_wing = None;
                        } else if self.pending_air_transfer_wing == Some(wing_id) {
                            self.pending_air_transfer_wing = None;
                        } else {
                            self.pending_air_transfer_wing = Some(wing_id);
                        }
                    }
                }
                hoi4_ui::air::AirCommand::ToggleReinforce { wing_id } => {
                    let wi = wing_id as usize;
                    if wi < self.world.air_wings.count
                        && self.world.air_wings.owners[wi]
                            == hoi4_state::CountryId(self.player_country as u16)
                    {
                        self.world.air_wings.reinforce_enabled[wi] =
                            !self.world.air_wings.reinforce_enabled[wi];
                    }
                }
                hoi4_ui::air::AirCommand::SplitWing { wing_id, planes } => {
                    Self::split_air_wing_data(
                        &mut self.world,
                        hoi4_state::CountryId(self.player_country as u16),
                        wing_id,
                        planes,
                    );
                }
                hoi4_ui::air::AirCommand::DisbandEmptyWing { wing_id } => {
                    let wi = wing_id as usize;
                    let player = hoi4_state::CountryId(self.player_country as u16);
                    if wi < self.world.air_wings.count
                        && self.world.air_wings.owners[wi] == player
                        && self.world.air_wings.count_planes[wi] == 0
                    {
                        self.world.air_wings.owners[wi] = hoi4_state::CountryId::NONE;
                        if self.air_transfer_source_wing == Some(wing_id) {
                            self.air_transfer_source_wing = None;
                        }
                        if self.pending_air_transfer_wing == Some(wing_id) {
                            self.pending_air_transfer_wing = None;
                        }
                    }
                }
                hoi4_ui::air::AirCommand::SetTransferSource { wing_id } => {
                    let wi = wing_id as usize;
                    if wi < self.world.air_wings.count
                        && self.world.air_wings.owners[wi]
                            == hoi4_state::CountryId(self.player_country as u16)
                    {
                        self.air_transfer_source_wing = Some(wing_id);
                    }
                }
                hoi4_ui::air::AirCommand::ClearTransferSource => {
                    self.air_transfer_source_wing = None;
                }
                hoi4_ui::air::AirCommand::TransferPlanes {
                    from_wing_id,
                    to_wing_id,
                    planes,
                } => {
                    Self::transfer_planes_between_wings_data(
                        &mut self.world,
                        hoi4_state::CountryId(self.player_country as u16),
                        from_wing_id,
                        to_wing_id,
                        planes,
                    );
                }
                hoi4_ui::air::AirCommand::Panel(panel_cmd) => {
                    panel_commands.push(panel_cmd);
                }
            }
        }
        for cmd in military_cmds {
            use hoi4_ui::military::MilitaryCommand;
            match cmd {
                MilitaryCommand::Train(template_idx) => {
                    let owner = hoi4_state::CountryId(self.player_country as u16);
                    let capital_state = self
                        .world
                        .countries
                        .capitals
                        .get(self.player_country)
                        .copied()
                        .unwrap_or(hoi4_state::StateId::NONE);
                    let data = self.world.data.clone();
                    if let Err(err) = hoi4_logic::military::training::enqueue_training(
                        &self.world,
                        &mut self.econ,
                        &data,
                        owner,
                        template_idx as u32,
                        1,
                        capital_state,
                        hoi4_data::ReinforcementPriority::Normal,
                    ) {
                        eprintln!("[military] enqueue training failed: {err:?}");
                    }
                }
                MilitaryCommand::NewTemplate => {
                    let tag = self
                        .world
                        .country_tag(hoi4_state::CountryId(self.player_country as u16));
                    if let Some(tag) = tag.map(str::to_owned) {
                        let data = std::sync::Arc::make_mut(&mut self.world.data);
                        let templates = data.division_templates.entry(tag.clone()).or_default();
                        let id = templates.len() as u32;
                        let template = hoi4_data::EditableDivisionTemplate::empty(
                            id,
                            format!("New Template {}", id + 1),
                            Some(tag),
                        )
                        .to_division_template();
                        templates.push(template);
                        self.selected_template_idx = Some((templates.len() - 1) as u16);
                        self.template_editor_open = true;
                    }
                }
                MilitaryCommand::OpenTemplateEditor(template_idx) => {
                    self.selected_template_idx = Some(template_idx);
                    self.template_editor_open = true;
                }
                MilitaryCommand::CloseTemplateEditor => {
                    self.template_editor_open = false;
                }
                MilitaryCommand::OpenLineSubunitPicker { template, row, col } => {
                    self.selected_template_idx = Some(template);
                    self.template_editor_open = true;
                    self.template_picker_target =
                        Some(hoi4_ui::military::TemplatePickerTarget::Line { template, row, col });
                }
                MilitaryCommand::OpenSupportSubunitPicker { template, slot } => {
                    self.selected_template_idx = Some(template);
                    self.template_editor_open = true;
                    self.template_picker_target =
                        Some(hoi4_ui::military::TemplatePickerTarget::Support { template, slot });
                }
                MilitaryCommand::CloseSubunitPicker => {
                    self.template_picker_target = None;
                }
                MilitaryCommand::SelectTemplate(template_idx) => {
                    self.selected_template_idx = Some(template_idx);
                }
                MilitaryCommand::RenameTemplate(template_idx, name) => {
                    let tag = self
                        .world
                        .country_tag(hoi4_state::CountryId(self.player_country as u16));
                    if let Some(tag) = tag.map(str::to_owned) {
                        let data = std::sync::Arc::make_mut(&mut self.world.data);
                        if let Some(templates) = data.division_templates.get_mut(&tag) {
                            let _ = hoi4_logic::military::templates::rename_template(
                                templates,
                                template_idx,
                                name,
                            );
                        }
                    }
                }
                MilitaryCommand::CloneTemplate(template_idx) => {
                    let tag = self
                        .world
                        .country_tag(hoi4_state::CountryId(self.player_country as u16));
                    if let Some(tag) = tag.map(str::to_owned) {
                        let data = std::sync::Arc::make_mut(&mut self.world.data);
                        if let Some(templates) = data.division_templates.get_mut(&tag) {
                            if let Ok(new_idx) = hoi4_logic::military::templates::clone_template(
                                templates,
                                template_idx,
                            ) {
                                self.selected_template_idx = Some(new_idx);
                            }
                        }
                    }
                }
                MilitaryCommand::DeleteTemplate(template_idx) => {
                    let tag = self
                        .world
                        .country_tag(hoi4_state::CountryId(self.player_country as u16));
                    if let Some(tag) = tag.map(str::to_owned) {
                        let data = std::sync::Arc::make_mut(&mut self.world.data);
                        if let Some(templates) = data.division_templates.get_mut(&tag) {
                            if (template_idx as usize) < templates.len() {
                                templates.remove(template_idx as usize);
                                self.template_picker_target = None;
                                self.selected_template_idx = if templates.is_empty() {
                                    None
                                } else {
                                    Some((template_idx as usize).min(templates.len() - 1) as u16)
                                };
                            }
                        }
                    }
                }
                MilitaryCommand::AddLineBattalion(template_idx, subunit) => {
                    let tag = self
                        .world
                        .country_tag(hoi4_state::CountryId(self.player_country as u16));
                    if let Some(tag) = tag.map(str::to_owned) {
                        let data = std::sync::Arc::make_mut(&mut self.world.data);
                        if let Some(templates) = data.division_templates.get_mut(&tag) {
                            if let Some((row, col)) = first_open_line_slot(templates, template_idx)
                            {
                                let _ = hoi4_logic::military::templates::set_line_battalion(
                                    templates,
                                    template_idx,
                                    row,
                                    col,
                                    Some(subunit),
                                );
                            }
                        }
                    }
                }
                MilitaryCommand::SetLineBattalion {
                    template,
                    row,
                    col,
                    subunit,
                } => {
                    let tag = self
                        .world
                        .country_tag(hoi4_state::CountryId(self.player_country as u16));
                    if let Some(tag) = tag.map(str::to_owned) {
                        let data = std::sync::Arc::make_mut(&mut self.world.data);
                        if let Some(templates) = data.division_templates.get_mut(&tag) {
                            let _ = hoi4_logic::military::templates::set_line_battalion(
                                templates, template, row, col, subunit,
                            );
                        }
                    }
                }
                MilitaryCommand::SetSupportCompany {
                    template,
                    slot,
                    subunit,
                } => {
                    let tag = self
                        .world
                        .country_tag(hoi4_state::CountryId(self.player_country as u16));
                    if let Some(tag) = tag.map(str::to_owned) {
                        let data = std::sync::Arc::make_mut(&mut self.world.data);
                        if let Some(templates) = data.division_templates.get_mut(&tag) {
                            let _ = hoi4_logic::military::templates::set_support_company(
                                templates, template, slot, subunit,
                            );
                        }
                    }
                }
                MilitaryCommand::AddSupportCompany(template_idx, subunit) => {
                    let tag = self
                        .world
                        .country_tag(hoi4_state::CountryId(self.player_country as u16));
                    if let Some(tag) = tag.map(str::to_owned) {
                        let data = std::sync::Arc::make_mut(&mut self.world.data);
                        if let Some(templates) = data.division_templates.get_mut(&tag) {
                            if let Some(slot) = first_open_support_slot(templates, template_idx) {
                                let _ = hoi4_logic::military::templates::set_support_company(
                                    templates,
                                    template_idx,
                                    slot,
                                    Some(subunit),
                                );
                            }
                        }
                    }
                }
                MilitaryCommand::CreateArmy => {
                    let owner = hoi4_state::CountryId(self.player_country as u16);
                    let members = self.selected_divisions.clone();
                    if members.is_empty() {
                        println!("[frontline] no divisions selected to create army");
                    } else {
                        let name = format!("Army {}", self.world.player_armies.len() + 1);
                        match hoi4_logic::military::frontline::create_army(
                            &mut self.world,
                            owner,
                            members,
                            name,
                        ) {
                            Ok(id) => {
                                self.selected_army_id = Some(id);
                                self.prev_armies_hash = 0;
                                println!("[frontline] created army {}", id.raw());
                            }
                            Err(e) => println!("[frontline] create_army failed: {e}"),
                        }
                    }
                }
                MilitaryCommand::DissolveArmy(id) => {
                    let aid = hoi4_state::ArmyId(id);
                    match hoi4_logic::military::frontline::dissolve_army(&mut self.world, aid) {
                        Ok(()) => {
                            self.prev_armies_hash = 0;
                            println!("[frontline] dissolved army {id}");
                        }
                        Err(e) => println!("[frontline] dissolve_army failed: {e}"),
                    }
                }
                MilitaryCommand::AddMembers(id) => {
                    let aid = hoi4_state::ArmyId(id);
                    let members = self.selected_divisions.clone();
                    if !members.is_empty() {
                        match hoi4_logic::military::frontline::add_members(
                            &mut self.world,
                            aid,
                            &members,
                        ) {
                            Ok(()) => {
                                self.prev_armies_hash = 0;
                                println!("[frontline] added {} members to army {id}", members.len())
                            }
                            Err(e) => println!("[frontline] add_members failed: {e}"),
                        }
                    }
                }
                MilitaryCommand::RemoveMembers(id) => {
                    let aid = hoi4_state::ArmyId(id);
                    let members = self.selected_divisions.clone();
                    if !members.is_empty() {
                        match hoi4_logic::military::frontline::remove_members(
                            &mut self.world,
                            aid,
                            &members,
                        ) {
                            Ok(()) => {
                                self.prev_armies_hash = 0;
                                println!(
                                    "[frontline] removed {} members from army {id}",
                                    members.len()
                                );
                            }
                            Err(e) => println!("[frontline] remove_members failed: {e}"),
                        }
                    }
                }
                MilitaryCommand::DrawFrontline(id) => {
                    self.frontline_painter.mode = PainterMode::ArmyPainter(hoi4_state::ArmyId(id));
                    self.frontline_painter.samples.clear();
                    self.frontline_painter.last_sample_at = std::time::Instant::now();
                    println!("[frontline] entering frontline paint mode for army {id}");
                }
                MilitaryCommand::ClearFrontline(id) => {
                    let aid = hoi4_state::ArmyId(id);
                    match hoi4_logic::military::frontline::clear_frontline_path(
                        &mut self.world,
                        aid,
                    ) {
                        Ok(()) => {
                            self.prev_armies_hash = 0;
                            println!("[frontline] cleared frontline for army {id}");
                        }
                        Err(e) => println!("[frontline] clear_frontline_path failed: {e}"),
                    }
                }
                MilitaryCommand::DrawArrow(id) => {
                    let aid = hoi4_state::ArmyId(id);
                    let anchor = self
                        .world
                        .player_armies
                        .iter()
                        .find(|a| a.id == aid)
                        .and_then(|a| {
                            a.order.as_ref().and_then(|o| {
                                o.anchor
                                    .or_else(|| o.path.get(o.path.len() / 2).copied())
                                    .or_else(|| o.path.first().copied())
                            })
                        })
                        .unwrap_or(hoi4_state::ProvinceId(0));
                    self.frontline_painter.mode = PainterMode::ArrowPainter(aid, anchor);
                    self.frontline_painter.samples.clear();
                    self.frontline_painter.last_sample_at = std::time::Instant::now();
                    println!("[frontline] entering arrow paint mode for army {id}");
                }
                MilitaryCommand::ClearArrow(id) => {
                    let aid = hoi4_state::ArmyId(id);
                    match hoi4_logic::military::frontline::clear_arrow(&mut self.world, aid) {
                        Ok(()) => {
                            self.prev_armies_hash = 0;
                            println!("[frontline] cleared arrow for army {id}");
                        }
                        Err(e) => println!("[frontline] clear_arrow failed: {e}"),
                    }
                }
                MilitaryCommand::ToggleOverlay => {
                    self.frontline_overlay_visible = !self.frontline_overlay_visible;
                    self.prev_armies_hash = 0;
                    println!(
                        "[frontline] overlay visible: {}",
                        self.frontline_overlay_visible
                    );
                }
                MilitaryCommand::SelectArmy(id) => {
                    self.selected_army_id = Some(hoi4_state::ArmyId(id));
                }
                MilitaryCommand::ClearArmySelection => {
                    self.selected_army_id = None;
                }
                MilitaryCommand::AddSelectedDivisionsToArmy(id) => {
                    let aid = hoi4_state::ArmyId(id);
                    let members = self.selected_divisions.clone();
                    if !members.is_empty() {
                        match hoi4_logic::military::frontline::add_members(
                            &mut self.world,
                            aid,
                            &members,
                        ) {
                            Ok(()) => {
                                self.selected_army_id = Some(aid);
                                self.prev_armies_hash = 0;
                                println!(
                                    "[frontline] right-click added {} divisions to army {id}",
                                    members.len()
                                )
                            }
                            Err(e) => println!("[frontline] add_members failed: {e}"),
                        }
                    }
                }
                MilitaryCommand::ToggleDivisionSelection(idx) => {
                    if let Some(pos) = self.selected_divisions.iter().position(|&x| x == idx) {
                        self.selected_divisions.remove(pos);
                    } else {
                        self.selected_divisions.push(idx);
                    }
                    self.selected_army_id = None;
                }
                MilitaryCommand::SelectAllDivisions => {
                    let player = hoi4_state::CountryId(self.player_country as u16);
                    self.selected_divisions.clear();
                    for i in 0..self.world.divisions.count {
                        if self.world.divisions.owners[i] == player {
                            self.selected_divisions.push(i);
                        }
                    }
                    self.selected_army_id = None;
                }
                MilitaryCommand::Panel(panel_cmd) => {
                    panel_commands.push(panel_cmd);
                }
                MilitaryCommand::ExecutePlan(id) => {
                    let aid = hoi4_state::ArmyId(id);
                    match hoi4_logic::military::frontline::execute_plan(&mut self.world, aid) {
                        Ok(()) => {
                            hoi4_logic::military::frontline::tick_frontlines(&mut self.world);
                            self.prev_armies_hash = 0;
                            eprintln!("[frontline] executing plan for army {id}");
                        }
                        Err(e) => eprintln!("[frontline] execute_plan failed: {e}"),
                    }
                }
                MilitaryCommand::HaltPlan(id) => {
                    let aid = hoi4_state::ArmyId(id);
                    match hoi4_logic::military::frontline::halt_plan(&mut self.world, aid) {
                        Ok(()) => {
                            self.prev_armies_hash = 0;
                            eprintln!("[frontline] halted plan for army {id}");
                        }
                        Err(e) => eprintln!("[frontline] halt_plan failed: {e}"),
                    }
                }
                MilitaryCommand::AssignGeneral {
                    army_id,
                    general_id,
                } => {
                    let aid = hoi4_state::ArmyId(army_id);
                    let gid = hoi4_state::GeneralId(general_id);
                    let player = hoi4_state::CountryId(self.player_country as u16);
                    let general_ok = self
                        .world
                        .generals
                        .iter()
                        .any(|g| g.id == gid && g.owner == player);
                    let assigned_elsewhere = self
                        .world
                        .player_armies
                        .iter()
                        .any(|a| a.id != aid && a.commander == Some(gid));
                    if general_ok && !assigned_elsewhere {
                        if let Some(army) =
                            self.world.player_armies.iter_mut().find(|a| a.id == aid)
                        {
                            if army.owner == player {
                                army.commander = Some(gid);
                            }
                        }
                    }
                }
                MilitaryCommand::UnassignGeneral(id) => {
                    let aid = hoi4_state::ArmyId(id);
                    if let Some(army) = self.world.player_armies.iter_mut().find(|a| a.id == aid) {
                        army.commander = None;
                    }
                }
            }
        }

        if let Some(cmd) = focus_cmd {
            use hoi4_ui::focus_tree_panel::FocusCommand;
            let player = hoi4_state::CountryId(self.player_country as u16);
            match cmd {
                FocusCommand::Start(id) => {
                    // P0.3??tart_focus ???????available ???
                    if !hoi4_content::start_focus(
                        &mut self.world,
                        player,
                        &self.content.focus_tree,
                        &id,
                        &self.content.global_flags,
                    ) {
                        println!("[focus] skipped {}: unavailable", id);
                    }
                }
                FocusCommand::Cancel => {
                    let i = self.player_country;
                    self.world.countries.current_focus[i] = None;
                    self.world.countries.focus_progress[i] = 0.0;
                }
                FocusCommand::Panel(panel_cmd) => {
                    panel_commands.push(panel_cmd);
                }
            }
        }

        if let Some(cmd) = event_cmd {
            use hoi4_ui::event_panel::EventCommand;
            match cmd {
                EventCommand::PickOption {
                    event_id,
                    option_idx,
                } => {
                    self.ui_sounds
                        .play_with_fallback(UiSound::OptionClick, UiSound::Click);
                    println!("[event] resolved: {event_id} option={option_idx}");
                    let (_, report) = self.content.event_scheduler.resolve_option(
                        option_idx,
                        &mut self.world,
                        &mut self.content.global_flags,
                    );
                    deferred_switch_player_country.extend(report.switch_player_country);
                    let cascaded = self.content.process_pending_triggers(&mut self.world);
                    deferred_switch_player_country
                        .extend(cascaded.effect_report.switch_player_country);
                    for w in report
                        .warnings
                        .iter()
                        .chain(cascaded.effect_report.warnings.iter())
                    {
                        println!("[effect] ???: {w}");
                    }
                    for e in report
                        .errors
                        .iter()
                        .chain(cascaded.effect_report.errors.iter())
                    {
                        println!("[effect] ???: {e}");
                    }
                    for target in &cascaded.cascaded_triggers {
                        println!("[event] cascaded trigger: {target}");
                    }
                    if cascaded.pause_for_country_event {
                        if self.world.speed != GameSpeed::Paused && self.pre_event_speed.is_none() {
                            self.pre_event_speed = Some(self.world.speed);
                        }
                        self.world.speed = GameSpeed::Paused;
                    }
                    if self.content.event_scheduler.pending_len() == 0 {
                        if let Some(prev) = self.pre_event_speed.take() {
                            if self.world.speed == GameSpeed::Paused {
                                self.world.speed = prev;
                                println!("[event] queue cleared, restored speed: {:?}", prev);
                            }
                        }
                    }
                }
            }
        }

        if let Some(hoi4_ui::surrender_notification::SurrenderNotificationCommand::Acknowledge) =
            surrender_notif_cmd
        {
            self.ui_sounds
                .play_with_fallback(UiSound::OptionClick, UiSound::Click);
            self.pending_surrender_notifications.remove(0);
            if self.pending_surrender_notifications.is_empty() {
                self.last_surrender_sound_key = None;
            }
        }

        // ???????????????????????????
        if !country_info_cmds.is_empty() {
            use hoi4_logic::diplomacy::{
                debug_grant_justified_wargoal, execute_action, DiplomaticAction,
            };
            use hoi4_ui::country_info_panel::CountryInfoCommand;
            let player = hoi4_state::CountryId(self.player_country as u16);
            for cmd in country_info_cmds {
                match cmd {
                    CountryInfoCommand::JustifyWargoal { target_tag } => {
                        if let Some(&target) = self.world.tag_to_country.get(&target_tag) {
                            if let Err(err) = execute_action(
                                &mut self.world,
                                player,
                                DiplomaticAction::StartJustification {
                                    target,
                                    kind: hoi4_state::WargoalType::Annex,
                                    target_state: None,
                                },
                            ) {
                                println!(
                                    "[diplomacy] Justify wargoal against {target_tag} failed: {err:?}"
                                );
                            }
                        }
                    }
                    CountryInfoCommand::DeclareWar { target_tag } => {
                        if let Some(&target) = self.world.tag_to_country.get(&target_tag) {
                            if self.settings.instant_war {
                                debug_grant_justified_wargoal(
                                    &mut self.world,
                                    player,
                                    target,
                                    hoi4_state::WargoalType::Annex,
                                    None,
                                );
                            }
                            match execute_action(
                                &mut self.world,
                                player,
                                DiplomaticAction::DeclareWar { target },
                            ) {
                                Ok(outcome) => {
                                    println!(
                                        "[diplomacy] Declared war on {target_tag} (war_id={:?})",
                                        outcome.war_id
                                    );
                                }
                                Err(e) => {
                                    println!(
                                        "[diplomacy] Declare war on {target_tag} failed: {e:?}"
                                    );
                                }
                            }
                            self.country_info_panel.close();
                        }
                    }
                    CountryInfoCommand::InviteToFaction { target_tag } => {
                        if let Some(&target) = self.world.tag_to_country.get(&target_tag) {
                            if let Err(err) = execute_action(
                                &mut self.world,
                                player,
                                DiplomaticAction::InviteToFaction { target },
                            ) {
                                println!(
                                    "[diplomacy] Invite {target_tag} to faction failed: {err:?}"
                                );
                            }
                        }
                    }
                    CountryInfoCommand::RequestMilitaryAccess { target_tag } => {
                        if let Some(&target) = self.world.tag_to_country.get(&target_tag) {
                            if let Err(err) = execute_action(
                                &mut self.world,
                                player,
                                DiplomaticAction::RequestMilitaryAccess { target },
                            ) {
                                println!(
                                    "[diplomacy] Request access from {target_tag} failed: {err:?}"
                                );
                            }
                        }
                    }
                    CountryInfoCommand::Close => {
                        self.country_info_panel.close();
                    }
                }
            }
        }

        if settings_close {
            self.open_panel = None;
            self.active_detail_panel = None;
        }

        // CR-4.5: Handle counter right-click menu commands.
        if let Some(cmd) = counter_menu_cmd {
            match cmd {
                "move" => {
                    // Select divisions in that province, then set pending_move_command
                    if let Some(pid) = self.counter_right_click_province {
                        let player = hoi4_state::CountryId(self.player_country as u16);
                        let prov = hoi4_state::ProvinceId(pid as u16);
                        self.selected_divisions.clear();
                        for i in 0..self.world.divisions.count {
                            if self.world.divisions.owners[i] == player
                                && self.world.divisions.locations[i] == prov
                            {
                                self.selected_divisions.push(i);
                            }
                        }
                        self.pending_move_command = true;
                    }
                }
                "cancel" => {
                    if let Some(pid) = self.counter_right_click_province {
                        let player = hoi4_state::CountryId(self.player_country as u16);
                        let prov = hoi4_state::ProvinceId(pid as u16);
                        for i in 0..self.world.divisions.count {
                            if self.world.divisions.owners[i] == player
                                && self.world.divisions.locations[i] == prov
                            {
                                self.world.divisions.destinations[i] = None;
                                hoi4_logic::military::command_executor::clear_division_command(
                                    &mut self.world,
                                    i,
                                );
                                self.division_motion.remove(&i);
                            }
                        }
                    }
                    self.counter_right_click_province = None;
                }
                "disband" => {
                    if let Some(pid) = self.counter_right_click_province {
                        println!("[counter_menu] Disband requested for province {pid}");
                    }
                    self.counter_right_click_province = None;
                }
                _ => {
                    self.counter_right_click_province = None;
                }
            }
        }
        for cmd in settings_cmds {
            let language_changed = match cmd {
                hoi4_ui::settings::SettingsCommand::SetLanguage(lang) => Some(lang),
                _ => None,
            };
            ui_binding::settings::apply_command(
                &mut self.settings,
                &mut self.settings_panel,
                &mut self.music_player,
                &mut self.ui_sounds,
                &s.window,
                cmd,
            );
            if let Some(lang) = language_changed {
                self.loc_catalog = load_loc_catalog_for_language(&self.path_cfg, lang);
            }
        }

        if saves_close {
            self.open_panel = None;
            self.active_detail_panel = None;
        }
        for cmd in save_cmds {
            use hoi4_ui::save_browser::SaveCommand;
            match cmd {
                SaveCommand::Load(path) => {
                    println!("[save] loading: {}", path.display());
                    if let Err(e) = hoi4_state::save::read(&path, &mut self.world) {
                        self.save_browser.last_error = Some(format!("load failed: {e}"));
                    } else {
                        self.save_browser.open = false;
                        self.open_panel = None;
                        self.active_detail_panel = None;
                    }
                }
                SaveCommand::Delete(path) => {
                    if let Err(e) = std::fs::remove_file(&path) {
                        self.save_browser.last_error = Some(format!("delete failed: {e}"));
                    } else {
                        // Rescan.
                        let dir = self.save_browser.saves_dir.clone();
                        let entries = hoi4_ui::save_browser::scan_saves(&dir, |p| {
                            hoi4_state::save::read_meta(p)
                                .ok()
                                .map(|m| (format!("{}", m.date), m.player_tag))
                        });
                        self.save_browser.set_saves(entries);
                    }
                }
                SaveCommand::Rename { from, to } => {
                    if let Err(e) = std::fs::rename(&from, &to) {
                        self.save_browser.last_error = Some(format!("rename failed: {e}"));
                    } else {
                        let dir = self.save_browser.saves_dir.clone();
                        let entries = hoi4_ui::save_browser::scan_saves(&dir, |p| {
                            hoi4_state::save::read_meta(p)
                                .ok()
                                .map(|m| (format!("{}", m.date), m.player_tag))
                        });
                        self.save_browser.set_saves(entries);
                    }
                }
                SaveCommand::Rescan => {
                    let dir = self.save_browser.saves_dir.clone();
                    let entries = hoi4_ui::save_browser::scan_saves(&dir, |p| {
                        hoi4_state::save::read_meta(p)
                            .ok()
                            .map(|m| (format!("{}", m.date), m.player_tag))
                    });
                    self.save_browser.set_saves(entries);
                }
            }
        }

        if let Some(cmd) = end_cmd {
            use hoi4_ui::end_screen::EndCommand;
            match cmd {
                EndCommand::Continue => {
                    self.world.speed = GameSpeed::Speed3;
                }
                EndCommand::ReturnToMainMenu => {
                    self.game_phase = GamePhase::MainMenu;
                    self.menu_kind = Some(MenuKind::MainMenu);
                    self.menu_hovered_btn = None;
                    self.menu_hovered_row = None;
                    self.last_main_buttons.clear();
                    self.last_country_layout = None;
                }
                EndCommand::Quit => {
                    // ????????????????exit event_loop???????speed=Paused ??????????                    // ???????????? window_event ???????CloseRequested??                    std::process::exit(0);
                }
            }
        }

        let profile_ui_commands_ms = profile_mark.elapsed().as_secs_f32() * 1000.0;
        profile_mark = Instant::now();

        // Update render params (zoom factor, time, selected pid).
        let world_extent = self.camera.world_size.x.max(self.camera.world_size.y);
        let max_dist = world_extent * 4.0;
        let zoom_fade_dist = max_dist * 0.7;
        let zoom_factor = (1.0 - self.camera.distance / zoom_fade_dist).clamp(0.0, 1.0);
        let time = self.start_time.elapsed().as_secs_f32();
        let mut params = RenderParams::new();
        params.selected_province_id = self.selected_province_id;
        params.zoom_factor = zoom_factor;
        params.time = time;
        // 3.6.4: write screen size for screen-space vignette.
        params.screen_width = s.config.width as f32;
        params.screen_height = s.config.height as f32;
        params.vignette_strength = 0.05; // keep terrain pre-vignette below the final postprocess vignette.
                                         // 5.5: sun direction for hillshade/lighting ???use vanilla defines so
                                         // terrain lighting matches the shadow caster direction.
        let date = self.world.date;
        params.sun_dir = RenderParams::shadow_sun_dir();
        params.month_phase = RenderParams::compute_month_phase(date.month, date.day);
        params.season_snow_offset = RenderParams::compute_season_snow_offset(params.month_phase);
        // Political view must read as a political map at gameplay zooms. The
        // shader still preserves atlas/colormap detail, but this is no longer
        // limited to a far-distance-only tint.
        params.map_mode_terrain_blend = map_mode_terrain_blend_for(self.map_mode);
        // Diplomacy border lines in all modes except terrain.
        params.diplomacy_mode = match self.map_mode {
            MapMode::Terrain => 0,
            _ => 1,
        };

        let season_result = self
            .seasons
            .season_for_date(date.month as u32, date.day as u32);
        let vanilla_map_space =
            VanillaMapSpace::from_world_size([self.camera.world_size.x, self.camera.world_size.y]);

        // Phase 3.12.8 ???update TreeFullPass season params per frame.
        // Drives tree color from `map/seasons.txt` ??`world.date` so trees
        // visibly cycle spring-green ???summer-deep ???autumn-yellow ???winter
        // as the in-game date advances.
        // Phase 3.12.9 ???update border pass params per frame.
        {
            let cam_dist_norm = 1.0 - zoom_factor;
            let sel_intensity = if self.selected_province_id != u32::MAX {
                0.6
            } else {
                0.0
            };
            let enabled_mask = if self.border_debug_view == passes::BorderDebugView::Off {
                passes::BorderParams::DEFAULT_VISIBLE_MASK
            } else {
                passes::BorderParams::ALL_VISIBLE_MASK
            };
            let bp = passes::BorderParams {
                cam_distance_norm: cam_dist_norm,
                selection_intensity: sel_intensity,
                enabled_mask,
                selected_province_id: self.selected_province_id,
                debug_view: self.border_debug_view.as_shader_value(),
                screen_width: s.config.width as f32,
                screen_height: s.config.height as f32,
                camera_distance_world: self.camera.distance,
                world_size: vanilla_map_space.world_size,
                map_size_px: vanilla_map_space.map_size_px,
            };
            s.border_pass.update_params(&s.queue, &bp);
        }

        // Cull + LOD into reused CPU buckets. Upload per-LOD instance buffers
        // only when the visible chunk set actually changed.
        let terrain_bucket_stats = build_wrapped_instance_buckets_into(
            &s.chunk_grid,
            &self.camera,
            &mut s.terrain_buckets,
        );
        for lod in 0..3 {
            let bucket_sig = terrain_bucket_stats.signatures[lod];
            let bucket_count = terrain_bucket_stats.counts[lod];
            if s.terrain_bucket_signature[lod] == bucket_sig
                && s.terrain_bucket_counts[lod] == bucket_count
            {
                continue;
            }
            s.terrain_bucket_signature[lod] = bucket_sig;
            s.terrain_bucket_counts[lod] = bucket_count;
            if s.terrain_buckets[lod].is_empty() {
                continue;
            }
            // Grow buffer if needed.
            let needed = bucket_count;
            if needed > s.instance_capacity[lod] {
                s.instance_buffers[lod] = s.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("chunk_instances"),
                    size: (needed as u64) * std::mem::size_of::<ChunkInstance>() as u64,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                s.instance_capacity[lod] = needed;
            }
            s.queue.write_buffer(
                &s.instance_buffers[lod],
                0,
                bytemuck::cast_slice(&s.terrain_buckets[lod]),
            );
        }

        let profile_params_buckets_ms = profile_mark.elapsed().as_secs_f32() * 1000.0;
        profile_mark = Instant::now();

        let frame = match s.surface.get_current_texture() {
            Ok(f) => f,
            Err(_) => return,
        };
        let surface_view = frame.texture.create_view(&Default::default());
        let capture_texture = if map_phase0_active {
            Some(s.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("map_phase0_output"),
                size: wgpu::Extent3d {
                    width: s.config.width,
                    height: s.config.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: s.config.format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            }))
        } else {
            None
        };
        let capture_view = capture_texture
            .as_ref()
            .map(|texture| texture.create_view(&Default::default()));
        let output_view = capture_view.as_ref().unwrap_or(&surface_view);
        let mut enc = s.device.create_command_encoder(&Default::default());
        let profile_surface_ms = profile_mark.elapsed().as_secs_f32() * 1000.0;
        profile_mark = Instant::now();

        // Phase 3.12.3: ?????shadow_view_proj ???????ShadowPass uniform???        // ????????????????????????GlobalFrameUniform.shadow_view_proj ???????receiver ???
        let world_size = self.camera.world_size;
        let shadow_vp = s.shadow_pass.update_shadow_view_proj(
            &s.queue,
            self.camera.target,
            world_size,
            HEIGHT_SCALE,
        );

        // Update global frame uniforms shared by the render passes.
        {
            let mut gu = GlobalFrameUniform::default();
            gu.view_proj = self.camera.view_proj().to_cols_array_2d();
            let cam_eye = self.camera.eye();
            gu.cam_pos = [cam_eye.x, cam_eye.y, cam_eye.z];
            gu.vanilla_map_size_world_size = [
                vanilla_map_space.map_size_px[0],
                vanilla_map_space.map_size_px[1],
                vanilla_map_space.world_size[0],
                vanilla_map_space.world_size[1],
            ];
            let cam_map_px = vanilla_map_space.world_xz_to_map_px([cam_eye.x, cam_eye.z]);
            gu.cam_pos_map_px = [cam_map_px[0], cam_map_px[1], 0.0, 0.0];
            gu.cam_look_at_dir = {
                let d = (self.camera.target - cam_eye).normalize_or_zero();
                [d.x, d.y, d.z]
            };
            gu.global_time = time;
            gu.fow_opacity_time_snow_max_speed =
                [params.season_snow_offset.max(0.0), time, 0.0, 5.0];
            gu.day_night_hour_sun_dir = {
                let sd = RenderParams::compute_sun_dir(12, self.world.date.month);
                let hour = (self.world.date.hour as f32) / 24.0;
                [hour, sd[0], sd[1], sd[2]]
            };
            gu.screen_size = [s.config.width as f32, s.config.height as f32];
            gu.cubemap_intensity = 1.35;
            gu.sun_specular_intensity = 1.45;
            gu.sun_diffuse_intensity = [1.08, 1.05, 0.98, 1.0];
            // Phase 3.12.3: shadow caster matrix
            gu.shadow_view_proj = shadow_vp.to_cols_array_2d();
            s.global_uniform_buf.write(&s.queue, &gu);
        }
        let profile_shadow_global_ms = profile_mark.elapsed().as_secs_f32() * 1000.0;
        profile_mark = Instant::now();

        // Phase 3.12.3 ???directional shadow caster pass???????3D pass ?????????
        // ????????? pass ???PCF ????????????depth map ???????????? Playing ????????
        let draw_3d_map = self.game_phase == GamePhase::Playing;
        let mut map_frame_plan = s.map_renderer.build_frame_plan(
            map_frame::MapFrameInput {
                draw_3d_map,
                enable_3d_terrain: self.settings.enable_3d_terrain,
                map_mode: self.map_mode,
                date,
                selected_province_id: self.selected_province_id,
                hovered_province_id: self.hovered_province_id,
                zoom_factor,
                time_seconds: time,
                screen_size: [s.config.width as f32, s.config.height as f32],
                layer_mask: map_layer_mask,
                quality_preset: render_quality_preset,
            }
            .context(),
            &s.pass_registry,
        );
        if self.force_water_pass && draw_3d_map {
            map_frame_plan.draw.water = true;
            map_frame_plan.draw.water_refraction = true;
        }
        self.last_map_prepare_cpu_ms = prepare_started.elapsed().as_secs_f32() * 1000.0;
        let static_decals = map_frame_plan.static_decals;
        let semantic_overlays = map_frame_plan.semantic_overlays;
        let world_objects = map_frame_plan.world_objects;
        let prepared_map_frame = s.map_renderer.prepare_frame(
            &map_frame_plan,
            MapPrepareFrameInput {
                layer_mask: map_layer_mask,
                dedicated_water_loaded: s.water_pass.any_loaded || self.force_water_pass,
                dedicated_river_loaded: s.river_pass.any_loaded,
                dedicated_border_loaded: s.border_pass.any_loaded,
            },
        );
        let terrain_ownership = prepared_map_frame.terrain_ownership;
        let water_ownership = prepared_map_frame.water_ownership;
        let map_fallback_report = prepared_map_frame.fallback_report;
        let profile_map_prepare_ms = profile_mark.elapsed().as_secs_f32() * 1000.0;
        profile_mark = Instant::now();
        let runtime_player_country = if self.player_country < self.world.countries.count {
            Some(hoi4_state::CountryId(self.player_country as u16))
        } else {
            None
        };
        s.vanilla_targets.update_frame(
            &s.queue,
            &self.world,
            &VanillaRuntimeTargetFrameParams {
                selected_province_id: self.selected_province_id,
                hovered_province_id: semantic_overlays.hovered_province_id,
                map_mode: self.map_mode,
                player_country: runtime_player_country,
                battle_plan_opacity: semantic_overlays
                    .frontlines
                    .opacity
                    .max(semantic_overlays.arrows.opacity),
                naval_dominance_opacity: semantic_overlays.straits.opacity,
                occupation_opacity: 0.0,
                selected_opacity: semantic_overlays.selected_province_pulse.opacity,
                hover_opacity: semantic_overlays.hover_highlight.opacity,
                map_mode_overlay_opacity: semantic_overlays.map_mode_overlay.opacity,
                season_snow_offset: params.season_snow_offset,
                season_blend: season_result.season_blend,
            },
        );
        let profile_vanilla_targets_ms = profile_mark.elapsed().as_secs_f32() * 1000.0;
        profile_mark = Instant::now();

        s.hoi3_counter_pass.update_opacity(
            &s.queue,
            world_objects.counters.opacity,
            s.config.width as f32,
            s.config.height as f32,
            self.camera.view_proj().to_cols_array_2d(),
            self.start_time.elapsed().as_secs_f32(),
        );

        params.object_opacity = world_objects.trees.opacity;
        params.object_scale = world_objects.trees.scale;
        s.queue
            .write_buffer(&s.params_buffer, 0, bytemuck::bytes_of(&params));

        if let Some(tf) = s.tree_full_pass.as_mut() {
            let tf_params = passes::TreeFullParams {
                season_lerp: season_result.season_lerp,
                season_column: season_result.season_column,
                fade_start: 24.0 + 10.0 * (1.0 - world_objects.trees.opacity),
                fade_end: 38.0 + 12.0 * (1.0 - world_objects.trees.opacity),
                world_w: vanilla_map_space.world_size[0],
                world_d: vanilla_map_space.world_size[1],
                season_column_next: season_result.season_column_next,
                season_blend: season_result.season_blend,
                opacity: world_objects.trees.opacity,
                scale: world_objects.trees.scale,
                _pad0: 0.0,
                _pad1: 0.0,
            };
            let eye = self.camera.eye();
            let cam_pos = [eye.x, eye.y, eye.z];
            let last = s.tree_lod_last_cam_pos;
            let moved_sq = (cam_pos[0] - last[0]).powi(2)
                + (cam_pos[1] - last[1]).powi(2)
                + (cam_pos[2] - last[2]).powi(2);
            let should_upload_trees = !s.tree_lod_uploaded || moved_sq > 25.0;
            tf.update_params(&s.queue, &tf_params);
            if should_upload_trees {
                tf.upload_instances(&s.device, &s.queue, cam_pos);
                s.tree_lod_last_cam_pos = cam_pos;
                s.tree_lod_uploaded = true;
            }
        }

        let water_runtime_quality = if self.force_water_pass {
            MapQualityPreset::High
        } else {
            render_quality_preset
        };
        let water_refraction_available = draw_3d_map
            && map_frame_plan.draw.water_refraction
            && map_frame_plan.draw.water
            && (s.water_pass.any_loaded || self.force_water_pass)
            && (render_quality_preset.water_refraction_enabled() || self.force_water_pass);
        let selected_water_effect = s
            .water_pass
            .update_runtime_effect(water_refraction_available, water_runtime_quality);
        s.water_pass.update_params(
            &s.queue,
            &passes::WaterParams {
                world_w: vanilla_map_space.world_size[0],
                world_d: vanilla_map_space.world_size[1],
                height_scale: HEIGHT_SCALE,
                selected_province_id: self.selected_province_id,
                debug_view: self.water_debug_view.as_shader_value(),
                final_water_owner: if water_ownership.final_color { 1 } else { 0 },
                effect_variant: selected_water_effect.as_shader_value(),
                refraction_available: u32::from(water_refraction_available),
                ..passes::WaterParams::default()
            },
        );
        s.river_pass.update_params(
            &s.queue,
            &passes::RiverParams {
                world_w: vanilla_map_space.world_size[0],
                world_d: vanilla_map_space.world_size[1],
                height_scale: HEIGHT_SCALE,
                zoom_factor,
                ..passes::RiverParams::default()
            },
        );

        s.queue.write_buffer(
            &s.railways_params_buffer,
            0,
            bytemuck::bytes_of(&RailwayParams {
                alpha: static_decals.railways.opacity,
                zoom_factor,
                ..RailwayParams::default()
            }),
        );
        s.queue.write_buffer(
            &s.frontlines_params_buffer,
            0,
            bytemuck::bytes_of(&FrontlineParams {
                opacity: semantic_overlays.frontlines.opacity,
                _pad: [0.0; 3],
            }),
        );
        s.queue.write_buffer(
            &s.buildings_params_buffer,
            0,
            bytemuck::bytes_of(&BuildingParams {
                opacity: world_objects.buildings.opacity,
                scale: world_objects.buildings.scale,
                brightness: 0.86,
                _pad0: 0.0,
            }),
        );
        s.pdxmesh_pass.update_phase8_controls(
            &s.queue,
            world_objects.buildings.opacity,
            world_objects.buildings.scale,
            0.86,
            season_result
                .season_blend
                .max(params.season_snow_offset.max(0.0)),
        );
        {
            let eye = self.camera.eye();
            s.pdxmesh_pass
                .set_lod_bias(render_quality_preset.controls().object_lod_bias);
            s.pdxmesh_pass
                .ensure_lod_uploaded(&s.device, &s.queue, [eye.x, eye.y, eye.z]);
        }
        let particle_quality = render_quality_preset.controls().particle_density;
        s.particle_pass.update_params(
            &s.queue,
            80.0 / particle_quality.max(0.25),
            400.0 / particle_quality.max(0.25),
        );
        s.maparrow_pass.update_params(
            &s.queue,
            &passes::maparrow::ArrowParams {
                overlay_opacity: semantic_overlays.arrows.opacity,
                frontline_opacity: semantic_overlays.frontlines.opacity,
                ..passes::maparrow::ArrowParams::default()
            },
        );
        s.traderoute_pass.update_params(
            &s.queue,
            &passes::traderoute::TradeRouteParams {
                opacity: semantic_overlays.trade_routes.opacity,
                ..passes::traderoute::TradeRouteParams::default()
            },
        );
        s.strait_pass.update_params(
            &s.queue,
            &passes::strait::StraitParams {
                opacity: semantic_overlays.straits.opacity,
                ..passes::strait::StraitParams::default()
            },
        );
        if let Some(mnp) = s.mapname_pass.as_ref() {
            mnp.update_params(
                &s.queue,
                world_objects.country_names.opacity,
                world_objects.country_names.scale,
            );
        }
        if let Some(pnp) = s.province_name_pass.as_ref() {
            pnp.update_params(
                &s.queue,
                world_objects.province_names.opacity,
                world_objects.province_names.scale,
            );
        }
        if let Some(poi) = s.poi_icon_pass.as_ref() {
            poi.update_params(
                &s.queue,
                &passes::poi_icon::PoiIconParams {
                    opacity: world_objects.poi_icons.opacity,
                    scale: world_objects.poi_icons.scale,
                    outline_strength: 0.82,
                    _pad0: 0.0,
                },
            );
        }
        if let Some(poi) = s.poi_icon_pass.as_mut() {
            if s.poi_zoom_bucket != world_objects.poi_detail_level {
                poi.upload(
                    &s.device,
                    &s.queue,
                    &s.poi_icon_instances,
                    world_objects.poi_detail_level,
                );
                s.poi_zoom_bucket = world_objects.poi_detail_level;
            }
        }

        // Phase 3: terrain material ownership. In final-quality frames, visible
        // water and borders are owned by their dedicated passes; terrain keeps
        // only the depth/base-material fallback path.
        {
            let selected_state_id = if self.selected_province_id != u32::MAX {
                let pi = self.selected_province_id as usize;
                self.world
                    .provinces
                    .state_of
                    .get(pi)
                    .copied()
                    .filter(|sid| !sid.is_none())
                    .map(|sid| sid.0 as u32)
                    .unwrap_or(u32::MAX)
            } else {
                u32::MAX
            };
            let pdx_params = passes::PdxMapParams {
                selected_province_id: self.selected_province_id,
                selected_state_id,
                hovered_province_id: semantic_overlays.hovered_province_id,
                terrain_blend: params.map_mode_terrain_blend,
                screen_width: params.screen_width,
                screen_height: params.screen_height,
                vignette_strength: 0.0,
                zoom_factor: params.zoom_factor,
                border_country_px: if terrain_ownership.terrain_sdf_borders {
                    params.border_country_px
                } else {
                    0.0
                },
                border_province_px: if terrain_ownership.terrain_sdf_borders {
                    params.border_province_px
                } else {
                    0.0
                },
                season_lerp: season_result.season_lerp,
                map_mode_terrain_blend: params.map_mode_terrain_blend,
                world_size_xy_height_lat: [
                    vanilla_map_space.world_size[0],
                    vanilla_map_space.world_size[1],
                    HEIGHT_SCALE,
                    LAT_CORRECTION,
                ],
                season_params: [
                    season_result.season_column,
                    params.season_snow_offset,
                    season_result.season_column_next,
                    season_result.season_blend,
                ],
                terrain_controls: [
                    self.terrain_debug_view.as_shader_value(),
                    if terrain_ownership.terrain_water_final_color {
                        1.0
                    } else {
                        0.0
                    },
                    if terrain_ownership.terrain_sdf_borders {
                        1.0
                    } else {
                        0.0
                    },
                    if terrain_ownership.terrain_overlays {
                        1.0
                    } else {
                        0.0
                    },
                ],
                overlay_controls: [
                    0.0,
                    semantic_overlays.selected_province_pulse.opacity,
                    semantic_overlays.hover_highlight.opacity,
                    semantic_overlays.map_mode_overlay.opacity,
                ],
                feature_flags: [
                    0.0,
                    if terrain_ownership.terrain_overlays
                        && map_frame_plan.draw.river
                        && !s.river_pass.any_loaded
                    {
                        1.0
                    } else {
                        0.0
                    },
                    0.0,
                    0.0,
                ],
                atlas_idx_array: {
                    let src = self.world.map.terrain_catalog.atlas_idx_array_256();
                    std::array::from_fn::<[u32; 4], 64, _>(|r| {
                        std::array::from_fn::<u32, 4, _>(|c| src[r * 4 + c] as u32)
                    })
                },
                terrain_flags_array: {
                    let src = self.world.map.terrain_catalog.terrain_flags_array_256();
                    std::array::from_fn::<[u32; 4], 64, _>(|r| {
                        std::array::from_fn::<u32, 4, _>(|c| src[r * 4 + c] as u32)
                    })
                },
            };
            s.terrain_pass.update_params(&s.queue, &pdx_params);
        }
        let profile_pass_params_ms = profile_mark.elapsed().as_secs_f32() * 1000.0;
        profile_mark = Instant::now();
        let postprocess_lut_selection =
            postprocess_lut_selection_for(&self.camera, &self.world, &vanilla_map_space);
        let map_renderer = s.map_renderer.clone();
        let map_draw_output = map_renderer.render_frame(
            s,
            &mut enc,
            map_draw::MapDrawInput {
                frame_plan: &map_frame_plan,
                terrain_counts: s.terrain_bucket_counts,
                draw_3d_map,
                show_province_names: self.show_province_names,
                zoom_factor,
                postprocess_debug_view: self.postprocess_debug_view,
                postprocess_chain_enabled: render_quality_preset.controls().postprocess_chain,
                postprocess_lut_selection,
                output_view,
            },
        );
        let use_full_chain = map_draw_output.use_full_chain;
        let profile_map_draw_ms = profile_mark.elapsed().as_secs_f32() * 1000.0;
        profile_mark = Instant::now();

        // F4 ??????????????pass ?????/ ????????????DebugOverlay???I ???????text_pass ??????
        let chain_label = map_draw_output.chain_label;
        let quality_label = if render_quality_preset == self.map_quality_preset {
            self.map_quality_preset.as_str().to_string()
        } else {
            format!(
                "{}->{}",
                self.map_quality_preset.as_str(),
                render_quality_preset.as_str()
            )
        };
        let postprocess_summary = s.post_process.calibration.summary();
        let postprocess_lut_summary = s
            .post_process
            .runtime_lut_summary(postprocess_lut_selection);
        s.debug_render_overlay.refresh(
            &s.pass_registry,
            s.hdr_target.width,
            s.hdr_target.height,
            &format!(
                "{} quality={} post_debug={} {} {} terrain_debug={} water_debug={} border_debug={} overlay_budget={:.2}/{:.2}/{:.2} map_fallbacks={} degraded={}",
                chain_label,
                quality_label,
                s.post_process.debug_view.name(),
                postprocess_summary,
                postprocess_lut_summary,
                self.terrain_debug_view.name(),
                self.water_debug_view.name(),
                self.border_debug_view.name(),
                semantic_overlays.budget.passive,
                semantic_overlays.budget.active,
                semantic_overlays.budget.map_mode,
                map_fallback_report.fallback_count(),
                map_fallback_report.is_degraded()
            ),
        );
        s.debug_render_overlay.append_lines([format!(
            "water_owner={} water_force={} water_refraction={}/{:?} available={} effect={} target={}x{} memory={:.1}MiB producer=fullscreen_9tap",
            if water_ownership.final_color {
                "water_pass"
            } else {
                "terrain_fallback"
            },
            self.force_water_pass,
            s.water_refraction_target.resolution_label(),
            s.water_refraction_target.format,
            water_refraction_available,
            selected_water_effect.effect_name(),
            s.water_refraction_target.width,
            s.water_refraction_target.height,
            s.water_refraction_target.memory_bytes() as f32 / (1024.0 * 1024.0)
        )]);
        let gpu_status = s
            .gpu_profiler
            .as_ref()
            .map(|profiler| profiler.status())
            .unwrap_or(GpuProfilerStatus::UNSUPPORTED);
        let texture_memory_bytes =
            estimate_frame_texture_memory_bytes(s.config.width, s.config.height, use_full_chain);
        let phase10_lines = phase10_overlay_lines(
            Phase10OverlayInput {
                preset: render_quality_preset,
                width: s.config.width,
                height: s.config.height,
                last_frame_cpu_ms: self.last_frame_cpu_ms,
                cpu_prepare_ms: self.last_map_prepare_cpu_ms,
                texture_memory_bytes,
                gpu_status,
            },
            &s.pass_registry,
        );
        s.debug_render_overlay.append_lines(phase10_lines);

        s.panel_pass.clear();
        let dpi = s.window.scale_factor() as f32;
        let logical_sw = s.config.width as f32 / dpi.max(0.0001);
        let logical_sh = s.config.height as f32 / dpi.max(0.0001);
        s.panel_pass
            .update_screen_size(&s.queue, logical_sw, logical_sh);

        // Phase 4.2: Render different UI based on game phase.
        s.text_pass.clear();
        if !map_phase0_debug_lines.is_empty() {
            for (idx, line) in map_phase0_debug_lines.iter().enumerate() {
                let size = if idx == 0 {
                    TextSize::Heading
                } else {
                    TextSize::Body
                };
                s.text_pass.draw_text_sized(
                    line,
                    24.0,
                    36.0 + idx as f32 * 24.0,
                    TextAlign::Left,
                    logical_sw - 48.0,
                    size,
                );
            }
        }
        // Phase 4.2 (redesign): Switch between menu rendering and topbar rendering.
        if self.game_phase != GamePhase::Playing {
            let dpi = s.window.scale_factor() as f32;
            let sw = s.config.width as f32 / dpi.max(0.0001);
            let sh = s.config.height as f32 / dpi.max(0.0001);

            s.panel_pass
                .push(panel_pass::Panel::full_screen_dim(sw, sh, 0.92));

            match self.game_phase {
                GamePhase::MainMenu => {
                    let layout = menu_pass::draw_main_menu(
                        &mut s.panel_pass,
                        &mut s.text_pass,
                        sw,
                        sh,
                        self.menu_hovered_btn,
                    );
                    self.last_main_buttons = layout.buttons;
                    self.last_country_layout = None;
                }
                GamePhase::CountrySelect => {
                    let layout = menu_pass::draw_country_select(
                        &mut s.panel_pass,
                        &mut s.text_pass,
                        sw,
                        sh,
                        &self.available_countries,
                        self.country_select_idx,
                        self.menu_hovered_btn,
                        self.menu_hovered_row,
                    );
                    // ?????????
                    if let Some(entry) = self.available_countries.get(self.country_select_idx) {
                        let world_idx = self
                            .world
                            .countries
                            .tags
                            .iter()
                            .position(|t| t == &entry.tag);
                        let (mp, civ, mil, dock, stab, ws) = if let Some(i) = world_idx {
                            (
                                recruitable_manpower(&self.world, &self.v6_db, CountryId(i as u16)),
                                0u32,
                                0u32,
                                0u32,
                                self.world
                                    .countries
                                    .stability
                                    .get(i)
                                    .copied()
                                    .unwrap_or(0.5),
                                self.world
                                    .countries
                                    .war_support
                                    .get(i)
                                    .copied()
                                    .unwrap_or(0.5),
                            )
                        } else {
                            (0, 0, 0, 0, 0.5, 0.5)
                        };
                        menu_pass::draw_country_detail(
                            &mut s.text_pass,
                            layout.detail_rect,
                            &entry.name,
                            &entry.tag,
                            &entry.ideology,
                            mp,
                            civ,
                            mil,
                            dock,
                            stab,
                            ws,
                        );
                    }
                    self.last_country_layout = Some(layout);
                    self.last_main_buttons.clear();
                }
                GamePhase::Playing => unreachable!(),
            }
        } else {
            self.last_main_buttons.clear();
            self.last_country_layout = None;
        }

        // Playing phase - topbar HUD
        // ????????? Phase 3.5 / 3.10.3: Country name labels ????????????????????????????????????????????????????????????
        // When the 3D atlas baked successfully (`mapname_pass.is_some()`) the
        // 3D pass above has already drawn country names; the 2D HUD path
        // below is only a fallback for systems where no system font could
        // be found.
        if self.game_phase == GamePhase::Playing && s.mapname_pass.is_none() {
            // LOD: at low zoom (zoomed out), only show countries with many provinces;
            // at high zoom, show all.
            {
                let view_proj = self.camera.view_proj();
                let sw = s.config.width as f32;
                let sh = s.config.height as f32;
                // zoom_factor 0 = far / 1 = near. Threshold of provinces required:
                let min_provinces = if zoom_factor < 0.20 {
                    40 // very far: only big countries
                } else if zoom_factor < 0.50 {
                    15
                } else {
                    3 // near: most countries
                };
                for (idx, label_opt) in s.country_labels.iter().enumerate() {
                    let Some(label) = label_opt else { continue };
                    if label.province_count < min_provinces {
                        continue;
                    }
                    // Lift label slightly above terrain so it sits ABOVE the surface.
                    // Use a fixed height of HEIGHT_SCALE * 0.5 above heightmap origin ???                // visually appears just above the country's territory.
                    let world_pos =
                        Vec4::new(label.world_x, HEIGHT_SCALE * 0.5, label.world_z, 1.0);
                    let clip = view_proj * world_pos;
                    if clip.w <= 0.01 {
                        continue; // behind camera
                    }
                    let ndc_x = clip.x / clip.w;
                    let ndc_y = clip.y / clip.w;
                    if ndc_x < -1.2 || ndc_x > 1.2 || ndc_y < -1.2 || ndc_y > 1.2 {
                        continue; // off-screen
                    }
                    let sx = (ndc_x + 1.0) * 0.5 * sw;
                    let sy = (1.0 - ndc_y) * 0.5 * sh;
                    // Don't draw over topbar (y < TOPBAR_HEIGHT).
                    if sy < TOPBAR_HEIGHT + 4.0 {
                        continue;
                    }
                    let tag = self
                        .world
                        .countries
                        .tags
                        .get(idx)
                        .cloned()
                        .unwrap_or_default();
                    if tag.is_empty() {
                        continue;
                    }
                    // Use Heading size (28 px) for country names.
                    s.text_pass.draw_text_sized(
                        &tag,
                        sx,
                        sy,
                        TextAlign::Center,
                        160.0,
                        TextSize::Heading,
                    );
                }
            }
        } // end mapname 2D fallback

        // 4.3 Step B (2026-05-18): ?????`politics_pass::draw_politics_overlay`
        // ?????????????????????????????????????????olitical_title / ideology / focus
        // ???????vanilla `countrypoliticsview.gui` ?????textbox widget ???????        // ??????????????`crate::binding::WorldBinding::query_string(widget_name)`
        // V5 ????????1 debug overlay ???????GuiRt-removed widget ??????????????
        // CR-4.6: Off-screen indicators for selected provinces with counters off-screen.
        if self.game_phase == GamePhase::Playing
            && s.hoi3_counter_pass.enabled()
            && !self.selected_province_ids.is_empty()
        {
            let dpi = s.window.scale_factor() as f32;
            let inv_dpi = 1.0 / dpi.max(1.0);
            let lw = s.config.width as f32 * inv_dpi;
            let lh = s.config.height as f32 * inv_dpi;
            let view_proj = self.camera.view_proj();
            let time_secs = self.start_time.elapsed().as_secs_f32();
            for c in self._cached_hoi3_counter_upload.iter() {
                if (c.flags & flag_bits::IS_UNDERLAY) != 0 {
                    continue;
                }
                let pid = c.province_id() as u32;
                if !self.selected_province_ids.contains(&pid) {
                    continue;
                }
                let Some(screen_pos) = project_counter_screen_pos(
                    c,
                    &view_proj,
                    s.config.width as f32,
                    s.config.height as f32,
                    time_secs,
                ) else {
                    continue;
                };
                let cx = (screen_pos[0] + c.size[0] * 0.5) * inv_dpi;
                let cy = (screen_pos[1] + c.size[1] * 0.5) * inv_dpi;
                if cx >= 0.0 && cx <= lw && cy >= 0.0 && cy <= lh {
                    continue;
                }
                // Off-screen: clamp to edge and draw arrow
                let ex = cx.clamp(16.0, lw - 16.0);
                let ey = cy.clamp(16.0, lh - 16.0);
                let arrow = if cx < 0.0 {
                    "<"
                } else if cx > lw {
                    ">"
                } else if cy < 0.0 {
                    "^"
                } else {
                    "v"
                };
                s.text_pass
                    .draw_text_aligned(arrow, ex, ey, TextAlign::Center, 32.0);
            }
        }

        // Phase 3.12.1: F4 ?????????????????????ass ?????/ HDR ?????/ ????????????
        if s.debug_render_overlay.enabled {
            let mut y = 8.0;
            for line in s.debug_render_overlay.latest_lines() {
                s.text_pass.draw_text(line, 8.0, y);
                y += 16.0;
            }
        }

        if self.game_phase == GamePhase::Playing {
            let view_proj = self.camera.view_proj();
            let dpi = s.window.scale_factor() as f32;
            let sw = s.config.width as f32 / dpi.max(0.0001);
            let sh = s.config.height as f32 / dpi.max(0.0001);
            let player_cid = hoi4_state::CountryId(self.player_country as u16);
            let centroids = &s.unit_counter_centroids;
            for army in &self.world.player_armies {
                if army.owner != player_cid && !self.settings.show_all_units {
                    continue;
                }
                if army.owner != player_cid && self.settings.hide_ai_frontlines {
                    continue;
                }
                if let Some(ref order) = army.order {
                    if order.path.is_empty() {
                        continue;
                    }
                    let mut sum_wx = 0.0f32;
                    let mut sum_wz = 0.0f32;
                    let mut count = 0u32;
                    for &pid in &order.path {
                        let (cx, cy) = centroids.get(pid.0 as usize).copied().unwrap_or((0.0, 0.0));
                        if cx == 0.0 && cy == 0.0 {
                            continue;
                        }
                        sum_wx += cx * WORLD_SCALE;
                        sum_wz += cy * WORLD_SCALE;
                        count += 1;
                    }
                    if count == 0 {
                        continue;
                    }
                    let avg_x = sum_wx / count as f32;
                    let avg_z = sum_wz / count as f32;
                    let world_y = 0.3f32;
                    let clip = view_proj * glam::Vec4::new(avg_x, world_y, avg_z, 1.0);
                    if clip.w <= 0.0 {
                        continue;
                    }
                    let ndc_x = clip.x / clip.w;
                    let ndc_y = clip.y / clip.w;
                    let screen_x = (ndc_x * 0.5 + 0.5) * sw;
                    let screen_y = (1.0 - (ndc_y * 0.5 + 0.5)) * sh;
                    if screen_x < -200.0
                        || screen_x > sw + 200.0
                        || screen_y < -50.0
                        || screen_y > sh + 50.0
                    {
                        continue;
                    }
                    let label = format!("{} - {} units", army.name, army.members.len());
                    let label_w = (label.chars().count() as f32 * 9.0 + 16.0).clamp(80.0, 200.0);
                    let label_h = 20.0;
                    let cy = screen_y - 14.0;
                    s.panel_pass.push(panel_pass::Panel {
                        x: screen_x - label_w * 0.5,
                        y: cy - label_h * 0.5,
                        w: label_w,
                        h: label_h,
                        radius: 4.0,
                        fill_top: [0.10, 0.07, 0.04, 0.85],
                        fill_bottom: [0.06, 0.04, 0.02, 0.85],
                        border: [0.79, 0.65, 0.36, 0.7],
                        border_width: 1.0,
                        shadow_offset: 1.0,
                        shadow_alpha: 0.35,
                    });
                    s.text_pass.draw_text_sized(
                        &label,
                        screen_x,
                        cy - 2.0,
                        TextAlign::Center,
                        label_w,
                        TextSize::Body,
                    );
                }
            }
        }

        if self.game_phase == GamePhase::Playing && self.selection_box.active {
            let [x, y, w, h] = self.selection_box.rect();
            if w > 1.0 && h > 1.0 {
                s.panel_pass.push(panel_pass::Panel {
                    x,
                    y,
                    w,
                    h,
                    radius: 0.0,
                    fill_top: [0.20, 0.45, 0.95, 0.16],
                    fill_bottom: [0.20, 0.45, 0.95, 0.10],
                    border: [0.65, 0.85, 1.0, 0.85],
                    border_width: 1.0,
                    shadow_offset: 0.0,
                    shadow_alpha: 0.0,
                });
            }
        }

        let profile_hud_build_ms = profile_mark.elapsed().as_secs_f32() * 1000.0;
        profile_mark = Instant::now();
        let ui_started = Instant::now();
        s.text_pass.prepare(&s.queue);
        s.panel_pass.prepare(&s.queue);
        // Legacy UI command rendering is removed; panel/text/flag passes remain.
        let dpi = s.window.scale_factor() as f32;
        let logical_sw = s.config.width as f32 / dpi.max(0.0001);
        let logical_sh = s.config.height as f32 / dpi.max(0.0001);
        s.queue.write_buffer(
            &s.flag_uniform_buffer,
            0,
            bytemuck::bytes_of(&[logical_sw, logical_sh]),
        );
        let flag_to_draw: Option<(String, String, [f32; 4])> = match self.game_phase {
            GamePhase::CountrySelect => self.last_country_layout.as_ref().and_then(|layout| {
                self.available_countries
                    .get(self.country_select_idx)
                    .map(|entry| {
                        let (fx, fy, fw, fh) = layout.flag_rect;
                        (entry.tag.clone(), entry.ideology.clone(), [fx, fy, fw, fh])
                    })
            }),
            GamePhase::Playing | GamePhase::MainMenu => None,
        };

        if let Some((_, _, [fx, fy, fw, fh])) = &flag_to_draw {
            let v: [[f32; 4]; 6] = [
                [*fx, *fy, 0.0, 0.0],
                [*fx + *fw, *fy, 1.0, 0.0],
                [*fx, *fy + *fh, 0.0, 1.0],
                [*fx + *fw, *fy, 1.0, 0.0],
                [*fx + *fw, *fy + *fh, 1.0, 1.0],
                [*fx, *fy + *fh, 0.0, 1.0],
            ];
            s.queue
                .write_buffer(&s.flag_vertex_buffer, 0, bytemuck::cast_slice(&v));
        }

        let ui_gpu_token = s
            .gpu_profiler
            .as_mut()
            .and_then(|profiler| profiler.begin_encoder_span(&mut enc, "ui"));
        {
            let mut ui_rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("ui_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: output_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load, // preserve 3D content
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });
            s.panel_pass.render(&mut ui_rp);

            // 4.1.bis.10 fix-fix (2026-05-16): the country flag draws AFTER
            // ui_pass ???vanilla's `GFX_shield_medium` is the metallic shield
            // frame and our renderer blits it as a single opaque quad, so
            // drawing the flag underneath would just be hidden. The correct
            // approximation (until we wire up `maskedflag.lua` masked composite)
            // is: paint the flag on top, sized to fill the shield frame.
            if let Some((tag, ideology, _)) = &flag_to_draw {
                let flag_bg = s.flag_bank.get_or_load(
                    &s.device,
                    &s.queue,
                    &self.path_cfg,
                    &s.flag_bgl,
                    &s.flag_uniform_buffer,
                    tag,
                    ideology,
                );
                ui_rp.set_pipeline(&s.flag_pipeline);
                ui_rp.set_bind_group(0, flag_bg, &[]);
                ui_rp.set_vertex_buffer(0, s.flag_vertex_buffer.slice(..));
                ui_rp.draw(0..6, 0..1);
            }

            s.text_pass.render(&mut ui_rp);
        }

        let ui_cbufs = s.ui.paint(
            &s.device,
            &s.queue,
            &mut enc,
            &s.window,
            output_view,
            [s.config.width, s.config.height],
        );
        if let (Some(profiler), Some(token)) = (s.gpu_profiler.as_mut(), ui_gpu_token) {
            profiler.end_encoder_span(&mut enc, token);
        }
        s.pass_registry
            .record_cpu_ms("ui", ui_started.elapsed().as_secs_f32() * 1000.0);
        s.pass_registry.record_draw_calls("ui", 3);
        let profile_ui_render_ms = profile_mark.elapsed().as_secs_f32() * 1000.0;
        profile_mark = Instant::now();

        let pending_readback = if let (Some(texture), Some(path)) =
            (capture_texture.as_ref(), map_phase0_capture_path)
        {
            Some(enqueue_png_readback(
                &s.device,
                &mut enc,
                texture,
                s.config.format,
                s.config.width,
                s.config.height,
                path,
            ))
        } else {
            None
        };

        if let Some(profiler) = s.gpu_profiler.as_mut() {
            profiler.finish_frame(&mut enc);
        }

        let profile_pre_submit_ms = profile_mark.elapsed().as_secs_f32() * 1000.0;
        profile_mark = Instant::now();
        s.queue
            .submit(ui_cbufs.into_iter().chain(std::iter::once(enc.finish())));
        let profile_submit_ms = profile_mark.elapsed().as_secs_f32() * 1000.0;
        if let Some(profiler) = s.gpu_profiler.as_mut() {
            profiler.map_pending_readback();
        }
        let frame_submit_us = render_started.elapsed().as_micros() as u64;
        let frame_time_ms = frame_submit_us as f32 / 1000.0;
        let present_started = Instant::now();
        frame.present();
        let profile_present_ms = present_started.elapsed().as_secs_f32() * 1000.0;
        self.last_redraw_at = Instant::now();
        self.last_frame_cpu_ms = frame_time_ms;
        if (frame_time_ms >= 25.0 || profile_present_ms >= 10.0)
            && self.last_render_profile_log.elapsed().as_millis() >= 250
        {
            let speed = match self.world.speed {
                GameSpeed::Paused => "P",
                GameSpeed::Speed1 => "1",
                GameSpeed::Speed2 => "2",
                GameSpeed::Speed3 => "3",
                GameSpeed::Speed4 => "4",
                GameSpeed::Speed5 => "5",
            };
            println!(
                "[render-prof] speed={} quality={} chain={} date={} frame={:.2}ms present={:.2}ms world={:.2}ms visual={:.2}ms ui_data={:.2}ms egui_begin={:.2}ms egui_input={:.2}ms egui_run={:.2}ms ui_cmd={:.2}ms params_buckets={:.2}ms surface={:.2}ms shadow_global={:.2}ms map_prepare={:.2}ms vanilla_targets={:.2}ms pass_params={:.2}ms map_draw={:.2}ms hud_build={:.2}ms ui_render={:.2}ms pre_submit={:.2}ms submit={:.2}ms",
                speed,
                quality_label,
                chain_label,
                self.world.date,
                frame_time_ms,
                profile_present_ms,
                profile_world_plan_ms,
                profile_visual_updates_ms,
                profile_ui_data_ms,
                profile_egui_begin_ms,
                egui_current_stats.input_us as f32 / 1000.0,
                egui_current_stats.run_us as f32 / 1000.0,
                profile_ui_commands_ms,
                profile_params_buckets_ms,
                profile_surface_ms,
                profile_shadow_global_ms,
                profile_map_prepare_ms,
                profile_vanilla_targets_ms,
                profile_pass_params_ms,
                profile_map_draw_ms,
                profile_hud_build_ms,
                profile_ui_render_ms,
                profile_pre_submit_ms,
                profile_submit_ms,
            );
            self.last_render_profile_log = Instant::now();
        }
        let capture_result =
            pending_readback.map(|pending| finish_png_readback(&s.device, pending));
        self.record_edge_pan_test_frame(
            frame_time_ms,
            profile_surface_ms,
            profile_present_ms,
            profile_egui_begin_ms,
            profile_vanilla_targets_ms,
        );
        if let Some(action) = topbar_action {
            ui_binding::apply_topbar_action(self, action);
        }
        if let Some(kind) = side_rail_panel_cmd {
            let command = if open_panel_kind == Some(kind) {
                hoi4_ui::PanelCommand::ClosePrimary
            } else {
                hoi4_ui::PanelCommand::OpenPrimary(hoi4_ui::ActivePrimaryPanel::from_panel_kind(
                    kind,
                ))
            };
            panel_commands.push(command);
        }
        if !panel_commands.is_empty() {
            ui_binding::apply_panel_commands(self, panel_commands);
        }
        for tag in deferred_switch_player_country {
            self.set_player_country_by_tag(&tag);
        }
        if let Some(result) = capture_result {
            self.map_phase0_after_capture(frame_time_ms, result);
        } else if map_phase0_active {
            self.map_phase0_after_uncaptured_frame();
        }
        self.perf_render_us = self.perf_render_us.saturating_add(frame_submit_us);
        self.perf_render_frames = self.perf_render_frames.saturating_add(1);
    }
}

fn add_manpower_to_pops(world: &mut World, country: CountryId, amount: u64) {
    let state_ids = world.country_state_ids(country);
    let indices = world
        .countries
        .pops
        .pops_by_class_in_country_mut(PopClass::Soldier, &state_ids);
    let mut remaining = amount;
    for &idx in &indices {
        if remaining == 0 {
            break;
        }
        let pg = &mut world.countries.pops.groups[idx];
        if pg.employed_at.is_none() {
            let add = remaining.min(u32::MAX as u64) as u32;
            pg.size = pg.size.saturating_add(add);
            remaining = remaining.saturating_sub(add as u64);
        }
    }
    if remaining > 0 {
        if let Some(&sid) = state_ids.first() {
            world.countries.pops.groups.push(hoi4_state::PopGroup {
                class: PopClass::Soldier,
                state: sid,
                size: remaining.min(u32::MAX as u64) as u32,
                employed_at: None,
                wage_rm: 0.0,
                tax_burden: 0.0,
                income_rm: 0.0,
                tax_paid_rm: 0.0,
                disposable_income_rm: 0.0,
                basic_consumption_budget: 0.0,
                satisfaction_law_modifier: 0.0,
                loyalty_coefficient: 1.0,
                loyalty_decay_mult: 1.0,
                satisfaction: 1.0,
                political_loyalty: 0.5,
                literacy: PopClass::Soldier.baseline_literacy(),
                skilled_ratio: PopClass::Soldier.baseline_skilled_ratio(),
                standard_of_living: 0.5,
                needs_fulfillment: 1.0,
                essential_needs_fulfillment: 1.0,
                normal_needs_fulfillment: 1.0,
                luxury_needs_fulfillment: 1.0,
                radicalism: 0.0,
            });
        }
    }
}

fn recruitable_manpower(world: &World, db: &hoi4_content::V6Database, country: CountryId) -> u64 {
    let policy = hoi4_logic::military::manpower::conscription_policy(world, db, country);
    hoi4_logic::military::manpower::recruitable_manpower_breakdown(world, country, policy).total
}

fn subtract_manpower_from_pops(world: &mut World, country: CountryId, amount: u64) {
    let state_ids = world.country_state_ids(country);
    let indices = world
        .countries
        .pops
        .pops_by_class_in_country_mut(PopClass::Soldier, &state_ids);
    let mut remaining = amount;
    for &idx in &indices {
        if remaining == 0 {
            break;
        }
        let pg = &mut world.countries.pops.groups[idx];
        if pg.employed_at.is_none() {
            let sub = remaining.min(pg.size as u64);
            pg.size = pg.size.saturating_sub(sub as u32);
            remaining = remaining.saturating_sub(sub);
        }
    }
}

fn load_loc_catalog_for_language(
    path_cfg: &PathConfig,
    lang: hoi4_ui::i18n::Language,
) -> hoi4_ui::loc::LocCatalog {
    let loc_dir = match lang {
        hoi4_ui::i18n::Language::Chinese => "simp_chinese",
        hoi4_ui::i18n::Language::English => "english",
    };
    let mut loc_catalog = hoi4_ui::loc::LocCatalog::load_from_dir(
        &path_cfg.game_path().join("localisation").join(loc_dir),
    );
    if loc_catalog.is_empty() && loc_dir != "english" {
        loc_catalog = hoi4_ui::loc::LocCatalog::load_from_dir(
            &path_cfg.game_path().join("localisation").join("english"),
        );
    }
    loc_catalog
}

/// Build the country selection list.
fn build_country_select_list(world: &World) -> Vec<CountryEntry> {
    let major_tags: &[&str] = &[
        "GER", "SPR", "ITA", "JAP", "ENG", "FRA", "USA", "SOV", "CHI",
    ];

    let mut entries = Vec::new();
    for tag in major_tags {
        let idx = world.countries.tags.iter().position(|t| t == *tag);
        let (name, ideology) = if let Some(i) = idx {
            let ideo = world
                .countries
                .ruling_party
                .get(i)
                .cloned()
                .unwrap_or_default();
            (hoi4_ui::i18n::tr(tag).to_string(), ideo)
        } else {
            continue;
        };

        entries.push(CountryEntry {
            tag: tag.to_string(),
            name,
            ideology,
            enabled: matches!(*tag, "GER" | "SPR"),
        });
    }
    entries
}

fn main() {
    let cli = bootstrap::parse_cli();
    if cli.help_requested {
        bootstrap::print_usage();
        return;
    }
    if let Some((project, reference)) = &cli.map_image_diff {
        match map_image_diff::write_diff_report(project, reference, &cli.map_image_diff_output) {
            Ok(metrics) => {
                println!("[map-image-diff] {}", metrics.summary());
                println!(
                    "[map-image-diff] wrote {}",
                    cli.map_image_diff_output.display()
                );
            }
            Err(err) => {
                eprintln!("[map-image-diff] failed: {err}");
                std::process::exit(1);
            }
        }
        return;
    }
    let path_cfg = bootstrap::resolve_path_config(&cli);

    if cli.map_audit {
        match map_baseline::write_map_audit(&path_cfg, &cli.map_audit_output) {
            Ok(path) => println!("[map-audit] wrote {}", path.display()),
            Err(err) => {
                eprintln!("[map-audit] failed: {err}");
                std::process::exit(1);
            }
        }
        return;
    }

    if cli.map_phase0_report_only {
        let output_dir = map_baseline::phase0_batch_output_dir(&cli.map_phase0_output);
        match map_baseline::write_phase0_report(
            &path_cfg,
            &output_dir,
            cli.map_phase0_reference_root.as_deref(),
        ) {
            Ok(path) => println!("[map-phase0] wrote {}", path.display()),
            Err(err) => {
                eprintln!("[map-phase0] failed: {err}");
                std::process::exit(1);
            }
        }
        return;
    }

    let world = bootstrap::load_world(&path_cfg);

    if cli.headless {
        bootstrap::run_headless(world, cli.headless_days);
        return;
    }

    if cli.map_phase0 {
        let output_dir = map_baseline::phase0_batch_output_dir(&cli.map_phase0_output);
        app_shell::run_map_phase0(
            world,
            path_cfg,
            output_dir,
            cli.map_phase0_reference_root.clone(),
        );
        return;
    }

    let edge_pan_test = if cli.edge_pan_test {
        Some(EdgePanTestConfig {
            country_tag: cli.edge_pan_test_country.clone(),
            duration_secs: cli.edge_pan_test_seconds,
        })
    } else {
        None
    };
    app_shell::run_windowed(world, path_cfg, edge_pan_test);
}

#[cfg(test)]
mod v6_app_tests {
    use super::MIN_FRAGMENT_SAMPLED_TEXTURES_FOR_PARITY;

    #[test]
    fn phase2_baseline_layers_select_expected_terrain_debug_views() {
        let cases = [
            (
                super::map_baseline::MapBaselineLayer::ProvinceSecondaryDebug,
                super::passes::TerrainDebugView::ProvinceSecondary,
            ),
            (
                super::map_baseline::MapBaselineLayer::GradientBorderCh3Debug,
                super::passes::TerrainDebugView::GradientBorderCh3,
            ),
            (
                super::map_baseline::MapBaselineLayer::TerrainRiverMaskDebug,
                super::passes::TerrainDebugView::RiverMask,
            ),
            (
                super::map_baseline::MapBaselineLayer::FowVisibilityDebug,
                super::passes::TerrainDebugView::FowVisibility,
            ),
            (
                super::map_baseline::MapBaselineLayer::TerrainFinalBeforePostprocessDebug,
                super::passes::TerrainDebugView::FinalBeforePostprocess,
            ),
            (
                super::map_baseline::MapBaselineLayer::TerrainOnly,
                super::passes::TerrainDebugView::Off,
            ),
        ];

        for (layer, expected) in cases {
            assert_eq!(
                super::terrain_debug_view_for_baseline_layer(layer),
                expected
            );
        }
    }

    #[test]
    fn political_mode_keeps_country_color_in_final_terrain_path() {
        let political = super::map_mode_terrain_blend_for(super::MapMode::Political);
        let terrain = super::map_mode_terrain_blend_for(super::MapMode::Terrain);
        let infrastructure = super::map_mode_terrain_blend_for(super::MapMode::Infrastructure);

        assert!(political < infrastructure);
        assert!(infrastructure < terrain);
        assert!(political <= 0.35, "political mode must not be terrain-led");
        assert!(terrain >= 0.80, "terrain mode should stay terrain-led");
    }

    #[test]
    fn economy_law_tiers_include_corporatist_war_economy() {
        let db = hoi4_content::V6Database::load();
        let tiers = super::build_law_tiers(hoi4_state::LawCategory::Economy, &db);

        assert!(
            tiers
                .iter()
                .any(|tier| tier.id == "corporatist_war_economy"),
            "Economy law panel tiers should include corporatist_war_economy"
        );
    }

    #[test]
    fn parity_required_limits_raise_sampled_texture_budget_for_water() {
        let mut adapter_limits = wgpu::Limits::default();
        adapter_limits.max_sampled_textures_per_shader_stage = 64;

        let requested = super::parity_required_limits(adapter_limits);

        assert!(
            requested.max_sampled_textures_per_shader_stage
                >= MIN_FRAGMENT_SAMPLED_TEXTURES_FOR_PARITY
        );
    }

    #[test]
    fn parity_required_limits_do_not_exceed_adapter_sampled_texture_budget() {
        let mut adapter_limits = wgpu::Limits::default();
        adapter_limits.max_sampled_textures_per_shader_stage = 24;

        let requested = super::parity_required_limits(adapter_limits.clone());

        assert_eq!(
            requested.max_sampled_textures_per_shader_stage,
            adapter_limits.max_sampled_textures_per_shader_stage
        );
    }
}
