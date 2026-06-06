use std::collections::HashSet;

use hoi4_app::ui_data::cache::UiPanelCache;
use crate::InGamePanel;

pub(crate) struct UiStateBundle {
    pub(crate) open_panel: Option<InGamePanel>,
    pub(crate) active_detail_panel: Option<hoi4_ui::ActiveDetailPanel>,
    pub(crate) active_popup: Option<hoi4_ui::ActivePopup>,
    pub(crate) diplomacy_sort_by_opinion: bool,
    pub(crate) diplomacy_selected_country_tag: Option<String>,
    pub(crate) construction_mode: Option<String>,
    pub(crate) construction_highlight_province_ids: HashSet<u32>,
    pub(crate) focus_panel: hoi4_ui::focus_tree_panel::FocusTreePanel,
    pub(crate) pre_event_speed: Option<hoi4_state::GameSpeed>,
    pub(crate) last_event_sound_id: Option<String>,
    pub(crate) pending_surrender_notifications:
        Vec<hoi4_ui::surrender_notification::SurrenderNotification>,
    pub(crate) last_surrender_sound_key: Option<String>,
    pub(crate) country_info_panel: hoi4_ui::country_info_panel::CountryInfoPanel,
    pub(crate) province_info_card: hoi4_ui::province_info::ProvinceInfoCard,
    pub(crate) province_info_data: hoi4_ui::province_info::ProvinceInfoData,
    pub(crate) ui_sounds: hoi4_audio::UiSoundBank,
    pub(crate) settings: hoi4_ui::settings::Settings,
    pub(crate) settings_panel: hoi4_ui::settings::SettingsPanel,
    pub(crate) save_browser: hoi4_ui::save_browser::SaveBrowser,
    pub(crate) end_screen: hoi4_ui::end_screen::EndScreen,
    pub(crate) prev_focuses_completed: u32,
    pub(crate) law_error_message: Option<String>,
    pub(crate) last_law_error_toast: Option<String>,
    pub(crate) panel_cache: UiPanelCache,
}

impl UiStateBundle {
    pub(crate) fn new(
        ui_sounds: hoi4_audio::UiSoundBank,
        settings: hoi4_ui::settings::Settings,
        settings_panel: hoi4_ui::settings::SettingsPanel,
        save_browser: hoi4_ui::save_browser::SaveBrowser,
    ) -> Self {
        Self {
            open_panel: None,
            active_detail_panel: None,
            active_popup: None,
            diplomacy_sort_by_opinion: false,
            diplomacy_selected_country_tag: None,
            construction_mode: None,
            construction_highlight_province_ids: HashSet::new(),
            focus_panel: hoi4_ui::focus_tree_panel::FocusTreePanel::new(),
            pre_event_speed: None,
            last_event_sound_id: None,
            pending_surrender_notifications: Vec::new(),
            last_surrender_sound_key: None,
            country_info_panel: hoi4_ui::country_info_panel::CountryInfoPanel::new(),
            province_info_card: hoi4_ui::province_info::ProvinceInfoCard::new(),
            province_info_data: hoi4_ui::province_info::ProvinceInfoData::default(),
            ui_sounds,
            settings,
            settings_panel,
            save_browser,
            end_screen: hoi4_ui::end_screen::EndScreen::new(),
            prev_focuses_completed: 0,
            law_error_message: None,
            last_law_error_toast: None,
            panel_cache: UiPanelCache::default(),
        }
    }
}
