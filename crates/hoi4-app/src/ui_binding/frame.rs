use crate::{App, GamePhase, InGamePanel};

pub fn build_frame_model(app: &App) -> hoi4_ui::UiFrameModel {
    if app.game_phase != GamePhase::Playing {
        return hoi4_ui::UiFrameModel::empty();
    }

    hoi4_ui::UiFrameModel {
        topbar: Some(super::topbar::build_data(app)),
        open_panel: app.open_panel.and_then(panel_kind),
    }
}

fn panel_kind(panel: InGamePanel) -> Option<hoi4_ui::PanelKind> {
    Some(match panel {
        InGamePanel::Politics | InGamePanel::NationalFocus => hoi4_ui::PanelKind::Politics,
        InGamePanel::Decisions => hoi4_ui::PanelKind::Decisions,
        InGamePanel::Laws => hoi4_ui::PanelKind::Laws,
        InGamePanel::Pops => hoi4_ui::PanelKind::Pops,
        InGamePanel::Market => hoi4_ui::PanelKind::Market,
        InGamePanel::Finance => hoi4_ui::PanelKind::Finance,
        InGamePanel::Trade => hoi4_ui::PanelKind::Trade,
        InGamePanel::ConstructionV6 => hoi4_ui::PanelKind::Construction,
        InGamePanel::Research => hoi4_ui::PanelKind::Research,
        InGamePanel::Diplomacy => hoi4_ui::PanelKind::Diplomacy,
        InGamePanel::Military => hoi4_ui::PanelKind::Military,
        InGamePanel::Naval => hoi4_ui::PanelKind::Naval,
        InGamePanel::Air => hoi4_ui::PanelKind::Air,
        InGamePanel::Logistics => hoi4_ui::PanelKind::Logistics,
        InGamePanel::Situation => hoi4_ui::PanelKind::Situation,
        InGamePanel::Settings => hoi4_ui::PanelKind::Settings,
        InGamePanel::Saves => hoi4_ui::PanelKind::Saves,
    })
}
