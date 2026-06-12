use std::collections::HashSet;

use crate::InGamePanel;
use hoi4_app::ui_data::cache::UiPanelCache;

pub(crate) fn is_politics_runtime_panel(panel: Option<InGamePanel>) -> bool {
    matches!(panel, Some(InGamePanel::Politics | InGamePanel::Laws))
}

pub(crate) fn politics_close_required_before_switch(
    current: Option<InGamePanel>,
    next: InGamePanel,
) -> bool {
    is_politics_runtime_panel(current) && !matches!(next, InGamePanel::Politics | InGamePanel::Laws)
}

pub(crate) struct UiStateBundle {
    pub(crate) open_panel: Option<InGamePanel>,
    pub(crate) show_election_panel: bool,
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
    pub(crate) pending_primary_panel_after_politics_close: Option<InGamePanel>,
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
            show_election_panel: false,
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
            pending_primary_panel_after_politics_close: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gate5_politics_close_request_keeps_panel_until_finish() {
        assert!(is_politics_runtime_panel(Some(InGamePanel::Politics)));
        assert!(is_politics_runtime_panel(Some(InGamePanel::Laws)));
        assert!(!is_politics_runtime_panel(Some(InGamePanel::Research)));
        assert!(!is_politics_runtime_panel(None));
    }

    #[test]
    fn gate5_politics_switches_to_other_panel_after_close_phase() {
        assert!(politics_close_required_before_switch(
            Some(InGamePanel::Politics),
            InGamePanel::Research
        ));
        assert!(politics_close_required_before_switch(
            Some(InGamePanel::Laws),
            InGamePanel::Decisions
        ));
        assert!(!politics_close_required_before_switch(
            Some(InGamePanel::Politics),
            InGamePanel::Laws
        ));
        assert!(!politics_close_required_before_switch(
            Some(InGamePanel::Research),
            InGamePanel::Politics
        ));
    }
}
