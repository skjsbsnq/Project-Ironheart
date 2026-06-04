//! 经济系统：V6 building-based economy with market/planned tick paths.
//!
//! Design:
//! - [`EconomyState`] holds runtime economic data alongside [`hoi4_state::World`]
//! - Daily tick routes to [`tick_daily_v6`] which dispatches per-country to
//!   market_tick or planned_tick based on economy law
//! - Legacy vanilla tick_daily has been removed (V6 replaces it)

pub mod building_runtime;
mod building_tick_common;
pub mod construction_planner;
pub mod construction_tick;
mod econ_system_tick;
pub mod finance_tick;
pub mod law_modifiers;
pub mod market_balance;
pub mod market_tick;
pub mod planned_tick;
pub mod production_chain;
pub mod stockpile;
pub mod v6_events;
pub mod valuation;

pub use econ_system_tick::EconomicSystemTick;
pub use market_tick::MarketTick;
pub use planned_tick::PlannedTick;

use std::collections::HashMap;

use building_tick_common::{collect_qualification_totals, QualificationTotals};
use hoi4_content::{v6_loader::BuildingDef, V6Database};
use hoi4_data::GameData;
use hoi4_state::{BuildingOwner, CountryId, InvestmentAccountKind, LawCategory, StateId, World};

// ─── Inlined type definitions (formerly in deleted submodules) ───

/// 玩家提交的建造订单
#[derive(Debug, Clone)]
pub struct BuildOrder {
    pub building_key: String,
    pub target_state: StateId,
    pub target_level: Option<u8>,
    pub funding_source: ConstructionFundingSource,
    pub owner_on_completion: BuildingOwner,
    pub reserved_funds_rm: f64,
    pub priority: i16,
    pub weight: f32,
    pub paused: bool,
}

impl BuildOrder {
    pub fn new(building_key: impl Into<String>, target_state: StateId) -> Self {
        Self {
            building_key: building_key.into(),
            target_state,
            target_level: None,
            funding_source: ConstructionFundingSource::Government,
            owner_on_completion: BuildingOwner::State,
            reserved_funds_rm: 0.0,
            priority: 0,
            weight: 1.0,
            paused: false,
        }
    }

    pub fn with_level(mut self, level: u8) -> Self {
        self.target_level = Some(level);
        self
    }

    pub fn with_funding(
        mut self,
        funding_source: ConstructionFundingSource,
        owner_on_completion: BuildingOwner,
        reserved_funds_rm: f64,
    ) -> Self {
        self.funding_source = funding_source;
        self.owner_on_completion = owner_on_completion;
        self.reserved_funds_rm = reserved_funds_rm.max(0.0);
        self
    }

    pub fn with_priority(mut self, priority: i16) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_weight(mut self, weight: f32) -> Self {
        self.weight = weight.max(0.1);
        self
    }

    pub fn paused(mut self, paused: bool) -> Self {
        self.paused = paused;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConstructionFundingSource {
    Government,
    Mefo,
    PrivatePool,
    CartelPool,
    OverlordInvestment { master: CountryId },
    ForeignInvestment { investor: CountryId },
}

/// 建造项目的材料需求条目
#[derive(Debug, Clone)]
pub struct MaterialNeed {
    pub good_id: String,
    pub total_needed: f32,
    pub consumed: f32,
}

/// 建造队列的每日产能分配汇总。
#[derive(Debug, Clone)]
pub struct ConstructionCapacity {
    pub total_cp: f32,
    pub national_admin_cp: f32,
    pub construction_sector_cp: f32,
    pub regional_labor_cp: f32,
    pub engineering_equipment_cp: f32,
    pub finance_cp: f32,
    pub material_cp: f32,
    pub allocated_cp: f32,
    pub idle_cp: f32,
    pub blocked_cp: f32,
}

impl Default for ConstructionCapacity {
    fn default() -> Self {
        Self {
            total_cp: 0.0,
            national_admin_cp: 0.0,
            construction_sector_cp: 0.0,
            regional_labor_cp: 0.0,
            engineering_equipment_cp: 0.0,
            finance_cp: 0.0,
            material_cp: 0.0,
            allocated_cp: 0.0,
            idle_cp: 0.0,
            blocked_cp: 0.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConstructionProjectRuntime {
    pub priority: i16,
    pub weight: f32,
    pub paused: bool,
    pub allocated_cp: f32,
    pub effective_cp: f32,
    pub blocked_cp: f32,
    pub fund_ratio: f32,
    pub material_ratio: f32,
    pub labor_ratio: f32,
    pub engineering_ratio: f32,
    pub infrastructure_ratio: f32,
    pub bottleneck: String,
    pub estimated_days: Option<u32>,
}

impl Default for ConstructionProjectRuntime {
    fn default() -> Self {
        Self {
            priority: 0,
            weight: 1.0,
            paused: false,
            allocated_cp: 0.0,
            effective_cp: 0.0,
            blocked_cp: 0.0,
            fund_ratio: 1.0,
            material_ratio: 1.0,
            labor_ratio: 1.0,
            engineering_ratio: 1.0,
            infrastructure_ratio: 1.0,
            bottleneck: "none".to_owned(),
            estimated_days: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConstructionItem {
    pub building_key: String,
    pub target_state: StateId,
    pub target_level: u8,
    pub progress: f32,
    pub cost: f32,
    pub funding_source: ConstructionFundingSource,
    pub owner_on_completion: BuildingOwner,
    pub reserved_funds_rm: f64,
    pub paid_funds_rm: f64,
    pub budget_needed_rm: f64,
    pub material_needs: Vec<MaterialNeed>,
    pub priority: i16,
    pub weight: f32,
    pub paused: bool,
    pub runtime: ConstructionProjectRuntime,
}

impl ConstructionItem {
    pub fn from_order(order: BuildOrder) -> Self {
        Self {
            building_key: order.building_key,
            target_state: order.target_state,
            target_level: order.target_level.unwrap_or(0),
            progress: 0.0,
            cost: 0.0,
            funding_source: order.funding_source,
            owner_on_completion: order.owner_on_completion,
            reserved_funds_rm: order.reserved_funds_rm,
            paid_funds_rm: 0.0,
            budget_needed_rm: 0.0,
            material_needs: Vec::new(),
            priority: order.priority,
            weight: order.weight.max(0.1),
            paused: order.paused,
            runtime: ConstructionProjectRuntime {
                priority: order.priority,
                weight: order.weight.max(0.1),
                paused: order.paused,
                ..ConstructionProjectRuntime::default()
            },
        }
    }

    pub fn completion(&self) -> f32 {
        if self.cost <= 0.0 {
            0.0
        } else {
            (self.progress / self.cost).clamp(0.0, 1.0)
        }
    }

    pub fn is_complete(&self) -> bool {
        self.cost > 0.0 && self.progress >= self.cost
    }

    pub fn funds_remaining_rm(&self) -> f64 {
        (self.budget_needed_rm - self.paid_funds_rm).max(0.0)
    }

    pub fn material_fulfillment(&self) -> f32 {
        if self.material_needs.is_empty() {
            return 1.0;
        }
        let mut total_ratio = 0.0_f32;
        let mut count = 0;
        for need in &self.material_needs {
            if need.total_needed > 0.0 {
                total_ratio += (need.consumed / need.total_needed).min(1.0);
                count += 1;
            }
        }
        if count == 0 {
            1.0
        } else {
            total_ratio / count as f32
        }
    }
}

/// 每个国家的建造队列
#[derive(Debug, Clone, Default)]
pub struct ConstructionQueue {
    pub items: Vec<ConstructionItem>,
    pub total_completed: u32,
    pub cancelled_sunk_cost_rm: f64,
    pub capacity: ConstructionCapacity,
}

impl ConstructionQueue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

/// 单条生产线
#[derive(Debug, Clone)]
pub struct ProductionLine {
    pub equipment_id: String,
    pub assigned_factories: u32,
    pub efficiency: f32,
    pub efficiency_cap: f32,
    pub days_running: u32,
    pub total_produced: f32,
    actual_factories_field: u32,
}

/// Runtime military procurement order created from force equipment shortfalls.
#[derive(Debug, Clone, PartialEq)]
pub struct GovernmentOrder {
    pub country: CountryId,
    pub equipment_category: String,
    pub daily_budget_rm: f64,
    pub remaining_days: u32,
    pub funding_source: GovernmentOrderFundingSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GovernmentOrderFundingSource {
    Treasury,
    Mefo,
}

impl ProductionLine {
    pub fn new(equipment_id: String, assigned_factories: u32) -> Self {
        Self {
            equipment_id,
            assigned_factories,
            efficiency: 0.10,
            efficiency_cap: 0.50,
            days_running: 0,
            total_produced: 0.0,
            actual_factories_field: 0,
        }
    }

    pub fn switch_equipment(&mut self, new_id: impl Into<String>, factor: f32) {
        self.equipment_id = new_id.into();
        self.efficiency *= factor.clamp(0.0, 1.0);
        self.days_running = 0;
    }

    pub fn switch_variant(&mut self, new_id: impl Into<String>) {
        self.switch_equipment(new_id, 0.90);
    }

    pub fn actual_factories(&self) -> u32 {
        self.actual_factories_field
    }
}

/// 单个国家本日的资源平衡
#[derive(Debug, Clone, Default)]
pub struct ResourceBalance {
    pub produced: HashMap<hoi4_data::ResourceKind, f32>,
    pub consumed: HashMap<hoi4_data::ResourceKind, f32>,
    pub imported: HashMap<hoi4_data::ResourceKind, f32>,
    pub exported: HashMap<hoi4_data::ResourceKind, f32>,
    pub available: HashMap<hoi4_data::ResourceKind, f32>,
    pub stored: HashMap<hoi4_data::ResourceKind, f32>,
}

impl ResourceBalance {
    pub fn net(&self, kind: hoi4_data::ResourceKind) -> f32 {
        let p = self.produced.get(&kind).copied().unwrap_or(0.0);
        let i = self.imported.get(&kind).copied().unwrap_or(0.0);
        let e = self.exported.get(&kind).copied().unwrap_or(0.0);
        let c = self.consumed.get(&kind).copied().unwrap_or(0.0);
        p + i - e - c
    }

    pub fn available_of(&self, kind: hoi4_data::ResourceKind) -> f32 {
        self.available.get(&kind).copied().unwrap_or(0.0)
    }

    pub fn empty() -> Self {
        Self::default()
    }
}

// ─── EconomyState ───

/// 完整的运行时经济状态（per-country 切片化）
pub struct EconomyState {
    /// 国家数（与 `world.countries.count` 一致）
    pub count: usize,

    // ─── 建筑 / 生产 ───
    /// 每个国家的建造队列
    pub construction: Vec<ConstructionQueue>,
    /// 每个国家的生产线列表
    pub production: Vec<Vec<ProductionLine>>,

    // ─── 装备库存 ───
    /// 每个国家持有的装备数量（key = equipment_id, value = 数量）
    pub stockpile: Vec<HashMap<String, f32>>,

    /// V7 government procurement orders generated by military demand.
    pub government_orders: Vec<Vec<GovernmentOrder>>,

    /// V7 division training queues. New divisions deploy only after time, manpower, and equipment allocation.
    pub training_queues: Vec<Vec<crate::military::training::TrainingQueueItem>>,

    // ─── 资源 ───
    /// 每日资源平衡（产出/消耗/可用净值）
    pub resources: Vec<ResourceBalance>,

    /// Last in-game day on which each country's detailed economy was ticked.
    /// Used by the hourly spread tick to avoid the midnight CPU spike.
    pub country_economy_last_tick_day: Vec<i64>,
    /// Last day ticked through the full daily entry point. The hourly spread
    /// path also checks this so a mixed caller cannot settle the same country
    /// twice on one game day.
    pub country_economy_daily_entry_last_tick_day: Vec<i64>,
    pub stockpile_last_tick_day: i64,
    qualification_totals_cache_hour: u64,
    qualification_totals_cache: Vec<QualificationTotals>,
}

impl EconomyState {
    /// 从 [`World`] 初始化
    pub fn new(world: &World) -> Self {
        let n = world.countries.count;
        Self {
            count: n,
            construction: (0..n).map(|_| ConstructionQueue::new()).collect(),
            production: vec![Vec::new(); n],
            stockpile: vec![HashMap::new(); n],
            government_orders: vec![Vec::new(); n],
            training_queues: vec![Vec::new(); n],
            resources: vec![ResourceBalance::default(); n],
            country_economy_last_tick_day: vec![i64::MIN; n],
            country_economy_daily_entry_last_tick_day: vec![i64::MIN; n],
            stockpile_last_tick_day: i64::MIN,
            qualification_totals_cache_hour: u64::MAX,
            qualification_totals_cache: Vec::new(),
        }
    }

    pub(crate) fn qualification_totals(&mut self, world: &World) -> &[QualificationTotals] {
        if self.qualification_totals_cache_hour != world.elapsed_hours
            || self.qualification_totals_cache.len() != world.countries.buildings_v6.buildings.len()
        {
            self.qualification_totals_cache = collect_qualification_totals(world);
            self.qualification_totals_cache_hour = world.elapsed_hours;
        }
        &self.qualification_totals_cache
    }

    pub(crate) fn invalidate_qualification_totals(&mut self) {
        self.qualification_totals_cache_hour = u64::MAX;
    }

    /// Ensure economy arrays cover at least `n` countries (extend if needed).
    pub fn ensure_capacity(&mut self, n: usize) {
        while self.construction.len() < n {
            self.construction.push(ConstructionQueue::new());
        }
        while self.production.len() < n {
            self.production.push(Vec::new());
        }
        while self.stockpile.len() < n {
            self.stockpile.push(HashMap::new());
        }
        while self.government_orders.len() < n {
            self.government_orders.push(Vec::new());
        }
        while self.training_queues.len() < n {
            self.training_queues.push(Vec::new());
        }
        while self.resources.len() < n {
            self.resources.push(ResourceBalance::default());
        }
        while self.country_economy_last_tick_day.len() < n {
            self.country_economy_last_tick_day.push(i64::MIN);
        }
        while self.country_economy_daily_entry_last_tick_day.len() < n {
            self.country_economy_daily_entry_last_tick_day
                .push(i64::MIN);
        }
        self.count = self.count.max(n);
    }

    /// 新增一项建造任务
    pub fn enqueue_construction(&mut self, country: CountryId, order: BuildOrder, world: &World) {
        if country.is_none() {
            return;
        }
        let si = order.target_state.0 as usize;
        if si < world.states.count {
            let used: u16 = world
                .countries
                .buildings_v6
                .buildings
                .iter()
                .filter(|building| building.state == order.target_state && building.level > 0)
                .map(|building| building.level as u16)
                .sum();
            let cap = v6_state_building_capacity(world, order.target_state);
            if used >= cap {
                return;
            }
        }
        let i = country.0 as usize;
        if let Some(q) = self.construction.get_mut(i) {
            q.items.push(ConstructionItem::from_order(order));
        }
    }

    pub fn enqueue_construction_checked(
        &mut self,
        country: CountryId,
        order: BuildOrder,
        world: &World,
        db: &V6Database,
    ) -> Result<(), construction_tick::BuildBlockReason> {
        if country.is_none() {
            return Err(construction_tick::BuildBlockReason::InvalidCountry);
        }
        let building_def: &BuildingDef = db
            .buildings
            .iter()
            .find(|def| def.id == order.building_key)
            .ok_or(construction_tick::BuildBlockReason::InvalidState)?;
        construction_tick::validate_build_location(
            world,
            db,
            country.0 as usize,
            building_def,
            order.target_state,
        )?;
        self.enqueue_construction(country, order, world);
        Ok(())
    }

    pub fn cancel_construction_item(
        &mut self,
        world: &mut World,
        ci: usize,
        queue_idx: usize,
        db: &V6Database,
    ) {
        if ci >= self.construction.len() || queue_idx >= self.construction[ci].items.len() {
            return;
        }
        let item = self.construction[ci].items.remove(queue_idx);
        let completion = item.completion();
        let refund_rate = if completion < 0.1 {
            0.9
        } else if completion < 0.5 {
            0.5
        } else {
            0.0
        };
        let sunk = item.paid_funds_rm * (1.0 - refund_rate);
        self.construction[ci].cancelled_sunk_cost_rm += sunk;

        let refund = item.paid_funds_rm * refund_rate;
        if refund > 0.0 {
            match item.funding_source {
                ConstructionFundingSource::Government | ConstructionFundingSource::Mefo => {
                    world.countries.treasury.treasuries[ci].cash_rm += refund;
                }
                ConstructionFundingSource::PrivatePool => {
                    if let Some(account) = world.countries.investment_account_mut(
                        CountryId(ci as u16),
                        InvestmentAccountKind::Private,
                    ) {
                        account.balance_rm += refund;
                    }
                }
                ConstructionFundingSource::CartelPool => {
                    if let Some(account) = world
                        .countries
                        .investment_account_mut(CountryId(ci as u16), InvestmentAccountKind::Cartel)
                    {
                        account.balance_rm += refund;
                    }
                }
                ConstructionFundingSource::OverlordInvestment { .. }
                | ConstructionFundingSource::ForeignInvestment { .. } => {
                    world.countries.treasury.treasuries[ci].cash_rm += refund;
                }
            }
        }
    }

    /// 新增一条生产线
    pub fn add_production_line(
        &mut self,
        country: CountryId,
        equipment_id: impl Into<String>,
        assigned_factories: u32,
    ) {
        if country.is_none() {
            return;
        }
        let i = country.0 as usize;
        if let Some(lines) = self.production.get_mut(i) {
            lines.push(ProductionLine::new(
                stockpile::normalize_equipment_id(&equipment_id.into()),
                assigned_factories,
            ));
        }
    }

    /// 当前国家可用于建造的民用工厂数（V6 placeholder: returns 0）
    pub fn available_civ_factories(&self, _world: &World, country: CountryId) -> u32 {
        if country.is_none() {
            return 0;
        }
        0
    }

    /// 当前国家某装备的库存量
    pub fn stockpile_of(&self, country: CountryId, equipment_id: &str) -> f32 {
        if country.is_none() {
            return 0.0;
        }
        let normalized = stockpile::normalize_equipment_id(equipment_id);
        self.stockpile
            .get(country.0 as usize)
            .map(|s| {
                s.iter()
                    .filter(|(id, _)| stockpile::normalize_equipment_id(id) == normalized)
                    .map(|(_, qty)| *qty)
                    .sum()
            })
            .unwrap_or(0.0)
    }

    pub fn upsert_government_order(
        &mut self,
        country: CountryId,
        equipment_category: &str,
        daily_budget_rm: f64,
        duration_days: u32,
    ) {
        self.upsert_government_order_with_funding(
            country,
            equipment_category,
            daily_budget_rm,
            duration_days,
            GovernmentOrderFundingSource::Treasury,
        )
    }

    pub fn upsert_government_order_with_funding(
        &mut self,
        country: CountryId,
        equipment_category: &str,
        daily_budget_rm: f64,
        duration_days: u32,
        funding_source: GovernmentOrderFundingSource,
    ) {
        if country.is_none() {
            return;
        }
        let ci = country.0 as usize;
        self.ensure_capacity(ci + 1);
        let normalized = stockpile::normalize_equipment_id(equipment_category);
        if let Some(order) = self.government_orders[ci]
            .iter_mut()
            .find(|order| order.equipment_category == normalized)
        {
            order.daily_budget_rm = order.daily_budget_rm.max(daily_budget_rm);
            order.remaining_days = order.remaining_days.max(duration_days);
            return;
        }
        self.government_orders[ci].push(GovernmentOrder {
            country,
            equipment_category: normalized,
            daily_budget_rm: daily_budget_rm.max(0.0),
            remaining_days: duration_days.max(1),
            funding_source,
        });
    }

    pub fn tick_government_orders(&mut self, ci: usize) {
        if ci >= self.government_orders.len() {
            return;
        }
        for order in &mut self.government_orders[ci] {
            order.remaining_days = order.remaining_days.saturating_sub(1);
        }
        self.government_orders[ci].retain(|order| order.remaining_days > 0);
    }
}

fn v6_state_building_capacity(world: &World, state: StateId) -> u16 {
    let si = state.0 as usize;
    if si >= world.states.count {
        return 0;
    }
    (world.states.category_slots[si] as u16).max(4) + 20
}

/// V6 经济每日 tick：按国家分派到 market_tick 或 planned_tick。
/// `day` 是自开局以来的天数（用于周更/季更判断）。
pub fn tick_daily_v6(world: &mut World, econ: &mut EconomyState, db: &V6Database, day: i64) {
    econ.ensure_capacity(world.countries.count);
    world.rebuild_runtime_country_indexes();
    world.rebuild_trade_export_surplus_index();
    crate::trade::reset_world_spot_daily(world);
    for ci in 0..world.countries.count {
        tick_country_daily_v6(world, econ, db, ci, day);
        econ.country_economy_daily_entry_last_tick_day[ci] = day;
    }
    if econ.stockpile_last_tick_day != day {
        let data = world.data.clone();
        stockpile::tick(world, econ, &data);
        econ.stockpile_last_tick_day = day;
    }
    if day % 30 == 0 {
        world.compact_similar_pop_groups();
    }
    world.rebuild_runtime_country_indexes();
}

/// Spread the expensive per-country economy tick across the 24 hours of a day.
/// This preserves one detailed tick per country per day while avoiding a single
/// midnight frame that runs every country, every building, and every POP at once.
pub fn tick_hourly_spread_v6(world: &mut World, econ: &mut EconomyState, db: &V6Database) {
    let day = world.date.days_since_epoch();
    let hour_bucket = world.date.hour as usize;
    econ.ensure_capacity(world.countries.count);
    if world.date.hour == 0 {
        world.rebuild_runtime_country_indexes();
        world.rebuild_trade_export_surplus_index();
    }

    for ci in 0..world.countries.count {
        if ci % 24 != hour_bucket {
            continue;
        }
        if econ.country_economy_last_tick_day[ci] == day
            || econ.country_economy_daily_entry_last_tick_day[ci] == day
        {
            continue;
        }
        tick_country_daily_v6(world, econ, db, ci, day);
        econ.country_economy_last_tick_day[ci] = day;
    }

    if world.date.hour == 23 && econ.stockpile_last_tick_day != day {
        let data = world.data.clone();
        stockpile::tick(world, econ, &data);
        econ.stockpile_last_tick_day = day;
        if day % 30 == 0 {
            world.compact_similar_pop_groups();
        }
        world.rebuild_runtime_country_indexes();
    }
}

fn tick_country_daily_v6(
    world: &mut World,
    econ: &mut EconomyState,
    db: &V6Database,
    ci: usize,
    day: i64,
) {
    if world.runtime_country_indexes_valid {
        let has_states = world
            .country_state_index
            .get(ci)
            .map(|states| !states.is_empty())
            .unwrap_or(false);
        if !has_states {
            return;
        }

        let has_pops = world
            .country_pop_index
            .get(ci)
            .map(|pops| !pops.is_empty())
            .unwrap_or(false);
        let has_buildings = world
            .country_building_index
            .get(ci)
            .map(|buildings| !buildings.is_empty())
            .unwrap_or(false);
        if !has_pops && !has_buildings {
            return;
        }
    }

    if !country_full_economy_due(world, ci, day) {
        construction_tick::run(world, econ, db, ci);
        v6_events::tick_v6_events(world, db, ci, day);
        return;
    }

    let current_economy_law = world.countries.law_store.law_sets[ci].0
        [LawCategory::Economy.index()]
    .current
    .as_str();

    if current_economy_law == "planned_economy" {
        planned_tick::PlannedTick::tick_daily(world, econ, db, ci, day);
    } else {
        market_tick::MarketTick::tick_daily(world, econ, db, ci, day);
        private_investment_tick(world, econ, db, ci, day);
    }
    construction_tick::run(world, econ, db, ci);
    v6_events::tick_v6_events(world, db, ci, day);
}

fn country_full_economy_due(world: &World, ci: usize, day: i64) -> bool {
    let country = CountryId(ci as u16);
    if country == world.player || world.countries.at_war.get(ci).copied().unwrap_or(false) {
        return true;
    }
    let building_count = world
        .country_building_index
        .get(ci)
        .map(|buildings| buildings.len())
        .unwrap_or(0);
    let pop_group_count = world
        .country_pop_index
        .get(ci)
        .map(|pops| pops.len())
        .unwrap_or(0);
    let _ = (building_count, pop_group_count);
    day % 14 == ci as i64 % 14
}

fn private_investment_tick(
    world: &mut World,
    econ: &mut EconomyState,
    db: &V6Database,
    ci: usize,
    day: i64,
) {
    if ci >= world.countries.count || ci >= econ.construction.len() {
        return;
    }
    if day < 180 || day % 90 != (ci as i64 % 90) {
        return;
    }
    let country = CountryId(ci as u16);
    let legacy_pool = world
        .countries
        .private_investment_pool_rm
        .get(ci)
        .copied()
        .unwrap_or(0.0);
    if let Some(account) = world
        .countries
        .investment_account_mut(country, InvestmentAccountKind::Private)
    {
        account.balance_rm = account.balance_rm.max(legacy_pool);
    }
    let pool_balance = world
        .countries
        .investment_balance_rm(country, InvestmentAccountKind::Private);
    if pool_balance < 250_000_000.0 {
        return;
    }
    if econ.construction[ci].items.len() >= 1 {
        return;
    }

    let cp = construction_tick::construction_cp_pool(world, ci).max(1.0) as u32;
    let plan = construction_planner::plan_construction(
        world,
        econ,
        db,
        country,
        construction_planner::ConstructionPlanningMode::PrivateInvestment,
        cp,
    );
    let Some(candidate) = plan.candidates.into_iter().find(|candidate| {
        db.buildings
            .iter()
            .find(|def| def.id == candidate.building_id)
            .map(|def| {
                !matches!(
                    def.kind,
                    hoi4_content::v6_loader::BuildingKindDef::Military
                        | hoi4_content::v6_loader::BuildingKindDef::MilitaryBase
                )
            })
            .unwrap_or(false)
    }) else {
        return;
    };
    let building_id = candidate.building_id;
    let sid = candidate.state;
    let building_def = db.buildings.iter().find(|def| def.id == building_id);
    let project_budget = building_def
        .map(|def| def.construction_recipe.funds_rm * 0.3)
        .unwrap_or(250_000_000.0);

    if project_budget > pool_balance {
        return;
    }

    let target_level = world
        .countries
        .buildings_v6
        .buildings
        .iter()
        .filter(|building| {
            building.state == sid && building.building_def_id == building_id.as_str()
        })
        .map(|building| building.level)
        .max()
        .unwrap_or(0)
        .saturating_add(1);
    econ.enqueue_construction(
        country,
        BuildOrder::new(building_id, sid)
            .with_level(target_level)
            .with_funding(
                ConstructionFundingSource::PrivatePool,
                BuildingOwner::Private,
                project_budget,
            ),
        world,
    );
    if let Some(account) = world
        .countries
        .investment_account_mut(country, InvestmentAccountKind::Private)
    {
        let deduction = project_budget.min(account.balance_rm);
        account.balance_rm = (account.balance_rm - deduction).max(0.0);
        account.last_spent_rm += deduction;
    }
    if let Some(pool) = world.countries.private_investment_pool_rm.get_mut(ci) {
        *pool = (*pool - project_budget).max(0.0);
    }
}

/// Helper: total manpower needed by a template (used by stockpile module)
pub(crate) fn stats_manpower(template: &hoi4_data::DivisionTemplate, data: &GameData) -> u32 {
    let mut total = 0u32;
    for sub_key in template.regiments.iter().chain(template.support.iter()) {
        if let Some(sub) = data.subunits.get(sub_key) {
            total = total.saturating_add(sub.manpower);
        }
    }
    total
}

/// 初始化新世界：把每个国家的燃油写入起始值
pub fn init_world(world: &mut World) {
    for i in 0..world.countries.count {
        let cap = if world.countries.fuel_capacity[i] > 0.0 {
            world.countries.fuel_capacity[i]
        } else {
            50_000.0
        };
        world.countries.fuel_capacity[i] = cap;
        world.countries.fuel[i] = cap * 0.25;
    }
}

/// 工具：用 game_id 把 state 内部 ID 反查
pub fn state_by_game_id(world: &World, game_id: u16) -> Option<StateId> {
    world.state_id_lookup.get(&game_id).copied()
}
