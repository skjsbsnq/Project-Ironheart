pub(crate) fn first_open_line_slot(
    templates: &[hoi4_data::DivisionTemplate],
    template_idx: u16,
) -> Option<(u8, u8)> {
    let template = templates.get(template_idx as usize)?;
    let editable = template.to_editable(template_idx as u32, false);
    for row in 0..5 {
        for col in 0..5 {
            if editable.line_battalions[row][col].is_none() {
                return Some((row as u8, col as u8));
            }
        }
    }
    None
}

pub(crate) fn first_open_support_slot(
    templates: &[hoi4_data::DivisionTemplate],
    template_idx: u16,
) -> Option<u8> {
    let template = templates.get(template_idx as usize)?;
    let editable = template.to_editable(template_idx as u32, false);
    editable
        .support_companies
        .iter()
        .position(|slot| slot.is_none())
        .map(|slot| slot as u8)
}

pub(crate) fn empty_template_editor_data() -> hoi4_ui::military::TemplateEditorData {
    hoi4_ui::military::TemplateEditorData {
        selected_template: None,
        name: String::new(),
        line_battalions: vec![vec![None; 5]; 5],
        support_companies: vec![None; 5],
        line_choices: Vec::new(),
        support_choices: Vec::new(),
        combat_width: 0.0,
        manpower: 0,
        max_organisation: 0.0,
        soft_attack: 0.0,
        hard_attack: 0.0,
        defense: 0.0,
        breakthrough: 0.0,
        suppression: 0.0,
        supply_consumption: 0.0,
        training_days: 0.0,
        equipment_needed: Vec::new(),
        stockpile_satisfied_divisions: 0.0,
    }
}

pub(crate) fn empty_template_subunit_picker_data() -> hoi4_ui::military::TemplateSubunitPickerData {
    hoi4_ui::military::TemplateSubunitPickerData {
        target: None,
        title: String::new(),
        current: None,
        choices: Vec::new(),
    }
}

pub(crate) fn build_template_subunit_picker_data(
    data: &hoi4_data::GameData,
    country_tag: &str,
    target: Option<hoi4_ui::military::TemplatePickerTarget>,
) -> hoi4_ui::military::TemplateSubunitPickerData {
    let Some(target) = target else {
        return empty_template_subunit_picker_data();
    };
    let Some(templates) = data.division_templates.get(country_tag) else {
        return empty_template_subunit_picker_data();
    };

    let (template_idx, current, choices, title) = match target {
        hoi4_ui::military::TemplatePickerTarget::Line { template, row, col } => {
            let current = templates
                .get(template as usize)
                .map(|template_def| template_def.to_editable(template as u32, false))
                .and_then(|editable| editable.line_battalions[row as usize][col as usize].clone());
            let mut choices = data
                .subunits
                .values()
                .filter(|subunit| subunit.is_land() && !looks_like_support_subunit(&subunit.key))
                .map(|subunit| subunit.key.clone())
                .collect::<Vec<_>>();
            choices.sort();
            choices.dedup();
            if choices.is_empty() {
                choices.push("infantry".to_owned());
            }
            (
                template,
                current,
                choices,
                format!("????????R{} C{}", row + 1, col + 1),
            )
        }
        hoi4_ui::military::TemplatePickerTarget::Support { template, slot } => {
            let current = templates
                .get(template as usize)
                .map(|template_def| template_def.to_editable(template as u32, false))
                .and_then(|editable| editable.support_companies[slot as usize].clone());
            let mut choices = data
                .subunits
                .values()
                .filter(|subunit| {
                    looks_like_support_subunit(&subunit.key) || subunit.suppression > 0.0
                })
                .map(|subunit| subunit.key.clone())
                .collect::<Vec<_>>();
            choices.sort();
            choices.dedup();
            if choices.is_empty() {
                choices.extend([
                    "engineer".to_owned(),
                    "recon".to_owned(),
                    "support_artillery".to_owned(),
                ]);
            }
            (template, current, choices, format!("????????{}", slot + 1))
        }
    };

    if (template_idx as usize) >= templates.len() {
        return empty_template_subunit_picker_data();
    }

    hoi4_ui::military::TemplateSubunitPickerData {
        target: Some(target),
        title,
        current,
        choices,
    }
}

pub(crate) fn build_template_editor_data(
    data: &hoi4_data::GameData,
    country_tag: &str,
    selected_template: Option<u16>,
    stockpile: Option<&std::collections::HashMap<String, f32>>,
) -> hoi4_ui::military::TemplateEditorData {
    let Some(templates) = data.division_templates.get(country_tag) else {
        return empty_template_editor_data();
    };
    if templates.is_empty() {
        return empty_template_editor_data();
    }

    let selected = selected_template
        .filter(|idx| (*idx as usize) < templates.len())
        .unwrap_or(0);
    let template = &templates[selected as usize];
    let editable = template.to_editable(selected as u32, false);
    let preview = hoi4_logic::military::templates::preview_template(template, data, stockpile);

    let line_battalions = editable
        .line_battalions
        .iter()
        .map(|row| row.iter().cloned().collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let support_companies = editable
        .support_companies
        .iter()
        .cloned()
        .collect::<Vec<_>>();

    let mut line_choices = data
        .subunits
        .values()
        .filter(|subunit| subunit.is_land() && !looks_like_support_subunit(&subunit.key))
        .map(|subunit| subunit.key.clone())
        .collect::<Vec<_>>();
    line_choices.sort();
    line_choices.dedup();
    if line_choices.is_empty() && data.subunits.contains_key("infantry") {
        line_choices.push("infantry".to_owned());
    }

    let mut support_choices = data
        .subunits
        .values()
        .filter(|subunit| looks_like_support_subunit(&subunit.key) || subunit.suppression > 0.0)
        .map(|subunit| subunit.key.clone())
        .collect::<Vec<_>>();
    support_choices.sort();
    support_choices.dedup();
    if support_choices.is_empty() {
        for fallback in ["engineer", "recon", "support_artillery"] {
            support_choices.push(fallback.to_owned());
        }
    }

    let mut equipment_needed = preview.equipment_needed.into_iter().collect::<Vec<_>>();
    equipment_needed.sort_by(|a, b| a.0.cmp(&b.0));

    hoi4_ui::military::TemplateEditorData {
        selected_template: Some(selected),
        name: template.name.clone(),
        line_battalions,
        support_companies,
        line_choices,
        support_choices,
        combat_width: preview.combat_width,
        manpower: preview.manpower,
        max_organisation: preview.max_organisation,
        soft_attack: preview.soft_attack,
        hard_attack: preview.hard_attack,
        defense: preview.defense,
        breakthrough: preview.breakthrough,
        suppression: preview.suppression,
        supply_consumption: preview.supply_consumption,
        training_days: preview.training_days,
        equipment_needed,
        stockpile_satisfied_divisions: preview.stockpile_satisfied_divisions,
    }
}

pub(crate) fn looks_like_support_subunit(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    key.contains("support")
        || key.contains("engineer")
        || key.contains("recon")
        || key.contains("maintenance")
        || key.contains("logistics")
        || key.contains("signal")
        || key.contains("hospital")
}

pub(crate) fn naval_mission_to_ui(
    mission: hoi4_state::NavalMission,
) -> hoi4_ui::naval::NavalMissionUi {
    match mission {
        hoi4_state::NavalMission::Idle => hoi4_ui::naval::NavalMissionUi::Idle,
        hoi4_state::NavalMission::Patrol => hoi4_ui::naval::NavalMissionUi::Patrol,
        hoi4_state::NavalMission::ConvoyEscort => hoi4_ui::naval::NavalMissionUi::ConvoyEscort,
        hoi4_state::NavalMission::StrikeForce => hoi4_ui::naval::NavalMissionUi::StrikeForce,
        hoi4_state::NavalMission::ConvoyRaiding => hoi4_ui::naval::NavalMissionUi::ConvoyRaiding,
        hoi4_state::NavalMission::MineLaying => hoi4_ui::naval::NavalMissionUi::MineLaying,
        hoi4_state::NavalMission::MineSweeping => hoi4_ui::naval::NavalMissionUi::MineSweeping,
        hoi4_state::NavalMission::NavalInvasionSupport => {
            hoi4_ui::naval::NavalMissionUi::NavalInvasionSupport
        }
    }
}

pub(crate) fn naval_mission_from_ui(
    mission: hoi4_ui::naval::NavalMissionUi,
) -> hoi4_state::NavalMission {
    match mission {
        hoi4_ui::naval::NavalMissionUi::Idle => hoi4_state::NavalMission::Idle,
        hoi4_ui::naval::NavalMissionUi::Patrol => hoi4_state::NavalMission::Patrol,
        hoi4_ui::naval::NavalMissionUi::ConvoyEscort => hoi4_state::NavalMission::ConvoyEscort,
        hoi4_ui::naval::NavalMissionUi::StrikeForce => hoi4_state::NavalMission::StrikeForce,
        hoi4_ui::naval::NavalMissionUi::ConvoyRaiding => hoi4_state::NavalMission::ConvoyRaiding,
        hoi4_ui::naval::NavalMissionUi::MineLaying => hoi4_state::NavalMission::MineLaying,
        hoi4_ui::naval::NavalMissionUi::MineSweeping => hoi4_state::NavalMission::MineSweeping,
        hoi4_ui::naval::NavalMissionUi::NavalInvasionSupport => {
            hoi4_state::NavalMission::NavalInvasionSupport
        }
    }
}

pub(crate) fn air_mission_to_ui(mission: hoi4_state::AirMission) -> hoi4_ui::air::AirMissionUi {
    match mission {
        hoi4_state::AirMission::Idle => hoi4_ui::air::AirMissionUi::Idle,
        hoi4_state::AirMission::AirSuperiority => hoi4_ui::air::AirMissionUi::AirSuperiority,
        hoi4_state::AirMission::Interception => hoi4_ui::air::AirMissionUi::Interception,
        hoi4_state::AirMission::CloseAirSupport => hoi4_ui::air::AirMissionUi::CloseAirSupport,
        hoi4_state::AirMission::StrategicBombing => hoi4_ui::air::AirMissionUi::StrategicBombing,
        hoi4_state::AirMission::PortStrike => hoi4_ui::air::AirMissionUi::PortStrike,
        hoi4_state::AirMission::NavalStrike => hoi4_ui::air::AirMissionUi::NavalStrike,
        hoi4_state::AirMission::NavalPatrol => hoi4_ui::air::AirMissionUi::NavalPatrol,
        hoi4_state::AirMission::LogisticalStrike => hoi4_ui::air::AirMissionUi::LogisticalStrike,
        hoi4_state::AirMission::Drop => hoi4_ui::air::AirMissionUi::Drop,
    }
}

pub(crate) fn air_mission_from_ui(mission: hoi4_ui::air::AirMissionUi) -> hoi4_state::AirMission {
    match mission {
        hoi4_ui::air::AirMissionUi::Idle => hoi4_state::AirMission::Idle,
        hoi4_ui::air::AirMissionUi::AirSuperiority => hoi4_state::AirMission::AirSuperiority,
        hoi4_ui::air::AirMissionUi::Interception => hoi4_state::AirMission::Interception,
        hoi4_ui::air::AirMissionUi::CloseAirSupport => hoi4_state::AirMission::CloseAirSupport,
        hoi4_ui::air::AirMissionUi::StrategicBombing => hoi4_state::AirMission::StrategicBombing,
        hoi4_ui::air::AirMissionUi::PortStrike => hoi4_state::AirMission::PortStrike,
        hoi4_ui::air::AirMissionUi::NavalStrike => hoi4_state::AirMission::NavalStrike,
        hoi4_ui::air::AirMissionUi::NavalPatrol => hoi4_state::AirMission::NavalPatrol,
        hoi4_ui::air::AirMissionUi::LogisticalStrike => hoi4_state::AirMission::LogisticalStrike,
        hoi4_ui::air::AirMissionUi::Drop => hoi4_state::AirMission::Drop,
    }
}
