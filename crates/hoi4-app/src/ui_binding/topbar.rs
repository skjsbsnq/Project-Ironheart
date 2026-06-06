use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time::Instant;

use crate::{recruitable_manpower, App};

const TOPBAR_DATA_REFRESH_SECS: f32 = 0.10;

pub fn build_data_cached(app: &mut App) -> hoi4_ui::topbar::TopBarData {
    let sig = topbar_signature(app);
    let stale = app.cached_topbar_data.is_none()
        || app.cached_topbar_sig != sig
        || app.last_topbar_rebuild_at.elapsed().as_secs_f32() >= TOPBAR_DATA_REFRESH_SECS;
    if stale {
        let data = build_data(app);
        app.cached_topbar_sig = sig;
        app.cached_topbar_data = Some(data);
        app.last_topbar_rebuild_at = Instant::now();
    }
    app.cached_topbar_data
        .clone()
        .unwrap_or_else(|| build_data(app))
}

pub fn build_data(app: &App) -> hoi4_ui::topbar::TopBarData {
    let player = app.view.player_country;
    let player_cid = hoi4_state::CountryId(player as u16);
    let treasury = app.world.countries.treasury.treasuries.get(player);
    let country_tag = app
        .world
        .countries
        .tags
        .get(player)
        .cloned()
        .unwrap_or_default();
    let ruling_party = app
        .world
        .countries
        .ruling_party
        .get(player)
        .cloned()
        .unwrap_or_default();
    hoi4_ui::topbar::TopBarData {
        country_name: app.country_display_name(player_cid),
        country_tag,
        ruling_party,
        political_power: app
            .world
            .countries
            .political_power
            .get(player)
            .copied()
            .unwrap_or(0.0),
        stability: app
            .world
            .countries
            .stability
            .get(player)
            .copied()
            .unwrap_or(0.5),
        war_support: app
            .world
            .countries
            .war_support
            .get(player)
            .copied()
            .unwrap_or(0.5),
        manpower: recruitable_manpower(&app.world, &app.v6_db, player_cid),
        gdp_gbp: treasury.map(|t| t.gdp_gbp).unwrap_or(0.0),
        gdp_growth_yoy: treasury.map(|t| t.gdp_growth_yoy).unwrap_or(0.0),
        construction_points: App::v6_construction_points(&app.world, player_cid) as f32,
        date: format!("{}", app.world.date),
        date_year: app.world.date.year,
        date_month: app.world.date.month,
        date_day: app.world.date.day,
        speed_index: speed_index(app.world.speed),
    }
}

fn topbar_signature(app: &App) -> u64 {
    let mut h = DefaultHasher::new();
    app.view.game_phase.hash(&mut h);
    app.view.player_country.hash(&mut h);
    app.world.date.year.hash(&mut h);
    app.world.date.month.hash(&mut h);
    app.world.date.day.hash(&mut h);
    speed_index(app.world.speed).hash(&mut h);
    if app.view.player_country < app.world.countries.count {
        app.world
            .countries
            .tags
            .get(app.view.player_country)
            .hash(&mut h);
        app.world
            .countries
            .ruling_party
            .get(app.view.player_country)
            .hash(&mut h);
        app.world
            .countries
            .political_power
            .get(app.view.player_country)
            .map(|value| quantize_f32(*value, 10.0))
            .hash(&mut h);
        app.world
            .countries
            .stability
            .get(app.view.player_country)
            .map(|value| quantize_f32(*value, 1000.0))
            .hash(&mut h);
        app.world
            .countries
            .war_support
            .get(app.view.player_country)
            .map(|value| quantize_f32(*value, 1000.0))
            .hash(&mut h);
        if let Some(treasury) = app
            .world
            .countries
            .treasury
            .treasuries
            .get(app.view.player_country)
        {
            quantize_f64(treasury.gdp_gbp, 10.0).hash(&mut h);
            quantize_f32(treasury.gdp_growth_yoy, 100.0).hash(&mut h);
        }
    }
    h.finish()
}

fn quantize_f32(value: f32, scale: f32) -> i64 {
    (value * scale).round() as i64
}

fn quantize_f64(value: f64, scale: f64) -> i64 {
    (value * scale).round() as i64
}

fn speed_index(speed: hoi4_state::GameSpeed) -> u8 {
    match speed {
        hoi4_state::GameSpeed::Paused => 0,
        hoi4_state::GameSpeed::Speed1 => 1,
        hoi4_state::GameSpeed::Speed2 => 2,
        hoi4_state::GameSpeed::Speed3 => 3,
        hoi4_state::GameSpeed::Speed4 => 4,
        hoi4_state::GameSpeed::Speed5 => 5,
    }
}

pub fn apply_topbar_action(app: &mut App, action: hoi4_ui::TopbarAction) {
    match action {
        hoi4_ui::TopbarAction::SetSpeed(cmd) => {
            app.world.speed = match cmd {
                hoi4_ui::topbar::SpeedCommand::Pause => hoi4_state::GameSpeed::Paused,
                hoi4_ui::topbar::SpeedCommand::Speed1 => hoi4_state::GameSpeed::Speed1,
                hoi4_ui::topbar::SpeedCommand::Speed2 => hoi4_state::GameSpeed::Speed2,
                hoi4_ui::topbar::SpeedCommand::Speed3 => hoi4_state::GameSpeed::Speed3,
                hoi4_ui::topbar::SpeedCommand::Speed4 => hoi4_state::GameSpeed::Speed4,
                hoi4_ui::topbar::SpeedCommand::Speed5 => hoi4_state::GameSpeed::Speed5,
            };
        }
    }
}
