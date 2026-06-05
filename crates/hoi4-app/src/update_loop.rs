use std::time::Instant;

use hoi4_runtime::ContentTickEvents;
use winit::keyboard::KeyCode;

use crate::{
    runtime, App, GamePhase, EDGE_PAN_MARGIN_PX, EDGE_PAN_SPEED_SCALE, MAX_INTERACTION_DT_SECS,
    MIN_SIM_SLICE_SECS,
};

impl App {
    pub(super) fn update(&mut self, max_sim_budget_secs: f32) {
        let now = Instant::now();
        let dt = (now - self.last_frame).as_secs_f32();
        let interaction_dt = dt.min(MAX_INTERACTION_DT_SECS);
        self.last_frame = now;

        if self.map_phase0.is_some() {
            return;
        }

        // Only run simulation and camera controls in Playing phase.
        if self.game_phase != GamePhase::Playing {
            return;
        }

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
            let dpi = s.window.scale_factor() as f32;
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

        let secs_per_hour = self.world.speed.seconds_per_hour();
        let mut simulation_advanced = false;
        let (base_max_ticks, speed_tick_budget) =
            runtime::systems_runtime::speed_tick_limits(self.world.speed);
        let tick_budget = speed_tick_budget.min(max_sim_budget_secs.max(0.0));
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
        if self.schedule.has_pending_interactive_work() {
            simulation_advanced |= runtime::systems_runtime::run_pending_interactive(
                &mut runtime::systems_runtime::HourlyRuntime {
                    world: &mut self.world,
                    econ: &mut self.econ,
                    research: &mut self.research,
                    politics_cache: &mut self.politics_cache,
                    script: &mut self.script,
                    ai: &mut self.ai,
                    v6_db: &self.v6_db,
                    schedule: &mut self.schedule,
                    feedback_bus: &mut self.feedback_bus,
                    content: &mut self.content,
                },
                tick_budget,
            );
            let pending_work_remains = self.schedule.has_pending_interactive_work();
            let frame_budget_spent =
                runtime::systems_runtime::should_stop_hourly_catchup(tick_started_at, tick_budget);
            stop_advancing_hours = pending_work_remains
                || frame_budget_spent
                || self.content.last_tick_events.day_changed;
        }

        if !stop_advancing_hours && secs_per_hour <= 0.0 {
            for _ in 0..24 {
                runtime::systems_runtime::tick_one_hour_interactive(
                    &mut runtime::systems_runtime::HourlyRuntime {
                        world: &mut self.world,
                        econ: &mut self.econ,
                        research: &mut self.research,
                        politics_cache: &mut self.politics_cache,
                        script: &mut self.script,
                        ai: &mut self.ai,
                        v6_db: &self.v6_db,
                        schedule: &mut self.schedule,
                        feedback_bus: &mut self.feedback_bus,
                        content: &mut self.content,
                    },
                );
                simulation_advanced = true;
                if self.schedule.has_pending_interactive_work() {
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
                        econ: &mut self.econ,
                        research: &mut self.research,
                        politics_cache: &mut self.politics_cache,
                        script: &mut self.script,
                        ai: &mut self.ai,
                        v6_db: &self.v6_db,
                        schedule: &mut self.schedule,
                        feedback_bus: &mut self.feedback_bus,
                        content: &mut self.content,
                    },
                );
                self.time_accumulator -= secs_per_hour;
                ticks += 1;
                simulation_advanced = true;

                if self.schedule.has_pending_interactive_work() {
                    let elapsed = tick_started_at.elapsed().as_secs_f32();
                    let remaining_budget = (tick_budget - elapsed).max(0.0);
                    if remaining_budget > 0.0 {
                        simulation_advanced |= runtime::systems_runtime::run_pending_interactive(
                            &mut runtime::systems_runtime::HourlyRuntime {
                                world: &mut self.world,
                                econ: &mut self.econ,
                                research: &mut self.research,
                                politics_cache: &mut self.politics_cache,
                                script: &mut self.script,
                                ai: &mut self.ai,
                                v6_db: &self.v6_db,
                                schedule: &mut self.schedule,
                                feedback_bus: &mut self.feedback_bus,
                                content: &mut self.content,
                            },
                            remaining_budget,
                        );
                    }
                    if self.schedule.has_pending_interactive_work()
                        || self.content.last_tick_events.day_changed
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

        // P0.1：content daily 已在 SystemSchedule 的 Content daily 系统中执行。
        // 此处读取 content.last_tick_events 处理 GUI 特有逻辑（暂停、刷新地图等）。
        let mut content_events = self.content.last_tick_events.clone();
        if content_events.day_changed {
            let auto_build_month = (self.world.date.year, self.world.date.month);
            if self.auto_build_enabled
                && self.world.date.day == 1
                && self.last_auto_build_month != Some(auto_build_month)
            {
                let added = Self::auto_enqueue_player_construction(
                    &self.world,
                    &mut self.econ,
                    &self.v6_db,
                    self.player_country,
                );
                self.last_auto_build_month = Some(auto_build_month);
                self.last_auto_build_explanations = added;
                if !self.last_auto_build_explanations.is_empty() {
                    println!(
                        "[construction] monthly auto-build queued {} projects",
                        self.last_auto_build_explanations.len()
                    );
                }
            }

            if content_events.focus_completed {
                self.world.speed = hoi4_state::GameSpeed::Paused;
                self.refresh_lut();
            }

            if !content_events.fired_events.is_empty() {
                if content_events.pause_for_country_event {
                    if self.world.speed != hoi4_state::GameSpeed::Paused
                        && self.pre_event_speed.is_none()
                    {
                        self.pre_event_speed = Some(self.world.speed);
                    }
                    self.world.speed = hoi4_state::GameSpeed::Paused;
                }
                for id in &content_events.fired_events {
                    println!("[event] fired: {id}");
                }
            }

            for ev in content_events.decision_events.drain(..) {
                use hoi4_content::DecisionTickEvent;
                match ev {
                    DecisionTickEvent::Completed(id) => println!("[decision] completed: {id}"),
                    DecisionTickEvent::Cancelled(id) => println!("[decision] cancelled: {id}"),
                }
            }

            // P1.3：效果执行错误报告
            self.apply_effect_report_app_requests(&content_events.effect_report);
            for w in &content_events.effect_report.warnings {
                println!("[effect] 警告: {w}");
            }
            for e in &content_events.effect_report.errors {
                println!("[effect] 错误: {e}");
            }

            if let Some(report) = &content_events.strategic_report {
                for name in &report.factions_created {
                    println!("[strategic] faction created: {name}");
                }
                for (leader, member) in &report.members_invited {
                    println!("[strategic] {leader} invited {member}");
                }
                for (joiner, faction) in &report.factions_joined {
                    println!("[strategic] {joiner} joined {faction}");
                }
            }

            // Situation effects need app-side map/label refresh handling.
            let had_ownership_change = runtime::situation_runtime::apply_situation_effects(self);
            if had_ownership_change {
                self.rebuild_country_labels_and_refresh();
            }

            let cascaded_from_situations = self.content.process_pending_triggers(&mut self.world);
            self.apply_effect_report_app_requests(&cascaded_from_situations.effect_report);
            for w in &cascaded_from_situations.effect_report.warnings {
                println!("[effect] 警告: {w}");
            }
            for e in &cascaded_from_situations.effect_report.errors {
                println!("[effect] 错误: {e}");
            }
            for target in &cascaded_from_situations.cascaded_triggers {
                println!("[event] cascaded trigger: {target}");
            }
            if cascaded_from_situations.pause_for_country_event {
                if self.world.speed != hoi4_state::GameSpeed::Paused
                    && self.pre_event_speed.is_none()
                {
                    self.pre_event_speed = Some(self.world.speed);
                }
                self.world.speed = hoi4_state::GameSpeed::Paused;
            }

            if !content_events.surrender_outcome.resolved.is_empty() {
                for eval in &content_events.surrender_outcome.resolved {
                    println!(
                        "[surrender] {} 投降 (首都丢失={}, 核心控制={:.0}%)",
                        eval.target,
                        eval.capital_lost,
                        eval.target_core_control_ratio * 100.0
                    );
                }
                for err in content_events.surrender_outcome.effect_errors.drain(..) {
                    println!("[surrender] effect error: {err}");
                }
                self.rebuild_country_labels_and_refresh();
            }

            // P1.1：和平自动结算通知（不暂停游戏）
            for resolved in &content_events.peace_resolution.resolved_wars {
                let (winner_tag, loser_tag) = match resolved.winning_side {
                    hoi4_state::WarSide::Attacker => (
                        &resolved.primary_attacker_tag,
                        &resolved.primary_defender_tag,
                    ),
                    hoi4_state::WarSide::Defender => (
                        &resolved.primary_defender_tag,
                        &resolved.primary_attacker_tag,
                    ),
                };
                let name_resolver = hoi4_app::ui_data::names::DisplayNameResolver::new(None);
                let winner_name = name_resolver.country_name(winner_tag, winner_tag);
                let loser_name = name_resolver.country_name(loser_tag, loser_tag);
                let mut results = Vec::new();
                if let Some(po) = &resolved.peace_outcome {
                    if po.annexed_countries > 0 {
                        results.push(hoi4_ui::surrender_notification::SurrenderResultEntry {
                            kind: hoi4_ui::surrender_notification::SurrenderResultKind::Annexed,
                            target_tag: loser_tag.clone(),
                            target_name: loser_name.clone(),
                            winner_tag: winner_tag.clone(),
                            winner_name: winner_name.clone(),
                            detail: format!("{} 吞并 {}", winner_tag, loser_tag),
                        });
                    }
                    if po.puppets_created > 0 {
                        results.push(hoi4_ui::surrender_notification::SurrenderResultEntry {
                            kind: hoi4_ui::surrender_notification::SurrenderResultKind::Puppeted,
                            target_tag: loser_tag.clone(),
                            target_name: loser_name.clone(),
                            winner_tag: winner_tag.clone(),
                            winner_name: winner_name.clone(),
                            detail: format!("{} 傀儡化 {}", winner_tag, loser_tag),
                        });
                    }
                    if po.states_transferred > 0 && po.annexed_countries == 0 {
                        results.push(hoi4_ui::surrender_notification::SurrenderResultEntry {
                            kind: hoi4_ui::surrender_notification::SurrenderResultKind::StateTransferred,
                            target_tag: loser_tag.clone(),
                            target_name: loser_name.clone(),
                            winner_tag: winner_tag.clone(),
                            winner_name: winner_name.clone(),
                            detail: format!("{} 占领 {} 州", winner_tag, po.states_transferred),
                        });
                    }
                    if po.governments_toppled > 0 {
                        results.push(hoi4_ui::surrender_notification::SurrenderResultEntry {
                            kind: hoi4_ui::surrender_notification::SurrenderResultKind::GovernmentToppled,
                            target_tag: loser_tag.clone(),
                            target_name: loser_name.clone(),
                            winner_tag: winner_tag.clone(),
                            winner_name: winner_name.clone(),
                            detail: format!("{} 推翻 {} 政府", winner_tag, loser_tag),
                        });
                    }
                }
                if results.is_empty() {
                    results.push(hoi4_ui::surrender_notification::SurrenderResultEntry {
                        kind: hoi4_ui::surrender_notification::SurrenderResultKind::WhitePeace,
                        target_tag: loser_tag.clone(),
                        target_name: loser_name.clone(),
                        winner_tag: winner_tag.clone(),
                        winner_name: winner_name.clone(),
                        detail: format!("{} 与 {} 白和", winner_tag, loser_tag),
                    });
                }
                self.pending_surrender_notifications.push(
                    hoi4_ui::surrender_notification::SurrenderNotification {
                        target_tag: loser_tag.clone(),
                        target_name: loser_name,
                        winner_tag: winner_tag.clone(),
                        winner_name,
                        kind: hoi4_ui::surrender_notification::SurrenderKind::AutoPeaceConference,
                        results,
                    },
                );
            }
            if content_events.peace_resolution.empty_wars_removed > 0 {
                self.pending_surrender_notifications.push(
                    hoi4_ui::surrender_notification::SurrenderNotification {
                        target_tag: String::new(),
                        target_name: String::new(),
                        winner_tag: String::new(),
                        winner_name: String::new(),
                        kind: hoi4_ui::surrender_notification::SurrenderKind::EmptyWarCleanup,
                        results: vec![hoi4_ui::surrender_notification::SurrenderResultEntry {
                            kind: hoi4_ui::surrender_notification::SurrenderResultKind::WarRemoved,
                            target_tag: String::new(),
                            target_name: String::new(),
                            winner_tag: String::new(),
                            winner_name: String::new(),
                            detail: format!(
                                "清理了 {} 场已结束的战争",
                                content_events.peace_resolution.empty_wars_removed
                            ),
                        }],
                    },
                );
            }
            if !content_events.peace_resolution.resolved_wars.is_empty()
                || content_events.peace_resolution.empty_wars_removed > 0
            {
                self.rebuild_country_labels_and_refresh();
            }

            // P1.2：级联触发后，检查 pending 队列中是否存在任意国家事件需要暂停
            for target in &content_events.cascaded_triggers {
                println!("[event] cascaded trigger: {target}");
            }
            if !content_events.cascaded_triggers.is_empty()
                && self
                    .content
                    .event_scheduler
                    .pending
                    .iter()
                    .any(|p| p.scope == hoi4_content::EventScope::Country)
            {
                if self.world.speed != hoi4_state::GameSpeed::Paused
                    && self.pre_event_speed.is_none()
                {
                    self.pre_event_speed = Some(self.world.speed);
                }
                self.world.speed = hoi4_state::GameSpeed::Paused;
            }

            self.refresh_map_if_province_ownership_changed();
        }

        self.content.last_tick_events = ContentTickEvents::default();

        // E.2: Auto-pause on new war involving player.
        let cur_wars = self.world.diplomacy.wars.len();
        if cur_wars > self.last_war_count {
            let player = hoi4_state::CountryId(self.player_country as u16);
            if self.world.diplomacy.is_at_war(player) {
                self.world.speed = hoi4_state::GameSpeed::Paused;
            }
        }
        self.last_war_count = cur_wars;

        if !self.end_screen.triggered
            && !self.end_screen.continued
            && self.game_phase == GamePhase::Playing
            && hoi4_ui::end_screen::EndScreen::should_trigger(
                self.world.date.year,
                self.world.date.month,
                self.world.date.day,
            )
        {
            let player = self.player_country;
            let player_cid = hoi4_state::CountryId(player as u16);
            let player_tag = self
                .world
                .countries
                .tags
                .get(player)
                .cloned()
                .unwrap_or_default();
            let divisions = (0..self.world.divisions.count)
                .filter(|&i| self.world.divisions.owners[i] == player_cid)
                .count() as u32;
            let provinces_occupied = (0..self.world.provinces.count)
                .filter(|&i| {
                    self.world.provinces.controllers[i] == player_cid
                        && self.world.provinces.owners[i] != player_cid
                })
                .count() as u32;
            let focuses_completed = self.world.countries.completed_focuses[player].len() as u32;
            let (civ, mil, dock) = Self::v6_industry_counts(&self.world, player_cid);
            let techs = self.world.countries.completed_techs[player].len() as u32;
            let faction_info = self
                .world
                .diplomacy
                .factions
                .iter()
                .find(|f| f.contains(player_cid))
                .map(|f| {
                    (
                        f.name.clone(),
                        f.members
                            .iter()
                            .map(|m| {
                                self.world
                                    .countries
                                    .tags
                                    .get(m.0 as usize)
                                    .cloned()
                                    .unwrap_or_default()
                            })
                            .collect::<Vec<_>>(),
                    )
                });
            let at_war_with: Vec<String> = self
                .world
                .diplomacy
                .wars
                .values()
                .filter(|w| w.attackers.contains(&player_cid) || w.defenders.contains(&player_cid))
                .flat_map(|w| {
                    let enemies = if w.attackers.contains(&player_cid) {
                        &w.defenders
                    } else {
                        &w.attackers
                    };
                    enemies.iter().map(|c| {
                        self.world
                            .countries
                            .tags
                            .get(c.0 as usize)
                            .cloned()
                            .unwrap_or_default()
                    })
                })
                .collect();
            let stats = hoi4_ui::end_screen::EndStats {
                player_tag,
                player_name: self
                    .world
                    .countries
                    .tags
                    .get(player)
                    .cloned()
                    .unwrap_or_default(),
                date_str: format!("{}", self.world.date),
                focuses_completed,
                civ_factories: civ,
                mil_factories: mil,
                shipyards: dock,
                divisions,
                provinces_occupied,
                at_war_with,
                faction_name: faction_info.as_ref().map(|(n, _)| n.clone()),
                faction_members: faction_info.map(|(_, m)| m).unwrap_or_default(),
                techs_researched: techs,
                world_tension: self.world.diplomacy.world_tension,
            };
            self.end_screen.trigger(stats);
            self.world.speed = hoi4_state::GameSpeed::Paused;
            println!("[game] campaign end reached: 1940-12-31");
        }

        // Auto-advance music when the current track finishes.
        self.music_player.tick_autoadvance();

        if (now - self.last_status_print).as_secs_f32() >= 1.0 {
            self.update_title();
            self.last_status_print = now;
        }
        self.print_perf_diag(now);
    }
}
