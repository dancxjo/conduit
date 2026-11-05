//! Compare owned value-contract payload with actual allocator retirement, using
//! the same observer as affine capability storage. No receipt/authority is issued.
use super::allocation;
use conduit_core::{kind_id, CheckedValueContract, ValueConstraint, BYTES_INFO_ID};
#[test]
fn contract_backing_matches_retirement_with_nested_spare_capacity() {
    let mut kind = kind_id(BYTES_INFO_ID);
    kind.0.reserve(80);
    let mut member = Vec::with_capacity(123);
    member.push(1);
    let mut members = Vec::with_capacity(7);
    members.push(member);
    let mut constraints = Vec::with_capacity(5);
    constraints.push(ValueConstraint::CanonicalMembership {
        members,
        negated: false,
    });
    let contract = CheckedValueContract::new(kind, 128, constraints).unwrap();
    let bytes = contract.retained_backing_bytes().unwrap();
    allocation::without_allocations(|| {
        assert_eq!(contract.retained_backing_bytes(), Some(bytes));
        contract.validate(&[1]).unwrap();
    });
    let (_, released) = allocation::released_bytes(|| drop(contract));
    assert_eq!(bytes, released);
}
