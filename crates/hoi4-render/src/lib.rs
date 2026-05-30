//! `hoi4-render` — wgpu 渲染管线（pipelines / shaders / mesh / 资源管线辅助）。
//!
//! Phase 0 之后这个 crate **不再是入口**：所有 `winit` 事件循环、命令行解析、
//! `SystemSchedule` 接入、以及 `World::tick_hour` 的驱动都在 `hoi4-app`。
//! 本 crate 只暴露"画一帧需要的全部数据结构与帮助函数"。
//!
//! 顶层模块按渲染 pipeline / 资产解析分组：
//!
//! * 摄像机 / 视椎：[`camera`]
//! * 地形（heightmap mesh + chunk×LOD）：[`terrain`] / [`terrain_palette`]
//! * 地图模式着色 LUT：[`map_mode`]
//! * 边界 SDF：[`sdf`]
//! * 树木 billboard：[`trees`]
//! * 铁路线：[`railways`]
//! * 单位 counter（在 GPU 端渲染兵牌实例数据，shader 在 hoi4-app）：[`units`]
//! * 前线：[`frontlines`]
//! * 渲染辅助资源：[`buildings`]

pub mod border_extract;
pub mod buildings;
pub mod camera;
pub mod counter_atlas;
pub mod counter_layout;
pub mod counter_v3;
pub mod defines;
pub mod defines_lua;
pub mod frontlines;
pub mod global_uniform;
pub mod map_mode;
pub mod mapname;
pub mod mapname_3d;
pub mod particles;
pub mod province_labels;
pub mod railways;
pub mod sdf;
pub mod shader_rt;
pub mod terrain;
pub mod terrain_palette;
pub mod texture_upload;
pub mod trees;
pub mod trees_mesh;
pub mod units;

/// 主地形 / 边界 / 海面着色 wgsl。
/// 主地形 / 边界 / 海面着色 wgsl（**3.12.4 起归档**）。
///
/// 3.12.4 后地形管线已迁到 `hoi4_app::passes::TerrainPass`（基于
/// `terrain.wgsl`，与 vanilla `pdxmap.shader` 等价）。Phase 11 下线旧 inline
/// terrain fallback 后，本常量只作为归档参考保留；运行时不再通过旧别名消费它。
pub const ARCHIVED_SHADER_MAIN_WGSL: &str = include_str!("shader.wgsl");
/// 树木 billboard。
pub const SHADER_TREES_WGSL: &str = include_str!("trees.wgsl");
/// 铁路线 LineList。
pub const SHADER_RAILWAYS_WGSL: &str = include_str!("railways.wgsl");
// CR-5：旧 MapSymbolPass 已删除。`units.rs` 仅保留 UnitArchetype /
// classify_subunit / template_main_archetype / visibility 模块。
// 新兵牌系统走 `counter_v3.rs` + `hoi4_app::passes::counter_v3`。
/// 前线 LineList。
pub const SHADER_FRONTLINES_WGSL: &str = include_str!("frontlines.wgsl");
/// Phase 3.3: 后处理（vignette + tonemap + saturation）。
pub const SHADER_POSTFX_WGSL: &str = include_str!("postfx.wgsl");
/// Phase 3.5: 建筑图标 instanced billboard。
pub const SHADER_BUILDINGS_WGSL: &str = include_str!("buildings.wgsl");
/// Phase 14: POI 图标 instanced billboard（sprite atlas + fallback procedural）。
pub const SHADER_POI_ICON_WGSL: &str = include_str!("poi_icon.wgsl");
/// Phase 3.6.3: 3D mesh 树木 instanced rendering。
pub const SHADER_TREES_MESH_WGSL: &str = include_str!("trees_mesh.wgsl");
/// Phase 3.10.3: 国名 3D 标签 instanced quad shader。
pub const SHADER_MAPNAME_3D_WGSL: &str = include_str!("mapname_3d.wgsl");

/// Phase 3.11.1: `standardfuncsgfx.fxh` 等价 wgsl 库。
/// 不含入口；通过 [`shader_rt::compose_shader`] 注入到具体 shader 顶部。
pub const SHADER_LIB_WGSL: &str = include_str!("shader_lib.wgsl");

// =============================================================================
// Phase 3.11.3 ~ 3.11.14 — vanilla shader 1:1 等价翻译（translations/*.wgsl）
//
// 这些 shader **当前未挂入主渲染管线**——它们与 main.rs 的 binding 创建 +
// pipeline 注册是**独立**的工作。本模块只负责把 wgsl 源码作为常量暴露 +
// shader_rt 注册名映射，等到具体 pass 接入时再消费。
// =============================================================================

/// 3.11.3 — pdxmap.shader 等价：完整地形 + 法线 + CSM + point lights + 季节
pub const SHADER_PDXMAP_WGSL: &str = include_str!("translations/pdxmap.wgsl");
/// 3.11.4 — pdxmesh.shader 等价：替代 flat_diffuse fallback，PBR-ish + 阴影 + 反射
pub const SHADER_PDXMESH_WGSL: &str = include_str!("translations/pdxmesh.wgsl");
/// 3.11.5 — pdxwater.shader 等价：4-tap 水面法线 + Fresnel + 反射 + 冰层
pub const SHADER_PDXWATER_WGSL: &str = include_str!("translations/pdxwater.wgsl");
/// 3.11.6 — river.shader 等价：流向滚动 + level mask 通道
pub const SHADER_RIVER_WGSL: &str = include_str!("translations/river.wgsl");
/// 3.11.7 — tree.shader 完整翻译（替代 trees.wgsl / trees_mesh.wgsl 的简化版）
pub const SHADER_TREE_FULL_WGSL: &str = include_str!("translations/tree.wgsl");
/// 3.11.8 — border.shader 等价：6 类 × 3 LOD 边界纹理混合
pub const SHADER_BORDER_WGSL: &str = include_str!("translations/border.wgsl");
/// 3.11.9 — mapname.shader 等价：vanilla 路径（vDistortedPos + 昼夜暗化）
pub const SHADER_MAPNAME_VANILLA_WGSL: &str = include_str!("translations/mapname.wgsl");
/// 3.11.10 — particle.shader / sky.shader
pub const SHADER_PARTICLE_WGSL: &str = include_str!("translations/particle.wgsl");
pub const SHADER_SKY_WGSL: &str = include_str!("translations/sky.wgsl");
/// 3.11.11 — maparrow / traderoute / arrow / strait
pub const SHADER_MAPARROW_WGSL: &str = include_str!("translations/maparrow.wgsl");
pub const SHADER_TRADEROUTE_WGSL: &str = include_str!("translations/traderoute.wgsl");
pub const SHADER_ARROW_WGSL: &str = include_str!("translations/arrow.wgsl");
pub const SHADER_STRAIT_WGSL: &str = include_str!("translations/strait.wgsl");
/// 3.12.15.bis.0 — mapsymbol (SymbolVertexShader + SymbolPixelShader)
pub const SHADER_MAPSYMBOL_WGSL: &str = include_str!("translations/mapsymbol.wgsl");
/// 3.11.12 — shadow caster pass + shadowblur 7-tap 高斯
pub const SHADER_SHADOW_WGSL: &str = include_str!("translations/shadow.wgsl");
pub const SHADER_SHADOWBLUR_WGSL: &str = include_str!("translations/shadowblur.wgsl");
/// 3.11.13 — 后处理链（5 个 pass）
pub const SHADER_DOWNSAMPLE_WGSL: &str = include_str!("translations/downsample.wgsl");
pub const SHADER_DOWNSAMPLE_LUMINANCE_WGSL: &str =
    include_str!("translations/downsample_luminance.wgsl");
pub const SHADER_BLOOM_WGSL: &str = include_str!("translations/bloom.wgsl");
pub const SHADER_LUT_BLENDER_WGSL: &str = include_str!("translations/lut_blender.wgsl");
pub const SHADER_RESTORESCENE_WGSL: &str = include_str!("translations/restorescene.wgsl");
pub const SHADER_SATURATION_SLIDER_WGSL: &str = include_str!("translations/saturation_slider.wgsl");
/// 3.11.14 — GUI shader 全套（10 vanilla → 3 unified）
pub const SHADER_GUI_BUTTON_WGSL: &str = include_str!("translations/gui_button.wgsl");
pub const SHADER_GUI_PROGRESS_WGSL: &str = include_str!("translations/gui_progress.wgsl");
pub const SHADER_GUI_SPECIAL_WGSL: &str = include_str!("translations/gui_special.wgsl");
