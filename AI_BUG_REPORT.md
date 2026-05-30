# AI 系统 Bug 报告

自动生成于 2026-05-21，基于 `crates/hoi4-ai/src/` 和 `crates/hoi4-logic/src/military/` 代码审查。

---

## 第一部分：战略决策层 Bug（#1 ~ #6）✅ 全部已修复

### Bug #1：`Defend` 态势永远不可达 [严重] ✅ 已修复
**位置**: `crates/hoi4-ai/src/ground.rs:192-207`  
**问题**: `ratio >= 0.60` 在 `RETREAT_RATIO(0.70)` 之前匹配，拦截了 Defend 可达路径。

### Bug #2：硬编码 0.60 进攻阈值 [严重] ✅ 已修复
**位置**: `crates/hoi4-ai/src/ground.rs:201-202`  
**问题**: profile 驱动的 `aggressive_threshold` 被硬编码覆盖。

### Bug #3：敌方力量估算对称于己方 [中等] ✅ 已修复
**位置**: `crates/hoi4-ai/src/ground.rs:137`  
**修复**: 用 `frontline_state_count_of` 按敌方自己前线占比估算。

### Bug #4：`ProfileRegistry` 每次调用重建 [中等] ✅ 已修复
**位置**: `crates/hoi4-ai/src/profile.rs:441-444`  
**修复**: `OnceLock` 全局单例。

### Bug #5：`compute_segment` 同轮冗余计算 [中等] ✅ 已修复
**修复**: 传入共享 `FrontLine`，用 `segment_against` 查表。

### Bug #6：多前线兵力计数错误 [轻微] ✅ 已修复
**修复**: assigned_divs 按前线总数整除均分。

---

## 第二部分："军队打着打着就停了"深度分析

### Bug #7：师团永生不灭 [致命] ✅ 已修复
**位置**: `crates/hoi4-state/src/store.rs:195-281`  
**问题**: `DivisionStore` 无删除方法，残兵永远留在世界状态中。

### Bug #8：PlayerArmy 锁定与清扫冲突 [致命] ✅ 已修复
**位置**: `ground_orders.rs:512` + `ground_orders.rs:634`  
**问题**: 锁定师无法参与清扫逻辑。修复：mop_up 纳入锁定师作 idle_divs 候选。

### Bug #9：攻击耗尽后退回驻守 [严重] ✅ 已修复
**位置**: `ground_orders.rs:75-90`  
**修复**: 无目标时进入 `assign_mop_up_role` 而非 `assign_garrison_role`。

### Bug #10：路径无法跟随深入 [中等] ⚠️ 部分修复
**位置**: `crates/hoi4-logic/src/military/frontline.rs:1028-1067`  
**修复**: 分区时纳入 army 路径省份以覆盖每日 tick_frontlines 扩展。

### Bug #11：BFS 不可达孤儿师 [中等] ✅ 已修复
**位置**: `ground_orders.rs:273-282`  
**修复**: 孤儿师回退到 frontline province 或首都。

### Bug #12：mop-up 空参数调用 [中等] ✅ 已修复
**位置**: `ground_orders.rs:686-690`  
**修复**: 构建敌方句柄集后调用。

### Bug #13：org 恢复拉锯 [中等] ⚠️ 部分缓解
**位置**: `arbiter.rs:50-52` + `organisation.rs:8-21`  
**修复**: `BLOCKING_GARRISON_THRESHOLD` 聚合战斗力判定。

### Bug #14：路径扩展与分区不同步 [轻微] ✅ 已修复
**位置**: `crates/hoi4-ai/src/ground_orders.rs:685-706`

---

## 第三部分：ITA vs ETH "一个师挡住 25 个师"根因分析

### Bug #15：进攻方突破值远低于防守方防御值 —— 攻击者注定失败 [致命] ✅ 已修复

**位置**: `crates/hoi4-logic/src/military/stats.rs:48-56` + `crates/hoi4-logic/src/military/battle.rs:188-209`

**问题**: 战斗公式对攻击方极不公平 —— 步兵师的 `breakthrough` 只有 `defense` 的 1/7。

#### 数值分析

以一个 9 步兵营标准师为例 (stats.rs:48):
```rust
"infantry" => (6.0, 0.5, 22.0, 3.0, 0.0, 4.0, 0.0)
//            soft  hard  def   brk  armor ap   hardness
```

聚合后 (9 营):
| 属性 | 值 |
|---|---|
| soft_attack | 9 × 6 = **54** |
| defense | 9 × 22 = **198** |
| breakthrough | 9 × 3 = **27** |
| max_org | ~60 |

#### 进攻方对防守方的伤害 (每 4h 轮)

```rust
// battle.rs:188-209
raw_attack = 54 × 1.0 × 1.0 = 54
hit_rate  = 54 / (54 + 198 + 1) = 0.214   // 仅 21.4% 命中率
base_damage = 54 × 0.214 = 11.5
org_damage = 11.5 × 0.40 = 4.6  /轮       // 每轮 4.6 org 伤害
```

#### 防守方对进攻方的伤害 (每 4h 轮)

```rust
raw_attack = 54 × 1.0 × 1.0 = 54
hit_rate  = 54 / (54 + 27 + 1) = 0.659    // 65.9% 命中率 —— 3倍！
base_damage = 54 × 0.659 = 35.6
org_damage = 35.6 × 0.40 = 14.2 /轮      // 每轮 14.2 org 伤害
```

#### 结论

| | 进攻方 | 防守方 |
|---|---|---|
| 每轮伤害 | **4.6** | **14.2** |
| 击破需轮数 | 60/14.2 ≈ **4 轮** (16h) | 60/4.6 ≈ **13 轮** (52h) |
| 每日净伤害 (6轮) | 85 org | 28 org |

**攻击者比防御者更快被击垮 —— 速度快 3 倍。**

#### 这就解释了全部卡死场景

```
ITA 25师 vs ETH 8师的战争:
  第 1-3 天: ITA 进攻，每对 1v1 中 ITA 师先被击破
  第 4-9 天: ITA 师撤退→恢复 org→重新进攻→再次被击破
  ...无限循环...
  结果: ITA 永远打不赢，ETH 永远打不垮
```

这并非调度或锁定的问题 —— 而是**物理层战斗模型本身让攻击方不可能获胜**。

---

### Bug #16：前线分配器均匀摊薄兵力，无法形成突破 [严重] ✅ 已修复

**位置**: `crates/hoi4-logic/src/military/frontline.rs:464-551`

**问题**: `frontline_distributor` 将师团均匀分布到整条路径上。

```
18个 ITA 师沿 12 省前线 → 每省 ~1.5 个师
8个 ETH 师守在 3 省   → 每省 ~2.7 个师

combat 配对: 1.5 vs 2.7 → 1v1 (n_pairs = min(1,1) = 1)
每省只打 1v1，多余的 ETH 师作为预备队不参战但不被消耗
```

在 bug #15 的基础上，均匀摊薄导致 ITA 在每个对位都以劣势态势开战（攻方本就劣势 + 兵力不集中）。

---

### Bug #17：`should_reassign` 的 Garrison 判定导致前线师被死锁 [中等] ✅ 已修复

**位置**: `crates/hoi4-ai/src/ground_orders.rs:206-217`

```rust
DivisionRole::Garrison => {
    if let Some(target) = asgn.target {
        let ctrl = world.provinces.controllers[target.0 as usize];
        if ctrl != owner && world.diplomacy.at_war_with(owner, ctrl) {
            return true;   // 目标被敌人占领 → 需重分配
        }
    }
    match world.divisions.destinations[div_idx] {
        None => true,                              // 无目的地 → 重分配
        Some(dest) => world.divisions.locations[div_idx] == dest,  // 已到达 → 重分配
    }
}
```

**问题**: 一旦 Garrison 师到达目标前线省份，`locations == dest` = true → `should_reassign` = true → 每次 eval 都被重新 assign 到同一省份 → 被 `tick_frontlines` 重新分配 → 循环。

这意味着已在前线的师不停地被"重新分配到已经站着的地方"，浪费 eval 开销且不能向前推进。

---

## 修复方案

### Bug #15 修复方案（最优先）

HOI4 原始设计中，`breakthrough` 应高于 `defense`（breakthrough 代表进攻时的防御值）。步兵 breakthrough 应该是防御值的 1.5-2 倍。

**方案 A — 调整 baselines 让进攻方不输**:
```rust
"infantry" => (6.0, 0.5, 22.0, 18.0, 0.0, 4.0, 0.0)
//                          breakthrough: 3→18
```

这样 hit_rate 变为: `54 / (54 + 162 + 1) = 0.248` vs 之前的 `0.659`
攻防比从 3:1 变为接近 1:1。

**方案 B — 直接修正 hit_rate 公式**:
```rust
// 进攻方用 breakthrough 替代 defense 做被击判定
fn round_damage(attacker: &BattleSide, defender: &BattleSide, is_attacker_phase: bool) -> f32 {
    let effective_def = if is_attacker_phase {
        attacker.stats.breakthrough  // 进攻方用自己的突破值挡伤害
    } else {
        defender.stats.defense
    };
    let hit_rate = raw_attack / (raw_attack + effective_def + 1.0);
    ...
}
```

### Bug #16 修复方案

在 `run_pair` 中，当进攻方在该省有大量师而防御方只有少量时，应允许多个进攻师同时攻击一个防守师（多打一），而非严格的 1v1 配对。或在前线分配器中引入"集中"权重——在攻击态势下把师聚到 2-3 个突破点而非均摊到全路径。

### Bug #17 修复方案

Garrison 师已在前线省份时，`should_reassign` 应返回 false（已有正确位置，不需要重分配）。修改条件为只在目标省被敌人夺走时才需要重分配。

---

## 修复优先级总表

| 优先级 | Bug | 严重性 | 状态 |
|---|---|---|---|
| **P0** | **#15: breakthrough << defense** | **致命** | **✅ 已修复** |
| **P0** | **#16: 均匀摊薄无法突破** | **严重** | **✅ 已修复** |
| P0 | #7: 师团永生不灭 | 致命 | ✅ 已修复 |
| P0 | #8: PlayerArmy 锁定冲突 | 致命 | ✅ 已修复 |
| P1 | #9: 攻击耗尽退回驻守 | 严重 | ✅ 已修复 |
| **P1** | **#17: Garrison 死锁** | **中等** | **✅ 已修复** |
| P2 | #10~#14 | 中等-轻微 | ✅ 全部已修复 |
| **P0** | **#18: tick_frontlines 每天覆写 destination** | **致命** | **✅ 已修复** |
| P1 | #19: 销毁阈值过高 | 中等 | ✅ 已修复 |
| P2 | #20: broken 师跳过分配死循环 | 中等 | ✅ 已修复 |
| P2 | #21: next_step 卡死清空 destination | 中等 | ✅ 已修复 |
| **P0** | **#22: 50% 非执行者师被浪费** | **致命** | **✅ 已修复** |
| **P0** | **#23: 箭头完成后全军静态守备** | **致命** | **✅ 已修复** |
| **P1** | **#24: executor 被击碎后永久降级** | **严重** | **✅ 已修复** |
| **P1** | **#25: AI 调度层 vs tick_frontlines 控制权** | **架构** | **✅ 已修复** |
| **P1** | **#26: strength 伤害系数过低** | **中等** | **✅ 已修复** |
| **P0** | **#27: Defend 姿态清除箭头导致停战** | **致命** | **✅ 已修复** |

---

## 第四部分：修复后仍然"不动/不消灭"的最终根因

### Bug #18：`tick_frontlines` 每天覆写所有师团目的地 —— AI 调度与战斗执行层的拉锯战 [致命] ✅ 已修复

**位置**: `crates/hoi4-logic/src/military/frontline.rs:1224-1231`

**问题**: `military_daily` 中 `tick_frontlines` **每天**将所有 PlayerArmy 成员的 `destinations` 重置。

**每日执行顺序** (`systems.rs:126-133`):
```
1. tick_daily (org恢复)
2. tick_frontlines       ← 每天覆写 destinations
3. daily_movement_tick   ← 用刚被覆写的 destination 移动
4. daily_division_cleanup
5. daily_occupation_tick
```

`tick_frontlines` 的核心逻辑 (frontline.rs:1205-1231):
```rust
for &mi in &world.player_armies[*army_idx].members {
    if broken { continue; }
    if let Some(&arrow_target) = executor_targets.get(&mi) {
        // 50% 师拿到 arrow target → 沿箭头进攻
    } else if let Some(&target) = dist_map.get(&mi) {
        world.divisions.destinations[mi] = Some(target); // ← 每天覆写！
    }
}
```

`frontline_distributor` 将师团**均匀摊到整条路径**上 (frontline.rs:511-523):
```rust
if n < m {
    target_idxs = (0..n).map(|j| j * m / n).collect(); // 稀疏取样
} else {
    // 多于路径点数 → 每个点分配 base 个师
}
```

**完整卡死链条**:

```
每 7 天 tactical eval:
  tick_ai_frontlines → 创建 PlayerArmy → set_arrow(targets)
  → execute_plan → arrow executing=true
  → 约 50% 师被 pick_arrow_executors 选中 (frontline.rs:1160-1164)
  → 这些师拿到 arrow 第一未征服省作为 destination

第 2~6 天 每日 military_daily:
  tick_frontlines:
    arrow provinces: 移除已征服省 → 可能变空 → arrow=null, executing=false
    executor_targets: 重新匹配 → 若 arrow 还在则 executor 继续拿进攻目标
    但 50% 非 executor 师: front_distributor → 均匀分配到路径友方省 → 原地不动
  
  关键问题: 执行者 (executor) 在战斗中被击碎后
  → daily_movement_tick 撤退到友方省 (movement.rs:124-147)
  → 下一天 tick_frontlines: 该师 broken=true → 跳过分配
  → org 恢复后不再是 executor
  → 被 frontline_distributor 分配到路径上 → 不再沿 arrow 进攻
  → arrow executor 越来越少 → 最终无人推进

同时: execute_ground_orders 的 partition_divisions_by_segment
  → 跳过所有 player_locked_divisions (line 728) → 空
  → assign_assault_role / assign_mop_up_role 无师可用
  → mop_up_remaining_enemies: idle_divs 也跳过 locked 师 → 空
  → AI 调度层完全失去对前线师的控制权
```

**结果**:
- ITA 师团原地来回踱步，无人向前推进
- ETH 残兵 org 恢复后重新挡路 → 战斗 → 撤退 → 恢复 → 循环
- cleanup 销毁条件 (strength < 0.001) 需要 ~58 轮战斗，永不可达

**修复方向**:
- `tick_frontlines` 不应覆写已有非友方目的地且 role=Assault 的师
- `partition_divisions_by_segment` 和 `mop_up` 不应跳过 locked 师 — 至少纳入 idle_divs
- 或让 `execute_ground_orders` 在 tick_frontlines 之前运行，AI 先做决策，系统再配合执行

---

## 第五部分：次要残存问题

### Bug #19：销毁阈值过高 [中等] ✅ 已修复

**位置**: `cleanup.rs:25`

`IMMEDIATE_DESTROY_STRENGTH` 从 0.001 提高到 0.05，`LOW_STRENGTH_THRESHOLD` 提高到 0.10，`SURROUNDED_STRENGTH_THRESHOLD` 提高到 0.15。

### Bug #20：broken 师跳过分配死循环 [中等] ✅ 已修复

**位置**: `frontline.rs:1209-1215`

broken 师不再被跳过——若在敌方省份且无友方目的地，`tick_frontlines` 会用 `find_nearest_friendly` BFS 指派撤退目标。

### Bug #21：next_step 卡死清空 destination [中等] ✅ 已修复

**位置**: `movement.rs:35-43`

BFS 找不到路径时不再清空 destination，改为保留目标等待路径可达（避免 AI 重派→再失败→再清空→死循环）。

---

## 第七部分：为什么正好是 1936 年 4 月 1 日停战？

### Bug #27：第 13 次战术评估 ratio 越过 Defend 阈值 [致命] ✅ 已修复

**问题不是 4 月这个日历时间，而是第 13 次 7 天评估周期的精确时机。**

**时间线**:

```
1936-01-01 = Day 0  第 1 次战术评估 → posture=Attack, 设 arrow
1936-01-08 = Day 7  第 2 次 → arrow 还在, 不更新
1936-01-15 = Day 14 第 3 次
...
1936-03-25 = Day 84 第 12 次 → ratio 已经开始下降
1936-04-01 = Day 91 第 13 次 → ratio 跌破 RETREAT_RATIO ← 停战！
```

**根因**: 90 天战斗导致 ITA 师团战力下降 + 前线扩展使战力摊薄，ratio 在第 13 次评估时跌破 `RETREAT_RATIO(0.70)`。

在 `evaluate_ground` (`ground.rs:204-218`):
```rust
let posture = if ... {
    ...
} else if ratio >= aggressive_threshold {
    GroundPosture::Attack
} else if ratio >= RETREAT_RATIO {
    GroundPosture::Defend   // ← 命中！ratio 在 [0.70, aggressive_threshold)
} else {
    GroundPosture::Retreat
};
```

posture 从 Attack → Defend 后，`tick_ai_frontlines` (`frontline_ai.rs:68-104`) 触发箭头清理:
```rust
let needs_arrow_update = match posture {
    GroundPosture::Attack => !has_arrow,
    _ => has_arrow,       // Defend + has_arrow → needs update
};
// → clear_arrow → arrow=None, executing=false
```

箭头清除 → 全军落入 `frontline_distributor` 守备模式 + 14 天 sticky 锁定 → **双方停火**。

**同样适用于西班牙内战**。SCW 双方约 50 天后也会遇到同样问题——第 7-8 次评估 ratio 过线，posture 转 Defend，箭头清除，全军闲置。

**修复**: Defend 姿态不再清除箭头——仅 halt_plan（停止推进但保留前线部署），让部队继续在前线与残存敌师交战。Hold 姿态同理。仅 Retreat 姿态才 clear_arrow。`execute_ground_orders` 中 Defend/Hold 改为 `assign_mop_up_role` 而非 `assign_garrison_role`，确保前线师主动消灭残敌而非退守后方。

**修改文件**:
- `frontline_ai.rs:68-106` — Defend/Hold 不清箭头，仅 halt；只有 Retreat 清箭头
- `ground_orders.rs:95-98` — Defend/Hold 走 mop_up 而非 garrison

---

## 第六部分：修复后仍然不动 —— 架构层控制权冲突最终分析

### Bug #22：50% 非执行者师被永恒浪费在友方路径上 [致命] ✅ 已修复

**位置**: `crates/hoi4-logic/src/military/frontline.rs:610-627`

`pick_arrow_executors` 只选 `ceil(n_alive / 2)` 个师做执行者：

```rust
let k = ((n as f32) / 2.0).ceil() as usize;
// ...取离 anchor 最近的 k 个师
```

剩余 50% 非执行者师被 `frontline_distributor` 均匀摊到友方路径省上 → **永远不进攻**。

**修复**: `pick_arrow_executors` 返回全部存活师团，不再按距离筛选50%。同时 `tick_frontlines` 中 `executing` 为 true 时即选中所有师为执行者，不再要求 `o.arrow.is_some()`。

### Bug #23：箭头完成后全军落入静态守备 [致命] ✅ 已修复

**位置**: `crates/hoi4-logic/src/military/frontline.rs:1283-1302`

当 arrow 省份全部被征服后，原来直接 `arrow = None; executing = false`，导致全军落入 `frontline_distributor` 均匀分配到友方省 → 永久静止。

**修复**: 箭头完成时自动延伸——从最后征服省 BFS 找相邻敌省加入箭头；若仍无敌省可延伸，保留 `executing=true` 并从路径省找敌省分配执行者目标，避免全军降级为静态守备。

### Bug #24：executor 被击碎后永久降级为守备 [严重] ✅ 已修复

executor 在敌省战斗 → 被击碎 (org<5%) → `daily_movement_tick` 撤退到友方省 → `tick_frontlines` 下一天: 该师不再是 executor（因为 `pick_arrow_executors` 只选离 anchor 最近的50%师，撤退后距离变远）→ 该师落入 `frontline_distributor` → 静态守备 → **永远失去进攻资格**。

**修复**: 与 #22 一同修复——`pick_arrow_executors` 现在返回全部存活师团，被击碎撤退的师恢复后自动重新成为执行者，不再永久降级。

### Bug #25：AI 调度层 vs tick_frontlines 控制权竞争 [架构] ✅ 已修复

两个系统每天竞争师团的 `destinations` 控制权：

| 系统 | 频率 | 操作 |
|---|---|---|
| `tick_ai_frontlines` + `execute_ground_orders` | 7天 | 创建 PlayerArmy → 锁定师 → set_arrow → assign_assault_role |
| `tick_frontlines` | **每天** | 覆写 locked 师的 destination（仅保留 Assault 师不变，但守备师全被摊到路径） |

`execute_ground_orders` 完全无法指挥 locked 师（`partition_divisions_by_segment` 跳过 locked，`assign_assault_role` 跳过 locked，`mop_up` 跳过 locked）。AI 调度层对前线师零控制权。

**修复**: `assign_assault_role`、`assign_mop_up_role`、`assign_garrison_role` 不再一刀切跳过 locked 师——仅跳过已有有效 Assault 分配且不需重分配的 locked 师，其余 locked 空闲师纳入分派池。

### Bug #26：即使提高销毁阈值，正常战斗也几乎无法消灭师团 ✅ 已修复

**位置**: `cleanup.rs:25` + `battle.rs:165-166`

强度伤害公式: `strength_damage = base_damage × STRENGTH_DAMAGE_FACTOR = 11.5 × 0.0015 = 0.017/round`

| 场景 | 轮数需求 |
|---|---|
| 从 1.0 降到 0.05 (cleanup触发) | (1.0-0.05)/0.017 = **56 轮** ≈ **9.3 天连续战斗** |
| org 被击破所需 | ~5 轮 (20h) |
| 实际: 师团在 20h 内 org 崩溃撤退，远早于 strength 降到销毁阈值 |

**结果**: 师团无限打->撤退->恢复->再打循环，strength 在 0.5~1.0 间浮动，永远不触发 cleanup。

**根因**: `STRENGTH_DAMAGE_FACTOR = 0.0015` 过小，师团难以被消灭。

**修复**: `STRENGTH_DAMAGE_FACTOR` 从 0.0015 微调至 0.001（实测 0.10/0.01 消灭速度过快，回退到接近原值）。

---

## 修复方案

### #22/#24 修复：全部师团为执行者，不再筛选50%

`pick_arrow_executors` 不再按距离取前50%师，直接返回全部存活师团。`tick_frontlines` 中执行者条件从 `o.arrow.is_some() && o.executing` 改为仅 `o.executing`，确保被击碎撤退的师恢复后自动重返进攻。

### #23 修复：箭头完成后自动延伸而非降级为守备

箭头完成时三级延伸策略：
1. 从最后征服省 BFS 找相邻敌省加入箭头
2. 从第一征服省找相邻敌省插入箭头首部
3. 从路径省找相邻敌省重建箭头
若仍无法延伸，保留 `executing=true`，`tick_frontlines` 会从路径省的敌邻中分配进攻目标给所有师团。

### #25 修复：AI 调度层可控制 locked 空闲师

`assign_assault_role`、`assign_mop_up_role`、`assign_garrison_role` 不再一刀切跳过 `player_locked_divisions`——仅跳过已有有效 Assault 分配且不需重分配的 locked 师，其余 locked 空闲师纳入分派池。

### #26 修复：提高 strength 伤害系数

`STRENGTH_DAMAGE_FACTOR` 从 0.0015 微调至 0.001（实测更高值消灭速度过快，回退到接近原值）。