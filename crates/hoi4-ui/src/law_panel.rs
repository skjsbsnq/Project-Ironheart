//! V6 law panel UI: six law categories, cooldown display, and PP cost.
//! V6.A acceptance: panel opens, cooldown display switches, modifiers are not wired yet.
use crate::i18n::tr;
use egui::Color32;

use hoi4_state::LawCategory;
use std::fmt::Write as _;
use std::sync::OnceLock;

fn category_label(cat: &LawCategory) -> &'static str {
    match cat {
        LawCategory::Conscription => tr("v6_law_conscription"),
        LawCategory::Economy => tr("v6_law_economy"),
        LawCategory::Trade => tr("v6_law_trade"),
        LawCategory::Taxation => tr("v6_law_taxation"),
        LawCategory::CivilRights => tr("v6_law_civil_rights"),
        LawCategory::InformationControl => tr("v6_law_information_control"),
    }
}

pub fn law_category_label(cat: &LawCategory) -> &'static str {
    category_label(cat)
}

pub fn law_category_key(cat: LawCategory) -> &'static str {
    match cat {
        LawCategory::Conscription => "Conscription",
        LawCategory::Economy => "Economy",
        LawCategory::Trade => "Trade",
        LawCategory::Taxation => "Taxation",
        LawCategory::CivilRights => "CivilRights",
        LawCategory::InformationControl => "InformationControl",
    }
}

pub fn law_category_from_key(key: &str) -> Option<LawCategory> {
    match key {
        "Conscription" | "conscription" => Some(LawCategory::Conscription),
        "Economy" | "economy" => Some(LawCategory::Economy),
        "Trade" | "trade" => Some(LawCategory::Trade),
        "Taxation" | "taxation" => Some(LawCategory::Taxation),
        "CivilRights" | "civil_rights" | "civilRights" => Some(LawCategory::CivilRights),
        "InformationControl" | "information_control" | "informationControl" => {
            Some(LawCategory::InformationControl)
        }
        _ => None,
    }
}

#[derive(Debug, Clone)]
pub struct LawTierEntry {
    pub id: String,
    pub name: String,
    pub pp_cost: u32,
    pub cooldown_days: u16,
    pub effects: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct LawSlotEntry {
    pub category: LawCategory,
    pub current_id: String,
    pub current_name: String,
    pub cooldown_days: u16,
    pub pending: Option<(String, String, u16)>,
    pub is_locked: bool,
    pub locked_reason: Option<String>,
    pub previous_before_lock: Option<String>,
    pub tiers: Vec<LawTierEntry>,
}

#[derive(Debug, Clone)]
pub struct LawPanelData {
    pub political_power: f32,
    pub slots: Vec<LawSlotEntry>,
}

pub const LAW_PANEL_GUI_FILE: &str = "interface/countrypoliticsview.gui";
pub const LAW_PANEL_PROFILE_ID: &str = "law_panel";
pub const LAW_PANEL_DEFAULT_ROUTE: &str = "vanilla_profile";
pub const LAW_PANEL_SAFE_FALLBACK_ROUTE: &str = "safe_project_style";
pub const LAW_PANEL_IRON_DEBUG_FALLBACK_ROUTE: &str = "hoi4_iron_debug";
pub const LAW_PANEL_IRON_FALLBACK_ENV: &str = "IRONHEART_LAW_PANEL_IRON_FALLBACK";
pub const LAW_PANEL_NEW_UI_STYLE_RULE: &str =
    "new_or_modified_law_panel_ui_must_use_vanilla_or_hoi4_iron_assets";
pub const LAW_PANEL_MAIN_ROOT_TEMPLATE: &str = "countrypoliticsview";
pub const LAW_PANEL_POPUP_ROOT_TEMPLATE: &str = "political_ideas_window";
pub const LAW_PANEL_IDEA_CATEGORY_TEMPLATE: &str = "country_politics_idea_category_entry";
pub const LAW_PANEL_IDEA_SLOT_TEMPLATE: &str = "political_idea_entry";
pub const LAW_PANEL_SELECTABLE_ENTRY_GRID_TEMPLATE: &str = "political_selectable_idea_entry_grid";
pub const LAW_PANEL_SELECTABLE_ENTRY_LIST_TEMPLATE: &str = "political_selectable_idea_entry_list";
pub const LAW_PANEL_CATEGORY_GRID_NODE: &str = "idea_categories_grid";
pub const LAW_PANEL_TIER_LIST_NODE: &str = "box_list";
const LAW_PANEL_DIAGNOSTICS_ONCE_ID: &str = "law_panel_vanilla_diagnostics_logged";

const LAW_PANEL_TEMPLATE_INSTANCES: &[crate::vanilla_gui::VanillaTemplateInstance] = &[
    crate::vanilla_gui::VanillaTemplateInstance {
        template_name: LAW_PANEL_IDEA_CATEGORY_TEMPLATE,
        count: 1,
    },
    crate::vanilla_gui::VanillaTemplateInstance {
        template_name: LAW_PANEL_IDEA_SLOT_TEMPLATE,
        count: 1,
    },
    crate::vanilla_gui::VanillaTemplateInstance {
        template_name: LAW_PANEL_SELECTABLE_ENTRY_LIST_TEMPLATE,
        count: 1,
    },
];

pub const LAW_PANEL_TEMPLATES: &[&str] = &[
    LAW_PANEL_MAIN_ROOT_TEMPLATE,
    LAW_PANEL_POPUP_ROOT_TEMPLATE,
    LAW_PANEL_IDEA_CATEGORY_TEMPLATE,
    LAW_PANEL_IDEA_SLOT_TEMPLATE,
    LAW_PANEL_SELECTABLE_ENTRY_GRID_TEMPLATE,
    LAW_PANEL_SELECTABLE_ENTRY_LIST_TEMPLATE,
];

pub const LAW_PANEL_SPRITES: &[&str] = &[
    "GFX_tiled_plain_bg",
    "GFX_tiled_window2_1b_border",
    "GFX_tiled_generic_overlay_bg1_small",
    "GFX_header_bg",
    "GFX_pol_view_bg",
    "GFX_category_header",
    "GFX_idea_categories",
    "GFX_add_pol_idea_button",
    "GFX_idea_entry_bg_2",
    "GFX_idea_entry_bg_3",
    "GFX_idea_traits_strip",
    "GFX_ongoing_generic_glow_yellow",
    "GFX_placeholder_bordered",
    "GFX_closebutton",
    "GFX_pol_power_icon",
    "GFX_idea_disarmed_nation",
    "GFX_idea_volunteer_only",
    "GFX_idea_limited_conscription",
    "GFX_idea_extensive_conscription",
    "GFX_idea_service_by_requirement",
    "GFX_idea_all_adults_serve",
    "GFX_idea_scraping_the_barrel",
    "GFX_idea_civilian_economy",
    "GFX_idea_war_economy",
    "GFX_idea_generic_central_management",
    "GFX_idea_free_trade",
    "GFX_idea_export_focus",
    "GFX_idea_limited_exports",
    "GFX_idea_closed_economy",
    "GFX_idea_ETH_taxed_nobility",
    "GFX_idea_RAJ_taxes",
    "GFX_idea_generic_constitutional_guarantees",
    "GFX_idea_generic_oppression",
    "GFX_idea_generic_secret_police",
    "GFX_idea_JAP_press_association_idea",
    "GFX_idea_generic_spy_political",
    "GFX_idea_ARG_anti_american_propaganda",
];

pub const LAW_PANEL_LAW_SPRITES: &[&str] = &[
    "GFX_law_volunteer_only",
    "GFX_law_limited_conscription",
    "GFX_law_extensive_conscription",
    "GFX_law_total_mobilization",
    "GFX_law_laissez_faire",
    "GFX_law_interventionism",
    "GFX_law_war_economy",
    "GFX_law_corporatist_war_economy",
    "GFX_law_planned_economy",
    "GFX_law_free_trade",
    "GFX_law_export_focus",
    "GFX_law_import_substitution",
    "GFX_law_autarky",
    "GFX_law_state_trade_monopoly",
    "GFX_law_low_taxation",
    "GFX_law_medium_taxation",
    "GFX_law_high_taxation",
    "GFX_law_war_taxation",
    "GFX_law_open_society",
    "GFX_law_limited_rights",
    "GFX_law_national_security_act",
    "GFX_law_police_state",
    "GFX_law_free_press",
    "GFX_law_regulated_press",
    "GFX_law_state_media",
    "GFX_law_total_propaganda",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LawPanelVanillaResource {
    pub role: &'static str,
    pub source_path: &'static str,
    pub template: Option<&'static str>,
    pub sprite: Option<&'static str>,
    pub texture_path: Option<&'static str>,
    pub fallback: &'static str,
}

pub const LAW_PANEL_VANILLA_RESOURCES: &[LawPanelVanillaResource] = &[
    LawPanelVanillaResource {
        role: "gui roots and reusable templates",
        source_path: LAW_PANEL_GUI_FILE,
        template: Some(LAW_PANEL_POPUP_ROOT_TEMPLATE),
        sprite: None,
        texture_path: None,
        fallback: "use the HOI4 iron law panel shell until the vanilla profile is available",
    },
    LawPanelVanillaResource {
        role: "popup background tile",
        source_path: "interface/countrytechtreeview.gfx",
        template: Some(LAW_PANEL_POPUP_ROOT_TEMPLATE),
        sprite: Some("GFX_tiled_plain_bg"),
        texture_path: Some("gfx/interface/tiles/tiled_plain_bg.dds"),
        fallback: "paint the project panel background color and keep text/buttons interactive",
    },
    LawPanelVanillaResource {
        role: "scroll list border",
        source_path: "interface/core.gfx",
        template: Some(LAW_PANEL_POPUP_ROOT_TEMPLATE),
        sprite: Some("GFX_tiled_window2_1b_border"),
        texture_path: Some("gfx/interface/tiles/tiled_window2_1b_border.dds"),
        fallback: "use GFX_tiled_window_1b_border or the HOI4 iron bordered surface",
    },
    LawPanelVanillaResource {
        role: "scroll list overlay",
        source_path: "interface/core.gfx",
        template: Some(LAW_PANEL_POPUP_ROOT_TEMPLATE),
        sprite: Some("GFX_tiled_generic_overlay_bg1_small"),
        texture_path: Some("gfx/interface/tiles/tiled_generic_overlay_bg1_small.dds"),
        fallback: "omit the overlay and keep the bordered list surface",
    },
    LawPanelVanillaResource {
        role: "politics title strip",
        source_path: "interface/general_stuff.gfx",
        template: Some(LAW_PANEL_MAIN_ROOT_TEMPLATE),
        sprite: Some("GFX_header_bg"),
        texture_path: Some("gfx/interface/header_bg.dds"),
        fallback: "draw title text directly on the popup background",
    },
    LawPanelVanillaResource {
        role: "politics panel background",
        source_path: "interface/countrypoliticsview.gfx",
        template: Some(LAW_PANEL_MAIN_ROOT_TEMPLATE),
        sprite: Some("GFX_pol_view_bg"),
        texture_path: Some("gfx/interface/pol_view_bg.dds"),
        fallback: "reuse GFX_tiled_plain_bg for a safe shell background",
    },
    LawPanelVanillaResource {
        role: "idea category row header",
        source_path: "interface/countrypoliticsview.gfx",
        template: Some(LAW_PANEL_IDEA_CATEGORY_TEMPLATE),
        sprite: Some("GFX_category_header"),
        texture_path: Some("gfx/interface/category_header.dds"),
        fallback: "draw a compact category row with the project stroke and text color",
    },
    LawPanelVanillaResource {
        role: "idea category icon strip",
        source_path: "interface/countrypoliticsview.gfx",
        template: Some(LAW_PANEL_IDEA_CATEGORY_TEMPLATE),
        sprite: Some("GFX_idea_categories"),
        texture_path: Some("gfx/interface/idea_categories.dds"),
        fallback: "use the law category color strip from the current panel",
    },
    LawPanelVanillaResource {
        role: "idea slot button",
        source_path: "interface/countrypoliticsview.gfx",
        template: Some(LAW_PANEL_IDEA_SLOT_TEMPLATE),
        sprite: Some("GFX_add_pol_idea_button"),
        texture_path: Some("gfx/interface/add_pol_idea_button.dds"),
        fallback: "use the HOI4 iron idea-slot frame while preserving click commands",
    },
    LawPanelVanillaResource {
        role: "two-column selectable idea row",
        source_path: "interface/countrypoliticsview.gfx",
        template: Some(LAW_PANEL_SELECTABLE_ENTRY_GRID_TEMPLATE),
        sprite: Some("GFX_idea_entry_bg_2"),
        texture_path: Some("gfx/interface/idea_entry_bg_2.dds"),
        fallback: "use the list row template or the HOI4 iron tier row background",
    },
    LawPanelVanillaResource {
        role: "single-column selectable idea row",
        source_path: "interface/countrypoliticsview.gfx",
        template: Some(LAW_PANEL_SELECTABLE_ENTRY_LIST_TEMPLATE),
        sprite: Some("GFX_idea_entry_bg_3"),
        texture_path: Some("gfx/interface/idea_entry_bg_3.dds"),
        fallback: "draw a fixed-height tier row with project primitives",
    },
    LawPanelVanillaResource {
        role: "idea trait marker strip",
        source_path: "interface/countrypoliticsview.gfx",
        template: Some(LAW_PANEL_IDEA_SLOT_TEMPLATE),
        sprite: Some("GFX_idea_traits_strip"),
        texture_path: Some("gfx/interface/idea_traits_strip.dds"),
        fallback: "hide the trait strip; law effects remain in text/tooltips",
    },
    LawPanelVanillaResource {
        role: "pending or available attention glow",
        source_path: "interface/countrypoliticsview.gfx",
        template: Some(LAW_PANEL_IDEA_SLOT_TEMPLATE),
        sprite: Some("GFX_ongoing_generic_glow_yellow"),
        texture_path: Some("gfx/interface/ongoing_generic_glow_yellow.dds"),
        fallback: "use the current warning accent stroke",
    },
    LawPanelVanillaResource {
        role: "law icon placeholder",
        source_path: "interface/debug.gfx",
        template: Some(LAW_PANEL_SELECTABLE_ENTRY_LIST_TEMPLATE),
        sprite: Some("GFX_placeholder_bordered"),
        texture_path: Some("gfx/interface/placeholder_bordered.dds"),
        fallback: "use the project generated placeholder square",
    },
    LawPanelVanillaResource {
        role: "close button",
        source_path: "interface/general_stuff.gfx",
        template: Some(LAW_PANEL_POPUP_ROOT_TEMPLATE),
        sprite: Some("GFX_closebutton"),
        texture_path: Some("gfx/interface/closebutton.dds"),
        fallback: "use a small project close button with the same close command",
    },
    LawPanelVanillaResource {
        role: "political power icon",
        source_path: "interface/topbar.gfx",
        template: None,
        sprite: Some("GFX_pol_power_icon"),
        texture_path: Some("gfx/interface/pol_power_icon.dds"),
        fallback: "show the existing PP text label without an icon",
    },
];

#[derive(Debug, Clone)]
pub struct LawPanelVanillaContext {
    pub path_cfg: hoi4_paths::PathConfig,
    pub document: crate::vanilla_gui::GuiDocument,
    pub gfx_index: crate::vanilla_gui::GfxIndex,
    pub diagnostics: LawPanelVanillaDiagnostics,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LawPanelVanillaDiagnostics {
    pub route: &'static str,
    pub fallback_reason: Option<String>,
    pub path_source: Option<hoi4_paths::PathSource>,
    pub gui_file: &'static str,
    pub gui_loaded: bool,
    pub missing_gui_files: Vec<String>,
    pub missing_templates: Vec<String>,
    pub sprite_hit_report: crate::vanilla_gui::GfxHitReport,
    pub loaded_gfx_files: usize,
}

impl LawPanelVanillaDiagnostics {
    pub fn fallback(reason: impl Into<String>) -> Self {
        Self {
            route: LAW_PANEL_SAFE_FALLBACK_ROUTE,
            fallback_reason: Some(reason.into()),
            path_source: None,
            gui_file: LAW_PANEL_GUI_FILE,
            gui_loaded: false,
            missing_gui_files: vec![LAW_PANEL_GUI_FILE.to_owned()],
            missing_templates: LAW_PANEL_TEMPLATES
                .iter()
                .map(|name| (*name).to_owned())
                .collect(),
            sprite_hit_report: crate::vanilla_gui::GfxHitReport {
                requested: LAW_PANEL_SPRITES.len(),
                hits: 0,
                missing: LAW_PANEL_SPRITES
                    .iter()
                    .map(|name| (*name).to_owned())
                    .collect(),
            },
            loaded_gfx_files: 0,
        }
    }

    pub fn uses_fallback(&self) -> bool {
        self.fallback_reason.is_some()
    }

    pub fn summary_line(&self) -> String {
        let fallback = self.fallback_reason.as_deref().unwrap_or("none");
        format!(
            "[ui][law_panel] route={} gui_loaded={} missing_gui={:?} templates_missing={:?} sprites={}/{} missing_sprites={:?} fallback_reason={}",
            self.route,
            self.gui_loaded,
            self.missing_gui_files,
            self.missing_templates,
            self.sprite_hit_report.hits,
            self.sprite_hit_report.requested,
            self.sprite_hit_report.missing,
            fallback
        )
    }

    pub fn report(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(out, "# Law Panel Vanilla GUI Diagnostics");
        let _ = writeln!(out, "- route: {}", self.route);
        let _ = writeln!(out, "- fallback: {}", self.uses_fallback());
        let _ = writeln!(
            out,
            "- fallback_reason: {}",
            self.fallback_reason.as_deref().unwrap_or("<none>")
        );
        let _ = writeln!(out, "- gui_file: {}", self.gui_file);
        let _ = writeln!(out, "- gui_loaded: {}", self.gui_loaded);
        let _ = writeln!(out, "- missing_gui_files: {:?}", self.missing_gui_files);
        let _ = writeln!(out, "- missing_templates: {:?}", self.missing_templates);
        let _ = writeln!(
            out,
            "- sprites: {}/{} hit",
            self.sprite_hit_report.hits, self.sprite_hit_report.requested
        );
        let _ = writeln!(
            out,
            "- missing_sprites: {:?}",
            self.sprite_hit_report.missing
        );
        let _ = writeln!(out, "- loaded_gfx_files: {}", self.loaded_gfx_files);
        if let Some(source) = self.path_source {
            let _ = writeln!(out, "- path_source: {:?}", source);
        }
        out
    }
}

fn law_panel_vanilla_context() -> Option<&'static LawPanelVanillaContext> {
    static CACHE: OnceLock<Option<LawPanelVanillaContext>> = OnceLock::new();
    CACHE
        .get_or_init(|| {
            let path_cfg = hoi4_paths::PathConfig::resolve(Default::default()).ok()?;
            law_panel_vanilla_context_from_path_config(&path_cfg).ok()
        })
        .as_ref()
}

pub fn warm_law_panel_runtime(icon_bank: &mut crate::icons::IconBank) {
    let _ = law_panel_vanilla_context();
    icon_bank.add_politics_search_dirs();
    let _ = icon_bank.diagnose_sprites(LAW_PANEL_SPRITES.iter().copied());
}

pub fn law_panel_vanilla_context_from_path_config(
    path_cfg: &hoi4_paths::PathConfig,
) -> Result<LawPanelVanillaContext, LawPanelVanillaDiagnostics> {
    let Some(gui_path) = path_cfg.find(LAW_PANEL_GUI_FILE) else {
        let mut diagnostics = LawPanelVanillaDiagnostics::fallback(format!(
            "missing required gui file `{LAW_PANEL_GUI_FILE}` under {}",
            path_cfg.game_path().display()
        ));
        diagnostics.path_source = Some(path_cfg.source());
        return Err(diagnostics);
    };

    let document = crate::vanilla_gui::parse_gui_file(&gui_path).map_err(|err| {
        let mut diagnostics = LawPanelVanillaDiagnostics::fallback(format!(
            "failed to parse `{LAW_PANEL_GUI_FILE}`: {err}"
        ));
        diagnostics.path_source = Some(path_cfg.source());
        diagnostics.missing_gui_files.clear();
        diagnostics
    })?;
    let template_index = document.template_index();
    let missing_templates: Vec<String> = LAW_PANEL_TEMPLATES
        .iter()
        .copied()
        .filter(|template| template_index.get(template).is_none())
        .map(str::to_owned)
        .collect();
    let gfx_index = crate::vanilla_gui::GfxIndex::from_path_config(path_cfg);
    let sprite_hit_report = gfx_index.hit_report(LAW_PANEL_SPRITES.iter().copied());
    let loaded_gfx_files = gfx_index.diagnostics().loaded_gfx_files;
    let fallback_reason = if !missing_templates.is_empty() {
        Some(format!(
            "missing required templates: {}",
            missing_templates.join(", ")
        ))
    } else if !sprite_hit_report.all_hit() {
        Some(format!(
            "missing required sprites: {}",
            sprite_hit_report.missing.join(", ")
        ))
    } else {
        None
    };
    let diagnostics = LawPanelVanillaDiagnostics {
        route: if fallback_reason.is_some() {
            LAW_PANEL_SAFE_FALLBACK_ROUTE
        } else {
            LAW_PANEL_DEFAULT_ROUTE
        },
        fallback_reason,
        path_source: Some(path_cfg.source()),
        gui_file: LAW_PANEL_GUI_FILE,
        gui_loaded: true,
        missing_gui_files: Vec::new(),
        missing_templates,
        sprite_hit_report,
        loaded_gfx_files,
    };

    if diagnostics.uses_fallback() {
        Err(diagnostics)
    } else {
        Ok(LawPanelVanillaContext {
            path_cfg: path_cfg.clone(),
            document,
            gfx_index,
            diagnostics,
        })
    }
}

pub fn law_panel_vanilla_diagnostics_report(
    path_cfg: &hoi4_paths::PathConfig,
) -> LawPanelVanillaDiagnostics {
    match law_panel_vanilla_context_from_path_config(path_cfg) {
        Ok(context) => context.diagnostics,
        Err(diagnostics) => diagnostics,
    }
}

fn law_panel_runtime_diagnostics() -> LawPanelVanillaDiagnostics {
    match hoi4_paths::PathConfig::resolve(Default::default()) {
        Ok(path_cfg) => law_panel_vanilla_diagnostics_report(&path_cfg),
        Err(err) => {
            LawPanelVanillaDiagnostics::fallback(format!("HOI4 path resolution failed: {err}"))
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum LawCommand {
    SwitchLaw {
        category: LawCategory,
        target_law_id: String,
    },
    Blocked {
        reason: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum LawPanelProfileCommand {
    SelectCategory {
        category: LawCategory,
    },
    OpenLawDetail {
        category: LawCategory,
        law_id: Option<String>,
    },
    SwitchLaw {
        category: LawCategory,
        target_law_id: String,
    },
    Blocked {
        reason: String,
    },
    Close,
}

pub struct LawPanelProfile;

impl LawPanelProfile {
    pub fn bind_node_with_selection(
        &self,
        node_path: &crate::vanilla_gui::GuiNodePath,
        data: &LawPanelData,
        selected_category: Option<LawCategory>,
    ) -> crate::vanilla_gui::GuiBinding {
        bind_law_panel_profile_node(node_path, data, selected_category)
    }

    pub fn handle_click_command(
        &self,
        command: &str,
        data: &LawPanelData,
    ) -> Option<LawPanelProfileCommand> {
        law_panel_profile_command_from_click(command, data)
    }
}

impl crate::vanilla_gui::VanillaPanelProfile for LawPanelProfile {
    type Data = LawPanelData;
    type Command = LawPanelProfileCommand;

    fn profile_id(&self) -> &'static str {
        LAW_PANEL_PROFILE_ID
    }

    fn root_template(&self) -> &'static str {
        LAW_PANEL_POPUP_ROOT_TEMPLATE
    }

    fn required_gui_files(&self) -> &'static [&'static str] {
        &[LAW_PANEL_GUI_FILE]
    }

    fn template_instances(&self) -> &'static [crate::vanilla_gui::VanillaTemplateInstance] {
        LAW_PANEL_TEMPLATE_INSTANCES
    }

    fn bind_node(
        &self,
        node_path: &crate::vanilla_gui::GuiNodePath,
        data: &Self::Data,
    ) -> crate::vanilla_gui::GuiBinding {
        self.bind_node_with_selection(node_path, data, None)
    }

    fn handle_action(
        &self,
        action: crate::vanilla_gui::GuiAction,
        data: &Self::Data,
    ) -> Option<Self::Command> {
        if action.kind != crate::vanilla_gui::GuiActionKind::Click {
            return None;
        }
        law_panel_profile_command_from_path(&action.node_path.to_string(), data)
    }
}

fn bind_law_panel_profile_node(
    node_path: &crate::vanilla_gui::GuiNodePath,
    data: &LawPanelData,
    selected_category: Option<LawCategory>,
) -> crate::vanilla_gui::GuiBinding {
    let path = node_path.to_string();
    let name = node_path.0.last().map(String::as_str).unwrap_or_default();

    bind_law_panel_shell_node(name, data, selected_category)
        .or_else(|| bind_law_panel_category_node(&path, name, data))
        .or_else(|| bind_law_panel_current_slot_node(&path, name, data))
        .or_else(|| bind_law_panel_tier_node(&path, name, data, selected_category))
        .unwrap_or_default()
}

fn bind_law_panel_shell_node(
    name: &str,
    data: &LawPanelData,
    selected_category: Option<LawCategory>,
) -> Option<crate::vanilla_gui::GuiBinding> {
    match name {
        "title" => {
            let title = law_panel_selected_slot(data, selected_category)
                .map(|slot| category_label(&slot.category).to_owned())
                .unwrap_or_else(|| tr("v6_law_panel_title").to_owned());
            Some(crate::vanilla_gui::GuiBinding::default().text(title))
        }
        "switch" => {
            let text = law_panel_selected_slot(data, selected_category)
                .map(|slot| {
                    format!(
                        "{} | {}: {:.0}",
                        slot.current_name,
                        tr("political_power"),
                        data.political_power
                    )
                })
                .unwrap_or_else(|| {
                    format!("{}: {:.0}", tr("political_power"), data.political_power)
                });
            Some(crate::vanilla_gui::GuiBinding::default().text(text))
        }
        LAW_PANEL_TIER_LIST_NODE => Some(
            crate::vanilla_gui::GuiBinding::default()
                .instances(law_panel_selected_tier_count(data, selected_category)),
        ),
        "close" | "close_button" => Some(
            crate::vanilla_gui::GuiBinding::default()
                .tooltip(tr("panel_close_hint"))
                .click("law_panel:close"),
        ),
        _ => None,
    }
}

fn bind_law_panel_category_node(
    path: &str,
    name: &str,
    data: &LawPanelData,
) -> Option<crate::vanilla_gui::GuiBinding> {
    if name == LAW_PANEL_CATEGORY_GRID_NODE {
        return Some(crate::vanilla_gui::GuiBinding::default().instances(data.slots.len()));
    }

    let slot_idx = law_panel_category_index_from_path(path)?;
    let slot = data.slots.get(slot_idx)?;
    match name {
        LAW_PANEL_IDEA_CATEGORY_TEMPLATE => Some(
            crate::vanilla_gui::GuiBinding::default()
                .tooltip(law_panel_category_tooltip(slot))
                .click(law_panel_category_click_command(slot.category)),
        ),
        "name" => {
            Some(crate::vanilla_gui::GuiBinding::default().text(category_label(&slot.category)))
        }
        "category_icon" => Some(
            crate::vanilla_gui::GuiBinding::default()
                .sprite("GFX_idea_categories")
                .frame(law_panel_category_frame(slot.category)),
        ),
        "ideas_grid" => Some(crate::vanilla_gui::GuiBinding::default().instances(1)),
        _ => None,
    }
}

fn bind_law_panel_current_slot_node(
    path: &str,
    name: &str,
    data: &LawPanelData,
) -> Option<crate::vanilla_gui::GuiBinding> {
    if !path.contains(LAW_PANEL_IDEA_SLOT_TEMPLATE) {
        return None;
    }
    let slot_idx = law_panel_category_index_from_path(path)?;
    let slot = data.slots.get(slot_idx)?;
    match name {
        LAW_PANEL_IDEA_SLOT_TEMPLATE | "add_idea_button" => Some(
            crate::vanilla_gui::GuiBinding::default()
                .sprite(law_panel_law_sprite(slot.category, &slot.current_id))
                .text(&slot.current_name)
                .tooltip(law_panel_current_law_tooltip(slot))
                .click(law_panel_detail_click_command(
                    slot.category,
                    Some(&slot.current_id),
                )),
        ),
        "idea_alert_glow" | "idea_traits" => {
            Some(crate::vanilla_gui::GuiBinding::default().visible(false))
        }
        _ => None,
    }
}

fn bind_law_panel_tier_node(
    path: &str,
    name: &str,
    data: &LawPanelData,
    selected_category: Option<LawCategory>,
) -> Option<crate::vanilla_gui::GuiBinding> {
    let tier_idx = law_panel_tier_index_from_path(path)?;
    let slot = law_panel_selected_slot(data, selected_category)?;
    let tier = slot.tiers.get(tier_idx)?;
    if path.contains(LAW_PANEL_SELECTABLE_ENTRY_LIST_TEMPLATE)
        && (name == LAW_PANEL_SELECTABLE_ENTRY_LIST_TEMPLATE
            || name.starts_with(&format!("{LAW_PANEL_SELECTABLE_ENTRY_LIST_TEMPLATE}[")))
    {
        return Some(
            crate::vanilla_gui::GuiBinding::default()
                .tooltip(law_tier_tooltip(slot, tier, data.political_power))
                .click(law_panel_switch_click_command(slot.category, &tier.id)),
        );
    }
    match name {
        "idea_entry_bg" => Some(
            crate::vanilla_gui::GuiBinding::default()
                .tooltip(law_tier_tooltip(slot, tier, data.political_power))
                .click(law_panel_switch_click_command(slot.category, &tier.id)),
        ),
        "name" => Some(crate::vanilla_gui::GuiBinding::default().text(&tier.name)),
        "cost" => {
            Some(crate::vanilla_gui::GuiBinding::default().text(format!("{} PP", tier.pp_cost)))
        }
        "stats" => Some(crate::vanilla_gui::GuiBinding::default().text(law_panel_tier_stats(tier))),
        "traits" => {
            Some(crate::vanilla_gui::GuiBinding::default().text(law_panel_tier_badge(slot, tier)))
        }
        "icon" | "idea_icon" => Some(
            crate::vanilla_gui::GuiBinding::default()
                .sprite(law_panel_law_sprite(slot.category, &tier.id)),
        ),
        _ => None,
    }
}

pub fn law_panel_selected_tier_count(
    data: &LawPanelData,
    selected_category: Option<LawCategory>,
) -> usize {
    law_panel_selected_slot(data, selected_category)
        .map(|slot| slot.tiers.len())
        .unwrap_or(0)
}

fn law_panel_selected_slot(
    data: &LawPanelData,
    selected_category: Option<LawCategory>,
) -> Option<&LawSlotEntry> {
    selected_category
        .and_then(|category| data.slots.iter().find(|slot| slot.category == category))
        .or_else(|| data.slots.first())
}

fn law_panel_category_index_from_path(path: &str) -> Option<usize> {
    law_panel_template_index_from_path(path, LAW_PANEL_IDEA_CATEGORY_TEMPLATE)
}

fn law_panel_tier_index_from_path(path: &str) -> Option<usize> {
    law_panel_template_index_from_path(path, LAW_PANEL_SELECTABLE_ENTRY_LIST_TEMPLATE).or_else(
        || law_panel_template_index_from_path(path, LAW_PANEL_SELECTABLE_ENTRY_GRID_TEMPLATE),
    )
}

fn law_panel_template_index_from_path(path: &str, template: &str) -> Option<usize> {
    let marker = format!("{template}[");
    let start = path.find(&marker)? + marker.len();
    let end = path[start..].find(']')? + start;
    let raw = &path[start..end];
    raw.split(':').next()?.parse::<usize>().ok()
}

fn law_panel_profile_command_from_path(
    path: &str,
    data: &LawPanelData,
) -> Option<LawPanelProfileCommand> {
    if path.ends_with(".close") || path.ends_with(".close_button") {
        return Some(LawPanelProfileCommand::Close);
    }
    if path.contains(LAW_PANEL_SELECTABLE_ENTRY_LIST_TEMPLATE)
        || path.contains(LAW_PANEL_SELECTABLE_ENTRY_GRID_TEMPLATE)
    {
        let tier_idx = law_panel_tier_index_from_path(path)?;
        let slot = law_panel_selected_slot(data, None)?;
        let tier = slot.tiers.get(tier_idx)?;
        return Some(law_panel_tier_profile_command(
            slot,
            tier,
            data.political_power,
        ));
    }
    if path.contains(LAW_PANEL_IDEA_SLOT_TEMPLATE) {
        let slot_idx = law_panel_category_index_from_path(path)?;
        let slot = data.slots.get(slot_idx)?;
        return Some(LawPanelProfileCommand::OpenLawDetail {
            category: slot.category,
            law_id: Some(slot.current_id.clone()),
        });
    }
    if path.contains(LAW_PANEL_IDEA_CATEGORY_TEMPLATE) {
        let slot_idx = law_panel_category_index_from_path(path)?;
        let slot = data.slots.get(slot_idx)?;
        return Some(LawPanelProfileCommand::SelectCategory {
            category: slot.category,
        });
    }
    None
}

fn law_panel_profile_command_from_click(
    command: &str,
    data: &LawPanelData,
) -> Option<LawPanelProfileCommand> {
    let mut parts = command.split(':');
    match (parts.next(), parts.next(), parts.next(), parts.next()) {
        (Some("law_panel"), Some("close"), None, None) => Some(LawPanelProfileCommand::Close),
        (Some("law_panel"), Some("category"), Some(category), None) => {
            let category = law_category_from_key(category)?;
            data.slots
                .iter()
                .any(|slot| slot.category == category)
                .then_some(LawPanelProfileCommand::SelectCategory { category })
        }
        (Some("law_panel"), Some("detail"), Some(category), law_id) => {
            let category = law_category_from_key(category)?;
            data.slots
                .iter()
                .any(|slot| slot.category == category)
                .then(|| LawPanelProfileCommand::OpenLawDetail {
                    category,
                    law_id: law_id.map(str::to_owned),
                })
        }
        (Some("law_panel"), Some("switch"), Some(category), Some(law_id)) => {
            let category = law_category_from_key(category)?;
            let slot = data.slots.iter().find(|slot| slot.category == category)?;
            let tier = slot.tiers.iter().find(|tier| tier.id == law_id)?;
            Some(law_panel_tier_profile_command(
                slot,
                tier,
                data.political_power,
            ))
        }
        _ => None,
    }
}

fn law_panel_tier_profile_command(
    slot: &LawSlotEntry,
    tier: &LawTierEntry,
    political_power: f32,
) -> LawPanelProfileCommand {
    if let Some(command) = law_switch_command(slot, tier, political_power) {
        match command {
            LawCommand::SwitchLaw {
                category,
                target_law_id,
            } => LawPanelProfileCommand::SwitchLaw {
                category,
                target_law_id,
            },
            LawCommand::Blocked { reason } => LawPanelProfileCommand::Blocked { reason },
        }
    } else {
        LawPanelProfileCommand::Blocked {
            reason: law_unavailable_reason(slot, tier, political_power)
                .unwrap_or_else(|| tr("locked").to_owned()),
        }
    }
}

fn law_panel_category_click_command(category: LawCategory) -> String {
    format!("law_panel:category:{}", law_category_key(category))
}

fn law_panel_detail_click_command(category: LawCategory, law_id: Option<&str>) -> String {
    match law_id {
        Some(law_id) => format!("law_panel:detail:{}:{law_id}", law_category_key(category)),
        None => format!("law_panel:detail:{}", law_category_key(category)),
    }
}

fn law_panel_switch_click_command(category: LawCategory, law_id: &str) -> String {
    format!("law_panel:switch:{}:{law_id}", law_category_key(category))
}

fn law_panel_category_frame(category: LawCategory) -> u32 {
    match category {
        LawCategory::Conscription => 1,
        LawCategory::Economy => 2,
        LawCategory::Trade => 3,
        LawCategory::Taxation => 4,
        LawCategory::CivilRights => 5,
        LawCategory::InformationControl => 6,
    }
}

pub fn law_panel_law_sprite(category: LawCategory, law_id: &str) -> &'static str {
    let normalized = normalize_law_sprite_key(law_id);
    law_panel_law_sprite_for_key(category, &normalized)
}

pub fn law_panel_law_sprite_from_name(category: LawCategory, law_name: &str) -> &'static str {
    let normalized = normalize_law_sprite_key(law_name);
    law_panel_law_sprite_for_key(category, &normalized)
}

fn law_panel_law_sprite_for_key(category: LawCategory, normalized: &str) -> &'static str {
    match category {
        LawCategory::Conscription => match normalized {
            "volunteeronly" | "volunteer" => "GFX_law_volunteer_only",
            "limitedconscription" => "GFX_law_limited_conscription",
            "extensiveconscription" => "GFX_law_extensive_conscription",
            "totalmobilization" => "GFX_law_total_mobilization",
            _ => "GFX_law_volunteer_only",
        },
        LawCategory::Economy => match normalized {
            "laissezfaire" | "civilianeconomy" => "GFX_law_laissez_faire",
            "interventionism" | "partialmobilization" => "GFX_law_interventionism",
            "wareconomy" => "GFX_law_war_economy",
            "corporatistwareconomy" => "GFX_law_corporatist_war_economy",
            "plannedeconomy" => "GFX_law_planned_economy",
            _ => "GFX_law_laissez_faire",
        },
        LawCategory::Trade => match normalized {
            "freetrade" => "GFX_law_free_trade",
            "exportfocus" => "GFX_law_export_focus",
            "importsubstitution" | "limitedexports" => "GFX_law_import_substitution",
            "autarky" | "closedeconomy" => "GFX_law_autarky",
            "statetrademonopoly" => "GFX_law_state_trade_monopoly",
            _ => "GFX_law_free_trade",
        },
        LawCategory::Taxation => match normalized {
            "lowtaxation" => "GFX_law_low_taxation",
            "mediumtaxation" => "GFX_law_medium_taxation",
            "hightaxation" => "GFX_law_high_taxation",
            "wartaxation" => "GFX_law_war_taxation",
            _ => "GFX_law_medium_taxation",
        },
        LawCategory::CivilRights => match normalized {
            "opensociety" => "GFX_law_open_society",
            "limitedrights" => "GFX_law_limited_rights",
            "nationalsecurityact" => "GFX_law_national_security_act",
            "policestate" => "GFX_law_police_state",
            _ => "GFX_law_limited_rights",
        },
        LawCategory::InformationControl => match normalized {
            "freepress" => "GFX_law_free_press",
            "regulatedpress" => "GFX_law_regulated_press",
            "statemedia" => "GFX_law_state_media",
            "totalpropaganda" => "GFX_law_total_propaganda",
            _ => "GFX_law_regulated_press",
        },
    }
}

fn normalize_law_sprite_key(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn law_panel_category_tooltip(slot: &LawSlotEntry) -> String {
    format!(
        "{}\n{}: {}",
        category_label(&slot.category),
        tr("current"),
        slot.current_name
    )
}

fn law_panel_current_law_tooltip(slot: &LawSlotEntry) -> String {
    let mut lines = vec![format!(
        "{}\n{}: {}",
        category_label(&slot.category),
        tr("current"),
        slot.current_name
    )];
    if let Some((_, target, days)) = &slot.pending {
        lines.push(format!(
            "{}: {} ({} days)",
            tr("v6_law_pending"),
            target,
            days
        ));
    } else if slot.cooldown_days > 0 {
        lines.push(format!("{}: {} days", tr("cooldown"), slot.cooldown_days));
    }
    if let Some(reason) = &slot.locked_reason {
        lines.push(reason.clone());
    }
    lines.join("\n")
}

pub fn law_tier_tooltip(slot: &LawSlotEntry, tier: &LawTierEntry, political_power: f32) -> String {
    let mut lines = vec![
        tier.name.clone(),
        format!("{} PP", tier.pp_cost),
        law_panel_tier_state(slot, tier, political_power),
    ];
    if let Some((_, target, days)) = &slot.pending {
        lines.push(format!(
            "{}: {target} ({} days)",
            tr("v6_law_pending"),
            days
        ));
    }
    if slot.cooldown_days > 0 {
        lines.push(format!("{}: {} days", tr("cooldown"), slot.cooldown_days));
    }
    if slot.is_locked {
        lines.push(
            slot.locked_reason
                .clone()
                .unwrap_or_else(|| "该法律类别当前被锁定。".to_owned()),
        );
    }
    if political_power < tier.pp_cost as f32 {
        lines.push(format!("政治力量不足，需要 {} PP。", tier.pp_cost));
    }
    lines.extend(tier.effects.iter().cloned());
    lines.join("\n")
}

fn law_panel_tier_stats(tier: &LawTierEntry) -> String {
    if tier.effects.is_empty() {
        return String::new();
    }
    tier.effects
        .iter()
        .take(3)
        .cloned()
        .collect::<Vec<_>>()
        .join("\n")
}

fn law_panel_tier_state(slot: &LawSlotEntry, tier: &LawTierEntry, political_power: f32) -> String {
    if tier.id == slot.current_id {
        tr("current").to_owned()
    } else if slot
        .pending
        .as_ref()
        .map_or(false, |(id, _, _)| id == &tier.id)
    {
        tr("v6_law_pending").to_owned()
    } else if law_switch_available(slot, tier, political_power) {
        tr("available").to_owned()
    } else {
        law_unavailable_reason(slot, tier, political_power)
            .unwrap_or_else(|| tr("locked").to_owned())
    }
}

fn law_panel_tier_badge(slot: &LawSlotEntry, tier: &LawTierEntry) -> String {
    if tier.id == slot.current_id {
        tr("current").to_owned()
    } else if slot
        .pending
        .as_ref()
        .map_or(false, |(id, _, _)| id == &tier.id)
    {
        tr("v6_law_pending").to_owned()
    } else {
        String::new()
    }
}

pub fn law_switch_available(
    slot: &LawSlotEntry,
    tier: &LawTierEntry,
    political_power: f32,
) -> bool {
    let is_pending = slot
        .pending
        .as_ref()
        .map_or(false, |(id, _, _)| id == &tier.id);
    tier.id != slot.current_id
        && !is_pending
        && slot.cooldown_days == 0
        && !slot.is_locked
        && political_power >= tier.pp_cost as f32
}

pub fn law_switch_command(
    slot: &LawSlotEntry,
    tier: &LawTierEntry,
    political_power: f32,
) -> Option<LawCommand> {
    law_switch_available(slot, tier, political_power).then(|| LawCommand::SwitchLaw {
        category: slot.category,
        target_law_id: tier.id.clone(),
    })
}

pub fn law_unavailable_reason(
    slot: &LawSlotEntry,
    tier: &LawTierEntry,
    political_power: f32,
) -> Option<String> {
    if law_switch_available(slot, tier, political_power) {
        return None;
    }
    let is_pending = slot
        .pending
        .as_ref()
        .map_or(false, |(id, _, _)| id == &tier.id);
    let has_pp = political_power >= tier.pp_cost as f32;
    Some(disabled_law_reason(slot, tier, is_pending, has_pp))
}

pub struct LawPanel;

impl LawPanel {
    pub fn show(ctx: &egui::Context, data: &LawPanelData) -> (bool, Vec<LawCommand>) {
        Self::show_for_category(ctx, data, None)
    }

    pub fn show_with_icon_bank(
        ctx: &egui::Context,
        data: &LawPanelData,
        icon_bank: &mut crate::icons::IconBank,
    ) -> (bool, Vec<LawCommand>) {
        Self::show_for_category_inner(ctx, data, None, Some(icon_bank))
    }

    pub fn show_for_category(
        ctx: &egui::Context,
        data: &LawPanelData,
        initial_category: Option<LawCategory>,
    ) -> (bool, Vec<LawCommand>) {
        Self::show_for_category_inner(ctx, data, initial_category, None)
    }

    pub fn show_for_category_with_icon_bank(
        ctx: &egui::Context,
        data: &LawPanelData,
        initial_category: Option<LawCategory>,
        icon_bank: &mut crate::icons::IconBank,
    ) -> (bool, Vec<LawCommand>) {
        Self::show_for_category_inner(ctx, data, initial_category, Some(icon_bank))
    }

    fn show_for_category_inner(
        ctx: &egui::Context,
        data: &LawPanelData,
        initial_category: Option<LawCategory>,
        icon_bank: Option<&mut crate::icons::IconBank>,
    ) -> (bool, Vec<LawCommand>) {
        if use_iron_law_panel_debug_fallback() {
            log_law_panel_diagnostics_once(
                ctx,
                &LawPanelVanillaDiagnostics {
                    route: LAW_PANEL_IRON_DEBUG_FALLBACK_ROUTE,
                    fallback_reason: Some(format!("forced by {LAW_PANEL_IRON_FALLBACK_ENV}")),
                    ..cached_law_panel_runtime_diagnostics().clone()
                },
            );
            return hoi4_iron_debug_show_law_with_initial_category(
                ctx,
                data,
                initial_category,
                icon_bank,
            );
        }

        let diagnostics = cached_law_panel_runtime_diagnostics();
        log_law_panel_diagnostics_once(ctx, diagnostics);
        if diagnostics.uses_fallback() {
            safe_project_style_show_law(ctx, data, initial_category, icon_bank)
        } else if let Some(icon_bank) = icon_bank {
            vanilla_profile_show_law_with_icon_bank(ctx, data, initial_category, icon_bank)
        } else {
            vanilla_profile_show_law(ctx, data, initial_category)
        }
    }
}

fn cached_law_panel_runtime_diagnostics() -> &'static LawPanelVanillaDiagnostics {
    static CACHE: OnceLock<LawPanelVanillaDiagnostics> = OnceLock::new();
    CACHE.get_or_init(law_panel_runtime_diagnostics)
}

fn log_law_panel_diagnostics_once(ctx: &egui::Context, diagnostics: &LawPanelVanillaDiagnostics) {
    let id = egui::Id::new(LAW_PANEL_DIAGNOSTICS_ONCE_ID);
    let already_logged = ctx
        .data_mut(|d| d.get_persisted::<bool>(id))
        .unwrap_or(false);
    if already_logged {
        return;
    }
    println!("{}", diagnostics.summary_line());
    ctx.data_mut(|d| d.insert_persisted(id, true));
}

pub fn use_iron_law_panel_debug_fallback() -> bool {
    std::env::var(LAW_PANEL_IRON_FALLBACK_ENV)
        .ok()
        .as_deref()
        .map(law_panel_iron_fallback_enabled)
        .unwrap_or(false)
}

pub fn law_panel_iron_fallback_enabled(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on" | "iron" | "hoi4"
    )
}

fn law_panel_close_requested_id() -> egui::Id {
    egui::Id::new("law_panel_vanilla_close_requested")
}

fn law_panel_animation_store_id() -> egui::Id {
    egui::Id::new("vanilla_gui_panel_animation_store")
}

pub fn request_law_panel_close(ctx: &egui::Context) {
    ctx.data_mut(|data| data.insert_persisted(law_panel_close_requested_id(), true));
    ctx.request_repaint();
}

fn update_law_panel_animation(
    ctx: &egui::Context,
    spec: crate::vanilla_gui::AnimationSpec,
) -> crate::vanilla_gui::PanelAnimationUpdate {
    let close_id = law_panel_close_requested_id();
    let store_id = law_panel_animation_store_id();
    let now_ms = ctx.input(|input| input.time * 1000.0);
    let update = ctx.data_mut(|data| {
        let mut store = data
            .get_persisted::<crate::vanilla_gui::PanelAnimationStore>(store_id)
            .unwrap_or_default();
        let mut close_requested = data.get_persisted::<bool>(close_id).unwrap_or(false);
        if close_requested && store.phase(LAW_PANEL_PROFILE_ID).is_none() {
            let _ = store.update(
                LAW_PANEL_PROFILE_ID,
                true,
                spec,
                now_ms - spec.duration_ms as f64,
            );
        } else if close_requested
            && matches!(
                store.phase(LAW_PANEL_PROFILE_ID),
                Some(crate::vanilla_gui::AnimationPhase::Closed)
            )
        {
            close_requested = false;
            data.insert_persisted(close_id, false);
        }
        let update = store.update(LAW_PANEL_PROFILE_ID, !close_requested, spec, now_ms);
        data.insert_persisted(store_id, store);
        update
    });
    if matches!(
        update.phase,
        crate::vanilla_gui::AnimationPhase::Opening | crate::vanilla_gui::AnimationPhase::Closing
    ) {
        ctx.request_repaint();
    }
    update
}

fn vanilla_profile_show_law(
    ctx: &egui::Context,
    data: &LawPanelData,
    initial_category: Option<LawCategory>,
) -> (bool, Vec<LawCommand>) {
    let Some(context) = law_panel_vanilla_context() else {
        return safe_project_style_show_law(ctx, data, initial_category, None);
    };
    let mut icon_bank = crate::icons::IconBank::new(ctx.clone(), context.path_cfg.clone());
    vanilla_profile_show_law_with_icon_bank(ctx, data, initial_category, &mut icon_bank)
}

pub fn vanilla_profile_show_law_with_icon_bank(
    ctx: &egui::Context,
    data: &LawPanelData,
    initial_category: Option<LawCategory>,
    icon_bank: &mut crate::icons::IconBank,
) -> (bool, Vec<LawCommand>) {
    let Some(context) = law_panel_vanilla_context() else {
        return safe_project_style_show_law(ctx, data, initial_category, Some(icon_bank));
    };
    let profile = LawPanelProfile;
    let Some(root) = context
        .document
        .template_index()
        .get(LAW_PANEL_POPUP_ROOT_TEMPLATE)
    else {
        return safe_project_style_show_law(ctx, data, initial_category, Some(icon_bank));
    };

    icon_bank.add_politics_search_dirs();
    let selected_id = egui::Id::new("law_panel_vanilla_selected_category");
    let mut selected_category = persisted_law_panel_selected_category(ctx, data, initial_category);
    let screen = ctx.screen_rect();
    let base_viewport = vanilla_law_panel_layout_viewport(screen, root);
    let shown_layout = crate::vanilla_gui::compute_layout_tree(
        root,
        &crate::vanilla_gui::LayoutOptions::new(base_viewport).shown_position(true),
    );
    let mut spec = crate::vanilla_gui::AnimationSpec::from_node(root);
    spec.hidden_position.x += base_viewport.x - screen.left();
    spec.hidden_position.y += base_viewport.y - screen.top();
    spec.shown_position.x += base_viewport.x - screen.left();
    spec.shown_position.y += base_viewport.y - screen.top();
    let update = update_law_panel_animation(ctx, spec);
    if update.close_finished {
        ctx.data_mut(|data| {
            data.insert_persisted(law_panel_close_requested_id(), false);
        });
        return (true, Vec::new());
    }
    if !update.visible {
        return (false, Vec::new());
    }
    let viewport = crate::vanilla_gui::GuiRect::new(
        screen.left() + update.position.x - shown_layout.rect.x,
        screen.top() + update.position.y - shown_layout.rect.y,
        screen.width(),
        screen.height(),
    );
    let layout = crate::vanilla_gui::compute_layout_tree(
        root,
        &crate::vanilla_gui::LayoutOptions::new(viewport).shown_position(true),
    );

    let mut close = ctx.input(|input| input.key_pressed(egui::Key::Q));
    let mut output = Vec::new();
    let mut stats = crate::vanilla_gui::RenderStats::default();
    let renderer = crate::vanilla_gui::VanillaGuiRenderer::new(&context.gfx_index);

    egui::Area::new(egui::Id::new("law_panel_vanilla_profile_popup"))
        .order(egui::Order::Foreground)
        .fixed_pos(screen.min)
        .show(ctx, |ui| {
            let outer: egui::Rect = layout.rect.into();
            ui.interact(
                outer.intersect(screen),
                ui.id().with("law_panel_vanilla_drag_region"),
                egui::Sense::click_and_drag(),
            );
            let bindings =
                bind_law_profile_tree_with_selection(&profile, root, data, selected_category);
            stats.merge(renderer.paint_tree(ui, root, &layout, &bindings, icon_bank));
            apply_law_panel_render_stats(
                &profile,
                data,
                &stats,
                &mut selected_category,
                &mut close,
                &mut output,
            );

            let bridge_stats = paint_law_panel_template_instance_bridge(
                ui,
                &renderer,
                context,
                &profile,
                data,
                root,
                &layout,
                selected_category,
                icon_bank,
            );
            apply_law_panel_render_stats(
                &profile,
                data,
                &bridge_stats,
                &mut selected_category,
                &mut close,
                &mut output,
            );
            stats.merge(bridge_stats);
        });

    log_law_panel_render_stats(ctx, &stats, icon_bank);
    ctx.data_mut(|d| d.insert_persisted(selected_id, selected_category));
    if close {
        request_law_panel_close(ctx);
    }
    (false, output)
}

fn persisted_law_panel_selected_category(
    ctx: &egui::Context,
    data: &LawPanelData,
    initial_category: Option<LawCategory>,
) -> Option<LawCategory> {
    let selected_id = egui::Id::new("law_panel_vanilla_selected_category");
    let mut selected_category = ctx
        .data_mut(|d| d.get_persisted::<Option<LawCategory>>(selected_id))
        .unwrap_or_else(|| data.slots.first().map(|slot| slot.category));
    if let Some(initial_category) = initial_category {
        let request_id = egui::Id::new("law_panel_vanilla_initial_category_request");
        let request_key = law_category_key(initial_category).to_owned();
        let last_request = ctx.data_mut(|d| d.get_persisted::<String>(request_id));
        if last_request.as_deref() != Some(request_key.as_str()) {
            selected_category = Some(initial_category);
            ctx.data_mut(|d| d.insert_persisted(request_id, request_key));
        }
    }
    if !selected_category
        .map(|category| data.slots.iter().any(|slot| slot.category == category))
        .unwrap_or(false)
    {
        selected_category = data.slots.first().map(|slot| slot.category);
    }
    selected_category
}

fn vanilla_law_panel_layout_viewport(
    screen: egui::Rect,
    root: &crate::vanilla_gui::GuiNode,
) -> crate::vanilla_gui::GuiRect {
    let base = crate::vanilla_gui::GuiRect::new(
        screen.left(),
        screen.top(),
        screen.width(),
        screen.height(),
    );
    let layout = crate::vanilla_gui::compute_layout_tree(
        root,
        &crate::vanilla_gui::LayoutOptions::new(base).shown_position(true),
    );
    let mut x = screen.left();
    let mut y = screen.top();
    let margin = 8.0;
    if layout.rect.right() > screen.right() - margin {
        x -= layout.rect.right() - (screen.right() - margin);
    }
    if layout.rect.x + x < screen.left() + margin {
        x += screen.left() + margin - (layout.rect.x + x);
    }
    if layout.rect.bottom() > screen.bottom() - margin {
        y -= layout.rect.bottom() - (screen.bottom() - margin);
    }
    if layout.rect.y + y < screen.top() + margin {
        y += screen.top() + margin - (layout.rect.y + y);
    }
    crate::vanilla_gui::GuiRect::new(x, y, screen.width(), screen.height())
}

fn bind_law_profile_tree_with_selection(
    profile: &LawPanelProfile,
    root: &crate::vanilla_gui::GuiNode,
    data: &LawPanelData,
    selected_category: Option<LawCategory>,
) -> crate::vanilla_gui::GuiBindingMap {
    let mut bindings = crate::vanilla_gui::GuiBindingMap::default();
    bind_law_profile_node_with_selection(
        profile,
        root,
        data,
        crate::vanilla_gui::GuiNodePath::root(root.path_label(0)),
        selected_category,
        &mut bindings,
    );
    bindings
}

fn bind_law_profile_node_with_selection(
    profile: &LawPanelProfile,
    node: &crate::vanilla_gui::GuiNode,
    data: &LawPanelData,
    path: crate::vanilla_gui::GuiNodePath,
    selected_category: Option<LawCategory>,
    bindings: &mut crate::vanilla_gui::GuiBindingMap,
) {
    bindings.insert_path(
        path.clone(),
        profile.bind_node_with_selection(&path, data, selected_category),
    );
    for (index, child) in node.children.iter().enumerate() {
        bind_law_profile_node_with_selection(
            profile,
            child,
            data,
            path.child(child.path_label(index)),
            selected_category,
            bindings,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn paint_law_panel_template_instance_bridge(
    ui: &mut egui::Ui,
    renderer: &crate::vanilla_gui::VanillaGuiRenderer<'_>,
    context: &LawPanelVanillaContext,
    profile: &LawPanelProfile,
    data: &LawPanelData,
    root: &crate::vanilla_gui::GuiNode,
    root_layout: &crate::vanilla_gui::LayoutNode,
    selected_category: Option<LawCategory>,
    icon_bank: &mut crate::icons::IconBank,
) -> crate::vanilla_gui::RenderStats {
    let mut stats = crate::vanilla_gui::RenderStats::default();
    stats.merge(paint_law_panel_selectable_tier_templates(
        ui,
        renderer,
        context,
        profile,
        data,
        root,
        root_layout,
        selected_category,
        icon_bank,
    ));
    stats
}

#[allow(clippy::too_many_arguments)]
fn paint_law_panel_selectable_tier_templates(
    ui: &mut egui::Ui,
    renderer: &crate::vanilla_gui::VanillaGuiRenderer<'_>,
    context: &LawPanelVanillaContext,
    profile: &LawPanelProfile,
    data: &LawPanelData,
    root: &crate::vanilla_gui::GuiNode,
    root_layout: &crate::vanilla_gui::LayoutNode,
    selected_category: Option<LawCategory>,
    icon_bank: &mut crate::icons::IconBank,
) -> crate::vanilla_gui::RenderStats {
    let Some(grid_node) = root.find_node_by_name(LAW_PANEL_TIER_LIST_NODE) else {
        return crate::vanilla_gui::RenderStats::default();
    };
    let Some(grid_layout) = root_layout.find_by_name(LAW_PANEL_TIER_LIST_NODE) else {
        return crate::vanilla_gui::RenderStats::default();
    };
    let Some(template) = context
        .document
        .template_index()
        .get(LAW_PANEL_SELECTABLE_ENTRY_LIST_TEMPLATE)
    else {
        return crate::vanilla_gui::RenderStats::default();
    };

    let count = law_panel_selected_tier_count(data, selected_category);
    let mut total_stats = crate::vanilla_gui::RenderStats::default();
    for (idx, slot) in crate::vanilla_gui::grid_slots(grid_node, grid_layout.rect, count)
        .into_iter()
        .enumerate()
    {
        let path = grid_layout
            .path
            .clone()
            .child(format!("{LAW_PANEL_SELECTABLE_ENTRY_LIST_TEMPLATE}[{idx}]"));
        let layout = crate::vanilla_gui::compute_layout_tree_with_path(
            template,
            &crate::vanilla_gui::LayoutOptions::new(slot),
            path.clone(),
        );
        let bindings =
            rebind_law_profile_tree_at_path(profile, template, data, selected_category, path);
        let stats = renderer.paint_tree(ui, template, &layout, &bindings, icon_bank);
        total_stats.merge(stats);
    }
    total_stats
}

fn rebind_law_profile_tree_at_path(
    profile: &LawPanelProfile,
    root: &crate::vanilla_gui::GuiNode,
    data: &LawPanelData,
    selected_category: Option<LawCategory>,
    root_path: crate::vanilla_gui::GuiNodePath,
) -> crate::vanilla_gui::GuiBindingMap {
    let mut bindings = crate::vanilla_gui::GuiBindingMap::default();
    bind_law_profile_node_with_selection(
        profile,
        root,
        data,
        root_path,
        selected_category,
        &mut bindings,
    );
    bindings
}

fn apply_law_panel_render_stats(
    profile: &LawPanelProfile,
    data: &LawPanelData,
    stats: &crate::vanilla_gui::RenderStats,
    selected_category: &mut Option<LawCategory>,
    close: &mut bool,
    output: &mut Vec<LawCommand>,
) {
    for command in &stats.clicked_commands {
        let panel_command = if command == "close" {
            Some(LawPanelProfileCommand::Close)
        } else {
            profile.handle_click_command(command, data)
        };
        match panel_command {
            Some(LawPanelProfileCommand::Close) => *close = true,
            Some(LawPanelProfileCommand::SelectCategory { category }) => {
                *selected_category = Some(category);
            }
            Some(LawPanelProfileCommand::OpenLawDetail { category, .. }) => {
                *selected_category = Some(category);
            }
            Some(LawPanelProfileCommand::SwitchLaw {
                category,
                target_law_id,
            }) => output.push(LawCommand::SwitchLaw {
                category,
                target_law_id,
            }),
            Some(LawPanelProfileCommand::Blocked { reason }) => {
                output.push(LawCommand::Blocked { reason });
            }
            None => {}
        }
    }
}

fn log_law_panel_render_stats(
    ctx: &egui::Context,
    stats: &crate::vanilla_gui::RenderStats,
    icon_bank: &crate::icons::IconBank,
) {
    let id = egui::Id::new("law_panel_vanilla_render_stats_logged");
    let already_logged = ctx
        .data_mut(|d| d.get_persisted::<bool>(id))
        .unwrap_or(false);
    if already_logged {
        return;
    }
    println!(
        "[ui][law_panel] vanilla_render nodes={}/{} sprites={} fallback={} text={} buttons={} icon_missing_cache={} fallback_labels={:?}",
        stats.nodes_painted,
        stats.nodes_seen,
        stats.sprites_painted,
        stats.fallback_painted,
        stats.text_painted,
        stats.buttons,
        icon_bank.missing_count(),
        stats.fallback_labels
    );
    ctx.data_mut(|d| d.insert_persisted(id, true));
}

fn safe_project_style_show_law(
    ctx: &egui::Context,
    data: &LawPanelData,
    initial_category: Option<LawCategory>,
    icon_bank: Option<&mut crate::icons::IconBank>,
) -> (bool, Vec<LawCommand>) {
    hoi4_iron_show_law_with_initial_category(ctx, data, initial_category, icon_bank)
}

fn hoi4_iron_show_law_with_initial_category(
    ctx: &egui::Context,
    data: &LawPanelData,
    initial_category: Option<LawCategory>,
    icon_bank: Option<&mut crate::icons::IconBank>,
) -> (bool, Vec<LawCommand>) {
    show_law_panel_project_shell(
        ctx,
        data,
        initial_category,
        "law_panel_hoi4_iron",
        icon_bank,
    )
}

fn hoi4_iron_debug_show_law_with_initial_category(
    ctx: &egui::Context,
    data: &LawPanelData,
    initial_category: Option<LawCategory>,
    icon_bank: Option<&mut crate::icons::IconBank>,
) -> (bool, Vec<LawCommand>) {
    show_law_panel_project_shell(
        ctx,
        data,
        initial_category,
        "law_panel_hoi4_iron_debug",
        icon_bank,
    )
}

fn show_law_panel_project_shell(
    ctx: &egui::Context,
    data: &LawPanelData,
    initial_category: Option<LawCategory>,
    state_scope: &'static str,
    icon_bank: Option<&mut crate::icons::IconBank>,
) -> (bool, Vec<LawCommand>) {
    let selected_id = egui::Id::new((state_scope, "selected_overview_category"));
    let mut selected_category = ctx
        .data_mut(|d| d.get_persisted::<Option<LawCategory>>(selected_id))
        .unwrap_or_else(|| data.slots.first().map(|slot| slot.category));
    if let Some(initial_category) = initial_category {
        let request_id = egui::Id::new((state_scope, "initial_category_request"));
        let request_key = law_category_key(initial_category).to_owned();
        let last_request = ctx.data_mut(|d| d.get_persisted::<String>(request_id));
        if last_request.as_deref() != Some(request_key.as_str()) {
            selected_category = Some(initial_category);
            ctx.data_mut(|d| d.insert_persisted(request_id, request_key));
        }
    }
    if selected_category
        .map(|category| data.slots.iter().any(|slot| slot.category == category))
        .unwrap_or(false)
        == false
    {
        selected_category = data.slots.first().map(|slot| slot.category);
    }

    let screen = ctx.screen_rect();
    let window = law_panel_window_rect(screen);
    let spec = crate::vanilla_gui::AnimationSpec {
        hidden_position: crate::vanilla_gui::GuiPoint {
            x: -window.width() - 24.0,
            y: window.top() - screen.top(),
        },
        shown_position: crate::vanilla_gui::GuiPoint {
            x: window.left() - screen.left(),
            y: window.top() - screen.top(),
        },
        show_curve: crate::vanilla_gui::AnimationCurve::Decelerated,
        hide_curve: crate::vanilla_gui::AnimationCurve::Accelerated,
        duration_ms: 300.0,
    };
    let update = update_law_panel_animation(ctx, spec);
    if update.close_finished {
        ctx.data_mut(|data| {
            data.insert_persisted(law_panel_close_requested_id(), false);
        });
        return (true, Vec::new());
    }
    if !update.visible {
        return (false, Vec::new());
    }
    let window = egui::Rect::from_min_size(
        egui::Pos2::new(
            screen.left() + update.position.x,
            screen.top() + update.position.y,
        ),
        window.size(),
    );
    let mut close = ctx.input(|input| input.key_pressed(egui::Key::Q));
    let mut output = Vec::new();
    let mut owned_icon_bank;
    let mut icon_bank = match icon_bank {
        Some(icon_bank) => Some(icon_bank),
        None => {
            owned_icon_bank = hoi4_paths::PathConfig::resolve(Default::default())
                .ok()
                .map(|path_cfg| {
                    let mut icon_bank = crate::icons::IconBank::new(ctx.clone(), path_cfg);
                    icon_bank.add_politics_search_dirs();
                    icon_bank
                });
            owned_icon_bank.as_mut()
        }
    };

    egui::Area::new(egui::Id::new((state_scope, "popup")))
        .order(egui::Order::Foreground)
        .fixed_pos(window.min)
        .show(ctx, |ui| {
            let (outer, _) = ui.allocate_exact_size(window.size(), egui::Sense::click_and_drag());
            let rects = law_panel_shell_rects(outer);
            paint_law_panel_window_shell(ui, &rects, data);
            if paint_law_panel_close_button(ui, rects.close_button).clicked() {
                close = true;
            }
            hoi4_iron_law_body(
                ui,
                rects.body,
                data,
                &mut selected_category,
                &mut output,
                &mut icon_bank,
            );
            paint_law_panel_footer(ui, rects.footer);
        });

    ctx.data_mut(|d| d.insert_persisted(selected_id, selected_category));
    if close {
        request_law_panel_close(ctx);
    }
    (false, output)
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct LawPanelShellRects {
    outer: egui::Rect,
    title_bar: egui::Rect,
    close_button: egui::Rect,
    summary: egui::Rect,
    body: egui::Rect,
    category_list: egui::Rect,
    tier_list: egui::Rect,
    footer: egui::Rect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LawPanelStatusCounts {
    locked: usize,
    cooling: usize,
    pending: usize,
    affordable: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LawPanelTierVisualState {
    Current,
    Pending,
    Available,
    Locked,
    Cooling,
    NotEnoughPoliticalPower,
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LawPanelTierVisual {
    state: LawPanelTierVisualState,
    accent: Color32,
    clickable: bool,
    status_label: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LawPanelStateMarker {
    key: &'static str,
    label: String,
}

pub const LAW_PANEL_PHASE3_SHELL_RESOURCES: &[&str] = &[
    "GFX_tiled_plain_bg",
    "GFX_tiled_window2_1b_border",
    "GFX_tiled_generic_overlay_bg1_small",
    "GFX_header_bg",
    "GFX_closebutton",
];

pub const LAW_PANEL_PHASE3_CATEGORY_RESOURCES: &[&str] = &[
    "GFX_category_header",
    "GFX_idea_categories",
    "GFX_add_pol_idea_button",
];

pub const LAW_PANEL_PHASE3_TIER_RESOURCES: &[&str] = &[
    "GFX_idea_entry_bg_3",
    "GFX_idea_traits_strip",
    "GFX_ongoing_generic_glow_yellow",
    "GFX_placeholder_bordered",
];

fn law_panel_window_rect(screen: egui::Rect) -> egui::Rect {
    let margin = 18.0;
    let max_w = (screen.width() - margin * 2.0).max(360.0);
    let max_h = (screen.height() - margin * 2.0).max(420.0);
    let desired_w = (screen.width() * 0.70).clamp(760.0, 1040.0).min(max_w);
    let desired_h = (screen.height() * 0.72).clamp(520.0, 720.0).min(max_h);
    let size = egui::Vec2::new(desired_w.max(360.0), desired_h.max(420.0));
    let pos = egui::Pos2::new(
        screen.left() + (screen.width() - size.x) * 0.5,
        screen.top() + (screen.height() - size.y) * 0.5,
    );
    egui::Rect::from_min_size(pos, size)
}

fn law_panel_shell_rects(outer: egui::Rect) -> LawPanelShellRects {
    let margin = if outer.width() < 760.0 { 10.0 } else { 14.0 };
    let title_h = 38.0;
    let summary_h = 38.0;
    let footer_h = 24.0;
    let title_bar = egui::Rect::from_min_size(
        outer.left_top() + egui::Vec2::splat(margin),
        egui::Vec2::new(outer.width() - margin * 2.0, title_h),
    );
    let close_button = egui::Rect::from_min_size(
        egui::Pos2::new(title_bar.right() - 28.0, title_bar.top() + 6.0),
        egui::Vec2::splat(24.0),
    );
    let summary = egui::Rect::from_min_size(
        egui::Pos2::new(title_bar.left(), title_bar.bottom() + 8.0),
        egui::Vec2::new(title_bar.width(), summary_h),
    );
    let footer = egui::Rect::from_min_size(
        egui::Pos2::new(title_bar.left(), outer.bottom() - margin - footer_h),
        egui::Vec2::new(title_bar.width(), footer_h),
    );
    let body = egui::Rect::from_min_max(
        egui::Pos2::new(title_bar.left(), summary.bottom() + 10.0),
        egui::Pos2::new(title_bar.right(), footer.top() - 10.0),
    );
    let (category_list, tier_list) = law_panel_body_rects(body);
    LawPanelShellRects {
        outer,
        title_bar,
        close_button,
        summary,
        body,
        category_list,
        tier_list,
        footer,
    }
}

fn law_panel_body_rects(body: egui::Rect) -> (egui::Rect, egui::Rect) {
    let gutter = if body.width() < 760.0 { 8.0 } else { 12.0 };
    let category_w = (body.width() * 0.34)
        .clamp(220.0, 318.0)
        .min((body.width() - gutter - 300.0).max(190.0));
    let category_list =
        egui::Rect::from_min_size(body.left_top(), egui::Vec2::new(category_w, body.height()));
    let tier_list = egui::Rect::from_min_max(
        egui::Pos2::new(category_list.right() + gutter, body.top()),
        body.right_bottom(),
    );
    (category_list, tier_list)
}

fn law_panel_status_counts(data: &LawPanelData) -> LawPanelStatusCounts {
    LawPanelStatusCounts {
        locked: data.slots.iter().filter(|slot| slot.is_locked).count(),
        cooling: data
            .slots
            .iter()
            .filter(|slot| slot.cooldown_days > 0)
            .count(),
        pending: data
            .slots
            .iter()
            .filter(|slot| slot.pending.is_some())
            .count(),
        affordable: hoi4_iron_law_affordable_count(data),
    }
}

fn law_panel_popup_accent(data: &LawPanelData) -> Color32 {
    let counts = law_panel_status_counts(data);
    if counts.locked > 0 {
        crate::v9::palette::BAD
    } else if counts.pending > 0 || counts.cooling > 0 {
        crate::v9::palette::WARN
    } else {
        crate::v9::palette::GOLD
    }
}

fn paint_law_panel_window_shell(
    ui: &mut egui::Ui,
    rects: &LawPanelShellRects,
    data: &LawPanelData,
) {
    let accent = law_panel_popup_accent(data);
    crate::v9::paint::paint_shadow(
        ui.painter(),
        rects.outer,
        crate::v9::Elevation::E3,
        crate::v9::radius::R1,
    );
    ui.painter().rect_filled(
        rects.outer,
        egui::epaint::CornerRadius::same(1),
        crate::v9::palette::SOOT_BLACK,
    );
    crate::v9::paint::paint_blackened_steel(ui.painter(), rects.outer.shrink(1.0), 1.0);
    crate::v9::paint::paint_bevel(
        ui.painter(),
        rects.outer,
        Color32::TRANSPARENT,
        crate::v9::palette::BRASS_DARK,
        crate::v9::radius::R1,
    );
    paint_law_panel_title_bar(ui, rects.title_bar, accent);
    paint_law_panel_summary_strip(ui, rects.summary, data);
}

fn paint_law_panel_title_bar(ui: &mut egui::Ui, rect: egui::Rect, accent: Color32) {
    ui.painter().rect_filled(
        rect,
        egui::epaint::CornerRadius::same(1),
        crate::v9::palette::IRON_BLACK,
    );
    crate::v9::paint::paint_vertical_gradient_mesh(
        ui.painter(),
        rect,
        Color32::from_rgba_premultiplied(0x42, 0x47, 0x3b, 126),
        Color32::from_rgba_premultiplied(0x05, 0x06, 0x05, 245),
    );
    ui.painter().rect_stroke(
        rect,
        egui::epaint::CornerRadius::same(1),
        egui::Stroke::new(1.0, crate::v9::palette::EDGE_DARK),
        egui::epaint::StrokeKind::Inside,
    );
    ui.painter().hline(
        (rect.left() + 8.0)..=(rect.right() - 8.0),
        rect.bottom() - 1.0,
        egui::Stroke::new(1.0, accent),
    );
    ui.painter().text(
        egui::Pos2::new(rect.left() + 12.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        tr("v6_law_panel_title"),
        crate::v9::TextRole::Heading.font_id(),
        crate::v9::palette::GOLD_HOT,
    );
    ui.painter().text(
        egui::Pos2::new(rect.left() + 156.0, rect.center().y + 1.0),
        egui::Align2::LEFT_CENTER,
        "Political Ideas",
        crate::v9::TextRole::Caption.font_id(),
        crate::v9::palette::PARCHMENT_DIM,
    );
}

fn paint_law_panel_close_button(ui: &mut egui::Ui, rect: egui::Rect) -> egui::Response {
    let response = ui.interact(
        rect,
        ui.id().with("law_panel_phase3_close_button"),
        egui::Sense::click(),
    );
    let fill = if response.is_pointer_button_down_on() {
        crate::v9::palette::IRON_BLACK
    } else if response.hovered() {
        crate::v9::palette::IRON_LIGHT
    } else {
        crate::v9::palette::IRON_DARK
    };
    crate::v9::paint::paint_bevel(
        ui.painter(),
        rect,
        fill,
        if response.hovered() {
            crate::v9::palette::GOLD
        } else {
            crate::v9::palette::EDGE_DARK
        },
        crate::v9::radius::R1,
    );
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        "X",
        crate::v9::TextRole::Caption.font_id(),
        crate::v9::palette::PARCHMENT,
    );
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response.on_hover_text(tr("panel_close_hint"))
}

fn paint_law_panel_summary_strip(ui: &mut egui::Ui, rect: egui::Rect, data: &LawPanelData) {
    let counts = law_panel_status_counts(data);
    paint_law_panel_region(ui.painter(), rect, crate::v9::palette::PANEL_DEEP);
    let items = [
        (
            "PP",
            format!("{:.0}", data.political_power),
            crate::v9::palette::GOLD,
        ),
        (
            "CAT",
            data.slots.len().to_string(),
            crate::v9::palette::INFO,
        ),
        (
            "OK",
            counts.affordable.to_string(),
            if counts.affordable > 0 {
                crate::v9::palette::GOOD
            } else {
                crate::v9::palette::MUTED
            },
        ),
        (
            "CD",
            counts.cooling.to_string(),
            if counts.cooling > 0 {
                crate::v9::palette::WARN
            } else {
                crate::v9::palette::MUTED
            },
        ),
        (
            "LOCK",
            counts.locked.to_string(),
            if counts.locked > 0 {
                crate::v9::palette::BAD
            } else {
                crate::v9::palette::MUTED
            },
        ),
        (
            "PEND",
            counts.pending.to_string(),
            if counts.pending > 0 {
                crate::v9::palette::WARN
            } else {
                crate::v9::palette::MUTED
            },
        ),
    ];
    let gap = 7.0;
    let cell_w = ((rect.width() - gap * (items.len().saturating_sub(1) as f32))
        / items.len() as f32)
        .max(54.0);
    for (idx, (label, value, color)) in items.into_iter().enumerate() {
        let cell = egui::Rect::from_min_size(
            egui::Pos2::new(rect.left() + idx as f32 * (cell_w + gap), rect.top()),
            egui::Vec2::new(cell_w, rect.height()),
        )
        .shrink2(egui::Vec2::new(2.0, 4.0));
        paint_law_panel_status_chip(ui, cell, label, &value, color);
    }
}

fn paint_law_panel_status_chip(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    label: &str,
    value: &str,
    color: Color32,
) {
    crate::v9::paint::paint_bevel(
        ui.painter(),
        rect,
        crate::v9::palette::SOOT_BLACK,
        crate::v9::palette::EDGE_DARK,
        crate::v9::radius::R1,
    );
    let icon = egui::Rect::from_min_size(
        rect.left_center() + egui::Vec2::new(6.0, -6.0),
        egui::Vec2::splat(12.0),
    );
    ui.painter()
        .rect_filled(icon, egui::epaint::CornerRadius::same(1), color);
    ui.painter().rect_stroke(
        icon,
        egui::epaint::CornerRadius::same(1),
        egui::Stroke::new(1.0, Color32::from_black_alpha(180)),
        egui::epaint::StrokeKind::Inside,
    );
    ui.painter().text(
        egui::Pos2::new(icon.right() + 5.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        crate::v9::TextRole::Small.font_id(),
        crate::v9::palette::PARCHMENT_DIM,
    );
    ui.painter().text(
        egui::Pos2::new(rect.right() - 7.0, rect.center().y),
        egui::Align2::RIGHT_CENTER,
        value,
        crate::v9::TextRole::Numeric.font_id(),
        color,
    );
}

fn paint_law_panel_footer(ui: &mut egui::Ui, rect: egui::Rect) {
    ui.painter().rect_filled(
        rect,
        egui::epaint::CornerRadius::same(1),
        Color32::from_black_alpha(96),
    );
    ui.painter().hline(
        (rect.left() + 5.0)..=(rect.right() - 5.0),
        rect.top(),
        egui::Stroke::new(1.0, crate::v9::palette::EDGE_DARK),
    );
    ui.painter().text(
        egui::Pos2::new(rect.left() + 8.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        "Q 关闭 | 选择法律类别 | 可用候选可切换",
        crate::v9::TextRole::Small.font_id(),
        crate::v9::palette::MUTED,
    );
}

fn hoi4_iron_law_body(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    data: &LawPanelData,
    selected_category: &mut Option<LawCategory>,
    cmds: &mut Vec<LawCommand>,
    icon_bank: &mut Option<&mut crate::icons::IconBank>,
) {
    ui.allocate_ui_at_rect(rect, |ui| {
        let (category_list, tier_list) = law_panel_body_rects(rect);
        hoi4_iron_law_group_list(ui, category_list, data, selected_category, icon_bank);
        let selected_slot = selected_category
            .and_then(|category| data.slots.iter().find(|slot| slot.category == category))
            .or_else(|| data.slots.first());
        hoi4_iron_law_impact_preview(
            ui,
            tier_list,
            selected_slot,
            data.political_power,
            cmds,
            icon_bank,
        );
    });
}

fn hoi4_iron_law_group_list(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    data: &LawPanelData,
    selected_category: &mut Option<LawCategory>,
    icon_bank: &mut Option<&mut crate::icons::IconBank>,
) {
    let inner = paint_law_panel_section(ui, rect, "法律类别");
    if data.slots.is_empty() {
        paint_law_panel_empty(ui, inner, "No law categories are currently available.");
        return;
    }
    let row_h = law_panel_category_row_height(inner, data.slots.len());
    let gap = if row_h < 48.0 { 4.0 } else { 6.0 };
    let mut y = inner.top();
    for slot in &data.slots {
        let row = egui::Rect::from_min_size(
            egui::Pos2::new(inner.left(), y),
            egui::Vec2::new(inner.width(), row_h),
        );
        let response = ui.interact(
            row,
            ui.id()
                .with(("law_group_hoi4_iron", category_label(&slot.category))),
            egui::Sense::click(),
        );
        if response.clicked() {
            *selected_category = Some(slot.category);
        }
        let selected = *selected_category == Some(slot.category);
        paint_law_panel_category_row(ui, row, slot, selected, response.hovered(), icon_bank);
        response.on_hover_text(law_panel_category_tooltip(slot));
        y += row_h + gap;
    }
}

fn hoi4_iron_law_impact_preview(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    slot: Option<&LawSlotEntry>,
    political_power: f32,
    cmds: &mut Vec<LawCommand>,
    icon_bank: &mut Option<&mut crate::icons::IconBank>,
) {
    let inner = paint_law_panel_section(ui, rect, "Candidate Laws");
    let Some(slot) = slot else {
        paint_law_panel_empty(ui, inner, "No law categories are currently available.");
        return;
    };

    let header = paint_law_panel_current_slot(ui, inner, slot, political_power, icon_bank);
    let list_rect = egui::Rect::from_min_max(
        egui::Pos2::new(inner.left(), header.bottom() + 8.0),
        inner.right_bottom(),
    );
    paint_law_panel_region(ui.painter(), list_rect, crate::v9::palette::PANEL_DEEP);
    ui.allocate_ui_at_rect(list_rect, |ui| {
        ui.set_clip_rect(list_rect);
        ui.set_min_size(list_rect.shrink2(egui::Vec2::splat(8.0)).size());
        egui::ScrollArea::vertical()
            .id_salt(("law_panel_phase3_tiers", law_category_key(slot.category)))
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(8.0);
                for tier in &slot.tiers {
                    hoi4_iron_law_tier_row(ui, slot, tier, political_power, cmds, icon_bank);
                    ui.add_space(6.0);
                }
            });
    });
}

fn hoi4_iron_law_tier_row(
    ui: &mut egui::Ui,
    slot: &LawSlotEntry,
    tier: &LawTierEntry,
    political_power: f32,
    cmds: &mut Vec<LawCommand>,
    icon_bank: &mut Option<&mut crate::icons::IconBank>,
) {
    let row_w = ui.available_width().max(280.0);
    let height = law_panel_tier_row_height(tier);
    let (row, response) =
        ui.allocate_exact_size(egui::Vec2::new(row_w, height), egui::Sense::click());
    let visual = law_panel_tier_visual(slot, tier, political_power);
    let hovered = response.hovered();
    let fill = if hovered {
        crate::v9::palette::IRON
    } else {
        crate::v9::palette::SOOT_BLACK
    };
    crate::v9::paint::paint_bevel(
        ui.painter(),
        row,
        fill,
        if hovered || visual.clickable {
            crate::v9::palette::BRASS_DARK
        } else {
            crate::v9::palette::EDGE_DARK
        },
        crate::v9::radius::R1,
    );
    ui.painter().rect_filled(
        egui::Rect::from_min_size(row.left_top(), egui::Vec2::new(5.0, row.height())),
        egui::epaint::CornerRadius::ZERO,
        visual.accent,
    );

    if matches!(
        visual.state,
        LawPanelTierVisualState::Pending | LawPanelTierVisualState::Available
    ) {
        let glow = egui::Rect::from_min_size(
            egui::Pos2::new(row.left() + 5.0, row.top() + 1.0),
            egui::Vec2::new(row.width() - 6.0, 2.0),
        );
        ui.painter().rect_filled(
            glow,
            egui::epaint::CornerRadius::ZERO,
            Color32::from_rgba_premultiplied(
                visual.accent.r(),
                visual.accent.g(),
                visual.accent.b(),
                90,
            ),
        );
    }

    let icon = egui::Rect::from_min_size(
        row.left_top() + egui::Vec2::new(14.0, 12.0),
        egui::Vec2::splat(52.0),
    );
    paint_law_panel_icon_slot(
        ui,
        icon,
        slot.category,
        Some(&tier.id),
        visual.accent,
        icon_bank,
    );
    let action_rect = egui::Rect::from_min_size(
        egui::Pos2::new(row.right() - 104.0, row.top() + 14.0),
        egui::Vec2::new(88.0, 26.0),
    );
    let text_left = icon.right() + 12.0;
    let text_right = action_rect.left() - 10.0;
    ui.painter().text(
        egui::Pos2::new(text_left, row.top() + 10.0),
        egui::Align2::LEFT_TOP,
        tier.name.as_str(),
        crate::v9::text::fit_font_to_width(
            tier.name.as_str(),
            crate::v9::TextRole::Subheading.font_id(),
            (text_right - text_left).max(40.0),
            0.68,
        ),
        if visual.state == LawPanelTierVisualState::Current {
            crate::v9::palette::GOOD
        } else {
            crate::v9::palette::PARCHMENT
        },
    );
    ui.painter().text(
        egui::Pos2::new(text_left, row.top() + 32.0),
        egui::Align2::LEFT_TOP,
        format!("{} PP | cooldown {} days", tier.pp_cost, tier.cooldown_days),
        crate::v9::TextRole::Caption.font_id(),
        crate::v9::palette::GOLD,
    );

    let markers = law_panel_state_markers(slot, tier, political_power);
    let mut marker_x = text_left;
    for marker in markers.iter().take(3) {
        let marker_w = law_panel_marker_width(&marker.label);
        let marker_rect = egui::Rect::from_min_size(
            egui::Pos2::new(marker_x, row.top() + 51.0),
            egui::Vec2::new(marker_w, 18.0),
        );
        paint_law_panel_state_marker(ui, marker_rect, &marker.label, visual.accent);
        marker_x += marker_w + 5.0;
        if marker_x > text_right - 44.0 {
            break;
        }
    }

    let mut effect_y = row.top() + 74.0;
    if tier.effects.is_empty() {
        ui.painter().text(
            egui::Pos2::new(text_left, effect_y),
            egui::Align2::LEFT_TOP,
            "暂无效果数据",
            crate::v9::TextRole::Caption.font_id(),
            crate::v9::palette::MUTED,
        );
    } else {
        for effect in tier.effects.iter().take(2) {
            ui.painter().text(
                egui::Pos2::new(text_left, effect_y),
                egui::Align2::LEFT_TOP,
                effect.as_str(),
                crate::v9::text::fit_font_to_width(
                    effect,
                    crate::v9::TextRole::Caption.font_id(),
                    (row.right() - text_left - 18.0).max(40.0),
                    0.66,
                ),
                crate::v9::palette::PARCHMENT_DIM,
            );
            effect_y += 14.0;
        }
    }

    if visual.clickable {
        if paint_law_panel_action_button(
            ui,
            action_rect,
            tr("v6_law_switch"),
            true,
            ("switch", law_category_key(slot.category), tier.id.as_str()),
        )
        .clicked()
        {
            if let Some(command) = law_switch_command(slot, tier, political_power) {
                cmds.push(command);
            }
        }
    } else {
        paint_law_panel_action_button(
            ui,
            action_rect,
            visual.status_label,
            false,
            ("status", law_category_key(slot.category), tier.id.as_str()),
        );
    }
    response.on_hover_text(law_tier_tooltip(slot, tier, political_power));
}

fn paint_law_panel_region(painter: &egui::Painter, rect: egui::Rect, fill: Color32) {
    painter.rect_filled(rect, egui::epaint::CornerRadius::same(1), fill);
    crate::v9::paint::paint_vertical_gradient_mesh(
        painter,
        rect.shrink(1.0),
        Color32::from_rgba_premultiplied(0x20, 0x22, 0x1d, 90),
        Color32::from_black_alpha(220),
    );
    painter.rect_stroke(
        rect,
        egui::epaint::CornerRadius::same(1),
        egui::Stroke::new(1.0, crate::v9::palette::EDGE_DARK),
        egui::epaint::StrokeKind::Inside,
    );
}

fn paint_law_panel_section(ui: &mut egui::Ui, rect: egui::Rect, title: &str) -> egui::Rect {
    paint_law_panel_region(ui.painter(), rect, crate::v9::palette::PANEL_DEEP);
    let header = egui::Rect::from_min_size(rect.left_top(), egui::Vec2::new(rect.width(), 28.0));
    ui.painter().rect_filled(
        header,
        egui::epaint::CornerRadius::same(1),
        crate::v9::palette::IRON_BLACK,
    );
    crate::v9::paint::paint_vertical_gradient_mesh(
        ui.painter(),
        header,
        Color32::from_rgba_premultiplied(0x42, 0x47, 0x3b, 105),
        Color32::from_black_alpha(214),
    );
    ui.painter().text(
        egui::Pos2::new(header.left() + 8.0, header.center().y),
        egui::Align2::LEFT_CENTER,
        title,
        crate::v9::TextRole::Subheading.font_id(),
        crate::v9::palette::BRASS_BRIGHT,
    );
    egui::Rect::from_min_max(
        egui::Pos2::new(rect.left() + 8.0, header.bottom() + 8.0),
        egui::Pos2::new(rect.right() - 8.0, rect.bottom() - 8.0),
    )
}

fn paint_law_panel_empty(ui: &mut egui::Ui, rect: egui::Rect, text: &str) {
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        text,
        crate::v9::TextRole::Body.font_id(),
        crate::v9::palette::MUTED,
    );
}

fn law_panel_category_row_height(inner: egui::Rect, count: usize) -> f32 {
    if count == 0 {
        return 52.0;
    }
    let gaps = 6.0 * count.saturating_sub(1) as f32;
    ((inner.height() - gaps) / count as f32).clamp(42.0, 58.0)
}

fn paint_law_panel_category_row(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    slot: &LawSlotEntry,
    selected: bool,
    hovered: bool,
    icon_bank: &mut Option<&mut crate::icons::IconBank>,
) {
    let accent = law_panel_category_accent(slot);
    let fill = if selected {
        crate::v9::palette::IRON
    } else if hovered {
        crate::v9::palette::IRON_DARK
    } else {
        crate::v9::palette::SOOT_BLACK
    };
    crate::v9::paint::paint_bevel(
        ui.painter(),
        rect,
        fill,
        if selected {
            crate::v9::palette::BRASS_DARK
        } else {
            crate::v9::palette::EDGE_DARK
        },
        crate::v9::radius::R1,
    );
    ui.painter().rect_filled(
        egui::Rect::from_min_size(rect.left_top(), egui::Vec2::new(4.0, rect.height())),
        egui::epaint::CornerRadius::ZERO,
        accent,
    );
    let icon_rect = egui::Rect::from_min_size(
        rect.left_top() + egui::Vec2::new(11.0, (rect.height() - 30.0) * 0.5),
        egui::Vec2::splat(30.0),
    );
    paint_law_panel_icon_slot(
        ui,
        icon_rect,
        slot.category,
        Some(&slot.current_id),
        accent,
        icon_bank,
    );
    let text_left = icon_rect.right() + 9.0;
    let status_rect = egui::Rect::from_min_size(
        egui::Pos2::new(rect.right() - 66.0, rect.center().y - 10.0),
        egui::Vec2::new(58.0, 20.0),
    );
    ui.painter().text(
        egui::Pos2::new(text_left, rect.top() + 8.0),
        egui::Align2::LEFT_TOP,
        category_label(&slot.category),
        crate::v9::text::fit_font_to_width(
            category_label(&slot.category),
            crate::v9::TextRole::Subheading.font_id(),
            (status_rect.left() - text_left - 8.0).max(32.0),
            0.68,
        ),
        if selected {
            crate::v9::palette::GOLD_HOT
        } else {
            crate::v9::palette::PARCHMENT
        },
    );
    ui.painter().text(
        egui::Pos2::new(text_left, rect.bottom() - 9.0),
        egui::Align2::LEFT_BOTTOM,
        slot.current_name.as_str(),
        crate::v9::text::fit_font_to_width(
            slot.current_name.as_str(),
            crate::v9::TextRole::Caption.font_id(),
            (status_rect.left() - text_left - 8.0).max(32.0),
            0.62,
        ),
        crate::v9::palette::PARCHMENT_DIM,
    );
    paint_law_panel_label_chip(ui, status_rect, law_panel_slot_status_label(slot), accent);
}

fn paint_law_panel_current_slot(
    ui: &mut egui::Ui,
    inner: egui::Rect,
    slot: &LawSlotEntry,
    political_power: f32,
    icon_bank: &mut Option<&mut crate::icons::IconBank>,
) -> egui::Rect {
    let header = egui::Rect::from_min_size(inner.left_top(), egui::Vec2::new(inner.width(), 82.0));
    crate::v9::paint::paint_bevel(
        ui.painter(),
        header,
        crate::v9::palette::SOOT_BLACK,
        crate::v9::palette::EDGE_DARK,
        crate::v9::radius::R1,
    );
    let accent = law_panel_category_accent(slot);
    let icon = egui::Rect::from_min_size(
        header.left_top() + egui::Vec2::new(12.0, 13.0),
        egui::Vec2::splat(54.0),
    );
    paint_law_panel_icon_slot(
        ui,
        icon,
        slot.category,
        Some(&slot.current_id),
        accent,
        icon_bank,
    );
    let right = egui::Rect::from_min_size(
        egui::Pos2::new(header.right() - 92.0, header.top() + 12.0),
        egui::Vec2::new(78.0, 23.0),
    );
    paint_law_panel_label_chip(
        ui,
        right,
        &format!("{political_power:.0} PP"),
        crate::v9::palette::GOLD,
    );
    let text_left = icon.right() + 12.0;
    ui.painter().text(
        egui::Pos2::new(text_left, header.top() + 10.0),
        egui::Align2::LEFT_TOP,
        category_label(&slot.category),
        crate::v9::TextRole::Subheading.font_id(),
        crate::v9::palette::BRASS_BRIGHT,
    );
    ui.painter().text(
        egui::Pos2::new(text_left, header.top() + 33.0),
        egui::Align2::LEFT_TOP,
        format!("{}: {}", tr("current"), slot.current_name),
        crate::v9::text::fit_font_to_width(
            &format!("{}: {}", tr("current"), slot.current_name),
            crate::v9::TextRole::Body.font_id(),
            (right.left() - text_left - 8.0).max(40.0),
            0.70,
        ),
        crate::v9::palette::PARCHMENT,
    );
    let status = law_panel_slot_status_text(slot);
    ui.painter().text(
        egui::Pos2::new(text_left, header.bottom() - 11.0),
        egui::Align2::LEFT_BOTTOM,
        status.as_str(),
        crate::v9::text::fit_font_to_width(
            status.as_str(),
            crate::v9::TextRole::Caption.font_id(),
            (header.right() - text_left - 12.0).max(40.0),
            0.68,
        ),
        law_panel_slot_status_color(slot),
    );
    header
}

fn paint_law_panel_icon_slot(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    category: LawCategory,
    law_id: Option<&str>,
    accent: Color32,
    icon_bank: &mut Option<&mut crate::icons::IconBank>,
) {
    crate::v9::paint::paint_bevel(
        ui.painter(),
        rect,
        crate::v9::palette::IRON_BLACK,
        accent,
        crate::v9::radius::R1,
    );
    if let (Some(law_id), Some(icon_bank)) = (law_id, icon_bank.as_mut()) {
        let sprite = law_panel_law_sprite(category, law_id);
        if let Some(handle) = icon_bank.get_or_load(sprite) {
            let image_rect = rect.shrink(4.0);
            ui.put(
                image_rect,
                egui::Image::from_texture(handle).fit_to_exact_size(image_rect.size()),
            );
            return;
        }
    }
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        law_panel_category_glyph(category),
        crate::v9::text::fit_font_to_width(
            law_panel_category_glyph(category),
            crate::v9::TextRole::Heading.font_id(),
            rect.width() - 8.0,
            0.55,
        ),
        crate::v9::palette::PARCHMENT,
    );
}

fn paint_law_panel_label_chip(ui: &mut egui::Ui, rect: egui::Rect, text: &str, color: Color32) {
    crate::v9::paint::paint_bevel(
        ui.painter(),
        rect,
        crate::v9::palette::IRON_BLACK,
        crate::v9::palette::EDGE_DARK,
        crate::v9::radius::R1,
    );
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        text,
        crate::v9::text::fit_font_to_width(
            text,
            crate::v9::TextRole::Small.font_id(),
            rect.width() - 8.0,
            0.62,
        ),
        color,
    );
}

fn paint_law_panel_state_marker(ui: &mut egui::Ui, rect: egui::Rect, text: &str, color: Color32) {
    ui.painter().rect_filled(
        rect,
        egui::epaint::CornerRadius::same(1),
        Color32::from_rgba_premultiplied(0x05, 0x06, 0x05, 225),
    );
    ui.painter().rect_stroke(
        rect,
        egui::epaint::CornerRadius::same(1),
        egui::Stroke::new(1.0, color),
        egui::epaint::StrokeKind::Inside,
    );
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        text,
        crate::v9::text::fit_font_to_width(
            text,
            crate::v9::TextRole::Small.font_id(),
            rect.width() - 6.0,
            0.58,
        ),
        color,
    );
}

fn paint_law_panel_action_button(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    text: &str,
    enabled: bool,
    id_source: impl std::hash::Hash,
) -> egui::Response {
    let response = ui.interact(
        rect,
        ui.id().with(("law_panel_tier_action", id_source)),
        if enabled {
            egui::Sense::click()
        } else {
            egui::Sense::hover()
        },
    );
    let fill = if enabled && response.hovered() {
        crate::v9::palette::IRON
    } else if enabled {
        crate::v9::palette::IRON_DARK
    } else {
        crate::v9::palette::SOOT_BLACK
    };
    crate::v9::paint::paint_bevel(
        ui.painter(),
        rect,
        fill,
        if enabled {
            crate::v9::palette::GOLD
        } else {
            crate::v9::palette::EDGE_DARK
        },
        crate::v9::radius::R1,
    );
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        text,
        crate::v9::text::fit_font_to_width(
            text,
            crate::v9::TextRole::Caption.font_id(),
            rect.width() - 8.0,
            0.64,
        ),
        if enabled {
            crate::v9::palette::GOLD_HOT
        } else {
            crate::v9::palette::MUTED
        },
    );
    if enabled && response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response
}

fn law_panel_tier_row_height(tier: &LawTierEntry) -> f32 {
    94.0 + (tier.effects.len().min(2) as f32 * 12.0)
}

fn law_panel_marker_width(text: &str) -> f32 {
    (crate::v9::text::approximate_text_width(text, crate::v9::TextRole::Small.size()) + 12.0)
        .clamp(34.0, 86.0)
}

fn law_panel_category_accent(slot: &LawSlotEntry) -> Color32 {
    if slot.is_locked {
        crate::v9::palette::BAD
    } else if slot.pending.is_some() || slot.cooldown_days > 0 {
        crate::v9::palette::WARN
    } else {
        hoi4_iron_law_category_color(&slot.category)
    }
}

fn law_panel_slot_status_label(slot: &LawSlotEntry) -> &'static str {
    if slot.is_locked {
        "LOCK"
    } else if slot.pending.is_some() {
        "PEND"
    } else if slot.cooldown_days > 0 {
        "CD"
    } else {
        "CUR"
    }
}

fn law_panel_slot_status_text(slot: &LawSlotEntry) -> String {
    if slot.is_locked {
        slot.locked_reason
            .clone()
            .unwrap_or_else(|| "该法律类别当前被锁定。".to_owned())
    } else if let Some((_, target, days)) = &slot.pending {
        format!("{}: {target} / {days} days", tr("v6_law_pending"))
    } else if slot.cooldown_days > 0 {
        format!("{}: {} days", tr("cooldown"), slot.cooldown_days)
    } else {
        tr("available").to_owned()
    }
}

fn law_panel_slot_status_color(slot: &LawSlotEntry) -> Color32 {
    if slot.is_locked {
        crate::v9::palette::BAD
    } else if slot.pending.is_some() || slot.cooldown_days > 0 {
        crate::v9::palette::WARN
    } else {
        crate::v9::palette::GOOD
    }
}

fn law_panel_category_glyph(category: LawCategory) -> &'static str {
    match category {
        LawCategory::Conscription => "C",
        LawCategory::Economy => "E",
        LawCategory::Trade => "T",
        LawCategory::Taxation => "$",
        LawCategory::CivilRights => "R",
        LawCategory::InformationControl => "I",
    }
}

fn law_panel_tier_visual(
    slot: &LawSlotEntry,
    tier: &LawTierEntry,
    political_power: f32,
) -> LawPanelTierVisual {
    let is_current = tier.id == slot.current_id;
    let is_pending = slot
        .pending
        .as_ref()
        .map_or(false, |(id, _, _)| id == &tier.id);
    let state = if is_current {
        LawPanelTierVisualState::Current
    } else if is_pending {
        LawPanelTierVisualState::Pending
    } else if slot.is_locked {
        LawPanelTierVisualState::Locked
    } else if slot.cooldown_days > 0 {
        LawPanelTierVisualState::Cooling
    } else if political_power < tier.pp_cost as f32 {
        LawPanelTierVisualState::NotEnoughPoliticalPower
    } else if law_switch_available(slot, tier, political_power) {
        LawPanelTierVisualState::Available
    } else {
        LawPanelTierVisualState::Disabled
    };
    let accent = match state {
        LawPanelTierVisualState::Current => crate::v9::palette::GOOD,
        LawPanelTierVisualState::Pending | LawPanelTierVisualState::Cooling => {
            crate::v9::palette::WARN
        }
        LawPanelTierVisualState::Available => crate::v9::palette::GOLD,
        LawPanelTierVisualState::Locked | LawPanelTierVisualState::NotEnoughPoliticalPower => {
            crate::v9::palette::BAD
        }
        LawPanelTierVisualState::Disabled => crate::v9::palette::MUTED,
    };
    let status_label = match state {
        LawPanelTierVisualState::Current => tr("current"),
        LawPanelTierVisualState::Pending => tr("v6_law_pending"),
        LawPanelTierVisualState::Available => tr("v6_law_switch"),
        LawPanelTierVisualState::Locked => tr("locked"),
        LawPanelTierVisualState::Cooling => tr("cooldown"),
        LawPanelTierVisualState::NotEnoughPoliticalPower => "PP low",
        LawPanelTierVisualState::Disabled => "Disabled",
    };
    LawPanelTierVisual {
        state,
        accent,
        clickable: state == LawPanelTierVisualState::Available,
        status_label,
    }
}

fn law_panel_state_markers(
    slot: &LawSlotEntry,
    tier: &LawTierEntry,
    political_power: f32,
) -> Vec<LawPanelStateMarker> {
    let mut markers = Vec::new();
    if tier.id == slot.current_id {
        markers.push(LawPanelStateMarker {
            key: "current",
            label: tr("current").to_owned(),
        });
    }
    if let Some((id, _, days)) = &slot.pending {
        if id == &tier.id {
            markers.push(LawPanelStateMarker {
                key: "pending",
                label: format!("{} {days}d", tr("v6_law_pending")),
            });
        }
    }
    if slot.cooldown_days > 0 {
        markers.push(LawPanelStateMarker {
            key: "cooldown",
            label: format!("CD {}d", slot.cooldown_days),
        });
    } else {
        markers.push(LawPanelStateMarker {
            key: "tier_cooldown",
            label: format!("CD+{}d", tier.cooldown_days),
        });
    }
    if slot.is_locked {
        markers.push(LawPanelStateMarker {
            key: "locked",
            label: tr("locked").to_owned(),
        });
    }
    if political_power < tier.pp_cost as f32 {
        markers.push(LawPanelStateMarker {
            key: "pp",
            label: "PP low".to_owned(),
        });
    }
    if markers.is_empty() && law_switch_available(slot, tier, political_power) {
        markers.push(LawPanelStateMarker {
            key: "available",
            label: tr("available").to_owned(),
        });
    }
    markers
}

fn hoi4_iron_law_affordable_count(data: &LawPanelData) -> usize {
    data.slots
        .iter()
        .flat_map(|slot| slot.tiers.iter().map(move |tier| (slot, tier)))
        .filter(|(slot, tier)| {
            tier.id != slot.current_id
                && slot
                    .pending
                    .as_ref()
                    .map_or(true, |(id, _, _)| id != &tier.id)
                && slot.cooldown_days == 0
                && !slot.is_locked
                && data.political_power >= tier.pp_cost as f32
        })
        .count()
}

fn hoi4_iron_law_category_color(cat: &LawCategory) -> Color32 {
    use crate::v9::tokens::palette;
    match cat {
        LawCategory::Conscription => palette::IDEO_FASCISM,
        LawCategory::Economy => palette::INFO,
        LawCategory::Trade => palette::GOOD,
        LawCategory::Taxation => palette::GOLD,
        LawCategory::CivilRights => palette::COLD_ATOMIC,
        LawCategory::InformationControl => palette::BAD,
    }
}

fn disabled_law_reason(
    slot: &LawSlotEntry,
    tier: &LawTierEntry,
    is_pending: bool,
    has_pp: bool,
) -> String {
    if tier.id == slot.current_id {
        return "当前正在使用该法律。".to_owned();
    }
    if is_pending {
        return "该法律已经在切换流程中。".to_owned();
    }
    if slot.is_locked {
        return slot
            .locked_reason
            .clone()
            .unwrap_or_else(|| "该法律类别当前被锁定。".to_owned());
    }
    if slot.cooldown_days > 0 {
        return format!("法律冷却中，还需 {} 天。", slot.cooldown_days);
    }
    if !has_pp {
        return format!("政治力量不足，需要 {} PP。", tier.pp_cost);
    }
    "暂不可切换。".to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vanilla_gui::{GuiAction, GuiActionKind, GuiNodePath, VanillaPanelProfile};
    use std::collections::BTreeSet;

    fn sample_slot() -> LawSlotEntry {
        LawSlotEntry {
            category: LawCategory::Conscription,
            current_id: "volunteer_only".into(),
            current_name: "志愿兵役".into(),
            cooldown_days: 0,
            pending: None,
            is_locked: false,
            locked_reason: None,
            previous_before_lock: None,
            tiers: vec![
                LawTierEntry {
                    id: "volunteer_only".into(),
                    name: "志愿兵役".into(),
                    pp_cost: 50,
                    cooldown_days: 60,
                    effects: Vec::new(),
                },
                LawTierEntry {
                    id: "limited_conscription".into(),
                    name: "有限征兵".into(),
                    pp_cost: 100,
                    cooldown_days: 90,
                    effects: Vec::new(),
                },
                LawTierEntry {
                    id: "extensive_conscription".into(),
                    name: "广泛征兵".into(),
                    pp_cost: 200,
                    cooldown_days: 120,
                    effects: Vec::new(),
                },
                LawTierEntry {
                    id: "total_mobilization".into(),
                    name: "Total Mobilization".into(),
                    pp_cost: 400,
                    cooldown_days: 180,
                    effects: Vec::new(),
                },
            ],
        }
    }

    #[test]
    fn sample_slot_constructs() {
        let s = sample_slot();
        assert_eq!(s.tiers.len(), 4);
        assert_eq!(s.current_id, "volunteer_only");
        assert!(!s.is_locked);
    }

    fn economy_slot() -> LawSlotEntry {
        LawSlotEntry {
            category: LawCategory::Economy,
            current_id: "civilian_economy".into(),
            current_name: "民用经济".into(),
            cooldown_days: 0,
            pending: None,
            is_locked: false,
            locked_reason: None,
            previous_before_lock: None,
            tiers: vec![
                LawTierEntry {
                    id: "civilian_economy".into(),
                    name: "民用经济".into(),
                    pp_cost: 0,
                    cooldown_days: 0,
                    effects: vec!["Consumer goods factories +30%".into()],
                },
                LawTierEntry {
                    id: "partial_mobilization".into(),
                    name: "部分动员".into(),
                    pp_cost: 150,
                    cooldown_days: 90,
                    effects: vec![
                        "Construction speed +10%".into(),
                        "Consumer goods factories -5%".into(),
                    ],
                },
            ],
        }
    }

    fn sample_panel_data() -> LawPanelData {
        LawPanelData {
            political_power: 175.0,
            slots: vec![sample_slot(), economy_slot()],
        }
    }

    fn all_phase3_slots() -> Vec<LawSlotEntry> {
        let mut conscription = sample_slot();
        conscription.category = LawCategory::Conscription;
        conscription.current_name = "Volunteer Only".into();

        let mut economy = economy_slot();
        economy.pending = Some((
            "partial_mobilization".into(),
            "Partial Mobilization".into(),
            24,
        ));

        let mut trade = economy_slot();
        trade.category = LawCategory::Trade;
        trade.current_id = "free_trade".into();
        trade.current_name = "Free Trade".into();
        trade.cooldown_days = 12;
        trade.pending = None;

        let mut taxation = economy_slot();
        taxation.category = LawCategory::Taxation;
        taxation.current_id = "medium_taxation".into();
        taxation.current_name = "Medium Taxation".into();

        let mut civil = economy_slot();
        civil.category = LawCategory::CivilRights;
        civil.current_id = "limited_rights".into();
        civil.current_name = "Limited Rights".into();
        civil.is_locked = true;
        civil.locked_reason = Some("Locked by political reform".into());

        let mut info = economy_slot();
        info.category = LawCategory::InformationControl;
        info.current_id = "regulated_press".into();
        info.current_name = "Regulated Press".into();

        vec![conscription, economy, trade, taxation, civil, info]
    }

    fn phase3_panel_data() -> LawPanelData {
        LawPanelData {
            political_power: 125.0,
            slots: all_phase3_slots(),
        }
    }

    #[test]
    fn law_command_equality() {
        let cmd = LawCommand::SwitchLaw {
            category: LawCategory::Economy,
            target_law_id: "war_economy".into(),
        };
        assert_eq!(
            cmd,
            LawCommand::SwitchLaw {
                category: LawCategory::Economy,
                target_law_id: "war_economy".into(),
            }
        );
    }

    #[test]
    fn gate5_law_panel_profile_declares_vanilla_skeleton_and_iron_debug_fallback_route() {
        let profile = LawPanelProfile;

        assert_eq!(profile.profile_id(), LAW_PANEL_PROFILE_ID);
        assert_eq!(profile.root_template(), LAW_PANEL_POPUP_ROOT_TEMPLATE);
        assert_eq!(profile.required_gui_files(), &[LAW_PANEL_GUI_FILE]);
        assert!(profile
            .template_instances()
            .iter()
            .any(|instance| instance.template_name == LAW_PANEL_IDEA_CATEGORY_TEMPLATE));
        assert!(profile
            .template_instances()
            .iter()
            .any(|instance| instance.template_name == LAW_PANEL_IDEA_SLOT_TEMPLATE));
        assert!(profile.template_instances().iter().any(|instance| {
            instance.template_name == LAW_PANEL_SELECTABLE_ENTRY_LIST_TEMPLATE
        }));
        assert_eq!(LAW_PANEL_DEFAULT_ROUTE, "vanilla_profile");
        assert_eq!(LAW_PANEL_IRON_DEBUG_FALLBACK_ROUTE, "hoi4_iron_debug");
    }

    #[test]
    fn gate6_law_panel_profile_binds_title_pp_category_and_current_law() {
        let profile = LawPanelProfile;
        let data = sample_panel_data();

        let title = profile.bind_node(
            &GuiNodePath::root(LAW_PANEL_POPUP_ROOT_TEMPLATE).child("title"),
            &data,
        );
        assert_eq!(
            title.text.as_deref(),
            Some(category_label(&LawCategory::Conscription))
        );

        let selected_title = profile.bind_node_with_selection(
            &GuiNodePath::root(LAW_PANEL_POPUP_ROOT_TEMPLATE).child("title"),
            &data,
            Some(LawCategory::Economy),
        );
        assert_eq!(
            selected_title.text.as_deref(),
            Some(category_label(&LawCategory::Economy))
        );

        let pp = profile.bind_node(
            &GuiNodePath::root(LAW_PANEL_POPUP_ROOT_TEMPLATE).child("switch"),
            &data,
        );
        assert_eq!(
            pp.text.as_deref(),
            Some(format!("志愿兵役 | {}: 175", tr("political_power")).as_str())
        );

        let category_name = profile.bind_node(
            &GuiNodePath::root(LAW_PANEL_MAIN_ROOT_TEMPLATE)
                .child(LAW_PANEL_CATEGORY_GRID_NODE)
                .child(format!("{LAW_PANEL_IDEA_CATEGORY_TEMPLATE}[1]"))
                .child("name"),
            &data,
        );
        assert_eq!(
            category_name.text.as_deref(),
            Some(category_label(&LawCategory::Economy))
        );

        let current_law = profile.bind_node(
            &GuiNodePath::root(LAW_PANEL_MAIN_ROOT_TEMPLATE)
                .child(LAW_PANEL_CATEGORY_GRID_NODE)
                .child(format!("{LAW_PANEL_IDEA_CATEGORY_TEMPLATE}[1]"))
                .child("ideas_grid")
                .child(format!("{LAW_PANEL_IDEA_SLOT_TEMPLATE}[0]"))
                .child("add_idea_button"),
            &data,
        );
        assert_eq!(current_law.text.as_deref(), Some("民用经济"));
        assert_eq!(
            current_law
                .click
                .as_ref()
                .map(|click| click.command.as_str()),
            Some("law_panel:detail:Economy:civilian_economy")
        );
        assert_eq!(current_law.sprite.as_deref(), Some("GFX_law_laissez_faire"));
    }

    #[test]
    fn gate7_law_panel_profile_uses_dynamic_slot_and_tier_counts() {
        let profile = LawPanelProfile;
        let mut data = sample_panel_data();

        let categories = profile.bind_node(
            &GuiNodePath::root(LAW_PANEL_MAIN_ROOT_TEMPLATE).child(LAW_PANEL_CATEGORY_GRID_NODE),
            &data,
        );
        assert_eq!(categories.instance_count, Some(2));

        let tiers = profile.bind_node_with_selection(
            &GuiNodePath::root(LAW_PANEL_POPUP_ROOT_TEMPLATE).child(LAW_PANEL_TIER_LIST_NODE),
            &data,
            Some(LawCategory::Economy),
        );
        assert_eq!(tiers.instance_count, Some(2));
        assert_eq!(
            law_panel_selected_tier_count(&data, Some(LawCategory::Economy)),
            2
        );

        data.slots[1].tiers.pop();
        assert_eq!(
            law_panel_selected_tier_count(&data, Some(LawCategory::Economy)),
            1
        );

        data.slots[1].tiers.clear();
        let empty_tiers = profile.bind_node_with_selection(
            &GuiNodePath::root(LAW_PANEL_POPUP_ROOT_TEMPLATE).child(LAW_PANEL_TIER_LIST_NODE),
            &data,
            Some(LawCategory::Economy),
        );
        assert_eq!(empty_tiers.instance_count, Some(0));
    }

    #[test]
    fn gate8_law_panel_profile_actions_return_detail_commands_not_switch_law() {
        let profile = LawPanelProfile;
        let data = sample_panel_data();

        let category = profile.handle_action(
            GuiAction {
                node_path: GuiNodePath::root(LAW_PANEL_MAIN_ROOT_TEMPLATE)
                    .child(LAW_PANEL_CATEGORY_GRID_NODE)
                    .child(format!("{LAW_PANEL_IDEA_CATEGORY_TEMPLATE}[1]")),
                kind: GuiActionKind::Click,
            },
            &data,
        );
        assert_eq!(
            category,
            Some(LawPanelProfileCommand::SelectCategory {
                category: LawCategory::Economy
            })
        );

        let current = profile.handle_action(
            GuiAction {
                node_path: GuiNodePath::root(LAW_PANEL_MAIN_ROOT_TEMPLATE)
                    .child(LAW_PANEL_CATEGORY_GRID_NODE)
                    .child(format!("{LAW_PANEL_IDEA_CATEGORY_TEMPLATE}[1]"))
                    .child("ideas_grid")
                    .child(format!("{LAW_PANEL_IDEA_SLOT_TEMPLATE}[0]"))
                    .child("add_idea_button"),
                kind: GuiActionKind::Click,
            },
            &data,
        );
        assert_eq!(
            current,
            Some(LawPanelProfileCommand::OpenLawDetail {
                category: LawCategory::Economy,
                law_id: Some("civilian_economy".into())
            })
        );

        let tier =
            profile.handle_click_command("law_panel:detail:Economy:partial_mobilization", &data);
        assert_eq!(
            tier,
            Some(LawPanelProfileCommand::OpenLawDetail {
                category: LawCategory::Economy,
                law_id: Some("partial_mobilization".into())
            })
        );
        assert_ne!(
            LawCommand::SwitchLaw {
                category: LawCategory::Economy,
                target_law_id: "partial_mobilization".into()
            },
            LawCommand::SwitchLaw {
                category: LawCategory::Conscription,
                target_law_id: "partial_mobilization".into()
            }
        );
    }

    #[test]
    fn gate9_law_panel_shell_uses_vanilla_resources_and_stays_in_viewports() {
        for sprite in LAW_PANEL_PHASE3_SHELL_RESOURCES {
            assert!(LAW_PANEL_SPRITES.contains(sprite));
        }

        for viewport in [
            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::new(1920.0, 1080.0)),
            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::new(960.0, 640.0)),
        ] {
            let window = law_panel_window_rect(viewport);
            assert!(viewport.contains_rect(window));
            let rects = law_panel_shell_rects(window);
            assert!(window.contains_rect(rects.title_bar));
            assert!(window.contains_rect(rects.close_button));
            assert!(window.contains_rect(rects.body));
            assert!(window.contains_rect(rects.footer));
            assert!(rects.close_button.width() >= 23.0);
            assert!(rects.close_button.height() >= 23.0);
        }
    }

    #[test]
    fn gate10_law_panel_category_rows_cover_all_six_laws_with_clear_state() {
        for sprite in LAW_PANEL_PHASE3_CATEGORY_RESOURCES {
            assert!(LAW_PANEL_SPRITES.contains(sprite));
        }
        let data = phase3_panel_data();
        let viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::new(960.0, 640.0));
        let rects = law_panel_shell_rects(law_panel_window_rect(viewport));
        let (category_rect, _) = law_panel_body_rects(rects.body);
        let inner = egui::Rect::from_min_max(
            egui::Pos2::new(category_rect.left() + 8.0, category_rect.top() + 36.0),
            egui::Pos2::new(category_rect.right() - 8.0, category_rect.bottom() - 8.0),
        );
        let row_h = law_panel_category_row_height(inner, data.slots.len());

        assert_eq!(data.slots.len(), 6);
        assert!(row_h >= 42.0);
        assert!(data
            .slots
            .iter()
            .any(|slot| law_panel_slot_status_label(slot) == "PEND"));
        assert!(data
            .slots
            .iter()
            .any(|slot| law_panel_slot_status_label(slot) == "CD"));
        assert!(data
            .slots
            .iter()
            .any(|slot| law_panel_slot_status_label(slot) == "LOCK"));
        assert!(data
            .slots
            .iter()
            .any(|slot| law_panel_slot_status_label(slot) == "CUR"));
    }

    #[test]
    fn gate11_law_panel_tier_visuals_distinguish_clickable_current_pending_and_disabled() {
        for sprite in LAW_PANEL_PHASE3_TIER_RESOURCES {
            assert!(LAW_PANEL_SPRITES.contains(sprite));
        }
        let slot = LawSlotEntry {
            category: LawCategory::Economy,
            current_id: "civilian_economy".into(),
            current_name: "Civilian Economy".into(),
            cooldown_days: 0,
            pending: Some((
                "partial_mobilization".into(),
                "Partial Mobilization".into(),
                24,
            )),
            is_locked: false,
            locked_reason: None,
            previous_before_lock: None,
            tiers: vec![
                LawTierEntry {
                    id: "civilian_economy".into(),
                    name: "Civilian Economy".into(),
                    pp_cost: 0,
                    cooldown_days: 0,
                    effects: vec!["Factory output baseline".into()],
                },
                LawTierEntry {
                    id: "partial_mobilization".into(),
                    name: "Partial Mobilization".into(),
                    pp_cost: 150,
                    cooldown_days: 90,
                    effects: vec!["Military factory construction speed +10%".into()],
                },
                LawTierEntry {
                    id: "war_economy".into(),
                    name: "War Economy".into(),
                    pp_cost: 250,
                    cooldown_days: 120,
                    effects: vec!["Military factory construction speed +20%".into()],
                },
            ],
        };

        let current = law_panel_tier_visual(&slot, &slot.tiers[0], 500.0);
        let pending = law_panel_tier_visual(&slot, &slot.tiers[1], 500.0);
        let expensive = law_panel_tier_visual(&slot, &slot.tiers[2], 50.0);
        let available = law_panel_tier_visual(&slot, &slot.tiers[2], 500.0);

        assert_eq!(current.state, LawPanelTierVisualState::Current);
        assert_eq!(pending.state, LawPanelTierVisualState::Pending);
        assert_eq!(
            expensive.state,
            LawPanelTierVisualState::NotEnoughPoliticalPower
        );
        assert_eq!(available.state, LawPanelTierVisualState::Available);
        assert!(!current.clickable);
        assert!(!pending.clickable);
        assert!(!expensive.clickable);
        assert!(available.clickable);
        assert!(law_panel_tier_row_height(&slot.tiers[2]) >= 106.0);
    }

    #[test]
    fn gate12_law_panel_state_markers_cover_pp_cooldown_locked_and_pending() {
        let mut slot = economy_slot();
        slot.cooldown_days = 7;
        slot.pending = Some((
            "partial_mobilization".into(),
            "Partial Mobilization".into(),
            19,
        ));
        slot.is_locked = true;
        let tier = slot
            .tiers
            .iter()
            .find(|tier| tier.id == "partial_mobilization")
            .unwrap();

        let markers = law_panel_state_markers(&slot, tier, 10.0);
        let keys: BTreeSet<&str> = markers.iter().map(|marker| marker.key).collect();

        assert!(keys.contains("pending"));
        assert!(keys.contains("cooldown"));
        assert!(keys.contains("locked"));
        assert!(keys.contains("pp"));
        assert!(law_panel_marker_width("PP low") >= 34.0);
        assert!(law_panel_marker_width("SW 120d") <= 86.0);
    }

    #[test]
    fn gate13_law_panel_default_visual_path_is_not_panel_shell_card_style() {
        let data = phase3_panel_data();
        let counts = law_panel_status_counts(&data);
        let rects = law_panel_shell_rects(law_panel_window_rect(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::Vec2::new(1280.0, 720.0),
        )));

        assert_eq!(LAW_PANEL_DEFAULT_ROUTE, "vanilla_profile");
        assert!(rects.title_bar.height() <= 40.0);
        assert!(rects.body.top() > rects.summary.bottom());
        assert_eq!(counts.pending, 1);
        assert_eq!(counts.cooling, 1);
        assert_eq!(counts.locked, 1);
        assert_ne!(law_panel_popup_accent(&data), crate::v9::palette::GOLD);
    }

    #[test]
    fn gate14_law_panel_profile_switch_click_returns_switch_law_or_blocked_feedback() {
        let profile = LawPanelProfile;
        let data = sample_panel_data();
        let tier_path = GuiNodePath::root(LAW_PANEL_POPUP_ROOT_TEMPLATE)
            .child(LAW_PANEL_TIER_LIST_NODE)
            .child(format!("{LAW_PANEL_SELECTABLE_ENTRY_LIST_TEMPLATE}[1]"))
            .child("idea_entry_bg");

        let binding =
            profile.bind_node_with_selection(&tier_path, &data, Some(LawCategory::Economy));
        assert_eq!(
            binding.click.as_ref().map(|click| click.command.as_str()),
            Some("law_panel:switch:Economy:partial_mobilization")
        );
        assert_eq!(
            profile.handle_click_command("law_panel:switch:Economy:partial_mobilization", &data),
            Some(LawPanelProfileCommand::SwitchLaw {
                category: LawCategory::Economy,
                target_law_id: "partial_mobilization".into(),
            })
        );

        let current = profile.bind_node_with_selection(
            &GuiNodePath::root(LAW_PANEL_POPUP_ROOT_TEMPLATE)
                .child(LAW_PANEL_TIER_LIST_NODE)
                .child(format!("{LAW_PANEL_SELECTABLE_ENTRY_LIST_TEMPLATE}[0]"))
                .child("idea_entry_bg"),
            &data,
            Some(LawCategory::Economy),
        );
        assert_eq!(
            current.click.as_ref().map(|click| click.command.as_str()),
            Some("law_panel:switch:Economy:civilian_economy"),
            "current law tiers stay clickable so disabled clicks can show feedback"
        );
        let current_icon = profile.bind_node_with_selection(
            &GuiNodePath::root(LAW_PANEL_POPUP_ROOT_TEMPLATE)
                .child(LAW_PANEL_TIER_LIST_NODE)
                .child(format!("{LAW_PANEL_SELECTABLE_ENTRY_LIST_TEMPLATE}[0]"))
                .child("icon"),
            &data,
            Some(LawCategory::Economy),
        );
        assert_eq!(
            current_icon.sprite.as_deref(),
            Some("GFX_law_laissez_faire")
        );

        let mut pending_data = data.clone();
        pending_data.slots[1].pending = Some((
            "partial_mobilization".into(),
            "Partial Mobilization".into(),
            12,
        ));
        assert_eq!(
            profile.handle_click_command(
                "law_panel:switch:Economy:partial_mobilization",
                &pending_data
            ),
            Some(LawPanelProfileCommand::Blocked {
                reason: law_unavailable_reason(
                    &pending_data.slots[1],
                    &pending_data.slots[1].tiers[1],
                    pending_data.political_power,
                )
                .unwrap(),
            }),
            "pending target must emit blocked feedback instead of a duplicate switch command"
        );
    }

    #[test]
    fn gate15_law_panel_clickability_matches_law_switch_available_for_all_rules() {
        let base = economy_slot();
        let current = base.tiers[0].clone();
        let target = base.tiers[1].clone();

        let mut cases = Vec::new();
        cases.push(("available", base.clone(), target.clone(), 200.0, true));
        cases.push(("current", base.clone(), current, 200.0, false));

        let mut pending = base.clone();
        pending.pending = Some((target.id.clone(), target.name.clone(), 18));
        cases.push(("pending", pending, target.clone(), 200.0, false));

        let mut cooling = base.clone();
        cooling.cooldown_days = 9;
        cases.push(("cooldown", cooling, target.clone(), 200.0, false));

        let mut locked = base.clone();
        locked.is_locked = true;
        locked.locked_reason = Some("Locked by political reform".into());
        cases.push(("locked", locked, target.clone(), 200.0, false));

        cases.push(("pp", base.clone(), target.clone(), 20.0, false));

        for (label, slot, tier, pp, expected) in cases {
            let available = law_switch_available(&slot, &tier, pp);
            let visual = law_panel_tier_visual(&slot, &tier, pp);
            assert_eq!(available, expected, "availability mismatch for {label}");
            assert_eq!(
                visual.clickable, available,
                "visual clickability must follow law_switch_available for {label}"
            );
            assert_eq!(
                law_switch_command(&slot, &tier, pp).is_some(),
                available,
                "command emission must follow law_switch_available for {label}"
            );
        }
    }

    #[test]
    fn gate16_law_tier_tooltip_includes_effects_and_all_unavailable_states() {
        let mut slot = economy_slot();
        slot.cooldown_days = 7;
        slot.pending = Some((
            "partial_mobilization".into(),
            "Partial Mobilization".into(),
            19,
        ));
        slot.is_locked = true;
        slot.locked_reason = Some("Locked by political reform".into());
        let tier = slot
            .tiers
            .iter()
            .find(|tier| tier.id == "partial_mobilization")
            .unwrap();

        let tooltip = law_tier_tooltip(&slot, tier, 10.0);

        assert!(tooltip.contains("Partial Mobilization"));
        assert!(tooltip.contains("150 PP"));
        assert!(tooltip.contains(tr("v6_law_pending")));
        assert!(tooltip.contains("7 days"));
        assert!(tooltip.contains("Locked by political reform"));
        assert!(tooltip.contains("政治力量不足"));
        assert!(tooltip.contains("Construction speed +10%"));
        assert!(tooltip.contains("Consumer goods factories -5%"));
    }

    #[test]
    fn gate17_law_panel_q_close_and_outside_click_do_not_emit_law_commands() {
        let ctx = egui::Context::default();
        let data = sample_panel_data();
        let mut close = false;
        let mut cmds = Vec::new();
        let raw_input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::Vec2::new(960.0, 640.0),
            )),
            events: vec![egui::Event::Key {
                key: egui::Key::Q,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
            ..Default::default()
        };
        let _ = ctx.run(raw_input, |ctx| {
            (close, cmds) = LawPanel::show(ctx, &data);
        });

        assert!(
            !close,
            "Q starts the slide-out instead of destroying the panel in the same frame"
        );
        assert!(
            cmds.is_empty(),
            "keyboard close must not synthesize law switch commands"
        );
        assert_eq!(
            ctx.data_mut(|d| d.get_persisted::<bool>(law_panel_close_requested_id())),
            Some(true)
        );

        let raw_input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::Vec2::new(960.0, 640.0),
            )),
            time: Some(1.0),
            ..Default::default()
        };
        let _ = ctx.run(raw_input, |ctx| {
            (close, cmds) = LawPanel::show(ctx, &data);
        });

        assert!(!close, "the first post-request frame starts the slide-out");
        assert!(cmds.is_empty());

        let raw_input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::Vec2::new(960.0, 640.0),
            )),
            time: Some(1.4),
            ..Default::default()
        };
        let _ = ctx.run(raw_input, |ctx| {
            (close, cmds) = LawPanel::show(ctx, &data);
        });

        assert!(
            close,
            "close is reported after the slide-out animation finishes"
        );
        assert!(cmds.is_empty());

        let profile = LawPanelProfile;
        assert_eq!(
            profile.handle_click_command("law_panel:outside", &sample_panel_data()),
            None
        );
        assert_eq!(
            profile.handle_action(
                GuiAction {
                    node_path: GuiNodePath::root(LAW_PANEL_POPUP_ROOT_TEMPLATE).child("body"),
                    kind: GuiActionKind::Click,
                },
                &sample_panel_data(),
            ),
            None
        );
    }

    #[test]
    fn gate18_missing_hoi4_gui_path_falls_back_without_breaking_switch_commands() {
        let fake_game = std::env::temp_dir().join("ironheart_law_panel_gate18_missing_gui");
        let _ = std::fs::remove_dir_all(&fake_game);
        std::fs::create_dir_all(fake_game.join("map")).unwrap();
        std::fs::create_dir_all(fake_game.join("common")).unwrap();
        let path_cfg = hoi4_paths::PathConfig::with_game_path(&fake_game);

        let diagnostics = law_panel_vanilla_diagnostics_report(&path_cfg);

        assert_eq!(diagnostics.route, LAW_PANEL_SAFE_FALLBACK_ROUTE);
        assert!(diagnostics.uses_fallback());
        assert!(diagnostics
            .fallback_reason
            .as_deref()
            .unwrap_or_default()
            .contains(LAW_PANEL_GUI_FILE));
        assert_eq!(diagnostics.missing_gui_files, vec![LAW_PANEL_GUI_FILE]);

        let data = sample_panel_data();
        let slot = data
            .slots
            .iter()
            .find(|slot| slot.category == LawCategory::Economy)
            .unwrap();
        let tier = slot
            .tiers
            .iter()
            .find(|tier| tier.id == "partial_mobilization")
            .unwrap();
        assert_eq!(
            law_switch_command(slot, tier, data.political_power),
            Some(LawCommand::SwitchLaw {
                category: LawCategory::Economy,
                target_law_id: "partial_mobilization".into()
            })
        );

        let ctx = egui::Context::default();
        let mut close = true;
        let mut commands = vec![LawCommand::SwitchLaw {
            category: LawCategory::Trade,
            target_law_id: "should_be_replaced".into(),
        }];
        let raw_input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::Vec2::new(960.0, 640.0),
            )),
            ..Default::default()
        };
        let _ = ctx.run(raw_input, |ctx| {
            (close, commands) = safe_project_style_show_law(ctx, &data, None, None);
        });
        assert!(!close);
        assert!(commands.is_empty());

        let _ = std::fs::remove_dir_all(&fake_game);
    }

    #[test]
    fn gate19_law_panel_debug_fallback_flag_is_explicit_hoi4_iron_only() {
        assert_eq!(
            LAW_PANEL_IRON_FALLBACK_ENV,
            "IRONHEART_LAW_PANEL_IRON_FALLBACK"
        );
        assert_eq!(LAW_PANEL_DEFAULT_ROUTE, "vanilla_profile");
        for value in ["1", "true", "yes", "on", "iron", "hoi4"] {
            assert!(law_panel_iron_fallback_enabled(value), "{value}");
        }
        for value in ["", "0", "false", "no", "vanilla", "safe", "v9", "legacy"] {
            assert!(!law_panel_iron_fallback_enabled(value), "{value}");
        }
    }

    #[test]
    fn gate20_law_panel_diagnostics_names_missing_resources_without_spam() {
        let fake_game = std::env::temp_dir().join("ironheart_law_panel_gate20_diagnostics");
        let _ = std::fs::remove_dir_all(&fake_game);
        std::fs::create_dir_all(fake_game.join("map")).unwrap();
        std::fs::create_dir_all(fake_game.join("common")).unwrap();
        let path_cfg = hoi4_paths::PathConfig::with_game_path(&fake_game);

        let diagnostics = law_panel_vanilla_diagnostics_report(&path_cfg);
        let summary = diagnostics.summary_line();
        let report = diagnostics.report();

        assert_eq!(summary.lines().count(), 1);
        assert!(summary.contains("route=safe_project_style"));
        assert!(summary.contains(LAW_PANEL_GUI_FILE));
        assert!(report.contains("missing_gui_files"));
        assert!(report.contains("missing_templates"));
        assert!(report.contains("missing_sprites"));
        assert!(report.contains("GFX_add_pol_idea_button"));

        let _ = std::fs::remove_dir_all(&fake_game);
    }

    #[test]
    fn gate22_law_panel_profile_actions_and_fallback_share_switch_rules() {
        let profile = LawPanelProfile;
        let data = sample_panel_data();
        let slot = data
            .slots
            .iter()
            .find(|slot| slot.category == LawCategory::Economy)
            .unwrap();
        let tier = slot
            .tiers
            .iter()
            .find(|tier| tier.id == "partial_mobilization")
            .unwrap();

        assert!(law_switch_available(slot, tier, data.political_power));
        assert_eq!(
            profile.handle_click_command("law_panel:switch:Economy:partial_mobilization", &data),
            Some(LawPanelProfileCommand::SwitchLaw {
                category: LawCategory::Economy,
                target_law_id: "partial_mobilization".into(),
            })
        );

        let mut no_pp = data.clone();
        no_pp.political_power = 10.0;
        assert!(!law_switch_available(
            &no_pp.slots[1],
            &no_pp.slots[1].tiers[1],
            no_pp.political_power
        ));
        assert_eq!(
            profile.handle_click_command("law_panel:switch:Economy:partial_mobilization", &no_pp),
            Some(LawPanelProfileCommand::Blocked {
                reason: law_unavailable_reason(
                    &no_pp.slots[1],
                    &no_pp.slots[1].tiers[1],
                    no_pp.political_power,
                )
                .unwrap(),
            })
        );
    }

    #[test]
    fn gate23_default_law_panel_route_is_vanilla_profile_with_debug_fallback() {
        assert_eq!(LAW_PANEL_DEFAULT_ROUTE, "vanilla_profile");
        assert_eq!(LAW_PANEL_SAFE_FALLBACK_ROUTE, "safe_project_style");
        assert_eq!(LAW_PANEL_IRON_DEBUG_FALLBACK_ROUTE, "hoi4_iron_debug");
        assert_eq!(
            LAW_PANEL_NEW_UI_STYLE_RULE,
            "new_or_modified_law_panel_ui_must_use_vanilla_or_hoi4_iron_assets"
        );
        assert!(!LAW_PANEL_VANILLA_RESOURCES
            .iter()
            .any(|resource| resource.source_path.ends_with(".dds")));
    }

    #[test]
    fn gate24_law_icons_cover_all_categories_without_add_button_fallback() {
        let cases = [
            (
                LawCategory::Conscription,
                "limited_conscription",
                "GFX_law_limited_conscription",
            ),
            (LawCategory::Economy, "war_economy", "GFX_law_war_economy"),
            (LawCategory::Trade, "export_focus", "GFX_law_export_focus"),
            (
                LawCategory::Taxation,
                "medium_taxation",
                "GFX_law_medium_taxation",
            ),
            (
                LawCategory::CivilRights,
                "police_state",
                "GFX_law_police_state",
            ),
            (
                LawCategory::InformationControl,
                "regulated_press",
                "GFX_law_regulated_press",
            ),
        ];
        for (category, law_id, expected) in cases {
            let sprite = law_panel_law_sprite(category, law_id);
            assert_eq!(sprite, expected);
            assert_ne!(sprite, "GFX_add_pol_idea_button");
            assert_ne!(sprite, "GFX_placeholder_bordered");
            assert!(
                LAW_PANEL_LAW_SPRITES.contains(&sprite),
                "law sprite {sprite} should be covered by startup diagnostics"
            );
        }
        assert!(!LAW_PANEL_LAW_SPRITES.contains(&"GFX_add_pol_idea_button"));
        assert!(!LAW_PANEL_LAW_SPRITES.contains(&"GFX_placeholder_bordered"));
    }

    #[test]
    fn gate26_project_law_icons_are_embedded_pngs_with_transparency() {
        let ctx = egui::Context::default();
        let fake_game = std::env::temp_dir().join("ironheart_law_icons_fake_game");
        let _ = std::fs::remove_dir_all(&fake_game);
        std::fs::create_dir_all(fake_game.join("map")).unwrap();
        std::fs::create_dir_all(fake_game.join("common")).unwrap();
        let path_cfg = hoi4_paths::PathConfig::with_game_path(&fake_game);
        let mut icon_bank = crate::icons::IconBank::new(ctx, path_cfg);

        for sprite in LAW_PANEL_LAW_SPRITES {
            assert!(sprite.starts_with("GFX_law_"), "{sprite}");
            assert!(
                icon_bank.get_or_load(sprite).is_some(),
                "project law icon {sprite} should load without a vanilla HOI4 asset"
            );
            let stats = crate::icons::IconBank::embedded_png_pixel_stats(sprite)
                .unwrap_or_else(|| panic!("{sprite} should have embedded PNG stats"));
            assert_eq!(stats.width, 64, "{sprite}");
            assert_eq!(stats.height, 64, "{sprite}");
            assert!(stats.non_transparent_pixels > 1024, "{sprite}: {stats:?}");
            assert_eq!(stats.min_alpha, 0, "{sprite} must keep transparent pixels");
            assert!(stats.max_alpha > 230, "{sprite}: {stats:?}");
            assert!(
                stats.mean_rgb[0] > 20.0 || stats.mean_rgb[1] > 20.0 || stats.mean_rgb[2] > 20.0,
                "{sprite} should not be an all-black block: {stats:?}"
            );
        }

        let _ = std::fs::remove_dir_all(&fake_game);
    }

    #[test]
    fn gate25_safe_fallback_uses_hoi4_iron_state_scope_not_debug_scope() {
        let ctx = egui::Context::default();
        let data = sample_panel_data();
        let raw_input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::Vec2::new(960.0, 640.0),
            )),
            ..Default::default()
        };

        let _ = ctx.run(raw_input, |ctx| {
            let _ = safe_project_style_show_law(ctx, &data, Some(LawCategory::Economy), None);
        });

        assert_eq!(
            ctx.data_mut(|d| d.get_persisted::<Option<LawCategory>>(egui::Id::new((
                "law_panel_hoi4_iron",
                "selected_overview_category"
            )))),
            Some(Some(LawCategory::Economy))
        );
        assert_eq!(
            ctx.data_mut(|d| d.get_persisted::<Option<LawCategory>>(egui::Id::new((
                "law_panel_hoi4_iron_debug",
                "selected_overview_category"
            )))),
            None
        );
    }

    #[test]
    fn gate23_law_panel_vanilla_route_paints_gui_root_and_tier_template_bridge() {
        let Ok(path_cfg) = hoi4_paths::PathConfig::resolve(Default::default()) else {
            return;
        };
        let Ok(context) = law_panel_vanilla_context_from_path_config(&path_cfg) else {
            return;
        };
        let profile = LawPanelProfile;
        let data = sample_panel_data();
        let root = context
            .document
            .template_index()
            .get(LAW_PANEL_POPUP_ROOT_TEMPLATE)
            .unwrap();
        let layout = crate::vanilla_gui::compute_layout_tree(
            root,
            &crate::vanilla_gui::LayoutOptions::new(crate::vanilla_gui::GuiRect::new(
                0.0, 0.0, 1280.0, 720.0,
            ))
            .shown_position(true),
        );
        let ctx = egui::Context::default();
        let mut icon_bank = crate::icons::IconBank::new(ctx.clone(), path_cfg);
        icon_bank.add_politics_search_dirs();
        let renderer = crate::vanilla_gui::VanillaGuiRenderer::new(&context.gfx_index);
        let mut root_stats = crate::vanilla_gui::RenderStats::default();
        let mut bridge_stats = crate::vanilla_gui::RenderStats::default();
        let raw_input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::Vec2::new(1280.0, 720.0),
            )),
            ..Default::default()
        };
        let _ = ctx.run(raw_input, |ctx| {
            egui::Area::new(egui::Id::new("gate23_law_vanilla_renderer")).show(ctx, |ui| {
                let bindings = bind_law_profile_tree_with_selection(
                    &profile,
                    root,
                    &data,
                    Some(LawCategory::Economy),
                );
                root_stats = renderer.paint_tree(ui, root, &layout, &bindings, &mut icon_bank);
                bridge_stats = paint_law_panel_template_instance_bridge(
                    ui,
                    &renderer,
                    &context,
                    &profile,
                    &data,
                    root,
                    &layout,
                    Some(LawCategory::Economy),
                    &mut icon_bank,
                );
            });
        });

        assert!(
            root_stats.nodes_seen >= 10,
            "root vanilla GUI tree should be rendered, got {root_stats:?}"
        );
        assert!(
            bridge_stats.nodes_seen >= data.slots[1].tiers.len(),
            "tier template bridge should render each selectable entry root, got {bridge_stats:?}"
        );
        assert!(
            bridge_stats.nodes_painted >= data.slots[1].tiers.len(),
            "custom selectable entry renderer should paint each row root, got {bridge_stats:?}"
        );
        assert!(
            bridge_stats.text_painted >= data.slots[1].tiers.len() * 2,
            "custom selectable entry renderer should paint row text directly, got {bridge_stats:?}"
        );
        assert!(
            root_stats.sprites_painted + bridge_stats.sprites_painted >= 4,
            "vanilla sprites should paint through IconBank/GfxIndex, root={root_stats:?} bridge={bridge_stats:?}"
        );
    }

    #[test]
    fn gate2_law_panel_vanilla_resource_manifest_covers_required_roles() {
        assert_eq!(LAW_PANEL_GUI_FILE, "interface/countrypoliticsview.gui");
        assert!(LAW_PANEL_TEMPLATES.contains(&LAW_PANEL_POPUP_ROOT_TEMPLATE));
        assert!(LAW_PANEL_TEMPLATES.contains(&LAW_PANEL_IDEA_CATEGORY_TEMPLATE));
        assert!(LAW_PANEL_TEMPLATES.contains(&LAW_PANEL_IDEA_SLOT_TEMPLATE));
        assert!(LAW_PANEL_TEMPLATES.contains(&LAW_PANEL_SELECTABLE_ENTRY_LIST_TEMPLATE));

        for role in [
            "gui roots and reusable templates",
            "popup background tile",
            "politics title strip",
            "politics panel background",
            "idea category row header",
            "idea category icon strip",
            "idea slot button",
            "single-column selectable idea row",
            "close button",
            "political power icon",
        ] {
            assert!(
                LAW_PANEL_VANILLA_RESOURCES
                    .iter()
                    .any(|resource| resource.role == role),
                "missing law panel vanilla resource role `{role}`"
            );
        }

        let sprite_set: BTreeSet<&str> = LAW_PANEL_SPRITES.iter().copied().collect();
        assert_eq!(
            sprite_set.len(),
            LAW_PANEL_SPRITES.len(),
            "LAW_PANEL_SPRITES should not contain duplicates"
        );
        for resource in LAW_PANEL_VANILLA_RESOURCES {
            assert!(
                resource.source_path.starts_with("interface/"),
                "resource paths stay relative to the runtime HOI4 install: {resource:?}"
            );
            if let Some(texture_path) = resource.texture_path {
                assert!(
                    texture_path.starts_with("gfx/interface/"),
                    "texture paths stay relative to the runtime HOI4 install: {resource:?}"
                );
            }
            if let Some(sprite) = resource.sprite {
                assert!(
                    sprite_set.contains(sprite),
                    "manifest sprite {sprite} must be checked by LAW_PANEL_SPRITES"
                );
            }
            assert!(
                !resource.fallback.trim().is_empty(),
                "every vanilla resource must declare a fallback: {resource:?}"
            );
        }
    }

    #[test]
    fn gate3_law_panel_sprites_hit_when_vanilla_available() {
        let Ok(path_cfg) = hoi4_paths::PathConfig::resolve(Default::default()) else {
            return;
        };
        if path_cfg.find(LAW_PANEL_GUI_FILE).is_none() {
            return;
        }

        let gfx_index = crate::vanilla_gui::GfxIndex::from_path_config(&path_cfg);
        let report = gfx_index.hit_report(LAW_PANEL_SPRITES.iter().copied());

        assert_eq!(report.requested, LAW_PANEL_SPRITES.len());
        assert!(
            report.all_hit(),
            "missing law panel vanilla sprites: {:?}",
            report.missing
        );
        assert_eq!(
            gfx_index
                .get("GFX_tiled_window2_1b_border")
                .map(|resource| &resource.kind),
            Some(&crate::vanilla_gui::GfxResourceKind::CorneredTile)
        );
        assert_eq!(
            gfx_index
                .get("GFX_idea_categories")
                .and_then(|resource| resource.frame_count),
            Some(6)
        );
        assert_eq!(
            gfx_index
                .get("GFX_idea_entry_bg_3")
                .and_then(|resource| resource.frame_count),
            Some(2)
        );
        assert!(
            gfx_index
                .get("GFX_add_pol_idea_button")
                .and_then(|resource| resource.fallback_texture_name())
                .is_some(),
            "add idea button must have a runtime texture mapping"
        );
    }

    #[test]
    fn gate4_law_panel_gui_templates_parse_when_vanilla_available() {
        let Ok(path_cfg) = hoi4_paths::PathConfig::resolve(Default::default()) else {
            return;
        };
        let Some(gui_path) = path_cfg.find(LAW_PANEL_GUI_FILE) else {
            return;
        };

        let doc = crate::vanilla_gui::parse_gui_file(gui_path).unwrap();
        let index = doc.template_index();
        for template in LAW_PANEL_TEMPLATES {
            assert!(
                index.get(template).is_some(),
                "missing law panel vanilla template {template}"
            );
        }

        let popup = index.get(LAW_PANEL_POPUP_ROOT_TEMPLATE).unwrap();
        assert_eq!(
            popup.find_node_by_name("title").map(|node| &node.kind),
            Some(&crate::vanilla_gui::GuiNodeKind::InstantTextbox)
        );
        assert_eq!(
            popup.find_node_by_name("switch").map(|node| &node.kind),
            Some(&crate::vanilla_gui::GuiNodeKind::InstantTextbox)
        );
        assert_eq!(
            popup.find_node_by_name("box_list").map(|node| &node.kind),
            Some(&crate::vanilla_gui::GuiNodeKind::GridBox)
        );
        assert_eq!(
            popup.find_node_by_name("close").map(|node| &node.kind),
            Some(&crate::vanilla_gui::GuiNodeKind::Button)
        );

        let category = index.get(LAW_PANEL_IDEA_CATEGORY_TEMPLATE).unwrap();
        assert_eq!(
            category
                .find_node_by_name("category_icon")
                .map(|node| &node.kind),
            Some(&crate::vanilla_gui::GuiNodeKind::Icon)
        );
        assert_eq!(
            category
                .find_node_by_name("ideas_grid")
                .map(|node| &node.kind),
            Some(&crate::vanilla_gui::GuiNodeKind::GridBox)
        );

        let slot = index.get(LAW_PANEL_IDEA_SLOT_TEMPLATE).unwrap();
        assert_eq!(
            slot.find_node_by_name("add_idea_button")
                .map(|node| &node.kind),
            Some(&crate::vanilla_gui::GuiNodeKind::Button)
        );

        let tier = index.get(LAW_PANEL_SELECTABLE_ENTRY_LIST_TEMPLATE).unwrap();
        for text_node in ["name", "traits", "cost", "stats"] {
            assert_eq!(
                tier.find_node_by_name(text_node).map(|node| &node.kind),
                Some(&crate::vanilla_gui::GuiNodeKind::InstantTextbox),
                "missing selectable law tier text node {text_node}"
            );
        }

        let viewport = crate::vanilla_gui::GuiRect::new(0.0, 0.0, 1920.0, 1080.0);
        let layout = crate::vanilla_gui::compute_layout_tree(
            popup,
            &crate::vanilla_gui::LayoutOptions::new(viewport).shown_position(true),
        );
        assert_eq!(layout.rect.x, 540.0);
        assert_eq!(layout.rect.y, 80.0);
        assert_eq!(layout.rect.width, 500.0);
        assert_eq!(layout.rect.height, 590.0);
    }
}
