// Phase 0.2: Entry point (moved from hoi4-render). Render module is now a library.
use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use glam::{Vec2, Vec4};
use wgpu::util::DeviceExt;
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
use hoi4_render::counter_v3::{flag_bits, generate_hoi3_counters_cr3, Hoi3CounterInstance};
use hoi4_render::frontlines::{generate_frontline_vertices, FrontVertex};
use hoi4_render::map_mode::{build_color_lut, build_occupation_lut, MapMode};
use hoi4_render::railways::{
    build_railway_vertices, compute_province_centroids, parse_railways, RailVertex, RailwayParams,
};
use hoi4_render::sdf::{compute_coast_sdf, compute_country_sdf, compute_province_sdf};
use hoi4_render::terrain::{
    build_wrapped_instance_buckets, vertex_count_for_lod, ChunkGrid, ChunkInstance, LOD_GRID,
};
use hoi4_render::trees::{generate_trees, TreeInstance};
use hoi4_render::trees_mesh::{
    build_tree_mesh, filter_instances_for_type, TreeMeshData, TreeMeshInstance, TreeMeshVertex,
};
use passes::counter_v3::Hoi3CounterPass;
use passes::PoiIconPass;

use hoi4_logic::economy::EconomyState;
use hoi4_logic::politics::PoliticsCache;
use hoi4_logic::research::ResearchState;
use hoi4_runtime::{init_simulation, AiState, ScriptState, SystemSchedule};

mod text_pass;
use text_pass::{TextAlign, TextPass, TextSize};

mod glyphon_text;

mod app_shell;
mod binding;
mod bootstrap;
mod content_bootstrap;
mod debug_commands;
mod flag_bank;
mod map_baseline;
mod map_perf;
mod map_renderer;
mod mapname_atlas;
mod menu_pass;
mod menu_scene;
mod panel_pass;
mod passes;
mod province_name_atlas;
mod render_collect;
mod runtime;
mod ui_binding;
mod update_loop;
use flag_bank::FlagBank;
use hoi4_render::global_uniform::GlobalFrameUniform;
use map_perf::{
    estimate_frame_texture_memory_bytes, phase10_overlay_lines, GpuProfilerStatus,
    GpuTimestampProfiler, MapQualityPreset, Phase10OverlayInput,
};
use map_renderer::{
    MapFrameContext, MapRenderSettings, MapRenderer, TerrainMaterialOwnership, WorldObjectPlan,
    WorldObjectSystem,
};
use menu_pass::{CountryEntry, MenuButton};
use menu_scene::MenuKind;
use panel_pass::PanelPass;
use passes::{
    DebugOverlay, GlobalUniformBuffer, HdrTarget, PassRegistry, PostProcessChain,
    PostProcessDebugView, PostProcessMode, SimpleBlitPass, TerrainPass, HDR_FORMAT,
};

/// World units per heightmap pixel (XZ). Smaller = "denser" world.
const WORLD_SCALE: f32 = 0.02;
/// World Y for full-white heightmap pixel (255 -> this height).
/// HOI4 vanilla heightmap goes 0..255; ~95 = sea level, mountains ~180-220.
/// With WORLD_SCALE=0.02 the world is 112脳41 units, so we want a height_scale
/// that gives readable relief without making the strategic map look spiky.
const HEIGHT_SCALE: f32 = 1.45;
/// Latitude squash factor (0..1). 0=no correction.
/// HOI4's source map already uses a partial projection correction; adding our
/// own parabolic squash on top distorts shapes more than it helps, so default
/// to off. Keep the plumbing for experimentation.
const LAT_CORRECTION: f32 = 0.0;
/// Number of chunks across the map (X 脳 Z).
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

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
enum UiPanelCacheKind {
    Finance,
    Market,
    Construction,
    Diplomacy,
    Military,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
struct UiPanelCacheKey {
    player: usize,
    day: i64,
    signature: u64,
    selected_tag: Option<String>,
}

impl UiPanelCacheKey {
    fn new(player: usize, day: i64, signature: u64) -> Self {
        Self {
            player,
            day,
            signature,
            selected_tag: None,
        }
    }

    fn with_selected_tag(mut self, selected_tag: Option<String>) -> Self {
        self.selected_tag = selected_tag;
        self
    }
}

#[derive(Default)]
struct UiPanelCache {
    finance: Option<(UiPanelCacheKey, hoi4_ui::finance_panel::FinancePanelData)>,
    market: Option<(UiPanelCacheKey, hoi4_ui::market_panel::MarketPanelData)>,
    construction: Option<(
        UiPanelCacheKey,
        hoi4_ui::construction_v6_panel::ConstructionV6PanelData,
    )>,
    diplomacy: Option<(UiPanelCacheKey, hoi4_ui::diplomacy::DiplomacyData)>,
    last_builds: Vec<UiPanelBuildPerf>,
}

struct UiPanelBuildPerf {
    kind: UiPanelCacheKind,
    elapsed: Duration,
    reused: bool,
}

struct MapPhase0Run {
    output_dir: PathBuf,
    started: Instant,
    captures: Vec<map_baseline::MapBaselinePlannedCapture>,
    scenes: Vec<map_baseline::MapBaselineScene>,
    asset_audit: hoi4_assets::MapAssetAudit,
    capture_index: usize,
    settle_frames: u8,
    finished: bool,
}

impl MapPhase0Run {
    fn new(path_cfg: &PathConfig, output_dir: PathBuf) -> Self {
        let asset_audit = map_baseline::build_asset_audit(path_cfg);
        let scenes = map_baseline::fixed_scenes();
        let captures = map_baseline::build_phase0_planned_captures(&scenes, &asset_audit);
        Self {
            output_dir,
            started: Instant::now(),
            captures,
            scenes,
            asset_audit,
            capture_index: 0,
            settle_frames: 2,
            finished: false,
        }
    }

    fn current_capture(&self) -> Option<&map_baseline::MapBaselinePlannedCapture> {
        self.captures.get(self.capture_index)
    }
}

struct PendingPngReadback {
    buffer: wgpu::Buffer,
    path: PathBuf,
    width: u32,
    height: u32,
    unpadded_bytes_per_row: u32,
    padded_bytes_per_row: u32,
    format: wgpu::TextureFormat,
}

impl UiPanelCache {
    fn clear(&mut self) {
        self.finance = None;
        self.market = None;
        self.construction = None;
        self.diplomacy = None;
    }

    fn begin_frame(&mut self) {
        self.last_builds.clear();
    }

    fn record(&mut self, kind: UiPanelCacheKind, elapsed: Duration, reused: bool) {
        self.last_builds.push(UiPanelBuildPerf {
            kind,
            elapsed,
            reused,
        });
    }

    fn perf_report(&self) -> Option<String> {
        if self.last_builds.is_empty() {
            return None;
        }
        Some(
            self.last_builds
                .iter()
                .map(|entry| {
                    let state = if entry.reused { "hit" } else { "build" };
                    format!(
                        "{:?}:{}:{:.2}ms",
                        entry.kind,
                        state,
                        entry.elapsed.as_secs_f64() * 1000.0
                    )
                })
                .collect::<Vec<_>>()
                .join(" | "),
        )
    }
}

fn localized_content_name(id: &str, fallback: &str) -> String {
    let translated = hoi4_ui::i18n::tr(id);
    if translated != id {
        translated.to_owned()
    } else if fallback.is_empty() {
        id.to_owned()
    } else {
        fallback.to_owned()
    }
}

fn build_law_tiers(
    cat: hoi4_state::LawCategory,
    db: &hoi4_content::V6Database,
) -> Vec<hoi4_ui::law_panel::LawTierEntry> {
    use hoi4_content::v6_loader::*;
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
                    format!("兵源转化 {:.1}%/日", l.conscription_conversion_rate * 100.0),
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
                        format!("建造速度 {:+.0}%", l.construction_speed_modifier * 100.0),
                        format!("消费品需求 x{:.2}", l.consumer_goods_factor),
                    ];
                    if l.id == "corporatist_war_economy" {
                        effects.push("军工政府订单：持续采购军工投入品".to_owned());
                        effects.push("MEFO 自动融资：赤字由票据覆盖至风险上限".to_owned());
                    }
                    if let Some(trade_law) = &l.forces_trade_law {
                        effects.push(format!("强制贸易法：{}", trade_law));
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
                    format!("科研槽位 {}", l.research_slots),
                    format!("福利支出 {:.0}%", l.welfare_rate * 100.0),
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
    /// V6 娉曞緥闈㈡澘
    Laws,
    /// V6 甯傚満瑙嗗浘闈㈡澘
    Market,
    /// V7.I1 POP read-only panel.
    Pops,
    /// V6 寤虹瓚鏂藉伐闈㈡澘
    ConstructionV6,
    /// V6 璐㈡斂闈㈡澘
    Finance,
    /// V6 璐告槗闈㈡澘
    Trade,
    /// J.4b: 瑁呭搴撳瓨闈㈡澘
    Logistics,
    Situation,
    Settings,
    Saves,
}

fn in_game_panel_for_panel_kind(kind: hoi4_ui::PanelKind) -> InGamePanel {
    match kind {
        hoi4_ui::PanelKind::Politics => InGamePanel::Politics,
        hoi4_ui::PanelKind::Decisions => InGamePanel::Decisions,
        hoi4_ui::PanelKind::Laws => InGamePanel::Laws,
        hoi4_ui::PanelKind::Pops => InGamePanel::Pops,
        hoi4_ui::PanelKind::Market => InGamePanel::Market,
        hoi4_ui::PanelKind::Finance => InGamePanel::Finance,
        hoi4_ui::PanelKind::Trade => InGamePanel::Trade,
        hoi4_ui::PanelKind::Construction => InGamePanel::ConstructionV6,
        hoi4_ui::PanelKind::Research => InGamePanel::Research,
        hoi4_ui::PanelKind::Diplomacy => InGamePanel::Diplomacy,
        hoi4_ui::PanelKind::Military => InGamePanel::Military,
        hoi4_ui::PanelKind::Naval => InGamePanel::Naval,
        hoi4_ui::PanelKind::Air => InGamePanel::Air,
        hoi4_ui::PanelKind::Logistics => InGamePanel::Logistics,
        hoi4_ui::PanelKind::Situation => InGamePanel::Situation,
        hoi4_ui::PanelKind::Settings => InGamePanel::Settings,
        hoi4_ui::PanelKind::Saves => InGamePanel::Saves,
    }
}

// 4.3 Step B (2026-05-18): 绉婚櫎 main 鍐呴儴???`PoliticsTab` 鏋氫妇銆倀ab 鐘舵€佹満灏嗗湪
// 搂6.3 (`tabbedWindowType`) 涓綔???GuiRt-removed 閫氱敤鍩虹璁炬柦瀹炵幇锛屼笉鍐嶇敱 main 鎸佹湁???
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PainterMode {
    Idle,
    ArmyPainter(hoi4_state::ArmyId),
    ArrowPainter(hoi4_state::ArmyId, hoi4_state::ProvinceId),
}

struct FrontlinePainterState {
    mode: PainterMode,
    samples: Vec<hoi4_state::ProvinceId>,
    last_sample_at: std::time::Instant,
}

impl Default for FrontlinePainterState {
    fn default() -> Self {
        Self {
            mode: PainterMode::Idle,
            samples: Vec::new(),
            last_sample_at: std::time::Instant::now(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct SelectionBoxState {
    active: bool,
    start: [f32; 2],
    current: [f32; 2],
    pressed_at: Instant,
}

impl Default for SelectionBoxState {
    fn default() -> Self {
        Self {
            active: false,
            start: [0.0; 2],
            current: [0.0; 2],
            pressed_at: Instant::now(),
        }
    }
}

impl SelectionBoxState {
    fn rect(self) -> [f32; 4] {
        let x0 = self.start[0].min(self.current[0]);
        let y0 = self.start[1].min(self.current[1]);
        let x1 = self.start[0].max(self.current[0]);
        let y1 = self.start[1].max(self.current[1]);
        [x0, y0, x1 - x0, y1 - y0]
    }

    fn large_enough(self) -> bool {
        let r = self.rect();
        r[2] >= UNIT_BOX_SELECT_THRESHOLD_PX && r[3] >= UNIT_BOX_SELECT_THRESHOLD_PX
    }

    fn held_long_enough(self) -> bool {
        self.pressed_at.elapsed().as_millis() >= UNIT_BOX_SELECT_HOLD_MS
    }
}

#[derive(Debug, Clone, Copy)]
struct VisualDivisionMotion {
    from: hoi4_state::ProvinceId,
    to: hoi4_state::ProvinceId,
    elapsed: f32,
    duration: f32,
}

#[derive(Clone, Debug)]
struct CombatSideSnapshot {
    country: hoi4_state::CountryId,
    tag: String,
    name: String,
    province: u16,
    active_divisions: usize,
    reserve_divisions: usize,
    avg_org_pct: f32,
    avg_strength_pct: f32,
    combat_width: f32,
    soft_attack: f32,
    hard_attack: f32,
    defense: f32,
    breakthrough: f32,
    is_attacking: bool,
}

#[derive(Clone, Debug)]
struct CombatBubbleSnapshot {
    id: u64,
    screen_pos: [f32; 2],
    chance_pct: u8,
    side_a: CombatSideSnapshot,
    side_b: CombatSideSnapshot,
    advantages: Vec<String>,
}

fn naval_mission_to_ui(mission: hoi4_state::NavalMission) -> hoi4_ui::naval::NavalMissionUi {
    match mission {
        hoi4_state::NavalMission::Idle => hoi4_ui::naval::NavalMissionUi::Idle,
        hoi4_state::NavalMission::Patrol => hoi4_ui::naval::NavalMissionUi::Patrol,
        hoi4_state::NavalMission::ConvoyEscort => hoi4_ui::naval::NavalMissionUi::ConvoyEscort,
        hoi4_state::NavalMission::StrikeForce => hoi4_ui::naval::NavalMissionUi::StrikeForce,
        hoi4_state::NavalMission::ConvoyRaiding => hoi4_ui::naval::NavalMissionUi::ConvoyRaiding,
        hoi4_state::NavalMission::MineLaying => hoi4_ui::naval::NavalMissionUi::MineLaying,
        hoi4_state::NavalMission::MineSweeping => hoi4_ui::naval::NavalMissionUi::MineSweeping,
        hoi4_state::NavalMission::NavalInvasionSupport => {
            hoi4_ui::naval::NavalMissionUi::NavalInvasionSupport
        }
    }
}

fn naval_mission_from_ui(mission: hoi4_ui::naval::NavalMissionUi) -> hoi4_state::NavalMission {
    match mission {
        hoi4_ui::naval::NavalMissionUi::Idle => hoi4_state::NavalMission::Idle,
        hoi4_ui::naval::NavalMissionUi::Patrol => hoi4_state::NavalMission::Patrol,
        hoi4_ui::naval::NavalMissionUi::ConvoyEscort => hoi4_state::NavalMission::ConvoyEscort,
        hoi4_ui::naval::NavalMissionUi::StrikeForce => hoi4_state::NavalMission::StrikeForce,
        hoi4_ui::naval::NavalMissionUi::ConvoyRaiding => hoi4_state::NavalMission::ConvoyRaiding,
        hoi4_ui::naval::NavalMissionUi::MineLaying => hoi4_state::NavalMission::MineLaying,
        hoi4_ui::naval::NavalMissionUi::MineSweeping => hoi4_state::NavalMission::MineSweeping,
        hoi4_ui::naval::NavalMissionUi::NavalInvasionSupport => {
            hoi4_state::NavalMission::NavalInvasionSupport
        }
    }
}

fn air_mission_to_ui(mission: hoi4_state::AirMission) -> hoi4_ui::air::AirMissionUi {
    match mission {
        hoi4_state::AirMission::Idle => hoi4_ui::air::AirMissionUi::Idle,
        hoi4_state::AirMission::AirSuperiority => hoi4_ui::air::AirMissionUi::AirSuperiority,
        hoi4_state::AirMission::Interception => hoi4_ui::air::AirMissionUi::Interception,
        hoi4_state::AirMission::CloseAirSupport => hoi4_ui::air::AirMissionUi::CloseAirSupport,
        hoi4_state::AirMission::StrategicBombing => hoi4_ui::air::AirMissionUi::StrategicBombing,
        hoi4_state::AirMission::PortStrike => hoi4_ui::air::AirMissionUi::PortStrike,
        hoi4_state::AirMission::NavalStrike => hoi4_ui::air::AirMissionUi::NavalStrike,
        hoi4_state::AirMission::NavalPatrol => hoi4_ui::air::AirMissionUi::NavalPatrol,
        hoi4_state::AirMission::LogisticalStrike => hoi4_ui::air::AirMissionUi::LogisticalStrike,
        hoi4_state::AirMission::Drop => hoi4_ui::air::AirMissionUi::Drop,
    }
}

fn air_mission_from_ui(mission: hoi4_ui::air::AirMissionUi) -> hoi4_state::AirMission {
    match mission {
        hoi4_ui::air::AirMissionUi::Idle => hoi4_state::AirMission::Idle,
        hoi4_ui::air::AirMissionUi::AirSuperiority => hoi4_state::AirMission::AirSuperiority,
        hoi4_ui::air::AirMissionUi::Interception => hoi4_state::AirMission::Interception,
        hoi4_ui::air::AirMissionUi::CloseAirSupport => hoi4_state::AirMission::CloseAirSupport,
        hoi4_ui::air::AirMissionUi::StrategicBombing => hoi4_state::AirMission::StrategicBombing,
        hoi4_ui::air::AirMissionUi::PortStrike => hoi4_state::AirMission::PortStrike,
        hoi4_ui::air::AirMissionUi::NavalStrike => hoi4_state::AirMission::NavalStrike,
        hoi4_ui::air::AirMissionUi::NavalPatrol => hoi4_state::AirMission::NavalPatrol,
        hoi4_ui::air::AirMissionUi::LogisticalStrike => hoi4_state::AirMission::LogisticalStrike,
        hoi4_ui::air::AirMissionUi::Drop => hoi4_state::AirMission::Drop,
    }
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
    last_status_print: Instant,
    last_perf_diag: Instant,
    perf_last_hours: u64,
    perf_counter_rebuilds: u32,
    perf_counter_cache_hits: u32,
    perf_counter_instances: usize,
    perf_render_us: u64,
    perf_render_frames: u32,
    perf_counter_update_us: u64,
    perf_arrow_update_us: u64,
    counter_visibility_cache: CounterVisibilityCache,
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
    last_frame_cpu_ms: f32,
    last_map_prepare_cpu_ms: f32,
    demo_visible: bool,
    b5_demo_visible: bool,
    demo_window: hoi4_ui::demo::DemoWindow,
    /// V9 demo 页（F12 切换）— 见 ROADMAP_V9_UI_FRONTEND_REDESIGN.md。
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
    /// Phase 4.2: 褰撳墠鏄剧ず鐨勮彍鍗曠被鍨嬨€侾laying 闃舵鏃犺彍鍗??    
    menu_kind: Option<MenuKind>,
    /// Phase 4.2 (redesign): 褰撳墠 hover 鐨勮彍鍗曟寜???id???btn_new_game" 绛夛級??    
    menu_hovered_btn: Option<&'static str>,
    /// Phase 4.2 (redesign): 褰撳墠 hover 鐨勫浗瀹跺垪琛ㄨ index??    
    menu_hovered_row: Option<usize>,
    /// Phase 4.2 (redesign): 鍥藉閫夋嫨鑿滃崟鍙€夊浗瀹跺垪琛紙Phase 4.2 榛樿鍙湁 GER 鍙€夛級??    
    available_countries: Vec<CountryEntry>,
    /// 涓婁竴甯у竷灞€缂撳瓨鐨勬寜???+ 琛岋紙鐢ㄤ簬 click 鍒ゅ畾锛??    
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
    /// Toggle: show province name labels (F7). Default OFF until the pass is fixed.
    show_province_names: bool,
    /// Phase 4.3: currently open in-game panel (None = no panel).
    open_panel: Option<InGamePanel>,
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
    // V5 鏀跺彛锛?026-05-18锛夛細鍒犻櫎 gui_rt_removed / menu_runtime / menu_hovered_id /
    // menu_pressed_id 绛夊瓧娈碘€斺€斿畠浠湇鍔′簬宸插垹闄ょ殑 vanilla GUI 瑙ｆ瀽鍣ㄨ矾绾?    // (`hoi4_assets::GuiRt-removed` / topbar.gui / countrypoliticsview.gui)銆?    // 鑿滃崟 hit-test 瀹屽叏璧?`last_main_buttons` / `last_country_layout`銆?    // topbar / 鏀挎不闈㈡澘娓叉煋鎺ㄨ繜鍒伴樁娈?B (egui)銆?
    content: hoi4_runtime::ContentRuntimeState,
    focus_panel: hoi4_ui::focus_tree_panel::FocusTreePanel,
    /// F.1: 事件触发时记下事件触发前的速度
    pre_event_speed: Option<GameSpeed>,
    /// Last event modal id that played the popup sound, to avoid replaying every frame.
    last_event_sound_id: Option<String>,
    /// P1.1：待显示的投降/和平通知队列
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
    /// 11.1锛欶rontline painter state (draw frontline / arrow by mouse drag).
    frontline_painter: FrontlinePainterState,
    /// 11.1锛歸hether frontline overlay is visible (toggle).
    frontline_overlay_visible: bool,
    /// 11.4锛歞irty hash for frontline arrow instances.
    prev_armies_hash: u64,
    frontline_overlay_hash: u64,
    /// 11.2锛歝urrently selected army (click frontline on map or select in bottom bar).
    selected_army_id: Option<hoi4_state::ArmyId>,
    template_editor_open: bool,
    selected_template_idx: Option<u16>,
    template_picker_target: Option<hoi4_ui::military::TemplatePickerTarget>,
    v6_db: hoi4_content::V6Database,
    historical_1936: hoi4_content::Historical1936Database,
    /// P1.3：法律切换失败时的中文提示（UI 显示用）
    law_error_message: Option<String>,
    last_law_error_toast: Option<String>,
    /// P1.6：面板数据缓存，UI 只消费逻辑层快照，避免 render 路径每帧重算。
    ui_panel_cache: UiPanelCache,
    map_phase0: Option<MapPhase0Run>,
}

#[derive(Default)]
struct CounterVisibilityCache {
    valid: bool,
    signature: u64,
    visible: Option<HashSet<CountryId>>,
    spotted: Option<HashSet<u16>>,
}

struct RenderState {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    /// Vanilla pdxmap-equivalent terrain pipeline. Phase 11 removed the old
    /// archived `shader.wgsl` render fallback; missing assets are now handled
    /// inside TerrainPass via explicit 1x1 texture fallbacks.
    terrain_pass: TerrainPass,
    camera_buffer: wgpu::Buffer,
    /// Per-frame render params (selection, zoom, time).
    params_buffer: wgpu::Buffer,
    /// Per-LOD instance buffers (one ChunkInstance per visible chunk in that LOD).
    instance_buffers: [wgpu::Buffer; 3],
    /// Capacity (in instances) of each instance buffer; grown as needed.
    instance_capacity: [u32; 3],
    /// Last uploaded terrain chunk buckets; avoids rewriting identical instance buffers while panning.
    terrain_bucket_signature: [u64; 3],
    terrain_bucket_counts: [u32; 3],
    lut_texture: wgpu::Texture,
    lut_width: u32,
    lut_height: u32,
    /// Occupation overlay LUT - held to keep the bind-group view alive.
    /// Rebuilt when controller changes can affect map colour overlays.
    #[allow(dead_code)]
    occupation_lut_texture: wgpu::Texture,
    depth_view: wgpu::TextureView,
    depth_format: wgpu::TextureFormat,
    chunk_grid: ChunkGrid,
    /// Trees pipeline (5.6). One large vertex buffer + a single instanced draw.
    trees_pipeline: wgpu::RenderPipeline,
    trees_bind_group: wgpu::BindGroup,
    trees_buffer: wgpu::Buffer,
    trees_count: u32,
    /// Railways pipeline (5.6). LineList draw.
    railways_pipeline: wgpu::RenderPipeline,
    railways_bind_group: wgpu::BindGroup,
    railways_params_buffer: wgpu::Buffer,
    railways_buffer: wgpu::Buffer,
    railways_vertex_count: u32,
    /// Phase I (CR-1.2): HOI3-style screen-space procedural counter pass.
    hoi3_counter_pass: Hoi3CounterPass,
    /// Province pixel centroids (heightmap-pixel coords). Cached on init so the
    /// per-frame zoom-driven counter aggregation doesn't need to recompute them.
    unit_counter_centroids: Vec<(f32, f32)>,
    /// Province pixel bounds in heightmap/province-map coordinates. This is
    /// static map data used by frontline arrow collection.
    province_pixel_bounds: Vec<Option<render_collect::ProvincePixelBounds>>,
    /// Frontlines pipeline (5.7). LineList.
    frontlines_pipeline: wgpu::RenderPipeline,
    frontlines_bind_group: wgpu::BindGroup,
    frontlines_buffer: wgpu::Buffer,
    frontlines_vertex_count: u32,
    frontlines_params_buffer: wgpu::Buffer,
    // V5 鏀跺彛???026-05-18锛夛細鍒犻櫎 `ui_pass: UI-pass-removed` 瀛楁銆倀opbar / 鏀挎不闈㈡澘
    // / 9-slice sprite 娓叉煋绠＄嚎宸插仠鐢紝绛夊緟闃舵 B ???egui 閲嶅仛???    /// Phase 2.9: Buildings instanced buffer (reuses units pipeline).
    buildings_buffer: wgpu::Buffer,
    buildings_params_buffer: wgpu::Buffer,
    buildings_bind_group: wgpu::BindGroup,
    buildings_count: u32,
    buildings_pipeline: wgpu::RenderPipeline,
    /// Phase 3.12.5 ???`PdxMeshPass` for vanilla 3D building meshes (replaces
    /// the procedural `buildings_pipeline` flat-coloured billboards above when
    /// `pdxmesh_pass.any_loaded` is true).
    pdxmesh_pass: passes::PdxMeshPass,
    /// Phase 3.12.6 ???`WaterPass` for vanilla pdxwater shading (LEAN normals,
    /// Fresnel, planar+cube reflection, sun spec, coastal foam, polar ice).
    /// Drawn after the terrain pass so it overdraws the inline water branch
    /// in `terrain.wgsl` with full pdxwater output. When water vanilla
    /// textures fail to load, falls back to terrain.wgsl's procedural water.
    water_pass: passes::WaterPass,
    /// Phase 3.12.7 ???`RiverPass` for vanilla river.shader rendering (flow
    /// scrolling + diffuse/normal/masks textures + alpha blend). Drawn after
    /// terrain, before water ???replaces inline navy-blue overlay.
    river_pass: passes::RiverPass,
    /// Phase 3.12.9 ???`BorderPass` for vanilla border.shader rendering
    /// (6-type 脳 3-LOD = 18 pre-baked SDF textures + gradient_border
    /// dual-channel fill). Drawn after water, replacing terrain.wgsl's
    /// inline SDF border code. SDF fallback stays in terrain.wgsl when
    /// border_pass.any_loaded is false.
    border_pass: passes::BorderPass,
    /// Phase 3.12.11 ???`SkyPass` for sky cubemap background.
    sky_pass: passes::SkyPass,
    /// Phase 3.12.10 ???`ParticlePass` for combat smoke / factory chimneys / scorched earth.
    particle_pass: passes::ParticlePass,
    /// Phase 16.1 ???`MapArrowPass` for military order arrows (move / invade / paradrop).
    maparrow_pass: passes::MapArrowPass,
    /// Phase 16.2 ???`TradeRoutePass` for flowing trade route dashed lines.
    traderoute_pass: passes::TradeRoutePass,
    /// Phase 16.3 ???`StraitPass` for strait / canal crossing lines.
    strait_pass: passes::StraitPass,
    /// Phase 14 ???`PoiIconPass` for vanilla POI icons (factories / ports / airbases / resources).
    poi_icon_pass: Option<passes::PoiIconPass>,
    poi_icon_instances: Vec<PoiIconInstance>,
    poi_zoom_bucket: u8,
    /// Phase 3.5: Text rendering pass.
    text_pass: TextPass,
    panel_pass: PanelPass,
    flag_bank: FlagBank,
    flag_pipeline: wgpu::RenderPipeline,
    flag_bgl: wgpu::BindGroupLayout,
    flag_uniform_buffer: wgpu::Buffer,
    flag_vertex_buffer: wgpu::Buffer,
    /// Phase 3.6.3: 3D mesh trees - one draw call per tree type.
    trees_mesh_pipeline: wgpu::RenderPipeline,
    trees_mesh_bind_groups: Vec<wgpu::BindGroup>, // one per tree type (with its texture)
    trees_mesh_vertex_buffers: Vec<wgpu::Buffer>, // mesh geometry per type
    trees_mesh_index_buffers: Vec<wgpu::Buffer>,  // mesh indices per type
    trees_mesh_instance_buffers: Vec<wgpu::Buffer>, // per-tree positions per type
    trees_mesh_index_counts: Vec<u32>,
    trees_mesh_instance_counts: Vec<u32>,
    /// Phase 3.12.8 鈥?vanilla tree.shader full integration (season coloring +
    /// tint overlay + shadow receive + day/night). Replaces the old
    /// `trees_mesh_*` path when `tree_full_pass.any_loaded` is true.
    tree_full_pass: Option<passes::TreeFullPass>,
    tree_lod_uploaded: bool,
    tree_lod_last_cam_pos: [f32; 3],
    /// Phase 3.5: Per-country world-space label anchors (centroid of owned provinces).
    /// `Some(CountryLabel)` for countries with at least one owned province.
    country_labels: Vec<Option<hoi4_render::mapname::CountryLabel>>,
    /// Phase 3.12.10: vanilla-equivalent 3D country-name label pass
    /// (GlobalFrameUniform + vDistortedPos + day/night 0.35 + stencil ref=4).
    /// `None` when atlas baking failed at startup; falls back to 2D HUD path.
    mapname_pass: Option<passes::MapnamePass>,
    /// Cached country-name atlas; reused when rebuilding labels after
    /// runtime ownership changes (civil war split, annexation).
    mapname_atlas: Option<mapname_atlas::CountryNameAtlas>,
    /// Phase 3.12.13: province-name label pass (zoom-gated, only land provinces).
    province_name_pass: Option<passes::ProvinceNamePass>,
    // 鈹€鈹€鈹€ Phase 3.12.1 鍏叡娓叉煋鍩虹璁炬柦 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€
    hdr_target: HdrTarget,
    global_uniform_buf: GlobalUniformBuffer,
    simple_blit: SimpleBlitPass,
    post_process: PostProcessChain,
    shadow_pass: passes::ShadowPass,
    map_renderer: MapRenderer,
    pass_registry: PassRegistry,
    debug_render_overlay: DebugOverlay,
    gpu_profiler: Option<GpuTimestampProfiler>,
    ui: hoi4_ui::UiState,
    nine_slice_window: Option<hoi4_ui::nine_slice::NineSlice>,
    icon_bank: hoi4_ui::icons::IconBank,
    window: Arc<Window>,
}

impl App {
    fn new(mut world: World, path_cfg: PathConfig) -> Self {
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
        // Phase 4.2 榛樿鍙湁 GER 鍙€夛紝鍏朵粬 majors 鐏版樉锛堟湭鏉ュ紑鏀撅級??
        let available_countries = build_country_select_list(&world);

        // Phase 3.12.8: load map/seasons.txt for tree season computation.
        let seasons_data =
            hoi4_map::load_seasons_txt(&path_cfg.game_path().join("map/seasons.txt"));

        let scenario_content = content_bootstrap::load_scenario_content("1936");

        // P0.1：在 world move 之前计算初始日期
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
            last_status_print: Instant::now(),
            last_perf_diag: Instant::now(),
            perf_last_hours: 0,
            perf_counter_rebuilds: 0,
            perf_counter_cache_hits: 0,
            perf_counter_instances: 0,
            perf_render_us: 0,
            perf_render_frames: 0,
            perf_counter_update_us: 0,
            perf_arrow_update_us: 0,
            counter_visibility_cache: CounterVisibilityCache::default(),
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
            show_province_names: false,
            open_panel: None,
            diplomacy_sort_by_opinion: false,
            diplomacy_selected_country_tag: None,
            construction_mode: None,
            construction_highlight_province_ids: HashSet::new(),
            auto_build_enabled: false,
            last_auto_build_month: None,
            last_auto_build_explanations: Vec::new(),
            // V5 鏀跺彛锛歡ui_rt_removed / menu_runtime / menu_hovered_id /
            // menu_pressed_id / politics_tab / politics_scroll 瀛楁宸插垹闄ゃ€?
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
            map_refresh_owners: Vec::new(),
            map_refresh_controllers: Vec::new(),
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
        }
    }

    fn enable_map_phase0(&mut self, output_dir: PathBuf) {
        let run = MapPhase0Run::new(&self.path_cfg, output_dir);
        println!(
            "[map-phase0] starting capture batch: scenes={} layers={} captures={} output={}",
            run.scenes.len(),
            map_baseline::MapBaselineLayer::ALL.len(),
            run.captures.len(),
            run.output_dir.display()
        );
        println!("[map-phase0] {}", run.asset_audit.summary_line());
        if !run.asset_audit.fallback_paths.is_empty() {
            for path in run.asset_audit.fallback_paths.iter().take(16) {
                eprintln!("[map-phase0] fallback asset: {path}");
            }
            if run.asset_audit.fallback_paths.len() > 16 {
                eprintln!(
                    "[map-phase0] fallback asset: ... {} more",
                    run.asset_audit.fallback_paths.len() - 16
                );
            }
        }
        if !run.asset_audit.can_use_for_visual_review() {
            eprintln!(
                "[map-phase0] critical fallback present; screenshots will be marked unusable for visual review"
            );
        }

        self.game_phase = GamePhase::Playing;
        self.world.speed = GameSpeed::Paused;
        self.open_panel = None;
        self.demo_visible = false;
        self.b5_demo_visible = false;
        self.debug_overlay = false;
        self.terrain_debug_view = passes::TerrainDebugView::Off;
        self.water_debug_view = passes::WaterDebugView::Off;
        self.border_debug_view = passes::BorderDebugView::Off;
        self.postprocess_debug_view = PostProcessDebugView::Final;
        self.map_mode = MapMode::Political;
        self.map_phase0 = Some(run);
    }

    fn map_phase0_finished(&self) -> bool {
        self.map_phase0
            .as_ref()
            .map(|run| run.finished)
            .unwrap_or(false)
    }

    fn current_map_layer_mask(&self) -> map_baseline::MapLayerMask {
        self.map_phase0
            .as_ref()
            .and_then(|run| run.current_capture())
            .map(|capture| capture.layer_mask)
            .unwrap_or_else(map_baseline::MapLayerMask::all)
    }

    fn map_phase0_capture_ready(&self) -> bool {
        self.map_phase0.as_ref().is_some_and(|run| {
            !run.finished && run.settle_frames == 0 && run.current_capture().is_some()
        })
    }

    fn map_phase0_capture_path(&self) -> Option<PathBuf> {
        let run = self.map_phase0.as_ref()?;
        let capture = run.current_capture()?;
        Some(run.output_dir.join(&capture.filename))
    }

    fn map_phase0_debug_lines(&self) -> Vec<String> {
        let Some(run) = self.map_phase0.as_ref() else {
            return Vec::new();
        };
        let mut lines = vec![
            "Map Renderer V2 Phase 0 - Asset Fallback Debug".to_string(),
            run.asset_audit.summary_line(),
            format!(
                "visual_review_usable={}",
                run.asset_audit.can_use_for_visual_review()
            ),
        ];
        if run.asset_audit.fallback_paths.is_empty() {
            lines.push("fallbacks=0".to_string());
        } else {
            lines.push(format!(
                "fallbacks={}",
                run.asset_audit.fallback_paths.len()
            ));
            for path in run.asset_audit.fallback_paths.iter().take(20) {
                lines.push(format!("fallback: {path}"));
            }
            if run.asset_audit.fallback_paths.len() > 20 {
                lines.push(format!(
                    "... {} more",
                    run.asset_audit.fallback_paths.len() - 20
                ));
            }
        }
        lines
    }

    fn prepare_map_phase0_capture(&mut self) {
        let Some(run) = self.map_phase0.as_ref() else {
            return;
        };
        if run.finished {
            return;
        }
        let Some(capture) = run.current_capture() else {
            self.finish_map_phase0();
            return;
        };
        let Some(scene) = run
            .scenes
            .iter()
            .find(|scene| scene.name == capture.scene_name)
            .cloned()
        else {
            return;
        };
        let mask = capture.layer_mask;
        let world_size = self.camera.world_size;
        self.camera.target = glam::Vec3::new(
            scene.camera.target_uv[0] * world_size.x,
            0.0,
            scene.camera.target_uv[1] * world_size.y,
        );
        self.camera.distance = (world_size.x.max(world_size.y) * scene.camera.distance_factor)
            .clamp(3.0, world_size.x.max(world_size.y) * 4.0);
        self.camera.pitch = scene.camera.pitch_degrees.to_radians();
        self.camera.yaw = scene.camera.yaw_degrees.to_radians();
        self.camera.clamp_target_to_map();
        self.show_province_names = mask.labels;
        if self.map_mode != MapMode::Political {
            self.map_mode = MapMode::Political;
            self.refresh_lut();
        }
        self.postprocess_debug_view = match capture.layer {
            map_baseline::MapBaselineLayer::HdrRaw => PostProcessDebugView::HdrRaw,
            map_baseline::MapBaselineLayer::TonemapOnly => PostProcessDebugView::TonemapOnly,
            map_baseline::MapBaselineLayer::BloomOnly => PostProcessDebugView::BloomOnly,
            _ => PostProcessDebugView::Final,
        };
        if let Some(s) = self.state.as_mut() {
            s.post_process.debug_view = self.postprocess_debug_view;
        }
        self.upload_camera();
    }

    fn map_phase0_after_uncaptured_frame(&mut self) {
        if let Some(run) = self.map_phase0.as_mut() {
            if !run.finished && run.settle_frames > 0 {
                run.settle_frames -= 1;
            }
        }
    }

    fn map_phase0_after_capture(&mut self, frame_time_ms: f32, result: Result<(), String>) {
        if let Err(err) = result {
            eprintln!("[map-phase0] screenshot write failed: {err}");
        }
        let Some(run) = self.map_phase0.as_mut() else {
            return;
        };
        if run.finished {
            return;
        }
        let total = run.captures.len();
        if let Some(capture) = run.captures.get_mut(run.capture_index) {
            capture.frame_time_ms = Some(frame_time_ms);
            println!(
                "[map-phase0] captured {}/{} {} frame_time_ms={:.2} usable={}",
                run.capture_index + 1,
                total,
                capture.filename,
                frame_time_ms,
                capture.visual_review_usable
            );
        }
        run.capture_index += 1;
        run.settle_frames = 1;
        if run.capture_index >= total {
            self.finish_map_phase0();
        }
    }

    fn finish_map_phase0(&mut self) {
        let Some(run) = self.map_phase0.as_mut() else {
            return;
        };
        if run.finished {
            return;
        }
        let report = map_baseline::MapBaselineReport::from_captures(
            run.scenes.clone(),
            run.captures.clone(),
            run.asset_audit.clone(),
            run.started.elapsed().as_secs_f64() * 1000.0,
        );
        match map_baseline::write_phase0_report_files(&report, &run.output_dir) {
            Ok(path) => println!("[map-phase0] wrote {}", path.display()),
            Err(err) => eprintln!("[map-phase0] report write failed: {err}"),
        }
        run.finished = true;
    }

    fn head_of_state_display(
        world: &World,
        historical_1936: &hoi4_content::Historical1936Database,
        country: hoi4_state::CountryId,
    ) -> (String, Option<String>) {
        if country.is_none() {
            return (String::new(), None);
        }
        let Some(tag) = world.countries.tags.get(country.0 as usize) else {
            return (String::new(), None);
        };

        if let Some(def) = historical_1936.head_of_state(tag) {
            let character = if def.character_key.is_empty() {
                None
            } else {
                world
                    .data
                    .characters
                    .iter()
                    .find(|character| character.key == def.character_key)
            };
            let name = if !def.name.is_empty() {
                def.name.clone()
            } else if let Some(character) = character {
                world
                    .data
                    .character_names
                    .get(&character.name_loc_key)
                    .cloned()
                    .unwrap_or_else(|| character.name_loc_key.clone())
            } else {
                def.character_key.clone()
            };
            let leader_fallback_portrait = if !def.character_key.is_empty() || def.name.is_empty() {
                world
                    .country_leader(country)
                    .and_then(|leader| leader.portrait_large.clone())
            } else {
                None
            };
            let portrait = if !def.portrait_gfx.is_empty() {
                Some(def.portrait_gfx.clone())
            } else {
                character
                    .and_then(|character| character.portrait_large.clone())
                    .or(leader_fallback_portrait)
            };
            return (name, portrait);
        }

        match world.country_leader(country) {
            Some(def) => {
                let name = world
                    .data
                    .character_names
                    .get(&def.name_loc_key)
                    .cloned()
                    .unwrap_or_else(|| def.name_loc_key.clone());
                (name, def.portrait_large.clone())
            }
            None => (String::new(), None),
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

    fn finance_panel_signature(world: &World, player: usize) -> u64 {
        let mut h = DefaultHasher::new();
        if let Some(t) = world.countries.treasury.treasuries.get(player) {
            t.cash_rm.to_bits().hash(&mut h);
            t.reserve_gbp.to_bits().hash(&mut h);
            t.daily_income_rm.to_bits().hash(&mut h);
            t.daily_expense_rm.to_bits().hash(&mut h);
            t.public_debt_rm.to_bits().hash(&mut h);
            t.mefo_debt_rm.to_bits().hash(&mut h);
            t.gdp_rm.to_bits().hash(&mut h);
            t.credit_rating.hash(&mut h);
        }
        if let Some(rate) = world.countries.treasury.exchange_rates.get(player) {
            rate.rm_per_gbp.to_bits().hash(&mut h);
        }
        h.finish()
    }

    fn market_panel_signature(world: &World, player: usize) -> u64 {
        let mut h = DefaultHasher::new();
        if let Some(market) = world.countries.market.markets.get(player) {
            market.supply.len().hash(&mut h);
            market.demand.len().hash(&mut h);
            market.price.len().hash(&mut h);
            market.stockpile.len().hash(&mut h);
            market.clearing_sheet.results.len().hash(&mut h);
        }
        world.countries.trade.routes.len().hash(&mut h);
        world.countries.buildings_v6.buildings.len().hash(&mut h);
        world.countries.pops.groups.len().hash(&mut h);
        Self::finance_panel_signature(world, player).hash(&mut h);
        h.finish()
    }

    fn construction_panel_signature(
        world: &World,
        econ: &EconomyState,
        auto_build_enabled: bool,
        last_auto_build_explanations_len: usize,
        player: usize,
    ) -> u64 {
        let mut h = DefaultHasher::new();
        world.countries.buildings_v6.buildings.len().hash(&mut h);
        world.countries.pops.groups.len().hash(&mut h);
        auto_build_enabled.hash(&mut h);
        last_auto_build_explanations_len.hash(&mut h);
        if let Some(queue) = econ.construction.get(player) {
            queue.items.len().hash(&mut h);
            for item in &queue.items {
                item.building_key.hash(&mut h);
                item.target_state.hash(&mut h);
                item.progress.to_bits().hash(&mut h);
                item.paid_funds_rm.to_bits().hash(&mut h);
                for need in &item.material_needs {
                    need.good_id.hash(&mut h);
                    need.consumed.to_bits().hash(&mut h);
                    need.total_needed.to_bits().hash(&mut h);
                }
            }
        }
        Self::market_panel_signature(world, player).hash(&mut h);
        h.finish()
    }

    fn diplomacy_panel_signature(world: &World, player: usize) -> u64 {
        let mut h = DefaultHasher::new();
        world.diplomacy.wars.len().hash(&mut h);
        world.diplomacy.factions.len().hash(&mut h);
        world.diplomacy.diplomatic_requests.len().hash(&mut h);
        world.diplomacy.world_tension.to_bits().hash(&mut h);
        world.countries.tags.len().hash(&mut h);
        player.hash(&mut h);
        h.finish()
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

    fn v6_industrial_levels(world: &World, country: CountryId) -> (u32, u32) {
        if country.is_none() {
            return (0, 0);
        }
        let mut industrial = 0u32;
        let mut military = 0u32;
        for building in &world.countries.buildings_v6.buildings {
            let state_idx = building.state.0 as usize;
            if state_idx >= world.states.count
                || world.states.owners[state_idx] != country
                || building.level == 0
            {
                continue;
            }
            match building.kind {
                hoi4_state::BuildingKind::Military => military += building.level as u32,
                hoi4_state::BuildingKind::MilitaryBase => {}
                _ => industrial += building.level as u32,
            }
        }
        (industrial, military)
    }

    fn v6_estimated_gdp_gbp(
        world: &World,
        db: &hoi4_content::V6Database,
        country: CountryId,
    ) -> f64 {
        if country.is_none() {
            return 0.0;
        }
        let ci = country.0 as usize;
        let rm_per_gbp = world.countries.treasury.exchange_rates[ci]
            .rm_per_gbp
            .max(0.1) as f64;
        let mut gdp_rm = 0.0_f64;
        for building in &world.countries.buildings_v6.buildings {
            let state_idx = building.state.0 as usize;
            if state_idx >= world.states.count
                || world.states.owners[state_idx] != country
                || building.level == 0
            {
                continue;
            }
            gdp_rm += building.value_added_rm;
        }
        gdp_rm * 365.0 / rm_per_gbp
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
                    format!("商品 {}", Self::v6_good_name(db, id))
                }
                hoi4_content::v6_loader::TechUnlockDef::PM(id) => {
                    format!("生产方式 {}", Self::v6_pm_name(db, id))
                }
                hoi4_content::v6_loader::TechUnlockDef::Building(id) => {
                    format!("建筑 {}", Self::v6_building_name(db, id))
                }
                hoi4_content::v6_loader::TechUnlockDef::Law(cat, law_id) => {
                    let label = Self::v6_required_law_label(db, Some(&(*cat, law_id.clone())))
                        .unwrap_or_else(|| law_id.clone());
                    format!("法律 {}", label)
                }
            })
            .collect()
    }

    fn v6_blockade_affected_buildings(
        world: &World,
        db: &hoi4_content::V6Database,
        player: usize,
    ) -> Vec<String> {
        let player_id = hoi4_state::CountryId(player as u16);
        let Some(market) = world.countries.market.markets.get(player) else {
            return Vec::new();
        };
        let imported_goods: Vec<String> = market
            .imports
            .iter()
            .filter(|(_, amount)| **amount > 0.0)
            .map(|(good_id, _)| good_id.clone())
            .collect();
        if imported_goods.is_empty() {
            return Vec::new();
        }
        let mut affected = Vec::new();
        for building in &world.countries.buildings_v6.buildings {
            let state_idx = building.state.0 as usize;
            if state_idx >= world.states.count
                || world.states.owners[state_idx] != player_id
                || building.level == 0
            {
                continue;
            }
            let pms = hoi4_content::active_pms_for_building(building, db);
            let blocked_inputs: Vec<String> = pms
                .iter()
                .flat_map(|pm| pm.input_good_ids.iter())
                .filter(|good_id| imported_goods.contains(good_id))
                .map(|good_id| Self::v6_good_name(db, good_id))
                .collect();
            if blocked_inputs.is_empty() {
                continue;
            }
            let state_name = world
                .states
                .names
                .get(state_idx)
                .cloned()
                .unwrap_or_default();
            let building_name = Self::v6_building_name(db, &building.building_def_id);
            let label = if state_name.is_empty() {
                format!("{}：缺 {}", building_name, blocked_inputs.join("/"))
            } else {
                format!(
                    "{} {}：缺 {}",
                    state_name,
                    building_name,
                    blocked_inputs.join("/")
                )
            };
            if !affected.contains(&label) {
                affected.push(label);
            }
            if affected.len() >= 6 {
                break;
            }
        }
        affected
    }

    fn v6_building_group(kind: hoi4_content::v6_loader::BuildingKindDef) -> &'static str {
        match kind {
            hoi4_content::v6_loader::BuildingKindDef::Resource
            | hoi4_content::v6_loader::BuildingKindDef::Agriculture => "资源与农业",
            hoi4_content::v6_loader::BuildingKindDef::Industrial => "城市工业",
            hoi4_content::v6_loader::BuildingKindDef::ConsumerGoods => "民生工业",
            hoi4_content::v6_loader::BuildingKindDef::Infrastructure => "基础设施",
            hoi4_content::v6_loader::BuildingKindDef::Military => "军工",
            hoi4_content::v6_loader::BuildingKindDef::MilitaryBase => "军事基地",
            hoi4_content::v6_loader::BuildingKindDef::Service => "政府与服务",
        }
    }

    fn construction_funding_source_label(
        source: hoi4_logic::economy::ConstructionFundingSource,
    ) -> &'static str {
        match source {
            hoi4_logic::economy::ConstructionFundingSource::Government => "政府",
            hoi4_logic::economy::ConstructionFundingSource::Mefo => "MEFO 融资",
            hoi4_logic::economy::ConstructionFundingSource::PrivatePool => "私人投资池",
            hoi4_logic::economy::ConstructionFundingSource::CartelPool => "法团投资池",
            hoi4_logic::economy::ConstructionFundingSource::OverlordInvestment { .. } => {
                "宗主国投资"
            }
            hoi4_logic::economy::ConstructionFundingSource::ForeignInvestment { .. } => "外资",
        }
    }

    fn building_owner_label(owner: hoi4_state::BuildingOwner) -> &'static str {
        match owner {
            hoi4_state::BuildingOwner::State => "国有",
            hoi4_state::BuildingOwner::Private => "私人",
            hoi4_state::BuildingOwner::Cartel => "法团",
        }
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
            "需要法律「{}」",
            law_name.unwrap_or(law_id.as_str())
        ))
    }

    fn v6_building_lock_reason(
        world: &World,
        db: &hoi4_content::V6Database,
        player: usize,
        building_def: &hoi4_content::v6_loader::BuildingDef,
    ) -> Option<String> {
        if !building_def.buildable {
            return Some("不可建造".to_owned());
        }

        if let Some(reason) = Self::v6_required_law_label(db, building_def.requires_law.as_ref()) {
            let current = building_def.requires_law.as_ref().and_then(|(cat, _)| {
                Some(
                    &world.countries.law_store.law_sets[player].0
                        [Self::v6_law_category_index(*cat)]
                    .current,
                )
            });
            if let Some(current) = current {
                if let Some((_, required_law)) = &building_def.requires_law {
                    if current != required_law {
                        return Some(reason);
                    }
                }
            }
        }

        if let Some(tech) = db.technologies.iter().find(|tech| {
            tech.unlocks.iter().any(|unlock| matches!(unlock, hoi4_content::v6_loader::TechUnlockDef::Building(id) if id == &building_def.id))
        }) {
            let unlocked = world.countries.completed_techs[player].contains(&tech.id)
                || world.countries.unlocked_buildings[player].contains(&building_def.id);
            if !unlocked {
                return Some(format!("科技锁定：需要「{}」", tech.name));
            }
        }

        None
    }

    fn v6_building_state_limit_reason(
        world: &World,
        building_def: &hoi4_content::v6_loader::BuildingDef,
    ) -> Option<String> {
        match building_def.state_limit_kind.as_deref() {
            Some("coastal") => Some("州限制：仅可在沿海州建造".to_owned()),
            Some("urban") => Some("州限制：仅可在城市州建造".to_owned()),
            Some("resource") => Some("州限制：仅可在资源州建造".to_owned()),
            Some(kind) => Some(format!("州限制：{}", kind)),
            None => {
                let _ = world;
                None
            }
        }
    }

    fn exit_construction_mode(&mut self) {
        self.construction_mode = None;
        self.construction_highlight_province_ids.clear();
    }

    fn construction_highlight_provinces(
        world: &World,
        v6_db: &hoi4_content::V6Database,
        player: usize,
        building_key: &str,
    ) -> HashSet<u32> {
        let mut highlighted = HashSet::new();
        let Some(building_def) = v6_db.buildings.iter().find(|def| def.id == building_key) else {
            return highlighted;
        };

        let player_cid = hoi4_state::CountryId(player as u16);
        for si in 0..world.states.count {
            if world.states.owners[si] != player_cid {
                continue;
            }
            let sid = hoi4_state::StateId(si as u16);
            if !Self::v6_state_has_free_building_slot(world, sid) {
                continue;
            }
            if hoi4_logic::economy::construction_tick::validate_build_location(
                world,
                v6_db,
                player,
                building_def,
                sid,
            )
            .is_err()
            {
                continue;
            }
            for province in &world.states.provinces[si] {
                highlighted.insert(province.0 as u32);
            }
        }
        highlighted
    }

    fn v6_state_has_free_building_slot(world: &World, state: hoi4_state::StateId) -> bool {
        let si = state.0 as usize;
        if si >= world.states.count {
            return false;
        }
        let used: u8 = world
            .countries
            .buildings_v6
            .buildings
            .iter()
            .filter(|building| building.state == state && building.level > 0)
            .map(|building| building.level)
            .sum();
        (used as u16) < Self::v6_state_building_capacity(world, state)
    }

    fn v6_state_building_capacity(world: &World, state: hoi4_state::StateId) -> u16 {
        let si = state.0 as usize;
        if si >= world.states.count {
            return 0;
        }
        (world.states.category_slots[si] as u16).max(4) + 20
    }

    fn v6_law_category_index(category: hoi4_content::v6_loader::LawCategoryDef) -> usize {
        match category {
            hoi4_content::v6_loader::LawCategoryDef::Conscription => {
                hoi4_state::LawCategory::Conscription.index()
            }
            hoi4_content::v6_loader::LawCategoryDef::Economy => {
                hoi4_state::LawCategory::Economy.index()
            }
            hoi4_content::v6_loader::LawCategoryDef::Trade => {
                hoi4_state::LawCategory::Trade.index()
            }
            hoi4_content::v6_loader::LawCategoryDef::Taxation => {
                hoi4_state::LawCategory::Taxation.index()
            }
            hoi4_content::v6_loader::LawCategoryDef::CivilRights => {
                hoi4_state::LawCategory::CivilRights.index()
            }
            hoi4_content::v6_loader::LawCategoryDef::InformationControl => {
                hoi4_state::LawCategory::InformationControl.index()
            }
        }
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
        db.goods
            .iter()
            .find(|good| good.id == good_id)
            .map(|good| localized_content_name(&good.id, &good.name))
            .unwrap_or_else(|| good_id.to_owned())
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
        if raw.is_empty() {
            return format!("{} {}", hoi4_ui::i18n::tr("belongs_to_state"), state_idx);
        }
        let localized = self.localize_key(raw);
        if localized == raw {
            if let Some(id) = raw
                .strip_prefix("STATE_")
                .and_then(|s| s.parse::<u16>().ok())
            {
                return format!("第{}州", id);
            }
        }
        localized
    }

    fn country_display_name(&self, country: hoi4_state::CountryId) -> String {
        self.world
            .countries
            .tags
            .get(country.0 as usize)
            .map(|tag| hoi4_ui::i18n::tr(tag).to_string())
            .unwrap_or_else(|| hoi4_ui::i18n::tr("unknown").to_owned())
    }

    fn province_display_name(
        &self,
        province_id: u32,
        vp_name: Option<&str>,
        state_name: Option<&str>,
    ) -> String {
        if let Some(name) = vp_name {
            if !name.is_empty() {
                return name.to_owned();
            }
        }
        if let Some(name) = state_name {
            if !name.is_empty() && name != hoi4_ui::i18n::tr("unknown") {
                return format!("{} #{}", name, province_id);
            }
        }
        format!("{} {}", hoi4_ui::i18n::tr("province"), province_id)
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

    fn equipment_display_name(equipment_id: &str) -> String {
        match equipment_id {
            "infantry_equipment" => "步兵装备".to_owned(),
            "artillery" => "火炮".to_owned(),
            "anti_tank" => "反坦克炮".to_owned(),
            "anti_air" => "防空炮".to_owned(),
            "support_equipment" => "支援装备".to_owned(),
            "motorized" => "摩托化装备".to_owned(),
            "mechanized" => "机械化装备".to_owned(),
            "armor" => "坦克".to_owned(),
            "aircraft" => "飞机".to_owned(),
            "naval_vessel" => "舰船".to_owned(),
            "convoy" => "运输船".to_owned(),
            "train" => "铁路车辆".to_owned(),
            _ => equipment_id.to_owned(),
        }
    }

    fn logistics_equipment_ids(db: &hoi4_content::V6Database) -> Vec<String> {
        const ORDER: [&str; 12] = [
            "infantry_equipment",
            "artillery",
            "anti_tank",
            "anti_air",
            "support_equipment",
            "motorized",
            "mechanized",
            "armor",
            "aircraft",
            "naval_vessel",
            "convoy",
            "train",
        ];
        let mut ids: Vec<String> = ORDER.into_iter().map(str::to_owned).collect();
        for pm in &db.production_methods {
            if let Some(eq) = &pm.equipment_output {
                if !ids.contains(&eq.equipment_category) {
                    ids.push(eq.equipment_category.clone());
                }
            }
        }
        ids
    }

    fn logistics_equipment_unit_cost_rm(equipment_id: &str) -> f64 {
        match equipment_id.to_ascii_lowercase().as_str() {
            e if e.contains("tank") || e.contains("armor") => 80_000.0,
            e if e.contains("fighter")
                || e.contains("bomber")
                || e.contains("plane")
                || e.contains("aircraft") =>
            {
                120_000.0
            }
            e if e.contains("artillery") => 18_000.0,
            e if e.contains("truck") || e.contains("motorized") => 12_000.0,
            e if e.contains("support") => 8_000.0,
            e if e.contains("ship") || e.contains("naval") => 250_000.0,
            e if e.contains("convoy") => 60_000.0,
            e if e.contains("train") => 40_000.0,
            _ => 4_000.0,
        }
    }

    fn logistics_building_output_ratio(
        building: &hoi4_state::Building,
        pms: &[&hoi4_content::ProductionMethodDef],
        market: &hoi4_state::market::NationalMarket,
    ) -> f32 {
        if building.level == 0 {
            return 0.0;
        }
        let mut total_needed = 0.0;
        let mut total_filled = 0.0;
        for class_idx in 0..hoi4_state::PopClass::COUNT {
            let needed = pms
                .iter()
                .map(|pm| pm.employment_demand.get(class_idx).copied().unwrap_or(0) as f32)
                .sum::<f32>()
                * building.level as f32;
            let filled = building.employment.get(class_idx).copied().unwrap_or(0) as f32;
            total_needed += needed;
            total_filled += filled.min(needed);
        }
        let employment_ratio = if total_needed <= 0.0 {
            1.0
        } else if total_filled <= 0.0 {
            // Logistics is a status panel; before the first labor tick, V6
            // buildings may be valid but have not received an employment
            // snapshot yet. Show their readable capacity instead of all zeroes.
            1.0
        } else {
            (total_filled / total_needed).clamp(0.0, 1.0)
        };
        let market_empty = market.supply.values().all(|v| v.abs() <= f32::EPSILON)
            && market.stockpile.values().all(|v| v.abs() <= f32::EPSILON);
        let input_ratio = pms
            .iter()
            .flat_map(|pm| {
                pm.input_good_ids
                    .iter()
                    .enumerate()
                    .map(move |(i, good_id)| (pm, i, good_id))
            })
            .fold(1.0_f32, |ratio, (pm, i, good_id)| {
                let demand = pm.input_good_amounts.get(i).copied().unwrap_or(0.0)
                    * building.level as f32
                    * employment_ratio;
                if demand <= 0.0 || market_empty {
                    ratio
                } else {
                    let available = market.supply.get(good_id).copied().unwrap_or(0.0)
                        + market.stockpile.get(good_id).copied().unwrap_or(0.0);
                    ratio.min((available / demand).clamp(0.0, 1.0))
                }
            });
        employment_ratio * input_ratio
    }

    fn logistics_resource_entries(
        world: &World,
        player: usize,
    ) -> Vec<hoi4_ui::logistics_panel::ResourceEntry> {
        let market = world.countries.market.markets.get(player);
        let country_id = hoi4_state::CountryId(player as u16);
        hoi4_data::ResourceKind::all()
            .iter()
            .map(|k| {
                let key = k.as_str();
                let market_produced = market
                    .and_then(|m| m.supply.get(key).copied())
                    .unwrap_or(0.0)
                    + market
                        .and_then(|m| m.imports.get(key).copied())
                        .unwrap_or(0.0);
                let static_produced = world
                    .data
                    .states
                    .iter()
                    .enumerate()
                    .filter(|(idx, _)| world.states.owners.get(*idx) == Some(&country_id))
                    .flat_map(|(_, state)| state.resources.iter())
                    .filter(|(kind, _)| kind == k)
                    .map(|(_, amount)| *amount)
                    .sum::<f32>();
                let produced = if market_produced > 0.0 {
                    market_produced
                } else {
                    static_produced
                };
                let consumed = market
                    .and_then(|m| m.demand.get(key).copied())
                    .unwrap_or(0.0)
                    + market
                        .and_then(|m| m.exports.get(key).copied())
                        .unwrap_or(0.0);
                let stored = market
                    .and_then(|m| m.stockpile.get(key).copied())
                    .unwrap_or(0.0);
                hoi4_ui::logistics_panel::ResourceEntry {
                    name: key.to_owned(),
                    produced,
                    consumed,
                    stored,
                }
            })
            .collect()
    }

    fn logistics_v6_military_outputs(
        world: &World,
        db: &hoi4_content::V6Database,
        player: usize,
    ) -> (HashMap<String, f32>, HashMap<String, Vec<String>>) {
        let country_id = hoi4_state::CountryId(player as u16);
        let completed_techs = world.countries.completed_techs[player].clone();
        let market = &world.countries.market.markets[player];
        let mut outputs: HashMap<String, f32> = HashMap::new();
        let mut sources: HashMap<String, Vec<String>> = HashMap::new();
        for building in &world.countries.buildings_v6.buildings {
            let state_idx = building.state.0 as usize;
            if state_idx >= world.states.count
                || world.states.owners[state_idx] != country_id
                || building.level == 0
            {
                continue;
            }
            let pms = hoi4_content::active_pms_for_building(building, db)
                .into_iter()
                .filter(|pm| {
                    let tech_ok = pm
                        .unlocked_by
                        .as_ref()
                        .map(|tech| completed_techs.contains(tech))
                        .unwrap_or(true);
                    let law_ok = pm
                        .required_law
                        .as_ref()
                        .map(|(cat, law_id)| {
                            world.countries.law_store.law_sets[player].0
                                [Self::v6_law_category_index(*cat)]
                            .current
                                == *law_id
                        })
                        .unwrap_or(true);
                    tech_ok && law_ok
                })
                .collect::<Vec<_>>();
            if pms.iter().all(|pm| pm.equipment_output.is_none()) {
                continue;
            }
            let ratio = Self::logistics_building_output_ratio(building, &pms, market);
            let state_name = world
                .states
                .names
                .get(state_idx)
                .cloned()
                .unwrap_or_default();
            let building_name = Self::v6_building_name(db, &building.building_def_id);
            for pm in pms {
                let Some(eq) = &pm.equipment_output else {
                    continue;
                };
                let daily = eq.daily_per_level
                    * pm.throughput_modifier.max(0.0)
                    * building.level as f32
                    * ratio;
                if daily <= 0.0 {
                    continue;
                }
                *outputs.entry(eq.equipment_category.clone()).or_insert(0.0) += daily;
                let source = if state_name.is_empty() {
                    format!("{} Lv{} +{:.1}/日", building_name, building.level, daily)
                } else {
                    format!(
                        "{} {} Lv{} +{:.1}/日",
                        state_name, building_name, building.level, daily
                    )
                };
                sources
                    .entry(eq.equipment_category.clone())
                    .or_default()
                    .push(source);
            }
        }
        for rows in sources.values_mut() {
            rows.truncate(3);
        }
        (outputs, sources)
    }

    fn logistics_force_needs(
        world: &World,
        player: usize,
    ) -> (HashMap<String, f32>, HashMap<String, f32>) {
        let country_id = hoi4_state::CountryId(player as u16);
        let tag = world.country_tag(country_id).map(|s| s.to_owned());
        let templates = tag
            .as_deref()
            .and_then(|t| world.data.division_templates.get(t));
        let mut total_need: HashMap<String, f32> = HashMap::new();
        let mut replenishment_need: HashMap<String, f32> = HashMap::new();
        let Some(templates) = templates else {
            return (total_need, replenishment_need);
        };
        for div_idx in 0..world.divisions.count {
            if world.divisions.owners[div_idx] != country_id {
                continue;
            }
            let tpl_idx = world.divisions.template_indices[div_idx] as usize;
            let Some(template) = templates.get(tpl_idx) else {
                continue;
            };
            let strength = world
                .divisions
                .strength
                .get(div_idx)
                .copied()
                .unwrap_or(1.0)
                .clamp(0.0, 1.0);
            let gap = 1.0 - strength;
            for (equipment, qty) in
                hoi4_logic::economy::stockpile::template_equipment_needs(template, &world.data)
            {
                let qty = qty as f32;
                *total_need.entry(equipment.clone()).or_insert(0.0) += qty;
                if gap > 0.0 && !world.divisions.in_combat[div_idx] {
                    *replenishment_need.entry(equipment).or_insert(0.0) += qty * gap.min(0.01);
                }
            }
        }
        (total_need, replenishment_need)
    }

    fn logistics_training_shortfalls(
        world: &World,
        econ: &hoi4_logic::economy::EconomyState,
        player: usize,
    ) -> HashMap<String, f32> {
        let mut shortfalls: HashMap<String, f32> = HashMap::new();
        let country_id = hoi4_state::CountryId(player as u16);
        let Some(tag) = world.country_tag(country_id) else {
            return shortfalls;
        };
        let Some(templates) = world.data.division_templates.get(tag) else {
            return shortfalls;
        };
        let Some(queue) = econ.training_queues.get(player) else {
            return shortfalls;
        };

        for item in queue {
            let Some(template) = templates.get(item.template_id as usize) else {
                continue;
            };
            for (equipment, qty) in
                hoi4_logic::economy::stockpile::template_equipment_needs(template, &world.data)
            {
                let needed = qty as f32 * item.count.max(1) as f32;
                let allocated = item
                    .equipment_allocated
                    .get(&equipment)
                    .copied()
                    .unwrap_or(0.0);
                let missing = (needed - allocated).max(0.0);
                if missing > 0.0 {
                    *shortfalls.entry(equipment).or_insert(0.0) += missing;
                }
            }
        }
        shortfalls
    }

    fn v6_active_pm_id_for_group(
        db: &hoi4_content::V6Database,
        building: &hoi4_state::Building,
        group: &str,
    ) -> String {
        building
            .active_pm_by_group
            .iter()
            .find(|active| active.group == group)
            .map(|active| active.pm_id.clone())
            .or_else(|| {
                hoi4_content::active_pms_for_building(building, db)
                    .into_iter()
                    .find(|pm| hoi4_content::production_method_group(pm) == group)
                    .map(|pm| pm.id.clone())
            })
            .unwrap_or_else(|| building.active_pm.clone())
    }

    fn v6_pm_lock_reason(
        world: &World,
        db: &hoi4_content::V6Database,
        player: usize,
        pm: &hoi4_content::ProductionMethodDef,
    ) -> Option<String> {
        if let Some(tech_id) = &pm.unlocked_by {
            if !world.countries.completed_techs[player].contains(tech_id) {
                let tech_name = db
                    .technologies
                    .iter()
                    .find(|tech| tech.id == *tech_id)
                    .map(|tech| tech.name.as_str())
                    .unwrap_or(tech_id.as_str());
                return Some(format!("科技锁定：需要「{}」", tech_name));
            }
        }
        if let Some(reason) = Self::v6_required_law_label(db, pm.required_law.as_ref()) {
            if let Some((cat, law_id)) = &pm.required_law {
                let current = &world.countries.law_store.law_sets[player].0
                    [Self::v6_law_category_index(*cat)]
                .current;
                if current != law_id {
                    return Some(reason);
                }
            }
        }
        None
    }

    fn v6_pm_prediction(
        db: &hoi4_content::V6Database,
        pm: &hoi4_content::ProductionMethodDef,
        level: u8,
    ) -> String {
        let mut parts = Vec::new();
        let output = pm
            .output_good_ids
            .iter()
            .enumerate()
            .take(2)
            .map(|(i, good_id)| {
                let amount = pm.output_good_amounts.get(i).copied().unwrap_or(0.0)
                    * pm.throughput_modifier.max(0.0)
                    * level as f32;
                format!("{} +{:.0}/d", Self::v6_good_name(db, good_id), amount)
            })
            .collect::<Vec<_>>()
            .join("、");
        if !output.is_empty() {
            parts.push(format!("产出 {}", output));
        }
        let input = pm
            .input_good_ids
            .iter()
            .enumerate()
            .take(2)
            .map(|(i, good_id)| {
                let amount = pm.input_good_amounts.get(i).copied().unwrap_or(0.0) * level as f32;
                format!("{} -{:.0}/d", Self::v6_good_name(db, good_id), amount)
            })
            .collect::<Vec<_>>()
            .join("、");
        if !input.is_empty() {
            parts.push(format!("投入 {}", input));
        }
        let workers: u32 = pm.employment_demand.iter().sum::<u32>() * level as u32;
        if workers > 0 {
            parts.push(format!("就业 {}", workers));
        }
        parts.join("；")
    }

    fn v6_pm_groups_for_building(
        world: &World,
        db: &hoi4_content::V6Database,
        player: usize,
        building: &hoi4_state::Building,
    ) -> Vec<hoi4_ui::construction_v6_panel::ProductionMethodGroupV6Entry> {
        let mut grouped: BTreeMap<String, Vec<&hoi4_content::ProductionMethodDef>> =
            BTreeMap::new();
        for pm in db
            .production_methods
            .iter()
            .filter(|pm| pm.building_id == building.building_def_id)
        {
            grouped
                .entry(hoi4_content::production_method_group(pm).to_owned())
                .or_default()
                .push(pm);
        }

        grouped
            .into_iter()
            .map(|(group_id, mut pms)| {
                pms.sort_by(|a, b| a.name.cmp(&b.name));
                let active_pm = Self::v6_active_pm_id_for_group(db, building, &group_id);
                let group_name = pms
                    .first()
                    .map(|pm| hoi4_content::production_method_group_name(pm))
                    .unwrap_or_else(|| hoi4_content::pm_group_name(&group_id).to_owned());
                let candidates = pms
                    .into_iter()
                    .map(
                        |pm| hoi4_ui::construction_v6_panel::ProductionMethodCandidateV6Entry {
                            pm_id: pm.id.clone(),
                            pm_name: localized_content_name(&pm.id, &pm.name),
                            locked_reason: Self::v6_pm_lock_reason(world, db, player, pm),
                            prediction: Self::v6_pm_prediction(db, pm, building.level),
                        },
                    )
                    .collect();
                hoi4_ui::construction_v6_panel::ProductionMethodGroupV6Entry {
                    group_id,
                    group_name,
                    active_pm,
                    candidates,
                }
            })
            .collect()
    }

    fn v6_set_building_pm_group(building: &mut hoi4_state::Building, group: &str, pm_id: &str) {
        if let Some(active) = building
            .active_pm_by_group
            .iter_mut()
            .find(|active| active.group == group)
        {
            active.pm_id = pm_id.to_owned();
        } else {
            building
                .active_pm_by_group
                .push(hoi4_state::ActiveProductionMethod {
                    group: group.to_owned(),
                    pm_id: pm_id.to_owned(),
                });
        }
        if group == "base" || building.active_pm.is_empty() {
            building.active_pm = pm_id.to_owned();
        }
    }

    fn v6_building_name(db: &hoi4_content::V6Database, building_id: &str) -> String {
        db.buildings
            .iter()
            .find(|bd| bd.id == building_id)
            .map(|bd| localized_content_name(&bd.id, &bd.name))
            .unwrap_or_else(|| building_id.to_owned())
    }

    fn v6_expected_profit_rm_daily(
        pms: &[&hoi4_content::ProductionMethodDef],
        level: u8,
        market: Option<&hoi4_state::market::NationalMarket>,
    ) -> f64 {
        hoi4_logic::economy::valuation::expected_profit_rm_daily(pms, level, market)
    }

    fn v6_flow_summary(
        db: &hoi4_content::V6Database,
        flows: &HashMap<String, f32>,
        sign: &str,
    ) -> String {
        let mut items: Vec<_> = flows.iter().collect();
        items.sort_by(|a, b| {
            b.1.abs()
                .partial_cmp(&a.1.abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        items
            .into_iter()
            .take(3)
            .filter(|(_, amount)| amount.abs() > 0.01)
            .map(|(good_id, amount)| {
                format!(
                    "{} {}{:.0}/d",
                    Self::v6_good_name(db, good_id),
                    sign,
                    amount.abs()
                )
            })
            .collect::<Vec<_>>()
            .join("、")
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

    fn world_size(&self) -> Vec2 {
        Vec2::new(
            self.world.map.province_map.width as f32 * WORLD_SCALE,
            self.world.map.province_map.height as f32 * WORLD_SCALE,
        )
    }

    fn init_render(&mut self, window: Arc<Window>) {
        let size = window.inner_size();
        // 4.1.bis.6: HiDPI handling. winit's `inner_size` returns physical pixels;
        // GUI layout (and our 2D shaders) must run in logical pixels so absolute
        // coordinates from `.gui` files (designed for 1920脳1080) resolve to the
        // right on-screen size regardless of the OS DPI scale.
        let dpi = window.scale_factor() as f32;
        let logical_w = size.width as f32 / dpi;
        let logical_h = size.height as f32 / dpi;
        // logical_w / logical_h are consumed when we build UI-pass-removed / TextPass / PanelPass below.
        println!(
            "[init] window physical={}x{} logical={:.0}x{:.0} dpi={:.2}",
            size.width, size.height, logical_w, logical_h, dpi
        );
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let surface = instance.create_surface(window.clone()).unwrap();

        let (adapter, device, queue) = pollster::block_on(async {
            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    compatible_surface: Some(&surface),
                    ..Default::default()
                })
                .await
                .unwrap();
            let supported = adapter.features();
            let want = wgpu::Features::TEXTURE_COMPRESSION_BC
                | wgpu::Features::TEXTURE_FORMAT_16BIT_NORM
                | wgpu::Features::TIMESTAMP_QUERY
                | wgpu::Features::TIMESTAMP_QUERY_INSIDE_PASSES
                | wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS;
            let required = wgpu::Features::TEXTURE_COMPRESSION_BC;
            let features = supported & want | required;
            if !features.contains(wgpu::Features::TEXTURE_FORMAT_16BIT_NORM) {
                eprintln!(
                    "[gpu] TEXTURE_FORMAT_16BIT_NORM not supported, heightmap will use R8Unorm fallback"
                );
            }
            if supported.contains(wgpu::Features::TIMESTAMP_QUERY) {
                eprintln!(
                    "[gpu] TIMESTAMP_QUERY supported; Phase 10 timing overlay will use GPU timestamps when pass/encoder writes are available"
                );
            } else {
                eprintln!(
                    "[gpu] TIMESTAMP_QUERY not supported; Phase 10 overlay will show CPU/draw-call budgets only"
                );
            }
            let (device, queue) = adapter
                .request_device(
                    &wgpu::DeviceDescriptor {
                        required_features: features,
                        ..Default::default()
                    },
                    None,
                )
                .await
                .unwrap();
            (adapter, device, queue)
        });
        self.heightmap_r16_supported = device
            .features()
            .contains(wgpu::Features::TEXTURE_FORMAT_16BIT_NORM);

        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(caps.formats[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        // Province index texture (R16Uint)
        let province_tex = device.create_texture_with_data(
            &queue,
            &wgpu::TextureDescriptor {
                label: Some("province_map"),
                size: wgpu::Extent3d {
                    width: self.world.map.province_map.width,
                    height: self.world.map.province_map.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R16Uint,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            bytemuck::cast_slice(&self.world.map.province_map.pixels),
        );
        let province_view = province_tex.create_view(&Default::default());

        // Heightmap texture. Phase 11.3: upgraded from R8Unorm to R16Unorm when
        // the GPU supports TEXTURE_FORMAT_16BIT_NORM. The source BMP is 8-bit;
        // each pixel is upcast to 16-bit (value << 8) to eliminate the 1/256
        // stepping that causes visible terraces on flat terrain at height_scale=4.0.
        let height_view = if self.heightmap_r16_supported {
            let heightmap_r16: Vec<u16> = self
                .world
                .map
                .heightmap
                .pixels
                .iter()
                .map(|&b| (b as u16) << 8)
                .collect();
            let height_tex = device.create_texture_with_data(
                &queue,
                &wgpu::TextureDescriptor {
                    label: Some("heightmap"),
                    size: wgpu::Extent3d {
                        width: self.world.map.heightmap.width,
                        height: self.world.map.heightmap.height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::R16Unorm,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                },
                wgpu::util::TextureDataOrder::LayerMajor,
                bytemuck::cast_slice(&heightmap_r16),
            );
            height_tex.create_view(&Default::default())
        } else {
            let height_tex = device.create_texture_with_data(
                &queue,
                &wgpu::TextureDescriptor {
                    label: Some("heightmap"),
                    size: wgpu::Extent3d {
                        width: self.world.map.heightmap.width,
                        height: self.world.map.heightmap.height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::R8Unorm,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                },
                wgpu::util::TextureDataOrder::LayerMajor,
                &self.world.map.heightmap.pixels,
            );
            height_tex.create_view(&Default::default())
        };

        // Terrain index texture (R8Uint) - per-pixel terrain.bmp index.
        let terrain_idx_tex = device.create_texture_with_data(
            &queue,
            &wgpu::TextureDescriptor {
                label: Some("terrain_idx"),
                size: wgpu::Extent3d {
                    width: self.world.map.terrain_bmp.width,
                    height: self.world.map.terrain_bmp.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R8Uint,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &self.world.map.terrain_bmp.pixels,
        );
        let terrain_idx_view = terrain_idx_tex.create_view(&Default::default());

        // 鈹€鈹€鈹€ 5.4 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€
        // SDF distance fields for borders. Computed once at startup.
        println!("Computing border SDFs (Chamfer 2-pass)...");
        let t_sdf = Instant::now();
        let country_sdf_data = compute_country_sdf(
            &self.world.map.province_map,
            &self.world.provinces.controllers,
        );
        let province_sdf_data = compute_province_sdf(&self.world.map.province_map);
        // 3.12.18 (2026-05-18): keep coast SDF in **raw pixel units**, not
        // normalized to its max. The shaders multiply `sample.r * 255` to
        // recover pixel distance ???that convention only works if the u8
        // value already *is* pixel distance (as country/province SDFs are).
        // Normalizing inflated `coast_dist_px` by `255 / coast_max`, which
        // turned `foam_band = 5 px` into a 30-50 px white ring at coasts
        // and produced the chunky "white edge" seen in zoom-out screenshots.
        let coast_sdf_data = compute_coast_sdf(&self.world.map.heightmap, 95);
        println!("  SDFs done in {:.2}s", t_sdf.elapsed().as_secs_f32());
        let map_w = self.world.map.province_map.width;
        let map_h = self.world.map.province_map.height;

        let country_sdf_tex = device.create_texture_with_data(
            &queue,
            &wgpu::TextureDescriptor {
                label: Some("country_sdf"),
                size: wgpu::Extent3d {
                    width: map_w,
                    height: map_h,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &country_sdf_data,
        );
        let country_sdf_view = country_sdf_tex.create_view(&Default::default());

        let province_sdf_tex = device.create_texture_with_data(
            &queue,
            &wgpu::TextureDescriptor {
                label: Some("province_sdf"),
                size: wgpu::Extent3d {
                    width: map_w,
                    height: map_h,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &province_sdf_data,
        );
        let province_sdf_view = province_sdf_tex.create_view(&Default::default());

        let coast_sdf_tex = device.create_texture_with_data(
            &queue,
            &wgpu::TextureDescriptor {
                label: Some("coast_sdf"),
                size: wgpu::Extent3d {
                    width: map_w,
                    height: map_h,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &coast_sdf_data,
        );
        let coast_sdf_view = coast_sdf_tex.create_view(&Default::default());

        // Phase 3.5: Load vanilla terrain atlas (map/terrain/atlas0.dds  ?2048x2048 BC3, 4脳4 tiles)
        let (terrain_atlas_view, _terrain_atlas_sampler) =
            load_terrain_atlas(&device, &queue, &self.path_cfg);

        // Phase 3.6.1: Load colormap (map/terrain/colormap.dds - continent natural color base)
        let (colormap_view, _colormap_sampler) = load_colormap(&device, &queue, &self.path_cfg);

        // Phase 3.6.6: Load rivers.bmp ???R8Unorm texture (per-pixel river level).
        let (rivers_view, _rivers_sampler) = load_rivers_texture(&device, &queue, &self.path_cfg);

        // Occupation overlay LUT (same layout as colour LUT but holds stripe colour + alpha).
        let occ_lut_data = build_occupation_lut(&self.world);
        // Use the same 256-wide layout as the colour LUT.
        let occ_lut_width: u32 = 256;
        let occ_lut_height: u32 =
            (occ_lut_data.len() as u32 / 4 + occ_lut_width - 1) / occ_lut_width;
        let occupation_lut_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("occupation_lut"),
            size: wgpu::Extent3d {
                width: occ_lut_width,
                height: occ_lut_height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let mut occ_padded = occ_lut_data;
        occ_padded.resize((occ_lut_width * occ_lut_height * 4) as usize, 0);
        upload_lut(
            &queue,
            &occupation_lut_texture,
            &occ_padded,
            occ_lut_width,
            occ_lut_height,
        );
        let occupation_lut_view = occupation_lut_texture.create_view(&Default::default());

        // RenderParams uniform buffer.
        let initial_params = RenderParams::new();
        let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("render_params"),
            contents: bytemuck::bytes_of(&initial_params),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Color LUT - 2D texture, 256 wide
        let player_cid = if self.player_country < self.world.countries.count {
            Some(hoi4_state::CountryId(self.player_country as u16))
        } else {
            None
        };
        let lut_data = build_color_lut(&self.world, self.map_mode, player_cid);
        let lut_width: u32 = 256;
        let lut_height: u32 = (lut_data.len() as u32 / 4 + lut_width - 1) / lut_width;
        let mut padded_lut = lut_data;
        padded_lut.resize((lut_width * lut_height * 4) as usize, 0);

        let lut_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("color_lut"),
            size: wgpu::Extent3d {
                width: lut_width,
                height: lut_height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        upload_lut(&queue, &lut_texture, &padded_lut, lut_width, lut_height);
        let lut_view = lut_texture.create_view(&Default::default());

        // Camera uniform
        // 4.1.bis.6 fix: aspect is unitless so logical vs physical math is the
        // same ???but use a single source (logical) to avoid future mismatches.
        self.camera.aspect = logical_w / logical_h.max(1.0);
        let cam_uniform = CameraUniform::from_camera(&self.camera, HEIGHT_SCALE, LAT_CORRECTION);
        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("camera"),
            contents: bytemuck::bytes_of(&cam_uniform),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Per-LOD chunk uniforms (just hold the grid count).
        let chunk_uniforms: [wgpu::Buffer; 3] = std::array::from_fn(|i| {
            let u = ChunkUniform {
                grid: LOD_GRID[i],
                _pad: [0; 3],
            };
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("chunk_uniform"),
                contents: bytemuck::bytes_of(&u),
                usage: wgpu::BufferUsages::UNIFORM,
            })
        });

        // Instance buffer (initial capacity = total chunks).
        let total_chunks = (CHUNKS_X * CHUNKS_Z) as u64;
        let instance_buffers: [wgpu::Buffer; 3] = std::array::from_fn(|_| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("chunk_instances"),
                size: total_chunks * std::mem::size_of::<ChunkInstance>() as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        });

        // Phase 3.12.3 ???shadow caster pipeline銆傚???camera_buffer + heightmap +
        // ???LOD chunk_uniform锛屽啓鍒颁竴寮犱笓???D32Float 2048脳2048 depth RT??
        let shadow_pass = passes::ShadowPass::new(
            &device,
            format,
            &camera_buffer,
            &height_view,
            &chunk_uniforms,
        );

        // Pipeline (with depth-stencil)
        let depth_format = wgpu::TextureFormat::Depth32Float;
        let depth_view =
            make_depth_view(&device, size.width.max(1), size.height.max(1), depth_format);

        // 鈹€鈹€鈹€ Phase 3.12.4 ???pdxmap-equivalent TerrainPass 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€
        // 鎶婂垰鎵嶅垱寤虹殑 19 涓棫 binding 瑙嗗浘浣滀负杈撳叆鍐嶆缁勭粐???vanilla pdxmap ???        // 3-bind-group 甯冨眬锛涘悓鏃舵寜 `MapResRole` 鍔犺浇 atlas_normal{0} +
        // world_normal.bmp + colormap_emissive + citylights_0 ???4 寮犳柊璐村浘???        // 褰撲换涓€鍔犺浇澶辫触鏃惰 pass 浼氳嚜鍔ㄧ敤 1脳1 fallback鈥斺€旀瀯閫犳案杩滄垚鍔??
        let global_uniform_buf = GlobalUniformBuffer::new(&device);
        let terrain_pass = {
            let inputs = passes::TerrainPassInputs {
                global_uniform_buffer: &global_uniform_buf.buffer,
                depth_format,
                lod_grid: LOD_GRID,
                shadow_map_view: &shadow_pass.depth_view,
                shadow_sampler: &shadow_pass.compare_sampler,
                colormap_view: &colormap_view,
                country_sdf_view: &country_sdf_view,
                province_sdf_view: &province_sdf_view,
                coast_sdf_view: &coast_sdf_view,
                occupation_lut_view: &occupation_lut_view,
                rivers_view: &rivers_view,
                heightmap_view: &height_view,
                province_view: &province_view,
                terrain_idx_view: &terrain_idx_view,
                terrain_atlas_view: &terrain_atlas_view,
                country_color_lut_view: &lut_view,
                map_set: None,
            };
            let terrain_pass = passes::TerrainPass::new(&device, &queue, &self.path_cfg, inputs);
            for w in &terrain_pass.load_warnings {
                println!("{}", w);
            }
            terrain_pass
        };

        // Build chunk grid from heightmap.
        let world_size = self.world_size();
        let chunk_grid = ChunkGrid::build(
            &self.world.map.heightmap,
            world_size,
            HEIGHT_SCALE,
            CHUNKS_X,
            CHUNKS_Z,
        );

        // 鈹€鈹€鈹€ 5.6 trees 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€
        println!("Generating tree instances...");
        let t_trees = Instant::now();
        // Phase 3.10.2: use map/trees.bmp + default.map's `tree = {3,4,7,10}`
        // when present; fall back to the legacy terrain.bmp routing only if
        // trees.bmp failed to load.
        let tree_data = if let Some(tree_bmp) = &self.world.map.tree_definition_bmp {
            generate_trees(
                tree_bmp,
                &self.world.map.tree_indices,
                &self.world.map.heightmap,
                WORLD_SCALE,
                HEIGHT_SCALE,
                3, // forest stride on trees.bmp (1650 wide)
                2, // jungle stride
            )
        } else {
            eprintln!("[trees] trees.bmp unavailable ???placing 0 trees");
            Vec::new()
        };
        println!(
            "  {} trees generated in {:.2}s",
            tree_data.len(),
            t_trees.elapsed().as_secs_f32()
        );
        let trees_count = tree_data.len() as u32;
        let trees_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("tree_instances"),
            contents: bytemuck::cast_slice(&tree_data),
            usage: wgpu::BufferUsages::VERTEX,
        });

        // Trees use camera + render params (procedural billboard, no texture needed).
        let trees_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("trees_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let trees_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("trees_bg"),
            layout: &trees_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: params_buffer.as_entire_binding(),
                },
            ],
        });

        let trees_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("trees_shader"),
            source: wgpu::ShaderSource::Wgsl(hoi4_render::SHADER_TREES_WGSL.into()),
        });
        let trees_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("trees_pipeline_layout"),
                bind_group_layouts: &[&trees_bgl],
                push_constant_ranges: &[],
            });
        let trees_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("trees_pipeline"),
            layout: Some(&trees_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &trees_shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<TreeInstance>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &[
                        // pos: vec3<f32> @ offset 0
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x3,
                            offset: 0,
                            shader_location: 0,
                        },
                        // scale: f32 @ offset 12
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32,
                            offset: 12,
                            shader_location: 1,
                        },
                        // tint: 4xu8 -> vec4<f32> via Unorm8x4 @ offset 16
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Unorm8x4,
                            offset: 16,
                            shader_location: 2,
                        },
                        // Phase 3.10.2: tree_type + pad as Uint8x2 @ offset 20.
                        // Shader reads `.x` for the species index.
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Uint8x2,
                            offset: 20,
                            shader_location: 3,
                        },
                        // Phase 3.10.2: slope (Snorm8x2) @ offset 22.
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Snorm8x2,
                            offset: 22,
                            shader_location: 4,
                        },
                    ],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &trees_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: HDR_FORMAT,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: depth_format,
                depth_write_enabled: false, // alpha-blended - no depth write
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });

        // 鈹€鈹€鈹€ 5.6 railways 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€
        println!("Generating railway vertices...");
        let rail_path = self
            .path_cfg
            .find("map/railways.txt")
            .unwrap_or_else(|| self.path_cfg.game_path().join("map/railways.txt"));
        let rail_text = std::fs::read_to_string(&rail_path).unwrap_or_default();
        let routes = parse_railways(&rail_text);
        let centroids = compute_province_centroids(&self.world.map.province_map);

        // Phase 3.5: Pre-compute per-country world-space label centroids.
        // O(num_provinces) one-shot; reused every frame for screen projection.
        let country_count = self.world.countries.count;
        let owners_for_labels: Vec<Option<usize>> = self
            .world
            .provinces
            .owners
            .iter()
            .map(|c| {
                if c.is_none() {
                    None
                } else {
                    Some(c.0 as usize)
                }
            })
            .collect();
        let country_labels = hoi4_render::mapname::compute_country_labels(
            &centroids,
            &owners_for_labels,
            country_count,
            WORLD_SCALE,
        );
        let labelled = country_labels.iter().filter(|l| l.is_some()).count();
        println!(
            "[mapname] computed {} country labels ({}/{} countries with owned provinces)",
            labelled, labelled, country_count
        );

        // Phase 3.10.3: per-country oriented bounding box (PCA over owned-
        // province pixels). Drives the 3D label quad's orientation + size.
        // O(map pixels), once at startup.
        let t_obb = Instant::now();
        let province_is_core: Vec<bool> = (0..self.world.provinces.count)
            .map(|pid| {
                let owner = self.world.provinces.owners[pid];
                if owner.is_none() {
                    return false;
                }
                let sid = self.world.provinces.state_of[pid];
                if sid.is_none() {
                    return false;
                }
                let si = sid.0 as usize;
                si < self.world.states.cores.len() && self.world.states.cores[si].contains(&owner)
            })
            .collect();
        let country_obbs = hoi4_render::mapname_3d::compute_country_obbs(
            &self.world.map.province_map,
            &owners_for_labels,
            &province_is_core,
            country_count,
        );
        let n_obb = country_obbs.iter().filter(|o| o.is_some()).count();
        println!(
            "[mapname_3d] OBB pass: {} countries with valid OBB in {:.2}s",
            n_obb,
            t_obb.elapsed().as_secs_f32()
        );

        // Phase 3.10.3: bake R8 atlas of country names with 1-pixel outline.
        // Names default to country tags; localised full names will arrive
        // with Phase 4.9. Falls back gracefully if no system font is
        // available ???the legacy 2D HUD path still works.
        let t_atlas = Instant::now();
        let names: Vec<Option<String>> = self
            .world
            .countries
            .tags
            .iter()
            .map(|tag| {
                if tag.is_empty() {
                    None
                } else {
                    Some(tag.clone())
                }
            })
            .collect();
        let mapname_atlas_opt = mapname_atlas::bake_country_name_atlas(&names, 36.0);
        if let Some(atlas) = &mapname_atlas_opt {
            println!(
                "[mapname_3d] atlas: {}x{} R8, {} entries baked in {:.2}s",
                atlas.width,
                atlas.height,
                atlas.count_baked(),
                t_atlas.elapsed().as_secs_f32()
            );
        } else {
            eprintln!("[mapname_3d] atlas bake failed (no system font?); 2D HUD fallback active");
        }

        let rail_verts = build_railway_vertices(
            &routes,
            &centroids,
            &self.world.map.heightmap,
            WORLD_SCALE,
            HEIGHT_SCALE,
            0.06, // lift above terrain
        );
        println!(
            "  {} railway segments ({} routes)",
            rail_verts.len() / 2,
            routes.len()
        );
        let railways_vertex_count = rail_verts.len() as u32;
        let railways_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("railway_verts"),
            contents: if rail_verts.is_empty() {
                &[0u8; 16]
            } else {
                bytemuck::cast_slice(&rail_verts)
            },
            usage: wgpu::BufferUsages::VERTEX,
        });

        let railways_params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("railway_params"),
            contents: bytemuck::bytes_of(&RailwayParams::default()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let frontlines_params_buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("frontline_params"),
                contents: bytemuck::bytes_of(&FrontlineParams {
                    opacity: 1.0,
                    _pad: [0.0; 3],
                }),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

        // Railways/frontlines use the shared camera uniform plus per-frame opacity.
        let rail_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("rail_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let frontlines_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("frontline_bg"),
            layout: &rail_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: frontlines_params_buffer.as_entire_binding(),
                },
            ],
        });
        let rail_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("rail_bg"),
            layout: &rail_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: railways_params_buffer.as_entire_binding(),
                },
            ],
        });
        let rail_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("rail_shader"),
            source: wgpu::ShaderSource::Wgsl(hoi4_render::SHADER_RAILWAYS_WGSL.into()),
        });
        let rail_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&rail_bgl],
            push_constant_ranges: &[],
        });
        let railways_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("railways_pipeline"),
            layout: Some(&rail_pl),
            vertex: wgpu::VertexState {
                module: &rail_shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<RailVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x3,
                            offset: 0,
                            shader_location: 0,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32,
                            offset: 12,
                            shader_location: 1,
                        },
                    ],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &rail_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: HDR_FORMAT,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::LineList,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: depth_format,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: Default::default(),
                bias: wgpu::DepthBiasState {
                    constant: 1,
                    slope_scale: 0.5,
                    clamp: 0.0,
                },
            }),
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });

        // 鈹€鈹€鈹€ 5.7 frontlines 鈹€鈹€鈹€鈹€鈹€
        println!("Generating frontlines...");
        let front_verts =
            generate_frontline_vertices(&self.world, &centroids, WORLD_SCALE, HEIGHT_SCALE);
        println!("  {} frontline border quads", front_verts.len() / 6);

        // Phase 2.9: Buildings - data generated, now with own pipeline (Phase 3.5).
        let building_instances =
            generate_buildings(&self.world, &centroids, WORLD_SCALE, HEIGHT_SCALE);
        let buildings_count = building_instances.len() as u32;
        let buildings_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("buildings"),
            contents: if building_instances.is_empty() {
                &[0u8; 16]
            } else {
                bytemuck::cast_slice(&building_instances)
            },
            usage: wgpu::BufferUsages::VERTEX,
        });
        println!("  {} building icons", buildings_count);

        let buildings_params_buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("building_params"),
                contents: bytemuck::bytes_of(&BuildingParams::default()),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });
        let buildings_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("buildings_bg"),
            layout: &rail_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: buildings_params_buffer.as_entire_binding(),
                },
            ],
        });

        // Buildings pipeline (16-byte per-instance: [f32;3] + f32)
        let buildings_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("buildings_shader"),
            source: wgpu::ShaderSource::Wgsl(hoi4_render::SHADER_BUILDINGS_WGSL.into()),
        });
        let buildings_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("buildings_pipeline"),
            layout: Some(&rail_pl),
            vertex: wgpu::VertexState {
                module: &buildings_shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: 16, // [f32;3] + f32 = 16 bytes
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &[
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: wgpu::VertexFormat::Float32x3,
                        },
                        wgpu::VertexAttribute {
                            offset: 12,
                            shader_location: 1,
                            format: wgpu::VertexFormat::Float32,
                        },
                    ],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &buildings_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: HDR_FORMAT,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: depth_format,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });

        // 鈹€鈹€鈹€ Phase I (CR-1.2 / CR-5) 鈥?HOI3 椋庢牸灞忓箷绌洪棿鍏电墝 pass锛堥粯璁ゅ惎鐢級 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€
        let hoi3_counter_pass = Hoi3CounterPass::new(
            &device,
            &queue,
            HDR_FORMAT,
            Hoi3CounterPass::DEFAULT_INITIAL_CAPACITY,
        );
        println!(
            "[hoi3_counter_v3] capacity = {}, enabled = {} (toggle: F8)",
            Hoi3CounterPass::DEFAULT_INITIAL_CAPACITY,
            hoi3_counter_pass.enabled()
        );

        // 鈹€鈹€鈹€ Phase 14 ???POI icon pass (factories / ports / airbases / resources) 鈹€
        let poi_icon_instances =
            generate_poi_icons(&self.world, &centroids, WORLD_SCALE, HEIGHT_SCALE);
        let poi_icon_pass = {
            let poi_instances = &poi_icon_instances;
            println!("  {} POI icon instances", poi_instances.len());
            let mut pass = PoiIconPass::new(
                &device,
                &queue,
                HDR_FORMAT,
                &camera_buffer,
                poi_instances.len().next_power_of_two().max(64) as u32,
            );
            for w in &pass.load_warnings {
                println!("{}", w);
            }
            pass.upload(&device, &queue, poi_instances, 3);
            println!(
                "[poi_icon] enabled = {}, {} instances uploaded",
                pass.enabled(),
                pass.instance_count()
            );
            Some(pass)
        };

        // Frontlines pipeline - actual contact-border strip triangles with per-vertex colour.
        let frontlines_vertex_count = front_verts.len() as u32;
        let frontlines_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("frontline_verts"),
            contents: if front_verts.is_empty() {
                &[0u8; 16]
            } else {
                bytemuck::cast_slice(&front_verts)
            },
            usage: wgpu::BufferUsages::VERTEX,
        });
        let front_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("front_shader"),
            source: wgpu::ShaderSource::Wgsl(hoi4_render::SHADER_FRONTLINES_WGSL.into()),
        });
        let frontlines_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("frontlines_pipeline"),
            layout: Some(&rail_pl),
            vertex: wgpu::VertexState {
                module: &front_shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<FrontVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x3,
                            offset: 0,
                            shader_location: 0,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Unorm8x4,
                            offset: 12,
                            shader_location: 1,
                        },
                    ],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &front_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: HDR_FORMAT,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: depth_format,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: Default::default(),
                bias: wgpu::DepthBiasState {
                    constant: 4,
                    slope_scale: 2.0,
                    clamp: 0.0,
                },
            }),
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });

        // V5 鏀跺彛锛歎I-pass-removed 宸插垹闄わ紙vanilla GUI command 娓叉煋绠＄嚎锛???        // Phase 3.5: Text pass
        let text_pass = TextPass::new(&device, format, logical_w, logical_h);

        // Phase 4.2 (redesign): Panel pass (鍦嗚鐭╁舰 SDF) + Flag bank (TGA 鍔犺浇)
        let panel_pass = PanelPass::new(&device, format, logical_w, logical_h);
        let mut flag_bank = FlagBank::new(&device, &queue);

        // Flag pipeline: 澶嶇敤 UI shader 姒傚康浣嗙嫭绔嬪疄渚嬶紝鍥犱负姣忎釜 flag 鐢ㄧ嫭绔嬬汗??
        let flag_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("flag_shader"),
            source: wgpu::ShaderSource::Wgsl(
                r#"
                struct U { screen_size: vec2<f32> };
                @group(0) @binding(0) var<uniform> u: U;
                @group(0) @binding(1) var t: texture_2d<f32>;
                @group(0) @binding(2) var s: sampler;
                struct VsIn { @location(0) pos: vec2<f32>, @location(1) uv: vec2<f32> };
                struct VsOut { @builtin(position) clip_pos: vec4<f32>, @location(0) uv: vec2<f32> };
                @vertex fn vs_main(in: VsIn) -> VsOut {
                    var o: VsOut;
                    let nx = (in.pos.x / u.screen_size.x) * 2.0 - 1.0;
                    let ny = 1.0 - (in.pos.y / u.screen_size.y) * 2.0;
                    o.clip_pos = vec4<f32>(nx, ny, 0.0, 1.0);
                    o.uv = in.uv;
                    return o;
                }
                @fragment fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
                    return textureSample(t, s, in.uv);
                }
            "#
                .into(),
            ),
        });
        let flag_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("flag_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let flag_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&flag_bgl],
            push_constant_ranges: &[],
        });
        let flag_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("flag_pipeline"),
            layout: Some(&flag_pl),
            vertex: wgpu::VertexState {
                module: &flag_shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: 16,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            offset: 8,
                            shader_location: 1,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                    ],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &flag_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });
        // 4.1.bis.6 fix (2026-05-16): flag layout coords come from
        // `last_country_layout` which is logical-space, so the shader needs
        // logical screen size to NDC-divide correctly.
        let flag_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("flag_uniforms"),
            contents: bytemuck::bytes_of(&[logical_w, logical_h]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let flag_vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("flag_verts"),
            size: 6 * 16,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        flag_bank.init_fallback(&device, &flag_bgl, &flag_uniform_buffer);

        // Phase 3.6.3: 3D mesh trees
        let (
            trees_mesh_pipeline,
            trees_mesh_bind_groups,
            trees_mesh_vertex_buffers,
            trees_mesh_index_buffers,
            trees_mesh_instance_buffers,
            trees_mesh_index_counts,
            trees_mesh_instance_counts,
        ) = setup_tree_mesh_pipeline(
            &device,
            &queue,
            &self.path_cfg,
            &tree_data,
            &camera_buffer,
            &params_buffer,
            HDR_FORMAT,
            depth_format,
        );

        // Phase 3.12.8 ???vanilla tree.shader full integration.
        // Constructed AFTER shadow_pass + global_uniform_buf so it can consume
        // the shared shadow depth view + comparison sampler. When all 3 tree
        // mesh types load successfully it replaces the old trees_mesh path.
        let (season_lerp, season_column) = {
            let sr = self.seasons.season_for_date(1, 1); // 1936-01-01 start
            (sr.season_lerp, sr.season_column)
        };
        let tree_full_pass = passes::TreeFullPass::new(
            &device,
            &queue,
            &self.path_cfg,
            &tree_data,
            passes::TreeFullPassInputs {
                global_uniform_buffer: &global_uniform_buf.buffer,
                shadow_depth_view: &shadow_pass.depth_view,
                shadow_compare_sampler: &shadow_pass.compare_sampler,
                depth_format,
                world_size: [self.camera.world_size.x, self.camera.world_size.y],
                season_lerp,
                season_column,
            },
        );
        println!(
            "[trees_full] TreeFullPass ready (any_loaded={}, {} warnings)",
            tree_full_pass.any_loaded,
            tree_full_pass.load_warnings.len()
        );
        for w in &tree_full_pass.load_warnings {
            eprintln!("  {}", w);
        }
        let tree_full_pass = if tree_full_pass.any_loaded {
            Some(tree_full_pass)
        } else {
            None
        };

        // Phase 3.12.5 ???pdxmesh pass for vanilla 3D buildings.
        // Constructed AFTER both `shadow_pass` (to share its depth view +
        // compare sampler) and `global_uniform_buf` (Phase 3.12.1 鍏变韩
        // GlobalFrameUniform). Loads civ_factory.mesh / factory.mesh /
        // dock_01.mesh + diffuse DDS + 1脳1脳6 grey cubemap fallback.
        let mut pdxmesh_pass = passes::PdxMeshPass::new(
            &device,
            &queue,
            &self.path_cfg,
            &global_uniform_buf.buffer,
            depth_format,
            &shadow_pass.depth_view,
            &shadow_pass.compare_sampler,
        );
        // Push the building instance data through (split by kind into 3
        // instance buffers ???civ / mil / dock).
        pdxmesh_pass.set_buildings(&device, &building_instances);
        println!(
            "[pdxmesh] {} building instances split across 3 mesh types (any_loaded={})",
            pdxmesh_pass.total_instances(),
            pdxmesh_pass.any_loaded
        );

        // Phase 3.12.7 ???vanilla river pass.
        // Reuses the same per-LOD instance buffers as the terrain pass; loads
        // 7 vanilla river textures (3 diffuse + 3 normal + masks) via FsAssetDb
        // with 1脳1 fallback. Drawn after terrain, before water ???replaces the
        // inline navy-blue overlay in terrain.wgsl with proper river rendering.
        let river_pass = passes::RiverPass::new(
            &device,
            &queue,
            &self.path_cfg,
            passes::RiverPassInputs {
                global_uniform_buffer: &global_uniform_buf.buffer,
                depth_format,
                lod_grid: LOD_GRID,
                heightmap_view: &height_view,
                rivers_view: &rivers_view,
                world_size: [self.camera.world_size.x, self.camera.world_size.y],
                height_scale: HEIGHT_SCALE,
            },
        );
        println!(
            "[river] RiverPass ready (any_loaded={}, {} warnings)",
            river_pass.any_loaded,
            river_pass.load_warnings.len()
        );
        for w in &river_pass.load_warnings {
            eprintln!("  {}", w);
        }

        // Phase 3.12.6 ???vanilla pdxwater pass.
        // Reuses the same per-LOD instance buffers as the terrain pass; loads
        // 7 vanilla water textures (lean1/2 + reflection + fow_water_spec +
        // colormap_water_0 + ice_diffuse + ice_noise_0) via FsAssetDb with
        // 1脳1 fallback per role. Drawn after terrain so it overdraws the
        // inline water branch in terrain.wgsl with full pdxwater shading.
        let mut water_pass = passes::WaterPass::new(
            &device,
            &queue,
            &self.path_cfg,
            passes::WaterPassInputs {
                global_uniform_buffer: &global_uniform_buf.buffer,
                depth_format,
                lod_grid: LOD_GRID,
                heightmap_view: &height_view,
                province_view: &province_view,
                coast_sdf_view: &coast_sdf_view,
                world_size: [self.camera.world_size.x, self.camera.world_size.y],
                height_scale: HEIGHT_SCALE,
            },
        );
        println!(
            "[water] WaterPass ready (any_loaded={}, loaded={}, fallback={}, critical_missing={}, {} warnings)",
            water_pass.any_loaded,
            water_pass.texture_load_stats.loaded,
            water_pass.texture_load_stats.fallback,
            water_pass.texture_load_stats.critical_missing,
            water_pass.load_warnings.len()
        );
        for w in &water_pass.load_warnings {
            eprintln!("  {}", w);
        }

        // Phase 3.12.9 (redesign) ???vanilla border pass using strip meshes.
        // CPU extracts border edges from province bitmap ???generates thin
        // quad-strip meshes that hug actual boundaries.
        let border_edges = hoi4_render::border_extract::extract_border_edges(&self.world);
        let border_meshes = hoi4_render::border_extract::generate_border_meshes(
            &border_edges,
            &self.world.map.heightmap,
            &hoi4_render::border_extract::StripParams {
                world_scale: WORLD_SCALE,
                height_scale: HEIGHT_SCALE,
                half_width: 0.009,
                y_bias: 0.018,
                tile_factor: 0.20,
                ..Default::default()
            },
        );
        println!(
            "[border] extracted {} edges ???{} mesh groups",
            border_edges.len(),
            border_meshes.len()
        );

        let border_pass = passes::BorderPass::new(
            &device,
            &queue,
            passes::BorderPassInputs {
                global_uniform_buffer: &global_uniform_buf.buffer,
                depth_format,
                meshes: &border_meshes,
                path_cfg: &self.path_cfg,
            },
        );
        println!(
            "[border] BorderPass ready (any_loaded={}, {} warnings)",
            border_pass.any_loaded,
            border_pass.load_warnings.len()
        );
        for w in &border_pass.load_warnings {
            eprintln!("  {}", w);
        }

        // Phase 3.12.11: Sky pass + EnvironmentMap cubemap.
        let sky_pass = passes::SkyPass::new(
            &device,
            &queue,
            &global_uniform_buf.buffer,
            HDR_FORMAT,
            depth_format,
            &self.path_cfg,
        );
        if sky_pass.loaded {
            water_pass.set_env_cubemap(&device, &sky_pass.cubemap_view);
            pdxmesh_pass.set_env_cubemap_with_shadow(
                &device,
                &sky_pass.cubemap_view,
                &shadow_pass.depth_view,
                &shadow_pass.compare_sampler,
            );
        }
        println!("[sky] SkyPass ready (cubemap_loaded={})", sky_pass.loaded);

        // Phase 3.12.10: Particle pass (combat smoke / factory chimneys / scorched earth).
        let particle_pass = passes::ParticlePass::new(
            &device,
            &queue,
            &global_uniform_buf.buffer,
            HDR_FORMAT,
            depth_format,
        );
        println!(
            "[particle] ParticlePass ready (max {})",
            hoi4_render::particles::MAX_PARTICLES
        );

        // Phase 16: Arrows family (maparrow / traderoute / strait).
        let mut maparrow_pass =
            passes::MapArrowPass::new(&device, &queue, &global_uniform_buf.buffer, depth_format);
        let mut traderoute_pass =
            passes::TradeRoutePass::new(&device, &queue, &global_uniform_buf.buffer, depth_format);
        let mut strait_pass =
            passes::StraitPass::new(&device, &queue, &global_uniform_buf.buffer, depth_format);

        let map_w = self.world.map.province_map.width as f32 * WORLD_SCALE;
        let map_d = self.world.map.province_map.height as f32 * WORLD_SCALE;
        let _ = (map_w, map_d);
        if self.world.player_armies.is_empty() {
            let mock_arrows = passes::maparrow::generate_mock_arrows([map_w, map_d]);
            maparrow_pass.set_arrows(&device, &queue, &mock_arrows);
        }

        // Generate mock trade routes for testing.
        let mock_trade_verts = passes::traderoute::generate_mock_trade_routes();
        traderoute_pass.set_routes(&device, &queue, &mock_trade_verts);

        // Build strait geometry from adjacency data.
        strait_pass.build_straits(
            &device,
            &queue,
            &self.world.map.special_adjacencies,
            &centroids,
            WORLD_SCALE,
        );
        println!(
            "[arrows] trade_verts={}, strait_verts={}",
            mock_trade_verts.len(),
            strait_pass.any_loaded as u32,
        );
        println!("[arrows] MapArrowPass / TradeRoutePass / StraitPass ready");

        // Phase 3.12.10: vanilla-equivalent 3D country-name label pass.
        let mapname_pass = mapname_atlas_opt.as_ref().map(|atlas| {
            passes::MapnamePass::new(
                &device,
                &queue,
                passes::MapnamePassInputs {
                    global_uniform_buffer: &global_uniform_buf.buffer,
                    depth_format,
                    obbs: &country_obbs,
                    atlas: Some(atlas),
                    world_scale: WORLD_SCALE,
                    label_y: HEIGHT_SCALE * 0.5,
                },
            )
        });
        let mapname_count = mapname_pass
            .as_ref()
            .map(|p| p.instance_count())
            .unwrap_or(0);
        println!(
            "[mapname] vanilla pass ready, {} label instances",
            mapname_count
        );

        // Phase 3.12.13: province-name labels (zoom-gated).
        let t_prov_labels = Instant::now();
        let province_labels = hoi4_render::province_labels::compute_province_labels(
            &self.world.map.province_map,
            &self.world.map.definitions,
            8000,
        );
        let n_prov_labels = province_labels.iter().filter(|l| l.is_some()).count();
        println!(
            "[province_name] {} land province labels computed in {:.2}s",
            n_prov_labels,
            t_prov_labels.elapsed().as_secs_f32()
        );

        let t_prov_atlas = Instant::now();
        let prov_names: Vec<Option<String>> = {
            let _defs = &self.world.map.definitions;
            let state_names = &self.world.states.names;
            let state_of = &self.world.provinces.state_of;
            let mut out: Vec<Option<String>> = vec![None; province_labels.len()];
            for id in 1..province_labels.len() {
                if province_labels[id].is_none() {
                    continue;
                }
                let sid = if id < state_of.len() {
                    state_of[id]
                } else {
                    hoi4_state::ids::StateId::NONE
                };
                let name = if sid != hoi4_state::ids::StateId::NONE {
                    let si = sid.0 as usize;
                    if si < state_names.len() && !state_names[si].is_empty() {
                        Some(state_names[si].clone())
                    } else {
                        Some(format!("PROV{}", id))
                    }
                } else {
                    Some(format!("PROV{}", id))
                };
                out[id] = name;
            }
            out
        };
        let prov_atlas_opt = province_name_atlas::bake_province_name_atlas(&prov_names, 14.0);
        if let Some(atlas) = &prov_atlas_opt {
            println!(
                "[province_name] atlas: {}x{} R8, {} entries baked in {:.2}s",
                atlas.width,
                atlas.height,
                atlas.count_baked(),
                t_prov_atlas.elapsed().as_secs_f32()
            );
        } else {
            eprintln!("[province_name] atlas bake failed (no system font?)");
        }

        let prov_instances = if let Some(atlas) = &prov_atlas_opt {
            province_name_atlas::build_province_label_instances(
                &province_labels,
                atlas,
                WORLD_SCALE,
                HEIGHT_SCALE * 0.45,
                100,
            )
        } else {
            Vec::new()
        };
        let prov_inst_count = prov_instances.len();
        let province_name_pass = if prov_atlas_opt.is_some() {
            Some(passes::ProvinceNamePass::new(
                &device,
                &queue,
                passes::province_name::ProvinceNamePassInputs {
                    global_uniform_buffer: &global_uniform_buf.buffer,
                    depth_format,
                    atlas: prov_atlas_opt.as_ref(),
                    instances: &prov_instances,
                },
            ))
        } else {
            None
        };
        println!(
            "[province_name] pass ready, {} label instances",
            prov_inst_count
        );

        // 鈹€鈹€鈹€ Phase 3.12.1 鍏叡娓叉煋鍩虹璁炬柦 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€
        // 绂诲睆 HDR ???RT锛圧GBA16Float锛???D pass 鍐欏叆杩欓噷锛屼箣鍚庣敱 PostProcessChain
        // ???SimpleBlitPass 妗ユ帴???swap chain??
        let hdr_target = HdrTarget::new(&device, config.width, config.height);
        let simple_blit = SimpleBlitPass::new(&device, format, &hdr_target.view);
        let post_process = PostProcessChain::new(
            &device,
            &hdr_target.view,
            hdr_target.width,
            hdr_target.height,
            format,
        );
        let map_renderer = MapRenderer::new();
        let mut pass_registry = PassRegistry::new();
        map_renderer.register_passes(&mut pass_registry);
        let debug_render_overlay = DebugOverlay::new();
        let gpu_profiler = GpuTimestampProfiler::new(&device, &queue);
        println!(
            "[phase10] quality={} budget={:?} gpu_timestamp={}",
            self.map_quality_preset.as_str(),
            self.map_quality_preset.budget(),
            gpu_profiler
                .as_ref()
                .map(|profiler| profiler.status())
                .unwrap_or(GpuProfilerStatus::UNSUPPORTED)
                .summary()
        );
        println!(
            "[render] HDR offscreen RT ready ({}x{} {:?}); post-process mode = {:?}; {}",
            hdr_target.width,
            hdr_target.height,
            HDR_FORMAT,
            post_process.mode,
            post_process.calibration.summary()
        );

        // Egui UI overlay must target the swapchain format.
        let ui = hoi4_ui::UiState::new(&device, format, 1, &window);
        // Apply vanilla UI theme and tooltip timing.
        let theme_assets = hoi4_ui::theme::apply_vanilla_theme(&ui.ctx);
        hoi4_ui::loc::configure_tooltip_delay(&ui.ctx);
        println!(
            "[render] egui UI overlay ready (target_format={:?}, dpi={:.2})\n[ui] vanilla theme: latin={:?} cjk={:?}",
            format,
            window.scale_factor(),
            theme_assets.latin_serif_path,
            theme_assets.cjk_fallback_path,
        );

        // Load vanilla 9-slice window texture with a plain-color fallback.
        let nine_slice_window = match hoi4_ui::nine_slice::NineSlice::load_vanilla(
            &ui.ctx,
            &self.path_cfg,
            "gfx/interface/tiles/tiled_window.dds",
            hoi4_ui::nine_slice::NineSliceEdges::uniform(32.0),
            "vanilla_tiled_window",
        ) {
            Ok(ns) => {
                println!(
                    "[ui] 9-slice tiled_window loaded ({}x{}, edges=32 px)",
                    ns.size.x as u32, ns.size.y as u32
                );
                Some(ns)
            }
            Err(e) => {
                eprintln!("[ui] 9-slice tiled_window load failed: {e} (UI 璧扮函鑹?fallback)");
                None
            }
        };

        // Sprite icon bank lazily loads icons on first use.
        let mut icon_bank = hoi4_ui::icons::IconBank::new(ui.ctx.clone(), self.path_cfg.clone());
        icon_bank.add_search_dir("gfx/interface/ideas");
        icon_bank.add_search_dir("gfx/interface/idea_categories");
        icon_bank.add_search_dir("gfx/event_pictures");
        icon_bank.add_search_dir("gfx/interface");
        // Register DLC leader portrait directories before base-game directories.
        {
            let dlc_dir = self.path_cfg.game_path().join("dlc");
            if dlc_dir.is_dir() {
                if let Ok(dlcs) = std::fs::read_dir(&dlc_dir) {
                    for dlc in dlcs.flatten() {
                        let leaders = dlc.path().join("gfx").join("leaders");
                        if leaders.is_dir() {
                            if let Ok(tags) = std::fs::read_dir(&leaders) {
                                for tag_dir in tags.flatten() {
                                    if tag_dir.path().is_dir() {
                                        let rel = format!(
                                            "dlc/{}/gfx/leaders/{}",
                                            dlc.file_name().to_string_lossy(),
                                            tag_dir.file_name().to_string_lossy()
                                        );
                                        icon_bank.add_search_dir(rel);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        icon_bank.add_leader_dirs(self.world.countries.tags.iter().filter(|t| !t.is_empty()));
        // Preload known GER focus icons used by the startup banner.
        for name in &["GFX_focus_GER_anschluss", "GFX_focus_GER_afrikakorps"] {
            if icon_bank.get_or_load(name).is_some() {
                if let Some(sz) = icon_bank.size_of(name) {
                    println!("[ui] icon preloaded: {name} ({}x{})", sz[0], sz[1]);
                }
            } else if let Some(reason) = icon_bank.missing_reason(name) {
                eprintln!("[ui] icon preload failed: {name}: {reason}");
            }
        }

        // Startup banner: verify country leader coverage for the seven majors.
        {
            let major_tags = ["GER", "SOV", "ENG", "FRA", "ITA", "JAP", "USA"];
            let mut hit = 0u32;
            let mut miss_list: Vec<&str> = Vec::new();
            for tag in &major_tags {
                let cid = self.world.country(tag);
                let has_leader = cid.and_then(|c| self.world.country_leader(c)).is_some();
                if has_leader {
                    hit += 1;
                } else {
                    miss_list.push(tag);
                }
            }
            println!(
                "[J.1] country_leader coverage: {hit}/{} majors hit{}",
                major_tags.len(),
                if miss_list.is_empty() {
                    String::new()
                } else {
                    format!(" 鈥?missing: {}", miss_list.join(", "))
                }
            );
        }

        let province_pixel_bounds =
            render_collect::build_province_pixel_bounds(&self.world.map.province_map);

        self.state = Some(RenderState {
            surface,
            device,
            queue,
            config,
            terrain_pass,
            camera_buffer,
            params_buffer,
            instance_buffers,
            instance_capacity: [total_chunks as u32; 3],
            terrain_bucket_signature: [0; 3],
            terrain_bucket_counts: [0; 3],
            lut_texture,
            lut_width,
            lut_height,
            occupation_lut_texture,
            depth_view,
            depth_format,
            chunk_grid,
            trees_pipeline,
            trees_bind_group,
            trees_buffer,
            trees_count,
            railways_pipeline,
            railways_bind_group: rail_bg,
            railways_params_buffer,
            railways_buffer,
            railways_vertex_count,
            hoi3_counter_pass,
            unit_counter_centroids: centroids.clone(),
            province_pixel_bounds,
            frontlines_pipeline,
            frontlines_bind_group: frontlines_bg,
            frontlines_buffer,
            frontlines_vertex_count,
            frontlines_params_buffer,
            buildings_buffer,
            buildings_params_buffer,
            buildings_bind_group,
            buildings_count,
            buildings_pipeline,
            pdxmesh_pass,
            water_pass,
            river_pass,
            border_pass,
            sky_pass,
            particle_pass,
            maparrow_pass,
            traderoute_pass,
            strait_pass,
            poi_icon_pass,
            poi_icon_instances,
            poi_zoom_bucket: 3,
            text_pass,
            panel_pass,
            flag_bank,
            flag_pipeline,
            flag_bgl,
            flag_uniform_buffer,
            flag_vertex_buffer,
            trees_mesh_pipeline,
            trees_mesh_bind_groups,
            trees_mesh_vertex_buffers,
            trees_mesh_index_buffers,
            trees_mesh_instance_buffers,
            trees_mesh_index_counts,
            trees_mesh_instance_counts,
            tree_full_pass,
            tree_lod_uploaded: false,
            tree_lod_last_cam_pos: [f32::NAN, f32::NAN, f32::NAN],
            country_labels,
            mapname_pass,
            mapname_atlas: mapname_atlas_opt,
            province_name_pass,
            // Phase 3.12.1
            hdr_target,
            global_uniform_buf,
            simple_blit,
            post_process,
            // Phase 3.12.3
            shadow_pass,
            map_renderer,
            pass_registry,
            debug_render_overlay,
            gpu_profiler,
            ui,
            nine_slice_window,
            icon_bank,
            window,
        });

        // Apply fullscreen setting after render state initialization.
        if self.settings.fullscreen {
            if let Some(s) = self.state.as_ref() {
                s.window.set_fullscreen(Some(Fullscreen::Borderless(None)));
            }
        }
        if let Some(s) = self.state.as_mut() {
            // Phase 3.5: Load font for text rendering
            let bgl = TextPass::bind_group_layout(&s.device);
            s.text_pass
                .load_font(&s.device, &s.queue, &self.path_cfg, &bgl);
        }
    }

    // V5 鏀跺彛锛?026-05-18锛夛細鍒犻櫎 rebuild_topbar_runtime 鈥斺€?vanilla topbar.gui /
    // countrypoliticsview.gui 瑙ｆ瀽璺嚎宸插仠鐢紝topbar / 鏀挎不闈㈡澘娓叉煋鎺ㄨ繜鍒伴樁娈?B銆?
    fn refresh_lut(&self) {
        let s = match &self.state {
            Some(s) => s,
            None => return,
        };
        let player_cid = if self.player_country < self.world.countries.count {
            Some(hoi4_state::CountryId(self.player_country as u16))
        } else {
            None
        };
        let lut_data = build_color_lut(&self.world, self.map_mode, player_cid);
        let mut padded = lut_data;

        // E.4: Combat province red pulse highlight.
        {
            let t = (self.world.elapsed_hours as f32 * 0.3).sin() * 0.5 + 0.5; // 0..1 pulse
            let red_blend = (80.0 + t * 80.0) as u8; // 80..160
            for div_idx in 0..self.world.divisions.count {
                if !self.world.divisions.in_combat[div_idx] {
                    continue;
                }
                let prov = self.world.divisions.locations[div_idx].0 as usize;
                let o = prov * 4;
                if o + 3 < padded.len() {
                    padded[o] = padded[o].saturating_add(red_blend);
                    padded[o + 1] = padded[o + 1].saturating_sub(40);
                    padded[o + 2] = padded[o + 2].saturating_sub(40);
                }
            }
        }

        // P1: selecting a province also highlights its whole state. The single
        // clicked province still gets the shader pulse; this LUT tint makes the
        // administrative state boundary readable without adding another pass.
        for pid in self
            .selected_province_ids
            .iter()
            .chain(self.construction_highlight_province_ids.iter())
        {
            let o = *pid as usize * 4;
            if o + 3 < padded.len() {
                padded[o] = padded[o].saturating_add(42);
                padded[o + 1] = padded[o + 1].saturating_add(32);
                padded[o + 2] = padded[o + 2].saturating_sub(18);
            }
        }

        padded.resize((s.lut_width * s.lut_height * 4) as usize, 0);
        upload_lut(&s.queue, &s.lut_texture, &padded, s.lut_width, s.lut_height);
        s.window.request_redraw();
    }

    /// Rebuild and upload the occupation stripe LUT.
    fn refresh_occupation_lut(&self) {
        let s = match &self.state {
            Some(s) => s,
            None => return,
        };
        let mut occ_padded = build_occupation_lut(&self.world);
        occ_padded.resize((s.lut_width * s.lut_height * 4) as usize, 0);
        upload_lut(
            &s.queue,
            &s.occupation_lut_texture,
            &occ_padded,
            s.lut_width,
            s.lut_height,
        );
        s.window.request_redraw();
    }

    fn rebuild_country_labels_and_refresh(&mut self) {
        let s = match &mut self.state {
            Some(s) => s,
            None => {
                println!("[map] rebuild_country_labels_and_refresh: state=None, SKIP");
                return;
            }
        };
        let centroids = &s.unit_counter_centroids;
        let country_count = self.world.countries.count;
        let owners_for_labels: Vec<Option<usize>> = self
            .world
            .provinces
            .owners
            .iter()
            .map(|oid| {
                if oid.is_none() {
                    None
                } else {
                    Some(oid.0 as usize)
                }
            })
            .collect();
        s.country_labels = hoi4_render::mapname::compute_country_labels(
            centroids,
            &owners_for_labels,
            country_count,
            WORLD_SCALE,
        );

        // Rebuild 3D country label instances from current owners.
        if let (Some(mapname_pass), Some(atlas)) =
            (s.mapname_pass.as_mut(), s.mapname_atlas.as_ref())
        {
            let province_is_core: Vec<bool> = (0..self.world.provinces.count)
                .map(|pid| {
                    let owner = self.world.provinces.owners[pid];
                    if owner.is_none() {
                        return false;
                    }
                    let sid = self.world.provinces.state_of[pid];
                    if sid.is_none() {
                        return false;
                    }
                    let si = sid.0 as usize;
                    si < self.world.states.cores.len()
                        && self.world.states.cores[si].contains(&owner)
                })
                .collect();
            let new_obbs = hoi4_render::mapname_3d::compute_country_obbs(
                &self.world.map.province_map,
                &owners_for_labels,
                &province_is_core,
                country_count,
            );
            mapname_pass.rebuild_instances(
                &s.device,
                &s.queue,
                &new_obbs,
                atlas,
                WORLD_SCALE,
                HEIGHT_SCALE * 0.5,
            );
        }
        // Now refresh the LUT
        let player_cid = if self.player_country < self.world.countries.count {
            Some(hoi4_state::CountryId(self.player_country as u16))
        } else {
            None
        };
        let lut_data = build_color_lut(&self.world, self.map_mode, player_cid);
        let mut padded = lut_data;
        {
            let t = (self.world.elapsed_hours as f32 * 0.3).sin() * 0.5 + 0.5;
            let red_blend = (80.0 + t * 80.0) as u8;
            for div_idx in 0..self.world.divisions.count {
                if !self.world.divisions.in_combat[div_idx] {
                    continue;
                }
                let prov = self.world.divisions.locations[div_idx].0 as usize;
                let o = prov * 4;
                if o + 3 < padded.len() {
                    padded[o] = padded[o].saturating_add(red_blend);
                    padded[o + 1] = padded[o + 1].saturating_sub(40);
                    padded[o + 2] = padded[o + 2].saturating_sub(40);
                }
            }
        }
        for pid in self
            .selected_province_ids
            .iter()
            .chain(self.construction_highlight_province_ids.iter())
        {
            let o = *pid as usize * 4;
            if o + 3 < padded.len() {
                padded[o] = padded[o].saturating_add(42);
                padded[o + 1] = padded[o + 1].saturating_add(32);
                padded[o + 2] = padded[o + 2].saturating_sub(18);
            }
        }
        padded.resize((s.lut_width * s.lut_height * 4) as usize, 0);
        upload_lut(&s.queue, &s.lut_texture, &padded, s.lut_width, s.lut_height);

        // Refresh occupation stripes when province controller data changes.
        let mut occ_padded = build_occupation_lut(&self.world);
        occ_padded.resize((s.lut_width * s.lut_height * 4) as usize, 0);
        upload_lut(
            &s.queue,
            &s.occupation_lut_texture,
            &occ_padded,
            s.lut_width,
            s.lut_height,
        );

        s.window.request_redraw();
    }

    fn refresh_map_if_province_ownership_changed(&mut self) {
        let owners_changed = self.map_refresh_owners != self.world.provinces.owners;
        let controllers_changed = self.map_refresh_controllers != self.world.provinces.controllers;
        if !owners_changed && !controllers_changed {
            return;
        }

        self.map_refresh_owners
            .clone_from(&self.world.provinces.owners);
        self.map_refresh_controllers
            .clone_from(&self.world.provinces.controllers);

        if owners_changed {
            self.rebuild_country_labels_and_refresh();
        } else {
            self.refresh_lut();
            self.refresh_occupation_lut();
        }
    }

    fn upload_camera(&self) {
        let s = match &self.state {
            Some(s) => s,
            None => return,
        };
        let cam = CameraUniform::from_camera(&self.camera, HEIGHT_SCALE, LAT_CORRECTION);
        s.queue
            .write_buffer(&s.camera_buffer, 0, bytemuck::bytes_of(&cam));
    }

    fn cursor_ndc_at(&self, x: f32, y: f32) -> Option<Vec2> {
        let s = self.state.as_ref()?;
        let dpi = s.window.scale_factor() as f32;
        let w = s.config.width as f32 / dpi.max(0.0001);
        let h = s.config.height as f32 / dpi.max(0.0001);
        if w <= 0.0 || h <= 0.0 {
            return None;
        }
        Some(Vec2::new((x / w) * 2.0 - 1.0, 1.0 - (y / h) * 2.0))
    }

    fn ground_point_at_cursor(&self, x: f32, y: f32) -> Option<Vec2> {
        let ndc = self.cursor_ndc_at(x, y)?;
        self.camera.pick_world_xz_at_height(ndc, HEIGHT_SCALE * 0.3)
    }

    fn zoom_at_cursor(&mut self, factor: f32) {
        let base_distance = self
            .smooth_zoom_target_distance
            .unwrap_or(self.camera.distance);
        self.smooth_zoom_target_distance =
            Some(self.camera.clamped_distance(base_distance * factor));
        self.smooth_zoom_anchor_mouse = self.last_mouse;
        if let Some(s) = &self.state {
            s.window.request_redraw();
        }
    }

    fn update_smooth_zoom(&mut self, dt: f32) {
        let Some(target_distance) = self.smooth_zoom_target_distance else {
            return;
        };
        if dt <= 0.0 {
            return;
        }

        let [mx, my] = self.smooth_zoom_anchor_mouse;
        let before = self.ground_point_at_cursor(mx, my);
        let response = 1.0 - (-SMOOTH_ZOOM_RESPONSE * dt).exp();
        let mut next_distance =
            self.camera.distance + (target_distance - self.camera.distance) * response;
        let snap_epsilon = (target_distance * 0.001).max(0.01);
        if (target_distance - next_distance).abs() <= snap_epsilon {
            next_distance = target_distance;
            self.smooth_zoom_target_distance = None;
        }

        self.camera.distance = self.camera.clamped_distance(next_distance);
        if let Some(before) = before {
            if let Some(after) = self.ground_point_at_cursor(mx, my) {
                self.camera.pan(before.x - after.x, before.y - after.y);
            }
        }
        self.upload_camera();
        if let Some(s) = &self.state {
            s.window.request_redraw();
        }
    }

    /// 3.12.15 fog-of-war for the on-map counter pass: the player only sees
    /// their own units, faction allies, neighbours, and active war
    /// belligerents. Returns `None` until a country has been selected ???that
    /// disables the filter (observer mode shows everyone, useful for
    /// development and during the main-menu phase).
    fn counter_visibility_signature(&self) -> u64 {
        let mut h = DefaultHasher::new();
        self.game_phase.hash(&mut h);
        self.settings.show_all_units.hash(&mut h);
        self.player_country.hash(&mut h);
        (self.world.elapsed_hours / 24).hash(&mut h);
        self.world.provinces.count.hash(&mut h);
        self.world.divisions.count.hash(&mut h);
        self.world.diplomacy.wars.len().hash(&mut h);
        self.world.diplomacy.factions.len().hash(&mut h);
        h.finish()
    }

    fn cached_counter_visibility(&mut self) -> (Option<HashSet<CountryId>>, Option<HashSet<u16>>) {
        if self.game_phase != GamePhase::Playing
            || self.settings.show_all_units
            || self.player_country >= self.world.countries.count
        {
            self.counter_visibility_cache.valid = false;
            return (None, None);
        }

        let sig = self.counter_visibility_signature();
        if self.counter_visibility_cache.valid && self.counter_visibility_cache.signature == sig {
            return (
                self.counter_visibility_cache.visible.clone(),
                self.counter_visibility_cache.spotted.clone(),
            );
        }

        let player = CountryId(self.player_country as u16);
        let visible = Some(hoi4_render::units::visibility::visible_countries(
            &self.world,
            player,
        ));
        let spotted = Some(hoi4_render::units::visibility::spotted_provinces(
            &self.world,
            player,
        ));
        self.counter_visibility_cache = CounterVisibilityCache {
            valid: true,
            signature: sig,
            visible: visible.clone(),
            spotted: spotted.clone(),
        };
        (visible, spotted)
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

    fn update_division_motion(&mut self, dt: f32, simulation_advanced: bool) {
        if self.last_division_locations.len() != self.world.divisions.count {
            self.last_division_locations = self.world.divisions.locations.clone();
            self.division_motion.clear();
            return;
        }

        if simulation_advanced {
            let centroids = self
                .state
                .as_ref()
                .map(|state| state.unit_counter_centroids.as_slice());
            for i in 0..self.world.divisions.count {
                let old = self.last_division_locations[i];
                let new = self.world.divisions.locations[i];
                if old == new {
                    continue;
                }
                if self
                    .division_motion
                    .get(&i)
                    .is_some_and(|motion| motion.from == old && motion.to == new)
                {
                    self.last_division_locations[i] = new;
                    continue;
                }

                let should_animate =
                    centroids.is_some() && self.should_animate_division_step(old, new);

                if should_animate {
                    let duration = match self.world.speed {
                        GameSpeed::Paused => 0.5,
                        speed => (speed.seconds_per_hour() * 24.0).clamp(0.08, 5.0),
                    };
                    self.division_motion.insert(
                        i,
                        VisualDivisionMotion {
                            from: old,
                            to: new,
                            elapsed: 0.0,
                            duration,
                        },
                    );
                } else {
                    self.division_motion.remove(&i);
                }
                self.last_division_locations[i] = new;
            }
        }

        if dt > 0.0 {
            self.division_motion.retain(|_, motion| {
                motion.elapsed += dt;
                motion.elapsed < motion.duration
            });
        }
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
            "[perf] speed={} date={} sim={:.2}d/s acc={:.3}s divs={} moving={} wars={} armies={} path={} pdi={} counters={} rebuild/s={} cache/s={} render={:.2}ms counter={:.2}ms arrows={:.2}ms ui={:.2}ms(begin={:.2} tess={:.2} buf={:.2} paint={:.2} prim={} tris={}) panels={} {}",
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

    fn visual_division_centroid_overrides(&self) -> HashMap<usize, (f32, f32)> {
        let mut overrides = HashMap::new();
        let Some(state) = self.state.as_ref() else {
            return overrides;
        };
        for (&div_idx, motion) in &self.division_motion {
            let Some(&(from_x, from_y)) = state.unit_counter_centroids.get(motion.from.0 as usize)
            else {
                continue;
            };
            let Some(&(to_x, to_y)) = state.unit_counter_centroids.get(motion.to.0 as usize) else {
                continue;
            };
            if (from_x + from_y) == 0.0 || (to_x + to_y) == 0.0 {
                continue;
            }
            let t = (motion.elapsed / motion.duration.max(0.001)).clamp(0.0, 1.0);
            overrides.insert(
                div_idx,
                (from_x + (to_x - from_x) * t, from_y + (to_y - from_y) * t),
            );
        }
        overrides
    }

    /// Builds data for the country info panel.
    fn build_country_info_data(
        &self,
        target: hoi4_state::CountryId,
        has_wargoal: bool,
    ) -> Option<hoi4_ui::country_info_panel::CountryInfoData> {
        let ti = target.0 as usize;
        if ti >= self.world.countries.count {
            return None;
        }
        let player = hoi4_state::CountryId(self.player_country as u16);

        let tag = self.world.countries.tags.get(ti)?.clone();
        if tag.is_empty() {
            return None;
        }
        let display_name = hoi4_ui::i18n::tr(&tag).to_string();

        // 鍏冮
        let (leader_name, leader_portrait_key) =
            Self::head_of_state_display(&self.world, &self.historical_1936, target);

        let ruling = self
            .world
            .countries
            .ruling_party
            .get(ti)
            .cloned()
            .unwrap_or_default();
        let party_loc_key_long = format!("{}_{}_party_long", tag, ruling);
        let party_loc_key = format!("{}_{}_party", tag, ruling);
        let party_full_name = self
            .world
            .data
            .party_names
            .get(&party_loc_key_long)
            .or_else(|| self.world.data.party_names.get(&party_loc_key))
            .cloned()
            .unwrap_or_else(|| ruling.clone());
        let ruling_party_label = hoi4_ui::i18n::tr(&ruling).to_string();

        // Economy stats from V6 buildings.
        let (industrial_level, military_industrial_level) =
            Self::v6_industrial_levels(&self.world, target);
        let gdp_gbp = self
            .world
            .countries
            .treasury
            .treasuries
            .get(ti)
            .map(|t| t.gdp_gbp)
            .unwrap_or_else(|| Self::v6_estimated_gdp_gbp(&self.world, &self.v6_db, target));
        let construction_points = Self::v6_construction_points(&self.world, target);
        let population_breakdown = self.world.country_population_breakdown(target);
        let population = population_breakdown.governed;
        let manpower = recruitable_manpower(&self.world, &self.v6_db, CountryId(ti as u16));
        let stability = self
            .world
            .countries
            .stability
            .get(ti)
            .copied()
            .unwrap_or(0.5);
        let war_support = self
            .world
            .countries
            .war_support
            .get(ti)
            .copied()
            .unwrap_or(0.0);

        // Division count.
        let division_count = (0..self.world.divisions.count)
            .filter(|&i| self.world.divisions.owners[i] == target)
            .count() as u32;

        // 鍏崇郴
        let opinion = self.world.diplomacy.opinions.get(player, target);
        let at_war = self.world.diplomacy.at_war_with(player, target);
        let player_faction = self.world.diplomacy.faction_of(player);
        let target_faction = self.world.diplomacy.faction_of(target);
        let same_faction = player_faction.is_some() && player_faction == target_faction;
        let faction_name = target_faction
            .and_then(|fid| self.world.diplomacy.faction(fid).map(|f| f.name.clone()));
        let autonomy = self.world.diplomacy.autonomy.get(&target);
        let overlord_name = autonomy.map(|a| self.country_display_name(a.master));
        let autonomy_level_name = autonomy.map(|a| autonomy_level_label(a.level).to_owned());
        let mut subject_names: Vec<String> = self
            .world
            .diplomacy
            .autonomy
            .values()
            .filter(|a| a.master == target)
            .map(|a| self.country_display_name(a.subject))
            .collect();
        subject_names.sort();

        let (justifying_wargoal, justify_progress, justify_days_remaining) = self
            .world
            .diplomacy
            .pending_wargoals
            .get(&player)
            .and_then(|wgs| wgs.iter().find(|w| w.target == target))
            .map(|wg| {
                if wg.justified {
                    (false, 1.0_f32, 0_u32)
                } else {
                    let total = wg.justify_total_days.max(1.0);
                    let prog = (wg.justify_progress / total).clamp(0.0, 1.0);
                    let days = (wg.justify_total_days - wg.justify_progress)
                        .max(0.0)
                        .ceil() as u32;
                    (true, prog, days)
                }
            })
            .unwrap_or((false, 0.0, 0));

        let justify_action = diplomacy_action_view(
            &self.world,
            player,
            hoi4_logic::diplomacy::DiplomaticAction::StartJustification {
                target,
                kind: hoi4_state::WargoalType::Annex,
                target_state: None,
            },
        );
        let mut declare_war_action = diplomacy_action_view(
            &self.world,
            player,
            hoi4_logic::diplomacy::DiplomaticAction::DeclareWar { target },
        );
        if self.settings.instant_war
            && !declare_war_action.enabled
            && declare_war_action.reason.as_deref() == Some("需要已正当化的战争目标")
        {
            declare_war_action = hoi4_ui::diplomacy::DiplomaticActionView::enabled(
                "直接宣战已开启：点击后会跳过正当化并立即宣战",
            );
        }
        let invite_to_faction_action = diplomacy_action_view(
            &self.world,
            player,
            hoi4_logic::diplomacy::DiplomaticAction::InviteToFaction { target },
        );
        let request_access_action = diplomacy_action_view(
            &self.world,
            player,
            hoi4_logic::diplomacy::DiplomaticAction::RequestMilitaryAccess { target },
        );

        Some(hoi4_ui::country_info_panel::CountryInfoData {
            tag: tag.clone(),
            display_name,
            flag_gfx: format!("GFX_flag_{}_{}", tag, ruling),
            leader_name,
            leader_portrait_key,
            ruling_party_label,
            party_full_name,
            gdp_gbp,
            industrial_level,
            military_industrial_level,
            construction_points,
            division_count,
            population,
            domestic_population: population_breakdown.domestic,
            colonial_population: population_breakdown.colonial,
            governed_population: population_breakdown.governed,
            subject_population: population_breakdown.subject,
            imperial_population: population_breakdown.imperial,
            manpower,
            stability,
            war_support,
            opinion,
            at_war,
            same_faction,
            faction_name,
            overlord_name,
            subject_names,
            autonomy_level_name,
            has_wargoal,
            justifying_wargoal,
            justify_progress,
            justify_days_remaining,
            wargoals: country_wargoal_details(&self.world, player, target),
            relation_factors: country_relation_factors(&self.world, player, target),
            justify_action,
            declare_war_action,
            invite_to_faction_action,
            request_access_action,
        })
    }

    fn state_population(&self, state: hoi4_state::StateId) -> u64 {
        self.world.state_population(state)
    }

    /// Pure province pick: returns the province ID under the current cursor,
    /// or `u32::MAX` if nothing is hit. No side effects.
    fn pick_province_at_cursor(&self) -> u32 {
        let (cw, ch) = match &self.state {
            Some(s) => {
                let dpi = s.window.scale_factor() as f32;
                (s.config.width as f32 / dpi, s.config.height as f32 / dpi)
            }
            None => return u32::MAX,
        };
        let cursor_x = self.last_mouse[0];
        let cursor_y = self.last_mouse[1];

        // Terrain ray-cast pick.
        let ndc_x = (cursor_x / cw) * 2.0 - 1.0;
        let ndc_y = 1.0 - (cursor_y / ch) * 2.0;
        let ndc = Vec2::new(ndc_x, ndc_y);
        let hmap = &self.world.map.heightmap;
        let world_size = self.camera.world_size;

        // Water is drawn as a flat sea-level surface. Pick that visible plane
        // first for sea/lake provinces; otherwise oblique clicks ray-cast to
        // the ocean floor and land on the wrong map pixel.
        if let Some(world_xz) = self.camera.pick_world_xz_at_height(
            ndc,
            hoi4_map::Heightmap::SEA_LEVEL as f32 / 255.0 * HEIGHT_SCALE,
        ) {
            let u = world_xz.x.rem_euclid(world_size.x) / world_size.x;
            let v = world_xz.y / world_size.y;
            if (0.0..=1.0).contains(&u) && (0.0..=1.0).contains(&v) {
                let pmap = &self.world.map.province_map;
                let px = ((u * pmap.width as f32) as u32).min(pmap.width - 1);
                let py = ((v * pmap.height as f32) as u32).min(pmap.height - 1);
                let pid = pmap.pixels[(py * pmap.width + px) as usize] as u32;
                if self
                    .world
                    .map
                    .definitions
                    .get(pid as usize)
                    .and_then(|def| def.as_ref())
                    .map(|def| {
                        matches!(
                            def.province_type,
                            hoi4_map::ProvinceType::Sea | hoi4_map::ProvinceType::Lake
                        )
                    })
                    .unwrap_or(false)
                {
                    return pid;
                }
            }
        }

        let mut ground_y = HEIGHT_SCALE * 0.3;
        let mut world_xz = match self.camera.pick_world_xz_at_height(ndc, ground_y) {
            Some(v) => v,
            None => return u32::MAX,
        };

        for _ in 0..3 {
            let u = world_xz.x.rem_euclid(world_size.x) / world_size.x;
            let v = world_xz.y / world_size.y;
            if !(0.0..=1.0).contains(&u) || !(0.0..=1.0).contains(&v) {
                break;
            }
            let hx = ((u * hmap.width as f32) as u32).min(hmap.width - 1);
            let hy = ((v * hmap.height as f32) as u32).min(hmap.height - 1);
            let raw_h = hmap.pixels[(hy * hmap.width + hx) as usize];
            ground_y = HEIGHT_SCALE * (raw_h as f32 / 255.0);
            world_xz = match self.camera.pick_world_xz_at_height(ndc, ground_y) {
                Some(v) => v,
                None => return u32::MAX,
            };
        }

        let u = world_xz.x.rem_euclid(world_size.x) / world_size.x;
        let v = world_xz.y / world_size.y;
        if (0.0..=1.0).contains(&u) && (0.0..=1.0).contains(&v) {
            let pmap = &self.world.map.province_map;
            let px = ((u * pmap.width as f32) as u32).min(pmap.width - 1);
            let py = ((v * pmap.height as f32) as u32).min(pmap.height - 1);
            pmap.pixels[(py * pmap.width + px) as usize] as u32
        } else {
            u32::MAX
        }
    }

    fn pick_counter_province_at_cursor(&self) -> Option<u32> {
        let [mx, my] = self.last_mouse;
        let dpi = self
            .state
            .as_ref()
            .map(|s| s.window.scale_factor() as f32)
            .unwrap_or(1.0);
        hit_test(&self._cached_hoi3_hit_regions, mx * dpi, my * dpi).map(|pid| pid as u32)
    }

    fn select_counter_stack_at_province(&mut self, pid: u32, ctrl_held: bool, shift_held: bool) {
        if ctrl_held {
            if self.selected_province_ids.contains(&pid) {
                self.selected_province_ids.remove(&pid);
            } else {
                self.selected_province_ids.insert(pid);
            }
        } else {
            let was_selected = self.selected_province_ids.contains(&pid);
            self.selected_province_ids.clear();
            self.expanded_stacks.clear();
            self.selected_province_ids.insert(pid);
            if was_selected {
                let has_stack = self._cached_hoi3_counter_upload.iter().any(|c| {
                    c.province_id() as u32 == pid
                        && (c.flags & flag_bits::IS_UNDERLAY) == 0
                        && c.stack_count > 1
                });
                if has_stack {
                    self.expanded_stacks.insert(pid);
                }
            }
        }

        let player = hoi4_state::CountryId(self.player_country as u16);
        let prov = hoi4_state::ProvinceId(pid as u16);
        if ctrl_held {
            for i in 0..self.world.divisions.count {
                if self.world.divisions.owners[i] == player
                    && self.world.divisions.locations[i] == prov
                {
                    if let Some(pos) = self.selected_divisions.iter().position(|&x| x == i) {
                        self.selected_divisions.remove(pos);
                    } else {
                        self.selected_divisions.push(i);
                    }
                }
            }
        } else if shift_held {
            self.selected_divisions.clear();
            for i in 0..self.world.divisions.count {
                if self.world.divisions.owners[i] == player {
                    self.selected_divisions.push(i);
                }
            }
        } else {
            self.selected_divisions.clear();
            for i in 0..self.world.divisions.count {
                if self.world.divisions.owners[i] == player
                    && self.world.divisions.locations[i] == prov
                {
                    self.selected_divisions.push(i);
                }
            }
        }
        self.selected_army_id = None;
        if let Some(s) = self.state.as_mut() {
            s.window.request_redraw();
        }
        self.refresh_lut();
    }

    fn select_counter_stacks_in_rect(&mut self, rect_logical: [f32; 4], additive: bool) -> bool {
        let Some(s) = self.state.as_ref() else {
            return false;
        };
        let dpi = s.window.scale_factor() as f32;
        let rect = [
            rect_logical[0] * dpi,
            rect_logical[1] * dpi,
            rect_logical[2] * dpi,
            rect_logical[3] * dpi,
        ];
        let rx1 = rect[0] + rect[2];
        let ry1 = rect[1] + rect[3];
        let player = hoi4_state::CountryId(self.player_country as u16);
        let mut province_hits = HashSet::new();

        for region in &self._cached_hoi3_hit_regions {
            let [x, y, w, h] = region.rect;
            let cx = x + w * 0.5;
            let cy = y + h * 0.5;
            if cx >= rect[0] && cx <= rx1 && cy >= rect[1] && cy <= ry1 {
                province_hits.insert(region.province_id as u32);
            }
        }

        if !additive {
            self.selected_divisions.clear();
            self.selected_province_ids.clear();
            self.expanded_stacks.clear();
        }

        for pid in province_hits {
            let prov = hoi4_state::ProvinceId(pid as u16);
            let mut any_here = false;
            for i in 0..self.world.divisions.count {
                if self.world.divisions.owners[i] == player
                    && self.world.divisions.locations[i] == prov
                    && !self.selected_divisions.contains(&i)
                {
                    self.selected_divisions.push(i);
                    any_here = true;
                }
            }
            if any_here {
                self.selected_province_ids.insert(pid);
            }
        }

        self.selected_army_id = None;
        self.refresh_lut();
        if let Some(s) = self.state.as_ref() {
            s.window.request_redraw();
        }
        !self.selected_divisions.is_empty()
    }

    /// Attempt to pick a province under the current cursor position.
    /// On hit, sets `selected_province_id` and triggers a redraw.
    fn try_pick_province(&mut self) {
        // Construction placement mode: clicking a province places the building
        if let Some(ref building_key) = self.construction_mode.clone() {
            let pid = self.pick_province_at_cursor();
            if pid != u32::MAX {
                let sid = self.world.provinces.state_of[pid as usize];
                if !sid.is_none() {
                    let si = sid.0 as usize;
                    let player_cid = hoi4_state::CountryId(self.player_country as u16);
                    if si < self.world.states.count && self.world.states.owners[si] == player_cid {
                        let used: u8 = self
                            .world
                            .countries
                            .buildings_v6
                            .buildings
                            .iter()
                            .filter(|building| building.state == sid && building.level > 0)
                            .map(|building| building.level)
                            .sum();
                        let max = Self::v6_state_building_capacity(&self.world, sid);
                        if (used as u16) < max {
                            let target_level =
                                Self::v6_next_construction_level(&self.world, building_key, sid);
                            let order = hoi4_logic::economy::BuildOrder::new(building_key, sid)
                                .with_level(target_level);
                            match self.econ.enqueue_construction_checked(
                                player_cid,
                                order,
                                &self.world,
                                &self.v6_db,
                            ) {
                                Ok(()) => {
                                    self.ui_sounds
                                        .play_with_fallback(UiSound::OptionClick, UiSound::Click);
                                }
                                Err(reason) => {
                                    println!(
                                        "[construction] could not queue {} in {}: {:?}",
                                        building_key, self.world.states.names[si], reason
                                    );
                                    self.ui_sounds
                                        .play_with_fallback(UiSound::Click, UiSound::Click);
                                }
                            }
                            // 涓嶉€€鍑哄缓閫犳ā寮忊€斺€旂帺瀹跺彲浠ヨ繛缁偣鍑诲涓渷浠藉缓閫犲悓绫诲缓绛戙€?                            // 鎸?ESC 鎴栧彸閿墠閫€鍑?construction_mode銆?                        } else {
                            println!(
                                "[construction] state {} is full ({}/{})",
                                self.world.states.names[si], used, max
                            );
                            self.ui_sounds
                                .play_with_fallback(UiSound::Click, UiSound::Click);
                        }
                    }
                }
            }
            if let Some(s) = &self.state {
                s.window.request_redraw();
            }
            return;
        }

        // CR-4.5: If pending_move_command, this click sets destination
        if self.pending_move_command {
            let dest_pid = self.pick_province_at_cursor();
            if dest_pid != u32::MAX {
                let dest = hoi4_state::ProvinceId(dest_pid as u16);
                self.issue_manual_move_to_selected_divisions(dest);
            }
            self.pending_move_command = false;
            self.counter_right_click_province = None;
            if let Some(s) = &self.state {
                s.window.request_redraw();
            }
            return;
        }

        if let Some(fleet_id) = self.pending_naval_move_fleet {
            let pid = self.pick_province_at_cursor();
            if pid != u32::MAX {
                let selected_sea_region = self
                    .world
                    .map
                    .definitions
                    .get(pid as usize)
                    .and_then(|def| def.as_ref())
                    .filter(|def| def.province_type == hoi4_map::ProvinceType::Sea)
                    .map(|def| def.id as u32);
                if let Some(region) = selected_sea_region {
                    let now = self.world.date.hours_since_epoch().max(0) as u64;
                    let _ = hoi4_logic::naval::movement::order_move_to_region(
                        &mut self.world,
                        hoi4_state::FleetId(fleet_id),
                        region,
                        now,
                    );
                    self.pending_naval_move_fleet = None;
                }
            }
            if let Some(s) = &self.state {
                s.window.request_redraw();
            }
            return;
        }

        if let Some(wing_id) = self.pending_air_transfer_wing {
            let pid = self.pick_province_at_cursor();
            if pid != u32::MAX {
                let selected_state = self
                    .world
                    .provinces
                    .state_of
                    .get(pid as usize)
                    .copied()
                    .unwrap_or(hoi4_state::StateId::NONE);
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
                }
            }
            if let Some(s) = &self.state {
                s.window.request_redraw();
            }
            return;
        }

        let new_pid = self.pick_province_at_cursor();

        // CR-4.3: Multi-select with Ctrl
        let ctrl_held = self.keys_held.contains(&KeyCode::ControlLeft)
            || self.keys_held.contains(&KeyCode::ControlRight);

        if ctrl_held {
            if new_pid != u32::MAX {
                if self.selected_province_ids.contains(&new_pid) {
                    self.selected_province_ids.remove(&new_pid);
                } else {
                    self.selected_province_ids.insert(new_pid);
                }
            }
        } else {
            self.selected_province_ids.clear();
            self.expanded_stacks.clear();
            if new_pid != u32::MAX {
                self.selected_province_ids.insert(new_pid);
            }
        }

        self.selected_province_id = new_pid;
        if new_pid != u32::MAX && !ctrl_held {
            let pi = new_pid as usize;
            if pi < self.world.provinces.count {
                let sid = self.world.provinces.state_of[pi];
                let si = sid.0 as usize;
                if si < self.world.states.count {
                    for province in &self.world.states.provinces[si] {
                        self.selected_province_ids.insert(province.0 as u32);
                    }
                }
            }
        }

        let date_s = self.world.date.to_string();
        let mode_s = self.map_mode.name().to_string();

        if let Some(s) = self.state.as_mut() {
            s.window.request_redraw();
            s.window.set_title(&format!(
                "HOI4 Rust 3D | selected province {} | {} | {} | edge/arrows pan, wheel zoom, M map mode, ESC quit",
                new_pid, date_s, mode_s,
            ));
        }
        self.refresh_lut();

        // Select player's divisions in this province
        let player = hoi4_state::CountryId(self.player_country as u16);
        let prov = hoi4_state::ProvinceId(new_pid as u16);
        let shift_held = self.keys_held.contains(&KeyCode::ShiftLeft)
            || self.keys_held.contains(&KeyCode::ShiftRight);

        if shift_held {
            self.selected_divisions.clear();
            for i in 0..self.world.divisions.count {
                if self.world.divisions.owners[i] == player {
                    self.selected_divisions.push(i);
                }
            }
        } else if ctrl_held {
            for i in 0..self.world.divisions.count {
                if self.world.divisions.owners[i] == player
                    && self.world.divisions.locations[i] == prov
                {
                    if let Some(pos) = self.selected_divisions.iter().position(|&x| x == i) {
                        self.selected_divisions.remove(pos);
                    } else {
                        self.selected_divisions.push(i);
                    }
                }
            }
        } else {
            self.selected_divisions.clear();
            for i in 0..self.world.divisions.count {
                if self.world.divisions.owners[i] == player
                    && self.world.divisions.locations[i] == prov
                {
                    self.selected_divisions.push(i);
                }
            }
        }

        // 11.2: If clicked province is on a player army's frontline, select that army
        let clicked_army = self.world.player_armies.iter().find(|a| {
            a.owner == player
                && a.order
                    .as_ref()
                    .map_or(false, |o| o.active && o.path.contains(&prov))
        });
        if new_pid != u32::MAX {
            self.selected_army_id = clicked_army.map(|a| a.id);
        }

        // J.2: Populate province info card
        if new_pid != u32::MAX {
            let pi = new_pid as usize;
            let state_id_from_province = if pi < self.world.provinces.count {
                self.world.provinces.state_of[pi]
            } else {
                hoi4_state::StateId::NONE
            };
            let si = state_id_from_province.0 as usize;
            let state_name = if si < self.world.states.count {
                self.state_display_name(si)
            } else {
                hoi4_ui::i18n::tr("unknown").to_owned()
            };
            let owner_id = if si < self.world.states.count {
                self.world.states.owners[si]
            } else {
                hoi4_state::CountryId::NONE
            };
            let state_ctrl_id = if si < self.world.states.count {
                self.world.states.controllers[si]
            } else {
                hoi4_state::CountryId::NONE
            };
            let prov_owner_id = if pi < self.world.provinces.count {
                self.world.provinces.owners[pi]
            } else {
                hoi4_state::CountryId::NONE
            };
            let prov_ctrl_id = if pi < self.world.provinces.count {
                self.world.provinces.controllers[pi]
            } else {
                hoi4_state::CountryId::NONE
            };
            let owner_tag = if (owner_id.0 as usize) < self.world.countries.count {
                self.world.countries.tags[owner_id.0 as usize].clone()
            } else {
                String::new()
            };
            let state_controller_tag = if (state_ctrl_id.0 as usize) < self.world.countries.count {
                self.world.countries.tags[state_ctrl_id.0 as usize].clone()
            } else {
                String::new()
            };
            let province_owner_tag = if (prov_owner_id.0 as usize) < self.world.countries.count {
                self.world.countries.tags[prov_owner_id.0 as usize].clone()
            } else {
                String::new()
            };
            let province_controller_tag = if (prov_ctrl_id.0 as usize) < self.world.countries.count
            {
                self.world.countries.tags[prov_ctrl_id.0 as usize].clone()
            } else {
                String::new()
            };
            // Static metadata from GameData; resource output below uses V6 buildings.
            let (category, vp) = self
                .world
                .data
                .states
                .iter()
                .find(|s| s.provinces.contains(&(new_pid as u16)))
                .map(|s| {
                    (
                        s.category.clone(),
                        s.victory_points
                            .iter()
                            .find(|(pid, _)| *pid == new_pid as u16)
                            .map(|(_, v)| *v)
                            .unwrap_or(0),
                    )
                })
                .unwrap_or_default();
            let vp_key = format!("VICTORY_POINTS_{}", new_pid);
            let vp_loc = self.localize_key(&vp_key);
            let vp_name = (vp_loc != vp_key).then_some(vp_loc.as_str());
            let province_name = self.province_display_name(new_pid, vp_name, Some(&state_name));
            let province_def = self.world.map.get_province(new_pid as u16);
            let province_type = Self::province_type_name(province_def);
            let terrain = province_def
                .map(|def| Self::terrain_display_name(def.terrain.as_str()))
                .unwrap_or_else(|| hoi4_ui::i18n::tr("unknown").to_owned());
            let coastal = province_def.map(|def| def.coastal).unwrap_or(false);
            let supply = self.world.provinces.supply.get(pi).copied().unwrap_or(0.0);
            let mut strategic_nodes = Vec::new();
            if vp > 0 {
                strategic_nodes.push(format!(
                    "{} {}",
                    hoi4_ui::i18n::tr("victory_points_label"),
                    vp
                ));
            }
            if coastal {
                strategic_nodes.push(hoi4_ui::i18n::tr("coastal_province").to_owned());
            }
            if province_def
                .map(|def| def.terrain == "urban")
                .unwrap_or(false)
            {
                strategic_nodes.push("城市".to_owned());
            }
            let adjacent_enemy = self.world.map.neighbors(new_pid as u16).iter().any(|&adj| {
                let adj_pi = adj as usize;
                adj_pi < self.world.provinces.count
                    && self.world.provinces.controllers[adj_pi] != prov_ctrl_id
                    && !self.world.provinces.controllers[adj_pi].is_none()
            });
            if adjacent_enemy {
                strategic_nodes.push("前线接触".to_owned());
            }
            // Divisions in this province (all countries visible)
            let div_names: Vec<String> = (0..self.world.divisions.count)
                .filter(|&i| self.world.divisions.locations[i] == prov)
                .map(|i| self.world.divisions.names[i].clone())
                .collect();
            self.province_info_card.open = true;
            let state_id = hoi4_state::StateId(si as u16);
            let mut v6_outputs: HashMap<String, (u32, f32)> = HashMap::new();
            if si < self.world.states.count {
                for building in self
                    .world
                    .countries
                    .buildings_v6
                    .buildings
                    .iter()
                    .filter(|building| building.state == state_id && building.level > 0)
                {
                    if !matches!(
                        building.kind,
                        hoi4_state::BuildingKind::Resource | hoi4_state::BuildingKind::Agriculture
                    ) {
                        continue;
                    }
                    for pm in hoi4_content::active_pms_for_building(building, &self.v6_db) {
                        let throughput =
                            pm.throughput_modifier.max(0.0) * building.production_rate.max(0.0);
                        for (i, good_id) in pm.output_good_ids.iter().enumerate() {
                            let amount = pm.output_good_amounts.get(i).copied().unwrap_or(0.0)
                                * building.level as f32
                                * throughput;
                            if amount <= 0.0 {
                                continue;
                            }
                            let entry = v6_outputs.entry(good_id.clone()).or_default();
                            entry.0 = entry.0.saturating_add(building.level as u32);
                            entry.1 += amount;
                        }
                    }
                }
            }
            let mut resources: Vec<(String, f32)> = v6_outputs
                .iter()
                .map(|(good_id, (level, _))| {
                    (Self::v6_good_name(&self.v6_db, good_id), *level as f32)
                })
                .collect();
            resources.sort_by(|a, b| a.0.cmp(&b.0));
            let mut resources_output: Vec<(String, f32)> = v6_outputs
                .iter()
                .map(|(good_id, (_, amount))| (Self::v6_good_name(&self.v6_db, good_id), *amount))
                .collect();
            resources_output.sort_by(|a, b| a.0.cmp(&b.0));
            let owner_ci = if owner_id.is_none() {
                usize::MAX
            } else {
                owner_id.0 as usize
            };
            let state_population = if si < self.world.states.count {
                self.state_population(state_id)
            } else {
                0
            };
            let state_buildings = if si < self.world.states.count {
                self.world
                    .countries
                    .buildings_v6
                    .buildings
                    .iter()
                    .filter(|building| building.state == state_id && building.level > 0)
                    .map(|building| {
                        let employment_gap =
                            Self::v6_employment_gap_for_building(&self.v6_db, building);
                        let mut warnings = Vec::new();
                        if employment_gap.iter().any(|&gap| gap > 0) {
                            warnings.push(format!(
                                "{}：{}",
                                hoi4_ui::i18n::tr("labor_shortage"),
                                employment_gap.iter().sum::<u32>()
                            ));
                        }
                        hoi4_ui::province_info::StateBuildingInfo {
                            name: Self::v6_building_name(&self.v6_db, &building.building_def_id),
                            level: building.level,
                            employment_rate: building.production_rate,
                            profit_rm_weekly: building.profit_rm * 7.0,
                            warnings,
                        }
                    })
                    .collect()
            } else {
                Vec::new()
            };
            let construction_projects = if owner_ci < self.econ.count {
                self.econ.construction[owner_ci]
                    .items
                    .iter()
                    .filter(|item| item.target_state == state_id)
                    .map(|item| {
                        let current_level = Self::v6_current_building_level(
                            &self.world,
                            &item.building_key,
                            item.target_state,
                        );
                        let target_level = if item.target_level == 0 {
                            current_level.saturating_add(1)
                        } else {
                            item.target_level
                        };
                        hoi4_ui::province_info::StateConstructionProjectInfo {
                            building_name: Self::v6_building_name(&self.v6_db, &item.building_key),
                            current_level,
                            target_level,
                            progress: item.completion(),
                            estimated_days_remaining: estimate_construction_days_remaining(
                                item.progress,
                                item.cost,
                            ),
                        }
                    })
                    .collect()
            } else {
                Vec::new()
            };
            self.province_info_data = hoi4_ui::province_info::ProvinceInfoData {
                province: hoi4_ui::province_info::ProvinceTacticalInfo {
                    province_id: new_pid,
                    province_name,
                    province_type,
                    terrain,
                    coastal,
                    owner_tag: province_owner_tag,
                    controller_tag: province_controller_tag,
                    state_name: state_name.clone(),
                    supply,
                    victory_points: vp,
                    strategic_nodes,
                    divisions: div_names,
                },
                state: hoi4_ui::province_info::StateEconomicInfo {
                    state_name,
                    owner_tag,
                    controller_tag: state_controller_tag,
                    population: state_population,
                    state_category: category,
                    infrastructure: if si < self.world.states.count {
                        self.world.states.infrastructure[si]
                    } else {
                        0
                    },
                    resources,
                    resources_output,
                    buildings: state_buildings,
                    construction_projects,
                    slots_used: if si < self.world.states.count {
                        self.world.state_building_levels(state_id) as u8
                    } else {
                        0
                    },
                    slots_max: if si < self.world.states.count {
                        self.world.states.category_slots[si]
                    } else {
                        0
                    },
                },
            };
        }
    }

    // V5 鏀跺彛锛氬垹???`handle_ui_click` 鈥???vanilla GUI widget hit-test 璺嚎宸插仠鐢紝
    // topbar 閫熷害鎸夐挳 / 鏀挎不闈㈡澘鍏ュ彛灏嗗湪闃舵 B 閫氳繃 egui 閲嶅仛???
    /// Phase 4.3: Handle mouse click within an open in-game panel.
    fn handle_panel_click(&mut self, _mx: f32, _my: f32) -> bool {
        false
    }

    /// Open or close an in-game panel from a single entry point.
    /// Opening any main panel hides the province info card so a stray map click
    /// cannot leave it layered on top of the new panel.
    fn toggle_in_game_panel(&mut self, panel: InGamePanel) {
        if self.open_panel == Some(panel) {
            self.open_panel = None;
        } else {
            self.open_panel = Some(panel);
            self.province_info_card.open = false;
            self.country_info_panel.close();
        }
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
                label: "佛朗哥权威".to_owned(),
                value: value("spa_franco_authority"),
                max: 15.0,
                detail: "开局只是外援与非洲军团的协调者；桑胡尔霍、莫拉等竞争者退出后才会真正抬高。8 点进入强势候选，12 点会锁定 P9 佛朗哥胜利路线。".to_owned(),
            },
            hoi4_ui::decisions_panel::MechanicGauge {
                label: "长枪党/蓝衫".to_owned(),
                value: value("spa_falange_power"),
                max: 15.0,
                detail: "1936 年 7 月仍是地方蓝衫、民兵和宣传网络，不是国民军最高统帅部；授权地方委员会、何塞·安东尼奥死亡和统一法令会让它制度化。".to_owned(),
            },
            hoi4_ui::decisions_panel::MechanicGauge {
                label: "军队忠诚".to_owned(),
                value: value("spa_army_loyalty"),
                max: 15.0,
                detail: "决定军政府、前线将领和复员后的军内服从度。".to_owned(),
            },
            hoi4_ui::decisions_panel::MechanicGauge {
                label: "教会影响".to_owned(),
                value: value("spa_church_influence"),
                max: 15.0,
                detail: "推动国民天主教秩序，也牵制长枪党地方扩张。".to_owned(),
            },
            hoi4_ui::decisions_panel::MechanicGauge {
                label: "卡洛斯派怨恨".to_owned(),
                value: value("spa_carlist_anger"),
                max: 15.0,
                detail: "过高会制造传统派反弹和王政问题风险。".to_owned(),
            },
            hoi4_ui::decisions_panel::MechanicGauge {
                label: "外援依赖".to_owned(),
                value: value("spa_foreign_dependency"),
                max: 15.0,
                detail: "德国与意大利援助越多，战后索偿和外交束缚越重。".to_owned(),
            },
            hoi4_ui::decisions_panel::MechanicGauge {
                label: "占领区抵抗".to_owned(),
                value: value("spa_occupation_resistance"),
                max: 15.0,
                detail: "清洗、恐怖广播和地方接管会抬高抵抗，军管治理可压低。".to_owned(),
            },
        ]
    }

    /// Block map clicks whenever an overlay or modal is meant to own input.
    /// Map interaction modes such as construction placement / frontline painting
    /// are exempt so they keep working while the relevant panel is open.
    fn ui_blocks_map_clicks(&self) -> bool {
        let panel_blocks_map = self
            .open_panel
            .is_some_and(|panel| !matches!(panel, InGamePanel::Air | InGamePanel::Naval));
        self.game_phase == GamePhase::Playing
            && self.construction_mode.is_none()
            && self.pending_move_command == false
            && self.frontline_painter.mode == PainterMode::Idle
            && self.counter_right_click_province.is_none()
            && (panel_blocks_map
                || self
                    .province_info_card
                    .contains_point(self.last_mouse[0], self.last_mouse[1])
                || self.country_info_panel.open
                || self.settings_panel.open
                || self.save_browser.open
                || (self.end_screen.triggered && !self.end_screen.continued))
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
                            // 榛樿 GER 楂樹寒
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
                    // BACK 鎸夐挳
                    if layout.back_button.contains(mx, my) {
                        self.game_phase = GamePhase::MainMenu;
                        self.reset_menu_state();
                        return;
                    }
                    // START 鎸夐挳
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

    /// Generate and upload HOI3-style screen-space counters each frame.
    fn update_hoi3_counter_pass(&mut self, world_objects: WorldObjectPlan) {
        let sig = self.hoi3_counter_signature(world_objects);
        if sig == self.cached_hoi3_counter_sig {
            self.perf_counter_cache_hits = self.perf_counter_cache_hits.saturating_add(1);
            return;
        }

        self.cached_hoi3_counter_sig = sig;
        self.perf_counter_rebuilds = self.perf_counter_rebuilds.saturating_add(1);

        let view_proj = self.camera.view_proj();
        let (visible, spotted) = self.cached_counter_visibility();
        let player = if self.player_country < self.world.countries.count {
            hoi4_state::CountryId(self.player_country as u16)
        } else {
            hoi4_state::CountryId::NONE
        };
        let visual_centroids_override = self.visual_division_centroid_overrides();
        let (sw, sh, unit_counter_centroids) = match self.state.as_ref() {
            Some(s) => (
                s.config.width as f32,
                s.config.height as f32,
                s.unit_counter_centroids.clone(),
            ),
            None => return,
        };
        let mut counters = generate_hoi3_counters_cr3(
            &self.world,
            &unit_counter_centroids,
            WORLD_SCALE,
            HEIGHT_SCALE,
            &view_proj,
            sw,
            sh,
            &self.selected_province_ids,
            self.camera.distance,
            visible.as_ref(),
            spotted.as_ref(),
            player,
            Some(&visual_centroids_override),
        );
        let counter_scale = world_objects.counters.scale;
        if (counter_scale - 1.0).abs() > 0.001 {
            for counter in &mut counters {
                let old = counter.size;
                counter.size = [old[0] * counter_scale, old[1] * counter_scale];
                counter.screen_pos[0] -= (counter.size[0] - old[0]) * 0.5;
                counter.screen_pos[1] -= (counter.size[1] - old[1]) * 0.5;
            }
        }

        // CR-4: Build layout counters from top counters (skip underlays), run layout, write back.
        use hoi4_render::counter_v3::flag_bits;
        let top_indices: Vec<usize> = counters
            .iter()
            .enumerate()
            .filter(|(_, c)| (c.flags & flag_bits::IS_UNDERLAY) == 0)
            .map(|(i, _)| i)
            .collect();
        let mut layout_counts: HashMap<u16, u16> = HashMap::new();
        let layout_keys: Vec<(u16, u16)> = top_indices
            .iter()
            .map(|&i| {
                let province_id = counters[i].province_id();
                let occurrence = layout_counts.entry(province_id).or_insert(0);
                let key = (province_id, *occurrence);
                *occurrence = occurrence.saturating_add(1);
                key
            })
            .collect();
        let mut layout_counters: Vec<LayoutCounter> = top_indices
            .iter()
            .map(|&i| {
                let c = &counters[i];
                LayoutCounter {
                    pos: c.screen_pos,
                    size: c.size,
                    anchor: c.screen_pos,
                    province_id: c.province_id(),
                }
            })
            .collect();
        // Close-up counters should be camera-stable: screen-space overlap
        // solving changes offsets as perspective changes, so panning the camera
        // can make only some counters appear to slide. Reserve layout solving
        // for the zoomed-out density-control views.
        let layout_threshold = 38.0 + 12.0 * world_objects.counter_layout_density;
        if self.camera.distance > layout_threshold {
            self.apply_or_rebuild_counter_layout(&mut layout_counters, &layout_keys, sw, sh);
        }
        // Write back adjusted positions
        for (li, &ti) in top_indices.iter().enumerate() {
            let delta = [
                layout_counters[li].pos[0] - counters[ti].screen_pos[0],
                layout_counters[li].pos[1] - counters[ti].screen_pos[1],
            ];
            counters[ti].screen_pos = layout_counters[li].pos;
            // Also shift underlays that precede this top counter
            if ti > 0 {
                let mut j = ti - 1;
                loop {
                    if (counters[j].flags & flag_bits::IS_UNDERLAY) != 0 {
                        counters[j].screen_pos[0] += delta[0];
                        counters[j].screen_pos[1] += delta[1];
                    } else {
                        break;
                    }
                    if j == 0 {
                        break;
                    }
                    j -= 1;
                }
            }
        }
        self._cached_hoi3_hit_regions = build_hit_regions(&layout_counters);

        // CR-4.4: Fan-out expanded stacks
        if !self.expanded_stacks.is_empty() {
            let mut expanded_counters: Vec<Hoi3CounterInstance> = Vec::new();
            for c in counters.iter() {
                let pid = c.province_id() as u32;
                if (c.flags & flag_bits::IS_UNDERLAY) != 0 {
                    // Skip underlays of expanded parents
                    if self.expanded_stacks.contains(&pid) {
                        continue;
                    }
                    expanded_counters.push(*c);
                    continue;
                }
                if self.expanded_stacks.contains(&pid) && c.stack_count > 1 {
                    // Fan out: generate one child per division in this province
                    let child_size = [c.size[0] * 0.85, c.size[1] * 0.85];
                    let player_cid = hoi4_state::CountryId(self.player_country as u16);
                    let prov = hoi4_state::ProvinceId(pid as u16);
                    let mut offset_x = 0.0f32;
                    for di in 0..self.world.divisions.count {
                        if self.world.divisions.locations[di] == prov
                            && self.world.divisions.owners[di] == player_cid
                        {
                            let mut child = *c;
                            child.screen_pos = [c.screen_pos[0] + offset_x, c.screen_pos[1]];
                            child.size = child_size;
                            child.stack_count = 1;
                            child.flags =
                                (child.flags & !flag_bits::IS_UNDERLAY) | flag_bits::EXPANDED_CHILD;
                            expanded_counters.push(child);
                            offset_x += child_size[0] + 4.0;
                        }
                    }
                    if offset_x == 0.0 {
                        // No divisions found, keep original
                        expanded_counters.push(*c);
                    }
                } else {
                    expanded_counters.push(*c);
                }
            }
            counters = expanded_counters;
        }

        let s = match self.state.as_mut() {
            Some(s) => s,
            None => return,
        };
        s.hoi3_counter_pass
            .upload(&s.device, &s.queue, &counters, sw, sh);
        s.hoi3_counter_pass
            .update_opacity(&s.queue, world_objects.counters.opacity, sw, sh);
        self.perf_counter_instances = counters.len();
        self._cached_hoi3_counter_upload = counters;
    }

    fn apply_or_rebuild_counter_layout(
        &mut self,
        counters: &mut [LayoutCounter],
        keys: &[(u16, u16)],
        screen_w: f32,
        screen_h: f32,
    ) {
        debug_assert_eq!(counters.len(), keys.len());
        let layout_sig = self.hoi3_counter_layout_signature(counters, keys, screen_w, screen_h);
        if layout_sig == self.cached_hoi3_counter_layout_sig {
            for (counter, key) in counters.iter_mut().zip(keys.iter()) {
                if let Some(offset) = self.cached_hoi3_counter_layout_offsets.get(key) {
                    counter.pos = [counter.anchor[0] + offset[0], counter.anchor[1] + offset[1]];
                }
            }
            return;
        }

        layout_screen_space(counters, screen_w, screen_h);
        self.cached_hoi3_counter_layout_sig = layout_sig;
        self.cached_hoi3_counter_layout_offsets.clear();
        for (counter, key) in counters.iter().zip(keys.iter()) {
            self.cached_hoi3_counter_layout_offsets.insert(
                *key,
                [
                    counter.pos[0] - counter.anchor[0],
                    counter.pos[1] - counter.anchor[1],
                ],
            );
        }
    }

    fn hoi3_counter_layout_signature(
        &self,
        counters: &[LayoutCounter],
        keys: &[(u16, u16)],
        screen_w: f32,
        screen_h: f32,
    ) -> u64 {
        let mut h = DefaultHasher::new();
        self.game_phase.hash(&mut h);
        self.player_country.hash(&mut h);
        self.settings.show_all_units.hash(&mut h);
        ((self.camera.distance * 4.0) as i32).hash(&mut h);
        ((screen_w / 4.0) as i32).hash(&mut h);
        ((screen_h / 4.0) as i32).hash(&mut h);
        self.selected_province_ids.len().hash(&mut h);
        for pid in &self.selected_province_ids {
            pid.hash(&mut h);
        }
        self.hovered_province_id.hash(&mut h);
        self.expanded_stacks.len().hash(&mut h);
        for pid in &self.expanded_stacks {
            pid.hash(&mut h);
        }
        keys.hash(&mut h);
        for counter in counters {
            counter.size[0].to_bits().hash(&mut h);
            counter.size[1].to_bits().hash(&mut h);
        }
        h.finish()
    }

    fn hoi3_counter_signature(&self, world_objects: WorldObjectPlan) -> u64 {
        let mut h = DefaultHasher::new();
        self.game_phase.hash(&mut h);
        self.player_country.hash(&mut h);
        self.camera.distance.to_bits().hash(&mut h);
        self.camera.target.x.to_bits().hash(&mut h);
        self.camera.target.y.to_bits().hash(&mut h);
        self.camera.target.z.to_bits().hash(&mut h);
        ((world_objects.counters.scale * 100.0) as i32).hash(&mut h);
        ((world_objects.counters.opacity * 100.0) as i32).hash(&mut h);
        ((world_objects.counter_layout_density * 100.0) as i32).hash(&mut h);
        for pid in &self.selected_province_ids {
            pid.hash(&mut h);
        }
        for pid in &self.expanded_stacks {
            pid.hash(&mut h);
        }
        self.world.divisions.count.hash(&mut h);
        self.division_motion.len().hash(&mut h);
        for (div_idx, motion) in &self.division_motion {
            div_idx.hash(&mut h);
            motion.from.hash(&mut h);
            motion.to.hash(&mut h);
            ((motion.elapsed * 120.0) as i32).hash(&mut h);
            ((motion.duration * 120.0) as i32).hash(&mut h);
        }
        for i in 0..self.world.divisions.count {
            self.world.divisions.locations[i].hash(&mut h);
            self.world.divisions.owners[i].hash(&mut h);
            self.world.divisions.in_combat[i].hash(&mut h);
            ((self.world.divisions.organisation[i] * 10.0) as i32).hash(&mut h);
            ((self.world.divisions.strength[i] * 100.0) as i32).hash(&mut h);
        }
        h.finish()
    }

    fn frontline_arrow_signature(&self) -> u64 {
        let mut h = DefaultHasher::new();
        self.game_phase.hash(&mut h);
        self.player_country.hash(&mut h);
        self.settings.show_all_units.hash(&mut h);
        self.settings.hide_ai_frontlines.hash(&mut h);
        self.frontline_overlay_visible.hash(&mut h);

        match self.frontline_painter.mode {
            PainterMode::Idle => 0u8.hash(&mut h),
            PainterMode::ArmyPainter(id) => {
                1u8.hash(&mut h);
                id.hash(&mut h);
            }
            PainterMode::ArrowPainter(id, anchor) => {
                2u8.hash(&mut h);
                id.hash(&mut h);
                anchor.hash(&mut h);
            }
        }
        self.frontline_painter.samples.hash(&mut h);

        self.world.player_armies.len().hash(&mut h);
        for army in &self.world.player_armies {
            army.id.hash(&mut h);
            army.owner.hash(&mut h);
            army.members.hash(&mut h);
            if let Some(order) = &army.order {
                true.hash(&mut h);
                order.path.hash(&mut h);
                order.anchor.hash(&mut h);
                order.active.hash(&mut h);
                order.executing.hash(&mut h);
                if let Some(arrow) = &order.arrow {
                    true.hash(&mut h);
                    arrow.provinces.hash(&mut h);
                } else {
                    false.hash(&mut h);
                }
            } else {
                false.hash(&mut h);
            }
        }

        self.selected_divisions.hash(&mut h);
        self.world.divisions.count.hash(&mut h);
        for &div_idx in &self.selected_divisions {
            div_idx.hash(&mut h);
            if div_idx < self.world.divisions.count {
                self.world.divisions.owners[div_idx].hash(&mut h);
                self.world.divisions.locations[div_idx].hash(&mut h);
                self.world.divisions.destinations[div_idx].hash(&mut h);
            }
        }

        h.finish()
    }

    fn collect_order_arrows(&self) -> Vec<passes::maparrow::ArrowInstance> {
        let Some(state) = self.state.as_ref() else {
            return Vec::new();
        };
        let painter_frontline_owner = match self.frontline_painter.mode {
            PainterMode::ArmyPainter(army_id) => self
                .world
                .player_armies
                .iter()
                .find(|army| army.id == army_id)
                .map(|army| army.owner),
            _ => None,
        };
        render_collect::collect_order_arrows(
            &self.world,
            self.player_country,
            self.settings.show_all_units,
            self.settings.hide_ai_frontlines,
            self.frontline_overlay_visible,
            &self.selected_divisions,
            &self.frontline_painter.samples,
            painter_frontline_owner,
            &state.unit_counter_centroids,
            &state.province_pixel_bounds,
        )
    }

    fn collect_combat_bubbles(&self) -> Vec<CombatBubbleSnapshot> {
        if self.game_phase != GamePhase::Playing {
            return Vec::new();
        }
        let Some(state) = self.state.as_ref() else {
            return Vec::new();
        };
        let dpi = state.window.scale_factor() as f32;
        let screen_w = state.config.width as f32 / dpi.max(0.0001);
        let screen_h = state.config.height as f32 / dpi.max(0.0001);
        let view_proj = self.camera.view_proj();
        let player = hoi4_state::CountryId(self.player_country as u16);
        let mut seen = HashSet::new();
        let mut bubbles = Vec::new();

        for raw_a in self.world.prov_div_index.keys().copied() {
            if raw_a as usize >= self.world.provinces.count {
                continue;
            }
            let ctrl_a = self.world.provinces.controllers[raw_a as usize];
            if ctrl_a.is_none() || !is_land_province_for_bubble(&self.world, raw_a) {
                continue;
            }
            for &raw_b in self.world.map.neighbors(raw_a) {
                if raw_a >= raw_b || raw_b as usize >= self.world.provinces.count {
                    continue;
                }
                let key = (raw_a, raw_b);
                if !seen.insert(key) {
                    continue;
                }
                if !is_land_province_for_bubble(&self.world, raw_b) {
                    continue;
                }
                let ctrl_b = self.world.provinces.controllers[raw_b as usize];
                if ctrl_b.is_none()
                    || ctrl_b == ctrl_a
                    || !self.world.diplomacy.at_war_with(ctrl_a, ctrl_b)
                {
                    continue;
                }

                let a_active = self.active_combat_divisions(raw_a, ctrl_a);
                let b_active = self.active_combat_divisions(raw_b, ctrl_b);
                if a_active.is_empty() || b_active.is_empty() {
                    continue;
                }

                let a_attacks_b = self.province_has_attack_towards(raw_a, raw_b, ctrl_a);
                let b_attacks_a = self.province_has_attack_towards(raw_b, raw_a, ctrl_b);
                let side_a = match self.combat_side_snapshot(raw_a, ctrl_a, a_attacks_b) {
                    Some(side) => side,
                    None => continue,
                };
                let side_b = match self.combat_side_snapshot(raw_b, ctrl_b, b_attacks_a) {
                    Some(side) => side,
                    None => continue,
                };
                let Some(screen_pos) =
                    self.combat_contact_screen_pos(raw_a, raw_b, &view_proj, screen_w, screen_h)
                else {
                    continue;
                };
                if screen_pos[0] < -80.0
                    || screen_pos[0] > screen_w + 80.0
                    || screen_pos[1] < -80.0
                    || screen_pos[1] > screen_h + 80.0
                {
                    continue;
                }

                let power_a = combat_side_power(&side_a);
                let power_b = combat_side_power(&side_b);
                let focus_country = if side_b.country == player {
                    side_b.country
                } else {
                    side_a.country
                };
                let focus_power = if side_b.country == focus_country {
                    power_b
                } else {
                    power_a
                };
                let total_power = (power_a + power_b).max(1.0);
                let chance_pct = ((focus_power / total_power) * 100.0).clamp(1.0, 99.0) as u8;
                let id = stable_combat_bubble_id(raw_a, raw_b, focus_country);
                let advantages = combat_advantages(&side_a, &side_b, focus_country);

                bubbles.push(CombatBubbleSnapshot {
                    id,
                    screen_pos,
                    chance_pct,
                    side_a,
                    side_b,
                    advantages,
                });
            }
        }

        bubbles.sort_by_key(|b| b.id);
        bubbles.truncate(96);
        bubbles
    }

    fn active_combat_divisions(&self, province: u16, owner: hoi4_state::CountryId) -> Vec<usize> {
        self.world
            .divisions_in_province(hoi4_state::ProvinceId(province))
            .iter()
            .copied()
            .filter(|&di| {
                di < self.world.divisions.count
                    && self.world.divisions.owners[di] == owner
                    && self.world.divisions.in_combat[di]
            })
            .collect()
    }

    fn province_has_attack_towards(
        &self,
        source: u16,
        target: u16,
        owner: hoi4_state::CountryId,
    ) -> bool {
        self.world
            .divisions_in_province(hoi4_state::ProvinceId(source))
            .iter()
            .copied()
            .any(|di| {
                if di >= self.world.divisions.count
                    || self.world.divisions.owners[di] != owner
                    || !self.world.divisions.in_combat[di]
                {
                    return false;
                }
                let Some(dest) = self.world.divisions.destinations[di] else {
                    return false;
                };
                if dest.0 == target {
                    return true;
                }
                self.destination_points_through_target(source, target, dest.0)
            })
    }

    fn destination_points_through_target(&self, source: u16, target: u16, dest: u16) -> bool {
        let Some(state) = self.state.as_ref() else {
            return false;
        };
        let centroids = &state.unit_counter_centroids;
        let Some(&(sx, sy)) = centroids.get(source as usize) else {
            return false;
        };
        let Some(&(tx, ty)) = centroids.get(target as usize) else {
            return false;
        };
        let Some(&(dx, dy)) = centroids.get(dest as usize) else {
            return false;
        };
        if (sx + sy == 0.0) || (tx + ty == 0.0) || (dx + dy == 0.0) {
            return false;
        }
        let source_d = (sx - dx).hypot(sy - dy);
        let target_d = (tx - dx).hypot(ty - dy);
        target_d < source_d
    }

    fn combat_side_snapshot(
        &self,
        province: u16,
        owner: hoi4_state::CountryId,
        is_attacking: bool,
    ) -> Option<CombatSideSnapshot> {
        let mut active_divisions = 0usize;
        let mut reserve_divisions = 0usize;
        let mut org_sum = 0.0f32;
        let mut str_sum = 0.0f32;
        let mut combat_width = 0.0f32;
        let mut soft_attack = 0.0f32;
        let mut hard_attack = 0.0f32;
        let mut defense = 0.0f32;
        let mut breakthrough = 0.0f32;

        for &di in self
            .world
            .divisions_in_province(hoi4_state::ProvinceId(province))
        {
            if di >= self.world.divisions.count || self.world.divisions.owners[di] != owner {
                continue;
            }
            if self.world.divisions.strength[di] < 0.05 {
                continue;
            }
            let max_org = self.world.divisions.max_organisation[di].max(1e-6);
            let org_ratio = (self.world.divisions.organisation[di] / max_org).clamp(0.0, 1.0);
            if org_ratio < 0.05 {
                continue;
            }
            let is_active = self.world.divisions.in_combat[di];
            if is_active {
                active_divisions += 1;
            } else {
                reserve_divisions += 1;
            }
            let mut stats = self.division_stats_for_ui(di)?;
            if let Some(modifier) =
                hoi4_logic::military::general::modifier_for_division(&self.world, di)
            {
                stats.soft_attack *= modifier.attack_mult;
                stats.hard_attack *= modifier.attack_mult;
                stats.breakthrough *= modifier.attack_mult;
                stats.defense *= modifier.defense_mult;
            }
            let strength = self.world.divisions.strength[di].clamp(0.0, 1.0);
            let current_factor = strength * org_ratio;
            let display_factor = if is_active {
                current_factor
            } else {
                current_factor * 0.35
            };
            org_sum += org_ratio;
            str_sum += strength;
            combat_width += stats.combat_width;
            soft_attack += stats.soft_attack * display_factor;
            hard_attack += stats.hard_attack * display_factor;
            defense += stats.defense * display_factor;
            breakthrough += stats.breakthrough * display_factor;
        }

        if active_divisions == 0 {
            return None;
        }
        let counted = (active_divisions + reserve_divisions).max(1) as f32;
        let tag = self.world.country_tag(owner).unwrap_or("???").to_owned();
        Some(CombatSideSnapshot {
            country: owner,
            tag,
            name: self.country_display_name(owner),
            province,
            active_divisions,
            reserve_divisions,
            avg_org_pct: (org_sum / counted) * 100.0,
            avg_strength_pct: (str_sum / counted) * 100.0,
            combat_width,
            soft_attack,
            hard_attack,
            defense,
            breakthrough,
            is_attacking,
        })
    }

    fn division_stats_for_ui(
        &self,
        div_idx: usize,
    ) -> Option<hoi4_logic::military::stats::DivisionStats> {
        let owner = self.world.divisions.owners.get(div_idx).copied()?;
        let tag = self.world.country_tag(owner)?;
        let templates = self.world.data.division_templates.get(tag)?;
        let template_idx = *self.world.divisions.template_indices.get(div_idx)? as usize;
        let template = templates.get(template_idx)?;
        Some(hoi4_logic::military::stats::DivisionStats::aggregate(
            template,
            self.world.data.as_ref(),
        ))
    }

    fn combat_contact_screen_pos(
        &self,
        province_a: u16,
        province_b: u16,
        view_proj: &glam::Mat4,
        screen_w: f32,
        screen_h: f32,
    ) -> Option<[f32; 2]> {
        let state = self.state.as_ref()?;
        let (ax, ay) = *state.unit_counter_centroids.get(province_a as usize)?;
        let (bx, by) = *state.unit_counter_centroids.get(province_b as usize)?;
        if ax + ay == 0.0 || bx + by == 0.0 {
            return None;
        }
        let wx = ((ax + bx) * 0.5) * WORLD_SCALE;
        let wz = ((ay + by) * 0.5) * WORLD_SCALE;
        let clip = *view_proj * glam::Vec4::new(wx, 0.42, wz, 1.0);
        if clip.w <= 0.0 {
            return None;
        }
        let ndc_x = clip.x / clip.w;
        let ndc_y = clip.y / clip.w;
        Some([
            (ndc_x * 0.5 + 0.5) * screen_w,
            (1.0 - (ndc_y * 0.5 + 0.5)) * screen_h,
        ])
    }

    fn update_frontline_arrows(&mut self) {
        let sig = self.frontline_arrow_signature();
        if sig == self.prev_armies_hash {
            return;
        }
        self.prev_armies_hash = sig;

        let arrows = self.collect_order_arrows();
        let s = match self.state.as_mut() {
            Some(s) => s,
            None => return,
        };
        s.maparrow_pass.set_arrows(&s.device, &s.queue, &arrows);
    }

    fn frontline_overlay_signature(&self) -> u64 {
        let mut h = DefaultHasher::new();
        self.world.diplomacy.wars.len().hash(&mut h);
        for (id, war) in &self.world.diplomacy.wars {
            id.hash(&mut h);
            let mut attackers: Vec<_> = war.attackers.iter().copied().collect();
            let mut defenders: Vec<_> = war.defenders.iter().copied().collect();
            attackers.sort_by_key(|country| country.0);
            defenders.sort_by_key(|country| country.0);
            attackers.hash(&mut h);
            defenders.hash(&mut h);
        }
        self.world.provinces.controllers.hash(&mut h);
        h.finish()
    }

    fn update_frontline_overlay(&mut self) {
        if self
            .state
            .as_ref()
            .is_some_and(|s| !s.pass_registry.is_enabled("3d_frontlines"))
        {
            return;
        }
        let sig = self.frontline_overlay_signature();
        if sig == self.frontline_overlay_hash {
            return;
        }
        self.frontline_overlay_hash = sig;

        let centroids = match self.state.as_ref() {
            Some(s) => s.unit_counter_centroids.clone(),
            None => return,
        };
        let front_verts =
            generate_frontline_vertices(&self.world, &centroids, WORLD_SCALE, HEIGHT_SCALE);
        let s = match self.state.as_mut() {
            Some(s) => s,
            None => return,
        };
        s.frontlines_vertex_count = front_verts.len() as u32;
        s.frontlines_buffer = s
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("frontline_verts_dynamic"),
                contents: if front_verts.is_empty() {
                    &[0u8; 16]
                } else {
                    bytemuck::cast_slice(&front_verts)
                },
                usage: wgpu::BufferUsages::VERTEX,
            });
    }

    fn render(&mut self) {
        let render_started = Instant::now();
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
        let pre_frame_world_objects = WorldObjectSystem::plan(MapFrameContext {
            draw_3d_map: self.game_phase == GamePhase::Playing,
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
            settings: MapRenderSettings::with_quality(map_layer_mask, self.map_quality_preset),
        });
        let counter_started = Instant::now();
        self.update_hoi3_counter_pass(pre_frame_world_objects);
        let counter_update_ms = counter_started.elapsed().as_secs_f32() * 1000.0;
        self.perf_counter_update_us = self
            .perf_counter_update_us
            .saturating_add((counter_update_ms * 1000.0) as u64);
        let arrow_started = Instant::now();
        self.update_frontline_overlay();
        self.update_frontline_arrows();
        let arrow_update_ms = arrow_started.elapsed().as_secs_f32() * 1000.0;
        self.perf_arrow_update_us = self
            .perf_arrow_update_us
            .saturating_add((arrow_update_ms * 1000.0) as u64);

        let ui_frame_model = ui_binding::build_frame_model(self);
        self.ui_panel_cache.begin_frame();
        let app_ui_enabled = !map_phase0_active || map_layer_mask.ui;

        // Begin the egui frame before rendering; painting happens after 3D passes.
        let elapsed_secs = self.start_time.elapsed().as_secs_f32();
        let game_phase = self.game_phase;
        let player_country = self.player_country;
        let demo_visible = app_ui_enabled && self.demo_visible;
        let mut deferred_switch_player_country: Vec<String> = Vec::new();
        let topbar_data = if app_ui_enabled {
            ui_frame_model.topbar.clone()
        } else {
            None
        };
        let open_panel_kind = if app_ui_enabled {
            ui_frame_model.open_panel
        } else {
            None
        };
        let event_badge_count = self.content.event_scheduler.pending_len();
        let surrender_badge_count = self.pending_surrender_notifications.len();
        let mut topbar_speed_cmd: Option<hoi4_ui::topbar::SpeedCommand> = None;
        let mut side_rail_panel_cmd: Option<hoi4_ui::PanelKind> = None;
        let open_panel = if app_ui_enabled {
            self.open_panel
        } else {
            None
        };
        let politics_data = if open_panel == Some(InGamePanel::Politics) {
            let player = player_country;
            let ruling = self
                .world
                .countries
                .ruling_party
                .get(player)
                .cloned()
                .unwrap_or_default();
            let pop_map = self
                .world
                .countries
                .party_popularity
                .get(player)
                .cloned()
                .unwrap_or_default();
            let mut pops: Vec<(String, f32)> = pop_map.into_iter().collect();
            pops.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            let ideas = self
                .world
                .countries
                .ideas
                .get(player)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .filter_map(|idea_key| {
                    if idea_key.starts_with("FLAG:")
                        || idea_key.starts_with("RESOURCE_DISCOVERY:")
                        || idea_key.starts_with("PM_UNLOCK:")
                        || idea_key.starts_with("GOV_ORDER:")
                        || idea_key.starts_with("MIL_SPENDING_SHARE:")
                    {
                        return None;
                    }
                    let idea_def = self.world.data.ideas.get(&idea_key)?;
                    let name = localized_content_name(&idea_def.key, &idea_def.key);
                    let modifiers = idea_def
                        .modifiers
                        .iter()
                        .map(|(k, v)| (k.clone(), *v))
                        .collect();
                    Some(hoi4_ui::politics::IdeaEntry {
                        key: idea_def.key.clone(),
                        name,
                        category: idea_def.category.clone(),
                        picture: idea_def.picture.clone(),
                        modifiers,
                    })
                })
                .collect::<Vec<_>>();
            let pp = self
                .world
                .countries
                .political_power
                .get(player)
                .copied()
                .unwrap_or(0.0);
            let player_tag_str = self
                .world
                .countries
                .tags
                .get(player)
                .cloned()
                .unwrap_or_default();
            let focus_available = self
                .content
                .focus_tree
                .country
                .eq_ignore_ascii_case(&player_tag_str);
            let (leader_name, leader_portrait_key) = Self::head_of_state_display(
                &self.world,
                &self.historical_1936,
                hoi4_state::CountryId(player as u16),
            );
            let party_loc_key_long = format!("{}_{}_party_long", player_tag_str, ruling);
            let party_loc_key = format!("{}_{}_party", player_tag_str, ruling);
            let party_full_name = self
                .world
                .data
                .party_names
                .get(&party_loc_key_long)
                .or_else(|| self.world.data.party_names.get(&party_loc_key))
                .cloned()
                .unwrap_or_default();
            let current_focus_id = self
                .world
                .countries
                .current_focus
                .get(player)
                .and_then(|f| f.as_deref());
            let current_focus_progress = self
                .world
                .countries
                .focus_progress
                .get(player)
                .copied()
                .unwrap_or(0.0);
            let current_focus = current_focus_id.and_then(|id| {
                self.content
                    .focus_tree
                    .focuses
                    .iter()
                    .find(|focus| focus.id == id)
            });
            let current_focus_name = current_focus.map(|focus| {
                let t = hoi4_ui::i18n::tr(&focus.id);
                if t == focus.id {
                    focus.name.clone()
                } else {
                    t.to_owned()
                }
            });
            let current_focus_cost_days = current_focus.map(|focus| focus.cost_days);

            Some(hoi4_ui::politics::PoliticsData {
                ruling_party: ruling,
                party_popularity: pops,
                ideas,
                political_power: pp,
                stability: self
                    .world
                    .countries
                    .stability
                    .get(player)
                    .copied()
                    .unwrap_or(0.5),
                war_support: self
                    .world
                    .countries
                    .war_support
                    .get(player)
                    .copied()
                    .unwrap_or(0.0),
                focus_available,
                current_focus_name: focus_available.then_some(current_focus_name).flatten(),
                current_focus_progress: if focus_available {
                    current_focus_progress
                } else {
                    0.0
                },
                current_focus_cost_days: if focus_available {
                    current_focus_cost_days
                } else {
                    None
                },
                country_tag: player_tag_str,
                leader_name,
                leader_portrait_key,
                party_full_name,
            })
        } else {
            None
        };
        let mut politics_close = false;
        let mut politics_decision_cmds: Vec<hoi4_ui::politics::DecisionCommand> = Vec::new();
        let decisions_panel_data = if open_panel == Some(InGamePanel::Decisions) {
            let player = player_country;
            Some(hoi4_ui::decisions_panel::DecisionsData {
                country_tag: self
                    .world
                    .countries
                    .tags
                    .get(player)
                    .cloned()
                    .unwrap_or_default(),
                political_power: self
                    .world
                    .countries
                    .political_power
                    .get(player)
                    .copied()
                    .unwrap_or(0.0),
                mechanics: self.build_decision_mechanics(player),
                decisions: self.build_decision_entries(player),
                country_flags: self
                    .world
                    .countries
                    .ideas
                    .get(player)
                    .map(|ideas| {
                        ideas
                            .iter()
                            .filter_map(|s| s.strip_prefix("FLAG:").map(|f| f.to_owned()))
                            .collect()
                    })
                    .unwrap_or_default(),
            })
        } else {
            None
        };
        let mut decisions_close = false;
        let mut decisions_cmds: Vec<hoi4_ui::politics::DecisionCommand> = Vec::new();
        let law_panel_data = if open_panel == Some(InGamePanel::Laws) {
            let player = player_country;
            let pp = self
                .world
                .countries
                .political_power
                .get(player)
                .copied()
                .unwrap_or(0.0);
            let player_id = hoi4_state::CountryId(player as u16);
            let law_set = self.world.countries.law_store.law_sets.get(player).cloned();
            let slots: Vec<hoi4_ui::law_panel::LawSlotEntry> = if let Some(ls) = law_set {
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
                        let tiers = build_law_tiers(cat, &self.v6_db);
                        let current_name = tiers
                            .iter()
                            .find(|t| t.id == slot.current)
                            .map(|t| t.name.clone())
                            .unwrap_or_else(|| {
                                localized_content_name(&slot.current, &slot.current)
                            });
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
                                Some("planned economy".to_owned())
                            } else {
                                None
                            },
                            previous_before_lock: slot.previous_before_lock.clone(),
                            tiers,
                        }
                    })
                    .collect()
            } else {
                Vec::new()
            };
            Some(hoi4_ui::law_panel::LawPanelData {
                political_power: pp,
                slots,
            })
        } else {
            None
        };
        let mut law_close = false;
        let mut law_cmds: Vec<hoi4_ui::law_panel::LawCommand> = Vec::new();
        let pop_panel_data = if open_panel == Some(InGamePanel::Pops) {
            let player = hoi4_state::CountryId(player_country as u16);
            let state_ids = self.world.country_state_ids(player);
            let class_name = |class: hoi4_state::PopClass| -> &'static str {
                match class {
                    hoi4_state::PopClass::Peasant => "Peasant",
                    hoi4_state::PopClass::Worker => "Worker",
                    hoi4_state::PopClass::Clerk => "Clerk",
                    hoi4_state::PopClass::Capitalist => "Capitalist",
                    hoi4_state::PopClass::Aristocrat => "Aristocrat",
                    hoi4_state::PopClass::Soldier => "Soldier",
                }
            };
            let integration_label = |status: hoi4_state::StateIntegrationStatus| -> &'static str {
                match status {
                    hoi4_state::StateIntegrationStatus::Metropole => "本土",
                    hoi4_state::StateIntegrationStatus::Incorporated => "整合",
                    hoi4_state::StateIntegrationStatus::Colony => "殖民地",
                    hoi4_state::StateIntegrationStatus::Protectorate => "保护国",
                    hoi4_state::StateIntegrationStatus::Mandate => "委任统治",
                    hoi4_state::StateIntegrationStatus::Concession => "租界",
                    hoi4_state::StateIntegrationStatus::Occupied => "占领区",
                }
            };

            #[derive(Clone, Copy, Default)]
            struct PopAgg {
                size: u64,
                employed: u64,
                wage_weighted: f64,
                tax_weighted: f64,
                income_weighted: f64,
                tax_paid_weighted: f64,
                disposable_income_weighted: f64,
                satisfaction_weighted: f64,
                loyalty_weighted: f64,
                standard_of_living_weighted: f64,
                literacy_weighted: f64,
                skilled_weighted: f64,
                needs_weighted: f64,
                essential_needs_weighted: f64,
                normal_needs_weighted: f64,
                luxury_needs_weighted: f64,
                radicalism_weighted: f64,
            }

            impl PopAgg {
                fn add(&mut self, pg: &hoi4_state::PopGroup) {
                    let size = pg.size as u64;
                    self.size += size;
                    if pg.employed_at.is_some() || pg.class == hoi4_state::PopClass::Soldier {
                        self.employed += size;
                    }
                    let weight = pg.size as f64;
                    self.wage_weighted += pg.wage_rm as f64 * weight;
                    self.tax_weighted += pg.tax_burden as f64 * weight;
                    self.income_weighted += pg.income_rm as f64 * weight;
                    self.tax_paid_weighted += pg.tax_paid_rm as f64 * weight;
                    self.disposable_income_weighted += pg.disposable_income_rm as f64 * weight;
                    self.satisfaction_weighted += pg.satisfaction as f64 * weight;
                    self.loyalty_weighted += pg.political_loyalty as f64 * weight;
                    self.standard_of_living_weighted += pg.standard_of_living as f64 * weight;
                    self.literacy_weighted += pg.literacy as f64 * weight;
                    self.skilled_weighted += pg.skilled_ratio as f64 * weight;
                    self.needs_weighted += pg.needs_fulfillment as f64 * weight;
                    self.essential_needs_weighted += pg.essential_needs_fulfillment as f64 * weight;
                    self.normal_needs_weighted += pg.normal_needs_fulfillment as f64 * weight;
                    self.luxury_needs_weighted += pg.luxury_needs_fulfillment as f64 * weight;
                    self.radicalism_weighted += pg.radicalism as f64 * weight;
                }

                fn avg_wage(self) -> f32 {
                    if self.size > 0 {
                        (self.wage_weighted / self.size as f64) as f32
                    } else {
                        0.0
                    }
                }

                fn avg_tax(self) -> f32 {
                    if self.size > 0 {
                        (self.tax_weighted / self.size as f64) as f32
                    } else {
                        0.0
                    }
                }

                fn avg_income(self) -> f32 {
                    if self.size > 0 {
                        (self.income_weighted / self.size as f64) as f32
                    } else {
                        0.0
                    }
                }

                fn avg_tax_paid(self) -> f32 {
                    if self.size > 0 {
                        (self.tax_paid_weighted / self.size as f64) as f32
                    } else {
                        0.0
                    }
                }

                fn avg_disposable_income(self) -> f32 {
                    if self.size > 0 {
                        (self.disposable_income_weighted / self.size as f64) as f32
                    } else {
                        0.0
                    }
                }

                fn avg_satisfaction(self) -> f32 {
                    if self.size > 0 {
                        (self.satisfaction_weighted / self.size as f64) as f32
                    } else {
                        0.0
                    }
                }

                fn avg_loyalty(self) -> f32 {
                    if self.size > 0 {
                        (self.loyalty_weighted / self.size as f64) as f32
                    } else {
                        0.0
                    }
                }

                fn avg_standard_of_living(self) -> f32 {
                    if self.size > 0 {
                        (self.standard_of_living_weighted / self.size as f64) as f32
                    } else {
                        0.0
                    }
                }

                fn avg_needs(self) -> f32 {
                    if self.size > 0 {
                        (self.needs_weighted / self.size as f64) as f32
                    } else {
                        0.0
                    }
                }

                fn avg_literacy(self) -> f32 {
                    if self.size > 0 {
                        (self.literacy_weighted / self.size as f64) as f32
                    } else {
                        0.0
                    }
                }

                fn avg_skilled(self) -> f32 {
                    if self.size > 0 {
                        (self.skilled_weighted / self.size as f64) as f32
                    } else {
                        0.0
                    }
                }

                fn avg_essential_needs(self) -> f32 {
                    if self.size > 0 {
                        (self.essential_needs_weighted / self.size as f64) as f32
                    } else {
                        0.0
                    }
                }

                fn avg_normal_needs(self) -> f32 {
                    if self.size > 0 {
                        (self.normal_needs_weighted / self.size as f64) as f32
                    } else {
                        0.0
                    }
                }

                fn avg_luxury_needs(self) -> f32 {
                    if self.size > 0 {
                        (self.luxury_needs_weighted / self.size as f64) as f32
                    } else {
                        0.0
                    }
                }

                fn avg_radicalism(self) -> f32 {
                    if self.size > 0 {
                        (self.radicalism_weighted / self.size as f64) as f32
                    } else {
                        0.0
                    }
                }
            }

            let mut total = PopAgg::default();
            let mut classes = [PopAgg::default(); hoi4_state::PopClass::COUNT];
            let mut soldier_pool = 0_u64;
            let mut state_aggs: std::collections::HashMap<hoi4_state::StateId, PopAgg> =
                std::collections::HashMap::new();
            let mut state_class_sizes: std::collections::HashMap<
                hoi4_state::StateId,
                [u64; hoi4_state::PopClass::COUNT],
            > = std::collections::HashMap::new();

            let state_id_set: std::collections::HashSet<hoi4_state::StateId> =
                state_ids.iter().copied().collect();

            for pg in self.world.countries.pops.groups.iter() {
                let state_idx = pg.state.0 as usize;
                if state_idx >= self.world.states.count
                    || self.world.states.owners[state_idx] != player
                    || !state_id_set.contains(&pg.state)
                {
                    continue;
                }
                total.add(pg);
                classes[pg.class.index()].add(pg);
                state_aggs.entry(pg.state).or_default().add(pg);
                state_class_sizes.entry(pg.state).or_default()[pg.class.index()] += pg.size as u64;
                if pg.class == hoi4_state::PopClass::Soldier && pg.employed_at.is_none() {
                    soldier_pool += pg.size as u64;
                }
            }

            let class_entries: Vec<hoi4_ui::pop_panel::PopClassEntry> = (0
                ..hoi4_state::PopClass::COUNT)
                .filter_map(|idx| {
                    let class = hoi4_state::PopClass::from_index(idx)?;
                    let agg = classes[idx];
                    if agg.size == 0 {
                        return None;
                    }
                    Some(hoi4_ui::pop_panel::PopClassEntry {
                        class_name: class_name(class).to_owned(),
                        size: agg.size,
                        employed: agg.employed,
                        unemployed: agg.size.saturating_sub(agg.employed),
                        avg_wage_rm: agg.avg_wage(),
                        avg_tax_burden: agg.avg_tax(),
                        avg_income_rm: agg.avg_income(),
                        avg_tax_paid_rm: agg.avg_tax_paid(),
                        avg_disposable_income_rm: agg.avg_disposable_income(),
                        avg_satisfaction: agg.avg_satisfaction(),
                        avg_loyalty: agg.avg_loyalty(),
                        avg_standard_of_living: agg.avg_standard_of_living(),
                        literacy: agg.avg_literacy(),
                        skilled_ratio: agg.avg_skilled(),
                        needs_fulfillment: agg.avg_needs(),
                        essential_needs_fulfillment: agg.avg_essential_needs(),
                        normal_needs_fulfillment: agg.avg_normal_needs(),
                        luxury_needs_fulfillment: agg.avg_luxury_needs(),
                        radicalism: agg.avg_radicalism(),
                    })
                })
                .collect();

            let state_names = self.world.states.names.clone();
            let loc_catalog = self.loc_catalog.clone();
            let mut state_entries: Vec<hoi4_ui::pop_panel::PopStateEntry> = state_ids
                .iter()
                .map(|state_id| {
                    let agg = state_aggs.get(state_id).copied().unwrap_or_default();
                    let state_idx = state_id.0 as usize;
                    let state_name = state_names
                        .get(state_idx)
                        .map(|raw| localized_state_name(raw, state_idx, &loc_catalog))
                        .unwrap_or_else(|| format!("第{}州", state_idx));
                    let integration_status = self.world.states.integration_status[state_idx];
                    let integration_kind = if integration_status.is_colonial_or_occupied() {
                        hoi4_ui::pop_panel::PopIntegrationKind::Colonial
                    } else {
                        hoi4_ui::pop_panel::PopIntegrationKind::Domestic
                    };
                    let dominant_class = state_class_sizes
                        .get(state_id)
                        .and_then(|sizes| {
                            sizes
                                .iter()
                                .enumerate()
                                .max_by_key(|(_, size)| **size)
                                .and_then(|(idx, _)| hoi4_state::PopClass::from_index(idx))
                        })
                        .map(class_name)
                        .unwrap_or("Unknown")
                        .to_owned();
                    hoi4_ui::pop_panel::PopStateEntry {
                        state_name,
                        state_id: state_id.0,
                        integration_kind,
                        integration_label: integration_label(integration_status).to_owned(),
                        population: agg.size,
                        employed: agg.employed,
                        unemployment_rate: if agg.size > 0 {
                            agg.size.saturating_sub(agg.employed) as f32 / agg.size as f32
                        } else {
                            0.0
                        },
                        avg_satisfaction: agg.avg_satisfaction(),
                        avg_wage_rm: agg.avg_wage(),
                        avg_income_rm: agg.avg_income(),
                        avg_disposable_income_rm: agg.avg_disposable_income(),
                        dominant_class,
                    }
                })
                .collect();
            let zero_pop_alert: Vec<String> = state_entries
                .iter()
                .filter(|s| s.population == 0)
                .map(|s| format!("州#{}", s.state_id))
                .collect();
            let zero_pop_count = zero_pop_alert.len();
            state_entries.sort_by(|a, b| b.population.cmp(&a.population));

            let workforce = total
                .size
                .saturating_sub(classes[hoi4_state::PopClass::Soldier.index()].size);
            let employed = total.employed;
            let unemployed = total.size.saturating_sub(total.employed);
            let unemployment_rate = if total.size > 0 {
                unemployed as f32 / total.size as f32
            } else {
                0.0
            };
            let radicalism = total.avg_radicalism();
            let strike_risk = ((radicalism - 0.25) / 0.50).clamp(0.0, 1.0);
            let draft_resistance = ((radicalism - 0.15) / 0.55).clamp(0.0, 1.0);
            let mut political_pressures = Vec::new();
            if total.avg_essential_needs() < 0.8 {
                political_pressures.push(hoi4_ui::pop_panel::PopPoliticalPressureEntry {
                    source: "基础需求不足".to_owned(),
                    pressure: ((0.8 - total.avg_essential_needs()) / 0.8).clamp(0.0, 1.0),
                    description: "粮食、衣物等基础商品短缺正在推高激进化".to_owned(),
                });
            }
            if unemployment_rate > 0.10 {
                political_pressures.push(hoi4_ui::pop_panel::PopPoliticalPressureEntry {
                    source: "失业".to_owned(),
                    pressure: ((unemployment_rate - 0.10) / 0.40).clamp(0.0, 1.0),
                    description: "劳动力闲置削弱就业安全并增加社会不满".to_owned(),
                });
            }
            if total.avg_tax() > 0.50 {
                political_pressures.push(hoi4_ui::pop_panel::PopPoliticalPressureEntry {
                    source: "高税负".to_owned(),
                    pressure: ((total.avg_tax() - 0.50) / 0.50).clamp(0.0, 1.0),
                    description: "税负超过可接受水平，压低生活水平与满意度".to_owned(),
                });
            }
            if total.avg_satisfaction() < 0.45 {
                political_pressures.push(hoi4_ui::pop_panel::PopPoliticalPressureEntry {
                    source: "低满意度".to_owned(),
                    pressure: ((0.45 - total.avg_satisfaction()) / 0.45).clamp(0.0, 1.0),
                    description: "长期低满意度会转化为组织化政治压力".to_owned(),
                });
            }
            let avg_income = total.avg_income();
            let avg_disposable = total.avg_disposable_income();
            if avg_income > 0.0 && avg_disposable / avg_income < 0.5 {
                political_pressures.push(hoi4_ui::pop_panel::PopPoliticalPressureEntry {
                    source: "低可支配收入".to_owned(),
                    pressure: ((0.5 - avg_disposable / avg_income) / 0.5).clamp(0.0, 1.0),
                    description: "税后可支配收入占比过低，抑制消费与生活水平".to_owned(),
                });
            }
            let needs = vec![
                hoi4_ui::pop_panel::PopNeedEntry {
                    tier_name: "基础需求".to_owned(),
                    fulfillment: total.avg_essential_needs(),
                    description: "粮食、衣物、燃料等维持生活的商品".to_owned(),
                },
                hoi4_ui::pop_panel::PopNeedEntry {
                    tier_name: "普通需求".to_owned(),
                    fulfillment: total.avg_normal_needs(),
                    description: "肉类、家具、交通、烟酒等日常消费".to_owned(),
                },
                hoi4_ui::pop_panel::PopNeedEntry {
                    tier_name: "奢侈需求".to_owned(),
                    fulfillment: total.avg_luxury_needs(),
                    description: "奢侈品、银行、汽车、无线电等高端消费".to_owned(),
                },
            ];
            Some(hoi4_ui::pop_panel::PopPanelData {
                total_population: total.size,
                workforce,
                employed,
                unemployed,
                unemployment_rate,
                average_wage_rm: total.avg_wage(),
                average_income_rm: total.avg_income(),
                average_disposable_income_rm: total.avg_disposable_income(),
                average_satisfaction: total.avg_satisfaction(),
                average_loyalty: total.avg_loyalty(),
                average_standard_of_living: total.avg_standard_of_living(),
                literacy: total.avg_literacy(),
                skilled_ratio: total.avg_skilled(),
                needs_fulfillment: total.avg_needs(),
                essential_needs_fulfillment: total.avg_essential_needs(),
                normal_needs_fulfillment: total.avg_normal_needs(),
                luxury_needs_fulfillment: total.avg_luxury_needs(),
                radicalism,
                strike_risk,
                draft_resistance,
                soldier_pool,
                classes: class_entries,
                states: state_entries,
                needs,
                political_pressures,
                alerts: if total.size == 0 {
                    vec!["当前国家没有 POP 数据；请检查历史开局 POP 注入。".to_owned()]
                } else if zero_pop_count > 0 {
                    vec![format!(
                        "{} 个州无人口数据（{}），可能缺少 state_population 配置",
                        zero_pop_count,
                        zero_pop_alert.join(", ")
                    )]
                } else {
                    Vec::new()
                },
            })
        } else {
            None
        };
        let mut pop_panel_close = false;
        let market_panel_data = if open_panel == Some(InGamePanel::Market) {
            let cache_started = Instant::now();
            let cache_key = UiPanelCacheKey::new(
                player_country,
                self.world.date.days_since_epoch(),
                Self::market_panel_signature(&self.world, player_country),
            );
            if let Some(data) = self
                .ui_panel_cache
                .market
                .as_ref()
                .filter(|(key, _)| *key == cache_key)
                .map(|(_, data)| data.clone())
            {
                self.ui_panel_cache
                    .record(UiPanelCacheKind::Market, cache_started.elapsed(), true);
                Some(data)
            } else {
                let built = {
                    let player = player_country;
                    let market = &self.world.countries.market.markets[player];
                    let player_id = hoi4_state::CountryId(player as u16);
                    let mut producers: std::collections::HashMap<
                        String,
                        Vec<hoi4_ui::market_panel::GoodFlowSource>,
                    > = std::collections::HashMap::new();
                    let mut consumers: std::collections::HashMap<
                        String,
                        Vec<hoi4_ui::market_panel::GoodFlowSource>,
                    > = std::collections::HashMap::new();
                    for building in &self.world.countries.buildings_v6.buildings {
                        let state_idx = building.state.0 as usize;
                        if state_idx >= self.world.states.count
                            || self.world.states.owners[state_idx] != player_id
                            || building.level == 0
                        {
                            continue;
                        }
                        let state_name = self
                            .world
                            .states
                            .names
                            .get(state_idx)
                            .cloned()
                            .unwrap_or_default();
                        let building_name = self
                            .v6_db
                            .buildings
                            .iter()
                            .find(|bd| bd.id == building.building_def_id)
                            .map(|bd| bd.name.as_str())
                            .unwrap_or(building.building_def_id.as_str());
                        let label = if state_name.is_empty() {
                            building_name.to_owned()
                        } else {
                            format!("{} - {}", state_name, building_name)
                        };
                        let pms = hoi4_content::active_pms_for_building(building, &self.v6_db);
                        let throughput = building.production_rate.max(0.0).max(1.0);
                        for pm in pms {
                            let pm_throughput = pm.throughput_modifier.max(0.0) * throughput;
                            for (i, good_id) in pm.output_good_ids.iter().enumerate() {
                                let amount = pm.output_good_amounts.get(i).copied().unwrap_or(0.0)
                                    * building.level as f32
                                    * pm_throughput;
                                if amount > 0.0 {
                                    producers.entry(good_id.clone()).or_default().push(
                                        hoi4_ui::market_panel::GoodFlowSource {
                                            name: label.clone(),
                                            amount,
                                        },
                                    );
                                }
                            }
                            for (i, good_id) in pm.input_good_ids.iter().enumerate() {
                                let amount = pm.input_good_amounts.get(i).copied().unwrap_or(0.0)
                                    * building.level as f32;
                                if amount > 0.0 {
                                    consumers.entry(good_id.clone()).or_default().push(
                                        hoi4_ui::market_panel::GoodFlowSource {
                                            name: label.clone(),
                                            amount,
                                        },
                                    );
                                }
                            }
                        }
                    }
                    for rows in producers.values_mut() {
                        rows.sort_by(|a, b| {
                            b.amount
                                .partial_cmp(&a.amount)
                                .unwrap_or(std::cmp::Ordering::Equal)
                        });
                    }
                    for rows in consumers.values_mut() {
                        rows.sort_by(|a, b| {
                            b.amount
                                .partial_cmp(&a.amount)
                                .unwrap_or(std::cmp::Ordering::Equal)
                        });
                    }
                    let economy_law = self.world.countries.law_store.law_sets[player].0
                        [hoi4_state::LawCategory::Economy.index()]
                    .current
                    .clone();
                    let division_count = self
                        .world
                        .divisions
                        .owners
                        .iter()
                        .filter(|owner| **owner == player_id)
                        .count() as f32;
                    let base_procurement = match economy_law.as_str() {
                        "corporatist_war_economy" => 8.0,
                        "war_economy" => 3.0,
                        "interventionism" => 1.0,
                        _ => 0.0,
                    };
                    let mefo_credit_mult = if economy_law == "corporatist_war_economy" {
                        self.world
                            .countries
                            .treasury
                            .treasuries
                            .get(player)
                            .map(|t| {
                                let mefo_room_ratio = if t.gdp_rm > 0.0 {
                                    ((t.gdp_rm * 0.30 - t.mefo_debt_rm).max(0.0) / t.gdp_rm) as f32
                                } else {
                                    0.30
                                };
                                1.0 + mefo_room_ratio * 10.0
                            })
                            .unwrap_or(1.0)
                    } else {
                        1.0
                    };
                    let procurement_mult =
                        base_procurement * (1.0 + division_count * 0.05) * mefo_credit_mult;
                    let mut government_orders: std::collections::HashMap<
                        String,
                        Vec<hoi4_ui::market_panel::GoodFlowSource>,
                    > = std::collections::HashMap::new();
                    if procurement_mult > 0.0 {
                        for g in &self.v6_db.goods {
                            if g.category
                                == hoi4_content::v6_loader::GoodCategoryDef::MilitaryIntermediate
                            {
                                government_orders.entry(g.id.clone()).or_default().push(
                                    hoi4_ui::market_panel::GoodFlowSource {
                                        name: "军工政府采购".to_owned(),
                                        amount: procurement_mult,
                                    },
                                );
                            }
                        }
                        for pm in &self.v6_db.production_methods {
                            if pm.equipment_output.is_some() {
                                for (i, good_id) in pm.input_good_ids.iter().enumerate() {
                                    let amount =
                                        pm.input_good_amounts.get(i).copied().unwrap_or(0.0)
                                            * procurement_mult;
                                    if amount > 0.0 {
                                        government_orders.entry(good_id.clone()).or_default().push(
                                            hoi4_ui::market_panel::GoodFlowSource {
                                                name: localized_content_name(&pm.id, &pm.name),
                                                amount,
                                            },
                                        );
                                    }
                                }
                            }
                        }
                    }
                    let mut pop_consumption: std::collections::HashMap<
                        String,
                        Vec<hoi4_ui::market_panel::GoodFlowSource>,
                    > = std::collections::HashMap::new();
                    let class_name = |class: hoi4_state::PopClass| -> &'static str {
                        match class {
                            hoi4_state::PopClass::Peasant => "Peasant",
                            hoi4_state::PopClass::Worker => "Worker",
                            hoi4_state::PopClass::Clerk => "Clerk",
                            hoi4_state::PopClass::Capitalist => "Capitalist",
                            hoi4_state::PopClass::Aristocrat => "Aristocrat",
                            hoi4_state::PopClass::Soldier => "Soldier",
                        }
                    };
                    for class_idx in 0..hoi4_state::PopClass::COUNT {
                        let Some(class) = hoi4_state::PopClass::from_index(class_idx) else {
                            continue;
                        };
                        let needs = self.v6_db.pop_need_entries_for_class(class);
                        if needs.is_empty() {
                            continue;
                        }
                        let total_pop: f32 = self
                            .world
                            .countries
                            .pops
                            .groups
                            .iter()
                            .filter(|pop| {
                                pop.class == class
                                    && pop.employed_at.is_some()
                                    && self
                                        .world
                                        .states
                                        .owners
                                        .get(pop.state.0 as usize)
                                        .copied()
                                        .unwrap_or(hoi4_state::CountryId::NONE)
                                        == player_id
                            })
                            .map(|pop| {
                                let si = pop.state.0 as usize;
                                let factor = self
                            .world
                            .states
                            .integration_status
                            .get(si)
                            .copied()
                            .map(hoi4_logic::economy::finance_tick::integration_consumption_factor)
                            .unwrap_or(1.0);
                                pop.size as f32 * factor
                            })
                            .sum();
                        let pop_millions = total_pop / 1_000_000.0;
                        if pop_millions > 0.0 {
                            for need in needs {
                                let amount = pop_millions * need.amount_per_million;
                                if amount <= 0.0 {
                                    continue;
                                }
                                pop_consumption
                                    .entry(need.good_id.clone())
                                    .or_default()
                                    .push(hoi4_ui::market_panel::GoodFlowSource {
                                        name: class_name(class).to_owned(),
                                        amount,
                                    });
                            }
                        }
                    }
                    let mut upstream_goods: std::collections::HashMap<String, Vec<String>> =
                        std::collections::HashMap::new();
                    let mut downstream_goods: std::collections::HashMap<String, Vec<String>> =
                        std::collections::HashMap::new();
                    for pm in &self.v6_db.production_methods {
                        for output in &pm.output_good_ids {
                            let upstream = upstream_goods.entry(output.clone()).or_default();
                            for input in &pm.input_good_ids {
                                if !upstream.contains(input) {
                                    upstream.push(input.clone());
                                }
                            }
                        }
                        for input in &pm.input_good_ids {
                            let downstream = downstream_goods.entry(input.clone()).or_default();
                            for output in &pm.output_good_ids {
                                if !downstream.contains(output) {
                                    downstream.push(output.clone());
                                }
                            }
                            if let Some(equipment) = &pm.equipment_output {
                                if !downstream.contains(&equipment.equipment_category) {
                                    downstream.push(equipment.equipment_category.clone());
                                }
                            }
                        }
                    }
                    let construction_demand = {
                        let cp = hoi4_logic::economy::construction_tick::construction_cp_pool(
                            &self.world,
                            player,
                        );
                        if self
                            .econ
                            .construction
                            .get(player)
                            .map(|q| q.items.is_empty())
                            .unwrap_or(true)
                        {
                            0.0
                        } else {
                            cp * 0.05
                        }
                    };
                    let any_blockaded = self.world.countries.trade.routes.iter().any(|r| {
                        (r.importer == player_id || r.exporter == player_id) && r.is_blockaded
                    });
                    let exchange_rate = self
                        .world
                        .countries
                        .treasury
                        .exchange_rates
                        .get(player)
                        .map(|er| er.rm_per_gbp)
                        .unwrap_or(12.5);
                    let paid_procurement_rm = self
                        .world
                        .countries
                        .treasury
                        .treasuries
                        .get(player)
                        .map(|t| t.daily_budget.expense_military_procurement_rm)
                        .unwrap_or(0.0);
                    let total_government_order_amount: f32 = government_orders
                        .values()
                        .flat_map(|rows| rows.iter())
                        .map(|row| row.amount)
                        .sum();
                    let goods: Vec<hoi4_ui::market_panel::GoodEntry> = self
                        .v6_db
                        .goods
                        .iter()
                        .map(|g| {
                            let price = market.price.get(&g.id).copied().unwrap_or(g.base_price_rm);
                            let supply = market.supply.get(&g.id).copied().unwrap_or(0.0);
                            let demand = market.demand.get(&g.id).copied().unwrap_or(0.0);
                            let stockpile = market.stockpile.get(&g.id).copied().unwrap_or(0.0);
                            let stockpile_coverage_days = market
                                .stockpile_coverage_days
                                .get(&g.id)
                                .copied()
                                .unwrap_or_else(|| {
                                    if demand > 0.0 {
                                        stockpile / demand
                                    } else {
                                        0.0
                                    }
                                });
                            let unmet_demand =
                                market.unmet_demand.get(&g.id).copied().unwrap_or(0.0);
                            let imports = market.imports.get(&g.id).copied().unwrap_or(0.0);
                            let exports = market.exports.get(&g.id).copied().unwrap_or(0.0);
                            let mut supply_sources: Vec<
                                hoi4_ui::market_panel::GoodSupplySourceEntry,
                            > = Vec::new();
                            let good_producers = producers.get(&g.id).cloned().unwrap_or_default();
                            let good_consumers = consumers.get(&g.id).cloned().unwrap_or_default();
                            let domestic_production: f32 =
                                good_producers.iter().map(|row| row.amount).sum();
                            if domestic_production > 0.0 {
                                supply_sources.push(hoi4_ui::market_panel::GoodSupplySourceEntry {
                                    kind: hoi4_ui::market_panel::GoodSupplySourceKind::Domestic,
                                    label: "国内建筑".to_owned(),
                                    amount: domestic_production,
                                });
                            }
                            let building_input_demand: f32 =
                                good_consumers.iter().map(|row| row.amount).sum();
                            let affected_pop_classes =
                                pop_consumption.get(&g.id).cloned().unwrap_or_default();
                            let pop_consumption_demand: f32 =
                                affected_pop_classes.iter().map(|row| row.amount).sum();
                            let good_government_orders =
                                government_orders.get(&g.id).cloned().unwrap_or_default();
                            let military_order_demand: f32 =
                                good_government_orders.iter().map(|row| row.amount).sum();
                            let construction_demand_for_good = if g.id == "machinery" {
                                construction_demand
                            } else {
                                0.0
                            };
                            let stockpile_draw = stockpile.min(demand.max(0.0) * 0.25);
                            if stockpile_draw > 0.0 {
                                supply_sources.push(hoi4_ui::market_panel::GoodSupplySourceEntry {
                                    kind: hoi4_ui::market_panel::GoodSupplySourceKind::Stockpile,
                                    label: "库存释放".to_owned(),
                                    amount: stockpile_draw,
                                });
                            }
                            for route in self.world.countries.trade.routes.iter().filter(|route| {
                                route.importer == player_id
                                    && route.good_id.as_deref() == Some(g.id.as_str())
                            }) {
                                if route.throughput <= 0.0 {
                                    continue;
                                }
                                let (kind, label) = if route.exporter.is_none() {
                                    (
                                        hoi4_ui::market_panel::GoodSupplySourceKind::WorldSpot,
                                        "世界现货".to_owned(),
                                    )
                                } else {
                                    let tag = self
                                        .world
                                        .countries
                                        .tags
                                        .get(route.exporter.0 as usize)
                                        .cloned()
                                        .unwrap_or_else(|| "未知伙伴".to_owned());
                                    if self
                                        .world
                                        .diplomacy
                                        .is_subject_of(route.exporter, player_id)
                                    {
                                        (
                                            hoi4_ui::market_panel::GoodSupplySourceKind::Subject,
                                            format!("{tag} 殖民/傀儡贡献"),
                                        )
                                    } else if self
                                        .world
                                        .countries
                                        .market
                                        .country_bloc
                                        .get(player)
                                        .copied()
                                        .flatten()
                                        == self
                                            .world
                                            .countries
                                            .market
                                            .country_bloc
                                            .get(route.exporter.0 as usize)
                                            .copied()
                                            .flatten()
                                    {
                                        (
                                            hoi4_ui::market_panel::GoodSupplySourceKind::MarketBloc,
                                            format!("{tag} 市场圈输入"),
                                        )
                                    } else {
                                        (
                                            hoi4_ui::market_panel::GoodSupplySourceKind::WorldSpot,
                                            format!("{tag} 外部进口"),
                                        )
                                    }
                                };
                                supply_sources.push(hoi4_ui::market_panel::GoodSupplySourceEntry {
                                    kind,
                                    label,
                                    amount: route.throughput,
                                });
                            }
                            let traded = if military_order_demand > 0.0 {
                                military_order_demand.min((supply + stockpile_draw).max(0.0))
                            } else {
                                demand.min((supply + stockpile_draw).max(0.0))
                            };
                            let paid_rm = if total_government_order_amount > 0.0
                                && military_order_demand > 0.0
                            {
                                paid_procurement_rm
                                    * (military_order_demand / total_government_order_amount) as f64
                            } else {
                                0.0
                            };
                            let affected_buildings = if demand > supply {
                                good_consumers.clone()
                            } else {
                                Vec::new()
                            };
                            let category = match g.category {
                                hoi4_content::v6_loader::GoodCategoryDef::RawMaterial => {
                                    hoi4_ui::market_panel::GoodCategory::RawMaterial
                                }
                                hoi4_content::v6_loader::GoodCategoryDef::Intermediate => {
                                    hoi4_ui::market_panel::GoodCategory::Intermediate
                                }
                                hoi4_content::v6_loader::GoodCategoryDef::Consumer => {
                                    hoi4_ui::market_panel::GoodCategory::Consumer
                                }
                                hoi4_content::v6_loader::GoodCategoryDef::Luxury => {
                                    hoi4_ui::market_panel::GoodCategory::Luxury
                                }
                                hoi4_content::v6_loader::GoodCategoryDef::Service => {
                                    hoi4_ui::market_panel::GoodCategory::Service
                                }
                                hoi4_content::v6_loader::GoodCategoryDef::MilitaryIntermediate => {
                                    hoi4_ui::market_panel::GoodCategory::MilitaryIntermediate
                                }
                            };
                            let good_name = Self::v6_good_name(&self.v6_db, &g.id);
                            hoi4_ui::market_panel::GoodEntry {
                                id: g.id.clone(),
                                name: good_name,
                                category,
                                price,
                                base_price: g.base_price_rm,
                                supply,
                                demand,
                                traded,
                                stockpile,
                                stockpile_coverage_days,
                                unmet_demand,
                                domestic_production,
                                stockpile_draw,
                                building_input_demand,
                                pop_consumption_demand,
                                military_order_demand,
                                supply_sources,
                                producers: good_producers,
                                consumers: good_consumers,
                                government_orders: good_government_orders,
                                construction_demand: construction_demand_for_good,
                                imports,
                                exports,
                                is_blockaded: any_blockaded && imports > 0.0,
                                affected_buildings,
                                affected_pop_classes: if demand > supply {
                                    affected_pop_classes
                                } else {
                                    Vec::new()
                                },
                                upstream_goods: upstream_goods
                                    .get(&g.id)
                                    .cloned()
                                    .unwrap_or_default(),
                                downstream_goods: downstream_goods
                                    .get(&g.id)
                                    .cloned()
                                    .unwrap_or_default(),
                                paid_rm,
                                clearing_fulfilled: market
                                    .clearing_sheet
                                    .results
                                    .get(&g.id)
                                    .map(|r| r.total_fulfilled)
                                    .unwrap_or(0.0),
                                clearing_unmet: market
                                    .clearing_sheet
                                    .results
                                    .get(&g.id)
                                    .map(|r| r.total_unmet)
                                    .unwrap_or(0.0),
                                clearing_shortage_ratio: market
                                    .clearing_sheet
                                    .results
                                    .get(&g.id)
                                    .map(|r| r.shortage_ratio)
                                    .unwrap_or(0.0),
                            }
                        })
                        .collect();
                    let total_shortage_value_rm: f64 = goods
                        .iter()
                        .map(|good| {
                            good.unmet_demand.max((good.demand - good.supply).max(0.0)) as f64
                                * good.price as f64
                        })
                        .sum();
                    let total_import_value_gbp: f64 = goods
                        .iter()
                        .map(|good| {
                            good.imports as f64 * good.price as f64 / exchange_rate.max(0.01) as f64
                        })
                        .sum();
                    let total_export_value_gbp: f64 = goods
                        .iter()
                        .map(|good| {
                            good.exports as f64 * good.price as f64 / exchange_rate.max(0.01) as f64
                        })
                        .sum();
                    let pop_demand_total: f32 =
                        goods.iter().map(|good| good.pop_consumption_demand).sum();
                    let pop_shortage_total: f32 = goods
                        .iter()
                        .filter(|good| good.pop_consumption_demand > 0.0)
                        .map(|good| good.unmet_demand.min(good.pop_consumption_demand))
                        .sum();
                    let pop_needs_fulfillment = if pop_demand_total > 0.0 {
                        (1.0 - pop_shortage_total / pop_demand_total).clamp(0.0, 1.0)
                    } else {
                        1.0
                    };
                    let military_order_total: f32 =
                        goods.iter().map(|good| good.military_order_demand).sum();
                    let military_unmet_total: f32 = goods
                        .iter()
                        .filter(|good| good.military_order_demand > 0.0)
                        .map(|good| (good.military_order_demand - good.traded).max(0.0))
                        .sum();
                    let military_supply_pressure = if military_order_total > 0.0 {
                        (military_unmet_total / military_order_total).clamp(0.0, 1.0)
                    } else {
                        0.0
                    };
                    let mut alerts: Vec<hoi4_ui::market_panel::MarketAlertEntry> = goods
                        .iter()
                        .filter(|good| {
                            good.unmet_demand.max((good.demand - good.supply).max(0.0)) > 0.0
                        })
                        .take(5)
                        .map(|good| hoi4_ui::market_panel::MarketAlertEntry {
                            severity: if good.stockpile_coverage_days <= 3.0 {
                                hoi4_ui::market_panel::MarketAlertSeverity::Critical
                            } else {
                                hoi4_ui::market_panel::MarketAlertSeverity::Warning
                            },
                            good_id: good.id.clone(),
                            title: format!("{} 短缺", good.name),
                            description: format!(
                                "缺口 {:.1}/日，库存覆盖 {:.1} 天。",
                                good.unmet_demand.max((good.demand - good.supply).max(0.0)),
                                good.stockpile_coverage_days
                            ),
                        })
                        .collect();
                    if alerts.is_empty() {
                        alerts.push(hoi4_ui::market_panel::MarketAlertEntry {
                            severity: hoi4_ui::market_panel::MarketAlertSeverity::Info,
                            good_id: String::new(),
                            title: "市场平稳".to_owned(),
                            description: "当前没有明显商品短缺。".to_owned(),
                        });
                    }
                    let cash_rm = self
                        .world
                        .countries
                        .treasury
                        .treasuries
                        .get(player)
                        .map(|t| t.cash_rm)
                        .unwrap_or(0.0);
                    let bloc =
                        self.world
                            .countries
                            .market
                            .bloc_for_country(player_id)
                            .map(|bloc| {
                                let member_set: std::collections::HashSet<hoi4_state::CountryId> =
                                    bloc.members.iter().copied().collect();
                                let price_rm = |good_id: &str| -> f32 {
                                    self.v6_db
                                        .goods
                                        .iter()
                                        .find(|good| good.id == good_id)
                                        .map(|good| good.base_price_rm)
                                        .unwrap_or(1.0)
                                };
                                let route_value_gbp = |route: &hoi4_state::TradeRoute| -> f64 {
                                    let price =
                                        route.good_id.as_deref().map(price_rm).unwrap_or(1.0);
                                    route.throughput as f64 * price as f64
                                        / exchange_rate.max(0.01) as f64
                                };
                                let internal_trade_value_gbp: f64 = self
                                    .world
                                    .countries
                                    .trade
                                    .routes
                                    .iter()
                                    .filter(|route| {
                                        member_set.contains(&route.importer)
                                            && member_set.contains(&route.exporter)
                                    })
                                    .map(route_value_gbp)
                                    .sum();
                                let external_trade_value_gbp: f64 = self
                                    .world
                                    .countries
                                    .trade
                                    .routes
                                    .iter()
                                    .filter(|route| {
                                        member_set.contains(&route.importer)
                                            ^ member_set.contains(&route.exporter)
                                    })
                                    .map(route_value_gbp)
                                    .sum();
                                let tag_of = |country: hoi4_state::CountryId| -> String {
                                    self.world
                                        .countries
                                        .tags
                                        .get(country.0 as usize)
                                        .cloned()
                                        .unwrap_or_default()
                                };
                                let members = bloc
                                    .members
                                    .iter()
                                    .map(|&member| {
                                        let market = self
                                            .world
                                            .countries
                                            .market
                                            .markets
                                            .get(member.0 as usize);
                                        let contribution_supply_value_rm = market
                                            .map(|market| {
                                                market
                                                    .supply
                                                    .iter()
                                                    .map(|(good_id, amount)| {
                                                        *amount as f64 * price_rm(good_id) as f64
                                                    })
                                                    .sum()
                                            })
                                            .unwrap_or(0.0);
                                        let contribution_demand_value_rm = market
                                            .map(|market| {
                                                market
                                                    .demand
                                                    .iter()
                                                    .map(|(good_id, amount)| {
                                                        *amount as f64 * price_rm(good_id) as f64
                                                    })
                                                    .sum()
                                            })
                                            .unwrap_or(0.0);
                                        let mut strategic_goods: Vec<
                                            hoi4_ui::market_panel::GoodFlowSource,
                                        > = market
                                            .map(|market| {
                                                [
                                                    "oil",
                                                    "rubber",
                                                    "steel",
                                                    "grain",
                                                    "fuel",
                                                    "machinery",
                                                ]
                                                .iter()
                                                .filter_map(|good_id| {
                                                    let mut amount = market
                                                        .supply
                                                        .get(*good_id)
                                                        .copied()
                                                        .unwrap_or(0.0);
                                                    if let Some(autonomy) =
                                                        self.world.diplomacy.autonomy.get(&member)
                                                    {
                                                        if autonomy.master == bloc.leader {
                                                            amount *= autonomy
                                                                .level
                                                                .master_resource_share();
                                                        }
                                                    }
                                                    if amount <= 0.0 {
                                                        return None;
                                                    }
                                                    let name = self
                                                        .v6_db
                                                        .goods
                                                        .iter()
                                                        .find(|good| good.id == *good_id)
                                                        .map(|good| {
                                                            localized_content_name(
                                                                &good.id, &good.name,
                                                            )
                                                        })
                                                        .unwrap_or_else(|| (*good_id).to_owned());
                                                    Some(hoi4_ui::market_panel::GoodFlowSource {
                                                        name,
                                                        amount,
                                                    })
                                                })
                                                .collect()
                                            })
                                            .unwrap_or_default();
                                        strategic_goods.sort_by(|a, b| {
                                            b.amount
                                                .partial_cmp(&a.amount)
                                                .unwrap_or(std::cmp::Ordering::Equal)
                                        });
                                        let relation = if member == bloc.leader {
                                            "领导国".to_owned()
                                        } else if member == player_id {
                                            "本国".to_owned()
                                        } else if let Some(autonomy) =
                                            self.world.diplomacy.autonomy.get(&member)
                                        {
                                            if autonomy.master == bloc.leader {
                                                match autonomy.level {
                                        hoi4_state::AutonomyLevel::Dominion => "自治领".to_owned(),
                                        hoi4_state::AutonomyLevel::Puppet => "傀儡".to_owned(),
                                        hoi4_state::AutonomyLevel::IntegratedPuppet => {
                                            "整合傀儡".to_owned()
                                        }
                                        hoi4_state::AutonomyLevel::Satellite => "卫星国".to_owned(),
                                        hoi4_state::AutonomyLevel::FreedomAssociation => {
                                            "自由协约".to_owned()
                                        }
                                        hoi4_state::AutonomyLevel::Integrated => {
                                            "整合领地".to_owned()
                                        }
                                    }
                                            } else {
                                                "成员".to_owned()
                                            }
                                        } else {
                                            "成员".to_owned()
                                        };
                                        let market_access =
                                            if self.world.countries.trade.routes.iter().any(
                                                |route| {
                                                    (route.importer == member
                                                        || route.exporter == member)
                                                        && route.kind.uses_sea_lanes()
                                                        && route.is_blockaded
                                                },
                                            ) {
                                                0.5
                                            } else {
                                                1.0
                                            };
                                        hoi4_ui::market_panel::MarketBlocMemberEntry {
                                            tag: tag_of(member),
                                            relation,
                                            market_access,
                                            contribution_supply_value_rm,
                                            contribution_demand_value_rm,
                                            strategic_goods,
                                        }
                                    })
                                    .collect();
                                hoi4_ui::market_panel::MarketBlocPanelData {
                                    name: bloc.name.clone(),
                                    kind: match bloc.kind {
                                        hoi4_state::MarketBlocKind::ImperialPreference => {
                                            "帝国优惠".to_owned()
                                        }
                                        hoi4_state::MarketBlocKind::FactionMarket => {
                                            "阵营市场".to_owned()
                                        }
                                        hoi4_state::MarketBlocKind::ColonialEmpire => {
                                            "殖民帝国".to_owned()
                                        }
                                        hoi4_state::MarketBlocKind::BilateralSphere => {
                                            "双边势力范围".to_owned()
                                        }
                                    },
                                    leader_tag: tag_of(bloc.leader),
                                    members,
                                    internal_trade_value_gbp,
                                    external_trade_value_gbp,
                                }
                            });
                    let subjects: Vec<hoi4_ui::market_panel::MarketSubjectEntry> = self
                        .world
                        .diplomacy
                        .autonomy
                        .values()
                        .filter(|autonomy| autonomy.master == player_id)
                        .map(|autonomy| {
                            let tag = self
                                .world
                                .countries
                                .tags
                                .get(autonomy.subject.0 as usize)
                                .cloned()
                                .unwrap_or_default();
                            let subject_market = self
                                .world
                                .countries
                                .market
                                .markets
                                .get(autonomy.subject.0 as usize);
                            let mut resource_contribution: Vec<
                                hoi4_ui::market_panel::GoodFlowSource,
                            > = ["oil", "rubber", "steel", "grain", "fuel", "coal"]
                                .iter()
                                .filter_map(|good_id| {
                                    let amount = subject_market
                                        .and_then(|market| market.exports.get(*good_id).copied())
                                        .or_else(|| {
                                            subject_market.and_then(|market| {
                                                market.supply.get(*good_id).copied().map(|supply| {
                                                    supply * autonomy.level.master_resource_share()
                                                })
                                            })
                                        })
                                        .unwrap_or(0.0);
                                    if amount <= 0.0 {
                                        return None;
                                    }
                                    let name = self
                                        .v6_db
                                        .goods
                                        .iter()
                                        .find(|good| good.id == *good_id)
                                        .map(|good| localized_content_name(&good.id, &good.name))
                                        .unwrap_or_else(|| (*good_id).to_owned());
                                    Some(hoi4_ui::market_panel::GoodFlowSource { name, amount })
                                })
                                .collect();
                            resource_contribution.sort_by(|a, b| {
                                b.amount
                                    .partial_cmp(&a.amount)
                                    .unwrap_or(std::cmp::Ordering::Equal)
                            });
                            let fiscal_contribution_gbp: f64 = resource_contribution
                                .iter()
                                .map(|row| row.amount as f64 * 0.05)
                                .sum();
                            let autonomy_level = match autonomy.level {
                                hoi4_state::AutonomyLevel::Integrated => "整合领地",
                                hoi4_state::AutonomyLevel::IntegratedPuppet => "整合傀儡",
                                hoi4_state::AutonomyLevel::Puppet => "傀儡",
                                hoi4_state::AutonomyLevel::Dominion => "自治领",
                                hoi4_state::AutonomyLevel::Satellite => "卫星国",
                                hoi4_state::AutonomyLevel::FreedomAssociation => "自由协约",
                            }
                            .to_owned();
                            let subject_states = self.world.country_state_ids(autonomy.subject);
                            let avg_resistance = if subject_states.is_empty() {
                                0.0
                            } else {
                                subject_states
                                    .iter()
                                    .map(|state| self.world.states.resistance[state.0 as usize])
                                    .sum::<f32>()
                                    / subject_states.len() as f32
                            };
                            let avg_compliance = if subject_states.is_empty() {
                                0.0
                            } else {
                                subject_states
                                    .iter()
                                    .map(|state| self.world.states.compliance[state.0 as usize])
                                    .sum::<f32>()
                                    / subject_states.len() as f32
                            };
                            let market_access =
                                hoi4_logic::occupation::country_governance_market_access(
                                    &self.world,
                                    autonomy.subject,
                                );
                            let risk = if avg_resistance >= 50.0 {
                                format!(
                            "高抵抗 {:.0}%：资源抽取、税基和市场准入受损；当前市场准入 {:.0}%",
                            avg_resistance,
                            market_access * 100.0
                        )
                            } else if autonomy.level.master_resource_share() >= 0.5 {
                                format!(
                            "高抽取会压低自治度进展并提高殖民风险；顺从 {:.0}%，市场准入 {:.0}%",
                            avg_compliance,
                            market_access * 100.0
                        )
                            } else {
                                format!(
                                    "以优先贸易为主；顺从 {:.0}%，市场准入 {:.0}%",
                                    avg_compliance,
                                    market_access * 100.0
                                )
                            };
                            hoi4_ui::market_panel::MarketSubjectEntry {
                                tag,
                                autonomy_level,
                                master_resource_share: autonomy.level.master_resource_share(),
                                resource_contribution,
                                fiscal_contribution_gbp,
                                risk,
                            }
                        })
                        .collect();
                    let mut actions: Vec<hoi4_ui::market_panel::MarketActionEntry> = goods
                        .iter()
                        .filter(|good| {
                            good.unmet_demand.max((good.demand - good.supply).max(0.0)) > 0.0
                        })
                        .take(5)
                        .enumerate()
                        .map(|(idx, good)| {
                            let shortage =
                                good.unmet_demand.max((good.demand - good.supply).max(0.0));
                            let (title, description) = if good.imports > 0.0 && good.is_blockaded {
                                (
                                    format!("解除 {} 进口封锁", good.name),
                                    format!(
                                        "{} 仍有 {:.1}/日缺口，当前进口 {:.1}/日受封锁风险影响。",
                                        good.name, shortage, good.imports
                                    ),
                                )
                            } else if good.domestic_production <= 0.0 && good.imports <= 0.0 {
                                (
                                    format!("建立 {} 供应", good.name),
                                    format!(
                                "{} 没有国内产出或稳定进口，优先建设对应产业或寻找贸易伙伴。",
                                good.name
                            ),
                                )
                            } else if good.pop_consumption_demand > good.building_input_demand {
                                (
                                    format!("补足 POP 所需 {}", good.name),
                                    format!(
                                        "POP 每日需要 {:.1}，短缺会压低满意度。",
                                        good.pop_consumption_demand
                                    ),
                                )
                            } else {
                                (
                                    format!("扩张 {} 上游链", good.name),
                                    format!(
                                "建筑/军购需求较高，检查上游、生产方式或进口路线。缺口 {:.1}/日。",
                                shortage
                            ),
                                )
                            };
                            hoi4_ui::market_panel::MarketActionEntry {
                                title,
                                description,
                                related_good_id: Some(good.id.clone()),
                                priority: idx as u8,
                            }
                        })
                        .collect();
                    if actions.is_empty() && any_blockaded {
                        actions.push(hoi4_ui::market_panel::MarketActionEntry {
                            title: "检查受封锁贸易路线".to_owned(),
                            description: "当前存在被封锁路线，优先修复港口、护航或寻找陆路替代。"
                                .to_owned(),
                            related_good_id: None,
                            priority: 0,
                        });
                    }
                    Some(hoi4_ui::market_panel::MarketPanelData {
                        goods,
                        bloc,
                        exchange_rate,
                        cash_rm,
                        total_shortage_value_rm,
                        total_import_value_gbp,
                        total_export_value_gbp,
                        pop_needs_fulfillment,
                        military_supply_pressure,
                        subjects,
                        actions,
                        alerts,
                    })
                };
                if let Some(data) = built.as_ref() {
                    self.ui_panel_cache.market = Some((cache_key, data.clone()));
                }
                self.ui_panel_cache.record(
                    UiPanelCacheKind::Market,
                    cache_started.elapsed(),
                    false,
                );
                built
            }
        } else {
            None
        };
        let mut market_close = false;
        let finance_panel_data = if open_panel == Some(InGamePanel::Finance) {
            let cache_started = Instant::now();
            let cache_key = UiPanelCacheKey::new(
                player_country,
                self.world.date.days_since_epoch(),
                Self::finance_panel_signature(&self.world, player_country),
            );
            if let Some(data) = self
                .ui_panel_cache
                .finance
                .as_ref()
                .filter(|(key, _)| *key == cache_key)
                .map(|(_, data)| data.clone())
            {
                self.ui_panel_cache.record(
                    UiPanelCacheKind::Finance,
                    cache_started.elapsed(),
                    true,
                );
                Some(data)
            } else {
                let built = {
                    let player = player_country;
                    let treasury = self.world.countries.treasury.treasuries.get(player);
                    let exchange_rate = self
                        .world
                        .countries
                        .treasury
                        .exchange_rates
                        .get(player)
                        .map(|er| er.rm_per_gbp)
                        .unwrap_or(12.5);
                    if let Some(t) = treasury {
                        let credit_rating = match t.credit_rating {
                            hoi4_state::CreditRating::AAA => "AAA",
                            hoi4_state::CreditRating::AA => "AA",
                            hoi4_state::CreditRating::A => "A",
                            hoi4_state::CreditRating::BBB => "BBB",
                            hoi4_state::CreditRating::BB => "BB",
                            hoi4_state::CreditRating::B => "B",
                            hoi4_state::CreditRating::CCC => "CCC",
                            hoi4_state::CreditRating::D => "D",
                        };
                        let can_print_mefo = hoi4_logic::economy::finance_tick::can_print_mefo(
                            &self.world,
                            &self.v6_db,
                            player,
                        );
                        let is_forex_control =
                            hoi4_logic::economy::finance_tick::is_foreign_exchange_control(
                                &self.world,
                                &self.v6_db,
                                player,
                            );
                        Some(hoi4_ui::finance_panel::FinancePanelData {
                            cash_rm: t.cash_rm,
                            reserve_gbp: t.reserve_gbp,
                            gold_kg: t.gold_kg,
                            daily_income_rm: t.daily_income_rm,
                            daily_expense_rm: t.daily_expense_rm,
                            budget_breakdown: hoi4_ui::finance_panel::BudgetBreakdownData {
                                income_taxes_rm: t.daily_budget.income_taxes_rm,
                                income_state_profit_rm: t.daily_budget.income_state_profit_rm,
                                income_domestic_bonds_rm: t.daily_budget.income_domestic_bonds_rm,
                                income_other_rm: t.daily_budget.income_other_rm,
                                expense_state_payroll_rm: t.daily_budget.expense_state_payroll_rm,
                                expense_military_wages_rm: t.daily_budget.expense_military_wages_rm,
                                expense_military_procurement_rm: t
                                    .daily_budget
                                    .expense_military_procurement_rm,
                                expense_military_maintenance_rm: t
                                    .daily_budget
                                    .expense_military_maintenance_rm,
                                expense_construction_goods_rm: t
                                    .daily_budget
                                    .expense_construction_goods_rm,
                                expense_construction_wages_rm: t
                                    .daily_budget
                                    .expense_construction_wages_rm,
                                expense_welfare_rm: t.daily_budget.expense_welfare_rm,
                                expense_debt_interest_rm: t.daily_budget.expense_debt_interest_rm,
                                expense_foreign_currency_rm: t
                                    .daily_budget
                                    .expense_foreign_currency_rm,
                                expense_mefo_forced_payment_rm: t
                                    .daily_budget
                                    .expense_mefo_forced_payment_rm,
                                expense_research_rm: t.daily_budget.expense_research_rm,
                                expense_other_rm: t.daily_budget.expense_other_rm,
                            },
                            financing_breakdown: hoi4_ui::finance_panel::FinancingBreakdownData {
                                mefo_issued_rm: t.daily_financing.mefo_issued_rm,
                                mefo_interest_capitalized_rm: t
                                    .daily_financing
                                    .mefo_interest_capitalized_rm,
                                domestic_bond_issued_rm: t.daily_financing.domestic_bond_issued_rm,
                            },
                            operating_income_rm: t.operating_income_rm,
                            operating_expense_rm: t.operating_expense_rm,
                            original_deficit_rm: t.original_deficit_rm,
                            mefo_coverage_rm: t.mefo_coverage_rm,
                            post_financing_cash_change_rm: t.post_financing_cash_change_rm,
                            public_debt_gbp: t.public_debt_gbp,
                            public_debt_rm: t.public_debt_rm,
                            mefo_debt_rm: t.mefo_debt_rm,
                            mefo_military_budget_rm: t.mefo_military_budget_rm,
                            mefo_military_spent_rm: t.mefo_military_spent_rm,
                            credit_rating: credit_rating.to_owned(),
                            bond_interest_rate: t.bond_interest_rate,
                            gdp_rm: t.gdp_rm,
                            gdp_gbp: t.gdp_gbp,
                            domestic_gdp_rm: t.domestic_gdp_rm,
                            domestic_gdp_gbp: t.domestic_gdp_gbp,
                            colonial_gdp_rm: t.colonial_gdp_rm,
                            colonial_gdp_gbp: t.colonial_gdp_gbp,
                            colonial_extracted_value_rm: t.colonial_extracted_value_rm,
                            colonial_extracted_value_gbp: t.colonial_extracted_value_gbp,
                            exchange_rate_rm_per_gbp: exchange_rate,
                            can_print_mefo,
                            can_issue_foreign_bond: t.credit_rating.can_issue_foreign_bonds(),
                            is_foreign_exchange_control: is_forex_control,
                        })
                    } else {
                        None
                    }
                };
                if let Some(data) = built.as_ref() {
                    self.ui_panel_cache.finance = Some((cache_key, data.clone()));
                }
                self.ui_panel_cache.record(
                    UiPanelCacheKind::Finance,
                    cache_started.elapsed(),
                    false,
                );
                built
            }
        } else {
            None
        };
        let mut finance_close = false;
        let mut finance_cmds: Vec<hoi4_ui::finance_panel::FinanceCommand> = Vec::new();
        let trade_panel_data = if open_panel == Some(InGamePanel::Trade) {
            let player = player_country;
            let market = &self.world.countries.market.markets[player];
            let treasury = self.world.countries.treasury.treasuries.get(player);
            let current_trade_law = self.world.countries.law_store.law_sets[player].0
                [hoi4_state::LawCategory::Trade.index()]
            .current
            .clone();
            let trade_def = self
                .v6_db
                .trade_laws
                .iter()
                .find(|l| l.id == current_trade_law);
            let import_tariff_rate = trade_def.map(|t| t.import_tariff_rate).unwrap_or(0.0);
            let export_tariff_rate = trade_def.map(|t| t.export_tariff_rate).unwrap_or(0.0);
            let fx_control = trade_def
                .map(|t| t.foreign_exchange_control)
                .unwrap_or(false);
            let trade_law_name = trade_def
                .map(|t| localized_content_name(&t.id, &t.name))
                .unwrap_or_default();
            let any_blockaded = self.world.countries.trade.routes.iter().any(|r| {
                (r.importer == hoi4_state::CountryId(player as u16)
                    || r.exporter == hoi4_state::CountryId(player as u16))
                    && r.is_blockaded
            });
            let blockade_affected = if any_blockaded {
                Self::v6_blockade_affected_buildings(&self.world, &self.v6_db, player)
            } else {
                Vec::new()
            };
            let flows: Vec<hoi4_ui::trade_panel::TradeFlowEntry> = self
                .v6_db
                .goods
                .iter()
                .map(|g| {
                    let imports = market.imports.get(&g.id).copied().unwrap_or(0.0);
                    let exports = market.exports.get(&g.id).copied().unwrap_or(0.0);
                    hoi4_ui::trade_panel::TradeFlowEntry {
                        good_id: g.id.clone(),
                        good_name: Self::v6_good_name(&self.v6_db, &g.id),
                        imports,
                        exports,
                        failure_reason: self
                            .world
                            .countries
                            .trade
                            .import_failures
                            .get(&g.id)
                            .cloned(),
                        import_tariff_rate,
                        export_tariff_rate,
                    }
                })
                .collect();
            let routes: Vec<hoi4_ui::trade_panel::TradeRouteEntry> = self
                .world
                .countries
                .trade
                .routes
                .iter()
                .filter(|r| {
                    r.importer == hoi4_state::CountryId(player as u16)
                        || r.exporter == hoi4_state::CountryId(player as u16)
                })
                .map(|r| {
                    let good_id = r.good_id.clone().unwrap_or_default();
                    let good_name = self
                        .v6_db
                        .goods
                        .iter()
                        .find(|g| g.id == good_id)
                        .map(|g| localized_content_name(&g.id, &g.name))
                        .unwrap_or_else(|| {
                            if good_id.is_empty() {
                                "综合贸易".to_owned()
                            } else {
                                good_id.clone()
                            }
                        });
                    let kind_str = match r.kind {
                        hoi4_state::TradeRouteKind::Sea => "海运".to_owned(),
                        hoi4_state::TradeRouteKind::Land => "陆运".to_owned(),
                        hoi4_state::TradeRouteKind::Transit => "转运".to_owned(),
                        hoi4_state::TradeRouteKind::ImperialPreference => "帝国优惠".to_owned(),
                    };
                    let partner = if r.importer == hoi4_state::CountryId(player as u16) {
                        r.exporter
                    } else {
                        r.importer
                    };
                    hoi4_ui::trade_panel::TradeRouteEntry {
                        good_id,
                        good_name,
                        kind: kind_str,
                        throughput: r.throughput,
                        is_blockaded: r.is_blockaded,
                        historical: r.historical,
                        partner_tag: self
                            .world
                            .countries
                            .tags
                            .get(partner.0 as usize)
                            .cloned()
                            .unwrap_or_default(),
                        affected: if r.is_blockaded {
                            blockade_affected.clone()
                        } else {
                            Vec::new()
                        },
                    }
                })
                .collect();
            let reserve_gbp = treasury.map(|t| t.reserve_gbp).unwrap_or(0.0);
            let exchange_rate = self
                .world
                .countries
                .treasury
                .exchange_rates
                .get(player)
                .map(|er| er.rm_per_gbp)
                .unwrap_or(12.5);
            let trade_balance_gbp = treasury.map(|t| t.daily_trade_balance_gbp).unwrap_or(0.0);
            let tariff_income_rm = 0.0;
            Some(hoi4_ui::trade_panel::TradePanelData {
                flows,
                routes,
                reserve_gbp,
                exchange_rate,
                tariff_income_daily_rm: tariff_income_rm,
                trade_balance_daily_gbp: trade_balance_gbp,
                is_fx_control: fx_control,
                is_blockaded: any_blockaded,
                current_trade_law: current_trade_law.clone(),
                current_trade_law_name: trade_law_name,
                trade_capacity: {
                    let cid = hoi4_state::CountryId(player as u16);
                    let port_lvl: f32 = self
                        .world
                        .countries
                        .buildings_v6
                        .buildings
                        .iter()
                        .filter(|b| {
                            let si = b.state.0 as usize;
                            si < self.world.states.count
                                && self.world.states.owners[si] == cid
                                && b.building_def_id == "port"
                        })
                        .map(|b| b.level as f32)
                        .sum();
                    let rail_lvl: f32 = self
                        .world
                        .countries
                        .buildings_v6
                        .buildings
                        .iter()
                        .filter(|b| {
                            let si = b.state.0 as usize;
                            si < self.world.states.count
                                && self.world.states.owners[si] == cid
                                && b.building_def_id == "railway"
                        })
                        .map(|b| b.level as f32)
                        .sum();
                    if port_lvl > 0.0 || rail_lvl > 0.0 {
                        (port_lvl * 10.0 + rail_lvl * 5.0).max(1.0)
                    } else {
                        5.0
                    }
                },
                trade_capacity_used: self.world.countries.market.markets[player]
                    .imports
                    .values()
                    .sum(),
            })
        } else {
            None
        };
        let mut trade_close = false;
        let construction_v6_data = if open_panel == Some(InGamePanel::ConstructionV6) {
            let cache_started = Instant::now();
            let cache_key = UiPanelCacheKey::new(
                player_country,
                self.world.date.days_since_epoch(),
                Self::construction_panel_signature(
                    &self.world,
                    &self.econ,
                    self.auto_build_enabled,
                    self.last_auto_build_explanations.len(),
                    player_country,
                ),
            );
            if let Some(data) = self
                .ui_panel_cache
                .construction
                .as_ref()
                .filter(|(key, _)| *key == cache_key)
                .map(|(_, data)| data.clone())
            {
                self.ui_panel_cache.record(
                    UiPanelCacheKind::Construction,
                    cache_started.elapsed(),
                    true,
                );
                Some(data)
            } else {
                let built = {
                    let player = player_country;
                    let country_id = hoi4_state::CountryId(player as u16);
                    let state_ids: Vec<hoi4_state::StateId> = (0..self.world.states.count)
                        .filter(|&si| self.world.states.owners[si] == country_id)
                        .map(|si| hoi4_state::StateId(si as u16))
                        .collect();
                    let queue: Vec<hoi4_ui::construction_v6_panel::ConstructionQueueV6Entry> = self
                .econ
                .construction
                .get(player)
                .map(|q| {
                    q.items
                        .iter()
                        .map(|item| {
                            let building_name = self
                                .v6_db
                                .buildings
                                .iter()
                                .find(|bd| bd.id == item.building_key)
                                .map(|bd| localized_content_name(&bd.id, &bd.name))
                                .unwrap_or_else(|| item.building_key.clone());
                            let state_name = self
                                .world
                                .states
                                .names
                                .get(item.target_state.0 as usize)
                                .cloned()
                                .unwrap_or_default();
                            let current_level = Self::v6_current_building_level(
                                &self.world,
                                &item.building_key,
                                item.target_state,
                            );
                            let target_level = if item.target_level == 0 {
                                current_level.saturating_add(1)
                            } else {
                                item.target_level
                            };
                            hoi4_ui::construction_v6_panel::ConstructionQueueV6Entry {
                                building_name,
                                state_name,
                                current_level,
                                target_level,
                                progress: item.completion(),
                                funding_source_label: Self::construction_funding_source_label(
                                    item.funding_source,
                                )
                                .to_owned(),
                                owner_on_completion_label: Self::building_owner_label(
                                    item.owner_on_completion,
                                )
                                .to_owned(),
                                paid_funds_rm: item.paid_funds_rm,
                                budget_needed_rm: item.budget_needed_rm,
                                material_fulfillment: item.material_fulfillment(),
                                fund_ratio: if item.budget_needed_rm > 0.0 && item.funds_remaining_rm() > 0.0 {
                                    let daily_payment = (item.funds_remaining_rm() * 0.015).max(1.0);
                                    let available = match item.funding_source {
                                        hoi4_logic::economy::ConstructionFundingSource::Government
                                        | hoi4_logic::economy::ConstructionFundingSource::Mefo => {
                                            self.world.countries.treasury.treasuries[player].cash_rm.max(0.0)
                                        }
                                        hoi4_logic::economy::ConstructionFundingSource::PrivatePool => {
                                            self.world.countries.investment_balance_rm(
                                                hoi4_state::CountryId(player as u16),
                                                hoi4_state::InvestmentAccountKind::Private,
                                            )
                                        }
                                        hoi4_logic::economy::ConstructionFundingSource::CartelPool => {
                                            self.world.countries.investment_balance_rm(
                                                hoi4_state::CountryId(player as u16),
                                                hoi4_state::InvestmentAccountKind::Cartel,
                                            )
                                        }
                                        _ => self.world.countries.treasury.treasuries[player].cash_rm.max(0.0),
                                    };
                                    (available / daily_payment).min(1.0) as f32
                                } else {
                                    1.0
                                },
                            }
                        })
                        .collect()
                })
                .unwrap_or_default();
                    let mut buildable_catalog: Vec<
                        hoi4_ui::construction_v6_panel::BuildableBuildingEntry,
                    > = self
                        .v6_db
                        .buildings
                        .iter()
                        .filter(|bd| bd.buildable)
                        .map(|bd| {
                            let locked_reason =
                                Self::v6_building_lock_reason(&self.world, &self.v6_db, player, bd);
                            let state_limit_reason =
                                Self::v6_building_state_limit_reason(&self.world, bd);
                            hoi4_ui::construction_v6_panel::BuildableBuildingEntry {
                                building_def_id: bd.id.clone(),
                                building_name: localized_content_name(&bd.id, &bd.name),
                                group_name: if bd.group.is_empty() {
                                    Self::v6_building_group(bd.kind).to_string()
                                } else {
                                    bd.group.clone()
                                },
                                locked_reason,
                                state_limit_reason,
                            }
                        })
                        .collect();
                    buildable_catalog.sort_by(|a, b| {
                        a.group_name
                            .cmp(&b.group_name)
                            .then(a.building_name.cmp(&b.building_name))
                    });
                    #[derive(Default)]
                    struct BuildingAggregate {
                        total_level: u32,
                        weighted_employment: f32,
                        profit_rm_weekly: f64,
                        outputs: HashMap<String, f32>,
                        inputs: HashMap<String, f32>,
                        input_shortages: BTreeMap<String, f32>,
                        labor_gap: u32,
                        pm_counts: BTreeMap<String, u32>,
                        states: Vec<hoi4_ui::construction_v6_panel::BuildingStateV6Entry>,
                    }

                    let market = self.world.countries.market.markets.get(player);
                    let mut aggregates: BTreeMap<String, BuildingAggregate> = BTreeMap::new();
                    for (building_idx, b) in self
                        .world
                        .countries
                        .buildings_v6
                        .buildings
                        .iter()
                        .enumerate()
                        .filter(|(_, b)| state_ids.contains(&b.state) && b.level > 0)
                    {
                        let pms = hoi4_content::active_pms_for_building(b, &self.v6_db);
                        let employment_gap = Self::v6_employment_gap_for_building(&self.v6_db, b);
                        let labor_gap: u32 = employment_gap.iter().sum();
                        let blocking_law = if let Some((cat, law_id)) = &b.requires_law {
                            let current = &self.world.countries.law_store.law_sets[player].0
                                [cat.index()]
                            .current;
                            if current != law_id {
                                Some(format!("{:?}: {}", cat, law_id))
                            } else {
                                None
                            }
                        } else {
                            None
                        };
                        let is_law_blocked = blocking_law.is_some();
                        let state_name = self
                            .world
                            .states
                            .names
                            .get(b.state.0 as usize)
                            .cloned()
                            .unwrap_or_default();
                        let pm_groups =
                            Self::v6_pm_groups_for_building(&self.world, &self.v6_db, player, b);
                        let pm_candidates: Vec<(String, String)> = pm_groups
                            .iter()
                            .flat_map(|group| {
                                group
                                    .candidates
                                    .iter()
                                    .filter(|pm| pm.locked_reason.is_none())
                                    .map(|pm| (pm.pm_id.clone(), pm.pm_name.clone()))
                            })
                            .collect();
                        let display_profit_rm_daily = b.profit_rm;

                        let aggregate = aggregates.entry(b.building_def_id.clone()).or_default();
                        aggregate.total_level += b.level as u32;
                        aggregate.weighted_employment += b.production_rate * b.level as f32;
                        aggregate.profit_rm_weekly += display_profit_rm_daily * 7.0;
                        aggregate.labor_gap += labor_gap;
                        *aggregate
                            .pm_counts
                            .entry(
                                pms.iter()
                                    .map(|pm| {
                                        format!(
                                            "{}：{}",
                                            hoi4_content::production_method_group_name(pm),
                                            pm.name
                                        )
                                    })
                                    .collect::<Vec<_>>()
                                    .join("、"),
                            )
                            .or_default() += b.level as u32;

                        for pm in &pms {
                            for (i, good_id) in pm.output_good_ids.iter().enumerate() {
                                let amount = pm.output_good_amounts.get(i).copied().unwrap_or(0.0)
                                    * pm.throughput_modifier.max(0.0)
                                    * b.level as f32
                                    * b.production_rate;
                                *aggregate.outputs.entry(good_id.clone()).or_default() += amount;
                            }
                            for (i, good_id) in pm.input_good_ids.iter().enumerate() {
                                let amount = pm.input_good_amounts.get(i).copied().unwrap_or(0.0)
                                    * b.level as f32
                                    * b.production_rate;
                                *aggregate.inputs.entry(good_id.clone()).or_default() += amount;
                                if let Some(market) = market {
                                    let supply = market.supply.get(good_id).copied().unwrap_or(0.0);
                                    let demand = market.demand.get(good_id).copied().unwrap_or(0.0);
                                    if demand > supply && amount > 0.0 {
                                        let shortage_pct = ((demand - supply) / demand.max(1.0)
                                            * 100.0)
                                            .clamp(1.0, 100.0);
                                        aggregate
                                            .input_shortages
                                            .entry(good_id.clone())
                                            .or_insert(shortage_pct);
                                    }
                                }
                            }
                        }

                        aggregate.states.push(
                            hoi4_ui::construction_v6_panel::BuildingStateV6Entry {
                                building_idx,
                                state_name,
                                level: b.level,
                                employment_rate: b.production_rate,
                                profit_rm_weekly: display_profit_rm_daily * 7.0,
                                employment_gap,
                                is_law_blocked,
                                blocking_law,
                                active_pm: b.active_pm.clone(),
                                pm_candidates,
                                pm_groups,
                            },
                        );
                    }

                    let entries: Vec<hoi4_ui::construction_v6_panel::BuildingTypeV6Entry> =
                        aggregates
                            .into_iter()
                            .map(|(building_def_id, mut aggregate)| {
                                aggregate
                                    .states
                                    .sort_by(|a, b| a.state_name.cmp(&b.state_name));
                                let building_name =
                                    Self::v6_building_name(&self.v6_db, &building_def_id);
                                let mut warnings: Vec<String> = aggregate
                                    .input_shortages
                                    .iter()
                                    .map(|(good_id, pct)| {
                                        format!(
                                            "{}：{} {:.0}%",
                                            hoi4_ui::i18n::tr("input_shortage"),
                                            Self::v6_good_name(&self.v6_db, good_id),
                                            pct
                                        )
                                    })
                                    .collect();
                                if aggregate.labor_gap > 0 {
                                    warnings.push(format!(
                                        "{}：{}",
                                        hoi4_ui::i18n::tr("labor_shortage"),
                                        aggregate.labor_gap
                                    ));
                                }
                                let pm_summary = aggregate
                                    .pm_counts
                                    .iter()
                                    .map(|(pm, level)| format!("{} Lv {}", pm, level))
                                    .collect::<Vec<_>>()
                                    .join("、");
                                hoi4_ui::construction_v6_panel::BuildingTypeV6Entry {
                                    building_def_id,
                                    building_name,
                                    total_level: aggregate.total_level,
                                    employment_rate: if aggregate.total_level > 0 {
                                        aggregate.weighted_employment / aggregate.total_level as f32
                                    } else {
                                        0.0
                                    },
                                    profit_rm_weekly: aggregate.profit_rm_weekly,
                                    output_summary: Self::v6_flow_summary(
                                        &self.v6_db,
                                        &aggregate.outputs,
                                        "+",
                                    ),
                                    input_summary: Self::v6_flow_summary(
                                        &self.v6_db,
                                        &aggregate.inputs,
                                        "-",
                                    ),
                                    warnings,
                                    pm_summary,
                                    states: aggregate.states,
                                }
                            })
                            .collect();
                    let total_cp: f32 = hoi4_logic::economy::construction_tick::construction_cp_pool(
                        &self.world,
                        player,
                    ) as f32;
                    let treasury = &self.world.countries.treasury.treasuries[player];
                    let (working_age_pop, unemployed_pop) = self
                        .world
                        .countries
                        .pops
                        .groups
                        .iter()
                        .filter(|pop| {
                            state_ids.contains(&pop.state) && pop.class != PopClass::Soldier
                        })
                        .fold((0u64, 0u64), |(total, unemployed), pop| {
                            let size = pop.size as u64;
                            (
                                total + size,
                                unemployed + if pop.employed_at.is_none() { size } else { 0 },
                            )
                        });
                    let unemployment_rate = if working_age_pop > 0 {
                        unemployed_pop as f32 / working_age_pop as f32
                    } else {
                        0.0
                    };
                    let mefo_risk = if treasury.gdp_rm > 0.0 {
                        (treasury.mefo_debt_rm / treasury.gdp_rm) as f32
                    } else {
                        0.0
                    };
                    let investment_account = |kind: hoi4_state::InvestmentAccountKind| {
                        self.world
                            .countries
                            .investment_accounts
                            .iter()
                            .find(|account| {
                                account.country == country_id && account.account_kind == kind
                            })
                    };
                    let private_rm = investment_account(hoi4_state::InvestmentAccountKind::Private)
                        .map(|account| account.balance_rm)
                        .unwrap_or_else(|| {
                            self.world
                                .countries
                                .private_investment_pool_rm
                                .get(player)
                                .copied()
                                .unwrap_or(0.0)
                        });
                    let cartel_rm = investment_account(hoi4_state::InvestmentAccountKind::Cartel)
                        .map(|account| account.balance_rm)
                        .unwrap_or(0.0);
                    let state_development_bank_rm =
                        investment_account(hoi4_state::InvestmentAccountKind::StateDevelopmentBank)
                            .map(|account| account.balance_rm)
                            .unwrap_or(0.0);
                    let colonial_extraction_rm =
                        investment_account(hoi4_state::InvestmentAccountKind::ColonialExtraction)
                            .map(|account| account.balance_rm)
                            .unwrap_or(0.0);
                    let foreign_capital_rm =
                        investment_account(hoi4_state::InvestmentAccountKind::ForeignCapital)
                            .map(|account| account.balance_rm)
                            .unwrap_or(0.0);
                    let investment_income_rm: f64 = self
                        .world
                        .countries
                        .investment_accounts
                        .iter()
                        .filter(|account| account.country == country_id)
                        .map(|account| account.last_income_rm)
                        .sum();
                    let investment_spent_rm: f64 = self
                        .world
                        .countries
                        .investment_accounts
                        .iter()
                        .filter(|account| account.country == country_id)
                        .map(|account| account.last_spent_rm)
                        .sum();
                    Some(hoi4_ui::construction_v6_panel::ConstructionV6PanelData {
                        entries,
                        queue,
                        buildable_catalog,
                        active_construction_key: self.construction_mode.clone(),
                        available_cp: total_cp,
                        total_cp,
                        gdp_gbp: treasury.gdp_gbp,
                        gdp_growth_yoy: treasury.gdp_growth_yoy,
                        construction_spend_rm: treasury.daily_budget.expense_construction_goods_rm
                            + treasury.daily_budget.expense_construction_wages_rm,
                        unemployment_rate,
                        military_orders_rm: treasury.daily_budget.expense_military_procurement_rm,
                        mefo_risk,
                        auto_build_enabled: self.auto_build_enabled,
                        auto_build_explanations: self
                            .last_auto_build_explanations
                            .iter()
                            .map(|candidate| {
                                let building_name =
                                    Self::v6_building_name(&self.v6_db, &candidate.building_id);
                                let state_name = self
                                    .world
                                    .states
                                    .names
                                    .get(candidate.state.0 as usize)
                                    .cloned()
                                    .unwrap_or_else(|| format!("State {}", candidate.state.0));
                                hoi4_ui::construction_v6_panel::AutoBuildExplanationEntry {
                                    building_name,
                                    state_name,
                                    score: candidate.score,
                                    reasons: candidate.reasons.clone(),
                                }
                            })
                            .collect(),
                        investment_pool: hoi4_ui::construction_v6_panel::InvestmentPoolV6Data {
                            total_rm: private_rm
                                + cartel_rm
                                + state_development_bank_rm
                                + colonial_extraction_rm
                                + foreign_capital_rm,
                            private_rm,
                            cartel_rm,
                            state_development_bank_rm,
                            colonial_extraction_rm,
                            foreign_capital_rm,
                            income_rm: investment_income_rm,
                            spent_rm: investment_spent_rm,
                        },
                    })
                };
                if let Some(data) = built.as_ref() {
                    self.ui_panel_cache.construction = Some((cache_key, data.clone()));
                }
                self.ui_panel_cache.record(
                    UiPanelCacheKind::Construction,
                    cache_started.elapsed(),
                    false,
                );
                built
            }
        } else {
            None
        };
        let mut construction_v6_close = false;
        let mut construction_v6_cmds: Vec<hoi4_ui::construction_v6_panel::ConstructionV6Command> =
            Vec::new();
        let research_data = if open_panel == Some(InGamePanel::Research) {
            let player = player_country;
            let slots: Vec<hoi4_ui::research::SlotEntry> = self
                .research
                .slots
                .get(player)
                .map(|ss| {
                    ss.iter()
                        .filter_map(|s| match s {
                            hoi4_logic::research::ResearchSlot::Active {
                                tech_key,
                                progress,
                                total_cost,
                            } => Some(hoi4_ui::research::SlotEntry {
                                tech_key: tech_key.clone(),
                                progress: if *total_cost > 0.0 {
                                    *progress / *total_cost
                                } else {
                                    0.0
                                },
                            }),
                            _ => None,
                        })
                        .collect()
                })
                .unwrap_or_default();
            let slot_count = self
                .research
                .slots
                .get(player)
                .map(|s| s.len())
                .unwrap_or(0);
            let completed = &self.world.countries.completed_techs[player];
            let researching_keys: Vec<&str> = slots.iter().map(|s| s.tech_key.as_str()).collect();
            let techs: Vec<hoi4_ui::research::TechNode> = self
                .v6_db
                .technologies
                .iter()
                .map(|t| {
                    let key = t.id.clone();
                    hoi4_ui::research::TechNode {
                        key: key.clone(),
                        name: localized_content_name(&t.id, &t.name),
                        category: Self::v6_tech_category_key(t.category).to_owned(),
                        start_year: t.start_year,
                        completed: completed.contains(&key),
                        researching: researching_keys.contains(&key.as_str()),
                        progress: slots
                            .iter()
                            .find(|s| s.tech_key == key)
                            .map(|s| s.progress)
                            .unwrap_or(0.0),
                        prerequisites: t
                            .prereqs
                            .iter()
                            .map(|id| {
                                self.v6_db
                                    .technologies
                                    .iter()
                                    .find(|tech| tech.id == *id)
                                    .map(|tech| localized_content_name(&tech.id, &tech.name))
                                    .unwrap_or_else(|| id.clone())
                            })
                            .collect(),
                        unlock_summary: Self::v6_tech_unlock_summary(&self.v6_db, t),
                    }
                })
                .collect();
            Some(hoi4_ui::research::ResearchData {
                current_year: self.world.date.year,
                slots,
                slot_count,
                techs,
            })
        } else {
            None
        };
        let mut research_close = false;
        let mut research_cmds: Vec<hoi4_ui::research::ResearchCommand> = Vec::new();
        let diplomacy_data = if open_panel == Some(InGamePanel::Diplomacy) {
            let cache_started = Instant::now();
            let selected_tag = self.diplomacy_selected_country_tag.clone().or_else(|| {
                self.world
                    .countries
                    .tags
                    .iter()
                    .enumerate()
                    .find(|(idx, tag)| *idx != player_country && !tag.is_empty())
                    .map(|(_, tag)| tag.clone())
            });
            let cache_key = UiPanelCacheKey::new(
                player_country,
                self.world.date.days_since_epoch(),
                Self::diplomacy_panel_signature(&self.world, player_country),
            )
            .with_selected_tag(selected_tag.clone());
            if let Some(data) = self
                .ui_panel_cache
                .diplomacy
                .as_ref()
                .filter(|(key, _)| *key == cache_key)
                .map(|(_, data)| data.clone())
            {
                self.ui_panel_cache.record(
                    UiPanelCacheKind::Diplomacy,
                    cache_started.elapsed(),
                    true,
                );
                Some(data)
            } else {
                let built =
                    {
                        let player = player_country;
                        let player_cid = hoi4_state::CountryId(player as u16);
                        let player_tag = self
                            .world
                            .countries
                            .tags
                            .get(player)
                            .cloned()
                            .unwrap_or_default();
                        let player_faction = self
                            .world
                            .diplomacy
                            .factions
                            .iter()
                            .find(|f| f.contains(player_cid))
                            .map(|f| hoi4_ui::diplomacy::FactionEntry {
                                name: f.name.clone(),
                                leader_tag: self
                                    .world
                                    .countries
                                    .tags
                                    .get(f.leader.0 as usize)
                                    .cloned()
                                    .unwrap_or_default(),
                                member_tags: f
                                    .members
                                    .iter()
                                    .map(|m| {
                                        self.world
                                            .countries
                                            .tags
                                            .get(m.0 as usize)
                                            .cloned()
                                            .unwrap_or_default()
                                    })
                                    .collect(),
                            });
                        let countries: Vec<hoi4_ui::diplomacy::CountryEntry> = (0..self
                            .world
                            .countries
                            .count)
                            .filter(|&i| i != player && !self.world.countries.tags[i].is_empty())
                            .map(|i| {
                                let cid = hoi4_state::CountryId(i as u16);
                                let opinion = self.world.diplomacy.opinions.get(player_cid, cid);
                                let same_faction = player_faction.is_some()
                                    && self
                                        .world
                                        .diplomacy
                                        .factions
                                        .iter()
                                        .any(|f| f.contains(player_cid) && f.contains(cid));
                                let (leader_name, leader_portrait_key) =
                                    Self::head_of_state_display(
                                        &self.world,
                                        &self.historical_1936,
                                        cid,
                                    );
                                let has_wargoal = self
                                    .world
                                    .diplomacy
                                    .pending_wargoals
                                    .get(&player_cid)
                                    .map(|goals| {
                                        goals
                                            .iter()
                                            .any(|goal| goal.target == cid && goal.justified)
                                    })
                                    .unwrap_or(false);
                                let should_build_detail = selected_tag
                                    .as_deref()
                                    .map(|tag| tag == self.world.countries.tags[i].as_str())
                                    .unwrap_or(false);
                                let detail = if should_build_detail {
                                    self.build_country_info_data(cid, has_wargoal).map(|info| {
                                        hoi4_ui::diplomacy::CountryDiplomacyDetail {
                                            tag: info.tag,
                                            display_name: info.display_name,
                                            opinion: info.opinion,
                                            at_war: info.at_war,
                                            same_faction: info.same_faction,
                                            faction_name: info.faction_name,
                                            overlord_name: info.overlord_name,
                                            subject_names: info.subject_names,
                                            autonomy_level_name: info.autonomy_level_name,
                                            domestic_population: info.domestic_population,
                                            colonial_population: info.colonial_population,
                                            governed_population: info.governed_population,
                                            subject_population: info.subject_population,
                                            imperial_population: info.imperial_population,
                                            has_wargoal: info.has_wargoal,
                                            justifying_wargoal: info.justifying_wargoal,
                                            justify_progress: info.justify_progress,
                                            justify_days_remaining: info.justify_days_remaining,
                                            wargoals: info.wargoals,
                                            relation_factors: info.relation_factors,
                                            justify_action: info.justify_action,
                                            declare_war_action: info.declare_war_action,
                                            invite_to_faction_action: info.invite_to_faction_action,
                                            request_access_action: info.request_access_action,
                                        }
                                    })
                                } else {
                                    None
                                };
                                hoi4_ui::diplomacy::CountryEntry {
                                    tag: self.world.countries.tags[i].clone(),
                                    flag_gfx: format!(
                                        "GFX_flag_{}_{}",
                                        self.world.countries.tags[i],
                                        self.world
                                            .countries
                                            .ruling_party
                                            .get(i)
                                            .map(String::as_str)
                                            .unwrap_or_default()
                                    ),
                                    opinion,
                                    at_war: self.world.diplomacy.at_war_with(player_cid, cid),
                                    same_faction,
                                    autonomy_summary: diplomacy_autonomy_summary(&self.world, cid),
                                    leader_name,
                                    leader_portrait_key,
                                    detail,
                                }
                            })
                            .collect();
                        let all_factions: Vec<hoi4_ui::diplomacy::FactionEntry> = self
                            .world
                            .diplomacy
                            .factions
                            .iter()
                            .map(|f| hoi4_ui::diplomacy::FactionEntry {
                                name: localized_content_name(&f.name, &f.name),
                                leader_tag: self
                                    .world
                                    .countries
                                    .tags
                                    .get(f.leader.0 as usize)
                                    .cloned()
                                    .unwrap_or_default(),
                                member_tags: f
                                    .members
                                    .iter()
                                    .map(|m| {
                                        self.world
                                            .countries
                                            .tags
                                            .get(m.0 as usize)
                                            .cloned()
                                            .unwrap_or_default()
                                    })
                                    .collect(),
                            })
                            .collect();
                        let mut active_wars: Vec<hoi4_ui::diplomacy::PeaceWarEntry> = self
                            .world
                            .diplomacy
                            .wars
                            .values()
                            .map(|war| {
                                let tag_of = |country: hoi4_state::CountryId| {
                                    self.world
                                        .countries
                                        .tags
                                        .get(country.0 as usize)
                                        .cloned()
                                        .unwrap_or_default()
                                };
                                let mut attacker_tags: Vec<String> =
                                    war.attackers.iter().copied().map(tag_of).collect();
                                attacker_tags.sort();
                                let mut defender_tags: Vec<String> =
                                    war.defenders.iter().copied().map(tag_of).collect();
                                defender_tags.sort();
                                let winning_side_goals = if war.side_of(player_cid)
                                    == Some(hoi4_state::WarSide::Defender)
                                {
                                    &war.defender_wargoals
                                } else {
                                    &war.attacker_wargoals
                                };
                                let wargoals = winning_side_goals
                                    .iter()
                                    .map(|goal| hoi4_ui::diplomacy::PeaceWargoalEntry {
                                        claimant_tag: tag_of(goal.claimant),
                                        target_tag: tag_of(goal.target),
                                        kind: match goal.kind {
                                            hoi4_state::WargoalType::Annex => "Annex".to_owned(),
                                            hoi4_state::WargoalType::TakeState => {
                                                "Take State".to_owned()
                                            }
                                            hoi4_state::WargoalType::Liberate => {
                                                "Liberate".to_owned()
                                            }
                                            hoi4_state::WargoalType::Puppet => "Puppet".to_owned(),
                                            hoi4_state::WargoalType::ToppleGovernment => {
                                                "Topple Government".to_owned()
                                            }
                                            hoi4_state::WargoalType::NavalAccess => {
                                                "Naval Access".to_owned()
                                            }
                                        },
                                        target_state: goal.target_state.map(|state| state.0),
                                    })
                                    .collect();
                                hoi4_ui::diplomacy::PeaceWarEntry {
                                    id: war.id,
                                    primary_attacker_tag: tag_of(war.primary_attacker),
                                    primary_defender_tag: tag_of(war.primary_defender),
                                    attacker_tags,
                                    defender_tags,
                                    attacker_score: war.attacker_war_score,
                                    defender_score: war.defender_war_score,
                                    player_side: match war.side_of(player_cid) {
                                        Some(hoi4_state::WarSide::Attacker) => {
                                            Some(hoi4_ui::diplomacy::PeaceSide::Attacker)
                                        }
                                        Some(hoi4_state::WarSide::Defender) => {
                                            Some(hoi4_ui::diplomacy::PeaceSide::Defender)
                                        }
                                        None => None,
                                    },
                                    wargoals,
                                    attacker_peace_action: diplomacy_action_view(
                                        &self.world,
                                        player_cid,
                                        hoi4_logic::diplomacy::DiplomaticAction::ResolvePeace {
                                            war_id: war.id,
                                            winning_side: hoi4_state::WarSide::Attacker,
                                        },
                                    ),
                                    defender_peace_action: diplomacy_action_view(
                                        &self.world,
                                        player_cid,
                                        hoi4_logic::diplomacy::DiplomaticAction::ResolvePeace {
                                            war_id: war.id,
                                            winning_side: hoi4_state::WarSide::Defender,
                                        },
                                    ),
                                }
                            })
                            .collect();
                        active_wars.sort_by_key(|war| war.id);
                        let tag_of = |country: hoi4_state::CountryId| {
                            self.world
                                .countries
                                .tags
                                .get(country.0 as usize)
                                .cloned()
                                .unwrap_or_default()
                        };
                        let mut requests: Vec<hoi4_ui::diplomacy::DiplomaticRequestEntry> =
                            self.world
                                .diplomacy
                                .diplomatic_requests
                                .iter()
                                .filter(|request| {
                                    request.from == player_cid || request.to == player_cid
                                })
                                .map(|request| {
                                    hoi4_ui::diplomacy::DiplomaticRequestEntry {
                    from_tag: tag_of(request.from),
                    to_tag: tag_of(request.to),
                    kind: match &request.kind {
                        hoi4_state::DiplomaticRequestKind::InviteToFaction { .. } => {
                            "邀请加入阵营".to_owned()
                        }
                        hoi4_state::DiplomaticRequestKind::RequestMilitaryAccess => {
                            "请求军事通行".to_owned()
                        }
                        hoi4_state::DiplomaticRequestKind::OfferNonAggressionPact => {
                            "互不侵犯条约".to_owned()
                        }
                        hoi4_state::DiplomaticRequestKind::OfferPeace => "和平提议".to_owned(),
                    },
                    status: match request.status {
                        hoi4_state::DiplomaticRequestStatus::Pending => "Pending".to_owned(),
                        hoi4_state::DiplomaticRequestStatus::Accepted => "Accepted".to_owned(),
                        hoi4_state::DiplomaticRequestStatus::Rejected => "Rejected".to_owned(),
                        hoi4_state::DiplomaticRequestStatus::Expired => "Expired".to_owned(),
                        hoi4_state::DiplomaticRequestStatus::Withdrawn => "Withdrawn".to_owned(),
                    },
                }
                                })
                                .collect();
                        requests.sort_by(|a, b| a.status.cmp(&b.status).then(a.kind.cmp(&b.kind)));
                        Some(hoi4_ui::diplomacy::DiplomacyData {
                            player_tag,
                            player_faction,
                            all_factions,
                            countries,
                            active_wars,
                            requests,
                            world_tension: self.world.diplomacy.world_tension,
                        })
                    };
                if let Some(data) = built.as_ref() {
                    self.ui_panel_cache.diplomacy = Some((cache_key, data.clone()));
                }
                self.ui_panel_cache.record(
                    UiPanelCacheKind::Diplomacy,
                    cache_started.elapsed(),
                    false,
                );
                built
            }
        } else {
            None
        };
        let mut diplomacy_close = false;
        let mut diplomacy_cmds: Vec<hoi4_ui::diplomacy::DiplomacyCommand> = Vec::new();
        let combat_bubbles = self.collect_combat_bubbles();
        let diplomacy_sort = &mut self.diplomacy_sort_by_opinion;
        let military_data = if game_phase == GamePhase::Playing {
            let cache_started = Instant::now();
            let side_panel_open = open_panel == Some(InGamePanel::Military);
            let player = player_country;
            let player_cid = hoi4_state::CountryId(player as u16);
            let player_tag = self
                .world
                .countries
                .tags
                .get(player)
                .cloned()
                .unwrap_or_default();
            let divisions: Vec<hoi4_ui::military::DivisionEntry> =
                if open_panel == Some(InGamePanel::Military) || self.selected_army_id.is_some() {
                    (0..self.world.divisions.count)
                        .filter(|&i| self.world.divisions.owners[i] == player_cid)
                        .map(|i| hoi4_ui::military::DivisionEntry {
                            index: i,
                            name: self.world.divisions.names[i].clone(),
                            organisation: self.world.divisions.organisation[i],
                            max_organisation: self.world.divisions.max_organisation[i],
                            experience: self.world.divisions.experience[i],
                            strength: self.world.divisions.strength[i],
                            in_combat: self.world.divisions.in_combat[i],
                            province_id: self.world.divisions.locations[i].0,
                            equipment_ratio: self.world.divisions.strength[i],
                            army_id: self
                                .world
                                .player_armies
                                .iter()
                                .find(|a| a.members.contains(&i))
                                .map(|a| a.id.raw()),
                            army_name: self
                                .world
                                .player_armies
                                .iter()
                                .find(|a| a.members.contains(&i))
                                .map(|a| a.name.clone()),
                        })
                        .collect()
                } else {
                    Vec::new()
                };
            let templates: Vec<hoi4_ui::military::TemplateEntry> =
                if open_panel == Some(InGamePanel::Military) {
                    let stockpile = self.econ.stockpile.get(player).cloned().unwrap_or_default();
                    self.world
                        .data
                        .division_templates
                        .get(&player_tag)
                        .map(|ts| {
                            ts.iter()
                                .enumerate()
                                .map(|(i, t)| {
                                    let preview = hoi4_logic::military::templates::preview_template(
                                        t,
                                        self.world.data.as_ref(),
                                        Some(&stockpile),
                                    );
                                    hoi4_ui::military::TemplateEntry {
                                        index: i as u16,
                                        name: t.name.clone(),
                                        battalion_count: t.battalion_count(),
                                        combat_width: preview.combat_width,
                                        manpower: preview.manpower,
                                        training_days: preview.training_days,
                                        stockpile_satisfied_divisions: preview
                                            .stockpile_satisfied_divisions,
                                    }
                                })
                                .collect()
                        })
                        .unwrap_or_default()
                } else {
                    Vec::new()
                };
            let template_editor = if open_panel == Some(InGamePanel::Military) {
                let stockpile = self.econ.stockpile.get(player).cloned().unwrap_or_default();
                build_template_editor_data(
                    self.world.data.as_ref(),
                    &player_tag,
                    self.selected_template_idx,
                    Some(&stockpile),
                )
            } else {
                empty_template_editor_data()
            };
            let template_subunit_picker = if open_panel == Some(InGamePanel::Military) {
                build_template_subunit_picker_data(
                    self.world.data.as_ref(),
                    &player_tag,
                    self.template_picker_target,
                )
            } else {
                empty_template_subunit_picker_data()
            };
            let training_queue: Vec<hoi4_ui::military::TrainingQueueEntry> = if open_panel
                == Some(InGamePanel::Military)
            {
                self.econ
                    .training_queues
                    .get(player)
                    .map(|queue| {
                        queue
                            .iter()
                            .map(|item| {
                                let template_name = self
                                    .world
                                    .data
                                    .division_templates
                                    .get(&player_tag)
                                    .and_then(|ts| ts.get(item.template_id as usize))
                                    .map(|t| t.name.clone())
                                    .unwrap_or_else(|| format!("Template {}", item.template_id));
                                let required_manpower = self
                                    .world
                                    .data
                                    .division_templates
                                    .get(&player_tag)
                                    .and_then(|ts| ts.get(item.template_id as usize))
                                    .map(|t| {
                                        hoi4_logic::military::stats::DivisionStats::aggregate(
                                            t,
                                            self.world.data.as_ref(),
                                        )
                                        .manpower
                                    })
                                    .unwrap_or(0);
                                hoi4_ui::military::TrainingQueueEntry {
                                    id: item.id,
                                    template_name,
                                    count: item.count,
                                    progress: item.progress_days / item.required_days.max(1.0),
                                    manpower_allocated: item.manpower_allocated,
                                    required_manpower,
                                }
                            })
                            .collect()
                    })
                    .unwrap_or_default()
            } else {
                Vec::new()
            };
            let capital_prov = self
                .world
                .countries
                .capitals
                .get(player)
                .map(|s| {
                    self.world
                        .states
                        .provinces
                        .get(s.0 as usize)
                        .and_then(|ps| ps.first())
                        .map(|p| p.0)
                        .unwrap_or(0)
                })
                .unwrap_or(0);
            let armies: Vec<hoi4_ui::military::ArmyEntry> = self
                .world
                .player_armies
                .iter()
                .filter(|a| a.owner == player_cid)
                .map(|a| {
                    let general = a
                        .commander
                        .and_then(|gid| self.world.generals.iter().find(|g| g.id == gid));
                    let modifier = general.map(|g| {
                        hoi4_logic::military::general::GeneralModifier::from_general(
                            g,
                            a.members.len(),
                        )
                    });
                    hoi4_ui::military::ArmyEntry {
                        id: a.id.raw(),
                        name: a.name.clone(),
                        member_count: a.members.len(),
                        has_path: a.order.as_ref().map_or(false, |o| !o.path.is_empty()),
                        has_arrow: a.order.as_ref().map_or(false, |o| o.arrow.is_some()),
                        active: a.order.as_ref().map_or(false, |o| o.active),
                        executing: a.order.as_ref().map_or(false, |o| o.executing),
                        commander: a.commander.map(|id| id.raw()),
                        commander_name: general.map(|g| g.name.clone()),
                        command_limit: general.map(|g| g.command_limit),
                        command_efficiency: modifier.map(|m| m.command_efficiency).unwrap_or(1.0),
                        attack_bonus_pct: modifier
                            .map(|m| (m.attack_mult - 1.0) * 100.0)
                            .unwrap_or(0.0),
                        defense_bonus_pct: modifier
                            .map(|m| (m.defense_mult - 1.0) * 100.0)
                            .unwrap_or(0.0),
                        planning_bonus_pct: modifier
                            .map(|m| (m.plan_efficiency_mult - 1.0) * 100.0)
                            .unwrap_or(0.0),
                        org_recovery_bonus_pct: modifier
                            .map(|m| (m.org_recovery_mult - 1.0) * 100.0)
                            .unwrap_or(0.0),
                        supply_reduction_pct: modifier
                            .map(|m| (1.0 - m.supply_mult) * 100.0)
                            .unwrap_or(0.0),
                    }
                })
                .collect();
            let generals: Vec<hoi4_ui::military::GeneralEntry> = self
                .world
                .generals
                .iter()
                .filter(|g| g.owner == player_cid)
                .map(|g| {
                    let assigned_army_id = self
                        .world
                        .player_armies
                        .iter()
                        .find(|a| a.owner == player_cid && a.commander == Some(g.id))
                        .map(|a| a.id.raw());
                    hoi4_ui::military::GeneralEntry {
                        id: g.id.raw(),
                        name: g.name.clone(),
                        skill: g.skill,
                        attack: g.attack,
                        defense: g.defense,
                        planning: g.planning,
                        logistics: g.logistics,
                        command_limit: g.command_limit,
                        assigned_army_id,
                    }
                })
                .collect();
            let active_army_count = armies.iter().filter(|a| a.active).count();
            let painter_info = match self.frontline_painter.mode {
                PainterMode::Idle => hoi4_ui::military::PainterModeInfo::Idle,
                PainterMode::ArmyPainter(id) => {
                    hoi4_ui::military::PainterModeInfo::ArmyPainter(id.raw())
                }
                PainterMode::ArrowPainter(id, _) => {
                    hoi4_ui::military::PainterModeInfo::ArrowPainter(id.raw())
                }
            };
            let data = Some(hoi4_ui::military::MilitaryData {
                divisions,
                templates,
                template_editor_open: self.template_editor_open,
                template_editor,
                template_subunit_picker,
                training_queue,
                player_capital_province: capital_prov,
                armies,
                generals,
                frontline_overlay_visible: self.frontline_overlay_visible,
                selected_division_count: self.selected_divisions.len(),
                active_army_count,
                max_armies_per_country: hoi4_logic::military::frontline::MAX_FRONTLINES_PER_COUNTRY,
                selected_army_id: self.selected_army_id.map(|id| id.raw()),
                painter_mode: painter_info,
            });
            self.ui_panel_cache.record(
                UiPanelCacheKind::Military,
                cache_started.elapsed(),
                !side_panel_open,
            );
            data
        } else {
            None
        };
        let mut military_close = false;
        let mut military_cmds: Vec<hoi4_ui::military::MilitaryCommand> = Vec::new();
        let mut naval_close = false;
        let mut naval_cmds: Vec<hoi4_ui::naval::NavalCommand> = Vec::new();
        let mut air_close = false;
        let mut air_cmds: Vec<hoi4_ui::air::AirCommand> = Vec::new();

        let air_data = if open_panel == Some(InGamePanel::Air) {
            let player_cid = hoi4_state::CountryId(self.player_country as u16);
            let air_control = hoi4_logic::air::air_superiority::AirControl::recompute(
                &self.world,
                self.world.data.as_ref(),
            );
            let mut wings = Vec::new();
            for wi in 0..self.world.air_wings.count {
                if self.world.air_wings.owners[wi] != player_cid {
                    continue;
                }
                let region = self.world.air_wings.region_id[wi];
                let target = (self.world.air_wings.target_region[wi] != u32::MAX)
                    .then_some(self.world.air_wings.target_region[wi]);
                let effective_region = target.unwrap_or(region);
                wings.push(hoi4_ui::air::AirWingEntry {
                    id: wi as u32,
                    name: self.world.air_wings.names[wi].clone(),
                    aircraft_key: self.world.air_wings.aircraft_keys[wi].clone(),
                    base_state: self.world.air_wings.base_state[wi],
                    region_id: region,
                    target_region: target,
                    mission: air_mission_to_ui(self.world.air_wings.mission[wi]),
                    planes: self.world.air_wings.count_planes[wi],
                    max_planes: self.world.air_wings.max_planes[wi],
                    organisation: self.world.air_wings.organisation[wi],
                    max_organisation: self.world.air_wings.max_organisation[wi],
                    range_km: self.world.air_wings.range_km[wi],
                    air_control_pct: air_control.control(effective_region, player_cid) * 100.0,
                    mission_efficiency_pct: hoi4_logic::air::operations::mission_efficiency(
                        &self.world,
                        wi,
                    ) * 100.0,
                    transferring: self.world.air_wings.transfer_arrival_hour[wi] != 0,
                    reinforce_enabled: self.world.air_wings.reinforce_enabled[wi],
                });
            }
            let total_planes: u32 = wings.iter().map(|wing| wing.planes).sum();
            let active_wings = wings
                .iter()
                .filter(|wing| wing.mission != hoi4_ui::air::AirMissionUi::Idle)
                .count();
            let mut base_usage: std::collections::HashMap<u16, u32> =
                std::collections::HashMap::new();
            for wing in &wings {
                *base_usage.entry(wing.base_state).or_insert(0) += wing.planes;
            }
            let over_capacity_bases = base_usage
                .into_iter()
                .filter(|(state, planes)| {
                    let state = hoi4_state::StateId(*state);
                    *planes
                        > hoi4_logic::air::regions::airbase_capacity(&self.world, player_cid, state)
                })
                .count();
            Some(hoi4_ui::air::AirData {
                wings,
                aircraft_stockpile: self
                    .econ
                    .stockpile
                    .get(self.player_country)
                    .and_then(|s| s.get("aircraft"))
                    .copied()
                    .unwrap_or(0.0),
                total_planes,
                active_wings,
                over_capacity_bases,
                transfer_source_wing: self.air_transfer_source_wing,
                pending_transfer_wing: self.pending_air_transfer_wing,
            })
        } else {
            None
        };

        let naval_data = if open_panel == Some(InGamePanel::Naval) {
            let player_cid = hoi4_state::CountryId(self.player_country as u16);
            let mut fleets = Vec::new();
            for fi in 0..self.world.fleets.count {
                if self.world.fleets.owners[fi] != player_cid {
                    continue;
                }
                let mut hp = 0.0f32;
                let mut max_hp = 0.0f32;
                let mut damaged = 0usize;
                for &ship in &self.world.fleets.ships[fi] {
                    let si = ship.0 as usize;
                    if si >= self.world.ships.count {
                        continue;
                    }
                    hp += self.world.ships.hp[si].max(0.0);
                    max_hp += self.world.ships.max_hp[si].max(0.0);
                    if self.world.ships.hp[si] + 0.01 < self.world.ships.max_hp[si] {
                        damaged += 1;
                    }
                }
                let region = self.world.fleets.region_id[fi];
                let convoy_risk_pct = if region != u32::MAX {
                    hoi4_logic::naval::missions::convoy_route_risk(
                        &self.world,
                        self.world.data.as_ref(),
                        player_cid,
                        &[region],
                    )
                    .loss_rate
                        * 100.0
                } else {
                    0.0
                };
                fleets.push(hoi4_ui::naval::FleetEntry {
                    id: fi as u32,
                    name: self.world.fleets.names[fi].clone(),
                    region_id: region,
                    target_region_id: (self.world.fleets.target_region_id[fi] != u32::MAX)
                        .then_some(self.world.fleets.target_region_id[fi]),
                    mission: naval_mission_to_ui(self.world.fleets.mission[fi]),
                    ship_count: self.world.fleets.ships[fi].len(),
                    damaged_ships: damaged,
                    hp_ratio: if max_hp > 0.0 { hp / max_hp } else { 0.0 },
                    convoy_risk_pct,
                    repair_state: format!("{:?}", self.world.fleets.repair_state[fi]),
                });
            }
            let stockpile = self.econ.stockpile.get(self.player_country);
            Some(hoi4_ui::naval::NavalData {
                fleets,
                convoys: stockpile
                    .and_then(|s| s.get("convoy"))
                    .copied()
                    .unwrap_or(0.0),
                naval_vessels: stockpile
                    .and_then(|s| s.get("naval_vessel"))
                    .copied()
                    .unwrap_or(0.0),
                transfer_source_fleet: self.naval_transfer_source_fleet,
                pending_move_fleet: self.pending_naval_move_fleet,
            })
        } else {
            None
        };

        // J.4b / V6.G7: logistics panel data from V6 military buildings + force needs.
        let logistics_data = if open_panel == Some(InGamePanel::Logistics) {
            let player = player_country;
            if let Some(stockpile) = self.econ.stockpile.get_mut(player) {
                hoi4_logic::economy::stockpile::normalize_stockpile_keys(stockpile);
            }
            let stockpile = &self.econ.stockpile[player];
            let (daily_outputs, production_sources) =
                Self::logistics_v6_military_outputs(&self.world, &self.v6_db, player);
            let (force_need, replenishment_need) = Self::logistics_force_needs(&self.world, player);
            let training_shortfalls =
                Self::logistics_training_shortfalls(&self.world, &self.econ, player);
            let treasury = &self.world.countries.treasury.treasuries[player];
            let mut ids = Self::logistics_equipment_ids(&self.v6_db);
            for id in stockpile
                .keys()
                .chain(daily_outputs.keys())
                .chain(force_need.keys())
                .chain(training_shortfalls.keys())
            {
                if !ids.contains(id) {
                    ids.push(id.clone());
                }
            }
            let total_shortfall_value: f64 = ids
                .iter()
                .map(|id| {
                    let daily_prod = daily_outputs.get(id).copied().unwrap_or(0.0);
                    let daily_replenishment_need =
                        replenishment_need.get(id).copied().unwrap_or(0.0);
                    let total_equipment_need = force_need.get(id).copied().unwrap_or(0.0);
                    let daily_maintenance_need = total_equipment_need * 0.0005;
                    let daily_consumption = daily_replenishment_need + daily_maintenance_need;
                    let daily_deficit = (daily_consumption - daily_prod).max(0.0);
                    daily_deficit as f64 * Self::logistics_equipment_unit_cost_rm(id)
                })
                .sum();
            let mut entries: Vec<hoi4_ui::logistics_panel::LogisticsEntry> = ids
                .into_iter()
                .map(|id| {
                    let qty = stockpile.get(&id).copied().unwrap_or(0.0);
                    let daily_prod = daily_outputs.get(&id).copied().unwrap_or(0.0);
                    let daily_replenishment_need =
                        replenishment_need.get(&id).copied().unwrap_or(0.0);
                    let total_equipment_need = force_need.get(&id).copied().unwrap_or(0.0);
                    let training_shortfall = training_shortfalls.get(&id).copied().unwrap_or(0.0);
                    let display_qty = qty - training_shortfall;
                    let daily_training_need = training_shortfall / 30.0;
                    let daily_maintenance_need = total_equipment_need * 0.0005;
                    let daily_consumption =
                        daily_replenishment_need + daily_training_need + daily_maintenance_need;
                    let net_change = daily_prod - daily_consumption;
                    let deficit = (daily_consumption - daily_prod).max(0.0);
                    let days_until_empty = if net_change < 0.0 && display_qty > 0.0 {
                        Some(display_qty / -net_change)
                    } else if deficit > 0.0 && qty <= 0.0 {
                        None
                    } else {
                        None
                    };
                    let shortfall_value =
                        deficit as f64 * Self::logistics_equipment_unit_cost_rm(&id);
                    let procurement_rm = if total_shortfall_value > 0.0 {
                        treasury.daily_budget.expense_military_procurement_rm * shortfall_value
                            / total_shortfall_value
                    } else {
                        0.0
                    };
                    hoi4_ui::logistics_panel::LogisticsEntry {
                        name: Self::equipment_display_name(&id),
                        stockpile: display_qty,
                        daily_production: daily_prod,
                        daily_replenishment_need,
                        daily_training_need,
                        daily_maintenance_need,
                        daily_consumption,
                        net_change,
                        deficit,
                        days_until_empty,
                        procurement_rm,
                        production_sources: production_sources
                            .get(&id)
                            .cloned()
                            .unwrap_or_default(),
                    }
                })
                .collect();
            entries.sort_by(|a, b| {
                b.deficit
                    .partial_cmp(&a.deficit)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| a.name.cmp(&b.name))
            });
            let deficit_types = entries.iter().filter(|e| e.deficit > 0.0).count();
            let total_daily_production = entries.iter().map(|e| e.daily_production).sum();
            let total_daily_need = entries.iter().map(|e| e.daily_consumption).sum();
            // Resource quick view; use the live V6 market with static state
            // deposits as a pre-tick fallback so the panel is not all zeroes.
            let resources = Self::logistics_resource_entries(&self.world, player_country);
            Some(hoi4_ui::logistics_panel::LogisticsData {
                total_types: entries.len(),
                deficit_types,
                total_daily_production,
                total_daily_need,
                military_procurement_rm: treasury.daily_budget.expense_military_procurement_rm,
                military_maintenance_rm: treasury.daily_budget.expense_military_maintenance_rm,
                entries,
                resources,
            })
        } else {
            None
        };
        let mut logistics_close = false;

        // J.5b: Situation panel data
        let situation_panel_data = if open_panel == Some(InGamePanel::Situation) {
            let player_cid = hoi4_state::CountryId(player_country as u16);
            let situations: Vec<hoi4_ui::situation_panel::SituationEntry> = self
                .content
                .situation_state
                .active
                .iter()
                .map(|active| {
                    let def = self
                        .content
                        .situation_state
                        .defs
                        .iter()
                        .find(|d| d.id == active.def_id);
                    let sides: Vec<hoi4_ui::situation_panel::SituationSideEntry> = active
                        .progress
                        .iter()
                        .enumerate()
                        .map(|(i, &prog)| {
                            let side_def = def.and_then(|d| d.sides.get(i));
                            let name = side_def
                                .map(|s| localized_content_name(&s.id, &s.name))
                                .unwrap_or_default();
                            let color = side_def
                                .map(|s| {
                                    hoi4_ui::egui::Color32::from_rgb(
                                        s.color[0], s.color[1], s.color[2],
                                    )
                                })
                                .unwrap_or(hoi4_ui::egui::Color32::GRAY);
                            let supporters: Vec<String> = active
                                .supporters
                                .get(i)
                                .map(|sups| {
                                    sups.iter()
                                        .filter_map(|&c| {
                                            self.world.country_tag(c).map(|t| t.to_string())
                                        })
                                        .collect()
                                })
                                .unwrap_or_default();
                            hoi4_ui::situation_panel::SituationSideEntry {
                                name,
                                progress: prog,
                                color,
                                supporters,
                            }
                        })
                        .collect();
                    let interventions: Vec<hoi4_ui::situation_panel::InterventionEntry> = def
                        .map(|d| {
                            d.interventions
                                .iter()
                                .map(|interv| {
                                    let cd = active
                                        .cooldowns
                                        .get(&(player_cid.0, interv.id.clone()))
                                        .copied()
                                        .unwrap_or(0);
                                    let cost_desc = if interv.cost_pp > 0.0 {
                                        format!("政治点数 {:.0}", interv.cost_pp)
                                    } else if interv.cost_manpower > 0 {
                                        format!("人力 {}", interv.cost_manpower)
                                    } else if !interv.cost_equipment.is_empty() {
                                        format!(
                                            "{} {:.0}",
                                            interv.cost_equipment[0].0, interv.cost_equipment[0].1
                                        )
                                    } else {
                                        String::new()
                                    };
                                    let side_name = d
                                        .sides
                                        .get(interv.side_index)
                                        .map(|s| localized_content_name(&s.id, &s.name))
                                        .unwrap_or_default();
                                    hoi4_ui::situation_panel::InterventionEntry {
                                        id: interv.id.clone(),
                                        name: localized_content_name(&interv.id, &interv.name),
                                        cost_desc,
                                        expected_impact: intervention_expected_impact(
                                            interv.progress_boost,
                                            interv.army_xp,
                                            interv.air_xp,
                                            interv.cooldown_days,
                                        ),
                                        available: true,
                                        cooldown_days: cd,
                                        side_name,
                                    }
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    let intervention_log: Vec<hoi4_ui::situation_panel::InterventionLogEntry> =
                        active
                            .intervention_log
                            .iter()
                            .map(|log| hoi4_ui::situation_panel::InterventionLogEntry {
                                country_tag: log.country_tag.clone(),
                                intervention_name: log.intervention_name.clone(),
                                side_name: log.side_name.clone(),
                                progress_boost: log.progress_boost,
                                army_xp: log.army_xp,
                                air_xp: log.air_xp,
                            })
                            .collect();

                    // Military overview is meaningful for territorial-control situations.
                    let military_overview = if let Some(d) = def {
                        if let hoi4_content::ProgressSource::TerritorialControl {
                            side_country_tags,
                        } = &d.progress_source
                        {
                            let mut div_counts = vec![0u32; d.sides.len()];
                            let mut eq_stocks = vec![0.0f32; d.sides.len()];
                            let mut belligerents: Vec<Vec<String>> =
                                side_country_tags.iter().map(|tags| tags.clone()).collect();
                            // Division counts and equipment stockpiles.
                            for (idx, tags) in side_country_tags.iter().enumerate() {
                                for tag in tags {
                                    if let Some(cid) = self.world.country(tag) {
                                        let ci = cid.0 as usize;
                                        for di in 0..self.world.divisions.count {
                                            if self.world.divisions.owners[di] == cid {
                                                div_counts[idx] += 1;
                                            }
                                        }
                                        if ci < self.econ.stockpile.len() {
                                            eq_stocks[idx] += self.econ.stockpile[ci]
                                                .get("infantry_equipment")
                                                .copied()
                                                .unwrap_or(0.0);
                                        }
                                    }
                                }
                            }
                            // Theater control.
                            let mut theater_control = vec![0u32; d.sides.len()];
                            let theater_total = active.theater_states.len() as u32;
                            for &si_raw in &active.theater_states {
                                let si = si_raw as usize;
                                if si >= self.world.states.count {
                                    continue;
                                }
                                let ctrl = self.world.states.controllers[si];
                                if ctrl.is_none() {
                                    continue;
                                }
                                for (idx, tags) in side_country_tags.iter().enumerate() {
                                    if tags.iter().any(|t| self.world.country(t) == Some(ctrl)) {
                                        theater_control[idx] += 1;
                                        break;
                                    }
                                }
                            }
                            // Key provinces: each main belligerent capital state.
                            let mut key_provinces: Vec<(String, String)> = Vec::new();
                            for tags in side_country_tags {
                                for tag in tags {
                                    if let Some(cid) = self.world.country(&tag) {
                                        let ci = cid.0 as usize;
                                        if ci < self.world.countries.count {
                                            let cap_state = self.world.countries.capitals[ci];
                                            if !cap_state.is_none() {
                                                let cap_si = cap_state.0 as usize;
                                                if cap_si < self.world.states.count {
                                                    let ctrl =
                                                        self.world.states.controllers[cap_si];
                                                    let ctrl_tag = self
                                                        .world
                                                        .country_tag(ctrl)
                                                        .map(|s| s.to_string())
                                                        .unwrap_or_else(|| "-".into());
                                                    key_provinces.push((
                                                        format!("{} (capital)", tag),
                                                        ctrl_tag,
                                                    ));
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            // Remove countries that no longer exist from belligerent lists.
                            belligerents.iter_mut().for_each(|tags| {
                                tags.retain(|t| self.world.country(t).is_some());
                            });
                            Some(hoi4_ui::situation_panel::MilitaryOverview {
                                division_counts: div_counts,
                                equipment_stockpile: eq_stocks,
                                belligerent_tags: belligerents,
                                theater_control,
                                theater_total,
                                key_provinces,
                            })
                        } else {
                            None
                        }
                    } else {
                        None
                    };

                    hoi4_ui::situation_panel::SituationEntry {
                        id: active.def_id.clone(),
                        title: def
                            .map(|d| localized_content_name(&d.id, &d.title))
                            .unwrap_or_default(),
                        description: def
                            .map(|d| {
                                localized_content_name(&format!("desc.{}", d.id), &d.description)
                            })
                            .unwrap_or_default(),
                        sides,
                        interventions,
                        ended: active.ended,
                        winner: active.winner.and_then(|i| {
                            def.and_then(|d| d.sides.get(i))
                                .map(|s| localized_content_name(&s.id, &s.name))
                        }),
                        intervention_log,
                        military_overview,
                    }
                })
                .collect();
            Some(hoi4_ui::situation_panel::SituationPanelData { situations })
        } else {
            None
        };
        let mut situation_close = false;
        let mut situation_cmds: Vec<hoi4_ui::situation_panel::SituationCommand> = Vec::new();

        let focus_tree = &self.content.focus_tree;
        let focus_panel = &mut self.focus_panel;
        let player_idx = self.player_country;
        let player_cid = hoi4_state::CountryId(self.player_country as u16);
        let completed_focuses = self.world.countries.completed_focuses[player_idx].clone();
        let current_focus_ref = self.world.countries.current_focus[player_idx].as_deref();
        let current_focus_progress = self.world.countries.focus_progress[player_idx];
        // P0.3：计算 available focus id 集合
        let available_focus_ids: std::collections::HashSet<String> = focus_tree
            .focuses
            .iter()
            .filter(|f| {
                hoi4_content::eval::eval_trigger(
                    &f.available,
                    &self.world,
                    player_cid,
                    &self.content.global_flags,
                )
            })
            .map(|f| f.id.clone())
            .collect();
        let mut focus_cmd: Option<hoi4_ui::focus_tree_panel::FocusCommand> = None;
        let country_info_panel = &mut self.country_info_panel;
        let mut country_info_cmds: Vec<hoi4_ui::country_info_panel::CountryInfoCommand> =
            Vec::new();
        let player_in_faction = self
            .world
            .diplomacy
            .faction_of(hoi4_state::CountryId(self.player_country as u16))
            .is_some();
        let has_bottom_bar = self.frontline_painter.mode != PainterMode::Idle
            || self.selected_army_id.is_some()
            || !self
                .world
                .player_armies
                .iter()
                .filter(|a| a.owner == hoi4_state::CountryId(self.player_country as u16))
                .next()
                .is_none()
            || !self.selected_divisions.is_empty();
        self.province_info_card.bottom_bar_height = if has_bottom_bar { 118.0 } else { 0.0 };
        let province_info_card = &mut self.province_info_card;
        let province_info_data = &self.province_info_data;
        // CR-4.5: Counter right-click menu
        let counter_rclick_prov = self.counter_right_click_province;
        let mut counter_menu_cmd: Option<&'static str> = None;
        let counter_menu_pos = self.last_mouse;
        let event_scheduler = &self.content.event_scheduler;
        let event_world = &self.world;
        let event_flags = &self.content.global_flags;
        let event_country = hoi4_state::CountryId(self.player_country as u16);
        let mut event_cmd: Option<hoi4_ui::event_panel::EventCommand> = None;
        let mut surrender_notif_cmd: Option<
            hoi4_ui::surrender_notification::SurrenderNotificationCommand,
        > = None;
        // V5 G.3 / G.4 / G.5锛氬啀澶?disjoint 鍊熺敤 self 鐨勫瓙瀛楁銆俿ettings / save_browser /
        let settings_panel_open_cmd = open_panel == Some(InGamePanel::Settings);
        let saves_open_cmd = open_panel == Some(InGamePanel::Saves);
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
        let law_error_toast = match (&self.law_error_message, &self.last_law_error_toast) {
            (Some(current), Some(last)) if current == last => None,
            (Some(current), _) => Some(current.clone()),
            (None, _) => None,
        };
        self.last_law_error_toast = self.law_error_message.clone();
        let v9_notifications = &mut self.v9_notifications;
        let mut settings_close = false;
        let mut settings_cmds: Vec<hoi4_ui::settings::SettingsCommand> = Vec::new();
        let mut saves_close = false;
        let mut save_cmds: Vec<hoi4_ui::save_browser::SaveCommand> = Vec::new();
        let mut end_cmd: Option<hoi4_ui::end_screen::EndCommand> = None;
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
                        "elapsed: {elapsed_secs:.2} s 路 phase: {game_phase:?} 路 player_country: {player_country}"
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

            if let Some(ref data) = law_panel_data {
                let (close, cmds) = hoi4_ui::law_panel::LawPanel::show(ctx, data);
                if close {
                    law_close = true;
                }
                law_cmds = cmds;
                // P1.3：法律切换失败时显示中文提示
            }

            if let Some(ref data) = pop_panel_data {
                let (close, _cmds) = hoi4_ui::pop_panel::PopPanel::show(ctx, data);
                if close {
                    pop_panel_close = true;
                }
            }

            if let Some(ref data) = market_panel_data {
                let (close, _cmds) = hoi4_ui::market_panel::MarketPanel::show(ctx, data);
                if close { market_close = true; }
            }

            if let Some(ref data) = finance_panel_data {
                let (close, cmds) = hoi4_ui::finance_panel::FinancePanel::show(ctx, data);
                if close { finance_close = true; }
                finance_cmds = cmds;
            }

            if let Some(ref data) = trade_panel_data {
                let (close, _cmds) = hoi4_ui::trade_panel::TradePanel::show(ctx, data);
                if close { trade_close = true; }
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
                    diplomacy_sort,
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
                if hoi4_ui::logistics_panel::LogisticsPanel::show(ctx, data) {
                    logistics_close = true;
                }
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

            // P1.1：投降/和平通知弹窗（不暂停游戏）
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
        }

        if decisions_close {
            self.open_panel = None;
        }

        if law_close {
            self.open_panel = None;
            self.law_error_message = None;
        }

        if pop_panel_close {
            self.open_panel = None;
        }

        if market_close {
            self.open_panel = None;
        }

        if finance_close {
            self.open_panel = None;
        }

        if trade_close {
            self.open_panel = None;
        }

        if !finance_cmds.is_empty() {
            let player = self.player_country;
            for cmd in &finance_cmds {
                let content_cmd = match cmd {
                    hoi4_ui::finance_panel::FinanceCommand::IssueDomesticBond { amount_rm } => {
                        hoi4_content::FinanceCommand::IssueDomesticBond {
                            amount_rm: *amount_rm,
                        }
                    }
                    hoi4_ui::finance_panel::FinanceCommand::IssueForeignBond { amount_gbp } => {
                        hoi4_content::FinanceCommand::IssueForeignBond {
                            amount_gbp: *amount_gbp,
                        }
                    }
                    hoi4_ui::finance_panel::FinanceCommand::PrintMefo => {
                        hoi4_content::FinanceCommand::PrintMefo
                    }
                    hoi4_ui::finance_panel::FinanceCommand::SellGold { kg } => {
                        hoi4_content::FinanceCommand::SellGold { kg: *kg }
                    }
                    hoi4_ui::finance_panel::FinanceCommand::BuyForeignCurrency { gbp_amount } => {
                        hoi4_content::FinanceCommand::BuyForeignCurrency {
                            gbp_amount: *gbp_amount,
                        }
                    }
                };
                let _ = hoi4_content::execute_finance_command(
                    &mut self.world,
                    &self.v6_db,
                    player,
                    &content_cmd,
                );
            }
        }

        let mut construction_highlight_changed = false;
        if construction_v6_close {
            self.open_panel = None;
            if self.construction_mode.is_some() {
                self.construction_mode = None;
                self.construction_highlight_province_ids.clear();
                construction_highlight_changed = true;
            }
        }
        for cmd in construction_v6_cmds {
            use hoi4_ui::construction_v6_panel::ConstructionV6Command;
            match cmd {
                ConstructionV6Command::ToggleAutoBuild(enabled) => {
                    self.auto_build_enabled = enabled;
                    if enabled {
                        self.last_auto_build_month = None;
                    }
                }
                ConstructionV6Command::MoveUp(idx) => {
                    let player = self.player_country;
                    if let Some(q) = self.econ.construction.get_mut(player) {
                        if idx > 0 && idx < q.items.len() {
                            q.items.swap(idx, idx - 1);
                        }
                    }
                }
                ConstructionV6Command::MoveDown(idx) => {
                    let player = self.player_country;
                    if let Some(q) = self.econ.construction.get_mut(player) {
                        if idx + 1 < q.items.len() {
                            q.items.swap(idx, idx + 1);
                        }
                    }
                }
                ConstructionV6Command::Remove(idx) => {
                    let player = self.player_country;
                    self.econ
                        .cancel_construction_item(&mut self.world, player, idx, &self.v6_db);
                }
                ConstructionV6Command::StartConstructionMode { building_key } => {
                    self.construction_highlight_province_ids =
                        Self::construction_highlight_provinces(
                            &self.world,
                            &self.v6_db,
                            self.player_country,
                            &building_key,
                        );
                    self.construction_mode = Some(building_key);
                    construction_highlight_changed = true;
                }
                ConstructionV6Command::SwitchPM {
                    building_idx,
                    group,
                    pm_id,
                } => {
                    let pm_valid = self.v6_db.production_methods.iter().any(|pm| {
                        self.world
                            .countries
                            .buildings_v6
                            .buildings
                            .get(building_idx)
                            .map(|b| {
                                pm.id == pm_id
                                    && pm.building_id == b.building_def_id
                                    && hoi4_content::production_method_group(pm) == group
                                    && Self::v6_pm_lock_reason(
                                        &self.world,
                                        &self.v6_db,
                                        self.player_country,
                                        pm,
                                    )
                                    .is_none()
                            })
                            .unwrap_or(false)
                    });
                    if pm_valid && building_idx < self.world.countries.buildings_v6.buildings.len()
                    {
                        Self::v6_set_building_pm_group(
                            &mut self.world.countries.buildings_v6.buildings[building_idx],
                            &group,
                            &pm_id,
                        );
                    }
                }
                ConstructionV6Command::SwitchPMNationwide {
                    building_def_id,
                    group,
                    pm_id,
                } => {
                    let player = self.player_country;
                    let pm_valid = self.v6_db.production_methods.iter().any(|pm| {
                        pm.id == pm_id
                            && pm.building_id == building_def_id
                            && hoi4_content::production_method_group(pm) == group
                            && Self::v6_pm_lock_reason(&self.world, &self.v6_db, player, pm)
                                .is_none()
                    });
                    if pm_valid {
                        for building in self
                            .world
                            .countries
                            .buildings_v6
                            .buildings
                            .iter_mut()
                            .filter(|building| building.building_def_id == building_def_id)
                        {
                            Self::v6_set_building_pm_group(building, &group, &pm_id);
                        }
                    }
                }
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
                                    "[law] 切换法律失败: {:?} → {}: {}",
                                    category, target_law_id, e
                                );
                                self.law_error_message = Some(format!("法律切换失败: {}", e));
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
            }
        }

        if diplomacy_close {
            self.open_panel = None;
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

                        let mut occupation_lut = build_occupation_lut(&self.world);
                        occupation_lut.resize((s.lut_width * s.lut_height * 4) as usize, 0);
                        upload_lut(
                            &s.queue,
                            &s.occupation_lut_texture,
                            &occupation_lut,
                            s.lut_width,
                            s.lut_height,
                        );
                        s.window.request_redraw();
                    }
                }
            }
        }

        if military_close {
            self.open_panel = None;
        }
        if naval_close {
            self.open_panel = None;
        }
        if air_close {
            self.open_panel = None;
        }
        if logistics_close {
            self.open_panel = None;
        }
        if situation_close {
            self.open_panel = None;
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
                    // P0.3：start_focus 现在检查 available 条件
                    if !hoi4_content::start_focus(
                        &mut self.world,
                        player,
                        &self.content.focus_tree,
                        &id,
                        &self.content.global_flags,
                    ) {
                        println!(
                            "[focus] 无法开始国策 {}: available 条件不满足或前置未完成",
                            id
                        );
                    }
                }
                FocusCommand::Cancel => {
                    let i = self.player_country;
                    self.world.countries.current_focus[i] = None;
                    self.world.countries.focus_progress[i] = 0.0;
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
                        println!("[effect] 警告: {w}");
                    }
                    for e in report
                        .errors
                        .iter()
                        .chain(cascaded.effect_report.errors.iter())
                    {
                        println!("[effect] 错误: {e}");
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

        // P1.1：投降/和平通知确认处理（不暂停游戏）
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

        // 澶栧浗鍥藉淇℃伅闈㈡澘鍛戒护鍒嗗彂
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
                    // 鏃犳硶鍦ㄨ繖閲岀洿鎺?exit event_loop锛岃缃?speed=Paused 骞舵爣璁般€?                    // 瀹為檯閫€鍑哄湪 window_event 涓鐞?CloseRequested銆?                    std::process::exit(0);
                }
            }
        }

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
        // 3.6.1: Set terrain blend based on map mode.
        // Political mode: low blend (country colors dominate).
        // Terrain mode: high blend (terrain textures dominate).
        params.map_mode_terrain_blend = match self.map_mode {
            MapMode::Political => 0.20,
            MapMode::Terrain => 0.95,
            _ => 0.40,
        };
        // Diplomacy border lines in all modes except terrain.
        params.diplomacy_mode = match self.map_mode {
            MapMode::Terrain => 0,
            _ => 1,
        };

        let season_result = self
            .seasons
            .season_for_date(date.month as u32, date.day as u32);

        // Phase 3.12.8 ???update TreeFullPass season params per frame.
        // Drives tree color from `map/seasons.txt` 脳 `world.date` so trees
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
            let bp = passes::BorderParams {
                cam_distance_norm: cam_dist_norm,
                selection_intensity: sel_intensity,
                enabled_mask: 0x3F,
                selected_province_id: self.selected_province_id,
                debug_view: self.border_debug_view.as_shader_value(),
                screen_width: s.config.width as f32,
                screen_height: s.config.height as f32,
                _pad: 0,
            };
            s.border_pass.update_params(&s.queue, &bp);
        }

        // Phase 14 POI icons: static source instances are cached at startup.
        // Only re-upload when crossing the zoom LOD bucket; regenerating and
        // filtering every frame was visible CPU/GPU upload overhead while time ran.
        // Cull + LOD.
        let buckets = build_wrapped_instance_buckets(&s.chunk_grid, &self.camera);

        // Upload per-LOD instance buffers (no-op if empty).
        for lod in 0..3 {
            let bucket_sig = terrain_bucket_signature(&buckets[lod]);
            let bucket_count = buckets[lod].len() as u32;
            if s.terrain_bucket_signature[lod] == bucket_sig
                && s.terrain_bucket_counts[lod] == bucket_count
            {
                continue;
            }
            s.terrain_bucket_signature[lod] = bucket_sig;
            s.terrain_bucket_counts[lod] = bucket_count;
            if buckets[lod].is_empty() {
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
                bytemuck::cast_slice(&buckets[lod]),
            );
        }

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

        // Phase 3.12.3: 璁＄畻 shadow_view_proj 骞跺啓???ShadowPass uniform???        // 鍚屾椂鎶婂悓涓€涓煩闃靛啓???GlobalFrameUniform.shadow_view_proj 渚涘悗???receiver 鐢??
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
            gu.cam_look_at_dir = {
                let d = (self.camera.target - cam_eye).normalize_or_zero();
                [d.x, d.y, d.z]
            };
            gu.global_time = time;
            gu.fow_opacity_time_snow_max_speed = [
                params.season_snow_offset.max(0.0),
                time,
                season_result.season_blend,
                5.0,
            ];
            gu.day_night_hour_sun_dir = {
                let sd = params.sun_dir;
                let hour = (self.world.date.hour as f32) / 24.0;
                [hour, sd[0], sd[1], sd[2]]
            };
            gu.screen_size = [s.config.width as f32, s.config.height as f32];
            // Phase 3.12.3: shadow caster matrix
            gu.shadow_view_proj = shadow_vp.to_cols_array_2d();
            s.global_uniform_buf.write(&s.queue, &gu);
        }

        // Phase 3.12.3 ???directional shadow caster pass銆傚湪???3D pass 涔嬪墠璺戯紝
        // 鍥犱负鍚庣画 pass ???PCF 鏃堕渶瑕佽繖???depth map 宸插氨浣嶃€備粎 Playing 闃舵娓??
        let draw_3d_map = self.game_phase == GamePhase::Playing;
        let map_frame_plan = s.map_renderer.build_frame_plan(
            MapFrameContext {
                draw_3d_map: draw_3d_map && self.settings.enable_3d_terrain,
                map_mode: self.map_mode,
                date,
                selected_province_id: self.selected_province_id,
                hovered_province_id: self.hovered_province_id,
                zoom_factor,
                time_seconds: time,
                screen_size: [s.config.width as f32, s.config.height as f32],
                settings: MapRenderSettings::with_quality(map_layer_mask, self.map_quality_preset),
            },
            &s.pass_registry,
        );
        self.last_map_prepare_cpu_ms = prepare_started.elapsed().as_secs_f32() * 1000.0;
        let map_draw = &map_frame_plan.draw;
        let static_decals = map_frame_plan.static_decals;
        let semantic_overlays = map_frame_plan.semantic_overlays;
        let world_objects = map_frame_plan.world_objects;
        let terrain_ownership: TerrainMaterialOwnership = map_draw.terrain_material_ownership(
            map_layer_mask,
            s.water_pass.any_loaded,
            s.border_pass.any_loaded,
            static_decals,
        );
        let water_ownership = map_draw.water_material_ownership(s.water_pass.any_loaded);

        s.hoi3_counter_pass.update_opacity(
            &s.queue,
            world_objects.counters.opacity,
            s.config.width as f32,
            s.config.height as f32,
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
                world_w: self.camera.world_size.x,
                world_d: self.camera.world_size.y,
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

        s.water_pass.update_params(
            &s.queue,
            &passes::WaterParams {
                world_w: self.camera.world_size.x,
                world_d: self.camera.world_size.y,
                height_scale: HEIGHT_SCALE,
                selected_province_id: self.selected_province_id,
                debug_view: self.water_debug_view.as_shader_value(),
                final_water_owner: if water_ownership.final_color { 1 } else { 0 },
                ..passes::WaterParams::default()
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
        );
        let particle_quality = self.map_quality_preset.controls().particle_density;
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
                vignette_strength: params.vignette_strength,
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
                    self.camera.world_size.x,
                    self.camera.world_size.y,
                    HEIGHT_SCALE,
                    LAT_CORRECTION,
                ],
                season_params: [
                    season_result.season_column,
                    params.season_snow_offset,
                    0.0,
                    0.0,
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
                    semantic_overlays.occupation_stripes.opacity,
                    semantic_overlays.selected_province_pulse.opacity,
                    semantic_overlays.hover_highlight.opacity,
                    semantic_overlays.map_mode_overlay.opacity,
                ],
                atlas_idx_array: {
                    let src = self.world.map.terrain_catalog.atlas_idx_array();
                    std::array::from_fn::<[u32; 4], 4, _>(|r| {
                        std::array::from_fn::<u32, 4, _>(|c| src[r * 4 + c] as u32)
                    })
                },
            };
            s.terrain_pass.update_params(&s.queue, &pdx_params);
        }

        if map_draw.shadow_caster {
            let pass_started = Instant::now();
            let token = s
                .gpu_profiler
                .as_mut()
                .and_then(|profiler| profiler.begin_encoder_span(&mut enc, "shadow_caster"));
            let counts = [
                buckets[0].len() as u32,
                buckets[1].len() as u32,
                buckets[2].len() as u32,
            ];
            s.shadow_pass.render_caster(
                &mut enc,
                &s.instance_buffers,
                counts,
                vertex_count_for_lod,
            );
            if let (Some(profiler), Some(token)) = (s.gpu_profiler.as_mut(), token) {
                profiler.end_encoder_span(&mut enc, token);
            }
            s.pass_registry.record_cpu_ms(
                "shadow_caster",
                pass_started.elapsed().as_secs_f32() * 1000.0,
            );
            s.pass_registry.record_draw_calls(
                "shadow_caster",
                counts.iter().filter(|&&count| count > 0).count() as u32,
            );
        }

        // Phase 3.12.11: update sky pass inverse view-proj before render.
        {
            let inv_vp = self.camera.view_proj().inverse().to_cols_array_2d();
            s.sky_pass.update_params(&s.queue, &inv_vp);
        }

        // Phase 3.12.10: update particle simulation.
        if self.game_phase == GamePhase::Playing {
            let dt = 1.0 / 60.0;
            s.particle_pass.update(&s.device, &s.queue, dt);
        }

        // 4.1.bis.6 fix (2026-05-16): only draw the 3D world during Playing.
        // In MainMenu / CountrySelect the menu panels render on top of the
        // surface clear color; previously the world map drew underneath and
        // bled through transparent panels as a tilted parallelogram.
        {
            let pass_token = s
                .gpu_profiler
                .as_mut()
                .and_then(|profiler| profiler.begin_encoder_span(&mut enc, "3d_world"));
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("3d_to_hdr"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &s.hdr_target.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.05,
                            g: 0.07,
                            b: 0.15,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &s.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            if draw_3d_map {
                // Phase 3.12.11 ???sky cubemap background (must render first,
                // before terrain, so the sky paints behind all 3D geometry).
                // depth_write=false + depth_compare=LessEqual means the sky
                // only appears where no 3D object has been drawn.
                if map_draw.sky {
                    let pass_started = Instant::now();
                    let token = s
                        .gpu_profiler
                        .as_mut()
                        .and_then(|profiler| profiler.begin_render_span(&mut pass, "3d_sky"));
                    s.sky_pass.render(&mut pass);
                    if let (Some(profiler), Some(token)) = (s.gpu_profiler.as_mut(), token) {
                        profiler.end_render_span(&mut pass, token);
                    }
                    s.pass_registry
                        .record_cpu_ms("3d_sky", pass_started.elapsed().as_secs_f32() * 1000.0);
                    s.pass_registry.record_draw_calls("3d_sky", 1);
                }

                // Phase 3.12.4 ???prefer the vanilla pdxmap-equivalent pass.
                {
                    {
                        let counts = [
                            buckets[0].len() as u32,
                            buckets[1].len() as u32,
                            buckets[2].len() as u32,
                        ];
                        let vert_counts = [
                            vertex_count_for_lod(0),
                            vertex_count_for_lod(1),
                            vertex_count_for_lod(2),
                        ];
                        if map_draw.terrain {
                            let pass_started = Instant::now();
                            let token = s.gpu_profiler.as_mut().and_then(|profiler| {
                                profiler.begin_render_span(&mut pass, "3d_terrain")
                            });
                            s.terrain_pass.render(
                                &mut pass,
                                &s.instance_buffers,
                                &counts,
                                &vert_counts,
                            );
                            if let (Some(profiler), Some(token)) = (s.gpu_profiler.as_mut(), token)
                            {
                                profiler.end_render_span(&mut pass, token);
                            }
                            s.pass_registry.record_cpu_ms(
                                "3d_terrain",
                                pass_started.elapsed().as_secs_f32() * 1000.0,
                            );
                            s.pass_registry.record_draw_calls(
                                "3d_terrain",
                                counts.iter().filter(|&&count| count > 0).count() as u32,
                            );
                        }

                        // Phase 3.12.6 ???vanilla pdxwater pass on top of terrain.
                        // Reuses the same instance buffers + chunk grid; clamps Y to
                        // SEA_LEVEL 脳 HEIGHT_SCALE in VS, discards land pixels via
                        // heightmap sample in FS. depth_compare = LessEqual + no
                        // depth write means it overwrites terrain water pixels at
                        // identical Z without blocking trees/buildings.
                        if map_draw.water {
                            let pass_started = Instant::now();
                            let token = s.gpu_profiler.as_mut().and_then(|profiler| {
                                profiler.begin_render_span(&mut pass, "3d_water")
                            });
                            s.water_pass.render(
                                &mut pass,
                                &s.instance_buffers,
                                &counts,
                                &vert_counts,
                            );
                            if let (Some(profiler), Some(token)) = (s.gpu_profiler.as_mut(), token)
                            {
                                profiler.end_render_span(&mut pass, token);
                            }
                            s.pass_registry.record_cpu_ms(
                                "3d_water",
                                pass_started.elapsed().as_secs_f32() * 1000.0,
                            );
                            s.pass_registry.record_draw_calls(
                                "3d_water",
                                counts.iter().filter(|&&count| count > 0).count() as u32,
                            );
                        }

                        // RiverPass is intentionally disabled for now. Its animated
                        // terrain-following geometry z-fights against TerrainPass on
                        // shallow slopes and reads as flashing brown lines. Rivers are
                        // rendered as a stable blue overlay inside terrain.wgsl instead.

                        // Phase 3.12.9 (redesign) ???vanilla border pass using strip meshes.
                        // Renders thin quad-strips along actual province/country boundaries.
                        if map_draw.borders {
                            let pass_started = Instant::now();
                            let token = s.gpu_profiler.as_mut().and_then(|profiler| {
                                profiler.begin_render_span(&mut pass, "3d_border")
                            });
                            s.border_pass.render(&mut pass);
                            if let (Some(profiler), Some(token)) = (s.gpu_profiler.as_mut(), token)
                            {
                                profiler.end_render_span(&mut pass, token);
                            }
                            s.pass_registry.record_cpu_ms(
                                "3d_border",
                                pass_started.elapsed().as_secs_f32() * 1000.0,
                            );
                            s.pass_registry.record_draw_calls("3d_border", 6);
                        }

                        // Phase 16 ???arrows family (maparrow / traderoute / strait).
                        // Drawn after borders, before map symbols. Alpha-blended overlays.
                        if map_draw.trade_routes {
                            let pass_started = Instant::now();
                            let token = s.gpu_profiler.as_mut().and_then(|profiler| {
                                profiler.begin_render_span(&mut pass, "3d_traderoute")
                            });
                            s.traderoute_pass.render(&mut pass);
                            if let (Some(profiler), Some(token)) = (s.gpu_profiler.as_mut(), token)
                            {
                                profiler.end_render_span(&mut pass, token);
                            }
                            s.pass_registry.record_cpu_ms(
                                "3d_traderoute",
                                pass_started.elapsed().as_secs_f32() * 1000.0,
                            );
                            s.pass_registry.record_draw_calls("3d_traderoute", 1);
                        }
                        if map_draw.straits {
                            let pass_started = Instant::now();
                            let token = s.gpu_profiler.as_mut().and_then(|profiler| {
                                profiler.begin_render_span(&mut pass, "3d_strait")
                            });
                            s.strait_pass.render(&mut pass);
                            if let (Some(profiler), Some(token)) = (s.gpu_profiler.as_mut(), token)
                            {
                                profiler.end_render_span(&mut pass, token);
                            }
                            s.pass_registry.record_cpu_ms(
                                "3d_strait",
                                pass_started.elapsed().as_secs_f32() * 1000.0,
                            );
                            s.pass_registry.record_draw_calls("3d_strait", 1);
                        }

                        // Phase 6 static map decals. Railways are drawn before
                        // counters/objects so they read as map ink, not symbols.
                        if map_draw.railways && s.railways_vertex_count > 0 {
                            let pass_started = Instant::now();
                            let token = s.gpu_profiler.as_mut().and_then(|profiler| {
                                profiler.begin_render_span(&mut pass, "3d_railways")
                            });
                            pass.set_pipeline(&s.railways_pipeline);
                            pass.set_bind_group(0, &s.railways_bind_group, &[]);
                            pass.set_vertex_buffer(0, s.railways_buffer.slice(..));
                            pass.draw(0..s.railways_vertex_count, 0..1);
                            if let (Some(profiler), Some(token)) = (s.gpu_profiler.as_mut(), token)
                            {
                                profiler.end_render_span(&mut pass, token);
                            }
                            s.pass_registry.record_cpu_ms(
                                "3d_railways",
                                pass_started.elapsed().as_secs_f32() * 1000.0,
                            );
                            s.pass_registry.record_draw_calls("3d_railways", 1);
                        }

                        if map_draw.hoi3_counters {
                            let pass_started = Instant::now();
                            let token = s.gpu_profiler.as_mut().and_then(|profiler| {
                                profiler.begin_render_span(&mut pass, "hoi3_counter_v3")
                            });
                            s.hoi3_counter_pass.render(&mut pass);
                            if let (Some(profiler), Some(token)) = (s.gpu_profiler.as_mut(), token)
                            {
                                profiler.end_render_span(&mut pass, token);
                            }
                            s.pass_registry.record_cpu_ms(
                                "hoi3_counter_v3",
                                pass_started.elapsed().as_secs_f32() * 1000.0,
                            );
                            s.pass_registry.record_draw_calls("hoi3_counter_v3", 1);
                        }
                    }
                }

                // 5.6 - trees pass.
                // Phase 3.12.8: prefer the new TreeFullPass (season coloring + tint +
                // shadow receive) when available; fall back to the old trees_mesh
                // pipeline, and further to the billboard when meshes failed to load.
                if map_draw.trees {
                    let pass_started = Instant::now();
                    let token = s
                        .gpu_profiler
                        .as_mut()
                        .and_then(|profiler| profiler.begin_render_span(&mut pass, "3d_trees"));
                    let mut draw_calls = 0u32;
                    if let Some(tf) = s.tree_full_pass.as_mut() {
                        tf.render(&mut pass);
                        draw_calls = 1;
                    } else {
                        let any_mesh_loaded = s.trees_mesh_instance_counts.iter().any(|&c| c > 0);
                        let all_mesh_loaded = !s.trees_mesh_instance_counts.is_empty()
                            && s.trees_mesh_instance_counts.iter().all(|&c| c > 0);
                        if s.trees_count > 0 && !all_mesh_loaded {
                            pass.set_pipeline(&s.trees_pipeline);
                            pass.set_bind_group(0, &s.trees_bind_group, &[]);
                            pass.set_vertex_buffer(0, s.trees_buffer.slice(..));
                            pass.draw(0..6, 0..s.trees_count);
                            draw_calls = draw_calls.saturating_add(1);
                        }

                        // Phase 3.7.2 - 3D mesh trees (instanced).
                        if any_mesh_loaded {
                            pass.set_pipeline(&s.trees_mesh_pipeline);
                            for ty in 0..s.trees_mesh_index_counts.len() {
                                let inst_count = s.trees_mesh_instance_counts[ty];
                                let idx_count = s.trees_mesh_index_counts[ty];
                                if inst_count == 0 || idx_count == 0 {
                                    continue;
                                }
                                pass.set_bind_group(0, &s.trees_mesh_bind_groups[ty], &[]);
                                pass.set_vertex_buffer(
                                    0,
                                    s.trees_mesh_vertex_buffers[ty].slice(..),
                                );
                                pass.set_vertex_buffer(
                                    1,
                                    s.trees_mesh_instance_buffers[ty].slice(..),
                                );
                                pass.set_index_buffer(
                                    s.trees_mesh_index_buffers[ty].slice(..),
                                    wgpu::IndexFormat::Uint32,
                                );
                                pass.draw_indexed(0..idx_count, 0, 0..inst_count);
                                draw_calls = draw_calls.saturating_add(1);
                            }
                        }
                    }
                    if let (Some(profiler), Some(token)) = (s.gpu_profiler.as_mut(), token) {
                        profiler.end_render_span(&mut pass, token);
                    }
                    s.pass_registry
                        .record_cpu_ms("3d_trees", pass_started.elapsed().as_secs_f32() * 1000.0);
                    s.pass_registry
                        .record_draw_calls("3d_trees", draw_calls.max(1));
                }

                // Phase 3.12.5 ???vanilla 3D building mesh (PdxMesh pipeline).
                if map_draw.buildings && s.pdxmesh_pass.any_loaded {
                    let pass_started = Instant::now();
                    let token = s
                        .gpu_profiler
                        .as_mut()
                        .and_then(|profiler| profiler.begin_render_span(&mut pass, "3d_buildings"));
                    s.pdxmesh_pass.render(&mut pass);
                    if let (Some(profiler), Some(token)) = (s.gpu_profiler.as_mut(), token) {
                        profiler.end_render_span(&mut pass, token);
                    }
                    s.pass_registry.record_cpu_ms(
                        "3d_buildings",
                        pass_started.elapsed().as_secs_f32() * 1000.0,
                    );
                    s.pass_registry.record_draw_calls("3d_buildings", 1);
                } else if map_draw.buildings && s.buildings_count > 0 {
                    let pass_started = Instant::now();
                    let token = s
                        .gpu_profiler
                        .as_mut()
                        .and_then(|profiler| profiler.begin_render_span(&mut pass, "3d_buildings"));
                    pass.set_pipeline(&s.buildings_pipeline);
                    pass.set_bind_group(0, &s.buildings_bind_group, &[]);
                    pass.set_vertex_buffer(0, s.buildings_buffer.slice(..));
                    pass.draw(0..6, 0..s.buildings_count);
                    if let (Some(profiler), Some(token)) = (s.gpu_profiler.as_mut(), token) {
                        profiler.end_render_span(&mut pass, token);
                    }
                    s.pass_registry.record_cpu_ms(
                        "3d_buildings",
                        pass_started.elapsed().as_secs_f32() * 1000.0,
                    );
                    s.pass_registry.record_draw_calls("3d_buildings", 1);
                }

                // Phase 14 ???POI icon pass.
                if map_draw.poi_icons {
                    if let Some(poi) = s.poi_icon_pass.as_ref() {
                        let pass_started = Instant::now();
                        let token = s.gpu_profiler.as_mut().and_then(|profiler| {
                            profiler.begin_render_span(&mut pass, "3d_poi_icons")
                        });
                        poi.render(&mut pass);
                        if let (Some(profiler), Some(token)) = (s.gpu_profiler.as_mut(), token) {
                            profiler.end_render_span(&mut pass, token);
                        }
                        s.pass_registry.record_cpu_ms(
                            "3d_poi_icons",
                            pass_started.elapsed().as_secs_f32() * 1000.0,
                        );
                        s.pass_registry.record_draw_calls("3d_poi_icons", 1);
                    }
                }

                // Phase 3.12.10 ???vanilla-equivalent 3D country-name labels.
                if map_draw.map_names {
                    if let Some(mnp) = s.mapname_pass.as_ref() {
                        let pass_started = Instant::now();
                        let token = s.gpu_profiler.as_mut().and_then(|profiler| {
                            profiler.begin_render_span(&mut pass, "3d_mapname")
                        });
                        mnp.render(&mut pass, world_objects.country_names.scale);
                        if let (Some(profiler), Some(token)) = (s.gpu_profiler.as_mut(), token) {
                            profiler.end_render_span(&mut pass, token);
                        }
                        s.pass_registry.record_cpu_ms(
                            "3d_mapname",
                            pass_started.elapsed().as_secs_f32() * 1000.0,
                        );
                        s.pass_registry.record_draw_calls("3d_mapname", 1);
                    }
                }

                // Phase 3.12.13 ???province-name labels (zoom-gated).
                // Only visible at medium/close zoom (zoom_factor >= 0.4).
                // Gated by show_province_names toggle (F7) ???disabled by default
                // until the visual bugs are resolved.
                if map_draw.province_names && self.show_province_names {
                    if let Some(pnp) = s.province_name_pass.as_ref() {
                        let pass_started = Instant::now();
                        let token = s.gpu_profiler.as_mut().and_then(|profiler| {
                            profiler.begin_render_span(&mut pass, "3d_province_name")
                        });
                        pnp.render(&mut pass, zoom_factor);
                        if let (Some(profiler), Some(token)) = (s.gpu_profiler.as_mut(), token) {
                            profiler.end_render_span(&mut pass, token);
                        }
                        s.pass_registry.record_cpu_ms(
                            "3d_province_name",
                            pass_started.elapsed().as_secs_f32() * 1000.0,
                        );
                        s.pass_registry.record_draw_calls("3d_province_name", 1);
                    }
                }

                // Phase 16 ???arrows + frontlines (drawn after all opaque geometry, alpha-blended overlays).
                if map_draw.map_arrows {
                    let pass_started = Instant::now();
                    let token = s
                        .gpu_profiler
                        .as_mut()
                        .and_then(|profiler| profiler.begin_render_span(&mut pass, "3d_maparrow"));
                    s.maparrow_pass.render(&mut pass);
                    if let (Some(profiler), Some(token)) = (s.gpu_profiler.as_mut(), token) {
                        profiler.end_render_span(&mut pass, token);
                    }
                    s.pass_registry.record_cpu_ms(
                        "3d_maparrow",
                        pass_started.elapsed().as_secs_f32() * 1000.0,
                    );
                    s.pass_registry.record_draw_calls("3d_maparrow", 1);
                }
                if map_draw.frontlines && s.frontlines_vertex_count > 0 {
                    let pass_started = Instant::now();
                    let token = s.gpu_profiler.as_mut().and_then(|profiler| {
                        profiler.begin_render_span(&mut pass, "3d_frontlines")
                    });
                    pass.set_pipeline(&s.frontlines_pipeline);
                    pass.set_bind_group(0, &s.frontlines_bind_group, &[]);
                    pass.set_vertex_buffer(0, s.frontlines_buffer.slice(..));
                    pass.draw(0..s.frontlines_vertex_count, 0..1);
                    if let (Some(profiler), Some(token)) = (s.gpu_profiler.as_mut(), token) {
                        profiler.end_render_span(&mut pass, token);
                    }
                    s.pass_registry.record_cpu_ms(
                        "3d_frontlines",
                        pass_started.elapsed().as_secs_f32() * 1000.0,
                    );
                    s.pass_registry.record_draw_calls("3d_frontlines", 1);
                }

                // Phase 3.12.10 ???particle pass (combat smoke / factory chimneys).
                // Alpha-blended, no depth write, drawn after all opaque + labels.
                if map_draw.particles {
                    let pass_started = Instant::now();
                    let token = s
                        .gpu_profiler
                        .as_mut()
                        .and_then(|profiler| profiler.begin_render_span(&mut pass, "3d_particles"));
                    s.particle_pass.render(&mut pass);
                    if let (Some(profiler), Some(token)) = (s.gpu_profiler.as_mut(), token) {
                        profiler.end_render_span(&mut pass, token);
                    }
                    s.pass_registry.record_cpu_ms(
                        "3d_particles",
                        pass_started.elapsed().as_secs_f32() * 1000.0,
                    );
                    s.pass_registry.record_draw_calls("3d_particles", 1);
                }
            } // end if draw_3d_map
            drop(pass);
            if let (Some(profiler), Some(token)) = (s.gpu_profiler.as_mut(), pass_token) {
                profiler.end_encoder_span(&mut enc, token);
            }
        }

        // 鈹€鈹€鈹€ Phase 3.12.1 / 3.12.2: HDR ???swap chain 妗ユ帴 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€
        // Playing 闃舵璧板畬鏁村悗澶勭悊閾撅紱鑿滃崟 / 鍥藉閫夋嫨闃舵璧扮畝???blit锛堥伩???        // 鑷姩鏇濆厜鎶婄┖鍦烘櫙杩囧害鎻愪寒锛??
        s.post_process.debug_view = self.postprocess_debug_view;
        let use_full_chain = map_draw.postprocess
            && s.post_process.mode == PostProcessMode::Full
            && self.map_quality_preset.controls().postprocess_chain;
        let postprocess_started = Instant::now();
        if use_full_chain {
            let token = s
                .gpu_profiler
                .as_mut()
                .and_then(|profiler| profiler.begin_encoder_span(&mut enc, "postprocess"));
            s.post_process.prepare(&s.queue);
            s.post_process.render(&mut enc, output_view);
            if let (Some(profiler), Some(token)) = (s.gpu_profiler.as_mut(), token) {
                profiler.end_encoder_span(&mut enc, token);
            }
            s.pass_registry.record_draw_calls("postprocess", 9);
        } else {
            let token = s
                .gpu_profiler
                .as_mut()
                .and_then(|profiler| profiler.begin_encoder_span(&mut enc, "postprocess"));
            s.simple_blit.render(&mut enc, output_view);
            if let (Some(profiler), Some(token)) = (s.gpu_profiler.as_mut(), token) {
                profiler.end_encoder_span(&mut enc, token);
            }
            s.pass_registry.record_draw_calls("postprocess", 1);
        }
        s.pass_registry.record_cpu_ms(
            "postprocess",
            postprocess_started.elapsed().as_secs_f32() * 1000.0,
        );

        s.shadow_pass.render_debug(&mut enc, output_view);

        // F4 璋冭瘯瑕嗙洊锛氭妸 pass 鍒楄〃 / 妯″紡鍒锋柊???DebugOverlay锛孶I 鏂囨湰???text_pass 娑堣垂??
        let chain_label = if use_full_chain {
            "post_process_full"
        } else {
            "simple_blit"
        };
        let postprocess_summary = s.post_process.calibration.summary();
        s.debug_render_overlay.refresh(
            &s.pass_registry,
            s.hdr_target.width,
            s.hdr_target.height,
            &format!(
                "{} quality={} post_debug={} {} terrain_debug={} water_debug={} border_debug={} overlay_budget={:.2}/{:.2}/{:.2}",
                chain_label,
                self.map_quality_preset.as_str(),
                s.post_process.debug_view.name(),
                postprocess_summary,
                self.terrain_debug_view.name(),
                self.water_debug_view.name(),
                self.border_debug_view.name(),
                semantic_overlays.budget.passive,
                semantic_overlays.budget.active,
                semantic_overlays.budget.map_mode
            ),
        );
        let gpu_status = s
            .gpu_profiler
            .as_ref()
            .map(|profiler| profiler.status())
            .unwrap_or(GpuProfilerStatus::UNSUPPORTED);
        let texture_memory_bytes =
            estimate_frame_texture_memory_bytes(s.config.width, s.config.height, use_full_chain);
        let phase10_lines = phase10_overlay_lines(
            Phase10OverlayInput {
                preset: self.map_quality_preset,
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
        let player = self.player_country;
        let pp = self
            .world
            .countries
            .political_power
            .get(player)
            .copied()
            .unwrap_or(0.0);
        let manpower = recruitable_manpower(
            &self.world,
            &self.v6_db,
            hoi4_state::CountryId(player as u16),
        );
        let stability = self
            .world
            .countries
            .stability
            .get(player)
            .copied()
            .unwrap_or(0.5);
        let war_support = self
            .world
            .countries
            .war_support
            .get(player)
            .copied()
            .unwrap_or(0.5);
        let civ = 0u32;
        let mil = 0u32;
        let dock = 0u32;
        let army_xp = self
            .world
            .countries
            .army_xp
            .get(player)
            .copied()
            .unwrap_or(0.0);
        let navy_xp = self
            .world
            .countries
            .navy_xp
            .get(player)
            .copied()
            .unwrap_or(0.0);
        let air_xp = self
            .world
            .countries
            .air_xp
            .get(player)
            .copied()
            .unwrap_or(0.0);

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
                    // 璇︽儏鍗＄墖
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
        // 鈹€鈹€鈹€ Phase 3.5 / 3.10.3: Country name labels 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€
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

        let _ = (
            pp,
            manpower,
            stability,
            war_support,
            civ,
            mil,
            dock,
            army_xp,
            navy_xp,
            air_xp,
        );

        // 4.3 Step B (2026-05-18): 鍒犻櫎 `politics_pass::draw_politics_overlay`
        // 鐨勭▼搴忓寲鏂囧瓧鍙犲姞璺緞銆傛斂娌婚潰鏉垮唴瀹癸紙political_title / ideology / focus
        // 绛夛級???vanilla `countrypoliticsview.gui` 鍐呯殑 textbox widget 娓叉煋???        // 鏂囧瓧鍐呭閫氳繃 `crate::binding::WorldBinding::query_string(widget_name)`
        // V5 鏀跺彛锛欶1 debug overlay 鍘熸湰璧?GuiRt-removed widget 鏍戯紝宸插仠鐢ㄣ€?
        let _ = player; // 涓婂眰鏁版嵁鍒囩墖浠嶅彲鑳借鍚庣画 UI 姝ラ娑堣垂锛屼繚鐣?player 涓嶆姤閿欍€?

        // Draw HOI3 counter stack labels when the counter pass is enabled.
        if self.game_phase == GamePhase::Playing
            && s.hoi3_counter_pass.enabled()
            && !self._cached_hoi3_counter_upload.is_empty()
        {
            let centers = hoi4_render::counter_v3::collect_top_counter_screen_centers(
                &self._cached_hoi3_counter_upload,
            );
            let dpi = s.window.scale_factor() as f32;
            let inv_dpi = 1.0 / dpi.max(1.0);
            for (cx, cy, sw, sh, stack) in centers.iter().take(512) {
                if *stack == 0 {
                    continue;
                }
                let label = if *stack > 99 {
                    "99+".to_string()
                } else {
                    stack.to_string()
                };
                let logical_h = sh * inv_dpi;
                let logical_w = sw * inv_dpi;
                let above_offset = logical_h * 0.55;
                let font_size = (logical_h * 0.42).clamp(8.0, 18.0);
                let lx = cx * inv_dpi + logical_w * 0.32; // 椤堕儴鍙充笂瑙掑窘绔犱腑蹇冿紝涓?shader 鍦嗗舰 badge 鍚屼綅
                let ly = cy * inv_dpi - logical_h * 0.32;
                let _ = above_offset;
                s.text_pass
                    .draw_text_aligned(&label, lx, ly, TextAlign::Center, font_size);
            }
        }

        // CR-4.6: Off-screen indicators for selected provinces with counters off-screen.
        if self.game_phase == GamePhase::Playing
            && s.hoi3_counter_pass.enabled()
            && !self.selected_province_ids.is_empty()
        {
            let dpi = s.window.scale_factor() as f32;
            let inv_dpi = 1.0 / dpi.max(1.0);
            let lw = s.config.width as f32 * inv_dpi;
            let lh = s.config.height as f32 * inv_dpi;
            for c in self._cached_hoi3_counter_upload.iter() {
                if (c.flags & flag_bits::IS_UNDERLAY) != 0 {
                    continue;
                }
                let pid = c.province_id() as u32;
                if !self.selected_province_ids.contains(&pid) {
                    continue;
                }
                let cx = (c.screen_pos[0] + c.size[0] * 0.5) * inv_dpi;
                let cy = (c.screen_pos[1] + c.size[1] * 0.5) * inv_dpi;
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

        // Phase 3.12.1: F4 娓叉煋璋冭瘯瑕嗙洊鏂囧瓧锛坧ass 鍒楄〃 / HDR 灏哄 / 褰撳墠妯″紡锛??
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
                    let label = format!("{} · {}师", army.name, army.members.len());
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

        s.queue
            .submit(ui_cbufs.into_iter().chain(std::iter::once(enc.finish())));
        if let Some(profiler) = s.gpu_profiler.as_mut() {
            profiler.map_pending_readback();
        }
        frame.present();
        let frame_time_ms = render_started.elapsed().as_secs_f32() * 1000.0;
        self.last_frame_cpu_ms = frame_time_ms;
        let capture_result =
            pending_readback.map(|pending| finish_png_readback(&s.device, pending));
        if let Some(action) = topbar_action {
            ui_binding::apply_topbar_action(self, action);
        }
        if let Some(kind) = side_rail_panel_cmd {
            self.toggle_in_game_panel(in_game_panel_for_panel_kind(kind));
        }
        for tag in deferred_switch_player_country {
            self.set_player_country_by_tag(&tag);
        }
        if let Some(result) = capture_result {
            self.map_phase0_after_capture(frame_time_ms, result);
        } else if map_phase0_active {
            self.map_phase0_after_uncaptured_frame();
        }
        self.perf_render_us = self
            .perf_render_us
            .saturating_add(render_started.elapsed().as_micros() as u64);
        self.perf_render_frames = self.perf_render_frames.saturating_add(1);
    }
}

fn enqueue_png_readback(
    device: &wgpu::Device,
    encoder: &mut wgpu::CommandEncoder,
    texture: &wgpu::Texture,
    format: wgpu::TextureFormat,
    width: u32,
    height: u32,
    path: PathBuf,
) -> PendingPngReadback {
    let unpadded_bytes_per_row = width * 4;
    let padded_bytes_per_row =
        wgpu::util::align_to(unpadded_bytes_per_row, wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("map_phase0_readback"),
        size: (padded_bytes_per_row * height) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_bytes_per_row),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    PendingPngReadback {
        buffer,
        path,
        width,
        height,
        unpadded_bytes_per_row,
        padded_bytes_per_row,
        format,
    }
}

fn finish_png_readback(device: &wgpu::Device, pending: PendingPngReadback) -> Result<(), String> {
    let swizzle = match pending.format {
        wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Rgba8UnormSrgb => [0, 1, 2, 3],
        wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb => [2, 1, 0, 3],
        other => return Err(format!("unsupported screenshot format {other:?}")),
    };

    let buffer_slice = pending.buffer.slice(..);
    let (tx, rx) = mpsc::channel();
    buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = tx.send(result.map_err(|err| format!("{err:?}")));
    });
    let _ = device.poll(wgpu::Maintain::Wait);
    rx.recv()
        .map_err(|err| format!("readback callback failed: {err}"))??;

    let mapped = buffer_slice.get_mapped_range();
    let mut rgba = vec![0u8; (pending.width * pending.height * 4) as usize];
    for y in 0..pending.height as usize {
        let src_start = y * pending.padded_bytes_per_row as usize;
        let src_end = src_start + pending.unpadded_bytes_per_row as usize;
        let src = &mapped[src_start..src_end];
        let dst = &mut rgba[y * pending.width as usize * 4..(y + 1) * pending.width as usize * 4];
        for (src_px, dst_px) in src.chunks_exact(4).zip(dst.chunks_exact_mut(4)) {
            dst_px[0] = src_px[swizzle[0]];
            dst_px[1] = src_px[swizzle[1]];
            dst_px[2] = src_px[swizzle[2]];
            dst_px[3] = src_px[swizzle[3]];
        }
    }
    drop(mapped);
    pending.buffer.unmap();

    if let Some(parent) = pending.path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| format!("create output dir: {err}"))?;
    }
    let file = std::fs::File::create(&pending.path).map_err(|err| format!("create png: {err}"))?;
    let writer = std::io::BufWriter::new(file);
    let mut encoder = png::Encoder::new(writer, pending.width, pending.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .map_err(|err| format!("png header: {err}"))?
        .write_image_data(&rgba)
        .map_err(|err| format!("png data: {err}"))?;
    Ok(())
}

fn estimate_construction_days_remaining(progress: f32, cost: f32) -> Option<u32> {
    if cost <= 0.0 || progress < 0.0 {
        return None;
    }
    let completion = (progress / cost).clamp(0.0, 1.0);
    if completion >= 1.0 {
        return Some(0);
    }
    Some(((1.0 - completion) * 100.0).ceil().max(1.0) as u32)
}

fn autonomy_level_label(level: hoi4_state::AutonomyLevel) -> &'static str {
    match level {
        hoi4_state::AutonomyLevel::Integrated => "整合领土",
        hoi4_state::AutonomyLevel::IntegratedPuppet => "深度傀儡",
        hoi4_state::AutonomyLevel::Puppet => "傀儡国",
        hoi4_state::AutonomyLevel::Dominion => "自治领",
        hoi4_state::AutonomyLevel::Satellite => "卫星国",
        hoi4_state::AutonomyLevel::FreedomAssociation => "自由协约",
    }
}

fn terrain_bucket_signature(bucket: &[ChunkInstance]) -> u64 {
    let mut h = DefaultHasher::new();
    bucket.len().hash(&mut h);
    for instance in bucket {
        instance.origin_xz[0].to_bits().hash(&mut h);
        instance.origin_xz[1].to_bits().hash(&mut h);
        instance.size_xz[0].to_bits().hash(&mut h);
        instance.size_xz[1].to_bits().hash(&mut h);
    }
    h.finish()
}

fn country_display_name_from_world(
    world: &hoi4_state::World,
    country: hoi4_state::CountryId,
) -> String {
    world
        .country_tag(country)
        .map(|tag| hoi4_ui::i18n::tr(tag).to_string())
        .unwrap_or_else(|| hoi4_ui::i18n::tr("unknown").to_owned())
}

fn diplomacy_autonomy_summary(
    world: &hoi4_state::World,
    country: hoi4_state::CountryId,
) -> Option<String> {
    if let Some(autonomy) = world.diplomacy.autonomy.get(&country) {
        let master = world
            .countries
            .tags
            .get(autonomy.master.0 as usize)
            .map(|tag| hoi4_ui::i18n::tr(tag).to_string())
            .unwrap_or_else(|| hoi4_ui::i18n::tr("unknown").to_owned());
        return Some(format!(
            "{}：{}",
            autonomy_level_label(autonomy.level),
            master
        ));
    }

    let subject_count = world
        .diplomacy
        .autonomy
        .values()
        .filter(|autonomy| autonomy.master == country)
        .count();
    if subject_count > 0 {
        Some(format!("宗主国：{} 个附庸", subject_count))
    } else {
        None
    }
}

fn country_wargoal_status(
    world: &hoi4_state::World,
    player: hoi4_state::CountryId,
    target: hoi4_state::CountryId,
) -> (bool, bool, f32, u32) {
    world
        .diplomacy
        .pending_wargoals
        .get(&player)
        .and_then(|wgs| wgs.iter().find(|w| w.target == target))
        .map(|wg| {
            if wg.justified {
                (true, false, 1.0_f32, 0_u32)
            } else {
                let total = wg.justify_total_days.max(1.0);
                let prog = (wg.justify_progress / total).clamp(0.0, 1.0);
                let days = (wg.justify_total_days - wg.justify_progress)
                    .max(0.0)
                    .ceil() as u32;
                (false, true, prog, days)
            }
        })
        .unwrap_or((false, false, 0.0, 0))
}

fn country_wargoal_details(
    world: &hoi4_state::World,
    player: hoi4_state::CountryId,
    target: hoi4_state::CountryId,
) -> Vec<hoi4_ui::diplomacy::WargoalDetailEntry> {
    world
        .diplomacy
        .pending_wargoals
        .get(&player)
        .map(|wargoals| {
            wargoals
                .iter()
                .filter(|goal| goal.target == target)
                .map(|goal| {
                    let total = goal.justify_total_days.max(1.0);
                    let progress = if goal.justified {
                        1.0
                    } else {
                        (goal.justify_progress / total).clamp(0.0, 1.0)
                    };
                    hoi4_ui::diplomacy::WargoalDetailEntry {
                        kind: wargoal_kind_label(goal.kind).to_owned(),
                        target_state: goal.target_state.map(|state| state.0),
                        status: if goal.justified {
                            "已正当化".to_owned()
                        } else {
                            "正当化中".to_owned()
                        },
                        progress,
                        days_remaining: if goal.justified {
                            0
                        } else {
                            (goal.justify_total_days - goal.justify_progress)
                                .max(0.0)
                                .ceil() as u32
                        },
                        source: "外交正当化".to_owned(),
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

fn country_relation_factors(
    world: &hoi4_state::World,
    player: hoi4_state::CountryId,
    target: hoi4_state::CountryId,
) -> Vec<hoi4_ui::diplomacy::RelationFactorEntry> {
    let mut factors = Vec::new();
    let opinion = world.diplomacy.opinions.get(player, target);
    factors.push(hoi4_ui::diplomacy::RelationFactorEntry {
        label: "我国对目标关系".to_owned(),
        value: format!("{opinion:+}"),
        positive: opinion >= 0,
    });

    let reverse = world.diplomacy.opinions.get(target, player);
    factors.push(hoi4_ui::diplomacy::RelationFactorEntry {
        label: "目标对我国关系".to_owned(),
        value: format!("{reverse:+}"),
        positive: reverse >= 0,
    });

    if world.diplomacy.at_war_with(player, target) {
        factors.push(hoi4_ui::diplomacy::RelationFactorEntry {
            label: "战争状态".to_owned(),
            value: "交战中".to_owned(),
            positive: false,
        });
    } else {
        factors.push(hoi4_ui::diplomacy::RelationFactorEntry {
            label: "战争状态".to_owned(),
            value: "和平".to_owned(),
            positive: true,
        });
    }

    match (
        world.diplomacy.faction_of(player),
        world.diplomacy.faction_of(target),
    ) {
        (Some(a), Some(b)) if a == b => factors.push(hoi4_ui::diplomacy::RelationFactorEntry {
            label: "阵营关系".to_owned(),
            value: "同阵营".to_owned(),
            positive: true,
        }),
        (Some(_), Some(_)) => factors.push(hoi4_ui::diplomacy::RelationFactorEntry {
            label: "阵营关系".to_owned(),
            value: "不同阵营".to_owned(),
            positive: false,
        }),
        _ => factors.push(hoi4_ui::diplomacy::RelationFactorEntry {
            label: "阵营关系".to_owned(),
            value: "无共同阵营".to_owned(),
            positive: false,
        }),
    }

    if world.diplomacy.is_subject_of(target, player) {
        factors.push(hoi4_ui::diplomacy::RelationFactorEntry {
            label: "附庸关系".to_owned(),
            value: "目标是我国附庸".to_owned(),
            positive: true,
        });
    } else if world.diplomacy.is_subject_of(player, target) {
        factors.push(hoi4_ui::diplomacy::RelationFactorEntry {
            label: "附庸关系".to_owned(),
            value: "我国是目标附庸".to_owned(),
            positive: false,
        });
    }

    factors
}

fn wargoal_kind_label(kind: hoi4_state::WargoalType) -> &'static str {
    match kind {
        hoi4_state::WargoalType::Annex => "吞并",
        hoi4_state::WargoalType::TakeState => "夺取州",
        hoi4_state::WargoalType::Liberate => "解放",
        hoi4_state::WargoalType::Puppet => "傀儡化",
        hoi4_state::WargoalType::ToppleGovernment => "推翻政府",
        hoi4_state::WargoalType::NavalAccess => "海军通行",
    }
}

fn diplomacy_action_view(
    world: &hoi4_state::World,
    actor: hoi4_state::CountryId,
    action: hoi4_logic::diplomacy::DiplomaticAction,
) -> hoi4_ui::diplomacy::DiplomaticActionView {
    let preview = hoi4_logic::diplomacy::preview_action(world, actor, &action).summary;
    let availability = hoi4_logic::diplomacy::evaluate_action(world, actor, &action);
    if availability.available {
        hoi4_ui::diplomacy::DiplomaticActionView::enabled(preview)
    } else {
        hoi4_ui::diplomacy::DiplomaticActionView::disabled(
            preview,
            diplomacy_unavailable_reason_text(availability.reason.as_ref()),
        )
    }
}

fn diplomacy_unavailable_reason_text(
    reason: Option<&hoi4_logic::diplomacy::UnavailableReason>,
) -> String {
    use hoi4_logic::diplomacy::UnavailableReason;
    match reason {
        Some(UnavailableReason::BadActor) => "行动发起国无效".to_owned(),
        Some(UnavailableReason::BadTarget) => "目标国家无效".to_owned(),
        Some(UnavailableReason::SelfTarget) => "不能以本国作为目标".to_owned(),
        Some(UnavailableReason::AlreadyAtWar) => "已经处于战争状态".to_owned(),
        Some(UnavailableReason::MissingJustifiedWargoal) => "需要已正当化的战争目标".to_owned(),
        Some(UnavailableReason::AlreadyInFaction) => "我国已经在阵营中".to_owned(),
        Some(UnavailableReason::NotInFaction) => "我国不在任何阵营中".to_owned(),
        Some(UnavailableReason::TargetAlreadyInFaction) => "目标已经加入阵营".to_owned(),
        Some(UnavailableReason::NoFactionToInviteFrom) => "我国需要先创建或加入阵营".to_owned(),
        Some(UnavailableReason::OpinionTooLow { current, required }) => {
            format!("目标对我国关系不足：当前 {current}，需要 {required}")
        }
        Some(UnavailableReason::DuplicateWargoal) => "已经存在相同战争目标".to_owned(),
        Some(UnavailableReason::InsufficientPoliticalPower) => "政治点数不足".to_owned(),
        Some(UnavailableReason::MissingTargetState) => "该战争目标需要指定州".to_owned(),
        Some(UnavailableReason::ExtraneousTargetState) => "该战争目标不能指定州".to_owned(),
        Some(UnavailableReason::BadFaction) => "阵营无效".to_owned(),
        Some(UnavailableReason::WarNotFound) => "战争不存在".to_owned(),
        Some(UnavailableReason::NotWarParticipant) => "我国不是该战争参战方".to_owned(),
        Some(UnavailableReason::DuplicatePendingRequest) => "已有待处理外交请求".to_owned(),
        Some(UnavailableReason::PeaceNotReady) => {
            "和平会议条件不足：需要足够战争分数或敌方投降".to_owned()
        }
        None => "行动不可用".to_owned(),
    }
}

fn intervention_expected_impact(
    progress_boost: f32,
    army_xp: f32,
    air_xp: f32,
    cooldown_days: u32,
) -> String {
    let mut parts = Vec::new();
    if progress_boost.abs() > f32::EPSILON {
        parts.push(format!("目标阵营进度 +{:.0}", progress_boost));
    }
    if army_xp > 0.0 {
        parts.push(format!("陆军经验 +{:.0}", army_xp));
    }
    if air_xp > 0.0 {
        parts.push(format!("空军经验 +{:.0}", air_xp));
    }
    if cooldown_days > 0 {
        parts.push(format!("执行后冷却 {} 天", cooldown_days));
    }
    if parts.is_empty() {
        "效果已由局势脚本定义，执行后立即结算。".to_owned()
    } else {
        parts.join("；")
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

fn localized_state_name(
    raw: &str,
    state_idx: usize,
    loc_catalog: &hoi4_ui::loc::LocCatalog,
) -> String {
    if raw.is_empty() {
        return format!("第{}州", state_idx);
    }
    let localized = loc_catalog.tr(raw);
    if localized != raw {
        return localized.to_owned();
    }
    if let Some(id) = raw
        .strip_prefix("STATE_")
        .and_then(|s| s.parse::<u16>().ok())
    {
        return format!("第{}州", id);
    }
    raw.to_owned()
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

fn decision_id_matches_player_tag(decision_id: &str, player_tag_lower: &str) -> bool {
    let Some((prefix, _)) = decision_id.split_once('.') else {
        return true;
    };
    prefix.eq_ignore_ascii_case(player_tag_lower)
}

/// Build the country selection list.
fn build_country_select_list(world: &World) -> Vec<CountryEntry> {
    // Vanilla 1936 8 majors + 鍑犱釜甯歌灏忓浗
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
            // ???tag 鏈姞杞斤紝璺宠繃
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

fn setup_tree_mesh_pipeline(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    path_cfg: &PathConfig,
    tree_instances: &[TreeInstance],
    camera_buffer: &wgpu::Buffer,
    params_buffer: &wgpu::Buffer,
    surface_format: wgpu::TextureFormat,
    depth_format: wgpu::TextureFormat,
) -> (
    wgpu::RenderPipeline,
    Vec<wgpu::BindGroup>,
    Vec<wgpu::Buffer>,
    Vec<wgpu::Buffer>,
    Vec<wgpu::Buffer>,
    Vec<u32>,
    Vec<u32>,
) {
    use hoi4_assets::{AssetDb, DdsImage, FsAssetDb, PdxMesh};

    let db = FsAssetDb::new(path_cfg.clone());

    let tree_defs: [(&str, &str); 3] = [
        (
            "gfx/models/mapitems/trees/beech.mesh",
            "gfx/models/mapitems/trees/beech_diffuse.dds",
        ),
        (
            "gfx/models/mapitems/trees/Pine_01.mesh",
            "gfx/models/mapitems/trees/pinetree_diffuse.dds",
        ),
        (
            "gfx/models/mapitems/trees/palmer.mesh",
            "gfx/models/mapitems/trees/palm_lod_diffuse.dds",
        ),
    ];

    let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("trees_mesh_bgl"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    });

    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Linear,
        address_mode_u: wgpu::AddressMode::Repeat,
        address_mode_v: wgpu::AddressMode::Repeat,
        ..Default::default()
    });

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("trees_mesh_shader"),
        source: wgpu::ShaderSource::Wgsl(hoi4_render::SHADER_TREES_MESH_WGSL.into()),
    });
    let pl_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[&bgl],
        push_constant_ranges: &[],
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("trees_mesh_pipeline"),
        layout: Some(&pl_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[
                wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<TreeMeshVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: wgpu::VertexFormat::Float32x3,
                        },
                        wgpu::VertexAttribute {
                            offset: 12,
                            shader_location: 1,
                            format: wgpu::VertexFormat::Float32x3,
                        },
                        wgpu::VertexAttribute {
                            offset: 24,
                            shader_location: 2,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                    ],
                },
                wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<TreeMeshInstance>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &[
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 3,
                            format: wgpu::VertexFormat::Float32x3,
                        },
                        wgpu::VertexAttribute {
                            offset: 12,
                            shader_location: 4,
                            format: wgpu::VertexFormat::Float32,
                        },
                        wgpu::VertexAttribute {
                            offset: 16,
                            shader_location: 5,
                            format: wgpu::VertexFormat::Unorm8x4,
                        },
                        // Phase 3.10.2: per-instance slope (Snorm8x2 ???vec2<f32>).
                        // Offset 22 follows tree_type+pad@20 (2 bytes Uint8x2)
                        // even though the mesh shader doesn't read tree_type
                        // (it's already encoded by which draw call uses
                        // which mesh+texture).
                        wgpu::VertexAttribute {
                            offset: 22,
                            shader_location: 6,
                            format: wgpu::VertexFormat::Snorm8x2,
                        },
                    ],
                },
            ],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: surface_format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: depth_format,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::Less,
            stencil: Default::default(),
            bias: Default::default(),
        }),
        multisample: Default::default(),
        multiview: None,
        cache: None,
    });

    let mut bind_groups = Vec::new();
    let mut vertex_buffers = Vec::new();
    let mut index_buffers = Vec::new();
    let mut instance_buffers = Vec::new();
    let mut index_counts = Vec::new();
    let mut instance_counts = Vec::new();

    let make_white = || -> wgpu::TextureView {
        let t = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("w1"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &t,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &[255u8, 255, 255, 255],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: None,
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        t.create_view(&Default::default())
    };

    for (type_idx, (mesh_path, tex_path)) in tree_defs.iter().enumerate() {
        // Load mesh
        let mesh_opt = db
            .open(*mesh_path)
            .ok()
            .and_then(|bytes| match PdxMesh::parse(&bytes) {
                Ok(m) => {
                    if m.meshes.is_empty() {
                        eprintln!("[trees_mesh] {} parsed OK but 0 submeshes", mesh_path);
                        None
                    } else {
                        let sub = m.meshes.into_iter().next().unwrap();
                        eprintln!(
                            "[trees_mesh] {} submesh: {} pos, {} idx",
                            mesh_path,
                            sub.positions.len(),
                            sub.indices.len()
                        );
                        build_tree_mesh(&sub.positions, &sub.normals, &sub.uvs, &sub.indices)
                    }
                }
                Err(e) => {
                    eprintln!("[trees_mesh] {} parse error: {}", mesh_path, e);
                    None
                }
            });

        let mesh_data = match mesh_opt {
            Some(d) => {
                // Debug: print mesh bounds
                let (mut mn, mut mx) = ([f32::MAX; 3], [f32::MIN; 3]);
                for v in &d.vertices {
                    for i in 0..3 {
                        mn[i] = mn[i].min(v.position[i]);
                        mx[i] = mx[i].max(v.position[i]);
                    }
                }
                println!(
                    "[trees_mesh] {}  ?{} verts, {} idx, bounds [{:.2},{:.2},{:.2}]鈫抂{:.2},{:.2},{:.2}]",
                    mesh_path,
                    d.vertex_count,
                    d.index_count,
                    mn[0],
                    mn[1],
                    mn[2],
                    mx[0],
                    mx[1],
                    mx[2]
                );
                d
            }
            None => {
                eprintln!("[trees_mesh] failed to load {}", mesh_path);
                // Push empty placeholders
                let tv = make_white();
                bind_groups.push(device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: None,
                    layout: &bgl,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: camera_buffer.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: params_buffer.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: wgpu::BindingResource::TextureView(&tv),
                        },
                        wgpu::BindGroupEntry {
                            binding: 3,
                            resource: wgpu::BindingResource::Sampler(&sampler),
                        },
                    ],
                }));
                vertex_buffers.push(
                    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: None,
                        contents: &[0u8; 32],
                        usage: wgpu::BufferUsages::VERTEX,
                    }),
                );
                index_buffers.push(
                    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: None,
                        contents: &[0u8; 4],
                        usage: wgpu::BufferUsages::INDEX,
                    }),
                );
                instance_buffers.push(device.create_buffer_init(
                    &wgpu::util::BufferInitDescriptor {
                        label: None,
                        contents: &[0u8; 24],
                        usage: wgpu::BufferUsages::VERTEX,
                    },
                ));
                index_counts.push(0);
                instance_counts.push(0);
                continue;
            }
        };

        // Load texture
        let tex_view = db
            .open(*tex_path)
            .ok()
            .and_then(|bytes| DdsImage::parse(&bytes).ok())
            .map(|dds| {
                let fmt = match dds.format {
                    hoi4_assets::DdsFormat::Bc1 => wgpu::TextureFormat::Bc1RgbaUnormSrgb,
                    hoi4_assets::DdsFormat::Bc3 => wgpu::TextureFormat::Bc3RgbaUnormSrgb,
                    _ => wgpu::TextureFormat::Bc3RgbaUnormSrgb,
                };
                // Only include mip levels >= 4脳4 for BC formats.
                let valid_mips = dds
                    .mips
                    .iter()
                    .take_while(|m| m.width >= 4 && m.height >= 4)
                    .count() as u32;
                let mip_count = valid_mips.max(1);
                let tex = device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("tree_tex"),
                    size: wgpu::Extent3d {
                        width: dds.width,
                        height: dds.height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: mip_count,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: fmt,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                });
                for (i, mip) in dds.mips.iter().take(mip_count as usize).enumerate() {
                    let data = &dds.data[mip.offset..mip.offset + mip.size];
                    let bw = (mip.width + 3) / 4;
                    let bpb: u32 = match dds.format {
                        hoi4_assets::DdsFormat::Bc1 => 8,
                        _ => 16,
                    };
                    queue.write_texture(
                        wgpu::TexelCopyTextureInfo {
                            texture: &tex,
                            mip_level: i as u32,
                            origin: wgpu::Origin3d::ZERO,
                            aspect: wgpu::TextureAspect::All,
                        },
                        data,
                        wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(bw * bpb),
                            rows_per_image: None,
                        },
                        wgpu::Extent3d {
                            width: mip.width,
                            height: mip.height,
                            depth_or_array_layers: 1,
                        },
                    );
                }
                println!(
                    "[trees_mesh] tex {} ({}x{} {:?} {} mips)",
                    tex_path, dds.width, dds.height, dds.format, mip_count
                );
                tex.create_view(&Default::default())
            })
            .unwrap_or_else(|| make_white());

        bind_groups.push(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: params_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&tex_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        }));

        vertex_buffers.push(
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("tree_vb"),
                contents: bytemuck::cast_slice(&mesh_data.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            }),
        );
        index_buffers.push(
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("tree_ib"),
                contents: bytemuck::cast_slice(&mesh_data.indices),
                usage: wgpu::BufferUsages::INDEX,
            }),
        );
        index_counts.push(mesh_data.index_count);

        let raw_instances = filter_instances_for_type(tree_instances, type_idx as u8);
        let pre_cap = raw_instances.len();
        let instances = hoi4_render::trees_mesh::cap_instances(
            raw_instances,
            hoi4_render::trees_mesh::INSTANCE_CAP_PER_TYPE,
        );
        println!(
            "[trees_mesh] type {}  ?{} instances (pre-cap {}, cap {})",
            type_idx,
            instances.len(),
            pre_cap,
            hoi4_render::trees_mesh::INSTANCE_CAP_PER_TYPE,
        );
        instance_counts.push(instances.len() as u32);
        instance_buffers.push(
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("tree_inst"),
                contents: if instances.is_empty() {
                    &[0u8; 24]
                } else {
                    bytemuck::cast_slice(&instances)
                },
                usage: wgpu::BufferUsages::VERTEX,
            }),
        );
    }

    (
        pipeline,
        bind_groups,
        vertex_buffers,
        index_buffers,
        instance_buffers,
        index_counts,
        instance_counts,
    )
}

/// Phase 3.6.3: Load tree billboard textures (3 separate compressed textures).
fn first_open_line_slot(
    templates: &[hoi4_data::DivisionTemplate],
    template_idx: u16,
) -> Option<(u8, u8)> {
    let template = templates.get(template_idx as usize)?;
    let editable = template.to_editable(template_idx as u32, false);
    for row in 0..5 {
        for col in 0..5 {
            if editable.line_battalions[row][col].is_none() {
                return Some((row as u8, col as u8));
            }
        }
    }
    None
}

fn first_open_support_slot(
    templates: &[hoi4_data::DivisionTemplate],
    template_idx: u16,
) -> Option<u8> {
    let template = templates.get(template_idx as usize)?;
    let editable = template.to_editable(template_idx as u32, false);
    editable
        .support_companies
        .iter()
        .position(|slot| slot.is_none())
        .map(|slot| slot as u8)
}

fn empty_template_editor_data() -> hoi4_ui::military::TemplateEditorData {
    hoi4_ui::military::TemplateEditorData {
        selected_template: None,
        name: String::new(),
        line_battalions: vec![vec![None; 5]; 5],
        support_companies: vec![None; 5],
        line_choices: Vec::new(),
        support_choices: Vec::new(),
        combat_width: 0.0,
        manpower: 0,
        max_organisation: 0.0,
        soft_attack: 0.0,
        hard_attack: 0.0,
        defense: 0.0,
        breakthrough: 0.0,
        suppression: 0.0,
        supply_consumption: 0.0,
        training_days: 0.0,
        equipment_needed: Vec::new(),
        stockpile_satisfied_divisions: 0.0,
    }
}

fn empty_template_subunit_picker_data() -> hoi4_ui::military::TemplateSubunitPickerData {
    hoi4_ui::military::TemplateSubunitPickerData {
        target: None,
        title: String::new(),
        current: None,
        choices: Vec::new(),
    }
}

fn build_template_subunit_picker_data(
    data: &hoi4_data::GameData,
    country_tag: &str,
    target: Option<hoi4_ui::military::TemplatePickerTarget>,
) -> hoi4_ui::military::TemplateSubunitPickerData {
    let Some(target) = target else {
        return empty_template_subunit_picker_data();
    };
    let Some(templates) = data.division_templates.get(country_tag) else {
        return empty_template_subunit_picker_data();
    };

    let (template_idx, current, choices, title) = match target {
        hoi4_ui::military::TemplatePickerTarget::Line { template, row, col } => {
            let current = templates
                .get(template as usize)
                .map(|template_def| template_def.to_editable(template as u32, false))
                .and_then(|editable| editable.line_battalions[row as usize][col as usize].clone());
            let mut choices = data
                .subunits
                .values()
                .filter(|subunit| subunit.is_land() && !looks_like_support_subunit(&subunit.key))
                .map(|subunit| subunit.key.clone())
                .collect::<Vec<_>>();
            choices.sort();
            choices.dedup();
            if choices.is_empty() {
                choices.push("infantry".to_owned());
            }
            (
                template,
                current,
                choices,
                format!("选择战斗营 R{} C{}", row + 1, col + 1),
            )
        }
        hoi4_ui::military::TemplatePickerTarget::Support { template, slot } => {
            let current = templates
                .get(template as usize)
                .map(|template_def| template_def.to_editable(template as u32, false))
                .and_then(|editable| editable.support_companies[slot as usize].clone());
            let mut choices = data
                .subunits
                .values()
                .filter(|subunit| {
                    looks_like_support_subunit(&subunit.key) || subunit.suppression > 0.0
                })
                .map(|subunit| subunit.key.clone())
                .collect::<Vec<_>>();
            choices.sort();
            choices.dedup();
            if choices.is_empty() {
                choices.extend([
                    "engineer".to_owned(),
                    "recon".to_owned(),
                    "support_artillery".to_owned(),
                ]);
            }
            (
                template,
                current,
                choices,
                format!("选择支援连 {}", slot + 1),
            )
        }
    };

    if (template_idx as usize) >= templates.len() {
        return empty_template_subunit_picker_data();
    }

    hoi4_ui::military::TemplateSubunitPickerData {
        target: Some(target),
        title,
        current,
        choices,
    }
}

fn build_template_editor_data(
    data: &hoi4_data::GameData,
    country_tag: &str,
    selected_template: Option<u16>,
    stockpile: Option<&std::collections::HashMap<String, f32>>,
) -> hoi4_ui::military::TemplateEditorData {
    let Some(templates) = data.division_templates.get(country_tag) else {
        return empty_template_editor_data();
    };
    if templates.is_empty() {
        return empty_template_editor_data();
    }

    let selected = selected_template
        .filter(|idx| (*idx as usize) < templates.len())
        .unwrap_or(0);
    let template = &templates[selected as usize];
    let editable = template.to_editable(selected as u32, false);
    let preview = hoi4_logic::military::templates::preview_template(template, data, stockpile);

    let line_battalions = editable
        .line_battalions
        .iter()
        .map(|row| row.iter().cloned().collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let support_companies = editable
        .support_companies
        .iter()
        .cloned()
        .collect::<Vec<_>>();

    let mut line_choices = data
        .subunits
        .values()
        .filter(|subunit| subunit.is_land() && !looks_like_support_subunit(&subunit.key))
        .map(|subunit| subunit.key.clone())
        .collect::<Vec<_>>();
    line_choices.sort();
    line_choices.dedup();
    if line_choices.is_empty() && data.subunits.contains_key("infantry") {
        line_choices.push("infantry".to_owned());
    }

    let mut support_choices = data
        .subunits
        .values()
        .filter(|subunit| looks_like_support_subunit(&subunit.key) || subunit.suppression > 0.0)
        .map(|subunit| subunit.key.clone())
        .collect::<Vec<_>>();
    support_choices.sort();
    support_choices.dedup();
    if support_choices.is_empty() {
        for fallback in ["engineer", "recon", "support_artillery"] {
            support_choices.push(fallback.to_owned());
        }
    }

    let mut equipment_needed = preview.equipment_needed.into_iter().collect::<Vec<_>>();
    equipment_needed.sort_by(|a, b| a.0.cmp(&b.0));

    hoi4_ui::military::TemplateEditorData {
        selected_template: Some(selected),
        name: template.name.clone(),
        line_battalions,
        support_companies,
        line_choices,
        support_choices,
        combat_width: preview.combat_width,
        manpower: preview.manpower,
        max_organisation: preview.max_organisation,
        soft_attack: preview.soft_attack,
        hard_attack: preview.hard_attack,
        defense: preview.defense,
        breakthrough: preview.breakthrough,
        suppression: preview.suppression,
        supply_consumption: preview.supply_consumption,
        training_days: preview.training_days,
        equipment_needed,
        stockpile_satisfied_divisions: preview.stockpile_satisfied_divisions,
    }
}

fn looks_like_support_subunit(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    key.contains("support")
        || key.contains("engineer")
        || key.contains("recon")
        || key.contains("maintenance")
        || key.contains("logistics")
        || key.contains("signal")
        || key.contains("hospital")
}

fn is_land_province_for_bubble(world: &World, raw_id: u16) -> bool {
    world
        .map
        .get_province(raw_id)
        .is_some_and(|def| matches!(def.province_type, hoi4_map::ProvinceType::Land))
}

fn stable_combat_bubble_id(a: u16, b: u16, focus: hoi4_state::CountryId) -> u64 {
    let mut h = DefaultHasher::new();
    a.hash(&mut h);
    b.hash(&mut h);
    focus.hash(&mut h);
    h.finish()
}

fn combat_side_power(side: &CombatSideSnapshot) -> f32 {
    let posture = if side.is_attacking {
        side.breakthrough * 0.35
    } else {
        side.defense * 0.35
    };
    (side.soft_attack + side.hard_attack * 0.75 + posture + side.active_divisions as f32 * 2.5)
        .max(0.1)
}

fn combat_advantages(
    side_a: &CombatSideSnapshot,
    side_b: &CombatSideSnapshot,
    focus_country: hoi4_state::CountryId,
) -> Vec<String> {
    let (own, enemy) = if side_b.country == focus_country {
        (side_b, side_a)
    } else {
        (side_a, side_b)
    };
    let mut advantages = Vec::new();
    let attack_ratio = own.soft_attack.max(1.0) / enemy.soft_attack.max(1.0);
    let org_delta = own.avg_org_pct - enemy.avg_org_pct;
    let strength_delta = own.avg_strength_pct - enemy.avg_strength_pct;
    let div_delta = own.active_divisions as i32 - enemy.active_divisions as i32;

    if attack_ratio >= 1.25 {
        advantages.push(format!("火力优势 +{:.0}%", (attack_ratio - 1.0) * 100.0));
    } else if attack_ratio <= 0.80 {
        advantages.push(format!("火力劣势 {:.0}%", (attack_ratio - 1.0) * 100.0));
    }
    if org_delta.abs() >= 8.0 {
        advantages.push(format!("组织度差 {:+.0}%", org_delta));
    }
    if strength_delta.abs() >= 8.0 {
        advantages.push(format!("兵力完整度 {:+.0}%", strength_delta));
    }
    if div_delta != 0 {
        advantages.push(format!("参战师数 {:+}", div_delta));
    }
    if advantages.is_empty() {
        advantages.push("态势接近，胜负主要取决于后续组织度消耗".to_owned());
    }
    advantages
}

fn combat_chance_color(chance: u8) -> hoi4_ui::egui::Color32 {
    if chance >= 65 {
        hoi4_ui::egui::Color32::from_rgb(0x44, 0xb8, 0x68)
    } else if chance >= 45 {
        hoi4_ui::egui::Color32::from_rgb(0xe0, 0xb8, 0x4c)
    } else {
        hoi4_ui::egui::Color32::from_rgb(0xc8, 0x4f, 0x46)
    }
}

fn show_combat_bubble_overlay(
    ctx: &hoi4_ui::egui::Context,
    bubbles: &[CombatBubbleSnapshot],
    selected: &mut Option<u64>,
) {
    use hoi4_ui::egui::{self, Align2, Color32, FontId, RichText, Sense, Stroke, Vec2};

    if selected.is_some_and(|id| !bubbles.iter().any(|b| b.id == id)) {
        *selected = None;
    }

    let mut pointer_over_combat_ui = false;
    let mut clicked_combat_ui = false;

    let avail = ctx.available_rect();
    for bubble in bubbles {
        let chance = bubble.chance_pct;
        let color = combat_chance_color(chance);
        let center = egui::pos2(bubble.screen_pos[0], bubble.screen_pos[1]);
        // Skip bubbles whose anchor falls under a side/bottom panel.
        if !avail.contains(center) {
            continue;
        }
        let id = egui::Id::new(("combat_bubble", bubble.id));
        let radius = 14.0;
        let size = Vec2::splat(radius * 2.0);
        let pos = egui::pos2(center.x - radius, center.y - radius);
        egui::Area::new(id)
            .order(egui::Order::Background)
            .fixed_pos(pos)
            .show(ctx, |ui| {
                let (rect, response) = ui.allocate_exact_size(size, Sense::click());
                let painter = ui.painter();
                let selected_here = *selected == Some(bubble.id);
                let body = Color32::from_rgba_premultiplied(232, 208, 152, 235);
                let outline = if selected_here {
                    Color32::from_rgba_premultiplied(40, 30, 18, 255)
                } else {
                    Color32::from_rgba_premultiplied(70, 52, 30, 230)
                };
                painter.circle_filled(
                    rect.center() + egui::vec2(0.0, 1.5),
                    radius,
                    Color32::from_rgba_premultiplied(0, 0, 0, 80),
                );
                painter.circle_filled(rect.center(), radius, body);
                painter.circle_stroke(
                    rect.center(),
                    radius,
                    Stroke::new(if selected_here { 1.8 } else { 1.0 }, outline),
                );
                painter.text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    chance.to_string(),
                    FontId::proportional(12.5),
                    color,
                );
                pointer_over_combat_ui |= response.hovered();
                if response.clicked() {
                    clicked_combat_ui = true;
                    *selected = if selected_here { None } else { Some(bubble.id) };
                }
                response.on_hover_text(format!(
                    "{} ({}) vs {} ({})：{}%",
                    bubble.side_a.tag,
                    bubble.side_a.active_divisions,
                    bubble.side_b.tag,
                    bubble.side_b.active_divisions,
                    chance
                ));
            });
    }

    let Some(selected_id) = *selected else {
        return;
    };
    let Some(bubble) = bubbles.iter().find(|b| b.id == selected_id) else {
        return;
    };
    let panel_pos = egui::pos2(
        (bubble.screen_pos[0] + 26.0).clamp(8.0, ctx.screen_rect().right() - 360.0),
        (bubble.screen_pos[1] - 28.0).clamp(52.0, ctx.screen_rect().bottom() - 360.0),
    );
    egui::Window::new("战斗详情")
        .id(egui::Id::new(("combat_detail", bubble.id)))
        .order(egui::Order::Background)
        .fixed_pos(panel_pos)
        .default_width(340.0)
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!("{}%", bubble.chance_pct))
                        .strong()
                        .color(combat_chance_color(bubble.chance_pct)),
                );
                ui.label(format!("{} vs {}", bubble.side_a.name, bubble.side_b.name));
            });
            ui.separator();
            combat_side_detail(ui, &bubble.side_a);
            ui.add_space(4.0);
            combat_side_detail(ui, &bubble.side_b);
            ui.separator();
            ui.label(RichText::new("态势").strong());
            for item in &bubble.advantages {
                ui.label(item);
            }
        })
        .inspect(|inner| {
            pointer_over_combat_ui |= inner.response.hovered();
        });

    let clicked_elsewhere =
        ctx.input(|i| i.pointer.any_click()) && !pointer_over_combat_ui && !clicked_combat_ui;
    if clicked_elsewhere {
        *selected = None;
    }
}

fn combat_side_detail(ui: &mut hoi4_ui::egui::Ui, side: &CombatSideSnapshot) {
    use hoi4_ui::egui::{Color32, RichText};
    let posture = if side.is_attacking {
        "进攻"
    } else {
        "防守"
    };
    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("{} {}", side.tag, posture)).strong());
            ui.with_layout(
                hoi4_ui::egui::Layout::right_to_left(hoi4_ui::egui::Align::Center),
                |ui| {
                    ui.label(format!("省份 {}", side.province));
                },
            );
        });
        ui.horizontal(|ui| {
            ui.label(format!(
                "参战 {} / 预备 {}",
                side.active_divisions, side.reserve_divisions
            ));
            ui.label(format!("宽度 {:.0}", side.combat_width));
        });
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("组织 {:.0}%", side.avg_org_pct)).color(
                    if side.avg_org_pct >= 50.0 {
                        Color32::from_rgb(0x73, 0xc5, 0x79)
                    } else {
                        Color32::from_rgb(0xe0, 0x64, 0x5f)
                    },
                ),
            );
            ui.label(format!("兵力 {:.0}%", side.avg_strength_pct));
        });
        ui.horizontal(|ui| {
            ui.label(format!("软攻 {:.0}", side.soft_attack));
            ui.label(format!("硬攻 {:.0}", side.hard_attack));
        });
        ui.horizontal(|ui| {
            ui.label(format!("防御 {:.0}", side.defense));
            ui.label(format!("突破 {:.0}", side.breakthrough));
        });
    });
}

fn load_tree_atlas(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    path_cfg: &PathConfig,
) -> (wgpu::TextureView, wgpu::Sampler) {
    use hoi4_assets::{AssetDb, DdsImage, FsAssetDb};
    let db = FsAssetDb::new(path_cfg.clone());

    let tree_paths = [
        "gfx/models/mapitems/trees/beech_diffuse.dds",
        "gfx/models/mapitems/trees/pinetree_diffuse.dds",
        "gfx/models/mapitems/trees/palm_lod_diffuse.dds",
    ];

    // Try to load all 3 textures.
    let mut images: Vec<DdsImage> = Vec::new();
    for p in &tree_paths {
        match db.open(*p) {
            Ok(bytes) => match DdsImage::parse(&bytes) {
                Ok(d) => {
                    println!(
                        "[trees] loaded {} ({}x{} {:?})",
                        p, d.width, d.height, d.format
                    );
                    images.push(d);
                }
                Err(e) => {
                    eprintln!("[trees] failed to parse {}: {}", p, e);
                    break;
                }
            },
            Err(e) => {
                eprintln!("[trees] not found {}: {}", p, e);
                break;
            }
        }
    }

    if images.len() < 3 {
        eprintln!("[trees] using 1脳1 white fallback (shader will use procedural trees)");
        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("tree_atlas_fallback"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &[255u8, 255, 255, 255],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: None,
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        return (tex.create_view(&Default::default()), sampler);
    }

    // Use the largest texture size as the atlas cell size (256脳256).
    // Create a 2D texture with 3 rows stacked vertically (256脳768).
    // All textures are BC3 (DXT5). We'll use the largest as target.
    let target_w: u32 = 256;
    let target_h: u32 = 256;
    let atlas_h = target_h * 3;

    // For BC3: 16 bytes per 4脳4 block. 256脳256 = 64脳64 blocks = 4096 blocks 脳 16 = 65536 bytes per layer.
    let blocks_per_row = target_w / 4;
    let blocks_per_col = target_h / 4;
    let bpb: u32 = 16; // BC3 = 16 bytes per block
    let layer_bytes = (blocks_per_row * blocks_per_col * bpb) as usize;
    let total_bytes = layer_bytes * 3;

    let mut atlas_data = vec![0u8; total_bytes];

    for (i, img) in images.iter().enumerate() {
        let mip0 = &img.data[img.mips[0].offset..img.mips[0].offset + img.mips[0].size];
        let dst_offset = i * layer_bytes;

        if img.width == target_w && img.height == target_h {
            // Direct copy.
            let copy_len = mip0.len().min(layer_bytes);
            atlas_data[dst_offset..dst_offset + copy_len].copy_from_slice(&mip0[..copy_len]);
        } else {
            // Smaller texture (e.g., 64脳64): tile it to fill 256脳256.
            let src_bw = img.width / 4;
            let src_bh = img.height / 4;
            for by in 0..blocks_per_col {
                for bx in 0..blocks_per_row {
                    let src_bx = bx % src_bw;
                    let src_by = by % src_bh;
                    let src_idx = ((src_by * src_bw + src_bx) * bpb) as usize;
                    let dst_idx = dst_offset + ((by * blocks_per_row + bx) * bpb) as usize;
                    if src_idx + bpb as usize <= mip0.len()
                        && dst_idx + bpb as usize <= atlas_data.len()
                    {
                        atlas_data[dst_idx..dst_idx + bpb as usize]
                            .copy_from_slice(&mip0[src_idx..src_idx + bpb as usize]);
                    }
                }
            }
        }
    }

    println!(
        "[trees] atlas: {}x{} BC3 (3 rows of {}x{})",
        target_w, atlas_h, target_w, target_h
    );

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("tree_atlas"),
        size: wgpu::Extent3d {
            width: target_w,
            height: atlas_h,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Bc3RgbaUnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });

    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &atlas_data,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(blocks_per_row * bpb),
            rows_per_image: None,
        },
        wgpu::Extent3d {
            width: target_w,
            height: atlas_h,
            depth_or_array_layers: 1,
        },
    );

    let view = texture.create_view(&Default::default());
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Nearest,
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        ..Default::default()
    });

    (view, sampler)
}

/// Phase 3.5: 鍔犺浇 vanilla `map/terrain/atlas0.dds` (2048x2048 BC3, 4脳4 tile grid).
fn load_terrain_atlas(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    path_cfg: &PathConfig,
) -> (wgpu::TextureView, wgpu::Sampler) {
    use hoi4_assets::{AssetDb, DdsImage, FsAssetDb};
    let db = FsAssetDb::new(path_cfg.clone());

    // Try atlas0 (highest LOD); fallback to atlas1 / atlas2 if missing
    let atlas_paths = [
        "map/terrain/atlas0.dds",
        "map/terrain/atlas1.dds",
        "map/terrain/atlas2.dds",
    ];

    let mut loaded: Option<DdsImage> = None;
    let mut chosen_path = "";
    for p in &atlas_paths {
        if let Ok(bytes) = db.open(*p) {
            if let Ok(d) = DdsImage::parse(&bytes) {
                chosen_path = *p;
                loaded = Some(d);
                break;
            }
        }
    }

    let dds = match loaded {
        Some(d) => d,
        None => {
            eprintln!("[terrain] no atlas found, using 1脳1 white fallback");
            // Create 1x1 white texture as fallback
            let tex = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("terrain_atlas_fallback"),
                size: wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &tex,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &[255u8, 255, 255, 255],
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(4),
                    rows_per_image: None,
                },
                wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
            );
            let sampler = device.create_sampler(&wgpu::SamplerDescriptor::default());
            return (tex.create_view(&Default::default()), sampler);
        }
    };

    println!(
        "[terrain] loaded {} ({}x{} {:?} mips={})",
        chosen_path,
        dds.width,
        dds.height,
        dds.format,
        dds.mip_count()
    );

    // Linear color space (terrain atlas was authored as linear, not sRGB)
    let wgpu_fmt = match dds.format {
        hoi4_assets::DdsFormat::Bc1 => wgpu::TextureFormat::Bc1RgbaUnormSrgb,
        hoi4_assets::DdsFormat::Bc3 => wgpu::TextureFormat::Bc3RgbaUnormSrgb,
        hoi4_assets::DdsFormat::Bc5 => wgpu::TextureFormat::Bc5RgUnorm,
        hoi4_assets::DdsFormat::Bgra8 => wgpu::TextureFormat::Bgra8UnormSrgb,
        _ => wgpu::TextureFormat::Bgra8UnormSrgb,
    };

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("terrain_atlas"),
        size: wgpu::Extent3d {
            width: dds.width,
            height: dds.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: dds.mip_count(),
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu_fmt,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });

    for (i, mip) in dds.mips.iter().enumerate() {
        let data = &dds.data[mip.offset..mip.offset + mip.size];
        let block_dim = if matches!(dds.format, hoi4_assets::DdsFormat::Bgra8) {
            1u32
        } else {
            4u32
        };
        let blocks_wide = (mip.width + block_dim - 1) / block_dim;
        let blocks_tall = (mip.height + block_dim - 1) / block_dim;
        let bpb = match dds.format {
            hoi4_assets::DdsFormat::Bc1 => 8u32,
            hoi4_assets::DdsFormat::Bc3 | hoi4_assets::DdsFormat::Bc5 => 16,
            _ => 4,
        };
        // 3.12.4 fix: BC mips need block-aligned copy size for non-pow2 sources.
        let copy_w = blocks_wide * block_dim;
        let copy_h = blocks_tall * block_dim;
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: i as u32,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(blocks_wide * bpb),
                rows_per_image: None,
            },
            wgpu::Extent3d {
                width: copy_w,
                height: copy_h,
                depth_or_array_layers: 1,
            },
        );
    }

    let view = texture.create_view(&Default::default());
    // Phase 3.6.5: 8脳 anisotropic filtering ???terrain remains crisp at oblique
    // viewing angles (camera looking down at the map at low pitch). Requires
    // anisotropy_clamp ???[1, 16]; wgpu validates 8 is supported on baseline tier.
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("terrain_atlas_sampler"),
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Linear,
        address_mode_u: wgpu::AddressMode::Repeat,
        address_mode_v: wgpu::AddressMode::Repeat,
        anisotropy_clamp: 8,
        ..Default::default()
    });

    // The TextureView holds an internal Arc to the texture, so dropping `texture`
    // here is fine - the view keeps the GPU resource alive until the view drops.
    drop(texture);

    (view, sampler)
}

/// Phase 3.6.1: Load vanilla terrain colormap (continent natural color base layer).
/// The colormap is a low-resolution (~5632脳2048 typically) DDS that provides a natural
/// color base for the entire map, eliminating flat-color feel from large terrain areas.
fn load_colormap(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    path_cfg: &PathConfig,
) -> (wgpu::TextureView, wgpu::Sampler) {
    use hoi4_assets::{AssetDb, DdsImage, FsAssetDb, MapResRole};
    let db = FsAssetDb::new(path_cfg.clone());

    // Current HOI4 stores the natural colormap in the RGB channels of the
    // city-emissive mask texture. Keep older/modded names as fallbacks.
    let colormap_emissive = MapResRole::ColormapEmissive.relative_path();
    let colormap_paths = [
        colormap_emissive.as_str(),
        "map/terrain/colormap.dds",
        "map/terrain/colormap_rgb.dds",
    ];

    let mut loaded: Option<DdsImage> = None;
    let mut chosen_path = "";
    for p in &colormap_paths {
        if let Ok(bytes) = db.open(*p) {
            if let Ok(d) = DdsImage::parse(&bytes) {
                chosen_path = *p;
                loaded = Some(d);
                break;
            }
        }
    }

    let dds = match loaded {
        Some(d) => d,
        None => {
            eprintln!("[terrain] no colormap found, using 1脳1 neutral fallback");
            // Create 1x1 neutral gray texture as fallback (won't affect blending much)
            let tex = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("colormap_fallback"),
                size: wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &tex,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &[128u8, 128, 128, 255],
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(4),
                    rows_per_image: None,
                },
                wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
            );
            let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            });
            return (tex.create_view(&Default::default()), sampler);
        }
    };

    println!(
        "[terrain] loaded colormap {} ({}x{} {:?} mips={})",
        chosen_path,
        dds.width,
        dds.height,
        dds.format,
        dds.mip_count()
    );

    // Colormap is sRGB color data
    let wgpu_fmt = match dds.format {
        hoi4_assets::DdsFormat::Bc1 => wgpu::TextureFormat::Bc1RgbaUnormSrgb,
        hoi4_assets::DdsFormat::Bc3 => wgpu::TextureFormat::Bc3RgbaUnormSrgb,
        hoi4_assets::DdsFormat::Bc5 => wgpu::TextureFormat::Bc5RgUnorm,
        hoi4_assets::DdsFormat::Bgra8 => wgpu::TextureFormat::Bgra8UnormSrgb,
        _ => wgpu::TextureFormat::Bgra8UnormSrgb,
    };

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("colormap"),
        size: wgpu::Extent3d {
            width: dds.width,
            height: dds.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: dds.mip_count(),
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu_fmt,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });

    for (i, mip) in dds.mips.iter().enumerate() {
        let data = &dds.data[mip.offset..mip.offset + mip.size];
        let block_dim = if matches!(dds.format, hoi4_assets::DdsFormat::Bgra8) {
            1u32
        } else {
            4u32
        };
        let blocks_wide = (mip.width + block_dim - 1) / block_dim;
        let blocks_tall = (mip.height + block_dim - 1) / block_dim;
        let bpb = match dds.format {
            hoi4_assets::DdsFormat::Bc1 => 8u32,
            hoi4_assets::DdsFormat::Bc3 | hoi4_assets::DdsFormat::Bc5 => 16,
            _ => 4,
        };
        // 3.12.4 fix: BC mips need block-aligned copy size for non-pow2 sources.
        let copy_w = blocks_wide * block_dim;
        let copy_h = blocks_tall * block_dim;
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: i as u32,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(blocks_wide * bpb),
                rows_per_image: None,
            },
            wgpu::Extent3d {
                width: copy_w,
                height: copy_h,
                depth_or_array_layers: 1,
            },
        );
    }

    let view = texture.create_view(&Default::default());
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("colormap_sampler"),
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Linear,
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        ..Default::default()
    });

    drop(texture);

    (view, sampler)
}

/// Phase 3.6.6: Load `map/rivers.bmp` and upload as an `R8Unorm` texture.
/// Each texel = `level / 4 * 255` (so 0 / 64 / 128 / 192 / 255 for levels 0..4).
/// Sampled in shader as f32 in [0, 1] for LOD-gated river overlay.
///
/// Uses `BmpDecoder` from hoi4-map; falls back to a 1脳1 zero texture if missing.
fn load_rivers_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    path_cfg: &PathConfig,
) -> (wgpu::TextureView, wgpu::Sampler) {
    use hoi4_map::rivers::load_rivers_bmp;

    let path = path_cfg.find("map/rivers.bmp");
    let rivers = match path.as_ref().map(|p| load_rivers_bmp(p)) {
        Some(Ok(r)) => r,
        Some(Err(e)) => {
            eprintln!("[rivers] parse error: {} ???using empty fallback", e);
            return rivers_fallback(device, queue);
        }
        None => {
            eprintln!("[rivers] map/rivers.bmp not found ???using empty fallback");
            return rivers_fallback(device, queue);
        }
    };

    let (w, h) = (rivers.width, rivers.height);
    let bytes = rivers.to_r8_normalised();
    println!(
        "[rivers] loaded {}x{}  ?{} river pixels (level???)",
        w,
        h,
        rivers.river_pixel_count()
    );

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("rivers_tex"),
        size: wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::R8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &bytes,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(w),
            rows_per_image: Some(h),
        },
        wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
    );

    let view = texture.create_view(&Default::default());
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("rivers_sampler"),
        // Linear filtering: smoother river width edges. Address mode clamp:
        // out-of-bounds reads return level 0 (no river).
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Nearest,
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        ..Default::default()
    });
    drop(texture);
    (view, sampler)
}

fn rivers_fallback(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> (wgpu::TextureView, wgpu::Sampler) {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("rivers_fallback"),
        size: wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::R8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &tex,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &[0u8],
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(1),
            rows_per_image: Some(1),
        },
        wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
    );
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor::default());
    (tex.create_view(&Default::default()), sampler)
}

fn make_depth_view(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
) -> wgpu::TextureView {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("depth"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    tex.create_view(&Default::default())
}

fn upload_lut(queue: &wgpu::Queue, tex: &wgpu::Texture, data: &[u8], w: u32, h: u32) {
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: tex,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        data,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(w * 4),
            rows_per_image: Some(h),
        },
        wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
    );
}

fn event_modal_sound(event: &hoi4_content::Event) -> UiSound {
    if matches!(event.scope, hoi4_content::EventScope::News) {
        let id = event.id.as_str();
        if id.contains("outbreak")
            || id.contains("declare_war")
            || id.contains("declaration")
            || id.contains("war_begins")
        {
            UiSound::WarDeclaration
        } else {
            UiSound::WorldNews
        }
    } else {
        UiSound::EventPopup
    }
}

fn surrender_notification_sound_key(
    notification: &hoi4_ui::surrender_notification::SurrenderNotification,
) -> String {
    format!(
        "{}:{}:{:?}:{}",
        notification.winner_tag,
        notification.target_tag,
        notification.kind,
        notification.results.len()
    )
}

fn main() {
    let cli = bootstrap::parse_cli();
    if cli.help_requested {
        bootstrap::print_usage();
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
        match map_baseline::write_phase0_report(&path_cfg, &cli.map_phase0_output) {
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
        app_shell::run_map_phase0(world, path_cfg, cli.map_phase0_output);
        return;
    }

    app_shell::run_windowed(world, path_cfg);
}

#[cfg(test)]
mod v6_app_tests {
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
}
