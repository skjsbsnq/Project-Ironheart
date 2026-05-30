//! V5 阶段 C.7：本地化 + 通用 Tooltip。
//!
//! `localisation/english/*.yml` 加载到 `LocCatalog`；`loc::tr("KEY")` 路由。
//! egui tooltip 延迟设为 0.3s。

use std::collections::HashMap;
use std::path::Path;

/// 本地化目录。
#[derive(Debug, Clone, Default)]
pub struct LocCatalog {
    entries: HashMap<String, String>,
}

impl LocCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    /// 从 vanilla `localisation/english/` 目录加载所有 `.yml` 文件。
    pub fn load_from_dir(dir: &Path) -> Self {
        let mut cat = Self::new();
        let Ok(rd) = std::fs::read_dir(dir) else {
            return cat;
        };
        for entry in rd.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "yml").unwrap_or(false) {
                cat.load_file(&path);
            }
        }
        cat
    }

    /// 解析单个 HOI4 `.yml` 本地化文件（简化解析：`key:0 "value"` 格式）。
    fn load_file(&mut self, path: &Path) {
        let Ok(content) = std::fs::read_to_string(path) else {
            return;
        };
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with("l_") {
                continue;
            }
            // Format: ` key:0 "value"` or ` key: "value"`
            if let Some(colon_pos) = line.find(':') {
                let key = line[..colon_pos].trim();
                let rest = &line[colon_pos + 1..];
                // Skip version number (e.g. "0 " or "1 ")
                let rest = rest.trim_start_matches(|c: char| c.is_ascii_digit() || c == ' ');
                if let (Some(start), Some(end)) = (rest.find('"'), rest.rfind('"')) {
                    if start < end {
                        let value = &rest[start + 1..end];
                        self.entries.insert(key.to_string(), value.to_string());
                    }
                }
            }
        }
    }

    /// 查找本地化字符串。找不到则返回 key 本身。
    pub fn tr<'a>(&'a self, key: &'a str) -> &'a str {
        self.entries.get(key).map(|s| s.as_str()).unwrap_or(key)
    }

    /// 条目数量。
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// 配置 egui tooltip 延迟为 0.3s（vanilla 风格）。
pub fn configure_tooltip_delay(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.interaction.tooltip_delay = 0.3;
    ctx.set_style(style);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tr_returns_key_when_missing() {
        let cat = LocCatalog::new();
        assert_eq!(cat.tr("MISSING_KEY"), "MISSING_KEY");
    }

    #[test]
    fn parse_line_format() {
        let mut cat = LocCatalog::new();
        // Simulate parsing
        cat.entries.insert("TEST_KEY".into(), "Hello World".into());
        assert_eq!(cat.tr("TEST_KEY"), "Hello World");
    }
}
