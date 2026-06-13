use super::render::UiRenderOutput;
use crate::app_command::AppCommand;
use crate::*;

#[derive(Default)]
pub(crate) struct UiCommandApplyOutput {
    pub(crate) topbar_action: Option<hoi4_ui::TopbarAction>,
    pub(crate) side_rail_panel_cmd: Option<hoi4_ui::PanelKind>,
    pub(crate) open_panel_kind: Option<hoi4_ui::PanelKind>,
    pub(crate) panel_commands: Vec<hoi4_ui::PanelCommand>,
    pub(crate) deferred_switch_player_country: Vec<String>,
    pub(crate) egui_current_stats: hoi4_ui::UiFrameStats,
}

pub(crate) fn apply_ui_render_output(
    app: &mut App,
    output: UiRenderOutput,
) -> UiCommandApplyOutput {
    app.apply_ui_render_output_inner(output)
}

pub(crate) fn apply_deferred_ui_commands(app: &mut App, output: UiCommandApplyOutput) {
    app.apply_deferred_ui_commands_inner(output);
}

impl App {
    fn apply_ui_render_output_inner(&mut self, ui_output: UiRenderOutput) -> UiCommandApplyOutput {
        let ui_driver::render::UiRenderOutput {
            open_panel_kind,
            commands,
            egui_current_stats,
        } = ui_output;
        let mut topbar_speed_cmd: Option<hoi4_ui::topbar::SpeedCommand> = None;
        let mut side_rail_panel_cmd: Option<hoi4_ui::PanelKind> = None;
        let mut panel_commands: Vec<hoi4_ui::PanelCommand> = Vec::new();
        let mut close_active_panel = false;
        let mut finish_politics_close = false;
        let mut law_close = false;
        let mut construction_v6_close = false;
        let mut finance_cmds: Vec<hoi4_ui::finance_panel::FinanceCommand> = Vec::new();
        let mut construction_v6_cmds: Vec<hoi4_ui::construction_v6_panel::ConstructionV6Command> =
            Vec::new();
        let mut law_cmds: Vec<hoi4_ui::law_panel::LawCommand> = Vec::new();
        let mut politics_decision_cmds: Vec<hoi4_ui::politics::DecisionCommand> = Vec::new();
        let mut research_cmds: Vec<hoi4_ui::research::ResearchCommand> = Vec::new();
        let mut diplomacy_cmds: Vec<hoi4_ui::diplomacy::DiplomacyCommand> = Vec::new();
        let mut military_cmds: Vec<hoi4_ui::military::MilitaryCommand> = Vec::new();
        let mut air_cmds: Vec<hoi4_ui::air::AirCommand> = Vec::new();
        let mut naval_cmds: Vec<hoi4_ui::naval::NavalCommand> = Vec::new();
        let mut situation_cmds: Vec<hoi4_ui::situation_panel::SituationCommand> = Vec::new();
        let mut focus_cmd: Option<hoi4_ui::focus_tree_panel::FocusCommand> = None;
        let mut country_info_cmds: Vec<hoi4_ui::country_info_panel::CountryInfoCommand> =
            Vec::new();
        let mut counter_menu_cmd: Option<&'static str> = None;
        let mut event_cmd: Option<hoi4_ui::event_panel::EventCommand> = None;
        let mut surrender_notif_cmd: Option<
            hoi4_ui::surrender_notification::SurrenderNotificationCommand,
        > = None;
        let mut settings_cmds: Vec<hoi4_ui::settings::SettingsCommand> = Vec::new();
        let mut save_cmds: Vec<hoi4_ui::save_browser::SaveCommand> = Vec::new();
        let mut end_cmd: Option<hoi4_ui::end_screen::EndCommand> = None;
        for command in commands {
            match command {
                AppCommand::TopbarSpeed(cmd) => topbar_speed_cmd = Some(cmd),
                AppCommand::SideRailPanel(cmd) => side_rail_panel_cmd = Some(cmd),
                AppCommand::Panel(cmd) => panel_commands.push(cmd),
                AppCommand::CloseActivePanel => close_active_panel = true,
                AppCommand::FinishPoliticsClose => finish_politics_close = true,
                AppCommand::CloseLawPanel => law_close = true,
                AppCommand::CloseConstructionPanel => construction_v6_close = true,
                AppCommand::Finance(cmd) => finance_cmds.push(cmd),
                AppCommand::ConstructionV6(cmd) => construction_v6_cmds.push(cmd),
                AppCommand::Law(cmd) => law_cmds.push(cmd),
                AppCommand::Decision(cmd) => politics_decision_cmds.push(cmd),
                AppCommand::Research(cmd) => research_cmds.push(cmd),
                AppCommand::Diplomacy(cmd) => diplomacy_cmds.push(cmd),
                AppCommand::Military(cmd) => military_cmds.push(cmd),
                AppCommand::Air(cmd) => air_cmds.push(cmd),
                AppCommand::Naval(cmd) => naval_cmds.push(cmd),
                AppCommand::Situation(cmd) => situation_cmds.push(cmd),
                AppCommand::Focus(cmd) => focus_cmd = Some(cmd),
                AppCommand::CountryInfo(cmd) => country_info_cmds.push(cmd),
                AppCommand::CounterMenu(cmd) => counter_menu_cmd = Some(cmd),
                AppCommand::Event(cmd) => event_cmd = Some(cmd),
                AppCommand::SurrenderNotification(cmd) => surrender_notif_cmd = Some(cmd),
                AppCommand::Settings(cmd) => settings_cmds.push(cmd),
                AppCommand::Save(cmd) => save_cmds.push(cmd),
                AppCommand::End(cmd) => end_cmd = Some(cmd),
            }
        }
        let mut deferred_switch_player_country: Vec<String> = Vec::new();
        if self.state.is_none() {
            return UiCommandApplyOutput::default();
        }

        let topbar_action = topbar_speed_cmd.map(hoi4_ui::TopbarAction::SetSpeed);

        if finish_politics_close {
            self.finish_country_politics_close_request();
            self.ui_state.law_error_message = None;
        }
        if close_active_panel {
            self.close_primary_panel();
        }
        if law_close {
            self.close_primary_panel();
            if !matches!(
                self.ui_state.open_panel,
                Some(InGamePanel::Politics | InGamePanel::Laws)
            ) {
                self.ui_state.law_error_message = None;
            }
        }

        if !finance_cmds.is_empty() {
            let player = self.view.player_country;
            for cmd in &finance_cmds {
                let content_cmd = match cmd {
                    hoi4_ui::finance_panel::FinanceCommand::IssueDomesticBond { amount_rm } => {
                        Some(hoi4_content::FinanceCommand::IssueDomesticBond {
                            amount_rm: *amount_rm,
                        })
                    }
                    hoi4_ui::finance_panel::FinanceCommand::IssueForeignBond { amount_gbp } => {
                        Some(hoi4_content::FinanceCommand::IssueForeignBond {
                            amount_gbp: *amount_gbp,
                        })
                    }
                    hoi4_ui::finance_panel::FinanceCommand::PrintMefo => {
                        Some(hoi4_content::FinanceCommand::PrintMefo)
                    }
                    hoi4_ui::finance_panel::FinanceCommand::SellGold { kg } => {
                        Some(hoi4_content::FinanceCommand::SellGold { kg: *kg })
                    }
                    hoi4_ui::finance_panel::FinanceCommand::BuyForeignCurrency { gbp_amount } => {
                        Some(hoi4_content::FinanceCommand::BuyForeignCurrency {
                            gbp_amount: *gbp_amount,
                        })
                    }
                    hoi4_ui::finance_panel::FinanceCommand::Panel(panel_cmd) => {
                        panel_commands.push(panel_cmd.clone());
                        None
                    }
                };
                if let Some(content_cmd) = content_cmd {
                    let _ = hoi4_content::execute_finance_command(
                        &mut self.world,
                        &self.v6_db,
                        player,
                        &content_cmd,
                    );
                }
            }
        }

        let mut construction_highlight_changed = false;
        if construction_v6_close {
            self.close_primary_panel();
            if self.ui_state.construction_mode.is_some() {
                self.ui_state.construction_mode = None;
                self.ui_state.construction_highlight_province_ids.clear();
                construction_highlight_changed = true;
            }
        }
        for cmd in construction_v6_cmds {
            if let hoi4_ui::construction_v6_panel::ConstructionV6Command::Panel(panel_cmd) = &cmd {
                panel_commands.push(panel_cmd.clone());
                continue;
            }
            let effect =
                hoi4_app::ui_data::construction_commands::apply_construction_control_command(
                    &cmd,
                    &mut self.world,
                    &mut self.runtime.econ,
                    &self.v6_db,
                    self.view.player_country,
                    &mut self.runtime.auto_build_enabled,
                    &mut self.ui_state.construction_mode,
                    &mut self.ui_state.construction_highlight_province_ids,
                );
            if effect.handled {
                if effect.reset_auto_build_month {
                    self.runtime.last_auto_build_month = None;
                }
                construction_highlight_changed |= effect.highlight_changed;
            }
        }

        if !law_cmds.is_empty() {
            let player_id = hoi4_state::CountryId(self.view.player_country as u16);
            for cmd in law_cmds {
                use hoi4_ui::law_panel::LawCommand;
                match cmd {
                    LawCommand::SwitchLaw {
                        category,
                        target_law_id,
                    } => {
                        match hoi4_content::set_law(
                            &mut self.world,
                            player_id,
                            category,
                            &target_law_id,
                            &self.v6_db,
                        ) {
                            Ok(()) => {
                                self.ui_state.law_error_message = None;
                            }
                            Err(e) => {
                                println!(
                                    "[law] switch failed: {:?} -> {}: {}",
                                    category, target_law_id, e
                                );
                                self.ui_state.law_error_message =
                                    Some(format!("法律切换失败：{}", e));
                            }
                        }
                    }
                    LawCommand::Blocked { reason } => {
                        self.ui_state.law_error_message = Some(reason);
                    }
                }
            }
        }

        if !politics_decision_cmds.is_empty() {
            let player_id = hoi4_state::CountryId(self.view.player_country as u16);
            for cmd in politics_decision_cmds {
                use hoi4_ui::politics::DecisionCommand;
                match cmd {
                    DecisionCommand::Activate(id) => {
                        match self.runtime.content.decision_state.activate(
                            &id,
                            &self.runtime.content.decision_db,
                            &mut self.world,
                            player_id,
                            &mut self.runtime.content.global_flags,
                        ) {
                            Ok(()) => println!("[decision] activated: {id}"),
                            Err(e) => println!("[decision] activate failed: {id}: {e}"),
                        }
                    }
                    DecisionCommand::OpenFocusTree => {
                        self.ui_state.focus_panel.open = true;
                    }
                    DecisionCommand::ToggleElectionPanel => {
                        self.ui_state.show_election_panel = !self.ui_state.show_election_panel;
                    }
                    DecisionCommand::Panel(panel_cmd) => {
                        panel_commands.push(panel_cmd);
                    }
                }
            }
        }

        if construction_highlight_changed {
            let player_cid = if self.view.player_country < self.world.countries.count {
                Some(hoi4_state::CountryId(self.view.player_country as u16))
            } else {
                None
            };
            let mut color_lut = build_color_lut(&self.world, self.map_mode, player_cid);
            for pid in self
                .interaction
                .selected_province_ids
                .iter()
                .chain(self.ui_state.construction_highlight_province_ids.iter())
            {
                let o = *pid as usize * 4;
                if o + 3 < color_lut.len() {
                    color_lut[o] = color_lut[o].saturating_add(42);
                    color_lut[o + 1] = color_lut[o + 1].saturating_add(32);
                    color_lut[o + 2] = color_lut[o + 2].saturating_sub(18);
                }
            }
            if let Some(s) = self.state.as_mut() {
                color_lut.resize((s.lut_width * s.lut_height * 4) as usize, 0);
                upload_lut(
                    &s.queue,
                    &s.lut_texture,
                    &color_lut,
                    s.lut_width,
                    s.lut_height,
                );
                s.window.request_redraw();
            }
        }

        for cmd in research_cmds {
            use hoi4_ui::research::ResearchCommand;
            match cmd {
                ResearchCommand::StartResearch(tech_key) => {
                    let _ = self.runtime.research.start(
                        &self.world,
                        hoi4_state::CountryId(self.view.player_country as u16),
                        &tech_key,
                        &self.v6_db,
                    );
                }
                ResearchCommand::Panel(panel_cmd) => {
                    panel_commands.push(panel_cmd);
                }
            }
        }

        for cmd in diplomacy_cmds {
            use hoi4_logic::diplomacy::{execute_action, DiplomaticAction};
            use hoi4_ui::diplomacy::DiplomacyCommand;
            let player_cid = hoi4_state::CountryId(self.view.player_country as u16);
            match cmd {
                DiplomacyCommand::JustifyWargoal(tag) => {
                    if let Some(&target) = self.world.tag_to_country.get(&tag) {
                        if let Err(err) = execute_action(
                            &mut self.world,
                            player_cid,
                            DiplomaticAction::StartJustification {
                                target,
                                kind: hoi4_state::WargoalType::Annex,
                                target_state: None,
                            },
                        ) {
                            println!("[diplomacy] Justify wargoal against {tag} failed: {err:?}");
                        }
                    }
                }
                DiplomacyCommand::DeclareWar(tag) => {
                    if let Some(&target) = self.world.tag_to_country.get(&tag) {
                        if let Err(err) = execute_action(
                            &mut self.world,
                            player_cid,
                            DiplomaticAction::DeclareWar { target },
                        ) {
                            println!("[diplomacy] Declare war on {tag} failed: {err:?}");
                        } else {
                            self.render_toggles.frontline_overlay_hash = 0;
                        }
                    }
                }
                DiplomacyCommand::CreateFaction => {
                    if let Err(err) = execute_action(
                        &mut self.world,
                        player_cid,
                        DiplomaticAction::CreateFaction {
                            name: "Player Faction".to_owned(),
                        },
                    ) {
                        println!("[diplomacy] Create faction failed: {err:?}");
                    }
                }
                DiplomacyCommand::LeaveFaction => {
                    if let Err(err) =
                        execute_action(&mut self.world, player_cid, DiplomaticAction::LeaveFaction)
                    {
                        println!("[diplomacy] Leave faction failed: {err:?}");
                    }
                }
                DiplomacyCommand::InviteToFaction(tag) => {
                    if let Some(&target) = self.world.tag_to_country.get(&tag) {
                        if let Err(err) = execute_action(
                            &mut self.world,
                            player_cid,
                            DiplomaticAction::InviteToFaction { target },
                        ) {
                            println!("[diplomacy] Invite {tag} to faction failed: {err:?}");
                        }
                    }
                }
                DiplomacyCommand::RequestMilitaryAccess(tag) => {
                    if let Some(&target) = self.world.tag_to_country.get(&tag) {
                        if let Err(err) = execute_action(
                            &mut self.world,
                            player_cid,
                            DiplomaticAction::RequestMilitaryAccess { target },
                        ) {
                            println!("[diplomacy] Request access from {tag} failed: {err:?}");
                        }
                    }
                }
                DiplomacyCommand::ResolvePeace {
                    war_id,
                    winning_side,
                } => {
                    let side = match winning_side {
                        hoi4_ui::diplomacy::PeaceSide::Attacker => hoi4_state::WarSide::Attacker,
                        hoi4_ui::diplomacy::PeaceSide::Defender => hoi4_state::WarSide::Defender,
                    };
                    if execute_action(
                        &mut self.world,
                        player_cid,
                        DiplomaticAction::ResolvePeace {
                            war_id,
                            winning_side: side,
                        },
                    )
                    .is_ok()
                    {
                        println!("[peace] war #{war_id} resolved through unified diplomacy action");
                        self.render_toggles.frontline_overlay_hash = 0;
                        let player_cid = if self.view.player_country < self.world.countries.count {
                            Some(hoi4_state::CountryId(self.view.player_country as u16))
                        } else {
                            None
                        };
                        let mut color_lut = build_color_lut(&self.world, self.map_mode, player_cid);
                        if let Some(s) = self.state.as_mut() {
                            color_lut.resize((s.lut_width * s.lut_height * 4) as usize, 0);
                            upload_lut(
                                &s.queue,
                                &s.lut_texture,
                                &color_lut,
                                s.lut_width,
                                s.lut_height,
                            );
                            s.window.request_redraw();
                        }
                    }
                }
                DiplomacyCommand::Panel(panel_cmd) => {
                    panel_commands.push(panel_cmd);
                }
            }
        }

        for cmd in situation_cmds {
            use hoi4_ui::situation_panel::SituationCommand;
            match cmd {
                SituationCommand::Intervene {
                    situation_id,
                    intervention_id,
                } => {
                    let player = hoi4_state::CountryId(self.view.player_country as u16);
                    let ci = self.view.player_country;
                    let stockpile = &mut self.runtime.econ.stockpile[ci];
                    self.runtime.content.situation_state.intervene(
                        &situation_id,
                        &intervention_id,
                        player,
                        &mut self.world,
                        Some(stockpile),
                    );
                }
            }
        }
        for cmd in naval_cmds {
            match cmd {
                hoi4_ui::naval::NavalCommand::SetMission { fleet_id, mission } => {
                    let fi = fleet_id as usize;
                    if fi < self.world.fleets.count
                        && self.world.fleets.owners[fi]
                            == hoi4_state::CountryId(self.view.player_country as u16)
                    {
                        self.world.fleets.mission[fi] = naval_mission_from_ui(mission);
                    }
                }
                hoi4_ui::naval::NavalCommand::MoveToSelectedSeaRegion { fleet_id } => {
                    let fi = fleet_id as usize;
                    if fi < self.world.fleets.count
                        && self.world.fleets.owners[fi]
                            == hoi4_state::CountryId(self.view.player_country as u16)
                    {
                        let selected_sea_region = if self.selected_province_id != u32::MAX {
                            self.world
                                .map
                                .definitions
                                .get(self.selected_province_id as usize)
                                .and_then(|def| def.as_ref())
                                .filter(|def| def.province_type == hoi4_map::ProvinceType::Sea)
                                .map(|def| def.id as u32)
                        } else {
                            None
                        };
                        if let Some(region) = selected_sea_region {
                            let now = self.world.date.hours_since_epoch().max(0) as u64;
                            let _ = hoi4_logic::naval::movement::order_move_to_region(
                                &mut self.world,
                                hoi4_state::FleetId(fleet_id),
                                region,
                                now,
                            );
                            self.interaction.pending_naval_move_fleet = None;
                        } else if self.interaction.pending_naval_move_fleet == Some(fleet_id) {
                            self.interaction.pending_naval_move_fleet = None;
                        } else {
                            self.interaction.pending_naval_move_fleet = Some(fleet_id);
                        }
                    }
                }
                hoi4_ui::naval::NavalCommand::SplitFleet { fleet_id, count } => {
                    Self::split_fleet_data(
                        &mut self.world,
                        hoi4_state::CountryId(self.view.player_country as u16),
                        fleet_id,
                        count,
                    );
                }
                hoi4_ui::naval::NavalCommand::DisbandEmptyFleet { fleet_id } => {
                    let fi = fleet_id as usize;
                    let player = hoi4_state::CountryId(self.view.player_country as u16);
                    if fi < self.world.fleets.count
                        && self.world.fleets.owners[fi] == player
                        && self.world.fleets.ships[fi].is_empty()
                    {
                        self.world.fleets.owners[fi] = hoi4_state::CountryId::NONE;
                        if self.interaction.naval_transfer_source_fleet == Some(fleet_id) {
                            self.interaction.naval_transfer_source_fleet = None;
                        }
                        if self.interaction.pending_naval_move_fleet == Some(fleet_id) {
                            self.interaction.pending_naval_move_fleet = None;
                        }
                    }
                }
                hoi4_ui::naval::NavalCommand::SetTransferSource { fleet_id } => {
                    let fi = fleet_id as usize;
                    if fi < self.world.fleets.count
                        && self.world.fleets.owners[fi]
                            == hoi4_state::CountryId(self.view.player_country as u16)
                    {
                        self.interaction.naval_transfer_source_fleet = Some(fleet_id);
                    }
                }
                hoi4_ui::naval::NavalCommand::ClearTransferSource => {
                    self.interaction.naval_transfer_source_fleet = None;
                }
                hoi4_ui::naval::NavalCommand::TransferShips {
                    from_fleet_id,
                    to_fleet_id,
                    count,
                } => {
                    Self::transfer_ships_between_fleets_data(
                        &mut self.world,
                        hoi4_state::CountryId(self.view.player_country as u16),
                        from_fleet_id,
                        to_fleet_id,
                        count,
                    );
                }
                hoi4_ui::naval::NavalCommand::Panel(panel_cmd) => {
                    panel_commands.push(panel_cmd);
                }
            }
        }
        for cmd in air_cmds {
            match cmd {
                hoi4_ui::air::AirCommand::SetMission { wing_id, mission } => {
                    let wi = wing_id as usize;
                    if wi < self.world.air_wings.count
                        && self.world.air_wings.owners[wi]
                            == hoi4_state::CountryId(self.view.player_country as u16)
                    {
                        self.world.air_wings.mission[wi] = air_mission_from_ui(mission);
                        if self.world.air_wings.target_region[wi] == u32::MAX {
                            self.world.air_wings.target_region[wi] =
                                self.world.air_wings.region_id[wi];
                        }
                    }
                }
                hoi4_ui::air::AirCommand::TransferToSelectedState { wing_id } => {
                    let wi = wing_id as usize;
                    if wi < self.world.air_wings.count
                        && self.world.air_wings.owners[wi]
                            == hoi4_state::CountryId(self.view.player_country as u16)
                    {
                        let selected_state = if self.selected_province_id != u32::MAX {
                            self.world
                                .provinces
                                .state_of
                                .get(self.selected_province_id as usize)
                                .copied()
                                .unwrap_or(hoi4_state::StateId::NONE)
                        } else {
                            hoi4_state::StateId::NONE
                        };
                        if !selected_state.is_none() {
                            let now = self.world.date.hours_since_epoch().max(0) as u64;
                            let _ = hoi4_logic::air::operations::order_transfer_to_base(
                                &mut self.world,
                                hoi4_state::AirWingId(wing_id),
                                selected_state,
                                selected_state.0 as u32,
                                now,
                            );
                            self.interaction.pending_air_transfer_wing = None;
                        } else if self.interaction.pending_air_transfer_wing == Some(wing_id) {
                            self.interaction.pending_air_transfer_wing = None;
                        } else {
                            self.interaction.pending_air_transfer_wing = Some(wing_id);
                        }
                    }
                }
                hoi4_ui::air::AirCommand::ToggleReinforce { wing_id } => {
                    let wi = wing_id as usize;
                    if wi < self.world.air_wings.count
                        && self.world.air_wings.owners[wi]
                            == hoi4_state::CountryId(self.view.player_country as u16)
                    {
                        self.world.air_wings.reinforce_enabled[wi] =
                            !self.world.air_wings.reinforce_enabled[wi];
                    }
                }
                hoi4_ui::air::AirCommand::SplitWing { wing_id, planes } => {
                    Self::split_air_wing_data(
                        &mut self.world,
                        hoi4_state::CountryId(self.view.player_country as u16),
                        wing_id,
                        planes,
                    );
                }
                hoi4_ui::air::AirCommand::DisbandEmptyWing { wing_id } => {
                    let wi = wing_id as usize;
                    let player = hoi4_state::CountryId(self.view.player_country as u16);
                    if wi < self.world.air_wings.count
                        && self.world.air_wings.owners[wi] == player
                        && self.world.air_wings.count_planes[wi] == 0
                    {
                        self.world.air_wings.owners[wi] = hoi4_state::CountryId::NONE;
                        if self.interaction.air_transfer_source_wing == Some(wing_id) {
                            self.interaction.air_transfer_source_wing = None;
                        }
                        if self.interaction.pending_air_transfer_wing == Some(wing_id) {
                            self.interaction.pending_air_transfer_wing = None;
                        }
                    }
                }
                hoi4_ui::air::AirCommand::SetTransferSource { wing_id } => {
                    let wi = wing_id as usize;
                    if wi < self.world.air_wings.count
                        && self.world.air_wings.owners[wi]
                            == hoi4_state::CountryId(self.view.player_country as u16)
                    {
                        self.interaction.air_transfer_source_wing = Some(wing_id);
                    }
                }
                hoi4_ui::air::AirCommand::ClearTransferSource => {
                    self.interaction.air_transfer_source_wing = None;
                }
                hoi4_ui::air::AirCommand::TransferPlanes {
                    from_wing_id,
                    to_wing_id,
                    planes,
                } => {
                    Self::transfer_planes_between_wings_data(
                        &mut self.world,
                        hoi4_state::CountryId(self.view.player_country as u16),
                        from_wing_id,
                        to_wing_id,
                        planes,
                    );
                }
                hoi4_ui::air::AirCommand::Panel(panel_cmd) => {
                    panel_commands.push(panel_cmd);
                }
            }
        }
        for cmd in military_cmds {
            use hoi4_ui::military::MilitaryCommand;
            match cmd {
                MilitaryCommand::Train(template_idx) => {
                    let owner = hoi4_state::CountryId(self.view.player_country as u16);
                    let capital_state = self
                        .world
                        .countries
                        .capitals
                        .get(self.view.player_country)
                        .copied()
                        .unwrap_or(hoi4_state::StateId::NONE);
                    let data = self.world.data.clone();
                    if let Err(err) = hoi4_logic::military::training::enqueue_training(
                        &self.world,
                        &mut self.runtime.econ,
                        &data,
                        owner,
                        template_idx as u32,
                        1,
                        capital_state,
                        hoi4_data::ReinforcementPriority::Normal,
                    ) {
                        eprintln!("[military] enqueue training failed: {err:?}");
                    }
                }
                MilitaryCommand::NewTemplate => {
                    let tag = self
                        .world
                        .country_tag(hoi4_state::CountryId(self.view.player_country as u16));
                    if let Some(tag) = tag.map(str::to_owned) {
                        let data = std::sync::Arc::make_mut(&mut self.world.data);
                        let templates = data.division_templates.entry(tag.clone()).or_default();
                        let id = templates.len() as u32;
                        let template = hoi4_data::EditableDivisionTemplate::empty(
                            id,
                            format!("New Template {}", id + 1),
                            Some(tag),
                        )
                        .to_division_template();
                        templates.push(template);
                        self.interaction.selected_template_idx = Some((templates.len() - 1) as u16);
                        self.interaction.template_editor_open = true;
                    }
                }
                MilitaryCommand::OpenTemplateEditor(template_idx) => {
                    self.interaction.selected_template_idx = Some(template_idx);
                    self.interaction.template_editor_open = true;
                }
                MilitaryCommand::CloseTemplateEditor => {
                    self.interaction.template_editor_open = false;
                }
                MilitaryCommand::OpenLineSubunitPicker { template, row, col } => {
                    self.interaction.selected_template_idx = Some(template);
                    self.interaction.template_editor_open = true;
                    self.interaction.template_picker_target =
                        Some(hoi4_ui::military::TemplatePickerTarget::Line { template, row, col });
                }
                MilitaryCommand::OpenSupportSubunitPicker { template, slot } => {
                    self.interaction.selected_template_idx = Some(template);
                    self.interaction.template_editor_open = true;
                    self.interaction.template_picker_target =
                        Some(hoi4_ui::military::TemplatePickerTarget::Support { template, slot });
                }
                MilitaryCommand::CloseSubunitPicker => {
                    self.interaction.template_picker_target = None;
                }
                MilitaryCommand::SelectTemplate(template_idx) => {
                    self.interaction.selected_template_idx = Some(template_idx);
                }
                MilitaryCommand::RenameTemplate(template_idx, name) => {
                    let tag = self
                        .world
                        .country_tag(hoi4_state::CountryId(self.view.player_country as u16));
                    if let Some(tag) = tag.map(str::to_owned) {
                        let data = std::sync::Arc::make_mut(&mut self.world.data);
                        if let Some(templates) = data.division_templates.get_mut(&tag) {
                            let _ = hoi4_logic::military::templates::rename_template(
                                templates,
                                template_idx,
                                name,
                            );
                        }
                    }
                }
                MilitaryCommand::CloneTemplate(template_idx) => {
                    let tag = self
                        .world
                        .country_tag(hoi4_state::CountryId(self.view.player_country as u16));
                    if let Some(tag) = tag.map(str::to_owned) {
                        let data = std::sync::Arc::make_mut(&mut self.world.data);
                        if let Some(templates) = data.division_templates.get_mut(&tag) {
                            if let Ok(new_idx) = hoi4_logic::military::templates::clone_template(
                                templates,
                                template_idx,
                            ) {
                                self.interaction.selected_template_idx = Some(new_idx);
                            }
                        }
                    }
                }
                MilitaryCommand::DeleteTemplate(template_idx) => {
                    let tag = self
                        .world
                        .country_tag(hoi4_state::CountryId(self.view.player_country as u16));
                    if let Some(tag) = tag.map(str::to_owned) {
                        let data = std::sync::Arc::make_mut(&mut self.world.data);
                        if let Some(templates) = data.division_templates.get_mut(&tag) {
                            if (template_idx as usize) < templates.len() {
                                templates.remove(template_idx as usize);
                                self.interaction.template_picker_target = None;
                                self.interaction.selected_template_idx = if templates.is_empty() {
                                    None
                                } else {
                                    Some((template_idx as usize).min(templates.len() - 1) as u16)
                                };
                            }
                        }
                    }
                }
                MilitaryCommand::AddLineBattalion(template_idx, subunit) => {
                    let tag = self
                        .world
                        .country_tag(hoi4_state::CountryId(self.view.player_country as u16));
                    if let Some(tag) = tag.map(str::to_owned) {
                        let data = std::sync::Arc::make_mut(&mut self.world.data);
                        if let Some(templates) = data.division_templates.get_mut(&tag) {
                            if let Some((row, col)) = first_open_line_slot(templates, template_idx)
                            {
                                let _ = hoi4_logic::military::templates::set_line_battalion(
                                    templates,
                                    template_idx,
                                    row,
                                    col,
                                    Some(subunit),
                                );
                            }
                        }
                    }
                }
                MilitaryCommand::SetLineBattalion {
                    template,
                    row,
                    col,
                    subunit,
                } => {
                    let tag = self
                        .world
                        .country_tag(hoi4_state::CountryId(self.view.player_country as u16));
                    if let Some(tag) = tag.map(str::to_owned) {
                        let data = std::sync::Arc::make_mut(&mut self.world.data);
                        if let Some(templates) = data.division_templates.get_mut(&tag) {
                            let _ = hoi4_logic::military::templates::set_line_battalion(
                                templates, template, row, col, subunit,
                            );
                        }
                    }
                }
                MilitaryCommand::SetSupportCompany {
                    template,
                    slot,
                    subunit,
                } => {
                    let tag = self
                        .world
                        .country_tag(hoi4_state::CountryId(self.view.player_country as u16));
                    if let Some(tag) = tag.map(str::to_owned) {
                        let data = std::sync::Arc::make_mut(&mut self.world.data);
                        if let Some(templates) = data.division_templates.get_mut(&tag) {
                            let _ = hoi4_logic::military::templates::set_support_company(
                                templates, template, slot, subunit,
                            );
                        }
                    }
                }
                MilitaryCommand::AddSupportCompany(template_idx, subunit) => {
                    let tag = self
                        .world
                        .country_tag(hoi4_state::CountryId(self.view.player_country as u16));
                    if let Some(tag) = tag.map(str::to_owned) {
                        let data = std::sync::Arc::make_mut(&mut self.world.data);
                        if let Some(templates) = data.division_templates.get_mut(&tag) {
                            if let Some(slot) = first_open_support_slot(templates, template_idx) {
                                let _ = hoi4_logic::military::templates::set_support_company(
                                    templates,
                                    template_idx,
                                    slot,
                                    Some(subunit),
                                );
                            }
                        }
                    }
                }
                MilitaryCommand::CreateArmy => {
                    let owner = hoi4_state::CountryId(self.view.player_country as u16);
                    let members = self.interaction.selected_divisions.clone();
                    if members.is_empty() {
                        println!("[frontline] no divisions selected to create army");
                    } else {
                        let name = format!("Army {}", self.world.player_armies.len() + 1);
                        match hoi4_logic::military::frontline::create_army(
                            &mut self.world,
                            owner,
                            members,
                            name,
                        ) {
                            Ok(id) => {
                                self.interaction.selected_army_id = Some(id);
                                self.render_toggles.prev_armies_hash = 0;
                                println!("[frontline] created army {}", id.raw());
                            }
                            Err(e) => println!("[frontline] create_army failed: {e}"),
                        }
                    }
                }
                MilitaryCommand::DissolveArmy(id) => {
                    let aid = hoi4_state::ArmyId(id);
                    match hoi4_logic::military::frontline::dissolve_army(&mut self.world, aid) {
                        Ok(()) => {
                            self.render_toggles.prev_armies_hash = 0;
                            println!("[frontline] dissolved army {id}");
                        }
                        Err(e) => println!("[frontline] dissolve_army failed: {e}"),
                    }
                }
                MilitaryCommand::AddMembers(id) => {
                    let aid = hoi4_state::ArmyId(id);
                    let members = self.interaction.selected_divisions.clone();
                    if !members.is_empty() {
                        match hoi4_logic::military::frontline::add_members(
                            &mut self.world,
                            aid,
                            &members,
                        ) {
                            Ok(()) => {
                                self.render_toggles.prev_armies_hash = 0;
                                println!("[frontline] added {} members to army {id}", members.len())
                            }
                            Err(e) => println!("[frontline] add_members failed: {e}"),
                        }
                    }
                }
                MilitaryCommand::RemoveMembers(id) => {
                    let aid = hoi4_state::ArmyId(id);
                    let members = self.interaction.selected_divisions.clone();
                    if !members.is_empty() {
                        match hoi4_logic::military::frontline::remove_members(
                            &mut self.world,
                            aid,
                            &members,
                        ) {
                            Ok(()) => {
                                self.render_toggles.prev_armies_hash = 0;
                                println!(
                                    "[frontline] removed {} members from army {id}",
                                    members.len()
                                );
                            }
                            Err(e) => println!("[frontline] remove_members failed: {e}"),
                        }
                    }
                }
                MilitaryCommand::DrawFrontline(id) => {
                    self.interaction.frontline_painter.mode =
                        PainterMode::ArmyPainter(hoi4_state::ArmyId(id));
                    self.interaction.frontline_painter.samples.clear();
                    self.interaction.frontline_painter.last_sample_at = std::time::Instant::now();
                    println!("[frontline] entering frontline paint mode for army {id}");
                }
                MilitaryCommand::ClearFrontline(id) => {
                    let aid = hoi4_state::ArmyId(id);
                    match hoi4_logic::military::frontline::clear_frontline_path(
                        &mut self.world,
                        aid,
                    ) {
                        Ok(()) => {
                            self.render_toggles.prev_armies_hash = 0;
                            println!("[frontline] cleared frontline for army {id}");
                        }
                        Err(e) => println!("[frontline] clear_frontline_path failed: {e}"),
                    }
                }
                MilitaryCommand::DrawArrow(id) => {
                    let aid = hoi4_state::ArmyId(id);
                    let anchor = self
                        .world
                        .player_armies
                        .iter()
                        .find(|a| a.id == aid)
                        .and_then(|a| {
                            a.order.as_ref().and_then(|o| {
                                o.anchor
                                    .or_else(|| o.path.get(o.path.len() / 2).copied())
                                    .or_else(|| o.path.first().copied())
                            })
                        })
                        .unwrap_or(hoi4_state::ProvinceId(0));
                    self.interaction.frontline_painter.mode =
                        PainterMode::ArrowPainter(aid, anchor);
                    self.interaction.frontline_painter.samples.clear();
                    self.interaction.frontline_painter.last_sample_at = std::time::Instant::now();
                    println!("[frontline] entering arrow paint mode for army {id}");
                }
                MilitaryCommand::ClearArrow(id) => {
                    let aid = hoi4_state::ArmyId(id);
                    match hoi4_logic::military::frontline::clear_arrow(&mut self.world, aid) {
                        Ok(()) => {
                            self.render_toggles.prev_armies_hash = 0;
                            println!("[frontline] cleared arrow for army {id}");
                        }
                        Err(e) => println!("[frontline] clear_arrow failed: {e}"),
                    }
                }
                MilitaryCommand::ToggleOverlay => {
                    self.render_toggles.frontline_overlay_visible =
                        !self.render_toggles.frontline_overlay_visible;
                    self.render_toggles.prev_armies_hash = 0;
                    println!(
                        "[frontline] overlay visible: {}",
                        self.render_toggles.frontline_overlay_visible
                    );
                }
                MilitaryCommand::SelectArmy(id) => {
                    self.interaction.selected_army_id = Some(hoi4_state::ArmyId(id));
                }
                MilitaryCommand::ClearArmySelection => {
                    self.interaction.selected_army_id = None;
                }
                MilitaryCommand::AddSelectedDivisionsToArmy(id) => {
                    let aid = hoi4_state::ArmyId(id);
                    let members = self.interaction.selected_divisions.clone();
                    if !members.is_empty() {
                        match hoi4_logic::military::frontline::add_members(
                            &mut self.world,
                            aid,
                            &members,
                        ) {
                            Ok(()) => {
                                self.interaction.selected_army_id = Some(aid);
                                self.render_toggles.prev_armies_hash = 0;
                                println!(
                                    "[frontline] right-click added {} divisions to army {id}",
                                    members.len()
                                )
                            }
                            Err(e) => println!("[frontline] add_members failed: {e}"),
                        }
                    }
                }
                MilitaryCommand::ToggleDivisionSelection(idx) => {
                    if let Some(pos) = self
                        .interaction
                        .selected_divisions
                        .iter()
                        .position(|&x| x == idx)
                    {
                        self.interaction.selected_divisions.remove(pos);
                    } else {
                        self.interaction.selected_divisions.push(idx);
                    }
                    self.interaction.selected_army_id = None;
                }
                MilitaryCommand::SelectAllDivisions => {
                    let player = hoi4_state::CountryId(self.view.player_country as u16);
                    self.interaction.selected_divisions.clear();
                    for i in 0..self.world.divisions.count {
                        if self.world.divisions.owners[i] == player {
                            self.interaction.selected_divisions.push(i);
                        }
                    }
                    self.interaction.selected_army_id = None;
                }
                MilitaryCommand::Panel(panel_cmd) => {
                    panel_commands.push(panel_cmd);
                }
                MilitaryCommand::ExecutePlan(id) => {
                    let aid = hoi4_state::ArmyId(id);
                    match hoi4_logic::military::frontline::execute_plan(&mut self.world, aid) {
                        Ok(()) => {
                            hoi4_logic::military::frontline::tick_frontlines(&mut self.world);
                            self.render_toggles.prev_armies_hash = 0;
                            eprintln!("[frontline] executing plan for army {id}");
                        }
                        Err(e) => eprintln!("[frontline] execute_plan failed: {e}"),
                    }
                }
                MilitaryCommand::HaltPlan(id) => {
                    let aid = hoi4_state::ArmyId(id);
                    match hoi4_logic::military::frontline::halt_plan(&mut self.world, aid) {
                        Ok(()) => {
                            self.render_toggles.prev_armies_hash = 0;
                            eprintln!("[frontline] halted plan for army {id}");
                        }
                        Err(e) => eprintln!("[frontline] halt_plan failed: {e}"),
                    }
                }
                MilitaryCommand::AssignGeneral {
                    army_id,
                    general_id,
                } => {
                    let aid = hoi4_state::ArmyId(army_id);
                    let gid = hoi4_state::GeneralId(general_id);
                    let player = hoi4_state::CountryId(self.view.player_country as u16);
                    let general_ok = self
                        .world
                        .generals
                        .iter()
                        .any(|g| g.id == gid && g.owner == player);
                    let assigned_elsewhere = self
                        .world
                        .player_armies
                        .iter()
                        .any(|a| a.id != aid && a.commander == Some(gid));
                    if general_ok && !assigned_elsewhere {
                        if let Some(army) =
                            self.world.player_armies.iter_mut().find(|a| a.id == aid)
                        {
                            if army.owner == player {
                                army.commander = Some(gid);
                            }
                        }
                    }
                }
                MilitaryCommand::UnassignGeneral(id) => {
                    let aid = hoi4_state::ArmyId(id);
                    if let Some(army) = self.world.player_armies.iter_mut().find(|a| a.id == aid) {
                        army.commander = None;
                    }
                }
            }
        }

        if let Some(cmd) = focus_cmd {
            use hoi4_ui::focus_tree_panel::FocusCommand;
            let player = hoi4_state::CountryId(self.view.player_country as u16);
            match cmd {
                FocusCommand::Start(id) => {
                    // P0.3??tart_focus ???????available ???
                    if !hoi4_content::start_focus(
                        &mut self.world,
                        player,
                        &self.runtime.content.focus_tree,
                        &id,
                        &self.runtime.content.global_flags,
                    ) {
                        println!("[focus] skipped {}: unavailable", id);
                    }
                }
                FocusCommand::Cancel => {
                    let i = self.view.player_country;
                    self.world.countries.current_focus[i] = None;
                    self.world.countries.focus_progress[i] = 0.0;
                }
                FocusCommand::Panel(panel_cmd) => {
                    panel_commands.push(panel_cmd);
                }
            }
        }

        if let Some(cmd) = event_cmd {
            use hoi4_ui::event_panel::EventCommand;
            match cmd {
                EventCommand::PickOption {
                    event_id,
                    option_idx,
                } => {
                    self.ui_state
                        .ui_sounds
                        .play_with_fallback(UiSound::OptionClick, UiSound::Click);
                    println!("[event] resolved: {event_id} option={option_idx}");
                    let (_, report) = self.runtime.content.event_scheduler.resolve_option(
                        option_idx,
                        &mut self.world,
                        &mut self.runtime.content.global_flags,
                    );
                    deferred_switch_player_country.extend(report.switch_player_country);
                    let cascaded = self
                        .runtime
                        .content
                        .process_pending_triggers(&mut self.world);
                    deferred_switch_player_country
                        .extend(cascaded.effect_report.switch_player_country);
                    for w in report
                        .warnings
                        .iter()
                        .chain(cascaded.effect_report.warnings.iter())
                    {
                        println!("[effect] ???: {w}");
                    }
                    for e in report
                        .errors
                        .iter()
                        .chain(cascaded.effect_report.errors.iter())
                    {
                        println!("[effect] ???: {e}");
                    }
                    for target in &cascaded.cascaded_triggers {
                        println!("[event] cascaded trigger: {target}");
                    }
                    if cascaded.pause_for_country_event {
                        if self.world.speed != GameSpeed::Paused
                            && self.ui_state.pre_event_speed.is_none()
                        {
                            self.ui_state.pre_event_speed = Some(self.world.speed);
                        }
                        self.world.speed = GameSpeed::Paused;
                    }
                    if self.runtime.content.event_scheduler.pending_len() == 0 {
                        if let Some(prev) = self.ui_state.pre_event_speed.take() {
                            if self.world.speed == GameSpeed::Paused {
                                self.world.speed = prev;
                                println!("[event] queue cleared, restored speed: {:?}", prev);
                            }
                        }
                    }
                }
            }
        }

        if let Some(hoi4_ui::surrender_notification::SurrenderNotificationCommand::Acknowledge) =
            surrender_notif_cmd
        {
            self.ui_state
                .ui_sounds
                .play_with_fallback(UiSound::OptionClick, UiSound::Click);
            self.ui_state.pending_surrender_notifications.remove(0);
            if self.ui_state.pending_surrender_notifications.is_empty() {
                self.ui_state.last_surrender_sound_key = None;
            }
        }

        // ???????????????????????????
        if !country_info_cmds.is_empty() {
            use hoi4_logic::diplomacy::{
                debug_grant_justified_wargoal, execute_action, DiplomaticAction,
            };
            use hoi4_ui::country_info_panel::CountryInfoCommand;
            let player = hoi4_state::CountryId(self.view.player_country as u16);
            for cmd in country_info_cmds {
                match cmd {
                    CountryInfoCommand::JustifyWargoal { target_tag } => {
                        if let Some(&target) = self.world.tag_to_country.get(&target_tag) {
                            if let Err(err) = execute_action(
                                &mut self.world,
                                player,
                                DiplomaticAction::StartJustification {
                                    target,
                                    kind: hoi4_state::WargoalType::Annex,
                                    target_state: None,
                                },
                            ) {
                                println!(
                                    "[diplomacy] Justify wargoal against {target_tag} failed: {err:?}"
                                );
                            }
                        }
                    }
                    CountryInfoCommand::DeclareWar { target_tag } => {
                        if let Some(&target) = self.world.tag_to_country.get(&target_tag) {
                            if self.ui_state.settings.instant_war {
                                debug_grant_justified_wargoal(
                                    &mut self.world,
                                    player,
                                    target,
                                    hoi4_state::WargoalType::Annex,
                                    None,
                                );
                            }
                            match execute_action(
                                &mut self.world,
                                player,
                                DiplomaticAction::DeclareWar { target },
                            ) {
                                Ok(outcome) => {
                                    println!(
                                        "[diplomacy] Declared war on {target_tag} (war_id={:?})",
                                        outcome.war_id
                                    );
                                }
                                Err(e) => {
                                    println!(
                                        "[diplomacy] Declare war on {target_tag} failed: {e:?}"
                                    );
                                }
                            }
                            self.ui_state.country_info_panel.close();
                        }
                    }
                    CountryInfoCommand::InviteToFaction { target_tag } => {
                        if let Some(&target) = self.world.tag_to_country.get(&target_tag) {
                            if let Err(err) = execute_action(
                                &mut self.world,
                                player,
                                DiplomaticAction::InviteToFaction { target },
                            ) {
                                println!(
                                    "[diplomacy] Invite {target_tag} to faction failed: {err:?}"
                                );
                            }
                        }
                    }
                    CountryInfoCommand::RequestMilitaryAccess { target_tag } => {
                        if let Some(&target) = self.world.tag_to_country.get(&target_tag) {
                            if let Err(err) = execute_action(
                                &mut self.world,
                                player,
                                DiplomaticAction::RequestMilitaryAccess { target },
                            ) {
                                println!(
                                    "[diplomacy] Request access from {target_tag} failed: {err:?}"
                                );
                            }
                        }
                    }
                    CountryInfoCommand::Close => {
                        self.ui_state.country_info_panel.close();
                    }
                }
            }
        }

        // CR-4.5: Handle counter right-click menu commands.
        if let Some(cmd) = counter_menu_cmd {
            match cmd {
                "move" => {
                    // Select divisions in that province, then set pending_move_command
                    if let Some(pid) = self.interaction.counter_right_click_province {
                        let player = hoi4_state::CountryId(self.view.player_country as u16);
                        let prov = hoi4_state::ProvinceId(pid as u16);
                        self.interaction.selected_divisions.clear();
                        for i in 0..self.world.divisions.count {
                            if self.world.divisions.owners[i] == player
                                && self.world.divisions.locations[i] == prov
                            {
                                self.interaction.selected_divisions.push(i);
                            }
                        }
                        self.interaction.pending_move_command = true;
                    }
                }
                "cancel" => {
                    if let Some(pid) = self.interaction.counter_right_click_province {
                        let player = hoi4_state::CountryId(self.view.player_country as u16);
                        let prov = hoi4_state::ProvinceId(pid as u16);
                        for i in 0..self.world.divisions.count {
                            if self.world.divisions.owners[i] == player
                                && self.world.divisions.locations[i] == prov
                            {
                                self.world.divisions.destinations[i] = None;
                                hoi4_logic::military::command_executor::clear_division_command(
                                    &mut self.world,
                                    i,
                                );
                                self.division_motion.remove(&i);
                            }
                        }
                    }
                    self.interaction.counter_right_click_province = None;
                }
                "disband" => {
                    if let Some(pid) = self.interaction.counter_right_click_province {
                        println!("[counter_menu] Disband requested for province {pid}");
                    }
                    self.interaction.counter_right_click_province = None;
                }
                _ => {
                    self.interaction.counter_right_click_province = None;
                }
            }
        }
        for cmd in settings_cmds {
            let language_changed = match cmd {
                hoi4_ui::settings::SettingsCommand::SetLanguage(lang) => Some(lang),
                _ => None,
            };
            let Some(s) = self.state.as_mut() else {
                continue;
            };
            ui_binding::settings::apply_command(
                &mut self.ui_state.settings,
                &mut self.ui_state.settings_panel,
                &mut self.runtime.music_player,
                &mut self.ui_state.ui_sounds,
                &s.window,
                &mut s.ui,
                cmd,
            );
            let dpi = s.ui_scale_factor();
            let logical_w = s.config.width as f32 / dpi;
            let logical_h = s.config.height as f32 / dpi;
            self.camera.aspect = logical_w / logical_h.max(1.0);
            self.camera.clamp_target_to_map();
            let cam = CameraUniform::from_camera(&self.camera, HEIGHT_SCALE, LAT_CORRECTION);
            s.queue
                .write_buffer(&s.camera_buffer, 0, bytemuck::bytes_of(&cam));
            s.text_pass.set_screen_size(&s.queue, logical_w, logical_h);
            if let Some(lang) = language_changed {
                self.loc_catalog = load_loc_catalog_for_language(&self.path_cfg, lang);
            }
        }

        for cmd in save_cmds {
            use hoi4_ui::save_browser::SaveCommand;
            match cmd {
                SaveCommand::Load(path) => {
                    println!("[save] loading: {}", path.display());
                    if let Err(e) = hoi4_state::save::read(&path, &mut self.world) {
                        self.ui_state.save_browser.last_error = Some(format!("load failed: {e}"));
                    } else {
                        self.visual_day_night_hour =
                            visual_day_night_hour_from_date(self.world.date);
                        self.ui_state.save_browser.open = false;
                        self.close_primary_panel();
                    }
                }
                SaveCommand::Delete(path) => {
                    if let Err(e) = std::fs::remove_file(&path) {
                        self.ui_state.save_browser.last_error = Some(format!("delete failed: {e}"));
                    } else {
                        // Rescan.
                        let dir = self.ui_state.save_browser.saves_dir.clone();
                        let entries = hoi4_ui::save_browser::scan_saves(&dir, |p| {
                            hoi4_state::save::read_meta(p)
                                .ok()
                                .map(|m| (format!("{}", m.date), m.player_tag))
                        });
                        self.ui_state.save_browser.set_saves(entries);
                    }
                }
                SaveCommand::Rename { from, to } => {
                    if let Err(e) = std::fs::rename(&from, &to) {
                        self.ui_state.save_browser.last_error = Some(format!("rename failed: {e}"));
                    } else {
                        let dir = self.ui_state.save_browser.saves_dir.clone();
                        let entries = hoi4_ui::save_browser::scan_saves(&dir, |p| {
                            hoi4_state::save::read_meta(p)
                                .ok()
                                .map(|m| (format!("{}", m.date), m.player_tag))
                        });
                        self.ui_state.save_browser.set_saves(entries);
                    }
                }
                SaveCommand::Rescan => {
                    let dir = self.ui_state.save_browser.saves_dir.clone();
                    let entries = hoi4_ui::save_browser::scan_saves(&dir, |p| {
                        hoi4_state::save::read_meta(p)
                            .ok()
                            .map(|m| (format!("{}", m.date), m.player_tag))
                    });
                    self.ui_state.save_browser.set_saves(entries);
                }
            }
        }

        if let Some(cmd) = end_cmd {
            use hoi4_ui::end_screen::EndCommand;
            match cmd {
                EndCommand::Continue => {
                    self.world.speed = GameSpeed::Speed3;
                }
                EndCommand::ReturnToMainMenu => {
                    self.view.game_phase = GamePhase::MainMenu;
                    self.view.menu_kind = Some(MenuKind::MainMenu);
                    self.view.menu_hovered_btn = None;
                    self.view.menu_hovered_row = None;
                    self.view.last_main_buttons.clear();
                    self.view.last_country_layout = None;
                }
                EndCommand::Quit => {
                    // ????????????????exit event_loop???????speed=Paused ??????????                    // ???????????? window_event ???????CloseRequested??                    std::process::exit(0);
                }
            }
        }

        UiCommandApplyOutput {
            topbar_action,
            side_rail_panel_cmd,
            open_panel_kind,
            panel_commands,
            deferred_switch_player_country,
            egui_current_stats,
        }
    }

    fn apply_deferred_ui_commands_inner(&mut self, mut output: UiCommandApplyOutput) {
        if let Some(action) = output.topbar_action {
            ui_binding::apply_topbar_action(self, action);
        }
        if let Some(kind) = output.side_rail_panel_cmd {
            let command = if output.open_panel_kind == Some(kind) {
                hoi4_ui::PanelCommand::ClosePrimary
            } else {
                hoi4_ui::PanelCommand::OpenPrimary(hoi4_ui::ActivePrimaryPanel::from_panel_kind(
                    kind,
                ))
            };
            output.panel_commands.push(command);
        }
        if !output.panel_commands.is_empty() {
            ui_binding::apply_panel_commands(self, output.panel_commands);
        }
        for tag in output.deferred_switch_player_country {
            self.set_player_country_by_tag(&tag);
        }
    }
}
