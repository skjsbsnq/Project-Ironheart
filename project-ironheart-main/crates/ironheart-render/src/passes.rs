#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderPassId {
    Shadow,
    Sky,
    Terrain,
    Water,
    Borders,
    StaticDecals,
    SemanticOverlays,
    WorldObjects,
    Counters,
    Labels,
    Particles,
    PostProcess,
    V9Ui,
    DebugOverlay,
}
