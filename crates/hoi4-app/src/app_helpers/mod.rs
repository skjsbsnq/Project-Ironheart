use hoi4_audio::UiSound;
use hoi4_render::camera::Camera;
use hoi4_render::defines::VanillaMapSpace;
use hoi4_render::map_mode::MapMode;

use crate::map_baseline;
use crate::passes::{PostProcessLutSelection, TerrainDebugView};

pub(crate) fn estimate_construction_days_remaining(progress: f32, cost: f32) -> Option<u32> {
    if cost <= 0.0 || progress < 0.0 {
        return None;
    }
    let completion = (progress / cost).clamp(0.0, 1.0);
    if completion >= 1.0 {
        return Some(0);
    }
    Some(((1.0 - completion) * 100.0).ceil().max(1.0) as u32)
}

pub(crate) fn postprocess_lut_selection_for(
    _camera: &Camera,
    _world: &hoi4_state::World,
    _map_space: &VanillaMapSpace,
) -> PostProcessLutSelection {
    // Stable Phase B default: keep the restore LUT on the source-backed
    // close-land day path until posteffect volume classification is mirrored.
    // Camera distance, screen-sampled water ratio, and camera-longitude night
    // factors made the entire frame switch LUTs while panning/zooming.
    PostProcessLutSelection {
        camera_distance_t: 0.0,
        night_factor: 0.0,
        water_factor: 0.0,
        winter_factor: 0.0,
    }
}

pub(crate) fn intervention_expected_impact(
    progress_boost: f32,
    army_xp: f32,
    air_xp: f32,
    cooldown_days: u32,
) -> String {
    let mut parts = Vec::new();
    if progress_boost.abs() > f32::EPSILON {
        parts.push(format!("????????? +{:.0}", progress_boost));
    }
    if army_xp > 0.0 {
        parts.push(format!("?????? +{:.0}", army_xp));
    }
    if air_xp > 0.0 {
        parts.push(format!("?????? +{:.0}", air_xp));
    }
    if cooldown_days > 0 {
        parts.push(format!("Cooldown {} days", cooldown_days));
    }
    if parts.is_empty() {
        "Unknown".to_owned()
    } else {
        parts.join(", ")
    }
}

pub(crate) fn decision_id_matches_player_tag(decision_id: &str, player_tag_lower: &str) -> bool {
    let Some((prefix, _)) = decision_id.split_once('.') else {
        return true;
    };
    prefix.eq_ignore_ascii_case(player_tag_lower)
}

pub(crate) fn event_modal_sound(event: &hoi4_content::Event) -> UiSound {
    if matches!(event.scope, hoi4_content::EventScope::News) {
        let id = event.id.as_str();
        if id.contains("outbreak")
            || id.contains("declare_war")
            || id.contains("declaration")
            || id.contains("war_begins")
        {
            UiSound::WarDeclaration
        } else {
            UiSound::WorldNews
        }
    } else {
        UiSound::EventPopup
    }
}

pub(crate) fn map_mode_from_capture_name(name: &str) -> MapMode {
    match name {
        "terrain" => MapMode::Terrain,
        "manpower" => MapMode::Manpower,
        "factories" => MapMode::Factories,
        "cores" => MapMode::Cores,
        "infrastructure" => MapMode::Infrastructure,
        "ideology" => MapMode::Ideology,
        "supply" => MapMode::Supply,
        "resistance" => MapMode::Resistance,
        _ => MapMode::Political,
    }
}

pub(crate) fn map_mode_terrain_blend_for(map_mode: MapMode) -> f32 {
    match map_mode {
        MapMode::Political => 0.44,
        MapMode::Terrain => 0.92,
        _ => 0.58,
    }
}

pub(crate) fn terrain_debug_view_for_baseline_layer(
    layer: map_baseline::MapBaselineLayer,
) -> TerrainDebugView {
    match layer {
        map_baseline::MapBaselineLayer::ProvinceSecondaryDebug => {
            TerrainDebugView::ProvinceSecondary
        }
        map_baseline::MapBaselineLayer::GradientBorderCh3Debug => {
            TerrainDebugView::GradientBorderCh3
        }
        map_baseline::MapBaselineLayer::TerrainRiverMaskDebug => TerrainDebugView::RiverMask,
        map_baseline::MapBaselineLayer::FowVisibilityDebug => TerrainDebugView::FowVisibility,
        map_baseline::MapBaselineLayer::TerrainFinalBeforePostprocessDebug => {
            TerrainDebugView::FinalBeforePostprocess
        }
        _ => TerrainDebugView::Off,
    }
}

pub(crate) fn surrender_notification_sound_key(
    notification: &hoi4_ui::surrender_notification::SurrenderNotification,
) -> String {
    format!(
        "{}:{}:{:?}:{}",
        notification.winner_tag,
        notification.target_tag,
        notification.kind,
        notification.results.len()
    )
}
