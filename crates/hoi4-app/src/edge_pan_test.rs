use crate::*;

pub(crate) struct EdgePanTestConfig {
    pub(crate) country_tag: String,
    pub(crate) duration_secs: f32,
}

pub(crate) struct EdgePanTestRun {
    config: EdgePanTestConfig,
    started_at: Option<Instant>,
    last_cursor_log_at: Instant,
    frames: u32,
    slow_frames: u32,
    max_frame_ms: f32,
    max_surface_ms: f32,
    max_present_ms: f32,
    max_egui_ms: f32,
    max_vanilla_ms: f32,
}

impl EdgePanTestRun {
    pub(crate) fn new(config: EdgePanTestConfig) -> Self {
        Self {
            config,
            started_at: None,
            last_cursor_log_at: Instant::now(),
            frames: 0,
            slow_frames: 0,
            max_frame_ms: 0.0,
            max_surface_ms: 0.0,
            max_present_ms: 0.0,
            max_egui_ms: 0.0,
            max_vanilla_ms: 0.0,
        }
    }
}

impl App {
    pub(crate) fn start_edge_pan_test(&mut self) {
        let Some(run) = self.audit.edge_pan_test.as_ref() else {
            return;
        };
        if run.started_at.is_some() {
            return;
        }
        let country_tag = run.config.country_tag.clone();
        let duration_secs = run.config.duration_secs.max(1.0);

        let _ = self.set_player_country_by_tag(&country_tag);
        self.view.game_phase = GamePhase::Playing;
        self.world.speed = GameSpeed::Speed5;
        self.time_accumulator = 0.0;
        self.close_primary_panel();
        self.ui_state.active_popup = None;
        self.ui_state.province_info_card.open = false;
        self.ui_state.country_info_panel.close();
        self.reset_menu_state();

        let now = Instant::now();
        if let Some(run) = self.audit.edge_pan_test.as_mut() {
            run.config.duration_secs = duration_secs;
            run.started_at = Some(now);
            run.last_cursor_log_at = now;
        }
        self.update_edge_pan_test_cursor(now);
        println!(
            "[edge-pan-test] started country={} speed=5 duration={:.1}s",
            country_tag, duration_secs
        );
        if let Some(s) = &self.state {
            s.window.request_redraw();
        }
    }

    pub(crate) fn update_edge_pan_test_cursor(&mut self, now: Instant) {
        let Some(started_at) = self.audit.edge_pan_test.as_ref().and_then(|run| run.started_at) else {
            return;
        };
        if self.view.game_phase != GamePhase::Playing {
            return;
        }
        if self.world.speed != GameSpeed::Speed5 {
            self.world.speed = GameSpeed::Speed5;
            self.ui_state.pre_event_speed = None;
        }
        let Some((w, h)) = self.state.as_ref().map(|s| {
            let dpi = s.window.scale_factor() as f32;
            (
                s.config.width as f32 / dpi.max(0.0001),
                s.config.height as f32 / dpi.max(0.0001),
            )
        }) else {
            return;
        };

        let edge = (EDGE_PAN_MARGIN_PX * 0.5).clamp(2.0, 24.0);
        let elapsed = now.saturating_duration_since(started_at).as_secs_f32();
        let segment_secs = 2.0;
        let segment = (elapsed / segment_secs).floor() as u32 % 4;
        let phase = (elapsed / segment_secs).fract();
        let travel_x = edge + phase * (w - edge * 2.0).max(1.0);
        let travel_y = edge + phase * (h - edge * 2.0).max(1.0);
        self.last_mouse = match segment {
            0 => [w - edge, travel_y],
            1 => [w - travel_x, h - edge],
            2 => [edge, h - travel_y],
            _ => [travel_x, edge],
        };

        if now.duration_since(self.last_hover_pick_at).as_millis() >= HOVER_PICK_INTERVAL_MS {
            self.last_hover_pick_at = now;
            if !self.ui_blocks_map_clicks() {
                let new_hover = self.pick_province_at_cursor();
                if new_hover != self.hovered_province_id {
                    self.hovered_province_id = new_hover;
                }
            }
        }

        let mut should_log = false;
        if let Some(run) = self.audit.edge_pan_test.as_mut() {
            if now
                .saturating_duration_since(run.last_cursor_log_at)
                .as_secs_f32()
                >= 5.0
            {
                run.last_cursor_log_at = now;
                should_log = true;
            }
        }
        if should_log {
            println!(
                "[edge-pan-test] t={:.1}s cursor=({:.0},{:.0}) date={} speed=5",
                elapsed, self.last_mouse[0], self.last_mouse[1], self.world.date
            );
        }
    }

    pub(crate) fn edge_pan_test_should_finish(&self, now: Instant) -> bool {
        self.audit.edge_pan_test
            .as_ref()
            .and_then(|run| run.started_at.map(|started| (run, started)))
            .is_some_and(|(run, started)| {
                now.saturating_duration_since(started).as_secs_f32()
                    >= run.config.duration_secs.max(1.0)
            })
    }

    pub(crate) fn finish_edge_pan_test(&mut self) {
        let Some(run) = self.audit.edge_pan_test.take() else {
            return;
        };
        let elapsed = run
            .started_at
            .map(|started| started.elapsed().as_secs_f32())
            .unwrap_or(0.0);
        println!(
            "[edge-pan-test] finished elapsed={:.1}s frames={} slow_frames={} max_frame={:.2}ms max_surface={:.2}ms max_present={:.2}ms max_egui={:.2}ms max_vanilla={:.2}ms date={}",
            elapsed,
            run.frames,
            run.slow_frames,
            run.max_frame_ms,
            run.max_surface_ms,
            run.max_present_ms,
            run.max_egui_ms,
            run.max_vanilla_ms,
            self.world.date,
        );
    }

    pub(crate) fn record_edge_pan_test_frame(
        &mut self,
        frame_ms: f32,
        surface_ms: f32,
        present_ms: f32,
        egui_ms: f32,
        vanilla_ms: f32,
    ) {
        let Some(run) = self.audit.edge_pan_test.as_mut() else {
            return;
        };
        if run.started_at.is_none() {
            return;
        }
        run.frames = run.frames.saturating_add(1);
        if frame_ms >= 25.0 {
            run.slow_frames = run.slow_frames.saturating_add(1);
        }
        run.max_frame_ms = run.max_frame_ms.max(frame_ms);
        run.max_surface_ms = run.max_surface_ms.max(surface_ms);
        run.max_present_ms = run.max_present_ms.max(present_ms);
        run.max_egui_ms = run.max_egui_ms.max(egui_ms);
        run.max_vanilla_ms = run.max_vanilla_ms.max(vanilla_ms);
    }
}
