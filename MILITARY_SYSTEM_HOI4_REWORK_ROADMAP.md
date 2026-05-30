# 军事系统 HOI4 化重做路线图

## 目标

把当前“选中师团后临时底栏 + 省份质心战线 + 无将领 + 右键移动无箭头 + 兵牌每日跳点”的军事系统，重构为更接近《钢铁雄心 IV》原版体验的系统：底部常驻集团军栏、选中师团点 `+` 创建集团军、任免将领、画前线、画进攻计划、执行计划、普通移动箭头反馈，以及兵牌线性移动表现。

本路线图只定义方向和阶段，不要求一次性完成。重构应优先保证现有战斗、移动、存档和 AI 调度不被破坏。

## 当前状态

### 已有能力

- 集团军/战线数据：`crates/hoi4-state/src/frontline.rs`
- 战线逻辑 API：`crates/hoi4-logic/src/military/frontline.rs`
- 军事 UI：`crates/hoi4-ui/src/military.rs`
- 输入和命令分发：`crates/hoi4-app/src/main.rs`
- 每日战线 tick：`crates/hoi4-app/src/systems.rs`
- 已有 `PlayerArmy`、`FrontlineOrder`、`OffensiveArrow`
- 已有创建/解散集团军、加入/移出师、画前线、画箭头、执行/停止计划
- 已有 `MapArrowPass`，当前用于战线/进攻箭头渲染
- 已有全局战争接触线生成：`crates/hoi4-render/src/frontlines.rs`

### 主要问题

- 底部军队栏不是 HOI4 式常驻栏，只是条件显示的上下文栏。
- 创建集团军按钮不是常驻 `+` 槽，且创建后没有完整的集团军卡片交互。
- 没有将领系统，也没有将领任免 UI、属性、肖像、指挥上限或加成。
- 普通右键移动只写入 `world.divisions.destinations`，没有移动箭头。
- 兵牌位置直接来自 `world.divisions.locations`，每日 tick 改省份后视觉上像闪现。
- 玩家画出的战线是省份质心折线，不沿真实敌我边界，非常不像 HOI4 原版。
- 玩家战线、进攻箭头、预览线都复用同一个 `MapArrowPass`，语义和视觉层级混乱。
- 当前玩家战线/箭头只按省份质心采样高度，山地和海岸附近可能悬空或穿地。
- 当前战线显示开关对玩家战线和 AI 战线的处理不完全一致。

## 不变量和边界

- 不要合并 `hoi4_state::command::Army` 和 `hoi4_state::frontline::PlayerArmy`。前者是 OOB 自动建制，后者是玩家/AI 画线命令容器。
- `player_armies` 容器名先保留，AI 也可复用，避免扩大存档迁移面。
- `FrontlineOrder.path: Vec<ProvinceId>` 继续作为逻辑和存档结构，不要为了视觉重做破坏它。
- 视觉层可以从 `path` 派生更真实的边界线，但逻辑分布器仍应基于 `path` 工作。
- 普通移动、集团军战线和进攻计划应共享底层移动系统，不引入第二套真实位移管线。
- 所有 UI 改造都应保留键鼠地图操作，不让面板挡住常用右键移动和拖拽画线。

## Phase 1：底部 HOI4 式集团军栏 MVP

目标：把当前临时底栏改成真正的军事指挥主入口。

任务：

- 重写 `hoi4_ui::military::MilitaryPanel::show_bottom_bar`。
- 底部栏在游戏内军事上下文中常驻显示，不再只在 `has_content` 时出现。
- 左侧显示选中师团数量。
- 常驻一个 `+` 创建集团军按钮。
- 未选中师团时 `+` 置灰并提示“请先选择师团”。
- 选中师团后点击 `+` 调用现有 `MilitaryCommand::CreateArmy`。
- 创建成功后自动选中新建集团军。
- 中间显示集团军卡片列表：名称、师数量、是否有前线、是否有进攻箭头、是否执行中。
- 右侧显示选中集团军命令：画前线、画进攻箭头、执行/停止计划、清除前线、清除箭头、加入/移出选中师、解散。
- 左侧军事面板保留训练、模板、总览，弱化集团军操作。

验收标准：

- 选中任意己方师团后，底栏 `+` 可直接创建集团军。
- 创建集团军后底栏立即选中新集团军。
- 不打开左侧军事面板，也可以完成创建集团军、画前线、画进攻计划、执行计划。

主要文件：

- `crates/hoi4-ui/src/military.rs`
- `crates/hoi4-app/src/main.rs`

## Phase 2：普通右键移动箭头

目标：右键省份后显示普通移动箭头，让玩家有即时命令反馈。

任务：

- 在 `collect_frontline_arrows()` 中纳入普通移动命令箭头，或重命名为更通用的 `collect_order_arrows()`。
- 遍历玩家选中师团或玩家所有有 `destinations[i]` 的师团。
- 从当前省份到目标省份生成视觉路径。
- MVP 可先画 `当前省质心 -> 目标省质心` 的箭头。
- 进阶版应使用 `next_step` 或 BFS 路径生成多段路线。
- 普通移动箭头颜色应区别于进攻计划箭头，例如白/浅蓝/浅黄低透明。
- 取消移动命令时箭头消失。
- 如果多个选中师目标相同，可以合并箭头，避免刷屏。

验收标准：

- 选中师团右键目标省后，地图上出现移动箭头。
- 到达目标或目的地清空后，移动箭头消失。
- 普通移动箭头和集团军进攻箭头视觉可区分。

主要文件：

- `crates/hoi4-app/src/main.rs`
- `crates/hoi4-app/src/passes/maparrow.rs`
- `crates/hoi4-logic/src/military/movement.rs`

## Phase 3：兵牌线性移动表现

目标：兵牌不要每日从一个省中心闪现到另一个省中心，而是在两个省之间线性移动。

任务：

- 不改变真实模拟的每日省份移动语义，先只做视觉插值。
- 新增渲染层移动快照，例如 `VisualDivisionMotion` 或 `App.division_motion`。
- 在检测到 `world.divisions.locations[i]` 变化时记录：旧省份、新省份、开始时间、持续时间。
- `generate_hoi3_counters_cr3` 当前只能按 `locations` 投影，需新增可选的“视觉位置覆盖”。
- 简单方案：在 app 层构造临时 counter 位置输入，而不是让 render crate 直接读 `locations`。
- 更干净方案：扩展 counter 生成函数，传入 `visual_centroids_override: Option<&HashMap<usize, (f32, f32)>>`。
- 插值时间应与游戏速度相关，暂停时停住，快速速度下仍可看清移动。
- 如果距离过大或路径不连续，可使用 snap 阈值，避免跨大陆长线飞牌。

验收标准：

- 师团移动到相邻省时，兵牌在屏幕上连续滑动。
- 游戏暂停时移动动画不继续跑。
- 快速推进多天时不会出现长距离乱飞，必要时可直接吸附。

主要文件：

- `crates/hoi4-app/src/main.rs`
- `crates/hoi4-render/src/counter_v3.rs`
- `crates/hoi4-state/src/store.rs`
- `crates/hoi4-logic/src/military/movement.rs`

## Phase 4：玩家战线视觉边界化

目标：玩家画出的前线不再是省份质心折线，而是沿真实敌我接触边界显示。

任务：

- 保留 `FrontlineOrder.path` 作为逻辑路径。
- 新增视觉转换函数：从某支集团军的 `order.path` 提取真实接触边。
- 复用 `crates/hoi4-render/src/frontlines.rs` 的 province bitmap 扫描思路，但限制到指定 `order.path`。
- 对每个 path 省份，扫描它与敌方控制陆地省份的 pixel 接触边。
- 将短边聚合成连续视觉段。
- 对边界线做简化、平滑和高度采样。
- 玩家战线应优先贴合敌我边界，不穿过省份内部。
- 如果某段 path 暂时没有敌接触边，使用低透明 fallback 质心线或隐藏该段。

验收标准：

- 玩家战线沿敌我真实接触边显示。
- 大省份、弯曲边界、山地和河流附近不再明显穿过省份中心。
- 视觉变化不影响师团分布、战斗、存档。

主要文件：

- `crates/hoi4-render/src/frontlines.rs`
- `crates/hoi4-app/src/main.rs`
- `crates/hoi4-app/src/passes/maparrow.rs`
- `crates/hoi4-state/src/frontline.rs`

## Phase 5：战线和箭头贴地形高度细分

目标：减少线条悬空、穿地和跨山地直插的问题。

任务：

- 给视觉线段增加按世界距离细分的采样函数。
- 每个采样点查询 heightmap，而不是只采样省份质心。
- 对玩家战线、普通移动箭头、进攻箭头、预览线统一使用。
- 控制最大实例数量，避免长战线生成过多 quad。
- 对水域、海岸、陡坡增加高度偏移保护。

验收标准：

- 山地、海岸和河谷附近线条更贴地。
- 长箭头不会只靠首尾高度插值。
- 性能无明显回退。

主要文件：

- `crates/hoi4-app/src/main.rs`
- `crates/hoi4-app/src/passes/maparrow.rs`

## Phase 6：进攻计划箭头独立升级

目标：进攻计划箭头从“普通分段 quad”升级成更像 HOI4 battleplan 的宽箭头。

任务：

- 将进攻计划箭头和普通移动箭头从视觉参数上彻底区分。
- 使用 spine 生成 body strip。
- 单独生成 arrow head mesh，而不是只靠最后几个 `segment_type`。
- 短箭头也要有稳定方向和合理头部。
- 转弯处平滑但不穿越无关省份。
- 执行中计划添加轻微流动/脉冲效果。
- 预览、已下达、执行中三种状态使用不同透明度和色彩。

验收标准：

- 进攻计划箭头明显区别于普通移动箭头。
- 箭头头部稳定，不会在短线或折线处乱摆。
- 执行计划时有清晰动态反馈。

主要文件：

- `crates/hoi4-app/src/passes/maparrow.rs`
- `crates/hoi4-app/src/main.rs`

## Phase 7：将领系统 MVP

目标：引入可任免将领，并在集团军卡片上显示。

任务：

- 在 `hoi4-state` 新增将领数据结构：`GeneralId`、`General`。
- 给 `World` 增加 `generals` 和 `next_general_id`。
- 给 `PlayerArmy` 增加 `commander: Option<GeneralId>`。
- 初始化每个主要国家的基础将领池，可先用占位姓名和属性。
- 军事底栏集团军卡片显示将领头像槽。
- 点击头像槽打开将领任免窗口。
- 支持任命、撤换、解除任命。
- 防止同一将领同时指挥多个集团军，除非后续设计允许。

验收标准：

- 每支集团军可以任命一个将领。
- 将领任免后，底栏卡片立即更新。
- 存档/读档后集团军将领关系保持。

主要文件：

- `crates/hoi4-state/src/frontline.rs`
- `crates/hoi4-state/src/world.rs`
- `crates/hoi4-ui/src/military.rs`
- `crates/hoi4-app/src/main.rs`
- `crates/hoi4-state/src/save/`

## Phase 8：将领属性接入战斗和计划

目标：让将领不只是 UI 装饰，而是影响战斗和计划执行。

任务：

- 将领基础属性：技能、进攻、防御、计划、后勤。
- 指挥上限：超过上限的师团降低加成。
- 进攻属性影响执行进攻计划时的攻击修正。
- 防御属性影响战线防御修正。
- 计划属性影响计划准备度或执行效率。
- 后勤属性影响补给消耗或组织恢复。
- 增加战斗日志或 tooltip，说明将领加成来源。

验收标准：

- 任命不同将领会改变战斗或计划相关数值。
- 超过指挥上限时有明确提示和惩罚。
- 加成可通过 UI 或日志验证。

主要文件：

- `crates/hoi4-logic/src/military/arbiter.rs`
- `crates/hoi4-logic/src/military/frontline.rs`
- `crates/hoi4-logic/src/military/organisation.rs`
- `crates/hoi4-ui/src/military.rs`

## Phase 9：画线交互重做

目标：画线跟手、预览可信、合法性反馈清晰。

任务：

- `FrontlinePainterState` 同时保存鼠标世界坐标轨迹和省份采样结果。
- 拖动时用世界坐标轨迹显示连续预览，而不是只显示省份质心。
- 鼠标附近查找最近合法前线边界。
- 合法段高亮，不合法段灰/红提示。
- 松手后将世界轨迹投影为合法 `ProvinceId` path，再调用现有 `set_frontline_path`。
- 快速拖动时补采样，避免漏线。
- 大省内移动也应有连续反馈。

验收标准：

- 玩家拖线时预览线跟手。
- 最终吸附结果与预览基本一致。
- 不合法区域有明确反馈。

主要文件：

- `crates/hoi4-app/src/main.rs`
- `crates/hoi4-logic/src/military/frontline.rs`

## Phase 10：存档、测试和兼容

目标：重构后稳定可验证。

任务：

- 扩展文本/二进制存档，保存将领、集团军 commander、作战计划状态。
- 对旧存档默认 `commander = None`、`generals = []` 或生成默认将领池。
- 增加普通移动箭头渲染测试。
- 增加玩家战线边界化测试：给定 toy map，输出不穿越省份中心。
- 增加将领任免测试：同一将领不能重复任命。
- 增加兵牌移动插值测试：位置变化时产生连续视觉坐标。
- 保留现有 frontline API 和分布器属性测试。

验收标准：

- `cargo test` 通过。
- 旧存档能加载。
- 创建集团军、任命将领、画线、执行计划、保存、读档后状态一致。

主要文件：

- `crates/hoi4-state/tests/save_roundtrip.rs`
- `crates/hoi4-logic/tests/frontline_api.rs`
- `crates/hoi4-logic/tests/frontline_path_invariant.rs`
- `crates/hoi4-app/tests/`

## 推荐执行顺序

1. Phase 1：底部 HOI4 式集团军栏 MVP。
2. Phase 2：普通右键移动箭头。
3. Phase 3：兵牌线性移动表现。
4. Phase 4：玩家战线视觉边界化。
5. Phase 5：战线和箭头贴地形高度细分。
6. Phase 6：进攻计划箭头独立升级。
7. Phase 7：将领系统 MVP。
8. Phase 8：将领属性接入战斗和计划。
9. Phase 9：画线交互重做。
10. Phase 10：存档、测试和兼容。

## 首批最小可交付范围

建议第一批不要同时引入将领和战斗加成，先解决最刺眼的交互和视觉问题：

- 底部常驻集团军栏和 `+` 创建集团军。
- 创建后自动选中新集团军。
- 普通右键移动箭头。
- 兵牌移动视觉插值。
- 玩家战线不再画省份质心折线，改为真实接触边界的基础版。

完成这批后，玩家会明显感觉军事系统从“调试面板式功能”变成“HOI4 式命令系统”的雏形。

## 后续设计问题

- 将领池是否从 vanilla 角色数据导入，还是先用本项目自定义占位数据？
- 集团军上限是否沿用当前 `MAX_FRONTLINES_PER_COUNTRY = 4`，还是改成更接近 HOI4 的多集团军？
- 普通右键移动箭头是否只显示选中部队，还是显示所有玩家移动中的部队？
- 兵牌线性移动是纯视觉动画，还是要把移动系统从每日跳点改为小时级进度？
- 玩家战线边界化后，多段不连续接触边如何排序和显示？
- AI 集团军战线在普通模式下是否隐藏，只在 debug `show_all_units` 下显示？
