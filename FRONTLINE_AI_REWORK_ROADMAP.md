# 战线视觉与 AI 战斗重构路线图

本文档记录当前战线划线、箭头渲染与 AI 战斗行为的主要问题、根因和分阶段改进路线。

## 当前结论

当前问题不是单纯的美术参数或单个 AI 阈值错误，而是三个系统都仍带有原型实现特征：

- 战线视觉使用省份质心折线，无法贴合真实前线边界。
- 箭头使用简单分段 quad，没有真实曲线、贴图、头尾融合和稳定视觉层级。
- AI 决策主要基于国家总战力按前线州数量分摊，缺少局部战术态势、突破点规划和执行反馈。

根本问题是目前把“视觉战线”“逻辑战线”“执行战线”都压缩成了省份 ID 路径和 destination 分配。

## 现状定位

### 静态前线渲染

相关文件：

- `crates/hoi4-render/src/frontlines.rs`
- `crates/hoi4-render/src/frontlines.wgsl`
- `crates/hoi4-app/src/main.rs`

主要问题：

- `generate_frontline_vertices` 直接连接相邻交战省份的 centroid。
- 使用 `LineList`，没有宽度、圆角、羽化、描边、阴影或纹理。
- 前线不贴真实边界，会穿过省份内部。
- 玩家拖线最终只保留省份 ID 序列，丢失鼠标轨迹。

### 箭头渲染

相关文件：

- `crates/hoi4-app/src/passes/maparrow.rs`
- `crates/hoi4-render/src/translations/maparrow.wgsl`
- `crates/hoi4-app/src/main.rs`

主要问题：

- `MapArrowPass` 使用程序化 quad 段，仍保留 mock/test 管线痕迹。
- 箭头路径来自省份质心折线，没有曲线插值。
- 头部只是最后一段 `segment_type = 1.0`，转角处非常突兀。
- 前线线段和进攻箭头共用同一 pass，视觉层级混乱。

### AI 战斗决策

相关文件：

- `crates/hoi4-ai/src/ground.rs`
- `crates/hoi4-ai/src/ground_orders.rs`
- `crates/hoi4-ai/src/frontline_ai.rs`
- `crates/hoi4-ai/src/frontline.rs`
- `crates/hoi4-logic/src/military/frontline.rs`

主要问题：

- `evaluate_ground` 用国家总战力按前线州数量分摊，缺少局部兵力判断。
- 攻防姿态主要依赖单个 ratio。
- 包围机会只看州邻接比例，不看实际补给线、退路和可达性。
- `pick_attack_targets_impl` 每个敌州只取第一个可进入省份，再按 VP/工厂/驻军粗略排序。
- `assign_assault_role` 本质是分配 destination，不是完整作战计划。
- `frontline_distributor` 虽有集中突破修补，但仍主要按敌军数量做机械排序。

## 总体目标

### 视觉目标

- 战线贴合真实敌我接触边界。
- 线条有厚度、层次、可读性和缩放稳定性。
- 箭头具备平滑曲线、头尾融合、方向清晰和军事地图风格。
- 玩家划线反馈要即时、自然，最终逻辑路径和视觉路径分离。

### AI 目标

- AI 从“前线级 ratio 判断”升级为“局部 sector 态势判断”。
- AI 能选择合理突破点，而不是把全线当作同质目标。
- AI 能根据失败、组织度、补给和敌军变化调整计划。
- AI 执行层有明确状态机，而不是反复覆盖 destination。

## Phase 1：视觉快速止血

目标：用最小改动显著改善战线和箭头观感。

任务：

- 将玩家/AI 战线路径从裸 centroid 折线改为平滑 polyline。
- 对省份路径做 Catmull-Rom 或简单 Chaikin smoothing。
- 在 `MapArrowPass` 前生成更多采样点，避免长直折线。
- 增加路径去抖和最小段长过滤，减少短碎线。
- 调整箭头宽度、透明度和颜色，区分 frontline/body/head。

验收标准：

- 玩家拖出来的线不再呈现明显锯齿折线。
- 箭头转角处不再出现生硬断裂。
- 不改变战斗逻辑，只改变视觉实例生成。

风险：

- 只处理 centroid 路径仍无法贴真实边界。
- smoothing 可能让视觉线穿越不相关区域，需要限制曲线偏移。

## Phase 2：前线真实边界化

目标：静态战争前线不再用省份中心连线，而是贴合真实接触边。

任务：

- 从 province bitmap 或现有 border extraction 中提取敌我接触边。
- 将相邻边聚合成前线 polyline。
- 对 polyline 做简化、平滑和断点合并。
- 用 triangle strip 替代 `LineList` 渲染前线。
- shader 增加 feather、outline、内外侧颜色或轻微脉冲。

验收标准：

- 战线沿真实省份边界显示。
- 战线不会穿过省份中心。
- 缩放时粗细稳定，可读性优于当前 `LineList`。

风险：

- 边界提取和聚合可能需要处理大量短边。
- 地图投影、海岸线和湖泊边界需要过滤。

## Phase 3：箭头系统重做

目标：进攻箭头从“分段 quad”升级为真实军事计划箭头。

任务：

- 将 arrow path 转为曲线 spine。
- 沿 spine 采样生成 body strip。
- 单独生成 arrow head mesh，不再只依赖最后一段。
- 支持 tail/body/head 三段视觉资源。
- 尝试接入 `translations/maparrow.wgsl` 的贴图思路，或实现 SDF procedural arrow。
- 前线、进攻箭头、预览线分离颜色和 pass 参数。

验收标准：

- 箭头头部始终沿整体路径末端方向，不随短段乱摆。
- 长箭头有连续流动感，短箭头也能保持明确方向。
- 玩家绘制预览、已保存前线、执行中箭头有清晰视觉差异。

风险：

- 如果没有美术贴图，procedural 方案需要 shader 反复调参。
- 曲线和地形高度采样要避免 z-fighting。

## Phase 4：AI 局部战术评估

目标：AI 不再只用国家总战力分摊，而是理解局部前线态势。

任务：

- 引入 `FrontSector` 概念，按相邻接触省份聚类。
- 每个 sector 统计己方师、敌方师、组织度、强度、攻击面、地形和补给。
- 替换或补强 `evaluate_ground` 的 ratio 计算。
- `pick_attack_targets_impl` 改为只选真实邻接、可支援、多方向可攻击的目标。
- 给目标评分加入敌军强度、地形惩罚、补给、距离和 VP/工厂价值。

验收标准：

- AI 不会因为全国 ratio 高就盲攻所有方向。
- AI 能倾向选择低敌军密度、高攻击面、可达的突破点。
- 战术日志能输出每个 sector 的评分原因。

风险：

- sector 聚类和局部统计会增加计算量，需要缓存。
- 需要避免 AI 过度保守导致不进攻。

## Phase 5：AI 作战计划状态机

目标：AI 从“每轮分配目的地”升级为“有阶段的作战计划”。

任务：

- 为每个前线/sector 引入 operation state：`Preparing`、`Attacking`、`Exploiting`、`Recovering`、`Paused`。
- `Preparing` 阶段先集结师团和恢复组织度。
- `Attacking` 阶段同步多方向攻击目标省。
- `Exploiting` 阶段根据突破结果自动延伸目标。
- `Recovering` 阶段失败后停止消耗并恢复组织度。
- `Paused` 阶段记录失败方向，短期内降低该方向评分。

验收标准：

- AI 不再频繁覆盖 destination 造成来回踱步。
- AI 攻击失败后会暂停或换方向，而不是无限撞墙。
- AI 成功突破后能继续扩大战果，而不是回到静态守备。

风险：

- 需要梳理 `tick_ai_frontlines`、`execute_ground_orders`、`tick_frontlines` 的控制权边界。
- 状态机需要存档支持。

## Phase 6：清扫与主攻分离

目标：避免 mop-up 抢走主攻部队或主攻部队忽略残敌。

任务：

- 将师团分为 Assault、Reserve、Garrison、MopUp 四类职责。
- 主攻计划锁定核心师团，清扫只使用空闲或低优先级师团。
- 残敌清扫目标按包围风险、补给切断和距离排序。
- 防止所有师团被 residual target 拉走。

验收标准：

- 主攻方向不会因为后方残敌反复中断。
- 被包围残敌会被合理清扫。
- 战争后期不会出现大批师团原地空闲。

风险：

- 分工过细可能导致小国师团不足时无法执行全部职责。

## 建议实施顺序

1. Phase 1：先改善玩家能立即看到的线条和箭头。
2. Phase 2：把静态前线贴到真实边界。
3. Phase 4：重做 AI 目标选择，先提高“会不会打”的质量。
4. Phase 5：再引入完整作战状态机。
5. Phase 3：在视觉路线稳定后完善箭头 pass。
6. Phase 6：最后处理清扫、主攻和预备队职责分离。

## 首个可执行任务建议

最适合先做的是 Phase 1：平滑玩家/AI 前线和箭头路径。

理由：

- 改动范围较小，主要集中在 `collect_frontline_arrows` 前后的实例生成。
- 不改变存档格式和战斗逻辑。
- 能快速验证视觉方向。
- 后续 Phase 2/3 可以复用曲线采样与 strip 构建思路。

最小实现点：

- 新增一个从 `Vec<ProvinceId>` 到 `Vec<[f32; 3]>` 的路径采样函数。
- 对 centroid 点列做去重、最小距离过滤和 Catmull-Rom 采样。
- 用采样结果生成 `ArrowInstance` 段。
- 对 frontline path 和 arrow path 使用不同宽度与 segment type。
