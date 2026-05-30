# 军事 UI 视觉化重做路线图

> 写于 2026-05-26。目标:缩小 Spain 1936 截图与原版 HoI4 在"集团军呈现"这块的差距,只解决用户点名的三块,并重做现有军团托盘/卡片/将领立绘的丑陋视觉。

## 目标

1. **点击集团军 → 弹出左侧详情面板**(将领立绘 + 师 NATO 图标 + 组织度/装备/兵力条)。
2. **右上角"选中集团军"摘要徽章**(对位 vanilla 的"西班牙第1战区"徽章位置)。
3. **重做底部军团托盘**:替换 `military.rs:252-272` 用 `circle_filled + rect_filled` 拼出来的"小人头像",替换 `tool_button` 的 ASCII 字符按钮(▰ ➜ ▶ ■ ⌫ × 等),调整间距/颜色/字号统一视觉。

## 不在范围

- 师模板设计器 dock 化(继续放在模态窗,Phase 8 再议)
- 海军任务条
- 顶部资源栏补燃料/油/工厂图标
- 计数器 CR-2 组织度/兵力条
- 省名标签
- 真实将领立绘资源(MVP 先用渐变+首字占位,后续可换)
- 引入 Theater 数据层(本路线图不动 `hoi4-state`)

## 设计决策(决定前先签字)

| 决策 | 选择 | 理由 |
|---|---|---|
| 右上徽章语义 | **复用为"选中集团军摘要"** | 我们没有 Theater 层级,引入会扩大存档/AI 改动面 |
| 将领立绘 MVP | **暗金渐变 + 姓首字 + 金边描边** | 零资源开销,与项目美术风格一致,后续可热替换 |
| NATO 图标源 | **`egui::Painter` 程序化绘制**(矩形 + 内符) | 零额外资源,与底栏小卡片风格统一 |
| 详情面板锚定 | **`egui::Area` 浮在左侧,不挤地图** | 避免 `SidePanel` 改变地图 viewport |

## 当前状态

### 已有
- `MilitaryData.armies` 已含 `commander_name`/`command_limit`/`attack_bonus_pct`/`defense_bonus_pct`/`planning_bonus_pct`/`org_recovery_bonus_pct`/`supply_reduction_pct`,可直接消费
- `MilitaryData.divisions` 已含 `organisation`/`max_organisation`/`strength`/`equipment_ratio`/`army_id`
- `MilitaryCommand::SelectArmy(u32)`/`ClearArmySelection` 已存在
- 底栏托盘已实现 (`military.rs:880-1145`)

### 丑/缺
- `military.rs:252-272` 将领立绘 = `circle_filled` + `rect_filled` 拼小人 → **丑**
- `military.rs:347` `tool_button` 用 `▰ ➜ ▶ ■ ⌫ × + -` 字符 → **不像图标**
- `military.rs:298-308` 状态文本 `IDLE/EXEC/PLAN/FRONT` 是英文+全大写 → **风格不统一**
- 选中军团详情只有 `show_side_panel:815-828` 三行文字,无立绘、无 NATO 图标 → **见即不可信**
- 右上无任何军团摘要 → **缺**

## 文件改动

### 新增
- `crates/hoi4-ui/src/portrait.rs` —— 将领立绘 widget(渐变+首字),供详情面板、徽章、底栏卡片共用
- `crates/hoi4-ui/src/nato_icon.rs` —— NATO 师符 widget(`egui::Painter` 程序化)
- `crates/hoi4-ui/src/army_detail_panel.rs` —— 点击军团弹出的左侧详情面板
- `crates/hoi4-ui/src/army_badge.rs` —— 右上摘要徽章

### 修改
- `crates/hoi4-ui/src/military.rs`
  - `army_card` (210-345): 调用 `portrait::draw` 替换 252-272 的拼接画法
  - `tool_button` (347-353): 改用 `components::action_button` 风格(已有),配中文短词 + tooltip,弃用 ASCII 字符
  - `show_bottom_bar` (880-1145): 状态指示 `IDLE/EXEC/PLAN/FRONT` 改为彩色点 + 中文短语,间距/字号/对齐精修
- `crates/hoi4-ui/src/lib.rs` —— 导出四个新模块
- `crates/hoi4-app/src/main.rs` —— 调用 `ArmyDetailPanel::show` 和 `ArmyBadge::show`(在 `MilitaryPanel::show_bottom_bar` 调用前后)

## 不变量

- 不改 `MilitaryData` 数据结构
- 不改 `MilitaryCommand` 枚举
- 不引入 `hoi4-state` 新字段,无存档迁移
- 现有底栏托盘的画线模式提示行为不变
- 现有侧栏军事面板(`show_side_panel`)保留不动(它是总览面板,与新详情面板不冲突)

## 分阶段

### Phase A:立绘 widget + 工具按钮重做

任务:
- 新建 `portrait.rs`,导出:
  ```rust
  pub fn draw_general_portrait(
      ui: &mut egui::Ui,
      name: Option<&str>,
      size: egui::Vec2,
      style: PortraitStyle, // Small/Medium/Large
  ) -> egui::Response;
  ```
  - 暗金渐变背景(`PANEL_CARD` → `PANEL_CARD_SOFT`)
  - 中央大字渲染姓首字(`name.chars().next()`),12-32pt 看 size
  - 1px `GOLD` 描边
  - 无 name 时显示空槽 + 灰色 "?"
- 替换 `military.rs:252-272` 改调用 `portrait::draw_general_portrait`
- `tool_button` 改为复用 `components::action_button`(已有于 `components.rs`),配中文短词(`画线` `箭头` `执行` `停止` 等)
- 状态短词 `IDLE/EXEC/PLAN/FRONT` → `待命/执行/计划/前线`,彩色点保留

验收:
- 底部军团卡片立绘不再是小人
- 工具按钮文字可读
- `cargo check -p hoi4-ui` 通过

### Phase B:NATO 师符 widget

任务:
- 新建 `nato_icon.rs`,导出:
  ```rust
  pub enum NatoArchetype {
      Infantry, Cavalry, Motorized, Mechanized, Armor, Mountain,
      Marine, Paratrooper, Artillery, AntiTank, AntiAir, Garrison, Unknown,
  }
  pub fn archetype_from_template_name(name: &str) -> NatoArchetype;
  pub fn draw_nato_symbol(
      painter: &egui::Painter,
      rect: egui::Rect,
      archetype: NatoArchetype,
      country_color: egui::Color32,
  );
  ```
  - 外框矩形(填 country_color 衍生淡色 + 1px 暗描边)
  - 内符按 archetype 用 `line_segment` 画(步兵 X、骑兵 /、山地 ^、装甲 椭圆、机械化 椭圆+斜杠等)
- 依据 `DivisionEntry.name`(模板名)匹配 archetype,匹配不到落到 `Unknown`(空框)

验收:
- 单元测试:每个 archetype 渲染不 panic
- 视觉上能从图标区分主要兵种

### Phase C:左侧军团详情面板

任务:
- 新建 `army_detail_panel.rs::ArmyDetailPanel::show(ctx, data) -> Vec<MilitaryCommand>`
- 仅当 `data.selected_army_id.is_some()` 显示
- `egui::Area` 锚 `LEFT_TOP + (8, 60)`(让出顶栏),宽 300,高自适应,最高 60% 屏高,内部滚动
- 三段:
  1. **头部 (高约 90px)**:
     - 大立绘 64x80 `portrait::draw_general_portrait`
     - 集团军名(粗体金色)
     - 将领名 + 技能等级
     - 4 个紧凑加成: 攻+X% 防+Y% 计+Z% 后+W%
  2. **师列表**(滚动,每行 28px):
     - NATO 符号 24x18
     - 师名(可缩短,带模板)
     - 组织度条 80px(颜色按 frac:红/黄/绿)
     - 装备% 标签
     - 兵力% 标签
     - 在战中显示红色 `●`
  3. **底部摘要**(高约 30px):
     - "X 师 / 指挥上限 Y / 效率 Z%"
- 在 `main.rs` 装配:在已有 `MilitaryPanel::show_bottom_bar` 调用前调用

验收:
- 地图点击军团 → 左侧弹详情面板
- 详情面板与底栏共存,不互相遮挡
- 师列表可滚动
- 组织度低/装备低用 BAD/WARN 颜色标出
- 关闭(点 ✕ 或 ClearArmySelection)隐藏面板

### Phase D:右上军团徽章

任务:
- 新建 `army_badge.rs::ArmyBadge::show(ctx, data) -> Vec<MilitaryCommand>`
- 仅当 `data.selected_army_id.is_some()` 显示
- `egui::Area` 锚 `RIGHT_TOP - (16, 60)`(避开顶栏)
- 内容(宽 200,高 56):
  - 迷你立绘 32x40(复用 `portrait`)
  - 集团军名(粗体)
  - "X 师" + 状态点(彩色)
  - 状态短词(待命/执行/计划/前线)
- 整张可点击 → `SelectArmy`(已选中则取消)
- 右键 → `ClearArmySelection`

验收:
- 选中军团时右上有徽章
- 与原版 HoI4 战区徽章位置对位
- 风格与底栏卡片统一

## 推荐执行顺序

A → B → C → D

每阶段结束跑 `cargo check -p hoi4-ui && cargo check -p hoi4-app`,Phase C/D 后启动游戏在 Spain 1936 选个集团军手动验。

## 完成定义

- A 完成:立绘和工具按钮重做,无 circle+rect 小人,`cargo check` 通过
- B 完成:NATO 符号 widget 可用,单元测试通过
- C 完成:点军团弹左侧详情,有立绘 + 师 NATO 图标 + 组织度/装备/兵力条
- D 完成:选中军团时右上有摘要徽章
- 累计后,Spain 1936 截图与 vanilla 在"集团军呈现"上的差距显著缩小

## 风险

| 风险 | 缓解 |
|---|---|
| `egui::Area` 浮层挡住地图右键点击 | 详情面板宽 300 锚左侧,徽章宽 200 锚右上;避开地图主操作区中央 |
| 选中军团后没及时 `clear`,面板挂死 | 监听 `Escape`/外部 `ClearArmySelection`,清空状态 |
| 师列表项太多卡顿 | 滚动区限高 + `auto_shrink([false, false])`,Phase C MVP 不做虚拟滚动 |
| 工具按钮中文文字超过原 36x34 宽度 | `action_button` 自适应宽度,容器宽度由 `TOOLBAR_W` 放宽到 460 |

## 后续(本路线图外)

- 真实将领立绘:在 `hoi4-assets/portraits/generals/CTRY_*.png` 添加,`portrait.rs` 检测到则替换渐变
- Theater 数据层:`MILITARY_SYSTEM_HOI4_REWORK_ROADMAP.md` Phase 7+ 议
- 师模板 dock 化:`UI_PANEL_REWORK_ROADMAP.md` 后续阶段议
