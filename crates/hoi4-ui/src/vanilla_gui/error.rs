use std::path::PathBuf;

/// Hard errors returned by explicit file-loading APIs.
///
/// The runtime avoids panics for bad vanilla/mod data.  A missing optional asset
/// usually becomes [`VanillaGuiIssue`]; these hard errors are reserved for caller
/// requested files that cannot be read at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VanillaGuiError {
    MissingGui(PathBuf),
    MissingGfx(PathBuf),
    Io { path: PathBuf, message: String },
}

impl std::fmt::Display for VanillaGuiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingGui(path) => write!(f, "missing .gui file: {}", path.display()),
            Self::MissingGfx(path) => write!(f, "missing .gfx file: {}", path.display()),
            Self::Io { path, message } => write!(f, "{}: {message}", path.display()),
        }
    }
}

impl std::error::Error for VanillaGuiError {}

/// Non-fatal runtime issue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VanillaGuiIssue {
    pub kind: VanillaGuiIssueKind,
    pub source: Option<PathBuf>,
    pub detail: String,
}

impl VanillaGuiIssue {
    pub fn new(kind: VanillaGuiIssueKind, detail: impl Into<String>) -> Self {
        Self {
            kind,
            source: None,
            detail: detail.into(),
        }
    }

    pub fn with_source(
        kind: VanillaGuiIssueKind,
        source: impl Into<PathBuf>,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            source: Some(source.into()),
            detail: detail.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VanillaGuiIssueKind {
    MissingGui,
    MissingGfx,
    MissingTexture,
    UnknownNodeType,
    InvalidSize,
    InvalidPosition,
    UnsupportedResourceType,
    ParseTolerance,
}
