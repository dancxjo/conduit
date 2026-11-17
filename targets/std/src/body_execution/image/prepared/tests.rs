use super::*;
use std::sync::atomic::Ordering;
fn host() -> StdHost {
    use conduit_host_make::{
        build_default_host_image, check_host_configuration, parse_host_configuration_conduit,
        BuildInputs,
    };
    let source = r#"host return-frame-test {
      schema = 1
      target = {architecture: "x86_64", machine: "computer", os: "linux"}
      base = {kind: "timer/monotonic", implementation: "hosted/monotonic-clock@1"}
      capability = {kind: "time/tick", pinned_implementation: "std/kernel-time-tick@2"}
      need = {id: "timer/main", class: "conduit.resource/timer-slot@1", slots: 8, bytes: 512}
      limits = {static_memory_bytes: 16777216, heap_arena_bytes: 67108864, queue_items: 4096, buffered_bytes: 16777216, active_instances: 512, operation_slots: 256, timer_slots: 128, line_sessions: 64, evidence_items: 4096}
    }"#;
    let catalog = conduit_workspace_make::catalog();
    let packages = conduit_workspace_make::package_set();
    let checked = check_host_configuration(
        parse_host_configuration_conduit(source).unwrap(),
        &catalog,
        &packages,
    )
    .unwrap();
    let (image, _) = build_default_host_image(
        checked.profile().clone(),
        &catalog,
        &packages,
        &BuildInputs {
            source_identity: "fixture/return-frame".into(),
            toolchain_available: true,
        },
    )
    .unwrap();
    StdHost::from_selected_timer_image(
        crate::StdHostConfig {
            host_id: "std/durable".into(),
            boot_id: "boot/return-frame".into(),
            offer_generation: conduit_core::OfferGeneration(1),
        },
        &image,
        &catalog,
    )
    .unwrap()
}

#[test]
fn root_preparation_shares_loaned_counter_and_retires_without_early_refund() {
    let mut host = host();
    let other = self::host();
    let used = host.kernel_resources.image_memory_observer();
    let root = host.image_root_preparation_owner().unwrap();
    root.verify_host(&host).unwrap();
    assert!(root.verify_host(&other).is_err());
    let (ready, received) = std::sync::mpsc::sync_channel(1);
    let (release, released) = std::sync::mpsc::sync_channel(1);
    let worker = std::thread::spawn(move || {
        // SAFETY: the complete fixed Vec has no effects/shared descendants.
        let prepared = unsafe {
            host.prepare_selected_image_value(32_000_000, 32_000_000, || {
                Ok(Vec::<u8>::with_capacity(31_999_000))
            })
        }
        .unwrap();
        ready.send(()).unwrap();
        released.recv().unwrap();
        drop(prepared);
        host
    });
    received.recv().unwrap();
    // SAFETY: pressure must refuse before the sentinel factory executes.
    assert!(unsafe {
        root.prepare_value::<Vec<u8>>(40_000_000, 40_000_000, || {
            panic!("bypassed original loaned Host counter")
        })
    }
    .is_err());
    // SAFETY: fixed Vec, inline metadata and bounded destruction fit64k.
    let prepared =
        unsafe { root.prepare_value(64_000, 64_000, || Ok(Vec::<u8>::with_capacity(63_000))) }
            .unwrap();
    root.verify_prepared(&prepared).unwrap();
    let retained = prepared.retain_memory();
    root.verify_retention(&retained).unwrap();
    assert!(other
        .verify_selected_image_memory_retention(&retained)
        .is_err());
    root.invalidate();
    let charged = used.load(Ordering::Acquire);
    // SAFETY: retired lifetime refuses before this factory.
    assert!(
        unsafe { root.prepare_value::<Vec<u8>>(128, 128, || panic!("retired Root prepared")) }
            .is_err()
    );
    assert_eq!(used.load(Ordering::Acquire), charged);
    drop(root);
    release.send(()).unwrap();
    let host = worker.join().unwrap();
    drop(host);
    drop(prepared);
    assert!(used.load(Ordering::Acquire) >= 64_000);
    drop(retained);
    // The observation Arc still owns the counter allocation itself. All
    // prepared value/Root charges retire; this real backing is not yet freed.
    assert_eq!(
        used.load(Ordering::Acquire),
        crate::kernel_preparation::StagedImageMemory::counter_backing_bytes()
    );
}

#[test]
fn root_metadata_byte_short_refuses_before_arc_allocation() {
    let host = host();
    let used = host.kernel_resources.image_memory_observer();
    let bytes = core::mem::size_of::<RootPreparationState>()
        + 2 * core::mem::size_of::<usize>()
        + ImageMemoryLease::owned_backing_bytes();
    let padding = host
        .kernel_resources
        .fund_tick_preparation(64 * 1024 * 1024 - used.load(Ordering::Acquire) - bytes + 1)
        .unwrap();
    let prior = used.load(Ordering::Acquire);
    assert!(host.image_root_preparation_owner().is_err());
    assert_eq!(used.load(Ordering::Acquire), prior);
    drop(padding);
}

#[test]
fn failed_and_unwinding_factories_release_original_charge() {
    let host = host();
    let root = host.image_root_preparation_owner().unwrap();
    let used = host.kernel_resources.image_memory_observer();
    let prior = used.load(Ordering::Acquire);
    // SAFETY: both factories retain no output and have no effects/workers.
    assert!(
        unsafe { root.prepare_value::<Vec<u8>>(4096, 4096, || Err("refused".into())) }.is_err()
    );
    assert_eq!(used.load(Ordering::Acquire), prior);
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
        root.prepare_value::<Vec<u8>>(4096, 4096, || panic!("bounded factory unwind"))
    }));
    assert!(outcome.is_err());
    assert_eq!(used.load(Ordering::Acquire), prior);
}
