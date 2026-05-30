# 实现计划：玩家划线战线（frontline-orders）

## 概览

按以下顺序逐任务实施：

1. **State 类型**（hoi4-state）→ 2. **存档 round-trip**（hoi4-state/save）→ 3. **纯算法**（hoi4-logic/military/frontline 子模块）→ 4. **公共 API**（同模块）→ 5. **每日 tick 集成**（systems.rs）→ 6. **AI override 互锁**（hoi4-ai/ground_orders）→ 7. **AI 决策器**（hoi4-ai/frontline_ai）→ 8. **UI/render**（hoi4-app）→ 9. **检查点 + 性能**。

每个任务都引用 design.md 章节、要修改的文件、要补的属性测试编号（1–17，与 design.md `## 正确性属性` 一一对应）和需求 ID。带 `*` 的是可选测试任务（属性测试 / 单元测试 / 集成测试）；不带 `*` 的是必须实施的核心任务。

## 任务

- [x] 1. State 类型
  - [x] 1.1 新建 `hoi4_state::frontline` 模块与三个数据类型
    - 创建 `crates/hoi4-state/src/frontline.rs`：定义 `ArmyId(u32)`（含 `NONE = u32::MAX`、`raw()` / `is_none()`）、`OffensiveArrow { provinces: Vec<ProvinceId> }`、`FrontlineOrder { path, arrow, anchor, active }`、`PlayerArmy { id, name, owner, members, order }`，全部 `#[derive(Clone, Debug, PartialEq)]`。
    - 在 `crates/hoi4-state/src/lib.rs` 加 `pub mod frontline;` 与 re-export `ArmyId`、`PlayerArmy`、`FrontlineOrder`、`OffensiveArrow`。
    - 文件：`crates/hoi4-state/src/frontline.rs`、`crates/hoi4-state/src/lib.rs`
    - _Requirements: 1.1, 5（数据形态）, 术语表_

  - [x] 1.2 在 `World` 上新增三个字段并初始化
    - 在 `crates/hoi4-state/src/world.rs` 的 `pub struct World` 内追加 `player_armies: Vec<PlayerArmy>`、`player_locked_divisions: HashSet<usize>`、`next_army_id: u32`。
    - `World::new` 把它们初始化为空 / 0。`populate_from_history` 不动。
    - 把 `dissolve_army` 留作后续任务实现入口；本任务仅加字段。
    - 文件：`crates/hoi4-state/src/world.rs`
    - _Requirements: 1.1, 8.4, 10.1, 15.1_

  - [ ]* 1.3 单元测试：默认 World 上字段为空 + ArmyId NONE 行为
    - 文件：`crates/hoi4-state/tests/frontline.rs`（新建）
    - _Requirements: 1.1_

- [x] 2. 存档 round-trip
  - [x] 2.1 文本格式扩展：在 country 块内写 `armies={…}`
    - 在 `crates/hoi4-state/src/save/text.rs` 内 `write_country` 之后（或并列）实现 `write_armies(s, world, cid)`：按 ArmyId 升序写每个 owner==cid 的 PlayerArmy；字段顺序固定 `name`、`members`、`path`、`arrow`、`anchor`、`active`，与现有缩进/引号风格一致。
    - 在 `read_country` / 顶层 `read_str` 中解析 `armies={…}` 块；越界成员丢弃 + emit `Parse` warning（_R10.4_）；非法 path（不相邻 / 重复 / id 越界）→ 丢 order 保 PlayerArmy 外壳（_R12.3_）。
    - 解析完成后 `world.next_army_id = max(existing ArmyId.0) + 1`，并按 ArmyId 升序排序。
    - 文件：`crates/hoi4-state/src/save/text.rs`
    - _Requirements: 10.1, 10.2, 10.3, 10.4, 12.1, 12.2, 12.3, 15.10_

  - [x] 2.2 二进制格式扩展：版本化 chunk `b"FRARM\x01"`
    - 在 `crates/hoi4-state/src/save/binary.rs` 内为每个 country 追加可选 chunk：`magic + version + len + entries`，每条 entry 用 LEB128 编码 ArmyId、members u32 列表、path/arrow u16 列表、anchor `Option<u16>`、active u8。
    - 不含此 chunk 的旧档被读到时跳过，`world.player_armies` 留空。
    - 文件：`crates/hoi4-state/src/save/binary.rs`
    - _Requirements: 10.1, 10.2, 10.3, 12.1_

  - [x]* 2.3 Property 5：存档 round-trip 属性测试
    - 在 `crates/hoi4-state/tests/save_roundtrip.rs` 中新增 `arbitrary_player_armies()` strategy；对文本与二进制各一条 `proptest!`。
    - 包含规范化字节相等子断言（`serialize(deserialize(serialize(a))) == serialize(a)`）。
    - **Property 5: 存档 round-trip**
    - **Validates: Requirements 10.1, 10.2, 12.1, 12.2, 15.10**

  - [x]* 2.4 单元测试：旧档兼容 + 越界成员 + 非法路径
    - 在 `tests/save_roundtrip.rs` 中加 3 个 `#[test]`：(a) 不含 `armies={…}` 的存档读后 `player_armies.is_empty()`（_R10.3_）；(b) 含越界成员的 armies 读后该成员被丢且仍含其它（_R10.4_）；(c) 含不相邻 path 的 armies 读后 `order==None && PlayerArmy 仍在`（_R12.3_）。
    - _Requirements: 10.3, 10.4, 12.3_

- [x] 3. 检查点：state + 存档先单独通过
  - 跑 `cargo test -p hoi4-state` 全绿（保证不破坏既有 round-trip 测试）。如有问题询问用户后修复。
  - _Requirements: 10, 12_

- [x] 4. 纯算法：吸附器与分布器
  - [x] 4.1 创建 `hoi4_logic::military::frontline` 子模块骨架
    - 新建 `crates/hoi4-logic/src/military/frontline.rs`，在 `crates/hoi4-logic/src/military/mod.rs` 加 `pub mod frontline;`。
    - 定义常量 `MAX_PATH_LEN = 64`、`MAX_ARROW_LEN = 32`、`MAX_FRONTLINES_PER_COUNTRY = 4`、`MAX_BRIDGE_DEPTH = 6`、`MAX_SAMPLES = 1024`、`BFS_MAX_DEPTH = 40`。
    - 定义 `pub enum FrontlineError`（11 种变体，见 design 第 2 节）。
    - 文件：`crates/hoi4-logic/src/military/frontline.rs`、`crates/hoi4-logic/src/military/mod.rs`
    - _Requirements: 2.5, 5.3, 5.4, 13.2, 14, 15.2_

  - [x] 4.2 `validate_path` 与 `co_belligerent` 工具
    - 实现 `pub fn co_belligerent_set(world, owner) -> HashSet<CountryId>`（owner ∪ 同阵营战时国 ∪ has_military_access）。
    - 实现 `pub fn validate_path(world, owner, path) -> Vec<ProvinceId>`：保留满足 land + 控制者 ∈ co_belligerent + 与某敌占陆地省相邻 OR 在两合法前线省 BFS 桥上 的省；保持顺序。
    - 实现 `pub(crate) fn bfs_land_dist(world, from, to, max_depth) -> Option<u32>` 与 `pub(crate) fn bfs_land_path(world, from, to, max_depth) -> Option<Vec<ProvinceId>>`，**不写 `world.path_cache`**（保持 `&World`）。
    - 文件：`crates/hoi4-logic/src/military/frontline.rs`
    - _Requirements: 7.1, 7.2, 7.5, 11.1, 11.4, 术语表_

  - [x] 4.3 `frontline_snapper`
    - 实现 `pub fn frontline_snapper(world, owner, samples) -> Result<Vec<ProvinceId>, FrontlineError>`：吸附 → 去连续重 → 缝合（缺口处 BFS ≤ 6 深度）→ 截断到 64 → `validate_path` 过滤。
    - 截断到 64 时返回 `Err(FrontlineError::PathTruncated(orig_len))`，但仍把截断结果通过 out-param 或 enum data 暴露给调用方（设计上先把截断值放在 `Err` payload 里，由 API 层根据策略保留前 64）。
    - 全空 → `Err(NoEligibleProvince)`。
    - 文件：`crates/hoi4-logic/src/military/frontline.rs`
    - _Requirements: 2.2, 2.3, 2.4, 2.5, 2.6, 14.2_

  - [x]* 4.4 Property 4 + Property 9：snapper 输出合法 + 幂等
    - 在 `crates/hoi4-logic/tests/frontline_path_invariant.rs`（新建）中：
      - `arb_grid_world(rows, cols)` 生成 4 邻接 toy World；
      - Property 4：任意 samples → snap → 检查 land + 相邻 + 控制者 + 无重复 + 长度 ∈ [1, 64]；
      - Property 9：任意已合法 path → `frontline_snapper(world, owner, &path) == Ok(path.clone())`。
    - **Property 4: 战线路径合法性**
    - **Property 9: 吸附器幂等**
    - **Validates: Requirements 2.3, 2.4, 2.5, 7.1, 7.2, 7.5**

  - [x] 4.5 `arrow_snapper`
    - 实现 `pub fn arrow_snapper(world, owner, anchor, samples) -> Result<Vec<ProvinceId>, FrontlineError>`：仅敌占 land 省合法；第 1 省必须与 anchor 相邻（否则 `AnchorNotAdjacent`）；缝合 BFS ≤ 6（否则 `BridgeUnreachable`）；截到 32（`ArrowTruncated`）。
    - 文件：`crates/hoi4-logic/src/military/frontline.rs`
    - _Requirements: 5.1, 5.2, 5.3, 5.4, 14.3, 14.4_

  - [x]* 4.6 单元测试：arrow_snapper 错误码覆盖
    - `crates/hoi4-logic/tests/frontline_path_invariant.rs` 加 3 个 `#[test]`：锚点不相邻、桥 > 6 跳、超过 32 省。
    - _Requirements: 5.3, 5.4, 14.3, 14.4_

  - [x] 4.7 `frontline_distributor`
    - 实现 `pub fn frontline_distributor(world, army) -> Vec<(usize, ProvinceId)>`：先过滤 alive 成员（非破碎、非 in_combat）、再按 floor/ceil 或 N<M 均匀挑选构造 `target_idxs`、再按 (BFS 距离, div_idx) 升序贪心填 capacity。
    - **必须** `world: &World`（不可变借用），不调用任何修改路径。
    - 文件：`crates/hoi4-logic/src/military/frontline.rs`
    - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5, 11.1, 11.2, 11.3, 11.4_

  - [x]* 4.8 Property 1 + Property 2 + Property 3 + Property 10：分布器
    - 文件：`crates/hoi4-logic/tests/frontline_distribution.rs`（新建）。
    - Property 1（均衡不变量）+ Property 2（确定性，连两次调用 assert_eq）+ Property 3（置换 members 后每省计数不变）+ Property 10（owner 字段不影响输出 multiset）。
    - **Property 1: 分布方案均衡不变量**
    - **Property 2: 分布器确定性**
    - **Property 3: 输入顺序置换汇合**
    - **Property 10: 玩家与 AI 分布等价**
    - **Validates: Requirements 3.2, 3.3, 3.4, 11.1, 11.2, 11.3, 15.1, 15.5**

  - [x] 4.9 `pick_arrow_executors`
    - 实现 `pub fn pick_arrow_executors(world, army) -> Vec<usize>`：按 (BFS_dist(loc, anchor), div_idx) 升序取前 `ceil(N_alive/2)`；若 anchor 不在 path 中则先选 path 中 BFS 最近的省作 fallback anchor。
    - 文件：`crates/hoi4-logic/src/military/frontline.rs`
    - _Requirements: 6.1, 6.2, 6.6_

- [x] 5. 公共 API（同 `frontline` 模块）
  - [x] 5.1 `create_army` / `dissolve_army`
    - `create_army(world, owner, members, name) -> Result<ArmyId, FrontlineError>`：
      - 空 members → `NoSelectedDivisions`；
      - 任 i `owners[i] != owner` → `DivisionNotOwned`；
      - 已活跃军数 ≥ 4 → `ArmyCapReached`；
      - 从其它 PlayerArmy 中先把这些 members 移除（保互斥）；
      - 用 `world.next_army_id` 分配 id 并 +=1；push 到 `player_armies`；更新 `player_locked_divisions`。
    - `dissolve_army(world, id)`：从 `player_armies` 删该项；`destinations` 不变；从 `player_locked_divisions` 删它的成员。
    - 文件：`crates/hoi4-logic/src/military/frontline.rs`
    - _Requirements: 1.1, 1.2, 1.3, 1.4, 14.1, 15.2_

  - [x] 5.2 `add_members` / `remove_members`
    - `add_members(world, id, members)`：所有 i 必须 `owners[i] == army.owner`；先从其它 PlayerArmy 移除（保互斥），再 push 到目标。
    - `remove_members(world, id, members)`：从 army.members 中删；不删空 PlayerArmy（保 R1.6 规则）。
    - 两者都更新 `player_locked_divisions`。
    - 文件：`crates/hoi4-logic/src/military/frontline.rs`
    - _Requirements: 1.4, 1.5, 1.6, 8.4_

  - [x] 5.3 `set_frontline_path` / `clear_frontline_path`
    - `set_frontline_path(world, id, samples)`：调 `frontline_snapper`；成功 → 替换 order 并清旧 arrow，置 `active=true`、`anchor=None`；snapper 返回 `NoEligibleProvince` → 不改原 order，传播错误；`PathTruncated` → 接收截断路径并设上去（仍算成功），但通过 `tracing::warn!` 报截断。
    - `clear_frontline_path(world, id)`：把 `order = None`；从 `player_locked_divisions` 删该军成员。
    - 文件：`crates/hoi4-logic/src/military/frontline.rs`
    - _Requirements: 2.2, 2.5, 2.6, 2.7, 14.2, 14.5_

  - [x] 5.4 `set_arrow` / `clear_arrow`
    - `set_arrow(world, id, anchor, samples)`：要求 army 已有活跃 path；调 `arrow_snapper`；成功 → 替换 arrow；选择 anchor 为 path 中与 arrow 第 1 省相邻、按距离最近的省。
    - `clear_arrow(world, id)`：`order.arrow = None`，path 不变。
    - 文件：`crates/hoi4-logic/src/military/frontline.rs`
    - _Requirements: 5.1, 5.2, 5.5, 5.6, 14.3, 14.4_

  - [x]* 5.5 Property 6 + Property 11 + Property 16：API 序列不变量
    - `crates/hoi4-logic/tests/frontline_api.rs`（新建）。
    - 任意 API 调用序列 generator → 序列结束扫：
      - Property 6（成员互斥）；
      - Property 11（每国 active count ≤ 4，超额返回 `ArmyCapReached`）；
      - Property 16（dissolve_army 后 destinations 不变 + lockset 不含原成员）。
    - **Property 6: 成员互斥**
    - **Property 11: 每国战线上限**
    - **Property 16: 解散保留 destinations**
    - **Validates: Requirements 1.3, 1.4, 1.5, 1.6, 15.2**

  - [x]* 5.6 单元测试：每条错误码至少 1 个 `#[test]`
    - 文件：`crates/hoi4-logic/tests/frontline_api.rs`
    - 11 个错误变体每个一条 happy/sad path 测试。
    - _Requirements: 1.2, 5.1, 5.2, 14.1–14.5, 15.2_

- [x] 6. 检查点：算法 + API 全绿
  - 跑 `cargo test -p hoi4-logic` 全绿，确保不破坏既有 military_battle / movement 测试。
  - _Requirements: 1, 2, 3, 5, 11, 14, 15.2_

- [x] 7. 每日 tick 集成
  - [x] 7.1 实现 `tick_frontlines(world)` 主循环
    - 在 `crates/hoi4-logic/src/military/frontline.rs` 内：
      - 步骤 1.a `validate_path` 删非法省（_R7.1, R7.2_）；
      - 步骤 1.b path 空 → `order.active=false`、清成员 destinations、tracing warn "前线已崩溃"（_R4.4, R7.3_）；
      - 步骤 1.c 调 `frontline_distributor` 得分布；
      - 步骤 1.d 若有 arrow：调 `pick_arrow_executors`；executors 的目的地覆盖为 arrow 上首个非 owner 控制省；非 executors 走分布；`can_enter_province==false` 的执行者保留分布器原结果（_R6.4_）；
      - 步骤 1.e 写 `destinations[i]`：locations[i]==target 则 None，否则 Some(target)；in_combat / broken 跳过（_R3.6, R3.8, R4.5_）；
      - 步骤 1.f arrow 全占领 → 清 arrow + tracing warn "箭头已完成"，path 仍 active（_R6.5_）；
      - 步骤 1.f' anchor 不在 path → 滚动选最近 path 省；path 空 → 清 arrow（_R6.6_）；
      - 步骤 1.g 删 `members` 中越界 / 销毁的 div idx（_R1.7, R15.11_）。
    - 步骤 2 重建 `world.player_locked_divisions`：所有 owner==某国 + order.active 的成员并集。
    - 步骤 0：`is_at_war(player_country)==false` 时玩家国家所有 PlayerArmy 置 inactive 但保留 path（_R7.4, R14.5_）。
    - 文件：`crates/hoi4-logic/src/military/frontline.rs`
    - _Requirements: 1.7, 3.6, 3.7, 3.8, 4.4, 4.5, 6.4, 6.5, 6.6, 7.1, 7.2, 7.3, 7.4, 8.4, 14.5, 15.11_

  - [x] 7.2 在 `military_daily` 中挂入 `tick_frontlines` 调用
    - 在 `crates/hoi4-app/src/systems.rs::military_daily` 内，于 `organisation::tick_daily` 之后、`daily_movement_tick` 之前加 `hoi4_logic::military::frontline::tick_frontlines(ctx.world);`。
    - 顺序必须严格按 design 第 "每日 tick 集成顺序" 节。
    - 文件：`crates/hoi4-app/src/systems.rs`
    - _Requirements: 3.7_

  - [ ]* 7.3 Property 8 + Property 12 + Property 13 + Property 14 + Property 15：tick 行为
    - `crates/hoi4-logic/tests/frontline_tick.rs`（新建）。
    - Property 8（塌陷 → active=false + destinations None）+ Property 12（destinations 写入语义、in_combat / broken 跳过）+ Property 13（执行者 ceil/floor 拆分 + can_enter）+ Property 14（arrow 完成 → 清 arrow 保 path）+ Property 15（anchor 滚动）。
    - **Property 8: 战线塌陷终止**
    - **Property 12: destinations 写入语义**
    - **Property 13: 箭头执行者选择 + 防御保留**
    - **Property 14: 箭头完成 → 清 arrow 保 path**
    - **Property 15: anchor 滚动**
    - **Validates: Requirements 3.6, 3.8, 4.4, 4.5, 6.1, 6.2, 6.3, 6.4, 6.5, 6.6, 7.3, 7.4, 14.5_**

- [x] 8. AI override 互锁
  - [x] 8.1 玩家保护短路 + AI 兜底逐师过滤
    - 在 `crates/hoi4-ai/src/ground_orders.rs::execute_ground_orders` 入口加：
      - 若 `country == world.player` 且玩家有任意 active PlayerArmy → 直接 `return`（_R8.1_）；
      - 否则，在 `assign_assault_role` / `assign_garrison_role` / `mop_up_remaining_enemies` / `clear_stale_assignments` 等 4 处写 `destinations[i]` 之前加 `if world.player_locked_divisions.contains(&i) { continue; }`（_R15.4_）。
    - 文件：`crates/hoi4-ai/src/ground_orders.rs`
    - _Requirements: 8.1, 8.2, 8.4, 8.5, 15.4_

  - [ ]* 8.2 Property 7：玩家/AI 集团军 destinations 保护
    - `crates/hoi4-ai/tests/frontline_override.rs`（新建）。
    - 任意带 active PlayerArmy 的 World + 任意 (eval, intent) → 调 `execute_ground_orders` → 验证活跃集团军成员 destinations / assignments 与调用前完全相同；输入空间双覆盖：`country==world.player` 与 `country` 为 AI 国家自身。
    - **Property 7: 玩家/AI 集团军 destinations 保护**
    - **Validates: Requirements 8.1, 8.2, 8.4, 8.5, 15.4**

- [x] 9. AI 决策器
  - [x] 9.1 新增 `hoi4_ai::frontline_ai` 模块
    - 新建 `crates/hoi4-ai/src/frontline_ai.rs`，在 `lib.rs` 加 `pub mod frontline_ai;`。
    - 实现 `pub fn tick_ai_frontlines(world, country, eval, intent)`：
      - 对每个 active segment：找现有 owner==country PlayerArmy 与之匹配；不存在 + active 数 < 4 → `create_army`；调 `set_frontline_path(friendly_states_to_provinces(seg))`；
      - 按 `eval.decisions[seg].posture`：Attack → `set_arrow(... pick_attack_targets)`；其它 → `clear_arrow`；
      - 解散没对应 segment 且 path 已塌陷的 AI 集团军；空成员 PlayerArmy 直接 `dissolve_army`（_R15.11_）。
    - 文件：`crates/hoi4-ai/src/frontline_ai.rs`、`crates/hoi4-ai/src/lib.rs`
    - _Requirements: 15.1, 15.3, 15.6, 15.7, 15.11_

  - [x] 9.2 在 orchestrator 中替换 ground 决策入口
    - 在 `crates/hoi4-ai/src/orchestrator.rs::tick` 中：原先调用 `execute_ground_orders` 之前加 `frontline_ai::tick_ai_frontlines(world, country, &eval, &current_intent);`；mop-up 路径不变（`!front.has_front()` 时仍走 `execute_mop_up_only`，_R15.7_）。
    - 文件：`crates/hoi4-ai/src/orchestrator.rs`
    - _Requirements: 15.3, 15.7_

  - [ ]* 9.3 集成测试：AI 国家在 toy world 上的全循环
    - `crates/hoi4-ai/tests/frontline_ai.rs`（新建）：构造一个有玩家 + 2 个 AI 国家的 toy world，跑 5 个 daily tick；验证：(a) AI 国家创建了对应 segment 的集团军，(b) attacker AI 在 Attack posture 下设了 arrow，(c) 全敌占场景下 active 数为 0 但 PlayerArmy 外壳保留，(d) `!has_front()` 走 mop-up（不创建集团军）。
    - _Requirements: 15.3, 15.6, 15.7_

- [ ] 10. 检查点：state + algo + tick + AI 全绿
  - 跑 `cargo test -p hoi4-state -p hoi4-logic -p hoi4-ai` 全绿。
  - _Requirements: 1–8, 11, 12, 14, 15_

- [x] 11. UI/render
  - [x] 11.1 `FrontlinePainterState` + 输入采样
    - 在 `crates/hoi4-app/src/main.rs` 的 `App` 内加 `frontline_painter: FrontlinePainterState`；定义 `enum PainterMode { Idle, ArmyPainter, ArrowPainter }`。
    - 鼠标按下 → `samples.clear()`、`last_sample_at = Instant::now()`；移动 → 距上次 ≥ 33 ms 且 picked ProvinceId 与 `samples.last()` 不同时追加；松开 → 调 `set_frontline_path` / `set_arrow`；样本上限 1024（超过按时间窗下采）；Esc/右键取消画线模式。
    - 文件：`crates/hoi4-app/src/main.rs`
    - _Requirements: 2.1, 2.2, 5.1, 13.2_

  - [x] 11.2 集团军面板（HOI4 风格底栏 + 侧面板）
    - 底栏 `army_bottom_bar`（`TopBottomPanel::bottom`，60px，Playing 阶段始终可见）：
      - 未选集团军：显示集团军列表按钮 + `+ Create Army (N selected divs)` + overlay toggle；
      - 已选集团军：显示名称/成员数 + Draw Frontline / Draw Arrow / Execute Plan / Halt Plan / Add Divs / Dissolve / Deselect 按钮；
      - 画线模式：顶部提示 "Drawing frontline / offensive arrow — Esc / right-click to cancel"。
    - 侧面板（I 键打开）：模板列表 + 师列表（详细信息）。
    - 点击地图上的前线省份自动选中该集团军（`selected_army_id`）。
    - 新增 `MilitaryCommand::SelectArmy / DeselectArmy / ExecutePlan / HaltPlan`。
    - 省份信息卡片 `bottom_bar_height` 动态偏移，避免与底栏重叠。
    - 文件：`crates/hoi4-ui/src/military.rs`、`crates/hoi4-app/src/main.rs`
    - _Requirements: 1.1, 1.3, 1.5, 1.6, 5.6, 9.6, 14.1_

  - [x] 11.3 `collect_frontline_arrows` 函数
    - 在 `App` 上实现 `fn collect_frontline_arrows(&self) -> Vec<ArrowInstance>`：
      - 遍历 `world.player_armies`：owner != player 且 `!show_all_units` 跳过（_R15.9_）；overlay 不可见跳过（_R9.6_）；
      - 有 path 即渲染（移除 `!order.active` 跳过——和平时期也显示战线）；
      - 战线身：`path.windows(2)` → `ArrowInstance { segment_type: 0.0, ... }`（_R9.1_）；
      - 箭头：`[anchor or last(path)] ++ arrow.provinces`，最后一段 `segment_type=1.0`，其余 `0.0`（_R9.2_）；
      - 当前预览（painter.samples）单独压一组（_R9.4_）。
    - 用 `unit_counter_centroids` × `WORLD_SCALE` 算坐标。
    - 文件：`crates/hoi4-app/src/main.rs`
    - _Requirements: 9.1, 9.2, 9.4, 9.5, 9.6, 15.9_

  - [x] 11.4 `MapArrowPass::set_arrows` 接入
    - 每帧 `update_frontline_arrows()` 调 `collect_frontline_arrows()` → `maparrow_pass.set_arrows(...)`（移除 dirty hash 优化，每帧上传）。
    - 启动时仅 `player_armies.is_empty()` 才画 mock arrows（便于调试）。
    - 文件：`crates/hoi4-app/src/main.rs`
    - _Requirements: 13.3_

  - [x] 11.5 集团军标签（名 + 成员数）渲染
    - 在路径质心位置用现有 `text_pass` 提交标签（_R9.3_）。位置 = path 中所有省 centroid 平均 → 3D→screen 投影。
    - 文件：`crates/hoi4-app/src/main.rs`
    - _Requirements: 9.3_

  - [x] 11.8 和平时期画线 + 执行计划机制
    - `FrontlineOrder` 新增 `executing: bool` 字段（玩家点击"执行计划"后为 true，师沿箭头推进）。
    - `frontline_snapper` / `validate_path` / `eligible_frontline_or_bridge`：非战时放宽条件，owner/cobel 控制的陆地省直接通过，不需要敌占邻居。
    - `set_frontline_path`：始终 `active=true`（师自动分布到战线），`executing=false`。
    - `tick_frontlines`：非战时仅清 `executing`，不再清 `active`；箭头推进仅 `executing=true` 时执行；箭头完成时同时清 `executing`。
    - 新增 API `execute_plan(world, id)` / `halt_plan(world, id)`。
    - 存档格式扩展：text 写 `executing=yes/no`，binary 多读/写 1 字节 u8。
    - 更新 `hoi4-ui/src/military.rs`：`ArmyEntry.executing`，底栏 `▶ Execute Plan` / `⏸ Halt Plan` 按钮。
    - 文件：`crates/hoi4-state/src/frontline.rs`、`crates/hoi4-logic/src/military/frontline.rs`、`crates/hoi4-state/src/save/text.rs`、`crates/hoi4-state/src/save/binary.rs`、`crates/hoi4-ui/src/military.rs`、`crates/hoi4-app/src/main.rs`
    - _Requirements: 2.2, 2.7, 4.5, 6.1–6.6, 7.4, 14.5_

  - [ ]* 11.6 Property 17：渲染叠加层尊重 owner 与 show_all_units
    - `crates/hoi4-app/tests/frontline_render.rs`（新建）。任意混合 owner 的 PlayerArmy + show_all_units 两态 → instance 来源校验。
    - **Property 17: 渲染叠加层尊重 owner 与 show_all_units**
    - **Validates: Requirements 9.6, 15.9**

  - [ ]* 11.7 单元测试：path/arrow 分段计数
    - 在同一文件加几条 `#[test]`：path 长 N → `N-1` 个 body 段；arrow 长 K → `K` 段（含起点 anchor 桥），最后 1 段为 head 类型。
    - _Requirements: 9.1, 9.2_

- [ ] 12. 性能与文档
  - [ ]* 12.1 性能基准（`#[ignore]`）
    - `crates/hoi4-app/tests/frontline_perf.rs`（新建）：(a) 100 师 × 64 省 distribution < 5 ms；(b) 1024 采样 snap < 5 ms；(c) 一国 `tick_ai_frontlines` < 1 ms。
    - 标记 `#[ignore]`；CI 不阻塞，本地 / perf job 跑。
    - _Requirements: 13.1, 13.2, 15.8_

  - [ ] 12.2 模块文档与交叉指引
    - 在 `crates/hoi4-state/src/frontline.rs` 顶部 `//!` 注明：本类型与 `command::Army` 语义不同（OOB 自动分组 vs 玩家划线 PlayerArmy）。
    - 在 `crates/hoi4-state/src/command.rs` 顶部加引用提示同件事。
    - 在 `crates/hoi4-logic/src/military/frontline.rs` 顶部 `//!` 列出 design.md `## 正确性属性` 节中 17 条属性的简短摘要。
    - 文件：以上三处。
    - _Requirements: 11.4, 15.1（语义说明）_

- [ ] 13. 最终检查点
  - 跑 `cargo test --workspace`（不含 `#[ignore]`）全绿。
  - 启动 `hoi4-app`，手动验证 1 局：选 GER → 选几个师 → 创建集团军 → 画战线 → 看 ArrowInstance 出现 → 加箭头 → 推进数日 → 看 destinations 与 controllers 正确翻转 → 存档 → 退出 → 加载 → 集团军/战线/箭头/anchor 复现。如有问题询问用户后修复。
  - _Requirements: 全部 1–15_

## 备注

- 带 `*` 的子任务是可选测试任务，可跳过以加速 MVP；核心实现任务（不带 `*`）必须完成。
- 每条任务都引用 design.md 节标题与具体的 Requirement ID + Property 编号（1–17），便于 spec-task-execution 单独执行。
- 任务 7.1（`tick_frontlines`）是单点最大风险——它绑定了 R3、R4、R6、R7、R14.5、R15.11 共计 13 条子需求；建议拆分实现：先空壳 + step 1.a/1.b（路径校验 + 塌陷），跑 Property 8；再加 step 1.c–e（分布 + destinations），跑 Property 12；再加 1.d/1.f/1.f'（arrow 执行 + 完成 + anchor 滚动），跑 Property 13/14/15。
- 文本与二进制存档格式扩展（任务 2.1、2.2）必须保留对旧档（无 `armies={…}` / 无 `FRARM` chunk）的兼容性；先写解析容错再写写入侧。
