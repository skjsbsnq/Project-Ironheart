# 1936 倍速越跑越卡：性能优化交接文档

## 背景

问题表现：1936 年开局后开始倍速推进，时间一走动起来，越往后越卡。

经过源码排查和 headless 基准，确认核心瓶颈不是单纯渲染，而是模拟层，尤其是 V6 经济系统。GUI 里渲染、UI、单位 counter、箭头等会继续叠加成本，但 headless 已经能复现模拟层超预算。

## 关键结论

- 当前最大瓶颈是 `hoi4-logic` 的 V6 经济系统。
- 经济系统长期占总模拟耗时约 `90%`。
- 原始 120 天 headless 基准约 `102.9 ms/day`，已经超过调度器注释目标 `1 day < 50 ms`。
- 5 倍速每帧需要推进更多游戏小时，实际模拟日耗时过高时，UI 就会表现为速度跑不满、帧率下降、越跑越卡。
- 多核不是万能：细粒度 Rayon 化可能严重变慢。真正有效的是先减少重复全局扫描和总工作量。

## 已保存的路线图

已有路线图文件：

`PERFORMANCE_MULTICORE_ROADMAP.md`

内容包括：

- P0 低风险多核化
- P1 索引化减少总工作量
- P2 调度降频
- P3 长期增长控制
- 验证标准

本文件是新会话交接用的更具体执行记录。

## 已做改动

### 1. 引入 Rayon

文件：`crates/hoi4-logic/Cargo.toml`

改动：

```toml
rayon = "1.10"
```

原因：让逻辑层也能做多核并行。之前只有 `hoi4-ai` 使用 Rayon。

注意：当前保留了 Rayon 依赖，但不是所有热点都适合并行。

### 2. 建造规划按州并行

文件：`crates/hoi4-logic/src/economy/construction_planner.rs`

改动：

- `plan_construction` 按 state 并行生成本地 `ConstructionPlanResult`。
- 最后合并 `candidates` / `rejected`。
- 候选仍统一排序，避免线程执行顺序影响结果确定性。

目的：AI/私投建造规划属于粗粒度只读评分，比细粒度 POP 扫描更适合 Rayon。

效果：120 天早期基准中收益不明显，但对后期建筑数量增多、规划更频繁时有价值。

### 3. 建造规划建筑等级索引

文件：`crates/hoi4-logic/src/economy/construction_planner.rs`

新增：

```rust
fn building_level_index(world: &World) -> HashMap<(u16, String), u8>
fn current_building_level(...)
```

原问题：

`current_building_level` 原来每个候选都会扫描全局建筑列表：

```text
国家拥有州数 * 建筑定义数 * 全局建筑数
```

新逻辑：

- 先构建 `(state, building_def_id) -> level`。
- 候选评分时 O(1) 查询。

目的：避免后期建筑越来越多后，AI/私投规划越来越慢。

### 4. 财政建筑扫描迁移到国家建筑索引

文件：`crates/hoi4-logic/src/economy/finance_tick.rs`

新增：

```rust
fn country_building_indices(world: &World, ci: usize) -> Vec<usize>
```

迁移了财政 tick 中多处按国家过滤全局建筑的循环，包括：

- 建筑利润税收扫描
- 国家工资支出扫描
- 建造部门 CP 扫描
- GDP 建筑 value-added 扫描

后来在 `building_tick_common.rs` 中也新增了通用 `country_building_indices`。后续可考虑让 `finance_tick.rs` 也复用通用 helper，减少重复代码。

### 5. 建筑资格统计缓存

文件：`crates/hoi4-logic/src/economy/mod.rs`

新增字段：

```rust
qualification_totals_cache_hour: u64,
qualification_totals_cache: Vec<QualificationTotals>,
```

新增方法：

```rust
pub(crate) fn qualification_totals(&mut self, world: &World) -> &[QualificationTotals]
pub(crate) fn invalidate_qualification_totals(&mut self)
```

文件：`crates/hoi4-logic/src/economy/market_tick.rs`

改动：

- `step_building_production` 使用 `econ.qualification_totals(world)`。
- `step_military_production` 使用缓存。
- `step_pop_employment` 后调用 `econ.invalidate_qualification_totals()`。

文件：`crates/hoi4-logic/src/economy/planned_tick.rs`

改动：

- `step_quota_production` 使用缓存。
- `step_military_production_planned` 使用缓存。
- 计划经济就业变动后调用 `econ.invalidate_qualification_totals()`。

原问题：

`collect_qualification_totals(world)` 会全局扫描 POP group，并构建每个建筑的资格统计。原来多个国家/子系统会在同一经济小时内重复调用。

现在：

- 同一 `world.elapsed_hours` 内只计算一次。
- 如果就业 tick 改变了 POP 分配，立即失效缓存。

效果：这是目前最明确有效的优化。

### 6. 去掉资格缓存调用处的临时 Vec 拷贝

文件：

- `crates/hoi4-logic/src/economy/market_tick.rs`
- `crates/hoi4-logic/src/economy/planned_tick.rs`

初版为了规避借用冲突使用了：

```rust
econ.qualification_totals(world).to_vec()
```

后续改成短作用域借用缓存，减少额外分配。

### 7. 通用国家建筑索引 helper

文件：`crates/hoi4-logic/src/economy/building_tick_common.rs`

新增：

```rust
pub(crate) fn country_building_indices(world: &World, ci: usize) -> Vec<usize>
```

使用位置：

- `market_tick.rs`
- `planned_tick.rs`

逻辑：

- 优先使用 `world.country_building_index[ci]`。
- 如果索引为空，fallback 到按 state owner 过滤全局建筑，避免初始化时序问题。

### 8. 市场/计划经济建筑生产循环迁移到国家建筑索引

文件：`crates/hoi4-logic/src/economy/market_tick.rs`

迁移：

- `step_building_production`
- `step_military_production`

文件：`crates/hoi4-logic/src/economy/planned_tick.rs`

迁移：

- `step_quota_production`
- `step_military_production_planned`

目的：避免每个国家重复遍历全局建筑列表再过滤。

## 已验证命令

每轮改动均运行过类似命令：

```bash
cargo check -p hoi4-logic
cargo fmt
cargo run --release -p hoi4-app -- --headless --headless-days 120
```

现有 warning 基本是项目既有未使用项，不是本轮优化引入的编译错误。

## 基准记录

### 原始排查基准

```text
120 days ticked in 12.345s (102.9 ms/day)
econ=11060.4ms/90%/2880x
```

### 细粒度 Rayon 化失败案例

曾尝试把 `collect_qualification_totals` 直接改成 Rayon 并行 fold/reduce。

结果：

```text
388.2 ms/day
econ=45307.8ms/97%/2880x
```

结论：这是错误方向，已回滚。

原因：该函数调用频率高，且每线程分配大 Vec，Rayon 任务切分和分配成本远大于收益。

### 回滚细粒度 Rayon 后

```text
102.5 ms/day
econ=11121.8ms
```

### 财政建筑索引 + 建造规划索引后

```text
103.9 ms/day
econ=11251.9ms
```

该结果在噪声范围内，没有明显收益，但该改动对后期建筑增长有长期意义。

### 建筑资格统计缓存后

```text
99.5 ms/day
econ=10692.5ms
```

### 去掉资格缓存临时 Vec 拷贝后

```text
97.2 ms/day
econ=10480.9ms
systems ms/day=85.8
```

### 市场/计划经济建筑循环迁移到国家建筑索引后

```text
96.2 ms/day
econ=10352.2ms
systems ms/day=86.2
```

### POP 索引安全维护 + 财政/工资 POP 循环索引化后

新增/改动：

- `crates/hoi4-state/src/world.rs` 新增 `World::push_pop_group(...)`，集中 push POP group 并同步追加 `country_pop_index`。
- `crates/hoi4-logic/src/economy/building_tick_common.rs` 新增通用 `country_pop_indices(...)`，优先用国家 POP 索引，索引为空时 fallback 全局扫描。
- `finance_tick.rs` 复用通用 `country_building_indices`，删除本地重复 helper。
- `finance_tick.rs` 的 `step_collect_taxes_planned`、`step_collect_taxes`、`step_pay_military_upkeep`、`step_welfare_spending` 改为遍历本国 POP 索引。
- `finance_tick.rs` 企业税建筑扫描改为国家建筑索引。
- `market_tick.rs` / `planned_tick.rs` 的就业 split、阶级流动新增 POP 改为 `world.push_pop_group(...)`。
- `law_modifiers.rs` 新增 Soldier POP 改为 `world.push_pop_group(...)`。
- `market_tick.rs` / `planned_tick.rs` 的 `step_pop_wage` 改为本国 POP 索引。
- 移除 `market_tick.rs` 的 `step_class_mobility` 末尾每国触发的全局 `compact_similar_pop_groups()`；保留 `economy/mod.rs` 中每 30 天全局 compact。

120 天结果：

```text
85.3 ms/day
econ=9000.6ms/88%/2880x
systems ms/day=84.7
```

阶段效果：相对 `96.2 ms/day` 下降约 `11.3%`，经济累计耗时下降约 `1351.6ms / 120天`。

365 天当前基线：

```text
365 days ticked in 31.485s (86.3 ms/day)
econ=27777.5ms/3.17avg/88%/8760x
systems ms/day=84.1
```

结论：365 天 `ms/day` 与 120 天 `85.3 ms/day` 接近，1936 年内没有明显线性恶化；下一阶段主要目标是继续压低固定经济 tick 成本，并关注更长期（730 天）是否出现 POP group/建筑/私投增长导致的趋势性上升。

### market/planned 低风险 POP 状态循环索引化后

新增/改动：

- `market_tick.rs` 的 `step_pop_income_from_existing_wage` 改为本国 POP 索引。
- `market_tick.rs` 的 `step_pop_consumption_demand` 改为本国 POP 索引；为避免同时可变借用 POP 和 market，先收集 demand additions，再统一写入 market。
- `market_tick.rs` 的 `step_pop_satisfaction_from_clearing`、`step_pop_satisfaction`、`step_pop_radicalism`、`step_pop_loyalty` 改为本国 POP 索引。
- `planned_tick.rs` 的 `step_rationing`、`step_pop_satisfaction_from_clearing`、`step_pop_radicalism_planned`、`step_pop_loyalty` 改为本国 POP 索引。

365 天结果：

```text
365 days ticked in 29.152s (79.9 ms/day)
econ=25521.1ms/2.91avg/88%/8760x
systems ms/day=78.5
```

阶段效果：相对 365 天基线 `86.3 ms/day` 下降约 `7.4%`；经济累计耗时下降约 `2256.4ms / 365天`。

### law modifiers POP 循环索引化后

新增/改动：

- `law_modifiers.rs` 的每日法律满意度/忠诚系数写入改为 `country_pop_indices(...)`。
- `law_modifiers.rs` 的征兵容量统计和转换扫描改为 `country_pop_indices(...)`。
- 删除 `law_modifiers.rs` 已无用的本地 `state_owned_by_parts` helper。

365 天结果：

```text
365 days ticked in 29.893s (81.9 ms/day)
econ=26191.6ms/2.99avg/88%/8760x
systems ms/day=79.2
```

注意：365 天 headless 当前存在较明显运行噪声。`market/planned` 低风险 POP 状态循环索引化后曾出现一次 `79.9 ms/day`，后续复跑在 `84-86 ms/day` 区间。`law_modifiers` 索引化后结果为 `81.9 ms/day`，可视为相对正式 365 天基线 `86.3 ms/day` 有改善，但后续仍应以多次 365 天或更长 730 天结果确认趋势。

已尝试但撤回：

- `market_tick.rs` / `planned_tick.rs` 的 `step_pop_education` 改为索引化。
- `planned_tick.rs` 的 `step_pop_employment` 外层建筑循环改为 `country_building_indices(...)`。

原因：365 天结果回归到 `84.1` / `85.9 ms/day`，未证明收益，已撤回。不要把所有全局扫描机械替换成索引；部分路径里 clone 索引、小 Vec 分配、额外 indirection 可能抵消收益。

## 当前累计效果

从原始：

```text
102.9 ms/day
econ=11060.4ms
```

到当前：

```text
85.3 ms/day
econ=9000.6ms
```

累计改善：

- 总模拟耗时约提升 `17.1%`。
- 经济累计耗时下降约 `2059.8ms / 120天`。
- 经济仍占 `88%`，说明主战场仍在经济系统，但 POP 索引迁移已明显降低重复全局扫描成本。

## 重要踩坑

### 1. 不能盲目多核化

细粒度 Rayon 化 `collect_qualification_totals` 会严重变慢。多核化适合：

- 粗粒度任务
- 只读评分
- 每个任务足够重
- 不频繁分配大临时数组

不适合：

- 每小时/每国高频小任务
- 每线程都分配大 Vec
- 需要频繁 reduce 大数组

### 2. POP 索引迁移不能直接做（已完成安全入口）

`market_tick.rs` 的就业逻辑会 split POP group：

```rust
world.countries.pops.groups.push(hired_group);
```

原先 `world.country_pop_index` 是重建型索引，不会在同 tick 内自动包含新 split 出来的 POP group。

如果直接把就业/工资/税收循环改成 `country_pop_index[ci]`，可能漏掉同 tick 新增 POP group，改变经济行为。

因此 POP 索引迁移前必须先设计：

- split 时同步更新 `country_pop_index`
- 或者延迟 split 到 tick 末统一应用
- 或者为经济 tick 使用临时本地 POP 索引并在 push 时同步追加

当前已采用方案 2：`World::push_pop_group(...)` 集中维护索引，并已替换经济 tick 内主要 split/push POP 路径。后续迁移 POP 循环时优先使用 `building_tick_common::country_pop_indices(...)`，不要直接 clone `world.country_pop_index[ci]`，以保留 fallback 行为。

### 3. 120 天基准主要反映早期和平经济

建造规划索引、AI 建造并行、建筑索引在早期 120 天收益不一定明显，但对后期建筑数量增长、私投开启、AI 规划频繁时更有价值。

## 当前还没解决的核心问题

### 1. 大量 POP 循环仍在全局扫描

热点仍包括：

- `market_tick.rs`
- `planned_tick.rs`
- `law_modifiers.rs`
- `stockpile.rs`

很多函数仍然是：

```rust
for pg in &mut world.countries.pops.groups {
    if !state_owned_by_parts(...) { continue; }
    ...
}
```

这导致复杂度接近：

```text
国家数 * 全局 POP group 数 * 多个经济步骤
```

这是下一阶段最大的优化空间。

### 2. 调度仍然高频

经济系统注册为 hourly spread：

`crates/hoi4-runtime/src/schedule.rs`

```rust
s.register(SystemId::Economy, Cadence::Hourly, economy_hourly_spread);
```

这只是分摊负载，并没有降低总工作量。真正降耗还需要：

- 税收、工资、价格、GDP、汇率、教育、忠诚/激进度分不同频率
- 小国低频 tick
- 无产业/无 POP/无战争国家跳过更多步骤

## 推荐下一步研究顺序

### Step 1：建立 365 天基准并转向长期增长瓶颈

120 天已经能看出经济系统改善，但当前目标改为 `365 天`。后续主基准命令改为：

```bash
cargo run --release -p hoi4-app -- --headless --headless-days 365
```

重点观察：

- `ms/day` 是否随时间、POP group、建筑、私投增长上升。
- `econ` 占比是否继续接近 90%。
- 365 天结果应优先低于当前代码基线，而不再只盯 120 天波动。

### Step 2：继续迁移 `market_tick.rs` / `planned_tick.rs` 的 POP 循环

已完成 `step_pop_wage`。下一批建议从风险较低、不会 split POP 的循环开始：

- `step_pop_income_from_existing_wage`
- `step_pop_consumption_demand`
- `step_pop_satisfaction_from_clearing`
- `step_pop_radicalism`
- `step_pop_loyalty`

`step_pop_employment` 和 `step_class_mobility` 改动风险更高，建议在 365 天基准稳定后单独处理。

### Step 3：检查 POP group 长期增长与 compact 策略

目标：验证移除每国 `step_class_mobility` 后的全局 compact 是否足够。当前仍有每 30 天 compact：

```rust
world.compact_similar_pop_groups();
```

如果 365 天显示 POP group 数长期增长导致变慢，应研究：

- 就业 split 是否产生过多无法合并的 wage/tax/literacy bucket。
- compact 频率是否要从 30 天调整，或在特定 POP 增长阈值触发。
- `compact_similar_pop_groups` 是否需要保持索引同步或集中重建。

### Step 4：设计可安全维护的 POP 索引（已完成入口，继续替换剩余 push）

目标：让同 tick 内 split 出来的 POP group 能进入国家索引。

可选方案：

1. 在所有 `pops.groups.push(...)` 处同步追加到 `country_pop_index[owner_ci]`。
2. 给 `World` 增加 `push_pop_group(...)` 方法，集中维护索引。
3. 经济 tick 内使用本地 `Vec<usize>`，split 时追加本地索引，tick 末重建全局索引。

推荐优先方案第 2 种已落地。后续如果修改非经济路径的 POP push，也应尽量改为 `world.push_pop_group(...)`，或在调用后重建 runtime indexes。

### Step 5：迁移 `finance_tick.rs` 的 POP 循环（已完成首批）

优先迁移相对简单的只读/简单写字段循环：

- `step_collect_taxes`
- `step_pay_military_upkeep` 的 Soldier POP 循环
- `step_welfare_spending`

注意：所有会 push/split POP 的路径要先完成索引同步。

### Step 6：迁移 `market_tick.rs` / `planned_tick.rs` 的高风险 POP 循环

这部分收益最大，但风险也最大。

重点函数：

- `step_pop_employment`
- `step_pop_wage`
- `step_pop_consumption_demand`
- `step_pop_satisfaction_from_clearing`
- `step_pop_radicalism`
- `step_pop_loyalty`
- `step_class_mobility`

建议每次只迁一个函数，跑 120 天和更长基准。

### Step 7：加入更长 benchmark

当前 120 天主要看早期和平期。建议新增测试：

```bash
cargo run --release -p hoi4-app -- --headless --headless-days 365
cargo run --release -p hoi4-app -- --headless --headless-days 730
```

重点观察：

- `ms/day` 是否随时间上升
- `econ` 占比
- 建筑数、POP group 数、师团数
- 私投开启后是否明显变慢

## 新会话开场建议

可以直接对新会话说：

```text
请读取 PERFORMANCE_OPTIMIZATION_HANDOFF.md 和 PERFORMANCE_MULTICORE_ROADMAP.md，继续优化 1936 倍速越跑越卡。当前目标改为 365 天基准下降；优先建立 365 天 headless 基线，然后继续迁移 market_tick/planned_tick 剩余 POP 循环，保持每轮改动后运行 cargo check、cargo fmt、365 天 headless benchmark。
```

## 当前验证基准命令

120 天快速回归仍可用：

```bash
cargo check -p hoi4-logic
cargo fmt
cargo run --release -p hoi4-app -- --headless --headless-days 120
```

当前 120 天参考结果：

```text
85.3 ms/day
econ=9000.6ms/88%/2880x
```

但下一阶段目标改为 365 天，主基准改用：

```bash
cargo run --release -p hoi4-app -- --headless --headless-days 365
```

当前 365 天基线：

```text
81.9 ms/day
econ=26191.6ms/88%/8760x
```

后续优化以 365 天下降为准。
 
 
## 2026-05-26 follow-up optimization record

Scope:
- `crates/hoi4-state/src/world.rs`
- `crates/hoi4-logic/src/economy/building_tick_common.rs`
- `crates/hoi4-logic/src/economy/market_tick.rs`
- `crates/hoi4-logic/src/economy/planned_tick.rs`

Changes:
- Added `World::runtime_country_indexes_valid`.
- `World::rebuild_runtime_country_indexes()` now marks runtime country indexes valid.
- `World::compact_similar_pop_groups()` now invalidates runtime country indexes after it rewrites POP storage.
- `country_pop_indices(...)` and `country_building_indices(...)` now trust empty indexes only when runtime country indexes are valid. This avoids treating legitimate empty countries as "index missing" and repeatedly falling back to global POP/building scans.
- `market_tick.rs` and `planned_tick.rs` class mobility paths now use per-country POP/building indexes for class counts, POP movement, average wage/tax lookup, and employment-demand totals.

Reason:
- The previous helper fallback used `filter(|indices| !indices.is_empty())`, so countries with no POPs/buildings performed full global scans in many high-frequency economy paths.
- With 431 countries, empty-country fallback scans were a large fixed cost. The new validity flag preserves fallback behavior for tests or pre-rebuild initialization while allowing hot paths to trust empty indexes after the daily/hourly rebuild.

Verification:

```text
cargo fmt
cargo check -p hoi4-logic
```

Warnings remain existing unused/dead-code warnings; no new compile errors.

Benchmark results:

```text
365 days, first release run after rebuild:
73.6 ms/day
econ=22575.9ms/2.58avg/84%/8760x
systems ms/day=69.6

365 days, warm release run:
69.9 ms/day
econ=21359.0ms/2.44avg/84%/8760x
systems ms/day=65.9

120 days, warm release run:
71.1 ms/day
econ=7143.5ms/2.48avg/84%/2880x
systems ms/day=66.9
```

Current practical 365-day baseline is now about `69.9 ms/day`, down from the previous recorded `81.9 ms/day`. Economy share is still high at about `84%`, but the roadmap `70 ms/day` stage target has been reached on the warm 365-day run.

Next recommended steps:
- Convert the most frequent remaining helper users from cloned `Vec<usize>` results to borrowed slices/Cow to remove repeated index Vec allocations.
- Audit direct writes to `world.countries.buildings_v6.buildings.push(...)`; if new buildings must be visible in the same tick, add a centralized `World::push_building_v6(...)` similar to `push_pop_group(...)` or invalidate/rebuild indexes after direct building pushes.
- Continue with 365-day primary benchmarks; use 120-day only as a quick regression check.
## 2026-05-26 second follow-up optimization record

Scope:
- `crates/hoi4-logic/src/economy/mod.rs`
- review context: `crates/hoi4-logic/src/economy/building_tick_common.rs`

Changes:
- Added an early return in `tick_country_daily_v6(...)` for countries with no owned states when runtime country indexes are valid.
- This skips market/planned economy, finance, trade, construction, and V6 event empty work for releasable/unlanded countries.
- Investigated returning borrowed index slices from `country_pop_indices(...)` / `country_building_indices(...)` to avoid Vec clones. That path was not kept because borrowed index slices conflict with the many same-loop mutable writes to POP groups and markets; keeping owned Vec results is currently the safer design unless call sites are split into explicit read-only and mutation phases.

Reason:
- The previous optimization removed empty-index global fallback scans, but unlanded countries still entered the full per-country economy pipeline.
- The economy tick rebuilds `country_state_index` before country ticks, so an empty state index is a reliable no-work signal in hot paths.

Verification:

```text
cargo fmt
cargo check -p hoi4-logic
```

Benchmark results:

```text
365 days, first release run after rebuild:
42.5 ms/day
econ=11058.8ms/1.26avg/71%/8760x
systems ms/day=38.5

365 days, warm release run:
38.4 ms/day
econ=9912.2ms/1.13avg/71%/8760x
systems ms/day=37.7

120 days, warm release run:
38.3 ms/day
econ=3215.5ms/1.12avg/70%/2880x
systems ms/day=36.6

730 days, warm release run:
40.5 ms/day
econ=21235.7ms/1.21avg/72%/17520x
systems ms/day=39.9
```

Current practical baseline:
- 120d: about `38.3 ms/day`
- 365d: about `38.4 ms/day`
- 730d: about `40.5 ms/day`

The original `1 day < 50 ms` scheduler target is now reached in headless benchmarks. Economy is still the largest single system at about `70-72%`, but absolute economy cost is now roughly `1.1-1.2 ms/tick avg` in these runs. Long-run 730d does not show a serious POP/building growth regression.

Next recommended steps:
- Shift attention to correctness risk around unlanded/exiled countries if future mechanics give them economy queues without owned states.
- For further economy gains, split selected POP routines into read-only collection and mutation phases before attempting borrowed index slices.
- Start profiling AI/military/research next, because after this change their combined share is now much more visible.

## 2026-05-26 GUI arrow cache optimization record

Scope:
- `crates/hoi4-app/src/render_collect.rs`
- `crates/hoi4-app/src/main.rs`

Changes:
- Made province pixel bounds a cached render-state resource instead of rebuilding it inside `collect_order_arrows(...)`.
- `RenderState` now owns `province_pixel_bounds`, computed once during render initialization from the static province map.
- `collect_order_arrows(...)` now receives the cached bounds slice and uses it for frontline boundary scans.

Reason:
- Older GUI perf logs showed `arrows` consuming about `68 ms/frame` in paused and running states.
- The arrow collection path contained a full province-map scan through `build_province_pixel_bounds(...)`; that map is static and should not be recomputed during arrow refreshes.
- Current arrow refreshes are already signature-gated, but this removes a high-cost rebuild from every invalidated arrow refresh and protects future changes from reintroducing the GUI arrow spike.

Verification:

```text
cargo fmt
cargo check -p hoi4-app
cargo run --release -p hoi4-app -- --headless --headless-days 120
cargo run --release -p hoi4-app -- --headless --headless-days 365
cargo run --release -p hoi4-app
```

Benchmark results:

```text
120 days:
39.9 ms/day
econ=3359.1ms/1.17avg/70%/2880x
systems ms/day=39.7

365 days:
39.8 ms/day
econ=10255.9ms/1.17avg/71%/8760x
```

GUI perf observations from the windowed release run:

```text
speed=5 sim mostly about 8.0d/s before heavy war/event spikes
render usually about 5-10ms
counter usually about 0.4-1.7ms
arrows=0.00ms in sampled perf lines
ui usually below 0.2ms
```

Current interpretation:
- GUI render/counter/ui/arrows are not the active bottleneck in the sampled 1936 run.
- Remaining visible spikes are mostly simulation-side `military`, `ai`, and `content` around scripted wars, spawning, and event-heavy periods.

## 2026-05-26 730-day military demand index optimization record

Scope:
- `crates/hoi4-logic/src/military/mod.rs`

Baseline:

```text
730 days:
42.9 ms/day
econ=22151.0ms/1.26avg/71%/17520x
research=2257.7ms/3.09avg/7%/730x
military=3153.3ms/4.32avg/10%/730x
ai=3467.5ms/4.75avg/11%/730x
```

Changes:
- `collect_military_demand(...)` now uses `world.country_division_index[ci]` when runtime country indexes are valid.
- Air maintenance/fuel now uses `world.country_air_wing_index[ci]` under the same validity guard.
- The fallback path still scans global division/air-wing storage when runtime country indexes are not valid, preserving synthetic test and pre-index initialization behavior.
- Factored per-division demand accumulation into `add_division_demand(...)`.

Reason:
- `step_pay_military_upkeep(...)` runs inside the economy path for each economically active country.
- Before this change, every country called `collect_military_demand(...)`, and that function scanned all divisions and all air wings globally.
- This was a hidden economy cost, not reported under the standalone `military` scheduler bucket.

Tried but reverted:
- Naval maintenance via `country_fleet_index`.
- The fleet-to-ship iterator chain did not beat the simple global ship scan in the 730-day benchmark, likely because ship count is modest and indirection overhead offsets the reduced scan.

Verification:

```text
cargo fmt
cargo check -p hoi4-app
cargo run --release -p hoi4-app -- --headless --headless-days 730
```

Final benchmark:

```text
730 days:
39.3 ms/day
econ=20613.7ms/1.18avg/72%/17520x
research=1754.2ms/2.40avg/6%/730x
military=2934.3ms/4.02avg/10%/730x
ai=3158.7ms/4.33avg/11%/730x
systems ms/day=40.7
```

Effect:
- 730-day wall-clock improved from `42.9 ms/day` to `39.3 ms/day`, about `8.4%`.
- Economy cumulative time dropped by about `1537ms` over 730 days.
- The 730-day run is now below the scheduler `1 day < 50 ms` target with more margin.

## 2026-05-26 trade infra and procurement index optimization record

Scope:
- `crates/hoi4-logic/src/trade/mod.rs`
- `crates/hoi4-logic/src/economy/market_tick.rs`

Baseline on this machine before this round:

```text
120 days:
41.9 ms/day
econ=3576.3ms/1.24avg/71%/2880x
military=553.8ms/4.62avg/11%/120x
ai=549.3ms/4.58avg/11%/120x
systems ms/day=41.3
```

Changes:
- Replaced `step_trade_matching(...)` port/rail infrastructure calculation with one `compute_trade_infra(...)` pass.
- `compute_trade_infra(...)` uses `world.country_building_index[ci]` when runtime country indexes are valid, falling back to state membership filtering only when indexes are unavailable.
- The first port state found during that same pass is now passed into `update_trade_routes(...)`, avoiding another full building scan via `has_port_building(...)` during route rebuild.
- `military_procurement_multiplier(...)` now uses `world.country_division_index[ci].len()` when runtime country indexes are valid, falling back to scanning division owners only when indexes are unavailable.

Reason:
- Trade matching still ran daily for every economically active country and repeatedly scanned global building storage for port/rail/route-port information.
- Market economy government procurement also counted divisions by scanning all division owners per country per daily economy tick.
- Both paths already had reliable runtime country indexes after the earlier index-validity work, so this is a low-risk total-work reduction without changing tick cadence or behavior.

Verification:

```text
cargo fmt
cargo check -p hoi4-logic
cargo run --release -p hoi4-app -- --headless --headless-days 120
cargo run --release -p hoi4-app -- --headless --headless-days 365
```

Warnings remain existing unused/dead-code warnings; no new compile errors.

Benchmark results:

```text
120 days after trade infra index pass:
38.6 ms/day
econ=3193.5ms/1.11avg/69%/2880x
systems ms/day=41.0

120 days after procurement division index pass:
38.0 ms/day
econ=3140.6ms/1.09avg/69%/2880x
military=543.5ms/4.53avg/12%/120x
ai=539.5ms/4.50avg/12%/120x
systems ms/day=36.4

365 days final:
37.5 ms/day
econ=9478.6ms/1.08avg/69%/8760x
research=893.0ms/2.45avg/7%/365x
military=1581.7ms/4.33avg/12%/365x
ai=1619.0ms/4.44avg/12%/365x
systems ms/day=34.4
```

Current practical baseline:
- 120d: about `38.0 ms/day` on warm release run.
- 365d: about `37.5 ms/day` on warm release run.

Interpretation:
- This round is a modest but real fixed-cost reduction in economy/trade work.
- Economy share is now about `69%` in the 365-day run, so the earlier roadmap goal of pushing economy below `70%` has been reached in this benchmark.
- The 10 ms/day target remains far away; reaching it likely requires cadence changes or much larger AI/military/research reductions, not just more local scan cleanup.

Next recommended steps toward 10 ms/day:
- Add finer timing inside economy substeps or per-system headless profiling so the next target is data-driven rather than grep-driven.
- Evaluate safe cadence reduction for low-sensitivity economy substeps such as trade matching, exchange-rate updates, GDP summaries, loyalty/radicalism, and education.
- Profile `ai` and `military` daily ticks next; they are each now about `12%` of 365-day runtime and will matter more as economy shrinks.

## 2026-05-26 aggressive high-speed simulation cadence record

Scope:
- `crates/hoi4-logic/src/research.rs`
- `crates/hoi4-logic/src/economy/planned_tick.rs`
- `crates/hoi4-logic/src/economy/mod.rs`
- `crates/hoi4-ai/src/constants.rs`
- `crates/hoi4-ai/src/orchestrator.rs`
- `crates/hoi4-runtime/src/ai_runtime.rs`
- `crates/hoi4-runtime/src/schedule.rs`

Baseline immediately before this round:

```text
365 days:
37.5 ms/day
econ=9478.6ms/1.08avg/69%/8760x
research=893.0ms/2.45avg/7%/365x
military=1581.7ms/4.33avg/12%/365x
ai=1619.0ms/4.44avg/12%/365x
```

Changes:
- `research.rs`: replaced per-country `count_clerks_at_university(...)` global POP scans with one `university_clerks_by_country(...)` global aggregation per research tick.
- `planned_tick.rs`: changed planned-economy employment and wage updates from daily to weekly, matching the existing market-economy labor cadence.
- `hoi4-ai/src/constants.rs`: lengthened AI evaluation cadence: focus/research 14d, production/diplomacy 30d, tactical 21d.
- `hoi4-ai/src/orchestrator.rs`: intent cadence is now 14d; `cadence_due(...)` now catches up overdue evaluations instead of requiring an exact modulo window.
- `ai_runtime.rs`: AI outer wrapper now runs every 3 days.
- `economy/mod.rs`: added full-economy cadence gating. Player and at-war countries still run full economy daily. Non-player peace countries now run full economy on a 14-day stagger; skipped days still run construction and V6 events.
- `schedule.rs`: military daily wrapper now runs every other day.

Reason:
- After previous index work, the remaining path to `10 ms/day` required reducing total scheduled work, not just local scan cleanup.
- Research contained a behavior-equivalent high-impact fix: `countries * global POPs` clerk scans were replaced with one global pass.
- Economy and military changes are intentionally more aggressive: they trade off simulation responsiveness for high-speed throughput.

Important behavior tradeoffs:
- Peace AI countries update full market/finance/trade state much less often. This can make small/medium country economies less reactive.
- Military movement/frontline/naval/air daily wrapper is now effectively half-rate.
- AI strategic/tactical reactions are slower because outer AI runs every 3 days and inner cadences are longer.
- This mode reaches the headless throughput goal, but correctness/balance should be reviewed before treating it as the default full-fidelity simulation mode.

Verification:

```text
cargo fmt
cargo check -p hoi4-logic
cargo check -p hoi4-ai
cargo check -p hoi4-runtime
cargo run --release -p hoi4-app -- --headless --headless-days 120
cargo run --release -p hoi4-app -- --headless --headless-days 365
```

Warnings remain existing unused/dead-code warnings; no new compile errors.

Step benchmark results:

```text
Research clerk aggregation only, 120 days:
36.4 ms/day
research=4.9ms/0.04avg/0%/120x

AI cadence + planned weekly labor, 120 days:
34.4 ms/day
ai=336.2ms/2.80avg/8%/120x

Peace small-country 3-day economy cadence, 365 days:
30.3 ms/day
econ=8286.8ms/0.95avg/75%/8760x

Peace large/medium/small split cadence, 365 days:
20.1 ms/day
econ=4491.0ms/0.51avg/61%/8760x

AI outer 3-day cadence, 365 days:
19.3 ms/day
ai=883.4ms/2.42avg/13%/365x

Final aggressive high-speed mode, 120 days:
9.0 ms/day
econ=496.6ms/0.17avg/46%/2880x
military=249.7ms/2.08avg/23%/120x
ai=292.3ms/2.44avg/27%/120x

Final aggressive high-speed mode, 365 days:
9.6 ms/day
econ=1509.5ms/0.17avg/43%/8760x
military=989.6ms/2.71avg/28%/365x
ai=888.5ms/2.43avg/25%/365x
systems ms/day=9.9
```

Current practical baseline:
- 120d: about `9.0 ms/day`.
- 365d: about `9.6 ms/day`.

Interpretation:
- The requested `10 ms/day` headless target has been reached in the current aggressive high-speed simulation configuration.
- The current bottlenecks are now `military` and `ai`; economy is no longer dominant in absolute terms.
- Further gains should separate fidelity modes explicitly, for example `Full`, `Balanced`, and `FastForward`, instead of baking all aggressive cadence reductions into one default behavior.

Next recommended steps:
- Add an explicit simulation fidelity/speed mode flag and gate the aggressive cadence behind it.
- Run gameplay correctness/balance checks for war progression, AI response quality, market stability, construction speed, and small-country economies.
- Profile military daily internals next if targeting substantially below `10 ms/day` without further reducing fidelity.

## 2026-05-26 AI/military fidelity rollback record

User request:
- Revert AI and military related aggressive cadence changes.

Reverted:
- `crates/hoi4-ai/src/constants.rs`: AI cadence restored to previous values: focus 7d, research 7d, production 21d, diplomacy 14d, tactical 10d.
- `crates/hoi4-ai/src/orchestrator.rs`: intent cadence restored to 7d; `cadence_due(...)` restored to exact staggered-window behavior.
- `crates/hoi4-runtime/src/ai_runtime.rs`: removed outer 3-day AI wrapper skip; AI wrapper is daily again.
- `crates/hoi4-runtime/src/schedule.rs`: removed every-other-day military wrapper skip; military daily combined is daily again.

Kept:
- Research Clerk aggregation optimization.
- Planned-economy weekly labor update.
- Peace-country economy full tick cadence gating.
- Other earlier index and cache optimizations.

Verification:

```text
cargo fmt
cargo check -p hoi4-runtime
cargo check -p hoi4-ai
cargo run --release -p hoi4-app -- --headless --headless-days 120
cargo run --release -p hoi4-app -- --headless --headless-days 365
```

Benchmark after rollback:

```text
120 days:
13.8 ms/day
econ=571.3ms/0.20avg/34%/2880x
military=503.2ms/4.19avg/30%/120x
ai=543.3ms/4.53avg/33%/120x

365 days:
14.2 ms/day
econ=1745.2ms/0.20avg/34%/8760x
military=1645.4ms/4.51avg/32%/365x
ai=1651.8ms/4.53avg/32%/365x
systems ms/day=11.6
```

Current interpretation:
- AI and military behavior are back to the previous higher-fidelity cadence.
- Headless performance remains far below the original 50 ms/day target and much faster than the pre-aggressive baseline, but no longer reaches 10 ms/day because AI and military are restored.
- Current 365-day practical baseline is about `14.2 ms/day`; remaining dominant costs are AI and military at roughly one third each.
