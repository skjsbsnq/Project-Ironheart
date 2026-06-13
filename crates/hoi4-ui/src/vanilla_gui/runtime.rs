use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::PathBuf;
use std::sync::OnceLock;

use hoi4_paths::{PathConfig, PathError};

use super::diagnostics::GfxHitReport;
use super::error::VanillaGuiIssueKind;
use super::{
    collect_gfx_references, focus_spacing_marker, link_begin_marker, link_end_marker,
    link_spacing_marker, national_focus_center_marker, parse_gui_file, parse_gui_str, GfxIndex,
    GuiBinding, GuiBindingMap, GuiDocument, GuiNode, GuiNodePath, VanillaProfileDescriptor,
};

pub const COUNTRY_POLITICS_PROFILE_ID: &str = "country_politics";
pub const COUNTRY_POLITICS_GUI_FILE: &str = "interface/countrypoliticsview.gui";
pub const COUNTRY_POLITICS_ROOT: &str = "countrypoliticsview";

pub const COUNTRY_DECISION_PROFILE_ID: &str = "country_decisions";
pub const COUNTRY_DECISION_GUI_FILE: &str = "interface/countrydecisionview.gui";
pub const COUNTRY_DECISION_ROOT: &str = "countrydecisionview";

pub const NATIONAL_FOCUS_PROFILE_ID: &str = "national_focus";
pub const NATIONAL_FOCUS_GUI_FILE: &str = "interface/nationalfocusview.gui";
pub const NATIONAL_FOCUS_ROOT: &str = "nationalfocusview";

pub const COUNTRY_LOGISTICS_PROFILE_ID: &str = "country_logistics";
pub const COUNTRY_LOGISTICS_GUI_FILE: &str = "interface/countrylogisticsview.gui";
pub const COUNTRY_LOGISTICS_GFX_FILE: &str = "interface/countrylogisticsview.gfx";
pub const COUNTRY_LOGISTICS_ROOT: &str = "countrylogisticsview";

pub const COUNTRY_FINANCE_PROFILE_ID: &str = "country_finance";
pub const COUNTRY_FINANCE_GUI_FILE: &str = "interface/countryfinanceview.gui";
pub const COUNTRY_FINANCE_ROOT: &str = "countryfinanceview";

pub const COUNTRY_DIPLOMACY_PROFILE_ID: &str = "country_diplomacy";
pub const COUNTRY_DIPLOMACY_GUI_FILE: &str = "interface/countrydiplomacyview.gui";
pub const COUNTRY_DIPLOMACY_GFX_FILE: &str = "interface/countrydiplomacyview.gfx";
pub const COUNTRY_DIPLOMACY_ROOT: &str = "countrydiplomacyview";

pub const POLITICS_REQUIRED_SPRITES: &[&str] = &[
    "GFX_tiled_plain_bg",
    "GFX_header_bg",
    "GFX_pol_view_bg",
    "GFX_pol_goal_bg",
    "GFX_goal_unknown",
    "GFX_pol_leader_frame",
    "GFX_add_pol_idea_button",
    "GFX_leading_pol_party_bg",
    "GFX_pol_party_colour_bg",
    "GFX_pol_party_colour",
    "GFX_idea_traits_strip",
    "GFX_category_header",
    "GFX_idea_categories",
];

pub const DECISION_REQUIRED_SPRITES: &[&str] = &[
    "GFX_decision_item_bg",
    "GFX_decision_unknown",
    "GFX_decision_category_header_bg",
    "GFX_decision_category_end",
];

pub const FOCUS_REQUIRED_SPRITES: &[&str] = &[
    "GFX_focus_unavailable",
    "GFX_focus_can_start",
    "GFX_focus_completed",
    "GFX_technology_unavailable_item_bg",
    "GFX_ongoing_focus_goal",
    "GFX_highlight_focus_goal",
    "GFX_goal_unknown",
    "GFX_tiled_window_thin_border2",
    "GFX_tech_info_top_win",
    "GFX_generic_bg_307x113",
    "GFX_tiled_paper_bg2",
    "GFX_unit_list_header",
    "GFX_button_123x34",
    "GFX_prod_progress_bar3",
    "GFX_production_progressbar_frame2",
    "GFX_focus_link_up_down",
    "GFX_focus_link_left_right",
    "GFX_focus_link_exclusive",
    "GFX_focus_exclusive_line1",
    "GFX_focus_exclusive_line2",
];

pub const LOGISTICS_REQUIRED_SPRITES: &[&str] = &[
    "GFX_logistics_progressbar",
    "GFX_logistics_fuelbar",
    "GFX_logistics_equipment_entry_bg",
    "GFX_logistics_naval_equipment_entry_bg",
    "GFX_logistics_air_equipment_entry_bg",
    "GFX_in_stock_icon",
    "GFX_balance_icon",
    "GFX_need_icon",
    "GFX_producing_icon",
];

pub const FINANCE_REQUIRED_SPRITES: &[&str] = &[
    "GFX_tiled_window_1b_thin_border",
    "GFX_tiled_paper_bg2",
    "GFX_tiled_header",
    "GFX_tiled_window_small",
    "GFX_tiled_stats_bg",
    "GFX_tiled_button",
    "GFX_prod_progress_bar3",
    "GFX_closebutton",
    "GFX_resources_strip",
];

pub const FINANCE_KEY_TEMPLATES: &[&str] = &[
    "finance_budget_row",
    "finance_gdp_row",
    "finance_sector_row",
    "finance_employment_row",
    "finance_diagnostic_row",
];

pub const DIPLOMACY_REQUIRED_SPRITES: &[&str] = &[
    "GFX_tiled_bg",
    "GFX_tiled_window_1b_thin_border",
    "GFX_tiled_window_transparent",
    "GFX_diplo_upper_win_bg",
    "GFX_diplo_upper_diplo_bg",
    "GFX_diplo_flag_frame",
    "GFX_diplo_leader_frame",
    "GFX_tab_diplomacy_bg",
    "GFX_tab_intel_ledger",
    "GFX_diplo_opinion_bg",
    "GFX_diplo_unity_bg",
    "GFX_stability_icon",
    "GFX_war_support_icon",
    "GFX_ideology_neutrality_group",
    "GFX_ideology_communism_group",
    "GFX_ideology_fascism_group",
    "GFX_ideology_democratic_group",
    "GFX_ideology_unknown",
    "GFX_political_chart_big",
    "GFX_political_chart",
    "GFX_pol_piechart_overlay",
    "GFX_diplo_nat_spirits_bg",
    "GFX_diplo_goal_button",
    "GFX_goal_unknown",
    "GFX_activegoal_progress",
    "GFX_pol_goal_progress_frame",
    "GFX_diplo_actions_bg",
    "GFX_win_header_short",
    "GFX_diplo_relations_bg",
    "GFX_relation_war_relation",
    "GFX_relation_faction",
    "GFX_relation_wargoal",
    "GFX_relation_military_access",
    "GFX_relation_puppet",
    "GFX_relation_master",
    "GFX_diplo_countrylist_entry",
    "GFX_diplo_countrylist_flag_frame",
    "GFX_opinion_bg",
    "GFX_opinion_arrow_left",
    "GFX_opinion_arrow_right",
    "GFX_accept_decline_icon",
    "GFX_closebutton",
];

pub const DIPLOMACY_KEY_TEMPLATES: &[&str] = &[
    "diplomacy_action_entry",
    "relation_strip_view",
    "subject_relation_strip_view",
    "diplomacy_country_list_country_entry",
    "diplomacy_wargoal_entry",
];

pub const DIPLOMACY_INTEL_HIDDEN_NODES: &[&str] = &[
    "intel_ledger_container",
    "info_tab_button",
    "ideas_info",
    "trade_info",
    "estimated_enemy_force_info",
];

pub const DIPLOMACY_EXCLUDED_TEMPLATES: &[&str] = &[
    "diplomacy_espionage_mission_ideologygroup_item",
    "diplomacy_espionage_state_entry",
];

pub const COUNTRY_POLITICS_DESCRIPTOR: VanillaProfileDescriptor = VanillaProfileDescriptor {
    profile_id: COUNTRY_POLITICS_PROFILE_ID,
    root_template: COUNTRY_POLITICS_ROOT,
    required_gui_files: &[COUNTRY_POLITICS_GUI_FILE],
    template_instances: &[
        super::VanillaTemplateInstance {
            template_name: "country_politics_idea_category_entry",
            count: 3,
        },
        super::VanillaTemplateInstance {
            template_name: "political_idea_entry",
            count: 6,
        },
        super::VanillaTemplateInstance {
            template_name: "political_party_info_entry",
            count: 4,
        },
    ],
    required_sprites: POLITICS_REQUIRED_SPRITES,
    key_templates: &[
        "country_politics_idea_category_entry",
        "political_idea_entry",
        "political_party_info_entry",
    ],
};

pub const COUNTRY_DECISION_DESCRIPTOR: VanillaProfileDescriptor = VanillaProfileDescriptor {
    profile_id: COUNTRY_DECISION_PROFILE_ID,
    root_template: COUNTRY_DECISION_ROOT,
    required_gui_files: &[COUNTRY_DECISION_GUI_FILE],
    template_instances: &[],
    required_sprites: DECISION_REQUIRED_SPRITES,
    key_templates: &[
        "category_header",
        "decision_category_desc",
        "decision_item",
        "timed_decision_item",
        "category_end",
    ],
};

pub const NATIONAL_FOCUS_DESCRIPTOR: VanillaProfileDescriptor = VanillaProfileDescriptor {
    profile_id: NATIONAL_FOCUS_PROFILE_ID,
    root_template: NATIONAL_FOCUS_ROOT,
    required_gui_files: &[NATIONAL_FOCUS_GUI_FILE],
    template_instances: &[],
    required_sprites: FOCUS_REQUIRED_SPRITES,
    key_templates: &[
        "national_focus_item",
        "national_focus_link",
        "national_focus_exclusive_item",
        "national_focus_detail_view",
    ],
};

pub const COUNTRY_LOGISTICS_DESCRIPTOR: VanillaProfileDescriptor = VanillaProfileDescriptor {
    profile_id: COUNTRY_LOGISTICS_PROFILE_ID,
    root_template: COUNTRY_LOGISTICS_ROOT,
    required_gui_files: &[COUNTRY_LOGISTICS_GUI_FILE],
    template_instances: &[],
    required_sprites: LOGISTICS_REQUIRED_SPRITES,
    key_templates: &[
        "logistics_overview_land_equipment_entry",
        "logistics_overview_naval_equipment_entry",
        "logistics_overview_air_equipment_entry",
        "logistics_overview_resource_item",
        "logistics_entry_resource_item",
    ],
};

pub const COUNTRY_FINANCE_DESCRIPTOR: VanillaProfileDescriptor = VanillaProfileDescriptor {
    profile_id: COUNTRY_FINANCE_PROFILE_ID,
    root_template: COUNTRY_FINANCE_ROOT,
    required_gui_files: &[COUNTRY_FINANCE_GUI_FILE],
    template_instances: &[],
    required_sprites: FINANCE_REQUIRED_SPRITES,
    key_templates: FINANCE_KEY_TEMPLATES,
};

pub const COUNTRY_DIPLOMACY_DESCRIPTOR: VanillaProfileDescriptor = VanillaProfileDescriptor {
    profile_id: COUNTRY_DIPLOMACY_PROFILE_ID,
    root_template: COUNTRY_DIPLOMACY_ROOT,
    required_gui_files: &[COUNTRY_DIPLOMACY_GUI_FILE],
    template_instances: &[],
    required_sprites: DIPLOMACY_REQUIRED_SPRITES,
    key_templates: DIPLOMACY_KEY_TEMPLATES,
};

#[derive(Debug)]
pub struct VanillaGuiRuntimeContext {
    pub path_cfg: PathConfig,
    pub gfx_index: GfxIndex,
    documents: BTreeMap<&'static str, GuiDocument>,
}

impl VanillaGuiRuntimeContext {
    pub fn load(required_gui_files: &[&'static str]) -> Option<Self> {
        Self::load_result(required_gui_files).ok()
    }

    pub fn load_result(
        required_gui_files: &[&'static str],
    ) -> Result<Self, VanillaGuiRuntimeLoadError> {
        let path_cfg = PathConfig::resolve(Default::default())
            .map_err(VanillaGuiRuntimeLoadError::Hoi4PathUnavailable)?;
        Self::load_with_path_config_result(path_cfg, required_gui_files)
    }

    pub fn load_with_path_config(
        path_cfg: PathConfig,
        required_gui_files: &[&'static str],
    ) -> Option<Self> {
        Self::load_with_path_config_result(path_cfg, required_gui_files).ok()
    }

    pub fn load_with_path_config_result(
        path_cfg: PathConfig,
        required_gui_files: &[&'static str],
    ) -> Result<Self, VanillaGuiRuntimeLoadError> {
        let gfx_index = GfxIndex::from_path_config(&path_cfg);
        let mut documents = BTreeMap::new();
        for rel_path in required_gui_files.iter().copied() {
            let Some(gui_path) = path_cfg.find(rel_path) else {
                return Err(VanillaGuiRuntimeLoadError::MissingGuiFile {
                    relative_path: rel_path.to_owned(),
                    attempted_path: path_cfg.game_path().join(rel_path),
                });
            };
            let document = parse_gui_file(&gui_path).map_err(|error| {
                VanillaGuiRuntimeLoadError::GuiParse {
                    relative_path: rel_path.to_owned(),
                    path: gui_path,
                    message: error.to_string(),
                }
            })?;
            documents.insert(rel_path, document);
        }
        Ok(Self {
            path_cfg,
            gfx_index,
            documents,
        })
    }

    pub fn load_with_embedded(
        path_cfg: PathConfig,
        embedded_gui_files: &[(&'static str, &'static str)],
    ) -> Self {
        let gfx_index = GfxIndex::from_path_config(&path_cfg);
        let mut documents = BTreeMap::new();
        for (rel_path, gui_text) in embedded_gui_files.iter().copied() {
            let document = parse_gui_str(Some(PathBuf::from(rel_path)), gui_text);
            documents.insert(rel_path, document);
        }
        Self {
            path_cfg,
            gfx_index,
            documents,
        }
    }

    pub fn load_all_profiles(profiles: &[VanillaProfileDescriptor]) -> Option<Self> {
        Self::load_all_profiles_result(profiles).ok()
    }

    pub fn load_all_profiles_result(
        profiles: &[VanillaProfileDescriptor],
    ) -> Result<Self, VanillaGuiRuntimeLoadError> {
        let mut files = BTreeSet::new();
        for profile in profiles {
            files.extend(profile.required_gui_files.iter().copied());
        }
        let required_gui_files: Vec<&'static str> = files.into_iter().collect();
        Self::load_result(&required_gui_files)
    }

    pub fn document(&self, rel_path: &str) -> Option<&GuiDocument> {
        self.documents.get(rel_path)
    }

    pub fn root_template(&self, rel_path: &str, root_template: &str) -> Option<&GuiNode> {
        self.document(rel_path)?.template_index().get(root_template)
    }

    pub fn profile_root(&self, profile: &VanillaProfileDescriptor) -> Option<&GuiNode> {
        profile
            .required_gui_files
            .iter()
            .find_map(|file| self.root_template(file, profile.root_template))
    }

    pub fn profile_report(&self, profile: &VanillaProfileDescriptor) -> VanillaProfileLoadReport {
        VanillaProfileLoadReport::from_context(self, profile)
    }

    pub fn loaded_gui_files(&self) -> usize {
        self.documents.len()
    }

    pub fn documents(&self) -> impl Iterator<Item = (&'static str, &GuiDocument)> {
        self.documents
            .iter()
            .map(|(path, document)| (*path, document))
    }
}

#[derive(Debug, Clone)]
pub enum VanillaGuiRuntimeLoadError {
    Hoi4PathUnavailable(PathError),
    MissingGuiFile {
        relative_path: String,
        attempted_path: PathBuf,
    },
    GuiParse {
        relative_path: String,
        path: PathBuf,
        message: String,
    },
}

impl VanillaGuiRuntimeLoadError {
    pub fn category(&self) -> VanillaGuiDiagnosticCategory {
        match self {
            Self::Hoi4PathUnavailable(_) => VanillaGuiDiagnosticCategory::Hoi4Path,
            Self::MissingGuiFile { .. } => VanillaGuiDiagnosticCategory::Gui,
            Self::GuiParse { .. } => VanillaGuiDiagnosticCategory::Parser,
        }
    }
}

impl std::fmt::Display for VanillaGuiRuntimeLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Hoi4PathUnavailable(err) => write!(f, "HOI4 path unavailable: {err}"),
            Self::MissingGuiFile {
                relative_path,
                attempted_path,
            } => write!(
                f,
                "required GUI file missing: {relative_path} (looked under {})",
                attempted_path.display()
            ),
            Self::GuiParse {
                relative_path,
                path,
                message,
            } => write!(
                f,
                "required GUI parse failed for {relative_path} at {}: {message}",
                path.display()
            ),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum VanillaGuiDiagnosticCategory {
    Hoi4Path,
    Parser,
    Layout,
    Gfx,
    TextureDecode,
    Binding,
    Gui,
}

impl VanillaGuiDiagnosticCategory {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Hoi4Path => "hoi4_path",
            Self::Parser => "parser",
            Self::Layout => "layout",
            Self::Gfx => "gfx",
            Self::TextureDecode => "texture_decode",
            Self::Binding => "binding",
            Self::Gui => "gui",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VanillaGuiDiagnosticLine {
    pub category: VanillaGuiDiagnosticCategory,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VanillaGuiProfileDiagnostics {
    pub profile_id: String,
    pub runtime_available: bool,
    pub gui_loaded: bool,
    pub root_loaded: bool,
    pub missing_gui_files: Vec<String>,
    pub missing_sprites: Vec<String>,
    pub lines: Vec<VanillaGuiDiagnosticLine>,
}

impl VanillaGuiProfileDiagnostics {
    pub fn from_context(
        context: &VanillaGuiRuntimeContext,
        profile: &VanillaProfileDescriptor,
    ) -> Self {
        let report = context.profile_report(profile);
        let mut lines = Vec::new();
        if report.missing_gui_files.is_empty() {
            lines.push(VanillaGuiDiagnosticLine {
                category: VanillaGuiDiagnosticCategory::Gui,
                message: format!(
                    "loaded GUI files: {:?}; node_count={}",
                    report.loaded_gui_files, report.node_count
                ),
            });
        } else {
            lines.push(VanillaGuiDiagnosticLine {
                category: VanillaGuiDiagnosticCategory::Gui,
                message: format!("missing GUI files: {:?}", report.missing_gui_files),
            });
        }
        if report.root_loaded {
            lines.push(VanillaGuiDiagnosticLine {
                category: VanillaGuiDiagnosticCategory::Layout,
                message: format!("root template `{}` loaded", profile.root_template),
            });
        } else {
            lines.push(VanillaGuiDiagnosticLine {
                category: VanillaGuiDiagnosticCategory::Layout,
                message: format!("root template `{}` missing", profile.root_template),
            });
        }
        if !report.key_templates_missing.is_empty() {
            lines.push(VanillaGuiDiagnosticLine {
                category: VanillaGuiDiagnosticCategory::Binding,
                message: format!("missing key templates: {:?}", report.key_templates_missing),
            });
        }
        if report.gfx_hits.missing.is_empty() {
            lines.push(VanillaGuiDiagnosticLine {
                category: VanillaGuiDiagnosticCategory::Gfx,
                message: format!(
                    "required sprites mapped: {}/{}",
                    report.gfx_hits.hits, report.gfx_hits.requested
                ),
            });
        } else {
            lines.push(VanillaGuiDiagnosticLine {
                category: VanillaGuiDiagnosticCategory::Gfx,
                message: format!(
                    "missing required sprite mappings: {:?}",
                    report.gfx_hits.missing
                ),
            });
        }
        for file in profile.required_gui_files {
            if let Some(document) = context.document(file) {
                for issue in &document.diagnostics.issues {
                    let category = category_for_issue_kind(issue.kind);
                    lines.push(VanillaGuiDiagnosticLine {
                        category,
                        message: issue.detail.clone(),
                    });
                }
            }
        }
        for issue in &context.gfx_index.diagnostics().issues {
            lines.push(VanillaGuiDiagnosticLine {
                category: category_for_issue_kind(issue.kind),
                message: issue.detail.clone(),
            });
        }

        Self {
            profile_id: profile.profile_id.to_owned(),
            runtime_available: true,
            gui_loaded: report.gui_loaded,
            root_loaded: report.root_loaded,
            missing_gui_files: report.missing_gui_files,
            missing_sprites: report.gfx_hits.missing,
            lines,
        }
    }

    pub fn unavailable(
        profile: &VanillaProfileDescriptor,
        error: VanillaGuiRuntimeLoadError,
    ) -> Self {
        let mut missing_gui_files = Vec::new();
        if let VanillaGuiRuntimeLoadError::MissingGuiFile { relative_path, .. } = &error {
            missing_gui_files.push(relative_path.clone());
        }
        let lines = vec![VanillaGuiDiagnosticLine {
            category: error.category(),
            message: error.to_string().replace('\n', " "),
        }];
        Self {
            profile_id: profile.profile_id.to_owned(),
            runtime_available: false,
            gui_loaded: false,
            root_loaded: false,
            missing_gui_files,
            missing_sprites: Vec::new(),
            lines,
        }
    }

    pub fn category_names(&self) -> Vec<&'static str> {
        let mut names: Vec<&'static str> = self
            .lines
            .iter()
            .map(|line| line.category.as_str())
            .collect();
        names.sort();
        names.dedup();
        names
    }

    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(out, "## {}", self.profile_id);
        let _ = writeln!(out, "- runtime_available: {}", self.runtime_available);
        let _ = writeln!(out, "- gui_loaded: {}", self.gui_loaded);
        let _ = writeln!(out, "- root_loaded: {}", self.root_loaded);
        let _ = writeln!(out, "- missing_gui_files: {:?}", self.missing_gui_files);
        let _ = writeln!(out, "- missing_sprites: {:?}", self.missing_sprites);
        let _ = writeln!(out, "- categories: {:?}", self.category_names());
        for line in &self.lines {
            let _ = writeln!(out, "- [{}] {}", line.category.as_str(), line.message);
        }
        out
    }
}

fn category_for_issue_kind(kind: VanillaGuiIssueKind) -> VanillaGuiDiagnosticCategory {
    match kind {
        VanillaGuiIssueKind::MissingGui => VanillaGuiDiagnosticCategory::Gui,
        VanillaGuiIssueKind::MissingGfx => VanillaGuiDiagnosticCategory::Gfx,
        VanillaGuiIssueKind::MissingTexture => VanillaGuiDiagnosticCategory::TextureDecode,
        VanillaGuiIssueKind::UnknownNodeType | VanillaGuiIssueKind::ParseTolerance => {
            VanillaGuiDiagnosticCategory::Parser
        }
        VanillaGuiIssueKind::InvalidSize
        | VanillaGuiIssueKind::InvalidPosition
        | VanillaGuiIssueKind::MissingLayoutMarker => VanillaGuiDiagnosticCategory::Layout,
        VanillaGuiIssueKind::UnsupportedResourceType => VanillaGuiDiagnosticCategory::Gfx,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VanillaProfileLoadReport {
    pub profile_id: String,
    pub gui_loaded: bool,
    pub loaded_gui_files: Vec<String>,
    pub missing_gui_files: Vec<String>,
    pub root_loaded: bool,
    pub node_count: usize,
    pub key_templates_present: Vec<String>,
    pub key_templates_missing: Vec<String>,
    pub gfx_hits: GfxHitReport,
}

impl VanillaProfileLoadReport {
    pub fn from_context(
        context: &VanillaGuiRuntimeContext,
        profile: &VanillaProfileDescriptor,
    ) -> Self {
        let mut loaded_gui_files = Vec::new();
        let mut missing_gui_files = Vec::new();
        let mut node_count = 0usize;
        for rel_path in profile.required_gui_files {
            if let Some(document) = context.document(rel_path) {
                loaded_gui_files.push((*rel_path).to_owned());
                node_count += document.node_count();
            } else {
                missing_gui_files.push((*rel_path).to_owned());
            }
        }

        let root_loaded = context.profile_root(profile).is_some();
        let mut key_templates_present = Vec::new();
        let mut key_templates_missing = Vec::new();
        for template in profile.key_templates {
            let present = profile.required_gui_files.iter().any(|file| {
                context
                    .document(file)
                    .and_then(|document| document.template_index().get(template))
                    .is_some()
            });
            if present {
                key_templates_present.push((*template).to_owned());
            } else {
                key_templates_missing.push((*template).to_owned());
            }
        }

        Self {
            profile_id: profile.profile_id.to_owned(),
            gui_loaded: missing_gui_files.is_empty(),
            loaded_gui_files,
            missing_gui_files,
            root_loaded,
            node_count,
            key_templates_present,
            key_templates_missing,
            gfx_hits: context
                .gfx_index
                .hit_report(profile.required_sprites.iter().copied()),
        }
    }

    pub fn missing_sprites(&self) -> &[String] {
        &self.gfx_hits.missing
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VanillaGuiRuntimeUnavailableReport {
    pub profile_id: String,
    pub missing_gui_files: Vec<String>,
    pub reason: String,
}

impl VanillaGuiRuntimeUnavailableReport {
    pub fn for_profile(profile: &VanillaProfileDescriptor) -> Self {
        let error = VanillaGuiRuntimeContext::load_result(profile.required_gui_files).err();
        let mut missing_gui_files = Vec::new();
        let reason = if let Some(error) = error {
            if let VanillaGuiRuntimeLoadError::MissingGuiFile { relative_path, .. } = &error {
                missing_gui_files.push(relative_path.clone());
            }
            error.to_string()
        } else {
            "required vanilla runtime loaded; panel root lookup failed".to_owned()
        };
        Self {
            profile_id: profile.profile_id.to_owned(),
            missing_gui_files,
            reason,
        }
    }
}

pub fn country_politics_runtime_context() -> Option<&'static VanillaGuiRuntimeContext> {
    static CACHE: OnceLock<Option<VanillaGuiRuntimeContext>> = OnceLock::new();
    CACHE
        .get_or_init(|| {
            VanillaGuiRuntimeContext::load(COUNTRY_POLITICS_DESCRIPTOR.required_gui_files)
        })
        .as_ref()
}

pub fn country_logistics_runtime_context() -> Option<&'static VanillaGuiRuntimeContext> {
    static CACHE: OnceLock<Option<VanillaGuiRuntimeContext>> = OnceLock::new();
    CACHE
        .get_or_init(|| {
            VanillaGuiRuntimeContext::load(COUNTRY_LOGISTICS_DESCRIPTOR.required_gui_files)
        })
        .as_ref()
}

pub fn country_finance_runtime_context() -> Option<&'static VanillaGuiRuntimeContext> {
    static CACHE: OnceLock<Option<VanillaGuiRuntimeContext>> = OnceLock::new();
    CACHE
        .get_or_init(|| {
            let path_cfg = PathConfig::resolve(Default::default()).ok()?;
            Some(VanillaGuiRuntimeContext::load_with_embedded(
                path_cfg,
                &[(
                    COUNTRY_FINANCE_GUI_FILE,
                    include_str!("../../assets/interface/countryfinanceview.gui"),
                )],
            ))
        })
        .as_ref()
}

pub fn country_diplomacy_runtime_context() -> Option<&'static VanillaGuiRuntimeContext> {
    static CACHE: OnceLock<Option<VanillaGuiRuntimeContext>> = OnceLock::new();
    CACHE
        .get_or_init(|| {
            VanillaGuiRuntimeContext::load(COUNTRY_DIPLOMACY_DESCRIPTOR.required_gui_files)
        })
        .as_ref()
}

pub fn vanilla_builtin_profile_descriptors() -> &'static [VanillaProfileDescriptor] {
    &[
        COUNTRY_POLITICS_DESCRIPTOR,
        COUNTRY_DECISION_DESCRIPTOR,
        NATIONAL_FOCUS_DESCRIPTOR,
        COUNTRY_LOGISTICS_DESCRIPTOR,
        COUNTRY_DIPLOMACY_DESCRIPTOR,
    ]
}

pub fn vanilla_builtin_runtime_context() -> Option<&'static VanillaGuiRuntimeContext> {
    static CACHE: OnceLock<Option<VanillaGuiRuntimeContext>> = OnceLock::new();
    CACHE
        .get_or_init(|| {
            VanillaGuiRuntimeContext::load_all_profiles(vanilla_builtin_profile_descriptors())
        })
        .as_ref()
}

pub fn collect_profile_gfx_references(
    context: &VanillaGuiRuntimeContext,
    profile: &VanillaProfileDescriptor,
) -> Vec<String> {
    let mut refs = Vec::new();
    for file in profile.required_gui_files {
        if let Some(document) = context.document(file) {
            refs.extend(collect_gfx_references(document));
        }
    }
    refs.sort();
    refs.dedup();
    refs
}

pub fn country_diplomacy_no_intel_bindings(root: &GuiNode) -> GuiBindingMap {
    let mut bindings = GuiBindingMap::default();
    collect_country_diplomacy_no_intel_bindings(
        root,
        GuiNodePath::root(root.path_label(0)),
        &mut bindings,
    );
    bindings
}

pub fn country_diplomacy_is_excluded_template(template_name: &str) -> bool {
    DIPLOMACY_EXCLUDED_TEMPLATES.contains(&template_name)
}

fn collect_country_diplomacy_no_intel_bindings(
    node: &GuiNode,
    path: GuiNodePath,
    bindings: &mut GuiBindingMap,
) {
    if node
        .name
        .as_deref()
        .is_some_and(|name| DIPLOMACY_INTEL_HIDDEN_NODES.contains(&name))
    {
        bindings.insert_path(path.clone(), GuiBinding::default().visible(false));
    }

    for (index, child) in node.children.iter().enumerate() {
        collect_country_diplomacy_no_intel_bindings(
            child,
            path.child(child.path_label(index)),
            bindings,
        );
    }
}

pub fn required_focus_marker_report(root: &GuiNode) -> BTreeMap<&'static str, (f32, f32)> {
    let mut out = BTreeMap::new();
    for (name, marker) in [
        ("focus_spacing", focus_spacing_marker(root)),
        ("national_focus_center", national_focus_center_marker(root)),
        ("link_spacing", link_spacing_marker(root)),
        ("link_begin", link_begin_marker(root)),
        ("link_end", link_end_marker(root)),
    ] {
        let position = marker.position();
        out.insert(name, (position.x, position.y));
    }
    out
}

pub fn vanilla_profiles_diagnostics_markdown(
    context: Option<&VanillaGuiRuntimeContext>,
    profiles: &[VanillaProfileDescriptor],
) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "# Vanilla GUI Runtime Profiles");
    let _ = writeln!(out);
    if let Some(context) = context {
        let gfx_diagnostics = context.gfx_index.diagnostics();
        let _ = writeln!(out, "- runtime_available: true");
        let _ = writeln!(out, "- gui_loaded: {}", context.loaded_gui_files());
        let _ = writeln!(
            out,
            "- gfx_files_loaded: {}",
            gfx_diagnostics.loaded_gfx_files
        );
        let _ = writeln!(
            out,
            "- gfx_resources_loaded: {}",
            gfx_diagnostics.resource_definitions
        );
        for profile in profiles {
            let report = context.profile_report(profile);
            let _ = writeln!(out);
            let _ = writeln!(out, "## {}", profile.profile_id);
            let _ = writeln!(out, "- gui_loaded: {}", report.gui_loaded);
            let _ = writeln!(out, "- root_loaded: {}", report.root_loaded);
            let _ = writeln!(out, "- loaded_gui_files: {:?}", report.loaded_gui_files);
            let _ = writeln!(out, "- missing_gui_files: {:?}", report.missing_gui_files);
            let _ = writeln!(out, "- node_count: {}", report.node_count);
            let _ = writeln!(
                out,
                "- key_templates_present: {:?}",
                report.key_templates_present
            );
            let _ = writeln!(
                out,
                "- key_templates_missing: {:?}",
                report.key_templates_missing
            );
            let _ = writeln!(
                out,
                "- gfx_hits: {}/{}",
                report.gfx_hits.hits, report.gfx_hits.requested
            );
            let _ = writeln!(out, "- missing_sprites: {:?}", report.gfx_hits.missing);
        }
    } else {
        let _ = writeln!(out, "- runtime_available: false");
        for profile in profiles {
            let report = VanillaGuiRuntimeUnavailableReport::for_profile(profile);
            let _ = writeln!(out);
            let _ = writeln!(out, "## {}", profile.profile_id);
            let _ = writeln!(out, "- gui_loaded: false");
            let _ = writeln!(out, "- reason: {}", report.reason.replace('\n', " "));
            let _ = writeln!(out, "- missing_gui_files: {:?}", report.missing_gui_files);
        }
    }
    out
}

pub fn vanilla_profile_diagnostics_markdown(
    context: Option<&VanillaGuiRuntimeContext>,
    profile: &VanillaProfileDescriptor,
) -> String {
    let diagnostics = match context {
        Some(context) => VanillaGuiProfileDiagnostics::from_context(context, profile),
        None => match VanillaGuiRuntimeContext::load_result(profile.required_gui_files) {
            Ok(context) => VanillaGuiProfileDiagnostics::from_context(&context, profile),
            Err(error) => VanillaGuiProfileDiagnostics::unavailable(profile, error),
        },
    };
    diagnostics.to_markdown()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vanilla_gui::VanillaProfileRegistry;

    #[test]
    fn builtin_descriptors_cover_three_target_profiles() {
        let mut registry = VanillaProfileRegistry::new();
        registry.register_descriptor(COUNTRY_POLITICS_DESCRIPTOR);
        registry.register_descriptor(COUNTRY_DECISION_DESCRIPTOR);
        registry.register_descriptor(NATIONAL_FOCUS_DESCRIPTOR);
        registry.register_descriptor(COUNTRY_LOGISTICS_DESCRIPTOR);
        registry.register_descriptor(COUNTRY_DIPLOMACY_DESCRIPTOR);

        assert_eq!(registry.len(), 5);
        assert!(registry.get(COUNTRY_POLITICS_PROFILE_ID).is_some());
        assert!(registry.get(COUNTRY_DECISION_PROFILE_ID).is_some());
        assert!(registry.get(NATIONAL_FOCUS_PROFILE_ID).is_some());
        assert!(registry.get(COUNTRY_LOGISTICS_PROFILE_ID).is_some());
        assert!(registry.get(COUNTRY_DIPLOMACY_PROFILE_ID).is_some());
        assert_eq!(
            registry
                .get(COUNTRY_DECISION_PROFILE_ID)
                .unwrap()
                .required_gui_files,
            &[COUNTRY_DECISION_GUI_FILE]
        );
        assert!(registry
            .get(NATIONAL_FOCUS_PROFILE_ID)
            .unwrap()
            .required_sprites
            .contains(&"GFX_focus_unavailable"));
    }

    #[test]
    fn politics_runtime_context_loads_root_when_vanilla_available() {
        let Some(context) = country_politics_runtime_context() else {
            return;
        };
        assert!(context
            .root_template(COUNTRY_POLITICS_GUI_FILE, COUNTRY_POLITICS_ROOT)
            .is_some());
        assert!(!context.gfx_index.is_empty());
        assert_eq!(context.loaded_gui_files(), 1);
    }

    #[test]
    fn load_with_embedded_parses_minimal_gui_root() {
        let path_cfg = PathConfig::with_game_path(
            std::env::temp_dir().join("ironheart-load-with-embedded-test"),
        );
        let context = VanillaGuiRuntimeContext::load_with_embedded(
            path_cfg,
            &[(
                "interface/embedded_test.gui",
                r#"
guiTypes = {
    containerWindowType = {
        name = "embedded_test_root"
        size = { width = 10 height = 10 }
    }
}
"#,
            )],
        );

        assert_eq!(context.loaded_gui_files(), 1);
        assert!(context
            .root_template("interface/embedded_test.gui", "embedded_test_root")
            .is_some());
    }

    #[test]
    fn country_finance_descriptor_uses_embedded_gui_key() {
        assert_eq!(COUNTRY_FINANCE_PROFILE_ID, "country_finance");
        assert_eq!(COUNTRY_FINANCE_GUI_FILE, "interface/countryfinanceview.gui");
        assert_eq!(COUNTRY_FINANCE_ROOT, "countryfinanceview");
        assert_eq!(
            COUNTRY_FINANCE_DESCRIPTOR.profile_id,
            COUNTRY_FINANCE_PROFILE_ID
        );
        assert_eq!(
            COUNTRY_FINANCE_DESCRIPTOR.required_gui_files,
            &[COUNTRY_FINANCE_GUI_FILE]
        );
        assert!(!vanilla_builtin_profile_descriptors()
            .iter()
            .any(|profile| profile.profile_id == COUNTRY_FINANCE_PROFILE_ID));
    }

    #[test]
    fn finance_runtime_context_loads_embedded_root_when_vanilla_available() {
        let Some(context) = country_finance_runtime_context() else {
            return;
        };

        assert!(context
            .root_template(COUNTRY_FINANCE_GUI_FILE, COUNTRY_FINANCE_ROOT)
            .is_some());
        assert_eq!(context.loaded_gui_files(), 1);
    }

    #[test]
    fn profile_diagnostics_report_is_clear_without_panics() {
        let profiles = vanilla_builtin_profile_descriptors();
        let report =
            vanilla_profiles_diagnostics_markdown(vanilla_builtin_runtime_context(), profiles);

        assert!(report.contains("country_politics"));
        assert!(report.contains("country_decisions"));
        assert!(report.contains("national_focus"));
        assert!(report.contains("country_logistics"));
        assert!(report.contains("country_diplomacy"));
        assert!(report.contains("gui_loaded:"));
        assert!(report.contains("gfx_hits:") || report.contains("runtime_available: false"));
        assert!(report.contains("missing_sprites:") || report.contains("reason:"));
    }
}
