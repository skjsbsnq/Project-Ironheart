# Project Ironheart 完整重构路线图

本文档定义 `project-ironheart/main-rewrite` 分支的总重构路线。目标不是把旧实现平移过来，而是在功能完整保留的前提下重建架构、渲染、V9 UI、模拟与经济系统。

> 完整项目级总控路线图以 [`013-project-ironheart-master-rewrite-roadmap.zh.md`](013-project-ironheart-master-rewrite-roadmap.zh.md) 为唯一权威。本文档只作为系统设计参考，不允许单独作为开工顺序使用。

## 1. 总目标

### 1.1 功能完整保留

重构完成后必须覆盖旧主线已经具备的玩法能力：

- 启动、路径配置、内容加载、国家选择、进入游戏。
- 地图浏览、相机、选择省份、选择部队、建造放置、前线/箭头/命令交互。
- 顶栏、侧边栏、政治、决议、法律、人口、市场、财政、贸易、建造、研究、外交、陆军、海军、空军、后勤、局势、设置、存档、事件、投降/和平通知。
- 时间推进、速度控制、暂停、每日/每周/每月调度。
- 历史 1936 数据加载、国家初始状态、人口、工业、财政、市场、军事、外交关系。
- AI、内容事件、焦点、决议、经济 tick、军事 tick、外交 tick。
- 截图、性能、视觉质量、调试 overlay。

功能完整不等于代码照搬。任何旧功能进入新线，必须先经过接口收敛和数据契约定义。

### 1.2 必须修复的问题

- 入口层不能再持有渲染 pass、UI 面板、模拟 tick 细节和大量临时状态。
- 渲染不能再靠巨型手写 draw 顺序和散落 bool 控制。
- UI 不能再混用旧皮肤、旧 GPU 菜单、egui 旧面板和 V9 组件。
- 经济系统不能再用目标 GDP 反推建筑，也不能让财政、人口、市场互相重复推导。
- POP 不能只是 UI 展示用数组，必须成为就业、收入、消费、税基、兵源、满意度的真实源。
- GDP 必须来自第一、第二、第三产业的增加值核算，同时支持历史注入和动态演化。
- 财政必须有现金流、预算、债务、信用、外汇、军费、建设资金、隐性融资的闭环。
- 测试不能只测“能跑”“不 panic”“数字大于零”，必须测账平、守恒、区间、因果、回归和验收场景。

## 2. 重构原则

### 2.1 保留行为，重建边界

旧主线作为参考实现，不作为新代码结构模板。每个系统迁移时先写目标接口，再适配旧数据，再替换实现。

### 2.2 快照驱动

渲染和 UI 不直接读取可变世界。模拟层产出快照：

```text
SimulationState
    -> FrameSnapshot
    -> RendererFrameInput
    -> UiFrameModel
```

UI 返回 typed command，engine 再把 command 应用到模拟。

### 2.3 资源和质量显式化

地图资源、字体、贴图、模型、音频、历史经济数据都要有质量状态：

- `Required`：缺失直接阻断对应验收。
- `DegradedAllowed`：允许运行，但必须可见标记。
- `DebugOnly`：只能在 debug fixture 中出现。

### 2.4 经济数据不可倒推污染

历史 GDP 是校准输入，不是建筑数量的反推结果。建筑、POP、产业、财政以各自数据源初始化，然后通过核算表对齐和校准因子做合理约束。

## 3. 目标模块

### 3.1 `ironheart-main`

只负责进程入口、平台窗口、命令行、日志、崩溃报告。禁止持有游戏规则、渲染 pass、UI 面板状态。

### 3.2 `ironheart-engine`

负责场景状态、输入路由、命令队列、时钟、调度、快照生成协调。

核心对象：

- `Engine`
- `SceneState`
- `InputRouter`
- `CommandBus`
- `FrameCoordinator`
- `SaveLoadCoordinator`

### 3.3 `ironheart-sim`

负责世界状态、经济、人口、财政、军事、外交、内容事件和 AI 的稳定接口。

核心对象：

- `WorldState`
- `SimulationRuntime`
- `SimulationSchedule`
- `CountrySnapshot`
- `MapSnapshot`
- `EconomySnapshot`
- `MilitarySnapshot`
- `DiplomacySnapshot`

### 3.4 `ironheart-render`

负责 GPU、资源、frame graph、pass 执行、视觉质量。

核心对象：

- `Renderer`
- `FrameGraph`
- `FramePlan`
- `RenderResourceRegistry`
- `MapMaterialSystem`
- `CounterPlacementSystem`
- `VisualQualityReport`

### 3.5 `ironheart-ui`

V9 唯一路线。负责 token、布局、面板壳、UI frame model、typed commands。

核心对象：

- `UiRuntime`
- `UiFrameModel`
- `UiCommand`
- `V9Tokens`
- `PanelShell`
- `Topbar`
- `SideRail`
- `ModalLayer`

## 4. 渲染重构路线

### Phase R0：视觉问题定界

输出固定截图组：

- 欧洲战略远景。
- 中欧作战密集区。
- 海岸/浅海。
- 沙漠/雪地/山地。
- 国家选择和游戏内 UI。

每张截图附带：

- frame graph 节点列表。
- 资源质量状态。
- fallback 数量。
- counter 数量、聚合数量、隐藏数量、碰撞数量。
- GPU/CPU 时间。

### Phase R1：FrameGraph 落地

新 renderer 不接受外部手写 pass 顺序。必须通过：

```text
RendererFrameInput -> FrameGraph::plan -> FramePlan -> Renderer::execute
```

FrameGraph 节点：

1. shadow
2. sky
3. terrain base
4. water
5. borders
6. static decals
7. semantic overlays
8. world objects
9. counters
10. labels
11. particles
12. postprocess
13. V9 UI
14. debug overlay

### Phase R2：地图材质预算

地图必须按 zoom class 分三套视觉预算：

- 战略远景：地图可读性优先，政治色、国界、集团军聚合、国家标签优先。
- 作战中景：前线、交通、补给、军队密度优先。
- 战术近景：单省、单部队、建筑、地形细节优先。

水体、季节、云雾、噪声、边界、政治覆盖层不能在同一频段互相抢注意力。

### Phase R3：兵牌和覆盖层重写

当前截图中最严重的问题是战略缩放下直接显示过多战术兵牌。新系统必须分层：

```text
DivisionSnapshot
    -> CounterAggregation
    -> CounterPriority
    -> ScreenPlacement
    -> CollisionSolver
    -> CounterDrawList
```

验收标准：

- 战略远景不能出现不可读兵牌海。
- 欧洲全图一屏时，显示聚合单位而不是所有师。
- 选中、悬停、前线附近单位优先显示。
- 被隐藏/合并单位必须可通过 hover 或点击展开。

### Phase R4：资源质量门禁

生产截图不允许关键资源静默 fallback。缺地形、水体、边界、字体、国家旗帜、后处理 LUT 等资源时必须：

- 阻断视觉验收，或
- 显示 degraded 标记，或
- 进入 debug fixture。

### Phase R5：后处理校准

后处理必须有固定场景校准，不能靠肉眼调。

验收项：

- 曝光稳定。
- 海面不过曝。
- 沙漠和雪地不糊成同一亮度。
- 政治色可辨。
- 标签和兵牌不会被 bloom/tonemap 破坏。

## 5. V9 UI 重构路线

### Phase U0：删除多路线 UI 策略

新分支只允许 V9：

- 不保留旧 GPU 菜单路径。
- 不保留旧面板 shell。
- 不允许面板私有调色板。
- 不允许 UI 直接读写 simulation。

### Phase U1：V9 Frame Model

统一数据入口：

```text
UiFrameModel {
    topbar,
    side_rail,
    map_hud,
    active_panel,
    modals,
    notifications,
    debug,
}
```

统一输出：

```text
Vec<UiCommand>
```

### Phase U2：主 HUD

优先重写：

- 顶栏：政治力、稳定度、战争支持、人力、GDP、GDP 增长、建造力、日期、速度、设置。
- 侧边栏：政治、决议、法律、民生、市场、财政、贸易、建造、研究、外交、陆军、海军、空军、后勤、局势。
- 地图 HUD：选中省份、选中部队、命令按钮、聚合兵牌展开。

### Phase U3：面板迁移

迁移顺序：

1. 设置、存档、调试。
2. 顶栏和财政总览。
3. 人口、市场、建造。
4. 政治、法律、决议。
5. 军事、海军、空军、后勤。
6. 外交、战争、和平、局势。
7. 事件、通知、结束界面。
8. 主菜单、国家选择、加载页。

每个面板必须有：

- 数据模型。
- command enum。
- V9 layout。
- 空状态。
- 错误状态。
- 小屏和 1080p/1440p 布局。
- 快照测试。

## 6. 经济系统重构路线

经济重构不在 UI 之后附带处理，而是核心模拟重构的一部分。

### Phase E0：历史经济数据契约

建立历史注入数据：

- 国家总人口。
- 州人口。
- 劳动力率。
- 第一/第二/第三产业份额。
- 1936 GDP。
- 政府收入/支出基线。
- 债务、黄金、外汇储备。
- 工业资产、农业产能、服务业容量。
- 殖民地/本土拆分。

历史 GDP 用作校准目标，不能直接生成建筑数量。

### Phase E1：产业核算表

GDP 改为三产增加值：

```text
GDP = AgricultureVA + IndustryVA + ServicesVA + GovernmentVA + NetColonialExtractionAdjustments
```

其中：

- 第一产业：农业、采矿、资源采掘、基础食物。
- 第二产业：制造、军工、建筑、能源、重工业、轻工业。
- 第三产业：金融、运输、行政、贸易、服务、教育、医疗、科研。

### Phase E2：建筑不再承载全部经济

建筑只表示可交互、可建设、可轰炸、可占领、可产出实物的资产。服务业和非建筑经济通过 sector capacity 和 employment profile 表达。

### Phase E3：POP 成为真实经济主体

POP 负责：

- 劳动力供给。
- 就业状态。
- 收入。
- 税负。
- 消费需求。
- 生活水平。
- 满意度、激进度、忠诚度。
- 兵源和职业转换。

### Phase E4：财政现金流闭环

财政 tick 改为账本：

```text
opening cash
+ tax revenue
+ state enterprise profit
+ bond proceeds
+ foreign borrowing
+ gold/forex operations
- wages
- procurement
- construction
- welfare
- research
- debt interest
- subsidies
= closing cash
```

任何现金变化必须有 ledger entry。

### Phase E5：市场和价格

市场负责商品供需、库存、进口、出口、价格、短缺。财政和 POP 通过订单进入市场，不直接改市场结果。

### Phase E6：经济 UI 接入

UI 显示来自 `EconomySnapshot`：

- GDP 总量、三产拆分、增长。
- 人口、就业、工资、生活水平。
- 预算收支、现金、债务、信用评级、MEFO/隐性融资。
- 商品短缺、产能利用率、建造瓶颈。

## 7. 模拟调度路线

### Phase S0：调度分层

分为：

- hourly：军事移动、战斗、短周期命令。
- daily：经济、市场、财政、POP 消费、训练、补给。
- weekly：AI、贸易、汇率、信用、政治动态。
- monthly：预算计划、人口迁移、工业扩张、长期建设、统计归档。

### Phase S1：可重复 tick

同样输入、同样命令、同样随机种子，必须产出同样结果。

### Phase S2：统计快照

每天结束生成国家经济统计快照，用于 UI、测试和存档。

## 8. 数据迁移路线

### Phase D0：数据 schema

新增：

- `historical_economy.ron`
- `sector_accounts.ron`
- `state_population.ron`
- `fiscal_baseline.ron`
- `pop_needs.ron`
- `industry_assets.ron`

### Phase D1：旧数据适配

先写 adapter，不直接把旧结构复制进新 sim。

### Phase D2：校准报告

启动时输出：

- 目标 GDP。
- 核算 GDP。
- 误差。
- 三产份额。
- 人口误差。
- 就业误差。
- 财政收入/支出误差。

误差超门槛则验收失败。

## 9. 分阶段交付

### Milestone 0：文档和边界

完成当前文档、基础 crate、编译通过。

### Milestone 1：空壳可运行

V9 空 HUD + engine loop + deterministic sim clock。

### Milestone 2：经济核心

历史经济注入、POP、三产 GDP、财政 ledger、市场订单、基础测试。

### Milestone 3：地图渲染核心

FrameGraph、资源质量门禁、基础地图、固定截图。

### Milestone 4：兵牌和地图交互

聚合兵牌、选择、悬停、省份交互、命令覆盖层。

### Milestone 5：V9 面板覆盖

所有旧功能面板完成 V9 迁移。

### Milestone 6：完整玩法闭环

1936 国家选择、时间推进、经济、军事、外交、内容事件、AI 全链路。

### Milestone 7：替换旧主线

只有当功能矩阵、视觉验收、经济验收、性能验收全部通过，才允许讨论合并回主线。

## 10. 禁止事项

- 禁止再创建巨型入口文件。
- 禁止把旧渲染 draw 顺序复制到新入口层。
- 禁止 UI 面板直接操作世界状态。
- 禁止用 GDP 反推建筑。
- 禁止用“数字大于 0”当经济正确性证明。
- 禁止生产路径出现 mock trade route、hardcoded battle arrows、静默 fallback。
- 禁止没有验收指标的“视觉优化”。
