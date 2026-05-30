//! Phase 3.12.10 — Particle pass (combat smoke / factory chimneys / scorched earth).
//!
//! Renders camera-facing billboard quads with alpha-blended soft particles.
//! CPU-side emitter logic lives in `hoi4_render::particles::ParticleEmitter`.
//! This pass owns the GPU pipeline, instance buffer, and the 1×1 white
//! fallback texture (vanilla particle textures are not parsed yet — Phase 7).
//!
//! ## Render order
//!
//! Drawn after terrain / water / river / border / trees / buildings / mapname,
//! before the HDR pass ends. Alpha blend (premultiplied), no depth write,
//! LessEqual depth test — particles layer on top of all opaque geometry.

use hoi4_render::global_uniform::GLOBAL_FRAME_UNIFORM_SIZE;
use hoi4_render::particles::{ParticleEmitter, ParticleInstance};
use hoi4_render::shader_rt;
use wgpu::util::DeviceExt;

const PARTICLE_WGSL: &str = r#"
@group(0) @binding(0) var<uniform> frame: GlobalFrameUniform;
@group(0) @binding(1) var<uniform> p_params: ParticleParams;

@group(1) @binding(0) var particle_tex: texture_2d<f32>;
@group(1) @binding(1) var p_sampler: sampler;

struct ParticleParams {
    fade_start: f32,
    fade_stop: f32,
    soft_thickness: f32,
    _pad: f32,
};

struct VsOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) tint: vec4<f32>,
    @location(3) view_z: f32,
};

@vertex
fn vs_main(
    @location(0) world_pos: vec3<f32>,
    @location(1) size: f32,
    @location(2) color: vec4<f32>,
    @location(3) age_lifetime: vec2<f32>,
    @location(4) rotation: f32,
    @builtin(vertex_index) vid: u32,
) -> VsOut {
    var out: VsOut;

    let corner = vec2<f32>(
        f32(i32(vid & 1u)) * 2.0 - 1.0,
        f32(i32((vid >> 1u) & 1u)) * 2.0 - 1.0,
    );
    let uv = corner * 0.5 + 0.5;

    let to_cam = normalize(frame.cam_pos - world_pos);
    let up = vec3<f32>(0.0, 1.0, 0.0);
    let right = normalize(cross(up, to_cam));
    let upr = cross(to_cam, right);

    let cs = cos(rotation);
    let sn = sin(rotation);
    let local = vec2<f32>(
        corner.x * cs - corner.y * sn,
        corner.x * sn + corner.y * cs,
    ) * size;

    let world = world_pos + right * local.x + upr * local.y;
    out.clip_pos = frame.view_proj * vec4<f32>(world, 1.0);
    out.world_pos = world;
    out.uv = uv;
    out.tint = color;
    out.view_z = out.clip_pos.z;

    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let s = textureSample(particle_tex, p_sampler, in.uv);
    if (s.a < 0.01) {
        discard;
    }

    let d = length(in.world_pos - frame.cam_pos);
    let fade = 1.0 - smoothstep(p_params.fade_start, p_params.fade_stop, d);

    var color = s.rgb * in.tint.rgb;
    var alpha = s.a * in.tint.a * fade;

    return vec4<f32>(color * alpha, alpha);
}
"#;

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct GpuParticleParams {
    fade_start: f32,
    fade_stop: f32,
    soft_thickness: f32,
    _pad: f32,
}

pub struct ParticlePass {
    pipeline: wgpu::RenderPipeline,
    instance_buffer: wgpu::Buffer,
    instance_capacity: u32,
    params_buffer: wgpu::Buffer,
    bg_g0: wgpu::BindGroup,
    bg_g1: wgpu::BindGroup,
    _white_texture: wgpu::Texture,
    _white_sampler: wgpu::Sampler,
    bgl_g0: wgpu::BindGroupLayout,
    bgl_g1: wgpu::BindGroupLayout,
    pub emitter: ParticleEmitter,
    pub enabled: bool,
}

impl ParticlePass {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        global_uniform_buffer: &wgpu::Buffer,
        hdr_format: wgpu::TextureFormat,
        depth_format: wgpu::TextureFormat,
    ) -> Self {
        let white_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("particle_white_tex"),
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
        let white_bytes: [u8; 4] = [255, 255, 255, 255];
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &white_tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &white_bytes,
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
        let white_view = white_tex.create_view(&wgpu::TextureViewDescriptor::default());

        let white_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("particle_sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let bgl_g0 = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("particle_bgl_g0"),
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
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: Some(
                            std::num::NonZeroU64::new(
                                std::mem::size_of::<GpuParticleParams>() as u64
                            )
                            .unwrap(),
                        ),
                    },
                    count: None,
                },
            ],
        });

        let bgl_g1 = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("particle_bgl_g1"),
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

        let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("particle_params"),
            contents: bytemuck::bytes_of(&GpuParticleParams {
                fade_start: 80.0,
                fade_stop: 400.0,
                soft_thickness: 0.5,
                _pad: 0.0,
            }),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bg_g0 = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("particle_bg_g0"),
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

        let bg_g1 = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("particle_bg_g1"),
            layout: &bgl_g1,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&white_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&white_sampler),
                },
            ],
        });

        let initial_cap = 256u32;
        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("particle_instances"),
            size: (initial_cap as u64) * (std::mem::size_of::<ParticleInstance>() as u64),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("particle_pipeline_layout"),
            bind_group_layouts: &[&bgl_g0, &bgl_g1],
            push_constant_ranges: &[],
        });

        let composed = shader_rt::compose_shader(PARTICLE_WGSL, true, true);
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("particle_shader"),
            source: wgpu::ShaderSource::Wgsl(composed.into()),
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("particle_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<ParticleInstance>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &[
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x3,
                            offset: 0,
                            shader_location: 0,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32,
                            offset: 12,
                            shader_location: 1,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x4,
                            offset: 16,
                            shader_location: 2,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x2,
                            offset: 32,
                            shader_location: 3,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32,
                            offset: 40,
                            shader_location: 4,
                        },
                    ],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: hdr_format,
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
            instance_buffer,
            instance_capacity: initial_cap,
            params_buffer,
            bg_g0,
            bg_g1,
            _white_texture: white_tex,
            _white_sampler: white_sampler,
            bgl_g0,
            bgl_g1,
            emitter: ParticleEmitter::new(),
            enabled: true,
        }
    }

    /// Update particle simulation and upload instances to GPU.
    /// Call once per frame before `render()`.
    pub fn update(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, dt: f32) {
        self.emitter.update(dt);

        let mut instances = Vec::new();
        self.emitter.collect_instances(&mut instances);

        if instances.is_empty() {
            return;
        }

        let needed = instances.len() as u32;
        if needed > self.instance_capacity {
            let new_cap = needed.next_power_of_two().max(256);
            self.instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("particle_instances"),
                size: (new_cap as u64) * (std::mem::size_of::<ParticleInstance>() as u64),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.instance_capacity = new_cap;
        }

        queue.write_buffer(&self.instance_buffer, 0, bytemuck::cast_slice(&instances));
    }

    /// Update fade params (call when camera distance changes).
    pub fn update_params(&self, queue: &wgpu::Queue, fade_start: f32, fade_stop: f32) {
        let p = GpuParticleParams {
            fade_start,
            fade_stop,
            soft_thickness: 0.5,
            _pad: 0.0,
        };
        queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(&p));
    }

    /// Render particles into the current 3D pass.
    /// Each particle is a 6-vertex quad (2 triangles) generated procedurally
    /// from the vertex index; instance data drives position/size/color.
    pub fn render(&self, pass: &mut wgpu::RenderPass<'_>) {
        if !self.enabled {
            return;
        }
        let count = self.emitter.alive_count();
        if count == 0 {
            return;
        }

        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bg_g0, &[]);
        pass.set_bind_group(1, &self.bg_g1, &[]);
        pass.set_vertex_buffer(0, self.instance_buffer.slice(..));
        pass.draw(0..6, 0..count);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gpu_params_size_is_16_bytes() {
        assert_eq!(std::mem::size_of::<GpuParticleParams>(), 16);
    }
}
