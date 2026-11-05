//! Bounded real compiler child -> prepared provider -> ordinary production kernel.
use super::*;
use conduit_core::*;
use conduit_planner::*;
use conduit_std_host::{source_check::*, *};
use std::collections::BTreeMap;
#[path = "../../../../make/body/tests/source_check_call/support/runtime.rs"]
mod support;
fn bounds() -> SourceCheckPreparationBounds {
    SourceCheckPreparationBounds {
        request_bytes: 16 * 1024,
        report_bytes: 64 * 1024,
        retained_bytes: 128 * 1024,
    }
}
struct Capture {
    bytes: Vec<u8>,
    calls: usize,
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
