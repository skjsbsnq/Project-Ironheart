pub mod audit;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderMode {
    Legacy,
    CleanMap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderStateBorrowBoundary {
    LegacyFrameWide,
    CleanBackendLocal,
}

impl RenderMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Legacy => "legacy",
            Self::CleanMap => "clean",
        }
    }

    pub const fn render_state_borrow_boundary(self) -> RenderStateBorrowBoundary {
        match self {
            Self::Legacy => RenderStateBorrowBoundary::LegacyFrameWide,
            Self::CleanMap => RenderStateBorrowBoundary::CleanBackendLocal,
        }
    }
}
