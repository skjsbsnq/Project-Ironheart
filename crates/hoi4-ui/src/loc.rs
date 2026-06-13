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
        cat.merge_from_dir(dir);
        cat
    }

    /// Load and merge all `.yml` files from multiple localization directories.
    ///
    /// Later directories override earlier directories. Callers that receive a
    /// high-priority-first path chain should reverse it before calling this.
    pub fn load_from_dirs<I, P>(dirs: I) -> Self
    where
        I: IntoIterator<Item = P>,
        P: AsRef<Path>,
    {
        let mut cat = Self::new();
        for dir in dirs {
            cat.merge_from_dir(dir.as_ref());
        }
        cat
    }

    /// Add entries from `fallback` only when this catalog does not already have
    /// the key.
    pub fn fill_missing_from(&mut self, fallback: &LocCatalog) {
        for (key, value) in &fallback.entries {
            self.entries
                .entry(key.clone())
                .or_insert_with(|| value.clone());
        }
    }

    fn merge_from_dir(&mut self, dir: &Path) {
        let Ok(rd) = std::fs::read_dir(dir) else {
            return;
        };
        let mut files: Vec<_> = rd
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.extension().map(|e| e == "yml").unwrap_or(false))
            .collect();
        files.sort();
        for path in files {
            self.load_file(&path);
        }
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

    #[test]
    fn load_from_dirs_applies_later_directory_overrides() {
        let root = std::env::temp_dir().join(format!(
            "ironheart_loc_dirs_{}_override",
            std::process::id()
        ));
        let low = root.join("low");
        let high = root.join("high");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&low).unwrap();
        std::fs::create_dir_all(&high).unwrap();
        std::fs::write(
            low.join("state_names_l_english.yml"),
            "l_english:\n STATE_1:0 \"Low Name\"\n STATE_2:0 \"Only Low\"\n",
        )
        .unwrap();
        std::fs::write(
            high.join("state_names_l_english.yml"),
            "l_english:\n STATE_1:0 \"High Name\"\n",
        )
        .unwrap();

        let cat = LocCatalog::load_from_dirs([low.as_path(), high.as_path()]);

        assert_eq!(cat.tr("STATE_1"), "High Name");
        assert_eq!(cat.tr("STATE_2"), "Only Low");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn fill_missing_from_keeps_existing_language_entries() {
        let mut selected = LocCatalog::new();
        selected
            .entries
            .insert("STATE_1".into(), "Selected Name".into());
        let mut fallback = LocCatalog::new();
        fallback.entries.insert("STATE_1".into(), "Fallback Name".into());
        fallback.entries.insert("STATE_2".into(), "Fallback Only".into());

        selected.fill_missing_from(&fallback);

        assert_eq!(selected.tr("STATE_1"), "Selected Name");
        assert_eq!(selected.tr("STATE_2"), "Fallback Only");
    }
}
