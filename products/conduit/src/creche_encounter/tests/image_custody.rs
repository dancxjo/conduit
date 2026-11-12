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

pub(super) fn with_verified_root(operation: impl FnOnce(&ImageRootPreparation<'_>)) -> StdHost {
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
fn failed_initial_host_keeps_input_custody_but_cannot_create_an_unfunded_encounter() {
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
    assert!(prepared
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
    assert!(original_charge.upgrade().is_some());
    assert_eq!(prepared.limits().maximum_evidence_bytes, 65536);
    drop(prepared);
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

fn creche_image_fixture() -> (conduit_host_make::CheckedHostConfiguration, HostImage) {
    // Crèche is installed on the x86 Linux authoring Host, not the Pi target
    // whose Seed it makes. Keep the existing 64 MiB declared heap unchanged.
    let source = source().replace("clock/monotonic", "timer/monotonic").replace("  limits =", r#"
  capability = {kind: "time/tick", pinned_implementation: "std/kernel-time-tick@2"}
  need = {id: "timer/main", class: "conduit.resource/timer-slot@1", slots: 1, bytes: 512}
  base = {kind: "creche/encounter", implementation: "std/kernel-creche-prepare@1"}
  capability = {kind: "creche/prepare", pinned_implementation: "std/kernel-creche-prepare@1"}
  need = {id: "pool/creche", class: "conduit.resource/admitted-creche-encounter@1", slots: 1, bytes: 4096}
  limits ="#);
    let catalog = conduit_workspace_make::catalog();
    let packages = conduit_workspace_make::package_set();
    let checked = conduit_host_make::check_host_configuration(
        conduit_host_make::parse_host_configuration_conduit(&source).unwrap(),
        &catalog,
        &packages,
    )
    .unwrap();
    let image = conduit_host_make::build_default_host_image(
        checked.profile().clone(),
        &catalog,
        &packages,
        &BuildInputs {
            source_identity: "fixture/initial-funded-creche".into(),
            toolchain_available: true,
        },
    )
    .unwrap()
    .0;
    (checked, image)
}

pub(super) fn initialized_factory() -> (
    StdHost,
    conduit_std_host::body_execution::ImagePreparedValue<
        std::cell::RefCell<CrecheEncounterFactory>,
    >,
    bundle_common::PrivateDirectory,
) {
    let (checked, image) = creche_image_fixture();
    let mut directory = None;
    // SAFETY: fixed metadata and immutable reviewed fixture inputs fit their
    // separate envelopes. Preparation starts no worker. The exact resource is
    // obtained from the actual initialized factory, not inferred from IMAGE.
    let (host, prepared) = unsafe {
        StdHost::from_selected_timer_and_creche_factory_image(
            configuration(),
            &image,
            &conduit_workspace_make::catalog(),
            &checked,
            4 * 1024 * 1024,
            2 * 1024 * 1024,
            |root| {
                let provider = root.prepare_value(4096, 512, || {
                    CrecheEncounterFactory::prepare_in_image(
                        root,
                        4 * 1024 * 1024,
                        2 * 1024 * 1024,
                        || {
                            let (factory, files) = factory();
                            directory = Some(files);
                            Ok(factory)
                        },
                    )
                    .map(std::cell::RefCell::new)
                })?;
                let resource = provider.value().borrow().resource_offer();
                Ok((resource, provider))
            },
        )
    }
    .unwrap();
    (host, prepared, directory.unwrap())
}

#[test]
fn actual_funded_factory_binds_to_initial_host_without_a_source_checker() {
    let (host, prepared, directory) = initialized_factory();
    host.verify_selected_image_prepared_value(&prepared)
        .unwrap();
    prepared
        .value()
        .borrow_mut()
        .verify_image_custody(&host)
        .unwrap();
    assert!(host
        .advertisement()
        .resources
        .contains(&prepared.value().borrow().resource_offer()));
    assert!(host
        .advertisement()
        .capabilities
        .contains(&conduit_body_make::deployment_runtime_catalog::creche_std_offer()));
    assert_eq!(host.advertisement().capabilities.len(), 2); // timer and Crèche only
    drop(host);
    assert_eq!(
        prepared.value().borrow().limits().maximum_evidence_bytes,
        65536
    );
    drop(prepared);
    drop(directory);
}

#[test]
fn initial_creche_metadata_pressure_refuses_before_actual_factory() {
    let (checked, image) = creche_image_fixture();
    let mut called = false;
    // SAFETY: metadata exceeds the checked 64 MiB heap, so the callback must
    // never run. No provider, File or Source authority is supplied or exercised.
    let result = unsafe {
        StdHost::from_selected_timer_and_creche_factory_image::<()>(
            configuration(),
            &image,
            &conduit_workspace_make::catalog(),
            &checked,
            65 * 1024 * 1024,
            2 * 1024 * 1024,
            |_| {
                called = true;
                Err("unadmitted Crèche factory".into())
            },
        )
    };
    assert!(!called);
    assert!(result.err().unwrap().contains("backing capacity"));
}

#[test]
fn initial_creche_metadata_and_provider_peaks_compete_before_file_effects() {
    let (checked, image) = creche_image_fixture();
    let mut called = false;
    // SAFETY: fixed metadata has a conservative declared peak. Its live frame
    // plus provider preparation cannot fit the same 64 MiB IMAGE; the provider
    // callback must not reach any fixture file or other native effect.
    let result = unsafe {
        StdHost::from_selected_timer_and_creche_factory_image::<()>(
            configuration(),
            &image,
            &conduit_workspace_make::catalog(),
            &checked,
            62 * 1024 * 1024,
            2 * 1024 * 1024,
            |root| {
                let provider = root.prepare_value(4 * 1024 * 1024, 512, || {
                    called = true;
                    Ok(())
                })?;
                Ok((
                    conduit_core::resource_offer(
                        "pool/creche",
                        conduit_body_make::deployment_runtime_catalog::CRECHE_RESOURCE,
                        1,
                    ),
                    provider,
                ))
            },
        )
    };
    assert!(!called);
    assert!(result.err().unwrap().contains("backing capacity"));
}
