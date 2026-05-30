//! 联队创建 / 飞机增减。

use hoi4_data::GameData;
use hoi4_state::{AirWingId, CountryId, World};

/// 在某空区创建一个新的空军联队，飞机数为 `plane_count`。
///
/// 返回 [`AirWingId`]；若 country 无效或飞机类型未注册则返回 `None`。
pub fn create_air_wing(
    world: &mut World,
    data: &GameData,
    owner: CountryId,
    aircraft_key: &str,
    region: u32,
    plane_count: u32,
    name: impl Into<String>,
) -> Option<AirWingId> {
    if owner.is_none() {
        return None;
    }
    let def = data.aircraft.get(aircraft_key)?;
    let max_org = def.max_organisation;
    let idx = world.air_wings.push(
        owner,
        aircraft_key.to_owned(),
        region,
        plane_count,
        max_org,
        name.into(),
    );
    let i = idx as usize;
    world.air_wings.range_km[i] = def.range;
    world.air_wings.base_state[i] = region.min(u16::MAX as u32) as u16;
    Some(AirWingId(idx))
}

/// 给联队补充飞机（不超过 max）；返回实际补充数。
pub fn reinforce(world: &mut World, wing: AirWingId, planes: u32) -> u32 {
    if wing.is_none() {
        return 0;
    }
    let i = wing.0 as usize;
    if i >= world.air_wings.count {
        return 0;
    }
    let cur = world.air_wings.count_planes[i];
    let max = world.air_wings.max_planes[i];
    let add = planes.min(max.saturating_sub(cur));
    world.air_wings.count_planes[i] = cur + add;
    add
}

/// 设置联队任务（同时记录 target_region）。
pub fn assign_mission(
    world: &mut World,
    wing: AirWingId,
    mission: hoi4_state::AirMission,
    target_region: u32,
) {
    if wing.is_none() {
        return;
    }
    let i = wing.0 as usize;
    if i >= world.air_wings.count {
        return;
    }
    world.air_wings.mission[i] = mission;
    world.air_wings.target_region[i] = target_region;
}

/// 移除已 0 飞机的联队（HP 全清空）。返回清理数。
pub fn purge_destroyed_wings(world: &mut World) -> usize {
    // SoA 不能 retain，所以手工压缩
    let n = world.air_wings.count;
    let mut keep_idx: Vec<usize> = Vec::with_capacity(n);
    for i in 0..n {
        if world.air_wings.count_planes[i] > 0 {
            keep_idx.push(i);
        }
    }
    if keep_idx.len() == n {
        return 0;
    }
    let removed = n - keep_idx.len();
    let owners = std::mem::take(&mut world.air_wings.owners);
    let aks = std::mem::take(&mut world.air_wings.aircraft_keys);
    let region = std::mem::take(&mut world.air_wings.region_id);
    let base_state = std::mem::take(&mut world.air_wings.base_state);
    let target = std::mem::take(&mut world.air_wings.target_region);
    let transfer_arrival = std::mem::take(&mut world.air_wings.transfer_arrival_hour);
    let range = std::mem::take(&mut world.air_wings.range_km);
    let reinforce_enabled = std::mem::take(&mut world.air_wings.reinforce_enabled);
    let mission = std::mem::take(&mut world.air_wings.mission);
    let count_planes = std::mem::take(&mut world.air_wings.count_planes);
    let max_planes = std::mem::take(&mut world.air_wings.max_planes);
    let org = std::mem::take(&mut world.air_wings.organisation);
    let max_org = std::mem::take(&mut world.air_wings.max_organisation);
    let in_combat = std::mem::take(&mut world.air_wings.in_combat);
    let names = std::mem::take(&mut world.air_wings.names);
    let exp = std::mem::take(&mut world.air_wings.experience);

    for i in keep_idx {
        world.air_wings.owners.push(owners[i]);
        world.air_wings.aircraft_keys.push(aks[i].clone());
        world.air_wings.region_id.push(region[i]);
        world.air_wings.base_state.push(base_state[i]);
        world.air_wings.target_region.push(target[i]);
        world
            .air_wings
            .transfer_arrival_hour
            .push(transfer_arrival[i]);
        world.air_wings.range_km.push(range[i]);
        world.air_wings.reinforce_enabled.push(reinforce_enabled[i]);
        world.air_wings.mission.push(mission[i]);
        world.air_wings.count_planes.push(count_planes[i]);
        world.air_wings.max_planes.push(max_planes[i]);
        world.air_wings.organisation.push(org[i]);
        world.air_wings.max_organisation.push(max_org[i]);
        world.air_wings.in_combat.push(in_combat[i]);
        world.air_wings.names.push(names[i].clone());
        world.air_wings.experience.push(exp[i]);
    }
    world.air_wings.count = world.air_wings.owners.len();
    removed
}

/// 每日 tick：组织度恢复（仅当不在战斗中），并补满飞机数（如果用户允许 — 我们这里
/// 简单恢复 5% max_planes 一日，模拟训练机制。受国家 air_xp 影响可后续接入）
pub fn tick_daily(world: &mut World) {
    let n = world.air_wings.count;
    for i in 0..n {
        // 组织度恢复
        if !world.air_wings.in_combat[i] {
            let max_org = world.air_wings.max_organisation[i];
            let cur = world.air_wings.organisation[i];
            if cur < max_org {
                let new = (cur + max_org * 0.30).min(max_org);
                world.air_wings.organisation[i] = new;
            }
        }
        // 飞机不会自动恢复（需要工厂 reinforce）；这里不动
    }
}
