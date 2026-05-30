//! Phase 16.3 — `StraitPass`: vanilla strait.shader equivalent.
//!
//! Renders strait crossings (Gibraltar / Danish / Suez / Panama) as
//! pulsing dashed lines between province centroids.
//!
//! ## Data source
//!
//! Uses `world.map.special_adjacencies` where `adj_type == Canal` or
//! the adjacency is a known strait (through province != 0).
//!
//! ## Rendering
//!
//! - Line strip with procedural dashed + pulse pattern
//! - Alpha-blended, no depth write, LessEqual depth test

use hoi4_map::adjacency::{Adjacency, AdjacencyType};
use wgpu::util::DeviceExt;

use crate::passes::HDR_FORMAT;

// ─── StraitParams uniform ──────────────────────────────────────────────────

/// 80 bytes — matches WGSL `StraitParams` (std140 padded).
#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct StraitParams {
    pub color: [f32; 4],
    pub pulse_speed: f32,
    pub opacity: f32,
    pub _pad: [f32; 2],
    pub _pad2: [f32; 4],
    pub _pad3: [f32; 4],
    pub _pad4: [f32; 4],
}

impl Default for StraitParams {
    fn default() -> Self {
        Self {
            color: [0.6, 0.85, 1.0, 0.8],
            pulse_speed: 2.0,
            opacity: 1.0,
            _pad: [0.0; 2],
            _pad2: [0.0; 4],
            _pad3: [0.0; 4],
            _pad4: [0.0; 4],
        }
    }
}

const _: () = assert!(std::mem::size_of::<StraitParams>() == 80);

// ─── Strait vertex ─────────────────────────────────────────────────────────

/// 20 bytes — matches WGSL `VsIn`.
#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct StraitVertex {
    pub world_pos: [f32; 3],
    pub uv: [f32; 2],
}

// ─── StraitPass ────────────────────────────────────────────────────────────

pub struct StraitPass {
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    vertex_count: u32,
    params_buffer: wgpu::Buffer,
    params_bind_group: wgpu::BindGroup,
    pub any_loaded: bool,
    pub load_warnings: Vec<String>,
}

impl StraitPass {
    pub fn new(
        device: &wgpu::Device,
        _queue: &wgpu::Queue,
        global_uniform_buffer: &wgpu::Buffer,
        depth_format: wgpu::TextureFormat,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("strait_shader"),
            source: wgpu::ShaderSource::Wgsl(STRAIT_WGSL.into()),
        });

        let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("strait_params"),
            contents: bytemuck::bytes_of(&StraitParams::default()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("strait_bgl"),
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
            label: Some("strait_bg"),
            layout: &bgl,
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

        // Empty vertex buffer initially; will be filled on first build_straits call
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("strait_vb"),
            size: 4096,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("strait_pl"),
            bind_group_layouts: &[&bgl],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("strait_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<StraitVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: wgpu::VertexFormat::Float32x3,
                        }, // world_pos
                        wgpu::VertexAttribute {
                            offset: 12,
                            shader_location: 1,
                            format: wgpu::VertexFormat::Float32x2,
                        }, // uv
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
                format: depth_format,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: Default::default(),
                bias: wgpu::DepthBiasState {
                    constant: 8,
                    slope_scale: 2.0,
                    clamp: 0.0,
                },
            }),
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });

        Self {
            pipeline,
            vertex_buffer,
            vertex_count: 0,
            params_buffer,
            params_bind_group,
            any_loaded: false,
            load_warnings: Vec::new(),
        }
    }

    /// Build strait geometry from adjacency data and province centroids.
    /// Province centroids are in heightmap-pixel coords; this converts to world units.
    pub fn build_straits(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        adjacencies: &[Adjacency],
        province_centroids: &[(f32, f32)],
        world_scale: f32,
    ) {
        let mut verts = Vec::new();
        let y = 0.22; // slightly above water

        for adj in adjacencies {
            // Render canals and sea straits (through != 0 means it's a special adjacency)
            if adj.adj_type != AdjacencyType::Canal && adj.through == 0 {
                continue;
            }

            let from_idx = adj.from as usize;
            let to_idx = adj.to as usize;

            if from_idx >= province_centroids.len() || to_idx >= province_centroids.len() {
                continue;
            }

            let (fx, fz) = province_centroids[from_idx];
            let (tx, tz) = province_centroids[to_idx];

            // Skip if centroids are at origin (unmapped)
            if fx.abs() < 0.01 && fz.abs() < 0.01 {
                continue;
            }
            if tx.abs() < 0.01 && tz.abs() < 0.01 {
                continue;
            }

            let p0 = [fx * world_scale, fz * world_scale];
            let p1 = [tx * world_scale, tz * world_scale];

            // Generate a strip quad between the two points
            let dir = [p1[0] - p0[0], p1[1] - p0[1]];
            let len = (dir[0] * dir[0] + dir[1] * dir[1]).sqrt();
            if len < 0.001 {
                continue;
            }
            let width = 0.008;
            let perp = [-dir[1] / len * width, dir[0] / len * width];

            // Interpolate across the strait for better UV mapping
            let segments = 4;
            for i in 0..segments {
                let t0 = i as f32 / segments as f32;
                let t1 = (i + 1) as f32 / segments as f32;
                let a = [p0[0] + dir[0] * t0, p0[1] + dir[1] * t0];
                let b = [p0[0] + dir[0] * t1, p0[1] + dir[1] * t1];

                let u0 = t0;
                let u1 = t1;

                // Quad as 2 triangles
                verts.push(StraitVertex {
                    world_pos: [a[0] + perp[0], y, a[1] + perp[1]],
                    uv: [u0, 0.0],
                });
                verts.push(StraitVertex {
                    world_pos: [a[0] - perp[0], y, a[1] - perp[1]],
                    uv: [u0, 1.0],
                });
                verts.push(StraitVertex {
                    world_pos: [b[0] + perp[0], y, b[1] + perp[1]],
                    uv: [u1, 0.0],
                });

                verts.push(StraitVertex {
                    world_pos: [a[0] - perp[0], y, a[1] - perp[1]],
                    uv: [u0, 1.0],
                });
                verts.push(StraitVertex {
                    world_pos: [b[0] - perp[0], y, b[1] - perp[1]],
                    uv: [u1, 1.0],
                });
                verts.push(StraitVertex {
                    world_pos: [b[0] + perp[0], y, b[1] + perp[1]],
                    uv: [u1, 0.0],
                });
            }
        }

        if verts.is_empty() {
            self.vertex_count = 0;
            self.any_loaded = false;
            return;
        }

        self.any_loaded = true;
        self.vertex_count = verts.len() as u32;

        // Recreate buffer if needed
        let needed = (verts.len() * std::mem::size_of::<StraitVertex>()) as u64;
        if needed > self.vertex_buffer.size() {
            self.vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("strait_vb"),
                contents: bytemuck::cast_slice(&verts),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            });
        } else {
            queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&verts));
        }
    }

    pub fn update_params(&self, queue: &wgpu::Queue, params: &StraitParams) {
        queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(params));
    }

    pub fn render<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>) {
        if !self.any_loaded || self.vertex_count == 0 {
            return;
        }
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.params_bind_group, &[]);
        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        pass.draw(0..self.vertex_count, 0..1);
    }
}

impl super::Pass for StraitPass {
    fn name(&self) -> &'static str {
        "strait"
    }
}

// ─── Inline WGSL ──────────────────────────────────────────────────────────

const STRAIT_WGSL: &str = r#"
// strait.wgsl — Procedural pulsing dashed strait crossing lines.

struct GlobalFrameUniform {
    view_proj: mat4x4<f32>,
    virtual_sun_pos: vec4<f32>,
    second_virtual_sun_pos: vec4<f32>,
    second_virtual_moon_pos: vec4<f32>,
    day_night_hour_sun_dir: vec4<f32>,
    fow_opacity_time_snow_max_speed: vec4<f32>,
    cam_pos: vec3<f32>,
    hdr_exposure: f32,
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
};

struct StraitParams {
    color: vec4<f32>,
    pulse_speed: f32,
    opacity: f32,
    _pad0: f32,
    _pad2: f32,
    _pad3: vec4<f32>,
    _pad4: vec4<f32>,
    _pad5: vec4<f32>,
};

@group(0) @binding(0) var<uniform> frame: GlobalFrameUniform;
@group(0) @binding(1) var<uniform> s_params: StraitParams;

struct VsIn {
    @location(0) world_pos: vec3<f32>,
    @location(1) uv: vec2<f32>,
};

struct VsOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var out: VsOut;
    out.clip_pos = frame.view_proj * vec4<f32>(in.world_pos, 1.0);
    // Z-bias to stay above water
    out.clip_pos.z -= 0.005 * out.clip_pos.w;
    out.uv = in.uv;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // Dashed line pattern along the strait
    let dash = fract(in.uv.x * 12.0);
    let dash_pattern = step(0.25, dash) * step(dash, 0.75);

    // Edge fade (fade at strip edges)
    let edge_fade = 1.0 - smoothstep(0.3, 0.5, abs(in.uv.y * 2.0 - 1.0));

    // Pulsing animation
    let pulse = 0.5 + 0.5 * cos(frame.global_time * s_params.pulse_speed);

    let alpha = dash_pattern * edge_fade * s_params.color.a * s_params.opacity;
    if (alpha < 0.02) {
        discard;
    }

    let color = s_params.color.rgb * (0.6 + 0.4 * pulse);
    return vec4<f32>(color, alpha);
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strait_params_size_is_80() {
        assert_eq!(std::mem::size_of::<StraitParams>(), 80);
    }

    #[test]
    fn strait_vertex_size_is_20() {
        assert_eq!(std::mem::size_of::<StraitVertex>(), 20);
    }
}
