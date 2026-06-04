# 1936 经济、建造力、生产链、POP、财政、建筑与 V9 UI 完整重构路线图

生成日期：2026-06-04

适用项目目录：

```text
C:\Users\19180\Documents\999\b1
```

本文是完整重构路线图兼技术设计文档。`GOAL_1936_ECONOMY_PRODUCTION_UI_REWORK_ZH.md` 只能作为历史背景资料，不能作为完成度判定依据。阶段完成度必须以实际源码、测试和玩家可见 DTO 为准：

```text
GOAL_1936_ECONOMY_PRODUCTION_UI_REWORK_ZH.md
```

## 0. 总目标

本次重构目标是把当前 V6/V7 经济系统重做为一套符合 1936 年开局、可解释、可验证、玩家能通过面板看见并操作的经济模拟系统。

最终必须达到：

1. `main.rs` 不再承载经济、建造、生产链、POP、财政和面板 DTO 的主体逻辑。
2. 1936 年开局地图上所有存在国家，都必须有经济、人口、建筑、财政和基础产业链档案。
3. 国家总人口不得先注入全国人口数字，必须由该国拥有州的人口汇总得到。
4. GDP 不得继续作为预注入主数据驱动建筑生成，必须由一产、二产、三产建筑运行、就业、服务、贸易、财政活动计算得到。
5. 建筑系统必须显式区分一产建筑、二产建筑、三产建筑，并保留基础设施、军事基地、政府建筑、军工建筑等 gameplay 分类。
6. 建造力系统必须重做，不能继续是全国固定 CP 加建造部门等级、且只推进队列首项的模型。
7. 生产链必须成为一等领域模型，不能继续由 UI 或 `main.rs` 临时从生产方法字符串反推。
8. 所有相关系统必须有 V9 面板，且必须有合理二级面板或主从详情结构，不能全部堆在一级面板。
9. 玩家可见地名、建筑名、商品名、国家名、州名、省份名、提示文本必须是中文。禁止向玩家显示 `state_xxx`、`STATE_xxx`、`State 12`、省份数字、内部 id 或乱码。
10. 项目运行环境按 Windows 11 + PowerShell + 中文环境处理，所有新增中文内容必须防止编码损坏。

## 1. Windows 11 PowerShell 中文编码硬规则

本项目当前已有大量中文文档和 RON 内容在 PowerShell 输出中呈现乱码。后续重构必须把编码问题当成工程约束，而不是显示小问题。

### 1.1 必须使用 UTF-8

所有新增或重写的中文文件必须使用 UTF-8 编码。

适用文件包括：

- `.md`
- `.ron`
- `.toml`
- `.rs`
- 本地化文件
- 历史档案文件
- 面板文案数据文件

禁止新增 GBK、ANSI、系统默认编码文件。

### 1.2 PowerShell 执行规则

在 Windows 11 PowerShell 中文环境下执行中文相关命令时，推荐先设置：

```powershell
[Console]::InputEncoding = [System.Text.UTF8Encoding]::new()
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new()
$OutputEncoding = [System.Text.UTF8Encoding]::new()
chcp 65001
```

如果只需要读取文件，不要用会重新写入文件的命令尝试“修复显示乱码”。

禁止用以下方式批量重写中文文件：

```powershell
Get-Content xxx | Set-Content xxx
```

原因：这类命令容易在默认编码、换行和 BOM 行为上制造二次损坏。

### 1.3 中文注释和排版规则

1. Rust 源码中文注释必须简短，优先说明领域规则，不写长段历史解释。
2. 文档可以大量使用中文，但每个标题、列表、代码块必须排版清楚。
3. 面板显示文本不应写在 Rust 逻辑中硬编码散落，应进入统一本地化或 DTO 文案层。
4. RON 中的中文名称必须经过一次可读取性检查，不能出现 `鐢垫姤灞€` 这类乱码。
5. 不得把中文名同时写在多个来源中。必须定义单一权威来源。

### 1.4 编码验收

任一阶段完成时，必须检查：

```powershell
rg -n "鐢|绋|鏂|鍥|€|�|STATE_|state_|State [0-9]|省份 [0-9]" .
```

说明：

- 该命令不是最终判定，只是快速发现明显乱码和内部地名泄露。
- `STATE_` 在解析器测试、原始 HOI4 数据兼容层中可以存在，但不得出现在玩家 UI DTO 和玩家可见文案中。

## 2. 当前源码事实

### 2.1 `main.rs` 过重

截至 2026-06-04 源码审计，`crates/hoi4-app/src/main.rs` 约 1.25 万行，仍然混合了：

- app 生命周期
- 渲染调度
- 输入处理
- UI 面板路由
- 经济面板 DTO 装配
- 建造命令处理
- 市场与产业链临时数据拼装
- POP 汇总
- 外交和局势 DTO
- 本地化 fallback

重构方向：

```text
main.rs
  只保留 App 编排、窗口生命周期、tick 调度、模块调用。

hoi4-app/src/ui_data/
  负责从 World + EconomyState + V6Database 生成 UI DTO。

hoi4-logic/src/economy/
  负责经济、市场、生产链、建造力、财政、POP 运行规则。

hoi4-content/src/history_1936/
  负责历史档案加载、校验和覆盖率报告。

hoi4-ui/src/
  只负责渲染 DTO，不再扫描 World 推导经济真相。
```

### 2.2 历史经济覆盖不完整

当前历史国家经济档案是手工 include，覆盖国家有限。POP 初始档案也只覆盖少数主要国家。非 profile 国家大量依靠算法 fallback。

必须改为：

- 按 1936 地图实际存在国家建立覆盖清单。
- 每国至少有最低完整经济档案。
- 历史精度可以分层，但不能缺档。
- loader 必须能报告缺档国家、缺人口州、缺建筑州、缺财政档案国家。

### 2.3 POP 注入方向错误

当前存在按国家 profile 目标人口缩放 POP 的路径。新系统必须反过来：

```text
州人口事实
  -> 州内 POP 阶层分布
  -> 国家拥有州人口汇总
  -> 国家总人口
```

国家 profile 可以记录历史目标人口，用于校验误差，不得作为直接覆盖值。

### 2.4 GDP 方向错误

当前开局 GDP 仍带有历史 profile 锚定和校准含义。新方向：

```text
建筑运行增加值
+ POP 收入
+ 政府服务
+ 军工和政府采购
+ 净出口
+ 金融与服务业
= GDP
```

历史 GDP 只能作为校验目标：

```text
computed_gdp_1936 与 historical_reference_gdp 误差 <= 阶段阈值
```

不得再用历史 GDP 直接反推出建筑等级。

### 2.5 建筑分类不满足一产、二产、三产

当前 `BuildingKind` 是 gameplay 分类：

- Resource
- Industrial
- Agriculture
- ConsumerGoods
- Service
- Military
- Infrastructure
- MilitaryBase

新系统必须新增经济部门字段：

```rust
pub enum EconomicSector {
    Primary,
    Secondary,
    Tertiary,
    Government,
    MilitarySupport,
    Infrastructure,
}
```

同时保留 gameplay 分类：

```rust
pub enum BuildingGameplayClass {
    Agriculture,
    ResourceExtraction,
    HeavyIndustry,
    LightIndustry,
    Service,
    MilitaryIndustry,
    Infrastructure,
    MilitaryBase,
    Government,
}
```

一栋建筑必须同时具备：

- 经济部门
- gameplay 分类
- 建造配方
- 生产方法组
- 就业结构
- GDP 增加值计算规则
- 初始历史放置规则
- UI 展示分类

## 3. 目标领域模型

### 3.1 国家经济档案

新增或重构为：

```text
CountryEconomicProfile1936
  tag
  history_tier
  reference_gdp
  reference_population
  fiscal_profile_id
  sector_profile_id
  industrial_profile_id
  agriculture_profile_id
  service_profile_id
  trade_profile_id
  notes
```

说明：

- `reference_gdp` 和 `reference_population` 只做校验。
- 真正人口来自州。
- 真正 GDP 来自运行模型。

### 3.2 州人口档案

```text
StatePopulationProfile1936
  game_state_id
  chinese_name
  total_population
  urbanization
  literacy
  class_distribution
  ethnic_or_cultural_notes
  owner_tag_1936
```

国家人口计算：

```text
country_population = sum(state_population where owner == country)
```

### 3.3 初始建筑档案

每国初始建筑必须来自州和国家特征，不是单纯 GDP 推导。

```text
StateBuildingProfile1936
  game_state_id
  buildings: [
    building_id
    level
    owner
    pm_group
    historical_reason
  ]
```

建筑生成层级：

1. 手写高精度州建筑档案。
2. 国家产业模板按州权重生成。
3. 全球最低完整 fallback。
4. 覆盖率报告列出 fallback 结果。

### 3.4 建造力模型

旧模型：

```text
base_cp + construction_sector_level * 30
只推进队列首项
材料固定为钢 + 机械
```

新模型：

```text
ConstructionCapacity
  national_admin_cp
  construction_sector_cp
  regional_labor_cp
  engineering_equipment_cp
  finance_cp
  material_cp
  idle_cp
  blocked_cp
```

每个项目运行态：

```text
ConstructionProjectRuntime
  project_id
  allocated_cp
  effective_cp
  progress
  estimated_days
  fund_ratio
  material_ratio
  labor_ratio
  infra_ratio
  main_bottleneck
  bottleneck_details
```

队列必须支持：

- 多项目同时推进
- 优先级
- 权重
- 暂停
- 上移/下移
- 自动建设候选解释
- 建造瓶颈解释

### 3.5 建造配方

每种建筑必须有专属配方：

```text
ConstructionRecipe
  building_id
  cp_cost
  money_cost_rm
  materials
  labor_need
  engineering_need
  urban_capacity_need
  coastal_required
  resource_deposit_required
```

示例方向：

- 农场：劳力、工具、少量机械。
- 矿山：劳力、机械、铁路/运输条件。
- 钢铁厂：钢材、机械、电力、城市容量。
- 港口：钢材、机械、海岸条件、较高工程需求。
- 大学：财政、城市容量、文员/受教育人口。
- 军工厂：机械、钢、电气设备、政府投资。

### 3.6 生产链图

新增一等模型：

```text
ProductionChainGraph
  goods nodes
  building nodes
  production method nodes
  demand bucket nodes
  equipment nodes
  edges
```

查询能力：

- 商品由哪些建筑/PM 生产。
- 商品被哪些建筑/POP/军工/建造项目消费。
- 商品短缺影响哪些建筑降产。
- 上游短缺来自哪些商品。
- 应建哪些建筑解决短缺。
- 哪些州适合建设。
- 是否可通过进口、市场圈、殖民地、库存缓冲解决。

UI 不得再只显示：

```text
上游：steel, coal
下游：machinery
```

必须显示：

```text
机械短缺
  主要原因：钢铁供应不足、机械厂产能低、进口被封锁
  影响：建造项目降速、军工厂降产、发动机厂缺料
  可操作：扩建机械厂、扩建钢铁厂、进口钢、暂停低优先级建造
```

## 4. V9 UI 面板体系

### 4.1 一级面板原则

一级面板只作为领域入口，不能承载所有信息。每个一级面板必须有二级 tab 或主从详情。

推荐一级入口：

- 国家
- 经济
- 建设
- 人口
- 市场
- 财政
- 军事生产
- 外交
- 科研
- 局势

### 4.2 经济面板二级结构

```text
经济
  总览
  GDP
  一产
  二产
  三产
  就业
  投资
  贸易影响
  诊断
```

每个二级面板必须可点击到：

- 州
- 建筑
- 商品
- POP 阶层
- 生产链
- 财政来源

### 4.3 建设面板二级结构

```text
建设
  总览
  队列
  建筑目录
  一产建筑
  二产建筑
  三产建筑
  基础设施
  军事设施
  瓶颈
  自动建设
```

建设总览必须显示：

- 总建造力
- 已分配建造力
- 闲置建造力
- 材料阻塞
- 资金阻塞
- 劳力阻塞
- 受影响项目

### 4.4 市场与生产链面板二级结构

```text
市场
  总览
  短缺
  商品
  产业链
  进口出口
  市场圈
  殖民/傀儡供给
  行动建议
```

产业链面板必须从商品表升级为链路图和原因解释。

### 4.5 POP 面板二级结构

```text
人口
  全国
  州人口
  阶层
  就业
  收入
  消费需求
  教育与技能
  满意度
```

必须支持从国家人口点入州人口，再点入州内 POP 阶层。

## 5. 中文地名和本地化

### 5.1 统一名称解析器

新增统一显示名解析层：

```text
DisplayNameResolver
  country_name(tag)
  state_name(state_id)
  province_name(province_id)
  building_name(building_id)
  good_name(good_id)
  pop_class_name(class_id)
```

所有 UI DTO 只能接收解析后的中文名，不得在面板内部拼接内部 id。

### 5.2 地名来源优先级

州名：

1. 手写中文 state 名称表。
2. HOI4 localization 中文键。
3. 英文名转中文映射。
4. 最后 fallback：`未命名州 {game_state_id}`，仅调试模式允许显示。

玩家正常 UI 禁止：

- `STATE_64`
- `state_64`
- `State 64`
- `64`
- `省份 64`

### 5.3 乱码清理

必须清理已存在的乱码内容，尤其是：

- 建筑 RON 中的中文名称和 group。
- 面板中的中文硬编码。
- 文档中被错误解码后重写的中文。
- 推荐文本和 action 文案。

## 6. 分阶段路线图

### Phase 0：只读审计和编码基线

目标：

- 固定当前源码事实。
- 建立编码规则。
- 建立缺档报告。
- 不改运行逻辑。

产物：

- main.rs 经济职责审计。
- 国家/州/建筑/POP/GDP 覆盖率报告。
- 乱码和内部地名泄露报告。
- 本文档和 goal 文档。

验收：

- 文档存在。
- 未修改运行逻辑。
- 能列出所有缺档国家和缺中文名字段。

### Phase 1：拆分 `main.rs` 的经济 DTO 装配

目标：

- 把市场、建设、财政、POP、国家信息 DTO 构建迁出 `main.rs`。
- UI 只吃 DTO。

建议模块：

```text
crates/hoi4-app/src/ui_data/economy.rs
crates/hoi4-app/src/ui_data/construction.rs
crates/hoi4-app/src/ui_data/market.rs
crates/hoi4-app/src/ui_data/pops.rs
crates/hoi4-app/src/ui_data/names.rs
```

验收：

- `main.rs` 行数明显下降。
- 现有面板行为不变。
- DTO 构建函数可单测。

### Phase 2：统一中文名称解析器

目标：

- 建立 `DisplayNameResolver`。
- UI DTO 全部使用中文 display name。
- 清理 `STATE_xxx` 和数字州名泄露。

验收：

- 玩家 UI 不显示内部 state/province id。
- 编码检查不出现新增乱码。
- 地名缺失有报告。

### Phase 3：历史 1936 覆盖层

目标：

- 建立地图存在国家清单。
- 为所有国家生成最低完整经济档案。
- 将国家人口改为州人口汇总。

验收：

- 所有存在国家都有经济、人口、建筑、财政最低档案。
- 国家总人口等于拥有州人口汇总。
- 没有国家因缺 profile 只有空经济。

### Phase 4：建筑 schema 重做

目标：

- 增加经济部门字段。
- 增加建造配方。
- 增加就业结构和 GDP 增加值规则。
- 清理建筑中文名乱码。

验收：

- 所有建筑都有一产/二产/三产或其他明确部门。
- 所有建筑都有 construction recipe。
- UI 可按部门展示建筑。

### Phase 5：建造力系统重做

目标：

- 替换单队首项目推进。
- 支持多项目 CP 分配。
- 支持瓶颈解释。
- 支持材料、资金、劳力、工程能力、地区基础设施影响。

验收：

- 队列多个项目可同时推进。
- 面板显示总 CP、分配 CP、闲置 CP、阻塞 CP。
- 每个项目显示主瓶颈和预计完成时间。
- 建造材料按建筑配方，不再统一钢+机械。

### Phase 6：生产链图重做

目标：

- 建立 `ProductionChainGraph`。
- 市场短缺能解释原因和影响。
- 生产链面板能给可操作入口。

验收：

- 商品短缺能追溯上游。
- 可列出受影响建筑、POP、军工订单、建造项目。
- 可推荐建设、进口、暂停项目等操作。

### Phase 7：GDP、财政、POP 联动重做

目标：

- GDP 从运行模型计算。
- 财政收入来自税制、关税、国企、债务、投资池等。
- POP 收入、就业、消费与建筑运行连接。

验收：

- GDP 分解可见。
- 财政收支可追溯到税种和支出类型。
- POP 满意度能解释商品短缺和收入不足。

### Phase 8：V9 二级面板全面替换

目标：

- 经济、建设、市场、人口、财政面板全部 V9 化。
- 所有重要对象可点击进入二级详情。

验收：

- 没有旧式大杂烩一级面板。
- 玩家能从短缺点到商品、建筑、州、建设入口。
- 玩家能从 GDP 点到产业部门、建筑和州。

### Phase 9：历史校准和回归

目标：

- 用 1936 历史参考值校准但不覆盖模拟值。
- 建立误差阈值。
- 保持游戏性能。

验收：

- 主要国家 GDP、人口、军工、资源大体符合 1936。
- 小国也有完整经济闭环。
- 全量 tick 和 UI 打开性能可接受。

## 6.1 2026-06-04 实际源码审计进度

本节只按实际源码判断，不采信 goal 文档的完成声明。审计范围包括：

- `crates/hoi4-app/src/main.rs`
- `crates/hoi4-app/src/ui_data/*`
- `crates/hoi4-content/src/v6_loader.rs`
- `crates/hoi4-content/src/v7_history_loader.rs`
- `crates/hoi4-content/content/economy_v6/*`
- `crates/hoi4-content/content/history_1936/*`
- `crates/hoi4-logic/src/economy/*`
- `crates/hoi4-state/src/world.rs`
- `crates/hoi4-ui/src/*_panel.rs`
- 相关测试目录

### 总体结论

当前不是从零开始，已经有一批关键实现落地：

- `ui_data/economy.rs`、`construction.rs`、`market.rs`、`pops.rs`、`names.rs` 已存在。
- `DisplayNameResolver` 已存在，能隐藏 `STATE_123`、纯数字州名和缺失省份 id。
- 建筑定义已经有 `economic_sector`、`gameplay_class`、`employment_profile`、`construction_recipe`。
- 建造队列已经支持多项目按权重/优先级分配 CP，并记录资金、材料、劳力、工程、基础设施瓶颈。
- `ProductionChainGraph` 已存在，并有独立测试。
- `World::country_governed_population()` 已经从拥有州人口汇总，`World::state_population()` 已经从 POP groups 汇总。
- GDP 已有运行期分解：建筑一二三产、POP 收入/消费、政府服务、军工采购、净出口、殖民增加值。
- 财政、建设、市场、人口面板均已有 V9 或 V9 化结构的一部分。

但仍未达到最终路线图标准：

- `main.rs` 仍约 1.25 万行，仍保留面板缓存、建设命令处理、GDP fallback、名称显示、后勤市场派生等大量职责。
- 1936 历史经济国家档案仍是手工 `include_str!` 32 个国家；POP 初始档案只有 16 个国家；州人口档案约 127 条，明显不是全地图全州全国家覆盖。
- `v6_loader.rs` 文件头部已存在真实乱码注释，`ui_data/market.rs`、`ui_data/pops.rs`、`ui_data/construction.rs` 仍有 `???`、`??????`、`Source`、`Details`、`Label`、英文状态文本等玩家可见占位。
- 建筑经济部门 enum 只有 `Primary/Secondary/Tertiary`，没有路线图要求的 `Government/MilitarySupport/Infrastructure` 独立部门；基础设施和军事设施目前靠 facility class 从 gameplay kind 派生。
- `ui_data/market.rs` 仍在 DTO builder 中扫描 `World`、建筑、POP、市场和贸易路线临时拼装产业链/短缺解释，未完全把领域规则下沉到 `hoi4-logic`。
- 市场短缺已有原因/影响/行动雏形，但仍有英文和乱码动作文案，且推荐动作主要是“建设生产者”，进口、暂停项目、市场圈、殖民/傀儡供给等操作入口不完整。
- V9 面板已有二级 tab，但不是所有对象都能点击进入二级详情；人口、市场、财政之间的跨面板 drill-down 仍有限。
- 缺少“所有 1936 存在国家/州都有档案”的硬测试；现有测试更多是批次覆盖和核心国家覆盖。

### Phase 完成度审计

| Phase | 实际完成度 | 已完成 | 未完成/风险 |
|---|---:|---|---|
| Phase 0：只读审计和编码基线 | 约 45% | 路线图存在；`DisplayNameResolver` 有缺名报告结构；测试中有部分 `STATE_` 隐藏用例；本节已补入实际源码审计。 | 没有独立覆盖率报告产物；没有自动列出所有缺档国家/州/中文名字段；源码和 DTO 仍有乱码/占位文本；之前文档事实与当前 `main.rs` 行数不一致。 |
| Phase 1：拆分 `main.rs` 的经济 DTO 装配 | 约 55% | `ui_data/economy.rs`、`construction.rs`、`market.rs`、`pops.rs`、`names.rs` 已存在；财政/建设/市场/POP 面板主要 DTO builder 已迁出；部分 builder 有单测。 | `main.rs` 仍约 1.25 万行；仍处理面板缓存、建设命令、建设高亮、GDP fallback、国家/州/省显示、后勤派生；`ui_data` 中仍直接扫描 `World` 推导市场/建筑/POP 规则。 |
| Phase 2：统一中文名称解析器 | 约 35% | `DisplayNameResolver` 已能解析 country/state/province/building/good/pop class；能隐藏 `STATE_123`、纯数字 state 和缺失省份 id；POP/建设/市场 DTO 已部分使用 resolver。 | resolver 多数时候用空中文 catalog 创建，依赖 fallback；玩家可见 DTO 仍有英文、`???`、`Label`、`Source`、`Details`；`PopStateEntry` 仍携带 `state_id`；缺名报告未统一输出为验收报告。 |
| Phase 3：历史 1936 覆盖层 | 约 30% | `Historical1936Database` 存在；有 32 个国家经济档案；有州人口、资源、贸易、军事、国家元首档案；`World` 人口查询已按拥有州 POP 汇总。 | 国家档案仍手工 include，未按实际地图国家自动全覆盖；POP 初始档案只有 16 个国家；州人口约 127 条，非全州覆盖；仍大量依赖 fallback；没有“所有存在国家都有经济/人口/建筑/财政档案”的测试。 |
| Phase 4：建筑 schema 重做 | 约 65% | `BuildingDef` 已有 `economic_sector`、`gameplay_class`、`employment_profile`、`construction_recipe`；`buildings.ron` 中多数建筑已有中文名、部门、配方、就业结构；建设 UI 可按一产/二产/三产、基础设施、军事设施展示。 | `EconomicSectorDef` 只有三类，缺 Government/MilitarySupport/Infrastructure；`BuildingGameplayClassDef` 缺 Government、HeavyIndustry、LightIndustry 等路线图细分；GDP 增加值规则主要来自运行期建筑值而非每建筑 schema 明确规则；`v6_loader.rs` 有乱码注释。 |
| Phase 5：建造力系统重做 | 约 75% | `ConstructionCapacity`、`ConstructionProjectRuntime` 已存在；`construction_tick` 支持多项目同时推进、权重/优先级、暂停、资金/材料/劳力/工程/基础设施瓶颈、预计天数；有多项目分配测试。 | CP 仍以 `BASE_CP_POOL + construction_sector * 30` 为总池基础；没有路线图要求的 `national_admin_cp`、`regional_labor_cp`、`engineering_equipment_cp` 等分项字段；UI 文案仍有英文 funding/source/lock reason；取消、上移/下移、自动建设已部分有命令但需要端到端验收。 |
| Phase 6：生产链图重做 | 约 55% | `ProductionChainGraph` 已存在；能索引商品生产者、消费者、建造配方、需求桶、短缺影响、建设行动；有 `production_chain_graph.rs` 测试；市场 DTO 已使用 `shortage_impact()` 和 `buildable_actions_for_shortage()`。 | 图模型仍偏静态数据库+市场清算索引；UI 短缺解释仍有大量 DTO 侧拼装；推荐动作主要是建生产建筑，进口/暂停项目/市场圈/殖民供给的可操作入口不足；部分市场行动文案乱码或英文。 |
| Phase 7：GDP、财政、POP 联动重做 | 约 60% | `finance_tick` 已计算 GDP 分解并写入 treasury；财政收入包含 POP 税、消费税、企业税、国企利润、债券/MEFO 等；POP 收入、税负、消费满足度、就业与建筑运行有联动；人口汇总来自州 POP。 | 历史 profile 的 GDP/人口仍在 loader 中大量参与初始经济、建筑目标、财政规模和校准；GDP 仍需要证明完全不再反推建筑；POP 面板文案占位严重；缺少 GDP 全局误差阈值和全量回归。 |
| Phase 8：V9 二级面板全面替换 | 约 50% | 财政面板有经济二级 tab；建设面板有总览/队列/目录/一产/二产/三产/基础设施/军事/瓶颈/自动建设；市场面板有总览/短缺/商品/产业链/进出口/市场圈/殖民傀儡/行动建议；POP 面板有 V9 表格和 tab。 | 仍不是所有重要对象可点击 drill-down；市场/POP/建设 DTO 仍有英文/乱码/占位；旧式职责仍留在 `main.rs`；面板之间缺少从短缺到商品、建筑、州、建设入口的完整闭环验收。 |
| Phase 9：历史校准和回归 | 约 25% | GDP 分解有 historical validation 字段；部分测试覆盖历史加载、德国 365 天生产、核心经济公式。 | 没有全 1936 国家 GDP/人口/军工/资源校准表；小国完整经济闭环未证明；全量 tick 和 UI 打开性能验收缺失；历史参考仍可能参与初始化而非只做校验。 |

### 当前最高优先级缺口

1. 先清理玩家可见乱码和英文占位：`ui_data/market.rs`、`ui_data/pops.rs`、`ui_data/construction.rs`、`v6_loader.rs`。
2. 建立真实覆盖率报告：实际 1936 国家列表、32 个经济档案、16 个 POP 初始档案、127 条州人口档案、缺口清单。
3. 把市场短缺解释和行动建议继续下沉到 `hoi4-logic/src/economy/production_chain.rs` 或相邻领域模块，减少 DTO builder 临时推导。
4. 完成 `main.rs` 二次拆分：建设命令、建设高亮、国家/州/省显示、后勤市场派生、GDP fallback。
5. 扩展建筑部门 schema，明确 Government/MilitarySupport/Infrastructure 与 gameplay class 的关系。
6. 补齐全量验收测试：全国家档案、全州人口、玩家 DTO 无内部 id/乱码、GDP 不驱动建筑生成、多项目建造端到端、市场短缺可操作。

## 7. 测试策略

必须新增测试类型：

1. 内容覆盖测试：所有 1936 存在国家有档案。
2. 中文名测试：玩家 DTO 不含 `STATE_`、`state_`、乱码。
3. 人口汇总测试：国家人口等于拥有州 POP 汇总。
4. 建筑 schema 测试：所有建筑有经济部门、配方、就业结构。
5. 建造力测试：多项目队列分配符合预期。
6. 生产链测试：商品上下游和短缺影响可追溯。
7. GDP 测试：GDP 分解加总等于国家 GDP。
8. UI DTO 测试：面板 DTO 不扫描全世界临时推导核心经济规则。

## 8. 明确禁止

1. 禁止继续在 `main.rs` 添加大段经济推导。
2. 禁止 UI 面板直接扫描 `World` 推导生产链。
3. 禁止用 GDP 直接生成建筑等级。
4. 禁止国家先注入总人口再分配到州。
5. 禁止新增玩家可见英文内部 id。
6. 禁止新增乱码中文。
7. 禁止把一产、二产、三产只写在中文 group 文本里。
8. 禁止只有主要国家有完整经济，小国为空。
9. 禁止只改面板外观，不改系统数据模型。
10. 禁止只给短缺商品列表，不给原因链和操作入口。

## 9. 最终完成标准

最终版本必须满足：

```text
all_existing_1936_countries_have_profiles = true
country_population_from_owned_states = true
gdp_from_runtime_economy = true
building_sector_schema_complete = true
construction_multi_project_allocation = true
construction_bottleneck_explainable = true
production_chain_graph_exists = true
market_shortage_actionable = true
v9_secondary_panels_complete = true
player_visible_chinese_names = true
no_player_visible_state_xxx = true
no_new_mojibake_text = true
main_rs_no_economy_dto_bulk = true
```
