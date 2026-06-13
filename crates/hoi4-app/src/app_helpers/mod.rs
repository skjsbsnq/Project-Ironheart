use hoi4_audio::UiSound;
use hoi4_render::camera::Camera;
use hoi4_render::defines::VanillaMapSpace;
use hoi4_render::map_mode::MapMode;
use hoi4_state::{GameDate, GameSpeed};

use crate::map_baseline;
use crate::passes::{PostProcessLutSelection, TerrainDebugView};

// Fast simulation can advance days per real second; keep aesthetic lighting below
// a temporal-aliasing threshold so day/night cannot appear as flash frames.
const VISUAL_DAY_NIGHT_MAX_HOURS_PER_SEC: f32 = 1.5;

pub(crate) fn visual_day_night_hour_from_date(date: GameDate) -> f32 {
    (date.hour as f32).rem_euclid(24.0)
}

pub(crate) fn advance_visual_day_night_hour(
    current_hour: f32,
    speed: GameSpeed,
    dt_secs: f32,
) -> f32 {
    if matches!(speed, GameSpeed::Paused) {
        return current_hour.rem_euclid(24.0);
    }
    let secs_per_hour = speed.seconds_per_hour();
    if !secs_per_hour.is_finite() || secs_per_hour <= 0.0 || dt_secs <= 0.0 {
        return current_hour.rem_euclid(24.0);
    }
    let game_hours = dt_secs / secs_per_hour;
    let visual_cap = VISUAL_DAY_NIGHT_MAX_HOURS_PER_SEC * dt_secs;
    (current_hour + game_hours.min(visual_cap)).rem_euclid(24.0)
}

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
    camera: &Camera,
    _world: &hoi4_state::World,
    map_space: &VanillaMapSpace,
) -> PostProcessLutSelection {
    let world_to_map_px = (map_space.world_to_map_px[0] + map_space.world_to_map_px[1]) * 0.5;
    let camera_height_px = camera.eye().y.max(0.0) * world_to_map_px;
    PostProcessLutSelection::from_camera_height_px(camera_height_px)
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
        MapMode::Political => 1.0,
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
        map_baseline::MapBaselineLayer::GradientBorderCh1RgbDebug => {
            TerrainDebugView::GradientBorderCh1Rgb
        }
        map_baseline::MapBaselineLayer::GradientBorderCh1AlphaDebug => {
            TerrainDebugView::GradientBorderCh1Alpha
        }
        map_baseline::MapBaselineLayer::GradientBorderCh2RgbDebug => {
            TerrainDebugView::GradientBorderCh2Rgb
        }
        map_baseline::MapBaselineLayer::GradientBorderCh2AlphaDebug => {
            TerrainDebugView::GradientBorderCh2Alpha
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visual_day_night_freezes_when_paused() {
        let hour = advance_visual_day_night_hour(23.5, GameSpeed::Paused, 10.0);
        assert!((hour - 23.5).abs() < f32::EPSILON);
    }

    #[test]
    fn visual_day_night_caps_fast_simulation() {
        let hour = advance_visual_day_night_hour(12.0, GameSpeed::Speed5, 1.0);
        assert!((hour - 13.5).abs() < 0.001);
    }

    #[test]
    fn visual_day_night_wraps_at_end_of_day() {
        let hour = advance_visual_day_night_hour(23.75, GameSpeed::Speed1, 1.0);
        assert!(hour < 2.0);
    }
}
