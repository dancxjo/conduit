use super::*;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::{Duration, Instant};

struct Cancellable(Arc<AtomicBool>);
// SAFETY: fixed output, no allocation, retains no buffers, observes cancellation.
unsafe impl NativeSourceWorkerProvider for Cancellable {
    fn read(
        &mut self,
        _: &[u8],
        _: &mut [u8],
        context: &NativeReadContext<'_>,
    ) -> Result<usize, WorkerFailure> {
        self.0.store(true, Ordering::Release);
        while !context.should_stop() {
            std::thread::yield_now();
        }
        Err(WorkerFailure::Cancelled)
    }
}
fn budget() -> NativeWorkerBudget {
    NativeWorkerBudget {
        requested_stack_bytes: 2 * 1024 * 1024,
        maximum_stack_mapping_bytes: 2 * 1024 * 1024 + 64 * 1024,
        capture_timeout: Duration::from_secs(2),
        cancellation_timeout: Duration::from_secs(1),
    }
}
#[test]
fn invalid_profile_refuses_before_provider_factory() {
    let called = AtomicBool::new(false);
    let factory = WorkerFactory {
        budget: NativeWorkerBudget {
            requested_stack_bytes: 1,
            ..budget()
        },
        prepare: Box::new(|| {
            called.store(true, Ordering::Release);
            Err("must not construct".into())
        }),
    };
    assert!(factory.prepare(1, 1).is_err());
    assert!(!called.load(Ordering::Acquire));
}
#[cfg(target_os = "linux")]
#[test]
fn original_factory_uses_existing_worker_observation_and_cancellation() {
    let entered = Arc::new(AtomicBool::new(false));
    let provider_entered = entered.clone();
    let factory = WorkerFactory {
        budget: budget(),
        prepare: Box::new(move || Ok(Box::new(Cancellable(provider_entered)))),
    };
    let mut worker = factory.prepare(1, 1).unwrap();
    assert_eq!(
        worker.observation().charged_bytes,
        budget().backing_bytes(1, 1).unwrap()
    );
    let generation = worker.submit(&[0]).unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    while !entered.load(Ordering::Acquire) {
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
    worker.cancel();
    loop {
        match worker.poll(generation, &mut [0]) {
            Ok(Some(result)) => {
                assert_eq!(result, Err(WorkerFailure::Cancelled));
                break;
            }
            Ok(None) => {
                assert!(Instant::now() < deadline);
                std::thread::yield_now();
            }
            other => panic!("unexpected worker result: {other:?}"),
        }
    }
    loop {
        match worker.retire() {
            Ok(provider) => {
                assert!(provider.is_some());
                break;
            }
            Err(WorkerFailure::Busy) => {
                assert!(Instant::now() < deadline);
                std::thread::yield_now();
            }
            Err(other) => panic!("unexpected worker retirement: {other:?}"),
        }
    }
}
