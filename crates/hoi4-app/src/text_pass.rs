//! Phase 3.5：文字渲染 pass（V5 收口版）。
//!
//! 用 fontdue 在 UI 层画文字，每个字符是一个 textured quad。V3 时代的 vanilla
//! BM-font-removed (.fnt + DDS atlas) fallback 已在 V5（2026-05-18）按 `ROADMAP_V5.md`
//! §4.4 移除。系统字体 (`msyh` / Segoe UI) 通过 fontdue 走唯一一条路径。
//!
//! 与 `menu_pass.rs` / `passes::*` 共享同一个 2D 正交投影 pipeline。

use hoi4_paths::PathConfig;
use wgpu::util::DeviceExt;

use super::glyphon_text::FontdueAtlas;

/// 单个字符 quad 的顶点。
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct GlyphVertex {
    pos: [f32; 2],
    uv: [f32; 2],
}

/// 一段要渲染的文字。
pub struct TextDraw {
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub color: [f32; 4],
    /// 对齐方式：left / center / right。配合 max_width 使用。
    pub align: TextAlign,
    /// 文字区域最大宽度（用于 center/right 对齐）。0 = 不限制。
    pub max_width: f32,
    /// Phase 4.2 (redesign): 字号档（决定用哪个 atlas）。
    pub size: TextSize,
}

/// 文字对齐方式。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TextAlign {
    Left,
    Center,
    Right,
}

/// Phase 4.2 (redesign): 字号档。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextSize {
    /// 主标题（48px）— 用于 "HEARTS OF IRON IV" 等
    Title,
    /// 区段标题（28px）— 用于 "MANPOWER" / "OVERVIEW" / 国名等
    Heading,
    /// 正文（20px）— 默认。
    Body,
}

#[derive(Debug, Clone, Copy)]
enum AtlasSlot {
    Body,
    Heading,
    Title,
}

/// 文字渲染器。持有 font atlas texture + pipeline。
pub struct TextPass {
    pipeline: wgpu::RenderPipeline,
    bind_group: Option<wgpu::BindGroup>,
    /// Phase 4.2 (redesign): 大字号 atlas + bind group（标题用）。
    bind_group_title: Option<wgpu::BindGroup>,
    bind_group_heading: Option<wgpu::BindGroup>,
    vertex_buffer: wgpu::Buffer,
    /// Phase 4.2 (redesign): 各 size 各自独立 vertex buffer。
    vertex_buffer_title: wgpu::Buffer,
    vertex_buffer_heading: wgpu::Buffer,
    max_chars: u32,
    /// Phase 3.8: fontdue-based glyph atlas（Body 字号，V5 起为唯一字体路径）。
    fontdue: Option<FontdueAtlas>,
    fontdue_texture: Option<wgpu::Texture>,
    /// Phase 4.2 (redesign): 大字号 atlas（标题）。
    fontdue_title: Option<FontdueAtlas>,
    fontdue_title_texture: Option<wgpu::Texture>,
    /// Phase 4.2 (redesign): 中字号 atlas（区段标题）。
    fontdue_heading: Option<FontdueAtlas>,
    fontdue_heading_texture: Option<wgpu::Texture>,
    /// Tracks whether fontdue atlas needs re-upload this frame.
    fontdue_dirty: bool,
    ///待渲染的文字列表（每帧重建）。
    draws: Vec<TextDraw>,
    /// 本帧生成的顶点数。
    vertex_count: u32,
    /// 各 size 的顶点数。
    vertex_count_title: u32,
    vertex_count_heading: u32,
    /// 4.1.bis.6 fix: per-slot uniform buffers so `set_screen_size` actually
    /// reaches the GPU. Previously each `upload_atlas` call created an
    /// anonymous local `uniform_buf` whose handle was dropped into the bind
    /// group only — meaning the only path to update screen size was to
    /// recreate the bind group, which we never did on resize.
    uniform_buffer_body: Option<wgpu::Buffer>,
    uniform_buffer_heading: Option<wgpu::Buffer>,
    uniform_buffer_title: Option<wgpu::Buffer>,
    screen_width: f32,
    screen_height: f32,
}

impl TextPass {
    /// 4.1.bis.6 fix (2026-05-16): also write the new size to all three GPU
    /// uniform buffers. Previously this only updated Rust fields → shaders kept
    /// using whatever screen size was alive when the atlas was uploaded, so
    /// every text quad drifted on resize / DPI mismatch.
    pub fn set_screen_size(&mut self, queue: &wgpu::Queue, w: f32, h: f32) {
        self.screen_width = w;
        self.screen_height = h;
        let size = [w, h];
        let bytes = bytemuck::bytes_of(&size);
        if let Some(b) = self.uniform_buffer_body.as_ref() {
            queue.write_buffer(b, 0, bytes);
        }
        if let Some(b) = self.uniform_buffer_heading.as_ref() {
            queue.write_buffer(b, 0, bytes);
        }
        if let Some(b) = self.uniform_buffer_title.as_ref() {
            queue.write_buffer(b, 0, bytes);
        }
    }
}

const TEXT_SHADER: &str = r#"
struct Uniforms {
    screen_size: vec2<f32>,
};
@group(0) @binding(0) var<uniform> uniforms: Uniforms;
@group(0) @binding(1) var atlas_tex: texture_2d<f32>;
@group(0) @binding(2) var atlas_sampler: sampler;

struct VsIn {
    @location(0) pos: vec2<f32>,
    @location(1) uv: vec2<f32>,
};
struct VsOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var out: VsOut;
    let ndc_x = (in.pos.x / uniforms.screen_size.x) * 2.0 - 1.0;
    let ndc_y = 1.0 - (in.pos.y / uniforms.screen_size.y) * 2.0;
    out.clip_pos = vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);
    out.uv = in.uv;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let sample = textureSample(atlas_tex, atlas_sampler, in.uv);
    // V5 收口：所有字体走 fontdue R8 atlas，glyph coverage 在 red 通道。
    let a = sample.r;
    if (a < 0.02) {
        discard;
    }
    return vec4<f32>(0.95, 0.93, 0.85, a);
}
"#;

impl TextPass {
    pub fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        screen_width: f32,
        screen_height: f32,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("text_shader"),
            source: wgpu::ShaderSource::Wgsl(TEXT_SHADER.into()),
        });

        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("text_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&bgl],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("text_pipeline"),
            layout: Some(&pl),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<GlyphVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
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
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });

        let max_chars = 512u32;
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("text_verts"),
            size: (max_chars as u64) * 6 * std::mem::size_of::<GlyphVertex>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let vertex_buffer_title = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("text_verts_title"),
            size: 64 * 6 * std::mem::size_of::<GlyphVertex>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let vertex_buffer_heading = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("text_verts_heading"),
            size: 256 * 6 * std::mem::size_of::<GlyphVertex>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            bind_group: None,
            bind_group_title: None,
            bind_group_heading: None,
            vertex_buffer,
            vertex_buffer_title,
            vertex_buffer_heading,
            max_chars,
            fontdue: None,
            fontdue_texture: None,
            fontdue_title: None,
            fontdue_title_texture: None,
            fontdue_heading: None,
            fontdue_heading_texture: None,
            fontdue_dirty: false,
            draws: Vec::new(),
            vertex_count: 0,
            vertex_count_title: 0,
            vertex_count_heading: 0,
            uniform_buffer_body: None,
            uniform_buffer_heading: None,
            uniform_buffer_title: None,
            screen_width,
            screen_height,
        }
    }

    /// 加载字体 atlas。在 init_render 后调用一次。
    ///
    /// V5 收口：唯一字体路径走 fontdue 系统字体。Body 20 px / Heading 28 px /
    /// Title 48 px 三档，分别上传到独立 atlas。
    pub fn load_font(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _path_cfg: &PathConfig,
        bgl: &wgpu::BindGroupLayout,
    ) {
        // Title atlas（48 px）— 主菜单标题等。
        if let Some(mut title) = FontdueAtlas::from_system_font(48.0) {
            for ch in ' '..='~' {
                title.get_glyph(ch);
            }
            self.upload_atlas(device, queue, bgl, title, AtlasSlot::Title);
        }
        // Heading atlas（28 px）— 区段标题 / 国名。
        if let Some(mut heading) = FontdueAtlas::from_system_font(28.0) {
            for ch in ' '..='~' {
                heading.get_glyph(ch);
            }
            self.upload_atlas(device, queue, bgl, heading, AtlasSlot::Heading);
        }
        // Body atlas（20 px）— 正文默认。
        if let Some(mut body) = FontdueAtlas::from_system_font(20.0) {
            for ch in ' '..='~' {
                body.get_glyph(ch);
            }
            self.upload_atlas(device, queue, bgl, body, AtlasSlot::Body);
        }
    }

    /// Phase 4.2 (redesign): 上传一个 fontdue atlas 到对应 slot。
    fn upload_atlas(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bgl: &wgpu::BindGroupLayout,
        atlas: FontdueAtlas,
        slot: AtlasSlot,
    ) {
        let label = match slot {
            AtlasSlot::Body => "fontdue_atlas_body",
            AtlasSlot::Heading => "fontdue_atlas_heading",
            AtlasSlot::Title => "fontdue_atlas_title",
        };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: atlas.atlas_width,
                height: atlas.atlas_height,
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
            atlas.atlas_bytes(),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(atlas.atlas_width),
                rows_per_image: None,
            },
            wgpu::Extent3d {
                width: atlas.atlas_width,
                height: atlas.atlas_height,
                depth_or_array_layers: 1,
            },
        );
        let view = texture.create_view(&Default::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let uniform_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("text_uniforms"),
            contents: bytemuck::bytes_of(&[self.screen_width, self.screen_height]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(label),
            layout: bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        match slot {
            AtlasSlot::Body => {
                self.bind_group = Some(bind_group);
                self.fontdue_texture = Some(texture);
                self.fontdue = Some(atlas);
                // 4.1.bis.6 fix: keep buffer so set_screen_size can update it.
                self.uniform_buffer_body = Some(uniform_buf);
            }
            AtlasSlot::Heading => {
                self.bind_group_heading = Some(bind_group);
                self.fontdue_heading_texture = Some(texture);
                self.fontdue_heading = Some(atlas);
                self.uniform_buffer_heading = Some(uniform_buf);
            }
            AtlasSlot::Title => {
                self.bind_group_title = Some(bind_group);
                self.fontdue_title_texture = Some(texture);
                self.fontdue_title = Some(atlas);
                self.uniform_buffer_title = Some(uniform_buf);
            }
        }
        self.fontdue_dirty = false;
    }

    /// 每帧开始时清空文字列表。
    pub fn clear(&mut self) {
        self.draws.clear();
        self.vertex_count = 0;
        self.vertex_count_title = 0;
        self.vertex_count_heading = 0;
    }

    /// 添加一段文字（左对齐，正文字号）。
    pub fn draw_text(&mut self, text: &str, x: f32, y: f32) {
        self.draws.push(TextDraw {
            text: text.to_string(),
            x,
            y,
            color: [1.0, 1.0, 1.0, 1.0],
            align: TextAlign::Left,
            max_width: 0.0,
            size: TextSize::Body,
        });
    }

    /// 添加一段文字（指定对齐方式和区域宽度，正文字号）。
    pub fn draw_text_aligned(
        &mut self,
        text: &str,
        x: f32,
        y: f32,
        align: TextAlign,
        max_width: f32,
    ) {
        self.draws.push(TextDraw {
            text: text.to_string(),
            x,
            y,
            color: [1.0, 1.0, 1.0, 1.0],
            align,
            max_width,
            size: TextSize::Body,
        });
    }

    /// Phase 4.2 (redesign): 添加一段文字，指定字号档。
    pub fn draw_text_sized(
        &mut self,
        text: &str,
        x: f32,
        y: f32,
        align: TextAlign,
        max_width: f32,
        size: TextSize,
    ) {
        self.draws.push(TextDraw {
            text: text.to_string(),
            x,
            y,
            color: [1.0, 1.0, 1.0, 1.0],
            align,
            max_width,
            size,
        });
    }

    /// 生成顶点数据写入 GPU buffer。在 render pass 之前调用。
    ///
    /// V5 收口：所有字号统一走 fontdue atlas，按 `TextSize` 分组到三个独立
    /// vertex buffer。
    pub fn prepare(&mut self, queue: &wgpu::Queue) {
        self.vertex_count = 0;
        self.vertex_count_title = 0;
        self.vertex_count_heading = 0;
        if self.draws.is_empty() {
            return;
        }

        // 按 size 分组
        let mut body_draws: Vec<&TextDraw> = Vec::new();
        let mut heading_draws: Vec<&TextDraw> = Vec::new();
        let mut title_draws: Vec<&TextDraw> = Vec::new();
        for d in &self.draws {
            match d.size {
                TextSize::Body => body_draws.push(d),
                TextSize::Heading => heading_draws.push(d),
                TextSize::Title => title_draws.push(d),
            }
        }

        // ── Body ──
        if !body_draws.is_empty() {
            if let Some(atlas) = self.fontdue.as_mut() {
                let (verts, dirty) = build_glyph_verts(atlas, &body_draws);
                if dirty {
                    if let Some(tex) = self.fontdue_texture.as_ref() {
                        queue.write_texture(
                            wgpu::TexelCopyTextureInfo {
                                texture: tex,
                                mip_level: 0,
                                origin: wgpu::Origin3d::ZERO,
                                aspect: wgpu::TextureAspect::All,
                            },
                            atlas.atlas_bytes(),
                            wgpu::TexelCopyBufferLayout {
                                offset: 0,
                                bytes_per_row: Some(atlas.atlas_width),
                                rows_per_image: None,
                            },
                            wgpu::Extent3d {
                                width: atlas.atlas_width,
                                height: atlas.atlas_height,
                                depth_or_array_layers: 1,
                            },
                        );
                    }
                }
                let max = (self.max_chars as usize) * 6;
                let count = verts.len().min(max);
                self.vertex_count = count as u32;
                if count > 0 {
                    queue.write_buffer(
                        &self.vertex_buffer,
                        0,
                        bytemuck::cast_slice(&verts[..count]),
                    );
                }
            }
        }

        // ── Heading ──
        if !heading_draws.is_empty() {
            if let Some(atlas) = self.fontdue_heading.as_mut() {
                let (verts, dirty) = build_glyph_verts(atlas, &heading_draws);
                if dirty {
                    if let Some(tex) = self.fontdue_heading_texture.as_ref() {
                        queue.write_texture(
                            wgpu::TexelCopyTextureInfo {
                                texture: tex,
                                mip_level: 0,
                                origin: wgpu::Origin3d::ZERO,
                                aspect: wgpu::TextureAspect::All,
                            },
                            atlas.atlas_bytes(),
                            wgpu::TexelCopyBufferLayout {
                                offset: 0,
                                bytes_per_row: Some(atlas.atlas_width),
                                rows_per_image: None,
                            },
                            wgpu::Extent3d {
                                width: atlas.atlas_width,
                                height: atlas.atlas_height,
                                depth_or_array_layers: 1,
                            },
                        );
                    }
                }
                let max = 256 * 6;
                let count = verts.len().min(max);
                self.vertex_count_heading = count as u32;
                if count > 0 {
                    queue.write_buffer(
                        &self.vertex_buffer_heading,
                        0,
                        bytemuck::cast_slice(&verts[..count]),
                    );
                }
            }
        }

        // ── Title ──
        if !title_draws.is_empty() {
            if let Some(atlas) = self.fontdue_title.as_mut() {
                let (verts, dirty) = build_glyph_verts(atlas, &title_draws);
                if dirty {
                    if let Some(tex) = self.fontdue_title_texture.as_ref() {
                        queue.write_texture(
                            wgpu::TexelCopyTextureInfo {
                                texture: tex,
                                mip_level: 0,
                                origin: wgpu::Origin3d::ZERO,
                                aspect: wgpu::TextureAspect::All,
                            },
                            atlas.atlas_bytes(),
                            wgpu::TexelCopyBufferLayout {
                                offset: 0,
                                bytes_per_row: Some(atlas.atlas_width),
                                rows_per_image: None,
                            },
                            wgpu::Extent3d {
                                width: atlas.atlas_width,
                                height: atlas.atlas_height,
                                depth_or_array_layers: 1,
                            },
                        );
                    }
                }
                let max = 64 * 6;
                let count = verts.len().min(max);
                self.vertex_count_title = count as u32;
                if count > 0 {
                    queue.write_buffer(
                        &self.vertex_buffer_title,
                        0,
                        bytemuck::cast_slice(&verts[..count]),
                    );
                }
            }
        }
    }

    /// 在 render pass 内画文字。
    pub fn render<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>) {
        pass.set_pipeline(&self.pipeline);

        // Body
        if self.vertex_count > 0 {
            if let Some(bg) = &self.bind_group {
                pass.set_bind_group(0, bg, &[]);
                pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
                pass.draw(0..self.vertex_count, 0..1);
            }
        }
        // Heading
        if self.vertex_count_heading > 0 {
            if let Some(bg) = &self.bind_group_heading {
                pass.set_bind_group(0, bg, &[]);
                pass.set_vertex_buffer(0, self.vertex_buffer_heading.slice(..));
                pass.draw(0..self.vertex_count_heading, 0..1);
            }
        }
        // Title
        if self.vertex_count_title > 0 {
            if let Some(bg) = &self.bind_group_title {
                pass.set_bind_group(0, bg, &[]);
                pass.set_vertex_buffer(0, self.vertex_buffer_title.slice(..));
                pass.draw(0..self.vertex_count_title, 0..1);
            }
        }
    }

    /// 获取 pipeline 的 bind group layout（供 load_font 使用）。
    pub fn bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("text_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        })
    }
}

/// Phase 4.2 (redesign): 把若干 TextDraw 编译为 GlyphVertex 数组（针对单一 atlas）。
/// 返回 (vertices, atlas_dirty)。
fn build_glyph_verts(atlas: &mut FontdueAtlas, draws: &[&TextDraw]) -> (Vec<GlyphVertex>, bool) {
    let atlas_w = atlas.atlas_width as f32;
    let atlas_h = atlas.atlas_height as f32;
    let mut verts = Vec::with_capacity(draws.len() * 32);
    let mut dirty = false;

    for draw in draws {
        let mut cursor_x = draw.x;
        let baseline_y = draw.y + atlas.line_height * 0.8;

        let text_width = if draw.align != TextAlign::Left && draw.max_width > 0.0 {
            let mut w = 0.0f32;
            for ch in draw.text.chars() {
                let had = atlas.glyphs.contains_key(&ch);
                w += atlas.advance_width(ch);
                if !had {
                    dirty = true;
                }
            }
            w
        } else {
            0.0
        };

        match draw.align {
            TextAlign::Center => {
                if draw.max_width > 0.0 {
                    cursor_x += (draw.max_width - text_width) / 2.0;
                }
            }
            TextAlign::Right => {
                if draw.max_width > 0.0 {
                    cursor_x += draw.max_width - text_width;
                }
            }
            TextAlign::Left => {}
        }

        for ch in draw.text.chars() {
            let had = atlas.glyphs.contains_key(&ch);
            if let Some(g) = atlas.get_glyph(ch) {
                if !had {
                    dirty = true;
                }
                if g.width > 0 && g.height > 0 {
                    let x0 = cursor_x + g.xoffset;
                    let y0 = baseline_y + g.yoffset;
                    let x1 = x0 + g.width as f32;
                    let y1 = y0 + g.height as f32;
                    let u0 = g.atlas_x as f32 / atlas_w;
                    let v0 = g.atlas_y as f32 / atlas_h;
                    let u1 = (g.atlas_x + g.width) as f32 / atlas_w;
                    let v1 = (g.atlas_y + g.height) as f32 / atlas_h;
                    verts.push(GlyphVertex {
                        pos: [x0, y0],
                        uv: [u0, v0],
                    });
                    verts.push(GlyphVertex {
                        pos: [x1, y0],
                        uv: [u1, v0],
                    });
                    verts.push(GlyphVertex {
                        pos: [x0, y1],
                        uv: [u0, v1],
                    });
                    verts.push(GlyphVertex {
                        pos: [x1, y0],
                        uv: [u1, v0],
                    });
                    verts.push(GlyphVertex {
                        pos: [x1, y1],
                        uv: [u1, v1],
                    });
                    verts.push(GlyphVertex {
                        pos: [x0, y1],
                        uv: [u0, v1],
                    });
                }
                cursor_x += g.xadvance;
            }
        }
    }

    (verts, dirty)
}
