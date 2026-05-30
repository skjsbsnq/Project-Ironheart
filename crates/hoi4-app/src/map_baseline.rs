use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::time::Instant;

use hoi4_assets::{FsAssetDb, MapAssetAudit, MapAssetQuality, VanillaMapSet};
use hoi4_paths::PathConfig;

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
pub enum MapBaselineLayer {
    FinalFull,
    PostprocessOff,
    HdrRaw,
    TonemapOnly,
    BloomOnly,
    TerrainOnly,
    WaterOnly,
    BordersOnly,
    OverlaysOnly,
    LabelsOnly,
    AssetFallbackDebug,
}

impl MapBaselineLayer {
    pub const ALL: [Self; 11] = [
        Self::FinalFull,
        Self::PostprocessOff,
        Self::HdrRaw,
        Self::TonemapOnly,
        Self::BloomOnly,
        Self::TerrainOnly,
        Self::WaterOnly,
        Self::BordersOnly,
        Self::OverlaysOnly,
        Self::LabelsOnly,
        Self::AssetFallbackDebug,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::FinalFull => "final_full",
            Self::PostprocessOff => "postprocess_off",
            Self::HdrRaw => "hdr_raw",
            Self::TonemapOnly => "tonemap_only",
            Self::BloomOnly => "bloom_only",
            Self::TerrainOnly => "terrain_only",
            Self::WaterOnly => "water_only",
            Self::BordersOnly => "borders_only",
            Self::OverlaysOnly => "overlays_only",
            Self::LabelsOnly => "labels_only",
            Self::AssetFallbackDebug => "asset_fallback_debug",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapLayerMask {
    pub sky: bool,
    pub terrain: bool,
    pub water: bool,
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
            | MapBaselineLayer::TonemapOnly
            | MapBaselineLayer::BloomOnly => Self::all(),
            MapBaselineLayer::TerrainOnly => Self {
                terrain: true,
                ..Self::none()
            },
            MapBaselineLayer::WaterOnly => Self {
                water: true,
                ..Self::none()
            },
            MapBaselineLayer::BordersOnly => Self {
                borders: true,
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
    pub name: &'static str,
    pub description: &'static str,
    pub map_mode: &'static str,
    pub date: &'static str,
    pub camera: MapBaselineCamera,
}

impl MapBaselineScene {
    pub fn screenshot_name(&self, layer: MapBaselineLayer, preset: MapBaselinePreset) -> String {
        format!("{}.{}.{}.png", self.name, layer.as_str(), preset.as_str())
    }
}

pub fn fixed_scenes() -> Vec<MapBaselineScene> {
    vec![
        scene(
            "western_europe_far",
            "Western Europe far political readability",
            [0.50, 0.34],
            0.64,
        ),
        scene(
            "germany_poland_mid",
            "Germany and Poland medium province/border density",
            [0.54, 0.32],
            0.25,
        ),
        scene(
            "italy_adriatic_coast",
            "Italy and Adriatic coastline, islands and shallow water",
            [0.54, 0.39],
            0.18,
        ),
        scene(
            "english_channel",
            "English Channel water, coast foam and labels",
            [0.48, 0.30],
            0.16,
        ),
        scene(
            "alps_close",
            "Alps close terrain, snow, height and LOD stability",
            [0.53, 0.36],
            0.08,
        ),
        scene(
            "japan_korea_close",
            "Japan and Korea islands, coast precision and borders",
            [0.82, 0.42],
            0.15,
        ),
        scene(
            "north_africa_desert",
            "North Africa desert texture repetition and coast transition",
            [0.52, 0.48],
            0.24,
        ),
        scene(
            "pacific_deep_ocean",
            "Pacific deep ocean color, bloom and sea region borders",
            [0.88, 0.53],
            0.45,
        ),
        scene(
            "soviet_winter_snow",
            "Soviet winter snow line, season and fog brightness",
            [0.63, 0.25],
            0.32,
        ),
        scene(
            "active_war_front",
            "Active war front overlays, arrows, counters and occupation",
            [0.57, 0.33],
            0.18,
        ),
    ]
}

fn scene(
    name: &'static str,
    description: &'static str,
    target_uv: [f32; 2],
    distance_factor: f32,
) -> MapBaselineScene {
    MapBaselineScene {
        name,
        description,
        map_mode: "political",
        date: "1936-01-01T12:00:00",
        camera: MapBaselineCamera {
            target_uv,
            distance_factor,
            pitch_degrees: 65.0,
            yaw_degrees: 0.0,
        },
    }
}

#[derive(Debug, Clone)]
pub struct MapBaselinePlannedCapture {
    pub scene_name: String,
    pub layer: MapBaselineLayer,
    pub preset: MapBaselinePreset,
    pub filename: String,
    pub layer_mask: MapLayerMask,
    pub frame_time_ms: Option<f32>,
    pub asset_quality: MapAssetQuality,
    pub asset_fallback_count: usize,
    pub visual_review_usable: bool,
}

#[derive(Debug, Clone)]
pub struct MapBaselineReport {
    pub scenes: Vec<MapBaselineScene>,
    pub planned_captures: Vec<MapBaselinePlannedCapture>,
    pub asset_audit: MapAssetAudit,
    pub elapsed_ms: f64,
}

impl MapBaselineReport {
    pub fn from_captures(
        scenes: Vec<MapBaselineScene>,
        planned_captures: Vec<MapBaselinePlannedCapture>,
        asset_audit: MapAssetAudit,
        elapsed_ms: f64,
    ) -> Self {
        Self {
            scenes,
            planned_captures,
            asset_audit,
            elapsed_ms,
        }
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
        let _ = writeln!(out, "{}", self.asset_audit.summary_line());
        let _ = writeln!(
            out,
            "visual_review_usable={}",
            self.asset_audit.can_use_for_visual_review()
        );
        out.push_str("\nscenes:\n");
        for scene in &self.scenes {
            let _ = writeln!(
                out,
                "  - {}: mode={} date={} target_uv={:.3},{:.3} distance_factor={:.3}",
                scene.name,
                scene.map_mode,
                scene.date,
                scene.camera.target_uv[0],
                scene.camera.target_uv[1],
                scene.camera.distance_factor
            );
        }
        out.push_str("\nplanned_captures:\n");
        for capture in &self.planned_captures {
            let _ = writeln!(
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
        }
        out.push_str("\nasset_audit:\n");
        out.push_str(&self.asset_audit.to_text_report());
        out
    }

    pub fn to_json_report(&self) -> String {
        let mut out = String::new();
        out.push_str("{\n");
        let _ = writeln!(out, "  \"phase\": \"0\",");
        let _ = writeln!(out, "  \"elapsed_ms\": {:.3},", self.elapsed_ms);
        out.push_str("  \"scenes\": [\n");
        for (idx, scene) in self.scenes.iter().enumerate() {
            out.push_str("    {\n");
            let _ = writeln!(out, "      \"name\": \"{}\",", json_escape(scene.name));
            let _ = writeln!(
                out,
                "      \"description\": \"{}\",",
                json_escape(scene.description)
            );
            let _ = writeln!(out, "      \"map_mode\": \"{}\",", scene.map_mode);
            let _ = writeln!(out, "      \"date\": \"{}\",", scene.date);
            let _ = writeln!(
                out,
                "      \"camera\": {{ \"target_uv\": [{:.6}, {:.6}], \"distance_factor\": {:.6}, \"pitch_degrees\": {:.3}, \"yaw_degrees\": {:.3} }}",
                scene.camera.target_uv[0],
                scene.camera.target_uv[1],
                scene.camera.distance_factor,
                scene.camera.pitch_degrees,
                scene.camera.yaw_degrees
            );
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
            write_layer_mask_json(&mut out, &capture.layer_mask);
            out.push('\n');
            out.push_str("    }");
            out.push_str(comma(idx + 1, self.planned_captures.len()));
            out.push('\n');
        }
        out.push_str("  ],\n");
        out.push_str("  \"asset_audit\": ");
        indent_json_object(&mut out, &self.asset_audit.to_json_report(), 2);
        out.push('\n');
        out.push_str("}\n");
        out
    }
}

pub fn build_phase0_report(path_cfg: &PathConfig) -> MapBaselineReport {
    let started = Instant::now();
    let asset_audit = build_asset_audit(path_cfg);
    let scenes = fixed_scenes();
    let planned_captures = build_phase0_planned_captures(&scenes, &asset_audit);
    MapBaselineReport {
        scenes,
        planned_captures,
        asset_audit,
        elapsed_ms: started.elapsed().as_secs_f64() * 1000.0,
    }
}

pub fn build_phase0_planned_captures(
    scenes: &[MapBaselineScene],
    asset_audit: &MapAssetAudit,
) -> Vec<MapBaselinePlannedCapture> {
    let mut planned_captures = Vec::with_capacity(scenes.len() * MapBaselineLayer::ALL.len());
    for scene in scenes {
        for layer in MapBaselineLayer::ALL {
            planned_captures.push(MapBaselinePlannedCapture {
                scene_name: scene.name.to_string(),
                layer,
                preset: MapBaselinePreset::High,
                filename: scene.screenshot_name(layer, MapBaselinePreset::High),
                layer_mask: MapLayerMask::for_layer(layer),
                frame_time_ms: None,
                asset_quality: asset_audit.quality,
                asset_fallback_count: asset_audit.fallback,
                visual_review_usable: asset_audit.can_use_for_visual_review(),
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

pub fn write_phase0_report(path_cfg: &PathConfig, output_dir: &Path) -> std::io::Result<PathBuf> {
    let report = build_phase0_report(path_cfg);
    write_phase0_report_files(&report, output_dir)
}

pub fn write_map_audit(path_cfg: &PathConfig, output_dir: &Path) -> std::io::Result<PathBuf> {
    let audit = build_asset_audit(path_cfg);
    write_map_audit_files(&audit, output_dir)
}

pub fn write_map_audit_files(audit: &MapAssetAudit, output_dir: &Path) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(output_dir)?;
    let text_path = output_dir.join("latest.txt");
    let json_path = output_dir.join("latest.json");
    std::fs::write(&text_path, audit.to_text_report())?;
    std::fs::write(&json_path, audit.to_json_report())?;
    Ok(json_path)
}

pub fn write_phase0_report_files(
    report: &MapBaselineReport,
    output_dir: &Path,
) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(output_dir)?;
    let text_path = output_dir.join("phase0_latest.txt");
    let json_path = output_dir.join("phase0_latest.json");
    let audit_json_path = output_dir.join("asset_audit_latest.json");
    let audit_text_path = output_dir.join("asset_audit_latest.txt");
    std::fs::write(&text_path, report.to_text_report())?;
    std::fs::write(&json_path, report.to_json_report())?;
    std::fs::write(&audit_json_path, report.asset_audit.to_json_report())?;
    std::fs::write(&audit_text_path, report.asset_audit.to_text_report())?;
    Ok(json_path)
}

fn write_layer_mask_json(out: &mut String, mask: &MapLayerMask) {
    out.push_str("      \"layer_mask\": { ");
    let _ = write!(
        out,
        "\"sky\": {}, \"terrain\": {}, \"water\": {}, \"borders\": {}, \"static_decals\": {}, \"overlays\": {}, \"objects\": {}, \"labels\": {}, \"particles\": {}, \"ui\": {}, \"postprocess\": {}, \"asset_fallback_debug\": {}",
        mask.sky,
        mask.terrain,
        mask.water,
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

    #[test]
    fn fixed_scene_matrix_matches_phase0_scope() {
        let scenes = fixed_scenes();
        assert_eq!(scenes.len(), 10);
        assert_eq!(MapBaselineLayer::ALL.len(), 11);
        assert_eq!(
            scenes[0].screenshot_name(MapBaselineLayer::FinalFull, MapBaselinePreset::High),
            "western_europe_far.final_full.high.png"
        );
    }

    #[test]
    fn layer_masks_isolate_expected_groups() {
        let terrain = MapLayerMask::for_layer(MapBaselineLayer::TerrainOnly);
        assert!(terrain.terrain);
        assert!(!terrain.water);
        assert!(!terrain.postprocess);

        let full = MapLayerMask::for_layer(MapBaselineLayer::FinalFull);
        assert!(full.terrain);
        assert!(full.water);
        assert!(full.postprocess);

        let post_off = MapLayerMask::for_layer(MapBaselineLayer::PostprocessOff);
        assert!(post_off.terrain);
        assert!(!post_off.postprocess);

        let hdr_raw = MapLayerMask::for_layer(MapBaselineLayer::HdrRaw);
        assert!(hdr_raw.terrain);
        assert!(hdr_raw.postprocess);
    }

    #[test]
    fn report_json_has_capture_matrix() {
        let map_set = VanillaMapSet {
            entries: Vec::new(),
        };
        let audit = MapAssetAudit::from_map_set(&map_set);
        let visual_review_usable = audit.can_use_for_visual_review();
        let asset_quality = audit.quality;
        let asset_fallback_count = audit.fallback;
        let scenes = fixed_scenes();
        let planned_captures = scenes
            .iter()
            .flat_map(|scene| {
                MapBaselineLayer::ALL
                    .into_iter()
                    .map(move |layer| MapBaselinePlannedCapture {
                        scene_name: scene.name.to_string(),
                        layer,
                        preset: MapBaselinePreset::High,
                        filename: scene.screenshot_name(layer, MapBaselinePreset::High),
                        layer_mask: MapLayerMask::for_layer(layer),
                        frame_time_ms: None,
                        asset_quality,
                        asset_fallback_count,
                        visual_review_usable,
                    })
            })
            .collect();
        let report = MapBaselineReport {
            scenes,
            planned_captures,
            asset_audit: audit,
            elapsed_ms: 0.0,
        };
        let json = report.to_json_report();
        assert!(json.contains("\"phase\": \"0\""));
        assert!(json.contains("asset_fallback_debug"));
        assert!(json.contains("\"asset_quality\""));
        assert!(json.contains("western_europe_far.final_full.high.png"));
    }
}
