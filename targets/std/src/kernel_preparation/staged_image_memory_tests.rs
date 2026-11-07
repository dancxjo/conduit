use super::*;
use conduit_host_make::{
    build_default_host_image, check_host_configuration, parse_host_configuration_conduit,
    BuildInputs, CheckedHostConfiguration, HostImage,
};

fn fixture() -> (
    crate::StdHostConfig,
    HostImage,
    CheckedHostConfiguration,
    HostAdvertisement,
) {
    let catalog = conduit_workspace_make::catalog();
    let packages = conduit_workspace_make::package_set();
    let checked = check_host_configuration(
        parse_host_configuration_conduit(include_str!(
            "../../profiles/raspberry-pi-3-tick-workset.host.conduit"
        ))
        .unwrap(),
        &catalog,
        &packages,
    )
    .unwrap();
    let image = build_default_host_image(
        checked.profile().clone(),
        &catalog,
        &packages,
        &BuildInputs {
            source_identity: "fixture/staged-memory".into(),
            toolchain_available: true,
        },
    )
    .unwrap()
    .0;
    let host = crate::StdHost::new();
    let actual = host.advertisement().clone();
    let config = crate::StdHostConfig {
        host_id: actual.host_id.clone(),
        boot_id: actual.boot_id.clone(),
        offer_generation: actual.offer_generation,
    };
    (config, image, checked, actual)
}
#[test]
fn staged_leases_survive_final_binding_without_reset_or_recharge() {
    let (config, image, checked, actual) = fixture();
    let staged = StagedImageMemory::verified(
        &config,
        &image,
        &checked,
        &conduit_workspace_make::catalog(),
    )
    .unwrap();
    let counter = staged.used.clone();
    let metadata = StagedImageMemory::counter_backing_bytes();
    let mut lease = staged.reserve(100).unwrap();
    assert_eq!(counter.load(Ordering::Acquire), metadata + 100);
    let mut ledger = KernelResourceLedger::new(&actual).unwrap();
    staged.bind(&mut ledger, &actual, &image).unwrap();
    assert!(Arc::ptr_eq(&counter, &ledger.image_memory_observer()));
    assert_eq!(counter.load(Ordering::Acquire), metadata + 100);
    let second = ledger.reserve_managed_base_memory(20).unwrap();
    lease.retain_result(40);
    assert_eq!(counter.load(Ordering::Acquire), metadata + 60);
    drop(lease);
    drop(second);
    assert_eq!(counter.load(Ordering::Acquire), metadata);
    assert!(ledger.bind_image_bounds(&image.manifest.bounds).is_err());
}
#[test]
fn staged_binding_refuses_changed_context_and_failure_releases_its_leases() {
    let (config, image, checked, actual) = fixture();
    for mutation in 0..6 {
        let staged = StagedImageMemory::verified(
            &config,
            &image,
            &checked,
            &conduit_workspace_make::catalog(),
        )
        .unwrap();
        let counter = staged.used.clone();
        let lease = staged.reserve(100).unwrap();
        let mut destination = actual.clone();
        let mut other_image = image.clone();
        match mutation {
            0 => destination.host_id = "fixture/foreign-host".into(),
            1 => destination.boot_id = "fixture/foreign-boot".into(),
            2 => destination.offer_generation.0 += 1,
            3 => other_image.manifest.image_id.push('x'),
            4 => other_image.manifest.bounds.heap_arena_bytes += 1,
            _ => {}
        }
        let mut ledger = KernelResourceLedger::new(&destination).unwrap();
        if mutation == 5 {
            ledger.bind_image_bounds(&image.manifest.bounds).unwrap();
        }
        assert!(staged
            .bind(&mut ledger, &destination, &other_image)
            .is_err());
        assert_eq!(
            counter.load(Ordering::Acquire),
            StagedImageMemory::counter_backing_bytes() + 100
        );
        drop(lease);
        assert_eq!(
            counter.load(Ordering::Acquire),
            StagedImageMemory::counter_backing_bytes()
        );
    }
}
#[test]
fn verified_staging_refuses_unverified_images_and_pressure_without_changing_charge() {
    let (config, image, checked, _) = fixture();
    let mut changed = image.clone();
    changed.manifest.bounds.heap_arena_bytes += 1;
    assert!(StagedImageMemory::verified(
        &config,
        &changed,
        &checked,
        &conduit_workspace_make::catalog()
    )
    .is_err());
    let staged = StagedImageMemory::verified(
        &config,
        &image,
        &checked,
        &conduit_workspace_make::catalog(),
    )
    .unwrap();
    let remaining = staged.limit - StagedImageMemory::counter_backing_bytes();
    let lease = staged.reserve(remaining).unwrap();
    assert!(staged.reserve(1).is_err());
    assert!(staged.reserve(0).is_err());
    assert!(staged.reserve(usize::MAX).is_err());
    drop(lease);
    assert!(staged.reserve(remaining).is_ok());
}

#[test]
fn root_preparation_values_keep_nested_custody_across_final_binding() {
    use crate::body_execution::ImageRootPreparation;
    let (config, image, checked, actual) = fixture();
    let staged = StagedImageMemory::verified(
        &config,
        &image,
        &checked,
        &conduit_workspace_make::catalog(),
    )
    .unwrap();
    let counter = staged.used.clone();
    let metadata = StagedImageMemory::counter_backing_bytes();
    let overhead = ImageMemoryLease::owned_backing_bytes();
    let value = {
        let mut reserve = |bytes| staged.reserve(bytes);
        let scope = ImageRootPreparation::new(&mut reserve);
        // SAFETY: fixed small values fit these independently bounded envelopes.
        unsafe {
            scope.prepare_value(4096, 512, || {
                assert_eq!(counter.load(Ordering::Acquire), metadata + 4096 + overhead);
                let nested = scope.prepare_value(256, 128, || {
                    assert_eq!(
                        counter.load(Ordering::Acquire),
                        metadata + 4096 + 256 + 2 * overhead
                    );
                    Ok(Box::new([7u8; 64]))
                })?;
                Ok((vec![9u8; 64], nested))
            })
        }
        .unwrap()
    };
    assert_eq!(value.value().0[0], 9);
    let retained = metadata + 512 + 128 + 2 * overhead;
    assert_eq!(counter.load(Ordering::Acquire), retained);
    let mut ledger = KernelResourceLedger::new(&actual).unwrap();
    staged.bind(&mut ledger, &actual, &image).unwrap();
    let (data, charge) = value.into_parts();
    ledger.require_input_memory_owner(&charge).unwrap();
    let mut copied_identity = KernelResourceLedger::new(&actual).unwrap();
    copied_identity
        .bind_image_bounds(&image.manifest.bounds)
        .unwrap();
    assert!(copied_identity.require_input_memory_owner(&charge).is_err());
    drop(ledger);
    assert_eq!(counter.load(Ordering::Acquire), retained);
    assert_eq!(data.1.value()[0], 7);
    drop(data);
    assert_eq!(counter.load(Ordering::Acquire), metadata + 512 + overhead);
    drop(charge);
    assert_eq!(counter.load(Ordering::Acquire), metadata);
}

#[test]
fn root_preparation_refuses_before_effects_and_releases_error_or_unwind() {
    use crate::body_execution::ImageRootPreparation;
    let (config, image, checked, _) = fixture();
    let staged = StagedImageMemory::verified(
        &config,
        &image,
        &checked,
        &conduit_workspace_make::catalog(),
    )
    .unwrap();
    let counter = staged.used.clone();
    let metadata = StagedImageMemory::counter_backing_bytes();
    let mut reserve = |bytes| staged.reserve(bytes);
    let scope = ImageRootPreparation::new(&mut reserve);
    // SAFETY: rejected bounds invoke no callback; the error/unwind factories
    // allocate no retained value and leave no worker or external effect behind.
    unsafe {
        for (peak, retained) in [(0, 8), (8, 9), (usize::MAX, 8), (staged.limit, 8)] {
            assert!(scope
                .prepare_value::<usize>(peak, retained, || panic!("unadmitted factory"))
                .is_err());
            assert_eq!(counter.load(Ordering::Acquire), metadata);
        }
        assert!(scope
            .prepare_value::<usize>(128, 8, || Err("fixture preparation error".into()))
            .is_err());
        assert_eq!(counter.load(Ordering::Acquire), metadata);
        let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = scope.prepare_value::<usize>(128, 8, || panic!("fixture unwind"));
        }));
        assert!(unwind.is_err());
        assert_eq!(counter.load(Ordering::Acquire), metadata);
    }
}

#[test]
fn image_memory_retention_outlives_value_without_retaining_its_data() {
    use crate::body_execution::ImageRootPreparation;
    use std::{cell::Cell, rc::Rc};
    struct Data(Rc<Cell<bool>>);
    impl Drop for Data {
        fn drop(&mut self) {
            self.0.set(true);
        }
    }
    let (config, image, checked, _) = fixture();
    let staged = StagedImageMemory::verified(
        &config,
        &image,
        &checked,
        &conduit_workspace_make::catalog(),
    )
    .unwrap();
    let counter = staged.used.clone();
    let metadata = StagedImageMemory::counter_backing_bytes();
    let destroyed = Rc::new(Cell::new(false));
    let value = {
        let mut reserve = |bytes| staged.reserve(bytes);
        let root = ImageRootPreparation::new(&mut reserve);
        // SAFETY: fixed boxed data fits this envelope; its independent observer
        // is caller-owned, and no effect or worker can escape preparation.
        unsafe { root.prepare_value(1024, 512, || Ok(Box::new(Data(destroyed.clone())))) }.unwrap()
    };
    let retained = metadata + 512 + ImageMemoryLease::owned_backing_bytes();
    let witness = value.retain_memory();
    let alias = witness.clone();
    drop(value);
    assert!(destroyed.get());
    assert_eq!(counter.load(Ordering::Acquire), retained);
    drop(witness);
    assert_eq!(counter.load(Ordering::Acquire), retained);
    drop(alias);
    assert_eq!(counter.load(Ordering::Acquire), metadata);
}
