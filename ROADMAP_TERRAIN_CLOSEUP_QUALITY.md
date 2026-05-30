# Roadmap: Terrain Close-Up Quality

> **Historical note (Phase 11, 2026-05-30):** This document is archived. Current terrain pass ownership, fallback policy, debug views, and baseline workflow live in [`MAP_RENDERER_V2_REFACTOR_ROADMAP.md`](./MAP_RENDERER_V2_REFACTOR_ROADMAP.md), [`docs/map_renderer_v2.md`](./docs/map_renderer_v2.md), and [`docs/map_renderer_v2_contributing.md`](./docs/map_renderer_v2_contributing.md).

> 目标：解决相机拉近后地图/地形显得粗糙、像粗模的问题。
>
> 范围只覆盖地形、地图材质、相机近景尺度、LOD/采样质量；不包含树木、UI、玩法逻辑。

## 现状判断

当前近景粗糙的主要原因不是资源没有加载，也不是树木数量，而是渲染尺度和资源精度不匹配：

- 相机最小距离过近，允许玩家把战略地图资源放大到资源本身撑不住的尺度。
- LOD0 地形网格仍偏稀，近景下三角面和高度采样会显得硬。
- heightmap 源数据是 8-bit，上传成 R16 只提升存储格式，不增加真实高度细节。
- 顶点高度使用 nearest `textureLoad`，几何高度没有双线性插值。
- terrain atlas 平铺频率偏低，近看纹理被放大，容易糊/粗。
- terrain.bmp 是像素级地形分类图，近景会看到大块地形类型过渡。

## 关键证据

### 相机允许过近

`crates/hoi4-render/src/camera.rs`

```rust
let min_dist = 1.0;
self.distance = (self.distance * factor).clamp(min_dist, max_dist);
```

当前地图世界尺寸约为：

```text
WORLD_SCALE = 0.02
heightmap = 5632 x 2048
world ~= 112.64 x 40.96
```

`distance = 1.0` 已经是非常近的视角，接近把战略地图位图当近景地形看。

### LOD0 网格密度不足以支撑超近景

`crates/hoi4-render/src/terrain.rs`

```rust
pub const LOD_GRID: [u32; 3] = [64, 32, 16];
```

`crates/hoi4-app/src/main.rs`

```rust
const CHUNKS_X: u32 = 32;
const CHUNKS_Z: u32 = 12;
```

估算：

```text
chunk width = 112.64 / 32 = 3.52 world units
LOD0 quad size = 3.52 / 64 = 0.055 world units
heightmap pixels per quad = 0.055 / 0.02 = 2.75 pixels
```

近景下一个三角网格单元覆盖约 2.75 个 heightmap 像素，容易出现几何粗糙感。

### heightmap 实际仍是 8-bit

`crates/hoi4-app/src/main.rs`

```rust
.map(|&b| (b as u16) << 8)
```

源数据还是 0-255，只是扩展为 R16，并没有增加新的高度细节。

### 顶点高度 nearest 采样

`crates/hoi4-app/src/passes/terrain.wgsl`

```wgsl
fn load_height(uv: vec2<f32>) -> f32 {
    let dim = vec2<f32>(textureDimensions(heightmap_tex));
    let xy = vec2<i32>(clamp(uv, vec2<f32>(0.0), vec2<f32>(1.0)) * dim);
    return textureLoad(heightmap_tex, xy, 0).r;
}
```

fragment 端有 `load_height_bilinear`，但 vertex 端实际几何高度仍是 nearest。

### terrain atlas 平铺频率偏低

`crates/hoi4-app/src/passes/terrain.wgsl`

```wgsl
let tile_uv = fract(world_pos.xz * 0.35);
```

估算：

```text
tile repeat world size = 1 / 0.35 = 2.86 world units
tile repeat source pixels = 2.86 / 0.02 ~= 143 heightmap pixels
```

近景时纹理细节会被放大，产生低清材质感。

## 路线图

### P0: 限制相机近景下限

目标：先阻止用户进入资源分辨率明显撑不住的距离。

建议修改：

`crates/hoi4-render/src/camera.rs`

```rust
let min_dist = 3.0;
```

可选范围：

- `3.0`：保留较近观察能力，粗糙感明显减少。
- `4.0`：更接近战略地图镜头，粗糙感更少。
- `2.5`：折中，仍可能在山地/海岸看到粗糙。

验收：

- 滚轮拉到最近时，地形不再明显像低模三角网格。
- 省份点击、单位显示、POI 显示不受影响。
- 相机 pan/zoom at cursor 行为正常。

### P0: 提高 terrain atlas 近景细节频率

目标：减少近景纹理被过度放大的低清感。

建议修改：

`crates/hoi4-app/src/passes/terrain.wgsl`

```wgsl
let tile_uv = fract(world_pos.xz * 0.8);
```

需要同步修改两处：

- `sample_atlas_tile`
- `sample_atlas_normal_tile`

建议测试值：

- `0.6`：温和提升，重复感较少。
- `0.8`：推荐初始值，近景细节提升明显。
- `1.2`：近景更清晰，但重复纹理可能变明显。

验收：

- 最近镜头下草地/山地/沙地不再大片糊。
- 中远景不能出现明显棋盘式重复。
- terrain atlas normal 与 diffuse 的尺度保持一致。

### P1: 顶点高度改为 bilinear 采样

目标：减少 nearest heightmap 导致的硬折线和块状坡面。

建议方案：

- 在 vertex shader 中改用 `load_height_bilinear` 或新建可用于 vertex 的 bilinear height 函数。
- 法线计算中的 `h_l/h_r/h_d/h_u` 也可以用 bilinear 版本。

风险：

- 海岸线高度过渡会更平滑，但可能改变水陆边缘几何形态。
- 与 WaterPass 的海平面覆盖需要检查 z-fight。
- 省份 picking 使用的 CPU 高度仍是原始 heightmap，视觉点选可能有微小偏差。

验收：

- 近景山坡不再出现明显阶梯/折线。
- 海岸没有新增闪烁或三角缝。
- WaterPass 仍能像素级覆盖海面。

### P1: LOD0 网格加密

目标：提高近景几何精度。

候选方案：

```rust
pub const LOD_GRID: [u32; 3] = [128, 64, 32];
```

或更保守：

```rust
pub const LOD_GRID: [u32; 3] = [96, 48, 24];
```

风险：

- 顶点数增长明显。
- 需要重新评估 frame time。
- LOD 边界缝和 skirt 可能需要重新调。

验收：

- 最近镜头下海岸、小岛、山地轮廓更顺。
- 帧率不出现明显下降。
- LOD 切换没有新增裂缝/黑线。

### P2: 增加近景 detail overlay

目标：用细节贴图掩盖 vanilla 战略地图资源近景分辨率不足。

方案：

- 新增 detail albedo/noise 高频层。
- 新增 detail normal 高频层。
- 根据 terrain id 控制 detail 强度。
- 近景强、远景弱，避免远景花。

示例思路：

```wgsl
let detail = vnoise2d(in.world_pos.xz * 24.0) - 0.5;
let close_detail = smoothstep(0.45, 0.95, params.zoom_factor);
color *= 1.0 + detail * 0.04 * close_detail;
```

风险：

- 纯噪声容易显得程序化。
- 需要与 atlas normal、world normal、postprocess 联调。

验收：

- 近景地表有细颗粒和微变化。
- 中远景不脏、不闪、不噪。

### P2: 地形类型边界软化升级

当前已有 4-tap indexed terrain blending，但近景仍可能看到 terrain.bmp 的分类感。

后续方向：

- 增加基于噪声的 blend 权重扰动。
- 对不同 terrain id 使用不同混合宽度。
- 避免省份/地形边界产生像素块状硬切。

验收：

- 草地到山地、沙地到岩地过渡更自然。
- 不产生明显花边或闪烁。

## 推荐实施顺序

1. P0: `camera.min_dist = 3.0`。
2. P0: terrain atlas scale 从 `0.35` 调到 `0.8`，同步 diffuse/normal。
3. 截图对比最近镜头、中镜头、默认镜头。
4. P1: vertex height bilinear。
5. P1: 评估 `LOD_GRID = [96, 48, 24]` 或 `[128, 64, 32]`。
6. P2: detail overlay。

## 验证清单

- `cargo check --workspace`
- `cargo run --release -p hoi4-app`
- 最近镜头检查：阿尔卑斯、意大利海岸、台湾/日本小岛、北非沙漠。
- 中镜头检查：不能出现重复纹理棋盘感。
- 远镜头检查：不能出现过度噪声、闪烁、LOD 缝。

## 非目标

- 不通过增加树木解决地形粗糙。
- 不优先改 UI。
- 不优先接真实 sky cubemap。
- 不在第一阶段重写整个 terrain pass。
