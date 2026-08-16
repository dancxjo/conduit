use conduit_core::{
    ConnectionId, KindId, PlanId, StructuredFieldType, StructuredFieldValue,
    StructuredInfoTransportRefusal, StructuredInfoType, StructuredInfoValue,
};
use conduit_wire::{
    decode_envelope, encode_envelope, structured_connection_envelope,
    structured_value_from_envelope, StructuredWireRefusal, WireError,
};

const MAXIMUM_PAYLOAD_BYTES: u32 = 2_048;

fn count_type() -> StructuredInfoType {
    StructuredInfoType::leaf(KindId::from("value/count@1")).unwrap()
}

fn text_type() -> StructuredInfoType {
    StructuredInfoType::leaf(KindId::from("value/text@1")).unwrap()
}

fn midi() -> StructuredInfoValue {
    let value_type = StructuredInfoType::record(
        KindId::from("music/midi-note@1"),
        vec![
            StructuredFieldType::new("channel", count_type()).unwrap(),
            StructuredFieldType::new("velocity", count_type()).unwrap(),
        ],
    )
    .unwrap();
    StructuredInfoValue::record(
        value_type,
        vec![
            StructuredFieldValue::new(
                "channel",
                StructuredInfoValue::leaf(count_type(), 2_u64.to_le_bytes().to_vec()).unwrap(),
            )
            .unwrap(),
            StructuredFieldValue::new(
                "velocity",
                StructuredInfoValue::leaf(count_type(), 96_u64.to_le_bytes().to_vec()).unwrap(),
            )
            .unwrap(),
        ],
    )
    .unwrap()
}

fn extraction() -> StructuredInfoValue {
    let value_type = StructuredInfoType::record(
        KindId::from("llm/extraction@1"),
        vec![StructuredFieldType::new("label", text_type()).unwrap()],
    )
    .unwrap();
    StructuredInfoValue::record(
        value_type,
        vec![StructuredFieldValue::new(
            "label",
            StructuredInfoValue::leaf(text_type(), b"ready".to_vec()).unwrap(),
        )
        .unwrap()],
    )
    .unwrap()
}

#[test]
fn unrelated_structured_values_cross_the_same_exact_connection_envelope() {
    for (sequence, value) in [midi(), extraction()].into_iter().enumerate() {
        let envelope = structured_connection_envelope(
            PlanId::from("plan-structured"),
            ConnectionId::from("connection-structured"),
            sequence as u64,
            &value,
            MAXIMUM_PAYLOAD_BYTES,
        )
        .unwrap();
        assert_eq!(
            envelope.value_kind,
            *value.value_type().profile().unwrap().value_kind()
        );
        let frame = encode_envelope(&envelope, MAXIMUM_PAYLOAD_BYTES).unwrap();
        let decoded = decode_envelope(&frame, MAXIMUM_PAYLOAD_BYTES).unwrap();
        assert_eq!(
            structured_value_from_envelope(value.value_type(), &decoded, MAXIMUM_PAYLOAD_BYTES,)
                .unwrap(),
            value
        );
    }
}

#[test]
fn profile_size_malformed_and_mutated_identity_refuse_distinctly() {
    let value = midi();
    let envelope = structured_connection_envelope(
        PlanId::from("plan-structured"),
        ConnectionId::from("connection-structured"),
        0,
        &value,
        MAXIMUM_PAYLOAD_BYTES,
    )
    .unwrap();
    assert_eq!(
        structured_value_from_envelope(extraction().value_type(), &envelope, MAXIMUM_PAYLOAD_BYTES,),
        Err(StructuredWireRefusal::ProfileMismatch)
    );
    assert_eq!(
        structured_connection_envelope(
            PlanId::from("plan-structured"),
            ConnectionId::from("connection-structured"),
            0,
            &value,
            80,
        ),
        Err(StructuredWireRefusal::Structured(
            StructuredInfoTransportRefusal::SizeExhausted
        ))
    );

    let mut malformed = envelope.clone();
    malformed.payload.pop();
    assert_eq!(
        structured_value_from_envelope(value.value_type(), &malformed, MAXIMUM_PAYLOAD_BYTES),
        Err(StructuredWireRefusal::Structured(
            StructuredInfoTransportRefusal::MalformedRepresentation
        ))
    );
    let mut mutated = envelope.clone();
    *mutated.payload.last_mut().unwrap() ^= 1;
    assert_eq!(
        structured_value_from_envelope(value.value_type(), &mutated, MAXIMUM_PAYLOAD_BYTES),
        Err(StructuredWireRefusal::Structured(
            StructuredInfoTransportRefusal::ValueIdentityMismatch
        ))
    );

    let mut frame = encode_envelope(&envelope, MAXIMUM_PAYLOAD_BYTES).unwrap();
    frame.pop();
    assert_eq!(
        decode_envelope(&frame, MAXIMUM_PAYLOAD_BYTES),
        Err(WireError::TruncatedFrame)
    );
}
