use egui_kittest::Harness;
use hoi4_paths::PathConfig;
use hoi4_state::LawCategory;
use hoi4_ui::detail_panel::DetailPanelHost;
use hoi4_ui::egui::{Pos2, Vec2};
use hoi4_ui::frame_model::ActiveDetailPanel;
use hoi4_ui::icons::IconBank;
use hoi4_ui::law_panel::{LawPanel, LawPanelData, LawSlotEntry, LawTierEntry};
use hoi4_ui::politics::{
    GovernmentPostEntry, IdeaEntry, PoliticsData, PoliticsLawEntry, PoliticsPanel,
};
use hoi4_ui::theme::apply_vanilla_theme;

fn snapshot_if_enabled<State>(harness: &mut Harness<'_, State>, name: &str) {
    let enabled = std::env::var_os("RUN_POLITICS_GATE0_SNAPSHOT").is_some()
        || std::env::var_os("UPDATE_SNAPSHOTS").is_some();
    if enabled {
        harness.snapshot(name);
    }
}

fn path_config() -> PathConfig {
    PathConfig::resolve(Default::default()).unwrap_or_else(|_| {
        let fallback = std::env::current_dir().unwrap_or_else(|_| std::env::temp_dir());
        PathConfig::with_game_path(fallback)
    })
}

fn gate0_politics_data() -> PoliticsData {
    PoliticsData {
        ruling_party: "fascism".to_owned(),
        party_popularity: vec![
            ("fascism".to_owned(), 0.71),
            ("neutrality".to_owned(), 0.18),
            ("democratic".to_owned(), 0.08),
            ("communism".to_owned(), 0.03),
        ],
        ideas: vec![
            IdeaEntry {
                key: "autarky".to_owned(),
                name: "Autarky".to_owned(),
                category: "country".to_owned(),
                picture: Some("GFX_idea_generic_industry".to_owned()),
                modifiers: vec![("Factory Output".to_owned(), 0.05)],
            },
            IdeaEntry {
                key: "four_year_plan".to_owned(),
                name: "Four Year Plan".to_owned(),
                category: "country".to_owned(),
                picture: Some("GFX_idea_generic_production".to_owned()),
                modifiers: vec![("Construction Speed".to_owned(), 0.1)],
            },
        ],
        political_power: 148.0,
        stability: 0.67,
        war_support: 0.42,
        focus_available: true,
        current_focus_name: Some("Rhineland".to_owned()),
        current_focus_progress: 0.35,
        current_focus_cost_days: Some(70),
        country_tag: "GER".to_owned(),
        leader_name: "Adolf Hitler".to_owned(),
        leader_portrait_key: Some("GFX_portrait_GER_adolf_hitler".to_owned()),
        party_full_name: "Nationalsozialistische Deutsche Arbeiterpartei".to_owned(),
        party_names: vec![
            ("fascism".to_owned(), "NSDAP".to_owned()),
            ("neutrality".to_owned(), "DNVP".to_owned()),
            ("democratic".to_owned(), "Zentrum".to_owned()),
            ("communism".to_owned(), "KPD".to_owned()),
        ],
        government_posts: vec![
            GovernmentPostEntry {
                office: "Head of Government".to_owned(),
                name: "Rudolf Hess".to_owned(),
                detail: "Party administration".to_owned(),
            },
            GovernmentPostEntry {
                office: "Foreign Minister".to_owned(),
                name: "Konstantin von Neurath".to_owned(),
                detail: "Diplomatic pressure".to_owned(),
            },
        ],
        law_slots: vec![
            PoliticsLawEntry {
                category: LawCategory::Conscription,
                current_id: "volunteer_only".to_owned(),
                current_name: "Volunteer Only".to_owned(),
                cooldown_days: 0,
                pending: None,
                is_locked: false,
            },
            PoliticsLawEntry {
                category: LawCategory::Economy,
                current_id: "laissez_faire".to_owned(),
                current_name: "Laissez-faire".to_owned(),
                cooldown_days: 0,
                pending: Some(("Partial Mobilization".to_owned(), 24)),
                is_locked: false,
            },
            PoliticsLawEntry {
                category: LawCategory::Trade,
                current_id: "free_trade".to_owned(),
                current_name: "Free Trade".to_owned(),
                cooldown_days: 12,
                pending: None,
                is_locked: false,
            },
            PoliticsLawEntry {
                category: LawCategory::Taxation,
                current_id: "medium_taxation".to_owned(),
                current_name: "Medium Taxation".to_owned(),
                cooldown_days: 0,
                pending: None,
                is_locked: false,
            },
            PoliticsLawEntry {
                category: LawCategory::CivilRights,
                current_id: "limited_rights".to_owned(),
                current_name: "Limited Rights".to_owned(),
                cooldown_days: 0,
                pending: None,
                is_locked: true,
            },
            PoliticsLawEntry {
                category: LawCategory::InformationControl,
                current_id: "regulated_press".to_owned(),
                current_name: "Regulated Press".to_owned(),
                cooldown_days: 0,
                pending: None,
                is_locked: false,
            },
        ],
    }
}

fn gate11_law_panel_data() -> LawPanelData {
    LawPanelData {
        political_power: 148.0,
        slots: vec![LawSlotEntry {
            category: LawCategory::Economy,
            current_id: "civilian_economy".to_owned(),
            current_name: "Civilian Economy".to_owned(),
            cooldown_days: 0,
            pending: Some((
                "partial_mobilization".to_owned(),
                "Partial Mobilization".to_owned(),
                24,
            )),
            is_locked: false,
            locked_reason: None,
            previous_before_lock: None,
            tiers: vec![
                LawTierEntry {
                    id: "civilian_economy".to_owned(),
                    name: "Civilian Economy".to_owned(),
                    pp_cost: 50,
                    cooldown_days: 60,
                    effects: vec!["Factory output baseline".to_owned()],
                },
                LawTierEntry {
                    id: "partial_mobilization".to_owned(),
                    name: "Partial Mobilization".to_owned(),
                    pp_cost: 150,
                    cooldown_days: 90,
                    effects: vec![
                        "Military factory construction speed +10%".to_owned(),
                        "Consumer goods factories -5%".to_owned(),
                    ],
                },
                LawTierEntry {
                    id: "war_economy".to_owned(),
                    name: "War Economy".to_owned(),
                    pp_cost: 250,
                    cooldown_days: 120,
                    effects: vec![
                        "Military factory construction speed +20%".to_owned(),
                        "Dockyard output +10%".to_owned(),
                    ],
                },
            ],
        }],
    }
}

fn phase3_law_panel_data() -> LawPanelData {
    let mk_tiers = |current: &str| {
        vec![
            LawTierEntry {
                id: current.to_owned(),
                name: current.replace('_', " ").to_owned(),
                pp_cost: 0,
                cooldown_days: 0,
                effects: vec!["Current legal framework remains active.".to_owned()],
            },
            LawTierEntry {
                id: "war_economy".to_owned(),
                name: "War Economy".to_owned(),
                pp_cost: 250,
                cooldown_days: 120,
                effects: vec![
                    "Military factory construction speed +20%".to_owned(),
                    "Dockyard output +10%".to_owned(),
                ],
            },
            LawTierEntry {
                id: "partial_mobilization".to_owned(),
                name: "Partial Mobilization".to_owned(),
                pp_cost: 150,
                cooldown_days: 90,
                effects: vec![
                    "Military factory construction speed +10%".to_owned(),
                    "Consumer goods factories -5%".to_owned(),
                ],
            },
        ]
    };

    LawPanelData {
        political_power: 125.0,
        slots: vec![
            LawSlotEntry {
                category: LawCategory::Conscription,
                current_id: "volunteer_only".to_owned(),
                current_name: "Volunteer Only".to_owned(),
                cooldown_days: 0,
                pending: Some((
                    "partial_mobilization".to_owned(),
                    "Partial Mobilization".to_owned(),
                    18,
                )),
                is_locked: false,
                locked_reason: None,
                previous_before_lock: None,
                tiers: mk_tiers("volunteer_only"),
            },
            LawSlotEntry {
                category: LawCategory::Economy,
                current_id: "civilian_economy".to_owned(),
                current_name: "Civilian Economy".to_owned(),
                cooldown_days: 0,
                pending: None,
                is_locked: false,
                locked_reason: None,
                previous_before_lock: None,
                tiers: mk_tiers("civilian_economy"),
            },
            LawSlotEntry {
                category: LawCategory::Trade,
                current_id: "free_trade".to_owned(),
                current_name: "Free Trade".to_owned(),
                cooldown_days: 12,
                pending: None,
                is_locked: false,
                locked_reason: None,
                previous_before_lock: None,
                tiers: mk_tiers("free_trade"),
            },
            LawSlotEntry {
                category: LawCategory::Taxation,
                current_id: "medium_taxation".to_owned(),
                current_name: "Medium Taxation".to_owned(),
                cooldown_days: 0,
                pending: None,
                is_locked: false,
                locked_reason: None,
                previous_before_lock: None,
                tiers: mk_tiers("medium_taxation"),
            },
            LawSlotEntry {
                category: LawCategory::CivilRights,
                current_id: "limited_rights".to_owned(),
                current_name: "Limited Rights".to_owned(),
                cooldown_days: 0,
                pending: None,
                is_locked: true,
                locked_reason: Some("Locked by political reform requirements.".to_owned()),
                previous_before_lock: None,
                tiers: mk_tiers("limited_rights"),
            },
            LawSlotEntry {
                category: LawCategory::InformationControl,
                current_id: "regulated_press".to_owned(),
                current_name: "Regulated Press".to_owned(),
                cooldown_days: 0,
                pending: None,
                is_locked: false,
                locked_reason: None,
                previous_before_lock: None,
                tiers: mk_tiers("regulated_press"),
            },
        ],
    }
}

fn phase4_law_panel_tooltip_data() -> LawPanelData {
    LawPanelData {
        political_power: 40.0,
        slots: vec![LawSlotEntry {
            category: LawCategory::Economy,
            current_id: "civilian_economy".to_owned(),
            current_name: "Civilian Economy".to_owned(),
            cooldown_days: 7,
            pending: Some((
                "partial_mobilization".to_owned(),
                "Partial Mobilization".to_owned(),
                19,
            )),
            is_locked: true,
            locked_reason: Some("Locked by political reform requirements.".to_owned()),
            previous_before_lock: None,
            tiers: vec![
                LawTierEntry {
                    id: "civilian_economy".to_owned(),
                    name: "Civilian Economy".to_owned(),
                    pp_cost: 0,
                    cooldown_days: 0,
                    effects: vec!["Current legal framework remains active.".to_owned()],
                },
                LawTierEntry {
                    id: "partial_mobilization".to_owned(),
                    name: "Partial Mobilization".to_owned(),
                    pp_cost: 150,
                    cooldown_days: 90,
                    effects: vec![
                        "Military factory construction speed +10%".to_owned(),
                        "Consumer goods factories -5%".to_owned(),
                        "Dockyard output +10%".to_owned(),
                    ],
                },
            ],
        }],
    }
}

fn phase6_law_panel_state_matrix_data() -> LawPanelData {
    let mk_tiers = |current: &str| {
        vec![
            LawTierEntry {
                id: current.to_owned(),
                name: current.replace('_', " ").to_owned(),
                pp_cost: 0,
                cooldown_days: 0,
                effects: vec!["Current legal framework remains active.".to_owned()],
            },
            LawTierEntry {
                id: "war_economy".to_owned(),
                name: "War Economy".to_owned(),
                pp_cost: 250,
                cooldown_days: 120,
                effects: vec![
                    "Military factory construction speed +20%".to_owned(),
                    "Dockyard output +10%".to_owned(),
                ],
            },
            LawTierEntry {
                id: "partial_mobilization".to_owned(),
                name: "Partial Mobilization".to_owned(),
                pp_cost: 150,
                cooldown_days: 90,
                effects: vec![
                    "Military factory construction speed +10%".to_owned(),
                    "Consumer goods factories -5%".to_owned(),
                ],
            },
        ]
    };

    LawPanelData {
        political_power: 175.0,
        slots: vec![
            LawSlotEntry {
                category: LawCategory::Economy,
                current_id: "civilian_economy".to_owned(),
                current_name: "Civilian Economy".to_owned(),
                cooldown_days: 0,
                pending: None,
                is_locked: false,
                locked_reason: None,
                previous_before_lock: None,
                tiers: mk_tiers("civilian_economy"),
            },
            LawSlotEntry {
                category: LawCategory::Conscription,
                current_id: "volunteer_only".to_owned(),
                current_name: "Volunteer Only".to_owned(),
                cooldown_days: 0,
                pending: Some((
                    "partial_mobilization".to_owned(),
                    "Partial Mobilization".to_owned(),
                    18,
                )),
                is_locked: false,
                locked_reason: None,
                previous_before_lock: None,
                tiers: mk_tiers("volunteer_only"),
            },
            LawSlotEntry {
                category: LawCategory::Trade,
                current_id: "free_trade".to_owned(),
                current_name: "Free Trade".to_owned(),
                cooldown_days: 12,
                pending: None,
                is_locked: false,
                locked_reason: None,
                previous_before_lock: None,
                tiers: mk_tiers("free_trade"),
            },
            LawSlotEntry {
                category: LawCategory::Taxation,
                current_id: "medium_taxation".to_owned(),
                current_name: "Medium Taxation".to_owned(),
                cooldown_days: 0,
                pending: None,
                is_locked: false,
                locked_reason: None,
                previous_before_lock: None,
                tiers: mk_tiers("medium_taxation"),
            },
            LawSlotEntry {
                category: LawCategory::CivilRights,
                current_id: "limited_rights".to_owned(),
                current_name: "Limited Rights".to_owned(),
                cooldown_days: 0,
                pending: None,
                is_locked: true,
                locked_reason: Some("Locked by political reform requirements.".to_owned()),
                previous_before_lock: None,
                tiers: mk_tiers("limited_rights"),
            },
            LawSlotEntry {
                category: LawCategory::InformationControl,
                current_id: "regulated_press".to_owned(),
                current_name: "Regulated Press".to_owned(),
                cooldown_days: 0,
                pending: None,
                is_locked: false,
                locked_reason: None,
                previous_before_lock: None,
                tiers: mk_tiers("regulated_press"),
            },
        ],
    }
}

#[test]
fn politics_gate0_project_baseline_snapshot() {
    let path_cfg = path_config();
    let data = gate0_politics_data();
    let mut harness = Harness::builder()
        .with_size(Vec2::new(1280.0, 720.0))
        .build(move |ctx| {
            let _ = apply_vanilla_theme(ctx);
            let mut icon_bank = IconBank::new(ctx.clone(), path_cfg.clone());
            icon_bank.add_search_dir("gfx/interface");
            icon_bank.add_search_dir("gfx/interface/ideas");
            icon_bank.add_leader_dirs(["GER"]);
            let _ = PoliticsPanel::show(ctx, &data, &mut icon_bank);
        });

    harness.run();
    snapshot_if_enabled(&mut harness, "politics_gate0_project_baseline");
}

#[test]
fn politics_gate11_law_detail_window_snapshot() {
    let data = gate11_law_panel_data();
    let detail = ActiveDetailPanel::Law {
        category: "Economy".to_owned(),
        law_id: None,
    };
    let mut harness = Harness::builder()
        .with_size(Vec2::new(1280.0, 720.0))
        .build(move |ctx| {
            let _ = apply_vanilla_theme(ctx);
            let _ = DetailPanelHost::show(
                ctx,
                Some(&detail),
                None,
                None,
                Some(&data),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                0.0,
                None,
                None,
            );
        });

    harness.run();
    snapshot_if_enabled(&mut harness, "politics_gate11_law_detail_window");
}

#[test]
fn law_panel_phase3_visual_snapshot() {
    let data = phase3_law_panel_data();
    let mut harness = Harness::builder()
        .with_size(Vec2::new(1280.0, 720.0))
        .build(move |ctx| {
            let _ = apply_vanilla_theme(ctx);
            let _ = LawPanel::show(ctx, &data);
        });

    harness.run();
    snapshot_if_enabled(&mut harness, "law_panel_phase3_visual");
}

#[test]
fn law_panel_phase3_small_window_snapshot() {
    let data = phase3_law_panel_data();
    let mut harness = Harness::builder()
        .with_size(Vec2::new(960.0, 640.0))
        .build(move |ctx| {
            let _ = apply_vanilla_theme(ctx);
            let _ = LawPanel::show(ctx, &data);
        });

    harness.run();
    snapshot_if_enabled(&mut harness, "law_panel_phase3_small_window");
}

#[test]
fn law_panel_phase4_tooltip_state_snapshot() {
    let data = phase4_law_panel_tooltip_data();
    let mut harness = Harness::builder()
        .with_size(Vec2::new(1280.0, 720.0))
        .build(move |ctx| {
            let _ = apply_vanilla_theme(ctx);
            ctx.style_mut(|style| {
                style.interaction.tooltip_delay = 0.0;
            });
            let _ = LawPanel::show(ctx, &data);
        });

    harness
        .input_mut()
        .events
        .push(hoi4_ui::egui::Event::PointerMoved(Pos2::new(620.0, 455.0)));
    harness.run_steps(2);
    snapshot_if_enabled(&mut harness, "law_panel_phase4_tooltip_state");
}

#[test]
fn law_panel_phase6_state_matrix_1080p_snapshot() {
    let data = phase6_law_panel_state_matrix_data();
    let mut harness = Harness::builder()
        .with_size(Vec2::new(1920.0, 1080.0))
        .build(move |ctx| {
            let _ = apply_vanilla_theme(ctx);
            let _ = LawPanel::show(ctx, &data);
        });

    harness.run();
    snapshot_if_enabled(&mut harness, "law_panel_phase6_state_matrix_1080p");
}

#[test]
fn law_panel_phase6_state_matrix_small_window_snapshot() {
    let data = phase6_law_panel_state_matrix_data();
    let mut harness = Harness::builder()
        .with_size(Vec2::new(960.0, 640.0))
        .build(move |ctx| {
            let _ = apply_vanilla_theme(ctx);
            let _ = LawPanel::show(ctx, &data);
        });

    harness.run();
    snapshot_if_enabled(&mut harness, "law_panel_phase6_state_matrix_small_window");
}

#[test]
fn politics_gate13_viewport_smoke_1080p_1440p_and_small() {
    for (size, snapshot_name) in [
        (Vec2::new(1920.0, 1080.0), "politics_gate9_1936_1080p"),
        (Vec2::new(2560.0, 1440.0), "politics_gate9_1936_1440p"),
        (Vec2::new(960.0, 640.0), "politics_gate9_1936_small_window"),
    ] {
        let path_cfg = path_config();
        let data = gate0_politics_data();
        let mut harness = Harness::builder().with_size(size).build(move |ctx| {
            let _ = apply_vanilla_theme(ctx);
            let mut icon_bank = IconBank::new(ctx.clone(), path_cfg.clone());
            icon_bank.add_search_dir("gfx/interface");
            icon_bank.add_search_dir("gfx/interface/ideas");
            icon_bank.add_leader_dirs(["GER"]);
            let _ = PoliticsPanel::show(ctx, &data, &mut icon_bank);
        });
        harness.run();
        snapshot_if_enabled(&mut harness, snapshot_name);

        let data = gate11_law_panel_data();
        let detail = ActiveDetailPanel::Law {
            category: "Economy".to_owned(),
            law_id: None,
        };
        let mut harness = Harness::builder().with_size(size).build(move |ctx| {
            let _ = apply_vanilla_theme(ctx);
            let _ = DetailPanelHost::show(
                ctx,
                Some(&detail),
                None,
                None,
                Some(&data),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                0.0,
                None,
                None,
            );
        });
        harness.run();
    }
}
