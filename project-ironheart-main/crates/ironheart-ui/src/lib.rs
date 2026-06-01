pub mod v9;

use ironheart_sim::SimSnapshot;

#[derive(Debug, Clone, PartialEq)]
pub struct UiFrameModel {
    pub topbar: TopbarModel,
    pub side_rail: SideRailModel,
    pub active_panel: Option<PanelKind>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TopbarModel {
    pub date_label: String,
    pub speed_label: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SideRailModel {
    pub entries: Vec<PanelKind>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelKind {
    Politics,
    Decisions,
    Laws,
    Population,
    Market,
    Finance,
    Trade,
    Construction,
    Research,
    Diplomacy,
    Military,
    Navy,
    Air,
    Logistics,
    Situation,
    Settings,
}

pub struct UiRuntime;

impl UiRuntime {
    pub fn new() -> Self {
        Self
    }

    pub fn build_frame(&self, snapshot: SimSnapshot) -> UiFrameModel {
        UiFrameModel {
            topbar: TopbarModel {
                date_label: snapshot.date_label,
                speed_label: "Paused".to_string(),
            },
            side_rail: SideRailModel {
                entries: vec![
                    PanelKind::Politics,
                    PanelKind::Decisions,
                    PanelKind::Construction,
                    PanelKind::Research,
                    PanelKind::Diplomacy,
                    PanelKind::Military,
                    PanelKind::Settings,
                ],
            },
            active_panel: None,
        }
    }
}

impl Default for UiRuntime {
    fn default() -> Self {
        Self::new()
    }
}
