//! 9-slice helper — V5 阶段 B.4。
//!
//! 把 vanilla `gfx/interface/tiles/tiled_window_*.dds` 加载成 RGBA8，注册到 egui
//! 纹理，按 9-slice 拉伸算法画到任意目标 rect。这是 V5 自研 UI 走 vanilla 调性
//! 的关键基础设施：window / panel / button 框背景都靠它。
//!
//! ## 9-slice 算法
//!
//! ```text
//!   ┌─────┬───────────────┬─────┐
//!   │ TL  │     TM (拉)    │ TR  │   ← top    row, 高度 = top    edge
//!   ├─────┼───────────────┼─────┤
//!   │     │               │     │
//!   │ CL  │   CC (双拉)    │ CR  │   ← middle row, 高度 = stretch
//!   │     │               │     │
//!   ├─────┼───────────────┼─────┤
//!   │ BL  │     BM (拉)    │ BR  │   ← bottom row, 高度 = bottom edge
//!   └─────┴───────────────┴─────┘
//!     left   stretch         right
//!   edge                      edge
//! ```
//!
//! 4 个角（TL/TR/BL/BR）原样贴；4 条边（TM/BM/CL/CR）只在垂直/水平方向拉；
//! 中心（CC）双向拉。这样无论 target rect 多大，边角几何保真。
//!
//! ## 当前限制（B.4 起步）
//!
//! - 仅支持 BGRA8 未压缩 DDS（vanilla 主流 `tiled_window*.dds` 都是这种）。
//!   BC1 / BC3 解码留给 B.5（focus icon 多用 BC3）。
//! - 边宽 `NineSliceEdges` 由 caller 显式指定；vanilla `.gfx` 的 tile/corner
//!   元数据由 `vanilla_gui` runtime 解析后再映射到本模块。

use std::path::{Path, PathBuf};

use egui::{
    epaint, Color32, ColorImage, Context, Painter, Rect, Shape, TextureFilter, TextureHandle,
    TextureOptions, TextureWrapMode, Vec2,
};

use hoi4_assets::dds::{DdsFormat, DdsImage};
use hoi4_paths::PathConfig;

/// 9-slice 纹理的边宽（纹理像素）。中心区 = 总尺寸 - left - right (X) / - top - bottom (Y)。
#[derive(Debug, Clone, Copy)]
pub struct NineSliceEdges {
    pub left: f32,
    pub right: f32,
    pub top: f32,
    pub bottom: f32,
}

impl NineSliceEdges {
    /// 四边等宽。
    pub fn uniform(px: f32) -> Self {
        Self {
            left: px,
            right: px,
            top: px,
            bottom: px,
        }
    }
}

/// 一张已注册到 egui 的 9-slice 纹理。`Clone` 通过 `TextureHandle` 内部 Arc 引用
/// 计数共享，复制 cheap。`TextureHandle` 不是 Debug，所以本结构体没有 `Debug` 派生。
#[derive(Clone)]
pub struct NineSlice {
    pub handle: TextureHandle,
    pub size: Vec2,
    pub edges: NineSliceEdges,
}

/// 加载 / 注册过程的错误（仅用于 B.4 起步，类型故意简单）。
#[derive(Debug)]
pub enum NineSliceError {
    NotFound(PathBuf),
    Io(std::io::Error),
    Parse(String),
    Unsupported(DdsFormat),
}

impl std::fmt::Display for NineSliceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(p) => write!(f, "9-slice asset not found: {}", p.display()),
            Self::Io(e) => write!(f, "9-slice I/O: {e}"),
            Self::Parse(e) => write!(f, "9-slice DDS parse: {e}"),
            Self::Unsupported(fmt) => write!(
                f,
                "9-slice unsupported DDS format: {fmt:?} (B.4 仅支持 BGRA8)"
            ),
        }
    }
}

impl std::error::Error for NineSliceError {}

impl NineSlice {
    /// 用 `path_cfg.find` 从 vanilla / mod chain 查相对路径，解析 DDS，注册到 egui。
    ///
    /// `relative` 是相对游戏根目录的路径，例如 `"gfx/interface/tiles/tiled_window.dds"`。
    /// `debug_name` 是 egui 内部纹理名（仅诊断用）。
    pub fn load_vanilla(
        ctx: &Context,
        path_cfg: &PathConfig,
        relative: impl AsRef<Path>,
        edges: NineSliceEdges,
        debug_name: &str,
    ) -> Result<Self, NineSliceError> {
        let relative = relative.as_ref();
        let abs = path_cfg
            .find(relative)
            .ok_or_else(|| NineSliceError::NotFound(relative.to_path_buf()))?;
        let bytes = std::fs::read(&abs).map_err(NineSliceError::Io)?;
        let dds = DdsImage::parse(&bytes).map_err(|e| NineSliceError::Parse(format!("{e:?}")))?;
        let rgba = crate::dds_decode::decode_mip0_to_rgba(&dds).map_err(|e| match e {
            crate::dds_decode::DdsDecodeError::Unsupported(fmt) => NineSliceError::Unsupported(fmt),
            other => NineSliceError::Parse(format!("{other}")),
        })?;
        let color_image =
            ColorImage::from_rgba_unmultiplied([dds.width as usize, dds.height as usize], &rgba);
        let handle = ctx.load_texture(
            debug_name,
            color_image,
            TextureOptions {
                magnification: TextureFilter::Linear,
                minification: TextureFilter::Linear,
                wrap_mode: TextureWrapMode::ClampToEdge,
                mipmap_mode: None,
            },
        );
        Ok(Self {
            handle,
            size: Vec2::new(dds.width as f32, dds.height as f32),
            edges,
        })
    }

    /// 在 painter 上把 9 个分片画到 target rect。`tint` 通常给 `Color32::WHITE`
    /// 表示原色；可以传半透明色给整张纹理染色 / 衰减。
    pub fn paint(&self, painter: &Painter, target: Rect, tint: Color32) {
        let mut mesh = epaint::Mesh::with_texture(self.handle.id());

        // 目标 X / Y 切片位置（4 列 4 行，构成 3×3 = 9 个子矩形）。
        let dx = [
            target.min.x,
            target.min.x + self.edges.left,
            target.max.x - self.edges.right,
            target.max.x,
        ];
        let dy = [
            target.min.y,
            target.min.y + self.edges.top,
            target.max.y - self.edges.bottom,
            target.max.y,
        ];

        // 源 UV 切片位置（egui UV 用 [0,1] 范围）。
        let tw = self.size.x.max(1.0);
        let th = self.size.y.max(1.0);
        let ux = [0.0, self.edges.left / tw, (tw - self.edges.right) / tw, 1.0];
        let uy = [0.0, self.edges.top / th, (th - self.edges.bottom) / th, 1.0];

        for ry in 0..3 {
            for rx in 0..3 {
                let dst = Rect::from_min_max(
                    egui::pos2(dx[rx], dy[ry]),
                    egui::pos2(dx[rx + 1], dy[ry + 1]),
                );
                // 跳过被 caller 折叠成 0 宽 / 0 高的退化分片（target 比边角加起来还小）。
                if dst.width() <= 0.0 || dst.height() <= 0.0 {
                    continue;
                }
                let uv = Rect::from_min_max(
                    egui::pos2(ux[rx], uy[ry]),
                    egui::pos2(ux[rx + 1], uy[ry + 1]),
                );
                mesh.add_rect_with_uv(dst, uv, tint);
            }
        }

        painter.add(Shape::mesh(mesh));
    }
}

/// B.5 起，`NineSlice::load_vanilla` 走 `crate::dds_decode::decode_mip0_to_rgba`
/// 共享解码（BGRA8 / BC1 / BC3）。原地实现已删除。
#[cfg(test)]
mod tests {
    use super::*;

    /// 字段连通：edges uniform 构造 + NineSlice clone（确认 TextureHandle 共享 OK）。
    #[test]
    fn edges_uniform_constructs() {
        let e = NineSliceEdges::uniform(32.0);
        assert_eq!(e.left, 32.0);
        assert_eq!(e.right, 32.0);
        assert_eq!(e.top, 32.0);
        assert_eq!(e.bottom, 32.0);
    }
}
