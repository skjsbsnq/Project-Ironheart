# 经济、人口、财政技术设计

本文档定义 Project Ironheart 新经济系统。目标是修复旧经济模型中 GDP、建筑、POP、财政、市场互相倒推和重复推导的问题。

## 1. 核心判断

旧系统最大问题不是公式不够多，而是权威来源混乱：

- GDP 被当成校准目标后，又被用于反推建筑和财政指标。
- 建筑承担了过多经济含义，导致非建筑经济、服务业、农业和财政基线失真。
- POP 有收入、消费、满意度字段，但不是完整的经济主体。
- 财政是若干收支函数堆叠，不是严格 ledger。
- 市场、财政、POP、建筑都能改钱和需求，导致账难以闭合。

新系统必须明确每个数字的来源。

## 2. 经济权威来源

### 2.1 历史注入数据

历史数据用于初始化和校准，不用于运行期硬锁。

国家级历史数据：

```rust
struct HistoricalCountryEconomy {
    country: CountryTag,
    year: i32,
    population_total: u64,
    labor_force_rate: f32,
    gdp_total_gbp: Money,
    sector_shares: SectorShares,
    government_revenue_gbp: Money,
    government_expense_gbp: Money,
    public_debt_gbp: Money,
    gold_kg: f64,
    foreign_reserve_gbp: Money,
    data_quality: HistoricalDataQuality,
}
```

州级历史数据：

```rust
struct HistoricalStateEconomy {
    state_id: StateId,
    population: u64,
    urbanization: f32,
    literacy: f32,
    agriculture_weight: f32,
    industry_weight: f32,
    services_weight: f32,
    resource_extraction_weight: f32,
}
```

### 2.2 运行期权威

运行期不再有单个“GDP 字段”到处改。改为：

```rust
struct NationalAccounts {
    gdp: GdpAccount,
    income: IncomeAccount,
    fiscal: FiscalAccount,
    external: ExternalAccount,
}
```

GDP 从 `GdpAccount` 计算：

```text
GDP = primary.value_added
    + secondary.value_added
    + tertiary.value_added
    + government.value_added
    + net_tax_adjustment
    + colonial_accounting_adjustment
```

财政从 `FiscalLedger` 汇总。POP 收入从就业、分红、转移支付和兵役工资汇总。市场价格从供需、库存、贸易和政策约束结算。

## 3. GDP 设计

### 3.1 不再用 GDP 反推建筑

原则：

- 历史 GDP 是校准目标。
- 建筑是实物产能和可交互资产。
- 服务业、农业、行政、金融不能强塞进建筑。
- 三产增加值单独核算。

### 3.2 三产分类

```rust
enum EconomicSector {
    Primary,
    Secondary,
    Tertiary,
    Government,
}
```

第一产业：

- 农业。
- 畜牧。
- 林业。
- 渔业。
- 矿业和基础资源采掘。

第二产业：

- 重工业。
- 轻工业。
- 军工。
- 能源。
- 建筑业。
- 加工制造。

第三产业：

- 金融。
- 商业。
- 运输。
- 行政服务。
- 医疗教育。
- 科研。
- 城市服务。

政府部门：

- 行政工资。
- 军队工资中非作战消耗部分。
- 公共服务。
- 研究与教育支出形成的政府增加值。

### 3.3 Sector Account

```rust
struct SectorAccount {
    sector: EconomicSector,
    gross_output_rm: Money,
    intermediate_input_rm: Money,
    wages_rm: Money,
    operating_surplus_rm: Money,
    taxes_on_products_rm: Money,
    subsidies_rm: Money,
    value_added_rm: Money,
    employment: u64,
    capacity_utilization: f32,
}
```

计算：

```text
value_added = gross_output - intermediate_input + taxes_on_products - subsidies
```

不允许 `value_added` 通过目标 GDP 强行缩放到正确。允许在初始化阶段记录 `calibration_factor`，但运行期增长必须由产能、就业、价格、消费、政府支出和贸易驱动。

### 3.4 历史校准

初始化流程：

1. 读取历史总人口、GDP、三产份额。
2. 读取州人口和产业权重。
3. 生成初始 POP。
4. 生成建筑和非建筑 sector capacity。
5. 计算一次基准 national accounts。
6. 生成 `CalibrationReport`。
7. 如果误差超过门槛，标记该国家 `DataQuality::NeedsReview`，不能通过经济验收。

建议门槛：

- 主要国家 GDP 总量误差 <= 5%。
- 三产份额误差 <= 8 个百分点。
- 人口误差 <= 2%。
- 政府收入/GDP 误差 <= 5 个百分点。
- 债务/GDP 误差 <= 5 个百分点。

## 4. 建筑系统

### 4.1 建筑只表达可交互资产

建筑保留：

- 军工厂、民用工业、炼油、造船、港口、机场、铁路、基础设施、银行、矿井、电厂、农场等可交互资产。

建筑不负责：

- 全部 GDP。
- 全部服务业。
- 全部人口收入。
- 全部财政收入。

### 4.2 建筑运行时产出

```rust
struct BuildingRuntimeOutput {
    goods_output: Vec<GoodAmount>,
    goods_input: Vec<GoodAmount>,
    employment: EmploymentByClass,
    wages_rm: Money,
    profit_rm: Money,
    value_added_rm: Money,
    sector: EconomicSector,
}
```

建筑产出进入对应 sector account。建筑利润进入所有者收入或国企收入。工资进入 POP 收入。

## 5. POP 系统

### 5.1 POP 是经济主体

```rust
struct PopGroup {
    id: PopId,
    state: StateId,
    class: PopClass,
    size: u64,
    labor_status: LaborStatus,
    employer: Option<EconomicEmployer>,
    income: PopIncome,
    taxes: PopTaxBurden,
    consumption: PopConsumption,
    welfare: PopWelfare,
    politics: PopPolitics,
}
```

### 5.2 POP 收入

```text
income = wages + dividends + farm_income + soldier_pay + welfare_transfers + colonial_transfers
```

每项必须有来源：

- 工资来自建筑或 sector employment。
- 分红来自企业利润分配。
- 农民收入来自第一产业账户。
- 士兵工资来自财政支出。
- 福利来自财政转移支付。

### 5.3 POP 税收

税种：

- 所得税。
- 消费税。
- 企业税。
- 关税。
- 殖民征收。

所得税不能直接凭 GDP 估算，应从 POP income 税基产生：

```text
income_tax = taxable_pop_income * effective_income_tax_rate * compliance
```

### 5.4 POP 消费

POP 消费分层：

- 必需品：粮食、衣物、燃料。
- 普通品：家具、交通、日用品。
- 服务：教育、医疗、金融、城市服务。
- 奢侈品：汽车、奢侈消费。

消费预算：

```text
disposable_income = income - taxes
basic_budget = disposable_income * basic_need_weight
normal_budget = disposable_income * normal_need_weight
luxury_budget = disposable_income * luxury_need_weight
```

消费通过订单进入市场，不直接扣库存。

### 5.5 满意度和政治

满意度来源：

- 必需品满足率。
- 失业率。
- 税负。
- 工资增长。
- 战争伤亡。
- 法律和意识形态修正。

激进度不能靠固定扣值堆叠，必须来自可解释原因。

## 6. 财政系统

### 6.1 Ledger 是唯一现金权威

所有财政现金变化必须写入：

```rust
struct FiscalLedgerEntry {
    date: GameDate,
    country: CountryId,
    account: FiscalAccountKind,
    direction: DebitCredit,
    amount_rm: Money,
    counterparty: Option<EconomicActor>,
    reason: FiscalReason,
    source: FiscalSource,
}
```

禁止直接改 `cash_rm`。

### 6.2 预算结构

收入：

- 所得税。
- 消费税。
- 企业税。
- 关税。
- 国企利润。
- 殖民上缴。
- 债券发行。
- 外债。
- 黄金/外汇操作。
- 事件收入。

支出：

- 行政工资。
- 军队工资。
- 军事采购。
- 维护。
- 建设。
- 福利。
- 研究。
- 债务利息。
- 补贴。
- 外汇采购。
- 事件支出。

### 6.3 债务和信用

债务账户：

- 国内债 RM。
- 外债 GBP。
- 短期隐性票据。
- 战时特别融资。

信用评级由以下因素决定：

```text
debt_to_gdp
interest_to_revenue
reserve_coverage
war_status
default_history
inflation_pressure
```

不能只看债务/GDP。

### 6.4 MEFO/隐性融资

隐性融资必须作为独立 liability：

- 发行额。
- 资本化利息。
- 到期风险。
- 军费用途追踪。
- 危机触发条件。
- 转换为显性债务的规则。

### 6.5 财政和市场交互

政府采购不能直接“生成装备”。流程：

```text
FiscalPolicy
    -> GovernmentOrder
    -> MarketDemand
    -> Production
    -> Delivery
    -> Stockpile
    -> LedgerExpense
```

建设支出同理：

```text
ConstructionQueue
    -> MaterialDemand
    -> LaborDemand
    -> FiscalFunding
    -> Progress
    -> CompletedAsset
```

## 7. 市场系统

### 7.1 市场权威

市场负责：

- 商品供给。
- 商品需求。
- 库存。
- 进口。
- 出口。
- 价格。
- 短缺。
- 订单履约。

市场不负责：

- 直接修改财政现金。
- 直接决定 GDP。
- 直接改变 POP 收入。

### 7.2 订单类型

```rust
enum MarketOrderKind {
    PopConsumption,
    BuildingInput,
    GovernmentProcurement,
    ConstructionMaterial,
    MilitarySupply,
    Export,
    Import,
}
```

每个订单必须有来源 id，便于调试和验收。

## 8. Tick 顺序

每日经济 tick：

```text
1. clear daily ledgers and market orders
2. update employment demand
3. match labor
4. compute production outputs and sector accounts
5. create POP income
6. compute taxes and fiscal revenue
7. create POP consumption orders
8. create government and construction orders
9. clear market
10. settle deliveries, shortages, stockpile
11. settle fiscal expenses
12. update debt, credit, exchange rate queues
13. compute national accounts
14. write country economy snapshot
15. validate invariants
```

每周：

- 汇率。
- 信用评级。
- AI 经济计划。
- 外贸路线。

每月：

- 预算计划。
- 人口迁移。
- 技能/识字率变化。
- 长期建设计划。
- 统计归档。

## 9. 快照设计

UI 和渲染只读快照：

```rust
struct EconomySnapshot {
    gdp_total: Money,
    gdp_growth_yoy: f32,
    sectors: Vec<SectorSnapshot>,
    population: PopulationSnapshot,
    employment: EmploymentSnapshot,
    fiscal: FiscalSnapshot,
    market: MarketSnapshot,
    construction: ConstructionSnapshot,
}
```

UI 不允许现场遍历 World 自己拼数据。

## 10. 数据质量与回退

经济数据可以缺，但不能静默伪装正确。

数据质量：

- `HistoricalExact`
- `HistoricalEstimated`
- `ModelEstimated`
- `Placeholder`
- `Invalid`

验收规则：

- 主要国家不能有 `Placeholder` GDP。
- 主要国家人口不能来自统一默认值。
- 三产份额缺失时必须在校准报告中列出。
- 财政基线缺失时财政面板显示“不具备历史验收”，不能当成通过。

## 11. 迁移策略

### Step 1：新 schema

先定义 `ironheart-sim` 新结构和测试 fixtures。

### Step 2：历史注入

导入 1936 数据，但只生成快照和报告，不接游戏 UI。

### Step 3：POP 和 sector

让 GDP 从三产账户算出。

### Step 4：财政 ledger

现金和债务只由 ledger 产生。

### Step 5：市场订单

POP、建筑、政府采购全部通过订单进入市场。

### Step 6：旧功能适配

将建造、军事采购、训练、贸易、财政面板逐个接入新 snapshot。

### Step 7：删除兼容层

当新验收通过后，删除旧经济路径 adapter。

## 12. 必须满足的不变量

经济：

- GDP = 各 sector 增加值合计，误差只允许来自四舍五入。
- sector employment 不能超过劳动力供给加允许的临时缺口。
- 商品库存不能无原因为负。
- 生产不能无投入无限扩大。

财政：

- closing cash = opening cash + ledger income - ledger expense + financing。
- 债务变化必须有 ledger 或 liability entry。
- 利息支出必须能追溯到债务本金和利率。

POP：

- disposable income = income - tax - forced payments + transfers。
- 消费支出不能长期超过可支配收入，除非有债务模型。
- POP 总人口按国家/州汇总守恒，迁移和征兵必须有事件。

市场：

- demand fulfilled <= demand。
- supply used <= supply + stockpile + imports。
- unmet demand 可解释。

## 13. UI 展示要求

财政面板必须能回答：

- 钱从哪里来？
- 钱花到哪里去？
- 债务为什么变化？
- 为什么信用评级下降？
- GDP 是哪几个产业贡献的？
- 人口为什么不满意？
- 建设为什么慢？

不能只显示一个总 GDP 和一个现金数字。
