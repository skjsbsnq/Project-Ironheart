//! V5 阶段 G.4：存档浏览器（list / load / delete / rename）。
//!
//! ## 设计
//!
//! 与 [`crate::settings`] 一致的「数据 + UI 解耦」架构：
//!
//! - [`SaveEntry`] 是纯数据：path / display_name / 修改时间 / 文件大小 / 元数据预览
//! - [`SaveBrowser`] 是 egui UI 状态：当前选中 / 重命名输入 buffer / 错误回显
//! - [`SaveBrowser::show`] 返回 [`SaveCommand`] 列表，caller（main.rs）执行真正的
//!   IO 副作用（删除 / 重命名 / 触发 load 流程）。
//!
//! 选这一架构是因为加载存档需要 [`hoi4_state::World`] 实例，写到 hoi4-ui 里
//! 会破坏 thin UI 性质。caller 拿到 `Load(path)` 命令后自行调
//! `hoi4_state::save::read(path, &mut world)`。

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::egui;
use crate::i18n::tr;
use crate::vanilla_iron::{UtilityWindowShell, VanillaIron};

/// G.4：存档目录路径。Windows 默认 `%USERPROFILE%/Documents/Ironheart/saves/`，
/// 其它系统 `~/Documents/Ironheart/saves/`。如果家目录无法解析，回退到当前目录的
/// `./saves/`。
pub fn default_saves_dir() -> PathBuf {
    let home = if cfg!(target_os = "windows") {
        std::env::var_os("USERPROFILE").map(PathBuf::from)
    } else {
        std::env::var_os("HOME").map(PathBuf::from)
    };
    home.map(|h| h.join("Documents").join("Ironheart").join("saves"))
        .unwrap_or_else(|| PathBuf::from("saves"))
}

/// G.4：单个存档条目。
#[derive(Debug, Clone)]
pub struct SaveEntry {
    pub path: PathBuf,
    pub display_name: String,
    pub size_bytes: u64,
    pub modified: Option<SystemTime>,
    /// 简单元数据（解析自存档头）。可能为 `None`（解析失败 / 未尝试）。
    pub date_str: Option<String>,
    pub player_tag: Option<String>,
}

impl SaveEntry {
    /// 修改时间相对当前的近似描述。
    pub fn modified_str(&self) -> String {
        let Some(t) = self.modified else {
            return "未知".into();
        };
        let dur = SystemTime::now().duration_since(t).unwrap_or_default();
        let secs = dur.as_secs();
        if secs < 60 {
            format!("{}秒前", secs)
        } else if secs < 3600 {
            format!("{}分钟前", secs / 60)
        } else if secs < 86_400 {
            format!("{}小时前", secs / 3600)
        } else {
            format!("{}天前", secs / 86_400)
        }
    }

    /// G.4：人类可读的尺寸（KB / MB）。
    pub fn size_str(&self) -> String {
        if self.size_bytes >= 1_048_576 {
            format!("{:.1} MB", self.size_bytes as f64 / 1_048_576.0)
        } else if self.size_bytes >= 1024 {
            format!("{:.1} KB", self.size_bytes as f64 / 1024.0)
        } else {
            format!("{} B", self.size_bytes)
        }
    }
}

/// G.4：扫描 saves 目录，返回所有 `.bin` / `.txt` / `.hoi4` 存档。
///
/// `meta_reader` 接收存档路径，返回可选的 `(date_str, player_tag)`，避免本 crate
/// 直接依赖 hoi4-state::save 的 read_meta（main.rs 在 caller 侧注入）。
pub fn scan_saves(
    saves_dir: &Path,
    mut meta_reader: impl FnMut(&Path) -> Option<(String, String)>,
) -> Vec<SaveEntry> {
    let Ok(entries) = std::fs::read_dir(saves_dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for ent in entries.flatten() {
        let path = ent.path();
        let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
            continue;
        };
        let lower = ext.to_ascii_lowercase();
        if lower != "bin" && lower != "txt" && lower != "hoi4" && lower != "save" {
            continue;
        }
        let meta = ent.metadata().ok();
        let size_bytes = meta.as_ref().map(|m| m.len()).unwrap_or(0);
        let modified = meta.and_then(|m| m.modified().ok());
        let display_name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("save")
            .to_owned();
        let (date_str, player_tag) = meta_reader(&path)
            .map(|(d, t)| (Some(d), Some(t)))
            .unwrap_or((None, None));
        out.push(SaveEntry {
            path,
            display_name,
            size_bytes,
            modified,
            date_str,
            player_tag,
        });
    }
    // 按修改时间倒序（最新的在前）。
    out.sort_by(|a, b| b.modified.cmp(&a.modified));
    out
}

/// G.4：UI 副作用命令。caller 执行实际 IO。
#[derive(Debug, Clone, PartialEq)]
pub enum SaveCommand {
    /// 加载某个存档。caller 调 `hoi4_state::save::read(path, &mut world)`。
    Load(PathBuf),
    /// 删除某个存档（已通过 confirm）。
    Delete(PathBuf),
    /// 重命名（保持扩展名）：`from → to`，caller 调 `std::fs::rename`。
    Rename { from: PathBuf, to: PathBuf },
    /// 重新扫描目录。
    Rescan,
}

/// G.4：存档浏览器 UI 状态。
pub struct SaveBrowser {
    pub open: bool,
    pub saves: Vec<SaveEntry>,
    /// 当前选中索引。
    pub selected: usize,
    /// 重命名输入 buffer（仅当 `renaming = true` 时编辑中）。
    pub rename_buffer: String,
    /// 是否处于「重命名」模式。
    pub renaming: bool,
    /// 是否处于「确认删除」模式。
    pub confirm_delete: bool,
    /// 上次错误（rename / delete 失败信息）。
    pub last_error: Option<String>,
    /// saves 目录（仅显示）。
    pub saves_dir: PathBuf,
}

impl SaveBrowser {
    pub fn new(saves_dir: PathBuf) -> Self {
        Self {
            open: false,
            saves: Vec::new(),
            selected: 0,
            rename_buffer: String::new(),
            renaming: false,
            confirm_delete: false,
            last_error: None,
            saves_dir,
        }
    }

    /// 替换扫描结果。
    pub fn set_saves(&mut self, saves: Vec<SaveEntry>) {
        self.saves = saves;
        if self.selected >= self.saves.len() {
            self.selected = self.saves.len().saturating_sub(1);
        }
    }

    pub fn current(&self) -> Option<&SaveEntry> {
        self.saves.get(self.selected)
    }

    fn show_v9(&mut self, ctx: &egui::Context) -> (bool, Vec<SaveCommand>) {
        if !self.open {
            return (false, Vec::new());
        }

        let mut cmds = Vec::new();
        let mut close_requested = false;

        let (shell_close, _) =
            UtilityWindowShell::new("save_browser_utility_window", tr("saves_title"))
                .subtitle("存档列表 / 读取 / 重命名 / 删除")
                .accent(VanillaIron::BRASS_BRIGHT)
                .footer("Q 关闭 | 双击存档读取")
                .show(ctx, |ui, layout| {
                    v9_save_browser_body(ui, layout.main, self, &mut cmds, &mut close_requested);
                });

        let close = shell_close || close_requested;
        if close {
            self.open = false;
            self.renaming = false;
            self.confirm_delete = false;
            self.last_error = None;
        }
        (close, cmds)
    }

    /// 渲染存档浏览器面板。返回 (close_panel, 命令列表)。
    pub fn show(&mut self, ctx: &egui::Context) -> (bool, Vec<SaveCommand>) {
        self.show_v9(ctx)
    }
}

/// G.4：拒绝路径分隔符 / NUL / 控制字符 / Windows 保留字符。
pub fn is_safe_filename(name: &str) -> bool {
    if name.is_empty() || name.len() > 200 {
        return false;
    }
    for c in name.chars() {
        if c == '/'
            || c == '\\'
            || c == '\0'
            || c == ':'
            || c == '*'
            || c == '?'
            || c == '"'
            || c == '<'
            || c == '>'
            || c == '|'
            || c.is_control()
        {
            return false;
        }
    }
    true
}

fn v9_save_browser_body(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    browser: &mut SaveBrowser,
    cmds: &mut Vec<SaveCommand>,
    close_requested: &mut bool,
) {
    use crate::v9::layout::{GridLayout, Track};
    use crate::v9::primitives::{Button, ButtonSize, ButtonVariant, Card};
    use crate::v9::tokens::{palette, spacing, TextRole};
    use egui::{Align2, Pos2, Rect, Vec2};

    ui.allocate_ui_at_rect(rect, |ui| {
        let grid = GridLayout::new(vec![Track::Fr(1.0)], vec![Track::Fr(0.58), Track::Fr(0.42)])
            .with_gutter(spacing::S5, 0.0);
        let cells = grid.measure(rect);
        let list = Card::new()
            .as_panel()
            .show_at(ui, GridLayout::cell(&cells, 0, 0));
        let detail = Card::new()
            .as_panel()
            .show_at(ui, GridLayout::cell(&cells, 0, 1));

        ui.painter().text(
            list.left_top(),
            Align2::LEFT_TOP,
            "存档列表",
            TextRole::Heading.font_id(),
            palette::BRASS_BRIGHT,
        );
        ui.painter().text(
            Pos2::new(list.right(), list.top()),
            Align2::RIGHT_TOP,
            format!("{} 个存档", browser.saves.len()),
            TextRole::Caption.font_id(),
            palette::MUTED,
        );
        let list_body = Rect::from_min_max(
            Pos2::new(list.left(), list.top() + 34.0),
            list.right_bottom(),
        );
        ui.allocate_ui_at_rect(list_body, |ui| {
            if browser.saves.is_empty() {
                crate::v9::composites::panel_shell::draw_empty_state(
                    ui,
                    list_body,
                    tr("no_saves"),
                    "存档目录中没有发现文件。",
                );
                return;
            }
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.set_width(list_body.width());
                    for (i, save) in browser.saves.iter().enumerate() {
                        let selected = i == browser.selected;
                        let label = format!(
                            "{}  |  {}  |  {}  |  {}",
                            save.display_name,
                            save.date_str.as_deref().unwrap_or("?"),
                            save.player_tag.as_deref().unwrap_or("?"),
                            save.modified_str(),
                        );
                        let response = ui
                            .selectable_label(selected, label)
                            .on_hover_text(save.path.display().to_string());
                        if response.clicked() {
                            browser.selected = i;
                            browser.confirm_delete = false;
                            browser.renaming = false;
                        }
                        if response.double_clicked() {
                            cmds.push(SaveCommand::Load(save.path.clone()));
                        }
                    }
                });
        });

        ui.painter().text(
            detail.left_top(),
            Align2::LEFT_TOP,
            "操作",
            TextRole::Heading.font_id(),
            palette::BRASS_BRIGHT,
        );
        let detail_body = Rect::from_min_max(
            Pos2::new(detail.left(), detail.top() + 34.0),
            detail.right_bottom(),
        );
        ui.allocate_ui_at_rect(detail_body, |ui| {
            ui.label(
                egui::RichText::new(format!("目录：{}", browser.saves_dir.display()))
                    .small()
                    .color(palette::MUTED),
            );
            if Button::new(tr("refresh"))
                .size(ButtonSize::Sm)
                .variant(ButtonVariant::Secondary)
                .show(ui)
                .clicked()
            {
                cmds.push(SaveCommand::Rescan);
            }
            ui.add_space(spacing::S4);

            let current = browser.current().map(|save| {
                (
                    save.path.clone(),
                    save.display_name.clone(),
                    save.date_str.clone(),
                    save.player_tag.clone(),
                    save.size_str(),
                )
            });
            if let Some((path, name, date, tag, size)) = current.clone() {
                ui.label(
                    egui::RichText::new(name.as_str())
                        .font(TextRole::Subheading.font_id())
                        .color(palette::PARCHMENT),
                );
                ui.label(
                    egui::RichText::new(format!(
                        "{}  |  {}  |  {}",
                        date.unwrap_or_else(|| "?".into()),
                        tag.unwrap_or_else(|| "?".into()),
                        size
                    ))
                    .small()
                    .color(palette::PARCHMENT_DIM),
                );
                ui.add_space(spacing::S4);

                ui.horizontal_wrapped(|ui| {
                    if Button::new(tr("load"))
                        .size(ButtonSize::Sm)
                        .variant(ButtonVariant::Primary)
                        .show(ui)
                        .clicked()
                    {
                        cmds.push(SaveCommand::Load(path.clone()));
                    }
                    if Button::new(tr("rename"))
                        .size(ButtonSize::Sm)
                        .variant(ButtonVariant::Secondary)
                        .show(ui)
                        .clicked()
                    {
                        browser.rename_buffer = name.clone();
                        browser.renaming = true;
                        browser.confirm_delete = false;
                    }
                    if Button::new(tr("delete"))
                        .size(ButtonSize::Sm)
                        .variant(ButtonVariant::Danger)
                        .show(ui)
                        .clicked()
                    {
                        browser.confirm_delete = true;
                        browser.renaming = false;
                    }
                });

                if browser.renaming {
                    ui.add_space(spacing::S4);
                    ui.label(egui::RichText::new(tr("new_name")).color(palette::MUTED));
                    ui.text_edit_singleline(&mut browser.rename_buffer);
                    ui.horizontal_wrapped(|ui| {
                        if Button::new(tr("confirm"))
                            .size(ButtonSize::Sm)
                            .variant(ButtonVariant::Primary)
                            .show(ui)
                            .clicked()
                        {
                            let trimmed = browser.rename_buffer.trim();
                            if trimmed.is_empty() {
                                browser.last_error = Some("名称不能为空".into());
                            } else if !is_safe_filename(trimmed) {
                                browser.last_error = Some("名称包含非法字符".into());
                            } else {
                                let ext =
                                    path.extension().and_then(|e| e.to_str()).unwrap_or("save");
                                let to = path.with_file_name(format!("{}.{}", trimmed, ext));
                                if to == path {
                                    browser.renaming = false;
                                } else if to.exists() {
                                    browser.last_error = Some("目标名称已存在".into());
                                } else {
                                    cmds.push(SaveCommand::Rename {
                                        from: path.clone(),
                                        to,
                                    });
                                    browser.renaming = false;
                                    browser.last_error = None;
                                }
                            }
                        }
                        if Button::new(tr("cancel"))
                            .size(ButtonSize::Sm)
                            .variant(ButtonVariant::Ghost)
                            .show(ui)
                            .clicked()
                        {
                            browser.renaming = false;
                            browser.last_error = None;
                        }
                    });
                }

                if browser.confirm_delete {
                    ui.add_space(spacing::S4);
                    ui.colored_label(
                        palette::BAD,
                        format!("{} '{}' ?", tr("confirm_delete"), name),
                    );
                    ui.horizontal_wrapped(|ui| {
                        if Button::new(tr("yes"))
                            .size(ButtonSize::Sm)
                            .variant(ButtonVariant::Danger)
                            .show(ui)
                            .clicked()
                        {
                            cmds.push(SaveCommand::Delete(path.clone()));
                            browser.confirm_delete = false;
                        }
                        if Button::new(tr("cancel"))
                            .size(ButtonSize::Sm)
                            .variant(ButtonVariant::Ghost)
                            .show(ui)
                            .clicked()
                        {
                            browser.confirm_delete = false;
                        }
                    });
                }
            } else {
                ui.colored_label(palette::MUTED, tr("no_saves"));
            }

            if let Some(err) = &browser.last_error {
                ui.add_space(spacing::S4);
                ui.colored_label(palette::BAD, err);
            }
            let close_rect = Rect::from_min_size(
                Pos2::new(detail.left(), detail.bottom() - 34.0),
                Vec2::new(128.0, 28.0),
            );
            if Button::new(tr("close"))
                .size(ButtonSize::Sm)
                .variant(ButtonVariant::Ghost)
                .show_at(ui, close_rect)
                .clicked()
            {
                *close_requested = true;
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_filename_basic() {
        assert!(is_safe_filename("germany_1939"));
        assert!(is_safe_filename("德国-1939"));
        assert!(!is_safe_filename(""));
        assert!(!is_safe_filename("with/slash"));
        assert!(!is_safe_filename("with\\backslash"));
        assert!(!is_safe_filename("with*star"));
        assert!(!is_safe_filename("with\nnewline"));
    }

    #[test]
    fn save_browser_starts_closed() {
        let b = SaveBrowser::new(default_saves_dir());
        assert!(!b.open);
        assert!(b.saves.is_empty());
        assert!(b.current().is_none());
    }

    #[test]
    fn set_saves_clamps_selected() {
        let mut b = SaveBrowser::new(default_saves_dir());
        b.selected = 100;
        b.set_saves(vec![SaveEntry {
            path: PathBuf::from("a.bin"),
            display_name: "a".into(),
            size_bytes: 0,
            modified: None,
            date_str: None,
            player_tag: None,
        }]);
        assert_eq!(b.selected, 0);
    }

    #[test]
    fn scan_saves_empty_dir_returns_empty() {
        // 不在文件系统中实际创建目录 — 不存在时返回空 vec。
        let dir = std::env::temp_dir().join("ironheart_g4_does_not_exist_xyz");
        let v = scan_saves(&dir, |_| None);
        assert!(v.is_empty());
    }

    #[test]
    fn scan_saves_picks_up_bin_and_txt() {
        let dir = std::env::temp_dir().join("ironheart_g4_test_scan");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.bin"), b"x").unwrap();
        std::fs::write(dir.join("b.txt"), b"x").unwrap();
        std::fs::write(dir.join("c.png"), b"x").unwrap(); // skip
        let v = scan_saves(&dir, |_| Some(("1936.01.01".into(), "GER".into())));
        assert_eq!(v.len(), 2);
        for s in &v {
            assert_eq!(s.player_tag.as_deref(), Some("GER"));
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn save_entry_size_str_human_readable() {
        let s = SaveEntry {
            path: PathBuf::from("a.bin"),
            display_name: "a".into(),
            size_bytes: 1_500_000,
            modified: None,
            date_str: None,
            player_tag: None,
        };
        let str = s.size_str();
        assert!(str.contains("MB"), "got '{}'", str);
    }

    #[test]
    fn default_saves_dir_under_documents_or_fallback() {
        let p = default_saves_dir();
        assert!(p.ends_with("saves"));
    }
}
