//! The ordinary placement identity recipe, shared with closed prepared factories.
use alloc::string::String;
use conduit_core::{CapabilityId, CheckedPlotId, GearId, HostId, PlacementId};
use sha2::{Digest, Sha256};

pub fn canonical_placement_id(
    plot: &CheckedPlotId,
    gear: &GearId,
    host: &HostId,
    capability: &CapabilityId,
) -> PlacementId {
    let mut hash = Sha256::new();
    for part in [
        "placement:",
        plot.as_str(),
        ":",
        gear.as_str(),
        ":",
        host.as_str(),
        ":",
        capability.as_str(),
    ] {
        hash.update(part.as_bytes());
    }
    let mut text = String::with_capacity(64);
    for byte in hash.finalize() {
        for digit in [byte >> 4, byte & 15] {
            text.push(b"0123456789abcdef"[usize::from(digit)] as char);
        }
    }
    PlacementId::from(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn streamed_placement_identity_preserves_original_formatted_recipe() {
        for host in ["host/a", "host/with:punctuation/and-longer-name"] {
            let expected = crate::hash_string(&alloc::format!(
                "placement:{}:{}:{}:{}",
                "checked/a",
                "gear/b",
                host,
                "cap/c"
            ));
            assert_eq!(
                canonical_placement_id(
                    &"checked/a".into(),
                    &"gear/b".into(),
                    &host.into(),
                    &"cap/c".into()
                )
                .as_str(),
                expected
            );
        }
    }
}
