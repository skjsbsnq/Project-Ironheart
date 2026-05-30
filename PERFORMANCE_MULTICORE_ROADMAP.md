# 1936 倍速越跑越卡：执行路线图与多核优化计划

## 当前结论

- 120 天 headless 基准：`102.9 ms/day`。
- 调度器目标：注释中要求 `1 day < 50 ms`。
- 当前最大瓶颈：`hoi4-logic` 的 V6 经济系统，占总模拟耗时约 `90%`。
- 直接表现：5 倍速每帧需要推进更多小时，但每日/每小时经济分摊已经超过预算，GUI 再叠加渲染、UI、单位 counter、箭头后会明显掉速。

## P0：低风险多核化

1. 给 `hoi4-logic` 引入 `rayon`。
2. 并行化 `collect_qualification_totals`。
3. 并行化 `construction_planner::plan_construction` 的州级候选评分。
4. 保持确定性：并行收集后继续统一排序候选。

## P1：索引化减少总工作量

1. 将经济 tick 中按国家过滤全局 POP 的循环迁到 `world.country_pop_index[ci]`。
2. 将经济 tick 中按国家过滤全局建筑的循环迁到 `world.country_building_index[ci]`。
3. 将财政 tick 中的 POP/建筑扫描迁到国家索引。
4. 给 `(state, building_def_id)` 建临时查询索引，替代 `current_building_level` 的全局线性扫描。

## P2：调度降频

1. 区分 daily/weekly/monthly 经济子系统。
2. 价格、GDP、汇率、阶级流动、教育、忠诚/激进度不应每天全国家全量跑。
3. 贸易撮合应按商品短缺国家或贸易区候选集合跑，不应全国家全商品重算。

## P3：长期增长控制

1. 给 AI 建造与私人投资增加更严格的队列上限。
2. 对小国、无产业国家、无战争国家降低经济和 AI 频率。
3. 监控 POP group 数、建筑数、师团数、路径缓存、玩家/AI battleplan 数。

## 验证标准

1. 每次优化后跑：`cargo run --release -p hoi4-app -- --headless --headless-days 120`。
2. 目标 1：`ms/day` 低于 `70 ms/day`。
3. 目标 2：经济系统占比从 `90%` 降到 `70%` 以下。
4. 目标 3：最终达到或接近 `50 ms/day`。
5. GUI 侧看 `[perf]` 日志，确认 `sim d/s` 稳定，`counter/arrows/ui/render` 没有成为新瓶颈。

## 注意事项

- 多核化只能降低 wall-clock 时间，不能减少总计算量。
- 真正解决“越跑越卡”还必须做索引化和增长控制。
- 所有并行化必须保持结果排序稳定，不能依赖线程执行顺序。
