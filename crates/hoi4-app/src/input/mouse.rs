use crate::*;
use winit::dpi::PhysicalPosition;
use winit::event::{ElementState, MouseScrollDelta};
use winit::keyboard::KeyCode;

impl App {
    pub(crate) fn handle_mouse_wheel(&mut self, delta: MouseScrollDelta) {
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

    pub(crate) fn handle_left_mouse_input(&mut self, btn_state: ElementState) {
        if btn_state == ElementState::Pressed {
            self.dragging = true;
            self.drag_pixels = 0.0;
            self.suppress_next_map_click = false;
            self.interaction.selection_box = SelectionBoxState {
                active: false,
                start: self.last_mouse,
                current: self.last_mouse,
                pressed_at: Instant::now(),
            };
            if self.interaction.frontline_painter.mode != PainterMode::Idle {
                self.interaction.frontline_painter.samples.clear();
                self.interaction.frontline_painter.last_sample_at = std::time::Instant::now();
            }
        } else {
            if self.interaction.frontline_painter.mode != PainterMode::Idle
                && !self.interaction.frontline_painter.samples.is_empty()
            {
                let samples = std::mem::take(&mut self.interaction.frontline_painter.samples);
                eprintln!(
                    "[frontline] painter released: {} samples, mode={:?}",
                    samples.len(),
                    self.interaction.frontline_painter.mode
                );
                match self.interaction.frontline_painter.mode {
                    PainterMode::ArmyPainter(id) => {
                        match hoi4_logic::military::frontline::set_frontline_path(
                            &mut self.world,
                            id,
                            &samples,
                        ) {
                            Ok(()) => {
                                eprintln!("[frontline] set_frontline_path OK for army {}", id.raw())
                            }
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
                self.interaction.frontline_painter.mode = PainterMode::Idle;
                self.suppress_next_map_click = true;
                if let Some(s) = &self.state {
                    s.window.request_redraw();
                }
            } else if self.interaction.frontline_painter.mode != PainterMode::Idle {
                self.interaction.frontline_painter.mode = PainterMode::Idle;
                self.suppress_next_map_click = true;
                if let Some(s) = &self.state {
                    s.window.request_redraw();
                }
            } else if self.interaction.selection_box.active
                && self.interaction.selection_box.held_long_enough()
                && self.interaction.selection_box.large_enough()
            {
                let ctrl_held = self.keys_held.contains(&KeyCode::ControlLeft)
                    || self.keys_held.contains(&KeyCode::ControlRight);
                let shift_held = self.keys_held.contains(&KeyCode::ShiftLeft)
                    || self.keys_held.contains(&KeyCode::ShiftRight);
                self.select_counter_stacks_in_rect(
                    self.interaction.selection_box.rect(),
                    ctrl_held || shift_held,
                );
                self.suppress_next_map_click = true;
            } else if self.drag_pixels < MAP_CLICK_DRAG_THRESHOLD_PX
                && !self.suppress_next_map_click
            {
                let [mx, my] = self.last_mouse;
                if self.view.game_phase != GamePhase::Playing {
                    self.handle_menu_mouse_click(mx, my);
                } else if self.ui_blocks_map_clicks() {
                } else if self.primary_panel_blocks_map_click_actions() {
                } else if self.handle_panel_click(mx, my) {
                } else if my < TOPBAR_HEIGHT && mx < 100.0 && self.ui_state.open_panel.is_none() {
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
            self.interaction.selection_box.active = false;
            self.dragging = false;
            self.suppress_next_map_click = false;
        }
    }

    pub(crate) fn handle_right_mouse_input(&mut self, btn_state: ElementState) {
        if btn_state == ElementState::Released && self.view.game_phase == GamePhase::Playing {
            if self.ui_blocks_map_clicks() || self.primary_panel_blocks_map_click_actions() {
                return;
            }

            if self.interaction.frontline_painter.mode != PainterMode::Idle {
                self.interaction.frontline_painter.mode = PainterMode::Idle;
                self.interaction.frontline_painter.samples.clear();
                if let Some(s) = &self.state {
                    s.window.request_redraw();
                }
            } else if self.ui_state.construction_mode.is_some() {
                self.exit_construction_mode();
                self.refresh_lut();
            } else {
                let counter_hit = self.pick_counter_province_at_cursor();
                if let Some(cpid) = counter_hit {
                    if !self.interaction.selected_divisions.is_empty() {
                        let dest = hoi4_state::ProvinceId(cpid as u16);
                        self.issue_manual_move_to_selected_divisions(dest);
                        self.interaction.counter_right_click_province = None;
                        if let Some(s) = &self.state {
                            s.window.request_redraw();
                        }
                    } else {
                        self.interaction.counter_right_click_province = Some(cpid as u32);
                        if let Some(s) = &self.state {
                            s.window.request_redraw();
                        }
                    }
                } else {
                    let right_click_pid = self.pick_province_at_cursor();
                    if right_click_pid != u32::MAX {
                        if !self.interaction.selected_divisions.is_empty() {
                            let dest = hoi4_state::ProvinceId(right_click_pid as u16);
                            self.issue_manual_move_to_selected_divisions(dest);
                            if let Some(s) = &self.state {
                                s.window.request_redraw();
                            }
                        } else {
                            let pid = right_click_pid as usize;
                            if pid < self.world.provinces.count {
                                let owner = self.world.provinces.owners[pid];
                                let player = hoi4_state::CountryId(self.view.player_country as u16);
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
                                            wgs.iter().any(|w| w.target == owner && w.justified)
                                        })
                                        .unwrap_or(false)
                                } else {
                                    false
                                };

                                if !is_own && !owner.is_none() && !tag.is_empty() {
                                    if let Some(data) = self.build_country_info_data(owner, has_wg)
                                    {
                                        self.close_primary_panel();
                                        self.ui_state.province_info_card.open = false;
                                        self.ui_state.country_info_panel.open_with(data);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    pub(crate) fn handle_cursor_moved(&mut self, position: PhysicalPosition<f64>) {
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
            let is_painting = self.interaction.frontline_painter.mode != PainterMode::Idle;
            if is_painting {
                self.last_mouse = [x, y];
                self.suppress_next_map_click = true;
                let pid = self.pick_province_at_cursor();
                if pid != u32::MAX {
                    let prov = hoi4_state::ProvinceId(pid as u16);
                    if self.append_frontline_painter_sample(prov) {
                        self.interaction.frontline_painter.last_sample_at =
                            std::time::Instant::now();
                        if let Some(s) = &self.state {
                            s.window.request_redraw();
                        }
                    }
                }
            } else if self.view.game_phase == GamePhase::Playing
                && !self.ui_blocks_map_clicks()
                && !self.primary_panel_blocks_map_click_actions()
                && self.ui_state.construction_mode.is_none()
                && self.interaction.pending_move_command == false
            {
                self.interaction.selection_box.current = [x, y];
                if self.interaction.selection_box.held_long_enough()
                    && self.interaction.selection_box.large_enough()
                {
                    self.interaction.selection_box.active = true;
                    self.suppress_next_map_click = true;
                    if let Some(s) = &self.state {
                        s.window.request_redraw();
                    }
                }
            }
        }
        self.last_mouse = [x, y];

        if self.view.game_phase == GamePhase::Playing {
            if self.ui_blocks_map_clicks() {
                if self.hovered_province_id != u32::MAX {
                    self.hovered_province_id = u32::MAX;
                }
            } else {
                let now = std::time::Instant::now();
                if now.duration_since(self.last_hover_pick_at).as_millis() >= HOVER_PICK_INTERVAL_MS
                {
                    self.last_hover_pick_at = now;
                    let new_hover = self.pick_province_at_cursor();
                    if new_hover != self.hovered_province_id {
                        self.hovered_province_id = new_hover;
                    }
                }
            }
        } else {
            self.update_menu_hover_from_cursor(x, y);
        }
    }
}
