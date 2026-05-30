# Project Ironheart — V9 前端重构设计文档

> 起草日期：2026-05-29
> 路线状态：研究稿，待用户签收后进入 Phase A。
> 上游：[`ROADMAP_V5.md`](ROADMAP_V5.md)（主线）
> 平行路线：[`SELF_GUI_REFACTOR_ROADMAP.md`](SELF_GUI_REFACTOR_ROADMAP.md)（DTO/Action 架构分层）、[`UI_PANEL_REWORK_ROADMAP.md`](UI_PANEL_REWORK_ROADMAP.md)（面板信息架构与文案）。
> 本文档与上述两条并行，**只负责视觉设计语言 + 布局规范 + 程序化美术**；不重复信息架构和数据流。

---

## 0. 决策摘要

| 维度 | 决策 | 含义 |
|---|---|---|
| 渲染底层 | **保留 egui** 作 tessellator / 字体光栅 / 输入路由 | 不需要从零实现 scroll/clip/IME/text input/拾取；继续吃 egui 0.31.1 的 GPU pipeline |
| 自研边界 | **布局层 + 视觉层 + 资产层完全自研**，egui 只做"画" | 禁用 `ui.horizontal/vertical` 自动流式排版；统一走 `GridLayout` |
| 美术资产 | **近期 100% 程序化生成**（SDF + 噪声 + 渐变） | 无外包/无 vanilla 资产；后期保留接入原创贴图库的能力 |
| 视觉锚点 | **WW2 主体 + HOI4 vanilla 工艺感 + KR/TNO 调性 + 一点冷战质感** | 暖金 + 木纹 + 黄铜边 + 羊皮纸 + 钢灰 + 信号红点缀 |
| 重构范围 | **42 个面板全部重做** + 主菜单 + 国家选择 + 顶栏 + HUD + 弹窗 + 通知 | 一次性切换到 V9 视觉语言；旧风格不留 |
| 兼容窗口 | feature flag `visual_v2`，灰度开启 | 编译期可切回旧视觉验证回归 |

---

## 1. 现状审计与问题归因

### 1.1 现状

| 层 | 实现 | 状态 |
|---|---|---|
| 主菜单 / 国家选择 / 加载页 | `crates/hoi4-app/src/menu_pass.rs`（669 行）+ `panel_pass.rs`（435 行 SDF 圆角矩形）+ `text_pass.rs`（794 行 fontdue） | 已自研，不走 egui；视觉接近"Civ VI 启动器"风格但缺乏统一规范 |
| 顶栏 | `crates/hoi4-ui/src/topbar.rs`（442 行）走 egui + `allocate_exact_size` 自绘 tile | 当前最接近"目标视觉"的面板，但尺寸常量散落 |
| 42 个 in-game 面板 | `crates/hoi4-ui/src/*.rs` 走 egui `Frame::show` + `ui.horizontal/vertical` | 视觉混杂；尺寸由 egui 决定，宽窄变化时常错位 |
| 共享组件 | `crates/hoi4-ui/src/components.rs`（430 行） | 提供 `section`/`metric_tile`/`tab_chip`/`zebra_row` 等，但调用方仍可绕过直接写 `Frame` |
| 主题 | `crates/hoi4-ui/src/theme.rs`（181 行） | 字体 + visuals 已配置；调色板与 `components.rs` 各持一份，长期会漂 |

### 1.2 错位根因（按出现频率）

| # | 根因 | 触发场景 | V9 对策 |
|---|---|---|---|
| R1 | `ui.horizontal()` 流式布局，子项之和超出容器后换行 | 顶栏 / 卡片 row / 状态条 | 改用 `GridLayout::cell(row, col, span)` 显式坐标 |
| R2 | `Label::wrap()` 在窄容器中拉高 | 长标题 / 长描述 | 显式 `Label::truncate()` + tooltip 完整文本 |
| R3 | `available_width()` 弹性分配，邻居加内容时反向影响 | 列表行的右侧值 | 列宽从 token 表读，不依赖 `available_width` |
| R4 | `inner_margin` / `Spacing` 各处不一致 | 全局 | `Spacing` token 表，禁用裸数字 |
| R5 | 字体度量在 CJK fallback 时不同，按钮高度漂移 | 中文按钮 | `Button` 强制 `min_size` 来自 token，文字 `truncate` 不参与高度 |
| R6 | DPI 缩放下圆角和描边亚像素抖动 | 高 DPI 屏 | 像素吸附（`round_to_pixels`）+ token 中所有尺寸为整数 |
| R7 | `Window` 默认 `resizable + auto_size` | 部分弹窗 | 全部 `fixed_size` + `anchor` |
| R8 | 颜色硬编码（`Color32::from_rgb(0x..)`）不走 token | 各文件 | clippy lint 禁用裸 Color32，必须经 `tokens::Palette` |

### 1.3 设计语言短板

- **没有"游戏感"的边框**：现在是矩形 + 1px 描边，缺乏"金属边框 / 黄铜钉头 / 雕花角"。
- **没有质感纹理**：底色是纯色，缺乏木纹 / 羊皮纸 / 噪声做底。
- **字号档位混乱**：`size(20.0)` / `size(14.0)` / `size(13.0)` / `size(12.0)` / `size(11.0)` / `size(10.0)` 6 档无明确语义。
- **强调色单一**：只有 `GOLD_BRIGHT` 一种高亮；状态色（成功/警告/危险/中性）和叙事色（轴心/同盟/共产/中立/冷战派系）混在同一份调色板里。
- **缺乏叙事载体**：KR/TNO 有"事件卡 / 国策标语 / 派系横幅"，现版本只有矩形 panel 列表。

---

## 2. V9 自研前端架构

### 2.1 三层结构

```
┌─────────────────────────────────────────────────────────────┐
│  Composites（屏幕 / 面板 / 模态）                            │
│  - MainMenuScreen / CountrySelectScreen                     │
│  - TopBar / SideRail / MiniMapHUD / NotificationStack       │
│  - PoliticsPanel / MarketPanel / ... (42 个)                │
│  - EventModal / Tooltip / Toast / Banner                    │
└─────────────────────────────────────────────────────────────┘
                            ▲
┌─────────────────────────────────────────────────────────────┐
│  Primitives（视觉元件 + 布局元件）                            │
│  - PanelFrame / Card / Tile / Pill / Ribbon                 │
│  - Tabs / List / DataTable / Tree / Modal                   │
│  - Button / Input / Toggle / Slider / Dropdown / Progress   │
│  - PortraitFrame / FlagFrame / CounterIcon                  │
│  - GridLayout / SplitLayout / StackLayout / AnchorLayout    │
└─────────────────────────────────────────────────────────────┘
                            ▲
┌─────────────────────────────────────────────────────────────┐
│  Tokens（设计常量）+ Procedural Art（程序化美术）              │
│  - Palette / Typography / Spacing / Radius / Border / Elev  │
│  - Motion (duration + easing)                               │
│  - FrameStyle / TextureProc / OrnamentProc                  │
│  - Z-Order layers                                           │
└─────────────────────────────────────────────────────────────┘
                            ▲
┌─────────────────────────────────────────────────────────────┐
│  egui 0.31.1（保留作 tessellator / 字体光栅 / 输入路由）       │
│  - 不再使用 ui.horizontal / ui.vertical 流式排版             │
│  - 只用 Painter 原语 + Sense::click 拾取                    │
└─────────────────────────────────────────────────────────────┘
```

### 2.2 模块分布

```
crates/hoi4-ui/src/
  v9/
    mod.rs              # 入口，暴露 tokens + primitives + composites
    tokens.rs           # Palette / Typography / Spacing / Radius / Border / Elev / Motion / ZLayer
    layout.rs           # GridLayout / SplitLayout / StackLayout / AnchorLayout / px helpers
    paint.rs            # 程序化绘制原语：wood_grain / parchment / metal / rivet / ornament
    frame.rs            # PanelFrame + FrameStyle enum
    text.rs             # 文本档位封装（TextRole = Title/Heading/Body/Caption/Numeric/Code）
    primitives/
      button.rs
      card.rs
      tile.rs
      tabs.rs
      list.rs
      table.rs
      tree.rs
      modal.rs
      tooltip.rs
      toast.rs
      pill.rs
      ribbon.rs
      progress.rs
      slider.rs
      toggle.rs
      dropdown.rs
      input.rs
      empty.rs
      portrait.rs
      flag.rs
      counter.rs
    composites/         # 由 primitives 拼装，单一职责屏幕级组件
      topbar.rs
      side_rail.rs
      mini_map_hud.rs
      main_menu.rs
      country_select.rs
      panel_shell.rs    # 标准面板外壳：header + summary + tabs + body + footer
      modal_event.rs
      notification_stack.rs
crates/hoi4-app/src/menu_pass_v2.rs    # 主菜单/国选 panel_pass 路径迁到 v9 tokens
crates/hoi4-ui/src/panels/             # 42 个面板按 PanelShell 重写
```

### 2.3 与既有路线的衔接

- **SELF_GUI_REFACTOR_ROADMAP（DTO/Action 分层）**：V9 仅替换 `hoi4-ui` 的视觉层；面板仍接收 `XxxPanelData`、返回 `XxxAction`，迁移期可与 DTO 化并行推进。
- **UI_PANEL_REWORK_ROADMAP（面板信息架构）**：V9 提供新的 `PanelShell` 外壳，旧面板已完成的 G0-G6 信息架构升级直接装进新外壳，不重写业务逻辑。
- **V6/V7/V8 经济内容**：本路线不动业务模型，只动 `pop_panel / market_panel / construction_v6_panel` 等的视觉外壳。

---

## 3. Design Tokens（设计常量）

### 3.1 调色板（KR/TNO + WW2 + 一点冷战）

```
基础（中性 / 木纹 / 羊皮纸）
  CANVAS_DEEP    #110a06   // 最底（窗口外背景）
  CANVAS         #1a1109   // 主背景（HUD 底）
  PANEL          #241a12   // 面板底
  PANEL_SOFT     #2e2218   // 面板悬浮 / 卡片
  PANEL_DEEP     #1a120c   // 凹槽（输入框 / progress 空槽）
  ZEBRA_DARK     #281d14   // 列表斑马深行
  ZEBRA_LIGHT    #2e2218   // 列表斑马浅行
  HAIRLINE       #3a2c20   // 1px 分隔线
  STROKE_DARK    #5a442c   // 1.5px 描边
  STROKE_MED     #6e5634   // 中线
  PARCHMENT      #e0d2a8   // 主文本（羊皮纸）
  PARCHMENT_DIM  #b0a285   // 次文本
  MUTED          #9b9b9b   // 灰文本（数据标签）

主调（黄铜 / 暖金 — KR/TNO 调性）
  BRASS_DARK     #5a442c   // 黄铜深
  BRASS          #8b6f3e   // 黄铜
  BRASS_BRIGHT   #c9a55b   // 黄铜亮（标题色）
  GOLD           #e0c078   // 强调金（hover / 选中边）
  GOLD_HOT       #f4d68b   // 高亮金（焦点 / active）

派系（HOI4 vanilla 政治色 — 略微提暖）
  IDEO_FASCISM      #9e3232   // 法西斯
  IDEO_DEMOCRATIC   #2e60a8   // 民主
  IDEO_COMMUNISM    #b02a2a   // 共产
  IDEO_NEUTRALITY   #c08a3e   // 中立 / 非阵营

冷战派系点缀（V9 留口 — TNO 调性）
  COLD_STEEL     #5a6675   // 钢灰
  COLD_SIGNAL    #d04030   // 信号红（警报 / 战时模式）
  COLD_ATOMIC    #2cb4a8   // 原子青（科研突破点缀 — 节制使用）

状态（语义色 — 不与派系混用）
  GOOD           #6cc070   // 盈余 / 充足 / 完成
  WARN           #f0b850   // 注意 / 缺料 / 即将完成
  BAD            #d8584c   // 短缺 / 亏损 / 失败
  INFO           #6896c8   // 提示 / 中性信息

数值色（图表与统计专用）
  STAT_PRIMARY    #c9a55b  // 主统计（GDP / 人力等）
  STAT_SECONDARY  #6896c8  // 次统计（增速 / 对比）
  STAT_TERTIARY   #8a8a8a  // 灰
```

### 3.2 字体与字号档（TextRole）

| Role | 字号 | 字重 | 字族 | 用途 | 示例 |
|---|---:|---|---|---|---|
| `Title` | 32 | Bold | Serif | 主菜单大标题 | `HEARTS OF IRON IV` |
| `Display` | 22 | Bold | Serif | 面板标题 / 模态标题 | `政治` |
| `Heading` | 17 | SemiBold | Serif | 章节标题 / 摘要数值 | `军费` |
| `Subheading` | 14 | SemiBold | Sans | tab 标题 / 列表标题 | `钢铁厂` |
| `Body` | 12 | Regular | Sans | 正文 / 列表行 | `83.4M RM/周` |
| `Caption` | 11 | Regular | Sans | 次说明 / 单位 | `RM/周` |
| `Small` | 10 | Regular | Sans | 角标 / debug | `Lv 42` |
| `Numeric` | 13 | Medium | **Mono** | 表格数值 | `+12.4 M` |
| `Code` | 11 | Regular | **Mono** | 调试 ID / 日志 | `GFX_flag_SPR` |

> **关键规则**：禁止在调用方直接写 `RichText::size(...)`；必须通过 `tokens::text(TextRole::Body)`。

字体族：
- Serif = Georgia / Palatino / Times（既有 `theme.rs::latin_candidates`）
- Sans = 系统默认（egui 自带 Ubuntu-Light）
- Mono = Consolas / Cascadia Code（新增）
- CJK fallback：Microsoft YaHei / PingFang / Noto Sans CJK（既有）

### 3.3 间距 / 圆角 / 描边 / 海拔

```
Spacing scale（用 token 名称，不裸写）
  s0  =  0
  s1  =  2
  s2  =  4
  s3  =  6
  s4  =  8
  s5  =  12
  s6  =  16
  s7  =  24
  s8  =  32
  s9  =  48
  s10 =  64

Radius scale
  r0 = 0  (硬边 — 大多数 in-game 面板)
  r1 = 2  (按钮 / chip)
  r2 = 4  (卡片)
  r3 = 6  (模态)
  r4 = 8  (主菜单大面板)

Border scale
  b0 = 0
  b1 = 1     (hairline)
  b2 = 1.5   (主边框)
  b3 = 2     (强调 / focus)
  b4 = 3     (顶栏底部金线 / hero divider)

Elevation（用 shadow alpha 表达，shadow_offset = elev * 1.5）
  e0 = 0      (扁平 — 嵌入式)
  e1 = 0.18   (轻浮 — 卡片)
  e2 = 0.32   (中浮 — 面板)
  e3 = 0.48   (高浮 — 模态 / 弹窗)
  e4 = 0.62   (最高 — 全屏弹窗 / 错误对话框)
```

### 3.4 Motion

```
Duration
  fast    = 80ms   (hover / press)
  normal  = 160ms  (tabs / 展开)
  slow    = 280ms  (模态进入 / 通知)
  cinema  = 480ms  (主菜单转场)

Easing
  ease_out      // 默认（hover / 展开）
  ease_in_out   // 模态 / 转场
  ease_back     // 通知"弹入"
  linear        // 进度条
```

### 3.5 Z-Order

```
Layer                      z
─────────────────────────────────
背景地图 / 3D scene         0
HUD overlay (顶栏 / 侧栏)   100
面板 (Politics / Market)   200
Tooltip                    300
模态对话框                 400
事件弹窗 (强制)            500
通知 toast                 600
全屏遮罩 (loading / menu)  700
调试 overlay               1000
```

---

## 4. 布局系统（核心反"egui 决定尺寸"）

### 4.1 GridLayout

```rust
pub struct GridLayout {
    rows: Vec<Track>,    // 每行高度（固定或弹性，但总和必须等于容器高度）
    cols: Vec<Track>,    // 每列宽度
    gutter_x: f32,
    gutter_y: f32,
}

pub enum Track {
    Fixed(f32),       // 像素
    Fr(f32),          // 弹性比例（在容器测量后展开为像素）
}

impl GridLayout {
    pub fn measure(&self, container: Rect) -> Vec<Vec<Rect>>;
    pub fn cell(&self, layout: &[Vec<Rect>], row: usize, col: usize) -> Rect;
    pub fn span(&self, layout: &[Vec<Rect>], row: usize, col: usize, rowspan: usize, colspan: usize) -> Rect;
}
```

**约束**：
- `GridLayout::measure` 在面板进入时调用**一次**，结果缓存到 `PanelLayoutCache`。
- 子项绘制走 `ui.allocate_rect(cell)` 拿到 `Painter`，不再走 `ui.horizontal/vertical`。
- 子项内部如果还要再分，再开一个 `GridLayout`。

### 4.2 其他布局元件

| 元件 | 用途 | 关键约束 |
|---|---|---|
| `SplitLayout` | 左右分栏 / 上下分栏 | 比例或固定，**支持拖动调整**（但拖动结果落到 token 之间） |
| `StackLayout` | 垂直堆叠等距列表 | 仅适合简单详情列表 |
| `AnchorLayout` | 模态对话框 / 通知 | `anchor: TopRight / BottomLeft / Center`，相对屏幕或父容器 |
| `FlowLayout` | **禁用**（仅 chip 列表合规使用） | 包含规则：所有子项尺寸来自 token，不依赖文字长度 |

### 4.3 断点

```
xs:   1280 × 720    (最低支持)
md:   1600 × 900    (推荐)
lg:   1920 × 1080
xl:   2560 × 1440+

策略：
  - 顶栏 tile 数量与宽度跟随断点（xs 折叠 GDP/增速到二级 tooltip）
  - 国家选择 detail 栏的 stat block 列数跟随断点
  - 标准面板 PanelShell 的 list+detail 比例固定 38/62（不随屏幕变）
  - 模态最大宽度 = 容器 × 0.66，最大高度 = 容器 × 0.80
```

### 4.4 DPI / 像素吸附

- 所有 token 中的尺寸单位是**逻辑像素**，乘以 `ctx.pixels_per_point()` 后做 `.round()` 到物理像素。
- 1px 描边在 1.25x DPI 下走 `1.0 * 1.25 = 1.25 → round → 1`，避免亚像素抖动。
- 圆角统一用 SDF 抗锯齿，不用 egui 自带的多边形圆角（在小半径下不稳定）。

### 4.5 禁用 / 必用 模式

```
✘ 禁用（lint warn）：
  - ui.horizontal_wrapped(|ui| ...)
  - ui.add_sized(vec2(0.0, ...), ...)     // 0 宽度 = 让 egui 决定
  - egui::ScrollArea::auto_shrink         // 默认 [true, false]，会让父级翻车
  - Frame::new().inner_margin(裸数字)     // 必须 inner_margin(spacing::s5)
  - Color32::from_rgb(...)                // 必须 palette::xxx
  - RichText::size(...)                   // 必须 text(TextRole::xxx)
  - Button::new(...)（裸）                 // 必须 Button::new(...).min_size(button::size(Size::Md))

✔ 必用：
  - GridLayout::cell(...) 划定矩形
  - PanelFrame::draw(painter, rect, FrameStyle::Card, &tokens)
  - text::draw(painter, rect, TextRole::Body, &content)
  - ScrollArea::vertical().auto_shrink([false, false]).show_rows(...)  // 虚拟化
```

> Phase A 不强制 lint，Phase B 起加 clippy 自定义规则（或 `cargo deny` 文本检查）。

---

## 5. 程序化美术管线

### 5.1 FrameStyle 枚举

| Style | 用途 | 程序化构成 |
|---|---|---|
| `Card` | 卡片 / 列表行 / 子区块 | 软底色 + 1px hairline + 极淡内阴影 |
| `Panel` | 标准面板 | 深木纹底（FBM noise）+ 1.5px 黄铜边 + 角部 ◆ 小标记 |
| `Ornate` | 主面板 / 强调区 | 木纹底 + 2px 黄铜边 + 四角铜钉头（SDF 圆）+ 顶部金线 |
| `Glass` | HUD / 半透浮层 | 半透暗底 + 1px 亮金边 + 顶部高光线 |
| `Modal` | 模态 / 事件弹窗 | 深色重底 + 2px 双线边（外铜内细金）+ 顶部 ribbon 区 + 阴影 e3 |
| `Ribbon` | 横幅 / 通知 / 派系标语 | 单色实底 + 左侧三角切角 + 右侧菱形端头 + 派系强调色 |
| `Hero` | 国选 / 主菜单大块 | 渐变（top deep → bottom darker）+ 顶底金色 hairline 双线 |

实现路径：所有 FrameStyle 由 `paint.rs::draw_frame(painter, rect, style, palette)` 一个函数分支处理，绘制顺序：
1. 阴影（`shadow_alpha` 控制）
2. 底纹（纯色或程序化纹理）
3. 边框（1-3 层 stroke）
4. 角部装饰（◆ / 铜钉 / 切角）
5. 内部高光（顶部 1px 亮线）

### 5.2 程序化纹理

| Texture | 实现 | 性能 |
|---|---|---|
| 木纹（panel 底） | 在 panel_pass shader 用 2-band FBM noise + horizontal warp，颜色映射到 `PANEL ↔ PANEL_SOFT` | 单 panel 单 quad，shader 内 16 octaves，<0.05ms/panel |
| 羊皮纸（modal 底） | low-freq FBM + 略微泛黄渐变 | 同上 |
| 黄铜金属（边框） | 径向亮度梯度 + 微噪声扰动 | 仅在边框区采样，可忽略 |
| 铜钉头（角部） | 单 SDF 圆 + 高光点 | 单像素级别 |
| 抓痕（worn 状态） | 高频低对比 noise overlay | 可选，开关在 settings |
| HDR 玻璃 | 半透 + 顶部 1px 亮线 + 内阴影 | egui Painter 即可 |

> 由于 egui Painter 不支持自定义 shader（每个 Rect 是 Mesh），程序化纹理用以下两种方式：
> - **小尺寸**（≤256px）：CPU 端预生成 1 张纹理（启动时构建），缓存为 `egui::TextureId`。
> - **大尺寸 / 大量 panel**：迁到 `panel_pass`，由 SDF shader 程序化生成（已有管线）。

### 5.3 角部装饰与分隔

```
角部装饰库
  ◆ (rosette)        - 主面板 / hero 标题旁
  ✦ (small star)     - 章节标题旁
  ▣ (filled square)  - tab 激活态
  ❖ (filigree)       - 模态标题
  ❘ (vertical bar)   - 派系强调色短条（用作徽章）
  ╳ (chevron)        - 进度尖头
  〤 (compass)        - 外交 / 地图相关

分隔库
  ─── 单 hairline
  ━━━ 双线（细+粗）
  ─◆─ 居中 ◆ + 两侧 hairline（hero 分隔）
  ───━━━─── 主从分隔（hero header 下方）

铜钉头（rivet）
  - SDF 圆，半径 r1=2
  - 主体色 BRASS_BRIGHT
  - 中心 1px 高光 GOLD_HOT
  - 1px 暗影偏移到右下
  - 用于 Ornate 四角、模态四角、Ribbon 两端
```

---

## 6. 组件库（重做后 30 个 primitives）

### 6.1 PanelFrame（最常用）

```rust
pub struct PanelFrame {
    pub style: FrameStyle,      // Card / Panel / Ornate / Glass / Modal / Ribbon / Hero
    pub rect: Rect,             // 来自 GridLayout::cell
    pub accent: Option<Color>,  // 派系色 / 状态色（用于 Ribbon / 顶部金线染色）
    pub inset: Margin,          // 内边距，必须来自 tokens::spacing
}

impl PanelFrame {
    pub fn draw(&self, painter: &Painter, tokens: &Tokens);
    pub fn inner_rect(&self, tokens: &Tokens) -> Rect;  // 扣除边框/inset 后的可用区
}
```

### 6.2 完整清单

| Primitive | 描述 | FrameStyle | 关键尺寸 token |
|---|---|---|---|
| **Button** | 主/次/危险/禁用 4 态 × 3 尺寸 (sm/md/lg) | Card | sm=24, md=32, lg=44 高 |
| **IconButton** | 图标按钮（关闭/最小化/打开） | (无) | 28×28 |
| **Card** | 简单卡片容器 | Card | 任意 |
| **Tile** | KPI 数值瓦片（label + 大数值 + 强调色条） | Card | 78 宽，可弹性高 |
| **HeroKpi** | hero 顶部居中 KPI | Card | 较 Tile 略大 |
| **Pill** | 状态胶囊（短缺/亏损/锁定） | (无 — 单色块) | 高 18 |
| **Badge** | 数字角标 | (无) | 16 |
| **Ribbon** | 派系横幅 / 派系标语 | Ribbon | 高 28 |
| **Tabs** | 顶部标签栏 | Card | 高 30，激活下划线 b3 |
| **Subtabs** | 子标签（chip 风） | (无) | 高 22 |
| **List** | 虚拟化列表（zebra） | (无外框) | 行高 28 |
| **DataTable** | 表格（固定列宽） | Card | 表头 26，行 22 |
| **Tree** | 树形（科技 / 国策 / 决议） | (无) | 节点 32×32 |
| **Modal** | 模态对话框 | Modal | 宽 660 默认 / 高自适应到 0.8 屏 |
| **Tooltip** | 悬浮提示 | Modal (e2) | 最大宽 360 |
| **Toast** | 通知 toast | Card (e2) | 320×56，stack 右下 |
| **EmptyState** | 空数据 | (无外框) | 居中 |
| **ProgressBar** | 进度条 | (无) | 高 8 |
| **ProgressRing** | 圆形进度（建造 / 焦点） | (无) | 32×32 |
| **Slider** | 滑动条 | (无) | 高 24 |
| **Toggle** | 开关 | (无) | 36×20 |
| **Checkbox** | 复选 | (无) | 18×18 |
| **Radio** | 单选 | (无) | 18×18 |
| **Dropdown** | 下拉 | Card | 高 28 |
| **TextInput** | 文本输入 / 搜索 | Card (凹) | 高 26 |
| **NumberInput** | 数字输入 + 加减 | Card (凹) | 高 26 |
| **PortraitFrame** | 领导人头像框 | Ornate | 96×120（大）/ 48×60（小） |
| **FlagFrame** | 国旗框（带派系色边） | Card | 82×52 (原比例 4×) / 41×26 (1×) |
| **CounterIcon** | NATO 师徽（既有 nato_icon.rs） | (无) | 32×24 |
| **MapHoverPanel** | 鼠标悬停省份卡 | Glass | 240 宽 |

### 6.3 组件使用规则

- 每个 primitive 必须暴露 **fixed-size 构造**（`Button::new("xx").size(Size::Md)`），不接受调用方传入 `vec2`。
- 每个 primitive 必须暴露 **single-shot draw**（`button.show(ui, rect) -> Response`），不写 `ui.add(button)`。
- 每个 primitive 必须有 **disabled / hovered / active** 三态 token 化样式。
- Composites 不允许直接调用 `Painter`，必须经过 primitives。

---

## 7. 屏幕模板（Composites）

### 7.1 主菜单 — MainMenuScreen

```
┌──────────────────────────────────────────────────────────────┐
│ ░░░ 全屏背景图（动态 / 静态二选一）+ 0.55 暗色蒙版 ░░░          │
│                                                              │
│         ┌────────────────────────────────────────┐           │
│         │ ◆                                   ◆ │           │
│         │     HEARTS OF IRON IV                  │           │
│         │     ─────────────                      │           │
│         │     Project Ironheart Engine           │           │
│         │     1936 战役 · 自研引擎               │           │
│         │                                        │           │
│         │   ▣ ▸ 新游戏                           │           │
│         │   ▣   继续游戏（灰）                   │           │
│         │   ▣   设置                             │           │
│         │   ▣   退出                             │           │
│         │                                        │           │
│         │     ENTER 进入   ESC 退出              │           │
│         │ ◆                                   ◆ │           │
│         └────────────────────────────────────────┘           │
│  v0.9.0-V9 · build 2026-05-29                                │
└──────────────────────────────────────────────────────────────┘
```

- 走 `panel_pass`（菜单已经在用）。
- 面板 `FrameStyle::Hero`，固定 480×540，居中略偏上（屏幕高 × 0.48）。
- 四角 ◆ rosette。
- 按钮 hover 时左侧出现 5px 黄铜亮条 + `▸` 字符前缀，整行不平移（避免抖动）。
- 顶/底 letterbox 34px / 40px，渲染时强制对齐到物理像素。

### 7.2 国家选择 — CountrySelectScreen

```
┌──────────────────────────────────────────────────────────────┐
│ ░░░ 背景：选中国家的派系色 + 6% 比例 + 暗色蒙版 ░░░             │
│                                                              │
│           选择国家                                            │
│           ─────────                                          │
│  [搜索 ▢]                       [筛选: 派系▼  人力▼  工业▼]    │
│                                                              │
│ ┌─────────────┐ ┌──────────────┐ ┌──────────────────────────┐│
│ │ 国家         │ │              │ │ 概览                     ││
│ │ ━━━          │ │   ┌──────┐   │ │ ━━━                      ││
│ │ ▮ DEU 德意志 │ │   │      │   │ │ 德意志国 1936             ││
│ │ ▮ SPR 西班牙 │ │   │ 国旗 │   │ │ Tag DEU  ·  独裁           ││
│ │   FRA 法兰西 │ │   │      │   │ │ 派系 法西斯                ││
│ │   GBR 大英 │ │   └──────┘   │ │                          ││
│ │   USA 美利坚 │ │              │ │ ◆ 人力                    ││
│ │   SOV 苏联   │ │ 德意志国     │ │   总池  68.0M             ││
│ │   ...        │ │ DEU          │ │   服役  240k              ││
│ │              │ │              │ │                          ││
│ │              │ │ ━━━━━━━     │ │ ◆ 工业                    ││
│ │              │ │ 法西斯       │ │   民用  32              ││
│ │              │ │              │ │   军工  18              ││
│ │              │ │              │ │   船坞   3              ││
│ │              │ │              │ │                          ││
│ │              │ │              │ │ ◆ 政治                    ││
│ │              │ │              │ │   稳定  73%               ││
│ │              │ │              │ │   战支  31%               ││
│ └─────────────┘ └──────────────┘ └──────────────────────────┘│
│                                                              │
│      [ 返回 ]                          [ 开始游戏 ▸ ]          │
│  ↑↓ 浏览  ENTER 确认  ESC 返回                                │
└──────────────────────────────────────────────────────────────┘
```

- 走 `panel_pass`。
- 三栏比例 28/32/40，固定宽度（不随屏幕弹性，仅整体居中），最大宽度 1400px。
- 列表行高 32（>= 4px gutter 上下），左侧派系 4px 条 + tag + 国名 + 锁定标记。
- 国旗框 4× 放大，外加 6px 凹槽 + 黄铜 2px 边 + 四角铜钉。
- 详情栏走 "◆ 标题 + 多行 KV 对" 的章节模式。
- 底部按钮固定 200×44，距详情栏底部 24px。

### 7.3 顶栏 — TopBar

```
┌──────────────────────────────────────────────────────────────────────────────────────────┐
│ [国旗88×52] [PP 220] [稳定 73%] [战支 31%] [人力 68M] [GDP £58B] [增速 +1.8%] [建造 240]   │
│                                                                                          │
│                                   1936 年 1 月 1 日                                       │
│                                                                                          │
│ [■] [Ⅰ] [Ⅱ] [Ⅲ] [Ⅳ] [Ⅴ]                                                                  │
└──────────────────────────────────────────────────────────────────────────────────────────┘
```

- 走 egui（已实现 70%）。
- 高度固定 72px（含 4px 顶部金线）。
- 国旗 plate 92×60（含 ridge + 内国旗 82×52）。
- Tile 高 58 + 横向 4px gutter。每个 tile **固定 88 宽**（不再让 egui 决定）。
- 日期 block 居中绝对定位（屏幕中线 ± 100px）。
- 速度按钮 6 个，统一 58×58。

### 7.4 标准面板 — PanelShell

```
┌─ Header ─────────────────────────────────────────────── ✕ ─┐
│ ◆ 政治                  德意志国 1936-01-01    [搜索 ▢]     │
├─ Summary Strip ────────────────────────────────────────────┤
│ [稳定 73%] [战支 31%] [PP 220] [国策 1/1] [决议 2/8]         │
├─ Tabs ─────────────────────────────────────────────────────┤
│  总览  |  国策  |  决议  |  顾问  |  议会  |  问题优先       │
├─ Body ─────────────────────────────────────────────────────┤
│ ┌── 左列 (38%) ──────────┐ ┌── 右列 (62%) ──────────────┐  │
│ │ 列表（zebra 行）          │ │ 选中详情                   │  │
│ │ - 项目 A                │ │ ◆ 标题                     │  │
│ │ - 项目 B（选中）         │ │ 描述...                    │  │
│ │ - 项目 C                │ │ ◆ 影响预览                  │  │
│ │ - 项目 D                │ │ + 稳定 +5%                  │  │
│ │ ...                     │ │ - PP -50                    │  │
│ │                        │ │ ◆ 操作                      │  │
│ │                        │ │ [ 激活 ] [ 取消 ]            │  │
│ └─────────────────────────┘ └────────────────────────────┘  │
├─ Footer ───────────────────────────────────────────────────┤
│ 状态：可激活                       Q 关闭  Tab 切换         │
└────────────────────────────────────────────────────────────┘
```

- 走 egui。
- 面板宽度根据 panel 类型固定：
  - 经济类（市场 / 财政 / 贸易）= 1080
  - 军政外交（政治 / 外交 / 军事）= 960
  - 详情型（科研 / 国策树 / 法律）= 1200
  - 简单设置（设置 / 存档）= 720
- 面板高度 = 屏幕高 × 0.78（最大 920，最小 540）。
- 锚定：屏幕中心（除非用户拖到一边，结果不持久化跨会话）。
- 5 个固定 grid 行：Header 48 + Summary 56 + Tabs 36 + Body fr1 + Footer 32。
- Body grid 列：list 38% / divider 1 / detail 62%。

### 7.5 模态事件弹窗

```
                  ┌────────────────────────────────┐
                  │ ❖ 重大事件 — 镇压风潮            │
                  ├────────────────────────────────┤
                  │  ┌──────┐                       │
                  │  │      │   1936-03-15           │
                  │  │ 头像 │                       │
                  │  │      │   元首决定...            │
                  │  └──────┘                       │
                  │                                │
                  │  长描述文字（最多 6 行，超出 ... + tooltip）│
                  │                                │
                  ├────────────────────────────────┤
                  │ ▸ 选项 A  (+稳定 5  −PP 30)     │
                  │   选项 B  (+战支 2  −稳定 3)    │
                  │   选项 C  (无效果)              │
                  └────────────────────────────────┘
```

- `FrameStyle::Modal` + 上方 28px Ribbon 区。
- 弹窗居中，固定 720×自适应（最大 0.8 屏高）。
- 阴影 e3 + 背景 0.45 暗蒙。
- 选项按 EmptyState 列表样式，hover 时整行高亮。

### 7.6 通知 Toast

- 右下角 stack。
- 每条 320×56，FrameStyle::Card。
- 左侧 4px 派系/状态色条。
- 顶部小字标题 + 下方 1-2 行正文。
- 4s 自动消失（除非 hover 暂停）。
- 进入动画：`ease_back` + 滑入 16px。

---

## 8. 实施阶段

### Phase A：地基（2 周）

目标：tokens + layout + 5 个核心 primitive，跑通 demo 页验证。

| 任务 | 文件 | 验收 |
|---|---|---|
| A.1 | `tokens.rs` 完整调色板 / 字号 / spacing / radius / border / elev / motion | `cargo doc` 全字段可见 |
| A.2 | `layout.rs::GridLayout` + `SplitLayout` + `AnchorLayout` | 单元测试覆盖：固定行高、Fr 弹性、gutter 计算 |
| A.3 | `paint.rs::draw_frame` 全 7 种 FrameStyle | 一张 demo 页同屏展示 7 种外框 |
| A.4 | `frame.rs::PanelFrame` + `text.rs::TextRole` | demo 页同屏展示 9 种 TextRole |
| A.5 | `primitives/{button, card, tile, tabs, list}.rs` | demo 页跑通 hover/active/disabled |
| A.6 | `crates/hoi4-ui/src/v9/demo.rs` | F12 切到 v9 demo，展示所有 primitive |

### Phase B：菜单 + 顶栏（1.5 周）

目标：玩家进入游戏前 30 秒接触的全部视觉。

| 任务 | 文件 | 验收 |
|---|---|---|
| B.1 | `menu_pass_v2.rs` 重做主菜单（迁到 v9 tokens） | 视觉对照 §7.1 mockup |
| B.2 | 国家选择重做（panel_pass 路径，三栏） | 视觉对照 §7.2 |
| B.3 | 顶栏迁到 v9 tokens + GridLayout 锁死宽度 | 1280 / 1920 / 2560 三档断点测试 |
| B.4 | 顶栏 tile 增加 hover tooltip（解释字段） | 鼠标悬停 PP tile 显示 "政治力 / 来源 / 月增" |

### Phase C：HUD + 弹窗系统（1.5 周）

| 任务 | 文件 | 验收 |
|---|---|---|
| C.1 | `composites/mini_map_hud.rs`（右下角小地图 + 时间控制副本） | 视觉一致 |
| C.2 | `composites/side_rail.rs`（左侧快捷面板入口） | 15 个现役面板入口 + 通知点 |
| C.3 | `primitives/modal.rs` + `composites/modal_event.rs` | 事件弹窗替换 `event_panel.rs` 外壳 |
| C.4 | `primitives/tooltip.rs`（地图省份悬停 + UI 通用 tooltip） | 走 v9 视觉 |
| C.5 | `composites/notification_stack.rs` + `primitives/toast.rs` | 4s 自动消失，hover 暂停 |

### Phase D：SPR/GER 内容面板（2 周）

> 优先重做 SPR/GER 演示路径会用到的面板（来自记忆 [[tag-content-coverage]]）。

| 任务 | 涉及面板 | 关键改动 |
|---|---|---|
| D.1 | `politics` | PanelShell 外壳 + 顾问槽 PortraitFrame 真实头像 |
| D.2 | `decisions_panel` | tab 列表 → 卡片网格 + 影响预览侧栏 |
| D.3 | `event_panel` | 用 C.3 的 modal_event 模板 |
| D.4 | `diplomacy` | 国家详情 panel + FlagFrame + 派系徽章 |
| D.5 | `situation_panel` | 卡片化局势 + 阶段进度 ProgressRing |
| D.6 | `focus_tree_panel` | TreeLayout + 节点 32×32 + 状态色描边 |
| D.7 | `country_info_panel` | 走 PanelShell 简化版 |

### Phase E：经济面板（2 周）

| 状态 | 任务 | 涉及面板 | 关键改动 |
|---|---|---|---|
| [x] | E.1 | `construction_v6_panel` | PanelShell + DataTable + Tile 摘要 |
| [x] | E.2 | `market_panel` | DataTable + 短缺 Pill + 商品详情侧栏 |
| [x] | E.3 | `finance_panel` | Tile 网格 + 收支 DataTable |
| [x] | E.4 | `trade_panel` | 路线 List + 影响 DataTable |
| [x] | E.5 | `logistics_panel` | 装备 DataTable + 缺口 ProgressBar |
| [x] | E.6 | `law_panel` | 法律组 List + 切换预览 ImpactPreview |
| [x] | E.7 | `research` | TreeLayout + 详情 ImpactPreview |
| [x] | E.8 | `pop_panel` / `pyatiletka_panel` | PanelShell 标准化 |

### Phase F：军事面板（1.5 周）

| 状态 | 任务 | 涉及面板 | 关键改动 |
|---|---|---|---|
| [x] | F.1 | `military` | 军团 List + 师 DataTable + 命令 Ribbon |
| [x] | F.2 | `army_detail_panel` | PanelShell + CounterIcon 师徽 |
| [x] | F.3 | `army_badge` | 走 v9 视觉（NATO 风） |
| [x] | F.4 | `naval` / `air` | 同 military 模板 |
| [x] | F.5 | `production` | 废弃 legacy 面板已移除，入口不再显示 |

### Phase G：其余面板（1 周）

| 状态 | 任务 | 涉及 |
|---|---|---|
| [x] | G.1 | `settings` + `save_browser` |
| [x] | G.2 | `end_screen` |
| [x] | G.3 | `province_info` / `province_menu` |
| [x] | G.4 | `surrender_notification` |
| [x] | G.5 | `data_table` / `icons` / `nato_icon` 实现迁到 v9 |

### Phase H：抛光（1 周）

| 状态 | 任务 | 内容 |
|---|---|---|
| [x] | H.1 | Motion：按钮 hover / tab 切换 / 模态进入 / toast 滑入 |
| [x] | H.2 | Sound hook：每个 primitive 接入 hoi4-audio（hover/click/error） |
| [x] | H.3 | Accessibility：色盲模式（用形状区分派系而非纯色）、字体放大档（×1.0 / ×1.15 / ×1.30） |
| [x] | H.4 | Profiler：v9 帧预算 ≤ 2ms（既有 `FRAME_BUDGET_US`），未达标的 primitive 单独优化 |
| [x] | H.5 | Visual snapshot test：用 egui_kittest 跑 5 个关键面板的视觉回归 |

---

## 9. 测试与验收

### 9.1 编译验收

每个 Phase 结束跑：
```powershell
cargo check --workspace
cargo test -p hoi4-ui v9
```

### 9.2 视觉回归

```powershell
# 启动游戏，按 F12 进入 v9 demo 页，逐个检查 primitive
cargo run -p hoi4-app --release -- --demo v9
```

每个 primitive 都有：
- 默认态截图
- hover 截图
- active 截图
- disabled 截图
- 极端文本截图（短中文 / 长中文 / 英文 / 数字 / 空）

### 9.3 布局回归

```powershell
# 跑三档断点（启动时传 --window 标记）
cargo run -p hoi4-app -- --window 1280x720
cargo run -p hoi4-app -- --window 1920x1080
cargo run -p hoi4-app -- --window 2560x1440
```

检查列表：
- 顶栏 8 个 tile 在 1280 是否折叠
- 标准面板在 1280 是否触发 list+detail 切换为 stack 模式
- 模态在 1280 是否不溢出
- 国旗 / 头像 frame 在 2560 是否清晰（4× 上限）

### 9.4 性能验收

```powershell
cargo run -p hoi4-app --release -- --perf
```

目标：
- 单帧 UI CPU 时间 ≤ 2ms（继承既有 `FRAME_BUDGET_US`）。
- 程序化纹理（木纹 / 羊皮纸）单次构建 ≤ 5ms（仅启动时一次）。
- 60fps 稳态下 UI 不掉帧。

### 9.5 中文残留验收

```powershell
rg "Draw Frontline|Execute Plan|Military Overview|Supporters|Winner|Intervention Log|Overlay Off" crates/hoi4-ui/src/v9
```

V9 期 0 容忍英文残留。

---

## 10. 风险与对策

| 风险 | 概率 | 影响 | 对策 |
|---|---|---|---|
| 工作量超期（42 面板 × 重做） | 高 | 主线停摆 | 用 `visual_v2` feature flag 灰度，每 Phase 单独可上线 |
| 程序化纹理性能不达标 | 中 | 60fps 掉帧 | 启动时 CPU 预生成纹理缓存到 `egui::TextureId`；shader 仅用于 panel_pass 大区块 |
| egui 自动测量与 v9 GridLayout 冲突 | 中 | 错位回归 | Phase A 写测试用例覆盖 6 种典型容器，每次回归跑 |
| 中文字体不同 fallback 导致行高漂移 | 中 | 按钮高度变化 | `Button` 强制 `min_size`，文字 truncate 不参与高度计算 |
| 视觉一致性在 D-G 期间因多人改动漂移 | 中 | 风格散乱 | 强制 PR 检查清单：触碰 UI 必须列出涉及的 token / primitive |
| 玩家不喜欢新风格 | 低 | 体验下滑 | feature flag 保留回退一个 minor 版本 |
| HOI4 vanilla 资产意外被引入 | 中 | 法律风险 | clippy + grep 检查禁用 `vanilla/interface/*.dds` 路径 |
| 程序化美术看起来"廉价" | 中 | 视觉短板 | Phase H 留口接入原创贴图，关键面板（菜单 / 模态 / 国选）优先升级 |

---

## 11. 关键决策记录

### D1：保留 egui 而非全面迁到 panel_pass
- 理由：scroll / clip / IME / 拾取 / tooltip 等都已成熟，从零实现需要 2-3 个月窗口期。
- 代价：仍要写 lint 规则禁用 egui 流式 API。
- 出口：Phase H 之后如果 egui 版本升级出问题，可考虑把"地基 5 个 primitive"迁到 panel_pass。

### D2：近期 100% 程序化美术
- 理由：无外包预算 / 无 vanilla 资产 / 想立刻上线。
- 代价：质感上限低于真贴图。
- 出口：保留 `FrameStyle::texture: Option<TextureId>` 字段，未来注入原创贴图无破坏。

### D3：一次性重做 42 面板
- 理由：用户明确选择"全部重做"；视觉一致性收益最大。
- 代价：8-10 周窗口期。
- 缓解：feature flag 灰度，每 Phase 可单独上线；旧面板在 v9 完成前继续可用。

### D4：调色板与 WW2 + KR/TNO + 冷战配比
- 主：暖金 / 黄铜 / 木纹 / 羊皮纸（WW2 + HOI4 vanilla 工艺感）。
- 辅：派系强调色（HOI4 vanilla 政治色微调，提暖）。
- 节制使用：冷战钢灰 / 信号红 / 原子青（仅 TNO 路线相关或战时模式提示）。
- 禁用：纯黑（用 `CANVAS_DEEP #110a06`）、纯白（用 `PARCHMENT #e0d2a8`）、霓虹色。

### D5：禁用 ui.horizontal / ui.vertical
- 理由：80% 的"错位"问题来自这两个 API。
- 出口：在 chip 列表等极少数场景允许使用 `ui.horizontal_wrapped`，前提是子项尺寸均来自 token。

### D6：所有屏幕级组件叫 Composite，不叫 Panel
- 理由：避免与 `PanelFrame` / `egui::SidePanel` 命名冲突。

---

## 12. 第一批可执行任务

按依赖关系列出 Phase A 的具体 PR（每个 PR 独立可合）：

1. **PR A.1** — 新建 `crates/hoi4-ui/src/v9/{mod,tokens}.rs`，迁移 `components.rs` 调色板到 `tokens::palette`（旧常量保留 re-export，标记 deprecated）。
2. **PR A.2** — `v9/layout.rs` 实现 `GridLayout` + 单元测试（固定/Fr/gutter 三类用例 12 个）。
3. **PR A.3** — `v9/paint.rs` 实现 7 种 FrameStyle 的 `draw_frame`，启动时预生成木纹 / 羊皮纸纹理到 `TextureId` 缓存。
4. **PR A.4** — `v9/frame.rs::PanelFrame` + `v9/text.rs::TextRole` 封装。
5. **PR A.5** — 5 个核心 primitive（`button` / `card` / `tile` / `tabs` / `list`），每个文件 < 200 行。
6. **PR A.6** — `v9/demo.rs` 演示页，F12 切换；同屏展示全部 tokens + primitives。

合并完上述 6 个 PR 后立刻进入 Phase B（菜单 + 顶栏）。

---

## 13. 完成定义

V9 完成时应满足：

- ✅ 所有 42 个 in-game 面板走 `PanelShell` 外壳，视觉一致。
- ✅ 主菜单 / 国家选择 / 顶栏 / HUD / 弹窗 / Toast / Tooltip 走 v9 视觉语言。
- ✅ `hoi4-ui` crate 内禁用 `Color32::from_rgb` 裸写、`RichText::size` 裸写、`ui.horizontal/vertical` 默认流式。
- ✅ 1280 / 1920 / 2560 三档断点无溢出 / 无错位。
- ✅ 单帧 UI CPU ≤ 2ms。
- ✅ 0 vanilla HOI4 美术资产被读取（grep + path lint 验证）。
- ✅ `cargo test -p hoi4-ui v9` 全绿。
- ✅ Demo 页（F12）展示完整 token / primitive 矩阵。
- ✅ feature flag `visual_v2` 移除（V9 成为唯一视觉路线）。

---

## 14. 最重要的原则

> **egui 负责画，不负责决定尺寸。**
>
> 尺寸来自 token，布局来自 GridLayout，视觉来自 FrameStyle，资产来自 paint 程序化生成。
>
> 任何"看起来错位"的地方，回溯都能落到三件事之一：
> 1. 用了流式排版（ui.horizontal/vertical）。
> 2. 用了 available_width / available_size。
> 3. 用了裸数字而不是 token。
>
> 三条同时守住，UI 就不会再错位。
