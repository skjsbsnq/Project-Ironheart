use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use hoi4_assets::{FsAssetDb, MapAssetAudit, MapAssetQuality, VanillaMapSet};
use hoi4_paths::PathConfig;
use hoi4_state::GameDate;

use crate::map_image_diff::ImageDiffMetrics;
use crate::vanilla_resource_views::{BindingAudit, VanillaResourceViews};

const MAP_SIZE_PX: [f32; 2] = [5632.0, 2048.0];
const SCENE_CONFIG_TSV: &str = include_str!("../map_parity_scenes.tsv");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapBaselinePreset {
    High,
}

impl MapBaselinePreset {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::High => "high",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapBaselineDiffStatus {
    NotRun,
    Ready,
    MissingProject,
    MissingReference,
    Failed,
}

impl MapBaselineDiffStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotRun => "not_run",
            Self::Ready => "ready",
            Self::MissingProject => "missing_project",
            Self::MissingReference => "missing_reference",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapBaselineLayer {
    FinalFull,
    PostprocessOff,
    HdrRaw,
    AvgLuminance,
    TonemapBefore,
    TonemapOnly,
    LutBefore,
    LutAfter,
    BloomOnly,
    TerrainOnly,
    WaterOnly,
    RiverMask,
    BordersOnly,
    ObjectsOnly,
    OverlaysOnly,
    LabelsOnly,
    ProvinceSecondaryDebug,
    GradientBorderCh3Debug,
    TerrainRiverMaskDebug,
    FowVisibilityDebug,
    TerrainFinalBeforePostprocessDebug,
    AssetFallbackDebug,
}

impl MapBaselineLayer {
    pub const ALL: [Self; 21] = [
        Self::FinalFull,
        Self::TerrainOnly,
        Self::WaterOnly,
        Self::RiverMask,
        Self::BordersOnly,
        Self::ObjectsOnly,
        Self::OverlaysOnly,
        Self::ProvinceSecondaryDebug,
        Self::GradientBorderCh3Debug,
        Self::TerrainRiverMaskDebug,
        Self::FowVisibilityDebug,
        Self::TerrainFinalBeforePostprocessDebug,
        Self::HdrRaw,
        Self::BloomOnly,
        Self::AvgLuminance,
        Self::TonemapBefore,
        Self::TonemapOnly,
        Self::LutBefore,
        Self::LutAfter,
        Self::PostprocessOff,
        Self::AssetFallbackDebug,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::FinalFull => "final",
            Self::PostprocessOff => "postprocess_off",
            Self::HdrRaw => "hdr",
            Self::AvgLuminance => "avg_luminance",
            Self::TonemapBefore => "tonemap_before",
            Self::TonemapOnly => "tonemap",
            Self::LutBefore => "lut_before",
            Self::LutAfter => "lut_after",
            Self::BloomOnly => "bloom",
            Self::TerrainOnly => "terrain",
            Self::WaterOnly => "water",
            Self::RiverMask => "rivers",
            Self::BordersOnly => "borders",
            Self::ObjectsOnly => "objects",
            Self::OverlaysOnly => "overlays",
            Self::LabelsOnly => "labels",
            Self::ProvinceSecondaryDebug => "province_secondary",
            Self::GradientBorderCh3Debug => "gradient_border_ch3",
            Self::TerrainRiverMaskDebug => "terrain_river_mask",
            Self::FowVisibilityDebug => "fow_visibility",
            Self::TerrainFinalBeforePostprocessDebug => "terrain_final_before_postprocess",
            Self::AssetFallbackDebug => "fallback_debug",
        }
    }

    pub const fn terrain_debug_view_name(self) -> Option<&'static str> {
        match self {
            Self::ProvinceSecondaryDebug => Some("province_secondary"),
            Self::GradientBorderCh3Debug => Some("gradient_border_ch3"),
            Self::TerrainRiverMaskDebug => Some("river_mask"),
            Self::FowVisibilityDebug => Some("fow_visibility"),
            Self::TerrainFinalBeforePostprocessDebug => Some("final_before_postprocess"),
            _ => None,
        }
    }

    pub const fn diagnostic_source(self) -> Option<&'static str> {
        match self {
            Self::ProvinceSecondaryDebug => Some("ProvinceSecondaryColorMap texture"),
            Self::GradientBorderCh3Debug => Some("GradientBorderChannel3 texture"),
            Self::TerrainRiverMaskDebug => Some("terrain material river mask"),
            Self::FowVisibilityDebug => Some("FOW visibility texture green channel"),
            Self::TerrainFinalBeforePostprocessDebug => {
                Some("terrain material final color before postprocess")
            }
            _ => None,
        }
    }

    pub const fn is_phase2_probe(self) -> bool {
        self.terrain_debug_view_name().is_some()
    }

    fn from_config_name(value: &str) -> Option<Self> {
        match value.trim() {
            "final" | "final_full" => Some(Self::FinalFull),
            "postprocess_off" => Some(Self::PostprocessOff),
            "hdr" | "hdr_raw" => Some(Self::HdrRaw),
            "avg_luminance" | "average_luminance" => Some(Self::AvgLuminance),
            "tonemap_before" | "tonemap_input" => Some(Self::TonemapBefore),
            "tonemap" | "tonemap_only" => Some(Self::TonemapOnly),
            "lut_before" => Some(Self::LutBefore),
            "lut_after" => Some(Self::LutAfter),
            "bloom" | "bloom_only" => Some(Self::BloomOnly),
            "terrain" | "terrain_only" => Some(Self::TerrainOnly),
            "water" | "water_only" => Some(Self::WaterOnly),
            "rivers" | "river_mask" | "river_only" => Some(Self::RiverMask),
            "borders" | "borders_only" => Some(Self::BordersOnly),
            "objects" | "objects_only" => Some(Self::ObjectsOnly),
            "overlays" | "overlays_only" => Some(Self::OverlaysOnly),
            "labels" | "labels_only" => Some(Self::LabelsOnly),
            "province_secondary" | "province_secondary_debug" => Some(Self::ProvinceSecondaryDebug),
            "gradient_border_ch3" | "gradient_border_ch3_debug" => {
                Some(Self::GradientBorderCh3Debug)
            }
            "terrain_river_mask" | "terrain_river_mask_debug" | "river_mask_input" => {
                Some(Self::TerrainRiverMaskDebug)
            }
            "fow_visibility" | "fow_visibility_debug" => Some(Self::FowVisibilityDebug),
            "terrain_final_before_postprocess"
            | "final_before_postprocess"
            | "terrain_final_before_postprocess_debug" => {
                Some(Self::TerrainFinalBeforePostprocessDebug)
            }
            "fallback_debug" | "asset_fallback_debug" => Some(Self::AssetFallbackDebug),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapLayerMask {
    pub sky: bool,
    pub terrain: bool,
    pub water: bool,
    pub river: bool,
    pub borders: bool,
    pub static_decals: bool,
    pub overlays: bool,
    pub objects: bool,
    pub labels: bool,
    pub particles: bool,
    pub ui: bool,
    pub postprocess: bool,
    pub asset_fallback_debug: bool,
}

impl MapLayerMask {
    pub fn for_layer(layer: MapBaselineLayer) -> Self {
        match layer {
            MapBaselineLayer::FinalFull => Self::all(),
            MapBaselineLayer::PostprocessOff => Self {
                postprocess: false,
                ..Self::all()
            },
            MapBaselineLayer::HdrRaw
            | MapBaselineLayer::AvgLuminance
            | MapBaselineLayer::TonemapBefore
            | MapBaselineLayer::TonemapOnly
            | MapBaselineLayer::LutBefore
            | MapBaselineLayer::LutAfter
            | MapBaselineLayer::BloomOnly => Self::all(),
            MapBaselineLayer::TerrainOnly
            | MapBaselineLayer::ProvinceSecondaryDebug
            | MapBaselineLayer::GradientBorderCh3Debug
            | MapBaselineLayer::TerrainRiverMaskDebug
            | MapBaselineLayer::FowVisibilityDebug
            | MapBaselineLayer::TerrainFinalBeforePostprocessDebug => Self {
                terrain: true,
                ..Self::none()
            },
            MapBaselineLayer::WaterOnly => Self {
                water: true,
                ..Self::none()
            },
            MapBaselineLayer::RiverMask => Self {
                river: true,
                static_decals: true,
                ..Self::none()
            },
            MapBaselineLayer::BordersOnly => Self {
                borders: true,
                ..Self::none()
            },
            MapBaselineLayer::ObjectsOnly => Self {
                objects: true,
                ..Self::none()
            },
            MapBaselineLayer::OverlaysOnly => Self {
                static_decals: true,
                overlays: true,
                objects: true,
                particles: true,
                ..Self::none()
            },
            MapBaselineLayer::LabelsOnly => Self {
                labels: true,
                ..Self::none()
            },
            MapBaselineLayer::AssetFallbackDebug => Self {
                asset_fallback_debug: true,
                ..Self::none()
            },
        }
    }

    pub const fn all() -> Self {
        Self {
            sky: true,
            terrain: true,
            water: true,
            river: true,
            borders: true,
            static_decals: true,
            overlays: true,
            objects: true,
            labels: true,
            particles: true,
            ui: false,
            postprocess: true,
            asset_fallback_debug: false,
        }
    }

    pub const fn none() -> Self {
        Self {
            sky: false,
            terrain: false,
            water: false,
            river: false,
            borders: false,
            static_decals: false,
            overlays: false,
            objects: false,
            labels: false,
            particles: false,
            ui: false,
            postprocess: false,
            asset_fallback_debug: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MapBaselineCamera {
    pub target_uv: [f32; 2],
    pub distance_factor: f32,
    pub pitch_degrees: f32,
    pub yaw_degrees: f32,
}

#[derive(Debug, Clone)]
pub struct MapBaselineScene {
    pub name: String,
    pub description: String,
    pub map_mode: String,
    pub date: GameDate,
    pub camera: MapBaselineCamera,
    pub enabled_layers: Vec<MapBaselineLayer>,
}

impl MapBaselineScene {
    pub fn screenshot_name(&self, layer: MapBaselineLayer, preset: MapBaselinePreset) -> String {
        format!(
            "project/{}/{}.{}.png",
            self.name,
            layer.as_str(),
            preset.as_str()
        )
    }

    pub fn vanilla_reference_name(
        &self,
        layer: MapBaselineLayer,
        preset: MapBaselinePreset,
    ) -> String {
        format!(
            "vanilla_reference/{}/{}.{}.png",
            self.name,
            layer.as_str(),
            preset.as_str()
        )
    }

    pub fn diff_report_name(&self, layer: MapBaselineLayer, preset: MapBaselinePreset) -> String {
        format!(
            "diff/{}/{}.{}.json",
            self.name,
            layer.as_str(),
            preset.as_str()
        )
    }

    pub fn map_px_center(&self) -> [f32; 2] {
        [
            self.camera.target_uv[0] * MAP_SIZE_PX[0],
            self.camera.target_uv[1] * MAP_SIZE_PX[1],
        ]
    }
}

pub fn fixed_scenes() -> Vec<MapBaselineScene> {
    parse_scene_config(SCENE_CONFIG_TSV).expect("map_parity_scenes.tsv must be valid")
}

#[derive(Debug, Clone)]
pub struct MapBaselinePlannedCapture {
    pub scene_name: String,
    pub layer: MapBaselineLayer,
    pub preset: MapBaselinePreset,
    pub filename: String,
    pub vanilla_reference_filename: String,
    pub diff_report_filename: String,
    pub layer_mask: MapLayerMask,
    pub frame_time_ms: Option<f32>,
    pub asset_quality: MapAssetQuality,
    pub asset_fallback_count: usize,
    pub visual_review_usable: bool,
    pub project_png_exists: bool,
    pub vanilla_reference_exists: bool,
    pub diff_status: MapBaselineDiffStatus,
    pub diff_metrics: Option<ImageDiffMetrics>,
    pub diff_error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct MapBaselineReport {
    pub scenes: Vec<MapBaselineScene>,
    pub planned_captures: Vec<MapBaselinePlannedCapture>,
    pub asset_audit: MapAssetAudit,
    pub binding_audit: BindingAudit,
    pub reference_source_root: Option<String>,
    pub elapsed_ms: f64,
}

impl MapBaselineReport {
    pub fn from_captures(
        scenes: Vec<MapBaselineScene>,
        planned_captures: Vec<MapBaselinePlannedCapture>,
        asset_audit: MapAssetAudit,
        binding_audit: BindingAudit,
        elapsed_ms: f64,
    ) -> Self {
        Self {
            scenes,
            planned_captures,
            asset_audit,
            binding_audit,
            reference_source_root: None,
            elapsed_ms,
        }
    }

    fn project_png_count(&self) -> usize {
        self.planned_captures
            .iter()
            .filter(|capture| capture.project_png_exists)
            .count()
    }

    fn vanilla_reference_count(&self) -> usize {
        self.planned_captures
            .iter()
            .filter(|capture| capture.vanilla_reference_exists)
            .count()
    }

    fn diff_ready_count(&self) -> usize {
        self.planned_captures
            .iter()
            .filter(|capture| capture.diff_status == MapBaselineDiffStatus::Ready)
            .count()
    }

    fn diff_missing_reference_count(&self) -> usize {
        self.planned_captures
            .iter()
            .filter(|capture| capture.diff_status == MapBaselineDiffStatus::MissingReference)
            .count()
    }

    fn diff_missing_project_count(&self) -> usize {
        self.planned_captures
            .iter()
            .filter(|capture| capture.diff_status == MapBaselineDiffStatus::MissingProject)
            .count()
    }

    pub fn to_text_report(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(
            out,
            "[map-baseline] scenes={} layers={} planned_captures={} elapsed_ms={:.2}",
            self.scenes.len(),
            MapBaselineLayer::ALL.len(),
            self.planned_captures.len(),
            self.elapsed_ms
        );
        let _ = writeln!(
            out,
            "project_png={} vanilla_reference_png={} diff_ready={} diff_missing_reference={}",
            self.project_png_count(),
            self.vanilla_reference_count(),
            self.diff_ready_count(),
            self.diff_missing_reference_count()
        );
        let _ = writeln!(
            out,
            "diff_missing_project={}",
            self.diff_missing_project_count()
        );
        if let Some(root) = &self.reference_source_root {
            let _ = writeln!(out, "reference_source_root={root}");
        }
        let _ = writeln!(out, "{}", self.asset_audit.summary_line());
        let _ = writeln!(
            out,
            "visual_review_usable={}",
            self.asset_audit.can_use_for_visual_review()
        );
        out.push_str("\nscenes:\n");
        for scene in &self.scenes {
            let map_px = scene.map_px_center();
            let _ = writeln!(
                out,
                "  - {}: mode={} date={} target_uv={:.3},{:.3} target_map_px={:.0},{:.0} distance_factor={:.3} pitch_degrees={:.1} yaw_degrees={:.1}",
                scene.name,
                scene.map_mode,
                format_game_date(scene.date),
                scene.camera.target_uv[0],
                scene.camera.target_uv[1],
                map_px[0],
                map_px[1],
                scene.camera.distance_factor,
                scene.camera.pitch_degrees,
                scene.camera.yaw_degrees
            );
        }
        out.push_str("\nplanned_captures:\n");
        for capture in &self.planned_captures {
            let _ = write!(
                out,
                "  - {} asset_quality={} fallback={} visual_review_usable={} frame_time_ms={}",
                capture.filename,
                capture.asset_quality.as_str(),
                capture.asset_fallback_count,
                capture.visual_review_usable,
                capture
                    .frame_time_ms
                    .map(|ms| format!("{ms:.3}"))
                    .unwrap_or_else(|| "not_captured".to_string())
            );
            let _ = write!(
                out,
                " project_png={} vanilla_reference_png={} diff_status={} diff_report={}",
                capture.project_png_exists,
                capture.vanilla_reference_exists,
                capture.diff_status.as_str(),
                capture.diff_report_filename
            );
            if let Some(debug_view) = capture.layer.terrain_debug_view_name() {
                let _ = write!(out, " terrain_debug_view={debug_view}");
            }
            if let Some(source) = capture.layer.diagnostic_source() {
                let _ = write!(out, " diagnostic_source={source}");
            }
            if let Some(metrics) = capture.diff_metrics {
                let _ = write!(out, " {}", metrics.summary());
            }
            if let Some(err) = &capture.diff_error {
                let _ = write!(out, " diff_error={}", err);
            }
            out.push('\n');
        }
        out.push_str("\nasset_audit:\n");
        out.push_str(&self.asset_audit.to_text_report());
        out.push_str("\nbinding_audit:\n");
        out.push_str(&self.binding_audit.to_text_report());
        out
    }

    pub fn to_json_report(&self) -> String {
        let mut out = String::new();
        out.push_str("{\n");
        let _ = writeln!(out, "  \"phase\": \"0\",");
        out.push_str("  \"capture_type\": \"map_parity\",\n");
        out.push_str("  \"scene_config\": \"crates/hoi4-app/map_parity_scenes.tsv\",\n");
        out.push_str("  \"project_capture_root\": \"project\",\n");
        out.push_str("  \"vanilla_reference_root\": \"vanilla_reference\",\n");
        out.push_str("  \"diff_report_root\": \"diff\",\n");
        out.push_str("  \"reference_source_root\": ");
        write_json_string_option(&mut out, self.reference_source_root.as_deref());
        out.push_str(",\n");
        let _ = writeln!(
            out,
            "  \"project_png_count\": {},",
            self.project_png_count()
        );
        let _ = writeln!(
            out,
            "  \"vanilla_reference_png_count\": {},",
            self.vanilla_reference_count()
        );
        let _ = writeln!(out, "  \"diff_ready_count\": {},", self.diff_ready_count());
        let _ = writeln!(
            out,
            "  \"diff_missing_project_count\": {},",
            self.diff_missing_project_count()
        );
        let _ = writeln!(
            out,
            "  \"diff_missing_reference_count\": {},",
            self.diff_missing_reference_count()
        );
        let _ = writeln!(out, "  \"elapsed_ms\": {:.3},", self.elapsed_ms);
        out.push_str("  \"fallback_status\": ");
        write_fallback_status_json(
            &mut out,
            self.asset_audit.fallback,
            self.asset_audit.quality,
            self.asset_audit.can_use_for_visual_review(),
            2,
        );
        out.push_str(",\n");
        out.push_str("  \"scenes\": [\n");
        for (idx, scene) in self.scenes.iter().enumerate() {
            out.push_str("    {\n");
            let _ = writeln!(out, "      \"name\": \"{}\",", json_escape(&scene.name));
            let _ = writeln!(
                out,
                "      \"description\": \"{}\",",
                json_escape(&scene.description)
            );
            let _ = writeln!(out, "      \"map_mode\": \"{}\",", scene.map_mode);
            let _ = writeln!(
                out,
                "      \"date_hour\": \"{}\",",
                format_game_date(scene.date)
            );
            let map_px = scene.map_px_center();
            let _ = writeln!(
                out,
                "      \"camera\": {{ \"target_uv\": [{:.6}, {:.6}], \"target_map_px\": [{:.3}, {:.3}], \"distance_factor\": {:.6}, \"pitch_degrees\": {:.3}, \"yaw_degrees\": {:.3} }},",
                scene.camera.target_uv[0],
                scene.camera.target_uv[1],
                map_px[0],
                map_px[1],
                scene.camera.distance_factor,
                scene.camera.pitch_degrees,
                scene.camera.yaw_degrees
            );
            out.push_str("      \"enabled_layers\": ");
            write_layer_array_json(&mut out, &scene.enabled_layers);
            out.push('\n');
            out.push_str("    }");
            out.push_str(comma(idx + 1, self.scenes.len()));
            out.push('\n');
        }
        out.push_str("  ],\n");
        out.push_str("  \"captures\": [\n");
        for (idx, capture) in self.planned_captures.iter().enumerate() {
            out.push_str("    {\n");
            let _ = writeln!(
                out,
                "      \"scene_name\": \"{}\",",
                json_escape(&capture.scene_name)
            );
            let _ = writeln!(out, "      \"layer\": \"{}\",", capture.layer.as_str());
            let _ = writeln!(out, "      \"preset\": \"{}\",", capture.preset.as_str());
            let _ = writeln!(
                out,
                "      \"filename\": \"{}\",",
                json_escape(&capture.filename)
            );
            let _ = writeln!(
                out,
                "      \"vanilla_reference_filename\": \"{}\",",
                json_escape(&capture.vanilla_reference_filename)
            );
            let _ = writeln!(
                out,
                "      \"diff_report_filename\": \"{}\",",
                json_escape(&capture.diff_report_filename)
            );
            let _ = writeln!(
                out,
                "      \"project_png_exists\": {},",
                capture.project_png_exists
            );
            let _ = writeln!(
                out,
                "      \"vanilla_reference_exists\": {},",
                capture.vanilla_reference_exists
            );
            let _ = writeln!(
                out,
                "      \"diff_status\": \"{}\",",
                capture.diff_status.as_str()
            );
            out.push_str("      \"diff_metrics\": ");
            write_diff_metrics_json(&mut out, capture.diff_metrics, 6);
            out.push_str(",\n");
            out.push_str("      \"diff_error\": ");
            write_json_string_option(&mut out, capture.diff_error.as_deref());
            out.push_str(",\n");
            let _ = writeln!(
                out,
                "      \"visual_review_usable\": {},",
                capture.visual_review_usable
            );
            let _ = writeln!(
                out,
                "      \"asset_quality\": \"{}\",",
                capture.asset_quality.as_str()
            );
            let _ = writeln!(
                out,
                "      \"asset_fallback_count\": {},",
                capture.asset_fallback_count
            );
            match capture.frame_time_ms {
                Some(ms) => {
                    let _ = writeln!(out, "      \"frame_time_ms\": {:.3},", ms);
                }
                None => out.push_str("      \"frame_time_ms\": null,\n"),
            }
            out.push_str("      \"terrain_debug_view\": ");
            write_json_string_option(&mut out, capture.layer.terrain_debug_view_name());
            out.push_str(",\n");
            out.push_str("      \"diagnostic_source\": ");
            write_json_string_option(&mut out, capture.layer.diagnostic_source());
            out.push_str(",\n");
            let _ = writeln!(
                out,
                "      \"phase2_probe\": {},",
                capture.layer.is_phase2_probe()
            );
            write_layer_mask_json(&mut out, &capture.layer_mask);
            out.push_str(",\n");
            write_pass_status_json(&mut out, &capture.layer_mask);
            out.push_str(",\n");
            out.push_str("      \"fallback_status\": ");
            write_fallback_status_json(
                &mut out,
                capture.asset_fallback_count,
                capture.asset_quality,
                capture.visual_review_usable,
                6,
            );
            out.push('\n');
            out.push_str("    }");
            out.push_str(comma(idx + 1, self.planned_captures.len()));
            out.push('\n');
        }
        out.push_str("  ],\n");
        out.push_str("  \"asset_audit\": ");
        indent_json_object(&mut out, &self.asset_audit.to_json_report(), 2);
        out.push_str(",\n");
        out.push_str("  \"binding_audit\": ");
        indent_json_object(&mut out, &self.binding_audit.to_json_report(), 2);
        out.push('\n');
        out.push_str("}\n");
        out
    }
}

pub fn build_phase0_report(path_cfg: &PathConfig) -> MapBaselineReport {
    let started = Instant::now();
    let vanilla_resources = VanillaResourceViews::load_for_audit(path_cfg);
    let asset_audit = MapAssetAudit::from_map_set(&vanilla_resources.map_set);
    let binding_audit = vanilla_resources.phase1_binding_audit();
    let scenes = fixed_scenes();
    let planned_captures = build_phase0_planned_captures(&scenes, &asset_audit);
    MapBaselineReport {
        scenes,
        planned_captures,
        asset_audit,
        binding_audit,
        reference_source_root: None,
        elapsed_ms: started.elapsed().as_secs_f64() * 1000.0,
    }
}

pub fn build_phase0_planned_captures(
    scenes: &[MapBaselineScene],
    asset_audit: &MapAssetAudit,
) -> Vec<MapBaselinePlannedCapture> {
    let mut planned_captures = Vec::with_capacity(scenes.len() * MapBaselineLayer::ALL.len());
    for scene in scenes {
        for &layer in &scene.enabled_layers {
            planned_captures.push(MapBaselinePlannedCapture {
                scene_name: scene.name.clone(),
                layer,
                preset: MapBaselinePreset::High,
                filename: scene.screenshot_name(layer, MapBaselinePreset::High),
                vanilla_reference_filename: scene
                    .vanilla_reference_name(layer, MapBaselinePreset::High),
                diff_report_filename: scene.diff_report_name(layer, MapBaselinePreset::High),
                layer_mask: MapLayerMask::for_layer(layer),
                frame_time_ms: None,
                asset_quality: asset_audit.quality,
                asset_fallback_count: asset_audit.fallback,
                visual_review_usable: asset_audit.can_use_for_visual_review(),
                project_png_exists: false,
                vanilla_reference_exists: false,
                diff_status: MapBaselineDiffStatus::NotRun,
                diff_metrics: None,
                diff_error: None,
            });
        }
    }
    planned_captures
}

pub fn build_asset_audit(path_cfg: &PathConfig) -> MapAssetAudit {
    let db = FsAssetDb::new(path_cfg.clone());
    let map_set = VanillaMapSet::load_for_audit(&db);
    MapAssetAudit::from_map_set(&map_set)
}

pub fn write_phase0_report(
    path_cfg: &PathConfig,
    output_dir: &Path,
    reference_root: Option<&Path>,
) -> std::io::Result<PathBuf> {
    let report = build_phase0_report(path_cfg);
    write_phase0_report_files_with_references(&report, output_dir, reference_root)
}

pub fn write_map_audit(path_cfg: &PathConfig, output_dir: &Path) -> std::io::Result<PathBuf> {
    let vanilla_resources = VanillaResourceViews::load_for_audit(path_cfg);
    let asset_audit = MapAssetAudit::from_map_set(&vanilla_resources.map_set);
    let binding_audit = vanilla_resources.phase1_binding_audit();
    let posteffect_values_report =
        crate::passes::postprocess::build_posteffect_values_report_json(path_cfg);
    let terrain_pdxmap_report =
        crate::passes::terrain::build_terrain_pdxmap_report_json(&binding_audit);
    write_map_audit_files(
        &asset_audit,
        &binding_audit,
        Some(&posteffect_values_report),
        Some(&terrain_pdxmap_report),
        output_dir,
    )
}

pub fn write_map_audit_files(
    audit: &MapAssetAudit,
    binding_audit: &BindingAudit,
    posteffect_values_report: Option<&str>,
    terrain_pdxmap_report: Option<&str>,
    output_dir: &Path,
) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(output_dir)?;
    let text_path = output_dir.join("latest.txt");
    let json_path = output_dir.join("latest.json");
    std::fs::write(
        &text_path,
        combined_map_audit_text(audit, binding_audit, posteffect_values_report),
    )?;
    std::fs::write(
        &json_path,
        combined_map_audit_json(
            audit,
            binding_audit,
            posteffect_values_report,
            terrain_pdxmap_report,
        ),
    )?;
    std::fs::write(output_dir.join("asset_audit.json"), audit.to_json_report())?;
    std::fs::write(
        output_dir.join("binding_audit.json"),
        binding_audit.to_json_report(),
    )?;
    if let Some(report) = posteffect_values_report {
        std::fs::write(output_dir.join("posteffect_values.json"), report)?;
    }
    if let Some(report) = terrain_pdxmap_report {
        std::fs::write(output_dir.join("terrain_pdxmap.json"), report)?;
    }
    std::fs::write(
        output_dir.join("parity_gate.txt"),
        p0_parity_gate_text(audit, binding_audit),
    )?;
    std::fs::write(
        output_dir.join("parity_gate.json"),
        p0_parity_gate_json(audit, binding_audit),
    )?;
    Ok(json_path)
}

#[derive(Debug, Clone, Copy)]
struct P0MappingEntry {
    b1i_id: &'static str,
    reverse_fact: &'static str,
    evidence: &'static str,
    target_files: &'static str,
    current_status: &'static str,
    acceptance: &'static str,
}

const P0_MAPPING_ENTRIES: &[P0MappingEntry] = &[
    P0MappingEntry {
        b1i_id: "B1I-001",
        reverse_fact: "vanilla frame order is terrain -> border -> river -> map layers -> water -> border -> bounded overlays -> postprocess -> UI",
        evidence: "reverse_out/10_vanilla_render_spec_for_b1.md; reverse_out/exports/b1_implementation_inputs.tsv",
        target_files: "crates/hoi4-app/src/map_renderer.rs; crates/hoi4-app/src/map_draw.rs; crates/hoi4-app/src/passes/mod.rs",
        current_status: "partial",
        acceptance: "map-audit reports b1 pass order and fallback ownership; P1 must align exact order IDs",
    },
    P0MappingEntry {
        b1i_id: "B1I-002",
        reverse_fact: "main map scene renders to R16G16B16A16_FLOAT HDR before restorescene",
        evidence: "reverse_out/10_vanilla_render_spec_for_b1.md",
        target_files: "crates/hoi4-app/src/passes/hdr_target.rs; crates/hoi4-app/src/map_draw.rs",
        current_status: "partial",
        acceptance: "map-audit reports HDR target format and any non-HDR fallback",
    },
    P0MappingEntry {
        b1i_id: "B1I-003",
        reverse_fact: "postfx uses half-res HDR bloom/luminance targets and writes final LDR swapchain after restorescene",
        evidence: "reverse_out/10_vanilla_render_spec_for_b1.md; reverse_out/09_postprocess_lut_fog_daynight.md",
        target_files: "crates/hoi4-app/src/passes/postprocess.rs",
        current_status: "partial",
        acceptance: "postprocess audit reports restore path, luminance chain, and swapchain gamma policy",
    },
    P0MappingEntry {
        b1i_id: "B1I-009",
        reverse_fact: "pdxmap terrain binds TerrainDiffuse through GradientBorderChannel2 at vanilla s0-s15 semantics",
        evidence: "reverse_out/10_vanilla_render_spec_for_b1.md; reverse_out/exports/pdxmap_bindings.tsv",
        target_files: "crates/hoi4-app/src/passes/terrain.rs; crates/hoi4-app/src/passes/terrain.wgsl",
        current_status: "partial",
        acceptance: "terrain_pdxmap audit lists vanilla s0-s15 semantics and current Rust/WGSL bindings",
    },
    P0MappingEntry {
        b1i_id: "B1I-011",
        reverse_fact: "terrain formula order is TerrainIDMap decode, atlas blend, HeightNormal, tint, snow/mud, borders, secondary, lights, FOW, fog, day/night",
        evidence: "reverse_out/06_pdxmap_terrain_composition.md; reverse_out/exports/pdxmap_formula_notes.md",
        target_files: "crates/hoi4-render/src/translations/pdxmap.wgsl; crates/hoi4-app/src/passes/terrain.wgsl",
        current_status: "missing",
        acceptance: "required_changes marks legacy color/vignette/river/coast heuristics as P3 blockers",
    },
    P0MappingEntry {
        b1i_id: "B1I-013",
        reverse_fact: "GradientBorderChannel1/2 are B8G8R8A8_UNORM_SRGB 2816x2050 two-page runtime targets",
        evidence: "reverse_out/13_gradient_border_runtime_mapping.md; reverse_out/exports/gradient_border_channels.tsv",
        target_files: "crates/hoi4-app/src/vanilla_targets/gradient_border.rs; crates/hoi4-app/src/vanilla_targets/mod.rs",
        current_status: "fallback",
        acceptance: "runtime target audit reports format/size/two-page requirement and flags SDF approximation",
    },
    P0MappingEntry {
        b1i_id: "B1I-014",
        reverse_fact: "GradientBorder active anchors/page mappings come from runtime packed logical layer banks, not fixed country/province/state SDFs",
        evidence: "reverse_out/13_gradient_border_runtime_mapping.md; reverse_out/exports/gradient_border_map_mode_anchors.tsv",
        target_files: "crates/hoi4-app/src/vanilla_targets/gradient_border.rs",
        current_status: "fallback",
        acceptance: "parity gate reports packed-bank producer as missing until P4",
    },
    P0MappingEntry {
        b1i_id: "B1I-015",
        reverse_fact: "ProvinceSecondaryColorMap is B8G8R8A8_UNORM_SRGB 2816x1024 with RGB secondary overlay and alpha occupation stripe gate",
        evidence: "reverse_out/14_province_secondary_producer.md; reverse_out/exports/province_secondary_semantics.tsv",
        target_files: "crates/hoi4-app/src/vanilla_targets/province_secondary.rs; crates/hoi4-app/src/passes/terrain.rs",
        current_status: "partial",
        acceptance: "audit reports RGBA semantics and whether alpha stripe gate is vanilla-backed",
    },
    P0MappingEntry {
        b1i_id: "B1I-016",
        reverse_fact: "SnowMudData, LightIndexMap, LightDataMap, CityLightsAndSnowNoise, and FOW feed terrain lighting/weather",
        evidence: "reverse_out/07_dynamic_map_targets.md; reverse_out/exports/mud_snow_light_fow_targets.tsv",
        target_files: "crates/hoi4-app/src/vanilla_targets/point_lights.rs; crates/hoi4-app/src/vanilla_targets/mod.rs",
        current_status: "partial",
        acceptance: "binding audit lists each dynamic target producer/consumer and fallback status",
    },
    P0MappingEntry {
        b1i_id: "B1I-017",
        reverse_fact: "water writes the HDR target with depth enabled, depth write off, func 4, blend state 1620, and traced raster state",
        evidence: "reverse_out/08_water_river_pipeline.md; reverse_out/exports/pdxwater_constants.tsv",
        target_files: "crates/hoi4-app/src/passes/water.rs; crates/hoi4-app/src/map_draw.rs",
        current_status: "implemented",
        acceptance: "map-audit reports water effect selector plus depth/write/blend state for the P5 HDR contributor path",
    },
    P0MappingEntry {
        b1i_id: "B1I-018",
        reverse_fact: "pdxwater binds HeightTexture, LEAN, ProvinceSecondary, Specular/FOW, refraction, ice, reflection cube, snow/mud, lights, gradients, shadow",
        evidence: "reverse_out/08_water_river_pipeline.md; reverse_out/exports/pdxwater_bindings.tsv",
        target_files: "crates/hoi4-app/src/passes/water.rs; crates/hoi4-render/src/translations/pdxwater.wgsl",
        current_status: "partial",
        acceptance: "water audit binds ShadowMap and flags WaterRefraction/ReflectionCubeMap as explicit degraded fallbacks",
    },
    P0MappingEntry {
        b1i_id: "B1I-019",
        reverse_fact: "water selector chooses water_low_gfx, water_no_refractions, or water from graphics/refraction booleans",
        evidence: "reverse_out/12_cpu_selector_dynamic_closure.md; reverse_out/exports/pdxwater_constants.tsv",
        target_files: "crates/hoi4-app/src/passes/water.rs",
        current_status: "implemented",
        acceptance: "map-audit reports the default water_no_refractions selector and selector unit tests cover all variants",
    },
    P0MappingEntry {
        b1i_id: "B1I-020",
        reverse_fact: "river is an independent HDR contributor at order 83, after terrain and before water",
        evidence: "reverse_out/08_water_river_pipeline.md; reverse_out/10_vanilla_render_spec_for_b1.md",
        target_files: "crates/hoi4-app/src/passes/river.rs; crates/hoi4-app/src/map_draw.rs",
        current_status: "implemented",
        acceptance: "map-audit reports dedicated order-83 river pass, depth disabled state, and terrain river overlay fallback status",
    },
    P0MappingEntry {
        b1i_id: "B1I-021",
        reverse_fact: "river bindings include diffuse/normal/masks, LEAN, lights, gradients, province secondary, reflection cube, and shadow",
        evidence: "reverse_out/exports/river_bindings.tsv",
        target_files: "crates/hoi4-app/src/passes/river.rs",
        current_status: "partial",
        acceptance: "binding audit lists river material/runtime target bindings and flags reflection fallback explicitly",
    },
    P0MappingEntry {
        b1i_id: "B1I-022",
        reverse_fact: "restorescene combines MainScene, RestoreBloom, AverageLuminanceTexture, and ColorCube",
        evidence: "reverse_out/09_postprocess_lut_fog_daynight.md; reverse_out/exports/postfx_pass_order.tsv",
        target_files: "crates/hoi4-app/src/passes/postprocess.rs; crates/hoi4-render/src/translations/restorescene.wgsl",
        current_status: "partial",
        acceptance: "posteffect audit reports Uncharted2 restore path and non-ACES default",
    },
    P0MappingEntry {
        b1i_id: "B1I-023",
        reverse_fact: "ColorCube is a 1024x32 flattened 32x32x32 LUT selected from colorcorrection*.tga",
        evidence: "reverse_out/09_postprocess_lut_fog_daynight.md; reverse_out/exports/colorcube_lut_selection.tsv",
        target_files: "crates/hoi4-app/src/passes/postprocess.rs; crates/hoi4-assets/src/tga.rs; crates/hoi4-assets/src/gfx.rs",
        current_status: "partial",
        acceptance: "map-audit reports loaded LUT candidate count and identity fallback status",
    },
    P0MappingEntry {
        b1i_id: "B1I-024",
        reverse_fact: "posteffect LUT selection branches on water, camera distance, winter, and night thresholds from R17",
        evidence: "reverse_out/17_final_unknown_closure.md; reverse_out/exports/colorcube_lut_selection.tsv",
        target_files: "crates/hoi4-app/src/passes/postprocess.rs",
        current_status: "partial",
        acceptance: "posteffect audit exposes water/far/night/winter selection keys",
    },
    P0MappingEntry {
        b1i_id: "B1I-025",
        reverse_fact: "distance fog uses squared distance, view factor, and fog color 0.12,0.28,0.6",
        evidence: "reverse_out/09_postprocess_lut_fog_daynight.md; reverse_out/exports/fog_daynight_constants.tsv",
        target_files: "crates/hoi4-render/src/shader_lib.wgsl; crates/hoi4-render/src/global_uniform.rs",
        current_status: "partial",
        acceptance: "shader tests cover apply_distance_fog and map-audit lists fog constants",
    },
    P0MappingEntry {
        b1i_id: "B1I-026",
        reverse_fact: "day/night is applied in terrain/water/river shader-local paths using traced cbuffer fields",
        evidence: "reverse_out/09_postprocess_lut_fog_daynight.md; reverse_out/exports/cbuffer_field_live_values.tsv",
        target_files: "crates/hoi4-render/src/shader_lib.wgsl; crates/hoi4-render/src/global_uniform.rs",
        current_status: "partial",
        acceptance: "shader tests cover day_night and audit maps global uniform vanilla fields",
    },
    P0MappingEntry {
        b1i_id: "B1I-032",
        reverse_fact: "ShadowMap/projected FOW is a full-res B8G8R8A8_UNORM screen target followed by two-pass blur",
        evidence: "reverse_out/16_shadow_fow_projected_producer.md; reverse_out/exports/shadow_fow_projected_producer.tsv",
        target_files: "crates/hoi4-app/src/passes/terrain.rs; crates/hoi4-app/src/passes/water.rs; crates/hoi4-app/src/vanilla_targets/mod.rs",
        current_status: "fallback",
        acceptance: "map-audit reports the full-res ShadowMap target and labels the neutral projected FOW producer as explicit fallback",
    },
];

#[derive(Debug, Clone, Copy)]
struct P0GateSummary {
    total: usize,
    implemented: usize,
    partial: usize,
    fallback: usize,
    missing: usize,
    asset_fallback_count: usize,
    critical_asset_fallback_count: usize,
    binding_fallback_count: usize,
    critical_binding_fallback_count: usize,
    producer_non_equivalent_count: usize,
    critical_fallback_count: usize,
    critical_fallback_silent_pass: bool,
}

impl P0GateSummary {
    fn from_audits(audit: &MapAssetAudit, binding_audit: &BindingAudit) -> Self {
        let implemented = P0_MAPPING_ENTRIES
            .iter()
            .filter(|entry| entry.current_status == "implemented")
            .count();
        let partial = P0_MAPPING_ENTRIES
            .iter()
            .filter(|entry| entry.current_status == "partial")
            .count();
        let fallback = P0_MAPPING_ENTRIES
            .iter()
            .filter(|entry| entry.current_status == "fallback")
            .count();
        let missing = P0_MAPPING_ENTRIES
            .iter()
            .filter(|entry| entry.current_status == "missing")
            .count();
        let critical_asset_fallback_count = audit.invalid_visual_review_paths.len();
        let critical_binding_fallback_count = binding_audit.critical_count();
        let producer_non_equivalent_count = binding_audit.producer_non_equivalent_count();
        let critical_fallback_count =
            critical_asset_fallback_count + critical_binding_fallback_count;

        Self {
            total: P0_MAPPING_ENTRIES.len(),
            implemented,
            partial,
            fallback,
            missing,
            asset_fallback_count: audit.fallback,
            critical_asset_fallback_count,
            binding_fallback_count: binding_audit.fallback_count(),
            critical_binding_fallback_count,
            producer_non_equivalent_count,
            critical_fallback_count,
            critical_fallback_silent_pass: false,
        }
    }

    fn status(self) -> &'static str {
        if self.critical_fallback_count == 0 && self.producer_non_equivalent_count == 0 {
            "pass"
        } else {
            "blocked"
        }
    }
}

fn p0_parity_gate_text(audit: &MapAssetAudit, binding_audit: &BindingAudit) -> String {
    let summary = P0GateSummary::from_audits(audit, binding_audit);
    let mut out = String::new();
    out.push_str("p0_parity_gate:\n");
    let _ = writeln!(out, "  status={}", summary.status());
    let _ = writeln!(
        out,
        "  mapping_total={} implemented={} partial={} fallback={} missing={}",
        summary.total, summary.implemented, summary.partial, summary.fallback, summary.missing
    );
    let _ = writeln!(
        out,
        "  fallback asset={} critical_asset={} binding={} critical_binding={} critical_total={}",
        summary.asset_fallback_count,
        summary.critical_asset_fallback_count,
        summary.binding_fallback_count,
        summary.critical_binding_fallback_count,
        summary.critical_fallback_count
    );
    let _ = writeln!(
        out,
        "  producer_non_equivalent={}",
        summary.producer_non_equivalent_count
    );
    let _ = writeln!(
        out,
        "  critical_fallback_silent_pass={}",
        summary.critical_fallback_silent_pass
    );
    out.push_str("  evidence_files=reverse_out/10_vanilla_render_spec_for_b1.md; reverse_out/exports/b1_implementation_inputs.tsv\n");
    out.push_str("  required_docs=tools/vanilla_trace/project_mapping.md; tools/vanilla_trace/b1_required_changes.md; tools/vanilla_trace/parity_gate.md\n");
    out.push_str("  acceptance=cargo test -p hoi4-render shader_lib_compiles; cargo test -p hoi4-app terrain; cargo run -p hoi4-app -- --map-audit\n");
    if !audit.invalid_visual_review_paths.is_empty() {
        out.push_str("  critical_asset_fallbacks:\n");
        for path in audit.invalid_visual_review_paths.iter().take(32) {
            let _ = writeln!(out, "    - {}", path);
        }
        if audit.invalid_visual_review_paths.len() > 32 {
            let _ = writeln!(
                out,
                "    - ... {} more",
                audit.invalid_visual_review_paths.len() - 32
            );
        }
    }
    let critical_entries: Vec<_> = binding_audit.critical_entries().collect();
    let producer_non_equivalent_entries: Vec<_> = binding_audit
        .entries
        .iter()
        .filter(|entry| entry.binding_present && !entry.producer_equivalent)
        .collect();
    if !critical_entries.is_empty() {
        out.push_str("  critical_binding_fallbacks:\n");
        for entry in critical_entries.iter().take(32) {
            let _ = writeln!(
                out,
                "    - {}.{} source={} reason={}",
                entry.pass,
                entry.binding,
                entry.source_kind.as_str(),
                entry.reason.as_deref().unwrap_or("unspecified")
            );
        }
        if critical_entries.len() > 32 {
            let _ = writeln!(out, "    - ... {} more", critical_entries.len() - 32);
        }
    }
    if !producer_non_equivalent_entries.is_empty() {
        out.push_str("  producer_non_equivalent_entries:\n");
        for entry in producer_non_equivalent_entries.iter().take(32) {
            let _ = writeln!(
                out,
                "    - {}.{} target={} provenance={} producer_status={} parity_status={}",
                entry.pass,
                entry.binding,
                entry.source_name,
                entry.provenance.as_str(),
                entry.producer_status.as_deref().unwrap_or("unspecified"),
                entry.parity_status.as_deref().unwrap_or("unspecified")
            );
        }
        if producer_non_equivalent_entries.len() > 32 {
            let _ = writeln!(
                out,
                "    - ... {} more",
                producer_non_equivalent_entries.len() - 32
            );
        }
    }
    out.push_str("  b1i_mapping:\n");
    for entry in P0_MAPPING_ENTRIES {
        let _ = writeln!(
            out,
            "    - {} status={} fact={} target={} acceptance={}",
            entry.b1i_id,
            entry.current_status,
            entry.reverse_fact,
            entry.target_files,
            entry.acceptance
        );
    }
    out
}

fn p0_parity_gate_json(audit: &MapAssetAudit, binding_audit: &BindingAudit) -> String {
    let summary = P0GateSummary::from_audits(audit, binding_audit);
    let critical_entries: Vec<_> = binding_audit.critical_entries().collect();
    let producer_non_equivalent_entries: Vec<_> = binding_audit
        .entries
        .iter()
        .filter(|entry| entry.binding_present && !entry.producer_equivalent)
        .collect();
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str("  \"kind\": \"p0_parity_gate\",\n");
    let _ = writeln!(out, "  \"status\": \"{}\",", summary.status());
    let _ = writeln!(out, "  \"mapping_total\": {},", summary.total);
    let _ = writeln!(out, "  \"implemented\": {},", summary.implemented);
    let _ = writeln!(out, "  \"partial\": {},", summary.partial);
    let _ = writeln!(out, "  \"fallback\": {},", summary.fallback);
    let _ = writeln!(out, "  \"missing\": {},", summary.missing);
    let _ = writeln!(
        out,
        "  \"asset_fallback_count\": {},",
        summary.asset_fallback_count
    );
    let _ = writeln!(
        out,
        "  \"critical_asset_fallback_count\": {},",
        summary.critical_asset_fallback_count
    );
    let _ = writeln!(
        out,
        "  \"binding_fallback_count\": {},",
        summary.binding_fallback_count
    );
    let _ = writeln!(
        out,
        "  \"critical_binding_fallback_count\": {},",
        summary.critical_binding_fallback_count
    );
    let _ = writeln!(
        out,
        "  \"producer_non_equivalent_count\": {},",
        summary.producer_non_equivalent_count
    );
    let _ = writeln!(
        out,
        "  \"critical_fallback_count\": {},",
        summary.critical_fallback_count
    );
    let _ = writeln!(
        out,
        "  \"critical_fallback_silent_pass\": {},",
        summary.critical_fallback_silent_pass
    );
    out.push_str("  \"evidence_files\": [\n");
    out.push_str("    \"reverse_out/10_vanilla_render_spec_for_b1.md\",\n");
    out.push_str("    \"reverse_out/exports/b1_implementation_inputs.tsv\"\n");
    out.push_str("  ],\n");
    out.push_str("  \"required_docs\": [\n");
    out.push_str("    \"tools/vanilla_trace/project_mapping.md\",\n");
    out.push_str("    \"tools/vanilla_trace/b1_required_changes.md\",\n");
    out.push_str("    \"tools/vanilla_trace/parity_gate.md\"\n");
    out.push_str("  ],\n");
    out.push_str("  \"critical_asset_fallbacks\": [");
    for (idx, path) in audit.invalid_visual_review_paths.iter().enumerate() {
        if idx > 0 {
            out.push_str(", ");
        }
        let _ = write!(out, "\"{}\"", json_escape(path));
    }
    out.push_str("],\n");
    out.push_str("  \"critical_binding_fallbacks\": [\n");
    for (idx, entry) in critical_entries.iter().enumerate() {
        out.push_str("    {\n");
        let _ = writeln!(out, "      \"pass\": \"{}\",", json_escape(entry.pass));
        let _ = writeln!(
            out,
            "      \"binding\": \"{}\",",
            json_escape(entry.binding)
        );
        let _ = writeln!(out, "      \"source\": \"{}\",", entry.source_kind.as_str());
        let _ = writeln!(
            out,
            "      \"reason\": \"{}\"",
            json_escape(entry.reason.as_deref().unwrap_or("unspecified"))
        );
        let suffix = if idx + 1 < critical_entries.len() {
            "    },\n"
        } else {
            "    }\n"
        };
        out.push_str(suffix);
    }
    out.push_str("  ],\n");
    out.push_str("  \"producer_non_equivalent_entries\": [\n");
    for (idx, entry) in producer_non_equivalent_entries.iter().enumerate() {
        out.push_str("    {\n");
        let _ = writeln!(out, "      \"pass\": \"{}\",", json_escape(entry.pass));
        let _ = writeln!(
            out,
            "      \"binding\": \"{}\",",
            json_escape(entry.binding)
        );
        let _ = writeln!(
            out,
            "      \"target\": \"{}\",",
            json_escape(&entry.source_name)
        );
        let _ = writeln!(
            out,
            "      \"provenance\": \"{}\",",
            entry.provenance.as_str()
        );
        let _ = writeln!(
            out,
            "      \"producer_status\": \"{}\",",
            json_escape(entry.producer_status.as_deref().unwrap_or("unspecified"))
        );
        let _ = writeln!(
            out,
            "      \"parity_status\": \"{}\"",
            json_escape(entry.parity_status.as_deref().unwrap_or("unspecified"))
        );
        let suffix = if idx + 1 < producer_non_equivalent_entries.len() {
            "    },\n"
        } else {
            "    }\n"
        };
        out.push_str(suffix);
    }
    out.push_str("  ],\n");
    out.push_str("  \"b1i_mapping\": [\n");
    for (idx, entry) in P0_MAPPING_ENTRIES.iter().enumerate() {
        out.push_str("    {\n");
        let _ = writeln!(out, "      \"b1i_id\": \"{}\",", entry.b1i_id);
        let _ = writeln!(
            out,
            "      \"reverse_fact\": \"{}\",",
            json_escape(entry.reverse_fact)
        );
        let _ = writeln!(
            out,
            "      \"evidence\": \"{}\",",
            json_escape(entry.evidence)
        );
        let _ = writeln!(
            out,
            "      \"target_files\": \"{}\",",
            json_escape(entry.target_files)
        );
        let _ = writeln!(
            out,
            "      \"current_status\": \"{}\",",
            entry.current_status
        );
        let _ = writeln!(
            out,
            "      \"acceptance\": \"{}\"",
            json_escape(entry.acceptance)
        );
        let suffix = if idx + 1 < P0_MAPPING_ENTRIES.len() {
            "    },\n"
        } else {
            "    }\n"
        };
        out.push_str(suffix);
    }
    out.push_str("  ]\n");
    out.push_str("}\n");
    out
}

pub fn write_phase0_report_files(
    report: &MapBaselineReport,
    output_dir: &Path,
) -> std::io::Result<PathBuf> {
    write_phase0_report_files_with_references(report, output_dir, None)
}

pub fn write_phase0_report_files_with_references(
    report: &MapBaselineReport,
    output_dir: &Path,
    reference_root: Option<&Path>,
) -> std::io::Result<PathBuf> {
    let mut report = report.clone();
    report.reference_source_root = reference_root.map(|root| root.display().to_string());
    std::fs::create_dir_all(output_dir)?;
    for scene in &report.scenes {
        std::fs::create_dir_all(output_dir.join("project").join(&scene.name))?;
        std::fs::create_dir_all(output_dir.join("vanilla_reference").join(&scene.name))?;
        std::fs::create_dir_all(output_dir.join("diff").join(&scene.name))?;
    }
    copy_phase0_reference_pngs(&mut report, output_dir, reference_root)?;
    refresh_phase0_artifact_status(&mut report, output_dir)?;
    std::fs::write(
        output_dir.join("vanilla_reference").join("README.txt"),
        vanilla_reference_readme(&report),
    )?;
    let text_path = output_dir.join("report.txt");
    let json_path = output_dir.join("report.json");
    let audit_json_path = output_dir.join("asset_audit.json");
    let audit_text_path = output_dir.join("asset_audit.txt");
    let binding_json_path = output_dir.join("binding_audit.json");
    let binding_text_path = output_dir.join("binding_audit.txt");
    std::fs::write(&text_path, report.to_text_report())?;
    std::fs::write(&json_path, report.to_json_report())?;
    std::fs::write(&audit_json_path, report.asset_audit.to_json_report())?;
    std::fs::write(&audit_text_path, report.asset_audit.to_text_report())?;
    std::fs::write(&binding_json_path, report.binding_audit.to_json_report())?;
    std::fs::write(&binding_text_path, report.binding_audit.to_text_report())?;
    std::fs::write(
        output_dir.join("phase0_latest.txt"),
        report.to_text_report(),
    )?;
    std::fs::write(
        output_dir.join("phase0_latest.json"),
        report.to_json_report(),
    )?;
    std::fs::write(
        output_dir.join("asset_audit_latest.json"),
        report.asset_audit.to_json_report(),
    )?;
    std::fs::write(
        output_dir.join("asset_audit_latest.txt"),
        report.asset_audit.to_text_report(),
    )?;
    std::fs::write(
        output_dir.join("binding_audit_latest.json"),
        report.binding_audit.to_json_report(),
    )?;
    std::fs::write(
        output_dir.join("binding_audit_latest.txt"),
        report.binding_audit.to_text_report(),
    )?;
    Ok(json_path)
}

fn copy_phase0_reference_pngs(
    report: &mut MapBaselineReport,
    output_dir: &Path,
    reference_root: Option<&Path>,
) -> std::io::Result<()> {
    let Some(reference_root) = reference_root else {
        return Ok(());
    };

    for capture in &report.planned_captures {
        let Some(source) = resolve_reference_source(reference_root, capture) else {
            continue;
        };
        let dest = output_dir.join(&capture.vanilla_reference_filename);
        if same_existing_path(&source, &dest) {
            continue;
        }
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(&source, &dest)?;
    }
    Ok(())
}

fn resolve_reference_source(
    reference_root: &Path,
    capture: &MapBaselinePlannedCapture,
) -> Option<PathBuf> {
    let direct = reference_root.join(&capture.vanilla_reference_filename);
    if direct.is_file() {
        return Some(direct);
    }

    let relative_without_root = capture
        .vanilla_reference_filename
        .strip_prefix("vanilla_reference/")
        .unwrap_or(&capture.vanilla_reference_filename);
    let nested = reference_root.join(relative_without_root);
    if nested.is_file() {
        Some(nested)
    } else {
        None
    }
}

fn same_existing_path(a: &Path, b: &Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

fn refresh_phase0_artifact_status(
    report: &mut MapBaselineReport,
    output_dir: &Path,
) -> std::io::Result<()> {
    for capture in &mut report.planned_captures {
        let project = output_dir.join(&capture.filename);
        let reference = output_dir.join(&capture.vanilla_reference_filename);
        let diff_report = output_dir.join(&capture.diff_report_filename);

        capture.project_png_exists = project.is_file();
        capture.vanilla_reference_exists = reference.is_file();
        capture.diff_metrics = None;
        capture.diff_error = None;

        capture.diff_status = if !capture.project_png_exists {
            MapBaselineDiffStatus::MissingProject
        } else if !capture.vanilla_reference_exists {
            MapBaselineDiffStatus::MissingReference
        } else {
            match crate::map_image_diff::diff_png_files(&project, &reference) {
                Ok(metrics) => {
                    capture.diff_metrics = Some(metrics);
                    MapBaselineDiffStatus::Ready
                }
                Err(err) => {
                    capture.diff_error = Some(err);
                    MapBaselineDiffStatus::Failed
                }
            }
        };

        write_phase0_capture_diff_report(capture, &project, &reference, &diff_report)?;
    }
    Ok(())
}

fn write_phase0_capture_diff_report(
    capture: &MapBaselinePlannedCapture,
    project: &Path,
    reference: &Path,
    output_path: &Path,
) -> std::io::Result<()> {
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut out = String::new();
    out.push_str("{\n");
    out.push_str("  \"phase\": \"0\",\n");
    out.push_str("  \"kind\": \"map_parity_diff\",\n");
    let _ = writeln!(
        out,
        "  \"scene_name\": \"{}\",",
        json_escape(&capture.scene_name)
    );
    let _ = writeln!(out, "  \"layer\": \"{}\",", capture.layer.as_str());
    let _ = writeln!(out, "  \"preset\": \"{}\",", capture.preset.as_str());
    let _ = writeln!(
        out,
        "  \"project\": \"{}\",",
        json_escape(&project.display().to_string())
    );
    let _ = writeln!(
        out,
        "  \"reference\": \"{}\",",
        json_escape(&reference.display().to_string())
    );
    let _ = writeln!(
        out,
        "  \"project_png_exists\": {},",
        capture.project_png_exists
    );
    let _ = writeln!(
        out,
        "  \"vanilla_reference_exists\": {},",
        capture.vanilla_reference_exists
    );
    let _ = writeln!(out, "  \"status\": \"{}\",", capture.diff_status.as_str());
    out.push_str("  \"metrics\": ");
    write_diff_metrics_json(&mut out, capture.diff_metrics, 2);
    out.push_str(",\n");
    out.push_str("  \"error\": ");
    write_json_string_option(&mut out, capture.diff_error.as_deref());
    out.push('\n');
    out.push_str("}\n");

    std::fs::write(output_path, out)
}

fn combined_map_audit_text(
    audit: &MapAssetAudit,
    binding_audit: &BindingAudit,
    posteffect_values_report: Option<&str>,
) -> String {
    let mut out = audit.to_text_report();
    out.push_str("\nbinding_audit:\n");
    out.push_str(&binding_audit.to_text_report());
    if let Some(report) = posteffect_values_report {
        out.push_str("\nposteffect_values:\n");
        out.push_str(report);
        if !report.ends_with('\n') {
            out.push('\n');
        }
    }
    out.push('\n');
    out.push_str(&p1_frame_graph_audit_text());
    out.push('\n');
    out.push_str(&p0_parity_gate_text(audit, binding_audit));
    out
}

fn combined_map_audit_json(
    audit: &MapAssetAudit,
    binding_audit: &BindingAudit,
    posteffect_values_report: Option<&str>,
    terrain_pdxmap_report: Option<&str>,
) -> String {
    let mut out = audit.to_json_report();
    while out.ends_with('\n') {
        out.pop();
    }
    if out.ends_with('}') {
        out.pop();
    }
    out.push_str(",\n  \"binding_audit\": ");
    indent_json_object(&mut out, &binding_audit.to_json_report(), 2);
    if let Some(report) = posteffect_values_report {
        out.push_str(",\n  \"posteffect_values\": ");
        indent_json_object(&mut out, report, 2);
    }
    if let Some(report) = terrain_pdxmap_report {
        out.push_str(",\n  \"terrain_pdxmap\": ");
        indent_json_object(&mut out, report, 2);
    }
    out.push_str(",\n  \"p1_frame_graph\": ");
    indent_json_object(&mut out, &p1_frame_graph_audit_json(), 2);
    out.push_str(",\n  \"p0_parity_gate\": ");
    indent_json_object(&mut out, &p0_parity_gate_json(audit, binding_audit), 2);
    out.push_str("\n}\n");
    out
}

#[derive(Debug, Clone, Copy)]
struct P1FrameGraphEntry {
    b1_pass: &'static str,
    vanilla_order: &'static str,
    target: &'static str,
    role: &'static str,
    status: &'static str,
}

const P1_FRAME_GRAPH_ENTRIES: &[P1FrameGraphEntry] = &[
    P1FrameGraphEntry {
        b1_pass: "3d_terrain",
        vanilla_order: "81",
        target: "main_hdr_scene",
        role: "pdxmap terrain",
        status: "implemented_submit_point",
    },
    P1FrameGraphEntry {
        b1_pass: "3d_border_first",
        vanilla_order: "82",
        target: "main_hdr_scene",
        role: "border first",
        status: "explicit_submit_point_fallback_geometry_pending",
    },
    P1FrameGraphEntry {
        b1_pass: "3d_river",
        vanilla_order: "83",
        target: "main_hdr_scene_no_depth_pass",
        role: "river independent HDR contributor",
        status: "implemented_submit_point_depth_disabled",
    },
    P1FrameGraphEntry {
        b1_pass: "3d_map_layers",
        vanilla_order: "84-85",
        target: "main_hdr_scene",
        role: "additional terrain/map layers",
        status: "explicit_submit_point_pending_traced_layer_bindings",
    },
    P1FrameGraphEntry {
        b1_pass: "3d_water",
        vanilla_order: "86",
        target: "main_hdr_scene",
        role: "pdxwater",
        status: "implemented_submit_point_state_partial",
    },
    P1FrameGraphEntry {
        b1_pass: "3d_border_second",
        vanilla_order: "87",
        target: "main_hdr_scene",
        role: "border second",
        status: "implemented_submit_point_strip_fallback",
    },
    P1FrameGraphEntry {
        b1_pass: "bounded_overlay_object_family",
        vanilla_order: "88-133",
        target: "main_hdr_scene",
        role: "overlays, map layers, objects, reflection/environment families",
        status: "bounded_family_only_order_133_not_second_full_water",
    },
    P1FrameGraphEntry {
        b1_pass: "postprocess",
        vanilla_order: "134-139",
        target: "swapchain",
        role: "restore scene after HDR map chain",
        status: "implemented_submit_point",
    },
    P1FrameGraphEntry {
        b1_pass: "ui",
        vanilla_order: "140+",
        target: "swapchain",
        role: "self-authored UI/sprite overlay after restore",
        status: "outside_hdr_postprocess",
    },
];

fn p1_frame_graph_audit_text() -> String {
    let mut out = String::new();
    out.push_str("p1_frame_graph:\n");
    out.push_str("  evidence=B1I-001; B1I-002; B1I-003; B1I-027; reverse_out/10_vanilla_render_spec_for_b1.md\n");
    out.push_str("  main_hdr_scene_format=Rgba16Float vanilla=R16G16B16A16_FLOAT unique_core_map_color_target=true\n");
    out.push_str(
        "  depth_format=Depth24PlusStencil8 vanilla=D24_UNORM_S8_UINT stencil_available=true\n",
    );
    out.push_str(
        "  postprocess_restores_to_swapchain=true ui_enters_hdr=false ui_after_restore=true\n",
    );
    out.push_str(
        "  order_133_policy=environment_reflection_family_only not_second_full_water=true\n",
    );
    out.push_str("  pass_order:\n");
    for entry in P1_FRAME_GRAPH_ENTRIES {
        let _ = writeln!(
            out,
            "    - b1_pass={} vanilla_order={} target={} role={} status={}",
            entry.b1_pass, entry.vanilla_order, entry.target, entry.role, entry.status
        );
    }
    out
}

fn p1_frame_graph_audit_json() -> String {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str("  \"kind\": \"p1_frame_graph\",\n");
    out.push_str("  \"evidence\": [\"B1I-001\", \"B1I-002\", \"B1I-003\", \"B1I-027\", \"reverse_out/10_vanilla_render_spec_for_b1.md\"],\n");
    out.push_str("  \"main_hdr_scene\": {\n");
    out.push_str("    \"format\": \"Rgba16Float\",\n");
    out.push_str("    \"vanilla_format\": \"R16G16B16A16_FLOAT\",\n");
    out.push_str("    \"unique_core_map_color_target\": true\n");
    out.push_str("  },\n");
    out.push_str("  \"depth\": {\n");
    out.push_str("    \"format\": \"Depth24PlusStencil8\",\n");
    out.push_str("    \"vanilla_format\": \"D24_UNORM_S8_UINT\",\n");
    out.push_str("    \"stencil_available\": true\n");
    out.push_str("  },\n");
    out.push_str("  \"postprocess_restores_to_swapchain\": true,\n");
    out.push_str("  \"ui_enters_hdr\": false,\n");
    out.push_str("  \"ui_after_restore\": true,\n");
    out.push_str(
        "  \"order_133_policy\": \"environment_reflection_family_only_not_second_full_water\",\n",
    );
    out.push_str("  \"pass_order\": [\n");
    for (idx, entry) in P1_FRAME_GRAPH_ENTRIES.iter().enumerate() {
        out.push_str("    {\n");
        let _ = writeln!(out, "      \"b1_pass\": \"{}\",", entry.b1_pass);
        let _ = writeln!(out, "      \"vanilla_order\": \"{}\",", entry.vanilla_order);
        let _ = writeln!(out, "      \"target\": \"{}\",", entry.target);
        let _ = writeln!(out, "      \"role\": \"{}\",", json_escape(entry.role));
        let _ = writeln!(out, "      \"status\": \"{}\"", entry.status);
        let suffix = if idx + 1 < P1_FRAME_GRAPH_ENTRIES.len() {
            "    },\n"
        } else {
            "    }\n"
        };
        out.push_str(suffix);
    }
    out.push_str("  ]\n");
    out.push_str("}\n");
    out
}

fn write_layer_mask_json(out: &mut String, mask: &MapLayerMask) {
    out.push_str("      \"layer_mask\": { ");
    let _ = write!(
        out,
        "\"sky\": {}, \"terrain\": {}, \"water\": {}, \"river\": {}, \"borders\": {}, \"static_decals\": {}, \"overlays\": {}, \"objects\": {}, \"labels\": {}, \"particles\": {}, \"ui\": {}, \"postprocess\": {}, \"asset_fallback_debug\": {}",
        mask.sky,
        mask.terrain,
        mask.water,
        mask.river,
        mask.borders,
        mask.static_decals,
        mask.overlays,
        mask.objects,
        mask.labels,
        mask.particles,
        mask.ui,
        mask.postprocess,
        mask.asset_fallback_debug
    );
    out.push_str(" }");
}

fn write_pass_status_json(out: &mut String, mask: &MapLayerMask) {
    out.push_str("      \"pass_status\": { ");
    let _ = write!(
        out,
        "\"terrain\": {}, \"water\": {}, \"river\": {}, \"borders\": {}, \"static_decals\": {}, \"overlays\": {}, \"objects\": {}, \"labels\": {}, \"postprocess\": {}, \"ui\": {}",
        mask.terrain,
        mask.water,
        mask.river,
        mask.borders,
        mask.static_decals,
        mask.overlays,
        mask.objects,
        mask.labels,
        mask.postprocess,
        mask.ui,
    );
    out.push_str(" }");
}

fn write_fallback_status_json(
    out: &mut String,
    fallback_count: usize,
    quality: MapAssetQuality,
    visual_review_usable: bool,
    spaces: usize,
) {
    let pad = " ".repeat(spaces);
    let _ = write!(
        out,
        "{{\n{pad}  \"asset_fallback_count\": {},\n{pad}  \"asset_quality\": \"{}\",\n{pad}  \"visual_review_usable\": {}\n{pad}}}",
        fallback_count,
        quality.as_str(),
        visual_review_usable
    );
}

fn write_diff_metrics_json(out: &mut String, metrics: Option<ImageDiffMetrics>, spaces: usize) {
    let Some(metrics) = metrics else {
        out.push_str("null");
        return;
    };
    let pad = " ".repeat(spaces);
    let _ = write!(
        out,
        "{{\n{pad}  \"width\": {},\n{pad}  \"height\": {},\n{pad}  \"pixels\": {},\n{pad}  \"ssim_luma\": {:.8},\n{pad}  \"average_color_delta\": {:.8},\n{pad}  \"luma_delta\": {:.8},\n{pad}  \"edge_delta\": {:.8}\n{pad}}}",
        metrics.width,
        metrics.height,
        metrics.pixels,
        metrics.ssim_luma,
        metrics.average_color_delta,
        metrics.luma_delta,
        metrics.edge_delta
    );
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

fn write_layer_array_json(out: &mut String, layers: &[MapBaselineLayer]) {
    out.push('[');
    for (idx, layer) in layers.iter().enumerate() {
        if idx > 0 {
            out.push_str(", ");
        }
        let _ = write!(out, "\"{}\"", layer.as_str());
    }
    out.push(']');
}

pub fn phase0_batch_output_dir(root: &Path) -> PathBuf {
    root.join(timestamp_utc_yyyymmdd_hhmmss())
}

fn timestamp_utc_yyyymmdd_hhmmss() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let days = now.div_euclid(86_400);
    let seconds = now.rem_euclid(86_400);
    let (year, month, day) = civil_from_unix_days(days);
    let hour = seconds / 3600;
    let minute = (seconds % 3600) / 60;
    let second = seconds % 60;
    format!("{year:04}{month:02}{day:02}_{hour:02}{minute:02}{second:02}")
}

fn civil_from_unix_days(days: i64) -> (i32, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 }.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096).div_euclid(365);
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2).div_euclid(153);
    let d = doy - (153 * mp + 2).div_euclid(5) + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    let year = y + if m <= 2 { 1 } else { 0 };
    (year as i32, m as u32, d as u32)
}

fn parse_scene_config(input: &str) -> Result<Vec<MapBaselineScene>, String> {
    let mut scenes = Vec::new();
    for (line_idx, raw_line) in input.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields: Vec<&str> = line.split('|').map(str::trim).collect();
        if fields.len() != 11 {
            return Err(format!(
                "line {}: expected 11 pipe-separated fields, got {}",
                line_idx + 1,
                fields.len()
            ));
        }
        let date = GameDate::parse(fields[7]).ok_or_else(|| {
            format!(
                "line {}: invalid date '{}', expected HOI4 format yyyy.m.d.h",
                line_idx + 1,
                fields[7]
            )
        })?;
        let enabled_layers = parse_layer_list(fields[10]).map_err(|err| {
            format!(
                "line {}: invalid enabled layer list '{}': {}",
                line_idx + 1,
                fields[10],
                err
            )
        })?;
        scenes.push(MapBaselineScene {
            name: fields[0].to_string(),
            description: fields[1].to_string(),
            camera: MapBaselineCamera {
                target_uv: [
                    parse_f32(fields[2], "center_u", line_idx)?,
                    parse_f32(fields[3], "center_v", line_idx)?,
                ],
                distance_factor: parse_f32(fields[4], "distance_factor", line_idx)?,
                pitch_degrees: parse_f32(fields[5], "pitch_degrees", line_idx)?,
                yaw_degrees: parse_f32(fields[6], "yaw_degrees", line_idx)?,
            },
            date,
            map_mode: fields[8].to_string(),
            enabled_layers,
        });
    }
    if scenes.is_empty() {
        return Err("scene config contained no scenes".to_string());
    }
    Ok(scenes)
}

fn parse_f32(value: &str, field: &str, line_idx: usize) -> Result<f32, String> {
    value.parse::<f32>().map_err(|err| {
        format!(
            "line {}: invalid {} value '{}': {}",
            line_idx + 1,
            field,
            value,
            err
        )
    })
}

fn parse_layer_list(value: &str) -> Result<Vec<MapBaselineLayer>, String> {
    let mut out = Vec::new();
    for raw in value.split(',') {
        let layer = MapBaselineLayer::from_config_name(raw)
            .ok_or_else(|| format!("unknown layer '{}'", raw.trim()))?;
        if !out.contains(&layer) {
            out.push(layer);
        }
    }
    if out.is_empty() {
        return Err("no layers specified".to_string());
    }
    Ok(out)
}

fn format_game_date(date: GameDate) -> String {
    format!(
        "{}.{:02}.{:02}.{:02}",
        date.year, date.month, date.day, date.hour
    )
}

fn vanilla_reference_readme(report: &MapBaselineReport) -> String {
    let mut out = String::new();
    out.push_str("Vanilla HOI4 reference screenshots belong in this directory.\n");
    out.push_str("Do not commit or redistribute Paradox screenshots or assets unless you have the right to do so.\n\n");
    out.push_str("Use the matching scene subdirectory and layer filename from report.json.\n\n");
    for scene in &report.scenes {
        let map_px = scene.map_px_center();
        let _ = writeln!(
            out,
            "- {}: map_px={:.0},{:.0} uv={:.4},{:.4} distance_factor={:.3} pitch={:.1} yaw={:.1} date={}",
            scene.name,
            map_px[0],
            map_px[1],
            scene.camera.target_uv[0],
            scene.camera.target_uv[1],
            scene.camera.distance_factor,
            scene.camera.pitch_degrees,
            scene.camera.yaw_degrees,
            format_game_date(scene.date)
        );
    }
    out
}

fn indent_json_object(out: &mut String, json: &str, spaces: usize) {
    let prefix = " ".repeat(spaces);
    let mut lines = json.lines();
    if let Some(first) = lines.next() {
        out.push_str(first);
    }
    for line in lines {
        out.push('\n');
        out.push_str(&prefix);
        out.push_str(line);
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vanilla_resource_views::{
        BindingAuditEntry, BindingBlockingLevel, BindingProvenance,
    };

    #[test]
    fn fixed_scene_matrix_matches_phase0_scope() {
        let scenes = fixed_scenes();
        assert_eq!(scenes.len(), 6);
        assert_eq!(MapBaselineLayer::ALL.len(), 21);
        assert_eq!(
            scenes[0].screenshot_name(MapBaselineLayer::FinalFull, MapBaselinePreset::High),
            "project/western_europe_close/final.high.png"
        );
        assert!(scenes.iter().any(|scene| scene.name.contains("mountain")));
        assert!(scenes.iter().any(|scene| scene.name.contains("night")));
        assert!(scenes.iter().any(|scene| scene.name.contains("distant")));
        assert!(scenes[0]
            .enabled_layers
            .contains(&MapBaselineLayer::TerrainOnly));
        assert!(scenes[0]
            .enabled_layers
            .contains(&MapBaselineLayer::AvgLuminance));
        assert!(scenes[0]
            .enabled_layers
            .contains(&MapBaselineLayer::LutAfter));
        assert!(scenes[0]
            .enabled_layers
            .contains(&MapBaselineLayer::OverlaysOnly));
        assert!(scenes[0]
            .enabled_layers
            .contains(&MapBaselineLayer::ProvinceSecondaryDebug));
        assert!(scenes[0]
            .enabled_layers
            .contains(&MapBaselineLayer::GradientBorderCh3Debug));
        assert!(scenes[0]
            .enabled_layers
            .contains(&MapBaselineLayer::TerrainRiverMaskDebug));
        assert!(scenes[0]
            .enabled_layers
            .contains(&MapBaselineLayer::FowVisibilityDebug));
        assert!(scenes[0]
            .enabled_layers
            .contains(&MapBaselineLayer::TerrainFinalBeforePostprocessDebug));
    }

    #[test]
    fn phase2_probe_layers_document_inputs_and_use_terrain_only_mask() {
        let probes = [
            (
                MapBaselineLayer::ProvinceSecondaryDebug,
                "province_secondary",
                "ProvinceSecondaryColorMap texture",
            ),
            (
                MapBaselineLayer::GradientBorderCh3Debug,
                "gradient_border_ch3",
                "GradientBorderChannel3 texture",
            ),
            (
                MapBaselineLayer::TerrainRiverMaskDebug,
                "river_mask",
                "terrain material river mask",
            ),
            (
                MapBaselineLayer::FowVisibilityDebug,
                "fow_visibility",
                "FOW visibility texture green channel",
            ),
            (
                MapBaselineLayer::TerrainFinalBeforePostprocessDebug,
                "final_before_postprocess",
                "terrain material final color before postprocess",
            ),
        ];

        for (layer, debug_name, source) in probes {
            assert!(layer.is_phase2_probe());
            assert_eq!(layer.terrain_debug_view_name(), Some(debug_name));
            assert_eq!(layer.diagnostic_source(), Some(source));

            let mask = MapLayerMask::for_layer(layer);
            assert!(mask.terrain);
            assert!(!mask.water);
            assert!(!mask.river);
            assert!(!mask.borders);
            assert!(!mask.objects);
            assert!(!mask.overlays);
            assert!(!mask.postprocess);
        }
    }

    #[test]
    fn layer_masks_isolate_expected_groups() {
        let terrain = MapLayerMask::for_layer(MapBaselineLayer::TerrainOnly);
        assert!(terrain.terrain);
        assert!(!terrain.water);
        assert!(!terrain.river);
        assert!(!terrain.postprocess);

        let full = MapLayerMask::for_layer(MapBaselineLayer::FinalFull);
        assert!(full.terrain);
        assert!(full.water);
        assert!(full.river);
        assert!(full.postprocess);

        let river = MapLayerMask::for_layer(MapBaselineLayer::RiverMask);
        assert!(river.river);
        assert!(!river.terrain);
        assert!(!river.water);
        assert!(river.static_decals);

        let post_off = MapLayerMask::for_layer(MapBaselineLayer::PostprocessOff);
        assert!(post_off.terrain);
        assert!(!post_off.postprocess);

        let hdr_raw = MapLayerMask::for_layer(MapBaselineLayer::HdrRaw);
        assert!(hdr_raw.terrain);
        assert!(hdr_raw.postprocess);

        let avg_lum = MapLayerMask::for_layer(MapBaselineLayer::AvgLuminance);
        assert!(avg_lum.terrain);
        assert!(avg_lum.postprocess);

        let lut_after = MapLayerMask::for_layer(MapBaselineLayer::LutAfter);
        assert!(lut_after.terrain);
        assert!(lut_after.postprocess);

        let objects = MapLayerMask::for_layer(MapBaselineLayer::ObjectsOnly);
        assert!(objects.objects);
        assert!(!objects.terrain);

        let overlays = MapLayerMask::for_layer(MapBaselineLayer::OverlaysOnly);
        assert!(overlays.overlays);
        assert!(overlays.static_decals);
        assert!(overlays.objects);
        assert!(!overlays.terrain);
    }

    #[test]
    fn p1_frame_graph_audit_reports_vanilla_order_and_targets() {
        let text = p1_frame_graph_audit_text();
        assert!(text.contains("main_hdr_scene_format=Rgba16Float"));
        assert!(text.contains("depth_format=Depth24PlusStencil8"));
        assert!(text.contains("ui_enters_hdr=false"));
        assert!(text.contains("b1_pass=3d_terrain vanilla_order=81"));
        assert!(text.contains("b1_pass=3d_border_first vanilla_order=82"));
        assert!(text.contains("b1_pass=3d_river vanilla_order=83"));
        assert!(text.contains("b1_pass=3d_map_layers vanilla_order=84-85"));
        assert!(text.contains("b1_pass=3d_water vanilla_order=86"));
        assert!(text.contains("b1_pass=3d_border_second vanilla_order=87"));
        assert!(text.contains("order_133_policy=environment_reflection_family_only"));

        let json = p1_frame_graph_audit_json();
        assert!(json.contains("\"kind\": \"p1_frame_graph\""));
        assert!(json.contains("\"unique_core_map_color_target\": true"));
        assert!(json.contains("\"postprocess_restores_to_swapchain\": true"));
        assert!(json.contains("\"ui_after_restore\": true"));
    }

    #[test]
    fn report_json_has_capture_matrix() {
        let map_set = VanillaMapSet {
            entries: Vec::new(),
        };
        let audit = MapAssetAudit::from_map_set(&map_set);
        let scenes = fixed_scenes();
        let planned_captures = build_phase0_planned_captures(&scenes, &audit);
        let report = MapBaselineReport {
            scenes,
            planned_captures,
            asset_audit: audit,
            binding_audit: BindingAudit::new(),
            reference_source_root: None,
            elapsed_ms: 0.0,
        };
        let json = report.to_json_report();
        assert!(json.contains("\"phase\": \"0\""));
        assert!(json.contains("\"project_capture_root\": \"project\""));
        assert!(json.contains("\"diff_report_root\": \"diff\""));
        assert!(json.contains("\"diff_report_filename\""));
        assert!(json.contains("\"pass_status\""));
        assert!(json.contains("\"binding_audit\""));
        assert!(json.contains("\"asset_quality\""));
        assert!(json.contains("\"phase2_probe\""));
        assert!(json.contains("\"terrain_debug_view\": \"province_secondary\""));
        assert!(json.contains("\"diagnostic_source\": \"GradientBorderChannel3 texture\""));
        assert!(json.contains("project/western_europe_close/final.high.png"));
        assert!(
            json.contains("project/western_europe_close/terrain_final_before_postprocess.high.png")
        );
    }

    #[test]
    fn p0_parity_gate_json_covers_required_b1i_mapping() {
        let map_set = VanillaMapSet {
            entries: Vec::new(),
        };
        let audit = MapAssetAudit::from_map_set(&map_set);
        let binding_audit = BindingAudit::new();

        let json = p0_parity_gate_json(&audit, &binding_audit);
        assert!(json.contains("\"kind\": \"p0_parity_gate\""));
        assert!(json.contains("\"status\": \"pass\""));
        assert!(json.contains("\"critical_fallback_silent_pass\": false"));
        for id in [
            "B1I-001", "B1I-002", "B1I-003", "B1I-009", "B1I-011", "B1I-013", "B1I-014", "B1I-015",
            "B1I-016", "B1I-017", "B1I-018", "B1I-019", "B1I-020", "B1I-021", "B1I-022", "B1I-023",
            "B1I-024", "B1I-025", "B1I-026", "B1I-032",
        ] {
            assert!(json.contains(id), "missing {id} in P0 mapping");
        }
    }

    #[test]
    fn p0_parity_gate_blocks_critical_binding_fallbacks() {
        let map_set = VanillaMapSet {
            entries: Vec::new(),
        };
        let audit = MapAssetAudit::from_map_set(&map_set);
        let mut binding_audit = BindingAudit::new();
        binding_audit.extend([BindingAuditEntry::mock(
            "water",
            "ShadowMap",
            "depth_shadow_fallback",
            "ordinary depth shadow is not the projected FOW ShadowMap",
            "water shadowing and FOW parity",
            BindingBlockingLevel::Critical,
        )]);

        let json = p0_parity_gate_json(&audit, &binding_audit);
        assert!(json.contains("\"status\": \"blocked\""));
        assert!(json.contains("\"critical_binding_fallback_count\": 1"));
        assert!(json.contains("\"critical_fallback_count\": 1"));
        assert!(json.contains("\"critical_fallback_silent_pass\": false"));
        assert!(json.contains("\"binding\": \"ShadowMap\""));
    }

    #[test]
    fn p0_parity_gate_blocks_non_equivalent_runtime_producers() {
        let map_set = VanillaMapSet {
            entries: Vec::new(),
        };
        let audit = MapAssetAudit::from_map_set(&map_set);
        let mut binding_audit = BindingAudit::new();
        binding_audit.extend([BindingAuditEntry::dynamic_target(
            "terrain",
            "mud_snow",
            "MudSnow",
            "terrain snow/mud parity",
        )
        .with_runtime_target_metadata(
            "zero fallback SnowMudData",
            "Bgra8Unorm; 4 bytes/pixel",
            "quarter-size SnowMudData 1408x512",
            "reverse_out/exports/mud_snow_light_fow_targets.tsv",
            "fallback/non-parity",
        )
        .with_producer_status(
            BindingProvenance::NeutralFallback,
            "SnowMudData=missing_game_state_zero_fallback_non_parity",
            false,
            BindingBlockingLevel::Degraded,
        )]);

        let json = p0_parity_gate_json(&audit, &binding_audit);
        assert!(json.contains("\"status\": \"blocked\""));
        assert!(json.contains("\"critical_fallback_count\": 0"));
        assert!(json.contains("\"producer_non_equivalent_count\": 1"));
        assert!(json.contains(
            "\"producer_status\": \"SnowMudData=missing_game_state_zero_fallback_non_parity\""
        ));
    }

    #[test]
    fn phase0_report_writes_diff_placeholders() {
        let dir = std::env::temp_dir().join("hoi4_phase0_report_diff_placeholder_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let map_set = VanillaMapSet {
            entries: Vec::new(),
        };
        let audit = MapAssetAudit::from_map_set(&map_set);
        let scenes = vec![fixed_scenes().remove(0)];
        let mut planned_captures = build_phase0_planned_captures(&scenes, &audit);
        planned_captures.truncate(1);
        let report = MapBaselineReport {
            scenes,
            planned_captures,
            asset_audit: audit,
            binding_audit: BindingAudit::new(),
            reference_source_root: None,
            elapsed_ms: 0.0,
        };

        write_phase0_report_files(&report, &dir).unwrap();
        let diff = dir.join("diff/western_europe_close/final.high.json");
        let diff_json = std::fs::read_to_string(diff).unwrap();
        assert!(diff_json.contains("\"status\": \"missing_project\""));
        let report_json = std::fs::read_to_string(dir.join("report.json")).unwrap();
        assert!(report_json.contains("\"diff_missing_project_count\": 1"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn timestamp_conversion_formats_unix_epoch() {
        assert_eq!(civil_from_unix_days(0), (1970, 1, 1));
        assert_eq!(civil_from_unix_days(20_544), (2026, 4, 1));
    }
}
