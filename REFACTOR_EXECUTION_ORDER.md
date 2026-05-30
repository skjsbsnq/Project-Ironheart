# 重构任务执行顺序

生成日期：2026-05-25

这份文档是唯一的线性执行清单。按编号从上往下做，不要跳着开工。

格式说明：

- `P0.x`：必须先做，修正确性和验证地基。
- `P1.x`：统一 simulation runtime 和 tick 路径。
- `P2.x`：瘦身 `main.rs`，建立清晰 app 结构。
- `P3.x`：自研 egui GUI binding 和 UI 解耦。
- `P4.x`：资产、数据、地图加载统一。
- `P5.x`：render/audio/assets 边界清理。
- `P6.x`：logic/economy/script/AI 深层整理。
- `P7.x`：测试、CI、长期维护。

## P0：正确性和测试地基

### P0.1 修 `World::remove_divisions()` remap

文件：

- `crates/hoi4-state/src/world.rs`

前置：无。

任务：

- 检查 `World::remove_divisions()`。
- 修正 `scrub_division_index(...)` / `remap_division_index(...)` 中 division index 搬移后的 command hierarchy remap。
- 确保 `command.div_to_corps` 和 `corps.divisions` 一致。

验收：

- 删除非末尾 division 后，搬到旧位置的 division 仍指向正确 corps。
- `command.div_to_corps.len() == world.divisions.len()`。
- `corps.divisions` 不含被删除旧 index。

验证：

```powershell
cargo test -p hoi4-state
```

### P0.2 修计划经济年份硬编码

文件：

- `crates/hoi4-logic/src/economy/planned_tick.rs`

前置：P0.1。

任务：

- 搜索 `let year = 1936i32;`。
- 改为使用当前 `world.date.year`。
- 增加或调整测试，证明 1936 之后年份使用当前年份。

验收：

- 计划经济配给/计划逻辑不再固定按 1936 年计算。

验证：

```powershell
cargo test -p hoi4-logic planned
```

如果过滤不合适：

```powershell
cargo test -p hoi4-logic
```

### P0.3 防止经济 daily/hourly 双 tick

文件：

- `crates/hoi4-logic/src/economy/mod.rs`

前置：P0.2。

任务：

- 检查 `tick_daily_v6(...)` 和 `tick_hourly_spread_v6(...)`。
- 明确一个官方入口，或让两者共享同一 daily guard。
- 防止同一国家同一天被经济结算两次。

验收：

- 同一天调用 daily 和 hourly spread 不会重复结算。
- 相关库存、财政、生产变化不会翻倍。

验证：

```powershell
cargo test -p hoi4-logic economy
```

### P0.4 修 state/province owner/controller 同步入口

文件：

- `crates/hoi4-script/src/effects.rs`
- 如需要，可新增 state transfer helper 到 `hoi4-state` 或 `hoi4-logic`

前置：P0.3。

任务：

- 检查 `transfer_state`。
- 确保转移 state 时同步所有下属 province owner/controller。
- 如果项目中有多个 state transfer 入口，统一到一个 helper。

验收：

- state owner/controller 与所有下属 province owner/controller 一致。
- 政治地图、省份 tooltip、前线逻辑不会读到冲突归属。

验证：

```powershell
cargo test -p hoi4-script
cargo test -p hoi4-state
```

### P0.5 记录当前测试基线

文件：

- `docs/test_baseline.md` 或更新已有审计文档

前置：P0.4。

任务：

- 跑核心测试。
- 记录哪些失败是旧失败，哪些是新增失败。
- 不要求一次性修完全部旧失败，但必须让后续重构能判断是否引入新问题。

建议验证：

```powershell
cargo test -p hoi4-state
cargo test -p hoi4-logic
cargo test -p hoi4-content
cargo test -p hoi4-integration
```

验收：

- 有明确测试基线文档。
- 后续任务可以区分旧失败和新增失败。

## P1：统一 Simulation Runtime

### P1.1 新建最小 `hoi4-runtime` crate

文件：

- `crates/hoi4-runtime/Cargo.toml`
- `crates/hoi4-runtime/src/lib.rs`
- `crates/hoi4-runtime/src/runtime.rs`
- `crates/hoi4-runtime/src/schedule.rs`

前置：P0.5。

任务：

- 新建 crate。
- 先只迁和 runtime 直接相关的类型。
- 不迁 GUI、render、input。

候选迁移：

- `crates/hoi4-app/src/systems.rs` 中的 `SystemSchedule`。
- `SimContext` 或其替代结构。

验收：

- `hoi4-runtime` 能独立 `cargo check`。
- `hoi4-app` 仍能编译。

验证：

```powershell
cargo check -p hoi4-runtime
cargo check -p hoi4-app
```

### P1.2 建立 `GameRuntime` 壳

文件：

- `crates/hoi4-runtime/src/runtime.rs`

前置：P1.1。

任务：

- 定义 `GameRuntime`。
- 聚合 `World` 和 simulation companion state。
- 暂时允许字段从 app 传入，先不追求完美封装。

目标结构：

```rust
pub struct GameRuntime {
    pub world: World,
    pub sim: SimulationState,
    pub schedule: SystemSchedule,
}
```

验收：

- runtime 可以持有完整模拟所需状态。
- 不需要 app 才能构造 tick 上下文。

验证：

```powershell
cargo check -p hoi4-runtime
```

### P1.3 迁移 `tick_one_hour` 入口

文件：

- `crates/hoi4-runtime/src/runtime.rs`
- 原 `crates/hoi4-app/src/runtime/systems_runtime.rs`

前置：P1.2。

任务：

- 把 `tick_one_hour` 逻辑迁入 `GameRuntime::tick_one_hour()`。
- 统一调用 `schedule.tick_hour(...)`。
- 返回 runtime events 或最小 report。

验收：

- `World::tick_hour()` 由 runtime 驱动。
- app 不再需要自己拼 `SimContext` 参数才能 tick。

验证：

```powershell
cargo check -p hoi4-runtime
cargo check -p hoi4-app
```

### P1.4 把 content daily tick 纳入 runtime

文件：

- `crates/hoi4-runtime/src/content_runtime.rs`
- 相关原调用在 `crates/hoi4-app/src/main.rs`

前置：P1.3。

任务：

- 迁入 `daily_focus_tick`。
- 迁入 `daily_event_tick`。
- 迁入 `daily_decision_tick`。
- 迁入 situation daily tick。
- 迁入 scripted surrender 触发。

验收：

- `main.rs` 不再直接调用这些 daily content 函数。
- daily content 只由 runtime day boundary 驱动。

验证：

```powershell
cargo check -p hoi4-runtime
cargo check -p hoi4-app
cargo test -p hoi4-content
```

### P1.5 把 AI weekly/strategic tick 纳入 runtime

文件：

- `crates/hoi4-runtime/src/ai_runtime.rs`
- 原调用在 `crates/hoi4-app/src/main.rs`

前置：P1.4。

任务：

- 把 `hoi4_ai::weekly_strategic_tick` 调度移入 runtime。
- cadence 只在 runtime 中判断。

验收：

- `main.rs` 不直接判断 strategic week。
- headless 和 GUI 的 AI cadence 一致。

验证：

```powershell
cargo check -p hoi4-runtime
cargo test -p hoi4-ai
```

### P1.6 headless 改用 `GameRuntime`

文件：

- `crates/hoi4-app/src/bootstrap.rs`

前置：P1.5。

任务：

- `run_headless(...)` 构造 `GameRuntime`。
- 循环调用 `runtime.tick_one_hour()`。

验收：

- headless 不再自己创建独立 schedule 路径。
- headless tick 与 runtime 完全一致。

验证：

```powershell
cargo check -p hoi4-app
```

### P1.7 GUI update 改用 `GameRuntime`

文件：

- `crates/hoi4-app/src/main.rs`

前置：P1.6。

任务：

- `App` 持有 `GameRuntime`，而不是散落持有 `world/econ/research/politics/script/ai/schedule`。
- `App::update` 只负责 real-time accumulator。
- tick 时调用 `runtime.tick_one_hour()`。

验收：

- GUI 和 headless 共用同一 runtime tick。
- `main.rs` 不再直接执行 daily/weekly gameplay tick。

验证：

```powershell
cargo check -p hoi4-app
cargo test -p hoi4-integration
```

### P1.8 integration 改依赖 `hoi4-runtime`

文件：

- `crates/hoi4-integration/Cargo.toml`
- `crates/hoi4-integration/src/lib.rs`

前置：P1.7。

任务：

- 移除为了 tick 而依赖 `hoi4-app` 的路径。
- integration helper 使用 `GameRuntime`。

验收：

- integration tick 不依赖 app shell。
- app crate 不再是 simulation runtime 的公共入口。

验证：

```powershell
cargo test -p hoi4-integration
```

## P2：瘦身 `main.rs`

### P2.1 抽 `content_bootstrap`

文件：

- `crates/hoi4-app/src/content_bootstrap.rs`
- `crates/hoi4-app/src/main.rs`

前置：P1.8。

任务：

- 把事件、决议、focus、situation、history/v6 初始化从 `App::new` 迁出。
- `main.rs` 不直接 include 具体内容文件。

验收：

- 新增国家内容不需要改 `main.rs`。
- content 初始化入口集中。

验证：

```powershell
cargo check -p hoi4-app
cargo test -p hoi4-content
```

### P2.2 抽 `update_loop`

文件：

- `crates/hoi4-app/src/update_loop.rs`
- `crates/hoi4-app/src/main.rs`

前置：P2.1。

任务：

- 把 real-time accumulator、speed -> hours、catch-up loop 迁出。
- update loop 输出 runtime tick 请求和 app events。

验收：

- `App::update` 明显变短。
- gameplay tick 仍只通过 `GameRuntime`。

验证：

```powershell
cargo check -p hoi4-app
```

### P2.3 抽 `input`

文件：

- `crates/hoi4-app/src/input.rs`
- `crates/hoi4-app/src/main.rs`

前置：P2.2。

任务：

- 把键盘、鼠标、地图点击、选择逻辑的输入映射迁出。
- input 层输出 app command，不直接散写 gameplay。

验收：

- `window_event` 中输入分支变薄。

验证：

```powershell
cargo check -p hoi4-app
```

### P2.4 抽 `render_runtime`

文件：

- `crates/hoi4-app/src/render_runtime/`
- `crates/hoi4-app/src/main.rs`

前置：P2.3。

任务：

- 迁移 `RenderState` 初始化和 frame orchestration。
- 暂不强行把 pass 移到 `hoi4-render`，先把 app 内结构理清。

验收：

- `main.rs` 不直接持有大量 render setup 细节。

验证：

```powershell
cargo check -p hoi4-app
```

### P2.5 抽 `debug_commands`

文件：

- `crates/hoi4-app/src/debug_commands.rs`
- `crates/hoi4-app/src/main.rs`

前置：P2.4。

任务：

- 调试命令解析和执行迁出。

验收：

- debug 逻辑不再混在 render/update/input 主路径。

验证：

```powershell
cargo check -p hoi4-app
```

## P3：自研 egui GUI binding

详细设计见 `SELF_GUI_REFACTOR_ROADMAP.md`。这里给线性执行顺序。

### P3.1 更新 GUI 路线文档

文件：

- `CONTRIBUTING.md`
- `docs/vanilla_assets_used.md`

前置：P2.5。

任务：

- 明确 `hoi4-ui` 是自研 egui UI，不是 `.gui` 解释器。
- 明确 `.gfx` 只能由 `hoi4-assets` 解析，`hoi4-ui` 不直接扫。

验收：

- 文档不再互相矛盾。

### P3.2 建立 `UiFrameModel` / `UiAction`

文件：

- `crates/hoi4-ui/src/lib.rs`
- 可新增 `crates/hoi4-ui/src/frame_model.rs`

前置：P3.1。

任务：

- 定义整帧 UI DTO。
- 定义统一 UI action enum。

验收：

- 后续面板能统一接入 DTO/action 模型。

验证：

```powershell
cargo check -p hoi4-ui
```

### P3.3 新建 app `ui_binding`

文件：

- `crates/hoi4-app/src/ui_binding/mod.rs`
- `crates/hoi4-app/src/ui_binding/frame.rs`

前置：P3.2。

任务：

- 建立 `build_frame_model(...)`。
- 建立 `apply_ui_actions(...)`。

验收：

- `main.rs` 通过统一入口构造 UI 数据。

验证：

```powershell
cargo check -p hoi4-app
```

### P3.4 迁移 topbar binding

文件：

- `crates/hoi4-app/src/ui_binding/topbar.rs`
- `crates/hoi4-ui/src/topbar.rs`

前置：P3.3。

任务：

- topbar DTO 构造从 `main.rs` 移到 binding。
- UI 只渲染数据，返回 action。

验收：

- `main.rs` 不再拼 topbar 数据。

验证：

```powershell
cargo check -p hoi4-app
```

### P3.5 迁移 settings / save browser

文件：

- `crates/hoi4-app/src/ui_binding/settings.rs`
- `crates/hoi4-app/src/ui_binding/save_browser.rs`
- `crates/hoi4-ui/src/settings.rs`
- `crates/hoi4-ui/src/save_browser.rs`

前置：P3.4。

任务：

- 先迁低 gameplay 耦合面板。
- 文件 IO 保留在 app binding 或专门 service，不放 UI draw 逻辑。

验收：

- UI crate 不直接扫描 saves 目录。

验证：

```powershell
cargo check -p hoi4-ui
cargo check -p hoi4-app
```

### P3.6 迁移 province / research / production

前置：P3.5。

任务：

- 迁移中等耦合面板。
- 所有可点击状态由 binding 给出，不由 UI 猜。

验收：

- UI 不直接访问 gameplay state。

验证：

```powershell
cargo check -p hoi4-app
```

### P3.7 迁移 politics / diplomacy / event / market

前置：P3.6。

任务：

- 最后迁最复杂面板。
- content model -> UI DTO 全部放 app binding。
- event option、decision activate、diplomacy action 只发 action，不在 UI 执行。

验收：

- `hoi4-ui` 不直接依赖 `hoi4-content`。
- `hoi4-ui` 对 `hoi4-state` 的直接依赖消失或被标记为 legacy。

验证：

```powershell
cargo check -p hoi4-ui
cargo check -p hoi4-app
```

## P4：Asset / Data / Map 加载统一

### P4.1 `GameMap::load_from_paths`

文件：

- `crates/hoi4-map/src/lib.rs`

前置：P3.7。

任务：

- 新增 `load_from_paths(&PathConfig)`。
- 保留 `load(&Path)` wrapper 作为过渡。

验收：

- 地图主加载路径可以走 `PathConfig`。

验证：

```powershell
cargo test -p hoi4-map
```

### P4.2 `GameData::load_from_paths` + LoadReport

文件：

- `crates/hoi4-data/src/loader.rs`

前置：P4.1。

任务：

- 新增 `load_from_paths(&PathConfig)`。
- 新增 `GameDataLoadReport`。
- 停止关键数据 `unwrap_or_default()` 静默吞错。

验收：

- app 启动可打印 loaded counts / warnings。

验证：

```powershell
cargo test -p hoi4-data
```

### P4.3 `.gfx` sprite parser 迁到 `hoi4-assets`

文件：

- `crates/hoi4-assets/src/gfx_sprite.rs`
- `crates/hoi4-ui/src/icons.rs`

前置：P4.2。

任务：

- 用 `clausewitz-parser` 解析 sprite metadata。
- UI 不再 line-based 解析 `.gfx`。

验收：

- `.gfx` parser 只存在于 `hoi4-assets`。
- `hoi4-ui` 不再扫描 `interface/*.gfx`。

验证：

```powershell
cargo test -p hoi4-assets
cargo check -p hoi4-ui
```

### P4.4 UI asset bridge

前置：P4.3。

任务：

- `IconBank` / `NineSlice` 改走 asset bridge 或注入后的 sprite index。
- UI 不做目录扫描和原始文件查找。

验收：

- UI texture 注册保留在 UI 层，但路径解析和字节读取不在 UI 层。

验证：

```powershell
cargo check -p hoi4-ui
cargo check -p hoi4-app
```

## P5：Render / Audio / Assets 边界清理

### P5.1 音频迁出 `hoi4-render`

前置：P4.4。

任务：

- 新建 `hoi4-audio` 或 app audio runtime。
- 迁移 `hoi4-render/src/audio.rs`。
- `hoi4-render` 移除 `rodio` 依赖。

验收：

- render crate 不含 audio。

验证：

```powershell
cargo check -p hoi4-render
cargo check -p hoi4-app
```

### P5.2 删除 render 旧 DDS/mesh parser

前置：P5.1。

任务：

- render 改用 `hoi4-assets` DDS/mesh parser。
- 删除或 re-export 旧模块。

验收：

- DDS/mesh parser 只有一套 authoritative 实现。

验证：

```powershell
cargo test -p hoi4-assets
cargo check -p hoi4-render
```

### P5.3 render pass 逐步迁入 `hoi4-render`

前置：P5.2。

任务：

- 从 app passes 中选择稳定 pass 迁入 render。
- app 只保留 frame orchestration。

验收：

- `hoi4-render` 成为真正渲染库。
- app 不再承载大量 pass 实现。

验证：

```powershell
cargo check -p hoi4-render
cargo check -p hoi4-app
```

## P6：Logic / Script / AI 深层整理

### P6.1 抽 economy 公共逻辑

前置：P5.3。

文件：

- `crates/hoi4-logic/src/economy/market_tick.rs`
- `crates/hoi4-logic/src/economy/planned_tick.rs`

任务：

- 抽 POP education。
- 抽 qualification ratio。
- 抽 input fulfillment。
- 抽 PM unlock。
- 抽 employment hire/fire。

验收：

- market/planned 不再复制大段相同逻辑。

验证：

```powershell
cargo test -p hoi4-logic economy
```

### P6.2 Script effect report

前置：P6.1。

文件：

- `crates/hoi4-script/src/effects.rs`

任务：

- unknown effect 不再完全静默。
- no-op effect 可报告。
- 需要外部系统的 effect 输出 command。

验收：

- 测试能断言 unknown/no-op effect report。

验证：

```powershell
cargo test -p hoi4-script
```

### P6.3 Content / Script runtime 关系收口

前置：P6.2。

任务：

- 明确 `hoi4-content` 是当前自研内容 runtime。
- `hoi4-script` 作为 trigger/effect engine 或 vanilla compatibility，不再和 content daily tick 并行抢职责。

验收：

- focus/event/decision/situation 的权威 tick 入口只在 runtime。

验证：

```powershell
cargo test -p hoi4-content
cargo test -p hoi4-runtime
```

### P6.4 AI / Military 长期战役 invariant

前置：P6.3。

任务：

- 增加 ITA vs ETH、SCW、Sino-Japanese War 等长期测试或 smoke。
- 检查 destination 控制权、frontline 推进、division cleanup。

验收：

- 长期战役不会全军静止或 mapping 崩坏。

验证：

```powershell
cargo test -p hoi4-ai
cargo test -p hoi4-integration
```

## P7：测试和维护

### P7.1 fixture 化测试

前置：P6.4。

任务：

- 默认测试不依赖真实 HOI4 安装。
- 真实 vanilla audit 用环境变量开启。

验收：

- CI 可跑 synthetic fixture。

### P7.2 核心 invariant tests

前置：P7.1。

任务：

- SoA len consistency。
- division/province index consistency。
- corps/division mapping consistency。
- state/province owner consistency。
- daily tick once-only。
- GUI/headless runtime parity。

验收：

- 这些 invariant 有自动化测试覆盖。

### P7.3 warning budget

前置：P7.2。

任务：

- 记录 warning baseline。
- 清理明显 dead code / unused imports。
- 后续 PR 不增加 warning。

验收：

- warning 不再淹没真实问题。

## 总执行顺序压缩版

```text
P0.1 remove_divisions remap
P0.2 planned_tick year
P0.3 economy daily/hourly guard
P0.4 transfer_state province sync
P0.5 test baseline

P1.1 create hoi4-runtime
P1.2 GameRuntime shell
P1.3 tick_one_hour
P1.4 content daily tick
P1.5 AI weekly tick
P1.6 headless uses runtime
P1.7 GUI uses runtime
P1.8 integration uses runtime

P2.1 content_bootstrap
P2.2 update_loop
P2.3 input
P2.4 render_runtime
P2.5 debug_commands

P3.1 GUI docs
P3.2 UiFrameModel / UiAction
P3.3 app ui_binding
P3.4 topbar
P3.5 settings/save
P3.6 province/research/production
P3.7 politics/diplomacy/event/market

P4.1 GameMap load_from_paths
P4.2 GameData load_from_paths + report
P4.3 gfx sprite parser in assets
P4.4 UI asset bridge

P5.1 audio out of render
P5.2 remove render DDS/mesh parser
P5.3 render passes into hoi4-render

P6.1 economy common logic
P6.2 script effect report
P6.3 content/script runtime relation
P6.4 AI/military long-run invariants

P7.1 fixture tests
P7.2 invariant tests
P7.3 warning budget
```

## 使用规则

- 正常情况下按编号顺序做。
- 如果某一步发现阻塞，先记录阻塞原因，不要跳到无关大任务。
- 每完成一个编号任务，就更新这份文档的状态或另建进度文档。
- 不要在 P1 完成前做 P5/P6。
- 不要在 P0 完成前大规模拆 `main.rs`。
