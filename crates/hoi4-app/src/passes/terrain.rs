//! Phase 3.12.4 鈥?`TerrainPass`锛氭妸鏃?inline 鍦板舰 pipeline 鍗囩骇涓?vanilla pdxmap 绛変环銆?//!
//! ## 鏇挎崲鍏崇郴
//!
//! - 鍘?`crates/hoi4-render/src/shader.wgsl` 鈫?绉诲埌褰掓。甯搁噺
//!   `hoi4_render::ARCHIVED_SHADER_MAIN_WGSL`锛屼粎浣滃巻鍙插弬鑰?//! - 鏂?`crates/hoi4-app/src/passes/terrain.wgsl` 鏄粍鍚堜綋锛?//!   淇濈暀鏃?chunk-tessellated 椤剁偣锛堟部鐢ㄧ幇鏈?`ChunkInstance` 瀹炰緥鏁版嵁 + LOD锛夛紝
//!   鐗囧厓渚ф寜 vanilla pdxmap 璺緞琛?atlas_normal / world_normal / city lights /
//!   shadow PCF / 瀛ｈ妭鏀挎不鑹?/ 鏄煎
//!
//! ## binding 甯冨眬锛? 涓?group锛?//!
//! - `@group(0)` 鈥?frame uniforms锛堟瘡 LOD 涓€涓?bind group锛屽洜涓?ChunkUniform 涓嶅悓锛?//!   - `@binding(0)` GlobalFrameUniform锛堝叡浜?352 瀛楄妭锛?//!   - `@binding(1)` PdxMapParams锛堟瘡甯ф洿鏂帮紱鍚?selected_pid / season / world_size锛?//!   - `@binding(2)` ChunkUniform锛坧er-LOD 4 瀛楄妭锛?//!
//! - `@group(1)` 鈥?璺?LOD 鍏变韩
//!   - shadow_map / shadow_sampler
//!   - season_map / color_map / color_map_second锛?.12.8 鎺?seasons.txt 鍓嶉兘鐢?colormap锛?//!   - light_data / light_index锛?.12.X 涔嬪墠鐢?1脳1 mock锛?//!   - gradient_border ch1/ch2锛?*澶嶇敤** country_sdf / province_sdf锛?.12.9 鍚庢帴 vanilla 18 寮狅級
//!   - province_secondary_color锛?脳1 mock锛?//!   - generic_sampler锛坙inear锛?//!   - occupation_lut / coast_sdf / rivers锛堝吋瀹圭幇鏈夌壒鎬э級
//!
//! - `@group(2)` 鈥?pass-specific 鍦板舰浣嶅浘
//!   - heightmap / province_id / terrain_idx / terrain_atlas
//!   - **NEW**: terrain_atlas_normal锛坅tlas_normal0.dds锛孊C5 鈫?Bc5RgUnorm锛?//!   - **NEW**: world_normal锛坵orld_normal.bmp锛? 閫氶亾 24-bit RGB锛?//!   - **NEW**: colormap_emissive锛坈olormap_rgb_cityemissivemask_a.dds锛孊C3 .a = emit mask锛?//!   - **NEW**: citylights锛坈itylights_rgb_snowmask_a_0.dds锛孊C3 .rgb = night light锛?//!   - country_color_lut锛坢ain.rs 鐨?lut_view锛?//!   - pass_sampler

use hoi4_assets::MapResRole;
use hoi4_paths::PathConfig;
use wgpu::util::DeviceExt;

use crate::passes::HDR_FORMAT;
use crate::vanilla_resource_views::{
    create_dynamic_target_1x1, upload_dds_or_fallback, BindingAudit, BindingAuditEntry,
    DdsUploadRequest, VanillaResourceViews,
};

const SHADER_WGSL: &str = include_str!("terrain.wgsl");

/// Public alias for the wgsl source 鈥?used by `tests/terrain_wgsl.rs` to
/// validate the shader via naga without recompiling the include.
pub const TERRAIN_WGSL_SOURCE: &str = SHADER_WGSL;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum TerrainDebugView {
    Off = 0,
    TerrainId = 1,
    AtlasTileId = 2,
    TerrainBlendState = 3,
    TerrainCorners = 4,
    PoliticalColor = 5,
    TerrainAlbedo = 6,
    Normal = 7,
    HeightSlope = 8,
    SnowMask = 9,
    MudMask = 10,
    RiverMask = 11,
    CityEmitMask = 12,
    CityLightsRgb = 13,
    NightFactor = 14,
    CityLightContribution = 15,
    MapUv = 16,
    MapPxGrid = 17,
    VanillaTileRepeat = 18,
    CitylightUv = 19,
}

impl TerrainDebugView {
    pub const ALL: [Self; 20] = [
        Self::Off,
        Self::TerrainId,
        Self::AtlasTileId,
        Self::TerrainBlendState,
        Self::TerrainCorners,
        Self::PoliticalColor,
        Self::TerrainAlbedo,
        Self::Normal,
        Self::HeightSlope,
        Self::SnowMask,
        Self::MudMask,
        Self::RiverMask,
        Self::CityEmitMask,
        Self::CityLightsRgb,
        Self::NightFactor,
        Self::CityLightContribution,
        Self::MapUv,
        Self::MapPxGrid,
        Self::VanillaTileRepeat,
        Self::CitylightUv,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::TerrainId => "terrain_id",
            Self::AtlasTileId => "atlas_tile_id",
            Self::TerrainBlendState => "terrain_blend_state",
            Self::TerrainCorners => "terrain_corners",
            Self::PoliticalColor => "political_color",
            Self::TerrainAlbedo => "terrain_albedo",
            Self::Normal => "normal",
            Self::HeightSlope => "height_slope",
            Self::SnowMask => "snow_mask",
            Self::MudMask => "mud_mask",
            Self::RiverMask => "river_mask",
            Self::CityEmitMask => "city_emit_mask",
            Self::CityLightsRgb => "citylights_rgb",
            Self::NightFactor => "night_factor",
            Self::CityLightContribution => "citylight_contribution",
            Self::MapUv => "map_uv",
            Self::MapPxGrid => "map_px_grid",
            Self::VanillaTileRepeat => "vanilla_tile_repeat",
            Self::CitylightUv => "citylight_uv",
        }
    }

    pub const fn as_shader_value(self) -> f32 {
        match self {
            Self::Off => 0.0,
            Self::TerrainId => 1.0,
            Self::AtlasTileId => 2.0,
            Self::TerrainBlendState => 3.0,
            Self::TerrainCorners => 4.0,
            Self::PoliticalColor => 5.0,
            Self::TerrainAlbedo => 6.0,
            Self::Normal => 7.0,
            Self::HeightSlope => 8.0,
            Self::SnowMask => 9.0,
            Self::MudMask => 10.0,
            Self::RiverMask => 11.0,
            Self::CityEmitMask => 12.0,
            Self::CityLightsRgb => 13.0,
            Self::NightFactor => 14.0,
            Self::CityLightContribution => 15.0,
            Self::MapUv => 16.0,
            Self::MapPxGrid => 17.0,
            Self::VanillaTileRepeat => 18.0,
            Self::CitylightUv => 19.0,
        }
    }

    pub fn next(self) -> Self {
        let idx = Self::ALL.iter().position(|view| *view == self).unwrap_or(0);
        Self::ALL[(idx + 1) % Self::ALL.len()]
    }
}

/// `PdxMapParams` mirrors the WGSL uniform in `terrain.wgsl`.
/// Size is 2176 bytes.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable, Debug)]
pub struct PdxMapParams {
    pub selected_province_id: u32,
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
    pub world_size_xy_height_lat: [f32; 4],
    pub season_params: [f32; 4],
    pub terrain_controls: [f32; 4],
    pub overlay_controls: [f32; 4],
    pub feature_flags: [f32; 4],
    pub atlas_idx_array: [[u32; 4]; 64],
    pub terrain_flags_array: [[u32; 4]; 64],
}

impl PdxMapParams {
    pub const VANILLA_PARITY_FEATURE_FLAGS: [f32; 4] = [0.0, 0.0, 0.0, 0.0];
    pub const LEGACY_ART_FEATURE_FLAGS: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
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
            feature_flags: Self::VANILLA_PARITY_FEATURE_FLAGS,
            atlas_idx_array: std::array::from_fn(|row| {
                std::array::from_fn(|col| ((row * 4 + col) & 15) as u32)
            }),
            terrain_flags_array: [[0; 4]; 64],
        }
    }
}

/// ChunkUniform mirrors the WGSL uniform for per-LOD grid size.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable, Debug)]
pub struct ChunkUniform {
    pub grid: u32,
    pub _pad: [u32; 3],
}

/// TerrainPass constructor inputs: all textures/buffers already uploaded in main.rs.
pub struct TerrainPassInputs<'a> {
    pub global_uniform_buffer: &'a wgpu::Buffer,
    pub depth_format: wgpu::TextureFormat,
    pub lod_grid: [u32; 3],
    pub shadow_map_view: &'a wgpu::TextureView,
    pub shadow_sampler: &'a wgpu::Sampler,
    pub colormap_view: &'a wgpu::TextureView,
    pub country_sdf_view: &'a wgpu::TextureView,
    pub province_sdf_view: &'a wgpu::TextureView,
    pub coast_sdf_view: &'a wgpu::TextureView,
    pub occupation_lut_view: &'a wgpu::TextureView,
    pub rivers_view: &'a wgpu::TextureView,
    pub heightmap_view: &'a wgpu::TextureView,
    pub province_view: &'a wgpu::TextureView,
    pub terrain_idx_view: &'a wgpu::TextureView,
    pub terrain_atlas_view: &'a wgpu::TextureView,
    pub country_color_lut_view: &'a wgpu::TextureView,
    pub vanilla_resources: &'a VanillaResourceViews,
}

/// Terrain pass state.
pub struct TerrainPass {
    pipeline: wgpu::RenderPipeline,
    bind_groups_g0: [wgpu::BindGroup; 3],
    bind_group_g1: wgpu::BindGroup,
    bind_group_g2: wgpu::BindGroup,
    params_buffer: wgpu::Buffer,
    chunk_buffers: [wgpu::Buffer; 3],
    _atlas_normal_tex: wgpu::Texture,
    _world_normal_tex: wgpu::Texture,
    _colormap_emissive_tex: wgpu::Texture,
    _citylights_tex: wgpu::Texture,
    _snow_normal_diffuse_tex: wgpu::Texture,
    _mud_diffuse_tex: wgpu::Texture,
    _mud_normal_tex: wgpu::Texture,
    _stub_textures: Vec<wgpu::Texture>,
    _samplers: Vec<wgpu::Sampler>,
    pub load_warnings: Vec<String>,
    pub binding_audit: BindingAudit,
}

impl TerrainPass {
    /// Construct a complete `TerrainPass`.
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _path_cfg: &PathConfig,
        inputs: TerrainPassInputs<'_>,
    ) -> Self {
        let mut warnings: Vec<String> = Vec::new();

        let mut binding_audit = BindingAudit::new();

        let atlas_normal = upload_dds_or_fallback(
            device,
            queue,
            inputs.vanilla_resources,
            DdsUploadRequest {
                role: MapResRole::TerrainAtlasNormal(0),
                label: "terrain_atlas_normal0",
                fallback_rgba: [128, 128, 255, 255],
                srgb: false,
                critical: true,
                pass: "terrain",
                binding: "terrain_atlas_normal",
                visual_impact: "terrain lighting and material normals flatten",
            },
            &mut warnings,
        );
        binding_audit.extend([atlas_normal.audit.clone()]);
        let atlas_normal_tex = atlas_normal.texture;
        let atlas_normal_view = atlas_normal.view;

        let (world_normal_tex, world_normal_view, world_normal_audit) =
            load_world_normal_bmp(device, queue, inputs.vanilla_resources, &mut warnings);
        binding_audit.extend([world_normal_audit]);

        let colormap_emissive = upload_dds_or_fallback(
            device,
            queue,
            inputs.vanilla_resources,
            DdsUploadRequest {
                role: MapResRole::ColormapEmissive,
                label: "colormap_rgb_cityemissivemask_a",
                fallback_rgba: [128, 128, 96, 0],
                srgb: true,
                critical: true,
                pass: "terrain",
                binding: "colormap_emissive",
                visual_impact: "terrain tint and city emissive mask are unavailable",
            },
            &mut warnings,
        );
        binding_audit.extend([colormap_emissive.audit.clone()]);
        let colormap_emissive_tex = colormap_emissive.texture;
        let colormap_emissive_view = colormap_emissive.view;

        let citylights = upload_dds_or_fallback(
            device,
            queue,
            inputs.vanilla_resources,
            DdsUploadRequest {
                role: MapResRole::CityLights(0),
                label: "citylights0",
                fallback_rgba: [0, 0, 0, 0],
                srgb: true,
                critical: true,
                pass: "terrain",
                binding: "citylights",
                visual_impact: "night city light contribution disappears",
            },
            &mut warnings,
        );
        binding_audit.extend([citylights.audit.clone()]);
        let citylights_tex = citylights.texture;
        let citylights_view = citylights.view;

        let snow_normal_diffuse = upload_dds_or_fallback(
            device,
            queue,
            inputs.vanilla_resources,
            DdsUploadRequest {
                role: MapResRole::SnowNormalDiffuse,
                label: "snow_normal_rgb_diffuse_a",
                fallback_rgba: [128, 128, 255, 0],
                srgb: false,
                critical: true,
                pass: "terrain",
                binding: "snow_normal_diffuse",
                visual_impact: "snow overlay loses vanilla normal/diffuse texture",
            },
            &mut warnings,
        );
        binding_audit.extend([snow_normal_diffuse.audit.clone()]);
        let snow_normal_diffuse_tex = snow_normal_diffuse.texture;
        let snow_normal_diffuse_view = snow_normal_diffuse.view;

        let mud_diffuse = upload_dds_or_fallback(
            device,
            queue,
            inputs.vanilla_resources,
            DdsUploadRequest {
                role: MapResRole::MudDiffuseGloss(0),
                label: "mud_diffuse_rgb_gloss_a_0",
                fallback_rgba: [78, 62, 45, 0],
                srgb: true,
                critical: true,
                pass: "terrain",
                binding: "mud_diffuse_gloss",
                visual_impact: "mud overlay falls back to a flat brown tint",
            },
            &mut warnings,
        );
        binding_audit.extend([mud_diffuse.audit.clone()]);
        let mud_diffuse_tex = mud_diffuse.texture;
        let mud_diffuse_view = mud_diffuse.view;

        let mud_normal = upload_dds_or_fallback(
            device,
            queue,
            inputs.vanilla_resources,
            DdsUploadRequest {
                role: MapResRole::MudNormalSpec(0),
                label: "mud_normal_rgb_spec_a_0",
                fallback_rgba: [128, 128, 255, 0],
                srgb: false,
                critical: true,
                pass: "terrain",
                binding: "mud_normal_spec",
                visual_impact: "mud overlay normals/specular flatten",
            },
            &mut warnings,
        );
        binding_audit.extend([mud_normal.audit.clone()]);
        let mud_normal_tex = mud_normal.texture;
        let mud_normal_view = mud_normal.view;

        // 鈹€鈹€ Stub textures for not-yet-implemented bindings 鈹€鈹€
        let mut stub_textures: Vec<wgpu::Texture> = Vec::new();
        let mut samplers: Vec<wgpu::Sampler> = Vec::new();

        // light_data / light_index (3.12.x: real point lights). Mock 1脳1.
        let (light_data_tex, light_data_view) =
            create_dynamic_target_1x1(device, queue, "light_data_empty_target", [0, 0, 0, 0]);
        let (light_index_tex, light_index_view) = create_dynamic_target_1x1(
            device,
            queue,
            "light_index_empty_target",
            [255, 255, 255, 255],
        );
        stub_textures.push(light_data_tex);
        stub_textures.push(light_index_tex);

        let (secondary_color_tex, secondary_color_view) = create_dynamic_target_1x1(
            device,
            queue,
            "province_secondary_color_empty_target",
            [0, 0, 0, 0],
        );
        stub_textures.push(secondary_color_tex);
        binding_audit.extend([
            BindingAuditEntry::dynamic_target_blocker(
                "terrain",
                "light_data",
                "light_data_empty_target",
                "Vanilla point light render target is not generated yet",
                "night lighting and local highlights are missing",
            ),
            BindingAuditEntry::dynamic_target_blocker(
                "terrain",
                "light_index",
                "light_index_empty_target",
                "Vanilla point light index target is not generated yet",
                "point light lookup is disabled",
            ),
            BindingAuditEntry::dynamic_target_blocker(
                "terrain",
                "province_secondary_color",
                "province_secondary_color_empty_target",
                "Province secondary color target is not generated yet",
                "occupation, selection, and map-mode secondary tint are absent",
            ),
            BindingAuditEntry::dynamic_target(
                "terrain",
                "gradient_border_ch1",
                "country_sdf",
                "temporary SDF input used until vanilla gradient border target exists",
            ),
            BindingAuditEntry::dynamic_target(
                "terrain",
                "gradient_border_ch2",
                "province_sdf",
                "temporary SDF input used until vanilla gradient border target exists",
            ),
        ]);

        // 鈹€鈹€ Samplers 鈹€鈹€
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

        // 鈹€鈹€ Uniform buffers 鈹€鈹€
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

        // 鈹€鈹€ Bind group layouts 鈹€鈹€
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
                // 10: snow_normal_rgb_diffuse_a
                texture_entry(10),
                // 11: mud_diffuse_rgb_gloss_a_0
                texture_entry(11),
                // 12: mud_normal_rgb_spec_a_0
                texture_entry(12),
            ],
        });

        // 鈹€鈹€ Build bind groups 鈹€鈹€
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
                wgpu::BindGroupEntry {
                    binding: 10,
                    resource: wgpu::BindingResource::TextureView(&snow_normal_diffuse_view),
                },
                wgpu::BindGroupEntry {
                    binding: 11,
                    resource: wgpu::BindingResource::TextureView(&mud_diffuse_view),
                },
                wgpu::BindGroupEntry {
                    binding: 12,
                    resource: wgpu::BindingResource::TextureView(&mud_normal_view),
                },
            ],
        });

        // 鈹€鈹€ Pipeline 鈹€鈹€
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
            _snow_normal_diffuse_tex: snow_normal_diffuse_tex,
            _mud_diffuse_tex: mud_diffuse_tex,
            _mud_normal_tex: mud_normal_tex,
            _stub_textures: stub_textures,
            _samplers: samplers,
            load_warnings: warnings,
            binding_audit,
        }
    }

    /// Update the per-frame `PdxMapParams` uniform.
    pub fn update_params(&self, queue: &wgpu::Queue, params: &PdxMapParams) {
        queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(params));
    }

    /// Submit terrain draws for each populated LOD inside an active render pass.
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

/// Load vanilla `world_normal.bmp` into a non-sRGB `Rgba8Unorm` texture.
fn load_world_normal_bmp(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    resources: &VanillaResourceViews,
    warnings: &mut Vec<String>,
) -> (wgpu::Texture, wgpu::TextureView, BindingAuditEntry) {
    let label = "world_normal";
    let role = MapResRole::WorldNormal;
    let bytes = resources.bytes(role);
    let bytes = match bytes {
        Some(b) if !b.is_empty() => b,
        _ => {
            let reason = if bytes.is_some() {
                "empty_resource"
            } else {
                "missing_resource"
            };
            warnings.push(format!(
                "[terrain] world_normal ({}) using flat-normal fallback: {}",
                role.relative_path(),
                reason
            ));
            let (texture, view) = create_dynamic_target_1x1(
                device,
                queue,
                "world_normal_fallback",
                [128, 128, 255, 255],
            );
            return (
                texture,
                view,
                BindingAuditEntry::vanilla(
                    "terrain",
                    "world_normal",
                    role,
                    false,
                    true,
                    Some(reason.to_string()),
                    "large-scale terrain lighting normal falls back to flat",
                ),
            );
        }
    };

    // Parse BMP header: minimum supported = 24-bit BI_RGB / 32-bit BGRA / BI_BITFIELDS.
    let parsed = parse_bmp_24_or_32(&bytes);
    let (w, h, rgba) = match parsed {
        Some(t) => t,
        None => {
            warnings.push(format!(
                "[terrain] world_normal ({}) using flat-normal fallback: bmp_parse_failed",
                role.relative_path()
            ));
            let (texture, view) = create_dynamic_target_1x1(
                device,
                queue,
                "world_normal_fallback",
                [128, 128, 255, 255],
            );
            return (
                texture,
                view,
                BindingAuditEntry::vanilla(
                    "terrain",
                    "world_normal",
                    role,
                    false,
                    true,
                    Some("bmp_parse_failed".to_string()),
                    "large-scale terrain lighting normal falls back to flat",
                ),
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
        "[terrain_pass] loaded {} ({}脳{} BMP鈫扲gba8Unorm)",
        label, w, h
    );
    (
        texture,
        view,
        BindingAuditEntry::vanilla(
            "terrain",
            "world_normal",
            role,
            true,
            true,
            None,
            "large-scale terrain lighting normal",
        ),
    )
}

/// Minimal BMP 24-bit / 32-bit decoder returning top-down RGBA bytes.
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

// Helper used only to keep samplers owned alongside their bind groups.
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
        // 8 vec4-sized groups plus two 256-entry LUTs packed as 64 vec4 rows.
        assert_eq!(std::mem::size_of::<PdxMapParams>(), 2176);
    }

    #[test]
    fn terrain_debug_view_cycles_through_shader_values() {
        assert_eq!(TerrainDebugView::Off.next(), TerrainDebugView::TerrainId);
        assert_eq!(
            TerrainDebugView::RiverMask.next(),
            TerrainDebugView::CityEmitMask
        );
        assert_eq!(TerrainDebugView::CitylightUv.next(), TerrainDebugView::Off);
        assert_eq!(TerrainDebugView::Normal.as_shader_value(), 7.0);
        assert_eq!(TerrainDebugView::AtlasTileId.name(), "atlas_tile_id");
        assert_eq!(TerrainDebugView::VanillaTileRepeat.as_shader_value(), 18.0);
    }

    #[test]
    fn chunk_uniform_size_is_16() {
        assert_eq!(std::mem::size_of::<ChunkUniform>(), 16);
    }

    #[test]
    fn parse_bmp_simple_24bit() {
        // 2脳2 BMP, 24-bit, all white pixels.
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
                                                    // Pixel rows: 2 px 脳 3 B = 6 B + 2 padding = 8 B per row, 2 rows.
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
