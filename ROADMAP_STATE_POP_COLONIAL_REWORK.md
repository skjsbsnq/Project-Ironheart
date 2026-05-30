# Project Ironheart — 州人口真相与殖民帝国联动重构路线图

> 写于 2026-05-24。
> 本路线图以当前源码为第一依据，现有 V7/V8 和其他路线图仅作为约束与上下文。
> 目标不是新增一套孤立人口系统，而是把人口、州、殖民地、建筑、市场、贸易、财政、军事、外交、UI 和存档连成可解释闭环。


## 0. 总目标

当前开局人口由国家级 profile 固定值驱动，州只是分配容器。这导致国家人口和实际领土不天然一致，尤其会在德国州人口、英法殖民地、傀儡与自治领、割地/吞并后人口变化上产生不合理结果。

目标状态：

```text
州人口是人口真相。
国家人口由拥有州 POP 汇总得出。
本土、殖民地、自治领、傀儡人口分别建账。
建筑和工业不能直接按总人口放大，而由 GDP、工业化、基础设施、资源、州整合状态决定。
殖民地通过资源、贸易、财政抽取、有限兵源、市场圈和政治风险影响宗主国。
```


## 1. 非孤立原则

本路线图必须和以下系统联动，不能单独只改人口加载：

| 系统 | 必须联动的原因 |
|---|---|
| `hoi4-state` 世界状态 | 州人口、POP、国家人口、本土/殖民状态必须成为运行时状态 |
| `hoi4-content` 历史数据 | 1936 州人口、殖民状态、国家 profile、初始法律、贸易和资源需要统一加载 |
| `hoi4-logic` 经济 tick | 税基、消费、就业、工资、生活水平、财政 GDP 必须按人口口径重算 |
| 建筑系统 | 初始建筑与建造校验不能被殖民人口放大，必须按州整合状态过滤 |
| 市场与贸易 | 殖民地资源和 subject 贡献必须通过市场圈、贸易路线、资源抽取进入宗主国 |
| 外交/傀儡 | Autonomy 关系必须影响资源份额、兵源、财政和市场准入 |
| 军事/动员 | 可动员人口不能等于总人口；殖民兵源必须受法律、自治和政治风险约束 |
| 财政 | 税收、GDP、公共债务和投资池不能把殖民人口当本土税基 |
| UI | 国家面板、POP 面板、市场面板、外交面板必须显示不同人口口径 |
| 存档 | 新状态字段会影响 save/load，需要版本化或明确不兼容旧中间存档 |
| 测试 | 每阶段必须有自动测试或 1936 回放验收，不能只靠视觉检查 |


## 2. 当前源码事实

### 2.1 人口注入

源码位置：

```text
crates/hoi4-content/src/v6_loader.rs
crates/hoi4-content/content/history_1936/countries/*.ron
crates/hoi4-content/content/economy_v6/pops/initial_*.ron
```

当前主要流程：

```text
inject_v6_into_world
  -> apply_historical_laws
  -> apply_historical_finance
  -> inject_historical_pops
  -> inject_historical_buildings
  -> calibrate_building_employment_to_population
  -> seed_initial_building_employment
  -> seed_initial_market_stockpiles
  -> apply_historical_gdp_from_buildings
```

关键事实：

| 项 | 当前实现 |
|---|---|
| 国家人口 | `HistoricalCountryEconomyDef.population` 固定写在 `history_1936/countries/*.ron` |
| 手写 POP | `initial_*.ron` 按国家分文件，内部引用 `state_id` |
| 主要国家 POP | 先注入手写 POP，再用国家 `profile.population` 补齐剩余人口 |
| 非 profile 国家 | 使用拥有州 `manpower_pool` 合计乘以 8.5 估算 |
| POP 存储 | `PopGroup` 只挂在 `StateId`，不挂在 province |
| 国家人口 UI | 当前已有通过 owned states 的 POP 汇总函数，方向正确 |

主要问题：

| 问题 | 后果 |
|---|---|
| 国家 profile 人口是强目标 | 国家人口不由拥有州自然决定 |
| `initial_*.ron` 和 `countries/*.ron` 双口径 | 德国等国家存在 6700 万 vs 6800 万类似差异 |
| 缺州级人口真相表 | 无法稳定处理割地、殖民、吞并和释放国家 |
| 缺本土/殖民状态 | 英法殖民地无法区别于本土人口 |
| 缺 subject/帝国人口口径 | 英国、法国、日本等无法显示帝国人口和国内人口差异 |


### 2.2 州与省份

源码位置：

```text
crates/hoi4-state/src/store.rs
crates/hoi4-state/src/world.rs
crates/hoi4-data/src/loader.rs
```

当前 `StateStore` 有：

```text
owners
controllers
cores
provinces
infrastructure
category_slots
manpower_pool
resistance
compliance
occupiers
names
```

当前没有：

```text
state population cache
integration status
colonial status
metropole flag
state GDP share
state urbanization/literacy profile
```

结论：第一阶段不必把人口下沉到 province。当前架构的自然粒度是 state。省份人口以后可以作为地图显示或战损细化扩展，但不应作为本轮重构前置条件。


### 2.3 初始建筑

源码位置：

```text
crates/hoi4-content/src/v6_loader.rs
crates/hoi4-state/src/buildings_v6.rs
crates/hoi4-content/content/economy_v6/buildings/buildings.ron
```

当前初始建筑目标主要由以下字段生成：

```text
gdp_1936_gbp
sector_shares
industrial_capacity_index
construction_capacity_index
tag-specific calibration targets
```

这是正确方向。人口州化以后，建筑目标仍不应直接按总人口生成。

当前风险点：

```text
owned_states_by_weight = manpower_pool + infrastructure * 750000
```

如果把殖民地人口或 manpower 真实化，但仍用所有 owned states 分配现代工业，会让殖民地错误获得钢厂、机床、军工、汽车厂、大学、银行等建筑。


### 2.4 傀儡与殖民经济

源码位置：

```text
crates/hoi4-state/src/diplomacy.rs
crates/hoi4-content/src/v6_loader.rs
crates/hoi4-logic/src/diplomacy/puppet.rs
```

已有结构：

```text
AutonomyLevel
Autonomy
master_resource_share()
```

已有初始关系：

```text
ENG -> CAN/AST/NZL/SAF as Dominion
ENG -> RAJ/MAL as Puppet
JAP -> MAN as Puppet
JAP -> MEN as IntegratedPuppet
```

当前不足：

| 项 | 现状 |
|---|---|
| 资源份额 | 有函数，但没有完整进入宗主国经济闭环 |
| 市场圈 | 已有 MarketBloc 雏形和 V8 目标，但运行语义还不完整 |
| 财政抽取 | 缺真实机制 |
| 人力贡献 | 缺 colonial/subject 兵源口径 |
| 工业控制 | 缺宗主国投资、产权、殖民建筑所有权 |
| 政治反馈 | 缺抽取导致自治度/激进化/抵抗变化 |


## 3. 和现有路线图的关系

本路线图不是替代 V7/V8，而是补齐其中“人口与殖民地口径”的缺口。

| 路线图 | 与本路线图关系 |
|---|---|
| `ROADMAP_V7_HISTORICAL_ECONOMY_RECONSTRUCTION.md` | V7 要建立历史 GDP/人口/资源/建筑/贸易闭环。本路线图把其中“人口”从国家强目标改成州级真相。 |
| `ROADMAP_V7_INDUSTRY_POP_REWORK.md` | V7 POP 强化要求人口影响就业、消费、税收、满意度、动员。本路线图提供州级 POP 来源和本土/殖民口径。 |
| `ROADMAP_V8_INVESTMENT_MARKET_EMPIRE_REWORK.md` | V8 要做帝国经济、市场圈、殖民抽取、投资产权。本路线图提供殖民人口、殖民地类型和资源/兵源口径。 |
| `MILITARY_SYSTEM_HOI4_REWORK_ROADMAP.md` | 军事动员必须从 POP 抽人。本路线图定义本土兵源、殖民兵源、subject 兵源边界。 |
| `DIPLOMACY_FACTION_UI_REWORK_ROADMAP.md` | 外交 UI 已显示傀儡/自治关系。本路线图要求外交详情显示人口和经济依赖。 |
| `UI_PANEL_REWORK_ROADMAP.md` | 人口口径、殖民地、市场圈需要进入 UI 信息架构，不能只放在调试面板。 |
| `ROADMAP_1936_HISTORICAL_FLAGS.md` | 历史旗帜/国家身份会影响 colony、mandate、dominion、puppet 的显示和规则。 |


## 4. 目标口径

系统必须同时维护或可计算以下人口口径。

| 口径 | 定义 | 用途 |
|---|---|---|
| State Population | 某州所有 POP group size 合计 | 地图、州面板、建筑就业、占领、割地 |
| Domestic Population | owner 为本国且州为 core/metropole/incorporated 的人口 | 国内政治、主税基、本土动员、本土消费 |
| Colonial Population | owner 为本国但州为 colony/unincorporated/protectorate 的人口 | 殖民治理、资源抽取、有限税基、有限兵源 |
| Governed Population | Domestic + Colonial | 国家管辖人口显示和行政负担 |
| Subject Population | diplomacy autonomy 中 subject 国家自己的人口 | 帝国人口、市场圈贡献、宗主国影响 |
| Imperial Population | Governed + subjects/dominions/puppets 人口 | 英法日等帝国面板显示 |
| Recruitable Domestic Population | 受征兵法影响的本土可动员人口 | 军事人力 |
| Recruitable Colonial Population | 受殖民政策/自治/事件影响的殖民可动员人口 | 殖民部队、政治风险 |
| Industrial Workforce | 可进入现代工业岗位的 POP 子集 | 建筑就业、产出、工资 |
| Taxable Base | 由工资、GDP、法律和整合状态决定 | 财政收入，不等于总人口 |


## 5. 数据模型目标

### 5.1 州历史人口数据

新增内容文件：

```text
crates/hoi4-content/content/history_1936/states/state_population.ron
```

目标 schema：

```rust
pub struct StatePopulation1936Def {
    pub state_id: u16,
    pub population: u32,
    pub integration: StateIntegrationDef,
    pub urbanization: Option<f32>,
    pub literacy: Option<f32>,
    pub workforce_profile: Option<WorkforceProfileDef>,
    pub data_quality: HistoricalDataQuality,
}

pub enum StateIntegrationDef {
    Metropole,
    Incorporated,
    Colony,
    Protectorate,
    Mandate,
    Concession,
    Occupied,
}

pub enum WorkforceProfileDef {
    Agrarian,
    Industrial,
    Mining,
    Plantation,
    UrbanServices,
    MilitaryFrontier,
}
```

规则：

| 字段 | 规则 |
|---|---|
| `state_id` | 使用 HOI4 game state id，通过 `state_id_lookup` 转内部 StateId |
| `population` | 1936 州常住人口，是州 POP 生成的目标 |
| `integration` | 决定本土/殖民/占领口径，不等同于 owner |
| `urbanization` | 缺省时从国家 profile 和 infrastructure 推断 |
| `literacy` | 缺省时从国家 profile、integration、workforce profile 推断 |
| `workforce_profile` | 决定 POP 阶级分布和建筑就业倾向 |
| `data_quality` | 测试和审计使用，不直接影响玩法 |


### 5.2 运行时州状态

目标文件：

```text
crates/hoi4-state/src/store.rs
```

建议新增：

```rust
pub enum StateIntegrationStatus {
    Metropole,
    Incorporated,
    Colony,
    Protectorate,
    Mandate,
    Concession,
    Occupied,
}

pub struct StateStore {
    ...
    pub integration_status: Vec<StateIntegrationStatus>,
    pub population_cache: Vec<u32>,
    pub population_cache_dirty: Vec<bool>,
}
```

说明：

| 字段 | 目的 |
|---|---|
| `integration_status` | 运行时规则判断，存档需要保存 |
| `population_cache` | UI 和频繁查询缓存，权威仍是 POP groups |
| `population_cache_dirty` | POP 迁移、伤亡、割地、事件后标记重算 |

第一阶段可以不加缓存，直接汇总 POP；但 UI 和大型世界运行后建议加缓存。


### 5.3 国家 profile 调整

当前：

```rust
pub struct HistoricalCountryEconomyDef {
    pub population: u32,
    ...
}
```

目标：

```rust
pub struct HistoricalCountryEconomyDef {
    pub reference_population: Option<u32>,
    pub gdp_1936_gbp: f64,
    ...
}
```

迁移期可以保留 `population` 字段，但语义必须改为：

```text
仅用于校验和 fallback，不作为主要国家强制补齐目标。
```

验收规则：

```text
若州人口汇总和 reference population 偏差超过阈值，测试给出 tag 和差异。
不能因为偏差自动缩放所有州人口，除非显式 fallback 模式。
```


### 5.4 POP 生成策略

目标文件：

```text
crates/hoi4-content/src/v6_loader.rs
```

现有 `inject_historical_pops()` 应拆成更小的步骤：

```text
load_state_population_profiles
clear_existing_starting_pops
inject_state_pops_from_profiles
inject_hand_authored_pop_overrides
generate_missing_state_pops_from_manpower
validate_country_reference_population
refresh_population_caches
```

规则：

| 情况 | 行为 |
|---|---|
| 州有 state_population profile | 以州人口为目标生成 POP |
| 州还有手写 POP override | override 可以替换或细化阶级分布，但不得改变州总人口，除非标记 `allow_total_override` |
| 州无 profile 但有 vanilla manpower | 用 `manpower * factor` 估算 |
| 州无 profile 且 manpower 为 0 | 用 category/infrastructure/owner fallback 低值估算 |
| 国家 profile 人口和州汇总不符 | 记录报告，不强行补齐 |


## 6. 建筑系统联动

### 6.1 建筑目标继续由经济 profile 决定

不能改成：

```text
人口越多，工厂越多。
```

应保持：

```text
GDP + sector_shares + industrial_capacity_index + construction_capacity_index + resources + historical calibration -> initial buildings
```


### 6.2 建筑分配按建筑类别选择州

新增分配函数：

```text
states_for_industrial_building(country)
states_for_resource_building(country, good_id)
states_for_agriculture_building(country)
states_for_service_building(country)
states_for_colonial_extraction(country)
```

规则：

| 建筑类型 | 本土 | 殖民地 | subject |
|---|---:|---:|---:|
| steel_mill | 高优先 | 禁止或极低权重 | 不直接放，除非 subject 自己 profile 生成 |
| machinery_workshop | 高优先 | 禁止或极低权重 | 不直接放 |
| machine_tool_works | 高优先 | 默认禁止 | 不直接放 |
| arms_industry | 高优先 | 默认禁止 | subject 自己生成 |
| vehicle_factory | 高优先 | 默认禁止 | subject 自己生成 |
| university | 高优先 | 低权重，仅大殖民行政中心 | subject 自己生成 |
| bank | 高优先 | 低权重，殖民金融中心例外 | subject 自己生成 |
| grain_farm | 正常 | 可放 | subject 自己生成 |
| livestock_ranch | 正常 | 可放 | subject 自己生成 |
| rubber_plantation | 资源地 | 高优先 | subject 自己生成并可被抽取 |
| oil_rig | 资源地 | 高优先 | subject 自己生成并可被抽取 |
| mines | 资源地 | 高优先 | subject 自己生成并可被抽取 |
| railway | 正常 | 可放但受限 | subject 自己生成 |
| port/shipyard | 港口本土优先 | 港口可放，shipyard 需历史锚点 | subject 自己生成 |


### 6.3 殖民建筑所有权

和 V8 投资产权路线图对齐。

目标：

```text
殖民地资源建筑可以位于 colony state，但 ownership/account 可部分属于宗主国。
```

第一阶段可简化：

```text
Building.owner 仍为当前 state owner。
Market/trade 抽取按 autonomy/integration 计算，不急着引入完整产权份额。
```

第二阶段与 V8 对齐：

```rust
OwnershipAccount::Overlord { master, subject }
InvestmentAccountKind::ColonialExtraction
```


## 7. 市场与贸易联动

### 7.1 市场圈

现有方向见 V8 `MarketBloc`。本路线图要求人口和殖民状态进入市场圈计算。

市场圈成员贡献至少包含：

```text
goods supply contribution
goods demand contribution
strategic resource contribution
population demand contribution
colonial extraction value
market access
blockade risk
```


### 7.2 殖民资源抽取

初始规则：

```text
Colony state surplus resource -> owner market
Subject state surplus resource -> subject market first, then master priority import by autonomy share
Dominion -> lower forced share, more normal trade
Puppet -> higher forced share
IntegratedPuppet -> near mandatory extraction
```

现有 `AutonomyLevel::master_resource_share()` 可作为第一版比例来源。


### 7.3 贸易路线

现有历史贸易文件：

```text
crates/hoi4-content/content/history_1936/trade/initial_trade_1936.ron
```

需要扩展：

```text
ImperialPreference route
ColonialExtraction route
SubjectContribution route
StrategicImport route
```

但运行时不一定要新增 enum，第一阶段可用已有 `TradeRouteKind::ImperialPreference` 并增加解释字段。


## 8. 财政与 GDP 联动

### 8.1 GDP

GDP 仍应由建筑、政府支出、消费、贸易解释，不能由人口直接给出。

人口州化后需要修正：

```text
POP consumption demand 按 state integration 修正。
殖民地消费可进入本地市场或宗主国市场的程度必须明确。
GDP 统计应区分 domestic GDP、colonial extracted value、subject contribution。
```


### 8.2 税基

税基不能简单使用总人口。

建议口径：

| 人口类型 | 税基系数 |
|---|---:|
| Metropole/Core | 1.00 |
| Incorporated | 0.75-1.00 |
| Colony | 0.15-0.45 |
| Protectorate/Mandate | 0.05-0.30 |
| Occupied | 0.00-0.20 |
| Subject | 不进入宗主国直接税基，只通过 tribute/trade/extraction |

系数应由法律、自治、占领政策和殖民政策修正。


### 8.3 投资池

和 V8 对齐：

```text
Private investment pool 不应直接吃殖民地全人口红利。
ColonialExtraction account 接收殖民资源利润的一部分。
OverlordInvestment 可在殖民地建设港口、铁路、矿山、种植园。
```


## 9. 军事与动员联动

### 9.1 可用人力

当前 `CountryStore::manpower` 已由 Soldier POP 派生，这是正确方向。

需要扩展：

```text
Soldier POP 生成时区分本土、殖民、subject。
招募时按 state integration 和法律过滤。
殖民兵源不等同于本土兵源。
```


### 9.2 征兵法律

征兵法必须有不同系数：

```text
domestic_recruitable_ratio
colonial_recruitable_ratio
subject_force_contribution_ratio
political_cost_multiplier
radicalism_gain_multiplier
```


### 9.3 殖民部队

第一阶段只需要规则化人力贡献：

```text
Colonial manpower contributes to recruit pool with cap.
Subject manpower does not directly become master manpower unless autonomy/military agreement allows.
```

第二阶段可接 OOB/模板：

```text
colonial divisions
expeditionary forces
subject templates
```


## 10. 外交、自治与占领联动

### 10.1 Autonomy

当前 AutonomyLevel 需要新增经济语义入口：

```rust
master_resource_share()
master_tax_share()
master_manpower_share()
market_access_modifier()
autonomy_pressure_from_extraction()
```

不一定都放在 enum impl；也可以放到 logic 层，避免 state crate 过重。


### 10.2 占领

占领系统已有 resistance/compliance 字段。人口州化后，占领收益和治安成本要按州人口计算。

规则：

```text
人口越多，占领行政成本越高。
资源州和高人口州需要更多 garrison。
compliance 提高可增加税基、资源抽取和劳动力可用性。
resistance 提高会破坏建筑、贸易和市场准入。
```


### 10.3 释放国家/割地/吞并

必须做到：

```text
转移 state owner 后，人口随州走。
国家人口无需单独修正。
建筑就业、市场、税基、人力缓存标 dirty。
subject/colonial status 可由和平条约或事件设置。
```


## 11. UI 联动

### 11.1 国家信息面板

需要显示：

```text
本土人口
殖民人口
管辖人口
subject 人口
帝国人口
可动员本土人口
可动员殖民人口
```


### 11.2 POP 面板

需要新增筛选：

```text
按州
按本土/殖民/占领
按阶级
按就业
按满意度/激进化
```


### 11.3 市场面板

需要和 V8 市场圈页对齐，显示：

```text
殖民资源贡献
subject 贡献
帝国市场进口
世界市场进口
殖民地短缺和激进化风险
```


### 11.4 外交面板

subject/puppet/detail 需要显示：

```text
自治等级
人口
资源贡献
财政贡献
人力贡献
市场准入
自治度变化来源
```


## 12. 存档与兼容

本路线图不要求兼容旧中间存档。

必须做：

```text
save text/binary 增加 state integration status。
POP fields 如果不变，可继续序列化。
新增 population cache 可以不存档，读档后重建。
旧 save 读取时对缺失 integration 使用 Core/Metropole fallback。
```

相关源码：

```text
crates/hoi4-state/src/save/text.rs
crates/hoi4-state/src/save/binary.rs
crates/hoi4-state/tests/save_roundtrip.rs
```


## 13. 实施阶段

### Phase 0: 审计和保护测试

目标：建立当前行为基线，防止重构中误伤建筑、GDP、贸易、军事。

任务：

| 编号 | 内容 | 文件 |
|---|---|---|
| P0.1 | 增加主要国家开局人口快照测试 | `crates/hoi4-logic/tests` 或 `crates/hoi4-content/tests` |
| P0.2 | 增加德国州 POP 汇总测试 | 同上 |
| P0.3 | 增加英帝国 subject 关系存在测试 | `crates/hoi4-content/tests/history_1936_loads.rs` |
| P0.4 | 增加初始建筑不落到错误殖民地的测试骨架 | `crates/hoi4-logic/tests` |
| P0.5 | 增加 save roundtrip 对 POP/state 字段的基线 | `crates/hoi4-state/tests/save_roundtrip.rs` |

验收：

```text
cargo test -p hoi4-content history_1936_loads
cargo test -p hoi4-state save_roundtrip
cargo test -p hoi4-logic v7_initial_gdp_calibration
```


### Phase 1: 州人口数据层

目标：新增州级历史人口数据，不改变运行逻辑。

任务：

| 状态 | 编号 | 内容 | 文件 |
|---|---|---|---|
| ✅ | P1.1 | 新增 `StatePopulation1936Def` schema | `crates/hoi4-content/src/v7_history_loader.rs` |
| ✅ | P1.2 | 新增 `state_population.ron` | `crates/hoi4-content/content/history_1936/states/` |
| ✅ | P1.3 | `Historical1936Database` 加载 state population | `v7_history_loader.rs` |
| ✅ | P1.4 | 增加 loader 测试，确保 state_id 可解析 | `crates/hoi4-content/tests/history_1936_loads.rs` |
| ✅ | P1.5 | 初始只覆盖德国、英国本土、法国本土、英属印度、马来亚、加拿大、澳洲、南非、新西兰 | 内容数据 |

验收：

```text
州人口 RON 可加载。
所有 state_id 在真实 World 中可解析。
主要样本国家州人口合计与 reference population 差异可报告。
运行逻辑未变化。
```


### Phase 2: 运行时州整合状态

目标：World 能保存和查询州本土/殖民/占领状态。

任务：

| 状态 | 编号 | 内容 | 文件 |
|---|---|---|---|
| ✅ | P2.1 | 新增 `StateIntegrationStatus` | `crates/hoi4-state/src/store.rs` 或新模块 |
| ✅ | P2.2 | `StateStore::new` 初始化 integration | `store.rs` |
| ✅ | P2.3 | `World::new` 从数据层写入 integration | `world.rs` 或 content 注入阶段 |
| ✅ | P2.4 | save text/binary 支持 integration | `save/text.rs`, `save/binary.rs` |
| ✅ | P2.5 | 增加 roundtrip 测试 | `save_roundtrip.rs` |

验收：

```text
开局后可查询每个州的 integration status。
存读档后 status 保持。
缺数据州 fallback 不崩溃。
```


### Phase 3: 州人口驱动 POP 生成

目标：主要人口来源从国家 profile 改为州人口 profile。

任务：

| 状态 | 编号 | 内容 | 文件 |
|---|---|---|---|
| ✅ | P3.1 | 拆分 `inject_historical_pops` | `v6_loader.rs` |
| ✅ | P3.2 | 新增 `inject_state_pops_from_profiles` | `v6_loader.rs` |
| ✅ | P3.3 | 手写 `initial_*.ron` 改为阶级分布 override，而不是国家总人口强目标 | 内容数据和 loader |
| ✅ | P3.4 | `profile.population` 改为校验/fallback | `v7_history_loader.rs`, `v6_loader.rs` |
| ✅ | P3.5 | 国家人口汇总测试 | `crates/hoi4-logic/tests` |

验收：

```text
德国人口 = 德国 owned states POP 合计。
移交一个州后，国家人口自然变化。
主要国家不再因 profile.population 自动补齐人口。
缺州人口数据的国家仍能 fallback 生成。
```


### Phase 4: 本土/殖民/subject 人口口径

目标：国家人口不再只有一个总数。

任务：

| 状态 | 编号 | 内容 | 文件 |
|---|---|---|---|
| ✅ | P4.1 | 新增 population aggregation helpers | `hoi4-state` 或 `hoi4-logic` |
| ✅ | P4.2 | 计算 domestic/colonial/governed/imperial population | 新 helper |
| ✅ | P4.3 | 国家信息面板显示多人口口径 | `crates/hoi4-app/src/main.rs`, `crates/hoi4-ui/src/diplomacy.rs` 或 country panel |
| ✅ | P4.4 | POP 面板支持本土/殖民筛选 | `crates/hoi4-ui/src/pop_panel.rs` |
| ✅ | P4.5 | 测试 ENG 帝国人口不等于 ENG 国内人口 | `crates/hoi4-logic/tests` |

验收：

```text
ENG domestic population 只含英国本土或 incorporated states。
RAJ/MAL 作为 subject population，不直接进入 ENG domestic population。
FRA colonial states 进入 colonial/governed，不进入 domestic。
```


### Phase 5: 建筑分配防殖民人口放大

目标：人口州化后，初始建筑不会被殖民人口错误放大或错配。

任务：

| 状态 | 编号 | 内容 | 文件 |
|---|---|---|---|
| ✅ | P5.1 | 替换或拆分 `owned_states_by_weight` | `v6_loader.rs` |
| ✅ | P5.2 | 按 building category 选择州 | `v6_loader.rs` |
| ✅ | P5.3 | 工业建筑默认只分配到 metropole/incorporated/core | `v6_loader.rs` |
| ✅ | P5.4 | 资源/农业建筑允许殖民地，但需要资源/土地/历史约束 | `v6_loader.rs` |
| ✅ | P5.5 | 测试英法本土工业不被殖民地吸走 | `crates/hoi4-logic/tests` |

验收：

```text
ENG 的银行、机床、造船、军工主要在本土和历史工业州。
FRA 的现代工业主要在本土。
MAL 橡胶、SAF 矿产、RAJ 农业/纺织按各自国家或殖民口径生成。
殖民高人口州不会自动生成大量现代工业。
```


### Phase 6: 财政、消费、税基修正

目标：不同人口口径影响财政和市场，但不破坏现有 GDP 闭环。

任务：

| 状态 | 编号 | 内容 | 文件 |
|---|---|---|---|
| ✅ | P6.1 | POP 消费按 integration status 修正市场归属和满足率 | `market_tick.rs`, `planned_tick.rs` |
| ✅ | P6.2 | 税基按 integration status 和法律修正 | `finance_tick.rs` |
| ✅ | P6.3 | GDP 统计拆分 domestic/colonial/extracted | `finance.rs`, `finance_tick.rs` |
| ✅ | P6.4 | UI 显示税基和殖民抽取 | finance/market panels |
| ✅ | P6.5 | 30/90 天 GDP 回放测试 | `crates/hoi4-logic/tests` |

验收：

```text
殖民人口增加不直接等比例增加宗主国税收。
殖民地消费不会无条件进入宗主国本土市场。
主要国家 1936 GDP 校准仍在容许误差。
```


### Phase 7: 市场圈和殖民资源抽取

目标：把殖民地/subject 资源通过市场和贸易联动到宗主国。

任务：

| 状态 | 编号 | 内容 | 文件 |
|---|---|---|---|
| ✅ | P7.1 | 使用 `AutonomyLevel::master_resource_share()` 计算 subject 资源可抽取份额 | trade/market logic |
| ✅ | P7.2 | 增强 MarketBloc 成员贡献快照 | `hoi4-state/src/market.rs`, `hoi4-app/src/main.rs` |
| ✅ | P7.3 | 帝国贸易优先于世界现货市场 | `crates/hoi4-logic/src/trade/mod.rs` |
| ✅ | P7.4 | 市场面板显示殖民/subject 来源 | `crates/hoi4-ui/src/market_panel.rs` |
| ✅ | P7.5 | 测试 ENG 从 MAL 获得橡胶优先权 | `crates/hoi4-logic/tests` |

验收：

```text
ENG 能从 MAL/RAJ/CAN/AST/NZL/SAF 看到市场圈贡献。
JAP 能从 MAN/MEN 看到 subject 贡献。
资源抽取会影响 subject 市场剩余和自治压力。
```


### Phase 8: 军事人力和殖民兵源

目标：动员从真实 POP 来，但殖民/subject 人口受限制。

任务：

| 状态 | 编号 | 内容 | 文件 |
|---|---|---|---|
| ✅ | P8.1 | 可动员人口 helper 按 integration status 计算 | `crates/hoi4-logic/src/military/manpower.rs` |
| ✅ | P8.2 | 征兵法增加 domestic/colonial/subject 系数 | `crates/hoi4-content/src/v6_loader.rs`, `crates/hoi4-content/content/economy_v6/laws/conscription.ron`, `crates/hoi4-logic/src/economy/law_modifiers.rs` |
| ✅ | P8.3 | 训练队列抽人时记录来源口径 | `crates/hoi4-logic/src/military/training.rs` |
| ✅ | P8.4 | 殖民兵源使用提高 radicalism/autonomy pressure | `crates/hoi4-logic/src/economy/law_modifiers.rs`, `crates/hoi4-logic/src/military/training.rs`, `crates/hoi4-state/src/diplomacy.rs` |
| ✅ | P8.5 | 测试 ENG 不能直接把 RAJ 全人口当本土 manpower | `crates/hoi4-logic/tests/phase8_colonial_manpower.rs` |

验收：

```text
本土征兵和殖民征兵分开。
subject 不直接给宗主国 manpower，除非法律/自治/事件允许。
扩军会减少对应 POP 或 Soldier POP。
```


### Phase 9: 殖民政治反馈

目标：殖民抽取、征兵和占领产生治理成本。

任务：

| 状态 | 编号 | 内容 | 文件 |
|---|---|---|---|
| ✅ | P9.1 | 抽取资源增加 autonomy pressure 或 radicalism | `crates/hoi4-logic/src/trade/mod.rs`, `crates/hoi4-state/src/diplomacy.rs` |
| ✅ | P9.2 | 高 resistance 降低资源抽取、税基和市场准入 | `crates/hoi4-logic/src/occupation/mod.rs`, `crates/hoi4-logic/src/economy/finance_tick.rs`, `crates/hoi4-logic/src/trade/mod.rs` |
| ✅ | P9.3 | compliance 提高殖民收益但有上限 | `crates/hoi4-logic/src/occupation/mod.rs` |
| ✅ | P9.4 | UI 显示殖民风险 | `crates/hoi4-app/src/main.rs`, `crates/hoi4-ui/src/market_panel.rs` |
| ✅ | P9.5 | 测试压榨殖民地会提高风险 | `crates/hoi4-logic/tests/v8_puppet_colonial_economy.rs`, `crates/hoi4-logic/tests/v7_occupation_garrison.rs` |


### Phase 10: 数据扩展与历史校准

目标：从主要国家扩展到全世界。

优先级：

| 状态 | 批次 | 内容 |
|---|---|---|
| ✅ | Batch A | GER, ENG metropole, FRA metropole, USA, SOV, ITA, JAP, CHI |
| ✅ | Batch B | RAJ, MAL, CAN, AST, NZL, SAF, MAN, MEN |
| ✅ | Batch C | 法国殖民地、荷兰殖民地、比利时刚果、葡萄牙殖民地 |
| ✅ | Batch D | 中东、拉美、巴尔干、北欧、次要国家 |
| ✅ | Batch E | 全世界缺口补齐和数据质量标记 |

当前落地范围：`state_population.ron` 已覆盖所有当前已接入 `history_1936/countries/*.ron` 且有手写 `initial_*.ron` POP profile 的国家州人口，并新增法国、荷兰、比利时和葡萄牙殖民地粗粒度州人口 profile。HOL/BEL/POR 国家经济 profile 尚未接入；对应殖民州会在真实 World 可解析 state id 时写入 integration status，POP 生成仍由 owner 是否进入历史经济 profile 决定。

Batch D/E 落地方式：仓库内当前没有全球 vanilla state history 文件可静态校验，因此不手写不可验证的全世界 state 表。运行时对所有缺 `state_population.ron` 显式记录的州执行全局 fallback：按州 `manpower_pool`、基础设施和 slots 估算州人口；按 owner/core/controller 判定 `Metropole`、`Colony` 或 `Occupied`；该路径作为 `Fallback` 数据质量语义，避免缺口州回退到国家人口强目标。

保护测试：

```text
cargo test -p hoi4-content --test history_1936_loads
cargo test -p hoi4-content p10_batch_de
```

数据质量允许分级：

```text
Primary
Estimated
Rough
Fallback
```


## 14. 冲突规避规则

### 14.1 不和 V7 冲突

V7 已经规定历史经济闭环，本路线图只改变人口来源和口径，不推翻 GDP/建筑/资源/贸易方向。

不能做：

```text
用人口直接决定 GDP。
用人口直接决定现代工业建筑数量。
恢复旧 HOI4 civ/mil/dock 工厂模型。
```


### 14.2 不和 V8 冲突

V8 要做投资、产权、帝国市场。本路线图第一阶段不强行实现完整产权，只提供必要人口和殖民状态。

不能做：

```text
新增另一套和 MarketBloc 平行的帝国市场系统。
新增另一套和 Autonomy 平行的 subject 关系。
新增另一套和 InvestmentAccount 平行的殖民投资池。
```

应做：

```text
复用 Autonomy。
复用 MarketBloc。
复用 InvestmentAccountKind::ColonialExtraction。
必要时给现有结构补字段，而不是平行造轮子。
```


### 14.3 不和军事系统冲突

不能让 manpower 又回到国家数字缓存。

应保持：

```text
manpower 从 Soldier POP 或可招募 POP 派生。
训练和补员必须消耗 POP。
殖民和 subject 兵源有单独规则。
```


### 14.4 不和存档系统冲突

新字段必须进入 save roundtrip 或可重建。

规则：

```text
integration_status 必须存。
population_cache 可重建，不必存。
state population profile 是 content 数据，不进存档。
运行期改变后的 integration/status 必须进存档。
```


## 15. 测试矩阵

| 测试 | 目标 |
|---|---|
| `state_population_1936_loads` | 州人口 RON 加载成功 |
| `state_population_ids_resolve` | 所有 state_id 可映射到 World StateId |
| `country_population_sums_owned_states` | 国家人口由 owned states POP 合计 |
| `transfer_state_moves_population` | 割让州后人口随州转移 |
| `eng_domestic_vs_imperial_population` | 英国本土人口和帝国人口分离 |
| `fra_colonial_population_not_domestic` | 法国殖民人口不进本土人口 |
| `industrial_buildings_prefer_metropole` | 现代工业不被殖民地人口吸走 |
| `colonial_resources_enter_market_bloc` | 殖民资源能进入市场圈贡献 |
| `subject_population_not_master_manpower` | subject 人口不直接变宗主国 manpower |
| `colonial_conscription_has_political_cost` | 殖民征兵有政治代价 |
| `save_roundtrip_state_integration` | 州整合状态存读档正确 |
| `gdp_calibration_survives_pop_rework` | 人口州化不破坏 GDP 校准 |


## 16. 验收命令

常用检查：

```text
cargo test -p hoi4-content
cargo test -p hoi4-state
cargo test -p hoi4-logic
cargo test -p hoi4-ui
cargo test -p hoi4-integration smoke_30d
```

阶段性重点：

```text
cargo test -p hoi4-content history_1936_loads
cargo test -p hoi4-state save_roundtrip
cargo test -p hoi4-logic v7_initial_gdp_calibration
cargo test -p hoi4-logic v8_market_bloc_trade
cargo test -p hoi4-logic v8_puppet_colonial_economy
```


## 17. 推荐落地顺序

最小安全顺序：

```text
Phase 0: 保护测试
Phase 1: 州人口数据只加载不生效
Phase 2: 州整合状态进入 World 和 save
Phase 3: POP 改为州人口驱动
Phase 4: UI 和 helper 显示多人口口径
Phase 5: 建筑分配过滤殖民地
Phase 6: 财政/消费/税基修正
Phase 7: 市场圈和殖民资源抽取
Phase 8: 军事人力和殖民兵源
Phase 9: 殖民政治反馈
Phase 10: 数据扩展和历史校准
```

不建议顺序：

```text
先大规模录入全球州人口，但不改建筑过滤。
先把 profile.population 删除，但不做 fallback。
先把殖民地人口算进国家总人口 UI，但不区分 domestic/colonial/imperial。
先做完整产权系统，再做人口口径。
```


## 18. 第一轮可交付目标

第一轮不要追求完整殖民系统，只交付一个稳定底座：

```text
新增 state_population.ron schema 和 loader。
World 有 state integration status。
德国人口改为州 POP 汇总。
英国至少能区分 domestic 和 subject/imperial population。
法国至少能区分 metropole 和 colony status。
现代工业建筑只分配到 metropole/incorporated/core。
现有 GDP 校准测试继续通过。
存档 roundtrip 通过。
```

第一轮成功后，再把殖民资源、市场圈、兵源、政治反馈逐步接上。


## 19. 最终验收标准

本路线图完成后，应满足：

```text
德国人口来自德国拥有州，而不是 GER.ron 写死数字。
英国本土人口、殖民人口、subject 人口、帝国人口能分开显示。
法国殖民人口不会直接放大法国本土工业和税基。
印度/马来亚/加拿大等 subject 不直接成为英国国内人口。
殖民地资源能通过帝国市场或强制贸易影响宗主国。
征兵不会把殖民地和 subject 人口无条件当本土 manpower。
割地、吞并、释放国家后人口自然随州变化。
初始建筑仍符合 V7 GDP/产业结构校准，不被人口重构破坏。
市场、财政、军事、外交、UI 都能解释这些人口口径。
```
