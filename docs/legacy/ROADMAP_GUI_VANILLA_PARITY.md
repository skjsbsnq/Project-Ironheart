&#x20;# Project Ironheart — GUI Vanilla Parity 路线图



&#x20; > 本文档是 \[`ROADMAP\_V3.md`](./ROADMAP\_V3.md) Phase 4（UI 全套）的\*\*补丁与诊断附录\*\*。

&#x20; >

&#x20; > \*\*触发原因（2026-05-18）\*\*：用户完成 Phase 4.3 一部分后发现政治面板"味道不对"，特别是

&#x20; > 点击国旗弹出的弹窗。逆向 vanilla `interface/\*.gui` 与项目 `gui\_runtime` / `ui\_pass` /

&#x20; > `politics\_pass` 后，发现差距不在某一处偏移，而是\*\*整套 GUI 渲染路径与解析器存在结构

&#x20; > 性缺口\*\*。本文档把这些缺口、根因、修复步骤、工作量、与 V3 的对接关系全部列出来。

&#x20; >

&#x20; > \*\*关系\*\*：本文档\*\*不替代\*\* ROADMAP\_V3，而是把 V3 Phase 4 章节\*\*拆细 + 加严\*\*。修完

&#x20; > 这里的事项之后，4.3+ 各面板的视觉与功能等价度才会真的接近 vanilla（V3 原本承诺的

&#x20; > 「视觉差距 < 5%」）。

&#x20; >

&#x20; > \*\*目标读者\*\*：实施者（决定"今天/这周/这月做什么"）。



&#x20; ---



&#x20; ## 0. 范围与目标



&#x20; ### ✅ 做



&#x20; - 修补 `.gui` 解析器：让 vanilla 写在 .gui 里的所有\*\*关键\*\*字段都被正确提取

&#x20; - 修补 `ui\_pass.rs` 的 `UiCommand::NineSlice` 分支：真正按 9-slice 切片渲染

&#x20; - 让 `GuiRuntime` 成为 in-game 面板（4.3+）的\*\*唯一\*\*布局/渲染源

&#x20; - 让 `countrypoliticsview` / `countrydiplomacyview` / `countrydecisionview` 三个核心面板

&#x20;   在视觉上肉眼难分辨于 vanilla

&#x20; - 把通用基础设施（gridBox 模板克隆、滚动条、tab 状态机、动画缓动）补齐，让 4.4-4.10

&#x20;   各面板能"按相同 pattern 复制粘贴 + 改 widget binding"完成



&#x20; ### ❌ 不做（明确延后到 V3 后续 Phase）



&#x20; - `scripted\_gui.txt` 解释器 → V3 Phase 4.10

&#x20; - Trigger / Effect AST 求值引擎扩展 → V3 Phase 5.2 / 5.3

&#x20; - 完整本地化 `localisation/\*.yml` 解析 + `\[Country.GetName]` 占位 + §Y 颜色码 + £icon 内联

&#x20;   → V3 Phase 4.9

&#x20; - DLC 专属面板内容（合法性 / 谍报 / MIO / 国联 / 秘密武器…）→ V3 Phase 4.11

&#x20; - 全 124 个 vanilla .gui 上屏验证 → 各面板自己的章节负责

&#x20; - mod 完整 `replace\_path` 覆盖机制 → V3 Phase 9.1

&#x20; - 完美 1:1 像素级一致 → 不可能。承诺的是「肉眼难分辨」，不是 100%



&#x20; ---



&#x20; ## 1. 三大根因



&#x20; ### 根因 1：9-slice 渲染\*\*完全没接通\*\*（最致命）



&#x20; \*\*症状\*\*：政治面板木纹背景、决议面板背景、所有 `corneredTileSpriteType` 引用都被

&#x20; 拉伸成糊一片。



&#x20; \*\*位置\*\*：`crates/hoi4-app/src/ui\_pass.rs:592` 的 `UiCommand::NineSlice` 分支与

&#x20; `UiCommand::Sprite` 分支字面相同，都调用 `Self::sprite\_quad()` 出\*\*单个 quad\*\*，

&#x20; 忽略了 `compute\_nine\_slice` 已经算好的 9 patch 几何与 UV。



&#x20; ```rust

&#x20; // 当前（错的）：

&#x20; UiCommand::NineSlice { sprite, rect, tint, .. } => {

&#x20;     let entry = match self.ensure\_sprite\_bind\_group(...) { ... };

&#x20;     let quad = Self::sprite\_quad(\&entry, rect.x, rect.y, rect.width, rect.height, 0);

&#x20;     // ↑ 只画一个把整张纹理拉伸成 rect 大小的 quad

&#x20;     ...

&#x20; }



&#x20; 注意：crates/hoi4-assets/src/gui\_runtime.rs:419-474 已实现正确的

&#x20; compute\_nine\_slice() 函数，且有单测通过——只是从未被消费。



&#x20; gfx.rs::SpriteKind::CorneredTile 已正确解析 corneredTileSpriteType 的

&#x20; size + borderSize + tilingCenter 字段。所以从数据到几何的链路只差最后一步上屏。



&#x20; 修了之后：GFX\_tiled\_plain\_bg（192×192, borderSize 64×64）填到 550×1000 的

&#x20; 政治面板背景上时，4 角保持 64×64 不缩，4 边按 REPEAT 平铺 64-wide 的纹理边沿，

&#x20; 中心区按 REPEAT 平铺中央 64×64——立刻"长得像 vanilla"。



&#x20; 根因 2：4.3 走错路线，与 4.1.bis 治理原则冲突



&#x20; 症状：打开政治面板时两套渲染路径同时画：



&#x20; 1. crates/hoi4-app/src/politics\_pass.rs::draw\_politics\_panel 画"暗色玻璃 + 暖金色"



&#x20;    程序化卡（继承 4.2 主菜单风格）



&#x20; 2. crates/hoi4-app/src/main.rs:3673 set\_visible\_windows(\["countrypoliticsview"]) +



&#x20;    move\_widget("countrypoliticsview", -6.0 \* scale, 78.0 \* scale) 让 GuiRuntime 同时

&#x20;    渲染 vanilla countrypoliticsview.gui 的 sprite 树



&#x20; 根本原因：politics\_pass.rs 头部注释自己写：



&#x20; //! 不加载 vanilla `politics\_view.gui`（该文件引用大量 DLC 特定 sprite，

&#x20; //! 且我们还没有 9-slice / scripted\_gui 完整管线）。



&#x20; 但实际上：



&#x20; - 9-slice 管线的几何与 GFX 解析已经写好（缺根因 1 的最后一步）

&#x20; - DLC sprite 缺失时 ensure\_sprite\_bind\_group 已经会优雅 fallback

&#x20; - scripted\_gui 是 V3 Phase 4.10 的事，4.3 不依赖它（决议数据已加载，scripted\_gui



&#x20;   影响的是动态可见性规则，缺它顶多决议显示偏多）



&#x20; 与 V3 治理原则冲突：4.1.bis 验收时明文规定"LayoutSolver 单一来源 = GuiRuntime"，

&#x20; 删除了 ui\_pass 与 main.rs 三套并行 layout。4.3 等于把 4.2 主菜单的"现代启动器"

&#x20; 风格延续到 in-game 面板，违反了 4.1.bis 自己定的边界（4.2 的设计语言仅限开场菜单，

&#x20; in-game 面板必须走 vanilla 路线）。



&#x20; 根因 3：.gui 解析器字面 ✅ 但功能 ⚠️



&#x20; 字面意义"完美" = 不 panic 不 abort。当前测试输出：



&#x20; \[gui\_vanilla] files\_loaded=447 files\_failed=0 windows=1843 warnings=0

&#x20; test result: ok. 1 passed



&#x20; tests/vanilla\_gui.rs 的 assert 字面是：



&#x20; assert!(idx.files\_loaded >= 120);

&#x20; assert\_eq!(idx.files\_failed, 0);

&#x20; assert!(idx.len() > 100);



&#x20; ——这只验证"加载得了、没炸、有 window"。



&#x20; 功能意义"完美" = 字段全读、控件 kind 全识、运行时全能跑。这个意义上远没有。



&#x20; crates/hoi4-assets/src/gui.rs::parse\_gui\_node 一共只读 19 个字段：



&#x20; name / position / size / orientation / quadTextureSprite (=spriteType) /

&#x20; texturefile / font / buttonFont / format / text / tooltip / shortcut /

&#x20; pdx\_tooltip / maxWidth / maxHeight / scrollbarType / frame / show / background



&#x20; 其他字段全部 // 未知字段静默忽略。在 vanilla 161 个 .gui 文件里，下列

&#x20; 15 个字段共 8436 处使用，全部丢弃：



&#x20; show\_position / show\_animation\_type / hide\_animation\_type / animation\_time /

&#x20; show\_sound / hide\_sound / alwaystransparent / clipping / centerposition /

&#x20; fixedsize / multiline / vertical\_alignment / slotsize / effectFile / clicksound



&#x20; 详见 §2 字段缺口清单 (#2-gui-解析器字段缺口清单)。



&#x20; ─────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────



&#x20; 2. .gui 解析器字段缺口清单



&#x20; 按危害降序。



&#x20; 2.1 P0 关键字段（修了 = 立即可见效果）



&#x20; ┌──────────────────────────────────────────────┬──────────────────────────────────────┬────────────────────────────────────────────┬──────────────────────────────────────────┐

&#x20; │ 字段                                         │ vanilla 用例                         │ 不修后果                                   │ 修复位置                                 │

&#x20; ├──────────────────────────────────────────────┼──────────────────────────────────────┼────────────────────────────────────────────┼──────────────────────────────────────────┤

&#x20; │ show\_position = { x=-6 y=78 }                │ countrypoliticsview 等所有滑入面板   │ 面板瞬间出现，无滑入动画                   │ gui.rs::parse\_gui\_node                   │

&#x20; ├──────────────────────────────────────────────┼──────────────────────────────────────┼────────────────────────────────────────────┼──────────────────────────────────────────┤

&#x20; │ show\_animation\_type = decelerated            │ 同上                                 │ 无缓动语义                                 │ 同上                                     │

&#x20; ├──────────────────────────────────────────────┼──────────────────────────────────────┼────────────────────────────────────────────┼──────────────────────────────────────────┤

&#x20; │ hide\_animation\_type = accelerated            │ 同上                                 │ 无关闭缓动                                 │ 同上                                     │

&#x20; ├──────────────────────────────────────────────┼──────────────────────────────────────┼────────────────────────────────────────────┼──────────────────────────────────────────┤

&#x20; │ animation\_time = 300                         │ 同上                                 │ 无时长                                     │ 同上                                     │

&#x20; ├──────────────────────────────────────────────┼──────────────────────────────────────┼────────────────────────────────────────────┼──────────────────────────────────────────┤

&#x20; │ alwaystransparent = yes                      │ 装饰图层 / 文字 / glow / icon        │ 装饰层偷点击，按钮无法被点                 │ gui.rs::GuiNode 加 bool 字段             │

&#x20; ├──────────────────────────────────────────────┼──────────────────────────────────────┼────────────────────────────────────────────┼──────────────────────────────────────────┤

&#x20; │ slotsize = { width=502 height=1 }            │ 所有 gridBoxType                     │ gridBox 行高=0，决议/ideas 列表完全不可见  │ gui.rs::GuiNode 加 GuiSize 字段          │

&#x20; ├──────────────────────────────────────────────┼──────────────────────────────────────┼────────────────────────────────────────────┼──────────────────────────────────────────┤

&#x20; │ centerposition = yes                         │ 居中图标（goal\_icon 等）             │ position 错算 = 图标偏移整张 sprite 的尺寸 │ gui.rs::GuiNode 加 bool 字段             │

&#x20; ├──────────────────────────────────────────────┼──────────────────────────────────────┼────────────────────────────────────────────┼──────────────────────────────────────────┤

&#x20; │ scale = 0.55                                 │ 缩小图标（power\_balance\_ICON 等）    │ 图标以 100% 大小渲染，撑爆面板             │ gui.rs::GuiNode 加 f32 字段              │

&#x20; ├──────────────────────────────────────────────┼──────────────────────────────────────┼────────────────────────────────────────────┼──────────────────────────────────────────┤

&#x20; │ clipping = yes/no                            │ 子元素是否裁剪                       │ 子图标越界画到屏幕外或别的面板上           │ gui.rs::GuiNode 加 bool 字段（默认 yes） │

&#x20; ├──────────────────────────────────────────────┼──────────────────────────────────────┼────────────────────────────────────────────┼──────────────────────────────────────────┤

&#x20; │ Orientation = "CENTER\_LEFT" / "CENTER\_RIGHT" │ 9 种锚点 vanilla 全用，项目只识 7 种 │ 这两种 fallback 到 UpperLeft，控件错位     │ gui.rs::GuiOrientation::from\_str         │

&#x20; └──────────────────────────────────────────────┴──────────────────────────────────────┴────────────────────────────────────────────┴──────────────────────────────────────────┘



&#x20; 2.2 P1 文字布局字段（影响所有文字框正确性）



&#x20; ┌────────────────────────────────────────┬───────────────────────────────────────┐

&#x20; │ 字段                                   │ 不修后果                              │

&#x20; ├────────────────────────────────────────┼───────────────────────────────────────┤

&#x20; │ fixedsize = yes                        │ maxWidth/maxHeight 不锁死，文字溢出框 │

&#x20; ├────────────────────────────────────────┼───────────────────────────────────────┤

&#x20; │ multiline = yes/no                     │ 长文字不换行（或反之）                │

&#x20; ├────────────────────────────────────────┼───────────────────────────────────────┤

&#x20; │ vertical\_alignment = center/top/bottom │ 按钮文字贴顶部，不是垂直居中          │

&#x20; ├────────────────────────────────────────┼───────────────────────────────────────┤

&#x20; │ buttonText = "DIPLOMACY\_RELATIONS\_TAB" │ 按钮上的文字 loc key 没读，按钮无标签 │

&#x20; └────────────────────────────────────────┴───────────────────────────────────────┘



&#x20; 2.3 P1 滚动条字段（外交/决议面板核心）



&#x20; ┌─────────────────────────────────────────────┬────────────────────────────────────┐

&#x20; │ 字段                                        │ 不修后果                           │

&#x20; ├─────────────────────────────────────────────┼────────────────────────────────────┤

&#x20; │ verticalScrollbar = "right\_vertical\_slider" │ 已有该字段提取，但滚动状态机未实现 │

&#x20; ├─────────────────────────────────────────────┼────────────────────────────────────┤

&#x20; │ vertical\_scroll\_step = 41                   │ 滚轮一次滚多少像素，未读           │

&#x20; ├─────────────────────────────────────────────┼────────────────────────────────────┤

&#x20; │ scroll\_wheel\_factor = 40                    │ 滚轮速度系数，未读                 │

&#x20; ├─────────────────────────────────────────────┼────────────────────────────────────┤

&#x20; │ smooth\_scrolling = yes                      │ 平滑滚动 vs 离散，未读             │

&#x20; ├─────────────────────────────────────────────┼────────────────────────────────────┤

&#x20; │ max\_slots\_horizontal = 1                    │ gridBox 横向几列，未读             │

&#x20; └─────────────────────────────────────────────┴────────────────────────────────────┘



&#x20; 2.4 P2 音效字段（视觉无影响，听觉缺失）



&#x20; ┌────────────────────────────────────────────────────────────────┬────────────────────────────────────────────┐

&#x20; │ 字段                                                           │ 不修后果                                   │

&#x20; ├────────────────────────────────────────────────────────────────┼────────────────────────────────────────────┤

&#x20; │ show\_sound = menu\_open\_window                                  │ 弹面板无音效                               │

&#x20; ├────────────────────────────────────────────────────────────────┼────────────────────────────────────────────┤

&#x20; │ hide\_sound = menu\_close\_window                                 │ 关面板无音效                               │

&#x20; ├────────────────────────────────────────────────────────────────┼────────────────────────────────────────────┤

&#x20; │ clicksound = click\_default / click\_close / decisions\_ui\_button │ 按钮点击无音效（依赖 V3 Phase 8 音频系统） │

&#x20; ├────────────────────────────────────────────────────────────────┼────────────────────────────────────────────┤

&#x20; │ oversound = ui\_menu\_over                                       │ hover 无音效                               │

&#x20; └────────────────────────────────────────────────────────────────┴────────────────────────────────────────────┘



&#x20; 2.5 P2 不常见控件 kind（vanilla 用 + 项目跳过）



&#x20; ┌───────────────┬───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┐

&#x20; │ 控件          │ vanilla 用例                                                                                                              │

&#x20; ├───────────────┼───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┤

&#x20; │ positionType  │ 纯锚点标记，无渲染。countrydiplomacyview 的 faction\_name\_position 用它                                                    │

&#x20; ├───────────────┼───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┤

&#x20; │ lineType      │ 折线图                                                                                                                    │

&#x20; ├───────────────┼───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┤

&#x20; │ barChartType  │ 条形图                                                                                                                    │

&#x20; ├───────────────┼───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┤

&#x20; │ linechartType │ 折线图变体                                                                                                                │

&#x20; ├───────────────┼───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┤

&#x20; │ pieChartType  │ 真饼图（注意：政治饼图实际用 iconType + GFX\_political\_chart 多帧 strip，但 interface/career\_profile/\* 用真 pieChartType） │

&#x20; └───────────────┴───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘



&#x20; 2.6 P3 Lua 状态机（按钮 hover/pressed 视觉）



&#x20; ┌─────────────────────────────────────────┬───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┐

&#x20; │ 字段                                    │ 备选方案                                                                                                                          │

&#x20; ├─────────────────────────────────────────┼───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┤

&#x20; │ effectFile = "gfx/FX/buttonstate\_\*.lua" │ 不解 Lua。4.1.bis.5 已用硬编码 frame 切换近似（normal=0/hover=1/pressed=2），多数 vanilla 按钮等价。少数 effectFile               │

&#x20; │                                         │ 内有特殊逻辑（如 buttonstate\_nodowneffect.lua = 不要 pressed 状态）只能识别文件名做白名单                                         │

&#x20; └─────────────────────────────────────────┴───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘



&#x20; 2.7 已知 typo / 边界情况



&#x20; ┌────────────────────────────────────────────────────┬─────────────────────────────────────────────────────────────┬─────────────────────────────────────────────────────────┐

&#x20; │ 现象                                               │ 当前行为                                                    │ 应当                                                    │

&#x20; ├────────────────────────────────────────────────────┼─────────────────────────────────────────────────────────────┼─────────────────────────────────────────────────────────┤

&#x20; │ width=100% 单百分号（vanilla 与 mod 都有）         │ lexer 吞成 Ident("100%")，unwrap\_or(0) → 宽度变 0           │ 4.1.bis.1 加了 100%% 处理但漏掉单 %，需补               │

&#x20; ├────────────────────────────────────────────────────┼─────────────────────────────────────────────────────────────┼─────────────────────────────────────────────────────────┤

&#x20; │ position = { x = -6 y = 78 } 负坐标 + UPPER\_RIGHT  │ 解析正确，但 layout 解释 anchor 时是否做"距右边 6           │                                                         │

&#x20; │ anchor                                             │ 像素"语义？需复核                                           │                                                         │

&#x20; ├────────────────────────────────────────────────────┼─────────────────────────────────────────────────────────────┼─────────────────────────────────────────────────────────┤

&#x20; │ name = "" 空字符串名                               │ 当前 if !node.name.is\_empty() 跳过，丢失这部分匿名子节点    │ 应当生成 synthetic 名（如 \_\_anon\_<hash>）以保留布局位置 │

&#x20; └────────────────────────────────────────────────────┴─────────────────────────────────────────────────────────────┴─────────────────────────────────────────────────────────┘



&#x20; ─────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────



&#x20; 3. 政治面板（4.3）改造步骤 A-E



&#x20; 3.1 步骤 A：9-slice 接通到 ui\_pass（半天-1 天，P0 先做）



&#x20; 改动范围：crates/hoi4-app/src/ui\_pass.rs + 可能扩展 crates/hoi4-assets/src/gfx.rs



&#x20; - \[x] gfx.rs 暴露 SpriteDef::border\_size: Option<(i32, i32)> 与 texture\_size: Option<(u32, u32)>（SpriteKind::CorneredTile 已有这俩，公开访问）

&#x20; - \[x] ui\_pass.rs::UiCommand::NineSlice 分支调用 compute\_nine\_slice(rect, bx, by, tex\_w, tex\_h) 拿到 9 patch 几何 + UV

&#x20; - \[x] 改 sprite\_quad 或新建 nine\_slice\_quads 让一次 NineSlice 命令生成 9 个 quad（顶点）；UV 用 compute\_nine\_slice 输出

&#x20; - \[x] wgsl shader / sampler 改 AddressMode::Repeat，UV 范围超出 \[0,1] 自动平铺中心 + 4 边

&#x20; - \[x] 单测：GFX\_tiled\_plain\_bg 192×192 + borderSize 64×64 + 目标 550×1000 → 验证 9 patch 几何 + UV 与手算一致

&#x20; - \[ ] 集成测：跑 cargo run -p hoi4-app --release，打开政治面板，木纹背景像 vanilla



&#x20; 验收标准：截图与 vanilla 同视角对比，木纹背景 4 角清晰、4 边按比例延展、中心平铺，

&#x20; 没有像素级糊作。



&#x20; 3.2 步骤 B：删除 politics\_pass 程序化路径，统一走 GuiRuntime（1-2 天）



&#x20; 改动范围：crates/hoi4-app/src/politics\_pass.rs + main.rs + binding.rs



&#x20; - \[x] 保留 PoliticsPanelData::from\_world 作为数据提取（这个写得对）

&#x20; - \[x] 删除 draw\_politics\_panel 与 draw\_politics\_overlay

&#x20; - \[x] WorldBinding::query\_string 加 widget-name 映射：

&#x20;   - political\_title → "POLITICAL\_POLITICAL"（4.9 前先返回 raw key 占位）

&#x20;   - current\_autonomy\_name → 自治状态名 / 空

&#x20;   - focus\_cost → 当前 focus PP 消耗

&#x20;   - power\_balance\_percentage / power\_balance\_levels → 政治平衡数据 / 空

&#x20;   - ideology → 执政党 ideology 名

&#x20;   - elections → 下次选举倒计时

&#x20;   - faction\_name → 阵营名 / 空

&#x20;   - overlord\_name → 宗主国名 / 空

&#x20;   - current\_autonomy\_level\_icon 等图标 → sprite name *（图标 sprite-override 机制延后；当前只填 textbox 类 widget）*



&#x20; - \[x] main.rs 删除任何"按 P → 调 draw\_politics\_panel"的代码，仅保留：

&#x20;   - set\_visible\_windows(\["topbar", "countrypoliticsview"])

&#x20;   - move\_widget("countrypoliticsview", ...) 做位置切换



&#x20; - \[ ] 决议列表 + ideas 列表 + 党派饼图 第一版只画框架不填内容（gridBox 内容动态填充见 §6.1） *（gridBox 框架由 GuiRuntime 自动渲染；ideas / decisions / 党派饼图填充本就属于 §6.1 范围，本步骤不动）*



&#x20; 验收标准：按 P 打开政治面板时，不再有任何程序化暗色卡。GuiRuntime 渲染的 vanilla

&#x20; 木纹 + 装饰图层 + 文字框 + 党派饼图（静态多帧）全部就位。决议列表暂为空 gridBox（接 §6.1 后填充）。



&#x20; 3.3 步骤 C：滑入缓动动画（半天）



&#x20; 改动范围：crates/hoi4-assets/src/gui\_runtime.rs



&#x20; - \[ ] GuiNode 加 Option<SlideAnim> 字段（仅在解析到 show\_position 时填充）

&#x20; - \[ ] SlideAnim struct：



&#x20;   pub struct SlideAnim {

&#x20;       pub from\_pos: GuiPos,    // = position

&#x20;       pub to\_pos: GuiPos,      // = show\_position

&#x20;       pub duration\_ms: u32,    // = animation\_time（默认 300）

&#x20;       pub ease: EaseKind,      // Decelerated / Accelerated / Linear

&#x20;       pub t: f32,              // 0..1, 当前进度

&#x20;       pub state: SlideState,   // Hidden / Showing / Visible / Hiding

&#x20;   }



&#x20; - \[ ] GuiRuntime::tick(dt: f32) 每帧推进 t += dt / duration，clamp 到 \[0,1]

&#x20; - \[ ] solve\_layout 中：当节点有 SlideAnim 且 state ∈ {Showing, Hiding}，用



&#x20;       lerp(from, to, ease(t)) 替代静态 position



&#x20; - \[ ] set\_visible\_windows 改为：

&#x20;   - 新加入的 window → state = Showing, t = 0

&#x20;   - 移除的 window → state = Hiding, t = 0，hide 完成后再真删



&#x20; - \[ ] easing 函数：

&#x20;   - Decelerated: 1.0 - (1.0 - t).powi(2)

&#x20;   - Accelerated: t.powi(2)

&#x20;   - Linear: t



&#x20; 验收标准：按 P 打开政治面板时，面板从屏幕左外侧 300ms 减速滑入；按 ESC 时

&#x20; 300ms 加速滑出。



&#x20; 3.4 步骤 D：国旗点击 → 弹外交面板（半天）



&#x20; 改动范围：main.rs + 给 InGamePanel 加新 variant



&#x20; - \[ ] InGamePanel 加 Diplomacy(target\_country\_idx: usize) variant

&#x20; - \[ ] 顶栏 player\_flag widget 的 hit-test handler：左键点击 → 打开自己的政治面板



&#x20;       （等价当前行为）；中键/右键先不做



&#x20; - \[ ] 地图上点击其他国家：try\_pick\_province 命中后，若按住 Shift 或别的修饰键，



&#x20;       → open\_panel = Some(Diplomacy(province\_owner))



&#x20; - \[ ] set\_visible\_windows 加 "countrydiplomacyview"

&#x20; - \[ ] WorldBinding::query\_string 增 diplomacy 相关 widget：

&#x20;   - country\_name → 目标国家本地化名

&#x20;   - leader\_name → 目标国家领导人名

&#x20;   - our\_opinion\_value / their\_opinion\_value → 双向关系值

&#x20;   - stability\_value / war\_support\_value → 目标国稳定度 / 战争支持

&#x20;   - faction\_name → 目标国阵营名

&#x20;   - diplo\_country\_flag → 目标国国旗 sprite（flag\_bank 已就位）



&#x20; - \[ ] tab 切换暂用静态显示"relations" tab（info\_tab\_button 暂禁用，§6.3 才做 tab 状态机）



&#x20; 验收标准：在地图上 Shift+点击 GER 时，弹出 vanilla 风格外交面板，显示 GER 国旗 +

&#x20; 名称 + 关系值 + 稳定度。功能性按钮（提议/宣战）暂全部禁用。



&#x20; 3.5 步骤 E：补 P0 解析器字段（半天，与 A/B 并行）



&#x20; 改动范围：crates/hoi4-assets/src/gui.rs::parse\_gui\_node + GuiNode



&#x20; 按 §2.1 P0 字段表 (#21-p0-关键字段修了--立即可见效果) 添加解析。每个字段的工作量：



&#x20; // 模板：

&#x20; "alwaystransparent" => {

&#x20;     if let Value::Bool(v) = \&e.value {

&#x20;         node.always\_transparent = \*v;

&#x20;     }

&#x20; }



&#x20; - \[ ] alwaystransparent → bool（默认 false）。GuiRuntime hit-test 时跳过 always\_transparent == true 的节点

&#x20; - \[ ] clipping → bool（默认 true）。layout 时若父节点 clipping == true，子节点 rect 与父 rect 求交

&#x20; - \[ ] centerposition → bool（默认 false）。layout 时若 true，position 解释为中心点而不是左上角

&#x20; - \[ ] scale → f32（默认 1.0）。渲染时 quad 几何按 scale 缩放（围绕中心）

&#x20; - \[ ] slotsize → GuiSize（默认 0×0）。gridBox 填充时按 slotsize.height 算行 y 偏移

&#x20; - \[ ] show\_position → GuiPos

&#x20; - \[ ] show\_animation\_type / hide\_animation\_type → enum（Decelerated / Accelerated / Linear）

&#x20; - \[ ] animation\_time → u32（默认 300）

&#x20; - \[ ] Orientation 字符串增加 CENTER\_LEFT / CENTER\_RIGHT 两个值

&#x20; - \[ ] 单百分号 % 处理：lexer 在 \\d+% 但下一字符不是 % 时报 warning + 仍当百分比



&#x20; 验收标准：vanilla\_gui.rs 测试新增断言：随机抽样 10 个 vanilla 控件，

&#x20; 检查 alwaystransparent / scale / slotsize 等字段被正确读取。



&#x20; ─────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────



&#x20; 4. 决议面板（不在 V3 4.3 内，但路线图缺它）



&#x20; V3 Phase 4 章节没单独立项决议面板。本节补上分级路径。



&#x20; 4.1 Level 0：UI 框架可见（半天）



&#x20; 目标：锤子按钮可点 → 弹出空白面板 → 可关闭。



&#x20; - \[ ] InGamePanel 加 Decisions variant

&#x20; - \[ ] 顶栏 decisionview\_button widget 的 hit-test handler：toggle Decisions panel

&#x20; - \[ ] set\_visible\_windows 加 "countrydecisionview"

&#x20; - \[ ] move\_widget 做滑入位移（动画依赖步骤 C）

&#x20; - \[ ] 锤子上的 alert badge（decisionview\_amount\_timeout\_items 等）继续被 hide\_widgets\_by\_name 隐藏

&#x20; - \[ ] 关闭按钮 hit-test → 设 open\_panel = None



&#x20; 视觉效果：木纹背景 + "Decisions" 标题 + 关闭按钮 + 空白滚动区。仍然不像 vanilla（因为没有决议条目），但比"完全打不开"好。



&#x20; 4.2 Level 1：决议条目静态可见（3-5 天，依赖 §6.1 gridBox 模板克隆）



&#x20; 目标：能看到 vanilla 真实决议条目（图标、名字、PP cost）。所有决议视觉上"看起来可点"，但点击没反应。



&#x20; - \[ ] 依赖 §6.1：GuiRuntime 实现 instantiate\_template(template\_name, parent\_widget, slot\_index) API

&#x20; - \[ ] 启动时把 category\_header / decision\_category\_desc / event\_header /



&#x20;       custom\_icon\_item 等独立顶层 window 标记为 template（不在 visible\_windows 中渲染）



&#x20; - \[ ] 打开决议面板时，按 World.data.decisions 的 category 字段分组：

&#x20;   - 每个 category 实例化 1 个 category\_header 模板，填充 name\_text + icon

&#x20;   - 每个该 category 下的决议实例化 1 个 row 模板（vanilla 实际是直接在 category\_header 下嵌套），填充 cost / icon / name



&#x20; - \[ ] slotsize.height 决定每行 y 偏移，gridBox max\_slots\_horizontal=1 强制单列

&#x20; - \[ ] 滚动条至少做"鼠标滚轮工作"（依赖 §6.2 滚动状态机）

&#x20; - \[ ] 决议可用性：所有决议视觉上显示为"可用"——available trigger 不能 eval，先按 true 处理（5.2 才能正确判定）

&#x20; - \[ ] 点击决议：先做 hit-test 但 不执行 effect，仅打印日志 + 扣 PP（如果 cost > 0）。这是占位行为，给后续 5.3 接上时切换



&#x20; 视觉效果：政治分类下显示 "Boost Party Popularity"、"Ask for Military Access"、

&#x20; "Improve Relations"…，每行有图标 + PP cost。和 vanilla 同视角难分辨。



&#x20; 4.3 Level 2：决议可用性 + 点击执行（依赖 V3 Phase 5.2/5.3）



&#x20; 目标：vanilla 等价。



&#x20; - \[ ] 依赖 V3 Phase 5.2：trigger 集 30 → 至少 100（覆盖决议常用 \~30 个 trigger：



&#x20;       has\_country\_flag / has\_completed\_focus / has\_idea / has\_government /

&#x20;       is\_in\_faction / has\_war\_with / has\_political\_power > X / has\_stability > X /

&#x20;       num\_of\_factories > X / has\_dlc / original\_tag = X / has\_navy\_size …）



&#x20; - \[ ] 依赖 V3 Phase 5.3：effect 集 28 → 至少 80（覆盖 \~30 个 effect：



&#x20;       add\_political\_power / add\_stability / add\_war\_support / set\_country\_flag /

&#x20;       clr\_country\_flag / country\_event / add\_ideas / remove\_ideas /

&#x20;       add\_to\_war / add\_named\_threat / add\_resource …）



&#x20; - \[ ] 依赖：Trigger / Effect AST 求值引擎重构（项目当前 effect runner 是 28 个 hardcoded



&#x20;       match arm，要重构为"递归 walk Block AST"的通用 eval）



&#x20; - \[ ] 决议面板每帧 / 每决议 eval available + visible trigger，更新视觉状态（可点/灰显/隐藏）

&#x20; - \[ ] 点击决议 → 扣 PP + run complete\_effect + 加入 DecisionCatalog 倒计时 +



&#x20;       到期 run remove\_effect + cooldown\_days 处理



&#x20; - \[ ] fire\_only\_once 标记

&#x20; - \[ ] days\_mission\_timeout（任务型决议带进度图标）



&#x20; 视觉效果：和 vanilla 同视角玩起来一致。



&#x20; 4.4 Level 3：完整体验（V3 Phase 4.10 + 4.11 范围）



&#x20; - scripted\_gui 解释器（DLC 决议大量依赖）

&#x20; - 任务型决议带图标进度条 + 倒计时显示

&#x20; - 决议触发的事件链 + 决议子决议联动

&#x20; - 完整本地化文字（loc key + \[Country.GetName] 占位 + 颜色码）

&#x20; - DLC 决议（common/decisions/<dlc>\_\*.txt）

&#x20; - 决议分类的 collapse/expand 状态机

&#x20; - 小事件 / 重大事件 popup 开关 checkbox



&#x20; ─────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────



&#x20; 5. 国旗弹窗（已在步骤 D 中解决基本部分，本节列扩展项）



&#x20; §3.4 步骤 D (#34-步骤-d国旗点击--弹外交面板半天) 做完后，外交面板基本可见。

&#x20; 但 vanilla countrydiplomacyview.gui 是 115 KB，下面是后续要做的：



&#x20; - \[ ] relations\_tab 完整内容：

&#x20;   - 提议改善关系 / 派遣志愿军 / 派遣远征军 / 租借法案 / 军事通行权 / 占领权

&#x20;   - 阵营管理（创建 / 加入 / 邀请 / 退出 / 解散）

&#x20;   - 宣战 / 给战争目标 / 提议和平

&#x20;   - 每个动作的 PP 消耗 + 需求 trigger（依赖 5.2）



&#x20; - \[ ] info\_tab 完整内容：

&#x20;   - 工业明细（civ/mil/dock 工厂 + 生产中装备数）

&#x20;   - 军事明细（陆海空师/舰/wing 数量与类型分布）

&#x20;   - 经济明细（资源产出 / 进出口 / 贸易路线）

&#x20;   - 历史 tab（DLC LaR 加，玩家与该国历史交互记录）



&#x20; - \[ ] tabbedWindowType 状态机：tab 切换 → 隐藏一组子控件 / 显示另一组（§6.3）

&#x20; - \[ ] 领导人头像 leader\_portrait：从 gfx/leaders/<TAG>/<leader\_id>.dds 加载（依赖 4.9 portrait 系统）

&#x20; - \[ ] 盾形国旗 GFX\_shield\_medium：vanilla 用专门的盾形 mask + 国旗组合（已有 flag\_bank 支持，加 mask 复用 gui\_special.wgsl）

&#x20; - \[ ] 意识形态图标 GFX\_ideology\_unknown 多帧 strip：按目标国 ideology 选 frame



&#x20; ─────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────



&#x20; 6. 通用基础设施债（多面板共享，优先级高于单个面板）



&#x20; 这些做完一次，4.4-4.10 各面板自动受益。不做这些，每个面板都要单独打补丁。



&#x20; 6.1 gridBoxType 模板克隆 API（P0，3-5 天）



&#x20; 为什么是 P0：决议面板 / 政治面板 ideas 列表 / 国策树 / 科研树 / 生产队列 /

&#x20; 建造队列 / 军队列表——所有列表型 UI 都用 gridBox。没这套 API，所有列表全空。



&#x20; vanilla 引擎的语义：



&#x20; - gridBox 在 .gui 里只声明 slotsize 与 max\_slots\_horizontal/vertical，是容器骨架

&#x20; - 引擎运行时根据数据克隆模板节点 N 次塞进 gridBox 的子节点列表

&#x20; - 模板节点是同 .gui 文件的其他顶层 containerWindow（如 category\_header），但通过命名约定或外部 API 关联



&#x20; 项目实现方案：



&#x20; - \[ ] GuiIndex 加 templates HashMap：把不在 containerWindowType "<panel>" 主入口



&#x20;       子树里的顶层 window 标记为 template（candidate criteria：是顶层 containerWindow 但

&#x20;       在 politics\_pass / decisionview / playable\_state\_view 等已知主面板的子节点

&#x20;       里被引用 by name，或位置 {x=0 y=0} 且 size 较小）



&#x20; - \[ ] GuiRuntime 加 instantiate\_template API：



&#x20;       pub fn instantiate\_template(

&#x20;           \&mut self,

&#x20;           template\_name: \&str,

&#x20;           parent\_id: u32,           // 通常是 gridBox 的 widget id

&#x20;           slot\_index: usize,

&#x20;           data\_overrides: HashMap<String, Value>,  // 子 widget 名 → 值

&#x20;       ) -> Option<u32>;             // 返回新建实例的 widget id



&#x20; - \[ ] 数据 override：模板内子 widget 名（name\_text / icon / pol\_power\_icon



&#x20;       / focus\_cost）通过 data\_overrides 替换 sprite 名 / 文字 / frame index



&#x20; - \[ ] slotsize 应用：gridBox 子节点的 y 偏移 = slot\_index \* slotsize.height，



&#x20;       x = (slot\_index % max\_slots\_horizontal) \* slotsize.width



&#x20; - \[ ] clear / re-instantiate：每次面板打开 / 数据变更，先 clear gridBox 子节点



&#x20;       再重新 instantiate（避免多次叠加）



&#x20; - \[ ] 单测：拿 countrydecisionview.gui 的 category\_header 模板，instantiate



&#x20;       3 次 with 不同 name + icon，验证 GuiRuntime 输出 3 套子节点 + 正确 y 偏移



&#x20; 依赖：解析器读到 slotsize + max\_slots\_horizontal（§2.3 P1）。



&#x20; 6.2 滚动条状态机 + 鼠标滚轮（P0，2-3 天）



&#x20; - \[ ] GuiRuntime 加 ScrollState { offset\_y: f32, max\_scroll: f32 } per-window

&#x20; - \[ ] window 有 verticalScrollbar = "right\_vertical\_slider" 时启用 viewport：

&#x20;   - 子节点位置整体下移 -offset\_y

&#x20;   - viewport 区域裁剪（依赖 §3.5 clipping）



&#x20; - \[ ] 鼠标滚轮事件：当鼠标悬停在该 window 内时，offset\_y += scroll\_delta \* scroll\_wheel\_factor

&#x20; - \[ ] vertical\_scroll\_step = 41 用于离散滚动（无 smooth\_scrolling = yes 时）

&#x20; - \[ ] smooth\_scrolling = yes 时用线性插值 t in 0..200ms

&#x20; - \[ ] 滚动条本身的 sprite 渲染（right\_vertical\_slider 是 vanilla 内置 widget；



&#x20;       第一版可以用程序化矩形占位，3.12.13 GUI shader 完成后用 gui\_special.wgsl::scrollbar variant）



&#x20; - \[ ] max\_scroll 由 gridBox 总高度 - viewport 高度自动算



&#x20; 6.3 tabbedWindowType 状态机（P1，2 天）



&#x20; - \[ ] GuiRuntime 加 per-tabbedWindow 的 active\_tab: u32

&#x20; - \[ ] 解析 tabbedWindowType 节点的子 tab buttons 与对应 content windows



&#x20;       （vanilla 约定：tab i 的 button 名 tab\_i\_button 对应 content tab\_i\_content，

&#x20;       或通过 tab\_button / tab\_content 字段指定）



&#x20; - \[ ] tab button click → 切换 active\_tab → 显示对应 content，隐藏其他

&#x20; - \[ ] 仅外交面板和国策树/科研树用，但是面板基础设施



&#x20; 6.4 字段消费：补 P1/P2 字段到运行时（P1，2-3 天）



&#x20; 补完 §3.5 步骤 E (#35-步骤-e补-p0-解析器字段半天与-ab-并行) 之后：



&#x20; - \[ ] fixedsize → text\_pass.draw\_text\_aligned 用 max\_width/max\_height 强制裁剪

&#x20; - \[ ] multiline → 字符串包含 \\n 或宽度溢出 max\_width 时是否自动换行

&#x20; - \[ ] vertical\_alignment → 文字 baseline 计算（top: y=0; center: y=(rect.h - line\_h)/2; bottom: y=rect.h-line\_h）

&#x20; - \[ ] buttonText → UiCommand::Sprite 之后追加 UiCommand::Text 在按钮 rect 中心

&#x20; - \[ ] centerposition → layout 解析 position 时偏移 -size/2



&#x20; 6.5 音效系统骨架（P2，依赖 V3 Phase 8）



&#x20; 不做。等 V3 Phase 8 hoi4-audio 就位后，加：



&#x20; - show\_sound / hide\_sound → 面板打开/关闭时播放

&#x20; - clicksound → 按钮点击时播放

&#x20; - oversound → 进入 hover 状态时播放（一次性，不重复）



&#x20; 6.6 Lua effectFile 白名单识别（P3，半天）



&#x20; - \[ ] 解析 effectFile = "gfx/FX/buttonstate\_\*.lua" 为字符串

&#x20; - \[ ] 已知文件名映射到行为（不解 Lua）：

&#x20;   - buttonstate.lua → 默认 normal/hover/pressed 三态切 frame

&#x20;   - buttonstate\_nodowneffect.lua → 仅 normal/hover，不切 pressed

&#x20;   - buttonstate\_blendframes.lua → 状态间淡入淡出（依赖 4.1.bis.5 的 blend\_t）



&#x20; - \[ ] 后续可加更多按需识别



&#x20; ─────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────



&#x20; 7. 工作量与里程碑表



&#x20; 按 P0 → P1 → P2 顺序排实施。不要按面板顺序做——按基础设施顺序做，做完一次所有面板自动受益。



&#x20; ┌──────────────────────────────────────────┬────────┬────────────────┬────────────────────────────────────────┐

&#x20; │ 工作项                                   │ 优先级 │ 估算           │ 依赖                                   │

&#x20; ├──────────────────────────────────────────┼────────┼────────────────┼────────────────────────────────────────┤

&#x20; │ Step A — 9-slice 接通                    │ P0     │ 半天-1 天      │ 无                                     │

&#x20; ├──────────────────────────────────────────┼────────┼────────────────┼────────────────────────────────────────┤

&#x20; │ Step E — P0 解析器字段补全               │ P0     │ 半天           │ 无                                     │

&#x20; ├──────────────────────────────────────────┼────────┼────────────────┼────────────────────────────────────────┤

&#x20; │ §6.1 — gridBox 模板克隆                  │ P0     │ 3-5 天         │ Step E (slotsize 字段)                 │

&#x20; ├──────────────────────────────────────────┼────────┼────────────────┼────────────────────────────────────────┤

&#x20; │ §6.2 — 滚动条状态机                      │ P0     │ 2-3 天         │ Step E (clipping 字段)                 │

&#x20; ├──────────────────────────────────────────┼────────┼────────────────┼────────────────────────────────────────┤

&#x20; │ Step C — 滑入缓动动画                    │ P1     │ 半天           │ Step E (4 个动画字段)                  │

&#x20; ├──────────────────────────────────────────┼────────┼────────────────┼────────────────────────────────────────┤

&#x20; │ Step B — 政治面板走 GuiRuntime           │ P1     │ 1-2 天         │ Step A + §6.1 (ideas / decisions 列表) │

&#x20; ├──────────────────────────────────────────┼────────┼────────────────┼────────────────────────────────────────┤

&#x20; │ Step D — 国旗 → 外交弹窗                 │ P1     │ 半天           │ Step B 的 binding 模式                 │

&#x20; ├──────────────────────────────────────────┼────────┼────────────────┼────────────────────────────────────────┤

&#x20; │ §6.3 — tabbedWindow 状态机               │ P1     │ 2 天           │ 无                                     │

&#x20; ├──────────────────────────────────────────┼────────┼────────────────┼────────────────────────────────────────┤

&#x20; │ §6.4 — P1/P2 字段消费                    │ P1     │ 2-3 天         │ Step E                                 │

&#x20; ├──────────────────────────────────────────┼────────┼────────────────┼────────────────────────────────────────┤

&#x20; │ 决议面板 Level 0                         │ P1     │ 半天           │ Step A + Step C                        │

&#x20; ├──────────────────────────────────────────┼────────┼────────────────┼────────────────────────────────────────┤

&#x20; │ 决议面板 Level 1                         │ P2     │ 3-5 天         │ §6.1 + §6.2 + Level 0                  │

&#x20; ├──────────────────────────────────────────┼────────┼────────────────┼────────────────────────────────────────┤

&#x20; │ §6.5 — 音效（hoi4-audio）                │ P2     │ —              │ V3 Phase 8                             │

&#x20; ├──────────────────────────────────────────┼────────┼────────────────┼────────────────────────────────────────┤

&#x20; │ §6.6 — Lua effectFile 白名单             │ P3     │ 半天           │ 无                                     │

&#x20; ├──────────────────────────────────────────┼────────┼────────────────┼────────────────────────────────────────┤

&#x20; │ 决议面板 Level 2                         │ —      │ 取决于 5.2/5.3 │ V3 Phase 5.2 + 5.3                     │

&#x20; ├──────────────────────────────────────────┼────────┼────────────────┼────────────────────────────────────────┤

&#x20; │ 国策树 / 科研 / 生产 / 建造 / 军队各面板 │ —      │ 每个 1-2 天    │ 上述全部 P0/P1 完成后                  │

&#x20; └──────────────────────────────────────────┴────────┴────────────────┴────────────────────────────────────────┘



&#x20; 推荐 Session 安排



&#x20; ┌─────────┬─────────────────────────────────────────┬─────────────────────────┐

&#x20; │ Session │ 目标                                    │ 预计完成                │

&#x20; ├─────────┼─────────────────────────────────────────┼─────────────────────────┤

&#x20; │ 1       │ Step A + Step E（9-slice + 解析器字段） │ 第一次"味道对了"        │

&#x20; ├─────────┼─────────────────────────────────────────┼─────────────────────────┤

&#x20; │ 2       │ §6.1 gridBox 模板克隆                   │ 决议条目能看到          │

&#x20; ├─────────┼─────────────────────────────────────────┼─────────────────────────┤

&#x20; │ 3       │ §6.2 滚动条 + Step C 缓动               │ 面板"动起来"了          │

&#x20; ├─────────┼─────────────────────────────────────────┼─────────────────────────┤

&#x20; │ 4       │ Step B + Step D                         │ 政治 + 外交面板基本完成 │

&#x20; ├─────────┼─────────────────────────────────────────┼─────────────────────────┤

&#x20; │ 5       │ 决议面板 Level 0 → Level 1              │ 决议骨架完成            │

&#x20; ├─────────┼─────────────────────────────────────────┼─────────────────────────┤

&#x20; │ 6       │ §6.3 tab + §6.4 字段消费 + 国策树骨架   │ 4.4 起步                │

&#x20; ├─────────┼─────────────────────────────────────────┼─────────────────────────┤

&#x20; │ 7+      │ 各面板按 binding 模式批量推进           │ 4.4 / 4.5 / 4.6 / 4.7   │

&#x20; └─────────┴─────────────────────────────────────────┴─────────────────────────┘



&#x20; 预估总工作量：单人专职约 4-6 周完成 P0/P1 全部 + 4.3/4.4/4.5/4.6/4.7 五大面板

&#x20; 骨架（不含 trigger/effect 扩展）。



&#x20; 完成后实际等价度：



&#x20; - 政治 / 外交 / 决议 面板视觉等价度 \~85%（缺：完整 trigger eval + 完整本地化 + 部分 DLC sprite）

&#x20; - 国策树 / 科研 / 生产 / 建造 / 军事 面板视觉等价度 \~70-80%（更多依赖 trigger/effect 与 scripted\_gui）

&#x20; - 整体游戏视觉相似度从当前 \~50%（HUD+面板）提升到 \~85%



&#x20; ─────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────



&#x20; 8. 不做（明确延后）



&#x20; - ❌ 完美 1:1 像素级一致——不可能，目标是「肉眼难分辨」

&#x20; - ❌ scripted\_gui 解释器——V3 Phase 4.10

&#x20; - ❌ trigger/effect AST 求值引擎扩展——V3 Phase 5.2/5.3，10-14 周量级

&#x20; - ❌ 完整本地化（loc key 解析 + §Y 颜色码 + £icon 内联 + \[Country.GetName] 占位）——V3 Phase 4.9

&#x20; - ❌ DLC 专属面板内容（合法性 / 谍报 / MIO / 国联 / 秘密武器 / 抵抗 / 朝廷）——V3 Phase 4.11 + 6.6/6.7/6.8

&#x20; - ❌ Lua 解释器——effectFile 用文件名白名单近似，不解 Lua 本体

&#x20; - ❌ 重新实现 Paradox .fxh shader 编译器——V3 已明确放弃，走 wgsl 等价层（Phase 3.11）

&#x20; - ❌ 音乐 / 音效 vanilla 1:1——V3 Phase 8，独立工作流

&#x20; - ❌ mod replace\_path 完整覆盖机制——V3 Phase 9.1

&#x20; - ❌ Steam Workshop 自动同步——V3 Phase 9.1

&#x20; - ❌ .fsb (FMOD bank) 解码——V3 Phase 8.2 决策为不做（默认走 ogg）



&#x20; ─────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────



&#x20; 9. 与 ROADMAP\_V3 的对接表



&#x20; 本路线图是 V3 Phase 4 的子项加严，不替代。下表说明每个工作项归属哪个 V3 Phase。



&#x20; ┌─────────────────────────────────────┬────────────────────────┬───────────────────────────────────────────────────────────────────┐

&#x20; │ 本文档 §                            │ V3 章节                │ 关系                                                              │

&#x20; ├─────────────────────────────────────┼────────────────────────┼───────────────────────────────────────────────────────────────────┤

&#x20; │ §3.1 Step A — 9-slice 接通          │ V3 2.6（已标 ✅）      │ 修正 ✅ 误打——补完才算真 ✅                                       │

&#x20; ├─────────────────────────────────────┼────────────────────────┼───────────────────────────────────────────────────────────────────┤

&#x20; │ §3.2 Step B — 政治面板走 GuiRuntime │ V3 4.3（部分 ✅）      │ 重做 4.3 视觉部分；保留 4.3 已有 PoliticsPanelData 数据层         │

&#x20; ├─────────────────────────────────────┼────────────────────────┼───────────────────────────────────────────────────────────────────┤

&#x20; │ §3.3 Step C — 滑入缓动              │ V3 4.1.bis（部分）     │ 新增 4.1.bis 没明确                                               │

&#x20; ├─────────────────────────────────────┼────────────────────────┼───────────────────────────────────────────────────────────────────┤

&#x20; │ §3.4 Step D — 国旗弹外交            │ V3 4.7                 │ 提前 4.7 部分到 4.3 阶段做最小集                                  │

&#x20; ├─────────────────────────────────────┼────────────────────────┼───────────────────────────────────────────────────────────────────┤

&#x20; │ §3.5 Step E — 解析器字段补全        │ V3 2.5（已标 ✅）      │ 修正 ✅ 误打——按字段全集才算 ✅                                   │

&#x20; ├─────────────────────────────────────┼────────────────────────┼───────────────────────────────────────────────────────────────────┤

&#x20; │ §4 决议面板分级                     │ V3 4.3 + 4.10          │ 拆分 Level 0/1 在 4.3 范围；Level 2 在 4.10 + 5.2/5.3             │

&#x20; ├─────────────────────────────────────┼────────────────────────┼───────────────────────────────────────────────────────────────────┤

&#x20; │ §6.1 gridBox 模板克隆               │ V3 4.1.bis（隐含）     │ 新增 4.1.bis 提到 gridBox 但没列模板克隆                          │

&#x20; ├─────────────────────────────────────┼────────────────────────┼───────────────────────────────────────────────────────────────────┤

&#x20; │ §6.2 滚动条状态机                   │ V3 4.1.bis（部分）     │ 新增 4.1.bis "完整 9-slice / 滚动条状态机" 已标延后，本节正式立项 │

&#x20; ├─────────────────────────────────────┼────────────────────────┼───────────────────────────────────────────────────────────────────┤

&#x20; │ §6.3 tabbedWindow                   │ V3 4.1.bis（部分）     │ 同上                                                              │

&#x20; ├─────────────────────────────────────┼────────────────────────┼───────────────────────────────────────────────────────────────────┤

&#x20; │ §6.4 字段消费                       │ V3 4.1.bis 延伸        │ 新增                                                              │

&#x20; ├─────────────────────────────────────┼────────────────────────┼───────────────────────────────────────────────────────────────────┤

&#x20; │ §6.5 音效骨架                       │ V3 Phase 8             │ 沿用 V3 节奏                                                      │

&#x20; ├─────────────────────────────────────┼────────────────────────┼───────────────────────────────────────────────────────────────────┤

&#x20; │ §6.6 Lua effectFile                 │ V3 4.1.bis（部分）     │ 新增                                                              │

&#x20; ├─────────────────────────────────────┼────────────────────────┼───────────────────────────────────────────────────────────────────┤

&#x20; │ 不做清单                            │ V3 各 Phase / 范围声明 │ 沿用 V3 决策                                                      │

&#x20; └─────────────────────────────────────┴────────────────────────┴───────────────────────────────────────────────────────────────────┘



&#x20; ─────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────



&#x20; 10. 验收闭环



&#x20; 每完成一个 P0/P1 工作项，执行：



&#x20; 1. cargo build --workspace 干净（仅允许 pre-existing 24 个 unused warning）

&#x20; 2. cargo test --workspace ALL PASS

&#x20; 3. cargo run --release -p hoi4-app -- --headless --headless-days 7 跑通（不退步 ms/day）

&#x20; 4. 启动 cargo run -p hoi4-app --release，进游戏，截图与 vanilla 同视角对比：

&#x20;   - 截图保存到 tests/screenshots/gui-parity/<工作项名>-after.png

&#x20;   - vanilla 同视角参考保存到 tests/screenshots/gui-parity/<工作项名>-vanilla.png

&#x20;   - 在 PR 描述中并排展示



&#x20; P0 全部完成后的最终验收（M-GUI-Parity）：



&#x20; - ✅ 顶栏 + 政治面板 + 外交弹窗 + 决议面板 4 个核心面板可见且视觉等价 vanilla ≥ 85%

&#x20; - ✅ \[gui\_vanilla] 测试断言扩展：随机抽样 20 个 vanilla 控件验证字段读取正确

&#x20; - ✅ cargo test -p hoi4-assets gui\_runtime 50+ 单测全 PASS

&#x20; - ✅ 解析器读取字段从 19 → 35+

&#x20; - ✅ 进游戏开决议面板能看到 vanilla 真实决议条目（即使 trigger 还没扩展）

&#x20; - ✅ 截图集 tests/screenshots/gui-parity/ 全部就位



&#x20; ─────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────



&#x20; 11. 当前状态（持续更新）



&#x20; 2026-05-18 创建。本文档基于以下逆向分析建立：



&#x20; - 读 vanilla interface/countrypoliticsview.gui（44 KB）

&#x20; - 读 vanilla interface/countrydiplomacyview.gui（115 KB）

&#x20; - 读 vanilla interface/countrydecisionview.gui（16 KB）

&#x20; - 读 vanilla interface/topbar.gui 锤子按钮区域

&#x20; - 读项目 crates/hoi4-assets/src/gui.rs::parse\_gui\_node

&#x20; - 读项目 crates/hoi4-assets/src/gui\_runtime.rs::compute\_nine\_slice / solve\_layout

&#x20; - 读项目 crates/hoi4-app/src/ui\_pass.rs::UiCommand::NineSlice（root cause #1）

&#x20; - 读项目 crates/hoi4-app/src/politics\_pass.rs::draw\_politics\_panel（root cause #2）

&#x20; - 读项目 crates/hoi4-app/src/main.rs:2390-2480 + 3660-3725 GuiRuntime 接入逻辑

&#x20; - 跑 cargo test -p hoi4-assets --test vanilla\_gui --release：



&#x20;   files\_loaded=447 files\_failed=0 windows=1843 warnings=0



&#x20; - grep vanilla 161 个 .gui 中 15 个未读字段：8436 处使用



&#x20; 进度（实施时勾选）：



&#x20; - \[x] §3.1 Step A — 9-slice 接通  *(2026-05-18 完成；compute\_nine\_slice + tiled UV + sampler::Repeat + 9 quad；单测 nine\_slice\_tiled\_plain\_bg\_550x1000 通过)*

&#x20; - \[ ] §3.5 Step E — P0 解析器字段补全

&#x20; - \[ ] §6.1 gridBox 模板克隆 API

&#x20; - \[ ] §6.2 滚动条状态机

&#x20; - \[ ] §3.3 Step C — 滑入缓动

&#x20; - \[x] §3.2 Step B — 政治面板走 GuiRuntime  *(2026-05-18 完成；删除 draw\_politics\_panel/overlay + PoliticsTab/Layout；WorldBinding 加 9 个 widget-name 映射；按 P/点旗帜仅切 open\_panel，set\_visible\_windows + move\_widget 路径接管渲染。决议 / ideas / 党派饼图填充延后到 §6.1)*

&#x20; - \[ ] §3.4 Step D — 国旗弹外交

&#x20; - \[ ] §6.3 tabbedWindow 状态机

&#x20; - \[ ] §6.4 P1/P2 字段消费

&#x20; - \[ ] §4.1 决议面板 Level 0

&#x20; - \[ ] §4.2 决议面板 Level 1

&#x20; - \[ ] §6.6 Lua effectFile 白名单



&#x20; ---



&#x20; 文件就到这里。手动复制到 `ROADMAP\_GUI\_VANILLA\_PARITY.md` 保存即可。



&#x20; 文档里有几处可能你想立刻改的地方：



&#x20; 1. \*\*§7 推荐 Session 安排\*\*那个表是我建议的执行顺序，实际可能要看你这周精力先做 Session 1 还是想直接挑战 Session 4 的政治面板换骨；

&#x20; 2. \*\*§9 与 ROADMAP\_V3 的对接表\*\*里把 V3 标过 ✅ 的 2.5/2.6 都标成"误打 ✅"——这是诚实评估，但写进 ROADMAP\_V3 时要不要把那两个 ✅ 改回 ⚠️ 需要你拍板；

&#x20; 3. \*\*§11 进度\*\* checkbox 留作实施进度跟踪用，按我建议的 P0 先 → P1 顺序排了。

