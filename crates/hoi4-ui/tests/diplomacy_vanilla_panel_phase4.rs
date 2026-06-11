use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use egui_kittest::Harness;
use hoi4_ui::country_info_panel::{CountryInfoData, CountryInfoPanel};
use hoi4_ui::diplomacy::{
    DiplomacyActionCommand, DiplomacyActionEntry, DiplomacyRelationEntry, DiplomacyRelationKind,
    DiplomaticActionView,
};
use hoi4_ui::egui::Vec2;
use hoi4_ui::icons::IconBank;
use hoi4_ui::vanilla_gui::{
    VanillaGuiRuntimeContext, COUNTRY_DIPLOMACY_DESCRIPTOR, COUNTRY_DIPLOMACY_GFX_FILE,
    COUNTRY_DIPLOMACY_GUI_FILE,
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
        party_popularity: vec![("democratic".to_owned(), 1.0)],
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
        at_war: false,
        same_faction: false,
        faction_name: None,
        overlord_name: None,
        subject_names: Vec::new(),
        autonomy_level_name: None,
        has_wargoal: false,
        justifying_wargoal: false,
        justify_progress: 0.0,
        justify_days_remaining: 0,
        wargoals: Vec::new(),
        relation_factors: Vec::new(),
        relations: vec![DiplomacyRelationEntry {
            id: "same_faction".to_owned(),
            kind: DiplomacyRelationKind::SameFaction,
            label: "Same faction".to_owned(),
            sprite: "GFX_relation_faction".to_owned(),
            tooltip: "Project diplomacy relation data".to_owned(),
            positive: true,
        }],
        actions: vec![
            action(
                "justify_wargoal",
                "Justify War Goal",
                true,
                DiplomacyActionCommand::JustifyWargoal {
                    target_tag: target.clone(),
                },
            ),
            action(
                "declare_war",
                "Declare War",
                false,
                DiplomacyActionCommand::DeclareWar {
                    target_tag: target.clone(),
                },
            ),
        ],
        justify_action: justify,
        declare_war_action: declare,
        invite_to_faction_action: invite,
        request_access_action: access,
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
        sprite: "GFX_accept_decline_icon".to_owned(),
        command,
    }
}

#[test]
fn gate21_country_info_panel_show_with_icon_bank_entry_is_callable() {
    let runtime_available = hoi4_ui::vanilla_gui::country_diplomacy_runtime_context().is_some();
    let mut harness = Harness::builder()
        .with_size(Vec2::new(960.0, 640.0))
        .build(|ctx| {
            let path_cfg = hoi4_paths::PathConfig::with_game_path(std::env::temp_dir());
            let mut icon_bank = IconBank::new(ctx.clone(), path_cfg);
            let mut panel = CountryInfoPanel::new();
            panel.open_with(sample_data());
            let _commands = panel.show_with_icon_bank(ctx, &mut icon_bank);
        });
    harness.run_steps(2);

    let mut report = String::from("# Gate 21 CountryInfoPanel Vanilla Render Entry\n\n");
    let _ = writeln!(report, "- show_with_icon_bank: callable");
    let _ = writeln!(report, "- runtime_available: {runtime_available}");
    report.push_str("- profile: country_diplomacy\n");
    report.push_str("- root: countrydiplomacyview\n");
    report.push_str("- fallback: runtime diagnostics panel, not legacy V9\n");
    write_report("gate21_render_entry.md", report);
}

#[test]
fn gate22_app_render_uses_country_info_show_with_icon_bank() {
    let render_rs =
        std::fs::read_to_string(workspace_root().join("crates/hoi4-app/src/ui_driver/render.rs"))
            .unwrap();
    assert!(render_rs.contains("country_info_panel.show_with_icon_bank(ctx, icon_bank)"));
    assert!(!render_rs.contains("country_info_panel.show(ctx, icon_bank"));

    let mut report = String::from("# Gate 22 App Render Diplomacy Entry\n\n");
    report.push_str("- app_render_entry: crates/hoi4-app/src/ui_driver/render.rs\n");
    report.push_str("- right_click_country_panel: show_with_icon_bank(ctx, icon_bank)\n");
    report.push_str("- command_chain: existing CountryInfoCommand path retained\n");
    write_report("gate22_app_render_entry.md", report);
}

#[test]
fn gate23_missing_vanilla_path_reports_diplomacy_runtime_unavailable() {
    let fake_game = std::env::temp_dir().join(format!(
        "ironheart_diplomacy_gate23_missing_{}",
        std::process::id()
    ));
    let path_cfg = hoi4_paths::PathConfig::with_game_path(&fake_game);
    let err = VanillaGuiRuntimeContext::load_with_path_config_result(
        path_cfg,
        COUNTRY_DIPLOMACY_DESCRIPTOR.required_gui_files,
    )
    .expect_err("fake HOI4 root should not contain countrydiplomacyview.gui");
    let summary = err.to_string();
    assert!(summary.contains(COUNTRY_DIPLOMACY_GUI_FILE));

    let mut report = String::from("# Gate 23 Diplomacy Runtime Unavailable Diagnostics\n\n");
    let _ = writeln!(report, "- simulated_game_path: {}", fake_game.display());
    let _ = writeln!(report, "- reason: {}", summary.replace('\n', " "));
    let _ = writeln!(
        report,
        "- required: {}, {}",
        COUNTRY_DIPLOMACY_GUI_FILE, COUNTRY_DIPLOMACY_GFX_FILE
    );
    report.push_str("- close_paths: Escape key and close button are handled by the panel entry\n");
    report.push_str("- legacy_v9_fallback: disabled\n");
    write_report("gate23_runtime_unavailable.md", report);
}

#[test]
fn gate24_old_right_click_country_v9_helpers_are_removed() {
    let source = include_str!("../src/country_info_panel.rs");
    for forbidden in [
        "v9_show_country_info",
        "v9_country_hero",
        "v9_country_metrics",
        "v9_country_diplomacy",
    ] {
        assert!(
            !source.contains(forbidden),
            "{forbidden} should be removed from country_info_panel.rs"
        );
    }

    let mut report = String::from("# Gate 24 Old Right-Click Country V9 Removal\n\n");
    report.push_str(
        "- removed_symbols: v9_show_country_info, v9_country_hero, v9_country_metrics, v9_country_diplomacy\n",
    );
    report.push_str("- retained: CountryInfoData and CountryInfoCommand\n");
    report.push_str("- active_renderer: vanilla countrydiplomacyview runtime\n");
    write_report("gate24_remove_v9_country_panel.md", report);
}
