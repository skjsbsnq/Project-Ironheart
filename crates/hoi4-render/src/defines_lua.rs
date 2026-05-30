//! Phase 3.11.15 — `defines.lua` 镜像扫描器 + CI 校验。
//!
//! 工作流：
//!
//! 1. 启动期 / CI 时跑 [`scan_defines_lua`] 解析 vanilla `00_graphics.lua`
//!    与 `00_defines.lua`，提取所有形如 `KEY = value,` 的顶层 entry。
//! 2. 用 [`crate::defines`] 已镜像的常量名做 cross-check：
//!    - vanilla 有但本项目缺 → 加到 [`MissingMirror`] 列表（启动 banner 列出，
//!      CI 设阈值 `< 5` 通过）
//!    - 本项目有但 vanilla 已不再含 → 加到 [`StaleMirror`] 列表（提示删除）
//! 3. 把扫描结果用文本 diff 形式输出（便于人工跟进 / 写 issue）。
//!
//! ## 不解析 lua 表达式
//!
//! 本扫描器**不**用真的 lua 解释器（项目 `lua_defines` crate 是单独的解析器，
//! 与渲染 const 镜像无关），只用正则 / 字符串扫描提取 `IDENT = NUMERIC` 模式。
//! 数值不参与比对，只比较"键的存在性"。完整数值校验留给 `defines.rs::tests`
//! 静态校验。

use std::collections::BTreeSet;

/// 与 vanilla 的差异报告。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DefinesDiff {
    /// vanilla 有，本项目 [`crate::defines`] 没镜像。需要补。
    pub missing_in_project: Vec<String>,
    /// 本项目镜像了，但 vanilla 已删（或从未有过）。需要清理。
    pub stale_in_project: Vec<String>,
    /// 共有的常量数。
    pub shared_count: usize,
}

impl DefinesDiff {
    /// 是否完全一致。
    pub fn is_empty(&self) -> bool {
        self.missing_in_project.is_empty() && self.stale_in_project.is_empty()
    }

    /// 用文本格式打印差异。
    pub fn report(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "[defines_lua] {} shared, {} missing in project, {} stale in project\n",
            self.shared_count,
            self.missing_in_project.len(),
            self.stale_in_project.len(),
        ));
        if !self.missing_in_project.is_empty() {
            out.push_str("\n  Missing in project (add to defines.rs):\n");
            for k in &self.missing_in_project {
                out.push_str(&format!("    - {}\n", k));
            }
        }
        if !self.stale_in_project.is_empty() {
            out.push_str("\n  Stale in project (consider removing):\n");
            for k in &self.stale_in_project {
                out.push_str(&format!("    - {}\n", k));
            }
        }
        out
    }
}

/// 扫描 vanilla lua 文件源（以字符串传入）→ 顶层键名集合。
///
/// 只识别形如 `^\s*IDENT\s*=` 的行（不解析嵌套表 / 复杂表达式 — vanilla
/// `00_graphics.lua` 顶层都是平的，足够）。
pub fn extract_lua_keys(src: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for line in src.lines() {
        // 跳过注释
        let line = line.split("--").next().unwrap_or(line).trim();
        if line.is_empty() || line.starts_with("--") {
            continue;
        }
        // 找第一个 `=`（注意 `==` 不算 — 但 vanilla 顶层不会写 ==）
        if let Some(eq_pos) = line.find('=') {
            let key_part = line[..eq_pos].trim();
            // 去 trailing 逗号 / 分号
            let key = key_part.trim_end_matches(|c: char| !c.is_alphanumeric() && c != '_');
            if !key.is_empty()
                && key.chars().all(|c| c.is_alphanumeric() || c == '_')
                && key
                    .chars()
                    .next()
                    .map(|c| c.is_alphabetic() || c == '_')
                    .unwrap_or(false)
            {
                out.insert(key.to_string());
            }
        }
    }
    out
}

/// 我们关心的 vanilla 渲染常量子集（白名单）——shader 实际引用的那一批。
/// 与 [`crate::defines`] 的 `pub const` 名应**完全一致**（除大小写约定）。
pub const PROJECT_MIRRORED_KEYS: &[&str] = &[
    "CAMERA_MIN_HEIGHT",
    "CAMERA_MAX_HEIGHT",
    "LIGHT_DIRECTION_X",
    "LIGHT_DIRECTION_Y",
    "LIGHT_DIRECTION_Z",
    "LIGHT_SHADOW_DIRECTION_X",
    "LIGHT_SHADOW_DIRECTION_Y",
    "LIGHT_SHADOW_DIRECTION_Z",
    "LIGHT_HDR_RANGE",
    "BORDER_WIDTH",
    "PORT_SHIP_OFFSET",
    "SHIP_IN_PORT_SCALE",
    "GMT_OFFSET",
    "DAY_NIGHT_FEATHER",
    "SOUTH_POLE_OFFSET",
    "NORTH_POLE_OFFSET",
    "PROVINCE_BORDER_FADE_NEAR",
    "PROVINCE_BORDER_FADE_FAR",
    "STATE_BORDER_FADE_NEAR",
    "STATE_BORDER_FADE_FAR",
    "DRAW_COUNTRY_NAMES_CUTOFF",
];

/// 从 vanilla lua 字符串得到 [`DefinesDiff`]。
pub fn diff_against_vanilla(vanilla_src: &str) -> DefinesDiff {
    let vanilla_keys = extract_lua_keys(vanilla_src);
    // 只比对项目关心的那一批（PROJECT_MIRRORED_KEYS）+ vanilla 中存在的**渲染相关
    // 子集**（用前缀启发式：包含 LIGHT/CAMERA/BORDER/MAP/SHADOW/FOG/COLOR 等）
    let render_relevant: BTreeSet<String> = vanilla_keys
        .into_iter()
        .filter(|k| is_render_relevant_key(k))
        .collect();

    let project: BTreeSet<String> = PROJECT_MIRRORED_KEYS
        .iter()
        .map(|s| s.to_string())
        .collect();

    let missing: Vec<String> = render_relevant.difference(&project).cloned().collect();
    let stale: Vec<String> = project.difference(&render_relevant).cloned().collect();
    let shared = render_relevant.intersection(&project).count();

    DefinesDiff {
        missing_in_project: missing,
        stale_in_project: stale,
        shared_count: shared,
    }
}

/// 启发式：哪些 lua 常量是"渲染相关"的，配得上进 [`crate::defines`] 镜像。
fn is_render_relevant_key(k: &str) -> bool {
    let prefixes = [
        "CAMERA_",
        "LIGHT_",
        "SHADOW_",
        "BORDER_",
        "FOG_",
        "FOW_",
        "GB_",
        "RIM_",
        "SNOW_",
        "ICE_",
        "WATER_",
        "MAP_",
        "TERRAIN_",
        "TREE_",
        "SPECULAR_",
        "PORT_",
        "SHIP_",
        "SEC_",
        "MUD_",
        "CITY_",
        "CLOUD_",
        "PROVINCE_BORDER_",
        "STATE_BORDER_",
        "DRAW_COUNTRY_NAMES",
        "GMT_",
        "DAY_NIGHT_",
        "SOUTH_POLE_",
        "NORTH_POLE_",
        "GLOBE_",
    ];
    prefixes.iter().any(|p| k.starts_with(p))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_simple_lua_assignments() {
        let src = r"
            -- comment line
            FOO_BAR = 1.5,
            BAZ = 'string',
            QUX = { 1, 2, 3 },  -- inline comment
        ";
        let keys = extract_lua_keys(src);
        assert!(keys.contains("FOO_BAR"));
        assert!(keys.contains("BAZ"));
        assert!(keys.contains("QUX"));
    }

    #[test]
    fn ignores_comment_lines() {
        let src = "-- CAMERA_MIN_HEIGHT = 50.0,\nCAMERA_MAX_HEIGHT = 3000.0,";
        let keys = extract_lua_keys(src);
        assert!(!keys.contains("CAMERA_MIN_HEIGHT"));
        assert!(keys.contains("CAMERA_MAX_HEIGHT"));
    }

    #[test]
    fn render_relevant_filter_works() {
        assert!(is_render_relevant_key("CAMERA_MIN_HEIGHT"));
        assert!(is_render_relevant_key("LIGHT_SHADOW_DIRECTION_X"));
        assert!(is_render_relevant_key("BORDER_WIDTH"));
        assert!(is_render_relevant_key("FOG_BEGIN"));
        // gameplay 常量应被过滤
        assert!(!is_render_relevant_key("DAYS_IN_MONTH"));
        assert!(!is_render_relevant_key("FACTORY_OUTPUT"));
    }

    #[test]
    fn diff_detects_missing_and_stale() {
        // vanilla 有 CAMERA_MIN_HEIGHT (在白名单) + FOG_BEGIN（白名单没列）+
        // CLOUD_SHADOW_FACTOR（render-relevant 但不在白名单）
        let vanilla = r"
            CAMERA_MIN_HEIGHT = 50.0,
            FOG_BEGIN = 1.0,
            CLOUD_SHADOW_FACTOR = 0.18,
            DAYS_IN_MONTH = 30,
        ";
        let diff = diff_against_vanilla(vanilla);
        // FOG_BEGIN 与 CLOUD_SHADOW_FACTOR 是 render-relevant 但 PROJECT_MIRRORED_KEYS 没有
        assert!(diff.missing_in_project.contains(&"FOG_BEGIN".to_string()));
        assert!(diff
            .missing_in_project
            .contains(&"CLOUD_SHADOW_FACTOR".to_string()));
        // CAMERA_MIN_HEIGHT 双方都有
        assert!(!diff
            .missing_in_project
            .contains(&"CAMERA_MIN_HEIGHT".to_string()));
        // gameplay 常量不应出现在差异里
        assert!(!diff
            .missing_in_project
            .contains(&"DAYS_IN_MONTH".to_string()));
    }

    #[test]
    fn diff_report_format_stable() {
        let diff = DefinesDiff {
            missing_in_project: vec!["FOO".to_string()],
            stale_in_project: vec!["BAR".to_string()],
            shared_count: 5,
        };
        let report = diff.report();
        assert!(report.contains("5 shared"));
        assert!(report.contains("Missing in project"));
        assert!(report.contains("FOO"));
        assert!(report.contains("Stale in project"));
        assert!(report.contains("BAR"));
    }

    #[test]
    fn empty_diff_is_clean() {
        let diff = DefinesDiff::default();
        assert!(diff.is_empty());
    }
}
