//! [`ScriptContext`] —— 脚本一次执行所需的所有"环境"。
//!
//! 因 Rust 借用规则不允许在 trait 对象中持有 `&mut World` + `&GameData` 又同时穿过
//! 多个回调，本模块对引擎采用"传参"风格而非把 ScriptContext 完全打包进结构体。
//! ScriptContext 只持有不可变的 `&GameData` + `ScopeChain` + 元数据，可变副作用直接
//! 通过 `&mut World` 穿入。

use crate::scope::ScopeChain;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScriptError {
    /// scope 类型不匹配
    ScopeMismatch {
        needed: crate::scope::ScopeKind,
        got: crate::scope::ScopeKind,
    },
    /// 未知 trigger
    UnknownTrigger(String),
    /// 未知 effect
    UnknownEffect(String),
    /// 块结构异常 / 缺字段
    Malformed(String),
}

impl std::fmt::Display for ScriptError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ScopeMismatch { needed, got } => {
                write!(f, "scope kind mismatch (need {:?}, got {:?})", needed, got)
            }
            Self::UnknownTrigger(s) => write!(f, "unknown trigger: {}", s),
            Self::UnknownEffect(s) => write!(f, "unknown effect: {}", s),
            Self::Malformed(s) => write!(f, "malformed: {}", s),
        }
    }
}

impl std::error::Error for ScriptError {}

/// 包装一次脚本执行所需的"上下文"。
///
/// `World` / `Variables` / `Flags` 因经常需要可变借用而单独传参；本结构只装
/// 当前 scope chain。
#[derive(Debug, Clone)]
pub struct ScriptContext {
    pub chain: ScopeChain,
}

impl ScriptContext {
    pub fn new(chain: ScopeChain) -> Self {
        Self { chain }
    }
}
