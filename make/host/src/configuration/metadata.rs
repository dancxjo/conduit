use serde::{Deserialize, Serialize};

/// Descriptive placement only: no presence, authority or acquisition-location claim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostMetadata {
    pub placement: String,
}
impl HostMetadata {
    pub(super) fn valid(&self) -> bool {
        !self.placement.trim().is_empty()
            && self.placement.len() <= 96
            && !self.placement.chars().any(char::is_control)
    }
}

#[cfg(test)]
#[path = "metadata_tests.rs"]
mod tests;
