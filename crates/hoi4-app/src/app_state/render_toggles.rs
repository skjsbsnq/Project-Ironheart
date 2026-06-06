use std::time::Instant;

use crate::map_perf::MapQualityPreset;
use crate::passes::{self, PostProcessDebugView};

pub(crate) struct RenderToggles {
    pub(crate) debug_overlay: bool,
    pub(crate) terrain_debug_view: passes::TerrainDebugView,
    pub(crate) water_debug_view: passes::WaterDebugView,
    pub(crate) border_debug_view: passes::BorderDebugView,
    pub(crate) postprocess_debug_view: PostProcessDebugView,
    pub(crate) map_quality_preset: MapQualityPreset,
    pub(crate) last_viewport_interaction_at: Option<Instant>,
    pub(crate) force_water_pass: bool,
    pub(crate) show_province_names: bool,
    pub(crate) frontline_overlay_visible: bool,
    pub(crate) prev_armies_hash: u64,
    pub(crate) frontline_overlay_hash: u64,
    pub(crate) trade_routes_hash: u64,
    pub(crate) last_frontline_arrow_rebuild_at: Instant,
    pub(crate) last_frontline_overlay_rebuild_at: Instant,
}

impl Default for RenderToggles {
    fn default() -> Self {
        Self {
            debug_overlay: false,
            terrain_debug_view: passes::TerrainDebugView::Off,
            water_debug_view: passes::WaterDebugView::Off,
            border_debug_view: passes::BorderDebugView::Off,
            postprocess_debug_view: PostProcessDebugView::Final,
            map_quality_preset: MapQualityPreset::High,
            last_viewport_interaction_at: None,
            force_water_pass: false,
            show_province_names: true,
            frontline_overlay_visible: true,
            prev_armies_hash: 0,
            frontline_overlay_hash: 0,
            trade_routes_hash: 0,
            last_frontline_arrow_rebuild_at: Instant::now(),
            last_frontline_overlay_rebuild_at: Instant::now(),
        }
    }
}
