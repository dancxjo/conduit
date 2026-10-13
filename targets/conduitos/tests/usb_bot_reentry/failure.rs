//! Failed class recovery closes Source; only Root may retire the real lease.
use super::*;

#[test]
fn actual_typed_clear_in_stall_closes_without_settlement_or_fresh_authorization() {
    let source = source();
    let observe = program(&source, "bot-reentry-control-event");
    let accept = program(&source, "bot-reentry-accept");
    let advance = program(&source, "bot-reentry-advance");
    let emission = program(&source, "bot-reentry-emission");
    let authorize = program(&source, "bot-reentry-raw-authorization");
    let event = evaluate(&observe, &unit(&observe.input_type, "stalled"));
    let original = state(fields(&accept.input_type)[0].value_type(), 7);
    let transition = evaluate(&accept, &pair(&accept.input_type, 7, event));
    let next = evaluate(&advance, &transition);
    let before = original.canonical_bytes().unwrap();
    let after = next.canonical_bytes().unwrap();
    let before = validate_canonical_structured_value(&before).unwrap();
    let after = validate_canonical_structured_value(&after).unwrap();
    for field in ["request", "permit"] {
        assert_eq!(
            before.record_field(field).unwrap(),
            after.record_field(field).unwrap(),
            "original {field} remains unchanged; Source cannot release its actual storage lease"
        );
    }
    for flag in ["failed", "terminal"] {
        assert_eq!(
            after
                .record_field(flag)
                .unwrap()
                .unwrap()
                .primitive_bytes("value/bool")
                .unwrap(),
            [1]
        );
    }
    let marked = evaluate(&emission, &transition).canonical_bytes().unwrap();
    assert_eq!(
        validate_canonical_structured_value(&marked)
            .unwrap()
            .variant_tag()
            .unwrap(),
        "absent"
    );
    let members = fields(&authorize.input_type);
    let request = identity(members[1].value_type(), 0);
    let raw = record(&authorize.input_type, |field| {
        if field.name() == members[0].name() {
            next.clone()
        } else {
            request.clone()
        }
    });
    let routed = evaluate(&authorize, &raw).canonical_bytes().unwrap();
    assert_eq!(
        validate_canonical_structured_value(&routed)
            .unwrap()
            .variant_tag()
            .unwrap(),
        "absent",
        "queued caller request cannot authorize reentry after failed clear-IN"
    );
}
