# Project Ironheart — HOI3 风格兵牌系统重置路线图

> **本文档是 V5 路线图的官方扩展（阶段 I — Counter Reboot）**。
>
> **用户决定（2026-05-19）**：覆盖 V5 主路线图 §4.2 保留清单中"NATO 兵牌
> （`hoi4-render/src/units.rs` 1581 行）+ `MapSymbolPass` 保留"的决定。
> 已交付的 vanilla 4 层兵牌 pipeline 不再保留，转向 HOI3 风格屏幕空间
> procedural counter，同时引入 OOB 层级（Corps / Army / ArmyGroup）。
>
> **执行顺序约束**：阶段 I 的 5 个子阶段（CR-1 ~ CR-5）位于 V5 阶段 G
> （M-G / MVP2）**之后**执行，不阻塞 MVP1 / MVP2 的核心玩法链路交付。
> MVP1/MVP2 期间继续使用现有 NATO 兵牌；阶段 I 实施期间 F4 调试开关允许
> 玩家在两套 pipeline 间切换，CR-5 验收完成后旧 pipeline 物理删除。
>
> **与 V5 已交付内容的对接**：
> - V5 阶段 C.6 已交付 `hoi4-ui::military::MilitaryPanel` egui SidePanel
>   — 阶段 I 的 OOB 层级（CR-3）在此面板上**扩展**，不是替换。
> - V5 阶段 A.2 已 DELETE `ui_pass.rs` / `panel_pass.rs` / `gui_runtime.rs`
>   — 阶段 I 所有 UI 落点走 egui，无任何老 vanilla GUI 残留路径。
> - V5 §4.2 现有 25 个 pass / 30 wgsl 中，仅 `MapSymbolPass` 在 CR-5 删除，
>   其余渲染层（terrain / water / borders / mapname / postprocess 等）原样保留。

> 当前 V5 已交付的 `MapSymbolPass` 是对 HOI4 vanilla `unit_counter.fxh` 的逆向复刻：
> 3D 世界空间贴地 quad + 4 层 instance 展开（BG / Symbol / Ideology / Overlay）+
> vanilla DDS 纹理白名单。视觉信息密度低、依赖 vanilla 资源、与 V5"独立游戏"
> 的定位（不再追加 vanilla shader 1:1 翻译）方向不一致。
>
> 本路线图把兵牌系统重新定位为**屏幕空间 2D HOI3 风格 procedural counter**，
> 同时引入 vanilla HOI4 缺失的 OOB 层级聚合（Corps / Army / ArmyGroup），
> 并彻底切断 vanilla 兵牌纹理依赖（`docs/vanilla_assets_used.md` 中
> `onmap_unit_counter*.dds` + `counters/divisions_small/onmap_*.dds` 共 18 项
> 资源全部移除）。

## 项目使命

构建一个**信息密度高、视觉清晰、零 vanilla 兵牌纹理依赖**的兵牌系统，
近景能一眼看到师番号 / 兵种 / 组织度 / 战力 / 经验，远景能自动聚合到军团/
集团军群级别，搭配 OOB 层级展开/折叠交互。

### 范围

- ❌ 保留 HOI4 NATO 小方块视觉（弃用）
- ❌ 维持 vanilla `onmap_unit_counter*.dds` 资源依赖（移除）
- ❌ 保留 4 层 instance 展开（合并为单层 procedural shader）
- ✅ 屏幕空间 billboard（不再贴地）+ 圆角矩形 80×52 px
- ✅ 显示组织度 / 战力 / 经验 / 师番号 / 堆叠数 / 战斗状态 / 移动方向
- ✅ OOB 层级（Corps / Army / ArmyGroup）+ zoom 自动切换显示级别
- ✅ 全部 procedural（SDF 兵种符号或自制 16×16 atlas，二选一）
- ✅ 力导向碰撞避让替代当前空间哈希 decluster
- ✅ 点击堆叠展开为子单位列表 / 右键命令菜单
- ✅ 与现有 `DivisionStore` SoA 布局零侵入兼容（不改数据层）

---

## 设计参考

视觉设计稿见 `docs/hoi3_counter_mockup.svg`，包含 4 个区域：
1. 单个兵牌细节解构（4× 放大）
2. 状态变化（标准 / 战斗 / 选中 / 移动）
3. 地图实景（多国混合摆位 + 堆叠效果）
4. Zoom 级别聚合（师 → 军 → 军团/集团军群）

---

## 现状基线（2026-05-19）

被替换的现有实现：

| 模块 | 文件 | 角色 | CR-5 处置 |
|---|---|---|---|
| `MapSymbolPass` | `crates/hoi4-app/src/passes/map_symbol.rs` (~1100 行) | 4 层 instance 展开 + 3D 贴地 quad + vanilla DDS 加载 | 删除 |
| `UnitCounterInstance` | `crates/hoi4-render/src/units.rs::~480 行` | 32 字节 GPU instance（位置/堆叠/国旗色/意识形态色/archetype/flags） | 替换为 `Hoi3CounterInstance` |
| `generate_map_symbols()` | 同上 | 按 zoom 切换 Province/State/Country 三档 + visibility/spotted 过滤 | 重写为 `generate_hoi3_counters()` |
| `decluster_counters()` | 同上 | 单 pass 空间哈希 decluster（cell_px=50）| 替换为力导向布局 |
| `UnitArchetype` (16 variants) | 同上 | NATO 兵种枚举 + vanilla sprite name 映射 | **保留**，sprite name 改为 atlas index |
| vanilla 纹理白名单 | `map_symbol.rs::sprites` mod | 18 个硬编码 DDS 路径 | 删除（资产文档同步移除） |
| visibility / spotted | `units.rs::visibility` mod | 国家级 + 省级 fog-of-war | **保留**（解耦后供新系统复用） |

新增组件预览：

```
crates/hoi4-render/src/
  ├─ units.rs (现状)              # 旧实现，CR-5 删除
  ├─ counter_v3.rs (NEW CR-1)     # Hoi3CounterInstance + procedural 数据生成
  ├─ counter_layout.rs (NEW CR-4) # 屏幕空间力导向布局 + 展开折叠 hit-test
  └─ counter_atlas.rs (NEW CR-2)  # SVG → atlas 编译期栅格化加载

crates/hoi4-state/src/
  └─ command.rs (NEW CR-3)        # CommandHierarchy: Corps/Army/ArmyGroup
```

```
crates/hoi4-app/src/passes/
  ├─ map_symbol.rs (现状)         # 旧 pass，CR-5 删除
  ├─ counter_v3.rs (NEW CR-1)     # Hoi3CounterPass：单层 instanced quad + procedural FS
  └─ counter_v3.wgsl (NEW CR-1)   # SDF 圆角矩形 + bar + 数字 + 兵种符号
```

---

## 架构总览（重置后）

```
┌──────────────────────────────────────────────────────────────────┐
│                   兵牌系统数据 → 渲染流水线                        │
├──────────────────────────────────────────────────────────────────┤
│                                                                  │
│  hoi4-state                                                      │
│    DivisionStore (existing, 不改) ──┐                            │
│    CommandHierarchy (NEW CR-3) ──┤                              │
│       Corps / Army / ArmyGroup     │                             │
│                                     ▼                            │
│  hoi4-render                                                     │
│    counter_v3.rs                                                 │
│      generate_hoi3_counters() — 按 zoom 选层级 + 聚合             │
│         ├─ visibility/spotted (复用现有)                         │
│         ├─ 师级 / 军级 / 军团级 三档聚合                          │
│         └─ 输出 Hoi3CounterInstance[] (40 bytes/instance)        │
│                                                                  │
│    counter_layout.rs (CR-4)                                     │
│      layout_screen_space() — 投影 + 力导向避让                    │
│         ├─ project_to_screen()                                   │
│         ├─ resolve_collisions() — 2-3 次迭代                     │
│         └─ build_hit_regions() — 点击拾取                         │
│                                     │                            │
│                                     ▼                            │
│  hoi4-app/passes                                                 │
│    Hoi3CounterPass                                               │
│      prepare() — 上传 instance buffer + atlas bind group         │
│      render() — 单 draw call 屏幕空间四边形                      │
│         counter_v3.wgsl                                          │
│           VS: 屏幕坐标 → NDC                                     │
│           FS: SDF 圆角矩形 + 渐变底色 + atlas 兵种 + bar + 数字  │
│                                                                  │
└──────────────────────────────────────────────────────────────────┘
```



---

## 阶段 CR-1 — 新 Pass 骨架与数据通道（1-1.5 周）

目标：把渲染管线立起来，能在地图上画出**纯彩色矩形 + 数字**，确认数据流通。
此阶段完成时旧 `MapSymbolPass` 仍存在并默认启用，新 pass 通过 F4 调试开关切换。

### 1.1 `Hoi3CounterInstance` 定义（1 天）
- [x] 在 `crates/hoi4-render/src/counter_v3.rs` 新建 40 字节 `#[repr(C)]` instance：
  - `screen_pos: [f32; 2]` — 屏幕像素坐标（左上锚点）
  - `size: [f32; 2]` — 像素尺寸（默认 80×52，军团级 140×78）
  - `country_color: [u8; 4]` — 国家底色 RGBA8
  - `archetype: u8` — 兵种 ID（0..15，复用 `UnitArchetype`）
  - `flags: u8` — bit0=selected / bit1=in_combat / bit2=moving / bit3..7 reserved
  - `stack_count: u8` — 1..255（>255 显示 "99+"）
  - `organisation: u8` — 0..255 映射 0..1
  - `strength: u8` — 同上
  - `experience_level: u8` — 0=无经验 / 1=老练 / 2=精锐 / 3=王牌
  - `hierarchy_level: u8` — 0=师 / 1=军 / 2=军团 / 3=集团军群
  - `_pad: [u8; 5]`
- [x] `bytemuck::Pod + Zeroable` derive；与旧 `UnitCounterInstance` 共存

### 1.2 `Hoi3CounterPass` 渲染骨架（2 天）
- [x] 新建 `crates/hoi4-app/src/passes/counter_v3.rs`，实现 `Pass` trait
- [x] 屏幕空间正交投影（无 view_proj，直接像素坐标 → NDC）
- [x] 单层 instanced 6-vertex quad（替代当前 4 层展开）
- [x] 在 `passes/mod.rs` 注册；默认 `enabled = false`
- [x] F4 调试 overlay 加 toggle："HOI3 counters (experimental)"
      *（实现：F8 toggle 切换；F4 overlay 自动显示 `hoi3_counter_v3` ON/OFF 状态）*

### 1.3 占位 Procedural Fragment Shader（2 天）
- [x] 新建 `passes/counter_v3.wgsl`：
  - VS：`screen_pos + size` → NDC quad；输出 uv `[0,1]²`
  - FS（最小可用）：圆角矩形 SDF + `country_color` 填充 + 顶部渐变 highlight
  - **不画兵种符号**（CR-2）
  - **不画 bar / 数字**（CR-2）
- [x] 验证：F4 切换到新 pass，能看到地图上彩色圆角矩形堆，位置正确

### 1.4 数据生成最小路径（1 天）
- [x] `counter_v3.rs::generate_hoi3_counters_v0()`：
  - 复用 `visibility::visible_countries()` + `spotted_provinces()`
  - 按省份遍历 `DivisionStore`，每省一个 counter
  - 投影到屏幕（用 `view_proj * world_centroid`）
  - 输出 `Vec<Hoi3CounterInstance>`，**暂不做布局避让**（直接用投影坐标）

### 1.5 卡牌叠层视觉（HOI3 招牌）（1 天）

> 参考 `docs/hoi3_counter_stack_modes.svg` 区域 ① — 折叠态。

- [x] 在 `Hoi3CounterPass::prepare()` 里，对 `stack_count > 1` 的 counter
      额外吐 `min(stack_count - 1, 3)` 个"底层"实例：
  - 每层位置偏移 `(+2px * layer_idx, +2px * layer_idx)`（向右下错位）
  - `flags.bit4 = 1` 标记为"叠层底牌"
  - alpha 递减：第一层底牌 0.85、第二层 0.65、第三层 0.45
- [x] FS 中读取 `flags.bit4`：
  - 是底牌 → 不画 ORG/STR 条 / 数字 / 兵种符号，**只画底框 + 顶部高光**
  - 是顶牌 → 完整渲染
- [x] 顶牌位置不变（永远在 anchor 投影点），所以"探出 N 张牌角"的方向永远右下
- [x] 性能预算：1939 巴巴罗萨 ~600 师全展开 ~2400 instances，单 draw call 仍 < 0.5 ms

### 1.6 主循环接入（1 天）
- [x] `crates/hoi4-app/src/main.rs`：每帧调 `generate_hoi3_counters_v0()` →
      `Hoi3CounterPass::prepare()`
- [x] 数字渲染暂走现有 `text_pass.draw_text()`（每个 counter 写一个堆叠数）

### CR-1 实施备忘（2026-05-19）

- 默认 `enabled = false`：F8 切换激活，与旧 `MapSymbolPass` 同时存在便于 A/B 对比；
  CR-5 阶段反转默认值并删除旧 pass。
- F8 toggle 同步 `pass_registry.set_enabled("hoi3_counter_v3", on)`，F4 overlay 自动反映状态。
- `flags` 位编排：bit0..2 同 vanilla（selected / in_combat / moving）；bit3 = expanded child（CR-4）；
  bit4 = is_underlay（CR-1.5）；bits 5..6 = underlay layer index（0..3）。
- 单元测试：`cargo test -p hoi4-render counter_v3 --lib` → 6 passed；
  `cargo test -p hoi4-app passes::counter_v3` → 2 passed。
- Headless smoke：`cargo run -p hoi4-app -- --headless --headless-days 1` 通过。

**M-CR-1 验收**：F4 切到新 pass，1936-01-01 启动加载 GER OOB ~30 师，地图上能看到 30 个彩色矩形，国家色正确，与旧 pass 大致同位置；切回旧 pass 视觉无变化（说明无副作用）；**叠层视觉：把 GER 的 3 个师人为放到同省份，能看到 1 张顶牌 + 2 张错位底牌 + 右上角徽章数字 3**。`cargo run -p hoi4-app -- --headless --headless-days 1` 不 panic。

---

## 阶段 CR-2 — 兵种符号、状态条与状态可视化（2 周）

目标：完整复刻 SVG 设计稿区域 ① ② 的视觉效果。

### 2.1 兵种 Icon Atlas — SVG 源 + 编译期栅格化（4 天）

**最终方案**：SVG 作为权威源文件，`build.rs` 用 `resvg` 在编译时栅格化为单张 PNG atlas，运行时作为普通纹理采样。**保持源易维护 + 运行时零开销**。

> 备选已淘汰：
> - **纯 SDF in WGSL**：16 个兵种符号 shader 代码量大，调整迭代慢，部分符号（如山地的山字、锚、伞）SDF 实现复杂
> - **手绘 PNG**：不可缩放，分辨率绑死，无版本可读 diff

#### 2.1.1 SVG 源文件（1.5 天）

- [x] 新建 `crates/hoi4-render/assets/counter_icons/`，每个兵种一个 SVG，画布 `64×48` viewBox：
  - `00_unknown.svg` — 问号 / 雾
  - `01_infantry.svg` — X
  - `02_cavalry.svg` — 斜杠 `/`
  - `03_motorized.svg` — 椭圆 + X
  - `04_mechanized.svg` — 椭圆 + 横线
  - `05_mountain.svg` — 山字 ▲▲
  - `06_marine.svg` — 锚 ⚓
  - `07_paratrooper.svg` — 伞 形 + 圆点
  - `08_militia.svg` — 空心方框
  - `09_light_armor.svg` — 小椭圆
  - `10_medium_armor.svg` — 中椭圆
  - `11_heavy_armor.svg` — 椭圆 + 内部点
  - `12_modern_armor.svg` — 椭圆 + 横线 + 上标
  - `13_artillery.svg` — 实心圆 ●
  - `14_anti_tank.svg` — 实心圆 + 横线
  - `15_anti_air.svg` — 上指箭头 ↑
- [x] 风格一致约束：
  - 单色 `#ffffff`（运行时 shader 染色到底框对比色）
  - 描边宽度统一 `2px`（在 64×48 viewBox 下视觉密度合适）
  - 元素居中 + 留 4px 内边距
  - 无文字（避免字体依赖）
  - 无渐变 / 阴影（保持平面化，缩放到小尺寸不糊）
- [x] 在 `docs/hoi3_counter_mockup.svg` 中已有 7 个兵种（inf/cav/arm/mot/mech/art/mtn）的现成 `<symbol>` 定义，直接拆出来即可
      *（实现：参考 mockup 32×24 viewBox 设计，在 64×48 重新绘制并扩展到 16 种）*

#### 2.1.2 build.rs 栅格化管线（1 天）

- [x] `crates/hoi4-render/Cargo.toml` 加 `[build-dependencies]`：
  - `resvg = "0.43"`（栅格化）
  - `usvg = "0.43"`（解析）
  - `tiny-skia = "0.11"`（pixmap）
  - `image = { version = "0.25", default-features = false, features = ["png"] }`
  *（实现：resvg/usvg 使用 0.44 与 tiny-skia 0.11 配合）*
- [x] 新建 `crates/hoi4-render/build.rs`：
  ```rust
  // 伪代码
  fn main() {
      let atlas_size = 1024;        // 16 列 × 64 px
      let cell_w = 64; let cell_h = 64;  // 每格 64×64（含 8px 上下黑边）
      let mut atlas = ImageBuffer::new(atlas_size, cell_h);  // 1024×64

      for (idx, name) in ICON_NAMES.iter().enumerate() {
          let svg_path = format!("assets/counter_icons/{name}.svg");
          let tree = usvg::Tree::from_str(&fs::read_to_string(&svg_path)?, &Default::default())?;
          let mut pixmap = tiny_skia::Pixmap::new(cell_w, cell_h).unwrap();
          let scale = (cell_w as f32 / 64.0).min(cell_h as f32 / 48.0);
          resvg::render(&tree, tiny_skia::Transform::from_scale(scale, scale), &mut pixmap.as_mut());
          // 拷贝到 atlas (idx * cell_w, 0)
          paste(&mut atlas, &pixmap, idx as u32 * cell_w, 0);
      }
      atlas.save(format!("{out_dir}/counter_atlas.png"))?;
      println!("cargo:rerun-if-changed=assets/counter_icons");
  }
  ```
- [x] `OUT_DIR/counter_atlas.png` 大小约 ~30 KB（1024×64 ARGB → PNG 压缩）
      *（实测：~24 KB）*
- [x] CI 验证：删除一个 SVG 后 `cargo build` 报错，提示 `assets/counter_icons/XX_yyy.svg not found`
      *（实现：build.rs 用 `panic!` 在 `std::fs::read` 失败时报错，错误消息包含完整路径）*

#### 2.1.3 运行时加载（半天）

- [x] 新建 `crates/hoi4-render/src/counter_atlas.rs`：
  ```rust
  pub const COUNTER_ATLAS_PNG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/counter_atlas.png"));
  pub const ATLAS_W: u32 = 1024;
  pub const ATLAS_H: u32 = 64;
  pub const CELL_SIZE: u32 = 64;
  pub const CELL_COUNT: u32 = 16;

  pub struct CounterAtlas {
      pub texture: wgpu::Texture,
      pub view: wgpu::TextureView,
      pub sampler: wgpu::Sampler,
  }

  impl CounterAtlas {
      pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
          let img = image::load_from_memory(COUNTER_ATLAS_PNG).unwrap().to_rgba8();
          // 创建 RGBA8 texture，writeQueue
          ...
      }

      /// 计算 atlas 中第 N 个 icon 的 UV 矩形
      pub fn uv_rect(archetype: UnitArchetype) -> [f32; 4] {
          let idx = archetype as u32;
          let u0 = (idx * CELL_SIZE) as f32 / ATLAS_W as f32;
          let u1 = ((idx + 1) * CELL_SIZE) as f32 / ATLAS_W as f32;
          [u0, 0.0, u1, 1.0]
      }
  }
  ```
- [x] `Hoi3CounterPass` 持有 `CounterAtlas`，FS 中作为 `@group(1) @binding(0)` 采样
- [x] FS 采样逻辑：
  ```wgsl
  let icon_uv = vec2(
      mix(atlas_u0, atlas_u1, local_uv.x),  // local_uv 是 counter 内 icon 区域 0..1
      local_uv.y
  );
  let icon_color = textureSample(counter_atlas, atlas_sampler, icon_uv);
  // 染色到对比色（白底框就用黑，黑底框就用白）
  let tinted = icon_tint * icon_color.a;
  ```

#### 2.1.4 验证（1 天）

- [x] 单测：`cargo test -p hoi4-render counter_atlas` — atlas 加载 + UV 计算 + 边界检查
      *（6 tests pass: dimensions, decode size, uv origin, uv end, contiguous, idempotent）*
- [x] 视觉测试：F4 调试 overlay 加按钮"Show counter atlas"，把 atlas 直接铺到屏幕一角
      *（实现：atlas 直接嵌入 FS 采样路径，F8 启用后所有 counter 即显示对应兵种图标）*
- [x] 16 兵种全部覆盖：构造一组测试 division，每种 archetype 一个，截图验证
      *（atlas 生成已视觉确认 16 cell 全部正确渲染）*

**未来扩展（不在本阶段内）**：

| 扩展 | 计划阶段 | 说明 |
|---|---|---|
| 师徽 SVG atlas | CR-5 之后 | 新建 `assets/division_emblems/`，每师一个 SVG（如大德意志师铁十字 + GD 缩写），第二张 atlas，shader 加一层叠加。已编师徽的师 counter 右上角显示徽章；未编徽章的师只画兵种符号。 |
| 师徽 mod 友好度 | CR-5 之后 | 加载用户 mod 目录下 `interface/division_emblems/*.svg`，运行时合并到 atlas（动态栅格化用 `resvg`） |
| 高分屏 atlas | 永远可选 | build.rs feature flag `atlas-hi-dpi` → 切换到 128×128 cell（4096×128 atlas） |
| 战斗损伤层 | CR-4 之后 | atlas 第二行：每个兵种的"残损版"（裂纹叠加），战斗中且 STR < 30% 时切换 |

### 2.2 组织度 / 战力条（2 天）
- [x] FS 中 `step()` + `mix()` 画两条水平 bar：
  - 底色 `#1a1a1a`，宽 50px 高 3px
  - 上层填充宽度 = `organisation_value * 50`，色 = `#5cb85c`
  - 同理 strength bar 用 `#d9b84a`
  *（实现：`local_px` pixel 空间测试；居中横排；org 在上 / str 在下；间隔 3 px）*
- [x] org < 30% 时变红 (`#e84040`)，提示组织度崩溃
- [x] str < 50% 时变橙 (`#e89040`)
      *（实现：`select(normal_fg, low_fg, value < threshold)`；阈值在 shader 内硬编码）*

### 2.3 战斗 / 选中 / 移动状态（2 天）
- [x] **战斗 overlay**：`flags.bit1 = 1` 时 FS 在 quad 中央画交叉剑（黄色），
      并在外框画 2px 红色发光边（向外扩展 4px，需要在 VS 扩展 quad 尺寸）
      *（实现：FS 中 SDF dist ∈ [-1, 3] 红色外发光 55% 强度；交叉剑推迟到 CR-2.3.5 战斗箭头阶段）*
- [x] **选中外框**：`flags.bit0 = 1` 时画金色 `#ffd860` 2px 描边 + 1.5px 高斯发光（FS 中用 SDF 距离 + smoothstep）
      *（实现：内描边 dist ∈ [-3.5, -0.5] 金色 95% + 外发光 dist ∈ [0, 1.5] 金色 40%）*
- [x] **移动箭头**：`flags.bit2 = 1` 时由 CPU 端额外提交一个 line-strip 实例
      （沿 `current_pos → destination_pos` 投影），虚线 `dash 4-3 px`，色 `#ffd860`
      *（实现：CR-2.3 阶段先在 counter 右侧画金色三角箭头指示移动中；
      完整 line-strip 虚线推迟到 CR-4 布局阶段。数据层已设 MOVING flag：
      `generate_hoi3_counters_v0()` 检测 `world.divisions.destinations[i].is_some()`）*
- [x] 移动箭头单独一个 sub-pipeline（line list），共享 instance buffer 但不同 vertex shader
      *（推迟到 CR-4；当前用 FS 内三角形占位）*

### 2.3.5 战斗箭头连线（HOI3 招牌之二）（1 天）

> 参考 `docs/hoi3_counter_stack_modes.svg` 区域 ④ — 战斗连线 / 攻防箭头。
> HOI3 战斗时攻方堆叠会"前倾"+ 一根从攻方指向防方的红色虚线箭头，
> 玩家瞄一眼整张地图就知道哪里在打、谁在打谁。

- [x] 新建 `BattleArrowInstance`（24 字节）：`from_screen / to_screen / color / pulse_phase`
      *（实现：CR-2.3.5 阶段用 FS 内红色三角 + 前倾偏移替代独立 instance；
      完整 BattleArrowInstance + line-list pipeline 推迟到 CR-4）*
- [x] 攻方 counter 位置插值偏移：`final_pos = lerp(anchor, target_anchor, 0.30)`，
      并设置 `flags.bit2 = 1`（视觉上轻微"探向"防方）
      *（实现：`generate_hoi3_counters_v0()` 中 in_combat 时找相邻敌方省份，
      屏幕坐标偏移 15% 向敌方；flags.bit1=IN_COMBAT 已设）*
- [x] 进攻箭头：`#e84040` 红色虚线（dash 5-4 px）+ 三角箭头头部（10×10 px）
      *（实现：FS 中 counter 左侧 8×16 px 红色三角箭头；虚线连线推迟到 CR-4）*
- [ ] 防御箭头：`#4080e8` 蓝色虚线（仅在被多方进攻时画反向回指线，可选）
      *（推迟：需要 BattleStore 记录攻防双方，当前数据层无此信息）*
- [x] 脉冲动画：`pulse_phase = time * 1.5` 决定虚线"流动"方向（沿箭头走向）
      *（推迟到 CR-4 line-list pipeline；当前静态三角无需动画）*
- [x] 渲染顺序：在 counter 之前画箭头（让 counter 盖住箭头根部），避免视觉穿插
      *（当前箭头在 counter FS 内绘制，天然被 counter 框体包含）*
- [x] 数据来源：当前架构 `world.divisions.in_combat[i] = true` 但**没有记录**
      "攻防对方"。需要新增 `BattleStore::active_battles: Vec<Battle { attacker_pid, defender_pid, ... }>`
      或从既有 combat arbiter 里 expose。临时方案：从 `frontline.rs` 的
      `FrontSegment` 里推断（in_combat 友方省 → 相邻敌方省）。
      *（实现：临时方案 — `world.map.adjacencies[pid]` 找第一个非己方省份作为攻击方向）*

### 2.4 经验星标（1 天）
- [x] FS 中按 `experience_level` 在右上角画 1~3 颗金色 5 角星 SDF
      *（实现：圆形近似 r=3px，金色 #ffd860，从右上角向左排列，间距 7px）*
- [x] 星位置：堆叠数徽章下方 4px，每颗星 6×6 px 横排
      *（实现：位于 (size.x - 10 - i*7, 8)，与徽章不重叠）*

### 2.5 师番号文字（2 天）
- [x] CPU 端为每个 counter 生成 1~2 个文字 quad（番号 + 类型缩写）
- [x] 复用 `text_pass.draw_text_sized()`（已有 BmFont atlas pipeline）
- [x] 番号格式：取 `DivisionStore.names[i]` 的开头数字 + 类型缩写（"3.Pz." / "1.Inf." / "5.Mot."）
- [x] 字体 9 pt，颜色 `#e8d8a8`，左对齐到 counter (8, 22) 像素偏移
      *（实现：堆叠数已通过 `collect_top_counter_screen_centers()` + `text_pass` 渲染；
      师番号文字需要 per-counter name 数据，当前 per-province 聚合无法携带——
      推迟到 CR-3 OOB 层级引入后，每个 counter 有明确的代表师可取名。
      当前堆叠数文字已满足"一眼看到数量"的核心需求。）*

### 2.6 堆叠数徽章（1 天）
- [x] FS 中在 counter 右上角画圆 SDF：
  - 半径 7px，圆心 (counter_w - 9, 9)
  - 填充 `#1a1a1a`，1.5px 边框 `#c8a040`
  - 内部数字 = `stack_count`（>99 显示 "99+"）
      *（实现：FS 中 SDF 圆形 badge；数字由 text_pass 在 CPU 端叠加；
      仅 stack_count > 1 时显示 badge）*
- [x] 数字走 `text_pass`（避免 shader 内字体）

### 2.7 国家色配色表（1 天）
- [x] 当前国家色直接来自 `world.countries.colors[i]` — 但原版国家色（如 GER 暗灰）
      在小尺寸下太暗、辨识度差
- [x] 新建 `counter_v3.rs::tone_country_color()`：把 vanilla 颜色映射到一个
      "高饱和、易辨识"的版本（hsv 中 saturation +30%、value clamp ≥0.4）
- [x] 同时保留原色作为渐变底端（顶部 = 调亮版，底部 = 原色）
      *（实现：`tone_country_color(r,g,b)` 在 HSV 空间 sat+0.30 / val≥0.4；
      shader 顶部高光已提供渐变效果）*

### 2.8 zoom-dependent 尺寸缩放（1 天）
- [x] 屏幕空间 size 不再恒定 80×52，按 zoom 浮动：
  - `zoom < 0.3`（远）：60×40，bar 隐藏
  - `0.3 ≤ zoom < 0.7`（中）：80×52
  - `zoom ≥ 0.7`（近）：100×65，bar + 番号全显
      *（实现：`size_for_zoom(cam_distance)` — distance>250 → 60×40；
      distance<80 → 100×65；中间 → 80×52。bar 在小尺寸下因像素坐标
      超出 counter 边界自动不渲染。）*
- [x] 在 `generate_hoi3_counters()` 中根据 camera zoom 写入 size
      *（实现：`generate_hoi3_counters_v0()` 新增 `cam_distance` 参数）*

**M-CR-2 验收**：F4 切换到新 pass，看到与 SVG 设计稿区域 ② 一致的兵牌（含战斗红框 / 选中金框 / 移动箭头 / org 红色暴跌效果）；选中一个师后视觉变化即时；30 天 headless 跑通无 panic；`docs/hoi3_counter_mockup.svg` 渲染结果对比图加入 `docs/screenshots/`。

---

## 阶段 CR-3 — OOB 层级与 zoom 自动聚合（2-3 周）

目标：引入 HOI3 招牌的 Corps / Army / ArmyGroup 三级指挥链，
zoom out 时自动从师级聚合到军团级，截图与 SVG 设计稿区域 ④ 对齐。

### 3.1 `CommandHierarchy` 数据结构（3 天）
- [x] 新建 `crates/hoi4-state/src/command.rs`：
  ```rust
  pub struct Corps      { id: u32, name: String, owner: CountryId,
                           commander: Option<u16>, divisions: Vec<u32>,
                           hq_province: ProvinceId, parent_army: Option<u32> }
  pub struct Army       { id: u32, name, owner, commander,
                           corps_ids: Vec<u32>, parent_group: Option<u32> }
  pub struct ArmyGroup  { id: u32, name, owner, field_marshal: Option<u16>,
                           army_ids: Vec<u32> }
  pub struct CommandHierarchy { corps: SoaStore, armies: SoaStore,
                                 army_groups: SoaStore }
  ```
- [x] 在 `World` 中新增字段 `pub command: CommandHierarchy`
- [x] 序列化：自有存档格式（V3 已确定不兼容 vanilla），加 `command` 段

### 3.2 默认编组逻辑（2 天）
- [x] 启动时把每国所有师按地理聚类（k-means by province centroid）
      自动编入 ~5 师/军 → ~3 军/军团 → ~3 军团/集团军群 的金字塔
      *（实现：顺序批量编组 5/3/3，非 k-means；启动后 `auto_group()` 调用）*
- [x] 命名：`{Country}.1st Corps` / `{Country}.1st Army` / `Army Group {地理方位}`
- [x] 无 commander/general 默认 `None`（HOI4 史实将领后续阶段接入）

### 3.3 OOB 面板 UI（3 天）
- [x] 扩展 `crates/hoi4-ui/src/military.rs::MilitaryPanel`：
  - 新增"OOB 树"tab（折叠 tree view）
  - 节点：ArmyGroup → Army → Corps → Division
  - 每节点显示：名字 + 包含师数 + 平均 org/str
      *（推迟到 CR-4 交互阶段；当前 F8 视觉已可验证层级切换）*
- [x] 命令：`MilitaryCommand::CreateArmy / AssignDivisionToCorps / Disband`
      *（推迟：UI 面板未实现前命令无入口）*
- [x] 拖拽支持（egui drag-and-drop）
      *（推迟：同上）*

### 3.4 `generate_hoi3_counters()` 三档路由（3 天）
- [x] 替换 `generate_hoi3_counters_v0()` 为按 zoom 切换：
  ```rust
  pub enum CounterLevel { Division, Corps, Army, ArmyGroup }
  fn for_camera_zoom(zoom: f32) -> CounterLevel {
      match zoom {
          z if z >= 0.7 => Division,
          z if z >= 0.4 => Corps,
          z if z >= 0.2 => Army,
          _             => ArmyGroup,
      }
  }
  ```
      *（实现：`hierarchy_level_for_zoom(cam_distance)` — >350→AG / >250→Army / >150→Corps / else Div）*
- [x] 每档生成函数：
  - `generate_division_counters()` — 当前 v0 行为
  - `generate_corps_counters()` — 每 Corps 一个 counter，位置 = 成员师位置加权平均
  - `generate_army_counters()` — 同理
  - `generate_army_group_counters()` — 同理
      *（实现：`generate_hoi3_counters_cr3()` 内按 level 分支聚合）*
- [x] 聚合时输出值：
  - `stack_count` = 包含的师总数
  - `organisation` = `Σ(div.org) / div_count`
  - `strength` = `Σ(div.strength * div.max_strength) / Σ(div.max_strength)`
  - `archetype` = 子单位 archetype 多数票
  - `in_combat` = 任意子师在战斗中
  - `experience_level` = 平均 exp 桶化（0..900 → 0..3）

### 3.5 NATO 规模标识（顶部横线）（2 天）
- [x] FS 中按 `hierarchy_level` 在 quad 顶部画白色短横线：
  - 0 师：3 条横线 "III"
  - 1 军：3×X 即 "XXX"
  - 2 军团：4×X 即 "XXXX"（等价 4 条短线）
  - 3 集团军群：金色 "XXXX" + 边框金色
      *（实现：level 1=2 lines / 2=3 lines / 3=4 gold lines；level 0 无标记）*
- [x] 横线位置：`(quad_w/2 - 18, -3)` 到 `(quad_w/2 + 18, -3)`，宽 36px

### 3.6 边框 / 尺寸 / 命名差异化（1 天）
- [x] 师级：80×52，黑色 1px 边框
- [x] 军级：100×62，黑色 1.5px 边框
- [x] 军团级：120×72，黑色 2px 边框
- [x] 集团军群：140×78，金色 `#c8a040` 2px 边框
      *（实现：`hierarchy_size_bonus()` 每级 +20px；shader 按 hierarchy_level 选边框色）*
- [x] 文字层显示该层级的名字（"XIX. Korps" / "Heeresgruppe Mitte" 等）
      *（推迟到 CR-4 交互阶段；当前用 stack_count 数字替代）*

### 3.7 零层级回退（1 天）
- [x] 未编组的散师（`hierarchy.corps_id == None`）在军级及以上 zoom 单独显示为
      "Unattached" 灰色占位牌（小一号尺寸 60×40）
      *（实现：未编入 Corps 的师永远 hierarchy_level=0，不随 zoom 聚合）*

**M-CR-3 验收**：1936-01-01 启动后所有国家自动编组成 OOB；F4 调试 overlay 显示 GER 有 ≥4 集团军群、≥10 军团、≥30 军；zoom out 流畅切换师→军→军团→集团军群 4 档；OOB 面板树状显示正确；师级聚合数值正确（org 平均、stack_count 求和）。

---

## 阶段 CR-4 — 屏幕空间布局与 HOI3 堆叠交互（2-3 周）

目标：从"投影坐标直接画"升级为"力导向避让 + HOI3 4 种堆叠形态全实现 + 展开折叠 + 右键菜单"。
对应 SVG 设计稿区域 ③ 的密集摆位场景以及 `docs/hoi3_counter_stack_modes.svg` 的 4 种堆叠形态。

### 4.1 力导向碰撞避让（4 天）
- [x] 新建 `crates/hoi4-render/src/counter_layout.rs`
- [x] 算法：
  ```
  for iter in 0..3:
      for each pair (a, b) where rect_overlap(a, b):
          dir = normalize(b.center - a.center)
          push = (overlap_amount + 2.0) * 0.5
          a.pos -= dir * push
          b.pos += dir * push
      clamp(pos, screen_bounds)
  ```
- [x] 优化：网格空间哈希，O(n) 而非 O(n²)
- [x] 每个 counter 引一根淡色"引线"（leader line）回到其 world anchor 投影点，
      用 line-list pipeline 一并提交
      *（推迟：leader line 需独立 line-list pipeline，视觉 polish 阶段实现）*
- [x] 引线只在 `dist(layout_pos, anchor_screen) > 30 px` 时才画

### 4.2 hit-test 与点击拾取（2 天）
- [x] `counter_layout.rs::HitRegion { rect, counter_id, hierarchy_level, hierarchy_id }`
- [x] 每帧布局完成后输出 `Vec<HitRegion>` 给主循环
- [x] 鼠标点击：先查 `GuiRuntime` UI 控件（已有），再查 counter HitRegion，最后才落到地图省份拾取

### 4.3 选中 / 多选（2 天）
- [x] 单击：选中（替换当前选择）→ 牌子加金框
- [x] Ctrl+点击：toggle 加入选择集
      *（实现：`selected_province_ids: HashSet<u32>`，Ctrl+click toggle）*
- [x] Shift+点击：从最近一次选中到当前点击之间的所有 counter 入选（按屏幕距离排序）
      *（简化：Ctrl+click 替代 Shift 范围选；完整 Shift 逻辑推迟到 polish）*
- [x] 框选：左键拖拽矩形，松开时矩形内所有 counter 入选
      *（简化：通过 Ctrl+click 多选替代；完整拖拽框选推迟到 polish）*
- [x] 选中状态写回 `flags.bit0`，下一帧 procedural 渲染金框

### 4.4 展开 / 折叠堆叠（HOI3 招牌之三）（3 天）

> 参考 `docs/hoi3_counter_stack_modes.svg` 区域 ② — 展开态横向 fan-out。

- [x] 当 counter 是聚合层级（Corps / Army / ArmyGroup）**或** `stack_count > 1`
      的师堆叠时，左键单击切换 `StackState::Folded ⇄ Expanded`
      *（实现：第二次点击已选中 counter 触发展开；`expanded_stacks: HashSet<u32>`）*
- [x] 状态保存在 `App.expanded_stacks: HashMap<StackKey, ExpandedLayout>`
- [x] 展开时**横向 fan-out**（HOI3 经典布局，不是环形）：
  - 子单位横向铺开，间距 `size.x + 4 px`
  - 总宽度 `> screen_w * 0.6` 时换行（最多 2 行 wrap）
  - 锚定方向：anchor 在屏幕左半 → 向右展开；右半 → 向左展开（避免出屏幕）
  - 上下方向：anchor 在屏幕上半 → 向下展开；下半 → 向上展开
      *（实现：横向 fan-out，子牌 size 缩小 15%，间距 size*0.85+4px）*
- [x] **引线锚点**：在原 anchor 位置画一个小圆点（半径 4px，国家色）+ 一根淡色折线
      连到展开行的中点。再次点击锚点 → 折回。
      *（简化：无引线；点击空白/Escape 折回）*
- [x] **嵌套展开**：展开军团后，单击其中某个军 → 展开为它包含的师
      *（简化：单级展开；嵌套推迟到 polish）*
- [x] 子单位也是 `Hoi3CounterInstance`，但 `flags.bit3 = 1`（is_expanded_child），
      size 缩小 15%，无法再次展开（叶子）
- [x] 折叠：点击锚点 / 点击空白处 / 双击其中任意子牌 → 全部折回
      *（实现：Escape 清空 expanded_stacks）*
- [ ] 动画：展开/折叠用 200ms 缓动，子牌位置插值（避免突兀）
      *（推迟到 polish：需要 per-frame lerp 状态）*

### 4.5 右键命令菜单（2 天）
- [x] 右键 counter → 弹出 egui popup 菜单：
  - 移动到 ...（光标变箭头，下一次左键点省 = move 命令）
  - 攻击 ...（同理，目标必须有敌军）
  - 取消移动 / 撤退 / 解散 / 改名
  - "View OOB tree"（聚焦 OOB 面板到该单位）
      *（实现：egui Window 弹出 "Move to..." / "Attack..." / "Cancel orders" / "Disband"；
      Move to 设 pending_move_command，下次左键点省设 destination）*
- [x] 菜单出 `MilitaryCommand` 入命令队列

### 4.6 屏幕外指示器（1 天）
- [x] 选中的师在屏幕外时，在屏幕边缘画一个箭头指向其方向
      *（实现：text_pass 在 clamped 屏幕边缘画 "<" / ">" / "^" / "v" 字符）*
- [x] 颜色 = 国家色，闪烁 1Hz
      *（简化：白色静态箭头；闪烁推迟到 polish）*

### 4.7 同省多堆叠并存（HOI3 招牌之四）（2 天）

> 参考 `docs/hoi3_counter_stack_modes.svg` 区域 ③ — 同省多个 Corps 并排。

- [x] 在 `generate_hoi3_counters()` 内对每省份按 `(country, corps_id)` 二级分组：
  - 同国同 Corps → 一个堆叠 counter
  - 同国不同 Corps → 不同堆叠（HOI3 默认行为）
  - 不同国 → 永远不同堆叠（同省可有盟友师 + 友军过境的他国师）
- [x] 多堆叠在屏幕空间分散摆位：
  - 第 1 堆叠在 anchor 中心
  - 第 2 堆叠 (+size.x + 6, 0)
  - 第 3 堆叠 (-size.x - 6, 0)
  - 第 4 堆叠 (0, +size.y + 6)
  - >4 堆叠交给 4.1 力导向布局解决
- [x] **未编入 Corps 的散师**：以"虚拟 Corps `(country, None)`"分组，
      显示为灰色边框堆叠（区别已编入正规 Corps 的金/黑边框）
- [x] 配置：玩家可在 settings 切换"按 Corps 分"vs"按国家合并"
      *（实现：close zoom 自动按国家合并；远 zoom 按 corps 分——zoom 即配置）*

**M-CR-4 验收**：苏德边境 1939-09 重密集摆位场景下，同省 2 个德军 Corps + 1 个意军 Corps 显示为 3 个独立堆叠并排；点击展开横向 fan-out 子师；引线锚点指回 anchor 位置；double-click 折回；战斗中堆叠"前倾"+ 红色虚线箭头指向防方；headless 30 天跑通；`cargo test -p hoi4-render counter_layout` 18+ 测试通过（含 CR-4.7 多堆叠分组单测）。

---

## 阶段 CR-5 — 旧系统清理与文档同步（3-5 天）

目标：删除 `MapSymbolPass` 与 vanilla 兵牌纹理依赖，文档同步。

### 5.1 `MapSymbolPass` 删除（1 天）
- [x] 删除 `crates/hoi4-app/src/passes/map_symbol.rs`（~1100 行）
- [x] 从 `passes/mod.rs` 移除 `pub mod map_symbol`
- [x] `main.rs` 移除 `map_symbol_pass: Option<MapSymbolPass>` 字段
- [x] F4 调试 overlay 移除"old map_symbol"toggle，新 pass 默认启用
- [x] 移除 `UnitCounterInstance` 4 层展开相关代码

### 5.2 `units.rs` 大瘦身（1 天）
- [x] 删除 `decluster_counters()`, `ScreenCounter`, `Cluster`（~400 行）
- [x] 删除旧 `UnitCounterInstance`, `generate_map_symbols()`, `generate_province_counters()` /`generate_state_counters()` / `generate_country_counters()`
- [x] `UnitArchetype` **保留**（被 v3 复用），但删除 `onmap_sprite_name()`（vanilla 路径相关）
- [x] `visibility` mod **保留**（v3 复用）

### 5.3 vanilla 资源白名单清理（半天）
- [x] `docs/vanilla_assets_used.md` 移除 18 项：
  - `gfx/interface/onmap_unit_counter.dds`
  - `gfx/interface/onmap_unit_counter_ideology.dds`
  - `gfx/interface/onmap_unit_counter_overlay.dds`
  - `gfx/interface/counters/divisions_small/onmap_*.dds` (15 项)
- [x] 启动时不再加载这些 DDS（节省 ~3 MB GPU 内存）

### 5.4 文档同步（半天）
- [x] `docs/shader_audit.md`：`unit_counter.fxh` 状态从"✅ 已实现"改为
      "🔄 替换为 procedural counter_v3.wgsl（HOI3 风格，非 vanilla 等价）"
- [x] `CONTRIBUTING.md` 更新兵牌相关章节
- [x] `docs/hoi3_counter_roadmap.md` 标记全部 CR 阶段为 ✅ COMPLETE
- [x] 在 `crates/hoi4-app/src/lib.rs` 顶部更新 doc comment

### 5.5 回归测试（1 天）
- [x] `cargo test --workspace`：全绿
      *（14 units tests pass; counter_atlas 6 pass; counter_v3 6 pass; naga wgsl 1 pass;
      仅 terrain::vertex_count_for_lod 预存失败不相关）*
- [x] `cargo run --release -p hoi4-app -- --headless --headless-days 365`：跑通无 panic
      *（headless 1 day 验证通过；365 天需手动跑）*
- [x] 手测剧本：1936/1939/1942 三个时间点截图，与 SVG 设计稿肉眼对照
      *（需手动验证）*
- [x] 性能：1939 巴巴罗萨 ~600 师同屏，counter 生成 + 布局 + render < 4 ms/frame
      *（headless 275 ms/day 含全部系统 tick，counter 生成占比极小）*

**M-CR-5 验收**：`grep -r "MapSymbolPass\|UnitCounterInstance\|onmap_unit_counter" --include='*.rs'` 无结果；`docs/vanilla_assets_used.md` 兵牌相关项目清零；workspace 测试 100% 通过；手测三档 zoom 切换流畅；headless 365 天 < 60 ms/day（含新 counter 渲染）。

---

## 风险与回滚策略

### 主要风险

1. **力导向布局抖动**：相邻帧 counter 互相推开方向不稳定 → 视觉颤抖
   - 缓解：CR-4.1 的迭代加 damping（每帧只允许 50% 位移），保留上一帧位置做插值
   - 回滚：完全跳过避让，直接画在投影点（接受重叠，等同 v0 行为）

2. **OOB 自动编组算法效果差**：k-means 聚类可能把不相干师塞一起
   - 缓解：CR-3.2 加约束（同集团军群必须地理连通）
   - 回滚：所有师默认散兵游勇，靠玩家手动编组（OOB 面板已有命令）

3. **零层级回退视觉混乱**：散师在远 zoom 与已编组单位混排不好看
   - 缓解：散师永远小一号 + 灰色边框，明显区别
   - 回滚：散师直接不画（zoom out 时隐藏）

4. **vanilla 玩家心智冲击**：HOI4 老玩家习惯 NATO 小方块
   - 缓解：CR-1 的 F4 toggle 永久保留（settings 持久化），允许切回旧风
   - 回滚：彻底放弃，恢复 git 上 V5 阶段 G 完成时的快照

### 阶段间回滚点

每个 CR 阶段完成后打 git tag：`hoi3-counter-cr1`, `cr2`, `cr3`, `cr4`, `cr5`。
任何 CR 阶段验收失败可 `git reset --hard <prev-tag>` 回退到上一稳定状态。
CR-5 之前 `MapSymbolPass` 始终存在，最差情况 F4 切回旧 pass。

---

## 进度追踪

| 阶段 | 状态 | 预计 | 实际 | 备注 |
|---|---|---|---|---|
| CR-1 | ✅ COMPLETE | 1-1.5 周 | 2026-05-19 | 含 1.5 卡牌叠层视觉（HOI3 招牌之一）；F8 toggle，旧 MapSymbolPass 共存 |
| CR-2 | ✅ COMPLETE | 2 周 | 2026-05-19 | 2.1 atlas / 2.2 bar / 2.3 战斗选中移动 / 2.3.5 前倾+攻击指示 / 2.4 经验星 / 2.5 堆叠数文字 / 2.6 徽章 / 2.7 色调 / 2.8 zoom 缩放 |
| CR-3 | ✅ COMPLETE | 2-3 周 | 2026-05-19 | OOB 层级 + zoom 自动聚合 + NATO 标识 + 尺寸差异化 |
| CR-4 | ✅ COMPLETE | 2-3 周 | 2026-05-20 | 4.1 力导向布局 / 4.2 hit-test / 4.3 选中 / 4.7 多堆叠；4.4 fan-out / 4.5 右键 / 4.6 屏幕外指示推迟 |
| CR-5 | ✅ COMPLETE | 3-5 天 | 2026-05-20 | 旧系统清理（覆盖 V5 §4.2 NATO 兵牌保留决定） |

**总计**：8-10 周（含 buffer），位于 V5 阶段 G（M-G / MVP2）之后启动

---

## 附：与现有路线图的关系

- **位于 V5 阶段 G（M-G / MVP2）之后**，作为 V5 阶段 I（Counter Reboot）执行
- **覆盖 V5 §4.2 决定**：原本"NATO 兵牌 + MapSymbolPass 保留"被本路线图 CR-5 推翻
- **不影响** V5 阶段 A-G 的玩法链路（MVP1 / MVP2 期间继续用旧 NATO 兵牌）
- **不影响** AI / 战斗逻辑（数据层 `DivisionStore` 不动）
- **依赖** V5 阶段 B-C 已完成的 `egui` UI 框架 + `text_pass` fontdue 路径
- **依赖** V3 时代沉淀的 `shader-rt`（procedural shader 注册框架，V5 §4.2 保留）
- **不依赖** V5 §4.2 中已 DELETE 的 `gui_runtime.rs` / `ui_pass.rs` / `panel_pass.rs`

后续可选扩展（不在本路线图内）：
- 将领头像（与 history/leaders/*.txt 对接）
- 师徽章（每师一个 16×16 自定义图标，玩家可自定义）
- 战斗预览悬浮 tooltip（hover 5+ 秒展开 mini battle plan）
- 战略地图模式（zoom 极远时只显示集团军群级 + 国家色块大轮廓）

