//! Phase 14 — `PoiIconPass`：vanilla POI 图标渲染。
//!
//! 在每省心位置显示该省的"已建建筑"图标（工厂 / 船坞 / 机场 / 资源），
//! 使用 instanced quad billboard + procedural SDF shape + kind-based color。
//!
//! ## 渲染管线
//!
//! - 实例化 billboard quad（6 顶点，每实例 `PoiIconInstance` 20 字节）
//! - `kind` 选择颜色和形状（工厂=方 / 港口=菱 / 机场=三角 / 资源=圆）
//! - `level` 轻微影响图标大小（高等级建筑略大）
//! - zoom-gated：远视角只画 major buildings，近视角画所有

use hoi4_render::buildings::PoiIconInstance;
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable, Debug)]
pub struct PoiIconParams {
    pub opacity: f32,
    pub scale: f32,
    pub outline_strength: f32,
    pub _pad0: f32,
}

impl Default for PoiIconParams {
    fn default() -> Self {
        Self {
            opacity: 1.0,
            scale: 1.0,
            outline_strength: 0.75,
            _pad0: 0.0,
        }
    }
}

const _: () = assert!(std::mem::size_of::<PoiIconParams>() == 16);

pub struct PoiIconPass {
    pipeline: wgpu::RenderPipeline,
    quad_vb: wgpu::Buffer,
    instance_buffer: wgpu::Buffer,
    instance_capacity: u32,
    instance_count: u32,
    camera_bgl: wgpu::BindGroupLayout,
    camera_bg: wgpu::BindGroup,
    params_buffer: wgpu::Buffer,
    pub load_warnings: Vec<String>,
    enabled: bool,
}

impl PoiIconPass {
    pub fn new(
        device: &wgpu::Device,
        _queue: &wgpu::Queue,
        target_format: wgpu::TextureFormat,
        depth_format: wgpu::TextureFormat,
        camera_buffer: &wgpu::Buffer,
        initial_capacity: u32,
    ) -> Self {
        let warnings: Vec<String> = Vec::new();

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("poi_icon_shader"),
            source: wgpu::ShaderSource::Wgsl(hoi4_render::SHADER_POI_ICON_WGSL.into()),
        });

        let camera_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("poi_icon_camera_bgl"),
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

        let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("poi_icon_params"),
            contents: bytemuck::bytes_of(&PoiIconParams::default()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let camera_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("poi_icon_camera_bg"),
            layout: &camera_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: params_buffer.as_entire_binding(),
                },
            ],
        });

        #[repr(C)]
        #[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
        struct QuadVert {
            pos: [f32; 3],
            uv: [f32; 2],
        }
        let quad_verts: [QuadVert; 6] = [
            QuadVert {
                pos: [-1.0, -1.0, 0.0],
                uv: [0.0, 1.0],
            },
            QuadVert {
                pos: [1.0, -1.0, 0.0],
                uv: [1.0, 1.0],
            },
            QuadVert {
                pos: [-1.0, 1.0, 0.0],
                uv: [0.0, 0.0],
            },
            QuadVert {
                pos: [-1.0, 1.0, 0.0],
                uv: [0.0, 0.0],
            },
            QuadVert {
                pos: [1.0, -1.0, 0.0],
                uv: [1.0, 1.0],
            },
            QuadVert {
                pos: [1.0, 1.0, 0.0],
                uv: [1.0, 0.0],
            },
        ];
        let quad_vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("poi_icon_quad_vb"),
            contents: bytemuck::cast_slice(&quad_verts),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("poi_icon_pl"),
            bind_group_layouts: &[&camera_bgl],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("poi_icon_pipeline"),
            layout: Some(&pl),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[
                    wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<QuadVert>() as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &[
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Float32x3,
                                offset: 0,
                                shader_location: 0,
                            },
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Float32x2,
                                offset: 12,
                                shader_location: 1,
                            },
                        ],
                    },
                    wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<PoiIconInstance>() as u64,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &[
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Float32x3,
                                offset: 0,
                                shader_location: 2,
                            },
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Float32,
                                offset: 12,
                                shader_location: 3,
                            },
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Float32,
                                offset: 16,
                                shader_location: 4,
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
                    format: target_format,
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
                format: depth_format,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });

        let cap = initial_capacity.max(64);
        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("poi_icon_instances"),
            size: (cap as u64) * (std::mem::size_of::<PoiIconInstance>() as u64),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            quad_vb,
            instance_buffer,
            instance_capacity: cap,
            instance_count: 0,
            camera_bgl,
            camera_bg,
            params_buffer,
            load_warnings: warnings,
            enabled: true,
        }
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }
    pub fn instance_count(&self) -> u32 {
        self.instance_count
    }

    pub fn upload(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        instances: &[PoiIconInstance],
        detail_level: u8,
    ) {
        let filtered: Vec<&PoiIconInstance> = instances
            .iter()
            .filter(|inst| {
                let kind = inst.kind as u8;
                match detail_level {
                    0 => false,
                    1 => kind <= 2,
                    2 => kind <= hoi4_render::buildings::PoiIconKind::RocketSite as u8,
                    _ => true,
                }
            })
            .collect();

        if filtered.is_empty() {
            self.instance_count = 0;
            return;
        }

        let needed = filtered.len() as u32;
        if needed > self.instance_capacity {
            let new_cap = needed.next_power_of_two().max(64);
            self.instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("poi_icon_instances"),
                size: (new_cap as u64) * (std::mem::size_of::<PoiIconInstance>() as u64),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.instance_capacity = new_cap;
        }

        let data: Vec<PoiIconInstance> = filtered.iter().map(|&i| *i).collect();
        queue.write_buffer(&self.instance_buffer, 0, bytemuck::cast_slice(&data));
        self.instance_count = needed;
    }

    pub fn update_params(&self, queue: &wgpu::Queue, params: &PoiIconParams) {
        queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(params));
    }

    pub fn render<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>) {
        if !self.enabled || self.instance_count == 0 {
            return;
        }

        pass.set_pipeline(&self.pipeline);
        pass.set_vertex_buffer(0, self.quad_vb.slice(..));
        pass.set_vertex_buffer(1, self.instance_buffer.slice(..));
        pass.set_bind_group(0, &self.camera_bg, &[]);
        pass.draw(0..6, 0..self.instance_count);
    }
}

#[cfg(test)]
mod tests {
    use hoi4_render::buildings::PoiIconInstance;

    #[test]
    fn poi_instance_size() {
        assert_eq!(std::mem::size_of::<PoiIconInstance>(), 20);
    }

    #[test]
    fn poi_icon_params_size_is_16() {
        assert_eq!(std::mem::size_of::<super::PoiIconParams>(), 16);
    }
}
