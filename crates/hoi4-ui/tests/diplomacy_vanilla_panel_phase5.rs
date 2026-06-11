use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use hoi4_ui::diplomacy::{
    CountryDiplomacyDetail, CountryEntry, DiplomacyActionCommand, DiplomacyActionEntry,
    DiplomacyCommand, DiplomacyData, DiplomacyRelationEntry, DiplomacyRelationKind,
    DiplomaticActionView, FactionEntry,
};
use hoi4_ui::diplomacy_profile::{
    diplomacy_panel_country_select_command, diplomacy_panel_vanilla_runtime_frame_parts,
    DiplomacyPanelProfile, DIPLOMACY_PANEL_SELECT_COUNTRY_PREFIX,
    DIPLOMACY_PANEL_SORT_NAME_COMMAND, DIPLOMACY_PANEL_SORT_OPINION_COMMAND,
};
use hoi4_ui::vanilla_gui::{
    bind_profile_tree, parse_gui_str, GuiAction, GuiActionKind, GuiNodePath, GuiRect,
    GuiRuntimeState, GuiTemplateRegistry, VanillaPanelProfile, COUNTRY_DIPLOMACY_GUI_FILE,
    COUNTRY_DIPLOMACY_PROFILE_ID, COUNTRY_DIPLOMACY_ROOT,
};
use hoi4_ui::{ActiveDetailPanel, PanelCommand};

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

        instantTextboxType = { name = "diplomacy_title" position = { x = 45 y = 8 } maxWidth = 240 maxHeight = 24 }

        containerWindowType = {
            name = "country_info"
            position = { x = 0 y = 50 }
            size = { width = 530 height = 100%% }
            iconType = { name = "diplo_country_flag" position = { x = 40 y = 1 } size = { width = 82 height = 52 } quadTextureSprite = "GFX_shield_medium" }
            instantTextboxType = { name = "country_name" position = { x = 245 y = 3 } maxWidth = 195 maxHeight = 20 }
            containerWindowType = {
                name = "faction"
                position = { x = 245 y = 23 }
                size = { width = 175 height = 20 }
                instantTextboxType = { name = "faction_name" position = { x = 0 y = 0 } maxWidth = 173 maxHeight = 20 }
            }
            instantTextboxType = { name = "leader_name" position = { x = 245 y = 43 } maxWidth = 173 maxHeight = 20 }
            instantTextboxType = { name = "our_opinion_value" position = { x = 407 y = 1 } maxWidth = 40 maxHeight = 20 }
            instantTextboxType = { name = "their_opinion_value" position = { x = 407 y = 31 } maxWidth = 40 maxHeight = 20 }
            iconType = { name = "leader_portrait" position = { x = 20 y = 120 } size = { width = 92 height = 120 } spriteType = "GFX_leader_unknown" }
            buttonType = { name = "back_button" position = { x = 14 y = 452 } size = { width = 80 height = 28 } quadTextureSprite = "GFX_sort_button_202x29" }
            buttonType = { name = "info_tab_button" position = { x = 275 y = 70 } size = { width = 260 height = 34 } quadTextureSprite = "GFX_tab_intel_ledger" }
            containerWindowType = {
                name = "relations_info"
                position = { x = 11 y = 285 }
                size = { width = 268 height = 156 }
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
        }

        containerWindowType = {
            name = "country_list"
            position = { x = 0 y = 50}
            size = { width = 530 height = 100%% }
            containerWindowType = {
                name = "top_filter_window"
                position = { x = 3 y = -10 }
                size = { width = 545 height = 135 }
                gridboxtype = {
                    name = "filters_grid"
                    position = { x = 105 y = 1 }
                    size = { width = 100%% height = 100%% }
                    slotsize = { width = 105 height = 35 }
                    max_slots_horizontal = 4
                    format = "UPPER_LEFT"
                }
            }
            containerWindowType = {
                name = "filters"
                position = { x = 3 y = 125 }
                size = { width = 542 height = 35 }
                buttonType = { name = "sort_alphabetical" position = { x= 10 y = 2 } size = { width = 202 height = 29 } quadTextureSprite = "GFX_sort_button_202x29" }
                buttonType = { name = "opinion_flag1" position = { x= 245 y = 4 } size = { width = 24 height = 16 } quadTextureSprite ="GFX_flag_small" }
                buttonType = { name = "opinion_flag2" position = { x= 307 y = 4 } size = { width = 24 height = 16 } quadTextureSprite ="GFX_flag_small" }
            }
            containerWindowType = {
                name = "countries"
                position = { x = 0 y = 155 }
                size = { width = 545 height = -5 }
                verticalScrollbar = "right_vertical_slider"
                scroll_wheel_factor = 45
                smooth_scrolling = yes
                gridboxtype = {
                    name = "countries_grid"
                    position = { x = 12 y = 5 }
                    size = { width = 100%% height = 100%% }
                    slotsize = { width = 100%% height = 45 }
                    max_slots_horizontal = 1
                    format = "UPPER_LEFT"
                }
            }
        }

        containerWindowType = { name = "intel_ledger_container" position = { x = 0 y = 151 } size = { width = 100%% height = 100%% } }
        buttonType = { name = "close_button" position = { x = -43 y = 9 } size = { width = 26 height = 26 } quadTextureSprite = "GFX_closebutton" shortcut = "ESCAPE" Orientation = "UPPER_RIGHT" }
    }

    containerWindowType = {
        name = "diplomacy_country_list_country_entry"
        size = { width = 500 height = 44 }
        background = { name = "Background" spriteType = "GFX_diplo_countrylist_entry" }
        iconType = { name = "diplolist_country_flag" position = { x= 10 y = 10 } size = { width = 36 height = 24 } quadTextureSprite ="GFX_flag_small2" }
        instantTextboxType = { name = "name" position = { x = 56 y = 6 } maxWidth = 110 maxHeight = 40 }
        instantTextboxType = { name = "our_opinion" position = { x = 232 y = 15 } maxWidth = 50 maxHeight = 20 }
        instantTextboxType = { name = "their_opinion" position = { x = 292 y = 15 } maxWidth = 50 maxHeight = 20 }
        OverlappingElementsBoxType = { name = "relations" position = { x = 350 y = 8 } size = { x = 155 y = 32 } }
    }

    containerWindowType = {
        name = "diplomacy_country_list_relation_entry"
        size = { width = 32 height = 32 }
        iconType = { name = "icon" position = { x = 0 y = 0 } size = { width = 32 height = 32 } quadTextureSprite = "GFX_relation_unknown" }
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
        iconType = { name = "relation_icon" position = { x = -3 y = 12 } size = { width = 32 height = 32 } spriteType = "GFX_relation_truce" }
    }
}
"#,
    )
}

fn sample_data(selected: Option<&str>) -> DiplomacyData {
    let countries = vec![
        country("FRA", "France", 15, selected),
        country("ENG", "United Kingdom", -10, selected),
        country("ITA", "Italy", 55, selected),
    ];
    DiplomacyData {
        player_tag: "GER".to_owned(),
        player_flag_gfx: "GFX_flag_GER_fascism".to_owned(),
        player_faction: Some(FactionEntry {
            name: "Axis".to_owned(),
            leader_tag: "GER".to_owned(),
            member_tags: vec!["GER".to_owned(), "ITA".to_owned()],
        }),
        all_factions: Vec::new(),
        countries,
        active_wars: Vec::new(),
        requests: Vec::new(),
        world_tension: 12.0,
    }
}

fn country(tag: &str, display_name: &str, opinion: i16, selected: Option<&str>) -> CountryEntry {
    let selected = selected == Some(tag);
    let detail = selected.then(|| detail(tag, display_name, opinion));
    CountryEntry {
        target_tag: tag.to_owned(),
        tag: tag.to_owned(),
        display_name: display_name.to_owned(),
        flag_gfx: format!("GFX_flag_{tag}_neutrality"),
        ruling_party: "neutrality".to_owned(),
        ruling_party_label: "Non-Aligned".to_owned(),
        party_full_name: format!("{display_name} party"),
        party_popularity: vec![("neutrality".to_owned(), 1.0)],
        opinion,
        our_opinion_of_target: opinion,
        their_opinion_of_us: -opinion,
        at_war: tag == "FRA",
        same_faction: tag == "ITA",
        relations: vec![
            relation(
                "same_faction",
                DiplomacyRelationKind::SameFaction,
                "GFX_relation_faction",
            ),
            relation(
                "wargoal",
                DiplomacyRelationKind::Wargoal,
                "GFX_relation_wargoal",
            ),
        ],
        selected,
        autonomy_summary: None,
        leader_name: format!("{display_name} leader"),
        leader_portrait_key: None,
        detail,
    }
}

fn detail(tag: &str, display_name: &str, opinion: i16) -> CountryDiplomacyDetail {
    let enabled = DiplomaticActionView::enabled("Can execute through project diplomacy.");
    let disabled = DiplomaticActionView::disabled("Declare war when ready.", "No war goal");
    CountryDiplomacyDetail {
        target_tag: tag.to_owned(),
        tag: tag.to_owned(),
        display_name: display_name.to_owned(),
        ruling_party: "neutrality".to_owned(),
        ruling_party_label: "Non-Aligned".to_owned(),
        party_full_name: format!("{display_name} party"),
        party_popularity: vec![("neutrality".to_owned(), 1.0)],
        opinion,
        our_opinion_of_target: opinion,
        their_opinion_of_us: -opinion,
        at_war: false,
        same_faction: false,
        faction_name: None,
        overlord_name: None,
        subject_names: Vec::new(),
        autonomy_level_name: None,
        domestic_population: 1_000_000,
        colonial_population: 0,
        governed_population: 1_000_000,
        subject_population: 0,
        imperial_population: 1_000_000,
        has_wargoal: false,
        justifying_wargoal: false,
        justify_progress: 0.0,
        justify_days_remaining: 0,
        wargoals: Vec::new(),
        relation_factors: Vec::new(),
        relations: vec![relation(
            "military_access",
            DiplomacyRelationKind::MilitaryAccess,
            "GFX_relation_military_access",
        )],
        actions: vec![
            action(
                "justify_wargoal",
                "Justify War Goal",
                true,
                DiplomacyActionCommand::JustifyWargoal {
                    target_tag: tag.to_owned(),
                },
            ),
            action(
                "declare_war",
                "Declare War",
                false,
                DiplomacyActionCommand::DeclareWar {
                    target_tag: tag.to_owned(),
                },
            ),
        ],
        justify_action: enabled.clone(),
        declare_war_action: disabled,
        invite_to_faction_action: enabled.clone(),
        request_access_action: enabled,
    }
}

fn relation(id: &str, kind: DiplomacyRelationKind, sprite: &str) -> DiplomacyRelationEntry {
    DiplomacyRelationEntry {
        id: id.to_owned(),
        kind,
        label: id.replace('_', " "),
        sprite: sprite.to_owned(),
        tooltip: format!("Project relation: {id}"),
        positive: !matches!(id, "wargoal"),
    }
}

fn action(
    id: &str,
    label: &str,
    enabled: bool,
    command: DiplomacyActionCommand,
) -> DiplomacyActionEntry {
    DiplomacyActionEntry {
        id: id.to_owned(),
        label: label.to_owned(),
        enabled,
        preview: format!("{label} preview"),
        reason: (!enabled).then(|| "Disabled by project diplomacy rules".to_owned()),
        cost_text: None,
        sprite: "GFX_diplo_actions_bg".to_owned(),
        command,
    }
}

fn frame_parts_for(
    data: &DiplomacyData,
    sort_by_opinion: bool,
) -> hoi4_ui::diplomacy_profile::DiplomacyPanelVanillaRuntimeFrameParts {
    let doc = minimal_doc();
    let root = doc
        .template_index()
        .get(COUNTRY_DIPLOMACY_ROOT)
        .expect("minimal diplomacy root");
    let registry = GuiTemplateRegistry::from_documents([(COUNTRY_DIPLOMACY_GUI_FILE, &doc)]);
    diplomacy_panel_vanilla_runtime_frame_parts(
        root,
        data,
        GuiRect::new(0.0, 0.0, 960.0, 640.0),
        GuiRuntimeState::shown(GuiRect::new(0.0, 0.0, 960.0, 640.0)),
        registry,
        None,
        None,
        sort_by_opinion,
    )
}

fn row_model_tags(
    parts: &hoi4_ui::diplomacy_profile::DiplomacyPanelVanillaRuntimeFrameParts,
) -> Vec<String> {
    parts
        .frame
        .generated_instances
        .iter()
        .filter(|instance| instance.template_name == "diplomacy_country_list_country_entry")
        .filter_map(|instance| instance.context.model_key.as_deref())
        .filter_map(|key| key.split_once(':').map(|(_, tag)| tag.to_owned()))
        .collect()
}

#[test]
fn gate25_diplomacy_panel_vanilla_country_list_entry_is_callable() {
    fn assert_profile_contract<
        P: VanillaPanelProfile<Data = DiplomacyData, Command = DiplomacyCommand>,
    >(
        _: &P,
    ) {
    }

    let profile = DiplomacyPanelProfile;
    assert_profile_contract(&profile);
    assert_eq!(profile.profile_id(), COUNTRY_DIPLOMACY_PROFILE_ID);
    assert_eq!(profile.root_template(), COUNTRY_DIPLOMACY_ROOT);

    let doc = minimal_doc();
    let root = doc.template_index().get(COUNTRY_DIPLOMACY_ROOT).unwrap();
    let data = sample_data(None);
    let bindings = bind_profile_tree(&profile, root, &data);
    let country_list = profile.bind_node(
        &GuiNodePath::root(COUNTRY_DIPLOMACY_ROOT).child("country_list"),
        &data,
    );
    let country_info = profile.bind_node(
        &GuiNodePath::root(COUNTRY_DIPLOMACY_ROOT).child("country_info"),
        &data,
    );
    let intel = profile.bind_node(
        &GuiNodePath::root(COUNTRY_DIPLOMACY_ROOT).child("intel_ledger_container"),
        &data,
    );
    let bottom = profile.bind_node(
        &GuiNodePath::root(COUNTRY_DIPLOMACY_ROOT).child("diplomacy_bottom"),
        &data,
    );
    let logistics = profile.bind_node(
        &GuiNodePath::root(COUNTRY_DIPLOMACY_ROOT).child("logistics_button"),
        &data,
    );
    assert_eq!(country_list.visible, Some(true));
    assert_eq!(country_info.visible, Some(false));
    assert_eq!(intel.visible, Some(false));
    assert_eq!(bottom.visible, Some(false));
    assert_eq!(logistics.visible, Some(false));
    assert_eq!(
        bindings
            .for_node(
                &GuiNodePath::root(COUNTRY_DIPLOMACY_ROOT)
                    .child("country_list")
                    .child("countries")
                    .child("countries_grid"),
                Some("countries_grid")
            )
            .instance_count,
        Some(data.countries.len())
    );

    let parts = frame_parts_for(&data, false);
    let row_count = parts
        .frame
        .generated_instances
        .iter()
        .filter(|instance| instance.template_name == "diplomacy_country_list_country_entry")
        .count();
    assert_eq!(row_count, data.countries.len());

    let selected = sample_data(Some("ITA"));
    let selected_country_list = profile.bind_node(
        &GuiNodePath::root(COUNTRY_DIPLOMACY_ROOT).child("country_list"),
        &selected,
    );
    let selected_country_info = profile.bind_node(
        &GuiNodePath::root(COUNTRY_DIPLOMACY_ROOT).child("country_info"),
        &selected,
    );
    let selected_bottom = profile.bind_node(
        &GuiNodePath::root(COUNTRY_DIPLOMACY_ROOT).child("diplomacy_bottom"),
        &selected,
    );
    let selected_back = profile.bind_node(
        &GuiNodePath::root(COUNTRY_DIPLOMACY_ROOT).child("back_button"),
        &selected,
    );
    let selected_logistics = profile.bind_node(
        &GuiNodePath::root(COUNTRY_DIPLOMACY_ROOT).child("logistics_button"),
        &selected,
    );
    let selected_relations_tab = profile.bind_node(
        &GuiNodePath::root(COUNTRY_DIPLOMACY_ROOT).child("relations_tab_button"),
        &selected,
    );
    assert_eq!(selected_country_list.visible, Some(false));
    assert_eq!(selected_country_info.visible, Some(true));
    assert_eq!(selected_bottom.visible, Some(true));
    assert_eq!(selected_back.visible, Some(true));
    assert_eq!(selected_logistics.visible, Some(true));
    assert_eq!(selected_logistics.enabled, Some(false));
    assert!(selected_relations_tab
        .text
        .as_deref()
        .is_some_and(|text| text != "DIPLOMACY_RELATIONS_TAB"));

    let mut report = String::from("# Gate 25 DiplomacyPanel Vanilla Country List Entry\n\n");
    report.push_str("- show_with_icon_bank: callable through DiplomacyPanel::show\n");
    report.push_str("- country_list_visible_without_selection: true\n");
    report.push_str("- country_info_hidden_without_selection: true\n");
    report.push_str("- intel_ledger_container_hidden: true\n");
    report.push_str("- detail_bottom_buttons: back visible, logistics disabled until wired\n");
    report.push_str("- relations_tab_button: localized/project text\n");
    let _ = writeln!(report, "- generated_country_rows: {row_count}");
    write_report("gate25_diplomacy_panel_entry.md", report);
}

#[test]
fn gate26_country_list_rows_sort_click_and_bind_project_data() {
    let data = sample_data(None);
    let by_name = frame_parts_for(&data, false);
    let by_opinion = frame_parts_for(&data, true);
    assert_eq!(row_model_tags(&by_name), vec!["FRA", "ITA", "ENG"]);
    assert_eq!(row_model_tags(&by_opinion), vec!["ITA", "FRA", "ENG"]);

    let first_row = by_name
        .frame
        .generated_instances
        .iter()
        .find(|instance| instance.template_name == "diplomacy_country_list_country_entry")
        .expect("country row instance");
    let row_binding = by_name.bindings.for_node(
        &first_row.path,
        Some("diplomacy_country_list_country_entry"),
    );
    assert_eq!(
        row_binding
            .click
            .as_ref()
            .map(|click| click.command.as_str()),
        Some(diplomacy_panel_country_select_command(&data.countries[0]).as_str())
    );
    assert!(row_binding
        .click
        .as_ref()
        .unwrap()
        .command
        .starts_with(DIPLOMACY_PANEL_SELECT_COUNTRY_PREFIX));

    let relation_icons = by_name
        .frame
        .generated_instances
        .iter()
        .filter(|instance| instance.template_name == "diplomacy_country_list_relation_entry")
        .count();
    assert_eq!(relation_icons, 6);

    let profile = DiplomacyPanelProfile;
    let select = profile.handle_action(
        GuiAction {
            node_path: GuiNodePath::root(format!("{DIPLOMACY_PANEL_SELECT_COUNTRY_PREFIX}ITA")),
            kind: GuiActionKind::Click,
        },
        &data,
    );
    assert!(matches!(
        select,
        Some(DiplomacyCommand::Panel(PanelCommand::OpenDetail(
            ActiveDetailPanel::Country(target)
        ))) if target.tag == "ITA"
    ));

    let sort_name = profile.bind_node(
        &GuiNodePath::root(COUNTRY_DIPLOMACY_ROOT).child("sort_alphabetical"),
        &data,
    );
    assert_eq!(
        sort_name.click.as_ref().map(|click| click.command.as_str()),
        Some(DIPLOMACY_PANEL_SORT_NAME_COMMAND)
    );
    let sort_opinion = profile.bind_node(
        &GuiNodePath::root(COUNTRY_DIPLOMACY_ROOT).child("opinion_flag1"),
        &data,
    );
    assert_eq!(
        sort_opinion
            .click
            .as_ref()
            .map(|click| click.command.as_str()),
        Some(DIPLOMACY_PANEL_SORT_OPINION_COMMAND)
    );

    let mut report = String::from("# Gate 26 Diplomacy Country List Rows\n\n");
    let _ = writeln!(report, "- name_sort_order: {:?}", row_model_tags(&by_name));
    let _ = writeln!(
        report,
        "- opinion_sort_order: {:?}",
        row_model_tags(&by_opinion)
    );
    let _ = writeln!(report, "- relation_icon_instances: {relation_icons}");
    report.push_str("- row_click_command: opens ActiveDetailPanel::Country\n");
    report.push_str("- row_fields: flag/name/our_opinion/their_opinion/project relations\n");
    write_report("gate26_country_list.md", report);
}

#[test]
fn gate27_old_main_diplomacy_v9_helpers_are_removed() {
    let source = include_str!("../src/diplomacy.rs");
    for forbidden in [
        "v9_show_diplomacy",
        "v9_diplomacy_body",
        "v9_diplomacy_country_list",
        "v9_diplomacy_detail",
    ] {
        assert!(
            !source.contains(forbidden),
            "{forbidden} should be removed from diplomacy.rs"
        );
    }
    assert!(source.contains("show_with_icon_bank"));
    assert!(source.contains("diplomacy_panel_vanilla_runtime_frame_parts"));

    let mut report = String::from("# Gate 27 Old Main Diplomacy V9 Removal\n\n");
    report.push_str(
        "- removed_symbols: v9_show_diplomacy, v9_diplomacy_body, v9_diplomacy_country_list, v9_diplomacy_detail\n",
    );
    report.push_str(
        "- active_renderer: vanilla countrydiplomacyview country_list/countries_grid runtime\n",
    );
    report.push_str("- retained: DiplomacyData, DiplomacyCommand, shared detail renderer\n");
    write_report("gate27_remove_v9_diplomacy_panel.md", report);
}
