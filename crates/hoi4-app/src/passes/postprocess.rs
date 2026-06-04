//! Phase 3.12.2 — 后处理链。
//!
//! 把 3.11.13 翻译的 6 个 wgsl（bloom / downsample / downsample_luminance /
//! lut_blender / restorescene / saturation_slider）串成完整链，接到 3.12.1
//! 建好的 [`super::HdrTarget`] 上。
//!
//! ## 链结构（自上而下）
//!
//! ```text
//!  HDR (W×H, RGBA16Float)
//!   ├─► bloom_bright (W/2×H/2, bright-pass + 9-tap blur)
//!   │      └─► downsample×3 (1/4 → 1/8 → 1/16, RGBA16Float, 13-tap Karis)
//!   │
//!   ├─► lum_log_reduction (W/16, R16Float, 9-tap log-luminance)
//!   │      └─► lum_avg_reduction×N (R16Float, 几何缩小到 1×1)
//!   │
//!   └─► restorescene (HDR + bloom_lvl4 + lum_1x1 → swap-chain LDR)
//!          │  ACES tonemap + 自动曝光 + bloom 叠 + vignette
//!          ▼
//!         (saturation_slider / lut 暂跳过；可在设置面板再接)
//! ```
//!
//! ## 与 sRGB 交换链的契合
//!
//! `restorescene_live.wgsl`（本模块内嵌的修改版）**不**在 fragment 末尾做手工
//! `pow(x, 1/2.2)` —— 我们写到 `Bgra8UnormSrgb`，硬件自动 sRGB 编码即可。**保留** wgsl
//! 原版翻译 `crates/hoi4-render/src/translations/restorescene.wgsl` 不动作为参考。
//!
//! ## 自动曝光
//!
//! 用一条 GPU-only 的多级 reduction：HDR → 1×1 R16Float `lum_tex`。restorescene
//! 直接 `textureSample(lum_tex, ..., (0.5, 0.5))` 取出 `avg_log_lum`，无 CPU 回读。
//!
//! ## 简化版 / 完整版切换
//!
//! `PostProcessMode::Off` → 只做简化 blit（[`super::SimpleBlitPass`] 取代）。
//! `PostProcessMode::Full` → 完整链。F4 / 设置面板可切换。

use crate::passes::HDR_FORMAT;

use std::collections::{BTreeSet, HashMap};
use std::fmt::Write as _;
use std::path::Path;

use hoi4_assets::{AssetDb, FsAssetDb, PostEffectValues, PostEffectVolumeIndex, TgaImage};
use hoi4_paths::PathConfig;

const POST_PROCESS_LUT_KEY_COUNT: usize = 10;
const STANDARD_TONEMAP_MIDDLE_GREY: f32 = 0.55;
const WATER_LUT_FRAME_THRESHOLD: f32 = 0.55;
const CAMERA_FAR_LUT_THRESHOLD: f32 = 0.72;
const CAMERA_MID_LUT_THRESHOLD: f32 = 0.36;
const WINTER_LUT_THRESHOLD: f32 = 0.55;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostProcessDebugView {
    Final,
    HdrRaw,
    BloomOnly,
    AvgLuminance,
    TonemapBefore,
    TonemapOnly,
    LutBefore,
    LutAfter,
}

impl PostProcessDebugView {
    pub const ALL: [Self; 8] = [
        Self::Final,
        Self::HdrRaw,
        Self::BloomOnly,
        Self::AvgLuminance,
        Self::TonemapBefore,
        Self::TonemapOnly,
        Self::LutBefore,
        Self::LutAfter,
    ];

    pub fn next(self) -> Self {
        let idx = Self::ALL.iter().position(|view| *view == self).unwrap_or(0);
        Self::ALL[(idx + 1) % Self::ALL.len()]
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Final => "final",
            Self::HdrRaw => "hdr_raw",
            Self::BloomOnly => "bloom_only",
            Self::AvgLuminance => "avg_luminance",
            Self::TonemapBefore => "tonemap_before",
            Self::TonemapOnly => "tonemap_after",
            Self::LutBefore => "lut_before",
            Self::LutAfter => "lut_after",
        }
    }

    const fn as_shader_value(self) -> f32 {
        match self {
            Self::Final => 0.0,
            Self::HdrRaw => 1.0,
            Self::BloomOnly => 2.0,
            Self::AvgLuminance => 3.0,
            Self::TonemapBefore => 4.0,
            Self::TonemapOnly => 5.0,
            Self::LutBefore => 6.0,
            Self::LutAfter => 7.0,
        }
    }
}

impl Default for PostProcessDebugView {
    fn default() -> Self {
        Self::Final
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PostProcessCalibration {
    pub bloom_bright_threshold: f32,
    pub bloom_prefilter_strength: f32,
    pub middle_grey: f32,
    pub exposure_min: f32,
    pub exposure_max: f32,
    pub exposure_bias: f32,
    pub uncharted_white_point: f32,
    pub final_bloom_strength: f32,
    pub lut_strength: f32,
    pub saturation: f32,
    pub hsv_hue_shift: f32,
    pub hsv_saturation: f32,
    pub hsv_value: f32,
    pub color_balance: [f32; 3],
    pub bloom_debug_gain: f32,
}

impl PostProcessCalibration {
    pub const fn phase10_vanilla() -> Self {
        Self {
            bloom_bright_threshold: 1.05,
            bloom_prefilter_strength: 0.68,
            middle_grey: STANDARD_TONEMAP_MIDDLE_GREY,
            exposure_min: 0.125,
            exposure_max: 8.0,
            exposure_bias: 1.02,
            uncharted_white_point: 11.2,
            final_bloom_strength: 0.14,
            lut_strength: 0.35,
            saturation: 0.96,
            hsv_hue_shift: 0.0,
            hsv_saturation: 0.94,
            hsv_value: 1.02,
            color_balance: [0.004, 0.002, -0.004],
            bloom_debug_gain: 4.0,
        }
    }

    pub const fn phase9_high() -> Self {
        Self::phase10_vanilla()
    }

    pub fn summary(self) -> String {
        format!(
            "restore=uncharted aces=off exposure=[{:.3},{:.1}] middle_grey={:.2} bias={:.2} white={:.1} bloom={:.2}/{:.2} lut={:.2} sat={:.2} hsv={:.2}/{:.2}/{:.2} balance={:.2},{:.2},{:.2}",
            self.exposure_min,
            self.exposure_max,
            self.middle_grey,
            self.exposure_bias,
            self.uncharted_white_point,
            self.bloom_bright_threshold,
            self.final_bloom_strength,
            self.lut_strength,
            self.saturation,
            self.hsv_hue_shift,
            self.hsv_saturation,
            self.hsv_value,
            self.color_balance[0],
            self.color_balance[1],
            self.color_balance[2],
        )
    }
}

impl Default for PostProcessCalibration {
    fn default() -> Self {
        Self::phase10_vanilla()
    }
}

/// 后处理执行模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostProcessLutKey {
    DefaultDay,
    DefaultNight,
    MidDistanceDay,
    MidDistanceNight,
    FarDistanceDay,
    FarDistanceNight,
    WaterDay,
    WaterNight,
    WinterDay,
    WinterNight,
}

impl PostProcessLutKey {
    pub const fn ordered() -> &'static [Self] {
        &[
            Self::DefaultDay,
            Self::DefaultNight,
            Self::MidDistanceDay,
            Self::MidDistanceNight,
            Self::FarDistanceDay,
            Self::FarDistanceNight,
            Self::WaterDay,
            Self::WaterNight,
            Self::WinterDay,
            Self::WinterNight,
        ]
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DefaultDay => "default_day",
            Self::DefaultNight => "default_night",
            Self::MidDistanceDay => "mid_distance_day",
            Self::MidDistanceNight => "mid_distance_night",
            Self::FarDistanceDay => "max_distance_day",
            Self::FarDistanceNight => "max_distance_night",
            Self::WaterDay => "blue_water_day",
            Self::WaterNight => "blue_water_night",
            Self::WinterDay => "winter_day",
            Self::WinterNight => "winter_night",
        }
    }

    pub const fn values_name(self) -> &'static str {
        match self {
            Self::DefaultDay => "default",
            Self::DefaultNight => "default_night",
            Self::MidDistanceDay => "mid_distance",
            Self::MidDistanceNight => "mid_distance_night",
            Self::FarDistanceDay => "max_distance",
            Self::FarDistanceNight => "max_distance_night",
            Self::WaterDay => "blue_water",
            Self::WaterNight => "blue_water_night",
            Self::WinterDay => "winter_values_day",
            Self::WinterNight => "winter_values_night",
        }
    }

    pub const fn selection_rule(self) -> &'static str {
        match self {
            Self::DefaultDay => "day, land, close camera, winter_factor <= 0.55",
            Self::DefaultNight => {
                "night blend target for land, close camera, winter_factor <= 0.55"
            }
            Self::MidDistanceDay => "day, land, 0.36 < camera_distance_t <= 0.72",
            Self::MidDistanceNight => {
                "night blend target for land, 0.36 < camera_distance_t <= 0.72"
            }
            Self::FarDistanceDay => "day, land, camera_distance_t > 0.72",
            Self::FarDistanceNight => "night blend target for land, camera_distance_t > 0.72",
            Self::WaterDay => "day, water_factor > 0.55",
            Self::WaterNight => "night blend target for water_factor > 0.55",
            Self::WinterDay => "day, land, close camera, winter_factor > 0.55",
            Self::WinterNight => "night blend target for land, close camera, winter_factor > 0.55",
        }
    }

    pub const fn tonemap_middle_grey(self) -> f32 {
        match self {
            Self::DefaultDay
            | Self::DefaultNight
            | Self::MidDistanceDay
            | Self::MidDistanceNight
            | Self::FarDistanceDay => 0.55,
            Self::FarDistanceNight => 0.65,
            Self::WaterDay => 0.50,
            Self::WaterNight => 0.40,
            Self::WinterDay | Self::WinterNight => 0.80,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ColorCubeImage {
    pub source_path: String,
    pub size: u32,
    pub pixels: Vec<u8>,
}

impl ColorCubeImage {
    pub fn from_tga(source_path: impl Into<String>, image: TgaImage) -> Result<Self, String> {
        if image.width != 1024 || image.height != 32 {
            return Err(format!(
                "unsupported ColorCube dimensions {}x{}; expected 1024x32 flattened 32x32x32",
                image.width, image.height
            ));
        }
        let expected_len = (image.width * image.height * 4) as usize;
        if image.pixels.len() != expected_len {
            return Err(format!(
                "ColorCube pixel data has {} bytes, expected {}",
                image.pixels.len(),
                expected_len
            ));
        }
        Ok(Self {
            source_path: source_path.into(),
            size: image.height,
            pixels: image.pixels,
        })
    }
}

#[derive(Debug, Clone)]
pub struct ColorCubeSource {
    pub cubes: Vec<ColorCubeImage>,
    pub bindings: [usize; POST_PROCESS_LUT_KEY_COUNT],
    pub warnings: Vec<String>,
}

impl ColorCubeSource {
    pub fn identity() -> Self {
        Self {
            cubes: vec![identity_color_cube_image()],
            bindings: [0; POST_PROCESS_LUT_KEY_COUNT],
            warnings: vec!["using identity ColorCube fallback".to_string()],
        }
    }

    pub fn load_from_path_config(path_cfg: &PathConfig) -> Self {
        let db = FsAssetDb::new(path_cfg.clone());
        Self::load_from_db(&db)
    }

    pub fn load_from_db(db: &impl AssetDb) -> Self {
        let mut warnings = Vec::new();
        let index = match PostEffectVolumeIndex::load(db) {
            Ok(index) => index,
            Err(err) => {
                warnings.push(err.to_string());
                return Self::identity_with_warnings(warnings);
            }
        };
        Self::load_from_index(db, &index, warnings)
    }

    fn load_from_index(
        db: &impl AssetDb,
        index: &PostEffectVolumeIndex,
        mut warnings: Vec<String>,
    ) -> Self {
        let mut by_path: HashMap<String, usize> = HashMap::new();
        let mut cubes = Vec::new();
        let mut binding_paths = Vec::with_capacity(POST_PROCESS_LUT_KEY_COUNT);
        let mut load_paths = BTreeSet::new();

        for key in PostProcessLutKey::ordered() {
            let path = index
                .resolved_lut_for_values(key.values_name())
                .or_else(|| index.default_lut())
                .map(normalize_lut_path);
            if let Some(path) = &path {
                load_paths.insert(path.clone());
            }
            binding_paths.push(path);
        }

        for path in index.lut_paths() {
            load_paths.insert(normalize_lut_path(path));
        }

        for path in &load_paths {
            if by_path.contains_key(path) {
                continue;
            }
            match hoi4_assets::load_tga(db, path.as_str()) {
                Ok(image) => match ColorCubeImage::from_tga(path.as_str(), image) {
                    Ok(cube) => {
                        let idx = cubes.len();
                        cubes.push(cube);
                        by_path.insert(path.clone(), idx);
                    }
                    Err(err) => warnings.push(format!("{path}: {err}")),
                },
                Err(err) => warnings.push(
                    err.with_referrer_path("gfx/posteffect_volumes.txt")
                        .to_string(),
                ),
            }
        }

        if cubes.is_empty() {
            return Self::identity_with_warnings(warnings);
        }

        let default_idx = binding_paths
            .first()
            .and_then(|path| path.as_ref())
            .and_then(|path| by_path.get(path).copied())
            .unwrap_or(0);
        let mut bindings = [default_idx; POST_PROCESS_LUT_KEY_COUNT];
        for (idx, path) in binding_paths.iter().enumerate() {
            if let Some(cube_idx) = path.as_ref().and_then(|path| by_path.get(path).copied()) {
                bindings[idx] = cube_idx;
            }
        }

        Self {
            cubes,
            bindings,
            warnings,
        }
    }

    fn identity_with_warnings(mut warnings: Vec<String>) -> Self {
        warnings.push("using identity ColorCube fallback".to_string());
        Self {
            cubes: vec![identity_color_cube_image()],
            bindings: [0; POST_PROCESS_LUT_KEY_COUNT],
            warnings,
        }
    }

    pub fn is_identity_fallback(&self) -> bool {
        self.cubes.len() == 1 && self.cubes[0].source_path == "identity_color_cube"
    }

    pub fn source_summary(&self) -> String {
        if self.is_identity_fallback() {
            return "identity_color_cube".to_string();
        }
        let mut paths: Vec<&str> = self
            .cubes
            .iter()
            .map(|cube| cube.source_path.as_str())
            .collect();
        paths.sort_unstable();
        paths.dedup();
        format!(
            "{} vanilla ColorCube LUTs: {}",
            paths.len(),
            paths.join(", ")
        )
    }
}

pub fn build_posteffect_values_report_json(path_cfg: &PathConfig) -> String {
    let db = FsAssetDb::new(path_cfg.clone());
    build_posteffect_values_report_json_from_db(&db)
}

pub fn build_posteffect_values_report_json_from_db(db: &impl AssetDb) -> String {
    let mut warnings = Vec::new();
    let index = match PostEffectVolumeIndex::load(db) {
        Ok(index) => Some(index),
        Err(err) => {
            warnings.push(err.to_string());
            None
        }
    };
    let color_cube_source = match &index {
        Some(index) => ColorCubeSource::load_from_index(db, index, warnings.clone()),
        None => ColorCubeSource::identity_with_warnings(warnings.clone()),
    };
    warnings.extend(color_cube_source.warnings.iter().cloned());
    warnings.sort();
    warnings.dedup();

    let empty_index = PostEffectVolumeIndex::new();
    let index_ref = index.as_ref().unwrap_or(&empty_index);
    let lut_paths = index_ref.lut_paths();
    let cube_by_path: HashMap<&str, &ColorCubeImage> = color_cube_source
        .cubes
        .iter()
        .map(|cube| (cube.source_path.as_str(), cube))
        .collect();
    let calibration = PostProcessCalibration::phase10_vanilla();

    let mut out = String::new();
    out.push_str("{\n");
    out.push_str("  \"phase\": \"5\",\n");
    out.push_str("  \"kind\": \"posteffect_values_audit\",\n");
    out.push_str("  \"source_inputs\": [\n");
    out.push_str("    \"gfx/posteffect_volumes.txt\",\n");
    out.push_str("    \"tools/vanilla_trace/shader_bindings.json\",\n");
    out.push_str("    \"tools/vanilla_trace/render_passes.json\",\n");
    out.push_str("    \"tools/vanilla_trace/runtime_targets.json\"\n");
    out.push_str("  ],\n");
    let _ = writeln!(out, "  \"index_loaded\": {},", index.is_some());
    let _ = writeln!(
        out,
        "  \"posteffect_values_count\": {},",
        index_ref.values.len()
    );
    let _ = writeln!(
        out,
        "  \"posteffect_height_volume_count\": {},",
        index_ref.height_volumes.len()
    );
    let _ = writeln!(
        out,
        "  \"posteffect_volume_count\": {},",
        index_ref.volumes.len()
    );
    let _ = writeln!(out, "  \"color_cube_lut_count\": {},", lut_paths.len());
    let _ = writeln!(
        out,
        "  \"color_cube_loaded_count\": {},",
        color_cube_source
            .cubes
            .iter()
            .filter(|cube| cube.source_path != "identity_color_cube")
            .count()
    );
    out.push_str("  \"color_cube_layout\": {\n");
    out.push_str("    \"dimensions\": \"1024x32\",\n");
    out.push_str("    \"cube_size\": 32,\n");
    out.push_str("    \"flattened_volume\": \"32x32x32\",\n");
    out.push_str("    \"legacy_16_cube\": false\n");
    out.push_str("  },\n");
    let _ = writeln!(
        out,
        "  \"identity_fallback\": {},",
        color_cube_source.is_identity_fallback()
    );
    out.push_str("  \"responsibility_boundary\": {\n");
    out.push_str("    \"hdr_scene\": \"render passes write linear HDR into Rgba16Float\",\n");
    out.push_str(
        "    \"tonemap\": \"restorescene applies Uncharted2 tonemap after exposure and bloom\",\n",
    );
    out.push_str("    \"lut\": \"restorescene samples vanilla ColorCube TGA layers selected from gfx/posteffect_volumes.txt\",\n");
    out.push_str(
        "    \"captured_dx11_swapchain\": \"R8G8B8A8_UNORM with shader-side gamma pow(1/2.2)\",\n",
    );
    out.push_str("    \"wgpu_surface_policy\": \"prefer Bgra8Unorm/Rgba8Unorm so restorescene follows captured shader-side gamma; sRGB is fallback-only\"\n");
    out.push_str("  },\n");
    out.push_str("  \"restore_order\": [\n");
    out.push_str("    \"MainScene\",\n");
    out.push_str("    \"+ RestoreBloom\",\n");
    out.push_str("    \"* MiddleGrey / AverageLuminance\",\n");
    out.push_str("    \"Uncharted2 tonemap W=11.2\",\n");
    out.push_str("    \"manual pow(1/2.2) only on non-sRGB target path\",\n");
    out.push_str("    \"ColorCube\",\n");
    out.push_str("    \"HSV\",\n");
    out.push_str("    \"ColorBalance\"\n");
    out.push_str("  ],\n");
    out.push_str("  \"restore_scene_bindings\": [\n");
    out.push_str("    { \"vanilla\": \"MainScene\", \"vanilla_slot\": \"t0/s0\", \"project\": \"hdr_tex/hdr_sampler\", \"project_binding\": \"@group(0) @binding(0/1)\" },\n");
    out.push_str("    { \"vanilla\": \"RestoreBloom\", \"vanilla_slot\": \"t1/s1\", \"project\": \"bloom_tex/bloom_sampler\", \"project_binding\": \"@group(0) @binding(2/3)\" },\n");
    out.push_str("    { \"vanilla\": \"AverageLuminanceTexture\", \"vanilla_slot\": \"t3/s3\", \"project\": \"lum_tex/lum_sampler\", \"project_binding\": \"@group(0) @binding(4/5)\" },\n");
    out.push_str("    { \"vanilla\": \"ColorCube\", \"vanilla_slot\": \"t2/s2\", \"project\": \"color_cube_tex/color_cube_sampler\", \"project_binding\": \"@group(0) @binding(6/7)\" }\n");
    out.push_str("  ],\n");
    out.push_str("  \"selection_thresholds\": {\n");
    let _ = writeln!(
        out,
        "    \"water\": \"water_factor > {:.2} selects blue_water day/night\",",
        WATER_LUT_FRAME_THRESHOLD
    );
    let _ = writeln!(
        out,
        "    \"far_distance\": \"camera_distance_t > {:.2} selects max_distance day/night\",",
        CAMERA_FAR_LUT_THRESHOLD
    );
    let _ = writeln!(
        out,
        "    \"mid_distance\": \"camera_distance_t > {:.2} selects mid_distance day/night\",",
        CAMERA_MID_LUT_THRESHOLD
    );
    let _ = writeln!(
        out,
        "    \"winter\": \"winter_factor > {:.2} selects winter_values_day/night for close land\",",
        WINTER_LUT_THRESHOLD
    );
    out.push_str("    \"night\": \"night_factor blends day layer to night layer\"\n");
    out.push_str("  },\n");
    out.push_str("  \"phase_b_runtime_policy\": {\n");
    out.push_str("    \"selection_priority\": [\"water\", \"far_distance\", \"mid_distance\", \"winter\", \"default\"],\n");
    out.push_str("    \"water_factor_source\": \"project screen-sample of province type; explicit non-parity classifier until posteffect_volume water bounds are reconstructed\",\n");
    out.push_str("    \"camera_distance_source\": \"project camera normalized distance using R17/colorcube_lut_selection thresholds\",\n");
    out.push_str("    \"winter_factor_source\": \"disabled/0.0 in default runtime path until gfx/posteffect_volumes.txt posteffect_volume winter classification or trace-backed formula is implemented\",\n");
    out.push_str("    \"winter_lut_values_preserved\": true,\n");
    out.push_str("    \"manual_lut_strength_adjustment\": false,\n");
    out.push_str("    \"manual_middle_grey_adjustment\": false,\n");
    out.push_str("    \"gamma_policy\": {\n");
    out.push_str(
        "      \"captured_dx11\": \"R8G8B8A8_UNORM plus restorescene shader pow(1/2.2)\",\n",
    );
    out.push_str("      \"wgpu_default_target\": \"Bgra8Unorm/Rgba8Unorm when supported; manual pow enabled\",\n");
    out.push_str("      \"wgpu_srgb_fallback_target\": \"manual pow disabled only if the platform exposes no non-sRGB surface format\"\n");
    out.push_str("    }\n");
    out.push_str("  },\n");
    out.push_str("  \"default_calibration\": {\n");
    let _ = writeln!(
        out,
        "    \"lut_strength\": {:.3},",
        calibration.lut_strength
    );
    let _ = writeln!(out, "    \"saturation\": {:.3},", calibration.saturation);
    let _ = writeln!(
        out,
        "    \"hsv_saturation\": {:.3},",
        calibration.hsv_saturation
    );
    let _ = writeln!(out, "    \"hsv_value\": {:.3},", calibration.hsv_value);
    let _ = writeln!(
        out,
        "    \"color_balance\": [{:.3}, {:.3}, {:.3}],",
        calibration.color_balance[0], calibration.color_balance[1], calibration.color_balance[2]
    );
    let _ = writeln!(
        out,
        "    \"exposure_bias\": {:.3},",
        calibration.exposure_bias
    );
    let _ = writeln!(
        out,
        "    \"uncharted_white_point\": {:.1},",
        calibration.uncharted_white_point
    );
    let _ = writeln!(
        out,
        "    \"restore_bloom_strength\": {:.3},",
        calibration.final_bloom_strength
    );
    out.push_str("    \"aces\": false,\n");
    out.push_str("    \"vignette\": false,\n");
    out.push_str("    \"manual_vivid_adjustment\": false\n");
    out.push_str("  },\n");
    out.push_str("  \"runtime_lut_bindings\": [\n");
    for (idx, key) in PostProcessLutKey::ordered().iter().enumerate() {
        let values_name = key.values_name();
        let lut_path = index_ref
            .resolved_lut_for_values(values_name)
            .or_else(|| index_ref.default_lut())
            .map(normalize_lut_path);
        let layer = color_cube_source.bindings[idx];
        let cube = lut_path
            .as_deref()
            .and_then(|path| cube_by_path.get(path).copied());
        out.push_str("    {\n");
        let _ = writeln!(out, "      \"key\": \"{}\",", key.as_str());
        let _ = writeln!(
            out,
            "      \"posteffect_values\": \"{}\",",
            json_escape(values_name)
        );
        let _ = writeln!(
            out,
            "      \"selection_rule\": \"{}\",",
            json_escape(key.selection_rule())
        );
        out.push_str("      \"lut_path\": ");
        write_json_string_option(&mut out, lut_path.as_deref());
        out.push_str(",\n");
        let _ = writeln!(out, "      \"array_layer\": {},", layer);
        let _ = writeln!(out, "      \"loaded\": {},", cube.is_some());
        out.push_str("      \"dimensions\": ");
        match cube {
            Some(cube) => {
                let _ = write!(out, "\"{}x{}\"", cube.size * cube.size, cube.size);
            }
            None => out.push_str("null"),
        }
        out.push_str(",\n");
        out.push_str("      \"effective_values\": ");
        write_posteffect_values_json(
            &mut out,
            index_ref.effective_values(values_name).as_ref(),
            6,
        );
        out.push('\n');
        out.push_str("    }");
        out.push_str(comma(idx + 1, PostProcessLutKey::ordered().len()));
        out.push('\n');
    }
    out.push_str("  ],\n");
    out.push_str("  \"color_cube_luts\": [\n");
    for (idx, path) in lut_paths.iter().enumerate() {
        let cube = cube_by_path.get(path.as_str()).copied();
        out.push_str("    {\n");
        let _ = writeln!(out, "      \"path\": \"{}\",", json_escape(path));
        let _ = writeln!(out, "      \"loaded\": {},", cube.is_some());
        out.push_str("      \"dimensions\": ");
        match cube {
            Some(cube) => {
                let _ = write!(out, "\"{}x{}\"", cube.size * cube.size, cube.size);
            }
            None => out.push_str("null"),
        }
        out.push('\n');
        out.push_str("    }");
        out.push_str(comma(idx + 1, lut_paths.len()));
        out.push('\n');
    }
    out.push_str("  ],\n");
    out.push_str("  \"warnings\": ");
    write_json_string_array(&mut out, &warnings, 2);
    out.push('\n');
    out.push_str("}\n");
    out
}

fn write_posteffect_values_json(
    out: &mut String,
    values: Option<&PostEffectValues>,
    indent: usize,
) {
    let Some(values) = values else {
        out.push_str("null");
        return;
    };
    let pad = " ".repeat(indent);
    let _ = writeln!(out, "{{");
    let _ = writeln!(out, "{pad}  \"name\": \"{}\",", json_escape(&values.name));
    let _ = write!(out, "{pad}  \"inherit\": ");
    write_json_string_option(out, values.inherit.as_deref());
    out.push_str(",\n");
    let _ = write!(out, "{pad}  \"lut\": ");
    write_json_string_option(out, values.lut.as_deref());
    out.push_str(",\n");
    write_f32_option_json(
        out,
        "tonemap_middlegrey",
        values.tonemap_middlegrey,
        indent + 2,
        true,
    );
    write_f32_option_json(out, "bloom_width", values.bloom_width, indent + 2, true);
    write_f32_option_json(out, "bloom_scale", values.bloom_scale, indent + 2, true);
    write_f32_option_json(
        out,
        "bright_threshold",
        values.bright_threshold,
        indent + 2,
        true,
    );
    write_f32_option_json(
        out,
        "hdr_min_adjustment",
        values.hdr_min_adjustment,
        indent + 2,
        true,
    );
    write_f32_option_json(
        out,
        "hdr_max_adjustment",
        values.hdr_max_adjustment,
        indent + 2,
        false,
    );
    let _ = write!(out, "{pad}}}");
}

fn write_f32_option_json(
    out: &mut String,
    key: &str,
    value: Option<f32>,
    indent: usize,
    trailing_comma: bool,
) {
    let pad = " ".repeat(indent);
    let _ = write!(out, "{pad}\"{}\": ", key);
    match value {
        Some(value) => {
            let _ = write!(out, "{value:.6}");
        }
        None => out.push_str("null"),
    }
    if trailing_comma {
        out.push(',');
    }
    out.push('\n');
}

fn write_json_string_option(out: &mut String, value: Option<&str>) {
    match value {
        Some(value) => {
            out.push('"');
            out.push_str(&json_escape(value));
            out.push('"');
        }
        None => out.push_str("null"),
    }
}

fn write_json_string_array(out: &mut String, values: &[String], indent: usize) {
    if values.is_empty() {
        out.push_str("[]");
        return;
    }
    let pad = " ".repeat(indent);
    out.push_str("[\n");
    for (idx, value) in values.iter().enumerate() {
        let _ = write!(out, "{pad}  \"{}\"", json_escape(value));
        out.push_str(comma(idx + 1, values.len()));
        out.push('\n');
    }
    let _ = write!(out, "{pad}]");
}

fn comma(done: usize, total: usize) -> &'static str {
    if done < total {
        ","
    } else {
        ""
    }
}

fn json_escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if ch.is_control() => {
                let _ = write!(out, "\\u{:04x}", ch as u32);
            }
            ch => out.push(ch),
        }
    }
    out
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct PostProcessLutSelection {
    pub camera_distance_t: f32,
    pub night_factor: f32,
    pub water_factor: f32,
    pub winter_factor: f32,
}

impl PostProcessLutSelection {
    pub fn key(self, night: bool) -> PostProcessLutKey {
        if self.water_factor > WATER_LUT_FRAME_THRESHOLD {
            return if night {
                PostProcessLutKey::WaterNight
            } else {
                PostProcessLutKey::WaterDay
            };
        }
        if self.camera_distance_t > CAMERA_FAR_LUT_THRESHOLD {
            return if night {
                PostProcessLutKey::FarDistanceNight
            } else {
                PostProcessLutKey::FarDistanceDay
            };
        }
        if self.camera_distance_t > CAMERA_MID_LUT_THRESHOLD {
            return if night {
                PostProcessLutKey::MidDistanceNight
            } else {
                PostProcessLutKey::MidDistanceDay
            };
        }
        if self.winter_factor > WINTER_LUT_THRESHOLD {
            return if night {
                PostProcessLutKey::WinterNight
            } else {
                PostProcessLutKey::WinterDay
            };
        }
        if night {
            PostProcessLutKey::DefaultNight
        } else {
            PostProcessLutKey::DefaultDay
        }
    }

    pub fn tonemap_middle_grey(self) -> f32 {
        let day = self.key(false).tonemap_middle_grey();
        let night = self.key(true).tonemap_middle_grey();
        day + (night - day) * self.night_factor.clamp(0.0, 1.0)
    }

    pub fn audit_summary(self) -> String {
        format!(
            "lut_day={} lut_night={} night={:.2} middle_grey={:.2} factors(camera={:.2},water={:.2},winter={:.2})",
            self.key(false).as_str(),
            self.key(true).as_str(),
            self.night_factor.clamp(0.0, 1.0),
            self.tonemap_middle_grey(),
            self.camera_distance_t,
            self.water_factor,
            self.winter_factor,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostProcessMode {
    /// 简化模式：跳过完整后处理链，由 [`super::SimpleBlitPass`] 接管。
    Off,
    /// 完整后处理链。
    Full,
}

impl Default for PostProcessMode {
    fn default() -> Self {
        // Phase 3.12.2 默认开启 Full；用户可通过设置面板（未来）回退到 Off。
        Self::Full
    }
}

// ─── 内嵌 / 改写的 shader 源 ──────────────────────────────────────────────────

/// 全屏三角形 + 9-tap bright-pass。源自 `translations/bloom.wgsl`，原样使用。
const BLOOM_BRIGHT_WGSL: &str = include_str!("../../../hoi4-render/src/translations/bloom.wgsl");

/// 13-tap Karis-下采样。源自 `translations/downsample.wgsl`。
const DOWNSAMPLE_WGSL: &str = include_str!("../../../hoi4-render/src/translations/downsample.wgsl");

/// 9-tap log-luminance 第一级。源自 `translations/downsample_luminance.wgsl`。
const LUM_LOG_WGSL: &str =
    include_str!("../../../hoi4-render/src/translations/downsample_luminance.wgsl");

/// 9-tap 普通平均（第 2..N 级 luminance reduction 用，本模块内嵌 — 翻译目录的
/// log 版本只适合第 1 级；后续级输入已经是标量 log-luminance 了）。
const LUM_AVG_WGSL: &str = r#"
@group(0) @binding(0) var src_tex: texture_2d<f32>;
@group(0) @binding(1) var src_sampler: sampler;

struct Params {
    inv_src_size: vec2<f32>,
    _pad: vec2<f32>,
};
@group(0) @binding(2) var<uniform> p: Params;

struct VsOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vid: u32) -> VsOut {
    var out: VsOut;
    let uv = vec2<f32>(f32((vid << 1u) & 2u), f32(vid & 2u));
    out.clip_pos = vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0);
    out.uv = vec2<f32>(uv.x, 1.0 - uv.y);
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let q = p.inv_src_size;
    var sum = 0.0;
    let offsets = array<vec2<f32>, 9>(
        vec2<f32>(-1.0, -1.0), vec2<f32>( 0.0, -1.0), vec2<f32>( 1.0, -1.0),
        vec2<f32>(-1.0,  0.0), vec2<f32>( 0.0,  0.0), vec2<f32>( 1.0,  0.0),
        vec2<f32>(-1.0,  1.0), vec2<f32>( 0.0,  1.0), vec2<f32>( 1.0,  1.0),
    );
    for (var i = 0; i < 9; i = i + 1) {
        sum = sum + textureSample(src_tex, src_sampler, in.uv + offsets[i] * q).r;
    }
    return vec4<f32>(sum / 9.0, 0.0, 0.0, 1.0);
}
"#;

/// 修改版 restorescene：避免双 sRGB（输出到 sRGB 交换链时跳过 pow），并通过
/// 1×1 `lum_tex` 提供 `avg_log_lum` 而非 uniform。其余 ACES + bloom + vignette
/// 与翻译目录原版逐行等价。
const RESTORESCENE_LIVE_WGSL: &str = r#"
@group(0) @binding(0) var hdr_tex: texture_2d<f32>;
@group(0) @binding(1) var hdr_sampler: sampler;
@group(0) @binding(2) var bloom_tex: texture_2d<f32>;
@group(0) @binding(3) var bloom_sampler: sampler;
@group(0) @binding(4) var lum_tex: texture_2d<f32>;
@group(0) @binding(5) var lum_sampler: sampler;

struct RestoreParams {
    middle_grey: f32,
    bloom_strength: f32,
    vignette_strength: f32,
    /// 0 = 输出非 sRGB（应用手工 gamma），1 = 输出 sRGB（跳过手工 gamma，硬件代劳）
    srgb_target: f32,
    exposure_min: f32,
    exposure_max: f32,
    aces_input_scale: f32,
    saturation: f32,
    debug_view: f32,
    bloom_debug_gain: f32,
    _pad0: vec2<f32>,
    center_uv: vec2<f32>,
    _pad: vec2<f32>,
};
@group(0) @binding(6) var<uniform> rp: RestoreParams;

struct VsOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vid: u32) -> VsOut {
    var out: VsOut;
    let uv = vec2<f32>(f32((vid << 1u) & 2u), f32(vid & 2u));
    out.clip_pos = vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0);
    out.uv = vec2<f32>(uv.x, 1.0 - uv.y);
    return out;
}

fn aces_tonemap(x: vec3<f32>) -> vec3<f32> {
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    return clamp((x * (a * x + b)) / (x * (c * x + d) + e), vec3<f32>(0.0), vec3<f32>(1.0));
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let scene = textureSample(hdr_tex, hdr_sampler, in.uv).rgb;
    let bloom = textureSample(bloom_tex, bloom_sampler, in.uv).rgb;
    let combined = scene + bloom * rp.bloom_strength;

    // 自动曝光：从 1×1 lum_tex 取场景平均 log-luminance，但 clamp 到 [-1.5, 1.5]
    // 防止极暗场景（如菜单 dark blue）把 exposure 推到 10× 以上。
    let raw_log_lum = textureSample(lum_tex, lum_sampler, vec2<f32>(0.5, 0.5)).r;
    let avg_log_lum = clamp(raw_log_lum, -1.5, 1.5);
    let avg_lum = max(exp(avg_log_lum), 1e-3);
    // exposure clamp 收到 [0.6, 1.3] —— 防止暗场景被自动曝光拉亮 2 倍把
    // 深 navy 水面变回亮 cyan,同时仍允许轻度场景适应
    let exposure = clamp(rp.middle_grey / avg_lum, rp.exposure_min, rp.exposure_max);
    let exposed = combined * exposure;

    var ldr = aces_tonemap(exposed * rp.aces_input_scale);

    // Desaturate 2% — 保留参考图政治色的鲜艳度,只压一点 ACES 的过饱和
    let lum = dot(ldr, vec3<f32>(0.2125, 0.7154, 0.0721));
    ldr = mix(vec3<f32>(lum), ldr, rp.saturation);

    if (rp.debug_view > 0.5 && rp.debug_view < 1.5) {
        ldr = clamp(scene, vec3<f32>(0.0), vec3<f32>(1.0));
    } else if (rp.debug_view > 1.5 && rp.debug_view < 2.5) {
        ldr = aces_tonemap(scene * exposure * rp.aces_input_scale);
    } else if (rp.debug_view > 2.5 && rp.debug_view < 3.5) {
        ldr = clamp(bloom * rp.bloom_debug_gain, vec3<f32>(0.0), vec3<f32>(1.0));
    }

    // 手工 gamma：仅在非 sRGB 目标时启用（sRGB 目标走硬件 sRGB 编码）
    if (rp.srgb_target < 0.5) {
        ldr = pow(ldr, vec3<f32>(1.0 / 2.2));
    }

    // vignette
    let d = distance(in.uv, rp.center_uv);
    let vig = 1.0 - smoothstep(0.4, 0.85, d) * rp.vignette_strength;
    ldr = ldr * vig;

    return vec4<f32>(ldr, 1.0);
}
"#;

// ─── Uniform 结构 ────────────────────────────────────────────────────────────

const RESTORESCENE_PHASE10_WGSL: &str = r#"
@group(0) @binding(0) var hdr_tex: texture_2d<f32>;
@group(0) @binding(1) var hdr_sampler: sampler;
@group(0) @binding(2) var bloom_tex: texture_2d<f32>;
@group(0) @binding(3) var bloom_sampler: sampler;
@group(0) @binding(4) var lum_tex: texture_2d<f32>;
@group(0) @binding(5) var lum_sampler: sampler;
@group(0) @binding(6) var color_cube_tex: texture_2d_array<f32>;
@group(0) @binding(7) var color_cube_sampler: sampler;

struct RestoreParams {
    middle_grey: f32,
    bloom_strength: f32,
    srgb_target: f32,
    exposure_bias: f32,
    exposure_min: f32,
    exposure_max: f32,
    uncharted_white_point: f32,
    saturation: f32,
    debug_view: f32,
    bloom_debug_gain: f32,
    lut_strength: f32,
    lut_size: f32,
    hsv_hue_shift: f32,
    hsv_saturation: f32,
    hsv_value: f32,
    _pad0: f32,
    color_balance: vec4<f32>,
    lut_blend: vec4<f32>,
};
@group(0) @binding(8) var<uniform> rp: RestoreParams;

struct VsOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vid: u32) -> VsOut {
    var out: VsOut;
    let uv = vec2<f32>(f32((vid << 1u) & 2u), f32(vid & 2u));
    out.clip_pos = vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0);
    out.uv = vec2<f32>(uv.x, 1.0 - uv.y);
    return out;
}

fn uncharted2_tonemap_partial(x: vec3<f32>) -> vec3<f32> {
    let a = 0.15;
    let b = 0.50;
    let c = 0.10;
    let d = 0.20;
    let e = 0.02;
    let f = 0.30;
    return ((x * (a * x + c * b) + d * e) / (x * (a * x + b) + d * f)) - e / f;
}

fn uncharted2_tonemap(color: vec3<f32>, white_point: f32) -> vec3<f32> {
    let curr = uncharted2_tonemap_partial(color);
    let white_scale = 1.0 / uncharted2_tonemap_partial(vec3<f32>(max(white_point, 0.001))).r;
    return clamp(curr * white_scale, vec3<f32>(0.0), vec3<f32>(1.0));
}

fn restore_map_tonemap(color: vec3<f32>, white_point: f32) -> vec3<f32> {
    let linear_restore = clamp(color, vec3<f32>(0.0), vec3<f32>(1.0));
    let shoulder = uncharted2_tonemap(color, white_point);
    let highlight = smoothstep(0.70, 1.35, max(color.r, max(color.g, color.b)));
    return mix(linear_restore, shoulder, 0.10 + highlight * 0.22);
}

fn rgb_to_hsv(c: vec3<f32>) -> vec3<f32> {
    let k = vec4<f32>(0.0, -1.0 / 3.0, 2.0 / 3.0, -1.0);
    let p = select(vec4<f32>(c.bg, k.wz), vec4<f32>(c.gb, k.xy), c.b < c.g);
    let q = select(vec4<f32>(p.xyw, c.r), vec4<f32>(c.r, p.yzx), p.x < c.r);
    let d = q.x - min(q.w, q.y);
    let e = 1.0e-10;
    return vec3<f32>(abs(q.z + (q.w - q.y) / (6.0 * d + e)), d / (q.x + e), q.x);
}

fn hsv_to_rgb(c: vec3<f32>) -> vec3<f32> {
    let k = vec4<f32>(1.0, 2.0 / 3.0, 1.0 / 3.0, 3.0);
    let p = abs(fract(c.xxx + k.xyz) * 6.0 - k.www);
    return c.z * mix(k.xxx, clamp(p - k.xxx, vec3<f32>(0.0), vec3<f32>(1.0)), c.y);
}

fn apply_hsv(color: vec3<f32>) -> vec3<f32> {
    var hsv = rgb_to_hsv(clamp(color, vec3<f32>(0.0), vec3<f32>(1.0)));
    hsv.x = fract(hsv.x + rp.hsv_hue_shift);
    hsv.y = clamp(hsv.y * rp.hsv_saturation, 0.0, 2.0);
    hsv.z = max(hsv.z * rp.hsv_value, 0.0);
    return hsv_to_rgb(hsv);
}

fn apply_color_cube(color: vec3<f32>) -> vec3<f32> {
    let n = max(rp.lut_size, 2.0);
    let c = clamp(color, vec3<f32>(0.0), vec3<f32>(1.0));
    let b_idx = c.b * (n - 1.0);
    let b_lo = floor(b_idx);
    let b_hi = min(b_lo + 1.0, n - 1.0);
    let b_frac = b_idx - b_lo;
    let inv_n = 1.0 / n;
    let cell_w = inv_n;
    let u_lo = (b_lo + c.r * (1.0 - inv_n) + 0.5 * inv_n) * cell_w;
    let u_hi = (b_hi + c.r * (1.0 - inv_n) + 0.5 * inv_n) * cell_w;
    let v = c.g * (1.0 - inv_n) + 0.5 * inv_n;
    let day_layer = i32(max(rp.lut_blend.x, 0.0));
    let night_layer = i32(max(rp.lut_blend.y, 0.0));
    let lo_day = textureSample(color_cube_tex, color_cube_sampler, vec2<f32>(u_lo, v), day_layer).rgb;
    let hi_day = textureSample(color_cube_tex, color_cube_sampler, vec2<f32>(u_hi, v), day_layer).rgb;
    let lo_night = textureSample(color_cube_tex, color_cube_sampler, vec2<f32>(u_lo, v), night_layer).rgb;
    let hi_night = textureSample(color_cube_tex, color_cube_sampler, vec2<f32>(u_hi, v), night_layer).rgb;
    let lut_day = mix(lo_day, hi_day, b_frac);
    let lut_night = mix(lo_night, hi_night, b_frac);
    let lut = mix(lut_day, lut_night, clamp(rp.lut_blend.z, 0.0, 1.0));
    return mix(c, lut, clamp(rp.lut_strength, 0.0, 1.0));
}

fn apply_saturation(color: vec3<f32>, saturation: f32) -> vec3<f32> {
    let lum = dot(color, vec3<f32>(0.2125, 0.7154, 0.0721));
    return mix(vec3<f32>(lum), color, saturation);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let scene = textureSample(hdr_tex, hdr_sampler, in.uv).rgb;
    let bloom = textureSample(bloom_tex, bloom_sampler, in.uv).rgb;
    let scene_with_bloom = scene + bloom * rp.bloom_strength;

    let raw_log_lum = textureSample(lum_tex, lum_sampler, vec2<f32>(0.5, 0.5)).r;
    let avg_log_lum = clamp(raw_log_lum, -8.0, 8.0);
    let avg_lum = max(exp(avg_log_lum), 1e-3);
    // The vanilla luminance chain adapts through LastLuminance. Until that
    // temporal target is mirrored, using the current frame's 1x1 average here
    // makes exposure pulse when the camera pans across different land/sea
    // ratios. Keep the final map path stable and leave avg_lum for debug.
    let exposure = rp.exposure_bias;
    let tonemap_input = scene_with_bloom * exposure;
    let tonemapped = restore_map_tonemap(tonemap_input, rp.uncharted_white_point);

    var graded = apply_color_cube(tonemapped);
    graded = apply_hsv(graded);
    graded = apply_saturation(graded, rp.saturation);
    graded = max(graded + rp.color_balance.rgb, vec3<f32>(0.0));
    var ldr = clamp(graded, vec3<f32>(0.0), vec3<f32>(1.0));

    if (rp.debug_view > 0.5 && rp.debug_view < 1.5) {
        ldr = clamp(scene, vec3<f32>(0.0), vec3<f32>(1.0));
    } else if (rp.debug_view > 1.5 && rp.debug_view < 2.5) {
        ldr = clamp(bloom * rp.bloom_debug_gain, vec3<f32>(0.0), vec3<f32>(1.0));
    } else if (rp.debug_view > 2.5 && rp.debug_view < 3.5) {
        let lum_debug = clamp(log2(max(avg_lum, 1e-4)) / 8.0 + 0.5, 0.0, 1.0);
        ldr = vec3<f32>(lum_debug);
    } else if (rp.debug_view > 3.5 && rp.debug_view < 4.5) {
        ldr = clamp(tonemap_input / max(rp.uncharted_white_point, 1.0), vec3<f32>(0.0), vec3<f32>(1.0));
    } else if (rp.debug_view > 4.5 && rp.debug_view < 5.5) {
        ldr = tonemapped;
    } else if (rp.debug_view > 5.5 && rp.debug_view < 6.5) {
        ldr = tonemapped;
    } else if (rp.debug_view > 6.5 && rp.debug_view < 7.5) {
        ldr = clamp(graded, vec3<f32>(0.0), vec3<f32>(1.0));
    }

    if (rp.srgb_target < 0.5) {
        ldr = pow(ldr, vec3<f32>(1.0 / 2.2));
    }

    return vec4<f32>(ldr, 1.0);
}
"#;

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct BloomParams {
    bright_threshold: f32,
    bloom_strength: f32,
    inv_size_x: f32,
    inv_size_y: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct DownParams {
    inv_src_size: [f32; 2],
    _pad: [f32; 2],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct LumParams {
    inv_src_size: [f32; 2],
    _pad: [f32; 2],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct RestoreParams {
    middle_grey: f32,
    bloom_strength: f32,
    srgb_target: f32,
    exposure_bias: f32,
    exposure_min: f32,
    exposure_max: f32,
    uncharted_white_point: f32,
    saturation: f32,
    debug_view: f32,
    bloom_debug_gain: f32,
    lut_strength: f32,
    lut_size: f32,
    hsv_hue_shift: f32,
    hsv_saturation: f32,
    hsv_value: f32,
    _pad0: f32,
    color_balance: [f32; 4],
    lut_blend: [f32; 4],
}

// ─── 单个 RT + bind group 的小辅助 ─────────────────────────────────────────────

struct PingTarget {
    #[allow(dead_code)]
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    width: u32,
    height: u32,
}

fn make_target(
    device: &wgpu::Device,
    label: &str,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
) -> PingTarget {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    PingTarget {
        texture,
        view,
        width: width.max(1),
        height: height.max(1),
    }
}

fn identity_color_cube_image() -> ColorCubeImage {
    const LUT_SIZE: u32 = 32;
    let width = LUT_SIZE * LUT_SIZE;
    let height = LUT_SIZE;
    let mut data = Vec::with_capacity((width * height * 4) as usize);
    for g in 0..LUT_SIZE {
        for b in 0..LUT_SIZE {
            for r in 0..LUT_SIZE {
                data.push(((r * 255) / (LUT_SIZE - 1)) as u8);
                data.push(((g * 255) / (LUT_SIZE - 1)) as u8);
                data.push(((b * 255) / (LUT_SIZE - 1)) as u8);
                data.push(255);
            }
        }
    }
    ColorCubeImage {
        source_path: "identity_color_cube".to_string(),
        size: LUT_SIZE,
        pixels: data,
    }
}

fn make_color_cube_array(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    source: &ColorCubeSource,
) -> (wgpu::Texture, wgpu::TextureView) {
    let lut_size = source.cubes.first().map(|cube| cube.size).unwrap_or(32);
    let width = lut_size * lut_size;
    let height = lut_size;
    let layers = source.cubes.len().max(1) as u32;
    let identity = identity_color_cube_image();
    let mut data = Vec::with_capacity((width * height * 4 * layers) as usize);
    for cube in &source.cubes {
        if cube.size == lut_size && cube.pixels.len() == (width * height * 4) as usize {
            data.extend_from_slice(&cube.pixels);
        } else if identity.pixels.len() == (width * height * 4) as usize {
            data.extend_from_slice(&identity.pixels);
        } else {
            data.resize(data.len() + (width * height * 4) as usize, 255);
        }
    }
    if data.is_empty() {
        data.extend_from_slice(&identity.pixels);
    }
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(if source.is_identity_fallback() {
            "postprocess_identity_color_cube"
        } else {
            "postprocess_vanilla_color_cube"
        }),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: layers,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &data,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(width * 4),
            rows_per_image: Some(height),
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: layers,
        },
    );
    let view = texture.create_view(&wgpu::TextureViewDescriptor {
        label: Some("postprocess_color_cube_array_view"),
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        ..Default::default()
    });
    (texture, view)
}

fn normalize_lut_path(path: impl AsRef<str>) -> String {
    let path = path.as_ref().replace('\\', "/");
    Path::new(&path)
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

// ─── 共享 BGL：全屏 sample + uniform ──────────────────────────────────────────

fn make_simple_bgl(device: &wgpu::Device, label: &str) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some(label),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    })
}

fn make_simple_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    src_view: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
    uniform: &wgpu::Buffer,
    label: &str,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(label),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(src_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: uniform.as_entire_binding(),
            },
        ],
    })
}

fn make_simple_pipeline(
    device: &wgpu::Device,
    label: &str,
    wgsl: &str,
    layout: &wgpu::BindGroupLayout,
    target_format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(label),
        source: wgpu::ShaderSource::Wgsl(wgsl.into()),
    });
    let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(label),
        bind_group_layouts: &[layout],
        push_constant_ranges: &[],
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(&pl),
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: Some("vs_main"),
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: target_format,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: Default::default(),
        multiview: None,
        cache: None,
    })
}

// ─── 真正的 PostProcessChain ─────────────────────────────────────────────────

const BLOOM_LEVELS: usize = 4;
const LUM_LEVELS: usize = 4;

pub struct PostProcessChain {
    /// 完整 / Off 切换。
    pub mode: PostProcessMode,
    pub calibration: PostProcessCalibration,
    pub debug_view: PostProcessDebugView,

    /// 共享：全屏 sample + uniform 用 BGL（bloom / downsample / lum 都用这个 layout）。
    simple_bgl: wgpu::BindGroupLayout,

    sampler: wgpu::Sampler,

    // bloom chain
    bloom_pipeline: wgpu::RenderPipeline,
    bloom_uniform: wgpu::Buffer,
    bloom_targets: Vec<PingTarget>, // 长度 = BLOOM_LEVELS（lvl0 = bright，lvl1..3 = downsample）
    bloom_bind_groups: Vec<wgpu::BindGroup>, // lvl0 输入 hdr，lvl1.. 输入 上一级 view

    downsample_pipeline: wgpu::RenderPipeline,
    downsample_uniforms: Vec<wgpu::Buffer>, // 一个 per downsample step

    // luminance reduction chain
    lum_log_pipeline: wgpu::RenderPipeline,
    lum_avg_pipeline: wgpu::RenderPipeline,
    lum_uniforms: Vec<wgpu::Buffer>,
    lum_targets: Vec<PingTarget>,
    lum_bind_groups: Vec<wgpu::BindGroup>,

    // restorescene final
    restore_bgl: wgpu::BindGroupLayout,
    restore_pipeline: wgpu::RenderPipeline,
    restore_uniform: wgpu::Buffer,
    restore_bind_group: wgpu::BindGroup,
    #[allow(dead_code)]
    color_cube_texture: wgpu::Texture,
    color_cube_view: wgpu::TextureView,
    color_cube_lut_size: f32,
    color_cube_bindings: [usize; POST_PROCESS_LUT_KEY_COUNT],
    pub color_cube_source: String,
    pub color_cube_fallback: bool,
    pub color_cube_warnings: Vec<String>,

    /// HDR full-res 尺寸（用来计算各级链尺寸）。
    hdr_w: u32,
    hdr_h: u32,
    /// swap-chain 输出格式 (`Bgra8UnormSrgb` 等)。
    swap_format: wgpu::TextureFormat,
    /// `1.0` 当 `swap_format` 是 sRGB 时（restorescene 跳过手工 gamma）。
    srgb_target_flag: f32,
}

impl PostProcessChain {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        hdr_view: &wgpu::TextureView,
        hdr_w: u32,
        hdr_h: u32,
        swap_format: wgpu::TextureFormat,
        color_cube_source: ColorCubeSource,
    ) -> Self {
        let _ = HDR_FORMAT; // sanity reference

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("postprocess_sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });

        let simple_bgl = make_simple_bgl(device, "postprocess_simple_bgl");

        // ── bloom ───────────────────────────────────────────────────────
        let bloom_pipeline = make_simple_pipeline(
            device,
            "bloom_bright",
            BLOOM_BRIGHT_WGSL,
            &simple_bgl,
            HDR_FORMAT,
        );
        let bloom_uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("bloom_uniform"),
            size: std::mem::size_of::<BloomParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let downsample_pipeline = make_simple_pipeline(
            device,
            "bloom_downsample",
            DOWNSAMPLE_WGSL,
            &simple_bgl,
            HDR_FORMAT,
        );

        // ── lum ─────────────────────────────────────────────────────────
        let lum_log_pipeline = make_simple_pipeline(
            device,
            "lum_log",
            LUM_LOG_WGSL,
            &simple_bgl,
            wgpu::TextureFormat::R16Float,
        );
        let lum_avg_pipeline = make_simple_pipeline(
            device,
            "lum_avg",
            LUM_AVG_WGSL,
            &simple_bgl,
            wgpu::TextureFormat::R16Float,
        );

        // ── restore ─────────────────────────────────────────────────────
        let (color_cube_texture, color_cube_view) =
            make_color_cube_array(device, queue, &color_cube_source);
        let color_cube_lut_size = color_cube_source
            .cubes
            .first()
            .map(|cube| cube.size as f32)
            .unwrap_or(32.0);
        let color_cube_bindings = color_cube_source.bindings;
        let color_cube_source_summary = color_cube_source.source_summary();
        let color_cube_fallback = color_cube_source.is_identity_fallback();
        let color_cube_warnings = color_cube_source.warnings.clone();
        let restore_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("restore_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 7,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 8,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let restore_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("restore_live"),
            source: wgpu::ShaderSource::Wgsl(RESTORESCENE_PHASE10_WGSL.into()),
        });
        let restore_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("restore_pl"),
            bind_group_layouts: &[&restore_bgl],
            push_constant_ranges: &[],
        });
        let restore_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("restore_pipeline"),
            layout: Some(&restore_pl),
            vertex: wgpu::VertexState {
                module: &restore_module,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &restore_module,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: swap_format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });
        let restore_uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("restore_uniform"),
            size: std::mem::size_of::<RestoreParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let srgb_target_flag = if swap_format.is_srgb() { 1.0 } else { 0.0 };

        let mut chain = Self {
            mode: PostProcessMode::default(),
            calibration: PostProcessCalibration::default(),
            debug_view: PostProcessDebugView::default(),
            simple_bgl,
            sampler,
            bloom_pipeline,
            bloom_uniform,
            bloom_targets: Vec::new(),
            bloom_bind_groups: Vec::new(),
            downsample_pipeline,
            downsample_uniforms: Vec::new(),
            lum_log_pipeline,
            lum_avg_pipeline,
            lum_uniforms: Vec::new(),
            lum_targets: Vec::new(),
            lum_bind_groups: Vec::new(),
            restore_bgl,
            restore_pipeline,
            restore_uniform,
            // restore_bind_group placeholder filled below
            restore_bind_group: device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("restore_bg_placeholder"),
                layout: &device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("placeholder_bgl"),
                    entries: &[],
                }),
                entries: &[],
            }),
            color_cube_texture,
            color_cube_view,
            color_cube_lut_size,
            color_cube_bindings,
            color_cube_source: color_cube_source_summary,
            color_cube_fallback,
            color_cube_warnings,
            hdr_w,
            hdr_h,
            swap_format,
            srgb_target_flag,
        };
        if !color_cube_fallback {
            chain.calibration.lut_strength = 1.0;
        }
        chain.rebuild_targets(device, hdr_view, hdr_w, hdr_h);
        chain
    }

    /// HDR resize 时调用：重新创建所有 ping-pong 目标 + 重绑 bind group。
    pub fn cycle_debug_view(&mut self) -> PostProcessDebugView {
        self.debug_view = self.debug_view.next();
        self.debug_view
    }

    pub fn rebuild_targets(
        &mut self,
        device: &wgpu::Device,
        hdr_view: &wgpu::TextureView,
        hdr_w: u32,
        hdr_h: u32,
    ) {
        self.hdr_w = hdr_w;
        self.hdr_h = hdr_h;

        // ── bloom targets (level 0 = W/2 × H/2; halve each step) ────────
        self.bloom_targets.clear();
        let mut w = (hdr_w / 2).max(2);
        let mut h = (hdr_h / 2).max(2);
        for lvl in 0..BLOOM_LEVELS {
            let label = format!("bloom_lvl{}", lvl);
            self.bloom_targets
                .push(make_target(device, &label, w, h, HDR_FORMAT));
            w = (w / 2).max(2);
            h = (h / 2).max(2);
        }

        // 先一次性创建 downsample uniform buffers（lvl1..N），等下再绑 bind group。
        self.downsample_uniforms.clear();
        for lvl in 1..BLOOM_LEVELS {
            let _ = lvl;
            let buf = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("downsample_uniform"),
                size: std::mem::size_of::<DownParams>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.downsample_uniforms.push(buf);
        }

        // bloom bind groups: lvl0 reads hdr; lvl1..3 read previous level
        self.bloom_bind_groups.clear();
        for lvl in 0..BLOOM_LEVELS {
            let src_view = if lvl == 0 {
                hdr_view
            } else {
                &self.bloom_targets[lvl - 1].view
            };
            let uniform = if lvl == 0 {
                &self.bloom_uniform
            } else {
                &self.downsample_uniforms[lvl - 1]
            };
            let bg = make_simple_bind_group(
                device,
                &self.simple_bgl,
                src_view,
                &self.sampler,
                uniform,
                &format!("bloom_bg_lvl{}", lvl),
            );
            self.bloom_bind_groups.push(bg);
        }

        // ── luminance reduction targets ─────────────────────────────────
        // lvl0 reads HDR full-res, outputs to W/16 × H/16 (R16Float)
        // lvl1..3 average down towards 1×1
        self.lum_targets.clear();
        let mut lw = (hdr_w / 16).max(2);
        let mut lh = (hdr_h / 16).max(2);
        for lvl in 0..LUM_LEVELS {
            let label = format!("lum_lvl{}", lvl);
            self.lum_targets.push(make_target(
                device,
                &label,
                lw,
                lh,
                wgpu::TextureFormat::R16Float,
            ));
            // 下一级再 ÷ 4
            lw = (lw / 4).max(1);
            lh = (lh / 4).max(1);
        }

        // 一次性创建 lum uniforms。
        self.lum_uniforms.clear();
        for _ in 0..LUM_LEVELS {
            let uniform = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("lum_uniform"),
                size: std::mem::size_of::<LumParams>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.lum_uniforms.push(uniform);
        }

        self.lum_bind_groups.clear();
        for lvl in 0..LUM_LEVELS {
            let src_view = if lvl == 0 {
                hdr_view
            } else {
                &self.lum_targets[lvl - 1].view
            };
            let bg = make_simple_bind_group(
                device,
                &self.simple_bgl,
                src_view,
                &self.sampler,
                &self.lum_uniforms[lvl],
                &format!("lum_bg_lvl{}", lvl),
            );
            self.lum_bind_groups.push(bg);
        }

        // ── restore bind group ──────────────────────────────────────────
        let bloom_final_view = &self.bloom_targets[BLOOM_LEVELS - 1].view;
        let lum_final_view = &self.lum_targets[LUM_LEVELS - 1].view;
        self.restore_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("restore_bg"),
            layout: &self.restore_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(hdr_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(bloom_final_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(lum_final_view),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::TextureView(&self.color_cube_view),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: self.restore_uniform.as_entire_binding(),
                },
            ],
        });
    }

    /// 每帧调一次，更新所有 uniform。
    pub fn prepare(&self, queue: &wgpu::Queue, selection: PostProcessLutSelection) {
        // bloom bright pass：threshold 抬到 1.05 → 仅真正 HDR 高光（>1.0）参与 bloom，
        // 防止 LDR 区域整体染上一层"奶白"提亮。strength 0.6 让 9-tap 模糊不超过场景亮度。
        let bp = BloomParams {
            bright_threshold: self.calibration.bloom_bright_threshold,
            bloom_strength: self.calibration.bloom_prefilter_strength,
            inv_size_x: 1.0 / self.hdr_w as f32,
            inv_size_y: 1.0 / self.hdr_h as f32,
        };
        queue.write_buffer(&self.bloom_uniform, 0, bytemuck::bytes_of(&bp));

        // downsample uniforms（每级用上一级尺寸）
        for (i, buf) in self.downsample_uniforms.iter().enumerate() {
            let src = &self.bloom_targets[i];
            let dp = DownParams {
                inv_src_size: [1.0 / src.width as f32, 1.0 / src.height as f32],
                _pad: [0.0; 2],
            };
            queue.write_buffer(buf, 0, bytemuck::bytes_of(&dp));
        }

        // lum uniforms
        for (i, buf) in self.lum_uniforms.iter().enumerate() {
            let (sw, sh) = if i == 0 {
                (self.hdr_w as f32, self.hdr_h as f32)
            } else {
                (
                    self.lum_targets[i - 1].width as f32,
                    self.lum_targets[i - 1].height as f32,
                )
            };
            let lp = LumParams {
                inv_src_size: [1.0 / sw, 1.0 / sh],
                _pad: [0.0; 2],
            };
            queue.write_buffer(buf, 0, bytemuck::bytes_of(&lp));
        }

        let selected_middle_grey = selection.tonemap_middle_grey();
        let middle_grey =
            self.calibration.middle_grey * (selected_middle_grey / STANDARD_TONEMAP_MIDDLE_GREY);

        // Restore params. Vanilla posteffect values use tonemap_middlegrey in
        // the 0.50-0.80 range depending on LUT volume; a hard-coded 0.18 makes
        // the map under-expose and then lets the LUT dominate the remaining
        // contrast.
        let rp = RestoreParams {
            middle_grey,
            bloom_strength: self.calibration.final_bloom_strength,
            srgb_target: self.srgb_target_flag,
            exposure_bias: self.calibration.exposure_bias,
            exposure_min: self.calibration.exposure_min,
            exposure_max: self.calibration.exposure_max,
            uncharted_white_point: self.calibration.uncharted_white_point,
            saturation: self.calibration.saturation,
            debug_view: self.debug_view.as_shader_value(),
            bloom_debug_gain: self.calibration.bloom_debug_gain,
            lut_strength: self.calibration.lut_strength,
            lut_size: self.color_cube_lut_size,
            hsv_hue_shift: self.calibration.hsv_hue_shift,
            hsv_saturation: self.calibration.hsv_saturation,
            hsv_value: self.calibration.hsv_value,
            _pad0: 0.0,
            color_balance: [
                self.calibration.color_balance[0],
                self.calibration.color_balance[1],
                self.calibration.color_balance[2],
                0.0,
            ],
            lut_blend: self.lut_blend(selection),
        };
        queue.write_buffer(&self.restore_uniform, 0, bytemuck::bytes_of(&rp));
    }

    fn lut_blend(&self, selection: PostProcessLutSelection) -> [f32; 4] {
        let day_key = selection.key(false) as usize;
        let night_key = selection.key(true) as usize;
        [
            self.color_cube_bindings[day_key] as f32,
            self.color_cube_bindings[night_key] as f32,
            selection.night_factor.clamp(0.0, 1.0),
            0.0,
        ]
    }

    pub fn gamma_policy(&self) -> &'static str {
        if self.srgb_target_flag >= 0.5 {
            "hardware_srgb_encode_manual_gamma_disabled"
        } else {
            "shader_side_pow_1_over_2_2_to_unorm"
        }
    }

    pub fn runtime_lut_summary(&self, selection: PostProcessLutSelection) -> String {
        let day_key = selection.key(false);
        let night_key = selection.key(true);
        format!(
            "{} layers(day={},night={}) gamma_policy={} colorcube_fallback={}",
            selection.audit_summary(),
            self.color_cube_bindings[day_key as usize],
            self.color_cube_bindings[night_key as usize],
            self.gamma_policy(),
            self.color_cube_fallback,
        )
    }

    /// 在已开 encoder 中提交完整链。
    ///
    /// `final_target` = swap-chain 当前帧 view。
    pub fn render(&self, encoder: &mut wgpu::CommandEncoder, final_target: &wgpu::TextureView) {
        // ── bloom: lvl0 = bright pass; lvl1..3 = downsample ──
        for lvl in 0..BLOOM_LEVELS {
            let pipeline = if lvl == 0 {
                &self.bloom_pipeline
            } else {
                &self.downsample_pipeline
            };
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(if lvl == 0 {
                    "bloom_bright"
                } else {
                    "bloom_downsample"
                }),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.bloom_targets[lvl].view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });
            rp.set_pipeline(pipeline);
            rp.set_bind_group(0, &self.bloom_bind_groups[lvl], &[]);
            rp.draw(0..3, 0..1);
        }

        // ── luminance reduction ──
        for lvl in 0..LUM_LEVELS {
            let pipeline = if lvl == 0 {
                &self.lum_log_pipeline
            } else {
                &self.lum_avg_pipeline
            };
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(if lvl == 0 { "lum_log" } else { "lum_avg" }),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.lum_targets[lvl].view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });
            rp.set_pipeline(pipeline);
            rp.set_bind_group(0, &self.lum_bind_groups[lvl], &[]);
            rp.draw(0..3, 0..1);
        }

        // ── restorescene final composite ──
        {
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("restorescene"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: final_target,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });
            rp.set_pipeline(&self.restore_pipeline);
            rp.set_bind_group(0, &self.restore_bind_group, &[]);
            rp.draw(0..3, 0..1);
        }
    }
}

impl super::Pass for PostProcessChain {
    fn name(&self) -> &'static str {
        "post_process_chain"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shader_strings_nonempty() {
        assert!(!BLOOM_BRIGHT_WGSL.is_empty());
        assert!(!DOWNSAMPLE_WGSL.is_empty());
        assert!(!LUM_LOG_WGSL.is_empty());
        assert!(LUM_AVG_WGSL.contains("vs_main"));
        assert!(RESTORESCENE_PHASE10_WGSL.contains("uncharted2_tonemap"));
        assert!(RESTORESCENE_PHASE10_WGSL.contains("apply_color_cube"));
        assert!(RESTORESCENE_PHASE10_WGSL.contains("rgb_to_hsv"));
        assert!(RESTORESCENE_PHASE10_WGSL.contains("color_balance"));
        assert!(RESTORESCENE_PHASE10_WGSL.contains("srgb_target"));
        assert!(RESTORESCENE_PHASE10_WGSL.contains("debug_view"));
        assert!(RESTORESCENE_PHASE10_WGSL.contains("bloom_debug_gain"));
        assert!(!RESTORESCENE_PHASE10_WGSL.contains("aces_tonemap"));
        assert!(!RESTORESCENE_PHASE10_WGSL.contains("vignette_strength"));
    }

    #[test]
    fn uniform_sizes_align_16() {
        // wgpu uniform buffers must be at least 16 byte aligned in size.
        assert!(std::mem::size_of::<BloomParams>() % 16 == 0);
        assert!(std::mem::size_of::<DownParams>() % 16 == 0);
        assert!(std::mem::size_of::<LumParams>() % 16 == 0);
        assert!(std::mem::size_of::<RestoreParams>() % 16 == 0);
    }

    #[test]
    fn phase10_calibration_is_atmosphere_tuned_restore_scene() {
        let calibration = PostProcessCalibration::phase10_vanilla();
        assert!((calibration.middle_grey - 0.55).abs() < f32::EPSILON);
        assert!(calibration.exposure_min <= 0.125);
        assert!(calibration.exposure_max >= 8.0);
        assert!((calibration.exposure_bias - 1.02).abs() < f32::EPSILON);
        assert!((calibration.uncharted_white_point - 11.2).abs() < f32::EPSILON);
        assert!((calibration.final_bloom_strength - 0.14).abs() < f32::EPSILON);
        assert!((calibration.bloom_bright_threshold - 1.05).abs() < f32::EPSILON);
        assert!((calibration.bloom_prefilter_strength - 0.68).abs() < f32::EPSILON);
        assert_eq!(calibration.lut_strength, 0.35);
        assert_eq!(calibration.saturation, 0.96);
        assert_eq!(calibration.hsv_saturation, 0.94);
        assert_eq!(calibration.hsv_value, 1.02);
        assert_eq!(calibration.color_balance, [0.004, 0.002, -0.004]);
        assert!(calibration.summary().contains("aces=off"));
        assert!(calibration.summary().contains("lut=0.35"));
        assert!(calibration.summary().contains("restore=uncharted"));
    }

    #[test]
    fn restore_shader_uses_stable_exposure_until_last_luminance_is_mirrored() {
        assert!(RESTORESCENE_PHASE10_WGSL.contains("let exposure = rp.exposure_bias;"));
        assert!(
            !RESTORESCENE_PHASE10_WGSL.contains("let exposure = clamp((rp.middle_grey / avg_lum)")
        );
    }

    #[test]
    fn restore_shader_keeps_map_luminance_near_simple_blit() {
        assert!(RESTORESCENE_PHASE10_WGSL.contains("fn restore_map_tonemap"));
        assert!(RESTORESCENE_PHASE10_WGSL.contains("let linear_restore = clamp(color"));
        assert!(RESTORESCENE_PHASE10_WGSL.contains("return mix(linear_restore, shoulder"));
        assert!(!RESTORESCENE_PHASE10_WGSL
            .contains("let tonemapped = uncharted2_tonemap(tonemap_input"));
    }

    #[test]
    fn postprocess_debug_view_cycles_through_phase9_views() {
        let mut view = PostProcessDebugView::Final;
        let mut seen = Vec::new();
        for _ in 0..PostProcessDebugView::ALL.len() {
            seen.push(view);
            view = view.next();
        }
        assert_eq!(seen, PostProcessDebugView::ALL);
        assert_eq!(view, PostProcessDebugView::Final);
        assert_eq!(PostProcessDebugView::BloomOnly.name(), "bloom_only");
        assert_eq!(PostProcessDebugView::AvgLuminance.name(), "avg_luminance");
        assert_eq!(PostProcessDebugView::TonemapBefore.name(), "tonemap_before");
        assert_eq!(PostProcessDebugView::TonemapOnly.name(), "tonemap_after");
        assert_eq!(PostProcessDebugView::LutBefore.name(), "lut_before");
        assert_eq!(PostProcessDebugView::LutAfter.name(), "lut_after");
    }

    #[test]
    fn restore_shader_validates() {
        let module = naga::front::wgsl::parse_str(RESTORESCENE_PHASE10_WGSL)
            .expect("restore shader should parse");
        let mut validator = naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        );
        validator
            .validate(&module)
            .expect("restore shader should validate");
    }

    #[test]
    fn color_cube_tga_accepts_32_cube_layout() {
        let image = TgaImage {
            width: 1024,
            height: 32,
            pixels: vec![0; 1024 * 32 * 4],
        };
        let cube = ColorCubeImage::from_tga("gfx/world/colorcorrection.tga", image).unwrap();
        assert_eq!(cube.size, 32);
    }

    #[test]
    fn color_cube_tga_rejects_legacy_16_cube_layout() {
        let image = TgaImage {
            width: 256,
            height: 16,
            pixels: vec![0; 256 * 16 * 4],
        };
        let err = ColorCubeImage::from_tga("gfx/world/legacy_lut.tga", image).unwrap_err();
        assert!(err.contains("expected 1024x32"));
    }

    #[test]
    fn postprocess_identity_fallback_uses_32_cube_layout() {
        let source = ColorCubeSource::identity();
        assert!(source.is_identity_fallback());
        assert_eq!(source.cubes[0].size, 32);
        assert_eq!(source.cubes[0].pixels.len(), 1024 * 32 * 4);
    }

    #[test]
    fn lut_selection_uses_distance_water_and_night_variants() {
        assert_eq!(
            PostProcessLutSelection {
                camera_distance_t: 0.1,
                night_factor: 0.0,
                water_factor: 0.0,
                winter_factor: 0.0,
            }
            .key(false),
            PostProcessLutKey::DefaultDay
        );
        assert_eq!(
            PostProcessLutSelection {
                camera_distance_t: 0.8,
                night_factor: 1.0,
                water_factor: 0.0,
                winter_factor: 0.0,
            }
            .key(true),
            PostProcessLutKey::FarDistanceNight
        );
        assert_eq!(
            PostProcessLutSelection {
                camera_distance_t: 0.1,
                night_factor: 0.0,
                water_factor: 0.56,
                winter_factor: 0.0,
            }
            .key(false),
            PostProcessLutKey::WaterDay
        );
        assert_eq!(
            PostProcessLutSelection {
                camera_distance_t: 0.1,
                night_factor: 0.0,
                water_factor: 0.55,
                winter_factor: 0.0,
            }
            .key(false),
            PostProcessLutKey::DefaultDay
        );
        assert_eq!(
            PostProcessLutSelection {
                camera_distance_t: 0.1,
                night_factor: 0.0,
                water_factor: 0.0,
                winter_factor: 1.0,
            }
            .key(false),
            PostProcessLutKey::WinterDay
        );
        assert!((PostProcessLutKey::WinterDay.tonemap_middle_grey() - 0.80).abs() < f32::EPSILON);
        assert!(
            (PostProcessLutSelection {
                camera_distance_t: 0.1,
                night_factor: 1.0,
                water_factor: 0.56,
                winter_factor: 0.0,
            }
            .tonemap_middle_grey()
                - 0.40)
                .abs()
                < f32::EPSILON
        );
    }

    #[test]
    fn lut_selection_audit_summary_exposes_phase_b_fields() {
        let summary = PostProcessLutSelection {
            camera_distance_t: 0.1,
            night_factor: 0.25,
            water_factor: 0.0,
            winter_factor: 0.0,
        }
        .audit_summary();
        assert!(summary.contains("lut_day=default_day"));
        assert!(summary.contains("lut_night=default_night"));
        assert!(summary.contains("middle_grey=0.55"));
        assert!(summary.contains("winter=0.00"));
    }
}
