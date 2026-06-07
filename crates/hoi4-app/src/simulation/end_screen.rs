use crate::*;

impl App {
    pub(crate) fn handle_campaign_end_screen(&mut self) {
        if !self.ui_state.end_screen.triggered
            && !self.ui_state.end_screen.continued
            && self.view.game_phase == GamePhase::Playing
            && hoi4_ui::end_screen::EndScreen::should_trigger(
                self.world.date.year,
                self.world.date.month,
                self.world.date.day,
            )
        {
            let player = self.view.player_country;
            let player_cid = hoi4_state::CountryId(player as u16);
            let player_tag = self
                .world
                .countries
                .tags
                .get(player)
                .cloned()
                .unwrap_or_default();
            let divisions = (0..self.world.divisions.count)
                .filter(|&i| self.world.divisions.owners[i] == player_cid)
                .count() as u32;
            let provinces_occupied = (0..self.world.provinces.count)
                .filter(|&i| {
                    self.world.provinces.controllers[i] == player_cid
                        && self.world.provinces.owners[i] != player_cid
                })
                .count() as u32;
            let focuses_completed = self.world.countries.completed_focuses[player].len() as u32;
            let (civ, mil, dock) = Self::v6_industry_counts(&self.world, player_cid);
            let techs = self.world.countries.completed_techs[player].len() as u32;
            let faction_info = self
                .world
                .diplomacy
                .factions
                .iter()
                .find(|f| f.contains(player_cid))
                .map(|f| {
                    (
                        f.name.clone(),
                        f.members
                            .iter()
                            .map(|m| {
                                self.world
                                    .countries
                                    .tags
                                    .get(m.0 as usize)
                                    .cloned()
                                    .unwrap_or_default()
                            })
                            .collect::<Vec<_>>(),
                    )
                });
            let at_war_with: Vec<String> = self
                .world
                .diplomacy
                .wars
                .values()
                .filter(|w| w.attackers.contains(&player_cid) || w.defenders.contains(&player_cid))
                .flat_map(|w| {
                    let enemies = if w.attackers.contains(&player_cid) {
                        &w.defenders
                    } else {
                        &w.attackers
                    };
                    enemies.iter().map(|c| {
                        self.world
                            .countries
                            .tags
                            .get(c.0 as usize)
                            .cloned()
                            .unwrap_or_default()
                    })
                })
                .collect();
            let stats = hoi4_ui::end_screen::EndStats {
                player_tag,
                player_name: self
                    .world
                    .countries
                    .tags
                    .get(player)
                    .cloned()
                    .unwrap_or_default(),
                date_str: format!("{}", self.world.date),
                focuses_completed,
                civ_factories: civ,
                mil_factories: mil,
                shipyards: dock,
                divisions,
                provinces_occupied,
                at_war_with,
                faction_name: faction_info.as_ref().map(|(n, _)| n.clone()),
                faction_members: faction_info.map(|(_, m)| m).unwrap_or_default(),
                techs_researched: techs,
                world_tension: self.world.diplomacy.world_tension,
            };
            self.ui_state.end_screen.trigger(stats);
            self.world.speed = hoi4_state::GameSpeed::Paused;
            println!("[game] campaign end reached: 1940-12-31");
        }
    }
}
