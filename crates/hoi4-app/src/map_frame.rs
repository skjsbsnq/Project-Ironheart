use crate::map_baseline::MapLayerMask;
use crate::map_perf::MapQualityPreset;
use crate::map_renderer::{MapFrameContext, MapRenderSettings};
use hoi4_render::map_mode::MapMode;
use hoi4_state::GameDate;

#[derive(Debug, Clone, Copy)]
pub(crate) struct MapFrameInput {
    pub(crate) draw_3d_map: bool,
    pub(crate) enable_3d_terrain: bool,
    pub(crate) map_mode: MapMode,
    pub(crate) date: GameDate,
    pub(crate) selected_province_id: u32,
    pub(crate) hovered_province_id: u32,
    pub(crate) zoom_factor: f32,
    pub(crate) time_seconds: f32,
    pub(crate) screen_size: [f32; 2],
    pub(crate) layer_mask: MapLayerMask,
    pub(crate) quality_preset: MapQualityPreset,
}

impl MapFrameInput {
    pub(crate) fn context(self) -> MapFrameContext {
        MapFrameContext {
            draw_3d_map: self.draw_3d_map && self.enable_3d_terrain,
            map_mode: self.map_mode,
            date: self.date,
            selected_province_id: self.selected_province_id,
            hovered_province_id: self.hovered_province_id,
            zoom_factor: self.zoom_factor,
            time_seconds: self.time_seconds,
            screen_size: self.screen_size,
            settings: MapRenderSettings::with_quality(self.layer_mask, self.quality_preset),
        }
    }
}
