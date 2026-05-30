//! Scope 系统 —— THIS / ROOT / FROM / PREV。
//!
//! HOI4 scope 是一个栈：
//! - `THIS` 是栈顶（当前 scope）
//! - `ROOT` 是栈底（最初进入脚本时的 scope）
//! - `FROM` 是把当前栈顶弹掉后的栈顶（即"调用者"）
//! - `PREV` 同 FROM；`PREV.PREV` 再上一层
//!
//! 当一个 scope changer（如 `every_country = { ... }`）进入子 block 时，
//! 子 block 的 THIS 是新 scope，FROM 是父 block 的 THIS，ROOT 不变。

use hoi4_state::{CountryId, ProvinceId, StateId};

/// scope 当前指向的实体
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Scope {
    None,
    Country(CountryId),
    State(StateId),
    Province(ProvinceId),
}

impl Scope {
    pub fn kind(&self) -> ScopeKind {
        match self {
            Self::None => ScopeKind::None,
            Self::Country(_) => ScopeKind::Country,
            Self::State(_) => ScopeKind::State,
            Self::Province(_) => ScopeKind::Province,
        }
    }

    pub fn as_country(&self) -> Option<CountryId> {
        if let Self::Country(c) = self {
            Some(*c)
        } else {
            None
        }
    }

    pub fn as_state(&self) -> Option<StateId> {
        if let Self::State(s) = self {
            Some(*s)
        } else {
            None
        }
    }

    pub fn as_province(&self) -> Option<ProvinceId> {
        if let Self::Province(p) = self {
            Some(*p)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeKind {
    None,
    Country,
    State,
    Province,
}

/// scope chain 的栈实现。
///
/// 不存外部 ROOT —— ROOT 永远是栈底（index 0）。
/// 每次 [`Self::push`] 推入新 THIS。
#[derive(Debug, Clone)]
pub struct ScopeChain {
    stack: Vec<Scope>,
}

impl ScopeChain {
    /// 用 ROOT 初始化
    pub fn new(root: Scope) -> Self {
        Self { stack: vec![root] }
    }

    /// THIS = 栈顶
    pub fn this(&self) -> Scope {
        self.stack.last().copied().unwrap_or(Scope::None)
    }

    /// ROOT = 栈底
    pub fn root(&self) -> Scope {
        self.stack.first().copied().unwrap_or(Scope::None)
    }

    /// FROM = 倒数第二（如果只有一层 THIS = ROOT，FROM 也 = ROOT）
    pub fn from(&self) -> Scope {
        let n = self.stack.len();
        if n >= 2 {
            self.stack[n - 2]
        } else {
            self.stack.first().copied().unwrap_or(Scope::None)
        }
    }

    /// PREV(n) — 从栈顶往下数 n 层（n=0 即 THIS，n=1 即 FROM）
    pub fn prev(&self, n: usize) -> Scope {
        if self.stack.is_empty() {
            return Scope::None;
        }
        let len = self.stack.len();
        if n >= len {
            self.stack[0]
        } else {
            self.stack[len - 1 - n]
        }
    }

    /// 推入一个新 scope（变成新 THIS）
    pub fn push(&mut self, s: Scope) {
        self.stack.push(s);
    }

    /// 弹出栈顶
    pub fn pop(&mut self) -> Option<Scope> {
        if self.stack.len() <= 1 {
            // 不允许弹空 ROOT
            return None;
        }
        self.stack.pop()
    }

    pub fn depth(&self) -> usize {
        self.stack.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn this_root_initial() {
        let c = ScopeChain::new(Scope::Country(CountryId(1)));
        assert_eq!(c.this(), Scope::Country(CountryId(1)));
        assert_eq!(c.root(), Scope::Country(CountryId(1)));
        // 单层时 FROM == ROOT
        assert_eq!(c.from(), Scope::Country(CountryId(1)));
    }

    #[test]
    fn push_pop_changes_this() {
        let mut c = ScopeChain::new(Scope::Country(CountryId(1)));
        c.push(Scope::State(StateId(5)));
        assert_eq!(c.this(), Scope::State(StateId(5)));
        assert_eq!(c.root(), Scope::Country(CountryId(1)));
        assert_eq!(c.from(), Scope::Country(CountryId(1)));
        c.pop();
        assert_eq!(c.this(), Scope::Country(CountryId(1)));
    }

    #[test]
    fn cannot_pop_root() {
        let mut c = ScopeChain::new(Scope::Country(CountryId(1)));
        assert!(c.pop().is_none());
        assert_eq!(c.this(), Scope::Country(CountryId(1)));
    }

    #[test]
    fn prev_n_levels() {
        let mut c = ScopeChain::new(Scope::Country(CountryId(1)));
        c.push(Scope::Country(CountryId(2)));
        c.push(Scope::State(StateId(7)));
        assert_eq!(c.prev(0), Scope::State(StateId(7))); // THIS
        assert_eq!(c.prev(1), Scope::Country(CountryId(2))); // FROM
        assert_eq!(c.prev(2), Scope::Country(CountryId(1))); // PREV.PREV / ROOT
                                                             // 越界 → ROOT
        assert_eq!(c.prev(99), Scope::Country(CountryId(1)));
    }
}
