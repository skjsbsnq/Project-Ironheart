# Project Ironheart V7 — 1936 产业链·汽车工业·市场面板·POP 强化·POP 面板改进路线图

> **本文档参考 [`ROADMAP_V6_ECONOMY.md`](./ROADMAP_V6_ECONOMY.md) 的结构编写。**
> 写于 2026-05-23。目标是把当前 V6/V7 经济从“可运行的建筑市场雏形”推进到“能表达 1936 年战争经济、工业分工、汽车工业、劳动力结构和社会反馈”的系统。
> 本路线图不替代 [`ROADMAP_V7_HISTORICAL_ECONOMY_RECONSTRUCTION.md`](./ROADMAP_V7_HISTORICAL_ECONOMY_RECONSTRUCTION.md)，而是对其中 **产业链、建筑、市场面板、POP、POP UI** 五个方向的细化落地方案。


## 0. 使命

当前 V6 已经有建筑、市场、财政、法律、POP 和军工库存的骨架，但产业链仍过度简化，POP 仍只是少量经济变量容器，UI 也缺少人口专属面板。

本路线图的使命是：

- 把 1936 工业链从“钢铁厂 + 机械厂 + 兵工厂”的简化结构，扩展为 **原料 -> 基础材料 -> 工业中间品 -> 军工半成品 -> 最终装备 -> 运行维护消耗** 的闭环。
- 补齐 1936 关键建筑，尤其是 **汽车厂、发动机厂、机床厂、炼油厂、橡胶制品厂、光学厂、无线电厂、装甲板厂、火炮厂**。
- 强化 POP 系统，让人口不只是人力池，而是劳动力、消费需求、税基、社会稳定、意识形态和动员代价的核心。
- 重构市场面板，让玩家能追踪每种商品的供给来源、消费来源、POP 消费、军工订单、建造需求、贸易、库存、短缺和价格传导。
- 新增 POP 专属面板，让玩家能直接看到人口结构、就业、工资、生活需求、满意度、忠诚、激进化和动员压力。


## 1. 范围

### 1.1 范围内

- ✅ 重构 `content/economy_v6/goods.ron` 商品层级。
- ✅ 重构 `content/economy_v6/buildings/buildings.ron` 建筑目录。
- ✅ 重构 `content/economy_v6/production_methods/*.ron` 生产方式链。
- ✅ 新增汽车工业链：`engine_plant`、`vehicle_factory`、`rubber_factory`、`oil_refinery`、`machine_tool_works`。
- ✅ 将 `motorized` 和 `mechanized` 从 `arms_industry` 迁出，转由 `vehicle_factory` 生产。
- ✅ 将 `oil` 与 `fuel` 分离，油田产原油，炼油厂产燃油。
- ✅ 将维护消耗从财政数字扩展为库存与消耗品需求。
- ✅ 扩展 POP schema：生活水平、需求满足率、激进化、识字率/资质等。
- ✅ 新增 POP 专属 UI 面板。
- ✅ 重构市场 UI 面板，支持产业链视图、瓶颈追踪、POP 消费拆分、交易成交解释。
- ✅ 建立 POP 与就业、工资、消费、税负、满意度、忠诚、动员之间的可解释链路。
- ✅ 为主要国家生成可用 POP，不再只有德国有明确 POP。
- ✅ 加测试和 1936 回放验收。

### 1.2 范围外

- ❌ 不做 Victoria 3 级别完整文化/宗教/全球移民模拟。
- ❌ 不做每个工厂品牌或企业级别模拟，例如不单独建 Opel、Ford、Fiat、GAZ、Toyota。
- ❌ 不做完整世界贸易公司与殖民地财政系统。
- ❌ 不保证旧中间存档兼容。
- ❌ 不恢复 HOI4 vanilla civ/mil/dock 三工厂模型。


## 2. 关键决策点

| # | 决策点 | 选定方案 |
|---|---|---|
| I1 | 产业链层级 | 采用 6 层：原料、基础材料、工业中间品、军工半成品、最终装备、运行消耗 |
| I2 | 汽车工业 | `vehicle_factory` 独立存在，生产 `motorized / mechanized / vehicle_parts`；不再由 `arms_industry` 生产卡车 |
| I3 | 发动机 | `engine_plant` 独立存在，服务汽车、坦克、飞机、舰船、铁路车辆 |
| I4 | 机床 | `machine_tool_works` 是高级工业和军工扩张瓶颈 |
| I5 | 石油 | `oil` 是原油，`fuel` 是炼油产物；军队和市场消耗燃油而非直接消耗原油 |
| I6 | 橡胶 | `rubber / synthetic_rubber -> rubber_parts`，车辆、飞机、维护消耗橡胶制品 |
| I7 | 支援装备 | `support_equipment` 由无线电、机械、橡胶制品、纺织品等组成，不再主要吃轻武器 |
| I8 | POP 强化 | 保留 6 阶级起步，但新增生活水平、需求满足率、激进化、识字率/资质字段 |
| I9 | POP 面板 | 必须新增独立面板，不塞进财政或政治面板 |
| I10 | 市场面板 | 必须从商品列表升级为“供需解释 + 产业链瓶颈 + POP 消费 + 贸易成交”面板 |
| I11 | 动员 | 扩军必须从 POP 抽人，并通过劳动力短缺影响产出 |
| I12 | 维护 | 维护必须形成库存/商品需求，不允许只扣财政抽象费用 |
| I13 | 验收 | 每阶段必须有测试或 1936 回放验收 |


## 3. 当前栈审计

### 3.1 当前建筑目录

建筑定义：

```text
crates/hoi4-content/content/economy_v6/buildings/buildings.ron
```

已有关键工业建筑：

```text
steel_mill
aluminium_plant
machinery_workshop
chemical_plant
arms_industry
munition_plant
shipyard
synthetic_refinery
```

明显缺失建筑：

```text
vehicle_factory       汽车厂
engine_plant          发动机厂
machine_tool_works    机床厂
oil_refinery          炼油厂
rubber_factory        橡胶制品厂
optics_works          光学仪器厂
radio_factory         无线电厂
ball_bearing_plant    轴承厂
armor_plate_works     装甲板厂
gun_factory           火炮/炮管厂
explosives_plant      炸药厂
rail_works            铁路车辆厂
ship_engine_works     船用发动机厂
cement_works          水泥厂
glass_works           玻璃厂
```

### 3.2 当前汽车工业问题

当前 `motorized` 在 `military.ron` 中由 `arms_industry` 生产：

```text
arms_industry_motorized_mk2
building_id: arms_industry
input: engines + steel + machinery
output: motorized
```

问题：

- 卡车、军车、牵引车不应由兵工厂主产。
- 汽车厂缺失导致美国、德国、日本、苏联的摩托化能力无法体现。
- 民用汽车工业无法战时转产。
- 橡胶、燃油、发动机、车辆零部件对机动化的约束不明显。

### 3.3 当前 POP 系统

POP schema：

```text
crates/hoi4-state/src/pops.rs
```

现有阶级：

```text
Peasant
Worker
Clerk
Capitalist
Aristocrat
Soldier
```

现有字段：

```text
class
state
size
employed_at
wage_rm
satisfaction_law_modifier
loyalty_coefficient
loyalty_decay_mult
satisfaction
political_loyalty
```

已有机制：

- 建筑雇佣 POP。
- POP 工资参与税收。
- POP 消费形成市场需求。
- 满意度受消费、失业、税负、法律影响。
- 忠诚度受满意度影响。
- Soldier POP 派生可用人力。

工资现状：

```text
market_tick.rs::step_pop_wage
planned_tick.rs::step_pop_wage
```

- 按阶级给基础日工资。
- Worker / Capitalist 会受经济法律工资系数影响。
- 已就业 POP 才有工资，失业 POP 工资为 0。
- 私营/卡特尔建筑无利润时会把对应工资因子压到 0。
- 资本家会从卡特尔利润分成中额外获得收入。

消费现状：

```text
market_tick.rs::step_pop_consumption
planned_tick.rs::step_rationing
```

- 市场经济中，POP 按阶级和就业人口向全国市场写入消费需求。
- 计划经济中，POP 需求进入配给逻辑，按计划供给计算 ration rate。
- 当前消费只写入 `market.demand`，没有给每个 POPGroup 记录“买到了多少”。
- 当前消费没有真实扣 POP 现金/财富，也没有明确生活水平。
- 当前需求满足率只在满意度公式里通过全局基础商品供需近似计算。

缺失：

- 无生活水平。
- 无需求满足率明细。
- 无激进化。
- 无识字率/资质。
- 无文化/民族。
- 无 POP 专属面板。
- 无跨阶级晋升/降级的完整实现。
- 无迁移。
- 无战争伤亡对家庭和社会的反馈。

### 3.4 当前市场面板

市场面板：

```text
crates/hoi4-ui/src/market_panel.rs
```

现有能力：

- 显示商品名称、分类、价格、基础价格。
- 显示供给、需求、库存、库存覆盖天数、未满足需求。
- 显示生产者、消费者、政府订单、建造需求、进口、出口、封锁、受影响建筑。
- 支持搜索和短缺优先。

核心不足：

- 只有商品列表，没有总览仪表盘。
- 不区分需求来源占比，例如 POP 消费、建筑投入、军工订单、建造、贸易出口。
- 不区分供给来源占比，例如国内建筑、库存释放、进口、计划配给。
- 不显示价格变动原因拆解。
- 不显示产业链上下游图。
- 不显示“缺这个商品会导致哪些装备/建筑停产”。
- 不显示 POP 生活需求受哪些商品短缺影响。
- 不显示成交量与未成交量，容易误以为“有需求就已经付款”。
- 不显示历史趋势，玩家看不出短缺是在恶化还是缓解。
- 商品多起来后，纯折叠列表会非常难用。


## 4. 目标产业模型

### 4.1 商品层级

目标商品分 6 层。

#### 原料 RawMaterial

```text
coal
iron_ore
bauxite
rubber
chromium
copper
sulfur
cotton
```

#### 基础材料 IndustrialMaterial

```text
steel
aluminium
fuel
chemicals
explosives
textiles
lumber
rubber_parts
```

#### 工业中间品 Intermediate

```text
machine_tools
machinery
engines
electrical_goods
optics
radio_sets
vehicle_parts
armor_plate
ball_bearings
```

#### 军工半成品 MilitaryIntermediate

```text
small_arms_parts
ammunition
artillery_shells
ship_sections
naval_guns
torpedoes
```

#### 最终装备 Equipment

仍使用现有 12 类整合军备：

```text
infantry_equipment
support_equipment
artillery
anti_tank
anti_air
motorized
mechanized
armor
aircraft
naval_vessel
convoy
train
```

#### 运行消耗 OperationalConsumption

```text
fuel
ammunition
spare_parts
medical_supplies
rubber_parts
```

### 4.2 建筑链

#### 资源建筑

```text
coal_mine -> coal
iron_mine -> iron_ore
bauxite_mine -> bauxite
rubber_plantation -> rubber
copper_mine -> copper
sulfur_mine -> sulfur
logging_camp -> timber
cotton_farm -> cotton
```

#### 基础工业

```text
steel_mill: iron_ore + coal -> steel
aluminium_plant: bauxite + coal/electricity -> aluminium
synthetic_refinery: coal + chemicals -> synthetic_rubber + synthetic_oil/fuel
chemical_plant: coal/oil + sulfur -> chemicals + explosives
textile_mill: cotton -> textiles
rubber_factory: rubber/synthetic_rubber -> rubber_parts
```

#### 工业中间品

```text
machine_tool_works: steel + machinery -> machine_tools
machinery_workshop: steel + machine_tools -> machinery
engine_plant: steel + machinery + rubber_parts + machine_tools -> engines
electrical_works: copper + steel + machinery -> electrical_goods
optics_works: glass/chemicals + machinery -> optics
radio_factory: copper + electrical_goods + machine_tools -> radio_sets
armor_plate_works: steel + chromium/tungsten + machinery -> armor_plate
```

#### 军工

```text
arms_industry: small_arms_parts + ammunition + steel + textiles -> infantry_equipment
arms_industry: radio_sets + machinery + rubber_parts + textiles -> support_equipment
munition_plant: steel + explosives -> ammunition
munition_plant: steel + explosives + chemicals -> artillery_shells
artillery_factory: gun_barrels + artillery_shells + machinery + steel -> artillery / anti_tank / anti_air
vehicle_factory: engines + steel + rubber_parts + vehicle_parts -> motorized
vehicle_factory: engines + armor_plate + machinery + rubber_parts -> mechanized
tank_factory: tank_hulls + engines + armor_plate + gun_barrels + optics + radio_sets -> armor
aircraft_factory: airframes + engines + aluminium + rubber_parts + radio_sets -> aircraft
shipyard: ship_sections + engines + steel + machinery + naval_guns -> naval_vessel
shipyard: ship_sections + engines + steel -> convoy
rail_works: steel + machinery + engines -> train
```


## 5. 目标 POP 模型

### 5.1 第一阶段 schema 扩展

在不大规模破坏现有逻辑的前提下，`PopGroup` 增加：

```rust
pub literacy: f32,
pub skilled_ratio: f32,
pub standard_of_living: f32,
pub needs_fulfillment: f32,
pub essential_needs_fulfillment: f32,
pub luxury_needs_fulfillment: f32,
pub radicalism: f32,
```

### 5.2 第二阶段 schema 扩展

进一步增加：

```rust
pub culture: String,
pub ideology_support: [f32; 4],
pub mobilized: u32,
pub dependents: u32,
```

### 5.3 POP 需求

需求分层：

```text
Essential: grain, clothes, fuel/coal, housing
Normal: meat, furniture, transport, liquor, tobacco
Luxury: luxury_goods, banking, telegraph, radios, automobiles
```

满意度公式目标：

```text
satisfaction_target =
    0.35 * essential_needs_fulfillment
  + 0.20 * standard_of_living
  + 0.15 * employment_security
  + 0.15 * (1.0 - tax_burden)
  + 0.10 * law_modifier
  - 0.10 * war_weariness
```

### 5.4 激进化

激进化来源：

```text
essential_needs_fulfillment < 0.8
unemployment_rate > 0.1
tax_burden > 0.5
war_weariness high
casualties high
occupation/non-core state
law ideology mismatch
```

激进化效果：

```text
stability 下降
strike_risk 上升
draft_resistance 上升
party_popularity 偏移
resistance 上升
```

### 5.5 资质与工业

高级工业建筑需要资质：

```text
machine_tool_works: skilled workers + clerks/engineers
engine_plant: skilled workers + engineers
radio_factory: clerks/engineers + literacy
optics_works: skilled workers + engineers
aircraft_factory: engineers + skilled workers
tank_factory: engineers + skilled workers
```

第一阶段不新增 `Engineer` 阶级，使用 `skilled_ratio` 和 `literacy` 近似。


## 6. POP 专属面板

### 6.1 新文件

```text
crates/hoi4-ui/src/pop_panel.rs
```

注册：

```text
crates/hoi4-ui/src/lib.rs
crates/hoi4-app/src/main.rs
```

`InGamePanel` 新增：

```rust
Pops
```

快捷键建议：

```text
   F9
```

### 6.2 数据结构

```rust
pub struct PopPanelData {
    pub total_population: u64,
    pub workforce: u64,
    pub employed: u64,
    pub unemployed: u64,
    pub unemployment_rate: f32,
    pub average_wage_rm: f32,
    pub average_satisfaction: f32,
    pub average_loyalty: f32,
    pub radicalism: f32,
    pub soldier_pool: u64,
    pub classes: Vec<PopClassEntry>,
    pub states: Vec<PopStateEntry>,
    pub needs: Vec<PopNeedEntry>,
    pub alerts: Vec<String>,
}
```

```rust
pub struct PopClassEntry {
    pub class_name: String,
    pub size: u64,
    pub employed: u64,
    pub unemployed: u64,
    pub avg_wage_rm: f32,
    pub avg_tax_burden: f32,
    pub avg_satisfaction: f32,
    pub avg_loyalty: f32,
    pub radicalism: f32,
}
```

```rust
pub struct PopStateEntry {
    pub state_name: String,
    pub population: u64,
    pub employed: u64,
    pub unemployment_rate: f32,
    pub avg_satisfaction: f32,
    pub avg_wage_rm: f32,
    pub dominant_class: String,
}
```

### 6.3 标签页

```text
总览
阶级
州分布
生活需求
政治
```

总览显示：

```text
总人口、劳动力、就业、失业率、平均工资、平均满意度、忠诚、激进化、士兵池
```

阶级显示：

```text
各阶级人口、就业率、工资、税负、满意度、忠诚、激进化
```

州分布显示：

```text
州人口、就业率、平均工资、满意度、主导阶级、迁移压力
```

生活需求显示：

```text
基础需求、普通需求、奢侈需求、短缺商品、价格压力
```

政治显示：

```text
忠诚来源、激进化来源、法律影响、税负影响、战争厌倦、罢工/征兵抵抗风险
```


## 7. 市场面板重构

### 7.1 目标

市场面板必须从“商品清单”升级成“经济诊断工具”。玩家打开市场面板时，应能回答：

```text
什么商品短缺？
为什么短缺？
谁在生产？谁在消耗？
哪些 POP 生活需求受影响？
哪些建筑/装备生产会停？
需要建什么厂、进口什么、换什么生产方式？
政府订单有没有真实成交？有没有只是挂单？
价格为什么涨？库存还能撑多久？
短缺是在恶化还是缓解？
```

### 7.2 新数据结构

扩展 `MarketPanelData`：

```rust
pub struct MarketPanelData {
    pub goods: Vec<GoodEntry>,
    pub exchange_rate: f32,
    pub cash_rm: f64,
    pub total_shortage_value_rm: f64,
    pub total_import_value_gbp: f64,
    pub total_export_value_gbp: f64,
    pub pop_needs_fulfillment: f32,
    pub military_supply_pressure: f32,
    pub alerts: Vec<MarketAlertEntry>,
}
```

扩展 `GoodEntry`：

```rust
pub struct GoodEntry {
    pub id: String,
    pub name: String,
    pub category: GoodCategory,
    pub price: f32,
    pub base_price: f32,
    pub price_change_7d: f32,
    pub supply: f32,
    pub demand: f32,
    pub traded: f32,
    pub unmet_demand: f32,
    pub stockpile: f32,
    pub stockpile_coverage_days: f32,
    pub domestic_production: f32,
    pub imports: f32,
    pub stockpile_draw: f32,
    pub building_input_demand: f32,
    pub pop_consumption_demand: f32,
    pub military_order_demand: f32,
    pub construction_demand: f32,
    pub export_demand: f32,
    pub producers: Vec<GoodFlowSource>,
    pub consumers: Vec<GoodFlowSource>,
    pub affected_buildings: Vec<GoodFlowSource>,
    pub affected_pop_classes: Vec<GoodFlowSource>,
    pub upstream_goods: Vec<String>,
    pub downstream_goods: Vec<String>,
}
```

新增告警：

```rust
pub struct MarketAlertEntry {
    pub severity: MarketAlertSeverity,
    pub good_id: String,
    pub title: String,
    pub description: String,
}
```

### 7.3 标签页

市场面板改为 5 个标签页：

```text
总览
短缺
商品
产业链
贸易
```

总览：

```text
总短缺价值
POP 需求满足率
军工供应压力
进口/出口价值
外汇压力
最严重 5 个短缺商品
价格上涨最快 5 个商品
```

短缺：

```text
按短缺严重度排序
显示缺口、库存天数、受影响建筑、受影响 POP、建议行动
```

商品：

```text
可搜索、分类、排序的商品表
供给/需求/成交/缺口/库存/价格
```

产业链：

```text
选中商品后显示上游和下游
例如 motorized:
rubber -> rubber_parts -> vehicle_factory -> motorized
oil -> fuel -> army maintenance / motorized operation
steel + machine_tools + engines -> vehicle_factory
```

贸易：

```text
进口、出口、封锁、外汇、关税、贸易失败原因
显示“需求存在但未成交”的订单
```

### 7.4 商品详情

每个商品详情必须显示：

```text
价格：当前 / 基础 / 7 日变化
供给：国内生产 / 进口 / 库存释放
需求：建筑投入 / POP 消费 / 军工订单 / 建造 / 出口
成交：成交量 / 未成交量
库存：库存数量 / 覆盖天数 / 趋势
影响：哪些建筑降产，哪些装备缺口，哪些 POP 生活需求下降
建议：建厂、进口、改 PM、削减订单、释放库存
```

### 7.5 POP 消费接入

市场面板必须从 POP 系统读取或接收消费拆分：

```text
Peasant -> grain, clothes, fuel/coal
Worker -> grain, meat, clothes, furniture, transport
Clerk -> grain, meat, clothes, furniture, transport, liquor, tobacco, radios
Capitalist -> luxury_goods, banking, automobiles, radios, services
Soldier families -> grain, clothes, medical_supplies
```

短缺商品应显示受影响阶级：

```text
grain shortage -> Peasant/Worker/Clerk 基础需求下降
clothes shortage -> 基础需求下降
fuel shortage -> 交通、军队、工业成本上升
rubber_parts shortage -> motorized/aircraft/maintenance 下降
radio_sets shortage -> support_equipment/aircraft/tank 效率下降
```

### 7.6 军购成交解释

市场面板必须区分：

```text
government_order_demand: 政府想买
traded: 实际成交
unmet_demand: 未成交
paid_rm: 实际付款
```

验收标准：

- 没有供给时，市场面板显示“政府订单未成交”，财政不扣军购现金。
- 有供给时，显示实际成交量和付款。
- 玩家能看到军购需求如何传导到上游钢铁、机械、橡胶、燃油、无线电。


## 8. 阶段计划

### V7.I1 — POP 面板只读版 ✅ 已完成

目标：先让玩家看见 POP。

任务：

- 新增 `pop_panel.rs`。
- 添加 `InGamePanel::Pops`。
- 从现有 POP 计算总人口、就业、失业、工资、税负、满意度、忠诚。
- 添加阶级表和州分布表。
- 添加快捷键。

验收：

- 打开面板不崩溃。
- 德国开局能显示各阶级人口。
- 人口总数与 `initial_ger.ron` 大致一致。
- 就业变化后面板数字变化。

完成记录：

- 新增 `crates/hoi4-ui/src/pop_panel.rs`，提供只读 POP 面板。
- 在 `crates/hoi4-ui/src/lib.rs` 导出 `pop_panel`。
- 在 `crates/hoi4-app/src/main.rs` 新增 `InGamePanel::Pops`，接入 `F9` 快捷键和关闭逻辑。
- 面板数据从现有 `World + PopStore + state ownership` 聚合，不新增 POP 规则。
- 已显示总人口、劳动力、就业、失业率、平均工资、平均满意度、平均忠诚、士兵池。
- 已显示阶级表与州分布表；数据随 POP 就业/工资/满意度变化刷新。
- 验证：`cargo fmt`、`cargo check` 通过。

### V7.I2 — 汽车工业第一批建筑 ✅ 已完成

目标：补齐汽车工业最小闭环。

新增建筑：

```text
machine_tool_works
engine_plant
vehicle_factory
rubber_factory
```

新增商品：

```text
fuel
rubber_parts
machine_tools
vehicle_parts
spare_parts
```

迁移生产：

- `motorized` 从 `arms_industry` 迁到 `vehicle_factory`。
- `mechanized` 从 `arms_industry` 迁到 `vehicle_factory`。
- `engines` 由 `engine_plant` 生产。
- `oil_rig` 产 `oil`，`oil_refinery` 产 `fuel`。

验收：

- 可建建筑目录出现汽车厂、发动机厂、机床厂、炼油厂、橡胶制品厂。
- 物流中 `motorized` 产出来自汽车厂。
- 缺橡胶/燃油会影响摩托化链。

完成记录：

- 更新 `crates/hoi4-content/content/economy_v6/goods.ron`，新增 `fuel`、`rubber_parts`、`machine_tools`、`vehicle_parts`、`spare_parts`。
- 更新 `crates/hoi4-content/content/economy_v6/buildings/buildings.ron`，新增 `machine_tool_works`、`engine_plant`、`vehicle_factory`、`rubber_factory`、`oil_refinery`。
- 更新 `industrial.ron`，新增机床、发动机、橡胶制品和炼油生产方式。
- 更新 `military.ron`，将 `motorized / mechanized` 从 `arms_industry` 迁到 `vehicle_factory`；新增车辆零部件产线。
- `oil_rig` 保持产 `oil`；`oil_refinery_default` 将 `oil` 转为 `fuel + chemicals`；`synthetic_refinery_default` 改为产 `synthetic_rubber + fuel`。
- 补充测试：`vehicle_factory_outputs_motorized`、`oil_refinery_outputs_fuel`。
- 验证：I2 精确内容测试通过；`cargo check` 通过。

### V7.I3 — 1936 军工配方重构 ✅ 已完成

目标：让最终装备不再直接吃泛用钢铁。

任务：

- `infantry_equipment` 改吃 `small_arms_parts + ammunition + steel + textiles`。
- `support_equipment` 改吃 `radio_sets + machinery + rubber_parts + textiles`。
- `artillery / anti_tank / anti_air` 改吃 `gun_barrels + artillery_shells + machinery + steel/optics/electrical_goods`。
- `armor` 改吃 `tank_hulls + engines + armor_plate + gun_barrels + optics + radio_sets`。
- `aircraft` 改吃 `airframes + engines + aluminium + rubber_parts + radio_sets`。

验收：

- 没有对应中间品时最终装备产出下降。
- 军购订单能向上游传导到钢、橡胶、燃油、无线电、光学等商品。

完成记录：

- 更新 `goods.ron`，新增/启用 `textiles`、`optics`、`radio_sets`、`armor_plate`、`gun_barrels`、`small_arms_parts`、`ammunition`、`airframes`。
- 更新 `consumer_goods.ron`，让 `textile_mill_default` 产出 `clothes + textiles`。
- 更新 `industrial.ron`，新增/扩展 `small_arms_parts`、`gun_barrels`、`armor_plate`、`radio_sets`、`optics` 的来源。
- 更新 `military.ron`，将最终装备配方改为 1936 中间品链：
- `infantry_equipment`：`small_arms_parts + ammunition + steel + textiles`。
- `support_equipment`：`radio_sets + machinery + rubber_parts + textiles`。
- `artillery / anti_tank / anti_air`：`gun_barrels + artillery_shells + machinery + steel/optics/electrical_goods`。
- `armor`：`tank_hulls + engines + armor_plate + gun_barrels + optics + radio_sets`。
- `aircraft`：`airframes + engines + aluminium + rubber_parts + radio_sets`。
- `mechanized`：`engines + armor_plate + machinery + rubber_parts + vehicle_parts`。
- 为避免默认 PM 分组只激活一条副产物线导致德国开局炮弹净流量为负，`ammunition_line` 同时产出 `ammunition + artillery_shells`。
- 补充测试：`final_equipment_uses_1936_intermediate_inputs`、`military_intermediate_goods_have_sources`。
- 验证：I3 精确内容测试、`h3_ger_initial_industrial_inputs_are_self_sustaining`、`cargo check` 通过。

### V7.I4 — 市场面板诊断版 ✅ 已完成

目标：市场面板先完成“解释能力”，再承载更复杂产业链。

任务：

- 增加市场总览标签页。
- 增加短缺标签页。
- 增加商品详情中的供给来源和需求来源拆分。
- 增加 POP 消费需求来源显示。
- 增加军购订单“想买/成交/未成交/付款”显示。
- 增加受影响建筑和受影响 POP 阶级列表。

验收：

- 无供给的政府订单不会显示为已成交。
- 粮食/衣物短缺能显示受影响 POP 阶级。
- 橡胶/燃油短缺能显示受影响装备和建筑。
- 玩家能从商品详情定位上游瓶颈。

完成记录：

- 更新 `crates/hoi4-ui/src/market_panel.rs`，将市场面板改为五页：`总览`、`短缺`、`商品`、`产业链`、`贸易`。
- 扩展 `MarketPanelData`，增加总短缺价值、进口/出口价值、POP 需求满足率、军工供应压力和市场告警。
- 扩展 `GoodEntry`，增加 `traded`、`domestic_production`、`stockpile_draw`、`building_input_demand`、`pop_consumption_demand`、`military_order_demand`、`affected_pop_classes`、`upstream_goods`、`downstream_goods`、`paid_rm`。
- 更新 `crates/hoi4-app/src/main.rs` 的市场面板快照构造逻辑，从现有建筑 PM、POP 消费公式、施工队列、政府订单和市场状态现场重建诊断拆分。
- 商品详情已显示供给拆分、需求拆分、成交/未成交、库存覆盖、生产者/消费者、政府订单、受影响建筑、受影响 POP、贸易封锁和上游/下游。
- 军购页已区分“想买 / 实际成交 / 未成交 / 付款”，避免把挂单误读为已付款成交。
- 注意：当前实现不改 `market_tick` 核心逻辑，不持久化逐订单历史；价格 7 日趋势和精确 paid/unpaid 分摊留给后续细化。
- 补充测试：`market_panel_splits_pop_consumption_demand`、`market_panel_distinguishes_order_from_traded_volume`。
- 验证：`cargo test -p hoi4-ui market_panel -- --nocapture`、内容层 sanity tests、`cargo check` 通过。

### V7.I5 — POP 需求与生活水平

目标：POP 不再只有满意度，开始有生活需求。

任务：

- `PopGroup` 增加需求满足和生活水平字段。
- `step_pop_consumption` 写入需求满足率。
- `step_pop_satisfaction` 使用新公式。
- POP 面板显示基础/普通/奢侈需求满足率。

验收：

- 粮食或衣物短缺会降低基础需求满足率。
- 高税负会降低生活水平/满意度。
- 生活需求变化能在 POP 面板显示。

### V7.I6 — POP 激进化与政治压力

目标：社会问题产生政治后果。

任务：

- `PopGroup` 增加 `radicalism`。
- 满意度低、失业、高税、战争伤亡提高激进化。
- 激进化汇总影响稳定度、征兵抵抗、罢工风险。
- POP 面板政治页显示激进化来源。

验收：

- 失业和商品短缺会提高激进化。
- 高激进化降低稳定度。
- 警察国家/宣传法能压制忠诚滑落但损害满意度。

### V7.I7 — 资质、识字率与高级工业 ✅ 已完成

目标：机床、无线电、光学、飞机、坦克需要熟练人口。

任务：

- `PopGroup` 增加 `literacy`、`skilled_ratio`。
- 建筑 PM 增加资质需求或使用现有 `employment_demand` 扩展规则。
- 大学、教育、职员比例提高资质增长。
- 高级工业缺资质时产出下降。

验收：

- 低识字率国家难以高效生产无线电/光学/飞机。
- 美国、德国、英国高级工业效率高于中国等低工业化国家。
- 建大学/教育投资能中期改善资质瓶颈。

完成记录：

- 更新 `crates/hoi4-state/src/pops.rs`，为 `PopGroup` 增加 `literacy` 与 `skilled_ratio`，并为 6 阶级提供基础识字率/熟练比例。
- 更新 `crates/hoi4-content/src/v6_loader.rs`，让手写 POP 可选填写 `literacy/skilled_ratio`，历史/算法注入 POP 时按工业化、城市化和阶级生成初始资质。
- 扩展 `ProductionMethodDef`，新增 `required_literacy` 与 `required_skilled_ratio`；在 `industrial.ron`、`military.ron`、`service.ron` 为机床、发动机、无线电、光学、坦克、飞机、大学等高级 PM 标注资质需求。
- 更新 `market_tick.rs` 与 `planned_tick.rs`，根据建筑实际雇佣 POP 的平均识字率/熟练比例计算资质倍率；资质不足时高级工业和高级军工产出折减，市场经济与计划经济路径均接入。
- 新增教育增长逻辑：大学等级和 Clerk 占比逐日提高国家 POP 的识字率和熟练比例，形成“建大学/扩大职员 -> 中期改善资质瓶颈”的链路。
- 更新 `pop_panel.rs` 与 `main.rs`，POP 面板显示国家与阶级层面的识字率、熟练人口和资质瓶颈提示。
- 补充测试：`low_qualification_reduces_advanced_industry_output`、`university_and_clerks_raise_qualifications`。
- 验证：`cargo fmt`、`cargo test -p hoi4-logic economy::market_tick -- --nocapture`、`cargo check` 通过。

### V7.I8 — 动员与劳动力冲突 ✅ 已完成

目标：扩军真实伤经济。

任务：

- 训练和扩军从 POP 中抽人。
- 高征兵法从 Worker/Peasant/Clerk 转 Soldier。
- 被征走人口离开建筑岗位，造成产出下降。
- 伤亡降低士兵 POP，并影响家庭满意度/战争支持。

验收：

- 大规模扩军后工厂就业下降。
- 高征兵法降低 POP 满意度。
- 长期高伤亡降低战争支持和稳定度。

完成记录：

- 更新 `crates/hoi4-logic/src/economy/law_modifiers.rs`，征兵转换从 `Peasant / Worker / Clerk` 抽人，未就业人口优先；不足时会抽走在岗人口并同步扣减对应建筑就业槽，让扩军直接伤害工厂就业和产出。
- 保留征兵法满意度修正，高征兵法通过 `pop_modifiers.satisfaction` 压低 POP 满意度。
- 更新 `crates/hoi4-logic/src/military/training.rs`，训练队列每日分配人力时实际消耗未编入师的 `Soldier` POP，不再只读取 `world.manpower()` 作为抽象可用池。
- 更新 `crates/hoi4-logic/src/economy/stockpile.rs`，战斗强度损失会按模板人力折算伤亡，降低 Soldier POP，并对家庭/社会产生满意度、激进化、稳定度、战争支持压力。
- 更新 `crates/hoi4-logic/src/military/arbiter.rs` 与 `crates/hoi4-integration/tests/stockpile_loop.rs`，适配战斗损失回写需要的可变 `World`。
- 补充测试：`mobilization_reduces_available_workers`、`combat_casualties_reduce_stability_and_pop_satisfaction`，并扩展 `training_queue_deploys_after_time_and_equipment` 验证训练消耗 Soldier POP。
- 验证：`cargo fmt`、I8 目标测试、`cargo check` 通过。

### V7.I9 — 主要国家 POP 与产业初始校准 ✅ 已完成

第一批国家：

```text
GER
USA
SOV
ENG
FRA
JAP
ITA
CHI
```

任务：

- 为每国生成初始 POP。
- 按历史工业结构放置汽车厂、发动机厂、机床厂、炼油厂等。
- 日本资源进口依赖、德国合成工业、美国汽车工业、苏联计划工业、英国海运金融必须可见。

验收：

- USA 汽车/机床/石油/炼油优势明显。
- GER 化工、机床、钢铁、军工强，但油/橡胶依赖明显。
- JAP 造船和军工可用，但油/铁/铝/橡胶依赖进口。
- SOV 原料和重工业可扩张，但资质和精密工业短板明显。

完成记录：

- 确认 `V6Database::load()` 已加载 `initial_usa/sov/eng/fra/jap/ita/chi/ger.ron`，八个主要国家均有手写初始 POP，并由历史 profile 补齐测试世界/未覆盖州的人口分布。
- 更新 `crates/hoi4-content/src/v6_loader.rs`，在 1936 历史建筑生成中增加国家专属产业校准目标，并优先放置关键产业，避免被通用 GDP/部门份额建筑挤占州槽。
- USA 优先获得 `vehicle_factory`、`engine_plant`、`machine_tool_works`、`oil_refinery` 等汽车、机床、石油炼制能力。
- GER 优先获得 `steel_mill`、`chemical_plant`、`machine_tool_works`、`arms_industry`、`munition_plant`、`synthetic_refinery`，同时不新增本土 `oil_rig/rubber_plantation`，保留油/橡胶依赖。
- JAP 优先获得 `shipyard`、`arms_industry`、`munition_plant`、`engine_plant`、`vehicle_factory`，并依赖历史贸易路线进口油、钢、橡胶。
- SOV 优先获得钢铁、机械、油田/炼油和有限机床/发动机，形成重工业与资源可扩张但精密工业、资质弱于 USA 的差异。
- ENG 优先获得 `bank`、`shipyard`、`telegraph_office`、机床/发动机/炼油，突出海运金融。
- 补充测试：`i9_major_country_industry_calibration_is_visible`，直接覆盖 USA、GER、JAP、SOV、ENG 的 I9 验收差异。
- 验证：`cargo fmt`、`cargo test -p hoi4-content i9_major_country_industry_calibration_is_visible -- --nocapture`、`cargo test -p hoi4-content h3_ -- --nocapture`、`cargo test -p hoi4-content h5_historical_trade_routes_are_injected -- --nocapture`、`cargo test -p hoi4-content h4_initial_gdp_is_calibrated_from_historical_buildings -- --nocapture`、`cargo test -p hoi4-content h2_historical_profiles_generate_initial_buildings -- --nocapture`、`cargo test -p hoi4-content history_1936_loads -- --nocapture`、`cargo test -p hoi4-logic --test v7_initial_gdp_calibration -- --nocapture`、`cargo check` 通过。


## 9. 测试与验收

### 8.1 单元测试

必须新增或更新：

```text
✅ content production_methods_vec_lengths_match
✅ all_buildings_have_production_methods
✅ vehicle_factory_outputs_motorized
✅ oil_refinery_outputs_fuel
⚠️ pop_panel_data_sums_population（I1 已做 UI 聚合与 cargo check；未单独落同名测试）
pop_needs_shortage_lowers_satisfaction
✅ low_qualification_reduces_advanced_industry_output
✅ university_and_clerks_raise_qualifications
✅ mobilization_reduces_available_workers
✅ combat_casualties_reduce_stability_and_pop_satisfaction
✅ training_queue_deploys_after_time_and_equipment
✅ market_panel_splits_pop_consumption_demand
✅ market_panel_distinguishes_order_from_traded_volume
⚠️ market_panel_shows_upstream_bottleneck（I4 已显示 upstream/downstream；未单独落同名测试）
```

当前验证记录：

- `cargo fmt` 通过。
- `cargo check` 通过。
- `cargo test -p hoi4-ui market_panel -- --nocapture` 通过。
- `cargo test -p hoi4-logic economy::market_tick -- --nocapture` 通过。
- `cargo test -p hoi4-logic mobilization_reduces_available_workers -- --nocapture` 通过。
- `cargo test -p hoi4-logic combat_casualties_reduce_stability_and_pop_satisfaction -- --nocapture` 通过。
- `cargo test -p hoi4-logic training_queue_deploys_after_time_and_equipment -- --nocapture` 通过。
- 内容层 sanity tests 通过：`production_methods_vec_lengths_match`、`all_buildings_have_production_methods`、`h3_ger_initial_industrial_inputs_are_self_sustaining`。
- `cargo test -p hoi4-content` 全量当前仍有 2 个既有 `event_tick` 失败：`event_tick::tests::mtth_zero_fires_immediately`、`event_tick::tests::one_tick_per_day`。这两个失败与 I1-I4 改动无关。

### 8.2 回放验收

1936 开局跑 90 天，检查：

- POP 总量不凭空大幅变化。
- 就业率合理变化。
- 军工订单能传导上游需求。
- 汽车厂产出摩托化装备。
- 石油短缺影响燃油。
- 橡胶短缺影响车辆/飞机维护和生产。
- 高失业/短缺提高 POP 激进化。

### 8.3 UI 验收

- POP 面板可以从快捷键打开。
- 总览、阶级、州分布、生活需求、政治五页可滚动。
- 大国 POP 数据不卡顿。
- 面板数据与实际 POP store 汇总一致。
- 市场面板总览、短缺、商品、产业链、贸易五页可滚动。
- 商品详情显示供给来源、需求来源、成交、未成交和价格原因。
- 市场面板能显示 POP 消费短缺影响。


## 10. 文件改动清单

### 9.1 内容层

```text
crates/hoi4-content/content/economy_v6/goods.ron
crates/hoi4-content/content/economy_v6/buildings/buildings.ron
crates/hoi4-content/content/economy_v6/production_methods/resource.ron
crates/hoi4-content/content/economy_v6/production_methods/industrial.ron
crates/hoi4-content/content/economy_v6/production_methods/military.ron
crates/hoi4-content/content/economy_v6/production_methods/consumer_goods.ron
crates/hoi4-content/content/economy_v6/pops/*.ron
crates/hoi4-content/content/history_1936/countries/*.ron
crates/hoi4-content/content/history_1936/resources/state_deposits.ron
```

### 9.2 状态层

```text
crates/hoi4-state/src/pops.rs
crates/hoi4-state/src/store.rs
crates/hoi4-state/src/world.rs
```

### 9.3 逻辑层

```text
crates/hoi4-logic/src/economy/market_tick.rs
crates/hoi4-logic/src/economy/planned_tick.rs
crates/hoi4-logic/src/economy/finance_tick.rs
crates/hoi4-logic/src/economy/stockpile.rs
crates/hoi4-logic/src/military/training.rs
crates/hoi4-logic/src/trade/mod.rs
```

### 9.4 UI 层

```text
crates/hoi4-ui/src/pop_panel.rs
crates/hoi4-ui/src/market_panel.rs
crates/hoi4-ui/src/lib.rs
crates/hoi4-app/src/main.rs
```


## 11. 风险

| 风险 | 说明 | 缓解 |
|---|---|---|
| 商品过多导致 UI 噪音 | 产业链扩展后市场面板变复杂 | 增加分类、搜索、短缺优先 |
| 生产链过深导致玩家看不懂 | 1936 工业比原先复杂很多 | POP/市场/建筑面板必须显示“瓶颈来源” |
| 市场面板信息过载 | 供给、需求、贸易、POP、军工全部显示会拥挤 | 标签页拆分，总览只显示关键告警 |
| 平衡崩溃 | 新中间品可能让装备全停产 | 分阶段启用，先给初始库存和初始建筑 |
| POP 性能 | 大国 POP 多 | 聚合 POPGroup，不逐人模拟 |
| 旧测试失败 | 税收、军购、生产方式语义改变 | 同步测试语义，不保旧错误行为 |


## 12. 第一批建议实施顺序

1. ✅ 新增 POP 面板只读版。
2. ✅ 新增 `engine_plant / vehicle_factory / oil_refinery / rubber_factory / machine_tool_works`。
3. ✅ 新增 `fuel / rubber_parts / machine_tools / vehicle_parts / spare_parts`。
4. ✅ 把 `motorized / mechanized` 迁到汽车厂。
5. ✅ 把油田和炼油拆开。
6. ✅ 市场面板增加总览、短缺、需求来源、供给来源、军购成交解释。
7. POP 增加需求满足率和激进化字段。
8. POP 面板显示生活需求和政治压力。
9. ✅ 重构支援装备、步兵装备、火炮、坦克、飞机配方。
10. 主要国家 POP 和产业初始校准。
11. 回放 90 天校准 GDP、就业、短缺、军工产出。


## 13. 完成定义

本路线图完成时，玩家应能看到并感受到：

- 美国为什么容易摩托化和大规模军工化。
- 德国为什么钢铁、化工、机床强，但油和橡胶是战略瓶颈。
- 日本为什么必须依赖进口油、铁、橡胶、铝。
- 苏联为什么可以扩重工业，但早期精密工业、无线电、机床和资质受限。
- 扩军为什么会抽走劳动力并影响工厂产出。
- 商品短缺为什么会压低 POP 生活水平并提高激进化。
- 市场面板能解释每个商品为什么涨价、为什么短缺、谁受影响、该建什么或进口什么。
- 汽车厂、发动机厂、机床厂、炼油厂、橡胶厂为什么是 1936 战争经济的核心。
