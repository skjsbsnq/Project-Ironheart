//! V9 motion helpers.

use egui::{Context, Id, Rect, Vec2};

use crate::v9::tokens::motion;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionCurve {
    Linear,
    EaseOut,
    EaseInOut,
    EaseBack,
}

#[derive(Debug, Clone, Copy)]
pub struct MotionSpec {
    pub duration: f32,
    pub curve: MotionCurve,
}

impl MotionSpec {
    pub const fn new(duration: f32, curve: MotionCurve) -> Self {
        Self { duration, curve }
    }
}

pub const HOVER: MotionSpec = MotionSpec::new(motion::FAST, MotionCurve::EaseOut);
pub const TAB_SWITCH: MotionSpec = MotionSpec::new(motion::NORMAL, MotionCurve::EaseInOut);
pub const MODAL_ENTER: MotionSpec = MotionSpec::new(motion::SLOW, MotionCurve::EaseBack);
pub const TOAST_SLIDE: MotionSpec = MotionSpec::new(motion::SLOW, MotionCurve::EaseBack);

pub fn animate_bool(ctx: &Context, id: Id, target: bool, spec: MotionSpec) -> f32 {
    ctx.animate_bool_with_time_and_easing(id, target, spec.duration.max(0.001), spec.curve.ease())
}

pub fn animate_value(ctx: &Context, id: Id, target: f32, spec: MotionSpec) -> f32 {
    let value = ctx.animate_value_with_time(id, target, spec.duration.max(0.001));
    spec.curve.apply(value.clamp(0.0, 1.0))
}

pub fn modal_enter_rect(ctx: &Context, id: Id, rect: Rect) -> Rect {
    let t = animate_value(ctx, id.with("enter"), 1.0, MODAL_ENTER);
    let offset = (1.0 - t) * 18.0;
    rect.translate(Vec2::new(0.0, offset))
}

pub fn toast_slide_rect(ctx: &Context, id: Id, rect: Rect) -> Rect {
    let t = animate_value(ctx, id.with("slide"), 1.0, TOAST_SLIDE);
    let offset = (1.0 - t) * 16.0;
    rect.translate(Vec2::new(offset, 0.0))
}

impl MotionCurve {
    pub fn apply(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Self::Linear => t,
            Self::EaseOut => 1.0 - (1.0 - t) * (1.0 - t),
            Self::EaseInOut => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(2) * 0.5
                }
            }
            Self::EaseBack => {
                let c1 = 1.70158;
                let c3 = c1 + 1.0;
                1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
            }
        }
    }

    fn ease(self) -> fn(f32) -> f32 {
        match self {
            Self::Linear => linear,
            Self::EaseOut => ease_out,
            Self::EaseInOut => ease_in_out,
            Self::EaseBack => ease_back,
        }
    }
}

fn linear(t: f32) -> f32 {
    MotionCurve::Linear.apply(t)
}

fn ease_out(t: f32) -> f32 {
    MotionCurve::EaseOut.apply(t)
}

fn ease_in_out(t: f32) -> f32 {
    MotionCurve::EaseInOut.apply(t)
}

fn ease_back(t: f32) -> f32 {
    MotionCurve::EaseBack.apply(t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curves_start_and_end_at_expected_values() {
        for curve in [
            MotionCurve::Linear,
            MotionCurve::EaseOut,
            MotionCurve::EaseInOut,
            MotionCurve::EaseBack,
        ] {
            assert!((curve.apply(0.0) - 0.0).abs() < 0.001);
            assert!((curve.apply(1.0) - 1.0).abs() < 0.001);
        }
    }

    #[test]
    fn phase_h_specs_use_token_durations() {
        assert_eq!(HOVER.duration, motion::FAST);
        assert_eq!(TAB_SWITCH.duration, motion::NORMAL);
        assert_eq!(MODAL_ENTER.duration, motion::SLOW);
        assert_eq!(TOAST_SLIDE.duration, motion::SLOW);
    }
}
