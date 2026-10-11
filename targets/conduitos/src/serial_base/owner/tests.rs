use super::*;
mod fixture;
use conduit_core::validate_canonical_structured_value;

#[test]
fn checked_semantic_octets_have_no_transport_or_utf8_interpretation() {
    let contract = SerialDuplexContract::prepare().unwrap();
    let decoder = PreparedSerialRequestDecoder::new(&contract).unwrap();
    let input = fixture::input("transmit", &[0, 255, 128, 10]);
    assert_eq!(
        decoder.decode(&input).unwrap(),
        SerialRequest::Transmit(&[0, 255, 128, 10])
    );
    assert!(decoder.decode(&input[..input.len() - 1]).is_err());
    for maximum in [0u64, 2049, u64::MAX] {
        assert_eq!(
            decoder.decode(&fixture::input("receive", &maximum.to_le_bytes())),
            Err(SerialRequestRefusal::Length)
        );
    }
    let oversized = fixture::input("transmit", &[0; 2049]);
    assert!(decoder.decode(&oversized).is_err());
}
#[test]
fn one_duplex_lease_preserves_partial_extents_and_rejects_replay_and_wrong_direction() {
    let mut owner = fixture::owner(7);
    let node = owner.node;
    let transmit = fixture::input("transmit", &[0, 255, 128, 10]);
    let submission = owner
        .begin(node, HostCallId(0), RequestId(0), &transmit)
        .unwrap();
    assert_eq!(
        submission.request(),
        &SerialRequest::Transmit(&[0, 255, 128, 10])
    );
    assert_eq!(
        owner
            .begin(node, HostCallId(0), RequestId(1), &transmit)
            .err(),
        Some(SerialOwnerRefusal::Pressure)
    );
    assert_eq!(
        unsafe {
            owner.finish_acknowledged(
                &submission,
                SerialObservation::Received {
                    actual: 1,
                    bytes: &[0],
                },
            )
        }
        .err(),
        Some(SerialOwnerRefusal::WrongBinding)
    );
    assert!(owner.pending.is_some());
    let encoded = unsafe {
        owner.finish_acknowledged(&submission, SerialObservation::Transmitted { actual: 2 })
    }
    .unwrap();
    let view = validate_canonical_structured_value(encoded).unwrap();
    assert_eq!(view.variant_tag().unwrap(), "transmitted");
    let payload = view.variant_payload("transmitted").unwrap().unwrap();
    assert_eq!(
        payload
            .record_field("actual")
            .unwrap()
            .unwrap()
            .primitive_bytes("value/u64")
            .unwrap(),
        2u64.to_le_bytes()
    );
    assert_eq!(
        owner
            .begin(node, HostCallId(0), RequestId(0), &transmit)
            .err(),
        Some(SerialOwnerRefusal::StaleRequest)
    );
    let receive = fixture::input("receive", &8u64.to_le_bytes());
    let submission = owner
        .begin(node, HostCallId(0), RequestId(1), &receive)
        .unwrap();
    assert!(unsafe {
        owner.finish_acknowledged(
            &submission,
            SerialObservation::Received {
                actual: 2,
                bytes: &[0],
            },
        )
    }
    .is_err());
    assert!(owner.pending.is_some());
    let result = unsafe {
        owner.finish_acknowledged(
            &submission,
            SerialObservation::Received {
                actual: 2,
                bytes: &[255, 128],
            },
        )
    }
    .unwrap();
    let payload = validate_canonical_structured_value(result)
        .unwrap()
        .variant_payload("received")
        .unwrap()
        .unwrap();
    assert_eq!(
        payload
            .record_field("short")
            .unwrap()
            .unwrap()
            .primitive_bytes("value/bool")
            .unwrap(),
        &[1]
    );
    assert_eq!(
        payload
            .record_field("ordinal")
            .unwrap()
            .unwrap()
            .primitive_bytes("value/u64")
            .unwrap(),
        1u64.to_le_bytes()
    );
}
#[test]
fn software_cancel_retains_possession_until_acknowledged_stop_and_refuses_late_publication() {
    let mut owner = fixture::owner(7);
    let node = owner.node;
    let input = fixture::input("receive", &8u64.to_le_bytes());
    let submission = owner
        .begin(node, HostCallId(0), RequestId(0), &input)
        .unwrap();
    owner.revoke(node, HostCallId(0)).unwrap();
    assert!(owner.pending.is_some());
    assert_eq!(
        owner
            .table
            .inspections()
            .next()
            .unwrap()
            .in_flight_operations,
        0
    );
    assert_eq!(
        owner.begin(node, HostCallId(0), RequestId(1), &input).err(),
        Some(SerialOwnerRefusal::Capability(
            BaseCapabilityRefusal::Revoked
        ))
    );
    let late = unsafe {
        owner.finish_acknowledged(
            &submission,
            SerialObservation::Disposition(SerialDisposition::ProviderLost),
        )
    };
    assert!(matches!(late, Err(SerialOwnerRefusal::Capability(_))));
    assert!(owner.pending.is_none());
    assert_eq!(
        owner
            .table
            .inspections()
            .next()
            .unwrap()
            .in_flight_operations,
        0
    );
    assert_eq!(
        unsafe { owner.discard_acknowledged(&submission) },
        Err(SerialOwnerRefusal::StaleRequest)
    );
}
#[test]
fn exact_plan_call_and_provider_generation_are_required() {
    let (fragment, mut lowered, active, placement) = fixture::selected();
    lowered.host_calls[0].binding.maximum_output_bytes += 1;
    assert_eq!(
        fixture::bind(
            SelectedOperationPlan {
                fragment: &fragment,
                lowered: &lowered,
                active: &active,
                placement_id: &placement
            },
            7
        )
        .err(),
        Some(SerialOwnerRefusal::WrongBinding)
    );
    let mut owner = fixture::owner(7);
    let node = owner.node;
    owner.claim.base_provider_generation += 1;
    let input = fixture::input("receive", &8u64.to_le_bytes());
    assert!(matches!(
        owner.begin(node, HostCallId(0), RequestId(0), &input),
        Err(SerialOwnerRefusal::Capability(_))
    ));
    assert!(owner.pending.is_none());
}

#[test]
fn repeated_duplex_octets_use_admitted_storage_without_play_allocations() {
    let mut owner = fixture::owner(7);
    let node = owner.node;
    let tx = fixture::input("transmit", &[255, 0, 128, 10]);
    let rx = fixture::input("receive", &8u64.to_le_bytes());
    let count = crate::test_allocations::allocations(|| {
        for sequence in 0..64u32 {
            let input = if sequence % 2 == 0 { &tx } else { &rx };
            let submission = owner
                .begin(node, HostCallId(0), RequestId(sequence), input)
                .unwrap();
            let observation = if sequence % 2 == 0 {
                SerialObservation::Transmitted { actual: 4 }
            } else {
                SerialObservation::Received {
                    actual: 4,
                    bytes: &[255, 0, 128, 10],
                }
            };
            unsafe { owner.finish_acknowledged(&submission, observation) }.unwrap();
        }
    });
    assert_eq!(count, 0);
    assert_eq!(
        owner
            .table
            .inspections()
            .next()
            .unwrap()
            .completed_operations,
        64
    );
}
#[test]
fn acknowledged_terminal_observation_prevents_any_next_effect() {
    for disposition in [
        SerialDisposition::Stalled,
        SerialDisposition::ProviderLost,
        SerialDisposition::Unsupported,
        SerialDisposition::Timeout,
    ] {
        let mut owner = fixture::owner(7);
        let node = owner.node;
        let input = fixture::input("receive", &8u64.to_le_bytes());
        let submission = owner
            .begin(node, HostCallId(0), RequestId(0), &input)
            .unwrap();
        unsafe {
            owner.finish_acknowledged(&submission, SerialObservation::Disposition(disposition))
        }
        .unwrap();
        assert_eq!(
            owner.begin(node, HostCallId(0), RequestId(1), &input).err(),
            Some(SerialOwnerRefusal::Terminal)
        );
    }
}

#[test]
fn catalog_contract_preserves_bounded_byte_refinements_without_offering_a_provider() {
    let contract = SerialDuplexContract::prepare().unwrap();
    let (startup, _profile) = contract.catalogs();
    let syntax = conduit_plot::parse_syntax_document("with serial/duplex-bytes/request as SerialRequest\nplot identity (\n >> request: SerialRequest...|\n result: SerialRequest...| >>\n) = (.)");
    assert!(syntax.diagnostics.is_empty());
    assert!(conduit_plot::check_syntax_document(&syntax, &startup).is_ok());
}
