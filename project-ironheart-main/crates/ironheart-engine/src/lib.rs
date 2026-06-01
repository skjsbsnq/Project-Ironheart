use ironheart_render::{Renderer, RendererFrameInput};
use ironheart_sim::SimRuntime;
use ironheart_ui::UiRuntime;

pub struct ProjectIronheart {
    sim: SimRuntime,
    renderer: Renderer,
    ui: UiRuntime,
    frame_index: u64,
}

impl ProjectIronheart {
    pub fn new() -> Self {
        Self {
            sim: SimRuntime::new(),
            renderer: Renderer::new(),
            ui: UiRuntime::new(),
            frame_index: 0,
        }
    }

    pub fn tick(&mut self) {
        self.sim.tick_hour();
        let snapshot = self.sim.snapshot();
        let frame_input = RendererFrameInput {
            frame_index: self.frame_index,
            zoom: snapshot.zoom,
            map_mode: snapshot.map_mode.clone(),
            date_label: snapshot.date_label.clone(),
        };
        let _plan = self.renderer.plan_frame(&frame_input);
        let _ui_model = self.ui.build_frame(snapshot);
        self.frame_index += 1;
    }

    pub fn startup_banner(&self) -> String {
        format!(
            "Project Ironheart Main rewrite booted: frame={}, graph_nodes={}",
            self.frame_index,
            self.renderer.graph_node_count()
        )
    }
}

impl Default for ProjectIronheart {
    fn default() -> Self {
        Self::new()
    }
}
