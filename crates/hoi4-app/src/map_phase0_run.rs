use std::path::PathBuf;

use crate::*;

pub(crate) struct MapPhase0Run {
    output_dir: PathBuf,
    reference_root: Option<PathBuf>,
    started: Instant,
    captures: Vec<map_baseline::MapBaselinePlannedCapture>,
    scenes: Vec<map_baseline::MapBaselineScene>,
    asset_audit: hoi4_assets::MapAssetAudit,
    binding_audit: vanilla_resource_views::BindingAudit,
    capture_index: usize,
    settle_frames: u8,
    finished: bool,
}

impl MapPhase0Run {
    pub(crate) fn new(
        path_cfg: &PathConfig,
        output_dir: PathBuf,
        reference_root: Option<PathBuf>,
    ) -> Self {
        let vanilla_resources = VanillaResourceViews::load_for_audit(path_cfg);
        let asset_audit = hoi4_assets::MapAssetAudit::from_map_set(&vanilla_resources.map_set);
        let binding_audit = vanilla_resources.phase1_binding_audit();
        let scenes = map_baseline::fixed_scenes();
        let captures = map_baseline::build_phase0_planned_captures(&scenes, &asset_audit);
        Self {
            output_dir,
            reference_root,
            started: Instant::now(),
            captures,
            scenes,
            asset_audit,
            binding_audit,
            capture_index: 0,
            settle_frames: 2,
            finished: false,
        }
    }

    pub(crate) fn current_capture(&self) -> Option<&map_baseline::MapBaselinePlannedCapture> {
        self.captures.get(self.capture_index)
    }
}

impl App {
    pub(crate) fn enable_map_phase0(
        &mut self,
        output_dir: PathBuf,
        reference_root: Option<PathBuf>,
    ) {
        let run = MapPhase0Run::new(&self.path_cfg, output_dir, reference_root);
        println!(
            "[map-phase0] starting capture batch: scenes={} layers={} captures={} output={}",
            run.scenes.len(),
            map_baseline::MapBaselineLayer::ALL.len(),
            run.captures.len(),
            run.output_dir.display()
        );
        if let Some(reference_root) = &run.reference_root {
            println!(
                "[map-phase0] vanilla reference root={}",
                reference_root.display()
            );
        }
        println!("[map-phase0] {}", run.asset_audit.summary_line());
        println!("[map-phase0] {}", run.binding_audit.summary_line());
        if !run.asset_audit.fallback_paths.is_empty() {
            for path in run.asset_audit.fallback_paths.iter().take(16) {
                eprintln!("[map-phase0] fallback asset: {path}");
            }
            if run.asset_audit.fallback_paths.len() > 16 {
                eprintln!(
                    "[map-phase0] fallback asset: ... {} more",
                    run.asset_audit.fallback_paths.len() - 16
                );
            }
        }
        if !run.asset_audit.can_use_for_visual_review() {
            eprintln!(
                "[map-phase0] critical fallback present; screenshots will be marked unusable for visual review"
            );
        }
        if run.binding_audit.critical_mock_count() > 0 {
            eprintln!(
                "[map-phase0] critical binding mocks present; parity screenshots are blocked"
            );
        }

        self.view.game_phase = GamePhase::Playing;
        self.world.speed = GameSpeed::Paused;
        self.close_primary_panel();
        self.ui_state.active_popup = None;
        self.demo_visible = false;
        self.b5_demo_visible = false;
        self.render_toggles.debug_overlay = false;
        self.render_toggles.terrain_debug_view = passes::TerrainDebugView::Off;
        self.render_toggles.water_debug_view = passes::WaterDebugView::Off;
        self.render_toggles.border_debug_view = passes::BorderDebugView::Off;
        self.render_toggles.postprocess_debug_view = PostProcessDebugView::Final;
        self.map_mode = MapMode::Political;
        self.audit.map_phase0 = Some(run);
    }

    pub(crate) fn map_phase0_finished(&self) -> bool {
        self.audit
            .map_phase0
            .as_ref()
            .map(|run| run.finished)
            .unwrap_or(false)
    }

    pub(crate) fn current_map_layer_mask(&self) -> map_baseline::MapLayerMask {
        self.audit
            .map_phase0
            .as_ref()
            .and_then(|run| run.current_capture())
            .map(|capture| capture.layer_mask)
            .unwrap_or_else(map_baseline::MapLayerMask::all)
    }

    pub(crate) fn map_phase0_capture_ready(&self) -> bool {
        self.audit.map_phase0.as_ref().is_some_and(|run| {
            !run.finished && run.settle_frames == 0 && run.current_capture().is_some()
        })
    }

    pub(crate) fn map_phase0_capture_path(&self) -> Option<PathBuf> {
        let run = self.audit.map_phase0.as_ref()?;
        let capture = run.current_capture()?;
        Some(run.output_dir.join(&capture.filename))
    }

    pub(crate) fn map_phase0_debug_lines(&self) -> Vec<String> {
        let Some(run) = self.audit.map_phase0.as_ref() else {
            return Vec::new();
        };
        let mut lines = vec![
            "Map Renderer V2 Phase 0 - Asset Fallback Debug".to_string(),
            run.asset_audit.summary_line(),
            run.binding_audit.summary_line(),
            format!(
                "visual_review_usable={}",
                run.asset_audit.can_use_for_visual_review()
            ),
        ];
        if run.asset_audit.fallback_paths.is_empty() {
            lines.push("fallbacks=0".to_string());
        } else {
            lines.push(format!(
                "fallbacks={}",
                run.asset_audit.fallback_paths.len()
            ));
            for path in run.asset_audit.fallback_paths.iter().take(20) {
                lines.push(format!("fallback: {path}"));
            }
            if run.asset_audit.fallback_paths.len() > 20 {
                lines.push(format!(
                    "... {} more",
                    run.asset_audit.fallback_paths.len() - 20
                ));
            }
        }
        lines
    }

    pub(crate) fn prepare_map_phase0_capture(&mut self) {
        let Some(run) = self.audit.map_phase0.as_ref() else {
            return;
        };
        if run.finished {
            return;
        }
        let Some(capture) = run.current_capture() else {
            self.finish_map_phase0();
            return;
        };
        let Some(scene) = run
            .scenes
            .iter()
            .find(|scene| scene.name == capture.scene_name)
            .cloned()
        else {
            return;
        };
        let mask = capture.layer_mask;
        let world_size = self.camera.world_size;
        self.camera.target = glam::Vec3::new(
            scene.camera.target_uv[0] * world_size.x,
            0.0,
            scene.camera.target_uv[1] * world_size.y,
        );
        self.camera.distance = (world_size.x.max(world_size.y) * scene.camera.distance_factor)
            .clamp(3.0, world_size.x.max(world_size.y) * 4.0);
        self.camera.pitch = scene.camera.pitch_degrees.to_radians();
        self.camera.yaw = scene.camera.yaw_degrees.to_radians();
        self.camera.clamp_target_to_map();
        self.world.date = scene.date;
        self.render_toggles.show_province_names = mask.labels;
        let scene_map_mode = map_mode_from_capture_name(&scene.map_mode);
        if self.map_mode != scene_map_mode {
            self.map_mode = scene_map_mode;
            self.refresh_lut();
        }
        self.render_toggles.terrain_debug_view =
            terrain_debug_view_for_baseline_layer(capture.layer);
        self.render_toggles.water_debug_view = passes::WaterDebugView::Off;
        self.render_toggles.border_debug_view = passes::BorderDebugView::Off;
        self.render_toggles.postprocess_debug_view = match capture.layer {
            map_baseline::MapBaselineLayer::HdrRaw => PostProcessDebugView::HdrRaw,
            map_baseline::MapBaselineLayer::AvgLuminance => PostProcessDebugView::AvgLuminance,
            map_baseline::MapBaselineLayer::TonemapBefore => PostProcessDebugView::TonemapBefore,
            map_baseline::MapBaselineLayer::TonemapOnly => PostProcessDebugView::TonemapOnly,
            map_baseline::MapBaselineLayer::BloomOnly => PostProcessDebugView::BloomOnly,
            map_baseline::MapBaselineLayer::LutBefore => PostProcessDebugView::LutBefore,
            map_baseline::MapBaselineLayer::LutAfter => PostProcessDebugView::LutAfter,
            _ => PostProcessDebugView::Final,
        };
        if let Some(s) = self.state.as_mut() {
            s.post_process.debug_view = self.render_toggles.postprocess_debug_view;
        }
        self.upload_camera();
    }

    pub(crate) fn map_phase0_after_uncaptured_frame(&mut self) {
        if let Some(run) = self.audit.map_phase0.as_mut() {
            if !run.finished && run.settle_frames > 0 {
                run.settle_frames -= 1;
            }
        }
    }

    pub(crate) fn map_phase0_after_capture(
        &mut self,
        frame_time_ms: f32,
        result: Result<(), String>,
    ) {
        if let Err(err) = result {
            eprintln!("[map-phase0] screenshot write failed: {err}");
        }
        let Some(run) = self.audit.map_phase0.as_mut() else {
            return;
        };
        if run.finished {
            return;
        }
        let total = run.captures.len();
        if let Some(capture) = run.captures.get_mut(run.capture_index) {
            capture.frame_time_ms = Some(frame_time_ms);
            println!(
                "[map-phase0] captured {}/{} {} frame_time_ms={:.2} usable={}",
                run.capture_index + 1,
                total,
                capture.filename,
                frame_time_ms,
                capture.visual_review_usable
            );
        }
        run.capture_index += 1;
        run.settle_frames = 1;
        if run.capture_index >= total {
            self.finish_map_phase0();
        }
    }

    pub(crate) fn finish_map_phase0(&mut self) {
        let Some(run) = self.audit.map_phase0.as_mut() else {
            return;
        };
        if run.finished {
            return;
        }
        let report = map_baseline::MapBaselineReport::from_captures(
            run.scenes.clone(),
            run.captures.clone(),
            run.asset_audit.clone(),
            run.binding_audit.clone(),
            run.started.elapsed().as_secs_f64() * 1000.0,
        );
        match map_baseline::write_phase0_report_files_with_references(
            &report,
            &run.output_dir,
            run.reference_root.as_deref(),
        ) {
            Ok(path) => println!("[map-phase0] wrote {}", path.display()),
            Err(err) => eprintln!("[map-phase0] report write failed: {err}"),
        }
        run.finished = true;
    }
}
