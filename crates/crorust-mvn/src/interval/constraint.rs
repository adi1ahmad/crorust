#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IntervalBoundEndpoint {
    pub version: String,
    pub inclusive: bool,
}

impl IntervalBoundEndpoint {
    pub fn inclusive(version: &str) -> Self {
        Self {
            version: version.to_string(),
            inclusive: true,
        }
    }

    pub fn exclusive(version: &str) -> Self {
        Self {
            version: version.to_string(),
            inclusive: false,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VersionConstraint {
    Exact(String),
    Greater(IntervalBoundEndpoint),
    Lower(IntervalBoundEndpoint),
    Between(IntervalBoundEndpoint, IntervalBoundEndpoint),
}
