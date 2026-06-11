use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time::Duration;

use hoi4_logic::economy::EconomyState;
use hoi4_state::{CountryId, World};

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum UiPanelCacheKind {
    Finance,
    Market,
    Construction,
    Diplomacy,
    Military,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct UiPanelCacheKey {
    player: usize,
    day: i64,
    signature: u64,
    selected_tag: Option<String>,
}

impl UiPanelCacheKey {
    pub fn new(player: usize, day: i64, signature: u64) -> Self {
        Self {
            player,
            day,
            signature,
            selected_tag: None,
        }
    }

    pub fn with_selected_tag(mut self, selected_tag: Option<String>) -> Self {
        self.selected_tag = selected_tag;
        self
    }
}

#[derive(Default)]
pub struct UiPanelCache {
    pub finance: Option<(UiPanelCacheKey, hoi4_ui::finance_panel::FinancePanelData)>,
    pub market: Option<(UiPanelCacheKey, hoi4_ui::market_panel::MarketPanelData)>,
    pub construction: Option<(
        UiPanelCacheKey,
        hoi4_ui::construction_v6_panel::ConstructionV6PanelData,
    )>,
    pub diplomacy: Option<(UiPanelCacheKey, hoi4_ui::diplomacy::DiplomacyData)>,
    last_builds: Vec<UiPanelBuildPerf>,
}

struct UiPanelBuildPerf {
    kind: UiPanelCacheKind,
    elapsed: Duration,
    reused: bool,
}

impl UiPanelCache {
    pub fn clear(&mut self) {
        self.finance = None;
        self.market = None;
        self.construction = None;
        self.diplomacy = None;
    }

    pub fn begin_frame(&mut self) {
        self.last_builds.clear();
    }

    pub fn record(&mut self, kind: UiPanelCacheKind, elapsed: Duration, reused: bool) {
        self.last_builds.push(UiPanelBuildPerf {
            kind,
            elapsed,
            reused,
        });
    }

    pub fn perf_report(&self) -> Option<String> {
        if self.last_builds.is_empty() {
            return None;
        }
        Some(
            self.last_builds
                .iter()
                .map(|entry| {
                    let state = if entry.reused { "hit" } else { "build" };
                    format!(
                        "{:?}:{}:{:.2}ms",
                        entry.kind,
                        state,
                        entry.elapsed.as_secs_f64() * 1000.0
                    )
                })
                .collect::<Vec<_>>()
                .join(" | "),
        )
    }
}

pub fn cached_market_panel(
    cache: &mut UiPanelCache,
    world: &World,
    db: &hoi4_content::V6Database,
    econ: &EconomyState,
    player: usize,
) -> Option<hoi4_ui::market_panel::MarketPanelData> {
    let cache_started = std::time::Instant::now();
    let cache_key = UiPanelCacheKey::new(
        player,
        world.date.days_since_epoch(),
        market_panel_signature(world, econ, player),
    );
    if let Some(data) = cache
        .market
        .as_ref()
        .filter(|(key, _)| *key == cache_key)
        .map(|(_, data)| data.clone())
    {
        cache.record(UiPanelCacheKind::Market, cache_started.elapsed(), true);
        Some(data)
    } else {
        let built = crate::ui_data::market::market_panel(world, db, econ, player);
        if let Some(data) = built.as_ref() {
            cache.market = Some((cache_key, data.clone()));
        }
        cache.record(UiPanelCacheKind::Market, cache_started.elapsed(), false);
        built
    }
}

pub fn cached_finance_panel(
    cache: &mut UiPanelCache,
    world: &World,
    db: &hoi4_content::V6Database,
    econ: &EconomyState,
    player: usize,
) -> Option<hoi4_ui::finance_panel::FinancePanelData> {
    let cache_started = std::time::Instant::now();
    let cache_key = UiPanelCacheKey::new(
        player,
        world.date.days_since_epoch(),
        finance_panel_signature(world, econ, player),
    );
    if let Some(data) = cache
        .finance
        .as_ref()
        .filter(|(key, _)| *key == cache_key)
        .map(|(_, data)| data.clone())
    {
        cache.record(UiPanelCacheKind::Finance, cache_started.elapsed(), true);
        Some(data)
    } else {
        let built = crate::ui_data::economy::build_finance_panel_data(world, econ, db, player);
        if let Some(data) = built.as_ref() {
            cache.finance = Some((cache_key, data.clone()));
        }
        cache.record(UiPanelCacheKind::Finance, cache_started.elapsed(), false);
        built
    }
}

pub fn cached_construction_panel(
    cache: &mut UiPanelCache,
    world: &World,
    db: &hoi4_content::V6Database,
    econ: &EconomyState,
    player: usize,
    construction_mode: &Option<String>,
    auto_build_enabled: bool,
    auto_build_explanations: &[hoi4_logic::economy::construction_planner::ConstructionCandidateScore],
) -> Option<hoi4_ui::construction_v6_panel::ConstructionV6PanelData> {
    let cache_started = std::time::Instant::now();
    let cache_key = UiPanelCacheKey::new(
        player,
        world.date.days_since_epoch(),
        construction_panel_signature(
            world,
            econ,
            auto_build_enabled,
            auto_build_explanations.len(),
            player,
        ),
    );
    if let Some(data) = cache
        .construction
        .as_ref()
        .filter(|(key, _)| *key == cache_key)
        .map(|(_, data)| data.clone())
    {
        cache.record(
            UiPanelCacheKind::Construction,
            cache_started.elapsed(),
            true,
        );
        Some(data)
    } else {
        let built = crate::ui_data::construction::build_construction_v6_panel_data(
            world,
            db,
            econ,
            player,
            construction_mode,
            auto_build_enabled,
            auto_build_explanations,
        );
        if let Some(data) = built.as_ref() {
            cache.construction = Some((cache_key, data.clone()));
        }
        cache.record(
            UiPanelCacheKind::Construction,
            cache_started.elapsed(),
            false,
        );
        built
    }
}

pub fn cached_diplomacy_panel(
    cache: &mut UiPanelCache,
    world: &World,
    historical_1936: &hoi4_content::Historical1936Database,
    db: &hoi4_content::V6Database,
    player: usize,
    selected_tag: Option<String>,
    instant_war: bool,
) -> Option<hoi4_ui::diplomacy::DiplomacyData> {
    let cache_started = std::time::Instant::now();
    let cache_key = UiPanelCacheKey::new(
        player,
        world.date.days_since_epoch(),
        diplomacy_panel_signature(world, player),
    )
    .with_selected_tag(selected_tag.clone());
    if let Some(data) = cache
        .diplomacy
        .as_ref()
        .filter(|(key, _)| *key == cache_key)
        .map(|(_, data)| data.clone())
    {
        cache.record(UiPanelCacheKind::Diplomacy, cache_started.elapsed(), true);
        Some(data)
    } else {
        let built = crate::ui_data::country::build_diplomacy_panel_data(
            world,
            historical_1936,
            db,
            player,
            selected_tag.as_deref(),
            instant_war,
        );
        if let Some(data) = built.as_ref() {
            cache.diplomacy = Some((cache_key, data.clone()));
        }
        cache.record(UiPanelCacheKind::Diplomacy, cache_started.elapsed(), false);
        built
    }
}

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
    if let Some(pp) = world.countries.political_power.get(player) {
        pp.to_bits().hash(&mut h);
    }
    if let Some(goals) = world
        .diplomacy
        .pending_wargoals
        .get(&CountryId(player as u16))
    {
        goals.len().hash(&mut h);
        for goal in goals {
            goal.target.0.hash(&mut h);
            (goal.kind as u8).hash(&mut h);
            goal.target_state.hash(&mut h);
            goal.justified.hash(&mut h);
        }
    }
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
