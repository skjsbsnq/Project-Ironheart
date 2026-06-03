use std::sync::mpsc;

use crate::passes::PassRegistry;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapQualityPreset {
    LowEnd,
    High,
    Ultra,
}

impl Default for MapQualityPreset {
    fn default() -> Self {
        Self::High
    }
}

impl MapQualityPreset {
    pub const ALL: [Self; 3] = [Self::LowEnd, Self::High, Self::Ultra];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LowEnd => "low_end",
            Self::High => "high",
            Self::Ultra => "ultra",
        }
    }

    pub fn next(self) -> Self {
        let idx = Self::ALL
            .iter()
            .position(|preset| *preset == self)
            .unwrap_or(0);
        Self::ALL[(idx + 1) % Self::ALL.len()]
    }

    pub const fn controls(self) -> MapQualityControls {
        match self {
            Self::LowEnd => MapQualityControls {
                terrain_lod_density: 0.60,
                tree_density: 0.35,
                particle_density: 0.20,
                border_detail: 0.55,
                label_density: 0.62,
                postprocess_chain: false,
                point_lights: false,
                runtime_target_scale: 0.50,
                object_lod_bias: 1,
            },
            Self::High => MapQualityControls {
                terrain_lod_density: 0.92,
                tree_density: 0.82,
                particle_density: 0.75,
                border_detail: 0.90,
                label_density: 0.88,
                postprocess_chain: true,
                point_lights: true,
                runtime_target_scale: 1.0,
                object_lod_bias: 0,
            },
            Self::Ultra => MapQualityControls {
                terrain_lod_density: 1.0,
                tree_density: 1.0,
                particle_density: 1.0,
                border_detail: 1.0,
                label_density: 1.0,
                postprocess_chain: true,
                point_lights: true,
                runtime_target_scale: 1.0,
                object_lod_bias: 0,
            },
        }
    }

    pub const fn budget(self) -> MapPerformanceBudget {
        match self {
            Self::LowEnd => MapPerformanceBudget {
                frame_1080p_ms: 25.0,
                frame_1440p_ms: 33.3,
                pass_gpu_ms: 22.0,
                cpu_prepare_ms: 5.0,
                draw_calls: 72,
                texture_memory_mb: 384,
            },
            Self::High => MapPerformanceBudget {
                frame_1080p_ms: 16.7,
                frame_1440p_ms: 22.2,
                pass_gpu_ms: 14.5,
                cpu_prepare_ms: 4.0,
                draw_calls: 96,
                texture_memory_mb: 512,
            },
            Self::Ultra => MapPerformanceBudget {
                frame_1080p_ms: 25.0,
                frame_1440p_ms: 33.3,
                pass_gpu_ms: 24.0,
                cpu_prepare_ms: 6.0,
                draw_calls: 144,
                texture_memory_mb: 768,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MapQualityControls {
    pub terrain_lod_density: f32,
    pub tree_density: f32,
    pub particle_density: f32,
    pub border_detail: f32,
    pub label_density: f32,
    pub postprocess_chain: bool,
    pub point_lights: bool,
    pub runtime_target_scale: f32,
    pub object_lod_bias: u8,
}

impl MapQualityControls {
    pub fn summary(self) -> String {
        format!(
            "lod={:.2} tree={:.2} particle={:.2} border={:.2} label={:.2} pp={} point_lights={} rt_scale={:.2} object_lod_bias={}",
            self.terrain_lod_density,
            self.tree_density,
            self.particle_density,
            self.border_detail,
            self.label_density,
            if self.postprocess_chain {
                "full"
            } else {
                "off"
            },
            self.point_lights,
            self.runtime_target_scale,
            self.object_lod_bias
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MapPerformanceBudget {
    pub frame_1080p_ms: f32,
    pub frame_1440p_ms: f32,
    pub pass_gpu_ms: f32,
    pub cpu_prepare_ms: f32,
    pub draw_calls: u32,
    pub texture_memory_mb: u32,
}

impl MapPerformanceBudget {
    pub fn frame_target_ms(self, width: u32, height: u32) -> f32 {
        let pixels = (width as f32 * height as f32).max(1.0);
        let p1080 = 1920.0 * 1080.0;
        let p1440 = 2560.0 * 1440.0;
        let t = ((pixels - p1080) / (p1440 - p1080)).clamp(0.0, 1.0);
        self.frame_1080p_ms + (self.frame_1440p_ms - self.frame_1080p_ms) * t
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetState {
    Within,
    Over,
}

impl BudgetState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Within => "ok",
            Self::Over => "over",
        }
    }
}

pub fn estimate_frame_texture_memory_bytes(width: u32, height: u32, postprocess_full: bool) -> u64 {
    let w = width.max(1) as u64;
    let h = height.max(1) as u64;
    let hdr_rgba16f = w * h * 8;
    let depth32 = w * h * 4;
    let mut total = hdr_rgba16f + depth32;

    if postprocess_full {
        let mut bw = (w / 2).max(2);
        let mut bh = (h / 2).max(2);
        for _ in 0..4 {
            total += bw * bh * 8;
            bw = (bw / 2).max(1);
            bh = (bh / 2).max(1);
        }

        let mut lw = (w / 16).max(2);
        let mut lh = (h / 16).max(2);
        for _ in 0..4 {
            total += lw * lh * 2;
            lw = (lw / 4).max(1);
            lh = (lh / 4).max(1);
        }

        // Vanilla ColorCube is a 1024x32 flattened 32x32x32 RGBA8 LUT.
        // The identity fallback keeps the same shape so missing assets do not
        // silently exercise the old 16^3 path.
        total += 1024 * 32 * 4;
    }

    total
}

pub fn mib(bytes: u64) -> f32 {
    bytes as f32 / (1024.0 * 1024.0)
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MapResourceCacheStats {
    pub dds_upload_entries: usize,
    pub mesh_parse_entries: usize,
    pub runtime_target_cache_entries: usize,
}

impl MapResourceCacheStats {
    pub fn summary(self) -> String {
        format!(
            "dds_upload={} mesh_parse={} runtime_targets={}",
            self.dds_upload_entries, self.mesh_parse_entries, self.runtime_target_cache_entries
        )
    }
}

pub struct Phase10OverlayInput {
    pub preset: MapQualityPreset,
    pub width: u32,
    pub height: u32,
    pub last_frame_cpu_ms: f32,
    pub cpu_prepare_ms: f32,
    pub texture_memory_bytes: u64,
    pub gpu_status: GpuProfilerStatus,
}

pub fn phase10_overlay_lines(input: Phase10OverlayInput, registry: &PassRegistry) -> Vec<String> {
    let budget = input.preset.budget();
    let target_ms = budget.frame_target_ms(input.width, input.height);
    let draw_calls = registry.total_draw_calls();
    let cpu_pass_ms = registry.total_cpu_ms();
    let gpu_pass_ms = registry.total_gpu_ms();
    let texture_mb = mib(input.texture_memory_bytes);
    let pass_texture_mb = mib(registry.total_texture_memory_bytes());
    let fallback_count = registry.total_fallback_count();

    let frame_state = if input.last_frame_cpu_ms <= target_ms || input.last_frame_cpu_ms <= 0.0 {
        BudgetState::Within
    } else {
        BudgetState::Over
    };
    let cpu_state = if input.cpu_prepare_ms <= budget.cpu_prepare_ms {
        BudgetState::Within
    } else {
        BudgetState::Over
    };
    let draw_state = if draw_calls <= budget.draw_calls {
        BudgetState::Within
    } else {
        BudgetState::Over
    };
    let texture_state = if texture_mb <= budget.texture_memory_mb as f32 {
        BudgetState::Within
    } else {
        BudgetState::Over
    };
    let gpu_state = if gpu_pass_ms <= 0.0 || gpu_pass_ms <= budget.pass_gpu_ms {
        BudgetState::Within
    } else {
        BudgetState::Over
    };

    vec![
        format!(
            "phase12 preset={} budget frame {:.1}/{:.1}ms {} cpu_prepare {:.2}/{:.2}ms {}",
            input.preset.as_str(),
            input.last_frame_cpu_ms,
            target_ms,
            frame_state.as_str(),
            input.cpu_prepare_ms,
            budget.cpu_prepare_ms,
            cpu_state.as_str()
        ),
        format!(
            "phase12 passes cpu={:.2}ms gpu={:.2}/{:.2}ms {} draw_calls={}/{} {}",
            cpu_pass_ms,
            gpu_pass_ms,
            budget.pass_gpu_ms,
            gpu_state.as_str(),
            draw_calls,
            budget.draw_calls,
            draw_state.as_str()
        ),
        format!(
            "phase12 texture_est={:.1}/{} MiB {} pass_tex={:.1}MiB fallback={} gpu_timestamp={}",
            texture_mb,
            budget.texture_memory_mb,
            texture_state.as_str(),
            pass_texture_mb,
            fallback_count,
            input.gpu_status.summary()
        ),
        format!("phase12 quality {}", input.preset.controls().summary()),
    ]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GpuProfilerStatus {
    pub supported: bool,
    pub pass_writes: bool,
    pub encoder_writes: bool,
    pub pending_readback: bool,
}

impl GpuProfilerStatus {
    pub const UNSUPPORTED: Self = Self {
        supported: false,
        pass_writes: false,
        encoder_writes: false,
        pending_readback: false,
    };

    pub fn summary(self) -> String {
        if !self.supported {
            return "unsupported".to_string();
        }
        format!(
            "on(pass={},encoder={},pending={})",
            self.pass_writes, self.encoder_writes, self.pending_readback
        )
    }
}

#[derive(Debug, Clone, Copy)]
pub struct GpuTimestampToken {
    name: &'static str,
    begin_index: u32,
    end_index: u32,
}

#[derive(Debug, Clone, Copy)]
struct GpuTimestampSpan {
    name: &'static str,
    begin_index: u32,
    end_index: u32,
}

#[derive(Debug)]
struct PendingGpuReadback {
    spans: Vec<GpuTimestampSpan>,
    bytes: u64,
    mapping_requested: bool,
}

pub struct GpuTimestampProfiler {
    query_set: wgpu::QuerySet,
    resolve_buffer: wgpu::Buffer,
    readback_buffer: wgpu::Buffer,
    timestamp_period_ns: f32,
    pass_writes: bool,
    encoder_writes: bool,
    max_queries: u32,
    cursor: u32,
    frame_active: bool,
    spans: Vec<GpuTimestampSpan>,
    pending: Option<PendingGpuReadback>,
    tx: mpsc::Sender<Result<(), String>>,
    rx: mpsc::Receiver<Result<(), String>>,
}

impl GpuTimestampProfiler {
    const MAX_QUERIES: u32 = 96;

    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Option<Self> {
        let features = device.features();
        let supported = features.contains(wgpu::Features::TIMESTAMP_QUERY);
        let pass_writes = features.contains(wgpu::Features::TIMESTAMP_QUERY_INSIDE_PASSES);
        let encoder_writes = features.contains(wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS);
        if !supported || (!pass_writes && !encoder_writes) {
            return None;
        }

        let max_queries = Self::MAX_QUERIES;
        let bytes = max_queries as u64 * wgpu::QUERY_SIZE as u64;
        let query_set = device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("phase10_gpu_timestamps"),
            ty: wgpu::QueryType::Timestamp,
            count: max_queries,
        });
        let resolve_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("phase10_gpu_timestamp_resolve"),
            size: bytes,
            usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let readback_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("phase10_gpu_timestamp_readback"),
            size: bytes,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let (tx, rx) = mpsc::channel();

        Some(Self {
            query_set,
            resolve_buffer,
            readback_buffer,
            timestamp_period_ns: queue.get_timestamp_period(),
            pass_writes,
            encoder_writes,
            max_queries,
            cursor: 0,
            frame_active: false,
            spans: Vec::new(),
            pending: None,
            tx,
            rx,
        })
    }

    pub fn status(&self) -> GpuProfilerStatus {
        GpuProfilerStatus {
            supported: true,
            pass_writes: self.pass_writes,
            encoder_writes: self.encoder_writes,
            pending_readback: self.pending.is_some(),
        }
    }

    pub fn poll_readback(&mut self, device: &wgpu::Device, registry: &mut PassRegistry) {
        let _ = device.poll(wgpu::Maintain::Poll);
        while let Ok(result) = self.rx.try_recv() {
            let Some(pending) = self.pending.take() else {
                continue;
            };
            if result.is_ok() {
                {
                    let data = self
                        .readback_buffer
                        .slice(0..pending.bytes)
                        .get_mapped_range();
                    let timestamps = bytemuck::cast_slice::<u8, u64>(&data);
                    for span in pending.spans {
                        let begin = timestamps.get(span.begin_index as usize).copied();
                        let end = timestamps.get(span.end_index as usize).copied();
                        if let (Some(begin), Some(end)) = (begin, end) {
                            let ticks = end.saturating_sub(begin);
                            let ms = ticks as f32 * self.timestamp_period_ns / 1_000_000.0;
                            if ms.is_finite() {
                                registry.record_gpu_ms(span.name, ms);
                            }
                        }
                    }
                }
                self.readback_buffer.unmap();
            }
        }
    }

    pub fn begin_frame(&mut self) {
        self.cursor = 0;
        self.spans.clear();
        self.frame_active = self.pending.is_none();
    }

    pub fn begin_render_span(
        &mut self,
        pass: &mut wgpu::RenderPass<'_>,
        name: &'static str,
    ) -> Option<GpuTimestampToken> {
        if !self.frame_active || !self.pass_writes {
            return None;
        }
        let token = self.alloc_token(name)?;
        pass.write_timestamp(&self.query_set, token.begin_index);
        Some(token)
    }

    pub fn end_render_span(&mut self, pass: &mut wgpu::RenderPass<'_>, token: GpuTimestampToken) {
        if !self.frame_active || !self.pass_writes {
            return;
        }
        pass.write_timestamp(&self.query_set, token.end_index);
        self.spans.push(GpuTimestampSpan {
            name: token.name,
            begin_index: token.begin_index,
            end_index: token.end_index,
        });
    }

    pub fn begin_encoder_span(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        name: &'static str,
    ) -> Option<GpuTimestampToken> {
        if !self.frame_active || !self.encoder_writes {
            return None;
        }
        let token = self.alloc_token(name)?;
        encoder.write_timestamp(&self.query_set, token.begin_index);
        Some(token)
    }

    pub fn end_encoder_span(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        token: GpuTimestampToken,
    ) {
        if !self.frame_active || !self.encoder_writes {
            return;
        }
        encoder.write_timestamp(&self.query_set, token.end_index);
        self.spans.push(GpuTimestampSpan {
            name: token.name,
            begin_index: token.begin_index,
            end_index: token.end_index,
        });
    }

    pub fn finish_frame(&mut self, encoder: &mut wgpu::CommandEncoder) {
        if !self.frame_active || self.cursor == 0 || self.spans.is_empty() {
            self.frame_active = false;
            return;
        }
        let bytes = self.cursor as u64 * wgpu::QUERY_SIZE as u64;
        encoder.resolve_query_set(&self.query_set, 0..self.cursor, &self.resolve_buffer, 0);
        encoder.copy_buffer_to_buffer(&self.resolve_buffer, 0, &self.readback_buffer, 0, bytes);
        self.pending = Some(PendingGpuReadback {
            spans: std::mem::take(&mut self.spans),
            bytes,
            mapping_requested: false,
        });
        self.frame_active = false;
    }

    pub fn map_pending_readback(&mut self) {
        if let Some(pending) = self.pending.as_mut() {
            if pending.mapping_requested {
                return;
            }
            pending.mapping_requested = true;
            let tx = self.tx.clone();
            self.readback_buffer.slice(0..pending.bytes).map_async(
                wgpu::MapMode::Read,
                move |result| {
                    let _ = tx.send(result.map_err(|err| err.to_string()));
                },
            );
        }
    }

    fn alloc_token(&mut self, name: &'static str) -> Option<GpuTimestampToken> {
        if self.cursor + 2 > self.max_queries {
            return None;
        }
        let begin_index = self.cursor;
        let end_index = self.cursor + 1;
        self.cursor += 2;
        Some(GpuTimestampToken {
            name,
            begin_index,
            end_index,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase10_quality_presets_are_ordered() {
        let low = MapQualityPreset::LowEnd.controls();
        let high = MapQualityPreset::High.controls();
        let ultra = MapQualityPreset::Ultra.controls();
        assert!(low.terrain_lod_density <= high.terrain_lod_density);
        assert!(high.terrain_lod_density <= ultra.terrain_lod_density);
        assert!(high.tree_density <= ultra.tree_density);
        assert!(high.particle_density <= ultra.particle_density);
        assert!(!low.point_lights);
        assert!(MapQualityPreset::LowEnd.next() == MapQualityPreset::High);
        assert!(MapQualityPreset::High.next() == MapQualityPreset::Ultra);
        assert!(MapQualityPreset::Ultra.next() == MapQualityPreset::LowEnd);
    }

    #[test]
    fn phase10_budgets_scale_with_resolution_and_preset() {
        let high = MapQualityPreset::High.budget();
        let ultra = MapQualityPreset::Ultra.budget();
        let low = MapQualityPreset::LowEnd.budget();
        assert!(high.frame_target_ms(2560, 1440) > high.frame_target_ms(1920, 1080));
        assert!(high.draw_calls > low.draw_calls);
        assert!(ultra.draw_calls > high.draw_calls);
        assert!(ultra.texture_memory_mb > high.texture_memory_mb);
    }

    #[test]
    fn phase10_texture_estimate_includes_postprocess_chain() {
        let off = estimate_frame_texture_memory_bytes(1920, 1080, false);
        let full = estimate_frame_texture_memory_bytes(1920, 1080, true);
        assert!(full > off);
        assert!(mib(full) < 128.0);
    }

    #[test]
    fn phase10_overlay_reports_budget_state() {
        let mut registry = PassRegistry::new();
        registry.register("3d_terrain");
        registry.record_cpu_ms("3d_terrain", 1.25);
        registry.record_draw_calls("3d_terrain", 3);
        registry.record_texture_memory_bytes("3d_terrain", 1024 * 1024);
        registry.record_fallback_count("3d_terrain", 1);
        let lines = phase10_overlay_lines(
            Phase10OverlayInput {
                preset: MapQualityPreset::High,
                width: 1920,
                height: 1080,
                last_frame_cpu_ms: 12.0,
                cpu_prepare_ms: 2.0,
                texture_memory_bytes: estimate_frame_texture_memory_bytes(1920, 1080, true),
                gpu_status: GpuProfilerStatus::UNSUPPORTED,
            },
            &registry,
        );
        assert!(lines.iter().any(|line| line.contains("preset=high")));
        assert!(lines.iter().any(|line| line.contains("draw_calls=3/96")));
        assert!(lines
            .iter()
            .any(|line| line.contains("gpu_timestamp=unsupported")));
        assert!(lines.iter().any(|line| line.contains("fallback=1")));
    }

    #[test]
    fn resource_cache_stats_summary_is_stable() {
        let stats = MapResourceCacheStats {
            dds_upload_entries: 7,
            mesh_parse_entries: 3,
            runtime_target_cache_entries: 10,
        };
        assert_eq!(
            stats.summary(),
            "dds_upload=7 mesh_parse=3 runtime_targets=10"
        );
    }
}
