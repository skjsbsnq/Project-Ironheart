//! Phase 3.12.11 — Sky pass + EnvironmentMap cubemap.
//!
//! Renders a full-screen sky background by sampling a cubemap texture.
//! The cubemap is loaded from vanilla `gfx/loadingscreens/sky_*.dds` (6 faces).
//! The same cubemap view is shared with WaterPass and PdxMeshPass for
//! environment reflections.

use hoi4_assets::dds::{DdsFormat, DdsImage};
use hoi4_assets::{AssetDb, FsAssetDb};
use hoi4_paths::PathConfig;
use hoi4_render::global_uniform::GLOBAL_FRAME_UNIFORM_SIZE;
use hoi4_render::shader_rt;
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct SkyParams {
    inv_view_proj: [[f32; 4]; 4],
}

const SKY_WGSL: &str = r#"
@group(0) @binding(0) var<uniform> frame: GlobalFrameUniform;
@group(0) @binding(1) var<uniform> sky: SkyParams;

@group(1) @binding(0) var sky_cube: texture_cube<f32>;
@group(1) @binding(1) var sky_sampler: sampler;

struct SkyParams {
    inv_view_proj: mat4x4<f32>,
};

struct VsOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) view_dir: vec3<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vid: u32) -> VsOut {
    var out: VsOut;
    // Full-screen triangle: vid 0→(-1,-1), 1→(3,-1), 2→(-1,3)
    let x = f32(i32(vid & 1u)) * 4.0 - 1.0;
    let y = f32(i32(vid >> 1u)) * 4.0 - 1.0;
    out.clip_pos = vec4<f32>(x, y, 1.0, 1.0);
    let far_point = sky.inv_view_proj * vec4<f32>(x, y, 1.0, 1.0);
    let world_pos = far_point.xyz / max(far_point.w, 0.0001);
    out.view_dir = normalize(world_pos - frame.cam_pos);
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let dir = normalize(in.view_dir);
    var color = textureSample(sky_cube, sky_sampler, dir).rgb;

    let horizon = 1.0 - smoothstep(0.02, 0.38, abs(dir.y));
    let fog_color = vec3<f32>(0.58, 0.66, 0.70);
    color = mix(color, fog_color, horizon * 0.20);
    color *= 1.12;

    let sun_dir = normalize(frame.day_night_hour_sun_dir.yzw);
    let sun_above = smoothstep(-0.04, 0.16, sun_dir.y);
    let sun_dot = max(dot(dir, sun_dir), 0.0);
    let sun_disk = pow(sun_dot, 720.0);
    let sun_glow = pow(sun_dot, 40.0) * 0.46 + pow(sun_dot, 9.0) * 0.12;
    color += vec3<f32>(1.0, 0.88, 0.62) * (sun_disk * 3.0 + sun_glow) * sun_above;

    // Day/night: darken + shift to blue at night
    let globe_n = calc_globe_normal(frame.cam_pos_map_px.xy, frame.day_night_hour_sun_dir.x);
    let night = day_night_factor(globe_n, frame.day_night_hour_sun_dir.yzw, 1.0);
    let night_color = vec3<f32>(0.055, 0.070, 0.095);
    color = mix(color, night_color, night * 0.52);

    return vec4<f32>(color, 1.0);
}
"#;

pub struct SkyPass {
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    params_buffer: wgpu::Buffer,
    /// The cubemap texture view — shared with WaterPass / PdxMeshPass.
    pub cubemap_view: wgpu::TextureView,
    /// Whether a real sky cubemap was loaded (false = fallback dim-blue).
    pub loaded: bool,
    _owned_texture: wgpu::Texture,
    _owned_sampler: wgpu::Sampler,
    bg_g0: wgpu::BindGroup,
}

impl SkyPass {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        global_uniform_buffer: &wgpu::Buffer,
        hdr_format: wgpu::TextureFormat,
        depth_format: wgpu::TextureFormat,
        path_cfg: &PathConfig,
    ) -> Self {
        let (cube_tex, cube_view, loaded) = load_sky_cubemap(device, queue, path_cfg);
        let sky_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("sky_sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let bgl_g0 = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("sky_bgl_g0"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: Some(
                            std::num::NonZeroU64::new(GLOBAL_FRAME_UNIFORM_SIZE as u64).unwrap(),
                        ),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: Some(
                            std::num::NonZeroU64::new(std::mem::size_of::<SkyParams>() as u64)
                                .unwrap(),
                        ),
                    },
                    count: None,
                },
            ],
        });
        let bgl_g2 = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("sky_bgl_g2"),
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

        let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("sky_params"),
            contents: bytemuck::bytes_of(&SkyParams {
                inv_view_proj: [[0.0; 4]; 4],
            }),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bg_g0 = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("sky_bg_g0"),
            layout: &bgl_g0,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: global_uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: params_buffer.as_entire_binding(),
                },
            ],
        });
        let bg_g2 = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("sky_bg_g2"),
            layout: &bgl_g2,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&cube_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sky_sampler),
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("sky_pipeline_layout"),
            bind_group_layouts: &[&bgl_g0, &bgl_g2],
            push_constant_ranges: &[],
        });

        let composed = shader_rt::compose_shader(SKY_WGSL, true, true);
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("sky_shader"),
            source: wgpu::ShaderSource::Wgsl(composed.into()),
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sky_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: hdr_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: depth_format,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        });

        Self {
            pipeline,
            bind_group: bg_g2,
            params_buffer,
            cubemap_view: cube_view,
            loaded,
            _owned_texture: cube_tex,
            _owned_sampler: sky_sampler,
            bg_g0,
        }
    }

    /// Update the inverse view-proj matrix (call once per frame before render).
    pub fn update_params(&self, queue: &wgpu::Queue, inv_view_proj: &[[f32; 4]; 4]) {
        let p = SkyParams {
            inv_view_proj: *inv_view_proj,
        };
        queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(&p));
    }

    /// Render the sky as the first draw call in the 3D pass.
    /// Must be called before terrain/water/etc. so the sky paints the background.
    pub fn render(&self, pass: &mut wgpu::RenderPass<'_>) {
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bg_g0, &[]);
        pass.set_bind_group(1, &self.bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}

/// Vanilla sky cubemap face filenames.
/// HOI4 uses `gfx/loadingscreens/sky_{pos,neg}_{x,y,z}.dds`.
const SKY_FACE_NAMES: [&str; 6] = [
    "gfx/loadingscreens/sky_pos_x.dds",
    "gfx/loadingscreens/sky_neg_x.dds",
    "gfx/loadingscreens/sky_pos_y.dds",
    "gfx/loadingscreens/sky_neg_y.dds",
    "gfx/loadingscreens/sky_pos_z.dds",
    "gfx/loadingscreens/sky_neg_z.dds",
];

/// Load 6 DDS face files and assemble into a single wgpu cubemap.
/// Returns `(texture, view, loaded)`. On any failure, falls back to a
/// 1×1×6 dim-blue procedural cubemap.
fn load_sky_cubemap(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    path_cfg: &PathConfig,
) -> (wgpu::Texture, wgpu::TextureView, bool) {
    let db = FsAssetDb::new(path_cfg.clone());

    let mut faces: Option<Vec<DecodedFace>> = None;
    for (i, name) in SKY_FACE_NAMES.iter().enumerate() {
        match try_load_face(&db, name) {
            Ok(f) => {
                if faces.is_none() {
                    faces = Some(Vec::new());
                }
                let faces_vec = faces.as_mut().unwrap();
                if faces_vec.len() != i {
                    break;
                }
                faces_vec.push(f);
            }
            Err(_) => break,
        }
    }

    let faces = match faces {
        Some(f) if f.len() == 6 => f,
        _ => {
            return create_fallback_cubemap(device, queue);
        }
    };

    let w = faces[0].width;
    let h = faces[0].height;
    let format = faces[0].format;
    let is_compressed = faces[0].bytes_per_pixel == 0;

    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("sky_cubemap"),
        size: wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 6,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });

    for (layer, face) in faces.iter().enumerate() {
        let bytes_per_row = if is_compressed {
            None
        } else {
            Some(w * face.bytes_per_pixel)
        };
        let rows_per_image = if is_compressed { None } else { Some(h) };
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &tex,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: 0,
                    y: 0,
                    z: layer as u32,
                },
                aspect: wgpu::TextureAspect::All,
            },
            &face.data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row,
                rows_per_image,
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
    }

    let view = tex.create_view(&wgpu::TextureViewDescriptor {
        label: Some("sky_cubemap_view"),
        dimension: Some(wgpu::TextureViewDimension::Cube),
        ..Default::default()
    });

    (tex, view, true)
}

struct DecodedFace {
    width: u32,
    height: u32,
    data: Vec<u8>,
    format: wgpu::TextureFormat,
    bytes_per_pixel: u32,
}

fn try_load_face(db: &FsAssetDb, path: &str) -> Result<DecodedFace, String> {
    let bytes = db.open(path).map_err(|e| format!("{}", e))?;
    let dds = DdsImage::parse(&bytes).map_err(|e| e.to_string())?;
    let mip0 = dds
        .mip_data(0)
        .ok_or_else(|| "DDS missing mip0".to_owned())?;
    match dds.format {
        DdsFormat::Bgra8 => {
            let w = dds.width;
            let h = dds.height;
            let mut rgba = Vec::with_capacity((w * h * 4) as usize);
            for chunk in mip0.chunks(4) {
                if chunk.len() < 4 {
                    break;
                }
                rgba.push(chunk[2]); // R
                rgba.push(chunk[1]); // G
                rgba.push(chunk[0]); // B
                rgba.push(chunk[3]); // A
            }
            Ok(DecodedFace {
                width: w,
                height: h,
                data: rgba,
                format: wgpu::TextureFormat::Rgba8Unorm,
                bytes_per_pixel: 4,
            })
        }
        DdsFormat::Bc1 | DdsFormat::Bc3 => {
            let format = if dds.format == DdsFormat::Bc1 {
                wgpu::TextureFormat::Bc1RgbaUnorm
            } else {
                wgpu::TextureFormat::Bc3RgbaUnorm
            };
            Ok(DecodedFace {
                width: dds.width,
                height: dds.height,
                data: mip0.to_vec(),
                format,
                bytes_per_pixel: 0,
            })
        }
        DdsFormat::Bc5 => Err("Unsupported sky DDS format: BC5".to_owned()),
        DdsFormat::Bgr555 => Err("Unsupported sky DDS format: BGR555".to_owned()),
        DdsFormat::Unknown(code) => Err(format!("Unsupported DDS format: {}", code)),
    }
}

fn create_fallback_cubemap(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> (wgpu::Texture, wgpu::TextureView, bool) {
    const SIZE: u32 = 128;
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("sky_cubemap_procedural_fallback"),
        size: wgpu::Extent3d {
            width: SIZE,
            height: SIZE,
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
        let face = build_procedural_sky_face(layer, SIZE);
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
            &face,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * SIZE),
                rows_per_image: Some(SIZE),
            },
            wgpu::Extent3d {
                width: SIZE,
                height: SIZE,
                depth_or_array_layers: 1,
            },
        );
    }
    let view = tex.create_view(&wgpu::TextureViewDescriptor {
        label: Some("sky_cubemap_procedural_fallback_view"),
        dimension: Some(wgpu::TextureViewDimension::Cube),
        ..Default::default()
    });
    (tex, view, false)
}

fn build_procedural_sky_face(layer: u32, size: u32) -> Vec<u8> {
    let mut data = Vec::with_capacity((size * size * 4) as usize);
    let inv_size = 1.0 / size as f32;
    for y in 0..size {
        let v = (y as f32 + 0.5) * inv_size * 2.0 - 1.0;
        for x in 0..size {
            let u = (x as f32 + 0.5) * inv_size * 2.0 - 1.0;
            data.extend_from_slice(&procedural_sky_rgba(cube_face_direction(layer, u, v)));
        }
    }
    data
}

fn cube_face_direction(layer: u32, u: f32, v: f32) -> [f32; 3] {
    let dir = match layer {
        0 => [1.0, -v, -u],
        1 => [-1.0, -v, u],
        2 => [u, 1.0, v],
        3 => [u, -1.0, -v],
        4 => [u, -v, 1.0],
        _ => [-u, -v, -1.0],
    };
    normalize3(dir)
}

fn procedural_sky_rgba(dir: [f32; 3]) -> [u8; 4] {
    let up = saturate(dir[1] * 0.5 + 0.5);
    let horizon = 1.0 - smoothstep(0.02, 0.52, dir[1].abs());
    let below_horizon = smoothstep(0.02, 0.70, -dir[1]);

    let mut color = mix3([0.100, 0.160, 0.205], [0.310, 0.500, 0.720], up.powf(0.70));
    color = mix3(color, [0.720, 0.770, 0.790], horizon * 0.54);
    color = mix3(color, [0.060, 0.090, 0.110], below_horizon * 0.80);

    let azimuth = dir[2].atan2(dir[0]);
    let mid_sky = smoothstep(-0.08, 0.24, dir[1]) * (1.0 - smoothstep(0.62, 0.98, dir[1]));
    let cloud_noise = ((azimuth * 6.5 + dir[0] * 12.0 - dir[2] * 4.0).sin() * 0.5 + 0.5) * 0.58
        + ((azimuth * 13.0 - dir[0] * 5.0 + dir[2] * 17.0).sin() * 0.5 + 0.5) * 0.42;
    let cloud = smoothstep(0.58, 0.86, cloud_noise) * mid_sky * 0.40;
    color = mix3(color, [0.865, 0.885, 0.855], cloud);

    let sun_dir = normalize3([0.05, 0.44, 0.90]);
    let sun_dot = dot3(dir, sun_dir).max(0.0);
    let sun_glow = sun_dot.powf(32.0) * 0.46 + sun_dot.powf(360.0) * 1.75;
    color = add3(color, mul3([1.0, 0.850, 0.560], sun_glow));

    [to_u8(color[0]), to_u8(color[1]), to_u8(color[2]), 255]
}

fn normalize3(v: [f32; 3]) -> [f32; 3] {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-6);
    [v[0] / len, v[1] / len, v[2] / len]
}

fn dot3(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn mix3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    let t = saturate(t);
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

fn add3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn mul3(v: [f32; 3], s: f32) -> [f32; 3] {
    [v[0] * s, v[1] * s, v[2] * s]
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = saturate((x - edge0) / (edge1 - edge0).max(1e-6));
    t * t * (3.0 - 2.0 * t)
}

fn saturate(x: f32) -> f32 {
    x.clamp(0.0, 1.0)
}

fn to_u8(x: f32) -> u8 {
    (saturate(x) * 255.0).round() as u8
}
