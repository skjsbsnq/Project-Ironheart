# 从这里开始重构

生成日期：2026-05-25

这份文档只回答一个问题：全项目重构到底从哪开始。

它不是缩小范围，不是说只做这里列出的几个任务。它是 `PROJECT_REFACTOR_MASTER_ROADMAP.md` 的入口索引：先按这里走完第一段，再回到总路线图继续 runtime、main.rs、GUI、assets、render、script、AI、测试等后续主线。

## 结论

全项目都要重构，但起手先做三件事，按顺序来：

1. 修核心正确性 bug。
2. 建立唯一 simulation runtime。
3. 再开始瘦身 `main.rs` 和 GUI binding。

不要从 GUI 美化、render pass 迁移、资产大改开始。那些都重要，而且后面会做，但不是第一步。

## 第一阶段：先修 P0 Bug

目标：先让模拟结果不要被明显 bug 污染，否则后面重构会把 bug 搬进新结构。

### Task 1：修 division 删除 remap

文件：

- `crates/hoi4-state/src/world.rs`

重点函数：

- `World::remove_divisions()`
- `scrub_division_index(...)`
- `remap_division_index(...)`

问题：

- 删除 division 后，`command.div_to_corps` 和 `corps.divisions` 可能不一致。

完成标准：

- 删除非末尾 division 后，搬动过来的 division 仍指向正确 corps。
- `corps.divisions` 中没有旧 index。
- `command.div_to_corps` 长度和 `world.divisions` 长度一致。

建议测试：

- 在 `hoi4-state` 增加一个最小测试。
- 构造两个 corps，删除中间 division，断言 mapping 正确。

验证命令：

```powershell
cargo test -p hoi4-state
```

### Task 2：修计划经济年份硬编码

文件：

- `crates/hoi4-logic/src/economy/planned_tick.rs`

问题位置：

- 搜索 `let year = 1936i32;`

问题：

- 计划经济配给永远按 1936 年计算。

完成标准：

- 使用当前世界日期年份，而不是硬编码 1936。
- 至少有一个测试证明非 1936 年不会走错年份。

验证命令：

```powershell
cargo test -p hoi4-logic planned
```

如果测试过滤不方便，就跑：

```powershell
cargo test -p hoi4-logic
```

### Task 3：防止经济 daily/hourly 双 tick

文件：

- `crates/hoi4-logic/src/economy/mod.rs`

重点函数：

- `tick_daily_v6(...)`
- `tick_hourly_spread_v6(...)`

问题：

- 同一天如果两个入口都被调用，经济可能重复结算。

完成标准：

- 明确一个官方入口。
- 或两个入口共享同一套 daily guard。
- 同一天重复调用不会重复增加库存、财政、生产等每日结果。

验证命令：

```powershell
cargo test -p hoi4-logic economy
```

## 第二阶段：建立唯一 Runtime

只有第一阶段做完后再开始。

目标：GUI、headless、integration 都走同一条 tick 路径。

### Task 4：新增 `hoi4-runtime` crate

新增：

```text
crates/hoi4-runtime/
  Cargo.toml
  src/lib.rs
  src/runtime.rs
  src/schedule.rs
```

先迁最小内容：

- `SystemSchedule`
- `SimContext`
- `tick_one_hour` 的纯 runtime 部分

不要一开始迁 GUI、render、input。

完成标准：

- `hoi4-runtime` 能编译。
- `hoi4-app` 仍能通过旧路径跑。
- `hoi4-integration` 暂时可以不改。

验证命令：

```powershell
cargo check -p hoi4-runtime
cargo check -p hoi4-app
```

### Task 5：让 headless 使用 `hoi4-runtime`

文件：

- `crates/hoi4-app/src/bootstrap.rs`

目标：

- `run_headless(...)` 调 `GameRuntime::tick_one_hour()`。
- 不再直接拼一套 schedule 参数。

完成标准：

- headless 和 runtime 使用同一 tick API。
- 不改变 GUI 路径。

验证命令：

```powershell
cargo check -p hoi4-app
```

### Task 6：让 GUI update 使用同一 runtime

文件：

- `crates/hoi4-app/src/main.rs`

目标：

- `App::update` 不直接调用 `daily_focus_tick`、`daily_event_tick`、`daily_decision_tick`、`weekly_strategic_tick`。
- 这些都进入 runtime。

完成标准：

- GUI 和 headless 的 day tick 逻辑一致。
- `main.rs` 只负责 real-time accumulator 和处理 runtime events。

验证命令：

```powershell
cargo check -p hoi4-app
cargo test -p hoi4-integration
```

## 第三阶段：再做 `main.rs` 和 GUI

只有 runtime 初步统一后再开始。

### Task 7：抽 `ui_binding`

新增：

```text
crates/hoi4-app/src/ui_binding/
  mod.rs
  frame.rs
  topbar.rs
```

先迁：

- topbar DTO 构造。

不要先迁：

- politics。
- diplomacy。
- market。
- event。

完成标准：

- `main.rs` 中 topbar 数据构造消失。
- UI 输出仍一致。

验证命令：

```powershell
cargo check -p hoi4-app
```

### Task 8：落实自研 egui 方向

文件：

- `CONTRIBUTING.md`
- `docs/vanilla_assets_used.md`

目标：

- 文档明确 `hoi4-ui` 是自研 egui UI，不是 `.gui` 解释器。
- `.gfx` 策略改成：只能由 `hoi4-assets` 解析，`hoi4-ui` 不直接扫。

完成标准：

- 文档不再互相矛盾。
- 后续 GUI PR 有统一标准。

## 现在不要做的事

暂时不要做：

- 不要先重画 UI。
- 不要先迁 render pass。
- 不要先拆所有 `main.rs`。
- 不要先做 `PathConfig` 大迁移。
- 不要先处理所有 warning。
- 不要同时开 GUI、runtime、assets、script 四条线。

原因：

- 当前最大风险是 tick 路径和核心状态不可靠。
- 先做视觉或文件拆分，会把现有混乱扩散。

## 最小执行队列

如果只想知道今天开哪个任务，就按这个队列：

1. `World::remove_divisions()` remap 修复。
2. `planned_tick.rs` 年份硬编码修复。
3. `economy/mod.rs` daily/hourly tick guard。
4. 新建最小 `hoi4-runtime` crate。
5. headless 改用 runtime。
6. GUI update 改用 runtime。
7. 抽 `ui_binding/topbar`。
8. 更新自研 egui 文档。

## 判断是否可以进入下一步

每一步都满足以下条件才进入下一步：

- 相关 crate 能 `cargo check`。
- 相关测试能跑，或失败项被记录为旧基线。
- 没有把新逻辑继续塞进 `main.rs`。
- 没有新增第二套 tick 或 asset parser。

## 一句话

先修 bug，再统一 runtime，再拆 `main.rs`，最后系统性做 GUI 和资产。不要反过来。
