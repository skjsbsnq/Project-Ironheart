# Project Ironheart V7 — 1936 历史经济·资源·建筑·POP·贸易·法律·军事全系统重构路线图

> **本文档是 [`ROADMAP_V6_ECONOMY.md`](./ROADMAP_V6_ECONOMY.md)、[`ROADMAP_V6_FINISH.md`](./ROADMAP_V6_FINISH.md) 与 [`ROADMAP_V6_BUILDINGS_UI_REFACTOR.md`](./ROADMAP_V6_BUILDINGS_UI_REFACTOR.md) 之后的历史校准与全系统重构路线。**
> 写于 2026-05-23。目标是把当前 V6 经济雏形从“能运行的经济系统”升级为“符合 1936 世界经济结构、资源约束、国家差异和战争动员逻辑的历史经济模拟”。
> 本路线不是单纯调 GDP 显示值，而是建立 **历史 GDP / 人口 / 资源 / 部门结构 -> 初始建筑 / POP / 贸易 / 法律 / 军事 -> 运行期 GDP** 的可解释闭环。


## 0. 为什么需要这份重构

2026-05-23 用户反馈：当前经济、POP、建造、生产、贸易、建筑、法律、国策、决议、军事等与经济相关系统，运行起来既不符合现实，也不符合 1936 年事实；各国 GDP 与真实世界 GDP 不符；资源建筑可以随意建设；各国初始建筑缺乏历史依据。

审计结论：

| 问题类型 | 当前现状 | 后果 |
|---|---|---|
| GDP 与实物经济脱节 | GER/SOV 有硬编码初始 GDP，建筑规模由启发式注入 | 顶栏 GDP 数字不可解释，经济运行后会偏离历史 |
| 资源建筑无资源约束 | `buildings.ron` 中资源建筑 `state_limit_kind: None` | 煤矿、油田、橡胶园可在任意州建设 |
| 初始建筑靠硬编码猜测 | `v6_loader.rs` 通过州名、基建、人口启发式注入建筑 | 各国工业分布、资源分布、1936 经济体量严重失真 |
| 多数国家无历史经济 profile | 当前只有 GER POP，SOV/其他国家只有非常粗的建筑生成 | USA/UK/JAP/CHI/ITA/FRA 等关键国家没有可信经济结构 |
| 贸易无历史依赖 | 动态撮合存在，但缺少 1936 初始贸易路线和资源依赖 | 德国/Japan/UK 的石油、橡胶、铁矿、粮食压力不真实 |
| 法律与国家制度不够深 | 6 类法律 schema 已有，但各国初始法律和系统影响不足 | 苏联计划经济、德国法团战备、美国大萧条、英国帝国贸易差异不明显 |
| 国策/决议仍偏旧系统 | 内容主要集中 GER，且没有完全绑定 V6 建筑、资源、贸易、财政、军工订单 | 国策无法真实改造经济结构 |
| 军事经济闭环不足 | 军队、装备、军费、动员、采购与经济仍有隔离 | 扩军不真正伤经济，军工订单与财政风险不足 |

**根因一句话**：V6 已经有建筑经济、市场、财政、法律、贸易、POP 的骨架，但缺少一层严肃的 1936 历史数据和由历史数据生成初始世界的机制。


## 1. 范围

### 1.1 范围内
- ✅ 新增 1936 历史经济数据层：国家 GDP、人口、部门结构、城市化、失业、军费、债务、外汇、黄金储备。
- ✅ 新增州级资源储量系统：资源建筑只能在有资源 deposit 的州建设，并受已探明储量上限限制。
- ✅ 用 GDP、人口、资源和部门结构反推各国初始建筑，替代 `v6_loader.rs` 中的硬编码启发式注入。
- ✅ 为主要国家设计 1936 初始建筑规模和分布：USA、GER、SOV、ENG、FRA、JAP、ITA、CHI 第一批；POL、ROM、SWE、BEL、NED、CAN、AUS 第二批。
- ✅ 为主要国家生成 POP：农民、工人、职员、资本家、贵族、士兵，并与建筑就业、动员、满意度挂钩。
- ✅ 建立历史初始贸易路线：德国石油/橡胶缺口，日本资源进口，英国帝国贸易，罗马尼亚石油，瑞典铁矿，马来亚橡胶等。
- ✅ 重构法律初始状态和法律效果：征兵、经济、贸易、税收、公民权、信息控制全部影响经济运行。
- ✅ 重构国策和决议效果：不再加旧工厂，而是操作 V6 建筑、资源勘探、贸易、PM、政府订单、MEFO、债务和法律。
- ✅ 重构军事经济闭环：军费 = 军人工资 + 装备采购 + 维护 + 燃油 + 训练；扩军从 POP 中抽人并影响建筑就业。
- ✅ 建立 GDP 校准测试：开局 30/90 天后主要国家 GDP 与目标值保持在容许误差内。

### 1.2 范围外
- ❌ 不追求每一个国家每一个州的完全学术级 GDP 精确值；第一阶段以主要国家和战略资源为准。
- ❌ 不做 Victoria 3 级别全世界逐 POP 文化/宗教/财富模拟；仍沿用 V6 的 6 类 POP 简化模型。
- ❌ 不做完整殖民地自治与帝国财政系统；第一阶段用贸易、资源控制和宗主国市场近似。
- ❌ 不重写地图渲染。
- ❌ 不做存档兼容。历史经济 schema 改动后，旧中间存档不保证可读。
- ❌ 不恢复旧 HOI4 civ/mil/dock 三槽经济；V7 必须继续 V6 建筑经济方向。


## 2. 关键决策点（V7 锁定）

| # | 决策点 | 选定方案 |
|---|---|---|
| H1 | GDP 口径 | 使用游戏内部 `gdp_1936_gbp` 作为国际比较口径；各国货币只用于财政记账 |
| H2 | GDP 来源 | GDP 必须由建筑增加值、POP 消费、政府支出、军工采购、净出口解释，不允许只写显示数字 |
| H3 | 初始建筑 | 由历史 GDP、部门结构、资源储量、人口就业反推生成；禁止继续用州名启发式硬编码 |
| H4 | 资源建筑 | 资源建筑必须绑定州级 deposit；无 deposit、已达探明储量上限、缺少科技时不可建 |
| H5 | 资源储量 | 区分 `discovered_level` 与 `potential_level`，允许通过决议/科技/国策勘探 |
| H6 | 国家范围 | 第一批精调 8 国：USA/GER/SOV/ENG/FRA/JAP/ITA/CHI |
| H7 | 贸易 | 1936 初始贸易路线显式配置；运行期贸易再由供需、法律、封锁、外汇动态调整 |
| H8 | POP | 所有主要国家都有 POP；非主要国家允许由人口和经济 profile 自动生成简化 POP |
| H9 | 法律 | 法律改变系统规则，而不是只给 modifier；计划经济、法团战备经济、自由市场要有不同 tick 逻辑 |
| H10 | 国策/决议 | 国策与决议必须使用 V6/V7 effect，不允许新增旧工厂/旧资源数值 |
| H11 | 军事 | 军事系统必须向经济系统输出真实需求：人力、装备、燃油、维护、训练、军工订单 |
| H12 | 验证 | 每个阶段都必须有自动测试或 1936 回放验收，不能只靠 UI 观感 |


## 3. 当前栈审计

### 3.1 建筑与资源
- 建筑定义位于 `crates/hoi4-content/content/economy_v6/buildings/buildings.ron`。
- 资源建筑包括 `iron_mine / coal_mine / oil_rig / rubber_plantation / bauxite_mine / chromium_mine / tungsten_mine`。
- 当前资源建筑普遍 `state_limit_kind: None`，没有州级资源储量约束。
- 建造逻辑位于 `crates/hoi4-logic/src/economy/construction_tick.rs`，只校验国家是否拥有目标州、法律是否满足、建造力是否推进。
- 当前 `construction_tick.rs` 没有资源、海岸、地形、城市容量、农业土地、基建、人口校验。

### 3.2 初始建筑
- 德国建筑由 `v6_loader.rs::inject_ger_buildings_from_vanilla` 注入。
- 苏联建筑由 `v6_loader.rs::inject_sov_buildings_from_vanilla` 注入。
- 其他国家由 `inject_baseline_buildings_for_remaining_countries` 给一个候选州塞少量农场/钢厂/纺织厂/铁路。
- 这套逻辑无法表达美国的超大工业、英国金融与海运、日本资源依赖、中国农业人口和低工业化、罗马尼亚石油、瑞典铁矿等历史结构。

### 3.3 GDP 与财政
- `Treasury` 已有 `gdp_gbp / gdp_rm / gdp_growth_yoy / reserve_gbp / gold_kg / public_debt / mefo_debt`。
- 德国初始 `gdp_rm = 83_000_000_000.0`，苏联初始 `gdp_rm = 45_000_000_000.0`，但建筑规模不是从该数反推。
- `finance_tick.rs::step_update_gdp` 已有运行期 GDP 更新，但需要改成和历史初始建筑校准一致。

### 3.4 POP
- POP schema 在 `crates/hoi4-state/src/pops.rs`。
- 目前只有 `content/economy_v6/pops/initial_ger.ron` 明确写了德国 POP。
- 其他国家没有可信 POP，无法真实模拟动员、就业、消费、满意度、税收。

### 3.5 生产与市场
- `market_tick.rs` 已按建筑、PM、输入、输出、就业、基建、法律计算商品供需。
- `planned_tick.rs` 为计划经济单独 tick，是正确方向。
- 旧 `EconomyState.production` 和旧 UI 生产线仍有残留，应继续降级为 legacy 并最终删除。

### 3.6 贸易
- `crates/hoi4-logic/src/trade/mod.rs` 已有进口、出口、外汇、封锁、关税、贸易路线雏形。
- 当前缺少 1936 历史贸易 profile，贸易主要由供需自动撮合，不能表现战略依赖。

### 3.7 法律、国策、决议、军事
- 法律 schema 已有 6 类：Conscription / Economy / Trade / Taxation / CivilRights / InformationControl。
- 国策和决议内容仍主要是德国，且 effect 体系需要扩展到 V6/V7 经济对象。
- 军事系统位于 `crates/hoi4-logic/src/military/*`，与经济系统的采购、军费、动员、补员闭环仍不足。


## 4. 目标数据模型

### 4.1 历史国家经济 profile

新增路径：

```text
crates/hoi4-content/content/history_1936/countries/*.ron
```

目标 schema：

```rust
pub struct HistoricalCountryEconomyDef {
    pub tag: String,
    pub population: u32,
    pub gdp_1936_gbp: f64,
    pub gdp_quality: HistoricalDataQuality,
    pub sector_shares: SectorShares,
    pub urbanization: f32,
    pub literacy: f32,
    pub unemployment: f32,
    pub industrial_capacity_index: f32,
    pub military_spending_share: f32,
    pub construction_capacity_index: f32,
    pub gold_reserve_gbp: f64,
    pub foreign_exchange_reserve_gbp: f64,
    pub public_debt_gbp: f64,
    pub initial_laws: InitialLawSet,
}
```

部门占比：

```rust
pub struct SectorShares {
    pub agriculture: f32,
    pub mining: f32,
    pub heavy_industry: f32,
    pub light_industry: f32,
    pub services: f32,
    pub government: f32,
    pub military_industry: f32,
}
```

### 4.2 州级资源储量

新增路径：

```text
crates/hoi4-content/content/history_1936/resources/state_deposits.ron
```

目标 schema：

```rust
pub struct StateResourceDepositDef {
    pub state_id: u16,
    pub deposits: Vec<ResourceDepositDef>,
}

pub struct ResourceDepositDef {
    pub good_id: String,
    pub discovered_level: u8,
    pub potential_level: u8,
    pub extraction_difficulty: f32,
    pub requires_tech: Option<String>,
}
```

规则：

| 字段 | 含义 |
|---|---|
| `discovered_level` | 1936 已探明、可直接建设的资源建筑上限 |
| `potential_level` | 通过勘探、科技、国策可开发到的最终上限 |
| `extraction_difficulty` | 影响建设成本、工资、产出效率 |
| `requires_tech` | 深井钻探、合成工艺、热带种植等科技门槛 |

### 4.3 建筑校准信息

`BuildingDef` 新增校准字段：

```rust
pub struct BuildingCalibrationDef {
    pub sector: EconomicSector,
    pub annual_value_gbp_per_level: f64,
    pub employment_per_level: u32,
    pub urban_capacity_use: u8,
    pub arable_land_use: u8,
}
```

目的：
- 从目标部门 GDP 反推建筑等级。
- 从建筑等级反推就业岗位。
- 校验 POP 是否足够。
- 校验州容量是否足够。

### 4.4 州经济 profile

新增或扩展州经济信息：

```rust
pub struct StateEconomicProfileDef {
    pub state_id: u16,
    pub arable_land: u8,
    pub urban_capacity: u8,
    pub industrial_capacity: u8,
    pub coastal: bool,
    pub major_port: bool,
    pub terrain: String,
    pub climate: String,
}
```

### 4.5 历史贸易 profile

新增路径：

```text
crates/hoi4-content/content/history_1936/trade/initial_trade_1936.ron
```

目标 schema：

```rust
pub struct HistoricalTradeProfileDef {
    pub importer: String,
    pub exporter: String,
    pub good_id: String,
    pub daily_quantity: f32,
    pub route_kind: TradeRouteKindDef,
    pub port_state: Option<u16>,
    pub strategic_importance: f32,
}
```

### 4.6 军事经济需求

新增经济需求接口：

```rust
pub struct MilitaryDemand {
    pub manpower_needed: u32,
    pub equipment_needed: HashMap<String, f32>,
    pub fuel_needed: f32,
    pub maintenance_rm: f64,
    pub training_goods: HashMap<String, f32>,
}
```


## 5. GDP 反推初始建筑算法

### 5.1 基本原则

初始建筑不是手填感觉值，而由以下输入决定：

```text
国家 GDP
+ 部门占比
+ 人口与就业结构
+ 州资源储量
+ 工业区权重
+ 基建与城市容量
+ 法律与所有制
= 初始建筑列表
```

### 5.2 算法步骤

1. 读取 `HistoricalCountryEconomyDef`。
2. 将 `gdp_1936_gbp` 按 `sector_shares` 拆成农业、采矿、重工业、轻工业、服务、政府、军工目标值。
3. 根据各建筑 `annual_value_gbp_per_level` 计算目标等级。
4. 资源建筑先按州级 deposit 分配，绝不超过 `discovered_level`。
5. 农业建筑按 `arable_land`、人口、地形、气候分配。
6. 工业建筑按工业区权重、城市容量、基建、已有资源接近度分配。
7. 服务和金融建筑按城市化、首都、港口、贸易中心分配。
8. 军工建筑按军费占比、历史军工基础、国策状态分配。
9. 生成 POP，就业岗位不能超过州内可用 POP；若超出则降低建筑等级或调整 PM 效率。
10. 跑 30 天模拟，GDP 偏离目标超过阈值则自动生成校准报告。

### 5.3 伪代码

```rust
for country in historical_profiles {
    let target_gdp = country.gdp_1936_gbp;
    let sector_targets = split_by_sector(target_gdp, country.sector_shares);

    let mut buildings = Vec::new();
    buildings.extend(allocate_resource_buildings(country, deposits, sector_targets.mining));
    buildings.extend(allocate_agriculture(country, states, sector_targets.agriculture));
    buildings.extend(allocate_industry(country, states, sector_targets.heavy_industry));
    buildings.extend(allocate_light_industry(country, states, sector_targets.light_industry));
    buildings.extend(allocate_services(country, states, sector_targets.services));
    buildings.extend(allocate_government(country, states, sector_targets.government));
    buildings.extend(allocate_military_industry(country, force_profile, sector_targets.military_industry));

    validate_resource_caps(buildings, deposits)?;
    validate_state_capacity(buildings, states)?;
    validate_population(country, buildings)?;
    validate_gdp(country, buildings, target_gdp)?;
}
```

### 5.4 误差门槛

| 国家类别 | 30 天 GDP 误差 | 90 天 GDP 误差 |
|---|---:|---:|
| 第一批主要国家 | ±10% | ±8% |
| 第二批重要国家 | ±15% | ±12% |
| 其他国家 | ±25% | ±20% |


## 6. 1936 主要国家初始方向

### 6.1 GDP 相对校准

第一版采用相对经济体量，不绑定单一现代资料口径。

| 国家 | 目标相对体量（USA=100） | 设计重点 |
|---|---:|---|
| USA | 100 | 最大工业、资源和消费市场；初始军工未完全动员 |
| GER | 40-45 | 重工业强、煤铁强、油橡胶弱、军工扩张、MEFO 风险 |
| SOV | 38-45 | 总量大、效率低、计划经济、重工业和农业并存 |
| ENG | 30-35 | 金融、海运、殖民贸易、海军工业，本土资源不足 |
| FRA | 25-30 | 工业中等、农业占比较高、动员和政治效率偏低 |
| JAP | 18-22 | 工业化较强、造船强、资源极缺、进口依赖极高 |
| ITA | 14-18 | 工业弱于列强，资源短缺，北强南弱 |
| CHI | 18-25 | 人口巨大、农业为主、工业和基建弱 |

### 6.2 德国 GER 初始建筑方向

| 建筑 | 全国等级建议 | 分布 |
|---|---:|---|
| `coal_mine` | 14-18 | 鲁尔、西里西亚、萨尔 |
| `iron_mine` | 5-8 | 西里西亚、中德少量 |
| `steel_mill` | 12-16 | 鲁尔、西里西亚、萨克森 |
| `machinery_workshop` | 12-15 | 鲁尔、柏林、萨克森、巴伐利亚 |
| `chemical_plant` | 6-9 | 莱茵、萨克森 |
| `electrical_works` | 4-6 | 柏林、莱茵 |
| `textile_mill` | 5-7 | 萨克森、南德 |
| `bank` | 3-4 | 柏林、汉堡、法兰克福类州 |
| `railway` | 12-18 | 高基建工业州 |
| `construction_sector` | 4-6 | 柏林、鲁尔、萨克森 |
| `arms_industry` | 5-8 | 柏林、鲁尔、萨克森 |
| `munition_plant` | 3-5 | 柏林、鲁尔 |
| `shipyard` | 3-5 | 汉堡、基尔 |
| `oil_rig` | 0-1 | 少量，不能解决战略需求 |
| `rubber_plantation` | 0 | 本土禁止 |

### 6.3 美国 USA 初始建筑方向

| 建筑 | 全国等级建议 | 分布 |
|---|---:|---|
| `coal_mine` | 25-35 | 阿巴拉契亚、宾州 |
| `iron_mine` | 15-25 | 五大湖、明尼苏达 |
| `oil_rig` | 25-35 | 德州、俄克拉荷马、加州 |
| `steel_mill` | 25-35 | 五大湖、宾州 |
| `machinery_workshop` | 25-35 | 东北、五大湖 |
| `electrical_works` | 12-18 | 东北、五大湖 |
| `chemical_plant` | 10-15 | 工业州 |
| `textile_mill` | 12-18 | 新英格兰、南部 |
| `bank` | 8-12 | 纽约、芝加哥等 |
| `construction_sector` | 8-12 | 受大萧条法律/决议限制 |
| `arms_industry` | 3-5 | 初始低 |
| `shipyard` | 8-12 | 东西海岸 |
| `aircraft_factory` | 2-4 | 加州、东北 |

### 6.4 苏联 SOV 初始建筑方向

| 建筑 | 全国等级建议 | 分布 |
|---|---:|---|
| `grain_farm` | 30-45 | 乌克兰、伏尔加、西伯利亚 |
| `coal_mine` | 18-25 | 顿巴斯、库兹巴斯 |
| `iron_mine` | 12-18 | 乌拉尔、乌克兰 |
| `oil_rig` | 8-12 | 巴库、高加索 |
| `steel_mill` | 12-18 | 乌拉尔、乌克兰、莫斯科周边 |
| `machinery_workshop` | 10-15 | 莫斯科、列宁格勒、乌拉尔 |
| `chemical_plant` | 4-6 | 工业核心州 |
| `construction_sector` | 8-12 | 计划经济高建造力 |
| `arms_industry` | 5-8 | 莫斯科、列宁格勒、乌拉尔 |
| `munition_plant` | 4-6 | 工业核心 |
| 消费品建筑 | 偏低 | 用消费品短缺体现计划经济压力 |

### 6.5 英国 ENG 初始建筑方向

| 建筑 | 全国等级建议 | 分布 |
|---|---:|---|
| `coal_mine` | 12-18 | 威尔士、约克郡、苏格兰 |
| `iron_mine` | 3-5 | 本土少量 |
| `steel_mill` | 8-12 | 米德兰、北英格兰 |
| `machinery_workshop` | 8-12 | 米德兰、伦敦周边 |
| `textile_mill` | 6-10 | 兰开夏 |
| `bank` | 8-12 | 伦敦高等级 |
| `port` | 高 | 伦敦、利物浦、格拉斯哥等 |
| `shipyard` | 8-12 | 主要海军基地 |
| `arms_industry` | 3-5 | 初始中低 |
| `oil_rig` | 0 | 依赖进口 |
| `rubber_plantation` | 0 本土 | 殖民地/贸易来源 |

### 6.6 日本 JAP 初始建筑方向

| 建筑 | 全国等级建议 | 分布 |
|---|---:|---|
| `coal_mine` | 5-8 | 北九州、北海道 |
| `iron_mine` | 1-3 | 严重不足 |
| `steel_mill` | 6-9 | 北九州、本州工业带 |
| `machinery_workshop` | 7-10 | 东京、大阪、名古屋 |
| `shipyard` | 8-12 | 吴、横须贺、长崎等 |
| `arms_industry` | 4-6 | 本州 |
| `munition_plant` | 3-5 | 本州 |
| `textile_mill` | 6-8 | 大阪等 |
| `oil_rig` | 0-1 | 几乎无 |
| `rubber_plantation` | 0 | 必须进口或南进 |

### 6.7 中国 CHI 初始建筑方向

| 建筑 | 全国等级建议 | 分布 |
|---|---:|---|
| `grain_farm` | 50+ | 全国农业州 |
| `coal_mine` | 8-12 | 山西、华北 |
| `iron_mine` | 3-6 | 华北、华中少量 |
| `tungsten_mine` | 4-8 | 江西、湖南、华南 |
| `steel_mill` | 2-4 | 少数工业城市 |
| `machinery_workshop` | 2-4 | 上海、南京、武汉等 |
| `textile_mill` | 6-10 | 上海、江浙 |
| `arms_industry` | 1-3 | 极少 |
| `construction_sector` | 2-4 | 基建弱 |
| `railway` | 低且断裂 | 战略移动弱 |


## 7. 资源建筑限制方案

### 7.1 数据改造

`buildings.ron` 中资源建筑必须改为：

```ron
(
    id: "coal_mine",
    name: "煤矿",
    kind: Resource,
    max_level: 10,
    owner_default: Private,
    buildable: true,
    group: "资源与农业",
    state_limit_kind: Some("coal"),
    requires_law: None,
)
```

同理：

| 建筑 | `state_limit_kind` |
|---|---|
| `iron_mine` | `iron` |
| `coal_mine` | `coal` |
| `oil_rig` | `oil` |
| `rubber_plantation` | `rubber` |
| `bauxite_mine` | `bauxite` |
| `chromium_mine` | `chromium` |
| `tungsten_mine` | `tungsten` |

### 7.2 建造校验

`construction_tick.rs` 新增：

```rust
fn validate_build_location(
    world: &World,
    db: &V6Database,
    ci: usize,
    building_def: &BuildingDef,
    state: StateId,
) -> Result<(), BuildBlockReason>
```

校验内容：

| 校验 | 失败原因 |
|---|---|
| 国家拥有该州 | `NotOwned` |
| 法律满足 | `LawBlocked` |
| 资源 deposit 存在 | `NoResourceDeposit` |
| 未超过已探明储量 | `DepositExhausted` |
| 海岸建筑需要海岸 | `RequiresCoast` |
| 港口/船坞需要港口容量 | `RequiresPort` |
| 农业建筑需要可耕地 | `NoArableLand` |
| 城市工业需要城市容量 | `NoUrbanCapacity` |
| 高级资源需要科技 | `RequiresTech` |

### 7.3 UI 展示

建造面板和州面板必须显示：

```text
不可建设：本州无煤炭储量
不可建设：已达到已探明储量上限 4/4
不可建设：需要科技“深井钻探”
不可建设：橡胶种植园需要热带气候
不可建设：船坞需要海岸与港口
```


## 8. POP、生产、贸易、法律、军事联动

### 8.1 POP 生成

由国家 profile 自动生成 POP：

| POP | 来源 |
|---|---|
| Peasant | 农业人口、低城市化人口 |
| Worker | 工业、采矿、建筑、铁路就业 |
| Clerk | 服务、政府、金融、科研就业 |
| Capitalist | 私营工业、银行、贸易所有者 |
| Aristocrat | 地主、传统军官集团、殖民统治阶层 |
| Soldier | 常备军、准军事、动员池 |

### 8.2 生产闭环

生产主体必须是建筑：

```text
建筑等级 × PM × 就业率 × 输入满足率 × 基建 × 法律 = 商品/装备产出
```

旧 `ProductionLine` 不再作为经济主模型。

### 8.3 贸易闭环

贸易分两层：

| 层 | 作用 |
|---|---|
| 历史初始贸易 | 1936 开局固定重要贸易路线和依赖 |
| 动态市场贸易 | 运行期根据价格、短缺、法律、封锁、外汇调整 |

必须表现：
- 德国缺石油、橡胶，部分依赖进口和合成。
- 日本缺石油、铁矿、橡胶，海运线极其关键。
- 英国依赖帝国海运和海外资源。
- 罗马尼亚石油、瑞典铁矿、马来亚橡胶具有战略价值。

### 8.4 法律闭环

法律不是显示 modifier，而是系统规则：

| 法律类 | 必须影响 |
|---|---|
| Conscription | Soldier 上限、转化速度、工业劳动力损失、满意度 |
| Economy | 所有制、工资、建造力、军工订单、计划 tick |
| Trade | 进口效率、出口效率、外汇管制、关税、封锁脆弱性 |
| Taxation | 所得税、消费税、企业税、满意度、私人投资 |
| CivilRights | 福利、科研、满意度、镇压成本 |
| InformationControl | 忠诚变化、战争支持、事件风险 |

### 8.5 军事经济闭环

军费公式：

```text
每日军费 = 士兵工资 + 装备采购 + 装备维护 + 燃油 + 训练 + 海军维护 + 空军维护
```

军事系统必须输出需求：

```text
部队缺口 -> 政府采购订单 -> 军工建筑生产 -> 装备库存 -> 部队补充
```

动员必须伤经济：

```text
农民/工人/职员 -> Soldier -> 工厂空岗 -> 产出下降 -> 税基变化 -> 满意度变化
```

### 8.6 军事系统重置总目标

2026-05-23 追加要求：军事系统不能只做军费闭环，必须整体重置为接近 HOI4 的“模板编辑器 + 训练部署 + 装备补给 + 占领区 + 傀儡国 + 地图控制色”体系。

当前审计：

| 系统 | 当前实现 | 缺口 |
|---|---|---|
| 师模板 | `hoi4-data/src/military.rs::DivisionTemplate` 只有 `regiments: Vec<String>` 与 `support: Vec<String>` | 没有 5×5 营格、支援连槽、宽度预览、装备需求预览、复制/保存/改名 |
| 训练 | `military.rs` 只有模板列表和 `Train` 按钮；`spawn_from_template` 直接生成师 | 没有训练队列、训练时间、部署地点、装备/人力阻塞、优先级 |
| 装备 | `stockpile.rs` 能按模板计算需求并消耗库存 | 缺少模板编辑时实时需求、训练中师的装备分配和缺口 UI |
| 占领区 | `StateStore` 有 `resistance/compliance`，地图有 Resistance 模式 | 没有占领政策、驻军模板、镇压需求、抵抗破坏、顺从收益 |
| 傀儡国 | `diplomacy/puppet.rs` 有自治度雏形 | 工业值是占位 0，缺少资源/贸易/兵役/军控/自治度变化来源 |
| 占领染色 | `owner/controller` 字段已存在，`build_occupation_lut` 有条纹 | 政治图 `fill_political` 仍按 owner 染色，不符合“占领地显示占领国颜色”要求 |

重置目标：

| 目标 | 说明 |
|---|---|
| 模板编辑器 | 加入 HOI4 风格 5 列战斗营网格、支援连槽、复制、改名、保存、经验花费、装备需求预览 |
| 训练部署 | 新师进入训练队列，按训练时间、人力、装备、部署州逐步生成 |
| 装备补给 | 模板变化、训练、战损、补员都走装备库存和军工采购 |
| 占领区 | 占领土地不等于吞并，使用 controller 表示军事控制，owner 保留法理归属 |
| 驻军系统 | 占领区需要驻军模板和装备，镇压不足会抵抗上升、破坏建筑/铁路/资源 |
| 傀儡国 | 傀儡保留 owner 和经济主体，通过自治度决定资源、贸易、军队、外交和工业控制 |
| 地图染色 | 战争中被占领省份政治图底色必须使用 controller 国家颜色，与占领国完全一致 |

### 8.7 HOI4 风格师模板编辑器

当前 `DivisionTemplate` 需要从扁平列表升级为可编辑布局。

目标 schema：

```rust
pub struct EditableDivisionTemplate {
    pub id: u32,
    pub name: String,
    pub country: CountryId,
    pub line_battalions: [[Option<String>; 5]; 5],
    pub support_companies: [Option<String>; 5],
    pub division_names_group: Option<String>,
    pub priority: ReinforcementPriority,
    pub locked_by_history: bool,
}
```

模板编辑规则：

| 操作 | 规则 |
|---|---|
| 新建模板 | 可从空模板或现有模板复制 |
| 改名 | 只改本国模板，不影响其他国家 |
| 添加战斗营 | 必须已解锁 subunit，消耗陆军经验 |
| 删除战斗营 | 可退还少量经验或不退还，第一版不退还 |
| 添加支援连 | 需要对应科技和支援装备 |
| 保存模板 | 重新计算 combat width、manpower、equipment、supply、suppression |
| 历史模板 | 1936 初始模板可编辑，但首次修改需要复制或支付经验 |

模板 UI 必须显示：

```text
师名 / 复制 / 改名 / 保存
5×5 战斗营网格
5 个支援连槽
战斗宽度、人员、组织度、软攻、硬攻、防御、突破、装甲、穿甲、补给、镇压
装备需求：步兵装备、火炮、支援装备、卡车、坦克等
库存满足率：当前库存能训练多少个该模板
训练时间与部署限制
```

最小 UI 命令：

```rust
pub enum TemplateEditorCommand {
    Open(u16),
    Clone(u16),
    Rename(u16, String),
    SetLineBattalion { template: u16, row: u8, col: u8, subunit: Option<String> },
    SetSupportCompany { template: u16, slot: u8, subunit: Option<String> },
    Save(u16),
    Delete(u16),
}
```

### 8.8 训练、部署与补员

现有 `spawn_from_template` 只能直接实例化师，V7 需要引入训练队列。

目标 schema：

```rust
pub struct TrainingQueueItem {
    pub id: u32,
    pub owner: CountryId,
    pub template_id: u32,
    pub count: u8,
    pub deploy_state: StateId,
    pub progress_days: f32,
    pub required_days: f32,
    pub manpower_allocated: u32,
    pub equipment_allocated: HashMap<String, f32>,
    pub priority: ReinforcementPriority,
}
```

训练规则：

| 规则 | 说明 |
|---|---|
| 人力先锁定 | 从 Soldier POP 池预留，取消训练返还 |
| 装备逐步分配 | 从库存按优先级分配，缺装备则训练可继续但部署强度不足 |
| 训练时间 | 由模板内营的 `training_time`、法律、军官团、经验决定 |
| 部署地点 | 必须是本国控制州，不能部署到敌占省份 |
| 部署强度 | 按人力和装备满足率决定初始 strength |
| 模板更新 | 现役师可切换模板，但会产生装备缺口和组织度惩罚 |

### 8.9 占领区系统

占领系统必须明确区分：

```text
owner = 法理拥有者
controller = 当前军事控制者
occupier = 对该州执行占领政策的国家，通常等于 controller
core = 是否为核心州
```

新增占领状态：

```rust
pub struct OccupationState {
    pub state: StateId,
    pub owner: CountryId,
    pub controller: CountryId,
    pub policy: OccupationPolicy,
    pub garrison_template_id: Option<u32>,
    pub required_suppression: f32,
    pub provided_suppression: f32,
    pub resistance: f32,
    pub compliance: f32,
    pub last_sabotage_day: i64,
}
```

占领政策：

| 政策 | 效果 |
|---|---|
| 温和占领 | 顺从增长较快，资源/工业获取低，抵抗低 |
| 民政管理 | 均衡 |
| 军事管制 | 资源/工业获取较高，抵抗较高 |
| 严酷镇压 | 短期抵抗下降，长期顺从下降，驻军消耗和政治风险高 |
| 掠夺经济 | 资源/工业获取最高，抵抗、破坏、国际紧张度高 |

抵抗与顺从影响：

| 指标 | 影响 |
|---|---|
| 抵抗 | 破坏铁路、工厂、资源建筑、补给，造成驻军伤亡和装备损耗 |
| 顺从 | 提高本地资源和工业可用比例，降低驻军需求 |
| 镇压不足 | 抵抗上升，破坏事件概率上升 |
| 驻军模板 | 使用模板的 `suppression`、人力和装备需求计算镇压 |

占领经济接入：

```text
占领州建筑产出 × 占领政策系数 × 顺从系数 × 抵抗惩罚 = 占领国可用产出
```

### 8.10 傀儡国系统

现有 `AutonomyLevel` 可以保留，但必须接入 V7 经济与战争。

傀儡关系必须影响：

| 领域 | 效果 |
|---|---|
| 资源 | 按自治等级给主国资源份额或贸易优先权 |
| 工业 | 深度傀儡可被主国使用部分军工/船坞/建设能力；普通傀儡只提供贸易和驻军 |
| 军事 | 主国可请求远征军、征用人力、控制傀儡师模板或训练队列 |
| 外交 | 傀儡不能自由宣战或加入阵营，随主国参战 |
| 贸易 | 主国市场优先，外汇结算可被主国控制 |
| 自治度 | 贸易贡献、租借、参战、建设援助、主国压榨共同改变自治进度 |

自治度计算不能继续使用占位工业值 0：

```text
daily_autonomy_delta =
    subject_industry / master_industry × base
  + subject_war_contribution
  + lend_lease_received_bonus
  - master_resource_extraction
  - forced_trade_penalty
  - occupation_policy_penalty
```

傀儡类型：

| 类型 | 用途 |
|---|---|
| IntegratedPuppet | 德国总督辖区、深度控制殖民地 |
| Puppet | 标准傀儡 |
| Dominion | 英联邦自治领 |
| Satellite | 苏联/势力范围卫星国 |
| FreedomAssociation | 准独立盟友 |

### 8.11 占领地图染色

用户硬要求：战争中被占领土地颜色应该和占领国一模一样。

当前问题：

```rust
fn fill_political(world: &World, lut: &mut [u8]) {
    let owner = world.provinces.owners[prov_idx];
    let c = world.countries.colors[owner.0 as usize];
}
```

目标改为：

```rust
let controller = world.provinces.controllers[prov_idx];
let country_for_color = if !controller.is_none() { controller } else { owner };
```

地图规则：

| 地图模式 | 染色规则 |
|---|---|
| Political | 使用 controller 颜色；被德国占领的法国省份显示德国色 |
| Ideology | 使用 controller 的意识形态颜色 |
| Resistance | 使用 owner/controller 差异显示抵抗与顺从 |
| Cores | 仍显示法理核心和 owner 信息，避免玩家误认吞并 |
| Occupation overlay | 政治图下弱化条纹，只作为“非核心占领地”提示 |

刷新规则：

| 触发 | 动作 |
|---|---|
| province controller 改变 | 标记 `color_lut_dirty` 和 `occupation_lut_dirty` |
| state controller 改变 | 标记州内所有省份 dirty |
| 战争结束和平会议 | 刷新 owner/controller、占领、外交边界 |
| 傀儡/吞并/释放国家 | 刷新政治图、国家标签、外交边界 |

验收：

| 场景 | 预期 |
|---|---|
| 德国战争中占领法国省份 | 政治图省份底色立刻变成德国颜色 |
| 和平会议吞并州 | owner 和 controller 都改为德国，弱占领提示消失 |
| 傀儡国存在 | 傀儡本土仍显示傀儡国家颜色，不显示主国颜色，除非主国军事占领 |
| 省份被夺回 | 颜色下一帧或下个 tick 恢复 controller 颜色 |


## 9. 国策与决议重构

### 9.1 新 V7 effect

新增脚本 effect：

```rust
AddBuildingLevel { state_selector, building_id, level }
AddResourceDiscovery { state_id, good_id, discovered_level }
AddProductionMethodUnlock { pm_id }
AddGovernmentOrder { equipment_category, daily_budget_rm, duration_days }
AddTradeAgreement { partner, good_id, daily_quantity }
ChangeLaw { category, law_id, lock_days }
AddMefoCapacity { amount_rm }
AddPrivateInvestmentPool { amount_rm }
AddConstructionCapacity { amount }
AddResearchSlot { amount }
AddMilitarySpendingShare { delta }
```

### 9.2 德国国策方向

| 国策 | V7 效果 |
|---|---|
| 四年计划 | 合成资源、化工、铝、军工订单、MEFO 容量 |
| 莱茵兰 | 战争支持、军费上限、MEFO 启动 |
| 高速公路 | 铁路、建造部门、就业、短期 GDP |
| 陆军扩张 | 步兵装备、火炮、卡车订单 |
| 空军扩张 | 航空组件、飞机厂、铝需求 |
| 海军扩张 | 船坞、钢、燃油、长期订单 |
| 合成橡胶 | 合成炼油建筑和 PM |

### 9.3 决议方向

| 决议 | V7 效果 |
|---|---|
| 资源勘探 | 提升 `discovered_level` |
| 战时债券 | 增加现金，增加债务，影响信用 |
| 军工补贴 | 军工建筑盈利、产量、财政支出上升 |
| 进口优先级 | 外汇优先购买特定资源 |
| 配给制 | 降低民用需求，降低满意度，军工输入优先 |
| 征用铁路 | 军事补给上升，民用运输下降 |
| 工业疏散 | 建筑迁移/复制，短期产出下降 |


## 10. 实施阶段

### H0 — 历史数据 schema 与 loader

| 子任务 | 内容 | 验收 |
|---|---|---|
| ✅ H0.a | 新增 `history_1936/countries/*.ron` schema | 8 个主要国家可加载 |
| ✅ H0.b | 新增 `state_deposits.ron` schema | 州级资源可读取 |
| ✅ H0.c | 新增历史贸易 schema | 初始贸易路线可读取 |
| ✅ H0.d | 新增历史军事 profile schema | 军费和初始军队需求可读取 |

### H1 — 资源建筑限制

| 子任务 | 内容 | 验收 |
|---|---|---|
| ✅ H1.a | `BuildingDef.state_limit_kind` 真实接入 | 资源建筑能读所需资源 |
| ✅ H1.b | `construction_tick.rs` 增加资源校验 | 无资源州不能建矿 |
| H1.c | UI 显示不可建原因 | 玩家能看到具体原因 |
| ✅ H1.d | AI 建造走同一校验 | AI 不能绕过限制 |

### H2 — 初始建筑生成器

| 子任务 | 内容 | 验收 |
|---|---|---|
| ✅ H2.a | 替代 GER/SOV 硬编码建筑注入 | GER/SOV 由历史 profile 生成 |
| ✅ H2.b | 替代其他国家 baseline 注入 | 不再只给一个州塞建筑 |
| ✅ H2.c | GDP 反推建筑等级 | 8 国建筑规模符合目标 |
| ✅ H2.d | 州分布权重 | 工业集中区符合 1936 常识 |

### H3 — POP 与就业校准

| 子任务 | 内容 | 验收 |
|---|---|---|
| ✅ H3.a | 国家 POP 自动生成 | 8 国均有 POP |
| ✅ H3.b | 建筑就业校验 | 建筑岗位不超过可用人口 |
| ✅ H3.c | 失业率接入 profile | 美国大萧条、德国再武装就业差异可见 |
| ✅ H3.d | 动员抽人 | 扩军会减少民用劳动力 |

### H4 — GDP 校准与财政

| 子任务 | 内容 | 验收 |
|---|---|---|
| ✅ H4.a | 初始 GDP 由建筑校准 | `Treasury.gdp_gbp` 不再孤立硬编码 |
| ✅ H4.b | 建筑增加值 GDP | 输入/输出不重复计入 |
| ✅ H4.c | 政府和军工采购计入 GDP | 再武装拉动 GDP 但增加财政风险 |
| ✅ H4.d | 30/90 天回放 | 主要国家 GDP 在误差门槛内 |

### H5 — 贸易与外汇

| 子任务 | 内容 | 验收 |
|---|---|---|
| ✅ H5.a | 初始历史贸易路线 | 德日英关键资源依赖存在 |
| H5.b | 外汇优先级 | 外汇不足时进口失败原因可见 |
| ✅ H5.c | 封锁影响 | 封锁能切断进口并影响建筑/军工 |
| ✅ H5.d | 战略资源价值 | 罗马尼亚油、瑞典铁、马来亚橡胶成为战略目标 |

### H6 — 法律、国策、决议

| 子任务 | 内容 | 验收 |
|---|---|---|
| ✅ H6.a | 各国初始法律 profile | 8 国法律符合 1936 制度 |
| ✅ H6.b | 法律接入系统规则 | 法律切换改变工资、贸易、动员、所有制 |
| ✅ H6.c | 国策 V7 effect | 不再加旧工厂 |
| ✅ H6.d | 决议 V7 effect | 资源勘探、战时债券、进口优先级可用 |

### H7 — 军事经济闭环

| 子任务 | 内容 | 验收 |
|---|---|---|
| ✅ H7.a | 军事系统输出 `MilitaryDemand` | 经济能读取装备/燃油/维护需求 |
| ✅ H7.b | 政府采购订单 | 军工建筑因订单生产和盈利 |
| H7.c | 军费预算分项 | 财政 UI 显示工资、采购、维护、燃油 |
| ✅ H7.d | 动员伤经济 | 征兵导致部分建筑就业下降 |

### H7.5 — 军事系统重置

| 子任务 | 内容 | 验收 |
|---|---|---|
| ✅ H7.5.a | 新增可编辑师模板 schema | 模板可表达 5×5 战斗营和 5 个支援连槽 |
| H7.5.b | 模板编辑 UI | 可复制、改名、添加/删除营、添加/删除支援连、保存 |
| ✅ H7.5.c | 模板预览 | 实时显示战斗宽度、属性、人力、装备、训练时间、库存满足率 |
| ✅ H7.5.d | 训练队列 | 新师不再瞬间生成，按人力、装备、训练时间推进 |
| ✅ H7.5.e | 现役师切换模板 | 会产生装备缺口、组织度惩罚和补员需求 |

### H7.6 — 占领区、驻军与傀儡国

| 子任务 | 内容 | 验收 |
|---|---|---|
| ✅ H7.6.a | 占领区 state schema | owner/controller/occupier/policy/garrison/resistance/compliance 分离 |
| ✅ H7.6.b | 占领政策 | 温和占领、民政管理、军事管制、严酷镇压、掠夺经济可切换 |
| ✅ H7.6.c | 驻军模板 | 占领区镇压由模板 suppression、装备和人力决定 |
| ✅ H7.6.d | 抵抗破坏 | 镇压不足会破坏铁路、建筑、资源并消耗驻军装备 |
| ✅ H7.6.e | 傀儡经济接入 | 自治度影响资源、工业、贸易、军事控制，不再使用 0 工业占位 |
| ✅ H7.6.f | 和平会议 | 外交面板显示进行中战争、双方、战争分数、和平条款，并可执行攻方/守方胜利结算 |

### H7.7 — 占领地图颜色

| 子任务 | 内容 | 验收 |
|---|---|---|
| ✅ H7.7.a | 政治图按 controller 染色 | 战争占领地显示占领国完整颜色 |
| ✅ H7.7.b | 意识形态图按 controller 染色 | 政治图和意识形态图表达一致 |
| ✅ H7.7.c | 弱化占领条纹 | 政治图不再被红色条纹覆盖，只保留弱提示 |
| H7.7.d | 地图 dirty refresh | controller 变化后下一帧或下个 tick 刷新 LUT |
| ✅ H7.7.e | Cores/Resistance 保留法理信息 | 玩家仍能区分占领、核心、吞并、傀儡 |

### H8 — 全局回放与平衡

| 子任务 | 内容 | 验收 |
|---|---|---|
| ✅ H8.a | 1936-1937 和平回放 | GDP、贸易、财政不爆炸 |
| ✅ H8.b | 德国再武装回放 | MEFO、军工订单、资源短缺合理 |
| ✅ H8.c | 日本资源压力回放 | 油/橡胶进口被切断后军工受影响 |
| ✅ H8.d | 英国封锁回放 | 海运受损影响食物、油、橡胶和满意度 |


## 11. 测试计划

新增测试文件：

```text
crates/hoi4-content/tests/history_1936_loads.rs
crates/hoi4-logic/tests/v7_resource_building_limits.rs
crates/hoi4-logic/tests/v7_initial_gdp_calibration.rs
crates/hoi4-logic/tests/v7_initial_buildings_1936.rs
crates/hoi4-logic/tests/v7_trade_dependency_1936.rs
crates/hoi4-logic/tests/v7_pop_employment_calibration.rs
crates/hoi4-logic/tests/v7_military_procurement.rs
crates/hoi4-logic/tests/v7_template_editor.rs
crates/hoi4-logic/tests/v7_training_queue.rs
crates/hoi4-logic/tests/v7_occupation_garrison.rs
crates/hoi4-logic/tests/v7_puppet_autonomy_economy.rs
crates/hoi4-render/tests/v7_controller_color_lut.rs
```

关键断言：

| 测试 | 断言 |
|---|---|
| `resource_building_requires_deposit` | 无煤州建 `coal_mine` 失败 |
| `resource_building_caps_at_discovered_level` | `discovered_level = 4` 时最多 4 级 |
| `gdp_order_matches_1936_targets` | USA > GER≈SOV > ENG > FRA > JAP > ITA |
| `china_has_high_total_gdp_low_industrial_capacity` | 中国总 GDP 不低，但工业建筑和军工很低 |
| `germany_lacks_oil_and_rubber_without_trade` | 德国无进口时油/橡胶短缺 |
| `japan_blockade_crashes_oil_supply` | 日本封锁后油供应下降并影响军工 |
| `mobilization_reduces_worker_employment` | 征兵会降低民用建筑就业 |
| `military_procurement_creates_orders` | 装备缺口会生成政府订单并支撑军工生产 |
| `template_editor_recomputes_stats_and_equipment` | 模板增减营后战斗宽度、人员、装备需求实时变化 |
| `training_queue_blocks_without_manpower_or_equipment` | 缺人力/装备时训练队列不能满强度部署 |
| `switching_template_creates_equipment_deficit` | 现役师切换模板后产生装备缺口和组织度惩罚 |
| `occupied_state_uses_controller_not_owner_for_output` | 占领州产出归属按 controller/occupation policy 计算 |
| `garrison_suppression_reduces_resistance_growth` | 驻军镇压足够时抵抗不上升或下降 |
| `resistance_sabotage_damages_buildings_and_rails` | 抵抗过高会破坏建筑、铁路或资源产出 |
| `puppet_autonomy_uses_real_industry` | 傀儡自治度不再使用 0 工业占位 |
| `political_lut_uses_controller_color` | 战争占领省份政治图颜色等于 controller 国家颜色 |
| `occupation_overlay_does_not_override_controller_color` | 占领条纹不能覆盖占领国底色 |

### 11.1 2026-05-23 验证记录

已执行并通过：

```text
cargo test -p hoi4-content --test history_1936_loads
cargo test -p hoi4-content h2_
cargo test -p hoi4-content h3_
cargo test -p hoi4-content resource_buildings_declare_required_deposit_kind
cargo test -p hoi4-content --test ger_tree_test --test ger_decisions_test
cargo test -p hoi4-logic resource_building_
cargo test -p hoi4-logic i17_conscription_law_changes_soldier_ratio_and_recruits_same_tick
cargo test -p hoi4-logic --test v7_initial_gdp_calibration
cargo test -p hoi4-logic --test v7_global_replay_balance
cargo test -p hoi4-logic --test v7_military_procurement --test v7_template_editor --test v7_training_queue
cargo test -p hoi4-logic --test v7_occupation_garrison --test v7_puppet_autonomy_economy
cargo test -p hoi4-render political_lut_uses_controller_color
```

未打勾项原因：
- H1.c：未确认建造 UI 已显示资源 deposit / 科技 / 上限等具体不可建原因。
- H5.b：未确认外汇不足时的进口失败原因 UI 或验收测试。
- H7.c：逻辑层已有工资、采购、维护、训练分项，但财政 UI 未确认完整显示燃油/训练分项。
- H7.5.b：未确认模板编辑 UI 的复制、改名、增删营/支援连、保存全流程。
- H7.7.d：未确认 controller 变化后地图 dirty refresh 的独立验收。


## 12. 文件改动清单

### 12.1 新增内容文件

```text
crates/hoi4-content/content/history_1936/countries/GER.ron
crates/hoi4-content/content/history_1936/countries/USA.ron
crates/hoi4-content/content/history_1936/countries/SOV.ron
crates/hoi4-content/content/history_1936/countries/ENG.ron
crates/hoi4-content/content/history_1936/countries/FRA.ron
crates/hoi4-content/content/history_1936/countries/JAP.ron
crates/hoi4-content/content/history_1936/countries/ITA.ron
crates/hoi4-content/content/history_1936/countries/CHI.ron
crates/hoi4-content/content/history_1936/resources/state_deposits.ron
crates/hoi4-content/content/history_1936/trade/initial_trade_1936.ron
crates/hoi4-content/content/history_1936/laws/initial_laws_1936.ron
crates/hoi4-content/content/history_1936/military/force_profiles_1936.ron
crates/hoi4-content/content/history_1936/military/division_templates_1936.ron
crates/hoi4-content/content/history_1936/military/training_profiles_1936.ron
crates/hoi4-content/content/history_1936/occupation/occupation_policies.ron
crates/hoi4-content/content/history_1936/puppets/initial_autonomy_1936.ron
```

### 12.2 主要代码改动

```text
crates/hoi4-content/src/v6_loader.rs
crates/hoi4-content/src/v7_history_loader.rs
crates/hoi4-state/src/store.rs
crates/hoi4-state/src/buildings_v6.rs
crates/hoi4-state/src/market.rs
crates/hoi4-state/src/trade.rs
crates/hoi4-state/src/occupation.rs
crates/hoi4-state/src/training.rs
crates/hoi4-state/src/templates.rs
crates/hoi4-logic/src/economy/construction_tick.rs
crates/hoi4-logic/src/economy/market_tick.rs
crates/hoi4-logic/src/economy/planned_tick.rs
crates/hoi4-logic/src/economy/finance_tick.rs
crates/hoi4-logic/src/trade/mod.rs
crates/hoi4-logic/src/military/mod.rs
crates/hoi4-logic/src/military/templates.rs
crates/hoi4-logic/src/military/training.rs
crates/hoi4-logic/src/military/reinforcement.rs
crates/hoi4-logic/src/occupation/mod.rs
crates/hoi4-logic/src/occupation/garrison.rs
crates/hoi4-logic/src/occupation/resistance.rs
crates/hoi4-logic/src/diplomacy/puppet.rs
crates/hoi4-logic/src/diplomacy/peace.rs
crates/hoi4-render/src/map_mode.rs
crates/hoi4-app/src/main.rs
crates/hoi4-script/src/effects.rs
crates/hoi4-script/src/decisions.rs
crates/hoi4-ui/src/construction_v6_panel.rs
crates/hoi4-ui/src/province_info.rs
crates/hoi4-ui/src/trade_panel.rs
crates/hoi4-ui/src/finance_panel.rs
crates/hoi4-ui/src/law_panel.rs
crates/hoi4-ui/src/military.rs
crates/hoi4-ui/src/template_editor.rs
crates/hoi4-ui/src/training_panel.rs
crates/hoi4-ui/src/occupation_panel.rs
crates/hoi4-ui/src/puppet_panel.rs
```


## 13. 验收标准

### 13.1 资源
- 无资源州不能建设资源建筑。
- 资源建筑不能超过州的 `discovered_level`。
- 勘探、科技、国策可以提高 `discovered_level`，但不能超过 `potential_level`。
- AI 和玩家使用同一套校验。

### 13.2 GDP
- USA GDP 最高。
- GER 与 SOV 接近，但结构不同：GER 工业效率高、资源短缺；SOV 建筑和人口大、效率和消费差。
- CHI 总 GDP 不应过低，但工业 GDP、军工能力、财政动员能力必须弱。
- 主要国家 30/90 天回放 GDP 在误差门槛内。

### 13.3 建筑
- 德国重工业集中在鲁尔、西里西亚、萨克森、柏林。
- 美国资源和工业显著强于欧洲单国。
- 英国金融、港口、造船强，本土油橡胶弱。
- 日本造船军工强，但油、铁、橡胶脆弱。
- 苏联总量大，消费品和效率问题明显。
- 中国农业巨大，工业少，基建弱。

### 13.4 贸易
- 德国缺油缺橡胶会影响军工和燃油。
- 日本被封锁后油和橡胶迅速短缺。
- 英国海运受损会影响食物、油、橡胶和满意度。
- 罗马尼亚石油、瑞典铁矿、马来亚橡胶具有可见战略价值。

### 13.5 法律与军事
- 征兵会抽走 POP 并降低部分民用产出。
- 军费可拆成工资、采购、维护、燃油、训练。
- 军工订单会支撑军工建筑盈利和装备库存增长。
- MEFO 能短期支撑德国军工，但长期形成财政/信用/事件风险。


## 14. 拒收清单

- ❌ 只改顶栏 GDP 数字但不改建筑和 POP。
- ❌ 资源建筑仍可在无资源州建设。
- ❌ AI 能绕过资源建筑限制。
- ❌ 用旧 HOI4 `industrial_complex / arms_factory / dockyard` 作为经济主模型。
- ❌ 国策继续加旧工厂而不是 V6/V7 建筑。
- ❌ 军费继续硬编码为师数量乘固定值。
- ❌ 各国初始建筑继续由州名字符串启发式生成。
- ❌ 没有 30/90 天 GDP 回放验收就标完成。


## 15. 最终目标

V7 完成后，1936 开局应满足：

```text
历史 GDP 目标
-> 合理部门结构
-> 合理州资源和资源建筑
-> 合理初始工业、农业、服务、军工、基建
-> 合理 POP 和就业
-> 合理贸易依赖和外汇压力
-> 合理法律与国家制度差异
-> 合理军费、装备、动员和财政风险
-> 运行期 GDP 可解释变化
```

这条链路完成后，Project Ironheart 的经济系统才真正从“HOI4 工厂槽位的替代品”变成“1936 世界经济与战争动员模拟”。
