# 项目总重构路线图

生成日期：2026-05-25

范围：本路线图覆盖全项目重构。`SELF_GUI_REFACTOR_ROADMAP.md` 只是 UI 路线中的一个分支，用于确认“自研 egui”为当前 GUI 方案；本文件继续覆盖 runtime、main.rs、调度、资产、数据、逻辑、AI、脚本、测试和工程健康。

## 先看这里：执行入口

这份路线图不是让你同时开十条线。它的正确读法是：

```text
执行清单：REFACTOR_EXECUTION_ORDER.md
  -> P0.1 -> P0.2 -> P0.3 -> ... -> P7.3

辅助入口：START_HERE_REFACTOR.md
  -> 解释为什么第一刀从 P0/P1 开始
```

`REFACTOR_EXECUTION_ORDER.md` 是唯一线性执行清单。`START_HERE_REFACTOR.md` 只是解释起点，不是任务总表。

### 当前唯一推荐起点

从 `REFACTOR_EXECUTION_ORDER.md` 的 `P0.1` 开始：

1. `crates/hoi4-state/src/world.rs`：修 `World::remove_divisions()` 的 division/corps remap。
2. `crates/hoi4-logic/src/economy/planned_tick.rs`：修计划经济年份硬编码。
3. `crates/hoi4-logic/src/economy/mod.rs`：修 daily/hourly economy tick 双执行风险。
4. 新建最小 `hoi4-runtime`，把 `SystemSchedule` 和 tick 管线迁进去。
5. 让 headless 和 GUI 都走同一个 runtime tick。

这 5 步是后面所有重构的前置条件。不是因为其它问题不重要，而是因为 runtime 和状态正确性不稳定时，先拆 UI、render、assets 会把问题扩散。

### 主线依赖关系

```text
P0 Bug 修复
  -> Runtime 统一
      -> main.rs 瘦身
          -> UI binding / 自研 egui 深化
          -> Render runtime 拆分
      -> Integration 测试重建
  -> Asset/Data 加载统一
      -> UI asset bridge
      -> Render parser 去重
  -> Script/Content 收口
      -> Event/Decision/Situation runtime 稳定
  -> AI/Military 长期战役验证
```

### 不要误读

- 不是只做 P0 bug。
- 不是 GUI 不做。
- 不是资产不做。
- 不是 script/AI 不做。
- 是先把阻塞其它重构的地基处理掉，再按依赖顺序推进。

## 0. 总体判断

项目目前不是“缺功能”，而是功能已经太多但边界不稳。主要风险来自：

- `hoi4-app/src/main.rs` 仍是 God Object。
- GUI、headless、integration 可能不走完全同一条 simulation tick 路径。
- `SystemSchedule` 存在，但旁路 daily / weekly 逻辑仍在 `main.rs` 中。
- app crate 承担 simulation runtime，导致 integration 依赖 app。
- UI 路线已选择自研 egui，但 UI 和 gameplay / asset 仍耦合。
- asset/data/map 加载没有统一走 `PathConfig` / `AssetDb`。
- `hoi4-render`、`hoi4-assets`、`hoi4-ui` 之间存在重复资产解析。
- `hoi4-script` 和 `hoi4-content` 职责重叠，且部分 effect no-op。
- 测试数量不少，但全 workspace 基线不健康，跨模块 tick 顺序覆盖不足。

## 1. 重构主线总览

推荐按以下主线并行规划，但实际执行时应小步提交：

| 主线 | 目标 | 优先级 |
|---|---|---|
| A. Correctness Bug 修复 | 先修已知真实 bug，避免带病重构 | P0 |
| B. Simulation Runtime | 建立唯一 tick pipeline，GUI/headless/integration 共用 | P0 |
| C. `main.rs` 瘦身 | 把 app shell、UI binding、render orchestration、content bootstrap 拆开 | P1 |
| D. 自研 egui GUI | 采用 DTO/Action + app binding，UI 不直接依赖 gameplay | P1 |
| E. Asset/Data 路径统一 | `PathConfig` / `AssetDb` 成为统一入口 | P1 |
| F. Render/Audio/Assets 边界 | 音频迁出 render，旧 parser 下线，pass 归位 | P2 |
| G. Logic/Economy 去重复 | 经济 market/planned 公共逻辑抽取，tick guard 统一 | P1/P2 |
| H. Script/Content 统一 | 明确 RON content 与 vanilla script runtime 的边界 | P2 |
| I. AI 和军事调度 | 保持 AI 与 military control 权限单一，补长期战役 invariant | P2 |
| J. 测试与 CI 基线 | 恢复全量测试可信度，fixture 化，区分 vanilla audit | P0/P1 |

## 2. Phase 0：先修真实 bug 和基线

目标：在大规模拆分前，先处理会污染模拟结果或阻断验证的明确问题。

### 2.1 Division 删除 remap

位置：

- `crates/hoi4-state/src/world.rs:544-625`

问题：

- `World::remove_divisions()` 使用 swap/remove 后，`command.div_to_corps` remap 可能留下错误 corps 映射。
- `corps.divisions` 和 `div_to_corps` 可能不一致。

建议：

- 重写删除流程为显式 old->new mapping。
- 所有索引容器统一应用 mapping。
- 增加 invariant 测试：删除非末尾 division 后，`div_to_corps` 与 `corps.divisions` 一致。

验收：

- 删除 division 后 command hierarchy 不出现悬挂引用。
- PlayerArmy、locked divisions、corps、division store 长度一致。

### 2.2 计划经济年份硬编码

位置：

- `crates/hoi4-logic/src/economy/planned_tick.rs:838-841`

问题：

- `step_rationing()` 内 `let year = 1936i32;`。

建议：

- 改用 `world.date.year`。
- 增加 1936 / 1938 / 1941 计划切换测试。

### 2.3 经济 daily/hourly tick 双执行风险

位置：

- `crates/hoi4-logic/src/economy/mod.rs:444-475`

问题：

- `tick_daily_v6()` 和 `tick_hourly_spread_v6()` 可以对同一天重复执行。

建议：

- 确定唯一官方入口。
- 或让两个入口共享 daily guard。
- 增加测试：同一天调用两种入口不会重复结算。

### 2.4 state/province owner/controller 同步

已有审计位置：

- `BUG_AUDIT_ROADMAP.md:49-57`
- `crates/hoi4-script/src/effects.rs:436-448`

问题：

- `transfer_state` 只改 state，可能不改 province。

建议：

- 所有 state transfer API 统一通过一个 helper。
- helper 同步 state owner/controller、province owner/controller、frontline/map dirty flags。

### 2.5 测试基线

已有记录：

- `docs/main_rs_split_audit.md:92-101`
- `BUG_AUDIT_ROADMAP.md:12-16`

目标：

- 重新明确当前 `cargo test --workspace` 状态。
- 把已知失败和新增失败分开。
- 恢复至少核心 crate 的测试可信度：`hoi4-state`、`hoi4-logic`、`hoi4-content`、`hoi4-integration`。

## 3. Phase 1：建立唯一 Simulation Runtime

目标：让 GUI、headless、integration 完全共享同一条 tick 管线。

### 3.1 新 crate：`hoi4-runtime` 或 `hoi4-sim`

建议新建：

```text
crates/hoi4-runtime/
  src/lib.rs
  src/runtime.rs
  src/schedule.rs
  src/companions.rs
  src/content_runtime.rs
  src/script_runtime.rs
  src/ai_runtime.rs
  src/feedback.rs
```

迁入内容：

- `crates/hoi4-app/src/systems.rs`
- `crates/hoi4-app/src/runtime/systems_runtime.rs`
- `crates/hoi4-app/src/runtime/situation_runtime.rs`
- `crates/hoi4-app/src/ai.rs` 中不依赖 app shell 的部分
- `crates/hoi4-app/src/script.rs` 中不依赖 app shell 的部分
- content daily tick / event / decision / focus / surrender 驱动

### 3.2 新核心类型

建议：

```rust
pub struct GameRuntime {
    pub world: World,
    pub sim: SimulationState,
    pub content: ContentRuntime,
    pub schedule: SystemSchedule,
    pub feedback: FeedbackBus,
}

pub struct SimulationState {
    pub economy: EconomyState,
    pub research: ResearchState,
    pub politics: PoliticsCache,
    pub script: ScriptState,
    pub ai: AiState,
    pub v6_db: V6Database,
}
```

### 3.3 Tick API

建议：

```rust
impl GameRuntime {
    pub fn tick_one_hour(&mut self) -> RuntimeEvents;
    pub fn tick_hours(&mut self, n: u32) -> RuntimeEvents;
    pub fn tick_until_day_change(&mut self) -> RuntimeEvents;
}
```

要求：

- `World::tick_hour()` 只由 runtime 调用。
- daily / weekly / monthly cadence 只在 runtime 内判定。
- `main.rs` 不直接调用 `daily_focus_tick` / `daily_event_tick` / `weekly_strategic_tick`。

### 3.4 Runtime events

runtime 不应直接操作 UI，而是返回事件：

```rust
pub enum RuntimeEvent {
    DayChanged(GameDate),
    MonthChanged(GameDate),
    WarStarted { attacker: CountryId, defender: CountryId },
    EventQueued { event_id: String, country: CountryId },
    MapOwnershipChanged,
    EndConditionReached(EndCondition),
    PauseRequested(PauseReason),
}
```

app 决定如何响应：

- 弹窗。
- 暂停。
- 刷新地图 LUT。
- 播音效。

### 3.5 验收

- headless 和 GUI 使用同一 `GameRuntime::tick_one_hour`。
- `hoi4-integration` 不再依赖 `hoi4-app` 才能 tick。
- `main.rs` 不再直接执行 content daily/weekly tick。
- 新增测试：GUI path 和 headless path 运行同样 30 天，关键 metrics 一致。

## 4. Phase 2：`main.rs` 瘦身

目标：`main.rs` 回到 app shell，而不是游戏引擎本体。

### 4.1 目标结构

建议：

```text
crates/hoi4-app/src/
  main.rs                  # 只保留入口和 ApplicationHandler glue
  app_shell.rs             # winit lifecycle
  app_state.rs             # App struct / high-level state
  update_loop.rs           # real time -> runtime ticks
  input.rs                 # keyboard/mouse -> app commands
  ui_binding/              # UI DTO/action binding
  render_runtime/          # RenderState orchestration
  content_bootstrap.rs     # 内容加载入口
  debug_commands.rs        # debug command parsing/execution
```

### 4.2 迁移顺序

推荐：

1. `content_bootstrap`：内容加载从 `App::new` 迁出。
2. `update_loop`：real-time accumulator 和 runtime tick 迁出。
3. `ui_binding`：按 `SELF_GUI_REFACTOR_ROADMAP.md` 执行。
4. `render_runtime`：`RenderState` 初始化和 frame orchestration 迁出。
5. `input`：键鼠命令映射迁出。
6. `debug_commands`：调试命令和日志迁出。

### 4.3 验收

- `main.rs` 不再超过约 1000-1500 行。
- `App::update` 不直接包含 gameplay daily tick。
- `App::render` 不直接构造大型 panel DTO。
- `App::new` 不直接 include 具体国家内容文件。

## 5. Phase 3：自研 egui GUI 重构

目标：采用方案 A，自研 egui GUI，但不是继续堆手写耦合 UI。

详细路线见：

- `SELF_GUI_REFACTOR_ROADMAP.md`

本总路线只记录关键约束：

- `hoi4-ui` 只负责绘制和局部交互。
- `hoi4-ui` 接收 DTO，返回 Action。
- `hoi4-ui` 不直接依赖 `hoi4-content`。
- `hoi4-ui` 不直接依赖 `hoi4-state`，至少不新增这种依赖。
- UI 不直接解析 `.gfx`。
- `.gfx` sprite metadata 迁到 `hoi4-assets`。
- app-side `ui_binding` 负责 World/content/runtime -> DTO，以及 Action -> command。

## 6. Phase 4：Asset / Data / Map 加载统一

目标：所有 HOI4 资产和数据路径都走统一入口，避免 mod chain、replace_path 和白名单策略失效。

### 6.1 `hoi4-map` 改造

当前位置：

- `crates/hoi4-map/src/lib.rs:55-146`

问题：

- `GameMap::load(game_path: &Path)` 直接 `game_path.join(...)`。

建议：

```rust
impl GameMap {
    pub fn load_from_paths(paths: &PathConfig) -> Result<Self, MapError>;
    pub fn load(game_path: &Path) -> Result<Self, MapError>; // 过渡 wrapper
}
```

规则：

- 单文件用 `paths.find("map/provinces.bmp")`。
- 目录/多文件用 `paths.find_all(...)`。
- 必需地图文件失败应 fatal。
- 可选贴图失败进入 load report。

### 6.2 `hoi4-data` 改造

当前位置：

- `crates/hoi4-data/src/loader.rs:115-147`

问题：

- 大量 `game_path.join(...)`。
- 大量 `unwrap_or_default()` 静默吞错。

建议：

```rust
pub struct GameDataLoadReport {
    pub loaded_counts: LoadedCounts,
    pub warnings: Vec<LoadWarning>,
    pub optional_missing: Vec<String>,
}

pub fn load_from_paths(paths: &PathConfig) -> Result<(GameData, GameDataLoadReport), LoadError>;
```

规则：

- 必需核心数据失败直接 `Err`。
- 可选数据 warning。
- app 启动 banner 打印 report。

### 6.3 `hoi4-assets` 扩展

应增加：

- `.gfx` sprite metadata index。
- icon fallback resolver。
- UI 9-slice source resolver。
- unsupported DDS format 明确报错。
- asset load report。

### 6.4 验收

- `hoi4-data` 和 `hoi4-map` 主加载路径不再直接依赖裸 `game_path.join`。
- mod chain / replace_path 至少有 synthetic 测试。
- loader 不再大规模 `unwrap_or_default()` 静默降级。

## 7. Phase 5：Render / Audio / Assets 边界重整

### 7.1 音频迁出 `hoi4-render`

现状：

- `crates/hoi4-render/src/audio.rs`
- `hoi4-render` 依赖 `rodio`

目标：

- 新建 `hoi4-audio`，或暂迁到 `hoi4-app::audio_runtime`。
- `hoi4-render` 移除 `rodio`。
- 音乐和 UI 音效加载走 `PathConfig` / `AssetDb`。

### 7.2 删除 render 旧 parser

现状：

- `crates/hoi4-render/src/dds.rs`
- `crates/hoi4-render/src/mesh.rs`
- `crates/hoi4-assets/src/dds.rs`
- `crates/hoi4-assets/src/pdx_mesh.rs`

目标：

- render 只使用 `hoi4-assets` 的 DDS/mesh parser。
- 如需要 public API，render re-export assets 类型。

### 7.3 render pass 归位

现状：

- 大量 pass 在 `crates/hoi4-app/src/passes`。
- `hoi4-render` 名义上是渲染库，但核心 pass 不都在里面。

目标：

- 稳定 pass 逐步迁到 `hoi4-render`。
- app 只管理 window/surface 生命周期和 frame orchestration。
- render 从 `RenderWorldSnapshot` 读取，不直接遍历完整 `World`。

## 8. Phase 6：Economy / Logic 去重复与调度收口

### 8.1 抽公共经济逻辑

重复区域：

- `crates/hoi4-logic/src/economy/market_tick.rs`
- `crates/hoi4-logic/src/economy/planned_tick.rs`

建议抽出：

- POP education。
- qualification ratio。
- input fulfillment。
- PM unlock。
- employment hire/fire。
- law category mapping。
- good demand/supply helpers。

目标文件：

```text
crates/hoi4-logic/src/economy/pop_runtime.rs
crates/hoi4-logic/src/economy/building_tick_common.rs
crates/hoi4-logic/src/economy/market_common.rs
```

### 8.2 统一 tick cadence

目标：

- economy tick 只能通过 runtime schedule 进入。
- military daily/hourly 顺序在 runtime 中声明。
- AI strategic/tactical cadence 在 runtime 中声明。
- script/content cadence 在 runtime 中声明。

### 8.3 Balance config

当前 magic numbers 分散在：

- `hoi4-ai/src/constants.rs`
- `hoi4-ai/src/ground_orders.rs`
- `hoi4-logic/src/military/*`
- `hoi4-logic/src/economy/*`

建议：

```rust
pub struct SimulationBalance {
    pub economy: EconomyBalance,
    pub military: MilitaryBalance,
    pub ai: AiBalance,
    pub cadence: CadenceConfig,
}
```

短期可以先集中常量，不急着做外部配置文件。

## 9. Phase 7：Script / Content 关系重整

当前问题：

- `hoi4-content` 是 RON content runtime。
- `hoi4-script` 是 trigger/effect/event/decision primitives。
- `SystemSchedule::Script` 跑一套，`main.rs` 又跑 content event/decision/focus。
- 多个 script effect 注册但 no-op。

### 9.1 先定权威关系

推荐短期方案：

- `hoi4-content` 是当前自研内容的 authoritative runtime。
- `hoi4-script` 是 vanilla compatibility / trigger-effect engine，不直接冒充完整 runtime。
- runtime 中建立 `ContentRuntime`，统一驱动 focus/event/decision/situation/surrender。

### 9.2 Effect report

`hoi4-script` 应返回报告：

```rust
pub struct EffectReport {
    pub executed: usize,
    pub unknown: Vec<String>,
    pub noop: Vec<String>,
    pub commands: Vec<ScriptCommand>,
}
```

要求：

- unknown effect 不应默认完全静默。
- no-op effect 在 dev/report 模式可见。
- 需要 `EconomyState` 的 effect 输出 command，由 runtime 应用。

### 9.3 Event scheduler 完整化

目标：

- trigger evaluation。
- immediate execution。
- option selection。
- `fire_only_once`。
- AI chance。
- pending event queue 和 UI modal 分离。

## 10. Phase 8：AI / Military 长期稳定性

已有 AI bug 文档显示大量战线问题已修，但该领域仍应防回归。

### 10.1 权限边界

目标：

- AI 决策层只输出 orders。
- military/frontline 执行层拥有 destination 最终写入权，或二者通过明确 command API 协作。
- 不再出现多个系统每天竞争覆盖 `division.destinations`。

### 10.2 长期战役测试

建议 fixtures：

- ITA vs ETH 180 天。
- Spanish Civil War 180 天。
- Sino-Japanese War 365 天。
- GER vs POL 60 天。

验收指标：

- 战线有推进或稳定防御，不全军静止。
- division count、corps mapping、province controller invariant 成立。
- AI orders 数量和执行结果可观测。

## 11. Phase 9：测试和 CI 重建

### 11.1 测试分层

建议分成：

- unit tests：纯函数和局部 store。
- fixture integration：不依赖真实 HOI4 安装。
- vanilla audit：需要本地 HOI4，默认 skip。
- long-run simulation：可 nightly / 手动跑。

### 11.2 真实 HOI4 安装测试

当前多个测试依赖 `PathConfig::resolve`。建议：

- 默认不跑真实安装测试。
- 使用环境变量显式开启，例如 `IRONHEART_RUN_VANILLA_AUDIT=1`。
- CI 默认跑 synthetic fixture。

### 11.3 Invariant tests

必须补：

- SoA store len consistency。
- division location/province index consistency。
- command corps/division mapping consistency。
- state/province owner/controller consistency。
- daily economy tick once-only。
- runtime GUI/headless parity。
- script unknown/no-op effect report。

### 11.4 Warning budget

已有 `BUG_AUDIT_ROADMAP.md` 记录 warning 多。建议：

- 先不要求全 workspace `-D warnings`。
- 先建立 warning count baseline。
- 每个 Phase 不增加新 warning。
- P0/P1 后再清理明显 dead code 和错误 lint 名。

## 12. 推荐实际执行顺序

### 第一批：稳定地基

1. 修 division remap。
2. 修计划经济年份硬编码。
3. 修/保护 economy daily/hourly 双 tick。
4. 记录并恢复核心测试基线。
5. 明确自研 egui 文档路线。

### 第二批：runtime 收口

1. 新建 `hoi4-runtime`。
2. 迁 `SystemSchedule`。
3. 迁 `systems_runtime`。
4. 把 content daily/weekly tick 纳入 runtime。
5. 让 headless 和 GUI 共用 runtime API。
6. 让 `hoi4-integration` 改依赖 runtime。

### 第三批：app 瘦身

1. 抽 `content_bootstrap`。
2. 抽 `update_loop`。
3. 按 GUI 路线抽 `ui_binding`。
4. 抽 `render_runtime`。
5. 抽 `input`。

### 第四批：资产和数据

1. `GameMap::load_from_paths`。
2. `GameData::load_from_paths`。
3. `GameDataLoadReport`。
4. `.gfx` parser 迁到 `hoi4-assets`。
5. UI icon/nine-slice 改走 asset bridge。

### 第五批：系统清理

1. 音频迁出 render。
2. 删除 render 旧 DDS/mesh parser。
3. render pass 逐步迁入 `hoi4-render`。
4. economy 公共逻辑抽取。
5. script/content 关系收口。

## 13. 不建议做的事

暂时不要：

- 不要一口气大规模格式化或移动所有文件。
- 不要在 runtime 没统一前继续加新的 daily/weekly 系统到 `main.rs`。
- 不要继续让 UI 面板直接读 `World`。
- 不要继续在 UI 中新增资产 parser。
- 不要把真实 HOI4 安装测试作为默认 CI 前提。
- 不要先做视觉大改，而不处理 tick pipeline。

## 14. 完成定义

全项目重构达到阶段性完成时，应满足：

- GUI、headless、integration 共享 `GameRuntime`。
- `hoi4-integration` 不再为了 tick 依赖 `hoi4-app`。
- `main.rs` 只保留 app shell，不再承担 gameplay runtime。
- 自研 egui UI 通过 DTO/Action 和 app binding 工作。
- `hoi4-ui` 不直接解析 `.gfx`，不新增 gameplay 依赖。
- `hoi4-data` / `hoi4-map` 主加载路径走 `PathConfig`。
- loader 有 report，不再静默空数据继续跑。
- `hoi4-render` 不包含 audio，不保留重复 DDS/mesh parser。
- economy daily/hourly tick 互斥且有测试。
- command hierarchy、state/province、division index 等核心 invariant 有测试。
- 全 workspace 测试失败项被明确分类，不再混成未知红灯。

## 15. 核心原则

这次重构的核心不是“把文件拆小”，而是让权威路径唯一：

```text
唯一 runtime tick；
唯一 asset/data 加载入口；
唯一 UI binding 边界；
唯一 command/state mutation 入口；
唯一测试基线解释。
```

只要权威路径唯一，项目还能继续长大；如果继续保留旁路逻辑和重复 parser，后面每加一个功能都会放大现有混乱。
