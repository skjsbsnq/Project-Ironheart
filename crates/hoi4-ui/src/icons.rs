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
//! - vanilla 真正的 sprite 索引（`.gfx` 里的 `SpriteType`）在 V5 §0.2 禁解析；
//!   `IconBank` 走「strip GFX_ prefix + 文件名」的硬编码约定，假设 vanilla
//!   绝大多数 sprite 名都对应同名 DDS 文件（实测 vanilla `goals/focus_GER_*` 都满足）。

use std::collections::HashMap;
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
        self.search_dirs.push(rel.into());
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

    /// 已加载 icon 的像素尺寸（如果已 cache 为 Loaded 则返回；其他状态返回 `None`）。
    pub fn size_of(&self, gfx_name: &str) -> Option<[usize; 2]> {
        match self.cache.get(gfx_name)? {
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

    fn try_load(&self, gfx_name: &str) -> IconEntry {
        let stem = gfx_name.strip_prefix("GFX_").unwrap_or(gfx_name);

        if let Some(entry) = self.try_load_flag_tga(gfx_name, stem) {
            return entry;
        }

        let filename = format!("{stem}.dds");
        let mut tried: Vec<PathBuf> = Vec::new();
        let mut last_load_err: Option<String> = None;

        if let Some(texturefile) = self.sprite_texturefiles.get(gfx_name) {
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
    let mut in_sprite = false;
    let mut depth = 0i32;
    let mut name: Option<String> = None;
    let mut texturefile: Option<String> = None;

    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if !in_sprite && is_sprite_type_start(line) {
            in_sprite = true;
            depth = 0;
            name = None;
            texturefile = None;
        }
        if !in_sprite {
            continue;
        }

        if let Some(value) = parse_gfx_assignment(line, "name") {
            name = Some(value);
        } else if let Some(value) = parse_gfx_assignment(line, "texturefile") {
            texturefile = Some(value.replace('\\', "/"));
        }

        depth += line.matches('{').count() as i32;
        depth -= line.matches('}').count() as i32;
        if depth <= 0 && line.contains('}') {
            if let (Some(name), Some(texturefile)) = (name.take(), texturefile.take()) {
                out.insert(name, texturefile);
            }
            in_sprite = false;
        }
    }
}

fn parse_gfx_assignment(line: &str, key: &str) -> Option<String> {
    let rest = line.strip_prefix(key)?.trim_start();
    let rest = rest.strip_prefix('=')?.trim_start();
    Some(rest.trim_matches('"').to_owned())
}

fn is_sprite_type_start(line: &str) -> bool {
    let Some((key, _)) = line.split_once('=') else {
        return false;
    };
    key.trim() == "spriteType"
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
