use crate::*;

impl App {
    pub(crate) fn handle_content_tick_events(&mut self) {
        // P0.1：content daily 已在 SystemSchedule 的 Content daily 系统中执行。
        // 此处读取 content.last_tick_events 处理 GUI 特有逻辑（暂停、刷新地图等）。
        let mut content_events = self.runtime.content.last_tick_events.clone();
        if content_events.day_changed {
            self.run_monthly_auto_build_if_due();

            if content_events.focus_completed {
                self.world.speed = hoi4_state::GameSpeed::Paused;
                self.refresh_lut();
            }

            if !content_events.fired_events.is_empty() {
                if content_events.pause_for_country_event {
                    if self.world.speed != hoi4_state::GameSpeed::Paused
                        && self.ui_state.pre_event_speed.is_none()
                    {
                        self.ui_state.pre_event_speed = Some(self.world.speed);
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

            let cascaded_from_situations = self
                .runtime
                .content
                .process_pending_triggers(&mut self.world);
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
                    && self.ui_state.pre_event_speed.is_none()
                {
                    self.ui_state.pre_event_speed = Some(self.world.speed);
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
                self.ui_state.pending_surrender_notifications.push(
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
                self.ui_state.pending_surrender_notifications.push(
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
                    .runtime
                    .content
                    .event_scheduler
                    .pending
                    .iter()
                    .any(|p| p.scope == hoi4_content::EventScope::Country)
            {
                if self.world.speed != hoi4_state::GameSpeed::Paused
                    && self.ui_state.pre_event_speed.is_none()
                {
                    self.ui_state.pre_event_speed = Some(self.world.speed);
                }
                self.world.speed = hoi4_state::GameSpeed::Paused;
            }

            self.refresh_map_if_province_ownership_changed();
        }

        self.runtime.content.last_tick_events = hoi4_runtime::ContentTickEvents::default();
    }
}
