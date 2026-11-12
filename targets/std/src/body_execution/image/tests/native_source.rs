//! CPU ledger proof; compilation/preparation is outside the factory fixture.
//! The product process test separately qualifies the actual bounded compiler.
use super::*;
use crate::source_check::*;
use crate::*;
use conduit_core::ImplementationOffer;
use conduit_core::*;
use conduit_plot::*;
#[path = "../../../../../../make/body/tests/source_check_call/support.rs"]
mod support;
fn bounds() -> SourceCheckPreparationBounds {
    SourceCheckPreparationBounds {
        request_bytes: 16384,
        report_bytes: 65536,
        retained_bytes: 131072,
    }
}
fn fixture(image_backed: bool) -> (StdHost, Wake, BodyPlan, Vec<ExternalForeInput>) {
    let (mut owner, request) = support::owner(
        include_str!("../../../../../../plots/hello/main.conduit"),
        7,
    );
    let schema = SourceCheckNativeSchema::prepare().unwrap();
    let input = schema.encode_request(&request, 16384).unwrap();
    let prepared = PreparedSourceChecker::prepare(
        &mut [SourceCheckPreparation {
            owner: &mut owner,
            request: &request,
        }],
        bounds(),
    )
    .unwrap();
    let source = r#"host compiler {
      schema = 1
      target = {architecture: "x86_64", machine: "computer", os: "linux"}
      base = {kind: "language/source-compiler", implementation: "std/bounded-source-check@1", initialization: {executable: "/unused-fixture", executable_sha256: "0000000000000000000000000000000000000000000000000000000000000000", maximum_executable_bytes: 536870912, address_space_bytes: 536870912, cpu_seconds: 10, deadline_milliseconds: 20000, maximum_startup_bytes: 16384, preparation_bytes: 570425344, retained_bytes: 1048576, request_bytes: 16384, reply_bytes: 65536}}
      base = {kind: "timer/monotonic", implementation: "hosted/monotonic-clock@1"}
      capability = {kind: "time/tick", pinned_implementation: "std/kernel-time-tick@2"}
      capability = {kind: "language/source-check", pinned_implementation: "std/bounded-source-check@1"}
      need = {id: "timer/main", class: "conduit.resource/timer-slot@1", slots: 1, bytes: 512}
      need = {id: "source/results", class: "language/prepared-source-check@1", slots: 1, bytes: 1048576}
      need = {id: "source/memory", class: "conduit.resource/runtime-memory@1", slots: 570425344, bytes: 570425344}
      limits = {static_memory_bytes: 16777216, heap_arena_bytes: 603979776, queue_items: 32, buffered_bytes: 1048576, active_instances: 16, operation_slots: 16, timer_slots: 16, line_sessions: 16, evidence_items: 16}
    }"#;
    let catalog = conduit_workspace_make::catalog();
    let checked = check_host_configuration(
        parse_host_configuration_conduit(source).unwrap(),
        &catalog,
        &conduit_workspace_make::package_set(),
    )
    .unwrap();
    let image = build_default_host_image(
        checked.profile().clone(),
        &catalog,
        &conduit_workspace_make::package_set(),
        &BuildInputs {
            source_identity: "fixture/source-receipt".into(),
            toolchain_available: true,
        },
    )
    .unwrap()
    .0;
    let reference = StdHost::new();
    let ad = reference.advertisement();
    let config = StdHostConfig {
        host_id: ad.host_id.clone(),
        boot_id: ad.boot_id.clone(),
        offer_generation: ad.offer_generation,
    };
    let mut definition = support::definition(ad, &prepared);
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
    let grant = definition.grant.clone();
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    prepared
        .install_authoring(&mut startup, &mut profiles)
        .unwrap();
    // SAFETY: Explicit fixture grant and exact finite precomputed owner. The
    // actual compiler is not run here or inferred from the dummy executable.
    let host = if image_backed {
        unsafe {
            StdHost::from_selected_timer_and_immediate_source_image(
                config,
                &image,
                &catalog,
                &checked,
                definition,
                ImmediateSourcePreparationBudget {
                    preparation_bytes: 570425344,
                    retained_bytes: 1048576,
                },
                move || Ok(Box::new(prepared.into_provider())),
            )
        }
        .unwrap()
    } else {
        let mut host = reference;
        definition.base.resources.truncate(1);
        // SAFETY: Same finite fixture owner/grant, deliberately installed via
        // the reference entrance without any IMAGE backing reservation.
        unsafe {
            host.install_immediate_native_source(definition, Box::new(prepared.into_provider()))
        }
        .unwrap_or_else(|(_, _, e)| panic!("{e}"));
        // Internal adversarial fixture: descriptive bounds cannot manufacture
        // the independently reserved provider's actual memory custody.
        host.kernel_resources
            .bind_image_bounds(&image.manifest.bounds)
            .unwrap();
        host
    };
    let syntax = check_syntax_document(
        &parse_syntax_document(include_str!(
            "../../../../../../make/body/tests/source_check_call/check.conduit"
        )),
        &startup,
    )
    .unwrap();
    let authored = expand_canonical_plot_for_authoring(&syntax, "check-source", &profiles).unwrap();
    let plan = support::plan(&host, &authored, &[grant], bounds()).unwrap();
    let plot = ResidentPlot::new(
        plan.source_document_id.clone(),
        plan.checked_plot_id.clone(),
    );
    let body = Body::born_with_plots(
        conduit_body::BodyWorkset::from_plots([plot.clone()]).unwrap(),
        1,
        "fixture/born".into(),
    )
    .unwrap();
    let wake = body.wake(1, "fixture/wake".into()).unwrap().1;
    let plan = BodyPlan::seal(&wake, vec![BodyPlotPlan { plot, plan }]).unwrap();
    (
        host,
        wake,
        plan,
        vec![ExternalForeInput {
            front_port_id: "request".into(),
            track: ConnectionTrack::Payload,
            bytes: input,
        }],
    )
}
fn receipt(
    host: &mut StdHost,
    wake: &Wake,
    plan: &BodyPlan,
    inputs: &[ExternalForeInput],
) -> usize {
    let play = BodyPlayIdentity::bind(plan, host.next_kernel_play_sequence);
    let control = RunControl::default();
    let request = BodyRunRequest {
        wake,
        plan,
        control: &control,
        keyboard: None,
    };
    let (prepared, sign) = prepare_lifecycle(&request, &play, &host.advertisement).unwrap();
    let lifecycle =
        crate::installed_std::body_kernel::memory::lifecycle_bytes(&play, &prepared, &sign)
            .unwrap();
    let mut kernel = BodyKernel::prepare_on_host(
        &plan.plots.iter().map(|part| &part.plan).collect::<Vec<_>>(),
        false,
        &play.active_play_id,
        inputs,
        false,
        true,
        &host.advertisement,
        host.native_source.as_mut().map(|owner| {
            crate::installed_std::body_kernel::NativeBodySource {
                owner,
                registry: &host.base_registry,
                host: &host.advertisement,
                wake,
                plan,
                play: &play,
            }
        }),
        #[cfg(target_os = "linux")]
        None,
    )
    .unwrap();
    kernel
        .prepare_memory(
            &mut ThreadTimer,
            &host.advertisement.host_id,
            &host.advertisement.boot_id,
            lifecycle,
        )
        .unwrap()
        .total_bytes()
}
#[test]
fn one_byte_short_source_receipt_refuses_before_start_and_exact_bytes_retain_result_custody() {
    let (mut host, wake, plan, inputs) = fixture(true);
    let bytes = receipt(&mut host, &wake, &plan, &inputs);
    let observed = host.kernel_resources.image_memory_observer();
    let baseline = observed.load(Ordering::Acquire);
    println!(
        "Source Body receipt: {bytes} bytes; independent retained Base charge: {baseline} bytes"
    );
    let limit = host
        .kernel_resources
        .admitted_image_bounds()
        .unwrap()
        .heap_arena_bytes as usize;
    let extra = host
        .kernel_resources
        .reserve_managed_base_memory(limit - baseline - bytes + 1)
        .unwrap();
    let mut started = false;
    let refusal = host.run_selected_image_body_plan_with_fore_to_with_start(
        BodyRunRequest {
            wake: &wake,
            plan: &plan,
            control: &RunControl::default(),
            keyboard: None,
        },
        &inputs,
        &mut Vec::new(),
        &mut ThreadTimer,
        Some(&mut AcceptOutput),
        |_, _| {
            started = true;
            Ok(())
        },
    );
    assert!(refusal
        .err()
        .unwrap()
        .contains("managed heap backing capacity"));
    assert!(!started);
    assert_eq!(host.next_kernel_play_sequence, 0);
    drop(extra);
    assert_eq!(observed.load(Ordering::Acquire), baseline);
    let extra = host
        .kernel_resources
        .reserve_managed_base_memory(limit - baseline - bytes)
        .unwrap();
    let report = host
        .run_selected_image_body_plan_with_fore_to_with_start(
            BodyRunRequest {
                wake: &wake,
                plan: &plan,
                control: &RunControl::default(),
                keyboard: None,
            },
            &inputs,
            &mut Vec::new(),
            &mut ThreadTimer,
            Some(&mut AcceptOutput),
            |_, _| Ok(()),
        )
        .unwrap();
    assert_eq!(report.report().terminal, TerminalDisposition::Completed);
    assert_eq!(report.report().fore_deliveries.len(), 1);
    let escaped = report.report().fore_deliveries[0].bytes.clone();
    drop(extra);
    drop(report);
    assert!(observed.load(Ordering::Acquire) > baseline);
    assert!(!escaped.is_empty());
    drop(escaped);
    assert_eq!(observed.load(Ordering::Acquire), baseline);
    drop(host);
    assert_eq!(observed.load(Ordering::Acquire), 0);
}

#[test]
fn declared_image_bounds_cannot_pay_for_a_reference_native_owner() {
    let (mut host, wake, plan, inputs) = fixture(false);
    let mut started = false;
    let refusal = host.run_selected_image_body_plan_with_fore_to_with_start(
        BodyRunRequest {
            wake: &wake,
            plan: &plan,
            control: &RunControl::default(),
            keyboard: None,
        },
        &inputs,
        &mut Vec::new(),
        &mut ThreadTimer,
        Some(&mut AcceptOutput),
        |_, _| {
            started = true;
            Ok(())
        },
    );
    assert!(refusal
        .err()
        .unwrap()
        .contains("independently charged provider backing"));
    assert!(!started);
    assert_eq!(host.next_kernel_play_sequence, 0);
}
