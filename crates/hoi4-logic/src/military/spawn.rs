//! 师生成（实例化）：从 `DivisionTemplate` 创建一个新的 division 实例。

use hoi4_data::GameData;
use hoi4_state::{CountryId, DivisionId, ProvinceId, World};

use super::stats::DivisionStats;

/// 从模板创建一个师，写入 `world.divisions`，返回 [`DivisionId`]
///
/// `template_index` 必须是该国 `data.division_templates[tag]` 中的下标
pub fn spawn_from_template(
    world: &mut World,
    data: &GameData,
    owner: CountryId,
    location: ProvinceId,
    template_index: u16,
    name: impl Into<String>,
) -> Option<DivisionId> {
    if owner.is_none() {
        return None;
    }
    let tag = world.country_tag(owner)?.to_owned();
    let templates = data.division_templates.get(&tag)?;
    let template = templates.get(template_index as usize)?;

    let stats = DivisionStats::aggregate(template, data);
    let idx = world.divisions.push(
        owner,
        location,
        template_index,
        stats.max_organisation,
        stats.max_strength,
        name.into(),
    );
    Some(DivisionId(idx))
}

/// 把战斗后的 [`super::battle::BattleSide`] 状态写回 World.divisions
pub fn apply_battle_result(
    world: &mut World,
    division: DivisionId,
    side: &super::battle::BattleSide,
) {
    if division.is_none() {
        return;
    }
    let i = division.0 as usize;
    if i >= world.divisions.count {
        return;
    }
    world.divisions.strength[i] = side.strength;
    world.divisions.organisation[i] = side.organisation;
}
