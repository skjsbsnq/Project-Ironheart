use crate::map_baseline::MapLayerMask;
use crate::map_perf::MapQualityPreset;
use crate::passes::PassRegistry;

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
    Sky,
    Terrain,
    Water,
    Borders,
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
    pub const PHASE1_ORDER: [Self; 19] = [
        Self::ShadowCaster,
        Self::Sky,
        Self::Terrain,
        Self::Water,
        Self::Borders,
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
            Self::Sky => "3d_sky",
            Self::Terrain => "3d_terrain",
            Self::Water => "3d_water",
            Self::Borders => "3d_border",
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
            Self::Sky => mask.sky,
            Self::Terrain => mask.terrain,
            Self::Water => mask.water,
            Self::Borders => mask.borders,
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
            Self::Postprocess => controls.postprocess_chain,
            _ => true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MapRenderGraph {
    passes: Vec<MapRenderPass>,
}

impl MapRenderGraph {
    pub fn phase1() -> Self {
        Self {
            passes: MapRenderPass::PHASE1_ORDER.to_vec(),
        }
    }

    pub fn passes(&self) -> &[MapRenderPass] {
        &self.passes
    }
}

#[derive(Debug, Clone, Default)]
pub struct MapPassDrawSet {
    pub shadow_caster: bool,
    pub sky: bool,
    pub terrain: bool,
    pub water: bool,
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
        if static_enabled
            && (zoom >= 0.44 || infrastructure_mode || context.map_mode == MapMode::Factories)
        {
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

        if (static_enabled || overlay_enabled) && zoom >= 0.26 {
            plan.trade_routes =
                StaticMapDecalDecision::visible(0.42 * smoothstep(0.24, 0.62, zoom));
        }
        if (static_enabled || overlay_enabled) && zoom >= 0.18 {
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
        let close_zoom = smoothstep(0.40, 0.72, zoom);
        let interaction_zoom = smoothstep(0.10, 0.28, zoom);
        let has_selection = context.selected_province_id != u32::MAX;
        let has_hover = context.hovered_province_id != u32::MAX
            && context.hovered_province_id != context.selected_province_id;

        let mut plan = SemanticOverlayPlan {
            hovered_province_id: context.hovered_province_id,
            budget,
            ..SemanticOverlayPlan::default()
        };

        plan.occupation_stripes =
            SemanticOverlayDecision::visible(budget.passive * mix(0.30, 0.72, close_zoom), 20);
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
        let close = smoothstep(0.38, 0.72, zoom);
        let very_close = smoothstep(0.62, 0.90, zoom);
        let map_mode_object_focus = map_mode_object_factor(context.map_mode);

        let mut plan = WorldObjectPlan {
            budget,
            ..WorldObjectPlan::default()
        };

        if mask.labels {
            let country_opacity =
                budget.labels * mix(0.88, 0.62, close) * mix(0.82, 1.0, strategic);
            let country_scale = mix(1.10, 0.76, close);
            plan.country_names = WorldObjectDecision::visible(country_opacity, country_scale, 60);

            let province_opacity = budget.labels * very_close;
            let province_min_pixels = if zoom < 0.62 {
                20_000
            } else if zoom < 0.78 {
                8_000
            } else {
                2_500
            };
            let province_min_pixels =
                ((province_min_pixels as f32) / quality.label_density.max(0.25)).round() as u32;
            plan.province_names =
                WorldObjectDecision::visible(province_opacity, mix(0.86, 1.0, very_close), 50);
            plan.province_name_min_pixels = province_min_pixels;
        }

        if mask.objects || mask.overlays {
            let counter_base = budget.counters * smoothstep(0.10, 0.26, zoom);
            let counter_scale = mix(0.82, 1.08, close);
            plan.counters = WorldObjectDecision::visible(counter_base, counter_scale, 90);
            plan.counter_layout_density = mix(0.68, 1.0, close);
        }

        if mask.objects {
            let poi_mode_boost = mix(0.60, 1.0, map_mode_object_focus);
            let poi_opacity = budget.objects * poi_mode_boost * smoothstep(0.30, 0.62, zoom);
            plan.poi_icons = WorldObjectDecision::visible(poi_opacity, mix(0.78, 1.05, close), 70);
            let poi_detail_level = if zoom < 0.34 {
                1
            } else if zoom < 0.60 {
                2
            } else {
                3
            };
            plan.poi_detail_level = if quality.tree_density < 0.9 {
                poi_detail_level.min(2)
            } else {
                poi_detail_level
            };

            let buildings_opacity =
                budget.objects * map_mode_object_focus.max(0.35) * smoothstep(0.46, 0.78, zoom);
            plan.buildings =
                WorldObjectDecision::visible(buildings_opacity, mix(0.74, 1.0, close), 55);

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
            MapRenderPass::Sky => self.sky = enabled,
            MapRenderPass::Terrain => self.terrain = enabled,
            MapRenderPass::Water => self.water = enabled,
            MapRenderPass::Borders => self.borders = enabled,
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
        dedicated_border_loaded: bool,
        static_decals: StaticMapDecalPlan,
    ) -> TerrainMaterialOwnership {
        let dedicated_water_active = self.water && dedicated_water_loaded;
        let dedicated_border_active = self.borders && dedicated_border_loaded;
        let terrain_static_decals = static_decals.terrain_rivers.visible
            || static_decals.shore_accents.visible
            || static_decals.impassable_marks.visible;
        TerrainMaterialOwnership {
            terrain_water_final_color: self.water && !dedicated_water_active,
            terrain_sdf_borders: self.borders && !dedicated_border_active,
            terrain_overlays: mask.overlays || terrain_static_decals || mask.particles,
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

fn map_mode_object_factor(mode: MapMode) -> f32 {
    match mode {
        MapMode::Factories | MapMode::Infrastructure | MapMode::Supply => 1.0,
        MapMode::Manpower | MapMode::Resistance => 0.82,
        MapMode::Cores | MapMode::Ideology => 0.62,
        MapMode::Political => 0.56,
        MapMode::Terrain => 0.42,
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
                "3d_sky",
                "3d_terrain",
                "3d_water",
                "3d_border",
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
        assert!(full.draw.water);
        assert!(full.draw.map_arrows);
        assert!(full.draw.postprocess);
        assert!(!full.draw.frontlines);
    }

    #[test]
    fn static_decal_plan_zoom_gates_railways() {
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
        assert!(close.draw.railways);
        assert!(close.static_decals.railways.opacity > 0.2);
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
        assert!(overlays.occupation_stripes.visible);
        assert!(overlays.arrows.visible);
        assert!(overlays.selected_province_pulse.visible);
        assert!(overlays.hover_highlight.visible);
        assert!(overlays.arrows.priority > overlays.frontlines.priority);
        assert!(overlays.selected_province_pulse.priority > overlays.occupation_stripes.priority);
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
        close.zoom_factor = 0.82;
        let close_plan = renderer.build_frame_plan(close, &registry);
        assert!(close_plan.draw.province_names);
        assert!(close_plan.draw.trees);
        assert!(close_plan.world_objects.province_name_min_pixels < 10_000);
    }

    #[test]
    fn phase10_quality_preset_scales_density_controls() {
        let renderer = MapRenderer::new();
        let mut registry = PassRegistry::new();
        renderer.register_passes(&mut registry);

        let mut high = test_context(MapLayerMask::all());
        high.zoom_factor = 0.82;
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
        assert!(
            ultra_plan.world_objects.poi_detail_level >= high_plan.world_objects.poi_detail_level
        );
    }

    #[test]
    fn factory_mode_prioritizes_poi_and_buildings() {
        let mut political = test_context(MapLayerMask::all());
        political.zoom_factor = 0.65;
        let mut factories = political;
        factories.map_mode = MapMode::Factories;

        let political_plan = WorldObjectSystem::plan(political);
        let factory_plan = WorldObjectSystem::plan(factories);

        assert!(factory_plan.poi_icons.opacity > political_plan.poi_icons.opacity);
        assert!(factory_plan.buildings.opacity > political_plan.buildings.opacity);
        assert_eq!(factory_plan.poi_detail_level, 3);
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
            plan.static_decals,
        );
        assert!(!ownership.terrain_water_final_color);
        assert!(!ownership.terrain_sdf_borders);
        assert!(ownership.terrain_overlays);

        let fallback = plan.draw.terrain_material_ownership(
            MapLayerMask::all(),
            false,
            false,
            plan.static_decals,
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
        let ownership = plan
            .draw
            .terrain_material_ownership(mask, true, true, plan.static_decals);
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
}
