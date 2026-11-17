//! Exact capacities of the supported immutable startup-field profile.
use super::*;
use crate::metadata_backing::Bytes;
impl KindConfigurationField {
    /// Owned payload only, excluding inline Self and allocator bookkeeping.
    /// Static structured bytes stay independently owned; dynamic Cow payload
    /// spare capacity is counted by its existing Core owner. Unknown private
    /// numeric evidence wrappers refuse rather than substituting slice length.
    pub fn retained_backing_bytes(&self) -> Option<usize> {
        let KindConfigurationField {
            key,
            default_value,
            rule,
        } = self;
        let mut n = Bytes::default();
        n.add(key.capacity())?;
        n.configuration_value(default_value)?;
        n.configuration_rule(rule)?;
        Some(n.0)
    }
}
