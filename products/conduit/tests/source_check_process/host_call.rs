//! Bounded real compiler child -> prepared provider -> ordinary production kernel.
use super::*;
use conduit_core::*;
use conduit_planner::*;
use conduit_std_host::{source_check::*, *};
use std::collections::BTreeMap;
#[path = "../../../../make/body/tests/source_check_call/support/runtime.rs"]
pub(super) mod support;
pub(super) fn bounds() -> SourceCheckPreparationBounds {
    SourceCheckPreparationBounds {
        request_bytes: 16 * 1024,
        report_bytes: 64 * 1024,
        retained_bytes: 128 * 1024,
    }
}
pub(super) struct Capture {
    pub(super) bytes: Vec<u8>,
    pub(super) calls: usize,
}
impl ExternalForeOutputAdapter for Capture {
    fn deliver(&mut self, _: ExternalForeDelivery) -> Result<(), String> {
        panic!("requires commit")
    }
    fn deliver_committed(
        &mut self,
        witness: CommittedForeValue<'_>,
        delivery: ExternalForeDelivery,
    ) -> Result<(), String> {
        assert_eq!(delivery.front_port_id.as_str(), "reply");
        self.bytes.extend_from_slice(witness.bytes());
        self.calls += 1;
        Ok(())
    }
}
#[test]
fn bounded_actual_worker_reports_flow_through_the_same_admitted_host_call() {
    let schema = SourceCheckNativeSchema::prepare().unwrap();
    let mut outputs = Vec::new();
    let mut requests = Vec::new();
    let mut expected = Vec::new();
    let mut worker = parent::worker(std::time::Duration::from_secs(20), 16 * 1024);
    for (source, generation) in [
        (include_str!("../../../../plots/hello/main.conduit"), 7),
        ("# α 🦀\nplot hello { first: unavailable/kind }\n", 8),
    ] {
        let (input, mut reference) = input_with_generation(source, generation);
        let report = reference.project(&input.request).unwrap();
        assert_eq!(
            report.check,
            if generation == 7 {
                CheckOutcome::Accepted
            } else {
                CheckOutcome::Rejected
            }
        );
        expected.push(
            schema
                .encode_reply(
                    SourceCheckReply::Checked(&report),
                    bounds().report_bytes as usize,
                )
                .unwrap(),
        );
        requests.push(
            schema
                .encode_request(&input.request, bounds().request_bytes as usize)
                .unwrap(),
        );
        outputs.push(worker.prepare(&input, &RunControl::default()).unwrap());
    }
    let prepared = PreparedSourceChecker::from_process_outputs(
        &schema,
        &outputs,
        bounds(),
        &RunControl::default(),
    )
    .unwrap();
    drop(outputs); // The actual children and their packets are retired before any Play.
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    prepared
        .install_authoring(&mut startup, &mut profiles)
        .unwrap();
    let checked = check_syntax_document(
        &parse_syntax_document(include_str!(
            "../../../../make/body/tests/source_check_call/check.conduit"
        )),
        &startup,
    )
    .unwrap();
    let authored =
        expand_canonical_plot_for_authoring(&checked, "check-source", &profiles).unwrap();
    let mut host = StdHost::new();
    assert!(support::plan(&host, &authored, &[], bounds()).is_err());
    let definition = support::definition(host.advertisement(), &prepared);
    let grant = definition.grant.clone();
    // SAFETY: Actual finite prepared provider, independent current Base and grant;
    // the compiler child is reaped and cannot be invoked by this immediate read.
    unsafe { host.install_immediate_native_source(definition, Box::new(prepared.into_provider())) }
        .unwrap_or_else(|(_, _, error)| panic!("{error}"));
    assert!(support::plan(&host, &authored, &[], bounds()).is_err());
    let plan = support::plan(&host, &authored, &[grant], bounds()).unwrap();
    for (request, expected) in requests.into_iter().zip(expected) {
        let mut capture = Capture {
            bytes: vec![],
            calls: 0,
        };
        let result = host
            .run_external_plot_to(
                plan.fragments[0].clone(),
                &[ExternalForeInput {
                    front_port_id: "request".into(),
                    track: ConnectionTrack::Payload,
                    bytes: request,
                }],
                &mut capture,
                &mut Vec::new(),
                &mut ThreadTimer,
            )
            .unwrap();
        assert_eq!(capture.calls, 1);
        assert_eq!(capture.bytes, expected);
        let kernel = result.kernel.unwrap();
        for kind in [
            conduit_kernel::KernelEventKind::HostCallRequested,
            conduit_kernel::KernelEventKind::HostCallCompleted,
        ] {
            assert!(kernel.kernel_sign.iter().any(|event| event.kind == kind));
        }
        assert_eq!(
            kernel.value_allocation_capacity_before,
            kernel.value_allocation_capacity_after
        );
    }
}

#[test]
fn refused_child_result_does_not_establish_a_prepared_source_scope() {
    let schema = SourceCheckNativeSchema::prepare().unwrap();
    let (mut input, _) =
        input_with_generation(include_str!("../../../../plots/hello/main.conduit"), 7);
    input.request.snapshot.generation += 1;
    let output = parent::worker(std::time::Duration::from_secs(20), 16 * 1024)
        .prepare(&input, &RunControl::default())
        .unwrap();
    schema
        .admit_reply(output.canonical_bytes(), bounds().report_bytes as usize)
        .unwrap();
    let error = PreparedSourceChecker::from_process_outputs(
        &schema,
        &[output],
        bounds(),
        &RunControl::default(),
    )
    .err()
    .expect("refused Source cannot establish a runtime scope");
    assert!(
        error.contains("did not establish a prepared Source context"),
        "{error}"
    );
}

fn memory_definition(
    host: &StdHost,
    kind: &Kind,
    capacity: u32,
    retained: u32,
) -> NativeSourceDefinition {
    let mut definition = support::definition_for(
        host.advertisement(),
        kind.clone(),
        bounds(),
        retained as u64,
    );
    definition.base.resources.push(ResourceOffer {
        pool_id: "fixture/source-preparation-memory".into(),
        class_id: RUNTIME_MEMORY_RESOURCE_CLASS.into(),
        capacity_units: capacity,
        compute: None,
        content: None,
    });
    definition
}
#[test]
fn actual_source_preparation_is_paid_before_factory_and_retained_through_play() {
    use std::cell::Cell;
    let schema = SourceCheckNativeSchema::prepare().unwrap();
    let kind = PreparedSourceChecker::kind_for_bounds(&schema, bounds()).unwrap();
    let mut host = StdHost::new();
    let before = host.advertisement().clone();
    let preparation_bytes = 544 * 1024 * 1024;
    let retained_bytes = 1024 * 1024;
    let budget = ImmediateSourcePreparationBudget {
        preparation_bytes,
        retained_bytes,
    };
    let ran = Cell::new(false);
    // SAFETY: The refused definition has insufficient independently configured
    // backing; this factory must never be invoked or create a native owner.
    let small = unsafe {
        host.prepare_and_install_immediate_native_source(
            memory_definition(&host, &kind, preparation_bytes - 1, retained_bytes),
            budget,
            || {
                ran.set(true);
                Err("must not run".into())
            },
        )
    };
    assert!(small.is_err());
    assert!(!ran.get());
    assert_eq!(host.advertisement(), &before);
    // SAFETY: A failed initializer returns with no work or owner retained.
    let failed = unsafe {
        host.prepare_and_install_immediate_native_source(
            memory_definition(&host, &kind, preparation_bytes, retained_bytes),
            budget,
            || Err("initialization refused with no live owner".into()),
        )
    };
    assert!(failed.is_err());
    assert_eq!(host.advertisement(), &before);

    let (input, mut reference) =
        input_with_generation(include_str!("../../../../plots/hello/main.conduit"), 7);
    let expected = schema
        .encode_reply(
            SourceCheckReply::Checked(&reference.project(&input.request).unwrap()),
            bounds().report_bytes as usize,
        )
        .unwrap();
    let request = schema
        .encode_request(&input.request, bounds().request_bytes as usize)
        .unwrap();
    let definition = memory_definition(&host, &kind, preparation_bytes, retained_bytes);
    let base = definition.base.provider_instance_id.clone();
    let grant = definition.grant.clone();
    // SAFETY: Actual bounded child is synchronously reaped by the shared driver;
    // the returned provider owns only finite immutable packets and has a plain
    // finite Vec/Box retirement. Root metadata/schema has separate caller custody.
    unsafe {
        host.prepare_and_install_immediate_native_source(definition, budget, || {
            let output = parent::worker(std::time::Duration::from_secs(20), 16 * 1024)
                .prepare(&input, &RunControl::default())
                .map_err(|e| format!("{e:?}"))?;
            let prepared = PreparedSourceChecker::from_process_outputs(
                &schema,
                &[output],
                bounds(),
                &RunControl::default(),
            )?;
            Ok(Box::new(prepared.into_provider()))
        })
    }
    .unwrap_or_else(|(_, error)| panic!("{error}"));
    let binding = |units| ResourceBinding {
        pool_id: "fixture/source-preparation-memory".into(),
        class_id: RUNTIME_MEMORY_RESOURCE_CLASS.into(),
        units,
        protected: None,
        compute: None,
        content: None,
    };
    assert!(host
        .reserve_provider_resources(&base, 1, &[binding(preparation_bytes)])
        .is_err());
    let spare = host
        .reserve_provider_resources(&base, 1, &[binding(preparation_bytes - retained_bytes)])
        .unwrap();
    // SAFETY: No child Play is active, but outstanding custody must still refuse.
    assert!(unsafe { host.retire_immediate_native_source() }.is_err());
    // SAFETY: This test reservation created no effects or worker.
    unsafe { host.release_provider_resources(spare) }
        .unwrap_or_else(|(_, error)| panic!("{error}"));

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
    let checked = check_syntax_document(
        &parse_syntax_document(include_str!(
            "../../../../make/body/tests/source_check_call/check.conduit"
        )),
        &startup,
    )
    .unwrap();
    let authored =
        expand_canonical_plot_for_authoring(&checked, "check-source", &profiles).unwrap();
    let plan = support::plan(&host, &authored, &[grant], bounds()).unwrap();
    let mut capture = Capture {
        bytes: vec![],
        calls: 0,
    };
    host.run_external_plot_to(
        plan.fragments[0].clone(),
        &[ExternalForeInput {
            front_port_id: "request".into(),
            track: ConnectionTrack::Payload,
            bytes: request,
        }],
        &mut capture,
        &mut Vec::new(),
        &mut ThreadTimer,
    )
    .unwrap();
    assert_eq!(capture.bytes, expected);
    // The persistent charge survives the completed child Play.
    assert!(host
        .reserve_provider_resources(&base, 1, &[binding(preparation_bytes)])
        .is_err());
    // SAFETY: The concrete immutable provider has no background work or custom Drop.
    unsafe { host.retire_immediate_native_source() }.unwrap();
    assert!(support::plan(&host, &authored, &[], bounds()).is_err());
    assert!(host
        .reserve_provider_resources(&base, 1, &[binding(1)])
        .is_err());
}
