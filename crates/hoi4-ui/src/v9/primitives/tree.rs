//! Tree layout helper for fixed-size node UIs.

use egui::{Pos2, Rect, Vec2};

#[derive(Debug, Clone, Copy)]
pub struct TreeLayout {
    pub node_size: Vec2,
    pub gap: Vec2,
}

impl TreeLayout {
    pub fn new(node_size: Vec2, gap: Vec2) -> Self {
        Self { node_size, gap }
    }

    pub fn phase_d() -> Self {
        Self::new(Vec2::splat(32.0), Vec2::new(126.0, 104.0))
    }

    pub fn canvas_size(&self, min_x: i32, max_x: i32, min_y: i32, max_y: i32, zoom: f32) -> Vec2 {
        let cols = (max_x - min_x + 1).max(1) as f32;
        let rows = (max_y - min_y + 1).max(1) as f32;
        Vec2::new(
            (cols * self.gap.x + self.node_size.x) * zoom,
            (rows * self.gap.y + self.node_size.y + 56.0) * zoom,
        )
    }

    pub fn node_rect(
        &self,
        origin: Pos2,
        min_x: i32,
        min_y: i32,
        position: (i32, i32),
        zoom: f32,
    ) -> Rect {
        let gx = (position.0 - min_x) as f32;
        let gy = (position.1 - min_y) as f32;
        let min = Pos2::new(
            origin.x + gx * self.gap.x * zoom,
            origin.y + gy * self.gap.y * zoom,
        );
        Rect::from_min_size(min, self.node_size * zoom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase_d_nodes_are_32_px() {
        assert_eq!(TreeLayout::phase_d().node_size, Vec2::splat(32.0));
    }
}
