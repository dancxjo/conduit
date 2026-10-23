#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
use conduit_core::{
    self as c, StructuredInfoType as Type, StructuredInfoTypeShape as Shape,
    StructuredInfoValue as Value,
};
use conduit_environment_runtime::publication::{
    PublicationBack, PublicationChannel as Channel, PublicationContract, PublicationSequence,
};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    Failure, FailureCode, PortId, ValueRef,
};
fn value(ty: &Type) -> Value {
    coordinates(ty, 20, 50)
}
fn coordinates(ty: &Type, temperature: i64, humidity: i64) -> Value {
    match ty.shape() {
        Shape::Leaf(kind) => {
            let bytes = match kind.as_str() {
                c::TEMPERATURE_INFO_ID => c::Quantity::new(temperature, c::Unit::Celsius)
                    .encode()
                    .to_vec(),
                c::RATIO_INFO_ID => c::Quantity::new(humidity, c::Unit::Percent)
                    .encode()
                    .to_vec(),
                c::EMPTY_INFO_ID => vec![],
                _ => vec![0],
            };
            Value::leaf(ty.clone(), bytes).unwrap()
        }
        Shape::Collection { element, length } => Value::collection(
            ty.clone(),
            (0..length)
                .map(|_| coordinates(element, temperature, humidity))
                .collect(),
        )
        .unwrap(),
        Shape::Record { fields, .. } => Value::record(
            ty.clone(),
            fields
                .iter()
                .map(|field| {
                    c::StructuredFieldValue::new(
                        field.name(),
                        coordinates(field.value_type(), temperature, humidity),
                    )
                    .unwrap()
                })
                .collect(),
        )
        .unwrap(),
        Shape::Variant { cases, .. } => Value::variant(
            ty.clone(),
            cases[0].tag(),
            coordinates(cases[0].payload_type(), temperature, humidity),
        )
        .unwrap(),
        _ => panic!("publication profile"),
    }
}
fn step(back: &mut PublicationBack, bytes: &[u8], declared: u32, fuel: u16) -> (StepOutcome, bool) {
    let reference = ValueRef {
        slot: 0,
        generation: 1,
        byte_len: declared,
    };
    let mut io = StepIo::test_frame([Some(reference)], [false], [None], None, fuel);
    let outcome = back.step(&mut io, &StepInputBytes::test_frame([Some(bytes)], None));
    (outcome, io.test_consumed(PortId(0)))
}
fn commit(back: &mut PublicationBack) {
    <PublicationBack as StepBack<1>>::step_committed(back);
}

#[test]
fn empty_publication_channels_prepare_within_one_kib() {
    for channel in [Channel::ProviderFailure, Channel::GeometryRefusal] {
        let contract = PublicationContract::prepare(channel).unwrap();
        let order = PublicationSequence::prepare(1);
        let (mut back, memory) =
            allocation_probe::observe(|| PublicationBack::prepare(&contract, 1, order).unwrap());
        assert!(memory.peak_bytes < 1024, "{channel:?}: {memory:?}");
        let bytes = value(contract.value_type()).canonical_bytes().unwrap();
        assert_eq!(
            step(&mut back, &bytes, bytes.len() as u32, 8).0,
            StepOutcome::Progress
        );
        assert!(back.snapshot().is_none());
        commit(&mut back);
        assert_eq!(back.snapshot().unwrap().canonical, bytes);
    }
}

#[test]
fn publication_storage_preserves_the_full_quantity_coordinate_width() {
    let contract = PublicationContract::prepare(Channel::Reading).unwrap();
    let values: Vec<_> = [(i64::MIN, i64::MIN), (i64::MAX, i64::MAX), (0, 100)]
        .into_iter()
        .map(|(temperature, humidity)| {
            coordinates(contract.value_type(), temperature, humidity)
                .canonical_bytes()
                .unwrap()
        })
        .collect();
    let mut back = PublicationBack::prepare(&contract, 3, PublicationSequence::prepare(3)).unwrap();
    let ((), memory) = allocation_probe::observe(|| {
        for bytes in &values {
            assert_eq!(
                step(&mut back, bytes, bytes.len() as u32, 8).0,
                StepOutcome::Progress
            );
            commit(&mut back);
            assert_eq!(back.snapshot().unwrap().canonical, bytes);
        }
    });
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
}
#[test]
fn publication_is_invisible_until_commit_and_uses_no_play_allocation() {
    for channel in Channel::ALL {
        let contract = PublicationContract::prepare(channel).unwrap();
        let bytes = value(contract.value_type()).canonical_bytes().unwrap();
        let mut back =
            PublicationBack::prepare(&contract, 2, PublicationSequence::prepare(2)).unwrap();
        assert!(back.snapshot().is_none());
        let ((), allocation) = allocation_probe::observe(|| {
            for sequence in 1..=2 {
                assert_eq!(
                    step(&mut back, &bytes, bytes.len() as u32, 8),
                    (StepOutcome::Progress, true)
                );
                assert_eq!(
                    back.snapshot().map(|s| s.sequence),
                    (sequence > 1).then_some(sequence - 1)
                );
                commit(&mut back);
                let snapshot = back.snapshot().unwrap();
                assert_eq!(snapshot.sequence, sequence);
                assert_eq!(snapshot.channel, channel);
                assert_eq!(snapshot.canonical, bytes);
                commit(&mut back);
                assert_eq!(back.snapshot().unwrap().sequence, sequence);
            }
            assert_eq!(
                step(&mut back, &bytes, bytes.len() as u32, 8),
                (
                    StepOutcome::Fail(Failure {
                        code: FailureCode::InvalidInput,
                        detail: 2
                    }),
                    false
                )
            );
            assert_eq!(back.snapshot().unwrap().sequence, 2);
            let mut closed = StepIo::test_frame([None], [true], [None], None, 8);
            assert_eq!(
                back.step(&mut closed, &StepInputBytes::test_frame([None], None)),
                StepOutcome::Complete
            );
            assert!(closed.test_consumed_closed(PortId(0)));
        });
        assert_eq!((allocation.allocations, allocation.reallocations), (0, 0));
    }
}
#[test]
fn malformed_wrong_typed_and_cancelled_publications_never_replace_current_state() {
    let contract = PublicationContract::prepare(Channel::Reading).unwrap();
    let bytes = value(contract.value_type()).canonical_bytes().unwrap();
    let wrong = value(
        PublicationContract::prepare(Channel::ProviderFailure)
            .unwrap()
            .value_type(),
    )
    .canonical_bytes()
    .unwrap();
    let mut back = PublicationBack::prepare(&contract, 4, PublicationSequence::prepare(4)).unwrap();
    for (input, declared) in [
        (&bytes[..bytes.len() - 1], (bytes.len() - 1) as u32),
        (wrong.as_slice(), wrong.len() as u32),
        (bytes.as_slice(), 1),
    ] {
        assert_eq!(
            step(&mut back, input, declared, 8),
            (
                StepOutcome::Fail(Failure {
                    code: FailureCode::InvalidInput,
                    detail: 4
                }),
                false
            )
        );
        commit(&mut back);
        assert!(back.snapshot().is_none());
    }
    assert_eq!(
        step(&mut back, &bytes, bytes.len() as u32, 0),
        (
            StepOutcome::Fail(Failure {
                code: FailureCode::InvalidLifecycle,
                detail: 2
            }),
            false
        )
    );
    assert_eq!(
        step(&mut back, &bytes, bytes.len() as u32, 8),
        (StepOutcome::Progress, true)
    );
    <PublicationBack as StepBack<1>>::cancel(&mut back);
    commit(&mut back);
    assert!(back.snapshot().is_none());
    assert_eq!(
        step(&mut back, &bytes, bytes.len() as u32, 8),
        (
            StepOutcome::Fail(Failure {
                code: FailureCode::Cancelled,
                detail: 1
            }),
            false
        )
    );
}
#[test]
fn zero_event_admission_refuses_values_but_still_closes_normally() {
    let contract = PublicationContract::prepare(Channel::ProviderFailure).unwrap();
    let bytes = value(contract.value_type()).canonical_bytes().unwrap();
    let mut back = PublicationBack::prepare(&contract, 0, PublicationSequence::prepare(0)).unwrap();
    assert_eq!(
        step(&mut back, &bytes, bytes.len() as u32, 8),
        (
            StepOutcome::Fail(Failure {
                code: FailureCode::InvalidInput,
                detail: 2
            }),
            false
        )
    );
    assert!(back.snapshot().is_none());
    let mut closed = StepIo::test_frame([None], [true], [None], None, 8);
    assert_eq!(
        back.step(&mut closed, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Complete
    );
}

#[test]
fn all_outcomes_share_commit_order_and_cancelled_reservations_return_capacity() {
    let order = PublicationSequence::prepare(4);
    let contracts = Channel::ALL.map(|channel| PublicationContract::prepare(channel).unwrap());
    let bytes = contracts
        .each_ref()
        .map(|contract| value(contract.value_type()).canonical_bytes().unwrap());
    let mut backs = contracts
        .each_ref()
        .map(|contract| PublicationBack::prepare(contract, 4, order.clone()).unwrap());
    // Stage in one order, commit in another: the committed order is authoritative.
    let ((), allocation) = allocation_probe::observe(|| {
        for i in 0..4 {
            assert_eq!(
                step(&mut backs[i], &bytes[i], bytes[i].len() as u32, 8),
                (StepOutcome::Progress, true)
            );
        }
        assert_eq!(order.committed(), 0);
        for (sequence, i) in [2, 0, 3, 1].into_iter().enumerate() {
            commit(&mut backs[i]);
            assert_eq!(backs[i].snapshot().unwrap().sequence, sequence as u64 + 1);
        }
        assert_eq!(order.committed(), 4);
        assert_eq!(
            step(&mut backs[0], &bytes[0], bytes[0].len() as u32, 8),
            (
                StepOutcome::Fail(Failure {
                    code: FailureCode::InvalidInput,
                    detail: 5
                }),
                false
            )
        );
        commit(&mut backs[0]);
        assert_eq!(order.committed(), 4);
    });
    assert_eq!((allocation.allocations, allocation.reallocations), (0, 0));

    let order = PublicationSequence::prepare(1);
    let mut abandoned = PublicationBack::prepare(&contracts[0], 1, order.clone()).unwrap();
    let mut retained = PublicationBack::prepare(&contracts[1], 1, order.clone()).unwrap();
    assert_eq!(
        step(&mut abandoned, &bytes[0], bytes[0].len() as u32, 8).0,
        StepOutcome::Progress
    );
    assert!(matches!(
        step(&mut retained, &bytes[1], bytes[1].len() as u32, 8).0,
        StepOutcome::Fail(_)
    ));
    <PublicationBack as StepBack<1>>::cancel(&mut abandoned);
    commit(&mut abandoned);
    assert_eq!(order.committed(), 0);
    assert_eq!(
        step(&mut retained, &bytes[1], bytes[1].len() as u32, 8).0,
        StepOutcome::Progress
    );
    // Drop of a staged Back must also release its reservation.
    drop(retained);
    let mut replacement = PublicationBack::prepare(&contracts[2], 1, order.clone()).unwrap();
    assert_eq!(
        step(&mut replacement, &bytes[2], bytes[2].len() as u32, 8).0,
        StepOutcome::Progress
    );
    commit(&mut replacement);
    assert_eq!(replacement.snapshot().unwrap().sequence, 1);
}

#[test]
fn committed_publications_record_and_recover_every_sensor_disposition() {
    use conduit_environment_runtime::{observation::ObservationContract, recording::*};
    let contract = ObservationContract::prepare().unwrap();
    let contracts = Channel::ALL.map(|channel| PublicationContract::prepare(channel).unwrap());
    let mut inputs: Vec<(usize, Vec<u8>)> = contracts
        .iter()
        .enumerate()
        .map(|(i, contract)| (i, value(contract.value_type()).canonical_bytes().unwrap()))
        .collect();
    let Shape::Variant { cases, .. } = contract.result_type().shape() else {
        panic!("result variant")
    };
    let failure_type = cases
        .iter()
        .find(|case| case.tag() == "unavailable")
        .unwrap()
        .payload_type();
    let Shape::Variant { cases, .. } = failure_type.shape() else {
        panic!("failure variant")
    };
    for tag in [
        "short-frame",
        "malformed-length",
        "checksum",
        "invalid-payload",
    ] {
        let ty = cases
            .iter()
            .find(|case| case.tag() == tag)
            .unwrap()
            .payload_type();
        let failure = Value::variant(failure_type.clone(), tag, value(ty)).unwrap();
        inputs.push((
            0,
            Value::variant(contract.result_type().clone(), "unavailable", failure)
                .unwrap()
                .canonical_bytes()
                .unwrap(),
        ));
    }
    let expected = [
        RecordingDisposition::Available,
        RecordingDisposition::CaptureTimeout,
        RecordingDisposition::ProviderFailed,
        RecordingDisposition::GeometryRefused,
        RecordingDisposition::ShortFrame,
        RecordingDisposition::MalformedLength,
        RecordingDisposition::Checksum,
        RecordingDisposition::InvalidPayload,
    ];
    let host = c::HostId::from("fixture/producer");
    let boot = c::BootId::from("fixture/boot");
    let plan = c::PlanId::from("fixture/plan");
    let play = c::ActivePlayId::from("fixture/play");
    let context = || RecordingContext {
        host: &host,
        boot: &boot,
        plan: &plan,
        play: &play,
    };
    let mut chunk = PreparedRecordingChunk::prepare(context(), &contract).unwrap();
    let reference = c::BoundedResourceRef {
        identity: c::ResourceSemanticIdentity::from_digest([1; 32]),
        content_profile: c::kind_id("text/json-utf8@1"),
        access_class: "resource/snapshot@1".into(),
        extent: c::ResourceExtent {
            bytes: PreparedRecordingSnapshot::content_bytes(context(), &contract, 8).unwrap()
                as u64,
            items: Some(1),
        },
        lifetime: c::ResourceLifetime {
            version: c::ResourceVersionIdentity::from_digest([2; 32]),
            expires_at: None,
        },
    };
    let mut snapshot =
        PreparedRecordingSnapshot::prepare(context(), &contract, 8, &reference).unwrap();

    let mut reader = PreparedRecordingReader::prepare(context(), &contract).unwrap();
    let mut input = PreparedRecordingInput::prepare(&contract).unwrap();
    let order = PublicationSequence::prepare(8);
    let mut backs = contracts
        .each_ref()
        .map(|contract| PublicationBack::prepare(contract, 8, order.clone()).unwrap());
    let ((), allocation) = allocation_probe::observe(|| {
        use conduit_environment_runtime::publication::PublicationSnapshot;
        assert!(matches!(
            input.append(
                &mut chunk,
                PublicationSnapshot {
                    channel: Channel::ProviderFailure,
                    sequence: 1,
                    canonical: &inputs[0].1,
                },
                0
            ),
            Err(RecordingRefusal::Canonical(_))
        ));
        assert!(chunk.is_empty());
        assert_eq!(
            input.append(
                &mut chunk,
                PublicationSnapshot {
                    channel: Channel::Reading,
                    sequence: 0,
                    canonical: &inputs[0].1,
                },
                0
            ),
            Err(RecordingRefusal::InvalidSequence)
        );
        assert!(chunk.is_empty());
        for (i, (channel, bytes)) in inputs.iter().enumerate() {
            let back = &mut backs[*channel];
            assert_eq!(
                step(back, bytes, bytes.len() as u32, 8).0,
                StepOutcome::Progress
            );
            commit(back);
            snapshot
                .append_publication(&mut input, back.snapshot().unwrap(), i as u64 * 60_000)
                .unwrap();
            assert_eq!(
                input
                    .append(&mut chunk, back.snapshot().unwrap(), i as u64 * 60_000)
                    .unwrap(),
                ChunkAppend::Buffered { gap_before: None }
            );
        }
        assert_eq!(snapshot.seal().unwrap(), chunk.seal().unwrap());
        reader.read(chunk.seal().unwrap()).unwrap();
        for (i, disposition) in expected.into_iter().enumerate() {
            let sample = reader.sample(i).unwrap();
            assert_eq!(sample.sequence, i as u64 + 1);
            assert_eq!(sample.disposition, disposition);
            assert_eq!(sample.sampled_at_boot_ms, i as u64 * 60_000);
            assert_eq!(sample.observation.is_some(), i == 0);
            assert_eq!(sample.gap_before, None);
        }
    });
    assert_eq!((allocation.allocations, allocation.reallocations), (0, 0));
}
