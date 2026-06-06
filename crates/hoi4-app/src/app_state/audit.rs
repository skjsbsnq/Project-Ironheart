use crate::edge_pan_test::{EdgePanTestConfig, EdgePanTestRun};
use crate::map_phase0_run::MapPhase0Run;

#[derive(Default)]
pub(crate) struct AuditState {
    pub(crate) map_phase0: Option<MapPhase0Run>,
    pub(crate) edge_pan_test: Option<EdgePanTestRun>,
}

impl AuditState {
    pub(crate) fn with_edge_pan_test(edge_pan_test: Option<EdgePanTestConfig>) -> Self {
        Self {
            map_phase0: None,
            edge_pan_test: edge_pan_test.map(EdgePanTestRun::new),
        }
    }
}
