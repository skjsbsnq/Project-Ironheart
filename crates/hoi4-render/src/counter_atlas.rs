//! CR-2.1.3 — HOI3 兵种 icon atlas 运行时加载器。
//!
//! 编译期 [`build.rs`](../../build.rs) 把 `assets/counter_icons/*.svg`
//! 栅格化为 `$OUT_DIR/counter_atlas.png`（1024×64 RGBA8）。本模块通过
//! `include_bytes!` 把 PNG 字节嵌入二进制，运行时解码并上传到 wgpu 纹理。
//!
//! ## 内存与性能
//!
//! - 嵌入字节：~24 KB（PNG 压缩后）。运行时解码 + 拷贝一次到 GPU，
//!   单次 ~1 ms。后续帧只采样、不再解码。
//! - GPU 占用：1024×64 RGBA8 = 256 KB。
//!
//! ## UV 坐标系
//!
//! Atlas 是一行 16 列 × 64 cell × 64 px 高。`uv_rect(arch)` 返回
//! `[u0, v0, u1, v1]`，其中 `u0 = arch_idx * 64 / 1024 = arch_idx / 16`，
//! `u1 = (arch_idx + 1) / 16`，`v0 = 0`，`v1 = 1`。
//!
//! ## 与 [`Hoi3CounterPass`] 的对接
//!
//! [`Hoi3CounterPass`] 在 CR-2.1.3（cont）持有 `CounterAtlas` 并把
//! `view + sampler` 绑到 `@group(1) @binding(0..1)`。FS 用 `archetype` 字段
//! 推算 atlas U 范围（无需 uniform；单 instance 即决定一个 cell）。

use crate::units::UnitArchetype;

/// 嵌入的 atlas PNG（1024×64 RGBA8 PNG，由 `build.rs` 生成）。
pub const COUNTER_ATLAS_PNG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/counter_atlas.png"));

/// Atlas 物理像素宽度（16 cell × 64 px）。
pub const ATLAS_W: u32 = 1024;
/// Atlas 物理像素高度（单行 64 px）。
pub const ATLAS_H: u32 = 64;
/// 每 archetype cell 的边长（正方形）。
pub const CELL_SIZE: u32 = 64;
/// Atlas 中 cell 数量（与 `UnitArchetype::COUNT` 一致）。
pub const CELL_COUNT: u32 = 16;

// 编译期 sanity：cell 总宽 = atlas 宽。删 archetype 就构建失败，强制同步。
const _: () = {
    assert!(ATLAS_W == CELL_SIZE * CELL_COUNT);
    assert!(CELL_COUNT == UnitArchetype::COUNT as u32);
};

/// 已上传到 GPU 的 atlas 纹理 + sampler 三元组。
///
/// `Hoi3CounterPass` 持有一份，整个程序生命周期内复用（atlas 是只读资产，
/// 不会动态更新）。
pub struct CounterAtlas {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
}

impl CounterAtlas {
    /// 解码嵌入的 PNG 字节并上传到一张 `Rgba8UnormSrgb` 纹理。
    ///
    /// `Rgba8UnormSrgb` 是与 HDR target（`Rgba16Float`）混合时的正确选择
    /// —— 采样后的值已经在 linear 空间。
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let img = image::load_from_memory(COUNTER_ATLAS_PNG)
            .expect("counter_atlas.png decode (build-time rasterization)")
            .to_rgba8();
        assert_eq!(
            img.width(),
            ATLAS_W,
            "atlas width mismatch (build.rs out of sync?)"
        );
        assert_eq!(
            img.height(),
            ATLAS_H,
            "atlas height mismatch (build.rs out of sync?)"
        );

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("counter_atlas"),
            size: wgpu::Extent3d {
                width: ATLAS_W,
                height: ATLAS_H,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            // sRGB：SVG 源是直接颜色（`#ffffff` 白），运行时 shader 混合到 HDR
            // 用 sRGB 视图自动 gamma 解码确保亮度一致。
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
            img.as_raw(),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(ATLAS_W * 4),
                rows_per_image: Some(ATLAS_H),
            },
            wgpu::Extent3d {
                width: ATLAS_W,
                height: ATLAS_H,
                depth_or_array_layers: 1,
            },
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("counter_atlas_sampler"),
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
        }
    }

    /// 计算 atlas 中第 N 个 cell 的 UV 矩形：`[u0, v0, u1, v1]`。
    ///
    /// 调用方保证 `archetype as u8 < CELL_COUNT`（`UnitArchetype` 是 0..15
    /// 的 enum，类型系统已保证）。
    #[inline]
    pub fn uv_rect(archetype: UnitArchetype) -> [f32; 4] {
        let idx = archetype as u32;
        let u0 = (idx * CELL_SIZE) as f32 / ATLAS_W as f32;
        let u1 = ((idx + 1) * CELL_SIZE) as f32 / ATLAS_W as f32;
        [u0, 0.0, u1, 1.0]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atlas_dimensions_consistent() {
        assert_eq!(ATLAS_W, CELL_SIZE * CELL_COUNT);
        assert_eq!(ATLAS_H, CELL_SIZE);
        assert_eq!(CELL_COUNT, UnitArchetype::COUNT as u32);
    }

    #[test]
    fn embedded_png_decodes_to_expected_size() {
        let img = image::load_from_memory(COUNTER_ATLAS_PNG)
            .expect("PNG should decode")
            .to_rgba8();
        assert_eq!(img.width(), ATLAS_W);
        assert_eq!(img.height(), ATLAS_H);
        // 4 字节/像素 RGBA。
        assert_eq!(img.as_raw().len(), (ATLAS_W * ATLAS_H * 4) as usize);
    }

    #[test]
    fn uv_rect_at_index_zero_starts_at_origin() {
        let [u0, v0, u1, v1] = CounterAtlas::uv_rect(UnitArchetype::Unknown);
        assert!((u0 - 0.0).abs() < 1e-6);
        assert!((v0 - 0.0).abs() < 1e-6);
        assert!((u1 - 1.0 / 16.0).abs() < 1e-6);
        assert!((v1 - 1.0).abs() < 1e-6);
    }

    #[test]
    fn uv_rect_at_last_index_ends_at_one() {
        let [u0, _v0, u1, _v1] = CounterAtlas::uv_rect(UnitArchetype::AntiAir);
        assert!((u0 - 15.0 / 16.0).abs() < 1e-6);
        assert!((u1 - 1.0).abs() < 1e-6);
    }

    #[test]
    fn uv_rects_are_contiguous_and_non_overlapping() {
        let archs = [
            UnitArchetype::Unknown,
            UnitArchetype::Infantry,
            UnitArchetype::Cavalry,
            UnitArchetype::Motorized,
            UnitArchetype::Mechanized,
            UnitArchetype::Mountain,
            UnitArchetype::Marine,
            UnitArchetype::Paratrooper,
            UnitArchetype::Militia,
            UnitArchetype::LightArmor,
            UnitArchetype::MediumArmor,
            UnitArchetype::HeavyArmor,
            UnitArchetype::ModernArmor,
            UnitArchetype::Artillery,
            UnitArchetype::AntiTank,
            UnitArchetype::AntiAir,
        ];
        let mut prev_u1 = 0.0;
        for arch in archs {
            let [u0, v0, u1, v1] = CounterAtlas::uv_rect(arch);
            assert!(u0 >= 0.0 && u1 <= 1.0, "uv out of bounds for {arch:?}");
            assert!(u0 < u1, "u0 < u1 for {arch:?}");
            assert!((v0 - 0.0).abs() < 1e-6);
            assert!((v1 - 1.0).abs() < 1e-6);
            assert!(
                (u0 - prev_u1).abs() < 1e-6,
                "cell start should equal previous cell end for {arch:?}: u0={u0}, prev_u1={prev_u1}"
            );
            prev_u1 = u1;
        }
        assert!((prev_u1 - 1.0).abs() < 1e-6);
    }

    #[test]
    fn uv_rect_is_idempotent() {
        // 调用 N 次必须返回相同结果（uv_rect 是纯函数）。
        let a = CounterAtlas::uv_rect(UnitArchetype::Infantry);
        let b = CounterAtlas::uv_rect(UnitArchetype::Infantry);
        assert_eq!(a, b);
    }
}
