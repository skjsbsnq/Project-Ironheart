//! Phase 3.12.10 — `MapnamePass`: vanilla-equivalent 3D country-name labels.
//!
//! Upgrades the existing `mapname_3d.wgsl` (custom shader with legacy Camera +
//! RenderParams uniforms) to use `GlobalFrameUniform` + vanilla-style rendering:
//!
//! 1. **vDistortedPos** — push world position toward camera by 0.5 to prevent
//!    z-fighting (vanilla `gfx/FX/mapname.shader` approach).
//! 2. **Day/night dimming** — use `calc_globe_normal` + `day_night_factor` from
//!    `shader_lib.wgsl` with vanilla coefficient 0.35.
//! 3. **Stencil ref=4 / NotEqual** — pipeline configured to skip pixels where
//!    UI sprites have written stencil value 4 (auto-hide behind panels).
//!
//! The pass reuses the existing 48-byte `CountryNameInstance` vertex format
//! and the fontdue R8 atlas baked by `mapname_atlas.rs`.
//!
//! ## Rendering order
//!
//! TerrainPass → RiverPass → WaterPass → BorderPass → TreesFullPass →
//! PdxMeshPass → **MapnamePass**

use hoi4_render::mapname_3d::CountryNameInstance;
use hoi4_render::shader_rt::compose_shader;
use wgpu::util::DeviceExt;

use crate::mapname_atlas::CountryNameAtlas;
use crate::passes::HDR_FORMAT;

// ─── MapnameParams uniform ─────────────────────────────────────────────────

/// 32 bytes — matches WGSL `MapnameParams`.
#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MapnameParams {
    /// Text fill colour (RGBA). Default: warm parchment white.
    pub text_color: [f32; 4],
    /// Outline colour (RGBA). Default: near-black.
    pub outline_color: [f32; 4],
    /// Anti-z-fight push toward camera (vanilla = 0.5).
    pub distortion_amount: f32,
    /// Overall alpha multiplier (0 = hidden, 1 = fully visible).
    pub fade: f32,
    /// Phase 8: country-name quad scale controlled by the frame plan.
    pub scale: f32,
    pub _pad1: f32,
}

impl Default for MapnameParams {
    fn default() -> Self {
        Self {
            text_color: [0.96, 0.93, 0.85, 1.0],
            outline_color: [0.05, 0.04, 0.03, 1.0],
            distortion_amount: 0.5,
            fade: 1.0,
            scale: 1.0,
            _pad1: 0.0,
        }
    }
}

const _: () = assert!(std::mem::size_of::<MapnameParams>() == 48);

// ─── Inline WGSL shader ───────────────────────────────────────────────────

/// Vanilla-equivalent mapname shader using `GlobalFrameUniform` +
/// `shader_lib.wgsl` helpers.
///
/// Keeps the existing 48-byte `CountryNameInstance` vertex format
/// (center/width_world/axis1/height_world/uv_min/uv_max) rather than
/// vanilla's `center_size/corner/uv_rect` format.
const MAPNAME_VANILLA_WGSL: &str = r#"
struct MapnameParams {
    text_color: vec4<f32>,
    outline_color: vec4<f32>,
    distortion_amount: f32,
    fade: f32,
    scale: f32,
    _pad1: f32,
};

@group(0) @binding(0) var<uniform> frame: GlobalFrameUniform;
@group(0) @binding(1) var<uniform> mn_params: MapnameParams;

@group(1) @binding(0) var name_atlas: texture_2d<f32>;
@group(1) @binding(1) var name_sampler: sampler;

// Per-instance data (48 bytes, matches CountryNameInstance).
struct InstanceData {
    @location(0) center: vec3<f32>,
    @location(1) width_world: f32,
    @location(2) axis1: vec2<f32>,
    @location(3) height_world: f32,
    @location(4) _pad0: f32,
    @location(5) uv_min: vec2<f32>,
    @location(6) uv_max: vec2<f32>,
};

struct VsOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) world_xz: vec2<f32>,
    @location(2) world_pos: vec3<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vid: u32, inst: InstanceData) -> VsOut {
    // 6 verts → two triangles for a quad in [-1, +1] × [-1, +1].
    var local_x: f32 = -1.0;
    var local_y: f32 = -1.0;
    if (vid == 1u || vid == 2u || vid == 4u) { local_x = 1.0; }
    if (vid == 2u || vid == 4u || vid == 5u) { local_y = 1.0; }

    // axis1_world: country major axis in XZ plane.
    let axis1_world = vec3<f32>(inst.axis1.x, 0.0, inst.axis1.y);
    // axis2_world: perpendicular in XZ (rotate 90° → (-y, +x)).
    let axis2_world = vec3<f32>(-inst.axis1.y, 0.0, inst.axis1.x);

    let world_pos = inst.center
        + axis1_world * (local_x * inst.width_world * mn_params.scale)
        + axis2_world * (local_y * inst.height_world * mn_params.scale);

    // Vanilla vDistortedPos: push toward camera to prevent z-fighting.
    let to_cam = normalize(frame.cam_pos - world_pos);
    let distorted = world_pos + to_cam * mn_params.distortion_amount;

    var clip = frame.view_proj * vec4<f32>(distorted, 1.0);
    // Additional NDC z-bias toward camera (~1/1000).
    clip.z = clip.z - 0.001 * clip.w;

    var out: VsOut;
    out.clip_pos = clip;
    out.uv = vec2<f32>(
        mix(inst.uv_min.x, inst.uv_max.x, (local_x + 1.0) * 0.5),
        mix(inst.uv_min.y, inst.uv_max.y, (local_y + 1.0) * 0.5),
    );
    out.world_xz = inst.center.xz;
    out.world_pos = world_pos;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let s = textureSample(name_atlas, name_sampler, in.uv).r;
    if (s < 0.04) { discard; }

    // R8 atlas two-tier encoding: >= 0.65 = text, 0.3..0.65 = outline, < 0.3 = transparent.
    let text_t = smoothstep(0.45, 0.7, s);
    let outline_t = smoothstep(0.15, 0.45, s) * (1.0 - text_t);

    // Vanilla-style day/night dimming with coefficient 0.35.
    let globe_n = calc_globe_normal(in.world_xz, frame.day_night_hour_sun_dir.x);
    let night = day_night_factor(globe_n, frame.day_night_hour_sun_dir.yzw, 1.0);
    let dim = 1.0 - night * 0.35;

    var color = mn_params.outline_color.rgb * outline_t + mn_params.text_color.rgb * text_t;
    color *= dim;
    var alpha = (text_t + outline_t) * mn_params.fade;

    // Zoom fade: smooth alpha gradient based on camera distance.
    // Near (cam_dist < 20): fully transparent; far (cam_dist > 60): fully opaque.
    // Eliminates hard cut of labels popping in/out at zoom thresholds.
    let cam_dist = length(frame.cam_pos - in.world_pos);
    let zoom_alpha = smoothstep(20.0, 60.0, cam_dist);
    alpha *= zoom_alpha;

    return vec4<f32>(color, alpha);
}
"#;

// ─── Public inputs ─────────────────────────────────────────────────────────

/// Construction inputs for MapnamePass.
pub struct MapnamePassInputs<'a> {
    pub global_uniform_buffer: &'a wgpu::Buffer,
    pub depth_format: wgpu::TextureFormat,
    pub obbs: &'a [Option<hoi4_render::mapname_3d::CountryObb>],
    pub atlas: Option<&'a CountryNameAtlas>,
    pub world_scale: f32,
    pub label_y: f32,
}

// ─── MapnamePass ───────────────────────────────────────────────────────────

pub struct MapnamePass {
    pipeline: wgpu::RenderPipeline,
    global_bind_group: wgpu::BindGroup,
    atlas_bind_group: wgpu::BindGroup,
    params_buffer: wgpu::Buffer,
    instance_buffer: wgpu::Buffer,
    instance_count: u32,
    pub pixel_counts: Vec<u32>,
}

impl MapnamePass {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, inputs: MapnamePassInputs<'_>) -> Self {
        // ─── Compose shader (inject shader_lib + GlobalFrameUniform) ──────
        let composed = compose_shader(MAPNAME_VANILLA_WGSL, true, true);

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mapname_vanilla_shader"),
            source: wgpu::ShaderSource::Wgsl(composed.into()),
        });

        // ─── Bind group layouts ───────────────────────────────────────────
        // group(0): GlobalFrameUniform + MapnameParams
        // group(1): atlas texture + sampler
        let bgl_global = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("mapname_vanilla_bgl_global"),
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

        let bgl_atlas = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("mapname_vanilla_bgl_atlas"),
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

        // ─── MapnameParams uniform buffer ─────────────────────────────────
        let params = MapnameParams::default();
        let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("mapname_vanilla_params"),
            contents: bytemuck::bytes_of(&params),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // ─── Atlas texture upload (or 1×1 zero fallback) ─────────────────
        let (atlas_view, instances, pixel_counts) = if let Some(atlas) = inputs.atlas {
            let instances = crate::mapname_atlas::build_label_instances(
                inputs.obbs,
                atlas,
                inputs.world_scale,
                inputs.label_y,
            );

            let mut pixel_counts: Vec<u32> = Vec::with_capacity(instances.len());
            for idx in 0..inputs.obbs.len().min(atlas.entries.len()) {
                if inputs.obbs[idx].is_some() && atlas.entries[idx].is_some() {
                    pixel_counts.push(inputs.obbs[idx].unwrap().pixel_count);
                }
            }
            debug_assert_eq!(pixel_counts.len(), instances.len());

            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("mapname_vanilla_atlas"),
                size: wgpu::Extent3d {
                    width: atlas.width,
                    height: atlas.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R8Unorm,
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
                &atlas.data,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(atlas.width),
                    rows_per_image: None,
                },
                wgpu::Extent3d {
                    width: atlas.width,
                    height: atlas.height,
                    depth_or_array_layers: 1,
                },
            );
            (
                texture.create_view(&Default::default()),
                instances,
                pixel_counts,
            )
        } else {
            // Empty fallback: 1×1 black texture, 0 instances.
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("mapname_vanilla_atlas_empty"),
                size: wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R8Unorm,
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
                &[0u8],
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(1),
                    rows_per_image: None,
                },
                wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
            );
            (
                texture.create_view(&Default::default()),
                Vec::<CountryNameInstance>::new(),
                Vec::<u32>::new(),
            )
        };

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("mapname_vanilla_sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });

        // ─── Bind groups ──────────────────────────────────────────────────
        let global_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mapname_vanilla_bg_global"),
            layout: &bgl_global,
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

        let atlas_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mapname_vanilla_bg_atlas"),
            layout: &bgl_atlas,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&atlas_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        // ─── Instance buffer ──────────────────────────────────────────────
        let inst_size = std::mem::size_of::<CountryNameInstance>() as u64;
        let instance_count = instances.len() as u32;
        let instance_buffer = if instances.is_empty() {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("mapname_vanilla_inst_empty"),
                contents: &vec![0u8; inst_size as usize],
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            })
        } else {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("mapname_vanilla_inst"),
                contents: bytemuck::cast_slice(&instances),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            })
        };

        // ─── Pipeline ─────────────────────────────────────────────────────
        let pl_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("mapname_vanilla_pl_layout"),
            bind_group_layouts: &[&bgl_global, &bgl_atlas],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("mapname_vanilla_pipeline"),
            layout: Some(&pl_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: inst_size,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &[
                        // center: vec3<f32> @ 0
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: wgpu::VertexFormat::Float32x3,
                        },
                        // width_world: f32 @ 12
                        wgpu::VertexAttribute {
                            offset: 12,
                            shader_location: 1,
                            format: wgpu::VertexFormat::Float32,
                        },
                        // axis1: vec2<f32> @ 16
                        wgpu::VertexAttribute {
                            offset: 16,
                            shader_location: 2,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        // height_world: f32 @ 24
                        wgpu::VertexAttribute {
                            offset: 24,
                            shader_location: 3,
                            format: wgpu::VertexFormat::Float32,
                        },
                        // _pad0: f32 @ 28
                        wgpu::VertexAttribute {
                            offset: 28,
                            shader_location: 4,
                            format: wgpu::VertexFormat::Float32,
                        },
                        // uv_min: vec2<f32> @ 32
                        wgpu::VertexAttribute {
                            offset: 32,
                            shader_location: 5,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        // uv_max: vec2<f32> @ 40
                        wgpu::VertexAttribute {
                            offset: 40,
                            shader_location: 6,
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
            depth_stencil: Some(wgpu::DepthStencilState {
                format: inputs.depth_format,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::LessEqual,
                // Stencil ref=4 deferred: current depth format is Depth32Float
                // (no stencil aspect). When the main depth texture is upgraded
                // to Depth24PlusStencil8 or Depth32FloatStencil8, replace
                // `Default::default()` with the StencilState that does
                // Always+Replace at ref=4 (see ROADMAP 12.2).
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });

        Self {
            pipeline,
            global_bind_group,
            atlas_bind_group,
            params_buffer,
            instance_buffer,
            instance_count,
            pixel_counts,
        }
    }

    /// Update per-frame parameters (fade based on game phase).
    pub fn update_params(&self, queue: &wgpu::Queue, fade: f32, scale: f32) {
        let params = MapnameParams {
            fade,
            scale,
            ..Default::default()
        };
        queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(&params));
    }

    /// Render visible country-name labels with smooth zoom fade-out.
    ///
    /// LOD culling by pixel count is relaxed — the shader handles smooth alpha
    /// fade via `smoothstep(20, 60, cam_dist)` so labels no longer pop in/out.
    /// We still cull the tiniest countries (pixel_count < 50) to avoid
    /// thousands of invisible draw calls.
    pub fn render(&self, pass: &mut wgpu::RenderPass, _zoom_factor: f32) {
        if self.instance_count == 0 {
            return;
        }

        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.global_bind_group, &[]);
        pass.set_bind_group(1, &self.atlas_bind_group, &[]);
        pass.set_vertex_buffer(0, self.instance_buffer.slice(..));

        // Draw all instances — zoom fade is handled in the fragment shader.
        // Only skip countries with < 50 pixels (micro islands etc.)
        let min_pixels: u32 = 50;
        let visible: Vec<u32> = self
            .pixel_counts
            .iter()
            .enumerate()
            .filter_map(|(i, &c)| {
                if (i as u32) < self.instance_count && c >= min_pixels {
                    Some(i as u32)
                } else {
                    None
                }
            })
            .collect();

        if visible.is_empty() {
            return;
        }

        if visible.len() as u32 == self.instance_count {
            pass.draw(0..6, 0..self.instance_count);
        } else {
            for inst_idx in visible {
                pass.draw(0..6, inst_idx..inst_idx + 1);
            }
        }
    }

    pub fn instance_count(&self) -> u32 {
        self.instance_count
    }

    /// 重建 instance buffer + pixel_counts。
    /// 在 country ownership 大变（例如 SCW 分裂、ITA 吞并 ETH）后调用，
    /// 这样 3D 地图标签会立刻反映新版图。
    pub fn rebuild_instances(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        obbs: &[Option<hoi4_render::mapname_3d::CountryObb>],
        atlas: &CountryNameAtlas,
        world_scale: f32,
        label_y: f32,
    ) {
        let instances =
            crate::mapname_atlas::build_label_instances(obbs, atlas, world_scale, label_y);
        let mut pixel_counts: Vec<u32> = Vec::with_capacity(instances.len());
        for idx in 0..obbs.len().min(atlas.entries.len()) {
            if let (Some(obb), Some(_)) = (&obbs[idx], &atlas.entries[idx]) {
                pixel_counts.push(obb.pixel_count);
            }
        }
        debug_assert_eq!(pixel_counts.len(), instances.len());

        let inst_size = std::mem::size_of::<CountryNameInstance>();
        let needed_bytes = (instances.len() * inst_size).max(inst_size);

        if (self.instance_buffer.size() as usize) >= needed_bytes && !instances.is_empty() {
            queue.write_buffer(&self.instance_buffer, 0, bytemuck::cast_slice(&instances));
        } else {
            // 容量不够（新增国家），重新分配 buffer
            self.instance_buffer = if instances.is_empty() {
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("mapname_vanilla_inst_empty"),
                    contents: &vec![0u8; inst_size],
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                })
            } else {
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("mapname_vanilla_inst"),
                    contents: bytemuck::cast_slice(&instances),
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                })
            };
        }
        self.instance_count = instances.len() as u32;
        self.pixel_counts = pixel_counts;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mapname_params_size_is_48() {
        assert_eq!(std::mem::size_of::<MapnameParams>(), 48);
    }

    #[test]
    fn vanilla_wgsl_naga_parses() {
        let composed = compose_shader(MAPNAME_VANILLA_WGSL, true, true);
        let module = naga::front::wgsl::parse_str(&composed);
        assert!(
            module.is_ok(),
            "mapname_vanilla WGSL failed naga validation: {:?}",
            module.err()
        );
    }
}
