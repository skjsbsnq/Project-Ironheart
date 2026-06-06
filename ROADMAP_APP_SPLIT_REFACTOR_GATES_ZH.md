# HOI4 App 拆分重构路线图

目标：在不取消、不弱化、不改变现有功能的前提下，把 `hoi4-app/src/main.rs` 和相关巨型模块逐步拆分为可维护边界。路线图采用 gate 制：每个 gate 必须独立可编译、可测试、可提交，并且完成后必须推送到 GitHub，方便随时回滚。

最终交付不只要求“代码能跑”，还要求结构清晰、职责边界明确、后续维护者能快速定位功能所属模块。重构完成后必须产出一份技术文档，说明模块结构、数据流、命令流、渲染流、测试策略和回滚方式。

## 总原则

- 不做大爆炸重写。每个 gate 只移动或收敛一个明确职责。
- 优先纯搬迁，后抽象，最后治理接口。
- 不删除现有功能、调试开关、fallback、测试、审计入口和命令行模式。
- 保持现有 UI、快捷键、地图渲染、模拟推进、存档、事件、通知、debug overlay 行为。
- 每个新模块必须有明确职责，避免生成新的“杂物模块”。
- 模块命名必须能反映业务职责，不能用 `misc`、`utils2`、`new_main` 这类名称。
- 每个 gate 完成后都要确认依赖方向没有倒置：底层数据/逻辑不能依赖 app UI，UI 数据构造不能直接持有 GPU 资源。
- 每个 gate 结束必须执行：`git status`、相关测试、`git add`、`git commit`、`git push`。
- 每个 gate 的 commit message 使用格式：`refactor(app): gate N - <short description>`。
- 如果某个 gate 编译失败或行为不确定，立即停止，不进入下一 gate。

## 最终结构要求

重构完成后，`hoi4-app` 推荐结构如下：

```text
crates/hoi4-app/src/
  main.rs
  app_shell.rs
  app_state/
    mod.rs
    runtime.rs
    view.rs
    interaction.rs
    ui.rs
    render_toggles.rs
    perf.rs
    audit.rs
  app_command.rs
  input/
    mod.rs
    keyboard.rs
    mouse.rs
    window.rs
    menu.rs
    debug_hotkeys.rs
  simulation/
    mod.rs
    tick.rs
    content_events.rs
    auto_build.rs
    war_pause.rs
    end_screen.rs
    music.rs
  ui_driver/
    mod.rs
    build.rs
    render.rs
    commands.rs
  render_frame/
    mod.rs
    prepare.rs
    hud.rs
    submit.rs
  map_interaction.rs
  camera_control.rs
  map_refresh.rs
  world_overlays.rs
  combat_overlay.rs
  military_ui_data.rs
  render_assets.rs
  gpu_utils.rs
  gpu_readback.rs
```

维护性验收标准：
- `main.rs` 负责入口、模块声明、`App` 组装，不再承载具体业务流程。
- `app_shell.rs` 只保留 winit 生命周期 glue，不直接写大段业务逻辑。
- `input/*` 只把输入事件转为 app 行为，不构造 UI 面板数据，不提交 GPU 命令。
- `ui_driver/build.rs` 只构造 UI 数据，不执行世界状态变更。
- `ui_driver/render.rs` 只绘制 egui 并收集命令，不直接改 `World`。
- `ui_driver/commands.rs` 统一执行 UI 命令，是 UI 副作用的主入口。
- `render_frame/*` 只负责 GPU frame，不处理外交、研究、建设、存档等业务命令。
- `simulation/*` 只负责模拟推进和模拟推进后的 app 侧副作用。
- `app_state/*` 只分组状态，不藏业务逻辑。
- 任一新文件超过 1500 行时必须重新评估是否继续拆分。
- 任一函数超过 250 行时必须说明原因，优先继续拆分。

## [x] Gate 0 - 基线确认

目标：建立重构前的可回滚基线，不改源码行为。

任务：
- 记录当前 dirty worktree，确认哪些文件是用户已有改动。
- 跑最小基线测试。
- 如果测试本身已有失败，记录失败列表，不在后续 gate 中扩大失败面。

建议命令：

```powershell
git status --short
cargo test -p hoi4-app
cargo test -p hoi4-ui
cargo test -p hoi4-content
cargo test -p hoi4-logic
```

验收：
- 明确知道重构前哪些测试通过、哪些测试已失败。
- 不修改源码。

提交与推送：
- 如果只新增本路线图，可提交并推送。
- 命令：

```powershell
git add ROADMAP_APP_SPLIT_REFACTOR_GATES_ZH.md
git commit -m "docs(refactor): gate 0 - app split roadmap"
git push
```

## [x] Gate 1 - 从 main.rs 搬出纯辅助函数

目标：只做机械搬迁，不改逻辑。

任务：
- 新建 `crates/hoi4-app/src/app_helpers/`。
- 搬迁不依赖 `App` 可变借用的纯函数：
  - `estimate_construction_days_remaining`
  - `postprocess_lut_selection_for`
  - `intervention_expected_impact`
  - `decision_id_matches_player_tag`
  - `map_mode_from_capture_name`
  - `map_mode_terrain_blend_for`
  - `terrain_debug_view_for_baseline_layer`
  - `surrender_notification_sound_key`
  - `event_modal_sound`
- 保留原函数签名语义，必要时 `pub(crate)` 暴露。
- 不改调用顺序。

验收：
- `main.rs` 行数下降。
- 所有原调用点仍使用同名函数或清晰模块路径。
- 无行为变化。

测试：

```powershell
cargo test -p hoi4-app
```

提交与推送：

```powershell
git status --short
git add crates/hoi4-app/src
git commit -m "refactor(app): gate 1 - move pure app helpers"
git push
```

## [x] Gate 2 - 拆 map phase0 与 edge pan test

目标：搬出测试/审计运行状态，降低 `App` 主体噪音。

任务：
- 新建 `crates/hoi4-app/src/map_phase0_run.rs`。
- 搬迁：
  - `MapPhase0Run`
  - `enable_map_phase0`
  - `map_phase0_finished`
  - `current_map_layer_mask`
  - `map_phase0_capture_ready`
  - `map_phase0_capture_path`
  - `map_phase0_debug_lines`
  - `prepare_map_phase0_capture`
  - `map_phase0_after_uncaptured_frame`
  - `map_phase0_after_capture`
  - `finish_map_phase0`
- 新建 `crates/hoi4-app/src/edge_pan_test.rs`。
- 搬迁：
  - `EdgePanTestRun`
  - `start_edge_pan_test`
  - `update_edge_pan_test_cursor`
  - `edge_pan_test_should_finish`
  - `finish_edge_pan_test`
  - `record_edge_pan_test_frame`

验收：
- `app_shell.rs` 中 map phase0 和 edge pan 调用行为不变。
- CLI `--map-phase0`、`--map-phase0-report-only`、`--edge-pan-test` 入口不变。

测试：

```powershell
cargo test -p hoi4-app
```

提交与推送：

```powershell
git status --short
git add crates/hoi4-app/src
git commit -m "refactor(app): gate 2 - isolate audit and edge pan state"
git push
```

## [x] Gate 3 - 拆地图交互与选择逻辑

目标：把地图点击、框选、counter 选择从主文件剥离。

任务：
- 新建 `crates/hoi4-app/src/map_interaction.rs`。
- 搬迁：
  - `SelectionBoxState`
  - `FrontlinePainterState`
  - `PainterMode`
  - `reconstruct_sample_bridge`
  - `append_frontline_painter_sample`
  - `thin_frontline_painter_samples`
  - `short_land_sample_bridge`
  - `is_land_province_raw`
  - `pick_province_at_cursor`
  - `pick_counter_province_at_cursor`
  - `refresh_counter_hit_regions_for_current_frame`
  - `select_counter_stack_at_province`
  - `select_counter_stacks_in_rect`
  - `try_pick_province`
  - `handle_panel_click`
  - `ui_blocks_map_clicks`
- 保持 `window_event` 的事件分支不变，只改方法位置。

验收：
- 左键省份选择、counter 选择、框选、多选、右键移动、前线绘制行为不变。
- `selection_box` 和 `frontline_painter` 状态仍由 `App` 持有，暂不拆字段。

测试：

```powershell
cargo test -p hoi4-app
cargo test -p hoi4-logic frontline
```

提交与推送：

```powershell
git status --short
git add crates/hoi4-app/src
git commit -m "refactor(app): gate 3 - isolate map interaction logic"
git push
```

## [x] Gate 4 - 拆摄像机与地图刷新逻辑

目标：将 camera、LUT、标签刷新、地图显示辅助函数独立。

任务：
- 新建 `crates/hoi4-app/src/camera_control.rs`。
- 搬迁：
  - `world_size`
  - `cursor_ndc_at`
  - `ground_point_at_cursor`
  - `zoom_at_cursor`
  - `update_smooth_zoom`
  - `upload_camera`
- 新建 `crates/hoi4-app/src/map_refresh.rs`。
- 搬迁：
  - `refresh_lut`
  - `lut_entry_with_highlights`
  - `controller_changes_affect_lut`
  - `refresh_lut_entries`
  - `rebuild_country_labels_and_refresh`
  - `refresh_map_if_province_ownership_changed`
  - `upload_lut`
  - `upload_lut_span`

验收：
- 地图模式、国家颜色、占领变化、内战/吞并后标签刷新保持原样。
- 摄像机缩放、平移、窗口 resize 后 aspect 逻辑保持原样。

测试：

```powershell
cargo test -p hoi4-app
cargo test -p hoi4-render
```

提交与推送：

```powershell
git status --short
git add crates/hoi4-app/src
git commit -m "refactor(app): gate 4 - isolate camera and map refresh"
git push
```

## [x] Gate 5 - 拆 counter、前线、贸易路线可视层

目标：把战棋计数器和世界对象 overlay 从 `main.rs` 剥离。

任务：
- 新建 `crates/hoi4-app/src/world_overlays.rs`。
- 搬迁：
  - `VisualDivisionMotion`
  - `CounterVisibilityCache`
  - `update_division_motion`
  - `counter_visibility_signature`
  - `cached_counter_visibility`
  - `visual_division_motion_overrides`
  - `update_hoi3_counter_pass`
  - `prepare_hoi3_counter_instances_for_upload`
  - `refresh_cached_hoi3_counter_screen_positions`
  - `apply_or_rebuild_counter_layout`
  - `hoi3_counter_layout_signature`
  - `hoi3_counter_signature`
  - `frontline_arrow_signature`
  - `collect_order_arrows`
  - `update_frontline_arrows`
  - `update_trade_routes_overlay`
  - `frontline_overlay_signature`
  - `update_frontline_overlay`
  - `high_speed_visual_rebuild_due`
  - `effective_render_quality_preset`

验收：
- 单位显示、聚合、移动动画、前线箭头、贸易路线 overlay 不变。
- `MapRenderer` 计划和 pass order 不变。

测试：

```powershell
cargo test -p hoi4-app
cargo test -p hoi4-app terrain_wgsl
cargo test -p hoi4-render shader_lib_compiles
```

提交与推送：

```powershell
git status --short
git add crates/hoi4-app/src
git commit -m "refactor(app): gate 5 - isolate world overlays"
git push
```

## [x] Gate 6 - 拆战斗气泡与军事 UI 辅助

目标：把 combat bubble 和 template editor 数据构造移出主文件。

任务：
- 新建 `crates/hoi4-app/src/combat_overlay.rs`。
- 搬迁：
  - `CombatSideSnapshot`
  - `CombatBubbleSnapshot`
  - `collect_combat_bubbles`
  - `active_combat_divisions`
  - `province_has_attack_towards`
  - `destination_points_through_target`
  - `combat_side_snapshot`
  - `division_stats_for_ui`
  - `combat_contact_screen_pos`
  - `is_land_province_for_bubble`
  - `stable_combat_bubble_id`
  - `combat_side_power`
  - `combat_advantages`
  - `combat_chance_color`
  - `show_combat_bubble_overlay`
  - `combat_side_detail`
- 新建 `crates/hoi4-app/src/military_ui_data.rs`。
- 搬迁：
  - `first_open_line_slot`
  - `first_open_support_slot`
  - `empty_template_editor_data`
  - `empty_template_subunit_picker_data`
  - `build_template_subunit_picker_data`
  - `build_template_editor_data`
  - `looks_like_support_subunit`
  - `naval_mission_to_ui`
  - `naval_mission_from_ui`
  - `air_mission_to_ui`
  - `air_mission_from_ui`

验收：
- 军事面板、海军面板、空军面板、模板编辑器、战斗气泡显示不变。

测试：

```powershell
cargo test -p hoi4-app
cargo test -p hoi4-ui
cargo test -p hoi4-logic military
```

提交与推送：

```powershell
git status --short
git add crates/hoi4-app/src
git commit -m "refactor(app): gate 6 - isolate combat and military UI helpers"
git push
```

## [x] Gate 7 - 拆资源加载与 GPU 工具

目标：把树、地形 atlas、colormap、river texture 加载从主文件移除。

任务：
- 新建 `crates/hoi4-app/src/render_assets.rs`。
- 搬迁：
  - `setup_tree_mesh_pipeline`
  - `load_tree_atlas`
  - `load_terrain_atlas_phase1`
  - `load_colormap_phase1`
  - `load_rivers_texture_phase1`
  - `load_terrain_atlas`
  - `load_colormap`
  - `load_rivers_texture`
  - `rivers_fallback`
- 新建 `crates/hoi4-app/src/gpu_readback.rs`。
- 搬迁：
  - `PendingPngReadback`
  - `enqueue_png_readback`
  - `finish_png_readback`
- 新建 `crates/hoi4-app/src/gpu_utils.rs`。
- 搬迁：
  - `make_depth_view`
  - `parity_required_limits`

验收：
- 地形、水、河流、树、截图 capture、map phase0 capture 都不变。
- `render_init.rs` 调用路径更新但初始化顺序不变。

测试：

```powershell
cargo test -p hoi4-app terrain_wgsl
cargo test -p hoi4-app river_wgsl
cargo test -p hoi4-render shader_lib_compiles
```

提交与推送：

```powershell
git status --short
git add crates/hoi4-app/src
git commit -m "refactor(app): gate 7 - isolate render assets and gpu utilities"
git push
```

## [x] Gate 8 - 拆 UI 数据构造阶段

目标：把 `render()` 中 4911-6151 的面板数据构造收拢为独立模块，但命令处理暂不改。

任务：
- 新建 `crates/hoi4-app/src/ui_driver/mod.rs`。
- 新建 `crates/hoi4-app/src/ui_driver/build.rs`。
- 定义 `UiBuildOutput`，包含现有所有面板数据、panel open 状态、badge count、combat bubbles、focus context、settings/save/end state references 所需信息。
- 先只把纯数据构造移动出去。
- 不改变 egui 绘制闭包内的控件调用顺序。

验收：
- UI 显示不变。
- 所有面板能打开，内容一致。
- 不提前或延后任何命令执行。

测试：

```powershell
cargo test -p hoi4-app
cargo test -p hoi4-ui
cargo test -p hoi4-ui v9_visual_snapshots
```

提交与推送：

```powershell
git status --short
git add crates/hoi4-app/src
git commit -m "refactor(app): gate 8 - isolate UI data build phase"
git push
```

## [x] Gate 9 - 拆 egui 绘制闭包

目标：把 `s.ui.begin_frame(... |ctx| { ... })` 内部绘制逻辑移到 `ui_driver/render.rs`。

任务：
- 新建 `crates/hoi4-app/src/ui_driver/render.rs`。
- 定义 `UiRenderInput` 和 `UiRenderOutput`。
- `UiRenderOutput` 收集：
  - topbar speed command
  - side rail panel command
  - panel commands
  - 各 panel 专属命令
  - settings/save/end command
  - event/surrender/focus/country info command
  - deferred switch player country
- 保持现有闭包中控件绘制顺序。

验收：
- egui draw order 不变。
- 声音事件 drain 仍在 frame 后处理。
- demo、v9 demo、通知、modal、settings、saves、end screen 不变。

测试：

```powershell
cargo test -p hoi4-app
cargo test -p hoi4-ui
cargo test -p hoi4-ui v9_visual_snapshots
```

提交与推送：

```powershell
git status --short
git add crates/hoi4-app/src
git commit -m "refactor(app): gate 9 - isolate egui render phase"
git push
```

## [x] Gate 10 - 拆 UI 命令执行阶段

目标：把 `render()` 中 6544-7872 的命令处理移到 `ui_driver/commands.rs`。

任务：
- 新建 `crates/hoi4-app/src/ui_driver/commands.rs`。
- 定义 `apply_ui_render_output(app, output)`。
- 分组处理：
  - close commands
  - finance commands
  - construction commands
  - law/politics/decision commands
  - research commands
  - diplomacy commands
  - military/naval/air commands
  - situation/focus/event/surrender commands
  - country info commands
  - settings/save/end commands
- 保留当前处理顺序，尤其是 `panel_commands` 的追加顺序。

验收：
- 所有按钮行为不变。
- 外交行动、宣战、正当化、派系、和平结算不变。
- 存档加载/删除/重命名不变。
- settings fullscreen/resolution/audio/language 不变。

测试：

```powershell
cargo test -p hoi4-app
cargo test -p hoi4-ui
cargo test -p hoi4-logic diplomacy
cargo test -p hoi4-state save_roundtrip
```

提交与推送：

```powershell
git status --short
git add crates/hoi4-app/src
git commit -m "refactor(app): gate 10 - isolate UI command handling"
git push
```

## Gate 11 - 拆 GPU frame 准备阶段（已完成）

目标：把 `render()` 中 7895-8447 的 GPU 参数更新和 pass 参数更新移出。

任务：
- 新建 `crates/hoi4-app/src/render_frame/prepare.rs`。
- 定义：
  - `FramePrepareInput`
  - `FramePrepareOutput`
  - `prepare_render_params`
  - `update_terrain_buckets`
  - `acquire_surface_and_capture_target`
  - `update_global_uniforms`
  - `prepare_map_renderer_frame`
  - `update_vanilla_targets`
  - `update_pass_params`
- 不改变 `RenderParams` 字段赋值顺序。
- 不改变 terrain bucket upload 策略。

验收：
- 地图渲染、water/refraction、border、tree、particle、maparrow、traderoute、strait 参数不变。
- map phase0 capture 不变。

测试：

```powershell
cargo test -p hoi4-app
cargo test -p hoi4-app terrain_wgsl
cargo test -p hoi4-app river_wgsl
cargo test -p hoi4-app counter_v3_wgsl
cargo test -p hoi4-render shader_lib_compiles
```

提交与推送：

```powershell
git status --short
git add crates/hoi4-app/src
git commit -m "refactor(app): gate 11 - isolate render frame preparation"
git push
```

## Gate 12 - 拆 HUD 与提交阶段（已完成）

目标：把 `render()` 中 8449-9101 的 HUD、legacy text/panel/flag pass、egui paint、submit、present 拆出。

任务：
- 新建 `crates/hoi4-app/src/render_frame/hud.rs`。
- 搬迁：
  - debug overlay refresh
  - phase10 overlay lines
  - legacy text HUD
  - menu/country select HUD
  - topbar fallback HUD
  - off-screen indicators
  - selection box
  - flag draw
- 新建 `crates/hoi4-app/src/render_frame/submit.rs`。
- 搬迁：
  - text/panel/flag prepare
  - egui paint
  - queue submit
  - present
  - profiler resolve
  - capture readback
  - edge pan frame record

验收：
- HUD、菜单、国家选择、debug overlay、flag、egui overlay 层级不变。
- frame timing 日志不变或字段同义。

测试：

```powershell
cargo test -p hoi4-app
cargo test -p hoi4-ui v9_visual_snapshots
```

提交与推送：

```powershell
git status --short
git add crates/hoi4-app/src
git commit -m "refactor(app): gate 12 - isolate hud and frame submission"
git push
```

## Gate 13 - 收敛 render() 为流水线（已完成）

目标：让 `App::render()` 只保留阶段调用，单函数控制在 150 行以内。

任务：
- 把 `render()` 改为：
  - visual prepare
  - UI build
  - egui render
  - UI command apply
  - GPU prepare
  - map draw
  - HUD draw
  - submit/present
  - deferred commands
  - perf accumulation
- 保持 deferred command 顺序：
  - `topbar_action`
  - `side_rail_panel_cmd`
  - `panel_commands`
  - `deferred_switch_player_country`
  - `capture_result`

验收：
- `main.rs` 不再有 4000 行 `render()`。
- 行为与 Gate 12 完全一致。

测试：

```powershell
cargo test -p hoi4-app
cargo test -p hoi4-ui
cargo test -p hoi4-content
cargo test -p hoi4-logic
```

提交与推送：

```powershell
git status --short
git add crates/hoi4-app/src
git commit -m "refactor(app): gate 13 - reduce render to pipeline orchestration"
git push
```

## Gate 14 - 拆 window_event 输入处理（已完成）

目标：把 [app_shell.rs](crates/hoi4-app/src/app_shell.rs) 中巨型 `window_event` 拆成输入路由。

任务：
- 新建 `crates/hoi4-app/src/input/`。
- 拆分：
  - `input/mod.rs`
  - `input/keyboard.rs`
  - `input/mouse.rs`
  - `input/window.rs`
  - `input/menu.rs`
  - `input/debug_hotkeys.rs`
- `ApplicationHandler for App` 保留在 `app_shell.rs`。
- `window_event` 只做：
  - egui consume 判断
  - global debug bypass 判断
  - 分发到 input 模块

验收：
- 所有快捷键 F1-F12、R、数字速度、面板热键、菜单 Enter/Esc、鼠标滚轮、左右键、拖拽、resize 行为不变。

测试：

```powershell
cargo test -p hoi4-app
```

提交与推送：

```powershell
git status --short
git add crates/hoi4-app/src
git commit -m "refactor(app): gate 14 - isolate window input handling"
git push
```

## Gate 15 - 拆 update_loop 后处理（已完成）

目标：把模拟推进后的 GUI/app 副作用从 [update_loop.rs](crates/hoi4-app/src/update_loop.rs) 拆开。

任务：
- 新建 `crates/hoi4-app/src/simulation/`。
- 拆分：
  - `simulation/tick.rs`
  - `simulation/content_events.rs`
  - `simulation/auto_build.rs`
  - `simulation/war_pause.rs`
  - `simulation/end_screen.rs`
  - `simulation/music.rs`
- 保留 `App::update(max_sim_budget_secs)` 的公开形态。
- 不改变 daily event 处理顺序。

验收：
- 游戏速度、每日事件、事件弹窗暂停、投降通知、和平通知、自动建设、1940 end screen、音乐自动切换不变。

测试：

```powershell
cargo test -p hoi4-app
cargo test -p hoi4-runtime
cargo test -p hoi4-content
cargo test -p hoi4-logic
```

提交与推送：

```powershell
git status --short
git add crates/hoi4-app/src
git commit -m "refactor(app): gate 15 - isolate simulation aftermath"
git push
```

## Gate 16 - 引入 AppCommand 汇总层

目标：统一 UI 命令的收集和执行入口，减少局部变量爆炸。

任务：
- 新建 `crates/hoi4-app/src/app_command.rs`。
- 定义 `AppCommand`，先包裹现有命令类型，不改 UI crate。
- `ui_driver/render.rs` 输出 `Vec<AppCommand>`。
- `ui_driver/commands.rs` 按原顺序执行 `Vec<AppCommand>`。

验收：
- 命令执行顺序与 Gate 15 等价。
- `render()` 内不再声明十多个 `*_cmds` 局部变量。

测试：

```powershell
cargo test -p hoi4-app
cargo test -p hoi4-ui
```

提交与推送：

```powershell
git status --short
git add crates/hoi4-app/src
git commit -m "refactor(app): gate 16 - introduce app command routing"
git push
```

## Gate 17 - 拆 App 状态字段

目标：在函数边界稳定后，把 `App` 巨型字段分组。

任务：
- 新建 `crates/hoi4-app/src/app_state/`。
- 定义：
  - `RuntimeState`
  - `ViewState`
  - `InteractionState`
  - `UiStateBundle`
  - `RenderToggles`
  - `PerfState`
  - `AuditState`
- 分批迁移字段，不一次性迁移全部。
- 每迁移一组字段都保持 `App` 对外方法名不变。

建议子 gate：
- Gate 17.1：迁移 perf 字段。
- Gate 17.2：迁移 audit/edge pan 字段。
- Gate 17.3：迁移 UI route 字段。
- Gate 17.4：迁移 interaction 字段。
- Gate 17.5：迁移 runtime companion states。

验收：
- `App` 主结构字段明显减少。
- `App::new` 仍完整初始化所有功能。

测试：

```powershell
cargo test -p hoi4-app
cargo test -p hoi4-ui
cargo test -p hoi4-runtime
```

每个子 gate 都要提交与推送：

```powershell
git status --short
git add crates/hoi4-app/src
git commit -m "refactor(app): gate 17.x - group <state group> state"
git push
```

## Gate 18 - 拆 v6_loader

目标：拆分 [v6_loader.rs](crates/hoi4-content/src/v6_loader.rs)，不改变 `V6Database` API。

任务：
- 新建 `crates/hoi4-content/src/v6/`。
- 拆为：
  - `schema.rs`
  - `load.rs`
  - `inject/mod.rs`
  - `inject/laws.rs`
  - `inject/finance.rs`
  - `inject/pops.rs`
  - `inject/buildings.rs`
  - `inject/technology.rs`
  - `events.rs`
  - `validation.rs`
- 保留旧路径 re-export：`hoi4_content::v6_loader::*` 仍可用。
- 不改变 `V6Database::load() -> Self`。
- 不改变现有 include_str 路径语义。

验收：
- 所有调用方无需大规模改动。
- `hoi4-content` 测试不减少。

测试：

```powershell
cargo test -p hoi4-content
cargo test -p hoi4-logic
cargo test -p hoi4-app
```

提交与推送：

```powershell
git status --short
git add crates/hoi4-content/src crates/hoi4-app/src crates/hoi4-logic/src
git commit -m "refactor(content): gate 18 - split v6 loader modules"
git push
```

## Gate 19 - 错误与 fallback 诊断增强

目标：在不改变加载成功路径的情况下，让静默 fallback 可观测。

任务：
- 给 `V6Database::load()` 内部增加诊断收集，但暂不改变返回类型。
- 新增 `V6Database::load_with_report() -> (Self, V6LoadReport)`。
- `load()` 调用 `load_with_report().0`，保持 API 兼容。
- 把关键 `unwrap_or_default()` 的失败原因写入 report。
- 不阻止游戏启动。

验收：
- 默认行为不变。
- 测试可断言 report 能发现坏 RON。

测试：

```powershell
cargo test -p hoi4-content
cargo test -p hoi4-app
```

提交与推送：

```powershell
git status --short
git add crates/hoi4-content/src
git commit -m "refactor(content): gate 19 - add v6 load diagnostics"
git push
```

## [x] Gate 20 - 最终清理与边界验收

目标：确认重构结果没有破坏功能，整理公开边界。

任务：
- 确认 `main.rs` 仅保留：
  - imports
  - constants
  - `App`
  - `App::new`
  - `main`
  - 少量 glue
- 确认 `render()`、`window_event()`、`update()` 都是阶段调用，不再是业务堆积区。
- 确认新增模块没有循环式命名和无意义 pass-through。
- 清理只在重构中产生的临时 `pub(crate)`，但不要影响功能。

验收：
- `main.rs` 显著低于 3000 行。
- `render()` 低于 150 行。
- `window_event()` 低于 200 行。
- `update()` 低于 200 行。
- 不删除任何测试。
- 目录结构符合“最终结构要求”。
- 新模块职责可用一句话说明，且没有职责重叠严重的模块。
- 没有新增 `misc`、`helpers2`、`temporary`、`new_*` 这类临时命名模块。
- UI 数据构造、UI 命令执行、GPU frame、输入事件、模拟推进五条主流程边界清晰。

完整测试：

```powershell
cargo test -p hoi4-app
cargo test -p hoi4-ui
cargo test -p hoi4-content
cargo test -p hoi4-logic
cargo test -p hoi4-state
cargo test -p hoi4-runtime
cargo test -p hoi4-render
cargo test -p hoi4-map
cargo test -p hoi4-assets
```

如果本机有 HOI4 安装路径，再执行：

```powershell
cargo test --features vanilla-audit
```

提交与推送：

```powershell
git status --short
git add crates
git commit -m "refactor(app): gate 20 - finalize split boundaries"
git push
```

## [x] Gate 21 - 技术文档交付

目标：产出维护者可用的技术文档，说明拆分后的结构和关键流程。

任务：
- 新建 `docs/APP_SPLIT_REFACTOR_TECHNICAL_ZH.md`。
- 文档必须包含：
  - 总体模块图。
  - `hoi4-app` 目录职责表。
  - `App` 状态分组说明。
  - 输入事件流：winit -> input -> AppCommand/app methods。
  - UI 流：build data -> egui render -> command collect -> command apply。
  - 渲染流：prepare -> pass params -> map draw -> HUD -> egui paint -> submit。
  - 模拟流：update -> runtime schedule -> content events -> app aftermath。
  - map phase0 / audit / screenshot capture 流程。
  - 如何新增一个面板。
  - 如何新增一个快捷键。
  - 如何新增一个渲染 pass。
  - 如何新增一个 UI command。
  - 测试策略和推荐命令。
  - 回滚策略和 gate commit 规则。
- 文档不能只描述愿景，必须以最终代码结构为准。
- 文档里要列出关键文件路径和职责。

建议文档结构：

```text
# HOI4 App 技术文档

## 架构概览
## 目录职责
## App 状态分组
## 输入事件流
## UI 数据与命令流
## 渲染帧流程
## 模拟推进流程
## 审计与截图流程
## 新增功能指南
## 测试策略
## 回滚策略
```

验收：
- 技术文档存在且可直接指导维护。
- 文档中的模块名、文件名、函数名与最终代码一致。
- 文档说明“不影响功能完整性”的关键约束。
- 文档说明每个 gate 完成后必须 push 的工作流。

测试：

```powershell
cargo test -p hoi4-app
cargo test -p hoi4-ui
cargo test -p hoi4-content
cargo test -p hoi4-logic
```

提交与推送：

```powershell
git status --short
git add docs/APP_SPLIT_REFACTOR_TECHNICAL_ZH.md ROADMAP_APP_SPLIT_REFACTOR_GATES_ZH.md
git commit -m "docs(app): gate 21 - add split refactor technical guide"
git push
```

## 回滚策略

每个 gate 都必须独立 commit 并 push。任何 gate 出问题时，优先用 GitHub 上一个 gate 的 commit 回滚。

查看历史：

```powershell
git log --oneline --decorate -20
```

回滚单个 gate：

```powershell
git revert <commit_sha>
git push
```

禁止在协作分支上使用：

```powershell
git reset --hard
git push --force
```

除非明确确认这是个人临时分支且没有他人基于该分支工作。

## 每个 Gate 的固定完成清单

- `git status --short`
- 确认没有误改无关文件。
- 执行 gate 指定测试。
- 若测试失败，确认是本 gate 引入还是既有失败。
- `git add` 只添加本 gate 相关文件。
- `git commit -m "..."`
- `git push`
- 在 GitHub 上确认 commit 已出现。
- 记录本 gate 的 commit SHA。

## 风险最高区域

- `App::render()`：命令执行时序、egui 借用、submit 后 deferred command 顺序。
- `window_event()`：egui consume、global debug key、UI click-through、map click 三者交互。
- `update_loop`：daily work 分片、content events、暂停逻辑。
- `RenderState`：GPU resource lifetime、bind group rebuild、surface resize。
- `V6Database::load()`：静默 fallback 与内容完整性。

## 推荐执行顺序

严格按 Gate 0 到 Gate 21 执行。不要跳过 Gate 8-13 直接拆 `App` 字段；否则 Rust 借用问题会和行为拆分问题叠在一起，回滚成本会很高。Gate 21 是最终交付的一部分，不是可选项。
