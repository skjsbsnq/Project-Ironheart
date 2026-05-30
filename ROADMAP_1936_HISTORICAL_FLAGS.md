# Roadmap: 1936 Historical Flags Rework

> **STATUS: PLANNED**
>
> 本路线图记录国旗系统从 HOI4 vanilla `gfx/flags/<TAG>_<ideology>.tga` 迁移到项目自带真实 1936 国旗资产的中等重构方案。目标是切断对 HOI4 原版国旗的运行时依赖，同时保留现有 wgpu 渲染管线和最小可控的实现范围。

---

## 背景

当前国旗系统集中在 `crates/hoi4-app/src/flag_bank.rs`。它按 `(tag, ideology)` 查找 HOI4 原版资产：

```text
gfx/flags/<TAG>_<ideology>.tga
gfx/flags/<TAG>.tga
gfx/flags/medium/<TAG>_<ideology>.tga
gfx/flags/medium/<TAG>.tga
```

这套机制适合复用 HOI4 原版资源，但不适合真实 1936 国旗：

- 真实国旗应由国家、日期、政权状态决定，而不是由 `fascism / democratic / communism / neutrality` 决定。
- 当前 UI 强制使用 HOI4 root flag 比例 `82:52`，会拉伸英国、美国、日本等真实比例国旗。
- `FlagBank` 同时承担路径决策、HOI4 命名规则、TGA 解析、GPU 缓存和 fallback，职责过重。
- 后续政治事件、内战、自治领、傀儡和历史符号替代都需要比 `(tag, ideology)` 更明确的数据模型。

---

## 目标

- [ ] 项目自带真实 1936 国旗资源，不再运行时读取 HOI4 原版国旗。
- [ ] 新增 `FlagCatalog` 内容层，负责 `tag/date/variant` 到国旗资产路径的解析。
- [ ] 重构 `FlagBank` 为按 asset path 缓存的 GPU 纹理银行，不再理解 ideology。
- [ ] 国家选择界面使用真实 1936 国旗，并按图片真实比例绘制。
- [ ] 为历史符号敏感内容预留 safe variant，不阻塞第一版交付。
- [ ] 保留现有渲染 pass、bind group layout、TGA 解码路径，避免一次性改动过大。

---

## 非目标

- [ ] 第一版不做全世界国家国旗全集，只覆盖当前国家选择列表 8 国。
- [ ] 第一版不引入 PNG 解码，继续使用当前 `TgaImage` 管线。
- [ ] 第一版不改政治系统、不改 `ruling_party` 存档格式。
- [ ] 第一版不做 HOI4 mod 兼容国旗覆盖。
- [ ] 第一版不替换所有 UI 面板，只先接国家选择界面。

---

## 当前入口

| 文件 | 当前职责 | 重构处理 |
|---|---|---|
| `crates/hoi4-app/src/flag_bank.rs` | 读取 vanilla flag TGA、上传 GPU、缓存 bind group | 移除 HOI4 路径规则，改为按 asset path 加载 |
| `crates/hoi4-app/src/main.rs` | 初始化 `FlagBank`，国家选择界面调用 `get_or_load(tag, ideology)` | 初始化 `FlagCatalog`，调用 catalog resolve 后把 path 交给 `FlagBank` |
| `crates/hoi4-app/src/menu_pass.rs` | 计算国旗框，固定 `82:52` 比例 | 改成固定外框 + 按图片比例 fit 内部 rect |
| `crates/hoi4-assets/src/tga.rs` | TGA 解码 | 第一版沿用 |
| `crates/hoi4-content/content/history_1936/countries/*.ron` | 1936 国家经济/法律数据 | 不直接塞国旗字段，国旗单独 manifest 管理 |

---

## 新内容结构

建议新增：

```text
crates/hoi4-content/content/flags/
  flags_1936.ron
  flag_sources.ron

crates/hoi4-render/assets/flags/1936/
  GER.tga
  ITA.tga
  JAP.tga
  ENG.tga
  FRA.tga
  USA.tga
  SOV.tga
  CHI.tga
  fallback.tga
```

如后续希望把自带资产放在 workspace 根，也可改为：

```text
assets/flags/1936/
```

但当前项目已有 `crates/hoi4-render/assets/counter_icons/`，所以第一版优先放在 `crates/hoi4-render/assets/flags/1936/`。

---

## Manifest 草案

第一版静态 manifest：

```ron
[
    (tag: "GER", file: "flags/1936/GER.tga", safe_file: Some("flags/1936/GER_safe.tga")),
    (tag: "ITA", file: "flags/1936/ITA.tga", safe_file: None),
    (tag: "JAP", file: "flags/1936/JAP.tga", safe_file: None),
    (tag: "ENG", file: "flags/1936/ENG.tga", safe_file: None),
    (tag: "FRA", file: "flags/1936/FRA.tga", safe_file: None),
    (tag: "USA", file: "flags/1936/USA.tga", safe_file: None),
    (tag: "SOV", file: "flags/1936/SOV.tga", safe_file: None),
    (tag: "CHI", file: "flags/1936/CHI.tga", safe_file: None),
]
```

后续扩展 manifest：

```ron
[
    (
        tag: "GER",
        valid_from: Some("1935.9.15"),
        valid_to: Some("1945.5.8"),
        variant: None,
        file: "flags/1936/GER.tga",
        safe_file: Some("flags/1936/GER_safe.tga"),
        source: Some("source_id_ger_1935"),
    ),
]
```

第一版可以先实现静态格式，类型命名时给日期和 variant 留字段，避免二次破坏性改名。

---

## 推荐数据类型

新增模块位置优先考虑：

```text
crates/hoi4-content/src/flags.rs
```

核心类型：

```rust
pub struct FlagDef {
    pub tag: String,
    pub file: String,
    pub safe_file: Option<String>,
    pub variant: Option<String>,
    pub valid_from: Option<String>,
    pub valid_to: Option<String>,
}

pub struct FlagCatalog {
    defs: Vec<FlagDef>,
}

pub struct FlagRequest<'a> {
    pub tag: &'a str,
    pub date: Option<hoi4_state::GameDate>,
    pub variant: Option<&'a str>,
    pub historical_symbols: bool,
}
```

第一版 `resolve()` 可只按 `tag` 匹配：

```rust
impl FlagCatalog {
    pub fn resolve<'a>(&'a self, request: &FlagRequest<'_>) -> Option<&'a str> {
        // v1: tag match + safe_file handling only
    }
}
```

注意：如果 `hoi4-content` 不应依赖 `hoi4-state`，则 `FlagRequest.date` 第一版使用 `Option<String>` 或拆到 `hoi4-app`，避免 crate 依赖倒置。

---

## FlagBank 重构

当前 key：

```rust
HashMap<(String, String), Arc<FlagTexture>>
HashMap<(String, String), wgpu::BindGroup>
```

建议改为：

```rust
HashMap<String, Arc<FlagTexture>>
HashMap<String, wgpu::BindGroup>
HashSet<String>
```

新增 API：

```rust
pub fn get_or_load_path(
    &mut self,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    path_cfg: &PathConfig,
    layout: &wgpu::BindGroupLayout,
    uniform_buffer: &wgpu::Buffer,
    asset_path: &str,
) -> &wgpu::BindGroup
```

保留兼容 API 的选择：

- 推荐直接删除 `get_or_load(tag, ideology)`，因为只有国家选择调用，改动面小。
- 如果编译期发现还有其他调用，可临时保留 wrapper，但标注 deprecated。

fallback 规则：

- 先由 `FlagCatalog` fallback 到 `flags/1936/fallback.tga`。
- 如果 fallback 图也加载失败，才使用当前 1x1 灰色 GPU fallback。
- 不再回退到 vanilla `gfx/flags/`。

---

## UI 比例修正

当前 `menu_pass.rs` 使用：

```rust
let flag_box_h = flag_box_w * (52.0 / 82.0);
```

建议改成两层 rect：

- `flag_frame_rect`：固定外框，保持当前布局稳定。
- `flag_image_rect`：根据 `FlagBank::size_path(asset_path)` 或 catalog metadata 按比例 fit 到外框。

第一版实现方式：

- `draw_country_select()` 仍返回 `flag_rect`。
- `main.rs` 在拿到 texture size 后计算实际绘制 rect。
- 或 `CountrySelectLayout` 增加 `flag_frame_rect`，渲染国旗前动态 fit。

fit 公式：

```text
scale = min(frame_w / image_w, frame_h / image_h)
draw_w = image_w * scale
draw_h = image_h * scale
draw_x = frame_x + (frame_w - draw_w) / 2
draw_y = frame_y + (frame_h - draw_h) / 2
```

---

## 第一批真实 1936 国旗

| Tag | 1936 国旗 | 备注 |
|---|---|---|
| `GER` | 德国 1935 年后国旗 | 需要 safe variant |
| `ITA` | 意大利王国三色旗 + 萨伏依盾徽 | 不使用纯三色共和国旗 |
| `JAP` | 日章旗 | 国家国旗，不用旭日军旗 |
| `ENG` | Union Flag | 英国本土旗 |
| `FRA` | 法国第三共和国三色旗 | 蓝白红三色旗 |
| `USA` | 48 星美国国旗 | 1936 年时不是 50 星 |
| `SOV` | 苏联红旗 | 镰锤星版本 |
| `CHI` | 中华民国青天白日满地红 | 南京国民政府 |

---

## 版权与来源记录

新增：

```text
crates/hoi4-content/content/flags/flag_sources.ron
```

建议字段：

```ron
[
    (
        id: "source_id_usa_48_star",
        tag: "USA",
        url: "...",
        license: "Public domain",
        notes: "Converted to TGA and resized for UI use.",
    ),
]
```

规则：

- 不从 HOI4 原版资产复制图像。
- 优先使用公有领域或明确可再分发素材。
- 每张图记录来源、许可、转换步骤。
- 敏感历史符号保留 `safe_file` 替代路径。

---

## 阶段计划

### Phase 1: 内容与类型骨架

- [ ] 新增 `crates/hoi4-content/content/flags/flags_1936.ron`。
- [ ] 新增 `crates/hoi4-content/content/flags/flag_sources.ron`。
- [ ] 新增 `crates/hoi4-content/src/flags.rs`。
- [ ] 定义 `FlagDef`、`FlagCatalog`、`FlagRequest`。
- [ ] 提供 `FlagCatalog::load_1936()` 或 include-str 静态加载。
- [ ] 添加单元测试：8 个 tag 都能 resolve。

### Phase 2: FlagBank 按路径缓存

- [ ] 将 cache key 从 `(tag, ideology)` 改为 `asset_path`。
- [ ] 新增 `get_or_load_path()`。
- [ ] 新增 `size_path()`，返回实际图片尺寸。
- [ ] 移除 `gfx/flags/` 候选路径逻辑。
- [ ] fallback 改为 manifest fallback + 1x1 GPU fallback。
- [ ] 添加测试或最小 fake asset fixture。

### Phase 3: 国家选择界面接入

- [ ] `main.rs` 初始化并保存 `FlagCatalog`。
- [ ] `flag_to_draw` 从 `(tag, ideology, rect)` 改为 `(asset_path, rect)`。
- [ ] `CountryEntry` 可继续保留 `ideology` 供党派色条使用，但国旗不再使用它。
- [ ] 国旗绘制前根据图片比例 fit 到 frame 内。
- [ ] 验证 8 国国家选择界面均显示真实 1936 国旗。

### Phase 4: 资源补齐

- [ ] 准备 8 张真实 1936 国旗 TGA。
- [ ] 准备 `fallback.tga`。
- [ ] 准备 `GER_safe.tga` 或先在 manifest 标注 TODO。
- [ ] 记录来源和许可。
- [ ] 确认图片方向、alpha、sRGB 显示正常。

### Phase 5: 后续扩展

- [ ] topbar 玩家国旗接入同一 catalog。
- [ ] 政治面板国旗接入。
- [ ] 外交列表小旗接入。
- [ ] 支持日期范围 `valid_from/valid_to`。
- [ ] 支持 `variant`，用于内战、傀儡、流亡政府、自治领。
- [ ] 评估 PNG 解码支持。

---

## 验收标准

- [ ] 不配置 HOI4 安装目录时，国家选择国旗仍能显示项目自带资源。
- [ ] `GER/ITA/JAP/ENG/FRA/USA/SOV/CHI` 均不再读取 `gfx/flags/`。
- [ ] `ENG`、`USA` 等非 HOI4 `82:52` 比例图片不会被横向或纵向拉伸。
- [ ] 缺失国旗只报一次 missing，并显示项目 fallback。
- [ ] `cargo check` 通过。
- [ ] `FlagBank` 注释不再描述 HOI4 vanilla 国旗命名为主路径。
- [ ] `flag_sources.ron` 覆盖所有新增图片。

---

## 风险

- 真实国旗素材许可不清晰会阻塞提交，需要先记录来源。
- 德国 1936 国旗涉及敏感符号，需 safe variant 或设置项兜底。
- 如果 `hoi4-content` 引入 `hoi4-state::GameDate` 会造成 crate 依赖不理想，第一版应避免。
- 当前项目资产系统主要围绕 HOI4 game path，项目自带 assets 路径需要确认最合适的读取入口。
- 若 TGA 资源由外部工具转换，需统一 top-down/bottom-up 和 24/32 bpp，避免显示倒置或 alpha 异常。

---

## 推荐优先级

| # | 任务 | 优先级 | 估算 | 状态 |
|---|---|---|---|---|
| 1 | `FlagCatalog` + `flags_1936.ron` | P0 | 0.5 天 | Planned |
| 2 | `FlagBank::get_or_load_path()` | P0 | 0.5 天 | Planned |
| 3 | 国家选择界面接入 catalog | P0 | 0.5 天 | Planned |
| 4 | 国旗比例 fit 修正 | P0 | 0.5 天 | Planned |
| 5 | 8 国 TGA + 来源记录 | P0 | 1 天 | Planned |
| 6 | topbar / 政治 / 外交面板统一接入 | P1 | 1-2 天 | Planned |
| 7 | 日期范围与 variant | P1 | 1 天 | Planned |
| 8 | PNG 支持 | P2 | 1 天 | Planned |
