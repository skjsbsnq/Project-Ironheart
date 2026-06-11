use egui::Color32;

use crate::country_info_panel::{CountryInfoCommand, CountryInfoData};
use crate::diplomacy::{
    CountryDiplomacyDetail, CountryEntry, DiplomacyActionCommand, DiplomacyActionEntry,
    DiplomacyCommand, DiplomacyData, DiplomacyRelationEntry, DiplomacyRelationKind,
};
use crate::icons::IconBank;
use crate::vanilla_gui::{
    bind_profile_tree, bind_profile_tree_with_path_and_context, grid_slots, template_instance_path,
    GfxIndex, GuiAction, GuiActionKind, GuiBinding, GuiBindingMap, GuiInstanceContext, GuiNode,
    GuiNodePath, GuiRect, GuiRuntimeFrame, GuiRuntimeFrameInput, GuiRuntimeInstanceSource,
    GuiRuntimeInstanceSpec, GuiRuntimeState, GuiTemplateRegistry, TemplateInstanceOptions,
    VanillaPanelProfile, VanillaTemplateInstance, COUNTRY_DIPLOMACY_DESCRIPTOR,
    COUNTRY_DIPLOMACY_GUI_FILE, COUNTRY_DIPLOMACY_PROFILE_ID, COUNTRY_DIPLOMACY_ROOT,
    DIPLOMACY_INTEL_HIDDEN_NODES, DIPLOMACY_KEY_TEMPLATES, DIPLOMACY_REQUIRED_SPRITES,
};
use crate::{ActiveDetailPanel, CountryDetailTarget, PanelCommand};

pub const COUNTRY_DIPLOMACY_CLOSE_COMMAND: &str = "close";
pub const COUNTRY_DIPLOMACY_ESCAPE_COMMAND: &str = "escape";
pub const COUNTRY_DIPLOMACY_ACTION_PREFIX: &str = "diplomacy:action:";
pub const DIPLOMACY_PANEL_SELECT_COUNTRY_PREFIX: &str = "diplomacy:list:select:";
pub const DIPLOMACY_PANEL_SORT_NAME_COMMAND: &str = "diplomacy:list:sort:name";
pub const DIPLOMACY_PANEL_SORT_OPINION_COMMAND: &str = "diplomacy:list:sort:opinion";
pub const DIPLOMACY_PANEL_BACK_COMMAND: &str = "diplomacy:list:back";

pub struct CountryDiplomacyProfile;

impl VanillaPanelProfile for CountryDiplomacyProfile {
    type Data = CountryInfoData;
    type Command = CountryInfoCommand;

    fn profile_id(&self) -> &'static str {
        COUNTRY_DIPLOMACY_PROFILE_ID
    }

    fn root_template(&self) -> &'static str {
        COUNTRY_DIPLOMACY_ROOT
    }

    fn required_gui_files(&self) -> &'static [&'static str] {
        &[COUNTRY_DIPLOMACY_GUI_FILE]
    }

    fn template_instances(&self) -> &'static [VanillaTemplateInstance] {
        &[]
    }

    fn required_sprites(&self) -> &'static [&'static str] {
        DIPLOMACY_REQUIRED_SPRITES
    }

    fn key_templates(&self) -> &'static [&'static str] {
        DIPLOMACY_KEY_TEMPLATES
    }

    fn bind_node(&self, node_path: &GuiNodePath, data: &Self::Data) -> GuiBinding {
        let name = node_name(node_path);
        if DIPLOMACY_INTEL_HIDDEN_NODES.contains(&name) {
            return GuiBinding::default().visible(false);
        }

        match name {
            "country_list" | "top_filter_window" | "filters" | "countries" => {
                GuiBinding::default().visible(false)
            }
            "player_info_box" | "diplomacy_bottom" | "back_button" | "logistics_button" => {
                GuiBinding::default().visible(false)
            }
            "close_button" => GuiBinding::default()
                .click(COUNTRY_DIPLOMACY_CLOSE_COMMAND)
                .tooltip("Close diplomacy panel"),
            "diplomacy_title" => GuiBinding::default().text(crate::i18n::tr("diplomacy")),
            "relations_tab_button" => {
                GuiBinding::default().text(crate::i18n::tr("DIPLOMACY_RELATIONS_TAB"))
            }
            "diplo_country_flag" => GuiBinding::default()
                .sprite(&data.flag_gfx)
                .layout_size(82.0, 52.0)
                .tooltip(country_title(data)),
            "ideology_icon" => ideology_icon_binding(&data.ruling_party, &data.ruling_party_label)
                .layout_size(66.0, 68.0),
            "country_name" => GuiBinding::default().text(country_title(data)),
            "faction_name" => GuiBinding::default().text(
                data.faction_name
                    .as_deref()
                    .filter(|value| !value.is_empty())
                    .unwrap_or_else(|| crate::i18n::tr("no_faction")),
            ),
            "leader_name" => GuiBinding::default().text(non_empty_or(
                data.leader_name.as_str(),
                crate::i18n::tr("leader_unknown"),
            )),
            "our_opinion_value" => GuiBinding::default()
                .text(format_signed(data.our_opinion_of_target))
                .text_color(opinion_color(data.our_opinion_of_target)),
            "their_opinion_value" => GuiBinding::default()
                .text(format_signed(data.their_opinion_of_us))
                .text_color(opinion_color(data.their_opinion_of_us)),
            "our_opinion_left_flag" => GuiBinding::default()
                .sprite(&data.player_flag_gfx)
                .layout_size(24.0, 16.0),
            "their_opinion_left_flag" => GuiBinding::default()
                .sprite(&data.flag_gfx)
                .layout_size(24.0, 16.0),
            "stability_value" => GuiBinding::default().text(format_percent(data.stability)),
            "war_support_value" => GuiBinding::default().text(format_percent(data.war_support)),
            "leader_portrait" => GuiBinding::default()
                .sprite(
                    data.leader_portrait_key
                        .as_deref()
                        .filter(|value| !value.is_empty())
                        .unwrap_or("GFX_leader_unknown"),
                )
                .tooltip(non_empty_or(
                    data.leader_name.as_str(),
                    crate::i18n::tr("leader_unknown"),
                )),
            "political_pie_chart" => GuiBinding::default().visible(true),
            "chart" if is_political_pie_chart_node(node_path) => party_pie_binding(
                &data.ruling_party,
                &data.ruling_party_label,
                &data.party_popularity,
            ),
            "party_name" => GuiBinding::default().text(non_empty_or(
                preferred_party_name(data).as_str(),
                crate::i18n::tr("unknown"),
            )),
            "ideology_name" => GuiBinding::default().text(non_empty_or(
                data.ruling_party_label.as_str(),
                crate::i18n::tr("unknown"),
            )),
            "elections" => GuiBinding::default().visible(false),
            "active_national_focus_info" | "national_spirit_info" => {
                GuiBinding::default().visible(true)
            }
            "show_national_goal_button" => GuiBinding::default()
                .enabled(false)
                .tooltip(crate::i18n::tr("unknown_national_focus")),
            "goal_icon" => GuiBinding::default()
                .sprite("GFX_goal_unknown")
                .tooltip(crate::i18n::tr("unknown_national_focus")),
            "national_focus_label" if path_contains(node_path, "active_national_focus_info") => {
                GuiBinding::default().text(crate::i18n::tr("unknown_national_focus"))
            }
            "progress" if path_contains(node_path, "active_national_focus_info") => {
                GuiBinding::default().progress(0.0)
            }
            "spirit_title" => GuiBinding::default().text(crate::i18n::tr("national_spirits")),
            "national_spirit_ideas_grid" | "nat_spirit_ideas_grid_over_defined" => {
                GuiBinding::default().instances(0)
            }
            "relations_info" => GuiBinding::default().visible(true),
            "relations_grid" => GuiBinding::default().instances(data.relations.len()),
            "diplomatic_actions" => GuiBinding::default().visible(!data.actions.is_empty()),
            "actions_grid" => GuiBinding::default().instances(data.actions.len()),
            _ => GuiBinding::default(),
        }
    }

    fn bind_node_with_context(
        &self,
        node_path: &GuiNodePath,
        data: &Self::Data,
        instance: Option<&GuiInstanceContext>,
    ) -> GuiBinding {
        let Some(instance) = instance else {
            return self.bind_node(node_path, data);
        };
        let name = instance_node_name(node_path, instance);
        match instance.semantic_role.as_deref() {
            Some("diplomacy_relation_entry") => {
                bind_relation_node(name, find_relation(data, instance.model_key.as_deref()))
            }
            Some("diplomacy_action_entry") => {
                bind_action_node(name, find_action(data, instance.model_key.as_deref()))
            }
            _ => self.bind_node(node_path, data),
        }
    }

    fn handle_action(&self, action: GuiAction, data: &Self::Data) -> Option<Self::Command> {
        if action.kind != GuiActionKind::Click {
            return None;
        }
        let command = action.node_path.to_string();
        if is_close_command(&command) {
            return Some(CountryInfoCommand::Close);
        }
        let action_id = command.strip_prefix(COUNTRY_DIPLOMACY_ACTION_PREFIX)?;
        let entry = data.actions.iter().find(|entry| entry.id == action_id)?;
        if !entry.enabled {
            return None;
        }
        country_info_command_from_action(&entry.command)
    }

    fn uses_slide_animation(&self) -> bool {
        true
    }
}

pub struct DiplomacyPanelProfile;

impl VanillaPanelProfile for DiplomacyPanelProfile {
    type Data = DiplomacyData;
    type Command = DiplomacyCommand;

    fn profile_id(&self) -> &'static str {
        COUNTRY_DIPLOMACY_PROFILE_ID
    }

    fn root_template(&self) -> &'static str {
        COUNTRY_DIPLOMACY_ROOT
    }

    fn required_gui_files(&self) -> &'static [&'static str] {
        &[COUNTRY_DIPLOMACY_GUI_FILE]
    }

    fn template_instances(&self) -> &'static [VanillaTemplateInstance] {
        &[]
    }

    fn required_sprites(&self) -> &'static [&'static str] {
        DIPLOMACY_REQUIRED_SPRITES
    }

    fn key_templates(&self) -> &'static [&'static str] {
        DIPLOMACY_KEY_TEMPLATES
    }

    fn bind_node(&self, node_path: &GuiNodePath, data: &Self::Data) -> GuiBinding {
        let name = node_name(node_path);
        if DIPLOMACY_INTEL_HIDDEN_NODES.contains(&name) {
            return GuiBinding::default().visible(false);
        }

        let selected_country = diplomacy_panel_selected_country(data);
        let detail_mode = selected_country.is_some_and(|country| country.selected);
        let selected_detail = selected_country.and_then(|country| country.detail.as_ref());

        match name {
            "country_info" => GuiBinding::default().visible(detail_mode),
            "country_list" | "top_filter_window" | "filters" | "countries" => {
                GuiBinding::default().visible(!detail_mode)
            }
            "countries_grid" => GuiBinding::default().instances(data.countries.len()),
            "sort_alphabetical" => GuiBinding::default()
                .text("Country")
                .click(DIPLOMACY_PANEL_SORT_NAME_COMMAND)
                .tooltip("Sort countries by name"),
            "opinion_flag1" | "opinion_flag2" | "our_opinion_icon" | "their_opinion_icon" => {
                GuiBinding::default()
                    .click(DIPLOMACY_PANEL_SORT_OPINION_COMMAND)
                    .tooltip("Sort countries by opinion")
            }
            "close_button" => GuiBinding::default()
                .click(COUNTRY_DIPLOMACY_CLOSE_COMMAND)
                .tooltip("Close diplomacy panel"),
            "diplomacy_bottom" => GuiBinding::default().visible(detail_mode),
            "back_button" => GuiBinding::default()
                .visible(detail_mode)
                .text(crate::i18n::tr("OPEN_COUNTRY_LIST"))
                .click(DIPLOMACY_PANEL_BACK_COMMAND)
                .tooltip("Back to country list"),
            "logistics_button" => GuiBinding::default()
                .visible(detail_mode)
                .enabled(false)
                .text(crate::i18n::tr("OPEN_COUNTRY_LOGISTICS"))
                .tooltip("Country logistics link is not available from diplomacy yet"),
            "diplomacy_title" => GuiBinding::default().text(crate::i18n::tr("diplomacy")),
            "relations_tab_button" => {
                GuiBinding::default().text(crate::i18n::tr("DIPLOMACY_RELATIONS_TAB"))
            }
            "player_name" => GuiBinding::default().text(&data.player_tag),
            "diplo_country_flag" => selected_country
                .map(|country| {
                    GuiBinding::default()
                        .sprite(&country.flag_gfx)
                        .layout_size(82.0, 52.0)
                        .tooltip(country_entry_title(country))
                })
                .unwrap_or_else(|| GuiBinding::default().visible(false)),
            "ideology_icon" => selected_country
                .map(|country| {
                    ideology_icon_binding(&country.ruling_party, &country.ruling_party_label)
                        .layout_size(66.0, 68.0)
                })
                .unwrap_or_else(|| GuiBinding::default().visible(false)),
            "country_name" => selected_country
                .map(|country| GuiBinding::default().text(country_entry_title(country)))
                .unwrap_or_default(),
            "faction_name" => GuiBinding::default().text(
                selected_detail
                    .and_then(|detail| detail.faction_name.as_deref())
                    .filter(|value| !value.is_empty())
                    .unwrap_or_else(|| crate::i18n::tr("no_faction")),
            ),
            "leader_name" => selected_country
                .map(|country| {
                    GuiBinding::default().text(non_empty_or(
                        country.leader_name.as_str(),
                        crate::i18n::tr("leader_unknown"),
                    ))
                })
                .unwrap_or_default(),
            "our_opinion_value" => selected_country
                .map(|country| {
                    GuiBinding::default()
                        .text(format_signed(country.our_opinion_of_target))
                        .text_color(opinion_color(country.our_opinion_of_target))
                })
                .unwrap_or_default(),
            "their_opinion_value" => selected_country
                .map(|country| {
                    GuiBinding::default()
                        .text(format_signed(country.their_opinion_of_us))
                        .text_color(opinion_color(country.their_opinion_of_us))
                })
                .unwrap_or_default(),
            "our_opinion_left_flag" => GuiBinding::default()
                .sprite(&data.player_flag_gfx)
                .layout_size(24.0, 16.0),
            "their_opinion_left_flag" => selected_country
                .map(|country| {
                    GuiBinding::default()
                        .sprite(&country.flag_gfx)
                        .layout_size(24.0, 16.0)
                })
                .unwrap_or_default(),
            "stability_bg" | "stability_icon" | "stability_value" | "war_support_bg"
            | "war_support_icon" | "war_support_value" => GuiBinding::default().visible(false),
            "leader_portrait" => selected_country
                .map(|country| {
                    GuiBinding::default()
                        .sprite(
                            country
                                .leader_portrait_key
                                .as_deref()
                                .filter(|value| !value.is_empty())
                                .unwrap_or("GFX_leader_unknown"),
                        )
                        .tooltip(non_empty_or(
                            country.leader_name.as_str(),
                            crate::i18n::tr("leader_unknown"),
                        ))
                })
                .unwrap_or_default(),
            "elections" | "ideas_info" | "trade_info" | "estimated_enemy_force_info" => {
                GuiBinding::default().visible(false)
            }
            "active_national_focus_info" | "national_spirit_info" => {
                GuiBinding::default().visible(detail_mode)
            }
            "show_national_goal_button" => GuiBinding::default()
                .enabled(false)
                .tooltip(crate::i18n::tr("unknown_national_focus")),
            "goal_icon" => GuiBinding::default()
                .sprite("GFX_goal_unknown")
                .tooltip(crate::i18n::tr("unknown_national_focus")),
            "national_focus_label" if path_contains(node_path, "active_national_focus_info") => {
                GuiBinding::default().text(crate::i18n::tr("unknown_national_focus"))
            }
            "progress" if path_contains(node_path, "active_national_focus_info") => {
                GuiBinding::default().progress(0.0)
            }
            "spirit_title" => GuiBinding::default().text(crate::i18n::tr("national_spirits")),
            "national_spirit_ideas_grid" | "nat_spirit_ideas_grid_over_defined" => {
                GuiBinding::default().instances(0)
            }
            "political_pie_chart" => selected_country
                .map(|_| GuiBinding::default().visible(true))
                .unwrap_or_else(|| GuiBinding::default().visible(false)),
            "chart" if is_political_pie_chart_node(node_path) => selected_country
                .map(|country| {
                    party_pie_binding(
                        &country.ruling_party,
                        &country.ruling_party_label,
                        &country.party_popularity,
                    )
                })
                .unwrap_or_else(|| GuiBinding::default().visible(false)),
            "party_name" => selected_country
                .map(|country| GuiBinding::default().text(preferred_country_party_name(country)))
                .unwrap_or_default(),
            "ideology_name" => selected_country
                .map(|country| {
                    GuiBinding::default().text(non_empty_or(
                        country.ruling_party_label.as_str(),
                        crate::i18n::tr("unknown"),
                    ))
                })
                .unwrap_or_default(),
            "relations_info" => GuiBinding::default().visible(detail_mode),
            "relations_grid" => GuiBinding::default()
                .instances(selected_detail.map(|d| d.relations.len()).unwrap_or(0)),
            "diplomatic_actions" => GuiBinding::default()
                .visible(selected_detail.is_some_and(|d| !d.actions.is_empty())),
            "actions_grid" => GuiBinding::default()
                .instances(selected_detail.map(|d| d.actions.len()).unwrap_or(0)),
            _ => GuiBinding::default(),
        }
    }

    fn bind_node_with_context(
        &self,
        node_path: &GuiNodePath,
        data: &Self::Data,
        instance: Option<&GuiInstanceContext>,
    ) -> GuiBinding {
        let Some(instance) = instance else {
            return self.bind_node(node_path, data);
        };
        let name = instance_node_name(node_path, instance);
        match instance.semantic_role.as_deref() {
            Some("diplomacy_country_list_country_entry") => {
                bind_country_list_node(name, find_country(data, instance.model_key.as_deref()))
            }
            Some("diplomacy_country_list_relation_entry") => bind_country_list_relation_node(
                name,
                find_country_list_relation(data, instance.model_key.as_deref()),
            ),
            Some("diplomacy_relation_entry") => bind_relation_node(
                name,
                find_panel_relation(data, instance.model_key.as_deref()),
            ),
            Some("diplomacy_action_entry") => {
                bind_action_node(name, find_panel_action(data, instance.model_key.as_deref()))
            }
            _ => self.bind_node(node_path, data),
        }
    }

    fn handle_action(&self, action: GuiAction, data: &Self::Data) -> Option<Self::Command> {
        if action.kind != GuiActionKind::Click {
            return None;
        }
        let command = action.node_path.to_string();
        if is_close_command(&command) {
            return Some(DiplomacyCommand::Panel(PanelCommand::ClosePrimary));
        }
        if let Some(tag) = command.strip_prefix(DIPLOMACY_PANEL_SELECT_COUNTRY_PREFIX) {
            return Some(DiplomacyCommand::Panel(PanelCommand::OpenDetail(
                ActiveDetailPanel::Country(CountryDetailTarget {
                    tag: tag.to_owned(),
                }),
            )));
        }
        let action_id = command.strip_prefix(COUNTRY_DIPLOMACY_ACTION_PREFIX)?;
        let entry = diplomacy_panel_selected_detail(data)?
            .actions
            .iter()
            .find(|entry| entry.id == action_id)?;
        if !entry.enabled {
            return None;
        }
        diplomacy_command_from_action(&entry.command)
    }

    fn uses_slide_animation(&self) -> bool {
        true
    }
}

#[derive(Debug, Clone)]
pub struct CountryDiplomacyVanillaRuntimeFrameParts {
    pub frame: GuiRuntimeFrame,
    pub bindings: GuiBindingMap,
}

pub fn country_diplomacy_vanilla_runtime_frame(
    root: &GuiNode,
    data: &CountryInfoData,
    viewport: GuiRect,
    runtime_state: GuiRuntimeState,
    registry: GuiTemplateRegistry<'_>,
) -> GuiRuntimeFrame {
    country_diplomacy_vanilla_runtime_frame_parts(
        root,
        data,
        viewport,
        runtime_state,
        registry,
        None,
        None,
    )
    .frame
}

pub fn country_diplomacy_vanilla_runtime_frame_parts(
    root: &GuiNode,
    data: &CountryInfoData,
    viewport: GuiRect,
    runtime_state: GuiRuntimeState,
    registry: GuiTemplateRegistry<'_>,
    gfx_index: Option<&GfxIndex>,
    icon_bank: Option<&mut IconBank>,
) -> CountryDiplomacyVanillaRuntimeFrameParts {
    let profile = CountryDiplomacyProfile;
    let mut bindings = bind_profile_tree(&profile, root, data);
    let base = GuiRuntimeFrame::build(
        GuiRuntimeFrameInput::new(root, viewport, profile.profile_id(), &bindings)
            .with_runtime_state(runtime_state.clone()),
    );

    let mut specs = Vec::new();
    if let Some(relations_grid) = base.root_layout.find_by_name("relations_grid") {
        let relations_grid_node = find_gui_node_by_layout_path(root, &relations_grid.path)
            .or_else(|| root.find_node_by_name("relations_grid"));
        if let Some(relations_grid_node) = relations_grid_node {
            specs.extend(country_diplomacy_relation_instance_specs(
                data,
                relations_grid.path.clone(),
                relations_grid_node,
                relations_grid.rect,
            ));
        }
    }
    if let Some(actions_grid) = base.root_layout.find_by_name("actions_grid") {
        let actions_grid_node = find_gui_node_by_layout_path(root, &actions_grid.path)
            .or_else(|| root.find_node_by_name("actions_grid"));
        if let Some(actions_grid_node) = actions_grid_node {
            specs.extend(country_diplomacy_action_instance_specs(
                data,
                actions_grid.path.clone(),
                actions_grid_node,
                actions_grid.rect,
            ));
        }
    }

    bindings.extend(country_diplomacy_vanilla_instance_bindings(
        data, &specs, &registry,
    ));
    let mut input = GuiRuntimeFrameInput::new(root, viewport, profile.profile_id(), &bindings)
        .with_runtime_state(runtime_state)
        .with_template_registry(registry)
        .with_profile_descriptor(&COUNTRY_DIPLOMACY_DESCRIPTOR)
        .with_instance_specs(specs);
    if let Some(gfx_index) = gfx_index {
        input = input.with_gfx_index(gfx_index);
    }
    if let Some(icon_bank) = icon_bank {
        input = input.with_icon_bank(icon_bank);
    }

    CountryDiplomacyVanillaRuntimeFrameParts {
        frame: GuiRuntimeFrame::build(input),
        bindings,
    }
}

#[derive(Debug, Clone)]
pub struct DiplomacyPanelVanillaRuntimeFrameParts {
    pub frame: GuiRuntimeFrame,
    pub bindings: GuiBindingMap,
}

pub fn diplomacy_panel_vanilla_runtime_frame_parts(
    root: &GuiNode,
    data: &DiplomacyData,
    viewport: GuiRect,
    runtime_state: GuiRuntimeState,
    registry: GuiTemplateRegistry<'_>,
    gfx_index: Option<&GfxIndex>,
    icon_bank: Option<&mut IconBank>,
    sort_by_opinion: bool,
) -> DiplomacyPanelVanillaRuntimeFrameParts {
    let profile = DiplomacyPanelProfile;
    let mut bindings = bind_profile_tree(&profile, root, data);
    let base = GuiRuntimeFrame::build(
        GuiRuntimeFrameInput::new(root, viewport, profile.profile_id(), &bindings)
            .with_runtime_state(runtime_state.clone()),
    );

    let mut specs = Vec::new();
    if let Some(countries_grid) = base.root_layout.find_by_name("countries_grid") {
        let countries_grid_node = find_gui_node_by_layout_path(root, &countries_grid.path)
            .or_else(|| root.find_node_by_name("countries_grid"));
        if let Some(countries_grid_node) = countries_grid_node {
            specs.extend(diplomacy_panel_country_instance_specs(
                data,
                countries_grid.path.clone(),
                countries_grid_node,
                countries_grid.rect,
                sort_by_opinion,
            ));
        }
    }
    if let Some(detail) = diplomacy_panel_selected_detail(data) {
        if let Some(relations_grid) = base.root_layout.find_by_name("relations_grid") {
            let relations_grid_node = find_gui_node_by_layout_path(root, &relations_grid.path)
                .or_else(|| root.find_node_by_name("relations_grid"));
            if let Some(relations_grid_node) = relations_grid_node {
                specs.extend(diplomacy_panel_relation_instance_specs(
                    detail,
                    relations_grid.path.clone(),
                    relations_grid_node,
                    relations_grid.rect,
                ));
            }
        }
        if let Some(actions_grid) = base.root_layout.find_by_name("actions_grid") {
            let actions_grid_node = find_gui_node_by_layout_path(root, &actions_grid.path)
                .or_else(|| root.find_node_by_name("actions_grid"));
            if let Some(actions_grid_node) = actions_grid_node {
                specs.extend(diplomacy_panel_action_instance_specs(
                    detail,
                    actions_grid.path.clone(),
                    actions_grid_node,
                    actions_grid.rect,
                ));
            }
        }
    }

    bindings.extend(diplomacy_panel_vanilla_instance_bindings(
        data, &specs, &registry,
    ));
    let mut input = GuiRuntimeFrameInput::new(root, viewport, profile.profile_id(), &bindings)
        .with_runtime_state(runtime_state)
        .with_template_registry(registry)
        .with_profile_descriptor(&COUNTRY_DIPLOMACY_DESCRIPTOR)
        .with_instance_specs(specs);
    if let Some(gfx_index) = gfx_index {
        input = input.with_gfx_index(gfx_index);
    }
    if let Some(icon_bank) = icon_bank {
        input = input.with_icon_bank(icon_bank);
    }

    DiplomacyPanelVanillaRuntimeFrameParts {
        frame: GuiRuntimeFrame::build(input),
        bindings,
    }
}

pub fn country_diplomacy_relation_instance_specs(
    data: &CountryInfoData,
    parent_path: GuiNodePath,
    parent_grid_node: &GuiNode,
    parent_rect: GuiRect,
) -> Vec<GuiRuntimeInstanceSpec> {
    let slots =
        diplomacy_scroll_content_grid_slots(parent_grid_node, parent_rect, data.relations.len());
    grouped_instance_specs(
        data.relations.iter().enumerate().map(|(index, relation)| {
            (
                relation_template_for_kind(&relation.kind),
                relation_model_key(index, relation),
            )
        }),
        slots,
        parent_path,
        "diplomacy_relation_entry",
    )
}

pub fn country_diplomacy_action_instance_specs(
    data: &CountryInfoData,
    parent_path: GuiNodePath,
    parent_grid_node: &GuiNode,
    parent_rect: GuiRect,
) -> Vec<GuiRuntimeInstanceSpec> {
    let slots =
        diplomacy_scroll_content_grid_slots(parent_grid_node, parent_rect, data.actions.len());
    grouped_instance_specs(
        data.actions
            .iter()
            .enumerate()
            .map(|(index, action)| ("diplomacy_action_entry", action_model_key(index, action))),
        slots,
        parent_path,
        "diplomacy_action_entry",
    )
}

pub fn diplomacy_panel_country_instance_specs(
    data: &DiplomacyData,
    parent_path: GuiNodePath,
    parent_grid_node: &GuiNode,
    parent_rect: GuiRect,
    sort_by_opinion: bool,
) -> Vec<GuiRuntimeInstanceSpec> {
    let sorted = sorted_country_entries(data, sort_by_opinion);
    let slots = diplomacy_scroll_content_grid_slots(parent_grid_node, parent_rect, sorted.len());
    let mut specs = grouped_instance_specs(
        sorted.iter().map(|(index, country)| {
            (
                "diplomacy_country_list_country_entry",
                country_model_key(*index, country),
            )
        }),
        slots.clone(),
        parent_path.clone(),
        "diplomacy_country_list_country_entry",
    );

    let mut relation_rects = Vec::new();
    let mut relation_model_keys = Vec::new();
    for ((country_index, country), slot) in sorted.iter().zip(slots.iter()) {
        for (relation_index, relation) in country.relations.iter().take(4).enumerate() {
            relation_rects.push(GuiRect::new(
                slot.x + 350.0 + relation_index as f32 * 34.0,
                slot.y + 8.0,
                32.0,
                32.0,
            ));
            relation_model_keys.push(country_relation_model_key(
                *country_index,
                relation_index,
                relation,
            ));
        }
    }
    if !relation_rects.is_empty() {
        specs.push(
            GuiRuntimeInstanceSpec::absolute_rects(
                "diplomacy_country_list_relation_entry",
                parent_path,
                relation_rects,
            )
            .with_options(TemplateInstanceOptions::default().template_size(true))
            .with_semantic_role("diplomacy_country_list_relation_entry")
            .with_model_keys(relation_model_keys),
        );
    }

    specs
}

pub fn diplomacy_panel_relation_instance_specs(
    detail: &CountryDiplomacyDetail,
    parent_path: GuiNodePath,
    parent_grid_node: &GuiNode,
    parent_rect: GuiRect,
) -> Vec<GuiRuntimeInstanceSpec> {
    let slots =
        diplomacy_scroll_content_grid_slots(parent_grid_node, parent_rect, detail.relations.len());
    grouped_instance_specs(
        detail
            .relations
            .iter()
            .enumerate()
            .map(|(index, relation)| {
                (
                    relation_template_for_kind(&relation.kind),
                    relation_model_key(index, relation),
                )
            }),
        slots,
        parent_path,
        "diplomacy_relation_entry",
    )
}

pub fn diplomacy_panel_action_instance_specs(
    detail: &CountryDiplomacyDetail,
    parent_path: GuiNodePath,
    parent_grid_node: &GuiNode,
    parent_rect: GuiRect,
) -> Vec<GuiRuntimeInstanceSpec> {
    let slots =
        diplomacy_scroll_content_grid_slots(parent_grid_node, parent_rect, detail.actions.len());
    grouped_instance_specs(
        detail
            .actions
            .iter()
            .enumerate()
            .map(|(index, action)| ("diplomacy_action_entry", action_model_key(index, action))),
        slots,
        parent_path,
        "diplomacy_action_entry",
    )
}

pub fn country_diplomacy_action_click_command(action: &DiplomacyActionEntry) -> String {
    format!("{COUNTRY_DIPLOMACY_ACTION_PREFIX}{}", action.id)
}

fn grouped_instance_specs(
    plan: impl IntoIterator<Item = (&'static str, String)>,
    slots: Vec<GuiRect>,
    parent_path: GuiNodePath,
    semantic_role: &'static str,
) -> Vec<GuiRuntimeInstanceSpec> {
    let mut groups: Vec<(&'static str, Vec<GuiRect>, Vec<String>)> = Vec::new();
    for ((template_name, model_key), rect) in plan.into_iter().zip(slots) {
        if let Some((_, rects, model_keys)) = groups
            .iter_mut()
            .find(|(group_template, _, _)| *group_template == template_name)
        {
            rects.push(rect);
            model_keys.push(model_key);
        } else {
            groups.push((template_name, vec![rect], vec![model_key]));
        }
    }
    groups
        .into_iter()
        .map(|(template_name, rects, model_keys)| {
            GuiRuntimeInstanceSpec::absolute_rects(template_name, parent_path.clone(), rects)
                .with_options(TemplateInstanceOptions::default().template_size(true))
                .with_semantic_role(semantic_role)
                .with_model_keys(model_keys)
        })
        .collect()
}

fn diplomacy_scroll_content_grid_slots(
    parent_grid_node: &GuiNode,
    parent_rect: GuiRect,
    count: usize,
) -> Vec<GuiRect> {
    if count == 0 {
        return Vec::new();
    }
    let Some(first_slot) = grid_slots(parent_grid_node, parent_rect, 1)
        .into_iter()
        .next()
    else {
        return Vec::new();
    };
    let expanded_rect = GuiRect::new(
        parent_rect.x,
        parent_rect.y,
        parent_rect.width,
        parent_rect
            .height
            .max(first_slot.height.max(1.0) * count as f32),
    );
    grid_slots(parent_grid_node, expanded_rect, count)
}

fn country_diplomacy_vanilla_instance_bindings(
    data: &CountryInfoData,
    specs: &[GuiRuntimeInstanceSpec],
    registry: &GuiTemplateRegistry<'_>,
) -> GuiBindingMap {
    let profile = CountryDiplomacyProfile;
    let mut bindings = GuiBindingMap::default();
    for spec in specs {
        let Some(template) = registry.get(spec.template_name) else {
            continue;
        };
        let count = match &spec.source {
            GuiRuntimeInstanceSource::Descriptor { count }
            | GuiRuntimeInstanceSource::Grid { count } => *count,
            GuiRuntimeInstanceSource::Absolute { rects } => rects.len(),
        };
        for index in 0..count {
            let path = template_instance_path(&spec.parent_path, spec.template_name, index);
            let mut context =
                GuiInstanceContext::new(spec.template_name, index, spec.parent_path.clone());
            if let Some(role) = spec.semantic_role.clone() {
                context = context.with_semantic_role(role);
            }
            if let Some(model_key) = spec.model_keys.get(index).cloned() {
                context = context.with_model_key(model_key);
            }
            bindings.extend(bind_profile_tree_with_path_and_context(
                &profile,
                template.node,
                data,
                path,
                Some(&context),
            ));
        }
    }
    bindings
}

fn diplomacy_panel_vanilla_instance_bindings(
    data: &DiplomacyData,
    specs: &[GuiRuntimeInstanceSpec],
    registry: &GuiTemplateRegistry<'_>,
) -> GuiBindingMap {
    let profile = DiplomacyPanelProfile;
    let mut bindings = GuiBindingMap::default();
    for spec in specs {
        let Some(template) = registry.get(spec.template_name) else {
            continue;
        };
        let count = match &spec.source {
            GuiRuntimeInstanceSource::Descriptor { count }
            | GuiRuntimeInstanceSource::Grid { count } => *count,
            GuiRuntimeInstanceSource::Absolute { rects } => rects.len(),
        };
        for index in 0..count {
            let path = template_instance_path(&spec.parent_path, spec.template_name, index);
            let mut context =
                GuiInstanceContext::new(spec.template_name, index, spec.parent_path.clone());
            if let Some(role) = spec.semantic_role.clone() {
                context = context.with_semantic_role(role);
            }
            if let Some(model_key) = spec.model_keys.get(index).cloned() {
                context = context.with_model_key(model_key);
            }
            bindings.extend(bind_profile_tree_with_path_and_context(
                &profile,
                template.node,
                data,
                path,
                Some(&context),
            ));
        }
    }
    bindings
}

fn bind_relation_node(name: &str, relation: Option<&DiplomacyRelationEntry>) -> GuiBinding {
    let Some(relation) = relation else {
        return GuiBinding::default();
    };
    match name {
        "relation_icon" => GuiBinding::default()
            .sprite(&relation.sprite)
            .tooltip(relation_tooltip(relation)),
        "diplo_relations_bg" | "relation_flags" => {
            GuiBinding::default().tooltip(relation_tooltip(relation))
        }
        "relation_strip_view" | "subject_relation_strip_view" => {
            GuiBinding::default().tooltip(relation_tooltip(relation))
        }
        _ => GuiBinding::default(),
    }
}

fn bind_action_node(name: &str, action: Option<&DiplomacyActionEntry>) -> GuiBinding {
    let Some(action) = action else {
        return GuiBinding::default();
    };
    match name {
        "diplo_actions_entry_bg" => {
            let binding = GuiBinding::default()
                .enabled(action.enabled)
                .tooltip(action_tooltip(action));
            if action.enabled {
                binding.click(country_diplomacy_action_click_command(action))
            } else {
                binding
            }
        }
        "name" => GuiBinding::default()
            .text(&action.label)
            .text_color(if action.enabled {
                Color32::from_rgb(0xe6, 0xdf, 0xc8)
            } else {
                Color32::from_rgb(0x8e, 0x88, 0x78)
            }),
        "cost" => GuiBinding::default()
            .text(action.cost_text.clone().unwrap_or_default())
            .layout_position(96.0, 5.0),
        "accept_icon" => GuiBinding::default()
            .sprite("GFX_accept_decline_icon")
            .frame(if action.enabled { 2 } else { 1 })
            .tooltip(action_tooltip(action)),
        "diplomacy_action_entry" => GuiBinding::default().tooltip(action_tooltip(action)),
        _ => GuiBinding::default(),
    }
}

fn bind_country_list_node(name: &str, country: Option<&CountryEntry>) -> GuiBinding {
    let Some(country) = country else {
        return GuiBinding::default();
    };
    match name {
        "diplomacy_country_list_country_entry" | "Background" => {
            let binding = GuiBinding::default()
                .tooltip(country_list_tooltip(country))
                .click(diplomacy_panel_country_select_command(country));
            if name == "Background" {
                binding.sprite("GFX_diplo_countrylist_entry")
            } else {
                binding
            }
        }
        "diplolist_country_flag" => GuiBinding::default()
            .sprite(&country.flag_gfx)
            .layout_size(36.0, 24.0)
            .tooltip(country_entry_title(country)),
        "name" => GuiBinding::default()
            .text(country_entry_title(country))
            .text_color(country_entry_color(country)),
        "our_opinion" => GuiBinding::default()
            .text(format_signed(country.our_opinion_of_target))
            .text_color(opinion_color(country.our_opinion_of_target)),
        "their_opinion" => GuiBinding::default()
            .text(format_signed(country.their_opinion_of_us))
            .text_color(opinion_color(country.their_opinion_of_us)),
        "relations" => GuiBinding::default().tooltip(country_relations_tooltip(country)),
        _ => GuiBinding::default(),
    }
}

fn bind_country_list_relation_node(
    name: &str,
    relation: Option<&DiplomacyRelationEntry>,
) -> GuiBinding {
    let Some(relation) = relation else {
        return GuiBinding::default();
    };
    match name {
        "diplomacy_country_list_relation_entry" | "icon" => GuiBinding::default()
            .sprite(&relation.sprite)
            .tooltip(relation_tooltip(relation)),
        _ => GuiBinding::default(),
    }
}

fn find_relation<'a>(
    data: &'a CountryInfoData,
    model_key: Option<&str>,
) -> Option<&'a DiplomacyRelationEntry> {
    find_relation_in_slice(&data.relations, model_key)
}

fn find_action<'a>(
    data: &'a CountryInfoData,
    model_key: Option<&str>,
) -> Option<&'a DiplomacyActionEntry> {
    find_action_in_slice(&data.actions, model_key)
}

fn find_panel_relation<'a>(
    data: &'a DiplomacyData,
    model_key: Option<&str>,
) -> Option<&'a DiplomacyRelationEntry> {
    let detail = diplomacy_panel_selected_detail(data)?;
    find_relation_in_slice(&detail.relations, model_key)
}

fn find_panel_action<'a>(
    data: &'a DiplomacyData,
    model_key: Option<&str>,
) -> Option<&'a DiplomacyActionEntry> {
    let detail = diplomacy_panel_selected_detail(data)?;
    find_action_in_slice(&detail.actions, model_key)
}

fn find_relation_in_slice<'a>(
    relations: &'a [DiplomacyRelationEntry],
    model_key: Option<&str>,
) -> Option<&'a DiplomacyRelationEntry> {
    let key = model_key?;
    parse_model_index(key)
        .and_then(|index| relations.get(index))
        .or_else(|| relations.iter().find(|entry| entry.id == key))
}

fn find_action_in_slice<'a>(
    actions: &'a [DiplomacyActionEntry],
    model_key: Option<&str>,
) -> Option<&'a DiplomacyActionEntry> {
    let key = model_key?;
    parse_model_index(key)
        .and_then(|index| actions.get(index))
        .or_else(|| actions.iter().find(|entry| entry.id == key))
}

fn find_country<'a>(data: &'a DiplomacyData, model_key: Option<&str>) -> Option<&'a CountryEntry> {
    let key = model_key?;
    parse_model_index(key)
        .and_then(|index| data.countries.get(index))
        .or_else(|| data.countries.iter().find(|entry| entry.tag == key))
}

fn find_country_list_relation<'a>(
    data: &'a DiplomacyData,
    model_key: Option<&str>,
) -> Option<&'a DiplomacyRelationEntry> {
    let key = model_key?;
    let (country_index, relation_index) = parse_country_relation_model_indices(key)?;
    data.countries
        .get(country_index)
        .and_then(|country| country.relations.get(relation_index))
}

fn parse_model_index(key: &str) -> Option<usize> {
    key.split_once(':')
        .and_then(|(prefix, _)| prefix.parse::<usize>().ok())
}

fn parse_country_relation_model_indices(key: &str) -> Option<(usize, usize)> {
    let mut parts = key.split(':');
    let country_index = parts.next()?.parse::<usize>().ok()?;
    let relation_index = parts.next()?.parse::<usize>().ok()?;
    Some((country_index, relation_index))
}

fn relation_template_for_kind(kind: &DiplomacyRelationKind) -> &'static str {
    match kind {
        DiplomacyRelationKind::Subject | DiplomacyRelationKind::Overlord => {
            "subject_relation_strip_view"
        }
        _ => "relation_strip_view",
    }
}

fn relation_model_key(index: usize, relation: &DiplomacyRelationEntry) -> String {
    format!("{index}:{}", relation.id)
}

fn action_model_key(index: usize, action: &DiplomacyActionEntry) -> String {
    format!("{index}:{}", action.id)
}

fn country_model_key(index: usize, country: &CountryEntry) -> String {
    format!("{index}:{}", country.tag)
}

fn country_relation_model_key(
    country_index: usize,
    relation_index: usize,
    relation: &DiplomacyRelationEntry,
) -> String {
    format!("{country_index}:{relation_index}:{}", relation.id)
}

fn sorted_country_entries(
    data: &DiplomacyData,
    sort_by_opinion: bool,
) -> Vec<(usize, &CountryEntry)> {
    let mut countries: Vec<(usize, &CountryEntry)> = data.countries.iter().enumerate().collect();
    if sort_by_opinion {
        countries.sort_by(|(_, a), (_, b)| {
            b.opinion
                .cmp(&a.opinion)
                .then(a.display_name.cmp(&b.display_name))
                .then(a.tag.cmp(&b.tag))
        });
    } else {
        countries.sort_by(|(_, a), (_, b)| {
            country_entry_title(a)
                .cmp(country_entry_title(b))
                .then(a.tag.cmp(&b.tag))
        });
    }
    countries
}

fn country_info_command_from_action(action: &DiplomacyActionCommand) -> Option<CountryInfoCommand> {
    match action {
        DiplomacyActionCommand::JustifyWargoal { target_tag } => {
            Some(CountryInfoCommand::JustifyWargoal {
                target_tag: target_tag.clone(),
            })
        }
        DiplomacyActionCommand::DeclareWar { target_tag } => Some(CountryInfoCommand::DeclareWar {
            target_tag: target_tag.clone(),
        }),
        DiplomacyActionCommand::InviteToFaction { target_tag } => {
            Some(CountryInfoCommand::InviteToFaction {
                target_tag: target_tag.clone(),
            })
        }
        DiplomacyActionCommand::RequestMilitaryAccess { target_tag } => {
            Some(CountryInfoCommand::RequestMilitaryAccess {
                target_tag: target_tag.clone(),
            })
        }
        DiplomacyActionCommand::Unavailable { .. } => None,
    }
}

fn diplomacy_command_from_action(action: &DiplomacyActionCommand) -> Option<DiplomacyCommand> {
    match action {
        DiplomacyActionCommand::JustifyWargoal { target_tag } => {
            Some(DiplomacyCommand::JustifyWargoal(target_tag.clone()))
        }
        DiplomacyActionCommand::DeclareWar { target_tag } => {
            Some(DiplomacyCommand::DeclareWar(target_tag.clone()))
        }
        DiplomacyActionCommand::InviteToFaction { target_tag } => {
            Some(DiplomacyCommand::InviteToFaction(target_tag.clone()))
        }
        DiplomacyActionCommand::RequestMilitaryAccess { target_tag } => {
            Some(DiplomacyCommand::RequestMilitaryAccess(target_tag.clone()))
        }
        DiplomacyActionCommand::Unavailable { .. } => None,
    }
}

pub fn diplomacy_panel_country_select_command(country: &CountryEntry) -> String {
    format!("{DIPLOMACY_PANEL_SELECT_COUNTRY_PREFIX}{}", country.tag)
}

fn relation_tooltip(relation: &DiplomacyRelationEntry) -> String {
    if relation.tooltip.is_empty() {
        relation.label.clone()
    } else {
        format!("{}\n{}", relation.label, relation.tooltip)
    }
}

fn country_list_tooltip(country: &CountryEntry) -> String {
    let mut parts = vec![
        country_entry_title(country).to_owned(),
        format!(
            "Opinion: {} / {}",
            format_signed(country.our_opinion_of_target),
            format_signed(country.their_opinion_of_us)
        ),
    ];
    if country.at_war {
        parts.push("At war".to_owned());
    } else if country.same_faction {
        parts.push("Same faction".to_owned());
    }
    if let Some(summary) = &country.autonomy_summary {
        parts.push(summary.clone());
    }
    parts.join("\n")
}

fn country_relations_tooltip(country: &CountryEntry) -> String {
    if country.relations.is_empty() {
        "No project diplomacy relations".to_owned()
    } else {
        country
            .relations
            .iter()
            .map(relation_tooltip)
            .collect::<Vec<_>>()
            .join("\n")
    }
}

fn action_tooltip(action: &DiplomacyActionEntry) -> String {
    match action.reason.as_deref() {
        Some(reason) if !reason.is_empty() => format!("{}\n{}", action.preview, reason),
        _ => action.preview.clone(),
    }
}

fn is_close_command(command: &str) -> bool {
    command == COUNTRY_DIPLOMACY_CLOSE_COMMAND
        || command == COUNTRY_DIPLOMACY_ESCAPE_COMMAND
        || command.eq_ignore_ascii_case("ESCAPE")
        || command.ends_with("close_button")
}

fn node_name(node_path: &GuiNodePath) -> &str {
    node_path.0.last().map(String::as_str).unwrap_or_default()
}

fn instance_node_name<'a>(node_path: &'a GuiNodePath, instance: &'a GuiInstanceContext) -> &'a str {
    let name = node_name(node_path);
    let template_name = instance.template_name.as_str();
    if name
        .strip_prefix(template_name)
        .is_some_and(|suffix| suffix.starts_with('['))
    {
        template_name
    } else {
        name
    }
}

fn is_political_pie_chart_node(node_path: &GuiNodePath) -> bool {
    node_path
        .0
        .windows(2)
        .any(|parts| parts[0] == "political_pie_chart" && parts[1] == "chart")
}

fn path_contains(node_path: &GuiNodePath, name: &str) -> bool {
    node_path.0.iter().any(|part| part == name)
}

fn country_title(data: &CountryInfoData) -> &str {
    non_empty_or(data.display_name.as_str(), data.tag.as_str())
}

fn country_entry_title(country: &CountryEntry) -> &str {
    non_empty_or(country.display_name.as_str(), country.tag.as_str())
}

fn diplomacy_panel_selected_country(data: &DiplomacyData) -> Option<&CountryEntry> {
    data.countries.iter().find(|country| country.selected)
}

fn diplomacy_panel_selected_detail(data: &DiplomacyData) -> Option<&CountryDiplomacyDetail> {
    diplomacy_panel_selected_country(data).and_then(|country| country.detail.as_ref())
}

fn preferred_party_name(data: &CountryInfoData) -> String {
    if data.party_full_name.is_empty() {
        data.ruling_party_label.clone()
    } else {
        data.party_full_name.clone()
    }
}

fn preferred_country_party_name(country: &CountryEntry) -> String {
    if country.party_full_name.is_empty() {
        country.ruling_party_label.clone()
    } else {
        country.party_full_name.clone()
    }
}

fn ideology_icon_binding(ideology: &str, label: &str) -> GuiBinding {
    GuiBinding::default()
        .sprite(ideology_icon_sprite(ideology))
        .tooltip(non_empty_or(label, "Unknown ideology"))
}

fn ideology_icon_sprite(key: &str) -> &'static str {
    match canonical_ideology_key(key).unwrap_or(key) {
        "fascism" => "GFX_ideology_fascism_group",
        "democratic" => "GFX_ideology_democratic_group",
        "communism" => "GFX_ideology_communism_group",
        "neutrality" => "GFX_ideology_neutrality_group",
        _ => "GFX_ideology_unknown",
    }
}

fn party_pie_binding(
    ruling_party: &str,
    ruling_party_label: &str,
    party_popularity: &[(String, f32)],
) -> GuiBinding {
    let mut binding =
        GuiBinding::default().tooltip(non_empty_or(ruling_party_label, "Unknown ideology"));
    binding.pie_segments = party_pie_segments(ruling_party, party_popularity);
    binding
}

fn party_pie_segments(
    ruling_party: &str,
    party_popularity: &[(String, f32)],
) -> Vec<(f32, Color32)> {
    let segments = normalized_party_popularity(ruling_party, party_popularity)
        .into_iter()
        .map(|(key, value)| (value.max(0.0), diplomacy_ideology_color(&key)))
        .filter(|(value, _)| *value > 0.0)
        .collect::<Vec<_>>();
    if !segments.is_empty() {
        return segments;
    }
    canonical_ideology_key(ruling_party)
        .map(|ruling| vec![(1.0, diplomacy_ideology_color(ruling))])
        .unwrap_or_default()
}

fn normalized_party_popularity(
    ruling_party: &str,
    party_popularity: &[(String, f32)],
) -> Vec<(String, f32)> {
    let mut totals = [
        ("fascism", 0.0_f32),
        ("democratic", 0.0_f32),
        ("communism", 0.0_f32),
        ("neutrality", 0.0_f32),
    ];
    for (key, value) in party_popularity {
        let Some(canonical) = canonical_ideology_key(key) else {
            continue;
        };
        if let Some((_, total)) = totals
            .iter_mut()
            .find(|(ideology, _)| *ideology == canonical)
        {
            *total += value.max(0.0);
        }
    }
    let mut popularity = totals
        .into_iter()
        .filter_map(|(ideology, value)| {
            (value > 0.0).then(|| (ideology.to_owned(), value.clamp(0.0, 1.0)))
        })
        .collect::<Vec<_>>();
    if popularity.is_empty() {
        if let Some(ruling) = canonical_ideology_key(ruling_party) {
            popularity.push((ruling.to_owned(), 1.0));
        }
    }
    popularity
}

fn diplomacy_ideology_color(key: &str) -> Color32 {
    match canonical_ideology_key(key).unwrap_or(key) {
        "fascism" => Color32::from_rgb(150, 75, 0),
        "democratic" => Color32::from_rgb(0, 0, 255),
        "communism" => Color32::from_rgb(255, 0, 0),
        "neutrality" => Color32::from_rgb(124, 124, 124),
        _ => Color32::from_rgb(0x66, 0x66, 0x60),
    }
}

fn canonical_ideology_key(key: &str) -> Option<&'static str> {
    match key.trim().to_ascii_lowercase().replace('-', "_").as_str() {
        "fascism" | "fascist" => Some("fascism"),
        "democratic" | "democracy" | "democrat" => Some("democratic"),
        "communism" | "communist" => Some("communism"),
        "neutrality" | "neutral" | "non_aligned" | "nonaligned" => Some("neutrality"),
        _ => None,
    }
}

fn non_empty_or<'a>(value: &'a str, fallback: &'a str) -> &'a str {
    if value.is_empty() {
        fallback
    } else {
        value
    }
}

fn format_signed(value: i16) -> String {
    format!("{value:+}")
}

fn format_percent(value: f32) -> String {
    format!("{:.0}%", value.clamp(0.0, 1.0) * 100.0)
}

fn opinion_color(value: i16) -> Color32 {
    if value >= 30 {
        Color32::from_rgb(0x70, 0xc8, 0x78)
    } else if value <= -30 {
        Color32::from_rgb(0xe0, 0x60, 0x58)
    } else {
        Color32::from_rgb(0xe6, 0xdf, 0xc8)
    }
}

fn country_entry_color(country: &CountryEntry) -> Color32 {
    if country.selected {
        Color32::from_rgb(0xff, 0xd3, 0x76)
    } else if country.at_war {
        Color32::from_rgb(0xe0, 0x60, 0x58)
    } else if country.same_faction {
        Color32::from_rgb(0x70, 0xc8, 0x78)
    } else {
        Color32::from_rgb(0xe6, 0xdf, 0xc8)
    }
}

fn find_gui_node_by_layout_path<'a>(node: &'a GuiNode, path: &GuiNodePath) -> Option<&'a GuiNode> {
    fn visit<'a>(node: &'a GuiNode, path: &GuiNodePath, depth: usize) -> Option<&'a GuiNode> {
        if depth >= path.0.len() {
            return Some(node);
        }
        node.children.iter().enumerate().find_map(|(index, child)| {
            (child.path_label(index) == path.0[depth])
                .then(|| visit(child, path, depth + 1))
                .flatten()
        })
    }

    if path
        .0
        .first()
        .is_some_and(|label| label == &node.path_label(0))
    {
        visit(node, path, 1)
    } else {
        None
    }
}
