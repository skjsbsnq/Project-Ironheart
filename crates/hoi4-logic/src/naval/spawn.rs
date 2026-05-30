//! 舰队 / 舰船创建 API。

use hoi4_data::GameData;
use hoi4_state::{CountryId, FleetId, ShipId, World};

/// 创建一个空舰队
pub fn create_fleet(
    world: &mut World,
    owner: CountryId,
    region: u32,
    name: impl Into<String>,
) -> Option<FleetId> {
    if owner.is_none() {
        return None;
    }
    let idx = world.fleets.push(owner, region, name.into());
    if region != super::regions::NO_SEA_REGION {
        let fi = idx as usize;
        world.fleets.repair_state[fi] = hoi4_state::NavalRepairState::AtSea;
    }
    Some(FleetId(idx))
}

/// 在某舰队中创建一艘舰
pub fn spawn_ship(
    world: &mut World,
    data: &GameData,
    fleet: FleetId,
    class_key: &str,
    name: impl Into<String>,
) -> Option<ShipId> {
    if fleet.is_none() {
        return None;
    }
    let fi = fleet.0 as usize;
    if fi >= world.fleets.count {
        return None;
    }
    let owner = world.fleets.owners[fi];
    let class = data.ship_classes.get(class_key)?;
    let max_hp = class.max_hp;
    let max_org = class.max_organisation;

    let idx = world.ships.push(
        owner,
        fleet,
        class_key.to_owned(),
        max_hp,
        max_org,
        name.into(),
    );
    let ship = ShipId(idx);
    world.fleets.ships[fi].push(ship);
    Some(ship)
}

/// 移除已沉没的舰只（HP <= 0）。返回沉没舰数。
pub fn purge_sunk_ships(world: &mut World) -> usize {
    let mut sunk = 0usize;
    for fi in 0..world.fleets.count {
        let original_len = world.fleets.ships[fi].len();
        world.fleets.ships[fi].retain(|&s| {
            if s.is_none() {
                return false;
            }
            let si = s.0 as usize;
            si < world.ships.count && world.ships.hp[si] > 0.0
        });
        sunk += original_len - world.fleets.ships[fi].len();
    }
    sunk
}
