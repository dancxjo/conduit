//! CPU-owned local caller and refusal consumer, with actual channel wakeups.
//! This establishes adapter mechanics, not Root caller/Mask admission.
use super::*;
use crate::{
    allocation_probe,
    display_execution::{
        fore::{Frame, LocalFore},
        Execution, Exit, Outcome,
    },
};
use conduit_kernel as k;
use embassy_sync::{blocking_mutex::raw::NoopRawMutex, channel::Channel, signal::Signal};
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    task::{Wake, Waker},
};
type Input = Result<Option<Frame>, k::Failure>;
struct Caller<'a> {
    input: &'a Channel<NoopRawMutex, Input, 1>,
    acknowledge: &'a Signal<NoopRawMutex, Result<(), k::Failure>>,
    next_calls: &'a Cell<u64>,
    accepted: &'a Cell<u64>,
    refusals: &'a Cell<u64>,
    delivered: &'a Cell<u64>,
}
impl LocalFore for Caller<'_> {
    async fn next(&mut self) -> Input {
        self.next_calls.set(self.next_calls.get() + 1);
        self.input.receive().await
    }
    fn accepted(&mut self, sequence: u64) {
        assert_eq!(sequence, self.accepted.get());
        self.accepted.set(sequence + 1);
    }
    async fn refused(&mut self, sequence: u64) -> Result<(), k::Failure> {
        assert_eq!(sequence, self.refusals.get());
        self.refusals.set(sequence + 1);
        self.acknowledge.wait().await?;
        self.delivered.set(self.delivered.get() + 1);
        Ok(())
    }
}
struct Wakes(AtomicUsize);
impl Wake for Wakes {
    fn wake(self: Arc<Self>) {
        self.0.fetch_add(1, Ordering::Relaxed);
    }
    fn wake_by_ref(self: &Arc<Self>) {
        self.0.fetch_add(1, Ordering::Relaxed);
    }
}
fn until<T>(
    future: &mut core::pin::Pin<&mut impl Future<Output = T>>,
    cx: &mut core::task::Context<'_>,
    predicate: impl Fn() -> bool,
) {
    for _ in 0..1000 {
        assert!(future.as_mut().poll(cx).is_pending());
        if predicate() {
            return;
        }
    }
    panic!("local Fore did not reach bounded wait");
}
pub(super) fn check(
    execution: &mut Execution<Pins>,
    mode: u8,
    edges: &Rc<Cell<usize>>,
) -> (Outcome, allocation_probe::Observation) {
    let mut encoder =
        conduit_environment_runtime::display_packets::PreparedDisplayFrame::prepare().unwrap();
    let valid = Frame::copy(
        encoder
            .encode([0x7d, 0xff, 0x66, 0x71, 0, 0], 3, true)
            .unwrap(),
    )
    .unwrap();
    let invalid = Frame::copy(encoder.encode([0; 6], 255, true).unwrap()).unwrap();
    let second_invalid = Frame::copy(invalid.canonical()).unwrap();
    assert!(Frame::copy(&[0]).is_err());
    let input = Channel::new();
    let acknowledge = Signal::new();
    let stop = Signal::<NoopRawMutex, ()>::new();
    let next_calls = Cell::new(0);
    let accepted = Cell::new(0);
    let refusals = Cell::new(0);
    let delivered = Cell::new(0);
    let mut caller = Caller {
        input: &input,
        acknowledge: &acknowledge,
        next_calls: &next_calls,
        accepted: &accepted,
        refusals: &refusals,
        delivered: &delivered,
    };
    let wakes = Arc::new(Wakes(AtomicUsize::new(0)));
    let waker = Waker::from(wakes.clone());
    let mut cx = core::task::Context::from_waker(&waker);
    let (outcome, memory) = allocation_probe::observe(|| {
        let mut future = core::pin::pin!(execution.run_with_fore(
            stop.wait(),
            &mut caller,
            if mode == 4 {
                0
            } else if mode == 8 {
                3
            } else {
                2
            }
        ));
        if matches!(mode, 4 | 8 | 11) {
            return complete(future);
        }
        until(&mut future, &mut cx, || next_calls.get() == 1);
        assert_eq!(edges.get(), 0);
        let before = wakes.0.load(Ordering::Relaxed);
        match mode {
            0 => {
                input.try_send(Ok(Some(valid))).ok().unwrap();
                assert!(wakes.0.load(Ordering::Relaxed) > before);
                until(&mut future, &mut cx, || next_calls.get() == 2);
                assert_eq!(accepted.get(), 1);
                input.try_send(Ok(Some(second_invalid))).ok().unwrap();
                until(&mut future, &mut cx, || refusals.get() == 1);
                acknowledge.signal(Ok(()));
                until(&mut future, &mut cx, || next_calls.get() == 3);
                input.try_send(Ok(None)).ok().unwrap();
            }
            1 | 3 | 7 | 10 => {
                input.try_send(Ok(Some(invalid))).ok().unwrap();
                until(&mut future, &mut cx, || refusals.get() == 1);
                assert_eq!((accepted.get(), delivered.get(), edges.get()), (1, 0, 0));
                for _ in 0..8 {
                    assert!(future.as_mut().poll(&mut cx).is_pending());
                }
                assert_eq!((next_calls.get(), edges.get()), (1, 0));
                if mode == 10 {
                    input.try_send(Ok(Some(valid))).ok().unwrap();
                    stop.signal(());
                } else if mode == 3 {
                    stop.signal(());
                } else if mode == 7 {
                    acknowledge.signal(Err(k::Failure {
                        code: k::FailureCode::HostCallDenied,
                        detail: 100,
                    }));
                } else {
                    input.try_send(Ok(Some(valid))).ok().unwrap();
                    acknowledge.signal(Ok(()));
                    until(&mut future, &mut cx, || next_calls.get() == 3);
                    input.try_send(Ok(None)).ok().unwrap();
                }
            }
            2 => stop.signal(()),
            5 => {
                for (index, frame) in [invalid, second_invalid].into_iter().enumerate() {
                    input.try_send(Ok(Some(frame))).ok().unwrap();
                    until(&mut future, &mut cx, || refusals.get() == index as u64 + 1);
                    acknowledge.signal(Ok(()));
                    until(&mut future, &mut cx, || {
                        next_calls.get() == index as u64 + 2
                    });
                }
                input.try_send(Ok(Some(valid))).ok().unwrap();
            }
            6 => input
                .try_send(Err(k::Failure {
                    code: k::FailureCode::InvalidInput,
                    detail: 99,
                }))
                .ok()
                .unwrap(),
            9 => input.try_send(Ok(None)).ok().unwrap(),
            _ => unreachable!(),
        }
        complete(future)
    });
    assert_eq!(outcome.cleanup, Ok(()));
    match mode {
        0 | 1 => {
            assert_eq!(outcome.exit, Exit::Drained);
            assert_eq!((accepted.get(), refusals.get(), delivered.get()), (2, 1, 1));
            assert_eq!(outcome.progress.native_calls, 1);
            assert!(edges.get() > 0);
        }
        2 | 3 | 10 => {
            assert_eq!(outcome.exit, Exit::Cancelled);
            assert_eq!(outcome.progress.native_calls, 0);
            assert_eq!((delivered.get(), edges.get()), (0, 0));
        }
        4 | 5 | 8 => {
            assert!(matches!(
                outcome.exit,
                Exit::Failed(k::Failure {
                    code: k::FailureCode::WorkBudgetExhausted,
                    ..
                })
            ));
            assert_eq!(accepted.get(), if mode == 5 { 2 } else { 0 });
            assert_eq!(edges.get(), 0);
        }
        6 => assert_eq!(
            outcome.exit,
            Exit::Failed(k::Failure {
                code: k::FailureCode::InvalidInput,
                detail: 99
            })
        ),
        7 => {
            assert_eq!(
                outcome.exit,
                Exit::Failed(k::Failure {
                    code: k::FailureCode::HostCallDenied,
                    detail: 100
                })
            );
            assert_eq!(
                (accepted.get(), refusals.get(), delivered.get(), edges.get()),
                (1, 1, 0, 0)
            );
        }
        9 => {
            assert_eq!(outcome.exit, Exit::Drained);
            assert_eq!((accepted.get(), refusals.get(), edges.get()), (0, 0, 0));
        }
        11 => {
            assert_eq!(
                outcome.exit,
                Exit::Failed(k::Failure {
                    code: k::FailureCode::InvalidLifecycle,
                    detail: 19
                })
            );
            assert_eq!((next_calls.get(), accepted.get(), edges.get()), (0, 0, 0));
        }
        _ => unreachable!(),
    }
    if mode == 10 {
        // Cancellation did not acknowledge or silently clear caller-owned input.
        let frame = input.try_receive().ok().unwrap().ok().unwrap().unwrap();
        assert!(
            conduit_environment_runtime::display_packets::validate_display_frame(frame.canonical())
                .is_ok()
        );
        assert_eq!(accepted.get(), 1);
    }
    (outcome, memory)
}
