//! V5 收口（2026-05-18）：菜单背景面板 SDF 渲染（slim 版）。
//!
//! V3 时代 `panel_pass.rs`（431 行）作为 4.3 政治面板的"通用 panel 基础"实现，
//! 提供圆角矩形 / 边框 / 阴影 / 渐变。V5 §3.2 删除 V3 4.3 政治面板路线后，本
//! 模块缩水为纯 [`crate::menu_pass`] 支撑层：相同 API 表面，但仅保留 menu_pass
//! 实际使用的几个构造器和最小的 SDF 渲染管线。
//!
//! 阶段 B（hoi4-ui + egui）会把整个菜单迁出，这时本文件可一并删除。

use wgpu::util::DeviceExt;

/// 一个待渲染的圆角矩形面板。所有字段保留 V3 时代命名以兼容 [`crate::menu_pass`]。
#[derive(Debug, Clone, Copy)]
pub struct Panel {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub radius: f32,
    pub fill_top: [f32; 4],
    pub fill_bottom: [f32; 4],
    pub border: [f32; 4],
    pub border_width: f32,
    pub shadow_offset: f32,
    pub shadow_alpha: f32,
}

impl Panel {
    pub fn solid(x: f32, y: f32, w: f32, h: f32, fill: [f32; 4]) -> Self {
        Self {
            x,
            y,
            w,
            h,
            radius: 0.0,
            fill_top: fill,
            fill_bottom: fill,
            border: [0.0; 4],
            border_width: 0.0,
            shadow_offset: 0.0,
            shadow_alpha: 0.0,
        }
    }

    pub fn card(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self {
            x,
            y,
            w,
            h,
            radius: 6.0,
            fill_top: [0.05, 0.05, 0.05, 0.78],
            fill_bottom: [0.02, 0.02, 0.02, 0.85],
            border: [0.85, 0.78, 0.55, 0.30],
            border_width: 1.0,
            shadow_offset: 6.0,
            shadow_alpha: 0.45,
        }
    }

    pub fn header_underline(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self {
            x,
            y,
            w,
            h,
            radius: 0.0,
            fill_top: [0.85, 0.74, 0.42, 0.85],
            fill_bottom: [0.85, 0.74, 0.42, 0.85],
            border: [0.0; 4],
            border_width: 0.0,
            shadow_offset: 0.0,
            shadow_alpha: 0.0,
        }
    }

    pub fn left_gradient_mask(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self {
            x,
            y,
            w,
            h,
            radius: 0.0,
            fill_top: [0.0, 0.0, 0.0, 0.78],
            fill_bottom: [0.0, 0.0, 0.0, 0.78],
            border: [0.0; 4],
            border_width: 0.0,
            shadow_offset: 0.0,
            shadow_alpha: 0.0,
        }
    }

    pub fn full_screen_dim(screen_w: f32, screen_h: f32, alpha: f32) -> Self {
        Self::solid(0.0, 0.0, screen_w, screen_h, [0.0, 0.0, 0.0, alpha])
    }

    pub fn hover_highlight(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self {
            x,
            y,
            w,
            h,
            radius: 4.0,
            fill_top: [0.85, 0.78, 0.55, 0.18],
            fill_bottom: [0.85, 0.78, 0.55, 0.10],
            border: [0.85, 0.78, 0.55, 0.55],
            border_width: 1.0,
            shadow_offset: 0.0,
            shadow_alpha: 0.0,
        }
    }

    /// V3 时代调试覆盖用过；保留以兼容 main.rs 残留 import（V5 已不主动调用）。
    pub fn debug_outline(x: f32, y: f32, w: f32, h: f32, color: [f32; 4]) -> Self {
        Self {
            x,
            y,
            w,
            h,
            radius: 0.0,
            fill_top: [0.0; 4],
            fill_bottom: [0.0; 4],
            border: color,
            border_width: 1.0,
            shadow_offset: 0.0,
            shadow_alpha: 0.0,
        }
    }
}

/// 4 顶点 instance（每 Panel 展开为一个 quad）。
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct PanelInstance {
    rect: [f32; 4], // x, y, w, h
    fill_top: [f32; 4],
    fill_bottom: [f32; 4],
    border: [f32; 4],
    radius_borderw_shadow: [f32; 4], // radius, border_width, shadow_offset, shadow_alpha
}

const PANEL_SHADER: &str = r#"
struct Uniforms {
    screen: vec2<f32>,
    _pad: vec2<f32>,
};
@group(0) @binding(0) var<uniform> u: Uniforms;

struct Inst {
    @location(0) rect: vec4<f32>,
    @location(1) fill_top: vec4<f32>,
    @location(2) fill_bottom: vec4<f32>,
    @location(3) border: vec4<f32>,
    @location(4) rbsa: vec4<f32>, // radius, border_w, shadow_off, shadow_a
};

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) local: vec2<f32>,   // 0..rect.zw
    @location(1) size: vec2<f32>,    // rect.zw
    @location(2) fill_top: vec4<f32>,
    @location(3) fill_bottom: vec4<f32>,
    @location(4) border: vec4<f32>,
    @location(5) rbsa: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vid: u32, inst: Inst) -> VsOut {
    // Quad corners (CCW)
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0), vec2<f32>(0.0, 1.0),
    );
    let c = corners[vid];
    // Expand quad slightly for shadow halo.
    let shadow_pad = max(inst.rbsa.z, 0.0) * 1.5;
    let pos = inst.rect.xy + c * (inst.rect.zw + vec2<f32>(shadow_pad * 2.0, shadow_pad * 2.0)) - vec2<f32>(shadow_pad, shadow_pad);
    let local = c * (inst.rect.zw + vec2<f32>(shadow_pad * 2.0, shadow_pad * 2.0)) - vec2<f32>(shadow_pad, shadow_pad);

    let ndc_x = (pos.x / u.screen.x) * 2.0 - 1.0;
    let ndc_y = 1.0 - (pos.y / u.screen.y) * 2.0;

    var o: VsOut;
    o.clip = vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);
    o.local = local;
    o.size = inst.rect.zw;
    o.fill_top = inst.fill_top;
    o.fill_bottom = inst.fill_bottom;
    o.border = inst.border;
    o.rbsa = inst.rbsa;
    return o;
}

// SDF for rounded rect centred at (size/2) with half-extent (size/2 - r), corner r.
fn sdf_rrect(p: vec2<f32>, half: vec2<f32>, r: f32) -> f32 {
    let q = abs(p) - half + vec2<f32>(r, r);
    return min(max(q.x, q.y), 0.0) + length(max(q, vec2<f32>(0.0, 0.0))) - r;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let r = in.rbsa.x;
    let bw = in.rbsa.y;
    let shadow_off = in.rbsa.z;
    let shadow_a = in.rbsa.w;

    let centre = in.size * 0.5;
    let half = centre;
    let p = in.local - centre;
    let d = sdf_rrect(p, half, r);

    // Shadow contribution (blurred, offset down-right).
    var shadow = 0.0;
    if (shadow_a > 0.001) {
        let sp = (in.local - vec2<f32>(shadow_off, shadow_off)) - centre;
        let sd = sdf_rrect(sp, half, r);
        shadow = clamp(1.0 - sd / (shadow_off * 1.5 + 0.5), 0.0, 1.0) * shadow_a;
    }

    // Antialiased panel coverage.
    let cov = clamp(0.5 - d, 0.0, 1.0);

    // Vertical gradient between fill_top / fill_bottom.
    let t = clamp(in.local.y / max(in.size.y, 1.0), 0.0, 1.0);
    let fill = mix(in.fill_top, in.fill_bottom, t);

    // Composite shadow first then panel.
    var col = vec4<f32>(0.0, 0.0, 0.0, shadow * (1.0 - cov));

    if (cov > 0.001) {
        var rgba = fill;
        rgba.a = rgba.a * cov;

        // Border ring (between -bw and 0 SDF).
        if (bw > 0.0) {
            let edge = clamp(0.5 + (d + bw), 0.0, 1.0);
            rgba = mix(rgba, in.border, edge * in.border.a);
        }
        // Over composite shadow.
        let one_minus = 1.0 - rgba.a;
        col = vec4<f32>(rgba.rgb * rgba.a + col.rgb * one_minus, rgba.a + col.a * one_minus);
    }

    if (col.a < 0.002) { discard; }
    return col;
}
"#;

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct PanelUniform {
    screen: [f32; 2],
    _pad: [f32; 2],
}

/// 圆角矩形面板渲染 pass。
pub struct PanelPass {
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    uniform_buf: wgpu::Buffer,
    instance_buf: wgpu::Buffer,
    capacity: u32,
    instances: Vec<PanelInstance>,
}

impl PanelPass {
    pub fn new(
        device: &wgpu::Device,
        target_format: wgpu::TextureFormat,
        screen_w: f32,
        screen_h: f32,
    ) -> Self {
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("panel_bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("panel_pl"),
            bind_group_layouts: &[&bgl],
            push_constant_ranges: &[],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("panel_shader"),
            source: wgpu::ShaderSource::Wgsl(PANEL_SHADER.into()),
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("panel_pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<PanelInstance>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array![
                        0 => Float32x4,
                        1 => Float32x4,
                        2 => Float32x4,
                        3 => Float32x4,
                        4 => Float32x4,
                    ],
                }],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview: None,
            cache: None,
        });

        let uniform_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("panel_uniform"),
            contents: bytemuck::bytes_of(&PanelUniform {
                screen: [screen_w, screen_h],
                _pad: [0.0, 0.0],
            }),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("panel_bg"),
            layout: &bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buf.as_entire_binding(),
            }],
        });

        const INITIAL_CAPACITY: u32 = 256;
        let instance_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("panel_instance_buf"),
            size: (INITIAL_CAPACITY as usize * std::mem::size_of::<PanelInstance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            bind_group,
            uniform_buf,
            instance_buf,
            capacity: INITIAL_CAPACITY,
            instances: Vec::new(),
        }
    }

    pub fn clear(&mut self) {
        self.instances.clear();
    }

    pub fn push(&mut self, p: Panel) {
        self.instances.push(PanelInstance {
            rect: [p.x, p.y, p.w, p.h],
            fill_top: p.fill_top,
            fill_bottom: p.fill_bottom,
            border: p.border,
            radius_borderw_shadow: [p.radius, p.border_width, p.shadow_offset, p.shadow_alpha],
        });
    }

    pub fn update_screen_size(&mut self, queue: &wgpu::Queue, w: f32, h: f32) {
        queue.write_buffer(
            &self.uniform_buf,
            0,
            bytemuck::bytes_of(&PanelUniform {
                screen: [w, h],
                _pad: [0.0, 0.0],
            }),
        );
    }

    pub fn prepare(&mut self, queue: &wgpu::Queue) {
        if self.instances.is_empty() {
            return;
        }
        let count = self.instances.len() as u32;
        let needed_bytes = (count as usize) * std::mem::size_of::<PanelInstance>();
        if count > self.capacity {
            // Buffer not large enough — silently truncate to current capacity to
            // avoid reallocation in the middle of frame. main loop's capacity
            // (256) covers menu draws comfortably.
            let truncated = self.capacity as usize;
            queue.write_buffer(
                &self.instance_buf,
                0,
                bytemuck::cast_slice(&self.instances[..truncated]),
            );
        } else {
            queue.write_buffer(
                &self.instance_buf,
                0,
                bytemuck::cast_slice(&self.instances[..]),
            );
            let _ = needed_bytes;
        }
    }

    pub fn render<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>) {
        if self.instances.is_empty() {
            return;
        }
        let count = (self.instances.len() as u32).min(self.capacity);
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_vertex_buffer(0, self.instance_buf.slice(..));
        pass.draw(0..6, 0..count);
    }
}
