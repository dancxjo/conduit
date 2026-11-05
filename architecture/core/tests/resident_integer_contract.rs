//! Borrowed fixed predicates preserve the owned definition and refusal laws.
use conduit_core::*;
#[path = "common/numerical_allocation.rs"]
mod allocation;

#[test]
fn borrowed_and_owned_ranges_match_all_fixed_widths_and_endpoints_without_allocation() {
    for (name, kind, signed) in [
        ("value/u8", PrimitiveInfoKind::U8, false),
        ("value/i8", PrimitiveInfoKind::I8, true),
        ("value/u16", PrimitiveInfoKind::U16, false),
        ("value/i16", PrimitiveInfoKind::I16, true),
        ("value/u32", PrimitiveInfoKind::U32, false),
        ("value/i32", PrimitiveInfoKind::I32, true),
        ("value/u64", PrimitiveInfoKind::U64, false),
        ("value/i64", PrimitiveInfoKind::I64, true),
        ("value/u128", PrimitiveInfoKind::U128, false),
        ("value/i128", PrimitiveInfoKind::I128, true),
    ] {
        let number = |n: i128| {
            if signed {
                FixedInteger::from_signed(kind, n).unwrap()
            } else {
                FixedInteger::from_unsigned(kind, n as u128).unwrap()
            }
        };
        let encoded = |n| {
            let (bytes, length) = number(n).encode();
            bytes[..length].to_vec()
        };
        let low = if signed { -2 } else { 2 };
        let high = 5;
        for minimum_endpoint in [IntervalEndpoint::Inclusive, IntervalEndpoint::Exclusive] {
            for maximum_endpoint in [IntervalEndpoint::Inclusive, IntervalEndpoint::Exclusive] {
                let minimum = encoded(low);
                let maximum = encoded(high);
                let owned = CheckedValueContract::new(
                    kind_id(name),
                    fixed_integer_bytes(kind) as u32,
                    vec![ValueConstraint::FixedIntegerRange {
                        minimum: Some(minimum.clone()),
                        maximum: Some(maximum.clone()),
                        minimum_endpoint,
                        maximum_endpoint,
                    }],
                )
                .unwrap();
                let spec = FixedIntegerContractSpec {
                    value_kind: name,
                    maximum_bytes: owned.maximum_bytes,
                    range: Some(FixedIntegerRange {
                        minimum: Some(&minimum),
                        maximum: Some(&maximum),
                        minimum_endpoint,
                        maximum_endpoint,
                    }),
                };
                let (_, allocations) = allocation::observe(|| {
                    let borrowed = BorrowedFixedIntegerContract::new(&spec).unwrap();
                    assert_eq!(borrowed.validation_work(), 5);
                    for n in [low - 1, low, low + 1, high - 1, high, high + 1] {
                        let (bytes, length) = number(n).encode();
                        assert_eq!(
                            borrowed.validate(&bytes[..length]),
                            owned.validate(&bytes[..length]),
                            "{name} {n}"
                        );
                    }
                    for malformed in [&[][..], &[0; 3][..], &[0; 17][..]] {
                        assert_eq!(borrowed.validate(malformed), owned.validate(malformed));
                    }
                });
                assert_eq!(allocations, 0);
            }
        }
    }
    assert_eq!(
        core::mem::size_of::<BorrowedFixedIntegerContract<'static>>(),
        core::mem::size_of::<usize>()
    );
}

#[test]
fn malformed_or_empty_range_witnesses_refuse_before_execution() {
    for (minimum, maximum, endpoint) in [
        (&[7][..], &[6][..], IntervalEndpoint::Inclusive),
        (&[7][..], &[7][..], IntervalEndpoint::Exclusive),
        (&[0, 0][..], &[7][..], IntervalEndpoint::Inclusive),
    ] {
        let range = FixedIntegerRange {
            minimum: Some(minimum),
            maximum: Some(maximum),
            minimum_endpoint: endpoint,
            maximum_endpoint: IntervalEndpoint::Inclusive,
        };
        let spec = FixedIntegerContractSpec {
            value_kind: "value/u8",
            maximum_bytes: 1,
            range: Some(range),
        };
        let ((), count) =
            allocation::observe(|| assert!(BorrowedFixedIntegerContract::new(&spec).is_err()));
        assert_eq!(count, 0);
    }
    for value_kind in ["value/bool", "value/text", "unknown/kind"] {
        let spec = FixedIntegerContractSpec {
            value_kind,
            maximum_bytes: 1,
            range: None,
        };
        assert!(BorrowedFixedIntegerContract::new(&spec).is_err());
    }
}
