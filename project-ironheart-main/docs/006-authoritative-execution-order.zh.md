# Project Ironheart 权威执行顺序

本文档现在降级为执行顺序专项参考。完整项目级总控路线图以 [`013-project-ironheart-master-rewrite-roadmap.zh.md`](013-project-ironheart-master-rewrite-roadmap.zh.md) 为准；实际开工、提交、验收、合并顺序必须服从 013。本文档只保留阶段顺序细节，不能单独作为总路线图使用。

核心原则：先稳定边界，再迁移系统；先数据契约，再 UI/渲染；先验收夹具，再实现替换。禁止多个系统同时争夺同一数据结构。

## 0. 总规则

### 0.1 阶段锁

每个阶段只能修改指定层。未列入的层只能增加只读 adapter、测试或文档，不能改行为。

### 0.2 单一 owner

每个共享接口同一时间只能有一个 owner：

- `SimulationSnapshot`：由 sim owner 修改。
- `UiFrameModel`：由 UI owner 修改。
- `RendererFrameInput`：由 render owner 修改。
- `EconomySnapshot`：由 economy owner 修改。
- `CommandBus`：由 engine owner 修改。

其它系统需要字段时，必须提交接口请求，不能直接改对方结构。

### 0.3 每阶段出口

每阶段完成必须满足：

- 编译通过。
- 新增或更新 contract tests。
- 文档更新。
- 功能矩阵更新。
- 没有临时 mock 污染生产路径。
- 没有绕过 snapshot 直接读写其它系统内部状态。

### 0.4 禁止跨阶段抢跑

例如：

- 渲染不能在 `SimulationSnapshot` 稳定前要求直接读 `WorldState`。
- UI 不能在 `UiFrameModel` 稳定前直接连财政内部字段。
- 经济不能在 ledger 未完成前让财政面板展示“正确现金流”。
- 兵牌不能在 counter aggregation 未完成前直接画全部师。

## 1. 总执行链

严格顺序如下：

```text
P0 分支和质量门禁
P1 基础 crate 与接口骨架
P2 数据契约和功能矩阵
P3 模拟快照与命令总线
P4 经济核心第一版
P5 财政 ledger 和 POP 闭环
P6 市场订单和建造闭环
P7 V9 UI shell 接快照
P8 渲染 frame graph 和资源质量
P9 地图基础渲染
P10 counter 聚合和地图交互
P11 旧功能面板逐个迁移
P12 AI、内容、事件和长期调度
P13 全功能 replay、视觉、性能验收
P14 替换旧主线准备
```

任何阶段失败，不能进入下一阶段。

## 2. P0：分支和质量门禁

### 目标

建立干净 orphan 重构线，不带旧项目文件，不污染主线。

### 允许修改

- docs。
- workspace skeleton。
- CI/ignore 文件。

### 禁止修改

- 不迁移旧功能。
- 不复制旧 `main.rs`。
- 不引入旧项目命名。

### 出口

- GitHub 分支顶层只包含 `project-ironheart-main/`。
- `cargo check --workspace` 通过。
- 文档说明主线冻结。

当前状态：已完成。

## 3. P1：基础 crate 与接口骨架

### 目标

只定义 crate 边界和最小类型，不做真实玩法。

### 允许修改

- `ironheart-main`
- `ironheart-engine`
- `ironheart-sim`
- `ironheart-render`
- `ironheart-ui`

### 必须产出

- `Engine`
- `SimulationRuntime`
- `Renderer`
- `UiRuntime`
- 最小 `tick`
- 最小 `FramePlan`
- 最小 `UiFrameModel`

### 禁止事项

- 禁止开始迁移经济公式。
- 禁止开始迁移旧 UI 面板。
- 禁止开始移植旧 pass。

### 出口

- 空壳能启动。
- `cargo check --workspace` 通过。
- 每个 crate 有边界说明。

当前状态：基本完成，后续只允许补边界测试。

## 4. P2：数据契约和功能矩阵

### 目标

先定义“要保留哪些功能”和“数据从哪里来”，再写实现。

### 允许修改

- docs。
- `ironheart-sim` schema。
- fixture 数据结构。
- contract test skeleton。

### 必须产出

- `feature_parity_matrix.ron` 或同等文档。
- `historical_economy` schema。
- `population` schema。
- `sector_accounts` schema。
- `fiscal_baseline` schema。
- `render_acceptance_scenes` schema。
- `ui_panel_matrix` schema。

### 禁止事项

- 禁止 UI 直接做面板。
- 禁止 renderer 接入真实地图。
- 禁止经济 tick 改世界。

### 出口

- 每个旧功能都有迁移状态。
- 每个经济核心字段有权威来源。
- 每个验收场景有输入和指标。

## 5. P3：模拟快照与命令总线

### 目标

建立所有系统协作的通道。后续 UI 和 renderer 只认快照，不认内部 World。

### 允许修改

- `ironheart-engine`
- `ironheart-sim`
- 快照类型。
- command 类型。

### 必须产出

```text
SimulationSnapshot
MapSnapshot
EconomySnapshot
MilitarySnapshot
DiplomacySnapshot
UiCommand
GameCommand
CommandBus
```

### 禁止事项

- UI 禁止绕过 command bus。
- renderer 禁止请求 mutable sim。
- economy 禁止把 UI 专用字段写入核心状态。

### 出口

- 能构建完整空快照。
- UI 和 renderer 消费同一个 frame snapshot。
- command round-trip 测试通过。

## 6. P4：经济核心第一版

### 目标

先让 GDP、三产、历史注入、国家账户成立。暂不做完整财政现金流和市场订单。

### 允许修改

- `ironheart-sim::economy`
- 经济 fixture。
- 经济 contract tests。

### 必须产出

- `HistoricalCountryEconomy`
- `HistoricalStateEconomy`
- `NationalAccounts`
- `SectorAccount`
- `GdpAccount`
- `CalibrationReport`

### 执行顺序

1. 定义货币、国家、州、产业 id。
2. 定义历史经济 schema。
3. 定义三产账户。
4. 写 GDP 合计 contract test。
5. 写 1936 主要国家 fixture。
6. 写 calibration report。
7. 只生成 snapshot，不接 UI。

### 禁止事项

- 禁止用 GDP 反推建筑。
- 禁止财政面板读取这些半成品字段。
- 禁止市场系统在此阶段改 GDP。

### 出口

- GDP = 一产 + 二产 + 三产 + 政府 + 调整项。
- 主要国家初始化误差报告可生成。
- GDP 不是单字段硬赋值通过测试。

## 7. P5：财政 ledger 和 POP 闭环

### 目标

让人口、收入、税收、财政现金流有闭环。

### 允许修改

- `ironheart-sim::population`
- `ironheart-sim::finance`
- `EconomySnapshot`
- 经济测试。

### 必须产出

- `PopGroup`
- `LaborPool`
- `PopIncome`
- `PopConsumptionBudget`
- `FiscalLedger`
- `FiscalLedgerEntry`
- `TreasuryAccount`
- `DebtAccount`

### 执行顺序

1. POP 总人口和州人口守恒。
2. 劳动力和就业状态。
3. POP 收入来源。
4. 所得税税基。
5. 财政 ledger。
6. 现金 opening/closing 平衡。
7. 债务和利息 entry。

### 禁止事项

- 禁止直接修改现金字段。
- 禁止税收从 GDP 粗暴比例生成。
- 禁止 POP 消费直接扣市场库存。

### 出口

- `closing_cash = opening_cash + income - expense + financing`。
- `disposable_income = income - tax + transfers`。
- 主要国家人口、就业、税收可解释。

## 8. P6：市场订单和建造闭环

### 目标

把 POP 消费、政府采购、建筑投入、建设材料统一通过 market order。

### 允许修改

- `ironheart-sim::market`
- `ironheart-sim::construction`
- `ironheart-sim::production`
- 经济和市场测试。

### 必须产出

- `MarketOrder`
- `MarketClearing`
- `GoodBalance`
- `ConstructionProject`
- `GovernmentProcurementOrder`
- `ProductionOutput`

### 执行顺序

1. 定义商品和订单。
2. POP 消费生成订单。
3. 建筑投入生成订单。
4. 政府采购生成订单。
5. 市场结算履约和短缺。
6. 建造进度受资金、材料、劳动力影响。
7. 完工生成资产。

### 禁止事项

- 禁止政府采购直接生成装备。
- 禁止建造直接瞬间增加 GDP。
- 禁止市场直接改财政现金。

### 出口

- demand fulfilled <= demand。
- supply used <= supply + stockpile + imports。
- 建设闭环可解释。
- 军事采购和财政支出可追踪。

## 9. P7：V9 UI shell 接快照

### 目标

UI 只接 `UiFrameModel`，先做壳，不做全部面板。

### 允许修改

- `ironheart-ui`
- `ironheart-engine` 的 UI frame model builder。
- UI snapshot tests。

### 必须产出

- V9 topbar。
- V9 side rail。
- V9 modal layer。
- V9 notification layer。
- V9 panel shell。
- UI command mapping。

### 执行顺序

1. V9 tokens。
2. V9 primitives。
3. topbar model。
4. side rail model。
5. command 输出。
6. layout snapshot。

### 禁止事项

- 禁止面板直接读 sim。
- 禁止旧 UI 皮肤。
- 禁止财政/人口面板在数据未 accepted 前宣称完成。

### 出口

- 空/基础 gameplay UI 可显示。
- 不存在第二套 UI 路径。
- token 守卫通过。

## 10. P8：渲染 frame graph 和资源质量

### 目标

先建 frame graph 和质量门禁，再迁移真实地图 pass。

### 允许修改

- `ironheart-render`
- render fixtures。
- render metadata tests。

### 必须产出

- `FrameGraph`
- `FramePlan`
- `RenderNode`
- `RenderResourceRegistry`
- `ResourceQualityReport`
- `VisualAcceptanceScene`

### 执行顺序

1. 定义 render node。
2. 定义资源质量策略。
3. 定义固定截图 metadata。
4. 实现空 pass 执行顺序测试。
5. 实现 fallback 阻断测试。

### 禁止事项

- 禁止直接移植旧 `main.rs` draw 顺序。
- 禁止 pass 自己绕过 graph。
- 禁止资源 fallback 静默通过 accepted。

### 出口

- FramePlan 可测试。
- 资源质量可报告。
- accepted screenshot 必须 critical fallback = 0。

## 11. P9：地图基础渲染

### 目标

在新 frame graph 下接入基础地图，不接兵牌海。

### 允许修改

- `ironheart-render`
- 地图资源 adapter。
- render acceptance scenes。

### 执行顺序

1. 地形基础。
2. 水体。
3. 边界。
4. 静态 decals。
5. 标签。
6. 后处理。
7. 固定截图。

### 禁止事项

- 禁止同时迁移 counters。
- 禁止 UI 直接叠旧面板。
- 禁止 debug mock route 出现在正常场景。

### 出口

- 无 counters 的地图可读。
- 固定场景 metadata 完整。
- 视觉质量报告可生成。

## 12. P10：counter 聚合和地图交互

### 目标

修复当前截图里的兵牌灾难。先聚合、再放置、再绘制。

### 允许修改

- `ironheart-sim::military_snapshot`
- `ironheart-render::counters`
- `ironheart-engine::map_interaction`
- V9 map HUD 的必要 command。

### 必须产出

- `DivisionSnapshot`
- `CounterAggregation`
- `CounterPriority`
- `CounterPlacement`
- `CounterDrawList`
- `MapInteractionCommand`

### 执行顺序

1. division snapshot。
2. zoom class。
3. aggregation。
4. priority。
5. collision solver。
6. draw list。
7. hover/selection 展开。
8. command round-trip。

### 禁止事项

- 禁止所有师全量直画。
- 禁止 renderer 自己查模拟内部部队。
- 禁止 UI 自己决定 counter 位置。

### 出口

- 欧洲战略远景兵牌可读。
- 隐藏/合并数量可报告。
- 选中和 hover 行为通过测试。

## 13. P11：旧功能面板逐个迁移

### 目标

按数据稳定程度迁移面板，不允许所有面板同时动。

### 顺序

1. 设置、存档、调试。
2. 顶栏详情、财政总览。
3. 人口、市场、建造。
4. 政治、法律、决议。
5. 研究。
6. 外交、战争、和平。
7. 陆军、海军、空军、后勤。
8. 事件、通知、结束界面。
9. 主菜单、国家选择、加载页。

### 每个面板流程

```text
PanelSnapshot -> PanelModel -> V9 Layout -> UiCommand -> Engine Command -> Sim Command
```

### 禁止事项

- 禁止两个面板共用未定义临时字段。
- 禁止面板自己计算经济核心数字。
- 禁止绕过 command bus。

### 出口

每个面板必须单独 accepted，不能“UI 系统一次性 accepted”。

## 14. P12：AI、内容、事件和长期调度

### 目标

在经济、UI、渲染边界稳定后接入内容系统和 AI。

### 执行顺序

1. content command adapter。
2. event trigger/effect adapter。
3. focus adapter。
4. decision adapter。
5. AI economy plan。
6. AI military plan。
7. weekly/monthly schedule。

### 禁止事项

- 禁止内容 effect 直接改财政现金。
- 禁止 AI 直接改 UI 状态。
- 禁止事件绕过 command/ledger。

### 出口

- 1936 运行 365 天 deterministic。
- 内容事件可追踪。
- AI 行为不破坏经济账。

## 15. P13：全功能验收

### 目标

证明新线能替换旧线。

### 必须通过

- 功能矩阵全部 accepted。
- 经济 1936 初始化 accepted。
- 德国 90 天经济 accepted。
- 365 天 replay accepted。
- 视觉固定截图 accepted。
- V9 UI snapshot accepted。
- 性能 accepted。
- 没有 critical fallback。
- 没有生产 mock。

## 16. P14：替换旧主线准备

### 目标

只有在 P13 完成后，才准备主线替换。

### 执行

1. 冻结旧主线。
2. 生成迁移报告。
3. 生成风险清单。
4. 开 milestone PR。
5. 人工验收。
6. 合并或保持长期分支。

## 17. 并行规则

允许并行：

- 文档和测试夹具。
- 不同系统的只读 adapter。
- 不共享接口的 UI primitives。
- 不接生产路径的 debug tooling。

禁止并行：

- UI 和 economy 同时改 `EconomySnapshot`。
- render 和 sim 同时改 `MapSnapshot`。
- engine 和 UI 同时改 command 语义。
- 两个面板同时新增同一财政字段。
- pass 迁移和 frame graph 接口修改同时进行。

## 18. 冲突处理

发现系统打架时按优先级处理：

1. 数据权威来源优先。
2. Snapshot 契约优先。
3. Command 契约优先。
4. 测试验收优先。
5. UI 展示最后。

如果 UI 想展示的数据还没有权威来源，UI 必须显示缺失/不可用状态，不能临时造数字。

## 19. 当前下一步

当前已经完成 P0 和 P1 的基础形态。下一步不是继续写 UI 或 renderer，而是进入 P2：

1. 建 `feature_parity_matrix`。
2. 建历史经济 schema。
3. 建 snapshot schema。
4. 建验收场景 schema。
5. 建测试夹具。

P2 没完成前，禁止开始真实地图迁移、旧面板迁移和经济 tick 实现。
