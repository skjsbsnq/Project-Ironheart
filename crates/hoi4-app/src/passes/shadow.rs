//! Phase 3.12.3 — directional shadow caster pass。
//!
//! 单级 shadow map（vanilla 同款，对 RTS 视角足够）。本 pass 把地形几何（与
//! 主 terrain pipeline 共享的 instance buffer + heightmap）从太阳视角投到
//! 一张 D32Float 2048×2048 的 depth-only RT。
//!
//! ## 当前接入范围（3.12.3）
//!
//! - **Caster**：terrain（LOD0 网格，与主 terrain pipeline 同一份 instance buffer）
//! - **Receive**：留给 3.12.4 / 3.12.5 / 3.12.8（pdxmap / pdxmesh / tree 完整版）
//!   各自把 `shadow_view_proj` + `shadow_pcf` 接到 fragment
//! - **Debug**：F3 在右上角显示 shadow map 灰度（线性深度可视化）
//!
//! ## shadow_view_proj
//!
//! 取相机 target 为 look-at；从 `LIGHT_SHADOW_DIRECTION_{X,Y,Z}` 反方向放一个
//! "光源 eye"，距离为 `world_extent * 1.5`；ortho 边界覆盖 `world_extent * 0.7`
//! 的方形区域 + 高度。computed by [`compute_shadow_view_proj`].
//!
//! ## 坐标系约定
//!
//! - World：X = east, Y = up, Z = south（与 [`hoi4_render::camera`] 一致）
//! - Sun direction: vanilla `LIGHT_SHADOW_DIRECTION = (-5, -8, 5)` — 表示
//!   光线**朝向**这个方向投射，所以光源 eye 在反方向 `(5, 8, -5)` 单位化。
//! - shadow_view_proj 输出 NDC [-1, 1]^3；接收方采样时 `xy * 0.5 + 0.5` 取 UV。

use glam::{Mat4, Vec2, Vec3};

use crate::passes::HDR_FORMAT;

/// 阴影贴图边长（vanilla 同款 2048）。
pub const SHADOW_MAP_SIZE: u32 = 2048;

/// 阴影深度纹理格式。`Depth32Float` 在 dx12/vulkan/metal 普遍支持作为
/// depth attachment + filterable 比较采样。
pub const SHADOW_DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

// ── shader 源 ────────────────────────────────────────────────────────────────

/// 阴影 caster 顶点着色器：与主 terrain pipeline 同一份 instance buffer + 同一
/// 张 heightmap，但用 `shadow_view_proj` 替代 `view_proj`。fragment 空（depth only）。
const CASTER_WGSL: &str = r#"
struct Camera {
    view_proj: mat4x4<f32>,
    eye: vec4<f32>,
    map_size: vec4<f32>,
};

struct ChunkUniform {
    grid: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
};

struct ShadowUniform {
    shadow_view_proj: mat4x4<f32>,
};

@group(0) @binding(0) var<uniform> camera: Camera;
@group(0) @binding(1) var heightmap_tex: texture_2d<f32>;
@group(0) @binding(2) var<uniform> chunk: ChunkUniform;
@group(0) @binding(3) var<uniform> shadow: ShadowUniform;

const SEA_LEVEL: f32 = 95.0 / 255.0;

struct VertexInput {
    @builtin(vertex_index) vid: u32,
    @builtin(instance_index) iid: u32,
    @location(0) origin_xz: vec2<f32>,
    @location(1) size_xz: vec2<f32>,
};

fn latitude_correct(world_xz: vec2<f32>, world_size: vec2<f32>, factor: f32) -> vec2<f32> {
    let v = world_xz.y / max(world_size.y, 0.0001);
    let dist_from_eq = abs(v - 0.5) * 2.0;
    let squash = 1.0 - factor * dist_from_eq * dist_from_eq;
    let centre_z = world_size.y * 0.5;
    let new_z = (world_xz.y - centre_z) * squash + centre_z;
    return vec2<f32>(world_xz.x, new_z);
}

fn load_height(uv: vec2<f32>) -> f32 {
    let dim = vec2<f32>(textureDimensions(heightmap_tex));
    let xy = vec2<i32>(clamp(uv, vec2<f32>(0.0), vec2<f32>(1.0)) * (dim - vec2<f32>(1.0)));
    return textureLoad(heightmap_tex, xy, 0).r;
}

@vertex
fn vs_main(input: VertexInput) -> @builtin(position) vec4<f32> {
    let grid = chunk.grid;
    let q_count = grid * grid;
    let q_idx = input.vid / 6u;
    let v_in_q = input.vid % 6u;

    var qx_u: u32 = q_idx % grid;
    var qz_u: u32 = q_idx / grid;
    if (q_idx >= q_count) {
        qx_u = 0u;
        qz_u = 0u;
    }

    var ox: u32 = 0u;
    var oz: u32 = 0u;
    if (v_in_q == 1u || v_in_q == 2u || v_in_q == 4u) { ox = 1u; }
    if (v_in_q == 2u || v_in_q == 4u || v_in_q == 5u) { oz = 1u; }

    let cell_size = input.size_xz / f32(grid);
    let local_xz = vec2<f32>(f32(qx_u + ox), f32(qz_u + oz)) * cell_size;
    let world_xz_raw = input.origin_xz + local_xz;

    let world_size = camera.map_size.xy;
    let uv = clamp(world_xz_raw / max(world_size, vec2<f32>(0.0001)), vec2<f32>(0.0), vec2<f32>(1.0));
    let world_xz = latitude_correct(world_xz_raw, world_size, camera.map_size.w);

    let h = load_height(uv);
    let height_scale = camera.map_size.z;
    var world_y: f32 = h * height_scale;
    if (h < SEA_LEVEL) {
        world_y = SEA_LEVEL * height_scale;
    }

    return shadow.shadow_view_proj * vec4<f32>(world_xz.x, world_y, world_xz.y, 1.0);
}

// fragment shader: depth-only, no color writes
@fragment
fn fs_main() {}
"#;

/// F3 调试可视化：把 shadow depth 采到右上角小窗口（160×160 px），灰度显示。
const DEBUG_VIEW_WGSL: &str = r#"
@group(0) @binding(0) var depth_tex: texture_depth_2d;
@group(0) @binding(1) var depth_sampler: sampler;

struct VsOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

// 全屏三角形按 sub-rect 重映射：可见 NDC 矩形 [0.55, 0.95] × [0.55, 0.95]。
// 三角形 3 顶点：(0.55, 0.55) / (1.35, 0.55) / (0.55, 1.35)；NDC 裁剪后留下右上角正方形。
@vertex
fn vs_main(@builtin(vertex_index) vid: u32) -> VsOut {
    var out: VsOut;
    let q = vec2<f32>(f32((vid << 1u) & 2u), f32(vid & 2u)); // (0,0) (2,0) (0,2)
    let ndc_xy = vec2<f32>(0.55, 0.55) + q * 0.4;
    out.clip_pos = vec4<f32>(ndc_xy, 0.0, 1.0);
    // UV：屏幕右上正方形的左下角（NDC 0.55,0.55）→ 纹理左下 (0, 1)；
    //     右上角（NDC 0.95,0.95）→ 纹理右上 (1, 0)。三角形端点 (2,1) / (0,-1) 在
    //     NDC 裁剪外，看不到 — 不影响可见正方形的右上正向显示。
    out.uv = vec2<f32>(q.x, 1.0 - q.y);
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // 视觉边框：UV 邻近 0/1 时出现亮黄边框，让用户即使深度图全空也能定位窗口。
    let edge = min(min(in.uv.x, 1.0 - in.uv.x), min(in.uv.y, 1.0 - in.uv.y));
    if (edge < 0.012) {
        return vec4<f32>(1.0, 0.85, 0.2, 1.0); // 鲜黄边框
    }

    let d = textureSample(depth_tex, depth_sampler, in.uv);

    // 默认 ortho 投影下绝大多数地形 depth 集中在 [0.3, 0.8]；线性 stretch 到 [0, 1]
    // 让高低差肉眼可分辨。
    let v = clamp((d - 0.3) / 0.5, 0.0, 1.0);
    return vec4<f32>(v, v, v, 1.0);
}
"#;

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct ShadowUniform {
    shadow_view_proj: [[f32; 4]; 4],
}

/// Phase 3.12.3 ShadowPass。
pub struct ShadowPass {
    pub depth_texture: wgpu::Texture,
    pub depth_view: wgpu::TextureView,
    /// 给后续 pass（pdxmap / pdxmesh）做 PCF 比较采样。
    pub compare_sampler: wgpu::Sampler,
    /// 给 F3 debug view 做线性 depth 采样。
    pub debug_sampler: wgpu::Sampler,

    caster_pipeline: wgpu::RenderPipeline,
    caster_bind_groups: [wgpu::BindGroup; 3],
    shadow_uniform: wgpu::Buffer,

    debug_pipeline: wgpu::RenderPipeline,
    debug_bind_group: wgpu::BindGroup,

    /// 上一帧计算出的矩阵（也写到 GlobalFrameUniform.shadow_view_proj 给接收方）。
    pub last_shadow_view_proj: Mat4,
    /// F3 调试覆盖切换。
    pub debug_visible: bool,
}

impl ShadowPass {
    /// 构造。`camera_buffer` / `heightmap_view` / `chunk_uniforms` 来自主
    /// terrain pipeline 的初始化（在 main.rs `init_render` 里）。
    pub fn new(
        device: &wgpu::Device,
        swap_format: wgpu::TextureFormat,
        camera_buffer: &wgpu::Buffer,
        heightmap_view: &wgpu::TextureView,
        chunk_uniforms: &[wgpu::Buffer; 3],
    ) -> Self {
        // ── 深度 RT ─────────────────────────────────────────────────────
        let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("shadow_depth"),
            size: wgpu::Extent3d {
                width: SHADOW_MAP_SIZE,
                height: SHADOW_MAP_SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: SHADOW_DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let depth_view = depth_texture.create_view(&Default::default());

        // 比较采样器 — 给后续接收方 PCF 用（虽然本节没人 sample，先创建好）
        let compare_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("shadow_compare"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });

        // 普通线性采样器 — F3 debug view 用
        let debug_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("shadow_debug"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // ── shadow_view_proj uniform ────────────────────────────────────
        let shadow_uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("shadow_uniform"),
            size: std::mem::size_of::<ShadowUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        // 写一个 identity，避免 GPU 第一帧读未初始化值
        let _ = ShadowUniform {
            shadow_view_proj: Mat4::IDENTITY.to_cols_array_2d(),
        };

        // ── caster pipeline ─────────────────────────────────────────────
        let caster_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("shadow_caster_shader"),
            source: wgpu::ShaderSource::Wgsl(CASTER_WGSL.into()),
        });
        let caster_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("shadow_caster_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let caster_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("shadow_caster_pl"),
            bind_group_layouts: &[&caster_bgl],
            push_constant_ranges: &[],
        });
        let caster_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("shadow_caster_pipeline"),
            layout: Some(&caster_pl),
            vertex: wgpu::VertexState {
                module: &caster_module,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    // 与主 terrain pipeline 同款 ChunkInstance（origin_xz + size_xz）
                    array_stride: 16, // 4 × f32
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &[
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x2,
                            offset: 0,
                            shader_location: 0,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x2,
                            offset: 8,
                            shader_location: 1,
                        },
                    ],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &caster_module,
                entry_point: Some("fs_main"),
                // depth-only — 没有 color attachment
                targets: &[],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                // 反向剔除（front-face culling）减少 acne — 让阴影 acne 出现在
                // 看不到的背面而非正面。RTS 俯视下"front face culling"普遍 OK。
                cull_mode: Some(wgpu::Face::Front),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: SHADOW_DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: Default::default(),
                bias: wgpu::DepthBiasState {
                    constant: 2,
                    slope_scale: 1.5,
                    clamp: 0.0,
                },
            }),
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });

        let caster_bind_groups: [wgpu::BindGroup; 3] = std::array::from_fn(|i| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("shadow_caster_bg"),
                layout: &caster_bgl,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: camera_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(heightmap_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: chunk_uniforms[i].as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: shadow_uniform.as_entire_binding(),
                    },
                ],
            })
        });

        // ── debug visualization pipeline ────────────────────────────────
        let debug_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("shadow_debug_shader"),
            source: wgpu::ShaderSource::Wgsl(DEBUG_VIEW_WGSL.into()),
        });
        let debug_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("shadow_debug_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
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
        let debug_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("shadow_debug_pl"),
            bind_group_layouts: &[&debug_bgl],
            push_constant_ranges: &[],
        });
        let debug_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("shadow_debug_pipeline"),
            layout: Some(&debug_pl),
            vertex: wgpu::VertexState {
                module: &debug_module,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &debug_module,
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
        let debug_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("shadow_debug_bg"),
            layout: &debug_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&depth_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&debug_sampler),
                },
            ],
        });

        let _ = HDR_FORMAT; // sanity reference

        Self {
            depth_texture,
            depth_view,
            compare_sampler,
            debug_sampler,
            caster_pipeline,
            caster_bind_groups,
            shadow_uniform,
            debug_pipeline,
            debug_bind_group,
            last_shadow_view_proj: Mat4::IDENTITY,
            debug_visible: false,
        }
    }

    /// 每帧调用：用相机 target / 世界范围 / 高度尺度算 shadow_view_proj，
    /// 写进自己的 uniform（同时返回让 main.rs 也写进 GlobalFrameUniform）。
    pub fn update_shadow_view_proj(
        &mut self,
        queue: &wgpu::Queue,
        camera_target: Vec3,
        world_size: Vec2,
        height_scale: f32,
    ) -> Mat4 {
        let m = compute_shadow_view_proj(camera_target, world_size, height_scale);
        self.last_shadow_view_proj = m;
        let payload = ShadowUniform {
            shadow_view_proj: m.to_cols_array_2d(),
        };
        queue.write_buffer(&self.shadow_uniform, 0, bytemuck::bytes_of(&payload));
        m
    }

    /// 渲染 caster pass —— 把传入的每个 LOD 实例 buffer 投到 shadow depth RT。
    pub fn render_caster(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        instance_buffers: &[wgpu::Buffer; 3],
        instance_counts: [u32; 3],
        vertex_count_for_lod: impl Fn(usize) -> u32,
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("shadow_caster"),
            color_attachments: &[],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });
        pass.set_pipeline(&self.caster_pipeline);
        for lod in 0..3 {
            let count = instance_counts[lod];
            if count == 0 {
                continue;
            }
            pass.set_bind_group(0, &self.caster_bind_groups[lod], &[]);
            pass.set_vertex_buffer(0, instance_buffers[lod].slice(..));
            pass.draw(0..vertex_count_for_lod(lod), 0..count);
        }
    }

    /// F3 切换。
    pub fn toggle_debug(&mut self) {
        self.debug_visible = !self.debug_visible;
    }

    /// 把 shadow map 灰度叠到右上角小窗（仅当 `debug_visible == true`）。
    pub fn render_debug(&self, encoder: &mut wgpu::CommandEncoder, target: &wgpu::TextureView) {
        if !self.debug_visible {
            return;
        }
        let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("shadow_debug"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            ..Default::default()
        });
        rp.set_pipeline(&self.debug_pipeline);
        rp.set_bind_group(0, &self.debug_bind_group, &[]);
        rp.draw(0..3, 0..1);
    }
}

impl super::Pass for ShadowPass {
    fn name(&self) -> &'static str {
        "shadow_caster"
    }
}

/// 计算 directional shadow caster 的 view × ortho 投影矩阵。
///
/// 算法：
/// 1. 取 [`hoi4_render::defines::LIGHT_SHADOW_DIRECTION_X/Y/Z`] 作为光线方向，
///    单位化反向得到 light eye 的方向 vector。
/// 2. light eye = camera_target + dir * extent * 1.5（远离场景）。
/// 3. ortho frustum：覆盖 ±extent×0.7 的方形 + 高度上下扩展，让大部分场景能投上阴影。
/// 4. 输出 `proj * view`（mat4）。
///
/// `world_size.x/y` 是地图的 X / Z 维度世界尺度；`height_scale` 是高度方向乘子。
pub fn compute_shadow_view_proj(camera_target: Vec3, world_size: Vec2, height_scale: f32) -> Mat4 {
    use hoi4_render::defines::{
        LIGHT_SHADOW_DIRECTION_X, LIGHT_SHADOW_DIRECTION_Y, LIGHT_SHADOW_DIRECTION_Z,
    };

    let extent = world_size.x.max(world_size.y);
    // 光线方向（指向地表）
    let light_dir = Vec3::new(
        LIGHT_SHADOW_DIRECTION_X,
        LIGHT_SHADOW_DIRECTION_Y,
        LIGHT_SHADOW_DIRECTION_Z,
    )
    .normalize_or_zero();
    if light_dir.length_squared() < 1e-6 {
        return Mat4::IDENTITY;
    }

    // 光源 eye：相机 target 反方向 1.5×extent 处
    let light_eye = camera_target - light_dir * (extent * 1.5);

    // up：与光线方向不平行的稳定向量（vanilla 用 +Z；这里用 +Y 否则 RTS 俯视下退化）
    let up = if light_dir.y.abs() > 0.95 {
        Vec3::Z
    } else {
        Vec3::Y
    };

    let view = Mat4::look_at_rh(light_eye, camera_target, up);

    // ortho：覆盖 0.7×extent 的方形 + 上下高度
    let half = extent * 0.7;
    let near = 0.1;
    let far = extent * 4.0 + height_scale * 4.0;
    let proj = Mat4::orthographic_rh(-half, half, -half, half, near, far);

    proj * view
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shadow_uniform_size_aligns_to_64() {
        // mat4 = 64 bytes，wgpu uniform 16-byte 对齐 OK
        assert_eq!(std::mem::size_of::<ShadowUniform>(), 64);
    }

    #[test]
    fn compute_shadow_view_proj_is_finite_and_nonidentity() {
        let m = compute_shadow_view_proj(Vec3::new(50.0, 0.0, 20.0), Vec2::new(100.0, 40.0), 4.0);
        for c in m.to_cols_array() {
            assert!(c.is_finite(), "shadow_view_proj must be finite");
        }
        assert_ne!(m, Mat4::IDENTITY);
    }

    #[test]
    fn shadow_map_constants_match_roadmap() {
        assert_eq!(SHADOW_MAP_SIZE, 2048);
        assert_eq!(SHADOW_DEPTH_FORMAT, wgpu::TextureFormat::Depth32Float);
    }
}
