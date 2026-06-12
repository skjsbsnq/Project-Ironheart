use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use egui_kittest::Harness;
use hoi4_ui::egui::Vec2;
use hoi4_ui::finance_panel::{
    BudgetBreakdownData, ConstructionFundingTraceData, EconomyDiagnosticEntry,
    EconomyEmploymentEntry, EconomySectorBuildingEntry, FinanceCommand, FinancePanel,
    FinancePanelData, FinancingBreakdownData, FiscalExpenseBreakdownData,
    FiscalRevenueBreakdownData, GdpBreakdownData, InvestmentPoolData,
    FINANCE_VANILLA_SNAPSHOT_1080P,
};
use hoi4_ui::finance_profile::{
    FinanceSector, FinanceTab, FinanceVanillaInput, FinanceVanillaProfile,
};
use hoi4_ui::icons::IconBank;
use hoi4_ui::theme::apply_vanilla_theme;
use hoi4_ui::vanilla_gui::{
    collect_gfx_references, country_finance_runtime_context, parse_gui_str, GfxIndex, GuiAction,
    GuiActionKind, GuiDrawCommandKind, GuiNodePath, GuiRect, GuiRuntimeFrame, GuiRuntimeState,
    GuiTemplateRegistry, VanillaPanelProfile, COUNTRY_FINANCE_DESCRIPTOR, COUNTRY_FINANCE_GUI_FILE,
    COUNTRY_FINANCE_ROOT, FINANCE_REQUIRED_SPRITES,
};
use hoi4_ui::{ActiveDetailPanel, ActivePrimaryPanel, PanelCommand};

const FORBIDDEN_FINANCE_SPRITE_PREFIX: &str = concat!("GFX_ih_", "finance");

fn report_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .join("target/finance_vanilla_gui")
        .join(name)
}

fn write_report(path: PathBuf, body: impl AsRef<str>) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, body.as_ref()).unwrap();
}

fn snapshot_if_enabled<State>(harness: &mut Harness<'_, State>, name: &str) {
    let enabled = std::env::var_os("RUN_FINANCE_PHASE6_SNAPSHOT").is_some()
        || std::env::var_os("UPDATE_SNAPSHOTS").is_some();
    if enabled {
        harness.snapshot(name);
    }
}

fn finance_doc() -> hoi4_ui::vanilla_gui::GuiDocument {
    parse_gui_str(
        Some(PathBuf::from(COUNTRY_FINANCE_GUI_FILE)),
        include_str!("../assets/interface/countryfinanceview.gui"),
    )
}

fn root_registry() -> (
    hoi4_ui::vanilla_gui::GuiDocument,
    GuiTemplateRegistry<'static>,
) {
    let doc = finance_doc();
    let leaked: &'static hoi4_ui::vanilla_gui::GuiDocument = Box::leak(Box::new(doc.clone()));
    let registry = GuiTemplateRegistry::from_documents([(COUNTRY_FINANCE_GUI_FILE, leaked)]);
    (doc, registry)
}

fn finance_gfx_index() -> GfxIndex {
    GfxIndex::parse_single(
        "finance_phase6.gfx",
        r#"
corneredTileSpriteType = {
    name = "GFX_tiled_window_1b_thin_border"
    texturefile = "gfx/interface/finance_test/window_frame.dds"
    borderSize = { x = 16 y = 16 }
}
spriteType = { name = "GFX_tiled_paper_bg2" texturefile = "gfx/interface/finance_test/paper.dds" }
spriteType = { name = "GFX_tiled_plain_bg" texturefile = "gfx/interface/finance_test/plain.dds" }
spriteType = { name = "GFX_win_header_short" texturefile = "gfx/interface/finance_test/header_short.dds" }
spriteType = { name = "GFX_header_bg" texturefile = "gfx/interface/finance_test/header.dds" }
spriteType = { name = "GFX_tab_diplomacy_bg" texturefile = "gfx/interface/finance_test/tab_bg.dds" noOfFrames = 3 }
spriteType = { name = "GFX_tab_intel_ledger" texturefile = "gfx/interface/finance_test/tab_selected.dds" noOfFrames = 3 }
spriteType = { name = "GFX_diplo_actions_bg" texturefile = "gfx/interface/finance_test/actions.dds" }
spriteType = { name = "GFX_diplo_relations_bg" texturefile = "gfx/interface/finance_test/relations.dds" }
spriteType = { name = "GFX_decision_category_header_bg" texturefile = "gfx/interface/finance_test/decision_header.dds" }
spriteType = { name = "GFX_diplo_countrylist_entry" texturefile = "gfx/interface/finance_test/list_entry.dds" }
spriteType = { name = "GFX_decision_item_bg" texturefile = "gfx/interface/finance_test/decision_item.dds" }
spriteType = { name = "GFX_production_progressbar_frame2" texturefile = "gfx/interface/finance_test/progress_frame.dds" }
spriteType = { name = "GFX_button_123x34" texturefile = "gfx/interface/finance_test/button.dds" noOfFrames = 4 }
spriteType = { name = "GFX_closebutton" texturefile = "gfx/interface/finance_test/close.dds" noOfFrames = 3 }
spriteType = { name = "GFX_resources_strip" texturefile = "gfx/interface/finance_test/resources.dds" noOfFrames = 6 }
progressbartype = {
    name = "GFX_prod_progress_bar3"
    textureFile1 = "gfx/interface/finance_test/progress_fg.dds"
    textureFile2 = "gfx/interface/finance_test/progress_bg.dds"
}
"#,
    )
}

fn sample_data() -> FinancePanelData {
    FinancePanelData {
        cash_rm: 1_250_000_000.0,
        reserve_gbp: 92_000_000.0,
        gold_kg: 4_800.0,
        daily_income_rm: 85_000_000.0,
        daily_expense_rm: 110_000_000.0,
        budget_breakdown: BudgetBreakdownData {
            income_taxes_rm: 64_000_000.0,
            income_pop_taxes_rm: 30_000_000.0,
            income_consumption_taxes_rm: 0.0,
            income_corporate_taxes_rm: 12_000_000.0,
            income_trade_tariffs_rm: 9_000_000.0,
            income_state_profit_rm: 13_000_000.0,
            income_domestic_bonds_rm: 20_000_000.0,
            income_other_rm: 1_000_000.0,
            expense_state_payroll_rm: 18_000_000.0,
            expense_military_wages_rm: 12_000_000.0,
            expense_military_procurement_rm: 32_000_000.0,
            expense_military_maintenance_rm: 10_000_000.0,
            expense_construction_goods_rm: 20_000_000.0,
            expense_construction_wages_rm: 6_000_000.0,
            expense_welfare_rm: 0.0,
            expense_debt_interest_rm: 16_000_000.0,
            expense_foreign_currency_rm: 7_000_000.0,
            expense_mefo_forced_payment_rm: 0.0,
            expense_research_rm: 5_000_000.0,
            expense_other_rm: 2_000_000.0,
        },
        financing_breakdown: FinancingBreakdownData {
            mefo_issued_rm: 18_000_000.0,
            mefo_interest_capitalized_rm: 2_000_000.0,
            domestic_bond_issued_rm: 20_000_000.0,
        },
        fiscal_revenue: FiscalRevenueBreakdownData {
            pop_income_taxes_rm: 30_000_000.0,
            consumption_taxes_rm: 0.0,
            corporate_taxes_rm: 12_000_000.0,
            trade_tariffs_rm: 9_000_000.0,
            state_profit_rm: 13_000_000.0,
            financing_rm: 20_000_000.0,
            other_rm: 1_000_000.0,
        },
        fiscal_expense: FiscalExpenseBreakdownData {
            military_rm: 54_000_000.0,
            construction_rm: 26_000_000.0,
            welfare_rm: 0.0,
            administration_rm: 18_000_000.0,
            interest_rm: 16_000_000.0,
            foreign_exchange_rm: 7_000_000.0,
            research_rm: 5_000_000.0,
            other_rm: 2_000_000.0,
        },
        construction_funding: ConstructionFundingTraceData {
            government_rm: 26_000_000.0,
            mefo_rm: 8_000_000.0,
            private_pool_rm: 11_000_000.0,
            cartel_pool_rm: 3_000_000.0,
            overlord_investment_rm: 0.0,
            foreign_investment_rm: 2_000_000.0,
            paid_rm: 21_000_000.0,
            remaining_rm: 9_000_000.0,
            active_projects: 7,
        },
        investment_pool: InvestmentPoolData {
            total_rm: 330_000_000.0,
            private_rm: 210_000_000.0,
            cartel_rm: 54_000_000.0,
            state_development_bank_rm: 26_000_000.0,
            colonial_extraction_rm: 14_000_000.0,
            foreign_capital_rm: 26_000_000.0,
            income_rm: 8_000_000.0,
            spent_rm: 5_000_000.0,
        },
        operating_income_rm: 65_000_000.0,
        operating_expense_rm: 110_000_000.0,
        original_deficit_rm: 45_000_000.0,
        mefo_coverage_rm: 18_000_000.0,
        post_financing_cash_change_rm: -7_000_000.0,
        public_debt_gbp: 320_000_000.0,
        public_debt_rm: 1_600_000_000.0,
        mefo_debt_rm: 420_000_000.0,
        mefo_military_budget_rm: 600_000_000.0,
        mefo_military_spent_rm: 180_000_000.0,
        credit_rating: "BBB".to_owned(),
        bond_interest_rate: 0.048,
        gdp_rm: 4_000_000_000.0,
        gdp_gbp: 800_000_000.0,
        domestic_gdp_rm: 3_200_000_000.0,
        domestic_gdp_gbp: 640_000_000.0,
        colonial_gdp_rm: 800_000_000.0,
        colonial_gdp_gbp: 160_000_000.0,
        colonial_extracted_value_rm: 90_000_000.0,
        colonial_extracted_value_gbp: 18_000_000.0,
        gdp_breakdown: GdpBreakdownData {
            building_primary_rm: 420_000_000.0,
            building_secondary_rm: 1_100_000_000.0,
            building_tertiary_rm: 680_000_000.0,
            pop_income_rm: 780_000_000.0,
            pop_consumption_rm: 430_000_000.0,
            government_services_rm: 270_000_000.0,
            military_procurement_rm: 190_000_000.0,
            net_exports_rm: 80_000_000.0,
            colonial_value_added_rm: 50_000_000.0,
            historical_validation_gbp: 795_000_000.0,
            historical_validation_error_ratio: 0.006,
        },
        sector_buildings: vec![
            EconomySectorBuildingEntry {
                sector_id: "primary".to_owned(),
                sector_name: "一产".to_owned(),
                building_name: "农场".to_owned(),
                level: 8,
                employed: 42_000,
                demand: 48_000,
                employment_rate: 0.875,
                value_added_rm: 120_000_000.0,
            },
            EconomySectorBuildingEntry {
                sector_id: "primary".to_owned(),
                sector_name: "一产".to_owned(),
                building_name: "矿山".to_owned(),
                level: 5,
                employed: 22_000,
                demand: 24_000,
                employment_rate: 0.916,
                value_added_rm: 90_000_000.0,
            },
            EconomySectorBuildingEntry {
                sector_id: "secondary".to_owned(),
                sector_name: "二产".to_owned(),
                building_name: "钢厂".to_owned(),
                level: 9,
                employed: 58_000,
                demand: 60_000,
                employment_rate: 0.966,
                value_added_rm: 310_000_000.0,
            },
            EconomySectorBuildingEntry {
                sector_id: "secondary".to_owned(),
                sector_name: "二产".to_owned(),
                building_name: "军工厂".to_owned(),
                level: 11,
                employed: 74_000,
                demand: 80_000,
                employment_rate: 0.925,
                value_added_rm: 360_000_000.0,
            },
            EconomySectorBuildingEntry {
                sector_id: "tertiary".to_owned(),
                sector_name: "三产".to_owned(),
                building_name: "铁路局".to_owned(),
                level: 4,
                employed: 18_000,
                demand: 20_000,
                employment_rate: 0.9,
                value_added_rm: 130_000_000.0,
            },
        ],
        employment_rows: vec![
            EconomyEmploymentEntry {
                label: "全国就业".to_owned(),
                employed: 214_000,
                demand: 232_000,
                employment_rate: 0.922,
                value_added_rm: 1_010_000_000.0,
            },
            EconomyEmploymentEntry {
                label: "短缺岗位".to_owned(),
                employed: 18_000,
                demand: 24_000,
                employment_rate: 0.75,
                value_added_rm: 70_000_000.0,
            },
        ],
        diagnostics: vec![
            EconomyDiagnosticEntry {
                source: "财政".to_owned(),
                status: "关注".to_owned(),
                detail: "赤字扩大".to_owned(),
            },
            EconomyDiagnosticEntry {
                source: "建设".to_owned(),
                status: "排队".to_owned(),
                detail: "资金缺口 9M".to_owned(),
            },
            EconomyDiagnosticEntry {
                source: "外汇".to_owned(),
                status: "正常".to_owned(),
                detail: "储备充足".to_owned(),
            },
        ],
        exchange_rate_rm_per_gbp: 5.0,
        can_print_mefo: true,
        can_issue_foreign_bond: true,
        is_foreign_exchange_control: false,
    }
}

fn build_frame(data: &FinancePanelData, tab: FinanceTab, sector: FinanceSector) -> GuiRuntimeFrame {
    let (doc, registry) = root_registry();
    let root = doc
        .template_index()
        .get(COUNTRY_FINANCE_ROOT)
        .expect("country finance root");
    let input = FinanceVanillaInput::new(data.clone(), tab, sector);
    let viewport = GuiRect::new(0.0, 0.0, 1920.0, 1080.0);
    hoi4_ui::finance_panel::finance_vanilla_runtime_frame_parts(
        root,
        &input,
        viewport,
        GuiRuntimeState::shown(viewport),
        registry,
        Some(&finance_gfx_index()),
        None,
    )
    .frame
}

fn generated_role_count(frame: &GuiRuntimeFrame, role: &str) -> usize {
    frame
        .generated_instances
        .iter()
        .filter(|instance| instance.context.semantic_role.as_deref() == Some(role))
        .count()
}

fn generated_model_keys(frame: &GuiRuntimeFrame, role: &str) -> Vec<String> {
    frame
        .generated_instances
        .iter()
        .filter(|instance| instance.context.semantic_role.as_deref() == Some(role))
        .filter_map(|instance| instance.context.model_key.clone())
        .collect()
}

#[test]
fn finance_vanilla_gate25_regression_summary() {
    let data = sample_data();
    let profile = FinanceVanillaProfile;
    let doc = finance_doc();
    let root = doc
        .template_index()
        .get(COUNTRY_FINANCE_ROOT)
        .expect("country finance root");

    for node in [
        "finance_title",
        "close_button",
        "kpi_strip",
        "tab_bar",
        "tab_body_overview",
        "tab_body_budget",
        "tab_body_debt",
        "tab_body_exchange",
        "tab_body_economy",
        "tab_body_funding",
    ] {
        assert!(root.find_node_by_name(node).is_some(), "missing {node}");
    }
    for template in [
        "finance_budget_row",
        "finance_gdp_row",
        "finance_sector_row",
        "finance_employment_row",
        "finance_diagnostic_row",
    ] {
        assert!(
            doc.template_index().get(template).is_some(),
            "missing template {template}"
        );
    }

    let references: BTreeSet<_> = collect_gfx_references(&doc).into_iter().collect();
    assert!(!references.is_empty());
    for reference in &references {
        assert!(reference.starts_with("GFX_"));
        assert!(!reference.starts_with(FORBIDDEN_FINANCE_SPRITE_PREFIX));
        assert!(
            FINANCE_REQUIRED_SPRITES.contains(&reference.as_str()),
            "GUI reference {reference} missing from FINANCE_REQUIRED_SPRITES"
        );
    }

    let required: BTreeSet<_> = COUNTRY_FINANCE_DESCRIPTOR
        .required_sprites
        .iter()
        .copied()
        .collect();
    assert!(required.contains("GFX_closebutton"));
    assert!(required.contains("GFX_prod_progress_bar3"));
    assert!(required.contains("GFX_button_123x34"));
    assert!(required.contains("GFX_tab_intel_ledger"));

    for active in FinanceTab::ALL {
        let input = FinanceVanillaInput::new(data.clone(), active, FinanceSector::Primary);
        let mut visible = Vec::new();
        for tab in FinanceTab::ALL {
            let body_name = format!("tab_body_{}", tab.id());
            let binding = profile.bind_node(
                &GuiNodePath::root(COUNTRY_FINANCE_ROOT).child(body_name),
                &input,
            );
            if binding.visible == Some(true) {
                visible.push(tab);
            }

            let hit = profile.bind_node(
                &GuiNodePath::root(COUNTRY_FINANCE_ROOT)
                    .child(format!("tab_{}", tab.id()))
                    .child("hit"),
                &input,
            );
            assert_eq!(
                hit.click.as_ref().map(|click| click.command.as_str()),
                Some(tab.tab_click_command().as_str())
            );
        }
        assert_eq!(visible, vec![active]);
    }

    let overview = build_frame(&data, FinanceTab::Overview, FinanceSector::Primary);
    assert_eq!(generated_role_count(&overview, "finance_diagnostic_row"), 3);
    assert_eq!(overview.generated_instances.len(), 3);

    let budget = build_frame(&data, FinanceTab::Budget, FinanceSector::Primary);
    assert_eq!(generated_role_count(&budget, "finance_budget_row"), 13);
    assert_eq!(budget.generated_instances.len(), 13);

    let economy_primary = build_frame(&data, FinanceTab::Economy, FinanceSector::Primary);
    assert_eq!(generated_role_count(&economy_primary, "finance_gdp_row"), 9);
    assert_eq!(
        generated_role_count(&economy_primary, "finance_sector_row"),
        2
    );
    assert_eq!(
        generated_role_count(&economy_primary, "finance_employment_row"),
        2
    );
    assert_eq!(
        generated_model_keys(&economy_primary, "finance_sector_row"),
        vec!["0".to_owned(), "1".to_owned()]
    );

    let economy_secondary = build_frame(&data, FinanceTab::Economy, FinanceSector::Secondary);
    assert_eq!(
        generated_model_keys(&economy_secondary, "finance_sector_row"),
        vec!["2".to_owned(), "3".to_owned()]
    );

    let input = FinanceVanillaInput::new(data.clone(), FinanceTab::Debt, FinanceSector::Primary);
    let click = |command: &str| {
        profile.handle_action(
            GuiAction {
                node_path: GuiNodePath::root(command),
                kind: GuiActionKind::Click,
            },
            &input,
        )
    };
    assert_eq!(
        click("finance:action:issue_domestic"),
        Some(FinanceCommand::IssueDomesticBond {
            amount_rm: 500_000_000.0
        })
    );
    assert_eq!(
        click("finance:action:issue_foreign"),
        Some(FinanceCommand::IssueForeignBond {
            amount_gbp: 100_000_000.0
        })
    );
    assert_eq!(
        click("finance:action:print_mefo"),
        Some(FinanceCommand::PrintMefo)
    );
    assert_eq!(
        click("finance:action:sell_gold"),
        Some(FinanceCommand::SellGold { kg: 480.0 })
    );
    assert_eq!(
        click("finance:action:buy_forex"),
        Some(FinanceCommand::BuyForeignCurrency {
            gbp_amount: 10_000_000.0
        })
    );
    assert_eq!(
        click("finance:goto:trade"),
        Some(FinanceCommand::Panel(PanelCommand::OpenPrimary(
            ActivePrimaryPanel::Trade
        )))
    );
    assert_eq!(
        click("finance:goto:logistics"),
        Some(FinanceCommand::Panel(PanelCommand::OpenPrimary(
            ActivePrimaryPanel::Logistics
        )))
    );
    assert_eq!(
        click("finance:goto:construction"),
        Some(FinanceCommand::Panel(PanelCommand::OpenPrimary(
            ActivePrimaryPanel::Construction
        )))
    );
    assert_eq!(
        click("finance:goto:debt"),
        Some(FinanceCommand::Panel(PanelCommand::OpenDetail(
            ActiveDetailPanel::FinanceDebt
        )))
    );

    for tab in FinanceTab::ALL {
        assert_eq!(click(&tab.tab_click_command()), None);
    }
    for sector in FinanceSector::ALL {
        assert_eq!(click(&sector.sector_click_command()), None);
    }

    let mut report = String::from("# Gate 25 Finance Regression Summary\n\n");
    let _ = writeln!(report, "- gui_references: {:?}", references);
    let _ = writeln!(
        report,
        "- required_sprites: {:?}",
        COUNTRY_FINANCE_DESCRIPTOR.required_sprites
    );
    let _ = writeln!(
        report,
        "- overview_instances: {}",
        overview.generated_instances.len()
    );
    let _ = writeln!(
        report,
        "- budget_instances: {}",
        budget.generated_instances.len()
    );
    let _ = writeln!(
        report,
        "- economy_primary_sector_keys: {:?}",
        generated_model_keys(&economy_primary, "finance_sector_row")
    );
    let _ = writeln!(
        report,
        "- economy_secondary_sector_keys: {:?}",
        generated_model_keys(&economy_secondary, "finance_sector_row")
    );
    report.push_str("- tab_and_sector_commands: intercepted by show path; profile emits None\n");
    write_report(report_path("gate25_regression_summary.md"), report);
}

#[test]
fn finance_vanilla_gate26_no_vanilla_assets_committed_audit() {
    let gui = include_str!("../assets/interface/countryfinanceview.gui");
    assert!(!gui.contains(FORBIDDEN_FINANCE_SPRITE_PREFIX));
    assert!(!gui.contains("assets/vanilla_gui"));

    let doc = finance_doc();
    let references = collect_gfx_references(&doc);
    assert!(references.iter().all(|name| name.starts_with("GFX_")));
    assert!(references
        .iter()
        .all(|name| !name.starts_with(FORBIDDEN_FINANCE_SPRITE_PREFIX)));

    let assets_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets");
    let mut forbidden_assets = Vec::new();
    collect_forbidden_asset_files(&assets_root, &mut forbidden_assets);
    assert!(
        forbidden_assets.is_empty(),
        "vanilla binary/gfx assets must not be committed: {forbidden_assets:?}"
    );

    let mut report = String::from("# Gate 26 Finance Asset Audit\n\n");
    let _ = writeln!(report, "- gfx_references: {:?}", references);
    let _ = writeln!(report, "- forbidden_assets: {:?}", forbidden_assets);
    report.push_str(
        "- countryfinanceview.gui is project-owned text and only references vanilla GFX names.\n",
    );
    write_report(report_path("gate26_asset_audit.md"), report);
}

fn collect_forbidden_asset_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name == "vanilla_gui")
            {
                continue;
            }
            collect_forbidden_asset_files(&path, out);
            continue;
        }
        let extension = path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(str::to_ascii_lowercase);
        if matches!(extension.as_deref(), Some("dds" | "gfx")) {
            out.push(path);
        }
    }
}

#[test]
fn finance_vanilla_gate27_visual_layout_smoke() {
    let data = sample_data();
    let overview = build_frame(&data, FinanceTab::Overview, FinanceSector::Primary);
    assert_eq!(
        overview.root_layout.rect,
        GuiRect::new(-6.0, 78.0, 550.0, 1080.0)
    );
    assert!(overview.root_layout.find_by_name("kpi_strip").is_some());
    assert!(overview.root_layout.find_by_name("tab_bar").is_some());
    assert!(overview.root_layout.find_by_name("debt_bar").is_some());
    assert!(overview.root_layout.find_by_name("mefo_bar").is_some());
    assert!(overview
        .hit_regions
        .iter()
        .any(|hit| hit.command == "close" && hit.enabled));

    let overview_resources: BTreeSet<_> = overview
        .draw_list
        .iter()
        .filter_map(|command| command.resource_name().map(str::to_owned))
        .collect();
    for expected in [
        "GFX_tiled_paper_bg2",
        "GFX_closebutton",
        "GFX_win_header_short",
        "GFX_tab_intel_ledger",
    ] {
        assert!(
            overview_resources.contains(expected),
            "missing resource {expected}: {overview_resources:?}"
        );
    }
    assert!(overview.root_layout.find_by_name("panel_frame").is_some());
    let overview_progress = overview
        .draw_list
        .iter()
        .filter(|command| matches!(command.kind, GuiDrawCommandKind::DrawProgress(_)))
        .count();
    assert!(overview_progress >= 2);

    let budget = build_frame(&data, FinanceTab::Budget, FinanceSector::Primary);
    assert_non_overlapping_rows_by_parent(&budget, "finance_budget_row");
    assert!(budget
        .draw_list
        .iter()
        .any(|command| { command.resource_name() == Some("GFX_diplo_countrylist_entry") }));

    let debt = build_frame(&data, FinanceTab::Debt, FinanceSector::Primary);
    for command in [
        "finance:action:issue_domestic",
        "finance:action:issue_foreign",
        "finance:action:print_mefo",
    ] {
        assert!(
            debt.hit_regions
                .iter()
                .any(|hit| hit.command == command && hit.enabled),
            "missing enabled debt action {command}"
        );
    }
    assert!(debt
        .draw_list
        .iter()
        .any(|command| command.resource_name() == Some("GFX_button_123x34")));

    let economy = build_frame(&data, FinanceTab::Economy, FinanceSector::Primary);
    assert_non_overlapping_rows_by_parent(&economy, "finance_gdp_row");
    assert_non_overlapping_rows_by_parent(&economy, "finance_sector_row");
    assert_non_overlapping_rows_by_parent(&economy, "finance_employment_row");
    assert!(economy
        .hit_regions
        .iter()
        .any(|hit| hit.command == "finance:sector:primary" && hit.enabled));

    let text_overflows: Vec<_> = economy
        .diagnostics
        .nodes
        .iter()
        .filter(|node| {
            node.name
                .as_deref()
                .is_some_and(|name| matches!(name, "building" | "label" | "amount" | "percent"))
                && (node.hit_rect.width <= 0.0 || node.hit_rect.height <= 0.0)
        })
        .collect();
    assert!(text_overflows.is_empty(), "{text_overflows:?}");

    let mut report = String::from("# Gate 27 Finance Visual Layout Smoke\n\n");
    let _ = writeln!(report, "- root_rect: {:?}", overview.root_layout.rect);
    let _ = writeln!(report, "- overview_resources: {:?}", overview_resources);
    let _ = writeln!(report, "- overview_progress_bars: {overview_progress}");
    let _ = writeln!(
        report,
        "- budget_rows: {}",
        generated_role_count(&budget, "finance_budget_row")
    );
    let _ = writeln!(
        report,
        "- economy_rows: gdp={} sector={} employment={}",
        generated_role_count(&economy, "finance_gdp_row"),
        generated_role_count(&economy, "finance_sector_row"),
        generated_role_count(&economy, "finance_employment_row")
    );
    let _ = writeln!(
        report,
        "- fallback_draw_commands: overview={} budget={} debt={} economy={}",
        overview.diagnostics.draw_commands.fallback,
        budget.diagnostics.draw_commands.fallback,
        debt.diagnostics.draw_commands.fallback,
        economy.diagnostics.draw_commands.fallback
    );
    report.push_str("- visual_check: root shell, tabs, progress bars, list rows, and action buttons use GUI/runtime slots without row overlap.\n");
    write_report(report_path("gate27_visual_layout_smoke.md"), report);
}

fn assert_non_overlapping_rows_by_parent(frame: &GuiRuntimeFrame, role: &str) {
    let mut by_parent: BTreeMap<String, Vec<_>> = BTreeMap::new();
    for instance in frame
        .generated_instances
        .iter()
        .filter(|instance| instance.context.semantic_role.as_deref() == Some(role))
    {
        by_parent
            .entry(instance.parent_path.to_string())
            .or_default()
            .push(instance.resolved_rect);
    }

    assert!(!by_parent.is_empty(), "no generated rows for {role}");
    for (parent, rows) in by_parent.iter_mut() {
        rows.sort_by(|a, b| {
            a.y.partial_cmp(&b.y)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal))
        });
        for pair in rows.windows(2) {
            let a = pair[0];
            let b = pair[1];
            assert!(
                b.y >= a.y + a.height - 0.5 || b.x >= a.x + a.width - 0.5,
                "{role} rows overlap in {parent}: {a:?} then {b:?}"
            );
        }
    }
}

#[test]
fn finance_vanilla_snapshot() {
    let Some(context) = country_finance_runtime_context() else {
        write_report(
            report_path("gate28_snapshot_1080p.md"),
            "# Gate 28 Finance 1080p Snapshot\n\n- skipped: HOI4 vanilla runtime unavailable\n",
        );
        return;
    };

    let sample = sample_data();
    let path_cfg = context.path_cfg.clone();
    let mut harness = Harness::builder()
        .with_size(Vec2::new(1920.0, 1080.0))
        .build(move |ctx| {
            let _ = apply_vanilla_theme(ctx);
            let mut icon_bank = IconBank::new(ctx.clone(), path_cfg.clone());
            let _ = FinancePanel::show_with_icon_bank(ctx, &sample, &mut icon_bank);
        });

    harness.run_steps(40);
    snapshot_if_enabled(&mut harness, "finance_vanilla_1080p");

    let snapshot_exists = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/snapshots/finance_vanilla_1080p.png")
        .exists();
    let mut report = String::from("# Gate 28 Finance 1080p Snapshot\n\n");
    let _ = writeln!(report, "- runtime_available: true");
    let _ = writeln!(report, "- snapshot: {FINANCE_VANILLA_SNAPSHOT_1080P}");
    let _ = writeln!(report, "- snapshot_exists: {snapshot_exists}");
    report.push_str("- note: snapshot is project test output; vanilla assets are loaded only from the local install at runtime.\n");
    write_report(report_path("gate28_snapshot_1080p.md"), report);

    assert!(
        snapshot_exists,
        "{FINANCE_VANILLA_SNAPSHOT_1080P} missing; run with UPDATE_SNAPSHOTS=1 to refresh it"
    );
}
