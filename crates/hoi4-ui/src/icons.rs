//! Sprite icon helper — V5 阶段 B.5。
//!
//! 给 caller 一个 `IconBank::get_or_load("GFX_focus_GER_xxx")` 接口，按
//! `GFX_xxx` 命名把 vanilla `gfx/interface/.../xxx.dds` 加载、解码、注册到
//! `egui::Context`，返回 [`egui::TextureHandle`]。同名 icon 仅加载一次。
//!
//! ## 命名映射
//!
//! - `GFX_focus_GER_anschluss` → strip `GFX_` → `focus_GER_anschluss` →
//!   依次在 [`IconBank::search_dirs`] 列出的目录里找 `<dir>/focus_GER_anschluss.dds`，
//!   命中即返回；全部失败缓存为 `Missing`。
//! - 默认 `search_dirs` = `["gfx/interface/goals"]`（B.5 起步）。
//!   B.5+ 阶段（C.2 政治面板 ideas / C.4 决议）按需 `add_search_dir(...)`。
//!
//! ## 缓存语义
//!
//! - 命中（`Loaded`）：缓存 `TextureHandle` + `[w, h]` 像素尺寸；后续 `get_or_load`
//!   直接从 cache 返回，不做 IO。
//! - 失败（`Missing`）：缓存「不存在」标记，避免每帧重试 path_cfg.find 浪费 IO。
//! - 调试目的可调 [`IconBank::clear_cache`] 强制下一次重新加载（热重载用）。
//!
//! ## 当前限制（B.5 起步）
//!
//! - 仅支持 BGRA8 / BC1 / BC3 三种 DDS 格式（[`crate::dds_decode`]）。
//! - 未实现热重载文件系统监听 —— `add_search_dir` 之后已经命中过的 GFX 名
//!   仍走 cache；需要重置 cache 才会重新解析路径。
//! - `IconBank` 会优先读取 vanilla/mod `.gfx` 的 sprite -> texturefile 索引；
//!   直接按 `<sprite stem>.dds` 搜索只作为缺失 mapping 时的 fallback。

use std::borrow::Cow;
use std::collections::HashMap;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use egui::{
    ColorImage, Context, TextureFilter, TextureHandle, TextureOptions, TextureWrapMode, Vec2,
};

use hoi4_assets::dds::DdsImage;
use hoi4_assets::tga::TgaImage;
use hoi4_paths::PathConfig;

use crate::dds_decode::{decode_mip0_to_rgba, DdsDecodeError};

/// 一个 GFX 名字在 cache 里的状态。
enum IconEntry {
    /// 已成功加载并注册到 egui。
    Loaded {
        handle: TextureHandle,
        size_px: [usize; 2],
    },
    /// 试过但失败（找不到 / 解码失败）。`reason` 仅诊断用。
    Missing { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IconSpriteProbeReport {
    pub sprite_name: String,
    pub texture_file: Option<String>,
    pub attempted_paths: Vec<String>,
    pub loaded: bool,
    pub failure_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextureFileProbeReport {
    pub texture_file: String,
    pub attempted_path: String,
    pub loaded: bool,
    pub failure_reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TexturePixelStats {
    pub width: u32,
    pub height: u32,
    pub non_transparent_pixels: usize,
    pub mean_rgb: [f32; 3],
    pub min_alpha: u8,
    pub max_alpha: u8,
    pub near_white_gray_ratio: f32,
}

#[derive(Debug, Clone, Copy)]
struct EmbeddedPngIcon {
    gfx_name: &'static str,
    file_name: &'static str,
    bytes: &'static [u8],
}

const PROJECT_LAW_ICONS: &[EmbeddedPngIcon] = &[
    EmbeddedPngIcon {
        gfx_name: "GFX_law_volunteer_only",
        file_name: "volunteer_only.png",
        bytes: include_bytes!("../assets/law_icons/volunteer_only.png"),
    },
    EmbeddedPngIcon {
        gfx_name: "GFX_law_limited_conscription",
        file_name: "limited_conscription.png",
        bytes: include_bytes!("../assets/law_icons/limited_conscription.png"),
    },
    EmbeddedPngIcon {
        gfx_name: "GFX_law_extensive_conscription",
        file_name: "extensive_conscription.png",
        bytes: include_bytes!("../assets/law_icons/extensive_conscription.png"),
    },
    EmbeddedPngIcon {
        gfx_name: "GFX_law_total_mobilization",
        file_name: "total_mobilization.png",
        bytes: include_bytes!("../assets/law_icons/total_mobilization.png"),
    },
    EmbeddedPngIcon {
        gfx_name: "GFX_law_laissez_faire",
        file_name: "laissez_faire.png",
        bytes: include_bytes!("../assets/law_icons/laissez_faire.png"),
    },
    EmbeddedPngIcon {
        gfx_name: "GFX_law_interventionism",
        file_name: "interventionism.png",
        bytes: include_bytes!("../assets/law_icons/interventionism.png"),
    },
    EmbeddedPngIcon {
        gfx_name: "GFX_law_war_economy",
        file_name: "war_economy.png",
        bytes: include_bytes!("../assets/law_icons/war_economy.png"),
    },
    EmbeddedPngIcon {
        gfx_name: "GFX_law_corporatist_war_economy",
        file_name: "corporatist_war_economy.png",
        bytes: include_bytes!("../assets/law_icons/corporatist_war_economy.png"),
    },
    EmbeddedPngIcon {
        gfx_name: "GFX_law_planned_economy",
        file_name: "planned_economy.png",
        bytes: include_bytes!("../assets/law_icons/planned_economy.png"),
    },
    EmbeddedPngIcon {
        gfx_name: "GFX_law_free_trade",
        file_name: "free_trade.png",
        bytes: include_bytes!("../assets/law_icons/free_trade.png"),
    },
    EmbeddedPngIcon {
        gfx_name: "GFX_law_export_focus",
        file_name: "export_focus.png",
        bytes: include_bytes!("../assets/law_icons/export_focus.png"),
    },
    EmbeddedPngIcon {
        gfx_name: "GFX_law_import_substitution",
        file_name: "import_substitution.png",
        bytes: include_bytes!("../assets/law_icons/import_substitution.png"),
    },
    EmbeddedPngIcon {
        gfx_name: "GFX_law_autarky",
        file_name: "autarky.png",
        bytes: include_bytes!("../assets/law_icons/autarky.png"),
    },
    EmbeddedPngIcon {
        gfx_name: "GFX_law_state_trade_monopoly",
        file_name: "state_trade_monopoly.png",
        bytes: include_bytes!("../assets/law_icons/state_trade_monopoly.png"),
    },
    EmbeddedPngIcon {
        gfx_name: "GFX_law_low_taxation",
        file_name: "low_taxation.png",
        bytes: include_bytes!("../assets/law_icons/low_taxation.png"),
    },
    EmbeddedPngIcon {
        gfx_name: "GFX_law_medium_taxation",
        file_name: "medium_taxation.png",
        bytes: include_bytes!("../assets/law_icons/medium_taxation.png"),
    },
    EmbeddedPngIcon {
        gfx_name: "GFX_law_high_taxation",
        file_name: "high_taxation.png",
        bytes: include_bytes!("../assets/law_icons/high_taxation.png"),
    },
    EmbeddedPngIcon {
        gfx_name: "GFX_law_war_taxation",
        file_name: "war_taxation.png",
        bytes: include_bytes!("../assets/law_icons/war_taxation.png"),
    },
    EmbeddedPngIcon {
        gfx_name: "GFX_law_open_society",
        file_name: "open_society.png",
        bytes: include_bytes!("../assets/law_icons/open_society.png"),
    },
    EmbeddedPngIcon {
        gfx_name: "GFX_law_limited_rights",
        file_name: "limited_rights.png",
        bytes: include_bytes!("../assets/law_icons/limited_rights.png"),
    },
    EmbeddedPngIcon {
        gfx_name: "GFX_law_national_security_act",
        file_name: "national_security_act.png",
        bytes: include_bytes!("../assets/law_icons/national_security_act.png"),
    },
    EmbeddedPngIcon {
        gfx_name: "GFX_law_police_state",
        file_name: "police_state.png",
        bytes: include_bytes!("../assets/law_icons/police_state.png"),
    },
    EmbeddedPngIcon {
        gfx_name: "GFX_law_free_press",
        file_name: "free_press.png",
        bytes: include_bytes!("../assets/law_icons/free_press.png"),
    },
    EmbeddedPngIcon {
        gfx_name: "GFX_law_regulated_press",
        file_name: "regulated_press.png",
        bytes: include_bytes!("../assets/law_icons/regulated_press.png"),
    },
    EmbeddedPngIcon {
        gfx_name: "GFX_law_state_media",
        file_name: "state_media.png",
        bytes: include_bytes!("../assets/law_icons/state_media.png"),
    },
    EmbeddedPngIcon {
        gfx_name: "GFX_law_total_propaganda",
        file_name: "total_propaganda.png",
        bytes: include_bytes!("../assets/law_icons/total_propaganda.png"),
    },
];

/// 一组 sprite icon 的 lazy 加载缓存。
///
/// `Clone` 不能：里面的 `Context` 与 `PathConfig` 都是 cheap-clone 的，
/// 但 `cache` 应单例所有；caller 应只在 `RenderState` 持有一份。
pub struct IconBank {
    ctx: Context,
    path_cfg: PathConfig,
    cache: HashMap<String, IconEntry>,
    sprite_texturefiles: HashMap<String, String>,
    /// 路径搜索顺序（高优先级在前）。每个元素是相对游戏根的目录。
    search_dirs: Vec<String>,
}

impl IconBank {
    /// 构造一个空 bank，默认仅搜索 `gfx/interface/goals`（B.5 起步）。
    pub fn new(ctx: Context, path_cfg: PathConfig) -> Self {
        let sprite_texturefiles = load_sprite_texturefiles(&path_cfg);
        Self {
            ctx,
            path_cfg,
            cache: HashMap::new(),
            sprite_texturefiles,
            search_dirs: vec!["gfx/interface/goals".to_owned()],
        }
    }

    /// 在搜索路径列表末尾追加一个目录（低优先级 fallback）。
    pub fn add_search_dir(&mut self, rel: impl Into<String>) {
        let rel = rel.into();
        if !self.search_dirs.iter().any(|existing| existing == &rel) {
            self.search_dirs.push(rel);
            self.cache
                .retain(|_, entry| !matches!(entry, IconEntry::Missing { .. }));
        }
    }

    /// Add the vanilla interface directories used by the country politics panel.
    ///
    /// The `.gfx` sprite map is still the preferred path, but these directories make
    /// direct `<sprite stem>.dds` fallback work for the politics panel shell and idea
    /// category textures when a mod omits a sprite mapping.
    pub fn add_politics_search_dirs(&mut self) {
        for rel in [
            "gfx/interface",
            "gfx/interface/ideas",
            "gfx/interface/tiles",
            "gfx/interface/goals",
        ] {
            self.add_search_dir(rel);
        }
    }

    /// Add vanilla interface directories used by `countrydecisionview`.
    ///
    /// Sprite `textureFile` metadata remains the preferred lookup path. These
    /// directories only help when a modded or partial `.gfx` set omits direct
    /// mappings for decision sprites.
    pub fn add_decision_search_dirs(&mut self) {
        for rel in [
            "gfx/interface/decisions",
            "gfx/interface",
            "gfx/interface/goals",
        ] {
            self.add_search_dir(rel);
        }
    }

    /// Add vanilla interface directories used by `nationalfocusview`.
    ///
    /// This covers focus node backgrounds, titlebar variants and focus/goal
    /// symbols without copying any vanilla texture into the project.
    pub fn add_focus_search_dirs(&mut self) {
        for rel in [
            "gfx/interface/focusview",
            "gfx/interface/focusview/titlebar",
            "gfx/interface/techtree",
            "gfx/interface/goals",
            "gfx/interface",
        ] {
            self.add_search_dir(rel);
        }
    }

    /// Add vanilla interface directories used by `countrylogisticsview`.
    ///
    /// Sprite `textureFile` metadata remains the primary lookup path; these
    /// directories are bounded fallbacks for partial or modded `.gfx` indexes.
    pub fn add_logistics_search_dirs(&mut self) {
        for rel in [
            "gfx/interface/production",
            "gfx/interface/archetypes",
            "gfx/interface/technologies",
            "gfx/interface",
        ] {
            self.add_search_dir(rel);
        }
    }

    /// Add vanilla interface directories used by `countrydiplomacyview`.
    ///
    /// The `.gfx` sprite map remains the authoritative lookup source. These
    /// directories are only bounded fallbacks for partial indexes or modded
    /// installs, and keep diplomacy assets runtime-only.
    pub fn add_diplomacy_search_dirs(&mut self) {
        for rel in [
            "gfx/interface",
            "gfx/interface/diplomacy",
            "gfx/interface/ideologies",
            "gfx/interface/ideas",
            "gfx/interface/goals",
            "gfx/leaders",
        ] {
            self.add_search_dir(rel);
        }
    }

    /// Add fallback search directories for a vanilla GUI profile id.
    pub fn add_profile_search_dirs(&mut self, profile_id: &str) {
        match profile_id {
            crate::vanilla_gui::COUNTRY_POLITICS_PROFILE_ID => self.add_politics_search_dirs(),
            crate::vanilla_gui::COUNTRY_DECISION_PROFILE_ID => self.add_decision_search_dirs(),
            crate::vanilla_gui::NATIONAL_FOCUS_PROFILE_ID => self.add_focus_search_dirs(),
            crate::vanilla_gui::COUNTRY_LOGISTICS_PROFILE_ID => self.add_logistics_search_dirs(),
            crate::vanilla_gui::COUNTRY_DIPLOMACY_PROFILE_ID => self.add_diplomacy_search_dirs(),
            _ => {}
        }
    }

    /// J.1.4：批量追加 `gfx/leaders/<TAG>/` 作为肖像查找路径。
    ///
    /// 调用方传入所有需要支持的 country tag 列表（通常 = `World.countries.tags`
    /// 中的非空项）。每个 tag 的目录会按 alphabetical 顺序（确定性）追加到
    /// search_dirs 末尾。重复 tag 不会重复添加。
    ///
    /// 之后调用 `get_or_load("GFX_portrait_<TAG>_<name>")` 时，命中流程：
    /// 1. 在每个 leader 目录里尝试 `<dir>/portrait_<TAG>_<name>.dds`（vanilla 新规约）
    /// 2. 全部 miss 则尝试 fuzzy fallback（vanilla 老规约：`Portrait_<English>_<Capitalized>.dds`），
    ///    详见 [`Self::try_load`]。
    pub fn add_leader_dirs<I, S>(&mut self, tags: I)
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut sorted: Vec<String> = tags.into_iter().map(|t| t.as_ref().to_owned()).collect();
        sorted.sort();
        sorted.dedup();
        for tag in sorted {
            if tag.is_empty() {
                continue;
            }
            let rel = format!("gfx/leaders/{tag}");
            if !self.search_dirs.iter().any(|d| d == &rel) {
                self.search_dirs.push(rel);
            }
        }
    }

    /// 全部搜索路径列表（只读，调试用）。
    pub fn search_dirs(&self) -> &[String] {
        &self.search_dirs
    }

    /// 拿名为 `gfx_name` 的 icon。命中返回 `&TextureHandle`，未命中返回 `None`。
    ///
    /// 第一次调用某 GFX 名字会触发 path_cfg.find + DDS 解码 + 上传纹理；之后从
    /// cache 直接命中。
    pub fn get_or_load(&mut self, gfx_name: &str) -> Option<&TextureHandle> {
        if !self.cache.contains_key(gfx_name) {
            let entry = self.try_load(gfx_name);
            self.cache.insert(gfx_name.to_owned(), entry);
        }
        match self.cache.get(gfx_name)? {
            IconEntry::Loaded { handle, .. } => Some(handle),
            IconEntry::Missing { .. } => None,
        }
    }

    /// Load a vanilla/mod `textureFile` path directly.
    ///
    /// This is used by `.gfx` resource types such as `progressbartype`, where
    /// foreground and background textures are separate fields under one GFX
    /// resource name.
    pub fn get_or_load_texture_file(&mut self, texture_file: &str) -> Option<&TextureHandle> {
        let texture_file = texture_file.replace('\\', "/").replace("//", "/");
        let cache_key = format!("textureFile:{texture_file}");
        if !self.cache.contains_key(&cache_key) {
            let entry = self.try_load_texture_file(&cache_key, &texture_file);
            self.cache.insert(cache_key.clone(), entry);
        }
        match self.cache.get(&cache_key)? {
            IconEntry::Loaded { handle, .. } => Some(handle),
            IconEntry::Missing { .. } => None,
        }
    }

    /// 已加载 icon 的像素尺寸（如果已 cache 为 Loaded 则返回；其他状态返回 `None`）。
    pub fn size_of(&self, gfx_name: &str) -> Option<[usize; 2]> {
        match self.cache.get(gfx_name)? {
            IconEntry::Loaded { size_px, .. } => Some(*size_px),
            _ => None,
        }
    }

    pub fn size_of_texture_file(&self, texture_file: &str) -> Option<[usize; 2]> {
        let normalized = texture_file.replace('\\', "/").replace("//", "/");
        let cache_key = format!("textureFile:{normalized}");
        match self.cache.get(&cache_key)? {
            IconEntry::Loaded { size_px, .. } => Some(*size_px),
            _ => None,
        }
    }

    /// 失败原因（调试用）。命中或未尝试时返回 `None`。
    pub fn missing_reason(&self, gfx_name: &str) -> Option<&str> {
        match self.cache.get(gfx_name)? {
            IconEntry::Missing { reason } => Some(reason.as_str()),
            _ => None,
        }
    }

    /// 清空 cache。下一次 `get_or_load` 会重新走 IO 路径（热重载用）。
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }

    /// 当前 cache 中已 Loaded 的 icon 数量（启动 banner / 调试）。
    pub fn loaded_count(&self) -> usize {
        self.cache
            .values()
            .filter(|e| matches!(e, IconEntry::Loaded { .. }))
            .count()
    }

    /// 当前 cache 中已 Missing 的 icon 数量。
    pub fn missing_count(&self) -> usize {
        self.cache
            .values()
            .filter(|e| matches!(e, IconEntry::Missing { .. }))
            .count()
    }

    /// Whether a sprite name was found in parsed vanilla/mod `.gfx` files.
    pub fn has_sprite_mapping(&self, gfx_name: &str) -> bool {
        self.sprite_texturefiles.contains_key(gfx_name)
    }

    pub fn sprite_texturefile(&self, gfx_name: &str) -> Option<&str> {
        self.sprite_texturefiles.get(gfx_name).map(String::as_str)
    }

    /// Probe a set of sprites and return `(loaded, missing)`.
    ///
    /// This intentionally goes through `get_or_load`, so the result validates the
    /// full path resolution and DDS decode path, not only `.gfx` text parsing.
    pub fn diagnose_sprites<'a>(
        &mut self,
        sprites: impl IntoIterator<Item = &'a str>,
    ) -> (usize, usize) {
        let mut loaded = 0usize;
        let mut missing = 0usize;
        for sprite in sprites {
            if self.get_or_load(sprite).is_some() {
                loaded += 1;
            } else {
                missing += 1;
            }
        }
        (loaded, missing)
    }

    pub fn diagnose_sprite(&mut self, gfx_name: &str) -> IconSpriteProbeReport {
        let _ = self.get_or_load(gfx_name);
        let stem = gfx_name.strip_prefix("GFX_").unwrap_or(gfx_name);
        let texture_file = self.sprite_texturefiles.get(gfx_name).cloned();
        let mut attempted_paths = Vec::new();
        if let Some(texture_file) = texture_file.as_deref() {
            attempted_paths.push(
                self.path_cfg
                    .find(texture_file)
                    .map(|path| path.display().to_string())
                    .unwrap_or_else(|| texture_file.to_owned()),
            );
        }
        for dir in &self.search_dirs {
            let rel = format!("{dir}/{stem}.dds");
            attempted_paths.push(
                self.path_cfg
                    .find(&rel)
                    .map(|path| path.display().to_string())
                    .unwrap_or(rel),
            );
        }
        attempted_paths.sort();
        attempted_paths.dedup();

        IconSpriteProbeReport {
            sprite_name: gfx_name.to_owned(),
            texture_file,
            attempted_paths,
            loaded: self.get_or_load(gfx_name).is_some(),
            failure_reason: self.missing_reason(gfx_name).map(str::to_owned),
        }
    }

    pub fn diagnose_texture_file(&mut self, texture_file: &str) -> TextureFileProbeReport {
        let normalized = texture_file.replace('\\', "/").replace("//", "/");
        let _ = self.get_or_load_texture_file(&normalized);
        let cache_key = format!("textureFile:{normalized}");
        let attempted_path = self
            .path_cfg
            .find(&normalized)
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| normalized.clone());
        TextureFileProbeReport {
            texture_file: normalized,
            attempted_path,
            loaded: self.get_or_load_texture_file(texture_file).is_some(),
            failure_reason: self.missing_reason(&cache_key).map(str::to_owned),
        }
    }

    pub fn texture_file_pixel_stats(
        &self,
        texture_file: &str,
    ) -> Result<TexturePixelStats, String> {
        let normalized = texture_file.replace('\\', "/").replace("//", "/");
        let abs = self
            .path_cfg
            .find(&normalized)
            .ok_or_else(|| format!("{normalized} textureFile path not found"))?;
        match abs
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_ascii_lowercase())
            .as_deref()
        {
            Some("dds") => {
                let bytes = std::fs::read(&abs)
                    .map_err(|e| format!("io error on {}: {e}", abs.display()))?;
                let dds = DdsImage::parse(&bytes)
                    .map_err(|e| format!("dds parse on {}: {e:?}", abs.display()))?;
                let rgba = decode_mip0_to_rgba(&dds)
                    .map_err(|e| format!("decode {}: {e}", abs.display()))?;
                Ok(texture_pixel_stats(dds.width, dds.height, &rgba))
            }
            Some("tga") => {
                let bytes = std::fs::read(&abs)
                    .map_err(|e| format!("io error on {}: {e}", abs.display()))?;
                let tga = TgaImage::parse(&bytes)
                    .map_err(|e| format!("tga parse on {}: {e}", abs.display()))?;
                Ok(texture_pixel_stats(tga.width, tga.height, &tga.pixels))
            }
            _ => Err(format!(
                "unsupported textureFile extension for {}",
                abs.display()
            )),
        }
    }

    pub fn embedded_png_pixel_stats(gfx_name: &str) -> Option<TexturePixelStats> {
        let icon = embedded_png_icon(gfx_name)?;
        let (width, height, rgba) = decode_png_rgba(icon.bytes).ok()?;
        Some(texture_pixel_stats(width, height, &rgba))
    }

    fn try_load(&self, gfx_name: &str) -> IconEntry {
        if let Some(icon) = embedded_png_icon(gfx_name) {
            return self
                .load_embedded_png_to_entry(gfx_name, icon)
                .unwrap_or_else(|reason| IconEntry::Missing { reason });
        }

        let stem = gfx_name.strip_prefix("GFX_").unwrap_or(gfx_name);

        if let Some(entry) = self.try_load_flag_tga(gfx_name, stem) {
            return entry;
        }

        let lookup_names = icon_lookup_names(gfx_name);
        let filename = format!("{stem}.dds");
        let mut tried: Vec<PathBuf> = Vec::new();
        let mut last_load_err: Option<String> = None;

        for lookup_name in &lookup_names {
            if let Some(texturefile) = self.sprite_texturefiles.get(lookup_name.as_ref()) {
                if let Some(abs) = self.path_cfg.find(texturefile) {
                    tried.push(abs.clone());
                    match self.load_dds_to_entry(gfx_name, &abs) {
                        Ok(entry) => return entry,
                        Err(reason) => last_load_err = Some(reason),
                    }
                } else {
                    tried.push(PathBuf::from(texturefile));
                }
            }
        }

        if is_event_picture_gfx(gfx_name) {
            for lookup_name in &lookup_names {
                let lookup_stem = lookup_name.strip_prefix("GFX_").unwrap_or(lookup_name);
                for rel in event_picture_fallback_rel_paths(lookup_stem) {
                    let abs = match self.path_cfg.find(&rel) {
                        Some(p) => p,
                        None => {
                            tried.push(PathBuf::from(rel));
                            continue;
                        }
                    };
                    tried.push(abs.clone());
                    match self.load_dds_to_entry(gfx_name, &abs) {
                        Ok(entry) => return entry,
                        Err(reason) => {
                            last_load_err = Some(reason);
                            continue;
                        }
                    }
                }
            }

            return IconEntry::Missing {
                reason: match last_load_err {
                    Some(r) => r,
                    None => format!(
                        "{gfx_name} event picture not found in sprite map or event-picture dirs (tried {} paths)",
                        tried.len()
                    ),
                },
            };
        }

        for dir in &self.search_dirs {
            let rel = format!("{dir}/{filename}");
            let abs = match self.path_cfg.find(&rel) {
                Some(p) => p,
                None => {
                    tried.push(PathBuf::from(rel));
                    continue;
                }
            };
            tried.push(abs.clone());
            match self.load_dds_to_entry(gfx_name, &abs) {
                Ok(entry) => return entry,
                Err(reason) => {
                    last_load_err = Some(reason);
                    // 同名不同目录的文件继续试；如果都失败再走 fuzzy fallback
                    continue;
                }
            }
        }

        // J.1.4 fallback：肖像 GFX 名形如 `portrait_<TAG>_<name>` 时，vanilla 老规约
        // 用 `Portrait_<English>_<Name>.dds` 命名（如 `Portrait_Britain_Stanley_Baldwin.dds`），
        // 与 GFX 名不直接匹配。我们扫描 `gfx/leaders/<TAG>/` 目录下所有 DDS，按
        // 名字 token 模糊匹配；命中即返回。仅对 portrait_* 类 GFX 启用。
        if let Some((tag, name_parts)) = parse_leader_gfx_name(stem) {
            if let Some(found) = self.fuzzy_find_leader_portrait(&tag, &name_parts) {
                match self.load_dds_to_entry(gfx_name, &found) {
                    Ok(entry) => return entry,
                    Err(reason) => last_load_err = Some(reason),
                }
            }
        }

        IconEntry::Missing {
            reason: match last_load_err {
                Some(r) => r,
                None => format!(
                    "{gfx_name} not found in any of {} search dirs (tried {} paths)",
                    self.search_dirs.len(),
                    tried.len()
                ),
            },
        }
    }

    fn try_load_texture_file(&self, cache_key: &str, texture_file: &str) -> IconEntry {
        let Some(abs) = self.path_cfg.find(texture_file) else {
            return IconEntry::Missing {
                reason: format!("{texture_file} textureFile path not found"),
            };
        };
        match abs
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_ascii_lowercase())
            .as_deref()
        {
            Some("dds") => self
                .load_dds_to_entry(cache_key, &abs)
                .unwrap_or_else(|reason| IconEntry::Missing { reason }),
            Some("tga") => self
                .load_tga_to_entry(cache_key, &abs)
                .unwrap_or_else(|reason| IconEntry::Missing { reason }),
            Some("png") => self
                .load_png_to_entry(cache_key, &abs)
                .unwrap_or_else(|reason| IconEntry::Missing { reason }),
            _ => IconEntry::Missing {
                reason: format!("unsupported textureFile extension for {}", abs.display()),
            },
        }
    }

    fn try_load_flag_tga(&self, gfx_name: &str, stem: &str) -> Option<IconEntry> {
        let rest = stem.strip_prefix("flag_")?;
        let mut parts = rest.split('_');
        let tag = parts.next()?.to_ascii_uppercase();
        if tag.is_empty() {
            return None;
        }
        let ideology = parts.next().unwrap_or_default().to_ascii_lowercase();

        if let Some(img) = hoi4_assets::generated_historical_flag(&tag) {
            return Some(self.rgba_to_entry(gfx_name, img.width, img.height, &img.pixels));
        }

        let candidates = if ideology.is_empty() {
            vec![
                format!("gfx/flags/{tag}.tga"),
                format!("gfx/flags/medium/{tag}.tga"),
            ]
        } else {
            vec![
                format!("gfx/flags/{tag}_{ideology}.tga"),
                format!("gfx/flags/{tag}.tga"),
                format!("gfx/flags/medium/{tag}_{ideology}.tga"),
                format!("gfx/flags/medium/{tag}.tga"),
            ]
        };

        let mut tried = 0usize;
        let mut last_err = None;
        for rel in candidates {
            tried += 1;
            let Some(abs) = self.path_cfg.find(&rel) else {
                continue;
            };
            let bytes = match std::fs::read(&abs) {
                Ok(bytes) => bytes,
                Err(e) => {
                    last_err = Some(format!("io error on {}: {e}", abs.display()));
                    continue;
                }
            };
            let img = match TgaImage::parse(&bytes) {
                Ok(img) => img,
                Err(e) => {
                    last_err = Some(format!("tga parse on {}: {e}", abs.display()));
                    continue;
                }
            };
            return Some(self.rgba_to_entry(gfx_name, img.width, img.height, &img.pixels));
        }

        Some(IconEntry::Missing {
            reason: last_err.unwrap_or_else(|| {
                format!("{gfx_name} flag TGA not found (tried {tried} candidate paths)")
            }),
        })
    }

    /// 公共载入子流程：把一个绝对 DDS 路径解码上传成 `IconEntry::Loaded`，否则
    /// 把失败原因作为 `Err(String)` 返回（caller 决定是否记 Missing）。
    fn load_dds_to_entry(&self, gfx_name: &str, abs: &Path) -> Result<IconEntry, String> {
        let bytes =
            std::fs::read(abs).map_err(|e| format!("io error on {}: {e}", abs.display()))?;
        let dds = DdsImage::parse(&bytes)
            .map_err(|e| format!("dds parse on {}: {e:?}", abs.display()))?;
        let rgba = match decode_mip0_to_rgba(&dds) {
            Ok(r) => r,
            Err(DdsDecodeError::Unsupported(fmt)) => {
                return Err(format!("unsupported DDS format {fmt:?} for {gfx_name}"));
            }
            Err(other) => {
                return Err(format!("decode {gfx_name}: {other}"));
            }
        };
        let img =
            ColorImage::from_rgba_unmultiplied([dds.width as usize, dds.height as usize], &rgba);
        Ok(self.color_image_to_entry(gfx_name, img, dds.width, dds.height))
    }

    fn load_tga_to_entry(&self, gfx_name: &str, abs: &Path) -> Result<IconEntry, String> {
        let bytes =
            std::fs::read(abs).map_err(|e| format!("io error on {}: {e}", abs.display()))?;
        let img =
            TgaImage::parse(&bytes).map_err(|e| format!("tga parse on {}: {e}", abs.display()))?;
        Ok(self.rgba_to_entry(gfx_name, img.width, img.height, &img.pixels))
    }

    fn load_png_to_entry(&self, gfx_name: &str, abs: &Path) -> Result<IconEntry, String> {
        let bytes =
            std::fs::read(abs).map_err(|e| format!("io error on {}: {e}", abs.display()))?;
        let (width, height, rgba) =
            decode_png_rgba(&bytes).map_err(|e| format!("png decode on {}: {e}", abs.display()))?;
        Ok(self.rgba_to_entry(gfx_name, width, height, &rgba))
    }

    fn load_embedded_png_to_entry(
        &self,
        gfx_name: &str,
        icon: EmbeddedPngIcon,
    ) -> Result<IconEntry, String> {
        let (width, height, rgba) = decode_png_rgba(icon.bytes)
            .map_err(|e| format!("embedded png {} decode: {e}", icon.file_name))?;
        Ok(self.rgba_to_entry(gfx_name, width, height, &rgba))
    }

    fn rgba_to_entry(&self, gfx_name: &str, width: u32, height: u32, rgba: &[u8]) -> IconEntry {
        let img = ColorImage::from_rgba_unmultiplied([width as usize, height as usize], rgba);
        self.color_image_to_entry(gfx_name, img, width, height)
    }

    fn color_image_to_entry(
        &self,
        gfx_name: &str,
        img: ColorImage,
        width: u32,
        height: u32,
    ) -> IconEntry {
        let handle = self.ctx.load_texture(
            gfx_name,
            img,
            TextureOptions {
                magnification: TextureFilter::Linear,
                minification: TextureFilter::Linear,
                wrap_mode: TextureWrapMode::ClampToEdge,
                mipmap_mode: None,
            },
        );
        IconEntry::Loaded {
            handle,
            size_px: [width as usize, height as usize],
        }
    }

    /// 在 `gfx/leaders/<TAG>/` 下扫描 DDS，找一个文件名（去 stem 小写）与
    /// `name_parts` 全部匹配的（substring 包含）。返回首个命中的绝对路径。
    fn fuzzy_find_leader_portrait(&self, tag: &str, name_parts: &[String]) -> Option<PathBuf> {
        for rel in self.leader_search_dirs_for_tag(tag) {
            let Some(dir_abs) = self.path_cfg.find(&rel) else {
                continue;
            };
            let Some(found) = find_matching_leader_portrait_in_dir(&dir_abs, name_parts) else {
                continue;
            };
            return Some(found);
        }
        None
    }

    fn leader_search_dirs_for_tag(&self, tag: &str) -> Vec<String> {
        let suffix = format!("/gfx/leaders/{tag}").to_ascii_lowercase();
        let direct = format!("gfx/leaders/{tag}");
        let mut dirs = Vec::new();
        for dir in &self.search_dirs {
            let lower = dir.to_ascii_lowercase();
            if lower == direct.to_ascii_lowercase() || lower.ends_with(&suffix) {
                dirs.push(dir.clone());
            }
        }
        if !dirs.iter().any(|dir| dir.eq_ignore_ascii_case(&direct)) {
            dirs.push(direct);
        }
        dirs
    }
}

fn texture_pixel_stats(width: u32, height: u32, rgba: &[u8]) -> TexturePixelStats {
    let mut non_transparent_pixels = 0usize;
    let mut rgb_sum = [0u64; 3];
    let mut min_alpha = u8::MAX;
    let mut max_alpha = u8::MIN;
    let mut near_white_gray = 0usize;
    for px in rgba.chunks_exact(4) {
        let [r, g, b, a] = [px[0], px[1], px[2], px[3]];
        min_alpha = min_alpha.min(a);
        max_alpha = max_alpha.max(a);
        if a > 0 {
            non_transparent_pixels += 1;
            rgb_sum[0] += r as u64;
            rgb_sum[1] += g as u64;
            rgb_sum[2] += b as u64;
            let channels_close = r.abs_diff(g) <= 8 && r.abs_diff(b) <= 8 && g.abs_diff(b) <= 8;
            if channels_close && r >= 180 && g >= 180 && b >= 180 {
                near_white_gray += 1;
            }
        }
    }
    if rgba.is_empty() {
        min_alpha = 0;
        max_alpha = 0;
    }
    let denom = non_transparent_pixels.max(1) as f32;
    TexturePixelStats {
        width,
        height,
        non_transparent_pixels,
        mean_rgb: [
            rgb_sum[0] as f32 / denom,
            rgb_sum[1] as f32 / denom,
            rgb_sum[2] as f32 / denom,
        ],
        min_alpha,
        max_alpha,
        near_white_gray_ratio: near_white_gray as f32 / denom,
    }
}

fn embedded_png_icon(gfx_name: &str) -> Option<EmbeddedPngIcon> {
    PROJECT_LAW_ICONS
        .iter()
        .copied()
        .find(|icon| icon.gfx_name == gfx_name)
}

fn decode_png_rgba(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), String> {
    let cursor = Cursor::new(bytes);
    let mut decoder = png::Decoder::new(cursor);
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
    let bytes = &buf[..info.buffer_size()];
    let rgba = match info.color_type {
        png::ColorType::Rgba => bytes.to_vec(),
        png::ColorType::Rgb => {
            let mut out = Vec::with_capacity((info.width * info.height * 4) as usize);
            for px in bytes.chunks_exact(3) {
                out.extend_from_slice(&[px[0], px[1], px[2], 255]);
            }
            out
        }
        png::ColorType::Grayscale => {
            let mut out = Vec::with_capacity((info.width * info.height * 4) as usize);
            for &v in bytes {
                out.extend_from_slice(&[v, v, v, 255]);
            }
            out
        }
        png::ColorType::GrayscaleAlpha => {
            let mut out = Vec::with_capacity((info.width * info.height * 4) as usize);
            for px in bytes.chunks_exact(2) {
                out.extend_from_slice(&[px[0], px[0], px[0], px[1]]);
            }
            out
        }
        png::ColorType::Indexed => {
            return Err("indexed color PNG should have been expanded".to_owned());
        }
    };
    Ok((info.width, info.height, rgba))
}

fn icon_lookup_names(gfx_name: &str) -> Vec<Cow<'_, str>> {
    let mut names = Vec::with_capacity(4);
    push_unique_lookup_name(&mut names, Cow::Borrowed(gfx_name));

    if let Some(alias) = focus_icon_alias(gfx_name) {
        push_unique_lookup_name(&mut names, Cow::Borrowed(alias));
    }

    if let Some(alias) = event_picture_alias(gfx_name) {
        push_unique_lookup_name(&mut names, Cow::Borrowed(alias));
    }

    if let Some(rest) = gfx_name.strip_prefix("GFX_event_") {
        push_unique_lookup_name(&mut names, Cow::Owned(format!("GFX_news_event_{rest}")));
        push_unique_lookup_name(&mut names, Cow::Owned(format!("GFX_report_event_{rest}")));
    }

    names
}

fn push_unique_lookup_name<'a>(names: &mut Vec<Cow<'a, str>>, candidate: Cow<'a, str>) {
    if names
        .iter()
        .any(|existing| existing.as_ref() == candidate.as_ref())
    {
        return;
    }
    names.push(candidate);
}

fn focus_icon_alias(gfx_name: &str) -> Option<&'static str> {
    match gfx_name {
        "GFX_focus_generic_air_doctrine" => Some("GFX_goal_generic_air_doctrine"),
        "GFX_focus_generic_air_production" => Some("GFX_goal_generic_air_production"),
        "GFX_focus_generic_alliance" => Some("GFX_goal_generic_alliance"),
        "GFX_focus_generic_annex" => Some("GFX_goal_generic_territory_or_war"),
        "GFX_focus_generic_army_doctrines" => Some("GFX_goal_generic_army_doctrines"),
        "GFX_focus_generic_demand_territory" => Some("GFX_goal_generic_demand_territory"),
        "GFX_focus_generic_democracy" => Some("GFX_goal_generic_support_democracy"),
        "GFX_focus_generic_diplomatic" => Some("GFX_goal_generic_improve_relations"),
        "GFX_focus_generic_fortify" => Some("GFX_goal_generic_fortify_city"),
        "GFX_focus_generic_industry" => Some("GFX_goal_generic_production"),
        "GFX_focus_generic_military_economy" => Some("GFX_goal_generic_construct_military"),
        "GFX_focus_generic_monarchy" => Some("GFX_goal_generic_neutrality_focus"),
        "GFX_focus_generic_navy" => Some("GFX_goal_generic_build_navy"),
        "GFX_focus_generic_parliament" => Some("GFX_goal_generic_political_pressure"),
        "GFX_focus_generic_provoke_war" => Some("GFX_goal_generic_major_war"),
        "GFX_focus_generic_purge" => Some("GFX_goal_generic_dangerous_deal"),
        "GFX_focus_generic_tank" => Some("GFX_goal_generic_build_tank"),
        "GFX_focus_generic_trade" => Some("GFX_goal_generic_trade"),
        _ => None,
    }
}

fn event_picture_alias(gfx_name: &str) -> Option<&'static str> {
    match gfx_name {
        "GFX_event_mustard_gas" | "GFX_event_addis_ababa" | "GFX_event_ethiopia_victory" => {
            Some("GFX_news_event_ETH_ethiopian_warriors")
        }
        "GFX_event_selassie_exile" => Some("GFX_news_event_ETH_selassie_league_of_nations"),
        "GFX_event_italy" | "GFX_event_italy_ethiopia" => {
            Some("GFX_report_event_generic_italian_celebration")
        }
        "GFX_event_chinese_defense" | "GFX_event_chinese_victory" => {
            Some("GFX_news_event_chinese_soldiers_mountain")
        }
        "GFX_event_shanghai_battle" => Some("GFX_news_event_generic_shanghai_clash"),
        "GFX_event_nanjing" | "GFX_event_chongqing_bombing" => {
            Some("GFX_news_event_chinese_soldiers_city_ruin")
        }
        "GFX_event_marco_polo_bridge" | "GFX_event_japanese_victory_china" => {
            Some("GFX_report_event_chinese_japanese_handshake")
        }
        _ => None,
    }
}

fn is_event_picture_gfx(gfx_name: &str) -> bool {
    gfx_name.starts_with("GFX_event_")
        || gfx_name.starts_with("GFX_news_event_")
        || gfx_name.starts_with("GFX_report_event_")
}

fn event_picture_fallback_rel_paths(stem: &str) -> [String; 2] {
    [
        format!("gfx/event_pictures/{stem}.dds"),
        format!("gfx/interface/{stem}.dds"),
    ]
}

fn find_matching_leader_portrait_in_dir(dir_abs: &Path, name_parts: &[String]) -> Option<PathBuf> {
    if !dir_abs.is_dir() {
        return None;
    }
    let entries = std::fs::read_dir(dir_abs).ok()?;
    for entry in entries.flatten() {
        let p = entry.path();
        let ext_ok = p
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("dds"))
            .unwrap_or(false);
        if !ext_ok {
            continue;
        }
        let stem = match p.file_stem().and_then(|s| s.to_str()) {
            Some(s) => normalize_leader_token(s),
            None => continue,
        };
        if name_parts.iter().all(|part| stem.contains(part)) {
            return Some(p);
        }
    }
    None
}

/// 拆解形如 `portrait_<TAG>_<name>_<words>` 的 GFX stem，得到 (TAG, name_words)。
/// 不匹配（不是 `portrait_` 起头或 TAG 不像 3-letter）则返回 `None`。
fn parse_leader_gfx_name(stem: &str) -> Option<(String, Vec<String>)> {
    let lower = stem.to_ascii_lowercase();
    let rest = lower.strip_prefix("portrait_")?;
    let mut iter = rest.splitn(2, '_');
    let tag = iter.next()?;
    if !(2..=4).contains(&tag.len()) {
        return None;
    }
    if !tag
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
    {
        return None;
    }
    let name = iter.next()?;
    let normalized_name = normalize_leader_token(name);
    if normalized_name.is_empty() {
        return None;
    }
    if normalized_name.len() < 2 {
        return None;
    }
    Some((tag.to_uppercase(), vec![normalized_name]))
}

fn normalize_leader_token(s: &str) -> String {
    s.chars()
        .flat_map(|c| c.to_lowercase())
        .filter(|c| c.is_ascii_alphanumeric())
        .collect()
}

fn load_sprite_texturefiles(path_cfg: &PathConfig) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for interface_dir in path_cfg.find_all("interface") {
        let Ok(entries) = std::fs::read_dir(interface_dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let is_gfx = path
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.eq_ignore_ascii_case("gfx"))
                .unwrap_or(false);
            if !is_gfx {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(path) else {
                continue;
            };
            parse_sprite_texturefiles_into(&text, &mut out);
        }
    }
    out
}

fn parse_sprite_texturefiles_into(text: &str, out: &mut HashMap<String, String>) {
    let mut in_gfx_resource = false;
    let mut depth = 0i32;
    let mut name: Option<String> = None;
    let mut texturefile: Option<String> = None;

    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if !in_gfx_resource && is_texture_backed_gfx_resource_start(line) {
            in_gfx_resource = true;
            depth = 0;
            name = None;
            texturefile = None;
        }
        if !in_gfx_resource {
            continue;
        }

        if let Some(value) = parse_gfx_assignment(line, "name") {
            name = Some(value);
        } else if texturefile.is_none() {
            if let Some(value) = parse_gfx_texturefile_assignment(line) {
                texturefile = Some(normalize_texture_file_path(&value));
            }
        }

        depth += line.matches('{').count() as i32;
        depth -= line.matches('}').count() as i32;
        if depth <= 0 && line.contains('}') {
            if let (Some(name), Some(texturefile)) = (name.take(), texturefile.take()) {
                out.insert(name, texturefile);
            }
            in_gfx_resource = false;
        }
    }
}

fn parse_gfx_assignment(line: &str, key: &str) -> Option<String> {
    let (line_key, rest) = line.split_once('=')?;
    if !line_key.trim().eq_ignore_ascii_case(key) {
        return None;
    }
    let rest = rest.trim_start();
    Some(rest.trim_matches('"').to_owned())
}

fn parse_gfx_texturefile_assignment(line: &str) -> Option<String> {
    let (line_key, rest) = line.split_once('=')?;
    if !line_key
        .trim()
        .to_ascii_lowercase()
        .starts_with("texturefile")
    {
        return None;
    }
    Some(rest.trim_start().trim_matches('"').to_owned())
}

fn normalize_texture_file_path(value: &str) -> String {
    let replaced = value.replace('\\', "/");
    let mut out = String::with_capacity(replaced.len());
    let mut prev_slash = false;
    for ch in replaced.chars() {
        if ch == '/' {
            if !prev_slash {
                out.push(ch);
            }
            prev_slash = true;
        } else {
            out.push(ch);
            prev_slash = false;
        }
    }
    out
}

fn is_texture_backed_gfx_resource_start(line: &str) -> bool {
    let Some((key, _)) = line.split_once('=') else {
        return false;
    };
    matches!(
        key.trim().to_ascii_lowercase().as_str(),
        "spritetype"
            | "corneredtilespritetype"
            | "frameanimatedspritetype"
            | "progressbartype"
            | "textspritetype"
            | "maskedshieldtype"
    )
}

/// 工具：把 `TextureHandle` 包成 `egui::Image`（自动取贴图尺寸）。caller 之后
/// 可以再链 `.max_size(...)` / `.tint(...)` 等。
pub fn image_from_handle(handle: &TextureHandle) -> egui::Image<'_> {
    egui::Image::from_texture(handle)
}

/// 在 `ui` 上画一张 icon。默认按贴图原尺寸；caller 可传 `Some(size)` 指定。
/// 加载失败返回 `None`（caller 负责 fallback，比如画一个 "?" 占位）。
pub fn show_icon(
    ui: &mut egui::Ui,
    bank: &mut IconBank,
    gfx_name: &str,
    fit_size: Option<Vec2>,
) -> Option<egui::Response> {
    let handle = bank.get_or_load(gfx_name)?;
    let mut img = egui::Image::from_texture(handle);
    if let Some(size) = fit_size {
        img = img.fit_to_exact_size(size);
    }
    Some(ui.add(img))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 字段连通：默认 search_dirs 仅 ["gfx/interface/goals"]。
    #[test]
    fn default_search_dirs() {
        // 不构造真实 ctx / path_cfg —— 只校验 Vec 形状。
        let dirs: Vec<String> = vec!["gfx/interface/goals".to_owned()];
        assert_eq!(dirs.len(), 1);
        assert_eq!(dirs[0], "gfx/interface/goals");
    }

    /// strip `GFX_` 前缀的命名约定。
    #[test]
    fn strip_gfx_prefix() {
        let name = "GFX_focus_GER_anschluss";
        assert_eq!(name.strip_prefix("GFX_"), Some("focus_GER_anschluss"));
        let name2 = "focus_GER_anschluss"; // 已经是 stem
        assert_eq!(
            name2.strip_prefix("GFX_").unwrap_or(name2),
            "focus_GER_anschluss"
        );
    }

    #[test]
    fn event_picture_lookup_uses_fast_names() {
        let names: Vec<String> = icon_lookup_names("GFX_event_mustard_gas")
            .into_iter()
            .map(Cow::into_owned)
            .collect();

        assert_eq!(names[0], "GFX_event_mustard_gas");
        assert!(names.contains(&"GFX_news_event_ETH_ethiopian_warriors".to_owned()));
        assert!(names.contains(&"GFX_news_event_mustard_gas".to_owned()));
        assert!(names.contains(&"GFX_report_event_mustard_gas".to_owned()));
    }

    #[test]
    fn focus_icon_lookup_uses_goal_aliases_for_missing_generic_icons() {
        let names: Vec<String> = icon_lookup_names("GFX_focus_generic_industry")
            .into_iter()
            .map(Cow::into_owned)
            .collect();

        assert_eq!(names[0], "GFX_focus_generic_industry");
        assert!(names.contains(&"GFX_goal_generic_production".to_owned()));
    }

    #[test]
    fn event_picture_fallback_paths_are_bounded() {
        assert!(is_event_picture_gfx("GFX_event_mustard_gas"));
        assert!(is_event_picture_gfx(
            "GFX_news_event_ETH_ethiopian_warriors"
        ));
        assert!(is_event_picture_gfx(
            "GFX_report_event_generic_italian_celebration"
        ));
        assert!(!is_event_picture_gfx("GFX_focus_GER_anschluss"));

        assert_eq!(
            event_picture_fallback_rel_paths("event_mustard_gas"),
            [
                "gfx/event_pictures/event_mustard_gas.dds".to_owned(),
                "gfx/interface/event_mustard_gas.dds".to_owned()
            ]
        );
    }

    /// J.1.4：parse_leader_gfx_name 正例 / 负例。
    #[test]
    fn parse_leader_gfx_name_basic() {
        let (tag, parts) = parse_leader_gfx_name("portrait_GER_adolf_hitler").unwrap();
        assert_eq!(tag, "GER");
        assert_eq!(parts, vec!["adolfhitler".to_owned()]);

        let (tag, parts) = parse_leader_gfx_name("portrait_SOV_iosif_stalin").unwrap();
        assert_eq!(tag, "SOV");
        assert_eq!(parts, vec!["iosifstalin".to_owned()]);

        // 非 portrait_* → None
        assert!(parse_leader_gfx_name("focus_GER_anschluss").is_none());
        // 缺少 name → None
        assert!(parse_leader_gfx_name("portrait_GER_").is_none());
        // 缺少 tag → None
        assert!(parse_leader_gfx_name("portrait_").is_none());
    }

    #[test]
    fn fuzzy_leader_match_uses_name_tokens() {
        let dir = std::env::temp_dir().join(format!(
            "ironheart-leader-portrait-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("Portrait_Britain_Stanley_Baldwin.dds");
        std::fs::write(&file, []).unwrap();

        let found = find_matching_leader_portrait_in_dir(&dir, &["stanleybaldwin".to_owned()]);

        assert_eq!(found, Some(file));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn sprite_texturefile_parser_reads_unquoted_names() {
        let mut out = HashMap::new();
        parse_sprite_texturefiles_into(
            r#"
spriteTypes = {
    spriteType = {
        name = GFX_portrait_FRA_pierre_laval
        texturefile = "gfx/leaders/FRA/Portrait_France_Pierre_Laval.dds"
    }
    spriteType = {
        name = GFX_portrait_SOV_iosif_stalin
        texturefile = "gfx/leaders/SOV/Portrait_Soviet_Joseph_Stalin.dds"
    }
}
"#,
            &mut out,
        );

        assert_eq!(
            out.get("GFX_portrait_FRA_pierre_laval").map(String::as_str),
            Some("gfx/leaders/FRA/Portrait_France_Pierre_Laval.dds")
        );
        assert_eq!(
            out.get("GFX_portrait_SOV_iosif_stalin").map(String::as_str),
            Some("gfx/leaders/SOV/Portrait_Soviet_Joseph_Stalin.dds")
        );
    }

    #[test]
    fn gfx_texturefile_parser_reads_cornered_tiles_and_mixed_case_keys() {
        let mut out = HashMap::new();
        parse_sprite_texturefiles_into(
            r#"
spriteTypes = {
    corneredTileSpriteType = {
        name = "GFX_event_report_tileable_midsection"
        textureFile = "gfx/interface/event_report_tileable_midsection.dds"
    }
    frameAnimatedSpriteType = {
        name = "GFX_event_operative_background"
        texturefile = "gfx/interface/events/event_operative_bg.dds"
    }
    maskedShieldType = {
        name = "GFX_player_flag"
        textureFile1 = "gfx/interface/flag_overlay.dds"
        textureFile2 = "gfx/interface/shield_mask.tga"
    }
}
"#,
            &mut out,
        );

        assert_eq!(
            out.get("GFX_event_report_tileable_midsection")
                .map(String::as_str),
            Some("gfx/interface/event_report_tileable_midsection.dds")
        );
        assert_eq!(
            out.get("GFX_event_operative_background")
                .map(String::as_str),
            Some("gfx/interface/events/event_operative_bg.dds")
        );
        assert_eq!(
            out.get("GFX_player_flag").map(String::as_str),
            Some("gfx/interface/flag_overlay.dds")
        );
    }

    #[test]
    fn gate2_texturefile_parser_normalizes_duplicate_separators() {
        assert_eq!(
            normalize_texture_file_path(r#"gfx\\interface//pol_view_bg.dds"#),
            "gfx/interface/pol_view_bg.dds"
        );
    }

    #[test]
    fn sprite_probe_report_explains_missing_paths() {
        let Ok(path_cfg) = hoi4_paths::PathConfig::resolve(Default::default()) else {
            return;
        };
        let ctx = egui::Context::default();
        let mut bank = IconBank::new(ctx, path_cfg);
        bank.add_politics_search_dirs();

        let report = bank.diagnose_sprite("GFX_missing_probe_for_test");

        assert_eq!(report.sprite_name, "GFX_missing_probe_for_test");
        assert!(!report.loaded);
        assert!(report.failure_reason.is_some());
        assert!(report
            .attempted_paths
            .iter()
            .any(|path| path.contains("missing_probe_for_test.dds")));
    }

    #[test]
    fn profile_search_dirs_cover_decisions_and_focuses() {
        let path_cfg = hoi4_paths::PathConfig::with_game_path(std::env::temp_dir());
        let ctx = egui::Context::default();
        let mut bank = IconBank::new(ctx, path_cfg);

        bank.add_profile_search_dirs(crate::vanilla_gui::COUNTRY_DECISION_PROFILE_ID);
        bank.add_profile_search_dirs(crate::vanilla_gui::NATIONAL_FOCUS_PROFILE_ID);
        bank.add_profile_search_dirs(crate::vanilla_gui::COUNTRY_LOGISTICS_PROFILE_ID);
        bank.add_profile_search_dirs(crate::vanilla_gui::COUNTRY_DIPLOMACY_PROFILE_ID);
        bank.add_profile_search_dirs(crate::vanilla_gui::COUNTRY_DECISION_PROFILE_ID);
        bank.add_profile_search_dirs(crate::vanilla_gui::COUNTRY_DIPLOMACY_PROFILE_ID);

        let dirs = bank.search_dirs();
        assert!(dirs.iter().any(|dir| dir == "gfx/interface/decisions"));
        assert!(dirs.iter().any(|dir| dir == "gfx/interface/focusview"));
        assert!(dirs
            .iter()
            .any(|dir| dir == "gfx/interface/focusview/titlebar"));
        assert!(dirs.iter().any(|dir| dir == "gfx/interface/techtree"));
        assert!(dirs.iter().any(|dir| dir == "gfx/interface/goals"));
        assert!(dirs.iter().any(|dir| dir == "gfx/interface/archetypes"));
        assert!(dirs.iter().any(|dir| dir == "gfx/interface/technologies"));
        assert!(dirs.iter().any(|dir| dir == "gfx/interface/diplomacy"));
        assert_eq!(
            dirs.iter()
                .filter(|dir| dir.as_str() == "gfx/interface/decisions")
                .count(),
            1
        );
        assert_eq!(
            dirs.iter()
                .filter(|dir| dir.as_str() == "gfx/interface/diplomacy")
                .count(),
            1
        );
    }

    #[test]
    fn decision_and_focus_key_sprites_are_diagnosable_when_vanilla_available() {
        let Ok(path_cfg) = hoi4_paths::PathConfig::resolve(Default::default()) else {
            return;
        };
        if path_cfg
            .find(crate::vanilla_gui::COUNTRY_DECISION_GUI_FILE)
            .is_none()
            || path_cfg
                .find(crate::vanilla_gui::NATIONAL_FOCUS_GUI_FILE)
                .is_none()
        {
            return;
        }

        let ctx = egui::Context::default();
        let mut bank = IconBank::new(ctx, path_cfg);
        bank.add_decision_search_dirs();
        bank.add_focus_search_dirs();

        for sprite in [
            "GFX_decision_item_bg",
            "GFX_decision_unknown",
            "GFX_focus_unavailable",
            "GFX_focus_can_start",
            "GFX_focus_completed",
            "GFX_goal_unknown",
        ] {
            let report = bank.diagnose_sprite(sprite);
            assert!(
                report.loaded
                    || report.texture_file.is_some()
                    || !report.attempted_paths.is_empty(),
                "sprite should produce a useful diagnostic: {report:?}"
            );
        }
    }

    #[test]
    fn vanilla_sprite_texturefiles_include_reported_leaders_when_available() {
        let Ok(path_cfg) = hoi4_paths::PathConfig::resolve(Default::default()) else {
            return;
        };
        let sprites = load_sprite_texturefiles(&path_cfg);

        assert_eq!(
            sprites
                .get("GFX_portrait_SOV_iosif_stalin")
                .map(String::as_str),
            Some("gfx/leaders/SOV/Portrait_Soviet_Joseph_Stalin.dds")
        );
        assert!(
            sprites.contains_key("GFX_portrait_fra_pierre_laval"),
            "vanilla .gfx should map France Laval portrait"
        );
    }
}
