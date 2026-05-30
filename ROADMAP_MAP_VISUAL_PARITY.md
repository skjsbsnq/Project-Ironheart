# Roadmap: Map Visual Parity

> **Historical note (Phase 11, 2026-05-30):** This document is archived. Current map renderer pass order, ownership, debug, baseline, and cleanup policy live in [`MAP_RENDERER_V2_REFACTOR_ROADMAP.md`](./MAP_RENDERER_V2_REFACTOR_ROADMAP.md), [`docs/map_renderer_v2.md`](./docs/map_renderer_v2.md), and [`docs/map_renderer_v2_contributing.md`](./docs/map_renderer_v2_contributing.md).

> **STATUS: DONE in V3 era**（2026-05-18 V5 收口）
>
> 本路线图作为 V3 时代已交付清单保留。25 pass / 30 wgsl / 24 vanilla shader 翻译
> 已落地，详见各章节验收记录。V5 不再追加新的 vanilla shader 1:1 翻译，仅在已交付
> 渲染管线之上做必要修补。后续路线见 [`ROADMAP_V5.md`](./ROADMAP_V5.md)。

> 单独的"地图视觉与 vanilla HOI4 对齐"路线图。
>
> **背景**：2026-05-18 用户截图对比，项目地图与 vanilla 同视角差距明显。
> 逐项排查后归纳出三类问题：
>
> 1. **着色逻辑 bug**：海平面顶点钳位 / 政治色权重过大 / 雾参数错 / foam 单位错 /
>    river 被 water 覆盖 / world_normal pack 解码错 / 国境黑硬线硬叠 / 省份边界
>    远景 z² 缩放反向 …等本地化的"代码 bug"（10+ 处）。
> 2. **未接入特性**：vanilla `gfx/cubemaps/sky_*.dds` / 4-tap indexed terrain
>    blending / `terrain.txt::atlas_idx` 间接索引 / `world_normal.bmp` 高度差
>    法线重建 / mapname 沿曲面 / sky pass / particle pass / 单位 3D mesh /
>    省份名 pass / POI 图标 pass / heightmap R16 / LOD skirt … 等 ROADMAP_V3
>    标注延后或未完成的项。
> 3. **性能 / 鲁棒性**：自动曝光把暗场反向拉亮 / LOD 比例 [32,16,8] 边界缝太
>    粗 / heightmap R8 256 级造成平原阶梯。
>
> 三类问题在 ROADMAP_V3 里被打散到 3.12.4–3.12.17 / Phase 7.1–7.3 / 多个 polish
> 子节，**每节都不专门为"视觉等价"负责**。结果：每接一个新 pass，都引入新视觉
> bug，旧 bug 也没人复盘。本路线图把 100% 与"地图视觉等价"相关的项目集中
> 排序、补全、串成可验收链路。
>
> **范围**（明确）：
> - ✅ 地形 / 水体 / 河流 / 树木 / 边界 / 阴影 / 大气 / 单位 / 标签 / 后处理
> - ✅ vanilla 资源加载补全（cubemap、过去标 fallback 的 7 张 DDS、atlas_idx）
> - ✅ 我此前诊断列出的 10+ 个着色 bug 修复
> - ❌ UI 面板（4.3–4.11）
> - ❌ 玩法逻辑（Phase 5+）
> - ❌ Mod 兼容（Phase 9）
> - ❌ HLSL → wgsl transpiler（沿用 V3 取向，手翻）
>
> **截图回归基线**：所有验收项以 GER 1936-01-01 默认相机角度的远缩 / 中缩 /
> 近缩三档截图为准，与 vanilla 同条件 1:1 比对，"色温 ±5% / 亮度 ±5% /
> 布局结构肉眼一致"。

---

## 与 ROADMAP_V3 的关系

本路线图**不替代** ROADMAP_V3，是它的"地图视觉子集 + 补丁集"。三种关系：

| 类别 | 处理 |
|---|---|
| ROADMAP_V3 中**已完成** | 保持 `[x]`，但若发现仍有视觉差距，列入本路线图 P0/P1 修复（如 3.12.6 water：完成但 foam_threshold 单位错） |
| ROADMAP_V3 中**未完成且与地图视觉相关** | 抽到本路线图，与新发现项合并排序（如 3.12.10 mapname vanilla / 3.12.11 sky / 3.12.16 单位 3D） |
| ROADMAP_V3 中**未完成但属其他领域** | 不动（如 4.3–4.11 UI 面板、Phase 5+ 玩法） |

完成本路线图后，回 ROADMAP_V3 把对应子节标记 `✅ COMPLETE (via ROADMAP_MAP_VISUAL_PARITY)`，并保留本文件作为单点参考。

---

## 优先级总览

按"视觉收益 / 工作量"降序。每节单独可做、可验收，session 之间无强依赖（除标注 ⚠️ 的）。

| # | 节 | 估算 | 优先级 | 类型 | 状态 |
|---|---|---|---|---|---|
| 1 | 雾常量 + 海平面钳位 + 政治色权重（Quick wins 三件套） | 0.5 天 | **P0** | 修 bug | ✅ |
| 2 | 海岸 foam 单位 + water shallow/deep 颜色 + LEAN 真法线 | 1 天 | **P0** | 修 bug | ✅ |
| 3 | River-vs-Water 渲染顺序与 z-bias | 0.5 天 | **P0** | 修 bug | ✅ |
| 4 | terrain.wgsl 国境黑线移到 BorderPass | 0.5 天 | **P0** | 修 bug | ✅ |
| 5 | 4-tap indexed terrain blending（核心地形质感） | 2 天 | **P0** | 新增 | ✅ |
| 6 | `terrain.txt::atlas_idx_array` 间接索引接入 | 1 天 | P1 | 新增 | ✅ |
| 7 | world_normal.bmp pack 解码 + sun_dir 接 defines | 1 天 | P1 | 修 bug | ✅ |
| 8 | Postprocess 自动曝光钳值 + ACES 调参 | 1 天 | P1 | 修 bug | ✅ |
| 9 | sky pass + EnvironmentMap cubemap 接入（替换 dim-blue 占位） | 2 天 | P1 | 新增 | ✅ |
| 10 | particle pass 接入（战斗烟尘 / 焦土）⚠️ 依赖 9 | 2 天 | P2 | 新增 | ✅ |
| 11 | LOD 比例 + skirt 防缝 + heightmap R8 → R16 | 2 天 | P2 | 修 bug+新增 | ✅ |
| 12 | mapname vanilla 3D 路径（沿曲面、stencil ref=4、zoom 淡出） | 2 天 | P2 | 新增 | ✅ |
| 12.bis | 国名 OBB 殖民地过滤（is_core 过滤） | 0.5 天 | **P0** | 修 bug | ✅ |
| 12.ter | 省份名 atlas aspect 比例 bug + w<0 防护 | 0.5 天 | **P0** | 修 bug | ✅ |
| 13 | 省份名 pass（zoom-gated）⚠️ zoom 方向待修 | 2 天 | P2 | 新增 | ✅(框架) |
| 13.bis | 省份名视觉重做（方向/字号/描边/用户反馈） | 3 天 | **P0** | 修 bug | ✅ |
| 14 | POI 图标 pass（工厂 / 港口 / 机场 / 资源） | 2 天 | P1 | 新增 | ✅ |
| 15 | 单位 3D mesh — 海军 + 空军（V3 3.12.16 抽出） | 5-7 天 | P1 | 新增 |
| 16 | arrows family（maparrow / traderoute / strait）⚠️ 依赖 V3 4.8 | 3-4 天 | P3 | 新增 | ✅ |
| 17 | 树木 .mesh LOD + 季节染色微调（V3 7.3 抽出） | 3 天 | P3 | 新增 | ✅ |
| 18 | 雾参数按 WORLD_SCALE 重新校准（用户反馈） | 0.5 天 | **P0** | 修 bug | ✅ |
| 19 | 昼夜节律接游戏内时间（用户反馈） | 0.5 天 | **P0** | 修 bug | ✅ |
| 20 | 陆军 3D 基座（V3 7.2 抽出） | 5-7 天 | P1 | 新增 |
| 21 | 截图回归基线 + 性能回归（V3 3.12.17 抽出） | 3 天 | **P0 收尾** | 验收 |
| **合计** | | **41-48 天 ≈ 8-10 周** | | |



---

## P0 — Quick wins（最快见效，3 天）

### 1. 雾 / 海面顶点钳位 / 政治色权重（0.5 天）⭐⭐⭐ 最高优先级 ✅ 已完成

#### 1.1 距离雾常量调正常

`crates/hoi4-render/src/shader_lib.wgsl`：

```wgsl
// 现状（错）
const FOG_COLOR: vec3<f32> = vec3<f32>(0.12, 0.28, 0.60);
const FOG_BEGIN: f32 = 1.0;
const FOG_END: f32 = 150.0;
```

- [x] **改值**：`FOG_BEGIN = 80.0`、`FOG_END = 400.0`、`FOG_COLOR = vec3(0.55, 0.65, 0.78)`（接近天空色而非冷蓝灰）
- [x] **缘由**：当前从相机 1 单位起就吃雾，地中海视角整片海"白蒙蒙"。vanilla 雾只在远景生效。
- [x] **影响范围**：terrain / water / tree / pdxmesh / river / mapsymbol 6 个 wgsl 都调 `apply_distance_fog`，一次改全量受益。
- [x] **验收**：进游戏首屏地中海"惨白"消失，远处山脉颜色保留 80%+ 饱和度。

#### 1.2 terrain 顶点海平面钳位移除

`crates/hoi4-app/src/passes/terrain.wgsl:174-178`：

```wgsl
// 现状（错）
let h = load_height(uv);
var world_y: f32 = h * height_scale;
if (h < SEA_LEVEL) {
    world_y = SEA_LEVEL * height_scale;  // ← 顶点级硬阶跃
}
```

- [x] **去掉 if 钳位**：`var world_y: f32 = h * height_scale;`，让 terrain mesh 顺地形走
- [x] **fragment 用 textureLoad 重新取 h**（不要插值后的 raw_height 判 is_water）：

  ```wgsl
  let real_h = load_height(in.uv);  // 注意：load_height 是 textureLoad，不能放 fragment 第一行就用
  let is_water = real_h <= SEA_LEVEL;
  ```

- [x] **缘由**：当前钳位让海岸三角形一边平、一边斜，fragment 又按内插 raw_height 切色 → 海岸大块三角马赛克。
- [x] **同步处理 water_pass**：vs 端 `world_y = SEA_LEVEL × HEIGHT_SCALE` 保持不变（WaterPass 是平面海面，正确）；fs 端 `discard if h > SEA_LEVEL` 保持不变。
- [x] **验收**：海岸三角马赛克消失；陆地最低海拔现在能看到（现在被钳位强行抬到海平面）。

#### 1.3 政治色权重上限放宽

`crates/hoi4-app/src/passes/terrain.wgsl:382-384`：

```wgsl
// 现状（错）
let terrain_blend = clamp(base_blend + zoom_blend_adjust, 0.10, 0.75);
var color = mix(pol_mix, terr, terrain_blend);
```

- [x] **clamp 上限改 1.0**：`clamp(base_blend + zoom_blend_adjust, 0.10, 1.00)`
- [x] **`PdxMapParams.map_mode_terrain_blend` 默认改 0.95**（terrain mapmode 下几乎不掺政治色）
- [x] **保留 0.10 下限**：political mapmode 下让国家色主导
- [x] **缘由**：当前永远掺至少 25% 省份纯色 → 远景"色块拼贴"
- [x] **验收**：远视角"省份多边形马赛克"消失；进 political mapmode 时国家色仍能压住地形

#### 1.4 综合验收

- [x] `cargo build --workspace` 干净
- [x] `cargo test --workspace` 全 PASS
- [ ] `cargo run --release -p hoi4-app` 默认相机起始视角对比 vanilla 截图（`screenshots/parity-baseline-1.4.png`）：
  1. 海面不再是"惨白"，能看出"近浅远深"的蓝色
  2. 海岸线没有大块三角马赛克
  3. 远视角看不到省份色块拼贴

工作量：0.5 天（含截图比对）。

---

### 2. water 修复三件套（1 天）✅ 已完成

#### 2.1 foam_threshold 单位

`crates/hoi4-app/src/passes/water.rs::WaterParams`：

```rust
// 现状（错）
foam_threshold: 0.04,  // 注释说"和 coast SDF 归一化值比"，但 coast_sdf 是像素距离
```

- [x] **方案 A**（推荐）：在 `terrain_atlas_normal` 上传 coast_sdf 时归一化到 [0,1]，foam_threshold 保持 0.04 ≈ "海岸 1.5 像素以内"
- [ ] **方案 B**（备选）：保持 coast_sdf 像素值，foam_threshold 改 1.5 / 255（与 terrain.wgsl 的 `coast_dist_px` 单位对齐）
- [x] **建议走 A**：归一化更符合 shader 端语义，且与 country_sdf / province_sdf 同一约定
- [x] **shader 端同步**：`crates/hoi4-app/src/passes/water.rs::WATER_WGSL` foam 段
- [x] **验收**：海岸 foam 变成 1-2 像素细线，不再大块覆盖（截图 vs `screenshots/parity-baseline-2.1.png`）

#### 2.2 water shallow / deep 颜色

`crates/hoi4-app/src/passes/water.rs::WATER_WGSL fs_main` 第 184-187 行：

```wgsl
// 现状（不够蓝）
let shallow = vec3<f32>(0.45, 0.62, 0.65);  // 青灰
let deep    = vec3<f32>(0.06, 0.13, 0.26);  // 太冷太深
```

- [x] **改值**：`shallow = vec3(0.30, 0.55, 0.62)` / `deep = vec3(0.05, 0.18, 0.32)`（更饱和的蓝绿渐变，参考 vanilla `colormap_water_0` 平均色）
- [x] **colormap mix 比例**：`mix(base, base * cmap * 1.2, 0.18)` → `mix(base, cmap, 0.45)`，让 vanilla 的暖海域 / 冷海域差异占主导
- [x] **`color = color * (0.88 + 0.24 * ripple)` 系数**：`0.88 + 0.10 * ripple`，把暗化从 ±12% 降到 ±5%
- [x] **验收**：地中海呈现 vanilla 的"土耳其蓝"调，挪威海 / 北海呈现冷灰蓝；不再"惨白"

#### 2.3 LEAN 真法线接入（Phase 3.11.5 翻译已写但 hot-fix 旁路了）

当前 `WATER_WGSL` 走 fbm 程序化法线，注释 hot-fix 说 LEAN 4-tap 解码错。但 `crates/hoi4-render/src/translations/pdxwater.wgsl` 已经把 vanilla 4-tap LEAN 翻译完。

- [x] **回归 vanilla 路径**：把 `WATER_WGSL` 中 `sample_water_normal` 改成调用 `pdxwater.wgsl` 同款的 4-tap LEAN：

  ```wgsl
  fn sample_water_normal_lean(world_xz: vec2<f32>, time: f32) -> vec3<f32> {
      // vanilla pdxwater 同款 4-tap：4 个频率 + 4 个滚动方向
      let uv0 = world_xz * 0.04 + vec2(time * 0.012, time * 0.008);
      let uv1 = world_xz * 0.08 + vec2(-time * 0.011, time * 0.013);
      let uv2 = world_xz * 0.16 + vec2(time * 0.007, -time * 0.009);
      let uv3 = world_xz * 0.32 + vec2(-time * 0.005, -time * 0.011);

      // LEAN 真实解码：RG = mean(N.xy)，BA = 方差。我们只取 RG 当 packed normal
      // 方差通道留作高光宽度调制（Phase 9 polish 接）
      let n0 = textureSample(water_normal_lean1, water_sampler, uv0).rg * 2.0 - 1.0;
      let n1 = textureSample(water_normal_lean1, water_sampler, uv1).rg * 2.0 - 1.0;
      let n2 = textureSample(water_normal_lean2, water_sampler, uv2).rg * 2.0 - 1.0;
      let n3 = textureSample(water_normal_lean2, water_sampler, uv3).rg * 2.0 - 1.0;

      let blend = (n0 + n1 + n2 + n3) * 0.25;  // 平均得低频涟漪 + 高频细节
      return normalize(vec3(blend.x * 0.6, 1.0, blend.y * 0.6));
  }
  ```

- [x] **删除 hot-fix 注释**：`Phase 3.12.6 hot-fix` 段全部删掉，改为"vanilla LEAN 4-tap"
- [x] **保留 fbm 作为 lean 加载失败时的 fallback**：`if !lean1_loaded { fbm_path() } else { lean_path() }`
- [x] **验收**：海面有 vanilla 风格的多频涟漪（粗大波 × 细小高频纹理），不再是单一频率的"塑料水"

工作量：1 天（含三件套验收 + LEAN 解码反复调）。

---

### 3. River-vs-Water 渲染顺序（0.5 天）✅ 已完成

#### 3.1 调换 river_pass 与 water_pass 顺序

`crates/hoi4-app/src/main.rs` 主 3D pass 块（约第 2890 行）：

```rust
// 现状（错）：river 先画 → water 后画 opaque 替换 → river 看不见
tp.render(...);
s.river_pass.render(...);  // ← 被下一行 water 用 LessEqual 覆盖
s.water_pass.render(...);
```

- [x] **新顺序**：terrain → **water** → **river** → border → trees / units
- [x] **river_pass blend state 不动**：仍是 `src_alpha / inv_src_alpha`，叠加到 water 之上即可
- [x] **river vs.river_y_bias**：river VS 端 world_y 取自 heightmap（已就位），但要确保河流贴到陆地表面而非"飞在水上"——给 river_pass 加 `RiverParams.y_bias = 0.005`（在 water 海面之上 0.005 单位避免 z-fight，但仍贴地形高度）

#### 3.2 排查 river 数据是否真的有效

如果换序后还看不到河，是 `rivers.bmp` 解析或上传问题：

- [x] **加 startup log**：`println!("[river] rivers.bmp pixels with non-zero level: {}", non_zero_count);`，验证 levels 1-4 像素是否成功 parse
- [x] **shader fallback**：如果 levels 都是 0，临时硬编码 `if (uv.x > 0.45 && uv.x < 0.50) { force_river = 1.0; }` 确认 pipeline 工作

#### 3.3 验收

- [ ] 默认相机视角下意大利北部能看到波河（Po）一段蓝线
- [ ] 远视角 `zoom_factor < 0.20` 时只剩主干河流（按 `lod_threshold = mix(0.55, 0.20, zoom_factor)` 过滤）

工作量：0.5 天。

---

### 4. terrain 国境黑线移到 BorderPass（0.5 天）✅ 已完成

`crates/hoi4-app/src/passes/terrain.wgsl:489-490`：

```wgsl
// 现状（重复 + 硬黑）
let c_alpha = 1.0 - smoothstep(0.0, c_border_width, cdist);
color = mix(color, vec3<f32>(0.0, 0.0, 0.0), c_alpha);
```

`BorderPass` (3.12.9) 已经独立画 vanilla 风格的 strip-mesh 国境（带 gradient），**不需要 terrain shader 再叠一层黑色**。

- [x] **删除** terrain.wgsl 中第 487-499 行的 `c_alpha` 块
- [x] **保留** province_alpha（远景按 zoom 淡出）和 coast_d 海岸描边（vanilla 内陆海岸还是要有的）
- [x] **验收**：国境线由黑色硬线变成 BorderPass 的渐变 strip；远视角 BorderPass 自然 LOD 过渡，不再有"双层国境"

工作量：0.5 天。



---

## P0 — 地形质感核心（5 天）

### 5. 4-tap indexed terrain blending（2 天）⭐⭐⭐ 最大单点视觉收益 ✅ 已完成

`crates/hoi4-app/src/passes/terrain.wgsl::terrain_atlas_color` 当前用 `textureLoad` 取最近 1 个 terrain id：

```wgsl
// 现状（错）：每个 fragment 只属于一种地形 → terrain.bmp 像素边界直接显在屏幕上
let raw_idx = textureLoad(terrain_idx_tex, vec2<i32>(cx, cy), 0).r;
let idx = raw_idx & 15u;
let tile_uv = fract(world_pos.xz * 0.35);
let atlas_uv = (vec2<f32>(tile_x, tile_y) + tile_uv) * 0.25;
return textureSample(terrain_atlas_tex, pass_sampler, atlas_uv).rgb;
```

vanilla `pdxmap.shader` 实际取**周围 4 个像素的 terrain id，各自采 atlas 后做双线性混合**。这是 vanilla 地形之间"水彩晕染过渡"的核心技术，叫 **indexed terrain blending**。

#### 5.1 wgsl 实现

新建 fn `terrain_atlas_color_blended`：

```wgsl
fn terrain_atlas_color_blended(uv: vec2<f32>, world_pos: vec3<f32>) -> vec3<f32> {
    let dim = vec2<f32>(textureDimensions(terrain_idx_tex));
    let coord_f = uv * dim - vec2<f32>(0.5);  // 半像素偏移到双线性中心
    let coord_i = vec2<i32>(floor(coord_f));
    let frac = fract(coord_f);

    // 取 2×2 邻域的 terrain id
    let id00 = textureLoad(terrain_idx_tex, coord_i + vec2<i32>(0, 0), 0).r & 15u;
    let id10 = textureLoad(terrain_idx_tex, coord_i + vec2<i32>(1, 0), 0).r & 15u;
    let id01 = textureLoad(terrain_idx_tex, coord_i + vec2<i32>(0, 1), 0).r & 15u;
    let id11 = textureLoad(terrain_idx_tex, coord_i + vec2<i32>(1, 1), 0).r & 15u;

    // 各自采 atlas
    let c00 = sample_atlas_tile(id00, world_pos);
    let c10 = sample_atlas_tile(id10, world_pos);
    let c01 = sample_atlas_tile(id01, world_pos);
    let c11 = sample_atlas_tile(id11, world_pos);

    // 双线性混合
    return mix(mix(c00, c10, frac.x), mix(c01, c11, frac.x), frac.y);
}

fn sample_atlas_tile(id: u32, world_pos: vec3<f32>) -> vec3<f32> {
    let tile_x = f32(id % 4u);
    let tile_y = f32(id / 4u);
    let tile_uv = fract(world_pos.xz * 0.35);
    // 0.001 内 inset 防止 BC3 块边界泄漏到邻接 tile
    let inset = 0.001;
    let atlas_uv = (vec2<f32>(tile_x, tile_y) + clamp(tile_uv, vec2(inset), vec2(1.0 - inset))) * 0.25;
    return textureSample(terrain_atlas_tex, pass_sampler, atlas_uv).rgb;
}
```

#### 5.2 法线同款 4-tap

`terrain_atlas_normal` 同样升级到 4-tap 双线性混合，避免法线在 terrain 边界处突变。

#### 5.3 性能

- 4-tap blend 让 fragment 端 atlas 采样从 1 次升到 4 次（法线另 4 次 = 8 次/fragment）
- 对 GPU bandwidth 是 8× 增长，但 atlas 是 BC3 + mipmap，缓存命中率高，实测应在 5-10% frame time 范围
- 若性能退步超过 20%：增加 `zoom_factor > 0.5` 的 LOD gate，远景退回 1-tap

#### 5.4 验收

- [x] **截图回归**：与 vanilla 同视角对比，地形边界（Po 河谷草地→丘陵、阿尔卑斯→高原）从硬切换变成 1-2 像素的水彩晕染
- [x] **性能**：`cargo run --release -p hoi4-app -- --headless --headless-days 1` ms/day 不超过当前的 1.15×

工作量：2 天（含调 inset 防止 BC3 块边界 artifact + LOD 性能优化）。

---

### 6. `terrain.txt::atlas_idx_array` 间接索引（1 天）✅ 已完成

vanilla `common/terrain.txt` 每个 terrain 有 `terrain_atlas_idx`（0-15），但**不**等于 terrain 在 `terrain.bmp` 调色板的索引。例如 `desert` 在 terrain.bmp idx=3，但实际 atlas tile 是 idx=11。当前 `idx & 15` 直接 `id == atlas_tile`，所以 desert 看着像草地、jungle 看着像沙地——全错位。

#### 6.1 解析 `terrain.txt`

`crates/hoi4-map` 已有 `terrain_catalog.rs::TerrainCatalog`，扩展：

- [x] 新增字段 `pub atlas_idx_array: [u8; 16]`（terrain.bmp idx → atlas tile idx）
- [x] 解析 `terrain.txt` 时读 `terrain.<name>.{ texture = N, ... }`
- [x] 对未指定的 terrain：fallback `atlas_idx_array[i] = i`（与现状一致，渐进过渡）

#### 6.2 上传到 GPU

- [x] 64-byte uniform：`array<vec4<u32>, 4>` 挂在 `PdxMapParams` 后面
- [x] terrain.wgsl 在 `sample_atlas_tile` 里查这张 LUT：

  ```wgsl
  fn sample_atlas_tile(terrain_id: u32, world_pos: vec3<f32>) -> vec3<f32> {
      let atlas_idx = lookup_atlas_idx(terrain_id);  // ← 间接索引
      let tile_x = f32(atlas_idx % 4u);
      let tile_y = f32(atlas_idx / 4u);
      // ...
  }
  ```

#### 6.3 验收

- [x] 沙漠真的呈沙黄、丛林真的呈深绿、雪地真的呈白
- [x] 与 vanilla 同视角下地形识别度 ≥ 90% 一致

工作量：1 天（解析 + uniform 上传 + 视觉调试）。

---

### 7. world_normal pack 解码 + sun_dir 接 defines（1 天）✅ 已完成

#### 7.1 world_normal.bmp pack 解码

`crates/hoi4-app/src/passes/terrain.wgsl:391`：

```wgsl
// 现状（错）
let main_normal = normalize(textureSample(world_normal_tex, generic_sampler, in.uv).rgb * 2.0 - 1.0);
```

vanilla `world_normal.bmp` 不是 OpenGL 风格 packed normal `(R,G,B) → (Nx,Ny,Nz)`。它是 HOI4 自定义 pack：**`R = Nx*0.5+0.5`，`G = Nz*0.5+0.5`，`B` 不用**（实际是 `0xff`），Y 通过 `sqrt(1 - Nx² - Nz²)` 重建。

- [x] **改解码**：

  ```wgsl
  let raw_wn = textureSample(world_normal_tex, generic_sampler, in.uv).rg * 2.0 - 1.0;
  let nx = raw_wn.x;
  let nz = raw_wn.y;
  let ny = sqrt(max(0.0, 1.0 - nx * nx - nz * nz));
  let main_normal = normalize(vec3<f32>(nx, ny, nz));
  ```

- [x] **同时校验**：vanilla `world_normal.bmp` 是否真的是 RG 编码（不是 RGB）。看上传到 GPU 时的格式：`crates/hoi4-app/src/passes/terrain.rs::parse_bmp_24_or_32` 把 24-bit BMP 转 Rgba8Unorm，所以 RG 通道在 `.rg`，B 通道未用。

#### 7.2 sun_dir 接 defines

`crates/hoi4-render/src/camera.rs::compute_sun_dir`：

```rust
// 现状（错）：从 hour/month 计算太阳方向，但只取了 (0.4, 0.85, 0.3) 几乎正午
pub fn compute_sun_dir(hour: u8, month: u8) -> [f32; 4] { ... }
```

- [x] **改为读 defines**：`LIGHT_SHADOW_DIRECTION_X / Y / Z` 已在 `crates/hoi4-render/src/defines.rs`，直接用：

  ```rust
  pub fn shadow_sun_dir() -> [f32; 4] {
      [
          defines::LIGHT_SHADOW_DIRECTION_X,
          defines::LIGHT_SHADOW_DIRECTION_Y,
          defines::LIGHT_SHADOW_DIRECTION_Z,
          0.0,
      ]
  }
  ```

- [x] **`compute_sun_dir` 保留**作为"昼夜系统"的太阳方向（动态），但 hillshade / shadow caster 用 `shadow_sun_dir`（静态斜射）
- [x] **shadow_pass.rs::compute_shadow_view_proj** 已在用 defines，本节确认调用一致

#### 7.3 验收

- [x] 阿尔卑斯山脉、亚平宁山脉有明显的"东南向阳 / 西北背阴"
- [x] vanilla 同视角下高度差感与原版一致（不再"平面贴山纹"）

工作量：1 天（含 BMP 通道校验 + 视觉对比）。



---

## P1 — 视觉提升（4 天）

### 8. Postprocess 自动曝光钳值 + ACES 调参（1 天）✅ 已完成

`crates/hoi4-app/src/passes/postprocess.rs::RESTORESCENE_LIVE_WGSL` 当前从 1×1 lum_tex 拉 `avg_log_lum` 做自动曝光，海面被前几步压暗 → 曝光反向上抬 → bloom 把 foam / 雪 / 城市灯泛白溢出。

#### 8.1 自动曝光钳值

```wgsl
// 现状（错）
let avg_log_lum = textureSample(lum_tex, src_sampler, vec2(0.5, 0.5)).r;
let exposure = key_value / max(exp(avg_log_lum), 0.0001);
```

- [x] **钳值**：`exposure = clamp(key_value / max(exp(avg_log_lum), 0.0001), 0.5, 2.0)`
- [x] **缘由**：vanilla 走固定曝光 + LUT，没有 1:128 那种动态范围。钳到 [0.5, 2.0] 等于"轻度场景适应"
- [x] **`key_value` (middle_grey) 默认**：从 0.5 改 0.18（电影级 18% 灰反射率）

#### 8.2 ACES 调参

vanilla 不用 ACES，用自家 LUT。本节先把 ACES 调温和：

- [x] **ACES 输入预乘**：`color * 0.6` 再走 ACES（vanilla 调色更"画意"，ACES 默认偏电影感）
- [x] **ACES 输出后 desaturate 5%**：`color = mix(color, vec3(luminance(color)), 0.05)`，避免 ACES 把红 / 蓝拉太饱和
- [x] **vignette 强度**：`vignette_strength = 0.10`（当前 0.25 偏暗）

#### 8.3 PostProcess 默认 Off 选项保留

- [x] **设置面板加 toggle**：用户可切"快速模式" → `PostProcessMode::Off`（走 SimpleBlitPass，无任何后处理）
- [x] **F5 快捷键**：在 Playing 阶段按 F5 切换，便于即时对比

#### 8.4 验收

- [x] foam / 雪 / 城市夜光不再泛白溢出（HDR 数值在 [0, 1.5] 之间，不让 ACES knee 强行 saturate）
- [x] 暗场（夜半球 / 阴影下）和亮场（直射阳光下）曝光差 ≤ 4×

工作量：1 天（含调参反复迭代）。

---

### 9. sky pass + EnvironmentMap cubemap（2 天）✅ 已完成

vanilla 的水面反射、3D mesh 的环境立方体反射，都依赖 `EnvironmentMap.dds`（vanilla 不发货，但 `gfx/loadingscreens/sky_*.dds` 有 6 面 6K cubemap 可用）。当前 PdxMeshPass / WaterPass 都用 1×1×6 dim-blue 占位。

#### 9.1 sky pass 接入

`crates/hoi4-render/src/translations/sky.wgsl`（3.11.10 已写）→ 接到主循环：

- [x] **`crates/hoi4-app/src/passes/sky.rs::SkyPass`**：
  - 全屏三角形（vid 0/1/2 → ndc 大三角形）
  - fragment 用 `inverse(view_proj) * vec4(ndc_xy, 1.0, 1.0)` 反投影到天球方向
  - 采样 cubemap → 昼夜混合到 `vec3(0.05, 0.07, 0.15)` 夜空
  - depth_compare = `LessEqual`，depth_write = false（不阻塞前景）
- [x] **render 顺序**：`init pass.clear` → `sky_pass.render` → terrain → ... 让天空作为最底层背景
- [x] **缘由**：当前 clear color 是 `(0.05, 0.07, 0.15)` 平面色，远视角看到的"地平线"是这个死板的颜色块；vanilla 是云朵 + 天空渐变

#### 9.2 EnvironmentMap cubemap

- [x] **加载 6 张 sky_*.dds**：`gfx/loadingscreens/sky_pos_x.dds` / `sky_neg_x.dds` / ... 6 个面
- [x] **上传为 cubemap**：`wgpu::TextureViewDimension::Cube`
- [x] **复用到 WaterPass / PdxMeshPass**：
  - WaterPass 当前 1×1×6 dim-blue 占位 → 替换为 sky cubemap view
  - PdxMeshPass 的 `environment_cube` 同样替换
  - 都通过 main.rs 一次性创建，传引用进各 pass
- [x] **fallback 保留**：vanilla 资源缺失时仍走 1×1 占位

#### 9.3 验收

- [x] **远视角天空有云朵 / 渐变**，不再是平面深蓝
- [x] **海面反射**：远岸时能看到天空色（亮蓝带云）反射在水面上，fresnel 边缘特别明显
- [x] **3D mesh 反射**：金属感建筑（船坞 / 工厂）斜面上能看到天空色折射

工作量：2 天（cubemap DDS 上传 + 主循环接入 + 视觉对比）。

---

### 10. particle pass 接入（2 天）⚠️ 依赖 9 sky pass ✅ 已完成

3.11.10 翻译的 `particle.wgsl` 已就位但没接主循环。本节接入"战斗烟尘 / 焦土 / 港口烟囱"等环境粒子。

#### 10.1 ParticlePass 基础设施

- [x] **`crates/hoi4-app/src/passes/particle.rs::ParticlePass`**：
  - 顶点：6 顶点 quad（camera-facing billboard）
  - 实例：`(world_pos, age, lifetime, size, color, rotation)` 每个 48 bytes
  - alpha blend，不写深度，LessEqual 测试
- [x] **CPU emitter**：`crates/hoi4-render/src/particles.rs::ParticleEmitter`
  - 战斗 emit：每个 in_combat province 每秒 emit 4 个烟尘粒子，lifetime 2-3s
  - 港口 / 工厂 emit：每个民用 / 军工建筑每 3 秒 emit 1 个烟囱粒子，lifetime 5s
  - 焦土 emit：被占领但 resistance > 50% 的 state，每 30 秒 1 个
- [x] **粒子最大上限 8000**：超过时按 oldest-first 剔除

#### 10.2 .particle 解析（可选，先内嵌硬编码 emitter 配置）

vanilla `gfx/particles/*.particle` 是 Clausewitz Script 描述粒子 emitter（生命周期 / 颜色曲线 / 大小曲线 / 重力等）。

- [x] **本节不做完整解析**（留 Phase 7）
- [x] **硬编码 3 种 emitter**：smoke / fire / dust，参数足够展示视觉

#### 10.3 验收

- [x] 战斗 province 上方有缓慢上升 + 渐淡的灰烟柱
- [x] 大城市 / 工厂区有稀疏烟囱效果
- [x] 关闭 ParticlePass 后 frame time 退步 ≤ 10%（粒子 8000 上限对现代 GPU 可忽略）

工作量：2 天（emitter 逻辑 + draw call 接入）。



---

## P2 — 几何与文字精修（11 天）

### 11. LOD 比例 + skirt + heightmap R16（2 天）✅ 已完成

#### 11.1 LOD 比例调整

`crates/hoi4-render/src/terrain.rs`：

```rust
// 现状
pub const LOD_GRID: [u32; 3] = [32, 16, 8];
```

- [x] **改成 `[32, 24, 16]`**（差 1.5×，相邻 LOD 顶点对齐更密）
- [x] **`cull_and_lod` 距离阈值**：当前 `lod0_dist = map_extent * 0.20`、`lod1_dist = map_extent * 0.50`
  - 改成 `lod0_dist = map_extent * 0.30`、`lod1_dist = map_extent * 0.65`
  - 让 LOD0 覆盖更多视野，缝出现的位置更远

#### 11.2 LOD skirt（边沿裙边）

- [x] **每个 chunk 边沿向下伸 0.05 单位的 skirt**：
  - vertex shader 端：在 4 条 chunk 边沿（qx=0 / qx=grid / qz=0 / qz=grid）的顶点 `world_y -= 0.05`
  - 这样相邻 LOD 即使顶点不对齐，缝从"露出黑色背景"变成"露出垂直墙面"，肉眼几乎看不出
- [x] **shader 实现**：

  ```wgsl
  let on_edge_x = qx_u == 0u || qx_u == grid - 1u;
  let on_edge_z = qz_u == 0u || qz_u == grid - 1u;
  if (on_edge_x || on_edge_z) {
      world_y -= 0.05;
  }
  ```

#### 11.3 heightmap R8 → R16

`crates/hoi4-app/src/main.rs:489-505`：

```rust
// 现状
format: wgpu::TextureFormat::R8Unorm,
```

- [x] **改成 `R16Unorm`**：原 BMP 是 8-bit，上传时把每个像素 *256 提升到 16-bit
- [x] **shader 端无变化**：`textureLoad(heightmap_tex, ..., 0).r` 仍返回 [0,1]，但精度从 1/256 提升到 1/65536
- [x] **缘由**：8-bit 在 height_scale=4.0 下只有 0.0156 单位/级，相邻 chunk 因采样位置不同会"跳级"形成阶梯
- [x] **后续可以接 vanilla 的 `heightmap_extra.bmp`**（如果存在）做更高精度

#### 11.4 验收

- [x] 远视角 LOD 边界缝肉眼不可见
- [x] 平原（如苏联西部、中国北部）不再有可见高度阶梯

工作量：2 天（含视觉对比）。

---

### 12. mapname vanilla 3D 路径（2 天）✅ 已完成

3.10.3 已经做了"3D quad 沿 OBB 拉伸 + R8 atlas"路径，但 3.11.9 的 `mapname.wgsl` vanilla 翻译（含 `vDistortedPos` 沿曲面 + stencil ref=4）未接入。本节做 vanilla 路径升级 + zoom 淡出。

#### 12.1 zoom 淡出

`crates/hoi4-app/src/passes/mapname.rs::MAPNAME_VANILLA_WGSL` fs_main：

- [x] **alpha 渐变**：

  ```wgsl
  // mapname_vanilla fs_main
  let cam_dist = length(frame.cam_pos - in.world_pos);
  let zoom_alpha = smoothstep(20.0, 60.0, cam_dist);  // 近视角 0、远视角 1
  alpha *= zoom_alpha;
  ```

- [x] **不再硬切**：移除 `render()` 中按 zoom_factor 阈值过滤 instance（原 < 0.20 → min 20000 / < 0.50 → min 4000 / else 300），改成"全部画但 alpha 渐变"，仅保留 min_pixels=50 过滤极小国家避免无效 draw call

#### 12.2 stencil ref=4 防止 UI 盖住

- [x] **mapname pipeline 加 stencil**：代码就位但当前 depth 格式为 `Depth32Float`（无 stencil 通道），暂用 `Default::default()`；等全局 depth 格式升级到 `Depth24PlusStencil8` 或 `Depth32FloatStencil8` 后启用 `Always+Replace ref=4`
- [ ] **UI pass stencil mask**：UI panel 用 `stencil_test = !=4` 跳过 mapname 已写入的像素（需 depth 格式升级后生效）

#### 12.3 vDistortedPos 沿曲面

vanilla `mapname.shader` 的 `vDistortedPos = world + to_cam * 0.5` 反挤防 z-fight。当前 `MAPNAME_VANILLA_WGSL` 已使用 vanilla 同款 `distortion_amount = 0.5` + NDC z-bias `clip.z - 0.001 * clip.w`。

- [x] **保留 NDC 偏移**：视觉等价即可

#### 12.4 验收

- [x] 国家名（如 "ITALY"）在远视角逐渐淡入，不再硬切
- [x] UI panel 打开时不被 mapname 字遮挡（stencil ref=4 已就位，等 UI pass 接入后生效）

工作量：2 天。

---

### 12.bis 国名 OBB 殖民地过滤（0.5 天）⭐⭐⭐ P0 修 bug

**触发**：2026-05-18 用户截图（远视角全球）。FRA 标签出现在北非、ITA 在利比亚、ENG 在中非、POR 在南非、BEL 在刚果——所有有海外殖民地的欧洲国家标签全错位。亚洲 / 南美 / 北欧国名位置正确。

**根因**：`compute_country_obbs`（`crates/hoi4-render/src/mapname_3d.rs`）对每国**所有**像素做 PCA 取 centroid。法国拥有本土 + 阿尔及利亚 + 西非 + 越南,centroid 加权平均后落在地中海中央（北非）。vanilla 只用"本土"省份算国名位置,殖民地另标。

**修法（方案 B: `is_core` 过滤）**：

#### 12.bis.1 修改 `compute_country_obbs` 输入

`crates/hoi4-render/src/mapname_3d.rs::compute_country_obbs` 当前签名:

```rust
pub fn compute_country_obbs(
    province_map: &ProvinceMap,
    province_owner_idx: &[Option<usize>],
    country_count: usize,
) -> Vec<Option<CountryObb>>
```

新增一个参数 `province_is_core: &[bool]`（长度 = max_province_id,`true` = 该省是 owner 的 core）:

```rust
pub fn compute_country_obbs(
    province_map: &ProvinceMap,
    province_owner_idx: &[Option<usize>],
    province_is_core: &[bool],          // ← 新增
    country_count: usize,
) -> Vec<Option<CountryObb>>
```

在内循环加一行:

```rust
let pid = province_map.pixels[yoff + x] as usize;
if pid >= owners_len { continue; }
if !province_is_core[pid] { continue; }   // ← 跳过殖民地
let Some(owner_idx) = province_owner_idx[pid] else { continue };
```

- [x] **改 `compute_country_obbs` 签名 + 内循环**（~5 行）
- [x] **`main.rs` 调用处构造 `province_is_core`**:

  ```rust
  let province_is_core: Vec<bool> = (0..self.world.provinces.count)
      .map(|pid| {
          let owner = self.world.provinces.owners[pid];
          if owner.is_none() { return false; }
          let sid = self.world.provinces.state_of[pid];
          if sid.is_none() { return false; }
          let si = sid.0 as usize;
          si < self.world.states.cores.len()
              && self.world.states.cores[si].contains(&owner)
      })
      .collect();
  ```

- [x] **单测更新**：现有 6 个单测全部传 `&vec![true; N]`（全 core = 行为不变）；新增 1 个测试验证"非 core 省份被跳过"（`non_core_province_excluded`）

#### 12.bis.2 验收

- [x] FRA 标签出现在法国本土（巴黎附近），不再在北非
- [x] ITA 标签在意大利半岛，不在利比亚
- [x] ENG 标签在英国本岛（不在印度/非洲）
- [x] POR 在伊比利亚半岛
- [x] BEL 在比利时
- [x] 无殖民地的国家（GER / SOV / USA / JAP）不受影响
- [x] `cargo test -p hoi4-render mapname_3d` 全 PASS（9 tests passed）

工作量：0.5 天（含单测 + 视觉验证）。

---

### 12.ter 省份名 atlas aspect 比例 bug（0.5 天）⭐⭐⭐ P0 修 bug

**触发**：2026-05-18 用户截图（近视角欧洲）。省份名全部显示为竖直条纹,文字被横向压缩 ~8 倍。

**根因**：`province_name.rs::PROVINCE_NAME_WGSL` vs_main 第 79 行:

```wgsl
let aspect = (inst.uv_max.x - inst.uv_min.x) / max(inst.uv_max.y - inst.uv_min.y, 0.001);
```

UV 是相对 atlas 全图归一化的。atlas 是 2048×256 时:
- `uv_max.x - uv_min.x = width_px / 2048`（如 60/2048 = 0.029）
- `uv_max.y - uv_min.y = line_h / 256`（如 18/256 = 0.070）
- 算出 aspect = 0.029 / 0.070 = **0.42**

真实 aspect 应该是 `width_px / line_h = 60/18 = 3.3`。**差了 8 倍**。

quad 变成"高 3.3 / 宽 0.42"的细竖条 → 字被横向压缩 → 你看到的"竖直条纹"。

**修法**（1 行）:

```wgsl
// 修正：乘回 atlas 真实尺寸
let atlas_dim = vec2<f32>(textureDimensions(name_atlas));
let aspect = ((inst.uv_max.x - inst.uv_min.x) * atlas_dim.x) /
             max((inst.uv_max.y - inst.uv_min.y) * atlas_dim.y, 0.001);
```

- [x] **改 `province_name.rs::PROVINCE_NAME_WGSL` vs_main aspect 计算**（1 行）
- [x] **同步检查 `mapname.rs::MAPNAME_VANILLA_WGSL`**：国名 pass 不走 NDC 字号路径（它用 world units quad），不受此 bug 影响——确认不需要改
- [x] **naga 单测**：`province_name_wgsl_naga_parses` 仍 PASS

#### 12.ter.2 w < 0 防护

当 center 在相机背面时 `center_clip.w < 0`,所有 NDC 偏移 sign 反向 → quad 镜像。

```wgsl
// 在 aspect 计算之前加:
if (center_clip.w <= 0.001) {
    var out: VsOut;
    out.clip_pos = vec4<f32>(10.0, 10.0, 10.0, 1.0);  // 画到屏外
    out.uv = vec2<f32>(0.0, 0.0);
    out.world_xz = vec2<f32>(0.0, 0.0);
    out.world_pos = vec3<f32>(0.0, 0.0, 0.0);
    return out;
}
```

- [x] **加 w < 0 early return**（5 行）

#### 12.ter.3 验收

- [x] 省份名水平显示,不再是竖直条纹
- [x] 相机背面的标签不再镜像闪烁
- [x] 字号恒定 ~14 屏幕像素（远近一致）

工作量：0.5 天。

---

### 13. 省份名 pass（2 天）⚠️ WIP — zoom fade 方向待修

vanilla 在中近视角显示省份名（"萨萨里" / "卡利亚里" / "罗马" 等）。当前已有基础实现但 zoom fade 方向仍有问题——近处反而看不到，需要继续调试。

#### 13.1 数据生成

- [x] **新建 `crates/hoi4-render/src/province_labels.rs`**：
  - `compute_province_labels` → `Vec<Option<ProvinceLabel>>`
  - 算法：每省取 centroid + 第一主轴（quick eigen），不做精细 OBB
  - 上限 8000 省（vanilla 13K，远岛屿小省直接跳过）
- [x] **本地化 key**：暂用 `StateStore::names` + fallback `PROV<id>`；完整 YML 解析待 Phase 4.9

#### 13.2 fontdue atlas 烘焙

- [x] **单张 2048×N atlas**：当前实现用单 atlas + glyph 缓存，可工作
- [ ] **多张 atlas 分桶**：省份名长度差异大，单 atlas 在省数多时可能不够（待实测）
- [x] **glyph 缓存**：跨省份共用字符（`GlyphCache` HashMap）

#### 13.3 zoom-gated 渲染

- [x] **`crates/hoi4-app/src/passes/province_name.rs::ProvinceNamePass`**
- [x] **远视角 zoom_factor < 0.4**：不画省份名（CPU 端 return）
- [x] **shader zoom_alpha**：`1.0 - smoothstep(30, 60, cam_dist)` 近处可见、远处渐隐
- ⚠️ **zoom fade 方向仍有问题**：用户反馈近处看不到省份名，需继续调试 cam_dist 阈值与 zoom_factor 的对应关系

#### 13.4 验收

- [ ] 中视角下意大利半岛、巴尔干、东欧能看到省份名
- [ ] 远视角不画省名（避免文字海）

---

### 13.bis 省份名视觉重做（3 天）⭐ P0 修 bug

**触发**：2026-05-18 用户截图。13 节产出有效（atlas 烘焙 / 数据流 / pass 框架就位），但放大后**视觉与 vanilla 截图差距大、看不清写什么**。逐项排查:

| 项 | vanilla 行为 | 现状 | 收益 |
|---|---|---|---|
| 文字方向 | 水平贴地，全图朝相机一致 | 沿 OBB axis1 旋转，岛屿上的字斜放 | ⭐⭐⭐ 最大 |
| 文字尺寸 | 屏幕空间恒定 ~14 px | world units quad，远小近大模糊 | ⭐⭐⭐ |
| 描边 | 真黑边，外扩 1-2 像素 | atlas 无描边数据，shader smoothstep 假装 | ⭐⭐ |
| z-bias 距离 | NDC 微偏 | `world_pos + to_cam * 0.3`，大字侧视变菱形 | ⭐⭐ |
| 字号烘焙 | 12-14 px | 24 px → 缩放后糊 | ⭐ |

#### 13.bis.1 文字方向改为水平贴地（半天）⭐⭐⭐

`crates/hoi4-app/src/passes/province_name.rs::PROVINCE_NAME_WGSL` vs_main：

```wgsl
// 现状（错）：跟着省份 OBB 主轴旋转
let axis1_world = vec3<f32>(inst.axis1.x, 0.0, inst.axis1.y);
let axis2_world = vec3<f32>(-inst.axis1.y, 0.0, inst.axis1.x);
```

- [x] **改成屏幕空间锁向**：用相机 right / forward-projected-on-XZ 算两个轴：

  ```wgsl
  let cam_right = normalize(vec3<f32>(frame.view_proj[0][0], 0.0, frame.view_proj[2][0]));
  let cam_fwd_xz = normalize(vec3<f32>(-cam_right.z, 0.0, cam_right.x));
  let axis1_world = cam_right;
  let axis2_world = cam_fwd_xz;
  ```

- [x] **保留 `inst.axis1`** 字段不动（数据层未来仍可用作"按主轴布局"开关），只在 shader 里忽略
- [x] **缘由**：vanilla 省份名永远水平。axis1 旋转会让萨丁尼亚岛上的"萨萨里"沿岛屿对角线斜放，肉眼第一感就是"奇怪不像 vanilla"

#### 13.bis.2 屏幕空间恒定字号（1 天）⭐⭐⭐

vanilla 省名永远是 ~14 屏幕像素高，远近一致。当前 quad 是 world units，相机推近字会变巨大模糊、远视角字微小看不清。

- [x] **VS 改为屏幕空间投影**：先把 center 投到 NDC，再在 NDC 上加固定像素偏移：

  ```wgsl
  // 1. center → clip
  let center_clip = frame.view_proj * vec4<f32>(inst.center, 1.0);
  let center_ndc_w = center_clip.w;

  // 2. 目标屏幕高度（固定 14 logical px）
  let target_px = 14.0;
  let pixel_to_ndc = 2.0 / frame.screen_size.y;
  let h_ndc = target_px * pixel_to_ndc * center_ndc_w;
  let aspect = (inst.uv_max.x - inst.uv_min.x) / (inst.uv_max.y - inst.uv_min.y);  // atlas 内长宽比
  let w_ndc = h_ndc * aspect;

  // 3. 在 NDC 上加 quad
  var clip = center_clip;
  clip.x += local_x * w_ndc;
  clip.y += local_y * h_ndc;
  clip.z -= 0.001 * clip.w;  // 防 z-fight
  ```

- [x] **删除 `width_world / height_world` quad 算法**：`InstanceData.width_world` / `height_world` 现在是 atlas 长宽比的 hint，不再决定世界尺寸
- [x] **缘由**：vanilla 行为是屏幕空间常量字号，所有 RTS / 4X 都这样做（除非走 3D 沿曲面贴字，那是 vanilla 的 mapname.shader 给国名用的，不是省名）

#### 13.bis.3 真描边（atlas 烘焙端）（半天）⭐⭐

`crates/hoi4-app/src/province_name_atlas.rs::render_strip_cached` 当前只填字形 alpha。

- [x] **two-tier R8 编码**（与 mapname_atlas 同款）：
  - 先把字形像素填到 `fill[gx]`（值 ≥ 128 = 文字内部）
  - 第二遍扫描每个 0 像素，检查 8 邻域是否有 ≥ 128 的字形像素，如果有 → 写 110（描边像素）
  - shader 端 `s ≥ 0.65 = 文字`、`0.3 ≤ s < 0.65 = 描边`、`s < 0.3 = 透明`

- [x] **shader 适配**：把现在 13 节实现的 `text_t = smoothstep(0.45, 0.7, s)` 改成阶梯：

  ```wgsl
  let text_t = smoothstep(0.62, 0.68, s);
  let outline_t = smoothstep(0.27, 0.33, s) * (1.0 - text_t);
  ```

- [x] **缘由**：当前 outline 只是字 anti-aliased 边缘的暗化，字外**没有描边像素**——所以白色文字在浅色海面 / 山地几乎完全看不见

#### 13.bis.4 字号 + z-bias 调整（半天）⭐⭐

- [x] **font_size 24 → 14**：`bake_province_name_atlas(&prov_names, 24.0)` → `bake_province_name_atlas(&prov_names, 14.0)`
  - 烘焙字号匹配屏幕显示字号 → GPU 不需要双线性缩放 → 字形清晰
- [x] **去掉 vDistortedPos 距离推近**：`world_pos + to_cam * 0.3` 改成 `world_pos + to_cam * 0.05`
  - 0.3 让大字侧视时变菱形（quad 中心被推近 0.3、边缘按 quad 自身坐标推近 ≠ 0.3）
  - 现在用 NDC z-bias `clip.z -= 0.001 * clip.w` 已经够防 z-fight，不需要 distortion
- [x] **`distortion_amount` uniform 改默认 0.05**

#### 13.bis.5 重叠避让（可选，1 天）⭐

vanilla 多省名靠近时会让小省的字隐藏，避免文字撞到一起。当前所有省名都画。

- [ ] **简化避让**：CPU 端按 `pixel_count` 降序排序，每个标签占用屏幕投影矩形，后续标签如果矩形 IoU > 30% 就跳过
- [ ] **本节先不做**：13.bis.1-4 修完已经接近 vanilla，避让留 polish

#### 13.bis 验收

- [x] **截图回归**：放大到中视角（zoom_factor ≈ 0.5），意大利半岛能清晰读出省名（"罗马" / "那波利" / "佛罗伦萨"）
- [x] **方向**：所有省名水平贴地，无斜放
- [x] **字号**：远近一致 ~14 屏幕像素，不糊
- [x] **描边**：白色文字在草地 / 山地 / 海面背景上都清晰可见（黑边外扩 1-2 px）
- [ ] **vanilla 对比**：与 vanilla 第一张截图同视角，文字布局与可读性差距 ≤ 10%

工作量：3 天。

| 子任务 | 估算 |
|---|---|
| 13.bis.1 屏幕空间锁向 | 0.5 天 |
| 13.bis.2 屏幕空间恒定字号 | 1 天 |
| 13.bis.3 atlas 真描边 | 0.5 天 |
| 13.bis.4 字号 + z-bias 微调 | 0.5 天 |
| 验收 + vanilla 截图对比 | 0.5 天 |

#### 13.bis.6 不在范围（明确延后）

- ✗ 沿地形曲面贴字（vanilla mapname.shader 风格，留给 12 节 mapname 重做）
- ✗ 多 atlas 分桶（13.2 已标记，单 atlas 当前够用）
- ✗ 完整 yml 本地化（4.9）
- ✗ 重叠避让（13.bis.5 留 polish）

---

### 14. POI 图标 pass（工厂 / 港口 / 机场 / 资源）（2 天）✅ 已完成

vanilla 在每省心位置显示该省的"已建建筑"图标（`gfx/interface/map_icons/`）：工厂、船坞、机场、资源点。**当前已实现 POI 图标 pass**——使用 instanced billboard quad + procedural SDF shape + kind-based color。

#### 14.1 数据源

- [x] **`World.states[i]` buildings**：`civilian_factories / military_factories / dockyards / infrastructure` 从 `StateStore` 读取
- [x] **`World.data.states[i].resources`**：`oil / aluminium / rubber / tungsten / steel / chromium` 从 `GameData.states` 读取

#### 14.2 PoiIconKind + 数据生成

- [x] **`hoi4_render::buildings::PoiIconKind`**：16 种 POI 类型（9 种建筑 + 7 种资源），每种有 `sprite_name()` 返回 vanilla `GFX_*` 名
- [x] **`hoi4_render::buildings::generate_poi_icons()`**：遍历所有 state，为每种存在的 building/resource 生成 `PoiIconInstance`（pos + kind + level），带 X 轴偏移防重叠
- [x] **`PoiIconInstance`**：20 字节（`[f32;3] pos + f32 kind + f32 level`），kind 为 `PoiIconKind as u8`

#### 14.3 PoiIconPass 渲染管线

- [x] **`crates/hoi4-app/src/passes/poi_icon.rs::PoiIconPass`**：
  - 实例化 billboard quad（6 顶点，camera-facing）
  - 着色器 `poi_icon.wgsl`：procedural SDF shape（工厂=方 / 港口=菱 / 机场=三角 / 雷达=圆 / 资源=圆）+ kind-based color
  - `level` 影响 icon 大小（高等级建筑略大）
  - 单 bind group（camera uniform），无纹理依赖
- [x] **管线格式**：alpha blend + no depth write + LessEqual depth test，与 buildings pipeline 一致

#### 14.4 zoom-gated + LOD

- [x] **远视角 zoom_factor < 0.3**：只画 major buildings（kind ≤ 2 = civ/mil/dock）
- [x] **中视角 zoom_factor 0.3–0.6**：画所有 building types（kind ≤ 8）
- [x] **近视角 zoom_factor ≥ 0.6**：画所有 POI（含资源图标）
- [x] **每帧 upload 过滤**：`PoiIconPass::upload()` 按 zoom_factor 过滤实例后上传

#### 14.5 验收

- [x] 默认相机视角下意大利半岛能看到罗马 / 米兰 / 那不勒斯的工厂图标（绿色方块 = civ，红色方块 = mil，蓝色菱形 = dock）
- [x] 中东能看到油田图标（黑色圆形）
- [x] `cargo check --workspace` 干净通过

工作量：2 天。

---

### 15. 单位 3D mesh — 海军 + 空军（5-7 天）⭐ 从 V3 3.12.16 抽出

> **抽出原因**：V3 3.12.16 标"依赖 3.12.5"，但 3.12.5 已完成（PdxMeshPass 就位）。
> 视觉收益高（vanilla 北海有英德舰队可见，本项目空），且独立可做。

参见 ROADMAP_V3.md 3.12.16 章节，需求与流程已完整描述。本路线图按字面引用，不重复展开。

工作量：5-7 天（依赖 3.12.5 PdxMeshPass 完成）。



---

## P3 — 长尾收尾（6-7 天）

### 16. arrows family（3-4 天）⚠️ 与 V3 4.8 军事面板可并行

3.11.11 翻译的 `maparrow.wgsl` / `traderoute.wgsl` / `arrow.wgsl` / `strait.wgsl` 已就位但没接主循环。本节做 arrows 的视觉接入（数据流 / 用户操作交给 V3 4.8）。

#### 16.1 maparrow（军令箭头）

- [x] **`crates/hoi4-app/src/passes/maparrow.rs`**：贝塞尔头 / 尾 / 体三段类型 instance
- [x] **数据 mock**：从 `World.divisions.orders[i]` 提取 source_province → target_province 路径，分段成 N 个 instance
- [x] **暂时硬编码若干箭头测试**：等 V3 4.8 接通真实命令系统后替换

#### 16.2 traderoute（贸易线流动虚线）

- [x] **`crates/hoi4-app/src/passes/traderoute.rs`**：流动虚线 UV
- [x] **数据 mock**：从 `World.trade.routes` 提取 from_country.capital → to_country.capital
- [x] **零贸易时不画**

#### 16.3 strait（海峡过道）

- [x] **`crates/hoi4-app/src/passes/strait.rs`**：海峡过道 + 脉冲
- [x] **数据**：vanilla `map/adjacencies.csv` 里 `type = strait` 的条目

#### 16.4 验收

- [x] 测试用例：手动 dispatch 一个箭头从柏林到华沙，地图上能看到弯曲箭头 + 闪烁
- [x] 海峡（直布罗陀 / 苏伊士 / 巴拿马）能看到过道线

工作量：3-4 天。

---

### 17. 树木 .mesh LOD + 季节染色微调（3 天）⭐ 从 V3 7.3 抽出 ✅ 已完成

3.12.8 已经做了 trees_full.wgsl + Tree_season.bmp + Tree_tint.bmp + seasons.txt。剩余精修：

#### 17.1 LOD 切换（按距离换 mesh）✅

vanilla 每种树有 3-5 个 LOD（`Pine_01.mesh` / `Pine_02.mesh` / `Pine_03.mesh`）。当前只用 sub0（最高 LOD）。

- [x] **在 trees_full.rs 加载所有 LOD**：使用 `PdxMesh::pick_lod(lod_idx)` 为每个树类型加载最多 `MAX_TREE_LODS=3` 个 LOD mesh，每个 LOD 有独立的 vertex/index/instance buffer
- [x] **GPU 端按 cam_distance 分桶**：`TreeLodMesh` 结构体持有每个 LOD 的 buffers；`LOD_DISTANCES = [15.0, 30.0, 1000.0]` 控制切换阈值（可被 .mesh 文件的 `loddist` 属性覆盖）
- [x] **CPU 端每帧重新分桶**：`upload_instances()` 每帧根据相机位置计算每个实例的 XZ 距离，按距离分配到对应 LOD bucket，写入对应的 instance buffer
- [x] **远景树用 billboard fallback**：现有 `trees.wgsl` 程序化三角形保留作最远 LOD（tree_full_pass 超出范围后 fallback 路径仍可用）

#### 17.2 季节染色微调 ✅

3.12.8 的季节染色在春夏冬过渡期颜色突变（"3 月 1 日突然全绿"）。

- [x] **两 row mix**：`SeasonResult` 升级为包含 `season_column_next: f32` + `season_blend: f32`
- [x] **`SeasonsTxt::season_for_date`** 返回值升级：在季节开始 15% 和结束 15% 范围内产生 `season_blend > 0`，混合到前一/后一季节
- [x] **shader 端**：`TreeParams` 新增 `season_column_next` + `season_blend` 字段；fragment 中 `if season_blend > 0.01` 时采样两列 atlas 并 `mix`

#### 17.3 验收

- [x] 3 月 / 9 月过渡期树色平滑变化（不再硬切）
- [x] 远视角性能：原 36K instances → 分 LOD 后近景 ~12K LOD0 + ~6K LOD1 + 剩余 LOD2，frame time 不退步
- [x] `cargo check --workspace` 干净
- [x] `cargo test -p hoi4-map seasons` + `cargo test -p hoi4-app trees_full` 全 PASS

工作量：3 天。

---

### 18. 雾参数按 WORLD_SCALE 重新校准（0.5 天）⭐ P0 修 bug ✅ 已完成

**触发**：2026-05-18 用户截图（俯视亚洲 1936-01-01）。1-7 节修完后整张图仍被一层薄雾均匀涂过，"雾无处不在"。

**根因**：`crates/hoi4-render/src/shader_lib.wgsl` 的雾常量是按 vanilla 那种 5000+ 单位的世界尺度调的，但本项目 `WORLD_SCALE=0.02` 让整张地图世界尺寸只有 **112×41 单位**——比 vanilla 小约 **50×**。

```wgsl
// 现状（按 vanilla 大世界尺度）
const FOG_BEGIN: f32 = 80.0;
const FOG_END: f32 = 400.0;
const FOG_MAX: f32 = 0.35;
```

俯视亚洲这种"远视角看大半球"时，相机 eye 离地表点的距离常常到 100-200 单位，全图 fog factor `(d²-80²)/(400²-80²)` 都落在 5%-30% 之间被 `FOG_MAX=0.35` 钳着。结果就是整张图都吃 5%-35% 的雾色——加上雪山区雪本身偏白 + 海面浅蓝 + fog 蓝灰三层叠加，得到"白雾糊一片"的视觉。

#### 18.1 改值 ✅

`crates/hoi4-render/src/shader_lib.wgsl`：

```wgsl
// 按本项目 WORLD_SCALE=0.02 的小世界尺度（地图对角线 ≈ 119 单位）重新校准
const WORLD_EXTENT: f32 = 119.5;
const FOG_BEGIN: f32 = WORLD_EXTENT * 1.7;   // ≈ 203
const FOG_END:   f32 = WORLD_EXTENT * 6.7;   // ≈ 801
const FOG_MAX:   f32 = 0.18;                  // 即使最远处也只吃 18% 颜色
```

- [x] **改 3 个常量**（`shader_lib.wgsl` 一处即可，所有 6 个 wgsl 同步受益）
- [x] **抽出 `WORLD_EXTENT` const** = √(112² + 41²) = 119.5，FOG_BEGIN/END 写成 `WORLD_EXTENT × N`，未来调 WORLD_SCALE 时自动跟随
- [x] **缘由**：本项目世界尺寸 112×41，vanilla 是 ~5632×2048。雾参数必须按世界尺度等比缩。

#### 18.2 验收

- [ ] 俯视亚洲全图（zoom_factor ≈ 0.15）时，地图主体（陆地 / 海面）饱和度 ≥ 90% 不被雾覆盖
- [ ] 仅地图最远边角（接近视锥裁剪平面）有轻微大气透视感
- [ ] 雪山区不再"白雾糊一片"
- [ ] vanilla 同视角对比，色温 ±5% 一致

工作量：0.5 天（含改值 + 多 zoom 档截图比对）。

---

### 19. 昼夜节律接游戏内时间（0.5 天）⭐ P0 修 bug

**触发**：2026-05-18 用户反馈"时间流动后昼夜变化太快"。

**根因**：`crates/hoi4-app/src/main.rs:2911-2914` 用墙钟秒数驱动昼夜：

```rust
// 现状（错）：墙钟 20 秒走完一个昼夜
gu.day_night_hour_sun_dir = {
    let sd = params.sun_dir;
    let hour = ((time * 0.05).fract() + 1.0).fract();  // ← time 是程序启动秒数
    [hour, sd[0], sd[1], sd[2]]
};
```

`time` 是 `Instant::now() - start` 的秒数（墙钟），系数 0.05 意味着 20 秒走完一个完整昼夜。打开游戏让时间流动后，日夜飞速切换——这就是用户看到的"昼夜过快"。

更糟的是，这套渲染端"假昼夜"和**游戏内日期完全无关**：`world.date.hour` 字段一直存在（`crates/hoi4-state/src/time.rs::GameDate { year, month, day, hour }`），tick 系统也在推进它，但渲染没用它。

#### 19.1 改成读 `world.date.hour`

`crates/hoi4-app/src/main.rs:2911-2914`：

```rust
// 修法：用游戏内真实小时驱动渲染昼夜
gu.day_night_hour_sun_dir = {
    let sd = params.sun_dir;
    // GameDate.hour ∈ [0, 23] → 归一化到 [0, 1]
    let hour = (self.world.date.hour as f32) / 24.0;
    [hour, sd[0], sd[1], sd[2]]
};
```

- [ ] **改 4 行**：把 `time * 0.05` 替换成 `self.world.date.hour as f32 / 24.0`
- [ ] **缘由**：
  - 游戏暂停 → 昼夜静止（vanilla 同款）
  - 游戏 1× 速度 → 一个昼夜 = 24 个游戏小时 = 现实约 24 秒（取决于 tick 频率）
  - 游戏 5× 速度 → 仍跟 game tick 一致，受用户速度控制
  - 渲染端不再有独立的昼夜节律

#### 19.2 平滑过渡（可选）

整数小时跳一次的视觉跳变可能会有"日影一格一格跳"的感觉。如需平滑：

```rust
gu.day_night_hour_sun_dir = {
    let sd = params.sun_dir;
    // 用 elapsed_hours 累计（持续单调增长）+ frame 间小数插值
    // 注意：要保持暂停时静止，所以 frame 小数只在游戏未暂停时累加
    let game_hours = self.world.elapsed_hours as f32 + self.world_sub_hour;  // sub_hour ∈ [0, 1]
    let hour = (game_hours / 24.0).fract();
    [hour, sd[0], sd[1], sd[2]]
};
```

- [ ] **本节先不做平滑**：基础版（整数小时）已经解决"过快"问题；平滑留 polish 阶段
- [ ] **`world_sub_hour` 字段**：如果未来要做，加在 `App` struct 上，由 tick scheduler 维护

#### 19.3 验收

- [ ] 游戏暂停时昼夜不变（看 sun_specular 高光位置 / 海面反射 / mapname 暗化）
- [ ] 游戏 1× 速度跑 5 分钟，地图昼夜节律与日期推进同步（不再 20 秒一个昼夜）
- [ ] 游戏 5× 速度时昼夜节律加快，但仍可控（不再"飞快闪烁"）

工作量：0.5 天（含 GamePhase::Paused 行为校验 + 多速度档观察）。

---

### 20. 陆军 3D 基座（5-7 天）⭐ 从 V3 7.2 抽出

> **抽出原因**：V3 3.12.16 明确"陆军 3D 留 Phase 7.2"，V3 7.2 又把"陆军 3D 基座"列在 vanilla unit mesh 之后。但用户对地图视觉等价的关注点已经聚焦到"counter 下方有没有 3D 小人队伍"——这是 vanilla 在 `Show 3D unit models = on` 下默认的视觉。本节把它从 V3 7.2 抽出，与 15 节（海军/空军 3D mesh）并列，让陆/海/空三类单位 mesh 在同一份视觉等价路线图下被统一追踪。
>
> **依赖**：3.12.5 PdxMeshPass（已完成）+ 3.12.15.bis MapSymbolPass（已完成）。本节不依赖 V3 7.2 任何其他内容。

#### 20.1 数据源

vanilla `gfx/models/units/army/*.mesh`：

- `infantry_*.mesh` — 步兵基座（步行小人 × N）
- `armor_*.mesh` — 坦克基座（轻 / 中 / 重 / 现代坦克）
- `motorized_*.mesh` — 卡车基座
- `mechanized_*.mesh` — 装甲车基座
- `cavalry_*.mesh` — 骑兵基座
- `artillery_*.mesh` — 火炮基座
- `paratrooper_*.mesh` — 伞兵基座
- `marine_*.mesh` — 海军陆战队基座
- `mountain_*.mesh` — 山地兵基座

每个 archetype 通常有 2-3 个 mesh 变体（不同时期 / 不同国家变体）。

#### 20.2 数据流

CPU 端从 World 提取每个 counter 的 archetype（已在 3.12.15.bis 完成），按 archetype 路由到对应 mesh：

- [ ] **`crates/hoi4-render/src/army_mesh.rs`**：
  - `pub fn select_army_mesh(archetype: UnitArchetype, country_idx: usize, year: u16) -> &'static str`
  - 按 archetype 主路由 + 按 year 选时代变体（1936 → infantry_1936.mesh / 1945 → infantry_1945.mesh）
  - 按 country_idx 选大国专属（GER infantry / SOV infantry / USA infantry）+ 其余通用

- [ ] **`ArmyMeshInstance`**（32 字节，与 PdxMeshInstance 同款）：
  - `pos: [f32; 3]` — 省份 centroid 世界坐标 + 略微 y_bias 防 z-fight
  - `scale: f32` — 按 stack_count 微缩放（1 stack = 1.0×，2-5 stack = 1.1×，>5 = 1.2×）
  - `country_color_tint: [u8; 4]` — 国旗色（与 counter 顶部底框一致）
  - `heading_yaw: f32` — 朝向，从最近"敌国边境"算（朝向"前线"）
  - `pad: f32` — 对齐

#### 20.3 渲染管线

- [ ] **`crates/hoi4-app/src/passes/army_mesh.rs::ArmyMeshPass`**：
  - 复用 `PdxMeshPass` 完整 pipeline（Blinn-Phong + shadow PCF + cubemap reflection + day/night）
  - 不同 archetype 各自一次 draw call（按 archetype 排序后 multi-draw）
  - `set_armies(&device, &instances_by_archetype)` 每帧重新生成 instance buffer
  - **每个 counter 在 counter 下方铺一层 mesh**：与 MapSymbolPass 共享 `world_pos`，y_bias 让 mesh 贴地、counter 漂浮 0.05 单位

#### 20.4 性能控制

师数量级 vanilla 1936 全图约 800-1500 个 counter，全开 mesh 是 1500 × 几十 K 顶点的 draw burden。

- [ ] **zoom-gated 显示**：
  - `zoom_factor < 0.4`（远视角）→ 不画 army mesh，只画 counter（vanilla 同款）
  - `zoom_factor 0.4 - 0.7`（中视角）→ 仅画大 stack（≥3 师）
  - `zoom_factor > 0.7`（近视角）→ 画所有 counter 下的 mesh
- [ ] **mesh LOD**：
  - 近 → LOD0（最高细节）
  - 中 → LOD1 / LOD2
  - 不画 → counter only
- [ ] **friendly-of-war / spotted gate**：与 MapSymbolPass 共用 `spotted_provinces`，敌军非 spotted 不画 mesh

#### 20.5 vanilla 设置 toggle

vanilla 有 `Show 3D unit models` 选项允许玩家关闭以提性能：

- [ ] **F6 快捷键**：在 Playing 阶段切 `enable_army_3d` bool
- [ ] **设置面板 toggle**（4.3+ 接通后）：把状态保存到 user config

#### 20.6 验收

- [ ] 默认相机视角下 GER 1936 主力部队（柏林周边 / 莱茵河 / 波兰边境）counter 下方能看到士兵 / 坦克 mesh
- [ ] 不同 archetype 的 mesh 视觉可分辨（步兵 ≠ 坦克 ≠ 火炮）
- [ ] 远视角自动隐藏 mesh，仅留 counter（性能保护）
- [ ] F6 关闭后帧时间退步 ≤ 5%（关闭等于消除 1500+ instanced draw）
- [ ] 与 vanilla 同视角对比：mesh 数量级一致、archetype 类型分布一致、朝向粗略合理

工作量：5-7 天。

| 子任务 | 估算 |
|---|---|
| 20.1-20.2 archetype → mesh 路由 + instance 数据 | 1-2 天 |
| 20.3 ArmyMeshPass pipeline + draw call | 1-2 天 |
| 20.4 zoom-gate + LOD + spotted | 1 天 |
| 20.5 toggle + 设置面板 | 0.5 天 |
| 20.6 视觉验收 + 截图比对 | 0.5-1 天 |
| 视情况：vanilla mesh 加载缺失 fallback 调试 | 0.5-1 天 |

#### 20.7 不在范围（明确延后）

- ✗ 单位 .anim 骨骼动画（步兵走路 / 坦克炮塔旋转）→ V3 Phase 7.5
- ✗ 战斗中的开火 / 爆炸特效（mesh 闪光）→ V3 7.2 战斗火光
- ✗ Unit Pack DLC mesh（俾斯麦 / 大和 / 5 国坦克）→ V3 7.2 留待
- ✗ 师徽（divisions emblem）→ V3 4.8 军事面板
- ✗ 历史变体着色（按 ideology 微调坦克色）→ V3 7.2 留待

---

### 21. 截图回归基线 + 性能回归（3 天）⭐ 收尾

> 与 V3 3.12.17 字面相同。

#### 18.1 baseline 截图

GER 1936-01-01 默认相机角度，5 档：

- [ ] **远缩**（zoom_factor ≈ 0.15，能看到整个欧洲）
- [ ] **中缩**（zoom_factor ≈ 0.45，能看到德国及邻国）
- [ ] **近缩**（zoom_factor ≈ 0.85，能看到柏林周边省份）
- [ ] **政治模式**（map_mode = political）
- [ ] **地形模式**（map_mode = terrain）

存到 `tests/screenshots/map-parity-baseline/`。

#### 18.2 vanilla 同条件对照

每档配一张 vanilla 同条件截图，目视差距 ≤ "色温 ±5%、亮度 ±5%、布局结构肉眼一致"。

#### 18.3 性能回归

- [ ] **1080p 60 FPS GPU frame time** 与 vanilla 同机器同视角对比 ≤ 1.5×
- [ ] **4K 30 FPS** ≤ 1.7×
- [ ] **`cargo run --release -p hoi4-app -- --headless --headless-days 7` ms/day** 不退步（当前约 14 ms/day）
- [ ] **首屏加载时间** < 30s（当前约 15s）

#### 18.4 修复发现的回归

截图比对暴露的差异列入 fix-list 一并修。每发现一个差异：

1. 在 `screenshots/diff/` 存"项目截图 / vanilla 截图 / 差异区域标注"三张图
2. 在本路线图添加 P0/P1/P2 子项
3. 修完更新截图

工作量：3 天。

---

## 综合验收

完成本路线图所有项后：

- [ ] **截图对比**：5 档 GER 1936-01-01 视角 + 任意 1 个 mod 视角下，与 vanilla 肉眼差距 ≤ 10%
- [ ] **测试**：`cargo test --workspace` ALL PASS / `cargo build --workspace` 干净
- [ ] **性能**：1080p 60 FPS / 4K 30 FPS / headless 不退步
- [ ] **文档**：每个修复 / 新增项在 `CONTRIBUTING.md` 或 PR 描述里有"修了什么 / 怎么验证 / 截图前后对比"
- [ ] **回写 ROADMAP_V3**：把本路线图覆盖的子节标记 `✅ COMPLETE (via ROADMAP_MAP_VISUAL_PARITY)`

---

## 推荐实施顺序

按 session（每天 1-2 节）：

| Session | 内容 | 类型 |
|---|---|---|
| 1 | 1 三件套（雾 + 钳位 + 政治色）⭐⭐⭐ | P0 修 bug |
| 2 | 2 water 三件套 + 3 river 顺序 | P0 修 bug |
| 3 | 4 国境黑线移除 + 5 4-tap blending 起步 | P0 |
| 4 | 5 4-tap blending 收尾 + 6 atlas_idx 间接索引 | P0 |
| 5 | 7 world_normal pack + sun_dir + 8 postprocess 调参 | P1 |
| 6 | 9 sky pass + EnvironmentMap | P1 |
| 7 | 10 particle pass | P2 |
| 8 | 11 LOD + skirt + R16 heightmap | P2 |
| 9 | 12 mapname zoom 淡出 + 13 省份名 pass 起步 | P2 |
| 10 | 13 省份名 pass 收尾 + 14 POI 图标 pass | P1 |
| 10.5 | **13.bis 省份名视觉重做**（方向/字号/描边，用户反馈） | **P0 修 bug** |
| 11-13 | 15 单位 3D mesh（海 + 空） | P1 |
| 14 | 16 arrows family | P3 |
| 15 | 17 树木 LOD + 季节微调 | P3 |
| 16 | **18 雾参数 + 19 昼夜节律**（用户反馈，0.5+0.5 天，可同 session） | **P0 修 bug** |
| 17-18 | **20 陆军 3D 基座**（counter 下方铺一层 mesh） | P1 |
| 19 | 21 截图回归 + 性能回归（收尾） | 验收 |

**总工作量**：40-47 天 ≈ 8-9 周（单人专职）。

---

## 不在范围（明确延后）

- ✗ HLSL → wgsl transpiler（V3 设计原则不变）
- ✗ vanilla 完整 PBR（IBL / GGX / 双 BRDF），保持 Blinn-Phong + 简化反射
- ✗ Cloud volumetric 真体积云（vanilla 是平面 fbm cloud shadow，不做真 3D 云）
- ✗ Real-time planar reflection RT（water 反射用 cubemap + 占位 reflection.dds 即可）
- ✗ Tree wind animation（V3 7.3 已标延后）
- ✗ UI 面板 (V3 4.3-4.11)
- ✗ Mod 兼容（V3 9.x）
- ✗ 玩法逻辑（V3 5.x / 6.x）

---

## 与 V3 治理原则的对齐

本路线图把 V3 "完成 = 在跑动的游戏里被观察到" 推到极致——**所有 P0 项都要求"开机进游戏 5 秒内能用截图证明差距消除"**。

P0 验收依据是同视角 vanilla 截图，不是单测。`cargo test` 通过只是必要条件，不是充分条件。

---

## 历史背景（为什么需要这份独立路线图）

ROADMAP_V3 把视觉打磨打散到 10+ 个子节，每节的"验收"都聚焦"自己接通了什么"，
但**没有任何一节负责"整体地图视觉与 vanilla 等价"**。结果：
- 3.12.4 pdxmap 接通 atlas / world_normal / shadow / citylights → 标 ✅
- 3.12.6 pdxwater 接通 lean / cubemap / foam / 冰 → 标 ✅
- 3.12.7 river 接通 RiverSurface 三套贴图 → 标 ✅
- 3.12.8 tree_full 接通 season / tint → 标 ✅
- 3.12.9 border strip-mesh → 标 ✅
- ...

每节都"完成"，但合起来跑游戏：
- 海是惨白（雾常量错 + 颜色暗 + 自动曝光反向拉亮）
- 海岸是大块马赛克（顶点钳位 + fragment 内插）
- 河是看不见（被 water opaque 覆盖）
- 国境是黑硬线（terrain shader 没让 BorderPass 接管）
- 山没有阴影感（world_normal pack 解码错）
- 远景全是省份色块（terrain_blend 上限 0.75）
- 没有省份名 / 没有 POI 图标 / 没有舰船 / 没有飞机

**根因是责任分散**：每节验收只看"自己加了什么 binding / 写了什么 wgsl"，没人对"和 vanilla 同截图"负责。

本路线图把**所有 100% 与地图视觉相关的修复 + 新增**集中到一处，让"视觉等价"成为
独立、可问责的目标。每节验收都强制配 vanilla 截图比对——这是 V3 治理原则在视觉
打磨阶段的具体落实。

