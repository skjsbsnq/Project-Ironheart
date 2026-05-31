use hoi4_render::mapname_3d::CountryNameInstance;
use hoi4_render::shader_rt::compose_shader;
use wgpu::util::DeviceExt;

use crate::passes::HDR_FORMAT;

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ProvinceNameParams {
    pub text_color: [f32; 4],
    pub outline_color: [f32; 4],
    pub distortion_amount: f32,
    pub fade: f32,
    pub zoom_threshold_near: f32,
    pub zoom_threshold_far: f32,
    pub scale: f32,
    pub _pad0: f32,
    pub _pad1: f32,
    pub _pad2: f32,
}

impl Default for ProvinceNameParams {
    fn default() -> Self {
        Self {
            text_color: [0.92, 0.89, 0.82, 1.0],
            outline_color: [0.04, 0.03, 0.02, 1.0],
            distortion_amount: 0.05,
            fade: 1.0,
            zoom_threshold_near: 0.7,
            zoom_threshold_far: 0.4,
            scale: 1.0,
            _pad0: 0.0,
            _pad1: 0.0,
            _pad2: 0.0,
        }
    }
}

const _: () = assert!(std::mem::size_of::<ProvinceNameParams>() == 64);

const PROVINCE_NAME_WGSL: &str = r#"
struct ProvinceNameParams {
    text_color: vec4<f32>,
    outline_color: vec4<f32>,
    distortion_amount: f32,
    fade: f32,
    zoom_threshold_near: f32,
    zoom_threshold_far: f32,
    scale: f32,
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
};

@group(0) @binding(0) var<uniform> frame: GlobalFrameUniform;
@group(0) @binding(1) var<uniform> pn_params: ProvinceNameParams;

@group(1) @binding(0) var name_atlas: texture_2d<f32>;
@group(1) @binding(1) var name_sampler: sampler;

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
    @location(3) map_px: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vid: u32, inst: InstanceData) -> VsOut {
    var local_x: f32 = -1.0;
    var local_y: f32 = -1.0;
    if (vid == 1u || vid == 2u || vid == 4u) { local_x = 1.0; }
    if (vid == 2u || vid == 4u || vid == 5u) { local_y = 1.0; }

    let to_cam = normalize(frame.cam_pos - inst.center);
    let distorted = inst.center + to_cam * pn_params.distortion_amount;
    let center_clip = frame.view_proj * vec4<f32>(distorted, 1.0);
    let center_ndc_w = center_clip.w;

    if (center_ndc_w <= 0.001) {
        var out: VsOut;
        out.clip_pos = vec4<f32>(10.0, 10.0, 10.0, 1.0);
        out.uv = vec2<f32>(0.0, 0.0);
        out.world_xz = vec2<f32>(0.0, 0.0);
        out.world_pos = vec3<f32>(0.0, 0.0, 0.0);
        out.map_px = vec2<f32>(0.0, 0.0);
        return out;
    }

    let atlas_dim = vec2<f32>(textureDimensions(name_atlas));
    let aspect = ((inst.uv_max.x - inst.uv_min.x) * atlas_dim.x) /
                 max((inst.uv_max.y - inst.uv_min.y) * atlas_dim.y, 0.001);
    let target_px = 14.0 * pn_params.scale;
    let pixel_to_ndc = 2.0 / frame.screen_size.y;
    let h_ndc = target_px * pixel_to_ndc * center_ndc_w;
    let w_ndc = h_ndc * aspect;

    var clip = center_clip;
    clip.x += local_x * w_ndc;
    clip.y += local_y * h_ndc;
    clip.z -= 0.001 * clip.w;

    var out: VsOut;
    out.clip_pos = clip;
    out.uv = vec2<f32>(
        mix(inst.uv_min.x, inst.uv_max.x, (local_x + 1.0) * 0.5),
        mix(inst.uv_min.y, inst.uv_max.y, (local_y + 1.0) * 0.5),
    );
    out.world_xz = inst.center.xz;
    out.world_pos = inst.center;
    out.map_px = world_xz_to_map_px(inst.center.xz, frame.vanilla_map_size_world_size.zw);
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let s = textureSample(name_atlas, name_sampler, in.uv).r;
    if (s < 0.04) { discard; }

    let text_t = smoothstep(0.62, 0.68, s);
    let outline_t = smoothstep(0.27, 0.33, s) * (1.0 - text_t);

    let globe_n = calc_globe_normal(in.map_px, frame.day_night_hour_sun_dir.x);
    let night = day_night_factor(globe_n, frame.day_night_hour_sun_dir.yzw, 1.0);
    let dim = 1.0 - night * 0.35;

    var color = pn_params.outline_color.rgb * outline_t + pn_params.text_color.rgb * text_t;
    color *= dim;
    var alpha = (text_t + outline_t) * pn_params.fade;

    let cam_dist = length(frame.cam_pos - in.world_pos);
    let zoom_alpha = smoothstep(60.0, 30.0, cam_dist);
    alpha *= zoom_alpha;

    return vec4<f32>(color, alpha);
}
"#;

pub struct ProvinceNamePassInputs<'a> {
    pub global_uniform_buffer: &'a wgpu::Buffer,
    pub depth_format: wgpu::TextureFormat,
    pub atlas: Option<&'a crate::province_name_atlas::ProvinceNameAtlas>,
    pub instances: &'a [CountryNameInstance],
}

pub struct ProvinceNamePass {
    pipeline: wgpu::RenderPipeline,
    global_bind_group: wgpu::BindGroup,
    atlas_bind_group: wgpu::BindGroup,
    params_buffer: wgpu::Buffer,
    instance_buffer: wgpu::Buffer,
    instance_count: u32,
    pixel_counts: Vec<u32>,
}

impl ProvinceNamePass {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        inputs: ProvinceNamePassInputs<'_>,
    ) -> Self {
        let composed = compose_shader(PROVINCE_NAME_WGSL, true, true);

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("province_name_shader"),
            source: wgpu::ShaderSource::Wgsl(composed.into()),
        });

        let bgl_global = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("province_name_bgl_global"),
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
            label: Some("province_name_bgl_atlas"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
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

        let params = ProvinceNameParams::default();
        let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("province_name_params"),
            contents: bytemuck::bytes_of(&params),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let atlas_view = if let Some(atlas) = inputs.atlas {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("province_name_atlas"),
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
            texture.create_view(&Default::default())
        } else {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("province_name_atlas_empty"),
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
            texture.create_view(&Default::default())
        };

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("province_name_sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });

        let global_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("province_name_bg_global"),
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
            label: Some("province_name_bg_atlas"),
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

        let instance_count = inputs.instances.len() as u32;
        let pixel_counts: Vec<u32> = inputs
            .instances
            .iter()
            .map(|inst| ((inst.width_world * inst.height_world) * 1_000_000.0).max(0.0) as u32)
            .collect();
        let inst_size = std::mem::size_of::<CountryNameInstance>() as u64;
        let instance_buffer = if inputs.instances.is_empty() {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("province_name_inst_empty"),
                contents: &vec![0u8; inst_size as usize],
                usage: wgpu::BufferUsages::VERTEX,
            })
        } else {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("province_name_inst"),
                contents: bytemuck::cast_slice(inputs.instances),
                usage: wgpu::BufferUsages::VERTEX,
            })
        };

        let pl_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("province_name_pl_layout"),
            bind_group_layouts: &[&bgl_global, &bgl_atlas],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("province_name_pipeline"),
            layout: Some(&pl_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: inst_size,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &[
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: wgpu::VertexFormat::Float32x3,
                        },
                        wgpu::VertexAttribute {
                            offset: 12,
                            shader_location: 1,
                            format: wgpu::VertexFormat::Float32,
                        },
                        wgpu::VertexAttribute {
                            offset: 16,
                            shader_location: 2,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            offset: 24,
                            shader_location: 3,
                            format: wgpu::VertexFormat::Float32,
                        },
                        wgpu::VertexAttribute {
                            offset: 28,
                            shader_location: 4,
                            format: wgpu::VertexFormat::Float32,
                        },
                        wgpu::VertexAttribute {
                            offset: 32,
                            shader_location: 5,
                            format: wgpu::VertexFormat::Float32x2,
                        },
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

    pub fn update_params(&self, queue: &wgpu::Queue, fade: f32, scale: f32) {
        let params = ProvinceNameParams {
            fade,
            scale,
            ..Default::default()
        };
        queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(&params));
    }

    pub fn render(&self, pass: &mut wgpu::RenderPass, zoom_factor: f32) {
        if self.instance_count == 0 {
            return;
        }

        if zoom_factor < 0.62 {
            return;
        }

        // At medium zoom (0.4–0.7), only draw large provinces (>= 5000 px).
        // At close zoom (> 0.7), draw all.
        // This is handled per-instance in the future; for now draw all
        // and let the shader smoothstep handle the fade.

        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.global_bind_group, &[]);
        pass.set_bind_group(1, &self.atlas_bind_group, &[]);
        pass.set_vertex_buffer(0, self.instance_buffer.slice(..));
        let min_area = if zoom_factor < 0.70 {
            18_000
        } else if zoom_factor < 0.84 {
            7_000
        } else {
            1_800
        };
        let visible: Vec<u32> = self
            .pixel_counts
            .iter()
            .enumerate()
            .filter_map(|(idx, &area)| {
                if area >= min_area {
                    Some(idx as u32)
                } else {
                    None
                }
            })
            .collect();
        for inst_idx in visible {
            pass.draw(0..6, inst_idx..inst_idx + 1);
        }
    }

    pub fn instance_count(&self) -> u32 {
        self.instance_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn province_name_params_size_is_64() {
        assert_eq!(std::mem::size_of::<ProvinceNameParams>(), 64);
    }

    #[test]
    fn province_name_wgsl_naga_parses() {
        let composed = compose_shader(PROVINCE_NAME_WGSL, true, true);
        let module = naga::front::wgsl::parse_str(&composed);
        assert!(
            module.is_ok(),
            "province_name WGSL failed naga validation: {:?}",
            module.err()
        );
    }
}
