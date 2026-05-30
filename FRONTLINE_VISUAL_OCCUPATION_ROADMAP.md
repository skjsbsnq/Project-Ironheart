# 战线视觉、画线手感与占领颜色改进路线图

本文档记录当前战线系统的主要问题、根因判断和分阶段改进方案。目标是先用小改动修复最影响观感的问题，再逐步把玩家战线从“省份质心折线”升级为“贴合真实敌我边界的地图表面线”。

## 当前问题

### 1. 玩家战线不能自然贴合地形和边界

当前玩家战线的逻辑路径是 `Vec<ProvinceId>`，视觉层通过省份质心生成折线：

- 数据结构：`crates/hoi4-state/src/frontline.rs`
- 吸附与校验：`crates/hoi4-logic/src/military/frontline.rs`
- 视觉转换：`crates/hoi4-app/src/main.rs::visual_path_from_provinces`
- 显示 pass：`crates/hoi4-app/src/passes/maparrow.rs`

这会导致战线穿过省份内部，即使加了 Catmull-Rom 平滑，也只是“更平滑的质心线”，不是沿真实边界。

### 2. 画线手感不舒服

当前拖线采样逻辑每约 33ms 取一次鼠标下省份 ID：

- `crates/hoi4-app/src/main.rs` 的 `WindowEvent::CursorMoved` 分支
- 采样结果写入 `frontline_painter.samples: Vec<ProvinceId>`
- 松开鼠标后才调用 `set_frontline_path` 或 `set_arrow`

问题：

- 鼠标真实轨迹没有保存。
- 大省内移动缺少连续反馈。
- 快速拖动可能漏采。
- 预览线和最终吸附结果都依赖省份质心。

### 3. 战线和箭头不能完全贴地形

玩家战线和箭头只在省份质心点采样 heightmap，线段中间是直线插值。跨山地、河谷、海岸或陡坡时，容易悬空或穿地。

相关位置：

- `crates/hoi4-app/src/main.rs::visual_path_from_provinces`
- `crates/hoi4-app/src/main.rs::push_visual_path_instances`
- `crates/hoi4-app/src/main.rs::push_arrow_spine_instances`

### 4. 占领颜色不符合 HOI4 原版观感

当前政治地图按法理拥有者 `owner` 上色：

- `crates/hoi4-render/src/map_mode.rs::fill_political`

占领效果另用红/灰条纹叠加：

- `crates/hoi4-render/src/map_mode.rs::build_occupation_lut`
- `crates/hoi4-app/src/passes/terrain.wgsl` 的占领条纹混色

而 HOI4 原版政治图更接近“被占领省份直接显示当前控制者颜色”。因此应优先按 `controller` 给政治地图染色，再用弱提示表达“这是被占领地”。

## 总体目标

- 政治地图中，被占领省份底色显示占领国颜色。
- 玩家战线视觉贴合真实敌我接触边，而不是省份质心。
- 玩家画线预览跟手，吸附结果可预测。
- 战线和箭头贴地稳定，不明显悬空或穿地。
- 全局战争前线、玩家战线、进攻箭头有清晰视觉层级。
- 不破坏现有战斗逻辑和 `FrontlineOrder.path` 存档结构。

## Phase 1：占领颜色快速修正

目标：先把最明显的地图颜色错误修掉。

任务：

- 将 `fill_political` 从使用 `world.provinces.owners` 改为优先使用 `world.provinces.controllers`。
- 将 `fill_ideology` 同步改为按 controller 所属国家执政党上色，避免政治图和意识形态图表达冲突。
- 调整 `build_occupation_lut`：政治图下不再使用强红色覆盖，最多保留弱斜纹或弱边缘提示。
- 确认 province controller 变化后会刷新 `color_lut_texture` 与 `occupation_lut_texture`。
- 如 diplomacy border/frontline 依赖 controller，也纳入同一 dirty refresh 机制。

验收标准：

- 德国占领法国省份时，政治地图中该省份底色为德国颜色。
- 被占领地仍能通过弱纹理或其他 UI 信息看出不是核心领土。
- 省份易手后地图颜色不会滞后一整局或只在重启后更新。

风险：

- 某些面板或地图模式可能仍希望显示法理 owner，需要区分“政治控制色”和“法理拥有者”。
- 如果 LUT 只在启动时上传，需要补充运行时刷新路径。

## Phase 2：动态地图资源刷新

目标：避免占领、宣战、停战后屏幕仍显示旧颜色或旧前线。

任务：

- 增加地图视觉 dirty 标记，例如 `map_visual_dirty` 或细分为 `color_lut_dirty`、`occupation_dirty`、`frontlines_dirty`。
- 在 province controller 改变时标记 dirty。
- 在外交关系改变时标记 diplomacy border/frontline dirty。
- 在渲染前统一检查 dirty，重建并上传：
  - `color_lut_texture`
  - `occupation_lut_texture`
  - `diplomacy_border_texture`
  - `frontlines_buffer`
- 避免每帧无条件重建大型纹理。

验收标准：

- 部队推进导致省份易手后，政治色和占领效果在下一帧或下一个 tick 内刷新。
- 宣战/停战后外交边界和战争前线同步刷新。
- 无明显帧率尖峰或每帧重复上传。

风险：

- controller 改变点可能分散在多个系统中，需要集中封装或保守标记。
- `frontlines_buffer` 从 province bitmap 扫描生成，重建成本需评估。

## Phase 3：玩家战线视觉边界化

目标：玩家战线不再显示省份质心折线，而是贴合敌我真实接触边。

任务：

- 保留 `FrontlineOrder.path: Vec<ProvinceId>` 作为逻辑和存档结构。
- 新增视觉转换：从 order path 提取对应的敌我接触边界。
- 复用 `crates/hoi4-render/src/frontlines.rs` 的 province bitmap 扫描思路，但限制到指定 army/order。
- 对接触边做聚合、排序、简化和平滑。
- 用 triangle strip 渲染玩家战线，逐步从 `MapArrowPass` 中拆出 frontline 本体。

验收标准：

- 玩家战线沿省份真实边界，不穿过省份中心。
- 同一条战线在山地、海岸、河流附近不会明显偏离接触边。
- 战线视觉变化不影响师分配、移动和存档。

风险：

- province bitmap 边界会产生大量短边，需要合并和简化。
- 玩家 path 是省份序列，而真实边界可能是多段断裂 polyline，需要处理断点。

## Phase 4：贴地形高度细分

目标：减少玩家战线和进攻箭头悬空、穿地。

任务：

- 对每条视觉线段按世界距离细分采样。
- 每个采样点查询 heightmap，而不是只在省份质心采样。
- 统一用于玩家战线预览、已保存战线和进攻箭头。
- 控制最小段长，避免实例数量爆炸。

验收标准：

- 山地和海岸附近线条明显更贴地。
- 长箭头不再只靠首尾高度插值。
- 性能成本可控。

风险：

- CPU 细分会增加 `ArrowInstance` 数量。
- 如果继续使用 quad 段，极端地形上仍可能出现轻微漂浮，长期应考虑 decal/terrain overlay。

## Phase 5：画线交互重做

目标：让玩家画线更跟手、更可预测。

任务：

- `FrontlinePainterState` 同时保存：
  - 鼠标射线打到地形后的世界坐标轨迹。
  - 省份 ID 采样结果。
- 拖动时用世界坐标轨迹显示即时预览。
- 鼠标附近查找最近合法前线边界，而不是只取当前省份。
- 合法段高亮，不合法段灰/红提示。
- 松手后将世界轨迹投影为合法 `ProvinceId` path，再调用现有 `set_frontline_path`。

验收标准：

- 大省内拖动也有连续视觉反馈。
- 快速拖动不容易丢线。
- 最终吸附结果和预览基本一致。
- 不合法区域反馈清晰。

风险：

- 需要新增“鼠标世界点到最近合法边界”的查询。
- 预览和最终逻辑吸附如果算法不一致，会造成用户困惑。

## Phase 6：箭头系统独立升级

目标：把进攻箭头从“分段 quad”升级为更接近军事计划箭头的视觉。

任务：

- 箭头从战线边界 anchor 点出发，而不是省份质心。
- 使用平滑 spine 生成 body strip。
- 单独生成 arrow head mesh，不只依赖最后几个 segment 标记。
- tail/body/head 视觉参数分离。
- 与玩家战线 pass 分离，避免层级混乱。

验收标准：

- 箭头头部方向稳定，短箭头也不会乱摆。
- 转角处连续，不明显断裂。
- 执行中箭头、预览箭头、已保存战线视觉差异清楚。

风险：

- 如果没有贴图资源，procedural shader 需要更多调参。
- 曲线过度平滑可能穿越不相关省份，需要限制偏移。

## 推荐优先级

1. Phase 1：占领颜色按 controller 染色。
2. Phase 2：地图资源 dirty refresh。
3. Phase 4：线段高度细分，先止血贴地问题。
4. Phase 3：玩家战线边界化。
5. Phase 5：画线交互重做。
6. Phase 6：箭头系统独立升级。

## 最小首批改动范围

建议第一批只做这些：

- `fill_political` 改 controller 色。
- `fill_ideology` 改 controller 色。
- `build_occupation_lut` 弱化红色占领条纹。
- controller 变化后刷新颜色和占领 LUT。
- 玩家战线/箭头生成前做高度细分采样。

这批改动风险较低，但能最快改善两个核心观感问题：占领颜色错误、线条不贴地。
