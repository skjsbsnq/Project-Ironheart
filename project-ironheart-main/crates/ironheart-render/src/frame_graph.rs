use crate::fallback::{FallbackPolicy, ResourceQuality};
use crate::passes::RenderPassId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderPhase {
    Shadow,
    WorldBase,
    WorldOverlay,
    WorldObjects,
    PostProcess,
    Ui,
    Debug,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderNode {
    pub pass: RenderPassId,
    pub phase: RenderPhase,
    pub fallback_policy: FallbackPolicy,
}

#[derive(Debug, Clone)]
pub struct FrameGraph {
    nodes: Vec<RenderNode>,
}

#[derive(Debug, Clone)]
pub struct FramePlan {
    pub nodes: Vec<RenderNode>,
    pub quality: ResourceQuality,
}

impl FrameGraph {
    pub fn project_default() -> Self {
        use FallbackPolicy::{DebugOnly, DegradedAllowed, Required};

        let nodes = vec![
            RenderNode {
                pass: RenderPassId::Shadow,
                phase: RenderPhase::Shadow,
                fallback_policy: DegradedAllowed,
            },
            RenderNode {
                pass: RenderPassId::Sky,
                phase: RenderPhase::WorldBase,
                fallback_policy: DegradedAllowed,
            },
            RenderNode {
                pass: RenderPassId::Terrain,
                phase: RenderPhase::WorldBase,
                fallback_policy: Required,
            },
            RenderNode {
                pass: RenderPassId::Water,
                phase: RenderPhase::WorldBase,
                fallback_policy: Required,
            },
            RenderNode {
                pass: RenderPassId::Borders,
                phase: RenderPhase::WorldOverlay,
                fallback_policy: Required,
            },
            RenderNode {
                pass: RenderPassId::StaticDecals,
                phase: RenderPhase::WorldOverlay,
                fallback_policy: DegradedAllowed,
            },
            RenderNode {
                pass: RenderPassId::SemanticOverlays,
                phase: RenderPhase::WorldOverlay,
                fallback_policy: Required,
            },
            RenderNode {
                pass: RenderPassId::WorldObjects,
                phase: RenderPhase::WorldObjects,
                fallback_policy: DegradedAllowed,
            },
            RenderNode {
                pass: RenderPassId::Counters,
                phase: RenderPhase::WorldObjects,
                fallback_policy: Required,
            },
            RenderNode {
                pass: RenderPassId::Labels,
                phase: RenderPhase::WorldObjects,
                fallback_policy: DegradedAllowed,
            },
            RenderNode {
                pass: RenderPassId::Particles,
                phase: RenderPhase::WorldObjects,
                fallback_policy: DegradedAllowed,
            },
            RenderNode {
                pass: RenderPassId::PostProcess,
                phase: RenderPhase::PostProcess,
                fallback_policy: Required,
            },
            RenderNode {
                pass: RenderPassId::V9Ui,
                phase: RenderPhase::Ui,
                fallback_policy: Required,
            },
            RenderNode {
                pass: RenderPassId::DebugOverlay,
                phase: RenderPhase::Debug,
                fallback_policy: DebugOnly,
            },
        ];

        Self { nodes }
    }

    pub fn plan(&self, zoom: f32, quality: ResourceQuality) -> FramePlan {
        let strategic_zoom = zoom < 0.45;
        let nodes = self
            .nodes
            .iter()
            .copied()
            .filter(|node| quality.allows(node.fallback_policy))
            .filter(|node| !(strategic_zoom && node.pass == RenderPassId::Particles))
            .collect();

        FramePlan { nodes, quality }
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }
}
