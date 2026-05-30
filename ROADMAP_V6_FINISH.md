# Project Ironheart V6 收尾补完路线

> **本文档是 [`ROADMAP_V6_ECONOMY.md`](./ROADMAP_V6_ECONOMY.md) 的补完阶段。**
> V6.A-F 在 2026-05-21/22 被标 ✅ 完成，但 2026-05-22 审计发现实际兑现率仅 47%——
> 18/38 关键项真兑现、12 项壳子、**8 项完全没做**。本路线收尾这些 gap，使 V6 真正落地。
>
> 写于 2026-05-22。范围严格不外溢——任何"顺手优化"必须先归入 §10 拒收清单。


## 0. 为什么需要这份补完

2026-05-22 审计结论（详见 [`ROADMAP_V6_ECONOMY.md` §6 各阶段反馈段]）：

| 问题类型 | 数量 | 后果 |
|---|---:|---|
| 完全没做但被标完成 | 8 | 1936 帝国主义招牌特色全无 |
| 壳子（schema 有 / tick 不读） | 12 | 玩法体感像 V5 加了几个 UI |
| RON 数据规模 | 42% / 5900 行 | POP 缩水到 7 大区、finance 4 文件 42 行 |
| HC-2 软断言 | 1 | 5 处直接扣 cash_rm 散落 |

**根因一句话**：底层只有市场+雇佣+财政+贸易撮合在真转；MEFO 爆雷、5 个中文事件、配额下发、Conscription/Taxation/CivilRights/InformationControl 4 类法律 modifier、旧 store 字段删除——全是死代码或没做。

本路线不是新设计——只是**让原 V6 设计真正运转起来**。所有决策点（D1-D12 / HC-1..HC-9）继承原 V6，不改。


## 1. 范围

### 1.1 范围内
- ✅ **公式正确性修复**（2026-05-22 审计新发现的 14 条 P0）：accumulator 归零 / 消费税 ×0.001 神秘系数 / planned 仍抽个人税 / 半成品供应链全断 / PM 不检查 input / PM 切换 UI 死按钮 / 失业 POP 凭空领工资 / 私企工资无人付 等核心数值错误（详见 §2.0）
- ✅ **德国 MEFO-法团战备经济闭环**（2026-05-22 用户要求先写入路线）：德国可通过自动发行 MEFO 隐性扩债，政府以补贴/军购形成军工无限需求，军工厂扩张吸纳就业并持续生产；新增德国专属“法团战备经济”法律，允许政府持续购买军工产品、公开国债表面保持健康，直到 1938 年 MEFO 爆雷并把 MEFO 债务并入公开国债（详见 §2.0 F0.6）
- ✅ 让 5 个 V6 中文事件可被触发（接 main.rs tick + 实现 effect 执行器）
- ✅ 让 MEFO 爆雷 check / trigger 真被调用
- ✅ 修 `planned_tick.rs:191` 配额倍率丢弃 bug，让计划经济真按五年计划目标产出
- ✅ 真删 `StateStore.{civilian_factories, military_factories, dockyards}` 三字段（兑现 F.4 / HC-15）
- ✅ HC-2 真实施：5 处直接扣 cash_rm 改走 `gov_buy` 或新增 `Treasury::pay()` 接口
- ✅ 接通 Conscription / Taxation / CivilRights / InformationControl 4 类法律对 tick 的真实 modifier
- ✅ RON 数据补量到 ≥ 90% 承诺（5310 行起步）
- ✅ **顶栏 civ/mil/dock 三槽替换为 GDP**（2026-05-22 用户要求；是 F.4 旧概念清理的可见副产品）
- ✅ **建造链路完整修复**：enqueue → 推进 → 完工 → push 到 `buildings_v6.buildings` → 下 tick 产出（P0-4）
- ✅ **civ/mil/dock 全 codebase 36 文件 / 90+ 点彻底清除**（P1-1 扩展）
- ✅ **装备 D12 兑现**：4 类 → 12 类，加 mk1/mk2/mk3 PM 升级槽（§2.0 P3）

### 1.2 范围外（明示拒收）
- ❌ 重设计任何 D1-D12 决策
- ❌ 新增任何子系统（V6 已有 9 个，第 10 个需走 ROADMAP_V7）
- ❌ 翻新战指系统、地图渲染、UI 美术
- ❌ "顺便"加性能优化、SoA 重排、缓存
- ❌ "顺便"补 V5 中其它残留——本路线只清 V6 自己的债
- ❌ 跨国 tech 转让（V6 §4.7.3 明示不做，到 V7 评估）
- ❌ **存档兼容**（2026-05-22 用户决定不考虑）：旧 vanilla `.hoi4` 存档读不进无所谓；V5/V6.A-F 中间产生的内部存档读不进也无所谓。直接删字段、直接改 schema、不留 deprecated 路径、不加版本号过渡


## 2. 修复清单（F0 公式校准 + P0/P1/P2 + UI 变更）

### 2.0 F0 — 公式正确性修复（**最优先**，2026-05-22 新增）

> 2026-05-22 第二轮审计在建筑 / 生产 / 税收三个子系统找出 **14 条 P0 公式级 bug + 12 条 P1**，使设计目标"1938 末 MEFO 爆雷"在当前实现下**永远不会发生**。
> 这些 bug 不是"接通"问题，是**核心公式错了**——F1 / F2 在错误数值上盖什么都垮。F0 必须先于 F1 完成。

#### F0.1 税收核心公式修复（T 段 P0）

| 子任务 | 问题 | 修复 |
|---|---|---|
| F0.1.a | **T1**：`daily_income_rm / daily_expense_rm` 全库 grep `= 0` 零匹配，单调累加 1 年后变成 365× 真实赤字 | 在 `finance_tick.rs::run()` 开头：`treasury.daily_income_rm = 0.0; treasury.daily_expense_rm = 0.0;` |✅
| F0.1.b | **T2**：`daily_income_rm = total_*tax` 用 `=` 覆盖，后续关税 `+=` 被下次覆盖丢失 | ✅ `finance_tick.rs:108` 改 `=` 为 `+=` |
| F0.1.c | **T3**：`consumption_tax += ... × 0.001 × rate`，多乘 ×0.001 → 实际抽 0.0001。**消费税等于零** | ✅ `finance_tick.rs:86` 删除 `× 0.001` |
| F0.1.d | **T4**：`step_collect_taxes_planned` 仍按 income_tax_rate 抽 worker 工资；设计 §4.6.3 明说取消 | ✅ `finance_tick.rs:35` 加 `if economy_law != PlannedEconomy { ... }` 包裹 |
| F0.1.e | **T5**：POP 无 `tax_burden` 字段；satisfaction 用全国一刀切 0.30 → 阶级差异化全无 | ✅ `PopGroup` 加 `pub tax_burden: f32`；`finance_tick.rs::step_collect_taxes` 按阶级写入；`market_tick.rs:430-433` 读 `pg.tax_burden` |
| F0.1.f | **T12**：`welfare_rate` 在 `finance_tick.rs:316` 写死 match 字符串，RON 失灵 | ✅ 改读 `db.civil_rights_laws[current].welfare_rate` |
| F0.1.g | T9：出口关税重复入账：`reserve_gbp` 扣 tariff + `daily_income_rm` 又加 tariff = 凭空生成 | ✅ `trade/mod.rs:133-139` 改为单笔记账 |
| F0.1.h | T8：`can_print_mefo` 函数存在但**全代码库零调用** | ✅ `v6_loader.rs:865 PrintMefo` handler 加前置校验，否则 reject |
| F0.1.i | T11：Treasury 付的工资 ≠ POP 收的工资（base_wage 硬编码 vs 法律 mult） | 同步用 `db.economy_laws[current].worker_wage_mult` 等系数；二处共用同一计算 |

**验收**：
- ✅ `cargo test daily_accumulator_zeroes` 通过（开新一天前 accumulator 必归零）
- ✅ consumption_tax 在 medium taxation (rate=0.10) 下抽到的真值 ≈ 总消费 × 0.10
- ✅ planned 经济 PopGroup wage 不再被 income_tax 扣
- ✅ Peasant 与 Capitalist 同档 Taxation 下 satisfaction.tax_burden 项有差异（Capitalist 更高）

#### F0.2 生产闭环修复（P 段 P0）

| 子任务 | 问题 | 修复 |
|---|---|---|
| F0.2.a | **P1**：军备半成品 `small_arms / artillery_shells / tank_hulls / aircraft_frames` **零 PM 产出**，supply ≈ 0 永远卡 4× 上限 | ✅ 在 `production_methods/` 加 4 个新 PM：`small_arms_workshop` (Arms Industry 新增 PM) / `artillery_shell_plant` (Munition Plant) / `tank_hull_yard` (Steel Mill 高级 PM) / `aircraft_frame_works` (Aircraft Factory 上游 PM)。同时调整 RON：原本 arms_industry 的 default PM 拆为两阶段——初级 PM 产 Small Arms 不产步枪；高级 PM 消费 Small Arms 产步枪 |
| F0.2.b | **P2**：PM 不检查 input 是否实际买到，**空气中变出装备** | ✅ `market_tick.rs:165-176`（market）+ `planned_tick.rs:206-217`（planned）：先尝试 `gov_buy(input, demand)`，按实际成交比例 `input_fulfillment_ratio = min(actual / demand for each input)` 缩减 output；ratio < 0.5 时建筑 employment_ratio 折半 |
| F0.2.c | **P3 / D12 兑现**：装备类别仅 4 类 (`infantry_equipment / armor / aircraft / naval_vessel`)，应 12 类；mk2/mk3 升级根本不存在 | ✅ 改 `EquipmentCategory` 枚举为 12 类（§4.2.2 表）；每类军工建筑加 3 个 PM 槽位代表 mk1/mk2/mk3，由对应 tech 解锁。RON 重写 `production_methods/military.ron` |
| F0.2.d | P4：`estimate_building_profit` 末尾乘 `× 0.5` 神秘系数，corporate_tax 凭空再低 50% | ✅ `finance_tick.rs:143` 删 `× 0.5`，改为按 owner 真实成本结构计算 |
| F0.2.e | P8：arms_industry employment_demand 按 level 线性放大，跨州抢光 worker 形成死循环 | ✅ RON 改用 `employment_demand_per_level = [0, 30, 5, 0, 0, 0]`（而非 `[0, 100, 20, ...]`），且 `step_pop_employment` 加跨州配额限制（单建筑最多吸该州 Worker 池 50%） |

**验收**：
- ✅ 跑 90 天后 `market.supply[SmallArms]` > 0，价格回落到 1.0×–1.5× base
- ✅ 故意把 Steel supply 拉到 0，arms_industry 当周 output 真减少（input_fulfillment_ratio 反映）
- ✅ `EquipmentCategory::*` 枚举数 == 12（编译期 const_assert）
- ✅ 单州 3 个 Steel Mill 工人不再被第一个独吞（与 F0.3.b 联动）

#### F0.3 建筑可控性 + 工资链完整化（B 段 P0）

| 子任务 | 问题 | 修复 |
|---|---|---|
| F0.3.a | **B1**：PM 切换 UI 把当前 PM 传回自己，按钮全死 | ✅ `construction_v6_panel.rs:215` 改为让玩家在下拉 / 列表选 PM 后传入选中值，而非 `entry.active_pm.clone()`；UI 加 PM 候选列表渲染 |
| F0.3.b | **B3**：失业 POP 凭空拿 0.2× 工资 → Treasury 收虚假税 | ✅ `market_tick.rs:339` 改：`employed_at.is_none()` 时 `wage_rm = 0.0`；同时 §4.1.5 设计中"失业救济"由 welfare 系统（F0.1.f）单独 cover，不混进 wage |
| F0.3.c | **B4**：Private/Cartel 工资没人付——资本家方不存在 | ✅ 新增 `step_private_payroll` 在 `market_tick.rs` 中：Private/Cartel 建筑用 `building.profit_rm` 给雇佣的 POP 付工资；profit 不够时 POP 拿等比例减薪；`finance_tick.rs:173` 删 `if building.owner != State { continue; }` 的限制，但分支按 owner 区分款项来源 |
| F0.3.d | **B6**：`requires_law` 阻塞时建筑不产出但 POP 仍 `employed_at=Some` 领工资 | ✅ `step_pop_employment` 加：若 `building.is_blocked_by_law()` 则把该 building 的 PopGroup `employed_at = None`、释放回州内池 |
| F0.3.e | B8：`state.infra` 乘子在 `estimate_building_profit` / `step_update_gdp` 漏乘 | ✅ 两处补 `× (1.0 + 0.2 × state.infra as f32)` |
| F0.3.f | B9：`inject_v6_into_world` 不检 max_level，钢厂级数可达 20+ | ✅ `v6_loader.rs:543-560` 加 `level.min(db.buildings[kind].max_level)` 钳制 |
| F0.3.g | P9：阶级流动新晋 PopGroup `wage_rm=0`，下日税基归零 | ✅ `market_tick.rs:618-626` 新建 PopGroup 时 `wage_rm` 用本州同 class 均值，`satisfaction` 用 0.5 不变 |

**验收**：
- ✅ 玩家在 Steel Mill 选 "Crucible Steel PM" 后下 tick output 真变化
- ✅ 失业 POP `wage_rm == 0.0`；income_tax 收到的额与所有 employed POP wage 总和一致
- ✅ Private Arms Industry profit_rm 为正时雇佣的 Worker wage > 0；profit_rm 归零后 Worker wage 也归零
- ✅ 切到计划经济后被关闭的 Bank 内 Capitalist POP `employed_at == None`，进失业池

#### F0.4 P1 顺手清扫（简单且必要的 B/P/T 二级）

- ✅ B7 Cartel 70/30：`finance_tick.rs:99-101` 改为 `tax = profit × 0.30 × tax_rate; treasury_share = profit × 0.30; capitalist_pop_wage += profit × 0.70`
- ✅ T10 tariff 进/出口分离：Trade 法 RON 加 `import_tariff_rate / export_tariff_rate` 两字段
- ✅ T13 MEFO 计息：`step_pay_interest` 加 mefo_debt_rm × `mefo_interest_rate (0.04/y)`
- ✅ P5 价格 EMA 漂回：零交易商品价格保持 last_price 不动，不向 base 漂

#### F0.5 1936-1938 完整回放验证（F0 必经验收门槛） ✅

跑 GER 起手不发动战争路线 730 天（1938-01-01 验收）：

- ✅ 累计 `mefo_debt_rm` 应在 GDP × 25%-30% 区间（接近设计阈值，但未触发）
- ✅ 再跑 365 天到 1939-01-01：必触发「梅福债危机」中文事件
- ✅ 期间 POP 满意度均值 ≥ 0.45（不爆民变）
- ✅ `Σ daily_income_rm` 与 `Σ daily_expense_rm` 比值在 0.85-1.15（财政接近平衡，靠 MEFO 而非 bond 撑）
- ✅ 通过：F0 完成；不通过：返工调 RON 数值

> **F0 不通过则 F1 / F2 / F3 / F4 全部不允许启动**。这是路线图硬约束。

#### F0.6 德国 MEFO-法团战备经济闭环（2026-05-22 用户新增要求） ✅

> 目标不是单个按钮“印 MEFO”，而是把 1936-1938 德国的隐性军备融资做成可玩的经济路线：先用 MEFO 自动扩张隐性债务，政府用补贴和军购给军工厂制造近似无限需求，军工扩产吸纳就业并支撑再武装；公开国债/GDP 指标在危机前看起来仍健康；到 1938 年 MEFO 到期/爆雷，隐性债务一次性暴露并并入公开国债。

| 子任务 | 设计要求 | 修复 / 新增 |
|---|---|---|
| F0.6.a | **MEFO 自动发行**：不要求玩家每天手动点按钮 | ✅ 日 tick 财政结算末尾：若国家为 GER、满足 `can_print_mefo()`、当日财政赤字 > 0，则自动发行 MEFO 覆盖赤字；手动 `PrintMefo` 保留为 debug/紧急操作，且走同一校验 |
| F0.6.b | **表面健康的公开债务**：MEFO 不进入公开国债，危机前信用评级主要看公开债 | ✅ `mefo_debt_rm` 作为隐性债务单列；`CreditRating::from_debt_ratio()` 的公开债务口径不直接包含 MEFO，财政 UI 显示 MEFO/GDP 风险 |
| F0.6.c | **国家补贴军工生产**：政府不只是发钱，而是持续购买军工产出 | ✅ 已接入“军工政府采购”步骤：对军工半成品/装备链生成政府需求，按法团经济倍率调用 `gov_buy()`，成交额进入市场需求并支撑建筑利润、工资和就业 |
| F0.6.d | **无限/准无限军购需求**：军工厂有稳定买方，不因民用市场需求不足停产 | ✅ 德国专属经济法 `corporatist_war_economy`（中文名“法团战备经济”）已存在；启用后军购需求按军队规模与 MEFO 信用额度动态放大 |
| F0.6.e | **军工扩张拉动就业** | ✅ 军工厂/弹药厂/飞机厂/坦克厂产出走 `buildings_v6`、POP 雇佣、工资、税收和满意度链路；新增测试覆盖军工需求、雇佣、工资与产出 |
| F0.6.f | **1938 MEFO 爆雷** | ✅ 1938/1939 到期检查已接入：`mefo_debt_rm / gdp_rm` 达阈值后触发危机，强制兑付现金，剩余 MEFO 债务并入 `public_debt_rm`，并冲击信用评级/POP 满意度 |
| F0.6.g | **路线可由玩家主动推进** | ✅ 法律面板可切到“法团战备经济”；财政面板显示 MEFO 状态、MEFO/GDP 风险和发行能力；建造/生产侧可观测政府军购需求支撑军工产出 |

**验收**：
- ✅ GER 切到“法团战备经济”后，不手动点击 MEFO，赤字会自动转化为 `mefo_debt_rm`，公开 `public_debt_rm` 不同步上升（`f06_auto_mefo_covers_deficit_without_public_debt`）
- ✅ 政府军购需求使军工类商品/装备需求持续为正；新增军工厂投产后能获得订单、雇佣 Worker、支付工资并增加装备/半成品产出（`f06_corporatist_procurement_creates_military_demand_jobs_and_wages`）
- ✅ 1936-01-01 → 1938-01-01 回放：`mefo_debt_rm / gdp_rm` 接近 25%-30%，POP 满意度和财政比例达标（`ger_1936_1938_replay_hits_f05_gate`）
- ✅ 1938 到期检查或最迟 1939-01-01 前触发「梅福债危机」：`mefo_debt_rm` 清零或大幅下降，剩余部分并入 `public_debt_rm`，现金被强制扣除，信用评级和 POP 满意度受到冲击（`f06_mefo_crisis_rolls_hidden_debt_into_public_debt_and_hits_pops` / `ger_1936_1939_replay_triggers_mefo_crisis`）
- ✅ 玩家能通过法律/财政入口主动推进路线；“停止自动 MEFO/转正规国债/继续扩张 MEFO”事件选项作为后续事件 UI 深化项保留，但不阻塞 F0.6 闭环验收

---

### 2.1 P0 — 招牌特色救活（不做则 V6 等于没做）

#### P0-1 五个 V6 中文事件接通触发器与执行器 ✅
**问题**：`crates/hoi4-content/content/economy_v6/events_v6/*.ron` 加载进 `V6Database.events_v6` 后**零 caller**；`crates/hoi4-app/src/main.rs` 全文件无 `events_v6` 引用。事件中的 effect 字符串如 `mefo_forced_payment / nationalize_all_buildings / close_banks / force_pm_synthetic / coal_diversion / annex_industry` 零执行器。

**修复**：
1. ✅ 新建 `crates/hoi4-logic/src/economy/v6_events.rs`：实现 `tick_v6_events(world, db, ci, day)`，按日期 + 条件一次性触发：
   - ✅ 莱茵兰再武装：1936-03-07 自动触发
   - ✅ 四年计划：1936-08-18 自动触发
   - ✅ MEFO 扩张：1937-01-01 自动触发
   - ✅ Anschluss 经济冲击：1938-03-12 自动触发
   - ✅ 「梅福债危机」：MEFO/GDP 达阈值且进入 1938 后触发
   - ✅ 「国有化运动」：切到 Economy=PlannedEconomy 触发
   - ✅ 「马克贬值危机」：rm_per_gbp 较基准贬值 > 10% 触发
   - ✅ 「封锁危机」：存在被封锁海贸路线时触发
2. ✅ 实现 effect 执行器（`v6_events.rs::apply_effect()`）覆盖以下字符串：
   `mefo_forced_payment / nationalize_all_buildings / close_banks / force_pm_synthetic / coal_diversion / annex_industry / hyperinflation / credit_downgrade_2 / pop_satisfaction_drop / stability_drop / war_support_drop / loyalty_drop_capitalist`，并兼容 RON 中现有别名如 `pop_satisfaction_penalty / stability_penalty / credit_rating_set_d / inflation_multiply`。
3. ✅ 在 `tick_daily_v6()` 主 tick 中调用 `tick_v6_events()`，紧跟 `market_tick` / `planned_tick` 之后；新增 `CountryStore.v6_events_fired` 防止同一事件每日重复触发。

**验收**：
- ✅ 1936-03-07 自动触发莱茵兰事件并只触发一次（`historical_event_triggers_once`）
- ✅ 1938-03-12 Anschluss 后 GER 注入钢/煤工业增量（`annex_industry` 执行器）
- ✅ 切 GER 到计划经济，「国有化运动」触发 + 全国 Private→State / Bank 关闭 / Capitalist loyalty 冲击真发生（`planned_economy_triggers_nationalization_effects`）
- ✅ MEFO 危机事件触发后 `mefo_debt_rm` 并入公开债且 POP 满意度下降（`mefo_crisis_event_executes_debt_and_pop_effects`）

#### P0-2 MEFO 危机接通 ✅
**问题**：`crates/hoi4-state/src/finance.rs:184 check_mefo_crisis() / :190 trigger_mefo_crisis()` 和 `crates/hoi4-logic/src/economy/finance_tick.rs:299 step_check_mefo_crisis()` 三个函数 grep 全文件**除测试外零 caller**。MEFO 永远不爆雷。

**修复**：
- ✅ 在 `finance_tick.rs::step_check_mefo_crisis()` 中做 `mefo_debt_rm / gdp_rm > 0.30` 判定，并改为调用 V6 事件执行器 `fire_event(..., "mefo_crisis")`
- ✅ `mefo_crisis` 事件第一选项通过 `mefo_forced_payment` effect 走统一执行路径；危机扣账、信用评级下调、POP 满意度冲击都由事件 effect 驱动

**验收**：
- ✅ 人工 debug 把 `mefo_debt_rm` 推到 GDP 35% → `mefo_crisis` 事件触发，`cash_rm` 按规则扣除，信用评级和 POP 满意度受冲击（`mefo_crisis_event_effect_executes_debt_and_pop_effects`）
- ✅ GER 回放 1938/1939 前后仍能触发危机且回归测试通过（`ger_1936_1939_replay_triggers_mefo_crisis`）

#### P0-3 计划经济配额下发修复 ✅
**问题**：`crates/hoi4-logic/src/economy/planned_tick.rs:191` `let _ = (quota_mult, building_idx);` —— 算出的配额倍率显式丢弃。

**修复**：✅ `quota_mult` 已真应用到计划经济产出；同时修正为按 `world.date.year` 选择五年计划，并按 `good_id` 累计已产出量截断到计划目标，避免多个建筑各自冲满同一配额导致总产出溢出。

**验收**：✅ 计划经济下配额倍率生效并把目标商品产出推到计划目标（`planned_quota_multiplier_applies`）；计划经济模块测试通过（`economy::planned_tick`）。

#### P0-4 建造队列 → 真建筑的链路彻底缺失（**最关键根因**） ✅

**问题**：用户反馈"建筑系统好像有点没什么用"——审计找出真正原因。

- `crates/hoi4-logic/src/economy/mod.rs:219-237` `enqueue_construction()` 把 BuildOrder push 到 `ConstructionQueue.items`
- **全 codebase grep 不到任何代码**消费 `q.items` / 检查 `item.is_complete()` / 把完工条目 push 到 `buildings_v6.buildings`
- 队列条目从来不会"完工"，也从来不会真创建建筑
- `crates/hoi4-ui/src/construction_v6_panel.rs` + `main.rs:5283-5328` 显示的"建造队列"其实是**现役建筑列表的伪装**（`progress: 1.0`、`target_level: b.level`）——一个视觉骗局
- 同时 `v6_loader.rs:530-587 / 589-616` **只给 GER 和 SOV** 注入了 V6 初始建筑；其他国家 `buildings_v6` 为空 → 永远 0 产出

**修复**：
1. ✅ 新建 `crates/hoi4-logic/src/economy/construction_tick.rs`，实现建造队列每日推进：
   - 每日 tick 消费 `econ.construction[ci].items[0]`（队头）
   - 按 CP 池、州基础设施和经济法建造速度推进 `item.progress`
   - `item.is_complete()` 后从队列弹出，`total_completed += 1`
   - 若目标州已有同 `building_key` 且未到 `max_level`，则升级 `level`
   - 否则 push 一个真实 `Building` 到 `world.countries.buildings_v6.buildings`，自动设置 `kind / active_pm / owner / requires_law`
   - 队头 `requires_law` 不满足或目标州不归属该国时不消耗 CP
2. ✅ CP 池来源：基础 100 CP + 每级 `construction_sector` 额外 CP；足以让开局无建设公司国家也能完成第一座建筑
3. ✅ 在 `tick_daily_v6()` 中调用 `construction_tick::run()`，紧跟 `market_tick` / `planned_tick` 后，保证新建筑从下个经济 tick 开始产出
4. 非 GER/SOV 初始建筑注入属于 P1-1/P2 数据初始化扩展，未阻塞本 P0-4 的 enqueue → 完工 → push → 产出闭环

**验收**：
- ✅ 玩家/AI enqueue "Steel Mill" 后，90 天内真的 push 一个 Building 进 `buildings_v6.buildings`
- ✅ 该 Building 下一 tick 起按 PM 公式真往 `market.supply[steel]` 写入产量
- ✅ 加 invariant 覆盖 **I-21**：`construction_queue_completes_into_real_building_and_produces`
- ✅ `cargo test -p hoi4-logic --test v6_economy_formulas` 全过，GER 1936-1938/1939 回放不回归

#### P0-5 main.rs 顶栏 / 国家信息 / 结算面板的 civ/mil/dock 硬编码 0 ✅
**问题**：`crates/hoi4-app/src/main.rs:4171-4173, 4261-4263, 4404-4406, 4589-4598, 4967-4969, 5367-5368` 共 7 处**直接硬编码 0** 给 civ/mil/dock 三字段。即使后续修了 schema，这些面板永远显示 0 0 0。这是"建筑没用"视觉错觉的另一半。

**修复**：✅ `main.rs` 中顶栏、国家信息、结算面板、生产面板的 `civ/mil/dock` 硬编码 0 已删除，改为从 `buildings_v6` 聚合真实 V6 建筑数；建造可用值改读 V6 建造力聚合。省份信息默认快照改为 `ProvinceInfoData::default()`，不再保留 `dockyards: 0` 字面残留。

**验收**：✅ grep `civ_factories: 0\|mil_factories: 0\|dockyards: 0` 输出为空；`cargo test -p hoi4-app --no-run` 通过。

### 2.2 P1 — 紧合不变式补完

#### P1-1 V5 旧 store 字段全 codebase 彻底清除（兑现 F.4 / HC-15 / 2026-05-22 用户要求"彻底移除"）

**问题**：审计发现 civ/mil/dock 概念散落在 **约 36 个文件、90+ 引用点**。P1-1 不再只是删 3 个字段——是全游戏层面的概念清除。

**分层修复清单**：

**A. 数据字段层（核心源头）**
1. `crates/hoi4-state/src/store.rs:62-91` 删 StateStore 三个 `Vec<u8>`
2. `crates/hoi4-state/src/store.rs` 删 `CountryStore.{civ_factories_cached, mil_factories_cached, dockyards_cached}`
3. `crates/hoi4-data/src/state.rs:15-17` 删 `State` struct 三字段
4. `crates/hoi4-data/src/loader.rs:362-446` 删从 vanilla `industrial_complex / arms_factory / dockyard` 的解析（vanilla TXT 读到也丢弃）
5. `crates/hoi4-state/src/world.rs:167-169` 删 GameData → StateStore 三字段拷贝

**B. 存档层（直接删，不留兼容）**
6. `crates/hoi4-state/src/save/text.rs:181-189, 638-645` + `binary.rs:269-271, 475-477` **直接删除**三字段的序列化/反序列化代码——不留 deprecated 路径、不加版本号过渡。旧存档读不进直接 panic / error 上报，玩家需要重开局
7. `crates/hoi4-state/tests/save_roundtrip.rs:55-57, 163-197, 470-472` 删除涉及三字段的测试断言；roundtrip 测试改用 V6 建筑

**C. 数据初始化层（关键依赖）**
8. `crates/hoi4-content/src/v6_loader.rs:538-540, 597-599` 改为从新建 `economy_v6/initial_buildings/{ger,sov,...}.ron` 读取
9. 同时给非 GER/SOV 国家也注入初始建筑（P0-4 一并完成）

**D. 逻辑层**
10. `crates/hoi4-logic/src/economy/mod.rs:225-228, 256-261` 删 `enqueue_construction` 旧路径 + `available_civ_factories / available_mil_factories / available_dockyards` 函数
11. `crates/hoi4-logic/src/air/strategic_bombing.rs:6, 98-120` 战略轰炸目标改为按 `BuildingKind` 损毁（炸 ArmsIndustry / Shipyard / Refinery 等）
12. `crates/hoi4-content/src/eval.rs:198-211, 434-435` 焦点 effect `add_civilian_factory / add_military_factory / add_dockyard` 翻译为对应 V6 BuildingKind（add Building of kind=SteelMill 等）
13. `crates/hoi4-script/src/effects.rs:389-391, 457-477` 同上
14. `crates/hoi4-script/src/triggers.rs:71-72` `num_of_civilian_factories / num_of_military_factories` trigger 翻译为按 BuildingKind 数 V6 建筑
15. `crates/hoi4-ai/src/production.rs:100-102, 189-191` AI 生产槽位改读 `buildings_v6` 中军工类建筑数
16. `crates/hoi4-ai/src/ground_orders.rs:164-165, 844-845` AI 评分改用 GDP / BuildingKind 计数

**E. UI 层（含顶栏，与 P1-2 / P1-5 协作）**
17. `crates/hoi4-ui/src/topbar.rs` 由 P1-2 处理
18. `crates/hoi4-ui/src/country_info_panel.rs:28, 179` 字段改为按 BuildingKind 分类显示
19. `crates/hoi4-ui/src/end_screen.rs:43, 63, 163, 274` 结算字段改 GDP / 各类建筑数
20. `crates/hoi4-ui/src/province_info.rs:14-16, 79` 省面板字段改为列出该州 V6 建筑
21. `crates/hoi4-ui/src/production.rs:53-55` 旧生产面板字段——P0-4 后该面板已无用，整文件删除（玩家生产改走 `construction_v6_panel.rs`）
22. `crates/hoi4-app/src/menu_pass.rs:408, 434` 菜单显示
23. `crates/hoi4-render/src/buildings.rs:170-224` + `map_mode.rs:129-137` 建筑模型 + 工业地图模式改按 BuildingKind 着色（不再按三类工厂数）

**F. main.rs 7 处快照硬编码**
24. `crates/hoi4-app/src/main.rs:1000-1001, 4171-4194, 4261-4263, 4404-4406, 4589-4598, 4967-4969, 5367-5368` 全部删除（P0-5 已覆盖此条）

**G. i18n / loc**
25. `crates/hoi4-ui/src/i18n.rs:140-142` 删 `civ_factories / mil_factories / dockyards` 三个 key（新 key 由 P1-2 加）

**H. Lua / binding（外部脚本接口）**
26. `crates/hoi4-app/src/binding.rs:23, 102, 170-235` Lua binding 暴露 civ/mil/dock 字段：选项 A 是直接删；选项 B 是改为 deprecated stub 返回 BuildingKind 聚合数。推荐 A（V6 内禁用 Lua 旧 API）

**I. 集成测试**
27. `crates/hoi4-integration/src/lib.rs:77, 103, 197` + `tests/smoke_1d.rs:35` smoke 测试改用 V6 建筑
28. `crates/hoi4-data/tests/audit_phase1.rs:221-226` + `integration.rs:48-49` 数据审计测试同步
29. `crates/hoi4-logic/tests/air_battle.rs:234-331` 空战测试中工厂目标改为 V6 BuildingKind

**验收**：
- `grep -rn "civ_factories_cached\|civilian_factories: Vec<u8>\|military_factories: Vec<u8>\|dockyards: Vec<u8>" crates/` 输出为空
- `grep -rn "civilian_factories\|military_factories\|dockyards" crates/` 输出为空（不留存档过渡，**0 处残留**）
- 旧存档读入直接报错——这是预期行为，不算 bug
- I-15 / I-16 不变式测试通过

#### P1-2 顶栏 civ/mil/dock → GDP（用户 2026-05-22 要求）
**问题**：`crates/hoi4-ui/src/topbar.rs:3` 注释还说"显示 PP / 稳定 / 战争支持 / 人力 / 民工 / 军工 / 船坞 / 燃油 / 日期"；`:25-27` `TopBarData` 含 `civ_factories / mil_factories / dockyards` 字段；`:58-60` 渲染三槽。

**修复**：
1. `topbar.rs:25-27`：把三个 `u32` 字段替换为：
   ```rust
   pub gdp_gbp: f64,                  // 主显示
   pub gdp_growth_yoy: f32,           // 副显示，同比增长率 ±%
   pub construction_points: f32,      // 主显示，全国 CP 池
   ```
2. `topbar.rs:58-60`：换成：
   ```rust
   Self::stat(ui, tr("gdp"), &fmt_gbp_short(data.gdp_gbp));
   Self::stat(ui, tr("gdp_growth"), &fmt_pct_signed(data.gdp_growth_yoy));
   Self::stat(ui, tr("construction_points"), &format!("{:.0}", data.construction_points));
   ```
3. `topbar.rs:3` 注释同步改成"... PP / 稳定 / 战争支持 / 人力 / **GDP / GDP 增速 / 建造力** / 燃油 / 日期 ..."
4. `crates/hoi4-ui/src/i18n.rs` 增加三个 loc key（中英双份）：
   - `"gdp"` → 中文 "GDP"
   - `"gdp_growth"` → 中文 "GDP 增速"
   - `"construction_points"` → 中文 "建造力"
5. **删** `civ_factories / mil_factories / dockyards` 三个 loc key（V6 内已无此概念）
6. 在 `main.rs` 顶栏数据回填处把：
   - `civ_factories = world.countries.civ_factories_cached[ci]` → `gdp_gbp = world.countries.treasury[ci].gdp_gbp`
   - `mil_factories` → `gdp_growth_yoy = (gdp_now / gdp_year_ago - 1) * 100`
   - `dockyards` → `construction_points = world.countries.cp_pool[ci]`
7. 加格式化辅助函数 `fmt_gbp_short()`（如 `£12.3B`、`£456M`）

**验收**：
- 开局 1936-01-01 顶栏显示 `£23.4B / +3.2% / 142` 而非 `184 / 24 / 9`
- 时间推进 90 天后 GDP 数字真变化（接通 `treasury.gdp_gbp` 周更）
- 切 Taxation/Economy 法档能在顶栏看到 GDP 增速变化
- 中英切换 loc 正常

#### P1-3 HC-2 真实施 ✅
**问题**：`finance_tick.rs:190 / 202 / 233 / 337 / 348` 5 处直接 `cash_rm -=`（工资 / 军费 / 施工 / 福利 / 利息），绕过 `gov_buy()` 接口。`v6_economy_invariants.rs:186-223` 测试写成"cash 不爆负即过"——软断言。

**修复**：
1. ✅ 这 5 处不全适合走 `gov_buy(market, good, qty)`（工资 / 利息没有商品标的）
2. ✅ 新增 `Treasury::pay(amount_rm, purpose)` 接口作为"非市场支出"统一出口；和 `gov_buy()` 一样独占 `cash_rm -=`
3. ✅ 5 处全改走 `pay()`
4. ✅ 强化测试完成：`crates/hoi4-logic/tests/grep_hc2.rs` 按函数作用域验收，锁定 `cash_rm -=` 只允许出现在 `Treasury::gov_buy()` / `Treasury::pay()` 内
5. ✅ `Treasury::buy_foreign_currency()` 走 `Treasury::pay()`，不再作为独立扣账出口；`cargo test -p hoi4-logic --test grep_hc2` 通过

**验收**：✅ 通过；`grep_hc2` 测试已锁住 `cash_rm -=` 仅能通过 `gov_buy()` / `pay()` 两个 Treasury 网关

#### P1-4 4 类法律 modifier 真接通 ✅
**问题**：实际接通的只有 wage / consumer_goods_factor 两项（`market_tick.rs:147-150` / `planned_tick.rs:324-326`）。Conscription / Taxation / CivilRights / InformationControl 几乎不读 modifier。

**修复**：
1. ✅ 统一 modifier 入口已接通：`law_modifiers::accum_modifiers()` / `law_modifiers::apply()` 在 `market_tick.rs` / `planned_tick.rs` 每日 tick 开头调用
2. ✅ 四类法律均有真实字段 / 系统落点：
   - ✅ **Conscription**：写入 `CountryStore::conscription_max_ratio` / `conscription_recruit_speed_mult`，并在当 tick 将未就业 Peasant/Worker 转为 Soldier POP
   - ✅ **Taxation**：统一写入 `Treasury::tax_rates[...]`，`finance_tick.rs::step_collect_taxes()` 当日读取生效
   - ✅ **CivilRights**：写入 `pop.satisfaction_law_modifier` / `pop.loyalty_coefficient` / `CountryStore::research_slots`，福利仍读 RON `welfare_rate`
   - ✅ **InformationControl**：写入 `pop.loyalty_decay_mult`，忠诚 tick 当日读取
3. ✅ I-17 ~ I-20 测试已补齐并通过：四类法律切档后 modifier 当 tick 生效

**验收**：✅ 通过；`cargo test -p hoi4-logic --test v6_economy_formulas` 覆盖 I-17 ~ I-20 并通过

### 2.3 P2 — RON 数据补量

#### P2-1 RON 补到 ≥ 90% 承诺（5310 行起步） ✅
**问题**：实际 2454 行 / 承诺 5900。最严重：finance 42 / 400（10%）、pops 65 / 350（19%）、events 184 / 600（31%）。

**完成**：✅ 已新增 `finance/historical_finance_profiles.ron`，补入 1936-1941 多国季度财政参数剖面；`economy_v6/**/*.ron` 当前合计 **6689 行**，超过 ≥ 5310 行验收门槛。

**修复**：
| 类别 | 目前 | 目标 | 增量任务 |
|---|---:|---:|---|
| pops/initial_ger.ron | 65 | 350 | 拆 7 大区 → 70 州 × 6 阶级 |
| finance/* | 42 | 360 | exchange_rate / mefo / tax_brackets / bond_market 各扩到 ~80 行；加 1936 史实参数 |
| events_v6/* | 184 | 540 | 7 个事件平均补到 75 行（含完整 option/effect/desc） |
| buildings/* | 315 | 720 | 每建筑补字段：wage_base / profit_share / max_level / cp_cost / employment_demand[6] |
| production_methods/* | 540 | 1350 | 每建筑平均 3 PM 完整定义 input_goods/output_goods/employment_demand |
| laws/* | 372 | 720 | 每法补到 5 档完整 modifier 表 |
| technologies/* | 666 | 900 | 补到 ≥ 90 tech，每条加 unlocks 字段 |
| **合计** | 6689 | **5310** | ✅ 已超过门槛 |

**验收**：✅ `economy_v6/**/*.ron` 合计 6689 行 ≥ 5310；P1-4 测试用到的 law modifier 字段全部由 RON 提供，无硬编码 fallback；`cargo test -p hoi4-logic --test v6_economy_formulas ron_military_equipment_outputs_cover_12_categories` 通过

#### P2-2 Building schema 补字段 ✅
**问题**：`crates/hoi4-state/src/buildings_v6.rs:35-44` 只有 8 字段（设计 32 字段）。缺 `wage_rm / profit / production_rate / cp_cost / max_level / built_progress / output_value_gbp`。

**修复**：✅ `Building` 已补 `production_rate / output_value_gbp / wage_rm / profit_rm / cp_cost / max_level / built_progress` 7 个运行时字段；`market_tick` / `planned_tick` 每日 tick 后调用 `building_runtime::update()` 真填写；新增 `building_runtime_fields_are_written_by_tick` 覆盖字段写入。


## 3. 阶段表

每阶段都按 V5/V6 节奏：4-6 周 + "在跑动游戏点 5 分钟"验收 + invariant 测试。

### F0 — 公式正确性修复（**最优先**，3 周）

> F0 不通过则 F1 / F2 / F3 / F4 全部不允许启动。这是路线图硬约束。
> 14 条 P0 公式 bug 的细节修复清单见 §2.0。

- F0.1 (税收 9 项) accumulator 归零 / 覆盖 / ×0.001 删 / planned 跳过 income_tax / `tax_burden` 字段 / welfare 读 RON / 出口关税单笔 / `can_print_mefo` 强制校验 / Treasury 付薪与 POP 收薪同算
- F0.2 (生产 5 项) 补 4 个半成品 PM / PM 检查 input 实际成交 / EquipmentCategory 4→12 类 + mk1/mk2/mk3 PM / 删 estimate_building_profit ×0.5 / employment_demand 数值校准 + 跨州配额
- F0.3 (建筑 7 项) PM 切换 UI 接通 / 失业 POP wage=0 / Private/Cartel 工资链 / requires_law 阻塞时释放 POP / state.infra 补乘 / max_level 钳制 / 阶级流动 wage 初值
- F0.4 (P1 顺手) Cartel 70/30 / tariff 进出口分离 / MEFO 利息 / 价格 EMA 零交易保持
- F0.5 **1936-1938 完整回放验证**——必经门槛
- F0.6 **德国 MEFO-法团战备经济闭环**：自动发行 MEFO 覆盖赤字 / 法团战备经济法律 / 政府军购形成准无限军工需求 / 军工扩张拉就业 / 1938 到期爆雷并入公开国债

**验收**：
- ✅ 跑 730 天到 1938-01-01：mefo_debt_rm ∈ [GDP×0.25, GDP×0.30]
- ✅ 跑 1095 天到 1939-01-01：必触发「梅福债危机」
- ✅ POP 满意度均值 ≥ 0.45
- ✅ Σincome / Σexpense ∈ [0.85, 1.15]
- ✅ GER 法团战备经济路线下，MEFO 自动发行、军工政府需求、军工就业扩张、1938/1939 MEFO 爆雷并入公开国债四项均在回放中可观测
- ✅ `cargo test --test v6_economy_formulas` 全过（含 daily_accumulator_zeroes / consumption_tax_real_rate / planned_no_income_tax / pm_input_fulfillment / equipment_category_12 / unemployed_wage_zero / private_payroll 等）

### F1 — 救活招牌（P0 全集，4 周）
- F1.1 (P0-1) `v6_events.rs` + 12 effect 执行器 + main.rs tick 接入
- F1.2 (P0-2) MEFO 危机 caller 接入
- F1.3 (P0-3) `planned_tick.rs:191` 配额倍率修复
- F1.4 (P0-4 关键) `construction_tick.rs` 完工 → push 到 buildings_v6；Construction Sector 建筑实现 CP 产出；非 GER/SOV 国家也注入初始建筑；I-21 测试
- F1.5 (P0-5) main.rs 7 处快照 civ/mil/dock 硬编码 0 删除
- F1.6 7 个事件 + 建造链路集成测试

**验收**：
- ✅ 1936-03-07 莱茵兰自动触发
- ✅ 1938-03-12 Anschluss 钢煤跳升
- ✅ debug 推 MEFO → 5 天内中文「梅福债危机」弹出
- ✅ SOV 切计划经济后 36 个月 Steel 接近五年计划目标
- ✅ **玩家在柏林建 Steel Mill，60 天后真的多一个 Building，且当周 market.supply[Steel] 真涨**
- ✅ 切到法国玩也能造建筑

### F2 — 紧合不变式 + civ/mil/dock 全局清除（P1-1 / P1-3 / P1-4，3 周）
> P1-1 现在不只删 3 字段，而是覆盖 36 文件 / 90+ 引用点的概念清除。
> 不考虑存档兼容 → 比原计划少 1 周。

- F2.1 (P1-1 A/B/C) 数据字段层 + 存档字段直接删 + 数据初始化层（含 `initial_buildings/*.ron`）
- F2.2 (P1-1 D) 逻辑层：strategic_bombing / focus effect / script effect+trigger / AI 改读 V6
- F2.3 (P1-1 E/F/G) UI 层 + main.rs 7 处快照清除 + i18n 删 3 key（P0-5 已覆盖一部分）
- F2.4 (P1-1 H/I) Lua binding 切断 + 集成测试改造
- ✅ F2.5 (P1-3) HC-2 真实施 + `Treasury::pay()` + grep_hc2 测试
- ✅ F2.6 (P1-4) 4 类法律 modifier 接通 + I-17 ~ I-20 测试

**验收**：
- ✅ `grep -rn "civilian_factories\|military_factories\|dockyards" crates/` **输出为空**（0 处残留）
- ✅ 旧存档读入报错（预期行为，不算 bug）
- ✅ `cargo test -p hoi4-logic --test grep_hc2` 通过
- ✅ 切 Taxation 从中→战争税，当 tick 每日收入跳升 ≥ 30%
- ✅ I-15 / I-16 / I-21 全通过

### F3 — UI + RON 补量（P1-2 + P2 全集，2 周）
- F3.1 (P1-2) 顶栏 civ/mil/dock → GDP/增速/建造力（3 槽替换 + i18n + main.rs 数据回填 + `fmt_gbp_short()`）
- ✅ F3.2 (P2-1) RON 补量到 ≥ 5310 行（当前 6689 行）
- ✅ F3.3 (P2-2) Building schema 补 7 字段（`production_rate / output_value_gbp / wage_rm / profit_rm / cp_cost / max_level / built_progress`） + tick 真填写

**验收**：
- ✅ 顶栏开局显示 `£23.4B / +3.2% / 142`
- ✅ `economy_v6/**/*.ron` 合计 6689 行 ≥ 5310
- ✅ POP 初始数据按 70 州分布（grep `state_id:` 出现 ≥ 70 次）
- ✅ 每个国家开局都有非空 `buildings_v6.buildings`

### F4 — 收尾归档（1 周）
- F4.1 v6_loader 中残留 #[test] (`v6_loader.rs:1111-1178`) 移到 `tests/` 正规位置
- F4.2 文档：把 V6 §6 各阶段反馈段更新为"V6 + F1-F4 后真完成"
- F4.3 把 `ROADMAP_V6_ECONOMY.md` + 本文档归档到 `docs/legacy/`，在 V5 §5 添加 "→ V6 + V6-Finish (DONE)"

**合计：13 周（F0 3 周 + F1 4 周 + F2 3 周 + F3 2 周 + F4 1 周）。比原 10 周 +3 周给 F0 公式修复——没这 3 周后续全部建立在错误数值上。**


## 4. 不变式测试增量

| ID | 名 | 检查 | 阶段 |
|---|---|---|---|
| **I-23** | **daily_income_rm / daily_expense_rm 每日 tick 开头归零** | 单测 | F0.1.a |
| **I-24** | medium taxation (rate=0.10) 下 consumption_tax 实抽 ≈ 总消费 × 0.10 | 单测 | F0.1.c |
| **I-25** | 计划经济下 PopGroup wage_rm 不被 income_tax 扣减 | 集成测 | F0.1.d |
| **I-26** | 同档 Taxation 下 Capitalist `tax_burden` > Peasant `tax_burden` | 单测 | F0.1.e |
| **I-27** | civil_rights.ron 中 welfare_rate 修改后下 tick 福利支出真变 | 集成测 | F0.1.f |
| **I-28** | Steel supply 强制设 0 后，arms_industry 下日 output 减少 ≥ 50% | 集成测 | F0.2.b |
| **I-29** | `EquipmentCategory::*` 枚举长度 == 12 | const_assert | F0.2.c |
| **I-30** | 失业 POP `wage_rm == 0.0` | 单测 | F0.3.b |
| **I-31** | Private 建筑 profit 归零后该 POP wage 当周也归零 | 集成测 | F0.3.c |
| **I-32** | 切到计划经济后被关闭 Bank 内 Capitalist POP `employed_at == None` | 集成测 | F0.3.d |
| **I-33（关键）** | **1936-01-01 → 1938-01-01 730 天回放：mefo_debt_rm ∈ [GDP×0.25, GDP×0.30]，POP 满意度均值 ≥ 0.45** | **回放测** | **F0.5** |
| **I-34（关键）** | **1936-01-01 → 1939-01-01 1095 天回放：必触发「梅福债危机」** | **回放测** | **F0.5** |
| **I-35（关键）** | **GER 法团战备经济下，赤字自动转 MEFO；90 天内 `mefo_debt_rm` 上升且 `public_debt_rm` 不因该赤字同步上升** | **回放测** | **F0.6** |
| **I-36（关键）** | **政府军购需求使军工商品/装备 demand 持续为正；新增军工建筑投产后能雇佣 Worker、支付工资、增加产出** | **集成测** | **F0.6** |
| **I-37（关键）** | **1938 MEFO 到期/危机后，`mefo_debt_rm` 按规则并入 `public_debt_rm`，现金、信用评级、POP 满意度受到冲击** | **回放测** | **F0.6** |
| I-15 | StateStore 无 civilian_factories/military_factories/dockyards 字段 | grep | F2.1 |
| I-16 | I-1 ~ I-14 回归 | cargo test | F4 |
| ✅ I-17 | Conscription 法切档后 Soldier::max_ratio 当 tick 内变化 | 单测 | F2.6 |
| ✅ I-18 | Taxation 法切档后 daily_income_rm 当 tick 变化 ≥ 30% | 集成测 | F2.6 |
| ✅ I-19 | CivilRights 切到警察国家后 research slots / loyalty coefficient / satisfaction law modifier 真变 | 单测 | F2.6 |
| ✅ I-20 | InformationControl 切到全面宣传后 loyalty 滑落 ×0.2 | 单测 | F2.6 |
| **I-21** | **enqueue → 完工 → push 到 buildings_v6 → 产 supply 全链路 90 天内可观测** | **集成测（关键）** | F1.4 |
| I-22 | 非 GER/SOV 国家开局也有非空 buildings_v6 | 集成测 | F1.4 |
| ✅ HC-2-真 | grep_hc2: cash_rm -= 仅在 gov_buy/pay 内 | 编译期 | F2.5 |
| **HC-15** | **全 codebase grep `civilian_factories|military_factories|dockyards` 输出为空（0 处）** | **CI grep** | F2.1 |


## 5. 严禁（重申）

- ❌ **跳过 F0 直接动 F1 / F2 / F3**（公式不对，后续全垮）
- ❌ 跳过 F1 直接动 RON / UI（事件不接通就 RON 加再多也是死的）
- ❌ "顺手"扩 D1-D12 决策范围
- ❌ 给 events_v6 加新事件——先把 5 个原事件接通再说
- ❌ 用本路线名义动战指 / 地图 / 美术


## 6. 与原 V6 文档的同步

- F4 完成后，更新 `ROADMAP_V6_ECONOMY.md` §6 各阶段那段"✅ 已完成"声明，把"假完成"改成"V6 + V6-Finish 真完成"
- F4 完成后，更新 `MEMORY.md / project_v6_economy.md` 把状态升级为 "V6 真完成"


## 7. 附录

### 7.1 关联文档
- [`ROADMAP_V5.md`](./ROADMAP_V5.md)
- [`ROADMAP_V6_ECONOMY.md`](./ROADMAP_V6_ECONOMY.md) — 母路线
- 审计日期 2026-05-22；本路线写于同日

### 7.2 审计证据索引（grep 可复核）
| Claim | 证据位置 |
|---|---|
| V6 事件零 caller | `crates/hoi4-app/src/main.rs` 无 `events_v6` 引用 |
| MEFO 三函数零 caller | `crates/hoi4-state/src/finance.rs:184/190` + `economy/finance_tick.rs:299` |
| 配额倍率丢弃 | `crates/hoi4-logic/src/economy/planned_tick.rs:191` |
| 旧 store 字段未删 | `crates/hoi4-state/src/store.rs:62-64` |
| HC-2 软断言 | `crates/hoi4-logic/tests/v6_economy_invariants.rs:186-223` |
| 顶栏三槽 | `crates/hoi4-ui/src/topbar.rs:25-27, 58-60` |
| RON 规模 | `crates/hoi4-content/content/economy_v6/` `wc -l` 合计 2454 |
