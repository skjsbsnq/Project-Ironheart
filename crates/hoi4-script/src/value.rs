//! [`ScriptValue`] — 脚本运行时通用值类型。

use hoi4_state::{CountryId, ProvinceId, StateId};

/// 比较运算符（HOI4 trigger 用）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compare {
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
}

impl Compare {
    pub fn from_op(op: &str) -> Option<Self> {
        match op {
            "=" | "==" => Some(Self::Eq),
            "!=" => Some(Self::Ne),
            "<" => Some(Self::Lt),
            ">" => Some(Self::Gt),
            "<=" => Some(Self::Le),
            ">=" => Some(Self::Ge),
            _ => None,
        }
    }

    /// 从 parser 的 [`clausewitz_parser::Operator`] 转换
    pub fn from_operator(op: &clausewitz_parser::Operator) -> Self {
        use clausewitz_parser::Operator as O;
        match op {
            O::Eq => Self::Eq,
            O::Ne => Self::Ne,
            O::Lt => Self::Lt,
            O::Gt => Self::Gt,
            O::Le => Self::Le,
            O::Ge => Self::Ge,
        }
    }

    pub fn apply(self, lhs: f64, rhs: f64) -> bool {
        match self {
            Self::Eq => (lhs - rhs).abs() < f64::EPSILON,
            Self::Ne => (lhs - rhs).abs() >= f64::EPSILON,
            Self::Lt => lhs < rhs,
            Self::Gt => lhs > rhs,
            Self::Le => lhs <= rhs,
            Self::Ge => lhs >= rhs,
        }
    }
}

/// 脚本运行时值
#[derive(Debug, Clone, PartialEq)]
pub enum ScriptValue {
    Bool(bool),
    Int(i64),
    Float(f64),
    /// 字符串（标识、国家 tag、ideology 名等）
    String(String),
    Country(CountryId),
    State(StateId),
    Province(ProvinceId),
    /// 表示"无"
    None,
}

impl ScriptValue {
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(b) => Some(*b),
            Self::String(s) => match s.as_str() {
                "yes" | "true" => Some(true),
                "no" | "false" => Some(false),
                _ => None,
            },
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Int(i) => Some(*i as f64),
            Self::Float(f) => Some(*f),
            Self::String(s) => s.parse::<f64>().ok(),
            Self::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        if let Self::String(s) = self {
            Some(s.as_str())
        } else {
            None
        }
    }

    pub fn as_country(&self) -> Option<CountryId> {
        match self {
            Self::Country(c) => Some(*c),
            _ => None,
        }
    }

    pub fn as_state(&self) -> Option<StateId> {
        match self {
            Self::State(s) => Some(*s),
            _ => None,
        }
    }
}

impl From<&clausewitz_parser::Value> for ScriptValue {
    fn from(v: &clausewitz_parser::Value) -> Self {
        use clausewitz_parser::Value as V;
        match v {
            V::Integer(i) => Self::Int(*i),
            V::Float(f) => Self::Float(*f),
            V::Bool(b) => Self::Bool(*b),
            V::String(s) => Self::String(s.clone()),
            // Block / 其它 → 用 None 占位（调用方一般会单独处理 block）
            _ => Self::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compare_apply() {
        assert!(Compare::Lt.apply(1.0, 2.0));
        assert!(Compare::Gt.apply(2.0, 1.0));
        assert!(Compare::Eq.apply(1.0, 1.0));
        assert!(Compare::Ge.apply(1.0, 1.0));
        assert!(Compare::Le.apply(1.0, 1.0));
        assert!(!Compare::Ne.apply(1.0, 1.0));
    }

    #[test]
    fn yes_no_to_bool() {
        let yes = ScriptValue::String("yes".into());
        let no = ScriptValue::String("no".into());
        assert_eq!(yes.as_bool(), Some(true));
        assert_eq!(no.as_bool(), Some(false));
    }

    #[test]
    fn numeric_coercion() {
        let i = ScriptValue::Int(42);
        let f = ScriptValue::Float(2.5);
        let s = ScriptValue::String("3.14".into());
        assert_eq!(i.as_f64(), Some(42.0));
        assert_eq!(f.as_f64(), Some(2.5));
        assert!((s.as_f64().unwrap() - 3.14).abs() < 1e-9);
    }
}
