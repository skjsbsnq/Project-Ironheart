use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use hoi4_logic::economy::EconomyState;
use hoi4_state::{CountryId, World};

pub fn finance_panel_signature(world: &World, econ: &EconomyState, player: usize) -> u64 {
    let mut h = DefaultHasher::new();
    if let Some(t) = world.countries.treasury.treasuries.get(player) {
        t.cash_rm.to_bits().hash(&mut h);
        t.reserve_gbp.to_bits().hash(&mut h);
        t.daily_income_rm.to_bits().hash(&mut h);
        t.daily_expense_rm.to_bits().hash(&mut h);
        t.public_debt_rm.to_bits().hash(&mut h);
        t.mefo_debt_rm.to_bits().hash(&mut h);
        t.gdp_rm.to_bits().hash(&mut h);
        t.credit_rating.hash(&mut h);
    }
    if let Some(rate) = world.countries.treasury.exchange_rates.get(player) {
        rate.rm_per_gbp.to_bits().hash(&mut h);
    }
    if let Some(queue) = econ.construction.get(player) {
        queue.items.len().hash(&mut h);
        for item in &queue.items {
            item.funding_source.hash(&mut h);
            item.budget_needed_rm.to_bits().hash(&mut h);
            item.reserved_funds_rm.to_bits().hash(&mut h);
            item.paid_funds_rm.to_bits().hash(&mut h);
        }
    }
    for account in world
        .countries
        .investment_accounts
        .iter()
        .filter(|account| account.country == CountryId(player as u16))
    {
        account.account_kind.hash(&mut h);
        account.balance_rm.to_bits().hash(&mut h);
        account.last_income_rm.to_bits().hash(&mut h);
        account.last_spent_rm.to_bits().hash(&mut h);
    }
    h.finish()
}

pub fn market_panel_signature(world: &World, econ: &EconomyState, player: usize) -> u64 {
    let mut h = DefaultHasher::new();
    if let Some(market) = world.countries.market.markets.get(player) {
        market.supply.len().hash(&mut h);
        market.demand.len().hash(&mut h);
        market.price.len().hash(&mut h);
        market.stockpile.len().hash(&mut h);
        market.clearing_sheet.results.len().hash(&mut h);
    }
    world.countries.trade.routes.len().hash(&mut h);
    world.countries.buildings_v6.buildings.len().hash(&mut h);
    world.countries.pops.groups.len().hash(&mut h);
    finance_panel_signature(world, econ, player).hash(&mut h);
    h.finish()
}

pub fn construction_panel_signature(
    world: &World,
    econ: &EconomyState,
    auto_build_enabled: bool,
    last_auto_build_explanations_len: usize,
    player: usize,
) -> u64 {
    let mut h = DefaultHasher::new();
    world.countries.buildings_v6.buildings.len().hash(&mut h);
    world.countries.pops.groups.len().hash(&mut h);
    auto_build_enabled.hash(&mut h);
    last_auto_build_explanations_len.hash(&mut h);
    if let Some(queue) = econ.construction.get(player) {
        queue.items.len().hash(&mut h);
        for item in &queue.items {
            item.building_key.hash(&mut h);
            item.target_state.hash(&mut h);
            item.progress.to_bits().hash(&mut h);
            item.paid_funds_rm.to_bits().hash(&mut h);
            for need in &item.material_needs {
                need.good_id.hash(&mut h);
                need.consumed.to_bits().hash(&mut h);
                need.total_needed.to_bits().hash(&mut h);
            }
        }
    }
    market_panel_signature(world, econ, player).hash(&mut h);
    h.finish()
}

pub fn diplomacy_panel_signature(world: &World, player: usize) -> u64 {
    let mut h = DefaultHasher::new();
    world.diplomacy.wars.len().hash(&mut h);
    world.diplomacy.factions.len().hash(&mut h);
    world.diplomacy.diplomatic_requests.len().hash(&mut h);
    world.diplomacy.world_tension.to_bits().hash(&mut h);
    world.countries.tags.len().hash(&mut h);
    player.hash(&mut h);
    h.finish()
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};
    use std::sync::Arc;

    use super::*;
    use hoi4_data::GameData;
    use hoi4_map::{GameMap, Heightmap, ProvinceMap, TerrainBitmap, TerrainCatalog};

    #[test]
    fn diplomacy_signature_changes_with_player() {
        let world = World::new(
            Arc::new(GameMap {
                definitions: vec![None],
                rgb_to_id: HashMap::new(),
                province_map: ProvinceMap {
                    width: 1,
                    height: 1,
                    pixels: vec![0],
                },
                adjacencies: vec![vec![]],
                special_adjacencies: vec![],
                heightmap: Heightmap {
                    width: 1,
                    height: 1,
                    pixels: vec![0],
                },
                terrain_bmp: TerrainBitmap {
                    width: 1,
                    height: 1,
                    pixels: vec![0],
                    palette: [[0; 3]; 256],
                },
                terrain_catalog: TerrainCatalog::default(),
                tree_definition_bmp: None,
                tree_indices: HashSet::new(),
            }),
            Arc::new(GameData::default()),
        );

        assert_ne!(
            diplomacy_panel_signature(&world, 0),
            diplomacy_panel_signature(&world, 1)
        );
    }
}
