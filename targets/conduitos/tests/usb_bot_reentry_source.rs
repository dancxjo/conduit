//! Checked Source-only bounded reentry policy; no native/class execution proof.
use conduit_core::*;
#[path = "usb_bot_reentry/fixture.rs"]
mod fixture;
use fixture::*;
#[test]
fn checked_reentry_has_two_storage_pairs_but_one_original_usb_operation_each() {
    let source = source();
    let permit = program(&source, "bot-reentry-permit");
    let contract =
        conduitos::storage_base::transaction_contract::BlockReadTransactionContract::prepare()
            .unwrap();
    assert_eq!(
        &permit.input_type,
        contract.permit_type(),
        "checked identity alias preserves the actual authorize result Type"
    );
    let expanded = source.expand("bot-storage-read-reentry").unwrap();
    for (kind, count) in [
        ("storage/block-read/authorize", 2),
        ("storage/block-read/settle", 2),
        ("machine/usb/control", 1),
        ("machine/usb/endpoint-read", 1),
        ("machine/usb/endpoint-write", 1),
    ] {
        assert_eq!(
            expanded
                .expanded
                .gears
                .iter()
                .filter(|g| g.kind_id.as_str() == kind)
                .count(),
            count
        );
    }
    for role in [
        "first-authorize",
        "first-settle",
        "second-authorize",
        "second-settle",
    ] {
        let selected = expanded
            .expanded
            .provenance
            .iter()
            .filter(|p| p.source_gear == role)
            .collect::<Vec<_>>();
        assert_eq!(
            selected.len(),
            1,
            "Root must explicitly map {role} to its matching independently issued pair"
        );
        assert_eq!(
            selected[0].gear_id,
            format!("bot-storage-read-reentry/{role}")
        );
    }
    assert_eq!(expanded.front.inputs().len(), 1);
    assert_eq!(expanded.front.outputs().len(), 1);
    assert_eq!(
        expanded.front.outputs()[0].value_kind,
        contract.settle_kind().outputs[0].value_kind,
        "both genuine settle results retain the exact semantic result Type"
    );

    let accept = program(&source, "bot-reentry-accept");
    // Actual canonical fixture extent, not merely a declared specialization.
    let state = state(fields(&accept.input_type)[0].value_type(), 14);
    assert!(state.canonical_bytes().unwrap().len() <= 4096);
}
#[test]
fn phase_guards_keep_first_submission_until_three_acks_and_original_settlement() {
    let source = source();
    let accept = program(&source, "bot-reentry-accept");
    let event_type = fields(&accept.input_type)[1].value_type();
    let permit = variant(event_type, "permit", |p| identity(p, 0));
    assert!(accepted(&accept, 0, permit.clone()));
    assert!(accepted(&accept, 10, permit));
    let stalled = read_event(&source, event_type, None, 0, false);
    assert!(accepted(&accept, 5, stalled.clone()));
    for phase in [4, 6, 7, 8, 10, 13, 14] {
        assert!(!accepted(&accept, phase, stalled.clone()));
    }
    let settled = settled_event(&source, false);
    for phase in [5, 6, 7, 8, 10, 11, 15] {
        assert!(!accepted(&accept, phase, settled.clone()));
    }
    assert!(accepted(&accept, 9, settled));
    for phase in [3, 6, 7, 8] {
        for ack in [true, false] {
            let event = variant(event_type, "control", |p| {
                record(p, |f| boolean(f.value_type(), f.name() == "ack" && ack))
            });
            assert_eq!(accepted(&accept, phase, event), ack);
        }
    }
    for (name, value) in [("block_size", 1024), ("count", 2), ("lba", 1)] {
        let wrong = variant(event_type, "command", |p| {
            record(p, |f| {
                if f.name() == name {
                    scalar(f.value_type(), value)
                } else {
                    scalar(
                        f.value_type(),
                        match f.name() {
                            "block_size" => 512,
                            "count" => 1,
                            _ => 0,
                        },
                    )
                }
            })
        });
        for phase in [0, 10] {
            assert!(!accepted(&accept, phase, wrong.clone()));
        }
    }
    let wrong = variant(event_type, "permit", |p| identity(p, 1));
    for phase in [0, 10] {
        assert!(!accepted(&accept, phase, wrong.clone()));
    }
}
#[test]
fn fresh_data_and_csw_use_native_ordinals_one_two_and_new_tag_two() {
    let source = source();
    let accept = program(&source, "bot-reentry-accept");
    let event_type = fields(&accept.input_type)[1].value_type();
    for (ordinal, length, short, expected) in [
        (1, 512, false, true),
        (0, 512, false, false),
        (2, 512, false, false),
        (1, 511, true, false),
        (1, 512, true, false),
    ] {
        assert_eq!(
            accepted(
                &accept,
                13,
                read_event(
                    &source,
                    event_type,
                    Some(&vec![0x80; length]),
                    ordinal,
                    short
                )
            ),
            expected
        );
    }
    let mut csw = *b"USBS\x02\0\0\0\0\0\0\0\0";
    assert!(accepted(
        &accept,
        14,
        read_event(&source, event_type, Some(&csw), 2, false)
    ));
    for ordinal in [0, 1, 3] {
        assert!(!accepted(
            &accept,
            14,
            read_event(&source, event_type, Some(&csw), ordinal, false)
        ));
    }
    for index in 0..13 {
        let old = csw[index];
        csw[index] = old.wrapping_add(1);
        assert!(
            !accepted(
                &accept,
                14,
                read_event(&source, event_type, Some(&csw), 2, false)
            ),
            "CSW octet {index}"
        );
        csw[index] = old;
    }
}
#[test]
fn recovery_wire_order_and_cbw_tags_are_source_owned_without_replay() {
    let source = source();
    let control = program(&source, "bot-reentry-control-request");
    for (phase, expected) in [
        (6, [0x21, 0xff, 0, 0, 0, 0, 0, 0]),
        (7, [2, 1, 0, 0, 0x81, 0, 0, 0]),
        (8, [2, 1, 0, 0, 2, 0, 0, 0]),
    ] {
        let out = evaluate(&control, &state(&control.input_type, phase))
            .canonical_bytes()
            .unwrap();
        assert_eq!(
            validate_canonical_structured_value(&out)
                .unwrap()
                .record_field("setup")
                .unwrap()
                .unwrap()
                .primitive_bytes("value/u64")
                .unwrap(),
            expected
        );
    }
    let write = program(&source, "bot-reentry-write-request");
    let mut previous = None;
    for (phase, tag) in [(4, 1), (12, 2)] {
        let out = evaluate(&write, &state(&write.input_type, phase))
            .canonical_bytes()
            .unwrap();
        let value = validate_canonical_structured_value(&out).unwrap();
        let wire = value
            .record_field("wire")
            .unwrap()
            .unwrap()
            .primitive_bytes("value/bytes")
            .unwrap();
        assert_eq!(wire.len(), 31);
        assert_eq!(&wire[..4], b"USBC");
        assert_eq!(u32::from_le_bytes(wire[4..8].try_into().unwrap()), tag);
        if let Some(previous) = previous {
            assert_ne!(wire, previous);
        }
        previous = Some(wire.to_vec());
    }
}
#[test]
fn failures_drain_without_recovery_effects_or_duplicate_settlement_emissions() {
    let source = source();
    let accept = program(&source, "bot-reentry-accept");
    let advance = program(&source, "bot-reentry-advance");
    let route = program(&source, "bot-reentry-route");
    let emission = program(&source, "bot-reentry-emission");
    let event_type = fields(&accept.input_type)[1].value_type();
    let command = variant(event_type, "command", |p| identity(p, 0));
    for phase in [1, 2, 3, 4, 5, 6, 7, 8, 9, 11, 12, 13, 14, 15] {
        let transition = evaluate(&accept, &pair(&accept.input_type, phase, command.clone()));
        let next = evaluate(&advance, &transition);
        let routed = evaluate(&route, &next).canonical_bytes().unwrap();
        assert_eq!(
            validate_canonical_structured_value(&routed)
                .unwrap()
                .variant_tag()
                .unwrap(),
            "ready"
        );
        let next_bytes = next.canonical_bytes().unwrap();
        let next_ref = validate_canonical_structured_value(&next_bytes).unwrap();
        assert_eq!(
            next_ref
                .record_field("phase")
                .unwrap()
                .unwrap()
                .primitive_bytes("value/u64")
                .unwrap(),
            16u64.to_le_bytes()
        );
        assert_eq!(
            next_ref
                .record_field("terminal")
                .unwrap()
                .unwrap()
                .primitive_bytes("value/bool")
                .unwrap(),
            [0]
        );
        let emitted = evaluate(&emission, &transition).canonical_bytes().unwrap();
        assert_eq!(
            validate_canonical_structured_value(&emitted)
                .unwrap()
                .variant_tag()
                .unwrap(),
            "absent"
        );
    }
    let closed = unit(event_type, "closed");
    assert!(accepted(&accept, 16, closed));
    assert!(accepted(&accept, 16, command));
}

#[test]
fn recovery_controls_require_actual_zero_data_completion_facts() {
    let source = source();
    let observe = program(&source, "bot-reentry-control-event");
    for (transferred, short, input, fault, expected) in [
        (0, false, vec![], None, true),
        (1, false, vec![], None, false),
        (0, true, vec![], None, false),
        (0, false, vec![1], None, false),
        (0, false, vec![], Some("stalled"), false),
        (0, false, vec![], Some("provider-lost"), false),
    ] {
        let result = if let Some(tag) = fault {
            unit(&observe.input_type, tag)
        } else {
            variant(&observe.input_type, "completed", |completion| {
                record(completion, |f| match f.name() {
                    "transferred" => StructuredInfoValue::leaf(
                        f.value_type().clone(),
                        (transferred as u16).to_le_bytes().to_vec(),
                    )
                    .unwrap(),
                    "short" => boolean(f.value_type(), short),
                    "input" => {
                        let StructuredInfoTypeShape::Sequence { element, .. } =
                            f.value_type().shape()
                        else {
                            panic!("sequence")
                        };
                        StructuredInfoValue::sequence(
                            f.value_type().clone(),
                            input
                                .iter()
                                .map(|byte| {
                                    StructuredInfoValue::leaf(element.clone(), vec![*byte]).unwrap()
                                })
                                .collect(),
                        )
                        .unwrap()
                    }
                    _ => panic!("control"),
                })
            })
        };
        let observed = evaluate(&observe, &result).canonical_bytes().unwrap();
        let facts = validate_canonical_structured_value(&observed)
            .unwrap()
            .variant_payload("control")
            .unwrap()
            .unwrap();
        assert_eq!(
            facts
                .record_field("ack")
                .unwrap()
                .unwrap()
                .primitive_bytes("value/bool")
                .unwrap(),
            [u8::from(expected)]
        );
    }
}
#[test]
fn candidate_identity_uses_retained_semantic_permit_not_cbw_tag_or_native_ordinal() {
    let source = source();
    for (name, phase, tag) in [
        ("bot-reentry-first-candidate", 9, "stalled"),
        ("bot-reentry-second-candidate", 15, "completed"),
    ] {
        let candidate = program(&source, name);
        let default = state(&candidate.input_type, phase);
        let input = record(&candidate.input_type, |f| {
            if f.name() == "permit" {
                identity(f.value_type(), 7)
            } else {
                let StructuredInfoValueShape::Record(values) = default.shape() else {
                    panic!("state record")
                };
                values
                    .iter()
                    .find(|value| value.name() == f.name())
                    .unwrap()
                    .value()
                    .clone()
            }
        });
        let bytes = evaluate(&candidate, &input).canonical_bytes().unwrap();
        let result = validate_canonical_structured_value(&bytes).unwrap();
        assert_eq!(result.variant_tag().unwrap(), tag);
        assert_eq!(
            result
                .variant_payload(tag)
                .unwrap()
                .unwrap()
                .record_field("ordinal")
                .unwrap()
                .unwrap()
                .primitive_bytes("value/u64")
                .unwrap(),
            7u64.to_le_bytes()
        );
    }
}

#[test]
fn complete_two_caller_phase_path_settles_each_identity_once_before_unlocking_next() {
    let source = source();
    let accept = program(&source, "bot-reentry-accept");
    let advance = program(&source, "bot-reentry-advance");
    let emit = program(&source, "bot-reentry-emission");
    let event_type = fields(&accept.input_type)[1].value_type();
    for phase in [0u64, 2, 3, 4, 5, 6, 7, 8, 9, 10, 12, 13, 14, 15] {
        let event = match phase {
            0 | 10 => variant(event_type, "permit", |p| identity(p, 0)),
            2 | 3 | 6 | 7 | 8 => variant(event_type, "control", |p| {
                record(p, |f| {
                    boolean(
                        f.value_type(),
                        if phase == 2 {
                            f.name() == "descriptor"
                        } else {
                            f.name() == "ack"
                        },
                    )
                })
            }),
            4 | 12 => variant(event_type, "write", |p| {
                variant(p, "completed", |v| {
                    record(v, |f| scalar(f.value_type(), 31))
                })
            }),
            5 => read_event(&source, event_type, None, 0, false),
            9 => settled_event(&source, false),
            13 => read_event(&source, event_type, Some(&[0x80; 512]), 1, false),
            14 => read_event(
                &source,
                event_type,
                Some(b"USBS\x02\0\0\0\0\0\0\0\0"),
                2,
                false,
            ),
            15 => settled_event(&source, true),
            _ => unreachable!(),
        };
        assert!(accepted(&accept, phase, event.clone()), "phase {phase}");
        let transition = evaluate(&accept, &pair(&accept.input_type, phase, event));
        let advanced = evaluate(&advance, &transition).canonical_bytes().unwrap();
        assert_eq!(
            validate_canonical_structured_value(&advanced)
                .unwrap()
                .record_field("phase")
                .unwrap()
                .unwrap()
                .primitive_bytes("value/u64")
                .unwrap(),
            (match phase {
                0 => 2u64,
                10 => 12,
                _ => phase + 1,
            })
            .to_le_bytes()
        );
        let emitted = evaluate(&emit, &transition).canonical_bytes().unwrap();
        assert_eq!(
            validate_canonical_structured_value(&emitted)
                .unwrap()
                .variant_tag()
                .unwrap(),
            if phase == 9 || phase == 15 {
                "emit"
            } else {
                "absent"
            }
        );
    }
}

#[path = "usb_bot_reentry/refinement.rs"]
mod refinement;

#[path = "usb_bot_reentry/ingress.rs"]
mod ingress;
