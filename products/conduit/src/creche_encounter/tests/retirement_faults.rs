//! Cleanup must quiesce actual threads even when normal ownership is faulted.
use super::*;

#[test]
fn poisoned_mailbox_cancels_and_joins_worker_and_drains_mask_without_reopening() {
    let (mut factory, _directory) = factory();
    let drained = Arc::new(AtomicBool::new(false));
    let (mut adapter, controller) = factory
        .encounter(
            binding(),
            Box::new(Custody {
                admitted: true,
                drained: drained.clone(),
            }),
        )
        .unwrap();
    let cancelled = adapter.channel.lock().unwrap().cancelled.clone();
    let (finished, observed) = std::sync::mpsc::sync_channel(1);
    let worker = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(2);
        while !cancelled.load(Ordering::Acquire) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(1));
        }
        finished.send(cancelled.load(Ordering::Acquire)).unwrap();
    });
    adapter.channel.lock().unwrap().worker = Some(worker);
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _guard = adapter.channel.state.lock().unwrap();
        panic!("fixture poisons mailbox while worker exists");
    }))
    .is_err());
    let error = adapter.cancel_and_drain().unwrap_err();
    assert!(error.contains("mailbox poisoned"));
    assert!(observed.try_recv().unwrap()); // cancellation joined the actual worker
    assert!(drained.load(Ordering::Acquire));
    assert!(!adapter.local.retired);
    assert!(controller.face().is_err());
    assert!(controller.start_production_if_reviewed().is_err());
    match adapter.channel.state.lock() {
        Err(poisoned) => {
            let state = poisoned.into_inner();
            assert!(state.worker.is_none());
            assert!(!state.active);
        }
        Ok(_) => panic!("cleanup must not clear ownership poison"),
    };
}

#[test]
fn panicked_worker_does_not_skip_independent_mask_drain() {
    let (mut factory, _directory) = factory();
    let drained = Arc::new(AtomicBool::new(false));
    let (mut adapter, controller) = factory
        .encounter(
            binding(),
            Box::new(Custody {
                admitted: true,
                drained: drained.clone(),
            }),
        )
        .unwrap();
    adapter.channel.lock().unwrap().worker = Some(std::thread::spawn(|| {
        panic!("fixture worker panics before its result");
    }));
    let error = adapter.cancel_and_drain().unwrap_err();
    assert!(error.contains("worker panicked"));
    assert!(drained.load(Ordering::Acquire));
    assert!(!adapter.local.retired);
    assert!(adapter.channel.lock().unwrap().worker.is_none());
    assert!(!controller.start_production_if_reviewed().unwrap());
    // The failed join already consumed the terminated thread. A successful
    // explicit retry can finish retirement without restarting production.
    adapter.cancel_and_drain().unwrap();
    assert!(adapter.local.retired);
}

#[test]
fn poisoned_mask_still_drains_and_preserves_its_fault() {
    let (mut factory, _directory) = factory();
    let drained = Arc::new(AtomicBool::new(false));
    let (mut adapter, controller) = factory
        .encounter(
            binding(),
            Box::new(Custody {
                admitted: true,
                drained: drained.clone(),
            }),
        )
        .unwrap();
    adapter.channel.lock().unwrap().worker = Some(std::thread::spawn(|| {
        panic!("fixture worker also fails during Mask retirement");
    }));
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _guard = adapter.channel.custody.lock().unwrap();
        panic!("fixture poisons selected Mask custody");
    }))
    .is_err());
    let error = adapter.cancel_and_drain().unwrap_err();
    assert!(error.contains("Mask custody poisoned"));
    assert!(error.contains("worker panicked"));
    assert!(drained.load(Ordering::Acquire));
    assert!(!adapter.local.retired);
    assert!(!controller.start_production_if_reviewed().unwrap());
    assert!(adapter.channel.custody.is_poisoned());
}
