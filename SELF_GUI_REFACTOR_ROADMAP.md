# 自研 GUI 重构路线图

生成日期：2026-05-25

范围：本路线图只针对自研 egui GUI 方向。目标是正式放弃 vanilla `.gui` 解释器作为当前主线，把项目现有 UI 从“能跑的手写面板集合”重构成边界清晰、资产入口统一、可测试、可持续扩展的自研 GUI 层。

## 0. 路线裁决

### 0.1 正式选择

本项目后续 GUI 主线采用：

- 自研 egui UI。
- 自研 ViewModel / DTO 绑定层。
- 自研 vanilla 风格主题、9-slice、icon、字体 fallback。
- 允许读取受白名单约束的 vanilla 美术资源。
- 不恢复旧 `.gui` 解释器路线作为近期目标。

### 0.2 不再维持的路线

以下路线在本阶段视为废弃或冻结：

- 不实现 vanilla `.gui` runtime 作为主 UI 框架。
- 不把 `hoi4-ui` 定义为 `.gui` 解释器。
- 不让 UI 面板直接解释 vanilla gameplay 脚本。
- 不继续在 `main.rs` 里无限堆 UI DTO 构造和面板 command 处理。

### 0.3 必须同步更新的旧文档

当前文档和代码路线冲突，重构前应先修正文档，否则后续 PR 审查标准会混乱。

需要更新：

- `CONTRIBUTING.md`：移除或改写“`hoi4-ui` 是 Phase 2 `.gui` 解释器”的表述。
- `docs/vanilla_assets_used.md`：把 `.gfx` 策略从“禁止读取”改成“只能由 `hoi4-assets` 受控解析 sprite metadata，`hoi4-ui` 不直接扫 `.gfx`”。
- 旧 GUI 相关 roadmap：如果仍有 `.gui` interpreter 目标，标记为历史路线或长期研究项。

## 1. 当前问题概览

### 1.1 `hoi4-ui` 边界不清

现状问题：

- `hoi4-ui` 自称 thin UI，但直接依赖 `hoi4-content` 和 `hoi4-state`。
- `politics.rs`、`event_panel.rs`、`law_panel.rs` 等面板直接使用 gameplay/content 类型。
- UI 内部自己扫描 `.gfx`、读 DDS/TGA、注册 texture。
- `save_browser.rs` 等 UI 模块含文件系统扫描逻辑。

目标：

- `hoi4-ui` 只负责展示和局部交互。
- UI 接收纯 DTO/ViewModel。
- UI 返回明确的 `UiAction` / panel action。
- gameplay 读取、命令执行、文件 IO、资产索引构建都从 UI crate 外移。

### 1.2 `main.rs` 里 UI binding 过重

现状问题：

- `App::render` 内直接从 `World`、`EconomyState`、`hoi4-content`、`settings` 等拼大量 UI 数据。
- UI command 处理、面板状态、gameplay 状态读写混在 app 主文件。
- 后续每加一个面板都会继续扩大 `main.rs`。

目标：

- 新建 app-side `ui_binding` 层。
- `main.rs` 只调用 `ui_binding::build_frame_model(...)` 和 `ui_binding::apply_actions(...)`。
- 每个面板的数据构造和动作处理独立成文件。

### 1.3 资产加载体系分裂

现状问题：

- `hoi4-ui::icons` 直接扫描和解析 `.gfx`。
- `hoi4-ui::nine_slice` 自己读 DDS。
- `hoi4-assets::TextureBank` 也有自己的纹理加载和缓存。
- `hoi4-render` 还有旧 DDS/mesh parser。

目标：

- `hoi4-assets` 统一负责 `.gfx` sprite metadata、DDS/TGA decode、raw cache、parsed cache。
- `hoi4-ui` 只接收已经解析好的 icon/sprite handle 或通过一个抽象 asset handle 请求纹理。
- UI 不直接做目录扫描。

### 1.4 文档路线冲突

现状问题：

- `CONTRIBUTING.md` 仍把 `hoi4-ui` 写成 `.gui` 解释器。
- `docs/vanilla_assets_used.md` 禁止 `.gfx`，但代码实际读取 `.gfx`。
- 文档说 V5 不做 mod / replace_path，但部分代码和 `PathConfig` 又在支持 mod chain。

目标：

- 先明确“自研 GUI”为当前路线。
- 明确 vanilla 资产白名单和 `.gfx` 解析权限。
- 明确 UI 层不承担 asset parser 职责。

## 2. 最终目标架构

### 2.1 Crate 职责

目标分层：

```text
hoi4-app
  - winit 生命周期
  - 每帧 orchestration
  - 调用 ui_binding 构造 UI 数据
  - 调用 ui_binding 应用 UI action

hoi4-app::ui_binding
  - World / runtime state -> UI ViewModel
  - UI action -> game command / runtime command
  - 面板级 binding 拆分

hoi4-ui
  - egui shell
  - theme / layout / widget / panel rendering
  - pure DTO 输入
  - pure action 输出
  - 不直接依赖 hoi4-state / hoi4-content / hoi4-logic

hoi4-assets
  - FsAssetDb
  - DDS/TGA decode
  - sprite metadata index
  - 9-slice metadata / texture source
  - icon path resolution

hoi4-render
  - 地图 / 3D / postprocess 渲染
  - 不负责 UI asset parsing
  - 不负责 audio
```

### 2.2 数据流

目标 UI 数据流：

```text
World + RuntimeState + ContentRuntime + Settings
  -> hoi4-app::ui_binding::build_frame_model
  -> hoi4-ui::UiState::begin_frame(frame_model)
  -> Vec<UiAction>
  -> hoi4-app::ui_binding::apply_actions
  -> GameRuntime / World / Settings / SaveSystem
```

### 2.3 资产流

目标 UI 资产流：

```text
PathConfig
  -> hoi4-assets::FsAssetDb
  -> hoi4-assets::SpriteIndex / IconIndex / NineSliceSource
  -> hoi4-app creates UiAssetBridge
  -> hoi4-ui requests/registers egui textures via bridge
```

约束：

- `hoi4-ui` 不调用 `PathConfig::find_all("interface")`。
- `hoi4-ui` 不直接扫 `.gfx`。
- `hoi4-ui` 不直接读 `std::fs::read_dir` 获取 vanilla 资产。
- UI 可以保留 egui texture registration，因为这是 UI 渲染实现细节。

## 3. Phase 0：路线和基线冻结

目标：先让项目所有人知道“自研 GUI 是正式路线”，并记录当前行为基线，不做功能迁移。

任务：

1. 更新 `CONTRIBUTING.md` 的 crate 边界。
2. 更新 `docs/vanilla_assets_used.md` 的 `.gfx` 策略。
3. 新增或更新 GUI 架构文档，引用本路线图。
4. 记录当前 UI 入口和面板列表。
5. 记录当前测试基线。

验收：

- 文档不再同时要求 `.gui` 解释器和自研 egui UI。
- `.gfx` 读取策略只有一个版本。
- 明确 `hoi4-ui` 不直接解析 `.gfx` 的最终目标。

风险：

- 不应在 Phase 0 同时搬代码。否则路线裁决和代码迁移容易混在一起，难以回滚。

## 4. Phase 1：建立 UI ViewModel / Action 规范

目标：先建立面板数据和动作的标准形态，不急着迁全部面板。

### 4.1 定义基础类型

建议在 `hoi4-ui` 中建立：

```rust
pub struct UiFrameModel {
    pub topbar: TopBarData,
    pub panels: PanelModels,
    pub overlays: OverlayModels,
    pub modal: Option<ModalModel>,
}

pub enum UiAction {
    Topbar(TopbarAction),
    Panel(PanelAction),
    Map(MapUiAction),
    Settings(SettingsAction),
    Save(SaveAction),
    None,
}
```

约束：

- `UiFrameModel` 不持有 `World` 引用。
- `UiAction` 不直接执行 gameplay。
- 所有 action 必须可 debug 打印。
- panel action 必须可单元测试。

### 4.2 面板迁移模板

每个面板按以下结构重构：

```text
hoi4-ui/src/<panel>.rs
  - <Panel>Data
  - <Panel>Action
  - draw_<panel>(ui, data) -> Vec<<Panel>Action>

hoi4-app/src/ui_binding/<panel>.rs
  - build_<panel>_data(runtime/world/settings) -> <Panel>Data
  - apply_<panel>_action(action, runtime/world/settings)
```

### 4.3 首批试点面板

优先选择低风险面板：

1. `settings.rs`
2. `save_browser.rs`
3. `topbar.rs`
4. `end_screen.rs`

暂不先迁：

- `politics.rs`
- `event_panel.rs`
- `market_panel.rs`
- `construction_v6_panel.rs`
- `diplomacy.rs`

原因：这些面板直接耦合 gameplay/content/economy，先迁会放大风险。

验收：

- 试点面板 UI crate 不直接访问 gameplay state。
- action 在 app binding 层被处理。
- 面板单元测试可以只构造 DTO，不构造 `World`。

## 5. Phase 2：从 `main.rs` 抽出 `ui_binding`

目标：把 UI 数据构造和 action 应用从 `main.rs` 中迁出。

建议目录：

```text
crates/hoi4-app/src/ui_binding/
  mod.rs
  frame.rs
  topbar.rs
  settings.rs
  save_browser.rs
  politics.rs
  diplomacy.rs
  construction.rs
  market.rs
  province_info.rs
  research.rs
  production.rs
  military.rs
  events.rs
```

### 5.1 `frame.rs`

职责：

- 构造整帧 `UiFrameModel`。
- 汇总当前 open panel。
- 汇总 modal / popup / tooltip 所需数据。
- 不直接执行命令。

### 5.2 panel binding

每个 binding 文件只做两件事：

- `build_data(...)`
- `apply_action(...)`

禁止：

- 在 `build_data` 中修改 `World`。
- 在 UI crate 中读取 `World`。
- 在 UI crate 中调用 logic 函数改变状态。

### 5.3 迁移顺序

推荐顺序：

1. `topbar`
2. `settings`
3. `save_browser`
4. `end_screen`
5. `province_info`
6. `research`
7. `production`
8. `construction_v6_panel`
9. `market_panel`
10. `politics`
11. `diplomacy`
12. `event_panel`
13. `military`

排序依据：

- 先迁只读展示和低副作用面板。
- 后迁会写 gameplay 或依赖 content runtime 的面板。

验收：

- `main.rs` 中不再出现大块 `TopBarData` / `PoliticsData` / `MarketData` 组装逻辑。
- `main.rs` 渲染阶段只调用统一入口。
- 每迁一个面板，至少保留一个 UI DTO 测试或 app binding 测试。

## 6. Phase 3：资产系统重构

目标：让自研 GUI 的资产入口统一，并消除 UI 内部 `.gfx` 扫描。

### 6.1 新增 `SpriteIndex`

建议放在 `hoi4-assets`：

```text
crates/hoi4-assets/src/gfx_sprite.rs
```

职责：

- 解析 `interface/*.gfx` 中的 `spriteType`。
- 建立 `GFX_xxx -> texturefile` 映射。
- 支持 `PathConfig::find_all("interface")`。
- 使用 `clausewitz-parser`，不要 line-based parser。
- 输出 warning/report，而不是静默吞错。

### 6.2 改造 `IconBank`

目标：

- `IconBank` 不再扫描 `.gfx`。
- `IconBank` 接收 `SpriteIndex` 或 `UiAssetBridge`。
- fallback 规则保留：如果 sprite metadata 没命中，再走 `strip GFX_ + search_dir`。
- 所有 fallback 命中/失败进入 asset report。

### 6.3 改造 `NineSlice`

目标：

- `NineSlice` 不直接从路径读文件。
- 由 app/asset bridge 提供 decoded image 或 egui texture handle。
- 9-slice 边距保留在 UI 或 asset metadata 中均可，但来源必须可追踪。

### 6.4 UI texture bridge

建议定义：

```rust
pub trait UiAssetBridge {
    fn icon(&mut self, sprite: &str) -> Option<egui::TextureId>;
    fn nine_slice(&mut self, id: &str) -> Option<NineSliceTexture>;
}
```

如果 trait 放在 `hoi4-ui` 会引入 egui 类型；如果放在 app 侧，则 `hoi4-ui` 只接收 texture id。优先选择简单方案：保留 `hoi4-ui` 内 egui texture 注册，但把索引和字节读取外移。

验收：

- `hoi4-ui/src/icons.rs` 不再出现 `find_all("interface")`。
- `.gfx` parser 只存在于 `hoi4-assets`。
- `.gfx` 相关测试迁到 `hoi4-assets`。
- UI icon 测试使用 synthetic `SpriteIndex`，不依赖真实 HOI4 安装。

## 7. Phase 4：面板解耦深水区

目标：迁移最耦合的 gameplay 面板。

### 7.1 Politics panel

当前风险：

- 直接依赖 `hoi4_content::{Decision, DecisionCategory}`。
- 决议、国策、leader portrait、idea、party、flags 等多源数据混在一起。

目标：

- `hoi4-ui::politics` 只接收 `PoliticsPanelData`。
- `DecisionEntry` 不再从 `hoi4_content::Decision` 构造。
- app binding 负责 content -> DTO。

建议 action：

- `StartFocus { focus_id }`
- `CancelFocus`
- `ActivateDecision { decision_id }`
- `OpenLeaderDetails`
- `OpenLawPanel`

### 7.2 Event panel

当前风险：

- UI 直接使用 `hoi4_content::EventScheduler` 或 content event 类型。

目标：

- UI 只显示 `EventModalData`。
- 选项点击返回 `EventAction::ChooseOption { event_instance_id, option_id }`。
- 事件 immediate/option effect 全部由 runtime/app binding 处理。

### 7.3 Diplomacy panel

当前风险：

- 外交 action 的可用性、失败理由、tooltip 逻辑容易散落在 UI 和 app。

目标：

- diplomacy binding 输出完整 `DiplomacyPanelData`。
- 每个 action 都有：`enabled`、`reason`、`predicted_effect`。
- UI 不再自己推断能否点击。

### 7.4 Market / Construction / Production panels

当前风险：

- 这些面板读取大量经济细节，既像 UI helper 又像经济逻辑。

目标：

- 经济计算留在 `hoi4-logic` 或 app binding helper。
- UI 只显示 rows、summary、warnings、actions。
- 所有“能否建造/能否切 PM/能否采购”的判断由 binding 提供。

验收：

- `hoi4-ui` 可以在不依赖 `hoi4-state`、`hoi4-content` 的情况下编译，或这些依赖被限制在临时 feature 中。
- 复杂面板可用 DTO fixture 单测。

## 8. Phase 5：统一 UI 状态和命令

目标：减少 `App` 中散落的 UI 状态字段。

建议新增：

```rust
pub struct AppUiRuntime {
    pub open_panel: OpenPanel,
    pub selected_country: Option<CountryRef>,
    pub selected_province: Option<ProvinceRef>,
    pub selected_divisions: Vec<DivisionRef>,
    pub modal_stack: Vec<ModalState>,
    pub last_layouts: UiLayoutCache,
    pub settings_ui: SettingsUiState,
}
```

约束：

- `AppUiRuntime` 可以保存 UI-only 状态。
- 不保存 authoritative gameplay state。
- 不复制 `World` 里的事实数据。

收益：

- `App` 字段减少。
- UI 状态生命周期清晰。
- 存档、设置、菜单、游戏内 panel 状态更容易测试。

## 9. Phase 6：测试体系

目标：GUI 重构不能只靠手工点。

### 9.1 UI crate 测试

测试重点：

- DTO -> action。
- 禁用状态不会发 action。
- 多占位符文本格式化正确。
- 列表选择不会选错行。
- 面板在空数据下不 panic。

### 9.2 app binding 测试

测试重点：

- World/content -> DTO。
- action -> command/state change。
- 后端拒绝时 UI reason 正确。
- 常见国家和面板数据完整。

### 9.3 asset tests

测试重点：

- synthetic `.gfx` 解析。
- sprite override 优先级。
- fallback icon 命中。
- unsupported DDS format 报错而不是猜格式。

### 9.4 snapshot / golden tests

可选：

- 对关键 DTO 做 insta snapshot。
- 对 `UiFrameModel` 做 debug snapshot。
- 暂不建议对 egui 像素输出做 golden，维护成本高。

## 10. Phase 7：性能和可观测性

目标：自研 GUI 必须可观测，否则后续会变成帧率黑洞。

### 10.1 Frame stats

已有 `UiFrameStats` 和 `FRAME_BUDGET_US`，应扩展：

- build model time
- egui layout time
- texture upload time
- asset cache hit/miss
- action count
- panel draw time

### 10.2 Asset report

启动时输出：

- sprite index loaded count
- icons loaded count
- missing icons count
- fallback count
- unsupported format count
- `.gfx` parse warning count

### 10.3 UI debug overlay

建议提供开发开关：

- 当前 open panel。
- UI action log。
- hovered widget / blocked map clicks。
- texture cache count。
- last frame model build cost。

## 11. 迁移红线

重构过程中禁止：

- 禁止在 `hoi4-ui` 新增对 `World` 的直接引用。
- 禁止在 `hoi4-ui` 新增对 `hoi4-logic` 的直接调用。
- 禁止在 UI draw 函数里修改 gameplay state。
- 禁止继续在 `main.rs` 里新增大型 panel DTO 构造。
- 禁止新增 line-based `.gfx` parser。
- 禁止静默吞 asset parse error。
- 禁止用“先跑起来”继续扩大 UI 和 app 的耦合。

允许的短期过渡：

- 可以保留旧 `IconBank` fallback，但要标记迁移目标。
- 可以保留 `hoi4-ui` 对 `hoi4-state` 的临时依赖，但新面板不得增加新直接依赖。
- 可以先迁面板数据构造，再迁 action 处理。

## 12. 建议执行顺序总表

| 阶段 | 名称 | 核心产出 | 风险 |
|---|---|---|---|
| Phase 0 | 路线和文档冻结 | 文档一致，自研 GUI 成为正式路线 | 低 |
| Phase 1 | ViewModel/Action 规范 | `UiFrameModel`、`UiAction`、面板模板 | 中 |
| Phase 2 | `ui_binding` 抽取 | `main.rs` UI DTO 逻辑外移 | 中 |
| Phase 3 | 资产系统统一 | `.gfx` 解析进 `hoi4-assets`，UI 不扫目录 | 中高 |
| Phase 4 | 深水面板解耦 | politics/event/diplomacy/economy 面板 DTO 化 | 高 |
| Phase 5 | UI 状态集中 | `AppUiRuntime` | 中 |
| Phase 6 | 测试体系 | DTO/action/binding/asset tests | 中 |
| Phase 7 | 性能与可观测性 | UI perf stats、asset report、debug overlay | 低中 |

## 13. 第一批具体任务清单

建议从以下小步开始：

1. 更新 `CONTRIBUTING.md`，把 `hoi4-ui` 描述改为“自研 egui UI + ViewModel”。
2. 更新 `docs/vanilla_assets_used.md`，明确 `.gfx` 只能由 `hoi4-assets` 解析。
3. 新建 `crates/hoi4-app/src/ui_binding/mod.rs` 和 `frame.rs`。
4. 先迁 `topbar` 的 DTO 构造。
5. 迁 `settings` action 处理。
6. 在 `hoi4-assets` 新建 `gfx_sprite.rs` 的 synthetic parser 测试。
7. 把 `hoi4-ui::icons` 中 `.gfx` 扫描逻辑替换为注入的 sprite index。
8. 为 `politics` 设计纯 DTO，但暂不迁全部逻辑。
9. 给 `UiAction` 加 action log，方便迁移期间排查。
10. 每完成一个面板迁移，删除 `main.rs` 中对应构造块。

## 14. 完成定义

本路线图完成时应满足：

- `hoi4-ui` 不直接依赖 `hoi4-content`。
- `hoi4-ui` 不直接依赖 `hoi4-state`，或依赖仅保留在明确标记的 legacy feature 中。
- `hoi4-ui` 不扫描 `.gfx`。
- `.gfx` sprite metadata 解析集中在 `hoi4-assets`。
- `main.rs` 不再包含大型 UI panel DTO 构造。
- 所有主要面板都有 `Data` + `Action`。
- UI action 统一由 app binding 或 runtime command 层处理。
- 关键面板可用 DTO fixture 测试。
- 启动日志能报告 UI 资产加载质量。

## 15. 最重要的原则

自研 GUI 不是把所有 UI 继续手写到 `main.rs`。

自研 GUI 的正确形态是：

```text
egui 负责画；
hoi4-ui 负责组件和面板；
ui_binding 负责把游戏状态翻译成面板数据；
runtime/logic 负责执行命令；
hoi4-assets 负责解释和加载资产。
```

只要这个边界守住，自研 GUI 可以继续扩展；如果继续让 UI、资产、gameplay、app lifecycle 混在一起，项目会继续膨胀到不可维护。
