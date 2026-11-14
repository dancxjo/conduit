//! Bounded actual native production and retained output on one IMAGE counter.
use super::encounter_memory::probe;
use super::*;

pub(super) const BOUNDS: CrecheWorkerBounds = CrecheWorkerBounds {
    memory_bytes: 16 * 1024 * 1024,
    stack_bytes: 2 * 1024 * 1024,
};

#[test]
fn admitted_image_worker_produces_seed_and_output_keeps_original_charge_after_join() {
    let (mut host, prepared, _directory) = image_custody::initialized_factory();
    let mut factory = prepared.value().borrow_mut();
    // SAFETY: fixed one-check fixture, one retained view, no external shared
    // backing. The complete selected production/output envelope is separate.
    let (mut adapter, controller) = unsafe {
        factory.encounter_in_image(
            &mut host,
            4 * 1024 * 1024,
            4 * 1024 * 1024,
            binding(),
            Box::new(Custody {
                admitted: true,
                drained: Arc::new(AtomicBool::new(false)),
            }),
        )
    }
    .unwrap();
    // SAFETY: this fixed tiny reviewed bundle and Body have a conservative
    // complete 16 MiB envelope including explicit 2 MiB stack and runtime cost.
    unsafe { controller.prepare_image_worker(&mut host, BOUNDS) }.unwrap();
    assert!(!probe(&mut host, 40)); // charged before thread creation or activation
    start_fixture(&mut adapter);
    let view = controller.face().unwrap().unwrap();
    let show = crate::mask_test_common::available_mask_show(&view);
    let input =
        FaceInteraction::new(&view, &show, "host-config/check", "host-config", vec![], 1).unwrap();
    controller.acknowledge_show(show).unwrap();
    controller.submit(input).unwrap();
    adapter.poll().unwrap();
    assert!(controller.start_production_if_reviewed().unwrap());
    assert!(!controller.start_production_if_reviewed().unwrap());
    let deadline = Instant::now() + Duration::from_secs(5);
    let outcome = loop {
        match adapter.poll().unwrap() {
            CrechePoll::Outcome(outcome) => break outcome,
            CrechePoll::Pending => {
                assert!(
                    Instant::now() < deadline,
                    "bounded IMAGE worker did not finish"
                );
                std::thread::yield_now();
            }
        }
    };
    assert_eq!(
        outcome.disposition,
        conduit_body_make::deployment_runtime::CrecheDisposition::Prepared,
        "{}",
        String::from_utf8_lossy(adapter.retained_evidence(&outcome).unwrap())
    );
    assert!(controller
        .take_products()
        .err()
        .unwrap()
        .contains("retained output custody"));
    let mut products = controller.take_retained_products().unwrap().unwrap();
    assert!(controller.take_retained_products().unwrap().is_none());
    assert_eq!(
        products.products().spore.manifest.body_id,
        adapter.channel.binding.biography.body_id.as_str()
    );
    let mut file = products.products().artifact.try_clone().unwrap();
    bundle_common::assert_seed_package(
        &mut file,
        &products.products().spore.manifest,
        &adapter.channel.binding.invitation,
        &products.products().receipt,
        b"artifact",
    );
    drop(file);
    use std::io::{Read, Seek};
    let mut reader = products.artifact_reader();
    reader.rewind().unwrap();
    let mut first = [0; 4];
    reader.read_exact(&mut first).unwrap();
    assert_eq!(&first, b"PK\x03\x04");
    drop(reader);
    adapter.cancel_and_drain().unwrap();
    assert!(controller.channel.lock().unwrap().worker.is_none());
    drop(adapter);
    drop(controller);
    drop(view);
    drop(factory);
    assert!(!probe(&mut host, 48)); // output still owns the 16 MiB worker charge
    assert!(!products.products().receipt.is_empty());
    drop(products);
    assert!(probe(&mut host, 60));
}

#[test]
fn image_worker_pressure_invalid_bounds_and_late_admission_do_not_launch() {
    let (mut host, prepared, _directory) = image_custody::initialized_factory();
    let mut factory = prepared.value().borrow_mut();
    // SAFETY: fixed local/shared stores; no worker is launched in this test.
    let (mut adapter, controller) = unsafe {
        factory.encounter_in_image(
            &mut host,
            4 * 1024 * 1024,
            4 * 1024 * 1024,
            binding(),
            Box::new(Custody {
                admitted: true,
                drained: Arc::new(AtomicBool::new(false)),
            }),
        )
    }
    .unwrap();
    // SAFETY: invalid/over-capacity bounds refuse before any platform operation.
    unsafe {
        assert!(controller
            .prepare_image_worker(
                &mut host,
                CrecheWorkerBounds {
                    memory_bytes: 64 * 1024 * 1024,
                    ..BOUNDS
                }
            )
            .unwrap_err()
            .contains("backing capacity"));
        assert!(controller
            .prepare_image_worker(
                &mut host,
                CrecheWorkerBounds {
                    stack_bytes: 0,
                    ..BOUNDS
                }
            )
            .unwrap_err()
            .contains("finite stack"));
    }
    assert!(controller.channel.lock().unwrap().worker_memory.is_none());
    assert!(probe(&mut host, 50));
    // SAFETY: fixed fixture envelope, reserved before activation.
    unsafe { controller.prepare_image_worker(&mut host, BOUNDS) }.unwrap();
    start_fixture(&mut adapter);
    // SAFETY: late admission is refused before reservation; no effect starts.
    assert!(
        unsafe { controller.prepare_image_worker(&mut host, BOUNDS) }
            .unwrap_err()
            .contains("inactive initial admission")
    );
    assert!(controller.channel.lock().unwrap().worker.is_none());
    adapter.cancel_and_drain().unwrap();
    // SAFETY: cancellation permanently closes this admission opportunity.
    assert!(unsafe { controller.prepare_image_worker(&mut host, BOUNDS) }.is_err());
}

#[test]
fn image_worker_cannot_relabel_counter_or_replace_existing_grant() {
    let (mut host, prepared, _directory) = image_custody::initialized_factory();
    let (mut other, _other_prepared, _other_directory) = image_custody::initialized_factory();
    let mut factory = prepared.value().borrow_mut();
    // SAFETY: same fixed bounded stores; setup starts no operation.
    let (mut adapter, controller) = unsafe {
        factory.encounter_in_image(
            &mut host,
            4 * 1024 * 1024,
            4 * 1024 * 1024,
            binding(),
            Box::new(Custody {
                admitted: true,
                drained: Arc::new(AtomicBool::new(false)),
            }),
        )
    }
    .unwrap();
    // SAFETY: owner mismatch refuses; correct owner reserves only fixed metadata.
    unsafe {
        assert!(controller
            .prepare_image_worker(&mut other, BOUNDS)
            .unwrap_err()
            .contains("another memory owner"));
        controller.prepare_image_worker(&mut host, BOUNDS).unwrap();
        assert!(controller
            .prepare_image_worker(&mut host, BOUNDS)
            .unwrap_err()
            .contains("inactive initial admission"));
    }
    assert!(!probe(&mut host, 40));
    adapter.cancel_and_drain().unwrap();
    drop(adapter);
    drop(controller);
    assert!(probe(&mut host, 60));
}

#[test]
fn image_worker_charge_stays_reserved_while_cancellation_waits_for_join() {
    let (mut host, prepared, _directory) = image_custody::initialized_factory();
    let mut factory = prepared.value().borrow_mut();
    // SAFETY: fixed stores; the injected bounded worker uses the same explicit
    // stack and no heap/output. Cancellation must join it before retiring.
    let (mut adapter, controller) = unsafe {
        factory.encounter_in_image(
            &mut host,
            4 * 1024 * 1024,
            4 * 1024 * 1024,
            binding(),
            Box::new(Custody {
                admitted: true,
                drained: Arc::new(AtomicBool::new(false)),
            }),
        )
    }
    .unwrap();
    unsafe { controller.prepare_image_worker(&mut host, BOUNDS) }.unwrap();
    let cancelled = controller.channel.lock().unwrap().cancelled.clone();
    let (seen, observed) = std::sync::mpsc::sync_channel(1);
    let (release, finish) = std::sync::mpsc::sync_channel(1);
    controller.channel.lock().unwrap().worker = Some(
        std::thread::Builder::new()
            .stack_size(BOUNDS.stack_bytes)
            .spawn(move || {
                let deadline = Instant::now() + Duration::from_secs(2);
                while !cancelled.load(Ordering::Acquire) && Instant::now() < deadline {
                    std::thread::sleep(Duration::from_millis(1));
                }
                seen.send(cancelled.load(Ordering::Acquire)).unwrap();
                finish.recv_timeout(Duration::from_secs(2)).unwrap();
            })
            .unwrap(),
    );
    std::thread::scope(|scope| {
        let joining = scope.spawn(move || {
            adapter.cancel_and_drain().unwrap();
            adapter
        });
        assert!(observed.recv_timeout(Duration::from_secs(2)).unwrap());
        assert!(!probe(&mut host, 40));
        release.send(()).unwrap();
        let adapter = joining.join().unwrap();
        drop(adapter);
    });
    drop(controller);
    assert!(probe(&mut host, 60));
}
