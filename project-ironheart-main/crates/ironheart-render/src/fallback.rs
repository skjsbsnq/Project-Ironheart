#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FallbackPolicy {
    Required,
    DegradedAllowed,
    DebugOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceQuality {
    ProductionValid,
    Degraded,
    DebugFixture,
    InvalidForVisualReview,
}

impl ResourceQuality {
    pub fn allows(self, policy: FallbackPolicy) -> bool {
        match (self, policy) {
            (Self::ProductionValid, _) => true,
            (Self::Degraded, FallbackPolicy::DegradedAllowed) => true,
            (Self::DebugFixture, FallbackPolicy::DebugOnly) => true,
            _ => false,
        }
    }
}
