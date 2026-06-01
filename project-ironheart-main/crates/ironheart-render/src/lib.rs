pub mod fallback;
pub mod frame_graph;
pub mod passes;

use fallback::ResourceQuality;
use frame_graph::{FrameGraph, FramePlan};

pub struct Renderer {
    graph: FrameGraph,
}

#[derive(Debug, Clone)]
pub struct RendererFrameInput {
    pub frame_index: u64,
    pub zoom: f32,
    pub map_mode: String,
    pub date_label: String,
}

impl Renderer {
    pub fn new() -> Self {
        Self {
            graph: FrameGraph::project_default(),
        }
    }

    pub fn plan_frame(&self, input: &RendererFrameInput) -> FramePlan {
        self.graph
            .plan(input.zoom, ResourceQuality::ProductionValid)
    }

    pub fn graph_node_count(&self) -> usize {
        self.graph.node_count()
    }
}

impl Default for Renderer {
    fn default() -> Self {
        Self::new()
    }
}
