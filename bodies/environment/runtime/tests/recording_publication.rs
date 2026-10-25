//! Production kernel and recorder Back; numeric placement and storage completion
//! are CPU fixtures, not browser storage or Root authority evidence.
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
use conduit_core as c;
use conduit_environment_runtime::{observation::ObservationContract, recording::*};
use conduit_kernel::{self as k, scheduler::*, ValueStorage};
use std::{cell::Cell, rc::Rc};
fn context() -> RecordingContext<'static> {
    use std::sync::LazyLock;
    static HOST: LazyLock<c::HostId> = LazyLock::new(|| "fixture/producer".into());
    static BOOT: LazyLock<c::BootId> = LazyLock::new(|| "fixture/boot".into());
    static PLAN: LazyLock<c::PlanId> = LazyLock::new(|| "fixture/plan".into());
    static PLAY: LazyLock<c::ActivePlayId> = LazyLock::new(|| "fixture/play".into());
    RecordingContext {
        host: &HOST,
        boot: &BOOT,
        plan: &PLAN,
        play: &PLAY,
    }
}
fn prepared() -> (
    RecordingSnapshotBack,
    RecordingCommitObserver,
    Vec<u8>,
    Vec<u8>,
) {
    prepared_generation(2, 12)
}
fn prepared_generation(
    version: u8,
    first: u64,
) -> (
    RecordingSnapshotBack,
    RecordingCommitObserver,
    Vec<u8>,
    Vec<u8>,
) {
    let contract = ObservationContract::prepare().unwrap();
    let reference = c::BoundedResourceRef {
        identity: c::ResourceSemanticIdentity::from_digest([1; 32]),
        content_profile: c::kind_id("text/json-utf8@1"),
        access_class: "resource/snapshot@1".into(),
        extent: c::ResourceExtent {
            bytes: PreparedRecordingSnapshot::content_bytes(context(), &contract, 2).unwrap()
                as u64,
            items: Some(1),
        },
        lifetime: c::ResourceLifetime {
            version: c::ResourceVersionIdentity::from_digest([version; 32]),
            expires_at: None,
        },
    };
    let (back, observer) =
        RecordingSnapshotBack::prepare(context(), &contract, 2, &reference).unwrap();
    let mut chunk =
        PreparedRecordingSnapshot::prepare(context(), &contract, 2, &reference).unwrap();
    chunk
        .append(first, 100, RecordingDisposition::CaptureTimeout, None)
        .unwrap();
    chunk
        .append(first + 2, 200, RecordingDisposition::Checksum, None)
        .unwrap();
    (
        back,
        observer,
        chunk.seal().unwrap().as_bytes().to_vec(),
        reference.encode().unwrap(),
    )
}
#[test]
fn completion_pressure_and_uncommitted_or_cancelled_steps_cannot_advance_the_cursor() {
    for cancel in [false, true] {
        let (mut back, observer, content, reference) = prepared();
        let value = k::ValueRef {
            slot: 0,
            generation: 1,
            byte_len: content.len() as u32,
        };
        let mut io = StepIo::test_frame([Some(value)], [false], [Some(512)], None, 16);
        assert_eq!(
            back.step(&mut io, &StepInputBytes::test_frame([Some(&content)], None)),
            StepOutcome::Progress
        );
        assert_eq!(observer.committed(), None);
        assert!(!observer.committed_content_matches(&content));
        <RecordingSnapshotBack as StepBack<1>>::step_committed(&mut back);
        let completion = Some((
            k::RequestId(0),
            k::HostCallOutcome {
                disposition: k::HostCallDisposition::Completed,
                output: Some(
                    k::BoundedValueRef::new(
                        k::ValueRef {
                            slot: 1,
                            generation: 1,
                            byte_len: reference.len() as u32,
                        },
                        512,
                    )
                    .unwrap(),
                ),
                failure: None,
            },
        ));
        let mut blocked = StepIo::test_frame([None], [false], [None], completion, 16);
        assert_eq!(
            back.step(
                &mut blocked,
                &StepInputBytes::test_frame([None], Some(&reference))
            ),
            StepOutcome::Await
        );
        <RecordingSnapshotBack as StepBack<1>>::step_committed(&mut back);
        assert_eq!(observer.committed(), None);
        assert!(!observer.committed_content_matches(&content));
        let mut ready = StepIo::test_frame([None], [false], [Some(512)], completion, 16);
        assert_eq!(
            back.step(
                &mut ready,
                &StepInputBytes::test_frame([None], Some(&reference))
            ),
            StepOutcome::Progress
        );
        assert_eq!(observer.committed(), None);
        assert!(!observer.committed_content_matches(&content));
        if cancel {
            <RecordingSnapshotBack as StepBack<1>>::cancel(&mut back);
        }
        <RecordingSnapshotBack as StepBack<1>>::step_committed(&mut back);
        assert_eq!(
            observer.committed(),
            (!cancel).then_some(RecordedRange {
                first_sequence: 12,
                last_sequence: 14,
                samples: 2
            })
        );
        let committed = observer.committed();
        <RecordingSnapshotBack as StepBack<1>>::cancel(&mut back);
        assert_eq!(observer.committed(), committed);
        assert_eq!(observer.committed_content_matches(&content), !cancel);
    }
}
enum Back {
    Source { value: k::ValueRef, sent: bool },
    Publish(Box<RecordingSnapshotBack>),
    Sink(Rc<Cell<u32>>),
}
impl StepBack<1> for Back {
    fn step(&mut self, io: &mut StepIo<1>, bytes: &StepInputBytes<'_, 1>) -> StepOutcome {
        match self {
            Self::Publish(back) => back.step(io, bytes),
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
            Self::Sink(count) => {
                if io.input(k::PortId(0)).is_some() {
                    io.consume(k::PortId(0)).unwrap();
                    count.set(count.get() + 1);
                    StepOutcome::Progress
                } else if io.input_closed(k::PortId(0)) {
                    io.consume_closed(k::PortId(0)).unwrap();
                    StepOutcome::Complete
                } else {
                    StepOutcome::Await
                }
            }
        }
    }
    fn step_committed(&mut self) {
        if let Self::Publish(back) = self {
            <RecordingSnapshotBack as StepBack<1>>::step_committed(back);
        }
    }
    fn cancel(&mut self) {
        if let Self::Publish(back) = self {
            <RecordingSnapshotBack as StepBack<1>>::cancel(back);
        }
    }
}
#[derive(Clone, Copy, Debug)]
enum ResultFixture {
    Published,
    Failed,
    Denied,
    Cancelled,
    WrongReference,
    MissingOutput,
    CorruptContent,
}
#[test]
fn actual_kernel_completion_is_required_before_the_recorded_range_becomes_visible() {
    for result in [
        ResultFixture::Published,
        ResultFixture::Failed,
        ResultFixture::Denied,
        ResultFixture::Cancelled,
        ResultFixture::WrongReference,
        ResultFixture::MissingOutput,
        ResultFixture::CorruptContent,
    ] {
        execute(result);
    }
}
fn execute(result: ResultFixture) -> RecordingCommitObserver {
    execute_generation(result, 2, 12)
}
fn execute_generation(result: ResultFixture, version: u8, first: u64) -> RecordingCommitObserver {
    let (back, observer, mut content, mut reference) = prepared_generation(version, first);
    if matches!(result, ResultFixture::CorruptContent) {
        content[0] = b'!';
    }
    let mut values = k::FixedValueStore::<4, 4096>::new(4 * 4096).unwrap();
    let input = values.store(&content).unwrap();
    let nodes = [
        NodeSpec {
            input_cords: [None],
            maximum_step_fuel: 16,
        },
        NodeSpec {
            input_cords: [Some(k::CordId(0))],
            maximum_step_fuel: 16,
        },
        NodeSpec {
            input_cords: [Some(k::CordId(1))],
            maximum_step_fuel: 16,
        },
    ];
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
                maximum_input_bytes: 4096,
                maximum_output_bytes: 512,
            },
        )
        .unwrap();
    calls.seal().unwrap();
    let seen = Rc::new(Cell::new(0));
    let mut kernel = FixedScheduler::<_, _, _, 3, 2, 1, 2, 3, 2, 3, 3>::new_with_host_calls(
        nodes,
        cords,
        routes,
        calls,
        [
            Back::Source {
                value: input,
                sent: false,
            },
            Back::Publish(Box::new(back)),
            Back::Sink(seen.clone()),
        ],
        values,
        k::FixedSignLog::<128>::new((128 * core::mem::size_of::<k::KernelEvent>()) as u32).unwrap(),
    )
    .unwrap();
    let mut requests = 0;
    let mut terminal = false;
    let ((), allocation) = allocation_probe::observe(|| {
        for _ in 0..40 {
            if let Some(request) = kernel.next_host_request() {
                requests += 1;
                assert_eq!(observer.committed(), None);
                assert!(!observer.committed_content_matches(&content));
                assert_eq!(kernel.host_value(request.input.value).unwrap(), content);
                if matches!(result, ResultFixture::WrongReference) {
                    reference[0] ^= 1;
                }
                let disposition = match result {
                    ResultFixture::Failed => k::HostCallDisposition::Failed,
                    ResultFixture::Denied => k::HostCallDisposition::Denied,
                    ResultFixture::Cancelled => k::HostCallDisposition::Cancelled,
                    _ => k::HostCallDisposition::Completed,
                };
                let output = (disposition == k::HostCallDisposition::Completed
                    && !matches!(result, ResultFixture::MissingOutput))
                .then(|| {
                    k::BoundedValueRef::new(kernel.store_host_value(&reference).unwrap(), 512)
                        .unwrap()
                });
                let failure = matches!(result, ResultFixture::Failed | ResultFixture::Denied)
                    .then_some(k::Failure {
                        code: if matches!(result, ResultFixture::Denied) {
                            k::FailureCode::HostCallDenied
                        } else {
                            k::FailureCode::HostCallFailed
                        },
                        detail: 99,
                    });
                kernel
                    .complete_host_call(
                        request.node,
                        request.request,
                        k::HostCallOutcome {
                            disposition,
                            output,
                            failure,
                        },
                    )
                    .unwrap();
                assert_eq!(observer.committed(), None);
                assert!(!observer.committed_content_matches(&content));
            }
            match kernel.step() {
                Ok(SchedulerStatus::Drained) => {
                    assert!(matches!(result, ResultFixture::Published));
                    terminal = true;
                    break;
                }
                Err(SchedulerError::BackFailed(failure)) => {
                    let expected = match result {
                        ResultFixture::Failed => k::FailureCode::HostCallFailed,
                        ResultFixture::Denied => k::FailureCode::HostCallDenied,
                        ResultFixture::Cancelled => k::FailureCode::Cancelled,
                        ResultFixture::WrongReference
                        | ResultFixture::MissingOutput
                        | ResultFixture::CorruptContent => k::FailureCode::InvalidInput,
                        ResultFixture::Published => panic!("successful fixture failed"),
                    };
                    assert_eq!(failure.code, expected);
                    if matches!(result, ResultFixture::Failed | ResultFixture::Denied) {
                        assert_eq!(failure.detail, 99);
                    }
                    terminal = true;
                    break;
                }
                Err(error) => panic!("unexpected kernel refusal: {error:?}"),
                _ => {}
            }
        }
    });
    assert!(
        terminal,
        "fixture must reach a terminal kernel disposition: {result:?}"
    );
    assert_eq!((allocation.allocations, allocation.reallocations), (0, 0));
    assert_eq!(
        requests,
        usize::from(!matches!(result, ResultFixture::CorruptContent))
    );
    assert_eq!(
        observer.committed(),
        matches!(result, ResultFixture::Published).then_some(RecordedRange {
            first_sequence: first,
            last_sequence: first + 2,
            samples: 2
        })
    );
    assert_eq!(
        seen.get(),
        u32::from(matches!(result, ResultFixture::Published))
    );
    let (matches_content, digest_allocation) =
        allocation_probe::observe(|| observer.committed_content_matches(&content));
    assert_eq!(matches_content, matches!(result, ResultFixture::Published));
    assert_eq!(
        (
            digest_allocation.allocations,
            digest_allocation.reallocations
        ),
        (0, 0)
    );
    if matches!(result, ResultFixture::Published) {
        // Same exact context, cardinality, sequence range and JSON extent; only
        // one valid acquisition time differs. Counters cannot prove content.
        let mut changed = content.clone();
        let start = core::str::from_utf8(&changed)
            .unwrap()
            .find("\"records\":\"")
            .unwrap()
            + "\"records\":\"".len();
        changed[start + 17] = b'5'; // first timestamp: 100 -> 101 boot-ms
        let mut reader =
            PreparedRecordingReader::prepare(context(), &ObservationContract::prepare().unwrap())
                .unwrap();
        reader
            .read(core::str::from_utf8(&changed).unwrap())
            .unwrap();
        assert_eq!(reader.len(), 2);
        assert_eq!(reader.sample(0).unwrap().sequence, first);
        assert_eq!(reader.sample(1).unwrap().sequence, first + 2);
        assert!(!observer.committed_content_matches(&changed));
        assert!(!observer.committed_content_matches(&reference));
        assert!(!observer.committed_content_matches(&content[..content.len() - 1]));
        assert!(!observer.committed_content_matches(&[0; MAXIMUM_CHUNK_BYTES + 1]));
    }
    observer
}
