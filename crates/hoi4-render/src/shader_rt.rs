//! Phase 3.2：`shader-rt` — 等价着色器注册表。
//!
//! 原版 `gfx/FX/*.shader` 是 Paradox 自定义 DSL，我们不重新实现编译器，
//! 而是建一个静态映射表：原版 shader path/name → 对应的 wgsl 模块源码。
//!
//! ## 用法
//! ```ignore
//! let registry = ShaderRegistry::new();
//! let wgsl = registry.resolve("pdxmap");
//! // wgsl == Some(&ShaderEntry { source: "...", label: "pdxmap" })
//! ```
//!
//! ## 设计
//! - `ShaderRegistry` 是只读静态表，启动时构造一次
//! - mesh 加载时取 `.mesh` 材质的 shader name，查表挂上对应 wgsl pipeline
//! - 缺失 shader → fallback `flat_diffuse` + 打 warn
//! - `MaterialParams` 结构体传递 .gfx 里的 cutoff / specular / tint 等参数

use std::collections::HashMap;

/// 一个已注册的 wgsl shader 等价物。
#[derive(Debug, Clone)]
pub struct ShaderEntry {
    /// 人类可读标签（用于 pipeline label / 诊断）。
    pub label: &'static str,
    /// wgsl 源码（编译时 include 或运行时生成）。
    pub source: &'static str,
    /// 该 shader 需要的 vertex 属性集合。
    pub vertex_attrs: VertexAttrs,
}

/// 顶点属性需求（shader 期望的输入）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VertexAttrs {
    pub position: bool,
    pub normal: bool,
    pub uv: bool,
    pub tangent: bool,
    pub color: bool,
    pub bone: bool,
}

impl VertexAttrs {
    pub const POS_ONLY: Self = Self {
        position: true,
        normal: false,
        uv: false,
        tangent: false,
        color: false,
        bone: false,
    };
    pub const POS_NORMAL_UV: Self = Self {
        position: true,
        normal: true,
        uv: true,
        tangent: false,
        color: false,
        bone: false,
    };
    pub const FULL: Self = Self {
        position: true,
        normal: true,
        uv: true,
        tangent: true,
        color: false,
        bone: false,
    };
    pub const SKINNED: Self = Self {
        position: true,
        normal: true,
        uv: true,
        tangent: true,
        color: false,
        bone: true,
    };
}

/// 材质参数（从 .gfx / .mesh 材质块传入 shader 的动态数据）。
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MaterialParams {
    /// 漫反射颜色 tint (RGBA)。
    pub diffuse_tint: [f32; 4],
    /// 高光强度。
    pub specular_intensity: f32,
    /// 高光指数。
    pub specular_power: f32,
    /// alpha cutoff（用于 alpha test）。
    pub alpha_cutoff: f32,
    /// 自发光强度。
    pub emissive: f32,
}

impl Default for MaterialParams {
    fn default() -> Self {
        Self {
            diffuse_tint: [1.0, 1.0, 1.0, 1.0],
            specular_intensity: 0.5,
            specular_power: 32.0,
            alpha_cutoff: 0.5,
            emissive: 0.0,
        }
    }
}

/// Fallback shader：纯漫反射 + 纹理采样。
const FLAT_DIFFUSE_WGSL: &str = r#"
struct Camera {
    view_proj: mat4x4<f32>,
};
@group(0) @binding(0) var<uniform> camera: Camera;

struct Material {
    diffuse_tint: vec4<f32>,
    specular_intensity: f32,
    specular_power: f32,
    alpha_cutoff: f32,
    emissive: f32,
};
@group(1) @binding(0) var<uniform> material: Material;
@group(1) @binding(1) var diffuse_tex: texture_2d<f32>;
@group(1) @binding(2) var diffuse_sampler: sampler;

struct VsIn {
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
};
struct VsOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) normal: vec3<f32>,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var out: VsOut;
    out.clip_pos = camera.view_proj * vec4<f32>(in.pos, 1.0);
    out.uv = in.uv;
    out.normal = in.normal;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let tex_color = textureSample(diffuse_tex, diffuse_sampler, in.uv);
    if tex_color.a < material.alpha_cutoff {
        discard;
    }
    let light = max(dot(normalize(in.normal), normalize(vec3<f32>(0.3, 1.0, 0.5))), 0.2);
    return vec4<f32>(tex_color.rgb * material.diffuse_tint.rgb * light, tex_color.a);
}
"#;

/// 着色器注册表。
pub struct ShaderRegistry {
    /// 原版 shader name (lowercase) → entry
    map: HashMap<&'static str, ShaderEntry>,
    /// fallback entry
    fallback: ShaderEntry,
}

impl ShaderRegistry {
    /// 构造注册表，注册所有已实现的等价 shader。
    pub fn new() -> Self {
        let mut map = HashMap::new();

        // -----------------------------------------------------------------
        // Phase 3.11.3+ 翻译 — 替换 3.2 的简化映射
        // -----------------------------------------------------------------
        map.insert(
            "pdxmap",
            ShaderEntry {
                label: "pdxmap_v2",
                source: super::SHADER_PDXMAP_WGSL,
                vertex_attrs: VertexAttrs::POS_NORMAL_UV,
            },
        );
        map.insert(
            "pdxmesh",
            ShaderEntry {
                label: "pdxmesh_full",
                source: super::SHADER_PDXMESH_WGSL,
                vertex_attrs: VertexAttrs::FULL,
            },
        );
        map.insert(
            "pdxwater",
            ShaderEntry {
                label: "pdxwater_full",
                source: super::SHADER_PDXWATER_WGSL,
                vertex_attrs: VertexAttrs::POS_NORMAL_UV,
            },
        );
        map.insert(
            "river",
            ShaderEntry {
                label: "river_full",
                source: super::SHADER_RIVER_WGSL,
                vertex_attrs: VertexAttrs::POS_NORMAL_UV,
            },
        );
        map.insert(
            "tree",
            ShaderEntry {
                label: "tree_full",
                source: super::SHADER_TREE_FULL_WGSL,
                vertex_attrs: VertexAttrs::POS_NORMAL_UV,
            },
        );
        map.insert(
            "border",
            ShaderEntry {
                label: "border_5lod",
                source: super::SHADER_BORDER_WGSL,
                vertex_attrs: VertexAttrs::POS_NORMAL_UV,
            },
        );
        map.insert(
            "mapname",
            ShaderEntry {
                label: "mapname_vanilla",
                source: super::SHADER_MAPNAME_VANILLA_WGSL,
                vertex_attrs: VertexAttrs::POS_NORMAL_UV,
            },
        );
        map.insert(
            "particle",
            ShaderEntry {
                label: "particle",
                source: super::SHADER_PARTICLE_WGSL,
                vertex_attrs: VertexAttrs::POS_NORMAL_UV,
            },
        );
        map.insert(
            "sky",
            ShaderEntry {
                label: "sky",
                source: super::SHADER_SKY_WGSL,
                vertex_attrs: VertexAttrs::POS_ONLY,
            },
        );
        map.insert(
            "maparrow",
            ShaderEntry {
                label: "maparrow",
                source: super::SHADER_MAPARROW_WGSL,
                vertex_attrs: VertexAttrs::POS_NORMAL_UV,
            },
        );
        map.insert(
            "traderoute",
            ShaderEntry {
                label: "traderoute",
                source: super::SHADER_TRADEROUTE_WGSL,
                vertex_attrs: VertexAttrs::POS_NORMAL_UV,
            },
        );
        map.insert(
            "arrow",
            ShaderEntry {
                label: "arrow",
                source: super::SHADER_ARROW_WGSL,
                vertex_attrs: VertexAttrs::POS_NORMAL_UV,
            },
        );
        map.insert(
            "strait",
            ShaderEntry {
                label: "strait",
                source: super::SHADER_STRAIT_WGSL,
                vertex_attrs: VertexAttrs::POS_NORMAL_UV,
            },
        );
        // 3.12.15.bis.0 — map symbol (unit counter 3D)
        map.insert(
            "mapsymbol",
            ShaderEntry {
                label: "mapsymbol",
                source: super::SHADER_MAPSYMBOL_WGSL,
                vertex_attrs: VertexAttrs::POS_NORMAL_UV,
            },
        );
        // 3.11.12 阴影管线
        map.insert(
            "shadow",
            ShaderEntry {
                label: "shadow_caster",
                source: super::SHADER_SHADOW_WGSL,
                vertex_attrs: VertexAttrs::POS_NORMAL_UV,
            },
        );
        map.insert(
            "shadowblur",
            ShaderEntry {
                label: "shadow_blur",
                source: super::SHADER_SHADOWBLUR_WGSL,
                vertex_attrs: VertexAttrs::POS_ONLY,
            },
        );
        // 3.11.13 后处理链
        map.insert(
            "downsample",
            ShaderEntry {
                label: "downsample",
                source: super::SHADER_DOWNSAMPLE_WGSL,
                vertex_attrs: VertexAttrs::POS_ONLY,
            },
        );
        map.insert(
            "downsample_luminance",
            ShaderEntry {
                label: "downsample_luminance",
                source: super::SHADER_DOWNSAMPLE_LUMINANCE_WGSL,
                vertex_attrs: VertexAttrs::POS_ONLY,
            },
        );
        map.insert(
            "bloom",
            ShaderEntry {
                label: "bloom",
                source: super::SHADER_BLOOM_WGSL,
                vertex_attrs: VertexAttrs::POS_ONLY,
            },
        );
        map.insert(
            "lut_blender",
            ShaderEntry {
                label: "lut_blender",
                source: super::SHADER_LUT_BLENDER_WGSL,
                vertex_attrs: VertexAttrs::POS_ONLY,
            },
        );
        map.insert(
            "restorescene",
            ShaderEntry {
                label: "restorescene",
                source: super::SHADER_RESTORESCENE_WGSL,
                vertex_attrs: VertexAttrs::POS_ONLY,
            },
        );
        map.insert(
            "saturation_slider",
            ShaderEntry {
                label: "saturation_slider",
                source: super::SHADER_SATURATION_SLIDER_WGSL,
                vertex_attrs: VertexAttrs::POS_ONLY,
            },
        );

        // standardfuncsgfx fallback —— 实际上是公共 lib，不应被某个 mesh shader
        // 直接引用；此处保留映射以避免过去的 mesh 文件查找失败。
        map.insert(
            "standardfuncsgfx",
            ShaderEntry {
                label: "stub_lib_marker",
                source: FLAT_DIFFUSE_WGSL,
                vertex_attrs: VertexAttrs::POS_NORMAL_UV,
            },
        );

        // -----------------------------------------------------------------
        // GUI shaders — 3.11.14 把 vanilla 10 个变体合并到 3 个 unified 文件
        // -----------------------------------------------------------------
        let gui_button_names = [
            "buttonstate",
            "buttonstate_blendframes",
            "buttonstate_fade_frames_to_black",
            "buttonstate_linear",
            "buttonstate_nodowneffect",
            "buttonstate_nodownordisableeffect",
            "buttonstate_nontransparent",
            "buttonstate_onlydisable",
            "buttonstate_rendertarget",
            "static_button",
        ];
        for name in gui_button_names {
            map.insert(
                name,
                ShaderEntry {
                    label: "gui_button",
                    source: super::SHADER_GUI_BUTTON_WGSL,
                    vertex_attrs: VertexAttrs::POS_NORMAL_UV,
                },
            );
        }
        let gui_progress_names = [
            "progress",
            "progress_minmax",
            "progress_radial",
            "progress_reverse",
            "progress_startend",
            "circularprogressbar",
        ];
        for name in gui_progress_names {
            map.insert(
                name,
                ShaderEntry {
                    label: "gui_progress",
                    source: super::SHADER_GUI_PROGRESS_WGSL,
                    vertex_attrs: VertexAttrs::POS_NORMAL_UV,
                },
            );
        }
        let gui_special_names = ["maskedflag", "coa_shield", "portrait", "linechart"];
        for name in gui_special_names {
            map.insert(
                name,
                ShaderEntry {
                    label: "gui_special",
                    source: super::SHADER_GUI_SPECIAL_WGSL,
                    vertex_attrs: VertexAttrs::POS_NORMAL_UV,
                },
            );
        }
        // text / color / simple / DebugLines / DebugTexture：UI 系本身有专用 pass，
        // 这里给个 GUI special 的占位以避免 fallback warn
        for name in ["text", "color", "simple", "DebugLines", "DebugTexture"] {
            map.insert(
                name,
                ShaderEntry {
                    label: "gui_special",
                    source: super::SHADER_GUI_SPECIAL_WGSL,
                    vertex_attrs: VertexAttrs::POS_NORMAL_UV,
                },
            );
        }

        let fallback = ShaderEntry {
            label: "flat_diffuse_fallback",
            source: FLAT_DIFFUSE_WGSL,
            vertex_attrs: VertexAttrs::POS_NORMAL_UV,
        };

        Self { map, fallback }
    }

    /// 按原版 shader name 查找等价 wgsl。
    /// name 可以是完整路径 `"gfx/FX/pdxmap.shader"` 或简短名 `"pdxmap"`。
    /// 大小写不敏感。
    pub fn resolve(&self, name: &str) -> &ShaderEntry {
        // 提取 stem：去路径前缀和扩展名
        let stem = extract_shader_stem(name);
        match self.map.get(stem) {
            Some(entry) => entry,
            None => {
                // 静默 warn（生产环境用 log crate；这里 eprintln 占位）
                eprintln!("[shader-rt] no equivalent for '{}', using fallback", name);
                &self.fallback
            }
        }
    }

    /// 查找但不 warn（用于检测是否有等价实现）。
    pub fn has(&self, name: &str) -> bool {
        let stem = extract_shader_stem(name);
        self.map.contains_key(stem)
    }

    /// 已注册的 shader 数量。
    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// fallback shader 源码。
    pub fn fallback(&self) -> &ShaderEntry {
        &self.fallback
    }

    /// 注册一个新的等价 shader（用于后续 Phase 3.3 逐步添加）。
    pub fn register(&mut self, name: &'static str, entry: ShaderEntry) {
        self.map.insert(name, entry);
    }
}

// =============================================================================
// Phase 3.11.1 — `compose_shader` helper
// =============================================================================
//
// 后续 14 节翻译都需要把 `shader_lib.wgsl` 与 `GLOBAL_FRAME_UNIFORM_WGSL` 注入
// 到每个具体 shader 顶部。手撸 `format!("{lib}\n{shader}")` 散落各处不可维护，
// 这里抽出统一接口。
//
// **不实现完整 `#include` 解析**——naga_oil 太复杂、wgsl 语法没有 preprocessor，
// 我们只做"前缀拼接"。如果某 shader 名字里写了 `#include "shader_lib.wgsl"` 注释
// 行，`compose_shader` 会把那一行替换为 lib 文本；否则 lib 默认放在最顶。

/// 标记符 — 在 wgsl 源里出现这一行时 `compose_shader` 会用 [`SHADER_LIB_WGSL`] 替换。
pub const INCLUDE_LIB_DIRECTIVE: &str = "//#include \"shader_lib.wgsl\"";

/// 标记符 — 用 [`crate::global_uniform::GLOBAL_FRAME_UNIFORM_WGSL`] 替换。
pub const INCLUDE_GLOBAL_UNIFORM_DIRECTIVE: &str = "//#include \"global_uniform.wgsl\"";

/// 把 `shader_lib.wgsl` 与 `GlobalFrameUniform` 的 wgsl 文本拼接到 `source` 前面。
///
/// **行为规则**：
///
/// 1. 若 `source` 中出现 `INCLUDE_LIB_DIRECTIVE` / `INCLUDE_GLOBAL_UNIFORM_DIRECTIVE`
///    行，**逐字替换**那一行为对应文本（保持后续行号尽量稳定）。
/// 2. 没出现 → 把两段文本默认 prepend 到 source 顶（`include_lib=true` 时）。
///
/// **不做**：
///
/// - 检查多次 include 重复
/// - 解析嵌套 include
/// - 条件编译 `#ifdef`
///
/// 这些可在后续 phase 用 `naga_oil` 替换。当前阶段够用。
pub fn compose_shader(source: &str, include_lib: bool, include_global_uniform: bool) -> String {
    use crate::global_uniform::GLOBAL_FRAME_UNIFORM_WGSL;
    use crate::SHADER_LIB_WGSL;

    let mut out = String::with_capacity(source.len() + SHADER_LIB_WGSL.len() + 256);

    let has_lib_directive = source.contains(INCLUDE_LIB_DIRECTIVE);
    let has_uniform_directive = source.contains(INCLUDE_GLOBAL_UNIFORM_DIRECTIVE);

    if has_lib_directive || has_uniform_directive {
        // 行级替换路径
        for line in source.lines() {
            let trimmed = line.trim();
            if include_lib && trimmed == INCLUDE_LIB_DIRECTIVE {
                out.push_str(SHADER_LIB_WGSL);
                out.push('\n');
            } else if include_global_uniform && trimmed == INCLUDE_GLOBAL_UNIFORM_DIRECTIVE {
                out.push_str(GLOBAL_FRAME_UNIFORM_WGSL);
                out.push('\n');
            } else {
                out.push_str(line);
                out.push('\n');
            }
        }
    } else {
        // 默认 prepend 路径
        if include_global_uniform {
            out.push_str(GLOBAL_FRAME_UNIFORM_WGSL);
            out.push('\n');
        }
        if include_lib {
            out.push_str(SHADER_LIB_WGSL);
            out.push('\n');
        }
        out.push_str(source);
    }
    out
}

/// 从路径或文件名提取 shader stem。
/// `"gfx/FX/pdxmap.shader"` → `"pdxmap"`
/// `"buttonstate_nodowneffect"` → `"buttonstate_nodowneffect"`
pub fn extract_shader_stem(name: &str) -> &str {
    let name = name.trim();
    // 去路径
    let after_slash = name.rsplit('/').next().unwrap_or(name);
    let after_backslash = after_slash.rsplit('\\').next().unwrap_or(after_slash);
    // 去扩展名
    match after_backslash.rsplit_once('.') {
        Some((stem, _)) => stem,
        None => after_backslash,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_resolves_known_shaders() {
        let reg = ShaderRegistry::new();
        let entry = reg.resolve("pdxmap");
        // Phase 3.11.3 起注册表指向新翻译，label 改为 pdxmap_v2
        assert_eq!(entry.label, "pdxmap_v2");
        assert!(reg.has("pdxmap"));
    }

    #[test]
    fn registry_resolves_with_path_and_extension() {
        let reg = ShaderRegistry::new();
        let entry = reg.resolve("gfx/FX/pdxmap.shader");
        assert_eq!(entry.label, "pdxmap_v2");
    }

    #[test]
    fn registry_falls_back_on_unknown() {
        let reg = ShaderRegistry::new();
        let entry = reg.resolve("some_mod_custom_shader");
        assert_eq!(entry.label, "flat_diffuse_fallback");
        assert!(!reg.has("some_mod_custom_shader"));
    }

    #[test]
    fn registry_gui_shaders_registered() {
        let reg = ShaderRegistry::new();
        assert!(reg.has("buttonstate"));
        assert!(reg.has("text"));
        assert!(reg.has("maskedflag"));
    }

    #[test]
    fn extract_stem_works() {
        assert_eq!(extract_shader_stem("gfx/FX/pdxmap.shader"), "pdxmap");
        assert_eq!(extract_shader_stem("buttonstate"), "buttonstate");
        assert_eq!(extract_shader_stem("gfx\\FX\\tree.shader"), "tree");
        assert_eq!(extract_shader_stem("pdxmesh.fxh"), "pdxmesh");
    }

    #[test]
    fn material_params_default() {
        let p = MaterialParams::default();
        assert_eq!(p.diffuse_tint, [1.0, 1.0, 1.0, 1.0]);
        assert_eq!(p.alpha_cutoff, 0.5);
    }

    #[test]
    fn material_params_size() {
        // 确保 GPU uniform 对齐
        assert_eq!(std::mem::size_of::<MaterialParams>(), 32);
    }

    // =========================================================================
    // Phase 3.11.1 compose_shader 测试
    // =========================================================================

    #[test]
    fn compose_shader_default_prepends_lib() {
        let user = "@fragment fn user_fs() -> @location(0) vec4<f32> { return vec4<f32>(1.0); }";
        let composed = compose_shader(user, true, false);
        // lib 在前，user 代码在后：用 `@fragment fn user_fs` 作为唯一锚点
        // （shader_lib.wgsl 自己的注释里提到了 "@fragment"，所以不能简单 find("@fragment")）
        assert!(composed.contains("fn to_gamma"));
        assert!(composed.contains("@fragment fn user_fs"));
        assert!(
            composed.find("fn to_gamma").unwrap() < composed.find("@fragment fn user_fs").unwrap()
        );
    }

    #[test]
    fn compose_shader_default_prepends_uniform_then_lib() {
        let user = "fn user_func() -> f32 { return 1.0; }";
        let composed = compose_shader(user, true, true);
        let uniform_pos = composed.find("struct GlobalFrameUniform").unwrap();
        let lib_pos = composed.find("fn to_gamma").unwrap();
        let user_pos = composed.find("fn user_func").unwrap();
        assert!(uniform_pos < lib_pos);
        assert!(lib_pos < user_pos);
    }

    #[test]
    fn compose_shader_inline_directive_replaced() {
        let user = "//#include \"shader_lib.wgsl\"\n\
                    fn user() -> f32 { return to_gamma_scalar(0.5); }";
        let composed = compose_shader(user, true, false);
        assert!(composed.contains("fn to_gamma_scalar"));
        // 替换后原指令不再存在
        assert!(!composed.contains(INCLUDE_LIB_DIRECTIVE));
        assert!(composed.contains("fn user"));
    }

    #[test]
    fn compose_shader_uniform_directive_replaced() {
        let user = "//#include \"global_uniform.wgsl\"\n\
                    @group(0) @binding(0) var<uniform> frame: GlobalFrameUniform;";
        let composed = compose_shader(user, false, true);
        assert!(composed.contains("struct GlobalFrameUniform"));
        assert!(!composed.contains(INCLUDE_GLOBAL_UNIFORM_DIRECTIVE));
    }

    #[test]
    fn compose_shader_disabled_means_no_inject() {
        let user = "fn user() -> f32 { return 1.0; }";
        let composed = compose_shader(user, false, false);
        assert!(!composed.contains("fn to_gamma"));
        assert!(!composed.contains("struct GlobalFrameUniform"));
        assert_eq!(composed.trim(), user.trim());
    }

    #[test]
    fn compose_shader_idempotent_when_no_directive() {
        // 同样输入应得同样输出
        let user = "fn x() -> f32 { return 1.0; }";
        let a = compose_shader(user, true, true);
        let b = compose_shader(user, true, true);
        assert_eq!(a, b);
    }
}
