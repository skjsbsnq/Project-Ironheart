# 局势 / 战争系统重构路线图

## 背景

当前的 `SituationState` 系统在以下两场战争中工作不正确：

- **意大利-埃塞俄比亚战争 (1936)**：进度条直接累加，1936-08-04 时意大利就已经"100% 进度"自动吞并 ETH，但地图上 ITA 师团一兵未动
- **西班牙内战 (1936-07-17)**：`SplitCountry` 创建 SPA，但 SPA 出生即空军零师团，进度条又脱离战场实际

根本问题：**局势系统在"自己玩自己的"——`base_progress` 凭空累加，`GradualTransfer` / `TransferStates` 直接改 `states.owners`，绕过了游戏真正的军事系统。**

## 设计原则

| 系统 | 角色 | 职责 |
|---|---|---|
| **军事系统**（hoi4-ai / hoi4-logic / divisions / frontline） | 真相 | 师团移动、控制权改变、占领判定 |
| **局势系统**（hoi4-content::situation） | 表达层 | 进度从战场反推；触发剧本事件、提供 UI 数据 |
| **脚本** | 事件点介入 | 开战、分裂、停战、签约——不每天偷偷改地图 |

进度条只是**派生数据**，不是驱动数据。

---

## Phase 0 — 拆除现有"假驱动"

**预估**：0.5 天
**目标**：把 situation 变成只读观察者，不再单方面改变世界状态

- [ ] 删除 `SituationEffect::GradualTransfer`
- [ ] 删除 `SituationEffect::TransferStates`（milestone 一次性转地）
- [ ] 删除 `daily_tick` 里 `base_progress` 自增逻辑；保留 def 数据但**进度从外部 setter 写入**
- [ ] `base_progress` 字段标 `#[deprecated]`
- [ ] 不再用 100% 触发 `AnnexCountry`
- [ ] 保留：`SplitCountry`、`CreateWar`、`TriggerEvent`、`AddIdea/Stab/WS`、新闻

**验证**：游戏跑到 1936-08-04 — ETH 与 SPA 都还在地图上、双方有兵、AI 在打（哪怕笨）

---

## Phase 1 — SCW 分裂时部队同步分

**预估**：1 天
**问题**：现在 `SplitCountry` 只转州，SPA 出生即空军零师团秒被推

- [ ] 新增 `SituationEffect::SplitDivisions { source_tag, rebel_tag, fraction }`
- [ ] 处理逻辑（main.rs SplitCountry handler 后续）：
  - 找到 `divisions.locations` 在 rebel 已得到的州里的师 → owner 改成 rebel
  - 同样处理 `air_wings` / `fleets`
- [ ] SCW 的 `on_start` 加这条，fraction 跟领土 fraction 一致（0.5）
- [ ] 同步 stockpile：`econ.stockpile[rebel] = econ.stockpile[source] * 0.5`，原方扣掉

**验证**：SCW 触发后两个西班牙都应该有几十个师在自己境内、有装备储备

---

## Phase 2 — 进度条改成战场反推 ✅

**预估**：0.5 天
**目标**：UI 显示的进度反映真实战况

- [x] `SituationDef` 新增字段 `progress_source: ProgressSource`：
  - `Manual` — 旧行为，用于无法从战场判断的事件
  - `TerritorialControl { side_country_tags: Vec<Vec<String>> }` — 一边对应一组 tag
- [x] `daily_tick` 重写：对 `TerritorialControl` 类型遍历相关 country 的 states
  - `side_0` 进度 = side_0 控制的州数 / 战区总州数 × 100
- [x] 战区定义：开战时刻属于战争双方（含核心州）的所有州的并集
- [x] `ActiveSituation` 加 `theater_states: Vec<StateId>` 缓存战区，开战时计算一次

**配置示例**：

- 意-埃：`side_country_tags = [[ITA], [ETH]]`，进度 = ITA 控制的"原 ETH+ITA 战区"州数
- SCW：`side_country_tags = [[SPA], [SPR]]`

**验证**：UI 进度条与"被占领的对方州 / 对方总州"一致，AI 推不动地图就进度条不涨

---

## Phase 3 — 真实结束条件 + 标准 peace flow ✅

**预估**：1 天
**问题**：现在 `progress >= 100` 触发 `AnnexCountry` 是假的；真正的胜利是占领所有州 + 首都

- [x] 删 `daily_tick` 里的 100% 检查
- [x] 新增 `check_military_victory(world, situation)` 每周调用：
  - 对 `TerritorialControl` 类型，看一方是否占领了对方**所有州** + 对方首都
  - 是 → 结束局势，触发 `on_end[winner]`
- [x] `on_end[winner]` 仍然包含 `AnnexCountry`，但现在是"军事完成后的正式吞并"
- [x] 加超时退出：超过 `end_date` → 按当前进度判定 winner（保留旧行为兜底）

**验证**：意大利只有真打到 Addis Ababa 才能赢；如果意大利兵被打散，战争会拖

---

## Phase 4 — 玩家干预：派志愿军 / 运装备 ✅

**预估**：1.5 天
**目标**：把"按钮 → 进度 +5"改成"按钮 → 真实兵 / 装备进入对方"

- [x] 新增脚本效果：
  - `LendDivisions { from, to, count, template_priority }` — 借出 N 个师，spawn 在 `to` 国首都所在省
  - `SendEquipment { from, to, equipment, amount }` — 从 from stockpile 抽装备给 to
  - `SendXpBuff { to, army_xp, air_xp }` — 直接 +XP 池
- [x] `Intervention` 新增 `effects: Vec<SituationEffect>` 字段（`{caller}` 占位符替换为干预国 tag）
- [x] 改造现有干预：SCW 和意-埃的"送兵 / 送装备 / 送顾问"全部走真实效果（不再 progress_boost）
- [ ] 借出的师在战争结束后归还（开个 `LentDivisionLedger` 记录）；MVP 先不还（已注明）

**验证**：玩家在 SCW 面板点 "派志愿军支持国民军" → 地图上 SPA 那边出现 GER 转移过去的师

---

## Phase 5 — 小国 AI 健全性 ✅（基础就绪）

**预估**：1-2 天（依赖 hoi4-ai 现状）
**目标**：保证 ITA / ETH / SPA / SPR 这些小国 AI 真的会调度部队

- [x] **现状盘点**：orchestrator 已对所有非玩家、非 annexed 国家迭代；只要 `is_at_war` 且
      `front.has_front()` 就会跑 `evaluate_ground` + `execute_ground_orders`
- [x] `distribute_to_frontline_defense` 让 Defend / Hold / Retreat 三种姿态都把师分散到
      与敌邻接的本国前线省，而不是全缩首都（这一点在 Task 4 中已落地）
- [x] `assign_attack` 智能化：每个师选 BFS 距离最近的可达目标，不再全冲首都
- [x] `daily_occupation_tick` 改为 vanilla 模型，"经过即占领"
- [x] 战斗参数 `ORG_DAMAGE_FACTOR=0.40` / `ORG_REGEN_PER_DAY=0.10` 让胜负出得来
- [ ] OOB 校准（意属东非 ~6 个师等）— 留给运行验证后调整，避免脱离 vanilla 数据
- [ ] SPA 出生时部队 morale / org sanity check — 同上，需运行验证

**验证**：暂停游戏看双方初始部署合理；速度5跑 30 天看是否有真实推进

---

## Phase 6 — 局势面板 UI ✅

**预估**：1 天
**目标**：让玩家看到战争实情，而非凭空进度条

- [x] 局势面板新增"军事概览"卡片：
  - 双方师数、装备储备（infantry_equipment）、参战国 tag 列表
  - 当前战区州控制情况（侧别 → 控制州数 / 总州数百分比）
  - 关键省份控制者（双方首都所在州当前 controller）
- [x] 关键事件时间线：保留 milestone 触发的新闻（已有）
- [x] 玩家干预按钮：显示冷却 + 上次干预日志（已有）

---

## Phase 7 — 内容扩展（按兴趣）

- 苏芬冬季战争（1939-11）
- 中日战争升级（1937-07 卢沟桥）
- 苏联吞并波罗的海三国（1940-06，纯外交不打仗 → 用 `AnnexCountry` 走完整流程，不用 situation）

---

## 时间预估

| 里程碑 | 包含 Phase | 累计天数 | 产出 |
|---|---|---|---|
| 最小可玩版 | 0-3 | **3 天** | 真实军队打仗、地图随战线推进 |
| 可交互版 | + 4-6 | **6-7 天** | 玩家可干预、UI 能看战况 |
| 内容齐全版 | + 7 | 视范围 | 多场历史战争都走相同框架 |

---

## 关键代码位置参考

| 系统 | 路径 |
|---|---|
| 师团存储 | `crates/hoi4-state/src/store.rs::DivisionStore` |
| 战争状态 | `crates/hoi4-state/src/diplomacy.rs::War` |
| 标准吞并 | `crates/hoi4-logic/src/diplomacy/peace.rs::annex_country` |
| 移动+控制权翻转 | `crates/hoi4-logic/src/military/movement.rs` |
| 前线计算 | `crates/hoi4-ai/src/frontline.rs::compute_front` |
| 地面 AI 决策 | `crates/hoi4-ai/src/ground.rs::evaluate_ground` |
| 地面 AI 执行 | `crates/hoi4-ai/src/ground_orders.rs::execute_ground_orders` |
| 局势 def / tick | `crates/hoi4-content/src/situation.rs` |
| 局势 def 实例化 / 效果处理 | `crates/hoi4-app/src/main.rs`（搜 `situation_state` / `SituationEffect`） |
| 标签刷新 | `crates/hoi4-app/src/main.rs::rebuild_country_labels_and_refresh` |

---

## 风险与注意

- **AI 成熟度是瓶颈**：如果小国 AI 不会主动调兵，战争会卡住。回退方案：脚本每月 emit 一个"AI ITA 必须保持至少 N 个师在东非前线"的目标
- **西班牙地形复杂**，AI 可能打成胶着；可先调高初始战力差让国民军倾向获胜
- **性能**：要确认师团调度 + 控制权更新对小国战争不会拖累主循环
- **存档兼容**：`SituationDef` 字段调整需要考虑序列化版本

---

## 已知遗留 bug（修复中 / 待修）

| 问题 | 状态 |
|---|---|
| SPA / SPR 标签倒置 | ✅ 已修（vanilla 中初始西班牙是 SPR） |
| ETH 注解后地图标签残留 | ✅ 已修（`MapnamePass::rebuild_instances`） |
| `spawn_country` 后 ResearchState / AiState 越界 panic | ✅ 已修（各自 `ensure_capacity`） |
| `AnnexCountry` 误删 annexer tag | ✅ 已修 |
| Weekly drain 后未刷新地图 LUT | ✅ 已修 |
| 战争节奏过快、地图不每日更新 | 🔧 本路线图 Phase 0-3 解决 |
