//! Actual checked IMAGE construction, not reference Host reconfiguration.
use super::host_call::{bounds, support, Capture};
use super::*;
use conduit_core::*;
use conduit_host_make::*;
use conduit_std_host::{source_check::*, *};
use std::{cell::Cell, rc::Rc};

fn configuration() -> CheckedHostConfiguration {
    let source = parent::configuration_source().replace("      limits =", r#"
      base = {kind: "timer/monotonic", implementation: "hosted/monotonic-clock@1"}
      capability = {kind: "time/tick", pinned_implementation: "std/kernel-time-tick@2"}
      capability = {kind: "language/source-check", pinned_implementation: "std/bounded-source-check@1"}
      need = {id: "timer/main", class: "conduit.resource/timer-slot@1", slots: 1, bytes: 512}
      need = {id: "source/results", class: "language/prepared-source-check@1", slots: 1, bytes: 1048576}
      need = {id: "source/memory", class: "conduit.resource/runtime-memory@1", slots: 570425344, bytes: 570425344}
      limits ="#);
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
fn definition(image: &HostImage, kind: Kind) -> (StdHostConfig, NativeSourceDefinition) {
    let reference = StdHost::new();
    let ad = reference.advertisement();
    let config = StdHostConfig {
        host_id: ad.host_id.clone(),
        boot_id: ad.boot_id.clone(),
        offer_generation: ad.offer_generation,
    };
    let mut definition = support::definition_for(ad, kind, bounds(), 1048576);
    let selected = image
        .manifest
        .base_selections
        .iter()
        .find(|b| b.kind == "language/source-compiler")
        .unwrap();
    definition.base.base_id = selected.id.as_str().into();
    definition.base.mechanism_family = selected.kind.as_str().into();
    definition.base.implementation_id = selected.driver.as_str().into();
    definition.base.capabilities[0]
        .implementation
        .implementation_id = "std/bounded-source-check@1".into();
    definition.base.resources[0].pool_id = "source/results".into();
    definition.base.resources.push(ResourceOffer {
        pool_id: "source/memory".into(),
        class_id: RUNTIME_MEMORY_RESOURCE_CLASS.into(),
        capacity_units: 570425344,
        compute: None,
        content: None,
    });
    (config, definition)
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
    let schema = SourceCheckNativeSchema::prepare().unwrap();
    let kind = PreparedSourceChecker::kind_for_bounds(&schema, bounds()).unwrap();
    for mismatch in [false, true] {
        let (config, mut definition) = definition(&image, kind.clone());
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
fn run_image_case(source: &str) {
    let checked = configuration();
    let image = image(&checked);
    let schema = SourceCheckNativeSchema::prepare().unwrap();
    let kind = PreparedSourceChecker::kind_for_bounds(&schema, bounds()).unwrap();
    let (config, definition) = definition(&image, kind.clone());
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
    let mut worker = conduit::source_check_worker::from_configuration(&checked).unwrap();
    let selected_file = source_file(source);
    // SAFETY: Fixture owns this exact Source snapshot and explicit Host/Boot
    // grant; the actual bounded child is reaped before immutable provider return.
    // Caller-owned schema/Root metadata has separate preparation custody.
    let mut host = unsafe {
        StdHost::from_selected_timer_and_immediate_source_image(
            config,
            &image,
            &conduit_workspace_make::catalog(),
            &checked,
            definition,
            budget(),
            move || {
                let selected =
                    conduit_std_host::source_check::process::OwnedSourceCheckInput::from_file(
                        selected_file,
                        bounds().request_bytes as usize,
                        &input.owner,
                        input.generation,
                        &input.request.entry,
                        input.request.context.clone(),
                    )
                    .map_err(|e| format!("selected Source capture: {e:?}"))?;
                if selected.request() != &input.request {
                    return Err(
                        "selected Source capture differs from its retained reference".into(),
                    );
                }
                let output = worker
                    .prepare_selected_source(&selected, &RunControl::default())
                    .map_err(|e| format!("{e:?}"))?;
                if output.canonical_bytes() != expected {
                    return Err("actual compiler differs from direct canonical reference".into());
                }
                let prepared = PreparedSourceChecker::from_process_outputs(
                    &schema,
                    &[output],
                    bounds(),
                    &RunControl::default(),
                )?;
                Ok(Box::new(prepared.into_provider()))
            },
        )
    }
    .unwrap();
    assert_eq!(
        host.image_identity().unwrap().image_id,
        image.manifest.image_id
    );
    assert_eq!(host.advertisement().capabilities.len(), 2);
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
    let inputs = [ExternalForeInput {
        front_port_id: "request".into(),
        track: ConnectionTrack::Payload,
        bytes: request,
    }];
    let mut capture = BoundedBodyCapture {
        bytes: Vec::with_capacity(expected_body_reply.len()),
        calls: 0,
    };
    let mut probe = None;
    let result = host.run_selected_image_body_plan_with_fore_to_with_start(
        conduit_std_host::body_execution::BodyRunRequest {
            wake: &wake,
            plan: &body_plan,
            control: &RunControl::default(),
            keyboard: None,
        },
        &inputs,
        &mut Vec::new(),
        &mut ThreadTimer,
        Some(&mut capture),
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
    let cancelled = host.run_selected_image_body_plan_with_fore_to_with_start(
        conduit_std_host::body_execution::BodyRunRequest {
            wake: &wake,
            plan: &body_plan,
            control: &control,
            keyboard: None,
        },
        &inputs,
        &mut Vec::new(),
        &mut ThreadTimer,
        Some(&mut capture),
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
}

#[test]
fn failed_initializer_returns_no_image_host_or_ready_provider() {
    let checked = configuration();
    let image = image(&checked);
    let schema = SourceCheckNativeSchema::prepare().unwrap();
    let kind = PreparedSourceChecker::kind_for_bounds(&schema, bounds()).unwrap();
    let (config, definition) = definition(&image, kind);
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
    let schema = SourceCheckNativeSchema::prepare().unwrap();
    let kind = PreparedSourceChecker::kind_for_bounds(&schema, bounds()).unwrap();
    let (config, definition) = definition(&image, kind);
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
        .contains("managed Base backing capacity"));
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
