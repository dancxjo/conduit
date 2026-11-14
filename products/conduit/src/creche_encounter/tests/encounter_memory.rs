//! Bounded actual encounter state and view ownership; no IMAGE Play is claimed.
use super::*;
use conduit_std_host::StdHost;

pub(super) fn probe(host: &mut StdHost, mib: usize) -> bool {
    // SAFETY: fixed unit result, no effect or worker, and no escaped backing.
    unsafe { host.prepare_selected_image_value(mib * 1024 * 1024, mib * 1024 * 1024, || Ok(())) }
        .is_ok()
}

#[test]
fn image_encounter_local_and_shared_charges_retire_at_their_actual_last_owners() {
    let (mut host, prepared, _directory) = image_custody::initialized_factory();
    let mut factory = prepared.value().borrow_mut();
    // SAFETY: fixed bounded fixture, one Face and no worker; each region fits
    // its envelope. Borrowed inventory and its separate charge remain retained.
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
    let LocalCustody::Image(local) = &adapter.local else {
        panic!("IMAGE local owner expected")
    };
    host.verify_selected_image_prepared_value(local).unwrap();
    let ChannelCustody::Image(shared) = &controller.channel else {
        panic!("IMAGE shared owner expected")
    };
    host.verify_selected_image_prepared_value(shared).unwrap();
    let retained = Arc::downgrade(shared);
    // SAFETY: fixed fixture reserves the complete envelope before activation.
    unsafe { controller.prepare_image_worker(&mut host, worker_memory::BOUNDS) }.unwrap();
    start_fixture(&mut adapter);
    let view = controller.face().unwrap().unwrap();
    assert!(!probe(&mut host, 40));
    adapter.cancel_and_drain().unwrap();
    drop(adapter);
    assert!(probe(&mut host, 40)); // local storage actually retired
    drop(controller);
    drop(factory);
    assert!(retained.upgrade().is_none()); // private channel can already drop
    assert!(!probe(&mut host, 60)); // exported Face still owns shared state
    assert!(!view.text.is_empty());
    drop(view);
    assert!(retained.upgrade().is_none());
    assert!(probe(&mut host, 60));
}

#[test]
fn image_encounter_shared_refusal_releases_local_preparation_and_foreign_owner_refuses() {
    let (mut host, prepared, _directory) = image_custody::initialized_factory();
    let mut factory = prepared.value().borrow_mut();
    assert!(factory
        .encounter(
            binding(),
            Box::new(Custody {
                admitted: true,
                drained: Arc::new(AtomicBool::new(false))
            }),
        )
        .err()
        .unwrap()
        .contains("requires its funded encounter entry"));
    // SAFETY: fixed local preparation is bounded; shared reservation refuses
    // before constructing its channel and starts no worker or effect.
    assert!(unsafe {
        factory.encounter_in_image(
            &mut host,
            4 * 1024 * 1024,
            64 * 1024 * 1024,
            binding(),
            Box::new(Custody {
                admitted: true,
                drained: Arc::new(AtomicBool::new(false)),
            }),
        )
    }
    .err()
    .unwrap()
    .contains("backing capacity"));
    assert!(probe(&mut host, 61));
    let (mut other, _other_prepared, _other_directory) = image_custody::initialized_factory();
    // SAFETY: only owner verification is attempted; copied identities cannot
    // migrate factory input custody to this otherwise equivalent Host.
    assert!(unsafe {
        factory.encounter_in_image(
            &mut other,
            4 * 1024 * 1024,
            4 * 1024 * 1024,
            binding(),
            Box::new(Custody {
                admitted: true,
                drained: Arc::new(AtomicBool::new(false)),
            }),
        )
    }
    .err()
    .unwrap()
    .contains("another memory owner"));
}

#[test]
fn image_encounter_refuses_activation_without_mandatory_worker_memory() {
    let (mut host, prepared, _directory) = image_custody::initialized_factory();
    let mut factory = prepared.value().borrow_mut();
    // SAFETY: fixed initial state fits these envelopes. No worker is admitted;
    // activation must refuse before publishing a current Face or starting work.
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
    prepare_fixture(&mut adapter);
    assert!(adapter
        .begin(CrecheRequest([9; 32]))
        .unwrap_err()
        .contains("activation requires admitted worker memory"));
    assert!(controller.face().unwrap().is_none());
    let state = controller.channel.lock().unwrap();
    assert!(!state.active);
    assert!(!state.started);
    assert!(state.worker.is_none());
    drop(state);
    assert!(!adapter.local.begun);
    // SAFETY: the same fixed bounded worker is now reserved before activation.
    unsafe { controller.prepare_image_worker(&mut host, worker_memory::BOUNDS) }.unwrap();
    adapter.begin(CrecheRequest([9; 32])).unwrap();
    assert!(controller.face().unwrap().is_some());
    assert!(controller.channel.lock().unwrap().worker.is_none());
    adapter.cancel_and_drain().unwrap();
}

#[test]
fn uncertain_image_mask_drop_keeps_shared_state_and_capacity_quarantined() {
    struct Uncertain;
    impl CrecheMaskShowCustody for Uncertain {
        fn validate_acknowledged(&mut self, _: &Face, _: &MaskShow) -> Result<(), String> {
            Ok(())
        }
        fn validate_returned_interaction(
            &mut self,
            _: &Face,
            _: &MaskShow,
            _: &FaceInteraction,
        ) -> Result<(), String> {
            Ok(())
        }
        fn retire_and_drain(&mut self) -> Result<(), String> {
            Err("presentation drain uncertain".into())
        }
    }
    let (mut host, prepared, _directory) = image_custody::initialized_factory();
    let mut factory = prepared.value().borrow_mut();
    // SAFETY: fixed fixture owns all local/shared data and starts no worker.
    // Its deliberate uncertainty must pin the shared charge, not reuse it.
    let (mut adapter, controller) = unsafe {
        factory.encounter_in_image(
            &mut host,
            4 * 1024 * 1024,
            4 * 1024 * 1024,
            binding(),
            Box::new(Uncertain),
        )
    }
    .unwrap();
    let ChannelCustody::Image(shared) = &controller.channel else {
        panic!("IMAGE shared owner expected")
    };
    let retained = Arc::downgrade(shared);
    assert!(adapter
        .cancel_and_drain()
        .unwrap_err()
        .contains("uncertain"));
    drop(adapter);
    drop(controller);
    drop(factory);
    assert!(retained.upgrade().is_some());
    assert!(probe(&mut host, 56)); // synchronous local state was freed
    assert!(!probe(&mut host, 60)); // uncertain shared state remains charged
                                    // Intentional last-resort quarantine is reclaimed at process exit. This
                                    // fixture is not a successful retirement or an allocator-containment proof.
}
