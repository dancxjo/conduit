//! Source-only phase-error recovery witness; no native/authority claim.
use super::*;

fn member<'a>(value: &'a StructuredInfoValue, name: &str) -> &'a StructuredInfoValue {
    let StructuredInfoValueShape::Record(values) = value.shape() else {
        panic!("record")
    };
    values
        .iter()
        .find(|field| field.name() == name)
        .unwrap()
        .value()
}
fn number(value: &StructuredInfoValue) -> u64 {
    let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
        panic!("scalar")
    };
    u64::from_le_bytes(bytes.try_into().unwrap())
}
fn step(
    source: &conduitos::protocol_source::PreparedProtocolSource,
    previous: &StructuredInfoValue,
    event: StructuredInfoValue,
    expected: bool,
) -> StructuredInfoValue {
    let accept = program(source, "bot-reentry-accept");
    let first_name = fields(&accept.input_type)[0].name();
    let pair = record(&accept.input_type, |field| {
        if field.name() == first_name {
            previous.clone()
        } else {
            event.clone()
        }
    });
    let transition = evaluate(&accept, &pair);
    let encoded = transition.canonical_bytes().unwrap();
    assert_eq!(
        validate_canonical_structured_value(&encoded)
            .unwrap()
            .record_field("accepted")
            .unwrap()
            .unwrap()
            .primitive_bytes("value/bool")
            .unwrap(),
        [u8::from(expected)],
        "phase {} failed {:?} terminal {:?}",
        number(member(previous, "phase")),
        member(previous, "failed").shape(),
        member(previous, "terminal").shape()
    );
    evaluate(&program(source, "bot-reentry-advance"), &transition)
}
fn csw(tag: u32, status: u8) -> Vec<u8> {
    let mut bytes = b"USBS".to_vec();
    bytes.extend(tag.to_le_bytes());
    bytes.extend(0u32.to_le_bytes());
    bytes.push(status);
    bytes
}

#[test]
fn phase_error_requires_exact_csw_and_three_acks_before_original_failed_settlement() {
    let source = source();
    let accept = program(&source, "bot-reentry-accept");
    let event = fields(&accept.input_type)[1].value_type();
    let first = state(fields(&accept.input_type)[0].value_type(), 5);
    let awaiting = step(
        &source,
        &first,
        read_event(&source, event, Some(&[0x80; 512]), 0, false),
        true,
    );
    assert_eq!(number(member(&awaiting, "phase")), 18);
    let route = evaluate(&program(&source, "bot-reentry-route"), &awaiting);
    assert_eq!(
        validate_canonical_structured_value(&route.canonical_bytes().unwrap())
            .unwrap()
            .variant_tag()
            .unwrap(),
        "read"
    );
    let request = evaluate(&program(&source, "bot-reentry-read-request"), &awaiting);
    assert_eq!(number(member(&request, "length")), 13);
    for (tag, status, ordinal, short) in [
        (2, 2, 1, false),
        (1, 0, 1, false),
        (1, 1, 1, false),
        (1, 2, 2, false),
        (1, 2, 1, true),
    ] {
        let stopped = step(
            &source,
            &awaiting,
            read_event(&source, event, Some(&csw(tag, status)), ordinal, short),
            false,
        );
        assert_eq!(number(member(&stopped, "phase")), 16);
    }
    let mut phase_error_csw = csw(1, 2);
    phase_error_csw[8..12].copy_from_slice(&0xdeadbeefu32.to_le_bytes());
    let mut current = step(
        &source,
        &awaiting,
        read_event(&source, event, Some(&phase_error_csw), 1, false),
        true,
    );
    assert_eq!(number(member(&current, "phase")), 6);
    for phase in 6..=8 {
        assert_eq!(number(member(&current, "phase")), phase);
        let refused = variant(event, "control", |payload| {
            record(payload, |field| boolean(field.value_type(), false))
        });
        let stopped = step(&source, &current, refused, false);
        assert_eq!(number(member(&stopped, "phase")), 16);
        let ack = variant(event, "control", |payload| {
            record(payload, |field| {
                boolean(field.value_type(), field.name() == "ack")
            })
        });
        current = step(&source, &current, ack, true);
    }
    assert_eq!(number(member(&current, "phase")), 9);
    let original = evaluate(&program(&source, "bot-reentry-first-candidate"), &current);
    assert_eq!(
        validate_canonical_structured_value(&original.canonical_bytes().unwrap())
            .unwrap()
            .variant_tag()
            .unwrap(),
        "command-failed"
    );
    let settled = evaluate(&program(&source, "bot-reentry-settled"), &original);
    current = step(&source, &current, settled, true);
    assert_eq!(number(member(&current, "phase")), 10);
    current = step(
        &source,
        &current,
        variant(event, "permit", |payload| identity(payload, 0)),
        true,
    );
    assert_eq!(number(member(&current, "phase")), 12);
    current = step(
        &source,
        &current,
        variant(event, "write", |payload| {
            variant(payload, "completed", |result| {
                record(result, |field| {
                    scalar(
                        field.value_type(),
                        if field.name() == "actual" { 31 } else { 0 },
                    )
                })
            })
        }),
        true,
    );
    current = step(
        &source,
        &current,
        read_event(&source, event, Some(&[0x80; 512]), 2, false),
        true,
    );
    for residue in [1u32, 256, 65536, 16777216] {
        let mut malformed = csw(2, 0);
        malformed[8..12].copy_from_slice(&residue.to_le_bytes());
        let refused = step(
            &source,
            &current,
            read_event(&source, event, Some(&malformed), 3, false),
            false,
        );
        assert_eq!(number(member(&refused, "phase")), 16);
    }
    current = step(
        &source,
        &current,
        read_event(&source, event, Some(&csw(2, 0)), 3, false),
        true,
    );
    assert_eq!(number(member(&current, "phase")), 15);
    let fresh = evaluate(&program(&source, "bot-reentry-second-candidate"), &current);
    assert_eq!(
        validate_canonical_structured_value(&fresh.canonical_bytes().unwrap())
            .unwrap()
            .variant_tag()
            .unwrap(),
        "completed"
    );
}
