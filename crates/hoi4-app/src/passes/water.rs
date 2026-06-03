//! Phase 6 �?`WaterPass`：把 vanilla `pdxwater.shader` 语义接到屏幕上�?//!
//! **替代�?*：`terrain.wgsl::is_water` 分支（深度渐�?+ 程序化法�?foam +
//! Phong 高光）—�?现在由本 pass 在独�?pipeline 里完成�?//!
//! **水面法线**：vanilla `pdxwater.shader` 4-tap LEAN 法线混合�?//! `lean1.dds` / `lean2.dds` �?LEAN（Linear Efficient Antialiased Normal
//! mapping）格�?—�?RG=mean(N.xy)，BA=方差�? 频率滚动采样后按 LEAN
//! moments 混合�?//!
//! Phase 6 材质路径覆盖�?//!
//! 1. **SampleWater**：`colormap_water_0/1/2.dds` + `underwater_terrain_0.dds`
//! 2. **reflection/refraction**：`reflection.dds`、`reflection_land_unit.dds`�?//!    environment cube fallback �?Fresnel 混合
//! 3. **water spec**：`fow_rgb_waterspec_a.dds` �?A 通道驱动太阳高光
//! 4. **ApplyIce**：`ice_diffuse.dds` + `ice_noise_0/1.dds`
//! 5. **runtime targets**：GradientBorderChannel1/2/3、ProvinceSecondaryColorMap�?//!    FOW �?distance fog
//! 6. **point lights**：共�?Phase 5 �?`LightDataMap` / `LightIndexMap`
//!    runtime targets，水面接收与地形/�?建筑一致的本地点光�?//!
//! ## 几何复用策略
//!
//! �?pass **�?*新建 mesh �?直接复用 [`crate::passes::TerrainPass`] �?//! main.rs 里上传的 per-LOD instance buffer + chunk uniform。WaterPass 自己
//! �?vertex shader：与 terrain.wgsl 字面同款�?chunk �?grid �?world_xz"
//! 路径，但�?world_y 始终钳到 `SEA_LEVEL × height_scale` 让海面是一�?//! **平的** sea quad（不跟随 heightmap 凹凸）�?//!
//! Fragment 端先采样 heightmap，若 `h > SEA_LEVEL` 直接 `discard`，避免画�?//! 陆地像素上�?//!
//! ## 渲染顺序
//!
//! WaterPass **�?TerrainPass 之后**绘制，使�?`depth_compare = LessEqual` +
//! `depth_write = false`�?//!
//! - 终于覆盖�?terrain 已经画在水面上的程序化水（视觉等价替换）�?//! - 不写深度，让 trees / buildings / units 仍能正确写入 z-buffer�?//! - alpha-blend 关，整像素替换（vanilla 也是不透明海面）�?//!
//! ## �?terrain pass 的水分支共存
//!
//! Phase 6 选择**保留** terrain pass 的水分支作为 fallback。理由：
//!
//! 1. mod 环境如果 vanilla `lean1.dds` / `colormap_water_0.dds` 等缺失，
//!    `WaterPass::any_loaded == false` �?main.rs 跳过 `WaterPass::render()`�?//!    terrain pass 的程序化水仍然兜底——不会黑屏；
//! 2. 水面位置 z-bias 完全一致（都是 `SEA_LEVEL × height_scale`），WaterPass
//!    覆盖 terrain water 像素�?*像素级替�?*而非透明叠加�?//! 3. 改动 terrain.wgsl 的代�?> 收益：terrain water 已经长大不影响视觉了�?//!
//! 后续若要�?PostProcessPass 的真平面反射 RT，再回来�?terrain water
//! fallback 分支收掉�?//!
//! ## EnvironmentMap cubemap
//!
//! �?[`crate::passes::PdxMeshPass`]，vanilla 不发�?`gfx/cubemaps/*.dds`�?//! 这里�?1×1×6 dim-blue `(80, 110, 150, 255)` 立方�?fallback 作为地平�?//! 反射的占位。后�?sky pass 完成后替换为 `gfx/loadingscreens/sky_*.dds`
//! �?6 个面�?
#![allow(dead_code)]

use hoi4_assets::MapResRole;
use hoi4_paths::PathConfig;
use wgpu::util::DeviceExt;

use crate::passes::HDR_FORMAT;
use crate::vanilla_resource_views::{
    upload_dds_or_fallback, BindingAudit, BindingAuditEntry, BindingBlockingLevel,
    DdsUploadRequest, VanillaResourceViews,
};
use crate::vanilla_targets::VanillaRuntimeTargets;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum WaterDebugView {
    Off = 0,
    DepthRatio = 1,
    CoastDistance = 2,
    NormalStrength = 3,
    FoamMask = 4,
    IceMask = 5,
    ReflectionContribution = 6,
    FinalWaterOnly = 7,
}

impl WaterDebugView {
    pub const ALL: [Self; 8] = [
        Self::Off,
        Self::DepthRatio,
        Self::CoastDistance,
        Self::NormalStrength,
        Self::FoamMask,
        Self::IceMask,
        Self::ReflectionContribution,
        Self::FinalWaterOnly,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::DepthRatio => "depth_ratio",
            Self::CoastDistance => "coast_distance",
            Self::NormalStrength => "normal_strength",
            Self::FoamMask => "foam_mask",
            Self::IceMask => "ice_mask",
            Self::ReflectionContribution => "reflection_contribution",
            Self::FinalWaterOnly => "final_water_only",
        }
    }

    pub const fn as_shader_value(self) -> u32 {
        self as u32
    }

    pub fn next(self) -> Self {
        let idx = Self::ALL.iter().position(|view| *view == self).unwrap_or(0);
        Self::ALL[(idx + 1) % Self::ALL.len()]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WaterMaterialComponent {
    Color,
    Normal,
    Specular,
    Reflection,
    Ice,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct WaterTextureSpec {
    role: MapResRole,
    fallback_rgba: [u8; 4],
    component: WaterMaterialComponent,
    critical: bool,
}

impl WaterTextureSpec {
    const fn new(
        role: MapResRole,
        fallback_rgba: [u8; 4],
        component: WaterMaterialComponent,
        critical: bool,
    ) -> Self {
        Self {
            role,
            fallback_rgba,
            component,
            critical,
        }
    }

    fn srgb(self) -> bool {
        self.role.is_srgb()
    }
}

pub struct WaterMaterialSystem;

impl WaterMaterialSystem {
    const TEXTURES: [WaterTextureSpec; 12] = [
        WaterTextureSpec::new(
            MapResRole::Lean1,
            [128, 128, 255, 255],
            WaterMaterialComponent::Normal,
            true,
        ),
        WaterTextureSpec::new(
            MapResRole::Lean2,
            [128, 128, 255, 255],
            WaterMaterialComponent::Normal,
            true,
        ),
        WaterTextureSpec::new(
            MapResRole::Reflection,
            [60, 90, 130, 255],
            WaterMaterialComponent::Reflection,
            false,
        ),
        WaterTextureSpec::new(
            MapResRole::FowWaterSpec,
            [255, 255, 255, 255],
            WaterMaterialComponent::Specular,
            false,
        ),
        WaterTextureSpec::new(
            MapResRole::ColormapWater(0),
            [40, 80, 120, 255],
            WaterMaterialComponent::Color,
            true,
        ),
        WaterTextureSpec::new(
            MapResRole::ColormapWater(1),
            [40, 82, 126, 255],
            WaterMaterialComponent::Color,
            false,
        ),
        WaterTextureSpec::new(
            MapResRole::ColormapWater(2),
            [34, 72, 118, 255],
            WaterMaterialComponent::Color,
            false,
        ),
        WaterTextureSpec::new(
            MapResRole::IceDiffuse,
            [220, 230, 240, 255],
            WaterMaterialComponent::Ice,
            false,
        ),
        WaterTextureSpec::new(
            MapResRole::IceNoise(0),
            [128, 128, 128, 255],
            WaterMaterialComponent::Ice,
            false,
        ),
        WaterTextureSpec::new(
            MapResRole::IceNoise(1),
            [128, 128, 128, 255],
            WaterMaterialComponent::Ice,
            false,
        ),
        WaterTextureSpec::new(
            MapResRole::ReflectionLandUnit,
            [90, 100, 105, 255],
            WaterMaterialComponent::Reflection,
            false,
        ),
        WaterTextureSpec::new(
            MapResRole::UnderwaterTerrain(0),
            [35, 70, 95, 255],
            WaterMaterialComponent::Color,
            false,
        ),
    ];

    fn texture_specs() -> &'static [WaterTextureSpec; 12] {
        &Self::TEXTURES
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaterEffectVariant {
    WaterLowGfx,
    WaterNoRefractions,
    Water,
}

impl WaterEffectVariant {
    pub const fn as_shader_value(self) -> u32 {
        match self {
            Self::WaterLowGfx => 0,
            Self::WaterNoRefractions => 1,
            Self::Water => 2,
        }
    }

    pub const fn effect_name(self) -> &'static str {
        match self {
            Self::WaterLowGfx => "pdxwater:water_low_gfx",
            Self::WaterNoRefractions => "pdxwater:water_no_refractions",
            Self::Water => "pdxwater:water",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WaterEffectSelector {
    pub high_graphics_byte_0x18: bool,
    pub draw_refractions: bool,
    pub refraction_byte_0x15: bool,
}

impl Default for WaterEffectSelector {
    fn default() -> Self {
        Self {
            high_graphics_byte_0x18: true,
            draw_refractions: false,
            refraction_byte_0x15: false,
        }
    }
}

impl WaterEffectSelector {
    pub const fn select(self) -> WaterEffectVariant {
        if !self.high_graphics_byte_0x18 {
            WaterEffectVariant::WaterLowGfx
        } else if self.draw_refractions && self.refraction_byte_0x15 {
            WaterEffectVariant::Water
        } else {
            WaterEffectVariant::WaterNoRefractions
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct WaterTextureLoadStats {
    pub loaded: usize,
    pub fallback: usize,
    pub critical_missing: usize,
}

struct LoadedWaterTexture {
    view: wgpu::TextureView,
    loaded: bool,
    critical: bool,
    audit: crate::vanilla_resource_views::BindingAuditEntry,
}

// ─── Water uniform（与 wgsl `WaterParams` 字面对齐）─────────────────────────

/// 64 bytes�? × f32 + 8 × u32�? × vec4 std140 槽）。与 [`WATER_WGSL`] `struct WaterParams`
/// 字段顺序一致�?///
/// **法线路径**：vanilla LEAN 4-tap（RG=mean(N.xy)�? 频率滚动采样平均）�?#[repr(C)]
#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct WaterParams {
    pub time_speed: f32,
    pub fresnel_power: f32,
    pub foam_threshold: f32,
    pub ice_latitude: f32,
    pub world_w: f32,
    pub world_d: f32,
    pub height_scale: f32,
    pub _pad: f32,
    pub selected_province_id: u32,
    pub debug_view: u32,
    pub final_water_owner: u32,
    /// 全局时间乘子（控制涟漪推进速度，vanilla ~0.04�?=静止）�?    pub time_speed: f32,
    /// Fresnel 指数（越高边缘高光越窄，vanilla 4.0~5.0）�?    pub fresnel_power: f32,
    /// foam 阈�?�?**像素**单位距海岸（�?terrain.wgsl �?`coast_dist_px`
    /// 同约定）。SDF u8 �?chamfer 出来�?0..255 px 距离；shader �?`*255`
    /// 复原为像素后比对�?.5 �?海岸 1.5 像素以内才出现白沫，�?vanilla
    /// "细线 foam" 视觉吻合�?    pub foam_threshold: f32,
    /// 极地冰层覆盖纬度阈值。值越大，南北两端的冰带越窄�?    pub ice_latitude: f32,
    /// 世界 X 维度（main.rs �?`WORLD_SCALE × heightmap.width`）；
    /// FS 端反�?map_uv 必需�?    pub world_w: f32,
    /// 世界 Z 维度�?    pub world_d: f32,
    /// 高度乘子（vertex 端把海面 Y 钳到 `SEA_LEVEL × height_scale`）�?    pub height_scale: f32,
    /// 16-byte 对齐填充�?    pub _pad: f32,
    /// 当前选中省份 id；u32::MAX = 无选中�?    pub selected_province_id: u32,
    /// 当前 water debug view�? = off�?    pub debug_view: u32,
    /// WaterPass 是否拥有最终可见水体颜色；0 �?fragment 直接 discard�?    pub final_water_owner: u32,
    pub effect_variant: u32,
    /// depth debug scale / coast debug max px / reserved / reserved.
    pub debug_controls: [f32; 4],
}

impl Default for WaterParams {
    fn default() -> Self {
        Self {
            time_speed: 0.04,
            fresnel_power: 4.5,
            foam_threshold: 1.15,
            ice_latitude: 1.01,
            // 默认值会�?main.rs �?WaterPass::new 里覆�?            world_w: 112.0,
            world_w: 112.0,
            world_d: 41.0,
            height_scale: 4.0,
            _pad: 0.0,
            selected_province_id: u32::MAX,
            debug_view: WaterDebugView::Off.as_shader_value(),
            final_water_owner: 1,
            effect_variant: WaterEffectSelector::default().select().as_shader_value(),
            debug_controls: [1.0, 24.0, 0.0, 0.0],
        }
    }
}

const _: () = assert!(std::mem::size_of::<WaterParams>() == 64);

// ─── Chunk uniform（与 terrain pass 同款，重复定义避免互�?import）──────────

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable, Debug)]
pub struct WaterChunkUniform {
    pub grid: u32,
    pub _pad: [u32; 3],
}

const _: () = assert!(std::mem::size_of::<WaterChunkUniform>() == 16);

// ─── WaterPass ────────────────────────────────────────────────────────────

pub struct WaterPass {
    pipeline: wgpu::RenderPipeline,
    bind_groups_g0: [wgpu::BindGroup; 3],
    bind_group_g1: wgpu::BindGroup,
    /// Per-LOD bind groups（chunk uniform 不同；其�?uniform 共享）�?    bind_groups_g0: [wgpu::BindGroup; 3],
    /// 共享：env cube + sampler�?    bind_group_g1: wgpu::BindGroup,
    bgl_g1: wgpu::BindGroupLayout,
    env_sampler: wgpu::Sampler,
    bind_group_g2: wgpu::BindGroup,
    params_buffer: wgpu::Buffer,
    /// 共享：water 纹理 + sampler�?    bind_group_g2: wgpu::BindGroup,
    /// `WaterParams` GPU buffer�?    params_buffer: wgpu::Buffer,
    chunk_buffers: [wgpu::Buffer; 3],
    /// Material-critical vanilla water resources are available; otherwise render is skipped.
    pub any_loaded: bool,
    pub selected_effect: WaterEffectVariant,
    pub texture_load_stats: WaterTextureLoadStats,
    pub load_warnings: Vec<String>,
    /// 启动 banner 用�?    pub load_warnings: Vec<String>,
    pub binding_audit: BindingAudit,
    _owned_textures: Vec<wgpu::Texture>,
    // 持有以延长生命周�?    _owned_textures: Vec<wgpu::Texture>,
    _owned_samplers: Vec<wgpu::Sampler>,
}

/// WaterPass 构造参数。所�?已经�?main.rs 上传"的资源都从这里传入�?pub struct WaterPassInputs<'a> {
pub struct WaterPassInputs<'a> {
    pub global_uniform_buffer: &'a wgpu::Buffer,
    pub depth_format: wgpu::TextureFormat,
    pub lod_grid: [u32; 3],
    /// �?terrain pass 共享�?heightmap（FS 用来 discard 陆地像素 + 算深度梯度）�?    pub heightmap_view: &'a wgpu::TextureView,
    /// �?terrain pass 共享�?province id 纹理（FS 用于海域选中高亮）�?    pub province_view: &'a wgpu::TextureView,
    /// 世界 X / Z 尺度（与 `terrain::PdxMapParams::world_size_xy_height_lat.xy` 同值）�?    /// 用于 FS �?`world_xz �?map_uv` 反推；mod �?WORLD_SCALE 时务必一致�?    pub world_size: [f32; 2],
    /// 高度乘子（与 `main.rs::HEIGHT_SCALE` 同值），决定海�?vertex Y�?    pub height_scale: f32,
    pub heightmap_view: &'a wgpu::TextureView,
    pub province_view: &'a wgpu::TextureView,
    pub world_size: [f32; 2],
    pub height_scale: f32,
    pub vanilla_resources: &'a VanillaResourceViews,
    pub runtime_targets: &'a VanillaRuntimeTargets,
}

impl WaterPass {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _path_cfg: &PathConfig,
        inputs: WaterPassInputs<'_>,
    ) -> Self {
        let mut warnings = Vec::new();
        let mut owned_textures: Vec<wgpu::Texture> = Vec::new();

        // ── shader 模块 ─────────────────────────────────────────────────
        let composed = hoi4_render::shader_rt::compose_shader(WATER_WGSL, true, true);
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("water_shader"),
            source: wgpu::ShaderSource::Wgsl(composed.into()),
        });

        // ── WaterParams uniform ────────────────────────────────────────
        let selected_effect = WaterEffectSelector::default().select();
        let params_init = WaterParams {
            world_w: inputs.world_size[0],
            world_d: inputs.world_size[1],
            height_scale: inputs.height_scale,
            effect_variant: selected_effect.as_shader_value(),
            ..WaterParams::default()
        };
        let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("water_params"),
            contents: bytemuck::bytes_of(&params_init),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // ── Chunk uniforms (per-LOD) ───────────────────────────────────
        let chunk_buffers: [wgpu::Buffer; 3] = std::array::from_fn(|i| {
            let u = WaterChunkUniform {
                grid: inputs.lod_grid[i],
                _pad: [0; 3],
            };
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("water_chunk_uniform"),
                contents: bytemuck::bytes_of(&u),
                usage: wgpu::BufferUsages::UNIFORM,
            })
        });

        let loaded_water_textures = load_water_material_textures(
            device,
            queue,
            inputs.vanilla_resources,
            &mut warnings,
            &mut owned_textures,
        );
        let texture_load_stats = water_texture_load_stats(&loaded_water_textures);
        let mut binding_audit = BindingAudit::new();
        binding_audit.extend(
            loaded_water_textures
                .iter()
                .map(|texture| texture.audit.clone()),
        );
        binding_audit.extend(
            inputs
                .runtime_targets
                .binding_audit_entries_for_pass("water"),
        );
        binding_audit.extend(water_p5_state_audit_entries(selected_effect));
        let [lean1_tex, lean2_tex, reflection_tex, fow_water_spec_tex, colormap_water_0_tex, colormap_water_1_tex, colormap_water_2_tex, ice_diffuse_tex, ice_noise_0_tex, ice_noise_1_tex, reflection_land_unit_tex, underwater_terrain_tex] =
            loaded_water_textures;

        // ── env cube fallback (1×1×6 dim-blue) ─────────────────────────
        let (env_cube_tex, env_cube_view) = create_dim_blue_cubemap(device, queue);
        owned_textures.push(env_cube_tex);

        // ── samplers ──────────────────────────────────────────────────
        let water_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("water_sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let water_map_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("water_map_sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let env_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("water_env_sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let heightmap_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("water_heightmap_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            // Heightmap is uploaded as a non-filterable Float texture (single
            // channel, no mipmaps used by water pass) �?wgpu requires the
            // bound sampler's filtering flags to match the BGL declaration.
            // NonFiltering = all three filter modes must be Nearest.
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        // ── BGL g0：global frame + water params + chunk + heightmap + sampler + province id ──
        let bgl_g0 = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("water_bgl_g0"),
            entries: &[
                // 0: GlobalFrameUniform
                uniform_entry(0, wgpu::ShaderStages::VERTEX_FRAGMENT),
                // 1: WaterParams
                uniform_entry(1, wgpu::ShaderStages::VERTEX_FRAGMENT),
                // 2: ChunkUniform
                uniform_entry(2, wgpu::ShaderStages::VERTEX),
                // 3: heightmap (vertex sampling for Y clamp + fragment for discard mask)
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // 4: heightmap sampler (non-filtering since heightmap is single-channel float)
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                    count: None,
                },
                // 5: province id map (R16Uint) for selected sea-region highlight.
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Uint,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });

        let bind_groups_g0: [wgpu::BindGroup; 3] = std::array::from_fn(|lod| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("water_bg_g0"),
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
                        resource: chunk_buffers[lod].as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::TextureView(inputs.heightmap_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: wgpu::BindingResource::Sampler(&heightmap_sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 5,
                        resource: wgpu::BindingResource::TextureView(inputs.province_view),
                    },
                ],
            })
        });

        // ── BGL g1：env cube + sampler ──────────────────────────────────
        let bgl_g1 = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("water_bgl_g1"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::Cube,
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
            label: Some("water_bg_g1"),
            layout: &bgl_g1,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&env_cube_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&env_sampler),
                },
            ],
        });

        // ── BGL g2：water 纹理�? 张）+ sampler ─────────────────────────
        let bgl_g2 = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("water_bgl_g2"),
            entries: &[
                fragment_tex_entry(0),
                fragment_tex_entry(1),
                fragment_tex_entry(2),
                fragment_tex_entry(3),
                fragment_tex_entry(4),
                fragment_tex_entry(5),
                fragment_tex_entry(6),
                fragment_tex_entry(7),
                wgpu::BindGroupLayoutEntry {
                    binding: 8,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                fragment_tex_entry(9),
                fragment_tex_entry(10),
                fragment_tex_entry(11),
                fragment_tex_entry(12),
                fragment_tex_entry(13),
                fragment_tex_entry(14),
                fragment_tex_entry(15),
                fragment_tex_entry(16),
                fragment_tex_entry(17),
                fragment_tex_entry(18),
                fragment_tex_entry_nonfilter(19),
                fragment_tex_entry(20),
                fragment_tex_entry(21),
                wgpu::BindGroupLayoutEntry {
                    binding: 22,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let bind_group_g2 = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("water_bg_g2"),
            layout: &bgl_g2,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&lean1_tex.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&lean2_tex.view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&reflection_tex.view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&fow_water_spec_tex.view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&colormap_water_0_tex.view),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::TextureView(&colormap_water_1_tex.view),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::TextureView(&colormap_water_2_tex.view),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: wgpu::BindingResource::TextureView(&ice_diffuse_tex.view),
                },
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: wgpu::BindingResource::Sampler(&water_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 9,
                    resource: wgpu::BindingResource::TextureView(&ice_noise_0_tex.view),
                },
                wgpu::BindGroupEntry {
                    binding: 10,
                    resource: wgpu::BindingResource::TextureView(&ice_noise_1_tex.view),
                },
                wgpu::BindGroupEntry {
                    binding: 11,
                    resource: wgpu::BindingResource::TextureView(&reflection_land_unit_tex.view),
                },
                wgpu::BindGroupEntry {
                    binding: 12,
                    resource: wgpu::BindingResource::TextureView(&underwater_terrain_tex.view),
                },
                wgpu::BindGroupEntry {
                    binding: 13,
                    resource: wgpu::BindingResource::TextureView(
                        &inputs.runtime_targets.projected_shadow_fow.view,
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 14,
                    resource: wgpu::BindingResource::TextureView(
                        &inputs.runtime_targets.gradient_border.ch1.view,
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 15,
                    resource: wgpu::BindingResource::TextureView(
                        &inputs.runtime_targets.gradient_border.ch2.view,
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 16,
                    resource: wgpu::BindingResource::TextureView(
                        &inputs.runtime_targets.gradient_border.ch3.view,
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 17,
                    resource: wgpu::BindingResource::TextureView(
                        &inputs.runtime_targets.province_secondary_color.view,
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 18,
                    resource: wgpu::BindingResource::TextureView(&inputs.runtime_targets.fow.view),
                },
                wgpu::BindGroupEntry {
                    binding: 19,
                    resource: wgpu::BindingResource::TextureView(
                        &inputs.runtime_targets.light_data.view,
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 20,
                    resource: wgpu::BindingResource::TextureView(
                        &inputs.runtime_targets.light_index.view,
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 21,
                    resource: wgpu::BindingResource::TextureView(
                        &inputs.runtime_targets.mud_snow.view,
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 22,
                    resource: wgpu::BindingResource::Sampler(&water_map_sampler),
                },
            ],
        });

        // ── pipeline ───────────────────────────────────────────────────
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("water_pl"),
            bind_group_layouts: &[&bgl_g0, &bgl_g1, &bgl_g2],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("water_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[
                    // �?terrain instance buffer 字面对齐：origin_xz (vec2) + size_xz (vec2)
                    wgpu::VertexBufferLayout {
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
                    },
                ],
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
                        alpha: wgpu::BlendComponent::REPLACE,
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
                // 不写深度：让 trees / buildings / units 仍能正确深度测试
                depth_write_enabled: false,
                // LessEqual �?water pass 等于 terrain 写下的水面像素时通过
                // �?像素级替�?terrain 的程序化�?                depth_compare: wgpu::CompareFunction::LessEqual,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });

        // Phase 4: WaterPass owns final water only when its material-critical
        // color + LEAN normal inputs are real. Otherwise terrain remains the
        // fallback water owner through MapPassDrawSet::terrain_material_ownership.
        let any_loaded = texture_load_stats.critical_missing == 0;

        Self {
            pipeline,
            bind_groups_g0,
            bind_group_g1,
            bgl_g1,
            env_sampler: env_sampler.clone(),
            bind_group_g2,
            params_buffer,
            chunk_buffers,
            any_loaded,
            selected_effect,
            texture_load_stats,
            load_warnings: warnings,
            binding_audit,
            _owned_textures: owned_textures,
            _owned_samplers: vec![
                water_sampler,
                water_map_sampler,
                env_sampler,
                heightmap_sampler,
            ],
        }
    }

    /// 每帧更新 `WaterParams`（如调试时改 fresnel_power / time_speed）�?    pub fn update_params(&self, queue: &wgpu::Queue, params: &WaterParams) {
    pub fn update_params(&self, queue: &wgpu::Queue, params: &WaterParams) {
        queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(params));
    }

    /// Replace the environment cubemap with the sky pass cubemap.
    pub fn set_env_cubemap(&mut self, device: &wgpu::Device, view: &wgpu::TextureView) {
        self.bind_group_g1 = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("water_bg_g1_sky"),
            layout: &self.bgl_g1,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.env_sampler),
                },
            ],
        });
    }

    /// 在已开 render pass 里画。caller 必须�?set_pipeline 自己的状态前�?    /// 复用 [`crate::passes::TerrainPass`] �?instance buffers�?    pub fn render<'a>(
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

// ─── 私有 helpers ─────────────────────────────────────────────────────────

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

fn water_p5_state_audit_entries(selected_effect: WaterEffectVariant) -> [BindingAuditEntry; 6] {
    [
        BindingAuditEntry::plain_resource(
            "water",
            "effect_selector",
            selected_effect.effect_name(),
            true,
            false,
            Some("R12 selector default: high graphics on, Draw.Refractions false, +0x15 false".into()),
            "water pass variant is selected from graphics/refraction booleans",
        ),
        BindingAuditEntry::plain_resource(
            "water",
            "blend_state_1620",
            "blend enabled, src_alpha / inv_src_alpha, RGB write only",
            true,
            false,
            Some("reverse_out/exports/pdxwater_constants.tsv blend_state 1620".into()),
            "water blends into HDR using shader alpha instead of overwriting RGB directly",
        ),
        BindingAuditEntry::plain_resource(
            "water",
            "depth_state_1348",
            "depth enabled, write off, func LessEqual",
            true,
            false,
            Some("reverse_out/exports/pdxwater_constants.tsv depth_state 1348".into()),
            "water depth test is enabled while preserving the terrain depth buffer",
        ),
        BindingAuditEntry::plain_resource(
            "water",
            "sampler_state",
            "material repeat/linear; map runtime clamp-to-edge/linear; heightmap clamp-to-edge/nearest",
            true,
            false,
            Some("tools/vanilla_trace/state_objects.json; reverse_out/exports/pdxwater_bindings.tsv".into()),
            "water material textures and runtime map targets use distinct address/filter policies",
        ),
        BindingAuditEntry::mock(
            "water",
            "WaterRefraction",
            "water_no_refractions_effect",
            "No dedicated pre-water refraction target is produced yet; selector defaults to water_no_refractions and shader does not substitute map-space underwater terrain as screen refraction.",
            "refraction contribution is explicit fallback and cannot count as full vanilla parity",
            BindingBlockingLevel::Degraded,
        ),
        BindingAuditEntry::mock(
            "water",
            "ReflectionCubeMap",
            "environment_cube_placeholder",
            "No vanilla cubemap producer is mirrored yet; WaterPass binds a fallback or sky cubemap view.",
            "cubemap reflection is explicit fallback and cannot count as full vanilla parity",
            BindingBlockingLevel::Degraded,
        ),
    ]
}

fn load_water_material_textures(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    resources: &VanillaResourceViews,
    warnings: &mut Vec<String>,
    owned: &mut Vec<wgpu::Texture>,
) -> [LoadedWaterTexture; 12] {
    std::array::from_fn(|idx| {
        let spec = WaterMaterialSystem::texture_specs()[idx];
        let uploaded = upload_dds_or_fallback(
            device,
            queue,
            resources,
            DdsUploadRequest {
                role: spec.role,
                label: water_binding_name(spec.role),
                fallback_rgba: spec.fallback_rgba,
                srgb: spec.srgb(),
                critical: spec.critical,
                pass: "water",
                binding: water_binding_name(spec.role),
                visual_impact: water_visual_impact(spec.component),
            },
            warnings,
        );
        let loaded = uploaded.audit.loaded;
        let critical = spec.critical;
        owned.push(uploaded.texture);
        LoadedWaterTexture {
            view: uploaded.view,
            loaded,
            critical,
            audit: uploaded.audit,
        }
    })
}

fn water_texture_load_stats(textures: &[LoadedWaterTexture; 12]) -> WaterTextureLoadStats {
    let mut stats = WaterTextureLoadStats::default();
    for texture in textures {
        if texture.loaded {
            stats.loaded += 1;
        } else {
            stats.fallback += 1;
            if texture.critical {
                stats.critical_missing += 1;
            }
        }
    }
    stats
}

fn water_binding_name(role: MapResRole) -> &'static str {
    match role {
        MapResRole::Lean1 => "water_normal_lean1",
        MapResRole::Lean2 => "water_normal_lean2",
        MapResRole::Reflection => "reflection_tex",
        MapResRole::FowWaterSpec => "fow_water_spec",
        MapResRole::ColormapWater(0) => "colormap_water",
        MapResRole::ColormapWater(1) => "colormap_water_1",
        MapResRole::ColormapWater(2) => "colormap_water_2",
        MapResRole::IceDiffuse => "ice_diffuse",
        MapResRole::IceNoise(0) => "ice_noise",
        MapResRole::IceNoise(1) => "ice_noise_1",
        MapResRole::ReflectionLandUnit => "reflection_land_unit",
        MapResRole::UnderwaterTerrain(0) => "underwater_terrain",
        _ => "water_resource",
    }
}

fn water_visual_impact(component: WaterMaterialComponent) -> &'static str {
    match component {
        WaterMaterialComponent::Color => "water base color falls back to flat color",
        WaterMaterialComponent::Normal => "water normal motion and specular detail flatten",
        WaterMaterialComponent::Specular => "water specular/FOW mask is approximated",
        WaterMaterialComponent::Reflection => "water reflection uses flat fallback",
        WaterMaterialComponent::Ice => "ice coverage and noise are approximated",
    }
}

fn create_1x1_rgba(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    rgba: [u8; 4],
    srgb: bool,
) -> (wgpu::Texture, wgpu::TextureView) {
    let format = if srgb {
        wgpu::TextureFormat::Rgba8UnormSrgb
    } else {
        wgpu::TextureFormat::Rgba8Unorm
    };
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("water_1x1_fallback"),
        size: wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
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

fn create_dim_blue_cubemap(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> (wgpu::Texture, wgpu::TextureView) {
    let texel: [u8; 4] = [80, 110, 150, 255];
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("water_env_cube_fallback"),
        size: wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 6,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    for layer in 0..6 {
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &tex,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: 0,
                    y: 0,
                    z: layer,
                },
                aspect: wgpu::TextureAspect::All,
            },
            &texel,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
    }
    let view = tex.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::Cube),
        ..Default::default()
    });
    (tex, view)
}

// ─── shader �?────────────────────────────────────────────────────────────

/// Phase 6 �?pdxwater.shader 翻译的实例化 / chunk-tessellated 变体�?///
/// �?`crates/hoi4-render/src/translations/pdxwater.wgsl` 同款（fragment �?/// 字面相同）但 vertex 端走 chunk-grid tessellation（与 terrain.wgsl 同款�?/// `(qx, qz) �?world_xz` 路径），world_y 钳到 SEA_LEVEL。FS 增加一�?/// heightmap discard 防止水面画到陆地上�?const WATER_WGSL: &str = r#"
const WATER_WGSL: &str = r#"
//#include "global_uniform.wgsl"
//#include "shader_lib.wgsl"

struct WaterParams {
    time_speed: f32,
    fresnel_power: f32,
    foam_threshold: f32,
    ice_latitude: f32,
    world_w: f32,
    world_d: f32,
    height_scale: f32,
    _pad: f32,
    selected_province_id: u32,
    debug_view: u32,
    final_water_owner: u32,
    effect_variant: u32,
    debug_controls: vec4<f32>,
};

struct ChunkUniform {
    grid: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
};

@group(0) @binding(0) var<uniform> frame: GlobalFrameUniform;
@group(0) @binding(1) var<uniform> wparams: WaterParams;
@group(0) @binding(2) var<uniform> chunk: ChunkUniform;
@group(0) @binding(3) var heightmap_tex: texture_2d<f32>;
@group(0) @binding(4) var heightmap_sampler: sampler;
@group(0) @binding(5) var province_id_tex: texture_2d<u32>;

@group(1) @binding(0) var environment_cube: texture_cube<f32>;
@group(1) @binding(1) var environment_sampler: sampler;

@group(2) @binding(0) var water_normal_lean1: texture_2d<f32>;
@group(2) @binding(1) var water_normal_lean2: texture_2d<f32>;
@group(2) @binding(2) var reflection_tex: texture_2d<f32>;
@group(2) @binding(3) var fow_water_spec: texture_2d<f32>;
@group(2) @binding(4) var colormap_water: texture_2d<f32>;
@group(2) @binding(5) var colormap_water_1: texture_2d<f32>;
@group(2) @binding(6) var colormap_water_2: texture_2d<f32>;
@group(2) @binding(7) var ice_diffuse: texture_2d<f32>;
@group(2) @binding(8) var water_sampler: sampler;
@group(2) @binding(9) var ice_noise: texture_2d<f32>;
@group(2) @binding(10) var ice_noise_1: texture_2d<f32>;
@group(2) @binding(11) var reflection_land_unit: texture_2d<f32>;
@group(2) @binding(12) var underwater_terrain: texture_2d<f32>;
@group(2) @binding(13) var shadow_map: texture_2d<f32>;
@group(2) @binding(14) var gradient_border_ch1: texture_2d<f32>;
@group(2) @binding(15) var gradient_border_ch2: texture_2d<f32>;
@group(2) @binding(16) var gradient_border_ch3: texture_2d<f32>;
@group(2) @binding(17) var province_secondary_color: texture_2d<f32>;
@group(2) @binding(18) var fow_tex: texture_2d<f32>;
@group(2) @binding(19) var light_data_tex: texture_2d<f32>;
@group(2) @binding(20) var light_index_tex: texture_2d<f32>;
@group(2) @binding(21) var mud_snow_tex: texture_2d<f32>;
@group(2) @binding(22) var water_map_sampler: sampler;

const SEA_LEVEL: f32 = 95.0 / 255.0;
const ID_NONE: u32 = 4294967295u;
const WATER_DEBUG_OFF: u32 = 0u;
const WATER_DEBUG_DEPTH_RATIO: u32 = 1u;
const WATER_DEBUG_COAST_DISTANCE: u32 = 2u;
const WATER_DEBUG_NORMAL_STRENGTH: u32 = 3u;
const WATER_DEBUG_FOAM_MASK: u32 = 4u;
const WATER_DEBUG_ICE_MASK: u32 = 5u;
const WATER_DEBUG_REFLECTION_CONTRIBUTION: u32 = 6u;
const WATER_DEBUG_FINAL_WATER_ONLY: u32 = 7u;

struct VertexInput {
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

fn load_height_uv(uv: vec2<f32>) -> f32 {
    let dim = vec2<f32>(textureDimensions(heightmap_tex));
    let xy = vec2<i32>(clamp(uv, vec2<f32>(0.0), vec2<f32>(1.0)) * (dim - vec2<f32>(1.0)));
    return textureLoad(heightmap_tex, xy, 0).r;
}

fn load_height_bilinear(uv: vec2<f32>) -> f32 {
    let dim = vec2<f32>(textureDimensions(heightmap_tex));
    let coord_f = clamp(uv, vec2<f32>(0.0), vec2<f32>(1.0)) * (dim - vec2<f32>(1.0));
    let coord_i = vec2<i32>(floor(coord_f));
    let frac_xy = fract(coord_f);
    let max_x = i32(dim.x) - 1;
    let max_y = i32(dim.y) - 1;
    let x0 = clamp(coord_i.x, 0, max_x);
    let y0 = clamp(coord_i.y, 0, max_y);
    let x1 = clamp(coord_i.x + 1, 0, max_x);
    let y1 = clamp(coord_i.y + 1, 0, max_y);
    let h00 = textureLoad(heightmap_tex, vec2<i32>(x0, y0), 0).r;
    let h10 = textureLoad(heightmap_tex, vec2<i32>(x1, y0), 0).r;
    let h01 = textureLoad(heightmap_tex, vec2<i32>(x0, y1), 0).r;
    let h11 = textureLoad(heightmap_tex, vec2<i32>(x1, y1), 0).r;
    let h0 = mix(h00, h10, frac_xy.x);
    let h1 = mix(h01, h11, frac_xy.x);
    return mix(h0, h1, frac_xy.y);
}

fn province_at(uv: vec2<f32>) -> u32 {
    let tex_size = vec2<f32>(textureDimensions(province_id_tex));
    let coord = vec2<i32>(uv * tex_size);
    let cx = clamp(coord.x, 0, i32(tex_size.x) - 1);
    let cy = clamp(coord.y, 0, i32(tex_size.y) - 1);
    return textureLoad(province_id_tex, vec2<i32>(cx, cy), 0).r;
}

@vertex
fn vs_main(in: VertexInput) -> VsOut {
    let grid = chunk.grid;
    let cell_idx = in.vid / 6u;
    let v_in_q = in.vid % 6u;
    let qx_u = cell_idx % grid;
    let qz_u = cell_idx / grid;

    // �?terrain.wgsl 同款 winding�?=(0,0) 1=(1,0) 2=(1,1) 3=(1,0) 4=(1,1) 5=(0,1)
    var ox: u32 = 0u;
    var oz: u32 = 0u;
    if (v_in_q == 1u || v_in_q == 2u || v_in_q == 4u) { ox = 1u; }
    if (v_in_q == 2u || v_in_q == 4u || v_in_q == 5u) { oz = 1u; }

    let cell_size = in.size_xz / f32(grid);
    let local_xz = vec2<f32>(f32(qx_u + ox), f32(qz_u + oz)) * cell_size;
    let world_xz = in.origin_xz + local_xz;
    let world_y = SEA_LEVEL * wparams.height_scale;
    let world_size = vec2<f32>(wparams.world_w, wparams.world_d);

    // 海面是平的：world_y = SEA_LEVEL × height_scale�?    let world_y = SEA_LEVEL * wparams.height_scale;

    // 全图 uv：X 轴环绕，Z 轴仍限制在南北边界内�?    let world_size = vec2<f32>(wparams.world_w, wparams.world_d);
    let map_uv = world_xz_to_map_uv(world_xz, world_size);
    let map_px = map_uv_to_px(map_uv);

    let world_pos = vec3<f32>(world_xz.x, world_y, world_xz.y);

    var out: VsOut;
    out.clip_pos = frame.view_proj * vec4<f32>(world_pos, 1.0);
    out.world_pos = world_pos;
    out.map_uv = map_uv;
    out.map_px = map_px;
    return out;
}

// LEAN moments blending used by pdxwater's multi-tap water normals.
fn blend_lean(a: vec4<f32>, b: vec4<f32>) -> vec4<f32> {
    let mean = (a.rg + b.rg) * 0.5;
    let variance = max((a.ba + b.ba) * 0.5, vec2<f32>(0.0));
    return vec4<f32>(mean, variance);
}

fn unpack_lean_normal(sample: vec4<f32>) -> vec3<f32> {
    let mean = sample.rg * 2.0 - 1.0;
    let variance = dot(sample.ba, vec2<f32>(0.5));
    let strength = clamp(0.72 - variance * 0.20, 0.35, 0.72);
    let nx = mean.x * strength;
    let nz = mean.y * strength;
    let ny = sqrt(max(1.0 - nx * nx - nz * nz, 0.0));
    return normalize(vec3<f32>(nx, ny, nz));
}

/// vanilla pdxwater 同款 4-tap LEAN 法线�? 个频�?+ 4 个滚动方向�?/// LEAN 纹理 RG = mean(N.xy)，BA = 方差。取 RG 解码为法线�?fn sample_water_normal_lean(map_px: vec2<f32>, time: f32) -> vec3<f32> {
fn sample_water_normal_lean(map_px: vec2<f32>, time: f32) -> vec3<f32> {
    let water_px = map_px / 128.0;
    let uv0 = water_px * 0.04 + vec2<f32>(time * 0.012, time * 0.008);
    let uv1 = water_px * 0.08 + vec2<f32>(-time * 0.011, time * 0.013);
    let uv2 = water_px * 0.16 + vec2<f32>(time * 0.007, -time * 0.009);
    let uv3 = water_px * 0.32 + vec2<f32>(-time * 0.005, -time * 0.011);

    let lean_a = blend_lean(
        textureSample(water_normal_lean1, water_sampler, uv0),
        textureSample(water_normal_lean1, water_sampler, uv1)
    );
    let lean_b = blend_lean(
        textureSample(water_normal_lean2, water_sampler, uv2),
        textureSample(water_normal_lean2, water_sampler, uv3)
    );
    return unpack_lean_normal(blend_lean(lean_a, lean_b));
}

// Disable screen-visible polar edge colouring; the map projection already
// reaches the window edge, so extra polar tint reads as a horizontal band.
fn polar_edge_mask(map_uv: vec2<f32>) -> f32 {
    return 0.0;
}

fn sample_water(map_uv: vec2<f32>, depth_ratio: f32, camera_dist: f32) -> vec3<f32> {
    let near_color = textureSample(colormap_water, water_sampler, map_uv).rgb;
    let mid_color = textureSample(colormap_water_1, water_sampler, map_uv).rgb;
    let far_color = textureSample(colormap_water_2, water_sampler, map_uv).rgb;
    let lod_t = smoothstep(32.0, 180.0, camera_dist);
    let map_color = mix(mix(near_color, mid_color, lod_t), far_color, lod_t * lod_t);
    let underwater = textureSample(underwater_terrain, water_sampler, map_uv * 4.0).rgb;
    return mix(map_color, underwater, clamp(depth_ratio * 0.22, 0.0, 0.22));
}

fn sample_refraction(map_uv: vec2<f32>, normal: vec3<f32>, depth_ratio: f32) -> vec3<f32> {
    if (wparams.effect_variant == 1u) {
        return textureSample(colormap_water_1, water_sampler, map_uv).rgb;
    }
    let offset = normal.xz * (0.0018 + 0.0026 * (1.0 - depth_ratio));
    let refracted_uv = map_uv + offset;
    let water_lod = textureSample(colormap_water_1, water_sampler, refracted_uv).rgb;
    let under = textureSample(underwater_terrain, water_sampler, refracted_uv * 4.0).rgb;
    return mix(water_lod, under, 0.35 * depth_ratio);
}

fn calculate_point_lights_water(map_px: vec2<f32>, world_pos: vec3<f32>, normal: vec3<f32>) -> vec3<f32> {
    let globe_n = calc_globe_normal(map_px, frame.day_night_hour_sun_dir.x);
    let night = day_night_factor(globe_n, frame.day_night_hour_sun_dir.yzw, 1.0);
    return calculate_point_lights(
        light_data_tex,
        light_index_tex,
        map_px,
        world_pos,
        normal,
        (0.15 + night * 0.85) * 0.40
    );
}

struct IceResult {
    color: vec3<f32>,
    mask: f32,
};

fn apply_ice(map_uv: vec2<f32>, base_color: vec3<f32>) -> IceResult {
    let lat_t = abs(map_uv.y - 0.5) * 2.0;
    if (lat_t <= wparams.ice_latitude) {
        return IceResult(base_color, 0.0);
    }
    let ice = textureSample(ice_diffuse, water_sampler, map_uv * 4.0).rgb;
    let noise0 = textureSample(ice_noise, water_sampler, map_uv * 8.0).r;
    let noise1 = textureSample(ice_noise_1, water_sampler, map_uv * 16.0 + vec2<f32>(0.37, 0.19)).r;
    let ice_mask = smoothstep(wparams.ice_latitude, wparams.ice_latitude + 0.06, lat_t)
                 * (0.35 + 0.45 * noise0 + 0.20 * noise1);
    let mask = clamp(ice_mask, 0.0, 1.0);
    return IceResult(mix(base_color, ice, mask), mask);
}

fn apply_water_mud_snow(map_uv: vec2<f32>, base_color: vec3<f32>, depth_ratio: f32) -> vec3<f32> {
    let mud_snow = textureSample(mud_snow_tex, water_map_sampler, map_uv);
    let season = clamp(frame.fow_opacity_time_snow_max_speed.z, 0.0, 1.0);
    let snow = get_snow(mud_snow, season);
    let mud = mix(mud_snow.r, mud_snow.a, season);
    let snow_tint = vec3<f32>(0.58, 0.68, 0.78);
    let mud_tint = vec3<f32>(0.06, 0.10, 0.13);
    var color = mix(base_color, mud_tint, mud * 0.12 * (1.0 - depth_ratio));
    color = mix(color, snow_tint, snow * 0.20);
    return color;
}

struct WaterMaterial {
    final_color: vec3<f32>,
    depth_ratio: f32,
    coast_distance_px: f32,
    normal_strength: f32,
    foam_mask: f32,
    ice_mask: f32,
    reflection_contribution: f32,
    alpha: f32,
    selected: bool,
};

fn water_debug_color(material: WaterMaterial) -> vec3<f32> {
    if (wparams.debug_view == WATER_DEBUG_DEPTH_RATIO) {
        let t = clamp(material.depth_ratio * wparams.debug_controls.x, 0.0, 1.0);
        return vec3<f32>(t, t, t);
    }
    if (wparams.debug_view == WATER_DEBUG_COAST_DISTANCE) {
        let max_px = max(wparams.debug_controls.y, 1.0);
        let t = clamp(material.coast_distance_px / max_px, 0.0, 1.0);
        return vec3<f32>(1.0 - t, 0.25 + 0.45 * (1.0 - t), t);
    }
    if (wparams.debug_view == WATER_DEBUG_NORMAL_STRENGTH) {
        let t = clamp(material.normal_strength, 0.0, 1.0);
        return vec3<f32>(0.08, t, 1.0 - t);
    }
    if (wparams.debug_view == WATER_DEBUG_FOAM_MASK) {
        return vec3<f32>(material.foam_mask);
    }
    if (wparams.debug_view == WATER_DEBUG_ICE_MASK) {
        return vec3<f32>(0.20 + material.ice_mask * 0.80, 0.35 + material.ice_mask * 0.60, 1.0);
    }
    if (wparams.debug_view == WATER_DEBUG_REFLECTION_CONTRIBUTION) {
        let t = clamp(material.reflection_contribution, 0.0, 1.0);
        return vec3<f32>(t, t * 0.75, 1.0 - t);
    }
    if (wparams.debug_view == WATER_DEBUG_FINAL_WATER_ONLY) {
        return material.final_color;
    }
    return material.final_color;
}

fn build_water_material(map_uv: vec2<f32>, map_px: vec2<f32>, screen_uv: vec2<f32>, world_pos: vec3<f32>, h: f32) -> WaterMaterial {
    let depth_ratio = clamp((SEA_LEVEL - h) / SEA_LEVEL, 0.0, 1.0);
    let polar_edge = polar_edge_mask(map_uv);
    let time = frame.global_time * wparams.time_speed * 25.0;
    let camera_dist = length(frame.cam_pos - world_pos);
    let normal = normalize(mix(
        sample_water_normal_lean(map_px, time),
        vec3<f32>(0.0, 1.0, 0.0),
        polar_edge * 0.88
    ));
    let normal_strength = clamp(length(normal.xz) / 0.6, 0.0, 1.0);

    // Base water color comes from vanilla water color LODs plus underwater terrain.
    let base = sample_water(map_uv, depth_ratio, camera_dist);
    let secondary = textureSample(province_secondary_color, water_map_sampler, map_uv);

    let to_camera = normalize(frame.cam_pos - world_pos);
    let reflect_dir = reflect(-to_camera, normal);
    let env_raw = textureSample(environment_cube, environment_sampler, reflect_dir).rgb * frame.cubemap_intensity;
    let plane_refl = textureSample(reflection_tex, water_sampler, map_uv).rgb;
    let land_unit_refl = textureSample(reflection_land_unit, water_sampler, map_uv).rgb;
    let refraction = sample_refraction(map_uv, normal, depth_ratio);
    let env = mix(plane_refl, env_raw, 0.35);
    let reflected = mix(env, land_unit_refl, secondary.a * 0.15);

    let fresnel_t = pow(1.0 - max(dot(normal, to_camera), 0.0), wparams.fresnel_power);
    let reflection_contribution = clamp(0.08 + fresnel_t * 0.52, 0.0, 0.70);
    var color = mix(mix(refraction, base, 0.68), reflected, reflection_contribution);

    let sun_dir = normalize(frame.day_night_hour_sun_dir.yzw);
    let half_dir = normalize(to_camera + sun_dir);
    let n_dot_h = max(dot(normal, half_dir), 0.0);
    let spec_mask = textureSample(fow_water_spec, water_sampler, map_uv).a;
    let sun_spec = pow(n_dot_h, 192.0) * spec_mask * max(frame.sun_specular_intensity, 0.4);
    color = color + vec3<f32>(1.0, 0.97, 0.85) * sun_spec * 0.20 * (1.0 - polar_edge * 0.90);

    let projected_shadow = textureSample(shadow_map, water_map_sampler, clamp(screen_uv, vec2<f32>(0.0), vec2<f32>(1.0)));
    let coast_d_px = 255.0;
    let country_d_px = textureSample(gradient_border_ch1, water_map_sampler, map_uv).r * 255.0;
    let province_d_px = textureSample(gradient_border_ch2, water_map_sampler, map_uv).r * 255.0;
    let semantic_d_px = textureSample(gradient_border_ch3, water_map_sampler, map_uv).r * 255.0;
    let border_hint = 1.0 - smoothstep(0.0, 3.0, min(min(country_d_px, province_d_px), semantic_d_px));
    color = mix(color, vec3<f32>(0.09, 0.16, 0.21), border_hint * 0.045);
    color = mix(color, secondary.rgb, secondary.a * 0.30);
    let foam_alpha = 0.0;
    let water_shore = 1.0 - smoothstep(SEA_LEVEL - 0.035, SEA_LEVEL - 0.004, h);
    let water_alpha = clamp(1.0 - water_shore, 0.0, 1.0);
    color = mix(color, color * projected_shadow.r, 1.0 - projected_shadow.r);

    color = color + calculate_point_lights_water(map_px, world_pos, normal) * 0.12;
    color = apply_water_mud_snow(map_uv, color, depth_ratio);
    let ice_result = apply_ice(map_uv, color);
    color = ice_result.color;

    let polar_neutral = vec3<f32>(0.075, 0.18, 0.27);
    color = mix(color, polar_neutral, polar_edge * 0.92);

    let pid = province_at(map_uv);
    let selected = wparams.selected_province_id != ID_NONE && pid == wparams.selected_province_id;
    if (selected) {
        let pulse = 0.5 + 0.5 * sin(frame.global_time * 5.0);
        color = mix(color, vec3<f32>(1.0, 0.78, 0.18), 0.50 + 0.18 * pulse);
    }

    let globe_n = calc_globe_normal(map_px, frame.day_night_hour_sun_dir.x);
    color = day_night(color, globe_n, frame.day_night_hour_sun_dir.yzw, 1.0);
    color = apply_wrapped_distance_fog(color, world_pos, frame.cam_pos, wparams.world_w);
    let fow_visibility = min(textureSample(fow_tex, water_map_sampler, map_uv).g, max(projected_shadow.b, projected_shadow.g));
    color = mix(color * 0.52, color, fow_visibility);

    return WaterMaterial(
        color,
        depth_ratio,
        coast_d_px,
        normal_strength,
        foam_alpha,
        ice_result.mask,
        reflection_contribution,
        water_alpha,
        selected
    );
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let map_uv = in.map_uv;

    // ── 1. heightmap 深度感知 + 陆地像素 discard ───────────────────────
    if (wparams.final_water_owner == 0u) {
        discard;
    }
    let h = load_height_bilinear(map_uv);
    if (h > SEA_LEVEL + 0.002) {
        discard;
    }
    let projected_shadow_uv = in.clip_pos.xy / max(frame.screen_size, vec2<f32>(1.0));
    let material = build_water_material(map_uv, in.map_px, projected_shadow_uv, in.world_pos, h);
    let color = water_debug_color(material);

    return vec4<f32>(color, material.alpha);
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn water_params_size_is_64() {
        assert_eq!(std::mem::size_of::<WaterParams>(), 64);
    }

    #[test]
    fn water_params_default_is_sane() {
        let p = WaterParams::default();
        assert!(p.time_speed > 0.0 && p.time_speed < 1.0);
        assert!(p.fresnel_power > 1.0 && p.fresnel_power < 10.0);
        assert!(p.foam_threshold > 0.0 && p.foam_threshold < 8.0);
        assert!(p.ice_latitude > 0.5 && p.ice_latitude <= 1.05);
        assert!(p.world_w > 0.0);
        assert!(p.world_d > 0.0);
        assert!(p.height_scale > 0.0);
        assert_eq!(p.debug_view, WaterDebugView::Off.as_shader_value());
        assert_eq!(p.final_water_owner, 1);
        assert!(p.debug_controls[1] > 0.0);
    }

    #[test]
    fn water_debug_view_cycles_through_all_modes() {
        let mut seen = Vec::new();
        let mut view = WaterDebugView::Off;
        for _ in 0..WaterDebugView::ALL.len() {
            seen.push(view);
            view = view.next();
        }
        assert_eq!(seen, WaterDebugView::ALL);
        assert_eq!(view, WaterDebugView::Off);
        assert_eq!(WaterDebugView::FoamMask.name(), "foam_mask");
    }

    #[test]
    fn water_material_system_declares_phase4_components() {
        let specs = WaterMaterialSystem::texture_specs();
        assert_eq!(specs.len(), 12);
        assert!(specs
            .iter()
            .any(|spec| spec.component == WaterMaterialComponent::Color));
        assert!(specs
            .iter()
            .any(|spec| spec.component == WaterMaterialComponent::Normal));
        assert!(specs
            .iter()
            .any(|spec| spec.component == WaterMaterialComponent::Specular));
        assert!(specs
            .iter()
            .any(|spec| spec.component == WaterMaterialComponent::Reflection));
        assert!(specs
            .iter()
            .any(|spec| spec.component == WaterMaterialComponent::Ice));
        assert!(specs
            .iter()
            .filter(|spec| spec.critical)
            .all(|spec| matches!(
                spec.role,
                MapResRole::Lean1 | MapResRole::Lean2 | MapResRole::ColormapWater(0)
            )));
        assert!(specs
            .iter()
            .any(|spec| spec.role == MapResRole::ColormapWater(1)));
        assert!(specs
            .iter()
            .any(|spec| spec.role == MapResRole::ColormapWater(2)));
        assert!(specs
            .iter()
            .any(|spec| spec.role == MapResRole::ReflectionLandUnit));
        assert!(specs
            .iter()
            .any(|spec| spec.role == MapResRole::UnderwaterTerrain(0)));
    }

    #[test]
    fn water_chunk_uniform_size_is_16() {
        assert_eq!(std::mem::size_of::<WaterChunkUniform>(), 16);
    }

    #[test]
    fn water_effect_selector_p5_rules_are_registered() {
        assert_eq!(
            WaterEffectSelector {
                high_graphics_byte_0x18: false,
                draw_refractions: true,
                refraction_byte_0x15: true,
            }
            .select(),
            WaterEffectVariant::WaterLowGfx
        );
        assert_eq!(
            WaterEffectSelector {
                high_graphics_byte_0x18: true,
                draw_refractions: false,
                refraction_byte_0x15: true,
            }
            .select(),
            WaterEffectVariant::WaterNoRefractions
        );
        assert_eq!(
            WaterEffectSelector {
                high_graphics_byte_0x18: true,
                draw_refractions: true,
                refraction_byte_0x15: true,
            }
            .select(),
            WaterEffectVariant::Water
        );
    }

    #[test]
    fn water_p5_state_audit_reports_phase_d_state() {
        let entries = water_p5_state_audit_entries(WaterEffectSelector::default().select());
        assert_eq!(entries.len(), 6);

        let entry = |binding: &str| {
            entries
                .iter()
                .find(|entry| entry.binding == binding)
                .unwrap_or_else(|| panic!("missing water audit binding `{binding}`"))
        };

        assert_eq!(
            entry("effect_selector").source_name,
            "pdxwater:water_no_refractions"
        );
        assert_eq!(
            entry("blend_state_1620").source_name,
            "blend enabled, src_alpha / inv_src_alpha, RGB write only"
        );
        assert_eq!(
            entry("depth_state_1348").source_name,
            "depth enabled, write off, func LessEqual"
        );
        assert!(entry("sampler_state")
            .reason
            .as_deref()
            .unwrap_or_default()
            .contains("state_objects.json"));

        let refraction = entry("WaterRefraction");
        assert_eq!(
            refraction.mock_name.as_deref(),
            Some("water_no_refractions_effect")
        );
        assert_eq!(refraction.blocking_level, BindingBlockingLevel::Degraded);
        assert!(refraction
            .reason
            .as_deref()
            .unwrap_or_default()
            .contains("does not substitute map-space underwater terrain"));
    }

    #[test]
    fn water_pipeline_source_uses_traced_phase_d_om_state() {
        let source = include_str!("water.rs");
        assert!(source.contains("BlendFactor::SrcAlpha"));
        assert!(source.contains("BlendFactor::OneMinusSrcAlpha"));
        assert!(source.contains("BlendComponent::REPLACE"));
        assert!(source.contains("ColorWrites::RED"));
        assert!(source.contains("ColorWrites::GREEN"));
        assert!(source.contains("ColorWrites::BLUE"));
        assert!(source.contains("depth_write_enabled: false"));
        assert!(source.contains("CompareFunction::LessEqual"));
    }

    #[test]
    fn water_wgsl_uses_effect_variant_alpha_and_no_refraction_fallback() {
        assert!(WATER_WGSL.contains("effect_variant: u32"));
        assert!(WATER_WGSL.contains("wparams.effect_variant == 1u"));
        assert!(WATER_WGSL.contains("return textureSample(colormap_water_1"));
        assert!(WATER_WGSL.contains("alpha: f32"));
        assert!(WATER_WGSL.contains("return vec4<f32>(color, material.alpha)"));
        assert!(!WATER_WGSL.contains("return vec4<f32>(color, 1.0)"));
    }

    #[test]
    fn water_projected_shadow_uses_screen_coordinate() {
        assert!(WATER_WGSL.contains("projected_shadow_uv = in.clip_pos.xy / max(frame.screen_size"));
        assert!(WATER_WGSL.contains("textureSample(shadow_map, water_map_sampler, clamp(screen_uv"));
        assert!(!WATER_WGSL.contains("textureSample(shadow_map, water_map_sampler, map_uv)"));
    }

    #[test]
    fn water_wgsl_applies_gradient_border_before_province_secondary_color() {
        let gradient_idx = WATER_WGSL
            .find("let country_d_px = textureSample(gradient_border_ch1")
            .expect("water shader should sample gradient border");
        let secondary_mix_idx = WATER_WGSL
            .find("color = mix(color, secondary.rgb")
            .expect("water shader should mix province secondary color");
        assert!(
            gradient_idx < secondary_mix_idx,
            "GradientBorder must be consumed before ProvinceSecondaryColorMap"
        );
    }

    #[test]
    fn water_wgsl_naga_parses() {
        let composed = hoi4_render::shader_rt::compose_shader(WATER_WGSL, true, true);
        let module = match naga::front::wgsl::parse_str(&composed) {
            Ok(module) => module,
            Err(e) => {
                eprintln!("=== composed water wgsl (first 600 chars) ===");
                eprintln!("{}", &composed[..composed.len().min(600)]);
                eprintln!("=== error ===");
                eprintln!("{}", e);
                panic!("WATER_WGSL should parse with naga");
            }
        };
        let mut validator = naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        );
        if let Err(e) = validator.validate(&module) {
            eprintln!("=== composed water wgsl (first 600 chars) ===");
            eprintln!("{}", &composed[..composed.len().min(600)]);
            eprintln!("=== validation error ===");
            eprintln!("{}", e.emit_to_string(&composed));
            panic!("WATER_WGSL should validate with naga");
        }
    }

    /// 验证 shader 引用了所�?9 �?group 2 binding（lean1/2 + reflection +
    /// fow_water_spec + colormap_water + ice_diffuse + ice_noise + ShadowMap
    /// + sampler）�?防止后续重构时漏接绑定�?    #[test]
    fn water_wgsl_references_all_water_bindings() {
        let names = [
            "water_normal_lean1",
            "water_normal_lean2",
            "reflection_tex",
            "fow_water_spec",
            "colormap_water",
            "colormap_water_1",
            "colormap_water_2",
            "ice_diffuse",
            "ice_noise",
            "ice_noise_1",
            "reflection_land_unit",
            "underwater_terrain",
            "shadow_map",
            "water_sampler",
            "gradient_border_ch1",
            "gradient_border_ch2",
            "gradient_border_ch3",
            "province_secondary_color",
            "fow_tex",
            "light_data_tex",
            "light_index_tex",
        ];
        for n in names {
            assert!(
                WATER_WGSL.contains(n),
                "WATER_WGSL should reference binding `{}`",
                n
            );
        }
    }

    #[test]
    fn water_wgsl_no_longer_uses_coast_sdf_in_p5_path() {
        assert!(
            !WATER_WGSL.contains("coast_sdf"),
            "P5 water parity path must not bind coast_sdf as a vanilla input"
        );
        assert!(WATER_WGSL.contains("shadow_map"));
    }

    #[test]
    fn water_wgsl_has_fragment_discard_and_fresnel() {
        // 防止后续重构丢掉关键 vanilla 行为
        assert!(
            WATER_WGSL.contains("discard"),
            "needs heightmap-mask discard"
        );
        assert!(WATER_WGSL.contains("fresnel_t"), "needs Fresnel mixing");
        assert!(
            WATER_WGSL.contains("sample_water_normal_lean"),
            "needs LEAN 4-tap normal blend"
        );
        assert!(
            WATER_WGSL.contains("fn sample_water"),
            "needs SampleWater path"
        );
        assert!(
            WATER_WGSL.contains("sample_refraction"),
            "needs refraction path"
        );
        assert!(WATER_WGSL.contains("apply_ice"), "needs ApplyIce path");
        assert!(WATER_WGSL.contains("ice_diffuse"), "needs polar ice path");
        assert!(
            WATER_WGSL.contains("apply_wrapped_distance_fog"),
            "needs wrap-aware fog"
        );
        assert!(WATER_WGSL.contains("day_night"), "needs day/night dim");
        assert!(
            WATER_WGSL.contains("calculate_point_lights_water"),
            "needs point-light binding path"
        );
        assert!(
            WATER_WGSL.contains("calculate_point_lights("),
            "needs shared point-light helper"
        );
        assert!(
            WATER_WGSL.contains("apply_water_mud_snow"),
            "needs SnowMudTexture/MudSnow water path"
        );
        assert!(
            WATER_WGSL.contains("textureSample(mud_snow_tex"),
            "bound mud_snow_tex must be consumed by the water material"
        );
    }

    #[test]
    fn water_wgsl_has_phase4_material_debug_and_owner_gate() {
        for token in [
            "struct WaterMaterial",
            "fn build_water_material",
            "fn water_debug_color",
            "debug_view: u32",
            "final_water_owner: u32",
            "WATER_DEBUG_DEPTH_RATIO",
            "WATER_DEBUG_COAST_DISTANCE",
            "WATER_DEBUG_NORMAL_STRENGTH",
            "WATER_DEBUG_FOAM_MASK",
            "WATER_DEBUG_ICE_MASK",
            "WATER_DEBUG_REFLECTION_CONTRIBUTION",
            "WATER_DEBUG_FINAL_WATER_ONLY",
        ] {
            assert!(WATER_WGSL.contains(token), "missing Phase 4 token: {token}");
        }
    }
}
