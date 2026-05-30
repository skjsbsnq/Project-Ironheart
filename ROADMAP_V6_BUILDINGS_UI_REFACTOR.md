# Project Ironheart V6.G — 建筑·生产·经济面板重构路线图

> **本文档是 [`ROADMAP_V6_ECONOMY.md`](./ROADMAP_V6_ECONOMY.md) 与 [`ROADMAP_V6_FINISH.md`](./ROADMAP_V6_FINISH.md) 之后的建筑/生产/UI 专项重构路线。**
> 写于 2026-05-22。目标是把当前“V6 经济后端 + 旧 HOI4 生产面板 + 半成品建筑面板”的混杂状态，重构成 **Vic3 风格建筑经济核心**。
> **硬要求**：所有新增/重构后的面板、按钮、表头、提示、建筑名、生产方式名、事件/财政/市场说明，必须提供中文显示；不得新增只显示英文的经济 UI。

---

## 0. 为什么需要这份重构

2026-05-22 用户连续反馈：

- GDP 不增长或增长不可解释。
- 建筑系统“不知道怎么新建筑”。
- 建造队列、建筑列表、生产线互相混杂。
- 军费被硬编码。
- 法团经济/MEFO 军工需求没有形成“政府持续购买军工品，赤字由 MEFO 覆盖”的真实闭环。
- 生产/建筑面板需要像 Victoria 3，而不是 HOI4 生产线。

审计结论：

| 问题类型 | 现状 | 后果 |
|---|---|---|
| 旧 HOI4 生产线残留 | `production.rs` 仍使用 `assigned_factories / efficiency / equipment_id` | 与 V6 建筑经济冲突 |
| 建筑面板伪装 | `ConstructionV6Panel` 显示现役建筑，`progress = 1.0` | 玩家看不到真实建造队列 |
| 建造入口错 ID | 旧按钮下单 `industrial_complex / arms_factory / dockyard / infrastructure` | 与 V6 `buildings.ron` 不匹配 |
| PM 模型过窄 | 每建筑只有 `active_pm: String` | 不能表达 Vic3 式多 PM 组 |
| 经济解释断裂 | 市场、财政、物流、建筑面板互不追踪 | GDP/军费/采购不可解释 |
| 中文化不完整 | 旧按钮、部分提示仍英文或乱码注释 | 新系统体验不统一 |

**根因一句话**：V6 后端已经走向“建筑等级 × POP 雇佣 × 商品市场 × PM”，但 UI 和部分生产/财政逻辑仍停留在“HOI4 工厂槽位 + 生产线”时代。

---

## 1. 范围

### 1.1 范围内

- ✅ 重构建筑与生产面板为 **Vic3 风格建筑经济核心面板**。
- ✅ 废弃旧 HOI4 `ProductionPanel` 的生产线主入口。
- ✅ 建造目录从 `V6Database.buildings` 自动生成，不再硬编码旧建筑 ID。
- ✅ 真实显示 `econ.construction[ci].items` 建造队列。
- ✅ 现有建筑按建筑类型/州聚合显示。
- ✅ 支持建筑扩建、队列重排、取消、PM 切换。
- ✅ 引入 PM 组设计：Base / Secondary / Automation / Ownership / Military / Government。
- ✅ 扩展建筑 runtime 快照，统一供 UI、GDP、财政、市场读取。
- ✅ 财政面板拆分收入/支出分项，明确军人工资、军工采购、维护费、建造品采购、MEFO 支付。
- ✅ 市场面板可反查商品来源和去向。
- ✅ 物流面板显示军工产出、库存、军队补充需求、缺口。
- ✅ 省份/州面板改为州建筑经济详情。
- ✅ 所有新增/重构 UI 必须中文化。

### 1.2 范围外

- ❌ 不重做战争指挥、前线、战斗系统。
- ❌ 不重做地图渲染。
- ❌ 不做完整 Vic3 多国 POP。
- ❌ 不做完整 Vic3 公司系统，法团/Cartel 只做 V6 必需闭环。
- ❌ 不做存档兼容。
- ❌ 不在第一阶段引入复杂私人投资池，先保留政府建造队列，结构预留。
- ❌ 不把装备成品放进商品市场；装备库存仍为 `econ.stockpile[ci]`。

---

## 2. 关键决策点（V6.G 锁定）

| # | 决策点 | 选定方案 |
|---|---|---|
| G1 | 生产系统主模型 | **建筑生产**。生产由建筑 + PM + 雇佣 + 商品输入决定，不再由 HOI4 工厂生产线决定 |
| G2 | 面板主入口 | 新建/重写为 **建筑面板 BuildingsPanel**，吸收旧 Production + ConstructionV6 的功能 |
| G3 | 建造目录 | 从 `content/economy_v6/buildings/buildings.ron` 自动读取 |
| G4 | 建造队列 | 使用 `econ.construction[ci].items` 作为唯一真实队列 |
| G5 | PM 模型 | 每建筑支持多个 PM 组，而不是单个 `active_pm` |
| G6 | GDP 来源 | 从真实建筑流量/成交/工资/采购累计，不再只按理论产能重算 |
| G7 | 军费 | 不允许硬编码 `500_000 × division_count`；改为军人工资 + 军工品补充采购 + 维护费 |
| G8 | 法团经济 | 德国法团战备经济下，军工品“生产出来就有政府订单”，赤字由 MEFO 自动覆盖 |
| G9 | 中文化 | 新面板、新按钮、新 tooltip、新 RON 内容必须有中文；不接受英文-only UI |
| G10 | 旧生产线 | `EconomyState.production` 进入 legacy，第一阶段停止 UI 依赖，后续删除 |

---

## 3. 当前面板审计

### 3.1 必须重构的 6 个面板

| 面板 | 文件 | 问题 | 目标 |
|---|---|---|---|
| 生产面板 | `crates/hoi4-ui/src/production.rs` | 旧 HOI4 生产线模型 | 废弃主入口，功能并入建筑面板 |
| 建筑/建造面板 | `crates/hoi4-ui/src/construction_v6_panel.rs` | 显示现有建筑伪装队列 | 重写为 Vic3 风格建筑/建造面板 |
| 市场面板 | `crates/hoi4-ui/src/market_panel.rs` | 只读供需条，不能解释来源/去向 | 商品可反查生产/消费/采购 |
| 财政面板 | `crates/hoi4-ui/src/finance_panel.rs` | 只有总收支，预算不可解释 | 收支分项 + MEFO + 军费明细 |
| 物流面板 | `crates/hoi4-ui/src/logistics_panel.rs` | 库存和旧生产线混杂 | 装备库存、日产、军队需求、缺口 |
| 省份信息 | `crates/hoi4-ui/src/province_info.rs` | 仍显示 owner production lines | 改为州建筑经济详情 |

### 3.2 需要联动改造的 5 个面板

| 面板 | 文件 | 联动点 |
|---|---|---|
| 顶栏 | `topbar.rs` | GDP 增速不能硬编码 0；显示建造力/预算风险/MEFO 风险 |
| 国家信息 | `country_info_panel.rs` | civ/mil/dock 改为 GDP、工业等级、军工等级、建造力 |
| 资源面板 | `resource_panel.rs` | 与 Logistics/Market 去重，保留 quick popup |
| 贸易面板 | `trade_panel.rs` | 贸易影响建筑输入、价格、外汇、封锁 |
| 法律面板 | `law_panel.rs` | 法律显示经济效果预览，特别是法团战备经济 |

### 3.3 可暂缓面板

| 面板 | 文件 | 处理 |
|---|---|---|
| 科研 | `research.rs` | 后续显示解锁建筑/PM，第一阶段可暂缓 |
| 政治 | `politics.rs` | 决议效果预览后续做 |
| 军事 | `military.rs` | 后续联动军费/装备缺口 |
| 局势 | `situation_panel.rs` | 暂缓 |
| 外交 | `diplomacy.rs` | 暂缓 |
| 国策 | `focus_tree_panel.rs` | 后端改 V6 建筑 effect，UI 暂缓 |
| 设置/存档/事件/结算 | 多文件 | 暂缓或轻量字段替换 |

---

## 4. 目标 UI 模型（Vic3 风格）

### 4.1 建筑面板总体布局

新面板建议命名：

```rust
BuildingsPanel
```

中文标题：

```text
建筑与生产
```

结构：

```text
顶部摘要
------------------------------------------------
GDP | GDP增速 | 建造力 | 建造开支 | 失业率 | 军工订单 | MEFO风险

标签页
------------------------------------------------
全部 / 城市工业 / 资源与农业 / 基础设施 / 军工 / 政府建筑 / 建造队列

左侧：建筑类型列表
------------------------------------------------
钢铁厂 Lv 42 | 就业 84% | 盈利 +12.4M RM/周 | 产出 钢 +420 | 缺口 煤
兵工厂 Lv 18 | 就业 91% | 政府订单充足 | 产出 步兵装备 +81/日

右侧：选中建筑详情
------------------------------------------------
州分布、PM 组、投入/产出、就业、工资、利润、扩建按钮
```

### 4.2 建筑类型行

每一行必须显示：

```text
建筑名
全国等级
就业率
盈利/亏损
主要产出
主要输入
短缺警告
PM 概览
扩建按钮
```

示例：

```text
钢铁厂  Lv 42  就业 84%  盈利 +12.4M RM/周
产出：钢材 +420/d
输入：煤 -180/d、铁矿 -160/d
警告：煤炭短缺 12%
[展开] [全国 PM] [+ 扩建]
```

### 4.3 州建筑行

展开建筑类型后：

```text
莱茵兰  Lv 8  就业 91%  盈利 +2.1M RM/周  PM：平炉炼钢
萨克森  Lv 5  就业 77%  盈利 +0.4M RM/周  警告：缺工人
```

按钮：

```text
[+] 扩建一级
[-] 缩减一级（后续）
[PM] 调整生产方式
[定位]
```

### 4.4 建造队列页

中文标题：

```text
建造队列
```

显示：

```text
建造力：125 / 125
建造品开支：1.4M RM/日
建设公司工资：0.3M RM/日
预计完成：第一项 38 天
```

队列项：

```text
1. 莱茵兰：钢铁厂 Lv 8 -> 9 | 42% | 预计 38 天
2. 萨克森：兵工厂 Lv 2 -> 3 | 12% | 预计 91 天
```

按钮：

```text
[上移] [下移] [暂停] [取消]
```

### 4.5 可建建筑目录

从 `db.buildings` 自动生成，按中文分类：

```text
资源与农业：煤矿、铁矿、粮食农场、牧场
城市工业：钢铁厂、机械厂、化工厂、电气工厂
民生工业：纺织厂、家具厂、酿酒厂
基础设施：铁路、港口、发电厂、建设公司
军工：兵工厂、弹药厂、坦克厂、飞机制造厂、造船厂、合成炼油厂
军事基地：空军基地、海军基地、防空阵地、雷达站、地堡、征兵站
政府与服务：大学、电报局、银行
```

锁定项必须显示中文原因：

```text
银行：需要经济法「自由放任」
建设公司：需要科技「城市规划」
坦克厂：需要科技「焊接装甲」
```

---

## 5. 内容中文化硬要求

### 5.1 UI 文本

所有新增 key 必须写入：

```text
crates/hoi4-ui/src/i18n.rs
```

每条必须包含英文和中文，但 UI 默认中文必须可读。

新增 key 示例：

```rust
("buildings_panel_title", ["Buildings & Production", "建筑与生产"]),
("construction_queue", ["Construction Queue", "建造队列"]),
("buildable_buildings", ["Buildable Buildings", "可建建筑"]),
("building_profit_weekly", ["Weekly Balance", "每周收支"]),
("employment_rate", ["Employment", "就业率"]),
("input_shortage", ["Input Shortage", "投入品短缺"]),
("government_orders", ["Government Orders", "政府订单"]),
("mefo_risk", ["MEFO Risk", "梅福票据风险"]),
```

### 5.2 RON 内容

以下 RON 内容必须中文名完整：

```text
content/economy_v6/buildings/buildings.ron
content/economy_v6/production_methods/*.ron
content/economy_v6/goods.ron
content/economy_v6/laws/*.ron
content/economy_v6/events_v6/*.ron
content/economy_v6/technologies/*.ron
```

要求：

- 建筑 `name` 必须中文。
- PM `name` 必须中文。
- 法律 `name` 必须中文。
- 事件 `title/body/options` 必须中文。
- 新 tooltip/原因文本必须中文。

### 5.3 禁止项

- ❌ 新增按钮只写 `+ Add Line`
- ❌ 新增弹窗只写 `Select Equipment`
- ❌ 新增建筑名只写 `Truck Factory`
- ❌ 新增 PM 只写 `Basic Production`
- ❌ 新增错误提示只写英文
- ❌ 新增乱码注释

---

## 6. 数据模型重构

### 6.1 建筑实例

当前：

```rust
pub struct Building {
    pub kind: BuildingKind,
    pub building_def_id: String,
    pub state: StateId,
    pub level: u8,
    pub active_pm: String,
    pub employment: [u32; 6],
    pub owner: BuildingOwner,
}
```

目标：

```rust
pub struct Building {
    pub kind: BuildingKind,
    pub building_def_id: String,
    pub state: StateId,
    pub level: u16,
    pub active_pm_by_group: Vec<ActiveProductionMethod>,
    pub employment: [u32; 6],
    pub owner: BuildingOwner,
    pub requires_law: Option<(LawCategory, String)>,
}
```

```rust
pub struct ActiveProductionMethod {
    pub group: String,
    pub pm_id: String,
}
```

第一阶段可保留 `active_pm: String`，但必须新增兼容函数：

```rust
active_pms_for_building(building, db) -> Vec<&ProductionMethodDef>
```

禁止 UI 继续直接假设一个建筑只有一个 PM。

### 6.2 建筑定义

当前：

```rust
pub struct BuildingDef {
    pub id: String,
    pub name: String,
    pub kind: BuildingKindDef,
    pub max_level: u8,
    pub owner_default: OwnerDef,
    pub requires_law: Option<(LawCategoryDef, String)>,
}
```

目标新增字段：

```rust
pub group: String,
pub construction_cost: f32,
pub buildable: bool,
pub is_government_funded: bool,
pub state_limit_kind: Option<String>,
pub urbanization: f32,
```

示例：

```ron
(
    id: "steel_mill",
    name: "钢铁厂",
    kind: Industrial,
    group: "城市工业",
    max_level: 15,
    construction_cost: 600.0,
    owner_default: Private,
    buildable: true,
    is_government_funded: false,
    requires_law: None,
)
```

### 6.3 生产方式定义

当前：

```rust
pub struct ProductionMethodDef {
    pub id: String,
    pub name: String,
    pub building_id: String,
    pub input_good_ids: Vec<String>,
    pub input_good_amounts: Vec<f32>,
    pub output_good_ids: Vec<String>,
    pub output_good_amounts: Vec<f32>,
    pub employment_demand: [u32; 6],
    pub unlocked_by: Option<String>,
    pub equipment_output: Option<EquipmentOutputDef>,
}
```

目标新增：

```rust
pub group: String,
pub group_name: String,
pub required_law: Option<(LawCategoryDef, String)>,
pub throughput_modifier: f32,
pub automation_modifier: f32,
```

PM 组示例：

| 组 ID | 中文名 | 说明 |
|---|---|---|
| base | 基础工艺 | 主输入/主产出 |
| secondary | 副产物 | 改变副产品结构 |
| automation | 自动化 | 减少工人、增加机械/电力消耗 |
| ownership | 所有制 | 私营/国营/法团 |
| military | 军工型号 | mk1/mk2/mk3 或装备类别 |
| government | 政府职能 | 大学/电报局/征兵站等 |

### 6.4 建筑 runtime 快照

新增：

```rust
pub struct BuildingRuntime {
    pub building_idx: usize,
    pub input_goods: Vec<GoodFlow>,
    pub output_goods: Vec<GoodFlow>,
    pub equipment_output: Vec<EquipmentFlow>,
    pub workforce_required: [u32; 6],
    pub workforce_employed: [u32; 6],
    pub employment_ratio: f32,
    pub input_fulfillment: f32,
    pub throughput: f32,
    pub revenue_rm: f64,
    pub input_cost_rm: f64,
    pub wage_cost_rm: f64,
    pub profit_rm: f64,
    pub productivity_rm: f64,
    pub weekly_balance_rm: f64,
    pub government_order_rm: f64,
}
```

UI、财政、GDP、市场都优先读 runtime，禁止各面板各算各的。

---

## 7. 经济逻辑重构

### 7.1 统一建筑生产流水线

当前 `market_tick.rs` 与 `planned_tick.rs` 复制生产逻辑。目标改为：

```text
1. reset daily flows
2. resolve active PMs
3. validate law/tech requirements
4. hire/fire workforce
5. compute input demand
6. buy/consume inputs
7. compute output by employment × input_fulfillment × throughput
8. write market supply / equipment stockpile
9. resolve government orders
10. pay wages/dividends
11. collect taxes
12. run construction
13. update GDP from actual flows
14. update UI runtime snapshot
```

市场经济和计划经济只差：

| 模块 | 市场经济 | 计划经济 |
|---|---|---|
| 价格 | 供需 EMA | 计划价/配给 |
| 产出目标 | 利润驱动 | 配额驱动 |
| 工资/税收 | 市场工资 + 税 | 国家工资 + 国营利润 |
| 消费 | POP 市场购买 | 配给实现率 |
| PM | 玩家/企业选择 | 计划强制 PM |

### 7.2 GDP 改造

禁止仅按理论产能重算：

```rust
gdp_rm = sum(pm.output_amount * price) * 365 * scale
```

目标：

```text
GDP 日流量 =
  建筑实际增加值
+ 政府采购成交额
+ 建造活动增加值
+ 服务/消费成交额
```

每周/月平滑成年化 GDP。

### 7.3 建造改造

建设公司必须是经济实体：

```text
建设公司雇佣 POP
建设公司消耗建材
政府支付建设工资
政府购买建设品
建设力来自建设公司等级与 PM
```

第一阶段最小实现：

- `construction_sector` 提供 CP。
- CP 消耗 `machinery / steel / cement-like goods`。
- 买不到建材则建造速度下降。
- 建造开支进入财政支出。
- 建造活动进入 GDP。

### 7.4 军费改造

当前硬编码必须删除：

```rust
let upkeep_per_division: f64 = 500_000.0;
```

目标军费：

```text
军费 =
  军人工资
+ 装备补充采购
+ 日常维护费
+ 训练消耗
+ 空军/海军维护
```

法团经济下：

```text
军工厂产出军工品
政府形成持续订单
现金不足也可成交为政府采购赤字
日末自动 MEFO 覆盖赤字
MEFO 债务增加
```

---

## 8. 面板重构清单

### 8.1 G0 — UI 入口清理（P0）✅ 已完成

| 子任务 | 问题 | 修复 |
|---|---|---|
| ✅ G0.a | `ProductionPanel` 与 V6 建筑经济冲突 | 从主入口隐藏或标记 legacy |
| ✅ G0.b | `ConstructionV6Panel` 名义是建造，实际显示现有建筑 | 改名/重写为 `BuildingsPanel` |
| ✅ G0.c | `main.rs::InGamePanel` 同时有 `Production` 和 `ConstructionV6` | 第一阶段保留枚举，入口统一指向新建筑面板 |
| ✅ G0.d | 旧按钮 hardcode 非 V6 id | 建造目录从 `db.buildings` 自动生成 |
| ✅ G0.e | 中文 key 不完整 | 新 UI key 全部补进 `i18n.rs` |

**验收**：

- ✅ 游戏内能打开“建筑与生产”面板。
- ✅ 面板里没有 `industrial_complex / arms_factory / dockyard / infrastructure` 旧 ID。
- ✅ 新增按钮、标题、tooltip 均有中文。
- ✅ 旧 `+ Add Line / Select Equipment` 不再作为主要经济入口出现。

### 8.2 G1 — 真建造队列（P0）✅ 已完成

| 子任务 | 问题 | 修复 |
|---|---|---|
| ✅ G1.a | 建造面板不显示 `econ.construction` | 显示真实队列 |
| ✅ G1.b | MoveUp/MoveDown/Remove 是 TODO | 接到 `econ.construction[player].items` |
| ✅ G1.c | 队列项缺州名/目标等级/预计天数 | 补 `ConstructionQueueEntry` UI 数据 |
| ✅ G1.d | 建造力显示算法不一致 | 使用统一 `construction_cp_pool()` |
| ✅ G1.e | 队列取消无中文确认 | 增加中文按钮和提示 |

**验收**：

- ✅ 玩家点击“钢铁厂”并选择州后，队列出现“钢铁厂 Lv N -> N+1”。
- ✅ 上移/下移/取消真实改变队列。
- ✅ tick 后进度增长。
- ✅ 完工后现有建筑等级增加。
- ✅ 队列 UI 显示中文州名、中文建筑名。

### 8.3 G2 — 可建建筑目录（P0）✅ 已完成

| 子任务 | 问题 | 修复 |
|---|---|---|
| ✅ G2.a | 可建建筑 hardcode | 读取 `db.buildings` |
| ✅ G2.b | 无分类 | 按建筑 group/kind 中文分类 |
| ✅ G2.c | 无锁定原因 | 显示法律/科技/州限制中文原因 |
| ✅ G2.d | 无批量建造 | 支持连续点击州下单 |
| ✅ G2.e | 无扩建入口 | 现有建筑行增加“扩建”按钮 |

**验收**：

- ✅ 所有 `buildings.ron` 中 `buildable = true` 的建筑出现在目录。
- ✅ 建筑名中文。
- ✅ 法律锁定显示中文原因。
- ✅ 科技锁定显示中文原因。
- ✅ 点“扩建”能加入真实队列。

### 8.4 G3 — 建筑列表与州详情（P0）✅

| 子任务 | 问题 | 修复 |
|---|---|---|
| ✅ G3.a | 现有建筑只是平铺 | 按建筑类型聚合 |
| ✅ G3.b | 无州展开 | 可展开到州 |
| ✅ G3.c | 盈利/就业不可解释 | 读 `BuildingRuntime` |
| ✅ G3.d | 缺口不可见 | 显示输入品短缺/劳动力短缺 |
| ✅ G3.e | 省份信息仍显示生产线 | 改为州建筑详情 |

**验收**：

- ✅ 钢铁厂全国行显示总等级、就业率、利润、主要产出。
- ✅ 展开后显示各州钢铁厂。
- ✅ 输入品短缺时有中文警告。
- ✅ 省份信息不再显示旧生产线。
- ✅ 省份/州面板显示本州建筑和施工项目。

### 8.5 G4 — PM 组（P1）✅ 已完成

| 子任务 | 问题 | 修复 |
|---|---|---|
| ✅ G4.a | `active_pm` 单槽 | 引入 `active_pm_by_group` |
| ✅ G4.b | RON 无 PM group | `ProductionMethodDef` 增加 group |
| ✅ G4.c | UI 只能切一个 PM | PM 弹窗按组显示 |
| ✅ G4.d | 无预测 | 显示输入/输出/就业/利润预估 |
| ✅ G4.e | requirements 不统一 | 科技/法律锁定统一检查 |

**验收**：

- ✅ 钢铁厂可同时选择“基础工艺”和“自动化”。
- ✅ 军工厂可选择“军工型号”。
- ✅ 切 PM 后下 tick 产出/雇佣变化。
- ✅ 锁定 PM 显示中文原因。
- ✅ 全国批量 PM 切换可用。

### 8.6 G5 — 财政面板分项（P1）

| 子任务 | 问题 | 修复 |
|---|---|---|
| G5.a | 总收支不可解释 | 增加预算分项 |
| G5.b | 军费硬编码 | 拆成军人工资/采购/维护 |
| G5.c | 建造开支不清 | 显示建造品采购/建设工资 |
| G5.d | MEFO 与军购不绑定 | 显示当日 MEFO 覆盖额 |
| G5.e | 中文说明不足 | 所有预算项中文 |

**验收**：

- ✅ 财政面板能看到“军人工资”“军工采购”“军队维护”。
- ✅ 建造时能看到“建造品采购”“建设工资”。
- ✅ 法团经济军购赤字能转为 MEFO。
- ✅ `daily_income_rm / daily_expense_rm` 分项合计等于总额。
- ✅ 所有分项中文显示。

### 8.7 G6 — 市场面板反查（P1）✅ 已完成

| 子任务 | 问题 | 修复 |
|---|---|---|
| ✅ G6.a | 商品只显示供需 | 增加生产/消费来源 |
| ✅ G6.b | 建筑短缺不可追踪 | 商品详情列消费建筑 |
| ✅ G6.c | 政府采购不可见 | 显示政府订单 |
| ✅ G6.d | 建造消耗不可见 | 显示建设部门需求 |
| ✅ G6.e | 贸易影响不清 | 联动进出口与封锁 |

**验收**：

- ✅ 点击钢材能看到生产钢材的钢铁厂。
- ✅ 点击小型武器能看到政府军购需求。
- ✅ 点击机械能看到建设公司/工厂输入需求。
- ✅ 商品短缺能反向定位受影响建筑。
- ✅ 商品详情中文显示。

### 8.8 G7 — 物流面板军工闭环（P1）

| 子任务 | 问题 | 修复 |
|---|---|---|
| ✅ G7.a | 日产来源不清 | 从军工建筑 runtime 统计 |
| ✅ G7.b | 军队需求不清 | 显示补充/训练/维护需求 |
| ✅ G7.c | 缺口不清 | 显示缺口和预计耗尽天数 |
| ✅ G7.d | 军工采购不可见 | 显示政府采购金额 |
| ✅ G7.e | 资源面板重复 | 资源 quick view 与物流去重 |

**验收**：

- ✅ 步兵装备显示库存、日产、军队补充、训练消耗。
- ✅ 坦克/飞机/火炮等 12 类装备全部显示。
- ✅ 缺口红色中文提示。
- ✅ 日产等于军工建筑实际产出。
- ✅ 不再依赖旧 `ProductionLine`。

### 8.9 G8 — 法律/贸易/科研联动（P2）

| 子任务 | 问题 | 修复 |
|---|---|---|
| ✅ G8.a | 法律无经济预览 | 法律面板显示建筑/工资/采购/MEFO影响 |
| ✅ G8.b | 贸易不解释短缺 | 贸易面板显示进口失败影响哪些建筑 |
| ✅ G8.c | 科技解锁不清 | 科研面板显示解锁建筑/PM |
| ✅ G8.d | 国家信息旧工业统计 | 国家信息改 GDP/工业等级/军工等级 |
| ✅ G8.e | 顶栏增速硬编码 | 接真实 GDP 增速 |

**验收**：

- ✅ 法团战备经济法律显示“军工政府订单、MEFO 自动融资”等中文效果。
- ✅ 贸易封锁时能看到哪些商品/建筑受影响。
- ✅ 科技完成后新建筑/PM 出现在建筑面板。
- ✅ 顶栏 GDP 增速不再是 0。
- ✅ 国家信息不再用 civ/mil/dock 作为主要经济指标。

---

## 9. 文件级实施计划

### 9.1 第一阶段必改文件

```text
crates/hoi4-ui/src/construction_v6_panel.rs
crates/hoi4-ui/src/production.rs
crates/hoi4-ui/src/province_info.rs
crates/hoi4-ui/src/topbar.rs
crates/hoi4-ui/src/i18n.rs
crates/hoi4-app/src/main.rs
```

目标：

- 建筑与生产主入口可用。
- 真实建造队列可见。
- 可建目录自动化。
- 中文完整。

### 9.2 第二阶段必改文件

```text
crates/hoi4-ui/src/finance_panel.rs
crates/hoi4-ui/src/market_panel.rs
crates/hoi4-ui/src/logistics_panel.rs
crates/hoi4-ui/src/trade_panel.rs
crates/hoi4-ui/src/law_panel.rs
```

目标：

- 财政、市场、物流能解释建筑经济。
- 军费与 MEFO 可见。
- 贸易/法律影响可见。

### 9.3 后端关键文件

```text
crates/hoi4-state/src/buildings_v6.rs
crates/hoi4-logic/src/economy/mod.rs
crates/hoi4-logic/src/economy/building_runtime.rs
crates/hoi4-logic/src/economy/construction_tick.rs
crates/hoi4-logic/src/economy/market_tick.rs
crates/hoi4-logic/src/economy/planned_tick.rs
crates/hoi4-logic/src/economy/finance_tick.rs
crates/hoi4-content/src/v6_loader.rs
```

目标：

- 多 PM 组。
- 建筑 runtime。
- 真建造开支。
- 真 GDP 流量。
- 真军费。
- 法团军购/MEFO 闭环。

### 9.4 内容文件

```text
crates/hoi4-content/content/economy_v6/buildings/buildings.ron
crates/hoi4-content/content/economy_v6/production_methods/*.ron
crates/hoi4-content/content/economy_v6/goods.ron
crates/hoi4-content/content/economy_v6/laws/*.ron
crates/hoi4-content/content/economy_v6/technologies/*.ron
```

目标：

- 建筑分类中文。
- PM 组中文。
- 解锁条件中文可解释。
- 数值能支撑 1936-1938 德国回放。

---

## 10. 验收门槛

### 10.1 UI 验收

- ✅ 游戏内打开“建筑与生产”面板。
- ✅ 面板至少包含：建筑目录、建造队列、现有建筑列表。
- ✅ 所有新增文本中文可读。
- ✅ 无英文-only 新按钮。
- ✅ 无乱码新增注释。
- ✅ 可建建筑从 RON 自动读取。
- ✅ 建造队列可重排/取消。
- ✅ 完工后建筑等级变化。
- ✅ 省份信息显示州建筑详情。

### 10.2 经济验收

- ✅ 新建钢铁厂后，完成后钢材实际产出增加。
- ✅ GDP 在建筑投产后出现可解释变化。
- ✅ 建造期间财政出现建造开支。
- ✅ 建设公司/建材短缺会影响建造速度。
- ✅ 军工厂产出进入装备库存。
- ✅ 军费不再是硬编码师数量乘常数。
- ✅ 法团经济下军工政府订单持续存在。
- ✅ MEFO 自动覆盖法团军购导致的赤字。
- ✅ 财政面板能解释军费来源。

### 10.3 回归测试

新增或修复测试：

```text
buildings_panel_uses_real_construction_queue
buildable_catalog_uses_v6_database_buildings
construction_queue_reorder_and_cancel
completed_construction_increases_building_level
building_pm_switch_changes_output
gdp_changes_after_new_building_produces
military_spending_not_hardcoded_by_division_count
corporatist_procurement_creates_mefo_financed_orders
finance_budget_breakdown_sums_to_daily_totals
market_good_detail_links_producers_and_consumers
```

---

## 11. 拒收清单

以下情况不允许标完成：

- ❌ 建筑面板仍显示假进度 `progress = 1.0` 当建造队列。
- ❌ 建造按钮仍使用 `industrial_complex / arms_factory / dockyard / infrastructure`。
- ❌ 新面板按钮仍是 `+ Add Line / Select Equipment` 英文。
- ❌ GDP 增速仍硬编码 `0.0`。
- ❌ 军费仍是 `division_count × 常数`。
- ❌ `ProductionPanel` 仍作为主要生产入口。
- ❌ 省份面板仍显示旧 production lines。
- ❌ 市场面板不能解释商品由哪些建筑生产/消费。
- ❌ 财政面板不能拆出军人工资、军工采购、维护费。
- ❌ 新 RON 建筑/PM 没中文名。
- ❌ 新增 UI 文本不进 `i18n.rs`。
- ❌ 只做 UI，不接真实 `econ.construction` / `buildings_v6` / runtime 数据。

---

## 12. 推荐里程碑

### V6.G0 — 面板入口止血

目标：

- 停止旧 Production 面板误导玩家。
- 新“建筑与生产”入口可打开。
- 可建建筑目录来自 RON。
- 真实建造队列可见。

完成条件：

- 玩家知道怎么新建建筑。
- 队列不是假的。
- 中文完整。

### V6.G1 — 建筑经济可解释

目标：

- 建筑列表按类型/州聚合。
- 显示就业、投入、产出、盈利。
- PM 切换有效。
- 省份面板显示州经济。

完成条件：

- 玩家能理解“这个建筑为什么赚钱/亏钱/停产”。

### V6.G2 — 财政/市场/物流闭环

目标：

- 财政能解释建造、军费、军购、MEFO。
- 市场能解释商品来源/去向。
- 物流能解释装备生产/消耗/缺口。

完成条件：

- 玩家能追踪“军工厂生产 -> 政府购买 -> MEFO 支出 -> 装备库存 -> 军队补充”。

### V6.G3 — 多 PM 组与长期扩展

目标：

- 实现 Vic3 式 PM 组。
- 科技/法律解锁 PM。
- 批量 PM 切换。
- 计划经济强制 PM 与配额联动。

完成条件：

- 建筑系统具备长期扩展基础，不再回到 HOI4 工厂槽位。

---

## 13. 最小首轮实施建议

第一轮只做这 5 件，避免大爆炸：

1. 重写 `ConstructionV6Panel` 为“建筑与生产”面板。
2. 可建目录从 `db.buildings` 自动生成。
3. 真实显示并操作 `econ.construction[player].items`。
4. 现有建筑按类型/州显示，保留单 PM 切换。
5. 全部新增 UI 文本进 `i18n.rs`，中文可读。

第一轮完成后，再进入财政/市场/物流闭环。这样风险最低，也能立刻解决“怎么新建筑”和“面板不像 Vic3”的核心体验问题。
