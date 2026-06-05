//! Phase I — CR-1.2 — `Hoi3CounterPass`：HOI3 风格屏幕空间兵牌渲染骨架。
//!
//! 替代旧 [`crate::passes::map_symbol::MapSymbolPass`] 的"3D 世界空间贴地 quad
//! + vanilla DDS 4 层展开"，改为：
//!
//! - **屏幕空间正交**：VS 把 `screen_pos + size * corner` 直接除以屏幕尺寸 → NDC。
//! - **单层 instanced quad**：每个兵牌一个 6 顶点四边形，无 4 层 instance 展开。
//! - **零 vanilla 资源依赖**：纯 procedural shader，CR-2 才接 SVG → atlas。
//!
//! ## 默认启用（CR-5）
//!
//! [`Hoi3CounterPass::new`] 把 `enabled = true`，F8 toggle 可关闭。
//!
//! ## 渲染目标 / 混合
//!
//! 写入 HDR target（`Rgba16Float`）。Alpha 启用 SrcAlpha / OneMinusSrcAlpha 混合，
//! 让叠层底牌（`flags.IS_UNDERLAY`）与顶牌平滑叠加。深度只读 — 兵牌总是压在地形 /
//! POI / 边界之上。

use hoi4_render::counter_atlas::CounterAtlas;
use hoi4_render::counter_v3::Hoi3CounterInstance;
use wgpu::util::DeviceExt;

use crate::passes::Pass;

/// `Hoi3CounterPass` 的屏幕尺寸 uniform（16 字节）。
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable, Debug)]
struct Hoi3CounterUniforms {
    screen_size: [f32; 2],
    /// 当前帧物理像素尺寸（与 [`Hoi3CounterInstance::screen_pos`] 同坐标）。
    opacity: f32,
    time_secs: f32,
    view_proj: [[f32; 4]; 4],
}

/// HOI3 风格屏幕空间兵牌渲染管线。
pub struct Hoi3CounterPass {
    pipeline: wgpu::RenderPipeline,

    /// 6 顶点单位 quad（[0,1]² 角点）。
    quad_vb: wgpu::Buffer,

    instance_buffer: wgpu::Buffer,
    instance_capacity: u32,
    instance_count: u32,

    /// 屏幕尺寸 uniform。
    uniforms_buffer: wgpu::Buffer,
    uniforms_bg: wgpu::BindGroup,

    /// CR-2.1.3 — 兵种 atlas 纹理 + sampler bind group（@group(1)）。
    /// `_atlas` 字段保留所有权（texture / sampler 不能 drop）。
    _atlas: CounterAtlas,
    atlas_bg: wgpu::BindGroup,

    enabled: bool,
}

impl Pass for Hoi3CounterPass {
    fn name(&self) -> &'static str {
        "hoi3_counter_v3"
    }
    fn enabled(&self) -> bool {
        self.enabled
    }
}

impl Hoi3CounterPass {
    /// CR-1 阶段默认 instance 容量（足以容纳 1939 巴巴罗萨 ~600 师 + 叠层底牌 4×）。
    pub const DEFAULT_INITIAL_CAPACITY: u32 = 4096;

    /// 创建 pass。`target_format` 必须是 HDR target 格式（`Rgba16Float`）。
    /// `initial_capacity` ≥ 64；过小会被夹到 64。
    ///
    /// CR-2.1.3：构造时同步加载 [`CounterAtlas`]（嵌入 PNG → wgpu 纹理）。
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target_format: wgpu::TextureFormat,
        depth_format: wgpu::TextureFormat,
        initial_capacity: u32,
    ) -> Self {
        let cap = initial_capacity.max(64);

        // ─── Shader module ────────────────────────────────────────────────
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("counter_v3_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("counter_v3.wgsl").into()),
        });

        // ─── Bind group layout group(0): screen-size uniform ──────────────
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("counter_v3_bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        // ─── Bind group layout group(1): counter atlas texture + sampler ──
        // CR-2.1.3 — Hoi3CounterPass 持有 atlas，FS 中读取 `archetype` 对应
        // cell。可视性：仅 FS。
        let atlas_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("counter_v3_atlas_bgl"),
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
            ],
        });

        let uniforms_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("counter_v3_uniforms"),
            contents: bytemuck::bytes_of(&Hoi3CounterUniforms {
                screen_size: [1.0, 1.0],
                opacity: 1.0,
                time_secs: 0.0,
                view_proj: glam::Mat4::IDENTITY.to_cols_array_2d(),
            }),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let uniforms_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("counter_v3_uniforms_bg"),
            layout: &bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniforms_buffer.as_entire_binding(),
            }],
        });

        // ─── Quad vertex buffer (6 verts, [0,1]² corner) ──────────────────
        #[repr(C)]
        #[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
        struct QuadVert {
            corner: [f32; 2],
        }
        // 三角形 1：(0,0) (1,0) (0,1)；三角形 2：(0,1) (1,0) (1,1)
        let quad_verts: [QuadVert; 6] = [
            QuadVert { corner: [0.0, 0.0] },
            QuadVert { corner: [1.0, 0.0] },
            QuadVert { corner: [0.0, 1.0] },
            QuadVert { corner: [0.0, 1.0] },
            QuadVert { corner: [1.0, 0.0] },
            QuadVert { corner: [1.0, 1.0] },
        ];
        let quad_vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("counter_v3_quad_vb"),
            contents: bytemuck::cast_slice(&quad_verts),
            usage: wgpu::BufferUsages::VERTEX,
        });

        // ─── Pipeline layout ──────────────────────────────────────────────
        // group(0): screen-size uniform；group(1): atlas texture + sampler。
        let pl_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("counter_v3_pl"),
            bind_group_layouts: &[&bgl, &atlas_bgl],
            push_constant_ranges: &[],
        });

        // ─── Render pipeline ──────────────────────────────────────────────
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("counter_v3_pipeline"),
            layout: Some(&pl_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[
                    // Vertex buffer 0: per-vertex corner attribute.
                    wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<QuadVert>() as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &[wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x2,
                            offset: 0,
                            shader_location: 0,
                        }],
                    },
                    // Vertex buffer 1: per-instance Hoi3CounterInstance.
                    wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<Hoi3CounterInstance>() as u64,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &[
                            // screen_pos @ 0  → loc 1
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Float32x2,
                                offset: 0,
                                shader_location: 1,
                            },
                            // size @ 8 → loc 2
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Float32x2,
                                offset: 8,
                                shader_location: 2,
                            },
                            // country_color @ 16 (Unorm8x4 → vec4<f32> 0..1) → loc 3
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Unorm8x4,
                                offset: 16,
                                shader_location: 3,
                            },
                            // state_pack: archetype | flags | stack_count | organisation
                            // @ 20 (Uint8x4 → vec4<u32>) → loc 4
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Uint8x4,
                                offset: 20,
                                shader_location: 4,
                            },
                            // hierarchy_pack: strength | exp_level | hierarchy_level | _pad0
                            // @ 24 (Uint8x4 → vec4<u32>) → loc 5
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Uint8x4,
                                offset: 24,
                                shader_location: 5,
                            },
                            // screen_offset @ 40 → loc 6
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Float32x2,
                                offset: 40,
                                shader_location: 6,
                            },
                            // world_pos @ 48 → loc 7
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Float32x3,
                                offset: 48,
                                shader_location: 7,
                            },
                            // motion_delta @ 64 → loc 8
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Float32x3,
                                offset: 64,
                                shader_location: 8,
                            },
                            // motion_times @ 76 → loc 9
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Float32x2,
                                offset: 76,
                                shader_location: 9,
                            },
                        ],
                    },
                ],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            // 屏幕空间兵牌位于最近平面，深度只读以避免被高地形遮挡。
            depth_stencil: Some(wgpu::DepthStencilState {
                format: depth_format,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Always,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });

        // ─── Instance buffer ──────────────────────────────────────────────
        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("counter_v3_instances"),
            size: (cap as u64) * (std::mem::size_of::<Hoi3CounterInstance>() as u64),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // ─── CR-2.1.3 — atlas 加载 + bind group ───────────────────────────
        let atlas = CounterAtlas::new(device, queue);
        let atlas_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("counter_v3_atlas_bg"),
            layout: &atlas_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&atlas.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&atlas.sampler),
                },
            ],
        });

        Self {
            pipeline,
            quad_vb,
            instance_buffer,
            instance_capacity: cap,
            instance_count: 0,
            uniforms_buffer,
            uniforms_bg,
            _atlas: atlas,
            atlas_bg,
            // CR-5：默认启用，F8 可切换关闭。
            enabled: true,
        }
    }

    /// 当前实例数（已上传，未画废丢弃）。
    pub fn instance_count(&self) -> u32 {
        self.instance_count
    }

    /// 是否启用渲染。F4 toggle 由 main 主循环维护。
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// 切换启用状态（true ↔ false）。
    pub fn toggle(&mut self) -> bool {
        self.enabled = !self.enabled;
        self.enabled
    }

    /// 每帧调用一次。`screen_w` / `screen_h` 必须是物理像素尺寸（与
    /// [`Hoi3CounterInstance::screen_pos`] 一致；`hoi4_render::counter_v3::generate_hoi3_counters_v0`
    /// 输出已经在物理像素坐标系）。
    pub fn upload(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        instances: &[Hoi3CounterInstance],
        screen_w: f32,
        screen_h: f32,
        view_proj: [[f32; 4]; 4],
        time_secs: f32,
    ) {
        // 屏幕尺寸 uniform 每帧刷新（窗口可能 resize 也走同一路径）。
        let u = Hoi3CounterUniforms {
            screen_size: [screen_w.max(1.0), screen_h.max(1.0)],
            opacity: 1.0,
            time_secs,
            view_proj,
        };
        queue.write_buffer(&self.uniforms_buffer, 0, bytemuck::bytes_of(&u));

        if instances.is_empty() {
            self.instance_count = 0;
            return;
        }

        let needed = instances.len() as u32;
        if needed > self.instance_capacity {
            // 容量不足：按 2 的幂扩容。CR-1 阶段 1939 巴巴罗萨上限 ~2400 不会触发。
            let new_cap = needed.next_power_of_two().max(64);
            self.instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("counter_v3_instances"),
                size: (new_cap as u64) * (std::mem::size_of::<Hoi3CounterInstance>() as u64),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.instance_capacity = new_cap;
        }

        queue.write_buffer(&self.instance_buffer, 0, bytemuck::cast_slice(instances));
        self.instance_count = needed;
    }

    /// 主 render 阶段调用。Pass 已禁用 / 实例数为 0 时直接返回（无 GPU 命令）。
    pub fn update_opacity(
        &self,
        queue: &wgpu::Queue,
        opacity: f32,
        screen_w: f32,
        screen_h: f32,
        view_proj: [[f32; 4]; 4],
        time_secs: f32,
    ) {
        let u = Hoi3CounterUniforms {
            screen_size: [screen_w.max(1.0), screen_h.max(1.0)],
            opacity: opacity.clamp(0.0, 1.0),
            time_secs,
            view_proj,
        };
        queue.write_buffer(&self.uniforms_buffer, 0, bytemuck::bytes_of(&u));
    }

    pub fn render<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>) {
        if !self.enabled || self.instance_count == 0 {
            return;
        }
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.uniforms_bg, &[]);
        pass.set_bind_group(1, &self.atlas_bg, &[]);
        pass.set_vertex_buffer(0, self.quad_vb.slice(..));
        pass.set_vertex_buffer(1, self.instance_buffer.slice(..));
        pass.draw(0..6, 0..self.instance_count);
    }
}

#[cfg(test)]
mod tests {
    use hoi4_render::counter_v3::Hoi3CounterInstance;

    #[test]
    fn instance_size_matches_vertex_layout() {
        // CR-1.2 vertex buffer 的 array_stride 必须与 Rust 端 size_of 一致；
        // 偏移 0/8/16/20/24/40/48/64/76 是硬编码进 Pass::new 的 vertex attribute 表。
        assert_eq!(std::mem::size_of::<Hoi3CounterInstance>(), 84);
    }

    #[test]
    fn uniforms_size_is_80() {
        assert_eq!(std::mem::size_of::<super::Hoi3CounterUniforms>(), 80);
    }
}

#[cfg(test)]
mod shader_tests {
    #[test]
    fn counter_wgsl_naga_parses() {
        let module = naga::front::wgsl::parse_str(include_str!("counter_v3.wgsl"));
        assert!(
            module.is_ok(),
            "counter_v3 WGSL failed naga validation: {:?}",
            module.err()
        );
    }
}
