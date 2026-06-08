//! `hoi4-paths` — Phase 0.1
//!
//! 路径与资源根。把"HOI4 安装目录在哪里"和"mod 链怎么排"集中到一个 crate，
//! 避免散在 11 个文件里硬编码 `C:/Program Files (x86)/Steam/...`。
//!
//! 许可边界：这里的路径解析只用于运行时读取用户本机合法安装目录中的 vanilla
//! 文件。项目代码只能保存相对路径、元数据和诊断信息；不得把 Paradox / HOI4
//! 原版 `.dds`、`.gui`、`.gfx`、字体或美术资源复制进仓库或随项目分发。
//!
//! ## 优先级（高→低）
//! 1. CLI `--game-path <PATH>`（由 `hoi4-app` 命令行解析后传入 [`PathConfig::from_overrides`]）
//! 2. 环境变量 `IRONHEART_HOI4_PATH`
//! 3. 用户配置文件 `~/.config/ironheart/config.toml`（Linux/macOS）/
//!    `%APPDATA%/ironheart/config.toml`（Windows）下的 `game_path = "..."`
//! 4. 平台 Steam 自动探测：
//!    - Windows：`C:/Program Files (x86)/Steam/steamapps/common/Hearts of Iron IV`
//!    - macOS：`~/Library/Application Support/Steam/steamapps/common/Hearts of Iron IV`
//!    - Linux：`~/.steam/steam/steamapps/common/Hearts of Iron IV`
//!
//! ## Mod 链
//! [`PathConfig::mod_chain`] 是一个 `Vec<PathBuf>`，按"高优先级在前"排列。
//! [`PathConfig::find`] 会沿 mod 链回退到 vanilla：先在每个 mod 根下找 `relative`，
//! 没找到再回 vanilla。`replace_path` 指令使该目录在 mod 启用时**屏蔽** vanilla 内容。
//!
//! ## 测试用 fixture
//! 单元测试通过 [`PathConfig::with_game_path`] 构造一个指向临时目录的实例，
//! 不需要真实安装。

use std::path::{Path, PathBuf};

/// 解析 + 路径查找的统一入口。
///
/// 由 `hoi4-app` 在启动时构造一次，之后所有需要"游戏目录"的代码都从这里取。
#[derive(Debug, Clone)]
pub struct PathConfig {
    /// vanilla 安装根（`Hearts of Iron IV` 目录本身）
    game_path: PathBuf,
    /// mod 链（高优先级在前，vanilla 不出现在这里）
    mod_chain: Vec<ModEntry>,
    /// Installed DLC roots, ordered high priority first.
    dlc_roots: Vec<PathBuf>,
    /// 来源（用于诊断 / 启动 log）
    source: PathSource,
}

/// 单个 mod 在路径解析中的元信息。
#[derive(Debug, Clone)]
pub struct ModEntry {
    pub name: String,
    pub root: PathBuf,
    /// `replace_path = "..."` 列表。出现在这里的子目录在 mod 启用时**完全替换** vanilla。
    pub replace_paths: Vec<PathBuf>,
}

/// 路径解析来源。仅用于诊断输出。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathSource {
    Cli,
    Env,
    ConfigFile,
    SteamAuto,
    /// 单元测试 / fixture。
    Manual,
}

/// 解析失败的原因。
#[derive(Debug, Clone)]
pub enum PathError {
    /// 所有候选路径都不存在或不是目录。
    NotFound { tried: Vec<PathBuf> },
    /// 显式提供了路径，但目录不存在。
    InvalidOverride { path: PathBuf, reason: &'static str },
}

impl std::fmt::Display for PathError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound { tried } => {
                writeln!(f, "无法定位 HOI4 安装目录。已尝试：")?;
                for p in tried {
                    writeln!(f, "  - {}", p.display())?;
                }
                writeln!(
                    f,
                    "请通过 --game-path、IRONHEART_HOI4_PATH 环境变量，\
                     或 ~/.config/ironheart/config.toml 指定。"
                )
            }
            Self::InvalidOverride { path, reason } => {
                write!(f, "指定的 game-path `{}` 无效：{}", path.display(), reason)
            }
        }
    }
}

impl std::error::Error for PathError {}

/// 解析时的输入参数。`hoi4-app` 解析完命令行后填这个 struct。
#[derive(Debug, Default, Clone)]
pub struct ResolveOverrides {
    pub cli_game_path: Option<PathBuf>,
    pub cli_mods: Vec<PathBuf>,
}

impl PathConfig {
    /// 主入口：按 CLI > env > config > Steam 顺序解析。
    ///
    /// 不读 mod descriptor。mod 链可由 [`PathConfig::with_mods`] 追加。
    pub fn resolve(overrides: ResolveOverrides) -> Result<Self, PathError> {
        // 1. CLI 显式
        if let Some(p) = overrides.cli_game_path.clone() {
            return validate_game_path(&p)
                .map(|game_path| Self::with_source(game_path, PathSource::Cli))
                .map_err(|reason| PathError::InvalidOverride { path: p, reason });
        }

        let mut tried = Vec::new();

        // 2. env
        if let Ok(env_p) = std::env::var("IRONHEART_HOI4_PATH") {
            let env_path = PathBuf::from(&env_p);
            tried.push(env_path.clone());
            if validate_game_path(&env_path).is_ok() {
                return Ok(Self::with_source(env_path, PathSource::Env));
            }
        }

        // 3. user config file
        if let Some(cfg_path) = read_config_game_path() {
            tried.push(cfg_path.clone());
            if validate_game_path(&cfg_path).is_ok() {
                return Ok(Self::with_source(cfg_path, PathSource::ConfigFile));
            }
        }

        // 4. Steam auto-detect
        for candidate in steam_candidates() {
            tried.push(candidate.clone());
            if validate_game_path(&candidate).is_ok() {
                return Ok(Self::with_source(candidate, PathSource::SteamAuto));
            }
        }

        Err(PathError::NotFound { tried })
    }

    /// 测试 / fixture 用：跳过任何探测，直接绑定到给定目录。
    pub fn with_game_path(game_path: impl Into<PathBuf>) -> Self {
        Self::with_source(game_path.into(), PathSource::Manual)
    }

    fn with_source(game_path: PathBuf, source: PathSource) -> Self {
        let dlc_roots = discover_dlc_roots(&game_path);
        Self {
            game_path,
            mod_chain: Vec::new(),
            dlc_roots,
            source,
        }
    }

    /// 追加 mod 链（高优先级在前）。
    pub fn with_mods(mut self, mods: impl IntoIterator<Item = ModEntry>) -> Self {
        self.mod_chain.extend(mods);
        self
    }

    /// vanilla 根目录。
    pub fn game_path(&self) -> &Path {
        &self.game_path
    }

    /// mod 链（不含 vanilla）。
    pub fn mod_chain(&self) -> &[ModEntry] {
        &self.mod_chain
    }

    pub fn dlc_roots(&self) -> &[PathBuf] {
        &self.dlc_roots
    }

    pub fn source(&self) -> PathSource {
        self.source
    }

    /// 沿 mod 链回退查找资产路径。
    ///
    /// 返回 `Some(absolute_path)`：第一个存在的物理文件；按 mod 链顺序，最后回 vanilla。
    /// 返回 `None`：所有候选都不存在。
    ///
    /// `relative` 应为 forward-slash 路径（例：`"interface/topbar.gui"`），
    /// 内部会用 [`Path::join`] 适配平台。
    pub fn find(&self, relative: impl AsRef<Path>) -> Option<PathBuf> {
        let relative = relative.as_ref();

        // mod 链顺序优先
        for m in &self.mod_chain {
            let p = m.root.join(relative);
            if p.exists() {
                return Some(p);
            }
        }

        // 检查 replace_path 是否阻塞所有低优先级官方内容
        if self.is_replaced_by_mod(relative) {
            return None;
        }

        for root in &self.dlc_roots {
            let p = root.join(relative);
            if p.exists() {
                return Some(p);
            }
        }

        let p = self.game_path.join(relative);
        if p.exists() {
            Some(p)
        } else {
            None
        }
    }

    /// 沿 mod 链收集**所有**匹配的物理路径（用于"加载某目录下全部文件"场景，例如所有
    /// `events/*.txt`）。Mod 优先；vanilla 在最后；命中 `replace_path` 时不收 vanilla。
    pub fn find_all(&self, relative: impl AsRef<Path>) -> Vec<PathBuf> {
        let relative = relative.as_ref();
        let mut out = Vec::new();
        for m in &self.mod_chain {
            let p = m.root.join(relative);
            if p.exists() {
                out.push(p);
            }
        }
        if !self.is_replaced_by_mod(relative) {
            for root in &self.dlc_roots {
                let p = root.join(relative);
                if p.exists() {
                    out.push(p);
                }
            }
            let p = self.game_path.join(relative);
            if p.exists() {
                out.push(p);
            }
        }
        out
    }

    fn is_replaced_by_mod(&self, relative: &Path) -> bool {
        for m in &self.mod_chain {
            for rp in &m.replace_paths {
                if path_starts_with(relative, rp) {
                    return true;
                }
            }
        }
        false
    }
}

fn path_starts_with(child: &Path, prefix: &Path) -> bool {
    let mut c = child.components();
    for pc in prefix.components() {
        match c.next() {
            Some(cc) if cc == pc => {}
            _ => return false,
        }
    }
    true
}

/// 一个目录是否"长得像"HOI4 安装根：必须有 `map/` 与 `common/`。
fn validate_game_path(p: &Path) -> Result<PathBuf, &'static str> {
    if !p.is_dir() {
        return Err("不是目录");
    }
    if !p.join("map").is_dir() {
        return Err("缺少 map/ 子目录");
    }
    if !p.join("common").is_dir() {
        return Err("缺少 common/ 子目录");
    }
    Ok(p.to_path_buf())
}

fn steam_candidates() -> Vec<PathBuf> {
    let mut v = Vec::new();
    if cfg!(target_os = "windows") {
        v.push(PathBuf::from(
            r"C:\Program Files (x86)\Steam\steamapps\common\Hearts of Iron IV",
        ));
        v.push(PathBuf::from(
            r"C:\Program Files\Steam\steamapps\common\Hearts of Iron IV",
        ));
        v.push(PathBuf::from(
            r"D:\SteamLibrary\steamapps\common\Hearts of Iron IV",
        ));
        v.push(PathBuf::from(
            r"E:\SteamLibrary\steamapps\common\Hearts of Iron IV",
        ));
    } else if cfg!(target_os = "macos") {
        if let Some(home) = home_dir() {
            v.push(
                home.join("Library/Application Support/Steam/steamapps/common/Hearts of Iron IV"),
            );
        }
    } else {
        // linux + others
        if let Some(home) = home_dir() {
            v.push(home.join(".steam/steam/steamapps/common/Hearts of Iron IV"));
            v.push(home.join(".local/share/Steam/steamapps/common/Hearts of Iron IV"));
        }
    }
    v
}

fn discover_dlc_roots(game_path: &Path) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    for container in ["dlc", "integrated_dlc"] {
        let dir = game_path.join(container);
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut children: Vec<PathBuf> = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.is_dir())
            .filter(|path| {
                path.join("gfx").is_dir()
                    || path.join("common").is_dir()
                    || path.join("interface").is_dir()
                    || path.join("history").is_dir()
                    || path.join("events").is_dir()
            })
            .collect();
        children.sort_by(|a, b| {
            b.file_name()
                .unwrap_or_default()
                .cmp(a.file_name().unwrap_or_default())
        });
        roots.extend(children);
    }
    roots
}

fn home_dir() -> Option<PathBuf> {
    if cfg!(target_os = "windows") {
        std::env::var_os("USERPROFILE").map(PathBuf::from)
    } else {
        std::env::var_os("HOME").map(PathBuf::from)
    }
}

fn config_file_path() -> Option<PathBuf> {
    if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join("ironheart").join("config.toml"))
    } else {
        home_dir().map(|h| h.join(".config").join("ironheart").join("config.toml"))
    }
}

/// 读取 `config.toml` 中的 `game_path` 字段（如果存在）。
///
/// 不引入 `toml` crate；用最小手解：找形如 `game_path = "..."` 的行。
fn read_config_game_path() -> Option<PathBuf> {
    let cfg = config_file_path()?;
    let text = std::fs::read_to_string(&cfg).ok()?;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("game_path") {
            // game_path = "..." 或 game_path="..."
            let rest = rest.trim_start();
            let rest = rest.strip_prefix('=')?.trim_start();
            let inner = rest.strip_prefix('"')?;
            let end = inner.find('"')?;
            return Some(PathBuf::from(&inner[..end]));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn make_fake_install(dir: &Path) {
        fs::create_dir_all(dir.join("map")).unwrap();
        fs::create_dir_all(dir.join("common")).unwrap();
    }

    #[test]
    fn validate_requires_map_and_common() {
        let tmp = std::env::temp_dir().join("ironheart_paths_test_validate");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        assert!(validate_game_path(&tmp).is_err());
        make_fake_install(&tmp);
        assert!(validate_game_path(&tmp).is_ok());
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn find_falls_back_to_vanilla() {
        let tmp = std::env::temp_dir().join("ironheart_paths_find");
        let _ = fs::remove_dir_all(&tmp);
        let game = tmp.join("game");
        let mod_a = tmp.join("mod_a");
        make_fake_install(&game);
        fs::create_dir_all(mod_a.join("interface")).unwrap();
        fs::write(game.join("map/foo.txt"), "vanilla").unwrap();
        fs::write(mod_a.join("interface/bar.gui"), "mod").unwrap();

        let cfg = PathConfig::with_game_path(&game).with_mods(vec![ModEntry {
            name: "mod_a".into(),
            root: mod_a.clone(),
            replace_paths: vec![],
        }]);

        // mod-only 文件
        assert_eq!(
            cfg.find("interface/bar.gui").unwrap(),
            mod_a.join("interface/bar.gui")
        );
        // vanilla-only 文件
        assert_eq!(cfg.find("map/foo.txt").unwrap(), game.join("map/foo.txt"));
        // 不存在
        assert!(cfg.find("does/not/exist").is_none());

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn find_checks_installed_dlc_before_vanilla() {
        let tmp = std::env::temp_dir().join("ironheart_paths_dlc");
        let _ = fs::remove_dir_all(&tmp);
        let game = tmp.join("game");
        make_fake_install(&game);
        let dlc_root = game.join("integrated_dlc/dlc999_test");
        fs::create_dir_all(dlc_root.join("gfx/entities")).unwrap();
        fs::create_dir_all(game.join("gfx/entities")).unwrap();
        fs::write(game.join("gfx/entities/buildings.gfx"), "vanilla").unwrap();
        fs::write(dlc_root.join("gfx/entities/buildings.gfx"), "dlc").unwrap();

        let cfg = PathConfig::with_game_path(&game);

        assert_eq!(
            cfg.find("gfx/entities/buildings.gfx").unwrap(),
            dlc_root.join("gfx/entities/buildings.gfx")
        );
        assert_eq!(cfg.dlc_roots().len(), 1);

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn replace_path_blocks_vanilla() {
        let tmp = std::env::temp_dir().join("ironheart_paths_replace");
        let _ = fs::remove_dir_all(&tmp);
        let game = tmp.join("game");
        let mod_a = tmp.join("mod_a");
        make_fake_install(&game);
        fs::create_dir_all(game.join("history/units")).unwrap();
        fs::create_dir_all(mod_a.join("history/units")).unwrap();
        fs::write(game.join("history/units/GER_1936.txt"), "vanilla").unwrap();
        fs::write(mod_a.join("history/units/SOV_1936.txt"), "modded").unwrap();

        let cfg = PathConfig::with_game_path(&game).with_mods(vec![ModEntry {
            name: "tno".into(),
            root: mod_a.clone(),
            replace_paths: vec![PathBuf::from("history/units")],
        }]);

        // SOV 来自 mod
        assert_eq!(
            cfg.find("history/units/SOV_1936.txt").unwrap(),
            mod_a.join("history/units/SOV_1936.txt")
        );
        // GER 不再走 vanilla：被 replace_path 屏蔽，结果是 None
        assert!(cfg.find("history/units/GER_1936.txt").is_none());

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn find_all_collects_mod_then_vanilla() {
        let tmp = std::env::temp_dir().join("ironheart_paths_findall");
        let _ = fs::remove_dir_all(&tmp);
        let game = tmp.join("game");
        let mod_a = tmp.join("mod_a");
        make_fake_install(&game);
        fs::create_dir_all(mod_a.join("interface")).unwrap();
        fs::write(game.join("map/foo.txt"), "v").unwrap();
        fs::write(mod_a.join("interface/extra.gui"), "m").unwrap();

        let cfg = PathConfig::with_game_path(&game).with_mods(vec![ModEntry {
            name: "x".into(),
            root: mod_a.clone(),
            replace_paths: vec![],
        }]);

        let multi = cfg.find_all("map/foo.txt");
        assert_eq!(multi, vec![game.join("map/foo.txt")]);
        let multi = cfg.find_all("interface/extra.gui");
        assert_eq!(multi, vec![mod_a.join("interface/extra.gui")]);

        let _ = fs::remove_dir_all(&tmp);
    }
}
