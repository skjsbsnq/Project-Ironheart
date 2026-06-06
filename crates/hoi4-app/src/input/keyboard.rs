use crate::*;
use winit::event::ElementState;
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::KeyCode;
use winit::window::Fullscreen;

impl App {
    pub(crate) fn handle_keyboard_input(
        &mut self,
        event_loop: &ActiveEventLoop,
        code: KeyCode,
        btn_state: ElementState,
    ) {
                if btn_state == ElementState::Pressed {
                    self.keys_held.insert(code);
                    let mut changed = false;
                    match code {
                        KeyCode::Escape => match self.view.game_phase {
                            GamePhase::MainMenu => event_loop.exit(),
                            GamePhase::CountrySelect => {
                                self.view.game_phase = GamePhase::MainMenu;
                                self.reset_menu_state();
                            }
                            GamePhase::Playing => {
                                if self.interaction.frontline_painter.mode != PainterMode::Idle {
                                    self.interaction.frontline_painter.mode = PainterMode::Idle;
                                    self.interaction.frontline_painter.samples.clear();
                                    changed = true;
                                } else if self.ui_state.construction_mode.is_some() {
                                    self.exit_construction_mode();
                                    self.refresh_lut();
                                    changed = true;
                                } else if self.interaction.counter_right_click_province.is_some() {
                                    self.interaction.counter_right_click_province = None;
                                    self.interaction.pending_move_command = false;
                                    changed = true;
                                } else if !self.interaction.expanded_stacks.is_empty() {
                                    self.interaction.expanded_stacks.clear();
                                    changed = true;
                                } else if self.ui_state.active_popup.is_some() {
                                    self.ui_state.active_popup = None;
                                    changed = true;
                                } else if self.ui_state.active_detail_panel.is_some() {
                                    self.ui_state.active_detail_panel = None;
                                    changed = true;
                                } else if self.ui_state.open_panel.is_some() {
                                    self.close_primary_panel();
                                    changed = true;
                                } else {
                                    event_loop.exit();
                                }
                            }
                        },
                        KeyCode::Enter | KeyCode::NumpadEnter => {
                            match self.view.game_phase {
                                GamePhase::MainMenu => {
                                    self.view.game_phase = GamePhase::CountrySelect;
                                    self.reset_menu_state();
                                }
                                GamePhase::CountrySelect => {
                                    let entry =
                                        self.view.available_countries.get(self.view.country_select_idx);
                                    let enabled = entry.map(|e| e.enabled).unwrap_or(false);
                                    if !enabled {
                                        println!(
                                            "[menu] '{}' not yet playable",
                                            entry.map(|e| e.tag.as_str()).unwrap_or("?")
                                        );
                                    } else {
                                        let tag = entry.unwrap().tag.clone();
                                        self.set_player_country_by_tag(&tag);
                                        self.view.game_phase = GamePhase::Playing;
                                        self.world.speed = GameSpeed::Paused;
                                        self.reset_menu_state();
                                        println!(
                                            "[game] Playing as {} (world index {})",
                                            tag, self.view.player_country
                                        );
                                    }
                                }
                                GamePhase::Playing => {}
                            }
                            changed = true;
                        }
                        KeyCode::ArrowUp if self.view.game_phase == GamePhase::CountrySelect => {
                            if self.view.country_select_idx > 0 {
                                self.view.country_select_idx -= 1;
                            }
                            changed = true;
                        }
                        KeyCode::ArrowDown if self.view.game_phase == GamePhase::CountrySelect => {
                            if self.view.country_select_idx + 1 < self.view.available_countries.len() {
                                self.view.country_select_idx += 1;
                            }
                            changed = true;
                        }
                        KeyCode::F1 => {
                            self.render_toggles.debug_overlay = !self.render_toggles.debug_overlay;
                            debug_commands::log_bool_toggle("gui overlay", self.render_toggles.debug_overlay);
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
                                    self.render_toggles.postprocess_debug_view = s.post_process.cycle_debug_view();
                                    debug_commands::log_value(
                                        "post-process debug view",
                                        self.render_toggles.postprocess_debug_view.name(),
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
                            self.render_toggles.terrain_debug_view = self.render_toggles.terrain_debug_view.next();
                            debug_commands::log_value(
                                "terrain debug view",
                                self.render_toggles.terrain_debug_view.name(),
                            );
                            changed = true;
                        }
                        KeyCode::F7 => {
                            self.render_toggles.show_province_names = !self.render_toggles.show_province_names;
                            debug_commands::log_bool_toggle(
                                "province names",
                                self.render_toggles.show_province_names,
                            );
                            changed = true;
                        }
                        KeyCode::F8 => {
                            let shift_held = self.keys_held.contains(&KeyCode::ShiftLeft)
                                || self.keys_held.contains(&KeyCode::ShiftRight);
                            if shift_held {
                                self.render_toggles.map_quality_preset = self.render_toggles.map_quality_preset.next();
                                if let Some(s) = &mut self.state {
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
                                }
                                debug_commands::log_value(
                                    "map quality preset",
                                    self.render_toggles.map_quality_preset.as_str(),
                                );
                            } else if let Some(s) = &mut self.state {
                                let on = s.hoi3_counter_pass.toggle();
                                s.pass_registry.set_enabled("hoi3_counter_v3", on);
                                debug_commands::log_bool_toggle("HOI3 counters", on);
                            }
                            changed = true;
                        }
                        KeyCode::F9 if self.view.game_phase == GamePhase::Playing => {
                            self.toggle_in_game_panel(InGamePanel::Pops);
                            changed = true;
                        }
                        KeyCode::F10 => {
                            self.render_toggles.water_debug_view = self.render_toggles.water_debug_view.next();
                            debug_commands::log_value(
                                "water debug view",
                                self.render_toggles.water_debug_view.name(),
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
                        KeyCode::KeyR => {
                            self.render_toggles.force_water_pass = !self.render_toggles.force_water_pass;
                            if let Some(s) = &mut self.state {
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
                            }
                            debug_commands::log_bool_toggle(
                                "force WaterPass/refraction",
                                self.render_toggles.force_water_pass,
                            );
                            changed = true;
                        }
                        KeyCode::KeyV => {
                            self.render_toggles.border_debug_view = self.render_toggles.border_debug_view.next();
                            debug_commands::log_value(
                                "border debug view",
                                self.render_toggles.border_debug_view.name(),
                            );
                            changed = true;
                        }
                        KeyCode::KeyQ if self.view.game_phase == GamePhase::Playing => {
                            self.toggle_in_game_panel(InGamePanel::Politics);
                            changed = true;
                        }
                        KeyCode::KeyD if self.view.game_phase == GamePhase::Playing => {
                            self.toggle_in_game_panel(InGamePanel::Decisions);
                            changed = true;
                        }
                        KeyCode::KeyT if self.view.game_phase == GamePhase::Playing => {
                            self.toggle_in_game_panel(InGamePanel::ConstructionV6);
                            changed = true;
                        }
                        KeyCode::KeyY if self.view.game_phase == GamePhase::Playing => {
                            self.toggle_in_game_panel(InGamePanel::Research);
                            changed = true;
                        }
                        KeyCode::KeyU if self.view.game_phase == GamePhase::Playing => {
                            self.toggle_in_game_panel(InGamePanel::Diplomacy);
                            changed = true;
                        }
                        KeyCode::KeyI if self.view.game_phase == GamePhase::Playing => {
                            self.toggle_in_game_panel(InGamePanel::Military);
                            changed = true;
                        }
                        KeyCode::KeyO if self.view.game_phase == GamePhase::Playing => {
                            self.toggle_in_game_panel(InGamePanel::Naval);
                            changed = true;
                        }
                        KeyCode::KeyA if self.view.game_phase == GamePhase::Playing => {
                            self.toggle_in_game_panel(InGamePanel::Air);
                            changed = true;
                        }
                        KeyCode::KeyL if self.view.game_phase == GamePhase::Playing => {
                            self.toggle_in_game_panel(InGamePanel::Logistics);
                            changed = true;
                        }
                        KeyCode::KeyJ if self.view.game_phase == GamePhase::Playing => {
                            self.toggle_in_game_panel(InGamePanel::Situation);
                            changed = true;
                        }
                        KeyCode::KeyP if self.view.game_phase == GamePhase::Playing => {
                            self.toggle_in_game_panel(InGamePanel::Laws);
                            changed = true;
                        }
                        KeyCode::KeyK if self.view.game_phase == GamePhase::Playing => {
                            self.toggle_in_game_panel(InGamePanel::Market);
                            changed = true;
                        }
                        KeyCode::KeyB if self.view.game_phase == GamePhase::Playing => {
                            self.toggle_in_game_panel(InGamePanel::ConstructionV6);
                            changed = true;
                        }
                        KeyCode::KeyF if self.view.game_phase == GamePhase::Playing => {
                            self.toggle_in_game_panel(InGamePanel::Finance);
                            changed = true;
                        }
                        KeyCode::KeyG if self.view.game_phase == GamePhase::Playing => {
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
}
