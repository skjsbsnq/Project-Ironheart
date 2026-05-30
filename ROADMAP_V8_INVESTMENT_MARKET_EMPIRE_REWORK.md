# Project Ironheart V8 — 投资产权·帝国经济·阵营市场·Victoria 3 式市场面板总重构路线图

> 写于 2026-05-23。
> 本路线图整合本轮审计中发现的所有相关问题：投资池与再投资、产权系统、傀儡/殖民经济、帝国与阵营市场、受限世界贸易、以及 Victoria 3 式市场面板。
> 它不是给既有路线图追加小节，而是作为后续经济系统深化的统一实施路线。旧路线图仍保留历史背景与已完成项，但本文件定义新的目标状态、实施顺序和验收口径。


## 0. 使命

当前 V6/V7 已经有建筑、POP、财政、国家市场、贸易路线、傀儡自治和市场面板的骨架，但这些系统之间没有形成 1936 年世界经济应有的结构性闭环：

- 投资池只是国家级单一金额，无法表达资本来源、产权归属、再投资行为和利润分配。
- 建造队列没有资金来源与建成产权字段，政府、私人、法团和殖民资本混在同一条队列里。
- `BuildingOwner::{Private, State, Cartel}` 过粗，不能表达混合所有制、外资、宗主国控制、殖民资产和国有化/私有化。
- 傀儡和自治关系已经存在，但主要用于外交和自治度进展，没有真正把资源、贸易、工业、财政、人力和军事控制接入宗主国经济。
- 贸易系统是“每国市场 + 自动双边补缺 + 历史路线”，没有帝国市场、阵营市场、共同市场、市场准入或贸易圈优先权。
- 1936 年不应是 Victoria 3 式统一全球市场，也不应是完全孤立国家市场；应是国家市场 + 帝国/阵营贸易圈 + 受海权、外汇、外交和战争限制的世界贸易。
- 市场面板已有五页诊断版，但仍是国家商品列表，不能承载阵营市场、殖民供应链、成员贡献、进口依赖、贸易圈价格、市场准入和玩家决策入口。

本路线图的使命是把经济系统从“能运行的国家市场雏形”升级为：

```text
国内资本与产权决定谁投资、谁拥有、谁拿利润；
帝国/阵营市场决定资源优先流向和贸易依赖；
受限世界贸易决定外部补缺、封锁风险和外汇压力；
市场面板让玩家能看懂上述系统，并能直接找到解决手段。
```


## 1. 范围

### 1.1 范围内

- 新增建造资金来源与建成产权：政府、私人投资池、法团投资池、宗主国投资、外资/贸易圈投资。
- 重构私人投资池：从国家级单一余额升级为可解释的投资账户和再投资规则。
- 重构产权系统：保留简单 owner 枚举兼容运行，但逐步引入产权账户、份额、利润分配和产权变更。
- 接入傀儡/殖民经济：资源优先权、强制贸易、财政/外汇抽取、工业控制、人力/军队贡献、自治度反馈。
- 新增市场圈/贸易圈模型：英帝国优惠体系、阵营市场、殖民贸易圈、双边势力范围。
- 重构贸易撮合：历史协议 -> 宗主/傀儡 -> 市场圈 -> 同阵营 -> 中立贸易 -> 受限世界现货市场。
- 限制 `CountryId::NONE` 的无限世界市场占位，改为有限、昂贵、可封锁、受外汇限制的世界现货供应。
- 重构市场面板为 Victoria 3 风格的复杂经济入口，支持市场圈、商品详情、成员贡献、贸易路线、殖民依赖、产业链、价格解释、行动建议。
- 增加自动测试和 1936 回放验收，覆盖英国帝国依赖、德国资源短缺、日本封锁、傀儡资源抽取、投资产权归属。

### 1.2 范围外

- 不做 Victoria 3 级别逐 POP 文化、宗教、财富阶层和全球移民模拟。
- 不做完整企业品牌系统，不单独模拟 Opel、Ford、Krupp、IG Farben 等公司资产负债表。
- 不做每一块殖民地的完整财政预算；第一阶段只做宗主国经济抽取和 subject 反馈。
- 不做完全自由的全球金融市场；外资和宗主国投资先以规则化账户表示。
- 不恢复 HOI4 vanilla civ/mil/dock 三工厂经济模型。
- 不保证旧中间存档兼容。


## 2. 当前实现审计

### 2.1 投资与产权

| 文件 | 当前状态 | 问题 |
|---|---|---|
| `crates/hoi4-state/src/buildings_v6.rs` | `BuildingOwner::{Private, State, Cartel}` | 只有单一所有制枚举，没有产权份额、投资主体或外资 |
| `crates/hoi4-state/src/store.rs` | `private_investment_pool_rm: Vec<f64>` | 投资池只是每国一个余额，不知道资金来自谁 |
| `crates/hoi4-logic/src/economy/mod.rs` | `private_investment_tick` 每 90 天用固定阈值入队 | 再投资选择粗糙，不看真实利润、市场圈、产权和资金来源 |
| `crates/hoi4-logic/src/economy/construction_tick.rs` | 建成 owner 来自 `owner_default` | 私人/政府/法团出资不会影响建成产权 |
| `crates/hoi4-logic/src/economy/finance_tick.rs` | 私营/法团利润有粗分配 | 多套利润估算，普通 Private 利润和资本家/投资池闭环弱 |

### 2.2 傀儡与殖民经济

| 文件 | 当前状态 | 问题 |
|---|---|---|
| `crates/hoi4-state/src/diplomacy.rs` | 有 `AutonomyLevel` 和 `master_resource_share()` | 资源份额没有真正转入宗主国经济 |
| `crates/hoi4-logic/src/diplomacy/puppet.rs` | 自治度按工业比推进 | 工业值只影响自治进度，不影响资源、贸易、财政、军工和人力 |
| `crates/hoi4-content/src/v6_loader.rs` | 注入 ENG-CAN/AST/NZL/SAF/RAJ/MAL 等关系 | 只是外交关系，不形成帝国经济体系 |
| `crates/hoi4-logic/tests/v7_puppet_autonomy_economy.rs` | 只测自治度使用真实工业 | 缺资源抽取、贸易优先、工业控制、财政吸血测试 |

### 2.3 市场与贸易

| 文件 | 当前状态 | 问题 |
|---|---|---|
| `crates/hoi4-state/src/market.rs` | 每国一个 `NationalMarket` | 无市场圈、共同市场、帝国市场、阵营市场 |
| `crates/hoi4-state/src/trade.rs` | `TradeRoute`/`TradeAgreement` | `TradeRouteKind` 没有保留 ImperialPreference 运行时语义 |
| `crates/hoi4-logic/src/trade/mod.rs` | 自动找出口方，傀儡/阵营加分 | 加分不是共同市场；`CountryId::NONE` 仍可无限补缺 |
| `crates/hoi4-content/content/history_1936/trade/initial_trade_1936.ron` | 有少量历史贸易 | 英帝国、法殖民、轴心、苏联势力圈依赖严重不足 |

### 2.4 市场面板

| 文件 | 当前状态 | 问题 |
|---|---|---|
| `crates/hoi4-ui/src/market_panel.rs` | 已有总览/短缺/商品/产业链/贸易五页 | 仍是国家商品诊断，不支持市场圈和殖民经济 |
| `crates/hoi4-app/src/main.rs` | 面板数据现场聚合 `World + V6Database` | 数据装配过长，缺市场圈快照、成员贡献、贸易来源拆分 |
| `crates/hoi4-ui/src/trade_panel.rs` | 贸易路线与失败原因 | 与市场面板割裂，不能从商品直接定位贸易圈/成员/路线 |


## 3. 目标架构

### 3.1 市场层级

采用三层市场，不做单一全球市场：

```text
国家市场 NationalMarket
  - 每国供给、需求、价格、库存、财政和外汇。

市场圈 MarketBloc
  - 帝国优惠体系、阵营市场、殖民贸易圈、双边势力范围。
  - 提供内部优先交易、关税优惠、资源配额、市场准入和成员贡献。

世界现货市场 WorldSpotMarket
  - 有限供应、价格惩罚、海运/封锁/外汇/外交限制。
  - 不能无限补缺，不能绕过战争和封锁。
```

### 3.2 产权层级

```text
建筑 Building
  -> 产权账户 OwnershipAccount
  -> 产权份额 OwnershipShare
  -> 利润分配 ProfitDistribution
  -> 投资账户 InvestmentAccount
```

保留 `BuildingOwner` 作为显示和快速分支，但不再作为唯一产权真相。

### 3.3 建造资金链

```text
BuildOrder
  -> ConstructionItem
  -> funding_source
  -> reserved_funds_rm
  -> owner_on_completion / ownership_shares
  -> construction progress
  -> completed Building
  -> profit distribution
  -> reinvestment pool
```

### 3.4 傀儡经济链

```text
Autonomy relation
  -> subject resource surplus
  -> master resource priority / forced export
  -> master market supply / imports
  -> subject reserve loss or export income split
  -> autonomy progress and radicalism feedback
```


## 4. 新数据结构

### 4.1 建造资金来源

目标文件：`crates/hoi4-logic/src/economy/mod.rs`

```rust
pub enum ConstructionFundingSource {
    Government,
    PrivatePool,
    CartelPool,
    OverlordInvestment { master: CountryId },
    ForeignInvestment { investor: CountryId },
}

pub struct ConstructionItem {
    pub building_key: String,
    pub target_state: StateId,
    pub target_level: u8,
    pub progress: f32,
    pub cost: f32,
    pub funding_source: ConstructionFundingSource,
    pub owner_on_completion: BuildingOwner,
    pub reserved_funds_rm: f64,
    pub expected_profit_rm: f64,
}
```

第一阶段可先不做 `ForeignInvestment` 的复杂逻辑，但字段必须预留。

### 4.2 产权账户

目标文件：`crates/hoi4-state/src/buildings_v6.rs`

```rust
pub enum OwnershipAccount {
    State { country: CountryId },
    DomesticPrivate { country: CountryId },
    Cartel { country: CountryId },
    Overlord { master: CountryId, subject: CountryId },
    ForeignPrivate { country: CountryId },
}

pub struct OwnershipShare {
    pub account: OwnershipAccount,
    pub share: f32,
}
```

短期实现：`Building.owner` 继续存在；新增可选 `ownership_shares`，并由 `owner` 生成默认份额。

### 4.3 投资账户

目标文件：`crates/hoi4-state/src/store.rs`

```rust
pub struct InvestmentAccount {
    pub country: CountryId,
    pub account_kind: InvestmentAccountKind,
    pub balance_rm: f64,
    pub last_income_rm: f64,
    pub last_spent_rm: f64,
}

pub enum InvestmentAccountKind {
    Private,
    Cartel,
    StateDevelopmentBank,
    ColonialExtraction,
    ForeignCapital,
}
```

`private_investment_pool_rm` 可保留为缓存或迁移期字段，但新逻辑应以账户为准。

### 4.4 市场圈

目标文件：`crates/hoi4-state/src/market.rs` 或新增 `crates/hoi4-state/src/market_bloc.rs`

```rust
pub enum MarketBlocKind {
    ImperialPreference,
    FactionMarket,
    ColonialEmpire,
    BilateralSphere,
}

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

pub struct MarketBlocStore {
    pub blocs: Vec<MarketBloc>,
    pub country_bloc: Vec<Option<MarketBlocId>>,
}
```

### 4.5 市场面板数据

目标文件：`crates/hoi4-ui/src/market_panel.rs`

```rust
pub struct MarketPanelData {
    pub goods: Vec<GoodEntry>,
    pub bloc: Option<MarketBlocPanelData>,
    pub selected_country_market: String,
    pub exchange_rate: f32,
    pub cash_rm: f64,
    pub reserve_gbp: f64,
    pub total_shortage_value_rm: f64,
    pub total_import_value_gbp: f64,
    pub total_export_value_gbp: f64,
    pub pop_needs_fulfillment: f32,
    pub military_supply_pressure: f32,
    pub market_access: f32,
    pub alerts: Vec<MarketAlertEntry>,
}

pub struct MarketBlocPanelData {
    pub name: String,
    pub kind: String,
    pub leader_tag: String,
    pub members: Vec<MarketBlocMemberEntry>,
    pub internal_trade_value_gbp: f64,
    pub external_trade_value_gbp: f64,
    pub dependency_entries: Vec<MarketDependencyEntry>,
}

pub struct MarketBlocMemberEntry {
    pub tag: String,
    pub relation: String,
    pub market_access: f32,
    pub contribution_supply_value_rm: f64,
    pub contribution_demand_value_rm: f64,
    pub strategic_goods: Vec<GoodFlowSource>,
}

pub struct MarketDependencyEntry {
    pub good_id: String,
    pub good_name: String,
    pub domestic_supply: f32,
    pub bloc_supply: f32,
    pub world_imports: f32,
    pub shortage: f32,
    pub dependency_level: f32,
}
```


## 5. 市场面板目标设计

市场面板应成为主要经济诊断入口，参考 Victoria 3 的市场与商品页面，但适配 1936 战争经济。

### 5.1 标签页

```text
总览
商品
短缺
产业链
市场圈
贸易路线
殖民/傀儡
价格与库存
行动建议
```

### 5.2 总览页

显示：

- 当前国家市场名称。
- 所属市场圈：例如 “英帝国优惠体系”。
- 市场领导国与成员数。
- 总供给、总需求、总短缺价值。
- POP 需求满足率。
- 军工供应压力。
- 外汇储备和进口覆盖天数。
- 市场准入：港口、铁路、封锁、战争造成的损失。
- 最严重短缺 5 项。
- 最关键进口依赖 5 项。
- 当前可执行建议：扩建、进口、改 PM、释放库存、压榨殖民地、解除封锁。

### 5.3 商品页

每个商品行显示：

- 商品名、类别、当前价格、基础价格、7 日变化。
- 供给/需求/成交/缺口。
- 国内生产、市场圈输入、世界进口、库存释放。
- 建筑需求、POP 需求、军购需求、建造需求、出口需求。
- 库存和覆盖天数。
- 受影响建筑和 POP。
- 是否战略物资：油、橡胶、钢、粮食、燃油、机床、无线电、发动机等。

商品详情展开必须显示：

```text
供给来源
  国内建筑
  傀儡/殖民地贡献
  市场圈成员贡献
  历史贸易协议
  世界现货市场

需求来源
  建筑输入
  POP 消费
  政府军购
  建造队列
  出口/宗主国抽取

价格解释
  供需比
  库存覆盖
  进口价格
  关税/外汇
  封锁/市场准入
```

### 5.4 市场圈页

市场圈页是本次重构核心。必须显示：

- 市场圈类型：帝国优惠、阵营市场、殖民帝国、双边势力范围。
- 领导国。
- 成员列表。
- 每个成员的供给贡献、需求贡献、市场准入、战略商品贡献。
- 内部贸易价值与外部贸易价值。
- 内部关税和外部关税。
- 成员是否被封锁、是否在战争、是否为傀儡/自治领。
- 对本国最关键的市场圈依赖。

英国示例：

```text
英帝国优惠体系
  ENG: 金融、造船、工业需求
  CAN: 粮食、木材、部分工业品
  RAJ: 人力、粮食、纺织、税收/外汇贡献
  MAL: 橡胶
  SAF: 黄金、矿产
  AST/NZL: 粮食、羊毛、战时人力
```

### 5.5 殖民/傀儡页

显示每个 subject：

- 自治等级。
- 宗主国资源份额。
- 实际资源贡献。
- 强制贸易量。
- 财政/外汇贡献。
- 军工/船坞/建造力可用份额。
- 人力/远征军/训练贡献。
- 自治度变化来源。
- 压榨造成的满意度/激进度风险。

### 5.6 贸易路线页

整合现有 trade_panel 能力，不再让玩家在市场和贸易两个面板之间来回猜：

- 历史贸易路线。
- 动态贸易路线。
- 帝国优惠路线。
- 陆路/海路/转运。
- 港口州。
- 吞吐量。
- 封锁状态。
- 外汇成本。
- 关税。
- 失败原因。
- 受影响商品和建筑。

`trade_panel.rs` 可保留为轻量贸易专页，但市场面板必须能从商品详情直接跳到相关路线。

### 5.7 行动建议页

面向新手，提供可执行、可理解的建议：

- “建设钢铁厂”，显示推荐州、预期补缺、所需建材。
- “进口橡胶”，显示可用伙伴、路线风险、外汇成本。
- “切换生产方式”，显示会减少哪些输入、降低多少产出。
- “提高殖民抽取”，显示资源收益和自治/激进度代价。
- “释放库存”，显示能撑几天。
- “降低军购/暂停训练”，显示减少需求。
- “修港口/铁路”，显示市场准入改善。

第一阶段建议只做只读建议，不直接执行命令；第二阶段再接按钮。


## 6. 投资与产权实施计划

### V8.A1 — 建造资金来源与建成产权

任务：

- 扩展 `ConstructionItem`，新增 `funding_source`、`owner_on_completion`、`reserved_funds_rm`。
- 玩家政府建造默认 `Government + State` 或根据法律选择产权。
- 私人投资入队默认 `PrivatePool + Private`。
- 法团投资入队默认 `CartelPool + Cartel`。
- 建造完成时优先使用 `owner_on_completion`，不再只看建筑定义 `owner_default`。
- UI 队列显示资金来源和建成产权。

验收：

- 私人投资池入队项目完成后建筑 owner 为 Private。
- 政府建造项目完成后建筑 owner 不被 `owner_default` 意外改成 Private。
- 法团经济项目可形成 Cartel 建筑。

### V8.A2 — 投资账户与再投资评分

任务：

- 新增 `InvestmentAccount`。
- 私营、法团、国家开发银行、殖民抽取分别记账。
- 投资池增长基于统一的 `building.profit_rm` 或单一利润函数。
- 重写 `private_investment_tick`，从固定 5 类建筑改为从可建目录评分。
- 评分考虑预期利润、短缺商品、市场圈需求、州基础设施、劳动力、输入可得性、法律限制。
- 允许私人项目有独立队列槽，不因政府队列非空完全停摆。

验收：

- 短缺钢铁时私人资本优先扩钢铁或上游矿业。
- 橡胶高价且有进口/殖民供应时资本会投橡胶制品链。
- 投资池不足时不入队，余额和原因可见。

### V8.A3 — 产权份额与利润分配

任务：

- 新增 `OwnershipShare`。
- State/Private/Cartel 生成默认 100% 份额。
- Cartel 70/30 不再硬编码在税收函数中，而由产权/利润分配规则表达。
- 支持国有化、私有化、宗主国投资形成份额。

验收：

- 建筑利润可按产权份额进入 Treasury、资本家收入、投资账户或宗主国账户。
- 国有化后后续利润进入国家财政。
- 宗主国投资 subject 建筑后，部分利润可回流宗主国。


## 7. 傀儡、殖民与帝国经济实施计划

### V8.B1 — 市场圈基础与英帝国初始化

任务：

- 新增 `MarketBlocStore`。
- 初始化英帝国优惠体系：ENG、CAN、AST、NZL、SAF、RAJ、MAL。
- 保留现有 autonomy relation，同时把这些国家加入同一市场圈。
- `ImperialPreference` 不再映射成普通 Sea，运行时保留路线语义。

验收：

- 市场面板显示英国属于 “英帝国优惠体系”。
- 加拿大、马来亚、印度等成员出现在市场圈页。
- 历史 ImperialPreference 路线在 UI 中显示为帝国优惠而不是普通海运。

### V8.B2 — Subject 资源优先与强制贸易

任务：

- `master_resource_share()` 真正接入贸易。
- 对 subject surplus 计算 `reserved_for_master`。
- IntegratedPuppet/Puppet 优先满足 master 缺口。
- Dominion 不强制抽取，只提供优先购买和低关税。
- 资源抽取影响自治度和 subject 满意度/激进度。

验收：

- ENG 缺橡胶时优先从 MAL 获得橡胶。
- ENG 缺粮时优先从 CAN 获取粮食，但 Dominion 不被按 Puppet 比例强制抽取。
- 提高抽取后 subject 自治度进展下降或激进度上升。

### V8.B3 — 宗主国工业、财政和人力控制

任务：

- IntegratedPuppet 可让 master 使用部分军工/船坞/建造力。
- Puppet 可提供资源、贸易和有限人力。
- Dominion 主要提供战时贸易、远征军和自愿贡献。
- 深度 subject 的外汇收入可部分进入 master reserve。

验收：

- 深度傀儡的部分工业能力可被宗主国军购调用。
- RAJ/MAL 对 ENG 有可见资源/外汇贡献。
- CAN/AST/NZL/SAF 不被当作普通 Puppet 经济吸干。


## 8. 市场与贸易实施计划

### V8.C1 — 分层贸易撮合

任务：

将 `choose_exporter_for_import` 改为分层：

```text
1. 历史贸易协议
2. 宗主国/傀儡保留供应
3. 市场圈内部供应
4. 同阵营供应
5. 中立友好贸易伙伴
6. 受限世界现货市场
```

验收：

- 同一商品有市场圈供应时，优先于普通中立国家。
- 战争敌国不会成为供应方。
- 世界现货市场不能无限满足所有缺口。

### V8.C2 — 受限世界现货市场

任务：

- 替换 `CountryId::NONE, f32::MAX` fallback。
- 新增按商品配置的世界现货供应上限和价格倍率。
- 现货市场受外汇、封锁、贸易法、战争风险影响。

验收：

- 没有真实出口方时，小额高价补缺可以发生，但无法无限填满军工缺口。
- 外汇不足时世界现货进口失败。
- 封锁时海运现货进口下降。

### V8.C3 — 市场准入与运输约束

任务：

- 每国计算 market access：港口、铁路、封锁、战争、陆路连接。
- 市场圈成员贡献受 market access 限制。
- 市场面板显示市场准入低的成员和原因。

验收：

- 日本被封锁后油/橡胶进口明显下降。
- 英国港口被封锁后帝国进口下降。
- 内陆陆路贸易不受海封锁影响，但受铁路/战争影响。


## 9. 市场面板实施计划

### V8.D1 — 数据快照拆分

任务：

- 从 `crates/hoi4-app/src/main.rs` 中抽出市场面板数据构造函数或模块。
- 新增 `MarketBlocPanelData`、`MarketBlocMemberEntry`、`MarketDependencyEntry`。
- 保留现有 `GoodEntry`，扩展供给来源枚举或来源标签。
- 区分 domestic、subject、bloc、agreement、world spot、stockpile。

验收：

- `main.rs` 中市场面板装配代码明显缩短。
- 面板数据能表达至少一个市场圈成员贡献。

### V8.D2 — 市场圈页与殖民页

任务：

- `MarketPanelTab` 增加 `MarketBloc`、`Subjects`、`Actions`。
- 市场圈页显示成员贡献和依赖。
- 殖民页显示 subject 资源、自治、抽取和风险。

验收：

- 英国开局市场面板能看到 CAN 粮食、MAL 橡胶、RAJ 贡献。
- 点击/展开成员能看到该成员贡献哪些商品。

### V8.D3 — 商品详情重构

任务：

- 商品详情显示供给来源堆叠：国内、市场圈、殖民/傀儡、世界、库存。
- 需求来源堆叠：建筑、POP、军购、建造、出口、宗主国抽取。
- 显示价格解释和行动建议。

验收：

- 橡胶详情能说明“国内 0、MAL 18、世界 0、短缺 X”。
- 粮食详情能说明“CAN 贡献、POP 消费、库存覆盖”。
- 军工中间品详情能显示上游瓶颈和未成交军购。

### V8.D4 — 新手友好行动建议

任务：

- 总览页给出 3-5 条最重要建议。
- 商品详情给出上下文建议。
- 建议先只读，不执行命令。

验收：

- 玩家能从市场面板知道该建什么、进口什么、改什么 PM、处理哪个封锁。
- 建议不泛泛而谈，必须引用具体商品/建筑/成员/路线。


## 10. 历史数据实施计划

### V8.E1 — 英帝国经济数据补齐

目标内容：

- CAN：粮食、木材、部分工业与战时人力。
- RAJ：粮食、纺织、人力、税收/外汇贡献。
- MAL：橡胶。
- SAF：黄金、矿产。
- AST/NZL：粮食、羊毛、人力。

目标文件：

```text
crates/hoi4-content/content/history_1936/countries/*.ron
crates/hoi4-content/content/history_1936/resources/state_deposits.ron
crates/hoi4-content/content/history_1936/trade/initial_trade_1936.ron
```

验收：

- 英国本土油/橡胶不足，但帝国体系可缓解。
- 失去 MAL 或海路封锁会影响英国橡胶链。
- 失去 CAN 粮食会影响英国 POP 需求满足。

### V8.E2 — 轴心、苏联和日本贸易圈

任务：

- 轴心贸易圈：德国与罗马尼亚石油、瑞典铁矿、意大利等。
- 日本势力圈：满洲、蒙疆、占领区资源，缺油脆弱。
- 苏联势力圈：内部资源和计划调拨。

验收：

- 德国无罗马尼亚油/瑞典铁时短缺明显。
- 日本被美国/英国封锁后油和橡胶断裂。
- 苏联更多依靠国内资源而非世界进口。


## 11. 自动建造系统重构计划

### 11.1 当前问题

当前自动建造位于 `crates/hoi4-app/src/main.rs::auto_enqueue_player_construction`，本质是玩家侧 UI 辅助，不是真正经济 AI：

- 每月触发一次，只在 `auto_build_enabled` 开启时为玩家国家补队列。
- 目标队列长度是 `((cp / 6) + 2).clamp(3, 8)`，与财政、投资池、国家战略、战争状态没有强绑定。
- 候选评分直接写在 app 层，混合了建筑类别常数、预期利润、军工订单、缺口商品和法团经济加成。
- 不区分政府建造、私人投资、法团投资和宗主国投资。
- 不做产业链追因：看到 `clothes` 短缺会偏向纺织厂，但不会判断纺织厂是否缺棉/染料/劳动力/市场准入；看到 `meat` 短缺不会判断粮食是否先不足。
- 不做长期规划：不会先补建造部门/电力/铁路/机床，再扩下游工厂。
- 不做财政约束：现金、债务、外汇、MEFO 风险、建材库存没有作为硬约束。
- 不做区域规划：州选择主要看基础设施和容量，没有比较资源、劳动力、市场准入、港口、战略安全和产业集群。
- 自动建造逻辑在 `main.rs` 中，难测试、难复用，私人投资 tick 又有另一套更粗的逻辑。

### 11.2 目标架构

新增独立规划模块：

```text
crates/hoi4-logic/src/economy/construction_planner.rs
```

核心结构：

```rust
pub struct ConstructionPlanContext {
    pub country: CountryId,
    pub planning_mode: ConstructionPlanningMode,
    pub budget: ConstructionBudgetSnapshot,
    pub market: MarketDiagnosisSnapshot,
    pub industry: IndustryChainSnapshot,
    pub war: WarEconomySnapshot,
}

pub enum ConstructionPlanningMode {
    PlayerAutoBuild,
    PrivateInvestment,
    CartelInvestment,
    PlannedEconomy,
    AiCountry,
    OverlordDevelopment,
}

pub struct ConstructionCandidateScore {
    pub building_id: String,
    pub state: StateId,
    pub score: f32,
    pub reasons: Vec<String>,
    pub funding_source: ConstructionFundingSource,
    pub owner_on_completion: BuildingOwner,
}
```

### 11.3 评分原则

自动建造必须从“按建筑类别加分”改成“经济诊断驱动”：

```text
总分 =
  商品缺口收益
+ 上游瓶颈缓解
+ 军工订单支撑
+ POP 生活需求收益
+ 预期利润/税收收益
+ 战略资源安全
+ 州适配度
+ 产业集群加成
+ 法律/产权适配
- 输入短缺惩罚
- 财政/建材/劳动力约束惩罚
- 重复建设/过剩产能惩罚
```

具体要求：

- 如果 `clothes` 短缺，优先评估纺织厂，但同时检查纺织厂输入、劳动力和现有库存。
- 如果 `meat` 短缺，先判断是牧场不足还是粮食不足；粮食不足时不应盲目扩牧场。
- 如果军工缺 `rubber_parts`，应优先评估橡胶来源、橡胶制品厂、进口路线，而不是只扩车辆/飞机厂。
- 如果钢铁短缺，应比较铁矿、煤矿、钢铁厂、进口铁矿/煤和现有钢厂就业率。
- 如果建造队列长期慢，应评估 construction_sector、machinery、steel、电力、铁路。
- 计划经济模式优先国家目标和基础工业；私人投资模式优先利润和消费市场；法团模式优先军工订单和上游重工业。

### 11.4 输出与 UI

自动建造不能是黑箱。建筑面板和市场面板应显示：

- 自动建造本月新增了什么。
- 每个候选的前 3 条评分原因。
- 被拒绝的主要原因：缺资源、缺工人、缺建材、财政风险、法律阻塞、市场已过剩。
- 自动建造模式：平衡、民生优先、军工优先、基础设施优先、私人市场优先、计划经济。

第一阶段只做只读解释，不必给玩家复杂参数面板；第二阶段再加模式选择。

### 11.5 验收

- 德国服装短缺时，自动建造能解释为什么选择纺织厂或为什么不选择。
- 肉类短缺但粮食不足时，自动建造优先粮食或农业基础，而不是盲目牧场。
- 军工扩张导致钢/机床/橡胶制品短缺时，自动建造优先上游链。
- 私人投资和玩家自动建造复用同一评分核心，但资金来源和产权不同。
- 自动建造逻辑可在 `hoi4-logic` 层单元测试，不依赖 `main.rs` UI 状态。


## 12. 产业链、消费品与产出规模重标定计划

### 12.1 当前问题

德国开局服装、肉类等民生商品出现几千到上万缺口，根因不是单点 bug，而是三套尺度不一致：

| 系统 | 当前尺度 | 问题 |
|---|---|---|
| POP 消费 | `employed_pop * weight * 0.001`，每个需求商品都加同样数量 | 6800 万人口会产生数万级日需求 |
| 民生建筑产出 | 纺织厂每级 `clothes 2.5`，牧场每级 `meat 2.0` | 每级日产个位数，与人口需求差 2-3 个数量级 |
| 初始建筑生成 | 德国 light/agriculture 由 GDP 部门份额生成，纺织/牧场只有个位或十几级 | 无法支撑按人口计算的消费需求 |

相关位置：

- 消费公式：`crates/hoi4-logic/src/economy/market_tick.rs::step_pop_consumption`
- 计划经济消费公式：`crates/hoi4-logic/src/economy/planned_tick.rs::step_rationing`
- UI 重建消费公式：`crates/hoi4-app/src/main.rs` 市场面板数据装配处
- 纺织/家具/酒类产出：`crates/hoi4-content/content/economy_v6/production_methods/consumer_goods.ron`
- 农业/牧场产出：`crates/hoi4-content/content/economy_v6/production_methods/agriculture.ron`
- 初始建筑目标：`crates/hoi4-content/src/v6_loader.rs::building_targets`

另一个问题是 `consumer_goods_factor` 在 `market_tick.rs::step_building_production` 中被乘到了所有建筑 output 上，而不是只影响民生供需或消费品分配。这会让战备经济下全部产出被统一打折，语义不清。

### 12.2 目标尺度

必须统一三个口径：

```text
1 建筑等级的日产量
1 POP 人口的日消费量
1 国家 1936 初始建筑规模
```

推荐采用“百万人口单位”口径：

- POP 消费先按 `population / 1_000_000` 聚合。
- 民生建筑每级日产对应几十万到数百万人需求，而不是个位数。
- 初始建筑生成以“满足基础需求比例”为硬约束，不只看 GDP 部门份额。

示例目标，不是最终平衡值：

```text
grain_farm level 1 -> grain 120-180 / day
livestock_ranch level 1 -> meat 60-90 / day, grain input 20-40
textile_mill level 1 -> clothes 80-120 / day, textiles 40-80
furniture_factory level 1 -> furniture 40-70 / day
distillery level 1 -> liquor 30-60 / day
```

对应消费也应改为：

```text
demand = pop_size / 1_000_000 * class_weight * good_need_weight * law/sol modifier
```

而不是所有需求商品共享同一个 `class_weight * 0.001`。

### 12.3 POP 需求表重构

新增内容配置：

```text
crates/hoi4-content/content/economy_v6/pop_needs.ron
```

示例：

```ron
[
  (class: Peasant, needs: [
    (good_id: "grain", tier: Essential, amount_per_million: 18.0),
    (good_id: "clothes", tier: Essential, amount_per_million: 8.0),
  ]),
  (class: Worker, needs: [
    (good_id: "grain", tier: Essential, amount_per_million: 16.0),
    (good_id: "clothes", tier: Essential, amount_per_million: 10.0),
    (good_id: "meat", tier: Normal, amount_per_million: 6.0),
    (good_id: "furniture", tier: Normal, amount_per_million: 3.0),
  ]),
]
```

要求：

- `market_tick`、`planned_tick` 和市场面板 UI 快照必须读同一张需求表。
- 删除 app 层重复硬编码的 POP 消费权重。
- 需求按阶级、生活水平、法律、配给制和战争状态调整。

### 12.4 初始建筑生成重构

`building_targets(profile)` 不能只用 GDP 部门份额。必须增加基础需求校准：

```text
required_grain = population_needs(grain) * target_self_sufficiency(grain)
required_clothes = population_needs(clothes) * target_self_sufficiency(clothes)
required_meat = population_needs(meat) * target_self_sufficiency(meat)

grain_farm_levels >= required_grain / expected_grain_output_per_level
textile_mill_levels >= required_clothes / expected_clothes_output_per_level
livestock_ranch_levels >= required_meat / expected_meat_output_per_level
```

国家差异：

- 德国：粮食较高自给，肉类和服装不能出现万级缺口，但可有轻微压力。
- 英国：本土粮食不足，但通过 CAN/RAJ/AST/NZL 帝国市场缓解。
- 日本：粮食/消费品可紧张，但不应在和平开局完全崩溃。
- 中国：农业供给强，工业消费品弱。
- 苏联：粮食总量大但消费品和服务短缺。

### 12.5 产出和输入链重构

民生产业应有更合理链路：

```text
grain_farm -> grain
livestock_ranch + grain -> meat
textile_mill + cotton/wool/dyes(后续可选) -> clothes + textiles
furniture_factory + wood/steel -> furniture
distillery + grain -> liquor
tobacco_plantation -> tobacco
```

第一阶段可以不新增 cotton/wool/dyes，但要预留；不能让纺织厂永远无输入且产出极低。

### 12.6 验收

- 德国开局 7 天后 `clothes`、`meat` 缺口不得达到几千/上万级。
- 德国可有战略资源短缺，如油、橡胶、部分军工中间品，但基础衣食不能夸张崩溃。
- 市场面板显示的 POP 需求与 `market_tick` 实际需求一致。
- 增加建筑或进口后，缺口按可解释比例下降。
- 主要国家 30/90 天回放不因民生尺度错误导致满意度全面崩盘。


## 13. 测试与验收

### 13.1 新增测试文件

```text
crates/hoi4-logic/tests/v8_investment_ownership.rs
crates/hoi4-logic/tests/v8_market_bloc_trade.rs
crates/hoi4-logic/tests/v8_puppet_colonial_economy.rs
crates/hoi4-logic/tests/v8_world_spot_market.rs
crates/hoi4-logic/tests/v8_construction_planner.rs
crates/hoi4-logic/tests/v8_consumer_goods_balance.rs
crates/hoi4-ui/tests/market_panel_v8.rs
```

### 13.2 必备测试

投资产权：

- `private_pool_project_completes_as_private_owner`
- `government_project_completes_as_state_owner`
- `cartel_project_completes_as_cartel_owner`
- `profit_distribution_follows_ownership_shares`

市场圈：

- `britain_imports_malaya_rubber_via_imperial_preference`
- `canadian_grain_has_bloc_priority_for_britain`
- `dominion_priority_does_not_force_full_resource_extraction`
- `market_bloc_member_contribution_is_visible`

傀儡经济：

- `puppet_resource_share_feeds_master_market`
- `subject_extraction_reduces_autonomy_progress`
- `integrated_puppet_industry_can_support_master_orders`

世界贸易：

- `world_spot_market_is_limited_not_infinite`
- `foreign_exchange_caps_world_spot_imports`
- `blockade_reduces_overseas_market_access`

市场面板：

- `market_panel_shows_bloc_members`
- `market_panel_splits_domestic_bloc_subject_world_supply`
- `market_panel_explains_colonial_dependency`
- `market_panel_actions_reference_specific_goods`

自动建造：

- `planner_prioritizes_textiles_when_clothes_shortage_is_real`
- `planner_prioritizes_grain_before_livestock_when_meat_chain_lacks_feed`
- `planner_explains_rejected_candidates`
- `private_investment_and_auto_build_share_scoring_but_use_different_funding`

消费品与产业尺度：

- `germany_opening_clothes_shortage_within_reasonable_band`
- `germany_opening_meat_shortage_within_reasonable_band`
- `market_tick_and_market_panel_use_same_pop_need_table`
- `consumer_goods_factor_does_not_reduce_all_industrial_output`

### 13.3 回放验收

```text
30 天英国和平回放：
  英国本土资源不足，但帝国贸易缓解粮食/橡胶压力。

30 天英国封锁回放：
  帝国海运受损，橡胶/粮食进口下降，POP 满足率或军工链受影响。

90 天德国回放：
  德国依赖罗马尼亚油、瑞典铁、合成资源；世界现货不能无限补缺。
  德国服装、肉类等基础民生缺口不得达到几千/上万级。

90 天日本封锁回放：
  油/橡胶进口下降，燃油、飞机、车辆链受影响。
```


## 14. 实施顺序

### 14.1 可执行任务列

> 执行时按 `P0 -> P1 -> P2 -> ...` 顺序推进。每个任务完成后先满足“验收”，再进入下一项。不要从后面的帝国市场或完整产权系统开始；先修基础消费品尺度和自动建造，否则后续市场信号都会被错误数据带偏。

| 编号 | 任务 | 主要文件 | 依赖 | 验收 |
|---|---|---|---|---|
| ✅ P0.1 | 新增统一 POP 需求表 | `crates/hoi4-content/content/economy_v6/pop_needs.ron`, `crates/hoi4-content/src/v6_loader.rs` | 无 | `V6Database` 能加载各阶级需求；需求项含商品、层级、每百万人需求量 |
| ✅ P0.2 | 市场 tick 改用统一 POP 需求表 | `crates/hoi4-logic/src/economy/market_tick.rs` | P0.1 | 市场经济 POP 消费不再使用硬编码 `PEASANT_NEEDS/WORKER_NEEDS/...`；需求按 `pop / 1_000_000` 计算 |
| ✅ P0.3 | 计划经济 tick 改用统一 POP 需求表 | `crates/hoi4-logic/src/economy/planned_tick.rs` | P0.1 | 计划经济配给与满意度读取同一张需求表；不再维护第二套消费硬编码 |
| ✅ P0.4 | 市场面板消费拆分改用统一 POP 需求表 | `crates/hoi4-app/src/main.rs`, `crates/hoi4-ui/src/market_panel.rs` | P0.1-P0.3 | 市场面板显示的 POP 需求与 `market_tick` 实际需求一致 |
| ✅ P0.5 | 重标定农业与民生产出 | `crates/hoi4-content/content/economy_v6/production_methods/agriculture.ron`, `consumer_goods.ron` | P0.1-P0.4 | `grain_farm/livestock_ranch/textile_mill` 等产出与百万人口需求同量级 |
| ✅ P0.6 | 德国开局民生建筑校准 | `crates/hoi4-content/src/v6_loader.rs`, `crates/hoi4-content/content/history_1936/countries/GER.ron` | P0.5 | 德国开局 7 天后 `clothes`、`meat` 缺口不得达到几千/上万级 |
| ✅ P0.7 | 修正 `consumer_goods_factor` 语义 | `crates/hoi4-logic/src/economy/market_tick.rs`, `crates/hoi4-content/content/economy_v6/laws/economy.ron` | P0.5 | 战备经济不再无差别降低所有建筑产出；测试覆盖 `consumer_goods_factor_does_not_reduce_all_industrial_output` |
| ✅ P0.8 | 添加消费品平衡测试 | `crates/hoi4-logic/tests/v8_consumer_goods_balance.rs` | P0.1-P0.7 | 覆盖德国服装/肉类合理区间、市场 tick 与面板需求一致、消费品因子语义 |
| ✅ P1.1 | 抽出自动建造规划器模块 | `crates/hoi4-logic/src/economy/construction_planner.rs`, `crates/hoi4-logic/src/economy/mod.rs` | P0 完成 | 自动建造评分核心从 `main.rs` 迁出，可单元测试 |
| ✅ P1.2 | 定义建造候选评分结构 | `construction_planner.rs` | P1.1 | 输出 `ConstructionCandidateScore { building_id, state, score, reasons }` |
| ✅ P1.3 | 接入玩家自动建造 | `crates/hoi4-app/src/main.rs` | P1.1-P1.2 | `auto_enqueue_player_construction` 调用 planner，不再直接维护完整评分逻辑 |
| ✅ P1.4 | 自动建造产业链追因 | `construction_planner.rs` | P1.3 | 衣物短缺能评估纺织厂；肉类短缺且粮食不足时优先粮食/农业基础；军工短缺能追上游 |
| ✅ P1.5 | 自动建造解释 UI | `crates/hoi4-ui/src/construction_v6_panel.rs`, `crates/hoi4-app/src/main.rs` | P1.4 | 建筑面板显示本月自动建造项目和前 3 条原因 |
| ✅ P1.6 | 添加自动建造测试 | `crates/hoi4-logic/tests/v8_construction_planner.rs` | P1.1-P1.5 | 覆盖纺织、粮食优先、拒绝原因、可测试性 |
| ✅ P2.1 | 建造队列加入资金来源与建成产权字段 | `crates/hoi4-logic/src/economy/mod.rs` | P1 完成 | `ConstructionItem` 有 `funding_source`、`owner_on_completion`、`reserved_funds_rm` |
| ✅ P2.2 | 建造完成使用队列产权 | `crates/hoi4-logic/src/economy/construction_tick.rs` | P2.1 | 私人/政府/法团项目完成后 owner 不再只由 `owner_default` 决定 |
| ✅ P2.3 | UI 显示建造资金来源和产权 | `crates/hoi4-ui/src/construction_v6_panel.rs`, `crates/hoi4-app/src/main.rs` | P2.1-P2.2 | 队列行显示政府/私人/法团与建成产权 |
| ✅ P2.4 | 添加投资产权测试 | `crates/hoi4-logic/tests/v8_investment_ownership.rs` | P2.1-P2.3 | 覆盖政府建成 State、私人建成 Private、法团建成 Cartel |
| ✅ P3.1 | 新增市场圈数据结构 | `crates/hoi4-state/src/market.rs` 或 `market_bloc.rs`, `crates/hoi4-state/src/lib.rs` | P0 完成 | 存在 `MarketBlocStore`、成员、leader、kind、country->bloc 映射 |
| ✅ P3.2 | 初始化英帝国市场圈 | `crates/hoi4-content/src/v6_loader.rs` | P3.1 | ENG、CAN、AST、NZL、SAF、RAJ、MAL 进入英帝国优惠体系 |
| ✅ P3.3 | 保留 ImperialPreference 路线语义 | `crates/hoi4-state/src/trade.rs`, `crates/hoi4-content/src/v6_loader.rs`, `crates/hoi4-logic/src/trade/mod.rs` | P3.1-P3.2 | `ImperialPreference` 不再退化成普通 `Sea` |
| ✅ P3.4 | 市场面板显示市场圈总览 | `crates/hoi4-ui/src/market_panel.rs`, `crates/hoi4-app/src/main.rs` | P3.1-P3.3 | 英国市场面板能看到“英帝国优惠体系”和成员列表 |
| ✅ P3.5 | 添加市场圈基础测试 | `crates/hoi4-logic/tests/v8_market_bloc_trade.rs` | P3.1-P3.4 | 覆盖英帝国成员、ImperialPreference 语义、成员可见 |
| ✅ P4.1 | 分层贸易撮合 | `crates/hoi4-logic/src/trade/mod.rs` | P3 完成 | 贸易顺序为历史协议、宗主/傀儡、市场圈、同阵营、中立、世界现货 |
| ✅ P4.2 | 限制世界现货市场 | `crates/hoi4-state/src/market.rs`, `crates/hoi4-logic/src/trade/mod.rs` | P4.1 | 替换 `CountryId::NONE, f32::MAX` 无限补缺 |
| ✅ P4.3 | Subject 资源优先权 | `crates/hoi4-logic/src/diplomacy/puppet.rs`, `crates/hoi4-logic/src/trade/mod.rs` | P4.1 | MAL 橡胶优先供 ENG；Dominion 不按 Puppet 强制抽取 |
| ✅ P4.4 | 添加傀儡/殖民经济测试 | `crates/hoi4-logic/tests/v8_puppet_colonial_economy.rs` | P4.1-P4.3 | 覆盖资源份额、自治度惩罚、Dominion 差异 |
| ✅ P5.1 | 市场面板商品详情来源拆分 | `crates/hoi4-ui/src/market_panel.rs`, `crates/hoi4-app/src/main.rs` | P3-P4 | 商品供给拆成国内、市场圈、傀儡/殖民、世界、库存 |
| ✅ P5.2 | 市场面板殖民/傀儡页 | `market_panel.rs` | P4.3 | 显示 subject 贡献、自治等级、抽取风险 |
| ✅ P5.3 | 市场面板行动建议页 | `market_panel.rs`, `construction_planner.rs` | P1、P5.1 | 显示具体建议：建什么、进口什么、改什么、哪条线被封锁 |
| ✅ P5.4 | 添加市场面板 V8 测试 | `crates/hoi4-ui/tests/market_panel_v8.rs` | P5.1-P5.3 | 覆盖市场圈成员、殖民依赖、供给来源拆分、具体建议 |
| ✅ P6.1 | 投资账户 | `crates/hoi4-state/src/store.rs` | P2 完成 | 新增 Private/Cartel/StateDevelopment/Colonial/Foreign 账户 |
| ✅ P6.2 | 私人投资复用 planner | `crates/hoi4-logic/src/economy/mod.rs`, `construction_planner.rs` | P1、P6.1 | `private_investment_tick` 不再使用固定 5 类建筑和固定阈值 |
| ✅ P6.3 | 产权份额与利润分配 | `crates/hoi4-state/src/buildings_v6.rs`, `crates/hoi4-logic/src/economy/finance_tick.rs` | P6.1-P6.2 | 利润按产权份额进入财政、资本家、投资账户或宗主国 |
| ✅ P7.1 | 1936 英帝国数据补齐 | `crates/hoi4-content/content/history_1936/**` | P3-P5 | CAN 粮食、MAL 橡胶、RAJ 人力/粮食/纺织等可见 |
| ✅ P7.2 | 轴心/日本/苏联贸易圈数据 | `history_1936/trade`, `countries`, `resources` | P4 | 德国、日、苏资源依赖更符合 1936 |
| ✅ P7.3 | 30/90 天回放校准 | `crates/hoi4-logic/tests/v7_global_replay_balance.rs` 或新 V8 replay test | P0-P7.2 | 英国、德国、日本关键资源依赖、民生缺口和 GDP 不崩 |

### 14.2 阶段总览

| 阶段 | 内容 | 原因 |
|---:|---|---|
| 1 | V8.A1 建造资金来源与建成产权 | 最小闭环，先修“谁出钱、归谁” |
| 2 | V8.B1 + V8.C1 市场圈基础与分层贸易 | 给阵营/帝国市场立骨架 |
| 3 | V8.D1 数据快照拆分 | 防止市场面板继续堆在 `main.rs` |
| 4 | V8.D2 市场圈页与殖民页 | 让玩家先看见新系统 |
| 5 | V8.B2 Subject 资源优先与强制贸易 | 真正让傀儡/殖民经济影响宗主国 |
| 6 | V8.C2 受限世界现货市场 | 移除无限世界市场补缺 |
| 7 | V8.A2 投资账户与再投资评分 | 让私人投资和市场短缺联动 |
| 8 | V8.12 消费品与产出规模重标定 | 先修德国服装/肉类等基础民生尺度崩坏 |
| 9 | V8.11 自动建造规划器 | 让自动建造基于产业链诊断而非 app 层常数 |
| 10 | V8.D3 商品详情重构 | 把新供需来源解释给玩家 |
| 11 | V8.B3 宗主国工业/财政/人力控制 | 深化殖民吸血与自治反馈 |
| 12 | V8.A3 产权份额与利润分配 | 完成长期产权系统 |
| 13 | V8.D4 行动建议 | 提升新手上手能力 |
| 14 | V8.E 历史数据补齐与回放校准 | 用 1936 场景验证系统价值 |


## 15. 完成定义

本路线图完成时，玩家应能在系统和 UI 中明确看到：

- 私人资本为什么投资某个行业，投资后建筑产权归谁，利润如何分配，再投资池如何增长。
- 政府、私人、法团、宗主国投资不是同一种建造队列条目。
- 英国本土无法自给橡胶和部分粮食，需要加拿大、马来亚、印度等帝国体系支撑。
- Dominion 和 Puppet 不同：加拿大是自治领贸易优先，马来亚/印度更接近殖民资源抽取。
- 阵营/帝国市场不是统一全球市场，而是优先交易和规则优惠的市场圈。
- 世界现货市场有限、昂贵、受外汇和封锁限制，不能无限补缺。
- 市场面板能用 Victoria 3 式细节解释每种商品：谁生产、谁消耗、谁进口、哪个殖民地贡献、哪条路线被封锁、为什么涨价、该怎么解决。
- 自动建造不再是黑箱，能解释为什么建某个建筑、为什么拒绝某个候选，并能按产业链上游/下游规划。
- 德国开局基础民生商品不会出现几千/上万级荒谬缺口，战略短缺集中在油、橡胶、机床、军工中间品等合理瓶颈。
- 1936 英国、德国、日本的资源依赖和战争经济脆弱性在回放中可见、可测、可解释。


## 16. 拒收清单

- 只在 UI 上显示“阵营市场”文字，但后端没有 `MarketBloc` 或等价结构。
- 继续把 `ImperialPreference` 映射为普通 `Sea` 后丢失语义。
- 继续允许 `CountryId::NONE` 无限供应所有进口缺口。
- 私人投资建造完成后仍只按 `owner_default` 决定产权。
- 傀儡的 `master_resource_share()` 只影响自治度，不影响实际资源/贸易流。
- 市场面板只显示商品表，不显示市场圈成员、殖民贡献、贸易来源和行动建议。
- 英国开局无法看出对加拿大、印度、马来亚、澳大利亚、南非等帝国体系的依赖。
- Dominion 被当成普通 Puppet 按高比例强制抽取。
- 自动建造继续留在 `main.rs` 中用固定常数评分，无法单元测试和解释候选理由。
- POP 消费、市场 tick 和市场面板继续各自硬编码一套需求公式。
- 德国开局服装、肉类等基础商品仍出现几千/上万级缺口。
- `consumer_goods_factor` 继续无差别乘到所有建筑产出上。
- 没有自动测试证明投资产权、市场圈贸易、傀儡资源抽取、自动建造规划、消费品尺度和市场面板解释能力。
