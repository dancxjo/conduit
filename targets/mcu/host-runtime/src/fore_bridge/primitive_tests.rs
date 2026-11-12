use super::*;
use std::vec;
fn abi(contract: Option<&c::CheckedValueContract>) -> c::AssignedForeAbi<'_> {
    c::AssignedForeAbi {
        value_kind: c::COUNT_INFO_ID,
        value_contract: contract,
        abnormal_kind: None,
        temporal: "flow-closing",
        track: c::ConnectionTrack::Payload,
        quantity_profile: c::QuantityValueProfile::Decimal,
        item_capacity: 1,
        byte_capacity: 8,
    }
}
fn pin(contract: Option<&c::CheckedValueContract>) -> c::AssignedLineForeRequirement {
    c::AssignedLineForeRequirement::new(
        c::AssignedForeRequirement::new(
            0,
            0,
            c::AssignedForeDirection::Input,
            c::AssignedIdentity([6; 16]),
            c::assigned_fore_abi_digest(abi(contract)),
        ),
        c::AssignedRemoteBinding {
            line: c::AssignedIdentity([1; 16]),
            local_host: c::AssignedIdentity([2; 16]),
            local_boot: c::AssignedIdentity([3; 16]),
            peer_host: c::AssignedIdentity([4; 16]),
            peer_boot: c::AssignedIdentity([5; 16]),
        },
        c::AssignedIdentity([7; 16]),
        c::AssignedIdentity([8; 16]),
        [9; 32],
    )
}
fn range(minimum: u64, maximum: u64) -> c::ValueConstraint {
    c::ValueConstraint::UnsignedRange {
        minimum: Some(minimum),
        maximum: Some(maximum),
        minimum_endpoint: c::IntervalEndpoint::Inclusive,
        maximum_endpoint: c::IntervalEndpoint::Inclusive,
    }
}
fn contract(constraints: std::vec::Vec<c::ValueConstraint>) -> c::CheckedValueContract {
    c::CheckedValueContract::new(c::COUNT_INFO_ID.into(), 8, constraints).unwrap()
}
#[test]
fn constrained_count_keeps_inclusive_bounds_after_cold_contract_retirement_without_allocation() {
    let cold = contract(vec![range(0, u32::MAX as u64)]);
    let pin = pin(Some(&cold));
    let (spec, cost) = crate::member_return_tests::return_allocation_probe::observe(|| {
        PrimitiveIngressSpec::checked(&pin, abi(Some(&cold))).unwrap()
    });
    assert_eq!((cost.allocations, cost.reallocations), (0, 0));
    drop(cold);
    let (_, cost) = crate::member_return_tests::return_allocation_probe::observe(|| {
        for value in [0, 1, u32::MAX as u64] {
            assert_eq!(spec.validate(&value.to_le_bytes()), Ok(()));
        }
        assert_eq!(
            spec.validate(&(u32::MAX as u64 + 1).to_le_bytes()),
            Err(Refusal::InvalidValue)
        );
        assert_eq!(
            spec.validate(&u64::MAX.to_le_bytes()),
            Err(Refusal::InvalidValue)
        );
        assert_eq!(spec.validate(&[0; 7]), Err(Refusal::InvalidValue));
    });
    assert_eq!((cost.allocations, cost.reallocations), (0, 0));
    let cold = contract(vec![range(2, 4)]);
    let pin = pin_for(&cold);
    let spec = PrimitiveIngressSpec::checked(&pin, abi(Some(&cold))).unwrap();
    for value in [2u64, 4] {
        assert_eq!(spec.validate(&value.to_le_bytes()), Ok(()));
    }
    for value in [1u64, 5] {
        assert_eq!(
            spec.validate(&value.to_le_bytes()),
            Err(Refusal::InvalidValue)
        );
    }
}
fn pin_for(cold: &c::CheckedValueContract) -> c::AssignedLineForeRequirement {
    pin(Some(cold))
}
#[test]
fn count_profiles_check_full_digest_and_refuse_widening_or_unsupported_constraints() {
    let original = contract(vec![range(0, u32::MAX as u64)]);
    let exact = pin(Some(&original));
    let widened = contract(vec![range(0, u64::MAX)]);
    for proposed in [None, Some(&widened)] {
        assert!(matches!(
            PrimitiveIngressSpec::checked(&exact, abi(proposed)),
            Err(Refusal::UnsupportedAbi)
        ));
    }
    for constraints in [
        vec![
            c::ValueConstraint::ByteLength {
                minimum: 8,
                maximum: 8,
            },
            range(0, 10),
        ],
        vec![c::ValueConstraint::ByteLength {
            minimum: 8,
            maximum: 8,
        }],
        vec![c::ValueConstraint::UnsignedRange {
            minimum: Some(0),
            maximum: Some(10),
            minimum_endpoint: c::IntervalEndpoint::Exclusive,
            maximum_endpoint: c::IntervalEndpoint::Inclusive,
        }],
    ] {
        let cold = contract(constraints);
        let pin = pin(Some(&cold));
        assert!(matches!(
            PrimitiveIngressSpec::checked(&pin, abi(Some(&cold))),
            Err(Refusal::UnsupportedAbi)
        ));
    }
    let empty = contract(vec![]);
    let pin = pin(Some(&empty));
    let spec = PrimitiveIngressSpec::checked(&pin, abi(Some(&empty))).unwrap();
    assert_eq!(spec.validate(&u64::MAX.to_le_bytes()), Ok(()));
    let foreign = c::CheckedValueContract::new("value/bool".into(), 8, vec![]).unwrap();
    let foreign_pin = pin_for(&foreign);
    assert!(matches!(
        PrimitiveIngressSpec::checked(&foreign_pin, abi(Some(&foreign))),
        Err(Refusal::UnsupportedAbi)
    ));
    let mut wrong = empty;
    wrong.maximum_bytes = 9;
    assert!(matches!(
        PrimitiveIngressSpec::checked(&pin, abi(Some(&wrong))),
        Err(Refusal::UnsupportedAbi)
    ));
}
