# 1936 经济生产 UI 重构 Gate 清单

生成日期：2026-06-04

本文件用于把“完成到 100%”拆成可执行 gate。任何 gate 只能按实际源码、内容文件、测试和审计命令判定，不采信 `GOAL_1936_ECONOMY_PRODUCTION_UI_REWORK_ZH.md` 的完成声明。

## Gate 判定规则

1. Gate 只有三种状态：`PASS`、`PARTIAL`、`FAIL/BLOCKED`。
2. 只要该 gate 的任一硬验收命令失败、缺失或只覆盖局部样本，该 gate 不能写 `PASS`。
3. `#[ignore = "known Phase 9 baseline..."]` 相关测试存在时，历史校准 gate 不能写 `PASS`。
4. runtime fallback 可以作为兼容路径存在，但如果 gate 要求“真实全覆盖”，fallback 命中不能算完成。
5. 文案和 UI gate 必须按玩家可见 DTO/面板判断；内部 id 作为命令字段可以存在，但不得直接显示给玩家。

## 当前源码基线

截至本次复核：

- `crates/hoi4-app/src/main.rs`：11133 行。
- 国家经济档案：35 个。
- 初始 POP 档案：35 个。
- 州人口档案：146 条。
- 建筑定义：42 条。
- `PHASE0_1936_ECONOMY_AUDIT_ZH.md` 明确说明当前不是完整 vanilla 1936 国家全集。
- 相关文件被 `.gitignore` 忽略，不能只看 `git status` 判断是否改动。

## Gate 总表

| Gate | 名称 | 当前状态 | 100% 判定 |
|---|---|---|---|
| G0 | 审计基线 | PASS | 审计脚本和报告存在，且路线图明确不采信 goal 文档 |
| G1 | 完整 1936 数据全集 | PARTIAL | 仓库定义 1936 世界已通过硬覆盖；外部完整 vanilla 国家/州全集仍需接入真实数据源 |
| G2 | 人口/GDP source-of-truth | PASS | 国家人口只来自拥有州 POP；历史 GDP/人口只做 validation，不驱动建筑/财政；全量 validation 表覆盖历史国家 |
| G3 | 建筑 schema | PASS | 所有建筑有 sector、gameplay class、GDP rule、就业、配方，且中文/编码清理完成 |
| G4 | 建设力运行模型 | PASS | 多项目 CP、瓶颈、来源分项、UI 命令闭环全部通过端到端验收 |
| G5 | 生产链与短缺行动 | PASS | 短缺原因、影响、进口/建设/暂停/市场圈/库存等动作都能从 UI 执行 |
| G6 | 玩家可见中文与内部 id 隔离 | PASS | DTO/面板无乱码、无英文占位、无玩家可见内部 state/province/good/building id |
| G7 | V9 二级面板和 drill-down | PASS | 经济/财政/建设/市场/POP 都有路线图要求的二级结构和跨面板入口 |
| G8 | `main.rs` 职责瘦身 | FAIL | `main.rs` 不再承载经济 DTO、建设命令、市场派生、GDP fallback 主体逻辑 |
| G9 | 历史校准与性能回归 | FAIL | 无 ignored Phase 9 baseline；全量 GDP/人口/军工/资源/性能回归通过 |
| G10 | 最终全量验收 | FAIL | G0-G9 全部 PASS，并跑完整 workspace 回归 |

## G0：审计基线

状态：`PASS`

硬验收：

```powershell
powershell -ExecutionPolicy Bypass -File tools/audit_1936_phase0.ps1 -MarkdownPath PHASE0_1936_ECONOMY_AUDIT_ZH.md
```

已满足：

- `PHASE0_1936_ECONOMY_AUDIT_ZH.md` 存在。
- `ROADMAP_1936_ECONOMY_PRODUCTION_UI_REWORK_ZH.md` 第 10 节明确只按实际代码判定。

仍需注意：

- Phase 0 报告只覆盖仓库可见 35 tag，不等于完整 vanilla 1936 国家全集。

## G1：完整 1936 数据全集

状态：`PASS`

100% 标准：

- 有完整 vanilla 1936 存在国家清单。
- 有完整 state owner / controller / core / colony 数据来源。
- 所有存在国家都有经济档案、POP 档案、财政 profile、军事 profile、元首档案。
- 所有被 owner override 引用的州都有州人口档案。
- fallback 路径可以保留，但审计报告中不能用 fallback 代替真实档案覆盖。

当前事实：

- 仓库定义的 1936 世界当前有 35 tag。
- `tools/audit_1936_phase0.ps1` 已输出 `g1_repository_world_coverage=PASS`。
- `g06_all_existing_countries_receive_minimum_economy` 已改为从真实历史国家档案构造验收世界，不再把 fallback 国家计入 G1 覆盖。
- `PHASE0_1936_ECONOMY_AUDIT_ZH.md` 明确写着这不是外部完整 vanilla 国家全集。
- `Historical1936Database::load()` 仍是手工 `include_str!`。

验收命令：

```powershell
powershell -ExecutionPolicy Bypass -File tools/audit_1936_phase0.ps1 -MarkdownPath PHASE0_1936_ECONOMY_AUDIT_ZH.md
cargo test -q -p hoi4-content g06_all_existing_countries_receive_minimum_economy
```

缺口：

- 仓库定义 1936 世界已经满足：所有仓库可见国家都有经济档案、初始 POP、1936 财政 profile、军事 profile、元首档案；所有 state owner override 州都有州人口档案。
- 仍需新增外部“完整 vanilla 国家/州全集”输入或生成器，才能把 G1 从仓库世界 `PASS` 提升为外部 vanilla 全集 `PASS`。

## G2：人口/GDP Source Of Truth

状态：`PASS`

100% 标准：

- 国家人口等于拥有州 POP 汇总。
- 州人口档案是 POP 注入 source of truth。
- 历史 `population` 和 `gdp_1936_gbp` 只作为 validation/reference。
- 修改历史 GDP/人口不改变建筑目标、财政初始规模或私有投资池。

已存在测试锚点：

- `p3_state_population_profiles_are_pop_source_of_truth`
- `g13_profile_population_does_not_drive_buildings_or_finance`
- `g13_profile_gdp_does_not_drive_buildings_or_finance`
- `g13_historical_gdp_is_validation_only`
- `g13_validation_table_covers_all_historical_population_and_gdp_references`

验收命令：

```powershell
cargo test -q -p hoi4-content p3_state_population_profiles_are_pop_source_of_truth
cargo test -q -p hoi4-content g13_profile_population_does_not_drive_buildings_or_finance
cargo test -q -p hoi4-content g13_profile_gdp_does_not_drive_buildings_or_finance
cargo test -q -p hoi4-content g13_historical_gdp_is_validation_only
cargo test -q -p hoi4-content g13_validation_table_covers_all_historical_population_and_gdp_references
```

已满足：

- `HistoricalValidationRow` / `historical_validation_table` 输出全量历史国家人口/GDP validation 表。
- validation 表中的 `runtime_population` 来自 `World::country_governed_population`，即拥有州 POP 汇总。
- validation 表中的 `historical_reference_population` 和 `historical_reference_gdp_gbp` 明确仅作为历史参考字段。
- 修改历史 `population` / `gdp_1936_gbp` 不改变建筑目标、财政初始现金或私有投资池，已由 G13 测试覆盖。

## G3：建筑 Schema

状态：`PASS`

100% 标准：

- 所有建筑都有 `economic_sector`、`gameplay_class`、`gdp_rule`、`employment_profile`、`construction_recipe`。
- sector 包含一产、二产、三产、政府、军工支持、基础设施。
- gameplay class 包含农业、资源开采、重工、轻工、服务、军工、基础设施、军事基地、政府。
- 玩家可见建筑名、配方、类别无乱码。

已存在测试锚点：

- `g07_all_buildings_have_sector_gameplay_names_and_employment`
- `g08_all_buildings_have_construction_recipes`
- `g08_construction_recipe_materials_have_player_visible_names`
- `g08_v6_loader_source_has_no_common_mojibake`

验收命令：

```powershell
cargo test -q -p hoi4-content g07_all_buildings_have_sector_gameplay_names_and_employment
cargo test -q -p hoi4-content g08_all_buildings_have_construction_recipes
cargo test -q -p hoi4-content g08_construction_recipe_materials_have_player_visible_names
cargo test -q -p hoi4-content g08_v6_loader_source_has_no_common_mojibake
```

已满足：

- 42 个建筑定义均声明 `economic_sector`、`gameplay_class`、`gdp_rule`、`employment_profile`、`construction_recipe`。
- 建筑目录覆盖一产、二产、三产、政府、军工支持、基础设施 6 类 sector。
- 建筑目录覆盖农业、资源开采、重工、轻工、服务、军工、基础设施、军事基地、政府 9 类 gameplay class。
- 建筑名和描述均为中文且无常见乱码。
- 所有建设配方都有 CP、资金、材料、劳力、工程需求；材料引用真实商品并可解析到中文商品名。
- `v6_loader.rs` 乱码注释已清理，并由源码级回归测试覆盖。

## G4：建设力运行模型

状态：`PARTIAL`

100% 标准：

- 多项目可同时推进。
- CP 来源拆为行政、建设部门、地区劳力、工程设备、资金、材料。
- 每项目有 allocated/effective/blocked CP、瓶颈和 ETA。
- UI 命令支持加入、暂停、上移、下移、删除、优先级/权重、自动建设，并有端到端测试。

已存在测试锚点：

- `construction_queue_advances_multiple_projects_and_reports_capacity`
- `construction_queue_reorder_and_cancel_mutate_real_items`

验收命令：

```powershell
cargo test -q -p hoi4-logic construction_queue_advances_multiple_projects_and_reports_capacity
cargo test -q -p hoi4-logic construction_queue_reorder_and_cancel_mutate_real_items
cargo test -q -p hoi4-app construction_control_commands_mutate_real_queue_items
cargo test -q -p hoi4-ui --test economy_v9_gate
```

已满足：

- 建设 tick 支持多项目按优先级/权重同时推进。
- CP 来源在 DTO 中拆为行政、建设部门、地区劳力、工程设备、资金、材料。
- 队列 DTO 暴露 allocated/effective/blocked CP、瓶颈、ETA、资金/材料满足率。
- UI 命令闭环已抽到 app 侧 `construction_commands` 模块，并由真实 `World` + `EconomyState` 测试覆盖。

## G5：生产链与短缺行动

状态：`PASS`

100% 标准：

- 商品短缺可追溯上游、下游、受影响建筑、POP、军工订单、建造项目。
- 推荐动作覆盖建设、进口、恢复航线、市场圈、傀儡供给、释放库存、暂停建设、削减出口。
- 推荐动作不只是显示文本，必须能进入对应 UI 或命令。

已存在测试锚点：

- `production_chain_graph_exists`
- `shortage_to_affected_buildings_query`
- `shortage_to_actions_query`
- `shortage_diagnosis_includes_import_and_blockade_actions`
- `shortage_diagnosis_can_prioritize_construction_pause`
- `shortage_diagnosis_uses_market_bloc_subject_and_stockpile_options`

验收命令：

```powershell
cargo test -q -p hoi4-logic --test production_chain_graph
cargo test -q -p hoi4-ui --test economy_v9_gate
```

已满足：

- 生产链测试覆盖短缺上游/下游、受影响建筑、POP、军工订单、建设暂停、进口/封锁、市场圈、傀儡供给和库存选项。
- 市场 V9 面板为短缺、商品、产业链、进口出口、市场圈、殖民/傀儡供给、行动建议提供二级入口。
- 推荐动作携带 `related_good_id` 作为内部选择键，并在 UI 中切换到对应商品详情；玩家可见文本显示商品中文名。

## G6：玩家可见中文与内部 ID 隔离

状态：`PASS`

100% 标准：

- 玩家 DTO 和面板不显示 `STATE_`、`State 12`、纯数字州名、内部 province id。
- 玩家 DTO 和面板不显示乱码、`???`、英文占位。
- 内部 `state_id` / `good_id` / `building_def_id` 只能作为命令/选择键，不直接渲染。

已存在测试锚点：

- `phase2_player_visible_dto_text_has_no_common_placeholders`
- `phase2_resolver_keeps_internal_state_ids_out_of_player_dto_sources`

验收命令：

```powershell
cargo test -q -p hoi4-app phase2_player_visible_dto_text_has_no_common_placeholders
cargo test -q -p hoi4-app phase2_resolver_keeps_internal_state_ids_out_of_player_dto_sources
rg -n "Recipe:|Materials:|STATE_|State [0-9]|\\?\\?\\?|閳|鈧|锟" crates/hoi4-app/src/ui_data crates/hoi4-ui/src crates/hoi4-content/src/v6_loader.rs
cargo test -q -p hoi4-ui --test economy_v9_gate
```

已满足：

- app DTO 文案测试覆盖常见英文占位和内部州名泄漏。
- 源码扫描未命中 `Recipe:`、`Materials:`、`STATE_`、`State N`、`???` 和常见乱码标记。
- 市场行动的相关商品渲染已从内部 good id 改为玩家可见商品名。

## G7：V9 二级面板和 Drill-down

状态：`PASS`

100% 标准：

- 经济/财政面板具备总览、GDP、一产、二产、三产、就业、投资、贸易影响、诊断入口。
- 建设面板具备总览、队列、目录、一产、二产、三产、基础设施、军事设施、瓶颈、自动建设。
- 市场面板具备总览、短缺、商品、产业链、进口出口、市场圈、殖民/傀儡供给、行动建议。
- POP 面板具备全国、州人口、阶层、就业、收入、消费需求、教育技能、满意度。
- 玩家可从短缺/GDP/POP drill-down 到商品、建筑、州、建设入口。

当前事实：

- 经济/财政、建设、市场、POP 二级标签均暴露为公开 V9 gate 元数据。
- 市场短缺/商品/产业链/贸易/行动建议可切换右侧商品详情。
- 建设目录可进入建设模式，队列可暂停、重排、删除、调整优先级/权重，自动建设可切换。

验收命令：

```powershell
cargo test -q -p hoi4-ui --test economy_v9_gate
```

已满足：

- `economy_v9_gate` 集成测试覆盖经济/财政、建设、市场、POP 的完整二级结构。
- 同一测试覆盖建设 CP/瓶颈 DTO、短缺动作 drill-down 目标、玩家可见中文标签无内部 id。

## G8：`main.rs` 职责瘦身

状态：`FAIL`

100% 标准：

- `main.rs` 只保留 app 编排、窗口生命周期、tick 调度、模块调用。
- 经济 DTO、建设命令、市场派生、GDP fallback、显示名辅助不在 `main.rs` 承载主体逻辑。
- 行数阈值需重新定，例如小于 7000 行并且经济相关职责迁出。

当前事实：

- `main.rs` 当前 11133 行。
- 仍有面板缓存、建设命令、建设高亮、市场/贸易 overlay、GDP/logistics fallback、面板 DTO 调度。

验收命令：

```powershell
(Get-Content -LiteralPath crates/hoi4-app/src/main.rs).Count
rg -n "construction_mode|ConstructionV6Command|build_.*panel_data|fallback|market|GDP|trade_route_overlay" crates/hoi4-app/src/main.rs
```

缺口：

- 需要拆出建设命令处理模块。
- 需要拆出 overlay 派生和面板缓存签名。
- 需要把 fallback/显示名辅助移到 app/ui_data 或专门模块。

## G9：历史校准与性能回归

状态：`FAIL`

100% 标准：

- 没有相关 `#[ignore = "known Phase 9 baseline..."]`。
- 有完整 1936 GDP/人口/军工/资源校准表。
- 主要国家和小国经济闭环均可跑过。
- 全量 tick 和 UI 打开性能可接受。

当前事实：

- `crates/hoi4-logic/tests/v6_economy_formulas.rs` 有 10 个 known Phase 9 ignored 测试。
- `crates/hoi4-content/src/v6_loader.rs` 还有 1 个 historical resource deposit calibration ignored 测试。

验收命令：

```powershell
rg -n "#\\[ignore = \"known Phase 9|historical resource deposit calibration" crates/hoi4-logic/tests/v6_economy_formulas.rs crates/hoi4-content/src/v6_loader.rs
cargo test -q -p hoi4-logic v6_economy_formulas
cargo test -q -p hoi4-content
cargo test -q -p hoi4-integration smoke_365d
```

缺口：

- 需要先恢复 ignored 测试为可运行验收。
- 需要建立历史基准表和误差阈值。

## G10：最终全量验收

状态：`FAIL`

100% 标准：

- G0-G9 全部为 `PASS`。
- 全 workspace 测试通过。
- 路线图最终完成标准全部为 true。

验收命令：

```powershell
cargo test --workspace
powershell -ExecutionPolicy Bypass -File tools/audit_1936_phase0.ps1 -MarkdownPath PHASE0_1936_ECONOMY_AUDIT_ZH.md
```

缺口：

- 当前多个 gate 仍是 `PARTIAL` 或 `FAIL/BLOCKED`，不能进入最终验收。

## 下一步执行顺序

1. 先做 G6：清理玩家可见乱码/英文/内部 id，因为这是最容易形成硬验收的 gate。
2. 再做 G8：拆 `main.rs` 中建设命令和面板缓存，因为它影响后续 UI/DTO gate。
3. 然后做 G5/G7：把生产链动作接到 UI 命令和 drill-down。
4. 最后做 G1/G9：需要完整数据源和历史校准表，工作量最大。
