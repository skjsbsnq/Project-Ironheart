//! Phase 3.11.1 — Shader-related constant mirror.
//!
//! 这一份文件把 vanilla `gfx/FX/constants.fxh` 与 `common/defines/00_graphics.lua` 中
//! **被 shader 引用**的全部常量镜像成 Rust `pub const`，以便:
//!
//! 1. 渲染管线代码（`crates/hoi4-render/src/*.rs`）需要用同样的魔数时直接引用
//!    （而不是继续散落"硬编码的 0.55 / 50.0"），保证与 vanilla 行为一致。
//! 2. 单元测试可以拿原版数值与 shader 内 `const` 值比较，做静态校验。
//! 3. Phase 3.11.15 的 CI 校验能用 `defines.lua` 解析器扫一遍 vanilla，对照本表
//!    检测"还有哪些常量被 shader 引用但本项目没镜像"。
//!
//! ## 命名约定
//!
//! - 每个常量的名字 **完全等于** 原版 HLSL/Lua 中的标识符（大写蛇形）。
//! - `f32` 默认；`vec3` 用 `[f32; 3]`，`vec4` 用 `[f32; 4]`。
//! - 注释里给出原版来源文件 + 行号锚点（如 `constants.fxh:55`）方便对照。
//!
//! ## 不收录
//!
//! - 与 shader 无关的纯 gameplay define（如 `DAYS_IN_MONTH`）：留给 `hoi4-data` /
//!   `hoi4-logic` 各自的 lua 加载层。
//! - 仅在 vanilla 注释里提到但实际未被 shader 引用的常量。
//!
//! ## 与 [`shader_lib`] 的关系
//!
//! `shader_lib.wgsl` 内部把这些常量复制了一份 wgsl 表达式（`const X: f32 = ...`），
//! 因为 wgsl 不能 `#include "rust"`。两边 **必须保持同步** ——
//! `tests/defines_match_shader_lib.rs` 用文本比较强制校验。

// =============================================================================
// constants.fxh — Lighting (ambient cube colors)
// =============================================================================

/// `constants.fxh:14` — `NIGHT_AMBIENT_BOOST`
pub const NIGHT_AMBIENT_BOOST: f32 = 3.0;

// constants.fxh:17-22 — DayAmbientMap{Pos,Neg}{X,Y,Z} 共 6 向量
pub const DAY_AMBIENT_POS_X: [f32; 3] = [0.10, 0.10, 0.05];
pub const DAY_AMBIENT_NEG_X: [f32; 3] = [0.15, 0.15, 0.15];
pub const DAY_AMBIENT_POS_Y: [f32; 3] = [0.03, 0.03, 0.06];
pub const DAY_AMBIENT_NEG_Y: [f32; 3] = [0.00, 0.00, 0.00];
pub const DAY_AMBIENT_POS_Z: [f32; 3] = [0.0502, 0.05023, 0.1023];
pub const DAY_AMBIENT_NEG_Z: [f32; 3] = [0.03, 0.033, 0.033];

// constants.fxh:24-29 — NightAmbientMap{Pos,Neg}{X,Y,Z}
pub const NIGHT_AMBIENT_POS_X: [f32; 3] = [0.20, 0.20, 0.20];
pub const NIGHT_AMBIENT_NEG_X: [f32; 3] = [0.00, 0.00, 0.00];
pub const NIGHT_AMBIENT_POS_Y: [f32; 3] = [0.01, 0.01, 0.01];
pub const NIGHT_AMBIENT_NEG_Y: [f32; 3] = [0.00, 0.00, 0.10];
pub const NIGHT_AMBIENT_POS_Z: [f32; 3] = [0.06, 0.10, 0.15];
pub const NIGHT_AMBIENT_NEG_Z: [f32; 3] = [0.14, 0.14, 0.20];

// =============================================================================
// constants.fxh — Specular
// =============================================================================

/// `constants.fxh:38` — `SPECULAR_WIDTH`
pub const SPECULAR_WIDTH: f32 = 15.0;
/// `constants.fxh:39` — `SPECULAR_MULTIPLIER`
pub const SPECULAR_MULTIPLIER: f32 = 1.0;
/// `constants.fxh:40` — `MAP_SPECULAR_WIDTH`
pub const MAP_SPECULAR_WIDTH: f32 = 15.0;

// =============================================================================
// constants.fxh — Terrain
// =============================================================================

/// `constants.fxh:46` — `CITY_LIGHTS_TILING`
pub const CITY_LIGHTS_TILING: f32 = 0.09103;
/// `constants.fxh:47` — `CITY_LIGHTS_INTENSITY`
pub const CITY_LIGHTS_INTENSITY: f32 = 5.5;
/// `constants.fxh:48` — `CITY_LIGHTS_BLOOM_FACTOR`
pub const CITY_LIGHTS_BLOOM_FACTOR: f32 = 0.3;

/// `constants.fxh:50` — `TERRAIN_TILE_FREQ`
pub const TERRAIN_TILE_FREQ: f32 = 128.0;
/// `constants.fxh:51` — `MAP_NUM_TILES` (atlas 4×4 = 16 地形)
pub const MAP_NUM_TILES: f32 = 4.0;
/// `constants.fxh:52` — `TEXELS_PER_TILE`
pub const TEXELS_PER_TILE: f32 = 512.0;
/// `constants.fxh:53` — `ATLAS_TEXEL_POW2_EXPONENT` (log2(2048))
pub const ATLAS_TEXEL_POW2_EXPONENT: f32 = 11.0;

/// `constants.fxh:54` — `TERRAIN_WATER_CLIP_HEIGHT`
pub const TERRAIN_WATER_CLIP_HEIGHT: f32 = 3.0;
/// `constants.fxh:55` — `TERRAIN_WATER_CLIP_CAM_HI`
pub const TERRAIN_WATER_CLIP_CAM_HI: f32 = 700.0;
/// `constants.fxh:56` — `TERRAIN_WATER_CLIP_CAM_LO`
pub const TERRAIN_WATER_CLIP_CAM_LO: f32 = 50.0;

// constants.fxh:58-60
pub const MUD_TILING: f32 = 0.09;
pub const MUD_NORMAL_CUTOFF: f32 = 10.982;
pub const MUD_STRENGHTEN: f32 = 1.0;

// constants.fxh:62-67 — snow / mud / ice 相机距离淡入淡出
pub const SNOW_OPACITY_MIN: f32 = 0.95;
pub const SNOW_OPACITY_MAX: f32 = 0.20;
pub const SNOW_CAM_MIN: f32 = 50.0;
pub const SNOW_CAM_MAX: f32 = 300.0;
pub const MUD_CAM_MIN: f32 = 50.0;
pub const MUD_CAM_MAX: f32 = 300.0;
pub const ICE_CAM_MIN: f32 = 100.0;
pub const ICE_CAM_MAX: f32 = 350.0;

// constants.fxh:71-79 — snow shading
pub const SNOW_START_HEIGHT: f32 = 3.0;
pub const SNOW_RIDGE_START_HEIGHT: f32 = 11.0;
pub const SNOW_NORMAL_START: f32 = 0.7;
pub const SNOW_COLOR: [f32; 3] = [0.46, 0.48, 0.69];
pub const SNOW_WATER_COLOR: [f32; 3] = [0.30, 0.60, 1.00];
pub const SNOW_CLIFFS: f32 = 5.0;
pub const SNOW_SPEC_GLOSS_MULT: f32 = 0.2;
pub const SNOW_TILING: f32 = 0.05;
pub const SNOW_NOISE_TILING: f32 = 0.06;
pub const SNOW_ICE_NOISE_TILING: f32 = 0.0625;
pub const SNOW_FROST_MIN_EFFECT: f32 = 0.4;

// constants.fxh:82-83 — ice
pub const ICE_COLOR: [f32; 3] = [0.50, 0.60, 0.90];
pub const ICE_NOISE_TILING: f32 = 0.10;

// constants.fxh:85-86 — water 表面颜色亮度 / 涟漪强度
pub const WATER_COLOR_LIGHTNESS: f32 = 0.5;
pub const WATER_RIPPLE_EFFECT: f32 = 0.0025;

// constants.fxh:88-89 — colormap overlay 强度
pub const COLORMAP_OVERLAY_STRENGTH: f32 = 0.75;
pub const COLORMAP_MUD_OVERLAY_STRENGTH: f32 = 0.5;

/// `constants.fxh:91` — `FAKE_CUBEMAP_COLOR`
pub const FAKE_CUBEMAP_COLOR: [f32; 3] = [0.0, 0.0, 0.0];

// =============================================================================
// constants.fxh / defines.lua — Border
// =============================================================================

/// `constants.fxh:101` — `BORDER_TILE`
pub const BORDER_TILE: f32 = 0.4;

/// `00_graphics.lua:764` — `BORDER_WIDTH = 1.5`
pub const BORDER_WIDTH: f32 = 1.5;

// =============================================================================
// constants.fxh — Trees / Tree season
// =============================================================================

/// `constants.fxh:113` — `TREE_SEASON_MIN`
pub const TREE_SEASON_MIN: f32 = 0.5;
/// `constants.fxh:114` — `TREE_SEASON_FADE_TWEAK`
pub const TREE_SEASON_FADE_TWEAK: f32 = 2.5;
/// `constants.fxh:120` — `TREE_SPECULAR`
pub const TREE_SPECULAR: f32 = 0.1;
/// `constants.fxh:121` — `TREE_ROUGHNESS`
pub const TREE_ROUGHNESS: f32 = 0.6;

// =============================================================================
// constants.fxh — HDR / Luminance
// =============================================================================

/// `constants.fxh:117` — `LUMINANCE_VECTOR` (Rec. 709)
pub const LUMINANCE_VECTOR: [f32; 3] = [0.2125, 0.7154, 0.0721];

// =============================================================================
// constants.fxh — Water surface
// =============================================================================

/// `constants.fxh:128` — `WATER_TIME_SCALE` = 1/50
pub const WATER_TIME_SCALE: f32 = 1.0 / 50.0;
/// `constants.fxh:129` — `WATER_HEIGHT`
pub const WATER_HEIGHT: f32 = 9.5;

// =============================================================================
// constants.fxh — Fog
// =============================================================================

/// `constants.fxh:147` — `FOG_COLOR`
pub const FOG_COLOR: [f32; 3] = [0.12, 0.28, 0.60];
/// `constants.fxh:148` — `FOG_BEGIN`
pub const FOG_BEGIN: f32 = 1.0;
/// `constants.fxh:149` — `FOG_END`
pub const FOG_END: f32 = 150.0;
/// `constants.fxh:150` — `FOG_MAX`
pub const FOG_MAX: f32 = 0.35;

// constants.fxh:154-156 — Fog of war 相机距离淡入
pub const FOW_MAX: f32 = 0.5;
pub const FOW_CAMERA_MIN: f32 = 200.0;
pub const FOW_CAMERA_MAX: f32 = 500.0;

// =============================================================================
// constants.fxh — Shadows (per-pass weight)
// =============================================================================

pub const SHADOW_WEIGHT_TERRAIN: f32 = 0.7;
pub const SHADOW_WEIGHT_MAP: f32 = 0.7;
pub const SHADOW_WEIGHT_BORDER: f32 = 0.7;
pub const SHADOW_WEIGHT_WATER: f32 = 0.5;
pub const SHADOW_WEIGHT_RIVER: f32 = 0.4;
pub const SHADOW_WEIGHT_TREE: f32 = 0.7;

// =============================================================================
// 00_graphics.lua — Light shadow direction (vanilla)
// =============================================================================

/// `00_graphics.lua:760` — directional light X 分量（投射阴影用）
pub const LIGHT_SHADOW_DIRECTION_X: f32 = -5.0;
/// `00_graphics.lua:761`
pub const LIGHT_SHADOW_DIRECTION_Y: f32 = -8.0;
/// `00_graphics.lua:762`
pub const LIGHT_SHADOW_DIRECTION_Z: f32 = 5.0;

/// `00_graphics.lua:763` — `LIGHT_HDR_RANGE`
pub const LIGHT_HDR_RANGE: f32 = 1.0;

// 00_graphics.lua:756-758 — 漫反射光主方向（不投阴影那个）
pub const LIGHT_DIRECTION_X: f32 = -1.0;
pub const LIGHT_DIRECTION_Y: f32 = -1.0;
pub const LIGHT_DIRECTION_Z: f32 = 0.5;

// =============================================================================
// 00_graphics.lua — Camera bounds
// =============================================================================

/// `00_graphics.lua:1495` — `CAMERA_MIN_HEIGHT`
pub const CAMERA_MIN_HEIGHT: f32 = 50.0;
/// `00_graphics.lua:1496` — `CAMERA_MAX_HEIGHT`
pub const CAMERA_MAX_HEIGHT: f32 = 3000.0;

// =============================================================================
// 00_graphics.lua — Day/Night & Globe normal
// =============================================================================

/// `00_graphics.lua:925` — `GMT_OFFSET = 2793`，地图上格林威治零经线的 X 像素位置
pub const GMT_OFFSET: f32 = 2793.0;
/// `00_graphics.lua:926` — `DAY_NIGHT_FEATHER`
pub const DAY_NIGHT_FEATHER: f32 = 0.024;
/// `00_graphics.lua:927` — `SOUTH_POLE_OFFSET`，球面投影南极
pub const SOUTH_POLE_OFFSET: f32 = 0.17;
/// `00_graphics.lua:928` — `NORTH_POLE_OFFSET`
pub const NORTH_POLE_OFFSET: f32 = 0.93;

// 以下三个原版没在 lua 里直接定义，但在 standardfuncsgfx.fxh 内被使用
// （`FEATHER_MIN/FEATHER_MAX` 用 DAY_NIGHT_FEATHER 推导，`MOON_FEATHER_*` 是月亮版本）。
// 这里给出与 vanilla shader 相同的取值。
pub const FEATHER_MIN: f32 = -DAY_NIGHT_FEATHER;
pub const FEATHER_MAX: f32 = DAY_NIGHT_FEATHER;
pub const MOON_FEATHER_MIN: f32 = -0.05;
pub const MOON_FEATHER_MAX: f32 = 0.05;

/// `standardfuncsgfx.fxh::GlobeNormalToMapNormal` — `GLOBE_NORMAL_LIMIT`
pub const GLOBE_NORMAL_LIMIT: f32 = 0.7;
/// `standardfuncsgfx.fxh` — `NIGHT_OPACITY` (默认 1.0；可由日历调整)
pub const NIGHT_OPACITY: f32 = 1.0;

// =============================================================================
// constants.fxh — Gradient borders (3.11.8 用)
// =============================================================================

pub const GB_CAM_MIN: f32 = 100.0;
pub const GB_CAM_MAX: f32 = 350.0;
pub const GB_CAM_MAX_FILLING_CLAMP: f32 = 0.8;
pub const GB_THRESHOLD: f32 = 0.05;
pub const GB_THRESHOLD2: f32 = 0.25;
pub const GB_OUTLINE_CUTOFF_SEA: f32 = 0.990;
pub const GB_OPACITY_NEAR: f32 = 1.0;
pub const GB_OPACITY_FAR: f32 = 0.85;
pub const BORDER_NIGHT_DESATURATION_MAX: f32 = 0.2;
pub const BORDER_FOW_REMOVAL_FACTOR: f32 = 0.8;
pub const BORDER_LIGHT_REMOVAL_FACTOR: f32 = 0.8;
pub const GB_STRENGTH_CH1: f32 = 1.0;
pub const GB_STRENGTH_CH2: f32 = 1.0;
pub const GB_FIRST_LAYER_PRIORITY: f32 = 0.4;
pub const BORDER_MAP_TILE: f32 = 18000.0;

// =============================================================================
// constants.fxh — Secondary color map（占领条纹 / 战时染色）
// =============================================================================

pub const SEC_MAP_TILE: f32 = 6000.0;

// =============================================================================
// constants.fxh — Map arrows (3.11.11)
// =============================================================================

pub const MAP_ARROW_SEL_BLINK_SPEED: f32 = 5.5;
pub const MAP_ARROW_SEL_BLINK_RANGE: f32 = 0.7;
pub const MAP_ARROW_NORMALS_STR_TERR: f32 = 0.0125;
pub const MAP_ARROW_NORMALS_STR_WATER: f32 = 0.08;

/// `00_graphics.lua` — `DEFAULT_MAP_ICON_SIZE`，兵牌默认缩放大小
pub const DEFAULT_MAP_ICON_SIZE: f32 = 8.0;

// =============================================================================
// constants.fxh — Particles (3.11.10)
// =============================================================================

pub const PARTICLE_FADE_START_DISTANCE: f32 = 100.0;
pub const PARTICLE_FADE_STOP_DISTANCE: f32 = 350.0;

// =============================================================================
// constants.fxh — Rim light (pdxmesh, 3.11.4)
// =============================================================================

pub const RIM_START: f32 = 0.55;
pub const RIM_END: f32 = 0.6;
pub const RIM_COLOR: [f32; 4] = [0.3, 0.3, 0.3, 0.0];

// =============================================================================
// constants.fxh — Map border (pdxmesh)
// =============================================================================

pub const BORDER_SUN_INTENSITY: [f32; 3] = [1.5, 1.5, 1.6];
pub const BORDER_SUN_DIRECTION: [f32; 3] = [-0.2, 0.9, 0.1];

// =============================================================================
// 00_graphics.lua — Port / ship offsets
// =============================================================================

/// `00_graphics.lua:739` — `PORT_SHIP_OFFSET`
pub const PORT_SHIP_OFFSET: f32 = 2.0;
/// `00_graphics.lua:740` — `SHIP_IN_PORT_SCALE`
pub const SHIP_IN_PORT_SCALE: f32 = 0.25;

// =============================================================================
// 00_graphics.lua — Province / state border fade
// =============================================================================

pub const PROVINCE_BORDER_FADE_NEAR: f32 = 200.0;
pub const PROVINCE_BORDER_FADE_FAR: f32 = 300.0;
pub const STATE_BORDER_FADE_NEAR: f32 = 400.0;
pub const STATE_BORDER_FADE_FAR: f32 = 500.0;

/// `00_graphics.lua` — `DRAW_COUNTRY_NAMES_CUTOFF`，国名标签隐藏阈值
pub const DRAW_COUNTRY_NAMES_CUTOFF: f32 = 260.0;

// =============================================================================
// Map dimensions（vanilla map/`heightmap.bmp` 尺寸）
// 这两个被 `standardfuncsgfx.fxh` 多次引用（`MAP_SIZE_X / MAP_SIZE_Y`）但 vanilla
// 不在 defines.lua 暴露——它直接由 `default.map` `image_width / image_height`
// 决定。我们在引擎启动时校验 vanilla 取值。
// =============================================================================

/// vanilla `default.map` `image_width = 5632`
pub const MAP_SIZE_X: f32 = 5632.0;
/// vanilla `default.map` `image_height = 2048`
pub const MAP_SIZE_Y: f32 = 2048.0;

/// `standardfuncsgfx.fxh::GetFoW` — `FOW_POW2_X`，FoW 纹理 X 维 power-of-2 缩放
pub const FOW_POW2_X: f32 = 5632.0 / 8192.0; // 5632 → 下一个 2^N (8192)
/// `standardfuncsgfx.fxh::GetFoW` — `FOW_POW2_Y`
pub const FOW_POW2_Y: f32 = 2048.0 / 2048.0; // 2048 已是 2^11

// =============================================================================
// 已知未镜像（占位，等到对应 phase 实做时再补）
// =============================================================================
// MILD_WINTER_VALUE / NORMAL_WINTER_VALUE / SEVERE_WINTER_VALUE：
//   vanilla 在 `gfx/FX/constants.fxh` 注释里说"see defines.lua"，但实际
//   只被脚本侧 weather 系统消费，shader 不直接读 → Phase 6 weather pass 再加。
// CONSTRUCTION_MAP_MODE_TRANSPARENCY_OVERRIDE / SUPPLY_MAP_MODE_*：
//   只供 mapmode 着色用 → Phase 4.* mapmode 重做时统一镜像。

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camera_bounds_sane() {
        assert!(CAMERA_MIN_HEIGHT > 0.0);
        assert!(CAMERA_MAX_HEIGHT > CAMERA_MIN_HEIGHT);
    }

    #[test]
    fn fog_distance_sane() {
        assert!(FOG_BEGIN > 0.0);
        assert!(FOG_END > FOG_BEGIN);
        assert!(FOG_MAX > 0.0 && FOG_MAX <= 1.0);
    }

    #[test]
    fn day_night_feather_symmetric() {
        assert_eq!(FEATHER_MIN, -FEATHER_MAX);
    }

    #[test]
    fn map_size_matches_vanilla() {
        // vanilla heightmap.bmp 是 5632×2048
        assert_eq!(MAP_SIZE_X as u32, 5632);
        assert_eq!(MAP_SIZE_Y as u32, 2048);
    }

    #[test]
    fn snow_color_in_unit_range() {
        for &c in &SNOW_COLOR {
            assert!(c >= 0.0 && c <= 1.0);
        }
    }

    #[test]
    fn shadow_weights_in_unit_range() {
        for w in [
            SHADOW_WEIGHT_TERRAIN,
            SHADOW_WEIGHT_MAP,
            SHADOW_WEIGHT_BORDER,
            SHADOW_WEIGHT_WATER,
            SHADOW_WEIGHT_RIVER,
            SHADOW_WEIGHT_TREE,
        ] {
            assert!((0.0..=1.0).contains(&w));
        }
    }

    #[test]
    fn pole_offsets_form_valid_range() {
        // SOUTH_POLE_OFFSET < NORTH_POLE_OFFSET，[0, 1] 内
        assert!(SOUTH_POLE_OFFSET >= 0.0);
        assert!(NORTH_POLE_OFFSET <= 1.0);
        assert!(NORTH_POLE_OFFSET > SOUTH_POLE_OFFSET);
    }

    #[test]
    fn luminance_vector_sums_to_one() {
        // Rec. 709 weights 应总和近 1.0
        let sum: f32 = LUMINANCE_VECTOR.iter().sum();
        assert!((sum - 1.0).abs() < 1e-3);
    }
}
