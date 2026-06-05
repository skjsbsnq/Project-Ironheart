use crate::topbar::{SpeedCommand, TopBarData};

#[derive(Debug, Clone, PartialEq)]
pub struct UiFrameModel {
    pub topbar: Option<TopBarData>,
    /// Gate 1 panel router: the single active primary panel slot.
    pub active_primary_panel: Option<ActivePrimaryPanel>,
    /// Gate 1 panel router: the single active object/detail panel slot.
    pub active_detail_panel: Option<ActiveDetailPanel>,
    /// Gate 1 panel router: short-lived modal/popup slot.
    pub active_popup: Option<ActivePopup>,
    /// Legacy compatibility alias for V9 side rail and already-migrated callers.
    ///
    /// Migration relation: `PanelKind` maps one-to-one to `ActivePrimaryPanel`.
    /// New code should emit `PanelCommand::OpenPrimary` instead of adding
    /// `*_secondary_tabs` or mutating this field directly.
    pub open_panel: Option<PanelKind>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivePrimaryPanel {
    Finance,
    Construction,
    Market,
    Trade,
    Pops,
    Politics,
    /// Legacy primary entry kept until Gate 5.2 folds laws into Politics.
    Laws,
    Research,
    Focus,
    Diplomacy,
    Military,
    Naval,
    Air,
    Logistics,
    Decisions,
    Situation,
    Settings,
    Saves,
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

impl ActivePrimaryPanel {
    pub fn from_panel_kind(kind: PanelKind) -> Self {
        match kind {
            PanelKind::Politics => Self::Politics,
            PanelKind::Decisions => Self::Decisions,
            PanelKind::Laws => Self::Politics,
            PanelKind::Pops => Self::Pops,
            PanelKind::Market => Self::Market,
            PanelKind::Finance => Self::Finance,
            PanelKind::Trade => Self::Trade,
            PanelKind::Construction => Self::Construction,
            PanelKind::Research => Self::Research,
            PanelKind::Diplomacy => Self::Diplomacy,
            PanelKind::Military => Self::Military,
            PanelKind::Naval => Self::Naval,
            PanelKind::Air => Self::Air,
            PanelKind::Logistics => Self::Logistics,
            PanelKind::Situation => Self::Situation,
            PanelKind::Settings => Self::Settings,
            PanelKind::Saves => Self::Saves,
        }
    }
}

impl From<ActivePrimaryPanel> for PanelKind {
    fn from(panel: ActivePrimaryPanel) -> Self {
        match panel {
            ActivePrimaryPanel::Finance => Self::Finance,
            ActivePrimaryPanel::Construction => Self::Construction,
            ActivePrimaryPanel::Market => Self::Market,
            ActivePrimaryPanel::Trade => Self::Trade,
            ActivePrimaryPanel::Pops => Self::Pops,
            ActivePrimaryPanel::Politics | ActivePrimaryPanel::Focus => Self::Politics,
            ActivePrimaryPanel::Laws => Self::Politics,
            ActivePrimaryPanel::Research => Self::Research,
            ActivePrimaryPanel::Diplomacy => Self::Diplomacy,
            ActivePrimaryPanel::Military => Self::Military,
            ActivePrimaryPanel::Naval => Self::Naval,
            ActivePrimaryPanel::Air => Self::Air,
            ActivePrimaryPanel::Logistics => Self::Logistics,
            ActivePrimaryPanel::Decisions => Self::Decisions,
            ActivePrimaryPanel::Situation => Self::Situation,
            ActivePrimaryPanel::Settings => Self::Settings,
            ActivePrimaryPanel::Saves => Self::Saves,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActiveDetailPanel {
    Building(BuildingDetailTarget),
    Goods(GoodsDetailTarget),
    State(StateDetailTarget),
    Province(ProvinceDetailTarget),
    Country(CountryDetailTarget),
    Army(ArmyDetailTarget),
    Fleet(FleetDetailTarget),
    AirWing(AirWingDetailTarget),
    Law {
        category: String,
        law_id: Option<String>,
    },
    Technology(TechnologyDetailTarget),
    Focus(FocusDetailTarget),
    PopGroup(PopGroupDetailTarget),
    JournalEntry(JournalEntryDetailTarget),
    FinanceDebt,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoodsDetailTarget {
    pub good_id: String,
    pub source: Option<DetailSource>,
    pub context: Option<String>,
}

impl GoodsDetailTarget {
    pub fn new(good_id: impl Into<String>) -> Self {
        Self {
            good_id: good_id.into(),
            source: None,
            context: None,
        }
    }

    pub fn from_source(good_id: impl Into<String>, source: DetailSource) -> Self {
        Self {
            good_id: good_id.into(),
            source: Some(source),
            context: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildingDetailTarget {
    pub building_key: String,
    pub state_id: Option<u16>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StateDetailTarget {
    pub state_id: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProvinceDetailTarget {
    pub province_id: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CountryDetailTarget {
    pub tag: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArmyDetailTarget {
    pub army_id: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FleetDetailTarget {
    pub fleet_id: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AirWingDetailTarget {
    pub air_wing_id: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TechnologyDetailTarget {
    pub technology_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusDetailTarget {
    pub focus_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PopGroupDetailTarget {
    pub pop_group_key: String,
    pub state_id: Option<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JournalEntryDetailTarget {
    pub entry_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetailSource {
    Market,
    Finance,
    Construction,
    Trade,
    Logistics,
    Map,
    Politics,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActivePopup {
    Confirm(ConfirmPopup),
    Rename(RenamePopup),
    ProductionMethodPicker(ProductionMethodTarget),
    EventChoice(String),
    ErrorMessage(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfirmPopup {
    pub title: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenamePopup {
    pub title: String,
    pub current_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductionMethodTarget {
    pub building_key: String,
    pub method_group: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PanelCommand {
    OpenPrimary(ActivePrimaryPanel),
    ClosePrimary,
    OpenDetail(ActiveDetailPanel),
    ReplaceDetail(ActiveDetailPanel),
    CloseDetail,
    OpenPopup(ActivePopup),
    ClosePopup,
    Back,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UiAction {
    Topbar(TopbarAction),
    Panel(PanelAction),
    PanelCommand(PanelCommand),
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

impl From<PanelAction> for PanelCommand {
    fn from(action: PanelAction) -> Self {
        match action {
            PanelAction::Open(kind) => {
                PanelCommand::OpenPrimary(ActivePrimaryPanel::from_panel_kind(kind))
            }
            PanelAction::Close(_) => PanelCommand::ClosePrimary,
        }
    }
}

impl UiFrameModel {
    pub fn empty() -> Self {
        Self {
            topbar: None,
            active_primary_panel: None,
            active_detail_panel: None,
            active_popup: None,
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
        assert_eq!(frame.active_primary_panel, None);
        assert_eq!(frame.active_detail_panel, None);
        assert_eq!(frame.active_popup, None);
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

    #[test]
    fn gate1_router_expresses_primary_and_detail_pairs() {
        let finance = ActivePrimaryPanel::Finance;
        let goods = ActiveDetailPanel::Goods(GoodsDetailTarget::from_source(
            "steel",
            DetailSource::Finance,
        ));
        let law = ActiveDetailPanel::Law {
            category: "Taxation".to_owned(),
            law_id: Some("medium_tax".to_owned()),
        };
        let state = ActiveDetailPanel::State(StateDetailTarget { state_id: 42 });

        assert_eq!(PanelKind::from(finance), PanelKind::Finance);
        assert!(matches!(goods, ActiveDetailPanel::Goods(_)));
        assert!(matches!(law, ActiveDetailPanel::Law { .. }));
        assert!(matches!(state, ActiveDetailPanel::State(_)));
    }

    #[test]
    fn panel_commands_cover_gate1_slots() {
        let commands = [
            PanelCommand::OpenPrimary(ActivePrimaryPanel::Market),
            PanelCommand::OpenDetail(ActiveDetailPanel::Goods(GoodsDetailTarget::new("oil"))),
            PanelCommand::OpenPopup(ActivePopup::ErrorMessage("不可执行".to_owned())),
            PanelCommand::ClosePopup,
            PanelCommand::CloseDetail,
            PanelCommand::ClosePrimary,
            PanelCommand::Back,
        ];

        assert_eq!(commands.len(), 7);
    }
}
