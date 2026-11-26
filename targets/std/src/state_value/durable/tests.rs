use super::*;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, HostCallDisposition, HostCallId, PortId, RequestId, ValueRef,
};

fn binding() -> DurableStateBinding {
    DurableStateBinding {
        body: "body/notebook".into(),
        state: StateId::from("note/current"),
        value_kind: KindId::from("Text"),
        maximum_value_bytes: conduit_data::MAXIMUM_DATA_TEXT_BYTES,
    }
}

#[test]
fn incomplete_candidate_is_ignored_and_commit_is_idempotent_but_not_ambiguous() {
    let mut host = DurableStateHost::new(binding()).unwrap();
    host.stage_candidate(1, b"A").unwrap();
    assert_eq!(
        host.recover(&binding()).unwrap().0,
        RecoveryDisposition::Absent
    );
    let receipt = host.commit_candidate().unwrap();
    assert_eq!(decode_receipt(&receipt), Some((1, digest(b"A"))));
    assert_eq!(host.commit(&binding(), 1, b"A").unwrap(), receipt);
    assert_eq!(
        host.commit(&binding(), 1, b"different"),
        Err(DurableStateRefusal::ConflictingDigest)
    );
    host.commit(&binding(), 2, b"B").unwrap();
    assert_eq!(
        host.recover(&binding()).unwrap().0,
        RecoveryDisposition::Recovered { generation: 2 }
    );
}

#[test]
fn recovery_distinguishes_absent_corrupt_incompatible_and_lost() {
    let mut host = DurableStateHost::new(binding()).unwrap();
    assert_eq!(
        host.recover(&binding()).unwrap().0,
        RecoveryDisposition::Absent
    );
    let mut other = binding();
    other.body = "body/other".into();
    assert_eq!(host.recover(&other), Err(DurableStateRefusal::Incompatible));
    host.commit(&binding(), 1, b"A").unwrap();
    host.corrupt_committed_value();
    assert_eq!(host.recover(&binding()), Err(DurableStateRefusal::Corrupt));

    let mut lost = DurableStateHost::new(binding()).unwrap();
    lost.commit(&binding(), 1, b"A").unwrap();
    lost.mark_committed_lost();
    assert_eq!(lost.recover(&binding()), Err(DurableStateRefusal::Lost));
}

#[test]
fn transition_waits_for_exact_commit_receipt_before_consuming_or_emitting() {
    let probe = ValueRef {
        slot: 9,
        generation: 1,
        byte_len: 1,
    };
    let initial = ValueRef {
        slot: 10,
        generation: 1,
        byte_len: 7,
    };
    let mut back = DurableStateBack::new(binding(), initial, probe).unwrap();
    let mut recover_request = StepIo::test_frame([None], [false], [Some(48)], None, 8);
    assert_eq!(
        back.step(
            &mut recover_request,
            &StepInputBytes::test_frame([None], None)
        ),
        StepOutcome::Progress
    );
    assert_eq!(
        recover_request.test_host_request().map(|item| item.1),
        Some(HostCallId(1))
    );

    let absent = encode_absent();
    let absent_ref = ValueRef {
        slot: 8,
        generation: 1,
        byte_len: absent.len() as u32,
    };
    let outcome = conduit_kernel::HostCallOutcome {
        disposition: HostCallDisposition::Completed,
        output: Some(BoundedValueRef::new(absent_ref, absent.len() as u32).unwrap()),
        failure: None,
    };
    let mut recovered = StepIo::test_frame(
        [None],
        [false],
        [Some(48)],
        Some((RequestId(0), outcome)),
        8,
    );
    assert_eq!(
        back.step(
            &mut recovered,
            &StepInputBytes::test_frame([None], Some(&absent))
        ),
        StepOutcome::Progress
    );
    assert_eq!(recovered.test_output(PortId(0)), Some(initial));

    let value = b"next";
    let value_ref = ValueRef {
        slot: 1,
        generation: 1,
        byte_len: value.len() as u32,
    };
    let mut requested = StepIo::test_frame([Some(value_ref)], [false], [Some(48)], None, 8);
    assert_eq!(
        back.step(
            &mut requested,
            &StepInputBytes::test_frame([Some(value)], None)
        ),
        StepOutcome::Progress
    );
    assert!(!requested.test_consumed(PortId(0)));
    assert!(requested.test_output(PortId(0)).is_none());

    let receipt = encode_receipt(1, digest(value));
    let receipt_ref = ValueRef {
        slot: 7,
        generation: 1,
        byte_len: receipt.len() as u32,
    };
    let completion = conduit_kernel::HostCallOutcome {
        disposition: HostCallDisposition::Completed,
        output: Some(BoundedValueRef::new(receipt_ref, receipt.len() as u32).unwrap()),
        failure: None,
    };
    let mut committed = StepIo::test_frame(
        [Some(value_ref)],
        [false],
        [Some(48)],
        Some((RequestId(2), completion)),
        8,
    );
    assert_eq!(
        back.step(
            &mut committed,
            &StepInputBytes::test_frame([Some(value)], Some(&receipt))
        ),
        StepOutcome::Progress
    );
    assert!(committed.test_consumed(PortId(0)));
    assert_eq!(committed.test_output(PortId(0)), Some(value_ref));
    assert_eq!(back.generation(), 1);
}

#[test]
fn four_kib_recovery_is_bound_to_metadata_and_forwards_the_hosted_value() {
    let probe = ValueRef {
        slot: 9,
        generation: 1,
        byte_len: 1,
    };
    let initial = ValueRef {
        slot: 10,
        generation: 1,
        byte_len: 0,
    };
    let mut back = DurableStateBack::new(binding(), initial, probe).unwrap();
    let mut metadata_request = StepIo::test_frame([None], [false], [Some(4096)], None, 8);
    assert_eq!(
        back.step(
            &mut metadata_request,
            &StepInputBytes::test_frame([None], None)
        ),
        StepOutcome::Progress
    );

    let value = vec![b'n'; conduit_data::MAXIMUM_DATA_TEXT_BYTES as usize];
    let mut metadata = Vec::with_capacity(RECOVERY_HEADER_BYTES);
    metadata.extend_from_slice(&[PROTOCOL_VERSION, RECOVERY_PRESENT]);
    metadata.extend_from_slice(&7_u64.to_be_bytes());
    metadata.extend_from_slice(&digest(&value));
    let metadata_ref = ValueRef {
        slot: 8,
        generation: 1,
        byte_len: metadata.len() as u32,
    };
    let metadata_outcome = conduit_kernel::HostCallOutcome {
        disposition: HostCallDisposition::Completed,
        output: Some(BoundedValueRef::new(metadata_ref, MAXIMUM_DURABLE_STATE_BYTES).unwrap()),
        failure: None,
    };
    let mut metadata_step = StepIo::test_frame(
        [None],
        [false],
        [Some(4096)],
        Some((RequestId(0), metadata_outcome)),
        8,
    );
    assert_eq!(
        back.step(
            &mut metadata_step,
            &StepInputBytes::test_frame([None], Some(&metadata))
        ),
        StepOutcome::Progress
    );
    assert!(metadata_step.test_host_completion_consumed());
    let (_, call, request_input) = metadata_step.test_host_request().unwrap();
    let mut calls = conduit_kernel::FixedHostCallBindings::<2>::new(2);
    calls
        .install(
            conduit_kernel::NodeId(0),
            conduit_kernel::HostCallBinding {
                call,
                maximum_input_bytes: RECOVERY_HEADER_BYTES as u32,
                maximum_output_bytes: MAXIMUM_DURABLE_STATE_BYTES,
            },
        )
        .unwrap();
    calls.seal().unwrap();
    assert_eq!(request_input.value.byte_len, 42);
    assert_eq!(request_input.admitted_bytes, 42);
    assert!(calls
        .admit_request(conduit_kernel::NodeId(0), call, request_input)
        .is_ok());
    // The original broad output envelope is not a legal 42-byte request.
    assert_eq!(
        calls.admit_request(
            conduit_kernel::NodeId(0),
            call,
            BoundedValueRef::new(metadata_ref, MAXIMUM_DURABLE_STATE_BYTES).unwrap()
        ),
        Err(conduit_kernel::ProtocolError::HostCallInputExceeded)
    );

    assert_eq!(
        metadata_step.test_host_request(),
        Some((
            RequestId(1),
            HostCallId(1),
            BoundedValueRef::new(metadata_ref, metadata.len() as u32).unwrap()
        ))
    );

    let value_ref = ValueRef {
        slot: 7,
        generation: 1,
        byte_len: value.len() as u32,
    };
    let value_outcome = conduit_kernel::HostCallOutcome {
        disposition: HostCallDisposition::Completed,
        output: Some(BoundedValueRef::new(value_ref, value.len() as u32).unwrap()),
        failure: None,
    };
    let mut value_step = StepIo::test_frame(
        [None],
        [false],
        [Some(4096)],
        Some((RequestId(1), value_outcome)),
        8,
    );
    assert_eq!(
        back.step(
            &mut value_step,
            &StepInputBytes::test_frame([None], Some(&value))
        ),
        StepOutcome::Progress
    );
    assert_eq!(value_step.test_output(PortId(0)), Some(value_ref));
    assert_eq!(value_step.test_discards()[0], Some(initial));
    assert_eq!(back.generation(), 7);
}

#[test]
fn durable_binding_refuses_more_than_the_canonical_text_bound() {
    let mut too_large = binding();
    too_large.maximum_value_bytes = conduit_data::MAXIMUM_DATA_TEXT_BYTES + 1;
    assert_eq!(
        too_large.validate(),
        Err(DurableStateRefusal::InvalidBinding)
    );
}

#[test]
fn mismatched_receipt_never_publishes_candidate() {
    let probe = ValueRef {
        slot: 9,
        generation: 1,
        byte_len: 1,
    };
    let initial = ValueRef {
        slot: 10,
        generation: 1,
        byte_len: 7,
    };
    let mut back = DurableStateBack::new(binding(), initial, probe).unwrap();
    let mut request = StepIo::test_frame([None], [false], [Some(48)], None, 8);
    back.step(&mut request, &StepInputBytes::test_frame([None], None));
    let absent = encode_absent();
    let absent_ref = ValueRef {
        slot: 8,
        generation: 1,
        byte_len: 2,
    };
    let recovery = conduit_kernel::HostCallOutcome {
        disposition: HostCallDisposition::Completed,
        output: Some(BoundedValueRef::new(absent_ref, 2).unwrap()),
        failure: None,
    };
    let mut recovered = StepIo::test_frame(
        [None],
        [false],
        [Some(48)],
        Some((RequestId(0), recovery)),
        8,
    );
    back.step(
        &mut recovered,
        &StepInputBytes::test_frame([None], Some(&absent)),
    );

    let value = b"next";
    let value_ref = ValueRef {
        slot: 1,
        generation: 1,
        byte_len: 4,
    };
    let mut requested = StepIo::test_frame([Some(value_ref)], [false], [Some(48)], None, 8);
    back.step(
        &mut requested,
        &StepInputBytes::test_frame([Some(value)], None),
    );
    let wrong = encode_receipt(1, digest(b"other"));
    let wrong_ref = ValueRef {
        slot: 7,
        generation: 1,
        byte_len: wrong.len() as u32,
    };
    let completion = conduit_kernel::HostCallOutcome {
        disposition: HostCallDisposition::Completed,
        output: Some(BoundedValueRef::new(wrong_ref, wrong.len() as u32).unwrap()),
        failure: None,
    };
    let mut refused = StepIo::test_frame(
        [Some(value_ref)],
        [false],
        [Some(48)],
        Some((RequestId(2), completion)),
        8,
    );
    assert!(matches!(
        back.step(
            &mut refused,
            &StepInputBytes::test_frame([Some(value)], Some(&wrong))
        ),
        StepOutcome::Fail(_)
    ));
    assert!(!refused.test_consumed(PortId(0)));
    assert!(refused.test_output(PortId(0)).is_none());
    assert_eq!(back.generation(), 0);
}
