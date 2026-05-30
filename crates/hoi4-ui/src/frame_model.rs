use crate::topbar::{SpeedCommand, TopBarData};

#[derive(Debug, Clone, PartialEq)]
pub struct UiFrameModel {
    pub topbar: Option<TopBarData>,
    pub open_panel: Option<PanelKind>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelKind {
    Politics,
    Decisions,
    Laws,
    Pops,
    Market,
    Finance,
    Trade,
    Construction,
    Research,
    Diplomacy,
    Military,
    Naval,
    Air,
    Logistics,
    Situation,
    Settings,
    Saves,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UiAction {
    Topbar(TopbarAction),
    Panel(PanelAction),
    Settings(crate::settings::SettingsCommand),
    None,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TopbarAction {
    SetSpeed(SpeedCommand),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PanelAction {
    Open(PanelKind),
    Close(PanelKind),
}

impl UiFrameModel {
    pub fn empty() -> Self {
        Self {
            topbar: None,
            open_panel: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_frame_has_no_gameplay_refs() {
        let frame = UiFrameModel::empty();

        assert_eq!(frame.topbar, None);
        assert_eq!(frame.open_panel, None);
    }

    #[test]
    fn topbar_action_is_debuggable_and_comparable() {
        let action = UiAction::Topbar(TopbarAction::SetSpeed(SpeedCommand::Speed3));

        assert_eq!(
            action,
            UiAction::Topbar(TopbarAction::SetSpeed(SpeedCommand::Speed3))
        );
        assert!(format!("{action:?}").contains("Speed3"));
    }
}
