//! Opaque handle authentication; public scope facts cannot reveal the table key.
use super::{KernelCapabilityScope, ProtectionDomainId};
use sha2::{Digest, Sha256};

pub(super) fn seal(
    secret: u64,
    domain: ProtectionDomainId,
    slot: usize,
    issuance: u32,
    scope: &KernelCapabilityScope,
) -> u64 {
    // HMAC-SHA256 with the Root-only key, truncated to the wire handle width.
    // The complete scope has a fixed-width encoding, independent of host ABI.
    let mut inner_pad = [0x36; 64];
    let mut outer_pad = [0x5c; 64];
    for (index, byte) in secret.to_le_bytes().iter().enumerate() {
        inner_pad[index] ^= byte;
        outer_pad[index] ^= byte;
    }
    let mut inner = Sha256::new();
    inner.update(inner_pad);
    inner.update(b"conduit.conduitos/domain-handle@2");
    inner.update(domain.0.to_le_bytes());
    inner.update((slot as u64).to_le_bytes());
    inner.update(issuance.to_le_bytes());
    for identity in [
        scope.host,
        scope.boot,
        scope.plan,
        scope.play,
        scope.implementation,
        scope.base,
        scope.resource,
        scope.subject,
        scope.authority,
    ] {
        inner.update(identity);
    }
    for bound in [
        scope.base_generation,
        scope.resource_generation,
        scope.operation,
        scope.maximum_parameter_bytes,
        scope.maximum_work_units,
        scope.maximum_operations,
    ] {
        inner.update(bound.to_le_bytes());
    }
    inner.update(scope.maximum_in_flight.to_le_bytes());
    let mut outer = Sha256::new();
    outer.update(outer_pad);
    outer.update(inner.finalize());
    let digest = outer.finalize();
    u64::from_le_bytes(digest[..8].try_into().expect("fixed SHA-256 prefix")) | 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn independently_generated_hmac_wire_vector() {
        // Python hmac/sha256, LE <IQI>, nine 32-byte IDs, <6IH> bounds.
        let scope = KernelCapabilityScope {
            host: [1; 32],
            boot: [2; 32],
            plan: [3; 32],
            play: [4; 32],
            implementation: [5; 32],
            base: [6; 32],
            resource: [7; 32],
            subject: [8; 32],
            authority: [9; 32],
            base_generation: 1,
            resource_generation: 1,
            operation: 7,
            maximum_parameter_bytes: 8,
            maximum_work_units: 2,
            maximum_operations: 2,
            maximum_in_flight: 1,
        };
        assert_eq!(
            seal(0x12345678, ProtectionDomainId(3), 0, 1, &scope),
            0x48a61932ad10b511
        );
    }
}
