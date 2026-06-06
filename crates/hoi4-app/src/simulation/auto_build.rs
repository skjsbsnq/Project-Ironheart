use crate::*;

impl App {
    pub(crate) fn run_monthly_auto_build_if_due(&mut self) {
        let auto_build_month = (self.world.date.year, self.world.date.month);
        if !self.runtime.auto_build_enabled
            || self.world.date.day != 1
            || self.runtime.last_auto_build_month == Some(auto_build_month)
        {
            return;
        }

        let added = Self::auto_enqueue_player_construction(
            &self.world,
            &mut self.runtime.econ,
            &self.v6_db,
            self.view.player_country,
        );
        self.runtime.last_auto_build_month = Some(auto_build_month);
        self.runtime.last_auto_build_explanations = added;
        if !self.runtime.last_auto_build_explanations.is_empty() {
            println!(
                "[construction] monthly auto-build queued {} projects",
                self.runtime.last_auto_build_explanations.len()
            );
        }
    }
}
