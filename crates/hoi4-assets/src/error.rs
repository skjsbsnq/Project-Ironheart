//! 资产错误链。
//!
//! 设计点：缺资产时不仅要说"哪个文件没找到"，还要说"哪个上层文件引用了它"——
//! 这样从启动 log 一眼能定位是 mod 引用了不存在的纹理还是 vanilla 自己漏文件。

use std::path::{Path, PathBuf};

/// 资产层错误。带 *referrer* 链：`A.gui` 引用 `B.dds`，`B.dds` 又引用 `C.fnt`，
/// 任一节点失败时整条链都能输出。
#[derive(Debug)]
pub struct AssetError {
    pub path: PathBuf,
    pub kind: AssetErrorKind,
    /// 引用本资产的上层资产路径（None = 直接由用户代码请求）
    pub referrer: Option<Box<AssetError>>,
}

#[derive(Debug)]
pub enum AssetErrorKind {
    /// 沿 mod 链 + vanilla 都找不到此相对路径。
    NotFound,
    /// 物理文件存在但 IO 读取失败（权限 / 损坏 / 远端断开）。
    Io(std::io::Error),
    /// 解析层报错（语法 / 格式不对）。`detail` 由解析器决定。
    Parse { detail: String },
}

impl AssetError {
    /// 构造一个"未找到"错误，无引用链。
    pub fn not_found(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            kind: AssetErrorKind::NotFound,
            referrer: None,
        }
    }

    /// 构造一个 IO 错误。
    pub fn io(path: impl Into<PathBuf>, err: std::io::Error) -> Self {
        Self {
            path: path.into(),
            kind: AssetErrorKind::Io(err),
            referrer: None,
        }
    }

    /// 构造一个解析错误。
    pub fn parse(path: impl Into<PathBuf>, detail: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            kind: AssetErrorKind::Parse {
                detail: detail.into(),
            },
            referrer: None,
        }
    }

    /// 给本错误挂上"我是被谁引用的"。链式构造，常用形态：
    ///
    /// ```ignore
    /// db.open(child).map_err(|e| e.with_referrer_path(parent))?;
    /// ```
    ///
    /// 多次 `with_referrer_*` 的语义是 *自下而上* 累积：先调用挂的是直接引用者
    /// （内层），后调用的是更上层引用者（外层）。
    pub fn with_referrer(self, new_outer: AssetError) -> Self {
        Self {
            path: self.path,
            kind: self.kind,
            referrer: Some(append_to_chain(self.referrer, new_outer)),
        }
    }

    /// 便捷：用一个路径直接构造一个 `referrer` 节点（kind 标记为 NotFound 占位，
    /// 仅作为引用链中的一环用，不会被 Display 打成"未找到"）。
    pub fn with_referrer_path(self, referrer_path: impl Into<PathBuf>) -> Self {
        let r = AssetError {
            path: referrer_path.into(),
            kind: AssetErrorKind::NotFound,
            referrer: None,
        };
        self.with_referrer(r)
    }

    /// 取出最深的（=真正失败的）资产路径。
    pub fn root_path(&self) -> &Path {
        // 因为 `with_referrer` 把 referrer 装在外层，self 永远是"叶子"=真正失败者。
        &self.path
    }

    /// 是否是"未找到"错误（缓存层用来决定是否重试）。
    pub fn is_not_found(&self) -> bool {
        matches!(self.kind, AssetErrorKind::NotFound)
    }
}

fn append_to_chain(existing: Option<Box<AssetError>>, new_outer: AssetError) -> Box<AssetError> {
    match existing {
        None => Box::new(new_outer),
        Some(mut head) => {
            // 从 head 走到链末端（referrer == None 那个节点），把 new_outer 挂到那里。
            // 这样后调用的 with_referrer 永远落在更"外层"，与 Display 输出方向一致：
            //   missing.dds → referenced by: topbar.gui → referenced by: all.gui
            {
                let mut cursor: &mut AssetError = head.as_mut();
                while cursor.referrer.is_some() {
                    cursor = cursor.referrer.as_mut().unwrap();
                }
                cursor.referrer = Some(Box::new(new_outer));
            }
            head
        }
    }
}

impl std::fmt::Display for AssetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.kind {
            AssetErrorKind::NotFound => {
                write!(f, "asset not found: {}", self.path.display())?;
            }
            AssetErrorKind::Io(err) => {
                write!(f, "asset io failed: {} ({})", self.path.display(), err)?;
            }
            AssetErrorKind::Parse { detail } => {
                write!(f, "asset parse failed: {}: {}", self.path.display(), detail)?;
            }
        }
        // 引用链：referenced by A → referenced by B → ...
        let mut cur = self.referrer.as_deref();
        while let Some(r) = cur {
            write!(f, "\n  referenced by: {}", r.path.display())?;
            cur = r.referrer.as_deref();
        }
        Ok(())
    }
}

impl std::error::Error for AssetError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.kind {
            AssetErrorKind::Io(e) => Some(e),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_includes_referrer_chain() {
        let inner = AssetError::not_found("gfx/textures/missing.dds")
            .with_referrer_path("interface/topbar.gui")
            .with_referrer_path("interface/all.gui");
        let s = format!("{}", inner);
        assert!(s.contains("missing.dds"));
        assert!(s.contains("referenced by: interface"));
        // 链顺序：最深错误在最上，引用者向下延展
        let topbar_pos = s.find("topbar.gui").unwrap();
        let all_pos = s.find("all.gui").unwrap();
        assert!(topbar_pos < all_pos, "outer chain order broken: {}", s);
    }

    #[test]
    fn root_path_is_leaf() {
        let e = AssetError::not_found("a")
            .with_referrer_path("b")
            .with_referrer_path("c");
        assert_eq!(e.root_path(), Path::new("a"));
    }

    #[test]
    fn is_not_found_propagates_through_chain() {
        let e = AssetError::not_found("x").with_referrer_path("y");
        assert!(e.is_not_found());
        let e2 = AssetError::parse("x", "bad").with_referrer_path("y");
        assert!(!e2.is_not_found());
    }
}
