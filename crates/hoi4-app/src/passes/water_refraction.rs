use super::HDR_FORMAT;
use crate::map_perf::MapQualityPreset;
use wgpu::util::DeviceExt;

pub const WATER_REFRACTION_LABEL: &str = "water_refraction_rt";

const WATER_REFRACTION_WGSL: &str = r#"
@group(0) @binding(0) var hdr_pre_water: texture_2d<f32>;
@group(0) @binding(1) var hdr_sampler: sampler;

struct WaterRefractionParams {
    source_size: vec2<f32>,
    target_size: vec2<f32>,
};

@group(0) @binding(2) var<uniform> params: WaterRefractionParams;

@vertex
fn vs_main(@builtin(vertex_index) vid: u32) -> @builtin(position) vec4<f32> {
    let xy = vec2<f32>(f32((vid << 1u) & 2u), f32(vid & 2u));
    return vec4<f32>(xy * 2.0 - 1.0, 0.0, 1.0);
}

@fragment
fn fs_main(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let target_dim = max(params.target_size, vec2<f32>(1.0));
    let source_dim = max(params.source_size, vec2<f32>(1.0));
    let uv = pos.xy / target_dim;
    let px = 1.0 / source_dim;
    let center = textureSample(hdr_pre_water, hdr_sampler, uv).rgb;
    let cardinal =
        textureSample(hdr_pre_water, hdr_sampler, uv + vec2<f32>( px.x, 0.0)).rgb +
        textureSample(hdr_pre_water, hdr_sampler, uv + vec2<f32>(-px.x, 0.0)).rgb +
        textureSample(hdr_pre_water, hdr_sampler, uv + vec2<f32>(0.0,  px.y)).rgb +
        textureSample(hdr_pre_water, hdr_sampler, uv + vec2<f32>(0.0, -px.y)).rgb;
    let diagonal =
        textureSample(hdr_pre_water, hdr_sampler, uv + vec2<f32>( px.x,  px.y)).rgb +
        textureSample(hdr_pre_water, hdr_sampler, uv + vec2<f32>(-px.x,  px.y)).rgb +
        textureSample(hdr_pre_water, hdr_sampler, uv + vec2<f32>( px.x, -px.y)).rgb +
        textureSample(hdr_pre_water, hdr_sampler, uv + vec2<f32>(-px.x, -px.y)).rgb;
    let blur = center * 0.56 + cardinal * 0.08 + diagonal * 0.03;
    let luma = dot(blur, vec3<f32>(0.2126, 0.7152, 0.0722));
    let color = max(mix(vec3<f32>(luma), blur, 1.12), vec3<f32>(0.0));
    return vec4<f32>(color, 1.0);
}
"#;

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct WaterRefractionParams {
    source_size: [f32; 2],
    target_size: [f32; 2],
}

impl WaterRefractionParams {
    fn new(source_width: u32, source_height: u32, target_width: u32, target_height: u32) -> Self {
        Self {
            source_size: [source_width.max(1) as f32, source_height.max(1) as f32],
            target_size: [target_width.max(1) as f32, target_height.max(1) as f32],
        }
    }
}

pub struct WaterRefractionTarget {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    pub width: u32,
    pub height: u32,
    pub format: wgpu::TextureFormat,
    pub quality: MapQualityPreset,
}

impl WaterRefractionTarget {
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        Self::for_quality(device, width, height, MapQualityPreset::High)
    }

    pub fn for_quality(
        device: &wgpu::Device,
        source_width: u32,
        source_height: u32,
        quality: MapQualityPreset,
    ) -> Self {
        let scale = quality.water_refraction_scale().max(0.0);
        let w = scaled_extent(source_width, scale);
        let h = scaled_extent(source_height, scale);
        let format = HDR_FORMAT;
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(WATER_REFRACTION_LABEL),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("water_refraction_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        Self {
            texture,
            view,
            sampler,
            width: w,
            height: h,
            format,
            quality,
        }
    }

    pub fn matches_source_size(&self, width: u32, height: u32, quality: MapQualityPreset) -> bool {
        let scale = quality.water_refraction_scale().max(0.0);
        self.width == scaled_extent(width, scale)
            && self.height == scaled_extent(height, scale)
            && self.quality == quality
    }

    pub fn memory_bytes(&self) -> u64 {
        self.width as u64 * self.height as u64 * bytes_per_pixel(self.format)
    }

    pub fn status_line(&self) -> String {
        format!(
            "water_refraction=created {}x{} {:?} sampler=linear_clamp producer=fullscreen_9tap",
            self.width, self.height, self.format
        )
    }

    pub fn resolution_label(&self) -> &'static str {
        self.quality.water_refraction_label()
    }
}

pub struct WaterRefractionPass {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    sampler: wgpu::Sampler,
    params_buffer: wgpu::Buffer,
    source_width: u32,
    source_height: u32,
    target_width: u32,
    target_height: u32,
}

impl WaterRefractionPass {
    pub fn new(
        device: &wgpu::Device,
        hdr_pre_water_view: &wgpu::TextureView,
        target_format: wgpu::TextureFormat,
        source_width: u32,
        source_height: u32,
        target_width: u32,
        target_height: u32,
    ) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("water_refraction_shader"),
            source: wgpu::ShaderSource::Wgsl(WATER_REFRACTION_WGSL.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("water_refraction_bgl"),
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
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("water_refraction_hdr_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("water_refraction_pl"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("water_refraction_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: None,
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

        let params =
            WaterRefractionParams::new(source_width, source_height, target_width, target_height);
        let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("water_refraction_params"),
            contents: bytemuck::bytes_of(&params),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let bind_group = make_bind_group(
            device,
            &bind_group_layout,
            hdr_pre_water_view,
            &sampler,
            &params_buffer,
        );

        Self {
            pipeline,
            bind_group_layout,
            bind_group,
            sampler,
            params_buffer,
            source_width: source_width.max(1),
            source_height: source_height.max(1),
            target_width: target_width.max(1),
            target_height: target_height.max(1),
        }
    }

    pub fn rebuild_bind_group(
        &mut self,
        device: &wgpu::Device,
        hdr_pre_water_view: &wgpu::TextureView,
        source_width: u32,
        source_height: u32,
        target_width: u32,
        target_height: u32,
    ) {
        self.source_width = source_width.max(1);
        self.source_height = source_height.max(1);
        self.target_width = target_width.max(1);
        self.target_height = target_height.max(1);
        self.bind_group = make_bind_group(
            device,
            &self.bind_group_layout,
            hdr_pre_water_view,
            &self.sampler,
            &self.params_buffer,
        );
    }

    pub fn update_params(&self, queue: &wgpu::Queue) {
        queue.write_buffer(
            &self.params_buffer,
            0,
            bytemuck::bytes_of(&WaterRefractionParams::new(
                self.source_width,
                self.source_height,
                self.target_width,
                self.target_height,
            )),
        );
    }

    pub fn render(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
    ) {
        self.update_params(queue);
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("water_refraction"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            ..Default::default()
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}

impl super::Pass for WaterRefractionPass {
    fn name(&self) -> &'static str {
        "water_refraction"
    }
}

impl super::Pass for WaterRefractionTarget {
    fn name(&self) -> &'static str {
        "water_refraction"
    }
}

fn make_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    hdr_pre_water_view: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
    params_buffer: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("water_refraction_bg"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(hdr_pre_water_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: params_buffer.as_entire_binding(),
            },
        ],
    })
}

fn scaled_extent(source: u32, scale: f32) -> u32 {
    ((source.max(1) as f32) * scale.max(0.0)).ceil().max(1.0) as u32
}

fn bytes_per_pixel(format: wgpu::TextureFormat) -> u64 {
    match format {
        wgpu::TextureFormat::Rgba16Float => 8,
        wgpu::TextureFormat::Rgba8Unorm
        | wgpu::TextureFormat::Rgba8UnormSrgb
        | wgpu::TextureFormat::Bgra8Unorm
        | wgpu::TextureFormat::Bgra8UnormSrgb => 4,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_estimate_matches_full_res_rgba16float() {
        assert_eq!(bytes_per_pixel(wgpu::TextureFormat::Rgba16Float), 8);
        assert_eq!(
            1920_u64 * 1080_u64 * bytes_per_pixel(HDR_FORMAT),
            1920 * 1080 * 8
        );
    }

    #[test]
    fn target_size_follows_quality_preset() {
        assert_eq!(
            scaled_extent(1920, MapQualityPreset::High.water_refraction_scale()),
            1920
        );
        assert_eq!(
            scaled_extent(1920, MapQualityPreset::Ultra.water_refraction_scale()),
            1920
        );
        assert_eq!(
            scaled_extent(1920, MapQualityPreset::Balanced.water_refraction_scale()),
            960
        );
        assert_eq!(
            scaled_extent(1920, MapQualityPreset::LowEnd.water_refraction_scale()),
            1
        );
    }

    #[test]
    fn producer_shader_samples_pre_water_hdr() {
        assert!(WATER_REFRACTION_WGSL.contains("hdr_pre_water"));
        assert!(WATER_REFRACTION_WGSL.contains("textureSample(hdr_pre_water"));
        assert!(WATER_REFRACTION_WGSL.contains("let cardinal"));
        assert!(WATER_REFRACTION_WGSL.contains("let diagonal"));
        assert!(WATER_REFRACTION_WGSL.contains("center * 0.56"));
        assert!(WATER_REFRACTION_WGSL.contains("diagonal * 0.03"));
        assert!(WATER_REFRACTION_WGSL.contains("mix(vec3<f32>(luma), blur, 1.12)"));
        assert!(WATER_REFRACTION_WGSL.contains("target_size"));
        assert!(WATER_REFRACTION_WGSL.contains("source_size"));
        assert!(WATER_REFRACTION_WGSL.contains("@builtin(position) pos"));
    }
}
