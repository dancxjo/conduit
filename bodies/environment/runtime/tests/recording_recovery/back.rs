//! Production kernel and recovery Back. The numeric graph and external read
//! completions below are explicit CPU fixtures, not browser persistence proof.
use super::{allocation_probe, history};
use conduit_environment_runtime::{observation::ObservationContract, recording::*};
use conduit_kernel::{self as k, scheduler::*, ValueStorage};
fn back() -> RecordingRecoveryBack {
    RecordingRecoveryBack::prepare(
        history::context(),
        &ObservationContract::prepare().unwrap(),
        2,
    )
    .unwrap()
}
fn offer(back: &mut RecordingRecoveryBack, bytes: &[u8], fuel: u16) -> StepOutcome {
    let value = k::ValueRef {
        slot: 0,
        generation: 1,
        byte_len: bytes.len() as u32,
    };
    back.step(
        &mut StepIo::test_frame([Some(value)], [false], [None], None, fuel),
        &StepInputBytes::test_frame([Some(bytes)], None),
    )
}
#[test]
fn only_committed_consumption_advances_recovery_and_cancellation_discards_staging() {
    let first = history::chunk(&[(1, 10), (2, 20)]);
    let next = history::chunk(&[(5, 30)]);
    let mut reader = back();
    let ((), allocations) = allocation_probe::observe(|| {
        assert_eq!(
            offer(&mut reader, first.as_bytes(), 16),
            StepOutcome::Progress
        );
        assert_eq!(reader.recovered(), RecoveredHistory::default());
        assert!(reader.sample(0).is_none());
        assert!(matches!(
            offer(&mut reader, b"{}", 16),
            StepOutcome::Fail(k::Failure {
                code: k::FailureCode::InvalidInput,
                ..
            })
        ));
        <RecordingRecoveryBack as StepBack<1>>::step_committed(&mut reader);
        assert_eq!(reader.recovered(), RecoveredHistory::default());
        // A discarded/uncommitted Step does not consume the chunk budget.
        assert_eq!(
            offer(&mut reader, first.as_bytes(), 16),
            StepOutcome::Progress
        );
        <RecordingRecoveryBack as StepBack<1>>::step_committed(&mut reader);
        assert_eq!(reader.recovered().samples, 2);
        assert_eq!(
            offer(&mut reader, next.as_bytes(), 0),
            StepOutcome::Fail(k::Failure {
                code: k::FailureCode::InvalidLifecycle,
                detail: 2311
            })
        );
        <RecordingRecoveryBack as StepBack<1>>::step_committed(&mut reader);
        assert_eq!(reader.recovered().last_sequence, Some(2));
        assert_eq!(
            offer(&mut reader, next.as_bytes(), 16),
            StepOutcome::Progress
        );
        assert_eq!(reader.sample(0).unwrap().sequence, 1);
        <RecordingRecoveryBack as StepBack<1>>::cancel(&mut reader);
        <RecordingRecoveryBack as StepBack<1>>::step_committed(&mut reader);
        assert_eq!(reader.recovered().last_sequence, Some(2));
        assert!(matches!(
            offer(&mut reader, next.as_bytes(), 16),
            StepOutcome::Fail(k::Failure {
                code: k::FailureCode::Cancelled,
                ..
            })
        ));
    });
    assert_eq!((allocations.allocations, allocations.reallocations), (0, 0));
}
#[test]
fn committed_chunks_expose_boundary_gaps_and_refusal_cannot_replace_them() {
    let mut reader = back();
    for json in [history::chunk(&[(1, 10)]), history::chunk(&[(4, 20)])] {
        assert_eq!(
            offer(&mut reader, json.as_bytes(), 16),
            StepOutcome::Progress
        );
        <RecordingRecoveryBack as StepBack<1>>::step_committed(&mut reader);
    }
    assert_eq!(reader.sample(0).unwrap().gap_before, Some((2, 3)));
    let before = reader.recovered();
    assert_eq!(
        offer(&mut reader, b"{}", 16),
        StepOutcome::Fail(k::Failure {
            code: k::FailureCode::StorageExhausted,
            detail: 2307
        })
    );
    <RecordingRecoveryBack as StepBack<1>>::step_committed(&mut reader);
    assert_eq!(reader.recovered(), before);
    assert_eq!(reader.sample(0).unwrap().gap_before, Some((2, 3)));
    assert_eq!(
        recovery_kind().inputs[0].value_kind,
        conduit_semantic_catalog::resource_snapshot_semantic_contract(false).outputs[0].value_kind
    );
}

enum Driver {
    Source { value: k::ValueRef, sent: bool },
    Read(HostCallBack),
    Recover(Box<RecordingRecoveryBack>),
}
impl StepBack<1> for Driver {
    fn step(&mut self, io: &mut StepIo<1>, bytes: &StepInputBytes<'_, 1>) -> StepOutcome {
        match self {
            Self::Read(back) => back.step(io, bytes),
            Self::Recover(back) => back.step(io, bytes),
            Self::Source { value, sent } => {
                if *sent {
                    return StepOutcome::Complete;
                }
                if !io.output_ready(k::PortId(0)) {
                    return StepOutcome::Await;
                }
                io.send(k::PortId(0), *value).unwrap();
                *sent = true;
                StepOutcome::Progress
            }
        }
    }
    fn step_committed(&mut self) {
        match self {
            Self::Read(b) => <HostCallBack as StepBack<1>>::step_committed(b),
            Self::Recover(b) => <RecordingRecoveryBack as StepBack<1>>::step_committed(b),
            _ => {}
        }
    }
    fn cancel(&mut self) {
        match self {
            Self::Read(b) => <HostCallBack as StepBack<1>>::cancel(b),
            Self::Recover(b) => <RecordingRecoveryBack as StepBack<1>>::cancel(b),
            _ => {}
        }
    }
}
#[test]
fn kernel_recovery_requires_successful_read_completion_and_valid_content() {
    for mode in 0..4 {
        let content = if mode == 3 {
            "{}".to_owned()
        } else {
            history::chunk(&[(7, 10)])
        };
        let mut values = k::FixedValueStore::<4, 4096>::new(16384).unwrap();
        let request = values.store(b"selected-reference-fixture").unwrap();
        let nodes = core::array::from_fn(|i| NodeSpec {
            input_cords: [i.checked_sub(1).map(|j| k::CordId(j as u16))],
            maximum_step_fuel: 16,
        });
        let cords = core::array::from_fn(|i| {
            CordSpec::local(
                k::CordId(i as u16),
                (k::NodeId(i as u16), k::PortId(0)),
                (k::NodeId(i as u16 + 1), k::PortId(0)),
                CordCapacity {
                    slot_start: i as u16,
                    item_capacity: 1,
                    byte_capacity: 4096,
                    pressure_policy: Default::default(),
                },
            )
        });
        let mut routes = k::FixedRoutes::<3, 2>::new(1);
        for i in 0..2 {
            routes
                .install(
                    k::NodeId(i),
                    k::PortId(0),
                    k::RouteRange { start: i, len: 1 },
                    &[k::RouteTarget {
                        cord: k::CordId(i),
                        sink: k::CordEndpoint::local(k::NodeId(i + 1), k::PortId(0)),
                    }],
                )
                .unwrap();
        }
        routes.seal().unwrap();
        let mut calls = k::FixedHostCallBindings::<3>::new(1);
        calls
            .install(
                k::NodeId(1),
                k::HostCallBinding {
                    call: k::HostCallId(0),
                    maximum_input_bytes: 512,
                    maximum_output_bytes: 4096,
                },
            )
            .unwrap();
        calls.seal().unwrap();
        let mut kernel = FixedScheduler::<_, _, _, 3, 2, 1, 2, 3, 2, 3, 3>::new_with_host_calls(
            nodes,
            cords,
            routes,
            calls,
            [
                Driver::Source {
                    value: request,
                    sent: false,
                },
                Driver::Read(HostCallBack::new(512)),
                Driver::Recover(Box::new(back())),
            ],
            values,
            k::FixedSignLog::<128>::new(128 * core::mem::size_of::<k::KernelEvent>() as u32)
                .unwrap(),
        )
        .unwrap();
        let mut terminal = false;
        let mut requests = 0;
        let ((), allocations) = allocation_probe::observe(|| {
            for _ in 0..40 {
                if let Some(request) = kernel.next_host_request() {
                    requests += 1;
                    let Driver::Recover(reader) = &kernel.drivers()[2] else {
                        unreachable!()
                    };
                    assert_eq!(reader.recovered(), RecoveredHistory::default());
                    let disposition = match mode {
                        1 => k::HostCallDisposition::Failed,
                        2 => k::HostCallDisposition::Cancelled,
                        _ => k::HostCallDisposition::Completed,
                    };
                    let output = (disposition == k::HostCallDisposition::Completed).then(|| {
                        k::BoundedValueRef::new(
                            kernel.store_host_value(content.as_bytes()).unwrap(),
                            4096,
                        )
                        .unwrap()
                    });
                    kernel
                        .complete_host_call(
                            request.node,
                            request.request,
                            k::HostCallOutcome {
                                disposition,
                                output,
                                failure: (mode == 1).then_some(k::Failure {
                                    code: k::FailureCode::HostCallFailed,
                                    detail: 1,
                                }),
                            },
                        )
                        .unwrap();
                }
                match kernel.step() {
                    Ok(SchedulerStatus::Drained) => {
                        assert_eq!(mode, 0);
                        terminal = true;
                        break;
                    }
                    Err(SchedulerError::BackFailed(_)) => {
                        assert_ne!(mode, 0);
                        terminal = true;
                        break;
                    }
                    Err(e) => panic!("unexpected scheduler refusal: {e:?}"),
                    _ => {}
                }
            }
        });
        assert!(terminal);
        assert_eq!(requests, 1);
        assert_eq!((allocations.allocations, allocations.reallocations), (0, 0));
        let Driver::Recover(reader) = &kernel.drivers()[2] else {
            unreachable!()
        };
        assert_eq!(reader.recovered().samples, u32::from(mode == 0));
    }
}

#[test]
fn committed_recovery_projects_into_the_ordinary_recording_face() {
    use conduit_environment_face::{
        PreparedRecordingFace, RecordingRecoveryFailure, RecoveredRecordingRange,
    };
    use conduit_face::{FaceBasis, FacePropertyValue};
    let mut reader = back();
    let mut face = PreparedRecordingFace::prepare(
        FaceBasis {
            body_id: None,
            wake_id: None,
            source_document_id: Some("fixture/recording/source".into()),
            checked_plot_id: Some("fixture/recording/checked".into()),
            expanded_plot_id: Some("fixture/recording/expanded".into()),
            plan_id: Some("fixture/recording/plan".into()),
            active_play_id: Some("fixture/recording/play".into()),
            sign_ids: vec![],
        },
        2,
        128,
        4,
    )
    .unwrap();
    let content = history::chunk(&[(1, 10), (3, 20)]);
    let ((), memory) = allocation_probe::observe(|| {
        assert_eq!(
            offer(&mut reader, content.as_bytes(), 16),
            StepOutcome::Progress
        );
        assert_eq!(reader.recovered(), RecoveredHistory::default());
        assert_eq!(face.current().revision, 0);
        <RecordingRecoveryBack as StepBack<1>>::step_committed(&mut reader);
        let recovered = reader.recovered();
        face.publish_recovered(RecoveredRecordingRange {
            chunks: recovered.chunks,
            samples: recovered.samples,
            first_sequence: recovered.first_sequence.unwrap(),
            last_sequence: recovered.last_sequence.unwrap(),
        })
        .unwrap();
        assert!(face
            .current()
            .properties
            .iter()
            .any(|p| p.name == "last-sequence" && p.value == FacePropertyValue::Count(3)));
        assert!(matches!(
            offer(&mut reader, b"{}", 16),
            StepOutcome::Fail(_)
        ));
        <RecordingRecoveryBack as StepBack<1>>::step_committed(&mut reader);
        face.publish_refusal(RecordingRecoveryFailure::InvalidContent)
            .unwrap();
        assert_eq!(reader.recovered(), recovered);
        assert!(face
            .current()
            .properties
            .iter()
            .any(|p| p.name == "samples" && p.value == FacePropertyValue::Count(2)));
        face.current().validate().unwrap();
    });
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
}
