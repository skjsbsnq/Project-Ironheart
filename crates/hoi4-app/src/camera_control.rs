use crate::*;

impl App {
    pub(crate) fn world_size(&self) -> Vec2 {
        Vec2::new(
            self.world.map.province_map.width as f32 * WORLD_SCALE,
            self.world.map.province_map.height as f32 * WORLD_SCALE,
        )
    }

    pub(crate) fn upload_camera(&self) {
        let s = match &self.state {
            Some(s) => s,
            None => return,
        };
        let cam = CameraUniform::from_camera(&self.camera, HEIGHT_SCALE, LAT_CORRECTION);
        s.queue
            .write_buffer(&s.camera_buffer, 0, bytemuck::bytes_of(&cam));
    }

    pub(crate) fn cursor_ndc_at(&self, x: f32, y: f32) -> Option<Vec2> {
        let s = self.state.as_ref()?;
        let dpi = s.window.scale_factor() as f32;
        let w = s.config.width as f32 / dpi.max(0.0001);
        let h = s.config.height as f32 / dpi.max(0.0001);
        if w <= 0.0 || h <= 0.0 {
            return None;
        }
        Some(Vec2::new((x / w) * 2.0 - 1.0, 1.0 - (y / h) * 2.0))
    }

    pub(crate) fn ground_point_at_cursor(&self, x: f32, y: f32) -> Option<Vec2> {
        let ndc = self.cursor_ndc_at(x, y)?;
        self.camera.pick_world_xz_at_height(ndc, HEIGHT_SCALE * 0.3)
    }

    pub(crate) fn zoom_at_cursor(&mut self, factor: f32) {
        let base_distance = self
            .smooth_zoom_target_distance
            .unwrap_or(self.camera.distance);
        self.smooth_zoom_target_distance =
            Some(self.camera.clamped_distance(base_distance * factor));
        self.smooth_zoom_anchor_mouse = self.last_mouse;
        if let Some(s) = &self.state {
            s.window.request_redraw();
        }
    }

    pub(crate) fn update_smooth_zoom(&mut self, dt: f32) {
        let Some(target_distance) = self.smooth_zoom_target_distance else {
            return;
        };
        if dt <= 0.0 {
            return;
        }

        let [mx, my] = self.smooth_zoom_anchor_mouse;
        let before = self.ground_point_at_cursor(mx, my);
        let response = 1.0 - (-SMOOTH_ZOOM_RESPONSE * dt).exp();
        let mut next_distance =
            self.camera.distance + (target_distance - self.camera.distance) * response;
        let snap_epsilon = (target_distance * 0.001).max(0.01);
        if (target_distance - next_distance).abs() <= snap_epsilon {
            next_distance = target_distance;
            self.smooth_zoom_target_distance = None;
        }

        self.camera.distance = self.camera.clamped_distance(next_distance);
        if let Some(before) = before {
            if let Some(after) = self.ground_point_at_cursor(mx, my) {
                self.camera.pan(before.x - after.x, before.y - after.y);
            }
        }
        self.upload_camera();
        if let Some(s) = &self.state {
            s.window.request_redraw();
        }
    }
}
