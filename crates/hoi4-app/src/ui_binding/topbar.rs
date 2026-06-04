use crate::{recruitable_manpower, App};

pub fn build_data(app: &App) -> hoi4_ui::topbar::TopBarData {
    let player = app.player_country;
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
        speed_index: match app.world.speed {
            hoi4_state::GameSpeed::Paused => 0,
            hoi4_state::GameSpeed::Speed1 => 1,
            hoi4_state::GameSpeed::Speed2 => 2,
            hoi4_state::GameSpeed::Speed3 => 3,
            hoi4_state::GameSpeed::Speed4 => 4,
            hoi4_state::GameSpeed::Speed5 => 5,
        },
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
