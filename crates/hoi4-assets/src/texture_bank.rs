//! `TextureBank` — 显式路径 → wgpu 纹理缓存（V5 收口版）。
//!
//! V3 时代本模块按 `.gfx` `SpriteDef` 加载纹理；V5（2026-05-18）放弃 vanilla GUI
//! 路线后改为显式相对路径加载。
//!
//! ## API
//!
//! - [`TextureBank::get_or_load_path`]：按显式相对路径加载 DDS / TGA → 上传 GPU
//!   → 缓存。重复调用返回同一 `Arc<TextureEntry>`。
//! - [`TextureBank::get_path`]：按已加载路径查询，不触发 IO。
//! - [`TextureBank::get_or_load_with_frames`]：同 `get_or_load_path` 但带横向
//!   切帧数（NATO 兵牌、动画 sprite 等需要）。
//!
//! 设计：`wgpu::Texture` 不 Clone，所以多个调用引用同一相对路径时共享同一个
//! `Arc<GpuTexture>`。

use std::collections::HashMap;
use std::sync::Arc;

use crate::db::AssetDb;
use crate::dds::{compute_frame_uvs, DdsFormat, DdsImage, FrameUv};
use crate::error::AssetError;
use crate::tga::TgaImage;

/// 一个已上传到 GPU 的纹理。多个 entry 可共享（同一 DDS / TGA 文件）。
pub struct GpuTexture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub width: u32,
    pub height: u32,
    pub format: wgpu::TextureFormat,
}

/// 一个纹理条目：GPU 句柄 + 横向切帧 UV。
pub struct TextureEntry {
    pub gpu: Arc<GpuTexture>,
    pub frame_uvs: Vec<FrameUv>,
}

/// 显式路径 → GPU 纹理的缓存。
pub struct TextureBank {
    /// lowercase(relative_path) → entry（同一路径的所有引用共享同一 entry）。
    entries: HashMap<String, Arc<TextureEntry>>,
    /// lowercase(relative_path) → 共享 GPU 纹理。`entries` 多个 frame-UV 变体
    /// 可指向同一 GPU 纹理；本表保证 GPU 上传只发生一次。
    by_path: HashMap<String, Arc<GpuTexture>>,
}

impl TextureBank {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            by_path: HashMap::new(),
        }
    }

    /// 已缓存的条目数。
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// 已上传的独立纹理文件数（去重后）。
    pub fn texture_count(&self) -> usize {
        self.by_path.len()
    }

    /// 按相对路径查询已加载的纹理条目（不触发 IO）。
    pub fn get_path(&self, rel_path: &str) -> Option<&Arc<TextureEntry>> {
        self.entries.get(&Self::path_key(rel_path))
    }

    /// 按显式相对路径加载纹理；DDS / TGA 自动判别。已缓存则直接返回。
    /// 帧数固定为 1（最常见的非 sprite 场景）。需要切帧请用
    /// [`Self::get_or_load_with_frames`]。
    pub fn get_or_load_path<D: AssetDb>(
        &mut self,
        rel_path: &str,
        db: &D,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<Arc<TextureEntry>, AssetError> {
        self.get_or_load_with_frames(rel_path, 1, db, device, queue)
    }

    /// 按显式相对路径加载纹理，并指定横向帧数。
    pub fn get_or_load_with_frames<D: AssetDb>(
        &mut self,
        rel_path: &str,
        no_of_frames: u32,
        db: &D,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<Arc<TextureEntry>, AssetError> {
        let key = format!("{}#{}", Self::path_key(rel_path), no_of_frames);
        if let Some(entry) = self.entries.get(&key) {
            return Ok(entry.clone());
        }

        let gpu = self.ensure_gpu_texture(rel_path, db, device, queue)?;
        let frame_uvs = compute_frame_uvs(no_of_frames.max(1));
        let entry = Arc::new(TextureEntry { gpu, frame_uvs });
        self.entries.insert(key, entry.clone());
        Ok(entry)
    }

    /// 内部：把相对路径规范化（统一斜杠 + 小写）。
    fn path_key(rel: &str) -> String {
        let mut out = String::with_capacity(rel.len());
        let mut prev_slash = false;
        for c in rel.chars() {
            let is_slash = c == '/' || c == '\\';
            if is_slash {
                if !prev_slash {
                    out.push('/');
                }
                prev_slash = true;
            } else {
                out.push(c.to_ascii_lowercase());
                prev_slash = false;
            }
        }
        out
    }

    /// 确保某 texturefile 路径的 GPU 纹理已上传。共享缓存。
    ///
    /// V3 4.1.bis.6 行为保留：vanilla `.gfx` 写 `.tga` 但磁盘是 `.dds` 时自动
    /// 反向 fallback；同样地 `.dds` 缺失时尝试 `.tga`。
    fn ensure_gpu_texture<D: AssetDb>(
        &mut self,
        tex_path: &str,
        db: &D,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<Arc<GpuTexture>, AssetError> {
        let normalised: String = {
            let mut out = String::with_capacity(tex_path.len());
            let mut prev_slash = false;
            for c in tex_path.chars() {
                let is_slash = c == '/' || c == '\\';
                if is_slash {
                    if !prev_slash {
                        out.push('/');
                    }
                    prev_slash = true;
                } else {
                    out.push(c);
                    prev_slash = false;
                }
            }
            out
        };

        let path_key = normalised.to_ascii_lowercase();
        if let Some(gpu) = self.by_path.get(&path_key) {
            return Ok(gpu.clone());
        }

        let candidates: Vec<String> = if path_key.ends_with(".tga") {
            let dds_alt = format!("{}.dds", &normalised[..normalised.len() - 4]);
            vec![normalised.clone(), dds_alt]
        } else if path_key.ends_with(".dds") {
            let tga_alt = format!("{}.tga", &normalised[..normalised.len() - 4]);
            vec![normalised.clone(), tga_alt]
        } else {
            vec![normalised.clone()]
        };

        let mut last_err: Option<AssetError> = None;
        for cand in &candidates {
            match db.open(cand) {
                Ok(bytes) => {
                    let lower = cand.to_ascii_lowercase();
                    let gpu = if lower.ends_with(".tga") {
                        let tga = TgaImage::parse(&bytes)?;
                        Arc::new(upload_tga_to_gpu(device, queue, &tga, cand))
                    } else {
                        let dds = DdsImage::parse(&bytes)?;
                        Arc::new(upload_dds_to_gpu(device, queue, &dds, cand))
                    };
                    self.by_path.insert(path_key, gpu.clone());
                    return Ok(gpu);
                }
                Err(e) => last_err = Some(e),
            }
        }
        Err(last_err.unwrap_or_else(|| AssetError::not_found(normalised)))
    }
}

impl Default for TextureBank {
    fn default() -> Self {
        Self::new()
    }
}

// ─── GPU upload ─────────────────────────────────────────────────────────────

/// DdsFormat → wgpu::TextureFormat（sRGB / linear 标记）。
pub fn dds_to_wgpu_format(format: DdsFormat) -> wgpu::TextureFormat {
    match format {
        DdsFormat::Bc1 => wgpu::TextureFormat::Bc1RgbaUnormSrgb,
        DdsFormat::Bc3 => wgpu::TextureFormat::Bc3RgbaUnormSrgb,
        DdsFormat::Bc5 => wgpu::TextureFormat::Bc5RgUnorm,
        DdsFormat::Bgra8 | DdsFormat::Bgr555 => wgpu::TextureFormat::Bgra8UnormSrgb,
        DdsFormat::Unknown(_) => wgpu::TextureFormat::Bgra8UnormSrgb,
    }
}

fn upload_dds_to_gpu(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    dds: &DdsImage,
    label: &str,
) -> GpuTexture {
    let wgpu_format = dds_to_wgpu_format(dds.format);
    let mip_count = dds.mip_count();

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: dds.width,
            height: dds.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: mip_count,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu_format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });

    for (i, mip) in dds.mips.iter().enumerate() {
        let data = &dds.data[mip.offset..mip.offset + mip.size];
        let block_size = block_dimensions(dds.format);
        let blocks_wide = (mip.width + block_size - 1) / block_size;
        let bytes_per_row = blocks_wide * bytes_per_block(dds.format);

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: i as u32,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: None,
            },
            wgpu::Extent3d {
                width: mip.width,
                height: mip.height,
                depth_or_array_layers: 1,
            },
        );
    }

    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

    GpuTexture {
        texture,
        view,
        width: dds.width,
        height: dds.height,
        format: wgpu_format,
    }
}

fn upload_tga_to_gpu(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    tga: &TgaImage,
    label: &str,
) -> GpuTexture {
    let format = wgpu::TextureFormat::Rgba8UnormSrgb;
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: tga.width,
            height: tga.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
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
        &tga.pixels,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(tga.width * 4),
            rows_per_image: None,
        },
        wgpu::Extent3d {
            width: tga.width,
            height: tga.height,
            depth_or_array_layers: 1,
        },
    );
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    GpuTexture {
        texture,
        view,
        width: tga.width,
        height: tga.height,
        format,
    }
}

/// Block 维度（BC 格式 = 4；未压缩 = 1）。
fn block_dimensions(format: DdsFormat) -> u32 {
    match format {
        DdsFormat::Bc1 | DdsFormat::Bc3 | DdsFormat::Bc5 => 4,
        _ => 1,
    }
}

/// 每 block 字节数。
fn bytes_per_block(format: DdsFormat) -> u32 {
    match format {
        DdsFormat::Bc1 => 8,
        DdsFormat::Bc3 | DdsFormat::Bc5 => 16,
        DdsFormat::Bgra8 | DdsFormat::Unknown(_) => 4,
        DdsFormat::Bgr555 => 2,
    }
}
