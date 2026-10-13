use conduit_core::{
    BOOL_ENCODED_LEN, BOOL_INFO_ID, InfoBool, InfoDecodeError, MICRO_SCALAR_ENCODED_LEN,
    MICRO_SCALAR_INFO_ID, MicroScalar, MicroScalarArithmeticError,
};

#[test]
fn bool_contract_has_one_canonical_byte_per_state() {
    assert_eq!(BOOL_INFO_ID, "value/bool");
    assert_eq!(BOOL_ENCODED_LEN, 1);
    assert_eq!(InfoBool::FALSE.encode(), [0]);
    assert_eq!(InfoBool::TRUE.encode(), [1]);
    assert_eq!(InfoBool::decode(&[0]), Ok(InfoBool::FALSE));
    assert_eq!(InfoBool::decode(&[1]), Ok(InfoBool::TRUE));
}

#[test]
fn bool_refuses_integer_like_and_malformed_encodings() {
    assert_eq!(
        InfoBool::decode(&[2]),
        Err(InfoDecodeError::NonCanonicalBoolean(2))
    );
    assert_eq!(
        InfoBool::decode(b"true"),
        Err(InfoDecodeError::WrongLength {
            expected: 1,
            actual: 4,
        })
    );
    assert_eq!(
        InfoBool::decode(&[]),
        Err(InfoDecodeError::WrongLength {
            expected: 1,
            actual: 0,
        })
    );
}

#[test]
fn micro_scalar_contract_is_exact_signed_fixed_point() {
    assert_eq!(MICRO_SCALAR_INFO_ID, "realization/micro-scalar@1");
    assert_eq!(MICRO_SCALAR_ENCODED_LEN, 8);
    assert_eq!(MicroScalar::SCALE, 1_000_000);

    for value in [
        MicroScalar::MIN,
        MicroScalar::from_raw_microunits(-1),
        MicroScalar::ZERO,
        MicroScalar::ONE,
        MicroScalar::MAX,
    ] {
        assert_eq!(MicroScalar::decode(&value.encode()), Ok(value));
    }
    assert!(MicroScalar::from_raw_microunits(-1) < MicroScalar::ZERO);
    assert!(MicroScalar::ONE < MicroScalar::MAX);
}

#[test]
fn micro_scalar_refuses_every_non_exact_width() {
    for length in 0..16 {
        if length == MICRO_SCALAR_ENCODED_LEN {
            continue;
        }
        let encoded = [0_u8; 16];
        assert_eq!(
            MicroScalar::decode(&encoded[..length]),
            Err(InfoDecodeError::WrongLength {
                expected: MICRO_SCALAR_ENCODED_LEN,
                actual: length,
            })
        );
    }
}

#[test]
fn micro_scalar_arithmetic_is_checked_and_multiplication_truncates_toward_zero() {
    let one_and_half = MicroScalar::from_raw_microunits(1_500_000);
    let two = MicroScalar::from_raw_microunits(2_000_000);
    assert_eq!(
        one_and_half.checked_mul(two),
        Ok(MicroScalar::from_raw_microunits(3_000_000))
    );
    assert_eq!(
        MicroScalar::from_raw_microunits(1).checked_mul(MicroScalar::from_raw_microunits(500_000)),
        Ok(MicroScalar::ZERO)
    );
    assert_eq!(
        MicroScalar::from_raw_microunits(-1).checked_mul(MicroScalar::from_raw_microunits(500_000)),
        Ok(MicroScalar::ZERO)
    );
    assert_eq!(
        MicroScalar::MAX.checked_add(MicroScalar::from_raw_microunits(1)),
        Err(MicroScalarArithmeticError::Overflow)
    );
    assert_eq!(
        MicroScalar::MIN.checked_sub(MicroScalar::from_raw_microunits(1)),
        Err(MicroScalarArithmeticError::Overflow)
    );
    assert_eq!(
        MicroScalar::MAX.checked_mul(MicroScalar::from_raw_microunits(1_000_001)),
        Err(MicroScalarArithmeticError::Overflow)
    );
    assert_eq!(
        MicroScalar::from_raw_microunits(3_000_000).checked_div(two),
        Ok(one_and_half)
    );
    assert_eq!(
        MicroScalar::ONE.checked_div(MicroScalar::ZERO),
        Err(MicroScalarArithmeticError::DivisionByZero)
    );
    assert_eq!(
        MicroScalar::from_raw_microunits(3_500_000).checked_rem(two),
        Ok(one_and_half)
    );
    assert_eq!(
        one_and_half.checked_neg(),
        Ok(MicroScalar::from_raw_microunits(-1_500_000))
    );
    assert_eq!(
        MicroScalar::MIN.checked_neg(),
        Err(MicroScalarArithmeticError::Overflow)
    );
}

#[test]
fn semantic_digests_bind_exact_contract_and_canonical_value() {
    assert_eq!(
        InfoBool::TRUE.semantic_digest(),
        [
            0xf3, 0xfa, 0x03, 0xc9, 0xf7, 0x8e, 0x12, 0x2e, 0x57, 0x15, 0x5c, 0x17, 0x65, 0x9d,
            0xa4, 0xac, 0x32, 0x3b, 0x06, 0xd5, 0x45, 0xac, 0x1d, 0x05, 0x34, 0x47, 0x20, 0x37,
            0xe0, 0xe4, 0xdf, 0x81,
        ]
    );
    assert_ne!(
        InfoBool::FALSE.semantic_digest(),
        InfoBool::TRUE.semantic_digest()
    );
    assert_eq!(
        MicroScalar::ZERO.semantic_digest(),
        conduit_core::semantic_digest(
            conduit_core::MICRO_SCALAR_INFO_ID,
            &MicroScalar::ZERO.encode()
        )
    );
    assert_ne!(
        InfoBool::FALSE.semantic_digest(),
        MicroScalar::ZERO.semantic_digest()
    );
}
