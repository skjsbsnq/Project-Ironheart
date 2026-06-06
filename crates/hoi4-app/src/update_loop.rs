use crate::App;

impl App {
    pub(super) fn update(&mut self, max_sim_budget_secs: f32) {
        self.update_simulation_tick(max_sim_budget_secs);
    }
}
