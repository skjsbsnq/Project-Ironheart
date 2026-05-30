//! Phase 4.2 (redesign): 国旗加载与缓存。
//!
//! HOI4 国旗存储格式：
//! - `gfx/flags/<TAG>.tga` — 中性国旗（少数国家有）
//! - `gfx/flags/<TAG>_<ideology>.tga` — 按意识形态变体（绝大多数）
//! - `gfx/flags/medium/<TAG>_<ideology>.tga` — 41×26 中等尺寸（菜单用）
//! - `gfx/flags/small/<TAG>_<ideology>.tga` — 较小尺寸
//!
//! 全部是 32bpp BGRA TGA（type=2 uncompressed）。
//! 我们用 medium 尺寸（够清晰、加载快）。
//!
//! 意识形态字符串和 vanilla 的 `set_politics ruling_party = X` 一致：
//! `fascism / democratic / communism / neutrality`。

use std::collections::HashMap;
use std::sync::Arc;

use hoi4_assets::{AssetDb, FsAssetDb, TgaImage};
use hoi4_paths::PathConfig;

pub struct FlagTexture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub width: u32,
    pub height: u32,
}

pub struct FlagBank {
    /// (tag.to_uppercase(), ideology) → texture
    cache: HashMap<(String, String), Arc<FlagTexture>>,
    /// Bind group 索引（每个 flag 一个，共享 layout/sampler）。
    bind_groups: HashMap<(String, String), wgpu::BindGroup>,
    sampler: wgpu::Sampler,
    /// 找不到的 (tag,ideology) 缓存，避免反复查盘。
    missing: std::collections::HashSet<(String, String)>,
    /// 1×1 fallback 灰色纹理。
    fallback: Arc<FlagTexture>,
    fallback_bg: Option<wgpu::BindGroup>,
}

impl FlagBank {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        // Phase 4.2 fix: vanilla flags are at most 82×52 px. At 320 wide that's 3.9× upscale.
        // Linear filtering blurs the edges; NEAREST mag preserves the original pixel art
        // aesthetic (crisp 黑白十字 + 旗帜锐利边缘). Use NEAREST for upscale, Linear for downscale.
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("flag_sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });

        // Fallback: 1×1 mid-gray.
        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("flag_fallback"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &[80u8, 80, 80, 255],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: None,
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        let view = tex.create_view(&Default::default());
        let fallback = Arc::new(FlagTexture {
            texture: tex,
            view,
            width: 1,
            height: 1,
        });

        Self {
            cache: HashMap::new(),
            bind_groups: HashMap::new(),
            sampler,
            missing: std::collections::HashSet::new(),
            fallback,
            fallback_bg: None,
        }
    }

    pub fn sampler(&self) -> &wgpu::Sampler {
        &self.sampler
    }

    /// 注册 fallback bind group（在外部创建好 layout 后调一次）。
    pub fn init_fallback(
        &mut self,
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        uniform_buffer: &wgpu::Buffer,
    ) {
        let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("flag_fallback_bg"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&self.fallback.view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
        self.fallback_bg = Some(bg);
    }

    /// 尝试加载 (tag, ideology) 对应的国旗。失败回退到 fallback。
    pub fn get_or_load(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        path_cfg: &PathConfig,
        layout: &wgpu::BindGroupLayout,
        uniform_buffer: &wgpu::Buffer,
        tag: &str,
        ideology: &str,
    ) -> &wgpu::BindGroup {
        let key = (tag.to_ascii_uppercase(), ideology.to_ascii_lowercase());
        if self.bind_groups.contains_key(&key) {
            return self.bind_groups.get(&key).unwrap();
        }
        if self.missing.contains(&key) {
            return self.fallback_bg.as_ref().unwrap();
        }

        let mut loaded: Option<TgaImage> = hoi4_assets::generated_historical_flag(&key.0);

        // 候选路径：root <TAG>_<id>.tga (82×52, max res) → root <TAG>.tga →
        // medium <TAG>_<id>.tga (41×26, fallback) → medium <TAG>.tga
        let candidates = [
            format!("gfx/flags/{}_{}.tga", key.0, key.1),
            format!("gfx/flags/{}.tga", key.0),
            format!("gfx/flags/medium/{}_{}.tga", key.0, key.1),
            format!("gfx/flags/medium/{}.tga", key.0),
        ];

        let db = FsAssetDb::new(path_cfg.clone());
        if loaded.is_none() {
            for cand in &candidates {
                if let Ok(bytes) = db.open(cand.as_str()) {
                    match TgaImage::parse(&bytes) {
                        Ok(img) => {
                            loaded = Some(img);
                            break;
                        }
                        Err(e) => {
                            eprintln!("[flag] {} parse failed: {}", cand, e);
                        }
                    }
                }
            }
        }

        let img = match loaded {
            Some(i) => i,
            None => {
                self.missing.insert(key);
                return self.fallback_bg.as_ref().unwrap();
            }
        };

        // Upload as sRGB to match background blending.
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("flag"),
            size: wgpu::Extent3d {
                width: img.width,
                height: img.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
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
            &img.pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(img.width * 4),
                rows_per_image: None,
            },
            wgpu::Extent3d {
                width: img.width,
                height: img.height,
                depth_or_array_layers: 1,
            },
        );
        let view = texture.create_view(&Default::default());

        let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("flag_bg"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });

        let entry = Arc::new(FlagTexture {
            texture,
            view,
            width: img.width,
            height: img.height,
        });
        self.cache.insert(key.clone(), entry);
        self.bind_groups.insert(key.clone(), bg);
        self.bind_groups.get(&key).unwrap()
    }

    /// 尺寸（拿来按比例画 quad）。
    pub fn size(&self, tag: &str, ideology: &str) -> (u32, u32) {
        let key = (tag.to_ascii_uppercase(), ideology.to_ascii_lowercase());
        if let Some(t) = self.cache.get(&key) {
            (t.width, t.height)
        } else {
            (82, 52) // 默认 medium 尺寸
        }
    }
}
