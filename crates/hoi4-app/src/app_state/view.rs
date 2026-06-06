use crate::menu_pass::{CountryEntry, MenuButton};
use crate::menu_scene::MenuKind;

pub(crate) struct ViewState {
    pub(crate) game_phase: crate::GamePhase,
    pub(crate) player_country: usize,
    pub(crate) country_select_idx: usize,
    pub(crate) menu_kind: Option<MenuKind>,
    pub(crate) menu_hovered_btn: Option<&'static str>,
    pub(crate) menu_hovered_row: Option<usize>,
    pub(crate) available_countries: Vec<CountryEntry>,
    pub(crate) last_main_buttons: Vec<MenuButton>,
    pub(crate) last_country_layout: Option<crate::menu_pass::CountrySelectLayout>,
}

impl ViewState {
    pub(crate) fn new(default_player: usize, available_countries: Vec<CountryEntry>) -> Self {
        Self {
            game_phase: crate::GamePhase::MainMenu,
            player_country: default_player,
            country_select_idx: 0,
            menu_kind: Some(MenuKind::MainMenu),
            menu_hovered_btn: None,
            menu_hovered_row: None,
            available_countries,
            last_main_buttons: Vec::new(),
            last_country_layout: None,
        }
    }
}
