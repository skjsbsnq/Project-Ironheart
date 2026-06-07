use std::collections::HashMap;

use clausewitz_parser::Block;

use super::ast::{GuiNode, GuiValueExt};
use super::layout::GuiPoint;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationCurve {
    Linear,
    Decelerated,
    Accelerated,
}

impl AnimationCurve {
    pub fn parse(value: Option<String>) -> Self {
        match value.as_deref() {
            Some("decelerated") => Self::Decelerated,
            Some("accelerated") => Self::Accelerated,
            _ => Self::Linear,
        }
    }

    fn apply(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Self::Linear => t,
            Self::Decelerated => 1.0 - (1.0 - t) * (1.0 - t),
            Self::Accelerated => t * t,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnimationSpec {
    pub hidden_position: GuiPoint,
    pub shown_position: GuiPoint,
    pub show_curve: AnimationCurve,
    pub hide_curve: AnimationCurve,
    pub duration_ms: f32,
}

impl AnimationSpec {
    pub fn from_node(node: &GuiNode) -> Self {
        let hidden_position = point_from_block(node.block("position"));
        let shown_position = node
            .block("show_position")
            .map(|_| point_from_block(node.block("show_position")))
            .unwrap_or(hidden_position);
        Self {
            hidden_position,
            shown_position,
            show_curve: AnimationCurve::parse(node.string("show_animation_type")),
            hide_curve: AnimationCurve::parse(node.string("hide_animation_type")),
            duration_ms: node.f32("animation_time").unwrap_or(0.0).max(0.0),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationPhase {
    Closed,
    Opening,
    Open,
    Closing,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PanelAnimationUpdate {
    pub position: GuiPoint,
    pub phase: AnimationPhase,
    pub visible: bool,
    pub close_finished: bool,
}

#[derive(Debug, Clone, Copy)]
struct PanelAnimationState {
    phase: AnimationPhase,
    start_ms: f64,
}

#[derive(Debug, Default, Clone)]
pub struct PanelAnimationStore {
    states: HashMap<String, PanelAnimationState>,
}

impl PanelAnimationStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update(
        &mut self,
        panel_id: impl Into<String>,
        wants_open: bool,
        spec: AnimationSpec,
        now_ms: f64,
    ) -> PanelAnimationUpdate {
        let panel_id = panel_id.into();
        let state = self.states.entry(panel_id).or_insert(PanelAnimationState {
            phase: if wants_open {
                AnimationPhase::Opening
            } else {
                AnimationPhase::Closed
            },
            start_ms: now_ms,
        });

        match (wants_open, state.phase) {
            (true, AnimationPhase::Closed | AnimationPhase::Closing) => {
                state.phase = AnimationPhase::Opening;
                state.start_ms = now_ms;
            }
            (false, AnimationPhase::Open | AnimationPhase::Opening) => {
                state.phase = AnimationPhase::Closing;
                state.start_ms = now_ms;
            }
            _ => {}
        }

        let elapsed = (now_ms - state.start_ms).max(0.0) as f32;
        let duration = spec.duration_ms.max(0.001);
        let raw_t = (elapsed / duration).clamp(0.0, 1.0);

        match state.phase {
            AnimationPhase::Closed => PanelAnimationUpdate {
                position: spec.hidden_position,
                phase: AnimationPhase::Closed,
                visible: false,
                close_finished: false,
            },
            AnimationPhase::Open => PanelAnimationUpdate {
                position: spec.shown_position,
                phase: AnimationPhase::Open,
                visible: true,
                close_finished: false,
            },
            AnimationPhase::Opening => {
                if raw_t >= 1.0 || spec.duration_ms <= 0.0 {
                    state.phase = AnimationPhase::Open;
                    return PanelAnimationUpdate {
                        position: spec.shown_position,
                        phase: AnimationPhase::Open,
                        visible: true,
                        close_finished: false,
                    };
                }
                let t = spec.show_curve.apply(raw_t);
                PanelAnimationUpdate {
                    position: lerp_point(spec.hidden_position, spec.shown_position, t),
                    phase: AnimationPhase::Opening,
                    visible: true,
                    close_finished: false,
                }
            }
            AnimationPhase::Closing => {
                if raw_t >= 1.0 || spec.duration_ms <= 0.0 {
                    state.phase = AnimationPhase::Closed;
                    return PanelAnimationUpdate {
                        position: spec.hidden_position,
                        phase: AnimationPhase::Closed,
                        visible: false,
                        close_finished: true,
                    };
                }
                let t = spec.hide_curve.apply(raw_t);
                PanelAnimationUpdate {
                    position: lerp_point(spec.shown_position, spec.hidden_position, t),
                    phase: AnimationPhase::Closing,
                    visible: true,
                    close_finished: false,
                }
            }
        }
    }

    pub fn phase(&self, panel_id: &str) -> Option<AnimationPhase> {
        self.states.get(panel_id).map(|state| state.phase)
    }
}

fn point_from_block(block: Option<&Block>) -> GuiPoint {
    let Some(block) = block else {
        return GuiPoint::default();
    };
    GuiPoint {
        x: block
            .get("x")
            .and_then(GuiValueExt::as_lossy_f32)
            .unwrap_or(0.0),
        y: block
            .get("y")
            .and_then(GuiValueExt::as_lossy_f32)
            .unwrap_or(0.0),
    }
}

fn lerp_point(from: GuiPoint, to: GuiPoint, t: f32) -> GuiPoint {
    GuiPoint {
        x: from.x + (to.x - from.x) * t,
        y: from.y + (to.y - from.y) * t,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vanilla_gui::ast::parse_gui_str;

    #[test]
    fn spec_reads_politics_positions() {
        let doc = parse_gui_str(
            None,
            r#"
guiTypes = {
    containerWindowType = {
        name = "countrypoliticsview"
        position = { x=-606 y=78 }
        show_position = { x=-6 y=78 }
        show_animation_type = decelerated
        hide_animation_type = accelerated
        animation_time = 300
    }
}
"#,
        );
        let node = doc.template_index().get("countrypoliticsview").unwrap();
        let spec = AnimationSpec::from_node(node);
        assert_eq!(spec.hidden_position.x, -606.0);
        assert_eq!(spec.shown_position.x, -6.0);
        assert_eq!(spec.show_curve, AnimationCurve::Decelerated);
        assert_eq!(spec.hide_curve, AnimationCurve::Accelerated);
        assert_eq!(spec.duration_ms, 300.0);
    }

    #[test]
    fn close_finishes_after_slide_out() {
        let spec = AnimationSpec {
            hidden_position: GuiPoint { x: -606.0, y: 78.0 },
            shown_position: GuiPoint { x: -6.0, y: 78.0 },
            show_curve: AnimationCurve::Decelerated,
            hide_curve: AnimationCurve::Accelerated,
            duration_ms: 300.0,
        };
        let mut store = PanelAnimationStore::new();
        let opening = store.update("politics", true, spec, 0.0);
        assert_eq!(opening.phase, AnimationPhase::Opening);
        let open = store.update("politics", true, spec, 400.0);
        assert_eq!(open.phase, AnimationPhase::Open);
        let closing = store.update("politics", false, spec, 450.0);
        assert_eq!(closing.phase, AnimationPhase::Closing);
        assert!(closing.visible);
        let closed = store.update("politics", false, spec, 800.0);
        assert!(closed.close_finished);
        assert!(!closed.visible);
        assert_eq!(store.phase("politics"), Some(AnimationPhase::Closed));
    }
}
