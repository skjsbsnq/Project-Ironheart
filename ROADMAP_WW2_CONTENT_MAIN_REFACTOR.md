# 二战内容重做与 main.rs 拆分路线图

## 目标

把当前分散在 `main.rs`、少量 RON 文件和若干系统中的二战内容，重构成可维护、可扩展、可测试的数据化内容框架。最终目标是支持 1936-1945 的完整二战流程：欧洲战场、抗日战争、太平洋战争、阵营外交、跨海运输、登陆、战线计划、战争结算与新闻/事件节奏。

本路线图强调分阶段推进，不要求一次性完成。优先顺序是：先拆分架构和修底层能力，再补亚洲与欧洲主线，最后扩展全局二战内容。

## 当前状态

### 已有能力

- 顶层应用和大量游戏循环逻辑集中在 `crates/hoi4-app/src/main.rs`。
- 事件系统已有 RON 格式：`crates/hoi4-content/src/event.rs`、`crates/hoi4-content/content/GER_events.ron`、`ITA_events.ron`、`news_events.ron`。
- 国策系统已有 RON 格式：`crates/hoi4-content/src/focus.rs`、`GER_focus_tree.ron`。
- 局势系统已有代码数据结构：`crates/hoi4-content/src/situation.rs`。
- 西班牙内战、无政府派起义、意埃战争已在 `main.rs` 中以 `SituationDef` 硬编码实现。
- 外交系统已有阵营、战争、战争目标、傀儡、军事通行等运行时状态：`crates/hoi4-state/src/diplomacy.rs`。
- 1936 经济/人口/贸易/军事 profile 已有一批：`crates/hoi4-content/content/history_1936`。
- 战线系统已有玩家/AI 共享的 `PlayerArmy`、`FrontlineOrder`、`OffensiveArrow`。
- 当前主经济/军工系统已经重做为 V6/V7 建筑、生产方法、商品市场、政府采购和装备库存体系，不应再按 HOI4 原版“军工厂分配生产线”来设计。
- 军工装备由 `ProductionMethodDef.equipment_output` 产出，典型建筑包括 `arms_industry`、`munition_plant`、`vehicle_factory`、`tank_factory`、`aircraft_factory`、`shipyard`、`machinery_workshop`。
- 海军有舰队、舰船、任务枚举、简化海战仲裁和简化制海权，但仍是原型级系统。
- 空军有联队、任务枚举、简化空战、简化制空权、CAS 加成和战略轰炸函数，但仍是原型级系统。
- AI 已有战略 orchestrator，会调用建设、招募、陆军、海军、空军决策，但没有充分接入 V6/V7 军工经济闭环。

### 主要问题

- `main.rs` 过大，混合了 app 生命周期、UI 分发、事件装载、局势定义、局势效果执行、输入处理、渲染数据收集、调试命令等职责。
- Situation 定义硬编码在 `main.rs`，继续新增二战内容会使文件不可维护。
- 事件库装载写死 `GER_events.ron`、`ITA_events.ron`、`news_events.ron`，新增国家事件需要改代码。
- `SituationEffect` 能力不足，无法干净表达统一战线、强制参战、批量军事通行、阵营创建、精确州转移等二战内容。
- 德国事件很多，但许多内容仍偏事件驱动，不一定有可运行战争、AI 目标和战后结算。
- 中国势力不全，当前历史经济 profile 只有 `CHI`、`MAN`、`MEN`。
- AI 生产逻辑中仍有旧式 `ProductionLine` 路径和 `production_lines` 决策，不能作为当前主生产模型继续扩展。
- AI 建设还没有从装备缺口反推军工建筑、上游工业、原料进口、政府订单和财政承受能力。
- AI 招募主要检查步兵装备和人力，不能根据模板体系、装备结构、战区需求、海空军需求进行长期扩军。
- 没有真正陆军海运/跨海调兵系统，日本本土军队无法自然进入中国战场。
- 战线执行计划会把多个师集中到同一箭头目标，缺少均衡多点进攻。
- 海军 `region_id` 当前实际用 naval base province id 代替战略海区，无法支撑制海权、护航、登陆。
- 海军没有舰队移动、海区巡逻执行、护航/袭击运输闭环、造船到舰队闭环、海军 UI。
- 空军没有真实空区、机场/航程、部署/调动、任务执行完整调度、飞机生产补员闭环、空军 UI。
- 测试覆盖偏局部，缺少 1936-1945 内容回放验收。

## 总体原则

- 先拆职责，再加内容。不要继续往 `main.rs` 里硬塞二战事件和局势。
- 先保证战争可运行，再追求事件完整。能由部队推进解决的，不用事件每日硬转地。
- 数据和执行分离。RON/内容文件描述历史节点，logic/app 只负责加载、校验、调度和执行。
- 军工经济以 V6/V7 建筑、生产方法、商品、库存、财政、政府订单为主，不回退到 HOI4 原版生产线模型。
- AI 扩军必须走建设、生产、库存、训练、补给和运输闭环。事件不得常态化刷兵。
- 事件可以触发战争、外交、动员、援助、政府订单、临时修正、历史新闻，但不能替代 AI 自主建设和扩军。
- 每个阶段都必须有可跑的验收测试或回放脚本。
- 保留现有可用系统，避免一次性推倒所有运行时结构。
- 不为短期兼容保留多套并行实现；迁移完成后删除旧硬编码入口。

## 目标架构

### crate 职责

- `hoi4-content`：内容 schema、RON loader、内容校验、事件/国策/局势/时间线定义。
- `hoi4-state`：纯运行时状态和存档结构，不直接理解具体历史内容。
- `hoi4-logic`：军事、外交、经济、占领、和平、海运、战线等纯逻辑。
- `hoi4-ai`：战略/战术决策，消费 logic API，不直接改外交/战争内部字段。
- `hoi4-ui`：面板和 DTO 展示，不直接承载内容逻辑。
- `hoi4-app`：窗口、渲染、输入、调度、跨 crate 组装，尽量只保留 orchestration。

### main.rs 拆分目标模块

- `crates/hoi4-app/src/app.rs`：`App` 主状态和 winit 生命周期 glue。
- `crates/hoi4-app/src/bootstrap.rs`：地图、数据、世界、内容库初始化。
- `crates/hoi4-app/src/runtime/event_runtime.rs`：事件 scheduler、弹窗、事件 effect 应用入口。
- `crates/hoi4-app/src/runtime/situation_runtime.rs`：局势 tick、start/end、milestone、effect 应用入口。
- `crates/hoi4-app/src/runtime/decision_runtime.rs`：决议 tick 与 activation。
- `crates/hoi4-app/src/runtime/focus_runtime.rs`：国策进度与完成 effect。
- `crates/hoi4-app/src/runtime/systems_runtime.rs`：每日/每小时系统调度。
- `crates/hoi4-app/src/input/mod.rs`：地图点击、框选、快捷键、战线绘制输入。
- `crates/hoi4-app/src/ui_dispatch.rs`：UI command 到 logic API 的分发。
- `crates/hoi4-app/src/render_collect.rs`：地图箭头、前线、标签、兵牌等渲染实例收集。
- `crates/hoi4-app/src/debug_commands.rs`：调试命令、临时工具、日志输出。
- `crates/hoi4-app/src/content_bootstrap.rs`：内容库装载和 validate，不包含具体内容定义。

## Phase 0：审计和护栏✅


目标：在大拆分前建立事实清单，避免拆坏现有功能。

任务：

- 统计 `main.rs` 中职责区块：初始化、事件装载、Situation 定义、effect 应用、UI 命令、输入、渲染收集、tick 调度、调试代码。
- 为每个区块记录函数名、字段依赖、读写 `World` 的范围。
- 建立 `docs/main_rs_split_audit.md`，列出迁移顺序和风险点。
- 跑现有测试：`cargo test` 或至少 `cargo test -p hoi4-content`、`cargo test -p hoi4-logic`、`cargo test -p hoi4-integration`。
- 固定一组烟测命令，作为之后每个 Phase 的回归门槛。

验收标准：

- 有审计文档，能说明 `main.rs` 哪些代码先迁出、哪些暂缓。
- 现有测试基线清楚，失败项有记录，不把旧失败误判为新回归。

主要文件：

- `crates/hoi4-app/src/main.rs`
- `docs/main_rs_split_audit.md`

## Phase 1：内容库装载数据化 ✅

状态：已完成。事件、新闻、国策和决议已通过 `ScenarioContent` / `crates/hoi4-content/content/scenarios/1936.ron` 统一加载；`main.rs` 不再直接 include 具体事件 RON 文件。局势具体 RON 数据化和从 `main.rs` 删除硬编码 `SituationDef` 属于 Phase 2。

目标：让事件、新闻、局势、国策、决议都通过统一 content registry 加载，新增国家内容不再改 `main.rs`。

任务：

- 在 `hoi4-content` 新增 `ContentRegistry` 或 `ScenarioContent`。
- 新增 `crates/hoi4-content/content/scenarios/1936.ron` 或 `content_manifest.ron`。
- manifest 列出要加载的事件库、新闻库、国策树、决议库、局势库、历史 profile。
- 将 `GER_events.ron`、`ITA_events.ron`、`news_events.ron` 从 `main.rs` 写死装载迁移到 registry。
- 保留现有 RON schema，先不强行重写所有内容格式。
- 增加 registry 校验：重复 id、缺失 TriggerEvent 目标、缺失文件、空事件库、重复 Situation id。
- `hoi4-app` 初始化只调用 `load_scenario_content("1936")`。

验收标准：

- 新增一个事件库只需改 manifest，不需改 `main.rs`。
- `GER_events.ron`、`ITA_events.ron`、`news_events.ron` 仍能全部加载并通过校验。
- `main.rs` 中不再直接 include 具体事件 RON 文件。

主要文件：

- `crates/hoi4-content/src/lib.rs`
- `crates/hoi4-content/src/event.rs`
- `crates/hoi4-content/content/scenarios/1936.ron`
- `crates/hoi4-app/src/content_bootstrap.rs`
- `crates/hoi4-app/src/main.rs`

## Phase 2：Situation 数据化与 runtime 拆分 ✅

状态：已完成。`SituationDef`、`SituationSide`、`Intervention`、`SituationEffect`、`ProgressSource` 已支持 serde；西班牙内战、无政府派起义、意埃战争已迁入 `crates/hoi4-content/content/situations/*.ron` 并由 1936 manifest 加载；`main.rs` 已移除具体 `SituationDef` 硬编码，局势 start/tick/AI/effect drain/apply 入口已迁到 `crates/hoi4-app/src/runtime/situation_runtime.rs`。

目标：把西班牙内战、无政府派起义、意埃战争从 `main.rs` 硬编码迁入内容层。

任务：

- 为 `SituationDef`、`SituationSide`、`Intervention`、`SituationEffect`、`ProgressSource` 增加 serde 支持。
- 新增 `crates/hoi4-content/content/situations/spanish_civil_war.ron`。
- 新增 `crates/hoi4-content/content/situations/spanish_anarchist_uprising.ron`。
- 新增 `crates/hoi4-content/content/situations/italo_ethiopian_war.ron`。
- 在 content registry 中加载 situation RON。
- 将 `SituationState::add_def` 的硬编码块从 `main.rs` 删除。
- 将局势 tick、start/end、milestone 迁到 `crates/hoi4-app/src/runtime/situation_runtime.rs`。
- 将 `SituationEffect` 应用逻辑从 `main.rs` 独立成 `apply_situation_effects()`。

验收标准：

- 西班牙内战仍在 1936-07-17 触发。
- 意埃战争仍从开局 active 并创建战争。
- 新闻事件仍能触发。
- `main.rs` 不再包含西班牙/埃塞具体 SituationDef 内容。

主要文件：

- `crates/hoi4-content/src/situation.rs`
- `crates/hoi4-content/content/situations/*.ron`
- `crates/hoi4-app/src/runtime/situation_runtime.rs`
- `crates/hoi4-app/src/main.rs`

## Phase 3：Effect 执行层统一 ✅

状态：已完成。`hoi4-logic/src/scripted_effects.rs` 已成为局势外交/战争效果的统一执行入口；宣战、建阵营、加入阵营、傀儡、军事通行、转州、加核心、吞并等效果已通过 scripted executor 调用 diplomacy/faction/war 逻辑路径。`SituationEffect` 已具备二战必需外交与生成师变体，`situation_runtime` 的外交效果已切到 `hoi4_logic::scripted_effects`，并新增 effect 级测试验证国策/事件路径与局势路径结果一致。

目标：减少 `main.rs` 中散落的直接 world mutation，让事件、国策、决议、局势共用可审计的 effect executor。

任务：

- 盘点 `hoi4-content::focus::Effect` 与 `SituationEffect` 的重叠项。
- 新增 `hoi4-logic/src/scripted_effects` 或扩展 `hoi4-content/src/eval.rs`，形成统一入口。
- 将宣战、吞并、转州、建阵营、加入阵营、傀儡、军事通行等效果改为调用 diplomacy/peace/faction 逻辑 API。
- 给 `SituationEffect` 补齐二战必需变体：`CreateFaction`、`AddToFaction`、`AddWarParticipant`、`GrantMilitaryAccess`、`SetAutonomy`、`TransferState`、`AddCore`、`SpawnDivisionsInStates`。
- 删除事件/局势路径中对 `world.diplomacy.factions`、`world.diplomacy.wars` 的重复手写操作。
- 增加 effect 级测试，覆盖每个外交/战争效果。

验收标准：

- 同一个外交效果从国策、事件、局势触发时结果一致。
- 不能直接按下标操作 faction Vec。
- 宣战后 `countries.at_war`、`diplomacy.wars`、阵营参战传播一致。

主要文件：

- `crates/hoi4-content/src/eval.rs`
- `crates/hoi4-content/src/focus.rs`
- `crates/hoi4-content/src/situation.rs`
- `crates/hoi4-logic/src/diplomacy/*`
- `crates/hoi4-app/src/runtime/*`

## Phase 4：main.rs 第一轮瘦身 ✅

状态：已完成。新增 `bootstrap.rs` 承载 CLI、路径解析、世界/地图/数据初始化和 headless smoke；`content_bootstrap.rs` 已承载 scenario content 装载；新增 `runtime/systems_runtime.rs` 承载小时 tick 调度壳和 speed catch-up 参数；新增 `render_collect.rs` 承载战线、进攻箭头、移动箭头渲染实例收集；新增 `debug_commands.rs` 承载调试日志命令。`main.rs` 已改为通过显式参数调用这些模块，`cargo check -p hoi4-app` 通过。

目标：先迁出低风险 orchestration，不碰复杂游戏规则。

任务：

- 新建 `bootstrap.rs`，迁出世界、地图、数据、历史库、UI 资源初始化辅助函数。
- 新建 `content_bootstrap.rs`，迁出 content registry 装载。
- 新建 `runtime/systems_runtime.rs`，迁出小时/每日 tick 调度壳。
- 新建 `render_collect.rs`，迁出前线、箭头、标签、兵牌渲染数据收集纯函数。
- 新建 `debug_commands.rs`，迁出调试菜单/调试命令。
- 每迁出一个模块就跑最小编译和相关测试，避免大爆炸式移动。

验收标准：

- `main.rs` 行数明显下降，且只保留 App 状态、生命周期、模块调用。
- 无行为变化。
- 所有 moved 函数仍只通过显式参数读写状态，不引入全局单例。

主要文件：

- `crates/hoi4-app/src/main.rs`
- `crates/hoi4-app/src/bootstrap.rs`
- `crates/hoi4-app/src/content_bootstrap.rs`
- `crates/hoi4-app/src/runtime/systems_runtime.rs`
- `crates/hoi4-app/src/render_collect.rs`
- `crates/hoi4-app/src/debug_commands.rs`

## Phase 5：战线执行计划修复

目标：解决执行计划时多个师叠到同一个目标的问题，为大规模二战陆战打基础。

任务：

- 修改 `tick_frontlines` 中 arrow 执行分支，不再把所有 executor 指向 `first_unconquered`。
- 从 `arrow.provinces` 和当前战线邻接关系生成多个当前可攻击目标。
- 为每个目标设置容量上限，MVP 可先固定 3-4 个师。
- executor 按当前位置到目标距离、目标当前堆叠、目标容量做贪心匹配。
- 未分配进攻目标的师继续执行 `frontline_distributor` 的守线目标。
- 箭头推进后滚动目标窗口，避免全军追单点。
- 增加测试：12 个师、4 个接敌目标，执行计划后不能全部同目标，且目标分布稳定。

验收标准：

- 画一条战线和进攻箭头后，执行计划会多点推进。
- 没有进攻目标时不会清空所有守线目的地。
- AI frontlines 复用同一逻辑。

主要文件：

- `crates/hoi4-logic/src/military/frontline.rs`
- `crates/hoi4-ai/src/frontline_ai.rs`
- `crates/hoi4-logic/tests/frontline_api.rs`

## Phase 6：V6/V7 军工经济 AI 重构✅

目标：让 AI 真正基于当前 V6/V7 建筑经济自己建设、生产装备、下政府订单、训练部队，而不是依赖事件刷兵或旧式生产线。

任务：

- 废弃 `hoi4-ai/src/production.rs` 中以 `production_lines` 为主的旧式决策路径，保留或迁移建设部分到新 AI 经济规划器。
- 新增 `hoi4-ai/src/war_economy.rs` 或 `hoi4-ai/src/industrial_planner.rs`，统一处理军工建设、政府订单、进口和库存目标。
- 从当前军队模板、现有师数量、目标师数量、损耗率、训练队列、前线态势计算装备需求。
- 将装备需求映射为 V6/V7 装备类别：`infantry_equipment`、`support_equipment`、`artillery`、`anti_tank`、`anti_air`、`motorized`、`mechanized`、`armor`、`aircraft`、`naval_vessel`、`convoy`、`train`。
- 根据装备缺口调用 `EconomyState::upsert_government_order`，用政府订单拉动军工中间品需求。
- 根据政府订单和市场短缺调用 `construction_planner::plan_construction`，优先建设对应军工建筑和上游工业。
- 对步枪缺口优先 `arms_industry`、`small_arms_parts`、`ammunition`、`steel`、`textiles`。
- 对炮兵缺口优先 `munition_plant`、`gun_barrels`、`artillery_shells`、`steel`、`machinery`、`chemicals`。
- 对坦克缺口优先 `tank_factory`、`tank_hulls`、`armor_plate`、`engines`、`gun_barrels`、`optics`、`radio_sets`。
- 对飞机缺口优先 `aircraft_factory`、`airframes`、`engines`、`aluminium`、`rubber_parts`、`radio_sets`。
- 对舰船/运输船缺口优先 `shipyard`、`steel`、`machinery`、`engines`。
- 如果国内资源不足，AI 应尝试贸易/进口或建设资源建筑/合成产业，而不是盲目继续招募。
- 招募逻辑改为优先进入 `military::training::enqueue_training`，避免直接即时 `spawn_from_template`。
- AI 招募按模板类型、战区需要和库存健康度决定：守线步兵、炮兵加强、机动师、装甲师、驻军、海军陆战/登陆部队。
- 引入“扩军预算”：和平期慢扩军，战争期加速扩军，但受财政、人口、库存、军工能力限制。
- 建立 AI 生产诊断日志：装备缺口、政府订单、建设目标、库存天数、训练队列。

验收标准：

- 禁用事件刷兵后，AI 国家仍能随时间扩军。
- AI 会因为步枪短缺建设 `arms_industry` 或相关上游建筑。
- AI 会因为飞机/运输船需求建设 `aircraft_factory` / `shipyard`。
- AI 训练新师消耗库存、人力和训练时间，不是即时凭空出现。
- 365 天回放中，主要 AI 国家库存、训练队列、建筑队列都有合理变化。

主要文件：

- `crates/hoi4-ai/src/production.rs`
- `crates/hoi4-ai/src/recruit.rs`
- `crates/hoi4-ai/src/orchestrator.rs`
- `crates/hoi4-ai/src/strategic_profile.rs`
- `crates/hoi4-logic/src/economy/construction_planner.rs`
- `crates/hoi4-logic/src/economy/market_tick.rs`
- `crates/hoi4-logic/src/economy/planned_tick.rs`
- `crates/hoi4-logic/src/military/training.rs`
- `crates/hoi4-content/content/economy_v6/production_methods/military.ron`

## Phase 7：海军完整重做 MVP ✅

状态：已完成 MVP。保留 `FleetStore`、`ShipStore`、`NavalMission` 与现有海战 stats，并新增战略海区/港口映射、舰队移动状态、任务 presence、convoy 风险、维修状态、抽象造船补充、AI 任务/移动决策和海军 UI MVP。验证命令：`cargo test -p hoi4-logic --test naval_mvp`、`cargo test -p hoi4-ai --test tactical`、`cargo check -p hoi4-app`。

目标：把当前海军原型升级为可玩、可被 AI 使用、能支撑海运和太平洋战争的系统。

任务：

- 明确当前海军状态：保留 `FleetStore`、`ShipStore`、`NavalMission`、海战 stats，但重做海区、移动、任务执行。
- 加载或自研战略海区映射，替换当前 naval base province id 占位。
- 为港口、海区、舰队当前位置建立稳定关联。
- 给舰队增加移动状态、目标海区、航速、抵达时间、补给/维修状态。
- 实现舰队任务执行：Patrol、StrikeForce、ConvoyEscort、ConvoyRaiding、NavalInvasionSupport。
- 实现海区 presence、侦察、发现概率、交战概率，而不是同 region 直接撮合。
- 实现 convoy 路线，连接贸易、陆军海运、登陆和护航/袭击。
- 将 `convoy` 从抽象库存接入运输容量和损失。
- 实现舰船维修和港口维修容量。
- 实现造船闭环 MVP：`shipyard` 产出 `naval_vessel` / `convoy` 后，AI/玩家可以补充 convoy 或建造抽象舰队单位。
- 后续再细化具体舰型建造，不在 MVP 阶段追求完整舰船设计器。
- 增加海军 UI MVP：舰队列表、所在海区、任务、舰船数量、战损、任务按钮。
- AI 根据战争目标分配舰队：日本护航中国/南进运输线，英国护航帝国航线，美国组建太平洋舰队。

验收标准：

- 舰队能从一个港口/海区移动到另一个海区。
- ConvoyEscort 能降低海运/贸易损失，ConvoyRaiding 能造成损失。
- 陆军海运和登陆能查询海区风险和护航状态。
- AI 能给主要舰队分配合理任务。
- 玩家能看到并修改舰队任务。

主要文件：

- `crates/hoi4-state/src/store.rs`
- `crates/hoi4-logic/src/naval/*`
- `crates/hoi4-ai/src/naval.rs`
- `crates/hoi4-ui/src/*`
- `crates/hoi4-map/src/*`

## Phase 8：空军完整重做 MVP ✅

状态：已完成 MVP。保留 `AirWingStore`、`AirMission`、简化空战、制空权、CAS 与战略轰炸函数，并新增空区/基地容量、联队基地/航程/调动状态、每日任务执行、战略轰炸调度、海军/港口打击、飞机库存补员与预备联队创建、AI 任务复用和中文空军 UI MVP。验证命令：`cargo test -p hoi4-logic --test air_mvp`、`cargo test -p hoi4-ai --test tactical`、`cargo check -p hoi4-ui`、`cargo check -p hoi4-app`。

目标：把当前空军原型升级为可玩、可被 AI 使用、能影响陆战/海战/工业的系统。

任务：

- 明确当前空军状态：保留 `AirWingStore`、`AirMission`、简化空战、制空权、CAS、战略轰炸函数，但重做空区、机场、调动和任务执行。
- 加载或自研战略空区映射，替换当前 `state_id` 占位。
- 定义机场/空军基地容量，关联 state/province/building。
- 给联队增加基地、目标空区、航程、调动时间、补员策略。
- 实现任务执行调度：AirSuperiority、Interception、CloseAirSupport、StrategicBombing、LogisticalStrike、NavalStrike、NavalPatrol、PortStrike、Drop。
- 空战不再只选每区一对最强联队，应按任务、飞机数、任务效率和区域交战率汇总计算。
- CAS 加成接入陆战 arbiter，而不是停留在可调用函数。
- StrategicBombing 接入每日任务 tick，实际破坏建筑/基建并产生日志。
- NavalStrike 接入海军任务和海区舰队目标。
- 飞机生产闭环：`aircraft_factory` 产出 `aircraft` 库存，补充联队或创建新联队。
- AI 根据前线、敌方空军、工业目标、海战需求建立/补充联队。
- 增加空军 UI MVP：联队列表、基地、任务、飞机数量、损失、制空权。

验收标准：

- 空军基地限制联队部署容量。
- 联队能调动到不同空区并执行任务。
- 制空权会影响 CAS、轰炸、登陆/海运风险。
- 战略轰炸能实际破坏建筑或基建。
- AI 能补充飞机并分配任务。

主要文件：

- `crates/hoi4-state/src/store.rs`
- `crates/hoi4-logic/src/air/*`
- `crates/hoi4-logic/src/military/arbiter.rs`
- `crates/hoi4-logic/src/naval/*`
- `crates/hoi4-ai/src/air.rs`
- `crates/hoi4-ui/src/*`
- `crates/hoi4-map/src/*`

## Phase 9：陆军海运 MVP✅

状态：已完成 MVP。`DivisionStore` 已新增陆军海运状态，`military::movement` 支持港口识别、友方港口到友方港口跨海运输命令、convoy capacity 检查、海军 convoy 风险造成延迟/组织度损失、运输中师团不参与陆路移动/占领/陆战；AI 战术层已加入简单跨海增援规则。验证命令：`cargo test -p hoi4-logic --test army_transport_mvp`、`cargo check -p hoi4-ai`、`cargo check -p hoi4-app`。

目标：让日本、英国、美国等岛国/跨海国家能把陆军从本土运到海外友方港口。

任务：

- 定义陆军运输状态：Embarking、AtSea、Disembarking 或等价结构。
- 给 `DivisionStore` 增加跨海移动相关字段，或新增独立 `TransportOrderStore`。
- 实现港口识别：从 `port` / `v6_naval_base` 建筑找到可用 embark/debark state/province。
- 扩展路径规划：陆路不可达时尝试 `land_to_port -> sea_leg -> port_to_target`。
- 消耗 convoy/运输能力，MVP 可按师数量占用抽象 convoy capacity。
- 海运过程中师不可参与陆战，抵达后恢复普通 movement。
- 海运安全先做简化：敌方舰队存在时增加延迟、组织度损失或运输失败概率。
- 为日本 AI 增加简单规则：中日开战后把本土富余师运往朝鲜/满洲/华北友方港口。

验收标准：

- 日本本土师能在非手动瞬移情况下抵达中国战场附近友方港口。
- 英国能把师从本土运到埃及/印度等己方港口。
- 没有港口或 convoy capacity 不足时，命令失败并给出可诊断原因。

主要文件：

- `crates/hoi4-state/src/store.rs`
- `crates/hoi4-state/src/world.rs`
- `crates/hoi4-logic/src/military/movement.rs`
- `crates/hoi4-logic/src/naval/*`
- `crates/hoi4-ai/src/ground_orders.rs`
- `crates/hoi4-ai/src/naval.rs`

## Phase 10：海区、护航与登陆基础✅

状态：已完成 MVP。海运 route 已从相邻海省 raw id 改为基于 `SeaRegionMap::from_world()` 的战略海区 id；`ConvoyEscort`、`ConvoyRaiding`、`Patrol` 和 `NavalInvasionSupport` 已能通过同一 route/presence 影响海运风险或登陆支援；新增 `order_naval_invasion()`，要求目标为敌控沿海陆省、7 天准备时间、convoy capacity 和最低登陆支援 presence，满足后复用陆军海运状态执行登陆运输。验证命令：`cargo test -p hoi4-logic --test army_transport_mvp`、`cargo check -p hoi4-logic`、`cargo check -p hoi4-ai`、`cargo check -p hoi4-app`。

目标：把海运从“港口瞬间连线”推进到可被舰队任务影响的战略海区系统。

任务：

- 加载 `map/strategicregions/*.txt` 或建立自研海区映射。
- 将 `FleetStore.region_id` 从 naval base province id 改为真正海区 id。
- 为港口 state/province 关联最近海区。
- 海运 route 记录经过的海区列表。
- `ConvoyEscort`、`ConvoyRaiding`、`Patrol`、`StrikeForce` 对海运风险产生影响。
- 新增登陆计划 MVP：从己方港口到敌方沿海省，需准备时间、运输容量、海军支援。
- 后续再接入登陆艇科技、登陆上限、制海权阈值。

验收标准：

- 舰队任务能影响海运安全。
- 敌方潜艇/舰队可干扰 convoy。
- 简单登陆可以执行，但受准备时间和容量限制。

主要文件：

- `crates/hoi4-map/src/*`
- `crates/hoi4-state/src/store.rs`
- `crates/hoi4-logic/src/naval/*`
- `crates/hoi4-logic/src/military/movement.rs`
- `crates/hoi4-ai/src/naval.rs`

## Phase 11：中国全势力 1936 重建

目标：把中国地区从单一 `CHI` 粗略数据扩展为完整可玩的 1936 中国局势。

任务：

- 确认 vanilla tag 和地图 state id：`CHI`、`PRC`、`SHX`、`GXC`、`YUN`、`XSM`、`SIK`、`TIB`、`MAN`、`MEN`。
- 为缺失势力新增 `history_1936/countries/*.ron`。
- 扩展 `force_profiles_1936.ron`，给每个势力定义兵力、军费、装备需求。
- 扩展 `head_of_state_1936.ron`，补齐领袖。
- 扩展 `state_population.ron`，把中国人口从少数大块拆到各势力核心州。
- 扩展 `state_deposits.ron`，补华北煤铁、华南钨、云南资源等。
- 修正 `hoi4-data/content/history_1936/state_owners.ron`，确保州归属、核心、满洲、蒙疆、租界符合目标开局。
- 扩展 `v7_history_loader.rs` 和 `history_1936_loads.rs`。

验收标准：

- 所有中国主要势力出现在国家列表，并有领袖、人口、经济、军队 profile。
- 满洲/蒙疆为日本傀儡关系仍成立。
- 中国各势力拥有正确州与核心，不再全部归入 `CHI`。

主要文件：

- `crates/hoi4-content/content/history_1936/countries/*.ron`
- `crates/hoi4-content/content/history_1936/military/force_profiles_1936.ron`
- `crates/hoi4-content/content/history_1936/politics/head_of_state_1936.ron`
- `crates/hoi4-content/content/history_1936/states/state_population.ron`
- `crates/hoi4-data/content/history_1936/state_owners.ron`
- `crates/hoi4-content/src/v7_history_loader.rs`
- `crates/hoi4-content/tests/history_1936_loads.rs`

## Phase 12：七七事变与抗日统一战线

目标：实装 1937-07-07 日本全面侵华和中国各势力统一抗战。

任务：

- 新增 `content/events/JAP_events.ron`。
- 新增 `content/events/CHI_events.ron`。
- 扩展 `news_events.ron`：卢沟桥事变、上海会战、南京陷落、武汉会战、重庆大轰炸、援华路线、抗战胜利。
- 新增 `situations/second_sino_japanese_war.ron`。
- on_start 创建 `JAP/MAN/MEN` vs `CHI` 战争。
- 创建或强制加入“中国抗日统一战线”阵营。
- 让 `PRC/SHX/GXC/YUN/XSM/SIK` 等加入中方战争，互相授予军事通行。
- 给日本傀儡参战。
- 在华北、满洲边境、上海/华东沿海调动已有部队；如必须补足历史 OOB，应通过训练队列、动员、政府订单和预置 OOB 数据解决，避免事件即时刷兵。
- 进度由战区 territorial control 推导，里程碑按华北、上海、南京、武汉、重庆方向触发。
- 中方获得事件/决议援助：苏联援华、滇缅路、美国租借法案后援助。

验收标准：

- 到 1937-07-07 自动触发卢沟桥事变。
- 日本与中国集团进入战争。
- 中国各势力能共同防御，军事通行有效。
- 日本能通过海运或大陆前线持续投入兵力。
- 战争进度不是单纯事件转地，而是由战场控制推动。

主要文件：

- `crates/hoi4-content/content/events/JAP_events.ron`
- `crates/hoi4-content/content/events/CHI_events.ron`
- `crates/hoi4-content/content/news_events.ron`
- `crates/hoi4-content/content/situations/second_sino_japanese_war.ron`
- `crates/hoi4-ai/src/strategic_profile.rs`
- `crates/hoi4-ai/src/ground_orders.rs`

## Phase 13：欧洲战争主线 1938-1941 ✅

状态：已完成 MVP。德国原有事件链已审计并保留；新增英法苏事件库、欧洲新闻事件库，以及 `poland_campaign`、`weserubung_and_western_campaign`、`mediterranean_war`、`greco_italian_war`、`barbarossa` 数据化局势。1936 scenario manifest 已加载这些内容；波兰战争会创建德国 vs 波兰战争并拉入英法/Allies，西线、意大利参战、希腊、巴巴罗萨会按历史日期形成可运行战争主线。验证命令：`cargo test -p hoi4-content --test phase13_european_war`、`cargo check -p hoi4-app`。完整投降、和平会议和 scripted surrender 属于 Phase 15。

目标：把德国事件从“事件链”升级为可运行欧洲战争流程。

任务：

- 审计 `GER_events.ron` 中 Anschluss、Sudeten、Munich、Memel、Danzig、Poland War、Warsaw Falls、Poland Partition 的实际效果。
- 将波兰战役做成 `situation/poland_campaign.ron` 或明确的 war timeline。
- 确认英法对德宣战和 Allies 阵营创建/加入。
- 新增丹麦/挪威、低地国家、法国战役事件和局势。
- 新增意大利参战、阿尔巴尼亚、希腊、北非的事件/局势。
- 新增苏联相关事件：东方波兰、波罗的海、冬季战争、巴巴罗萨准备。
- 新增 `situation/barbarossa.ron`，驱动德苏战争。

验收标准：

- 1939 波兰战争可由部队推进并结束。
- 英法参战后欧洲大战形成。
- 德苏战争能在 1941 触发并形成长期东线。

主要文件：

- `crates/hoi4-content/content/GER_events.ron`
- `crates/hoi4-content/content/events/SOV_events.ron`
- `crates/hoi4-content/content/events/ENG_events.ron`
- `crates/hoi4-content/content/events/FRA_events.ron`
- `crates/hoi4-content/content/situations/*.ron`

## Phase 14：太平洋战争 1941-1945 ✅

状态：已完成 MVP。新增日本太平洋事件库、美国事件库、太平洋新闻事件库，以及 `pacific_war` 和 `southern_resource_area` 数据化局势。1941-12-07 珍珠港会触发日本对美国和英国战争、美国参战、太平洋 Allies/United Nations 组建与英联邦/殖民地参战；1941-12-08 南方资源区会触发马来亚、菲律宾、荷属东印度方向战争目标和日本 AI 优先级 flag。内容不使用即时吞并或硬转州，推进仍依赖海运、登陆、舰队护航/袭击和战场控制。验证命令：`cargo test -p hoi4-content --test phase14_pacific_war`、`cargo test -p hoi4-content --test phase13_european_war`、`cargo check -p hoi4-app`。完整日本投降、岛屿反攻结算和战后和平仍属于 Phase 15/16。

目标：在海运、海区、登陆基础上实装日本南进和美国参战。

任务：

- 新增珍珠港事件链。
- 新增日本南进决策/事件：菲律宾、马来亚、新加坡、荷属东印度、缅甸。
- 新增美国参战、太平洋舰队、租借法案、岛屿反攻事件。
- 新增荷属东印度/英属马来亚资源目标和日本资源压力。
- AI 支持跨海调兵、护航、登陆支援、岛屿目标选择。
- 用 convoy、海区风险和舰队任务影响太平洋推进速度。

验收标准：

- 1941 后日本能对美英荷开战。
- 日本可以通过海运/登陆推进东南亚。
- 美国能跨海投入兵力反攻。

主要文件：

- `crates/hoi4-content/content/events/JAP_events.ron`
- `crates/hoi4-content/content/events/USA_events.ron`
- `crates/hoi4-content/content/situations/pacific_war.ron`
- `crates/hoi4-ai/src/naval.rs`
- `crates/hoi4-ai/src/ground_orders.rs`

## Phase 15：战争结束、投降与和平会议 ✅

状态：已完成 MVP。新增 `hoi4-content::surrender` 数据 schema、scenario manifest 的 `surrender_libraries`、`content/surrender/major_surrenders.ron` 和投降新闻事件库；新增 `hoi4-logic::diplomacy::surrender`，按首都丢失与核心州控制率评估主要国家 scripted surrender，并复用既有 scripted diplomatic effects 与 `SituationEffect` 执行退战、flag、新闻、稳定/战争支持等结算。`hoi4-app` 每日局势 tick 后会评估 scripted surrender 并把新闻事件推入现有事件触发队列。当前 MVP 覆盖法国、意大利、德国、日本投降路径；完整和平会议 UI、复杂分区占领、政府流亡和战后会议仍可后续扩展。验证命令：`cargo test -p hoi4-content --test phase15_surrender_content`、`cargo test -p hoi4-logic --test scripted_surrender`、`cargo check -p hoi4-app`。

目标：让大型战争有合理终局，不靠单个事件硬吞并所有国家。

任务：

- 审计现有 `diplomacy/peace.rs` 能力。
- 建立 surrender progress：胜利点、首都、核心州控制、战争支持、稳定度。
- 为主要国家定义 scripted surrender 条件：法国投降、意大利投降、德国投降、日本投降。
- 支持政府流亡/傀儡/占领区/分区占领。
- 支持战后转州、释放国家、傀儡、阵营解散或冷战前置。
- 先做 scripted peace，后续再做完整和平会议 UI。

验收标准：

- 法国、德国、日本等主要国家不会无限战争到 1945 后仍无结算。
- 战后州归属和占领状态可解释、可测试。

主要文件：

- `crates/hoi4-logic/src/diplomacy/peace.rs`
- `crates/hoi4-state/src/diplomacy.rs`
- `crates/hoi4-content/content/surrender/*.ron`
- `crates/hoi4-ui/src/diplomacy.rs`

## Phase 16：1936-1945 回放验收

目标：建立完整内容回归门槛，防止后续新增内容破坏历史节奏。

任务：

- 新增 30/365/1095/3285 天 smoke replay。
- Replay 记录关键日期状态：战争数量、阵营成员、主要国家存活、主要战线控制、经济崩溃状态。
- 1937-07-07 验证中日战争触发。
- 1939-09-01 验证欧洲大战形成。
- 1941 验证德苏战争和太平洋战争可触发。
- 1945 验证至少存在可结束路径或 scripted surrender。
- 对 AI 性能设上限，防止全球战争后 tick 爆炸。

验收标准：

- Replay 可在 CI 或本地稳定运行。
- 历史关键节点不因内容改动静默失效。
- 性能退化能被测试发现。

主要文件：

- `crates/hoi4-integration/tests/*`
- `crates/hoi4-logic/tests/*`
- `crates/hoi4-content/tests/*`

## 推荐执行顺序

1. Phase 0：先审计，不改大结构。
2. Phase 1：统一内容装载，打通新增内容入口。
3. Phase 2：Situation 数据化，移出西班牙/埃塞硬编码。
4. Phase 4：第一轮 `main.rs` 瘦身，把低风险代码迁出。
5. Phase 3：统一 effect executor，避免新增内容继续散写 world mutation。
6. Phase 5：修战线执行计划分兵。
7. Phase 6：重构 V6/V7 军工经济 AI，让 AI 自己建设、生产、训练。
8. Phase 7：海军完整重做 MVP，先有舰队移动、任务和 convoy 风险。
9. Phase 8：空军完整重做 MVP，先有空区、基地、任务和补员闭环。
10. Phase 9：做陆军海运 MVP，接入 convoy 和海军风险。
11. Phase 10：补海区、护航、登陆，支撑太平洋。
12. Phase 11：补全中国 1936 势力和数据。
13. Phase 12：做七七事变和抗日统一战线。
14. Phase 13：欧洲主线。
15. Phase 14：太平洋主线。
16. Phase 15：战争结束和和平。
17. Phase 16：长期回放验收。

## 风险清单

- 过早重写所有内容格式会拖慢进度。应先 registry 化，再逐步迁移 schema。
- `main.rs` 一次性大拆容易引入生命周期和借用问题。应按低风险函数逐块迁移。
- 如果 AI 军工经济不先接入 V6/V7 建筑、PM、库存、训练闭环，二战内容会继续被迫依赖事件刷兵。
- 旧式 `ProductionLine` 路径容易误导后续开发，应明确降级为遗留路径或删除。
- 海军和空军虽然有原型代码，但不能当作可玩系统依赖；太平洋战争前必须重做。
- 海运如果不先做，日本和太平洋内容会大量依赖瞬移事件，后续难清理。
- 战线分兵不修，大规模陆战会出现堆叠冲锋，历史内容越多越明显。
- 中国势力拆分会影响人口、经济、州归属、AI、外交、贸易，需要配套测试。
- 直接硬转州或刷师可快速做事件，但会破坏“战争由战场和经济推动”的目标，应仅用于无血吞并、战后结算或明确历史 OOB 初始化。
- 战略海区改造会影响舰队、贸易封锁、海运、登陆，要分 MVP 和完整版本。
- 空区改造会影响空战、CAS、轰炸、海运风险、AI 性能，要先做 MVP 再扩大。
- 内容扩展会放大 AI 性能问题，必须同步做 replay 性能门槛。

## 最小可交付切片

如果需要先做一个能看到成果的版本，建议切片如下：

- 内容 registry 加载事件库和 Situation 库。
- 西班牙内战从 `main.rs` 迁到 RON。
- 战线执行计划多目标分兵。
- AI 通过 V6/V7 军工建筑、政府订单、库存和训练队列自主扩军。
- 海军 MVP 有舰队移动、convoy 护航/袭击风险。
- 空军 MVP 有空区、机场、制空、CAS/轰炸基本闭环。
- 陆军海运 MVP，只支持友方港口到友方港口。
- 中国新增 `PRC/SHX/GXC/YUN/XSM/SIK/TIB` profile 和州归属。
- 1937-07-07 触发中日战争与统一战线。
- 日本能把本土训练出的师运到大陆友方港口。
- 365 天 replay 验证中日战争持续运行且不崩溃。
