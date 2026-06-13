use std::time::Instant;

use winit::keyboard::KeyCode;

use crate::{
    advance_visual_day_night_hour, runtime, App, GamePhase, EDGE_PAN_MARGIN_PX,
    EDGE_PAN_SPEED_SCALE, INTERACTIVE_FAST_SIM_BUDGET_SECS, MAX_INTERACTION_DT_SECS,
    MIN_SIM_SLICE_SECS,
};

impl App {
    pub(crate) fn update_simulation_tick(&mut self, max_sim_budget_secs: f32) {
        let now = Instant::now();
        let dt = (now - self.last_frame).as_secs_f32();
        let interaction_dt = dt.min(MAX_INTERACTION_DT_SECS);
        self.last_frame = now;

        if self.audit.map_phase0.is_some() {
            return;
        }

        // Only run simulation and camera controls in Playing phase.
        if self.view.game_phase != GamePhase::Playing {
            return;
        }

        self.update_edge_pan_test_cursor(now);
        self.update_smooth_zoom(interaction_dt);

        // Arrow-key and HOI4-style screen-edge panning. WASD is reserved for panels/hotkeys.
        let pan_speed = self.camera.distance * 0.6 * interaction_dt;
        let mut pan_x = 0.0;
        let mut pan_z = 0.0;
        if self.keys_held.contains(&KeyCode::ArrowUp) {
            pan_z -= pan_speed;
        }
        if self.keys_held.contains(&KeyCode::ArrowDown) {
            pan_z += pan_speed;
        }
        if self.keys_held.contains(&KeyCode::ArrowLeft) {
            pan_x -= pan_speed;
        }
        if self.keys_held.contains(&KeyCode::ArrowRight) {
            pan_x += pan_speed;
        }
        if let Some(s) = self.state.as_ref() {
            let dpi = s.ui_scale_factor();
            let w = s.config.width as f32 / dpi.max(0.0001);
            let h = s.config.height as f32 / dpi.max(0.0001);
            let [mx, my] = self.last_mouse;
            let edge_pan_speed = self.camera.distance * EDGE_PAN_SPEED_SCALE * interaction_dt;
            if mx <= EDGE_PAN_MARGIN_PX {
                pan_x -= edge_pan_speed;
            } else if mx >= w - EDGE_PAN_MARGIN_PX {
                pan_x += edge_pan_speed;
            }
            if my <= EDGE_PAN_MARGIN_PX {
                pan_z -= edge_pan_speed;
            } else if my >= h - EDGE_PAN_MARGIN_PX {
                pan_z += edge_pan_speed;
            }
        }
        let viewport_interacting = pan_x != 0.0
            || pan_z != 0.0
            || self.dragging
            || self.smooth_zoom_target_distance.is_some();
        if viewport_interacting {
            self.render_toggles.last_viewport_interaction_at = Some(now);
        }
        if pan_x != 0.0 || pan_z != 0.0 {
            self.camera.pan(pan_x, pan_z);
            self.upload_camera();
        }

        if self.world.speed == hoi4_state::GameSpeed::Paused {
            self.time_accumulator = 0.0;
            self.update_division_motion(interaction_dt, false);
            self.print_perf_diag(now);
            // Periodic title refresh.
            if (now - self.last_status_print).as_secs_f32() >= 1.0 {
                self.update_title();
                self.last_status_print = now;
            }
            return;
        }
        self.visual_day_night_hour = advance_visual_day_night_hour(
            self.visual_day_night_hour,
            self.world.speed,
            interaction_dt,
        );

        let secs_per_hour = self.world.speed.seconds_per_hour();
        let mut simulation_advanced = false;
        let (base_max_ticks, speed_tick_budget) =
            runtime::systems_runtime::speed_tick_limits(self.world.speed);
        let interaction_budget = if viewport_interacting
            && matches!(
                self.world.speed,
                hoi4_state::GameSpeed::Speed4 | hoi4_state::GameSpeed::Speed5
            ) {
            INTERACTIVE_FAST_SIM_BUDGET_SECS
        } else {
            speed_tick_budget
        };
        let tick_budget = interaction_budget.min(max_sim_budget_secs.max(0.0));
        let tick_started_at = Instant::now();
        let mut ticks = 0;
        let mut stop_advancing_hours = false;

        // Accumulate real time before any pending daily work runs. Pending
        // work may consume this frame, but it should not make game time vanish.
        let sim_dt = dt.min(0.10);
        if secs_per_hour.is_finite() && secs_per_hour > 0.0 {
            self.time_accumulator += sim_dt;
        }

        if tick_budget < MIN_SIM_SLICE_SECS {
            self.update_division_motion(interaction_dt, false);
            if (now - self.last_status_print).as_secs_f32() >= 1.0 {
                self.update_title();
                self.last_status_print = now;
            }
            self.print_perf_diag(now);
            return;
        }

        // Daily work can be much heavier than an hourly clock step. Run the
        // queued daily/weekly/monthly systems cooperatively before advancing
        // more hours, so input and redraws get a chance between slices.
        if self.runtime.schedule.has_pending_interactive_work() {
            simulation_advanced |= runtime::systems_runtime::run_pending_interactive(
                &mut runtime::systems_runtime::HourlyRuntime {
                    world: &mut self.world,
                    econ: &mut self.runtime.econ,
                    research: &mut self.runtime.research,
                    politics_cache: &mut self.runtime.politics_cache,
                    script: &mut self.runtime.script,
                    ai: &mut self.runtime.ai,
                    v6_db: &self.v6_db,
                    schedule: &mut self.runtime.schedule,
                    feedback_bus: &mut self.runtime.feedback_bus,
                    content: &mut self.runtime.content,
                },
                tick_budget,
            );
            let pending_work_remains = self.runtime.schedule.has_pending_interactive_work();
            let frame_budget_spent =
                runtime::systems_runtime::should_stop_hourly_catchup(tick_started_at, tick_budget);
            stop_advancing_hours = pending_work_remains
                || frame_budget_spent
                || self.runtime.content.last_tick_events.day_changed;
        }

        if !stop_advancing_hours && secs_per_hour <= 0.0 {
            for _ in 0..24 {
                runtime::systems_runtime::tick_one_hour_interactive(
                    &mut runtime::systems_runtime::HourlyRuntime {
                        world: &mut self.world,
                        econ: &mut self.runtime.econ,
                        research: &mut self.runtime.research,
                        politics_cache: &mut self.runtime.politics_cache,
                        script: &mut self.runtime.script,
                        ai: &mut self.runtime.ai,
                        v6_db: &self.v6_db,
                        schedule: &mut self.runtime.schedule,
                        feedback_bus: &mut self.runtime.feedback_bus,
                        content: &mut self.runtime.content,
                    },
                );
                simulation_advanced = true;
                if self.runtime.schedule.has_pending_interactive_work() {
                    break;
                }
            }
        } else if !stop_advancing_hours {
            // Cap catch-up by both tick count and frame budget. Accumulated
            // time is preserved across daily-work frames so high speeds remain
            // linear instead of silently losing time at each day boundary.
            let accumulated_ticks = (self.time_accumulator / secs_per_hour).floor() as u32;
            let max_ticks = base_max_ticks.max(accumulated_ticks.min(base_max_ticks * 4));
            while self.time_accumulator >= secs_per_hour && ticks < max_ticks {
                runtime::systems_runtime::tick_one_hour_interactive(
                    &mut runtime::systems_runtime::HourlyRuntime {
                        world: &mut self.world,
                        econ: &mut self.runtime.econ,
                        research: &mut self.runtime.research,
                        politics_cache: &mut self.runtime.politics_cache,
                        script: &mut self.runtime.script,
                        ai: &mut self.runtime.ai,
                        v6_db: &self.v6_db,
                        schedule: &mut self.runtime.schedule,
                        feedback_bus: &mut self.runtime.feedback_bus,
                        content: &mut self.runtime.content,
                    },
                );
                self.time_accumulator -= secs_per_hour;
                ticks += 1;
                simulation_advanced = true;

                if self.runtime.schedule.has_pending_interactive_work() {
                    let elapsed = tick_started_at.elapsed().as_secs_f32();
                    let remaining_budget = (tick_budget - elapsed).max(0.0);
                    if remaining_budget > 0.0 {
                        simulation_advanced |= runtime::systems_runtime::run_pending_interactive(
                            &mut runtime::systems_runtime::HourlyRuntime {
                                world: &mut self.world,
                                econ: &mut self.runtime.econ,
                                research: &mut self.runtime.research,
                                politics_cache: &mut self.runtime.politics_cache,
                                script: &mut self.runtime.script,
                                ai: &mut self.runtime.ai,
                                v6_db: &self.v6_db,
                                schedule: &mut self.runtime.schedule,
                                feedback_bus: &mut self.runtime.feedback_bus,
                                content: &mut self.runtime.content,
                            },
                            remaining_budget,
                        );
                    }
                    if self.runtime.schedule.has_pending_interactive_work()
                        || self.runtime.content.last_tick_events.day_changed
                    {
                        break;
                    }
                }

                if runtime::systems_runtime::should_stop_hourly_catchup(
                    tick_started_at,
                    tick_budget,
                ) {
                    break;
                }
            }
        }

        if secs_per_hour.is_finite() && secs_per_hour > 0.0 {
            let max_carry = secs_per_hour * 96.0;
            if self.time_accumulator > max_carry {
                self.time_accumulator = max_carry;
            }
        }
        self.update_division_motion(interaction_dt, simulation_advanced);

        self.handle_content_tick_events();

        self.handle_new_war_auto_pause();

        self.handle_campaign_end_screen();

        self.update_music_autoadvance();

        if (now - self.last_status_print).as_secs_f32() >= 1.0 {
            self.update_title();
            self.last_status_print = now;
        }
        self.print_perf_diag(now);
    }
}
