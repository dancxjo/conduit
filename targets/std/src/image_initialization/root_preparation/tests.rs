use super::*;
use std::{cell::Cell, sync::atomic::Ordering};

fn fixture() -> (
    StdHostConfig,
    conduit_host_make::CheckedHostConfiguration,
    HostImage,
    MakeCatalog,
) {
    use conduit_host_make::{
        build_default_host_image, check_host_configuration, parse_host_configuration_conduit,
        BuildInputs,
    };
    let catalog = conduit_workspace_make::catalog();
    let checked = check_host_configuration(
        parse_host_configuration_conduit(include_str!(
            "../../../profiles/raspberry-pi-3-tick-workset.host.conduit"
        ))
        .unwrap(),
        &catalog,
        &conduit_workspace_make::package_set(),
    )
    .unwrap();
    let image = build_default_host_image(
        checked.profile().clone(),
        &catalog,
        &conduit_workspace_make::package_set(),
        &BuildInputs {
            source_identity: "fixture/pre-host-root".into(),
            toolchain_available: true,
        },
    )
    .unwrap()
    .0;
    (
        StdHostConfig {
            host_id: "fixture/pre-host".into(),
            boot_id: "boot/pre-host".into(),
            offer_generation: OfferGeneration(1),
        },
        checked,
        image,
        catalog,
    )
}

fn initialize<T>(
    config: StdHostConfig,
    image: &HostImage,
    checked: &conduit_host_make::CheckedHostConfiguration,
    catalog: &MakeCatalog,
    recipe: impl FnOnce(&ImageRootPreparation<'_>) -> Result<ImagePreparedValue<T>, String>,
) -> Result<(StdHost, ImagePreparedValue<T>), String> {
    let prepared = StdHost::prepare_selected_image_root(&config, image, checked, catalog, recipe)?;
    StdHost::from_selected_timer_image_with_prepared_root(prepared, catalog)
}

#[test]
fn prepared_value_binds_to_final_host_and_last_retention_keeps_charge() {
    let (config, checked, image, catalog) = fixture();
    let (host, prepared) = initialize(config, &image, &checked, &catalog, |scope| {
        // SAFETY: complete fixed Vec and bounded drop fit4096; inputs are
        // independently owned by this hosted test before observation.
        unsafe { scope.prepare_value(4096, 4096, || Ok(Vec::<u8>::with_capacity(1024))) }
    })
    .unwrap();
    host.verify_selected_image_prepared_value(&prepared)
        .unwrap();
    let counter = host.kernel_resources.image_memory_observer();
    let retention = prepared.retain_memory();
    let retained = 4096 + crate::kernel_preparation::ImageMemoryLease::owned_backing_bytes();
    assert!(counter.load(Ordering::Acquire) >= retained);
    drop(host);
    drop(prepared);
    assert_eq!(
        counter.load(Ordering::Acquire),
        retained + StagedImageMemory::counter_backing_bytes()
    );
    drop(retention);
    assert_eq!(
        counter.load(Ordering::Acquire),
        StagedImageMemory::counter_backing_bytes()
    );
}

#[test]
fn rejects_foreign_result_even_when_intended_image_labels_match() {
    let (config, checked, image, catalog) = fixture();
    let mut other = StdHost::from_selected_timer_image(config.clone(), &image, &catalog).unwrap();
    // SAFETY: fixed inline scalar, no owned descendants/effects.
    let foreign = unsafe { other.prepare_selected_image_value(64, 64, || Ok(7u64)) }.unwrap();
    let error = initialize(config, &image, &checked, &catalog, |_| Ok(foreign))
        .err()
        .unwrap();
    assert!(error.contains("another original IMAGE counter"));
}

#[test]
fn invalid_image_refuses_before_recipe_and_pressure_refuses_before_real_factory() {
    let (config, checked, mut image, catalog) = fixture();
    let called = Cell::new(false);
    image.manifest.bounds.heap_arena_bytes += 1;
    assert!(initialize::<u64>(config, &image, &checked, &catalog, |_| {
        called.set(true);
        Err("must not enter".into())
    })
    .is_err());
    assert!(!called.get());
    let (config, checked, image, catalog) = fixture();
    let factory = Cell::new(false);
    let result = initialize::<u64>(config, &image, &checked, &catalog, |scope| {
        // SAFETY: an oversized envelope must refuse before this factory.
        unsafe {
            scope.prepare_value(64 * 1024 * 1024, 64, || {
                factory.set(true);
                Ok(7)
            })
        }
    });
    assert!(result.is_err());
    assert!(!factory.get());
}

#[test]
fn recipe_refusal_and_unwind_do_not_construct_or_publish_a_host() {
    let (config, checked, image, catalog) = fixture();
    assert!(
        initialize::<u64>(config, &image, &checked, &catalog, |_| Err(
            "cold refusal".into()
        ))
        .is_err()
    );
    let (config, checked, image, catalog) = fixture();
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        initialize::<u64>(config, &image, &checked, &catalog, |scope| {
            // SAFETY: fixed scalar factory has no effects or descendants.
            unsafe { scope.prepare_value(64, 64, || panic!("cold unwind")) }
        })
    }))
    .is_err());
}
