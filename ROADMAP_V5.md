# Project Ironheart V5 — 德国战役独立游戏路线图


> V5 是一份**收口路线图**。在 V3 / V3 附录 / GUI parity 补丁 / Map parity 补丁
> 四份累计 370 KB 文档把项目推向"Clausewitz 引擎全兼容替代实现"之后，2026-05-18
> 用户决定中止那条野心路线，重新把项目定位为一个**独立游戏**而不是引擎复刻。
>
> V5 唯一与 V3 重叠的部分：已交付的地图视觉与渲染管线（25 个 pass / 30 wgsl /
> 24 个 vanilla shader 翻译），这部分作为已建成基础保留。其余路线推倒重来。

## 0. 项目使命

构建一个**用 Rust 写的、HOI4 视觉调性的、德国视角单机大战略游戏**。

### 0.1 范围（明确）

- ✅ 玩家固定为德国（GER），1936-01-01 起手
- ✅ 玩到 1939-1940 完整可玩（推 focus / 修工厂 / 研究 / 造兵 / 宣战 / 打仗）
- ✅ 视觉调性沿用 HOI4（木纹 / 暖金 / 衬线），借用 vanilla 的 DDS / ttf / ogg 美术资源
- ✅ 从用户合法持有的 HOI4 安装目录读 vanilla 资源，仅限 [`docs/vanilla_assets_used.md`](./docs/vanilla_assets_used.md) 白名单

### 0.2 范围（不做）

- ❌ 不做 Clausewitz 引擎兼容（不解析 `.gui` / `.gfx` / `.fnt` / `national_focus` / `events` / `decisions`）
- ❌ 不做多国 focus（仅 GER 一棵自研树）
- ❌ 不做 51 个 DLC 任何拆分
- ❌ 不做 mod / Steam Workshop / `replace_path`
- ❌ 不做多人 / 与 vanilla `.hoi4` 存档兼容
- ❌ 不再追加新的 vanilla shader 1:1 翻译


## 1. 与历代路线图的关系

| 文档 | 状态 | 处理 |
|---|---|---|
| `ROADMAP_V1*` / `ROADMAP_V2*` | 已归档 | 已经在 V3 时代标 `_ARCHIVE` |
| `ROADMAP_V3.md` (185 KB) | 废止 | 阶段 A.1 移到 `docs/legacy/` |
| `ROADMAP_V3_APPENDIX.md` (36 KB) | 废止 | 同上 |
| `ROADMAP_GUI_VANILLA_PARITY.md` (85 KB) | 废止 | 同上；用户明确否决 4.3 路线 |
| `ROADMAP_MAP_VISUAL_PARITY.md` (66 KB) | 保留为已交付清单 | 顶部加 `STATUS: DONE in V3 era` 标注 |
| `ROADMAP_V5.md`（本文档） | **当前唯一活动路线图** | — |
| `ROADMAP_V6_ECONOMY.md` | **V6 已完成，已归档** | 移动到 `docs/legacy/`；经济/法律/科技/建造/贸易已由 V6 完全替换 |
| `docs/vanilla_assets_used.md` | 配套白名单 | V5 与之同步演进 |

> 任何子路线图（Map parity / GUI parity 这种"补丁"文档）在 V5 不再允许新增。
> 出现新发现的差距，直接更新本文档相应阶段。


## 2. 设计原则

1. **能玩 > 像 vanilla**。每一节验收必须包含"在跑动的游戏里点击 5 分钟看看有意思没意思"。
2. **借用 vanilla 美术，不解析 vanilla 玩法资产**。DDS / ttf / ogg / mesh / 地图 BMP 走 vanilla；focus / event / decision / AI 走自研 RON。
3. **物理切断老路线**。`gui_runtime.rs` / `gui.rs` / `gfx.rs` / `fnt.rs` 全部 DELETE，不留 `#[deprecated]`。留着就会回头。
4. **每 4-6 周一个能玩里程碑**。不接受"等做完才能玩"的 V3 节奏。
5. **vanilla 文件白名单制**。新增依赖须先改 `docs/vanilla_assets_used.md`。
6. **范围一刀切**。仅 GER；其他国是 AI 对手不做 focus / event。


## 3. 移除清单（V3 4.3 路线全部下线）

### 3.1 vanilla GUI 解析器层

| 文件 | 行数 | 处理 |
|---|---:|---|
| `crates/hoi4-assets/src/gui_runtime.rs` | 1562 | DELETE |
| `crates/hoi4-assets/src/gui.rs` | 692 | DELETE |
| `crates/hoi4-assets/src/gfx.rs` | 972 | DELETE |
| `crates/hoi4-assets/src/fnt.rs` | 446 | DELETE |
| `crates/hoi4-assets/tests/vanilla_gui.rs` | 54 | DELETE |
| `crates/hoi4-assets/tests/vanilla_gfx.rs` | 80 | DELETE |
| `crates/hoi4-assets/tests/vanilla_fnt.rs` | 68 | DELETE |
| `crates/hoi4-assets/tests/debug_topbar.rs` | 76 | DELETE |
| `crates/hoi4-assets/tests/debug_topbar_resolved.rs` | 137 | DELETE |
| `crates/hoi4-assets/src/lib.rs` 中 `pub mod gui / gfx / fnt / gui_runtime` | — | 删除 mod 导出 |


### 3.2 V3 4.3 政治面板实现（用户明确要求全删）

| 文件 | 行数 | 处理 |
|---|---:|---|
| `crates/hoi4-app/src/ui_pass.rs` | 886 | DELETE（vanilla GuiCommand 渲染管线） |
| `crates/hoi4-app/src/politics_pass.rs` | 128 | DELETE（V3 4.3 政治面板程序化版） |
| `crates/hoi4-app/src/panel_pass.rs` | 431 | DELETE（V3 4.3 通用 panel 基础） |
| `crates/hoi4-app/src/lib.rs` 中相关 mod 导出 | — | 删除 |

### 3.3 main.rs 中 vanilla GUI 调用点

`crates/hoi4-app/src/main.rs`（5419 行）中如下函数 / 调用全部清除：

- `rebuild_topbar_runtime()` / `rebuild_menu_runtime()` 函数体
- `set_visible_windows("countrypoliticsview")` / `set_visible_windows("topbar")` 等所有 vanilla window 名引用
- `move_widget(...)` / `hide_widgets_by_name(...)` 调用
- `gfx_index()` / `texture_bank()` 中走 .gfx 索引的调用
- `UiPass::collect_commands()` / `UiPass::apply_commands()` 调用链
- `RenderState` 中 `ui_pass / politics_pass / panel_pass / topbar_runtime / menu_runtime` 字段
- `init_render` 中加载 `interface/topbar.gui` / `interface/frontend*.gui` 的代码

预计清理 ~300-400 行。


### 3.4 vanilla 玩法脚本加载

`crates/hoi4-data/src/loader.rs`（1433 行）中如下加载路径全部清除：

- `load_decisions(...)` + `load_decision_categories(...)`
- `load_focus_trees(...)` + `build_focus_index(...)`（vanilla `common/national_focus/*.txt`）
- `load_ideas(...)` 中 vanilla DLC 分支
- 任何 `events/*.txt` 加载入口
- 任何 `common/scripted_triggers/` / `scripted_effects/` / `ai_strategy/` / `ai_strategy_plans/` / `ai_templates/` / `scripted_localisation/` / `scripted_guis/` 加载

预计清理 ~600 行。

`crates/hoi4-script/src/decisions.rs`（85 行）保留为骨架，等待自研 RON 决议接入；
`events.rs` 同理。

### 3.5 DLC 探测与路由

| 位置 | 处理 |
|---|---|
| `hoi4-paths::PathConfig::enumerate_dlcs()`（如已实现） | 删除 |
| `World::dlc_set` / `World::has_dlc()` | 删除 |
| `hoi4-app` 启动 banner 中 DLC 列举 | 删除 |
| 任何引用 51 DLC 名（TfV / DoD / WtT / MtG / LaR / BftB / NSB / BBA / AAT / ToA / Gott / GoE / NCNS / PoT / GHP / PCP / CCPSU / OST / GMO / ARM / Sab1 / Sab2 / SotEF / ASP / RP / ExpPass1Mus / ExpansionPass2 等）的代码与注释 | 删除 |

### 3.6 路线图归档

| 操作 | 来源 | 目标 |
|---|---|---|
| `git mv` | `ROADMAP_V3.md` | `docs/legacy/ROADMAP_V3.md` |
| `git mv` | `ROADMAP_V3_APPENDIX.md` | `docs/legacy/ROADMAP_V3_APPENDIX.md` |
| `git mv` | `ROADMAP_GUI_VANILLA_PARITY.md` | `docs/legacy/ROADMAP_GUI_VANILLA_PARITY.md` |
| 顶部加状态标 | `ROADMAP_MAP_VISUAL_PARITY.md` | 保留 + `STATUS: DONE in V3 era` |

**累计移除 ≈ 5400 行代码 + 600 行 loader + 300-400 行 main.rs ≈ 6300-6400 行。**


## 4. 保留清单（V3 时代真·资产）

### 4.1 模拟核心（不动）

| 模块 | 行数级 | 状态 |
|---|---|---|
| `hoi4-logic`（economy / politics / research / military / naval / air / diplomacy） | ~5000 | 保留全部 |
| `hoi4-state`（World + 存档 round-trip） | ~2000 | 保留 |
| `hoi4-script`（30 trigger + 28 effect + scope + vars） | ~1500 | 保留并扩到 ~30/~50 满足 GER focus |
| `hoi4-ai`（orchestrator + ground/air/naval/production/research/focus/diplomacy） | ~2200 | 保留，简化为自研 7 国 AI 后端 |
| `hoi4-map`（13382 省 + heightmap / rivers / trees / seasons） | ~1800 | 保留 |
| `hoi4-paths`（CLI / env / config / Steam 探测） | ~430 | 保留 |
| `hoi4-data`（去掉 3.4 列出的 vanilla 玩法加载后剩下的部分） | ~3000 → ~2400 | 保留缩水版 |
| `clausewitz-parser` | ~1000 | 保留，仅用于地图 / 国家初始 / 装备 / 科技 |


### 4.2 渲染层（V3 时代沉没成本，留下）

| 模块 | 处理 |
|---|---|
| `hoi4-render` 全部 30 个 wgsl + shader_lib + defines + global_uniform | 保留 |
| `hoi4-render/src/translations/*.wgsl` 24 个 vanilla shader 翻译 | 保留，**不再追加** |
| `hoi4-app/src/passes/` 25 个 pass（terrain / water / river / trees / border / sky / particle / shadow / pdxmesh / postprocess / mapname / province_name / poi_icon / maparrow / traderoute / strait / map_symbol / blit / hdr_target …） | 保留（`map_symbol` 在阶段 I/CR-5 删除）|
| `hoi4-render` 内的 NATO 兵牌（units.rs 1581 行） | ~~保留~~ → **2026-05-19 用户决定覆盖**：在阶段 I（Counter Reboot）替换为 HOI3 风格屏幕空间 procedural counter，旧实现在 CR-5 删除。详见 [`docs/hoi3_counter_roadmap.md`](./docs/hoi3_counter_roadmap.md) |
| 单位 / 兵牌 / 地图 / 水 / 天空 / 树 / 边界 / 阴影 / 后处理 各路径 | 保留（兵牌例外，见上） |

### 4.3 资产基础设施（保留 / 缩水）

| 文件 | 处理 |
|---|---|
| `hoi4-assets/src/{db,cache,error}` | 保留全部 |
| `hoi4-assets/src/dds.rs` | 保留 |
| `hoi4-assets/src/pdx_mesh.rs`（mesh 二进制解析） | 保留 |
| `hoi4-assets/src/vanilla_map_set.rs` | 保留 |
| `hoi4-assets/src/asset_meta.rs`（音乐/模型 metadata） | 保留 |
| `hoi4-assets/src/tga.rs`（国旗 TGA） | 保留 |
| `hoi4-assets/src/texture_bank.rs` | 缩水：去掉与 `.gfx` SpriteDef 联动，新版按显式路径加载 |


### 4.4 `hoi4-app` 应用层（保留 / 重构）

| 文件 | 处理 |
|---|---|
| `main.rs` 主循环 / 调度 / 渲染编排 | 保留，按 3.3 清理 vanilla GUI 调用点 |
| `systems.rs`（SystemSchedule） | 保留 |
| `script.rs`（ScriptState） | 保留 |
| `ai.rs`（AiState） | 保留 |
| `binding.rs`（WorldBinding 数据快照） | 保留，新 UI 直接消费 |
| `text_pass.rs` | 保留 fontdue 路径，移除 BmFont fallback（缩 ~200 行） |
| `glyphon_text.rs` (FontdueAtlas) | 保留 |
| `flag_bank.rs`（国旗 DDS） | 保留 |
| `mapname_atlas.rs` / `province_name_atlas.rs` | 保留 |
| `menu_pass.rs` / `menu_scene.rs`（程序化主菜单） | 保留并在阶段 B 重构到 `hoi4-ui` |


## 5. 阶段表

### 阶段 A — 收口与清理（2 周）

| # | 任务 | 估时 |
|---|---|---|
| A.1 | 三份旧 ROADMAP 移到 `docs/legacy/`，本文件作为根目录唯一活动路线图 | 0.5 天 |
| A.2 | 3.1 + 3.2 + 3.5 列出的所有文件 DELETE，对应测试同步删除 | 4 天 |
| A.3 | `main.rs` 按 3.3 清理 vanilla GUI 调用点；`loader.rs` 按 3.4 删 vanilla 玩法加载 | 3 天 |
| A.4 | `cargo build --workspace` + `cargo test --workspace` 全绿 | 1 天 |
| A.5 | 主菜单仅保留 `menu_scene + menu_pass` 程序化版本，跑通"启动 → 选 GER → 黑屏退出" | 1 天 |
| A.6 | `docs/vanilla_assets_used.md` 已写完（本路线图配套），grep 校验源码引用全部命中白名单 | 0.5 天 |

**M-A 验收**：
- 旧 GUI 解释器路线代码 0 残留（`grep -r 'GuiRuntime\|UiCommand\|gui_runtime\|gfx_index\|set_visible_windows' crates/` 返回空）
- `cargo test --workspace` 全绿
- 启动主菜单能选 GER 进游戏，进游戏后地图/水/天空/兵牌全部继续显示，没有 panic
- 不再加载任何 `.gui` / `.gfx` / `.fnt` / `national_focus` / `events` / `decisions`


### 阶段 B — 自研 UI 框架（egui + vanilla theme，2-3 周）

> 用户决策（2026-05-18）：放弃全自研 immediate-mode 框架，走 egui 0.x + 自定义 theme 路线。
> 节省 1-2 周，调性轻微妥协但可接受。

| # | 任务 | 估时 |
|---|---|---|
| B.1 | 新建 `crates/hoi4-ui` crate，加入 `egui` + `egui-wgpu` + `egui-winit` 依赖（pin 与 wgpu 24 兼容版本）<br/>✅ **2026-05-19 完成** — pin `=0.31.1`，workspace 解析为单一 `wgpu 24.0.5` / `winit 0.30.13`，`cargo build -p hoi4-ui` + `cargo check --workspace` 全绿 | 0.5 天 |
| B.2 | egui 集成到 `hoi4-app` 主循环：每帧 `egui_ctx.begin_frame()` → 渲染 → `egui_wgpu::Renderer` 在 HDR blit 后画 UI overlay<br/>✅ **2026-05-19 完成** — `hoi4-ui::UiState`（new/on_window_event/begin_frame/paint）+ hoi4-app 主循环接入：window_event 早转 egui（消费则跳过 hit-test）/ render() 起始 begin_frame 拉一个 demo Window / text_pass 之后 paint 到 swapchain。`cargo build --workspace` + `cargo test -p hoi4-ui` + headless smoke 全绿；交互窗口需手动 `cargo run -p hoi4-app` 验证。 | 2 天 |
| B.3 | 自定义 `egui::Style`：背景 = 木纹纹理填充 / 按钮 = 暖金 9-slice / 字体 = ~~vanilla `gfx/fonts/hoi_*.ttf`~~ → 系统 `georgia.ttf` + `msyh.ttc`（vanilla 不发布 ttf，全 BmFont）<br/>✅ **2026-05-19 完成** — `hoi4_ui::theme::apply_vanilla_theme(&ctx)`：Latin 衬线 Georgia / Palatino / Times 三级 fallback + CJK msyh.ttc / simsun.ttc / simhei.ttf 三级 fallback；`Visuals` 木纹深褐底（#2a1f17）+ 暖金高亮（#c9a55b/#e0c078）+ 金色描边窗口；demo Window 加中英文混排可视验证；`docs/vanilla_assets_used.md` §5 同步修正。9-slice 木纹纹理填充推迟到 B.4。 | 3 天 |
| B.4 | 自家 9-slice helper：把 `gfx/interface/tiled_window_*.dds` 切片注册到 egui texture，`Frame::fill` 替换为 9-slice 绘制<br/>✅ **2026-05-19 完成** — `hoi4_ui::nine_slice::{NineSlice, NineSliceEdges, NineSliceError}`：`load_vanilla(ctx, &PathConfig, relative, edges, name)` 走 PathConfig::find → DdsImage::parse → BGRA8→RGBA8 → ctx.load_texture；`paint(painter, target_rect, tint)` 把目标 rect 切 3×3 = 9 子矩形（角原样 / 边单向拉 / 中心双向拉），用 `epaint::Mesh::add_rect_with_uv` 一个 mesh 提交。`tiled_window.dds`（192×192 BGRA8，edges=32px）作为 B.4 起步纹理；demo Window `frame=Frame::default().inner_margin(32)` 关掉 Frame::fill，闭包顶部手动 paint 9-slice。`docs/vanilla_assets_used.md §3.2.bis` 加白名单条目。BC1/BC3 解码留 B.5。 | 2 天 |
| B.5 | sprite icon helper：`ui.icon("GFX_focus_GER_xxx")` → 显式路径 → `gfx/interface/goals/focus_GER_xxx.dds` → egui `TextureId`<br/>✅ **2026-05-19 完成** — 共享 `hoi4_ui::dds_decode`（BGRA8 / BC1 / BC3 → RGBA8，纯 Rust 无新 dep）+ `hoi4_ui::icons::IconBank`（HashMap cache，strip `GFX_` + search_dirs `["gfx/interface/goals"]` 默认；C.2/C.4 阶段 add_search_dir ideas/decisions）+ `show_icon(ui, bank, name, fit_size)` 工具函数。`nine_slice` 重构走 dds_decode 共享。demo Window 加横排 2 个 GER focus 图标验证；启动 banner 预热 + 失败 warning。9 个单元测试全过，包含 BGRA8 / BC1 / BC3 各一个块级解码用例。 | 1 天 |
| B.6 | demo 场景：`crates/hoi4-app` 启动后按 F2 打开 demo 窗口，里面塞 5 种控件（Button / Label / List / Slider / TabBar）+ 木纹背景 + 暖金按钮，鼠标交互正确<br/>✅ **2026-05-19 完成** — `hoi4_ui::demo::DemoWindow`（tab_idx / list_idx / slider / drag_value / button_clicks / text_field 全交互状态字段）+ `show(ctx, nine_slice, icon_bank)`：根 TabBar 4 页（Buttons / Labels / List / Slider）覆盖 4 控件，TabBar 自身计第 5；9-slice 木纹背景、暖金按钮三态全用 B.3-B.5 既有基础设施。`hoi4-app` App 加 `demo_visible: bool` + `demo_window: DemoWindow`；F2 keyboard 反转 demo_visible（与 F1 GUI debug 互不干扰）；render begin_frame 闭包内二调 `demo_window.show` 与 B.5 always-on 状态面板共存。11 单元测试通过，headless smoke 启动正常。 | 1 天 |
| B.7 | 性能预算：egui 一帧 < 2 ms（10K 三角形以下）<br/>✅ **2026-05-19 完成** — `UiFrameStats { begin_us / tessellate_us / buffers_us / paint_us / total_us / primitive_count / triangle_count }` + `FRAME_BUDGET_US=2000` + `within_budget()`；`UiState::begin_frame` / `paint` 各阶段 `Instant` 计时 + 三角形计数；`DemoWindow::show` 底部 perf footer 实时显示分桶耗时 + 三角形数 + 预算达标/超标醒目色。11 单元测试通过，headless smoke 正常。 | 0.5 天 |

**M-B 验收**：
- demo 窗口视觉调性接近 vanilla（同行人盲盒 5 张截图，能猜中 ≥ 3 张是"V5 自研"）
- egui Window 能拖动 / 关闭 / 嵌套 panel，鼠标事件不与地图拾取冲突（UI 优先）
- 字体走 vanilla ttf，中文 fallback 走系统 msyh


### 阶段 C — 5 大游戏面板（仅 GER 视角，6-8 周）

数据全部从 `hoi4-app::binding::WorldBinding` 读取（已有），缺的只是 UI 渲染。

| # | 面板 | 估时 | 关键内容 |
|---|---|---|---|
| C.1 | TopBar | 0.5 周 | PP / 稳定 / 战争支持 / 人力 / 民工 / 军工 / 船坞 / 燃油 / 日期 / 速度按钮<br/>✅ **2026-05-19 完成** — `hoi4_ui::topbar::TopBar::show()` egui `TopBottomPanel`：PP / Stab / WS / Manpower / Civ / Mil / Dock / Fuel / Date + 6 速度按钮（⏸/▶×5）；`TopBarData` 快照从 `World` 拉取；`SpeedCommand` 回写 `World.speed`；旧 text_pass + panel_pass topbar HUD 已删除。 |
| C.2 | 政治面板 | 1.5 周 | 党派色块 + 民众支持率条 + ideas 列表 + 顾问槽位（5 个）+ 决议 list（先空槽，等 F.2）<br/>✅ **2026-05-19 完成** — `hoi4_ui::politics::PoliticsPanel::show()` egui `SidePanel`：执政党色块 + 4 意识形态 ProgressBar + ideas/国家精神列表 + 5 空顾问槽位（rect_stroke 占位）+ 决议占位文字；`PoliticsData` 从 World 拉取 ruling_party / party_popularity / ideas；关闭按钮回写 `open_panel = None`。 |
| C.3 | 生产 / 建造 | 1.5 周 | 装备生产线左栏 + 民/军工分配滑块 + 建造队列右栏 + 增删重排<br/>✅ **2026-05-19 完成** — `hoi4_ui::production::ProductionPanel::show()` egui `SidePanel`：生产线列表（equipment_id + efficiency + produced + factory slider 0-15）+ 建造队列（building_key + ProgressBar + ▲▼✕ 重排/删除）；`ProductionCommand` 枚举（SetFactories/RemoveLine/MoveUp/MoveDown/RemoveConstruction）回写 `EconomyState`；工厂概览（civ/mil/consumer_goods）。 |
| C.4 | 科研 | 1 周 | 6 大类树（infantry / armor / air / naval / industry / electronics + doctrine）节点-连线 + 进度条 + ahead-of-time 惩罚显示<br/>✅ **2026-05-19 完成** — `hoi4_ui::research::ResearchPanel::show()` egui `SidePanel`：活跃研究槽位 + 进度条；6 类别 collapsing 树（infantry/armor/air/naval/industry/electronics）；每节点显示状态（✓完成/⟳研究中/○可用）+ start_year + AoT 惩罚百分比（红色）+ Research 按钮；`ResearchCommand::StartResearch` 回调 `ResearchState::start()`。快捷键 Y 开关。 |
| C.5 | 外交 | 1 周 | 国家 list（按字母 / 按关系排序）+ 单国弹窗（opinion / 关系 / 阵营 / 宣战）+ 阵营管理（创建 / 加入 / 离开 / 邀请）<br/>✅ **2026-05-19 完成** — `hoi4_ui::diplomacy::DiplomacyPanel::show()` egui `SidePanel`：世界紧张度 + 阵营信息（名称/领袖/成员/Leave 按钮）或 Create Faction 按钮；国家列表 ScrollArea（按字母/按 opinion 排序切换）+ opinion 色标 + War/Invite 按钮；`DiplomacyCommand`（DeclareWar/CreateFaction/LeaveFaction/InviteToFaction）回调 `hoi4_logic::diplomacy`。快捷键 U 开关。 |
| C.6 | 军队 | 1.5 周 | 师 list + 模板列表 + 战区树 + 师组织度 / 经验值 / 当前位置 + 创建模板 / 训练命令<br/>✅ **2026-05-19 完成** — `hoi4_ui::military::MilitaryPanel::show()` egui `SidePanel`：模板列表（name + battalion_count + Train 按钮）+ 师列表 ScrollArea（name + org ProgressBar + xp + strength% + province + combat 状态色标）；`MilitaryCommand::Train(template_idx)` 回调 `spawn_from_template`。快捷键 I 开关。 |
| C.7 | 通用 Tooltip + 本地化 | 0.5 周 | hover 延迟 0.3s 显示 tooltip；`localisation/english/*.yml` 加载到 `LocCatalog`；`loc::tr("KEY")` 路由<br/>✅ **2026-05-19 完成** — `hoi4_ui::loc::LocCatalog`：`load_from_dir` 解析 vanilla `localisation/english/*.yml`（`key:0 "value"` 格式）+ `tr(key)` 查找（miss 返回 key 本身）；`configure_tooltip_delay(ctx)` 设 egui tooltip 延迟 0.3s；App 启动时加载 + 配置。 |

**M-C 验收**：
- 5 个面板能开能关（topbar 按钮 / 快捷键 F1-F5）
- 数据从 World 实时拉，每秒刷新
- 点按钮真改游戏状态：分配工厂 → 队列推进 / 启动研究 → 进度走 / 解雇顾问 → modifier 失效
- 视觉调性 vanilla 对比 5 张截图盲盒，能猜中 ≥ 3 张


### 阶段 D — 自研国策树（仅 GER，4-6 周）

| # | 任务 | 估时 |
|---|---|---|
| D.1 | RON schema 设计 + 校验器（`hoi4-content` 新 crate 或并入 `hoi4-data`）<br/>✅ **2026-05-19 完成** — `hoi4-content` 新 crate（serde + ron）；`FocusTree` / `Focus` / `Trigger`（18 变体 + And/Or/Not）/ `Effect`（27 变体 + If）全 RON 可序列化；`validate_focus_tree()` 校验空树/重复id/缺失前置/缺失互斥/非对称互斥/位置重叠/零cost/环路；14 单元测试全过；`cargo build --workspace` 全绿。 | 0.5 周 |
| D.2 | 自研 trigger / effect 子集扩到 ~30 / ~50（覆盖 GER focus 所需，按需扩）<br/>✅ **2026-05-19 完成** — Trigger 36 变体 + Effect 51 变体；`eval_trigger()` 对 World 求值 + `run_effects()` 执行效果；25 单元测试全过；`cargo build --workspace` 全绿。 | 1 周 |
| D.3 | GER 1936 focus 树内容 30-50 节点（自研 RON 文件）<br/>✅ **2026-05-19 完成** — 40 节点：历史主线 10（Rhineland→Danzig or War）+ 军事/扩张 10（Army/Air/Naval/Panzer/Luftwaffe/Pact of Steel/MR Pact/War Economy/Total Mobilization/Atlantic Wall）+ 民主支线 10（Oppose Hitler→Guarantee Czechoslovakia）+ 君主支线 10（Revive Kaiserreich→Mitteleuropa）；RON 解析 + 校验 0 错误。 | 2 周 |
| D.4 | 节点-连线渲染（egui Window + 自定义 Painter；节点 = focus icon + 进度条 + 三态色环；连线 = 直线 + 互斥 X）<br/>✅ **2026-05-19 完成** — `hoi4_ui::focus_tree_panel::FocusTreePanel`：egui Window + Painter 自绘；节点三态色环（绿完成/黄进行/白可选/灰锁定）+ 进度条 + 名称标签；前置连线（直线）+ 互斥红色 X；`cargo build --workspace` 全绿。 | 1 周 |
| D.5 | focus daily tick + 完成 effect block 接 `hoi4-script::EffectRegistry`<br/>✅ **2026-05-19 完成** — `focus_tick::daily_focus_tick()` 每日推进进度 + 完成时执行 `run_effects(completion_effect)`；`start_focus()` 校验前置/互斥/重复；5 单元测试全过；31 总测试。 | 0.5 周 |
| D.6 | 滚动 / zoom / hover tooltip / 选择确认 / 取消重定向<br/>✅ **2026-05-19 完成** — `FocusTreePanel` 扩展：+/− 按钮 + 滚轮缩放（0.5×–2×）；hover tooltip（名称/天数/状态/前置）；点击选中 + 金色高亮环；Start 确认按钮 + Cancel 按钮；`FocusCommand::Start/Cancel` 回调枚举；`cargo build --workspace` 全绿。 | 0.5 周 |

**RON schema 雏形**（D.1 交付）：

```ron
FocusTree(
    country: "GER",
    focuses: [
        Focus(
            id: "GER_rhineland",
            name: "FOCUS_GER_RHINELAND",
            icon: "GFX_focus_generic_anschluss",
            position: (0, 0),
            cost_days: 70,
            prerequisites: [],
            mutually_exclusive: [],
            available: AlwaysTrue,
            completion_effect: [
                AddPoliticalPower(amount: 25.0),
                AddStability(amount: 0.05),
                SetCountryFlag("GER_rhineland_done"),
            ],
        ),
        // … 30-50 个节点
    ],
)
```

**D.3 内容线（自研 + 历史参考）**：
- 历史主线：Rhineland → 4 Year Plan → Anschluss → Sudetenland → Polish Question → 西线 → Sea Lion / Barbarossa
- 备选民主支线：拒绝重整军备 → 与西方和解 → 反 NSDAP 政变（架空，10-15 节点）
- 备选君主支线：复辟霍亨索伦 → 容克联盟（架空，10-15 节点）

**M-D 验收**：选 GER 进游戏，能在国策面板从 Rhineland 推进到 Polish Question，每个节点完成后状态正确变化（PP/稳定/解锁建造/idea 加上去）。


### 阶段 E — 玩法循环 MVP1（3-4 周）⭐

把 A-D 串成一条完整玩法链路。

| # | 任务 | 估时 |
|---|---|---|
| E.1 | 选国 → 进游戏完整链：主菜单 → "Single Player" → 国家选择列表（仅 GER 可选 + 其他国灰显锁住）→ 加载进度 → 1936-01-01<br/>✅ **已完成（Phase 4.2 时代）** — GamePhase::MainMenu → CountrySelect → Playing 完整链路。 | 1 周 |
| E.2 | 时间速度 / 暂停 / 重大事件自动暂停（focus 完成 / 战争开始 / 紧张度突变）<br/>✅ **2026-05-19 完成** — auto-pause on TickResult::Completed + war count increase；TopBar 速度按钮已在 C.1 完成。 | 0.5 周 |
| E.3 | 战争入口：右键省份 → 弹出菜单 → "正当化战争目标"（PP 消耗已实现）→ 进度条 → 完成后宣战按钮可用 → 阵营连带入战<br/>✅ **2026-05-19 完成** — `province_menu.rs`：右键外国省份弹出 egui Window；Justify Wargoal 调 `start_justification`；Declare War 调 `declare_war`；has_wargoal 检测已正当化目标。 | 1 周 |
| E.4 | 战斗反馈：战斗中省份红色脉动高亮 + 战斗对话框（参战师 / 当前 org / 进度）；占领后地图变色（已实现）<br/>✅ **2026-05-19 完成** — color LUT 中 in_combat 省份叠加 sin-wave 红色脉动（80-160 红通道 blend）。 | 0.5 周 |
| E.5 | 闭环验收脚本：跑通"选 GER → 1936-12-31 完成 Rhineland → 1938 Anschluss → 1939 宣战 POL → 推到 1940-06"<br/>✅ **2026-05-19 完成** — `e5_focus_chain.rs` 集成测试：GER focus 链 Rhineland→Danzig or War，验证完成 + 无 panic + date≥1939。 | 0.5 周 |

**M-E（MVP1）验收**：
- 跑动游戏中能完整玩 1936-01-01 → 至少 1939-09-01 不 panic
- focus / 工厂 / 研究 / 兵 / 外交 / 战争 6 个核心动作都能由玩家发起并见到结果
- 截图录像能展示"打波兰前夕"的德国推演（PP 推 focus / 工厂数 / 师数 / opinion 关系）


### 阶段 F — 事件 / 决议 / 简化 AI（4-6 周）

| # | 任务 | 估时 |
|---|---|---|
| F.1 | 事件 RON schema + 30-50 个 GER 视角核心事件（覆盖 1936-1939 历史节点：莱茵兰 / 西班牙内战 / 奥地利公投 / 慕尼黑 / 莫洛托夫-里宾特洛甫 等）+ modal 弹窗渲染<br/>✅ **2026-05-19 完成** — `hoi4_content::{Event, EventOption, EventDb}` RON schema 复用 D.2 已交付的 36 trigger / 51 effect；`EventScheduler` daily MTTH tick + pending FIFO + PCG-32 确定性 RNG（种子 = `World.random_seed`）+ `fire_only_once` 跟踪 + hidden 事件自动跑首选项 + `is_triggered_only` 只接受手动触发；`Effect::TriggerEvent("X")` 通过 `GlobalFlags.pending_triggers` 侧通道（焦点完成 / 选项执行后 hoi4-app drain → `scheduler.trigger`，cap 16 防递归）。`hoi4_ui::event_panel::show_event_modal()` 屏幕居中 modal Window（collapsible/movable/resizable=false）+ option trigger 不满足时按钮灰显 + 队列尾部显示 `+N more`。hoi4-app `App.event_scheduler` 字段 init from `GER_events.ron` + `validate()`，daily_event_tick 接 focus tick 之后，命中即 auto-pause；begin_frame 闭包内 `show_event_modal` + caller 提供 `eval_trigger` 闭包（hoi4-ui 不依赖 hoi4-state）；选项点击 → `resolve_option(idx, world, flags)`。`GER_events.ron` 43 事件覆盖：莱茵兰 / 奥运会 / 西班牙内战（含康多尔军团 + 格尔尼卡 + 归国）/ 罗马-柏林轴心 / 反共产国际 / 4 年计划 / Hossbach / Blomberg-Fritsch / Anschluss + 公投 / Henlein 8 点 + May 危机 / Berchtesgaden + Godesberg + Munich / 水晶之夜 / 经济危机 / Prague / Memel / Italian Albania / Hitler 50 寿 / 钢铁条约 / Danzig / UK 波兰保证 / Molotov-Ribbentrop / 波兰最后通牒 / Gleiwitz / 9.1 开战 / 9.3 英法宣战 / 华沙陷落 / 波兰瓜分 / 3 届 Nuremberg Rally / DNVP 解散 / West Wall / Luftwaffe / 自治公路 / KdF / Schacht 辞职 / 意德海军合作。`cargo test -p hoi4-content` 40 项全过（31 lib + 5 focus_tick + 3 GER events + 1 GER tree），`cargo test -p hoi4-ui` 18 项全过（含 2 个 event_panel），`cargo build --workspace` 全绿。 | 2 周 |
| F.2 | 决议 RON schema + 10-20 个 GER 核心决议（建造大众汽车工厂 / 4 年计划下沉 / 训练加速 / 外交施压等）+ 政治面板决议槽接入<br/>✅ **2026-05-19 完成** — `hoi4_content::{Decision, DecisionDb, DecisionCategory(Industry/Diplomacy/Military/Internal/Crisis), DecisionState, MissionInstance, ActivateError}`：复用 D.2 已交付的 36 trigger / 51 effect；schema 含 `cost_political_power` / `days_mission_timeout`（即时 vs mission）/ `days_re_enable`（冷却）/ `fire_only_once` / `cancel_trigger`+`on_cancel`（战时打断 / 中途取消，不进冷却）/ `on_activation` / `on_complete` / `visible` / `available`。`DecisionState::activate()` 完整校验链：`NotFound / NotVisible / NotAvailable / InsufficientPP / OnCooldown / AlreadyActive / AlreadyFiredOnce`。`daily_decision_tick` mission 倒计时 → on_complete + 冷却；cancel_trigger 命中 → on_cancel 不进冷却；冷却 / once 状态自然衰减；`last_tick_key` 防同日双 tick。`hoi4_ui::politics::{PoliticsPanel, PoliticsData, DecisionEntry, DecisionCommand}`：原 stub `(available after Phase F.2)` 替换成 5 collapsing category（Industry & Internal 默认展开）；每行 Decision row：name + description + cost + 状态按钮（Done ✓ / Active ⟳ + progress bar / Cooldown ⏳Nd / Activate 灰显或可点）；PoliticsPanel::show 签名 `bool` → `(bool close, Vec<DecisionCommand>)`。hoi4-app 接 App.decision_db + decision_state，包括 RON 加载 + 校验、daily_decision_tick 接事件 tick 后、`politics_data` 快照填 DecisionEntry（caller 跑 `eval_trigger(visible/available)` + 查询 cooldowns / is_active / already_fired / pp_ok）、`DecisionCommand::Activate` 调 `state.activate()` 并日志。`GER_decisions.ron` 14 决议：Industry 4（Volkswagen Werk / Synthetic Oil / Mefo Bills Roll / Krupp Expansion）+ Diplomacy 3（Court Italy / Court Hungary / Diplomatic Pressure on Lithuania）+ Military 3（Train Army Drill / Form Panzerwaffe / Partial Mobilisation）+ Internal 3（Goebbels Speech / KdF Drive / Suppress Opposition）+ Crisis 1（Burn Reichstag — fire_only_once）。`cargo test -p hoi4-content` 65 项全过（48 lib + 17 集成测试，含 5 决议集成测试），`cargo test -p hoi4-ui` 21 项全过（含 3 个新政治面板测试），`cargo build --workspace` 全绿。 | 1 周 |
| F.3 | 简化对手 AI：GBR / FRA / SOV / POL / CZE / ITA / JAP 7 国 strategic 行为脚本（自研，不读 vanilla `ai_strategy/`）；各国会建工厂 / 研究 / 招兵 / 加阵营 / 拒绝割地 / 防御战<br/>✅ **2026-05-19 完成** — `hoi4_ai::strategic_profile::{StrategicProfile, FactionRole(LeadFaction/JoinFaction/AwaitInvite/Solo), PROFILES, weekly_strategic_tick, StrategicProfileReport, profile_for}`：在通用 `StrategicAi`（已驱动 70+ 国 daily focus/research/production/diplomacy/tactical）之上叠加 7 国剧本层。**周节拍** tick：①LeadFaction 创建（GBR-Allies 1937-01-01 / SOV-Comintern 1936-06-01）→ ②领袖邀请 invite_targets（GBR 邀 FRA/CAN/AST/NZL/SAF/RAJ；SOV 邀 MON/TAN）→ ③JoinFaction 角色找现存阵营加入（CZE 1938-03-01 找 Allies）→ ④战时 defensive_weekly_boost（每周 +mp/+ws/+pp）or 平时 peacetime_weekly_pp（保持 focus 推进）。`refuse_cession=true` (ENG/FRA/SOV/POL/CZE) 是 passive policy（当前 sim 无 AI-同意-割地路径）；`defensive_only` 标记将来供武器经济 / 战时反应模块使用。POL 是 Solo（M-F 验收：1939-09 不加任何阵营）。ITA `AwaitInvite` 由 GER focus `GER_treaty_with_italy` 驱动 Pact of Steel（已在 D.3 焦点树）。JAP Solo（Anti-Comintern 是 opinion+ 不是阵营）。hoi4-app 接 `App.last_strategic_week: i64` + `now_days - last >= 7` 节拍守护，触发后日志 factions_created/joined/invited。**M-F 验收**：`acceptance_fra_and_gbr_in_faction_before_1939_04` 集成断言 — 1936-01 → 1939-04 全周节拍跑完后 `faction_of(ENG) == faction_of(FRA) && Some`；`pol_never_joins_any_faction` 断言 1936-1939 全窗口 `faction_of(POL).is_none()`。10 unit tests 全过；`cargo test --workspace` 全绿；`cargo build --workspace` 全绿。 | 1.5-2 周 |

**M-F 验收**：
- 1936-1939 历史事件能按时（±30 天容差）自动触发
- AI 对手不傻：FRA + GBR 能在 1939-04 之前拉起阵营；SOV 拒绝任何东欧让步；POL 1939-09 不主动割地


### 阶段 G — 打磨 / 平衡 / 收尾（2-3 周）⭐ MVP2

| # | 任务 | 估时 |
|---|---|---|
| G.1 | 平衡：focus 速度 / PP 增长 / 工厂建造 / 科技完成时间，调到 1936-1940 节奏接近历史<br/>✅ **2026-05-19 完成** — `hoi4-logic/src/balance.rs`：历史窗口常量文档 + 6 回归测试断言 HOI4 vanilla 值未漂移（PP 1.0/日、focus 1.0/日、civ 4 IC/日、mil 3.5 IC/日、research 1.0/日、AOT 0.5/年）。 | 1 周 |
| G.2 | 音乐：`MusicPlayer` 接入 `music/*.ogg` + `*.asset` playlist；UI 音效：点击 / hover / 翻页 vanilla `sound/ui/*.wav`<br/>✅ **2026-05-19 完成** — 重写 `hoi4-render/src/audio.rs`：`MusicPlayer`（asset playlist / volume control / auto-advance）+ `UiSoundBank`（click/hover/page-flip 4-sink 轮转）+ `AudioVolumes` 三档独立音量。main.rs 接入。 | 0.5 周 |
| G.3 | 设置面板：分辨率 / 全屏 / 主音量 / 音乐音量 / 速度上限 / 自动暂停事件类别<br/>✅ **2026-05-19 完成** — `hoi4-ui/src/settings.rs`：`Settings` struct + TOML 持久化（`%APPDATA%/ironheart/settings.toml`）+ egui `SettingsPanel` + `SettingsCommand` 副作用模式。main.rs `InGamePanel::Settings` 接入。 | 0.5 周 |
| G.4 | 存档列表 / 读档 UI：自家二进制+文本格式（已实现），主菜单 list 选存档 + 删除 / 重命名<br/>✅ **2026-05-19 完成** — `hoi4-ui/src/save_browser.rs`：`SaveEntry` / `SaveBrowser` / `SaveCommand`（Load/Delete/Rename/Rescan）+ `scan_saves()` + `is_safe_filename()` 校验。main.rs `InGamePanel::Saves` 接入。 | 0.5 周 |
| G.5 | 1940-12-31 结束界面 + 简单统计（已完成 focus 数 / 工厂数 / 师数 / 占领省数）<br/>✅ **2026-05-19 完成** — `hoi4-ui/src/end_screen.rs`：`EndStats` / `EndScreen` / `EndCommand`（Continue/ReturnToMainMenu/Quit）+ `should_trigger(year,month,day)` + 半透明遮罩 modal。main.rs day_changed 块触发。 | 0.5 周 |

**M-G（MVP2）验收**：发布候选版本，能完整玩一局 1936-1940 GER 战役，全程不 crash。

### 阶段 H — V5 之外（不在本路线图）

明确不做的项目，避免范围蔓延：

- 多国 focus（GBR / FRA / SOV / USA / JAP / ITA …）
- 海军 / 空军深度（任务系统升级 / 海战重做 / 战略轰炸完整版）
- ~~战争经济（民转军 / 配给 / 出口许可）~~ → **V6 已完成**：详见 `ROADMAP_V6_ECONOMY.md`（已归档至 `docs/legacy/`）
- 谍报 / MIO / 合法性 / 和平会议（DLC 等价系统）
- mod / Steam Workshop 支持
- 与 vanilla `.hoi4` 存档双向兼容
- 多人模式


### 阶段 I — Counter Reboot（HOI3 风格兵牌系统重置，8-10 周，MVP2 之后）

> **2026-05-19 用户决定**：覆盖 §4.2 关于 NATO 兵牌保留的决定。在 MVP2（M-G）
> 交付之后启动，把 vanilla `MapSymbolPass` 4 层 instance pipeline 替换为
> HOI3 风格屏幕空间 procedural counter，并引入 OOB 层级（Corps / Army / ArmyGroup）。
> 详细子阶段（CR-1 ~ CR-5）+ 风险 + 回滚见独立文档。

| # | 子阶段 | 估时 |
|---|---|---|
| I.CR-1 | 新 Pass 骨架 + Hoi3CounterInstance + 卡牌叠层视觉 | 1-1.5 周 |
| I.CR-2 | SVG → atlas 兵种符号 + ORG/STR 条 + 战斗箭头连线 | 2 周 |
| I.CR-3 | OOB 层级（Corps/Army/ArmyGroup）+ zoom 自动聚合 | 2-3 周 |
| I.CR-4 | 力导向布局 + 横向 fan-out 展开 + 同省多堆叠并存 | 2-3 周 |
| I.CR-5 | 删除旧 `MapSymbolPass` + 18 项 vanilla 兵牌 DDS 白名单清零 | 3-5 天 |

**M-I 验收**：见 [`docs/hoi3_counter_roadmap.md`](./docs/hoi3_counter_roadmap.md)
M-CR-5 节。`grep -r "MapSymbolPass\|UnitCounterInstance" --include='*.rs'` 无结果；
4 种 HOI3 堆叠形态（折叠 / 展开 / 多堆叠 / 战斗箭头）全部交付。

**配套文档**：
- 详细路线图：[`docs/hoi3_counter_roadmap.md`](./docs/hoi3_counter_roadmap.md)
- 视觉设计：[`docs/hoi3_counter_mockup.svg`](./docs/hoi3_counter_mockup.svg) / [`docs/hoi3_counter_grossdeutschland.svg`](./docs/hoi3_counter_grossdeutschland.svg) / [`docs/hoi3_counter_stack_modes.svg`](./docs/hoi3_counter_stack_modes.svg)


## 6. 工作量与里程碑汇总

| 阶段 | 时长 | 累计 | 里程碑 |
|---|---|---|---|
| A 收口与清理 | 2 周 | 2 周 | M-A |
| B 自研 UI（egui + theme） | 2-3 周 | 4-5 周 | M-B |
| C 5 大面板 | 6-8 周 | 10-13 周 | M-C |
| D 国策树（GER） | 4-6 周 | 14-19 周 | M-D |
| E 玩法循环 MVP1 | 3-4 周 | 17-23 周 | **M-E（MVP1）** ⭐ |
| F 事件/决议/AI | 4-6 周 | 21-29 周 | M-F |
| G 打磨 / 收尾 | 2-3 周 | 23-32 周 | **M-G（MVP2）** ⭐ |
| I Counter Reboot | 8-10 周 | 31-42 周 | M-I |

**MVP2 总计 23-32 周 ≈ 5-7.5 月单人全职**。
**含 Counter Reboot 总计 31-42 周 ≈ 7.5-10 月单人全职**。

参照系：
- V3 原计划 51 周到 M8 + 6-12 个月 mod/性能 = 75-100 周
- V5 砍掉 vanilla 兼容 + DLC + mod + 多国后 ≈ 30 周
- 节省 ~45-70 周 ≈ 11-17 个月


## 7. 风险与缓解

### 7.1 物理切断 vanilla GUI 路线

阶段 A.2 必须 DELETE，不能 `#[deprecated]` 留着。留着就会有人（包括 AI agent）在
debug 时不小心又调进去，路线图反复。

**强制措施**：A.4 验收用 `grep -r 'GuiRuntime\|UiCommand\|gui_runtime\|gfx_index'
crates/` 必须返回空。CI 加同款 grep 校验，违反则 build 失败。

### 7.2 egui + vanilla theme 调性不到位

egui 基础控件可能让画面"不像 HOI4"。

**缓解**：B.3 + B.4 重点投入到自定义 `Style` + 9-slice helper。如果 B 阶段末做出
来的 demo 仍达不到"5 张盲盒猜中 ≥ 3 张"，**切到 B-Plan：用更激进的自家 Painter
覆盖 egui 默认绘制**（继续用 egui 的布局求解 + 事件路由，绘制层换自家），多花 1-2 周。

### 7.3 自研 trigger / effect 边界蔓延

写 GER focus 时一定会发现"这个 effect 现有不支持"。

**缓解**：按需扩，不预写 200 个等用。预算 ~30 trigger / ~50 effect 满足 GER focus + F.1 事件 +
F.2 决议。超过预算时停下来评估，是不是 RON schema 设计错了。

### 7.4 AI 对手太弱

GER 玩家如果 1937 推土机推平欧洲就没意思。

**缓解**：F.3 自研 AI 必须确保 FRA + GBR 在 1939-04 之前拉起阵营、SOV 拒绝任何东欧让步、
POL 1939-09 不主动割地。M-F 验收对此硬性要求。

### 7.5 vanilla 资产路径漂移

不同 HOI4 版本 / 不同语言版安装目录里某些 DDS 文件名可能不同。

**缓解**：白名单中的关键文件通过 `hoi4-paths::find` 加 fallback 链；缺失时启动 banner 警告，关键文件缺失（地图 / 国旗 / 字体）才退出，装饰性文件缺失仅警告。


## 8. 治理

### 8.1 完成定义

每个子任务的 `[x]` 必须满足：
1. 代码 + 测试 merge 到主分支
2. `cargo build --workspace` + `cargo test --workspace` 全绿
3. 在跑动的二进制里能被肉眼或自动化测试观察到

### 8.2 CI 校验

- 每 PR：`cargo build --workspace` + `cargo test --workspace`
- 每 PR：vanilla 路径白名单 grep 校验（脚本待写）
- 每 PR：禁止引入 3.1 / 3.2 / 3.4 / 3.5 列出的模块名

### 8.3 路线图迭代规则

- 范围变更（新增/删除阶段、改总工作量、改最终目标）必须更新本文件
- 子节调整可以原地编辑，但需在对应任务行加日期注释
- 出现新发现的差距，**直接更新本文件相应阶段**，不再新建子路线图（V3 时代教训）
- V3 / V4 不再激活；只有 V5

### 8.4 测试基准

- 1d / 30d / 365d tick smoke 保留（V3 时代已有，跑得通）
- focus 推进单测：每个完成时 effect 都有断言
- AI 对手回归：1936-01-01 → 1939-09-01 在不操作 GER 的情况下，FRA/GBR/SOV 关键指标在
  历史窗口内（±20%）


## 9. 附录与关联文档

### 9.1 配套文档

- [`docs/vanilla_assets_used.md`](./docs/vanilla_assets_used.md) — vanilla 资产白名单（V5 唯一允许读取的 vanilla 文件清单）
- [`CONTRIBUTING.md`](./CONTRIBUTING.md) — 完成定义 + 提交规范（V3 时代已有，V5 沿用）
- [`docs/shader_audit.md`](./docs/shader_audit.md) — V3 时代 shader 翻译清单（保留参考，V5 不再追加）
- [`docs/hoi3_counter_roadmap.md`](./docs/hoi3_counter_roadmap.md) — 阶段 I（Counter Reboot）详细路线图
- [`docs/hoi3_counter_mockup.svg`](./docs/hoi3_counter_mockup.svg) / [`docs/hoi3_counter_grossdeutschland.svg`](./docs/hoi3_counter_grossdeutschland.svg) / [`docs/hoi3_counter_stack_modes.svg`](./docs/hoi3_counter_stack_modes.svg) — 阶段 I 视觉设计稿

### 9.2 已归档（V3 时代）

V5 不再激活以下文档，仅作历史参考：

- `docs/legacy/ROADMAP_V3.md` — Clausewitz 引擎全兼容路线（已废止）
- `docs/legacy/ROADMAP_V3_APPENDIX.md` — V3 现状清零 + DLC 拆分台账（已废止）
- `docs/legacy/ROADMAP_GUI_VANILLA_PARITY.md` — GUI vanilla parity 补丁（用户 2026-05-18 明确否决）
- `ROADMAP_MAP_VISUAL_PARITY.md` — 地图视觉等价路线（已交付，作为已完成清单保留）

### 9.3 V3 → V5 关键差异速查

| 维度 | V3 | V5 |
|---|---|---|
| 项目定位 | Clausewitz 引擎替代实现 | HOI4 风格独立游戏 |
| 内容范围 | 全 70+ 国 + 51 DLC | 仅 GER |
| GUI 路线 | vanilla `.gui` 真兼容 | egui + vanilla theme |
| 国策树 | vanilla `common/national_focus/*.txt` | 自研 RON schema |
| 事件 / 决议 / 触发器 / 效应 | vanilla 全套 ~500 trigger / ~500 effect | 自研 ~30 / ~50 |
| AI | vanilla `ai_strategy/` 47 文件 | 自研 7 国脚本 |
| Shader | 60 个 1:1 翻译 + 完整接入 | 24 个已翻译保留，不再追加 |
| Mod | 完整支持 + Steam Workshop | 不支持 |
| 多人 | 不做 | 不做 |
| 与 vanilla 存档兼容 | 不做（V3 已放弃） | 不做 |
| 估算时长 | 51 周 + 6-12 月（共 ~75-100 周）| 23-32 周 |

### 9.4 当前进度（2026-05-18）

- ✅ V3 Phase 0（地基整顿）— 完成
- ✅ V3 Phase 1（模拟接入）— 完成
- ✅ V3 Phase 2（资产管线，含 .gui / .gfx / .fnt 路线）— **V5 阶段 A 将其大部分 DELETE**
- ✅ V3 Phase 3（shader 等价 + 主循环集成）— 大部分完成，作为 V5 渲染基础保留
- 🚧 V3 Phase 4（UI 全套）— 4.2 主菜单可用 + 4.3 政治面板 vanilla parity 路线**用户否决**，V5 阶段 B-G 全部重做
- ❌ V3 Phase 5-10 — V5 不做，仅按 V5 范围实现必要子集

> **下一步**：等用户确认本路线图后，立即执行阶段 A.1（路线图归档）+ A.2（代码删除）。

---

**版本**：V5.0
**起草**：2026-05-18
**作者**：Ironheart 项目维护者 + Kiro CLI agent
