//! V6 全国商品市场（单池，D2 延伸）。

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::ids::CountryId;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum GoodCategory {
    RawMaterial,
    Intermediate,
    Consumer,
    Luxury,
    Service,
    MilitaryIntermediate,
}

/// 需求桶类型：标识商品的不同需求来源
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum DemandBucketKind {
    /// 建筑投入（工厂生产所需原材料）
    BuildingInput,
    /// POP 基础消费（粮食、衣物等必需品）
    PopBasicConsumption,
    /// POP 非基础消费（日用品、奢侈品）
    PopNonBasicConsumption,
    /// 政府采购（军事中间品、战略物资）
    GovernmentProcurement,
    /// 军工生产投入（钢、机械、燃料等）
    MilitaryInput,
    /// 建造投入（机械、钢等建筑材料）
    ConstructionInput,
    /// 出口订单
    Export,
}

impl DemandBucketKind {
    /// 需求桶优先级：数字越小优先级越高
    pub fn priority(self) -> u8 {
        match self {
            DemandBucketKind::PopBasicConsumption => 1,
            DemandBucketKind::BuildingInput => 2,
            DemandBucketKind::MilitaryInput => 3,
            DemandBucketKind::GovernmentProcurement => 4,
            DemandBucketKind::ConstructionInput => 5,
            DemandBucketKind::PopNonBasicConsumption => 6,
            DemandBucketKind::Export => 7,
        }
    }
}

/// 单个需求桶的清算记录
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClearingBucket {
    pub kind: DemandBucketKind,
    /// 请求量
    pub requested: f32,
    /// 实际满足量
    pub fulfilled: f32,
    /// 未满足量
    pub unmet: f32,
}

/// 单种商品的日清算结果
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GoodClearingResult {
    /// 期初库存
    pub stockpile_opening: f32,
    /// 国内生产
    pub domestic_production: f32,
    /// 进口
    pub imports: f32,
    /// 可用供给 = 国内生产 + 进口 + 库存释放
    pub supply_available: f32,
    /// 各需求桶清算结果
    pub buckets: Vec<ClearingBucket>,
    /// 实际总满足量
    pub total_fulfilled: f32,
    /// 总未满足量
    pub total_unmet: f32,
    /// 出口
    pub exports: f32,
    /// 期末库存
    pub stockpile_closing: f32,
    /// 库存覆盖天数
    pub stockpile_coverage_days: f32,
    /// 短缺比例 = total_unmet / (total_fulfilled + total_unmet)
    pub shortage_ratio: f32,
    /// 政府采购 RM 支出（由清算后确认步骤填入）
    #[serde(default)]
    pub gov_procurement_rm: f64,
}

impl Default for GoodClearingResult {
    fn default() -> Self {
        Self {
            stockpile_opening: 0.0,
            domestic_production: 0.0,
            imports: 0.0,
            supply_available: 0.0,
            buckets: Vec::new(),
            total_fulfilled: 0.0,
            total_unmet: 0.0,
            exports: 0.0,
            stockpile_closing: 0.0,
            stockpile_coverage_days: 0.0,
            shortage_ratio: 0.0,
            gov_procurement_rm: 0.0,
        }
    }
}

/// 每日商品清算表
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MarketClearingSheet {
    /// key = good_id, value = 该商品的清算结果
    pub results: HashMap<String, GoodClearingResult>,
}

/// 分桶需求：每种商品按需求桶分开记录请求量。
/// 各系统（建筑投入、POP消费、军工、政府采购等）写入各自的桶，
/// 清算层按优先级分配供给时直接读取精确的分桶需求。
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct BucketDemand {
    /// key = (good_id, bucket_kind)，value = 请求量
    pub entries: HashMap<(String, DemandBucketKind), f32>,
}

impl BucketDemand {
    pub fn add(&mut self, good_id: &str, kind: DemandBucketKind, amount: f32) {
        if amount <= 0.0 {
            return;
        }
        *self
            .entries
            .entry((good_id.to_owned(), kind))
            .or_insert(0.0) += amount;
    }

    pub fn get(&self, good_id: &str, kind: DemandBucketKind) -> f32 {
        self.entries
            .get(&(good_id.to_owned(), kind))
            .copied()
            .unwrap_or(0.0)
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NationalMarket {
    pub supply: HashMap<String, f32>,
    pub demand: HashMap<String, f32>,
    pub price: HashMap<String, f32>,
    pub stockpile: HashMap<String, f32>,
    #[serde(default)]
    pub unmet_demand: HashMap<String, f32>,
    #[serde(default)]
    pub stockpile_coverage_days: HashMap<String, f32>,
    pub imports: HashMap<String, f32>,
    pub exports: HashMap<String, f32>,
    /// 每日商品清算表（由统一清算层写入）
    #[serde(default)]
    pub clearing_sheet: MarketClearingSheet,
    /// 分桶需求：各系统写入各自的需求桶，清算层按优先级分配
    #[serde(default)]
    pub bucket_demand: BucketDemand,
}

impl Default for NationalMarket {
    fn default() -> Self {
        Self {
            supply: HashMap::new(),
            demand: HashMap::new(),
            price: HashMap::new(),
            stockpile: HashMap::new(),
            unmet_demand: HashMap::new(),
            stockpile_coverage_days: HashMap::new(),
            imports: HashMap::new(),
            exports: HashMap::new(),
            clearing_sheet: MarketClearingSheet::default(),
            bucket_demand: BucketDemand::default(),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MarketStore {
    pub markets: Vec<NationalMarket>,
    #[serde(default)]
    pub blocs: Vec<MarketBloc>,
    #[serde(default)]
    pub country_bloc: Vec<Option<MarketBlocId>>,
    #[serde(default)]
    pub world_spot: WorldSpotMarket,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct WorldSpotMarket {
    #[serde(default)]
    pub daily_supply_caps: HashMap<String, f32>,
    #[serde(default)]
    pub price_multipliers: HashMap<String, f32>,
}

impl WorldSpotMarket {
    pub fn available_for(&self, good_id: &str) -> f32 {
        self.daily_supply_caps
            .get(good_id)
            .copied()
            .unwrap_or_else(|| default_world_spot_cap(good_id))
    }

    pub fn price_multiplier_for(&self, good_id: &str) -> f32 {
        self.price_multipliers
            .get(good_id)
            .copied()
            .unwrap_or_else(|| default_world_spot_price_multiplier(good_id))
    }
}

fn default_world_spot_cap(good_id: &str) -> f32 {
    match good_id {
        "oil" | "rubber" | "fuel" => 6.0,
        "steel" | "coal" | "grain" => 12.0,
        "machinery" | "machine_tools" => 4.0,
        _ => 3.0,
    }
}

fn default_world_spot_price_multiplier(good_id: &str) -> f32 {
    match good_id {
        "oil" | "rubber" | "fuel" => 2.25,
        "steel" | "coal" | "grain" => 1.75,
        _ => 2.0,
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct MarketBlocId(pub u32);

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum MarketBlocKind {
    ImperialPreference,
    FactionMarket,
    ColonialEmpire,
    BilateralSphere,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MarketBloc {
    pub id: MarketBlocId,
    pub name: String,
    pub leader: CountryId,
    pub members: Vec<CountryId>,
    pub kind: MarketBlocKind,
    pub internal_tariff_mult: f32,
    pub external_tariff_mult: f32,
    pub internal_trade_priority: i32,
}

impl MarketStore {
    pub fn new(count: usize) -> Self {
        Self {
            markets: (0..count).map(|_| NationalMarket::default()).collect(),
            blocs: Vec::new(),
            country_bloc: vec![None; count],
            world_spot: WorldSpotMarket::default(),
        }
    }

    pub fn ensure_capacity(&mut self, n: usize) {
        while self.markets.len() < n {
            self.markets.push(NationalMarket::default());
        }
        while self.country_bloc.len() < n {
            self.country_bloc.push(None);
        }
    }

    pub fn add_bloc(
        &mut self,
        name: impl Into<String>,
        leader: CountryId,
        members: Vec<CountryId>,
        kind: MarketBlocKind,
        internal_tariff_mult: f32,
        external_tariff_mult: f32,
        internal_trade_priority: i32,
    ) -> MarketBlocId {
        let id = MarketBlocId(self.blocs.len() as u32);
        for member in &members {
            let idx = member.0 as usize;
            self.ensure_capacity(idx + 1);
            self.country_bloc[idx] = Some(id);
        }
        self.blocs.push(MarketBloc {
            id,
            name: name.into(),
            leader,
            members,
            kind,
            internal_tariff_mult,
            external_tariff_mult,
            internal_trade_priority,
        });
        id
    }

    pub fn bloc_for_country(&self, country: CountryId) -> Option<&MarketBloc> {
        let id = self
            .country_bloc
            .get(country.0 as usize)
            .copied()
            .flatten()?;
        self.blocs.get(id.0 as usize)
    }
}
