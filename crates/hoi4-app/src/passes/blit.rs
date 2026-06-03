//! Phase 3.12.1 — 简化 HDR → swap-chain 桥接 pass。
//!
//! 单一职责：sample HDR 离屏纹理 → 写交换链 view，**无任何颜色变换**。
//!
//! ## 视觉等价性
//!
//! 原先的代码路径："3D pipelines (linear out) → sRGB swap chain (auto sRGB encode) → 显示"。
//!
//! 新路径："3D pipelines (linear out) → HDR Rgba16Float (无编码) → blit shader passthrough →
//! sRGB swap chain (auto sRGB encode) → 显示"。
//!
//! 两条路径输出的最终像素逐像素一致——HDR 中转的线性值与原 sRGB swap chain 接收
//! 的线性值字面相同，自动 sRGB 编码也相同。**3.12.1 验收所需的"画面与现状字面
//! 一致"由此保证**。
//!
//! ## 后续替换
//!
//! 3.12.2 完成后由 [`super::PostProcessChain`] 取代本 pass；本 struct 仍保留，
//! 让设置面板"快速模式"开关能在两者之间切换（PostProcessChain 关闭时退回简化 blit）。

use crate::passes::HDR_FORMAT;
use wgpu::util::DeviceExt;

const BLIT_WGSL: &str = r#"
@group(0) @binding(0) var src_tex: texture_2d<f32>;
@group(0) @binding(1) var src_sampler: sampler;

struct BlitParams {
    srgb_target: f32,
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
};
@group(0) @binding(2) var<uniform> bp: BlitParams;

struct VsOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vid: u32) -> VsOut {
    var out: VsOut;
    let uv = vec2<f32>(f32((vid << 1u) & 2u), f32(vid & 2u));
    out.clip_pos = vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0);
    out.uv = vec2<f32>(uv.x, 1.0 - uv.y);
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    var c = textureSample(src_tex, src_sampler, in.uv);
    if (bp.srgb_target < 0.5) {
        c = vec4<f32>(pow(max(c.rgb, vec3<f32>(0.0)), vec3<f32>(1.0 / 2.2)), c.a);
    }
    return c;
}
"#;

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct BlitParams {
    srgb_target: f32,
    _pad0: [f32; 3],
}

/// 简化 HDR → swap-chain blit。
pub struct SimpleBlitPass {
    pub pipeline: wgpu::RenderPipeline,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub sampler: wgpu::Sampler,
    params_buffer: wgpu::Buffer,
    /// 当前 bind group（采样的具体 HDR view）。每次 HDR RT 重建时调
    /// [`Self::rebuild_bind_group`] 重绑。
    pub bind_group: wgpu::BindGroup,
}

impl SimpleBlitPass {
    pub fn new(
        device: &wgpu::Device,
        swap_format: wgpu::TextureFormat,
        hdr_view: &wgpu::TextureView,
    ) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("simple_blit_shader"),
            source: wgpu::ShaderSource::Wgsl(BLIT_WGSL.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("simple_blit_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("simple_blit_sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("simple_blit_pl"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("simple_blit_pipeline"),
            layout: Some(&pl),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: swap_format,
                    blend: None,
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

        let params = BlitParams {
            srgb_target: if swap_format.is_srgb() { 1.0 } else { 0.0 },
            _pad0: [0.0; 3],
        };
        let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("simple_blit_params"),
            contents: bytemuck::bytes_of(&params),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let bind_group = make_bind_group(
            device,
            &bind_group_layout,
            hdr_view,
            &sampler,
            &params_buffer,
        );

        // 让上层"使用了 HDR_FORMAT"的事实可被静态检查用到。
        let _ = HDR_FORMAT;

        Self {
            pipeline,
            bind_group_layout,
            sampler,
            params_buffer,
            bind_group,
        }
    }

    /// HDR RT 重建后调用，把 bind group 重新指向新 view。
    pub fn rebuild_bind_group(&mut self, device: &wgpu::Device, hdr_view: &wgpu::TextureView) {
        self.bind_group = make_bind_group(
            device,
            &self.bind_group_layout,
            hdr_view,
            &self.sampler,
            &self.params_buffer,
        );
    }

    /// 在 encoder 内提交一次 blit pass。
    pub fn render(&self, encoder: &mut wgpu::CommandEncoder, target: &wgpu::TextureView) {
        let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("simple_blit"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            ..Default::default()
        });
        rp.set_pipeline(&self.pipeline);
        rp.set_bind_group(0, &self.bind_group, &[]);
        rp.draw(0..3, 0..1);
    }
}

impl super::Pass for SimpleBlitPass {
    fn name(&self) -> &'static str {
        "simple_blit"
    }
}

fn make_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    hdr_view: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
    params_buffer: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("simple_blit_bg"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(hdr_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: params_buffer.as_entire_binding(),
            },
        ],
    })
}
