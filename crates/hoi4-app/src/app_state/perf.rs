use std::time::Instant;

pub(crate) struct PerfState {
    pub(crate) last_perf_diag: Instant,
    pub(crate) last_render_profile_log: Instant,
    pub(crate) last_hours: u64,
    pub(crate) counter_rebuilds: u32,
    pub(crate) counter_cache_hits: u32,
    pub(crate) counter_instances: usize,
    pub(crate) render_us: u64,
    pub(crate) render_frames: u32,
    pub(crate) counter_update_us: u64,
    pub(crate) arrow_update_us: u64,
    pub(crate) last_frame_cpu_ms: f32,
    pub(crate) last_map_prepare_cpu_ms: f32,
}

impl Default for PerfState {
    fn default() -> Self {
        Self {
            last_perf_diag: Instant::now(),
            last_render_profile_log: Instant::now(),
            last_hours: 0,
            counter_rebuilds: 0,
            counter_cache_hits: 0,
            counter_instances: 0,
            render_us: 0,
            render_frames: 0,
            counter_update_us: 0,
            arrow_update_us: 0,
            last_frame_cpu_ms: 0.0,
            last_map_prepare_cpu_ms: 0.0,
        }
    }
}
