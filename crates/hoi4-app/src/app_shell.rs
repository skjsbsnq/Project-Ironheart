use std::sync::Arc;
use std::time::{Duration, Instant};

use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Fullscreen, Window, WindowId};

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
        self.update_title();
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.map_phase0_finished() {
            event_loop.exit();
            return;
        }
        self.update();
        if let Some(s) = &self.state {
            s.window.request_redraw();
        }
        let simulation_running =
            self.game_phase == GamePhase::Playing && self.world.speed != GameSpeed::Paused;
        if simulation_running || self.map_phase0.is_some() {
            event_loop.set_control_flow(ControlFlow::Poll);
        } else {
            event_loop.set_control_flow(ControlFlow::WaitUntil(
                Instant::now() + Duration::from_millis(16),
            ));
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        // Give egui first chance to consume input events.
        let consumed = if let Some(s) = self.state.as_mut() {
            let response = s.ui.on_window_event(&s.window, &event);
            if response.repaint {
                s.window.request_redraw();
            }
            response.consumed
        } else {
            false
        };
        let forward_map_click_through_ui = consumed
            && self.game_phase == GamePhase::Playing
            && matches!(self.open_panel, Some(InGamePanel::Air | InGamePanel::Naval))
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
        if consumed && !forward_map_click_through_ui {
            if matches!(
                event,
                WindowEvent::MouseInput {
                    state: ElementState::Released,
                    button: winit::event::MouseButton::Left,
                    ..
                }
            ) {
                self.selection_box.active = false;
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
                if btn_state == ElementState::Pressed {
                    self.keys_held.insert(code);
                    let mut changed = false;
                    match code {
                        KeyCode::Escape => match self.game_phase {
                            GamePhase::MainMenu => event_loop.exit(),
                            GamePhase::CountrySelect => {
                                self.game_phase = GamePhase::MainMenu;
                                self.reset_menu_state();
                            }
                            GamePhase::Playing => {
                                if self.frontline_painter.mode != PainterMode::Idle {
                                    self.frontline_painter.mode = PainterMode::Idle;
                                    self.frontline_painter.samples.clear();
                                    changed = true;
                                } else if self.construction_mode.is_some() {
                                    self.exit_construction_mode();
                                    self.refresh_lut();
                                    changed = true;
                                } else if self.counter_right_click_province.is_some() {
                                    self.counter_right_click_province = None;
                                    self.pending_move_command = false;
                                    changed = true;
                                } else if !self.expanded_stacks.is_empty() {
                                    self.expanded_stacks.clear();
                                    changed = true;
                                } else if self.open_panel.is_some() {
                                    self.open_panel = None;
                                    changed = true;
                                } else {
                                    event_loop.exit();
                                }
                            }
                        },
                        KeyCode::Enter | KeyCode::NumpadEnter => {
                            match self.game_phase {
                                GamePhase::MainMenu => {
                                    self.game_phase = GamePhase::CountrySelect;
                                    self.reset_menu_state();
                                }
                                GamePhase::CountrySelect => {
                                    let entry =
                                        self.available_countries.get(self.country_select_idx);
                                    let enabled = entry.map(|e| e.enabled).unwrap_or(false);
                                    if !enabled {
                                        println!(
                                            "[menu] '{}' not yet playable",
                                            entry.map(|e| e.tag.as_str()).unwrap_or("?")
                                        );
                                    } else {
                                        let tag = entry.unwrap().tag.clone();
                                        self.set_player_country_by_tag(&tag);
                                        self.game_phase = GamePhase::Playing;
                                        self.world.speed = GameSpeed::Paused;
                                        self.reset_menu_state();
                                        println!(
                                            "[game] Playing as {} (world index {})",
                                            tag, self.player_country
                                        );
                                    }
                                }
                                GamePhase::Playing => {}
                            }
                            changed = true;
                        }
                        KeyCode::ArrowUp if self.game_phase == GamePhase::CountrySelect => {
                            if self.country_select_idx > 0 {
                                self.country_select_idx -= 1;
                            }
                            changed = true;
                        }
                        KeyCode::ArrowDown if self.game_phase == GamePhase::CountrySelect => {
                            if self.country_select_idx + 1 < self.available_countries.len() {
                                self.country_select_idx += 1;
                            }
                            changed = true;
                        }
                        KeyCode::F1 => {
                            self.debug_overlay = !self.debug_overlay;
                            debug_commands::log_bool_toggle("gui overlay", self.debug_overlay);
                            changed = true;
                        }
                        KeyCode::F2 => {
                            self.demo_visible = !self.demo_visible;
                            self.b5_demo_visible = self.demo_visible;
                            println!(
                                "[ui] demo window {}",
                                if self.demo_visible { "ON" } else { "OFF" }
                            );
                            changed = true;
                        }
                        KeyCode::F3 => {
                            if let Some(s) = &mut self.state {
                                s.shadow_pass.toggle_debug();
                                debug_commands::log_bool_toggle(
                                    "shadow map overlay",
                                    s.shadow_pass.debug_visible,
                                );
                            }
                            changed = true;
                        }
                        KeyCode::F4 => {
                            if let Some(s) = &mut self.state {
                                s.debug_render_overlay.toggle();
                                debug_commands::log_bool_toggle(
                                    "render overlay",
                                    s.debug_render_overlay.enabled,
                                );
                            }
                            changed = true;
                        }
                        KeyCode::F5 => {
                            if let Some(s) = &mut self.state {
                                let shift_held = self.keys_held.contains(&KeyCode::ShiftLeft)
                                    || self.keys_held.contains(&KeyCode::ShiftRight);
                                if shift_held {
                                    self.postprocess_debug_view = s.post_process.cycle_debug_view();
                                    debug_commands::log_value(
                                        "post-process debug view",
                                        self.postprocess_debug_view.name(),
                                    );
                                } else {
                                    s.post_process.mode = match s.post_process.mode {
                                        PostProcessMode::Full => PostProcessMode::Off,
                                        PostProcessMode::Off => PostProcessMode::Full,
                                    };
                                    debug_commands::log_value(
                                        "post-process mode",
                                        format!("{:?}", s.post_process.mode),
                                    );
                                }
                            }
                            changed = true;
                        }
                        KeyCode::F6 => {
                            self.terrain_debug_view = self.terrain_debug_view.next();
                            debug_commands::log_value(
                                "terrain debug view",
                                self.terrain_debug_view.name(),
                            );
                            changed = true;
                        }
                        KeyCode::F7 => {
                            self.show_province_names = !self.show_province_names;
                            debug_commands::log_bool_toggle(
                                "province names",
                                self.show_province_names,
                            );
                            changed = true;
                        }
                        KeyCode::F8 => {
                            let shift_held = self.keys_held.contains(&KeyCode::ShiftLeft)
                                || self.keys_held.contains(&KeyCode::ShiftRight);
                            if shift_held {
                                self.map_quality_preset = self.map_quality_preset.next();
                                debug_commands::log_value(
                                    "map quality preset",
                                    self.map_quality_preset.as_str(),
                                );
                            } else if let Some(s) = &mut self.state {
                                let on = s.hoi3_counter_pass.toggle();
                                s.pass_registry.set_enabled("hoi3_counter_v3", on);
                                debug_commands::log_bool_toggle("HOI3 counters", on);
                            }
                            changed = true;
                        }
                        KeyCode::F9 if self.game_phase == GamePhase::Playing => {
                            self.toggle_in_game_panel(InGamePanel::Pops);
                            changed = true;
                        }
                        KeyCode::F10 => {
                            self.water_debug_view = self.water_debug_view.next();
                            debug_commands::log_value(
                                "water debug view",
                                self.water_debug_view.name(),
                            );
                            changed = true;
                        }
                        KeyCode::F11 => {
                            if let Some(s) = &self.state {
                                let next = match s.window.fullscreen() {
                                    Some(_) => None,
                                    None => Some(Fullscreen::Borderless(None)),
                                };
                                s.window.set_fullscreen(next.clone());
                                debug_commands::log_value(
                                    "fullscreen",
                                    if next.is_some() {
                                        "ON (borderless)"
                                    } else {
                                        "OFF"
                                    },
                                );
                            }
                            changed = true;
                        }
                        KeyCode::F12 => {
                            self.v9_demo.open = !self.v9_demo.open;
                            debug_commands::log_bool_toggle("v9 demo", self.v9_demo.open);
                            changed = true;
                        }
                        KeyCode::Space => {
                            self.world.speed = if self.world.speed == GameSpeed::Paused {
                                GameSpeed::Speed3
                            } else {
                                GameSpeed::Paused
                            };
                            changed = true;
                        }
                        KeyCode::Digit1 => {
                            self.world.speed = GameSpeed::Speed1;
                            changed = true;
                        }
                        KeyCode::Digit2 => {
                            self.world.speed = GameSpeed::Speed2;
                            changed = true;
                        }
                        KeyCode::Digit3 => {
                            self.world.speed = GameSpeed::Speed3;
                            changed = true;
                        }
                        KeyCode::Digit4 => {
                            self.world.speed = GameSpeed::Speed4;
                            changed = true;
                        }
                        KeyCode::Digit5 => {
                            self.world.speed = GameSpeed::Speed5;
                            changed = true;
                        }
                        KeyCode::NumpadAdd | KeyCode::Equal => {
                            self.world.speed = match self.world.speed {
                                GameSpeed::Paused => GameSpeed::Speed1,
                                GameSpeed::Speed1 => GameSpeed::Speed2,
                                GameSpeed::Speed2 => GameSpeed::Speed3,
                                GameSpeed::Speed3 => GameSpeed::Speed4,
                                GameSpeed::Speed4 => GameSpeed::Speed5,
                                GameSpeed::Speed5 => GameSpeed::Speed5,
                            };
                            changed = true;
                        }
                        KeyCode::NumpadSubtract | KeyCode::Minus => {
                            self.world.speed = match self.world.speed {
                                GameSpeed::Paused => GameSpeed::Paused,
                                GameSpeed::Speed1 => GameSpeed::Paused,
                                GameSpeed::Speed2 => GameSpeed::Speed1,
                                GameSpeed::Speed3 => GameSpeed::Speed2,
                                GameSpeed::Speed4 => GameSpeed::Speed3,
                                GameSpeed::Speed5 => GameSpeed::Speed4,
                            };
                            changed = true;
                        }
                        KeyCode::KeyM => {
                            self.map_mode = self.map_mode.next();
                            self.refresh_lut();
                            changed = true;
                        }
                        KeyCode::KeyV => {
                            self.border_debug_view = self.border_debug_view.next();
                            debug_commands::log_value(
                                "border debug view",
                                self.border_debug_view.name(),
                            );
                            changed = true;
                        }
                        KeyCode::KeyQ if self.game_phase == GamePhase::Playing => {
                            self.toggle_in_game_panel(InGamePanel::Politics);
                            changed = true;
                        }
                        KeyCode::KeyD if self.game_phase == GamePhase::Playing => {
                            self.toggle_in_game_panel(InGamePanel::Decisions);
                            changed = true;
                        }
                        KeyCode::KeyT if self.game_phase == GamePhase::Playing => {
                            self.toggle_in_game_panel(InGamePanel::ConstructionV6);
                            changed = true;
                        }
                        KeyCode::KeyY if self.game_phase == GamePhase::Playing => {
                            self.toggle_in_game_panel(InGamePanel::Research);
                            changed = true;
                        }
                        KeyCode::KeyU if self.game_phase == GamePhase::Playing => {
                            self.toggle_in_game_panel(InGamePanel::Diplomacy);
                            changed = true;
                        }
                        KeyCode::KeyI if self.game_phase == GamePhase::Playing => {
                            self.toggle_in_game_panel(InGamePanel::Military);
                            changed = true;
                        }
                        KeyCode::KeyO if self.game_phase == GamePhase::Playing => {
                            self.toggle_in_game_panel(InGamePanel::Naval);
                            changed = true;
                        }
                        KeyCode::KeyA if self.game_phase == GamePhase::Playing => {
                            self.toggle_in_game_panel(InGamePanel::Air);
                            changed = true;
                        }
                        KeyCode::KeyL if self.game_phase == GamePhase::Playing => {
                            self.toggle_in_game_panel(InGamePanel::Logistics);
                            changed = true;
                        }
                        KeyCode::KeyJ if self.game_phase == GamePhase::Playing => {
                            self.toggle_in_game_panel(InGamePanel::Situation);
                            changed = true;
                        }
                        KeyCode::KeyP if self.game_phase == GamePhase::Playing => {
                            self.toggle_in_game_panel(InGamePanel::Laws);
                            changed = true;
                        }
                        KeyCode::KeyK if self.game_phase == GamePhase::Playing => {
                            self.toggle_in_game_panel(InGamePanel::Market);
                            changed = true;
                        }
                        KeyCode::KeyB if self.game_phase == GamePhase::Playing => {
                            self.toggle_in_game_panel(InGamePanel::ConstructionV6);
                            changed = true;
                        }
                        KeyCode::KeyF if self.game_phase == GamePhase::Playing => {
                            self.toggle_in_game_panel(InGamePanel::Finance);
                            changed = true;
                        }
                        KeyCode::KeyG if self.game_phase == GamePhase::Playing => {
                            self.toggle_in_game_panel(InGamePanel::Trade);
                            changed = true;
                        }
                        _ => {}
                    }
                    if changed {
                        self.update_title();
                    }
                } else {
                    self.keys_held.remove(&code);
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                if self
                    .state
                    .as_ref()
                    .map(|s| s.ui.ctx.is_pointer_over_area())
                    .unwrap_or(false)
                {
                    return;
                }
                let scroll = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 / 100.0,
                };
                if scroll.abs() > f32::EPSILON {
                    let zoom_factor = 0.9_f32.powf(scroll.clamp(-6.0, 6.0));
                    self.zoom_at_cursor(zoom_factor);
                }
            }
            WindowEvent::MouseInput {
                state: btn_state,
                button: winit::event::MouseButton::Left,
                ..
            } => {
                if btn_state == ElementState::Pressed {
                    self.dragging = true;
                    self.drag_pixels = 0.0;
                    self.suppress_next_map_click = false;
                    self.selection_box = SelectionBoxState {
                        active: false,
                        start: self.last_mouse,
                        current: self.last_mouse,
                        pressed_at: Instant::now(),
                    };
                    if self.frontline_painter.mode != PainterMode::Idle {
                        self.frontline_painter.samples.clear();
                        self.frontline_painter.last_sample_at = std::time::Instant::now();
                    }
                } else {
                    if self.frontline_painter.mode != PainterMode::Idle
                        && !self.frontline_painter.samples.is_empty()
                    {
                        let samples = std::mem::take(&mut self.frontline_painter.samples);
                        eprintln!(
                            "[frontline] painter released: {} samples, mode={:?}",
                            samples.len(),
                            self.frontline_painter.mode
                        );
                        match self.frontline_painter.mode {
                            PainterMode::ArmyPainter(id) => {
                                match hoi4_logic::military::frontline::set_frontline_path(
                                    &mut self.world,
                                    id,
                                    &samples,
                                ) {
                                    Ok(()) => eprintln!(
                                        "[frontline] set_frontline_path OK for army {}",
                                        id.raw()
                                    ),
                                    Err(e) => eprintln!(
                                        "[frontline] set_frontline_path FAILED for army {}: {}",
                                        id.raw(),
                                        e
                                    ),
                                }
                            }
                            PainterMode::ArrowPainter(id, anchor) => {
                                match hoi4_logic::military::frontline::set_arrow(
                                    &mut self.world,
                                    id,
                                    anchor,
                                    &samples,
                                ) {
                                    Ok(()) => {
                                        eprintln!("[frontline] set_arrow OK for army {}", id.raw())
                                    }
                                    Err(e) => eprintln!(
                                        "[frontline] set_arrow FAILED for army {}: {}",
                                        id.raw(),
                                        e
                                    ),
                                }
                            }
                            PainterMode::Idle => {}
                        }
                        self.frontline_painter.mode = PainterMode::Idle;
                        self.suppress_next_map_click = true;
                        if let Some(s) = &self.state {
                            s.window.request_redraw();
                        }
                    } else if self.frontline_painter.mode != PainterMode::Idle {
                        self.frontline_painter.mode = PainterMode::Idle;
                        self.suppress_next_map_click = true;
                        if let Some(s) = &self.state {
                            s.window.request_redraw();
                        }
                    } else if self.selection_box.active
                        && self.selection_box.held_long_enough()
                        && self.selection_box.large_enough()
                    {
                        let ctrl_held = self.keys_held.contains(&KeyCode::ControlLeft)
                            || self.keys_held.contains(&KeyCode::ControlRight);
                        let shift_held = self.keys_held.contains(&KeyCode::ShiftLeft)
                            || self.keys_held.contains(&KeyCode::ShiftRight);
                        self.select_counter_stacks_in_rect(
                            self.selection_box.rect(),
                            ctrl_held || shift_held,
                        );
                        self.suppress_next_map_click = true;
                    } else if self.drag_pixels < MAP_CLICK_DRAG_THRESHOLD_PX
                        && !self.suppress_next_map_click
                    {
                        let [mx, my] = self.last_mouse;
                        if self.game_phase != GamePhase::Playing {
                            self.handle_menu_mouse_click(mx, my);
                        } else if self.ui_blocks_map_clicks() {
                        } else if self.handle_panel_click(mx, my) {
                        } else if my < TOPBAR_HEIGHT && mx < 100.0 && self.open_panel.is_none() {
                            self.toggle_in_game_panel(InGamePanel::Politics);
                            self.update_title();
                            if let Some(s) = &self.state {
                                s.window.request_redraw();
                            }
                        } else {
                            let ctrl_held = self.keys_held.contains(&KeyCode::ControlLeft)
                                || self.keys_held.contains(&KeyCode::ControlRight);
                            let shift_held = self.keys_held.contains(&KeyCode::ShiftLeft)
                                || self.keys_held.contains(&KeyCode::ShiftRight);
                            if let Some(pid) = self.pick_counter_province_at_cursor() {
                                self.select_counter_stack_at_province(pid, ctrl_held, shift_held);
                            } else {
                                self.try_pick_province();
                            }
                        }
                    }
                    self.selection_box.active = false;
                    self.dragging = false;
                    self.suppress_next_map_click = false;
                }
            }
            WindowEvent::MouseInput {
                state: btn_state,
                button: winit::event::MouseButton::Right,
                ..
            } => {
                if btn_state == ElementState::Released && self.game_phase == GamePhase::Playing {
                    if self.frontline_painter.mode != PainterMode::Idle {
                        self.frontline_painter.mode = PainterMode::Idle;
                        self.frontline_painter.samples.clear();
                        if let Some(s) = &self.state {
                            s.window.request_redraw();
                        }
                    } else if self.construction_mode.is_some() {
                        self.exit_construction_mode();
                        self.refresh_lut();
                    } else {
                        let [mx, my] = self.last_mouse;
                        let dpi = self
                            .state
                            .as_ref()
                            .map(|s| s.window.scale_factor() as f32)
                            .unwrap_or(1.0);
                        let counter_hit =
                            hit_test(&self._cached_hoi3_hit_regions, mx * dpi, my * dpi);
                        if let Some(cpid) = counter_hit {
                            if !self.selected_divisions.is_empty() {
                                let dest = hoi4_state::ProvinceId(cpid as u16);
                                self.issue_manual_move_to_selected_divisions(dest);
                                self.counter_right_click_province = None;
                                if let Some(s) = &self.state {
                                    s.window.request_redraw();
                                }
                            } else {
                                self.counter_right_click_province = Some(cpid as u32);
                                if let Some(s) = &self.state {
                                    s.window.request_redraw();
                                }
                            }
                        } else {
                            let right_click_pid = self.pick_province_at_cursor();
                            if right_click_pid != u32::MAX {
                                if !self.selected_divisions.is_empty() {
                                    let dest = hoi4_state::ProvinceId(right_click_pid as u16);
                                    self.issue_manual_move_to_selected_divisions(dest);
                                    if let Some(s) = &self.state {
                                        s.window.request_redraw();
                                    }
                                } else {
                                    let pid = right_click_pid as usize;
                                    if pid < self.world.provinces.count {
                                        let owner = self.world.provinces.owners[pid];
                                        let player =
                                            hoi4_state::CountryId(self.player_country as u16);
                                        let tag = if !owner.is_none()
                                            && (owner.0 as usize) < self.world.countries.count
                                        {
                                            self.world.countries.tags[owner.0 as usize].clone()
                                        } else {
                                            String::new()
                                        };
                                        let is_own = owner == player;
                                        let has_wg = if !is_own && !owner.is_none() {
                                            self.world
                                                .diplomacy
                                                .pending_wargoals
                                                .get(&player)
                                                .map(|wgs| {
                                                    wgs.iter()
                                                        .any(|w| w.target == owner && w.justified)
                                                })
                                                .unwrap_or(false)
                                        } else {
                                            false
                                        };

                                        if !is_own && !owner.is_none() && !tag.is_empty() {
                                            if let Some(data) =
                                                self.build_country_info_data(owner, has_wg)
                                            {
                                                self.open_panel = None;
                                                self.province_info_card.open = false;
                                                self.country_info_panel.open_with(data);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                let dpi = self
                    .state
                    .as_ref()
                    .map(|s| s.window.scale_factor() as f32)
                    .unwrap_or(1.0);
                let x = position.x as f32 / dpi;
                let y = position.y as f32 / dpi;
                if self.dragging && self.state.is_some() {
                    let dx = x - self.last_mouse[0];
                    let dy = y - self.last_mouse[1];
                    self.drag_pixels += dx.abs() + dy.abs();
                    let is_painting = self.frontline_painter.mode != PainterMode::Idle;
                    if is_painting {
                        self.last_mouse = [x, y];
                        self.suppress_next_map_click = true;
                        let now = std::time::Instant::now();
                        let elapsed = now.duration_since(self.frontline_painter.last_sample_at);
                        if elapsed.as_millis() >= 33 {
                            let pid = self.pick_province_at_cursor();
                            if pid != u32::MAX {
                                let prov = hoi4_state::ProvinceId(pid as u16);
                                let last_differs = self
                                    .frontline_painter
                                    .samples
                                    .last()
                                    .map_or(true, |&last| last != prov);
                                if last_differs {
                                    if self.frontline_painter.samples.len()
                                        < hoi4_logic::military::frontline::MAX_SAMPLES
                                    {
                                        self.frontline_painter.samples.push(prov);
                                    } else {
                                        let n = self.frontline_painter.samples.len();
                                        let step = n / (n - 1).max(1);
                                        let mut deduped: Vec<hoi4_state::ProvinceId> =
                                            Vec::with_capacity(n);
                                        let mut i = 0;
                                        while i < n {
                                            deduped.push(self.frontline_painter.samples[i]);
                                            i += if i + step < n { step } else { 1 };
                                        }
                                        deduped.push(prov);
                                        self.frontline_painter.samples = deduped;
                                    }
                                    self.frontline_painter.last_sample_at = now;
                                    if let Some(s) = &self.state {
                                        s.window.request_redraw();
                                    }
                                }
                            }
                        }
                    } else if self.game_phase == GamePhase::Playing
                        && !self.ui_blocks_map_clicks()
                        && self.construction_mode.is_none()
                        && self.pending_move_command == false
                    {
                        self.selection_box.current = [x, y];
                        if self.selection_box.held_long_enough()
                            && self.selection_box.large_enough()
                        {
                            self.selection_box.active = true;
                            self.suppress_next_map_click = true;
                            if let Some(s) = &self.state {
                                s.window.request_redraw();
                            }
                        }
                    }
                }
                self.last_mouse = [x, y];

                if self.game_phase == GamePhase::Playing {
                    if self.ui_blocks_map_clicks() {
                        if self.hovered_province_id != u32::MAX {
                            self.hovered_province_id = u32::MAX;
                            if let Some(s) = &self.state {
                                s.window.request_redraw();
                            }
                        }
                    } else {
                        let now = std::time::Instant::now();
                        if now.duration_since(self.last_hover_pick_at).as_millis()
                            >= HOVER_PICK_INTERVAL_MS
                        {
                            self.last_hover_pick_at = now;
                            let new_hover = self.pick_province_at_cursor();
                            if new_hover != self.hovered_province_id {
                                self.hovered_province_id = new_hover;
                                if let Some(s) = &self.state {
                                    s.window.request_redraw();
                                }
                            }
                        }
                    }
                } else {
                    let mut new_btn: Option<&'static str> = None;
                    let mut new_row: Option<usize> = None;
                    match self.game_phase {
                        GamePhase::MainMenu => {
                            for btn in &self.last_main_buttons {
                                if btn.enabled && btn.contains(x, y) {
                                    new_btn = Some(btn.id);
                                    break;
                                }
                            }
                        }
                        GamePhase::CountrySelect => {
                            if let Some(layout) = &self.last_country_layout {
                                if layout.start_button.contains(x, y) {
                                    new_btn = Some(layout.start_button.id);
                                } else if layout.back_button.contains(x, y) {
                                    new_btn = Some(layout.back_button.id);
                                }
                                for (rect, idx) in &layout.list_rows {
                                    let (rx, ry, rw, rh) = *rect;
                                    if x >= rx && x < rx + rw && y >= ry && y < ry + rh {
                                        new_row = Some(*idx);
                                        break;
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                    if new_btn != self.menu_hovered_btn || new_row != self.menu_hovered_row {
                        self.menu_hovered_btn = new_btn;
                        self.menu_hovered_row = new_row;
                        if let Some(s) = &self.state {
                            s.window.request_redraw();
                        }
                    }
                }
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

pub(crate) fn run_windowed(world: hoi4_state::World, path_cfg: hoi4_paths::PathConfig) {
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = App::new(world, path_cfg);
    event_loop.run_app(&mut app).unwrap();
}

pub(crate) fn run_map_phase0(
    world: hoi4_state::World,
    path_cfg: hoi4_paths::PathConfig,
    output_dir: std::path::PathBuf,
) {
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = App::new(world, path_cfg);
    app.enable_map_phase0(output_dir);
    event_loop.run_app(&mut app).unwrap();
}
