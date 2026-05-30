# Project Ironheart V3 — 路线图附录

> **Historical note (Phase 11, 2026-05-30):** This legacy appendix is archived. Current map renderer pass order, ownership, debug, baseline, and cleanup policy live in [`../../MAP_RENDERER_V2_REFACTOR_ROADMAP.md`](../../MAP_RENDERER_V2_REFACTOR_ROADMAP.md), [`../map_renderer_v2.md`](../map_renderer_v2.md), and [`../map_renderer_v2_contributing.md`](../map_renderer_v2_contributing.md).

> 本文档收纳 V3 路线图的非-路线图内容（现状记录、设计原则、与 V2 的差异、
> 行动清单、治理变化、当前进度、DLC 拆分台账）。主路线图见同目录 
> [ROADMAP_V3.md](./ROADMAP_V3.md)。

---

## 现状清零（2026-05-14）

> 不要看 V1 的"✅"，看代码。

### 真实可用

| 模块 | 状态 |
|---|---|
| `clausewitz-parser` | 完整 Lexer/Parser/AST，能解原版任意 .txt（21 测试） |
| `lua_defines` | 解 `00_defines.lua` 28 sections |
| `hoi4-map` 数据加载 | provinces.bmp / definition.csv / adjacencies.csv / heightmap.bmp / terrain.bmp / strategicregions / supplyareas，13382 省，0.24 s |
| `hoi4-data` 数据加载 | countries / country_tags / states / units / equipment / technologies / national_focus / ideologies / buildings / resources / terrain / localisation YAML，0.17 s |
| `hoi4-state` World 初始化 | 国家/州/省 SoA，工厂缓存，capitals，初始解锁 |
| 3D 地形渲染 | heightmap mesh + chunk×LOD + 视椎剔除 + hillshade + 三平面噪声 + 雪线 + 海岸沙带 + SDF 边界 + 水面 fbm + 海岸泡沫 + Phong + 程序云层 + 昼夜循环 + 季节雪线 |
| 树木 | 25 K GPU instance billboard |
| 铁路线 | 3534 段 LineList |

### 写完但**未接入主循环**（V1 标 ✅ 的虚假完成）

| 模块 | 真相 | 影响 |
|---|---|---|
| `hoi4-logic` 经济 / 科技 / 政治 / 陆军 / 海军 / 空军 / 外交 | `hoi4-render/Cargo.toml` **不依赖 `hoi4-logic`**，`World::tick_hour` 只是 `date.advance_hour()` | 时间走但什么都不发生 |
| `hoi4-script` (30 trigger / 28 effect) | 同上，**未接入主循环**；只在自己的集成测试里跑 | 0 事件触发 / 0 决议运行 |
| `hoi4-ai` (战略 + 战术) | 同上，**未接入主循环** | AI 永不思考 |
| `World.divisions` | 启动 = `DivisionStore::new()`（空） | **运行时世界上 0 个师存在** |
| `units.rs` (NATO 符号管线) | 管线接好了，但 `World.divisions` 是空 | 上屏 0 个图标 |
| `buildings.rs` | 文件存在，**`main.rs` 未 import 也未调用** | 上屏 0 个建筑 |
| `dds.rs` | 解析 BC1/BC3 OK，**`main.rs` 未调用** | 地形仍是程序化色块 |
| `mesh.rs` | 启发式解析 OK，**`main.rs` 未调用** | 上屏 0 个 .mesh |
| `audio.rs` (`MusicPlayer`) | rodio 封装 OK，**从未被构造** | 0 音乐 |
| `hoi4-state` 存档系统 | 自家文本/二进制 round-trip OK；从未读过一个**真实** `.hoi4` 文件 | 不能读现成存档 |

### 完全没动

- `.gui` / `.gfx` / `.fnt` / `.asset` / `.anim` / `.fxh` / `.shader` / `.fsb` 解析器：**0**
- 字体管线 / 文字渲染：**0**
- 任何 UI 控件 / 按钮 / 面板：**0**
- `events/*.txt` 加载（vanilla 123 文件）：**0**
- `common/decisions/*.txt` 加载（vanilla 83 文件）：**0**
- `common/scripted_triggers/*.txt`（50）/ `scripted_effects/*.txt`（49）：**0**
- `common/ai_strategy/*.txt`（47）/ `ai_templates/`：**0**
- 国策树**执行**（数据加载 OK，但运行时不被推进）：**0**
- 真实 `.hoi4` 存档读取：**0**
- `HOI4_PATH` 配置化：**0**（11 个文件硬编码 `C:/Program Files (x86)/Steam/...`）

### 资产规模锚点（vanilla，本机实测）

| 类型 | 数量 |
|---|---|
| `interface/*.gui` | 124 |
| `interface/*.gfx` | 142 |
| `gfx/fonts/*.fnt` | 59 |
| `gfx/**/*.mesh` | 414 |
| `gfx/**/*.dds` | 21 087 |
| `events/*.txt` | 123 |
| `common/decisions/*.txt` | 83 |
| `common/scripted_triggers/*.txt` | 50 |
| `common/scripted_effects/*.txt` | 49 |
| `common/ai_strategy/*.txt` | 47 |

DLC / mod 还会再加一倍以上。所以"资产格式解析"必须做对，做对一次就一劳永逸。



---

## V3 设计原则

1. **资产兼容是第一公民**。`.gui` / `.gfx` / `.fnt` / `.dds` / `.mesh` 解析与上屏属于"主干"，不是"打磨阶段"。
2. **声明式优先**。UI / shader / 资源不要在 Rust 里写硬编码副本，引擎只做"解释器"。原版换一个文件，引擎跟着变。
3. **集成纵切**。每个 Phase 完成的标志是"在运行的二进制里能被肉眼或自动化测试观察到"。
4. **资产层冻结后再做内容**。`.gui` 解释器、字体、shader 等价物如果不先打通，做面板就是在"画自定义 UI 然后假装是 HOI4"。
5. **DLC / Mod 自动跟进**。引擎只读用户安装目录，不嵌入任何 Paradox 资产。
6. **存档不与原版兼容**（用户决定）。释放 ironman token 表逆向 / 字段 1:1 映射的工程量。

---

## 与 V2 的差异

| 维度 | V2 | V3 |
|---|---|---|
| 项目定位 | "用 Rust 重写 HOI4" | "Clausewitz 引擎的开源替代实现"（ScummVM 模式） |
| 自定义 vs 兼容 | 自研 GUI 框架（egui / 自研），可后期换 .gui | **`.gui` 解释器是 Phase 2 主干**，永远不写自定义 UI |
| Shader | 现有 wgsl 视为最终；DDS atlas 当作"资源管线"延后 | **shader-rt 等价映射注册表**，每个 .fxh 对应一个 wgsl，资产自动驱动 |
| 资源管线位置 | Phase 4（中后） | **Phase 2**（仅次于地基与模拟接入） |
| 字体 | Phase 9（最后做本地化） | **Phase 2.4**（没字体连一个 GUI 数字都画不出来） |
| 存档兼容 | Phase 8 双向兼容 | **取消**（用户决策释放）；只保留自家格式 |
| `.gui` 优先级 | "可选先填充" | **必须真兼容** |
| Crate 拆分 | `hoi4-render` 一家做主 | 新增 `hoi4-app` / `hoi4-ui` / `hoi4-audio` / `hoi4-assets` / `hoi4-paths` |
| Phase 数 | 12 | 11（合并视觉打磨与资源管线） |

---

## 立即可动手的清单（按周）

**第 1 周**：
1. 改 README + Cargo.toml 描述 → ScummVM 模式定位（30 分钟）
2. 新建 `hoi4-paths` crate，加 `--game-path` / env / config 三档（半天）
3. 替换 11 处 `HOI4_PATH` 硬编码（半天）
4. 新建 `hoi4-app` crate，从 `hoi4-render/main.rs` 抽出入口（1 天）
5. CI 加 `cargo check --all` + path-fixture smoke（半天）

**第 2 周**：
6. `SystemSchedule` 抽象（1 天）
7. `hoi4-app` 加 `hoi4-logic` / `hoi4-ai` / `hoi4-script` 依赖（1 天）
8. `World::tick_hour` 路由到所有子系统的 no-op 占位（1 天）
9. 集成测试 1d/30d/365d smoke（1 天）

**第 3-4 周**：
10. `World` 初始化加载 OOB（divisions / fleets / air_wings）
11. economy / politics / research / military daily 接入 SystemSchedule
12. 30 天回归测试（GER 民工增长 ±10%）

**第 5-8 周**：Phase 1 剩余（外交 / 脚本 / AI 接入），M1 验收。

**第 9 周开始进 Phase 2 资产管线**。

---

## V3 治理变化

| 维度 | V2 | V3 |
|---|---|---|
| 完成定义 | "在运行的游戏里能被观察到" | 同 + **资产驱动**：换 vanilla 文件不动 Rust 代码就跟着变 |
| 阶段依据 | 集成纵切 | 同 + **资产层冻结优先于内容层** |
| 路径配置 | env / config / CLI | 同 + mod 链 + replace_path |
| 测试 | 单测 + smoke + tick 回归 | 同 + 资产 fixture 树 + .gui parse-不-panic 测试 + 关键截图回归 |
| DLC 标识 | 显式 `[DLC: XXX]` | 同 |
| 现状诚实 | "已知未达完成"清单 | 同 + **真实资产规模锚点**（gui=124, gfx=142, fonts=59, mesh=414, dds=21 087, events=123, decisions=83, scripted_triggers=50, scripted_effects=49, ai_strategy=47）|
| 存档兼容 | Phase 8 必做 | **取消**（用户释放） |

---

## 当前进度（2026-05-15）

- ✅  Phase 0（地基整顿） — 完成 2026-05-14
  - `hoi4-paths` crate（CLI/env/config/Steam，mod 链 + replace_path）
  - `hoi4-app` 入口 + `hoi4-render` 退化为库
  - `SystemSchedule`（hourly/daily/weekly/monthly + 计时统计）
  - `hoi4-integration` 1d/30d/365d smoke + metrics 基线
  - 327/327 测试通过；标题栏 `systems: econ ✓ politics ✓ ... — 0.0 ms/day` 与 M0 字面一致
- 🚧  Phase 1（模拟接入） — 进行中
  - ✅  1.1 World 初始化扩展（OOB land/naval/air + 国家 ideas/focus/vars/flags） — 完成 2026-05-14
    GER 30, ENG 36, SOV 138, FRA 74, USA 36, JAP 60, ITA 46, CHI 57（与 vanilla 文件 ±0%）
  - ✅  1.2 经济 / 政治 / 科技接入 — 完成 2026-05-15
    - `SimContext` 模式：`SystemFn = fn(&mut SimContext)` 持有 World + EconomyState + ResearchState + PoliticsCache
    - `economy_daily` / `politics_daily` / `research_daily` adapter 函数接入 SystemSchedule
    - `PoliticsCache` modifier 正确传递：construction_speed_factor → 建造速度，research_speed_factor → 研究速度
    - `init_simulation()` 封装初始化序列（economy::init_world + companion states + recompute_modifiers）
    - 标题栏：`systems: econ ✓ politics ✓ research ✓ military · diplomacy · ai · script · — 0.9 ms/day`
    - smoke_1d ✓ / smoke_30d ✓ / smoke_365d ✓ / economy_year 8/8 ✓ / research_year 13/13 ✓ / politics_year 11/11 ✓
  - ✅  1.3 军事 / 外交接入 — 完成 2026-05-15
    - 陆/海/空 daily org regen（`military::organisation::tick_daily` + 新 `naval::organisation::tick_daily` + 已有 `air::spawn::tick_daily`）
    - 陆/海/空 hourly arbiter（每 4 小时一轮）：`military::arbiter` 扫邻接陆省对、`naval::arbiter` 按 region_id 分组、`air::arbiter` 按 region_id 分组（fighter 优先拦截）
    - 外交 daily：`tick_world_tension_daily` + `advance_justification` + `tick_autonomy_daily`
    - 新增 `reconcile_war_membership`（晚加入阵营对账）+ `evict_capitulated_daily`（投降清理）
    - 标题栏：`systems: econ ✓ politics ✓ research ✓ military ✓ diplomacy ✓ ai · script · — 1.5 ms/day`（30d release）
    - 348/348 测试通过；新增 phase_1_3 集成测试 5/5（org regen / 紧张度增长 / wargoal 完成 / 陆战仲裁器 / 调度器 active）
    - v1 局限：海/空区按 OOB 填的 region_id 分组（非真海/空区），等 Phase 6 加载后批量替换
  - ✅  1.4 脚本引擎接入 — 完成 2026-05-15
    - 新增 `hoi4-app::script::ScriptState` companion：EventScheduler + DecisionCatalog + Variables + Flags + Trigger/Effect 注册表
    - `SimContext` 增加 `script: &mut ScriptState` 字段（4 元组改 5 元组），所有调用点同步更新
    - `script_daily`：`drain_due` 弹事件 → 跑 immediate + ai_chance 最高的 option（v1 无 UI 自动选择）
    - `script_daily`：`DecisionCatalog::tick_daily` 推 days_remove 倒计时 → 到期跑 `remove_effect`（fallback `complete_effect`）→ purge
    - `script_monthly`：MTTH 抽签（xorshift64 PRNG，1 - (1 - 1/days)^30 概率近似 + trigger 求值）
    - 国策 completion_reward：politics 已有 effect runner 在 1.4 调度下继续工作（已有验证）
    - 标题栏：`systems: econ ✓ politics ✓ research ✓ military ✓ diplomacy ✓ ai ✓ script ✓ — 1.0 ms/day`（30d release）
    - 353/353 测试通过；新增 phase_1_4 集成测试 5/5（调度器 active / 事件 immediate+option / 决议倒计时 / MTTH 触发 / 国策完成）
    - v1 局限：vanilla `events/*.txt` / `common/decisions/*.txt` / scripted_triggers/effects 加载是 Phase 5.1 的工作，本期 EventScheduler/DecisionCatalog 默认空，wiring 已就位
  - ✅  1.5 AI 接入 — 完成 2026-05-15
    - 新增 `hoi4-app::ai::AiState` companion：包含 `StrategicAi` 实例（按国家持有 cooldown 跟踪 + personality）
    - `SimContext` 增加 `ai: &mut AiState` 字段（5 元组改 6 元组），所有调用点同步更新
    - `ai_daily`：转发到 `StrategicAi::tick(&mut World, &mut EconomyState, &mut ResearchState)`，orchestrator 内部按 cadence 评估 focus / research / production / diplomacy / tactical
    - 标题栏：`systems: econ ✓ politics ✓ research ✓ military ✓ diplomacy ✓ ai ✓ script ✓ — 1.6 ms/day`（30d release）
    - 100 天回归：active focus 增长（pre 0 → post 数十国），≥ 1 国完成至少 1 项 tech，≥ 1 国创建或加入阵营
    - 性能：100d release ~ 21 ms/day debug；30d release **5.6 ms/day**（预算 50）
    - 355/355 测试通过；新增 phase_1_5 集成测试 2/2（调度器 active / 100 天 AI 接管不 panic 且达回归口径）
    - phase_1_4 focus_completion_runs_existing_effect_runner 因 AI 自动续 focus 调整断言（要求 `current_focus != 已完成项` 而非"必须为 None"）
- ✅ Phase 1（模拟接入）— 完成 2026-05-15。M1 验收：年回归 ±5% 实测见 smoke_365d 输出
- 🚧  Phase 2（资产管线） — 进行中
  - ✅  2.1 `hoi4-assets` crate 骨架 — 完成 2026-05-15
    - 新 crate `hoi4-assets`：`AssetDb` trait + `FsAssetDb` 默认实现 + `AssetError` 错误链
    - 两层 LRU：原始字节 cache（`PathBuf → Arc<[u8]>`）+ 类型化解析 cache（`(PathBuf, TypeId) → Arc<dyn Any + Send + Sync>`）
    - `parse_or_get<T>(rel, parser)` 单次解析 → `Arc<T>` 多处共享，`Arc::ptr_eq` 验证零拷贝
    - mod 链回退 + replace_path 屏蔽通过 `hoi4-paths::PathConfig::find/find_all` 直通
    - referrer 链：`asset not found: X.dds\n  referenced by: parent.gui\n  referenced by: top.gui`
    - 22/22 测试通过（17 lib + 5 integration），workspace build 干净
    - v1 局限：`RefCell` 内部可变 → 单线程；多线程加载方案在 `db.rs` 注释里写明（Mutex 替换即可）
  - ✅  2.2 `.gfx` 解析 — 完成 2026-05-15
  - ✅  2.3 DDS 上屏管线 — 完成 2026-05-15
    - `DdsImage`：BC1/BC3/BC5/BGRA8 + 完整 mip 链 + DX10 扩展头 + sRGB/linear 标记
    - `TextureBank`：sprite → `GpuTexture` 共享缓存 + `compute_frame_uvs` 帧切片
    - `hoi4-app` 请求 `Features::TEXTURE_COMPRESSION_BC`
  - ✅  2.4 字体管线 — 完成 2026-05-15
    - `BmFont` 解析器 + glyph/kerning 查询 + `text_width()` + `parse_rich_text()`
    - vanilla 59 个 .fnt 回归通过
  - ✅  2.5 `.gui` 解析器与 AST — 完成 2026-05-15
    - `GuiIndex` + `GuiNode`（15 种控件类型）+ 递归子控件 + 未知字段静默忽略
    - vanilla 124 个 .gui 回归通过
  - ✅  2.6 `.gui` 控件运行时 — 完成 2026-05-15
    - `GuiRuntime` layout solve + 9-slice + hit-test + focus + `DataBinding` trait
  - ✅  2.7 `.mesh` 升级 — 完成 2026-05-15
    - `PdxMesh`：position/normal/uv/tangent/bone + u16 indices + `MeshMaterial`
  - ✅  2.8 `.asset` 元数据 — 完成 2026-05-15
    - `AssetMetaIndex`：music + model 条目解析 + `load_dir()` 批量加载
  - ✅  2.9 渲染集成 + 主循环接入 — 完成 2026-05-15
    - `ui_pass.rs`：2D UI pipeline + topbar 30 个 sprite 上屏
    - orientation 感知定位 + 帧切片 UV + 背景拉伸
    - `MusicPlayer` 接入：启动自动播放 `music/*.ogg`
    - `generate_buildings()` 数据就绪（draw 延后 Phase 7）
    - 待接入：BmFont 文字渲染 / GuiRuntime hit-test / TextureBank 统一 — ✅ 全部完成
- ✅  Phase 2（资产管线）— 完成 2026-05-15
    - 解析层：72 测试通过（60 unit + 12 integration）
    - 渲染集成：topbar sprite 上屏 + 音乐播放 + buildings 数据就绪
    - 待 Phase 4 补齐：文字渲染 + hit-test + 精确布局
- ✅  数据层（解析器 / 地图 / 数据加载） — 真完成
- ✅  渲染基础（5.1~5.7 视觉部分） — 真完成
- ✅  Phase 3.7（Paradox `.mesh` 二进制格式逆向 + 3D 树木 instanced 渲染） — 完成 2026-05-16
  - `crates/hoi4-assets/src/pdx_mesh.rs` typed binary tree 解析器（!  / [ 节点 + i/f/s 类型 + 深度栈跟踪）
  - 全 414 个 vanilla mesh 解析率 99.7%（413/414）
  - `crates/hoi4-render/src/trees_mesh.rs` u32 indices + INSTANCE_CAP_PER_TYPE=12000 + cap_instances() stride 抽样
  - `crates/hoi4-app/src/main.rs::render` 接入 trees_mesh draw_indexed call；ALL 3 类树 mesh 加载成功时跳过 billboard
  - 单测 8 + 5 PASS / vanilla 集成 3 PASS / cargo build --workspace 干净 / headless smoke 13.5 ms/day
- ❌  Phase 4 (UI 全套)、Phase 5 (脚本补齐)、Phase 6-10 — 全部未开始
- ✅  Phase 3 全部完成 — 完成 2026-05-16
  - 3.1~3.4 已早完成（着色风格校准 + shader 等价层）
  - 3.5 mapname 接入：`crates/hoi4-render/src/mapname.rs::compute_country_labels()` + main.rs 投影渲染（5/5 单测）
  - 3.6.1~3.6.3 已早完成（政治色权重 + 边界精修 + 树木 mesh 替换）
  - 3.6.4 vignette 减弱到 0.15（screen_width/height/strength 入 RenderParams + 屏幕空间 vignette）
  - 3.6.5 各向异性过滤 8× + 复核 terrain atlas 是真 vanilla
  - 3.6.6 河流渲染：`hoi4-map/src/rivers.rs` BMP 解析 + R8 GPU 上传 + shader.wgsl 内嵌 navy 蓝叠加 + zoom-LOD（3/3 单测）
  - 3.7 PdxMesh typed binary tree 解析 + 3D 树木 instanced 渲染（已早完成）
  - 全 workspace 测试 PASS / cargo build 干净 / headless 7d smoke 14.1 ms/day
- ✅  Phase 3.10（与原版渲染保真度对齐 — 树木尺寸 / `trees.bmp` 选址 / 国名 3D 标签） — 完成 2026-05-16
  - 起因：与 vanilla 同视角对比（`screenshots/2026-05-16 113050.png`）发现三处显眼差距，全部来自几何/选址/技术路线，非 shader 调参可解
  - 3.10.1 树木世界尺寸校准：`base_scale × world_scale` 在 `generate_trees` 一次性算掉（方案 B），新基准 forest/conifer 0.0012 / palm 0.0016；trees.wgsl billboard 与 trees_mesh.wgsl 3D mesh 同步缩小
  - 3.10.2 选址源切到 `map/trees.bmp` + `default.map::tree = {3,4,7,10}` — 新增 `crates/hoi4-map/src/trees_bmp.rs`（186 行 + 5 单测）+ `parse_default_map_tree_block_str`（4 单测）+ `GameMap::tree_definition_bmp/tree_indices`；`generate_trees` 完全改写按 trees.bmp 像素枚举，下标 3/4/7/10 → 0/0/1/2 路由；`vSlopes` 斜坡修正（heightmap 中央差分 → Snorm8x2 packed → wgsl 应用）
  - 3.10.3 国名 3D atlas：fontdue 烘焙 R8 atlas 1024×N + 1px 描边 two-tier 编码（mapname_atlas.rs 412 行 + 2 单测）；2D PCA OBB 计算（mapname_3d.rs 262 行 + 6 单测，f64 累加器避免溢出）；mapname_3d.wgsl（124 行）instanced 3D quad 沿 OBB axis1 拉伸 + 昼夜调暗 + z-bias；2D HUD 路径降级为 fallback；setup_mapname_3d_pipeline 在 main.rs 装好 + LOD 阈值过滤
  - 测试：cargo test -p hoi4-map 25/25 + hoi4-render 83 + hoi4-app mapname_atlas 2/2 全 PASS；`cargo build --workspace --release` 干净；代码净增 ~1300 行
- 🔜  Phase 3.11（全管线逆向：原版 shader / 资源 1:1 等价移植） — **2026-05-16 启动，3.11.1 已交付**
  - 起因：用户验收明确要求"用上全部的 shader 和资源 能逆向就逆向"。Phase 3.2 当初的"等价映射注册表 = 简化版"原则被推翻：现状 7 个 shader entry + 5 张地形纹理远不够，原版输入是 45 shader / 9865 行 HLSL + 71 张 map 资源 / 140 MB
  - 15 子节按渲染数据流排：3.11.1 shader_lib 基建 → .2 全资源加载 → .3 pdxmap → .4 pdxmesh → .5 pdxwater → .6 river → .7 tree 完整 → .8 border 5-LOD → .9 mapname 原版路径 → .10 particle+sky → .11 箭头四件套 → .12 阴影管线 → .13 后处理链 → .14 GUI 10 个 → .15 defines.lua 镜像 + RenderDoc 回归
  - 关键缺口（视觉影响最大）：法线贴图（atlas_normal/world_normal）、CSM 阴影、后处理链（bloom/lut/saturation/restorescene 当前没接进主循环）、城市夜光（colormap 的 A 通道 + citylights_*）、PBR-ish pdxmesh 替换 30 行 flat_diffuse fallback
  - 工作量估算 5-7 个月（单人专职）；放弃完全 1:1 = 12+ 个月
  - 设计原则更新：V3 第 1 条改为"**资产 + 着色管线 1:1 等价是第一公民**"
  - ✅ **3.11.1 完成 (2026-05-16)**：shader_lib.wgsl（10 组 ~30 个函数）+ defines.rs（~80 vanilla 常量）+ global_uniform.rs（256 byte `GlobalFrameUniform` + WGSL struct）+ shader_rt.rs::compose_shader helper（默认 prepend 或 inline `//#include` 替换）+ 8 个 naga 静态校验集成测试。`cargo test --workspace` ALL PASS；`cargo build` 干净。原 7 shader entry 全部跑过新公共库 parse 校验。
  - ✅ **3.11.2-3.11.15 wgsl 翻译层完成 (2026-05-16)**：
    - **3.11.2** vanilla_map_set.rs（548 行 + 9 单测）：71 资源枚举 + sRGB / 必需性元数据 + IO 加载
    - **3.11.3-3.11.14** 翻译过的 wgsl 共 24 文件 ~1900 行：pdxmap / pdxmesh / pdxwater / river / tree / border / mapname / particle / sky / maparrow / traderoute / arrow / strait / shadow / shadowblur / downsample / downsample_luminance / bloom / lut_blender / restorescene / saturation_slider / gui_button / gui_progress / gui_special。每个用 `compose_shader` 注入 shader_lib + GlobalFrameUniform，naga 静态校验全 PASS
    - **3.11.15** defines_lua.rs scanner（256 行 + 6 单测）：lua 键提取 + 渲染相关启发式过滤 + DefinesDiff 报告
    - **shader_rt 注册表**全量替换：所有 21 个 vanilla shader 名 + 10 个 GUI 名 + 5 个 fallback 名共 31 个 shader 名指向真实 wgsl
    - 测试：`cargo test -p hoi4-render` 110 lib + 32 integration = 142 PASS；`cargo test --workspace` ALL PASS；`cargo build --workspace` 干净
    - **WGSL 已就位但未上屏**：`ShaderRegistry::new()` 注册了名字 → wgsl 映射，但 main.rs 创建 pipeline 时**根本没查这张表**，仍然 `include_str!` 旧的 `shader.wgsl` / `trees.wgsl` / `buildings.wgsl`。所以进游戏看到的画面与 3.11 启动前**字面无差别**——这部分工作单独立项为 [Phase 3.12](#312-主循环集成--把-311-的翻译挂到屏幕上3-4-周)。
- 🔜  **Phase 3.12 主循环集成（2026-05-16 立项；进行中）**：把 3.11 翻译产物**真正接到屏幕**
    - 15 子节，3-4 周单人专职估算
    - 推荐顺序：3.12.1 公共基础设施（offscreen RT + texture upload + uniform buffer 共享） → 3.12.2 后处理链（**最快见效，进游戏第一次看到差别**） → 3.12.3 阴影 → 3.12.4 pdxmap → 3.12.5 pdxmesh → 3.12.6 水 → 3.12.7 河 → 3.12.9 边界 → 3.12.8 树完整 → 3.12.11 粒子+天空 → 3.12.10 mapname → 3.12.12 箭头 → 3.12.13 GUI 替换 → 3.12.14 收尾 → 3.12.15 截图+性能+RenderDoc 回归
    - V3 完成定义"在跑动的游戏里被观察到"严格执行：3.11 写完不算完，3.12 完成才算
    - **进度（2026-05-16）**：
      - ✅ **3.12.1 公共渲染基础设施** — HdrTarget + GlobalUniformBuffer + TextureUploadHelper + SimpleBlitPass + PassRegistry + DebugOverlay + 6 个 3D pipeline 全部切换到 RGBA16Float HDR RT
      - ✅ **3.12.2 后处理链** — bloom_bright + 3 级 downsample + 4 级 luminance reduction + restorescene_live (ACES + auto-exposure)；F5 切换 Off/Full
      - ✅ **3.12.3 阴影管线集成** — `ShadowPass` (D32Float 2048×2048) + caster pipeline + `GlobalFrameUniform.shadow_view_proj` (320 byte)；接收方留各 pass 自行接入
      - ✅ **3.12.4 pdxmap 集成** — `TerrainPass` + atlas_normal / world_normal / colormap_emissive / citylights × 4 张新贴图 + 3 bind groups；旧 SHADER_MAIN_WGSL 归档
      - ✅ **3.12.5 pdxmesh 集成** — `PdxMeshPass` + 1×1×6 grey cubemap fallback + 3 类建筑 mesh（civ_factory / factory / dock_01）+ 3 类 indexed draw call；建筑数据通过 `set_buildings(...)` 按 `kind` 分到 3 个 instance buffers；`any_loaded == false` 时 fallback 到旧程序化 billboard pipeline。10/10 单测 PASS（含 naga wgsl 校验）
      - ✅ **3.12.6 pdxwater 集成** — `WaterPass` 复用 TerrainPass per-LOD instance buffer + chunk grid（不新建 mesh），VS 把 world_y 钳到 SEA_LEVEL × HEIGHT_SCALE，FS 采样 heightmap 把 `h > SEA_LEVEL` 的陆地像素 discard；7 张 vanilla 水纹理（lean1/2 / reflection / fow_water_spec / colormap_water_0 / ice_diffuse / ice_noise_0）经 `load_or_fallback` 加载，缺失时 1×1 fallback。Pipeline depth_compare=LessEqual + depth_write=false 在终于覆盖 terrain pass 的程序化水的同时不影响 trees/buildings 深度。6/6 单测 PASS（含 `water_wgsl_naga_parses` + `references_all_water_bindings` + `has_fragment_discard_and_fresnel`）
      - ✅ **3.12.15 兵牌升级** — counter art with vanilla DDS atlas + archetype symbols + selection
- ✅  Phase 4.1.bis（GUI 布局引擎重做）— 完成 2026-05-16
  - 起因：用户对比项目顶栏与 vanilla 同视角截图，全屏错位 / 缺图标 / 右侧菜单按钮整组消失。Phase 4.1 已勾的 ✅ 仅指数据 hook，**布局这部分从未对过**——保留 4.1 已完成状态，新立 4.1.bis 单独修
  - 7 个子节按依赖顺序全部完成：
    - **4.1.bis.1** lexer 修 `100%%` 百分比 — `Token::Percent(i32)` + `Value::Percent(i32)` + `Block::get_percent()`；25 测全 PASS（含 6 新 case）
    - **4.1.bis.2** LayoutSolver 单一来源 — `gui_runtime::solve_layout` 处理 percent + scale + 任意嵌套；删除 ui_pass.rs::resolve_position（错 anchor）；ui_pass 不再 layout，只接 `Vec<UiCommand>` 画 quad；81 测试 PASS（含 12 新 layout case）
    - **4.1.bis.3** 文字走 GuiRuntime — `WorldBinding::query_string(widget_name)` 覆盖 vanilla 占位符（pol_power / stability / war_support / manpower / industrial_capacity / fuel_value / convoys_count / experience_value / DateText 等）；`UiCommand::Text { format, max_width }` 透传 `.gui::format`；删除 main.rs 130 行硬编码 topbar 文字渲染
    - **4.1.bis.4** BmFont 默认 + 字号匹配 — `text_pass::load_font` 优先级反转；BmFont 抢 Body slot，fontdue 28/48 px 抢 Heading/Title slot；`prepare()` 完全重写，按 size 分组路由到对应 atlas；新增 `build_bmfont_verts` 抽出 BmFont 顶点构造（kerning + xadvance + xoffset/yoffset + 半 texel UV inset）
    - **4.1.bis.5** sprite 多帧 UV — `compute_frame_uvs(N)` 横向均分 + `draw_sprite(... frame)` 用 `frame_uvs[frame]` 切片；button hover/pressed 帧切换已工作；speed_step / world_tension_icon 数值驱动帧选择留 4.3+ 增量
    - **4.1.bis.6** Reference resolution + HiDPI — `with_inner_size(LogicalSize::new(1920, 1080))`；`init_render` 取 `window.scale_factor()` 算 logical = physical / dpi；ui_pass / text_pass / panel_pass / GuiRuntime 全用 logical（之前用 physical 导致 HiDPI 1.5× 显示器上 sprite 全挤左半屏）；`WindowEvent::Resized` 同步 4 个 pass 的 screen_size
    - **4.1.bis.7** 次级 .gui 白名单 — `GuiRuntime::set_visible_windows`/`window_roots`/`find_root`；`collect_commands` 按 root id 过滤；rebuild_topbar_runtime 加载 topbar + minimap + playable_state_view 三个根，但默认仅 topbar 渲染
  - 实测验证：`cargo build --workspace` 干净；启动日志 `[ui_pass] gfx_index loaded: 22946 sprite types`、`[text_pass] BmFont 'Ubuntu' 512x256 (333 glyphs)`、`[gui_runtime] built 112 widgets at 1920x1080 (scale 1.000)`
  - 关键发现：vanilla 自带的 `gfx/fonts/hoi_18mbs.fnt` 实际字面叫 'Ubuntu'（而非 hoi 自有字体）；HiDPI 1.5× 显示器上必须按 logical 计算 layout


---

## DLC 内容拆分总台账（2026-05-16）

> 用户要求"全部 51 个 DLC 都加入路线图"。已按已完成 / 未完成 phase 分流：
> 所有 DLC 内容**只塞进未完成 phase**（Phase 3.10 / 3.11 / 4 / 5 / 6 / 7 / 8 / 9 / 10），
> 不动已完成的 Phase 0 / 1 / 2 / 3.1-3.7 / 4.1 / 4.2。
>
> 表格按 **DLC 类型 → DLC 全名 → 缩写 → 拆分位置 → 主要新机制** 列；
> "拆分位置"是路线图里实际负责实现的章节号。
> Steam 安装目录扫描结果共 51 个 DLC（dlc001-dlc051）。

### A. 内容扩展（14 个，含核心玩法机制）

| 缩写 | 全名 | Steam dlc id | 主要新机制 | 拆分位置（实现） |
|---|---|---|---|---|
| **TfV** | Together for Victory | — (基础 DLC) | 自治领系统 / 英联邦国策树 / 装备租借 | 4.4（自治领焦点树）/ 4.7（自治领外交 + Lend-Lease）/ 5.1 数据加载 / 5.2 trigger / 5.3 effect / 6.x（自治需求模拟） |
| **DoD** | Death or Dishonor | — | 装备转换器 / 东欧国策树 | 4.4 / 5.1 / 5.2 / 5.3 |
| **WtT** | Waking the Tiger | — | 决议系统 / 指挥点 / 将领特性 / 中日重做 | 4.3（决议系统化）/ 4.4（中日焦点树）/ 4.8（CP / trait 面板）/ 5.1 / 5.2 / 5.3 / 6.3（将领 + CP）/ 4.6（核武研究）/ 4.11（核反应堆面板） |
| **MtG** | Man the Guns | — | 海军重做 + 舰船设计器 / 燃料 / 美英重做 / 出口许可 | 4.4（USA/ENG/MEX/HOL 树）/ 4.5（燃料 + 出口）/ 4.7（外交） / 4.8（任务系统 + task force + 演习）/ 4.11（核动力科技 4.6）/ 5.1 / 5.2 / 5.3 / 6.3（海军将领）/ 6.4（海战重做）/ 6.5（舰船设计器）/ 7.2（unit pack mesh） |
| **LaR** | La Résistance | dlc028_la_resistance | 谍报 / 情报局 / 抵抗 / 镇压 / 法西葡国策树 | 4.4（FRA/SPR/POR）/ 4.7（谍报面板）/ 4.11（抵抗 / 服从 / 特工详情）/ 5.1 / 5.2 / 5.3 / **6.6 全章谍报系统** / 7.6 头像 / 8.2 音效 |
| **BftB** | Battle for the Bosporus | dlc031_battle_for_the_bosporus | 土希保国策树 | 4.4（TUR/GRE/BUL）/ 5.1 / 5.2 / 5.3 |
| **NSB** | No Step Back | dlc034_no_step_back | 苏波国策树 / 坦克设计器 / 补给重做 / 和平会议重做 | 4.3（决议）/ 4.4（SOV/POL/EST/LAT/LIT）/ 4.5（坦克变体生产）/ 4.7（和平会议预览）/ 5.1 / 5.2 / 5.3 / **6.1 补给重做** / **6.4 战斗宽度重做** / **6.5 坦克设计器** / **6.9 全章和平会议重做** |
| **BBA** | By Blood Alone | dlc036_by_blood_alone | 意瑞埃国策树 / 飞机设计器 / 国联 | 4.4（ITA/SWI/ETH）/ 4.7（国联）/ 4.11（国联议程）/ 5.1 / 5.2 / 5.3 / 6.4（战略轰炸）/ **6.5 飞机设计器** / 7.2 unit pack |
| **AAT** | Arms Against Tyranny | dlc038_arms_against_tyranny | 北欧国策树 / 特种部队 / 军工组织 (MIO) / 雇佣兵 | 4.4（SWE/NOR/FIN/DEN）/ 4.5（MIO 面板）/ 4.6（特种部队科技）/ 4.8（特种部队上限 + 雇佣兵）/ 4.11（MIO 详情）/ 5.1 / 5.2 / 5.3 / **6.7 全章 MIO + 特种部队 + 雇佣兵** / 7.1（MIO 总部 mesh）|
| **ToA** | Trial of Allegiance | dlc040_trial_of_allegiance | 南美国策树 / 合法性系统 / 朝廷 | 4.3（合法性面板 + 顾问组）/ 4.4（ARG/BRA/CHL）/ 4.7（合法性外交效应）/ 4.11（合法性 + 朝廷弹窗 + 货币）/ 5.1 / 5.2 / 5.3 / **6.8 全章合法性 + 朝廷** |
| **Gott** | Götterdämmerung | dlc043_gotterdammerung | 德奥比国策树重做 / 秘密科技 / 装备分支 | 4.3（核心势力范围）/ 4.4（GER/AUS/BEL 重做 + load_focus_tree）/ 4.5（秘密武器生产）/ 4.6（秘密科技分支）/ 4.11（秘密武器 + 原子弹弹窗）/ 5.1 / 5.2 / 5.3 / 7.1（核反应堆 + 秘密设施 mesh） |
| **GoE** | Graveyard of Empires | dlc046_graveyard_of_empires | 阿伊伊国策树 / 中亚 / 部族系统 | 4.4（AFG/IRN/IRQ + 中亚）/ 5.1 / 5.2 / 5.3 / 7.6（头像）|
| **NCNS** | No Compromise, No Surrender | dlc049_no_compromise_no_surrender | 日本重做 / 满洲 / 中国战区改进 / 拓殖部 | 4.3（拓殖部）/ 4.4（JAP/MAN + CHI 改进）/ 4.11（科研院 / 设计局）/ 5.1 / 5.2 / 5.3 / 6.4（中国战区事件）/ 8.1（日本主题音乐） |
| **PoT** | Peace for Our Time | dlc051_peace_for_our_time | 绥靖路线 / 国联机制深化 | 4.4（GER/ENG/FRA 绥靖支线）/ 4.7（绥靖外交动作）/ 4.11（绥靖谈判窗口）/ 5.1 / 5.2 / 5.3 |

### B. 单位包（10 个，仅装备 mesh + 装备数据）

| 缩写 | 全名 | Steam dlc id | 拆分位置 |
|---|---|---|---|
| RLUP | Rocket Launcher Unit Pack | dlc003 | 7.2（mesh 接入）+ 5.1（装备定义） |
| FBUP | Famous Battleships Unit Pack | dlc004 | 7.2 + 5.1 |
| HCUP | Heavy Cruisers Unit Pack | dlc005 | 7.2 + 5.1 |
| STUP | Soviet Tanks Unit Pack | dlc006 | 7.2 + 5.1 |
| GTUP | German Tanks Unit Pack | dlc007 | 7.2 + 5.1 |
| FTUP | French Tanks Unit Pack | dlc008 | 7.2 + 5.1 |
| BTUP | British Tanks Unit Pack | dlc009 | 7.2 + 5.1 |
| USTUP | US Tanks Unit Pack | dlc010 | 7.2 + 5.1 |
| AAP | Axis Armor Pack | dlc025 | 7.2 + 5.1 |
| AlAP | Allied Armor Pack | dlc029 | 7.2 + 5.1 |
| EFPP | Eastern Front Planes Pack | dlc032 | 7.2 + 5.1 |
| PV | Prototype Vehicles | dlc047 | 7.2 + 5.1 |
| WotP | Warships of the Pacific | dlc050 | 7.2 + 5.1 |

### C. 头像 / 2D 美术包（3 个）

| 缩写 | 全名 | Steam dlc id | 拆分位置 |
|---|---|---|---|
| GHP | German Historical Portraits | dlc001 | 7.6（portrait DLC 接入）+ 5.1 |
| PCP | Polish Content Pack | dlc002 | 7.6 + 5.1 |
| CCPSU | CCP Soviet Union 2D Art | dlc042 | 7.6 + 5.1 |

### D. 音乐 + 语音包（8 个）

| 缩写 | 全名 | Steam dlc id | 拆分位置 |
|---|---|---|---|
| GMO | German March Order Music Pack | dlc011 | 8.1（DLC 音乐链） |
| ARM | Allied Radio Music Pack | dlc012 | 8.1 |
| Sab1 | Sabaton Soundtrack | dlc013 | 8.1 |
| OST | Original Soundtrack | dlc017 | 8.1 |
| Sab2 | Sabaton Vol. 2 | dlc019 | 8.1 |
| RP | Radio Pack | dlc026 | 8.1 |
| ASP | Allied Speeches Pack | dlc030 | 8.1 |
| SotEF | Songs of the Eastern Front | dlc033 | 8.1 |

### E. 装饰性 / 周边（5 个，路线图仅做 launcher 列出 + 资产链合并，**不接入引擎**）

| 缩写 | 全名 | Steam dlc id | 处理 |
|---|---|---|---|
| Sabaton Wallpaper | Sabaton Wallpaper | dlc014 | 9.1（DLC 启用 UI 列出，不接入运行时）|
| Artbook | Artbook | dlc016 | 9.1 |
| Anniv | Anniversary Pack | dlc021 | 9.1 |
| MtGWall | Man the Guns Wallpaper | dlc024 | 9.1 |
| ExpPass1Sup | Expansion Pass 1 Supporter Pack | dlc045 | 9.1 |

### F. 预购奖励（5 个，仅少量装饰物 / 头像，混入正式 DLC 资产链）

| 缩写 | 全名 | Steam dlc id | 处理 |
|---|---|---|---|
| LaRPre | La Résistance Preorder Bonus | dlc027 | 7.6（混入 LaR 头像 / 资产链）|
| NSBPre | No Step Back Preorder Bonus | dlc035 | 7.6（混入 NSB 资产链）|
| BBAPre | By Blood Alone Preorder Bonus | dlc037 | 7.6（混入 BBA）|
| AATPre | Arms Against Tyranny Preorder Bonus | dlc039 | 7.6（混入 AAT）|
| ToAPre | Trial of Allegiance Preorder Bonus | dlc041 | 7.6（混入 ToA）|

### G. 扩展通行证标记 + 小型衍生（3 个）

| 缩写 | 全名 | Steam dlc id | 处理 |
|---|---|---|---|
| ExpPass1Mus | Expansion Pass 1: Ride of the Valkyries | dlc044 | 8.1（音乐包）|
| ExpPass2Sup | Expansion Pass 2: Seaplane Tenders | dlc048 | 7.2（水上飞机母舰 mesh）+ 5.1 |
| Gott  preorder | Götterdämmerung Preorder | （含在 dlc043 中） | 7.6（混入 Gott） |

### 拆分原则

1. **不动已完成的 phase**（Phase 0 / 1 / 2 / 3.1-3.7 / 4.1 / 4.2）。
2. **基础设施先行**：DLC 探测层（`hoi4-paths::enumerate_dlcs` + `World.has_dlc`）放进 5.1，是所有 DLC 数据加载、trigger 门控、focus 路由的先决条件。
3. **机制先于内容**：补给重做 / MIO / 谍报 / 合法性 / 和平会议这种"系统性新机制"放 Phase 6（军事深度）；具体国策树 / 事件 / 决议放 Phase 4 + Phase 5（数据加载 + UI）。
4. **资产分层**：
   - 视觉（mesh / 头像 / 旗帜 / focus icon）→ Phase 7
   - 音频（音乐 / 音效 / 语音）→ Phase 8
   - 数据（events / decisions / focus / triggers / effects / ideas / advisors）→ Phase 5
5. **每个内容 DLC 都有"历史路径回归"**：Phase 9.2 列出 14 个内容 DLC 各自一条 1936-1939 历史路径 smoke 测试，通过即视为该 DLC 完成度可验收。

### 跟踪表（启动后填充）

> 引擎启动 banner 应输出形如：
> ```
> [dlc] Detected 14 content DLCs + 13 unit packs + 8 music packs (51 total)
>   ✓ Together for Victory (compat: TfV)
>   ✓ Death or Dishonor (compat: DoD)
>   ...
>   ✗ Götterdämmerung — focus tree load failed (see [scripts] log)
> ```
> 该 banner 的实现属于 Phase 5.1 DLC 探测层。
