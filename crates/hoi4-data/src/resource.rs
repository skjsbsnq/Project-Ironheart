//! 资源类型定义（oil / steel / aluminium 等）

/// HOI4 默认 7 种战略资源
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResourceKind {
    Oil,
    Aluminium,
    Rubber,
    Tungsten,
    Steel,
    Chromium,
    Coal,
}

impl ResourceKind {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "oil" => Some(Self::Oil),
            "aluminium" | "aluminum" => Some(Self::Aluminium),
            "rubber" => Some(Self::Rubber),
            "tungsten" => Some(Self::Tungsten),
            "steel" => Some(Self::Steel),
            "chromium" => Some(Self::Chromium),
            "coal" => Some(Self::Coal),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Oil => "oil",
            Self::Aluminium => "aluminium",
            Self::Rubber => "rubber",
            Self::Tungsten => "tungsten",
            Self::Steel => "steel",
            Self::Chromium => "chromium",
            Self::Coal => "coal",
        }
    }

    /// 全部资源（按规范顺序）
    pub fn all() -> [Self; 7] {
        [
            Self::Oil,
            Self::Aluminium,
            Self::Rubber,
            Self::Tungsten,
            Self::Steel,
            Self::Chromium,
            Self::Coal,
        ]
    }
}

/// 资源定义（来自 `common/resources/`）
#[derive(Debug, Clone)]
pub struct ResourceDef {
    pub kind: ResourceKind,
    /// 多少民用工厂等价于 1 单位资源（用于贸易定价）。HOI4 default = 0.125
    pub cic: f32,
    /// 每多少单位需要 1 个船团（默认 0.1，即 10 单位/船团）
    pub convoys: f32,
}
