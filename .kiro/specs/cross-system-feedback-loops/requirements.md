# Requirements Document

## Introduction

当前 HOI4-style 游戏中，经济、政治、外交、军事、研究、AI 各自在 `SystemSchedule` 的不同 cadence
（hourly / daily / weekly / monthly）上独立 tick。子系统之间的耦合主要通过共享 `World` 数据结构
被动表达：A 系统改字段，B 系统下次 tick 才"碰巧"读到。结果是玩家做的关键决策（完成 focus、
签条约、打胜仗、占领核心州）在其他模块里几乎看不到回响，世界感觉割裂。

本特性引入一条**显式的跨系统反馈通道**（统称 `Feedback_Bus`），让"重大状态变化"成为一等公民
的领域事件，由订阅方在同一 tick 内或下一 tick 起反应；同时把三条最痛的联动链路实装：
**战斗结果 → 政治稳定度**、**外交事件 → AI 心情/立场**、**领土占领 → 经济产能**。其余链路
（focus → 外交立场、research → 生产能力等）作为后续可扩展点统一接入同一总线。

本特性不改变现有系统的内部算法，也不引入新内容（focus / event / decision 文本仍由
`hoi4-content` 提供）；只补全"系统之间的电线"，并提供玩家可见的反馈表层（通知 feed +
关键弹窗）。

## Glossary

- **Feedback_Bus**: 本特性引入的进程内、单线程、按 tick 排空的领域事件通道，承担跨 crate 解耦。
  存活于 `hoi4-app` 的 `SimContext` 内，生命周期与一次 `tick_hour` 对齐。
- **Domain_Event**: 由某个生产者系统发布的强类型事件值（Rust enum），描述一次"已发生的世界
  状态变化"，例如 `BattleResolved` / `StateOccupied` / `FocusCompleted` / `WarDeclared` /
  `TreatySigned` / `TechResearched`。Domain_Event 不可携带可变引用，只携带 `CountryId`、
  `StateId`、数值差量等纯数据。
- **Producer**: 任意发布 Domain_Event 的系统模块（如 `hoi4-logic::military::arbiter`、
  `hoi4-logic::diplomacy::war`、`hoi4-content::focus`）。
- **Subscriber**: 在 Domain_Event 发生后被调用的反应函数，按订阅注册到 Feedback_Bus；
  反应允许读写 `World` 与 companion states，但 MUST 在同一 tick 内完成。
- **Feedback_Loop**: 一对（Producer, Subscriber）+ 触发条件 + 反应公式构成的命名链路，
  例如 `Loop_Battle_To_Stability`。
- **Notification_Feed**: 玩家可见的滚动事件列表（已存在的 UI 容器或新建），用于显示由
  Domain_Event 派生的人话条目。
- **Modifier_Patch**: 某个 Subscriber 写入 `World` 的具体数值变更集合（增量值 + 持续天数 +
  来源 ID），可被聚合面板查询和工具提示展示。
- **Tick_Boundary**: 一次 `SystemSchedule::tick_hour` 调用的开始或结束。Feedback_Bus 在每次
  `tick_hour` 末尾排空。
- **Player_Country**: `World::player` 指向的 `CountryId`，决定哪些 Domain_Event 触发玩家可见
  通知。

## Requirements

### Requirement 1: Feedback_Bus 基础设施

**User Story:** 作为引擎开发者，我想要一条统一的领域事件通道，这样跨 crate 的"A 影响 B"
就不再依赖偶然的字段读写顺序。

#### Acceptance Criteria

1. THE Feedback_Bus SHALL 暴露 `publish(event: Domain_Event)` 与 `subscribe(kind, handler)`
   两个 API，供 `hoi4-logic` / `hoi4-content` / `hoi4-ai` 调用。
2. THE Feedback_Bus SHALL 在单次 `SystemSchedule::tick_hour` 内保证：所有 hourly producer
   发布的事件，在该 tick 末尾被排空且每个事件至少触发一次对应订阅。
3. WHEN 一个 Subscriber 在处理 Domain_Event 时再次调用 `publish`，THE Feedback_Bus SHALL
   把新事件入队并在同一 tick 内继续排空，直到队列为空或达到 `MAX_CASCADE_DEPTH = 8`。
4. IF Subscriber 级联深度达到 `MAX_CASCADE_DEPTH`，THEN THE Feedback_Bus SHALL 丢弃后续
   入队事件并通过 `tracing::warn!` 记录原始事件类型与级联链。
5. THE Feedback_Bus SHALL 在 `tick_hour` 末尾保证队列为空（不允许跨 tick 残留事件）。
6. THE Feedback_Bus SHALL 是单线程、非 Send，避免引入并发同步开销。
7. WHEN `tick_hour` 完成排空，THE Feedback_Bus SHALL 把本 tick 内发布的全部事件按时间顺序
   写入一个固定容量为 4096 的环形缓冲，供 F3 调试面板读取。
8. THE Feedback_Bus SHALL 提供 `clear_subscriptions()` API，使集成测试可在每个 case 起点
   建立确定的订阅集。

### Requirement 2: 战斗结果 → 政治稳定度 联动（Loop_Battle_To_Stability）

**User Story:** 作为玩家，我希望打了胜仗稳定度涨、丢省份稳定度跌，这样军事胜负能在国内
政治层面立刻被感受到。

#### Acceptance Criteria

1. WHEN `hoi4-logic::military::arbiter` 解算完一场地面战斗，THE Producer SHALL 发布
   `Domain_Event::BattleResolved { attacker, defender, winner, attacker_losses,
   defender_losses, province }`。
2. WHEN `Domain_Event::BattleResolved` 被订阅者 `Loop_Battle_To_Stability` 处理，THE
   Subscriber SHALL 给 winner 国家应用 `stability += 0.001 * winner_kill_ratio`，
   `war_support += 0.002`，clamp 到 `[0.0, 1.0]`。
3. WHEN `Domain_Event::BattleResolved` 被订阅者 `Loop_Battle_To_Stability` 处理，THE
   Subscriber SHALL 给 loser 国家应用 `stability -= 0.0005 * loser_loss_ratio`，
   `war_support -= 0.001`，clamp 到 `[0.0, 1.0]`。
4. WHEN 一国当日累计净 stability 变化超过 `±0.02`，THE Notification_Feed SHALL 写入一条
   "Stability shaken / Stability rallied" 条目（仅对 Player_Country 显示）。
5. THE Loop_Battle_To_Stability SHALL 通过 `Modifier_Patch` 形式记录每次变更的来源
   `BattleResolved#province`，使政治面板的稳定度 tooltip 可列出最近 30 天前 5 个最大贡献来源。

### Requirement 3: 外交事件 → AI 心情/立场 联动（Loop_Diplomacy_To_AI_Mood）

**User Story:** 作为玩家，我希望我对一国宣战、签条约、加入阵营之后，第三国 AI 的态度立即
表现出"恐惧 / 谴责 / 靠拢"的差异，而不是继续按既定剧本走。

#### Acceptance Criteria

1. WHEN `hoi4-logic::diplomacy::war::declare_war` 完成，THE Producer SHALL 发布
   `Domain_Event::WarDeclared { aggressor, target }`。
2. WHEN `hoi4-logic::diplomacy::factions::join_faction` 完成，THE Producer SHALL 发布
   `Domain_Event::FactionJoined { faction, member }`。
3. WHEN `Domain_Event::WarDeclared` 被订阅者 `Loop_Diplomacy_To_AI_Mood` 处理，THE
   Subscriber SHALL 对所有第三国 AI 的 `personality.threat_perception[aggressor]`
   施加 `+0.1 * (1.0 + neighbor_factor)`，其中 `neighbor_factor` 在第三国与 aggressor 接壤
   时取 `1.0` 否则 `0.0`，clamp 到 `[0.0, 1.0]`。
4. WHEN `Domain_Event::WarDeclared` 被订阅者 `Loop_Diplomacy_To_AI_Mood` 处理，THE
   Subscriber SHALL 把 aggressor 与 target 之间的 `world.diplomacy.opinions` 设为
   `min(current, -50)`。
5. WHEN `Domain_Event::FactionJoined` 被订阅者 `Loop_Diplomacy_To_AI_Mood` 处理，THE
   Subscriber SHALL 给同阵营各国之间的 `opinions` 增加 `+25`，clamp 到 `[-200, +200]`。
6. WHEN AI orchestrator 在下一次 daily 评估时读取 `personality.threat_perception`，THE
   AI SHALL 在外交评分中叠加 `threat * 20.0` 作为对该国的敌对得分加权。
7. THE Loop_Diplomacy_To_AI_Mood SHALL 在 Player_Country 是 aggressor / target / faction
   member 之一时，向 Notification_Feed 写入对应条目。

### Requirement 4: 领土占领 → 经济产能 联动（Loop_Occupation_To_Economy）

**User Story:** 作为玩家，我希望占领敌人核心州后，敌人民用/军用工厂立刻不能再开足马力，
而我自己拿到一部分占领产能，这样推线的"经济回报"是即时可见的。

#### Acceptance Criteria

1. WHEN `hoi4-state` 改写一州的 `controllers[state]` 使 `controller != owner`，THE Producer
   SHALL 发布 `Domain_Event::StateOccupied { state, owner, new_controller }`。
2. WHEN `Domain_Event::StateOccupied` 被订阅者 `Loop_Occupation_To_Economy` 处理，THE
   Subscriber SHALL 重算 owner 的 `EconomyState.civ_factories_available` 与
   `mil_factories_available`，把被占领州贡献从可用值中扣除。
3. WHEN `Domain_Event::StateOccupied` 被订阅者 `Loop_Occupation_To_Economy` 处理，
   WHERE 占领方与 owner 处于交战状态，THE Subscriber SHALL 把被占领州工厂数的
   `OCCUPATION_YIELD = 0.20` 比例，作为加成写入 new_controller 的 `civ_factories_available`，
   持续到该州被解放或战争结束。
4. WHEN `Domain_Event::StateOccupied` 被订阅者 `Loop_Occupation_To_Economy` 处理，THE
   Subscriber SHALL 把对应州的 `EconomyState.resources` 贡献从 owner 转移到 new_controller，
   按 `OCCUPATION_YIELD` 比例。
5. WHEN owner 的 `civ_factories_available` 在一次占领事件后跌幅超过 `10%`，THE
   Notification_Feed SHALL 写入 "Industrial heartland under occupation" 条目（仅对
   Player_Country = owner 显示）。
6. WHEN 一州的 controller 重新等于 owner（解放/和平），THE Producer SHALL 发布
   `Domain_Event::StateLiberated { state, owner }`，且 Subscriber SHALL 撤销之前为该州写入
   的全部 Modifier_Patch。

### Requirement 5: 已完成 Focus → 外交立场 联动（Loop_Focus_To_Diplomacy）

**User Story:** 作为玩家，当我完成一个有外交语义的国策（例如"重整军备"、"莱茵兰"），
我希望邻国 AI 的 opinion 与紧张度立刻反映出"对方害怕了"。

#### Acceptance Criteria

1. WHEN `hoi4-content::focus::daily_focus_tick` 把一个 focus 写入 `completed_focuses`，
   THE Producer SHALL 发布 `Domain_Event::FocusCompleted { country, focus_id }`。
2. WHERE focus 数据在 `hoi4-data` 中带有 `tags = ["aggressive"]`，WHEN
   `Domain_Event::FocusCompleted` 被订阅者 `Loop_Focus_To_Diplomacy` 处理，THE Subscriber
   SHALL 把全球 `world.tension` 增加 `0.02`（clamp 到 `[0.0, 1.0]`），并把 country 与所有
   neighbour 之间的 opinions 减 `15`。
3. WHERE focus 数据带有 `tags = ["peaceful"]`，WHEN `Domain_Event::FocusCompleted` 被订阅者
   `Loop_Focus_To_Diplomacy` 处理，THE Subscriber SHALL 给 country 与所有 neighbour 之间的
   opinions 增加 `5`。
4. WHEN Player_Country 是上述 country 或 neighbour，THE Notification_Feed SHALL 写入
   "Focus completed: <name> — neighbours react" 条目并附 opinion 差量。
5. THE Loop_Focus_To_Diplomacy SHALL 仅对带 `aggressive` 或 `peaceful` 标签的 focus 触发，
   其余 focus SHALL 不产生外交副作用。

### Requirement 6: 已完成 Research → 生产能力 联动（Loop_Research_To_Production）

**User Story:** 作为玩家，研究完一项装备/工业科技，我希望生产线立刻吃到加成，AI 也立刻
开始优先选用更优装备，而不必等到下一次 production tick 偶然读到。

#### Acceptance Criteria

1. WHEN `hoi4-logic::research::tick_daily` 把一项科技标记为完成，THE Producer SHALL 发布
   `Domain_Event::TechResearched { country, tech_id, category }`。
2. WHEN `Domain_Event::TechResearched` 被订阅者 `Loop_Research_To_Production` 处理，
   WHERE `category == Industry`，THE Subscriber SHALL 重算
   `EconomyState.factory_output_factor[country]` 应用科技加成。
3. WHEN `Domain_Event::TechResearched` 被订阅者 `Loop_Research_To_Production` 处理，
   WHERE `category == Equipment`，THE Subscriber SHALL 通知 AI production 模块在下一次
   production 评估时把 `tech_id` 引入的新装备型号加入候选池。
4. WHEN Player_Country 完成 Industry 类科技，THE Notification_Feed SHALL 写入
   "Industry boosted: +X% factory output" 条目，X 由本次重算前后差值给出。
5. THE Loop_Research_To_Production SHALL 在事件处理结束前完成 `factory_output_factor`
   重算，确保同一 tick 内随后跑的 production tick 立刻读到新值。

### Requirement 7: 玩家可见反馈表层（Notification_Feed）

**User Story:** 作为玩家，我希望所有跨系统联动都汇集到一个可滚动的 feed，让我无需切面板
就能看到"刚才发生了什么影响什么"。

#### Acceptance Criteria

1. THE Notification_Feed SHALL 保留最近 200 条来自 Feedback_Bus 的、与 Player_Country
   相关的条目，旧条目按 FIFO 丢弃。
2. THE Notification_Feed SHALL 为每条条目记录：游戏日期、Domain_Event 种类、人话标题、
   差量摘要（最多 3 个数值变化）、来源国家 / 州 ID。
3. WHEN 玩家点击一条 Notification_Feed 条目，THE 主面板 SHALL 跳转到对应来源（state →
   定位地图、country → 外交面板、focus → focus 面板）。
4. WHERE Domain_Event 在 `Settings.auto_pause` 中被勾选为关键类（默认包含
   `WarDeclared`、`FocusCompleted`、`StateOccupied` 当对象是 Player_Country 时），
   WHEN 该事件被排空，THE 主循环 SHALL 把 `GameSpeed` 设为 `Paused` 并存储
   `pre_event_speed`，行为与既有 event_scheduler 一致。
5. THE Notification_Feed 渲染 SHALL 在主线程的 UI tick 内完成，不阻塞 SystemSchedule。

### Requirement 8: 调试 / 可观测性

**User Story:** 作为开发者，我需要能离线核对"X 联动到底有没有触发"，否则 bug 会无声潜伏。

#### Acceptance Criteria

1. THE Feedback_Bus SHALL 在 `tracing` target `feedback_bus` 下，对每次 publish 输出
   `info!` 记录（事件类型 + 关键字段）。
2. WHEN F3 调试面板打开，THE 调试 UI SHALL 展示最近 4096 条 Domain_Event 的滚动列表，
   并支持按事件类型过滤。
3. THE 集成测试 SHALL 提供 `assert_event_published(world, Domain_Event::pattern)` helper，
   断言一段 tick 内是否发布了某事件。
4. WHEN 单条 Subscriber 处理耗时超过 `2 ms`，THE Feedback_Bus SHALL 通过 `tracing::warn!`
   记录订阅者标识与耗时，避免 ms/day 预算被默默吞掉。

### Requirement 9: 性能 / 兼容性约束

**User Story:** 作为玩家，我不希望联动机制让模拟变慢；作为开发者，我不希望它把现有 7 个
系统的调度结构推倒重写。

#### Acceptance Criteria

1. THE Feedback_Bus SHALL 在 1939-01-01 满世界状态下，单 game-day 总开销不超过既有
   `ms_per_day` 的 `+10%`（即 5 ms/day baseline 下不超过 5.5 ms/day）。
2. THE Feedback_Bus SHALL 复用既有 `SystemSchedule::tick_hour` 入口，不引入新的全局调度器
   或额外线程。
3. THE Feedback_Bus SHALL 在 `SimContext` 内构造与销毁，使现有 systems schedule 单元测试
   不需要修改即可继续通过。
4. WHEN 集成测试运行 30 天 smoke 测试（既有 `crates/hoi4-integration/tests/smoke_30d.rs`），
   THE 测试 SHALL 在引入 Feedback_Bus 后仍然通过且 ms/day 在预算内。
5. IF 某 Subscriber 抛出 panic，THEN THE Feedback_Bus SHALL 捕获 panic、记录
   `tracing::error!`，丢弃该事件，并继续处理队列其余事件。
