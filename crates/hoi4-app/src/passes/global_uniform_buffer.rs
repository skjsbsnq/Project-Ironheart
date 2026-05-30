//! Phase 3.12.1 — `GlobalFrameUniform` 共享 GPU buffer。
//!
//! 把 [`hoi4_render::global_uniform::GlobalFrameUniform`] 在 GPU 上的存放生命周期
//! 集中到一个 wrapper：每帧 main.rs 调一次 [`GlobalUniformBuffer::write`]，所有
//! 后续 3.12 pass 通过该 buffer 的 `bind_group_layout_entry` / `binding_resource`
//! 接到自己的 `@group(0) @binding(0)`。
//!
//! ## 设计点
//!
//! - 共享 binding：避免每个 pass 各自上传一份相同数据
//! - 静态 layout：bind group layout entry 暴露成关联函数 [`Self::bind_group_layout_entry`]，
//!   方便后续 pass 在自己的 BGL 里把 group 0 第 0 项填同一种声明，确保 `set_bind_group(0, ...)`
//!   能跨 pass 共用同一个 group（如果上层愿意构造 group 0）。
//! - **本节不强制 group 0 共享**——3.12.1 的简化 blit / 现有 3D pass 都还各自管 group，
//!   等 3.12.4 起替换 `pdxmap` pipeline 时再统一接到这套 layout。

use hoi4_render::global_uniform::{GlobalFrameUniform, GLOBAL_FRAME_UNIFORM_SIZE};

/// `GlobalFrameUniform` 的 GPU 端 owner。
pub struct GlobalUniformBuffer {
    pub buffer: wgpu::Buffer,
}

impl GlobalUniformBuffer {
    /// 用默认值（identity view-proj、白光、未启用阴影）初始化。
    pub fn new(device: &wgpu::Device) -> Self {
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("global_frame_uniform"),
            size: GLOBAL_FRAME_UNIFORM_SIZE as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        // 写一次默认值，避免读取 GPU 未初始化内存
        let init = GlobalFrameUniform::default();
        let bytes = bytemuck::bytes_of(&init);
        // 这里没有 queue 上下文，所以仅创建；真正首帧值由 main.rs 在 prepare 阶段
        // 调 `write` 写入。
        let _ = bytes; // silence unused
        Self { buffer }
    }

    /// 每帧更新。
    pub fn write(&self, queue: &wgpu::Queue, uniform: &GlobalFrameUniform) {
        queue.write_buffer(&self.buffer, 0, bytemuck::bytes_of(uniform));
    }

    /// `@group(0) @binding(0)` 的 BGL entry 描述（visibility = VS+FS+CS）。
    pub fn bind_group_layout_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
        wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT | wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: wgpu::BufferSize::new(GLOBAL_FRAME_UNIFORM_SIZE as u64),
            },
            count: None,
        }
    }

    /// 拿到 binding 对应的 `BindingResource`，方便上层组装 BindGroup。
    pub fn binding_resource(&self) -> wgpu::BindingResource<'_> {
        self.buffer.as_entire_binding()
    }
}
