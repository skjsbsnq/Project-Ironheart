use crate::{App, GamePhase, InGamePanel};

pub fn build_frame_model(app: &mut App) -> hoi4_ui::UiFrameModel {
    if app.view.game_phase != GamePhase::Playing {
        return hoi4_ui::UiFrameModel::empty();
    }

    hoi4_ui::UiFrameModel {
        topbar: Some(super::topbar::build_data_cached(app)),
        active_primary_panel: app.ui_state.open_panel.and_then(active_primary_panel),
        active_detail_panel: app.ui_state.active_detail_panel.clone(),
        active_popup: app.ui_state.active_popup.clone(),
        open_panel: app.ui_state.open_panel.and_then(panel_kind),
    }
}

fn active_primary_panel(panel: InGamePanel) -> Option<hoi4_ui::ActivePrimaryPanel> {
    Some(match panel {
        InGamePanel::Politics => hoi4_ui::ActivePrimaryPanel::Politics,
        InGamePanel::Decisions => hoi4_ui::ActivePrimaryPanel::Decisions,
        InGamePanel::NationalFocus => hoi4_ui::ActivePrimaryPanel::Focus,
        InGamePanel::Research => hoi4_ui::ActivePrimaryPanel::Research,
        InGamePanel::Diplomacy => hoi4_ui::ActivePrimaryPanel::Diplomacy,
        InGamePanel::Military => hoi4_ui::ActivePrimaryPanel::Military,
        InGamePanel::Naval => hoi4_ui::ActivePrimaryPanel::Naval,
        InGamePanel::Air => hoi4_ui::ActivePrimaryPanel::Air,
        InGamePanel::Laws => hoi4_ui::ActivePrimaryPanel::Politics,
        InGamePanel::Market => hoi4_ui::ActivePrimaryPanel::Market,
        InGamePanel::Pops => hoi4_ui::ActivePrimaryPanel::Pops,
        InGamePanel::ConstructionV6 => hoi4_ui::ActivePrimaryPanel::Construction,
        InGamePanel::Finance => hoi4_ui::ActivePrimaryPanel::Finance,
        InGamePanel::Trade => hoi4_ui::ActivePrimaryPanel::Trade,
        InGamePanel::Logistics => hoi4_ui::ActivePrimaryPanel::Logistics,
        InGamePanel::Situation => hoi4_ui::ActivePrimaryPanel::Situation,
        InGamePanel::Settings => hoi4_ui::ActivePrimaryPanel::Settings,
        InGamePanel::Saves => hoi4_ui::ActivePrimaryPanel::Saves,
    })
}

fn panel_kind(panel: InGamePanel) -> Option<hoi4_ui::PanelKind> {
    Some(match panel {
        InGamePanel::Politics | InGamePanel::NationalFocus => hoi4_ui::PanelKind::Politics,
        InGamePanel::Decisions => hoi4_ui::PanelKind::Decisions,
        InGamePanel::Laws => hoi4_ui::PanelKind::Politics,
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

pub fn apply_panel_commands(
    app: &mut App,
    commands: impl IntoIterator<Item = hoi4_ui::PanelCommand>,
) {
    let mut popup_commands = Vec::new();
    let mut detail_commands = Vec::new();
    let mut primary_commands = Vec::new();

    for command in commands {
        match command {
            hoi4_ui::PanelCommand::OpenPopup(_)
            | hoi4_ui::PanelCommand::ClosePopup
            | hoi4_ui::PanelCommand::Back => popup_commands.push(command),
            hoi4_ui::PanelCommand::OpenDetail(_)
            | hoi4_ui::PanelCommand::ReplaceDetail(_)
            | hoi4_ui::PanelCommand::CloseDetail => detail_commands.push(command),
            hoi4_ui::PanelCommand::OpenPrimary(_) | hoi4_ui::PanelCommand::ClosePrimary => {
                primary_commands.push(command)
            }
        }
    }

    for command in popup_commands
        .into_iter()
        .chain(detail_commands)
        .chain(primary_commands)
    {
        apply_panel_command(app, command);
    }
}

fn apply_panel_command(app: &mut App, command: hoi4_ui::PanelCommand) {
    match command {
        hoi4_ui::PanelCommand::OpenPrimary(primary) => {
            app.ui_state.open_panel = Some(in_game_panel_for_active_primary(primary));
            app.ui_state.province_info_card.open = false;
            app.ui_state.country_info_panel.close();
        }
        hoi4_ui::PanelCommand::ClosePrimary => {
            app.close_primary_panel();
        }
        hoi4_ui::PanelCommand::OpenDetail(detail)
        | hoi4_ui::PanelCommand::ReplaceDetail(detail) => {
            app.ui_state.active_detail_panel = Some(detail);
        }
        hoi4_ui::PanelCommand::CloseDetail => {
            app.ui_state.active_detail_panel = None;
        }
        hoi4_ui::PanelCommand::OpenPopup(popup) => {
            app.ui_state.active_popup = Some(popup);
        }
        hoi4_ui::PanelCommand::ClosePopup => {
            app.ui_state.active_popup = None;
        }
        hoi4_ui::PanelCommand::Back => {
            if app.ui_state.active_popup.is_some() {
                app.ui_state.active_popup = None;
            } else if app.ui_state.active_detail_panel.is_some() {
                app.ui_state.active_detail_panel = None;
            } else {
                app.close_primary_panel();
            }
        }
    }
}

fn in_game_panel_for_active_primary(panel: hoi4_ui::ActivePrimaryPanel) -> InGamePanel {
    match panel {
        hoi4_ui::ActivePrimaryPanel::Finance => InGamePanel::Finance,
        hoi4_ui::ActivePrimaryPanel::Construction => InGamePanel::ConstructionV6,
        hoi4_ui::ActivePrimaryPanel::Market => InGamePanel::Market,
        hoi4_ui::ActivePrimaryPanel::Trade => InGamePanel::Trade,
        hoi4_ui::ActivePrimaryPanel::Pops => InGamePanel::Pops,
        hoi4_ui::ActivePrimaryPanel::Politics => InGamePanel::Politics,
        hoi4_ui::ActivePrimaryPanel::Laws => InGamePanel::Politics,
        hoi4_ui::ActivePrimaryPanel::Research => InGamePanel::Research,
        hoi4_ui::ActivePrimaryPanel::Focus => InGamePanel::NationalFocus,
        hoi4_ui::ActivePrimaryPanel::Diplomacy => InGamePanel::Diplomacy,
        hoi4_ui::ActivePrimaryPanel::Military => InGamePanel::Military,
        hoi4_ui::ActivePrimaryPanel::Naval => InGamePanel::Naval,
        hoi4_ui::ActivePrimaryPanel::Air => InGamePanel::Air,
        hoi4_ui::ActivePrimaryPanel::Logistics => InGamePanel::Logistics,
        hoi4_ui::ActivePrimaryPanel::Decisions => InGamePanel::Decisions,
        hoi4_ui::ActivePrimaryPanel::Situation => InGamePanel::Situation,
        hoi4_ui::ActivePrimaryPanel::Settings => InGamePanel::Settings,
        hoi4_ui::ActivePrimaryPanel::Saves => InGamePanel::Saves,
    }
}
