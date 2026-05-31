# HOI4 原版地图视觉完美移植重构路线图

> 目标：在合法读取用户本机 Steam 版 Hearts of Iron IV 资源的前提下，把当前项目的地图渲染从“使用部分原版素材的近似实现”重构为“以原版渲染语义为基准的兼容管线”，最终达到同地点、同相机、同时间、同地图模式下与 HOI4 原版高度一致的视觉效果。

> 重要边界：本路线图不要求也不允许把 HOI4 原版贴图、shader、mesh、GUI 资源提交到仓库或随项目发布。项目只能在运行时通过 `--game-path` 或用户配置读取本机合法安装目录。可以移植渲染行为、shader 公式和管线语义，但不能分发 Paradox 原始资源。

---

## 1. 当前结论

### 1.1 资源不是主要问题

本地审计结果已经确认：

- `target/map_audit/latest.json`：`total=71`
- `loaded=71`
- `missing=0`
- `fallback=0`
- `quality=Valid`
- `visual_review_usable=true`

这说明 Steam HOI4 地图资源已经能被项目找到，截图差距的主因不是“缺贴图”，而是：

- 原版 shader 语义没有完整移植。
- 原版运行时生成的中间贴图没有生成或被 1x1 mock 替代。
- 地形、水体、河流、边界、树木、点光源、FOW、后处理被项目自定义逻辑近似替代。
- 当前截图对比存在相机距离、地图模式、图层开启状态不同的问题。

### 1.2 当前项目实际状态

关键代码状态：

- `crates/hoi4-app/src/main.rs`
  - `WORLD_SCALE = 0.02`，项目世界坐标约为 `112 x 41` 单位。
  - 默认 `map_mode = MapMode::Political`。
  - `Political => map_mode_terrain_blend = 0.20`，政治颜色强压地形。
  - `TerrainPassInputs.map_set = None`，不是所有地形资源都从完整 `VanillaMapSet` 输入。
  - `RiverPass` 已构造但渲染阶段被显式禁用，河流改为 terrain shader 内蓝色 overlay。

- `crates/hoi4-app/src/passes/terrain.rs`
  - 已加载部分原版贴图：`atlas_normal0`、`world_normal.bmp`、`colormap_rgb_cityemissivemask_a`、`citylights_0` 等。
  - `light_data`、`light_index`、`province_secondary_color` 目前是 1x1 mock。
  - `GradientBorderChannel1/2` 目前由项目 SDF 贴图或边界 pass 替代，不是原版动态通道。

- `crates/hoi4-app/src/passes/terrain.wgsl`
  - 有自定义 `terrain_material_weights`。
  - 有自定义 procedural noise、jitter、grain、coast tint。
  - 河流在地形中混蓝色。
  - 雪、泥、城市灯光、政治色叠加与原版公式不完全一致。

- `crates/hoi4-app/src/passes/water.rs`
  - Phase 6 已接入 `pdxwater.shader` 语义：SampleWater、LEAN normal、reflection/refraction、fow_water_spec、ice、gradient border、secondary color、FOW/distance fog。
  - `light_data`、`light_index` 已走显式 blocker binding；真实点光源内容生成仍归 Phase 5。
  - 环境 cubemap 有 dim-blue fallback。

- `crates/hoi4-app/src/passes/border.rs`
  - 使用 strip mesh border。
  - 与原版 shader 依赖的 `GradientBorderChannel1/2/3` 不是同一套运行时输入。

- `crates/hoi4-app/src/passes/postprocess.rs`
  - 当前为 ACES、自定义自动曝光、bloom、vignette。
  - 原版 `restorescene.shader` 是 bloom、exposure、Uncharted tonemap、ColorCube/LUT 的组合。

### 1.3 原版 HOI4 实际依赖

原版效果不是单个 shader，而是一整套渲染系统：

- `gfx/FX/pdxmap.shader`
- `gfx/FX/pdxwater.shader`
- `gfx/FX/river.shader`
- `gfx/FX/tree.shader`
- `gfx/FX/border.shader`
- `gfx/FX/pdxmesh.shader`
- `gfx/FX/restorescene.shader`
- `gfx/FX/standardfuncsgfx.fxh`
- `gfx/FX/constants.fxh`
- `gfx/FX/fow.fxh`
- `gfx/FX/tiled_pointlights.fxh`
- `gfx/FX/shadow.fxh`

这些 shader 共同依赖：

- 原始地图贴图：`provinces.bmp`、`heightmap.bmp`、`terrain.bmp`、`rivers.bmp`、`trees.bmp`、`cities.bmp`、`world_normal.bmp`。
- terrain 资源：atlas、atlas_normal、mud、snow、ice、water、reflection、citylights、tree season/tint、border、fow 等。
- 运行时贴图：`GradientBorderChannel*`、`ProvinceSecondaryColorMap`、`LightDataMap`、`LightIndexMap`、FOW、MudSnow、ShadowMap。
- 原版坐标常量：`MAP_SIZE_X=5632`、`MAP_SIZE_Y=2048`。
- 相机、时间、季节、昼夜、fog、shadow、point light、map mode 的统一 uniform。

---

## 2. 重构总原则

### 2.1 先 parity，后美术调参

在达到原版对齐前，禁止继续添加新的“看起来更好”的自定义调色、noise、bloom、vignette、foam、border 宽度调参。否则无法判断差距来自哪里。

要求：

- parity 阶段保留一个 `vanilla_parity` 配置。
- 在该配置下关闭所有非原版艺术增强。
- 所有自定义风格化必须放到 parity 之后，且可开关。

### 2.2 以原版坐标为 shader 事实来源

项目可以继续使用 `WORLD_SCALE=0.02` 作为渲染世界单位，但 shader 中必须同时提供：

- `world_pos`：项目世界坐标。
- `map_px_pos`：原版地图像素坐标，范围接近 `0..5632` / `0..2048`。
- `map_uv`：标准 `0..1` UV。

原版逻辑中的：

- terrain tile repeat
- city lights tiling
- FOW 采样
- day/night globe normal
- point light wrapping
- tree mask
- mud/snow
- gradient border

都应优先使用原版地图像素坐标，而不是项目缩放后的 world unit。

### 2.3 所有 fallback 必须可视化

当前最危险的问题是“资源可用，但 shader 实际用了 mock”。需要新增调试视图：

- 当前 pass 是否使用 mock。
- 每个 binding 是否来自原版资源、动态 render target、项目 fallback。
- fallback 是否影响视觉验收。

运行时 HUD 或 debug 输出必须能列出：

- `TerrainDiffuse`
- `TerrainNormal`
- `MudDiffuseGloss`
- `MudNormalSpec`
- `SnowTexture`
- `CityLightsAndSnowNoise`
- `GradientBorderChannel1/2/3`
- `ProvinceSecondaryColorMap`
- `LightDataMap`
- `LightIndexMap`
- `FOW`
- `ShadowMap`

### 2.4 每个阶段必须有截图回归

不能只靠肉眼运行一次判断。每个阶段必须生成固定基线截图：

- 地形 only
- 水体 only
- 河流 only
- 边界 only
- 树木/建筑 only
- 后处理前 HDR
- 最终 LDR

至少固定 5 个场景：

1. 西欧近景：比利时、荷兰、鲁尔、法国北部。
2. 中欧中景：德国、波兰、捷克。
3. 地中海：意大利、亚得里亚海、巴尔干。
4. 北欧/苏联冬季：雪线、森林、低太阳角。
5. 太平洋/海岸：深海、浅海、岛屿、海峡。

---

## 3. 目标架构

### 3.1 模块分层

重构后地图渲染建议分为 8 层：

1. `hoi4-assets`
   - 只负责从 Steam 路径读取原版资源。
   - 维护白名单、格式解析、DDS/BMP 上传计划、审计报告。

2. `hoi4-map`
   - 解析地图基础数据。
   - 生成 province id、state id、terrain id、height、river/tree/city 分布。

3. `hoi4-render::vanilla`
   - 新增建议模块。
   - 放原版常量、坐标转换、shader shared helpers、render target 定义。

4. `hoi4-app::vanilla_targets`
   - 新增建议模块。
   - 生成运行时贴图：GradientBorder、ProvinceSecondaryColor、LightData、LightIndex、FOW、MudSnow。

5. `hoi4-app::passes`
   - terrain、水体、河流、树木、border、pdxmesh、postprocess。
   - 每个 pass 明确声明输入 ownership。

6. `map_renderer.rs`
   - 只负责 pass plan、zoom gates、feature flags、debug ownership。

7. `map_baseline.rs`
   - 截图基线和回归测试。

8. `tools/` 或 `xtask`
   - 批量截图、图片 diff、资源审计、shader binding 审计。

### 3.2 新增核心类型

建议新增：

```rust
pub struct VanillaMapSpace {
    pub map_size_px: [f32; 2],      // [5632, 2048]
    pub world_size: [f32; 2],       // 项目世界尺寸
    pub world_to_map_px: [f32; 2],
    pub map_px_to_world: [f32; 2],
}

pub struct VanillaFrameUniform {
    pub map_size_px: [f32; 2],
    pub world_size: [f32; 2],
    pub cam_pos_world: [f32; 4],
    pub cam_pos_map_px: [f32; 4],
    pub day_night_hour_sun_dir: [f32; 4],
    pub snow_mud_fow_data: [f32; 4],
    pub gb_cam_dist_outline_cutoff: [f32; 4],
    pub fow_opacity_time_snow_max_speed: [f32; 4],
}

pub struct VanillaRuntimeTargets {
    pub gradient_border_ch1: TextureView,
    pub gradient_border_ch2: TextureView,
    pub gradient_border_ch3: TextureView,
    pub province_secondary_color: TextureView,
    pub light_data: TextureView,
    pub light_index: TextureView,
    pub fow: TextureView,
    pub mud_snow: TextureView,
}
```

### 3.3 pass ownership

目标 ownership：

| 内容 | 最终 owner | 当前问题 |
|---|---|---|
| 陆地地形 | `TerrainPass` 原版 pdxmap path | 自定义混色/noise 太多 |
| 水体 | `WaterPass` 原版 pdxwater path | 近似 Fresnel/foam/reflection |
| 河流 | `RiverPass` | 当前禁用，地形蓝色 overlay 代替 |
| 国界/省界渐变 | `VanillaRuntimeTargets` + shader 内应用 | 当前 strip mesh 与 shader 输入割裂 |
| 树木 | `TreeFullPass` 原版 tree path | 缺 gradient/secondary/point light/FOW 完整语义 |
| 城市/建筑/单位模型 | `PdxMeshPass` | 需要 `.gfx/.mesh` 与 pdxmesh shader 语义 |
| 后处理 | `PostProcess` 原版 restorescene path | 当前 ACES 链路不一致 |

---

## 4. 阶段路线

## Phase 0：建立可验证 parity 环境

目标：先解决“无法客观比较”的问题。

周期：1-2 天。

### 任务

- [x] 新增固定截图场景配置文件。
- [x] 每个场景保存：
  - map center
  - zoom/distance
  - pitch/yaw
  - date/hour
  - map mode
  - enabled layers
- [x] 扩展 `--map-phase0` 或新增 `--map-parity-capture`。
- [x] 输出目录：
  - `target/map_parity/YYYYMMDD_HHMMSS/terrain.png`
  - `target/map_parity/YYYYMMDD_HHMMSS/water.png`
  - `target/map_parity/YYYYMMDD_HHMMSS/borders.png`
  - `target/map_parity/YYYYMMDD_HHMMSS/objects.png`
  - `target/map_parity/YYYYMMDD_HHMMSS/final.png`
  - `target/map_parity/YYYYMMDD_HHMMSS/report.json`
- [x] 把当前项目截图和原版截图分开保存，不覆盖。
- [x] 新增文档说明如何在 HOI4 原版中复现相同位置。

### 修改文件

- `crates/hoi4-app/src/map_baseline.rs`
- `crates/hoi4-app/src/main.rs`
- `docs/map_renderer_v2.md`
- 新增 `docs/map_parity_capture_zh.md`

### 验收标准

- [x] 能一键生成同一批固定场景截图。
- [x] 截图报告写明 pass 状态、fallback 状态、相机参数。
- [x] 不再用两张不同 zoom/mode 的截图直接做结论。

---

## Phase 1：资源与 binding 审计重构

目标：让“原版资源已经加载”真正变成“shader 实际使用了正确资源”。

周期：3-5 天。

### 任务

- [x] `TerrainPassInputs.map_set` 改为传入完整 `VanillaMapSet` 或统一的 `VanillaResourceViews`。
- [x] 建立 `VanillaResourceViews`：
  - terrain atlas 0/1/2
  - terrain atlas normal 0/1/2
  - mud diffuse/normal 0/1
  - snow normal diffuse
  - citylights 0/1/2
  - colormap variants
  - water resources
  - river resources
  - tree season/tint
  - border textures
- [x] 每个 pass 增加 `BindingAudit`。
- [x] 任何 mock texture 必须进入报告：
  - mock name
  - reason
  - visual impact
  - blocking level
- [x] CI 或本地测试增加：
  - `map_asset_audit_valid`
  - `terrain_binding_no_critical_mock_in_parity`
  - `water_binding_no_critical_mock_in_parity`

### 修改文件

- `crates/hoi4-assets/src/vanilla_map_set.rs`
- `crates/hoi4-assets/src/map_asset_audit.rs`
- `crates/hoi4-app/src/passes/terrain.rs`
- `crates/hoi4-app/src/passes/water.rs`
- `crates/hoi4-app/src/passes/river.rs`
- 新增 `crates/hoi4-app/src/vanilla_resource_views.rs`

### 验收标准

- parity 模式下，critical binding 不允许静默 mock。
- 启动日志明确列出每个原版资源被哪个 pass 使用。
- `target/map_audit/latest.json` 和 `target/map_parity/*/report.json` 能互相对应。

---

## Phase 2：原版坐标系统重构

目标：解决项目 world scale 与原版 shader 常量不一致导致的 tiling、fog、day/night、citylight、tree mask 偏差。

周期：3-5 天。

### 任务

- [x] 新增 `VanillaMapSpace`。
- [x] 所有地图 pass 的 vertex output 增加：
  - `world_pos`
  - `map_uv`
  - `map_px`
- [x] shader 中原版公式统一使用 `map_px`。
- [x] 保留 `world_pos` 给项目相机、深度、mesh 变换使用。
- [x] 检查以下函数输入：
  - `calc_globe_normal`
  - `day_night_factor`
  - `citylights`
  - `tree mask`
  - `fow`
  - `mud/snow`
  - `point light wrapping`
  - `terrain tile repeat`
- [x] 添加 debug view：
  - map uv
  - map px grid
  - vanilla tile repeat
  - citylight uv

### 修改文件

- `crates/hoi4-render/src/defines.rs`
- `crates/hoi4-render/src/global_uniform.rs`
- `crates/hoi4-render/src/shader_lib.wgsl`
- `crates/hoi4-render/src/translations/*.wgsl`
- `crates/hoi4-app/src/main.rs`
- `crates/hoi4-app/src/passes/terrain.wgsl`
- `crates/hoi4-app/src/passes/water.rs`
- `crates/hoi4-app/src/passes/trees_full.wgsl`
- `crates/hoi4-app/src/passes/border.rs`
- `crates/hoi4-app/src/passes/mapname.rs`
- `crates/hoi4-app/src/passes/province_name.rs`
- `crates/hoi4-app/src/passes/pdxmesh.rs`
- `crates/hoi4-app/src/passes/river.rs`
- `crates/hoi4-app/src/passes/sky.rs`

### 验收标准

- atlas tiling 与原版比例一致。
- citylights 不再因为 world scale 偏差而过密或过稀。
- 树木 mask 与 `trees.bmp` 空间位置一致。
- day/night 分界线与原版方向一致。

---

## Phase 3：TerrainPass 严格重写为 pdxmap parity

目标：地形是画面主体，必须先把地形从“近似”改为“原版语义优先”。

周期：2-3 周。

完成记录（2026-05-31）：`TerrainPass` 已切换为 parity-first 路径。默认 feature flags 关闭项目自定义美术；terrain atlas 使用 256 项 LUT；snow/mud/citylights/world_normal 接入真实资源视图；debug view 扩展到 terrain blend、四角 terrain id、mud、city emit、citylights、night factor 与 citylight contribution。PointLights 的真实内容生成仍归 Phase 5，Phase 3 只保留 TerrainPass binding/audit。

### 3.1 禁用自定义地形美术

- [x] parity 模式下禁用：
  - procedural terrain jitter
  - 自定义 triplanar noise
  - 自定义 grain
  - 自定义 coast tint
  - terrain 内河流蓝色 overlay
  - 自定义政治色混合公式
  - 自定义 vignette
- [x] 所有禁用项保留 feature flag，不能直接删除。

### 3.2 移植 terrain id 到 atlas index 的完整路径

- [x] 校验 `common/terrain/00_terrain.txt`。
- [x] 确认 `terrain.bmp` 的 index 与 terrain catalog 的映射。
- [x] 支持原版 `atlas_idx` 或等价 LUT。
- [x] 增加 debug view：
  - raw terrain id
  - atlas index
  - all-same / blended state
  - four-corner terrain index

### 3.3 移植原版 terrain atlas 采样

- [x] 实现原版 `calculate_map_tex_index` 等价逻辑。
- [x] 实现原版 `sample_terrain` 等价逻辑。
- [x] 支持四邻域 terrain diffuse 混合。
- [x] 支持四邻域 terrain normal 混合。
- [x] 正确处理 atlas mip/lod。
- [x] 正确处理 atlas normal alpha 中的 spec/gloss 语义。

### 3.4 移植地形颜色叠加

- [x] `GetOverlay` 使用与原版一致的 gamma/linear 流程。
- [x] `COLORMAP_OVERLAY_STRENGTH` 使用原版常量。
- [x] `TerrainColorTint` 使用原版 colormap 路径。
- [x] 政治地图模式的颜色叠加独立为 map mode overlay，不污染 terrain parity path。

### 3.5 移植雪、泥、季节

- [x] 接入 `snow_normal_rgb_diffuse_a.dds`。
- [x] 接入 `mud_diffuse_rgb_gloss_a_{0,1}.dds`。
- [x] 接入 `mud_normal_rgb_spec_a_{0,1}.dds`。
- [x] 实现 `GetMudSnowColor` 等价数据来源。
- [x] 实现 `ApplySnow`。
- [x] 实现 `GetMudColor`。
- [x] 季节参数由日期驱动，而不是固定或近似。

### 3.6 移植城市夜光

- [x] 接入全部 citylights LOD 或至少与原版当前 zoom 对应的 LOD。
- [x] 使用原版 city light tiling。
- [x] 使用 colormap alpha/emissive mask。
- [x] 夜晚强度使用原版 day/night 因子。
- [x] debug view 显示：
  - city emit mask
  - citylights rgb
  - night factor
  - final citylight contribution

### 3.7 地形光照

- [x] Terrain normal = height/world normal + atlas normal 的原版组合。
- [x] SunLight 使用原版方向和强度。
- [x] ShadowMap 输入保持一致。
- [x] PointLights 等待 Phase 5，但 TerrainPass binding 先接入动态 target 槽位并进入 BindingAudit。

### 修改文件

- `crates/hoi4-app/src/passes/terrain.rs`
- `crates/hoi4-app/src/passes/terrain.wgsl`
- `crates/hoi4-render/src/shader_lib.wgsl`
- `crates/hoi4-map/src/terrain_catalog.rs`
- `crates/hoi4-render/src/defines.rs`

### 验收标准

- [x] terrain-only 截图在西欧近景下接近原版。
- [x] 地形纹理不再像低频政治色块。
- [x] 山地、森林、平原、城市区域的纹理分类与原版一致。
- [x] 雪/泥/城市夜光 debug view 有正确输出。

---

## Phase 4：动态 overlay render targets ✅ 已完成

目标：补齐原版 shader 依赖但 Steam 目录中不存在的运行时贴图。

周期：1-2 周。

### 4.1 GradientBorderChannel

- [x] 生成 `GradientBorderChannel1`。
- [x] 生成 `GradientBorderChannel2`。
- [x] 生成 `GradientBorderChannel3`。
- [x] 明确各通道语义：
  - country border
  - province border
  - state/sea/impassable border
  - alpha/gradient/stripe 信息
- [x] terrain、水体、树木、pdxmesh 都使用同一组 gradient channel。
- [x] strip mesh border 只保留为 debug 或 fallback。

### 4.2 ProvinceSecondaryColorMap

- [x] 生成真实 `ProvinceSecondaryColorMap`。
- [x] 支持：
  - occupation
  - battle plan
  - selection
  - hovered province
  - map mode secondary tint
  - naval dominance
- [x] 替换 terrain 中现有零值 mock。
- [x] 替换 water/tree 中缺失的 secondary mask 输入。

### 4.3 FOW 与 MudSnow

- [x] 新增 FOW target。
- [x] 新增 MudSnow target。
- [x] 接入原版 `fow.fxh` 语义。
- [x] 支持 debug：
  - unexplored
  - visible
  - enemy spotted
  - snow amount
  - mud amount

### 修改文件

- 新增 `crates/hoi4-app/src/vanilla_targets/mod.rs`
- 新增 `crates/hoi4-app/src/vanilla_targets/gradient_border.rs`
- 新增 `crates/hoi4-app/src/vanilla_targets/province_secondary.rs`
- 新增 `crates/hoi4-app/src/vanilla_targets/fow.rs`
- `crates/hoi4-app/src/passes/terrain.rs`
- `crates/hoi4-app/src/passes/water.rs`
- `crates/hoi4-app/src/passes/trees_full.rs`
- `crates/hoi4-app/src/passes/pdxmesh.rs`
- `crates/hoi4-app/src/passes/border.rs`

### 验收标准

- [x] terrain/water/tree 不再使用 `province_secondary_color_mock`。
- [x] terrain/water/tree/pdxmesh 共享同一套 gradient border 输入。
- [x] 国界视觉从“外部画线”转为“shader 材质内渐变影响”。

完成记录：

- `VanillaRuntimeTargets` 统一生成 `GradientBorderChannel1/2/3`、`ProvinceSecondaryColorMap`、`FOW`、`MudSnow`，并为 Phase 4 target 声明格式/语义元数据与单测。
- `ProvinceSecondaryColorMap` 覆盖 occupation、battle plan、selection、hover、map mode secondary tint、naval dominance 近似输入。
- terrain/water/tree/pdxmesh 均绑定共享 runtime target；terrain 增加 FOW unexplored/visible/enemy spotted 与 MudSnow snow/mud debug view。
- `LightDataMap` / `LightIndexMap` 仍是 Phase 5 的 point light 系统 blocker，不归入 Phase 4。

---

## Phase 5：PointLights、FOW、Shadow 统一光照系统

目标：补齐原版夜景、城市、单位、建筑、树木的局部光照语义。

周期：1 周。

### 任务

- [ ] 实现 `LightDataMap`。
- [ ] 实现 `LightIndexMap`。
- [ ] 移植 `CalculatePointLights` 等价逻辑。
- [ ] 地图上生成点光源：
  - 城市
  - 港口
  - 机场
  - 战斗/爆炸特效
  - 单位/建筑可选光源
- [ ] 所有 pass 共享：
  - shadow map
  - FOW map
  - light data/index
  - day/night uniform

### 修改文件

- 新增 `crates/hoi4-app/src/vanilla_targets/point_lights.rs`
- `crates/hoi4-render/src/shader_lib.wgsl`
- `crates/hoi4-app/src/passes/terrain.wgsl`
- `crates/hoi4-app/src/passes/water.rs`
- `crates/hoi4-app/src/passes/trees_full.wgsl`
- `crates/hoi4-app/src/passes/pdxmesh.rs`

### 验收标准

- 夜晚城市区域有原版式局部亮度。
- 树木、水面、建筑能受到同一套点光源影响。
- 远近景点光源衰减稳定，没有闪烁。

---

## Phase 6：WaterPass 重写为 pdxwater parity ✅ 已完成

目标：把水体从项目风格化水面改为原版 `pdxwater.shader` 语义。

周期：1-2 周。

### 任务

- [x] 移植 `SampleWater`。
- [x] 移植 LEAN normal blending。
- [x] 使用 `lean1.dds`、`lean2.dds`。
- [x] 使用 `fow_rgb_waterspec_a.dds` 的 spec 语义。
- [x] 使用 `reflection.dds` 与 cubemap。
- [x] 实现 refraction path。
- [x] 实现 fresnel 与 reflection/refraction 混合。
- [x] 实现 `ApplyIce`。
- [x] 使用 `ice_diffuse.dds`、`ice_noise_0/1.dds`。
- [x] 接入 gradient border。
- [x] 接入 province secondary color。
- [x] 接入 point lights binding path；真实 `LightData/LightIndex` 内容生成仍是 Phase 5 blocker。
- [x] 接入 FOW 与 distance fog。
- [x] 移除 parity 模式下自定义 foam/深浅渐变调色。

### 修改文件

- `crates/hoi4-app/src/passes/water.rs`
- `crates/hoi4-render/src/shader_lib.wgsl`
- `crates/hoi4-assets/src/vanilla_map_set.rs`

### 验收标准

- 海岸、浅海、深海、北极冰层与原版近似。
- 水面反射不再是 dim-blue fallback 观感。
- 水面上的国界/占领/选择渐变与陆地一致。

完成记录（2026-05-31）：`WaterPass` 已扩展到 12 个水体 material bindings，接入 SampleWater、4-tap LEAN normal、reflection/refraction、ApplyIce、gradient border、province secondary color、FOW/distance fog，并移除程序化深浅渐变/噪声水面调色。`light_data` 与 `light_index` 绑定为显式 Phase 5 blocker，不伪装成真实点光源 target。`cargo test -p hoi4-app water_ -- --nocapture` 与 WGSL/Naga validate 已通过。

---

## Phase 7：RiverPass 恢复并按 river.shader 重写 ✅ 已完成

目标：河流必须作为独立 pass 渲染，不能混在地形颜色中。

周期：1 周。

### 任务

- [x] 修复当前 RiverPass z-fighting。
- [x] 使用稳定 depth bias 或 terrain-following offset。
- [x] 接入 `RiverSurface_diffuse_{0,1}.dds`。
- [x] 接入 `RiverSurface_normal_{0,1}.dds`。
- [x] 接入 `RiverSurface_masks.dds`。
- [x] 实现流动动画。
- [x] 实现河流宽度/透明度/深度变化。
- [x] terrain parity 模式禁用蓝色 river overlay。
- [x] 河流和 water pass 明确 draw order。

### 修改文件

- `crates/hoi4-app/src/passes/river.rs`
- `crates/hoi4-app/src/main.rs`
- `crates/hoi4-app/src/map_renderer.rs`
- `crates/hoi4-app/src/map_baseline.rs`
- `crates/hoi4-app/src/passes/terrain.wgsl`
- `crates/hoi4-map/src/rivers.rs`
- `crates/hoi4-app/tests/river_wgsl.rs`

### 验收标准

- [x] 河流不闪烁、不 z-fight。
- [x] 河流颜色、宽度、流动方向接近原版。
- [x] 河流不会被水体错误覆盖。

完成记录：

- `rivers.bmp` 上传改为 `Rgba8Unorm`：R 存粗河流等级，G/B 存稳定局部流向，A 保留原 palette index。
- `RiverPass` 恢复独立渲染，按 Terrain -> Water -> River -> Borders 顺序进入 frame plan，并记录 `3d_river` profiler/pass stats。
- `RiverPass` 使用 3 级 LOD 参数 buffer，绘制全部地形 LOD；增加 terrain-following y offset、clip-space z bias 与 depth bias 来避免 z-fighting。
- 河流材质接入 3 张 diffuse、3 张 normal 与 masks，基于流向滚动 UV，并按等级调整 alpha、深度 tint、specular 和宽度遮罩。
- terrain 内蓝色 river overlay 只在 dedicated `RiverPass` 未加载时作为 fallback；river-only capture 不再强制 TerrainDebugView。

---

## Phase 8：TreeFullPass 与植被 parity ✅ 已完成

目标：树木要从“有树模型”提升为“原版树木材质与遮罩语义”。

周期：1-2 周。

### 任务

- [x] 确认 `trees.bmp` 生成的实例分布与原版一致。
- [x] 使用 `TreeMaskTexture` 语义裁剪远景树。
- [x] 使用 `Tree_season.bmp`。
- [x] 使用 `Tree_tint.bmp`。
- [x] 支持 `ColorMap` 与 `ColorMapSecond`。
- [x] 实现原版 tree snow。
- [x] 接入 gradient border。
- [x] 接入 province secondary color。
- [x] 接入 point lights。
- [x] 接入 FOW 与 distance fog。
- [x] 检查 tree mesh LOD 与 alpha clip。

### 修改文件

- `crates/hoi4-app/src/passes/trees_full.rs`
- `crates/hoi4-app/src/passes/trees_full.wgsl`
- `crates/hoi4-render/src/trees.rs`
- `crates/hoi4-render/src/trees_mesh.rs`

### 验收标准

- 欧洲森林密度与原版接近。
- 季节变化正确。
- 远景树不会变成噪点或完全消失。
- 树木在边界、占领、夜晚、雪地条件下与地形一致。

完成记录（2026-05-31）：`TreeFullPass` 已接入 `trees.bmp` 分布统计、`TreeMaskTexture`、`Tree_season.bmp`、`Tree_tint.bmp`、terrain colormap、MudSnow snow mask、GradientBorderChannel1/2/3、ProvinceSecondaryColorMap、FOW/distance fog，并保留 mesh LOD 与 alpha clip 路径。`LightData/LightIndex` 已作为显式 Phase 5 blocker binding 接入，shader 具备 point-light lookup 路径，但真实点光源 target 内容生成仍归 Phase 5。`cargo test -p hoi4-render trees -- --nocapture`、`cargo test -p hoi4-app trees_full -- --nocapture`、`cargo test -p hoi4-app vanilla_resource_views -- --nocapture` 与 `cargo test -p hoi4-app vanilla_targets -- --nocapture` 已通过。

---

## Phase 9：PdxMesh、城市、建筑、单位与地图物件

目标：补齐原版截图中大量“细节密度”的来源。

周期：2-4 周。

### 任务

- [ ] 解析并加载 `.gfx` entity。
- [ ] 解析并加载 `.mesh` LOD。
- [ ] 建筑、港口、机场、雷达、防空、堡垒等地图物件走 `PdxMeshPass`。
- [ ] 接入 `pdxmesh.shader` 等价材质：
  - diffuse
  - normal
  - spec/gloss
  - snow
  - FOW
  - shadow
  - point lights
  - day/night
- [ ] 军队单位牌和 3D unit 分离：
  - counter 是 UI/overlay 层。
  - unit mesh 是 3D object 层。
- [ ] 对象层加入 zoom LOD，避免远景过载。

### 修改文件

- `crates/hoi4-app/src/passes/pdxmesh.rs`
- `crates/hoi4-render/src/mesh.rs`
- `crates/hoi4-assets/src/gfx.rs`
- `crates/hoi4-assets/src/mesh.rs`
- `crates/hoi4-app/src/map_renderer.rs`

### 验收标准

- 原版近景中的城市、港口、机场、建筑密度明显提升。
- 建筑与树木、地形的光照和 FOW 统一。
- 远景性能稳定。

---

## Phase 10：PostProcess 重写为 restorescene parity

目标：最后统一色彩、曝光、bloom 和 tonemap。

周期：1 周。

### 任务

- [ ] parity 模式禁用 ACES path。
- [ ] 移植原版 `RestoreScene` 语义。
- [ ] 支持 ColorCube/LUT。
- [ ] 使用原版 bloom 合成方式。
- [ ] 使用原版 exposure/average luminance。
- [ ] 使用 Uncharted tonemap。
- [ ] saturation、HSV、ColorBalance 参数接入统一配置。
- [ ] 输出 debug：
  - HDR scene
  - bloom
  - avg luminance
  - tonemap before/after
  - LUT before/after

### 修改文件

- `crates/hoi4-app/src/passes/postprocess.rs`
- `crates/hoi4-render/src/translations/`
- `crates/hoi4-render/src/shader_lib.wgsl`

### 验收标准

- 同一 terrain-only 输入下，最终 LDR 颜色与原版接近。
- 不再出现项目自定义 ACES 导致的过曝、过灰、过饱和或暗部错误。

---

## Phase 11：地图文字、图标、路线与单位牌对齐

目标：补齐原版截图中明显的 UI/overlay 视觉结构。

周期：2-3 周。

### 任务

- [ ] 国名 `mapname.shader` parity：
  - 沿地形曲面。
  - zoom fade。
  - stencil/depth 规则。
  - 字体 atlas 与描边。
- [ ] 省名 pass：
  - zoom-gated。
  - 最小可读尺寸。
  - 不穿帮、不重叠。
- [ ] POI 图标：
  - 工厂
  - 港口
  - 机场
  - 要塞
  - 资源
- [ ] 箭头和路线：
  - `maparrow`
  - `traderoute`
  - `strait`
- [ ] 单位牌：
  - 与原版截图的 scale、位置、层级、遮挡一致。

### 修改文件

- `crates/hoi4-app/src/passes/mapname.rs`
- `crates/hoi4-app/src/passes/province_name.rs`
- `crates/hoi4-app/src/passes/poi.rs`
- `crates/hoi4-app/src/passes/maparrow.rs`
- `crates/hoi4-app/src/passes/traderoute.rs`
- `crates/hoi4-render/src/counter_v3.rs`
- `crates/hoi4-app/src/map_renderer.rs`

### 验收标准

- 原版截图中的信息密度被补齐。
- 地图文字不遮挡主要视觉。
- 远中近三个 zoom 档位都有稳定布局。

---

## Phase 12：性能、缓存与最终回归

目标：在视觉接近原版后，确保性能和可维护性可接受。

周期：1-2 周。

### 任务

- [ ] GPU profiler 覆盖所有 pass。
- [ ] 每个 pass 记录：
  - CPU prepare ms
  - GPU draw ms
  - draw calls
  - texture memory
  - fallback count
- [ ] 资源缓存：
  - DDS upload cache
  - mesh parse cache
  - generated runtime target cache
- [ ] 降级策略：
  - low-end mode
  - no point lights
  - reduced tree density
  - reduced object LOD
  - lower-res gradient/FOW targets
- [ ] 图片 diff 工具：
  - SSIM
  - average color delta
  - luma delta
  - edge delta
- [ ] 最终建立 `PARITY_STATUS.md`。

### 验收标准

- 1080p 下默认场景帧率稳定。
- 关键场景截图回归不再随机变化。
- 所有 critical fallback 为 0。
- 文档清楚说明哪些地方是 1:1，哪些地方是有意近似。

---

## 5. 推荐执行顺序

不要按“看起来最酷”的 pass 做，按依赖顺序做：

1. Phase 0：截图基线。
2. Phase 1：binding 审计。
3. Phase 2：原版坐标系统。
4. Phase 3：TerrainPass parity。
5. Phase 4：动态 overlay targets。
6. Phase 7：RiverPass 恢复。
7. Phase 6：WaterPass parity。
8. Phase 8：TreeFullPass parity。
9. Phase 5：PointLights/FOW/Shadow 统一。
10. Phase 10：PostProcess parity。
11. Phase 9：PdxMesh/建筑/单位物件。
12. Phase 11：文字、图标、路线、单位牌。
13. Phase 12：性能与最终回归。

说明：

- terrain 必须早做，因为它决定画面主体。
- dynamic targets 必须在 water/tree/pdxmesh 完整 parity 前做，因为这些 pass 都依赖它们。
- postprocess 必须等 terrain/water 大体正确后做，否则会用后处理掩盖材质问题。
- 单位、建筑、文字属于视觉密度补齐，不能替代底层地形 parity。

---

## 6. P0 立即行动清单

### P0.1 固定同图对比

- [ ] 在项目内新增西欧固定相机 preset。
- [ ] 使用同一 map mode、同一日期、同一 zoom 截图。
- [ ] 保存原版截图和项目截图。
- [ ] 不再用远景政治图对比近景原版图。

### P0.2 关闭 parity 模式下的自定义增强

- [ ] terrain noise off。
- [ ] terrain blue river overlay off。
- [ ] ACES off。
- [ ] custom vignette off。
- [ ] custom water foam off。
- [ ] border strip debug only。

### P0.3 去掉 critical mock

- [ ] `province_secondary_color_mock` 替换为真实 render target 或明确 blocker。
- [ ] `light_data_mock` 替换为真实 target 或明确 blocker。
- [ ] `light_index_mock` 替换为真实 target 或明确 blocker。
- [ ] `GradientBorderChannel1/2` 不再伪装成 SDF 输入。

### P0.4 恢复 RiverPass

- [ ] 先解决 z-fighting。
- [ ] 再移植 river material。
- [ ] terrain 内河流 overlay 只作为 fallback。

### P0.5 改造 `TerrainPassInputs.map_set`

- [x] 从 `main.rs` 传入完整 `VanillaMapSet` 或 `VanillaResourceViews`。
- [x] 不允许 TerrainPass 内部自己绕过资源视图体系。

---

## 7. 验收矩阵

| 阶段 | 必须截图 | 必须测试 | 阻塞条件 |
|---|---|---|---|
| Phase 0 | 5 场景 final | capture 命令可重复 | 没有固定相机 |
| Phase 1 | binding debug | critical mock=0 | mock 静默存在 |
| Phase 2 | map_px debug | 坐标转换单测 | tiling 比例错误 |
| Phase 3 | terrain-only | shader validate + screenshot diff | 地形分类错误 |
| Phase 4 | border/secondary debug | target 生成单测 | gradient 缺失 |
| Phase 5 | night lighting | point light 单测 | light map mock |
| Phase 6 | water-only | shader validate | 水体 fallback |
| Phase 7 | river-only | z-fighting 检查 | 河流闪烁 |
| Phase 8 | trees-only | tree mask 单测 | 森林密度错误 |
| Phase 9 | objects-only | mesh/gfx 解析测试 | 资源路径不稳定 |
| Phase 10 | HDR/LDR | tonemap 单测 | ACES 仍在 parity 中 |
| Phase 11 | labels/icons | layout snapshot | 文本重叠 |
| Phase 12 | final | perf + image diff | 回归不可重复 |

---

## 8. 风险与处理

### 8.1 原版资源版权风险

风险：把 Steam 资源提交进仓库或发布。

处理：

- `.gitignore` 明确屏蔽缓存目录。
- 文档写明资源只从本机 `--game-path` 读取。
- 审计工具只输出路径和元数据，不复制原文件。

### 8.2 shader 翻译偏差

风险：手写 WGSL 时公式、gamma、sampler、mip、坐标方向出错。

处理：

- 每个函数旁标注原版来源文件和行号。
- 对 `GetOverlay`、`sample_terrain`、day/night、fog 等写数值单测。
- debug view 显示中间结果。

### 8.3 动态 target 语义不清

风险：`GradientBorderChannel`、`ProvinceSecondaryColorMap` 不是文件资源，必须自己生成，容易误解。

处理：

- 先写 target spec 文档。
- 每个通道输出 debug PNG。
- terrain/water/tree 先只采样 debug target，再接入真实状态。

### 8.4 性能失控

风险：完整原版效果会增加多个 pass 和大贴图。

处理：

- 每阶段加入 profiler。
- 每个新增 pass 同时设计 low-end fallback。
- 先正确，再优化，但不能没有度量。

### 8.5 “看起来更好”干扰 parity

风险：自定义美术调参让截图更好看，但离原版更远。

处理：

- parity 配置和 stylized 配置分离。
- 验收只看 parity 配置。
- 所有自定义增强必须可关闭。

---

## 9. 最终完成定义

满足以下条件才算“原版效果移植完成”：

- [ ] 5 个固定场景，远/中/近三档截图均可复现。
- [ ] critical fallback = 0。
- [ ] terrain、水体、河流、边界、树木、建筑、后处理都有独立 debug 截图。
- [ ] parity 模式下没有非原版自定义调色干扰。
- [ ] terrain-only 与原版在分类、纹理、亮度、雪/泥、城市夜光上接近。
- [ ] water-only 与原版在深浅、反射、冰、海岸上接近。
- [ ] river-only 不闪烁，且材质/流动接近原版。
- [ ] final LDR 颜色由原版式 postprocess 控制。
- [ ] 文档列出所有仍然近似的地方。
- [ ] 项目不分发任何原版资产。

---

## 10. 建议第一批提交拆分

### Commit 1：parity capture 基础设施

- 新增固定相机 preset。
- 新增分层截图输出。
- 新增 report JSON。

### Commit 2：binding audit

- 新增 `BindingAudit`。
- Terrain/Water/Tree 输出 binding 来源。
- parity 模式 critical mock 报错。

### Commit 3：VanillaMapSpace

- 新增坐标转换。
- terrain shader 输出 `map_px`。
- 添加 map grid debug view。

### Commit 4：TerrainPass 去自定义增强

- parity flag。
- 关闭 noise、river overlay、custom tint。
- 保留现有路径作为 non-parity。

### Commit 5：terrain atlas parity 第一版

- terrain id 到 atlas index。
- 4-tap diffuse/normal。
- debug view。

### Commit 6：snow/mud/citylights

- 接入缺失贴图。
- 移植 snow/mud/citylight 公式。

### Commit 7：dynamic targets skeleton

- GradientBorder target。
- ProvinceSecondary target。
- mock 替换为真实但可先输出空状态。

### Commit 8：RiverPass re-enable

- z-bias 修复。
- terrain blue overlay 改 fallback。

---

## 11. 预计工作量

粗略估算：

- Phase 0-2：1-2 周。
- Phase 3 terrain parity：2-3 周。
- Phase 4-7 overlay/water/river/light：3-5 周。
- Phase 8-11 objects/postprocess/labels：4-7 周。
- Phase 12 性能与回归：1-2 周。

总计：约 11-19 周，取决于是否要求像素级对齐、是否要完整解析 `.gfx/.mesh`、是否要把 FOW/point light/game state 全部做到原版语义。

如果只追求“截图肉眼接近”，最短路径是：

1. 固定同图对比。
2. terrain parity。
3. dynamic border/secondary target。
4. river 恢复。
5. water parity。
6. postprocess parity。

这条路径约 4-7 周，能解决当前两张截图中最明显的差距。
