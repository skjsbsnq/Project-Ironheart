//! Phase 3.12.1 — 离屏 HDR 主 RT。
//!
//! 把"主 3D 渲染先写入一张 RGBA16Float 离屏纹理，再经过后处理 / blit 落到
//! 交换链"的所有 GPU 状态打包到一个小 struct，方便 main.rs 在 init / resize
//! 两处都只调一次。
//!
//! ## 为什么是 `Rgba16Float`
//!
//! - 颜色范围 ≫ 1.0：bloom / 自动曝光需要保留原始亮度信息
//! - 16-bit 半精度：dx12 / vulkan 普遍支持作为 storage + render attachment
//! - filterable：bilinear 采样在所有目标后端均可用（无需 `FLOAT32_FILTERABLE`
//!   feature）
//!
//! ## 与交换链的关系
//!
//! - 3D pass：写 HDR（**不**做 sRGB 编码，保留线性高动态范围）
//! - PostProcessChain：sample HDR → 写交换链（`Bgra8UnormSrgb`，自动 sRGB 编码）
//! - 简化 blit（3.12.1 临时桥接）：sample HDR → 写交换链（passthrough 同样的
//!   线性值；交换链格式自动做 sRGB 编码 → 与原"3D 直接渲到 sRGB"等价）

/// 离屏 HDR 主 RT。
pub struct HdrTarget {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub width: u32,
    pub height: u32,
}

impl HdrTarget {
    /// 创建（首次或 resize 后）。
    ///
    /// `width` / `height` 必须 ≥ 1（main.rs 已经做 `.max(1)`，这里再 clamp 一次防御）。
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let w = width.max(1);
        let h = height.max(1);
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("hdr_main_rt"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: super::HDR_FORMAT,
            // RENDER_ATTACHMENT 让 3D pipeline 写入；TEXTURE_BINDING 让后处理 / blit 采样。
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        Self {
            texture,
            view,
            width: w,
            height: h,
        }
    }

    /// 当前尺寸是否匹配 swap-chain 配置。Resize 时检查后再决定是否重建。
    pub fn matches_size(&self, width: u32, height: u32) -> bool {
        self.width == width.max(1) && self.height == height.max(1)
    }
}
