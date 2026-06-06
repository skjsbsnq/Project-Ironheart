use crate::*;

impl App {
    pub(crate) fn handle_new_war_auto_pause(&mut self) {
        // E.2: Auto-pause on new war involving player.
        let cur_wars = self.world.diplomacy.wars.len();
        if cur_wars > self.runtime.last_war_count {
            let player = hoi4_state::CountryId(self.view.player_country as u16);
            if self.world.diplomacy.is_at_war(player) {
                self.world.speed = hoi4_state::GameSpeed::Paused;
            }
        }
        self.runtime.last_war_count = cur_wars;


    }
}
