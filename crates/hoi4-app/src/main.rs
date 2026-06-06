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
use hoi4_render::buildings::{PoiIconInstance, generate_buildings, generate_poi_icons};
use hoi4_render::camera::{Camera, CameraUniform, RenderParams};
use hoi4_render::counter_layout::{
    HitRegion, LayoutCounter, build_hit_regions, hit_test, layout_screen_space,
};
use hoi4_render::counter_v3::{
    CounterMotionOverride, Hoi3CounterInstance, flag_bits, generate_hoi3_counters_cr3,
    project_counter_anchor_screen, project_counter_screen_pos,
};
use hoi4_render::defines::VanillaMapSpace;
use hoi4_render::frontlines::{FrontVertex, generate_frontline_vertices};
use hoi4_render::map_mode::{MapMode, build_color_lut, color_lut_entry};
use hoi4_render::railways::{
    RailVertex, RailwayParams, build_railway_vertices_with_bridges, compute_province_centroids,
    parse_railways,
};
use hoi4_render::sdf::{compute_coast_sdf, compute_country_sdf, compute_province_sdf};
use hoi4_render::terrain::{
    ChunkGrid, ChunkInstance, LOD_GRID, build_wrapped_instance_buckets_into,
};
use hoi4_render::trees::generate_trees_with_stats;
use passes::PoiIconPass;
use passes::counter_v3::Hoi3CounterPass;

use hoi4_logic::economy::EconomyState;
use hoi4_logic::politics::PoliticsCache;
use hoi4_logic::research::ResearchState;
use hoi4_runtime::{AiState, ScriptState, SystemSchedule, init_simulation};

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
mod input;
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
mod render_frame;
mod render_init;
mod render_state;
mod simulation;
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
use edge_pan_test::{EdgePanTestConfig, EdgePanTestRun};
use flag_bank::FlagBank;
use gpu_readback::{enqueue_png_readback, finish_png_readback};
use gpu_utils::{
    MIN_FRAGMENT_SAMPLED_TEXTURES_FOR_PARITY, make_depth_view, parity_required_limits,
};
use hoi4_app::ui_data::cache::{UiPanelCache, UiPanelCacheKind};
use hoi4_app::ui_data::names::localized_content_name;
pub use hoi4_app::vanilla_resource_views;
pub use hoi4_app::vanilla_targets;
use hoi4_render::global_uniform::GlobalFrameUniform;
use map_interaction::{FrontlinePainterState, PainterMode, SelectionBoxState};
use map_perf::{
    GpuProfilerStatus, GpuTimestampProfiler, MapQualityPreset, Phase10OverlayInput,
    estimate_frame_texture_memory_bytes, phase10_overlay_lines,
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
    ColorCubeSource, DebugOverlay, GlobalUniformBuffer, HDR_FORMAT, HdrTarget, PassRegistry,
    PostProcessChain, PostProcessDebugView, PostProcessMode, SimpleBlitPass, TerrainPass,
    WaterRefractionPass, WaterRefractionTarget,
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
        self.render_pipeline();
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
