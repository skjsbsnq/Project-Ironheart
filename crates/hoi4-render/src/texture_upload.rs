//! Shared texture upload helpers for render passes.

use std::collections::HashMap;
use std::sync::Arc;

use hoi4_assets::{dds_upload_plan, AssetBytes, DdsFormat, DdsImage, MapResRole, VanillaMapSet};

pub struct UploadedTexture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub width: u32,
    pub height: u32,
    pub format: wgpu::TextureFormat,
}

pub struct TextureUploadHelper {
    map_set: Arc<VanillaMapSet>,
    cache: HashMap<MapResRole, Arc<UploadedTexture>>,
    pub linear_sampler: wgpu::Sampler,
    pub nearest_sampler: wgpu::Sampler,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TextureUploadCacheStats {
    pub entries: usize,
}

impl TextureUploadHelper {
    pub fn new(device: &wgpu::Device, map_set: Arc<VanillaMapSet>) -> Self {
        let linear_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("vanilla_linear"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            ..Default::default()
        });
        let nearest_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("vanilla_nearest"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });
        Self {
            map_set,
            cache: HashMap::new(),
            linear_sampler,
            nearest_sampler,
        }
    }

    pub fn loaded_count(&self) -> usize {
        self.cache.len()
    }

    pub fn cache_stats(&self) -> TextureUploadCacheStats {
        TextureUploadCacheStats {
            entries: self.cache.len(),
        }
    }

    pub fn upload_role(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        role: MapResRole,
    ) -> Option<Arc<UploadedTexture>> {
        if let Some(existing) = self.cache.get(&role) {
            return Some(existing.clone());
        }
        let bytes = self.map_set.bytes(role)?.clone();
        let uploaded = upload_dds_bytes(device, queue, role, &bytes)?;
        let arc = Arc::new(uploaded);
        self.cache.insert(role, arc.clone());
        Some(arc)
    }

    pub fn get_cached(&self, role: MapResRole) -> Option<Arc<UploadedTexture>> {
        self.cache.get(&role).cloned()
    }
}

fn upload_dds_bytes(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    role: MapResRole,
    bytes: &AssetBytes,
) -> Option<UploadedTexture> {
    let path = role.relative_path();
    if !path.to_ascii_lowercase().ends_with(".dds") {
        eprintln!(
            "[texture_upload] role {:?} (path={}) is not a DDS; use specialized loader",
            role, path
        );
        return None;
    }

    let dds = match DdsImage::parse(bytes) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("[texture_upload] DDS parse failed for {:?}: {}", role, e);
            return None;
        }
    };

    let format = dds_to_wgpu_format_for_role(dds.format, role);
    let upload_plan = match dds_upload_plan(&dds) {
        Some(plan) if plan.upload_mip_count > 0 => plan,
        _ => {
            eprintln!(
                "[texture_upload] DDS has no uploadable mips for {:?}: {}",
                role, path
            );
            return None;
        }
    };
    let mip_count = upload_plan.upload_mip_count;

    let label = format!("vanilla_{:?}", role);
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(&label),
        size: wgpu::Extent3d {
            width: dds.width,
            height: dds.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: mip_count,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });

    for mip in &upload_plan.mips {
        let data = &dds.data[mip.offset..mip.offset + mip.size];

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: mip.level,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(mip.bytes_per_row),
                rows_per_image: None,
            },
            wgpu::Extent3d {
                width: mip.copy_width,
                height: mip.copy_height,
                depth_or_array_layers: 1,
            },
        );
    }

    let view = texture.create_view(&Default::default());
    Some(UploadedTexture {
        texture,
        view,
        width: dds.width,
        height: dds.height,
        format,
    })
}

fn dds_to_wgpu_format_for_role(format: DdsFormat, role: MapResRole) -> wgpu::TextureFormat {
    let srgb = role.is_srgb();
    match (format, srgb) {
        (DdsFormat::Bc1, true) => wgpu::TextureFormat::Bc1RgbaUnormSrgb,
        (DdsFormat::Bc1, false) => wgpu::TextureFormat::Bc1RgbaUnorm,
        (DdsFormat::Bc3, true) => wgpu::TextureFormat::Bc3RgbaUnormSrgb,
        (DdsFormat::Bc3, false) => wgpu::TextureFormat::Bc3RgbaUnorm,
        (DdsFormat::Bc5, _) => wgpu::TextureFormat::Bc5RgUnorm,
        (DdsFormat::Bgra8, true) => wgpu::TextureFormat::Bgra8UnormSrgb,
        (DdsFormat::Bgra8, false) => wgpu::TextureFormat::Bgra8Unorm,
        (DdsFormat::Bgr555, true) => wgpu::TextureFormat::Bgra8UnormSrgb,
        (DdsFormat::Bgr555, false) => wgpu::TextureFormat::Bgra8Unorm,
        (DdsFormat::Unknown(_), true) => wgpu::TextureFormat::Bgra8UnormSrgb,
        (DdsFormat::Unknown(_), false) => wgpu::TextureFormat::Bgra8Unorm,
    }
}

#[cfg(test)]
fn block_size_for_format(format: wgpu::TextureFormat) -> (u32, u32) {
    use wgpu::TextureFormat as F;
    match format {
        F::Bc1RgbaUnorm | F::Bc1RgbaUnormSrgb => (4, 8),
        F::Bc3RgbaUnorm | F::Bc3RgbaUnormSrgb => (4, 16),
        F::Bc5RgUnorm => (4, 16),
        F::Bgra8Unorm | F::Bgra8UnormSrgb | F::Rgba8Unorm | F::Rgba8UnormSrgb => (1, 4),
        _ => (1, 4),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_sizes_known() {
        assert_eq!(
            block_size_for_format(wgpu::TextureFormat::Bc1RgbaUnormSrgb),
            (4, 8)
        );
        assert_eq!(
            block_size_for_format(wgpu::TextureFormat::Bc3RgbaUnormSrgb),
            (4, 16)
        );
        assert_eq!(
            block_size_for_format(wgpu::TextureFormat::Bgra8Unorm),
            (1, 4)
        );
    }

    #[test]
    fn role_controls_color_space_for_shared_dds_uploads() {
        assert_eq!(
            dds_to_wgpu_format_for_role(DdsFormat::Bc3, MapResRole::ColormapWater(0)),
            wgpu::TextureFormat::Bc3RgbaUnormSrgb
        );
        assert_eq!(
            dds_to_wgpu_format_for_role(DdsFormat::Bc3, MapResRole::RiverMasks),
            wgpu::TextureFormat::Bc3RgbaUnorm
        );
        assert_eq!(
            dds_to_wgpu_format_for_role(DdsFormat::Bc5, MapResRole::Lean1),
            wgpu::TextureFormat::Bc5RgUnorm
        );
    }

    #[test]
    fn cache_stats_default_to_empty() {
        let stats = TextureUploadCacheStats::default();
        assert_eq!(stats.entries, 0);
    }
}
