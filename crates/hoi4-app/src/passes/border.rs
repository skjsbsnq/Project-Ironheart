//! Phase 3.12.9 (redesign) — `BorderPass`: vanilla-equivalent strip-mesh borders.
//!
//! **Previous implementation was a design error**: it treated vanilla's small
//! strip textures (256×128) as full-map overlays rendered via the chunk grid,
//! causing the entire screen to be tinted pink at far zoom due to mip averaging.
//!
//! **New design** (matches vanilla `border.shader`):
//! - CPU extracts border edges from the province bitmap → generates thin
//!   quad-strip meshes that hug actual boundaries (see `hoi4_render::border_extract`).
//! - GPU renders these strips with per-type `BorderDiffuse` textures (small
//!   256×128 DDS, UV.x across strip, UV.y tiling along border direction).
//! - Alpha-blended on top of terrain + water, depth LessEqual + no write.
//! - Per-type LOD: `cam_distance_norm` selects between lod0/1/2 textures.
//! - Phase 4 keeps this strip mesh as the explicit border/debug fallback while
//!   terrain, water, and trees consume shared GradientBorderChannel targets.
//!
//! ## Rendering order
//!
//! TerrainPass → RiverPass → WaterPass → **BorderPass** → MapSymbolPass

use hoi4_assets::{dds_upload_plan, AssetDb, DdsImage, FsAssetDb, MapResRole};
use hoi4_paths::PathConfig;
use hoi4_render::border_extract::{BorderKind, BorderMesh, BorderVertex};
use hoi4_render::defines::{MAP_SIZE_X, MAP_SIZE_Y};
use wgpu::util::DeviceExt;

use crate::passes::HDR_FORMAT;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum BorderDebugView {
    Off = 0,
    CountryOnly = 1,
    StateOnly = 2,
    ProvinceOnly = 3,
    SeaOnly = 4,
    SelectedOnly = 5,
    FalseColorHierarchy = 6,
}

impl BorderDebugView {
    pub const ALL: [Self; 7] = [
        Self::Off,
        Self::CountryOnly,
        Self::StateOnly,
        Self::ProvinceOnly,
        Self::SeaOnly,
        Self::SelectedOnly,
        Self::FalseColorHierarchy,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::CountryOnly => "country_only",
            Self::StateOnly => "state_only",
            Self::ProvinceOnly => "province_only",
            Self::SeaOnly => "sea_only",
            Self::SelectedOnly => "selected_only",
            Self::FalseColorHierarchy => "false_color_hierarchy",
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

// ─── BorderParams uniform ──────────────────────────────────────────────────

/// 48 bytes - matches WGSL `BorderParams`.
#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct BorderParams {
    pub cam_distance_norm: f32,
    pub selection_intensity: f32,
    pub enabled_mask: u32,
    pub selected_province_id: u32,
    pub debug_view: u32,
    pub screen_width: f32,
    pub screen_height: f32,
    pub camera_distance_world: f32,
    pub world_size: [f32; 2],
    pub map_size_px: [f32; 2],
}

impl Default for BorderParams {
    fn default() -> Self {
        Self {
            cam_distance_norm: 0.3,
            selection_intensity: 0.0,
            enabled_mask: Self::DEFAULT_VISIBLE_MASK,
            selected_province_id: u32::MAX,
            debug_view: BorderDebugView::Off.as_shader_value(),
            screen_width: 1920.0,
            screen_height: 1080.0,
            camera_distance_world: 96.0,
            world_size: [112.0, 41.0],
            map_size_px: [MAP_SIZE_X, MAP_SIZE_Y],
        }
    }
}

impl BorderParams {
    pub const COUNTRY_MASK: u32 = 1 << 0;
    pub const STATE_MASK: u32 = 1 << 1;
    pub const PROVINCE_MASK: u32 = 1 << 2;
    pub const SEA_MASK: u32 = 1 << 3;
    pub const SEA_REGION_MASK: u32 = 1 << 4;
    pub const IMPASSABLE_MASK: u32 = 1 << 5;
    pub const ALL_VISIBLE_MASK: u32 = Self::COUNTRY_MASK
        | Self::STATE_MASK
        | Self::PROVINCE_MASK
        | Self::SEA_MASK
        | Self::SEA_REGION_MASK
        | Self::IMPASSABLE_MASK;
    pub const DEFAULT_VISIBLE_MASK: u32 = 0;
}

const _: () = assert!(std::mem::size_of::<BorderParams>() == 48);

// ─── Public inputs ─────────────────────────────────────────────────────────

/// Construction inputs for BorderPass.
pub struct BorderPassInputs<'a> {
    pub global_uniform_buffer: &'a wgpu::Buffer,
    pub depth_format: wgpu::TextureFormat,
    /// Pre-generated strip meshes from `border_extract::generate_border_meshes`.
    pub meshes: &'a [BorderMesh],
    pub path_cfg: &'a PathConfig,
}

// ─── Per-kind GPU data ─────────────────────────────────────────────────────

struct KindBuffers {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
    /// Bind group holding the 3 LOD textures + sampler for this border kind.
    texture_bind_group: wgpu::BindGroup,
}

// ─── BorderPass ────────────────────────────────────────────────────────────

pub struct BorderPass {
    pipeline: wgpu::RenderPipeline,
    params_buffer: wgpu::Buffer,
    params_bind_group: wgpu::BindGroup,
    kinds: Vec<(BorderKind, KindBuffers)>,
    pub any_loaded: bool,
    pub load_warnings: Vec<String>,
    _owned_textures: Vec<wgpu::Texture>,
}

impl BorderPass {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, inputs: BorderPassInputs<'_>) -> Self {
        let mut warnings = Vec::new();
        let mut owned_textures: Vec<wgpu::Texture> = Vec::new();

        // ── Shader ────────────────────────────────────────────────────
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("border_strip_shader"),
            source: wgpu::ShaderSource::Wgsl(BORDER_STRIP_WGSL.into()),
        });

        // ── Params uniform ────────────────────────────────────────────
        let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("border_params"),
            contents: bytemuck::bytes_of(&BorderParams::default()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // ── Bind group layouts ────────────────────────────────────────
        // Group 0: GlobalFrameUniform + BorderParams
        let bgl_g0 = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("border_bgl_g0"),
            entries: &[
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
            ],
        });

        let params_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("border_bg_g0"),
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
            ],
        });

        // Group 1: per-kind texture (3 LOD) + sampler
        let bgl_g1 = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("border_bgl_g1"),
            entries: &[
                tex_entry(0),
                tex_entry(1),
                tex_entry(2),
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        // ── Sampler (locked to mip 0, Wrap U / Clamp V like vanilla) ──
        let border_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("border_strip_sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            lod_min_clamp: 0.0,
            lod_max_clamp: 0.0,
            ..Default::default()
        });

        // ── Load textures + build per-kind buffers ────────────────────
        let db = FsAssetDb::new(inputs.path_cfg.clone());
        let mut kinds_vec: Vec<(BorderKind, KindBuffers)> = Vec::new();

        for mesh in inputs.meshes {
            if mesh.vertices.is_empty() || mesh.indices.is_empty() {
                continue;
            }
            let roles = lod_roles(mesh.kind);
            let mut views: Vec<wgpu::TextureView> = Vec::with_capacity(3);
            for role in &roles {
                let view = load_border_tex(
                    device,
                    queue,
                    &db,
                    *role,
                    &mut warnings,
                    &mut owned_textures,
                );
                views.push(view);
            }

            let texture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("border_kind_bg"),
                layout: &bgl_g1,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&views[0]),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&views[1]),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(&views[2]),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::Sampler(&border_sampler),
                    },
                ],
            });

            let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("border_vb"),
                contents: bytemuck::cast_slice(&mesh.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
            let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("border_ib"),
                contents: bytemuck::cast_slice(&mesh.indices),
                usage: wgpu::BufferUsages::INDEX,
            });

            kinds_vec.push((
                mesh.kind,
                KindBuffers {
                    vertex_buffer,
                    index_buffer,
                    index_count: mesh.indices.len() as u32,
                    texture_bind_group,
                },
            ));
        }

        // Draw weaker internal lines first, then state/country borders on top.
        // Otherwise province strips can visually wash over national borders.
        kinds_vec.sort_by_key(|(kind, _)| border_render_priority(*kind));

        let any_loaded = !kinds_vec.is_empty();

        // ── Pipeline ──────────────────────────────────────────────────
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("border_strip_pl"),
            bind_group_layouts: &[&bgl_g0, &bgl_g1],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("border_strip_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<BorderVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: wgpu::VertexFormat::Float32x3,
                        },
                        wgpu::VertexAttribute {
                            offset: 12,
                            shader_location: 1,
                            format: wgpu::VertexFormat::Float32x3,
                        },
                        wgpu::VertexAttribute {
                            offset: 24,
                            shader_location: 2,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            offset: 32,
                            shader_location: 3,
                            format: wgpu::VertexFormat::Uint32,
                        },
                        wgpu::VertexAttribute {
                            offset: 36,
                            shader_location: 4,
                            format: wgpu::VertexFormat::Uint32,
                        },
                        wgpu::VertexAttribute {
                            offset: 40,
                            shader_location: 5,
                            format: wgpu::VertexFormat::Uint32,
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

        Self {
            pipeline,
            params_buffer,
            params_bind_group,
            kinds: kinds_vec,
            any_loaded,
            load_warnings: warnings,
            _owned_textures: owned_textures,
        }
    }

    pub fn update_params(&self, queue: &wgpu::Queue, params: &BorderParams) {
        queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(params));
    }

    pub fn render<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>) {
        if !self.any_loaded {
            return;
        }
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.params_bind_group, &[]);

        for (_kind, kb) in &self.kinds {
            pass.set_bind_group(1, &kb.texture_bind_group, &[]);
            pass.set_vertex_buffer(0, kb.vertex_buffer.slice(..));
            pass.set_index_buffer(kb.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..kb.index_count, 0, 0..1);
        }
    }
}

impl super::Pass for BorderPass {
    fn name(&self) -> &'static str {
        "border_strip"
    }
}

// ─── Helpers ────────────────────────────────────────────────────────────────

fn tex_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
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

fn lod_roles(kind: BorderKind) -> [MapResRole; 3] {
    match kind {
        BorderKind::Country => [
            MapResRole::BorderCountry(0),
            MapResRole::BorderCountry(1),
            MapResRole::BorderCountry(2),
        ],
        BorderKind::State => [
            MapResRole::BorderState(0),
            MapResRole::BorderState(1),
            MapResRole::BorderState(2),
        ],
        BorderKind::Province => [
            MapResRole::BorderProvince(0),
            MapResRole::BorderProvince(1),
            MapResRole::BorderProvince(2),
        ],
        BorderKind::Sea => [
            MapResRole::BorderSea(0),
            MapResRole::BorderSea(1),
            MapResRole::BorderSea(2),
        ],
        BorderKind::SeaRegion => [
            MapResRole::BorderSeaRegion(0),
            MapResRole::BorderSeaRegion(1),
            MapResRole::BorderSeaRegion(2),
        ],
        BorderKind::Impassable => [
            MapResRole::BorderImpassable(0),
            MapResRole::BorderImpassable(1),
            MapResRole::BorderImpassable(2),
        ],
    }
}

fn border_render_priority(kind: BorderKind) -> u8 {
    match kind {
        BorderKind::Province => 0,
        BorderKind::SeaRegion => 1,
        BorderKind::Sea => 2,
        BorderKind::State => 3,
        BorderKind::Impassable => 4,
        BorderKind::Country => 5,
    }
}

fn load_border_tex(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    db: &FsAssetDb,
    role: MapResRole,
    warnings: &mut Vec<String>,
    owned: &mut Vec<wgpu::Texture>,
) -> wgpu::TextureView {
    let path = role.relative_path();
    let bytes = match db.open(&path) {
        Ok(b) => b,
        Err(_) => {
            warnings.push(format!("[border] {} missing — using 1×1 fallback", path));
            return create_fallback_tex(device, queue, owned);
        }
    };
    let dds = match DdsImage::parse(&bytes) {
        Ok(d) => d,
        Err(e) => {
            warnings.push(format!(
                "[border] {} parse error: {} — using fallback",
                path, e
            ));
            return create_fallback_tex(device, queue, owned);
        }
    };

    let format = match dds.format {
        hoi4_assets::DdsFormat::Bc1 => wgpu::TextureFormat::Bc1RgbaUnormSrgb,
        hoi4_assets::DdsFormat::Bc3 => wgpu::TextureFormat::Bc3RgbaUnormSrgb,
        hoi4_assets::DdsFormat::Bc5 => wgpu::TextureFormat::Bc5RgUnorm,
        hoi4_assets::DdsFormat::Bgra8 => wgpu::TextureFormat::Bgra8UnormSrgb,
        _ => {
            warnings.push(format!(
                "[border] {} unsupported format — using fallback",
                path
            ));
            return create_fallback_tex(device, queue, owned);
        }
    };

    let upload_plan = match dds_upload_plan(&dds) {
        Some(plan) if plan.upload_mip_count > 0 => plan,
        _ => {
            warnings.push(format!(
                "[border] {} has no uploadable DDS mips — using fallback",
                path
            ));
            return create_fallback_tex(device, queue, owned);
        }
    };
    // Only upload mip 0 (we do LOD selection manually via 3 separate textures).
    let mip0 = &upload_plan.mips[0];

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(&path),
        size: wgpu::Extent3d {
            width: mip0.width,
            height: mip0.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });

    let data = &dds.data[mip0.offset..mip0.offset + mip0.size];
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        data,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(mip0.bytes_per_row),
            rows_per_image: None,
        },
        wgpu::Extent3d {
            width: mip0.copy_width,
            height: mip0.copy_height,
            depth_or_array_layers: 1,
        },
    );

    let view = texture.create_view(&Default::default());
    owned.push(texture);
    view
}

fn create_fallback_tex(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    owned: &mut Vec<wgpu::Texture>,
) -> wgpu::TextureView {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("border_fallback"),
        size: wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
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
        &[0u8, 0, 0, 0],
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
    owned.push(tex);
    view
}

// ─── Inline WGSL ────────────────────────────────────────────────────────────

const BORDER_STRIP_WGSL: &str = r#"
// BorderPass strip-mesh shader (vanilla border.shader equivalent).
// Vertex: world-space pos + UV from CPU-generated strip mesh.
// Fragment: sample BorderDiffuse with UV, apply shadow/fog/day-night.

struct GlobalFrameUniform {
    view_proj: mat4x4<f32>,
    virtual_sun_pos: vec4<f32>,
    virtual_moon_pos: vec4<f32>,
    second_virtual_sun_pos: vec4<f32>,
    second_virtual_moon_pos: vec4<f32>,
    day_night_hour_sun_dir: vec4<f32>,
    fow_opacity_time_snow_max_speed: vec4<f32>,
    cam_pos: vec3<f32>,
    hdr_range: f32,
    cam_look_at_dir: vec3<f32>,
    global_time: f32,
    screen_size: vec2<f32>,
    shadow_fade_factor: f32,
    fow_fade_factor: f32,
    sun_diffuse_intensity: vec4<f32>,
    moon_diffuse_intensity: vec4<f32>,
    min_mesh_alpha: f32,
    neg_fog_multiplier: f32,
    cubemap_intensity: f32,
    sun_specular_intensity: f32,
    shadow_view_proj: mat4x4<f32>,
    vanilla_map_size_world_size: vec4<f32>,
    cam_pos_map_px: vec4<f32>,
};

struct BorderParams {
    cam_distance_norm: f32,
    selection_intensity: f32,
    enabled_mask: u32,
    selected_province_id: u32,
    debug_view: u32,
    screen_width: f32,
    screen_height: f32,
    camera_distance_world: f32,
    world_size: vec2<f32>,
    map_size_px: vec2<f32>,
};

@group(0) @binding(0) var<uniform> frame: GlobalFrameUniform;
@group(0) @binding(1) var<uniform> bparams: BorderParams;

@group(1) @binding(0) var border_tex_lod0: texture_2d<f32>;
@group(1) @binding(1) var border_tex_lod1: texture_2d<f32>;
@group(1) @binding(2) var border_tex_lod2: texture_2d<f32>;
@group(1) @binding(3) var border_sampler: sampler;

struct VsIn {
    @location(0) pos: vec3<f32>,
    @location(1) center: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) province_a: u32,
    @location(4) province_b: u32,
    @location(5) kind: u32,
};

struct VsOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) world_pos: vec3<f32>,
    @location(2) center_clip: vec4<f32>,
    @location(3) @interpolate(flat) province_a: u32,
    @location(4) @interpolate(flat) province_b: u32,
    @location(5) @interpolate(flat) kind: u32,
    @location(6) map_uv: vec2<f32>,
    @location(7) map_px: vec2<f32>,
};

const BORDER_TILE: f32 = 0.4;
const VANILLA_PROVINCE_BORDER_FADE_NEAR: f32 = 200.0;
const VANILLA_PROVINCE_BORDER_FADE_FAR: f32 = 300.0;
const VANILLA_STATE_BORDER_FADE_NEAR: f32 = 400.0;
const VANILLA_STATE_BORDER_FADE_FAR: f32 = 500.0;
const KIND_COUNTRY: u32 = 0u;
const KIND_STATE: u32 = 1u;
const KIND_PROVINCE: u32 = 2u;
const KIND_SEA: u32 = 3u;
const KIND_SEA_REGION: u32 = 4u;
const KIND_IMPASSABLE: u32 = 5u;

const BORDER_DEBUG_OFF: u32 = 0u;
const BORDER_DEBUG_COUNTRY_ONLY: u32 = 1u;
const BORDER_DEBUG_STATE_ONLY: u32 = 2u;
const BORDER_DEBUG_PROVINCE_ONLY: u32 = 3u;
const BORDER_DEBUG_SEA_ONLY: u32 = 4u;
const BORDER_DEBUG_SELECTED_ONLY: u32 = 5u;
const BORDER_DEBUG_FALSE_COLOR: u32 = 6u;
const ID_NONE: u32 = 4294967295u;

fn world_xz_to_map_uv(world_xz: vec2<f32>, world_size: vec2<f32>) -> vec2<f32> {
    let uv_raw = world_xz / max(world_size, vec2<f32>(0.0001));
    return vec2<f32>(fract(uv_raw.x), clamp(uv_raw.y, 0.0, 1.0));
}

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var out: VsOut;
    let world_pos = in.pos;
    let map_uv = world_xz_to_map_uv(world_pos.xz, bparams.world_size);

    var clip = frame.view_proj * vec4<f32>(world_pos, 1.0);
    var center_clip = frame.view_proj * vec4<f32>(in.center, 1.0);
    // Z-bias: push slightly toward camera to avoid z-fight with terrain
    clip.z = clip.z - 0.002 * clip.w;
    center_clip.z = center_clip.z - 0.002 * center_clip.w;

    out.clip_pos = clip;
    out.uv = in.uv;
    out.world_pos = world_pos;
    out.center_clip = center_clip;
    out.province_a = in.province_a;
    out.province_b = in.province_b;
    out.kind = in.kind;
    out.map_uv = map_uv;
    out.map_px = map_uv * bparams.map_size_px;
    return out;
}

fn kind_enabled(kind: u32) -> bool {
    return (bparams.enabled_mask & (1u << kind)) != 0u;
}

fn selected_border(in: VsOut) -> bool {
    return bparams.selected_province_id != ID_NONE
        && (in.province_a == bparams.selected_province_id || in.province_b == bparams.selected_province_id);
}

fn debug_allows_kind(kind: u32, is_selected: bool) -> bool {
    if (bparams.debug_view == BORDER_DEBUG_OFF || bparams.debug_view == BORDER_DEBUG_FALSE_COLOR) {
        return true;
    }
    if (bparams.debug_view == BORDER_DEBUG_SELECTED_ONLY) {
        return is_selected;
    }
    if (bparams.debug_view == BORDER_DEBUG_COUNTRY_ONLY) {
        return kind == KIND_COUNTRY;
    }
    if (bparams.debug_view == BORDER_DEBUG_STATE_ONLY) {
        return kind == KIND_STATE;
    }
    if (bparams.debug_view == BORDER_DEBUG_PROVINCE_ONLY) {
        return kind == KIND_PROVINCE;
    }
    if (bparams.debug_view == BORDER_DEBUG_SEA_ONLY) {
        return kind == KIND_SEA || kind == KIND_SEA_REGION;
    }
    return true;
}

fn suppressed_in_final_view(kind: u32) -> bool {
    return bparams.debug_view == BORDER_DEBUG_OFF && (kind == KIND_SEA || kind == KIND_SEA_REGION);
}

fn false_color(kind: u32) -> vec3<f32> {
    if (kind == KIND_COUNTRY) {
        return vec3<f32>(1.0, 0.18, 0.12);
    }
    if (kind == KIND_STATE) {
        return vec3<f32>(1.0, 0.72, 0.08);
    }
    if (kind == KIND_PROVINCE) {
        return vec3<f32>(0.48, 1.0, 0.24);
    }
    if (kind == KIND_SEA) {
        return vec3<f32>(0.22, 0.72, 1.0);
    }
    if (kind == KIND_SEA_REGION) {
        return vec3<f32>(0.55, 0.38, 1.0);
    }
    return vec3<f32>(1.0, 0.30, 0.86);
}

fn hierarchy_alpha(kind: u32, zoom: f32, distance_norm: f32, camera_distance_world: f32, is_selected: bool) -> f32 {
    var alpha = 0.0;
    if (kind == KIND_COUNTRY) {
        alpha = mix(0.70, 0.42, distance_norm);
    } else if (kind == KIND_STATE) {
        let state_fade = 1.0 - smoothstep(
            95.0,
            175.0,
            camera_distance_world
        );
        alpha = 0.14 * state_fade * mix(0.82, 0.32, distance_norm);
    } else if (kind == KIND_PROVINCE) {
        let province_fade = 1.0 - smoothstep(
            38.0,
            78.0,
            camera_distance_world
        );
        alpha = 0.014 * province_fade * smoothstep(0.92, 0.99, zoom);
    } else if (kind == KIND_SEA) {
        alpha = 0.10 * smoothstep(0.56, 0.80, zoom);
    } else if (kind == KIND_SEA_REGION) {
        alpha = 0.045 * smoothstep(0.62, 0.82, zoom) * (1.0 - smoothstep(0.70, 0.90, distance_norm));
    } else {
        alpha = 0.38 * smoothstep(0.52, 0.72, zoom);
    }

    if (is_selected) {
        alpha = max(alpha, 0.90);
    }
    return alpha;
}

fn target_half_width_px(kind: u32, zoom: f32, is_selected: bool) -> f32 {
    var px = 0.75;
    if (kind == KIND_COUNTRY) {
        px = mix(0.82, 1.55, zoom);
    } else if (kind == KIND_STATE) {
        px = mix(0.32, 0.74, zoom);
    } else if (kind == KIND_PROVINCE) {
        px = mix(0.08, 0.26, zoom);
    } else if (kind == KIND_SEA) {
        px = mix(0.22, 0.50, zoom);
    } else if (kind == KIND_SEA_REGION) {
        px = mix(0.16, 0.38, zoom);
    } else {
        px = mix(0.42, 0.86, zoom);
    }
    if (is_selected) {
        px = max(px, 1.35);
    }
    return px;
}

fn display_line_color(kind: u32, sampled_rgb: vec3<f32>) -> vec3<f32> {
    if (kind == KIND_COUNTRY) {
        return mix(sampled_rgb, vec3<f32>(0.18, 0.18, 0.15), 0.55);
    }
    if (kind == KIND_STATE) {
        return mix(sampled_rgb, vec3<f32>(0.28, 0.29, 0.24), 0.72);
    }
    if (kind == KIND_PROVINCE) {
        return mix(sampled_rgb, vec3<f32>(0.39, 0.40, 0.34), 0.86);
    }
    return mix(sampled_rgb, vec3<f32>(0.30, 0.34, 0.34), 0.68);
}

fn screen_width_mask(in: VsOut, kind: u32, zoom: f32, is_selected: bool) -> f32 {
    let viewport = max(vec2<f32>(bparams.screen_width, bparams.screen_height), vec2<f32>(1.0, 1.0));
    let center_ndc = in.center_clip.xy / max(in.center_clip.w, 0.0001);
    let center_px = vec2<f32>(
        center_ndc.x * 0.5 + 0.5,
        0.5 - center_ndc.y * 0.5
    ) * viewport;
    let half_px = length(in.clip_pos.xy - center_px);
    let target_px = target_half_width_px(kind, zoom, is_selected);
    return 1.0 - smoothstep(target_px, target_px + 0.95, half_px);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let kind = in.kind;
    if (!kind_enabled(kind)) {
        discard;
    }
    if (suppressed_in_final_view(kind)) {
        discard;
    }

    let is_selected = selected_border(in);
    if (!debug_allows_kind(kind, is_selected)) {
        discard;
    }

    // Sample border texture — UV.y tiles along border, UV.x across width.
    // Vanilla uses: tex2D(BorderDiffuse, float2(uv.y * BORDER_TILE, uv.x))
    let sample_uv = vec2<f32>(in.uv.y * BORDER_TILE, in.uv.x);

    // LOD selection based on camera distance
    let d = bparams.cam_distance_norm;
    let s0 = textureSample(border_tex_lod0, border_sampler, sample_uv);
    let s1 = textureSample(border_tex_lod1, border_sampler, sample_uv);
    let s2 = textureSample(border_tex_lod2, border_sampler, sample_uv);

    var color: vec4<f32>;
    if (d < 0.5) {
        color = mix(s0, s1, d * 2.0);
    } else {
        color = mix(s1, s2, (d - 0.5) * 2.0);
    }

    var rgb = color.rgb;
    var alpha = color.a;

    let debug_active = bparams.debug_view != BORDER_DEBUG_OFF;
    if (alpha < 0.01 && !debug_active) {
        discard;
    }
    if (debug_active) {
        alpha = max(alpha, 0.82);
    }

    // The source border meshes are extracted from a pixel province bitmap.
    // Feather and distance-fade aggressively enough that far zoom does not turn
    // the map into a visible pixel grid, while retaining close political borders.
    let zoom = clamp(1.0 - d, 0.0, 1.0);
    let edge_fade = smoothstep(0.03, 0.42, in.uv.x) * (1.0 - smoothstep(0.58, 0.97, in.uv.x));
    let width_mask = screen_width_mask(in, kind, zoom, is_selected);
    var layer_alpha = hierarchy_alpha(kind, zoom, d, bparams.camera_distance_world, is_selected);
    if (debug_active) {
        layer_alpha = max(layer_alpha, 0.82);
    }
    alpha = alpha * edge_fade * width_mask * layer_alpha;
    if (!debug_active) {
        rgb = display_line_color(kind, rgb);
    }

    if (bparams.debug_view == BORDER_DEBUG_FALSE_COLOR) {
        rgb = false_color(kind);
        alpha = max(alpha, 0.86);
    }

    if (is_selected && bparams.selection_intensity > 0.0) {
        let pulse = (sin(frame.global_time * 2.5) * 0.5 + 0.5) * bparams.selection_intensity;
        rgb = mix(rgb, vec3<f32>(1.0, 0.84, 0.30), 0.56 + pulse * 0.20);
        alpha = max(alpha, 0.82 + pulse * 0.16);
    }

    // Simple distance fog (fade to transparent at extreme distance)
    let dist = length(in.world_pos - frame.cam_pos);
    let fog_fade = clamp(1.0 - dist / 500.0, 0.0, 1.0);

    return vec4<f32>(rgb, alpha * fog_fade);
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn border_params_size_is_48() {
        assert_eq!(std::mem::size_of::<BorderParams>(), 48);
    }

    #[test]
    fn border_params_default_enabled_mask_all() {
        let p = BorderParams::default();
        assert_eq!(p.enabled_mask, BorderParams::DEFAULT_VISIBLE_MASK);
        assert_eq!(BorderParams::ALL_VISIBLE_MASK, 0x3F);
        assert_eq!(p.map_size_px, [MAP_SIZE_X, MAP_SIZE_Y]);
    }

    #[test]
    fn border_strip_shader_uses_vanilla_tile_constant() {
        assert!(BORDER_STRIP_WGSL.contains("const BORDER_TILE: f32 = 0.4;"));
        assert!(BORDER_STRIP_WGSL.contains("vec2<f32>(in.uv.y * BORDER_TILE, in.uv.x)"));
    }

    #[test]
    fn internal_border_alpha_uses_far_zoom_suppression() {
        for needle in [
            "95.0",
            "175.0",
            "38.0",
            "78.0",
            "0.014 * province_fade",
            "display_line_color",
            "camera_distance_world",
            "fn suppressed_in_final_view",
            "kind == KIND_SEA || kind == KIND_SEA_REGION",
        ] {
            assert!(
                BORDER_STRIP_WGSL.contains(needle),
                "border shader missing {needle}"
            );
        }
    }

    #[test]
    fn border_debug_view_cycles_through_all_modes() {
        assert_eq!(BorderDebugView::Off.next(), BorderDebugView::CountryOnly);
        assert_eq!(
            BorderDebugView::FalseColorHierarchy.next(),
            BorderDebugView::Off
        );
    }

    #[test]
    fn border_strip_wgsl_naga_validates() {
        let module = match naga::front::wgsl::parse_str(BORDER_STRIP_WGSL) {
            Ok(module) => module,
            Err(err) => {
                eprintln!(
                    "[shader-validate] border strip WGSL parse failed:\n{}",
                    err.emit_to_string(BORDER_STRIP_WGSL)
                );
                panic!("border strip WGSL parse failed");
            }
        };

        let mut validator = naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        );
        if let Err(err) = validator.validate(&module) {
            eprintln!(
                "[shader-validate] border strip WGSL validation failed:\n{}",
                err.emit_to_string(BORDER_STRIP_WGSL)
            );
            panic!("border strip WGSL validation failed");
        }
    }
}
