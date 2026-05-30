//! V9 布局系统：栅格 / 分栏 / 锚定。
//!
//! 这是 V9 反"egui 决定尺寸"的核心：调用方不再用 `ui.horizontal/vertical` 流式
//! 排版，而是声明 `GridLayout::new(rows, cols)`，调用 `measure(container)` 拿到
//! 每个 cell 的 `Rect`，再用 `ui.allocate_rect(cell)` 拿到 `Painter` 自绘。
//!
//! ## 三种布局元件
//!
//! - [`GridLayout`] — 行列网格，支持 `Fixed(px)` 与 `Fr(weight)` 弹性混合。
//! - [`SplitLayout`] — 左右 / 上下 二分栏。
//! - [`AnchorLayout`] — 锚定到容器的 9 个位置（用于模态 / 通知 / hover 卡）。

use egui::{Pos2, Rect, Vec2};

// ─── Track ───────────────────────────────────────────────────────

/// 网格行 / 列尺寸定义。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Track {
    /// 固定像素。
    Fixed(f32),
    /// 弹性份数（按 weight 占用剩余空间）。
    Fr(f32),
}

impl Track {
    fn fixed_px(self) -> f32 {
        match self {
            Self::Fixed(px) => px,
            Self::Fr(_) => 0.0,
        }
    }

    fn fr_weight(self) -> f32 {
        match self {
            Self::Fixed(_) => 0.0,
            Self::Fr(w) => w,
        }
    }
}

// ─── GridLayout ──────────────────────────────────────────────────

/// 行列网格。`measure(container)` 在面板进入时调用**一次**，结果 cache 到
/// `Vec<Vec<Rect>>`，子项绘制走 `cell(layout, row, col)` 读 `Rect`。
#[derive(Debug, Clone)]
pub struct GridLayout {
    pub rows: Vec<Track>,
    pub cols: Vec<Track>,
    pub gutter_x: f32,
    pub gutter_y: f32,
}

impl GridLayout {
    /// 构造无 gutter 的网格。
    pub fn new(rows: Vec<Track>, cols: Vec<Track>) -> Self {
        Self {
            rows,
            cols,
            gutter_x: 0.0,
            gutter_y: 0.0,
        }
    }

    pub fn with_gutter(mut self, x: f32, y: f32) -> Self {
        self.gutter_x = x;
        self.gutter_y = y;
        self
    }

    /// 测量：根据容器 `Rect` 把 `Track` 展开为像素 `Rect`，返回 `cells[row][col]`。
    ///
    /// `Fixed` 取字面尺寸；`Fr` 按 weight 瓜分扣掉 `Fixed` + `gutter` 后剩余空间。
    /// 如果剩余空间为负数，`Fr` cell 尺寸为 0。
    pub fn measure(&self, container: Rect) -> Vec<Vec<Rect>> {
        let col_widths = resolve_tracks(&self.cols, container.width(), self.gutter_x);
        let row_heights = resolve_tracks(&self.rows, container.height(), self.gutter_y);

        let mut cells: Vec<Vec<Rect>> = Vec::with_capacity(self.rows.len());
        let mut y = container.min.y;
        for (ri, &rh) in row_heights.iter().enumerate() {
            let mut row_cells = Vec::with_capacity(self.cols.len());
            let mut x = container.min.x;
            for (ci, &cw) in col_widths.iter().enumerate() {
                row_cells.push(Rect::from_min_size(Pos2::new(x, y), Vec2::new(cw, rh)));
                x += cw;
                if ci + 1 < col_widths.len() {
                    x += self.gutter_x;
                }
            }
            cells.push(row_cells);
            y += rh;
            if ri + 1 < row_heights.len() {
                y += self.gutter_y;
            }
        }
        cells
    }

    /// 取单个 cell 的 `Rect`。out-of-bounds 返回 `Rect::NOTHING`。
    pub fn cell(layout: &[Vec<Rect>], row: usize, col: usize) -> Rect {
        layout
            .get(row)
            .and_then(|r| r.get(col))
            .copied()
            .unwrap_or(Rect::NOTHING)
    }

    /// 跨多个 cell 的合并 `Rect`。
    pub fn span(
        layout: &[Vec<Rect>],
        row: usize,
        col: usize,
        rowspan: usize,
        colspan: usize,
    ) -> Rect {
        if rowspan == 0 || colspan == 0 {
            return Rect::NOTHING;
        }
        let tl = Self::cell(layout, row, col);
        let br = Self::cell(layout, row + rowspan - 1, col + colspan - 1);
        if tl == Rect::NOTHING || br == Rect::NOTHING {
            return Rect::NOTHING;
        }
        Rect::from_min_max(tl.min, br.max)
    }
}

/// 把 `Track` 列表展开为像素长度数组。
fn resolve_tracks(tracks: &[Track], total: f32, gutter: f32) -> Vec<f32> {
    if tracks.is_empty() {
        return Vec::new();
    }
    let fixed_total: f32 = tracks.iter().map(|t| t.fixed_px()).sum();
    let fr_total: f32 = tracks.iter().map(|t| t.fr_weight()).sum();
    let gutter_total = gutter * (tracks.len() as f32 - 1.0).max(0.0);
    let remaining = (total - fixed_total - gutter_total).max(0.0);

    tracks
        .iter()
        .map(|t| match *t {
            Track::Fixed(px) => px,
            Track::Fr(w) if fr_total > 0.0 => remaining * (w / fr_total),
            Track::Fr(_) => 0.0,
        })
        .collect()
}

// ─── SplitLayout ─────────────────────────────────────────────────

/// 分割轴。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitAxis {
    Horizontal,
    Vertical,
}

/// 二分栏。`split` 可以是 `Fixed(px)`（左/上侧固定像素）或 `Fr(0..1)`（左/上侧比例）。
#[derive(Debug, Clone, Copy)]
pub struct SplitLayout {
    pub axis: SplitAxis,
    pub split: Track,
    pub gutter: f32,
}

impl SplitLayout {
    pub fn new(axis: SplitAxis, split: Track) -> Self {
        Self {
            axis,
            split,
            gutter: 0.0,
        }
    }

    pub fn with_gutter(mut self, gutter: f32) -> Self {
        self.gutter = gutter;
        self
    }

    /// 测量：返回 `(left_or_top, right_or_bottom)` 两段 `Rect`。
    pub fn measure(&self, container: Rect) -> (Rect, Rect) {
        let (total, primary_size) = match self.axis {
            SplitAxis::Horizontal => (container.width(), self.primary_px(container.width())),
            SplitAxis::Vertical => (container.height(), self.primary_px(container.height())),
        };
        let secondary = (total - primary_size - self.gutter).max(0.0);
        match self.axis {
            SplitAxis::Horizontal => {
                let left =
                    Rect::from_min_size(container.min, Vec2::new(primary_size, container.height()));
                let right = Rect::from_min_size(
                    Pos2::new(
                        container.min.x + primary_size + self.gutter,
                        container.min.y,
                    ),
                    Vec2::new(secondary, container.height()),
                );
                (left, right)
            }
            SplitAxis::Vertical => {
                let top =
                    Rect::from_min_size(container.min, Vec2::new(container.width(), primary_size));
                let bottom = Rect::from_min_size(
                    Pos2::new(
                        container.min.x,
                        container.min.y + primary_size + self.gutter,
                    ),
                    Vec2::new(container.width(), secondary),
                );
                (top, bottom)
            }
        }
    }

    fn primary_px(&self, total: f32) -> f32 {
        match self.split {
            Track::Fixed(px) => px.min(total),
            Track::Fr(ratio) => (total - self.gutter).max(0.0) * ratio.clamp(0.0, 1.0),
        }
    }
}

// ─── AnchorLayout ────────────────────────────────────────────────

/// 9 锚点（用于模态 / 通知 / 悬浮卡）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnchorLayout {
    TopLeft,
    TopCenter,
    TopRight,
    CenterLeft,
    Center,
    CenterRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

impl AnchorLayout {
    /// 把 `size` 大小的盒子锚定到 `container` 的对应位置，加 `margin` 偏移。
    pub fn place(self, container: Rect, size: Vec2, margin: Vec2) -> Rect {
        let x = match self {
            Self::TopLeft | Self::CenterLeft | Self::BottomLeft => container.min.x + margin.x,
            Self::TopCenter | Self::Center | Self::BottomCenter => {
                container.center().x - size.x * 0.5
            }
            Self::TopRight | Self::CenterRight | Self::BottomRight => {
                container.max.x - size.x - margin.x
            }
        };
        let y = match self {
            Self::TopLeft | Self::TopCenter | Self::TopRight => container.min.y + margin.y,
            Self::CenterLeft | Self::Center | Self::CenterRight => {
                container.center().y - size.y * 0.5
            }
            Self::BottomLeft | Self::BottomCenter | Self::BottomRight => {
                container.max.y - size.y - margin.y
            }
        };
        Rect::from_min_size(Pos2::new(x, y), size)
    }
}

// ─── Helpers ─────────────────────────────────────────────────────

/// 物理像素对齐。把逻辑像素 `value` 乘以 `pixels_per_point` 后 round 回逻辑像素，
/// 消除 1px 描边在非整数 DPI 下的亚像素抖动。
pub fn snap_px(value: f32, pixels_per_point: f32) -> f32 {
    (value * pixels_per_point).round() / pixels_per_point
}

/// 把 `rect` 的 min/max 都做像素吸附。
pub fn snap_rect(rect: Rect, pixels_per_point: f32) -> Rect {
    Rect::from_min_max(
        Pos2::new(
            snap_px(rect.min.x, pixels_per_point),
            snap_px(rect.min.y, pixels_per_point),
        ),
        Pos2::new(
            snap_px(rect.max.x, pixels_per_point),
            snap_px(rect.max.y, pixels_per_point),
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, h))
    }

    #[test]
    fn grid_all_fixed() {
        let g = GridLayout::new(
            vec![Track::Fixed(40.0), Track::Fixed(60.0)],
            vec![Track::Fixed(50.0), Track::Fixed(100.0)],
        );
        let cells = g.measure(r(0.0, 0.0, 200.0, 100.0));
        assert_eq!(cells.len(), 2);
        assert_eq!(cells[0].len(), 2);
        assert_eq!(cells[0][0], r(0.0, 0.0, 50.0, 40.0));
        assert_eq!(cells[0][1], r(50.0, 0.0, 100.0, 40.0));
        assert_eq!(cells[1][0], r(0.0, 40.0, 50.0, 60.0));
        assert_eq!(cells[1][1], r(50.0, 40.0, 100.0, 60.0));
    }

    #[test]
    fn grid_all_fr_equal_weights() {
        let g = GridLayout::new(
            vec![Track::Fr(1.0), Track::Fr(1.0)],
            vec![Track::Fr(1.0), Track::Fr(1.0)],
        );
        let cells = g.measure(r(0.0, 0.0, 100.0, 80.0));
        assert_eq!(cells[0][0], r(0.0, 0.0, 50.0, 40.0));
        assert_eq!(cells[0][1], r(50.0, 0.0, 50.0, 40.0));
        assert_eq!(cells[1][0], r(0.0, 40.0, 50.0, 40.0));
        assert_eq!(cells[1][1], r(50.0, 40.0, 50.0, 40.0));
    }

    #[test]
    fn grid_fr_unequal_weights() {
        let g = GridLayout::new(
            vec![Track::Fixed(100.0)],
            vec![Track::Fr(1.0), Track::Fr(3.0)],
        );
        let cells = g.measure(r(0.0, 0.0, 200.0, 100.0));
        // 1+3=4 weights, 200 px → 50 / 150
        assert_eq!(cells[0][0], r(0.0, 0.0, 50.0, 100.0));
        assert_eq!(cells[0][1], r(50.0, 0.0, 150.0, 100.0));
    }

    #[test]
    fn grid_mixed_fixed_fr() {
        let g = GridLayout::new(
            vec![Track::Fixed(50.0)],
            vec![Track::Fixed(80.0), Track::Fr(1.0), Track::Fixed(40.0)],
        );
        let cells = g.measure(r(0.0, 0.0, 200.0, 50.0));
        // Fr 拿剩余 = 200 - 80 - 40 = 80
        assert_eq!(cells[0][0].width(), 80.0);
        assert_eq!(cells[0][1].width(), 80.0);
        assert_eq!(cells[0][2].width(), 40.0);
    }

    #[test]
    fn grid_with_gutter() {
        let g = GridLayout::new(
            vec![Track::Fr(1.0), Track::Fr(1.0)],
            vec![Track::Fr(1.0), Track::Fr(1.0)],
        )
        .with_gutter(10.0, 8.0);
        let cells = g.measure(r(0.0, 0.0, 110.0, 88.0));
        // cols: 100 / 2 = 50 each, gutter 10 in middle
        // rows: 80 / 2 = 40 each, gutter 8 in middle
        assert_eq!(cells[0][0], r(0.0, 0.0, 50.0, 40.0));
        assert_eq!(cells[0][1], r(60.0, 0.0, 50.0, 40.0));
        assert_eq!(cells[1][0], r(0.0, 48.0, 50.0, 40.0));
        assert_eq!(cells[1][1], r(60.0, 48.0, 50.0, 40.0));
    }

    #[test]
    fn grid_overflow_no_fr_returns_fixed() {
        // 容器宽 = 100，但 fixed 之和 = 150。Fixed 仍按字面取，不缩。
        let g = GridLayout::new(
            vec![Track::Fixed(50.0)],
            vec![Track::Fixed(80.0), Track::Fixed(70.0)],
        );
        let cells = g.measure(r(0.0, 0.0, 100.0, 50.0));
        assert_eq!(cells[0][0].width(), 80.0);
        assert_eq!(cells[0][1].width(), 70.0);
    }

    #[test]
    fn grid_fr_collapse_to_zero_when_no_space() {
        let g = GridLayout::new(
            vec![Track::Fixed(50.0)],
            vec![Track::Fixed(120.0), Track::Fr(1.0)],
        );
        let cells = g.measure(r(0.0, 0.0, 100.0, 50.0));
        // remaining = 100 - 120 = negative，clamp 到 0
        assert_eq!(cells[0][0].width(), 120.0);
        assert_eq!(cells[0][1].width(), 0.0);
    }

    #[test]
    fn grid_empty_returns_empty_layout() {
        let g = GridLayout::new(vec![], vec![]);
        let cells = g.measure(r(0.0, 0.0, 100.0, 100.0));
        assert!(cells.is_empty());
    }

    #[test]
    fn grid_cell_out_of_bounds_returns_nothing() {
        let g = GridLayout::new(vec![Track::Fixed(10.0)], vec![Track::Fixed(10.0)]);
        let cells = g.measure(r(0.0, 0.0, 10.0, 10.0));
        assert_eq!(GridLayout::cell(&cells, 5, 5), Rect::NOTHING);
        assert_eq!(GridLayout::cell(&cells, 0, 0), r(0.0, 0.0, 10.0, 10.0));
    }

    #[test]
    fn grid_span_merges_cells() {
        let g = GridLayout::new(
            vec![Track::Fixed(20.0), Track::Fixed(30.0)],
            vec![Track::Fixed(40.0), Track::Fixed(50.0)],
        );
        let cells = g.measure(r(0.0, 0.0, 90.0, 50.0));
        let span = GridLayout::span(&cells, 0, 0, 2, 2);
        assert_eq!(span, r(0.0, 0.0, 90.0, 50.0));
    }

    #[test]
    fn split_horizontal_fixed() {
        let s = SplitLayout::new(SplitAxis::Horizontal, Track::Fixed(120.0));
        let (left, right) = s.measure(r(0.0, 0.0, 400.0, 100.0));
        assert_eq!(left, r(0.0, 0.0, 120.0, 100.0));
        assert_eq!(right, r(120.0, 0.0, 280.0, 100.0));
    }

    #[test]
    fn split_horizontal_ratio_with_gutter() {
        let s = SplitLayout::new(SplitAxis::Horizontal, Track::Fr(0.3)).with_gutter(10.0);
        let (left, right) = s.measure(r(0.0, 0.0, 100.0, 50.0));
        // primary = (100 - 10) * 0.3 = 27
        assert!((left.width() - 27.0).abs() < 0.001);
        assert!((right.width() - (100.0 - 27.0 - 10.0)).abs() < 0.001);
    }

    #[test]
    fn split_vertical_fixed() {
        let s = SplitLayout::new(SplitAxis::Vertical, Track::Fixed(40.0));
        let (top, bottom) = s.measure(r(0.0, 0.0, 100.0, 200.0));
        assert_eq!(top, r(0.0, 0.0, 100.0, 40.0));
        assert_eq!(bottom, r(0.0, 40.0, 100.0, 160.0));
    }

    #[test]
    fn anchor_top_left() {
        let placed = AnchorLayout::TopLeft.place(
            r(0.0, 0.0, 200.0, 100.0),
            Vec2::new(50.0, 30.0),
            Vec2::ZERO,
        );
        assert_eq!(placed.min, Pos2::new(0.0, 0.0));
        assert_eq!(placed.size(), Vec2::new(50.0, 30.0));
    }

    #[test]
    fn anchor_center() {
        let placed = AnchorLayout::Center.place(
            r(0.0, 0.0, 200.0, 100.0),
            Vec2::new(60.0, 40.0),
            Vec2::ZERO,
        );
        assert_eq!(placed.min, Pos2::new(70.0, 30.0));
    }

    #[test]
    fn anchor_bottom_right_with_margin() {
        let placed = AnchorLayout::BottomRight.place(
            r(0.0, 0.0, 200.0, 100.0),
            Vec2::new(60.0, 40.0),
            Vec2::new(8.0, 6.0),
        );
        assert_eq!(
            placed.min,
            Pos2::new(200.0 - 60.0 - 8.0, 100.0 - 40.0 - 6.0)
        );
    }

    #[test]
    fn snap_px_rounds_to_physical_pixel() {
        assert_eq!(snap_px(10.4, 1.0), 10.0);
        assert_eq!(snap_px(10.5, 1.0), 11.0); // banker's rounding edge — f32 round
        assert_eq!(snap_px(10.4, 2.0), 10.5); // 10.4 * 2 = 20.8 → 21 → / 2 = 10.5
        assert_eq!(snap_px(10.3, 1.25), 10.4); // 12.875 → 13 → /1.25 = 10.4
    }
}
