//! V5 阶段 G.3：设置面板（分辨率 / 全屏 / 音量 / 速度上限 / 自动暂停类别）。
//!
//! ## 设计
//!
//! 这一面板**只**负责读 / 写一个 [`Settings`] 值对象，并提供一个 egui 子 UI
//! [`SettingsPanel::show`] 让玩家修改它。它不直接持有 wgpu / winit / 音频后端
//! 的引用 —— 应用 [`Settings`] 到 window / surface / 音频设备由 caller
//! 在 `show()` 返回 [`SettingsCommand`] 后自行处理（main.rs）。
//!
//! 选这一架构是因为：
//!
//! - hoi4-ui crate 不依赖 winit / wgpu / 音频后端（保留 thin 性质）
//! - settings UI 与"应用 settings 的副作用"互相解耦，便于在 caller 侧做 dry-run
//!   （比如先校验分辨率是否被显示器支持，再发 `SetResolution` 命令）
//!
//! ## 持久化
//!
//! [`Settings::load_or_default`] / [`Settings::save`] 把 settings 写入 TOML
//! 文件（`%APPDATA%/ironheart/settings.toml` 或 `~/.config/ironheart/settings.toml`）。
//! 为避免引入 toml crate，使用极简手解 / 手写格式（仅 `key = value` / 整数 /
//! 浮点 / 布尔 / 字符串）。

use std::path::PathBuf;

use crate::egui;
use crate::i18n::{self, tr, Language};
use crate::v9::accessibility::{AccessibilitySettings, ColorBlindMode, FontScale};

/// G.3 唯一的 settings 值对象。所有字段都是显式默认值，没有 `Option`。
#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    /// 窗口分辨率（物理像素）。`None` 表示沿用桌面 / 当前窗口尺寸。
    pub resolution: Option<(u32, u32)>,
    /// 是否全屏（borderless）。
    pub fullscreen: bool,
    /// master 音量 0.0..=1.0。
    pub master_volume: f32,
    /// 音乐音量 0.0..=1.0（与 master 相乘）。
    pub music_volume: f32,
    /// UI 音效音量 0.0..=1.0（与 master 相乘）。
    pub ui_volume: f32,
    /// 速度上限：5 = 无限制；1..=4 = 玩家最高只能选到 SpeedN。
    pub max_speed: u8,
    /// 自动暂停类别开关。
    pub auto_pause: AutoPauseCategories,
    /// UI 语言。
    pub language: Language,
    pub color_blind_mode: ColorBlindMode,
    pub font_scale: FontScale,
    /// 显示开关：启用 3D 地形。关闭时使用 legacy 平面地形 fallback。
    pub enable_3d_terrain: bool,
    /// 调试开关：显示所有国家的部队（关闭迷雾过滤）。
    /// 用于观察 AI 战斗、SCW 双方部署等。默认 false。
    pub show_all_units: bool,
    /// 调试开关：显示所有部队时隐藏 AI 战线和进攻箭头。默认 false。
    pub hide_ai_frontlines: bool,
    /// 调试开关：跳过正当化战争目标，允许直接宣战。默认 false（正常游戏需要先正当化）。
    pub instant_war: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            resolution: None,
            fullscreen: false,
            master_volume: 0.80,
            music_volume: 0.65,
            ui_volume: 0.85,
            max_speed: 5,
            auto_pause: AutoPauseCategories::default(),
            language: Language::default(),
            color_blind_mode: ColorBlindMode::Off,
            font_scale: FontScale::Normal,
            enable_3d_terrain: true,
            show_all_units: false,
            hide_ai_frontlines: false,
            instant_war: false,
        }
    }
}

/// G.3：自动暂停事件类别。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AutoPauseCategories {
    /// 重大事件（events / 焦点完成 / 战争开始）触发时自动暂停。
    pub major_events: bool,
    /// 焦点完成后自动暂停。
    pub focus_completed: bool,
    /// 新战争（自身参战 / 阵营连带参战）后自动暂停。
    pub war_started: bool,
    /// 决议任务完成后自动暂停。
    pub decision_completed: bool,
    /// 关键紧张度突变（≥ ±5%）。
    pub tension_spike: bool,
}

impl Default for AutoPauseCategories {
    fn default() -> Self {
        Self {
            major_events: true,
            focus_completed: true,
            war_started: true,
            decision_completed: false,
            tension_spike: false,
        }
    }
}

impl Settings {
    pub fn accessibility(&self) -> AccessibilitySettings {
        AccessibilitySettings {
            color_blind_mode: self.color_blind_mode,
            font_scale: self.font_scale,
        }
    }

    /// 配置文件路径。
    pub fn config_path() -> Option<PathBuf> {
        if cfg!(target_os = "windows") {
            std::env::var_os("APPDATA")
                .map(|a| PathBuf::from(a).join("ironheart").join("settings.toml"))
        } else {
            std::env::var_os("HOME").map(|h| {
                PathBuf::from(h)
                    .join(".config")
                    .join("ironheart")
                    .join("settings.toml")
            })
        }
    }

    /// 从磁盘读 settings；任何错误（缺文件 / 解析失败）回退到 default。
    pub fn load_or_default() -> Self {
        let Some(path) = Self::config_path() else {
            return Self::default();
        };
        let Ok(text) = std::fs::read_to_string(&path) else {
            return Self::default();
        };
        Self::parse(&text)
    }

    /// 写入磁盘。失败仅返回 io::Error，不 panic。
    pub fn save(&self) -> std::io::Result<()> {
        let Some(path) = Self::config_path() else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "no config dir",
            ));
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, self.serialize())
    }

    /// G.3：极简 TOML 格式序列化，避免引入 toml crate。
    pub fn serialize(&self) -> String {
        let mut s = String::new();
        s.push_str("# Ironheart V5 — G.3 settings\n");
        if let Some((w, h)) = self.resolution {
            s.push_str(&format!("resolution_w = {}\n", w));
            s.push_str(&format!("resolution_h = {}\n", h));
        }
        s.push_str(&format!("fullscreen = {}\n", self.fullscreen));
        s.push_str(&format!("master_volume = {:.3}\n", self.master_volume));
        s.push_str(&format!("music_volume = {:.3}\n", self.music_volume));
        s.push_str(&format!("ui_volume = {:.3}\n", self.ui_volume));
        s.push_str(&format!("max_speed = {}\n", self.max_speed));
        s.push_str(&format!(
            "auto_pause_major_events = {}\n",
            self.auto_pause.major_events
        ));
        s.push_str(&format!(
            "auto_pause_focus_completed = {}\n",
            self.auto_pause.focus_completed
        ));
        s.push_str(&format!(
            "auto_pause_war_started = {}\n",
            self.auto_pause.war_started
        ));
        s.push_str(&format!(
            "auto_pause_decision_completed = {}\n",
            self.auto_pause.decision_completed
        ));
        s.push_str(&format!(
            "auto_pause_tension_spike = {}\n",
            self.auto_pause.tension_spike
        ));
        s.push_str(&format!("language = {}\n", self.language.code()));
        s.push_str(&format!(
            "color_blind_mode = {}\n",
            self.color_blind_mode.code()
        ));
        s.push_str(&format!("font_scale = {}\n", self.font_scale.code()));
        s.push_str(&format!("enable_3d_terrain = {}\n", self.enable_3d_terrain));
        s.push_str(&format!("show_all_units = {}\n", self.show_all_units));
        s.push_str(&format!(
            "hide_ai_frontlines = {}\n",
            self.hide_ai_frontlines
        ));
        s.push_str(&format!("instant_war = {}\n", self.instant_war));
        s
    }

    /// G.3：极简 TOML 解析（按行；`key = value`，注释以 `#` 开头）。
    pub fn parse(text: &str) -> Self {
        let mut out = Self::default();
        let mut res_w: Option<u32> = None;
        let mut res_h: Option<u32> = None;
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some(eq) = line.find('=') else { continue };
            let key = line[..eq].trim();
            let val = line[eq + 1..].trim();
            match key {
                "resolution_w" => res_w = val.parse().ok(),
                "resolution_h" => res_h = val.parse().ok(),
                "fullscreen" => out.fullscreen = parse_bool(val).unwrap_or(out.fullscreen),
                "master_volume" => {
                    if let Ok(v) = val.parse::<f32>() {
                        out.master_volume = v.clamp(0.0, 1.0);
                    }
                }
                "music_volume" => {
                    if let Ok(v) = val.parse::<f32>() {
                        out.music_volume = v.clamp(0.0, 1.0);
                    }
                }
                "ui_volume" => {
                    if let Ok(v) = val.parse::<f32>() {
                        out.ui_volume = v.clamp(0.0, 1.0);
                    }
                }
                "max_speed" => {
                    if let Ok(v) = val.parse::<u8>() {
                        out.max_speed = v.clamp(1, 5);
                    }
                }
                "auto_pause_major_events" => {
                    if let Some(b) = parse_bool(val) {
                        out.auto_pause.major_events = b;
                    }
                }
                "auto_pause_focus_completed" => {
                    if let Some(b) = parse_bool(val) {
                        out.auto_pause.focus_completed = b;
                    }
                }
                "auto_pause_war_started" => {
                    if let Some(b) = parse_bool(val) {
                        out.auto_pause.war_started = b;
                    }
                }
                "auto_pause_decision_completed" => {
                    if let Some(b) = parse_bool(val) {
                        out.auto_pause.decision_completed = b;
                    }
                }
                "auto_pause_tension_spike" => {
                    if let Some(b) = parse_bool(val) {
                        out.auto_pause.tension_spike = b;
                    }
                }
                "language" => {
                    out.language = Language::from_code(val);
                }
                "color_blind_mode" => {
                    out.color_blind_mode = ColorBlindMode::from_code(val);
                }
                "font_scale" => {
                    out.font_scale = FontScale::from_code(val);
                }
                "enable_3d_terrain" => {
                    if let Some(b) = parse_bool(val) {
                        out.enable_3d_terrain = b;
                    }
                }
                "show_all_units" => {
                    if let Some(b) = parse_bool(val) {
                        out.show_all_units = b;
                    }
                }
                "hide_ai_frontlines" => {
                    if let Some(b) = parse_bool(val) {
                        out.hide_ai_frontlines = b;
                    }
                }
                "instant_war" => {
                    if let Some(b) = parse_bool(val) {
                        out.instant_war = b;
                    }
                }
                _ => {}
            }
        }
        if let (Some(w), Some(h)) = (res_w, res_h) {
            if w >= 800 && h >= 600 {
                out.resolution = Some((w, h));
            }
        }
        out
    }
}

fn parse_bool(s: &str) -> Option<bool> {
    match s.trim().to_ascii_lowercase().as_str() {
        "true" | "yes" | "1" => Some(true),
        "false" | "no" | "0" => Some(false),
        _ => None,
    }
}

/// G.3：一组常见的 16:9 候选分辨率。
pub const COMMON_RESOLUTIONS: &[(u32, u32)] = &[
    (1280, 720),
    (1366, 768),
    (1600, 900),
    (1920, 1080),
    (2560, 1440),
    (3840, 2160),
];

/// G.3：从面板返回的副作用命令。caller 按序执行。
#[derive(Debug, Clone, PartialEq)]
pub enum SettingsCommand {
    /// 切换全屏（borderless）状态。`fullscreen` 是新值。
    SetFullscreen(bool),
    /// 设置窗口分辨率（仅 `fullscreen=false` 时有效；caller 决定是否真改）。
    SetResolution(u32, u32),
    /// 音量改变 —— caller 把它转给 `MusicPlayer::set_volumes`
    /// 与 `UiSoundBank::set_volumes`。
    SetVolumes,
    /// 设置最高速度。caller 把它 clamp 到当前 `world.speed`。
    SetMaxSpeed(u8),
    /// 语言切换。caller 调 `i18n::set_language`。
    SetLanguage(Language),
    SetAccessibility(AccessibilitySettings),
    /// 显示：切换 3D 地形。
    SetEnable3dTerrain(bool),
    /// 调试：切换"显示所有部队"开关。
    SetShowAllUnits(bool),
    /// 调试：隐藏 AI 战线和进攻箭头。
    SetHideAiFrontlines(bool),
    /// 调试：切换"跳过正当化直接宣战"开关。
    SetInstantWar(bool),
    /// 持久化到磁盘。
    Save,
}

/// G.3：egui 设置面板。
pub struct SettingsPanel {
    pub open: bool,
    pub draft: Settings,
    /// 上次成功 save 的 settings；用于 cancel 还原。
    pub committed: Settings,
    /// 当前选中的分辨率索引（仅 UI 状态）。
    pub resolution_idx: usize,
    /// 上一次错误信息（save 失败 / 解析失败）。
    pub last_error: Option<String>,
}

impl SettingsPanel {
    pub fn new(initial: Settings) -> Self {
        let resolution_idx = COMMON_RESOLUTIONS
            .iter()
            .position(|r| Some(*r) == initial.resolution)
            .unwrap_or(0);
        Self {
            open: false,
            draft: initial.clone(),
            committed: initial,
            resolution_idx,
            last_error: None,
        }
    }

    /// 把当前 committed 值打开面板。
    pub fn open_with(&mut self, current: Settings) {
        self.committed = current.clone();
        self.draft = current;
        self.resolution_idx = COMMON_RESOLUTIONS
            .iter()
            .position(|r| Some(*r) == self.draft.resolution)
            .unwrap_or(0);
        self.open = true;
        self.last_error = None;
    }

    fn show_v9(&mut self, ctx: &egui::Context) -> (bool, Vec<SettingsCommand>) {
        if !self.open {
            return (false, Vec::new());
        }
        use crate::v9::composites::panel_shell::{
            draw_summary_tiles, draw_tab_strip, PanelClass, PanelShell,
        };
        use crate::v9::tokens::palette;

        let mut cmds: Vec<SettingsCommand> = Vec::new();
        let mut close_requested = false;
        let resolution = self
            .draft
            .resolution
            .map(|(w, h)| format!("{}x{}", w, h))
            .unwrap_or_else(|| "Current".to_owned());
        let terrain = if self.draft.enable_3d_terrain {
            "3D"
        } else {
            "Legacy"
        };

        let (shell_close, _) = PanelShell::new("settings_panel_v9", tr("settings_title"))
            .subtitle("Display / Audio / Gameplay")
            .class(PanelClass::Settings)
            .accent(palette::INFO)
            .footer("Q Close  |  Apply writes settings.toml")
            .show(ctx, |ui, layout| {
                draw_summary_tiles(
                    ui,
                    layout.summary,
                    &[
                        (
                            "Language",
                            self.draft.language.code().to_owned(),
                            palette::GOLD,
                        ),
                        ("Resolution", resolution, palette::INFO),
                        (
                            "Fullscreen",
                            if self.draft.fullscreen { "On" } else { "Off" }.to_owned(),
                            if self.draft.fullscreen {
                                palette::GOOD
                            } else {
                                palette::MUTED
                            },
                        ),
                        (
                            "Speed",
                            self.draft.max_speed.to_string(),
                            palette::BRASS_BRIGHT,
                        ),
                        (
                            "Font",
                            self.draft.font_scale.label().to_owned(),
                            palette::INFO,
                        ),
                        ("Terrain", terrain.to_owned(), palette::GOLD),
                    ],
                );
                draw_tab_strip(
                    ui,
                    layout.tabs,
                    "Settings / Debug / Auto-pause",
                    palette::INFO,
                );
                v9_settings_body(ui, layout.body, self, &mut cmds, &mut close_requested);
            });

        let close = shell_close || close_requested;
        if close {
            self.open = false;
        }
        (close, cmds)
    }

    /// 渲染面板。返回 (`close_panel`, 副作用命令列表)。caller 应在 [`close_panel=true`] 时
    /// 关闭其外部状态对应的面板（与 InGamePanel::Settings 联动）。
    #[allow(unreachable_code)]
    pub fn show(&mut self, ctx: &egui::Context) -> (bool, Vec<SettingsCommand>) {
        return self.show_v9(ctx);

        if !self.open {
            return (false, Vec::new());
        }
        use crate::components;
        let mut cmds: Vec<SettingsCommand> = Vec::new();
        let mut close = false;
        let mut window_open = self.open;
        egui::Window::new(tr("settings_title"))
            .open(&mut window_open)
            .default_width(440.0)
            .resizable(false)
            .frame(
                egui::Frame::new()
                    .fill(egui::Color32::from_rgb(0x22, 0x18, 0x10))
                    .stroke(egui::Stroke::new(1.5, components::GOLD_DIM))
                    .inner_margin(egui::Margin::symmetric(12, 10)),
            )
            .show(ctx, |ui| {
                // ── 语言 ────────────────────────────────────────
                components::section(ui, tr("language"), |ui| {
                    ui.horizontal(|ui| {
                        let prev_lang = self.draft.language;
                        for &lang in Language::all() {
                            ui.selectable_value(
                                &mut self.draft.language,
                                lang,
                                lang.display_name(),
                            );
                        }
                        if prev_lang != self.draft.language {
                            i18n::set_language(self.draft.language);
                            cmds.push(SettingsCommand::SetLanguage(self.draft.language));
                        }
                    });
                });

                // ── 显示 ────────────────────────────────────────
                components::section(ui, tr("display"), |ui| {
                    let was_fs = self.draft.fullscreen;
                    ui.checkbox(&mut self.draft.fullscreen, tr("fullscreen"));
                    if was_fs != self.draft.fullscreen {
                        cmds.push(SettingsCommand::SetFullscreen(self.draft.fullscreen));
                    }

                    ui.add_enabled_ui(!self.draft.fullscreen, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(format!("{}:", tr("resolution")));
                            let prev_idx = self.resolution_idx;
                            egui::ComboBox::from_id_salt("settings_res")
                                .selected_text(format!(
                                    "{} × {}",
                                    COMMON_RESOLUTIONS[self.resolution_idx].0,
                                    COMMON_RESOLUTIONS[self.resolution_idx].1
                                ))
                                .show_ui(ui, |ui| {
                                    for (i, (w, h)) in COMMON_RESOLUTIONS.iter().enumerate() {
                                        ui.selectable_value(
                                            &mut self.resolution_idx,
                                            i,
                                            format!("{} × {}", w, h),
                                        );
                                    }
                                });
                            if prev_idx != self.resolution_idx {
                                let (w, h) = COMMON_RESOLUTIONS[self.resolution_idx];
                                self.draft.resolution = Some((w, h));
                                cmds.push(SettingsCommand::SetResolution(w, h));
                            }
                        });
                    });

                    let prev_3d_terrain = self.draft.enable_3d_terrain;
                    ui.checkbox(&mut self.draft.enable_3d_terrain, tr("enable_3d_terrain"));
                    if prev_3d_terrain != self.draft.enable_3d_terrain {
                        cmds.push(SettingsCommand::SetEnable3dTerrain(
                            self.draft.enable_3d_terrain,
                        ));
                    }
                });

                // ── 音量 ────────────────────────────────────────
                components::section(ui, tr("audio"), |ui| {
                    let mut volumes_changed = false;
                    volumes_changed |= ui
                        .add(
                            egui::Slider::new(&mut self.draft.master_volume, 0.0..=1.0)
                                .text(tr("master_volume")),
                        )
                        .changed();
                    volumes_changed |= ui
                        .add(
                            egui::Slider::new(&mut self.draft.music_volume, 0.0..=1.0)
                                .text(tr("music_volume")),
                        )
                        .changed();
                    volumes_changed |= ui
                        .add(
                            egui::Slider::new(&mut self.draft.ui_volume, 0.0..=1.0)
                                .text(tr("ui_volume")),
                        )
                        .changed();
                    if volumes_changed {
                        cmds.push(SettingsCommand::SetVolumes);
                    }
                });

                // ── 速度上限 ────────────────────────────────────
                components::section(ui, tr("game"), |ui| {
                    let prev_speed = self.draft.max_speed;
                    ui.horizontal(|ui| {
                        ui.label(format!("{}:", tr("max_speed")));
                        for n in 1u8..=5 {
                            ui.selectable_value(&mut self.draft.max_speed, n, format!("{}", n));
                        }
                    });
                    if prev_speed != self.draft.max_speed {
                        cmds.push(SettingsCommand::SetMaxSpeed(self.draft.max_speed));
                    }
                });

                // ── 自动暂停 ────────────────────────────────────
                components::section(ui, tr("auto_pause"), |ui| {
                    ui.checkbox(
                        &mut self.draft.auto_pause.major_events,
                        tr("auto_pause_major_events"),
                    );
                    ui.checkbox(
                        &mut self.draft.auto_pause.focus_completed,
                        tr("auto_pause_focus_completed"),
                    );
                    ui.checkbox(
                        &mut self.draft.auto_pause.war_started,
                        tr("auto_pause_war_started"),
                    );
                    ui.checkbox(
                        &mut self.draft.auto_pause.decision_completed,
                        tr("auto_pause_decision_completed"),
                    );
                    ui.checkbox(
                        &mut self.draft.auto_pause.tension_spike,
                        tr("auto_pause_tension_spike"),
                    );
                });

                // ── 调试 ────────────────────────────────────────
                components::section(ui, tr("debug"), |ui| {
                    let prev_show_all = self.draft.show_all_units;
                    ui.checkbox(&mut self.draft.show_all_units, tr("show_all_units"));
                    if prev_show_all != self.draft.show_all_units {
                        cmds.push(SettingsCommand::SetShowAllUnits(self.draft.show_all_units));
                    }
                    let prev_hide_ai_frontlines = self.draft.hide_ai_frontlines;
                    ui.checkbox(&mut self.draft.hide_ai_frontlines, tr("hide_ai_frontlines"));
                    if prev_hide_ai_frontlines != self.draft.hide_ai_frontlines {
                        cmds.push(SettingsCommand::SetHideAiFrontlines(
                            self.draft.hide_ai_frontlines,
                        ));
                    }
                    let prev_instant = self.draft.instant_war;
                    ui.checkbox(&mut self.draft.instant_war, tr("instant_war"));
                    if prev_instant != self.draft.instant_war {
                        cmds.push(SettingsCommand::SetInstantWar(self.draft.instant_war));
                    }
                });

                // ── 操作按钮 ────────────────────────────────────
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if components::action_button_colored(
                        ui,
                        true,
                        tr("apply_save"),
                        components::GOOD,
                    )
                    .clicked()
                    {
                        cmds.push(SettingsCommand::Save);
                        self.committed = self.draft.clone();
                    }
                    if components::action_button_colored(ui, true, tr("revert"), components::WARN)
                        .clicked()
                    {
                        let prev_lang = self.draft.language;
                        self.draft = self.committed.clone();
                        self.resolution_idx = COMMON_RESOLUTIONS
                            .iter()
                            .position(|r| Some(*r) == self.draft.resolution)
                            .unwrap_or(0);
                        i18n::set_language(self.draft.language);
                        if prev_lang != self.draft.language {
                            cmds.push(SettingsCommand::SetLanguage(self.draft.language));
                        }
                    }
                    if components::action_button_colored(ui, true, tr("close"), components::MUTED)
                        .clicked()
                    {
                        close = true;
                    }
                });
                if let Some(err) = &self.last_error {
                    ui.add_space(4.0);
                    ui.colored_label(components::BAD, err);
                }
            });
        if !window_open {
            close = true;
        }
        if close {
            self.open = false;
        }
        (close, cmds)
    }
}

fn v9_settings_body(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    panel: &mut SettingsPanel,
    cmds: &mut Vec<SettingsCommand>,
    close_requested: &mut bool,
) {
    use crate::v9::layout::{GridLayout, Track};
    use crate::v9::primitives::{Button, ButtonSize, ButtonVariant, Card};
    use crate::v9::tokens::{palette, spacing, TextRole};
    use egui::{Align2, Pos2, Rect, Vec2};

    ui.allocate_ui_at_rect(rect, |ui| {
        let grid = GridLayout::new(vec![Track::Fr(1.0)], vec![Track::Fr(0.52), Track::Fr(0.48)])
            .with_gutter(spacing::S5, 0.0);
        let cells = grid.measure(rect);
        let left = Card::new()
            .as_panel()
            .show_at(ui, GridLayout::cell(&cells, 0, 0));
        let right = Card::new()
            .as_panel()
            .show_at(ui, GridLayout::cell(&cells, 0, 1));

        ui.allocate_ui_at_rect(left, |ui| {
            ui.label(
                egui::RichText::new(tr("display"))
                    .font(TextRole::Heading.font_id())
                    .color(palette::BRASS_BRIGHT),
            );
            let prev_lang = panel.draft.language;
            ui.horizontal_wrapped(|ui| {
                ui.label(egui::RichText::new(tr("language")).color(palette::MUTED));
                for &lang in Language::all() {
                    ui.selectable_value(&mut panel.draft.language, lang, lang.display_name());
                }
            });
            if prev_lang != panel.draft.language {
                i18n::set_language(panel.draft.language);
                cmds.push(SettingsCommand::SetLanguage(panel.draft.language));
            }

            let prev_accessibility = panel.draft.accessibility();
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Color mode").color(palette::MUTED));
                egui::ComboBox::from_id_salt("settings_color_mode_v9")
                    .selected_text(panel.draft.color_blind_mode.label())
                    .show_ui(ui, |ui| {
                        for &mode in ColorBlindMode::all() {
                            ui.selectable_value(
                                &mut panel.draft.color_blind_mode,
                                mode,
                                mode.label(),
                            );
                        }
                    });
            });
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Font scale").color(palette::MUTED));
                for &scale in FontScale::all() {
                    ui.selectable_value(&mut panel.draft.font_scale, scale, scale.label());
                }
            });
            if prev_accessibility != panel.draft.accessibility() {
                cmds.push(SettingsCommand::SetAccessibility(
                    panel.draft.accessibility(),
                ));
            }

            let was_fs = panel.draft.fullscreen;
            ui.checkbox(&mut panel.draft.fullscreen, tr("fullscreen"));
            if was_fs != panel.draft.fullscreen {
                cmds.push(SettingsCommand::SetFullscreen(panel.draft.fullscreen));
            }
            ui.add_enabled_ui(!panel.draft.fullscreen, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(tr("resolution")).color(palette::MUTED));
                    let prev_idx = panel.resolution_idx;
                    egui::ComboBox::from_id_salt("settings_res_v9")
                        .selected_text(format!(
                            "{} x {}",
                            COMMON_RESOLUTIONS[panel.resolution_idx].0,
                            COMMON_RESOLUTIONS[panel.resolution_idx].1
                        ))
                        .show_ui(ui, |ui| {
                            for (i, (w, h)) in COMMON_RESOLUTIONS.iter().enumerate() {
                                ui.selectable_value(
                                    &mut panel.resolution_idx,
                                    i,
                                    format!("{} x {}", w, h),
                                );
                            }
                        });
                    if prev_idx != panel.resolution_idx {
                        let (w, h) = COMMON_RESOLUTIONS[panel.resolution_idx];
                        panel.draft.resolution = Some((w, h));
                        cmds.push(SettingsCommand::SetResolution(w, h));
                    }
                });
            });

            let prev_terrain = panel.draft.enable_3d_terrain;
            ui.checkbox(&mut panel.draft.enable_3d_terrain, tr("enable_3d_terrain"));
            if prev_terrain != panel.draft.enable_3d_terrain {
                cmds.push(SettingsCommand::SetEnable3dTerrain(
                    panel.draft.enable_3d_terrain,
                ));
            }

            ui.add_space(spacing::S5);
            ui.label(
                egui::RichText::new(tr("audio"))
                    .font(TextRole::Heading.font_id())
                    .color(palette::BRASS_BRIGHT),
            );
            let mut volumes_changed = false;
            volumes_changed |= ui
                .add(
                    egui::Slider::new(&mut panel.draft.master_volume, 0.0..=1.0)
                        .text(tr("master_volume")),
                )
                .changed();
            volumes_changed |= ui
                .add(
                    egui::Slider::new(&mut panel.draft.music_volume, 0.0..=1.0)
                        .text(tr("music_volume")),
                )
                .changed();
            volumes_changed |= ui
                .add(egui::Slider::new(&mut panel.draft.ui_volume, 0.0..=1.0).text(tr("ui_volume")))
                .changed();
            if volumes_changed {
                cmds.push(SettingsCommand::SetVolumes);
            }

            ui.add_space(spacing::S5);
            ui.label(
                egui::RichText::new(tr("game"))
                    .font(TextRole::Heading.font_id())
                    .color(palette::BRASS_BRIGHT),
            );
            let prev_speed = panel.draft.max_speed;
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(tr("max_speed")).color(palette::MUTED));
                for n in 1u8..=5 {
                    ui.selectable_value(&mut panel.draft.max_speed, n, format!("{}", n));
                }
            });
            if prev_speed != panel.draft.max_speed {
                cmds.push(SettingsCommand::SetMaxSpeed(panel.draft.max_speed));
            }
        });

        ui.allocate_ui_at_rect(right, |ui| {
            ui.label(
                egui::RichText::new(tr("auto_pause"))
                    .font(TextRole::Heading.font_id())
                    .color(palette::BRASS_BRIGHT),
            );
            ui.checkbox(
                &mut panel.draft.auto_pause.major_events,
                tr("auto_pause_major_events"),
            );
            ui.checkbox(
                &mut panel.draft.auto_pause.focus_completed,
                tr("auto_pause_focus_completed"),
            );
            ui.checkbox(
                &mut panel.draft.auto_pause.war_started,
                tr("auto_pause_war_started"),
            );
            ui.checkbox(
                &mut panel.draft.auto_pause.decision_completed,
                tr("auto_pause_decision_completed"),
            );
            ui.checkbox(
                &mut panel.draft.auto_pause.tension_spike,
                tr("auto_pause_tension_spike"),
            );

            ui.add_space(spacing::S5);
            ui.label(
                egui::RichText::new(tr("debug"))
                    .font(TextRole::Heading.font_id())
                    .color(palette::BRASS_BRIGHT),
            );
            let prev_show_all = panel.draft.show_all_units;
            ui.checkbox(&mut panel.draft.show_all_units, tr("show_all_units"));
            if prev_show_all != panel.draft.show_all_units {
                cmds.push(SettingsCommand::SetShowAllUnits(panel.draft.show_all_units));
            }
            let prev_hide_ai = panel.draft.hide_ai_frontlines;
            ui.checkbox(
                &mut panel.draft.hide_ai_frontlines,
                tr("hide_ai_frontlines"),
            );
            if prev_hide_ai != panel.draft.hide_ai_frontlines {
                cmds.push(SettingsCommand::SetHideAiFrontlines(
                    panel.draft.hide_ai_frontlines,
                ));
            }
            let prev_instant = panel.draft.instant_war;
            ui.checkbox(&mut panel.draft.instant_war, tr("instant_war"));
            if prev_instant != panel.draft.instant_war {
                cmds.push(SettingsCommand::SetInstantWar(panel.draft.instant_war));
            }

            let actions_top = right.bottom() - 92.0;
            ui.painter().hline(
                right.left()..=right.right(),
                actions_top - spacing::S3,
                egui::Stroke::new(1.0, palette::HAIRLINE),
            );
            if Button::new(tr("apply_save"))
                .size(ButtonSize::Md)
                .variant(ButtonVariant::Primary)
                .show_at(
                    ui,
                    Rect::from_min_size(
                        Pos2::new(right.left(), actions_top),
                        Vec2::new(160.0, 32.0),
                    ),
                )
                .clicked()
            {
                cmds.push(SettingsCommand::Save);
                panel.committed = panel.draft.clone();
            }
            if Button::new(tr("revert"))
                .size(ButtonSize::Md)
                .variant(ButtonVariant::Secondary)
                .show_at(
                    ui,
                    Rect::from_min_size(
                        Pos2::new(right.left() + 172.0, actions_top),
                        Vec2::new(132.0, 32.0),
                    ),
                )
                .clicked()
            {
                let prev_lang = panel.draft.language;
                let prev_accessibility = panel.draft.accessibility();
                panel.draft = panel.committed.clone();
                panel.resolution_idx = COMMON_RESOLUTIONS
                    .iter()
                    .position(|r| Some(*r) == panel.draft.resolution)
                    .unwrap_or(0);
                i18n::set_language(panel.draft.language);
                if prev_lang != panel.draft.language {
                    cmds.push(SettingsCommand::SetLanguage(panel.draft.language));
                }
                if prev_accessibility != panel.draft.accessibility() {
                    cmds.push(SettingsCommand::SetAccessibility(
                        panel.draft.accessibility(),
                    ));
                }
            }
            if Button::new(tr("close"))
                .size(ButtonSize::Md)
                .variant(ButtonVariant::Ghost)
                .show_at(
                    ui,
                    Rect::from_min_size(
                        Pos2::new(right.left() + 316.0, actions_top),
                        Vec2::new(112.0, 32.0),
                    ),
                )
                .clicked()
            {
                *close_requested = true;
            }
            if let Some(err) = &panel.last_error {
                ui.painter().text(
                    Pos2::new(right.left(), actions_top + 46.0),
                    Align2::LEFT_TOP,
                    err,
                    TextRole::Caption.font_id(),
                    palette::BAD,
                );
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_sane() {
        let s = Settings::default();
        assert_eq!(s.max_speed, 5);
        assert!(!s.fullscreen);
        assert!((s.master_volume - 0.80).abs() < 1e-6);
    }

    #[test]
    fn round_trip_serialize() {
        let s1 = Settings {
            resolution: Some((1920, 1080)),
            fullscreen: true,
            master_volume: 0.55,
            music_volume: 0.40,
            ui_volume: 0.90,
            max_speed: 3,
            auto_pause: AutoPauseCategories {
                major_events: false,
                focus_completed: true,
                war_started: true,
                decision_completed: true,
                tension_spike: true,
            },
            language: Language::Chinese,
            color_blind_mode: ColorBlindMode::Deuteranopia,
            font_scale: FontScale::ExtraLarge,
            enable_3d_terrain: false,
            show_all_units: true,
            hide_ai_frontlines: true,
            instant_war: true,
        };
        let text = s1.serialize();
        let s2 = Settings::parse(&text);
        assert_eq!(s1, s2);
    }

    #[test]
    fn parse_ignores_garbage() {
        let s = Settings::parse(
            r#"
# comment
not a key value pair
master_volume = 0.42
junk junk junk
"#,
        );
        assert!((s.master_volume - 0.42).abs() < 1e-6);
        assert_eq!(s.max_speed, 5); // default
    }

    #[test]
    fn parse_clamps_volume() {
        let s = Settings::parse("master_volume = 5.0\n");
        assert_eq!(s.master_volume, 1.0);
    }

    #[test]
    fn parse_clamps_max_speed() {
        let s = Settings::parse("max_speed = 99\n");
        assert_eq!(s.max_speed, 5);
        let s = Settings::parse("max_speed = 0\n");
        assert_eq!(s.max_speed, 1);
    }

    #[test]
    fn parse_accessibility_settings() {
        let s = Settings::parse("color_blind_mode = protanopia\nfont_scale = 1.15\n");
        assert_eq!(s.color_blind_mode, ColorBlindMode::Protanopia);
        assert_eq!(s.font_scale, FontScale::Large);
    }

    #[test]
    fn parse_resolution_requires_both_w_and_h() {
        let s = Settings::parse("resolution_w = 1920\n");
        assert!(s.resolution.is_none());
        let s = Settings::parse("resolution_w = 1920\nresolution_h = 1080\n");
        assert_eq!(s.resolution, Some((1920, 1080)));
    }

    #[test]
    fn parse_rejects_too_small_resolution() {
        let s = Settings::parse("resolution_w = 320\nresolution_h = 240\n");
        assert!(s.resolution.is_none());
    }

    #[test]
    fn auto_pause_defaults_have_three_on() {
        let a = AutoPauseCategories::default();
        let on_count = [
            a.major_events,
            a.focus_completed,
            a.war_started,
            a.decision_completed,
            a.tension_spike,
        ]
        .iter()
        .filter(|b| **b)
        .count();
        assert_eq!(on_count, 3);
    }

    #[test]
    fn settings_panel_starts_closed() {
        let p = SettingsPanel::new(Settings::default());
        assert!(!p.open);
    }

    #[test]
    fn open_with_resets_draft_to_current() {
        let initial = Settings {
            master_volume: 0.10,
            ..Default::default()
        };
        let mut p = SettingsPanel::new(Settings::default());
        p.draft.master_volume = 0.99;
        p.open_with(initial.clone());
        assert!(p.open);
        assert_eq!(p.draft, initial);
        assert_eq!(p.committed, initial);
    }
}
