# `main.rs` 拆分审计

本文档记录 `crates/hoi4-app/src/main.rs` 在 Phase 0 时的职责分布、直接依赖、迁移优先级和回归门槛。目标是给后续拆分提供稳定事实清单，避免边迁移边补洞。

## 审计结论

- `main.rs` 当前仍然是 app 生命周期、世界初始化、事件/决议装载、局势定义、局势 effect 应用、输入、UI 命令、渲染收集和调试命令的总汇入口。
- 文件内存在大量与 `World` 直接耦合的读写代码，尤其是外交、局势、生产、陆军选择、面板 DTO、渲染实例收集。
- Phase 0 的目标不是立刻重构这些逻辑，而是把职责边界和风险点固定下来，确保后续拆分不误伤现有功能。

## 职责区块清单

### 1. 启动与生命周期

- 入口与生命周期：`impl ApplicationHandler for App`，包含 `resumed`、`about_to_wait`、`window_event`。
- 主入口：`fn main()`。
- CLI：`fn parse_cli()`、`fn print_usage()`。
- World / runtime 持有者：`struct App`。
- 关键依赖：`state`、`world`、`camera`、`path_cfg`、`music_player`、`ui`、`window`、`schedule`。
- 风险点：生命周期逻辑与游戏规则混在同一文件，初始化阶段还直接加载内容 RON。

### 2. 初始化与内容装载

- 核心构造：`fn new(mut world: World, path_cfg: PathConfig) -> Self`。
- 直接装载内容：`GER_focus_tree.ron`、`GER_events.ron`、`ITA_events.ron`、`news_events.ron`、`GER_decisions.ron`。
- 直接触发校验：事件库 `validate()`、决议库 `validate()`。
- 依赖字段：`focus_tree`、`event_scheduler`、`decision_db`、`situation_state`、`global_flags`、`v6_db`、`historical_1936`、`loc_catalog`、`settings`。
- 风险点：内容库是写死路径，新增国家内容仍需改 `main.rs`。

### 3. 局势定义与局势效果

- 当前硬编码局势：`spanish_civil_war`、`spanish_anarchist_uprising`、`italo_ethiopian_war`。
- 主要入口：`situation_state: { ... ss.add_def(...) ... }`。
- 局势效果类型：`SituationEffect::SplitCountry`、`SpawnFrontlineDivisions`、`SplitDivisions`、`SendEquipment`、`SendXpBuff`、`TriggerEvent`、`AnnexCountry`、`CreateWar`、`AddIdea`、`AddStability`、`AddWarSupport`、`AddOpinion`、`SetCountryFlag`、`LendDivisions`。
- 依赖字段：`world.countries.at_war`、`world.diplomacy.annexed_countries`、`world.diplomacy.wars`、`world.diplomacy.factions`、`world.provinces`、`world.divisions`、`world.air_wings`、`world.fleets`。
- 风险点：局势定义和执行都在 `main.rs`，拆分时必须保持效果语义一致。

### 4. 外交与战争 UI / 命令

- 相关辅助函数：`country_wargoal_status`、`country_wargoal_details`、`country_relation_factors`、`diplomacy_action_view`、`diplomacy_unavailable_reason_text`、`intervention_expected_impact`、`diplomacy_autonomy_summary`。
- 相关面板数据：`build_country_info_data`。
- 直接依赖：`world.diplomacy.at_war_with`、`faction_of`、`faction`、`pending_wargoals`、`world_tension`、`countries.at_war`、`opinions`、`autonomy`。
- 风险点：UI 入口已经在读大量外交内部状态，后续一旦迁移 action API，需要同步更新 DTO 和失败理由。

### 5. 生产、建设和 V6/V7 后勤辅助

- 相关函数：`v6_industry_counts`、`v6_construction_points`、`auto_enqueue_player_construction`、`v6_industrial_levels`、`v6_estimated_gdp_gbp`、`v6_building_lock_reason`、`v6_building_state_limit_reason`、`v6_pm_prediction`、`v6_flow_summary`、`v6_employment_gap_for_building`。
- 直接依赖：`world.countries.buildings_v6`、`world.countries.market`、`world.countries.treasury`、`world.states`、`world.data.division_templates`、`world.countries.law_store`、`world.countries.completed_techs`。
- 风险点：这部分既是 UI 数据层，也是经济逻辑读模型，后续应尽量向纯 helper 或逻辑 crate 下沉。

### 6. 地图、渲染和可视化收集

- 相关函数：`visual_path_from_provinces`、`filter_visual_path_points`、`smooth_visual_path_points`、`resample_visual_path_heights`、`push_visual_path_instances`、`push_battleplan_arrow_instances`、`push_move_arrow_instances`、`find_land_visual_path`、`collect_player_frontline_boundary_segments`。
- 渲染初始化：`init_render`、`setup_tree_mesh_pipeline`、`load_tree_atlas`、`load_terrain_atlas`、`load_colormap`、`load_rivers_texture`、`make_depth_view`、`upload_lut`。
- 依赖字段：`world.map`、`world.states`、`world.provinces`、`world.diplomacy`、`world.player_armies`、`RenderState` 大量 GPU 资源。
- 风险点：这些函数与内容逻辑耦合度相对低，但体量大，适合作为后续瘦身阶段的稳定迁出目标。

### 7. 输入、UI 状态和 tick 调度

- 相关函数：`update`、`pick_province_at_cursor`、`pick_counter_province_at_cursor`、`select_counter_stack_at_province`、`try_pick_province`、`handle_panel_click`、`toggle_in_game_panel`、`ui_blocks_map_clicks`、`reset_menu_state`、`handle_menu_mouse_click`、`update_hoi3_counter_pass`、`update_frontline_arrows`、`render`。
- 依赖字段：`keys_held`、`selected_province_id`、`open_panel`、`construction_mode`、`selected_divisions`、`selected_province_ids`、`expanded_stacks`、`frontline_painter`、`last_main_buttons`、`last_country_layout`。
- 风险点：这些函数横跨 UI、输入、渲染和战斗计划，迁出时应按“低风险 orchestration -> 复杂规则”的顺序进行。

### 8. 调试、测试和辅助入口

- 相关函数：`main` 之外的 `run_headless`、`v6` 相关测试模块。
- 依赖字段：`world.speed`、战役回放状态、日志输出、headless 驱动。
- 风险点：Phase 0 只需要固定基线，不要求迁走这些测试辅助代码。

## 迁移顺序建议

1. 先迁出纯初始化和内容装载：事件库、决议库、局势硬编码、路径配置和内容校验。
2. 再迁出渲染收集和输入辅助：这类逻辑依赖明确，最容易切成独立模块。
3. 接着迁出 UI DTO 构建：例如外交、建设、模板编辑和省份信息。
4. 最后再处理局势 effect 应用和战争/外交命令，这部分改动风险最高。

## 主要风险点

- `main.rs` 中的 `SituationDef` 与 `SituationEffect` 当前是硬编码数据 + 执行混合体，拆分时最容易发生语义漂移。
- `include_str!` 仍然直接绑定具体内容文件，新增国家内容会持续扩大入口文件职责。
- `World` 直接读写非常广，拆分时必须避免引入隐式全局状态。
- 外交 UI 已经依赖内部字段和索引语义，未来迁出 action API 时要同步调整 DTO 与错误提示。
- 生产和经济辅助函数虽然看似只是 UI helper，但实际上读取了大量状态，拆分时要区分“展示层 helper”和“逻辑入口”。

## Phase 0 回归门槛

- 固定命令 1：`cargo test`。
- 固定命令 2：如果全量测试过慢或失败，至少跑 `cargo test -p hoi4-content`、`cargo test -p hoi4-logic`、`cargo test -p hoi4-integration`。
- 固定命令 3：记录本次基线命令的输出，后续 Phase 1 起把新增失败与旧失败区分开。
- 固定命令 4：后续每次迁出 `main.rs` 的一个大块，至少补一个相关编译或测试命令，不再只靠手工运行。

## Phase 0 测试基线

执行日期：2026-05-24。

- `cargo test`：失败。当前全量测试在 `hoi4-ai` 编译阶段失败，原因是测试/库测试中手写 `World { ... }` 初始化缺少新增字段 `generals` 和 `next_general_id`。
- `cargo test -p hoi4-content`：失败，79 passed / 3 failed。失败项：`event_tick::tests::mtth_zero_fires_immediately`、`event_tick::tests::one_tick_per_day`、`v6_loader::tests::h2_resource_buildings_do_not_exceed_discovered_deposits`。
- `cargo test -p hoi4-logic`：失败，61 passed / 3 failed。失败项：`economy::v6_events::tests::historical_event_triggers_once`、`economy::v6_events::tests::mefo_crisis_event_effect_executes_debt_and_pop_effects`、`economy::v6_events::tests::planned_economy_triggers_nationalization_effects`。
- `cargo test -p hoi4-integration`：失败。已通过的测试包括 crate lib smoke、`defines_lua_smoke`、`leader_portrait`、`oob_phase_1_1` 和 `phase_1_3` 中 4 项；失败项：`phase_1_3::wargoal_justifies_after_full_duration`。

这些失败在 Phase 0 只作为当前基线记录，不在本阶段修复。后续 Phase 的回归判断必须把这些已知失败与新增失败分开。

## 已确认的硬编码点

- `main.rs` 直接 `include_str!` 加载 `GER_events.ron`、`ITA_events.ron`、`news_events.ron`、`GER_decisions.ron`、`GER_focus_tree.ron`。
- `main.rs` 直接构造并注册三个局势：西班牙内战、无政府派起义、意埃战争。
- `main.rs` 里仍有大量直接写入 `world.diplomacy`、`world.countries.at_war`、`world.provinces`、`world.divisions` 的分支。

## 后续工作接口

- Phase 1 应优先把内容装载迁到统一 registry，减少 `main.rs` 对具体 RON 文件的直接依赖。
- Phase 2 再把局势定义迁出 `main.rs`，保留执行入口。
- Phase 4 才做大规模拆文件，避免在内容装载未稳定时扩大改动面。
