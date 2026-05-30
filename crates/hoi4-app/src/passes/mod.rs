//! Phase 3.12.1 — 公共渲染基础设施。
//!
//! 把"3D 主渲染 → 离屏 HDR RT → 后处理 / 简化 blit → 交换链 → UI"这条新流水线
//! 的所有共享结构集中放这里。后续 3.12.2 ~ 3.12.14 的具体 pass 都会接到本模块
//! 暴露的 [`Pass`] trait + [`PassRegistry`]。
//!
//! ## 模块结构
//!
//! - [`hdr_target::HdrTarget`]：RGBA16Float 离屏主 RT，主 3D pass 的目标。
//! - [`global_uniform_buffer::GlobalUniformBuffer`]：包装 [`hoi4_render::global_uniform::GlobalFrameUniform`]
//!   GPU buffer，**所有** 3.12 pass 共用 `@group(0) @binding(0)`。
//! - [`hoi4_render::texture_upload::TextureUploadHelper`]：从 [`hoi4_assets::VanillaMapSet`]
//!   按 [`hoi4_assets::MapResRole`] 拉字节流 → 解 BMP / DDS → 上传到 wgpu Texture
//!   + 缓存。一行 API：`helper.upload_role(role)`。
//! - [`blit::SimpleBlitPass`]：Phase 3.12.1 临时桥接——直接 sample HDR → 输出
//!   到 sRGB 交换链（无任何后处理）。3.12.2 起被 [`postprocess::PostProcessChain`] 替换。
//! - [`postprocess::PostProcessChain`]：Phase 3.12.2 完整后处理链
//!   （bloom-bright + 4× 下采样 + 亮度归约 + LUT + restorescene + saturation）。
//! - [`debug_overlay::DebugOverlay`]：F4 渲染调试面板。
//!
//! ## Pass trait 与执行顺序
//!
//! [`Pass`] trait 规定 `prepare()` / `render()` 分两阶段：
//!
//! 1. `prepare(queue, ...)`：每帧一次，更新自身的 uniform / instance buffer。
//!    *不*碰 encoder。
//! 2. `render(encoder, ...)`：在已开 encoder 的 frame 内提交命令。**不**自己开
//!    新的 encoder。
//!
//! [`PassRegistry`] 是一个简单 vec，主 render 按顺序遍历执行。`enabled` 标志
//! 让 F4 调试覆盖能临时禁用单个 pass 做对比。
//!
//! ## 当前阶段使用范围
//!
//! 本模块的某些类型（如 [`global_uniform_buffer::GlobalUniformBuffer::bind_group_layout_entry`]）
//! [`global_uniform_buffer::GlobalUniformBuffer::bind_group_layout_entry`]）在 3.12.1 / 3.12.2
//! 已就位但尚未被 main.rs 接进任何 pipeline——它们是 3.12.4+（pdxmap / pdxmesh /
//! pdxwater 等）的"准备好的钩子"。允许 `dead_code` 直到这些 pass 接入。

#![allow(dead_code)]

pub mod blit;
pub mod border;
pub mod counter_v3;
pub mod debug_overlay;
pub mod global_uniform_buffer;
pub mod hdr_target;
pub mod maparrow;
pub mod mapname;
pub mod particle;
pub mod pdxmesh;
pub mod poi_icon;
pub mod postprocess;
pub mod province_name;
pub mod river;
pub mod shadow;
pub mod sky;
pub mod strait;
pub mod terrain;
pub mod traderoute;
pub mod trees_full;
pub mod water;

pub use blit::SimpleBlitPass;
pub use border::{BorderDebugView, BorderParams, BorderPass, BorderPassInputs};
pub use counter_v3::Hoi3CounterPass;
pub use debug_overlay::DebugOverlay;
pub use global_uniform_buffer::GlobalUniformBuffer;
pub use hdr_target::HdrTarget;
pub use maparrow::MapArrowPass;
pub use mapname::{MapnameParams, MapnamePass, MapnamePassInputs};
pub use particle::ParticlePass;
pub use pdxmesh::PdxMeshPass;
pub use poi_icon::PoiIconPass;
pub use postprocess::{
    PostProcessCalibration, PostProcessChain, PostProcessDebugView, PostProcessMode,
};
pub use province_name::{ProvinceNameParams, ProvinceNamePass};
pub use river::{RiverParams, RiverPass, RiverPassInputs};
pub use shadow::{ShadowPass, SHADOW_DEPTH_FORMAT, SHADOW_MAP_SIZE};
pub use sky::SkyPass;
pub use strait::StraitPass;
pub use terrain::{
    ChunkUniform as TerrainChunkUniform, PdxMapParams, TerrainDebugView, TerrainPass,
    TerrainPassInputs,
};
pub use traderoute::TradeRoutePass;
pub use trees_full::{TreeFullParams, TreeFullPass, TreeFullPassInputs};
pub use water::{WaterDebugView, WaterParams, WaterPass, WaterPassInputs};

/// HDR 主 RT 的 wgpu 格式。统一 `Rgba16Float`：放得下任意正向高光值，
/// 后处理链按 ACES tonemap 收回 LDR。
pub const HDR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

/// Phase 3.12.1 的 [`Pass`] trait。
///
/// 子节实现者把自己的 pipeline + bind groups + uniform 封装到一个 struct，
/// 然后实现 `prepare` 与 `render`。`PassRegistry` 按注册顺序串联调用。
///
/// 因为不同 pass 的 `render` 签名差异很大（depth attachment / clear color /
/// 参与 instance count 等），本 trait 不强制 `render` 的签名——使用方各自调用
/// 即可，trait 只提供"统一名字 + 启用开关"两个治理点。
pub trait Pass {
    /// 人类可读 pipeline 名字（用于 F4 overlay）。
    fn name(&self) -> &'static str;

    /// 该 pass 当前是否启用（F4 / 设置面板可临时关闭）。
    fn enabled(&self) -> bool {
        true
    }
}

/// 简易 pass 注册表。
///
/// 仅持有 `(name, enabled)` 用于 F4 overlay 显示——具体 pass 的 owner 仍是
/// `RenderState`（不能 `Box<dyn Pass>` 因为 render 签名不统一）。本表的目的
/// 是给调试覆盖 / 日志一个集中清单，并在未来加 GPU 时间统计时有处贴 timestamp。
#[derive(Debug, Default, Clone)]
pub struct PassRegistry {
    pub entries: Vec<PassEntry>,
}

#[derive(Debug, Clone)]
pub struct PassEntry {
    pub name: &'static str,
    pub enabled: bool,
    /// Last measured CPU time for this pass in milliseconds.
    pub cpu_ms: f32,
    /// Last measured GPU time for this pass in milliseconds.
    pub gpu_ms: f32,
    /// Draw calls submitted by this pass during the current frame.
    pub draw_calls: u32,
}

impl PassRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// 注册一个 pass（按调用顺序入栈）。
    pub fn register(&mut self, name: &'static str) {
        self.entries.push(PassEntry {
            name,
            enabled: true,
            cpu_ms: 0.0,
            gpu_ms: 0.0,
            draw_calls: 0,
        });
    }

    /// 切换某 pass 启用状态（F4 数字键暂未启用，但 API 已留好）。
    pub fn set_enabled(&mut self, name: &str, enabled: bool) {
        if let Some(e) = self.entries.iter_mut().find(|e| e.name == name) {
            e.enabled = enabled;
        }
    }

    pub fn is_enabled(&self, name: &str) -> bool {
        self.entries
            .iter()
            .find(|e| e.name == name)
            .map(|e| e.enabled)
            .unwrap_or(true)
    }

    pub fn begin_frame_stats(&mut self) {
        for entry in &mut self.entries {
            entry.cpu_ms = 0.0;
            entry.draw_calls = 0;
        }
    }

    pub fn record_cpu_ms(&mut self, name: &str, cpu_ms: f32) {
        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.name == name) {
            entry.cpu_ms += cpu_ms.max(0.0);
        }
    }

    pub fn record_gpu_ms(&mut self, name: &str, gpu_ms: f32) {
        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.name == name) {
            entry.gpu_ms = gpu_ms.max(0.0);
        }
    }

    pub fn record_draw_calls(&mut self, name: &str, draw_calls: u32) {
        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.name == name) {
            entry.draw_calls = entry.draw_calls.saturating_add(draw_calls);
        }
    }

    pub fn total_cpu_ms(&self) -> f32 {
        self.entries.iter().map(|entry| entry.cpu_ms).sum()
    }

    pub fn total_gpu_ms(&self) -> f32 {
        self.entries.iter().map(|entry| entry.gpu_ms).sum()
    }

    pub fn total_draw_calls(&self) -> u32 {
        self.entries
            .iter()
            .map(|entry| entry.draw_calls)
            .fold(0u32, u32::saturating_add)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_registers_and_toggles() {
        let mut r = PassRegistry::new();
        r.register("terrain");
        r.register("postprocess");
        assert_eq!(r.entries.len(), 2);
        assert!(r.is_enabled("terrain"));
        r.set_enabled("terrain", false);
        assert!(!r.is_enabled("terrain"));
        assert!(r.is_enabled("postprocess"));
    }

    #[test]
    fn registry_accumulates_phase10_stats() {
        let mut r = PassRegistry::new();
        r.register("terrain");
        r.record_cpu_ms("terrain", 1.25);
        r.record_cpu_ms("terrain", 0.75);
        r.record_gpu_ms("terrain", 2.5);
        r.record_draw_calls("terrain", 3);
        r.record_draw_calls("terrain", 4);
        assert!((r.total_cpu_ms() - 2.0).abs() < f32::EPSILON);
        assert!((r.total_gpu_ms() - 2.5).abs() < f32::EPSILON);
        assert_eq!(r.total_draw_calls(), 7);

        r.begin_frame_stats();
        assert_eq!(r.total_cpu_ms(), 0.0);
        assert_eq!(r.total_draw_calls(), 0);
        assert_eq!(r.total_gpu_ms(), 2.5);
    }

    #[test]
    fn hdr_format_is_rgba16float() {
        assert_eq!(HDR_FORMAT, wgpu::TextureFormat::Rgba16Float);
    }
}
