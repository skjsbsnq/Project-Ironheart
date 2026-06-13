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

//! Vanilla `river.shader` pass.
//!
//! P1 frame graph facts:
//! - Draws after terrain/border-first and before map layers/water.
//! - Contributes to the main HDR scene color target.
//! - Runs in its own render pass with no depth-stencil attachment because the
//!   traced vanilla river state has depth disabled.
//! - Reuses terrain chunk instances; the fragment shader samples `rivers.bmp`
//!   and the vanilla RiverSurface diffuse/normal/mask textures.
//!
//! The old terrain navy-blue river overlay remains only as fallback when this
//! dedicated pass is unavailable.

#![allow(dead_code)]

use hoi4_assets::MapResRole;
use hoi4_paths::PathConfig;
use wgpu::util::DeviceExt;

use crate::passes::HDR_FORMAT;
use crate::vanilla_resource_views::{
    upload_dds_or_fallback, BindingAudit, BindingAuditEntry, BindingBlockingLevel,
    DdsUploadRequest, VanillaResourceViews,
};
use crate::vanilla_targets::{self, VanillaRuntimeTargets};

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
            flow_speed: 0.035,
            base_alpha: 0.82,
            z_bias: 0.0025,
            height_scale: 4.0,
            world_w: 112.0,
            world_d: 41.0,
            zoom_factor: 1.0,
            grid: 32.0,
            y_bias: 0.018,
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
    params_buffers: [wgpu::Buffer; 3],
    lod_grid: [u32; 3],
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
    pub runtime_targets: &'a VanillaRuntimeTargets,
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
        let params_base = RiverParams {
            height_scale: inputs.height_scale,
            world_w: inputs.world_size[0],
            world_d: inputs.world_size[1],
            ..RiverParams::default()
        };
        let params_buffers: [wgpu::Buffer; 3] = std::array::from_fn(|lod| {
            let mut params = params_base;
            params.grid = inputs.lod_grid[lod] as f32;
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("river_params"),
                contents: bytemuck::bytes_of(&params),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            })
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

        let water_color = upload_dds_or_fallback(
            device,
            queue,
            inputs.vanilla_resources,
            river_upload_request(
                MapResRole::ColormapWater(0),
                "water_color",
                [42, 82, 122, 255],
                true,
                true,
                "river base water color falls back to flat blue-green",
            ),
            &mut warnings,
        );
        binding_audit.extend([water_color.audit.clone()]);
        let water_color_view = water_color.view;
        owned_textures.push(water_color.texture);

        let lean1 = upload_dds_or_fallback(
            device,
            queue,
            inputs.vanilla_resources,
            river_upload_request(
                MapResRole::Lean1,
                "lean1",
                [128, 128, 255, 255],
                false,
                true,
                "river LEAN normal input falls back to flat normal data",
            ),
            &mut warnings,
        );
        binding_audit.extend([lean1.audit.clone()]);
        let lean1_view = lean1.view;
        owned_textures.push(lean1.texture);

        let lean2 = upload_dds_or_fallback(
            device,
            queue,
            inputs.vanilla_resources,
            river_upload_request(
                MapResRole::Lean2,
                "lean2",
                [128, 128, 255, 255],
                false,
                true,
                "river second LEAN normal input falls back to flat normal data",
            ),
            &mut warnings,
        );
        binding_audit.extend([lean2.audit.clone()]);
        let lean2_view = lean2.view;
        owned_textures.push(lean2.texture);

        let citylights = upload_dds_or_fallback(
            device,
            queue,
            inputs.vanilla_resources,
            river_upload_request(
                MapResRole::CityLights(0),
                "citylights_snow_noise",
                [0, 0, 0, 255],
                true,
                false,
                "river citylights/snow-noise input is approximated",
            ),
            &mut warnings,
        );
        binding_audit.extend([citylights.audit.clone()]);
        let citylights_view = citylights.view;
        owned_textures.push(citylights.texture);

        let reflection = upload_dds_or_fallback(
            device,
            queue,
            inputs.vanilla_resources,
            river_upload_request(
                MapResRole::Reflection,
                "reflection",
                [60, 90, 130, 255],
                true,
                false,
                "river reflection contribution falls back to flat reflection data",
            ),
            &mut warnings,
        );
        binding_audit.extend([reflection.audit.clone()]);
        let reflection_view = reflection.view;
        owned_textures.push(reflection.texture);
        binding_audit.extend(vanilla_targets::runtime_target_audit_entries_for_pass(
            "river",
        ));
        binding_audit.extend(river_p5_state_audit_entries());

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
                        resource: params_buffers[_lod].as_entire_binding(),
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

        // ── BGL g1: rivers.bmp RGBA level/flow + sampler ─────────────
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
                fragment_tex_entry(8),
                fragment_tex_entry(9),
                fragment_tex_entry(10),
                fragment_tex_entry(11),
                fragment_tex_entry(12),
                fragment_tex_entry(13),
                fragment_tex_entry_nonfilter(14),
                fragment_tex_entry(15),
                fragment_tex_entry(16),
                fragment_tex_entry(17),
                fragment_tex_entry(18),
                fragment_tex_entry(19),
                fragment_tex_entry(20),
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
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: wgpu::BindingResource::TextureView(&water_color_view),
                },
                wgpu::BindGroupEntry {
                    binding: 9,
                    resource: wgpu::BindingResource::TextureView(&lean1_view),
                },
                wgpu::BindGroupEntry {
                    binding: 10,
                    resource: wgpu::BindingResource::TextureView(&lean2_view),
                },
                wgpu::BindGroupEntry {
                    binding: 11,
                    resource: wgpu::BindingResource::TextureView(
                        &inputs.runtime_targets.province_secondary_color.view,
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 12,
                    resource: wgpu::BindingResource::TextureView(
                        &inputs.runtime_targets.mud_snow.view,
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 13,
                    resource: wgpu::BindingResource::TextureView(&citylights_view),
                },
                wgpu::BindGroupEntry {
                    binding: 14,
                    resource: wgpu::BindingResource::TextureView(
                        &inputs.runtime_targets.light_data.view,
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 15,
                    resource: wgpu::BindingResource::TextureView(
                        &inputs.runtime_targets.light_index.view,
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 16,
                    resource: wgpu::BindingResource::TextureView(
                        &inputs.runtime_targets.gradient_border.ch1.view,
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 17,
                    resource: wgpu::BindingResource::TextureView(
                        &inputs.runtime_targets.gradient_border.ch2.view,
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 18,
                    resource: wgpu::BindingResource::TextureView(
                        &inputs.runtime_targets.gradient_border.ch3.view,
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 19,
                    resource: wgpu::BindingResource::TextureView(&reflection_view),
                },
                wgpu::BindGroupEntry {
                    binding: 20,
                    resource: wgpu::BindingResource::TextureView(
                        &inputs.runtime_targets.projected_shadow_fow.view,
                    ),
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
            depth_stencil: None,
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
            params_buffers,
            lod_grid: inputs.lod_grid,
            any_loaded,
            load_warnings: warnings,
            binding_audit,
            _owned_textures: owned_textures,
            _owned_samplers: vec![river_sampler, heightmap_sampler, rivers_sampler],
        }
    }

    pub fn update_params(&self, queue: &wgpu::Queue, params: &RiverParams) {
        for lod in 0..3 {
            let mut lod_params = *params;
            lod_params.grid = self.lod_grid[lod] as f32;
            queue.write_buffer(
                &self.params_buffers[lod],
                0,
                bytemuck::bytes_of(&lod_params),
            );
        }
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
        if !instance_counts
            .iter()
            .zip(vertex_counts.iter())
            .any(|(&instances, &vertices)| instances > 0 && vertices > 0)
        {
            return;
        }
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(1, &self.bind_group_g1, &[]);
        pass.set_bind_group(2, &self.bind_group_g2, &[]);
        for lod in 0..3 {
            if instance_counts[lod] == 0 || vertex_counts[lod] == 0 {
                continue;
            }
            pass.set_bind_group(0, &self.bind_groups_g0[lod], &[]);
            pass.set_vertex_buffer(0, instance_buffers[lod].slice(..));
            pass.draw(0..vertex_counts[lod], 0..instance_counts[lod]);
        }
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
@group(2) @binding(8) var water_color: texture_2d<f32>;
@group(2) @binding(9) var lean1_tex: texture_2d<f32>;
@group(2) @binding(10) var lean2_tex: texture_2d<f32>;
@group(2) @binding(11) var province_secondary_color: texture_2d<f32>;
@group(2) @binding(12) var mud_snow_tex: texture_2d<f32>;
@group(2) @binding(13) var citylights_snow_noise: texture_2d<f32>;
@group(2) @binding(14) var light_data_tex: texture_2d<f32>;
@group(2) @binding(15) var light_index_tex: texture_2d<f32>;
@group(2) @binding(16) var gradient_border_ch1: texture_2d<f32>;
@group(2) @binding(17) var gradient_border_ch2: texture_2d<f32>;
@group(2) @binding(18) var gradient_border_ch3: texture_2d<f32>;
@group(2) @binding(19) var reflection_tex: texture_2d<f32>;
@group(2) @binding(20) var shadow_map: texture_2d<f32>;

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
const RIVER_TILE_PX: f32 = 96.0;
const RIVER_POINT_LIGHTS_ENABLED: bool = false;
const GB_TEXTURE_HEIGHT_RIVER: f32 = 1024.0;

fn gradient_border_page_uv(uv: vec2<f32>, page: f32) -> vec2<f32> {
    let half_pix = 0.5 / GB_TEXTURE_HEIGHT_RIVER;
    return vec2<f32>(
        uv.x,
        uv.y * (0.5 - half_pix) + page * 0.5
    );
}

fn gradient_border_ch1_sample(uv: vec2<f32>) -> vec4<f32> {
    return textureSample(gradient_border_ch1, river_sampler, gradient_border_page_uv(uv, 0.0));
}

fn gradient_border_ch2_sample(uv: vec2<f32>) -> vec4<f32> {
    return textureSample(gradient_border_ch2, river_sampler, gradient_border_page_uv(uv, 0.0));
}

fn apply_river_gradient_border(base_color: vec3<f32>, uv: vec2<f32>) -> vec3<f32> {
    let ch1 = gradient_border_ch1_sample(uv);
    let ch2 = gradient_border_ch2_sample(uv);
    let country_gate = clamp(ch2.g, 0.0, 1.0);
    let outline_alpha = smoothstep(0.74, 1.0, ch1.a) * country_gate;
    let country_bleed = clamp(ch1.a * country_gate, 0.0, 1.0) * 0.06;
    let outline_color = min(base_color, ch1.rgb * 0.52 + vec3<f32>(0.006, 0.012, 0.018));
    return mix(base_color, outline_color, clamp(outline_alpha * 0.18 + country_bleed, 0.0, 0.22));
}

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

    var flow_dir = river_sample.gb * 2.0 - vec2<f32>(1.0);
    if dot(flow_dir, flow_dir) < 0.01 {
        flow_dir = vec2<f32>(0.0, 1.0);
    }
    flow_dir = normalize(flow_dir);
    let tangent = normalize(vec3<f32>(flow_dir.x, 0.0, flow_dir.y));

    let level = clamp(round(river_lvl * 4.0), 1.0, 4.0);
    let level_norm = (level - 1.0) / 3.0;
    let texture_lod = 2.0 - 2.0 * clamp(zoom_factor, 0.0, 1.0);
    let time = frame.global_time;
    let base_uv = in.map_px / vec2<f32>(RIVER_TILE_PX);
    let scrolled_uv = base_uv - flow_dir * time * rparams.flow_speed;

    let flow_ripple_uv = scrolled_uv + flow_dir.yx * vec2<f32>(0.18, -0.18) * sin(frame.global_time * 0.45 + in.map_px.x * 0.004);
    let diffuse0 = textureSample(river_diffuse_0, river_sampler, flow_ripple_uv).rgb;
    let diffuse1 = textureSample(river_diffuse_1, river_sampler, scrolled_uv * 0.92 + vec2<f32>(0.13, 0.07)).rgb;
    let diffuse2 = textureSample(river_diffuse_2, river_sampler, scrolled_uv * 0.84 + vec2<f32>(0.29, 0.19)).rgb;
    let diffuse01 = mix(diffuse0, diffuse1, clamp(texture_lod, 0.0, 1.0));
    let diffuse = mix(diffuse01, diffuse2, clamp(texture_lod - 1.0, 0.0, 1.0));

    let lean_uv = in.map_px / vec2<f32>(128.0);
    let lean = mix(
        textureSample(lean1_tex, river_sampler, lean_uv + flow_dir * frame.global_time * 0.015),
        textureSample(lean2_tex, river_sampler, lean_uv * 1.7 - flow_dir * frame.global_time * 0.011),
        0.5
    );
    let lean_normal = unpack_normal(vec3<f32>(lean.r, lean.g, 1.0));
    let normal0 = normalize(mix(
        unpack_normal(textureSample(river_normal_0, river_sampler, scrolled_uv).rgb),
        lean_normal,
        0.18
    ));
    let normal1 = unpack_normal(textureSample(river_normal_1, river_sampler, scrolled_uv * 0.92 + vec2<f32>(0.13, 0.07)).rgb);
    let normal2 = unpack_normal(textureSample(river_normal_2, river_sampler, scrolled_uv * 0.84 + vec2<f32>(0.29, 0.19)).rgb);
    let normal01 = normalize(mix(normal0, normal1, clamp(texture_lod, 0.0, 1.0)));
    let normal = normalize(mix(normal01, normal2, clamp(texture_lod - 1.0, 0.0, 1.0)));

    let to_camera = normalize(frame.cam_pos - in.world_pos);
    let sun_dir = normalize(vec3<f32>(0.4, 1.0, 0.3));
    let flow_half = normalize(to_camera + sun_dir + tangent * 0.55);
    let flow_streak = pow(abs(dot(normalize(flow_dir), normalize(vec2<f32>(normal.x, normal.z) + flow_dir * 0.25))), 2.0);
    let spec = pow(max(dot(normal, flow_half), 0.0), 44.0) * mix(0.14, 0.36, level_norm) * (0.65 + flow_streak * 0.35);

    let depth_tint = mix(vec3<f32>(0.88, 0.95, 0.96), vec3<f32>(0.55, 0.68, 0.82), level_norm);
    let water_base = textureSample(water_color, river_sampler, in.map_uv).rgb;
    let river_tint = mix(vec3<f32>(0.040, 0.115, 0.165), vec3<f32>(0.060, 0.145, 0.230), level_norm);
    let river_surface = mix(river_tint, diffuse, mix(0.26, 0.42, level_norm));
    var color = mix(river_surface, water_base, 0.10) * depth_tint + vec3<f32>(spec * 0.72);

    let secondary = textureSample(province_secondary_color, river_sampler, in.map_uv);
    color = mix(color, secondary.rgb, secondary.a * 0.055);

    let mud_snow = textureSample(mud_snow_tex, river_sampler, in.map_uv);
    let snow = get_snow(mud_snow, clamp(frame.fow_opacity_time_snow_max_speed.z, 0.0, 1.0));
    color = mix(color, vec3<f32>(0.58, 0.68, 0.78), snow * 0.16);

    color = apply_river_gradient_border(color, in.map_uv);

    let reflection = textureSample(reflection_tex, river_sampler, in.map_uv).rgb;
    color = mix(color, reflection, 0.020 + spec * 0.016);

    let city_noise = textureSample(citylights_snow_noise, river_sampler, in.map_uv * 4.0).a;
    color = color + vec3<f32>(city_noise) * snow * 0.025;

    var river_point_lights = vec3<f32>(0.0);
    if (RIVER_POINT_LIGHTS_ENABLED) {
        river_point_lights = calculate_point_lights(
            light_data_tex,
            light_index_tex,
            in.map_px,
            in.world_pos,
            normal,
            0.18
        );
    }
    color = color + river_point_lights;

    let projected_shadow_uv = in.clip_pos.xy / max(frame.screen_size, vec2<f32>(1.0));
    let projected_shadow = textureSample(shadow_map, river_sampler, clamp(projected_shadow_uv, vec2<f32>(0.0), vec2<f32>(1.0)));
    color = mix(color * 0.58, color, projected_shadow.r);

    color = day_night(
        color,
        calc_globe_normal(in.map_px, frame.day_night_hour_sun_dir.x),
        frame.day_night_hour_sun_dir.yzw,
        1.0,
    );
    color = apply_distance_fog(color, in.world_pos, frame.cam_pos);

    let mask_sample = textureSample(river_masks, river_sampler, scrolled_uv);
    let mask_rg = mix(mask_sample.r, mask_sample.g, clamp(level - 1.0, 0.0, 1.0));
    let mask_ba = mix(mask_sample.b, mask_sample.a, clamp(level - 3.0, 0.0, 1.0));
    let level_alpha = mix(mask_rg, mask_ba, clamp((level - 2.0) * 0.5, 0.0, 1.0));
    let river_alpha = smoothstep(zoom_cut - 0.10, zoom_cut + 0.10, river_lvl)
        * mix(0.34, 0.70, level_norm)
        * mix(0.28, 0.78, level_alpha)
        * max(projected_shadow.g, projected_shadow.b)
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

fn fragment_tex_entry_nonfilter(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: false },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

fn river_p5_state_audit_entries() -> [BindingAuditEntry; 4] {
    [
        BindingAuditEntry::plain_resource(
            "river",
            "effect_selector",
            "river:river",
            true,
            false,
            Some("R12 selector default: high graphics byte +0x18 true".into()),
            "river pass variant is selected independently from terrain and water",
        ),
        BindingAuditEntry::plain_resource(
            "river",
            "state_1620_blend",
            "src_alpha/inv_src_alpha rgb, preserve destination alpha",
            true,
            false,
            Some("reverse_out/exports/pdxwater_constants.tsv blend_state 1620".into()),
            "river blends into the shared HDR scene with traced state 1620 semantics",
        ),
        BindingAuditEntry::plain_resource(
            "river",
            "depth_disabled",
            "no depth-stencil attachment",
            true,
            false,
            Some("reverse_out/08_water_river_pipeline.md river depth disabled".into()),
            "river is submitted as an independent order-83 HDR pass before water",
        ),
        BindingAuditEntry::mock(
            "river",
            "ReflectionCubeMap",
            "reflection_2d_placeholder",
            "No cubemap-specific river reflection binding is produced yet; pass uses reflection.dds fallback.",
            "river reflection is explicit fallback and cannot count as full vanilla parity",
            BindingBlockingLevel::Degraded,
        ),
    ]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn river_p5_state_audit_reports_depth_disabled_and_blend() {
        let entries = river_p5_state_audit_entries();
        assert!(entries
            .iter()
            .any(|entry| entry.binding == "effect_selector" && entry.source_name == "river:river"));
        assert!(entries
            .iter()
            .any(|entry| entry.binding == "state_1620_blend"));
        assert!(entries.iter().any(|entry| {
            entry.binding == "depth_disabled"
                && entry
                    .reason
                    .as_deref()
                    .is_some_and(|reason| reason.contains("depth disabled"))
        }));
        assert!(entries
            .iter()
            .any(|entry| entry.binding == "ReflectionCubeMap"
                && entry.blocking_level == BindingBlockingLevel::Degraded));
    }
}
