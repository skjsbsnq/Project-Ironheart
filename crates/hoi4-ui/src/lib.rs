//! `hoi4-ui` — V5 阶段 B 自研 UI 框架（egui + vanilla theme）。
//!
//! ## 路线图位置
//!
//! 见 `ROADMAP_V5.md` 阶段 B：
//!
//! - **B.1（2026-05-19 完成）**：新建 crate + pin `egui` / `egui-wgpu` /
//!   `egui-winit` 到 0.31.1。
//! - **B.2（本提交）**：`UiState` 集成 `egui::Context` + `egui_winit::State` +
//!   `egui_wgpu::Renderer`，暴露 `new` / `on_window_event` / `begin_frame` /
//!   `paint` 四个 API。`hoi4-app` 在主循环每帧调用 `begin_frame` → 已有 3D /
//!   HDR blit → `paint` 把 egui overlay 画到 swapchain。
//! - **B.3..B.7**：vanilla theme / 9-slice / icon helper / demo / perf budget。
//!
//! ## 设计原则
//!
//! - **不解析 vanilla `.gui` / `.gfx` / `.fnt`**：那条路线在 V5 阶段 A 已物理切断。
//! - 本 crate 只提供「egui 接入薄壳」+ 后续 vanilla theme，**不持有任何业务状态**。
//!   World / WorldBinding 由 caller 在 `begin_frame` 闭包内捕获。
//!
//! ## API 速记
//!
//! ```ignore
//! // 一次构造（init_render 末尾）
//! let mut ui = hoi4_ui::UiState::new(&device, surface_format, 1, &window);
//!
//! // window_event（最早处理，决定是否被 egui 消费）
//! let resp = ui.on_window_event(&window, &event);
//! if resp.consumed { return; }
//!
//! // 每帧 render
//! ui.begin_frame(&window, |ctx| {
//!     egui::Window::new("demo").show(ctx, |ui| { ui.label("hello"); });
//! });
//! // ... 原 3D + HDR blit ...
//! let cmds = ui.paint(&device, &queue, &mut enc, &window, &swap_view, [w, h]);
//! queue.submit(cmds.into_iter().chain(std::iter::once(enc.finish())));
//! frame.present();
//! ```

pub use egui;
pub use egui_wgpu;
pub use egui_winit;

pub mod air;
pub mod army_badge;
pub mod army_detail_panel;
pub mod components;
pub mod construction_v6_panel;
pub mod country_info_panel;
pub mod data_table;
pub mod dds_decode;
pub mod decisions_panel;
pub mod demo;
pub mod detail_panel;
pub mod diplomacy;
pub mod end_screen;
pub mod event_panel;
pub mod finance_panel;
pub mod focus_tree_panel;
pub mod frame_model;
pub mod i18n;
pub mod icons;
pub mod law_panel;
pub mod loc;
pub mod logistics_panel;
pub mod market_panel;
pub mod military;
pub mod nato_icon;
pub mod naval;
pub mod nine_slice;
pub mod politics;
pub mod pop_panel;
pub mod portrait;
pub mod province_info;
pub mod pyatiletka_panel;
pub mod research;
pub mod save_browser;
pub mod settings;
pub mod situation_panel;
pub mod surrender_notification;
pub mod theme;
pub mod topbar;
pub mod trade_panel;
pub mod v9;
pub mod vanilla_iron;

pub use frame_model::{
    ActiveDetailPanel, ActivePopup, ActivePrimaryPanel, AirWingDetailTarget, ArmyDetailTarget,
    BuildingDetailTarget, ConfirmPopup, CountryDetailTarget, DetailSource, FleetDetailTarget,
    FocusDetailTarget, GoodsDetailTarget, JournalEntryDetailTarget, PanelAction, PanelCommand,
    PanelKind, PopGroupDetailTarget, ProductionMethodTarget, ProvinceDetailTarget, RenamePopup,
    StateDetailTarget, TechnologyDetailTarget, TopbarAction, UiAction, UiFrameModel,
};

use std::time::Instant;

use winit::event::WindowEvent;
use winit::window::Window;

/// V5 阶段 B.7：egui 一帧 CPU 时间预算 = **2 ms**（10K 三角形以下）。
/// 由 [`UiState::last_stats`] 实时暴露，[`UiFrameStats::within_budget`] 校验。
pub const FRAME_BUDGET_US: u32 = 2_000;

/// 一帧 egui CPU 时间分桶（微秒）+ 几何统计。所有耗时字段累加 = `total_us`。
///
/// 由 [`UiState::begin_frame`] / [`UiState::paint`] 在每帧实际工作时填充，
/// 调用方读 [`UiState::last_stats`] 获取上一帧的成绩，用于 demo 显示 / 性能
/// 回归告警。
#[derive(Debug, Clone, Copy, Default)]
pub struct UiFrameStats {
    /// `begin_frame`（take_egui_input + ctx.run + UI 闭包）耗时。
    pub begin_us: u32,
    /// `paint` 阶段：`Context::tessellate`（Shape → ClippedPrimitive）耗时。
    pub tessellate_us: u32,
    /// `paint` 阶段：`update_texture` + `update_buffers` 写入 GPU 资源耗时。
    pub buffers_us: u32,
    /// `paint` 阶段：开 RenderPass + Renderer::render + free_texture 耗时。
    pub paint_us: u32,
    /// 上述 4 项之和（CPU 端，不含 GPU 异步执行时间）。
    pub total_us: u32,
    /// 本帧 ClippedPrimitive 数量（≈ draw call 数）。
    pub primitive_count: u32,
    /// 本帧三角形总数（所有 mesh.indices.len() / 3 之和）。
    pub triangle_count: u32,
}

impl UiFrameStats {
    /// `total_us` 是否在 [`FRAME_BUDGET_US`] 之内。
    pub fn within_budget(&self) -> bool {
        self.total_us <= FRAME_BUDGET_US
    }
}

/// egui 接入到 hoi4-app 主循环所需的全部 GPU + 输入状态。
///
/// 字段 `pub` 暴露主要为方便 caller 在 B.3+ 调整 `Style` / 注册自家纹理；
/// 正常每帧调用走 `new` / `on_window_event` / `begin_frame` / `paint` 四个方法即可。
pub struct UiState {
    /// egui 主上下文。配置（fonts / style / texture cache）从这里改。
    pub ctx: egui::Context,
    /// 把 winit 事件翻译成 egui RawInput 的桥接器。
    pub winit_state: egui_winit::State,
    /// egui 的 wgpu 后端渲染器。
    pub renderer: egui_wgpu::Renderer,
    /// `begin_frame` 之后、`paint` 之前缓存的 `FullOutput`。
    pending: Option<egui::FullOutput>,
    /// V5 阶段 B.7：上一帧 perf 探针结果（每帧由 `begin_frame` + `paint` 累加填充）。
    pub last_stats: UiFrameStats,
    /// 临时计时器：begin_frame 起点，paint 取走（保留 `begin_us` 的累积值）。
    /// `Option` 避免 paint 在没有 begin_frame 的状态下使用未初始化值。
    pending_begin_started: Option<Instant>,
}

impl UiState {
    /// 构造 UiState。
    ///
    /// - `target_format`: 目标 color attachment 格式（hoi4-app 用 surface sRGB 格式）。
    /// - `msaa_samples`: 1 = 不开 MSAA（hoi4-app HDR 链下游已经无 MSAA）。
    pub fn new(
        device: &wgpu::Device,
        target_format: wgpu::TextureFormat,
        msaa_samples: u32,
        window: &Window,
    ) -> Self {
        let ctx = egui::Context::default();
        let viewport_id = ctx.viewport_id();
        let winit_state = egui_winit::State::new(
            ctx.clone(),
            viewport_id,
            window,
            Some(window.scale_factor() as f32),
            None, // theme — 跟随系统
            None, // max_texture_side — 让 egui 走 device 上限
        );
        let renderer = egui_wgpu::Renderer::new(
            device,
            target_format,
            None, // depth_format — 不参与深度
            msaa_samples,
            false, // dithering — B.2 阶段不需要
        );
        Self {
            ctx,
            winit_state,
            renderer,
            pending: None,
            last_stats: UiFrameStats::default(),
            pending_begin_started: None,
        }
    }

    /// 把 winit window 事件转给 egui。
    ///
    /// 返回值的 `consumed=true` 时，caller 应**早退**，跳过本身的 hit-test
    /// （UI 优先原则，避免 egui Window 上的点击被地图拾取吃掉）。
    pub fn on_window_event(
        &mut self,
        window: &Window,
        event: &WindowEvent,
    ) -> egui_winit::EventResponse {
        self.winit_state.on_window_event(window, event)
    }

    /// 开始一帧：从 winit_state 拉 RawInput，运行 UI 闭包，缓存 FullOutput
    /// 等待 `paint` 消费。
    pub fn begin_frame(&mut self, window: &Window, run_ui: impl FnMut(&egui::Context)) {
        // V5 阶段 B.7：起点计时；paint 阶段累加其余分桶。
        let t_begin = Instant::now();
        self.pending_begin_started = Some(t_begin);
        let raw_input = self.winit_state.take_egui_input(window);
        let full_output = self.ctx.run(raw_input, run_ui);
        self.pending = Some(full_output);
        // begin_us 在这里就锁定（含 take_egui_input + ctx.run + UI 闭包）。
        self.last_stats.begin_us = t_begin.elapsed().as_micros() as u32;
    }

    /// 把 `begin_frame` 缓存的输出绘制到 `target_view`。
    ///
    /// 必须在 caller 完成所有 3D pass + HDR blit 之后再调，因为 LoadOp::Load 会
    /// 把 egui overlay 叠在已有内容上。
    ///
    /// 返回 `update_buffers` 产生的 user paint callback 命令缓冲（B.2 通常为空，
    /// 但 API 必须把它们交给 caller 与主 encoder 一起 submit，否则 paint callback
    /// 会丢失 GPU 工作）。
    pub fn paint(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        window: &Window,
        target_view: &wgpu::TextureView,
        size_in_pixels: [u32; 2],
    ) -> Vec<wgpu::CommandBuffer> {
        let Some(full_output) = self.pending.take() else {
            return Vec::new();
        };

        // 反馈 platform output（光标 / 剪贴板 / IME）回 winit。
        self.winit_state
            .handle_platform_output(window, full_output.platform_output);

        let pixels_per_point = full_output.pixels_per_point;

        // V5 阶段 B.7：tessellate 计时（Shape → ClippedPrimitive 是 CPU 大头）。
        let t_tess = Instant::now();
        let primitives = self.ctx.tessellate(full_output.shapes, pixels_per_point);
        self.last_stats.tessellate_us = t_tess.elapsed().as_micros() as u32;

        // 几何统计：primitive count + 三角形总数（仅 Mesh 计入；Callback 不算）。
        self.last_stats.primitive_count = primitives.len() as u32;
        self.last_stats.triangle_count = primitives
            .iter()
            .map(|p| match &p.primitive {
                egui::epaint::Primitive::Mesh(m) => (m.indices.len() / 3) as u32,
                _ => 0,
            })
            .sum();

        let screen_descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels,
            pixels_per_point,
        };

        // V5 阶段 B.7：buffers 计时（update_texture + update_buffers）。
        let t_buf = Instant::now();
        // 1) 上传 / 删除字体图集等纹理增量。
        for (id, image_delta) in &full_output.textures_delta.set {
            self.renderer
                .update_texture(device, queue, *id, image_delta);
        }
        // 2) 把顶点 / 索引 / uniform 写到 GPU。可能产生 user paint callback cbufs。
        let user_cbufs =
            self.renderer
                .update_buffers(device, queue, encoder, &primitives, &screen_descriptor);
        self.last_stats.buffers_us = t_buf.elapsed().as_micros() as u32;

        // V5 阶段 B.7：paint 计时（render pass record + free_texture）。
        let t_paint = Instant::now();
        // 3) 开 render pass 把 egui 画到 target_view 上。
        {
            let render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("egui_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            // wgpu 24 + egui_wgpu 0.31 的 render() 要 `&mut RenderPass<'static>`。
            let mut render_pass = render_pass.forget_lifetime();
            self.renderer
                .render(&mut render_pass, &primitives, &screen_descriptor);
        }

        // 4) 释放本帧不再需要的字体纹理。
        for id in &full_output.textures_delta.free {
            self.renderer.free_texture(id);
        }
        self.last_stats.paint_us = t_paint.elapsed().as_micros() as u32;

        // total = begin（B.7 在 begin_frame 已锁定） + tess + buffers + paint。
        self.last_stats.total_us = self.last_stats.begin_us
            + self.last_stats.tessellate_us
            + self.last_stats.buffers_us
            + self.last_stats.paint_us;
        self.pending_begin_started = None;

        user_cbufs
    }
}

#[cfg(test)]
mod tests {
    /// B.1 烟雾测试：仅校验 egui 上游 API 可达。
    #[test]
    fn egui_context_constructs() {
        let _ctx = super::egui::Context::default();
    }

    /// B.2 烟雾测试：UiState 字段 `pending` 默认为 `None`，begin_frame 会写入，
    /// paint 取走。这里只校验类型可达，不构造 GPU device（CI 通常无 GPU）。
    #[test]
    fn ui_state_type_is_sendable() {
        // 仅类型层面：UiState 在 *Send* 上没承诺，但起码能命名。
        fn assert_named<T>() {}
        assert_named::<super::UiState>();
    }
}
