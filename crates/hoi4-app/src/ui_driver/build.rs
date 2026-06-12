use std::collections::HashSet;

use crate::combat_overlay::CombatBubbleSnapshot;
use crate::*;

fn build_election_hemicycle_seats_from(election: &hoi4_content::Election1936) -> Vec<hoi4_ui::vanilla_gui::binding::HemicycleSeat> {
    let mut seats = Vec::new();
    let mut seat_index = 0u32;

    for bloc in &election.blocs {
        let color = hoi4_ui::vanilla_gui::binding::HemicycleSeat::color_from_rgb(bloc.color.0, bloc.color.1, bloc.color.2);
        for party in &bloc.parties {
            for _ in 0..party.seats {
                seats.push(hoi4_ui::vanilla_gui::binding::HemicycleSeat {
                    angle_index: seat_index,
                    ring: 0,
                    color,
                    party_id: party.id.clone(),
                });
                seat_index += 1;
            }
        }
    }

    seats
}

pub(crate) struct UiBuildOutput {
    pub(crate) elapsed_secs: f32,
    pub(crate) game_phase: GamePhase,
    pub(crate) player_country: usize,
    pub(crate) demo_visible: bool,
    pub(crate) topbar_data: Option<hoi4_ui::topbar::TopBarData>,
    pub(crate) open_panel_kind: Option<hoi4_ui::PanelKind>,
    pub(crate) active_detail_panel: Option<hoi4_ui::ActiveDetailPanel>,
    pub(crate) event_badge_count: usize,
    pub(crate) surrender_badge_count: usize,
    pub(crate) open_panel: Option<InGamePanel>,
    pub(crate) politics_data: Option<hoi4_ui::politics::PoliticsData>,
    pub(crate) decisions_panel_data: Option<hoi4_ui::decisions_panel::DecisionsData>,
    pub(crate) law_panel_data: Option<hoi4_ui::law_panel::LawPanelData>,
    pub(crate) pop_panel_data: Option<hoi4_ui::pop_panel::PopPanelData>,
    pub(crate) market_panel_data: Option<hoi4_ui::market_panel::MarketPanelData>,
    pub(crate) finance_panel_data: Option<hoi4_ui::finance_panel::FinancePanelData>,
    pub(crate) trade_panel_data: Option<hoi4_ui::trade_panel::TradePanelData>,
    pub(crate) construction_v6_data:
        Option<hoi4_ui::construction_v6_panel::ConstructionV6PanelData>,
    pub(crate) research_data: Option<hoi4_ui::research::ResearchData>,
    pub(crate) diplomacy_data: Option<hoi4_ui::diplomacy::DiplomacyData>,
    pub(crate) combat_bubbles: Vec<CombatBubbleSnapshot>,
    pub(crate) military_data: Option<hoi4_ui::military::MilitaryData>,
    pub(crate) air_data: Option<hoi4_ui::air::AirData>,
    pub(crate) naval_data: Option<hoi4_ui::naval::NavalData>,
    pub(crate) logistics_data: Option<hoi4_ui::logistics_panel::LogisticsData>,
    pub(crate) situation_panel_data: Option<hoi4_ui::situation_panel::SituationPanelData>,
    pub(crate) player_cid: hoi4_state::CountryId,
    pub(crate) completed_focuses: HashSet<String>,
    pub(crate) current_focus_id: Option<String>,
    pub(crate) current_focus_progress: f32,
    pub(crate) available_focus_ids: HashSet<String>,
    pub(crate) player_in_faction: bool,
    pub(crate) province_info_bottom_bar_height: f32,
    pub(crate) counter_rclick_prov: Option<u32>,
    pub(crate) counter_menu_pos: [f32; 2],
    pub(crate) settings_panel_open_cmd: bool,
    pub(crate) saves_open_cmd: bool,
    pub(crate) law_error_toast: Option<String>,
}

pub(crate) fn build_ui_data(app: &mut App, app_ui_enabled: bool) -> UiBuildOutput {
    let mut ui_frame_model = ui_binding::build_frame_model(app);
    app.ui_state.panel_cache.begin_frame();

    // Begin the egui frame before rendering; painting happens after 3D passes.
    let elapsed_secs = app.start_time.elapsed().as_secs_f32();
    let game_phase = app.view.game_phase;
    let player_country = app.view.player_country;
    let demo_visible = app_ui_enabled && app.demo_visible;
    let topbar_data = if app_ui_enabled {
        ui_frame_model.topbar.take()
    } else {
        None
    };
    let active_primary_panel = if app_ui_enabled {
        ui_frame_model.active_primary_panel
    } else {
        None
    };
    let open_panel_kind = active_primary_panel.map(hoi4_ui::PanelKind::from);
    let active_detail_panel = if app_ui_enabled {
        ui_frame_model.active_detail_panel.clone()
    } else {
        None
    };
    let event_badge_count = app.runtime.content.event_scheduler.pending_len();
    let surrender_badge_count = app.ui_state.pending_surrender_notifications.len();
    let open_panel = if app_ui_enabled {
        app.ui_state.open_panel
    } else {
        None
    };
    let politics_data = if matches!(
        open_panel,
        Some(InGamePanel::Politics) | Some(InGamePanel::Laws)
    ) {
        let player = player_country;
        let ruling = app
            .world
            .countries
            .ruling_party
            .get(player)
            .cloned()
            .unwrap_or_default();
        let pop_map = app
            .world
            .countries
            .party_popularity
            .get(player)
            .cloned()
            .unwrap_or_default();
        let pops = normalize_politics_party_popularity(pop_map, &ruling);
        let ideas = app
            .world
            .countries
            .ideas
            .get(player)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|idea_key| {
                if idea_key.starts_with("FLAG:")
                    || idea_key.starts_with("RESOURCE_DISCOVERY:")
                    || idea_key.starts_with("PM_UNLOCK:")
                    || idea_key.starts_with("GOV_ORDER:")
                    || idea_key.starts_with("MIL_SPENDING_SHARE:")
                {
                    return None;
                }
                let idea_def = app.world.data.ideas.get(&idea_key)?;
                if !is_politics_national_spirit_category(&idea_def.category) {
                    return None;
                }
                let name = localized_content_name(&idea_def.key, &idea_def.key);
                let modifiers = idea_def
                    .modifiers
                    .iter()
                    .map(|(k, v)| (k.clone(), *v))
                    .collect();
                Some(hoi4_ui::politics::IdeaEntry {
                    key: idea_def.key.clone(),
                    name,
                    category: idea_def.category.clone(),
                    picture: idea_def.picture.clone(),
                    modifiers,
                })
            })
            .collect::<Vec<_>>();
        let pp = app
            .world
            .countries
            .political_power
            .get(player)
            .copied()
            .unwrap_or(0.0);
        let player_tag_str = app
            .world
            .countries
            .tags
            .get(player)
            .cloned()
            .unwrap_or_default();
        let focus_available = app
            .runtime
            .content
            .focus_tree
            .country
            .eq_ignore_ascii_case(&player_tag_str);
        let (leader_name, leader_portrait_key) = hoi4_app::ui_data::country::head_of_state_display(
            &app.world,
            &app.historical_1936,
            hoi4_state::CountryId(player as u16),
        );
        let party_loc_key_long = format!("{}_{}_party_long", player_tag_str, ruling);
        let party_loc_key = format!("{}_{}_party", player_tag_str, ruling);
        let party_full_name = app
            .world
            .data
            .party_names
            .get(&party_loc_key_long)
            .or_else(|| app.world.data.party_names.get(&party_loc_key))
            .cloned()
            .unwrap_or_default();
        let party_names = pops
            .iter()
            .map(|(ideology, _)| {
                let long_key = format!("{}_{}_party_long", player_tag_str, ideology);
                let short_key = format!("{}_{}_party", player_tag_str, ideology);
                let name = app
                    .world
                    .data
                    .party_names
                    .get(&short_key)
                    .or_else(|| app.world.data.party_names.get(&long_key))
                    .cloned()
                    .unwrap_or_else(|| hoi4_ui::i18n::tr(ideology).to_owned());
                (ideology.clone(), name)
            })
            .collect::<Vec<_>>();
        let current_focus_id = app
            .world
            .countries
            .current_focus
            .get(player)
            .and_then(|f| f.as_deref());
        let current_focus_progress = app
            .world
            .countries
            .focus_progress
            .get(player)
            .copied()
            .unwrap_or(0.0);
        let current_focus = current_focus_id.and_then(|id| {
            app.runtime
                .content
                .focus_tree
                .focuses
                .iter()
                .find(|focus| focus.id == id)
        });
        let current_focus_name = current_focus.map(|focus| {
            let t = hoi4_ui::i18n::tr(&focus.id);
            if t == focus.id {
                focus.name.clone()
            } else {
                t.to_owned()
            }
        });
        let current_focus_cost_days = current_focus.map(|focus| focus.cost_days);
        let current_focus_name_for_panel = if focus_available {
            current_focus_name.clone()
        } else {
            None
        };
        let current_focus_cost_days_for_panel = if focus_available {
            current_focus_cost_days
        } else {
            None
        };
        let ruling_support = pops
            .iter()
            .find(|(key, _)| key == &ruling)
            .map(|(_, pop)| *pop)
            .unwrap_or(0.0)
            .clamp(0.0, 1.0);
        let leader_display = if leader_name.is_empty() {
            hoi4_ui::i18n::tr("leader_unknown").to_owned()
        } else {
            leader_name.clone()
        };
        let ideology_display = hoi4_ui::i18n::tr(&ruling).to_owned();
        let party_display = if party_full_name.is_empty() {
            ideology_display.clone()
        } else {
            party_full_name.clone()
        };
        let focus_post_name = current_focus_name_for_panel.clone().unwrap_or_else(|| {
            if focus_available {
                "未选择国策".to_owned()
            } else {
                "国策树未接入".to_owned()
            }
        });
        let focus_post_detail = current_focus_cost_days_for_panel
            .filter(|days| *days > 0)
            .map(|days| {
                format!(
                    "{:.0}% 进度",
                    (current_focus_progress / days as f32).clamp(0.0, 1.0) * 100.0
                )
            })
            .unwrap_or_else(|| {
                if focus_available {
                    "可打开国策".to_owned()
                } else {
                    "无本国国策树".to_owned()
                }
            });
        let government_posts = vec![
            hoi4_ui::politics::GovernmentPostEntry {
                office: "国家元首".to_owned(),
                name: leader_display,
                detail: player_tag_str.clone(),
            },
            hoi4_ui::politics::GovernmentPostEntry {
                office: "执政党".to_owned(),
                name: party_display,
                detail: format!("{:.0}% 支持", ruling_support * 100.0),
            },
            hoi4_ui::politics::GovernmentPostEntry {
                office: "意识形态".to_owned(),
                name: ideology_display,
                detail: ruling.clone(),
            },
            hoi4_ui::politics::GovernmentPostEntry {
                office: "当前国策".to_owned(),
                name: focus_post_name,
                detail: focus_post_detail,
            },
            hoi4_ui::politics::GovernmentPostEntry {
                office: "顾问系统".to_owned(),
                name: "未接入".to_owned(),
                detail: "无内阁槽位".to_owned(),
            },
        ];
        let law_slots = build_politics_law_entries(
            app.world.countries.law_store.law_sets.get(player),
            &app.v6_db,
        );

        let player_tag_for_election = app.world.countries.tags.get(player).cloned().unwrap_or_default();
        let election_hemicycle_seats = if player_tag_for_election == "SPR" && !app.world.countries.ideas.get(player).map(|ideas| ideas.iter().any(|s| s == "FLAG:spanish_civil_war_started")).unwrap_or(false) {
            app.runtime.content.elections.get("spr_election_1936").map(|e| build_election_hemicycle_seats_from(e)).unwrap_or_default()
        } else {
            Vec::new()
        };

        Some(hoi4_ui::politics::PoliticsData {
            ruling_party: ruling,
            party_popularity: pops,
            ideas,
            political_power: pp,
            stability: app
                .world
                .countries
                .stability
                .get(player)
                .copied()
                .unwrap_or(0.5),
            war_support: app
                .world
                .countries
                .war_support
                .get(player)
                .copied()
                .unwrap_or(0.0),
            focus_available,
            current_focus_name: current_focus_name_for_panel,
            current_focus_progress: if focus_available {
                current_focus_progress
            } else {
                0.0
            },
            current_focus_cost_days: current_focus_cost_days_for_panel,
            country_tag: player_tag_str,
            leader_name,
            leader_portrait_key,
            party_full_name,
            party_names,
            government_posts,
            law_slots,
            election_hemicycle_seats,
            show_election_panel: app.ui_state.show_election_panel,
        })
    } else {
        None
    };
    let decisions_panel_data = if open_panel == Some(InGamePanel::Decisions) {
        let player = player_country;
        Some(hoi4_ui::decisions_panel::DecisionsData {
            country_tag: app
                .world
                .countries
                .tags
                .get(player)
                .cloned()
                .unwrap_or_default(),
            political_power: app
                .world
                .countries
                .political_power
                .get(player)
                .copied()
                .unwrap_or(0.0),
            mechanics: app.build_decision_mechanics(player),
            decisions: app.build_decision_entries(player),
            country_flags: app
                .world
                .countries
                .ideas
                .get(player)
                .map(|ideas| {
                    ideas
                        .iter()
                        .filter_map(|s| s.strip_prefix("FLAG:").map(|f| f.to_owned()))
                        .collect()
                })
                .unwrap_or_default(),
            prewar: app.build_prewar_standoff(player),
        })
    } else {
        None
    };
    let needs_law_panel_data = open_panel == Some(InGamePanel::Laws)
        || open_panel == Some(InGamePanel::Politics)
        || matches!(
            active_detail_panel.as_ref(),
            Some(hoi4_ui::ActiveDetailPanel::Law { .. })
        );
    let law_panel_data = if needs_law_panel_data {
        let player = player_country;
        let pp = app
            .world
            .countries
            .political_power
            .get(player)
            .copied()
            .unwrap_or(0.0);
        let slots = build_law_slot_entries(
            app.world.countries.law_store.law_sets.get(player),
            &app.v6_db,
        );
        Some(hoi4_ui::law_panel::LawPanelData {
            political_power: pp,
            slots,
        })
    } else {
        None
    };
    let needs_pop_panel_data = open_panel == Some(InGamePanel::Pops)
        || matches!(
            active_detail_panel.as_ref(),
            Some(hoi4_ui::ActiveDetailPanel::PopGroup(_))
        );
    let pop_panel_data = if needs_pop_panel_data {
        hoi4_app::ui_data::pops::panel_data(
            &app.world,
            &app.v6_db,
            &app.loc_catalog,
            player_country,
        )
    } else {
        None
    };
    let needs_market_panel_data = open_panel == Some(InGamePanel::Market)
        || matches!(
            active_detail_panel.as_ref(),
            Some(hoi4_ui::ActiveDetailPanel::Goods(_))
        );
    let market_panel_data = if needs_market_panel_data {
        hoi4_app::ui_data::cache::cached_market_panel(
            &mut app.ui_state.panel_cache,
            &app.world,
            &app.v6_db,
            &app.runtime.econ,
            player_country,
        )
    } else {
        None
    };
    let finance_panel_data = if open_panel == Some(InGamePanel::Finance)
        || matches!(
            active_detail_panel.as_ref(),
            Some(hoi4_ui::ActiveDetailPanel::FinanceDebt)
        ) {
        hoi4_app::ui_data::cache::cached_finance_panel(
            &mut app.ui_state.panel_cache,
            &app.world,
            &app.v6_db,
            &app.runtime.econ,
            player_country,
        )
    } else {
        None
    };
    let trade_panel_data = if open_panel == Some(InGamePanel::Trade) {
        hoi4_app::ui_data::market::trade_panel(&app.world, &app.v6_db, player_country)
    } else {
        None
    };
    let needs_construction_data = open_panel == Some(InGamePanel::ConstructionV6)
        || matches!(
            active_detail_panel.as_ref(),
            Some(hoi4_ui::ActiveDetailPanel::Building(_) | hoi4_ui::ActiveDetailPanel::State(_))
        );
    let construction_v6_data = if needs_construction_data {
        hoi4_app::ui_data::cache::cached_construction_panel(
            &mut app.ui_state.panel_cache,
            &app.world,
            &app.v6_db,
            &app.runtime.econ,
            player_country,
            &app.ui_state.construction_mode,
            app.runtime.auto_build_enabled,
            &app.runtime.last_auto_build_explanations,
        )
    } else {
        None
    };
    let research_data = if open_panel == Some(InGamePanel::Research) {
        let player = player_country;
        let slots: Vec<hoi4_ui::research::SlotEntry> = app
            .runtime
            .research
            .slots
            .get(player)
            .map(|ss| {
                ss.iter()
                    .filter_map(|s| match s {
                        hoi4_logic::research::ResearchSlot::Active {
                            tech_key,
                            progress,
                            total_cost,
                        } => Some(hoi4_ui::research::SlotEntry {
                            tech_key: tech_key.clone(),
                            progress: if *total_cost > 0.0 {
                                progress / *total_cost
                            } else {
                                0.0
                            },
                        }),
                        _ => None,
                    })
                    .collect()
            })
            .unwrap_or_default();
        let slot_count = app
            .runtime
            .research
            .slots
            .get(player)
            .map(|s| s.len())
            .unwrap_or(0);
        let completed = &app.world.countries.completed_techs[player];
        let researching_keys: Vec<&str> = slots.iter().map(|s| s.tech_key.as_str()).collect();
        let techs: Vec<hoi4_ui::research::TechNode> = app
            .v6_db
            .technologies
            .iter()
            .map(|t| {
                let key = t.id.clone();
                hoi4_ui::research::TechNode {
                    key: key.clone(),
                    name: localized_content_name(&t.id, &t.name),
                    category: App::v6_tech_category_key(t.category).to_owned(),
                    start_year: t.start_year,
                    completed: completed.contains(&key),
                    researching: researching_keys.contains(&key.as_str()),
                    progress: slots
                        .iter()
                        .find(|s| s.tech_key == key)
                        .map(|s| s.progress)
                        .unwrap_or(0.0),
                    prerequisites: t
                        .prereqs
                        .iter()
                        .map(|id| {
                            app.v6_db
                                .technologies
                                .iter()
                                .find(|tech| tech.id == *id)
                                .map(|tech| localized_content_name(&tech.id, &tech.name))
                                .unwrap_or_else(|| id.clone())
                        })
                        .collect(),
                    unlock_summary: App::v6_tech_unlock_summary(&app.v6_db, t),
                }
            })
            .collect();
        Some(hoi4_ui::research::ResearchData {
            current_year: app.world.date.year,
            slots,
            slot_count,
            techs,
        })
    } else {
        None
    };
    let needs_diplomacy_data = open_panel == Some(InGamePanel::Diplomacy)
        || matches!(
            active_detail_panel.as_ref(),
            Some(hoi4_ui::ActiveDetailPanel::Country(_))
        );
    let diplomacy_data = if needs_diplomacy_data {
        let selected_tag = match active_detail_panel.as_ref() {
            Some(hoi4_ui::ActiveDetailPanel::Country(target)) => Some(target.tag.clone()),
            _ => app
                .ui_state
                .diplomacy_selected_country_tag
                .clone()
                .or_else(|| {
                    app.world
                        .countries
                        .tags
                        .iter()
                        .enumerate()
                        .find(|(idx, tag)| *idx != player_country && !tag.is_empty())
                        .map(|(_, tag)| tag.clone())
                }),
        };
        hoi4_app::ui_data::cache::cached_diplomacy_panel(
            &mut app.ui_state.panel_cache,
            &app.world,
            &app.historical_1936,
            &app.v6_db,
            player_country,
            selected_tag,
            app.ui_state.settings.instant_war,
        )
    } else {
        None
    };
    let combat_bubbles = app.collect_combat_bubbles();
    let military_data = if game_phase == GamePhase::Playing {
        let cache_started = Instant::now();
        let side_panel_open = open_panel == Some(InGamePanel::Military);
        let player = player_country;
        let player_cid = hoi4_state::CountryId(player as u16);
        let player_tag = app
            .world
            .countries
            .tags
            .get(player)
            .cloned()
            .unwrap_or_default();
        let divisions: Vec<hoi4_ui::military::DivisionEntry> = if open_panel
            == Some(InGamePanel::Military)
            || app.interaction.selected_army_id.is_some()
        {
            (0..app.world.divisions.count)
                .filter(|&i| app.world.divisions.owners[i] == player_cid)
                .map(|i| hoi4_ui::military::DivisionEntry {
                    province_name: app
                        .province_display_name_by_id(app.world.divisions.locations[i].0),
                    index: i,
                    name: app.world.divisions.names[i].clone(),
                    organisation: app.world.divisions.organisation[i],
                    max_organisation: app.world.divisions.max_organisation[i],
                    experience: app.world.divisions.experience[i],
                    strength: app.world.divisions.strength[i],
                    in_combat: app.world.divisions.in_combat[i],
                    province_id: app.world.divisions.locations[i].0,
                    equipment_ratio: app.world.divisions.strength[i],
                    army_id: app
                        .world
                        .player_armies
                        .iter()
                        .find(|a| a.members.contains(&i))
                        .map(|a| a.id.raw()),
                    army_name: app
                        .world
                        .player_armies
                        .iter()
                        .find(|a| a.members.contains(&i))
                        .map(|a| a.name.clone()),
                })
                .collect()
        } else {
            Vec::new()
        };
        let templates: Vec<hoi4_ui::military::TemplateEntry> =
            if open_panel == Some(InGamePanel::Military) {
                let stockpile = app
                    .runtime
                    .econ
                    .stockpile
                    .get(player)
                    .cloned()
                    .unwrap_or_default();
                app.world
                    .data
                    .division_templates
                    .get(&player_tag)
                    .map(|ts| {
                        ts.iter()
                            .enumerate()
                            .map(|(i, t)| {
                                let preview = hoi4_logic::military::templates::preview_template(
                                    t,
                                    app.world.data.as_ref(),
                                    Some(&stockpile),
                                );
                                hoi4_ui::military::TemplateEntry {
                                    index: i as u16,
                                    name: t.name.clone(),
                                    battalion_count: t.battalion_count(),
                                    combat_width: preview.combat_width,
                                    manpower: preview.manpower,
                                    training_days: preview.training_days,
                                    stockpile_satisfied_divisions: preview
                                        .stockpile_satisfied_divisions,
                                }
                            })
                            .collect()
                    })
                    .unwrap_or_default()
            } else {
                Vec::new()
            };
        let template_editor = if open_panel == Some(InGamePanel::Military) {
            let stockpile = app
                .runtime
                .econ
                .stockpile
                .get(player)
                .cloned()
                .unwrap_or_default();
            build_template_editor_data(
                app.world.data.as_ref(),
                &player_tag,
                app.interaction.selected_template_idx,
                Some(&stockpile),
            )
        } else {
            empty_template_editor_data()
        };
        let template_subunit_picker = if open_panel == Some(InGamePanel::Military) {
            build_template_subunit_picker_data(
                app.world.data.as_ref(),
                &player_tag,
                app.interaction.template_picker_target,
            )
        } else {
            empty_template_subunit_picker_data()
        };
        let training_queue: Vec<hoi4_ui::military::TrainingQueueEntry> =
            if open_panel == Some(InGamePanel::Military) {
                app.runtime
                    .econ
                    .training_queues
                    .get(player)
                    .map(|queue| {
                        queue
                            .iter()
                            .map(|item| {
                                let template_name = app
                                    .world
                                    .data
                                    .division_templates
                                    .get(&player_tag)
                                    .and_then(|ts| ts.get(item.template_id as usize))
                                    .map(|t| t.name.clone())
                                    .unwrap_or_else(|| format!("Template {}", item.template_id));
                                let required_manpower = app
                                    .world
                                    .data
                                    .division_templates
                                    .get(&player_tag)
                                    .and_then(|ts| ts.get(item.template_id as usize))
                                    .map(|t| {
                                        hoi4_logic::military::stats::DivisionStats::aggregate(
                                            t,
                                            app.world.data.as_ref(),
                                        )
                                        .manpower
                                    })
                                    .unwrap_or(0);
                                hoi4_ui::military::TrainingQueueEntry {
                                    id: item.id,
                                    template_name,
                                    count: item.count,
                                    progress: item.progress_days / item.required_days.max(1.0),
                                    manpower_allocated: item.manpower_allocated,
                                    required_manpower,
                                }
                            })
                            .collect()
                    })
                    .unwrap_or_default()
            } else {
                Vec::new()
            };
        let capital_prov = app
            .world
            .countries
            .capitals
            .get(player)
            .map(|s| {
                app.world
                    .states
                    .provinces
                    .get(s.0 as usize)
                    .and_then(|ps| ps.first())
                    .map(|p| p.0)
                    .unwrap_or(0)
            })
            .unwrap_or(0);
        let armies: Vec<hoi4_ui::military::ArmyEntry> = app
            .world
            .player_armies
            .iter()
            .filter(|a| a.owner == player_cid)
            .map(|a| {
                let general = a
                    .commander
                    .and_then(|gid| app.world.generals.iter().find(|g| g.id == gid));
                let modifier = general.map(|g| {
                    hoi4_logic::military::general::GeneralModifier::from_general(g, a.members.len())
                });
                hoi4_ui::military::ArmyEntry {
                    id: a.id.raw(),
                    name: a.name.clone(),
                    member_count: a.members.len(),
                    has_path: a.order.as_ref().map_or(false, |o| !o.path.is_empty()),
                    has_arrow: a.order.as_ref().map_or(false, |o| o.arrow.is_some()),
                    active: a.order.as_ref().map_or(false, |o| o.active),
                    executing: a.order.as_ref().map_or(false, |o| o.executing),
                    commander: a.commander.map(|id| id.raw()),
                    commander_name: general.map(|g| g.name.clone()),
                    command_limit: general.map(|g| g.command_limit),
                    command_efficiency: modifier.map(|m| m.command_efficiency).unwrap_or(1.0),
                    attack_bonus_pct: modifier
                        .map(|m| (m.attack_mult - 1.0) * 100.0)
                        .unwrap_or(0.0),
                    defense_bonus_pct: modifier
                        .map(|m| (m.defense_mult - 1.0) * 100.0)
                        .unwrap_or(0.0),
                    planning_bonus_pct: modifier
                        .map(|m| (m.plan_efficiency_mult - 1.0) * 100.0)
                        .unwrap_or(0.0),
                    org_recovery_bonus_pct: modifier
                        .map(|m| (m.org_recovery_mult - 1.0) * 100.0)
                        .unwrap_or(0.0),
                    supply_reduction_pct: modifier
                        .map(|m| (1.0 - m.supply_mult) * 100.0)
                        .unwrap_or(0.0),
                }
            })
            .collect();
        let generals: Vec<hoi4_ui::military::GeneralEntry> = app
            .world
            .generals
            .iter()
            .filter(|g| g.owner == player_cid)
            .map(|g| {
                let assigned_army_id = app
                    .world
                    .player_armies
                    .iter()
                    .find(|a| a.owner == player_cid && a.commander == Some(g.id))
                    .map(|a| a.id.raw());
                hoi4_ui::military::GeneralEntry {
                    id: g.id.raw(),
                    name: g.name.clone(),
                    skill: g.skill,
                    attack: g.attack,
                    defense: g.defense,
                    planning: g.planning,
                    logistics: g.logistics,
                    command_limit: g.command_limit,
                    assigned_army_id,
                }
            })
            .collect();
        let active_army_count = armies.iter().filter(|a| a.active).count();
        let painter_info = match app.interaction.frontline_painter.mode {
            PainterMode::Idle => hoi4_ui::military::PainterModeInfo::Idle,
            PainterMode::ArmyPainter(id) => {
                hoi4_ui::military::PainterModeInfo::ArmyPainter(id.raw())
            }
            PainterMode::ArrowPainter(id, _) => {
                hoi4_ui::military::PainterModeInfo::ArrowPainter(id.raw())
            }
        };
        let data = Some(hoi4_ui::military::MilitaryData {
            divisions,
            templates,
            template_editor_open: app.interaction.template_editor_open,
            template_editor,
            template_subunit_picker,
            training_queue,
            player_capital_province: capital_prov,
            armies,
            generals,
            frontline_overlay_visible: app.render_toggles.frontline_overlay_visible,
            selected_division_count: app.interaction.selected_divisions.len(),
            active_army_count,
            max_armies_per_country: hoi4_logic::military::frontline::MAX_FRONTLINES_PER_COUNTRY,
            selected_army_id: app.interaction.selected_army_id.map(|id| id.raw()),
            painter_mode: painter_info,
        });
        app.ui_state.panel_cache.record(
            UiPanelCacheKind::Military,
            cache_started.elapsed(),
            !side_panel_open,
        );
        data
    } else {
        None
    };
    let air_data = if open_panel == Some(InGamePanel::Air) {
        let player_cid = hoi4_state::CountryId(app.view.player_country as u16);
        let air_control = hoi4_logic::air::air_superiority::AirControl::recompute(
            &app.world,
            app.world.data.as_ref(),
        );
        let mut wings = Vec::new();
        for wi in 0..app.world.air_wings.count {
            if app.world.air_wings.owners[wi] != player_cid {
                continue;
            }
            let region = app.world.air_wings.region_id[wi];
            let target = (app.world.air_wings.target_region[wi] != u32::MAX)
                .then_some(app.world.air_wings.target_region[wi]);
            let effective_region = target.unwrap_or(region);
            wings.push(hoi4_ui::air::AirWingEntry {
                id: wi as u32,
                name: app.world.air_wings.names[wi].clone(),
                aircraft_key: app.world.air_wings.aircraft_keys[wi].clone(),
                base_state: app.world.air_wings.base_state[wi],
                region_id: region,
                target_region: target,
                mission: air_mission_to_ui(app.world.air_wings.mission[wi]),
                planes: app.world.air_wings.count_planes[wi],
                max_planes: app.world.air_wings.max_planes[wi],
                organisation: app.world.air_wings.organisation[wi],
                max_organisation: app.world.air_wings.max_organisation[wi],
                range_km: app.world.air_wings.range_km[wi],
                air_control_pct: air_control.control(effective_region, player_cid) * 100.0,
                mission_efficiency_pct: hoi4_logic::air::operations::mission_efficiency(
                    &app.world, wi,
                ) * 100.0,
                transferring: app.world.air_wings.transfer_arrival_hour[wi] != 0,
                reinforce_enabled: app.world.air_wings.reinforce_enabled[wi],
            });
        }
        let total_planes: u32 = wings.iter().map(|wing| wing.planes).sum();
        let active_wings = wings
            .iter()
            .filter(|wing| wing.mission != hoi4_ui::air::AirMissionUi::Idle)
            .count();
        let mut base_usage: std::collections::HashMap<u16, u32> = std::collections::HashMap::new();
        for wing in &wings {
            *base_usage.entry(wing.base_state).or_insert(0) += wing.planes;
        }
        let over_capacity_bases = base_usage
            .into_iter()
            .filter(|(state, planes)| {
                let state = hoi4_state::StateId(*state);
                *planes > hoi4_logic::air::regions::airbase_capacity(&app.world, player_cid, state)
            })
            .count();
        Some(hoi4_ui::air::AirData {
            wings,
            aircraft_stockpile: app
                .runtime
                .econ
                .stockpile
                .get(app.view.player_country)
                .and_then(|s| s.get("aircraft"))
                .copied()
                .unwrap_or(0.0),
            total_planes,
            active_wings,
            over_capacity_bases,
            transfer_source_wing: app.interaction.air_transfer_source_wing,
            pending_transfer_wing: app.interaction.pending_air_transfer_wing,
        })
    } else {
        None
    };

    let naval_data = if open_panel == Some(InGamePanel::Naval) {
        let player_cid = hoi4_state::CountryId(app.view.player_country as u16);
        let mut fleets = Vec::new();
        for fi in 0..app.world.fleets.count {
            if app.world.fleets.owners[fi] != player_cid {
                continue;
            }
            let mut hp = 0.0f32;
            let mut max_hp = 0.0f32;
            let mut damaged = 0usize;
            for &ship in &app.world.fleets.ships[fi] {
                let si = ship.0 as usize;
                if si >= app.world.ships.count {
                    continue;
                }
                hp += app.world.ships.hp[si].max(0.0);
                max_hp += app.world.ships.max_hp[si].max(0.0);
                if app.world.ships.hp[si] + 0.01 < app.world.ships.max_hp[si] {
                    damaged += 1;
                }
            }
            let region = app.world.fleets.region_id[fi];
            let convoy_risk_pct = if region != u32::MAX {
                hoi4_logic::naval::missions::convoy_route_risk(
                    &app.world,
                    app.world.data.as_ref(),
                    player_cid,
                    &[region],
                )
                .loss_rate
                    * 100.0
            } else {
                0.0
            };
            fleets.push(hoi4_ui::naval::FleetEntry {
                id: fi as u32,
                name: app.world.fleets.names[fi].clone(),
                region_id: region,
                target_region_id: (app.world.fleets.target_region_id[fi] != u32::MAX)
                    .then_some(app.world.fleets.target_region_id[fi]),
                mission: naval_mission_to_ui(app.world.fleets.mission[fi]),
                ship_count: app.world.fleets.ships[fi].len(),
                damaged_ships: damaged,
                hp_ratio: if max_hp > 0.0 { hp / max_hp } else { 0.0 },
                convoy_risk_pct,
                repair_state: format!("{:?}", app.world.fleets.repair_state[fi]),
            });
        }
        let stockpile = app.runtime.econ.stockpile.get(app.view.player_country);
        Some(hoi4_ui::naval::NavalData {
            fleets,
            convoys: stockpile
                .and_then(|s| s.get("convoy"))
                .copied()
                .unwrap_or(0.0),
            naval_vessels: stockpile
                .and_then(|s| s.get("naval_vessel"))
                .copied()
                .unwrap_or(0.0),
            transfer_source_fleet: app.interaction.naval_transfer_source_fleet,
            pending_move_fleet: app.interaction.pending_naval_move_fleet,
        })
    } else {
        None
    };

    // J.4b / V6.G7: logistics panel data from V6 military buildings + force needs.
    let logistics_data = if open_panel == Some(InGamePanel::Logistics) {
        hoi4_app::ui_data::logistics::panel_data(
            &app.world,
            &app.v6_db,
            &mut app.runtime.econ,
            player_country,
        )
    } else {
        None
    };

    // J.5b: Situation panel data
    let situation_panel_data = if open_panel == Some(InGamePanel::Situation) {
        let player_cid = hoi4_state::CountryId(player_country as u16);
        let situations: Vec<hoi4_ui::situation_panel::SituationEntry> = app
            .runtime
            .content
            .situation_state
            .active
            .iter()
            .map(|active| {
                let def = app
                    .runtime
                    .content
                    .situation_state
                    .defs
                    .iter()
                    .find(|d| d.id == active.def_id);
                let sides: Vec<hoi4_ui::situation_panel::SituationSideEntry> = active
                    .progress
                    .iter()
                    .enumerate()
                    .map(|(i, &prog)| {
                        let side_def = def.and_then(|d| d.sides.get(i));
                        let name = side_def
                            .map(|s| localized_content_name(&s.id, &s.name))
                            .unwrap_or_default();
                        let color = side_def
                            .map(|s| {
                                hoi4_ui::egui::Color32::from_rgb(s.color[0], s.color[1], s.color[2])
                            })
                            .unwrap_or(hoi4_ui::egui::Color32::GRAY);
                        let supporters: Vec<String> = active
                            .supporters
                            .get(i)
                            .map(|sups| {
                                sups.iter()
                                    .filter_map(|&c| {
                                        app.world.country_tag(c).map(|t| t.to_string())
                                    })
                                    .collect()
                            })
                            .unwrap_or_default();
                        hoi4_ui::situation_panel::SituationSideEntry {
                            name,
                            progress: prog,
                            color,
                            supporters,
                        }
                    })
                    .collect();
                let interventions: Vec<hoi4_ui::situation_panel::InterventionEntry> = def
                    .map(|d| {
                        d.interventions
                            .iter()
                            .map(|interv| {
                                let cd = active
                                    .cooldowns
                                    .get(&(player_cid.0, interv.id.clone()))
                                    .copied()
                                    .unwrap_or(0);
                                let cost_desc = if interv.cost_pp > 0.0 {
                                    format!("?????? {:.0}", interv.cost_pp)
                                } else if interv.cost_manpower > 0 {
                                    format!("??? {}", interv.cost_manpower)
                                } else if !interv.cost_equipment.is_empty() {
                                    format!(
                                        "{} {:.0}",
                                        interv.cost_equipment[0].0, interv.cost_equipment[0].1
                                    )
                                } else {
                                    String::new()
                                };
                                let side_name = d
                                    .sides
                                    .get(interv.side_index)
                                    .map(|s| localized_content_name(&s.id, &s.name))
                                    .unwrap_or_default();
                                hoi4_ui::situation_panel::InterventionEntry {
                                    id: interv.id.clone(),
                                    name: localized_content_name(&interv.id, &interv.name),
                                    cost_desc,
                                    expected_impact: intervention_expected_impact(
                                        interv.progress_boost,
                                        interv.army_xp,
                                        interv.air_xp,
                                        interv.cooldown_days,
                                    ),
                                    available: true,
                                    cooldown_days: cd,
                                    side_name,
                                }
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let intervention_log: Vec<hoi4_ui::situation_panel::InterventionLogEntry> = active
                    .intervention_log
                    .iter()
                    .map(|log| hoi4_ui::situation_panel::InterventionLogEntry {
                        country_tag: log.country_tag.clone(),
                        intervention_name: log.intervention_name.clone(),
                        side_name: log.side_name.clone(),
                        progress_boost: log.progress_boost,
                        army_xp: log.army_xp,
                        air_xp: log.air_xp,
                    })
                    .collect();

                // Military overview is meaningful for territorial-control situations.
                let military_overview = if let Some(d) = def {
                    if let hoi4_content::ProgressSource::TerritorialControl { side_country_tags } =
                        &d.progress_source
                    {
                        let mut div_counts = vec![0u32; d.sides.len()];
                        let mut eq_stocks = vec![0.0f32; d.sides.len()];
                        let mut belligerents: Vec<Vec<String>> =
                            side_country_tags.iter().map(|tags| tags.clone()).collect();
                        // Division counts and equipment stockpiles.
                        for (idx, tags) in side_country_tags.iter().enumerate() {
                            for tag in tags {
                                if let Some(cid) = app.world.country(tag) {
                                    let ci = cid.0 as usize;
                                    for di in 0..app.world.divisions.count {
                                        if app.world.divisions.owners[di] == cid {
                                            div_counts[idx] += 1;
                                        }
                                    }
                                    if ci < app.runtime.econ.stockpile.len() {
                                        eq_stocks[idx] += app.runtime.econ.stockpile[ci]
                                            .get("infantry_equipment")
                                            .copied()
                                            .unwrap_or(0.0);
                                    }
                                }
                            }
                        }
                        // Theater control.
                        let mut theater_control = vec![0u32; d.sides.len()];
                        let theater_total = active.theater_states.len() as u32;
                        for &si_raw in &active.theater_states {
                            let si = si_raw as usize;
                            if si >= app.world.states.count {
                                continue;
                            }
                            let ctrl = app.world.states.controllers[si];
                            if ctrl.is_none() {
                                continue;
                            }
                            for (idx, tags) in side_country_tags.iter().enumerate() {
                                if tags.iter().any(|t| app.world.country(t) == Some(ctrl)) {
                                    theater_control[idx] += 1;
                                    break;
                                }
                            }
                        }
                        // Key provinces: each main belligerent capital state.
                        let mut key_provinces: Vec<(String, String)> = Vec::new();
                        for tags in side_country_tags {
                            for tag in tags {
                                if let Some(cid) = app.world.country(&tag) {
                                    let ci = cid.0 as usize;
                                    if ci < app.world.countries.count {
                                        let cap_state = app.world.countries.capitals[ci];
                                        if !cap_state.is_none() {
                                            let cap_si = cap_state.0 as usize;
                                            if cap_si < app.world.states.count {
                                                let ctrl = app.world.states.controllers[cap_si];
                                                let ctrl_tag = app
                                                    .world
                                                    .country_tag(ctrl)
                                                    .map(|s| s.to_string())
                                                    .unwrap_or_else(|| "-".into());
                                                key_provinces
                                                    .push((format!("{} (capital)", tag), ctrl_tag));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        // Remove countries that no longer exist from belligerent lists.
                        belligerents.iter_mut().for_each(|tags| {
                            tags.retain(|t| app.world.country(t).is_some());
                        });
                        Some(hoi4_ui::situation_panel::MilitaryOverview {
                            division_counts: div_counts,
                            equipment_stockpile: eq_stocks,
                            belligerent_tags: belligerents,
                            theater_control,
                            theater_total,
                            key_provinces,
                        })
                    } else {
                        None
                    }
                } else {
                    None
                };

                hoi4_ui::situation_panel::SituationEntry {
                    id: active.def_id.clone(),
                    title: def
                        .map(|d| localized_content_name(&d.id, &d.title))
                        .unwrap_or_default(),
                    description: def
                        .map(|d| localized_content_name(&format!("desc.{}", d.id), &d.description))
                        .unwrap_or_default(),
                    sides,
                    interventions,
                    ended: active.ended,
                    winner: active.winner.and_then(|i| {
                        def.and_then(|d| d.sides.get(i))
                            .map(|s| localized_content_name(&s.id, &s.name))
                    }),
                    intervention_log,
                    military_overview,
                }
            })
            .collect();
        Some(hoi4_ui::situation_panel::SituationPanelData { situations })
    } else {
        None
    };
    let focus_tree = &app.runtime.content.focus_tree;
    let player_idx = app.view.player_country;
    let player_cid = hoi4_state::CountryId(app.view.player_country as u16);
    let completed_focuses = app.world.countries.completed_focuses[player_idx].clone();
    let current_focus_ref = app.world.countries.current_focus[player_idx].as_deref();
    let current_focus_progress = app.world.countries.focus_progress[player_idx];
    // P0.3?????available focus id ???
    let available_focus_ids: std::collections::HashSet<String> = focus_tree
        .focuses
        .iter()
        .filter(|f| {
            hoi4_content::eval::eval_trigger(
                &f.available,
                &app.world,
                player_cid,
                &app.runtime.content.global_flags,
            )
        })
        .map(|f| f.id.clone())
        .collect();
    let player_in_faction = app
        .world
        .diplomacy
        .faction_of(hoi4_state::CountryId(app.view.player_country as u16))
        .is_some();
    let has_bottom_bar = app.interaction.frontline_painter.mode != PainterMode::Idle
        || app.interaction.selected_army_id.is_some()
        || !app
            .world
            .player_armies
            .iter()
            .filter(|a| a.owner == hoi4_state::CountryId(app.view.player_country as u16))
            .next()
            .is_none()
        || !app.interaction.selected_divisions.is_empty();
    let province_info_bottom_bar_height = if has_bottom_bar { 118.0 } else { 0.0 };
    // CR-4.5: Counter right-click menu
    let counter_rclick_prov = app.interaction.counter_right_click_province;
    let counter_menu_pos = app.last_mouse;
    // V5 G.3 / G.4 / G.5???????disjoint ?????app ?????????????ttings / save_browser /
    let settings_panel_open_cmd = open_panel == Some(InGamePanel::Settings);
    let saves_open_cmd = open_panel == Some(InGamePanel::Saves);
    let law_error_toast = match (
        &app.ui_state.law_error_message,
        &app.ui_state.last_law_error_toast,
    ) {
        (Some(current), Some(last)) if current == last => None,
        (Some(current), _) => Some(current.clone()),
        (None, _) => None,
    };

    UiBuildOutput {
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
        current_focus_id: current_focus_ref.map(str::to_owned),
        current_focus_progress,
        available_focus_ids,
        player_in_faction,
        province_info_bottom_bar_height,
        counter_rclick_prov,
        counter_menu_pos,
        settings_panel_open_cmd,
        saves_open_cmd,
        law_error_toast,
    }
}

fn is_politics_national_spirit_category(category: &str) -> bool {
    matches!(category, "country" | "national_spirit")
}

fn normalize_politics_party_popularity(
    pop_map: std::collections::HashMap<String, f32>,
    ruling_party: &str,
) -> Vec<(String, f32)> {
    const IDEOLOGIES: [&str; 4] = ["fascism", "democratic", "communism", "neutrality"];
    let mut totals = std::collections::HashMap::<&'static str, f32>::new();
    for (key, value) in pop_map {
        let Some(canonical) = canonical_politics_ideology_key(&key) else {
            continue;
        };
        *totals.entry(canonical).or_insert(0.0) += value.max(0.0);
    }
    if totals.values().all(|value| *value <= f32::EPSILON) {
        if let Some(canonical) = canonical_politics_ideology_key(ruling_party) {
            totals.insert(canonical, 1.0);
        }
    }
    IDEOLOGIES
        .into_iter()
        .map(|key| (key.to_owned(), totals.get(key).copied().unwrap_or(0.0)))
        .collect()
}

fn canonical_politics_ideology_key(key: &str) -> Option<&'static str> {
    match key.trim().to_ascii_lowercase().replace('-', "_").as_str() {
        "fascism" | "fascist" => Some("fascism"),
        "democratic" | "democracy" | "democrat" => Some("democratic"),
        "communism" | "communist" => Some("communism"),
        "neutrality" | "neutral" | "non_aligned" | "nonaligned" => Some("neutrality"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{is_politics_national_spirit_category, normalize_politics_party_popularity};
    use std::collections::HashMap;

    #[test]
    fn politics_national_spirits_exclude_law_and_advisor_idea_categories() {
        assert!(is_politics_national_spirit_category("country"));
        assert!(is_politics_national_spirit_category("national_spirit"));

        for category in [
            "trade_laws",
            "economy",
            "mobilization_laws",
            "political_advisor",
            "theorist",
            "army_chief",
            "navy_chief",
            "air_chief",
            "high_command",
        ] {
            assert!(
                !is_politics_national_spirit_category(category),
                "{category} should render in law/advisor slots, not national spirits"
            );
        }
    }

    #[test]
    fn politics_party_popularity_normalizes_to_vanilla_ideologies() {
        let pops = normalize_politics_party_popularity(
            HashMap::from([
                ("fascist".to_owned(), 0.65),
                ("democracy".to_owned(), 0.25),
                ("custom".to_owned(), 0.10),
            ]),
            "fascism",
        );

        assert_eq!(
            pops,
            vec![
                ("fascism".to_owned(), 0.65),
                ("democratic".to_owned(), 0.25),
                ("communism".to_owned(), 0.0),
                ("neutrality".to_owned(), 0.0),
            ]
        );
    }

    #[test]
    fn empty_party_popularity_falls_back_to_ruling_party() {
        let pops = normalize_politics_party_popularity(HashMap::new(), "neutrality");

        assert_eq!(
            pops,
            vec![
                ("fascism".to_owned(), 0.0),
                ("democratic".to_owned(), 0.0),
                ("communism".to_owned(), 0.0),
                ("neutrality".to_owned(), 1.0),
            ]
        );
    }
}
