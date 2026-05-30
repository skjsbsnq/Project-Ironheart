# Project Ironheart

Project Ironheart 是一个用 Rust 构建的 HOI4 风格大战略游戏技术栈实验项目。当前仓库重点是把数据加载、世界状态、模拟调度、AI、脚本、UI 和渲染逐步收敛到可运行、可测试、可重构的工程结构中。

> 本仓库不应提交 Paradox / HOI4 原版私有资源。运行时通过 `--game-path` 指向本机合法安装目录。

## 当前状态

项目仍处于重构和集成阶段。仓库里包含大量路线图、审计文档和阶段性实现，优先目标是：

- 修复会污染模拟结果的核心逻辑问题。
- 建立统一的 simulation runtime。
- 将 GUI、headless、integration test 接到同一条 tick 路径。
- 逐步整理 `hoi4-app`、`hoi4-render`、`hoi4-ui`、`hoi4-assets`、`hoi4-logic` 等 crate 的边界。

## 项目结构

```text
crates/
  hoi4-app           程序入口、CLI、窗口和主循环
  hoi4-runtime       模拟初始化和系统调度
  hoi4-state         World、存档、运行时状态
  hoi4-logic         经济、军事、政治、贸易等规则逻辑
  hoi4-ai            战略和战术 AI
  hoi4-script        trigger、effect、event、decision 运行逻辑
  hoi4-content       事件、决议、国策和历史内容加载
  hoi4-data          国家、地区、装备等静态数据
  hoi4-map           地图、省份、地形和邻接数据
  hoi4-render        wgpu 渲染管线和 shader
  hoi4-ui            游戏 UI 面板和组件
  hoi4-assets        资源数据库、纹理和 mesh 加载
  hoi4-paths         HOI4 安装目录和 mod 路径解析
  hoi4-integration   集成测试和 smoke test
  clausewitz-parser  Clausewitz 文本格式 lexer/parser
```

## 构建

```powershell
cargo build --workspace
```

Release 构建：

```powershell
cargo build --workspace --release
```

## 测试

运行全部测试：

```powershell
cargo test --workspace
```

按 crate 运行：

```powershell
cargo test -p hoi4-state
cargo test -p hoi4-logic
cargo test -p hoi4-content
```

## 运行

查看命令行参数：

```powershell
cargo run -p hoi4-app -- --help
```

指定 HOI4 安装目录启动：

```powershell
cargo run -p hoi4-app --release -- --game-path "D:\SteamLibrary\steamapps\common\Hearts of Iron IV"
```

无窗口 smoke run：

```powershell
cargo run -p hoi4-app -- --headless --headless-days 30
```

地图审计：

```powershell
cargo run -p hoi4-app -- --map-audit
```

Map Renderer V2 Phase 0 报告：

```powershell
cargo run -p hoi4-app -- --map-phase0-report-only
```

## 工作约定

- 不提交 `target/`、日志、临时 `.exe`、本地 `.env`。
- 不把 HOI4 原版资源嵌进仓库。
- 新功能优先落到对应 crate，避免继续膨胀 `hoi4-app/src/main.rs`。
- 重要逻辑改动至少补一个能证明行为的测试。
- 渲染、UI、AI、脚本等工作需要说明它在运行时如何被观察到。

## 重要文档

- `CONTRIBUTING.md`
- `START_HERE_REFACTOR.md`
- `PROJECT_REFACTOR_MASTER_ROADMAP.md`
- `REFACTOR_EXECUTION_ORDER.md`
- `MAP_RENDERER_V2_REFACTOR_ROADMAP.md`
- `PERFORMANCE_OPTIMIZATION_HANDOFF.md`
