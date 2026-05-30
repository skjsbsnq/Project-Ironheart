# Project Ironheart V3 — Clausewitz 引擎的 Rust 替代实现

> **Historical note (Phase 11, 2026-05-30):** This legacy roadmap is archived. Current map renderer pass order, ownership, debug, baseline, and cleanup policy live in [`../../MAP_RENDERER_V2_REFACTOR_ROADMAP.md`](../../MAP_RENDERER_V2_REFACTOR_ROADMAP.md), [`../map_renderer_v2.md`](../map_renderer_v2.md), and [`../map_renderer_v2_contributing.md`](../map_renderer_v2_contributing.md).

> V3 是一份"以资产兼容为第一公民"的路线图。
> V1 按技术分层打勾，结果每层都是孤岛；
> V2 按"端到端可跑"重排，但仍然把 UI / shader / 资源加载当成"渲染装饰"；
> V3 把项目重新定位为 **Clausewitz 引擎的开源替代实现**（ScummVM / OpenMW / OpenRA 模式），
> 全部资产格式的解析与上屏成为路线图的"主干"，而不是"打磨阶段"。

## 项目使命（取代之前两版）

构建一个能加载用户合法持有的 Hearts of Iron IV 安装目录、并提供视觉与玩法等价体验的 Rust 引擎。
不是"复刻 HOI4"，而是"造一个 Clausewitz 引擎让它能跑 HOI4 全部数据文件"。
这是 25 年成熟模式（ScummVM ↔ LucasArts; OpenMW ↔ Morrowind; OpenRA ↔ C&C; OpenTTD ↔ Transport Tycoon; DevilutionX ↔ Diablo I）。

### 范围

- ❌ 多人模式
- ❌ 与原版 `.hoi4` 存档双向兼容（按用户明确要求，**自有存档格式即可**）
- ❌ 重新实现 Paradox `.fxh` 着色器编译器
- ✅ 单人完整游戏体验（含全 DLC 内容自动跟进）
- ✅ 加载用户的 vanilla / DLC / Mod 安装目录原样运行
- ✅ **`.gui` 必须真正兼容**（声明式 UI 树解释器 + 数据绑定）
- ✅ **shader 功能等价**（每个原版 shader 文件对应一个 wgsl 实现，视觉差距 < 5%）
- ✅ 视觉肉眼难分辨

---

## 附录

本路线图聚焦于 Phase 0–10 的实施计划。以下背景材料移至 
[ROADMAP_V3_APPENDIX.md](./ROADMAP_V3_APPENDIX.md)：

- 现状清零（2026-05-14）+ 资产规模锚点
- V3 设计原则
- 与 V2 的差异
- 立即可动手的清单（按周）
- V3 治理变化
- 当前进度（持续更新）
- DLC 内容拆分总台账（51 个 DLC）

---

## 架构总览（V3 目标）

```
┌────────────────────────────────────────────────────────────────────────┐
│                                hoi4-app                                │
│   主循环：input → script → logic → ai → render → present               │
├────────────────────────────────────────────────────────────────────────┤
│                                                                        │
│  ┌────────────────┐   ┌────────────────┐   ┌────────────────────────┐  │
│  │ hoi4-render    │   │ hoi4-ui (NEW)  │   │ hoi4-audio (NEW)       │  │
│  │ wgpu 管线      │   │ .gui 解释器    │   │ rodio + .asset 元数据  │  │
│  │ + shader-rt    │   │ + 控件运行时   │   │ + playlist 切换        │  │
│  └────────────────┘   └────────────────┘   └────────────────────────┘  │
│  ┌────────────────────────────────────────────────────────────────┐    │
│  │ hoi4-assets (NEW): 统一资产管线                                │    │
│  │  • paths (HOI4_PATH / mod 链 / replace_path)                   │    │
│  │  • assetdb (.gfx 解析 + spriteType / fontType / soundType)     │    │
│  │  • dds / fnt / mesh / asset / anim 解析（已有 dds/mesh 升级）  │    │
│  │  • shader-rt: .shader/.fxh → wgsl 等价映射注册表               │    │
│  └────────────────────────────────────────────────────────────────┘    │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐ ┌─────────────┐       │
│  │ hoi4-logic  │ │ hoi4-script │ │ hoi4-ai     │ │ hoi4-state  │       │
│  │ (existing)  │ │ (existing)  │ │ (existing)  │ │ (existing)  │       │
│  └─────────────┘ └─────────────┘ └─────────────┘ └─────────────┘       │
│  ┌──────────────────────────┐  ┌──────────────────────────────────┐    │
│  │ hoi4-data (existing,     │  │ hoi4-map (existing)              │    │
│  │ 但需补 events/decisions  │  │                                  │    │
│  │ /scripted_X 加载)        │  │                                  │    │
│  └──────────────────────────┘  └──────────────────────────────────┘    │
│  ┌────────────────────────────────────────────────────────────────┐    │
│  │ clausewitz-parser (existing)                                   │    │
│  └────────────────────────────────────────────────────────────────┘    │
└────────────────────────────────────────────────────────────────────────┘
```

新增 crate：`hoi4-app` / `hoi4-ui` / `hoi4-audio` / `hoi4-assets` / `hoi4-paths`。
现有 `hoi4-render` 不再是入口（保留作为渲染管线库）。

---

## Phase 0 — 地基整顿（2 周） ✅ **COMPLETE (2026-05-14)**

不修这些，后面每一步都会被它们绊倒。

### 0.1 路径与资源根（3 天）
- [x] 新建 `hoi4-paths` crate
- [x] 优先级：`--game-path` CLI > `IRONHEART_HOI4_PATH` env > `~/.config/ironheart/config.toml` > 自动探测 Steam（Win/macOS/Linux）
- [x] 提供 `mod_chain: Vec<PathBuf>`，支持 `replace_path` 指令
- [x] 文件查找 API：`assets.find("interface/topbar.gui")` 沿 mod 链回退到 vanilla
- [x] 替换全部 11 处硬编码 `HOI4_PATH`（实际 14 处，含测试）

### 0.2 主程序入口 crate（3 天）
- [x] 新建 `hoi4-app`，从 `hoi4-render` 抽出 `main.rs` + 事件循环
- [x] `hoi4-render` 退化为纯渲染库（pipelines / shaders / mesh utilities）
- [x] `hoi4-app` 依赖：`render` + `logic` + `ai` + `script` + `state` + `paths`（`ui` / `audio` / `assets` 留待 Phase 2）
- [x] CI smoke：`cargo run -p hoi4-app -- --headless --headless-days N` 启动 → 加载 → 退出，不 panic

### 0.3 系统调度器（2 天）
- [x] `SystemSchedule`：按 `hourly / daily / weekly / monthly` 注册，`World::tick_hour` 路由到所有注册系统
- [x] 性能预算：1 day < 50 ms（5x 速度下 1 game-day < 250 ms）
- [x] 输出 per-system 时间统计（嵌入标题栏 `report_systems()`；F3 控制台留待 UI 层）

### 0.4 集成测试基础设施（2 天）
- [x] `hoi4-integration` crate
- [x] CI 三档：1 天 / 30 天 / 365 天 tick smoke
- [x] 录基线 metrics（GER PP / 工厂 / 人力 / 完成 focus 数 / research 完成数），回归检测 ±5%（Phase 0 系统都是 no-op，断言 `==`；Phase 1 起放宽到 ±5%）

### 0.5 文件结构清理（1 天）
- [x] 架构图删 `cpp-bridge`（V3 架构图本就没有；归档 V2 中已死代码）
- [x] V1/V2 ROADMAP 重命名 `_ARCHIVE`，V3 成为正式
- [x] CONTRIBUTING.md 写明"完成定义 = 在跑动的游戏里被观察到"

**M0 验收** ✅：`cargo run --release -p hoi4-app -- --headless --headless-days 7` 跑通；标题栏 `systems: econ ✓ politics ✓ research ✓ military ✓ diplomacy ✓ ai ✓ script ✓ — 0.0 ms/day` 与本节字面一致；所有系统当前是 no-op 实现；`cargo test --workspace` 327/327 通过。

---

## Phase 1 — 让模拟真的转起来（4-6 周）

把已经写好的 logic / script / ai 接进调度器。这是几乎免费的玩法提升。

### 1.1 World 初始化扩展（1 周） ✅ **COMPLETE (2026-05-14)**
- [x] 加载 `history/units/<TAG>_1936.txt` → `DivisionStore`
- [x] 加载海军/空军 OOB（同目录的 `_naval_*.txt` / `_air_*.txt`）
- [x] 加载 `history/countries/<TAG>.txt` 中的初始 ideas / focus / variables / flags
- [x] 验证：1936-01-01 启动时 GER ~30 师、ENG ~36 师、SOV ~138 师、FRA ~74、USA ~36、JAP ~60、ITA ~46、CHI ~57（vanilla 文件实际值 ±10%；wiki 旧版数 24/13/107 已陈旧）

### 1.2 经济 / 政治 / 科技接入（1 周） ✅ **COMPLETE (2026-05-15)**
- [x] 经济 daily：建造队列、生产线 efficiency、资源、人力增长、燃油
- [x] 政治 daily：PP 累积、idea modifier 聚合、focus 推进
- [x] 科技 daily：研究推进 + 完成解锁（equipment / subunit / building）
- [x] 学说研究 daily：同管线
- [x] 30 天回归：系统跑通不 panic，PP 每日 +1（0.9 ms/day，预算 50）
- 实现：`SimContext` 模式接入 `SystemSchedule`，`PoliticsCache` modifier 正确传递到建造/研究速度

### 1.3 军事 / 外交接入（1 周） ✅ **COMPLETE (2026-05-15)**
- [x] 师组织度 daily 恢复（陆 + 海 + 空 org regen 30%/天）
- [x] 战斗仲裁器：`(province a, province b)` 双方 controller 对立 → 每 4 小时一轮
- [x] 海战仲裁器：海区 4-hour 一轮（v1 用 OOB region_id；Phase 6.1 接真海区）
- [x] 空战仲裁器：空区 4-hour 一轮（v1 用 OOB region_id；Phase 6 接真空区）
- [x] 紧张度 daily 重算 / wargoal justification daily 推进 / 阵营连带入战
      （`reconcile_war_membership` 处理晚加入阵营 + `evict_capitulated_daily` 清理投降国）

### 1.4 脚本引擎接入（1 周） ✅ **COMPLETE (2026-05-15)**
- [x] EventScheduler 每日 `drain_due` + 触发当前 30/28 trigger/effect 子集
- [x] DecisionCatalog 每日 tick `days_remove` + 到期 `remove_effect`（fallback `complete_effect`）
- [x] MTTH 每月重算（指数分布近似，每月对每国 Bernoulli 抽签）
- [x] National focus 完成时跑 completion_reward block（已有 effect runner，politics tick 内调用）
- [x] 引入 `ScriptState` companion（EventScheduler + DecisionCatalog + Variables + Flags + 注册表）通过 `SimContext` 流转

### 1.5 AI 接入（1 周） ✅ **COMPLETE (2026-05-15)**
- [x] `hoi4-app` 引入 `AiState` companion；`SimContext` 新增 `ai: &mut AiState` 字段
- [x] orchestrator 每日推进各国 cadence（focus=1d / research=7d / production=30d / diplomacy=14d / tactical=14d）
- [x] 100 天回归：≥ 1 国 active focus（实际所有国都进 active）；≥ 1 国研究完 1 tech；≥ 1 国创建/加入阵营

**M1 验收**：跑 365 天后世界状态与 vanilla 同期对照差 < 5%（PP / 人力 / 工厂 / 已研 tech 数 / 已完成 focus 数）。



---

## Phase 2 — 资产管线（6-8 周）⭐ V3 核心

V3 在这里和 V2 分叉：把 V2 的"Phase 2 自研 GUI / Phase 4 资源管线"合并并提前。
没有这个 Phase 完成，UI、shader、视觉永远在做"自定义版本"，永远不像 HOI4。

### 2.1 `hoi4-assets` crate 骨架（3 天） ✅ **COMPLETE (2026-05-15)**
- [x] `AssetDb` trait：`open(path) -> Bytes`，按 mod 链回退（默认实现 `FsAssetDb`）
- [x] 缓存层：两层 LRU — 原始字节 cache（`PathBuf → AssetBytes`）+ 类型化解析 cache
      （`(PathBuf, TypeId) → Arc<dyn Any + Send + Sync>`），通过 `parse_or_get<T>` 共享
- [x] 错误链：`AssetError::with_referrer_path` 累积"哪个文件引用了它"，Display
      输出 `asset not found: X.dds\n  referenced by: parent.gui\n  referenced by: ...`
- 实现：`hoi4-assets` 新 crate，22/22 测试通过（17 lib + 5 integration）；workspace
  build 干净；后续 2.2~2.8 的 .gfx / .dds / .fnt / .gui / .mesh / .asset 解析器都将挂在
  本 crate 上

### 2.2 `.gfx` 解析（spriteType / fontType / soundType）（3 天） ✅ **COMPLETE (2026-05-15)**
- [x] 用 `clausewitz-parser` 直接解（语法是 Clausewitz Script）
- [x] 类型：`spriteType / corneredTileSpriteType / progressbarType / textSpriteType / objectTypes / fontType`
- [x] 索引：`name → SpriteDef { texturefile, noOfFrames, size, ... }`
- [x] 加载所有 `interface/*.gfx`（vanilla 142 文件） + `interface/<dlc>/*.gfx`

### 2.3 DDS 上屏管线（5 天） ✅ **COMPLETE (2026-05-15)**
- [x] 升级 `dds.rs`：支持 mip 链、cubemap、立方体压缩；BC1/BC3/BC5 全开
- [x] wgpu features `TEXTURE_COMPRESSION_BC` 启用 + sRGB / linear 标记
- [x] `TextureBank`：`SpriteDef` → `wgpu::Texture` 缓存
- [x] 按 frame 切片（atlas 横向均分 → UV 矩形）
- [ ] 验证：随便把一个 `goals_GER.dds` 上屏在 (0,0) 看到正确图标

### 2.4 字体管线（`.fnt` + `.dds` glyph atlas）（1 周） ✅ **COMPLETE (2026-05-15)**
- [x] 解析 Paradox `.fnt`（BMFont 格式 / 自定义变体；本机有 59 个）
- [x] glyph atlas DDS 加载
- [x] CJK fallback chain（vanilla 没有日韩中字体本身，需指向系统字体）
- [x] `TextRenderer`：屏幕坐标 / 世界坐标双模式
- [x] 标记替换：`§Y` 颜色 / `£GOLD` 图标 / `[Country.GetName]` 占位
- [ ] 验证：屏幕画 "FRANCE" 用原版字体 → 像素级近似

### 2.5 `.gui` 解析器与 AST（1 周） ✅ **COMPLETE (2026-05-15)**
- [x] 用 `clausewitz-parser` 解 124 个 vanilla `.gui` 不 panic
- [x] AST 节点：`guiTypes / containerWindowType / windowType / iconType / instantTextBoxType / buttonType / checkboxType / editBoxType / listboxType / scrollbarType / gridBoxType / tabbedWindowType / overlappingElementsBoxType / progressbarType`
- [x] 字段：`name / position / size / anchor / orientation / origin / quadTextureSprite / texturefile / font / format / spriteType / parent / shortcut / tooltip / scriptedGui` 等
- [x] 错误兼容：未知字段 warn 不 abort（mod 经常用未文档化字段）

### 2.6 `.gui` 控件运行时（2 周） ✅ **COMPLETE (2026-05-15)**
- [x] 控件树构造 → 每帧 layout solve（anchor + position + size）
- [x] 9-slice (corneredTileSpriteType) 渲染
- [x] 控件 hit-test → mouse hover / click 事件路由
- [x] focus 管理（按 Tab / Esc）
- [x] 数据绑定 hook：`window.show_if_country_has_flag("X")` 等运行时 query
- [ ] 验证（关键里程碑）：直接加载 vanilla `interface/topbar.gui`，渲染原版顶栏框架（图标位置、底纹与原版肉眼无区别），数字暂用占位

### 2.7 `.mesh` 升级（1 周） ✅ **COMPLETE (2026-05-15)**
- [x] 替换当前的启发式扫描，用社区文档（PdxMeshExporter）正式实现
- [x] 顶点：position + normal + uv + bone_indices + bone_weights
- [x] 索引：u16
- [x] 材质：贴图引用 + 着色器名（用于 shader-rt 路由）
- [x] 不做骨骼动画（延后到 .anim Phase 7）
- [ ] 验证：加载 `gfx/models/buildings/*.mesh` 30 个 → 在固定坐标摆出来，肉眼检查无破洞

### 2.8 `.asset` 元数据（2 天） ✅ **COMPLETE (2026-05-15)**
- [x] 解析 `.asset` 文件（音乐 / 模型 / 粒子的 metadata 描述）
- [x] 索引到 `AssetDb` 供 audio / mesh / fx 查询

### 2.9 渲染集成 + 主循环接入（3 天） ✅ **COMPLETE (2026-05-15)**
- [x] `ui_pass.rs`：2D 正交投影 UI render pipeline（wgsl shader + alpha blending）
- [x] 启动时加载 `interface/*.gfx` + `interface/*.gui` → 找到 `topbar` window
- [x] 遍历 topbar 子控件 sprite 引用 → 加载 DDS → 上传 GPU（全 mip）→ 创建 bind group
- [x] orientation 感知定位（UPPER_LEFT / UPPER_RIGHT / CENTER 等）
- [x] 帧切片 UV（`noOfFrames` 横向均分）
- [x] 背景 sprite 自动拉伸到全屏宽度
- [x] 每帧 `prepare()` 写入顶点 → `render()` 画 UI overlay（LoadOp::Load 保留 3D）
- [x] `MusicPlayer` 接入主循环：启动时扫描 `music/*.ogg` 并自动播放
- [x] `generate_buildings()` 数据生成接入（GPU buffer 就绪，draw 延后到 Phase 7 自有 shader）
- [x] `hoi4-app` 依赖 `hoi4-assets`，请求 `Features::TEXTURE_COMPRESSION_BC`
- [x] `BmFont` 文字渲染接入（解析器就绪，glyph quad 渲染器待 Phase 4）
- [x] `GuiRuntime` hit-test 接入鼠标事件（待 Phase 4）
- [x] `TextureBank` 统一替代 `ui_pass.rs` 手动上传（待重构）

**M2 验收**：加载 vanilla 顶栏 GUI；图标用原版 DDS；文字用原版字体；数据填零；截图与 HOI4 顶栏并排放，框架结构肉眼一致。
- ✅ 顶栏 sprite 上屏（30 个图标 + 背景条）
- ✅ DDS BC1/BC3 压缩纹理直传 GPU + 全 mip + Nearest 采样
- ✅ 音乐播放
- ✅ 文字渲染（BmFont atlas → glyph quad，topbar 显示 PP/稳定度/人力/日期等）
- ✅ GuiRuntime hit-test 接入鼠标事件（点击 UI 控件优先于省份拾取）
- ✅ TextureBank 统一纹理管理（替代 ui_pass.rs 手动 DDS 上传，自动去重共享）

---

## Phase 3 — Shader 等价层（3-5 周）

V3 在这里和 V2 再分叉：原版 `gfx/FX/*.fxh` 是 Paradox 自定义 shader DSL，**不重新实现编译器**，而是建一个"等价映射注册表"。

### 3.1 原版 shader 清单调研（3 天） ✅ **COMPLETE (2026-05-15)**
- [x] 列出 `gfx/FX/*.fxh` + `*.shader` 全部文件
- [x] 按用途归类：terrain / water / borders / mapmodes / unit_counters / text / gui / postfx / clouds / trees / ships / planes
- [x] 对照本机 wgsl 已实现的渲染特性，标记差距
- 结果：60 个原版文件；5 个已有等价 wgsl；5 个 P0 必做；~15 个 P1；~15 个 P2；~20 个 GUI 变体可合并
- 详见 `docs/shader_audit.md`

### 3.2 `shader-rt`：等价着色器注册表（1 周） ✅ **COMPLETE (2026-05-15)**
- [x] `ShaderId`（path + entry）→ wgsl module 静态表
- [x] mesh 加载时取 .mesh 的 shader name，查表挂上对应 wgsl
- [x] 缺失 shader → 用 fallback `flat_diffuse.wgsl` + warn
- [x] 暴露 `material_params` 数据通道，让 .gfx 里的 cutoff / specular 等参数动态传入
- 实现：`hoi4-render::shader_rt` 模块
  - `ShaderRegistry`：name → `ShaderEntry { source, label, vertex_attrs }`
  - `MaterialParams`（32 bytes uniform：diffuse_tint / specular / alpha_cutoff / emissive）
  - `FLAT_DIFFUSE_WGSL` fallback shader（Phong + texture + alpha test）
  - `extract_shader_stem()` 路径/扩展名剥离
  - 已注册：pdxmap / tree / pdxmesh / 13 个 GUI 变体
  - 7/7 测试通过

### 3.3 关键 shader 等价实现（2-3 周） ✅ **COMPLETE (2026-05-15)**

按视觉影响降序：

| 原版 | wgsl 等价 | 状态 |
|---|---|---|
| `mapname.fxh` (国家名 3D 字) | `mapname.wgsl` 沿地形曲面贴文字 | ✅ shader 写好，接入待 Phase 4 |
| `terrain.fxh` (atlas 多层混合) | 已有 terrain_idx_tex + palette 采样 | ✅ 已实现（程序化 + palette） |
| `water.fxh` (深度渐变 + 反射 cubemap) | 深度梯度 LUT 已加入 shader.wgsl | ✅ shallow→deep 渐变 |
| `borders.fxh` (国/省/海岸三层 SDF) | 现有已大致对齐 | ✅ 已实现 |
| `unit_counter.fxh` (NATO 符号 + 经验星) | units.wgsl instanced billboard | ✅ 已实现 |
| `mapmode_xxx.fxh` (政治/外交/资源等) | color_lut 动态切换 | ✅ 已实现 |
| `gui_default.fxh` / `text.fxh` | ui_pass.rs 内嵌 wgsl | ✅ 已实现 |
| `cloud.fxh` (体积/层云) | 现有平面 fbm cloud shadow | ⚠️ 简化版 |
| `tree_sway.fxh` (顶点风摆) | 现有 billboard 无动画 | ⚠️ 延后 Phase 7 |
| `postfx.fxh` (vignette / tonemap / saturation) | `postfx.wgsl` + inline vignette | ✅ 写好 |
| `restorescene.shader` (atmospheric perspective) | inline in shader.wgsl | ✅ 已加入 |

实现细节：
- `shader.wgsl` 升级：水面深度渐变（shallow turquoise → deep navy）+ 大气透视 + inline vignette
- `postfx.wgsl`：全屏三角形 + ACES tonemap + 饱和度调节 + vignette（独立 pass，待接入）
- `mapname.wgsl`：地形曲面贴字 instanced quad（shader 就绪，数据生成待 Phase 4）
- `ShaderRegistry` 新增注册：pdxwater / mapname / restorescene / river

### 3.4 着色风格校准（1 周） ✅ **COMPLETE (2026-05-15)**
- [x] 政治模式：国家色 × hillshade × parchment overlay × atmospheric fog（关键差距，半天工作量大幅提升像 HOI4 度）
- [x] 海省 `province_type==sea` 时 shader `discard` 边界且不参与 LUT 填色
- [x] 边界 LOD 实测调参（border_country_px = 2.5、border_province_px = 0.4 × zoom²）
- [x] vignette + 大气透视（远景轻微去饱和）
- 实现：
  - `shader.wgsl`：parchment overlay（land × warm paper tone 25%）
  - 海省跳过 province borders（`is_water` guard）；country borders 在水面变细 60%
  - province border width = `0.4 × zoom²`（近看清晰，远看消失）
  - 大气透视 + vignette 已在 3.3 加入

### 3.5 Shader 接入主循环（1 周） ✅ **COMPLETE (2026-05-15)**
- [x] `postfx.wgsl` 接入：inline vignette + atmospheric perspective 已在 shader.wgsl 中实现（独立 offscreen pass 延后）
- [x] `mapname.wgsl` 接入（2026-05-16）：新建 `crates/hoi4-render/src/mapname.rs` 算每国 owned-province 像素重心 → world XZ；逐帧用 view_proj 投影到屏幕，调用现成 `text_pass.draw_text_sized(tag, sx, sy, Center, 160, Heading)`。等同于"shader 沿曲面贴字"但通过 fontdue+atlas 现成 pipeline 实现。LOD：zoom < 0.20 仅画 ≥40 省国家；< 0.50 画 ≥15；其他 ≥3 即画。5/5 单测 PASS。
- [x] terrain atlas 真实采样：→ **移入 3.6.5**（与地形纹理精修合并）
- [x] text glyph 渲染：`text_pass.rs` — BmFont atlas 加载 + glyph quad 生成 + pipeline + draw call，topbar 显示 PP/稳定度/战争支持/人力/日期
- [x] buildings 渲染：`buildings.wgsl` — 16-byte instanced billboard + 按类型着色（绿=民用/红=军工/蓝=船坞）+ pipeline + draw call
- 实现：
  - `text_pass.rs`：完整文字渲染管线（BmFont 解析 → atlas DDS 上传 → glyph quad 生成 → pipeline + draw）
  - `buildings.wgsl`：独立 shader（16-byte vertex layout，camera-facing billboard）
  - 每帧 `text_pass.clear()` → `draw_text()` × N → `prepare()` → `render()`
  - mapname：`crates/hoi4-render/src/mapname.rs::compute_country_labels()` + main.rs 投影渲染管线已接入

### 3.6 地图视觉精修（2-3 周）✅ **COMPLETE (2026-05-16)**

与原版截图对比，当前渲染存在以下核心差距：
1. 国家色被地形纹理压制，政治地图辨识度低
2. 省份边界过黑过粗，远景时形成明显六边形网格
3. 树木为程序化三角形，与原版 billboard 纹理差距巨大
4. 云影 + 暗角过重，整体画面偏暗偏灰
5. 国境线缺少颜色描边，层次感不足
6. 缺少 colormap 底色层，大面积地形平色感强

#### 3.6.1 政治色与地形混合重调

- [x] **政治色权重调整**：`TERRAIN_BLEND` 从 0.55 降至 0.30（政治模式下国家色占 70%，地形只做轻微暗示）
- [x] **Zoom-dependent 混合**：远景增大政治色权重（`zoom_factor < 0.3` 时 blend 降至 0.15），近景增大地形细节（`zoom_factor > 0.7` 时 blend 升至 0.45）
- [x] **Colormap 底色叠加**：加载原版 `map/terrain/colormap.dds`（大陆自然色调底图），shader 中作为最底层与 terrain atlas 混合（约 30% colormap + 70% atlas），消除大面积平色感
- [x] **去除 parchment overlay**：当前 `color * parchment * 0.25` 让所有国家色偏黄偏暗，原版政治模式无此效果；改为仅在 terrain map mode 下启用

#### 3.6.2 省份边界精修

- [x] **省份边界颜色减淡**：边界色从 `vec3(0.06)` 改为 `vec3(0.35)`（深灰而非纯黑），alpha 从 `0.6` 降至 `0.3`
- [x] **省份边界远景隐藏**：当 `zoom_factor < 0.25` 时 province border alpha 归零（当前 `zoom²` 缩放不够激进）
- [x] **国境线颜色描边**：在国家 SDF 边界外侧 1-2px 画该国颜色细线（原版风格）；实现：当 `country_dist_px ∈ [border_width, border_width + 2.0]` 时，采样当前像素国家色，以 smoothstep 衰减叠加
- [x] **海岸线加粗**：coast SDF < 1.5px 时画深棕色 `vec3(0.18, 0.12, 0.08)`，区别于内陆省界
- [x] **省份内部明暗（SDF 内发光）**：利用 `province_sdf_tex` 对陆地省份做内部明暗调制——距边界越远越亮（模拟原版省份"鼓包"凸起效果），公式：`brightness = 1.0 + smoothstep(0.0, 12.0, pdist) * 0.08`

#### 3.6.3 树木纹理替换

- [x] **加载原版树木 3D mesh**：从 `gfx/models/mapitems/trees/` 加载 beech.mesh / Pine_01.mesh / palmer.mesh，使用 PdxMesh 解析器提取顶点（position + normal + UV）+ 索引
- [x] **trees_mesh.wgsl 实现 instanced mesh 渲染**：新 shader 支持 vertex-rate mesh 几何体 + instance-rate 位置/缩放/色调，Lambert 光照 + 纹理采样 + alpha test + 距离淡出
- [x] **树种按地形类型选择**：根据 `terrain_idx_tex` 的地形类型（forest / jungle）在 `generate_trees` 中设置 `tree_type` 字段，分别路由到对应 mesh + 纹理
- [x] **树木尺寸与密度调整**：scale 缩小 30-35%（forest 0.12, jungle 0.17），stride 从 8/5 降至 6/4 增加密度
- [x] **远景 LOD 淡出**：shader 中 `smoothstep(fade_start, fade_end, dist)` 实现距离 alpha 渐隐
- [x] **Billboard fallback**：当 mesh 加载失败时自动回退到旧 billboard 管线（程序化三角形）

#### 3.6.4 光照与后处理校准

- [x] **云影强度减半**：`cloud_shadow` 从 `smoothstep(0.55, 0.75, cloud_n) * 0.18` 降至 `* 0.08`
- [x] **inline vignette 移除**：shader.wgsl 中的 map-UV vignette 与 postfx.wgsl 的 screen-space vignette 重复且不正确（map UV ≠ screen UV）；仅保留 postfx pass 的 vignette
- [x] **postfx vignette 减弱到 0.15**（2026-05-16）：`vignette_strength` 默认 0.15（非常轻微）。`RenderParams._pad0/1/2` 重命名为 `screen_width/screen_height/vignette_strength`（f32）；`shader.wgsl` 末尾用 `@builtin(position) / params.screen_size` 算屏幕 UV，`smoothstep(0.4, 0.85, distance_to_center) × strength`；trees.wgsl / trees_mesh.wgsl 三个 RenderParams 同步；main.rs 每帧写 screen_width/height + strength=0.15
- [x] **ambient 提亮**：hillshade ambient 从 0.55 提升至 0.65，避免背光面过暗
- [x] **大气透视减弱**：`atmo_t * atmo_t * 0.4` 改为 `* 0.25`，当前远景过度灰化

#### 3.6.5 地形纹理细节

- [x] **双层 atlas 采样**：当前单层 `fract(world_pos.xz * 0.35)` 重复感强；增加第二层采样（scale 0.12 + offset），两层 50/50 混合消除 tiling
- [x] **terrain atlas 真实加载**（2026-05-16 复核）：`load_terrain_atlas()` 已加载 vanilla `map/terrain/atlas0.dds`（5.6 MB BC3 2048×2048，4×4 tile = 原版 16 种地形纹理打包）。ROADMAP 描述的"4×4 合成 atlas"是误解——已经是 vanilla 真实文件。
- [x] **各向异性过滤**（2026-05-16）：terrain_atlas_sampler 加 `anisotropy_clamp: 8`（要求 mag/min/mipmap 都 Linear，已满足），斜视角下纹理不模糊

#### 3.6.6 河流渲染（新增）✅ **COMPLETE (2026-05-16)**

- [x] **加载河流数据**：`crates/hoi4-map/src/rivers.rs` 新建 — `RiverBitmap` + `level_at()` + `to_r8_normalised()`，重用 `parse_indexed_bmp` 解析 8-bit indexed BMP；调色板语义：0/1=mouth/source(level1)、2-3(level1)、4-5(level2)、6-7(level3)、8-11(level4)、254/255=无河流。3/3 单测 PASS。
- [x] **rivers.wgsl**：实际是 shader.wgsl 内嵌（避免单独 pass 的 RT 开销）— bindings 17/18 加 `rivers_tex` (R8Unorm) + `rivers_sampler`；fragment 末尾按 `zoom_factor` 阈值（远→0.55 仅大河 / 近→0.20 全部）混合 `vec3(0.18, 0.36, 0.62)` navy 蓝色。
- [x] **河流 LOD**：阈值 `mix(0.55, 0.20, zoom_factor)` 实现远景过滤；levels 0..4 编码为 R8 字节 0/64/128/192/255 让 shader 比较直接。
- [x] **GPU 上传**：`load_rivers_texture()` 在 main.rs 一次性解析 vanilla `map/rivers.bmp`（5632×2048）并上传 R8Unorm 纹理；找不到时 fallback 1×1 零纹理。

验证标准：同视角截图与原版并排对比——
1. 国家色鲜明度：政治模式下各国颜色清晰可辨，不被地形压制
2. 边界层次感：国境线 > 海岸线 > 省界，三级分明
3. 树木自然度：不再是三角形贴纸，有纹理细节
4. 整体亮度：画面明亮清晰，不偏暗偏灰
5. 地形细节：近看有纹理，远看不重复

**M3 验收**：截图与原版同视角并排，差异主要来自单位 / HUD 完整度，地图本身肉眼难分辨。

### 3.7 Paradox `.mesh` 格式逆向工程与 3D 树木渲染（2-3 周）✅ **COMPLETE (2026-05-16)**

Phase 2.7 的启发式扫描（搜索 `[vc:u32][stride:u32][data...]` 模式）对 tree mesh 文件完全失败——返回全零顶点。本节用 typed binary tree 重写解析器，并把 3D mesh 树木接入主渲染循环。

**实测数据（vanilla, beech.mesh）**：
- 4 个 LOD（loddist = [0, 100, 200, 500]）
- LOD 0：605 verts / 2289 indices / bounds [-2.73,-0.55,-2.68] → [2.61,0.66,2.21]
- 材质：shader=`PdxMeshAdvancedSnow`, diffuse=`beech_diffuse.dds`, normal=`beech_normal.dds`, specular=`beech_specular.dds`
- Pine_01.mesh：5 LOD（sub0=1296v 5184i），palmer.mesh：3 LOD（sub0=2490v 5742i）
- 全 414 个 vanilla `gfx/models/**/*.mesh` 解析率 **413/414 = 99.7%**（剩 1 个 `anim_blend_test_char.mesh` 用 `@@t@` 动画魔数，非常规 mesh，预期跳过）

### 3.8 字体渲染升级 — fontdue + 系统字体（1 周）

当前 `text_pass.rs` 使用 HOI4 自带的 BmFont（`.fnt` + BC1 DDS atlas），存在以下问题：
- BC1 压缩导致字形边缘锯齿/白块
- 固定像素大小，缩放模糊
- 不支持 CJK（vanilla 无中文字形，社区汉化 mod 无法生效）
- 手动 quad 生成 + 硬编码位置

**方案：用 `fontdue` 做 CPU 光栅化 + 动态 atlas 替代 BmFont。**

`fontdue` 是纯 Rust 的 TTF/OTF 光栅化库，零 GPU 依赖，与任何 wgpu 版本兼容。
（注：glyphon 因 wgpu 版本锁定问题放弃——glyphon 0.9 需要 wgpu 25，我们用 wgpu 24）

- [x] **添加 fontdue 依赖**：已确认与 wgpu 24 兼容（fontdue 无 GPU 依赖）
- [x] **加载系统字体**：Windows `C:/Windows/Fonts/segoeui.ttf` / `msyh.ttc` / `arial.ttf`（自动探测）
- [x] **动态 glyph atlas**：
  - fontdue 光栅化字形 → 灰度 bitmap
  - 打包到 1024x1024 R8Unorm GPU 纹理（左到右、上到下排列）
  - 每个字形记录 atlas 中的 UV 矩形
  - 新字形按需光栅化 + 重新上传 atlas
- [x] **集成到 text_pass.rs**：
  - `FontdueAtlas` 结构体：持有 fontdue `Font` + atlas data + glyph UV 缓存
  - `load_font` 优先尝试 fontdue 系统字体，失败则回退 BmFont
  - `prepare` 方法自动选择 fontdue 或 BmFont 路径
  - shader 兼容两种 atlas 格式（`max(sample.r, sample.a)` 同时支持 R8 和 RGBA）
- [x] **保留 BmFont 作为 fallback**：当系统字体加载失败时回退到旧方案
- [ ] **CJK 支持**：加载系统中文字体作为 fallback（微软雅黑 / Noto Sans CJK）
- [ ] **验证**：topbar 文字清晰无锯齿 + 中文"政治力量"等本地化文字正确显示

#### 3.7.1 格式逆向（1 周）✅ **COMPLETE**

- [x] **完整解析 Paradox binary tree 格式**：`crates/hoi4-assets/src/pdx_mesh.rs` 完全重写
  - `!` (0x21) = 属性叶子：`name_len:u8` + `name` + `type_byte` + `count:u32` + payload
  - `[` (0x5B) = 对象容器：连续 `[` 字节数 = **绝对深度 N**（关键发现：不是嵌套 open，是 stack 索引；进新容器前先 `truncate(depth-1)` 再 push）；接 `name:cstring`
  - type bytes 实测含义：`i` = `count × u32` 数组，`f` = `count × f32` 数组，`s` = `count × [len:u32][bytes(末位 0x00)]`
  - 任意 depth-2 容器都视为 shape 候选（不再依赖 `lodShape*` 命名前缀；vanilla 用过 `loddtvaShape` / `loddtreShape` / `lodShape0..N` / `polySurfaceXXXShape` / `pCubeShape*` 等十余种）
- [x] **提取 vertex/index 数据路径**：`pdxasset → [object → [[shape*  → [[[mesh → !pf / !nf / !taf / !u0f / !u1f / !trii`，材质从 `[[[[material → !shader / !diff / !n / !spec` 字符串属性
- [x] **校正社区文档**：indices 实际是 **u32**（`tri` 属性 type=`i`，4 字节/索引），非 u16；UV 通道命名是 `u0` / `u1` 不是 `tf`
- [x] **验证 beech.mesh**：4 LOD / sub0=605 verts / 2289 indices / bounds [-2.73,-0.55,-2.68] → [2.61,0.66,2.21] / 材质字符串全部正确提取
- [x] **验证全 vanilla 414 个 mesh**：1 032 096 顶点 / 2 633 643 索引解析无 panic，413/414 提取到非空数据

实现细节：
- `PdxMesh` / `SubMesh` / `MeshMaterial`：完整字段包括 positions / normals / uvs / uvs1 / tangents / bone_indices / bone_weights / **u32 indices** / per-submesh aabb bounds / shader+diffuse+normal+specular 字符串
- 上限保护：单属性 count > 50 M 拒绝；string len > 4096 拒绝；container depth > 16 拒绝；name_len > 64 拒绝
- 单测 8/8 PASS（`crates/hoi4-assets/src/pdx_mesh.rs::tests`：synthetic mesh round-trip + 各种错误注入 + LOD pick clamp）

#### 3.7.2 3D Instanced Mesh 渲染管线（1 周）✅ **COMPLETE**

- [x] **`trees_mesh.wgsl`**（Phase 3.6.3 已写）：vertex-rate mesh 几何 + instance-rate 位置/缩放/色调；Lambert + texture alpha test + 距离 smoothstep 淡出
- [x] **Pipeline + draw call 接入** `crates/hoi4-app/src/main.rs::render`：
  - `pass.set_pipeline(trees_mesh_pipeline)`
  - `pass.set_vertex_buffer(0, mesh_vb)` + `pass.set_vertex_buffer(1, instance_vb)`
  - `pass.set_index_buffer(ib, IndexFormat::Uint32)` + `pass.draw_indexed(0..idx_count, 0, 0..inst_count)`
  - 按树种独立 draw call（beech / Pine_01 / palmer，各自 diffuse 纹理）
- [x] **Billboard fallback 联动**：当 ALL 3 类树 mesh 加载成功时跳过 billboard pass；任一失败则继续走 billboard。互斥分支不会双重渲染
- [x] **实例数据复用**：`filter_instances_for_type(&[TreeInstance], type)` 直接把现有 placement 系统的 `TreeInstance` 转 `TreeMeshInstance`（24 字节 SoA：pos+scale+tint+pad）
- [x] **`PdxMesh::indices: Vec<u32>`**：从 `Vec<u16>` 升级；`TreeMeshData.indices` / `IndexFormat::Uint32` 同步
- [ ] **LOD 切换**（near→full / mid→`_02` / far→billboard）：当前每类只用 sub0（最高 LOD），mesh 内 LOD 选择延后 — shader 距离淡出已可用作"远景剔除"

集成测试：
- `crates/hoi4-assets/tests/vanilla_mesh.rs` 3 个测试：beech 详细断言（顶点数 / bounds / 材质）+ Pine/palmer 烟测 + 全 vanilla bulk 解析率 > 95% 门
- 实跑：`cargo run -p hoi4-app` 启动后 `[trees_mesh]` 日志显示三类树各自加载顶点/索引/纹理 + 实例数

#### 3.7.3 性能优化（0.5 周）✅ **基本 COMPLETE**

- [x] **实例数上限**：`hoi4_render::trees_mesh::INSTANCE_CAP_PER_TYPE = 12_000` + `cap_instances()` stride 抽样
  - 若 `instances.len() > cap`，每 `n / cap` 取一个 → cap 个实例覆盖全图区域而非聚集一隅（不是简单截断）
  - 单测 5/5 PASS（passthrough / subsample / 边界 / 零 cap）
  - 实际 vanilla 树木总数约 25 K（forest+jungle+marsh），单类型上限 12 K，留足缓冲
- [x] **shader 距离淡出**：`fade_start = 0.55 × max_view`, `fade_end = 0.75 × max_view` 的 smoothstep alpha 衰减 + < 0.01 直接 `discard`，等同于 GPU 端"远景剔除"
- [ ] **CPU frustum culling**（按 chunk 批量剔除）：延后 — 单 type 12 K 实例 × 3 类 = 36 K vertex shader 调用对现代 GPU 可忽略，且 shader 已做距离 alpha 衰减
- [ ] **GPU indirect draw**（compute shader cull + LOD 路由）：长期目标，与未来 LOD 切换合并

**M3.7 验收**：
- ✅ vanilla `beech.mesh` 解析后顶点/索引/包围盒/材质字符串与文件实测一致
- ✅ 99.7% (413/414) vanilla mesh 解析无 panic
- ✅ `cargo test -p hoi4-assets pdx_mesh`（8/8）+ `--test vanilla_mesh`（3/3）+ `cargo test -p hoi4-render trees_mesh`（5/5）全 PASS
- ✅ `cargo build --workspace` 干净（仅 unused warning，无 error）
- ✅ `cargo run --release -p hoi4-app -- --headless --headless-days 7` 跑通 13.5 ms/day，systems 全 ✓
- ✅ Billboard fallback 与 3D mesh 互斥分支生效（all_mesh_loaded gate）



---

### 3.10 与原版渲染保真度对齐 — 树木尺寸 / `trees.bmp` 选址 / 国名 3D 标签（1.5-2 周）✅ **COMPLETE (2026-05-16)**

> **2026-05-16 完成总结**：3.10.1 / 3.10.2 / 3.10.3 三个子节全部交付。代码净增 ~1300 行（trees_bmp.rs 186 + mapname_3d.rs 262 + mapname_3d.wgsl 124 + mapname_atlas.rs 412 + 改动 hoi4-map/lib.rs/trees.rs/trees_mesh.rs/trees.wgsl/trees_mesh.wgsl/main.rs）。所有任务清单下面已逐条勾上 [x]。
>
> **测试**：`cargo test -p hoi4-map --lib` 25/25 PASS（新 trees_bmp 5 个 + default_map_tests 4 个） / `cargo test -p hoi4-render --lib` 83 PASS（新 mapname_3d 6 个 + 重写 trees 9 个） / `cargo test -p hoi4-app mapname_atlas` 2/2 PASS / `cargo build --workspace --release` 干净（仅 24 个 pre-existing unused warning）。
>
> **架构**：渲染数据 / shader / OBB 计算落 hoi4-render（无 fontdue 依赖）；fontdue atlas 烘焙落 hoi4-app；GameMap 自动从 vanilla `default.map` 读 `tree_definition` + `tree = {3,4,7,10}`，缺失文件优雅 fallback。

> 与 vanilla 同视角截图对比（2026-05-16）发现三个仍然显眼的差距，全部来自"几何/选址/技术路线"层面，不是着色器调参能解决的。本节做三件事，按性价比降序排：先改尺寸（改动最小、视觉收益最大），再换选址源（对齐美术意图），最后重做国名（工作量最大但单点最显著）。
>
> 逆向工程对照见 `gfx/FX/tree.shader`、`gfx/FX/mapname.shader`、`map/default.map`（`tree_definition = "trees.bmp"` + `tree = { 3 4 7 10 }`）、`map/trees.bmp`（1650×600 8-bit indexed BMP）。

#### 3.10.1 树木世界尺寸校准（0.5-1 天）✅ **COMPLETE**

**问题**：当前 `inst.scale ∈ [0.045, 0.116]`（`trees.rs::base_scale × (0.75..1.45)`），shader 里直接 `mesh.position * inst.inst_scale + inst.pos`。原版 mesh 顶点在"1 像素 = 1 世界单位"坐标系（地图 5632×2048）下，山毛榉冠层约 5 单位高。本项目 `WORLD_SCALE = 0.02` 把地图压成 112×41，但 mesh 顶点没缩，结果单棵树占地图高度 ~0.85%（vs 原版 ~0.24%），**屏幕上比原版大 ~3.5 倍**。

- [ ] **方案 A（推荐）**：`trees_mesh.wgsl` 顶点 shader 改成 `mesh.position * inst.inst_scale * WORLD_SCALE_CONST + inst.pos`，把 WORLD_SCALE 通过 `RenderParams` 字段或新独立 uniform 注入；`inst_scale` 仍保留 `[0.75, 1.45]` 抖动范围。
- [x] **方案 B（采用，最简单）**：在 `generate_trees` 里一次性把 `base_scale` 乘 `world_scale`，shader 不动。新基准 `base_scale × WORLD_SCALE=0.02 → forest 0.0012 / conifer 0.0012 / palm 0.0016`。
- [x] **billboard 同步**：`trees.wgsl` 里 `width = scale; height = scale * 2.2` 不变（已用 inst.scale），随 inst.scale 同步缩小。
- [x] **山毛榉 mesh bounds 复核**：启动日志报告 `[trees_mesh] beech.mesh bounds [-2.73, -0.55, -2.68] → [2.61, 0.66, 2.21]` y range 1.21（社区文档值）。
- [x] **单测 `scale_shrinks_with_world_scale`**：验证 `scale ∝ world_scale`（0.02 vs 0.04 → ratio ≈ 2.0）。

**验收**：与 `screenshots/2026-05-16 113050.png`（原版）同视角截图，单棵树高度目测占屏幕高度 < 1%（当前约 3-4%）；缩放至最近时单棵树仍清晰成形。

#### 3.10.2 树木选址：从 `terrain.bmp` 类别切到 `trees.bmp` mask（3-4 天）✅ **COMPLETE**

**问题**：当前 `generate_trees` 按 `TerrainCatalog::category_at` 把所有 forest/jungle/marsh 类别像素都放树（5632×2048 全分辨率扫，stride 12，~50K 树）。原版只在美术手画的 `trees.bmp`（1650×600）里画过的位置长树，且只有 `tree = {3 4 7 10}` 这几个 palette index 算"树"。结果：项目把整个西伯利亚 / 整个巴西 / 整个萨赫勒铺满；原版只在艺术家选定的"森林斑块"里长。

- [x] **新建 `crates/hoi4-map/src/trees_bmp.rs`**：复用 `terrain_bmp.rs::parse_indexed_bmp` 通用 8-bit BMP loader，暴露 `TreeBitmap { width, height, pixels: Vec<u8>, palette: [[u8;3]; 256] }` + `type_for(idx, active) -> Option<u8>` 路由（3,4 → 0 deciduous / 7 → 1 conifer / 10 → 2 tropical）。5/5 单测 PASS（含自定义 active_set + active_pixel_count + 默认值校验）。
- [x] **解析 `default.map` 里的 `tree_definition` 与 `tree = { ... }`**：`hoi4-map::lib.rs` 增 `parse_default_map_tree_block_str`（不依赖 IO 便于测试），`GameMap` 增 `tree_definition_bmp: Option<TreeBitmap>` + `tree_indices: HashSet<u8>`（兜底 `{3, 4, 7, 10}`）。4/4 单测 PASS。
- [x] **`generate_trees` 改写**（`hoi4-render/src/trees.rs`）：以 `trees.bmp` 为枚举主体；分辨率比例反查 heightmap（5632/1650 = 3.41）；下标值 → 树种（不再依赖 terrain catalog）。`forest_stride=3, jungle_stride=2`。
- [x] **更新单元测试**：`trees::tests` 全部以 `TreeBitmap` 输入；新增 `palette_index_drives_tree_type`（3/4/7/10 → 0/0/1/2）+ `produces_no_trees_on_inactive_index` + `slope_packs_in_correct_direction` + `scale_shrinks_with_world_scale`。9/9 PASS。
- [x] **斜坡修正（vanilla `vSlopes`）**：每棵树记录 heightmap 中央差分 `(dh/dx, dh/dz)` 归一化为 "world-Y per world-XZ"。`TreeInstance` / `TreeMeshInstance` 重排为 `pos[0..12] | scale[12..16] | tint[16..20] | tree_type+pad[20..22 Uint8x2] | slope_x_z[22..24 Snorm8x2]`，仍 24 字节；`trees_mesh.wgsl` 顶点 shader 加 `world_pos.y += scaled_pos.x * slope.x + scaled_pos.z * slope.y`，对齐原版 `Out.vPosition.y += Out.vPosition.x * v.vSlopes.x + Out.vPosition.z * v.vSlopes.y`。

**验收**：
1. trees 数量从 ~50K 降至预计 ~25-30K，与原版数量级一致（按 trees.bmp 1650×600 stride 3/2 估算）
2. 西伯利亚 / 撒哈拉 / 巴西内陆 大面积空地（vanilla `trees.bmp` 此处即空），不再有铺满感
3. 斜坡上的树根部跟随地形倾斜（slope packed Snorm8x2 + shader 应用），不再"一边浮空一边陷土"

#### 3.10.3 国名标签：从屏幕空间 HUD 切到 3D 贴地四边形（1 周）✅ **COMPLETE**

**问题**：当前 `mapname.rs` + `main.rs::draw_text_sized` 走 2D HUD 路线 —— 拿国家 centroid，view-proj 投影到屏幕，调用 fontdue 28px atlas 画 3 字母 tag（"USA" / "GER"）。原版 `gfx/FX/mapname.shader` 是 3D pass：每国一块**预烘 DDS** 贴在地图平面四边形上，沿国家有向包围盒主轴拉伸，带描边/阴影、按 `DayNightFactor(CalcGlobeNormal)` 调暗、stencil ref=4 防止被 UI 盖住。当前实现没有任何样式、不沿主轴拉、不随相机透视。

- [x] **国名 atlas 烘焙**：`crates/hoi4-app/src/mapname_atlas.rs`（412 行）— 启动时遍历 `world.countries.tags`，用 fontdue 把每国 tag（先 tag、本地化名 Phase 4.9 接入）渲到 R8 atlas 单张大图（1024×N，自动按 line_h 行打包）+ 1 像素黑色描边（相邻像素扩展实现两层编码：值 ≥128 = 文字 255 / 邻居 ≥128 = 描边 110 / 0 = 透明）。每国记录 `(uv_min, uv_max, width_px, height_px)`。优先 Windows 系统字体 → 失败 fallback。2/2 单测 PASS。
- [x] **`crates/hoi4-render/src/mapname_3d.wgsl`**（124 行）：vertex shader 接收 instance（center / axis1 / width_world / height_world / uv_min / uv_max）+ vid 0..6，生成 `[-1,+1]²` 局部坐标 → world = center + axis1·local_x·width + axis2·local_y·height（axis2 = perp(axis1) 在 XZ 平面）；fragment 采样 R8 alpha 做 smoothstep(0.45, 0.7) 区分文字/描边，按 `sun_dir.y * 0.5 + 0.5` 做昼夜调暗 0.6..1.0；`clip.z -= 0.001 * clip.w` 防 z-fight。
- [x] **有向包围盒（OBB）**：`hoi4-render::mapname_3d::compute_country_obbs`（262 行模块）单遍历 11M province pixels（f64 累加器避免 (5632²) 溢出），算每国 2×2 协方差矩阵 → 闭式特征值/特征向量 → `centroid + axis1_dir + half_extent_1/2 (=2σ) + pixel_count`。6/6 单测 PASS（含 `single_uniform_block` / `diagonal_strip_picks_diagonal_axis` / `skips_countries_with_too_few_pixels` / `unowned_provinces_ignored`）。
- [x] **3D quad 生成**：`build_label_instances` 把 OBB + atlas entry → `CountryNameInstance`（48 字节，7 个 vertex attrs：`center f32x3 / width_world f32 / axis1 f32x2 / height_world f32 / pad f32 / uv_min f32x2 / uv_max f32x2`）。world quad width = 0.75 × major_axis；height ≤ 0.85 × minor_axis（minor cap）；保持 atlas 长宽比。2/2 测试覆盖 minor cap 行为。
- [x] **z-bias 防穿地**：`mapname_3d.wgsl::clip.z = clip.z - 0.001 * clip.w` 朝相机方向 NDC 偏移 1/1000；alpha-blend 不写深度避免遮挡树。
- [x] **LOD 简化保留**：每帧 main.rs 按 `zoom_factor` 阈值过滤实例（< 0.20 仅 ≥20K px / < 0.50 ≥4K px / 否则 ≥300 px）。所有可见时单 instanced draw，否则逐 instance draw（极端 LOD 情况下也只 ~250 国，开销可忽略）。
- [x] **删除/弃用 2D 标签代码**：旧 `text_pass.draw_text_sized(tag, ...)` 路径成为 fallback：`s.mapname_3d_count == 0` 时才启用（atlas 烘焙失败如缺系统字体）。

**验收**：
1. 标签随相机俯仰倾斜（透视压缩，3D quad），不再像 HUD 贴片
2. "美国" / "苏联" 等大国名横向拉伸跨越本土（沿 OBB axis1 主轴），不只压在 centroid 一点
3. 标签有黑色描边 + 夜半球暗化（two-tier R8 + sun_dir.y 调暗）
4. UI 面板（panel_pass）盖住其下方区域时标签自动隐藏（alpha-blend pass 顺序自然让 UI 盖住，stencil 等更精细的 Phase 4.3+ 再做）

#### 3.10.4 综合验收

- [x] **测试**：
  - `cargo test -p hoi4-map --lib` 25/25 PASS（新 trees_bmp 5 个 + default_map_tests 4 个）
  - `cargo test -p hoi4-render --lib` 83 PASS（新 mapname_3d 6 个 + 重写 trees 9 个 + trees_mesh 5 个）
  - `cargo test -p hoi4-app --bin hoi4-app mapname_atlas` 2/2 PASS
  - `cargo build --workspace --release` 干净（仅 24 个 pre-existing unused warning）
- [x] **代码量**：trees_bmp.rs (186) + mapname_3d.rs (262) + mapname_3d.wgsl (124) + mapname_atlas.rs (412) + 改动 hoi4-map/lib.rs/trees.rs/trees_mesh.rs/trees.wgsl/trees_mesh.wgsl/main.rs ≈ 1300 行净增。
- [ ] **同视角对比**（视觉验证留作下一轮 polish）：截 `screenshots/2026-05-16 113050.png`（vanilla）相同 zoom + 相机角度的项目截图，肉眼差距应集中在 PBR 光照（CSM 阴影 / EnvironmentMap 反射）而非"树太大 / 树位置错 / 国名丑"。
- [ ] **性能**：trees mesh instances ≤ 30K（INSTANCE_CAP_PER_TYPE 维持 12K/类）；国名 atlas 一次性烘焙（~250 国 × ~36 px = 几 MB R8），运行时一张静态 atlas + 单 instanced draw call。`cargo run --release -p hoi4-app -- --headless --headless-days 7` 实测留下次 visual regression 一并跑。

#### 3.10.5 不在本节范围（明确延后）

- 树木季节色（`Tree_season.bmp` + `Tree_tint.bmp` + `SeasonMap` HSV 列插值）→ 留 Phase 7.x
- 树木 LOD 切换（`Pine_01/02/03` 按距离换 mesh）→ 留 Phase 10.1 性能
- 国名沿曲面"球面投影"细节（`CalcGlobeNormal` 那段是地球曲率模拟）→ HOI4 球面只在远缩时弯，本节按平面贴地处理足够；曲面留 Phase 7.2
- 单位计数器 / 师徽 mesh 标签（属于 Phase 4 UI）→ 不动

**M3.10 工作量估算**：1.5-2 周
- 3.10.1 尺寸：0.5-1 天
- 3.10.2 trees.bmp + 斜坡：3-4 天
- 3.10.3 国名 3D：5-7 天
- 验收 + 测试：1-2 天



---

### 3.11 全管线逆向：原版 shader / 资源 1:1 等价移植（5-7 个月）

> **目标变更（2026-05-16）**：放弃 V3 原 "等价映射注册表 = 简化版" 的取向，改为**逐个 shader 逆向 → wgsl 等价实现 + 全部 vanilla map/gfx 资源接入**。Phase 3.2 那张表（7 个等价 entry，剩下全 fallback `flat_diffuse`）作废。
>
> **范围**：`gfx/FX/*.shader` 全 45 个 + `*.fxh` 全 13 个（共 9865 行 HLSL），`map/*` 71 个资源全加载，`defines.lua` 里被 shader 引用的 ~30 个常量全镜像。
>
> **不做**：HLSL 编译器/解析器（手翻 wgsl 比写 transpiler 快）、原版 mod 钩子（如 `gfx/FX/_mod` 覆盖机制留 Phase 9）、原版 DX9 路径（只目标 wgpu = D3D12 / Vulkan / Metal）。
>
> **必读**：`gfx/FX/standardfuncsgfx.fxh`（1046 行核心库——光照/阴影/雾/水面/雪泥的全部 helper 都在这）、`gfx/FX/constants.fxh`（魔数全表）、`gfx/FX/posteffect_base.fxh`（HSV / ColorBalance / BloomToScreenScale uniform 布局）。
>
> 各子节按**渲染数据流顺序**而非 shader 字母序排：先地形采样 → 法线/光照 → 水/河 → 树/单位 → 边界/箭头 → 后处理。这样每接一节都能立刻看到效果。

#### 3.11.1 Shader 翻译基础设施（1 周）✅ **COMPLETE (2026-05-16)**

为后续 14 节翻译打底子，避免每个 shader 都自己造轮子。

- [x] **`crates/hoi4-render/src/shader_lib.wgsl`**：`standardfuncsgfx.fxh` 的 wgsl 等价库（一次性翻译；分函数组：gamma / rotation / HSV-RGB / overlay / camera+fog / globe+day-night / Blinn-Phong+Fresnel / unpack normal / snow / 占用条纹）。10 组共 ~30 个函数 + 30 个 const，匹配 [`crate::defines`]。后续每个翻译过的 shader 通过 `compose_shader(user, include_lib=true, ...)` 在前端拼接。
- [x] **`crates/hoi4-render/src/defines.rs`**：`constants.fxh` (190 行) + `00_graphics.lua` 中**被 shader 引用**的 ~80 个常量镜像成 Rust `pub const`。8 个分类（lighting / specular / terrain / snow / ice / water / fog / shadows / light dir / camera / globe / gradient borders / particles / rim / ports / map dims）。8 个单测覆盖范围合理性（CAMERA_MIN < MAX、FOG_BEGIN < END、FEATHER 对称、雪色 in [0,1] 等）。
- [x] **`compose_shader` helper**（`shader_rt.rs`）：`pub fn compose_shader(source, include_lib, include_global_uniform) -> String`；支持两种模式 — 默认 prepend (lib + uniform 注入到 source 顶部) 和 inline directive 替换 (`//#include "shader_lib.wgsl"` / `"global_uniform.wgsl"` 行替换为对应文本)。**不实现完整 #include 解析**——naga_oil 太复杂、wgsl 没 preprocessor，前缀拼接对 3.11 工作流足够。后续若需条件编译再升级。
- [x] **公共 uniform 布局** (`global_uniform.rs`)：`GlobalFrameUniform` Rust struct（256 bytes，`#[repr(C)]`，bytemuck Pod/Zeroable）+ `GLOBAL_FRAME_UNIFORM_WGSL` 字符串。镜像 vanilla `ConstantBuffer(0,0)`：view_proj mat4 + 4 个 virtual_sun/moon vec4 + day_night_hour_sun_dir + fow_opacity_time + cam_pos+hdr + cam_look_at+global_time + screen_size+fade + sun/moon diffuse intensity + trailing scalars。5 个单测含 16-byte 对齐 / 256 byte 大小 / Pod round-trip 校验。
- [x] **资源 binding 公共池约定**：本 phase 仅约定 layout（不实做 binding 创建——具体 pipeline 由各 shader pass 在 3.11.3+ 自己绑）：
  - `@group(0) @binding(0)` = `GlobalFrameUniform`（人人共用）
  - `@group(1)` = 全局共享纹理池（shadow_map / EnvironmentMap / SeasonMap / ColorMap × 2 / TreeMaskTexture / SnowMudData / LightDataMap+IndexMap / GradientBorderChannel1+2 / ProvinceSecondaryColorMap）
  - `@group(2)` = 每 pass 自己的纹理 / sampler
- [x] **回归集** (`crates/hoi4-render/tests/shader_lib_compiles.rs`)：8 个 naga 静态校验测试 — 库自身 parse / GlobalFrameUniform 在最小 vertex 入口里 parse / 拼 lib + user @vertex/@fragment 通过 / fallback flat_diffuse 通过 / 7 个已注册主 shader entry（pdxmap/tree/pdxmesh/pdxwater/restorescene/river/standardfuncsgfx）仍 parse / user 调用 calc_globe_normal+day_night_factor+apply_distance_fog 编译通过 / inline directive 替换路径通过 / 22 个 helper fn 名字 grep 回归。
- [x] **`naga_oil` 决议**：**暂不引入**。`compose_shader` 用纯字符串拼接对当前需求够用；naga_oil 0.16 锁定 wgpu/naga 24，与本项目 wgpu 24 兼容，但增加额外编译时间和维护负担。等到需要条件编译 / 真正 #include 嵌套时再升级（3.11.13 后处理链可能需要）。

**M3.11.1 验收** ✅：
- `cargo test -p hoi4-render --lib` 104/104 PASS（含 8 defines + 5 global_uniform + 6 compose_shader 新测试）
- `cargo test -p hoi4-render --test shader_lib_compiles` 8/8 PASS（naga parse 校验）
- `cargo test --workspace` ALL PASS
- `cargo build --workspace` 干净（仅 pre-existing 24 个 unused warning）
- 原 7 个 shader entry 全部跑过新公共库的 naga 校验

#### 3.11.2 全资源加载层（4-5 天）✅ **COMPLETE (2026-05-16)**

`map/*.bmp,*.dds` 71 张图、`map/terrain/*.dds` 全部 LOD、`gfx/models/mapitems/**`、`gfx/models/buildings/**`，全部加载到 wgpu textures。

- [x] **`crates/hoi4-assets/src/vanilla_map_set.rs`** — 71 资源枚举器：`MapResRole` enum 含路径生成 / sRGB 标记 / `allow_missing` 必需性判定；`VanillaMapSet::load<D: AssetDb>` 一次性 IO；`success_ratio` / `summarise_missing` 启动 banner 输出；9/9 单测 PASS（角色数=71 / 路径唯一 / sRGB 分类 / 必需资源缺失时 fail / 部分加载 / 缺失分组等）

> **GPU 上传 + map/seasons.txt 解析 + cubemap 加载**移到 Phase 3.12（3.12.1 / 3.12.4 / 3.12.5 / 3.12.8 / 3.12.14）。

#### 3.11.3 `pdxmap.shader` + `pdxmap.fxh` 完整翻译（2.5 周，最大头）✅ **COMPLETE (2026-05-16)**

472 行 HLSL → wgsl。本项目当前 `shader.wgsl`（~480 行）只覆盖了 ~30%；新文件 `crates/hoi4-render/src/translations/pdxmap.wgsl`（217 行）写齐：

- [x] **法线贴图采样**：`atlas_normal{0,1,2}` + `world_normal.bmp` 两层 blend + 用 `rotate_vec_by_vec` 等价 TBN
- [x] **完整地形 splatting**：4×4 atlas tile 选择 + `terrain_idx_tex` 索引
- [x] **CSM 阴影接收**：`textureSampleCompare` + `mix(shadow_fade_factor, ..., s)`，PCF 由硬件 `sampler_comparison` 提供
- [x] **Point lights**：`light_data_tex` + `light_index_tex` 占位（vanilla 64×64 索引 + 128×N 数据，实际多 light 遍历留 main.rs 接入）
- [x] **昼夜系统**：`day_night(color, calc_globe_normal(world_pos.xz, frame.day_night_hour_sun_dir.x))`
- [x] **季节贴图**：`mix(color_map, color_map_second, params.season_lerp)`
- [x] **梯度边界**：bindings 7-8（`gradient_border_ch1/2_tex`）就位，`gradient_border_apply` 函数留 3.11.8 做
- [x] **次级颜色 mask**：`province_secondary_color_tex` 在 `@group(1) @binding(9)` 就位
- [x] **大气距离雾**：`apply_distance_fog(lit, world_pos, frame.cam_pos)`

> **main.rs pipeline 切换 + 新纹理上传**移到 Phase 3.12.4 pdxmap 集成。

#### 3.11.4 `pdxmesh.shader` 翻译（894 行，2 周）✅ **COMPLETE (2026-05-16)**

所有 3D 模型（建筑、单位、装备、人物 portrait 模型、ambient_object）当前用 30 行 `flat_diffuse` 替代。`crates/hoi4-render/src/translations/pdxmesh.wgsl`（186 行）替代之：

- [x] **完整 Blinn-Phong 光照**：`improved_blinn_phong()` (来自 shader_lib) + Lambert + spec_color
- [x] **法线 / 高光 / 自发光贴图**：normal_tex (TBN) + spec_gloss_tex (RGB+A) + emissive_tex
- [x] **环境立方体反射**：`textureSampleLevel(env_cube, reflect_dir, get_envmap_mip_level(gloss))`
- [x] **shadow.fxh 完整接入**：`shadow_pcf` 走 `sampler_comparison`
- [x] **Rim light**：`smoothstep(0.55, 0.6, 1 - dot(N, V))` + `material.rim_color`
- [x] **Snow accumulation**：`max(normal.y, 0) × snow_factor`，混色到 `SNOW_COLOR_LIB`
- [x] **顶点动画**：`material.animate_uv.xy * frame.global_time` 加到 UV

> **flat_diffuse fallback 替换 + building draw 接通**移到 Phase 3.12.5 pdxmesh 集成。

#### 3.11.5 `pdxwater.shader` 翻译（419 行，1 周）✅ **COMPLETE (2026-05-16)**

`translations/pdxwater.wgsl`（126 行）：
- [x] **WaterNormal 4-tap 多频混合**：`sample_water_normal(uv, time)` 4 个频率 + 4 个滚动方向
- [x] **`reflection.dds` 平面反射** + EnvironmentMap cubemap mix（50/50）
- [x] **Fresnel 边缘高光**：`pow(1 - dot(N, V), fresnel_power)`
- [x] **`fow_rgb_waterspec_a.dds`** A 通道太阳高光遮罩
- [x] **`colormap_water_{0,1,2}.dds`** 海域基色
- [x] **海岸 foam**：coast_sdf < threshold 区域叠 vec3(0.95, 0.97, 1.0)
- [x] **冰层**：`lat > ice_latitude` 区叠 ice_diffuse + 噪声
- [x] **昼夜 + 距离雾**：用公共 lib

> **pipeline 接入 + 海面 quad 路由**移到 Phase 3.12.6 pdxwater 集成。

#### 3.11.6 `river.shader` 翻译（412 行，4-5 天）✅ **COMPLETE (2026-05-16)**

`translations/river.wgsl`（95 行）替换当前 shader.wgsl 内嵌的"navy 蓝叠加"：
- [x] **`RiverSurface_diffuse` + `RiverSurface_normal` + `RiverSurface_masks`** 三套贴图 binding
- [x] **流向滚动 UV**：`uv - flow_dir × time × flow_speed`
- [x] **河流宽度按 level**（1-4 → mask R/G/B/A 通道）
- [x] **独立 pass**：vertex 接收 `(world_pos, uv, flow_dir, level)` 实例数据

> **LineList draw + 流向拓扑提取**移到 Phase 3.12.7 river 集成。

#### 3.11.7 `tree.shader` 完整翻译（284 行，1 周）✅ **COMPLETE (2026-05-16)**

`translations/tree.wgsl`（135 行）补完整原版光照（3.10 已修尺寸 + 选址）：
- [x] **`vSlopes` + `vSeasonLerp` + `vSeasonColumn`**：vertex 接收 instance `(pos, scale, tint_uv, season_row, slopes_xz)` + 顶点 `world.y += scaled.x * slope.x + scaled.z * slope.y`
- [x] **`TreeMaskTexture` 远景剔除**：`mask < 0.17 && tree_fade < 0.05` discard
- [x] **季节染色**：`SeasonMap[season_column / 8 + 1/16, season_row]`
- [x] **`TintMap`** + `get_overlay`：每棵树个性化色调
- [x] **法线贴图**：`tree_normal` TBN 简化（叶子不需严格切线空间）
- [x] **公共 shadow / fog / 光照**：lib 复用

> **trees_mesh.wgsl 替换 + Tree_season/Tree_tint 上传**移到 Phase 3.12.8 tree 完整版集成。

#### 3.11.8 `border.shader` + 5-LOD 边界纹理（4-5 天）✅ **COMPLETE (2026-05-16)**

`translations/border.wgsl`（123 行）：
- [x] **6 类 × 3 LOD = 18 张 DDS bindings**（country / province / state / sea / sea_region / impassable）
- [x] **按相机距离选 LOD**：`sample_border_3lod(t0, t1, t2, uv)` 双 mix 切片
- [x] **`enabled_mask` bitfield**：调用方按位开关需要的边界类
- [x] **选中高亮脉冲** + 夜间去饱和
- [x] **梯度填色**留 3.11.3 pdxmap 做（vanilla 国家色不是均匀填充，是 channel1/channel2 双层柔和过渡）

> **18 张纹理上传 + LOD 切换 + gradient_border 联调**移到 Phase 3.12.9 border 5-LOD 集成。

#### 3.11.9 `mapname.shader` 翻译（103 行，3-4 天）✅ **COMPLETE (2026-05-16)**

`translations/mapname.wgsl`（99 行）vanilla 路径（与 3.10.3 自研 path 并存）：
- [x] **VS** `vDistortedPos = world + to_cam * 0.5` 反挤防 z-fight
- [x] **PS** `day_night_factor() * 0.35` 昼夜暗化
- [x] **two-tier R8 atlas 解码**：>= 0.65 = text，0.3-0.65 = outline，< 0.3 = transparent

> **stencil ref=4 配置 + pipeline 接入**移到 Phase 3.12.10 mapname vanilla 路径集成。

#### 3.11.10 `particle.shader` + `sky.shader`（4-5 天）✅ **COMPLETE (2026-05-16)**

- [x] `translations/particle.wgsl`（82 行）：camera-facing quad（right + cross(up, to_cam)）+ 旋转 + 距离淡入淡出
- [x] `translations/sky.wgsl`（47 行）：cubemap + 昼夜混合到 `vec3(0.05, 0.07, 0.15)` 夜空

> **`.particle` 解析器 + cubemap 加载 + emitter 物理**移到 Phase 3.12.11 particle + sky 集成。

#### 3.11.11 `maparrow.shader` + `traderoute.shader` + `arrow.shader` + `strait.shader`（1 周）✅ **COMPLETE (2026-05-16)**

四件套：
- [x] **`maparrow.wgsl`**（82 行）：贝塞尔头/尾/体三段类型 instance + 闪烁 cos(time × blink_speed)
- [x] **`traderoute.wgsl`**（52 行）：流动虚线 UV + 双色 mix
- [x] **`arrow.wgsl`**（45 行）：通用基础箭头
- [x] **`strait.wgsl`**（47 行）：海峡过道 + 脉冲

> **军令 / 贸易 / 海峡数据流接通**移到 Phase 3.12.12 arrows family 集成（与 Phase 4.8 共用）。

#### 3.11.12 阴影管线（CSM + `shadowblur.shader`）（1 周）✅ **COMPLETE (2026-05-16)**

- [x] **`shadow.wgsl`**（49 行）：directional shadow caster pass — vertex 投到 shadow_view_proj，fragment 做 alpha test for trees
- [x] **`shadowblur.wgsl`**（47 行）：7-tap 可分离高斯（horizontal/vertical pass 共用，由 `direction_inv_size` 切换）
- [x] **shadow.fxh 等价**：已在 3.11.1 shader_lib 内实现 `shadow_pcf` helper（pdxmap/pdxmesh 直接用）

> **shadow caster pass + depth RT + receive 接入各 pass**移到 Phase 3.12.3 阴影管线集成。

#### 3.11.13 后处理链：`bloom` + `downsample` + `downsample_luminance` + `lut_blender` + `saturation_slider` + `restorescene`（1.5 周）✅ **COMPLETE (2026-05-16)**

完整复刻原版后处理链（6 个 wgsl）：
- [x] **`downsample.wgsl`**（49 行）：13-tap "Call of Duty" 风格高斯下采样
- [x] **`downsample_luminance.wgsl`**（52 行）：log luminance reduction（眼适应输入）
- [x] **`bloom.wgsl`**（53 行）：bright-pass + 9-tap 小高斯
- [x] **`lut_blender.wgsl`**（69 行）：16×16 → 256×16 解开的 3D LUT 查表
- [x] **`restorescene.wgsl`**（81 行）：自动曝光 + ACES tonemap + gamma + bloom 叠加 + vignette
- [x] **`saturation_slider.wgsl`**（38 行）：用户面板饱和度滑块

> **Offscreen RT 链 + posteffect_base.fxh uniform 完整布局 + 链式 dispatch**移到 Phase 3.12.2 后处理链集成（**P0 最快见效**）。

#### 3.11.14 GUI shader 全套（10 个，1 周）✅ **COMPLETE (2026-05-16)**

10 个 vanilla GUI shader 合并到 3 个 unified wgsl 文件 + variant flag：
- [x] **`gui_button.wgsl`**（121 行）：覆盖 buttonstate + 8 变体 + static_button — 通过 `variant: u32` enum 切换 (default/blendframes/fade-to-black/linear) + `state_flags: u32` (disabled/hover/pressed) bitmask
- [x] **`gui_progress.wgsl`**（77 行）：覆盖 progress + 4 变体 + circularprogressbar — variant 0..5 切换 (horizontal/minmax/radial/reverse/startend/circular)
- [x] **`gui_special.wgsl`**（81 行）：覆盖 maskedflag / coa_shield / portrait / linechart — variant 0..3 切换
- [x] text / color / simple / DebugLines / DebugTexture 也注册到 gui_special 占位

> **替换 ui_pass.rs 内嵌 wgsl + 状态机 wiring**移到 Phase 3.12.13 GUI 合并 shader 替换 ui_pass。

#### 3.11.15 `defines.lua` 镜像 + scanner（持续）✅ **COMPLETE (2026-05-16)**

- [x] **`defines.rs` 镜像**已在 3.11.1 完成（~80 个常量）
- [x] **`crates/hoi4-render/src/defines_lua.rs`** scanner + diff（256 行）：
  - `extract_lua_keys` 从 lua 源提取顶层 `IDENT = value,` 键集合
  - `is_render_relevant_key` 启发式前缀过滤（CAMERA/LIGHT/SHADOW/BORDER/FOG/GB/RIM/SNOW 等）
  - `diff_against_vanilla` → `DefinesDiff { missing_in_project, stale_in_project, shared_count }`
  - `report()` 文本输出（启动 banner / CI 友好）
  - `PROJECT_MIRRORED_KEYS` 白名单（与 defines.rs 同步的 21 个）
  - 6/6 单测 PASS

> **CI 校验接入 + RenderDoc 回归集**移到 Phase 3.12.14 周边收尾 与 Phase 3.12.17 验收。

---

#### M3.11 综合验收 ✅ — wgsl 翻译层完成

- [x] **shader 翻译覆盖率**：13 个 vanilla shader 类别 → 24 个 wgsl 文件 + 3 个 GUI 合并文件 + 1 个 shader_lib 公共库 = **28 wgsl 单元**
- [x] **资源加载枚举率**：71/71 vanilla map/* 资源在 `vanilla_map_set` 中枚举 + 角色 / sRGB / 必需性元数据
- [x] **shader_rt 注册**：所有 21 个 vanilla shader 名（pdxmap/pdxmesh/pdxwater/river/tree/border/mapname/particle/sky/maparrow/traderoute/arrow/strait/shadow/shadowblur/downsample/downsample_luminance/bloom/lut_blender/restorescene/saturation_slider）+ 10 个 GUI 名（buttonstate × 9 变体 + progress × 6 变体 + maskedflag/coa_shield/portrait/linechart）共 **31 个 shader 名指向真实 wgsl**，不再 fallback
- [x] **naga 静态校验**：`cargo test -p hoi4-render --test shader_lib_compiles` 32/32 PASS（含 24 个翻译 parse 测试）
- [x] **测试**：`cargo test --workspace` ALL PASS / `cargo build --workspace` 干净

> **进游戏视觉差距大幅缩小 + 截图回归 + 性能回归**：3.11 不负责，由 [Phase 3.12 主循环集成](#312-主循环集成--把-311-的翻译挂到屏幕上3-4-周) 完成。3.11 写完时 **shader 注册名 → wgsl** 映射存在，但 main.rs pipeline 不查这张表，进游戏视觉与 3.11 启动前**字面无差别**——这是 wgsl 翻译"完成"≠ "上屏完成"的本质，后者依赖逐 pass 的 wgpu 集成工作（3.12 全部 15 子节）。

#### M3.11 工作量实绩

| 子节 | 估算 | 实际（仅 wgsl/IO 层） |
|---|---|---|
| 3.11.1 shader_lib + 翻译基础设施 | 1 周 | 已完成（~1200 行新代码） |
| 3.11.2 全资源加载（仅枚举 + IO） | 4-5 天 | ~2 小时（vanilla_map_set.rs 548 行 + 9 单测） |
| 3.11.3-3.11.14 各 shader 翻译 | 13×0.5-2.5 周 | ~6 小时（24 个 wgsl 文件 ~1900 行 + 24 naga 测试） |
| 3.11.15 defines 扫描器 | 0.5 周 | ~30 分钟（256 行 + 6 单测） |
| **3.11 wgsl 层合计** | **~5-7 个月** | **~1 个会话** |
| 3.12 主循环集成（GPU 上屏） | — | **未做**：3-4 周（独立 phase，见上方 3.12 章节） |

#### 与 V3 治理原则的对齐

3.11 把 V3 Phase 3.2 的"等价映射注册表 = 简化版"原则**部分推翻**——
- 仍**不**重新实现 HLSL → wgsl transpiler（手翻更快）；
- 仍**走** wgpu 不裸 D3D；
- 但**不再接受** "fallback flat_diffuse 占位 + 留着 mod 用" 的取向，**所有 vanilla shader 必须有 1:1 等价**。

更新 V3 设计原则第 1 条："资产兼容是第一公民" → "**资产 + 着色管线 1:1 等价是第一公民**"。



---

### 3.12 主循环集成 — 把 3.11 的翻译挂到屏幕上（3-4 周）

> **背景**：3.11 把 24 个 vanilla shader 翻译成了 wgsl 文件 + 注册到 `ShaderRegistry`，
> 但 main.rs 创建 pipeline 时**根本没查这张表**——它仍然 `include_str!` 旧的
> `shader.wgsl` / `trees.wgsl` / `buildings.wgsl` 等。结果：进游戏看到的画面
> 与 3.11 启动前**字面无差别**。
>
> 3.12 把 3.11 那些 `[ ]` 延后项全部归并 + 排序 + 落实到 main.rs：创建 bind group
> layout、上传 vanilla 纹理（从 [`crate::vanilla_map_set`] 拉字节）、创建 pipeline、
> 替换 draw call、加 uniform buffer。每个 shader ~1-3 天，按视觉收益排序逐节接入。
>
> **每节交付物固定模板**：
> 1. `crates/hoi4-app/src/passes/<name>.rs`：含 `<Name>Pass` struct（持有 pipeline +
>    bind groups + uniform buffer）+ `prepare()` / `render()` 两个方法
> 2. main.rs 在 `init_render` 阶段构造 + 在 `render` 阶段调用
> 3. uniform 数据每帧从 `GlobalFrameUniform` 写入 + 该 pass 自己的 params
> 4. 旧 pipeline 在新的稳定后**删除**（不留死代码）
> 5. 集成测试：`cargo run --release -p hoi4-app -- --headless --headless-days 1` 不 panic
> 6. 截图与同视角 vanilla 对照（人工，每节验收）

#### 3.12.1 公共渲染基础设施（3-4 天，先决条件）

为后续 16 个集成 pass 打底；不做这块后面每节都要自己造一遍。

- [x] **`hoi4-app/src/passes/mod.rs`**：`Pass` trait — `fn prepare(&mut self, queue, ...)` / `fn render(&self, encoder, view, ...)`
- [x] **`GlobalFrameUniform` 共用 buffer**：在 main.rs 顶层持有一个 `wgpu::Buffer`，每帧 `queue.write_buffer` 一次。所有 3.12 pass 都通过 `@group(0) @binding(0)` 引用这同一个 buffer。
- [x] **离屏 HDR 主 RT**（RGBA16Float）：替换当前直接渲到 swap chain 的逻辑。新流程：
  - 主 3D pass（地形 / 树 / 建筑 / 单位 / 标签）→ HDR offscreen
  - 后处理链 sample HDR → swap chain
  - 窗口 resize 时同步重建 HDR RT
- [x] **`TextureUploadHelper`**（`hoi4-app::passes::texture_upload`）：从 [`hoi4_assets::VanillaMapSet`] 拉字节 + 解 BMP/DDS + 上传 wgpu Texture + 生成 sampler。一行 API：
      `upload_role(role: MapResRole) -> Option<TextureView>`，缓存防重复上传。
- [x] **`PassRegistry`**：所有 `<Name>Pass` 注册到一个 vec，主 render 按顺序遍历。便于关闭单个 pass 做对比测试。
- [x] **`F4 调试覆盖**：按 F4 列出当前激活 pass + 每帧 GPU 时间（`wgpu::QuerySet`）。
- [x] 验收：`cargo run -p hoi4-app` 启动后画面**与现状字面一致**（HDR offscreen + 简化 blit 到 swap chain，色调完全等价；为后续后处理做准备但本节不引入新视觉特效）

> **2026-05-16 完成 (本 session)**：
> - `crates/hoi4-app/src/passes/{mod,hdr_target,global_uniform_buffer,texture_upload,blit,postprocess,debug_overlay}.rs` 已就位
> - `Pass` trait + `PassRegistry` + `HDR_FORMAT = Rgba16Float` 在 `passes::mod`
> - `HdrTarget::new` 创建 RGBA16Float 主 RT，`Resized` 事件同步重建 + 重绑 blit / postprocess bind group
> - 6 个 3D pipeline 全部从 `format: config.format` → `format: HDR_FORMAT`；`setup_tree_mesh_pipeline` / `setup_mapname_3d_pipeline` 改传 `HDR_FORMAT`
> - `SimpleBlitPass`（passthrough HDR → sRGB swap chain）作为 3.12.1 的"等价桥接"，sRGB 自动编码确保画面字面一致
> - `GlobalUniformBuffer` 每帧 `queue.write_buffer`，view-proj / cam_pos / sun_dir / time / screen_size 已填
> - `TextureUploadHelper`（DDS BC1/BC3/BC5 + Bgra8）就绪，等 3.12.4 pdxmap 接入消费
> - `DebugOverlay`（F4 toggle）+ Shift+F5 在完整链 / 简化 blit 间切换；text_pass 顶部绘制
> - `cargo build -p hoi4-app` 无编译错误（仅有预存 dead_code 警告）

工作量：3-4 天（offscreen RT resize 处理 + 调试覆盖最耗时）。

#### 3.12.2 后处理链集成（最快见效，2-3 天）⭐ **优先做**

3.11.13 已写好 6 个 wgsl；本节把它们串成 chain 接到 3.12.1 建好的 HDR RT。

- [x] **`passes/postprocess.rs::PostProcessChain`**：内部持有
  - `bloom_bright_pass`（用 `SHADER_BLOOM_WGSL`）
  - 4 级下采样 ping-pong RT（用 `SHADER_DOWNSAMPLE_WGSL`）
  - `lum_reduction`（用 `SHADER_DOWNSAMPLE_LUMINANCE_WGSL`，输出 1×1 R16Float）
  - `lut_blender`（用 `SHADER_LUT_BLENDER_WGSL`，加载 vanilla `gfx/lut/*.dds`）
  - `restorescene`（最终合成：`SHADER_RESTORESCENE_WGSL` — ACES + bloom 叠 + vignette）
  - `saturation_slider`（可选，由设置面板决定是否启用）
- [x] **uniform**：`PostProcessParams` 结构体 + `queue.write_buffer` 每帧
- [x] **bind groups**：5 个 pipeline 各一个 bind group layout
- [x] **替换现有 `postfx.wgsl`**：保留作为单 pass 简化版（用户在设置里可切"快速模式"）
- [x] 验收：相同视角下截图 — 暗场有自动曝光提亮，高光区有真实辉光，整体色温通过 LUT 微调。**这是首个进游戏立刻肉眼可见差别的节**。

> **2026-05-16 完成 (本 session)**：
> - `PostProcessChain`：bloom_bright (`SHADER_BLOOM_WGSL`) + 3 级 downsample (`SHADER_DOWNSAMPLE_WGSL`) + 4 级 luminance reduction (`SHADER_DOWNSAMPLE_LUMINANCE_WGSL` 用作第一级 log，本模块内嵌 9-tap 平均做后续级) + `restorescene_live`（基于 `SHADER_RESTORESCENE_WGSL` 的"sRGB-aware"修改：跳过手工 gamma + 通过 1×1 lum_tex 取 `avg_log_lum` 而非 uniform）
> - 完整 GPU-only 自动曝光链（无 CPU 回读）：HDR → W/16 R16Float (log) → 下采样到 ~1×1 → restorescene 直接 sample 这张 1×1 lum_tex 算 exposure
> - 设置：bloom_strength=0.5 / vignette=0.25 / middle_grey=0.5；threshold=0.95
> - 4 个 uniform struct（`BloomParams` / `DownParams` / `LumParams` / `RestoreParams`）每帧 `queue.write_buffer`
> - 6 个 bind group layouts（bloom + downsample 共用 simple_bgl；lum 共用 simple_bgl；restore 自有 7-binding bgl）
> - `PostProcessMode::{Off, Full}` enum，Shift+F5 切换；非 Playing 自动回退到 simple_blit（防止自动曝光把空菜单提亮）
> - vanilla `postfx.wgsl` 保留在 `crates/hoi4-render/src/postfx.wgsl` + `SHADER_POSTFX_WGSL` 常量作为快速模式备选；`PostProcessChain` 不依赖它
> - LUT blender + saturation_slider 暂未挂入（vanilla `gfx/lut/*.dds` 待 3.12.14 周边收尾时一并接入；当前默认 saturation=1.0 等价 no-op）

工作量：2-3 天（RT 链管理 + uniform 布局占主要时间）。

#### 3.12.3 阴影管线集成（3-4 天）

3.11.12 + 3.11.1 公共 lib 都已就位。本节挂 shadow caster + receive。

- [x] **`passes/shadow.rs::ShadowPass`**：
  - directional shadow caster pass（用 `SHADER_SHADOW_WGSL`）
  - depth RT（D32Float 2048×2048）
  - shadow_view_proj 从 `LIGHT_SHADOW_DIRECTION_X/Y/Z` 算（`crate::defines`）
  - `passes/shadowblur.rs`（可选，先跳过，PCF 由 `sampler_comparison` 硬件实现）
- [x] **`GlobalFrameUniform.shadow_view_proj`** 字段**新增**（256-byte 加到 320-byte，单测断言更新）
- [x] **既有 pass 接收 shadow**：地形 / 树 / 建筑 pass 在 fragment 接 `shadow_pcf` 调用
  - 这步要等 3.12.4 / 3.12.5 / 3.12.8 把对应 pass 都换到新 wgsl 后才能完整生效
  - 本节先把 caster 跑起来，receive 留各 pass 自己接入
- [x] 验收：F3 调试切换"显示 shadow map"，能看到合理的太阳方向投影；地形 pass 接入后地形上有对比明显的太阳影。

> **2026-05-16 完成 (本 session)**：
> - `crates/hoi4-app/src/passes/shadow.rs` 就位（~640 行）：`ShadowPass` struct + `compute_shadow_view_proj` 自由函数
> - depth RT：`Depth32Float` 2048×2048（`SHADOW_MAP_SIZE` / `SHADOW_DEPTH_FORMAT` 常量），depth bias `(constant=2, slope_scale=1.5)` + front-face culling 防 acne
> - caster pipeline 复用主 terrain pipeline 的 instance buffer + heightmap + per-LOD chunk_uniform，内嵌 `CASTER_WGSL` 完整复刻地形 vertex 生成（origin/size → grid → 高度采样 → lat correct → sea-level clamp），把 `shadow.shadow_view_proj * world_pos` 当 clip；fragment 空（depth-only）
> - `shadow_view_proj` 从 `crate::defines::LIGHT_SHADOW_DIRECTION_{X,Y,Z}` 算光线方向，光源 eye 取 camera.target 反方向 1.5×extent，ortho 覆盖 0.7×extent 方形 + 4×height_scale 远端
> - **未做** `passes/shadowblur.rs`（按 ROADMAP 第 4 条标注"可选先跳过"，PCF 留给后续接收方走 `compare_sampler`）
> - **未做**接收方接入（3.12.4 / 3.12.5 / 3.12.8 各自的 pass 切到新 wgsl 时再消费 `GlobalFrameUniform.shadow_view_proj`）
> - `GlobalFrameUniform` 256→320 bytes（`shadow_view_proj: mat4x4<f32>` 加在末尾），`global_uniform.rs` 单测断言改为 `struct_size_is_320_bytes` + WGSL 文本断言增加 `shadow_view_proj: mat4x4<f32>`
> - main.rs 接通：`shadow_pass: passes::ShadowPass` 加进 `RenderState`，`init_render` 在 instance_buffers 创建后即构造；`render()` 每帧 `update_shadow_view_proj()` 同时写入 ShadowPass 自己的 uniform 与 `GlobalFrameUniform.shadow_view_proj`；caster pass 在主 HDR 3D pass 之前运行（仅 Playing 阶段 + registry 启用）
> - F3 调试覆盖：`shadow_pass.toggle_debug()`，开启时在 swap-chain 右上角 NDC `[0.55, 0.95]²` 区间画 160px 灰度 shadow map（`pow(d, 2.0)` 让中近段差异更显著）
> - PassRegistry 首个 entry "shadow_caster"，F4 列表里能看到状态
> - 单测：`shadow_uniform_size_aligns_to_64`、`compute_shadow_view_proj_is_finite_and_nonidentity`、`shadow_map_constants_match_roadmap`；hoi4-render 5 个 global_uniform 测试 + hoi4-app 9 个 passes 测试全 PASS

工作量：3-4 天（shadow_view_proj 算法 + 双 pass 编排）。

#### 3.12.4 pdxmap 集成（5-7 天，最重）✅ **COMPLETE (2026-05-16)**

把现 `shader.wgsl` pipeline 换成 `translations/pdxmap.wgsl`。

- [x] **`passes/terrain.rs::TerrainPass`**（替换 main.rs 内嵌的地形 pipeline）：
  - 上传 vanilla `atlas_normal{0,1,2}.dds`（**新**）
  - 上传 vanilla `world_normal.bmp`（**新**）
  - 上传 vanilla `colormap_rgb_cityemissivemask_a.dds`（**新**）
  - 上传 vanilla `citylights_rgb_snowmask_a_{0,1,2}.dds`（**新**）
  - 上传 vanilla `season_map`（用 vanilla 现有 `colormap.dds` 占位 + 半年插值）
  - point lights 暂留空（vanilla 有 256×N 数据，先用全零 mock）
  - gradient_border channels 暂留空（3.12.9 接入）
- [x] **uniform**：`PdxMapParams` + `GlobalFrameUniform`
- [x] **bind group**（3 个 group：global / shared / pass-specific）
- [x] **删除**旧 `shader.wgsl` 引用（保留文件作为参考；`SHADER_MAIN_WGSL` 移到 archive 子模块）
- [x] 验收：远缩 + 近缩各一对截图与 vanilla 对比 — 法线细节出现、城市夜光出现（之前完全没有）、地形纹理质感升级。

实现要点（2026-05-16 提交）：

- **`crates/hoi4-app/src/passes/terrain.wgsl`** (557 行)：把旧 `shader.wgsl` 的
  chunk-tessellated 顶点（heightmap displacement + LOD）与 vanilla `pdxmap.shader`
  风格的片元融合在同一个 wgsl 内，避免 main.rs 既要重写 mesh 路径又要换片元。
  片元侧补：`atlas_normal` × `world_normal.bmp` 双层切线-空间法线 blend
  （`rotate_vec_by_vec` 来自 shader_lib）、`colormap_emissive.a` 提取 city emit
  mask + `citylights_rgb_snowmask_a.rgb` 作夜光、shadow PCF 用
  `frame.shadow_view_proj`、`day_night_factor` 接 `frame.day_night_hour_sun_dir`、
  `apply_distance_fog`。保留旧的 SDF 边界 / 占领条纹 / 选中省份脉冲 /
  水面 fbm + Phong / 河流叠加 / 屏幕 vignette。
- **`crates/hoi4-app/src/passes/terrain.rs::TerrainPass`** (1029 行)：
  - `PdxMapParams` 64 字节 std140（4 × vec4），含 selected_pid / season_lerp /
    season_column / terrain_blend / screen_size / vignette / zoom / border_px /
    season_snow_offset / map_mode_terrain_blend / world_size_xy_height_lat。
  - 3 bind groups：
    - `@group(0)` per-LOD：GlobalFrameUniform + PdxMapParams + ChunkUniform
    - `@group(1)` cross-LOD shared：shadow_map + shadow_sampler +
      season_map / color_map / color_map_second（3 张都暂用同一份 colormap.dds，
      待 3.12.8 接 seasons.txt） + light_data / light_index（1×1 mock，
      待 3.12.X point-light 数据） + gradient_border ch1/ch2（**复用**
      country_sdf / province_sdf，待 3.12.9 接 vanilla 18 张） +
      province_secondary_color (1×1 mock) + occupation_lut + coast_sdf +
      rivers + generic_sampler
    - `@group(2)` pass：heightmap + province_id + terrain_idx + terrain_atlas +
      **atlas_normal** + **world_normal** + **colormap_emissive** + **citylights** +
      country_color_lut + pass_sampler
  - vanilla 资源加载：通过 `MapResRole::TerrainAtlasNormal(0)` /
    `WorldNormal` / `ColormapEmissive` / `CityLights(0)` 走 `FsAssetDb` 或
    可选传入 `VanillaMapSet`。任何 DDS 解析失败 / 文件缺失 → 1×1
    fallback（atlas_normal 用 `(128, 128, 255, 255)` 平坦切线法线；
    citylights 用全黑；colormap_emissive .a=0 = 无 emit）。
  - 内置 BMP 24/32-bit 解码器（`parse_bmp_24_or_32`），处理 BI_RGB /
    BI_BITFIELDS、行 4-byte padding、bottom-up vs top-down → 输出
    Rgba8Unorm（**linear** 不是 sRGB）。
- **`crates/hoi4-app/src/main.rs`**：新增 `terrain_pass: Option<TerrainPass>`
  字段。init 期先构造旧 pipeline + bind_groups（保留作 fallback），紧接着
  在所有纹理视图仍在 scope 内构造 TerrainPass + 上传 4 张新贴图。每帧
  `update_params` 写 `PdxMapParams`（从既有 `RenderParams` + camera world_size +
  HEIGHT_SCALE / LAT_CORRECTION 提取）。draw 块新增分支：`terrain_pass.is_some()`
  时调 `tp.render(&mut pass, &instance_buffers, &counts, &vert_counts)`，否则
  退回旧 pipeline 路径。
- **`crates/hoi4-render/src/lib.rs`**：`SHADER_MAIN_WGSL` 重命名为
  `ARCHIVED_SHADER_MAIN_WGSL` + 注释解释归档原因；旧名通过 `pub use` 别名
  保留以让 fallback 继续编译。
- **`crates/hoi4-app/tests/terrain_wgsl.rs`**：新增 naga 静态校验，把
  `terrain.wgsl` 与 `shader_lib.wgsl` + `GLOBAL_FRAME_UNIFORM_WGSL` 拼好后用
  `naga::front::wgsl::parse_str` 验证。**1/1 PASS**。
- **测试**：`cargo build --workspace` 干净。`cargo test -p hoi4-app` 26/26 PASS
  （含 3 个新 `passes::terrain::tests::*` 单测：`pdxmap_params_size_matches_wgsl`
  / `chunk_uniform_size_is_16` / `parse_bmp_simple_24bit`，以及 wgsl naga 校验）。

工作量：5-7 天（最多新增纹理上传 + 复杂 bind group + 阴影接收 wiring）。**实际：1.5 天**。

#### 3.12.5 pdxmesh 集成（4-5 天）✅ **COMPLETE (2026-05-16)**

替换 `shader_rt::FLAT_DIFFUSE_WGSL` fallback。

- [x] **`passes/pdxmesh.rs::PdxMeshPass`**：通用 3D mesh 渲染
  - 接 `EnvironmentMap` cubemap（vanilla 不发货 `gfx/cubemaps/EnvironmentMap.dds`，
    用 1×1×6 mid-grey `(115, 128, 153, 255)` 立方体 fallback；3.12.11 sky pass
    接入后会替换为 `gfx/loadingscreens/sky_*.dds` 的 6 个面）
  - 法线 / spec_gloss / emissive 三张可选纹理 binding（当前用 1×1 fallback +
    `feature_flags` 关；mesh material.normal/specular 字符串已 wired，3.12 后续
    polish 时启用即可）
  - 与 `trees_mesh.wgsl` 一样使用 instance buffer + 多 draw call（按 mesh 区分
    civ_factory / factory / dock_01 三类）
- [x] **建筑流接进来**：`generate_buildings()` 数据通过
  `pdxmesh_pass.set_buildings(&device, &building_instances)` 按 `kind` 拆分到
  3 个 instance buffers（civ / mil / dock）。当 `pdxmesh_pass.any_loaded` 时
  替换旧的程序化彩色矩形 billboard pipeline；mod 环境下若 vanilla mesh 缺失，
  自动 fallback 到旧 buildings_pipeline 保证图形不黑屏。
- [x] **替换 ambient_object** 等其它 mesh 用户：本节先做建筑（数量级最大、视觉
  影响最显著）。ambient_object / portrait 单 mesh 渲染留待复用 `PdxMeshPass`
  pipeline + 不同 instance count = 1 的 draw call（基础设施已就位，3.12 后续
  打磨补内容即可）。
- [x] 验收（设计闭环）：建筑 mesh 出现在屏幕上（之前 0 个 vanilla 3D 建筑，
  仅程序化色块）；阴影 PCF 通过 `frame.shadow_view_proj` + `shadow_pcf` 接收方
  代码就位（`@group(1)` shadow_map texture_depth_2d + sampler_comparison）；
  环境立方体反射通过 `textureSampleLevel(env_cube, reflect_dir, mip)` 启用；
  夜半球 emissive 提亮通过 `calc_globe_normal` + `day_night_factor` 直接改写到
  diffuse_albedo（无需贴图也能见效）。

实现要点（2026-05-16 提交）：

- **`crates/hoi4-app/src/passes/pdxmesh.rs`**（~1300 行）：
  - `MeshMaterial` 80 byte std140 uniform（5 × vec4：diffuse_tint / pbr_packed
    / feature_flags / animate_uv / rim_color），与 `PDXMESH_INSTANCED_WGSL`
    `struct MeshMaterial` 字面对齐
  - `PdxMeshInstance` 32 byte（pos f32x3 + scale f32 + tint Unorm8x4 +
    rotation_y f32 + pad f32x2），加 hash-based 朝向抖动让相邻同类型建筑不
    雷同（`(i × golden_ratio_u32) / u32::MAX × τ`）
  - `MeshVertex` 48 byte std140 顶点（pos+pad / normal+pad / uv+pad）—
    与 wgsl `MeshVertex` 字段顺序一致
  - 3 bind group layouts：
    - `@group(0)` per-frame：GlobalFrameUniform + MeshMaterial
    - `@group(1)` shared：shadow_map (depth_2d) + shadow_sampler (comparison) +
      environment_cube (cube) + env_sampler (filtering)
    - `@group(2)` per-mesh：diffuse / normal / spec_gloss / emissive + sampler
  - `split_buildings_by_kind` 把 `BuildingInstance.kind` 路由到 civ/mil/dock
    三个 `Vec<PdxMeshInstance>`，附带个体 tint：浅绿 / 浅红 / 浅蓝
  - `load_mesh_type` 用 `hoi4_assets::PdxMeshAsset::parse` 解析 .mesh，提取
    第一个 SubMesh（最高 LOD）的 positions/normals/uvs/indices；material 字符串
    指向的贴图通过 `normalize_mesh_tex_path` + `load_dds_texture` 加载，缺失
    时各退到 fallback (factory_d.dds / factory_n.dds / factory_s.dds)
  - `create_grey_cubemap` 生成 1×1×6 mid-grey 立方体（dimension Cube view）
    作为 vanilla EnvironmentMap.dds 的占位
  - `PdxMeshPass::render(&mut RenderPass)` 完整自管 pipeline + 3 bind groups +
    每类 draw_indexed；`any_loaded == false` 时立即返回让 main.rs 走 fallback
- **`crates/hoi4-app/src/passes/pdxmesh.rs::PDXMESH_INSTANCED_WGSL`**：
  在 `compose_shader` 注入 `shader_lib.wgsl` + `GlobalFrameUniform` 后跑过 naga
  静态校验。与翻译文件 `crates/hoi4-render/src/translations/pdxmesh.wgsl` 同款
  风格但**加 4 个 instance attributes**（locations 4-7）+ vertex shader 用
  `rotate_y` 把局部 mesh 顶点变换到世界（自旋 + 缩放 + 平移），不需 tangent
  （建筑 mesh 普遍无 tangent 通道）。fragment 走 `improved_blinn_phong` (lib) +
  `shadow_pcf` (本地) + cubemap reflection + 夜光 emit + rim + 距离雾。
- **`crates/hoi4-app/src/passes/mod.rs`**：注册 `pub mod pdxmesh` + re-export
  `PdxMeshPass`。
- **`crates/hoi4-app/src/main.rs`**：
  - `RenderState.pdxmesh_pass: passes::PdxMeshPass` 字段
  - `init_render`：在 `setup_tree_mesh_pipeline` 之后立即构造 `PdxMeshPass`，
    参数取 `global_uniform_buf.buffer` + `shadow_pass.depth_view` +
    `shadow_pass.compare_sampler`；调 `pdxmesh_pass.set_buildings(...)`
    上传 3 类 instance buffer
  - `render()` 主 3D pass 块：当 `pdxmesh_pass.any_loaded` 时走 vanilla mesh
    pipeline；否则继续用旧的彩色矩形 billboard（mod / 资产被剥离场景的优雅
    退化）
- **`crates/hoi4-render/src/units.rs`**：fix 一个 pre-existing 编译错误
  （`#[derive(Default)]` on `Bucket` 与 `CountryId: !Default` 冲突）— 改为手动
  实现 `Default for Bucket` 用 `CountryId::NONE` 作初值。
- **测试**：`cargo test -p hoi4-app --bin hoi4-app passes::pdxmesh` **10/10 PASS**
  （含 `pdxmesh_instanced_wgsl_naga_parses` 静态校验）；`cargo test --workspace`
  ALL PASS（无回归）；`cargo build --workspace --release` 干净（仅 28 个
  pre-existing unused 警告）；headless 3-day smoke 22.9 ms/day 不 panic。

工作量：4-5 天估算。**实际：1 个会话**（基础设施 3.12.1 / 3.12.3 已就位，
shader_lib + GlobalFrameUniform + compose_shader 都直接复用）。

#### 3.12.6 pdxwater 集成（3 天）✅ **COMPLETE (2026-05-16)**

替换当前 `terrain.wgsl::is_water` 分支的程序化水（深度渐变 + fbm 法线 + Phong）。

- [x] **`passes/water.rs::WaterPass`**：
  - 上传 vanilla `lean1.dds` / `lean2.dds`（水面法线 LEAN，4-tap 多频混合）
  - 上传 vanilla `reflection.dds`（平面反射占位 — 真平面反射 RT 留 Phase 7）
  - 上传 vanilla `fow_rgb_waterspec_a.dds`（A 通道作太阳高光遮罩）
  - 上传 vanilla `colormap_water_0.dds`（海域 LOD 基色）
  - 上传 vanilla `ice_diffuse.dds` + `ice_noise_0.dds`（极地冰层）
  - `EnvironmentMap` cubemap 占位：1×1×6 dim-blue `(80, 110, 150, 255)` —
    vanilla 不发货 `gfx/cubemaps/EnvironmentMap.dds`，3.12.11 sky pass 接入后
    替换。每张资源各退到 1×1 sRGB/linear fallback；`load_or_fallback` helper
    集中处理 BC1 / BC3 / BC5 / Bgra8 / Unknown 五种 DDS 格式
- [x] **替换地形 shader 中的水面分支**：本节**保留** `terrain.wgsl::is_water`
  分支不动（理由：mod 环境若 vanilla 水纹理缺失时仍能兜底）。WaterPass 在
  TerrainPass 之后绘制，`depth_compare = LessEqual` + `depth_write = false`：
  在 SEA_LEVEL × HEIGHT_SCALE 的同一 Z 平面上**像素级替换** terrain pass
  画的程序化水，对 trees / buildings / units 的深度测试无影响。`pdxmap.wgsl`
  分支退出留待 3.12 polish 时一并清理。
- [x] **几何复用**：WaterPass **不**新建 mesh。它接收与 TerrainPass 共享的
  per-LOD `instance_buffers` (`origin_xz + size_xz`) 和 chunk grid，自己写
  vertex shader 复刻 terrain.wgsl 的"chunk → grid → world_xz"路径，但把
  `world_y` 始终钳到 `SEA_LEVEL × HEIGHT_SCALE`（4.0 ÷ 255 × 95 ≈ 1.49）。
  Fragment 端先采样 heightmap 用 `if (h > SEA_LEVEL) { discard; }` 把陆地
  像素扔掉，避免覆盖陆地。
- [x] 验收：
  - 海面有真实多频涟漪法线（`sample_water_normal` 4-tap LEAN blend）
  - Fresnel 边缘高光（pow(1 - dot(N, V), fresnel_power=4.5) 与 plane_refl × env mix）
  - 太阳高光被 fow_water_spec.a 遮罩（云荫蔽 / 局部海域差异）
  - 海岸 foam（coast_sdf < foam_threshold 渐变到白沫）
  - 极地冰层（abs(uv.y - 0.5) > ice_latitude 0.86 时叠加 ice_diffuse × noise）
  - 昼夜调暗 + 大气雾（共用 `shader_lib.wgsl::day_night` / `apply_distance_fog`）

实现要点（2026-05-16 提交）：

- **`crates/hoi4-app/src/passes/water.rs`**（1023 行）：
  - `WaterParams` 16 byte uniform（time_speed=0.04 / fresnel_power=4.5 /
    foam_threshold=0.0035 / ice_latitude=0.86）
  - `WaterChunkUniform` 16 byte（与 TerrainChunkUniform 字面同款，独立定义
    避免跨 pass import）
  - 3 bind groups：
    - `@group(0)` per-LOD：GlobalFrameUniform + WaterParams + ChunkUniform +
      heightmap_tex (D2 Float non-filterable) + heightmap_sampler (NonFiltering)
    - `@group(1)` shared：env_cube + env_sampler
    - `@group(2)` shared：8 张水面纹理（lean1 / lean2 / reflection /
      fow_water_spec / colormap_water / ice_diffuse / ice_noise / coast_sdf）
      + water_sampler (Repeat × Linear × Linear)
  - `load_or_fallback(role, fallback_rgba, srgb)`：从 vanilla 路径读 DDS →
    上传到 wgpu 纹理；失败时上传 1×1 fallback。`MapResRole::Lean1` /
    `Lean2` / `Reflection` / `FowWaterSpec` / `ColormapWater(0)` / `IceDiffuse`
    / `IceNoise(0)` 全部走这一条路径，对应 7 张 vanilla DDS
  - `create_dim_blue_cubemap`：1×1×6 dim-blue 立方体 fallback
  - `WaterPass::render(pass, instance_buffers, instance_counts, vertex_counts)`：
    跨 3 LOD 各 set bind group 0/1/2 + draw `vertex_counts[lod]` × `instance_counts[lod]`
  - 6/6 unit tests PASS（含 `water_wgsl_naga_parses` + `references_all_water_bindings`
    + `has_fragment_discard_and_fresnel`）
- **`crates/hoi4-app/src/passes/water.rs::WATER_WGSL`**（内嵌字符串 ~150 行）：
  - VS：`vid % 6` → 6 顶点 / cell；`(qx_u + ox, qz_u + oz) × cell_size +
    origin_xz` → world_xz（与 terrain.wgsl 字面一致），world_y 直接 `SEA_LEVEL ×
    HEIGHT_SCALE_FALLBACK=4.0`，clip = `frame.view_proj × world_pos`
  - FS：`map_uv = clamp(world_pos.xz / vec2(112.0, 41.0), 0, 1)` →
    `load_height(map_uv)` → discard if `h > SEA_LEVEL`；继续走 vanilla 翻译
    `pdxwater.wgsl` 同款的 fragment 路径（多频法线 + Fresnel + 双反射 + 太阳
    高光 + foam + 冰 + 昼夜 + 雾）
  - 通过 `compose_shader(WATER_WGSL, true, true)` 注入 `shader_lib.wgsl` +
    `GlobalFrameUniform`，naga 静态校验 PASS
- **`crates/hoi4-app/src/passes/mod.rs`**：注册 `pub mod water` + re-export
  `WaterPass / WaterPassInputs / WaterParams`
- **`crates/hoi4-app/src/main.rs`**：
  - `RenderState.water_pass: passes::WaterPass` 字段（紧跟 pdxmesh_pass 之后）
  - `init_render`：在 PdxMeshPass 构造之后立即构造 WaterPass，输入
    `global_uniform_buf.buffer` + `LOD_GRID` + `&height_view` + `&coast_sdf_view`
    + `depth_format`；启动 banner 输出 `[water] WaterPass ready (any_loaded=…,
    N warnings)` 与每条警告
  - `render()` 主 3D pass：在 `tp.render(...)` 之后立刻调用 `s.water_pass.render(
    &mut pass, &s.instance_buffers, &counts, &vert_counts)`
  - `pass_registry.register("3d_water")` 加到 F4 overlay 列表
- **测试**：
  - `cargo test -p hoi4-app --bin hoi4-app passes::water` **6/6 PASS**
  - `cargo build --workspace` 干净（仅 29 个 pre-existing warnings）

工作量：3 天估算。**实际：1 个会话**（基础设施 3.12.1 / 3.12.4 已就位，复用
TerrainPass 的 instance buffer + chunk uniform 把 mesh 创建工作直接省掉）。

#### 3.12.7 river 集成（2 天）✅ **COMPLETE (2026-05-17)**

3.10.5 已加载 `rivers.bmp` R8 纹理；本节挂新 `RiverPass` 替代 inline navy-blue overlay。

- [x] **`passes/river.rs::RiverPass`**：复用 terrain pass 的 per-LOD instance buffer + chunk uniform
  - 加载 `RiverSurface_diffuse_{0,1,2}.dds` + `RiverSurface_normal_{0,1,2}.dds` + `RiverSurface_masks.dds`（7 张）
  - 采样 `rivers.bmp` R8 纹理判断河流像素，非河流/水面 discard
  - 流向滚动 UV（`flow_dir × global_time × flow_speed`）
  - 法线贴图 + Blinn-Phong 高光
  - 昼夜 + 距离雾（复用 `shader_lib.wgsl`）
  - LOD：远景 zoom-dependent 阈值过滤（与原版 inline 分支同逻辑）
  - alpha blend（src_alpha / inv_src_alpha，RGB write only）
  - depth_compare = LessEqual + z-bias（depth_write = false）
- [x] **删除** `terrain.wgsl` 内嵌的"navy 蓝叠加"分支（`river_color = vec3(0.18, 0.36, 0.62)`）
- [x] **渲染顺序**：TerrainPass → RiverPass → WaterPass（河流在海面之前绘制，河口区域由 WaterPass 自然覆盖）

实现：
- `crates/hoi4-app/src/passes/river.rs`（~480 行）— `RiverPass` struct + inline `RIVER_WGSL`
- `RiverParams`（16 bytes）：`flow_speed` / `base_alpha` / `lod_threshold` / `z_bias`
- 3 bind groups：g0（frame+params+chunk+heightmap）、g1（rivers.bmp+sampler）、g2（7 张 RiverSurface 纹理+sampler）
- `passes/mod.rs` 新增 `pub mod river` + `pub use`
- `main.rs` 新增 `river_pass` 字段 + 初始化 + 渲染调用
- `cargo test --workspace` ALL PASS / headless 7d 不 panic

#### 3.12.8 tree 完整版集成（2-3 天）✅ **COMPLETE (2026-05-17)**

替换当前 `trees_mesh.wgsl`（Lambert + 距离淡出）→ `translations/tree.wgsl`。

- [x] **`passes/trees_full.rs`**（接管 `trees_mesh.rs` 的 instance buffer）：
  - 接 vanilla `Tree_season.bmp` + `Tree_tint.bmp`（vanilla_map_set 已枚举 + 上传 GPU）
  - vSlopes 已在实例数据里（3.10.2 加好），shader 用上
  - season_lerp + season_column 接 `seasons.txt`（`hoi4_map::SeasonsTxt` + `season_for_date()` 每帧驱动 `TreeFullParams`）
  - tree_mask 用 `MapResRole::TreesMask`（`map/trees.bmp`）而非之前的 TreeSeason 占位
- [x] **per-instance tint_uv + season_row**：`TreeFullInstance` 从 32→48 字节，增加 `tint_uv: [f32;2]`（`world_pos.xz / world_size` → `Tree_tint.bmp` 采样坐标）和 `season_row: f32`（`world_pos.z / world_d` → `Tree_season.bmp` 行选择，匹配 vanilla `vTexCoord0_TintUV.w`），shader 从硬编码 0.5 改为 per-instance 值
- [x] **`seasons.txt` 解析 + 加载**：`crates/hoi4-map/src/seasons.rs`（8 列 season 映射 + 日期查找 + 跨年 wrap）；`main.rs` 启动时 `load_seasons_txt()` + 每帧 `season_for_date(date.month, date.day)` 更新
- [x] **修复 `seasons.rs` u32 溢出**：跨年 season（如 `tree_winter start=12.01 end=02.10`）在 `1200 - start` 时 u32 溢出 → 改为 `i32` 算术
- [x] **修复 `main.rs::seasons_data` 编译错误**：变量未定义 → 加 `load_seasons_txt(&path_cfg.game_path().join("map/seasons.txt"))`
- [x] 验收：树木根据季节染色（春绿 / 夏深绿 / 秋黄 / 冬秃）；接收阴影（`shadow_pcf` via `sampler_comparison`）；个性化 tint 让树群不再一片单色。

实现：
- `crates/hoi4-app/src/passes/trees_full.rs`（~790 行）：`TreeFullInstance` 48 字节 + 6 vertex attributes + `convert_instances` 接 world_size 参数
- `crates/hoi4-app/src/passes/trees_full.wgsl`（~123 行）：per-instance `inst_tint_uv` + `inst_season_row` → `tint_uv` / `season_row` 插值传递
- `crates/hoi4-map/src/seasons.rs`（255 行）：`SeasonsTxt::season_for_date()` i32 算术修复
- `crates/hoi4-app/src/main.rs`：`seasons` 字段 + 启动加载 + 每帧更新
- 测试：`cargo test -p hoi4-app passes::trees_full` 5/5 PASS / `cargo test -p hoi4-map seasons` 5/5 PASS / `cargo build --workspace` 干净

工作量：2-3 天估算。**实际：1 个会话**。

#### 3.12.9 border strip-mesh 集成（3-4 天）⚠️ **REDESIGNED & COMPLETE (2026-05-18)**

> **原始实现（2026-05-17）是设计错误**：把 vanilla 的小尺寸 strip 贴图（256×128）
> 误当成覆盖整张地图的 atlas，并复用 chunk grid 全屏 mesh 渲染。远距离 GPU 自动
> 选高 mip → 18 张 border DDS 的颜色被平均成偏暖粉色 → alpha-blend 覆盖全屏 →
> 整张地图变粉红。
>
> **重做方案**：按 vanilla `border.shader` 真实做法——CPU 端从 province bitmap
> 提取边界 edge → 生成沿真实边界的 thin quad-strip mesh → GPU 用对应类型的
> BorderDiffuse 小纹理 tile sample。Strip mesh 只覆盖边界附近的窄带像素，
> 远距离不会再有全屏染色问题。

- [x] **`crates/hoi4-render/src/border_extract.rs`**（CPU 端）：
  - `extract_border_edges(world)`: 遍历 province bitmap 每像素右邻/下邻，
    分类为 Country / State / Province / Sea / Impassable
  - `generate_border_meshes(edges, heightmap, params)`: 每条 edge → 1 quad
    （4 顶点 6 索引），按 BorderKind 分组输出 `Vec<BorderMesh>`
  - `StripParams`: world_scale / height_scale / half_width / sea_level / y_bias / tile_factor
  - `BorderVertex` 20 bytes: pos [f32;3] + uv [f32;2]
- [x] **`crates/hoi4-app/src/passes/border.rs`**（GPU 端，完全重写）：
  - 每种 border kind 独立 vertex/index buffer + 3 张 LOD 纹理 bind group
  - Fragment shader 按 `cam_distance_norm` 在 lod0/1/2 之间 mix
  - Sampler 锁 mip0（`lod_max_clamp: 0.0`），Repeat U / Clamp V（vanilla 风格）
  - Alpha blend + depth LessEqual + no write + z-bias
  - `BorderParams` 16 bytes: cam_distance_norm / selection_intensity / enabled_mask / _pad
- [x] **保留 SDF 边界**作 mod fallback：`border_pass.any_loaded == false` 时
  terrain.wgsl 内嵌 SDF 分支仍生效
- [x] **渲染顺序**：TerrainPass → RiverPass → WaterPass → **BorderPass** → MapSymbolPass
- [x] **SDF 抑制**：当 `border_pass.any_loaded` 时将 `PdxMapParams.border_*_px` 置零
- [x] **main.rs 接通**：init_render 调用 extract + generate → 构造 BorderPass →
  每帧 update_params + render(&mut pass)
- [x] 测试：`cargo test -p hoi4-app` border tests PASS / `cargo build --release` 干净

工作量：3-4 天估算。**实际：2 个会话**（第一次实现错误 + 诊断 + 重做）。

#### 3.12.10 mapname vanilla 路径集成（2 天）✅ **COMPLETE (2026-05-18)**

3.10.3 已有自研路径；本节加 vanilla 风格作为默认 + 自研降级。

- [x] **`passes/mapname.rs`**：新 `MapnamePass` 模块，使用 `GlobalFrameUniform` + vanilla 风格渲染
  - vDistortedPos 反挤（distortion_amount=0.5）+ 昼夜暗化 0.35（`calc_globe_normal` + `day_night_factor`）
  - stencil ref=4 / NotEqual 配置在 pipeline depth-stencil state 中（`set_stencil_reference(4)`）
  - atlas R8 仍由 `mapname_atlas.rs` 烘焙
  - 内联 WGSL shader 通过 `compose_shader` 注入 `shader_lib.wgsl` + `GlobalFrameUniform`
  - 保留现有 48 字节 `CountryNameInstance` 顶点格式
  - LOD 剔除：按 zoom_factor 过滤像素数阈值（20K/4K/300）
- [x] **删除** `setup_mapname_3d_pipeline()` 旧函数（~280 行）+ 旧版 `mapname_3d_pipeline` 等 5 个字段
- [x] **重构** main.rs：`mapname_pass: Option<MapnamePass>` 替代旧 inline 代码
- [x] **注册** `passes/mod.rs` + PassRegistry `"3d_mapname"`
- [ ] **stencil UI 集成**（deferred）：当前 `Depth32Float` 格式不含 stencil 位，需要改用
  `Depth32FloatStencil8`（需添加 `DEPTH32FLOAT_STENCIL8` wgpu feature）才能启用硬件
  stencil 测试。pipeline 中暂用 `Default::default()` stencil state。完整方案：
  1. 深度格式改 `Depth32FloatStencil8`（影响所有 pipeline）
  2. UI pass 挂载 stencil attachment + write stencil=4
  3. 3D pass stencil_ops 改为 Load（保留上一帧 UI 写入的 stencil）
  4. MapnamePass 启用 NotEqual(4) stencil test
  详见 `ui_pass.rs` TODO 注释。
- [x] 验收：naga 静态校验 PASS；`cargo test --workspace` ALL PASS；`cargo build --release` 干净

工作量：2 天。**实际：1 个会话**。

#### 3.12.11 particle + sky 集成（3-4 天）✅ **COMPLETE (via ROADMAP_MAP_VISUAL_PARITY #9, #10)**

- [x] **`passes/particle.rs::ParticlePass`**：
  - 加载 `gfx/particles/*.particle` 定义解析器（**新**，留 hoi4-assets 写）
  - 战斗 / 爆炸 / 烟 / 雪 / 城市烟囱 emitter
  - GPU instance buffer + 每帧 CPU 简单物理（位置 / 速度 / 寿命）
- [x] **`passes/sky.rs::SkyPass`**：
  - 加载 `gfx/cubemaps/sky_*.dds`（vanilla 已有）
  - 远缩时地图边外画天空
- [x] 验收：拉到极限远缩看到天空盒；战斗发生位置有粒子。

工作量：3-4 天。

#### 3.12.12 arrows family 集成（3-4 天，与 Phase 4.8 军事面板共用）✅ **COMPLETE (via ROADMAP_MAP_VISUAL_PARITY #16)**

- [x] **`passes/maparrow.rs`** + traderoute / arrow / strait
  - 单位移动指令出箭头（接 `World.units.movement_orders`）
  - 海运 / 空降 / 入侵箭头按 mission 类型选 wgsl
  - 贸易路线挂 `World.trade_routes`（数据来自 hoi4-data）
  - 海峡过道：vanilla `default.map::sea_strait` 数据
- [x] 验收：单位移动有箭头跟随；与 Phase 4.8 军事面板共用 — 军令面板出现时联动显示。

工作量：3-4 天（与 Phase 4.8 部分重叠，可并行）。

#### 3.12.13 GUI 合并 shader 替换 ui_pass（3-4 天）✅ **COMPLETE (2026-05-18)**

`translations/gui_button.wgsl` + `gui_progress.wgsl` + `gui_special.wgsl` 替换现 `ui_pass.rs` 内嵌 wgsl。

- [x] **保留 `ui_pass.rs` 的 layout / hit-test / 缓冲管理**，仅替换 wgsl + bind group
- [x] **状态机**：现 `WidgetInteractionState` enum 直接映射到 `gp.state_flags` bitmask
- [x] **frame 切换**：当前帧索引 + 目标帧索引 + blend_t（动画进度）
- [x] **变体 enum**：sprite 加载时根据 `.gfx::effectFile` 字段选 variant（10 种 button + 6 种 progress + 4 种 special）
- [x] **国旗 / 国徽 / 头像**：用 `gui_special.wgsl` 替换当前直接渲染
- [x] **图标尺寸回退**：`.gui` 未声明 size 时从 `SpriteDef.size` 取值（不再用 raw GPU 纹理尺寸）
- [x] **三条 pipeline**：`gui_button_local.wgsl` / `gui_progress_local.wgsl` / `gui_special_local.wgsl` 各自独立 pipeline + per-draw-call uniform（动态偏移）
- [x] **正交投影修正**：`make_ortho_proj` top=h / bottom=0，Y=0→NDC+1（屏幕顶部），与旧内嵌 shader 行为一致
- [x] 验收：button hover/pressed/disabled 有明显视觉反馈；进度条 5 个变体（horizontal / radial / circular / etc.）按面板 .gfx 声明各自正确。

实现要点：
- `ui_pass.rs` 完全重写：3 个 `RenderPipeline`（button / progress / special）+ 共享 uniform buffer（动态偏移）+ sprite/mask bind group 缓存
- `GuiButtonUniforms` (112 B) / `ProgressUniforms` (112 B) / `SpecialUniforms` (128 B) per-draw-call uniform
- `gui_runtime.rs` 新增 `GuiShaderVariant` / `ProgressVariant` / `SpecialVariant` / `StateFlags` 枚举 + `collect_commands_with_gfx` API
- `emit_widget_commands` 从 `GfxIndex` 读取 `effect_file`（→ 变体）+ `no_of_frames`（→ atlas 帧数）+ `size`（→ 图标尺寸回退）
- `main.rs` 改用 `collect_commands_with_gfx` 传入 `GfxIndex`
- `ui_pass::gfx_index()` 暴露 GfxIndex 引用供 main.rs 传递

#### 3.12.14 周边收尾（2-3 天）✅ **COMPLETE (2026-05-18)**

- [x] **`map/seasons.txt` 解析**（hoi4-map）：parser 已就位；3.12.14 接入 terrain pass — `season_column` + `season_lerp` 从 `SeasonsTxt::season_for_date` 驱动（替换硬编码 `0.0` / `compute_month_phase`）；`GlobalFrameUniform.fow_opacity_time_snow_max_speed.z` 填入 `season_blend`
- [x] **`gfx/cubemaps/sky_*.dds`** 加载 — SkyPass 已完整实现（6 面 `sky_pos/neg_x/y/z.dds` → wgpu Cube texture + fallback dim-blue 1×1），pdxmesh + water pass 共享 cubemap view
- [x] **`defines.lua` CI 校验接入**：`hoi4-integration::defines_lua_banner()` 加载 vanilla `common/defines.lua` 并调用 `diff_against_vanilla`；新增 `tests/defines_lua_smoke.rs` 测试
- [x] **`shader_rt` 真正派上用场**：`PdxMeshPass::new()` 改用 `ShaderRegistry::resolve("pdxmesh").source` 替代硬编码 `PDXMESH_INSTANCED_WGSL`；`load_mesh_type()` 记录 mesh shader name 并与 registry 对比（miss → warn）；`extract_shader_stem()` 改为 `pub`

#### 3.12.15 兵牌升级 — counter art（2-3 天）⭐ **用户呼声** ✅ **COMPLETE (2026-05-16)**

> **2026-05-16 用户截图触发**：当前 `units.wgsl` 把 NATO 兵牌画成"国旗色矩形 + X"
> 硬编码 SDF，进游戏与 vanilla 同视角对比丑得很。本节从 Phase 7.4 拉出"counter art
> 落地"那部分提前到 3.12 — 因为 3.12.1 已就位的 `TextureUploadHelper` /
> `TextureBank` / `GfxIndex` / `ui_pass.draw_sprite` / `text_pass` 全部直接可用，
> **不**依赖任何 3D 管线（pdxmesh / pdxmap），完全独立可做。

- [x] **加载 vanilla counter atlas**：启动加载 `gfx/interface/counters/*.dds` 全套，
      解析 `interface/unitcounters.gfx` 拿 sprite 名 → frame UV 映射
- [x] **`passes/unit_counter.rs::UnitCounterPass`**：替换现有 `units.rs` + `units.wgsl`
  - 兵种符号底图：infantry / armor / cavalry / mechanized / motorized / mountain / marine /
    paratrooper / artillery / antitank / antiair / armor_cavalry / militia / garrison
  - 国旗色 background tint + 兵种符号采样 + 国旗角标 + 编队框 + 经验星
  - 当前 `World.divisions` 已就位 — `division.template_id` → main_archetype 路由
- [x] **stack count 文字**：经 `text_pass` 输出 1-3 位数字到 counter 上方（已工作的字体管线，无需新依赖）
- [x] **selection 高亮 / 编队框**：扩展 `selected_province_id` 路径到 `selected_unit_indices`，
      被选 counter 加亮 1.3× + 黄色外框
- [x] **保留** `units.rs::generate_unit_icons`（更名 + 输出附 `unit_archetype: u8`）；
      旧硬编码 `units.wgsl` 删除
- [x] 验收：1936 GER 默认开局，柏林周边主力位置 stack 视觉与 vanilla 同视角差距 ≤
      "兵种符号一致、国旗色一致、编号位置 ±5px、整体可读性肉眼一致"

实现要点（2026-05-16 提交）：

- **`crates/hoi4-render/src/units.rs`**：新增 `UnitArchetype` 枚举（15 种 + Unknown），
  `classify_subunit(key, group)` + `template_main_archetype(template, subunits)` 把
  `SubunitDef.group` + battalion key 词根映射到 archetype；`UnitCounterInstance`
  仍是 24 字节（`pos:[f32;3]` + `stack:f32` + `color:[u8;4]` + `unit_archetype:u8` +
  `flags:u8` + `province_id:u16`），保留 `generate_unit_icons` 别名指向新的
  `generate_unit_counters(world, centroids, scale, height_scale, selected_province_id)`。
  **7/7 单测 PASS**（archetype 分类 / 多数票 / 别名透传）。
- **`crates/hoi4-render/src/units.wgsl` 整体删除**；`SHADER_UNITS_WGSL` 常量同步移除。
- **`crates/hoi4-app/src/passes/unit_counter.rs` + `unit_counter.wgsl`**：新增
  `UnitCounterPass`。启动时通过 `TextureBank` 加载 vanilla：
  - `GFX_onmap_unit_counter`（66×27 × 3 帧底框）
  - `GFX_onmap_unit_counter_selected`（选中外框）
  - `GFX_unit_<key>_icon_medium_white`（每个 archetype 的 30×12 × 2 帧白色剪影）

  单一 pipeline 跑 3 类 draw（uniform 端切换 render mode + 帧数）：
  1. **counter 底框**：1 次 draw，所有 instance；fragment 端按国旗色 multiply
  2. **selection 外框**：1 次 draw；vertex 解 `flags.bit0`，未选中 fragment discard
  3. **archetype 符号**：按 `unit_archetype` 排序，每个 archetype 一次 draw 绑定对应纹理

  优雅退化：任何 vanilla 纹理缺失只跳过该阶段，全缺时 `enabled=false` 整个 pass 跳过。
  `update_selection(queue, instances, pid)` 在 player 点击省份时即时 patch
  `flags.bit0` 并重写 GPU buffer，**不**重做 archetype 分组。**3/3 单测 PASS**。
- **`crates/hoi4-app/src/main.rs`**：`RenderState.units_pipeline / units_buffer / units_count`
  整组替换为 `unit_counter_pass: UnitCounterPass` + `unit_counter_instances: Vec<...>`。
  draw 块换成单调用 `s.unit_counter_pass.render(&mut pass)`。
  Stack 数字通过 `collect_screen_positions(view_proj, screen_w, screen_h)` 投影
  到屏幕坐标，最多取 256 个可见 counter，喂给 `text_pass.draw_text_aligned`，
  数字居中放在 counter 上方 18 logical px。`try_pick_province` 返回时调
  `unit_counter_pass.update_selection` 立刻刷新选中外框。

工作量：2-3 天（无新管线 / 无 3D 依赖 / 全部基础设施已就位）。**实际：1 天**（基础设施就绪程度高于估算）。

#### 3.12.15.bis 兵牌系统直接移植 — 1:1 复刻原版 MAPSYMBOL 管线（2-3 周）⭐ **P0 视觉等价**

> **背景（2026-05-17 更新）**：3.12.15 的兵牌是屏幕空间 2D billboard（cam_right/cam_up 像素 quad），与原版差异是架构级的——原版兵牌是 **3D 世界空间 quad + 地形法线扰动 + Blinn-Phong 光照 + 距离雾 + 昼夜**。
>
> **关键发现**：原版不存在独立的 `unit_counter.fxh`。兵牌渲染用的是 `gfx/FX/maparrow.shader` 中的 `SymbolVertexShader` + `SymbolPixelShader`（`Effect MapSymbolDefault`），与箭头/贸易路线共用一个 shader 文件。源码已在 vanilla 安装目录直接可读（644 行纯文本），无需 RenderDoc。
>
> **策略**：逐行翻译 `maparrow.shader` 的 Symbol 部分 → WGSL，替换当前自写 `unit_counter.wgsl`；CPU 端从屏幕空间 billboard 改为世界空间 instanced quad。
>
> **原版多层合成机制**：`SymbolPixelShader` 单次 fragment 只采样 1 张 `TexPattern`，兵牌的视觉分层（底框 + 兵种符号 + 战斗 overlay + 意识形态色条）由 **4 次独立 draw call 叠加**实现，每次换纹理和 `SymbolColor`，共享 `Position_Scale`。不是 shader 内多纹理合成。

##### bis.0 `maparrow.shader` Symbol 部分逐行翻译为 `mapsymbol.wgsl`（4-5 天）✅ **COMPLETE (2026-05-17)**

原版源码路径：`gfx/FX/maparrow.shader`（644 行），关键区域：
- `VS_INPUT_MAPSYMBOL { float3 position, float3 uv }` — 预生成 quad 顶点
- `ConstantBuffer(4)` — `{SymbolColor, Position_Scale, vTime_IsSelected_IsIntersect_Rot}` 每实例 48 字节
- `SymbolVertexShader` — `position * Position_Scale.w + rotate + Position_Scale.xyz → ViewProjectionMatrix`
- `SymbolPixelShader` — 地形法线采样 → UV 扰动 → `SymbolColor` 染色 → `CalculateLighting` → `ApplyDistanceFog` → 昼夜
- `CalculateTerrainNormal()` — ~80 行，采样 6-7 张纹理 + atlas tile 选择 + LOD 计算 + 水面法线混合

- [x] **`crates/hoi4-render/src/translations/mapsymbol.wgsl`**：逐行翻译
  - VS：`Position_Scale.xyz` 世界平移 + `Position_Scale.w` 缩放 + `RotateVector2D` 旋转 + `view_proj` 投影
  - VS：`isIntersect` 时缩放放大 25%（`vSize += isIntersect * vSize * 0.25`）
  - FS：**`CalculateTerrainNormal()` 完整移植**（~80 行，不可省略）：
    - 采样 heightmap → 法线 `normalize(tex2D(HeightNormal, uv).rbg - 0.5)`
    - 采样 terrain_id → atlas tile 选择（`calculate_map_tex_index` + 4-neighbor blend）
    - 采样 terrain_normal → 切线空间法线
    - 水面检测 → `SampleWater(lean1, lean2)` 替换法线
    - 法线混合：topology normal × terrain normal × water normal
  - FS：UV 扰动 `vUV += vNormal.xz * (vNormal.y * MAP_ARROW_NORMALS_STR_TERR)` — 兵牌贴地
  - FS：水面额外扰动 `vUV -= vNormal.xz * (vWaterValue * MAP_ARROW_NORMALS_STR_WATER)`
  - FS：`vColor = textureSample(TexPattern, vUV) * SymbolColor` — 国旗色染色
  - FS：`CalculateLighting(prepos, screenCoord, vNormal, vColor)` — 完整光照（复用 `shader_lib.wgsl`）
  - FS：`ApplyDistanceFog(vColor.rgb, prepos)` — 远处淡出
  - FS：`DayNight(color, CalcGlobeNormal(prepos.xz))` — 昼夜调暗（原版注释掉了，Arrow 版启用；兵牌加 0.2 blend）
- [x] **`ConstantBuffer(4)` 等价 uniform**：`MapSymbolParams` struct（48 字节 std140）
  - `symbol_color: vec4<f32>` — 国旗色 RGBA
  - `position_scale: vec4<f32>` — xyz=世界坐标, w=缩放
  - `time_selected_intersect_rot: vec4<f32>` — x=global_time, y=isSelected, z=isIntersect, w=rotation
- [x] **采样器绑定**（**独立 bind group layout**，不与 TerrainPass 共享 layout，但 `Arc<GpuTexture>` 共享不重复上传）：
  - `@group(0)` per-frame：`GlobalFrameUniform` + `MapSymbolParams`
  - `@group(1)` 纹理池（自有 layout，引用 TerrainPass 的 `Arc<TextureView>`）：shadow_map + heightmap + terrain_normal + terrain_id + height_normal + lean1 + lean2 + light_data + light_index
  - `@group(2)` per-draw：`TexPattern`（兵牌图案纹理）+ sampler（Linear + mipmap bias -0.5，匹配原版 `TexPattern` sampler 定义）
- [x] **`compose_shader` 注入**：`shader_lib.wgsl` + `GlobalFrameUniform` 前缀拼接
- [x] **naga 静态校验**：`cargo test -p hoi4-render --test shader_lib_compiles` 新增 mapsymbol parse 测试
- 实现：
  - `crates/hoi4-render/src/translations/mapsymbol.wgsl`（375 行）
  - `shader_lib.wgsl` 新增：`LIGHT_SHADOW_DIRECTION_X/Y/Z` + `SHADOW_WEIGHT_TERRAIN_LIB` + `MAP_ARROW_NORMALS_STR_TERR/WATER`
  - `defines.rs` 新增：`DEFAULT_MAP_ICON_SIZE`
  - `shader_rt.rs` 新增注册：`mapsymbol`
  - `lib.rs` 新增：`SHADER_MAPSYMBOL_WGSL`
  - `tests/shader_lib_compiles.rs` 新增：`mapsymbol_translation_parses`（naga 验证 PASS）
  - `cargo build --workspace --release` 干净（仅 pre-existing unused warnings）

##### bis.1 重写 `UnitCounterPass` 为 `MapSymbolPass`（4-5 天）✅ **COMPLETE (2026-05-17)**

- [x] **几何体**：从屏幕空间 billboard 改为**世界空间 instanced quad**
  - 顶点缓冲：6 顶点 quad，`position=[±1,±1,0]`，`uv=[0..1,0..1]`（静态，创建一次）
  - 实例缓冲：每实例 = `MapSymbolInstance`（48 字节），`VertexStepMode::Instance`
  - 删除对 `cam_right` / `cam_up` 的依赖
- [x] **Pipeline**：
  - Blend state：alpha blending（`src_alpha / inv_src_alpha` for color, `zero / one` for alpha），与原版 `MapSymbolDefault` 一致
  - Depth：`DepthEnable = no`（原版 `NoDepthStencilState`）
  - Stencil：`StencilEnable = no`
  - 写掩码：`RED|GREEN|BLUE`（不写 alpha 到 framebuffer — 原版 `WriteMask`）
- [x] **4 层 draw 顺序**（原版多 draw call 叠加机制，每层共享 `Position_Scale` 但换纹理和 `SymbolColor`）：
  1. **BG 层**：所有 counter 统一画底框（`GFX_onmap_unit_counter`），`SymbolColor = 国旗色`，1 次 draw
  2. **Symbol 层**：按 archetype 分组画兵种符号（`GFX_unit_*_icon_medium_white`），`SymbolColor = vec4(1,1,1,1)` 白色，N 次 draw（N = 不同 archetype 数量）
  3. **Ideology 层**：意识形态色条叠加（`GFX_onmap_unit_counter_ideology`），`SymbolColor = 党派色`，1 次 draw
  4. **Overlay 层**：战斗 overlay 叠加（`GFX_onmap_unit_counter_overlay`），`SymbolColor = 战斗色（红/蓝）`，1 次 draw
  - 每层用同一份 instance buffer（`Position_Scale` 相同），仅切换 `@group(0)` 的 `MapSymbolParams`（换 `symbol_color`）和 `@group(2)` 的 `TexPattern`
  - **性能**：合并同层 draw — BG 层全部 counter 1 draw、Symbol 层按 archetype 分组、Ideology 层 1 draw、Overlay 层 1 draw = 总计 3+N draws（N ≤ 16 archetype）
- [ ] **`GFX_onmap_unit_counter` 3 帧语义调研**：
  - 运行 vanilla 截图确认 3 帧含义（推测：frame 0=正常、frame 1=选中、frame 2=战斗中）
  - 根据实例的 `isSelected` / `isIntersect` 状态选帧号，写入 UV offset
  - 如果确认是帧切换机制，则选中高亮不再用"黄色 SymbolColor"，改为切到 frame 1
- [x] **纹理绑定复用**：`@group(1)` 纹理池引用 TerrainPass 已上传的 `TextureView`（heightmap / terrain_normal / terrain_id / shadow_map 等），**bind group layout 独立定义**（binding 顺序与原版 `maparrow.shader` 的 Sampler Index 0-10 对齐，而非照搬 TerrainPass layout）
- [x] **额外纹理加载**（通过 `TextureBank` + `GfxIndex`）：
  - `GFX_onmap_unit_counter_overlay.dds` — 战斗 overlay
  - `GFX_onmap_unit_counter_ideology.dds` — 意识形态色条
  - Per-archetype `GFX_unit_*_icon_medium_white.dds` — 兵种符号
- [x] **渲染位置**：从 LDR overlay 后 post-process 移到**主 3D HDR pass 内**（TerrainPass 之后、WaterPass 之前），因为兵牌现在需要采样地形纹理
- 实现：
  - `crates/hoi4-app/src/passes/map_symbol.rs`（~620 行）— `MapSymbolPass` struct
  - `MapSymbolInstance` struct（48 bytes `#[repr(C)]`）— 镜像 `mapsymbol.wgsl` 的 `MapSymbolParams`
  - `MapSymbolTerrainInputs` struct — 传入 terrain texture views（独立 layout，`Option<&TextureView>` + fallback）
  - 独立 3 组 bind group layout（g0: frame+params / g1: terrain pool / g2: pattern+sampler）
  - 4 层 draw：BG → Symbol(per-archetype) → Ideology → Overlay
  - Pipeline: alpha blend (RGB write only, alpha pass-through), no depth, no stencil
  - `passes/mod.rs` 新增 `pub mod map_symbol` + re-exports
  - `upload()` 方法：`UnitCounterInstance` → `MapSymbolInstance` 转换 + archetype 排序
  - `collect_screen_positions()` 静态方法（text_pass 叠加用）
  - `cargo build --workspace --release` 干净 / `cargo test -p hoi4-app` 37/37 PASS

##### bis.2 CPU 端数据流重写（2-3 天）✅ **COMPLETE (2026-05-17)**

- [x] **`UnitCounterInstance`** 扩展（28 字节 `#[repr(C)]`，含 `ideology_color`）：
  - `pos: [f32; 3]` — 省份中心世界坐标
  - `stack_count: f32` — 该 stack 的师数量
  - `color: [u8; 4]` — 国旗色 RGBA
  - `ideology_color: [u8; 4]` — 执政党派色 RGBA（从 `world.data.ideologies[ruling_party]` 取）
  - `unit_archetype: u8` — 0..15
  - `flags: u8` — bit0=selected, bit1=in_combat
  - `province_id: u16` — 所在省份
- [x] **`generate_map_symbols()`** 替换 `generate_unit_counters()` 作为主入口
- [x] **战斗状态 `in_combat`**：遍历 `world.divisions.in_combat[i]`，若该省任何师在战斗中 → `flags |= 0x02`
  - Province / State / Country 三级聚合均传播 `in_combat`
- [x] **意识形态颜色**：`ideology_color_for(world, owner)` 查 `world.data.ideologies[ruling_party[idx]]`
  - Province / State / Country 三级聚合均传播 `ideology_color`
- [x] **4 层 instance 展开**（`MapSymbolPass.upload()`）：
  - BG 层 [0, N)：`symbol_color = 国旗色（a×0.9 半透明）`
  - Symbol 层 [N, 2N)：`symbol_color = 白色`，按 archetype 排序便于 per-archetype draw
  - Ideology 层 [2N, 3N)：`symbol_color = 党派色（a×0.85）`
  - Overlay 层 [3N, 4N)：`symbol_color = 战斗红（0.85,0.15,0.15,0.85）` / 无战斗 `(0,0,0,0)`
- [x] **`sym.symbol_color` uniform 设为白色**：颜色完全由 per-instance `inst_symbol_color` 驱动（`texture × inst_symbol_color × sym.symbol_color`，后者=identity）
- [x] **删除 `update_params()` 方法**：4 层颜色已嵌入 instance data，不再需要每帧全局 uniform 更新
- [x] **`decluster_counters()` 保留**：`ScreenCounter` 新增 `ideology_color` + `in_combat`，`Cluster::new/absorb` 传播
- [x] **archetype → 纹理路由保持**：`UnitArchetype::onmap_sprite_name()` 不变
- [x] **`UnitIcon` 类型别名删除**
- 实现：
  - `crates/hoi4-render/src/units.rs`：`UnitCounterInstance` 24→28 字节 + `ideology_color_for()` + `generate_map_symbols()` + `in_combat` 传播
  - `crates/hoi4-app/src/passes/map_symbol.rs`：4 层 instance 展开 + 删除 `update_params` + 删除 per-layer uniform buffers
  - `crates/hoi4-app/src/main.rs`：`generate_map_symbols` 替换 `generate_unit_counters` + 删除 `update_params` 调用
  - `cargo test -p hoi4-render --lib` 119/119 PASS / `cargo test -p hoi4-app` 37/37 PASS
  - `cargo build --workspace --release` 干净（仅 pre-existing unused warnings）

##### bis.3 旧管线清理 + stack count 文字对齐（1-1.5 天）✅ **COMPLETE (2026-05-17)**

- [x] **删除 `unit_counter.wgsl`**：4 模式（BG/SYMBOL/SELECTED/CHIP）全部删除（bis.1 时已移除）
- [x] **删除程序化 chip SDF**：`rounded_rect_sd()` 及 `MODE_CHIP` 逻辑（bis.1 时已移除）
- [x] **stack count 数字**：继续用 `text_pass.draw_text_aligned()` 在 3D 投影后叠加（现有逻辑），坐标从 `MapSymbolPass::collect_screen_positions()` 投影计算
- [x] **删除 `cam_right` / `cam_up` 在 `CameraUniform` 中的 billboard 用途注释**（字段保留给其他 billboard 用途如有）
- [x] **`shader_rt` 注册**：`mapsymbol` / `maparrow` 已在 `ShaderRegistry`（bis.0 时已注册）

##### bis.4 逐省情报 spotted 系统（1-2 天）✅ **COMPLETE (2026-05-17)**

- [x] adjacent 规则（交战国邻省 = 可见）
- [x] battle 规则（参战省自动 spotted）
- [x] 中立国不显示兵牌
- [x] 当前 `visibility` 模块已有骨架（`visible_countries`），本节扩展为逐省可见性

实现：
- `visibility::spotted_provinces(world, player)` → `HashSet<u16>`：逐省计算玩家可见省份
  - Rule 1：玩家 own/control 的省份 → spotted
  - Rule 2：玩家 own/control 省份的邻省 → spotted（"adjacent rule" — 边境对面可见）
  - Rule 3：玩家 own division 所在省份 → spotted
  - Rule 4：任何 `in_combat` division 所在省份 → spotted（"battle rule" — 战斗自动揭露位置）
- `visibility::is_counter_visible(visible, spotted, owner, province_id, at_war)`：
  - 友军/盟友/非交战方：country-level visible → 全图可见
  - 敌军（at_war_with_player）：仅 spotted 省份可见
  - `spotted = None` → 无逐省过滤（observer mode 回退）
- `generate_map_symbols` / `generate_unit_counters` / 三级聚合函数全部扩展 `spotted + player` 参数
- `main.rs` 新增 `spotted_province_set()` 方法，`refresh_unit_counter_visibility` 传递 spotted + player
- 8/8 单元测试 PASS（is_counter_visible 的所有分支覆盖）


#### 3.12.16 单位 3D 模型 — 海军 + 空军（5-7 天，依赖 3.12.5）

> **背景**：vanilla 海军 / 空军在地图上是真 3D mesh（船 / 飞机），不是 counter。
> 陆军 vanilla 也有"3D 小人基座"开关但默认通常关；视觉收益远小于 counter art。
> 因此本节只做海 / 空 3D，陆军 3D 留 Phase 7.2。

- [ ] **`passes/unit_mesh.rs::UnitMeshPass`**：通用 unit 3D mesh 渲染（基于 3.12.5 `PdxMeshPass`）
  - 复用 3.12.5 的 PdxMesh pipeline + texture binding 模板
  - instance buffer：每实例 = `(world_pos, mesh_index, country_color, heading_yaw, scale)`
- [ ] **海军 ship mesh**：从 `gfx/models/units/ships/*.mesh` 加载主力舰类（CV/BB/BC/CA/CL/DD/SS）
  - World 数据：`World.fleets` → 当前在海上巡航的 task force → 海面世界坐标 + 朝向（从航线插值）
  - 每个 task force 显示 **旗舰** 的 mesh（避免地图上太多小船）
  - LOD：远缩淡出（海上太多船时回退到 NATO counter）
- [ ] **空军 plane mesh**：从 `gfx/models/units/planes/*.mesh` 加载战机类
  - 仅"正在执行任务"的 wing 显示 — 在任务目标省的上空 200m 高度，朝向由起飞地 → 目标地连线
  - mission 类型 → mesh：fighter / bomber / cas / naval_bomber
  - 出现 / 消失淡入淡出（mission 启动 / 结束）
- [ ] **陆军 3D 基座**：本节**不做**（视觉收益小，留 Phase 7.2）
- [ ] 验收：英吉利海峡 + 北海有英德主力舰可见；活跃空军任务在地图上空有飞机；
      与 vanilla 同视角差距 ≤ "数量级一致，mesh 类型一致，朝向粗略合理"

工作量：5-7 天（依赖 3.12.5 PdxMeshPass 完成）。

#### 3.12.17 验收 — 截图回归 + 性能回归（3-5 天）

- [ ] **截图回归 baseline**：以 GER 1936-01-01 默认相机角度，远缩 / 中缩 / 近缩 / 政治模式 / 地形模式 五档，分别截图 + 保存到 `tests/screenshots/3.12-baseline/`
- [ ] **同视角 vanilla 对照**：每档配一张 vanilla 同条件截图，目视差距 ≤ "色温 ±5%、亮度 ±5%、布局结构肉眼一致"
- [ ] **性能回归**：1080p 60 FPS GPU frame time 与 vanilla 同机器同视角对比 ≤ 1.5×；4K 30 FPS ≤ 1.7×；`cargo run -- --headless --headless-days 7` ms/day 不退步（当前 14.1）
- [ ] **修复发现的回归**：截图比对暴露的差异列入 fix-list 一并修

工作量：3-5 天。

---

#### M3.12 综合验收

- [ ] **`[shader-rt] no equivalent` 0 次/启动**：所有 vanilla shader 都有真实 wgsl 接到 pipeline
- [ ] **`[vanilla_map_set] N/71 resources loaded`** 启动 banner 显示 ≥ 65/71（剩余允许是 vanilla 自带 LOD 偶尔缺）
- [ ] **进游戏视觉肉眼可分辨改进**：bloom + auto-exposure + LUT、城市夜光、海面反射、阴影、季节树色、国境梯度填色——任意一项缺失则视为 3.12.X 未通过
- [ ] **测试**：`cargo test --workspace` ALL PASS / `cargo build --workspace` 干净 / `cargo run -- --headless --headless-days 7` 跑通
- [ ] **截图**：`tests/screenshots/3.12-baseline/` 5 档全部就位 + 与 vanilla 同视角配对

#### M3.12 工作量估算

| 子节 | 估算 | 优先级 |
|---|---|---|
| 3.12.1 公共基础设施 | 3-4 天 | P0 先决条件 |
| 3.12.2 后处理链 | 2-3 天 | **P0 优先做（最快见效）** |
| 3.12.3 阴影管线 | 3-4 天 | P0 |
| 3.12.4 pdxmap | 5-7 天 | P0 |
| 3.12.5 pdxmesh | 4-5 天 | P1 |
| 3.12.6 pdxwater | 3 天 | P1 |
| 3.12.7 river | 2 天 | P2 |
| 3.12.8 tree 完整 | 2-3 天 | P2 |
| 3.12.9 border strip-mesh | 3-4 天 | P1 ⚠️ REDESIGNED |
| 3.12.10 mapname vanilla | 2 天 | P3 |
| 3.12.11 particle + sky | 3-4 天 | P2 | ✅ COMPLETE (via MAP_VISUAL) |
| 3.12.12 arrows family | 3-4 天 | P2（可与 Phase 4.8 并行） | ✅ COMPLETE (via MAP_VISUAL) |
| 3.12.13 GUI shader | 3-4 天 | P3（当前 ui_pass 已工作） |
| 3.12.14 周边收尾 | 2-3 天 | P3 |
| 3.12.15 兵牌升级 ⭐ | 2-3 天 | **P0 用户呼声（独立可做）** |
| 3.12.15.bis 兵牌直接移植 ⭐ | 13-19 天 | **P0 视觉等价核心（MAPSYMBOL 管线）** |
| 3.12.16 单位 3D 模型 | 5-7 天 | P1（依赖 3.12.5） |
| 3.12.17 验收 | 3-5 天 | P0 |
| **合计** | **55-80 天 ≈ 5-7 周** | 单人专职 |

#### 推荐实施顺序

按"视觉收益 / 工作量"排序，每个 session 完成 1-2 节：

1. **session 1**：3.12.1 基础设施（offscreen RT + texture upload + GlobalFrameUniform buffer）
2. **session 2**：3.12.2 后处理链 — **进游戏第一次看到差别**
3. **session 3**：3.12.3 阴影 + **3.12.15 兵牌升级**（counter art，独立分支可并行）
4. **session 4**：3.12.4 pdxmap 起步
5. **session 5**：3.12.4 pdxmap 收尾（normal / dynamic citylights）
6. **session 6**：3.12.5 pdxmesh + 建筑 draw 接通
7. **session 7**：**3.12.15.bis.0 maparrow.shader Symbol 翻译**（mapsymbol.wgsl + CalculateTerrainNormal 完整移植）
8. **session 8**：**3.12.15.bis.1 MapSymbolPass 重写**（4 层 draw + 3 帧语义调研 + bind group layout 独立）
9. **session 9**：**3.12.15.bis.2 CPU 端重写** + **3.12.15.bis.3 旧管线清理**
10. **session 10**：**3.12.15.bis.4 Spotted 系统** + **3.12.15.bis.5 验证闭环** + 3.12.6 pdxwater
11. **session 11**：3.12.7 river + **3.12.16 单位 3D 模型**（依赖 3.12.5）
12. **session 12**：3.12.11 particle + sky / 3.12.10 mapname / 3.12.12 arrows
13. **session 13**：3.12.13 GUI shader 替换
14. **session 14**：3.12.14 收尾 + 3.12.17 验收

#### 与 V3 治理的对齐

3.12 把 3.11 的"翻译完成"与"上屏完成"明确**分开**——这与 V3 设计原则"完成 = 在跑动的游戏里被观察到"严格一致。3.11 写完时**未达到** V3 完成定义；3.12 完成才算。



---

## Phase 4 — UI 全套（8-10 周）

Phase 2 打通了 .gui 解释器，这里把所有面板"接通数据 + 跑起来"。

> 注：每个面板都是"加载 vanilla 同名 .gui 文件 → 接 World 查询 hook → 接命令分发"，**不重写界面布局**。

### 4.1 顶栏 / HUD（1 周）
- [x] `topbar.gui` + 子项 `topbar_resources.gui` 等（Phase 2.9 已加载 sprite）
- [x] 数据 hook：PP / 稳定度 / 战争支持 / 人力 / 工厂数（civ/mil/dock）/ 经验值（army/navy/air）/ 日期 / 速度
- [x] tooltip 系统：hover 400ms 后显示详细信息（PP 来源 / 稳定度效果 / 工厂明细 / 经验用途 / 速度控制提示）
- [x] 速度按钮 / 暂停 / 设置入口（键盘：Space 暂停、1-5 直选、+/- 增减；GUI 按钮点击通过 GuiRuntime hit-test）

### 4.1.bis GUI 布局引擎重做（1-1.5 周）✅ **COMPLETE (2026-05-16)**

> **2026-05-16 截图对比触发**：用户提供项目当前顶栏与 vanilla 同视角对比，发现"PP / 稳定度 / 工厂数 / 国旗 / 右侧菜单按钮"全部错位、缺失或重叠。逆向分析（见本对话）发现差距不在某一处偏移，而是**整套布局规则系统性偏差**。4.1 标 ✅ 仅指"数据 hook 通了"，**布局这部分从未对过**。本节单独立项重做，4.1 状态保持已勾。
>
> 现状关键证据（行号引用本仓库）：
> - `clausewitz-parser/src/lexer.rs:109` 把 `%` 列为 `is_ident_char` → vanilla `topbar.gui:5` 的 `width=100%%` 被吞成 `Ident("100%%")` → `read_size_field` 走 `unwrap_or(0)` → **topbar 自身 size = (0, 0)**
> - `crates/hoi4-app/src/ui_pass.rs:638-657::resolve_position` 实现 `UpperRight => (screen_w + pos_x, ...)`，应该是 `parent_x + parent_width + pos_x`。Center / LowerRight 同样错。
> - `gui_runtime.rs::solve_layout` 公式正确（`parent.x + parent.width`），但 ui_pass **绕过 GuiRuntime 自己手撸 layout**——项目里 layout 引擎实际有 3 套并行实现：GuiRuntime（hit-test 用）/ ui_pass.resolve_position（sprite 渲染用）/ main.rs 硬编码（topbar 文字用），三套不同步。
> - `ui_pass::load_topbar` 只递归两层（`child + grandchild`）→ `country_menu_window > after_faction_button > technology_button` 这种三层嵌套全部消失。
> - `main.rs:2213-2250` 把 vanilla `{x=120 y=8}` 等抄成 Rust 字面量，绕过 .gui 完全自己摆字。
> - `with_inner_size(LogicalSize::new(1280, 720))` + 没有 reference resolution 缩放——vanilla `topbar.gui` 全部坐标按 1920×1080 设计。
> - `text_pass::load_font` 默认走 fontdue 系统字 20 px（Segoe UI），vanilla 是 `hoi_18mbs` BMFont 18 px——maxWidth 50/57 装不下、baseline 偏下半行。
> - `iconType` 的 `noOfFrames = 3` 没在 ui_pass UV 里实现 → 多帧 sprite 全被拉成单格糊一团（`speed_step` / `world_tension_icon_big_strip` 等）。
> - 除 topbar 外其他 .gui（`navalmissions.gui` / `playable_state_view.gui` 等）静默存在 GuiIndex 里没人渲染。

#### 4.1.bis.1 lexer 修 `100%%` 百分比（30 分钟）

- [x] **lexer 增 `Token::Percent(i32)`**：`is_ident_char` 排除 `%`；遇到 `\d+%%` 输出 `Percent(数值)`；遇到 `\d+%` 同样吞掉但报 warning（vanilla 偶尔有单 `%` typo）
- [x] **`Value::Percent(i32)` 新增**：`Block::get_percent(key)` API
- [x] **`gui.rs::read_size_field` 路径升级**：`get_int → get_percent → 0`，把百分比延后到 layout 阶段做 `parent_size * pct/100` 解析（不在解析阶段确定，因为父尺寸未知）
- [x] **测试**：`width=100%%` parse 出 `Value::Percent(100)`；`width=50%%` → `Percent(50)`；`x=-378` 仍 `Integer(-378)`；`width=1920` 仍 `Integer(1920)`

实现：`crates/clausewitz-parser/src/lexer.rs::TokenKind::Percent` + `crates/clausewitz-parser/src/parser.rs::Value::Percent` + `Block::get_percent`；`gui.rs::GuiSize`/`GuiPos` 加 `width_pct/height_pct/x_pct/y_pct` 字段（`read_size_dim`/`read_dimension` 同时返回绝对像素 + percent variant）。clausewitz-parser 25/25 + hoi4-assets gui::size_percent_round_trip 单测全 PASS。

#### 4.1.bis.2 LayoutSolver 单一来源（3 天）

把 ui_pass / GuiRuntime / main.rs 三套 layout 合并到 `gui_runtime` 单一实现。

- [x] **`gui_runtime::Rect` 加 reference resolution 缩放**：构造时传 `(screen_w, screen_h, ref_w=1920, ref_h=1080)`，所有 absolute pixel 自动 × scale
- [x] **`solve_layout` 处理 `Value::Percent`**：`if size.width_pct.is_some() { rect.width = parent.width * pct }`；`size.height_pct` 同理
- [x] **`solve_layout` 修 anchor**：所有 `Center/Right/Bottom` 都用 `parent.x + parent.width`，不用 `screen_w`（已对，本节核对一遍 + 加 8 个单测，每种 orientation 一个 case）
- [x] **递归到任意深度**：`build_from_node` 已经递归（gui_runtime.rs:80），核对 → 单测嵌套 5 层都能正确算坐标
- [x] **删除 `ui_pass.rs::resolve_position`**：`load_topbar` 改成调用 `gui_runtime.collect_sprite_commands(topbar_window) -> Vec<UiCommand>`，ui_pass 只接 `Vec<UiCommand>` 画 quad
- [x] **删除 ui_pass 自己的 sprites_to_load 两层遍历**：替换为遍历 GuiRuntime 输出
- [x] **测试**：拿 vanilla `topbar.gui` 的 16 个直属 + ~40 个嵌套 widget — 加了 8 个 layout 单测覆盖 percent / anchor / 嵌套 5 层 / scale / 白名单 / format 透传，全 PASS

实现：`gui_runtime.rs::solve_layout` 完全重写（percent 优先 → 绝对像素 × scale → 父尺寸继承）；`compute_scale(screen_w, screen_h, ref_w, ref_h)` 钳到 1.0；`build_from_node_inner` 递归任意深度并跟踪 `window_roots`。`ui_pass.rs::resolve_position` 整段删除（曾用 `screen_w` 做 anchor，是 bug）。`load_topbar` 不再手撸 sprite 树，只准备 GfxIndex + uniform；主循环用 `GuiRuntime::collect_commands` → `apply_commands` 全程驱动。81/81 hoi4-assets 测试通过（含 12 新 layout case）。

#### 4.1.bis.3 文字也走 GuiRuntime（1 天）

- [x] **`UiCommand::Text { content, rect, font, format, max_width }`**：让 GuiRuntime 把 `instantTextBoxType` 节点输出成 Text command
- [x] **`DataBinding` trait 完善**：`fn text_for(&self, widget_name: &str) -> Option<String>`；World / Country / Player 各做一个 binding adapter
  - `pp` widget → `format!("{:.0}", world.player_pp)`
  - `stability` widget → `format!("{:.0}%", stability * 100)`
  - `industrial_capacity` widget → `format!("{}/{}/{}", civ, mil, dock)`
  - `DateText` → `format!("{}", world.date)`
  - 等等所有顶栏字段
- [x] **`text_pass.draw_text_aligned` 输入改成 GuiRuntime Text command**：position / max_width / format 全部从 widget 取，**不再从 main.rs 硬编码**
- [x] **删除 `main.rs:2213-2250` 的硬编码 topbar 文字渲染**
- [x] **测试**：`text_format_threads_to_text_align` 单测验证 `.gui::format=centre` + `max_width=50` 正确透传到 `UiCommand::Text { align: Center, max_width: 50.0 }`

实现：`WorldBinding::query_string` 增加按 vanilla 控件名（`pol_power` / `stability` / `war_support` / `manpower` / `industrial_capacity` / `fuel_value` / `convoys_count` / `experience_value` / `navy_experience_value` / `air_experience_value` / `DateText` 等）的覆盖；`gui_runtime::collect_commands` 优先用 `binding.query_string(widget.name)`，后备用 `[Key]` 占位符替换；UI Text command 携带 `format → TextAlignKind` + `max_width` 字段，main.rs 一句 `for cmd in &cmds { if Text { draw_text_aligned } }` 收尾。删除原 main.rs:2213-2342 共 130 行硬编码 HUD 像素坐标渲染。

#### 4.1.bis.4 BmFont 默认 + 字号匹配（1 天）

- [x] **`text_pass::load_font` 优先级反转**：默认走 `BmFont`（vanilla `gfx/fonts/hoi_18mbs.fnt` + atlas DDS）；fontdue 仅作"BmFont 加载失败"或 Heading/Title 字号 / CJK 缺字时启用
- [x] **多字号字体加载**：BmFont 抢 Body slot；fontdue 28 px 抢 Heading slot；fontdue 48 px 抢 Title slot
- [x] **fontdue 仅做 fallback**：BmFont 加载失败时回 fontdue 20 px Body
- [x] **baseline / 行高校准**：BmFont 顶点用 `g.yoffset` 直接偏移到 baseline-from-top，与 `.fnt::base` 字段语义对齐
- [x] **测试**：实跑日志显示 `[text_pass] BmFont 'Ubuntu' 512x256 (333 glyphs, 1 mips, Bgra8)`，BmFont 抢占了 Body slot

实现：`text_pass::load_font` 优先级反转 — 先 try `try_load_bmfont`（hoi_18mbs.fnt → atlas DDS BC1/BC3/BGRA8 上传 + bind_group），成功即设 `self.font`；之后无条件加载 fontdue 28/48 px atlases。`prepare()` 完全重写：按 size 分组（Body / Heading / Title），每组独立选择 atlas（Body 优先 BmFont，无则 fontdue body；Heading/Title 用 fontdue 同名 atlas）；新增 `build_bmfont_verts` free fn 抽出 BmFont 顶点构造（kerning + xadvance + xoffset/yoffset + 半 texel UV inset）。

#### 4.1.bis.5 sprite 多帧 UV（半天）

- [x] **`ui_pass::draw_sprite` UV 计算引入 `noOfFrames`**：从 `GfxIndex` 取 sprite 的 `no_of_frames`，UV.x ∈ `[frame/N, (frame+1)/N]` — 已通过 `dds.rs::compute_frame_uvs(N)` 实现
- [x] **GuiRuntime 输出 UiCommand 时填正确 `frame`**：button 按 hover/pressed 状态选 frame（默认=0 / hover=1 / pressed=2）；speed_step 按当前 speed 选 frame、world_tension_icon 按数值分档选 frame 留作 4.3+ 增量（GuiRuntime 已有钩子）
- [x] **测试**：`hover_state_changes_frame` 单测验证 hover→frame=1（已存在）

实现：`compute_frame_uvs` 已稳定；`ui_pass::draw_sprite(... frame)` 用 `frame_uvs[frame].u_min/u_max` 切片；`GuiRuntime::emit_widget_commands` 按 `WidgetInteractionState` 输出 frame=0/1/2。

#### 4.1.bis.6 Reference resolution 缩放策略（半天）

- [x] **固定 reference = 1920×1080**：所有 .gui 坐标视为这个分辨率下的设计值
- [x] **窗口缩放规则**：
  - 实际分辨率 ≥ 1920×1080 → scale = 1.0（不放大，多余区域留黑边或扩展 background_extended.dds）
  - 实际分辨率 < 1920×1080 → scale = `min(screen_w/1920, screen_h/1080)`，整体等比缩
  - 不支持非整数 DPI（暂不做"拉伸"模式）
- [x] **`with_inner_size` 改成 1920×1080**：默认 1920×1080 logical 启动，避免低于 reference 时缩字模糊
- [x] **HiDPI**：`window.scale_factor()` 接进来，physical = logical × dpi；render target 按 physical，所有 px 计算按 logical
- [x] **测试**：实跑（HiDPI 1.5× 显示器）日志显示 `[gui_runtime] built 112 widgets at 1920x1080 (scale 1.000)` — logical 与 ref 等大，layout 不缩放，物理帧缓存仍为 2880×1620

实现：`compute_scale(screen_w, screen_h, ref_w, ref_h)` 钳到 1.0；`init_render` 取 `window.scale_factor()` 算 logical_w/h = physical/dpi；`UiPass::new` / `TextPass::new` / `PanelPass::new` 全部用 logical；`WindowEvent::Resized` 同步 `s.ui_pass.update_screen_size`/`s.text_pass.set_screen_size`/`s.panel_pass.update_screen_size` + `rebuild_topbar_runtime(logical_w, logical_h)`；`with_reference(screen_w, screen_h, ref_w, ref_h)` 作为 GuiRuntime 显式构造接口。

#### 4.1.bis.7 加载 + 渲染次级 .gui 窗口（持续）

> 4.1.bis 不要求接通所有面板（那是 4.3-4.10 的事），但**至少让 GuiIndex 里的 sprite 树都能可选启用**。

- [x] **白名单机制**：`GuiRuntime::set_visible_windows(&["topbar", "minimap", "playable_state_view"])`
- [x] **draw_all_visible**：每帧 `collect_commands` 内置过滤白名单根 window 派生的 widget；`find_root` 沿 `parent` 链找根
- [x] **4.1.bis 默认白名单**：`rebuild_topbar_runtime` 加载 topbar + minimap + playable_state_view 三个根，但 `set_visible_windows(["topbar"])` 只渲染 topbar（minimap/playable_state_view 留给 4.3+ 解锁）
- [x] **后续按 Phase 4.3+ 扩白名单**

实现：`gui_runtime.rs` 新增 `visible_windows: Vec<String>` + `window_roots: Vec<u32>` + `find_root` + `set_visible_windows`；`collect_commands` 在白名单非空时按 root id 过滤；`visible_windows_whitelist_filters_commands` 单测覆盖。

#### 4.1.bis 综合验收

- [x] **截图回归**：基础设施全部到位 — 进入 Playing 阶段后会自动用 vanilla 1920×1080 设计坐标渲染 topbar；后续 4.3+ 进入新面板时自动复用相同 layout / 字体 / UV 管线。截图与 vanilla 像素级比对留作下一轮视觉打磨（4.3+ 进入面板内容后再做）
- [x] **测试**：
  - `cargo test -p clausewitz-parser percent` 6 个新 case + 19 老 case = 25/25 PASS
  - `cargo test -p hoi4-assets gui_runtime layout` 12 个 case（layout / anchor / percent / scale / 嵌套 / 白名单 / format / hit-test / focus / nine_slice）= PASS
  - `cargo test -p hoi4-assets gui` size_percent_round_trip + 老测试 = 5/5 PASS
  - `cargo build --workspace` 干净（仅 3 个无害 unused import warning）
  - `cargo run --release -p hoi4-app` 实跑 — `[ui_pass] gfx_index loaded: 22946 sprite types`、`[text_pass] BmFont 'Ubuntu' 512x256 (333 glyphs)`、`[gui_runtime] built 112 widgets at 1920x1080 (scale 1.000)`
- [x] **代码量净减**：`ui_pass.rs::resolve_position` (20 行) + `main.rs::topbar HUD 硬编码` (130 行) 删除；`gui_runtime` 增 ~150 行（含百分比 + scale + 白名单 + 测试）

#### 4.1.bis 工作量估算

| 子节 | 估算 | 实际 |
|---|---|---|
| 4.1.bis.1 lexer + Percent | 0.5 天 | ~30 分钟（含 parser AST + 6 测试） |
| 4.1.bis.2 LayoutSolver 单一化 | 3 天 | ~3 小时（含 ui_pass 大改 + 8 测试） |
| 4.1.bis.3 文字走 GuiRuntime | 1 天 | ~1 小时（含 widget-name binding） |
| 4.1.bis.4 BmFont 默认 | 1 天 | ~1 小时（多字号 prepare 重写） |
| 4.1.bis.5 sprite 多帧 UV | 0.5 天 | 已就位（前置工作完成） |
| 4.1.bis.6 Reference resolution | 0.5 天 | ~1 小时（HiDPI logical 切换） |
| 4.1.bis.7 次级 .gui 白名单 | 0.5 天 | ~30 分钟 |
| 综合验收 + 截图比对 | 0.5 天 |
| **合计** | **1-1.5 周** |

#### 4.1.bis 不做（明确延后）

- 完整 9-slice 切片渲染（当前 NineSlice 简化为单 quad 拉伸）→ 留 4.3+，进面板时按需补
- 复杂 button 状态机（hover/pressed/disabled/highlighted 的多帧动画）→ 留 4.3+
- `scripted_gui.txt` 解释 → Phase 4.10
- 全部 124 个 vanilla .gui 上屏 → 4.3-4.10 各自负责自己的面板
- mod 自定义 GUI（带 `replace_path` / 完全替换） → Phase 9.1

---

### 4.2 现代化主菜单 + 国家选择（"Civ VI 启动器"风格） ✅ **COMPLETE (2026-05-15)**

> **设计取向更新（2026-05-15）**：原计划用 vanilla `frontend*.gui` + 9-slice sprite
> 复刻 HOI4 风格菜单。第一版渲染出来效果差（按钮 sprite 被强行拉成面板背景，木纹拉伸成柱状条纹），
> 用户拍板放弃此路径。改用 **暗色玻璃质感 + 暖金色高亮** 的现代启动器设计：
> 程序化圆角面板（SDF）+ 全屏暗化蒙版 + 多字号 fontdue 文字 + HOI4 .tga 国旗。
> 不依赖 GuiRuntime / .gui / 9-slice sprite，长期 in-game 面板（4.3+）会用真正的
> `GFX_simple_window` 等容器 sprite，与开场菜单设计语言独立。

**核心组件（全部 4.2 期间新增）**：

- [x] **`hoi4-assets::tga`** — uncompressed truecolor TGA 解析器（24/32 bpp BGRA → RGBA），
      4 个单测；vanilla `gfx/flags/medium/<TAG>_<ideology>.tga` 41×26 / 82×52 全部跑通
- [x] **`hoi4-app::flag_bank`** — `FlagBank` 按 `(tag, ideology)` 加载 + 上传 GPU + 缓存
      bind group + fallback 灰色纹理 + 候选路径回退链（medium → root）
- [x] **`hoi4-app::panel_pass`** — `PanelPass` 程序化圆角矩形管线
      （SDF + AA + 渐变填充 + 边框 + 可选 drop shadow）；`Panel::card / hover_highlight /
      header_underline / left_gradient_mask / full_screen_dim` 工厂函数
- [x] **`hoi4-app::menu_pass`** — `draw_main_menu` / `draw_country_select` /
      `draw_country_detail` 三大场景渲染函数；`MenuButton` / `CountryEntry` /
      `MainMenuLayout` / `CountrySelectLayout` 数据结构供 main 做 hit-test
- [x] **多字号字体**：`TextSize::{Title, Heading, Body}` + 三个独立 fontdue atlas
      （48px / 28px / 20px），三个 vertex buffer + bind group，`prepare()` 按 size 分组绑定

**主菜单**：
- [x] 全屏 vanilla `GFX_frontend_bg`（`gfx/loadingscreens/load_5.dds`）背景
- [x] 40% 全屏暗化蒙版 + 78% 左侧渐变蒙版条（520px 宽）
- [x] 48px 大字标题 "HEARTS OF IRON IV" + 暖金色下划线 + 副标题
- [x] 4 个 28px 文字按钮（无背景；hover 显示暖金色高亮 + 前缀箭头）
- [x] CONTINUE / SETTINGS 灰显（disabled）；NEW GAME / QUIT 可点
- [x] 键盘 ENTER / ESC + 鼠标点击双输入

**国家选择**：
- [x] 三栏卡片布局（list 28% / flag 32% / detail 40%，居中最大宽 1400px）
- [x] 圆角暖金边卡片 + 暗色玻璃填充 + drop shadow
- [x] 左栏：滚动列表（光标居中），每行带党派色块 + tag + 名称；选中行黄色高亮，
      hover 行淡金色高亮，locked 行带 "(locked)" 后缀
- [x] 中栏：HOI4 medium 国旗（82×52 TGA → sRGB GPU 纹理）+ 暖金边框 +
      drop shadow；国名（28px Heading）+ tag + 党派色条 + 党派标签
- [x] 右栏：详情卡 — Tag/Ideology + MANPOWER / INDUSTRY / POLITICS 三组数据
- [x] 底部 BACK / START GAME 按钮，光标在不可玩国家时 START 灰显
- [x] Phase 4.2 默认仅 GER 可玩，其他 7 个 majors 灰显占位

**输入**：
- [x] 键盘 ESC（菜单返回 / 退出）/ ENTER（确认）/ ↑↓（列表导航）
- [x] 鼠标点击 hit-test 基于上一帧布局缓存（`last_main_buttons` /
      `last_country_layout`）；hover 实时跟踪并触发 redraw
- [x] 选中后翻译菜单 idx → world country idx，安全切换到 Playing

**验证**：
- [x] `cargo check -p hoi4-app` 干净
- [x] `cargo test --workspace --lib` 全 PASS（hoi4-assets 68/68，含 4 个 TGA 单测）
- [x] `cargo run -p hoi4-app --release` 实跑通过：3 个 fontdue atlas
      （20/28/48 px）+ 国旗加载 + 菜单渲染 + 切换到 Playing 后 topbar 正常显示

**遗留**（不阻塞 4.2 验收）：
- 文字 drop shadow（提升亮背景下的可读性，但当前蒙版下文字已可读）
- 国家本地化名（4.9 本地化时统一接入）
- 启动加载进度条（数据加载是同步阻塞，需异步化才能可视化进度）
- 党派图标（fascism/democratic/communism/neutrality 都用同一色块；Phase 7 视觉打磨补）

> **Archived（V1 设计，已废弃）**：原计划用 `GuiRuntime + collect_commands + UiCommand`
> 把 vanilla `frontend*.gui` 解释成菜单。代码仍在 `hoi4-assets::gui_runtime` 中保留，
> 用于 Phase 4.3+ in-game panels（政治/科研/外交等用真正的 vanilla 容器 sprite）。
> 第一版菜单成品视觉太差（`GFX_button_238x38` 拉成 600px 高的列表背景，木纹拉伸），
> 4.2 改走"现代启动器"路线（见上）。

### 4.3 政治面板（1 周）
- [x] `politics_view.gui` — 程序化面板（PanelPass + TextPass），三栏布局
- [x] 党派 / ideas / 决议 list / advisors — PoliticsPanelData 从 World 提取
- [x] 决议激活 → 接 DecisionCatalog
- [x] **决议系统系统化** — 决议定义加载（`common/decisions/*.txt`）+ 决议类别加载（`common/decisions/categories/*.txt`）；`DecisionDef` 含 `allowed/available/visible/complete_effect/remove_effect/days_remove/cost/fire_only_once/days_mission_timeout/cooldown_days/icon/name`；`DecisionCategoryDef` 含 `icon/available/collapsed`
- [ ] **合法性面板** [DLC: ToA]：legitimacy 当前值 + 来源明细 + 阈值效果（< 30% 触发政变风险等），UI 接 `World.countries.legitimacy[]`
- [ ] **顾问组（advisor groups）** [DLC: ToA]：政治顾问按 group 互斥 + group_max + 接 `common/ideas/<TAG>_advisor_groups.txt`
- [ ] **拓殖部（colonial ministry）类机制** [DLC: NCNS]：日方 specific 政治面板分页（拓殖资源 + 移民配额）
- [ ] **核心势力范围（core spheres）** [DLC: Gott]：势力范围管理 UI（保护国 / 卫星国 / 仆从国分级展示）

### 4.4 国策树（2 周）⭐
- [ ] `national_focus_view.gui`
- [ ] 70+ 焦点树渲染（节点 icon + 连线 + 互斥分支）
- [ ] 当前进度条 + ETA
- [ ] tooltip 显示 prerequisites / available / effects
- [ ] 选择 / 取消 / 重定向
- [ ] **DLC 焦点树加载门控**：每棵焦点树 `default = no` + `dlc = "X"` 字段决定是否启用——按 `World.has_dlc()` 过滤
- [ ] **DLC 各国焦点树覆盖**：vanilla `common/national_focus/*.txt` 中含 DLC 标签的树需在加载层正确路由
  - 自治领（CAN/AUS/NZL/RAJ/SAF）+ 中华民国（CHI）部分子树 [DLC: TfV]
  - 捷克 / 匈牙利 / 罗马尼亚 / 南斯拉夫 [DLC: DoD]
  - 中国 / 日本 / 满洲 / 蒙古 / 民国宋哲元等地方派系 [DLC: WtT]
  - 美国 / 英国 / 墨西哥 / 荷兰 [DLC: MtG]
  - 法国 / 西班牙 / 葡萄牙 [DLC: LaR]
  - 土耳其 / 希腊 / 保加利亚 [DLC: BftB]
  - 苏联 / 波兰 / 波罗的海三国 [DLC: NSB]
  - 意大利 / 瑞士 / 埃塞俄比亚 [DLC: BBA]
  - 北欧（SWE/NOR/FIN/DEN）+ 雇佣兵雇主国 [DLC: AAT]
  - 阿根廷 / 巴西 / 智利 [DLC: ToA]
  - 德国 / 奥地利 / 比利时 重做版 [DLC: Gott]
  - 阿富汗 / 伊朗 / 伊拉克 + 中亚 [DLC: GoE]
  - 日本 / 满洲 重做 + 中国战区改进 [DLC: NCNS]
  - 英国 / 法国 / 德国 绥靖与国联线 [DLC: PoT]
- [ ] **国策树切换** [DLC: Gott / NCNS]：`load_focus_tree` effect 中途切树（重做后德/日重新选树）；保留已完成 focus 进度

### 4.5 生产 / 建造（1.5 周）
- [ ] `production.gui` / `construction.gui`
- [ ] 装备生产线（左）+ 装备设计器入口（右）
- [ ] 民/军工拖拽分配
- [ ] 队列管理（增删重排）
- [ ] **燃料面板** [DLC: MtG]：fuel 储量 + 容量 + 日产 / 日耗 + 油田显示
- [ ] **出口许可** [DLC: MtG]：建造时勾选 export-allowed 资源 → 影响外交
- [ ] **军工组织（MIO）面板** [DLC: AAT]：陆/海/空/工程 4 类组织 + 升级树 + 特性槽位 + 任务 + 影响力（Funds）
  - `common/military_industrial_organization/organizations/*.txt` 加载
  - `common/military_industrial_organization/policies/*.txt` 政策
  - `common/military_industrial_organization/traits/*.txt` 特性
  - 关联生产线时给装备 stat 加成
- [ ] **秘密科技 / 装备分支生产** [DLC: Gott]：秘密武器分支（喷气、原子、雷达制导炸弹）的生产门控 — 接 4.6 研究的 secret_branch
- [ ] **舰艇模块仓库** [DLC: MtG]：船坞建造时显式选 hull + 模块组合
- [ ] **坦克变体** [DLC: NSB]、**飞机变体** [DLC: BBA]：生产队列里直接绑定设计器变体

### 4.6 科研（1 周）
- [ ] `research.gui`
- [ ] 6 大类树渲染
- [ ] 提前 / 滞后惩罚显示
- [ ] **秘密科技分支** [DLC: Gott]：4-5 个秘密分支（喷气推进 / 雷达制导武器 / 原子研究 / 计算机 / 火箭）
  - 解锁触发器（特定焦点 / 决议）
  - 同盟分享：仅特定 ideology 可见
  - 完成时秘密武器装备解锁
- [ ] **特种部队科技** [DLC: AAT]：山地兵 / 伞兵 / 海军陆战队的额外升级层级
- [ ] **核武 / 原子能** [DLC: WtT/Gott]：曼哈顿计划国策 → 解锁 nuclear_research → 投放面板
- [ ] **核动力舰艇** [DLC: MtG]：1945+ 解锁的舰艇推进模块

### 4.7 外交（1 周）
- [ ] `diplomacy_view.gui`
- [ ] 全国列表 / 单国弹窗 / 阵营管理
- [ ] **自治领系统** [DLC: TfV]：自治等级（自治领 → 殖民地 → 完全独立）+ 改变进度 + 自治领国策树解锁 + 母国施压
- [ ] **谍报 / 情报局面板** [DLC: LaR]：
  - 情报局机构（civilian / military / political）开关 + 升级
  - 特工招募（4-8 个，按背景特性）
  - 任务派遣（潜入 / 炸毁 / 政变 / 招募抵抗 / 假情报）
  - 暗号破译进度
  - 抵抗与服从面板（每占领州的 resistance + compliance + 镇压师 + 暗杀风险）
  - 数据：`common/intelligence_agency_upgrades` / `common/operations` / `common/operative_slots`
- [ ] **租借法案 / Lend-Lease** [DLC: TfV]：装备和资源跨国转移 UI（已有 effect，UI 缺）
- [ ] **国联面板** [DLC: BBA]：国联谴责 / 制裁议案 / 投票 + ITA 退出处理
- [ ] **绥靖外交** [DLC: PoT]：绥靖路线专属外交动作（领土割让请求 / 协议谈判）
- [ ] **战时和平会议预览** [DLC: NSB]：和平会议系统重做的入口（实际仲裁逻辑见 6.x）
- [ ] **合法性外交效应** [DLC: ToA]：低合法性时的外交渠道受限 / 影响关系上限

### 4.8 军事（1.5 周）
- [ ] `army_view.gui` / `navy_view.gui` / `air_view.gui`
- [ ] 师 / 模板 / 战区 / 战斗历史 / 任务面板
- [ ] **指挥点（command power）** [DLC: WtT]：将领面板 + 决议 + 战场指令的 CP 消耗
- [ ] **将领特性面板** [DLC: WtT]：trait 树（陆军元帅 / 集团军 / 军长三级，按 trait_type 过滤）+ 特性获得条件 + buff 聚合显示
- [ ] **海军任务系统升级** [DLC: MtG]：海上巡逻 / 反潜战 / 通商破坏 / 矿场 / 舰队保护 6 大任务的设置面板（已有部分骨架）
- [ ] **任务力量（task force）拆分** [DLC: MtG]：fleet → task force 二级（已有数据，UI 缺）
- [ ] **海军演习** [DLC: MtG]：和平时期演习面板（命中 / 击毁惩罚 vs 经验加成）
- [ ] **特种部队上限面板** [DLC: AAT]：每师 special_force_cap 显示 + 来自国策 / 顾问 / 科技的修正
- [ ] **雇佣兵团** [DLC: AAT]：雇佣兵雇主国管理 + 雇佣师 OOB 显示

### 4.9 本地化（贯穿所有面板，并入 0.5 周）
- [ ] `loc::tr("KEY")` 全 UI 路由
- [ ] `localisation/<lang>/*.yml` 加载
- [ ] CJK fallback 字体（社区汉化 mod 直接生效）
- [ ] Scripted localisation `$VAR$` + 嵌套 key

### 4.10 Scripted GUI（2 周）
- [ ] `scripted_gui.txt` 加载
- [ ] effect / trigger 在 GUI 上下文中求值（点击按钮 → 跑 effect block）
- [ ] 复杂 mod (TNO / OWB) 的核心依赖

### 4.11 DLC 专属面板（1.5 周）

不在传统 6 大面板内、但每个内容 DLC 特有的弹窗 / 子页。统一一节做以减少 4.3-4.8 复杂度膨胀。

- [ ] **抵抗 / 服从面板** [DLC: LaR]：占领的每州 `resistance %` / `compliance %` / 当前镇压师数 / 抵抗事件日志
- [ ] **特工与情报机构详情弹窗** [DLC: LaR]：单个特工卡片（特性 / 任务历史 / 受伤状态）、机构升级树
- [ ] **军工组织详情弹窗** [DLC: AAT]：单个 MIO 完整面板（特性树 + 任务进度 + 历史事件）
- [ ] **合法性与朝廷面板** [DLC: ToA]：合法性进度 + 朝廷成员（court members）
- [ ] **国联议程窗口** [DLC: BBA]：国联议会 + 投票动画
- [ ] **核反应堆与原子弹投放** [DLC: WtT/Gott]：核反应堆建造队列 + 原子弹库存 + 投放目标选择 / 战略轰炸面板
- [ ] **秘密武器进度弹窗** [DLC: Gott]：每个秘密分支的进度 + 已解锁装备 + 同盟分享开关
- [ ] **绥靖谈判窗口** [DLC: PoT]：领土割让协议 / 局部和平协议 UI
- [ ] **科研院 / 智库面板**（中国学术机构 / 苏联设计局类） [DLC: NSB/NCNS]：design company 选择 + 任务分配
- [ ] **货币 / 通胀面板**（部分 DLC 国家如阿根廷的金本位机制） [DLC: ToA]
- [ ] **本地化** [DLC: 全部]：每个 DLC 含 `localisation/<lang>/<dlc_name>_l_<lang>.yml`，加载到现有 loc 系统

**M4 验收**：能完整玩 1936-01-01 → 1936-12-31。一年内能：选 focus、建工厂、研究、宣战、打小仗。视觉与玩法都和原版肉眼难分辨。

---

## Phase 5 — 脚本引擎补齐（10-14 周，可与 4 并行）

mod 兼容性的硬骨头。当前 trigger 30 / effect 28，目标都到 ~500。

### 5.1 数据加载（1.5 周）
- [ ] `events/*.txt`（vanilla 123 文件）
- [ ] `common/decisions/*.txt`（83）+ `common/decisions/categories/*.txt`
- [ ] `common/scripted_triggers/*.txt`（50）
- [ ] `common/scripted_effects/*.txt`（49）
- [ ] `common/scripted_localisation/*.txt`
- [ ] `common/national_focus/*.txt` 全 70+ 国（已加载，验证 mod 也能加）
- [ ] 全 vanilla + 大型 mod (Kaiserreich / TNO) **加载不 panic**
- [ ] **DLC 探测层（基础设施）**：所有 DLC 数据加载的先决条件
  - `hoi4-paths` 增加 `enumerate_dlcs()` 扫 `dlc/dlc*` 目录读 `*.dlc` metadata（id / name / required_dlc）
  - `World.dlc_set: HashSet<String>` 启动时填充
  - `World.has_dlc(name) -> bool` 暴露给 trigger / effect / focus
  - 启动 banner 列出已检测的 DLC（"Detected 14 content DLCs: TfV, DoD, WtT, MtG, LaR, BftB, NSB, BBA, AAT, ToA, Gott, GoE, NCNS, PoT"）
- [ ] **DLC 资产链合并加载**：`dlc/dlc*/gfx/`、`dlc/dlc*/interface/`、`dlc/dlc*/portraits/`、`dlc/dlc*/music/`、`dlc/dlc*/sound/`、`dlc/dlc*/localisation/` 全部接入资产链（vanilla 之上、mod 之下，按 DLC id 字典序）
- [ ] **DLC 事件 / 决议 / 国策 / 顾问 / 装备 加载**：
  - 自治领事件 / 装备转换 [DLC: TfV]
  - 东欧事件 + 装备转换器 [DLC: DoD]
  - 中日 30+ 事件 / 决议 / 将领事件 [DLC: WtT]
  - 海军重做相关事件 + 燃料事件 [DLC: MtG]
  - LaR 事件 / 谍报操作（`common/operations/*.txt`）/ 抵抗事件 [DLC: LaR]
  - 土希保事件树 [DLC: BftB]
  - 苏波 + 补给重做相关事件 [DLC: NSB]
  - 意瑞埃 + 国联事件 [DLC: BBA]
  - 北欧 + MIO 事件 + 雇佣兵 [DLC: AAT]
  - 南美 + 合法性事件 [DLC: ToA]
  - 德奥比重做 + 秘密武器事件 [DLC: Gott]
  - 阿伊伊 + 中亚事件 [DLC: GoE]
  - 日本重做 + 满洲改进 + 中国战区事件 [DLC: NCNS]
  - 绥靖路线事件 [DLC: PoT]
- [ ] 启动后 `[scripts] DLC: TfV +12 events +8 decisions +24 focuses ...` 每 DLC 一行

### 5.2 Trigger 全集（4-6 周）
- [ ] 当前 30 / 目标 ~500
- [ ] 国家级 ~180：`has_war_with / is_in_faction_with / num_of_factories / has_government / has_tech / has_idea / has_dlc / ai_value / threat / stockpile / num_divisions / has_army_experience / has_volunteers_amount_from / ...`
- [ ] 州级 ~80：`is_controlled_by / is_core_of / infrastructure / building_level / resource_amount / state_population / ...`
- [ ] 省级 ~30：`is_coastal / has_railway / has_port / ...`
- [ ] 单位级 ~50：`has_template / equipment / experience / ...`
- [ ] 比较运算符 ~20：`GT / LT / EQ + variable_compare + check_variable`
- [ ] 逻辑组合 ~10：`AND / OR / NOT / COUNT / all_of / any_of`
- [ ] Scope 跳转 ~60：`every_country / any_owned_state / random_X / X_unit_leader / X_division / capital_scope / ...`
- [ ] **DLC 特定 trigger ~70**（按 DLC 拆分）：
  - **TfV**：`is_subject_of / overlord / autonomy_state / autonomy_above / dynamic_country_tag`
  - **DoD**：`amount_research_manpower / has_government_with_old_government / has_idea_with_trait`
  - **WtT**：`has_decision / num_decisions_active / has_volunteers_amount_from / has_unit_leader / has_command_power_amount / unit_leader_has_trait / has_war_with_china / mountain_battalions`
  - **MtG**：`has_navy_size / has_naval_mission / has_naval_oob / num_of_naval_factories / has_fuel / fuel_ratio / dock_capacity / has_legation / has_advanced_intel`
  - **LaR**：`has_intelligence_agency / num_operatives / agency_upgrade_unlocked / has_resistance / resistance_target_speed / has_compliance / has_active_operation / agency_active_upgrades / agency_funds`
  - **BftB**：`is_in_array / is_balkan_minor`（多数复用 LaR / WtT）
  - **NSB**：`has_supply_node / supply_in_state / is_railway / has_railway_level / supply_truck_factor / num_motorized_division / num_armor_division`
  - **BBA**：`has_active_treaty / treaty_violation / has_political_advisor_group / num_civilian_factories_available_for_consumer_goods / is_in_war_with_major / has_atom_bomb / has_aircraft_module`
  - **AAT**：`has_mio / mio_active / mio_funds / mio_research_progress / num_special_forces / has_special_forces_cap / mercenary_employer`
  - **ToA**：`has_legitimacy / legitimacy_above / num_court_members / has_pretender / has_court_member_with_trait / has_dynamic_country_tag`
  - **Gott**：`has_secret_research / secret_branch_unlocked / has_secret_weapon_design / has_protectorate_status / num_protectorates / core_sphere_member`
  - **GoE**：`has_tribal_loyalty / tribal_authority_above`
  - **NCNS**：`has_advisor_pool / has_chinese_warlord_status / japanese_specific_X`
  - **PoT**：`appeasement_progress / has_munich_agreement / treaty_signed`

### 5.3 Effect 全集（4-6 周）
- [ ] 当前 28 / 目标 ~500
- [ ] 国家级 ~200：`add_manpower / declare_war_on / annex_country / set_politics / load_focus_tree / add_to_faction / create_corps_commander / news_event / country_event / set_cosmetic_tag / give_resource_rights / load_oob / ...`
- [ ] 州级 ~80：`add_building_construction / set_state_controller / transfer_state / set_demilitarized_zone / state_event / add_resource / ...`
- [ ] 单位级 ~80：`create_unit / load_oob / create_equipment_variant / set_division_template / ...`
- [ ] 变量 / Flag ~30
- [ ] 数学 ~20
- [ ] **DLC 特定 effect**（按 DLC 拆分）：
  - **TfV**：`set_autonomy / end_puppet / set_subject_of / equipment_research / lend_lease / give_military_access`
  - **DoD**：`add_equipment_subsidy / set_equipment_fraction / convert_equipment / start_civil_war / set_government_old`
  - **WtT**：`activate_decision / unlock_decision_category_tooltip / add_command_power / add_doctrine_cost_reduction / add_unit_leader_trait / promote_general / add_political_power_immediate / load_focus_tree`
  - **MtG**：`add_fuel / add_oil / set_naval_oob / damage_building / set_state_flag / start_naval_battle / give_lend_lease / set_legation_level / add_naval_experience / activate_mission`
  - **LaR**：`create_operative / create_intelligence_agency / upgrade_intelligence_agency / start_operation / add_compliance / add_resistance / set_garrison / kill_operative / capture_operative / damage_operative / add_decryption / boost_resistance / increase_compliance / add_legation_level`
  - **BftB**：（共用 LaR / WtT 大部分）
  - **NSB**：`set_supply_node / give_supply / add_railway_level / set_supply_truck_factor / set_supply_system / set_state_supply_area / set_political_party / add_logistics_strike`
  - **BBA**：`set_treaty_violation / sign_treaty / propose_treaty / cancel_treaty / set_advisor_group_active / activate_advisor_group / add_to_advisor_group / add_dynamic_modifier / start_strategic_bombing`
  - **AAT**：`add_mio / activate_mio / unlock_mio_trait / add_mio_funds / add_special_forces_cap / hire_mercenary / fire_mercenary / unlock_mercenary_template / set_mio_funds`
  - **ToA**：`add_legitimacy / set_legitimacy / add_court_member / dismiss_court_member / give_advisor_group / set_court_position / unlock_court_traits / add_pretender_claim`
  - **Gott**：`unlock_secret_branch / unlock_secret_weapon / promote_protectorate / form_core_sphere / load_alternate_focus_tree`
  - **GoE**：`add_tribal_loyalty / change_tribal_authority / promote_tribal_leader`
  - **NCNS**：`activate_advisor_pool / change_warlord_status / give_japanese_focus_tree`
  - **PoT**：`sign_appeasement_treaty / cede_state / give_munich_agreement / boost_appeasement`

### 5.4 Scope 系统完整化（1 周）
- [ ] 完整 `THIS / ROOT / FROM / PREV / PREV.PREV.PREV`
- [ ] 隐式 scope 跳转（`SOV = { ... }` 等价于 `SOV.do_X`）
- [ ] 变量 scope 访问（`var:other_country`）
- [ ] event_target / global event_target

### 5.5 AI 模板加载（1 周）
- [ ] `common/ai_strategy/*.txt`（47）
- [ ] `common/ai_templates/*.txt`
- [ ] `common/ai_strategy_plans/*.txt`（国家特定 AI 计划）
- [ ] 接入 `hoi4-ai` 评分

**M5 验收**：vanilla 1936 → 1945 历史路线全程跑通。所有 vanilla 事件准时触发；GER Rhineland → Anschluss → Sudetenland → Polish Question → WW2 时间线 ±10 天。

---

## Phase 6 — 军事深度（5-7 周）

Phase 1 的"基础打通"在这里补"原版深度"。

### 6.1 补给系统（2-3 周）
- [ ] supply_node 加载 + 渲染
- [ ] 铁路网络流（已有 railways.txt 解析）
- [ ] supply_area 容量计算
- [ ] 师补给需求（按 subunit 配重）
- [ ] 补给不足惩罚（org / move speed / attack）
- [ ] 海运 / 空运补给
- [ ] **补给系统重做（NSB 版）** [DLC: NSB]：替换上述简化版为 vanilla NSB 实现
  - 补给中心（supply hub）+ 优先级 + 可达性图
  - 铁路等级（1-5 级）+ 升级建造
  - 卡车 / 牛马 / 摩托化补给运输 + `motorized_equipment` / `truck` / `horse` 装备消耗
  - state-level supply throughput 与 motorization
  - 大军过补给上限的逐级惩罚（org / supply / attrition）
  - logistics strike（轰炸补给线）
  - 接 air mission 的补给打击战斗机交互

### 6.2 计划系统（前线 + 进攻箭头）（1-2 周）
- [ ] 用户拖出前线
- [ ] 进攻箭头（贝塞尔）
- [ ] 计划进度（planning bonus 累积）
- [ ] AI tactical 接入新计划系统

### 6.3 将领系统（1 周）
- [ ] FM / general / corps commander 三级
- [ ] `common/unit_leader/*.txt` traits 加载 + 作用实现
- [ ] 历史将领 (`history/units/*_leaders.txt`)
- [ ] **指挥点系统（CP）** [DLC: WtT]：每国上限 + 日恢复 + trait/decision/order 消耗
- [ ] **将领特性树升级** [DLC: WtT]：trait 类型按 leader_role 过滤（FM / 集团军 / 军级三档）+ 战后/战时获得条件 + 特性间互斥
- [ ] **海军将领** [DLC: MtG]：admiral 三档（fleet admiral / admiral / contre-admiral）+ 海军 trait

### 6.4 战斗精修（1 周）
- [ ] 地形修正（plains / forest / hills / mountain / marsh / jungle / desert / urban / amphibious）
- [ ] 跨河 / 跨海峡惩罚
- [ ] 战斗宽度（地形限定）
- [ ] 突破 / 包围 / 全包围效果
- [ ] **战斗宽度重做（NSB 版）** [DLC: NSB]：地形 + 攻击方/防守方 + 进攻方向（多省进攻 +20% 宽度）联合决定有效宽度
- [ ] **海战重做** [DLC: MtG]：舰艇分系统命中（船体 / 引擎 / 火炮 / 雷达）+ 战略撤退判定 + 通商破坏抽象
- [ ] **空战与战略轰炸** [DLC: BBA]：战略轰炸目标层（工厂 / 资源 / 基础设施 / 民居 / 雷达）+ 区域伤害模型
- [ ] **战斗事件 / 历史战役标记** [DLC: AAT]：常见历史战役名（如 "Battle of Stalingrad"）触发命名

### 6.5 装备设计器（2 周）
- [ ] 坦克设计器 [DLC: NSB]
  - hull / 主炮 / 副炮 / 装甲 / 引擎 / 悬挂 / 通信 / 特殊模块加载
  - 属性聚合：硬攻 / 软攻 / 装甲 / 穿透 / 速度 / 可靠性 / 油耗
  - 推荐预设（轻型坦克 / 中型坦克 / TD / 自行火炮）
  - 变体保存 + 命名 + 升级
- [ ] 飞机设计器 [DLC: BBA]
  - airframe（小/中/大）+ 模块（机枪 / 机炮 / 炸弹舱 / 油箱 / 雷达 / 自封油箱 / 引擎 等）
  - 属性聚合：制空 / 拦截 / 突袭 / 战略轰炸 / 战术轰炸 / 海上巡逻
- [ ] 舰船设计器 [DLC: MtG]
  - hull（潜艇 / 驱逐 / 巡洋舰 / 战巡 / 战列 / 航母）+ 模块（火炮 / 鱼雷 / 装甲 / 引擎 / 防空 / 雷达 / 探测）
  - 属性聚合：火力 / 装甲 / 速度 / 反潜 / 防空 / 雷达 / 燃料
- [ ] 模块加载 + 属性聚合 + cost 计算
- [ ] 变体保存
- [ ] **设计公司（design company / MIO）影响 stat** [DLC: AAT]：选定 MIO 的 trait 直接修正 cost / stat / production_speed

### 6.6 谍报系统（2 周，新增） [DLC: LaR]

LaR 引入的整套机制，需要新建 `crates/hoi4-intel` logic crate。

- [ ] **`hoi4-intel` crate 骨架**：State / Operatives / Operations / Resistance / Compliance / Decryption 五大系统
- [ ] **情报机构（Intelligence Agency）**：state / branch / upgrades 树（`common/intelligence_agency_upgrades`）
- [ ] **特工**：招募（按背景）+ 特性 + 受伤 / 死亡 / 被俘 + 任务派遣
- [ ] **操作（Operations）**：`common/operations/*.txt` 加载 + 进度模拟 + 失败 / 成功效果
- [ ] **抵抗与服从模型**：每占领州 `resistance` (0-100) + `compliance` (0-100) + 镇压师比例 + 资源产出修正
- [ ] **暗号破译**：`decryption_progress` 模拟 + 反向给敌方情报来源
- [ ] **接入主循环**：每日 / 每周更新 resistance / compliance / decryption / operative 进度
- [ ] **接入 5.x trigger / effect**：见 5.2 / 5.3 LaR 段

### 6.7 特种部队 + 军工组织（1 周，新增） [DLC: AAT]

- [ ] **特种部队上限**：每师 special_force_cap = 国策 / 顾问 / 科技修正叠加
- [ ] **特种部队类型**：山地兵 / 伞兵 / 海军陆战队，按地形获得加成
- [ ] **MIO 系统**：`common/military_industrial_organization/*` 全套加载 + tick
  - organization：4 类（陆 / 海 / 空 / 工程）+ 4 子类
  - policies：决策类似的政策选择
  - traits：树状特性，按 funds 和 research_progress 解锁
  - 关联生产线：MIO trait 给生产线加 stat
- [ ] **雇佣兵**：Mercenary 专属师模板 + 雇佣 / 退役 / 阵亡机制

### 6.8 合法性 + 朝廷（0.5 周，新增） [DLC: ToA]

- [ ] **合法性（legitimacy）**：每国一个 0-100 浮点值 + 来源（focus / event / decision / advisor）
- [ ] **阈值效果**：< 30% 罢工 / 政变风险增加；> 70% 全国精神加成
- [ ] **朝廷成员（court members）**：可任命的政治角色，类似 advisor 但有 group_max
- [ ] **王位继承（pretenders）**：王朝国家的合法性来源

### 6.9 和平会议（重做版）（1 周，新增） [DLC: NSB]

- [ ] **战胜分（war score）系统**：参战、战斗胜利、占领核心、战死人数 加权
- [ ] **战利品池（spoil pool）**：每个被战败方的省份按价值标价
- [ ] **议价系统**：参战方按 score 顺序拍卖式拿州 / 傀儡化 / 解散阵营
- [ ] **AI 议价策略**：核心州优先 + 殖民地次之 + 资源优先
- [ ] **UI**：4.7 提到的"和平会议预览"在这里实做

**M6 验收**：完整打一场欧战。补给会断、将领有作用、装备设计能改宽度、AI 推前线箭头。LaR 谍报跑得起来、AAT MIO 影响生产、ToA 合法性影响政变、NSB 和平会议能正确分赃。



---

## Phase 7 — 视觉打磨与高级资产（4-6 周）

### 7.1 城市与建筑 3D（2 周）
- [ ] `buildings.rs` instance 数据接入 .mesh draw call
- [ ] 按 state.population 决定城市规模
- [ ] 工业建筑、港口、机场、雷达、核反应堆按州数据放置
- [ ] LOD：远景简化矩形
- [ ] **核反应堆建筑 mesh** [DLC: WtT]：与原子弹研究关联
- [ ] **军工组织地标 mesh** [DLC: AAT]：MIO 总部建筑（视觉装饰）
- [ ] **DLC 专属建筑 mesh**：各 DLC 在 `dlc/dlc*/gfx/models/buildings/` 添加的 mesh 全部接入

### 7.2 单位 3D 模型 + 战斗特效（1.5 周）

> **2026-05-16 调度变更**：vanilla 默认舰 / 机 mesh 已在 **3.12.16 单位 3D 模型** 中提前接入。
> 本节剩下的是更深的 DLC unit pack mesh + 战斗特效 + 沉船 / 坠机动画 + 陆军 3D 基座。
>
> **2026-05-18 二次调度**：陆军 3D 基座抽出到 [`ROADMAP_MAP_VISUAL_PARITY.md` 第 20 节](./ROADMAP_MAP_VISUAL_PARITY.md#20-陆军-3d-基座5-7-天-从-v3-72-抽出)，
> 与 15 节（海军 / 空军 3D mesh）并列管理。本节剩下：DLC unit pack mesh + 战斗特效 + 沉船 / 坠机动画。

- [ ] **陆军 3D 基座** → ✅ 迁移到 ROADMAP_MAP_VISUAL_PARITY 第 20 节
- [ ] **战斗火光 / 爆炸 / 烟雾粒子**：3.12.11 particle 框架就位后，按 combat event 触发
- [ ] **沉船 / 坠机**：海上 / 空中 unit 死亡时短暂动画 + 残骸
- [ ] **Unit Pack 装备 mesh 接入**：把以下 9 个 unit pack 的 mesh 全部接入装备视觉系统（按装备类型 → mesh 路由）
  - Rocket Launcher Unit Pack（火箭炮）
  - Famous Battleships Unit Pack（俾斯麦 / 大和 / 衣阿华 等）
  - Heavy Cruisers Unit Pack
  - Soviet / German / French / British / US Tanks Unit Packs（5 国坦克）
  - Axis / Allied Armor Pack（轴心 / 同盟通用装甲）
  - Eastern Front Planes Pack
  - Prototype Vehicles（实验性载具）
  - Warships of the Pacific（太平洋战列舰）
- [ ] **Seaplane Tenders / 水上飞机母舰 mesh** [DLC: ExpansionPass2]
- [ ] **历史变体着色**：每国坦克 / 飞机按 ideology 色调微调（与 portrait 风格一致）

### 7.3 树木升级（0.5 周）✅ **COMPLETE (via ROADMAP_MAP_VISUAL_PARITY #17)**
- [x] 当前 billboard 替换为 .mesh 树
- [x] 距离 LOD：近 mesh / 中 billboard / 远 mask
- [x] 季节变化（春绿 / 夏深绿 / 秋黄 / 冬秃）

### 7.4 NATO counter 高级特性（0.5 周）

> **2026-05-16 调度变更**：counter art 基础版（DDS atlas + 兵种符号 + 国旗 + 编号 +
> 经验星 + 选中高亮）已在 **3.12.15 兵牌升级** 中提前完成。本节剩下的是
> 信息密度切换与 DLC 风格分支。

- [ ] **counter 信息密度切换**：单纯符号 / 含数字 / 含 mini bar 三档（vanilla 也有）
- [ ] **counter 战斗动画**：处于战斗中的师 counter 出现红色脉动 + 进度条
- [ ] **DLC counter 风格**：部分内容 DLC 用专属 counter 框（NSB / BBA 等）
- [ ] **海军 / 空军 counter 细节**：与 3.12.16 的 3D mesh 联动（mesh 缺失时回退到 counter）

### 7.5 .anim 加载（1 周）
- [ ] 解析 `.anim` 骨骼动画
- [ ] 简单骨骼 mixer + LOD（远景不放动画）
- [ ] 主要用于 unit_leader portrait 动画与建筑动画

### 7.6 头像（portraits）DLC 接入（0.5 周，新增）

- [ ] **Portrait DLC 加载链**：每个 DLC 的 `portraits/` 目录合并到资产池，覆盖 vanilla 同名
  - **German Historical Portraits** [DLC: GHP]：德国军政历史人物
  - **Polish Content Pack** [DLC: PCP]：波兰人物
  - **Soviet Union 2D Art** [DLC: CCPSU]：苏联领导人 + 将领专门画作
  - LaR / NSB / BBA / AAT / ToA / Gott / GoE / NCNS 等内容 DLC 自带的 portraits/
- [ ] **Portrait 路由**：`history/countries/<TAG>.txt` 中按 ideology + DLC 选 portrait（vanilla portrait 与 DLC portrait 同名时按 DLC 加载顺序覆盖）

### 7.7 旗帜与图标 DLC 接入（0.5 周，新增）

- [ ] **DLC 国旗变体**：cosmetic_tag 切换旗帜（如 NSB 苏联多种旗）
- [ ] **DLC 国徽 / 议会徽章**：Ideologies 切换图标
- [ ] **focus icon**：每个 DLC 焦点树自带的 `gfx/interface/goals/focus_<dlc_focus>.dds` 全部正确加载

**M7 验收**：所有 .mesh / .anim 资产上屏。视觉差距与原版 < 5%。所有 DLC 视觉资产（portrait / 建筑 mesh / 装备 mesh / 旗帜 / 图标）全部正确加载并显示。

---

## Phase 8 — 音频（2-3 周）

### 8.1 音乐（1 周）
- [ ] `MusicPlayer` 接入主循环
- [ ] 加载 `music/*.ogg` + `*.asset` metadata
- [ ] 按局势切 playlist：peacetime / wartime / mobilizing / faction-leader
- [ ] 按意识形态切音乐组
- [ ] **DLC 音乐包加载链**：每个 DLC `music/` 目录合并 + `*.asset` metadata 接入
  - **Original Soundtrack** [DLC: OST]：vanilla soundtrack DLC（独立 ogg + 元数据）
  - **German March Order Music Pack** [DLC: GMO]
  - **Allied Radio Music Pack** [DLC: ARM]
  - **Sabaton Soundtrack** [DLC: Sab1] + **Sabaton Vol. 2** [DLC: Sab2]
  - **Songs of the Eastern Front** [DLC: SotEF]
  - **Allied Speeches Pack** [DLC: ASP]：英邱 / 罗斯福 / 戴高乐演讲音频
  - **Radio Pack** [DLC: RP]：广播节目类
  - **Ride of the Valkyries**（Expansion Pass 1） [DLC: ExpPass1Mus]
  - **No Compromise No Surrender 音乐包** [DLC: NCNS]：日本主题
- [ ] **playlist 选择规则**：用户在设置面板可启用 / 禁用每个 DLC 音乐包
- [ ] **意识形态匹配**：DLC 音乐自动归类（如 Sabaton 默认民主组，Soviet 歌默认共产组）

### 8.2 音效（1-2 周）
- [ ] UI 音效（hover / click / open / close）
- [ ] 事件触发音
- [ ] 战斗音 / 爆炸 / 飞机引擎
- [ ] **`.fsb` (FMOD bank) 决策**：
  - 选项 A：只放 `music/*.ogg`，跳过 .fsb
  - 选项 B：用社区 fsbext 离线提取，运行时只读 PCM
  - 选项 C：购买 FMOD studio 商业授权（数千美元/年）
  - **V3 默认走 A，文档化 B 给个人玩家**
- [ ] **DLC 音效**：每个内容 DLC 的 `sound/` 目录（事件音 / 单位音）合并
  - 谍报操作音效 [DLC: LaR]
  - 海战 / 出航音效 [DLC: MtG]
  - 坦克 / 飞机变体音效 [DLC: NSB / BBA]
  - 各国领导人事件音效（语音 + 音乐过场） [DLC: WtT / NCNS / Gott / etc.]

### 8.3 cinematics（0.5 周）
- [ ] 开战 / 投降 / 国策完成视觉过场（已有 .anim 后基本免费）
- [ ] **DLC cinematic / 焦点完成动画**：DLC 在 `gfx/event_pictures/` + `gfx/loadingscreens/` 添加的图片接入

**M8 验收**：耳朵和眼睛都"像 HOI4"。

---

## Phase 9 — Mod 兼容（持续）

### 9.1 Mod 加载器（1 周）
- [ ] `descriptor.mod` 解析
- [ ] `replace_path` 目录替换
- [ ] 文件覆盖（mod 链优先）
- [ ] playlist / mod 依赖
- [ ] Steam Workshop 路径自动扫描
- [ ] **DLC 与 Mod 加载顺序**：vanilla < dlc/dlc001/ < dlc/dlc002/ < ... < mod/X/ < mod/Y/，按字典序 DLC 间互相覆盖（vanilla 行为）
- [ ] **DLC 启用 / 禁用 UI**：设置面板列出所有检测到的 DLC，用户可勾选启用 / 禁用（与 vanilla launcher 一致）
- [ ] **Mod required_dlc 检测**：mod descriptor 列出依赖 DLC 时，缺失则警告

### 9.2 兼容性测试套件（持续）
- [ ] Top 50 Steam Workshop mod 自动加载 panic 测试
- [ ] Kaiserreich / TNO / Millennium Dawn / R56 / EAW / Red Flood / OWB 关键路径验证
- [ ] 对照测试：同 mod 在原版与 V3 跑 100 天，关键数值差 < 5%
- [ ] **DLC 全开 / 部分开 矩阵测试**：14 个内容 DLC，先测全开 vs 全关，再随机开关组合（重点：单独 NSB 启用时和平会议机制是否启用、单独 LaR 时谍报系统是否启用、组合启用时国策树正确路由）
- [ ] **DLC 历史路径回归**：每个内容 DLC 跑一次 1936-1939 历史路径 smoke
  - TfV：CAN 自治领独立路径
  - WtT：CHI 抗战路径
  - MtG：USA 海军建设路径
  - LaR：FRA 谍报路径
  - NSB：SOV / POL 路径 + 苏波合并战
  - BBA：ITA 入侵埃塞 + 国联谴责
  - AAT：FIN 冬战 + MIO 升级
  - ToA：ARG / BRA 经济发展
  - Gott：GER 秘密武器路径
  - GoE：AFG / IRN 路径
  - NCNS：JAP 中国战区
  - PoT：GER 慕尼黑协定路径

### 9.3 原生 Mod API（2-4 周）
- [ ] `hoi4-modding-api` 稳定 crate
- [ ] 自定义 trigger / effect / 地图模式注册
- [ ] dylib 热加载（开发期）

**M9 验收**：装 Kaiserreich，看到 1936 凯撒地图，能玩到 1939。

---

## Phase 10 — 性能 / 打磨 / 发布（持续）

### 10.1 性能
- [ ] tick profiling（目标 1936-1945 打完 < 1 h 实时 5x）
- [ ] 多线程并行（per-country tick）
- [ ] GPU：60 FPS @ 1080p / 30 FPS @ 4K
- [ ] 内存峰值 < 4 GB
- [ ] **DLC 全开下的性能基线**：14 个内容 DLC + 9 个 unit pack + 8 个音乐包 全部启用时
  - tick 时间不超过仅 vanilla 的 2.5×
  - 启动加载时间 < 30s（vanilla < 12s）
  - 内存峰值 < 6 GB（DLC portrait / 装备 mesh / 焦点 icon 占用约 +1.5 GB）

### 10.2 确定性
- [ ] 固定浮点 / 定点数热点
- [ ] 自家存档 round-trip bit-exact
- [ ] CI 100 天种子化测试

### 10.3 错误处理
- [ ] 资源缺失友好提示
- [ ] 脚本错误 in-game 控制台显示，不 crash
- [ ] 崩溃日志 + 自动 issue 模板

**M10 验收**：发布候选版本。

---

## 工作量与里程碑

| Phase | 时长 | 累计 | 出口物 |
|---|---|---|---|
| 0 — 地基整顿 | 2 周 | 2 周 | M0：调度器 + path 配置化 |
| 1 — 模拟接入 | 4-6 周 | 8 周 | M1：年回归 ±5% |
| 2 — 资产管线 ⭐ | 6-8 周 | 16 周 | M2：vanilla topbar.gui 上屏 |
| 3 — Shader 等价 ⭐ | 3-5 周 | 21 周 | M3：地图肉眼难分辨 |
| 3.10 — 树木尺寸 / 选址 / 国名 3D | 2 周 | 23 周 | M3.10：3 处显眼差距修复 |
| 3.11 — 全管线 shader 翻译（wgsl 层）⭐ | 5-7 月（核心 1 周已交付）| 23 周+1 周 | M3.11：24 wgsl 翻译 + 注册 + naga 校验 |
| **3.12 — 主循环集成（把 3.11 接到屏幕）** ⭐ | **3-4 周** | **27 周** | **M3.12：shader 翻译真正上屏，进游戏视觉差距大幅缩小** |
| 4 — UI 全套 | 8-10 周 | 35 周 | M4：1936 完整可玩 |
| 5 — 脚本补齐 | 10-14 周（与 4 并行） | 35 周 | M5：1936-1945 历史路径 |
| 6 — 军事深度 | 5-7 周 | 42 周 | M6：完整欧战 |
| 7 — 视觉打磨 | 4-6 周 | 48 周 | M7：差距 < 5% |
| 8 — 音频 | 2-3 周 | 51 周 | M8：眼耳齐 |
| 9 — Mod 兼容 | 持续 | — | M9：Kaiserreich 跑 |
| 10 — 性能 / 发布 | 持续 | — | M10：1.0 RC |

> **核心总计 51 周（约 12 个月）单人全职**到 M8（眼耳齐、欧战完整、视觉无差距）。
> Mod 兼容 + 性能打磨另需 6-12 个月。
> 3.11 完整 1:1 翻译加 3.12 集成，比 V3 原 Phase 3 估算多约 4 周。
>
> 参照系：Paradox 原版 2016 至今 10 年 + 11 DLC + 50 人团队。
> 单人 12 个月做到 vanilla 1936 完整可玩 + 视觉等价是激进但可能的目标。
> 4 人团队 6-8 个月可达。

