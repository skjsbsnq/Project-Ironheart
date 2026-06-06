use crate::map_baseline::MapLayerMask;
use crate::map_draw::{self, MapDrawInput, MapDrawOutput};
use crate::map_perf::MapQualityPreset;
use crate::passes::PassRegistry;
use crate::render_state::RenderState;

use hoi4_render::map_mode::MapMode;
use hoi4_state::GameDate;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MapRenderSettings {
    pub layer_mask: MapLayerMask,
    pub quality_preset: MapQualityPreset,
}

impl MapRenderSettings {
    pub const fn new(layer_mask: MapLayerMask) -> Self {
        Self {
            layer_mask,
            quality_preset: MapQualityPreset::High,
        }
    }

    pub const fn with_quality(layer_mask: MapLayerMask, quality_preset: MapQualityPreset) -> Self {
        Self {
            layer_mask,
            quality_preset,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub struct MapFrameContext {
    pub draw_3d_map: bool,
    pub map_mode: MapMode,
    pub date: GameDate,
    pub selected_province_id: u32,
    pub hovered_province_id: u32,
    pub zoom_factor: f32,
    pub time_seconds: f32,
    pub screen_size: [f32; 2],
    pub settings: MapRenderSettings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapRenderPass {
    ShadowCaster,
    ProjectedFowShadow,
    Sky,
    Terrain,
    BorderFirst,
    River,
    WaterRefraction,
    MapLayers,
    Water,
    BorderSecond,
    TradeRoutes,
    Straits,
    Hoi3Counters,
    Trees,
    Railways,
    Buildings,
    PoiIcons,
    MapNames,
    ProvinceNames,
    MapArrows,
    Frontlines,
    Particles,
    Postprocess,
    Ui,
}

impl MapRenderPass {
    pub const PHASE1_ORDER: [Self; 24] = [
        Self::ShadowCaster,
        Self::ProjectedFowShadow,
        Self::Sky,
        Self::Terrain,
        Self::BorderFirst,
        Self::River,
        Self::WaterRefraction,
        Self::MapLayers,
        Self::Water,
        Self::BorderSecond,
        Self::TradeRoutes,
        Self::Straits,
        Self::Railways,
        Self::Hoi3Counters,
        Self::Trees,
        Self::Buildings,
        Self::PoiIcons,
        Self::MapNames,
        Self::ProvinceNames,
        Self::MapArrows,
        Self::Frontlines,
        Self::Particles,
        Self::Postprocess,
        Self::Ui,
    ];

    pub const fn registry_name(self) -> &'static str {
        match self {
            Self::ShadowCaster => "shadow_caster",
            Self::ProjectedFowShadow => "projected_fow_shadow",
            Self::Sky => "3d_sky",
            Self::Terrain => "3d_terrain",
            Self::BorderFirst => "3d_border_first",
            Self::River => "3d_river",
            Self::WaterRefraction => "water_refraction",
            Self::MapLayers => "3d_map_layers",
            Self::Water => "3d_water",
            Self::BorderSecond => "3d_border_second",
            Self::TradeRoutes => "3d_traderoute",
            Self::Straits => "3d_strait",
            Self::Hoi3Counters => "hoi3_counter_v3",
            Self::Trees => "3d_trees",
            Self::Railways => "3d_railways",
            Self::Buildings => "3d_buildings",
            Self::PoiIcons => "3d_poi_icons",
            Self::MapNames => "3d_mapname",
            Self::ProvinceNames => "3d_province_name",
            Self::MapArrows => "3d_maparrow",
            Self::Frontlines => "3d_frontlines",
            Self::Particles => "3d_particles",
            Self::Postprocess => "postprocess",
            Self::Ui => "ui",
        }
    }

    fn enabled_by_mask(self, mask: MapLayerMask) -> bool {
        match self {
            Self::ShadowCaster => mask.terrain || mask.objects,
            Self::ProjectedFowShadow => mask.terrain || mask.water || mask.river || mask.objects,
            Self::Sky => mask.sky,
            Self::Terrain => mask.terrain,
            Self::BorderFirst | Self::BorderSecond => mask.borders,
            Self::River => mask.river,
            Self::WaterRefraction => mask.water,
            Self::MapLayers => mask.terrain,
            Self::Water => mask.water,
            Self::TradeRoutes | Self::Straits => mask.static_decals || mask.overlays,
            Self::Hoi3Counters => mask.objects || mask.overlays,
            Self::Trees | Self::Buildings | Self::PoiIcons => mask.objects,
            Self::Railways => mask.static_decals,
            Self::MapNames | Self::ProvinceNames => mask.labels,
            Self::MapArrows | Self::Frontlines => mask.overlays,
            Self::Particles => mask.particles,
            Self::Postprocess => mask.postprocess,
            Self::Ui => mask.ui,
        }
    }

    const fn requires_3d_map(self) -> bool {
        !matches!(self, Self::Ui)
    }

    fn allowed_by_static_decal_plan(self, plan: StaticMapDecalPlan) -> bool {
        match self {
            Self::Railways => plan.railways.visible,
            _ => true,
        }
    }

    fn allowed_by_semantic_overlay_plan(self, plan: SemanticOverlayPlan) -> bool {
        match self {
            Self::TradeRoutes => plan.trade_routes.visible,
            Self::Straits => plan.straits.visible,
            Self::MapArrows => plan.arrows.visible || plan.frontlines.visible,
            Self::Frontlines => plan.frontlines.visible,
            _ => true,
        }
    }

    fn allowed_by_world_object_plan(self, plan: WorldObjectPlan) -> bool {
        match self {
            Self::Hoi3Counters => plan.counters.visible,
            Self::Trees => plan.trees.visible,
            Self::Buildings => plan.buildings.visible,
            Self::PoiIcons => plan.poi_icons.visible,
            Self::MapNames => plan.country_names.visible,
            Self::ProvinceNames => plan.province_names.visible,
            _ => true,
        }
    }

    fn allowed_by_quality(self, preset: MapQualityPreset) -> bool {
        let controls = preset.controls();
        match self {
            Self::ShadowCaster | Self::ProjectedFowShadow | Self::Particles => {
                !matches!(preset, MapQualityPreset::LowEnd)
            }
            Self::Postprocess => controls.postprocess_chain,
            Self::WaterRefraction => preset.water_refraction_enabled(),
            _ => true,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MapRenderGraph {
    passes: &'static [MapRenderPass],
}

impl MapRenderGraph {
    pub fn phase1() -> Self {
        Self {
            passes: &MapRenderPass::PHASE1_ORDER,
        }
    }

    pub fn passes(&self) -> &[MapRenderPass] {
        &self.passes
    }
}

#[derive(Debug, Clone, Default)]
pub struct MapPassDrawSet {
    pub shadow_caster: bool,
    pub projected_fow_shadow: bool,
    pub sky: bool,
    pub terrain: bool,
    pub border_first: bool,
    pub river: bool,
    pub water_refraction: bool,
    pub map_layers: bool,
    pub water: bool,
    pub border_second: bool,
    pub borders: bool,
    pub trade_routes: bool,
    pub straits: bool,
    pub hoi3_counters: bool,
    pub trees: bool,
    pub railways: bool,
    pub buildings: bool,
    pub poi_icons: bool,
    pub map_names: bool,
    pub province_names: bool,
    pub map_arrows: bool,
    pub frontlines: bool,
    pub particles: bool,
    pub postprocess: bool,
    pub ui: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerrainMaterialOwnership {
    pub terrain_water_final_color: bool,
    pub terrain_sdf_borders: bool,
    pub terrain_overlays: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WaterMaterialOwnership {
    pub final_color: bool,
    pub shallow_deep_transition: bool,
    pub coast_foam: bool,
    pub normal: bool,
    pub specular: bool,
    pub ice: bool,
    pub reflection: bool,
    pub sea_selection_highlight: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BorderMaterialOwnership {
    pub final_borders: bool,
    pub country_borders: bool,
    pub state_borders: bool,
    pub province_borders: bool,
    pub sea_borders: bool,
    pub impassable_borders: bool,
    pub selected_borders: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MapPassFallbackReport {
    pub terrain_water_fallback: bool,
    pub terrain_border_fallback: bool,
    pub terrain_overlay_fallback: bool,
    pub water_degraded: bool,
    pub borders_degraded: bool,
    pub water_invalid: bool,
    pub borders_invalid: bool,
}

impl MapPassFallbackReport {
    pub fn fallback_count(self) -> u32 {
        [
            self.terrain_water_fallback,
            self.terrain_border_fallback,
            self.terrain_overlay_fallback,
            self.water_degraded,
            self.borders_degraded,
            self.water_invalid,
            self.borders_invalid,
        ]
        .into_iter()
        .filter(|enabled| *enabled)
        .count() as u32
    }

    pub fn is_degraded(self) -> bool {
        self.water_degraded || self.borders_degraded || self.water_invalid || self.borders_invalid
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapPrepareFrameInput {
    pub layer_mask: MapLayerMask,
    pub dedicated_water_loaded: bool,
    pub dedicated_river_loaded: bool,
    pub dedicated_border_loaded: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapPreparedFrame {
    pub terrain_ownership: TerrainMaterialOwnership,
    pub water_ownership: WaterMaterialOwnership,
    pub border_ownership: BorderMaterialOwnership,
    pub fallback_report: MapPassFallbackReport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum StaticMapDecalKind {
    TerrainRivers,
    Roads,
    Railways,
    ShoreAccents,
    ImpassableMarks,
    InfrastructureOverlay,
    TradeRoutes,
    Straits,
}

impl StaticMapDecalKind {
    #[cfg(test)]
    pub const ALL: [Self; 8] = [
        Self::TerrainRivers,
        Self::Roads,
        Self::Railways,
        Self::ShoreAccents,
        Self::ImpassableMarks,
        Self::InfrastructureOverlay,
        Self::TradeRoutes,
        Self::Straits,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum SemanticOverlayKind {
    OccupationStripes,
    Frontlines,
    Arrows,
    TradeRoutes,
    Straits,
    SelectedProvincePulse,
    HoverHighlight,
    MapModeOverlay,
}

impl SemanticOverlayKind {
    #[cfg(test)]
    pub const ALL: [Self; 8] = [
        Self::OccupationStripes,
        Self::Frontlines,
        Self::Arrows,
        Self::TradeRoutes,
        Self::Straits,
        Self::SelectedProvincePulse,
        Self::HoverHighlight,
        Self::MapModeOverlay,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StaticMapDecalDecision {
    pub visible: bool,
    pub opacity: f32,
}

impl StaticMapDecalDecision {
    pub const OFF: Self = Self {
        visible: false,
        opacity: 0.0,
    };

    pub fn visible(opacity: f32) -> Self {
        Self {
            visible: opacity > 0.01,
            opacity: opacity.clamp(0.0, 1.0),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SemanticOverlayDecision {
    pub visible: bool,
    pub opacity: f32,
    pub priority: u8,
}

impl SemanticOverlayDecision {
    pub const OFF: Self = Self {
        visible: false,
        opacity: 0.0,
        priority: 0,
    };

    pub fn visible(opacity: f32, priority: u8) -> Self {
        Self {
            visible: opacity > 0.01,
            opacity: opacity.clamp(0.0, 1.0),
            priority,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SemanticOverlayBudget {
    pub passive: f32,
    pub active: f32,
    pub map_mode: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SemanticOverlayPlan {
    pub occupation_stripes: SemanticOverlayDecision,
    pub frontlines: SemanticOverlayDecision,
    pub arrows: SemanticOverlayDecision,
    pub trade_routes: SemanticOverlayDecision,
    pub straits: SemanticOverlayDecision,
    pub selected_province_pulse: SemanticOverlayDecision,
    pub hover_highlight: SemanticOverlayDecision,
    pub map_mode_overlay: SemanticOverlayDecision,
    pub hovered_province_id: u32,
    pub budget: SemanticOverlayBudget,
}

impl Default for SemanticOverlayPlan {
    fn default() -> Self {
        Self {
            occupation_stripes: SemanticOverlayDecision::OFF,
            frontlines: SemanticOverlayDecision::OFF,
            arrows: SemanticOverlayDecision::OFF,
            trade_routes: SemanticOverlayDecision::OFF,
            straits: SemanticOverlayDecision::OFF,
            selected_province_pulse: SemanticOverlayDecision::OFF,
            hover_highlight: SemanticOverlayDecision::OFF,
            map_mode_overlay: SemanticOverlayDecision::OFF,
            hovered_province_id: u32::MAX,
            budget: SemanticOverlayBudget {
                passive: 0.0,
                active: 0.0,
                map_mode: 0.0,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum WorldObjectKind {
    CountryNames,
    ProvinceNames,
    Counters,
    PoiIcons,
    Buildings,
    Trees,
}

impl WorldObjectKind {
    #[cfg(test)]
    pub const ALL: [Self; 6] = [
        Self::CountryNames,
        Self::ProvinceNames,
        Self::Counters,
        Self::PoiIcons,
        Self::Buildings,
        Self::Trees,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorldObjectDecision {
    pub visible: bool,
    pub opacity: f32,
    pub scale: f32,
    pub priority: u8,
}

impl WorldObjectDecision {
    pub const OFF: Self = Self {
        visible: false,
        opacity: 0.0,
        scale: 1.0,
        priority: 0,
    };

    pub fn visible(opacity: f32, scale: f32, priority: u8) -> Self {
        Self {
            visible: opacity > 0.01 && scale > 0.01,
            opacity: opacity.clamp(0.0, 1.0),
            scale: scale.clamp(0.25, 1.5),
            priority,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorldObjectBudget {
    pub labels: f32,
    pub counters: f32,
    pub objects: f32,
}

const COUNTER_STRATEGIC_HIDE_ZOOM: f32 = 0.30;
const COUNTER_STRATEGIC_FULL_ZOOM: f32 = 0.78;
const COUNTER_SELECTED_FAR_OPACITY: f32 = 0.44;
const COUNTER_SELECTED_FAR_SCALE: f32 = 0.72;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorldObjectPlan {
    pub country_names: WorldObjectDecision,
    pub province_names: WorldObjectDecision,
    pub counters: WorldObjectDecision,
    pub poi_icons: WorldObjectDecision,
    pub buildings: WorldObjectDecision,
    pub trees: WorldObjectDecision,
    pub province_name_min_pixels: u32,
    pub poi_detail_level: u8,
    pub counter_layout_density: f32,
    pub counter_selected_only: bool,
    pub budget: WorldObjectBudget,
}

impl Default for WorldObjectPlan {
    fn default() -> Self {
        Self {
            country_names: WorldObjectDecision::OFF,
            province_names: WorldObjectDecision::OFF,
            counters: WorldObjectDecision::OFF,
            poi_icons: WorldObjectDecision::OFF,
            buildings: WorldObjectDecision::OFF,
            trees: WorldObjectDecision::OFF,
            province_name_min_pixels: u32::MAX,
            poi_detail_level: 0,
            counter_layout_density: 0.0,
            counter_selected_only: false,
            budget: WorldObjectBudget {
                labels: 0.0,
                counters: 0.0,
                objects: 0.0,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StaticMapDecalPlan {
    pub terrain_rivers: StaticMapDecalDecision,
    pub roads: StaticMapDecalDecision,
    pub railways: StaticMapDecalDecision,
    pub shore_accents: StaticMapDecalDecision,
    pub impassable_marks: StaticMapDecalDecision,
    pub infrastructure_overlay: StaticMapDecalDecision,
    pub trade_routes: StaticMapDecalDecision,
    pub straits: StaticMapDecalDecision,
}

impl Default for StaticMapDecalPlan {
    fn default() -> Self {
        Self {
            terrain_rivers: StaticMapDecalDecision::OFF,
            roads: StaticMapDecalDecision::OFF,
            railways: StaticMapDecalDecision::OFF,
            shore_accents: StaticMapDecalDecision::OFF,
            impassable_marks: StaticMapDecalDecision::OFF,
            infrastructure_overlay: StaticMapDecalDecision::OFF,
            trade_routes: StaticMapDecalDecision::OFF,
            straits: StaticMapDecalDecision::OFF,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct StaticMapDecalSystem;

impl StaticMapDecalSystem {
    pub fn plan(context: MapFrameContext) -> StaticMapDecalPlan {
        if !context.draw_3d_map {
            return StaticMapDecalPlan::default();
        }

        let mask = context.settings.layer_mask;
        let zoom = context.zoom_factor.clamp(0.0, 1.0);
        let static_enabled = mask.static_decals;
        let overlay_enabled = mask.overlays;
        let infrastructure_mode =
            matches!(context.map_mode, MapMode::Infrastructure | MapMode::Supply);

        let mut plan = StaticMapDecalPlan::default();
        if static_enabled {
            plan.terrain_rivers = StaticMapDecalDecision::visible(1.0);
            plan.shore_accents =
                StaticMapDecalDecision::visible(0.30 * smoothstep(0.45, 0.85, zoom));
            plan.impassable_marks =
                StaticMapDecalDecision::visible(0.35 * smoothstep(0.55, 0.85, zoom));
        }
        if static_enabled && (infrastructure_mode || context.map_mode == MapMode::Factories) {
            let zoom_alpha = smoothstep(0.32, 0.70, zoom) * 0.62;
            let mode_floor: f32 = match context.map_mode {
                MapMode::Infrastructure | MapMode::Supply => 0.76,
                MapMode::Factories => 0.58,
                MapMode::Terrain => 0.24,
                _ => 0.36,
            };
            let rail_alpha = zoom_alpha.max(mode_floor).min(0.78);
            plan.railways = StaticMapDecalDecision::visible(rail_alpha);
        }

        if static_enabled && infrastructure_mode {
            plan.infrastructure_overlay = StaticMapDecalDecision::visible(0.65);
        }

        let network_line_mode =
            matches!(context.map_mode, MapMode::Supply | MapMode::Infrastructure);
        if network_line_mode && (static_enabled || overlay_enabled) && zoom >= 0.26 {
            plan.trade_routes =
                StaticMapDecalDecision::visible(0.42 * smoothstep(0.24, 0.62, zoom));
        }
        if network_line_mode && (static_enabled || overlay_enabled) && zoom >= 0.18 {
            plan.straits = StaticMapDecalDecision::visible(0.55 * smoothstep(0.18, 0.55, zoom));
        }

        plan
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SemanticOverlaySystem;

impl SemanticOverlaySystem {
    pub fn plan(
        context: MapFrameContext,
        static_decals: StaticMapDecalPlan,
    ) -> SemanticOverlayPlan {
        if !context.draw_3d_map || !context.settings.layer_mask.overlays {
            return SemanticOverlayPlan::default();
        }

        let zoom = context.zoom_factor.clamp(0.0, 1.0);
        let quality = context.settings.quality_preset.controls();
        let mut budget = overlay_budget_for_mode(context.map_mode);
        budget.passive *= quality.label_density;
        budget.active *= quality.label_density;
        budget.map_mode *= quality.label_density;
        let strategic = 1.0 - smoothstep(0.72, 0.95, zoom);
        let mid_zoom = smoothstep(0.16, 0.46, zoom);
        let interaction_zoom = smoothstep(0.10, 0.28, zoom);
        let has_selection = context.selected_province_id != u32::MAX;
        let has_hover = context.hovered_province_id != u32::MAX
            && context.hovered_province_id != context.selected_province_id;

        let mut plan = SemanticOverlayPlan {
            hovered_province_id: context.hovered_province_id,
            budget,
            ..SemanticOverlayPlan::default()
        };

        plan.frontlines = SemanticOverlayDecision::visible(
            budget.active * mix(0.58, 0.92, strategic) * mid_zoom,
            70,
        );
        plan.arrows =
            SemanticOverlayDecision::visible(budget.active * mix(0.74, 1.0, interaction_zoom), 80);
        plan.trade_routes = SemanticOverlayDecision::visible(
            static_decals.trade_routes.opacity * budget.passive,
            35,
        );
        plan.straits = SemanticOverlayDecision::visible(
            static_decals.straits.opacity * budget.passive.max(0.45),
            45,
        );
        if has_selection {
            plan.selected_province_pulse =
                SemanticOverlayDecision::visible(0.82 * interaction_zoom.max(0.35), 100);
        }
        if has_hover {
            plan.hover_highlight = SemanticOverlayDecision::visible(0.36 * interaction_zoom, 95);
        }
        plan.map_mode_overlay = SemanticOverlayDecision::visible(
            budget.map_mode * map_mode_overlay_factor(context.map_mode, zoom),
            30,
        );

        plan
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct WorldObjectSystem;

impl WorldObjectSystem {
    pub fn plan(context: MapFrameContext) -> WorldObjectPlan {
        if !context.draw_3d_map {
            return WorldObjectPlan::default();
        }

        let mask = context.settings.layer_mask;
        let zoom = context.zoom_factor.clamp(0.0, 1.0);
        let quality = context.settings.quality_preset.controls();
        let mut budget = world_object_budget_for_mode(context.map_mode);
        budget.labels *= quality.label_density;
        budget.counters *= quality.label_density;
        budget.objects *= quality.tree_density.max(quality.border_detail).min(1.0);
        let strategic = 1.0 - smoothstep(0.60, 0.90, zoom);
        let close = smoothstep(0.56, 0.86, zoom);
        let very_close = smoothstep(0.78, 0.96, zoom);
        let province_label_zoom = smoothstep(0.86, 0.98, zoom);

        let mut plan = WorldObjectPlan {
            budget,
            ..WorldObjectPlan::default()
        };

        if mask.labels {
            let country_opacity =
                budget.labels * mix(0.62, 0.78, strategic) * mix(0.92, 0.70, close);
            let country_scale = mix(0.92, 0.74, close);
            plan.country_names = WorldObjectDecision::visible(country_opacity, country_scale, 60);

            let province_opacity = budget.labels * province_label_zoom * 0.58;
            let province_min_pixels = if zoom < 0.86 {
                80_000
            } else if zoom < 0.94 {
                45_000
            } else {
                18_000
            };
            let province_min_pixels =
                ((province_min_pixels as f32) / quality.label_density.max(0.25)).round() as u32;
            plan.province_names = WorldObjectDecision::visible(
                province_opacity,
                mix(0.86, 1.0, province_label_zoom),
                50,
            );
            plan.province_name_min_pixels = province_min_pixels;
        }

        if mask.objects || mask.overlays {
            let has_selection = context.selected_province_id != u32::MAX;
            let counter_visibility = smoothstep(
                COUNTER_STRATEGIC_HIDE_ZOOM,
                COUNTER_STRATEGIC_FULL_ZOOM,
                zoom,
            );
            let counter_base = if counter_visibility > 0.0 {
                budget.counters * mix(0.16, 1.0, counter_visibility)
            } else if has_selection {
                budget.counters * COUNTER_SELECTED_FAR_OPACITY
            } else {
                0.0
            };
            let counter_scale = if counter_visibility > 0.0 {
                mix(0.54, 0.94, close)
            } else {
                COUNTER_SELECTED_FAR_SCALE
            };
            plan.counters = WorldObjectDecision::visible(counter_base, counter_scale, 90);
            plan.counter_layout_density = mix(0.20, 0.82, counter_visibility);
            plan.counter_selected_only = counter_visibility <= 0.0 && has_selection;
        }

        if mask.objects {
            plan.poi_icons = WorldObjectDecision::OFF;
            plan.poi_detail_level = 0;
            plan.buildings = WorldObjectDecision::OFF;

            let tree_noise_gate = smoothstep(0.24, 0.58, zoom);
            let tree_close_quality = 1.0 - (very_close * 0.12);
            plan.trees = WorldObjectDecision::visible(
                budget.objects * tree_noise_gate * tree_close_quality,
                mix(0.78, 1.0, close),
                35,
            );
        }

        plan
    }
}

impl MapPassDrawSet {
    fn set(&mut self, pass: MapRenderPass, enabled: bool) {
        match pass {
            MapRenderPass::ShadowCaster => self.shadow_caster = enabled,
            MapRenderPass::ProjectedFowShadow => self.projected_fow_shadow = enabled,
            MapRenderPass::Sky => self.sky = enabled,
            MapRenderPass::Terrain => self.terrain = enabled,
            MapRenderPass::BorderFirst => {
                self.border_first = enabled;
                self.borders = self.border_first || self.border_second;
            }
            MapRenderPass::River => self.river = enabled,
            MapRenderPass::WaterRefraction => self.water_refraction = enabled,
            MapRenderPass::MapLayers => self.map_layers = enabled,
            MapRenderPass::Water => self.water = enabled,
            MapRenderPass::BorderSecond => {
                self.border_second = enabled;
                self.borders = self.border_first || self.border_second;
            }
            MapRenderPass::TradeRoutes => self.trade_routes = enabled,
            MapRenderPass::Straits => self.straits = enabled,
            MapRenderPass::Hoi3Counters => self.hoi3_counters = enabled,
            MapRenderPass::Trees => self.trees = enabled,
            MapRenderPass::Railways => self.railways = enabled,
            MapRenderPass::Buildings => self.buildings = enabled,
            MapRenderPass::PoiIcons => self.poi_icons = enabled,
            MapRenderPass::MapNames => self.map_names = enabled,
            MapRenderPass::ProvinceNames => self.province_names = enabled,
            MapRenderPass::MapArrows => self.map_arrows = enabled,
            MapRenderPass::Frontlines => self.frontlines = enabled,
            MapRenderPass::Particles => self.particles = enabled,
            MapRenderPass::Postprocess => self.postprocess = enabled,
            MapRenderPass::Ui => self.ui = enabled,
        }
    }

    pub fn terrain_material_ownership(
        &self,
        mask: MapLayerMask,
        dedicated_water_loaded: bool,
        dedicated_river_loaded: bool,
        dedicated_border_loaded: bool,
        static_decals: StaticMapDecalPlan,
        semantic_overlays: SemanticOverlayPlan,
    ) -> TerrainMaterialOwnership {
        let dedicated_water_active = self.water && dedicated_water_loaded;
        let dedicated_river_active = self.river && dedicated_river_loaded;
        let dedicated_border_active = self.borders && dedicated_border_loaded;
        let terrain_rivers_fallback =
            static_decals.terrain_rivers.visible && !dedicated_river_active;
        let terrain_semantic_overlays = mask.overlays
            && (semantic_overlays.selected_province_pulse.visible
                || semantic_overlays.hover_highlight.visible
                || semantic_overlays.map_mode_overlay.visible);
        TerrainMaterialOwnership {
            terrain_water_final_color: self.water && !dedicated_water_active,
            terrain_sdf_borders: self.borders && !dedicated_border_active,
            terrain_overlays: terrain_rivers_fallback || terrain_semantic_overlays,
        }
    }

    pub fn water_material_ownership(&self, dedicated_water_loaded: bool) -> WaterMaterialOwnership {
        let owns_water_material = self.water && dedicated_water_loaded;
        WaterMaterialOwnership {
            final_color: owns_water_material,
            shallow_deep_transition: owns_water_material,
            coast_foam: owns_water_material,
            normal: owns_water_material,
            specular: owns_water_material,
            ice: owns_water_material,
            reflection: owns_water_material,
            sea_selection_highlight: owns_water_material,
        }
    }

    pub fn border_material_ownership(
        &self,
        dedicated_border_loaded: bool,
    ) -> BorderMaterialOwnership {
        let owns_borders = self.borders && dedicated_border_loaded;
        BorderMaterialOwnership {
            final_borders: owns_borders,
            country_borders: owns_borders,
            state_borders: owns_borders,
            province_borders: owns_borders,
            sea_borders: owns_borders,
            impassable_borders: owns_borders,
            selected_borders: owns_borders,
        }
    }
}

#[derive(Clone)]
pub struct MapFramePlan {
    pub draw: MapPassDrawSet,
    pub static_decals: StaticMapDecalPlan,
    pub semantic_overlays: SemanticOverlayPlan,
    pub world_objects: WorldObjectPlan,
}

#[derive(Debug, Clone)]
pub struct MapRenderer {
    graph: MapRenderGraph,
}

impl Default for MapRenderer {
    fn default() -> Self {
        Self {
            graph: MapRenderGraph::phase1(),
        }
    }
}

impl MapRenderer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_passes(&self, registry: &mut PassRegistry) {
        for pass in self.graph.passes() {
            registry.register(pass.registry_name());
        }
        // Keep the old frontline mesh disabled by default; map arrows remain
        // the stable battleplan/frontline overlay path.
        registry.set_enabled(MapRenderPass::Frontlines.registry_name(), false);
    }

    pub fn build_frame_plan(
        &self,
        context: MapFrameContext,
        registry: &PassRegistry,
    ) -> MapFramePlan {
        let mut draw = MapPassDrawSet::default();
        let mask = context.settings.layer_mask;
        let static_decals = StaticMapDecalSystem::plan(context);
        let semantic_overlays = SemanticOverlaySystem::plan(context, static_decals);
        let world_objects = WorldObjectSystem::plan(context);
        for pass in self.graph.passes() {
            let enabled = (!pass.requires_3d_map() || context.draw_3d_map)
                && pass.enabled_by_mask(mask)
                && pass.allowed_by_static_decal_plan(static_decals)
                && pass.allowed_by_semantic_overlay_plan(semantic_overlays)
                && pass.allowed_by_world_object_plan(world_objects)
                && pass.allowed_by_quality(context.settings.quality_preset)
                && registry.is_enabled(pass.registry_name());
            draw.set(*pass, enabled);
        }
        MapFramePlan {
            draw,
            static_decals,
            semantic_overlays,
            world_objects,
        }
    }

    pub fn prepare_frame(
        &self,
        plan: &MapFramePlan,
        input: MapPrepareFrameInput,
    ) -> MapPreparedFrame {
        let terrain_ownership = plan.draw.terrain_material_ownership(
            input.layer_mask,
            input.dedicated_water_loaded,
            input.dedicated_river_loaded,
            input.dedicated_border_loaded,
            plan.static_decals,
            plan.semantic_overlays,
        );
        let water_ownership = plan
            .draw
            .water_material_ownership(input.dedicated_water_loaded);
        let border_ownership = plan
            .draw
            .border_material_ownership(input.dedicated_border_loaded);
        let fallback_report = MapPassFallbackReport {
            terrain_water_fallback: terrain_ownership.terrain_water_final_color,
            terrain_border_fallback: terrain_ownership.terrain_sdf_borders,
            terrain_overlay_fallback: terrain_ownership.terrain_overlays,
            water_degraded: plan.draw.water && !water_ownership.final_color,
            borders_degraded: plan.draw.borders && !border_ownership.final_borders,
            water_invalid: input.layer_mask.water
                && !plan.draw.water
                && !terrain_ownership.terrain_water_final_color,
            borders_invalid: input.layer_mask.borders
                && !plan.draw.borders
                && !terrain_ownership.terrain_sdf_borders,
        };
        MapPreparedFrame {
            terrain_ownership,
            water_ownership,
            border_ownership,
            fallback_report,
        }
    }

    pub(crate) fn render_frame(
        &self,
        s: &mut RenderState,
        enc: &mut wgpu::CommandEncoder,
        input: MapDrawInput<'_>,
    ) -> MapDrawOutput {
        map_draw::render_map_frame(s, enc, input)
    }
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let denom = (edge1 - edge0).abs().max(f32::EPSILON);
    let t = ((x - edge0) / denom).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn mix(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

fn overlay_budget_for_mode(mode: MapMode) -> SemanticOverlayBudget {
    match mode {
        MapMode::Political => SemanticOverlayBudget {
            passive: 0.72,
            active: 0.92,
            map_mode: 0.18,
        },
        MapMode::Terrain => SemanticOverlayBudget {
            passive: 0.48,
            active: 0.78,
            map_mode: 0.08,
        },
        MapMode::Supply | MapMode::Infrastructure => SemanticOverlayBudget {
            passive: 0.62,
            active: 0.88,
            map_mode: 0.58,
        },
        MapMode::Factories | MapMode::Manpower | MapMode::Resistance => SemanticOverlayBudget {
            passive: 0.66,
            active: 0.86,
            map_mode: 0.44,
        },
        MapMode::Cores | MapMode::Ideology => SemanticOverlayBudget {
            passive: 0.70,
            active: 0.88,
            map_mode: 0.34,
        },
    }
}

fn map_mode_overlay_factor(mode: MapMode, zoom: f32) -> f32 {
    match mode {
        MapMode::Political | MapMode::Terrain => 0.0,
        MapMode::Supply | MapMode::Infrastructure => mix(0.48, 0.92, smoothstep(0.20, 0.72, zoom)),
        MapMode::Factories | MapMode::Manpower | MapMode::Resistance => {
            mix(0.32, 0.72, smoothstep(0.28, 0.76, zoom))
        }
        MapMode::Cores | MapMode::Ideology => mix(0.24, 0.56, smoothstep(0.24, 0.70, zoom)),
    }
}

fn world_object_budget_for_mode(mode: MapMode) -> WorldObjectBudget {
    match mode {
        MapMode::Political => WorldObjectBudget {
            labels: 1.0,
            counters: 1.0,
            objects: 0.72,
        },
        MapMode::Terrain => WorldObjectBudget {
            labels: 0.78,
            counters: 0.92,
            objects: 0.54,
        },
        MapMode::Supply | MapMode::Infrastructure => WorldObjectBudget {
            labels: 0.84,
            counters: 1.0,
            objects: 0.92,
        },
        MapMode::Factories | MapMode::Manpower | MapMode::Resistance => WorldObjectBudget {
            labels: 0.86,
            counters: 0.96,
            objects: 1.0,
        },
        MapMode::Cores | MapMode::Ideology => WorldObjectBudget {
            labels: 0.92,
            counters: 0.96,
            objects: 0.70,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_context(mask: MapLayerMask) -> MapFrameContext {
        MapFrameContext {
            draw_3d_map: true,
            map_mode: MapMode::Political,
            date: GameDate::START,
            selected_province_id: u32::MAX,
            hovered_province_id: u32::MAX,
            zoom_factor: 0.5,
            time_seconds: 1.0,
            screen_size: [1920.0, 1080.0],
            settings: MapRenderSettings::new(mask),
        }
    }

    fn water_border_mask() -> MapLayerMask {
        MapLayerMask {
            terrain: true,
            water: true,
            borders: true,
            ..MapLayerMask::none()
        }
    }

    #[test]
    fn phase1_graph_owns_pass_order() {
        let renderer = MapRenderer::new();
        let names: Vec<_> = renderer
            .graph
            .passes()
            .iter()
            .map(|pass| pass.registry_name())
            .collect();
        assert_eq!(
            names,
            vec![
                "shadow_caster",
                "projected_fow_shadow",
                "3d_sky",
                "3d_terrain",
                "3d_border_first",
                "3d_river",
                "water_refraction",
                "3d_map_layers",
                "3d_water",
                "3d_border_second",
                "3d_traderoute",
                "3d_strait",
                "3d_railways",
                "hoi3_counter_v3",
                "3d_trees",
                "3d_buildings",
                "3d_poi_icons",
                "3d_mapname",
                "3d_province_name",
                "3d_maparrow",
                "3d_frontlines",
                "3d_particles",
                "postprocess",
                "ui",
            ]
        );
    }

    #[test]
    fn registry_is_seeded_from_graph() {
        let renderer = MapRenderer::new();
        let mut registry = PassRegistry::new();
        renderer.register_passes(&mut registry);
        assert_eq!(registry.entries.len(), renderer.graph.passes().len());
        assert!(registry.is_enabled("3d_terrain"));
        assert!(!registry.is_enabled("3d_frontlines"));
    }

    #[test]
    fn vanilla_core_passes_keep_p1_order() {
        let names: Vec<_> = MapRenderPass::PHASE1_ORDER
            .iter()
            .map(|pass| pass.registry_name())
            .collect();
        let projected_fow_shadow = names
            .iter()
            .position(|name| *name == "projected_fow_shadow")
            .unwrap();
        let terrain = names.iter().position(|name| *name == "3d_terrain").unwrap();
        let border_first = names
            .iter()
            .position(|name| *name == "3d_border_first")
            .unwrap();
        let water = names.iter().position(|name| *name == "3d_water").unwrap();
        let river = names.iter().position(|name| *name == "3d_river").unwrap();
        let water_refraction = names
            .iter()
            .position(|name| *name == "water_refraction")
            .unwrap();
        let map_layers = names
            .iter()
            .position(|name| *name == "3d_map_layers")
            .unwrap();
        let border_second = names
            .iter()
            .position(|name| *name == "3d_border_second")
            .unwrap();
        let postprocess = names
            .iter()
            .position(|name| *name == "postprocess")
            .unwrap();
        let ui = names.iter().position(|name| *name == "ui").unwrap();
        assert!(projected_fow_shadow < terrain);
        assert!(terrain < border_first);
        assert!(border_first < river);
        assert!(river < water_refraction);
        assert!(water_refraction < water);
        assert!(river < map_layers);
        assert!(map_layers < water);
        assert!(water < border_second);
        assert!(border_second < postprocess);
        assert!(postprocess < ui);
    }

    #[test]
    fn layer_masks_drive_frame_plan() {
        let renderer = MapRenderer::new();
        let mut registry = PassRegistry::new();
        renderer.register_passes(&mut registry);

        let terrain = renderer.build_frame_plan(
            test_context(MapLayerMask::for_layer(
                crate::map_baseline::MapBaselineLayer::TerrainOnly,
            )),
            &registry,
        );
        assert!(terrain.draw.terrain);
        assert!(terrain.draw.shadow_caster);
        assert!(terrain.draw.projected_fow_shadow);
        assert!(!terrain.draw.water);
        assert!(!terrain.draw.borders);
        assert!(!terrain.draw.postprocess);

        let hdr_raw = renderer.build_frame_plan(
            test_context(MapLayerMask::for_layer(
                crate::map_baseline::MapBaselineLayer::HdrRaw,
            )),
            &registry,
        );
        assert!(hdr_raw.draw.terrain);
        assert!(hdr_raw.draw.postprocess);

        let full = renderer.build_frame_plan(test_context(MapLayerMask::all()), &registry);
        assert!(full.draw.terrain);
        assert!(full.draw.projected_fow_shadow);
        assert!(full.draw.water);
        assert!(full.draw.river);
        assert!(full.draw.map_arrows);
        assert!(full.draw.postprocess);
        assert!(!full.draw.buildings);
        assert!(!full.draw.poi_icons);
        assert!(!full.draw.frontlines);
    }

    #[test]
    fn static_decal_plan_keeps_railways_to_network_modes() {
        let renderer = MapRenderer::new();
        let mut registry = PassRegistry::new();
        renderer.register_passes(&mut registry);

        let mut far_context = test_context(MapLayerMask::all());
        far_context.zoom_factor = 0.2;
        let far = renderer.build_frame_plan(far_context, &registry);
        assert!(!far.draw.railways);
        assert!(!far.static_decals.railways.visible);
        assert!(far.static_decals.terrain_rivers.visible);

        let mut close_context = far_context;
        close_context.zoom_factor = 0.75;
        let close = renderer.build_frame_plan(close_context, &registry);
        assert!(!close.draw.railways);
        assert!(!close.static_decals.railways.visible);

        close_context.map_mode = MapMode::Infrastructure;
        let infrastructure = renderer.build_frame_plan(close_context, &registry);
        assert!(infrastructure.draw.railways);
        assert!(infrastructure.static_decals.railways.opacity > 0.2);
    }

    #[test]
    fn network_lines_do_not_clutter_default_political_map() {
        let renderer = MapRenderer::new();
        let mut registry = PassRegistry::new();
        renderer.register_passes(&mut registry);

        let mut context = test_context(MapLayerMask::all());
        context.map_mode = MapMode::Political;
        context.zoom_factor = 0.75;
        let political = renderer.build_frame_plan(context, &registry);
        assert!(!political.static_decals.trade_routes.visible);
        assert!(!political.static_decals.straits.visible);
        assert!(!political.draw.trade_routes);
        assert!(!political.draw.straits);

        context.map_mode = MapMode::Supply;
        let supply = renderer.build_frame_plan(context, &registry);
        assert!(supply.static_decals.trade_routes.visible);
        assert!(supply.static_decals.straits.visible);
        assert!(supply.draw.trade_routes);
        assert!(supply.draw.straits);
    }

    #[test]
    fn static_decal_kind_catalog_covers_phase6_scope() {
        assert_eq!(StaticMapDecalKind::ALL.len(), 8);
        assert!(StaticMapDecalKind::ALL.contains(&StaticMapDecalKind::TerrainRivers));
        assert!(StaticMapDecalKind::ALL.contains(&StaticMapDecalKind::Railways));
        assert!(StaticMapDecalKind::ALL.contains(&StaticMapDecalKind::ShoreAccents));
        assert!(StaticMapDecalKind::ALL.contains(&StaticMapDecalKind::ImpassableMarks));
        assert!(StaticMapDecalKind::ALL.contains(&StaticMapDecalKind::InfrastructureOverlay));
    }

    #[test]
    fn semantic_overlay_kind_catalog_covers_phase7_scope() {
        assert_eq!(SemanticOverlayKind::ALL.len(), 8);
        assert!(SemanticOverlayKind::ALL.contains(&SemanticOverlayKind::OccupationStripes));
        assert!(SemanticOverlayKind::ALL.contains(&SemanticOverlayKind::Frontlines));
        assert!(SemanticOverlayKind::ALL.contains(&SemanticOverlayKind::Arrows));
        assert!(SemanticOverlayKind::ALL.contains(&SemanticOverlayKind::SelectedProvincePulse));
        assert!(SemanticOverlayKind::ALL.contains(&SemanticOverlayKind::HoverHighlight));
    }

    #[test]
    fn world_object_kind_catalog_covers_phase8_scope() {
        assert_eq!(WorldObjectKind::ALL.len(), 6);
        assert!(WorldObjectKind::ALL.contains(&WorldObjectKind::CountryNames));
        assert!(WorldObjectKind::ALL.contains(&WorldObjectKind::ProvinceNames));
        assert!(WorldObjectKind::ALL.contains(&WorldObjectKind::Counters));
        assert!(WorldObjectKind::ALL.contains(&WorldObjectKind::PoiIcons));
        assert!(WorldObjectKind::ALL.contains(&WorldObjectKind::Buildings));
        assert!(WorldObjectKind::ALL.contains(&WorldObjectKind::Trees));
    }

    #[test]
    fn semantic_overlay_plan_has_priority_and_budget_rules() {
        let renderer = MapRenderer::new();
        let mut registry = PassRegistry::new();
        renderer.register_passes(&mut registry);

        let mut context = test_context(MapLayerMask::all());
        context.zoom_factor = 0.5;
        context.selected_province_id = 42;
        context.hovered_province_id = 43;
        let plan = renderer.build_frame_plan(context, &registry);
        let overlays = plan.semantic_overlays;
        assert!(!overlays.occupation_stripes.visible);
        assert!(overlays.arrows.visible);
        assert!(overlays.selected_province_pulse.visible);
        assert!(overlays.hover_highlight.visible);
        assert!(overlays.arrows.priority > overlays.frontlines.priority);
        assert!(overlays.selected_province_pulse.priority > overlays.frontlines.priority);
    }

    #[test]
    fn semantic_overlay_plan_predictably_scales_by_map_mode() {
        let mut political = test_context(MapLayerMask::all());
        political.zoom_factor = 0.55;
        let mut supply = political;
        supply.map_mode = MapMode::Supply;

        let political_plan =
            SemanticOverlaySystem::plan(political, StaticMapDecalSystem::plan(political));
        let supply_plan = SemanticOverlaySystem::plan(supply, StaticMapDecalSystem::plan(supply));

        assert!(supply_plan.map_mode_overlay.opacity > political_plan.map_mode_overlay.opacity);
        assert!(supply_plan.budget.map_mode > political_plan.budget.map_mode);
    }

    #[test]
    fn infrastructure_mode_keeps_railways_available_at_medium_zoom() {
        let renderer = MapRenderer::new();
        let mut registry = PassRegistry::new();
        renderer.register_passes(&mut registry);

        let mut context = test_context(MapLayerMask::all());
        context.map_mode = MapMode::Infrastructure;
        context.zoom_factor = 0.35;
        let plan = renderer.build_frame_plan(context, &registry);
        assert!(plan.draw.railways);
        assert!(plan.static_decals.infrastructure_overlay.visible);
    }

    #[test]
    fn world_object_plan_zoom_gates_province_names_and_trees() {
        let renderer = MapRenderer::new();
        let mut registry = PassRegistry::new();
        renderer.register_passes(&mut registry);

        let mut far = test_context(MapLayerMask::all());
        far.zoom_factor = 0.18;
        let far_plan = renderer.build_frame_plan(far, &registry);
        assert!(far_plan.draw.map_names);
        assert!(!far_plan.draw.province_names);
        assert!(!far_plan.draw.trees);

        let mut close = far;
        close.zoom_factor = 0.96;
        let close_plan = renderer.build_frame_plan(close, &registry);
        assert!(close_plan.draw.province_names);
        assert!(close_plan.draw.trees);
        assert!(close_plan.world_objects.province_name_min_pixels <= 21_000);
    }

    #[test]
    fn world_object_plan_counter_visibility_for_far_mid_close_zoom() {
        let renderer = MapRenderer::new();
        let mut registry = PassRegistry::new();
        renderer.register_passes(&mut registry);

        let mut far = test_context(MapLayerMask::all());
        far.zoom_factor = 0.18;
        let far_plan = renderer.build_frame_plan(far, &registry);
        assert!(!far_plan.draw.hoi3_counters);
        assert!(!far_plan.world_objects.counters.visible);
        assert!(!far_plan.world_objects.counter_selected_only);

        let mut selected_far = far;
        selected_far.selected_province_id = 42;
        let selected_far_plan = renderer.build_frame_plan(selected_far, &registry);
        assert!(selected_far_plan.draw.hoi3_counters);
        assert!(selected_far_plan.world_objects.counters.visible);
        assert!(selected_far_plan.world_objects.counter_selected_only);
        assert!(selected_far_plan.world_objects.counters.opacity < 0.50);

        let mut mid = far;
        mid.zoom_factor = 0.55;
        let mid_plan = renderer.build_frame_plan(mid, &registry);
        assert!(mid_plan.draw.hoi3_counters);
        assert!(!mid_plan.world_objects.counter_selected_only);
        assert!(mid_plan.world_objects.counter_layout_density < 0.70);

        let mut close = far;
        close.zoom_factor = 0.92;
        let close_plan = renderer.build_frame_plan(close, &registry);
        assert!(close_plan.draw.hoi3_counters);
        assert!(!close_plan.world_objects.counter_selected_only);
        assert!(
            close_plan.world_objects.counters.opacity > mid_plan.world_objects.counters.opacity
        );
        assert!(
            close_plan.world_objects.counter_layout_density
                > mid_plan.world_objects.counter_layout_density
        );
    }

    #[test]
    fn phase10_quality_preset_scales_density_controls() {
        let renderer = MapRenderer::new();
        let mut registry = PassRegistry::new();
        renderer.register_passes(&mut registry);

        let mut high = test_context(MapLayerMask::all());
        high.zoom_factor = 0.96;
        high.settings =
            MapRenderSettings::with_quality(MapLayerMask::all(), MapQualityPreset::High);
        let mut ultra = high;
        ultra.settings =
            MapRenderSettings::with_quality(MapLayerMask::all(), MapQualityPreset::Ultra);

        let high_plan = renderer.build_frame_plan(high, &registry);
        let ultra_plan = renderer.build_frame_plan(ultra, &registry);
        assert!(ultra_plan.world_objects.trees.opacity >= high_plan.world_objects.trees.opacity);
        assert!(
            ultra_plan.world_objects.province_name_min_pixels
                <= high_plan.world_objects.province_name_min_pixels
        );
        assert_eq!(high_plan.world_objects.poi_detail_level, 0);
        assert_eq!(ultra_plan.world_objects.poi_detail_level, 0);
    }

    #[test]
    fn low_end_quality_skips_expensive_dynamic_effect_passes() {
        let renderer = MapRenderer::new();
        let mut registry = PassRegistry::new();
        renderer.register_passes(&mut registry);

        let mut low = test_context(MapLayerMask::all());
        low.settings =
            MapRenderSettings::with_quality(MapLayerMask::all(), MapQualityPreset::LowEnd);
        let plan = renderer.build_frame_plan(low, &registry);

        assert!(plan.draw.terrain);
        assert!(plan.draw.water);
        assert!(!plan.draw.shadow_caster);
        assert!(!plan.draw.projected_fow_shadow);
        assert!(!plan.draw.water_refraction);
        assert!(!plan.draw.particles);
        assert!(!plan.draw.postprocess);
    }

    #[test]
    fn building_and_poi_objects_stay_disabled_for_now() {
        let mut political = test_context(MapLayerMask::all());
        political.zoom_factor = 0.65;
        let mut factories = political;
        factories.map_mode = MapMode::Factories;

        let political_plan = WorldObjectSystem::plan(political);
        let factory_plan = WorldObjectSystem::plan(factories);

        assert!(!political_plan.poi_icons.visible);
        assert!(!factory_plan.poi_icons.visible);
        assert!(!political_plan.buildings.visible);
        assert!(!factory_plan.buildings.visible);
        assert_eq!(political_plan.poi_detail_level, 0);
        assert_eq!(factory_plan.poi_detail_level, 0);

        factories.settings =
            MapRenderSettings::with_quality(MapLayerMask::all(), MapQualityPreset::Ultra);
        let ultra_factory_plan = WorldObjectSystem::plan(factories);
        assert!(!ultra_factory_plan.poi_icons.visible);
        assert!(!ultra_factory_plan.buildings.visible);
        assert_eq!(ultra_factory_plan.poi_detail_level, 0);
    }

    #[test]
    fn political_building_meshes_stay_disabled_at_mid_and_close_zoom() {
        let mut mid = test_context(MapLayerMask::all());
        mid.zoom_factor = 0.72;
        let mid_plan = WorldObjectSystem::plan(mid);
        assert!(!mid_plan.buildings.visible);

        let mut close = mid;
        close.zoom_factor = 0.96;
        let close_plan = WorldObjectSystem::plan(close);
        assert!(!close_plan.buildings.visible);
    }

    #[test]
    fn disabling_3d_map_suppresses_render_passes_without_legacy_fallback() {
        let renderer = MapRenderer::new();
        let mut registry = PassRegistry::new();
        renderer.register_passes(&mut registry);
        let mut context = test_context(MapLayerMask::all());
        context.draw_3d_map = false;
        let plan = renderer.build_frame_plan(context, &registry);
        assert!(!plan.draw.terrain);
        assert!(!plan.draw.water);
        assert!(!plan.draw.river);
        assert!(!plan.draw.borders);
        assert!(!plan.draw.hoi3_counters);
        assert!(!plan.draw.ui);

        let mut ui_context = test_context(MapLayerMask {
            ui: true,
            ..MapLayerMask::none()
        });
        ui_context.draw_3d_map = false;
        let ui_plan = renderer.build_frame_plan(ui_context, &registry);
        assert!(ui_plan.draw.ui);
        assert!(!ui_plan.draw.terrain);
    }

    #[test]
    fn terrain_material_ownership_uses_dedicated_passes_when_loaded() {
        let renderer = MapRenderer::new();
        let mut registry = PassRegistry::new();
        renderer.register_passes(&mut registry);

        let plan = renderer.build_frame_plan(test_context(MapLayerMask::all()), &registry);
        let ownership = plan.draw.terrain_material_ownership(
            MapLayerMask::all(),
            true,
            true,
            true,
            plan.static_decals,
            plan.semantic_overlays,
        );
        assert!(!ownership.terrain_water_final_color);
        assert!(!ownership.terrain_sdf_borders);
        assert!(!ownership.terrain_overlays);

        let river_fallback = plan.draw.terrain_material_ownership(
            MapLayerMask::all(),
            true,
            false,
            true,
            plan.static_decals,
            plan.semantic_overlays,
        );
        assert!(river_fallback.terrain_overlays);

        let fallback = plan.draw.terrain_material_ownership(
            MapLayerMask::all(),
            false,
            false,
            false,
            plan.static_decals,
            plan.semantic_overlays,
        );
        assert!(fallback.terrain_water_final_color);
        assert!(fallback.terrain_sdf_borders);
    }

    #[test]
    fn terrain_only_disables_terrain_owned_overlay_effects() {
        let renderer = MapRenderer::new();
        let mut registry = PassRegistry::new();
        renderer.register_passes(&mut registry);

        let mask = MapLayerMask::for_layer(crate::map_baseline::MapBaselineLayer::TerrainOnly);
        let plan = renderer.build_frame_plan(test_context(mask), &registry);
        let ownership = plan.draw.terrain_material_ownership(
            mask,
            true,
            true,
            true,
            plan.static_decals,
            plan.semantic_overlays,
        );
        assert!(!ownership.terrain_water_final_color);
        assert!(!ownership.terrain_sdf_borders);
        assert!(!ownership.terrain_overlays);
    }

    #[test]
    fn water_material_ownership_is_complete_when_loaded_and_drawn() {
        let renderer = MapRenderer::new();
        let mut registry = PassRegistry::new();
        renderer.register_passes(&mut registry);

        let plan = renderer.build_frame_plan(test_context(MapLayerMask::all()), &registry);
        let ownership = plan.draw.water_material_ownership(true);
        assert!(ownership.final_color);
        assert!(ownership.shallow_deep_transition);
        assert!(ownership.coast_foam);
        assert!(ownership.normal);
        assert!(ownership.specular);
        assert!(ownership.ice);
        assert!(ownership.reflection);
        assert!(ownership.sea_selection_highlight);

        let fallback = plan.draw.water_material_ownership(false);
        assert!(!fallback.final_color);
        assert!(!fallback.coast_foam);
        assert!(!fallback.reflection);
    }

    #[test]
    fn border_material_ownership_is_complete_when_loaded_and_drawn() {
        let renderer = MapRenderer::new();
        let mut registry = PassRegistry::new();
        renderer.register_passes(&mut registry);

        let plan = renderer.build_frame_plan(test_context(MapLayerMask::all()), &registry);
        let ownership = plan.draw.border_material_ownership(true);
        assert!(ownership.final_borders);
        assert!(ownership.country_borders);
        assert!(ownership.state_borders);
        assert!(ownership.province_borders);
        assert!(ownership.sea_borders);
        assert!(ownership.impassable_borders);
        assert!(ownership.selected_borders);

        let fallback = plan.draw.border_material_ownership(false);
        assert!(!fallback.final_borders);
        assert!(!fallback.country_borders);
        assert!(!fallback.selected_borders);
    }

    #[test]
    fn prepare_frame_reports_water_and_border_fallbacks() {
        let renderer = MapRenderer::new();
        let mut registry = PassRegistry::new();
        renderer.register_passes(&mut registry);

        let mask = water_border_mask();
        let plan = renderer.build_frame_plan(test_context(mask), &registry);
        let prepared = renderer.prepare_frame(
            &plan,
            MapPrepareFrameInput {
                layer_mask: mask,
                dedicated_water_loaded: false,
                dedicated_river_loaded: true,
                dedicated_border_loaded: false,
            },
        );

        assert!(prepared.terrain_ownership.terrain_water_final_color);
        assert!(prepared.terrain_ownership.terrain_sdf_borders);
        assert!(prepared.fallback_report.terrain_water_fallback);
        assert!(prepared.fallback_report.terrain_border_fallback);
        assert!(prepared.fallback_report.water_degraded);
        assert!(prepared.fallback_report.borders_degraded);
        assert!(prepared.fallback_report.is_degraded());
        assert_eq!(prepared.fallback_report.fallback_count(), 4);
    }

    #[test]
    fn prepare_frame_keeps_final_water_and_borders_in_dedicated_passes() {
        let renderer = MapRenderer::new();
        let mut registry = PassRegistry::new();
        renderer.register_passes(&mut registry);

        let mask = water_border_mask();
        let plan = renderer.build_frame_plan(test_context(mask), &registry);
        let prepared = renderer.prepare_frame(
            &plan,
            MapPrepareFrameInput {
                layer_mask: mask,
                dedicated_water_loaded: true,
                dedicated_river_loaded: true,
                dedicated_border_loaded: true,
            },
        );

        assert!(!prepared.terrain_ownership.terrain_water_final_color);
        assert!(!prepared.terrain_ownership.terrain_sdf_borders);
        assert!(prepared.water_ownership.final_color);
        assert!(prepared.border_ownership.final_borders);
        assert_eq!(prepared.fallback_report.fallback_count(), 0);
        assert!(!prepared.fallback_report.is_degraded());
    }
}
