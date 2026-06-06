use crate::*;
use winit::event::{ElementState, KeyEvent, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::PhysicalKey;

impl App {
    pub(crate) fn handle_window_event(&mut self, event_loop: &ActiveEventLoop, event: WindowEvent) {
        // Give egui first chance to consume input events.
        let consumed = if let Some(s) = self.state.as_mut() {
            let response = s.ui.on_window_event(&s.window, &event);
            response.consumed
        } else {
            false
        };
        let global_debug_key = super::debug_hotkeys::is_global_debug_key(&event);
        let forward_map_click_through_ui = consumed
            && self.view.game_phase == GamePhase::Playing
            && matches!(self.ui_state.open_panel, Some(InGamePanel::Air | InGamePanel::Naval))
            && matches!(
                &event,
                WindowEvent::MouseInput {
                    button: winit::event::MouseButton::Left,
                    ..
                }
            )
            && self
                .state
                .as_ref()
                .map(|s| !s.ui.ctx.is_pointer_over_area())
                .unwrap_or(false);
        if consumed && !global_debug_key && !forward_map_click_through_ui {
            if matches!(
                event,
                WindowEvent::MouseInput {
                    state: ElementState::Released,
                    button: winit::event::MouseButton::Left,
                    ..
                }
            ) {
                self.interaction.selection_box.active = false;
                self.dragging = false;
                self.suppress_next_map_click = false;
            }
            return;
        }

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(s) = &mut self.state {
                    s.config.width = size.width.max(1);
                    s.config.height = size.height.max(1);
                    s.surface.configure(&s.device, &s.config);
                    s.depth_view =
                        make_depth_view(&s.device, s.config.width, s.config.height, s.depth_format);
                    s.hdr_target = HdrTarget::new(&s.device, s.config.width, s.config.height);
                    let water_target_quality = if self.render_toggles.force_water_pass {
                        MapQualityPreset::High
                    } else {
                        self.render_toggles.map_quality_preset
                    };
                    s.water_refraction_target = WaterRefractionTarget::for_quality(
                        &s.device,
                        s.hdr_target.width,
                        s.hdr_target.height,
                        water_target_quality,
                    );
                    s.water_refraction_pass.rebuild_bind_group(
                        &s.device,
                        &s.hdr_target.view,
                        s.hdr_target.width,
                        s.hdr_target.height,
                        s.water_refraction_target.width,
                        s.water_refraction_target.height,
                    );
                    s.water_pass.rebuild_refraction_binding(
                        &s.device,
                        &s.water_refraction_target.view,
                        &s.water_refraction_target.sampler,
                    );
                    s.simple_blit
                        .rebuild_bind_group(&s.device, &s.hdr_target.view);
                    s.post_process.rebuild_targets(
                        &s.device,
                        &s.hdr_target.view,
                        s.hdr_target.width,
                        s.hdr_target.height,
                    );
                    let dpi = s.window.scale_factor() as f32;
                    let logical_w = s.config.width as f32 / dpi;
                    let logical_h = s.config.height as f32 / dpi;
                    println!(
                        "[resize] physical={}x{} logical={:.0}x{:.0} dpi={:.2}",
                        s.config.width, s.config.height, logical_w, logical_h, dpi
                    );
                    self.camera.aspect = logical_w / logical_h.max(1.0);
                    self.camera.clamp_target_to_map();
                    let cam =
                        CameraUniform::from_camera(&self.camera, HEIGHT_SCALE, LAT_CORRECTION);
                    s.queue
                        .write_buffer(&s.camera_buffer, 0, bytemuck::bytes_of(&cam));
                    s.text_pass.set_screen_size(&s.queue, logical_w, logical_h);
                }
                self.reset_menu_state();
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(code),
                        state: btn_state,
                        repeat: false,
                        ..
                    },
                ..
            } => {
                self.handle_keyboard_input(event_loop, code, btn_state);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                self.handle_mouse_wheel(delta);
            }
            WindowEvent::MouseInput {
                state: btn_state,
                button: winit::event::MouseButton::Left,
                ..
            } => {
                self.handle_left_mouse_input(btn_state);
            }
            WindowEvent::MouseInput {
                state: btn_state,
                button: winit::event::MouseButton::Right,
                ..
            } => {
                self.handle_right_mouse_input(btn_state);
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.handle_cursor_moved(position);
            }
            WindowEvent::RedrawRequested => {
                self.render();
                if self.map_phase0_finished() {
                    event_loop.exit();
                }
            }
            _ => {}
        }
    
    }
}
