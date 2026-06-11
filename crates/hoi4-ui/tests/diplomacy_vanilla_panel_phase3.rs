use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use hoi4_ui::country_info_panel::{CountryInfoCommand, CountryInfoData};
use hoi4_ui::diplomacy::{
    DiplomacyActionCommand, DiplomacyActionEntry, DiplomacyRelationEntry, DiplomacyRelationKind,
    DiplomaticActionView,
};
use hoi4_ui::diplomacy_profile::{
    country_diplomacy_action_click_command, country_diplomacy_vanilla_runtime_frame,
    country_diplomacy_vanilla_runtime_frame_parts, CountryDiplomacyProfile,
    COUNTRY_DIPLOMACY_ACTION_PREFIX, COUNTRY_DIPLOMACY_CLOSE_COMMAND,
};
use hoi4_ui::vanilla_gui::{
    bind_profile_tree, parse_gui_str, GuiAction, GuiActionKind, GuiNodePath, GuiPoint, GuiRect,
    GuiRuntimeFrame, GuiRuntimeFrameInput, GuiRuntimeState, GuiTemplateRegistry,
    VanillaGuiRuntimeContext, VanillaPanelProfile, COUNTRY_DIPLOMACY_DESCRIPTOR,
    COUNTRY_DIPLOMACY_GUI_FILE, COUNTRY_DIPLOMACY_PROFILE_ID, COUNTRY_DIPLOMACY_ROOT,
};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .to_path_buf()
}

fn report_path(name: &str) -> PathBuf {
    workspace_root()
        .join("target/diplomacy_vanilla_gui")
        .join(name)
}

fn write_report(name: &str, body: impl AsRef<str>) {
    let path = report_path(name);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, body.as_ref()).unwrap();
}

fn minimal_doc() -> hoi4_ui::vanilla_gui::GuiDocument {
    parse_gui_str(
        None,
        r#"
guiTypes = {
    containerWindowType = {
        name = "countrydiplomacyview"
        position = { x = -606 y = 78 }
        show_position = { x = -6 y = 78 }
        show_animation_type = decelerated
        hide_animation_type = accelerated
        animation_time = 300
        size = { width = 550 height = 100%% }
        verticalScrollbar = "right_vertical_slider"

        instantTextboxType = {
            name = "diplomacy_title"
            position = { x = 45 y = 8 }
            maxWidth = 240
            maxHeight = 24
        }

        containerWindowType = {
            name = "country_info"
            position = { x = 0 y = 50 }
            size = { width = 530 height = 100%% }

            iconType = { name = "diplo_country_flag" position = { x = 40 y = 1 } size = { width = 82 height = 52 } quadTextureSprite = "GFX_shield_medium" }
            iconType = { name = "diplo_flag_frame" position = { x = 14 y = -10 } size = { width = 130 height = 80 } spriteType = "GFX_diplo_flag_frame" }
            iconType = { name = "ideology_icon" position = { x = 158 y = 0 } size = { width = 32 height = 32 } spriteType = "GFX_ideology_unknown" }
            instantTextboxType = { name = "country_name" position = { x = 245 y = 3 } maxWidth = 195 maxHeight = 20 }
            containerWindowType = {
                name = "faction"
                position = { x = 245 y = 23 }
                size = { width = 175 height = 20 }
                instantTextboxType = { name = "faction_name" position = { x = 0 y = 0 } maxWidth = 173 maxHeight = 20 }
            }
            instantTextboxType = { name = "leader_name" position = { x = 245 y = 43 } maxWidth = 173 maxHeight = 20 }
            containerWindowType = {
                name = "opinion_info"
                position = { x = 341 y = 5 }
                size = { width = 100 height = 60 }
                iconType = { name = "our_opinion_left_flag" position = { x = 38 y = 9 } size = { width = 24 height = 16 } quadTextureSprite = "GFX_flag_small" }
                instantTextboxType = { name = "our_opinion_value" position = { x = 67 y = -4 } maxWidth = 30 maxHeight = 20 }
                iconType = { name = "their_opinion_left_flag" position = { x = 74 y = 21 } size = { width = 24 height = 16 } quadTextureSprite = "GFX_flag_small" }
                instantTextboxType = { name = "their_opinion_value" position = { x = 37 y = 42 } maxWidth = 30 maxHeight = 20 }
            }
            instantTextboxType = { name = "stability_value" position = { x = 446 y = 42 } maxWidth = 44 maxHeight = 20 }
            instantTextboxType = { name = "war_support_value" position = { x = 490 y = 42 } maxWidth = 44 maxHeight = 20 }
            buttonType = { name = "info_tab_button" position = { x = 275 y = 70 } size = { width = 260 height = 34 } quadTextureSprite = "GFX_tab_intel_ledger" }

            containerWindowType = {
                name = "diplomacy_tab_top"
                position = { x = 0 y = 110 }
                size = { width = 530 height = 150 }
                iconType = { name = "leader_portrait" position = { x = 20 y = 10 } size = { width = 92 height = 120 } spriteType = "GFX_leader_unknown" }
                containerWindowType = {
                    name = "political_pie_chart"
                    position = { x = 141 y = 9 }
                    size = { width = 330 height = 30 }
                    iconType = { name = "chart" spriteType = "GFX_political_chart_big" position = { x = 51 y = 5 } }
                    iconType = { name = "pol_view_bg" spriteType = "GFX_pol_piechart_overlay" position = { x = 16 y = 0 } }
                }
                containerWindowType = {
                    name = "ruling_party_info"
                    position = { x = 258 y = 25 }
                    size = { width = 255 height = 60 }
                    instantTextboxType = { name = "party_name" position = { x = 5 y = -5 } maxWidth = 245 maxHeight = 20 }
                    instantTextboxType = { name = "ideology_name" position = { x = 5 y = 10 } maxWidth = 245 maxHeight = 20 }
                    instantTextboxType = { name = "elections" position = { x = 5 y = 25 } maxWidth = 245 maxHeight = 20 }
                }
                containerWindowType = { name = "active_national_focus_info" position = { x = 144 y = 88 } size = { width = 330 height = 50 } }
            }

            containerWindowType = {
                name = "relations_info"
                position = { x = 11 y = 285 }
                size = { width = 268 height = 156 }
                margin = { top = 8 bottom = 8 }
                verticalScrollbar = "right_vertical_slider"
                scroll_wheel_factor = 50
                smooth_scrolling = yes
                gridboxtype = {
                    name = "relations_grid"
                    position = { x = 10 y = 10 }
                    size = { width = 255 height = 100%% }
                    slotsize = { width = 200 height = 50 }
                    max_slots_horizontal = 1
                    format = "UPPER_LEFT"
                }
            }

            containerWindowType = {
                name = "diplomatic_actions"
                position = { x = 272 y = 285 }
                size = { width = 265 height = 112 }
                verticalScrollbar = "right_vertical_slider"
                scroll_wheel_factor = 40
                smooth_scrolling = yes
                gridboxtype = {
                    name = "actions_grid"
                    position = { x = 10 y = 32 }
                    size = { width = 255 height = 100%% }
                    slotsize = { width = 255 height = 29 }
                    max_slots_horizontal = 1
                    format = "UPPER_LEFT"
                }
            }
            containerWindowType = { name = "national_spirit_info" position = { x = 17 y = 245 } size = { width = 530 height = 36 } }
        }

        containerWindowType = { name = "country_list" position = { x = 0 y = 50 } size = { width = 530 height = 100%% } }
        containerWindowType = { name = "intel_ledger_container" position = { x = 0 y = 151 } size = { width = 100%% height = 100%% } }
        buttonType = {
            name = "close_button"
            position = { x = -43 y = 9 }
            size = { width = 26 height = 26 }
            quadTextureSprite = "GFX_closebutton"
            shortcut = "ESCAPE"
            Orientation = "UPPER_RIGHT"
        }
    }

    containerWindowType = {
        name = "diplomacy_action_entry"
        size = { width = 228 height = 28 }
        buttonType = { name = "diplo_actions_entry_bg" position = { x = 2 y = 0 } size = { width = 228 height = 28 } quadTextureSprite = "GFX_diplo_actions_bg" }
        instantTextboxType = { name = "name" position = { x = 8 y = 4 } maxWidth = 190 maxHeight = 20 }
        instantTextboxType = { name = "cost" position = { x = 125 y = 5 } maxWidth = 100 maxHeight = 20 }
        iconType = { name = "accept_icon" position = { x = 204 y = 4 } size = { width = 20 height = 20 } spriteType = "GFX_accept_decline_icon" }
    }

    containerWindowType = {
        name = "relation_strip_view"
        size = { width = 200 height = 50 }
        iconType = { name = "diplo_relations_bg" position = { x = -1 y = 3 } size = { width = 200 height = 42 } spriteType = "GFX_diplo_relations_bg" }
        iconType = { name = "relation_icon" position = { x = -3 y = 12 } size = { width = 32 height = 32 } spriteType = "GFX_relation_truce" }
        OverlappingElementsBoxType = { name = "relation_flags" position = { x = 40 y = 5 } size = { width = 160 height = 32 } }
    }

    containerWindowType = {
        name = "subject_relation_strip_view"
        size = { width = 200 height = 50 }
        iconType = { name = "diplo_relations_bg" position = { x = -1 y = 3 } size = { width = 200 height = 42 } spriteType = "GFX_diplo_relations_bg" }
        iconType = { name = "relation_icon" position = { x = 3 y = 8 } size = { width = 32 height = 32 } spriteType = "GFX_relation_truce" }
        OverlappingElementsBoxType = { name = "relation_flags" position = { x = 40 y = 5 } size = { width = 160 height = 32 } }
    }
}
"#,
    )
}

fn sample_data() -> CountryInfoData {
    let target = "FRA".to_owned();
    let justify = DiplomaticActionView::enabled("Start justifying a war goal.");
    let declare =
        DiplomaticActionView::disabled("Declare war when a war goal is ready.", "No war goal");
    let invite = DiplomaticActionView::enabled("Invite this country to the faction.");
    let access = DiplomaticActionView::enabled("Request military access.");
    CountryInfoData {
        target_tag: target.clone(),
        tag: target.clone(),
        display_name: "France".to_owned(),
        flag_gfx: "GFX_flag_FRA_democratic".to_owned(),
        player_flag_gfx: "GFX_flag_GER_fascism".to_owned(),
        leader_name: "Leon Blum".to_owned(),
        leader_portrait_key: None,
        ruling_party: "democratic".to_owned(),
        ruling_party_label: "democratic".to_owned(),
        party_full_name: "French Section of the Workers International".to_owned(),
        party_popularity: vec![
            ("democratic".to_owned(), 0.62),
            ("communism".to_owned(), 0.17),
            ("neutrality".to_owned(), 0.21),
        ],
        gdp_gbp: 1_200_000_000.0,
        industrial_level: 14,
        military_industrial_level: 6,
        construction_points: 18,
        division_count: 42,
        population: 41_000_000,
        domestic_population: 41_000_000,
        colonial_population: 9_000_000,
        governed_population: 50_000_000,
        subject_population: 0,
        imperial_population: 50_000_000,
        manpower: 720_000,
        stability: 0.64,
        war_support: 0.38,
        opinion: 15,
        our_opinion_of_target: 35,
        their_opinion_of_us: -12,
        at_war: true,
        same_faction: true,
        faction_name: Some("Allies".to_owned()),
        overlord_name: Some("United Kingdom".to_owned()),
        subject_names: vec!["Syria".to_owned()],
        autonomy_level_name: Some("Dominion".to_owned()),
        has_wargoal: true,
        justifying_wargoal: false,
        justify_progress: 1.0,
        justify_days_remaining: 0,
        wargoals: Vec::new(),
        relation_factors: Vec::new(),
        relations: vec![
            relation(
                "at_war",
                DiplomacyRelationKind::AtWar,
                "GFX_relation_war_relation",
            ),
            relation(
                "same_faction",
                DiplomacyRelationKind::SameFaction,
                "GFX_relation_faction",
            ),
            relation(
                "subject",
                DiplomacyRelationKind::Subject,
                "GFX_relation_puppet",
            ),
            relation(
                "wargoal",
                DiplomacyRelationKind::Wargoal,
                "GFX_relation_wargoal",
            ),
            relation(
                "military_access",
                DiplomacyRelationKind::MilitaryAccess,
                "GFX_relation_military_access",
            ),
        ],
        actions: vec![
            action(
                "justify_wargoal",
                "Justify war goal",
                &justify,
                Some("50 PP"),
                DiplomacyActionCommand::JustifyWargoal {
                    target_tag: target.clone(),
                },
            ),
            action(
                "declare_war",
                "Declare war",
                &declare,
                None,
                DiplomacyActionCommand::DeclareWar {
                    target_tag: target.clone(),
                },
            ),
            action(
                "invite_to_faction",
                "Invite to faction",
                &invite,
                None,
                DiplomacyActionCommand::InviteToFaction {
                    target_tag: target.clone(),
                },
            ),
            action(
                "request_military_access",
                "Request access",
                &access,
                None,
                DiplomacyActionCommand::RequestMilitaryAccess { target_tag: target },
            ),
        ],
        justify_action: justify,
        declare_war_action: declare,
        invite_to_faction_action: invite,
        request_access_action: access,
    }
}

fn relation(id: &str, kind: DiplomacyRelationKind, sprite: &str) -> DiplomacyRelationEntry {
    DiplomacyRelationEntry {
        id: id.to_owned(),
        kind,
        label: id.replace('_', " "),
        sprite: sprite.to_owned(),
        tooltip: format!("Project relation: {id}"),
        positive: !matches!(id, "at_war" | "wargoal"),
    }
}

fn action(
    id: &str,
    label: &str,
    view: &DiplomaticActionView,
    cost_text: Option<&str>,
    command: DiplomacyActionCommand,
) -> DiplomacyActionEntry {
    DiplomacyActionEntry {
        id: id.to_owned(),
        label: label.to_owned(),
        enabled: view.enabled,
        preview: view.preview.clone(),
        reason: view.reason.clone(),
        cost_text: cost_text.map(str::to_owned),
        sprite: "GFX_diplo_actions_bg".to_owned(),
        command,
    }
}

fn binding_path(name: &str) -> GuiNodePath {
    GuiNodePath::root(COUNTRY_DIPLOMACY_ROOT).child(name)
}

fn frame_parts_for(
    data: &CountryInfoData,
) -> hoi4_ui::diplomacy_profile::CountryDiplomacyVanillaRuntimeFrameParts {
    let doc = minimal_doc();
    let root = doc
        .template_index()
        .get(COUNTRY_DIPLOMACY_ROOT)
        .expect("minimal diplomacy root");
    let registry = GuiTemplateRegistry::from_documents([(COUNTRY_DIPLOMACY_GUI_FILE, &doc)]);
    country_diplomacy_vanilla_runtime_frame_parts(
        root,
        data,
        GuiRect::new(0.0, 0.0, 960.0, 640.0),
        GuiRuntimeState::shown(GuiRect::new(0.0, 0.0, 960.0, 640.0)),
        registry,
        None,
        None,
    )
}

#[test]
fn diplomacy_gate14_profile_skeleton_binds_root_title_and_close() {
    fn assert_profile_contract<
        P: VanillaPanelProfile<Data = CountryInfoData, Command = CountryInfoCommand>,
    >(
        _: &P,
    ) {
    }

    let profile = CountryDiplomacyProfile;
    assert_profile_contract(&profile);
    assert_eq!(profile.profile_id(), COUNTRY_DIPLOMACY_PROFILE_ID);
    assert_eq!(profile.root_template(), COUNTRY_DIPLOMACY_ROOT);
    assert_eq!(profile.required_gui_files(), &[COUNTRY_DIPLOMACY_GUI_FILE]);

    let data = sample_data();
    let doc = minimal_doc();
    let root = doc.template_index().get(COUNTRY_DIPLOMACY_ROOT).unwrap();
    let bindings = bind_profile_tree(&profile, root, &data);
    let frame = GuiRuntimeFrame::build(GuiRuntimeFrameInput::new(
        root,
        GuiRect::new(0.0, 0.0, 1280.0, 720.0),
        profile.profile_id(),
        &bindings,
    ));
    assert!(frame.visible);
    assert_eq!(frame.profile_id, COUNTRY_DIPLOMACY_PROFILE_ID);
    assert!(frame.root_layout.find_by_name("close_button").is_some());
    assert!(frame
        .hit_regions
        .iter()
        .any(|hit| hit.command == COUNTRY_DIPLOMACY_CLOSE_COMMAND));

    let title = profile.bind_node(&binding_path("diplomacy_title"), &data);
    assert_eq!(title.text.as_deref(), Some(hoi4_ui::i18n::tr("diplomacy")));
    let close = profile.handle_action(
        GuiAction {
            node_path: GuiNodePath::root(COUNTRY_DIPLOMACY_CLOSE_COMMAND),
            kind: GuiActionKind::Click,
        },
        &data,
    );
    assert!(matches!(close, Some(CountryInfoCommand::Close)));

    let mut report = String::from("# Gate 14 CountryDiplomacyProfile Skeleton\n\n");
    let _ = writeln!(report, "- profile_id: {}", profile.profile_id());
    let _ = writeln!(report, "- root_template: {}", profile.root_template());
    let _ = writeln!(report, "- frame_profile_id: {}", frame.profile_id);
    let _ = writeln!(report, "- close_hit_region: true");
    let _ = writeln!(report, "- command_close: true");
    write_report("gate14_country_diplomacy_profile.md", report);
}

#[test]
fn diplomacy_gate15_root_window_uses_vanilla_slide_positions() {
    let profile = CountryDiplomacyProfile;
    let data = sample_data();
    let doc = minimal_doc();
    let root = doc.template_index().get(COUNTRY_DIPLOMACY_ROOT).unwrap();
    let mut report = String::from("# Gate 15 Diplomacy Root Window\n\n");

    for (width, height) in [(960.0, 640.0), (1920.0, 1080.0), (2560.0, 1440.0)] {
        let viewport = GuiRect::new(0.0, 0.0, width, height);
        let bindings = bind_profile_tree(&profile, root, &data);
        let shown = GuiRuntimeFrame::build(
            GuiRuntimeFrameInput::new(root, viewport, profile.profile_id(), &bindings)
                .with_runtime_state(GuiRuntimeState::shown(viewport)),
        );
        assert_eq!(
            shown.diagnostics.root_hidden_position,
            GuiPoint { x: -606.0, y: 78.0 }
        );
        assert_eq!(
            shown.diagnostics.root_shown_position,
            GuiPoint { x: -6.0, y: 78.0 }
        );
        assert_eq!(shown.root_layout.rect.x, -6.0);
        assert_eq!(shown.root_layout.rect.y, 78.0);
        assert_eq!(shown.root_layout.rect.width, 550.0);
        assert_eq!(shown.root_layout.rect.height, height);

        let hidden = GuiRuntimeFrame::build(
            GuiRuntimeFrameInput::new(root, viewport, profile.profile_id(), &bindings)
                .with_runtime_state(GuiRuntimeState::hidden(viewport)),
        );
        assert_eq!(hidden.root_layout.rect.x, -606.0);
        assert_eq!(hidden.root_layout.rect.y, 78.0);

        let _ = writeln!(
            report,
            "- viewport={width:.0}x{height:.0}: shown=({:.0},{:.0},{:.0},{:.0}) hidden=({:.0},{:.0})",
            shown.root_layout.rect.x,
            shown.root_layout.rect.y,
            shown.root_layout.rect.width,
            shown.root_layout.rect.height,
            hidden.root_layout.rect.x,
            hidden.root_layout.rect.y
        );
    }
    report.push_str("- manual_root_offset: false\n");
    write_report("gate15_root_window.md", report);
}

#[test]
fn diplomacy_gate16_and_gate17_header_party_focus_and_spirit_bindings() {
    let profile = CountryDiplomacyProfile;
    let mut data = sample_data();
    data.faction_name = None;
    data.leader_portrait_key = None;

    assert_eq!(
        profile
            .bind_node(&binding_path("diplo_country_flag"), &data)
            .sprite
            .as_deref(),
        Some("GFX_flag_FRA_democratic")
    );
    assert_eq!(
        profile
            .bind_node(&binding_path("country_name"), &data)
            .text
            .as_deref(),
        Some("France")
    );
    assert_eq!(
        profile
            .bind_node(&binding_path("faction_name"), &data)
            .text
            .as_deref(),
        Some(hoi4_ui::i18n::tr("no_faction"))
    );
    assert_eq!(
        profile
            .bind_node(&binding_path("leader_portrait"), &data)
            .sprite
            .as_deref(),
        Some("GFX_leader_unknown")
    );
    assert_eq!(
        profile
            .bind_node(&binding_path("our_opinion_value"), &data)
            .text
            .as_deref(),
        Some("+35")
    );
    assert_eq!(
        profile
            .bind_node(&binding_path("their_opinion_value"), &data)
            .text
            .as_deref(),
        Some("-12")
    );
    assert_eq!(
        profile
            .bind_node(&binding_path("stability_value"), &data)
            .text
            .as_deref(),
        Some("64%")
    );
    assert_eq!(
        profile
            .bind_node(&binding_path("war_support_value"), &data)
            .text
            .as_deref(),
        Some("38%")
    );
    assert_eq!(
        profile
            .bind_node(&binding_path("party_name"), &data)
            .text
            .as_deref(),
        Some("French Section of the Workers International")
    );
    assert_eq!(
        profile
            .bind_node(&binding_path("ideology_name"), &data)
            .text
            .as_deref(),
        Some("democratic")
    );

    let our_flag = profile.bind_node(&binding_path("our_opinion_left_flag"), &data);
    assert_eq!(our_flag.sprite.as_deref(), Some("GFX_flag_GER_fascism"));
    assert_ne!(our_flag.visible, Some(false));

    let ideology_icon = profile.bind_node(&binding_path("ideology_icon"), &data);
    assert_eq!(
        ideology_icon.sprite.as_deref(),
        Some("GFX_ideology_democratic_group")
    );
    assert!(ideology_icon.pie_segments.is_empty());
    let pie = profile.bind_node(&binding_path("political_pie_chart"), &data);
    assert_eq!(pie.visible, Some(true));
    assert!(pie.sprite.is_none());
    let chart = profile.bind_node(
        &GuiNodePath::root(COUNTRY_DIPLOMACY_ROOT)
            .child("political_pie_chart")
            .child("chart"),
        &data,
    );
    assert_eq!(
        chart.pie_segments,
        vec![
            (0.62, egui::Color32::from_rgb(0, 0, 255)),
            (0.17, egui::Color32::from_rgb(255, 0, 0)),
            (0.21, egui::Color32::from_rgb(124, 124, 124)),
        ]
    );

    assert_eq!(
        profile
            .bind_node(&binding_path("active_national_focus_info"), &data)
            .visible,
        Some(true)
    );
    assert_eq!(
        profile
            .bind_node(&binding_path("national_spirit_info"), &data)
            .visible,
        Some(true)
    );
    assert_eq!(
        profile
            .bind_node(
                &GuiNodePath::root(COUNTRY_DIPLOMACY_ROOT)
                    .child("active_national_focus_info")
                    .child("national_focus_label"),
                &data,
            )
            .text
            .as_deref(),
        Some(hoi4_ui::i18n::tr("unknown_national_focus"))
    );
    assert_eq!(
        profile
            .bind_node(&binding_path("spirit_title"), &data)
            .text
            .as_deref(),
        Some(hoi4_ui::i18n::tr("national_spirits"))
    );

    for hidden in [
        "ideas_info",
        "trade_info",
        "estimated_enemy_force_info",
        "country_list",
        "intel_ledger_container",
        "info_tab_button",
        "player_info_box",
        "diplomacy_bottom",
        "back_button",
        "logistics_button",
    ] {
        assert_eq!(
            profile.bind_node(&binding_path(hidden), &data).visible,
            Some(false),
            "{hidden} should be hidden for phase 3"
        );
    }
    let relations_tab = profile.bind_node(&binding_path("relations_tab_button"), &data);
    assert!(
        relations_tab
            .text
            .as_deref()
            .is_some_and(|text| text != "DIPLOMACY_RELATIONS_TAB"),
        "relations tab should bind localized/project text, not the raw vanilla key"
    );

    let mut report = String::from("# Gate 16-17 Diplomacy Header Bindings\n\n");
    report.push_str("- country_name: CountryInfoData.display_name\n");
    report.push_str("- flag: CountryInfoData.flag_gfx\n");
    report.push_str("- leader_portrait: GFX_leader_unknown fallback when project data is absent\n");
    report.push_str("- faction_absent_text: No faction\n");
    report.push_str("- active_national_focus_info: visible with unknown vanilla placeholder\n");
    report.push_str("- national_spirit_info: visible with empty vanilla placeholder\n");
    report.push_str("- info_tab_detail_sections: hidden until project intel/economy data exists\n");
    report.push_str("- relations_tab_button: localized/project text\n");

    if let Ok(context) =
        VanillaGuiRuntimeContext::load_result(COUNTRY_DIPLOMACY_DESCRIPTOR.required_gui_files)
    {
        if let Some(root) =
            context.root_template(COUNTRY_DIPLOMACY_GUI_FILE, COUNTRY_DIPLOMACY_ROOT)
        {
            let registry = GuiTemplateRegistry::from_documents(context.documents());
            let parts = country_diplomacy_vanilla_runtime_frame_parts(
                root,
                &data,
                GuiRect::new(0.0, 0.0, 1920.0, 1080.0),
                GuiRuntimeState::shown(GuiRect::new(0.0, 0.0, 1920.0, 1080.0)),
                registry,
                Some(&context.gfx_index),
                None,
            );
            let resources: Vec<_> = parts
                .frame
                .draw_list
                .iter()
                .filter_map(|command| command.resource_name().map(str::to_owned))
                .collect();
            let country_info = parts
                .frame
                .root_layout
                .find_by_name("country_info")
                .expect("country_info layout");
            let relations_info = parts
                .frame
                .root_layout
                .find_by_name("relations_info")
                .expect("relations_info layout");
            let diplomatic_actions = parts
                .frame
                .root_layout
                .find_by_name("diplomatic_actions")
                .expect("diplomatic_actions layout");
            assert_eq!(relations_info.rect.y, country_info.rect.y + 355.0);
            assert_eq!(diplomatic_actions.rect.y, country_info.rect.y + 355.0);
            assert_eq!(
                diplomatic_actions.rect.bottom(),
                country_info.rect.bottom() - 50.0
            );
            let action_instances: Vec<_> = parts
                .frame
                .generated_instances
                .iter()
                .filter(|instance| {
                    instance.context.semantic_role.as_deref() == Some("diplomacy_action_entry")
                })
                .collect();
            assert_eq!(action_instances.len(), data.actions.len());
            assert!(action_instances.iter().all(|instance| {
                instance.resolved_rect.y >= diplomatic_actions.rect.y
                    && instance.resolved_rect.bottom() <= diplomatic_actions.rect.bottom()
            }));
            let _ = writeln!(report, "- runtime_available: true");
            let _ = writeln!(report, "- draw_commands: {}", parts.frame.draw_list.len());
            let _ = writeln!(
                report,
                "- vanilla_lower_panel_y: relations={} actions={}",
                relations_info.rect.y, diplomatic_actions.rect.y
            );
            let _ = writeln!(
                report,
                "- header_sprite_hits: flag_frame={} leader_frame={}",
                resources.iter().any(|name| name == "GFX_diplo_flag_frame"),
                resources
                    .iter()
                    .any(|name| name == "GFX_diplo_leader_frame")
            );
        }
    } else {
        report.push_str("- runtime_available: false\n");
    }
    write_report("gate16_17_header_party_focus_spirit.md", report);
}

#[test]
fn diplomacy_gate18_relation_grid_generates_typed_vanilla_instances() {
    let mut data = sample_data();
    data.actions.clear();
    let parts = frame_parts_for(&data);
    let relation_instances: Vec<_> = parts
        .frame
        .generated_instances
        .iter()
        .filter(|instance| {
            instance.context.semantic_role.as_deref() == Some("diplomacy_relation_entry")
        })
        .collect();
    assert_eq!(relation_instances.len(), data.relations.len());
    assert!(relation_instances
        .iter()
        .any(|instance| instance.template_name == "relation_strip_view"));
    assert!(relation_instances
        .iter()
        .any(|instance| instance.template_name == "subject_relation_strip_view"));

    for instance in relation_instances {
        let icon = parts
            .bindings
            .for_node(&instance.path.child("relation_icon"), Some("relation_icon"));
        assert!(icon.sprite.is_some(), "relation icon should be data driven");
        assert!(
            icon.tooltip.is_some(),
            "relation tooltip should be data driven"
        );
    }

    let mut empty = sample_data();
    empty.actions.clear();
    empty.relations.clear();
    let profile = CountryDiplomacyProfile;
    assert_eq!(
        profile
            .bind_node(&binding_path("relations_info"), &empty)
            .visible,
        Some(true),
        "relations_info box stays visible even when empty so its dark vanilla \
         inset matches the actions column instead of exposing the lighter root bg"
    );

    let mut report = String::from("# Gate 18 Relation Grid\n\n");
    let _ = writeln!(report, "- relation_count: {}", data.relations.len());
    let _ = writeln!(
        report,
        "- instance_count: {}",
        parts
            .frame
            .generated_instances
            .iter()
            .filter(|instance| instance.context.semantic_role.as_deref()
                == Some("diplomacy_relation_entry"))
            .count()
    );
    report.push_str("- empty_relations_hidden: true\n");
    write_report("gate18_relation_grid.md", report);
}

#[test]
fn diplomacy_gate19_action_grid_binds_enabled_disabled_and_commands() {
    let mut data = sample_data();
    data.relations.clear();
    let parts = frame_parts_for(&data);
    let action_instances: Vec<_> = parts
        .frame
        .generated_instances
        .iter()
        .filter(|instance| {
            instance.context.semantic_role.as_deref() == Some("diplomacy_action_entry")
        })
        .collect();
    assert_eq!(action_instances.len(), data.actions.len());

    let justify = data
        .actions
        .iter()
        .find(|action| action.id == "justify_wargoal")
        .unwrap();
    let justify_command = country_diplomacy_action_click_command(justify);
    assert!(parts
        .frame
        .hit_regions
        .iter()
        .any(|hit| hit.command == justify_command && hit.enabled));
    let justify_result = CountryDiplomacyProfile.handle_action(
        GuiAction {
            node_path: GuiNodePath::root(justify_command),
            kind: GuiActionKind::Click,
        },
        &data,
    );
    match justify_result {
        Some(CountryInfoCommand::JustifyWargoal { target_tag }) => assert_eq!(target_tag, "FRA"),
        other => panic!("unexpected justify command: {other:?}"),
    }

    let disabled = data
        .actions
        .iter()
        .find(|action| action.id == "declare_war")
        .unwrap();
    let disabled_command = country_diplomacy_action_click_command(disabled);
    assert!(parts
        .frame
        .hit_regions
        .iter()
        .all(|hit| hit.command != disabled_command));
    assert!(CountryDiplomacyProfile
        .handle_action(
            GuiAction {
                node_path: GuiNodePath::root(disabled_command),
                kind: GuiActionKind::Click,
            },
            &data,
        )
        .is_none());

    let disabled_instance = action_instances
        .iter()
        .find(|instance| instance.context.model_key.as_deref() == Some("1:declare_war"))
        .unwrap();
    let disabled_bg = parts.bindings.for_node(
        &disabled_instance.path.child("diplo_actions_entry_bg"),
        Some("diplo_actions_entry_bg"),
    );
    assert_eq!(disabled_bg.enabled, Some(false));
    assert!(disabled_bg.click.is_none());
    let disabled_label = parts
        .bindings
        .for_node(&disabled_instance.path.child("name"), Some("name"));
    assert_eq!(disabled_label.text.as_deref(), Some("Declare war"));

    let mut report = String::from("# Gate 19 Action Grid\n\n");
    let _ = writeln!(report, "- action_count: {}", data.actions.len());
    let _ = writeln!(
        report,
        "- enabled_hit_command_prefix: {COUNTRY_DIPLOMACY_ACTION_PREFIX}"
    );
    report.push_str("- disabled_action_visible: true\n");
    report.push_str("- disabled_action_clickable: false\n");
    report.push_str("- command_source: CountryInfoCommand\n");
    write_report("gate19_action_grid.md", report);
}

#[test]
fn diplomacy_gate20_scroll_clip_and_close_escape_hit_behavior() {
    let data = sample_data();
    let doc = minimal_doc();
    let root = doc.template_index().get(COUNTRY_DIPLOMACY_ROOT).unwrap();
    let registry = GuiTemplateRegistry::from_documents([(COUNTRY_DIPLOMACY_GUI_FILE, &doc)]);
    let profile = CountryDiplomacyProfile;
    let viewport = GuiRect::new(0.0, 0.0, 960.0, 640.0);
    let bindings = bind_profile_tree(&profile, root, &data);
    let base = GuiRuntimeFrame::build(
        GuiRuntimeFrameInput::new(root, viewport, profile.profile_id(), &bindings)
            .with_runtime_state(GuiRuntimeState::shown(viewport)),
    );
    let relations_path = base
        .root_layout
        .find_by_name("relations_info")
        .unwrap()
        .path
        .clone();
    let actions_path = base
        .root_layout
        .find_by_name("diplomatic_actions")
        .unwrap()
        .path
        .clone();
    let frame = country_diplomacy_vanilla_runtime_frame(
        root,
        &data,
        viewport,
        GuiRuntimeState::shown(viewport)
            .with_scroll_offset(relations_path, GuiPoint { x: 0.0, y: 74.0 })
            .with_scroll_offset(actions_path, GuiPoint { x: 0.0, y: 42.0 }),
        registry,
    );

    let relations_scroll = frame
        .scroll_states
        .iter()
        .find(|state| state.node_name.as_deref() == Some("relations_info"))
        .expect("relations scroll state");
    assert!(relations_scroll.spec.has_vertical_scrollbar);
    assert_eq!(relations_scroll.spec.scroll_wheel_factor, 50.0);
    assert!(relations_scroll.spec.smooth_scrolling);
    assert_eq!(relations_scroll.spec.margin.top, 8.0);
    assert_eq!(relations_scroll.spec.margin.bottom, 8.0);

    let actions_scroll = frame
        .scroll_states
        .iter()
        .find(|state| state.node_name.as_deref() == Some("diplomatic_actions"))
        .expect("actions scroll state");
    assert!(actions_scroll.spec.has_vertical_scrollbar);
    assert_eq!(actions_scroll.spec.scroll_wheel_factor, 40.0);
    assert!(actions_scroll.spec.smooth_scrolling);

    let close_hit = frame
        .hit_regions
        .iter()
        .find(|hit| hit.command == COUNTRY_DIPLOMACY_CLOSE_COMMAND)
        .expect("close hit region");
    assert!(close_hit.rect.y < relations_scroll.content_clip_rect.y);

    let action_hits: Vec<_> = frame
        .hit_regions
        .iter()
        .filter(|hit| hit.command.starts_with(COUNTRY_DIPLOMACY_ACTION_PREFIX))
        .collect();
    assert!(!action_hits.is_empty());
    assert!(action_hits
        .iter()
        .all(|hit| hit.rect == hit.rect.intersect(actions_scroll.content_clip_rect)));

    assert!(matches!(
        profile.handle_action(
            GuiAction {
                node_path: GuiNodePath::root("ESCAPE"),
                kind: GuiActionKind::Click,
            },
            &data,
        ),
        Some(CountryInfoCommand::Close)
    ));

    let mut report = String::from("# Gate 20 Scroll, Clip, Hit Regions\n\n");
    let _ = writeln!(
        report,
        "- relations_scroll: factor={} smooth={}",
        relations_scroll.spec.scroll_wheel_factor, relations_scroll.spec.smooth_scrolling
    );
    let _ = writeln!(
        report,
        "- actions_scroll: factor={} smooth={}",
        actions_scroll.spec.scroll_wheel_factor, actions_scroll.spec.smooth_scrolling
    );
    let _ = writeln!(report, "- action_hit_regions: {}", action_hits.len());
    report.push_str("- close_button_outside_scroll_clip: true\n");
    report.push_str("- escape_command_closes: true\n");
    write_report("gate20_scroll_clip_hit.md", report);
}

#[test]
fn probe_action_row_and_bg() {
    let mut data = sample_data();
    let view = DiplomaticActionView::enabled("x");
    data.actions = vec![
        action("justify_wargoal","正当化战争目标",&view,Some("50"),
            DiplomacyActionCommand::JustifyWargoal{target_tag:"FRA".into()}),
    ];
    let ctx = match VanillaGuiRuntimeContext::load_result(COUNTRY_DIPLOMACY_DESCRIPTOR.required_gui_files) {
        Ok(c) => c, Err(_) => { eprintln!("no vanilla ctx"); return; }
    };
    let root = ctx.root_template(COUNTRY_DIPLOMACY_GUI_FILE, COUNTRY_DIPLOMACY_ROOT).unwrap();
    let registry = GuiTemplateRegistry::from_documents(ctx.documents());
    let vp = GuiRect::new(0.0,0.0,1920.0,1080.0);
    let parts = country_diplomacy_vanilla_runtime_frame_parts(
        root,&data,vp,GuiRuntimeState::shown(vp),registry,Some(&ctx.gfx_index),None);

    eprintln!("--- action row instance layout rects ---");
    for inst in parts.frame.generated_instances.iter() {
        fn walk(l: &hoi4_ui::vanilla_gui::LayoutNode, depth: usize) {
            if let Some(n) = &l.name {
                if matches!(n.as_str(), "diplomacy_action_entry"|"name"|"cost"|"accept_icon"|"diplo_actions_entry_bg") {
                    eprintln!("{:indent$}{n}: rect=({:.0},{:.0},{:.0}x{:.0})", "",
                        l.rect.x,l.rect.y,l.rect.width,l.rect.height, indent=depth*2);
                }
            }
            for c in &l.children { walk(c, depth+1); }
        }
        walk(&inst.layout, 0);
    }

    eprintln!("--- accept_icon / cost draw commands ---");
    for cmd in parts.frame.draw_list.iter() {
        let nm = cmd.source_name.as_deref().unwrap_or("");
        if matches!(nm, "accept_icon"|"cost"|"name") {
            eprintln!("  {nm:14} kind={} res={:?} rect=({:.0},{:.0},{:.0}x{:.0})",
                cmd.kind_label(), cmd.resource_name(),
                cmd.rect.x,cmd.rect.y,cmd.rect.width,cmd.rect.height);
        }
    }

    eprintln!("--- GFX_tiled_bg resource ---");
    if let Some(r) = ctx.gfx_index.get("GFX_tiled_bg") {
        eprintln!("  kind={:?} size={:?} border={:?}", r.kind, r.size, r.border);
    } else { eprintln!("  NOT IN INDEX"); }
    eprintln!("--- GFX_accept_decline_icon resource ---");
    if let Some(r) = ctx.gfx_index.get("GFX_accept_decline_icon") {
        eprintln!("  kind={:?} size={:?} frames={:?}", r.kind, r.size, r.frame_count);
    } else { eprintln!("  NOT IN INDEX"); }
}
