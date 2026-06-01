//! Phase 16.2 — `TradeRoutePass`: vanilla traderoute.shader equivalent.
//!
//! Renders trade routes as flowing dashed lines between country capitals.
//!
//! ## Data source
//!
//! Reads `World.countries.trade.routes` and builds strip meshes from the
//! route endpoints used by the simulation.
//!
//! ## Rendering
//!
//! - Strip mesh along path (triangle list, 2 verts per cross-section)
//! - Procedural flowing dashed line in fragment shader
//! - Alpha-blended, no depth write, LessEqual depth test

use wgpu::util::DeviceExt;

use crate::passes::HDR_FORMAT;

// ─── TradeRouteParams uniform ──────────────────────────────────────────────

/// 80 bytes — matches WGSL `TradeParams` (std140 padded).
#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TradeRouteParams {
    pub flow_speed: f32,
    pub opacity: f32,
    pub _pad0: [f32; 2],
    pub color_a: [f32; 4],
    pub color_b: [f32; 4],
    pub _pad1: [f32; 4],
    pub _pad2: [f32; 4],
}

impl Default for TradeRouteParams {
    fn default() -> Self {
        Self {
            flow_speed: 0.3,
            opacity: 1.0,
            _pad0: [0.0; 2],
            color_a: [0.2, 0.6, 0.9, 0.7], // blue-ish
            color_b: [0.9, 0.8, 0.2, 0.7], // gold-ish
            _pad1: [0.0; 4],
            _pad2: [0.0; 4],
        }
    }
}

const _: () = assert!(std::mem::size_of::<TradeRouteParams>() == 80);

// ─── Trade route vertex ────────────────────────────────────────────────────

/// 28 bytes — matches WGSL `VsIn`.
#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TradeRouteVertex {
    pub world_pos: [f32; 3],
    pub uv: [f32; 2],
    pub trade_amount: f32,
    pub _pad: f32,
}

// ─── TradeRoutePass ────────────────────────────────────────────────────────

pub struct TradeRoutePass {
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    vertex_capacity: u32,
    vertex_count: u32,
    params_buffer: wgpu::Buffer,
    params_bind_group: wgpu::BindGroup,
    pub any_loaded: bool,
    pub load_warnings: Vec<String>,
}

impl TradeRoutePass {
    pub fn new(
        device: &wgpu::Device,
        _queue: &wgpu::Queue,
        global_uniform_buffer: &wgpu::Buffer,
        depth_format: wgpu::TextureFormat,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("traderoute_shader"),
            source: wgpu::ShaderSource::Wgsl(TRADEROUTE_WGSL.into()),
        });

        let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("traderoute_params"),
            contents: bytemuck::bytes_of(&TradeRouteParams::default()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("traderoute_bgl"),
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
            label: Some("traderoute_bg"),
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

        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("traderoute_vb"),
            size: 4096,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("traderoute_pl"),
            bind_group_layouts: &[&bgl],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("traderoute_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<TradeRouteVertex>() as u64,
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
                        wgpu::VertexAttribute {
                            offset: 20,
                            shader_location: 2,
                            format: wgpu::VertexFormat::Float32,
                        }, // trade_amount
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
                    constant: 6,
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
            vertex_capacity: 256,
            vertex_count: 0,
            params_buffer,
            params_bind_group,
            any_loaded: false,
            load_warnings: Vec::new(),
        }
    }

    /// Upload trade route vertices for this frame.
    pub fn set_routes(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        vertices: &[TradeRouteVertex],
    ) {
        if vertices.is_empty() {
            self.vertex_count = 0;
            self.any_loaded = false;
            return;
        }
        self.any_loaded = true;
        self.vertex_count = vertices.len() as u32;

        if vertices.len() as u32 > self.vertex_capacity {
            self.vertex_capacity = (vertices.len() as u32).next_power_of_two();
            self.vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("traderoute_vb"),
                size: (self.vertex_capacity as usize * std::mem::size_of::<TradeRouteVertex>())
                    as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }

        queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(vertices));
    }

    pub fn update_params(&self, queue: &wgpu::Queue, params: &TradeRouteParams) {
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

impl super::Pass for TradeRoutePass {
    fn name(&self) -> &'static str {
        "traderoute"
    }
}

// ─── Mesh generation ───────────────────────────────────────────────────────

/// Generate a strip mesh along a path (list of world XYZ points).
/// Returns triangle-list vertices with UV for flowing dash effect.
fn generate_route_strip(
    path: &[[f32; 3]],
    width: f32,
    amount: f32,
    height_lift: f32,
) -> Vec<TradeRouteVertex> {
    if path.len() < 2 {
        return Vec::new();
    }

    let mut verts = Vec::new();

    for i in 0..path.len() - 1 {
        let p0 = path[i];
        let p1 = path[i + 1];
        let dir = [p1[0] - p0[0], p1[2] - p0[2]];
        let len = (dir[0] * dir[0] + dir[1] * dir[1]).sqrt();
        if len < 0.0001 {
            continue;
        }
        let perp = [-dir[1] / len * width, dir[0] / len * width];

        let u_start = i as f32 / (path.len() - 1) as f32;
        let u_end = (i + 1) as f32 / (path.len() - 1) as f32;

        // Two triangles forming a quad
        verts.push(TradeRouteVertex {
            world_pos: [p0[0] + perp[0], p0[1] + height_lift, p0[2] + perp[1]],
            uv: [u_start, 0.0],
            trade_amount: amount,
            _pad: 0.0,
        });
        verts.push(TradeRouteVertex {
            world_pos: [p0[0] - perp[0], p0[1] + height_lift, p0[2] - perp[1]],
            uv: [u_start, 1.0],
            trade_amount: amount,
            _pad: 0.0,
        });
        verts.push(TradeRouteVertex {
            world_pos: [p1[0] + perp[0], p1[1] + height_lift, p1[2] + perp[1]],
            uv: [u_end, 0.0],
            trade_amount: amount,
            _pad: 0.0,
        });

        verts.push(TradeRouteVertex {
            world_pos: [p0[0] - perp[0], p0[1] + height_lift, p0[2] - perp[1]],
            uv: [u_start, 1.0],
            trade_amount: amount,
            _pad: 0.0,
        });
        verts.push(TradeRouteVertex {
            world_pos: [p1[0] - perp[0], p1[1] + height_lift, p1[2] - perp[1]],
            uv: [u_end, 1.0],
            trade_amount: amount,
            _pad: 0.0,
        });
        verts.push(TradeRouteVertex {
            world_pos: [p1[0] + perp[0], p1[1] + height_lift, p1[2] + perp[1]],
            uv: [u_end, 0.0],
            trade_amount: amount,
            _pad: 0.0,
        });
    }

    verts
}

/// Generate hardcoded trade routes for pass geometry tests.
#[cfg(test)]
pub fn generate_mock_trade_routes() -> Vec<TradeRouteVertex> {
    let s = 0.02; // WORLD_SCALE

    let mut all_verts = Vec::new();

    // Route 1: USA → UK (transatlantic)
    let usa_cap = [30.0 * s, 0.25, 52.0 * s];
    let uk_cap = [34.0 * s, 0.25, 50.0 * s];
    // Simple curved path across ocean
    let mid = [(usa_cap[0] + uk_cap[0]) * 0.5, 0.25, usa_cap[2] + 0.05];
    let path = vec![usa_cap, mid, uk_cap];
    all_verts.extend(generate_route_strip(&path, 0.005, 0.8, 0.0));

    // Route 2: GER → SOV (east-west)
    let ger_cap = [56.0 * s, 0.25, 40.0 * s];
    let sov_cap = [80.0 * s, 0.25, 38.0 * s];
    let path2 = vec![ger_cap, sov_cap];
    all_verts.extend(generate_route_strip(&path2, 0.005, 0.5, 0.0));

    // Route 3: ITA → FRA
    let ita_cap = [48.0 * s, 0.25, 30.0 * s];
    let fra_cap = [38.0 * s, 0.25, 42.0 * s];
    let path3 = vec![ita_cap, fra_cap];
    all_verts.extend(generate_route_strip(&path3, 0.004, 0.3, 0.0));

    all_verts
}

/// Build trade-route strip vertices from the simulation's real route store.
pub fn generate_trade_route_vertices(
    world: &hoi4_state::World,
    centroids: &[(f32, f32)],
    world_scale: f32,
    height_scale: f32,
) -> Vec<TradeRouteVertex> {
    let mut all_verts = Vec::new();
    let max_throughput = world
        .countries
        .trade
        .routes
        .iter()
        .filter(|route| !route.is_blockaded)
        .map(|route| route.throughput.max(0.0))
        .fold(0.0_f32, f32::max)
        .max(1.0);

    for route in &world.countries.trade.routes {
        if route.is_blockaded || route.throughput <= 0.0 {
            continue;
        }
        let Some(importer) =
            country_capital_pos(world, route.importer, centroids, world_scale, height_scale)
        else {
            continue;
        };
        let Some(exporter) =
            country_capital_pos(world, route.exporter, centroids, world_scale, height_scale)
        else {
            continue;
        };

        let amount = (route.throughput / max_throughput).clamp(0.15, 1.0);
        let width = 0.004 + amount * 0.0035;
        let lift = match route.kind {
            hoi4_state::TradeRouteKind::Land => 0.035,
            hoi4_state::TradeRouteKind::Transit => 0.04,
            hoi4_state::TradeRouteKind::Sea | hoi4_state::TradeRouteKind::ImperialPreference => {
                0.055
            }
        };

        let mut path = Vec::with_capacity(3);
        path.push(importer);
        if let Some(port_state) = route.port_state {
            if let Some(port) = state_pos(world, port_state, centroids, world_scale, height_scale) {
                if distance_xz(importer, port) > 0.001 && distance_xz(port, exporter) > 0.001 {
                    path.push(port);
                }
            }
        } else if route.kind.uses_sea_lanes() {
            path.push(curve_midpoint(importer, exporter, height_scale));
        }
        path.push(exporter);

        all_verts.extend(generate_route_strip(&path, width, amount, lift));
    }

    all_verts
}

fn country_capital_pos(
    world: &hoi4_state::World,
    country: hoi4_state::CountryId,
    centroids: &[(f32, f32)],
    world_scale: f32,
    height_scale: f32,
) -> Option<[f32; 3]> {
    if country.is_none() {
        return None;
    }
    if let Some(&state) = world.countries.capitals.get(country.0 as usize) {
        if let Some(pos) = state_pos(world, state, centroids, world_scale, height_scale) {
            return Some(pos);
        }
    }
    world
        .states
        .owners
        .iter()
        .enumerate()
        .find_map(|(idx, &owner)| {
            (owner == country).then(|| {
                state_pos(
                    world,
                    hoi4_state::StateId(idx as u16),
                    centroids,
                    world_scale,
                    height_scale,
                )
            })?
        })
}

fn state_pos(
    world: &hoi4_state::World,
    state: hoi4_state::StateId,
    centroids: &[(f32, f32)],
    world_scale: f32,
    height_scale: f32,
) -> Option<[f32; 3]> {
    if state.is_none() {
        return None;
    }
    let provinces = world.states.provinces.get(state.0 as usize)?;
    let mut sx = 0.0;
    let mut sy = 0.0;
    let mut n = 0.0;
    for province in provinces {
        let idx = province.0 as usize;
        let Some(&(px, py)) = centroids.get(idx) else {
            continue;
        };
        if px == 0.0 && py == 0.0 {
            continue;
        }
        sx += px;
        sy += py;
        n += 1.0;
    }
    if n <= 0.0 {
        return None;
    }
    let px = sx / n;
    let py = sy / n;
    Some([
        px * world_scale,
        sample_height(world, px, py, height_scale),
        py * world_scale,
    ])
}

fn sample_height(world: &hoi4_state::World, px: f32, py: f32, height_scale: f32) -> f32 {
    let hmap = &world.map.heightmap;
    if hmap.width == 0 || hmap.height == 0 || hmap.pixels.is_empty() {
        return 0.0;
    }
    let x = px.clamp(0.0, hmap.width.saturating_sub(1) as f32) as u32;
    let y = py.clamp(0.0, hmap.height.saturating_sub(1) as f32) as u32;
    hmap.pixels[(y * hmap.width + x) as usize] as f32 / 255.0 * height_scale
}

fn curve_midpoint(a: [f32; 3], b: [f32; 3], height_scale: f32) -> [f32; 3] {
    let dx = b[0] - a[0];
    let dz = b[2] - a[2];
    let len = (dx * dx + dz * dz).sqrt().max(0.0001);
    let bend = (len * 0.18).clamp(0.04, 0.45);
    [
        (a[0] + b[0]) * 0.5 - dz / len * bend,
        ((a[1] + b[1]) * 0.5).max(height_scale * 95.0 / 255.0),
        (a[2] + b[2]) * 0.5 + dx / len * bend,
    ]
}

fn distance_xz(a: [f32; 3], b: [f32; 3]) -> f32 {
    let dx = b[0] - a[0];
    let dz = b[2] - a[2];
    (dx * dx + dz * dz).sqrt()
}

// ─── Inline WGSL ──────────────────────────────────────────────────────────

const TRADEROUTE_WGSL: &str = r#"
// traderoute.wgsl — Procedural flowing dashed trade route lines.

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

struct TradeParams {
    flow_speed: f32,
    opacity: f32,
    _pad0_0: f32,
    _pad0_1: f32,
    color_a: vec4<f32>,
    color_b: vec4<f32>,
    _pad1: vec4<f32>,
    _pad2: vec4<f32>,
};

@group(0) @binding(0) var<uniform> frame: GlobalFrameUniform;
@group(0) @binding(1) var<uniform> t_params: TradeParams;

struct VsIn {
    @location(0) world_pos: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) trade_amount: f32,
};

struct VsOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) amount: f32,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var out: VsOut;
    out.clip_pos = frame.view_proj * vec4<f32>(in.world_pos, 1.0);
    // Z-bias
    out.clip_pos.z -= 0.004 * out.clip_pos.w;
    // Flowing UV: scroll along path direction
    out.uv = vec2<f32>(in.uv.x - frame.global_time * t_params.flow_speed, in.uv.y);
    out.amount = in.trade_amount;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // Procedural dashed line: repeat along UV.x, fade at edges (UV.y)
    let dash = fract(in.uv.x * 8.0);
    let dash_pattern = step(0.3, dash) * step(dash, 0.7);
    let edge_fade = 1.0 - smoothstep(0.3, 0.5, abs(in.uv.y * 2.0 - 1.0));

    let alpha = dash_pattern * edge_fade;
    if (alpha < 0.05) {
        discard;
    }

    let color = mix(t_params.color_a.rgb, t_params.color_b.rgb, in.amount);
    return vec4<f32>(color, alpha * t_params.color_a.a * t_params.opacity);
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trade_route_params_size_is_80() {
        assert_eq!(std::mem::size_of::<TradeRouteParams>(), 80);
    }

    #[test]
    fn trade_route_vertex_size_is_28() {
        assert_eq!(std::mem::size_of::<TradeRouteVertex>(), 28);
    }

    #[test]
    fn mock_trade_routes_generated() {
        let verts = generate_mock_trade_routes();
        assert!(!verts.is_empty());
        // 3 routes × 2 segments avg × 6 verts/tri = ~36
        assert!(verts.len() >= 18);
    }

    #[test]
    fn route_strip_simple_path() {
        let path = vec![[0.0, 0.2, 0.0], [1.0, 0.3, 0.0]];
        let verts = generate_route_strip(&path, 0.05, 0.5, 0.05);
        assert_eq!(verts.len(), 6); // 1 quad = 2 triangles = 6 verts
        assert!((verts[0].world_pos[1] - 0.25).abs() < 1e-5);
    }
}
