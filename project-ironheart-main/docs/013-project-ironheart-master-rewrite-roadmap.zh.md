# Project Ironheart 完整项目重构总路线图

## 0. 本文档地位

本文档是 `project-ironheart/main-rewrite` 分支的项目级总控路线图。其它文档只作为专项说明或历史记录；如果其它文档和本文档冲突，以本文档为准。

这不是“渲染重构方案”“经济重构方案”“UI 重构方案”的拼接，而是一条完整的项目替换路线。执行时必须按本文档阶段顺序推进，不能让 UI、渲染、经济、POP、财政、AI、内容系统各自抢先改同一个接口。

当前结论：

- Project Ironheart 的重构目标是保留完整玩法能力，但重建代码边界、数据契约、渲染管线、V9 UI、经济/人口/财政模型和验收体系。
- 旧主线冻结，只作为行为参考和功能清单来源；不能把旧入口层、旧渲染顺序、旧 UI 面板、旧经济公式直接搬进新线。
- 新线只使用 Project Ironheart / `ironheart-*` 身份，不再使用旧项目命名作为产品身份。
- 先定功能矩阵、数据契约和快照/命令边界，再实现经济、运行时、UI、渲染和面板。否则系统一定会继续互相打架。

## 1. 重构总目标

### 1.1 功能完整一样

重构完成后，新线必须覆盖旧主线已经具备或承诺具备的游戏能力：

- 启动、配置、日志、崩溃报告、国家选择、读取内容、进入游戏。
- 地图浏览、缩放、相机、选择省份、选择部队、前线/箭头/命令交互。
- 时间推进、暂停、速度控制、每日/每周/每月调度。
- 政治、法律、决议、人口、市场、财政、贸易、建设、研究、外交、陆军、海军、空军、后勤、局势、事件、通知、设置、存档和读档。
- 1936 历史数据初始化，包括国家、州、省份、人口、产业、财政、市场、军队、外交关系、内容事件和 AI 初始计划。
- 地图视觉质量、兵牌可读性、UI 可用性、经济账本正确性、性能和确定性 replay。

“功能完整一样”不等于照搬旧代码。旧线只提供功能清单、行为样例和缺陷反证；新线必须按新的模块边界和验收标准重新实现。

### 1.2 必须修复的问题

- 入口层不能再变成巨型文件，不能持有渲染 pass、UI 面板、模拟 tick、经济临时状态和大量开关。
- 渲染不能再依靠手写 draw 顺序和散落的 bool 控制；必须有可测试的 frame graph、资源质量门禁和固定截图验收。
- UI 不能再混用旧 GPU UI、旧面板、egui 过渡面板和 V9 组件；新线只允许 V9。
- GDP 不能再用目标总量反推建筑。历史 GDP 是校准输入，建筑只是可交互的物理/工业资产，不代表全部经济。
- GDP 必须由第一产业、第二产业、第三产业的增加值核算合成，并有历史注入、校准报告和动态演化路径。
- POP 不能只是 UI 展示数组，必须成为劳动、收入、消费、税基、兵源、满意度和政治反应的真实主体。
- 财政不能靠直接改现金字段或按 GDP 粗暴比例生成税收，必须有 ledger、预算、收入、支出、债务、利息、融资和结余闭环。
- 市场、建设、财政、POP 不能互相直接改对方内部字段，必须通过订单、账本、快照和命令协作。
- 测试不能只测能跑、不 panic、数字大于零；必须测守恒、账平、区间、因果、回归、视觉和性能验收。

## 2. 总架构目标

### 2.1 模块边界

新项目的基本边界如下：

| 模块 | 职责 | 禁止事项 |
| --- | --- | --- |
| `ironheart-main` | 进程入口、窗口、日志、配置、崩溃报告 | 禁止持有玩法规则、渲染 pass、UI 面板状态 |
| `ironheart-engine` | 场景状态、输入路由、命令总线、调度、快照协调、存档协调 | 禁止把经济/UI/渲染逻辑写进 engine |
| `ironheart-sim` | 世界状态、经济、POP、财政、市场、军事、外交、内容、AI、确定性 tick | 禁止为 UI 或渲染写临时展示字段 |
| `ironheart-render` | GPU、资源、frame graph、地图、兵牌、后处理、视觉报告 | 禁止读取可变世界状态，禁止绕过 frame graph |
| `ironheart-ui` | V9 tokens、布局、面板、HUD、命令输出、UI 快照测试 | 禁止直接读写 simulation，禁止混入第二套 UI |

### 2.2 数据流

每帧只能走这一条主链路：

```text
Input
  -> ironheart-engine::InputRouter
  -> UiCommand / GameCommand
  -> ironheart-engine::CommandBus
  -> ironheart-sim::SimulationRuntime
  -> SimulationSnapshot
  -> RendererFrameInput + UiFrameModel
  -> ironheart-render + ironheart-ui
```

渲染和 UI 只消费快照。它们不能直接访问 `WorldState`，不能自己推导经济事实，也不能为了显示方便修改模拟层。

### 2.3 权威数据来源

| 数据 | 权威 owner | 下游消费者 |
| --- | --- | --- |
| 世界事实、国家、州、省份 | `ironheart-sim` | engine、render、ui |
| 命令语义和调度 | `ironheart-engine` | sim、ui |
| GDP、POP、财政、市场 | `ironheart-sim::economy` 及其子模块 | ui、ai、content |
| 地图 draw list、资源状态 | `ironheart-render` | debug/验收工具 |
| V9 布局和用户命令 | `ironheart-ui` | engine |
| 历史数据、内容脚本 | data/content adapter | sim |

任何系统需要新增共享字段，必须先改 schema、测试和文档，再改实现。禁止为了赶界面直接把字段塞进别的系统结构里。

## 3. 总执行顺序

严格执行以下阶段。阶段失败时不能进入下一阶段。

```text
P0  分支隔离和主线冻结
P1  新工作区骨架和 crate 边界
P2  功能完整矩阵和验收矩阵
P3  数据契约、ID、单位和历史数据 schema
P4  快照、命令总线、确定性时钟和事件日志
P5  历史经济注入和三产业 GDP 核算
P6  POP、劳动、收入、消费和财政 ledger
P7  市场、产业、建设、采购和库存闭环
P8  运行时调度、存档、内容脚本 adapter
P9  V9 UI 基础系统和空壳 HUD
P10 渲染 frame graph、资源质量门禁和视觉 fixture
P11 地图基础渲染重建
P12 兵牌聚合、覆盖层和地图交互
P13 面板按数据稳定顺序逐项迁移
P14 军事、外交、AI、事件和完整内容接入
P15 全项目验收、性能、replay 和回归门禁
P16 替换旧主线或保持长期重构分支
```

这个顺序的理由很简单：

- 先 P2/P3，是为了知道到底要保留哪些功能、每个数据从哪里来、用什么验收。
- 再 P4，是为了让 UI 和渲染从第一天就不能绕过快照和命令总线。
- 先 P5-P7，是因为 GDP、POP、财政、市场是玩法底座；不先修，UI 面板只会继续展示错误数字。
- P9 和 P10 可以在 P5-P7 稳定后进入，但只能消费快照和 fixture，不能倒逼模拟层改内部结构。
- P13 必须晚于经济、UI shell、渲染基础，因为面板迁移依赖稳定数据模型和 V9 规则。
- P14 晚于核心经济和命令系统，因为 AI、事件、内容会放大所有底层错误。

## 4. 阶段路线图

### P0 分支隔离和主线冻结

目标：让旧主线冻结，新重构线完全隔离。

输入：

- 旧主线作为只读行为参考。
- 新 orphan/rewrite 分支。

允许工作：

- 新目录、新 workspace、新文档、新 CI 门禁。
- 记录旧主线冻结规则。

禁止工作：

- 从旧主线复制巨型入口文件。
- 迁移旧 UI 或旧渲染路径。
- 在旧主线继续做普通重构。

交付物：

- 独立分支 `project-ironheart/main-rewrite`。
- 独立目录 `project-ironheart-main/`。
- 主线冻结说明。

退出标准：

- 新分支顶层只包含重构线需要的文件。
- `cargo check --workspace` 通过。
- 旧主线没有被重构工作污染。

当前状态：已完成。

### P1 新工作区骨架和 crate 边界

目标：先建立模块边界，不实现真实玩法。

输入：

- P0 的 clean rewrite 工作区。
- 总架构边界。

允许工作：

- `ironheart-main`
- `ironheart-engine`
- `ironheart-sim`
- `ironheart-render`
- `ironheart-ui`
- 最小启动、最小 tick、最小 frame plan、最小 UI model。

禁止工作：

- 迁移旧经济公式。
- 迁移旧面板。
- 移植旧 draw 顺序。

交付物：

- 每个 crate 的职责说明。
- 最小可编译 workspace。
- 基础 smoke test 或 compile gate。

退出标准：

- `cargo check --workspace` 通过。
- 入口层不包含系统细节。
- 每个 crate 只暴露最小公共接口。

当前状态：已完成基础骨架，后续只允许补边界测试和文档，不允许在 P1 里偷做系统实现。

### P2 功能完整矩阵和验收矩阵

目标：定义“完整一样”到底是什么意思，避免后续凭感觉迁移。

输入：

- 旧主线行为观察。
- 用户期望功能。
- 当前视觉失败、经济失败、UI 混乱问题。

允许工作：

- 功能矩阵文档。
- 验收场景 schema。
- 旧行为 sample。
- 测试 fixture 目录结构。

禁止工作：

- 开始迁移 UI 面板。
- 开始真实地图渲染。
- 开始经济 tick。

必须建立的矩阵字段：

| 字段 | 说明 |
| --- | --- |
| `feature_id` | 稳定功能 ID |
| `domain` | runtime / map / ui / economy / military / diplomacy / content / save |
| `old_reference` | 旧行为参考位置或截图/记录 |
| `target_behavior` | 新线目标行为 |
| `data_owner` | 权威数据来源 |
| `snapshot_contract` | 对外快照字段 |
| `commands` | 用户或 AI 能触发的命令 |
| `acceptance` | 可执行验收条件 |
| `status` | missing / contracted / implemented / accepted |

必须覆盖的功能域：

- 启动和配置。
- 地图和相机。
- 省份、州、国家选择。
- 时间推进和速度控制。
- 经济、GDP、POP、财政、市场、建设。
- 政治、法律、决议。
- 陆海空军、后勤、前线、命令。
- 外交、战争、和平、贸易。
- AI、事件、内容脚本。
- 存档、读档、replay。
- UI 面板、通知、设置、debug。
- 渲染资源、截图、性能。

退出标准：

- 每个旧功能都有 `feature_id`。
- 每个 `feature_id` 有 owner、数据来源、验收方式。
- 没有“之后再说”的核心功能。
- 没有只有口头描述、无法测试的验收项。

### P3 数据契约、ID、单位和历史数据 schema

目标：先定数据语言，再写系统。

输入：

- P2 功能矩阵。
- 历史经济/人口/财政/产业/军事资料来源清单。

允许工作：

- ID 类型。
- 货币、时间、人口、产量、价格、GDP 单位。
- 历史数据 schema。
- snapshot schema。
- command schema。
- fixture loader skeleton。

禁止工作：

- 在 schema 未定时实现经济 tick。
- 在单位未定时展示 UI 数字。
- 用字符串 ID 或裸 `f64` 到处传经济事实。

必须定义：

```text
CountryId
StateId
ProvinceId
PopGroupId
IndustrySectorId
GoodId
CurrencyUnit
Money
PopulationCount
LaborCount
PriceIndex
ValueAdded
GameDate
TickId
```

历史数据 schema 至少包括：

- 国家总人口、州人口、城市/农村拆分。
- 劳动力率、就业结构。
- 第一/第二/第三产业份额和增加值。
- 历史 GDP 校准值。
- 政府收入、支出、现金、债务、黄金、外汇储备。
- 产业资产、农业产能、服务业容量。
- 军事编制、装备库存、补给基础。

退出标准：

- schema 能加载 fixture。
- 错误数据能给出明确校验报告。
- 单位转换有测试。
- 快照字段能映射到 P2 功能矩阵。

### P4 快照、命令总线、确定性时钟和事件日志

目标：建立所有系统合作的主通道。

输入：

- P3 schema 和 ID。
- P2 功能矩阵中的命令清单。

允许工作：

- `SimulationSnapshot`
- `RendererFrameInput`
- `UiFrameModel`
- `GameCommand`
- `UiCommand`
- `CommandBus`
- `SimulationClock`
- `EventLog`

禁止工作：

- UI 直接调用 sim 内部方法。
- renderer 直接读取世界状态。
- 内容脚本直接改财政现金或 POP 字段。

必须满足：

```text
UI action -> UiCommand -> GameCommand -> CommandBus -> SimulationRuntime
SimulationRuntime -> SimulationSnapshot -> UiFrameModel + RendererFrameInput
```

退出标准：

- 空世界能产生完整空快照。
- 命令 round-trip 有测试。
- 同输入、同 seed、同 tick 结果一致。
- 事件日志可以解释一次状态变化来自哪个命令。

### P5 历史经济注入和三产业 GDP 核算

目标：修复经济底座，GDP 不再反推建筑。

输入：

- P3 历史经济 schema。
- P4 snapshot/command 基础。

允许工作：

- `HistoricalEconomyInput`
- `SectorAccount`
- `NationalAccounts`
- `GdpAccount`
- `CalibrationReport`
- GDP/产业 fixture 和 contract tests。

禁止工作：

- 用 GDP 总量生成建筑数量。
- 让建筑总产值代表全部 GDP。
- 用 UI 需要的显示数字污染经济核心。
- 让财政或市场在本阶段改 GDP 结果。

核算原则：

```text
GDP = PrimarySectorValueAdded
    + SecondarySectorValueAdded
    + TertiarySectorValueAdded
```

其中：

- 第一产业：农业、采矿、资源采掘、基础食物和原料。
- 第二产业：制造、军工、能源、建设、重工业、轻工业。
- 第三产业：运输、贸易、金融、行政、教育、医疗、科研、普通服务。

历史 GDP 的作用：

- 作为国家/州/年份的校准输入。
- 用来检查三产业核算结果是否在允许误差内。
- 生成 `CalibrationReport`，说明误差来自产业份额、人口、生产率、殖民/本土拆分或数据缺失。

历史 GDP 不能做的事：

- 不能直接除以建筑单价得到工厂数。
- 不能直接决定税收。
- 不能直接决定 POP 收入。
- 不能替代产业产能、就业和生产率。

退出标准：

- 主要国家 1936 GDP fixture 可加载。
- 三产业合计等于 GDP account 总量。
- 校准误差超过阈值时测试失败。
- GDP account 能解释每一项来源，不存在无来源硬编码总量。

### P6 POP、劳动、收入、消费和财政 ledger

目标：让人口和财政成为闭环系统，而不是 UI 数字。

输入：

- P5 经济账户。
- P3 人口和财政 baseline schema。

允许工作：

- `PopGroup`
- `LaborPool`
- `EmploymentState`
- `PopIncome`
- `PopConsumptionBudget`
- `TaxBase`
- `FiscalLedger`
- `FiscalLedgerEntry`
- `TreasuryAccount`
- `DebtAccount`

禁止工作：

- 直接改 `cash` 字段而没有 ledger entry。
- 用 GDP 固定比例生成所有税收。
- POP 消费直接扣市场库存。
- POP 人口只在 UI 显示，不参与劳动、税基、消费和兵源。

POP 必须负责：

- 总人口和州人口守恒。
- 劳动力供给。
- 就业、失业、职业结构。
- 工资、利润分配、转移支付。
- 税基、可支配收入、消费预算。
- 生活水平、满意度、激进度、忠诚度。
- 兵源和训练潜力。

财政 ledger 必须满足：

```text
closing_cash = opening_cash
             + revenue
             - expense
             + financing
             + valuation_adjustment
```

每一笔现金变化必须有：

- 时间。
- 国家。
- 科目。
- 来源系统。
- 金额。
- 对应命令或自动调度原因。

退出标准：

- POP 总人口、州人口、劳动力人数守恒。
- 财政 opening/closing 可重放。
- 税收来自税基，不来自裸 GDP 比例。
- 债务和利息有 ledger entry。
- UI 所需财政/人口字段全部来自 snapshot，而不是面板自算。

### P7 市场、产业、建设、采购和库存闭环

目标：把 POP、财政、建设和生产统一接到 market order，而不是互相改内部字段。

输入：

- P5 GDP/产业账户。
- P6 POP 和财政 ledger。

允许工作：

- `Good`
- `MarketOrder`
- `MarketClearing`
- `Inventory`
- `ProductionOutput`
- `ConstructionProject`
- `GovernmentProcurementOrder`
- `IndustrialAsset`

禁止工作：

- 政府采购直接生成装备。
- 建设完成直接凭空增加 GDP。
- 市场直接修改财政现金。
- 建筑系统作为整个经济的唯一载体。

闭环顺序：

```text
POP income -> consumption budget -> market demand
Government budget -> procurement/construction orders -> market demand
Industry assets + labor + inputs -> production output -> supply/inventory
Market clearing -> prices/shortages -> income/ledger/production constraints
Construction progress -> asset completion -> future capacity
```

退出标准：

- demand fulfilled 不超过 demand。
- supply used 不超过 supply + inventory + import。
- 建设项目受资金、材料、劳动力影响。
- 军事采购支出能追踪到财政 ledger。
- 市场短缺会影响建设/生产，而不是只显示红字。

### P8 运行时调度、存档、内容脚本 adapter

目标：让系统在确定性时间轴上运行，并为内容和存档提供稳定接口。

输入：

- P4 命令和事件日志。
- P5-P7 核心经济闭环。

允许工作：

- daily/weekly/monthly schedule。
- save/load schema。
- replay log。
- content trigger/effect adapter。
- migration-safe save version。

禁止工作：

- 内容脚本直接绕过 command/ledger 改内部状态。
- 存档保存 UI 临时状态当作世界事实。
- AI 在调度外修改世界。

调度层级：

- hourly：军事移动、战斗、短周期命令。
- daily：经济、市场、财政、POP 消费、训练、补给。
- weekly：AI、贸易、汇率、信用、政治动态。
- monthly：预算计划、人口迁移、工业扩张、长期建设、统计归档。

退出标准：

- 1936 空跑 365 天 deterministic。
- 存档后读取再跑，结果一致。
- 事件 effect 通过命令或 ledger 记录。
- 内容脚本错误能定位到脚本、trigger、effect 和目标对象。

### P9 V9 UI 基础系统和空壳 HUD

目标：建立唯一 UI 路线，但不抢先做未稳定数据的复杂面板。

输入：

- P4 `UiFrameModel`。
- P2 UI 功能矩阵。
- P5-P7 已稳定的经济/人口/财政 snapshot 字段。

允许工作：

- V9 tokens。
- V9 primitives。
- topbar。
- side rail。
- modal layer。
- notification layer。
- panel shell。
- map HUD shell。
- UI command mapping。

禁止工作：

- 第二套 UI 路线。
- 面板直接读 simulation。
- 在数据未 accepted 前展示“看起来正确”的财政、GDP、POP 数字。
- 用旧皮肤或旧面板结构凑 V9。

V9 基础原则：

- 静态风格 token 统一。
- 面板只消费 `UiFrameModel`。
- 所有按钮、切换、滑块、列表选择都输出 typed command。
- 面板布局要有小屏、1080p、1440p 和宽屏约束。
- 文本不能溢出、重叠或遮挡关键地图交互。

退出标准：

- 空 HUD 可运行。
- topbar/side rail/modal/notification 都有 snapshot 测试。
- 没有旧 UI 路线残留。
- UI command round-trip 通过。

### P10 渲染 frame graph、资源质量门禁和视觉 fixture

目标：先重建渲染管线规则，再接真实地图。

输入：

- P4 `RendererFrameInput`。
- P2 视觉验收场景。

允许工作：

- `FrameGraph`
- `FramePlan`
- `RenderNode`
- `RenderResourceRegistry`
- `ResourceQualityReport`
- `VisualAcceptanceScene`

禁止工作：

- 复制旧 draw 顺序。
- 让 pass 绕过 frame graph。
- 关键资源缺失时静默 fallback。
- 没有截图验收就说视觉优化完成。

标准节点顺序：

```text
shadow
sky
terrain_base
water
borders
static_decals
semantic_overlays
world_objects
counters
labels
particles
postprocess
v9_ui
debug_overlay
```

资源质量等级：

- `Required`：缺失则阻断验收。
- `DegradedAllowed`：允许运行，但验收报告必须标记。
- `DebugOnly`：只能在 debug fixture 出现。

退出标准：

- FrameGraph 可生成可测试 FramePlan。
- 资源缺失有报告。
- 关键 fallback 数量为 0 才能通过生产截图验收。
- 固定视觉场景 metadata 完整。

### P11 地图基础渲染重建

目标：在新 frame graph 下重建地图可读性。

输入：

- P10 frame graph 和资源质量门禁。
- P3 map/state/province schema。
- P4 `MapSnapshot`。

允许工作：

- 地形基础。
- 水体。
- 国界/州界/省界。
- 政治色。
- 地形/补给/建设/资源覆盖层。
- 标签。
- 后处理校准。

禁止工作：

- 同时迁移兵牌海。
- 为了截图好看写 debug mock battlefield。
- 让 UI 覆盖层直接控制地图绘制。

地图视觉预算：

- 战略远景：政治色、国界、国家标签、聚合军事态势优先。
- 作战中景：前线、交通、补给、军队密度优先。
- 战术近景：省份、建筑、地形细节、单位细节优先。

退出标准：

- 无兵牌地图可读。
- 欧洲远景、中欧中景、海岸/海域、沙漠/雪地/山地固定截图通过。
- 政治色、边界、标签不互相打架。
- 视觉报告能说明资源质量和后处理参数。

### P12 兵牌聚合、覆盖层和地图交互

目标：解决当前游戏效果差的核心视觉问题之一：战略缩放下兵牌不可读。

输入：

- P11 地图基础渲染。
- P4 military snapshot 和 command bus。

允许工作：

- `DivisionSnapshot`
- `CounterAggregation`
- `CounterPriority`
- `CounterPlacement`
- `CollisionSolver`
- `CounterDrawList`
- map hover/selection command。
- front line / battle plan overlay。

禁止工作：

- 所有师全量直接画到屏幕。
- renderer 自己查询 sim 内部部队。
- UI 自己决定 counter 位置。
- hover/selection 绕过 command bus。

兵牌链路：

```text
DivisionSnapshot
  -> zoom_class
  -> aggregation
  -> priority
  -> screen placement
  -> collision solving
  -> draw list
```

退出标准：

- 欧洲战略远景不出现不可读兵牌海。
- 聚合、隐藏、碰撞数量可报告。
- 选中、悬停、展开和命令反馈有测试。
- 前线/箭头不遮挡关键地图标签和兵牌。

### P13 面板按数据稳定顺序逐项迁移

目标：按数据依赖迁移面板，不能所有面板同时改。

输入：

- P9 V9 shell。
- P5-P8 稳定 snapshot。
- P2 UI panel matrix。

允许工作：

- 一个面板一个 snapshot model。
- 一个面板一组 command。
- 一个面板一套 layout/snapshot tests。

禁止工作：

- 面板自算经济核心数字。
- 两个面板同时新增同一个共享字段。
- 为赶 UI 绕过 engine command。
- 用未 accepted 的数据做正式面板。

面板迁移顺序：

1. 设置、存档、读档、调试。
2. 顶栏详情、财政总览。
3. 人口、市场、建设。
4. 政治、法律、决议。
5. 研究。
6. 外交、战争、和平、贸易。
7. 陆军、海军、空军、后勤。
8. 事件、通知、结束界面。
9. 主菜单、国家选择、加载页。

单个面板流程：

```text
PanelSnapshot -> PanelModel -> V9 Layout -> UiCommand -> Engine Command -> Sim Command
```

退出标准：

- 每个面板单独 accepted。
- 每个面板有空状态、错误状态、正常状态、大数据状态。
- 1080p、1440p、小屏布局不重叠。
- 面板命令能回放。

### P14 军事、外交、AI、事件和完整内容接入

目标：在经济、UI、渲染和命令边界稳定后接入高层玩法。

输入：

- P8 调度和内容 adapter。
- P12 地图交互。
- P13 基础面板。

允许工作：

- 军事 tick。
- 外交关系和战争状态。
- AI plan。
- event/focus/decision adapter。
- 内容脚本错误报告。

禁止工作：

- AI 直接改 UI。
- 事件直接改财政现金。
- 内容 effect 绕过 command、ledger 或 event log。
- 为 AI 增加特殊内部捷径。

退出标准：

- 1936 主要国家能按日推进。
- AI 行为不破坏经济账平。
- 事件和决议能追踪来源。
- 战争、外交、贸易状态能存档和 replay。

### P15 全项目验收、性能、replay 和回归门禁

目标：证明新线可以替代旧线，而不是只在局部跑通。

输入：

- P2 全功能矩阵。
- P5-P14 所有 accepted 功能。

必须通过：

- 功能矩阵全部 accepted 或有明确延期批准。
- 1936 初始化数据 accepted。
- 德国/英国/苏联/美国/日本/中国等主要国家经济校准 accepted。
- 至少一个 90 天经济和财政 replay。
- 至少一个 365 天完整 gameplay replay。
- 固定视觉截图 accepted。
- V9 UI snapshot accepted。
- 存档/读档/replay 一致性 accepted。
- 性能预算 accepted。
- critical fallback = 0。
- 生产路径 mock = 0。

禁止通过方式：

- 只跑 `cargo check` 就宣布验收。
- 只看一张截图就宣布视觉完成。
- 只要数字非零就宣布经济正确。
- 临时跳过 failing tests。
- 把未实现功能从矩阵里删掉。

退出标准：

- CI 能复现验收。
- 验收报告包含失败项、跳过项、性能和视觉差异。
- 所有跳过项都有 owner、原因和后续 milestone。

### P16 替换旧主线或保持长期重构分支

目标：只有在 P15 完成后，才讨论替换旧主线。

输入：

- P15 全验收报告。
- 迁移风险清单。

允许工作：

- 生成迁移报告。
- 生成旧线/新线功能对比。
- 开 milestone PR。
- 人工验收。
- 合并、重命名或保持长期分支。

禁止工作：

- P15 未完成就合回主线。
- 把旧主线未清理代码混进新线。
- 为了合并降低验收标准。

退出标准：

- 用户明确接受新线。
- 旧线冻结状态和后续维护策略明确。
- 新线成为 Project Ironheart 的主开发线，或继续作为独立长期线维护。

## 5. 旧到新迁移矩阵

| 旧线职责 | 新线位置 | 迁移方式 |
| --- | --- | --- |
| 巨型入口层 | `ironheart-main` + `ironheart-engine` | 只提取职责，不复制结构 |
| 游戏循环和调度 | `ironheart-engine` + `ironheart-sim` | 先 command/snapshot，再 tick |
| 世界状态 | `ironheart-sim::world` | schema 重建，旧数据 adapter 读取 |
| 经济公式 | `ironheart-sim::economy` | 按三产业、POP、财政 ledger 重写 |
| 人口展示数据 | `ironheart-sim::population` | 变为真实主体和经济输入 |
| 财政字段 | `ironheart-sim::finance` | 变为 ledger 和账户 |
| 市场/建设 | `ironheart-sim::market` / `construction` | 变为订单和库存闭环 |
| 渲染 draw 顺序 | `ironheart-render::frame_graph` | 重建为节点和 FramePlan |
| 地图 pass | `ironheart-render::map` | 接受 `RendererFrameInput` |
| 兵牌绘制 | `ironheart-render::counters` | 聚合、优先级、碰撞、draw list |
| 多套 UI | `ironheart-ui` | V9 唯一路线 |
| 面板逻辑 | `ironheart-ui::panels` + snapshot builder | 面板不读 sim 内部 |
| 内容事件 | `ironheart-sim::content` | trigger/effect adapter + event log |
| AI | `ironheart-sim::ai` | 通过 plan 和 command 影响世界 |
| 存档 | `ironheart-engine::save` + sim schema | versioned snapshot/save model |

迁移原则：旧线只给出“要有什么”和“哪里坏了”，新线决定“应该怎么设计”。

## 6. 经济专项总路线

经济重构不是 UI 附属任务，是 Project Ironheart 玩法底座重构。

### 6.1 数据注入顺序

```text
historical raw data
  -> validated historical input
  -> sector baseline
  -> population baseline
  -> fiscal baseline
  -> market/industry baseline
  -> calibration report
  -> initial SimulationSnapshot
```

GDP 是历史校准输入和核算结果，不是建筑反推器。

### 6.2 GDP 核算

核心账户：

```text
GdpAccount {
    primary_value_added,
    secondary_value_added,
    tertiary_value_added,
    total,
    calibration_target,
    calibration_error,
}
```

要求：

- `total = primary + secondary + tertiary`。
- 三产业份额必须可解释。
- 服务业不能被迫塞进建筑数量。
- 政府行政、教育、医疗等第三产业要有独立容量/就业/工资来源。
- 军工产出属于第二产业，但军事采购支出通过财政 ledger 进入。

### 6.3 POP 模型

POP 最少维度：

- 地区。
- 阶层/职业。
- 文化/语言。
- 收入来源。
- 就业状态。
- 消费篮子。
- 税负。
- 满意度和政治倾向。
- 可征召人口。

POP 不是每帧 UI 数组。POP 是经济 tick、政治稳定、兵源和消费市场的输入。

### 6.4 财政模型

财政分三层：

- Budget plan：政府计划。
- Ledger：已发生账目。
- Treasury/debt：现金、债务、利息、融资能力。

所有财政结果必须能回答：

- 钱从哪里来？
- 花到哪里去？
- 哪个命令或调度导致？
- 是否影响市场订单？
- 是否影响债务和信用？

### 6.5 市场和建设

建设不直接增加 GDP。建设消耗资金、材料、劳动力，完成后增加资产或基础设施，未来通过产能、就业和市场供给影响 GDP。

市场不直接改财政现金。财政通过采购/建设订单进入市场，市场结算再生成 ledger 或结算事件。

## 7. 渲染专项总路线

渲染重构目标不是“看起来稍微好一点”，而是建立可控、可验收、可维护的渲染管线。

### 7.1 管线原则

- 所有 pass 必须在 frame graph 注册。
- draw order 由 FramePlan 生成。
- 资源缺失必须被报告。
- 视觉验收使用固定场景和截图 metadata。
- debug overlay 和生产画面必须可区分。

### 7.2 地图质量目标

必须解决：

- 缩放层级信息密度失控。
- 兵牌遮挡和不可读。
- 地形、水体、政治色、边界、标签互相抢视觉焦点。
- 后处理把标签、兵牌、边界弄脏。
- 缺资源时静默 fallback 导致画面劣化却没人发现。

### 7.3 验收场景

固定场景至少包括：

- 欧洲全图战略远景。
- 中欧高密度作战中景。
- 单省/单州近景。
- 海岸和浅海。
- 沙漠、雪地、山地。
- 国家选择界面。
- UI + 地图混合画面。

每个场景记录：

- frame graph 节点列表。
- 资源 fallback 数量。
- counter 数量、聚合数量、隐藏数量、碰撞数量。
- CPU/GPU 时间。
- 截图 hash 或视觉差异指标。

## 8. V9 UI 专项总路线

V9 是唯一 UI 路线。重构不是给旧面板换皮，而是重建 UI 数据流、布局系统和交互命令。

### 8.1 UI 数据原则

UI 只接受：

```text
UiFrameModel
PanelModel
NotificationModel
MapHudModel
```

UI 只输出：

```text
UiCommand
```

UI 不做：

- 经济核算。
- POP 推导。
- 财政结算。
- 兵牌位置计算。
- 世界状态修改。

### 8.2 V9 验收

每个面板必须通过：

- 空状态。
- 正常状态。
- 极端数据状态。
- 错误状态。
- 小屏、1080p、1440p、宽屏布局。
- 命令输出测试。
- 文本不溢出、不重叠、不遮挡关键操作。

### 8.3 风格约束

- 统一 tokens。
- 不允许混用旧 UI 皮肤。
- 不允许面板内部私有调色板。
- 图标、按钮、切换、滑块、表格、标签、列表必须有统一 primitive。
- 业务面板保持高信息密度和可扫描性，不做营销页式布局。

## 9. 测试和验收总策略

验收必须有明确目的，不能乱测、白测。

### 9.1 测试类型

| 测试 | 目的 | 示例 |
| --- | --- | --- |
| contract test | 保证接口不漂移 | snapshot schema、command round-trip |
| invariant test | 保证守恒和账平 | 人口守恒、财政 closing cash |
| calibration test | 保证历史数据合理 | 1936 GDP 三产业误差 |
| scenario test | 保证玩法链路 | 建设订单影响财政和市场 |
| replay test | 保证确定性 | 同 seed 365 天一致 |
| visual test | 保证画面质量 | 固定地图截图 diff |
| UI snapshot test | 保证布局和状态 | 面板空/正常/错误状态 |
| performance test | 保证可玩性 | frame time、tick time、内存 |
| migration test | 保证旧功能保留 | feature matrix accepted |

### 9.2 不合格测试

以下测试不能作为验收依据：

- 只检查程序能启动。
- 只检查没有 panic。
- 只检查 GDP 大于 0。
- 只检查 UI 节点存在。
- 只保存一张人工截图，没有固定场景和指标。
- 只跑 debug mock 数据。
- 跳过失败项但没有 owner 和后续计划。

### 9.3 CI 门禁

每个阶段至少需要：

```powershell
cargo check --workspace
cargo test --workspace
```

随着阶段推进增加：

- schema validation。
- economy calibration tests。
- replay tests。
- visual acceptance scenes。
- UI snapshot tests。
- performance budgets。

## 10. 防止系统打架的治理规则

### 10.1 单 owner

| 接口 | owner |
| --- | --- |
| `SimulationSnapshot` | sim owner |
| `EconomySnapshot` | economy owner |
| `UiFrameModel` | UI owner + engine coordinator |
| `RendererFrameInput` | render owner + engine coordinator |
| `GameCommand` | engine owner |
| `FiscalLedger` | finance owner |
| `FrameGraph` | render owner |
| `V9Tokens` | UI owner |

### 10.2 接口变更流程

任何共享接口变更必须按顺序：

1. 在功能矩阵或专项文档写明需求。
2. 增加或更新 schema/contract test。
3. 由 owner 修改接口。
4. 下游通过 adapter 消费。
5. 更新验收。

禁止下游直接改 owner 的结构体来满足自己需求。

### 10.3 阶段锁

每个阶段有允许修改范围。未列入范围的模块只能：

- 增加只读 adapter。
- 增加测试 fixture。
- 增加文档。

不能改生产行为。

### 10.4 冲突处理优先级

发生系统冲突时，按以下顺序裁决：

1. 数据权威来源。
2. snapshot 契约。
3. command 契约。
4. 测试和验收。
5. UI 展示需求。

UI 想显示的数据如果没有权威来源，UI 必须显示缺失/不可用/未实现状态，不能临时造数字。

## 11. 当前立即下一步

当前已完成 P0 和 P1 的基础形态。下一步不是继续写 UI，也不是继续写 renderer，更不是先写经济 tick。

下一步必须进入 P2：

1. 建立 `feature_parity_matrix`。
2. 建立 `acceptance_matrix`。
3. 建立旧行为参考索引。
4. 建立历史经济、人口、财政、市场、地图、UI、存档的 schema 草案。
5. 建立测试 fixture 目录和最小 contract tests。

P2 没完成前，禁止：

- 真实地图 pass 迁移。
- 旧面板迁移。
- 经济 tick 实现。
- AI 接入。
- 生产截图验收。

## 12. 风险和刹车条件

必须立即刹车的情况：

- 某个文件开始重新膨胀成入口巨型文件。
- UI 或 renderer 直接访问 sim 内部状态。
- GDP 又开始从建筑数量或单个总量字段反推。
- 财政现金出现没有 ledger entry 的变化。
- POP 再次退化成只展示的人口数字。
- 渲染 pass 绕过 frame graph。
- V9 之外出现第二套 UI。
- 测试只剩 smoke test，没有验收意义。
- 为了赶进度删除功能矩阵中的旧功能。

发现以上情况时，当前阶段停止，先修边界和测试，再继续功能实现。

## 13. 最终完成定义

Project Ironheart 重构完成，不是指“新线能跑起来”，而是同时满足：

- 功能矩阵完整覆盖并 accepted。
- GDP、POP、财政、市场、建设闭环通过账本和守恒验收。
- 渲染管线由 frame graph 驱动，固定截图通过，critical fallback 为 0。
- V9 是唯一 UI 路线，所有面板按 snapshot/command 工作。
- 365 天 replay deterministic。
- 存档/读档一致。
- 性能预算通过。
- 旧主线不再是普通开发线，只作为归档参考或被正式替换。

