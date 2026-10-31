//! Shared canonical configuration fields and their immutable local commitment.
use super::*;
pub(crate) fn push_configuration(
    canonical: &mut impl crate::canonical_bytes::CanonicalSink,
    configuration: &[crate::ConfigurationEntry],
) {
    push_u32(canonical, configuration.len() as u32);
    for entry in configuration {
        push_string(canonical, &entry.key);
        push_configuration_value(canonical, &entry.value);
    }
}
pub(crate) fn configuration_fingerprint(configuration: &[crate::ConfigurationEntry]) -> [u8; 32] {
    let mut canonical = CanonicalHash::new();
    push_configuration(&mut canonical, configuration);
    canonical.finish()
}
