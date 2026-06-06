pub(crate) const MIN_FRAGMENT_SAMPLED_TEXTURES_FOR_PARITY: u32 = 32;

pub(crate) fn parity_required_limits(adapter_limits: wgpu::Limits) -> wgpu::Limits {
    let mut limits = wgpu::Limits::default().using_resolution(adapter_limits.clone());
    limits.max_sampled_textures_per_shader_stage =
        adapter_limits.max_sampled_textures_per_shader_stage.min(
            MIN_FRAGMENT_SAMPLED_TEXTURES_FOR_PARITY
                .max(limits.max_sampled_textures_per_shader_stage),
        );
    limits
}

pub(crate) fn make_depth_view(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
) -> wgpu::TextureView {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("depth"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    tex.create_view(&Default::default())
}
