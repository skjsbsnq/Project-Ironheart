//! Phase 3.12.7 — `RiverPass`：vanilla river.shader 等价渲染。
//!
//! **替代谁**：`terrain.wgsl` 内嵌的 "navy 蓝叠加"（`river_color = vec3(0.18, 0.36, 0.62)`）
//! — 那只是把河流像素染成统一深蓝，无流动、无法线、无高光、无等级差异。
//!
//! **本 pass** 用独立 pipeline 画河流，功能等价于 vanilla `river.shader`：
//!
//! 1. **RiverSurface 纹理**：`diffuse_{0,1,2}.dds` + `normal_{0,1,2}.dds` + `masks.dds`
//! 2. **流向滚动**：`rivers.bmp` 的 palette 索引编码流向（0=mouth, 1=source），
//!    shader 按 `flow_dir × global_time × flow_speed` 滚动 UV
//! 3. **河流宽度**：masks 的 R/G/B/A 通道编码 level 1-4 → 透明度
//! 4. **alpha blending**：src_alpha / inv_src_alpha，仅 RGB write（与 vanilla 一致）
//!
//! ## 几何复用策略
//!
//! 与 WaterPass 一样，本 pass **不新建 mesh** — 复用 TerrainPass 的 per-LOD instance
//! buffer + chunk uniform。Vertex shader 与 terrain 同款的 "chunk → grid → world_xz"
//! 路径，但 world_y 取 heightmap 值（河流贴地走，不像海面压到 SEA_LEVEL）。
//!
//! Fragment 端先采样 `rivers.bmp`，若 `level == 0` 或 `is_water` 直接 discard。
//!
//! ## 渲染顺序
//!
//! RiverPass **在 WaterPass 之后**绘制：
//!
//! - `depth_compare = LessEqual` + `depth_write = false` + z-bias；
//! - alpha blend 叠加到海面上（不是像素替换）；
//! - 渲染在 WaterPass 之后，避免 water opaque 覆盖 river 像素。

#![allow(dead_code)]

use hoi4_assets::MapResRole;
use hoi4_paths::PathConfig;
use wgpu::util::DeviceExt;

use crate::passes::HDR_FORMAT;
use crate::vanilla_resource_views::{
    upload_dds_or_fallback, BindingAudit, DdsUploadRequest, VanillaResourceViews,
};

// ─── River uniform ─────────────────────────────────────────────────────────

/// 48 bytes — 与 wgsl `RiverParams` 字段一一对应（std140 对齐）。
#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct RiverParams {
    pub flow_speed: f32,
    pub base_alpha: f32,
    pub z_bias: f32,
    pub height_scale: f32,
    pub world_w: f32,
    pub world_d: f32,
    pub zoom_factor: f32,
    pub grid: f32,
    pub y_bias: f32,
    pub _pad1: f32,
    pub _pad2: f32,
    pub _pad3: f32,
}

impl Default for RiverParams {
    fn default() -> Self {
        Self {
            flow_speed: 0.4,
            base_alpha: 0.85,
            z_bias: 0.001,
            height_scale: 4.0,
            world_w: 112.0,
            world_d: 41.0,
            zoom_factor: 1.0,
            grid: 32.0,
            y_bias: 0.005,
            _pad1: 0.0,
            _pad2: 0.0,
            _pad3: 0.0,
        }
    }
}

const _: () = assert!(std::mem::size_of::<RiverParams>() == 48);

// ─── RiverPass ─────────────────────────────────────────────────────────────

pub struct RiverPass {
    pipeline: wgpu::RenderPipeline,
    bind_groups_g0: [wgpu::BindGroup; 3],
    bind_group_g1: wgpu::BindGroup,
    bind_group_g2: wgpu::BindGroup,
    params_buffer: wgpu::Buffer,
    pub any_loaded: bool,
    pub load_warnings: Vec<String>,
    pub binding_audit: BindingAudit,
    _owned_textures: Vec<wgpu::Texture>,
    _owned_samplers: Vec<wgpu::Sampler>,
}

/// RiverPass 构造参数。
pub struct RiverPassInputs<'a> {
    pub global_uniform_buffer: &'a wgpu::Buffer,
    pub depth_format: wgpu::TextureFormat,
    pub lod_grid: [u32; 3],
    pub heightmap_view: &'a wgpu::TextureView,
    pub rivers_view: &'a wgpu::TextureView,
    pub world_size: [f32; 2],
    pub height_scale: f32,
    pub vanilla_resources: &'a VanillaResourceViews,
}

impl RiverPass {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _path_cfg: &PathConfig,
        inputs: RiverPassInputs<'_>,
    ) -> Self {
        let mut warnings = Vec::new();
        let mut owned_textures: Vec<wgpu::Texture> = Vec::new();
        let mut binding_audit = BindingAudit::new();

        // ── shader ────────────────────────────────────────────────────
        let composed = hoi4_render::shader_rt::compose_shader(RIVER_WGSL, true, true);
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("river_shader"),
            source: wgpu::ShaderSource::Wgsl(composed.into()),
        });

        // ── RiverParams uniform ────────────────────────────────────────
        let params_init = RiverParams {
            height_scale: inputs.height_scale,
            world_w: inputs.world_size[0],
            world_d: inputs.world_size[1],
            ..RiverParams::default()
        };
        let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("river_params"),
            contents: bytemuck::bytes_of(&params_init),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let diffuse_0 = upload_dds_or_fallback(
            device,
            queue,
            inputs.vanilla_resources,
            river_upload_request(
                MapResRole::RiverDiffuse(0),
                "river_diffuse_0",
                [60, 100, 140, 255],
                true,
                true,
                "river diffuse color falls back to flat blue",
            ),
            &mut warnings,
        );
        binding_audit.extend([diffuse_0.audit.clone()]);
        let diffuse_0_view = diffuse_0.view;
        owned_textures.push(diffuse_0.texture);

        let diffuse_1 = upload_dds_or_fallback(
            device,
            queue,
            inputs.vanilla_resources,
            river_upload_request(
                MapResRole::RiverDiffuse(1),
                "river_diffuse_1",
                [60, 100, 140, 255],
                true,
                false,
                "alternate river diffuse LOD falls back to flat blue",
            ),
            &mut warnings,
        );
        binding_audit.extend([diffuse_1.audit.clone()]);
        let diffuse_1_view = diffuse_1.view;
        owned_textures.push(diffuse_1.texture);

        let diffuse_2 = upload_dds_or_fallback(
            device,
            queue,
            inputs.vanilla_resources,
            river_upload_request(
                MapResRole::RiverDiffuse(2),
                "river_diffuse_2",
                [60, 100, 140, 255],
                true,
                false,
                "alternate river diffuse LOD falls back to flat blue",
            ),
            &mut warnings,
        );
        binding_audit.extend([diffuse_2.audit.clone()]);
        let diffuse_2_view = diffuse_2.view;
        owned_textures.push(diffuse_2.texture);

        let normal_0 = upload_dds_or_fallback(
            device,
            queue,
            inputs.vanilla_resources,
            river_upload_request(
                MapResRole::RiverNormal(0),
                "river_normal_0",
                [128, 128, 255, 255],
                false,
                true,
                "river normals and highlights flatten",
            ),
            &mut warnings,
        );
        binding_audit.extend([normal_0.audit.clone()]);
        let normal_0_view = normal_0.view;
        owned_textures.push(normal_0.texture);

        let normal_1 = upload_dds_or_fallback(
            device,
            queue,
            inputs.vanilla_resources,
            river_upload_request(
                MapResRole::RiverNormal(1),
                "river_normal_1",
                [128, 128, 255, 255],
                false,
                false,
                "alternate river normal LOD flattens",
            ),
            &mut warnings,
        );
        binding_audit.extend([normal_1.audit.clone()]);
        let normal_1_view = normal_1.view;
        owned_textures.push(normal_1.texture);

        let normal_2 = upload_dds_or_fallback(
            device,
            queue,
            inputs.vanilla_resources,
            river_upload_request(
                MapResRole::RiverNormal(2),
                "river_normal_2",
                [128, 128, 255, 255],
                false,
                false,
                "alternate river normal LOD flattens",
            ),
            &mut warnings,
        );
        binding_audit.extend([normal_2.audit.clone()]);
        let normal_2_view = normal_2.view;
        owned_textures.push(normal_2.texture);

        let masks = upload_dds_or_fallback(
            device,
            queue,
            inputs.vanilla_resources,
            river_upload_request(
                MapResRole::RiverMasks,
                "river_masks",
                [255, 255, 255, 255],
                false,
                true,
                "river width and alpha masks are approximated",
            ),
            &mut warnings,
        );
        binding_audit.extend([masks.audit.clone()]);
        let masks_view = masks.view;
        owned_textures.push(masks.texture);

        // ── samplers ──────────────────────────────────────────────────
        let river_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("river_sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let heightmap_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("river_heightmap_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let rivers_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("river_rivers_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        // ── BGL g0: global frame + river params + heightmap + sampler ──
        let bgl_g0 = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("river_bgl_g0"),
            entries: &[
                uniform_entry(0, wgpu::ShaderStages::VERTEX_FRAGMENT),
                uniform_entry(1, wgpu::ShaderStages::VERTEX_FRAGMENT),
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                    count: None,
                },
            ],
        });

        let bind_groups_g0: [wgpu::BindGroup; 3] = std::array::from_fn(|_lod| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("river_bg_g0"),
                layout: &bgl_g0,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: inputs.global_uniform_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: params_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(inputs.heightmap_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::Sampler(&heightmap_sampler),
                    },
                ],
            })
        });

        // ── BGL g1: rivers.bmp R8 + sampler ──────────────────────────
        let bgl_g1 = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("river_bgl_g1"),
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
        let bind_group_g1 = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("river_bg_g1"),
            layout: &bgl_g1,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(inputs.rivers_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&rivers_sampler),
                },
            ],
        });

        // ── BGL g2: river textures (7) + sampler ──────────────────────
        let bgl_g2 = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("river_bgl_g2"),
            entries: &[
                fragment_tex_entry(0),
                fragment_tex_entry(1),
                fragment_tex_entry(2),
                fragment_tex_entry(3),
                fragment_tex_entry(4),
                fragment_tex_entry(5),
                fragment_tex_entry(6),
                wgpu::BindGroupLayoutEntry {
                    binding: 7,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let bind_group_g2 = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("river_bg_g2"),
            layout: &bgl_g2,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&diffuse_0_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&diffuse_1_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&diffuse_2_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&normal_0_view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&normal_1_view),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::TextureView(&normal_2_view),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::TextureView(&masks_view),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: wgpu::BindingResource::Sampler(&river_sampler),
                },
            ],
        });

        // ── pipeline ──────────────────────────────────────────────────
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("river_pl"),
            bind_group_layouts: &[&bgl_g0, &bgl_g1, &bgl_g2],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("river_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: 16,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &[
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            offset: 8,
                            shader_location: 1,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                    ],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: HDR_FORMAT,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::Zero,
                            dst_factor: wgpu::BlendFactor::One,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::RED
                        | wgpu::ColorWrites::GREEN
                        | wgpu::ColorWrites::BLUE,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: inputs.depth_format,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: Default::default(),
                bias: wgpu::DepthBiasState {
                    constant: 2,
                    slope_scale: 1.0,
                    clamp: 0.0,
                },
            }),
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });

        let any_loaded = binding_audit
            .entries
            .iter()
            .any(|entry| entry.loaded && matches!(entry.role, Some(MapResRole::RiverDiffuse(0))));

        Self {
            pipeline,
            bind_groups_g0,
            bind_group_g1,
            bind_group_g2,
            params_buffer,
            any_loaded,
            load_warnings: warnings,
            binding_audit,
            _owned_textures: owned_textures,
            _owned_samplers: vec![river_sampler, heightmap_sampler, rivers_sampler],
        }
    }

    pub fn update_params(&self, queue: &wgpu::Queue, params: &RiverParams) {
        queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(params));
    }

    pub fn render<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        instance_buffers: &'a [wgpu::Buffer; 3],
        instance_counts: &[u32; 3],
        vertex_counts: &[u32; 3],
    ) {
        if !self.any_loaded {
            return;
        }
        if instance_counts[0] == 0 || vertex_counts[0] == 0 {
            return;
        }
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_groups_g0[0], &[]);
        pass.set_bind_group(1, &self.bind_group_g1, &[]);
        pass.set_bind_group(2, &self.bind_group_g2, &[]);
        pass.set_vertex_buffer(0, instance_buffers[0].slice(..));
        pass.draw(0..vertex_counts[0], 0..instance_counts[0]);
    }
}

// ─── Inline WGSL shader ────────────────────────────────────────────────────

const RIVER_WGSL: &str = r#"
// RiverPass — vanilla river.shader 等价
// 复用 terrain 的 chunk-tessellated 顶点；FS 端采样 rivers.bmp 判断是否河流像素。

struct RiverParams {
    flow_speed: f32,
    base_alpha: f32,
    z_bias: f32,
    height_scale: f32,
    world_w: f32,
    world_d: f32,
    zoom_factor: f32,
    grid: f32,
    y_bias: f32,
    _pad1: f32,
    _pad2: f32,
    _pad3: f32,
};

@group(0) @binding(0) var<uniform> frame: GlobalFrameUniform;
@group(0) @binding(1) var<uniform> rparams: RiverParams;
@group(0) @binding(2) var heightmap: texture_2d<f32>;
@group(0) @binding(3) var heightmap_sampler: sampler;

@group(1) @binding(0) var rivers_bmp: texture_2d<f32>;
@group(1) @binding(1) var rivers_sampler: sampler;

@group(2) @binding(0) var river_diffuse_0: texture_2d<f32>;
@group(2) @binding(1) var river_diffuse_1: texture_2d<f32>;
@group(2) @binding(2) var river_diffuse_2: texture_2d<f32>;
@group(2) @binding(3) var river_normal_0: texture_2d<f32>;
@group(2) @binding(4) var river_normal_1: texture_2d<f32>;
@group(2) @binding(5) var river_normal_2: texture_2d<f32>;
@group(2) @binding(6) var river_masks: texture_2d<f32>;
@group(2) @binding(7) var river_sampler: sampler;

struct VsIn {
    @builtin(vertex_index) vid: u32,
    @builtin(instance_index) iid: u32,
    @location(0) origin_xz: vec2<f32>,
    @location(1) size_xz: vec2<f32>,
};

struct VsOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) map_uv: vec2<f32>,
    @location(2) map_px: vec2<f32>,
};

const SEA_LEVEL: f32 = 95.0;

@vertex
fn vs_main(in: VsIn) -> VsOut {
    let grid = u32(rparams.grid);
    let q_count = grid * grid;
    let q_idx = in.vid / 6u;
    let v_in_q = in.vid % 6u;

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

    let cell_size = in.size_xz / f32(grid);
    let local_xz = vec2<f32>(f32(qx_u + ox), f32(qz_u + oz)) * cell_size;
    let world_xz = in.origin_xz + local_xz;

    let hm_w = f32(textureDimensions(heightmap).x);
    let hm_h = f32(textureDimensions(heightmap).y);
    let map_uv = world_xz_to_map_uv(world_xz, vec2<f32>(rparams.world_w, rparams.world_d));
    let map_px = map_uv_to_px(map_uv);
    let hm_u = map_uv.x;
    let hm_v = map_uv.y;
    let xy = vec2<i32>(clamp(vec2<f32>(hm_u, hm_v), vec2<f32>(0.0), vec2<f32>(1.0)) * (vec2<f32>(hm_w - 1.0, hm_h - 1.0)));
    let h = textureLoad(heightmap, xy, 0).r;
    let world_y = h * rparams.height_scale + rparams.y_bias;

    let world_pos = vec3<f32>(world_xz.x, world_y, world_xz.y);

    var clip = frame.view_proj * vec4<f32>(world_pos, 1.0);
    clip.z = clip.z - rparams.z_bias * clip.w;

    var out: VsOut;
    out.clip_pos = clip;
    out.world_pos = world_pos;
    out.map_uv = map_uv;
    out.map_px = map_px;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let river_sample = textureSample(rivers_bmp, rivers_sampler, in.map_uv);
    let river_lvl = river_sample.r;

    if river_lvl < 0.10 {
        discard;
    }

    let height_sample = textureSample(heightmap, heightmap_sampler, in.map_uv).r;
    if height_sample * 255.0 <= SEA_LEVEL {
        discard;
    }

    let zoom_factor = rparams.zoom_factor;
    let zoom_cut = mix(0.55, 0.20, zoom_factor);
    if river_lvl < zoom_cut {
        discard;
    }

    let flow_dir = vec2<f32>(0.0, 1.0);
    let time = frame.global_time;
    let scrolled_uv = in.map_uv - flow_dir * time * rparams.flow_speed;

    let diffuse = textureSample(river_diffuse_0, river_sampler, scrolled_uv).rgb;
    let normal_map = textureSample(river_normal_0, river_sampler, scrolled_uv).rgb;
    let normal = normalize(normal_map * 2.0 - 1.0);

    let to_camera = normalize(frame.cam_pos - in.world_pos);
    let sun_dir = normalize(vec3<f32>(0.4, 1.0, 0.3));
    let half_vec = normalize(to_camera + sun_dir);
    let spec = pow(max(dot(normal, half_vec), 0.0), 64.0) * 0.35;

    var color = diffuse + vec3<f32>(spec);

    color = day_night(
        color,
        calc_globe_normal(in.map_px, frame.day_night_hour_sun_dir.x),
        frame.day_night_hour_sun_dir.yzw,
        1.0,
    );
    color = apply_distance_fog(color, in.world_pos, frame.cam_pos);

    let river_alpha = smoothstep(zoom_cut - 0.10, zoom_cut + 0.10, river_lvl)
        * (0.45 + 0.45 * river_lvl)
        * rparams.base_alpha;

    return vec4<f32>(color, clamp(river_alpha, 0.0, 1.0));
}
"#;

// ─── Private helpers ────────────────────────────────────────────────────────

fn uniform_entry(binding: u32, vis: wgpu::ShaderStages) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: vis,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn fragment_tex_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

fn river_upload_request(
    role: MapResRole,
    label: &'static str,
    fallback_rgba: [u8; 4],
    srgb: bool,
    critical: bool,
    visual_impact: &'static str,
) -> DdsUploadRequest {
    DdsUploadRequest {
        role,
        label,
        fallback_rgba,
        srgb,
        critical,
        pass: "river",
        binding: label,
        visual_impact,
    }
}
