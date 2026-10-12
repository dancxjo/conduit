//! Adversarial checked Source command/data/status correlation; no USB device.
use super::*;

pub(super) fn frame(
    ty: &StructuredInfoType,
    wire: &[u8],
    ordinal: u64,
    short: bool,
) -> StructuredInfoValue {
    variant(ty, "completed", |payload| {
        record(payload, |field| match field.name() {
            "wire" => StructuredInfoValue::leaf(field.value_type().clone(), wire.to_vec()).unwrap(),
            "short" => StructuredInfoValue::leaf(field.value_type().clone(), vec![u8::from(short)])
                .unwrap(),
            "actual" => scalar(field.value_type(), wire.len() as u64),
            "ordinal" => scalar(field.value_type(), ordinal),
            _ => unreachable!(),
        })
    })
}
fn csw(tag: u32, residue: u32, status: u8) -> Vec<u8> {
    let mut wire = b"USBS".to_vec();
    wire.extend(tag.to_le_bytes());
    wire.extend(residue.to_le_bytes());
    wire.push(status);
    wire
}

#[test]
fn status_correlation_rejects_stale_duplicate_and_contradictory_completions() {
    let package = package();
    let source_text = package.source.clone();
    let source = PreparedProtocolSource::prepare(package).unwrap();
    let observe = program(&source, "bot-storage-read-event");
    let accept = program(&source, "bot-storage-accept");
    let emit = program(&source, "bot-storage-emission");
    let pair_type = &accept.input_type;
    let state_type = fields(pair_type)[0].value_type();
    let event_type = fields(pair_type)[1].value_type();
    let bank_hex = source_text
        .split("bytes/literal(\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    let bank = conduit_data::bytes_literal::decode_hex(bank_hex).unwrap();
    let check = |ordinal: u64,
                 phase: u64,
                 data_size: usize,
                 wire: &[u8],
                 read_ordinal: u64,
                 short: bool,
                 expected: bool| {
        let data = vec![0x80; data_size];
        let state = record(state_type, |field| match field.name() {
            "bank" => StructuredInfoValue::leaf(field.value_type().clone(), bank.clone()).unwrap(),
            "command" => record(field.value_type(), |wire| {
                StructuredInfoValue::leaf(
                    wire.value_type().clone(),
                    bank[ordinal as usize * 31..ordinal as usize * 31 + 31].to_vec(),
                )
                .unwrap()
            }),
            "request" => record(field.value_type(), |request| {
                scalar(
                    request.value_type(),
                    match request.name() {
                        "block_size" => 512,
                        "count" => 1,
                        "lba" => 0,
                        _ => unreachable!(),
                    },
                )
            }),
            "data" => frame(field.value_type(), &data, ordinal * 2, data_size < 512),
            "phase" => scalar(field.value_type(), phase),
            "operations" => scalar(field.value_type(), ordinal),
            "terminal" => StructuredInfoValue::leaf(field.value_type().clone(), vec![0]).unwrap(),
            _ => unreachable!(),
        });
        let input = frame(&observe.input_type, wire, read_ordinal, short)
            .canonical_bytes()
            .unwrap();
        let observation =
            StructuredInfoValue::from_canonical_bytes(&observe.evaluate(&input).unwrap()).unwrap();
        let event = variant(event_type, "observation", |_| observation.clone());
        let pair = record(pair_type, |field| {
            if field.value_type() == state_type {
                state.clone()
            } else {
                event.clone()
            }
        });
        let transition = accept.evaluate(&pair.canonical_bytes().unwrap()).unwrap();
        assert_eq!(
            validate_canonical_structured_value(&transition)
                .unwrap()
                .record_field("accepted")
                .unwrap()
                .unwrap()
                .primitive_bytes("value/bool")
                .unwrap(),
            &[u8::from(expected)]
        );
        if expected {
            let output = emit.evaluate(&transition).unwrap();
            let value = validate_canonical_structured_value(&output).unwrap();
            let result = value.variant_payload("emit").unwrap().unwrap();
            assert_eq!(
                result.variant_tag().unwrap(),
                if data_size == 512 {
                    "completed"
                } else {
                    "incomplete"
                }
            );
        }
    };
    for ordinal in 0..16 {
        let tag = ordinal as u32 + 1;
        let receipt = ordinal * 2 + 1;
        check(ordinal, 5, 512, &csw(tag, 0, 0), receipt, false, true);
        check(ordinal, 5, 512, &csw(tag + 1, 0, 0), receipt, false, false);
        check(ordinal, 5, 512, &csw(tag, 0, 0), receipt + 1, false, false);
        check(ordinal, 6, 512, &csw(tag, 0, 0), receipt, false, false);
        for (residue, status) in [(1, 0), (512, 0), (768, 0), (0, 1), (0, 2), (0, 3)] {
            check(
                ordinal,
                5,
                512,
                &csw(tag, residue, status),
                receipt,
                false,
                false,
            );
        }
        check(ordinal, 5, 512, &csw(tag, 0, 0), receipt, true, false);
        let mut malformed = csw(tag, 0, 0);
        malformed[0] ^= 1;
        check(ordinal, 5, 512, &malformed, receipt, false, false);
        check(ordinal, 5, 500, &csw(tag, 12, 0), receipt, false, true);
        check(ordinal, 5, 500, &csw(tag, 0, 0), receipt, false, false);
    }
}
