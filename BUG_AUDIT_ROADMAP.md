# Bug 自检修复路线图

生成时间：2026-05-23

范围：本次自检覆盖 UI 面板、状态系统、经济/贸易、脚本系统，以及构建/测试健康度。

## 项目方确认

- 原审计第 2 项：TopBar 资源点击行为是故意设计，不作为 bug 修复。
- 原审计第 3 项：外交面板列表里直接宣战行为是故意设计，不作为 bug 修复。

## 当前验证状态

- `cargo check --workspace`：通过，但有较多 warning。
- `cargo test --workspace`：失败，失败点在 `hoi4-app` 的测试目标编译阶段。

## P0：先恢复测试能力

### 1. `cargo test --workspace` 在 `hoi4-app` 测试构建中失败

- 严重程度：高
- 位置：`crates/hoi4-app/src/systems.rs:487-493`
- 现象：测试专用的 `EconomyState` 初始化缺少新字段 `government_orders` 和 `training_queues`。
- 影响：整个 workspace 测试无法跑完，后续修复没有可靠回归验证基础。
- 建议修复：更新 `fake_companions()` 测试夹具，把 `EconomyState` 的所有现有字段补齐。
- 建议验证：重新运行 `cargo test --workspace`，确认测试目标能编译。

## P1：经济、贸易与世界状态一致性

### 2. 每日贸易余额没有每日清零

- 严重程度：高
- 位置：`crates/hoi4-state/src/finance.rs:159-164`
- 相关使用：`crates/hoi4-logic/src/economy/finance_tick.rs:378-379`
- 现象：`Treasury::reset_daily_accumulators()` 清理了 RM 收入、支出和预算明细，但没有清理 `daily_trade_balance_gbp`。
- 影响：每日净出口会跨天累计，财政面板和 GDP 计算可能越跑越偏。
- 建议修复：在 `reset_daily_accumulators()` 中把 `daily_trade_balance_gbp` 重置为 `0.0`。
- 建议测试：构造多日贸易场景，确认第 N 天贸易余额不会泄漏到第 N+1 天。

### 3. 封锁解除后贸易路线可能永久保持阻断

- 严重程度：高
- 位置：`crates/hoi4-logic/src/trade/mod.rs:168-174`, `crates/hoi4-logic/src/trade/mod.rs:193-209`
- 现象：`step_blockade_check()` 在国家不处于战争时直接 `return`，不会清理旧封锁状态。路线从封锁变为未封锁时，只设置 `route.is_blockaded = false`，但没有恢复曾被置为 `0.0` 的 `route.throughput`。
- 影响：战争结束或海上封锁解除后，贸易路线仍可能长期显示为阻断，市场/贸易面板数据错误。
- 建议修复：不在战争时也应清理该国相关封锁状态；路线解除封锁时恢复吞吐量。
- 建议测试：创建海运路线，使其被封锁，然后结束战争或移除封锁条件，断言路线未封锁且吞吐量大于 0。

### 4. `transfer_state` 没有同步省份 owner/controller

- 严重程度：高
- 位置：`crates/hoi4-script/src/effects.rs:436-448`
- 相关渲染读取：`crates/hoi4-render/src/map_mode.rs:101-112`, `crates/hoi4-render/src/map_mode.rs:274-280`
- 现象：脚本效果 `transfer_state` 只更新 `world.states.owners/controllers`，没有更新该 state 下属省份的 owner/controller。
- 影响：政治地图、占领图层、省份 tooltip、前线逻辑可能和 state 所属权显示不一致。
- 建议修复：转移 state 时遍历该 state 下属省份，同步更新 province owner/controller。
- 建议测试：脚本转移一个 state 后，断言 state 和所有下属 province 的 owner/controller 一致。

## P2：脚本与经济逻辑一致性

### 5. `set_politics` 改执政党后不会刷新国家领袖

- 严重程度：中
- 位置：`crates/hoi4-script/src/effects.rs:368-374`
- 现象：原本应该执行的 `input.world.refresh_country_leader(c);` 被写在注释同一行里，实际没有执行。
- 影响：事件/国策改变 ruling party 后，政治面板可能显示新党派但旧领袖。
- 建议修复：把 `refresh_country_leader(c)` 移到可执行代码行，在执政党变更后调用。
- 建议测试：执行 `set_politics` 切换执政党后，断言国家领袖按规则刷新。

### 6. 自动贸易匹配忽略“只有需求、没有供给 key”的商品

- 严重程度：中
- 位置：`crates/hoi4-logic/src/trade/mod.rs:54-75`
- 现象：贸易匹配的 `good_keys` 只来自 `market.supply.keys()`。
- 影响：如果某商品只有需求但 supply map 中没有对应 key，就不会触发进口流。
- 建议修复：用 supply key 和 demand key 的并集作为商品遍历集合。
- 建议测试：构造只有 demand、没有 supply key 的商品，确认在外汇和路线允许时会生成进口。

### 7. 人口消费需求在 demand key 不存在时被静默丢弃

- 严重程度：中
- 位置：`crates/hoi4-logic/src/economy/market_tick.rs:591-669`
- 现象：人口消费需求使用 `market.demand.get_mut(good_id)`，如果 key 不存在就直接跳过。
- 影响：空市场或新商品不会被人口消费创建需求，进而影响价格、进口和满意度。
- 建议修复：使用 `market.demand.entry(good_id.to_owned()).or_insert(0.0)` 后再增加需求。
- 建议测试：从空 demand map 和有就业人口的场景开始，tick 后确认对应消费品需求 key 被创建。

## P3：UI 面板正确性

### 8. 研究面板允许点击未满足前置条件的科技

- 严重程度：中
- 位置：`crates/hoi4-ui/src/research.rs:187-193`
- 相关显示：`crates/hoi4-ui/src/research.rs:205-217`
- 现象：开始研究按钮只判断未完成、未研究中、有空闲槽，没有判断前置科技是否满足。
- 影响：UI 会对锁定科技发出 `StartResearch`。如果后端拒绝，玩家会看到按钮可点但无效果；如果某路径后端漏检，则会绕过科技树。
- 建议修复：在 `TechRow` 或 app binding 中加入“可研究/前置已满足”字段，对锁定科技禁用或隐藏按钮。
- 建议测试：传入前置未满足的科技数据，断言无法发出 `StartResearch` 命令。

### 9. 结束界面世界紧张度显示放大 100 倍

- 严重程度：中
- 位置：`crates/hoi4-ui/src/end_screen.rs:56-57`, `crates/hoi4-ui/src/end_screen.rs:197-199`
- 现象：结束界面把 `world_tension` 注释为 `0..=1`，显示时乘以 100；但状态系统和外交面板使用的是 `0..=100`。
- 影响：实际 50 的世界紧张度会显示成 `5000%`。
- 建议修复：结束界面与状态系统统一，直接把 `world_tension` 当作百分数显示。
- 建议测试：`world_tension = 50.0` 时应显示 `50%`。

### 10. 多占位符翻译替换会把所有占位符替换成第一个值

- 严重程度：中
- 位置：`crates/hoi4-ui/src/production.rs:90-94`, `crates/hoi4-ui/src/diplomacy.rs:115-117`, `crates/hoi4-ui/src/diplomacy.rs:150-158`, `crates/hoi4-ui/src/province_menu.rs:49-52`
- 现象：代码先调用 `.replace("{}", first)`，这会一次替换所有 `{}`，后续 `.replacen(...)` 已经找不到占位符。
- 影响：面板文案可能出现重复值，例如民用工厂、军用工厂、消费品都显示成同一个数字。
- 建议修复：每个值都用 `replacen("{}", value, 1)` 逐个替换，或新增一个小型格式化 helper。
- 建议测试：含 3 个占位符的翻译字符串应能渲染 3 个不同值。

### 11. 存档浏览器行点击区域可能重叠，导致选错存档

- 严重程度：中
- 位置：`crates/hoi4-ui/src/save_browser.rs:220-231`
- 现象：行本身用了 `ui.selectable_label(...)`，但选择逻辑又单独用 `ui.interact(ui.min_rect(), ...)`。`ui.min_rect()` 是父 UI 的累计区域，不是当前行区域。
- 影响：行点击区域可能变大或互相覆盖，点击一个存档时选中另一个存档。
- 建议修复：直接使用 `selectable_label` 返回的 `Response` 做点击判断，或只对该 response 的 rect 做交互。
- 建议测试：人工 UI 检查或 egui 测试夹具，确认每一行只会选中自身。

### 12. 省份/州资源面板可能把资源和产出错配

- 严重程度：低到中
- 位置：`crates/hoi4-ui/src/province_info.rs:199-220`
- 现象：`resources` 和 `resources_output` 直接用 `zip` 绑定，隐含假设两者长度和顺序完全一致。
- 影响：如果输出列表更短、为空或排序不同，资源会漏显示或对应到错误产出。
- 建议修复：按资源 id/name 绑定产出，或传入单一结构化 row 数据。
- 建议测试：传入输出列表更短或顺序不同的数据，确认显示仍然正确。

## P4：Warning 清理

### 13. Warning 数量过多，容易掩盖真实问题

- 严重程度：低到中
- 位置：多个 crate
- 典型例子：`crates/hoi4-ui/src/topbar.rs:50`, `crates/hoi4-logic/src/balance.rs:51`
- 现象：存在大量 unused imports、unused variables、dead code 等 warning；还有错误 lint 名 `unused_import`，应为 `unused_imports`。
- 影响：新增 warning 很容易被噪音淹没，降低维护和回归效率。
- 建议修复：先清理明显无用代码和错误 lint 名；核心 bug 修完后再考虑 CI warning 预算。

## 明确排除的项目

### A. TopBar 资源点击行为

- 原审计项：TopBar 资源未渲染/不可点击，`resource_clicked` 永远为 false。
- 项目方确认：这是故意设计，不是 bug。
- 处理结论：除非需求变更，否则不要修。

### B. 外交列表直接宣战行为

- 原审计项：外交国家列表允许不检查战争目标直接宣战。
- 项目方确认：这是故意设计，不是 bug。
- 处理结论：除非需求变更，否则不要修。
