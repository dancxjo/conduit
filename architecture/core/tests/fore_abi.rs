//! Runtime validation can fingerprint an immutable Fore without copying metadata.
use conduit_core::*;
#[path = "../../plot/tests/common/allocation_probe.rs"]
mod probe;
#[global_allocator]
static ALLOCATOR: probe::Allocator = probe::Allocator;
#[test]
fn borrowed_checked_contract_fingerprint_allocates_nothing() {
    let contract = CheckedValueContract {
        value_kind: "value/count".into(),
        maximum_bytes: 8,
        constraints: vec![ValueConstraint::ByteLength {
            minimum: 8,
            maximum: 8,
        }],
    };
    let (digest, memory) = probe::observe(|| {
        assigned_fore_abi_digest(AssignedForeAbi {
            value_kind: contract.value_kind.as_str(),
            value_contract: Some(&contract),
            abnormal_kind: None,
            temporal: "stream",
            track: ConnectionTrack::Payload,
            quantity_profile: QuantityValueProfile::Decimal,
            item_capacity: 1,
            byte_capacity: 8,
        })
    });
    assert_ne!(digest, [0; 32]);
    assert_eq!(memory.allocation_attempts, 0);
    assert_eq!(memory.peak_bytes, 0);
}
