# Project Ironheart

Project Ironheart 是一个用 Rust 构建的二战大战略游戏技术栈实验项目。仓库当前重点是把世界状态、模拟调度、经济/军事/外交规则、AI、脚本、UI 和 wgpu 渲染整理成可运行、可测试、可继续重构的工程结构。

本项目不包含、也不应提交 Paradox 或 Hearts of Iron IV 的原版私有资源。需要读取本机资源时，请通过启动参数指向你自己的合法安装目录。

## 当前状态

项目仍处在重构和集成阶段，主要目标包括：

- 统一游戏 tick、headless smoke run、GUI 和集成测试的运行路径。
- 拆分并收敛 `hoi4-app`、`hoi4-runtime`、`hoi4-state`、`hoi4-logic`、`hoi4-render`、`hoi4-ui` 等 crate 的职责边界。
- 建立可验证的经济、军事、外交、AI、事件、国策和内容加载流程。
- 推进自研 UI 面板、地图渲染和数据驱动内容的工程化。

路线图、审计记录和阶段性研究文档默认只保留在本地，不随 GitHub 仓库发布。

## 项目结构

```text
crates/
  hoi4-app           程序入口、窗口、CLI、主循环和运行时集成
  hoi4-runtime       模拟初始化、调度和系统运行封装
  hoi4-state         World、存档、ID、运行时状态
  hoi4-logic         经济、军事、政治、贸易、外交等规则逻辑
  hoi4-ai            战略、战术、生产、科研和外交 AI
  hoi4-script        trigger、effect、event、decision 脚本运行逻辑
  hoi4-content       事件、决议、国策、历史和情景内容
  hoi4-data          国家、地区、装备、科技、建筑等静态数据
  hoi4-map           地图、省份、地形、河流和邻接数据
  hoi4-render        wgpu 渲染管线、shader 和地图绘制
  hoi4-ui            egui 游戏 UI、面板、组件和主题
  hoi4-assets        资源数据库、纹理、DDS、mesh 和旗帜加载
  hoi4-paths         HOI4 安装目录、mod 和资源路径解析
  hoi4-integration   集成测试与 smoke test
  hoi4-audio         音频接口
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

只检查编译：

```powershell
cargo check --workspace
```

按 crate 运行测试示例：

```powershell
cargo test -p hoi4-ui
cargo test -p hoi4-logic
cargo test -p hoi4-content
```

## 运行

查看命令行参数：

```powershell
cargo run -p hoi4-app -- --help
```

指定本机 HOI4 安装目录启动：

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

## 仓库约定

- 不提交 `target/`、日志、临时二进制、本地 `.env`。
- 不提交 HOI4 原版私有资源。
- Markdown 规划、审计和研究文档默认本地保留，不随 GitHub 发布；当前仓库只跟踪这个 README。
- 新功能优先放入对应 crate，避免继续膨胀应用入口。
- 重要逻辑改动应补测试，UI 和渲染改动应能通过截图、快照或 smoke run 观察。
