use conduit_core::{
    kind_id, Quantity, QuantityUnit, StructuredInfoType, StructuredInfoValue,
    StructuredInfoValueShape, QUANTITY_INFO_ID,
};
use conduit_net::*;

fn record_parts() -> (String, Vec<u8>) {
    let value = StructuredInfoValue::leaf(
        StructuredInfoType::leaf(kind_id("value/text@1")).unwrap(),
        b"CALLING".to_vec(),
    )
    .unwrap();
    let value_kind = value
        .value_type()
        .profile()
        .unwrap()
        .value_kind()
        .as_str()
        .to_string();
    (value_kind, value.canonical_bytes().unwrap())
}

fn encoded() -> ([u8; MAXIMUM_TYPED_RECORD_FRAME_BYTES], usize) {
    let mut bytes = [0_u8; MAXIMUM_TYPED_RECORD_FRAME_BYTES];
    let (value_kind, payload) = record_parts();
    let record = TypedRecordRef::new(&value_kind, &payload).unwrap();
    let length = encode_typed_record_into(record, &mut bytes).unwrap();
    (bytes, length)
}

fn quantity_record_parts() -> (String, Vec<u8>) {
    let value = StructuredInfoValue::leaf(
        StructuredInfoType::leaf(kind_id(QUANTITY_INFO_ID)).unwrap(),
        Quantity::new(42, QuantityUnit::Millivolt).encode().to_vec(),
    )
    .unwrap();
    let value_kind = value
        .value_type()
        .profile()
        .unwrap()
        .value_kind()
        .as_str()
        .to_string();
    (value_kind, value.canonical_bytes().unwrap())
}

#[test]
fn bounded_frame_round_trips_exact_type_and_payload_in_caller_storage() {
    let (bytes, length) = encoded();
    let decoded = decode_typed_record(&bytes[..length]).unwrap();
    let (value_kind, payload) = record_parts();
    let expected = TypedRecordRef::new(&value_kind, &payload).unwrap();
    assert_eq!(decoded.value_kind(), expected.value_kind());
    assert_eq!(decoded.payload(), expected.payload());
    assert_eq!(
        length,
        TYPED_RECORD_FRAME_HEADER_BYTES + expected.value_kind().len() + expected.payload().len()
    );
}

#[test]
fn same_framing_contract_carries_a_non_text_quantity_record() {
    let (text_kind, _) = record_parts();
    let (quantity_kind, quantity_payload) = quantity_record_parts();
    assert_ne!(quantity_kind, text_kind);
    let mut frame = [0_u8; MAXIMUM_TYPED_RECORD_FRAME_BYTES];
    let length = encode_typed_record_into(
        TypedRecordRef::new(&quantity_kind, &quantity_payload).unwrap(),
        &mut frame,
    )
    .unwrap();
    let decoded = decode_typed_record(&frame[..length]).unwrap();
    assert_eq!(decoded.value_kind(), quantity_kind);
    let value = StructuredInfoValue::from_canonical_bytes(decoded.payload()).unwrap();
    let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
        panic!("quantity record must remain a leaf")
    };
    assert_eq!(
        Quantity::decode(bytes).unwrap(),
        Quantity::new(42, QuantityUnit::Millivolt)
    );
}

#[test]
fn incomplete_malformed_version_integrity_and_trailing_frames_stay_distinct() {
    let (bytes, length) = encoded();
    assert_eq!(
        decode_typed_record(&bytes[..length - 1]),
        Err(TypedRecordFrameRefusal::Truncated)
    );

    let mut malformed = bytes;
    malformed[0] = b'X';
    assert_eq!(
        decode_typed_record(&malformed[..length]),
        Err(TypedRecordFrameRefusal::InvalidMagic)
    );

    let mut unsupported = bytes;
    unsupported[4] = TYPED_RECORD_FRAME_VERSION + 1;
    assert_eq!(
        decode_typed_record(&unsupported[..length]),
        Err(TypedRecordFrameRefusal::UnsupportedVersion)
    );

    let mut corrupt = bytes;
    corrupt[length - 1] ^= 1;
    assert_eq!(
        decode_typed_record(&corrupt[..length]),
        Err(TypedRecordFrameRefusal::IntegrityMismatch)
    );

    let mut rebound = bytes;
    let original_kind_len = u16::from_le_bytes([rebound[5], rebound[6]]);
    let original_payload_len =
        u32::from_le_bytes([rebound[7], rebound[8], rebound[9], rebound[10]]);
    rebound[5..7].copy_from_slice(&(original_kind_len - 1).to_le_bytes());
    rebound[7..11].copy_from_slice(&(original_payload_len + 1).to_le_bytes());
    assert_eq!(
        decode_typed_record(&rebound[..length]),
        Err(TypedRecordFrameRefusal::IntegrityMismatch)
    );

    let mut invalid_kind = bytes;
    invalid_kind[11] = 0xff;
    assert_eq!(
        decode_typed_record(&invalid_kind[..length]),
        Err(TypedRecordFrameRefusal::InvalidValueKindEncoding)
    );

    let mut trailing = bytes;
    trailing[length] = 1;
    assert_eq!(
        decode_typed_record(&trailing[..length + 1]),
        Err(TypedRecordFrameRefusal::TrailingBytes)
    );
}

#[test]
fn type_payload_and_output_bounds_refuse_before_writing_past_capacity() {
    let (value_kind, payload) = record_parts();
    assert_eq!(
        TypedRecordRef::new("", &payload),
        Err(TypedRecordFrameRefusal::MissingValueKind)
    );
    let oversized_payload = [0_u8; MAXIMUM_TYPED_RECORD_PAYLOAD_BYTES + 1];
    assert_eq!(
        TypedRecordRef::new("value/bytes@1", &oversized_payload),
        Err(TypedRecordFrameRefusal::PayloadTooLarge)
    );
    assert_eq!(
        encode_typed_record_into(
            TypedRecordRef::new(&value_kind, &payload).unwrap(),
            &mut [0_u8; 8],
        ),
        Err(TypedRecordFrameRefusal::OutputTooSmall)
    );
    assert_eq!(
        TypedRecordRef::new("structured-info/wrong@1", &payload),
        Err(TypedRecordFrameRefusal::PayloadTypeMismatch)
    );
    assert_eq!(
        TypedRecordRef::new("value/text@1", b"not canonical"),
        Err(TypedRecordFrameRefusal::MalformedPayload)
    );
}

#[cfg(feature = "form-catalog")]
#[test]
fn framing_and_deframing_are_independent_reusable_checked_forms() {
    use conduit_form::{
        check_syntax_document, expand_canonical_form_for_authoring, parse_syntax_document,
        ProfileCatalog, StartupCatalog,
    };

    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_typed_record_catalogs(&mut startup, &mut profile).unwrap();
    for (name, source, kind) in [
        (
            "typed-record-frame",
            include_str!("../../../forms/typed-record-frame/main.conduit"),
            TYPED_RECORD_FRAME_KIND,
        ),
        (
            "typed-record-deframe",
            include_str!("../../../forms/typed-record-deframe/main.conduit"),
            TYPED_RECORD_DEFRAME_KIND,
        ),
    ] {
        let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
        let authored = expand_canonical_form_for_authoring(&checked, name, &profile).unwrap();
        assert_eq!(authored.input_bindings.len(), 1);
        assert_eq!(authored.output_bindings.len(), 1);
        assert_eq!(authored.expanded.gears[0].kind_id.as_str(), kind);
        for forbidden in ["WebSocket", "WebRTC", "HostId", "LineId", "Desk"] {
            assert!(!source.contains(forbidden));
        }
    }
}
