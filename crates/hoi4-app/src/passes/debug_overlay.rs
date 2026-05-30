//! Phase 3.12.1 — F4 调试覆盖（pass list + 状态）。
//!
//! 单职责：维护 F4 toggle 与一段每帧重新生成的"调试文本"。具体绘制由现有
//! `text_pass` 在主 render 内消费 `latest_lines()` 输出。
//!
//! 后续 3.12.X 接入 `wgpu::QuerySet` GPU timestamp 后，把每个 pass 的耗时填到
//! `PassRegistry.entries[i].gpu_ms`，本模块自动一起渲染。

use crate::passes::{PassEntry, PassRegistry};

/// F4 调试覆盖。
#[derive(Debug, Default)]
pub struct DebugOverlay {
    pub enabled: bool,
    /// 上一帧准备好的每行文本。
    lines: Vec<String>,
}

impl DebugOverlay {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn toggle(&mut self) {
        self.enabled = !self.enabled;
    }

    /// 每帧调用：根据 [`PassRegistry`] 重新生成 4-6 行摘要。
    pub fn refresh(&mut self, registry: &PassRegistry, hdr_w: u32, hdr_h: u32, mode: &str) {
        self.lines.clear();
        if !self.enabled {
            return;
        }
        self.lines.push(format!(
            "=== F4 Render Debug — HDR {}x{} mode={} ===",
            hdr_w, hdr_h, mode
        ));
        for (i, e) in registry.entries.iter().enumerate() {
            self.lines.push(format_entry(i, e));
        }
        if registry.entries.is_empty() {
            self.lines.push("  (no passes registered)".to_string());
        }
        self.lines.push("(press F4 again to hide)".to_string());
    }

    pub fn append_lines<I>(&mut self, lines: I)
    where
        I: IntoIterator<Item = String>,
    {
        if self.enabled {
            self.lines.extend(lines);
        }
    }

    /// 给绘制器消费的当前行（启用时为空表示不绘制）。
    pub fn latest_lines(&self) -> &[String] {
        &self.lines
    }
}

fn format_entry(idx: usize, e: &PassEntry) -> String {
    let on = if e.enabled { "ON " } else { "OFF" };
    if e.cpu_ms > 0.0 || e.gpu_ms > 0.0 || e.draw_calls > 0 {
        format!(
            "  [{:>2}] {} {:<24} cpu={:>5.2} gpu={:>5.2} draw={}",
            idx, on, e.name, e.cpu_ms, e.gpu_ms, e.draw_calls
        )
    } else {
        format!("  [{:>2}] {} {}", idx, on, e.name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refresh_emits_lines_when_enabled() {
        let mut reg = PassRegistry::new();
        reg.register("terrain");
        reg.register("postprocess");

        let mut ov = DebugOverlay::new();
        ov.refresh(&reg, 1920, 1080, "simple_blit");
        assert!(
            ov.latest_lines().is_empty(),
            "disabled overlay yields nothing"
        );

        ov.toggle();
        ov.refresh(&reg, 1920, 1080, "simple_blit");
        let lines = ov.latest_lines();
        assert!(lines.iter().any(|l| l.contains("HDR 1920x1080")));
        assert!(lines.iter().any(|l| l.contains("terrain")));
        assert!(lines.iter().any(|l| l.contains("postprocess")));
    }

    #[test]
    fn refresh_includes_phase10_stats_when_recorded() {
        let mut reg = PassRegistry::new();
        reg.register("terrain");
        reg.record_cpu_ms("terrain", 1.0);
        reg.record_gpu_ms("terrain", 2.0);
        reg.record_draw_calls("terrain", 3);

        let mut ov = DebugOverlay::new();
        ov.toggle();
        ov.refresh(&reg, 1920, 1080, "simple_blit");
        assert!(ov
            .latest_lines()
            .iter()
            .any(|line| line.contains("cpu=") && line.contains("draw=3")));
    }
}
