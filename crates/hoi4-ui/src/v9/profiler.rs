//! V9 frame-budget profiler helpers.

use std::time::Instant;

use egui::{Context, Id};

use crate::FRAME_BUDGET_US;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct V9Budget {
    pub frame_budget_us: u32,
    pub primitive_budget_us: u32,
}

impl Default for V9Budget {
    fn default() -> Self {
        Self {
            frame_budget_us: FRAME_BUDGET_US,
            primitive_budget_us: 250,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct V9PrimitiveSample {
    pub name: &'static str,
    pub elapsed_us: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct V9FrameProfile {
    pub frame_us: u32,
    pub samples: Vec<V9PrimitiveSample>,
    pub budget: V9Budget,
}

impl V9FrameProfile {
    pub fn within_frame_budget(&self) -> bool {
        self.frame_us <= self.budget.frame_budget_us
    }

    pub fn slow_primitives(&self) -> Vec<&V9PrimitiveSample> {
        self.samples
            .iter()
            .filter(|sample| sample.elapsed_us > self.budget.primitive_budget_us)
            .collect()
    }

    pub fn report(&self) -> String {
        let status = if self.within_frame_budget() {
            "OK"
        } else {
            "OVER"
        };
        let mut out = format!(
            "v9={}us budget={}us status={}",
            self.frame_us, self.budget.frame_budget_us, status
        );
        let slow = self.slow_primitives();
        if !slow.is_empty() {
            out.push_str(" slow=");
            for (idx, sample) in slow.iter().enumerate() {
                if idx > 0 {
                    out.push(',');
                }
                out.push_str(sample.name);
            }
        }
        out
    }
}

pub struct V9FrameProfiler {
    started: Instant,
    budget: V9Budget,
    samples: Vec<V9PrimitiveSample>,
}

#[derive(Debug, Clone)]
struct V9ProfilerState {
    started: Instant,
    budget: V9Budget,
    samples: Vec<V9PrimitiveSample>,
}

impl V9FrameProfiler {
    pub fn begin() -> Self {
        Self::with_budget(V9Budget::default())
    }

    pub fn with_budget(budget: V9Budget) -> Self {
        Self {
            started: Instant::now(),
            budget,
            samples: Vec::new(),
        }
    }

    pub fn measure<R>(&mut self, name: &'static str, f: impl FnOnce() -> R) -> R {
        let started = Instant::now();
        let out = f();
        self.samples.push(V9PrimitiveSample {
            name,
            elapsed_us: started.elapsed().as_micros() as u32,
        });
        out
    }

    pub fn finish(self) -> V9FrameProfile {
        V9FrameProfile {
            frame_us: self.started.elapsed().as_micros() as u32,
            samples: self.samples,
            budget: self.budget,
        }
    }
}

pub fn profile_frame<R>(f: impl FnOnce(&mut V9FrameProfiler) -> R) -> (R, V9FrameProfile) {
    let mut profiler = V9FrameProfiler::begin();
    let out = f(&mut profiler);
    let profile = profiler.finish();
    (out, profile)
}

pub fn begin_frame_ctx(ctx: &Context) {
    ctx.data_mut(|data| {
        data.insert_temp(
            profiler_state_id(),
            V9ProfilerState {
                started: Instant::now(),
                budget: V9Budget::default(),
                samples: Vec::new(),
            },
        )
    });
}

pub fn measure_ctx<R>(ctx: &Context, name: &'static str, f: impl FnOnce() -> R) -> R {
    let started = Instant::now();
    let out = f();
    let elapsed_us = started.elapsed().as_micros() as u32;
    ctx.data_mut(|data| {
        if let Some(mut state) = data.get_temp::<V9ProfilerState>(profiler_state_id()) {
            state.samples.push(V9PrimitiveSample { name, elapsed_us });
            data.insert_temp(profiler_state_id(), state);
        }
    });
    out
}

pub fn finish_frame_ctx(ctx: &Context) -> Option<V9FrameProfile> {
    ctx.data_mut(|data| {
        data.get_temp::<V9ProfilerState>(profiler_state_id())
            .map(|state| V9FrameProfile {
                frame_us: state.started.elapsed().as_micros() as u32,
                samples: state.samples,
                budget: state.budget,
            })
    })
}

fn profiler_state_id() -> Id {
    Id::new("v9_profiler_state")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_budget_uses_existing_ui_budget() {
        assert_eq!(V9Budget::default().frame_budget_us, FRAME_BUDGET_US);
    }

    #[test]
    fn slow_primitive_detection_uses_primitive_budget() {
        let profile = V9FrameProfile {
            frame_us: 100,
            samples: vec![
                V9PrimitiveSample {
                    name: "button",
                    elapsed_us: 10,
                },
                V9PrimitiveSample {
                    name: "table",
                    elapsed_us: 300,
                },
            ],
            budget: V9Budget::default(),
        };
        let slow = profile.slow_primitives();
        assert_eq!(slow.len(), 1);
        assert_eq!(slow[0].name, "table");
    }
}
