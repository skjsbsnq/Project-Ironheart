# 地图渲染 V2 重构路线图

> 日期：2026-05-29
>
> 范围：地图渲染架构、视觉质量管线、资源校验、调试工具、截图验收体系。
>
> 非目标：不改玩法逻辑、AI、经济、内容脚本、普通 UI 面板行为。

## 1. 总结

当前地图显示效果差，不是因为某一个 shader 常量写错，也不是单纯缺少某个滤镜。更核心的问题是：项目已经堆出了很多类似 HOI4 的渲染部件，但这些部件没有被一个统一的地图渲染架构、资源质量标准和截图验收体系约束。

现在工程里已经有不少基础：

- 地形 atlas 采样。
- 水体 pass。
- 边界 pass。
- sky、trees、POI、地图名、省份名、箭头、frontline、particles。
- HDR 后处理、bloom、曝光、tonemap。
- 一些 vanilla shader 翻译结果。

但这些东西主要在 `crates/hoi4-app/src/main.rs` 中手工排顺序，通过 `terrain.wgsl` 里大量混合逻辑和各 pass 自己的 fallback 机制拼起来。最终效果容易出现：

- 地形和政治色互相污染。
- 省界、国界、选择高亮、占领条纹等语义效果重复叠加。
- 水体既在 terrain shader 中被着色，又被独立 WaterPass 覆盖。
- 边界既有 BorderPass，又有 terrain 内部 SDF 暗线。
- 资源缺失时仍然能运行，但视觉已经不具备评估价值。
- 后处理在没有固定基准图的情况下调参，容易掩盖底图问题。

要达到真正游戏级地图效果，应该把目标从“继续微调 shader”升级为“建立 Map Renderer V2”。第一阶段不是马上改漂亮，而是让地图质量变得可观测、可拆层、可回归、可定位。

## 2. 当前代码证据

以下是本路线图参考的主要代码位置。

| 区域 | 代码位置 | 含义 |
|---|---|---|
| 世界尺度 | `crates/hoi4-app/src/main.rs:79` `WORLD_SCALE = 0.02`，`:84` `HEIGHT_SCALE = 1.45`，`:91-92` chunk 为 `32 x 12` | 地图尺度是全局常量，但很多视觉参数分散在各 pass 和 shader 中。 |
| 地形 LOD | `crates/hoi4-render/src/terrain.rs:20` `LOD_GRID = [64, 32, 16]` | LOD 已存在，但它只是网格密度策略，不是完整画质策略。 |
| 相机近距限制 | `crates/hoi4-render/src/camera.rs:113` `min_dist = 3.0` | 已经有近景粗糙缓解，但不能解决整体渲染架构问题。 |
| 主 pass 顺序 | `crates/hoi4-app/src/main.rs:13056+` | sky、terrain、water、border、路线、counter、trees、建筑、POI、labels、arrows、particles、postprocess 都在主流程中手工组织。 |
| 地形颜色混合 | `crates/hoi4-app/src/passes/terrain.wgsl:489-507` | 政治色、季节色、colormap、atlas、zoom blend、map mode blend 在一个 fragment shader 中多次混合。 |
| terrain 内部省界 | `crates/hoi4-app/src/passes/terrain.wgsl:616-650` | 已经有 BorderPass，但 terrain shader 仍然做省界 SDF 暗化。 |
| terrain 内部水体 | `crates/hoi4-app/src/passes/terrain.wgsl:570-590` | terrain shader 仍然给水域像素着色，然后 WaterPass 再覆盖。 |
| WaterPass | `crates/hoi4-app/src/passes/water.rs` | 水体已独立，但依赖共享 height/province/coast 数据，并存在大量 fallback。 |
| BorderPass | `crates/hoi4-app/src/passes/border.rs` | 条带网格边界已存在，但边界贴图 fallback、mip 和层级策略仍需硬化。 |
| 资源策略 | `crates/hoi4-assets/src/vanilla_map_set.rs:207-208` | 多数资源允许缺失 fallback，这对健壮性有利，但对画质评估危险。 |
| 后处理 | `crates/hoi4-app/src/passes/postprocess.rs` | HDR、bloom、亮度 reduction、tonemap 已存在，但没有固定地图场景校准。 |

## 3. 主要问题诊断

### 3.1 架构问题

当前地图渲染更像“多个 pass 的堆叠”，不是一个有明确职责边界的 renderer。

最典型的问题在 `terrain.wgsl`：

- 它负责地形材质。
- 它负责政治色混合。
- 它负责季节、雪线、噪声、法线。
- 它负责水域基础色。
- 它负责河流 overlay。
- 它负责占领条纹。
- 它负责选中省份 pulse。
- 它负责省界 SDF 暗线。
- 它负责云影、雾、城市夜光、昼夜。
- 它还参与最终视觉的暗角式处理。

这会导致每次修一个视觉问题，都可能影响另一个视觉层。例如把地形调清晰，可能让政治色更脏；把边界调明显，可能和 terrain 内部省界重复；把水调蓝，可能被后处理曝光拉偏。

### 3.2 视觉组合问题

当前最终图像不是一个统一的美术方向，而是多个局部修补的叠加：

- 地形和政治色混合次数太多，容易脏。
- 省界和国界既有纹理/几何，又有 SDF 阴影，容易硬、黑、乱。
- 水体存在双重责任，容易出现海岸双边、泡沫和深浅水不一致。
- 河流目前为了避免 z-fighting 被放进 terrain shader，但这又让静态地图细节和地形材质耦合。
- 占领、选择、高亮、frontline、箭头、trade route、counter、label 都在后段叠加，远景很容易变成线条噪声。
- HDR 和曝光没有固定地图场景校准，容易把海水、雪地、夜景拉偏。

### 3.3 资源质量问题

当前 fallback 机制让程序更容易启动，但也让画质问题变得不可判定：

- 看起来糊，可能是 shader 问题。
- 看起来灰，可能是 colormap fallback。
- 边界不好，可能是 border texture fallback 或 mip 上传不足。
- 水不好，可能是 water normal、reflection、water colormap fallback。
- 地形没细节，可能是 terrain atlas normal 或 world_normal 缺失。

游戏级渲染需要区分两种状态：

- 可以启动：允许缺资源，用 fallback 保证开发流程不断。
- 可以评估画质：关键资源必须齐全，fallback 必须直接标记为不合格。

### 3.4 验证体系问题

没有稳定截图基准，就没有办法判断“改好了”还是“只是换了一种坏法”。

必须建立固定场景：

- 固定国家。
- 固定日期。
- 固定相机。
- 固定地图模式。
- 固定画质 preset。
- 固定输出层。

并且每次重构都要能输出 terrain-only、water-only、borders-only、overlays-only、labels-only、postprocess-off、final-full 等图。

## 4. V2 目标质量标准

Map Renderer V2 的目标不是完全复制 HOI4，而是达到同级别的工程和视觉标准。

目标如下：

1. 远景政治可读性：远景看国家、战线、海域，不应该看到满屏省份噪声。
2. 中景地形质感：山地、平原、沙漠、雪地、水域要有材质差异，但不能像重复贴图。
3. 近景稳定性：海岸、小岛、山地、边界不能出现大块马赛克、闪烁、z-fighting。
4. 水体统一性：浅水、深水、海岸泡沫、冰面、反射、法线应该像一个材质系统。
5. 边界层级：国界、州界、省界、海区、不可通行边界、选中边界必须有明确强弱关系。
6. overlay 克制：frontline、箭头、占领、路线、counter、POI、label 不应淹没底图。
7. 色彩一致性：纹理色彩空间、HDR、tonemap、gamma、曝光必须可解释。
8. 可调试：任何主要图层都能单独显示和截图。
9. 可回归：同一场景每次输出可比较，不依赖手工视角。

## 5. 目标架构

### 5.1 新的核心对象

建议逐步形成以下结构。

| 模块 | 职责 |
|---|---|
| `MapRenderer` | 地图渲染总入口，拥有 pass 顺序、frame resource、layer mask、quality preset、debug toggle。 |
| `MapRenderGraph` | 声明 pass 依赖、render target、执行顺序、调试输出。 |
| `MapRenderSettings` | 集中管理地图模式、画质 preset、边界显示、label 显示、debug 层。 |
| `MapAssetAudit` | 校验地图资源，输出缺失/fallback/质量状态。 |
| `MapFrameContext` | 每帧相机、时间、季节、地图模式、选中省份、zoom、屏幕尺寸。 |
| `TerrainMaterialSystem` | 地形材质输入、atlas 采样、地形/政治色混合策略。 |
| `WaterMaterialSystem` | 水体颜色、海岸泡沫、冰、法线、反射、海域高亮。 |
| `BorderSystem` | 边界提取、边界层级、贴图、宽度、淡出、选中效果。 |
| `StaticMapDecalSystem` | 河流、铁路、海岸静态细节、不可通行标记等。 |
| `OverlaySystem` | 占领、frontline、箭头、路线、地图模式 overlay、选择/hover。 |
| `MapLabelSystem` | 国家名、省份名、海域名、缩放门槛、避让、遮挡。 |
| `MapScreenshotHarness` | 固定场景截图、图层截图、像素统计、回归报告。 |

### 5.2 推荐 render graph

目标渲染顺序应集中声明，而不是散在主程序里：

```text
MapFrame
  01 Sky / atmosphere
  02 Terrain depth + terrain material
  03 Water material
  04 Static map decals: rivers, coast accents, impassable marks
  05 Border hierarchy
  06 Strategic overlays: occupation, supply, fronts, routes, arrows
  07 World objects: trees, buildings, railways, POI, counters
  08 Labels: country, state, province, sea regions
  09 Particles and transient effects
  10 HDR postprocess
  11 Debug overlay and UI
```

每个 pass 必须明确：

- 输入 texture/buffer。
- 输出 render target。
- depth state。
- blend state。
- debug view。
- quality preset 行为。
- fallback 行为。

### 5.3 画质 preset

不要再把画质散落在 shader 常量里。建议引入统一 preset：

| Preset | 目标 |
|---|---|
| `Low` | 保证稳定，降低树、粒子、label、边界细节。 |
| `Medium` | 默认可玩画质。 |
| `High` | 主要目标，面向正常玩家机器。 |
| `Ultra` | 截图级画质，更高 LOD、更多细节、更完整后处理。 |
| `Debug` | 强制开启图层隔离、false-color、fallback 可视化。 |

## 6. 分阶段路线图

### Phase 0：基准截图和资源审计

目标：停止靠肉眼猜。

状态：已完成并通过本机 smoke 验证。

完成记录：

- `--map-phase0-report-only` 可生成 `phase0_latest.json`、`phase0_latest.txt` 和 `asset_audit_latest.json`。
- `--map-phase0` 可加载世界、固定相机批量截图并自动退出。
- 当前 baseline 输出位于 `target/map_baseline`，包含 10 个固定场景、8 个图层、80 张 PNG。
- 当前资源审计为 71/71 loaded、0 missing、quality=`Valid`、visual_review_usable=true。
- 每条 capture 都记录 `frame_time_ms`，并携带对应 `MapLayerMask` 和资源可验收状态。

工作内容：

1. 建立固定截图场景：
   - 欧洲远景政治视图。
   - 德国/波兰中景。
   - 意大利和亚得里亚海海岸。
   - 英吉利海峡。
   - 阿尔卑斯近景。
   - 日本和朝鲜近景。
   - 北非沙漠。
   - 太平洋深海。
   - 苏联冬季雪地。
   - 战争前线场景。
2. 每个场景输出：
   - `final_full`。
   - `postprocess_off`。
   - `terrain_only`。
   - `water_only`。
   - `borders_only`。
   - `overlays_only`。
   - `labels_only`。
   - `asset_fallback_debug`。
3. 建立 `MapAssetAudit`：
   - 资源总数。
   - 已加载数量。
   - 缺失数量。
   - fallback 列表。
   - 关键资源质量状态。
   - 按类别统计。
4. 增加 debug layer toggle：
   - 只看地形。
   - 只看水体。
   - 只看边界。
   - 只看 overlay。
   - 只看 label。
   - 后处理开关。
   - fallback 可视化。
5. 记录每个场景 frame time。

验收标准：

- 可以重复输出同一批截图。
- 资源 fallback 会出现在日志、debug overlay 或报告文件中。
- 缺关键资源时，截图被标记为不可用于视觉验收。

预计工作量：3 到 5 天。

风险：低。此阶段不应重写视觉。

### Phase 1：MapRenderer 所有权重构

目标：把地图 pass 调度从 `main.rs` 拆到专门的 `MapRenderer`。

状态：已完成并通过 Phase 0 回归截图验证。

完成记录：

- 新增 `crates/hoi4-app/src/map_renderer.rs`，集中定义 `MapRenderer`、`MapFrameContext`、`MapRenderGraph` 和 `MapPassDrawSet`。
- `PassRegistry` 由 `MapRenderer::register_passes` 根据 Phase 1 graph 初始化，不再在 `main.rs` 手写完整 pass 注册表。
- 每帧通过 `MapRenderer::build_frame_plan` 结合 `MapRenderSettings`、`MapLayerMask`、quality preset 和 registry toggle 生成 draw plan；Phase 11 已移除 `TerrainRenderPath`。
- terrain、water、border、sky、trees、rail/building、POI、labels、arrows/frontlines、particles、postprocess 的开关已接入 `MapPassDrawSet`。
- Phase 1 曾保留 legacy terrain fallback 作为迁移保护；Phase 11 已移除该路径，terrain 现在必须经过 `TerrainPass`。
- Phase 1 后已重新输出 `target/map_baseline_phase1`，包含 10 个固定场景、8 个图层、80 张 PNG；资源审计仍为 quality=`Valid`。

工作内容：

1. 新增 `MapRenderer`。
2. 新增 `MapFrameContext`。
3. 逐步迁移：
   - terrain pass。
   - water pass。
   - border pass。
   - sky pass。
   - trees。
   - rail/building。
   - POI。
   - labels。
   - arrows/frontlines。
   - particles。
   - postprocess 连接点。
4. 初始行为保持不变，先不调画面。
5. 引入 layer mask，但默认全部开启。
6. legacy terrain fallback 保留为显式兼容路径。

验收标准：

- 重构前后截图基本一致。
- `main.rs` 不再直接维护完整地图 pass 顺序。
- 所有地图层能从一个 registry/settings 里开关。

预计工作量：5 到 8 天。

风险：中。主要风险是 pass 顺序、depth state、bind group 生命周期改变。

### Phase 2：资源契约和 fallback 策略

目标：区分“能启动”和“画质有效”。

状态：已完成并通过本机资源审计和针对性测试。

完成记录：

- `MapAssetAudit` 已拆分资源契约：`StartupRequired`、`BaseMapRequired`、`HighQuality`、`Optional`，并增加 `MapAssetFallbackPolicy` 区分启动阻断、视觉验收阻断、降级允许和可选 fallback。
- 质量状态固定为 `Valid`、`Degraded`、`FallbackOnly`、`InvalidForVisualReview`；`FallbackOnly` 与关键资源 fallback 都不会通过视觉验收。
- 审计条目现在记录 DDS `mip_status`、格式、尺寸、source/upload mip 数、block 信息和 fallback reason；atlas、水体、边界贴图状态可在 JSON/TXT 中直接查到。
- terrain、water、border、river 以及共享 `hoi4-render::texture_upload` 改为复用统一 `dds_upload_plan`，BC mip copy extent 统一按 block 对齐。
- 新增 `--map-audit [--map-audit-output <DIR>]`，默认输出 `target/map_audit/latest.json` 和 `latest.txt`。
- Phase 0 capture manifest 的每张截图都记录 `asset_quality` 与 `asset_fallback_count`。
- 当前 `target/map_audit/latest.json` 审计为 71/71 loaded、0 missing、0 fallback、quality=`Valid`、visual_review_usable=true，62 个 DDS 条目 mip 状态均为 `Complete`。

工作内容：

1. 资源分级：
   - 启动必需：provinces、heightmap、terrain index。
   - 底图必需：terrain atlas、colormap、water colormap。
   - 高质量必需：terrain normal atlas、world_normal、water normals、border textures、river textures、city lights。
   - 可选：粒子、部分装饰 mesh、特殊 overlay。
2. 引入质量状态：
   - `Valid`。
   - `Degraded`。
   - `FallbackOnly`。
   - `InvalidForVisualReview`。
3. 对画质关键资源，不允许静默 1x1 fallback。
4. terrain、水体、边界 DDS mip 上传策略统一。
5. 输出审计文件，例如 `target/map_audit/latest.json`。
6. 为资源枚举和 fallback 分类加测试。

验收标准：

- 每张截图都能对应一份资源质量状态。
- 关键资源 fallback 时不能通过视觉验收。
- atlas、水体、边界贴图的 mip 状态可见。

预计工作量：4 到 7 天。

风险：中。部分开发环境可能没有完整 vanilla 资源，需要保留 degraded 启动模式。

### Phase 3：地形材质重写

目标：把 `terrain.wgsl` 从“大杂烩 shader”重构为可校准的地形材质系统。

状态：已完成并通过 shader validation、frame-plan ownership 测试和 Phase 0 回归截图验证。

完成记录：

- `terrain.wgsl` 已拆出 `TerrainMaterialWeights`、`TerrainMaterial`、`build_terrain_material` 和 `terrain_debug_color`，把 albedo、normal、snow、detail、river mask、height/slope 等材质量从最终片元组合中分离。
- 地形混色改为命名权重：`terrain_albedo_weight`、`political_tint_weight`、`season_weight`、`detail_weight`、`snow_weight`、`map_mode_weight`，political/terrain 模式仍由 `map_mode_terrain_blend` 驱动。
- 新增 `TerrainDebugView` 和 `PdxMapParams.terrain_controls`，F6 可运行时切换 terrain id、atlas tile id、政治色、纯地形 albedo、normal、height/slope、snow mask、river mask，不需要改 WGSL。
- `MapPassDrawSet::terrain_material_ownership` 明确了材质 owner：当专用 WaterPass/BorderPass 已加载且本帧启用时，terrain 不再负责最终水体颜色和普通 SDF 省界/国界暗化，只保留 fallback owner。
- terrain-only 图层禁用 terrain 内部河流、占领和选择类 overlay，避免 terrain-only 截图被 overlay 污染。
- `terrain_wgsl` 测试升级为 naga parse + validation，并增加 Phase 3 token 回归；`map_renderer` 增加 terrain ownership 单元测试。
- 已重新输出 `target/map_baseline_phase3`，包含 10 个固定场景、8 个图层、80 张 PNG；审计为 71/71 loaded、0 missing、0 fallback、quality=`Valid`。

当前问题：

`terrain.wgsl` 同时处理地形、政治色、水体、河流、占领、选择、省界、云影、雾、城市夜光、昼夜和边缘修正。职责过多导致任何调参都不稳定。

目标结构：

```text
TerrainMaterial
  输入:
    height, slope, terrain id, atlas id, colormap, season, map mode
  材质:
    albedo, normal, detail, snow, roughness-like control
  地图模式:
    political tint, terrain tint, debug false color
  环境:
    day-night, cloud shadow, fog
  输出:
    terrain HDR color + depth
```

工作内容：

1. terrain 不再负责最终水体颜色。
2. terrain 不再在最终模式负责普通省界/国界暗化。
3. occupation、selected province pulse 尽量迁到 OverlaySystem 或 BorderSystem。
4. 把嵌套 `mix()` 改成命名权重：
   - `terrain_albedo_weight`。
   - `political_tint_weight`。
   - `season_weight`。
   - `detail_weight`。
   - `snow_weight`。
   - `map_mode_weight`。
5. 增加 debug output：
   - terrain id。
   - atlas tile id。
   - 政治色。
   - 纯地形 albedo。
   - normal。
   - height/slope。
   - snow mask。
   - river mask。
6. 按远景、中景、近景分别校准。
7. 材质清理后再评估 LOD_GRID 和 chunk 密度。

验收标准：

- terrain-only 截图在无边界、无水体、无 overlay 时仍然自然。
- political 模式不变成纯色块。
- terrain 模式不被政治色污染。
- 近景有细节，但不出现明显重复棋盘纹。
- debug 模式不需要改 WGSL 即可切换。

预计工作量：8 到 14 天。

风险：高。这是核心视觉重写。

### Phase 4：水体系统重写

目标：让水体成为唯一、完整、统一的材质系统。

状态：已完成并通过 shader validation、frame-plan ownership 测试和 Phase 0 回归截图验证。

完成记录：

- `WaterPass` 已增加 `WaterMaterialSystem` 资源清单，统一声明 water color、LEAN normal、specular mask、reflection、ice diffuse/noise 等材质输入，并记录 loaded/fallback/critical_missing 统计。
- `WaterParams` 扩展到 64 字节，新增 `debug_view`、`final_water_owner` 和 `debug_controls`；运行时由实际 frame plan 写入，WaterPass 未加载或未绘制时不会抢占 terrain fallback。
- WGSL 已拆出 `WaterMaterial`、`build_water_material` 和 `water_debug_color`，水体颜色、浅深过渡、海岸 foam、normal strength、specular、ice、reflection、海域选中高亮集中在 WaterPass。
- 新增 `WaterDebugView`，F10 可切换 depth ratio、coast distance、normal strength、foam mask、ice mask、reflection contribution、final water only；F4 overlay 同时显示 terrain/water debug 名称。
- `MapPassDrawSet::water_material_ownership` 明确 WaterPass 对最终水色、浅深、foam、normal、specular、ice、reflection 和海域选中高亮的完整所有权；terrain ownership 继续只保留 fallback。
- 水体 DDS 加载复用 `dds_upload_plan`，mip copy extent 与 Phase 2 的 block-aligned 策略一致；sRGB/linear 由 `MapResRole::is_srgb()` 统一决定。
- 已重新输出 `target/map_baseline_phase4`，包含 10 个固定场景、8 个图层、80 张 PNG；审计为 71/71 loaded、0 missing、0 fallback、quality=`Valid`。

工作内容：

1. 明确 WaterMaterialSystem 负责：
   - 水体颜色。
   - 浅水/深水过渡。
   - 海岸 foam。
   - normal。
   - specular。
   - ice。
   - reflection。
   - 海域选择高亮。
2. final-quality 模式下禁用 terrain shader 的水体最终颜色。
3. 海岸泡沫和岸线使用同一套 coast distance 策略。
4. 用固定场景校准：
   - 地中海。
   - 英吉利海峡。
   - 太平洋。
   - 日本小岛。
   - 冰海。
5. 统一 water texture 的 mip、sampler、色彩空间。
6. 增加 water debug：
   - depth ratio。
   - coast distance。
   - normal strength。
   - foam mask。
   - ice mask。
   - reflection contribution。
   - final water only。

验收标准：

- 海岸没有双边、棕色环、闪烁。
- 水色在后处理开关下稳定。
- 小岛和窄海清晰但不锯齿。
- 冰面不是随机覆盖层。

预计工作量：5 到 9 天。

风险：中高。海岸和深度交互很敏感。

### Phase 5：边界系统重写和层级化

目标：让边界清晰、有层级、不形成像素网格。

状态：已完成并通过 BorderPass 单元测试、WGSL naga validation、`cargo check -p hoi4-app` 验证。

已落地：

- BorderPass 成为 final-quality 边界 owner；terrain SDF 边界只保留 fallback 路径。
- `BorderKind` 扩展到 country/state/province/sea/sea-region/impassable 六类，并在顶点中携带省份对和 kind。
- shader 内实现层级颜色、alpha、zoom gate、screen-space width clamp、远景 fade 和选中省份 accent。
- 增加 border debug view：country/state/province/sea/selected/false-color hierarchy，并接入 `V` 快捷键和 debug overlay。
- border texture LOD/格式路径继续复用 Phase 2 DDS mip 上传策略，并补充 BC5 支持。
- 无 sea-region 数据时不再把所有海省内部边界误画成海区网格。

工作内容：

1. 让 BorderSystem 成为唯一边界所有者。
2. final-quality 模式下禁用 terrain 内部省界 SDF 暗化。
3. 定义边界层级：
   - 国界最强。
   - 州界中等。
   - 省界最弱，并按 zoom 显示。
   - 海区边界单独风格。
   - 不可通行边界特殊风格。
   - 选中边界作为独立 accent。
4. 宽度和淡出尽量基于 screen-space，而不是只按世界距离。
5. 审计 border texture 加载和 mip。
6. 增加抗锯齿策略：
   - texture alpha。
   - screen-space width clamp。
   - shader AA。
   - 远景 fade。
7. 增加 border debug：
   - country only。
   - state only。
   - province only。
   - sea only。
   - selected only。
   - false-color hierarchy。

验收标准：

- 远景只强调国家，不显示满屏省份格子。
- 中景能看州/省，但不污染地形。
- 近景边界清楚但不是黑裂缝。
- 选中省份清晰，不压过正常地图。

预计工作量：6 到 10 天。

风险：高。边界决定战略地图的基本观感。

### Phase 6：河流、铁路和静态地图 decal

目标：把静态地图细节做成受控系统，而不是散在 terrain 和独立 pass 之间。

状态：已完成并通过 `map_renderer` frame-plan 测试、railway WGSL validation、`cargo check -p hoi4-app` 验证。

已落地：

- 新增 `StaticMapDecalSystem` frame-plan 决策层，统一管理 rivers、roads、railways、shore accents、impassable marks、infrastructure overlay、trade routes、straits 的可见性和 opacity budget。
- 河流采用稳定路线：继续由 terrain 内部采样 `rivers.bmp`，RiverPass 保留但不进入 final-quality 绘制，避免 depth bias / z-fighting 导致的闪烁和棕线。
- 铁路改为受 zoom gate 和 map mode 控制的半透明静态 decal；Infrastructure/Supply/Factories 模式提高铁路可见性，远景隐藏，避免盖住底图。
- 铁路 shader 增加 `RailwayParams` uniform、alpha blending、naga validation，并把绘制顺序前移到静态 decal 段，低于 counters/objects。
- terrain-only baseline 保持无 static decal；static_decals layer 才启用 terrain-owned rivers 等基础静态细节。

工作内容：

1. 明确河流归属：
   - 保留 terrain 内稳定采样。
   - 或修复 RiverPass 的 depth bias 和 z-fighting。
2. 新增或整理 `StaticMapDecalSystem`：
   - rivers。
   - roads。
   - railways。
   - shore accents。
   - impassable marks。
   - 地图模式相关基础设施 overlay。
3. decal 需要 zoom gate 和 map-mode-aware。
4. decal 不能和边界、地形材质抢视觉主导权。

验收标准：

- 河流可见但不闪烁。
- 河流不应显示成异常棕线。
- 铁路/道路远景不应盖住底图。
- decal 后处理后仍然可读。

预计工作量：4 到 7 天。

风险：中。

### Phase 7：语义 overlay 和降噪

目标：避免 gameplay overlay 毁掉地图可读性。
状态：已完成并通过 frame-plan、shader validation 和 `cargo check -p hoi4-app` 验证。

已落地：

- 新增 `SemanticOverlaySystem` frame-plan 决策层，统一管理 occupation stripes、frontlines、arrows、trade routes、straits、selected province pulse、hover highlight 和 map-mode overlay 的可见性、opacity budget 与优先级。
- overlay pass gate 改为由 `SemanticOverlayPlan` 控制；trade routes/straits 不再只依赖 static decal 计划，arrows/frontlines 按语义 overlay 预算决定是否绘制。
- terrain 内部 occupation/selection/map-mode overlay 仍保留为兼容 owner，但强度改由 `PdxMapParams.overlay_controls` 写入，避免占领条纹和地图模式 overlay 抢底图主导权。
- 新增 hover province tracking，并在 terrain shader 中接入轻量 hover rim；selected/hover priority 高于 passive overlay。
- `MapArrowPass`、`TradeRoutePass`、`StraitPass` 和旧 `frontlines` mesh shader 均接入 per-frame opacity uniform；arrows 保持高于 frontlines，frontlines/trade/strait 受 zoom gate 和 map-mode budget 约束。
- F4 debug overlay 增加当前 semantic overlay budget 显示；overlay-only 截图继续通过 `MapLayerMask::OverlaysOnly` 走 overlays/static_decals/objects/particles 组合。

工作内容：

1. 统一管理：
   - occupation stripes。
   - frontlines。
   - arrows。
   - trade routes。
   - straits。
   - selected province pulse。
   - hover highlight。
   - supply 或地图模式 overlay。
2. 建立 zoom gate 和优先级：
   - 战略层远景可见。
   - 省份细节近景可见。
   - selected/hover 永远高于普通 passive overlay。
3. 每种 map mode 有 opacity budget。
4. 建立冲突规则：
   - arrows 高于 frontlines。
   - selected border 高于 province border。
   - counters 高于多数 map overlay。
   - labels 高于地形，但低于关键 UI。
5. 支持 overlay-only 截图。

验收标准：

- 远景不是满屏等权重线条。
- 战争前线可读。
- 占领条纹不把 terrain 搅脏。
- 切换地图模式时 overlay 强度可预测。

预计工作量：5 到 8 天。

风险：中。

### Phase 8：标签、counter 和世界物件

目标：让 label、counter、POI、树、建筑像地图的一部分，而不是贴上去的调试元素。
状态：已完成并通过 frame-plan、shader validation 和 `cargo check -p hoi4-app` 验证。

已落地：

- 新增 `WorldObjectSystem` frame-plan 决策层，统一管理 country labels、province labels、counters、POI、buildings 和 trees 的可见性、opacity、scale、priority、zoom gate 与 map-mode budget。
- 国家名接入 frame-plan opacity/scale；省份名默认只在近景显示，并按实例面积做 declutter，避免 labels-only 或近景模式变成 debug text。
- HOI3 counter 接入 Phase 8 的 screen-space scale、opacity 和 layout density；counter 仍高于大多数地图 overlay，密集前线会先按规划尺寸参与避让再上传 GPU。
- POI 图标改为使用 frame-plan detail level、opacity、scale 和描边强度；Factories/Infrastructure/Supply 等地图模式提高建筑/POI 可见性。
- 建筑 mesh 与 fallback billboard 都接入 opacity/scale/brightness 控制；fallback billboard 不再复用 railway params bind group。
- TreeFull、legacy billboard trees 和 mesh trees 接入 object opacity/scale；远景通过 zoom gate 隐藏树，避免树点噪声压住战略视图。

工作内容：

1. 国家名：
   - 按国家屏幕面积缩放。
   - 稳定旋转或曲线。
   - 遮挡和 fade。
   - 欧洲远景避免过密。
2. 省份名：
   - 默认近景才显示。
   - 强 declutter。
   - 不和 counter/POI 重叠。
3. counter：
   - screen-space 尺寸策略。
   - front density 下堆叠/避让。
   - 高于大多数地图 overlay。
4. POI/建筑：
   - map-mode visibility。
   - zoom gate。
   - 图标亮度和描边校准。
5. trees：
   - 远景不能像噪声。
   - 近景不应暴露低模感。

验收标准：

- 1936 欧洲远景可读。
- 战争前线 counter 不乱。
- label 缩放时不明显闪烁。
- 省份名启用时不像 debug text。

预计工作量：6 到 12 天。

风险：高。label/counter 直接影响可玩性。

### Phase 9：色彩管线和后处理校准

目标：最终输出稳定、可解释。

状态：已完成，并通过 frame-plan、shader validation、texture upload tests 和 `cargo check` 验证。

工作内容：

1. 明确所有 texture 色彩空间：
   - albedo/color 使用 sRGB。
   - normal/mask/data 使用 linear。
   - HDR target 使用 linear HDR。
   - swapchain 输出正确 gamma。
2. 审计 texture loader 的 sRGB 标记。
3. 固定场景校准：
   - exposure clamp。
   - ACES 参数。
   - bloom threshold/strength。
   - saturation。
   - vignette。
   - fog color/distance。
4. 增加 postprocess debug：
   - HDR raw。
   - tonemap only。
   - bloom only。
   - final。
5. 禁止用后处理掩盖底图错误。postprocess off 时，terrain/water 仍应基本成立。

验收标准：

- 后处理开启是锦上添花，不是遮丑。
- 海水不会因曝光变成荧光蓝或灰白。
- 政治色不刺眼。
- 雪、雾、夜光不压死细节。

完成记录：

- `PostProcessCalibration` 已显式固化 Phase 9 校准参数：exposure clamp `[0.6, 1.3]`、middle grey `0.18`、ACES input scale `0.6`、bloom threshold `1.05`、final bloom strength `0.05`、saturation `0.98`、vignette `0.13`。
- `PostProcessDebugView` 已接入 `final`、`hdr_raw`、`tonemap_only`、`bloom_only`，并集成 Shift+F5 循环切换和 F4 overlay 显示。
- baseline layer 已增加 `hdr_raw`、`tonemap_only`、`bloom_only` 调试捕获层；固定 10 个场景时，计划输出为 10 x 11 = 110 张 PNG。
- shared `TextureUploadHelper` 已改为按 `MapResRole::is_srgb()` 选择 sRGB/linear GPU format，避免 BC1/BC3 mask/data 资源被错误当作 sRGB。
- restore shader 已增加 naga parse/validation 测试；Phase 9 相关 postprocess、baseline layer mask、frame-plan 和 texture upload 测试已通过。
- `postprocess_off` 仍走显式 simple blit baseline layer，确保关闭后处理时 terrain/water 仍可独立验收。

预计工作量：4 到 8 天。

风险：中。

### Phase 10：性能和显存预算

目标：高画质必须可运行、可度量。

状态：已完成，并通过 Phase 10 预算/overlay 单元测试、frame-plan 质量 preset 测试和 `cargo check` 验证。

工作内容：

1. 建立预算：
   - 1080p frame time。
   - 1440p frame time。
   - draw call 数。
   - GPU pass time。
   - CPU culling/layout time。
   - texture memory。
2. 支持 GPU timestamp。
3. 统计每个 pass：
   - terrain。
   - water。
   - borders。
   - overlays。
   - labels。
   - objects。
   - postprocess。
4. quality preset 影响：
   - LOD density。
   - tree density。
   - particle count。
   - border detail。
   - label density。
   - postprocess chain。

验收标准：

- High preset 在目标机器稳定。
- Ultra 可以更贵，但必须被测量。
- debug overlay 显示 pass timing。
- 画质提升不能悄悄让帧时间翻倍。

完成记录：

- 新增 `MapQualityPreset` / `MapQualityControls` / `MapPerformanceBudget`，固化 High 与 Ultra 的 1080p/1440p frame time、GPU pass time、CPU prepare、draw call、texture memory 预算。
- `MapRenderSettings` 已接入质量 preset；High/Ultra 会影响 label/object/POI/particle/postprocess 控制，Shift+F8 可在运行时切换，F4 overlay 会显示当前 quality。
- `PassRegistry` 已扩展为 Phase 10 统计载体，逐 pass 记录 CPU ms、GPU ms、draw calls，并汇总 total CPU/GPU/draw call。
- 新增可选 `GpuTimestampProfiler`：当设备支持 `TIMESTAMP_QUERY` 和 pass/encoder timestamp writes 时写入 `wgpu::QuerySet`，异步 readback 后填充每个 pass 的 GPU ms；不支持时 overlay 明确显示 `gpu_timestamp=unsupported`。
- F4 debug overlay 已显示 Phase 10 预算状态：frame target、CPU prepare、pass CPU/GPU、draw call、frame texture memory 估算，以及 GPU timestamp 状态。
- 显存预算当前覆盖 frame-local HDR target、depth、bloom chain 和 luminance chain；静态资源显存仍由后续 asset/profiling 文档继续细化。

预计工作量：4 到 7 天。

风险：中。

### Phase 11：清理、文档和旧路径下线

目标：让地图渲染可维护。

状态：已完成，并通过旧 terrain fallback 下线回归测试、pass graph 测试和 `cargo check` 验证。

工作内容：

1. 删除 V2 稳定后不再需要的旧 shader 分支。
2. 删除重复水体和重复边界职责。
3. 为每个 pass 写文档：
   - inputs。
   - outputs。
   - depth/blend state。
   - fallback policy。
   - debug views。
   - known limitations。
4. 旧路线图标记为历史文档，并指向本 V2 路线图。
5. 增加贡献说明：
   - 如何新增 map layer。
   - 如何新增 debug view。
   - 如何更新截图 baseline。
   - 如何分类资源。

验收标准：

- 新人不用读完整 `main.rs` 就能理解地图渲染。
- pass 顺序、画质设置、debug 层有单一来源。
- legacy 路径明确、有限、可删除。

完成记录：

- 移除 `TerrainRenderPath` / `LegacyCompat` frame context 分支，`MapRenderer::build_frame_plan` 不再选择旧 terrain shader fallback。
- 移除 app 侧旧 `SHADER_MAIN_WGSL` terrain pipeline、bind groups 和 `render_legacy_terrain()`；`RenderState` 现在强制持有 `TerrainPass`。
- `hoi4-render` 只保留 `ARCHIVED_SHADER_MAIN_WGSL` 作为历史参考，并下线 `SHADER_MAIN_WGSL` 兼容别名。
- `TerrainPass` 成为唯一 terrain pipeline；剩余 fallback 限定为 pass 内 1x1 texture、空 instance、程序化占位或资源审计可见的 asset fallback。
- Water/Border 的常规最终职责归 `WaterPass` / `BorderPass`；terrain 只保留 dedicated pass 不可用时的有限 fallback owner 位。
- 新增 [`docs/map_renderer_v2.md`](docs/map_renderer_v2.md)，记录 pass 输入、输出、depth/blend、fallback、debug view 和限制。
- 新增 [`docs/map_renderer_v2_contributing.md`](docs/map_renderer_v2_contributing.md)，记录新增 map layer、debug view、baseline 和资源分类流程，并从 [`CONTRIBUTING.md`](CONTRIBUTING.md) 链出。
- `ROADMAP_MAP_VISUAL_PARITY.md`、`ROADMAP_DEPTH.md`、`ROADMAP_TERRAIN_CLOSEUP_QUALITY.md`、`docs/shader_audit.md`、`docs/legacy/ROADMAP_V3.md` 和 `docs/legacy/ROADMAP_V3_APPENDIX.md` 已标记为历史文档并指向当前 V2 文档。

预计工作量：3 到 6 天。

风险：低到中。

## 7. 里程碑

### Milestone A：可观测地图渲染

包含：

- Phase 0。
- Phase 2 的最小版本。

产出：

- 截图 harness。
- 资源审计。
- 图层开关。
- baseline 报告。

这是后续一切画质重构的前置条件。

### Milestone B：MapRenderer V2 骨架

包含：

- Phase 1。

产出：

- `MapRenderer`。
- `MapFrameContext`。
- 中央 pass 顺序。
- layer mask。
- 与 baseline 基本一致的画面。

### Milestone C：干净底图

包含：

- Phase 3。
- Phase 4。
- Phase 5。

产出：

- 干净地形。
- 统一水体。
- 层级化边界。
- 完整 debug view。

这是最大画质跃迁。

### Milestone D：可玩的战略地图

包含：

- Phase 6。
- Phase 7。
- Phase 8。

产出：

- 稳定河流和静态 decal。
- 克制 overlay。
- 可读 label/counter。

### Milestone E：可长期维护的高画质

包含：

- Phase 9。
- Phase 10。
- Phase 11。

产出：

- 校准色彩和后处理。
- 性能预算。
- 清理旧路径。
- 文档。

## 8. 推荐执行顺序

不要一上来重写 `terrain.wgsl`。推荐顺序：

1. 建立截图 baseline 和资源审计。
2. 在不改变画面的前提下迁移到 `MapRenderer`。
3. 加 layer mask 和 debug view。
4. 固化 fallback 策略。
5. 重写 terrain material。
6. 让 water 成为唯一水体 owner。
7. 让 border 成为唯一边界 owner。
8. 把 overlay 从 terrain 中剥离。
9. 校准 label/counter/postprocess。
10. 有截图证明后再删除 legacy 分支。

## 9. 固定验收场景

| 场景 | 主要检查 |
|---|---|
| 西欧远景 | 政治可读性、国界、国家名、counter 密度。 |
| 德国/波兰中景 | 省份密度、州界、省界、选中省份、counter。 |
| 意大利和亚得里亚海 | 海岸、小岛、浅水、国界。 |
| 英吉利海峡 | 水体、海域、海岸 foam、label。 |
| 阿尔卑斯近景 | 高度、雪、地形材质、LOD 稳定性。 |
| 北非 | 沙漠纹理重复、政治色、海岸过渡。 |
| 日本和朝鲜 | 小岛、coast SDF、边界精度。 |
| 太平洋深海 | 水色、曝光、海区边界。 |
| 苏联冬季 | 雪线、季节、雾和亮度。 |
| 活跃战争前线 | frontline、箭头、counter、占领、粒子。 |

每个场景至少输出：

- `final_full`。
- `postprocess_off`。
- `terrain_only`。
- `water_only`。
- `borders_only`。
- `overlays_only`。
- `labels_only`。
- `asset_fallback_debug`。

## 10. 质量门槛

### Gate 1：资源有效性

以下情况截图不能用于视觉验收：

- critical terrain atlas fallback。
- water colormap 或 water normal fallback。
- border textures fallback。
- heightmap/province map/terrain index 无效。
- texture 色彩空间未知。

### Gate 2：图层隔离

必须能单独截图：

- terrain。
- water。
- borders。
- static decals。
- overlays。
- labels。
- objects。
- postprocess。

### Gate 3：无重复所有权

同一种语义效果只能有一个 owner：

- 水体颜色属于 WaterMaterialSystem。
- 普通省界/国界属于 BorderSystem。
- 占领属于 OverlaySystem。
- 选中高亮属于 OverlaySystem 或 BorderSystem，只能选一个。
- 河流属于 StaticMapDecalSystem 或 TerrainMaterialSystem，必须明确。

### Gate 4：可读性

每个验收场景必须满足：

- 远景不显示满屏省份网格噪声。
- 近景无明显 z-fighting。
- terrain 模式不脏。
- political 模式不丢地形形体。
- label 不大面积重叠。
- water 在后处理开关下稳定。

### Gate 5：性能

每个里程碑都要记录：

- frame time。
- pass timing 或 draw call 数。
- map texture 显存估计。
- 与上一 baseline 的对比。

## 11. 文件级重构目标

### `crates/hoi4-app/src/main.rs`

目标：

- 移除详细地图 pass 编排。
- 保留 app phase、UI、input、frame lifecycle。
- 地图渲染委托给 `MapRenderer`。

风险：

- 当前此文件协调大量资源，必须小步迁移。

### `crates/hoi4-app/src/passes/terrain.wgsl`

目标：

- 收敛为地形材质和地形环境效果。
- V2 final 模式禁用水体最终颜色。
- V2 final 模式禁用普通省界暗化。
- overlay 尽量迁出。
- 增加 debug output。

风险：

- 视觉风险最高，必须截图驱动。

### `crates/hoi4-app/src/passes/terrain.rs`

目标：

- 只拥有 terrain pipeline 资源。
- 接收 `TerrainMaterialSettings`。
- 暴露 debug bind 和 quality preset。
- 不静默 fallback 画质关键资源。

### `crates/hoi4-app/src/passes/water.rs`

目标：

- 拥有全部水体颜色和水体效果。
- 提供 foam、depth、normal、reflection、ice debug view。
- 收紧 fallback 和 mip 策略。

### `crates/hoi4-app/src/passes/border.rs`

目标：

- 拥有全部边界视觉。
- 增加层级、zoom fade、screen-space width 策略。
- 收紧 fallback 和 mip 策略。

### `crates/hoi4-app/src/passes/postprocess.rs`

目标：

- 以固定地图场景校准。
- 暴露后处理阶段 debug。
- 不用于掩盖底图错误。

### `crates/hoi4-assets/src/vanilla_map_set.rs`

目标：

- 在 `allow_missing` 之上增加画质分类。
- 输出 audit report。
- 区分“可启动”和“可视觉验收”。

### `crates/hoi4-render/src/terrain.rs`

目标：

- 在 terrain material 清理后重新评估 LOD。
- 将网格密度和材质画质分开管理。

### `crates/hoi4-render/src/camera.rs`

目标：

- 如有需要，将近距限制和 quality preset 关联。
- 支持固定验收场景相机。

## 12. 风险表

| 风险 | 影响 | 缓解 |
|---|---|---|
| 没有 baseline 就改视觉 | 高 | Phase 0 必须先完成。 |
| 缺资源导致误判 shader | 高 | MapAssetAudit 和 InvalidForVisualReview。 |
| terrain 重写范围过大 | 高 | 先拆 debug output 和 material stage。 |
| terrain 和 border 双重画边界 | 高 | Gate 3 强制单 owner。 |
| terrain 和 water 在海岸 depth 冲突 | 中高 | water-only 和 coast 场景验收。 |
| postprocess 掩盖底图问题 | 中 | 必须输出 postprocess_off。 |
| label/counter declutter 影响玩法 | 中高 | 使用战争前线场景验收。 |
| 性能退化不可见 | 中 | 增加 pass timing 和预算。 |
| 旧路线图继续误导 | 中 | V2 骨架稳定后，把旧路线图标记为历史。 |

## 13. 时间估算

| 阶段 | 估算 |
|---|---:|
| Phase 0：基准截图和审计 | 3-5 天 |
| Phase 1：MapRenderer 所有权重构 | 5-8 天 |
| Phase 2：资源契约和 fallback | 4-7 天 |
| Phase 3：地形材质重写 | 8-14 天 |
| Phase 4：水体系统重写 | 5-9 天 |
| Phase 5：边界系统重写 | 6-10 天 |
| Phase 6：河流和静态 decal | 4-7 天 |
| Phase 7：语义 overlay | 5-8 天 |
| Phase 8：label/counter/object | 6-12 天 |
| Phase 9：色彩和后处理 | 4-8 天 |
| Phase 10：性能预算 | 4-7 天 |
| Phase 11：清理和文档 | 3-6 天 |

总计约 57 到 101 个工程日。

建议：

- 单人可以顺序完成。
- 两人可在 Phase 1 后拆分：一人做底图/材质，一人做 overlay/label/counter。
- Phase 0 和 Phase 1 稳定前，不建议并行重写 terrain、water、border。

## 14. 第一批具体任务

第一批应该小、稳、可回滚：

1. 新增 `MapAssetAudit` 数据模型和报告输出。
2. 新增固定相机场景定义。
3. 规定截图命名：
   - `scene_name.layer_name.preset.png`
4. 新增 layer mask settings。
5. 增加 postprocess on/off 截图。
6. 生成第一版 baseline。
7. 然后才开始把 pass 编排迁到 `MapRenderer`。

第一批完成定义：

- 没有视觉重写。
- 能证明加载了哪些资源。
- 能重复输出截图。
- 能分别看 terrain、water、borders、overlays、labels。

## 15. 最终建议

本项目要达到 HOI4 那种级别的地图效果，不能继续把它当作 shader 微调问题。应该把它当作一个完整地图渲染产品来做。

正确路线是：

1. 先让质量可观测。
2. 再让 pass 所有权清晰。
3. 移除重复渲染同一语义的效果。
4. 分别校准 terrain、water、border、overlay、label、postprocess。
5. 用截图回归和资源审计锁住改进。

在这些基础建立之前，继续单独调 `terrain.wgsl`、`water.rs` 或 `postprocess.rs`，只会得到局部变好、整体仍然不稳定的结果。
