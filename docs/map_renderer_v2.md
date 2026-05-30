# Map Renderer V2 维护说明

本文是 Phase 11 后的地图渲染入口文档。实现仍分布在 `crates/hoi4-app/src/main.rs`、`crates/hoi4-app/src/map_renderer.rs` 和 `crates/hoi4-app/src/passes/*`，但读代码前应先用本文定位职责边界。

## 单一来源

- Pass 顺序和 registry 名称：`MapRenderPass::PHASE1_ORDER` 和 `MapRenderPass::registry_name()`。
- 每帧是否绘制：`MapRenderer::build_frame_plan()`，输入是 `MapFrameContext`、`MapRenderSettings`、`MapLayerMask`、quality preset 和 `PassRegistry` toggle。
- 截图图层：`MapBaselineLayer::ALL` 和 `MapLayerMask::for_layer()`。
- 运行时 debug：F4 显示 pass/perf overlay；F6 循环 terrain debug；F10 循环 water debug；V 循环 border debug；Shift+F5 循环 postprocess debug；Shift+F8 切换 quality preset；M 切换 map mode。
- 性能预算：`MapQualityPreset`、`MapPerformanceBudget`、`GpuTimestampProfiler` 和 `phase10_overlay_lines()`。

## Phase 11 旧路径策略

旧 app inline terrain pipeline 和 `TerrainRenderPath::LegacyCompat` 已下线。当前 terrain 必须经过 `TerrainPass`；`hoi4-render::ARCHIVED_SHADER_MAIN_WGSL` 只保留为历史参考，不再有 `SHADER_MAIN_WGSL` 兼容别名。

允许存在的 fallback 必须是显式、有限、可审计的 pass 内 fallback：

- 必需启动资源缺失时阻断或标记不可视觉验收，例如 provinces、heightmap、terrain index。
- 可选材质缺失时用 1x1 texture 或空 instance buffer。
- object/label 类资源缺失时可用程序化占位或空绘制，但必须在审计或日志中可见。
- terrain 不再作为常规 water/border owner。只有 dedicated `WaterPass` 或 `BorderPass` 不可用时，`MapPassDrawSet::terrain_material_ownership()` 才允许打开有限 fallback 位。

## Pass 合约

| Pass | 主要输入 | 输出 | Depth / Blend | Fallback | Debug 和限制 |
|---|---|---|---|---|---|
| `shadow_caster` | camera、heightmap、LOD chunk uniforms、visible chunks | `SHADOW_DEPTH_FORMAT` shadow map | depth-only，write=true，无 color | pass 关闭时下游收到现有 shadow view 但不更新阴影 | F3 可看 shadow overlay；目前 caster 主要覆盖 terrain |
| `3d_sky` | camera、time、sky texture 或程序化参数 | HDR target 背景 | depth test，write=false，blend=replace | sky 资源缺失时使用默认/程序化颜色 | 无专用 shader debug；作为最早 color pass，避免清屏色泄漏 |
| `3d_terrain` | global uniforms、heightmap、province id、terrain index、terrain atlas、atlas normal、world normal、colormap、citylights、SDF、occupation LUT、rivers、shadow map、country LUT | HDR land material 和主 depth | depth write=true，blend=none | 可选贴图走 1x1 fallback；无旧 shader fallback | F6: `terrain_id`、`atlas_tile_id`、`political_color`、`terrain_albedo`、`normal`、`height_slope`、`snow_mask`、`river_mask`；season/light data 仍有近似 |
| `3d_water` | heightmap、coast SDF、water/lean/reflection/ice/FoW textures、water params、visible chunks | HDR final water color | depth test，write=false，blend=none，fragment 内 discard 非水域 | 贴图缺失走 1x1；pass 不可用时 terrain fallback 位才可接管最终水色 | F10: `depth_ratio`、`coast_distance`、`normal_strength`、`foam_mask`、`ice_mask`、`reflection_contribution`、`final_water_only`；折射和完整 FoW 仍未 1:1 |
| `3d_border` | extracted border meshes、border LOD textures、selection/hover params、camera | HDR alpha border lines | depth LessEqual，write=false，alpha blending | 缺失 texture 用 1x1；mesh 为空则无 draw | V: `country_only`、`state_only`、`province_only`、`sea_only`、`selected_only`、`false_color_hierarchy`；terrain SDF border 仅为 fallback |
| `3d_traderoute` | trade route instances、camera、time、overlay plan | HDR alpha route overlay | depth LessEqual，write=false，alpha blending | 程序化 dashed line，无 texture 依赖；无 route 时空绘制 | F4 统计可见；数据来源和 vanilla route 细节仍有限 |
| `3d_strait` | strait geometry/instances、camera、time、overlay plan | HDR alpha strait overlay | depth LessEqual，write=false，alpha blending | 缺失数据时空绘制 | F4 统计可见；当前偏静态 decal |
| `3d_railways` | railway line buffers、camera、static decal plan | HDR alpha railway lines | depth test，write=false，alpha blending | 无铁路数据时空绘制 | F4 统计可见；只在 static decal mask 允许时绘制 |
| `hoi3_counter_v3` | visible counter instances、counter atlas、country colors、selection state | HDR alpha unit counters | depth test，write=false，alpha blending | atlas/instance 缺失时空绘制或占位 atlas | F8 toggle counters，F4 统计；布局仍由当前 counter 系统约束 |
| `3d_trees` | tree mask、season/tint textures、tree mesh or billboard assets、camera、quality density | HDR tree geometry | depth write=true，alpha blending | texture/mesh 缺失时可用程序化或空实例 fallback | F4 统计，quality preset 控密度；远景/季节混合仍可继续校准 |
| `3d_buildings` | city/building instances、pdxmesh resources、flat diffuse fallback、camera | HDR building geometry | depth write=true，alpha blending | 缺失 mesh/material 时走 flat diffuse 或空实例 | F4 统计；建筑种类和 vanilla placement 仍不完整 |
| `3d_poi_icons` | POI instances、sprite atlas 或程序化 icon、camera、quality density | HDR alpha icons | depth test，write=false，premult alpha blending | atlas 缺失时生成占位或空绘制 | F4 统计；icon set 仍可扩充 |
| `3d_mapname` | generated glyph atlas、country name instances、camera、zoom | HDR alpha country labels | depth test，write=false，alpha blending | 字体/glyph 缺失时跳过对应 label | labels baseline 可验收；弯曲路径和 vanilla 字距仍有限 |
| `3d_province_name` | generated glyph atlas、province label instances、camera、zoom | HDR alpha province labels | depth test，write=false，alpha blending | 字体/glyph 缺失时跳过对应 label | labels baseline 可验收；遮挡和密度规则仍需继续调优 |
| `3d_maparrow` | battleplan/frontline arrow instances、camera、time、overlay plan | HDR alpha arrows | depth LessEqual，write=false，alpha blending | fragment 程序化 pattern，无 texture 依赖 | F4 统计；当前是稳定的 battleplan/frontline overlay 路径 |
| `3d_frontlines` | old frontline line mesh、camera、overlay plan | HDR alpha lines | depth test，write=false，alpha blending | registry 默认关闭；map arrows 是默认路径 | 保留为可比较路径，不应新增职责 |
| `3d_particles` | particle instances、particle texture、camera、quality budget | HDR premult alpha particles | depth test，write=false，premult alpha blending | 1x1 texture 或空实例 | F4/quality 统计；天气和战斗粒子仍是近似 |
| `postprocess` | HDR target、bloom targets、luminance targets、LUT、calibration、debug view | swapchain LDR final frame | no depth，blend=none | `PostProcessMode::Off` 走 simple blit | Shift+F5: `final`、`hdr_raw`、`tonemap_only`、`bloom_only`；校准值集中在 `PostProcessCalibration` |
| `ui` | egui/text draw data、F4 overlay lines、swapchain view | swapchain UI overlay | no depth，UI 自己处理 blend | 无 UI 数据则无 draw | F4 overlay 从 `PassRegistry` 和 perf budget 生成 |

## 验收入口

- 快速编译：`cargo check -p hoi4-app`。
- Pass graph 单测：`cargo test -p hoi4-app --bin hoi4-app phase1_graph_owns_pass_order -- --nocapture`。
- 旧 terrain fallback 回归：`cargo test -p hoi4-app --bin hoi4-app disabling_3d_map_suppresses_render_passes_without_legacy_fallback -- --nocapture`。
- 截图 baseline：`cargo run -p hoi4-app -- --map-phase0 --map-phase0-output target/map_baseline_phase11`。
- 只生成报告和资源审计：`cargo run -p hoi4-app -- --map-phase0-report-only --map-phase0-output target/map_baseline_phase11`。
