# 需求文档

## 引言

本文档为玩家加入 vanilla HOI4 的"划线战线"交互。玩家把选中的师团编成一支集团军（Army），在地图上沿己方/接敌省份画出一条战线，游戏自动把该集团军的师团均匀分布到这条线上作为防御驻军。玩家还可以在已有战线上叠加一条进攻箭头，让该集团军借由现有的每日移动系统跨过战线推进入敌境。战线会动态响应控制权变化：省份易手时，师团自动重新分布到剩余的合法前线省。AI 地面调度必须尊重玩家发出的集团军指令，不得覆盖这些师团的目的地。

本特性复用现有基础设施：

- 玩家选中师团：`crates/hoi4-app/src/main.rs` 中的 `App.selected_divisions: Vec<usize>`
- 前线探测：`hoi4_ai::frontline::compute_segment`（state 级邻接）
- 移动引擎：`hoi4_logic::military::movement::{daily_movement_tick, can_enter_province, next_step}`
- 战斗仲裁器：`crates/hoi4-logic/src/military/arbiter.rs`（已支持多师 8v8）
- 指挥层级类型：`hoi4_state::command::{Army, CommandHierarchy}`
- 地图箭头渲染：`crates/hoi4-app/src/passes/maparrow.rs`（`MapArrowPass`、`ArrowInstance`）

## 术语表

- **集团军（Army）**：一组玩家师团下标的命名容器，共享同一条战线指令。挂在 `World` 上，存为 `player_armies: Vec<PlayerArmy>`，按 `ArmyId` 索引。
- **ArmyId**：集团军的稳定 `u32` 标识，UI 与指令使用。
- **战线指令（Frontline_Order）**：附在某支集团军上的持久玩家指令。包含一条有序的友方陆地省 id 列表（战线路径）+ 可选的进攻箭头。
- **战线路径（Frontline_Path）**：构成集团军防线的有序、去重陆地省 id 序列。每对相邻条目在 `world.map.adjacencies` 中陆地相邻，每条目的当前控制者为玩家国家或共战盟友。长度限制 1..=64 省。
- **进攻箭头（Offensive_Arrow）**：可选的有序敌方控制陆地省 id 序列，集团军在战线填满后沿此推进。锚定在战线路径上的某一省（锚点省）。
- **锚点省（Anchor_Province）**：战线路径上发出进攻箭头的省份；必须与进攻箭头第 1 个省陆地相邻。
- **分布方案（Distribution_Plan）**：分布器为每个集团军成员师团分配到战线路径上某一目标省的确定性映射。
- **战线分布器（Frontline_Distributor）**：把集团军（N 个师）映射到战线路径（M 个省）的纯函数，每省分到 `floor(N/M)` 或 `ceil(N/M)` 个师。
- **战线吸附器（Frontline_Snapper）**：把玩家画的折线采样点转换成合法战线路径的函数：每个采样点吸附到最近的合法陆地省，缺口用陆地邻接 BFS 补连。
- **合法省（Eligible_Province）**：玩家国家或共战盟友控制的陆地省，且：要么与某敌方控制陆地省相邻（前线省），要么位于两个前线省之间的 BFS 路径上。
- **战线绘制模式（Army_Painter）**：玩家拖动鼠标画战线时的 UI 模式。
- **箭头绘制模式（Arrow_Painter）**：玩家在已有战线上画进攻箭头时的 UI 模式。
- **玩家国家（Player_Country）**：`world.player`，人类操控的 `CountryId`。
- **共战国（Co_Belligerent）**：与玩家国家在同一阵营（按 `world.diplomacy.faction_of`）或与玩家国家有军事通行权的国家。
- **每日 tick（Daily_Tick）**：由 `hoi4-logic/src/military/movement.rs` 的 `daily_movement_tick` 等驱动的每日一次扫描。
- **AI 接管保护（AI_Override_Guard）**：阻止 `hoi4_ai::ground_orders::execute_ground_orders` 改写玩家国家集团军成员师 `divisions.destinations[i]` 的规则。

## 需求

### 需求 1：集团军的创建与解散

**用户故事：** 作为玩家，我想把选中的师团编成一支命名集团军，这样可以一次给多个师下达同一条战线指令。

#### 验收标准

1. WHEN 玩家在 `selected_divisions` 中至少选中一个师并按下"创建集团军"UI 控件，THE 战线系统 SHALL 创建一个由玩家国家拥有、包含这些师下标的新集团军，并 SHALL 返回一个不与任何已存在 ArmyId 相同的全新 ArmyId。
2. THE 战线系统 SHALL 拒绝创建集团军当 `selected_divisions` 包含任何 `owners[i]` 不为玩家国家的师，返回"非所有者"错误，并不创建集团军。
3. WHEN 玩家携带 ArmyId 调用"解散集团军"，THE 战线系统 SHALL 把该集团军从 `world.player_armies` 中移除、清除其在任何战线指令中的成员引用，并 SHALL 让每个原成员师的 `destinations[i]` 保持原值。
4. THE 战线系统 SHALL 保证任意时刻每个师下标至多归属于一个集团军。
5. WHEN 玩家携带 ArmyId 与师下标列表调用"加入师团"，THE 战线系统 SHALL 把这些师转入目标集团军（先从其它集团军移除），前提是每个师都归玩家国家所有。
6. WHEN 玩家携带 ArmyId 与师下标列表调用"移除师团"，THE 战线系统 SHALL 从该集团军的成员列表中删除这些下标，且 SHALL 在没有成员剩余时让该集团军保持空壳而不删除。
7. IF 集团军的成员师被销毁（其下标从 `DivisionStore` 中移除，或其 `strength[i]` 归零），THEN THE 战线系统 SHALL 在下一个每日 tick 把该下标从该集团军中移除。

### 需求 2：绘制战线

**用户故事：** 作为玩家，我想沿地图拖动鼠标为集团军画一条战线，这样我能告诉师团去哪儿守。

#### 验收标准

1. WHILE 玩家选中了某支集团军且战线绘制模式激活，WHEN 玩家按下并拖动左键划过地图，THE 战线系统 SHALL 以不低于 30 次/秒的采样率收集鼠标轨迹的地图采样点序列。
2. WHEN 玩家在战线绘制模式下松开左键，THE 战线吸附器 SHALL 把捕获的采样点转换成由合法省构成的战线路径，并 SHALL 把它作为选中集团军的活跃战线指令。
3. THE 战线吸附器 SHALL 产出每对相邻省都在 `world.map.adjacencies` 中陆地相邻的战线路径。
4. THE 战线吸附器 SHALL 产出无重复省 id 的战线路径。
5. THE 战线吸附器 SHALL 把战线路径长度限制在闭区间 `[1, 64]` 之内，对超过 64 省的画线在第 64 个之后截断，并报告"路径已截断"警告。
6. IF 捕获的采样点不包含任何合法省，THEN THE 战线系统 SHALL 丢弃此次绘制手势、保留任何已有战线指令，并发出"无合法前线"信息。
7. WHEN 玩家在已有战线指令的集团军上重新画一条战线路径，THE 战线系统 SHALL 原地替换之前的战线路径，并 SHALL 清除其关联的进攻箭头。

### 需求 3：师团沿战线分布

**用户故事：** 作为玩家，我想让集团军的师团沿画出的战线均匀展开，这样不用我一师一师地微操就能覆盖整条线。

#### 验收标准

1. WHEN 一支集团军接收到或更新带有 N 个师在 M 个省上的战线指令，THE 战线分布器 SHALL 产出一份将每个师恰好分配到战线路径上某一省的分布方案。
2. THE 战线分布器 SHALL 保证战线路径上任一省份分到的师数为 `floor(N/M)` 或 `ceil(N/M)`。
3. WHEN N 大于 0 且 M 大于 0 且 N ≥ M，THE 战线分布器 SHALL 产出一份分布方案使得战线路径上每个省至少分到一个师。
4. WHEN N 小于 M，THE 战线分布器 SHALL 沿战线路径按下标均匀挑出 N 个目标省（例如 M=10、N=3 时挑出的下标 SHALL 为 0、4、8 或等价的均匀分布），并 SHALL 让其余省份保持未分配。
5. THE 战线分布器 SHALL 优先把每个师分配给陆地 BFS 距离最近的目标省，距离相同时按更小的师下标取胜，从而保证同一世界状态连续两次运行产出相同的分布方案。
6. WHEN 战线分布器为某个师产出目标省，THE 战线系统 SHALL 把 `world.divisions.destinations[div_idx]` 设为该省，除非该师已经在该省，此时 SHALL 把 `destinations[div_idx]` 设为 None。
7. THE 战线系统 SHALL 在每个每日 tick 对所有持有活跃战线指令的集团军重新计算分布方案。
8. IF 战线路径上的某个师当前在战斗中（`in_combat[i] == true`），THEN THE 战线系统 SHALL 不在该次每日 tick 修改其目的地。

### 需求 4：战线上的防御姿态

**用户故事：** 作为玩家，我想让集团军守住战线而不是追击敌人，这样突破不会把我整条线挖空。

#### 验收标准

1. WHILE 一支集团军持有战线指令且无进攻箭头，THE 战线系统 SHALL 不把任何成员师的目的地设到一个控制者对玩家国家敌对的省份。
2. WHEN 一个敌方师进入集团军成员所守战线路径省的某个相邻省，THE 战线系统 SHALL 让该防守师停留在它被分配的战线路径省上，并 SHALL 依赖 `daily_movement_tick` 的战斗阻挡逻辑在敌方踏上战线路径时进行交战。
3. WHEN 战线路径上某个省的控制者从玩家国家翻为敌方控制，THE 战线系统 SHALL 在下一个每日 tick 把该省从活跃战线路径中删除，并 SHALL 在剩余合法省上重新计算分布方案。
4. WHEN 战线路径上每个省都已被敌方控制，THE 战线系统 SHALL 让该集团军的战线指令失活、清除全部成员的目的地，并发出"战线已崩溃"信息。
5. THE 战线系统 SHALL 不发出与 `daily_movement_tick` 中破碎师撤退逻辑冲突的撤退指令；破碎师 SHALL 继续走该撤退逻辑，且 SHALL 在 `organisation/max_organisation < 0.05` 或 `strength < 0.05` 期间被战线分布器跳过。

### 需求 5：绘制进攻箭头

**用户故事：** 作为玩家，我想从战线上画一支进攻箭头到敌境，这样我的集团军能沿选定方向推进。

#### 验收标准

1. WHILE 一支集团军有活跃战线指令且箭头绘制模式激活，WHEN 玩家从战线路径上的某省拖动光标进入相邻的敌方控制省，THE 战线系统 SHALL 把该手势记为候选进攻箭头。
2. WHEN 玩家松开鼠标，THE 战线吸附器 SHALL 把该候选手势转换为一支首省与战线路径上锚点省陆地相邻、且每对相邻省都陆地相邻的进攻箭头。
3. THE 战线吸附器 SHALL 把进攻箭头长度限制在闭区间 `[1, 32]` 之内，对超过 32 省的手势在第 32 省截断，并报告"箭头已截断"警告。
4. IF 候选进攻箭头中存在某省与上一省不陆地相邻，THEN THE 战线吸附器 SHALL 用陆地邻接 BFS 在不超过 6 跳深度内补桥，且 SHALL 在不存在这样的桥时拒绝该箭头。
5. WHEN 一支进攻箭头被附加到战线指令上，THE 战线系统 SHALL 替换该指令上之前的进攻箭头。
6. THE 战线系统 SHALL 允许玩家通过"取消箭头"UI 控件清除进攻箭头而不清除战线路径。

### 需求 6：进攻箭头的执行

**用户故事：** 作为玩家，我想让集团军在画完箭头后真的沿箭头推进，这样指令在模拟中才有效果。

#### 验收标准

1. WHILE 一支集团军同时持有活跃战线路径与进攻箭头，每个每日 tick THE 战线系统 SHALL 挑选数量为 `ceil(N * 0.5)` 的成员师，并 SHALL 把它们的目的地设为进攻箭头上下一个未被攻克的省。
2. THE 战线系统 SHALL 从距锚点省 BFS 距离最近的师开始挑选进攻箭头的执行者，距离相同时按更小的师下标取胜，使得选择确定。
3. THE 战线系统 SHALL 把剩余 `floor(N * 0.5)` 个师留在战线路径的防御位置上，确保推进背后的线不丢。
4. THE 战线系统 SHALL 仅在 `can_enter_province` 对该师与目标省返回 true 时才把目的地设到进攻箭头上。
5. WHEN 进攻箭头上每个省都已被玩家国家控制（controller 等于玩家国家），THE 战线系统 SHALL 把该进攻箭头从指令中移除，并发出"箭头已完成"信息；战线路径 SHALL 保持活跃。
6. IF 锚点省已不在战线路径上（因为它易手或被移除），THEN THE 战线系统 SHALL 选择 BFS 距离最近的剩余战线路径省作为新锚点省；若不存在这样的省，THE 战线系统 SHALL 清除进攻箭头。

### 需求 7：战线的动态重算

**用户故事：** 作为玩家，我想让战线在战况变化时自动调整，这样我不必每次战斗后重画。

#### 验收标准

1. WHEN 每日 tick 开始时战线路径上某省的控制者既不是玩家国家也不是共战国，THE 战线系统 SHALL 在重新计算分布方案之前把该省从活跃战线路径中删除。
2. WHEN 每日 tick 开始时战线路径上某省没有任何陆地相邻的敌方控制省，THE 战线系统 SHALL 在重新计算分布方案之前把该省从活跃战线路径中删除。
3. WHEN 删除非法省后战线路径变空，THE 战线系统 SHALL 让战线指令失活，并发出"战线已崩溃"信息。
4. WHEN `world.diplomacy.is_at_war(玩家国家)` 变为 false，THE 战线系统 SHALL 让玩家国家拥有的每条战线指令失活，并 SHALL 让成员师团原地不动、不带目的地。
5. THE 战线系统 SHALL 用 `compute_segment(world, 玩家国家, enemy)` 对每个与玩家国家交战的敌国重算合法省集合，并 SHALL 把这些 state 级前线段与玩家画的战线路径合并以校验合法性。

### 需求 8：玩家手动指令的优先级

**用户故事：** 作为玩家，我想让 AI 地面调度器不要碰我的集团军师团，这样我的指令在下一次 AI tick 后还在。

#### 验收标准

1. WHILE 某个师属于一支带活跃战线指令的**玩家**集团军，THE 战线系统 SHALL 阻止 `hoi4_ai::ground_orders::execute_ground_orders` 修改该师的 `world.divisions.destinations[i]` 或 `world.divisions.assignments[i]`。
2. THE 战线系统 SHALL 仅对玩家国家的集团军施加该保护；AI 国家**自身的**集团军（见需求 15）SHALL 由 AI 通过同一战线 API 改写。
3. WHEN 一支玩家集团军的战线指令失活，THE 战线系统 SHALL 停止保护其原成员师，使下一次 AI tick 起允许 AI 指令再次作用于它们（仅在玩家切换扮演国时相关）。
4. THE 战线系统 SHALL 通过 `hoi4_ai::ground_orders::execute_ground_orders` 跳过下标登记在 `world.player_locked_divisions: HashSet<usize>` 中的师来实现，该集合由战线系统在每次玩家指令变更时更新。
5. WHEN AI 国家的师属于一支 AI 集团军（`PlayerArmy.owner != world.player`）且该集团军有活跃战线指令，THE 战线系统 SHALL 同样阻止 `execute_ground_orders` 中**绕过战线 API 的逐师调度路径**改写其目的地——AI 改师的唯一合法途径是调用战线 API（创建/调整集团军、重画战线、重画箭头）。

### 需求 9：渲染战线与箭头

**用户故事：** 作为玩家，我想在地图上看到我的战线与进攻箭头，这样我能一眼核对指令。

#### 验收标准

1. WHILE 一支集团军有活跃战线指令，THE 战线系统 SHALL 为战线路径上每对相邻省份提交一个 `ArrowInstance` 给 `MapArrowPass`，使用 `segment_type = 0`（线身）和与该集团军颜色匹配的虚线颜色。
2. WHILE 一支集团军有活跃进攻箭头，THE 战线系统 SHALL 为该箭头额外提交 `ArrowInstance`：线身段使用 `segment_type = 0`、最末段使用 `segment_type = 1`（箭头），并使用对比色。
3. THE 战线系统 SHALL 在战线路径所有省份的屏幕投影质心位置放置一个集团军标签（集团军名 + 当前成员数）。
4. WHILE 战线绘制模式或箭头绘制模式激活，THE 战线系统 SHALL 用同一 `MapArrowPass` 通道在光标下渲染一条预览线，alpha 0.5 半透明。
5. WHEN 摄像机移动，THE 战线系统 SHALL 重新提交 `ArrowInstance` 数据，使渲染线锚定在画出的省份上。
6. IF 玩家通过 UI 切换隐藏指令叠加层，THEN THE 战线系统 SHALL 不为指令提交任何 `ArrowInstance`。

### 需求 10：存档/读档持久化

**用户故事：** 作为玩家，我想让我的集团军与战线指令在存读档后还在，这样不用每次重画。

#### 验收标准

1. THE 战线系统 SHALL 通过现有 `hoi4_state::save` 模块把所有 `world.player_armies`（包括每支集团军的 id、名字、所有者、成员师下标、战线路径、进攻箭头）序列化进存档。
2. WHEN 加载当前版本产生的存档，THE 战线系统 SHALL 还原每支集团军的 id、成员、战线路径、进攻箭头，使下一个每日 tick 产生与存档前相同的分布方案（round-trip 性质）。
3. IF 加载的存档是本特性引入之前生成的，THEN THE 战线系统 SHALL 把 `world.player_armies` 初始化为空，并 SHALL 让 AI 地面指令保持不变。
4. THE 战线系统 SHALL 丢弃任何加载后越界 `DivisionStore` 的成员师下标，并对每个被丢弃下标发出"过期成员"警告。

### 需求 11：分布器的确定性与汇合性

**用户故事：** 作为开发者，我想让战线分布是游戏状态的纯函数，这样属性测试与回放调试可用。

#### 验收标准

1. THE 战线分布器 SHALL 是 (集团军成员师下标、战线路径、`world.divisions.locations`、`world.map.adjacencies`、`world.provinces.controllers`) 的纯函数。
2. WHEN 在相同输入与无中间世界变更下连续调用两次，THE 战线分布器 SHALL 产出字节相同的分布方案（幂等性）。
3. WHEN 集团军成员师下标列表的传入顺序被置换，THE 战线分布器 SHALL 仍产出满足需求 3 中 `floor(N/M)`/`ceil(N/M)` 每省计数不变量的分布方案（输入顺序的汇合性）。
4. THE 战线分布器 SHALL 不调用任何会改变 `world` 的代码路径（无随机数生成、无全局状态、无副作用日志通道）。

### 需求 12：战线存档格式的 round-trip

**用户故事：** 作为开发者，我想让战线指令的磁盘表示能干净地 round-trip，这样存档格式变更不会静默破坏指令。

#### 验收标准

1. 对所有合法的 `world.player_armies` 值，通过存档格式做"序列化 + 反序列化"SHALL 得到与原值相等的值（解析器/打印器对的 round-trip 性质）。
2. WHEN 存档文本表示被解析后再打印，THE 战线系统 SHALL 对规范输入产生与原打印形式按字节等价的文本表示。
3. IF 反序列化时遇到非法战线路径（不相邻省、重复、id 越界），THEN THE 战线系统 SHALL 丢弃该集团军的指令、保留集团军外壳与成员，并发出可读的解析错误。

### 需求 13：性能预算

**用户故事：** 作为玩家，我想让战线指令不带来任何可感知的卡顿，这样在长期战争中地图依然响应灵敏。

#### 验收标准

1. THE 战线系统 SHALL 在 `crates/hoi4-app/tests` 中记录的参考性能目标上，于 5 毫秒内完成 100 师 × 64 省战线的分布方案计算。
2. THE 战线系统 SHALL 通过复用 `find_friendly_retreat` 已有的 40 跳 BFS 深度上限，把单次战线吸附器调用对至多 1024 采样点的画线手势限制在 5 毫秒以内。
3. WHEN 玩家未在主动绘制，THE 战线系统 SHALL 仅在指令自上一帧以来发生变化时提交 `ArrowInstance` 数据，使空闲态 CPU 成本由"脏集团军数量"上界决定。

### 需求 14：对非法输入的反馈

**用户故事：** 作为玩家，我想在画的手势无法形成合法指令时得到清晰反馈，这样我能知道为什么指令没生成。

#### 验收标准

1. IF 玩家在 `selected_divisions` 为空时尝试创建集团军，THEN THE 战线系统 SHALL 发出"请先选择师团"信息，并 SHALL 不创建集团军。
2. IF 玩家画的战线不包含任何合法省，THEN THE 战线系统 SHALL 发出"无合法前线"信息，并 SHALL 保留任何已有指令。
3. IF 玩家画的进攻箭头其锚点不与战线路径相邻，THEN THE 战线系统 SHALL 发出"箭头必须从战线开始"信息，并 SHALL 丢弃该箭头。
4. IF 玩家画的进攻箭头要求两个相邻手势采样之间的桥接超过 6 跳 BFS 深度，THEN THE 战线系统 SHALL 发出"箭头路径不可达"信息，并 SHALL 丢弃该箭头。
5. IF 玩家在非战时下达战线指令，THEN THE 战线系统 SHALL 仍记录战线路径，但 SHALL 跳过分布方案计算，并 SHALL 在战争开始之前持续发出"非战时，战线仅作信息显示"信息。

### 需求 15：AI 使用同一套战线系统

**用户故事：** 作为开发者，我想让 AI 国家通过完全相同的战线 API 调度自己的师团，这样玩家与 AI 共享一套底层模型——一处优化、双方受益；玩家也能在地图上看到对手的战线意图。

#### 验收标准

1. THE 战线系统 SHALL 暴露与玩家相同的公共 API（创建/解散集团军、加入/移除师团、设置/重置战线路径、设置/取消进攻箭头）供 `hoi4-ai` 调用，同一份 `world.player_armies` 容器存放玩家与 AI 双方的集团军，每条由 `PlayerArmy.owner: CountryId` 区分所有者。容器名保留 `player_armies` 仅为兼容历史命名。
2. THE 战线系统 SHALL 把每个国家最多可同时存在的活跃战线指令数限制为 `MAX_FRONTLINES_PER_COUNTRY = 4`，超出时拒绝创建并返回"集团军数量已达上限"错误。
3. WHEN AI 协调器在每个 AI tick 上为某 AI 国家做地面决策，THE `hoi4_ai::ground_orders` SHALL 通过该战线 API 表达决策——而不是直接写 `world.divisions.destinations[i]`：
   - 该 AI 国家**没有**集团军时：将其全部师按所属敌国前线段（`compute_segment` 的输出）划分，对每段敌国（每对 attacker/defender）创建一支集团军，画出当前 segment 的 friendly_states 链作为战线路径。
   - 该 AI 国家**已有**集团军时：在每次 tick 重画战线路径以贴合当前 segment（不重建集团军、不打乱成员），调用相同的吸附器/分布器。
   - 该国当前 ground posture 为 `Attack` 时：在该集团军上加一条进攻箭头指向 `pick_attack_targets` 选出的最高优 enemy_state；`Defend / Hold / Retreat` 时取消任何现存箭头。
4. WHEN AI 国家把一支集团军交给战线系统后，`execute_ground_orders` SHALL 不再绕过战线 API 直接逐师设 destinations 给该集团军的成员（与需求 8.5 一致）。游离师（不在任何集团军里的）走旧的逐师调度路径，作为兜底。
5. THE 战线分布器与吸附器 SHALL 对玩家与 AI 调用产生**完全相同**的输出（属性 B 的输入空间扩展到 `owner != world.player` 的集团军）。
6. WHEN 某 AI 集团军的战线指令在每日 tick 中失活（路径塌陷或不再交战），THE 战线系统 SHALL 把该集团军标为"无活跃指令"并保留集团军外壳；AI 协调器 SHALL 在下一次 tick 决定是否重画战线、解散集团军、或转入 mop-up 路径。
7. WHEN 一国处于 mop-up 路径（`!front.has_front()` 但仍 `is_at_war`，见 `execute_mop_up_only`），THE AI SHALL 不创建集团军，而 SHALL 走旧的逐师追击逻辑；该路径不与战线系统冲突。
8. THE 战线系统 SHALL 让 AI 创建/调整集团军的总开销在每个 AI 国家每日 tick 上不超过 1 毫秒（含吸附器与分布器一次调用），以保留需求 13 的全国性能预算。
9. THE 渲染层 SHALL 仅在 `world.settings.show_all_units == true`（调试开关，见已存在的"显示所有部队"设置）时显示 AI 集团军的战线与箭头；常规模式下仅显示玩家自己的集团军，避免地图被 50 国前线塞满。
10. THE 存档系统 SHALL 同样持久化 AI 集团军（与玩家集团军走同一序列化路径，见需求 10）。
11. WHEN AI 集团军的某成员师被战斗摧毁或归零（`strength[i] < 0.05` 且 `organisation[i] < 0.05` 或下标在 `DivisionStore` 中失效），THE 战线系统 SHALL 在下一个每日 tick 把它从该 AI 集团军中移除；若集团军成员归零，THE AI 协调器 SHALL 解散该集团军。

## 正确性属性

下列基于属性的测试种子由上述验收标准推导。每条属性必须对该属性输入空间内的所有合法输入成立。

### 属性 A：分布方案均衡不变量（R3.2）

对所有 (N>0 师的集团军、M>0 个不同相邻省的战线路径)，战线分布器产出的分布方案满足：战线路径上每省分到 `floor(N/M)` 或 `ceil(N/M)` 个师，已分配的总师数等于 `min(N, 沿路径求和)`，且没有师出现两次。

### 属性 B：分布方案确定性（R11.2）

对所有 (世界快照、集团军输入)，`frontline_distributor(snapshot, army)` 在相同输入上调用两次返回相等的分布方案。

### 属性 C：输入顺序置换下的汇合性（R11.3）

对集团军成员下标列表的所有置换 `π`，得到的分布方案仍满足属性 A。每师的目标省可能因置换不同而不同，但每省的计数分布必须不变。

### 属性 D：战线路径合法性不变量（R2.3、R2.4、R7.1、R7.2）

战线吸附器输出后或任何 tick 后重算后，活跃战线路径满足：每省都是陆地、每对相邻省都在 `world.map.adjacencies` 中陆地相邻、每省的控制者等于玩家国家或共战国、无重复、长度在 `[1, 64]`。

### 属性 E：存档 round-trip（R10.2、R12.1）

对所有合法的 `world.player_armies` 值 `a`，`deserialize(serialize(a)) == a`。

### 属性 F：集团军成员互斥（R1.4）

对战线系统的所有操作，每个师下标至多出现在一个集团军的成员列表中。

### 属性 G：玩家手动指令的优先级（R8.1、R15.4）

对玩家国家拥有至少一支带活跃战线指令的集团军的所有 AI 地面指令 tick，玩家国家任何集团军成员下标 `i` 对应的 `world.divisions.destinations[i]` 不被 `execute_ground_orders` 修改。
对任何国家（含 AI）拥有的有活跃指令的集团军，其成员 `i` 的 `destinations[i]` 仅可由战线 API 改写，不可被 `execute_ground_orders` 的兜底逐师路径修改。

### 属性 H：战线塌陷的终止性（R7.3）

若战线路径上每个省在某次每日 tick 中都变得不合法，则下一个每日 tick 观察到该集团军的战线指令处于已失活状态。

### 属性 I：吸附器对已合法输入的幂等性（R2）

对任何已经满足属性 D 的战线路径 `P`，`frontline_snapper(map_points_of(P)) == P`（先打印再吸附保持路径不变）。

### 属性 J：AI 与玩家分布等价（R15.5）

把同一组 (集团军成员、战线路径、世界快照) 分别以 `owner = world.player` 与 `owner = some_ai_country` 调用战线分布器，输出的分布方案在"成员到目标省"的多重集合意义下相等（owner 字段不影响分布结果）。

### 属性 K：每国战线数量上限（R15.2）

对任何国家、任何战线 API 调用序列，在该序列结束后该国持有的活跃战线指令数 ≤ `MAX_FRONTLINES_PER_COUNTRY`。超过上限的创建调用必须返回错误且不改变 `world.player_armies`。
