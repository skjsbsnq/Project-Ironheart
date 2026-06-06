use std::sync::Arc;
use std::time::{Duration, Instant};

use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

use crate::*;

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        // 4.1.bis.6: vanilla `.gui` coordinates target 1920x1080. Defaulting to a
        // smaller logical size forced reference-resolution down-scaling on every
        // launch, blurring text and crowding widgets. Use the design size by default.
        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title("HOI4 Rust 3D")
                        .with_inner_size(winit::dpi::LogicalSize::new(1920, 1080))
                        // 4.1.bis.6 fix (2026-05-16): clamp minimum size so the
                        // OS can shrink the window to fit the desktop without
                        // dropping below `.gui` reference resolution targets.
                        .with_min_inner_size(winit::dpi::LogicalSize::new(1280, 720)),
                )
                .unwrap(),
        );
        self.init_render(window);
        self.start_edge_pan_test();
        if let Some(s) = &self.state {
            s.window.request_redraw();
        }
        self.update_title();
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.map_phase0_finished() {
            event_loop.exit();
            return;
        }
        let now = Instant::now();
        if self.edge_pan_test_should_finish(now) {
            self.finish_edge_pan_test();
            event_loop.exit();
            return;
        }
        let simulation_running =
            self.view.game_phase == GamePhase::Playing && self.world.speed != GameSpeed::Paused;

        let time_since_redraw = now.saturating_duration_since(self.last_redraw_at);
        let redraw_due =
            self.state.is_some() && time_since_redraw.as_secs_f32() >= TARGET_UI_FRAME_SECS;
        if redraw_due {
            let sim_budget = if simulation_running {
                REDRAW_OVERDUE_SIM_BUDGET_SECS
            } else {
                0.0
            };
            self.update(sim_budget);
            if let Some(s) = &self.state {
                s.window.request_redraw();
            }
            event_loop.set_control_flow(ControlFlow::Poll);
            return;
        }

        let sim_budget_secs = if simulation_running {
            (TARGET_UI_FRAME_SECS - time_since_redraw.as_secs_f32() - REDRAW_GUARD_SECS).max(0.0)
        } else {
            f32::INFINITY
        };

        if simulation_running && self.state.is_some() && sim_budget_secs < MIN_SIM_SLICE_SECS {
            self.update(0.0);
            if let Some(s) = &self.state {
                s.window.request_redraw();
            }
            event_loop.set_control_flow(ControlFlow::Poll);
            return;
        }

        self.update(sim_budget_secs);

        let redraw_after_update = self.audit.map_phase0.is_some()
            || Instant::now()
                .saturating_duration_since(self.last_redraw_at)
                .as_secs_f32()
                >= TARGET_UI_FRAME_SECS;
        if redraw_after_update {
            if let Some(s) = &self.state {
                s.window.request_redraw();
            }
        }

        if simulation_running || self.audit.map_phase0.is_some() || redraw_after_update {
            event_loop.set_control_flow(ControlFlow::Poll);
        } else {
            let now = Instant::now();
            let next_redraw_at =
                self.last_redraw_at + Duration::from_secs_f32(TARGET_UI_FRAME_SECS);
            event_loop.set_control_flow(ControlFlow::WaitUntil(next_redraw_at.max(now)));
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        self.handle_window_event(event_loop, event);
    }
}

pub(crate) fn run_windowed(
    world: hoi4_state::World,
    path_cfg: hoi4_paths::PathConfig,
    edge_pan_test: Option<EdgePanTestConfig>,
) {
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = App::new(world, path_cfg, edge_pan_test);
    event_loop.run_app(&mut app).unwrap();
}

pub(crate) fn run_map_phase0(
    world: hoi4_state::World,
    path_cfg: hoi4_paths::PathConfig,
    output_dir: std::path::PathBuf,
    reference_root: Option<std::path::PathBuf>,
) {
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = App::new(world, path_cfg, None);
    app.enable_map_phase0(output_dir, reference_root);
    event_loop.run_app(&mut app).unwrap();
}
