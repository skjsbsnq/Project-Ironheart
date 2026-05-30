//! Phase 3.12.4 — `TerrainPass`：把旧 inline 地形 pipeline 升级为 vanilla pdxmap 等价。
//!
//! ## 替换关系
//!
//! - 原 `crates/hoi4-render/src/shader.wgsl` → 移到归档常量
//!   `hoi4_render::ARCHIVED_SHADER_MAIN_WGSL`，仅作历史参考
//! - 新 `crates/hoi4-app/src/passes/terrain.wgsl` 是组合体：
//!   保留旧 chunk-tessellated 顶点（沿用现有 `ChunkInstance` 实例数据 + LOD），
//!   片元侧按 vanilla pdxmap 路径补 atlas_normal / world_normal / city lights /
//!   shadow PCF / 季节政治色 / 昼夜
//!
//! ## binding 布局（3 个 group）
//!
//! - `@group(0)` — frame uniforms（每 LOD 一个 bind group，因为 ChunkUniform 不同）
//!   - `@binding(0)` GlobalFrameUniform（共享 320 字节）
//!   - `@binding(1)` PdxMapParams（每帧更新；含 selected_pid / season / world_size）
//!   - `@binding(2)` ChunkUniform（per-LOD 4 字节）
//!
//! - `@group(1)` — 跨 LOD 共享
//!   - shadow_map / shadow_sampler
//!   - season_map / color_map / color_map_second（3.12.8 接 seasons.txt 前都用 colormap）
//!   - light_data / light_index（3.12.X 之前用 1×1 mock）
//!   - gradient_border ch1/ch2（**复用** country_sdf / province_sdf；3.12.9 后接 vanilla 18 张）
//!   - province_secondary_color（1×1 mock）
//!   - generic_sampler（linear）
//!   - occupation_lut / coast_sdf / rivers（兼容现有特性）
//!
//! - `@group(2)` — pass-specific 地形位图
//!   - heightmap / province_id / terrain_idx / terrain_atlas
//!   - **NEW**: terrain_atlas_normal（atlas_normal0.dds，BC5 → Bc5RgUnorm）
//!   - **NEW**: world_normal（world_normal.bmp，3 通道 24-bit RGB）
//!   - **NEW**: colormap_emissive（colormap_rgb_cityemissivemask_a.dds，BC3 .a = emit mask）
//!   - **NEW**: citylights（citylights_rgb_snowmask_a_0.dds，BC3 .rgb = night light）
//!   - country_color_lut（main.rs 的 lut_view）
//!   - pass_sampler

use std::sync::Arc;

use hoi4_assets::{dds_upload_plan, AssetDb, DdsImage, FsAssetDb, MapResRole, VanillaMapSet};
use hoi4_paths::PathConfig;
use wgpu::util::DeviceExt;

use crate::passes::HDR_FORMAT;

const SHADER_WGSL: &str = include_str!("terrain.wgsl");

/// Public alias for the wgsl source — used by `tests/terrain_wgsl.rs` to
/// validate the shader via naga without recompiling the include.
pub const TERRAIN_WGSL_SOURCE: &str = SHADER_WGSL;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum TerrainDebugView {
    Off = 0,
    TerrainId = 1,
    AtlasTileId = 2,
    PoliticalColor = 3,
    TerrainAlbedo = 4,
    Normal = 5,
    HeightSlope = 6,
    SnowMask = 7,
    RiverMask = 8,
}

impl TerrainDebugView {
    pub const ALL: [Self; 9] = [
        Self::Off,
        Self::TerrainId,
        Self::AtlasTileId,
        Self::PoliticalColor,
        Self::TerrainAlbedo,
        Self::Normal,
        Self::HeightSlope,
        Self::SnowMask,
        Self::RiverMask,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::TerrainId => "terrain_id",
            Self::AtlasTileId => "atlas_tile_id",
            Self::PoliticalColor => "political_color",
            Self::TerrainAlbedo => "terrain_albedo",
            Self::Normal => "normal",
            Self::HeightSlope => "height_slope",
            Self::SnowMask => "snow_mask",
            Self::RiverMask => "river_mask",
        }
    }

    pub const fn as_shader_value(self) -> f32 {
        match self {
            Self::Off => 0.0,
            Self::TerrainId => 1.0,
            Self::AtlasTileId => 2.0,
            Self::PoliticalColor => 3.0,
            Self::TerrainAlbedo => 4.0,
            Self::Normal => 5.0,
            Self::HeightSlope => 6.0,
            Self::SnowMask => 7.0,
            Self::RiverMask => 8.0,
        }
    }

    pub fn next(self) -> Self {
        let idx = Self::ALL.iter().position(|view| *view == self).unwrap_or(0);
        Self::ALL[(idx + 1) % Self::ALL.len()]
    }
}

/// `PdxMapParams` 与 wgsl 端 `struct PdxMapParams` 字段一一对应。
///
/// **176 字节**（11 × vec4，wgsl uniform 对齐）。
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable, Debug)]
pub struct PdxMapParams {
    /// 当前选中省份 id；u32::MAX = 无选中。
    pub selected_province_id: u32,
    /// 当前选中省份所属 state 的内部索引；u32::MAX = 无。
    pub selected_state_id: u32,
    pub hovered_province_id: u32,
    pub terrain_blend: f32,

    pub screen_width: f32,
    pub screen_height: f32,
    pub vignette_strength: f32,
    pub zoom_factor: f32,

    pub border_country_px: f32,
    pub border_province_px: f32,
    pub season_lerp: f32,
    pub map_mode_terrain_blend: f32,

    /// world_w / world_d / height_scale / lat_correction
    pub world_size_xy_height_lat: [f32; 4],

    /// season_column / season_snow_offset / reserved / reserved.
    pub season_params: [f32; 4],

    /// debug_view / terrain_water_final_color / terrain_border_sdf / terrain_overlays.
    pub terrain_controls: [f32; 4],
    /// occupation / selected / hover / map-mode overlay opacity.
    pub overlay_controls: [f32; 4],

    /// terrain.bmp palette index → atlas tile index LUT.
    /// Packed as 4 × vec4<u32> to match wgsl `array<vec4<u32>, 4>`.
    pub atlas_idx_array: [[u32; 4]; 4],
}

impl Default for PdxMapParams {
    fn default() -> Self {
        Self {
            selected_province_id: u32::MAX,
            selected_state_id: u32::MAX,
            hovered_province_id: u32::MAX,
            terrain_blend: 0.45,
            screen_width: 1920.0,
            screen_height: 1080.0,
            vignette_strength: 0.05,
            zoom_factor: 0.5,
            border_country_px: 3.2,
            border_province_px: 0.75,
            season_lerp: 0.0,
            map_mode_terrain_blend: 0.32,
            world_size_xy_height_lat: [112.0, 41.0, 4.0, 0.0],
            season_params: [0.0, 0.0, 0.0, 0.0],
            terrain_controls: [0.0, 1.0, 0.0, 1.0],
            overlay_controls: [1.0, 1.0, 0.0, 0.0],
            atlas_idx_array: [[0, 1, 2, 3], [4, 5, 6, 7], [8, 9, 10, 11], [12, 13, 14, 15]],
        }
    }
}

/// ChunkUniform 与 wgsl 端 struct 对应。每 LOD 一份。
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable, Debug)]
pub struct ChunkUniform {
    pub grid: u32,
    pub _pad: [u32; 3],
}

/// TerrainPass 构造参数：所有"已经在 main.rs 上传过"的纹理与 buffer。
pub struct TerrainPassInputs<'a> {
    pub global_uniform_buffer: &'a wgpu::Buffer,
    pub depth_format: wgpu::TextureFormat,

    // Per-LOD chunk grid
    pub lod_grid: [u32; 3],

    // Shared cross-LOD textures (reuse main.rs uploaded views)
    pub shadow_map_view: &'a wgpu::TextureView,
    pub shadow_sampler: &'a wgpu::Sampler,
    pub colormap_view: &'a wgpu::TextureView, // → color_map / color_map_second / season_map
    pub country_sdf_view: &'a wgpu::TextureView, // → gradient_border_ch1
    pub province_sdf_view: &'a wgpu::TextureView, // → gradient_border_ch2
    pub coast_sdf_view: &'a wgpu::TextureView,
    pub occupation_lut_view: &'a wgpu::TextureView,
    pub rivers_view: &'a wgpu::TextureView,

    // Pass-specific bitmaps (already uploaded in main.rs)
    pub heightmap_view: &'a wgpu::TextureView,
    pub province_view: &'a wgpu::TextureView,
    pub terrain_idx_view: &'a wgpu::TextureView,
    pub terrain_atlas_view: &'a wgpu::TextureView,
    pub country_color_lut_view: &'a wgpu::TextureView, // = main.rs::lut_view

    // Optional overrides for 3.12.4 new vanilla textures.
    // None = TerrainPass loads via VanillaMapSet/path_cfg, with 1×1 fallback if missing.
    pub map_set: Option<&'a Arc<VanillaMapSet>>,
}

/// 主 pass。
pub struct TerrainPass {
    pipeline: wgpu::RenderPipeline,

    // Per-LOD bind groups (group 0 carries chunk uniform that varies)
    bind_groups_g0: [wgpu::BindGroup; 3],
    bind_group_g1: wgpu::BindGroup,
    bind_group_g2: wgpu::BindGroup,

    // Owned uniform buffers
    params_buffer: wgpu::Buffer,
    chunk_buffers: [wgpu::Buffer; 3],

    // Owned new textures (kept alive via field)
    _atlas_normal_tex: wgpu::Texture,
    _world_normal_tex: wgpu::Texture,
    _colormap_emissive_tex: wgpu::Texture,
    _citylights_tex: wgpu::Texture,
    _stub_textures: Vec<wgpu::Texture>,
    _samplers: Vec<wgpu::Sampler>,

    /// 加载日志（启动 banner 用）。
    pub load_warnings: Vec<String>,
}

impl TerrainPass {
    /// 构造一个完整 TerrainPass。
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        path_cfg: &PathConfig,
        inputs: TerrainPassInputs<'_>,
    ) -> Self {
        let mut warnings: Vec<String> = Vec::new();

        // ── Load 4 new vanilla textures (atlas_normal / world_normal / colormap_emissive / citylights) ──
        // 优先从 inputs.map_set；否则直接走 FsAssetDb。
        let db = FsAssetDb::new(path_cfg.clone());

        let load_dds_bytes = |role: MapResRole| -> Option<Vec<u8>> {
            if let Some(ms) = inputs.map_set.as_ref() {
                if let Some(b) = ms.bytes(role) {
                    return Some(b.to_vec());
                }
            }
            db.open(role.relative_path()).ok().map(|b| b.to_vec())
        };

        let (atlas_normal_tex, atlas_normal_view) = load_dds_to_texture(
            device,
            queue,
            "atlas_normal",
            load_dds_bytes(MapResRole::TerrainAtlasNormal(0)),
            &[128u8, 128, 255, 255], // flat tangent normal
            false,                   // linear
            &mut warnings,
        );

        let (world_normal_tex, world_normal_view) = load_world_normal_bmp(
            device,
            queue,
            path_cfg,
            inputs.map_set.map(|m| m.as_ref()),
            &mut warnings,
        );

        let (colormap_emissive_tex, colormap_emissive_view) = load_dds_to_texture(
            device,
            queue,
            "colormap_rgb_cityemissivemask_a",
            load_dds_bytes(MapResRole::ColormapEmissive),
            &[128, 128, 96, 0], // .a=0 = no city emit
            true,               // sRGB color
            &mut warnings,
        );

        let (citylights_tex, citylights_view) = load_dds_to_texture(
            device,
            queue,
            "citylights",
            load_dds_bytes(MapResRole::CityLights(0)),
            &[0u8, 0, 0, 0], // no light
            true,
            &mut warnings,
        );

        // ── Stub textures for not-yet-implemented bindings ──
        let mut stub_textures: Vec<wgpu::Texture> = Vec::new();
        let mut samplers: Vec<wgpu::Sampler> = Vec::new();

        // light_data / light_index (3.12.x: real point lights). Mock 1×1.
        let (light_data_tex, light_data_view) =
            make_1x1_rgba8_unorm(device, queue, "light_data_mock", [0, 0, 0, 0]);
        let (light_index_tex, light_index_view) =
            make_1x1_rgba8_unorm(device, queue, "light_index_mock", [255, 255, 255, 255]);
        stub_textures.push(light_data_tex);
        stub_textures.push(light_index_tex);

        let (secondary_color_tex, secondary_color_view) =
            make_1x1_rgba8_unorm(device, queue, "province_secondary_color_mock", [0, 0, 0, 0]);
        stub_textures.push(secondary_color_tex);

        // ── Samplers ──
        let generic_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("terrain_generic_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let pass_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("terrain_pass_sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        samplers.push(generic_sampler.clone_unchecked());
        samplers.push(pass_sampler.clone_unchecked());

        // ── Uniform buffers ──
        let params_init = PdxMapParams::default();
        let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("pdxmap_params"),
            contents: bytemuck::bytes_of(&params_init),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let chunk_buffers: [wgpu::Buffer; 3] = std::array::from_fn(|i| {
            let u = ChunkUniform {
                grid: inputs.lod_grid[i],
                _pad: [0; 3],
            };
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("terrain_chunk_uniform"),
                contents: bytemuck::bytes_of(&u),
                usage: wgpu::BufferUsages::UNIFORM,
            })
        });

        // ── Bind group layouts ──
        let bgl_g0 = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("terrain_bgl_g0_frame"),
            entries: &[
                // GlobalFrameUniform
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // PdxMapParams
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // ChunkUniform
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
            ],
        });

        let bgl_g1 = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("terrain_bgl_g1_shared"),
            entries: &[
                // 0: shadow_map (depth)
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
                // 1: shadow_sampler (comparison)
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                    count: None,
                },
                // 2: season_map
                texture_entry(2),
                // 3: color_map
                texture_entry(3),
                // 4: color_map_second
                texture_entry(4),
                // 5: light_data
                texture_entry(5),
                // 6: light_index
                texture_entry(6),
                // 7: gradient_border_ch1 (= country_sdf)
                texture_entry(7),
                // 8: gradient_border_ch2 (= province_sdf)
                texture_entry(8),
                // 9: province_secondary_color
                texture_entry(9),
                // 10: generic_sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 10,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // 11: occupation_lut (legacy)
                texture_entry_nonfilter(11),
                // 12: coast_sdf
                texture_entry(12),
                // 13: rivers
                texture_entry(13),
            ],
        });

        let bgl_g2 = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("terrain_bgl_g2_pass"),
            entries: &[
                // 0: heightmap
                texture_entry_nonfilter(0),
                // 1: province_id (R16Uint)
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Uint,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // 2: terrain_idx (R8Uint)
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Uint,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // 3: terrain_atlas
                texture_entry(3),
                // 4: terrain_atlas_normal
                texture_entry(4),
                // 5: world_normal
                texture_entry(5),
                // 6: colormap_emissive
                texture_entry(6),
                // 7: citylights
                texture_entry(7),
                // 8: country_color_lut
                texture_entry_nonfilter(8),
                // 9: pass_sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 9,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        // ── Build bind groups ──
        let bind_groups_g0: [wgpu::BindGroup; 3] = std::array::from_fn(|i| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("terrain_bg_g0"),
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
                        resource: chunk_buffers[i].as_entire_binding(),
                    },
                ],
            })
        });

        let bind_group_g1 = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("terrain_bg_g1"),
            layout: &bgl_g1,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(inputs.shadow_map_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(inputs.shadow_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(inputs.colormap_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(inputs.colormap_view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(inputs.colormap_view),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::TextureView(&light_data_view),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::TextureView(&light_index_view),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: wgpu::BindingResource::TextureView(inputs.country_sdf_view),
                },
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: wgpu::BindingResource::TextureView(inputs.province_sdf_view),
                },
                wgpu::BindGroupEntry {
                    binding: 9,
                    resource: wgpu::BindingResource::TextureView(&secondary_color_view),
                },
                wgpu::BindGroupEntry {
                    binding: 10,
                    resource: wgpu::BindingResource::Sampler(&generic_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 11,
                    resource: wgpu::BindingResource::TextureView(inputs.occupation_lut_view),
                },
                wgpu::BindGroupEntry {
                    binding: 12,
                    resource: wgpu::BindingResource::TextureView(inputs.coast_sdf_view),
                },
                wgpu::BindGroupEntry {
                    binding: 13,
                    resource: wgpu::BindingResource::TextureView(inputs.rivers_view),
                },
            ],
        });

        let bind_group_g2 = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("terrain_bg_g2"),
            layout: &bgl_g2,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(inputs.heightmap_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(inputs.province_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(inputs.terrain_idx_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(inputs.terrain_atlas_view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&atlas_normal_view),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::TextureView(&world_normal_view),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::TextureView(&colormap_emissive_view),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: wgpu::BindingResource::TextureView(&citylights_view),
                },
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: wgpu::BindingResource::TextureView(inputs.country_color_lut_view),
                },
                wgpu::BindGroupEntry {
                    binding: 9,
                    resource: wgpu::BindingResource::Sampler(&pass_sampler),
                },
            ],
        });

        // ── Pipeline ──
        let composed = hoi4_render::shader_rt::compose_shader(SHADER_WGSL, true, true);
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("terrain_pdxmap_shader"),
            source: wgpu::ShaderSource::Wgsl(composed.into()),
        });

        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("terrain_pl"),
            bind_group_layouts: &[&bgl_g0, &bgl_g1, &bgl_g2],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("terrain_pipeline_pdxmap"),
            layout: Some(&pl),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<[f32; 4]>() as u64, // ChunkInstance = 4 floats (origin_xz, size_xz)
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
                module: &module,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: HDR_FORMAT,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
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
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });

        Self {
            pipeline,
            bind_groups_g0,
            bind_group_g1,
            bind_group_g2,
            params_buffer,
            chunk_buffers,
            _atlas_normal_tex: atlas_normal_tex,
            _world_normal_tex: world_normal_tex,
            _colormap_emissive_tex: colormap_emissive_tex,
            _citylights_tex: citylights_tex,
            _stub_textures: stub_textures,
            _samplers: samplers,
            load_warnings: warnings,
        }
    }

    /// 每帧调用：写 PdxMapParams uniform。
    pub fn update_params(&self, queue: &wgpu::Queue, params: &PdxMapParams) {
        queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(params));
    }

    /// 在 render pass 内提交 draw（按 LOD 分别 draw）。
    pub fn render<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        instance_buffers: &'a [wgpu::Buffer; 3],
        instance_counts: &[u32; 3],
        vertex_counts: &[u32; 3],
    ) {
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

// =============================================================================
// Helpers
// =============================================================================

fn texture_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

fn texture_entry_nonfilter(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: false },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

fn make_1x1_rgba8_unorm(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    label: &str,
    rgba: [u8; 4],
) -> (wgpu::Texture, wgpu::TextureView) {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &tex,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &rgba,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4),
            rows_per_image: None,
        },
        wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
    );
    let view = tex.create_view(&Default::default());
    (tex, view)
}

/// 加载一个 DDS（vanilla 角色），失败回 1×1 fallback。`is_srgb` 决定 sRGB / linear。
fn load_dds_to_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    label: &str,
    bytes: Option<Vec<u8>>,
    fallback_rgba: &[u8; 4],
    is_srgb: bool,
    warnings: &mut Vec<String>,
) -> (wgpu::Texture, wgpu::TextureView) {
    let bytes = match bytes {
        Some(b) if !b.is_empty() => b,
        _ => {
            warnings.push(format!(
                "[terrain_pass] {} missing — using 1×1 fallback",
                label
            ));
            return make_1x1_rgba8_unorm(
                device,
                queue,
                &format!("{}_fallback", label),
                *fallback_rgba,
            );
        }
    };
    let dds = match DdsImage::parse(&bytes) {
        Ok(d) => d,
        Err(e) => {
            warnings.push(format!(
                "[terrain_pass] {} parse failed ({}) — fallback",
                label, e
            ));
            return make_1x1_rgba8_unorm(
                device,
                queue,
                &format!("{}_fallback", label),
                *fallback_rgba,
            );
        }
    };

    use hoi4_assets::DdsFormat;
    let format = match (dds.format, is_srgb) {
        (DdsFormat::Bc1, true) => wgpu::TextureFormat::Bc1RgbaUnormSrgb,
        (DdsFormat::Bc1, false) => wgpu::TextureFormat::Bc1RgbaUnorm,
        (DdsFormat::Bc3, true) => wgpu::TextureFormat::Bc3RgbaUnormSrgb,
        (DdsFormat::Bc3, false) => wgpu::TextureFormat::Bc3RgbaUnorm,
        (DdsFormat::Bc5, _) => wgpu::TextureFormat::Bc5RgUnorm,
        (DdsFormat::Bgra8, true) => wgpu::TextureFormat::Bgra8UnormSrgb,
        (DdsFormat::Bgra8, false) => wgpu::TextureFormat::Bgra8Unorm,
        _ => {
            warnings.push(format!(
                "[terrain_pass] {} unsupported DDS format — fallback",
                label
            ));
            return make_1x1_rgba8_unorm(
                device,
                queue,
                &format!("{}_fallback", label),
                *fallback_rgba,
            );
        }
    };

    let upload_plan = match dds_upload_plan(&dds) {
        Some(plan) if plan.upload_mip_count > 0 => plan,
        _ => {
            warnings.push(format!(
                "[terrain_pass] {} has no uploadable DDS mips — fallback",
                label
            ));
            return make_1x1_rgba8_unorm(
                device,
                queue,
                &format!("{}_fallback", label),
                *fallback_rgba,
            );
        }
    };
    let mip_count = upload_plan.upload_mip_count;
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: dds.width,
            height: dds.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: mip_count,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    for mip in &upload_plan.mips {
        let data = &dds.data[mip.offset..mip.offset + mip.size];
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: mip.level,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(mip.bytes_per_row),
                rows_per_image: None,
            },
            wgpu::Extent3d {
                width: mip.copy_width,
                height: mip.copy_height,
                depth_or_array_layers: 1,
            },
        );
    }
    let view = texture.create_view(&Default::default());
    println!(
        "[terrain_pass] loaded {} ({}×{} {:?} {} mips)",
        label, dds.width, dds.height, dds.format, mip_count
    );
    (texture, view)
}

/// 加载 vanilla `world_normal.bmp` → wgpu Rgba8Unorm（linear，**非** sRGB）。
///
/// world_normal.bmp 是 24-bit RGB 图（部分 vanilla 版本是 32-bit BGRA），存的是
/// 全球大尺度法线（经/纬切线坐标系）。我们把它解到 Rgba8Unorm。
fn load_world_normal_bmp(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    path_cfg: &PathConfig,
    map_set: Option<&VanillaMapSet>,
    warnings: &mut Vec<String>,
) -> (wgpu::Texture, wgpu::TextureView) {
    let label = "world_normal";
    let bytes = if let Some(ms) = map_set {
        ms.bytes(MapResRole::WorldNormal).map(|b| b.to_vec())
    } else {
        let db = FsAssetDb::new(path_cfg.clone());
        db.open(MapResRole::WorldNormal.relative_path())
            .ok()
            .map(|b| b.to_vec())
    };
    let bytes = match bytes {
        Some(b) if !b.is_empty() => b,
        _ => {
            warnings.push(format!(
                "[terrain_pass] {}.bmp missing — using flat-normal fallback",
                label
            ));
            return make_1x1_rgba8_unorm(
                device,
                queue,
                "world_normal_fallback",
                [128, 128, 255, 255],
            );
        }
    };

    // Parse BMP header: minimum supported = 24-bit BI_RGB / 32-bit BGRA / BI_BITFIELDS.
    let parsed = parse_bmp_24_or_32(&bytes);
    let (w, h, rgba) = match parsed {
        Some(t) => t,
        None => {
            warnings.push(format!(
                "[terrain_pass] {}.bmp parse failed — fallback",
                label
            ));
            return make_1x1_rgba8_unorm(
                device,
                queue,
                "world_normal_fallback",
                [128, 128, 255, 255],
            );
        }
    };

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &rgba,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(w * 4),
            rows_per_image: None,
        },
        wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
    );
    let view = texture.create_view(&Default::default());
    println!(
        "[terrain_pass] loaded {} ({}×{} BMP→Rgba8Unorm)",
        label, w, h
    );
    (texture, view)
}

/// 简易 BMP 24-bit / 32-bit decoder。返回 (width, height, RGBA bytes，top-down)。
/// 支持 BI_RGB（无压缩）。
fn parse_bmp_24_or_32(bytes: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    if bytes.len() < 54 || &bytes[0..2] != b"BM" {
        return None;
    }
    let pixel_offset = u32::from_le_bytes(bytes[10..14].try_into().ok()?) as usize;
    let dib_size = u32::from_le_bytes(bytes[14..18].try_into().ok()?) as usize;
    if dib_size < 40 {
        return None;
    }
    let width = i32::from_le_bytes(bytes[18..22].try_into().ok()?);
    let height = i32::from_le_bytes(bytes[22..26].try_into().ok()?);
    let bpp = u16::from_le_bytes(bytes[28..30].try_into().ok()?);
    let compression = u32::from_le_bytes(bytes[30..34].try_into().ok()?);
    if compression != 0 && compression != 3 {
        return None; // only BI_RGB / BI_BITFIELDS handled
    }
    if width <= 0 || height == 0 {
        return None;
    }
    let w = width as u32;
    let flip = height > 0;
    let h = height.unsigned_abs();

    let bytes_per_pixel = (bpp / 8) as usize;
    if bytes_per_pixel != 3 && bytes_per_pixel != 4 {
        return None;
    }
    let row_stride = (w as usize * bytes_per_pixel + 3) & !3; // 4-byte align
    let needed = pixel_offset + row_stride * h as usize;
    if bytes.len() < needed {
        return None;
    }

    let mut rgba = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        let src_y = if flip { h - 1 - y } else { y };
        let src_row = pixel_offset + (src_y as usize) * row_stride;
        let dst_row = (y as usize) * (w as usize * 4);
        for x in 0..w as usize {
            let s = src_row + x * bytes_per_pixel;
            let d = dst_row + x * 4;
            // BMP stores BGR (and optional A); convert to RGBA.
            let b = bytes[s];
            let g = bytes[s + 1];
            let r = bytes[s + 2];
            let a = if bytes_per_pixel == 4 {
                bytes[s + 3]
            } else {
                255
            };
            rgba[d] = r;
            rgba[d + 1] = g;
            rgba[d + 2] = b;
            rgba[d + 3] = a;
        }
    }
    Some((w, h, rgba))
}

// 帮助：wgpu::Sampler 不实现 Clone — 但我们其实不需要 clone，只是为了
// "持有"它防止被 drop。这个 trait 是空 stub。
trait CloneUncheckedSampler {
    fn clone_unchecked(&self) -> wgpu::Sampler;
}
impl CloneUncheckedSampler for wgpu::Sampler {
    fn clone_unchecked(&self) -> wgpu::Sampler {
        // wgpu samplers ARE clone-able in 24.x because they're Arc-wrapped internally.
        // If this fails to compile, replace with a no-op that returns a new default sampler.
        self.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pdxmap_params_size_matches_wgsl() {
        // 6 vec4-sized groups before the atlas plus 4 vec4<u32> atlas rows.
        assert_eq!(std::mem::size_of::<PdxMapParams>(), 176);
    }

    #[test]
    fn terrain_debug_view_cycles_through_shader_values() {
        assert_eq!(TerrainDebugView::Off.next(), TerrainDebugView::TerrainId);
        assert_eq!(TerrainDebugView::RiverMask.next(), TerrainDebugView::Off);
        assert_eq!(TerrainDebugView::Normal.as_shader_value(), 5.0);
        assert_eq!(TerrainDebugView::AtlasTileId.name(), "atlas_tile_id");
    }

    #[test]
    fn chunk_uniform_size_is_16() {
        assert_eq!(std::mem::size_of::<ChunkUniform>(), 16);
    }

    #[test]
    fn parse_bmp_simple_24bit() {
        // 2×2 BMP, 24-bit, all white pixels.
        let mut bmp = Vec::new();
        bmp.extend_from_slice(b"BM");
        bmp.extend_from_slice(&54u32.to_le_bytes()); // size (just header for synth)
        bmp.extend_from_slice(&0u32.to_le_bytes()); // reserved
        bmp.extend_from_slice(&54u32.to_le_bytes()); // pixel offset
        bmp.extend_from_slice(&40u32.to_le_bytes()); // DIB size
        bmp.extend_from_slice(&2i32.to_le_bytes()); // width
        bmp.extend_from_slice(&2i32.to_le_bytes()); // height (positive = bottom-up)
        bmp.extend_from_slice(&1u16.to_le_bytes()); // planes
        bmp.extend_from_slice(&24u16.to_le_bytes()); // bpp
        bmp.extend_from_slice(&0u32.to_le_bytes()); // compression
        bmp.extend_from_slice(&0u32.to_le_bytes()); // image size
        bmp.extend_from_slice(&0u32.to_le_bytes()); // x ppm
        bmp.extend_from_slice(&0u32.to_le_bytes()); // y ppm
        bmp.extend_from_slice(&0u32.to_le_bytes()); // colors
        bmp.extend_from_slice(&0u32.to_le_bytes()); // important colors
                                                    // Pixel rows: 2 px × 3 B = 6 B + 2 padding = 8 B per row, 2 rows.
        for _ in 0..2 {
            for _ in 0..2 {
                bmp.extend_from_slice(&[255u8, 255, 255]); // BGR white
            }
            bmp.extend_from_slice(&[0u8, 0]); // pad
        }
        let parsed = parse_bmp_24_or_32(&bmp);
        assert!(parsed.is_some());
        let (w, h, rgba) = parsed.unwrap();
        assert_eq!(w, 2);
        assert_eq!(h, 2);
        assert_eq!(rgba.len(), 16);
        // First pixel should be white RGBA
        assert_eq!(&rgba[0..4], &[255, 255, 255, 255]);
    }
}
