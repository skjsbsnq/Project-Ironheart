# 贡献指南

> 地图渲染相关改动先读 [`docs/map_renderer_v2.md`](docs/map_renderer_v2.md) 和
> [`docs/map_renderer_v2_contributing.md`](docs/map_renderer_v2_contributing.md)；当前路线图是
> [`MAP_RENDERER_V2_REFACTOR_ROADMAP.md`](MAP_RENDERER_V2_REFACTOR_ROADMAP.md)。

> 一份让 V3 不再像 V1/V2 那样"按层打勾"的工作守则。

## 1. 完成的定义

> **在运行的游戏里能被肉眼或自动化测试观察到。**

V1 在每个 crate 里都打了 ✅ 但主循环根本不调用它们；V2 同样。V3 取消"模块完成"
的概念，只有"集成完成"。判定一项工作是否做完，问以下三个问题：

1. **能不能在 `cargo run --release -- --game-path <fixture>` 起来的二进制里**
   **看到 / 听到 / 跑到这件事？** —— UI 元素出现在屏幕上、音频从喇叭出来、
   AI 真的下了命令、metric 真的上涨。
2. **CI 集成测试能不能在没有人盯着的情况下检出回归？** —— `hoi4-integration`
   crate 的 1d / 30d / 365d smoke 必须覆盖该路径。
3. **资产换一个文件，引擎跟不跟着变？** —— UI / shader / 数据驱动的特性，
   引擎只做"解释器"。Mod 换了 vanilla 同名文件而不动 Rust，行为应改变。

只有三个问题都"是"，才算 done。

## 2. 几条硬规则

* 写新功能前**先**读 `ROADMAP_V3.md` 当前 Phase 的章节，确认这件事是不是地基够了
  再做的。Phase 0 没完之前不要碰 UI / shader 等价层。
* 任何"加载 HOI4 资产"的代码必须走 `hoi4_paths::PathConfig`，禁止再硬编码
  `C:/Program Files (x86)/Steam/...`。
* 渲染相关代码进 `hoi4-render` library；引擎入口 / 事件循环 / 调度器进
  `hoi4-app`。`hoi4-render` 不再有 `main.rs`。
* 加新的脚本 trigger / effect 必须**至少**写一条引用 vanilla 真实使用场景的
  单元测试，证明它在该场景跑出与原版同号的结果。
* 存档兼容**不要**走原版二进制 `.hoi4` 格式（用户已确认释放该需求）；
  自家 round-trip 必须 bit-exact。
* 不在仓库里嵌入任何 Paradox 资产。

## 3. PR 流程

1. 在 `ROADMAP_V3.md` 找到对应 Phase / 子节，本地分支命名 `phase-<N.X>-<short>`。
2. 打开 PR 时贴：
   * **What changed** — 一段话。
   * **Where it shows up at runtime** — 截图 / 日志 / 集成测试 println。
     Phase 0 之后没有这一项的 PR 不合并。
   * **Tests** — 增加 / 修改了哪些；`cargo test --workspace` 通过的截图。
3. 至少跑一次 `cargo run --release` 启动游戏，标题栏 `[systems: ...]` 行截图附 PR。

## 4. Crate 边界

```
hoi4-app           主入口（main.rs + SystemSchedule）
hoi4-render        wgpu 渲染管线（library）
hoi4-ui            (Phase 2) .gui 解释器
hoi4-audio         (Phase 8) rodio + .asset 音频
hoi4-assets        (Phase 2) AssetDb / .gfx / dds / mesh / shader-rt
hoi4-paths         路径配置（HOI4 安装根 + mod 链）
hoi4-logic         经济 / 政治 / 科技 / 军事 / 外交（已存在，待接调度器）
hoi4-script        trigger / effect / event / decision（已存在，待接调度器）
hoi4-ai            战略 / 战术 AI（已存在，待接调度器）
hoi4-state         World / SoA / 存档（已存在）
hoi4-data          country / state / equipment / focus 静态加载（已存在）
hoi4-map           provinces.bmp / heightmap / terrain（已存在）
hoi4-integration   CI smoke + 回归基线
clausewitz-parser  Lexer / Parser / AST
```

不要把渲染逻辑挤进 logic 层，反之亦然。Crate 边界是"可不可以单独装回原系统的
另一半"的划分线。

## 5. 何时暂停加新功能

* 当前 Phase 的"M 验收"截图还没出来。
* `cargo test --workspace` 红的。
* `cargo run --release` 启动后第 1 秒崩。
* 任何看起来像在 V1 套子里"再画一个自定义 UI"的工作 —— 停下来，先把 `.gui`
  解释器做完。

## 6. 不要做的事

* 不要为了"打勾"在 crate 内部加测试而不接到主程序。
* 不要复刻 Paradox 私有格式（`.fxh` 编译器、ironman token 表）。
* 不要在 PR 描述里写"完成"但没有运行时证据。
