//! Adapted from runtime/tests/recording_publication.rs. Numeric routes, capacities
//! and request bindings are CPU fixtures; scheduling and commit are production.
use conduit_environment_history::{CommittedHistoryRefusal, PreparedCommittedHistoryFace};
use conduit_environment_runtime::recording::{RecordingCommitObserver, RecordingSnapshotBack};
use conduit_kernel::{self as k, scheduler::*, ValueStorage};
use conduit_linux_snapshot_store::FileSnapshotRefusal;
use std::{cell::Cell, rc::Rc};

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

/// The supplied effect invokes the prepared real file operation on the original
/// kernel-owned request bytes. No fixture calls `step_committed` directly.
pub(super) fn execute(
    back: RecordingSnapshotBack,
    observer: &RecordingCommitObserver,
    content: &str,
    reference: &[u8],
    history: &mut PreparedCommittedHistoryFace<'_>,
    mut publish: impl FnMut(&[u8]) -> Result<(), FileSnapshotRefusal>,
) -> Result<(), FileSnapshotRefusal> {
    let mut values = k::FixedValueStore::<4, 4096>::new(4 * 4096).unwrap();
    let input = values.store(content.as_bytes()).unwrap();
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
    let initial = history.contribution(super::common::basis()).unwrap();
    assert_eq!(
        history.read_next(content),
        Err(CommittedHistoryRefusal::Uncommitted)
    );
    let mut result = None;
    let mut expected_failure = None;
    let mut terminal = false;
    for _ in 0..40 {
        if let Some(request) = kernel.next_host_request() {
            assert!(result.is_none(), "one admitted publication, no retries");
            assert_eq!(observer.committed(), None);
            assert_eq!(
                history.read_next(content),
                Err(CommittedHistoryRefusal::Uncommitted)
            );
            assert_eq!(
                history.contribution(super::common::basis()).unwrap(),
                initial
            );
            let bytes = kernel.host_value(request.input.value).unwrap();
            assert_eq!(bytes, content.as_bytes());
            let effect = publish(bytes);
            let disposition = match &effect {
                Ok(()) => k::HostCallDisposition::Completed,
                Err(FileSnapshotRefusal::Binding(_)) => k::HostCallDisposition::Denied,
                Err(_) => k::HostCallDisposition::Failed,
            };
            // Even completed real file effects cannot advance historical Face
            // until this exact request's output commits in the kernel.
            assert_eq!(
                history.read_next(content),
                Err(CommittedHistoryRefusal::Uncommitted)
            );
            let output = effect.is_ok().then(|| {
                k::BoundedValueRef::new(kernel.store_host_value(reference).unwrap(), 512).unwrap()
            });
            expected_failure = match disposition {
                k::HostCallDisposition::Denied => Some(k::FailureCode::HostCallDenied),
                k::HostCallDisposition::Failed => Some(k::FailureCode::HostCallFailed),
                _ => None,
            };
            kernel
                .complete_host_call(
                    request.node,
                    request.request,
                    k::HostCallOutcome {
                        disposition,
                        output,
                        failure: expected_failure.map(|code| k::Failure { code, detail: 91 }),
                    },
                )
                .unwrap();
            result = Some(effect);
            assert_eq!(observer.committed(), None);
            assert_eq!(
                history.read_next(content),
                Err(CommittedHistoryRefusal::Uncommitted)
            );
            assert_eq!(
                history.contribution(super::common::basis()).unwrap(),
                initial
            );
        }
        match kernel.step() {
            Ok(SchedulerStatus::Drained) => {
                assert!(result.as_ref().is_some_and(Result::is_ok));
                terminal = true;
                break;
            }
            Err(SchedulerError::BackFailed(failure)) => {
                assert_eq!(Some(failure.code), expected_failure);
                assert_eq!(failure.detail, 91);
                terminal = true;
                break;
            }
            Err(error) => panic!("unexpected production kernel refusal: {error:?}"),
            _ => {}
        }
    }
    assert!(terminal, "bounded fixture must reach terminal disposition");
    let result = result.expect("original kernel request must reach real file effect");
    assert_eq!(seen.get(), u32::from(result.is_ok()));
    assert_eq!(observer.committed().is_some(), result.is_ok());
    if result.is_ok() {
        assert!(observer.committed_content_matches(content.as_bytes()));
        history.read_next(content).unwrap();
    } else {
        assert_eq!(
            history.read_next(content),
            Err(CommittedHistoryRefusal::Uncommitted)
        );
        assert_eq!(
            history.contribution(super::common::basis()).unwrap(),
            initial
        );
    }
    result
}
