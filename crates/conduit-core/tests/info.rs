use conduit_core::{
    InfoBool, InfoDecodeError, Scalar, ScalarArithmeticError, BOOL_ENCODED_LEN, BOOL_INFO_ID,
    SCALAR_ENCODED_LEN, SCALAR_INFO_ID,
};

#[test]
fn bool_contract_has_one_canonical_byte_per_state() {
    assert_eq!(BOOL_INFO_ID, "value/bool@1");
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
fn scalar_contract_is_exact_signed_fixed_point() {
    assert_eq!(SCALAR_INFO_ID, "value/scalar@1");
    assert_eq!(SCALAR_ENCODED_LEN, 8);
    assert_eq!(Scalar::SCALE, 1_000_000);

    for value in [
        Scalar::MIN,
        Scalar::from_raw_microunits(-1),
        Scalar::ZERO,
        Scalar::ONE,
        Scalar::MAX,
    ] {
        assert_eq!(Scalar::decode(&value.encode()), Ok(value));
    }
    assert!(Scalar::from_raw_microunits(-1) < Scalar::ZERO);
    assert!(Scalar::ONE < Scalar::MAX);
}

#[test]
fn scalar_refuses_every_non_exact_width() {
    for length in 0..16 {
        if length == SCALAR_ENCODED_LEN {
            continue;
        }
        let encoded = [0_u8; 16];
        assert_eq!(
            Scalar::decode(&encoded[..length]),
            Err(InfoDecodeError::WrongLength {
                expected: SCALAR_ENCODED_LEN,
                actual: length,
            })
        );
    }
}

#[test]
fn scalar_arithmetic_is_checked_and_multiplication_truncates_toward_zero() {
    let one_and_half = Scalar::from_raw_microunits(1_500_000);
    let two = Scalar::from_raw_microunits(2_000_000);
    assert_eq!(
        one_and_half.checked_mul(two),
        Ok(Scalar::from_raw_microunits(3_000_000))
    );
    assert_eq!(
        Scalar::from_raw_microunits(1).checked_mul(Scalar::from_raw_microunits(500_000)),
        Ok(Scalar::ZERO)
    );
    assert_eq!(
        Scalar::from_raw_microunits(-1).checked_mul(Scalar::from_raw_microunits(500_000)),
        Ok(Scalar::ZERO)
    );
    assert_eq!(
        Scalar::MAX.checked_add(Scalar::from_raw_microunits(1)),
        Err(ScalarArithmeticError::Overflow)
    );
    assert_eq!(
        Scalar::MIN.checked_sub(Scalar::from_raw_microunits(1)),
        Err(ScalarArithmeticError::Overflow)
    );
    assert_eq!(
        Scalar::MAX.checked_mul(Scalar::from_raw_microunits(1_000_001)),
        Err(ScalarArithmeticError::Overflow)
    );
}

#[test]
fn semantic_digests_bind_exact_contract_and_canonical_value() {
    assert_eq!(
        InfoBool::TRUE.semantic_digest(),
        [
            0xb8, 0x9b, 0x4b, 0xa7, 0xd3, 0x86, 0x0c, 0xd4, 0x8f, 0x58, 0x64, 0x74, 0x9d, 0x32,
            0x27, 0x11, 0x6f, 0xc2, 0x7c, 0x2e, 0x02, 0xa3, 0x84, 0x83, 0x4d, 0x50, 0xc5, 0x89,
            0xb3, 0x31, 0x03, 0x0b,
        ]
    );
    assert_ne!(
        InfoBool::FALSE.semantic_digest(),
        InfoBool::TRUE.semantic_digest()
    );
    assert_eq!(
        Scalar::ZERO.semantic_digest(),
        [
            0xfe, 0xc8, 0x2c, 0x67, 0x62, 0x30, 0xb0, 0x03, 0xe4, 0x2c, 0x58, 0xd4, 0xb7, 0x68,
            0x1d, 0x76, 0xf9, 0x7d, 0x45, 0x31, 0xef, 0x64, 0x56, 0x0b, 0x4b, 0x5c, 0xa7, 0x43,
            0x40, 0x81, 0x25, 0xcc,
        ]
    );
    assert_ne!(
        InfoBool::FALSE.semantic_digest(),
        Scalar::ZERO.semantic_digest()
    );
}
