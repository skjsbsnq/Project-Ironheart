use std::collections::HashMap;

use hoi4_data::{DivisionTemplate, EditableDivisionTemplate, GameData, TemplatePreview};
use hoi4_state::World;

use super::stats::DivisionStats;

pub fn preview_template(
    template: &DivisionTemplate,
    data: &GameData,
    stockpile: Option<&HashMap<String, f32>>,
) -> TemplatePreview {
    let stats = DivisionStats::aggregate(template, data);
    let equipment_needed = crate::economy::stockpile::template_equipment_needs(template, data);
    let training_days = template_training_days(template, data);
    let stockpile_satisfied_divisions = stockpile
        .map(|stockpile| stockpile_satisfied_divisions(&equipment_needed, stockpile))
        .unwrap_or(0.0);

    TemplatePreview {
        combat_width: stats.combat_width,
        manpower: stats.manpower,
        max_organisation: stats.max_organisation,
        soft_attack: stats.soft_attack,
        hard_attack: stats.hard_attack,
        defense: stats.defense,
        breakthrough: stats.breakthrough,
        armor_value: stats.armor_value,
        ap_attack: stats.ap_attack,
        supply_consumption: stats.supply_consumption,
        suppression: stats.suppression,
        equipment_needed,
        training_days,
        stockpile_satisfied_divisions,
    }
}

pub fn preview_editable_template(
    template: &EditableDivisionTemplate,
    data: &GameData,
    stockpile: Option<&HashMap<String, f32>>,
) -> TemplatePreview {
    preview_template(&template.to_division_template(), data, stockpile)
}

pub fn template_training_days(template: &DivisionTemplate, data: &GameData) -> f32 {
    let mut max_days = 0_u32;
    let mut battalions = 0_u32;
    for subunit in template.regiments.iter().chain(template.support.iter()) {
        if let Some(def) = data.subunits.get(subunit) {
            max_days = max_days.max(def.training_time);
            battalions += 1;
        }
    }
    if battalions == 0 {
        1.0
    } else {
        max_days.max(1) as f32 + battalions as f32 * 0.5
    }
}

fn stockpile_satisfied_divisions(
    equipment_needed: &HashMap<String, u32>,
    stockpile: &HashMap<String, f32>,
) -> f32 {
    let mut supported = f32::INFINITY;
    for (equipment, needed) in equipment_needed {
        if *needed == 0 {
            continue;
        }
        let normalized = crate::economy::stockpile::normalize_equipment_id(equipment);
        let have = stockpile.get(&normalized).copied().unwrap_or(0.0);
        supported = supported.min(have / *needed as f32);
    }
    if supported.is_finite() {
        supported
    } else {
        0.0
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TemplateEditError {
    MissingTemplate,
    SlotOutOfBounds,
    MissingDivision,
    WrongOwnerTemplate,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TemplateSwitchResult {
    pub old_template: u16,
    pub new_template: u16,
    pub equipment_deficit: HashMap<String, f32>,
    pub organisation_penalty: f32,
}

pub fn clone_template(
    templates: &mut Vec<DivisionTemplate>,
    index: u16,
) -> Result<u16, TemplateEditError> {
    let mut cloned = templates
        .get(index as usize)
        .cloned()
        .ok_or(TemplateEditError::MissingTemplate)?;
    cloned.name = format!("{} Copy", cloned.name);
    templates.push(cloned);
    Ok((templates.len() - 1) as u16)
}

pub fn rename_template(
    templates: &mut [DivisionTemplate],
    index: u16,
    name: String,
) -> Result<(), TemplateEditError> {
    let template = templates
        .get_mut(index as usize)
        .ok_or(TemplateEditError::MissingTemplate)?;
    template.name = name;
    Ok(())
}

pub fn set_line_battalion(
    templates: &mut [DivisionTemplate],
    index: u16,
    row: u8,
    col: u8,
    subunit: Option<String>,
) -> Result<(), TemplateEditError> {
    if row >= 5 || col >= 5 {
        return Err(TemplateEditError::SlotOutOfBounds);
    }
    let template = templates
        .get_mut(index as usize)
        .ok_or(TemplateEditError::MissingTemplate)?;
    let mut editable = template.to_editable(index as u32, false);
    editable.line_battalions[row as usize][col as usize] = subunit;
    *template = editable.to_division_template();
    Ok(())
}

pub fn set_support_company(
    templates: &mut [DivisionTemplate],
    index: u16,
    slot: u8,
    subunit: Option<String>,
) -> Result<(), TemplateEditError> {
    if slot >= 5 {
        return Err(TemplateEditError::SlotOutOfBounds);
    }
    let template = templates
        .get_mut(index as usize)
        .ok_or(TemplateEditError::MissingTemplate)?;
    let mut editable = template.to_editable(index as u32, false);
    editable.support_companies[slot as usize] = subunit;
    *template = editable.to_division_template();
    Ok(())
}

pub fn switch_division_template(
    world: &mut World,
    data: &GameData,
    division_idx: usize,
    new_template: u16,
) -> Result<TemplateSwitchResult, TemplateEditError> {
    if division_idx >= world.divisions.count {
        return Err(TemplateEditError::MissingDivision);
    }
    let owner = world.divisions.owners[division_idx];
    let tag = world
        .country_tag(owner)
        .ok_or(TemplateEditError::WrongOwnerTemplate)?;
    let templates = data
        .division_templates
        .get(tag)
        .ok_or(TemplateEditError::MissingTemplate)?;
    let old_template = world.divisions.template_indices[division_idx];
    let old = templates
        .get(old_template as usize)
        .ok_or(TemplateEditError::MissingTemplate)?;
    let new = templates
        .get(new_template as usize)
        .ok_or(TemplateEditError::MissingTemplate)?;

    let old_needs = crate::economy::stockpile::template_equipment_needs(old, data);
    let new_needs = crate::economy::stockpile::template_equipment_needs(new, data);
    let mut equipment_deficit = HashMap::new();
    for (equipment, new_qty) in new_needs {
        let old_qty = old_needs.get(&equipment).copied().unwrap_or(0);
        if new_qty > old_qty {
            equipment_deficit.insert(equipment, (new_qty - old_qty) as f32);
        }
    }

    let old_stats = super::stats::DivisionStats::aggregate(old, data);
    let new_stats = super::stats::DivisionStats::aggregate(new, data);
    let org_penalty = if new_stats.manpower > old_stats.manpower || !equipment_deficit.is_empty() {
        0.35
    } else {
        0.15
    };

    world.divisions.template_indices[division_idx] = new_template;
    world.divisions.max_organisation[division_idx] = new_stats.max_organisation;
    world.divisions.max_strength[division_idx] = new_stats.max_strength;
    world.divisions.organisation[division_idx] = (world.divisions.organisation[division_idx]
        * (1.0 - org_penalty))
        .min(new_stats.max_organisation);
    if !equipment_deficit.is_empty() {
        world.divisions.strength[division_idx] =
            (world.divisions.strength[division_idx] * 0.85).max(0.0);
    }

    Ok(TemplateSwitchResult {
        old_template,
        new_template,
        equipment_deficit,
        organisation_penalty: org_penalty,
    })
}
