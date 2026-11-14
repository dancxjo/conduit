//! Actual checked IMAGE construction, not reference Host reconfiguration.
use super::host_call::{bounds, support, Capture};
use super::*;
use conduit_core::*;
use conduit_host_make::*;
use conduit_std_host::{source_check::*, *};
use std::{cell::Cell, rc::Rc};

fn configuration() -> CheckedHostConfiguration {
    configuration_with_creche(false)
}
fn configuration_with_creche(creche: bool) -> CheckedHostConfiguration {
    configuration_with_equipment(creche, false)
}
fn configuration_with_equipment(creche: bool, text_file: bool) -> CheckedHostConfiguration {
    let source = parent::configuration_source().replace("      limits =", r#"
      base = {kind: "timer/monotonic", implementation: "hosted/monotonic-clock@1"}
      capability = {kind: "time/tick", pinned_implementation: "std/kernel-time-tick@2"}
      capability = {kind: "language/source-check", pinned_implementation: "std/bounded-source-check@1"}
      need = {id: "timer/main", class: "conduit.resource/timer-slot@1", slots: 1, bytes: 512}
      need = {id: "source/results", class: "language/prepared-source-check@1", slots: 1, bytes: 1048576}
      need = {id: "source/memory", class: "conduit.resource/runtime-memory@1", slots: 570425344, bytes: 570425344}
      limits ="#);
    let source = if creche {
        source.replace("      limits =", r#"
      base = {kind: "creche/encounter", implementation: "std/kernel-creche-prepare@1"}
      capability = {kind: "creche/prepare", pinned_implementation: "std/kernel-creche-prepare@1"}
      need = {id: "creche/slot", class: "conduit.resource/admitted-creche-encounter@1", slots: 1, bytes: 4096}
      limits ="#)
    } else {
        source
    };
    let source = if text_file {
        source.replace("      limits =", r#"
      base = {kind: "storage/text-generation", implementation: "std/file-data-save-text@1", initialization: {maximum_text_bytes: 256, preparation_bytes: 32768, retained_bytes: 16384}}
      capability = {kind: "data/save/text", pinned_implementation: "std/file-data-save-text@1"}
      need = {id: "recording/generation", class: "data/immutable-generation@1", slots: 1, bytes: 256}
      limits ="#)
    } else {
        source
    };
    check_host_configuration(
        parse_host_configuration_conduit(&source).unwrap(),
        &conduit_workspace_make::catalog(),
        &conduit_workspace_make::package_set(),
    )
    .unwrap()
}
fn image(checked: &CheckedHostConfiguration) -> HostImage {
    build_default_host_image(
        checked.profile().clone(),
        &conduit_workspace_make::catalog(),
        &conduit_workspace_make::package_set(),
        &BuildInputs {
            source_identity: "fixture/checked-source-compiler-image".into(),
            toolchain_available: true,
        },
    )
    .unwrap()
    .0
}
fn definition(
    checked: &CheckedHostConfiguration,
    image: &HostImage,
) -> (StdHostConfig, NativeSourceDefinition) {
    let config = StdHostConfig {
        host_id: "fixture/authorized-compiler-host".into(),
        boot_id: "fixture/actual-compiler-boot".into(),
        offer_generation: OfferGeneration(1),
    };
    let grant = AuthorityGrant {
        grant_id: "fixture/independent-source-read-grant".into(),
        contract_id: SOURCE_CHECK_AUTHORITY.into(),
        host_call_contract_id: SOURCE_CHECK_CALL.into(),
        subject_kind: SOURCE_CHECK_KIND.into(),
        host_id: config.host_id.clone(),
        boot_id: config.boot_id.clone(),
        capability_id: conduit_std_offers::SOURCE_CHECK_IMPLEMENTATION.into(),
    };
    let definition = conduit::source_check_worker::derive_image_definition(
        &config,
        image,
        checked,
        &conduit_workspace_make::catalog(),
        &SourceCheckNativeSchema::prepare().unwrap(),
        grant,
        conduit::source_check_worker::SourceCheckProviderIdentity {
            instance: "fixture/independently-initialized-compiler".into(),
            generation: 1,
            resource_generation: ResourceGenerationId("fixture/compiler-resources@1".into()),
        },
    )
    .unwrap();
    (config, definition)
}
#[test]
fn image_definition_rejects_foreign_grants_and_ambiguous_pools() {
    let checked = configuration();
    let image = image(&checked);
    let schema = SourceCheckNativeSchema::prepare().unwrap();
    let (config, definition) = definition(&checked, &image);
    for mismatch in 0..6 {
        let mut grant = definition.grant.clone();
        match mismatch {
            0 => grant.host_id = "other/host".into(),
            1 => grant.boot_id = "other/boot".into(),
            2 => grant.capability_id = "other/capability".into(),
            3 => grant.subject_kind = "other/kind".into(),
            4 => grant.host_call_contract_id = "other/call".into(),
            _ => grant.contract_id = "other/authority".into(),
        }
        assert!(conduit::source_check_worker::derive_image_definition(
            &config,
            &image,
            &checked,
            &conduit_workspace_make::catalog(),
            &schema,
            grant,
            conduit::source_check_worker::SourceCheckProviderIdentity {
                instance: "fixture/current-provider".into(),
                generation: 1,
                resource_generation: ResourceGenerationId("fixture/current-resources".into()),
            },
        )
        .is_err());
    }
    // Build a valid, checked IMAGE with two distinct pools of the same class.
    // A broken manifest digest would only test the outer integrity guard.
    let mut ambiguous = checked.configuration().clone();
    let mut pool = ambiguous
        .resources
        .iter()
        .find(|p| p.class == SOURCE_CHECK_RESOURCE)
        .unwrap()
        .clone();
    pool.id.push_str("/second");
    ambiguous.resources.push(pool);
    let ambiguous_checked = check_host_configuration(
        ambiguous,
        &conduit_workspace_make::catalog(),
        &conduit_workspace_make::package_set(),
    )
    .unwrap();
    let ambiguous_image = self::image(&ambiguous_checked);
    verify_configuration_image(
        &ambiguous_checked,
        &ambiguous_image,
        &conduit_workspace_make::catalog(),
    )
    .unwrap();
    let error = conduit::source_check_worker::derive_image_definition(
        &config,
        &ambiguous_image,
        &ambiguous_checked,
        &conduit_workspace_make::catalog(),
        &schema,
        definition.grant,
        conduit::source_check_worker::SourceCheckProviderIdentity {
            instance: "fixture/current-provider".into(),
            generation: 1,
            resource_generation: ResourceGenerationId("fixture/current-resources".into()),
        },
    )
    .err()
    .expect("ambiguous valid pools must refuse selection");
    assert!(error.contains("ambiguous"), "{error}");
}
fn budget() -> ImmediateSourcePreparationBudget {
    ImmediateSourcePreparationBudget {
        preparation_bytes: 570425344,
        retained_bytes: 1048576,
    }
}
#[test]
fn image_refuses_mismatched_equipment_and_memory_before_running_factory() {
    let checked = configuration();
    let image = image(&checked);
    for mismatch in [false, true] {
        let (config, mut definition) = definition(&checked, &image);
        let mut budget = budget();
        if mismatch {
            definition.base.base_id = "not-in-image".into();
        } else {
            budget.preparation_bytes += 1;
        }
        let ran = Rc::new(Cell::new(false));
        let invoked = ran.clone();
        // SAFETY: Invalid equipment/capacity must refuse before effects.
        let result = unsafe {
            StdHost::from_selected_timer_and_immediate_source_image(
                config,
                &image,
                &conduit_workspace_make::catalog(),
                &checked,
                definition,
                budget,
                move || {
                    invoked.set(true);
                    Err("must not run".into())
                },
            )
        };
        assert!(result.is_err());
        assert!(!ran.get());
    }
}
#[test]
fn actual_compiler_initializes_inside_image_and_preserves_execution_admission() {
    for source in [
        include_str!("../../../../plots/hello/main.conduit"),
        "plot hello { first: unavailable/kind }\n",
    ] {
        run_image_case(source);
    }
}
#[test]
fn actual_compiler_and_creche_share_initial_image_without_widening_source_authority() {
    run_image_case_with_creche(include_str!("../../../../plots/hello/main.conduit"), true);
}
fn run_image_case(source: &str) {
    run_image_case_with_creche(source, false);
}
fn run_image_case_with_creche(source: &str, creche: bool) {
    run_image_case_with_equipment(source, creche, false);
}
#[path = "image_text.rs"]
mod text;
#[test]
fn actual_compiler_and_text_file_share_original_image_without_extra_runtime_pool() {
    run_image_case_with_equipment(
        include_str!("../../../../plots/hello/main.conduit"),
        false,
        true,
    );
}
fn run_image_case_with_equipment(source: &str, creche: bool, text_file: bool) {
    let checked = configuration_with_equipment(creche, text_file);
    let image = image(&checked);
    let schema = SourceCheckNativeSchema::prepare().unwrap();
    let kind = PreparedSourceChecker::kind_for_bounds(&schema, bounds()).unwrap();
    let (config, definition) = definition(&checked, &image);
    let (file_directory, file_initialization) = if text_file {
        let (directory, initialization) = text::initialization(&config, &image);
        (Some(directory), Some(initialization))
    } else {
        (None, None)
    };
    let file_grant = file_initialization
        .as_ref()
        .map(|file| file.definition.authority.clone());
    let grant = definition.grant.clone();
    let provider = definition.base.provider_instance_id.clone();
    let (input, mut owner) = input(source);
    let expected = schema
        .encode_reply(
            SourceCheckReply::Checked(&owner.project(&input.request).unwrap()),
            bounds().report_bytes as usize,
        )
        .unwrap();
    let expected_body_reply = expected.clone();
    let request = schema
        .encode_request(&input.request, bounds().request_bytes as usize)
        .unwrap();
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    schema.install_types(&mut startup).unwrap();
    startup
        .insert(KindSignature {
            kind: SOURCE_CHECK_KIND.into(),
            startup_parameters: vec![],
        })
        .unwrap();
    startup
        .insert_fore(SOURCE_CHECK_KIND, kind.checked_front())
        .unwrap();
    profiles.insert_kind(kind).unwrap();
    let control = RunControl::default();
    let catalog = conduit_workspace_make::catalog();
    // SAFETY: Fixture owns this selected File, exact Source scope and independent
    // Host/Boot grant. Its caller metadata has separate preparation custody.
    let initialized = unsafe {
        conduit::source_check_worker::initialize_image_with_caller_workspace(
            conduit::source_check_worker::SourceCheckImageInstallation {
                config,
                image: &image,
                checked: &checked,
                catalog: &catalog,
                grant: definition.grant,
                provider: conduit::source_check_worker::SourceCheckProviderIdentity {
                    instance: definition.base.provider_instance_id,
                    generation: definition.base.provider_generation,
                    resource_generation: definition.resource_generation,
                },
                file: source_file(source),
                maximum_source_bytes: bounds().request_bytes as usize,
                capture_budget: ImmediateSourcePreparationBudget {
                    preparation_bytes: 512 * 1024,
                    retained_bytes: 16 * 1024,
                },
                snapshot: &input.request.snapshot,
                entry: &input.request.entry,
                context: input.request.context.clone(),
                control: &control,
            },
            creche.then(|| {
                resource_offer(
                    "creche/slot",
                    conduit_body_make::deployment_runtime_catalog::CRECHE_RESOURCE,
                    1,
                )
            }),
            None,
            file_initialization,
        )
    }
    .unwrap();
    assert_eq!(initialized.request.input().bytes, request);
    assert_eq!(initialized.source.value().source(), source);
    assert_eq!(initialized.source.value().request(), &input.request);
    assert!(initialized.source.value().owned_backing_bytes().unwrap() <= 16 * 1024);
    initialized
        .host
        .verify_selected_image_prepared_value(&initialized.source)
        .unwrap();
    let foreign = StdHost::new();
    assert!(foreign
        .verify_selected_image_prepared_value(&initialized.source)
        .is_err());
    let mut retained_source = Some(initialized.source);
    let mut host = initialized.host;
    assert_eq!(host.text_file_authority_grant(), file_grant.as_ref());
    if let Some(directory) = &file_directory {
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
    }
    if !creche {
        let before =
            remaining_preparation(&mut host, image.manifest.bounds.heap_arena_bytes as usize);
        let witness = retained_source.as_ref().unwrap().retain_memory();
        host.verify_selected_image_memory_retention(&witness)
            .unwrap();
        assert!(foreign
            .verify_selected_image_memory_retention(&witness)
            .is_err());
        drop(retained_source.take());
        assert_eq!(
            remaining_preparation(&mut host, image.manifest.bounds.heap_arena_bytes as usize),
            before
        );
        drop(witness);
        assert!(
            remaining_preparation(&mut host, image.manifest.bounds.heap_arena_bytes as usize)
                >= before + 16 * 1024
        );
    }
    assert_eq!(
        host.image_identity().unwrap().image_id,
        image.manifest.image_id
    );
    assert_eq!(
        host.advertisement().capabilities.len(),
        2 + usize::from(creche) + usize::from(text_file)
    );
    if creche {
        assert!(host
            .advertisement()
            .capabilities
            .contains(&conduit_body_make::deployment_runtime_catalog::creche_std_offer()));
    }
    let syntax = check_syntax_document(
        &parse_syntax_document(include_str!(
            "../../../../make/body/tests/source_check_call/check.conduit"
        )),
        &startup,
    )
    .unwrap();
    let authored = expand_canonical_plot_for_authoring(&syntax, "check-source", &profiles).unwrap();
    assert!(support::plan(&host, &authored, &[], bounds()).is_err());
    let plan = support::plan(&host, &authored, &[grant], bounds()).unwrap();
    let mut capture = Capture {
        bytes: vec![],
        calls: 0,
    };
    let refusal = host
        .run_external_plot_to(
            plan.fragments[0].clone(),
            &[ExternalForeInput {
                front_port_id: "request".into(),
                track: ConnectionTrack::Payload,
                bytes: request.clone(),
            }],
            &mut capture,
            &mut Vec::new(),
            &mut ThreadTimer,
        )
        .expect_err("ordinary IMAGE execution still requires its complete managed-memory receipt");
    assert!(
        refusal.contains("complete ordinary managed-memory accounting"),
        "{refusal}"
    );
    assert_eq!(capture.calls, 0);
    let plot = conduit_body::ResidentPlot::new(
        plan.source_document_id.clone(),
        plan.checked_plot_id.clone(),
    );
    let body = conduit_body::Body::born_with_plots(
        conduit_body::BodyWorkset::from_plots([plot.clone()]).unwrap(),
        1,
        "image-source/born".into(),
    )
    .unwrap();
    let wake = body.wake(1, "image-source/woke".into()).unwrap().1;
    let body_plan =
        conduit_body::BodyPlan::seal(&wake, vec![conduit_body::BodyPlotPlan { plot, plan }])
            .unwrap();
    let mut capture = BoundedBodyCapture {
        bytes: Vec::with_capacity(expected_body_reply.len()),
        calls: 0,
    };
    let mut probe = None;
    let result = host.run_selected_image_body_plan_with_retained_input_to_with_start(
        conduit_std_host::body_execution::BodyRunRequest {
            wake: &wake,
            plan: &body_plan,
            control: &RunControl::default(),
            keyboard: None,
        },
        &initialized.request,
        &mut Vec::new(),
        &mut ThreadTimer,
        &mut capture,
        |_, _| {
            probe = Some(crate::allocation_probe::begin());
            Ok(())
        },
    );
    let current = crate::allocation_probe::current();
    let allocations = probe.take().expect("actual admitted start").finish();
    let report = result.unwrap();
    assert_eq!(current, allocations);
    assert_eq!(
        allocations, 0,
        "selected Source Play must not allocate after start"
    );
    assert_eq!(
        report.report().terminal,
        TerminalDisposition::Completed,
        "{:?}",
        report.report().failure
    );
    assert_eq!(capture.calls, 1);
    assert_eq!(capture.bytes, expected_body_reply);
    for kind in [
        conduit_kernel::KernelEventKind::HostCallRequested,
        conduit_kernel::KernelEventKind::HostCallCompleted,
    ] {
        assert!(report
            .report()
            .kernel_events
            .iter()
            .any(|event| event.kind == kind));
    }
    drop(report);
    let control = RunControl::default();
    control
        .request_stop(RunControlRequestId::new("image-source/stop").unwrap())
        .unwrap();
    capture.bytes.clear();
    capture.calls = 0;
    let mut probe = None;
    let cancelled = host.run_selected_image_body_plan_with_retained_input_to_with_start(
        conduit_std_host::body_execution::BodyRunRequest {
            wake: &wake,
            plan: &body_plan,
            control: &control,
            keyboard: None,
        },
        &initialized.request,
        &mut Vec::new(),
        &mut ThreadTimer,
        &mut capture,
        |_, _| {
            probe = Some(crate::allocation_probe::begin());
            Ok(())
        },
    );
    let allocations = probe.take().expect("admitted cancellation start").finish();
    let cancelled = cancelled.unwrap();
    assert_eq!(
        allocations, 0,
        "selected Source cancellation must not allocate after start"
    );
    assert!(matches!(
        cancelled.report().terminal,
        TerminalDisposition::Cancelled { .. }
    ));
    assert_eq!(capture.calls, 0);
    assert!(cancelled.report().fore_deliveries.is_empty());
    drop(cancelled);
    let reserved = host.reserve_provider_resources(
        &provider,
        1,
        &[ResourceBinding {
            pool_id: "source/memory".into(),
            class_id: RUNTIME_MEMORY_RESOURCE_CLASS.into(),
            units: budget().preparation_bytes,
            protected: None,
            compute: None,
            content: None,
        }],
    );
    assert!(
        reserved.is_err(),
        "retained provider backing must remain charged across completed and cancelled Plays"
    );
    // SAFETY: No active Play; IMAGE reconfiguration must still refuse.
    assert!(unsafe { host.retire_immediate_native_source() }.is_err());
    drop(host);
    if let Some(source_owner) = retained_source {
        // The actual immutable capture and its original charge outlive the Host;
        // neither the process report nor a filesystem reread supplies these bytes.
        assert_eq!(source_owner.value().source(), source);
        assert_eq!(source_owner.value().request(), &input.request);
    }
}

fn remaining_preparation(host: &mut StdHost, ceiling: usize) -> usize {
    let (mut low, mut high) = (0usize, ceiling);
    while low < high {
        let bytes = low + (high - low).div_ceil(2);
        // SAFETY: the immediate factory owns one inline byte, no asynchronous
        // effects or external captures. The envelope deliberately overbounds it.
        let probe = unsafe { host.prepare_selected_image_value(bytes, 1, || Ok(0u8)) };
        if probe.is_ok() {
            low = bytes;
        } else {
            high = bytes - 1;
        }
        drop(probe);
    }
    low
}

#[test]
fn failed_initializer_returns_no_image_host_or_ready_provider() {
    let checked = configuration();
    let image = image(&checked);
    let (config, definition) = definition(&checked, &image);
    let ran = Rc::new(Cell::new(false));
    let invoked = ran.clone();
    // SAFETY: This bounded refusal has no live effect or retained native owner.
    let result = unsafe {
        StdHost::from_selected_timer_and_immediate_source_image(
            config,
            &image,
            &conduit_workspace_make::catalog(),
            &checked,
            definition,
            budget(),
            move || {
                invoked.set(true);
                Err("initialization refused without live work".into())
            },
        )
    };
    assert!(ran.get());
    assert_eq!(
        result.err().unwrap(),
        "initialization refused without live work"
    );
}

#[test]
fn image_heap_includes_the_actual_retained_charge_owner_before_effects() {
    let original = configuration();
    let mut source = original.configuration().clone();
    source.limits.heap_arena_bytes = u64::from(budget().preparation_bytes);
    let checked = check_host_configuration(
        source,
        &conduit_workspace_make::catalog(),
        &conduit_workspace_make::package_set(),
    )
    .unwrap();
    let image = image(&checked);
    let (config, definition) = definition(&checked, &image);
    let ran = Rc::new(Cell::new(false));
    let invoked = ran.clone();
    // SAFETY: IMAGE has no room for the actual charge owner; no effect may run.
    let result = unsafe {
        StdHost::from_selected_timer_and_immediate_source_image(
            config,
            &image,
            &conduit_workspace_make::catalog(),
            &checked,
            definition,
            budget(),
            move || {
                invoked.set(true);
                Err("must not run".into())
            },
        )
    };
    assert!(!ran.get());
    assert!(result
        .err()
        .unwrap()
        .contains("managed heap backing capacity"));
}

struct BoundedBodyCapture {
    bytes: Vec<u8>,
    calls: usize,
}
impl conduit_std_host::body_execution::BodyForeOutputAdapter for BoundedBodyCapture {
    fn deliver(&mut self, value: &ExternalForeDelivery) -> Result<(), String> {
        assert!(self.bytes.len() + value.bytes.len() <= self.bytes.capacity());
        self.bytes.extend_from_slice(&value.bytes);
        self.calls += 1;
        Ok(())
    }
}

// A real selected resource, unlinked before preparation. No pathname survives
// for the worker or packet labels to reopen or replace.
fn source_file(source: &str) -> std::fs::File {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let path = std::env::temp_dir().join(format!(
        "conduit-image-source-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let mut file = std::fs::OpenOptions::new()
        .create_new(true)
        .read(true)
        .write(true)
        .open(&path)
        .unwrap();
    file.write_all(source.as_bytes()).unwrap();
    std::fs::remove_file(path).unwrap();
    file
}

#[test]
fn product_installation_refuses_grant_drift_and_stop_before_source_capture() {
    use std::io::Seek;
    for stopped in [false, true] {
        let checked = configuration();
        let image = image(&checked);
        let (config, mut definition) = definition(&checked, &image);
        let source = include_str!("../../../../plots/hello/main.conduit");
        let (input, _) = input(source);
        let file = source_file(source);
        let mut probe = file.try_clone().unwrap();
        probe.rewind().unwrap();
        let control = RunControl::default();
        if stopped {
            control
                .request_stop(RunControlRequestId::new("before-source-capture").unwrap())
                .unwrap();
        } else {
            definition.grant.boot_id = "other/boot".into();
        }
        let catalog = conduit_workspace_make::catalog();
        // SAFETY: Independent fixture File/scope/grant; refusal must precede
        // Source capture and publish no initialized Host or provider.
        let result = unsafe {
            conduit::source_check_worker::initialize_image(
                conduit::source_check_worker::SourceCheckImageInstallation {
                    config,
                    image: &image,
                    checked: &checked,
                    catalog: &catalog,
                    grant: definition.grant,
                    provider: conduit::source_check_worker::SourceCheckProviderIdentity {
                        instance: definition.base.provider_instance_id,
                        generation: definition.base.provider_generation,
                        resource_generation: definition.resource_generation,
                    },
                    file,
                    maximum_source_bytes: bounds().request_bytes as usize,
                    capture_budget: ImmediateSourcePreparationBudget {
                        preparation_bytes: 512 * 1024,
                        retained_bytes: 16 * 1024,
                    },
                    snapshot: &input.request.snapshot,
                    entry: &input.request.entry,
                    context: input.request.context,
                    control: &control,
                },
            )
        };
        let error = result
            .err()
            .expect("no Host may escape failed installation");
        assert!(error.contains(if stopped {
            "preparation cancelled"
        } else {
            "independent current provider and grant"
        }));
        assert_eq!(probe.stream_position().unwrap(), 0);
    }
}

#[test]
fn selected_source_snapshot_drift_refuses_before_compiler_preparation() {
    for changed in 0..3 {
        let checked = configuration();
        let image = image(&checked);
        let (config, definition) = definition(&checked, &image);
        let original = include_str!("../../../../plots/hello/main.conduit");
        let (input, _) = input(original);
        let mut snapshot = input.request.snapshot.clone();
        let file = if changed == 0 {
            let replacement = original.replace("hello", "jello");
            assert_ne!(replacement, original);
            assert_eq!(replacement.len(), original.len());
            source_file(&replacement)
        } else {
            if changed == 1 {
                snapshot.sha256 = "0".repeat(64);
            } else {
                snapshot.source_document_id = "different/reviewed-source".into();
            }
            source_file(original)
        };
        let catalog = conduit_workspace_make::catalog();
        let control = RunControl::default();
        // SAFETY: This fixture owns the descriptor and original scoped grant.
        // Contrary snapshot evidence must refuse, never authorize new bytes.
        let result = unsafe {
            conduit::source_check_worker::initialize_image(
                conduit::source_check_worker::SourceCheckImageInstallation {
                    config,
                    image: &image,
                    checked: &checked,
                    catalog: &catalog,
                    grant: definition.grant,
                    provider: conduit::source_check_worker::SourceCheckProviderIdentity {
                        instance: definition.base.provider_instance_id,
                        generation: definition.base.provider_generation,
                        resource_generation: definition.resource_generation,
                    },
                    file,
                    maximum_source_bytes: bounds().request_bytes as usize,
                    capture_budget: ImmediateSourcePreparationBudget {
                        preparation_bytes: 512 * 1024,
                        retained_bytes: 16 * 1024,
                    },
                    snapshot: &snapshot,
                    entry: &input.request.entry,
                    context: input.request.context,
                    control: &control,
                },
            )
        };
        assert!(result
            .err()
            .expect("snapshot drift must not publish an initialized Host")
            .contains("selected Source changed since owner review"));
    }
}

#[test]
fn metadata_and_provider_preparation_compete_in_one_image_heap() {
    let checked = configuration();
    let image = image(&checked);
    let (config, definition) = definition(&checked, &image);
    let provider_ran = Rc::new(Cell::new(false));
    let probe = provider_ran.clone();
    let provider_budget = budget();
    // SAFETY: fixed fixture metadata is much smaller than this conservative
    // envelope. Its retained charge plus provider preparation exceeds the one
    // IMAGE heap, so native preparation must never be invoked.
    let result = unsafe {
        StdHost::from_selected_timer_and_source_factory_image(
            config,
            &image,
            &conduit_workspace_make::catalog(),
            &checked,
            ImmediateSourcePreparationBudget {
                preparation_bytes: 50 * 1024 * 1024,
                retained_bytes: 50 * 1024 * 1024,
            },
            None,
            None,
            move |_, _root| {
                Ok((
                    definition,
                    provider_budget,
                    move || -> Result<Box<dyn ImmediateNativeSource>, String> {
                        probe.set(true);
                        Err("unadmitted provider must not run".into())
                    },
                ))
            },
        )
    };
    assert!(!provider_ran.get());
    assert!(result.err().unwrap().contains("backing capacity"));
}

#[test]
fn retained_root_preparation_refuses_native_start_without_losing_its_value() {
    let checked = configuration();
    let image = image(&checked);
    let (config, definition) = definition(&checked, &image);
    let provider_ran = Rc::new(Cell::new(false));
    let probe = provider_ran.clone();
    let provider_budget = budget();
    let mut retained = None;
    // SAFETY: fixed fixture metadata and immutable bytes fit these conservative
    // envelopes. Their independent retained charges must deny provider start.
    let result = unsafe {
        StdHost::from_selected_timer_and_source_factory_image(
            config,
            &image,
            &conduit_workspace_make::catalog(),
            &checked,
            ImmediateSourcePreparationBudget {
                preparation_bytes: 2 * 1024 * 1024,
                retained_bytes: 2 * 1024 * 1024,
            },
            None,
            None,
            |_, root| {
                retained = Some(
                    root.prepare_value(48 * 1024 * 1024, 48 * 1024 * 1024, || Ok(vec![7u8; 64]))?,
                );
                Ok((
                    definition,
                    provider_budget,
                    move || -> Result<Box<dyn ImmediateNativeSource>, String> {
                        probe.set(true);
                        Err("unadmitted provider must not run".into())
                    },
                ))
            },
        )
    };
    assert!(!provider_ran.get());
    assert!(result.err().unwrap().contains("backing capacity"));
    assert_eq!(retained.as_ref().unwrap().value().as_slice(), &[7u8; 64]);
    drop(retained);
}

#[test]
fn source_capture_refuses_unfunded_io_or_retention_before_starting_child() {
    use std::io::{Seek, SeekFrom};
    for mode in 0..3 {
        let pressure = mode != 1;
        let checked = configuration();
        let image = image(&checked);
        let (config, definition) = definition(&checked, &image);
        let (mut input, _) = input(include_str!("../../../../plots/hello/main.conduit"));
        if !pressure {
            input.request.context.catalog.reserve(65_536);
        }
        let mut file = source_file(&input.source);
        file.seek(SeekFrom::Start(3)).unwrap();
        let mut alias = file.try_clone().unwrap();
        let children = std::fs::read_to_string("/proc/thread-self/children").unwrap();
        let catalog = conduit_workspace_make::catalog();
        // SAFETY: independent fixture grant/descriptor. Pressure denies capture;
        // otherwise 512 KiB covers the finite capture and 65 KiB moved context,
        // whose insufficient retention must refuse before provider preparation.
        let result = unsafe {
            conduit::source_check_worker::initialize_image_with_caller_workspace(
                conduit::source_check_worker::SourceCheckImageInstallation {
                    config,
                    image: &image,
                    checked: &checked,
                    catalog: &catalog,
                    grant: definition.grant,
                    provider: conduit::source_check_worker::SourceCheckProviderIdentity {
                        instance: definition.base.provider_instance_id,
                        generation: definition.base.provider_generation,
                        resource_generation: definition.resource_generation,
                    },
                    file,
                    maximum_source_bytes: bounds().request_bytes as usize,
                    capture_budget: ImmediateSourcePreparationBudget {
                        preparation_bytes: if pressure {
                            image.manifest.bounds.heap_arena_bytes as u32
                        } else {
                            512 * 1024
                        },
                        retained_bytes: 16 * 1024,
                    },
                    snapshot: &input.request.snapshot,
                    entry: &input.request.entry,
                    context: input.request.context,
                    control: &RunControl::default(),
                },
                None,
                (mode == 2).then_some(image.manifest.bounds.heap_arena_bytes as usize),
                None,
            )
        };
        let expected = if pressure {
            "backing capacity"
        } else {
            "qualified retained backing"
        };
        assert!(result.err().unwrap().contains(expected));
        assert_eq!(
            alias.stream_position().unwrap(),
            if pressure {
                3
            } else {
                input.source.len() as u64
            }
        );
        assert_eq!(
            std::fs::read_to_string("/proc/thread-self/children").unwrap(),
            children
        );
    }
}

#[test]
fn local_selected_file_revision_commits_exact_workset_and_rejects_replay() {
    use conduit_body::{
        AuthenticatedHostObservation, Body, BodyBiographyEvidence, BodyMembership,
        MembershipProofId, PartId,
    };
    use sha2::{Digest, Sha256};
    let root = std::env::temp_dir().join(format!(
        "conduit-selected-file-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(root.join("body")).unwrap();
    let checked = configuration();
    let source = canonical_host_configuration_conduit(checked.configuration())
        .unwrap()
        .replace("603979776", "1073741824");
    let checked = check_host_configuration(
        parse_host_configuration_conduit(&source).unwrap(),
        &conduit_workspace_make::catalog(),
        &conduit_workspace_make::package_set(),
    )
    .unwrap();
    let image = image(&checked);
    let host_id = HostId::from("host/local-selected-file-test");
    let body = Body::born(
        "source/before-file-policy".into(),
        "checked/before-file-policy".into(),
        1,
        SignId::from("sign/file-policy/born"),
    )
    .unwrap();
    let mut membership = BodyMembership::new(body.body_id.clone()).unwrap();
    let mut biography = BodyBiographyEvidence::born(
        body.clone(),
        membership.clone(),
        "Selected File policy test".into(),
    )
    .unwrap();
    let part = PartId::bind(&body.body_id, host_id.as_str(), 0).unwrap();
    let proof = MembershipProofId::bind("conduit/test/local-source-policy").unwrap();
    let admitted = membership
        .admit(
            &body.body_id,
            membership.revision,
            part.clone(),
            proof.clone(),
            "sign/file-policy/admit".into(),
        )
        .unwrap();
    let present = membership
        .observe_present(
            &body.body_id,
            membership.revision,
            &part,
            AuthenticatedHostObservation {
                host_id: host_id.clone(),
                boot_id: "boot/before-file-policy".into(),
                offer_generation: OfferGeneration(1),
                proof_id: proof,
                sequence: 0,
            },
            "sign/file-policy/present".into(),
        )
        .unwrap();
    biography
        .append_membership_events(membership, &[(admitted, 2), (present, 3)])
        .unwrap();
    let biography_path = root.join("body/biography.json");
    let biography_bytes = serde_json::to_vec_pretty(&biography).unwrap();
    std::fs::write(&biography_path, &biography_bytes).unwrap();
    let executable = root.join("reviewed-installed-image");
    std::fs::write(&executable, b"reviewed finite fixture image").unwrap();
    let release_digest = format!(
        "sha256:{:x}",
        Sha256::digest(std::fs::read(&executable).unwrap())
    );
    let installed = serde_json::json!({
        "schema": "conduit.install/durable-host@1", "host_id": host_id.as_str(),
        "release_source_identity": image.manifest.source_identity,
        "release_bundle_sha256": release_digest, "product_executable": executable,
        "body_state": {"body_id": body.body_id, "biography_path": biography_path, "biography_sha256": format!("sha256:{:x}", Sha256::digest(&biography_bytes))},
        "selected_host_configuration": {"source": source, "configuration_id": checked.configuration_id(), "host_image": image},
    });
    std::fs::write(
        root.join("installation.json"),
        serde_json::to_vec_pretty(&installed).unwrap(),
    )
    .unwrap();
    let selected_source =
        "plot selected-clock (\n) {\n clock: time/tick(count = 1, period-ms = 0)\n}\n";
    let file = root.join("selected.conduit");
    std::fs::write(&file, selected_source).unwrap();
    let policy = serde_json::json!({
        "host_id": host_id, "configuration_id": checked.configuration_id(), "release_bundle_sha256": release_digest,
        "expected": biography, "file": file,
        "snapshot": {"owner": body.body_id, "generation": 1, "sha256": format!("{:x}", Sha256::digest(selected_source.as_bytes())), "source_document_id": canonical_source_document_id(selected_source)},
        "entries": ["selected-clock"], "maximum_source_bytes": 16384,
        "capture_preparation_bytes": 524288, "capture_retained_bytes": 16384,
        "caller_workspace_bytes": 134217728,
    });
    let policy_file = root.join("policy.json");
    std::fs::write(&policy_file, serde_json::to_vec_pretty(&policy).unwrap()).unwrap();
    let command = || {
        let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_conduit"));
        command
            .arg("host")
            .arg("service")
            .arg("adopt-source")
            .arg(&policy_file)
            .arg("--state-dir")
            .arg(&root);
        command
    };
    #[cfg(feature = "root-allocation-observation")]
    {
        let prepared = command().arg("--prepare-only").output().unwrap();
        assert!(
            prepared.status.success(),
            "{}",
            String::from_utf8_lossy(&prepared.stderr)
        );
        let report: serde_json::Value = serde_json::from_slice(
            &std::fs::read(root.join("source-policy-preparation.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(report["source_committed"], false);
        assert_eq!(report["owner_published"], false);
        assert_eq!(report["rust_requested_peak_within_caller_envelope"], true);
        assert!(
            report["observation"]["peak_requested_bytes"]
                .as_u64()
                .unwrap()
                > 0
        );
        assert_eq!(std::fs::read(&biography_path).unwrap(), biography_bytes);
        assert!(!root.join("runtime.json").exists());
        assert!(!root.join("control.sock").exists());
        assert!(!root.join("source-policy-owner-peer.json").exists());
        assert!(!root.join("control.token").exists());
    }
    let mut service = command()
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    while !root.join("control.sock").exists() {
        if let Some(status) = service.try_wait().unwrap() {
            let output = service.wait_with_output().unwrap();
            panic!(
                "selected Source service ended {status}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        if std::time::Instant::now() >= deadline {
            service.kill().unwrap();
            let output = service.wait_with_output().unwrap();
            panic!(
                "selected Source service startup deadline: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    if let Some(status) = service.try_wait().unwrap() {
        let output = service.wait_with_output().unwrap();
        panic!(
            "selected Source service exited after binding {status}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    assert!(root.join("runtime.json").exists());
    #[cfg(feature = "root-allocation-observation")]
    {
        let observed: serde_json::Value =
            serde_json::from_slice(&std::fs::read(root.join("source-policy-memory.json")).unwrap())
                .unwrap();
        assert_eq!(
            observed["schema"],
            "conduit.diagnostic/root-preparation-allocations@1"
        );
        assert!(
            observed["observation"]["peak_requested_bytes"]
                .as_u64()
                .unwrap()
                > 0
        );
    }
    service.kill().unwrap();
    service.wait().unwrap();
    let current: BodyBiographyEvidence =
        serde_json::from_slice(&std::fs::read(&biography_path).unwrap()).unwrap();
    assert_eq!(current.body_id, body.body_id);
    assert_eq!(current.body.workset.plots().len(), 1);
    assert_eq!(
        current.body.workset.plots()[0].source_document_id,
        canonical_source_document_id(selected_source)
    );
    let replay = command().output().unwrap();
    assert!(!replay.status.success());
    assert!(
        String::from_utf8_lossy(&replay.stderr).contains("exact retained Body basis"),
        "{}",
        String::from_utf8_lossy(&replay.stderr)
    );
    assert_eq!(
        std::fs::read(&biography_path).unwrap(),
        serde_json::to_vec_pretty(&current).unwrap()
    );
    std::fs::remove_dir_all(root).unwrap();
}
