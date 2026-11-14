use conduit_core::*;

#[test]
fn reported_widths_are_enforced_by_the_original_primitive_decoders() {
    for identity in [
        EMPTY_INFO_ID,
        CANCELLATION_REQUEST_INFO_ID,
        BOOL_INFO_ID,
        COUNT_INFO_ID,
        F32_INFO_ID,
        F64_INFO_ID,
        "value/u8",
        "value/u16",
        "value/u32",
        "value/u64",
        "value/u128",
        "value/i8",
        "value/i16",
        "value/i32",
        "value/i64",
        "value/i128",
    ] {
        let width = fixed_primitive_info_payload_bytes(identity).unwrap();
        assert!(
            validate_primitive_info(identity, &vec![0; width]).is_ok(),
            "{identity}"
        );
        assert!(
            validate_primitive_info(identity, &vec![0; width + 1]).is_err(),
            "{identity}"
        );
        if width > 0 {
            assert!(
                validate_primitive_info(identity, &vec![0; width - 1]).is_err(),
                "{identity}"
            );
        }
    }
    for identity in [
        TEXT_INFO_ID,
        BYTES_INFO_ID,
        "domain/unknown",
        "value/scalar",
    ] {
        assert_eq!(fixed_primitive_info_payload_bytes(identity), None);
    }
}
