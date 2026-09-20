use conduit_core::{
    InfoBool, InfoDecodeError, Scalar, ScalarArithmeticError, BOOL_ENCODED_LEN, BOOL_INFO_ID,
    SCALAR_ENCODED_LEN, SCALAR_INFO_ID,
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
fn scalar_contract_is_exact_signed_fixed_point() {
    assert_eq!(SCALAR_INFO_ID, "value/scalar");
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
        Scalar::ZERO.semantic_digest(),
        [
            0xa6, 0x3d, 0x80, 0xfb, 0x49, 0x63, 0x22, 0x1a, 0xe7, 0x4f, 0xda, 0xc6, 0x6b, 0x1e,
            0xb0, 0xca, 0x58, 0xf6, 0xf0, 0xca, 0xae, 0x28, 0x77, 0xc2, 0xbc, 0x6c, 0x5c, 0xcc,
            0xfd, 0x86, 0xc0, 0x36,
        ]
    );
    assert_ne!(
        InfoBool::FALSE.semantic_digest(),
        Scalar::ZERO.semantic_digest()
    );
}
