//! [`AssetDb`] trait + [`FsAssetDb`] 默认实现。
//!
//! 设计点：
//! - **trait** 而非具体类型 → 测试可注入 mock；后续如果出现 in-memory zip / 远端
//!   资产源也能换实现，调用方代码不变。
//! - **缓存通过 RefCell 内部可变** → 接口保持 `&self`，调用站点不需要 `&mut`。
//! - **类型化解析缓存** 用 `Arc<dyn Any + 'static>` 存值 + `TypeId` 入键，
//!   `parse_or_get<T>` 调用安全的 downcast。
//! - **错误链**：`open_referenced(path, referrer)` 在失败时把 `referrer` 挂到错误。

use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use hoi4_paths::PathConfig;

use crate::cache::Cache;
use crate::error::AssetError;
use crate::{AssetBytes, DEFAULT_PARSED_CACHE_CAP, DEFAULT_RAW_CACHE_CAP};

/// 统一资产取数接口。
///
/// 默认实现是 [`FsAssetDb`]（文件系统 + mod 链 + LRU 缓存）。
/// 测试可以实现自己的 in-memory 版本。
pub trait AssetDb {
    /// 取相对路径资产的字节（沿 mod 链 → vanilla 回退）。
    fn open(&self, relative: impl AsRef<Path>) -> Result<AssetBytes, AssetError>;

    /// 取相对路径资产，并把"我是被谁引用的"挂到错误链上。
    /// 默认实现委托给 [`AssetDb::open`]，再在错误上挂 referrer。
    fn open_referenced(
        &self,
        relative: impl AsRef<Path>,
        referrer: impl AsRef<Path>,
    ) -> Result<AssetBytes, AssetError> {
        match self.open(relative) {
            Ok(b) => Ok(b),
            Err(e) => Err(e.with_referrer_path(referrer.as_ref().to_path_buf())),
        }
    }

    /// 列出某相对目录下所有匹配的物理路径（mod 链 + vanilla，先 mod 后 vanilla）。
    /// 直接转发到 [`PathConfig::find_all`]。
    fn list(&self, relative: impl AsRef<Path>) -> Vec<PathBuf>;

    /// 解析驻留：如果 (path, T) 已解析过，返回缓存的 `Arc<T>`；否则用 `parser`
    /// 解析当前字节，缓存并返回。`parser` 可以失败（返回 [`AssetError`]）。
    ///
    /// `T: Send + Sync + 'static`：解析结果常被多处共享（UI 树、纹理库），克隆
    /// `Arc<T>` 是 O(1)。`Send + Sync` 要求确保后续如果切多线程加载也无锁问题。
    fn parse_or_get<T, F>(
        &self,
        relative: impl AsRef<Path>,
        parser: F,
    ) -> Result<Arc<T>, AssetError>
    where
        T: Send + Sync + 'static,
        F: FnOnce(&[u8]) -> Result<T, AssetError>;
}

/// 文件系统实现。
///
/// ## 缓存
/// - **raw**：`PathBuf → AssetBytes`，避免重复 IO
/// - **parsed**：`(PathBuf, TypeId) → Arc<dyn Any + Send + Sync>`，避免重复解析
///
/// 两者都是 LRU；容量在构造时设定，超容时按访问时间淘汰。
///
/// ## 线程安全（v1 局限）
/// 内部用 [`RefCell`]。**只能从单线程调用**（典型场景：启动加载阶段、UI 主线程）。
/// 多线程加载方案：把 `RefCell<Cache<...>>` 换成 `Mutex<Cache<...>>`，trait 不变。
/// 真正进 Phase 4+ 多线程加载时再改，避免现在引入无谓的锁开销。
pub struct FsAssetDb {
    path_cfg: PathConfig,
    raw: RefCell<Cache<PathBuf, AssetBytes>>,
    parsed: RefCell<Cache<ParsedKey, Arc<dyn Any + Send + Sync>>>,
}

#[derive(Clone, Eq, PartialEq, Hash)]
struct ParsedKey {
    path: PathBuf,
    type_id: TypeId,
}

impl FsAssetDb {
    /// 用默认缓存容量构造。
    pub fn new(path_cfg: PathConfig) -> Self {
        Self::with_capacity(path_cfg, DEFAULT_RAW_CACHE_CAP, DEFAULT_PARSED_CACHE_CAP)
    }

    /// 自定义缓存容量。`raw_cap < 1` 或 `parsed_cap < 1` 会被夹到 1。
    pub fn with_capacity(path_cfg: PathConfig, raw_cap: usize, parsed_cap: usize) -> Self {
        Self {
            path_cfg,
            raw: RefCell::new(Cache::new(raw_cap)),
            parsed: RefCell::new(Cache::new(parsed_cap)),
        }
    }

    /// 取出底层 [`PathConfig`]（用于诊断 / 路径调试）。
    pub fn paths(&self) -> &PathConfig {
        &self.path_cfg
    }

    /// 缓存命中率统计：当前 raw / parsed 缓存大小（用于 F3 诊断 / 启动 log）。
    pub fn cache_sizes(&self) -> (usize, usize) {
        (self.raw.borrow().len(), self.parsed.borrow().len())
    }

    /// 清空两个缓存。mod 切换 / 热重载时调用。
    pub fn clear_caches(&self) {
        self.raw.borrow_mut().clear();
        self.parsed.borrow_mut().clear();
    }

    fn read_file(&self, abs: &Path) -> Result<AssetBytes, AssetError> {
        match std::fs::read(abs) {
            Ok(v) => Ok(Arc::from(v.into_boxed_slice())),
            Err(e) => Err(AssetError::io(abs.to_path_buf(), e)),
        }
    }
}

impl AssetDb for FsAssetDb {
    fn open(&self, relative: impl AsRef<Path>) -> Result<AssetBytes, AssetError> {
        let relative = relative.as_ref().to_path_buf();

        // 缓存命中（按相对路径键，mod 链不变期间稳定）
        if let Some(b) = self.raw.borrow_mut().get(&relative).cloned() {
            return Ok(b);
        }

        // 路径解析
        let abs = self
            .path_cfg
            .find(&relative)
            .ok_or_else(|| AssetError::not_found(relative.clone()))?;

        let bytes = self.read_file(&abs)?;
        // 写入 raw cache（淘汰旧条目，丢弃淘汰返回值）
        let _ = self.raw.borrow_mut().put(relative, bytes.clone());
        Ok(bytes)
    }

    fn list(&self, relative: impl AsRef<Path>) -> Vec<PathBuf> {
        self.path_cfg.find_all(relative.as_ref())
    }

    fn parse_or_get<T, F>(
        &self,
        relative: impl AsRef<Path>,
        parser: F,
    ) -> Result<Arc<T>, AssetError>
    where
        T: Send + Sync + 'static,
        F: FnOnce(&[u8]) -> Result<T, AssetError>,
    {
        let relative = relative.as_ref().to_path_buf();
        let key = ParsedKey {
            path: relative.clone(),
            type_id: TypeId::of::<T>(),
        };

        // 命中
        if let Some(any) = self.parsed.borrow_mut().get(&key).cloned() {
            // downcast 安全：插入时 type_id == TypeId::of::<T>()
            return any.downcast::<T>().map_err(|_| {
                AssetError::parse(relative.clone(), "downcast failed (cache type mismatch)")
            });
        }

        // 未命中：取字节、解析、入缓存
        let bytes = self.open(&relative)?;
        let parsed = parser(&bytes).map_err(|mut e| {
            // 把 parse 错误的 path 修正为相对路径（如果解析器没设过）
            if e.path.as_os_str().is_empty() {
                e.path = relative.clone();
            }
            e
        })?;

        let arc: Arc<T> = Arc::new(parsed);
        let any: Arc<dyn Any + Send + Sync> = arc.clone();
        let _ = self.parsed.borrow_mut().put(key, any);
        Ok(arc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hoi4_paths::PathConfig;

    /// 在临时目录建一个最小 fake install（含 map/ + common/）。
    fn make_fixture(tag: &str) -> (PathConfig, PathBuf) {
        let tmp = std::env::temp_dir().join(format!("ironheart_assets_{tag}"));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(tmp.join("map")).unwrap();
        std::fs::create_dir_all(tmp.join("common")).unwrap();
        std::fs::create_dir_all(tmp.join("interface")).unwrap();
        let cfg = PathConfig::with_game_path(&tmp);
        (cfg, tmp)
    }

    #[test]
    fn open_returns_bytes_and_caches() {
        let (cfg, tmp) = make_fixture("open");
        std::fs::write(tmp.join("interface/topbar.gui"), b"abc").unwrap();
        let db = FsAssetDb::new(cfg);
        let a = db.open("interface/topbar.gui").unwrap();
        let b = db.open("interface/topbar.gui").unwrap();
        assert_eq!(&*a, b"abc");
        // 两次取到同一 Arc（缓存命中）
        assert!(Arc::ptr_eq(&a, &b));
        assert_eq!(db.cache_sizes(), (1, 0));
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn open_missing_returns_not_found() {
        let (cfg, tmp) = make_fixture("missing");
        let db = FsAssetDb::new(cfg);
        let err = db.open("interface/nope.gui").unwrap_err();
        assert!(err.is_not_found());
        assert!(format!("{}", err).contains("nope.gui"));
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn open_referenced_attaches_chain() {
        let (cfg, tmp) = make_fixture("ref");
        let db = FsAssetDb::new(cfg);
        let err = db
            .open_referenced("gfx/missing.dds", "interface/topbar.gui")
            .unwrap_err();
        let s = format!("{}", err);
        assert!(s.contains("gfx/missing.dds") || s.contains("missing.dds"));
        assert!(s.contains("referenced by") && s.contains("topbar.gui"));
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn parse_or_get_caches_typed_results() {
        let (cfg, tmp) = make_fixture("parse");
        std::fs::write(tmp.join("interface/x.txt"), b"hello").unwrap();
        let db = FsAssetDb::new(cfg);

        // 假设的解析器：把字节长度装到 struct 里
        #[derive(Debug)]
        struct Parsed {
            len: usize,
        }
        // 解析次数计数：第二次调用应直接命中缓存
        let calls = std::cell::Cell::new(0);
        let parse = |bytes: &[u8]| {
            calls.set(calls.get() + 1);
            Ok::<Parsed, AssetError>(Parsed { len: bytes.len() })
        };

        let a = db
            .parse_or_get::<Parsed, _>("interface/x.txt", parse)
            .unwrap();
        let b = db
            .parse_or_get::<Parsed, _>("interface/x.txt", |_| {
                panic!("should not be called on cache hit")
            })
            .unwrap();
        assert_eq!(a.len, 5);
        assert!(Arc::ptr_eq(&a, &b));
        assert_eq!(calls.get(), 1);
        assert_eq!(db.cache_sizes(), (1, 1));
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn parse_or_get_different_type_caches_independently() {
        let (cfg, tmp) = make_fixture("parse2");
        std::fs::write(tmp.join("interface/x.txt"), b"abcd").unwrap();
        let db = FsAssetDb::new(cfg);

        struct A(usize);
        struct B(usize);

        let a = db
            .parse_or_get::<A, _>("interface/x.txt", |b| Ok(A(b.len())))
            .unwrap();
        let b = db
            .parse_or_get::<B, _>("interface/x.txt", |b| Ok(B(b.len())))
            .unwrap();
        assert_eq!(a.0, 4);
        assert_eq!(b.0, 4);
        // 两个不同 type_id 各占一格
        assert_eq!(db.cache_sizes(), (1, 2));
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn parse_or_get_propagates_parse_error_with_referrer() {
        let (cfg, tmp) = make_fixture("parse_err");
        std::fs::write(tmp.join("interface/bad.gfx"), b"junk").unwrap();
        let db = FsAssetDb::new(cfg);

        #[derive(Debug)]
        struct Parsed;
        let result = db.parse_or_get::<Parsed, _>("interface/bad.gfx", |_| {
            Err(AssetError::parse("", "expected `spriteType`"))
        });
        let err = result.unwrap_err();
        let s = format!("{}", err);
        assert!(s.contains("bad.gfx"));
        assert!(s.contains("expected `spriteType`"));
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn lru_evicts_at_capacity() {
        let (cfg, tmp) = make_fixture("lru");
        for i in 0..4 {
            std::fs::write(tmp.join(format!("interface/{i}.gui")), b"x").unwrap();
        }
        let db = FsAssetDb::with_capacity(cfg, 2, 2);
        let _ = db.open("interface/0.gui").unwrap();
        let _ = db.open("interface/1.gui").unwrap();
        let _ = db.open("interface/2.gui").unwrap(); // 淘汰 0
        let (raw, _) = db.cache_sizes();
        assert_eq!(raw, 2);
        // 再取 0 会重新读盘但成功
        assert_eq!(&*db.open("interface/0.gui").unwrap(), b"x");
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn list_returns_existing_paths() {
        let (cfg, tmp) = make_fixture("list");
        std::fs::write(tmp.join("interface/a.gui"), b"x").unwrap();
        let db = FsAssetDb::new(cfg);
        let v = db.list("interface/a.gui");
        assert_eq!(v.len(), 1);
        assert_eq!(v[0], tmp.join("interface/a.gui"));
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn clear_caches_resets() {
        let (cfg, tmp) = make_fixture("clear");
        std::fs::write(tmp.join("interface/x.gui"), b"x").unwrap();
        let db = FsAssetDb::new(cfg);
        let _ = db.open("interface/x.gui").unwrap();
        assert_eq!(db.cache_sizes(), (1, 0));
        db.clear_caches();
        assert_eq!(db.cache_sizes(), (0, 0));
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
