use super::build::UiBuildOutput;
use crate::combat_overlay::show_combat_bubble_overlay;
use crate::*;

#[derive(Default)]
pub(crate) struct UiRenderOutput {
    pub(crate) topbar_speed_cmd: Option<hoi4_ui::topbar::SpeedCommand>,
    pub(crate) side_rail_panel_cmd: Option<hoi4_ui::PanelKind>,
    pub(crate) open_panel_kind: Option<hoi4_ui::PanelKind>,
    pub(crate) panel_commands: Vec<hoi4_ui::PanelCommand>,
    pub(crate) politics_close: bool,
    pub(crate) politics_decision_cmds: Vec<hoi4_ui::politics::DecisionCommand>,
    pub(crate) decisions_close: bool,
    pub(crate) decisions_cmds: Vec<hoi4_ui::politics::DecisionCommand>,
    pub(crate) law_close: bool,
    pub(crate) law_cmds: Vec<hoi4_ui::law_panel::LawCommand>,
    pub(crate) pop_panel_close: bool,
    pub(crate) market_close: bool,
    pub(crate) finance_close: bool,
    pub(crate) finance_cmds: Vec<hoi4_ui::finance_panel::FinanceCommand>,
    pub(crate) trade_close: bool,
    pub(crate) construction_v6_close: bool,
    pub(crate) construction_v6_cmds: Vec<hoi4_ui::construction_v6_panel::ConstructionV6Command>,
    pub(crate) research_close: bool,
    pub(crate) research_cmds: Vec<hoi4_ui::research::ResearchCommand>,
    pub(crate) diplomacy_close: bool,
    pub(crate) diplomacy_cmds: Vec<hoi4_ui::diplomacy::DiplomacyCommand>,
    pub(crate) military_close: bool,
    pub(crate) military_cmds: Vec<hoi4_ui::military::MilitaryCommand>,
    pub(crate) air_close: bool,
    pub(crate) air_cmds: Vec<hoi4_ui::air::AirCommand>,
    pub(crate) naval_close: bool,
    pub(crate) naval_cmds: Vec<hoi4_ui::naval::NavalCommand>,
    pub(crate) logistics_close: bool,
    pub(crate) situation_close: bool,
    pub(crate) situation_cmds: Vec<hoi4_ui::situation_panel::SituationCommand>,
    pub(crate) focus_cmd: Option<hoi4_ui::focus_tree_panel::FocusCommand>,
    pub(crate) country_info_cmds: Vec<hoi4_ui::country_info_panel::CountryInfoCommand>,
    pub(crate) counter_menu_cmd: Option<&'static str>,
    pub(crate) event_cmd: Option<hoi4_ui::event_panel::EventCommand>,
    pub(crate) surrender_notif_cmd:
        Option<hoi4_ui::surrender_notification::SurrenderNotificationCommand>,
    pub(crate) settings_close: bool,
    pub(crate) settings_cmds: Vec<hoi4_ui::settings::SettingsCommand>,
    pub(crate) saves_close: bool,
    pub(crate) save_cmds: Vec<hoi4_ui::save_browser::SaveCommand>,
    pub(crate) end_cmd: Option<hoi4_ui::end_screen::EndCommand>,
    pub(crate) egui_current_stats: hoi4_ui::UiFrameStats,
}

pub(crate) fn render_ui(app: &mut App, input: UiBuildOutput) -> UiRenderOutput {
    let UiBuildOutput {
        elapsed_secs,
        game_phase,
        player_country,
        demo_visible,
        topbar_data,
        open_panel_kind,
        active_detail_panel,
        event_badge_count,
        surrender_badge_count,
        open_panel,
        politics_data,
        decisions_panel_data,
        law_panel_data,
        pop_panel_data,
        market_panel_data,
        finance_panel_data,
        trade_panel_data,
        construction_v6_data,
        research_data,
        diplomacy_data,
        combat_bubbles,
        military_data,
        air_data,
        naval_data,
        logistics_data,
        situation_panel_data,
        player_cid,
        completed_focuses,
        current_focus_id,
        current_focus_progress,
        available_focus_ids,
        player_in_faction,
        province_info_bottom_bar_height,
        counter_rclick_prov,
        counter_menu_pos,
        settings_panel_open_cmd,
        saves_open_cmd,
        law_error_toast,
    } = input;
    let current_focus_ref = current_focus_id.as_deref();
    app.province_info_card.bottom_bar_height = province_info_bottom_bar_height;

    let mut topbar_speed_cmd: Option<hoi4_ui::topbar::SpeedCommand> = None;
    let mut side_rail_panel_cmd: Option<hoi4_ui::PanelKind> = None;
    let mut panel_commands: Vec<hoi4_ui::PanelCommand> = Vec::new();
    let mut politics_close = false;
    let mut politics_decision_cmds: Vec<hoi4_ui::politics::DecisionCommand> = Vec::new();
    let mut decisions_close = false;
    let mut decisions_cmds: Vec<hoi4_ui::politics::DecisionCommand> = Vec::new();
    let law_close = false;
    let mut law_cmds: Vec<hoi4_ui::law_panel::LawCommand> = Vec::new();
    let mut pop_panel_close = false;
    let mut market_close = false;
    let mut finance_close = false;
    let mut finance_cmds: Vec<hoi4_ui::finance_panel::FinanceCommand> = Vec::new();
    let mut trade_close = false;
    let mut construction_v6_close = false;
    let mut construction_v6_cmds: Vec<hoi4_ui::construction_v6_panel::ConstructionV6Command> =
        Vec::new();
    let mut research_close = false;
    let mut research_cmds: Vec<hoi4_ui::research::ResearchCommand> = Vec::new();
    let mut diplomacy_close = false;
    let mut diplomacy_cmds: Vec<hoi4_ui::diplomacy::DiplomacyCommand> = Vec::new();
    let mut military_close = false;
    let mut military_cmds: Vec<hoi4_ui::military::MilitaryCommand> = Vec::new();
    let mut air_close = false;
    let mut air_cmds: Vec<hoi4_ui::air::AirCommand> = Vec::new();
    let mut naval_close = false;
    let mut naval_cmds: Vec<hoi4_ui::naval::NavalCommand> = Vec::new();
    let mut logistics_close = false;
    let mut situation_close = false;
    let mut situation_cmds: Vec<hoi4_ui::situation_panel::SituationCommand> = Vec::new();
    let focus_tree = &app.content.focus_tree;
    let focus_panel = &mut app.focus_panel;
    let mut focus_cmd: Option<hoi4_ui::focus_tree_panel::FocusCommand> = None;
    let country_info_panel = &mut app.country_info_panel;
    let mut country_info_cmds: Vec<hoi4_ui::country_info_panel::CountryInfoCommand> = Vec::new();
    let province_info_card = &mut app.province_info_card;
    let province_info_data = &app.province_info_data;
    let mut counter_menu_cmd: Option<&'static str> = None;
    let event_scheduler = &app.content.event_scheduler;
    let event_world = &app.world;
    let event_flags = &app.content.global_flags;
    let event_country = hoi4_state::CountryId(app.player_country as u16);
    let mut event_cmd: Option<hoi4_ui::event_panel::EventCommand> = None;
    let mut surrender_notif_cmd: Option<
        hoi4_ui::surrender_notification::SurrenderNotificationCommand,
    > = None;
    if settings_panel_open_cmd && !app.settings_panel.open {
        app.settings_panel.open_with(app.settings.clone());
    }
    if saves_open_cmd && !app.save_browser.open {
        app.save_browser.open = true;
        let saves_dir = app.save_browser.saves_dir.clone();
        let entries = hoi4_ui::save_browser::scan_saves(&saves_dir, |p| {
            hoi4_state::save::read_meta(p)
                .ok()
                .map(|m| (format!("{}", m.date), m.player_tag))
        });
        app.save_browser.set_saves(entries);
    }
    let settings_panel = &mut app.settings_panel;
    let save_browser = &mut app.save_browser;
    let end_screen = &mut app.end_screen;
    let pending_surrender_notifications = &mut app.pending_surrender_notifications;
    app.last_law_error_toast = app.law_error_message.clone();
    let v9_notifications = &mut app.v9_notifications;
    let mut settings_close = false;
    let mut settings_cmds: Vec<hoi4_ui::settings::SettingsCommand> = Vec::new();
    let mut saves_close = false;
    let mut save_cmds: Vec<hoi4_ui::save_browser::SaveCommand> = Vec::new();
    let mut end_cmd: Option<hoi4_ui::end_screen::EndCommand> = None;

    let s = match app.state.as_mut() {
        Some(s) => s,
        None => return UiRenderOutput::default(),
    };
    let nine_slice = s.nine_slice_window.as_ref();
    let edge_px: f32 = nine_slice.map(|ns| ns.edges.left).unwrap_or(0.0);
    let icon_bank = &mut s.icon_bank;
    let b5_open = &mut app.b5_demo_visible;
    let demo_window = &mut app.demo_window;
    let v9_demo = &mut app.v9_demo;
    let ui_last_stats = s.ui.last_stats;
    let accessibility_settings = app.settings.accessibility();
    let mut v9_frame_profile: Option<hoi4_ui::v9::profiler::V9FrameProfile> = None;
    s.ui.begin_frame(&s.window, |ctx| {
        hoi4_ui::v9::accessibility::set_settings(ctx, accessibility_settings);
        hoi4_ui::v9::profiler::begin_frame_ctx(ctx);
        let frame = if nine_slice.is_some() {
            hoi4_ui::egui::Frame::default()
                .inner_margin(hoi4_ui::egui::Margin::same(edge_px as i8))
        } else {
            hoi4_ui::egui::Frame::window(&ctx.style())
        };
        hoi4_ui::egui::Window::new("hoi4-ui demo")
            .open(b5_open)
            .default_pos([20.0, 80.0])
            .default_width(420.0)
            .resizable(true)
            .frame(frame)
            .show(ctx, |ui| {
                if let Some(ns) = nine_slice {
                    let outer = ui.max_rect().expand(edge_px);
                    ns.paint(ui.painter(), outer, hoi4_ui::egui::Color32::WHITE);
                }

                ui.heading("Project Ironheart V5");
                ui.label("This is a Latin paragraph rendered with Georgia (Garamond-alike).");
                ui.label("CJK fallback font is enabled.");
                ui.label(format!(
                    "elapsed: {elapsed_secs:.2} s ??phase: {game_phase:?} ??player_country: {player_country}"
                ));
                ui.separator();

                ui.label("Vanilla focus icons (B.5):");
                ui.horizontal(|ui| {
                    for name in &["GFX_focus_GER_anschluss", "GFX_focus_GER_afrikakorps"] {
                        ui.vertical(|ui| {
                            let resp = hoi4_ui::icons::show_icon(
                                ui,
                                icon_bank,
                                name,
                                Some(hoi4_ui::egui::vec2(80.0, 70.0)),
                            );
                            if resp.is_none() {
                                ui.label(format!("Missing {name}"));
                            }
                            ui.label(name.trim_start_matches("GFX_focus_GER_"));
                        });
                    }
                });
                ui.separator();

                ui.label("Button state preview: default, hover, and pressed.");
                if ui.button("Test button").clicked() {
                    println!("[ui] demo button clicked");
                }
                if nine_slice.is_some() {
                    ui.label("Using vanilla tiled_window.dds 9-slice background.");
                } else {
                    ui.label("9-slice not loaded; using Frame::fill fallback.");
                }
                ui.separator();
                ui.label("Press F2 to toggle this UI demo.");
            });

        if demo_visible {
            demo_window.show(ctx, nine_slice, icon_bank, ui_last_stats);
        }

        v9_demo.show(ctx);

        if let Some(ref data) = topbar_data {
            let (speed, _res_click) = hoi4_ui::topbar::TopBar::show(ctx, data, icon_bank);
            topbar_speed_cmd = speed;

            let badge = (event_badge_count + surrender_badge_count).min(u8::MAX as usize) as u8;
            let rail_data = hoi4_ui::v9::composites::SideRailData::gameplay(open_panel_kind)
                .with_badge(hoi4_ui::PanelKind::Situation, badge);
            side_rail_panel_cmd = hoi4_ui::v9::composites::SideRail::show(ctx, &rail_data);
        }

        if let Some(ref data) = politics_data {
            let (close, cmds) = hoi4_ui::politics::PoliticsPanel::show(ctx, data, icon_bank);
            if close {
                politics_close = true;
            }
            politics_decision_cmds = cmds;
        }

        if let Some(ref data) = decisions_panel_data {
            let (close, cmds) = hoi4_ui::decisions_panel::DecisionsPanel::show(ctx, data);
            if close {
                decisions_close = true;
            }
            decisions_cmds = cmds;
        }

        // Gate 5.2: laws are rendered inside the politics workbench. `law_panel_data`
        // remains available for LawDetailPanel and no longer opens an isolated V9 report.

        if let Some(ref data) = pop_panel_data {
            let (close, cmds) = hoi4_ui::pop_panel::PopPanel::show(ctx, data);
            if close {
                pop_panel_close = true;
            }
            panel_commands.extend(cmds);
        }

        if let Some(ref data) = market_panel_data {
            let (close, cmds) = hoi4_ui::market_panel::MarketPanel::show(ctx, data);
            if close {
                market_close = true;
            }
            panel_commands.extend(cmds);
        }

        if let Some(ref data) = finance_panel_data {
            let (close, cmds) = hoi4_ui::finance_panel::FinancePanel::show(ctx, data);
            if close {
                finance_close = true;
            }
            finance_cmds = cmds;
        }

        if let Some(ref data) = trade_panel_data {
            let (close, cmds) = hoi4_ui::trade_panel::TradePanel::show(ctx, data);
            if close {
                trade_close = true;
            }
            panel_commands.extend(cmds);
        }

        if let Some(ref data) = construction_v6_data {
            let (close, cmds) =
                hoi4_ui::construction_v6_panel::ConstructionV6Panel::show(ctx, data);
            if close {
                construction_v6_close = true;
            }
            construction_v6_cmds = cmds;
        }

        if let Some(ref data) = research_data {
            let (close, cmds) = hoi4_ui::research::ResearchPanel::show(ctx, data);
            if close {
                research_close = true;
            }
            research_cmds = cmds;
        }

        if let Some(ref data) = diplomacy_data {
            let (close, cmds) = hoi4_ui::diplomacy::DiplomacyPanel::show(
                ctx,
                data,
                &mut app.diplomacy_sort_by_opinion,
                &mut app.diplomacy_selected_country_tag,
                icon_bank,
            );
            if close {
                diplomacy_close = true;
            }
            diplomacy_cmds = cmds;
        }

        if open_panel == Some(InGamePanel::Military) {
            if let Some(ref data) = military_data {
                let (close, cmds) =
                    hoi4_ui::military::MilitaryPanel::show_side_panel(ctx, data);
                if close {
                    military_close = true;
                }
                military_cmds = cmds;
            }
        }

        if let Some(ref data) = military_data {
            let bottom_cmds = hoi4_ui::military::MilitaryPanel::show_bottom_bar(ctx, data);
            military_cmds.extend(bottom_cmds);
            let detail_cmds = hoi4_ui::army_detail_panel::ArmyDetailPanel::show(ctx, data);
            military_cmds.extend(detail_cmds);
            let badge_cmds = hoi4_ui::army_badge::ArmyBadge::show(ctx, data);
            military_cmds.extend(badge_cmds);
        }

        if let Some(ref data) = naval_data {
            let (close, cmds) = hoi4_ui::naval::NavalPanel::show(ctx, data);
            if close {
                naval_close = true;
            }
            naval_cmds = cmds;
        }

        if let Some(ref data) = air_data {
            let (close, cmds) = hoi4_ui::air::AirPanel::show(ctx, data);
            if close {
                air_close = true;
            }
            air_cmds = cmds;
        }

        if let Some(ref data) = logistics_data {
            let (close, cmds) = hoi4_ui::logistics_panel::LogisticsPanel::show(ctx, data);
            if close {
                logistics_close = true;
            }
            panel_commands.extend(cmds);
        }

        if let Some(ref data) = situation_panel_data {
            let (close, cmds) = hoi4_ui::situation_panel::SituationPanel::show(ctx, data);
            if close {
                situation_close = true;
            }
            situation_cmds = cmds;
        }

        if focus_tree
            .country
            .eq_ignore_ascii_case(app.world.country_tag(player_cid).unwrap_or_default())
        {
            focus_cmd = focus_panel.show(
                ctx,
                focus_tree,
                &completed_focuses,
                current_focus_ref,
                current_focus_progress,
                &available_focus_ids,
            );
        } else {
            focus_panel.open = false;
        }

        country_info_cmds = country_info_panel.show(ctx, icon_bank, player_in_faction);

        province_info_card.show(ctx, province_info_data);

        if let Some(cmd) = hoi4_ui::detail_panel::DetailPanelHost::show(
            ctx,
            active_detail_panel.as_ref(),
            market_panel_data.as_ref(),
            finance_panel_data.as_ref(),
            law_panel_data.as_ref(),
            construction_v6_data.as_ref(),
            pop_panel_data.as_ref(),
            diplomacy_data.as_ref(),
            Some(province_info_data),
            military_data.as_ref(),
            naval_data.as_ref(),
            air_data.as_ref(),
            logistics_data.as_ref(),
            research_data.as_ref(),
            decisions_panel_data.as_ref(),
            Some(focus_tree),
            Some(&completed_focuses),
            current_focus_ref,
            current_focus_progress,
            Some(&available_focus_ids),
        ) {
            if let Some(panel_cmd) = cmd.panel_command {
                panel_commands.push(panel_cmd);
            }
            law_cmds.extend(cmd.law_commands);
            diplomacy_cmds.extend(cmd.diplomacy_commands);
            politics_decision_cmds.extend(cmd.decision_commands);
        }

        show_combat_bubble_overlay(ctx, &combat_bubbles, &mut app.selected_combat_bubble);

        if counter_rclick_prov.is_some() {
            hoi4_ui::egui::Window::new("Counter Menu")
                .fixed_pos([counter_menu_pos[0], counter_menu_pos[1]])
                .collapsible(false)
                .title_bar(false)
                .resizable(false)
                .show(ctx, |ui| {
                    if ui.button("Move to...").clicked() {
                        counter_menu_cmd = Some("move");
                    }
                    if ui.button("Attack...").clicked() {
                        counter_menu_cmd = Some("move");
                    }
                    if ui.button("Cancel orders").clicked() {
                        counter_menu_cmd = Some("cancel");
                    }
                    if ui.button("Disband").clicked() {
                        counter_menu_cmd = Some("disband");
                    }
                });
        }

        let front_event_id = event_scheduler.front().map(|pending| pending.event_id.clone());
        if app.last_event_sound_id.as_deref() != front_event_id.as_deref() {
            if let Some(ref event_id) = front_event_id {
                let popup_sound = event_scheduler
                    .db
                    .find(event_id)
                    .map(event_modal_sound)
                    .unwrap_or(UiSound::EventPopup);
                if !app
                    .ui_sounds
                    .play_with_fallback(popup_sound, UiSound::EventPopup)
                {
                    println!("[audio] event popup sound requested but no UI sound sample is loaded");
                }
            }
            app.last_event_sound_id = front_event_id.clone();
        }

        event_cmd = hoi4_ui::event_panel::show_event_modal_with_icons(
            ctx,
            event_scheduler,
            icon_bank,
            |id, idx| {
                let Some(ev) = event_scheduler.db.find(id) else {
                    return false;
                };
                let Some(opt) = ev.options.get(idx) else {
                    return false;
                };
                hoi4_content::eval_trigger(&opt.trigger, event_world, event_country, event_flags)
            },
        );

        if !pending_surrender_notifications.is_empty() {
            let remaining = pending_surrender_notifications.len().saturating_sub(1);
            let current = pending_surrender_notifications.first().unwrap();
            let surrender_sound_key = surrender_notification_sound_key(current);
            if app.last_surrender_sound_key.as_deref() != Some(surrender_sound_key.as_str()) {
                if !app
                    .ui_sounds
                    .play_with_fallback(UiSound::WorldDefeat, UiSound::EventPopup)
                {
                    println!(
                        "[audio] surrender notification sound requested but no UI sound sample is loaded"
                    );
                }
                app.last_surrender_sound_key = Some(surrender_sound_key);
            }
            surrender_notif_cmd = hoi4_ui::surrender_notification::show_surrender_notification(
                ctx, current, remaining, 0,
            );
        }

        {
            let (close, cmds) = settings_panel.show(ctx);
            if close {
                settings_close = true;
            }
            settings_cmds = cmds;
        }

        {
            let (close, cmds) = save_browser.show(ctx);
            if close {
                saves_close = true;
            }
            save_cmds = cmds;
        }

        end_cmd = end_screen.show(ctx);

        if let Some(ref err) = law_error_toast {
            v9_notifications.push_keyed(
                "law_error",
                "Law change failed",
                err.clone(),
                hoi4_ui::v9::primitives::ToastKind::Bad,
                4.0,
            );
        }
        v9_notifications.show(ctx);
        v9_frame_profile = hoi4_ui::v9::profiler::finish_frame_ctx(ctx);
    });
    let egui_current_stats = s.ui.last_stats;

    for event in hoi4_ui::v9::sound::drain(&s.ui.ctx) {
        let sound = match event.event {
            hoi4_ui::v9::sound::V9SoundEvent::Hover => UiSound::Hover,
            hoi4_ui::v9::sound::V9SoundEvent::Click => UiSound::Click,
            hoi4_ui::v9::sound::V9SoundEvent::Error => UiSound::Click,
            hoi4_ui::v9::sound::V9SoundEvent::Page => UiSound::PageFlip,
            hoi4_ui::v9::sound::V9SoundEvent::Modal => UiSound::EventPopup,
        };
        app.ui_sounds.play_with_fallback(sound, UiSound::Click);
    }
    if let Some(profile) = v9_frame_profile {
        if !profile.within_frame_budget() && app.perf_render_frames % 60 == 0 {
            println!("[v9-perf] {}", profile.report());
        }
    }

    UiRenderOutput {
        topbar_speed_cmd,
        side_rail_panel_cmd,
        open_panel_kind,
        panel_commands,
        politics_close,
        politics_decision_cmds,
        decisions_close,
        decisions_cmds,
        law_close,
        law_cmds,
        pop_panel_close,
        market_close,
        finance_close,
        finance_cmds,
        trade_close,
        construction_v6_close,
        construction_v6_cmds,
        research_close,
        research_cmds,
        diplomacy_close,
        diplomacy_cmds,
        military_close,
        military_cmds,
        air_close,
        air_cmds,
        naval_close,
        naval_cmds,
        logistics_close,
        situation_close,
        situation_cmds,
        focus_cmd,
        country_info_cmds,
        counter_menu_cmd,
        event_cmd,
        surrender_notif_cmd,
        settings_close,
        settings_cmds,
        saves_close,
        save_cmds,
        end_cmd,
        egui_current_stats,
    }
}
