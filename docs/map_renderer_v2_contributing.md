# Map Renderer V2 贡献指南

本文补充根目录 `CONTRIBUTING.md`，只覆盖地图渲染相关改动。任何新增 pass、baseline layer、debug view 或 vanilla 资源角色，都要保持单一来源和可验收性。

## 新增 map layer

1. 在 `crates/hoi4-app/src/map_baseline.rs` 增加 `MapBaselineLayer` variant。
2. 更新 `MapBaselineLayer::ALL`、`as_str()` 和 `MapLayerMask::for_layer()`。
3. 如果 layer 只是暴露已有 pass，优先只改 `MapLayerMask`；不要新增 pass。
4. 如果必须新增 pass，同步更新 `MapRenderPass`、`PHASE1_ORDER`、`registry_name()`、`enabled_by_mask()`、`MapPassDrawSet` 字段和 `MapPassDrawSet::set()`。
5. 在 `main.rs` render 分支只根据 `map_frame_plan.draw.<field>` 绘制，不要复制 layer 判断。
6. 给 `map_baseline.rs` 或 `map_renderer.rs` 加小范围单测，证明 layer mask 和 frame plan 会打开正确 pass。
7. 更新 baseline 后检查 `phase0_latest.json`、`phase0_latest.txt` 和 `asset_audit_latest.json`。

当前 baseline CLI：

```powershell
cargo run -p hoi4-app -- --map-phase0 --map-phase0-output target/map_baseline_phase11
cargo run -p hoi4-app -- --map-phase0-report-only --map-phase0-output target/map_baseline_phase11
```

当前固定输出是 10 个 scene x 11 个 layer = 110 张 PNG。新增 layer 后必须在路线图或 PR 记录里写清新总数。

## 新增 debug view

Shader pass 级 debug view 使用同一模式：

1. 在对应 pass 文件增加或扩展 `enum XxxDebugView`。
2. 同步维护 `ALL`、`name()`、`as_shader_value()` 和 `next()`。
3. 在 `App` 或对应 pass state 保存当前 view。
4. 在 `app_shell.rs` 接键位。已有约定是 F6 terrain、F10 water、V border、Shift+F5 postprocess。
5. 在 prepare/update params 时写入 uniform，不要在 shader 中读全局 magic number。
6. 如需截图验收，新增或复用 `MapBaselineLayer`，并在 `apply_map_phase0_capture_settings()` 里设置 view。
7. 添加循环单测，确保最后一个 view 回到 off/final。

F4 overlay 不是 shader debug view。它的数据来自 `PassRegistry`、`GpuTimestampProfiler` 和 `phase10_overlay_lines()`，用于 pass 状态、CPU/GPU ms、draw call 和预算信息。

## 更新截图 baseline

1. 先跑 `cargo check -p hoi4-app`，避免在截图阶段发现编译问题。
2. 用固定输出目录生成完整 baseline。
3. 检查 `asset_audit_latest.json`：`quality` 必须可用于视觉验收；critical fallback 不可忽略。
4. 检查每个 planned capture 的 `layer_mask` 是否符合预期。
5. 保留 phase 命名目录，例如 `target/map_baseline_phase11`，方便和上一 phase 对比。
6. 在路线图完成记录里写清 scene、layer、PNG 数量和资源审计摘要。

只改资源分类或 manifest 时可先用 report-only：

```powershell
cargo run -p hoi4-app -- --map-phase0-report-only --map-phase0-output target/map_baseline_phase11
```

## 新增资源分类

1. 在 `crates/hoi4-assets/src/vanilla_map_set.rs` 增加 `MapResRole` variant。
2. 实现 `relative_path()`，路径必须是 vanilla/mod 相对路径。
3. 实现 `is_srgb()`：颜色、albedo、diffuse、colormap 通常是 sRGB；normal、mask、ID、height、lookup 数据必须保持 linear。
4. 实现 `allow_missing()`：只有缺失后仍能启动并有明确 fallback 的资源才返回 true。
5. 把角色加入 `all_vanilla_roles()`，保证 audit 能枚举。
6. 如果 pass 会上传该资源，优先走 `TextureUploadHelper::upload_role()` 或已有 DDS/BMP structured loader，不要手写路径读取。
7. 为路径、sRGB、allow_missing 和 audit 分类补单测。

Fallback 分类必须写在代码或文档里：

- `startup-required`：缺失时地图无法正确启动。
- `visual-required`：缺失时可启动，但 baseline 不可作为画质验收。
- `cosmetic`：缺失时使用 1x1、程序化占位或空实例，必须可审计。

## 新增 pass 的最低要求

- pass 名称必须先加入 `MapRenderPass::PHASE1_ORDER`，不要只在 `main.rs` 插 draw call。
- `PassRegistry` 名称必须稳定，因为 F4 overlay、perf stats 和测试会依赖它。
- pass 是否绘制必须通过 `MapRenderer::build_frame_plan()` 收敛。
- 输入、输出、depth/blend、fallback、debug view 和限制要同步写入 `docs/map_renderer_v2.md`。
- 至少跑一个 frame-plan 单测和 `cargo check -p hoi4-app`。
