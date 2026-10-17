use super::*;
#[test]
fn zero_and_actual_out_extent_preserve_the_exact_completed_contract() {
    let contract = EndpointWriteContract::prepare().unwrap();
    let validator =
        conduit_core::PreparedStructuredValueValidator::new(contract.result_type(), 4096).unwrap();
    let mut encoder = PreparedEndpointWriteResultEncoder::new(&contract).unwrap();
    for (requested, actual) in [(0, 0), (2048, 2048), (8, 4)] {
        let bytes = encoder.completed(requested, actual).unwrap();
        validator.validate(bytes).unwrap();
        let value = validate_canonical_structured_value(bytes).unwrap();
        let completed = value.variant_payload("completed").unwrap().unwrap();
        assert_eq!(
            completed
                .record_field("actual")
                .unwrap()
                .unwrap()
                .primitive_bytes("value/u64")
                .unwrap(),
            &u64::from(actual).to_le_bytes()
        );
    }
    assert_eq!(
        encoder.completed(0, 1),
        Err(EndpointWriteResultRefusal::ActualLength)
    );
    assert_eq!(
        encoder.completed(2049, 0),
        Err(EndpointWriteResultRefusal::DataEnvelope)
    );
}
#[test]
fn acknowledged_dispositions_are_canonical() {
    let contract = EndpointWriteContract::prepare().unwrap();
    let validator =
        conduit_core::PreparedStructuredValueValidator::new(contract.result_type(), 4096).unwrap();
    let mut encoder = PreparedEndpointWriteResultEncoder::new(&contract).unwrap();
    for (disposition, tag) in [
        (EndpointWriteDisposition::Stalled, "stalled"),
        (EndpointWriteDisposition::ProviderLost, "provider-lost"),
        (EndpointWriteDisposition::Unsupported, "unsupported"),
    ] {
        let bytes = encoder.disposition(disposition).unwrap();
        validator.validate(bytes).unwrap();
        assert_eq!(
            validate_canonical_structured_value(bytes)
                .unwrap()
                .variant_tag()
                .unwrap(),
            tag
        );
    }
}
