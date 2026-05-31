//! Phase 3.11.1 — `GlobalFrameUniform` 公共 uniform 布局。
//!
//! vanilla `gfx/FX/standardfuncsgfx.fxh:2` 的 `ConstantBuffer(0, 0)` 把"几乎人人都
//! 用得到的全帧状态"集中放在一个共享 uniform 里：ViewProjectionMatrix /
//! vCamPos / vSeasonLerp / vTime / 太阳方向 / 雾参数 / FoW 参数 / 屏幕尺寸 /
//! shadow matrix。
//!
//! 我们对应给 wgsl `@group(0) @binding(0) var<uniform> frame: GlobalFrameUniform`。
//! 所有后续翻译的 shader 都共用这个 binding，**不重复**自己造一份。
//!
//! ## 大小要求
//!
//! `wgpu` 24 在 d3d12/vulkan 后端要求 uniform binding 至少 16-byte 对齐。本结构
//! 共 **352 bytes**，包含主 ViewProj、shadow ViewProj 和 Phase 2 map-space 状态。
//!
//! ## 与 vanilla `ConstantBuffer(0, 0)` 的差异
//!
//! vanilla 的字段顺序按 HLSL packing 规则（`mat4 + mat4 + 一堆 vec4 + scalar`）；
//! wgpu/std140 类似但不完全相同。本结构按 **wgsl std140** 对齐重排：先
//! 大尺寸（mat4 / vec4），再 vec3+f32 配对（避开 16-byte alignment hole）。
//!
//! ## 字段说明
//!
//! | 字段 | 类型 | offset | 用途 |
//! |---|---|---|---|
//! | view_proj | mat4x4 | 0 | 主投影矩阵（替 vanilla `ViewProjectionMatrix`） |
//! | virtual_sun_pos | vec4 | 64 | 太阳虚拟世界坐标（用于 day-night 抗 wrap-around） |
//! | virtual_moon_pos | vec4 | 80 | 月亮虚拟世界坐标 |
//! | second_virtual_sun_pos | vec4 | 96 | 第二个虚拟太阳（绕地球另一侧） |
//! | second_virtual_moon_pos | vec4 | 112 | 第二个虚拟月亮 |
//! | day_night_hour_sun_dir | vec4 | 128 | x = day_night_hour [0,1]; yzw = sun direction |
//! | fow_opacity_time_snow_max_speed | vec4 | 144 | x=FoW opacity / y=global time / z=snow_mud_fade / w=max_game_speed |
//! | cam_pos | vec3 | 160 | 相机世界坐标 |
//! | hdr_range | f32 | 172 | HDR 范围乘子（vanilla `HdrRange`） |
//! | cam_look_at_dir | vec3 | 176 | 相机视线方向 |
//! | global_time | f32 | 188 | 全局时间秒（也存于上面 vec4 的 y，留单独字段方便 wgsl 按 scalar 用） |
//! | screen_size | vec2 | 192 | logical 屏幕尺寸 |
//! | shadow_fade_factor | f32 | 200 | vanilla `ShadowFadeFactor` |
//! | fow_fade_factor | f32 | 204 | vanilla `FOWFadeFactor` |
//! | sun_diffuse_intensity | vec4 | 208 | vanilla `SunDiffuseIntensity` |
//! | moon_diffuse_intensity | vec4 | 224 | vanilla `MoonDiffuseIntensity` |
//! | min_mesh_alpha | f32 | 240 | vanilla `MinMeshAlpha` |
//! | neg_fog_multiplier | f32 | 244 | vanilla `NegFogMultiplier` |
//! | cubemap_intensity | f32 | 248 | vanilla `CubemapIntensity` |
//! | sun_specular_intensity | f32 | 252 | vanilla `SunSpecularIntensity` |
//! | shadow_view_proj | mat4x4 | 256 | Phase 3.12.3 — directional shadow caster matrix |
//! | vanilla_map_size_world_size | vec4 | 320 | xy=vanilla map pixels, zw=renderer world XZ size |
//! | cam_pos_map_px | vec4 | 336 | camera position in vanilla map-pixel space |
//!
//! 共 **352 bytes**（Phase 2 map-space 扩展后）。原 256/320 bytes 阶段保留在 git 历史。

use bytemuck::{Pod, Zeroable};

/// 与 wgsl `struct GlobalFrameUniform` 按 std140 等价对齐的 Rust 镜像。
///
/// **构造**：用 `default()` 拿一个零值默认，然后按 setter 风格更新感兴趣的字段；
/// 或直接构造（推荐用 `..Default::default()`）。
///
/// **GPU 上传**：
/// ```ignore
/// let bytes: &[u8] = bytemuck::bytes_of(&uniform);
/// queue.write_buffer(&buf, 0, bytes);
/// ```
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct GlobalFrameUniform {
    /// view × projection 主矩阵（column-major，wgsl `mat4x4<f32>` 与 glam `Mat4` 一致）。
    pub view_proj: [[f32; 4]; 4],

    /// 太阳虚拟世界坐标。w 通道留作 sun-on-water 切换。
    pub virtual_sun_pos: [f32; 4],
    /// 月亮虚拟世界坐标。w 通道同上。
    pub virtual_moon_pos: [f32; 4],
    /// 第二个虚拟太阳（处理地球另一侧、防止跨子午线时拉伸）。
    pub second_virtual_sun_pos: [f32; 4],
    /// 第二个虚拟月亮。
    pub second_virtual_moon_pos: [f32; 4],

    /// `(day_night_hour, sun_dir.x, sun_dir.y, sun_dir.z)`。
    /// vanilla `DayNight_Hour_SunDir` 同款打包。
    pub day_night_hour_sun_dir: [f32; 4],

    /// `(fow_opacity, global_time, snow_mud_fade, max_game_speed)`。
    /// vanilla `vFoWOpacity_FoWTime_SnowMudFade_MaxGameSpeed` 同款打包。
    pub fow_opacity_time_snow_max_speed: [f32; 4],

    /// 相机世界坐标。
    pub cam_pos: [f32; 3],
    /// HDR 范围乘子。
    pub hdr_range: f32,

    /// 相机视线方向。
    pub cam_look_at_dir: [f32; 3],
    /// 全局时间秒（与 `fow_opacity_time_snow_max_speed.y` 同步；scalar 路径方便）。
    pub global_time: f32,

    /// logical 屏幕尺寸（受 HiDPI 影响走 logical，不是 physical）。
    pub screen_size: [f32; 2],
    /// shadow 远端淡出因子。
    pub shadow_fade_factor: f32,
    /// FoW 全局淡出因子。
    pub fow_fade_factor: f32,

    /// 太阳漫反射强度（rgb + 总强度 a）。
    pub sun_diffuse_intensity: [f32; 4],
    /// 月亮漫反射强度。
    pub moon_diffuse_intensity: [f32; 4],

    /// mesh 的最小 alpha（防止远景全透 + clip）。
    pub min_mesh_alpha: f32,
    /// 雾在 -y 朝下时的乘子（让水面反射的天空雾不奇怪）。
    pub neg_fog_multiplier: f32,
    /// 立方体反射强度。
    pub cubemap_intensity: f32,
    /// 太阳高光强度。
    pub sun_specular_intensity: f32,

    /// Phase 3.12.3 — directional shadow caster 的 view × ortho-proj 矩阵。
    /// main.rs 每帧根据 [`crate::defines::LIGHT_SHADOW_DIRECTION_X/Y/Z`] + 场景包围盒计算。
    /// 接收方 shader 用 `shadow_view_proj * world_pos` 得到 shadow-map UV。
    pub shadow_view_proj: [[f32; 4]; 4],

    /// `(map_px_w, map_px_h, world_w, world_d)`.
    pub vanilla_map_size_world_size: [f32; 4],
    /// Camera position in vanilla map-pixel space. xy used, zw reserved.
    pub cam_pos_map_px: [f32; 4],
}

impl Default for GlobalFrameUniform {
    fn default() -> Self {
        Self {
            view_proj: identity4(),
            virtual_sun_pos: [0.0; 4],
            virtual_moon_pos: [0.0; 4],
            second_virtual_sun_pos: [0.0; 4],
            second_virtual_moon_pos: [0.0; 4],
            day_night_hour_sun_dir: [0.5, 0.0, 1.0, 0.0],
            fow_opacity_time_snow_max_speed: [0.0, 0.0, 0.0, 5.0],
            cam_pos: [0.0; 3],
            hdr_range: 1.0,
            cam_look_at_dir: [0.0, -1.0, 0.0],
            global_time: 0.0,
            screen_size: [1920.0, 1080.0],
            shadow_fade_factor: 1.0,
            fow_fade_factor: 1.0,
            sun_diffuse_intensity: [1.0, 1.0, 1.0, 1.0],
            moon_diffuse_intensity: [0.3, 0.3, 0.5, 1.0],
            min_mesh_alpha: 0.0,
            neg_fog_multiplier: 1.0,
            cubemap_intensity: 1.0,
            sun_specular_intensity: 1.0,
            shadow_view_proj: identity4(),
            vanilla_map_size_world_size: [5632.0, 2048.0, 112.0, 41.0],
            cam_pos_map_px: [0.0; 4],
        }
    }
}

const fn identity4() -> [[f32; 4]; 4] {
    [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

/// 与上方 Rust struct 字面对应的 wgsl `struct` 定义。
///
/// 后续翻译的每个 shader 在最顶部 `compose_shader` 时把这段拼进来即可
/// （或通过 [`crate::shader_rt::compose_shader`] 自动注入）。
///
/// **重要**：字段顺序与 [`GlobalFrameUniform`] 的 `#[repr(C)]` 字段顺序
/// **必须完全一致**——`tests/global_uniform_size.rs` 用 `size_of` 对照
/// 352 byte 期望值，shader compose 测试用 naga 检查类型。
pub const GLOBAL_FRAME_UNIFORM_WGSL: &str = r#"
struct GlobalFrameUniform {
    view_proj: mat4x4<f32>,
    virtual_sun_pos: vec4<f32>,
    virtual_moon_pos: vec4<f32>,
    second_virtual_sun_pos: vec4<f32>,
    second_virtual_moon_pos: vec4<f32>,
    day_night_hour_sun_dir: vec4<f32>,
    fow_opacity_time_snow_max_speed: vec4<f32>,
    cam_pos: vec3<f32>,
    hdr_range: f32,
    cam_look_at_dir: vec3<f32>,
    global_time: f32,
    screen_size: vec2<f32>,
    shadow_fade_factor: f32,
    fow_fade_factor: f32,
    sun_diffuse_intensity: vec4<f32>,
    moon_diffuse_intensity: vec4<f32>,
    min_mesh_alpha: f32,
    neg_fog_multiplier: f32,
    cubemap_intensity: f32,
    sun_specular_intensity: f32,
    shadow_view_proj: mat4x4<f32>,
    vanilla_map_size_world_size: vec4<f32>,
    cam_pos_map_px: vec4<f32>,
};
"#;

/// 期望大小（bytes）。**必须**与 wgsl std140 + Rust `#[repr(C)]` 匹配。
pub const GLOBAL_FRAME_UNIFORM_SIZE: usize = 352;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn struct_size_is_352_bytes() {
        // Phase 2 map-space fields extend the shadow-enabled layout by 32 bytes.
        assert_eq!(
            std::mem::size_of::<GlobalFrameUniform>(),
            GLOBAL_FRAME_UNIFORM_SIZE
        );
        assert_eq!(GLOBAL_FRAME_UNIFORM_SIZE, 352);
    }

    #[test]
    fn struct_size_is_16_aligned() {
        // wgpu uniform binding 要求 16-byte 对齐。
        assert!(std::mem::size_of::<GlobalFrameUniform>() % 16 == 0);
    }

    #[test]
    fn default_values_sane() {
        let u = GlobalFrameUniform::default();
        assert_eq!(u.hdr_range, 1.0);
        assert_eq!(u.view_proj[0][0], 1.0); // identity
        assert_eq!(u.screen_size, [1920.0, 1080.0]);
    }

    #[test]
    fn pod_round_trip() {
        // bytemuck::bytes_of → from_bytes 必须等价
        let u = GlobalFrameUniform {
            global_time: 12.5,
            ..Default::default()
        };
        let bytes = bytemuck::bytes_of(&u);
        assert_eq!(bytes.len(), GLOBAL_FRAME_UNIFORM_SIZE);
        let back: GlobalFrameUniform = *bytemuck::from_bytes(bytes);
        assert_eq!(back.global_time, 12.5);
    }

    #[test]
    fn wgsl_struct_text_present() {
        assert!(GLOBAL_FRAME_UNIFORM_WGSL.contains("struct GlobalFrameUniform"));
        assert!(GLOBAL_FRAME_UNIFORM_WGSL.contains("view_proj: mat4x4<f32>"));
        assert!(GLOBAL_FRAME_UNIFORM_WGSL.contains("sun_specular_intensity: f32"));
        // Phase 3.12.3 — shadow caster matrix
        assert!(GLOBAL_FRAME_UNIFORM_WGSL.contains("shadow_view_proj: mat4x4<f32>"));
        assert!(GLOBAL_FRAME_UNIFORM_WGSL.contains("vanilla_map_size_world_size: vec4<f32>"));
        assert!(GLOBAL_FRAME_UNIFORM_WGSL.contains("cam_pos_map_px: vec4<f32>"));
    }
}
