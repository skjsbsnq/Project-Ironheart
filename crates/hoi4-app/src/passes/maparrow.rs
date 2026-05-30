//! Phase 16.1 — `MapArrowPass`: vanilla maparrow.shader equivalent.
//!
//! Renders military order arrows (move / invade / paradrop / naval transport)
//! as instanced quads along a path with blinking animation.
//!
//! ## Data source
//!
//! Currently uses mock data (hardcoded test arrows). Will be connected to
//! `World.divisions.orders[i]` when V3 4.8 military panel is implemented.
//!
//! ## Rendering
//!
//! - Instanced quad per arrow segment (body / head / tail types)
//! - Procedural arrow pattern in fragment shader (no texture dependency)
//! - Alpha-blended, no depth write, LessEqual depth test
//! - Blinking animation via `global_time`

use wgpu::util::DeviceExt;

use crate::passes::HDR_FORMAT;

// ─── Arrow segment instance ────────────────────────────────────────────────

/// 32 bytes — matches WGSL `ArrowInstance`.
#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ArrowInstance {
    pub start_xz: [f32; 2],
    pub end_xz: [f32; 2],
    pub width: f32,
    pub segment_type: f32, // 0=body, 1=head, 2=frontline, 3=tail
    pub start_y: f32,
    pub end_y: f32,
}

// ─── ArrowParams uniform ───────────────────────────────────────────────────

/// 128 bytes — matches WGSL `ArrowParams` (std140 padded).
#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ArrowParams {
    pub blink_speed: f32,
    pub blink_range: f32,
    pub overlay_opacity: f32,
    pub frontline_opacity: f32,
    pub body_color: [f32; 4],
    pub head_color: [f32; 4],
    pub frontline_color: [f32; 4],
    pub move_color: [f32; 4],
    pub battleplan_color: [f32; 4],
    pub battleplan_head_color: [f32; 4],
    pub _pad0: [f32; 4],
}

impl Default for ArrowParams {
    fn default() -> Self {
        Self {
            blink_speed: hoi4_render::defines::MAP_ARROW_SEL_BLINK_SPEED,
            blink_range: hoi4_render::defines::MAP_ARROW_SEL_BLINK_RANGE,
            overlay_opacity: 1.0,
            frontline_opacity: 1.0,
            body_color: [0.95, 0.76, 0.18, 0.78],
            head_color: [1.0, 0.92, 0.38, 0.92],
            frontline_color: [0.86, 0.16, 0.12, 0.82],
            move_color: [0.78, 0.9, 1.0, 0.58],
            battleplan_color: [0.88, 0.58, 0.06, 0.62],
            battleplan_head_color: [1.0, 0.78, 0.18, 0.88],
            _pad0: [0.0; 4],
        }
    }
}

const _: () = assert!(std::mem::size_of::<ArrowParams>() == 128);

// ─── MapArrowPass ──────────────────────────────────────────────────────────

pub struct MapArrowPass {
    pipeline: wgpu::RenderPipeline,
    quad_vb: wgpu::Buffer,
    instance_buffer: wgpu::Buffer,
    instance_capacity: u32,
    instance_count: u32,
    params_buffer: wgpu::Buffer,
    params_bind_group: wgpu::BindGroup,
    pub any_loaded: bool,
    pub load_warnings: Vec<String>,
}

impl MapArrowPass {
    pub fn new(
        device: &wgpu::Device,
        _queue: &wgpu::Queue,
        global_uniform_buffer: &wgpu::Buffer,
        depth_format: wgpu::TextureFormat,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("maparrow_shader"),
            source: wgpu::ShaderSource::Wgsl(MAPARROW_WGSL.into()),
        });

        // Params uniform
        let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("maparrow_params"),
            contents: bytemuck::bytes_of(&ArrowParams::default()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Bind group layout: group(0) = GlobalFrameUniform + ArrowParams
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("maparrow_bgl"),
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
            label: Some("maparrow_bg"),
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

        // Quad vertex buffer: 2 triangles forming a [-1,1]×[-1,1] quad
        #[repr(C)]
        #[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
        struct QuadVert {
            corner: [f32; 2],
            uv: [f32; 2],
        }
        let quad_verts: [QuadVert; 6] = [
            QuadVert {
                corner: [-1.0, -1.0],
                uv: [0.0, 1.0],
            },
            QuadVert {
                corner: [1.0, -1.0],
                uv: [1.0, 1.0],
            },
            QuadVert {
                corner: [-1.0, 1.0],
                uv: [0.0, 0.0],
            },
            QuadVert {
                corner: [-1.0, 1.0],
                uv: [0.0, 0.0],
            },
            QuadVert {
                corner: [1.0, -1.0],
                uv: [1.0, 1.0],
            },
            QuadVert {
                corner: [1.0, 1.0],
                uv: [1.0, 0.0],
            },
        ];
        let quad_vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("maparrow_quad_vb"),
            contents: bytemuck::cast_slice(&quad_verts),
            usage: wgpu::BufferUsages::VERTEX,
        });

        // Initial empty instance buffer (will be grown on first upload)
        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("maparrow_instance_buf"),
            size: 256, // space for 8 instances initially
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("maparrow_pl"),
            bind_group_layouts: &[&bgl],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("maparrow_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[
                    // Quad vertex
                    wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<QuadVert>() as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &[
                            wgpu::VertexAttribute {
                                offset: 0,
                                shader_location: 0,
                                format: wgpu::VertexFormat::Float32x2,
                            }, // corner
                            wgpu::VertexAttribute {
                                offset: 8,
                                shader_location: 1,
                                format: wgpu::VertexFormat::Float32x2,
                            }, // uv
                        ],
                    },
                    // Instance data
                    wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<ArrowInstance>() as u64,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &[
                            wgpu::VertexAttribute {
                                offset: 0,
                                shader_location: 2,
                                format: wgpu::VertexFormat::Float32x2,
                            },
                            wgpu::VertexAttribute {
                                offset: 8,
                                shader_location: 3,
                                format: wgpu::VertexFormat::Float32x2,
                            },
                            wgpu::VertexAttribute {
                                offset: 16,
                                shader_location: 4,
                                format: wgpu::VertexFormat::Float32,
                            },
                            wgpu::VertexAttribute {
                                offset: 20,
                                shader_location: 5,
                                format: wgpu::VertexFormat::Float32,
                            },
                            wgpu::VertexAttribute {
                                offset: 24,
                                shader_location: 6,
                                format: wgpu::VertexFormat::Float32,
                            },
                            wgpu::VertexAttribute {
                                offset: 28,
                                shader_location: 7,
                                format: wgpu::VertexFormat::Float32,
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
                    constant: 4,
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
            quad_vb,
            instance_buffer,
            instance_capacity: 8,
            instance_count: 0,
            params_buffer,
            params_bind_group,
            any_loaded: false,
            load_warnings: Vec::new(),
        }
    }

    /// Upload arrow instances for this frame. Call from the render prepare phase.
    pub fn set_arrows(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        instances: &[ArrowInstance],
    ) {
        if instances.is_empty() {
            self.instance_count = 0;
            self.any_loaded = false;
            return;
        }
        self.any_loaded = true;
        self.instance_count = instances.len() as u32;

        // Grow buffer if needed
        if instances.len() as u32 > self.instance_capacity {
            self.instance_capacity = (instances.len() as u32).next_power_of_two();
            self.instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("maparrow_instance_buf"),
                size: (self.instance_capacity as usize * std::mem::size_of::<ArrowInstance>())
                    as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }

        queue.write_buffer(&self.instance_buffer, 0, bytemuck::cast_slice(instances));
    }

    pub fn update_params(&self, queue: &wgpu::Queue, params: &ArrowParams) {
        queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(params));
    }

    pub fn render<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>) {
        if !self.any_loaded || self.instance_count == 0 {
            return;
        }
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.params_bind_group, &[]);
        pass.set_vertex_buffer(0, self.quad_vb.slice(..));
        pass.set_vertex_buffer(1, self.instance_buffer.slice(..));
        pass.draw(0..6, 0..self.instance_count);
    }
}

impl super::Pass for MapArrowPass {
    fn name(&self) -> &'static str {
        "maparrow"
    }
}

// ─── Mock data generator ───────────────────────────────────────────────────

/// Generate mock arrow instances for testing.
/// Returns a few hardcoded arrows (Berlin→Warsaw, etc.) to verify the pipeline.
/// Will be replaced by `World.divisions.orders` data when V3 4.8 is ready.
pub fn generate_mock_arrows(world_size: [f32; 2]) -> Vec<ArrowInstance> {
    // World coordinates (WORLD_SCALE=0.02, so pixel coords × 0.02)
    // Approximate province centroids in world units
    let berlin = [56.0 * 0.02, 40.0 * 0.02];
    let warsaw = [70.0 * 0.02, 36.0 * 0.02];
    let paris = [38.0 * 0.02, 42.0 * 0.02];
    let london = [34.0 * 0.02, 50.0 * 0.02];
    let rome = [48.0 * 0.02, 30.0 * 0.02];

    let _ = world_size; // suppress unused warning
    let width = 0.015;

    let mut arrows = Vec::new();

    // Arrow 1: Berlin → Warsaw (body + head)
    let mid = [(berlin[0] + warsaw[0]) * 0.5, (berlin[1] + warsaw[1]) * 0.5];
    arrows.push(ArrowInstance {
        start_xz: berlin,
        end_xz: mid,
        width,
        segment_type: 0.0,
        start_y: 1.0,
        end_y: 1.0,
    });
    arrows.push(ArrowInstance {
        start_xz: mid,
        end_xz: warsaw,
        width,
        segment_type: 1.0,
        start_y: 1.0,
        end_y: 1.0,
    });

    // Arrow 2: Paris → London (body + head)
    let mid2 = [(paris[0] + london[0]) * 0.5, (paris[1] + london[1]) * 0.5];
    arrows.push(ArrowInstance {
        start_xz: paris,
        end_xz: mid2,
        width,
        segment_type: 0.0,
        start_y: 1.0,
        end_y: 1.0,
    });
    arrows.push(ArrowInstance {
        start_xz: mid2,
        end_xz: london,
        width,
        segment_type: 1.0,
        start_y: 1.0,
        end_y: 1.0,
    });

    // Arrow 3: Berlin → Rome (body + body + head)
    let q1 = [
        berlin[0] + (rome[0] - berlin[0]) * 0.33,
        berlin[1] + (rome[1] - berlin[1]) * 0.33,
    ];
    let q2 = [
        berlin[0] + (rome[0] - berlin[0]) * 0.66,
        berlin[1] + (rome[1] - berlin[1]) * 0.66,
    ];
    arrows.push(ArrowInstance {
        start_xz: berlin,
        end_xz: q1,
        width,
        segment_type: 0.0,
        start_y: 1.0,
        end_y: 1.0,
    });
    arrows.push(ArrowInstance {
        start_xz: q1,
        end_xz: q2,
        width,
        segment_type: 0.0,
        start_y: 1.0,
        end_y: 1.0,
    });
    arrows.push(ArrowInstance {
        start_xz: q2,
        end_xz: rome,
        width,
        segment_type: 1.0,
        start_y: 1.0,
        end_y: 1.0,
    });

    arrows
}

// ─── Inline WGSL ──────────────────────────────────────────────────────────

const MAPARROW_WGSL: &str = r#"
// maparrow.wgsl — Procedural military order arrow rendering.
// Instanced quads along arrow path with blinking animation.

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

struct ArrowParams {
    blink_speed: f32,
    blink_range: f32,
    overlay_opacity: f32,
    frontline_opacity: f32,
    body_color: vec4<f32>,
    head_color: vec4<f32>,
    frontline_color: vec4<f32>,
    move_color: vec4<f32>,
    battleplan_color: vec4<f32>,
    battleplan_head_color: vec4<f32>,
    _pad0: vec4<f32>,
};

@group(0) @binding(0) var<uniform> frame: GlobalFrameUniform;
@group(0) @binding(1) var<uniform> a_params: ArrowParams;

struct VsIn {
    @location(0) corner: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) start_xz: vec2<f32>,
    @location(3) end_xz: vec2<f32>,
    @location(4) width: f32,
    @location(5) segment_type: f32,
    @location(6) start_y: f32,
    @location(7) end_y: f32,
};

struct VsOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) seg_t: f32,
    @location(2) seg_type: f32,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var out: VsOut;
    let dir = in.end_xz - in.start_xz;
    let len = length(dir);
    let dir_n = dir / max(len, 0.001);
    let perp = vec2<f32>(-dir_n.y, dir_n.x);

    let t = in.corner.x * 0.5 + 0.5;
    let along = mix(in.start_xz, in.end_xz, t);
    let world_xz = along + perp * in.corner.y * in.width;

    let world_y = mix(in.start_y, in.end_y, t);
    out.clip_pos = frame.view_proj * vec4<f32>(world_xz.x, world_y, world_xz.y, 1.0);
    out.clip_pos.z -= 0.005 * out.clip_pos.w;
    out.uv = in.uv;
    out.seg_t = t;
    out.seg_type = in.segment_type;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let edge = 1.0 - smoothstep(0.72, 1.0, abs(in.uv.y * 2.0 - 1.0));
    var alpha = edge;

    let is_move = in.seg_type > 3.5 && in.seg_type < 5.5;
    let is_battleplan = in.seg_type > 5.5;
    let is_battleplan_head = in.seg_type > 6.5 && in.seg_type < 7.5 || in.seg_type > 9.5 && in.seg_type < 10.5;
    let is_battleplan_tail = in.seg_type > 7.5 && in.seg_type < 8.5 || in.seg_type > 10.5;
    let is_battleplan_executing = in.seg_type > 8.5;

    if (is_move) {
        let center_y = in.uv.y * 2.0 - 1.0;
        if (in.seg_type > 4.5) {
            let point = smoothstep(0.05, 0.55, in.seg_t);
            let head_width = mix(0.18, 1.05, point);
            alpha = (1.0 - smoothstep(head_width * 0.78, head_width, abs(center_y))) * point;
        } else {
            let lane = 1.0 - smoothstep(0.42, 0.72, abs(center_y));
            let dash_phase = fract(in.uv.x * 4.5 - frame.global_time * 0.55);
            let dash = smoothstep(0.06, 0.16, dash_phase) * (1.0 - smoothstep(0.62, 0.82, dash_phase));
            alpha = alpha * lane * (0.28 + 0.72 * dash);
        }
    } else if (is_battleplan) {
        let center_y = in.uv.y * 2.0 - 1.0;
        if (is_battleplan_head) {
            let head_t = smoothstep(0.0, 0.95, in.seg_t);
            let head_width = mix(0.22, 1.18, head_t);
            alpha = (1.0 - smoothstep(head_width * 0.82, head_width, abs(center_y))) * smoothstep(0.04, 0.42, head_t);
        } else if (is_battleplan_tail) {
            let tail = smoothstep(0.0, 0.9, in.seg_t);
            let tail_width = mix(0.18, 0.92, tail);
            alpha = alpha * tail * (1.0 - smoothstep(tail_width * 0.82, tail_width, abs(center_y)));
        } else {
            let body_edge = 1.0 - smoothstep(0.78, 1.0, abs(center_y));
            let center_lane = 0.82 + 0.18 * smoothstep(0.0, 0.35, abs(center_y));
            alpha = alpha * body_edge * center_lane;
        }
        if (is_battleplan_executing) {
            let wave = 0.72 + 0.28 * smoothstep(0.0, 1.0, fract(in.uv.x * 2.6 - frame.global_time * 0.65));
            alpha = alpha * wave;
        }
    } else if (in.seg_type > 2.5) {
        let center_y = in.uv.y * 2.0 - 1.0;
        let taper = smoothstep(0.0, 0.85, in.seg_t);
        let tail_width = mix(0.25, 0.9, taper);
        let tail_edge = 1.0 - smoothstep(0.72 * tail_width, tail_width, abs(center_y));
        alpha = alpha * taper * tail_edge;
    } else if (in.seg_type > 1.5) {
        let blink = (cos(frame.global_time * a_params.blink_speed) * 0.5 + 0.5) * a_params.blink_range;
        let stripe = 0.92 + 0.08 * smoothstep(0.15, 0.85, in.uv.x);
        let color = a_params.frontline_color.rgb * (1.0 + blink) * stripe;
        return vec4<f32>(color, alpha * a_params.frontline_color.a * a_params.frontline_opacity);
    }

    // Head: triangular tip at seg_t near 1
    if (in.seg_type > 0.5 && in.seg_type < 1.5) {
        let head_t = in.seg_t;
        let head_shape = smoothstep(0.12, 0.65, head_t);
        let head_width = 0.55 + head_t * 0.75;
        let center_y = in.uv.y * 2.0 - 1.0;
        let head_edge = 1.0 - smoothstep(0.7 * head_width, head_width, abs(center_y));
        alpha = head_shape * head_edge;
    }

    if (in.seg_type < 0.5) {
        let lane = 0.88 + 0.12 * smoothstep(0.0, 1.0, fract(in.uv.x * 2.0 + frame.global_time * 0.18));
        alpha = alpha * lane;
    }

    if (alpha < 0.02) {
        discard;
    }

    let blink = (cos(frame.global_time * a_params.blink_speed) * 0.5 + 0.5) * a_params.blink_range;

    var color = a_params.body_color.rgb;
    var out_alpha = a_params.body_color.a;
    if (is_move) {
        color = a_params.move_color.rgb;
        out_alpha = a_params.move_color.a;
    } else if (is_battleplan_head) {
        color = a_params.battleplan_head_color.rgb;
        out_alpha = a_params.battleplan_head_color.a;
    } else if (is_battleplan) {
        color = a_params.battleplan_color.rgb;
        out_alpha = a_params.battleplan_color.a;
    } else if (in.seg_type > 0.5 && in.seg_type < 1.5) {
        color = a_params.head_color.rgb;
        out_alpha = a_params.head_color.a;
    }
    color *= 1.0 + blink;

    return vec4<f32>(color, alpha * out_alpha * a_params.overlay_opacity);
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arrow_instance_size_is_32() {
        assert_eq!(std::mem::size_of::<ArrowInstance>(), 32);
    }

    #[test]
    fn arrow_params_size_is_128() {
        assert_eq!(std::mem::size_of::<ArrowParams>(), 128);
    }

    #[test]
    fn mock_arrows_generated() {
        let arrows = generate_mock_arrows([100.0, 100.0]);
        assert!(!arrows.is_empty());
        // Should have at least Berlin→Warsaw + Paris→London + Berlin→Rome
        assert!(arrows.len() >= 7);
    }
}
