# HOI4 App 技术文档

## 架构概览

`hoi4-app` 现在以 `crates/hoi4-app/src/main.rs` 为入口和组装层，具体业务流按职责拆到输入、UI、渲染、模拟、状态和审计模块中。主路径如下：

```text
winit
  -> app_shell.rs
  -> input/*
  -> App methods / AppCommand

App::render()
  -> render_frame::prepare::render_pipeline()
  -> ui_driver::build::build_ui_data()
  -> ui_driver::render::render_ui()
  -> ui_driver::commands::apply_ui_render_output()
  -> render_frame prepare / draw / hud / submit

App::update()
  -> simulation::tick::update_simulation_tick()
  -> runtime::systems_runtime
  -> simulation aftermath modules
```

`main.rs` 保留模块声明、核心常量、`App` 结构、`App::new`、少量 glue、命令行入口和测试。`App::render()` 只调用 `render_pipeline()`；`ApplicationHandler::window_event()` 只转发到 `input::window::handle_window_event()`；`App::update()` 只转发到 `update_simulation_tick()`。

## 目录职责

| 路径 | 职责 |
| --- | --- |
| `main.rs` | 应用入口、模块声明、`App` 组装和全局常量；不承载长业务流程。 |
| `app_shell.rs` | winit 生命周期 glue，创建 `App` 并接入窗口事件。 |
| `app_command.rs` | UI 命令汇总枚举，连接 UI 输出和 app 副作用执行。 |
| `app_state/*` | 对 `App` 字段做职责分组，不执行业务流程。 |
| `input/*` | 将 winit 输入事件路由到 app 行为、快捷键和鼠标交互。 |
| `ui_driver/build.rs` | 从 `App` 和 `World` 构造 UI 数据快照。 |
| `ui_driver/render.rs` | 执行 egui 绘制并收集 `AppCommand`。 |
| `ui_driver/commands.rs` | 按原顺序应用 UI 命令和延迟命令。 |
| `render_frame/prepare.rs` | 渲染帧流水线编排、GPU 参数准备、map draw/HUD/submit 阶段连接。 |
| `render_frame/hud.rs` | 构造 debug overlay、legacy HUD、菜单 HUD、flag/text HUD。 |
| `render_frame/submit.rs` | egui paint、queue submit、present、截图 readback 和 profiler 收尾。 |
| `simulation/*` | 模拟推进及推进后的 app 侧副作用。 |
| `map_interaction.rs` | 地图点击、框选、counter 选择和前线绘制。 |
| `camera_control.rs` | 摄像机坐标、缩放和平滑缩放上传。 |
| `map_refresh.rs` | LUT、国家标签和地图刷新。 |
| `world_overlays.rs` | counter、前线箭头、前线 overlay、贸易路线 overlay。 |
| `combat_overlay.rs` | 战斗气泡数据与显示辅助。 |
| `military_ui_data.rs` | 陆海空和模板编辑器 UI 数据转换辅助。 |
| `render_assets.rs` | 地形、树、colormap、river texture 等资源加载。 |
| `gpu_utils.rs` / `gpu_readback.rs` | GPU 限制、depth view 和 PNG readback。 |
| `map_phase0_run.rs` / `edge_pan_test.rs` | 审计批处理和 edge pan 自动测试状态。 |

## App 状态分组

`App` 仍是运行期总装对象，但大块状态已经分组：

| 类型 | 文件 | 内容 |
| --- | --- | --- |
| `RuntimeState` | `app_state/runtime.rs` | 经济、科研、政治缓存、脚本、AI、系统调度、音乐、内容 runtime、自动建设和战争计数。 |
| `ViewState` | `app_state/view.rs` | 游戏阶段、玩家国家、主菜单/国家选择菜单状态。 |
| `InteractionState` | `app_state/interaction.rs` | 选中单位、省份、counter 右键菜单、海空军转移、前线绘制和模板编辑状态。 |
| `UiStateBundle` | `app_state/ui.rs` | 面板、弹窗、事件音效、设置、存档浏览器、end screen、UI cache。 |
| `RenderToggles` | `app_state/render_toggles.rs` | debug overlay、地形/水/边界/postprocess 调试视图、质量 preset、overlay hash。 |
| `PerfState` | `app_state/perf.rs` | 性能日志时间戳、counter rebuild/cache、帧耗时统计。 |
| `AuditState` | `app_state/audit.rs` | `MapPhase0Run` 和 `EdgePanTestRun`。 |

未迁移到这些 bundle 的字段主要是跨阶段基础设施或热路径缓存，例如 `RenderState`、`Camera`、`World`、`MapMode`、counter layout cache、seasons、V6 数据库和历史数据库。

## 输入事件流

1. `app_shell.rs` 的 `ApplicationHandler for App` 接收 winit 生命周期事件。
2. `window_event()` 仅调用 `self.handle_window_event(event_loop, event)`。
3. `input/window.rs` 先让 egui 消费事件，再处理全局 debug key 和海空军面板的地图点击穿透。
4. 窗口 resize 在 `input/window.rs` 内更新 surface、depth、HDR、水反射目标、postprocess target、camera aspect 和 text pass 尺寸。
5. 键盘事件进入 `input/keyboard.rs`，鼠标滚轮/左右键/移动进入 `input/mouse.rs`，菜单 hover 进入 `input/menu.rs`。
6. 输入模块可以调用 app 方法或改变 app 状态，但不构造 UI 面板数据，也不提交 GPU command。

新增快捷键时优先修改 `input/keyboard.rs`。如果快捷键需要绕过 egui 消费，先在 `input/debug_hotkeys.rs::is_global_debug_key()` 登记，再在 keyboard 处理里执行具体行为。

## UI 数据与命令流

UI 分三段：

1. `ui_driver::build::build_ui_data(app, app_ui_enabled)` 构造 `UiBuildOutput`。这里读取 world、runtime、UI cache 和面板打开状态，生成 topbar、面板、战斗气泡、focus、设置、存档等数据。
2. `ui_driver::render::render_ui(app, ui_data)` 执行 `s.ui.begin_frame(... |ctx| { ... })`，保持 egui 控件绘制顺序，并把按钮输出汇总成 `Vec<AppCommand>`。
3. `ui_driver::commands::apply_ui_render_output(app, output)` 分组应用命令；`apply_deferred_ui_commands(app, output)` 在帧末按顺序处理 topbar、side rail、panel commands、延迟切换国家等。

新增面板的推荐步骤：

1. 在 `InGamePanel` 和需要的 `PanelKind` 转换处登记新面板。
2. 在 `ui_driver/build.rs` 给面板增加数据字段，并只做数据构造。
3. 在 `ui_driver/render.rs` 调用对应 `hoi4-ui` 面板，收集 close 和 panel command。
4. 如有副作用，在 `app_command.rs` 增加命令变体，并在 `ui_driver/commands.rs` 执行。
5. 给 `hoi4-ui` 面板和 app 侧命令路径补测试。

新增 UI command 时必须保持命令顺序：先在 `AppCommand` 加变体，再在 `render_ui()` 收集，最后在 `apply_ui_render_output_inner()` 归类和执行。不要让 `ui_driver/render.rs` 直接改 `World`。

## 渲染帧流程

`App::render()` 是单行入口，实际流程在 `render_frame/prepare.rs::render_pipeline()`：

1. `prepare_visual_ui_phase()`：准备 map phase0 capture、计算 layer mask、规划 world objects、更新 counter/frontline/trade overlays、构造 UI 数据、开始 egui frame 并应用 UI 命令。
2. `prepare_render_params()`：生成 `RenderParams`、zoom、time、date、season 和 vanilla map space。
3. `update_terrain_buckets()`：更新地形 instance bucket。
4. `acquire_surface_and_capture_target()`：获取 surface frame，必要时创建 capture target。
5. `update_global_uniforms()`：上传 camera/global uniform。
6. `prepare_map_renderer_frame()`：构造 map renderer frame plan 和 prepared frame。
7. `update_vanilla_targets()`：更新 vanilla runtime targets。
8. `update_pass_params()`：更新 terrain、水、border、tree、particle、maparrow、traderoute、strait 等 pass 参数。
9. `draw_map_and_hud()`：通过 `map_draw::render_map_frame()` 绘制地图，再调用 `render_frame/hud.rs` 生成 HUD。
10. `submit_prepared_render_frame()`：调用 `render_frame/submit.rs` 完成 egui paint、submit、present、readback 和 perf 统计。

新增渲染 pass 时，pass 资源和 draw API 放在 `passes/*` 或对应渲染模块；每帧参数更新接入 `update_pass_params()`；实际绘制顺序接入 `map_draw.rs` 或 `MapRenderer` 的 frame plan。不要从 pass 模块调用外交、科研、建设、存档等业务命令。

## 模拟推进流程

`update_loop.rs::App::update(max_sim_budget_secs)` 只调用 `simulation::tick::update_simulation_tick()`。`update_simulation_tick()` 负责：

1. 计算 frame dt 和 interaction dt。
2. 在 map phase0 或非 Playing 阶段直接返回。
3. 更新 edge pan、smooth zoom、键盘/边缘平移和 camera upload。
4. paused 时只更新单位动画、标题和 perf。
5. 非 paused 时按速度预算推进小时 tick，并通过 `runtime::systems_runtime` 运行 pending daily/weekly/monthly work。
6. 推进后调用 `simulation/content_events.rs`、`war_pause.rs`、`end_screen.rs`、`music.rs` 等 app 侧副作用。

自动建设逻辑在 `simulation/auto_build.rs`，内容事件弹窗和事件暂停在 `simulation/content_events.rs`，战争自动暂停在 `simulation/war_pause.rs`，940 end screen 在 `simulation/end_screen.rs`，音乐自动切换在 `simulation/music.rs`。

## 审计与截图流程

`map_phase0_run.rs` 持有 `MapPhase0Run`，由 CLI `--map-phase0` 通过 `app_shell::run_map_phase0()` 启动：

1. `enable_map_phase0()` 加载 vanilla resource audit、asset audit、binding audit，并切换到 Playing + Paused。
2. `prepare_map_phase0_capture()` 根据当前 planned capture 设置 camera、date、map mode、debug view 和 UI/label mask。
3. `render_pipeline()` 使用 `current_map_layer_mask()` 控制 UI 和 layer。
4. `submit_render_frame()` 结束后通过 `gpu_readback::enqueue_png_readback()` 和 `finish_png_readback()` 写 PNG。
5. `map_phase0_after_capture()` 推进 capture index；结束时 `finish_map_phase0()` 写报告。

`edge_pan_test.rs` 管理 `EdgePanTestRun`，在 update 阶段更新 cursor，在 submit 阶段记录 frame，完成后由 `finish_edge_pan_test()` 输出结果。

普通截图 readback 走 `gpu_readback.rs` 的 `PendingPngReadback`，只处理 GPU buffer 到 PNG 的读回，不改变世界状态。

## 新增功能指南

新增面板：先在 `hoi4-ui` 定义数据和命令，再在 `ui_driver/build.rs` 构造数据，在 `ui_driver/render.rs` 绘制，在 `app_command.rs` / `ui_driver/commands.rs` 执行副作用。

新增快捷键：在 `input/keyboard.rs` 添加处理；如果是全局 debug 快捷键，同步更新 `input/debug_hotkeys.rs`，保证 egui 消费规则不破坏原有面板输入。

新增渲染 pass：资源初始化放到 `render_init.rs` 或 `render_assets.rs`，pass 类型放到 `passes/*`，每帧参数更新接入 `render_frame/prepare.rs::update_pass_params()`，绘制接入 `map_draw.rs` 或 `MapRenderer`。

新增 UI command：在 `app_command.rs` 增加变体，`ui_driver/render.rs` 只收集命令，`ui_driver/commands.rs` 统一执行。需要延迟执行的命令放进 `UiCommandApplyOutput` 并由 `apply_deferred_ui_commands()` 处理。

新增模拟后副作用：优先放到 `simulation/*` 中与职责匹配的文件。只有 runtime tick 和 schedule 协调留在 `simulation/tick.rs`。

## 测试策略

常规 app 边界变更至少执行：

```powershell
cargo test -p hoi4-app
cargo test -p hoi4-ui
cargo test -p hoi4-content
cargo test -p hoi4-logic
```

完整 Gate 20 验证执行：

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

如果本机有 HOI4 vanilla 安装路径，再执行：

```powershell
cargo test --features vanilla-audit
```

渲染 pass 相关改动额外执行 shader 测试：

```powershell
cargo test -p hoi4-app terrain_wgsl
cargo test -p hoi4-app river_wgsl
cargo test -p hoi4-app counter_v3_wgsl
cargo test -p hoi4-render shader_lib_compiles
```

UI 视觉相关改动额外执行：

```powershell
cargo test -p hoi4-ui v9_visual_snapshots
```

## 回滚策略

每个 gate 必须独立 commit 并 push，commit message 使用路线图指定格式，例如：

```text
refactor(app): gate 20 - finalize split boundaries
docs(app): gate 21 - add split refactor technical guide
```

回滚优先使用 GitHub 上已经 push 的上一 gate commit：

```powershell
git log --oneline --decorate -20
git revert <commit_sha>
git push
```

协作分支禁止使用 `git reset --hard` 和强推，除非明确确认这是个人临时分支且没有其他人基于该分支工作。

## 边界约束

当前边界验收约束：

- `main.rs` 显著低于 3000 行。
- `App::render()` 是阶段入口，低于 150 行。
- `ApplicationHandler::window_event()` 是 winit 转发 glue，低于 200 行。
- `App::update()` 是模拟阶段入口，低于 200 行。
- 没有新增 `misc`、`helpers2`、`temporary`、`new_*` 这类临时命名模块。
- UI 数据构造、UI 命令执行、GPU frame、输入事件、模拟推进五条主流程分别落在 `ui_driver/build.rs`、`ui_driver/commands.rs`、`render_frame/*`、`input/*`、`simulation/*`。
- 不删除测试，不把 GPU 资源所有权放进 UI 数据构造，不让底层数据或模拟逻辑依赖 UI 绘制模块。
