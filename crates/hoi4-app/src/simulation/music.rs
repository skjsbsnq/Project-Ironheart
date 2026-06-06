use crate::*;

impl App {
    pub(crate) fn update_music_autoadvance(&mut self) {
        // Auto-advance music when the current track finishes.
        self.music_player.tick_autoadvance();


    }
}
