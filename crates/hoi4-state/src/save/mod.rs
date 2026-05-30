//! 存档系统：负责把 [`World`] 写入磁盘并恢复回来。
//!
//! 提供两种格式：
//! 1. **HOI4 兼容文本格式** ([`text`])
//!    - 基于 Clausewitz Script 语法（人类可读）
//!    - 头部为 `HOI4txt` 魔术字，主要字段名/层级与原版一致
//!    - 用于与原版游戏交换数据 / debug
//! 2. **快速二进制格式** ([`binary`])
//!    - 自定义紧凑布局，按 SoA store 直接写入
//!    - 用于游戏内 quick-save / autosave，体积小、速度快
//!
//! 公共入口：
//! - [`read`] / [`write`]            — 自动按文件后缀或文件头分发
//! - [`read_text`] / [`write_text`]  — 显式文本格式
//! - [`read_binary`] / [`write_binary`] — 显式二进制格式
//! - [`read_meta`]                   — 仅读取存档头部元数据（用于存档列表）

use std::fs;
use std::io;
use std::path::Path;

use crate::time::GameDate;
use crate::World;

pub mod binary;
pub mod text;

/// 引擎写入存档时使用的版本号（与 HOI4 引擎版本无关，标识我们自己的格式）
pub const ENGINE_VERSION: &str = "Ironheart 0.1.0";

/// 二进制存档魔术字（"IRHRT" + 三字节版本）
pub const BINARY_MAGIC: &[u8; 8] = b"IRHRT001";

/// 文本存档魔术字（首行）
pub const TEXT_MAGIC: &str = "HOI4txt";

/// 存档格式标识
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveKind {
    /// HOI4 兼容文本格式
    Text,
    /// 自定义快速二进制格式
    Binary,
}

/// 存档头部元数据（不需要加载完整 World 即可读取）
#[derive(Debug, Clone)]
pub struct SaveMeta {
    pub kind: SaveKind,
    pub version: String,
    pub date: GameDate,
    pub player_tag: String,
    pub random_seed: u64,
    pub game_unique_id: u64,
    pub elapsed_hours: u64,
    pub ironman: bool,
}

impl SaveMeta {
    /// 从 World 抓取元数据（用于写入）
    pub fn from_world(world: &World) -> Self {
        let player_tag = if world.player.is_none() {
            String::new()
        } else {
            world.country_tag(world.player).unwrap_or("").to_owned()
        };
        Self {
            kind: SaveKind::Text,
            version: ENGINE_VERSION.to_owned(),
            date: world.date,
            player_tag,
            random_seed: world.random_seed,
            game_unique_id: world.game_unique_id,
            elapsed_hours: world.elapsed_hours,
            ironman: false,
        }
    }
}

/// 存档错误
#[derive(Debug)]
pub enum SaveError {
    Io(io::Error),
    /// 文件头不识别
    BadMagic(String),
    /// 解析阶段错误（缺字段、类型不对等）
    Parse(String),
    /// 国家 tag 在 World.data 中找不到
    UnknownCountry(String),
    /// 存档格式版本不兼容
    UnsupportedVersion(String),
    /// 数据损坏（二进制读到结尾或长度异常）
    Corrupt(String),
}

impl std::fmt::Display for SaveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "save IO error: {}", e),
            Self::BadMagic(s) => write!(f, "bad save magic: {}", s),
            Self::Parse(s) => write!(f, "save parse error: {}", s),
            Self::UnknownCountry(s) => write!(f, "unknown country tag in save: {}", s),
            Self::UnsupportedVersion(s) => write!(f, "unsupported save version: {}", s),
            Self::Corrupt(s) => write!(f, "corrupt save data: {}", s),
        }
    }
}

impl std::error::Error for SaveError {}

impl From<io::Error> for SaveError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

pub type SaveResult<T> = Result<T, SaveError>;

// ─── 高层 API ───────────────────────────────────────────────────────

/// 嗅探文件头，判断格式
pub fn detect_kind(bytes: &[u8]) -> SaveKind {
    if bytes.starts_with(BINARY_MAGIC) {
        SaveKind::Binary
    } else {
        SaveKind::Text
    }
}

/// 自动按文件头读取存档（修改 world 状态）
pub fn read<P: AsRef<Path>>(path: P, world: &mut World) -> SaveResult<()> {
    let bytes = fs::read(path.as_ref())?;
    match detect_kind(&bytes) {
        SaveKind::Binary => binary::read_bytes(&bytes, world),
        SaveKind::Text => {
            let s = std::str::from_utf8(&bytes)
                .map_err(|e| SaveError::Parse(format!("text save is not valid UTF-8: {}", e)))?;
            text::read_str(s, world)
        }
    }
}

/// 自动按文件后缀写入存档。`.bin` → 二进制；其他 → 文本
pub fn write<P: AsRef<Path>>(path: P, world: &World) -> SaveResult<()> {
    let p = path.as_ref();
    let is_binary = p
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("bin"))
        .unwrap_or(false);
    if is_binary {
        write_binary(p, world)
    } else {
        write_text(p, world)
    }
}

/// 仅读取头部元数据（不解析全部内容）
pub fn read_meta<P: AsRef<Path>>(path: P) -> SaveResult<SaveMeta> {
    let bytes = fs::read(path.as_ref())?;
    match detect_kind(&bytes) {
        SaveKind::Binary => binary::read_meta_bytes(&bytes),
        SaveKind::Text => {
            let s = std::str::from_utf8(&bytes)
                .map_err(|e| SaveError::Parse(format!("text save is not valid UTF-8: {}", e)))?;
            text::read_meta_str(s)
        }
    }
}

// ─── 文本格式 wrappers ──────────────────────────────────────────────

pub fn read_text<P: AsRef<Path>>(path: P, world: &mut World) -> SaveResult<()> {
    let s = fs::read_to_string(path.as_ref())?;
    text::read_str(&s, world)
}

pub fn write_text<P: AsRef<Path>>(path: P, world: &World) -> SaveResult<()> {
    let s = text::write_string(world);
    fs::write(path.as_ref(), s.as_bytes())?;
    Ok(())
}

// ─── 二进制格式 wrappers ────────────────────────────────────────────

pub fn read_binary<P: AsRef<Path>>(path: P, world: &mut World) -> SaveResult<()> {
    let bytes = fs::read(path.as_ref())?;
    binary::read_bytes(&bytes, world)
}

pub fn write_binary<P: AsRef<Path>>(path: P, world: &World) -> SaveResult<()> {
    let bytes = binary::write_bytes(world);
    fs::write(path.as_ref(), bytes)?;
    Ok(())
}
