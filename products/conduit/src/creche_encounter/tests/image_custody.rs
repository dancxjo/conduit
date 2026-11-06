//! Actual factory/controller ownership; not installed Crèche IMAGE execution.
use super::*;
use conduit_std_host::{
    body_execution::ImageRootPreparation, ImmediateNativeSource, ImmediateSourcePreparationBudget,
    NativeSourceDefinition, StdHost, StdHostConfig,
};

type UnusedNativeFactory = fn() -> Result<Box<dyn ImmediateNativeSource>, String>;
type UnusedMetadata = (
    NativeSourceDefinition,
    ImmediateSourcePreparationBudget,
    UnusedNativeFactory,
);

fn configuration() -> StdHostConfig {
    StdHostConfig {
        host_id: "host/creche-image-custody".into(),
        boot_id: "boot/creche-image-custody".into(),
        offer_generation: conduit_core::OfferGeneration(1),
    }
}

fn with_verified_root(operation: impl FnOnce(&ImageRootPreparation<'_>)) -> StdHost {
    let catalog = conduit_workspace_make::catalog();
    let packages = conduit_workspace_make::package_set();
    let source = include_str!(
        "../../../../../targets/std/profiles/raspberry-pi-3-tick-workset.host.conduit"
    );
    let checked = conduit_host_make::check_host_configuration(
        conduit_host_make::parse_host_configuration_conduit(source).unwrap(),
        &catalog,
        &packages,
    )
    .unwrap();
    let image = conduit_host_make::build_default_host_image(
        checked.profile().clone(),
        &catalog,
        &packages,
        &BuildInputs {
            source_identity: "fixture/creche-image-custody".into(),
            toolchain_available: true,
        },
    )
    .unwrap()
    .0;
    // SAFETY: this exercises verified Root staging only. The metadata callback
    // always refuses before providing any Source definition, grant or native
    // factory. Its fixed fixture captures fit the independent metadata envelope.
    let result = unsafe {
        StdHost::from_selected_timer_and_source_factory_image(
            configuration(),
            &image,
            &catalog,
            &checked,
            ImmediateSourcePreparationBudget {
                preparation_bytes: 4 * 1024 * 1024,
                retained_bytes: 4 * 1024 * 1024,
            },
            None,
            |_, root| -> Result<UnusedMetadata, String> {
                operation(root);
                Err("fixture stops before native construction".into())
            },
        )
    };
    assert!(result
        .err()
        .unwrap()
        .contains("fixture stops before native construction"));
    // Same declared IDs and IMAGE, but a different actual memory counter.
    StdHost::from_selected_timer_image(configuration(), &image, &catalog).unwrap()
}

#[test]
fn image_charge_follows_controller_after_adapter_factory_and_staging_drop() {
    let mut prepared = None;
    let mut directory = None;
    let copied_identity = with_verified_root(|root| {
        // SAFETY: fixed reviewed fixture files/catalogs fit these conservative
        // envelopes. Preparation starts no worker and exports no input alias.
        prepared = Some(
            unsafe {
                CrecheEncounterFactory::prepare_in_image(
                    root,
                    4 * 1024 * 1024,
                    2 * 1024 * 1024,
                    || {
                        let ((factory, files), measured) =
                            crate::allocation_probe::observe(factory);
                        // Requested Rust-allocation evidence for this fixture,
                        // not process RSS or a whole installed admission bound.
                        assert!(measured.peak_live_bytes < 4 * 1024 * 1024);
                        assert!(measured.live_bytes + 4096 < 2 * 1024 * 1024);
                        eprintln!("Crèche factory allocation fixture: {measured:?}");
                        directory = Some(files);
                        Ok(factory)
                    },
                )
            }
            .unwrap(),
        );
        // SAFETY: this fixed no-effect probe must refuse: 60 MiB plus the
        // retained factory and metadata exceed the same 64 MiB IMAGE budget.
        assert!(
            unsafe { root.prepare_value(60 * 1024 * 1024, 60 * 1024 * 1024, || Ok(())) }
                .err()
                .unwrap()
                .contains("backing capacity")
        );
    });
    let mut prepared = prepared.unwrap();
    assert!(prepared
        .verify_image_custody(&copied_identity)
        .unwrap_err()
        .contains("another memory owner"));
    let InputsCustody::Image(owner) = &prepared.inputs else {
        panic!("funded factory expected")
    };
    let original_charge = Arc::downgrade(owner);
    let (adapter, controller) = prepared
        .encounter(
            binding(),
            Box::new(Custody {
                admitted: true,
                drained: Arc::new(AtomicBool::new(false)),
            }),
        )
        .unwrap();
    drop(adapter);
    drop(prepared);
    assert!(original_charge.upgrade().is_some());
    // Channel state still owns the original funded inputs, even though no Host
    // was constructed and the adapter/factory are gone.
    assert_eq!(
        controller.channel.inputs.installation.encounter_id,
        "encounter/test"
    );
    drop(controller);
    assert!(original_charge.upgrade().is_none());
    drop(directory);
}

#[test]
fn image_pressure_refuses_before_factory_and_existing_alias_refuses_transfer() {
    let mut called = false;
    with_verified_root(|root| {
        // SAFETY: bounds refuse before invoking this deliberately unreachable
        // factory. No file, worker, Source or provider effect can start.
        assert!(unsafe {
            CrecheEncounterFactory::prepare_in_image(
                root,
                80 * 1024 * 1024,
                2 * 1024 * 1024,
                || {
                    called = true;
                    Err("unadmitted factory".into())
                },
            )
        }
        .err()
        .unwrap()
        .contains("backing capacity"));
    });
    assert!(!called);

    let (prepared, _directory) = factory();
    let independently_owned = prepared.inputs.clone();
    with_verified_root(|root| {
        // SAFETY: pre-existing reference fixture inputs retain independent
        // custody. Refused transfer leaves that original alias usable.
        assert!(unsafe {
            CrecheEncounterFactory::prepare_in_image(root, 4 * 1024 * 1024, 2 * 1024 * 1024, || {
                Ok(prepared)
            })
        }
        .err()
        .unwrap()
        .contains("unique unfunded inputs"));
    });
    assert_eq!(
        independently_owned.installation.encounter_id,
        "encounter/test"
    );
}
