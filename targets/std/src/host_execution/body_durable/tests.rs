//! CPU offer/Body fixtures around actual installed State, Fore, file and process
//! boundaries. No recorder, catalog, producer ACK or Body admission is supplied.
use super::*;
use conduit_core::*;
use conduit_planner::{
    default_expanded_placements, plan_expanded_authoring_with_options, ConnectionQueueLimits,
    ForeBoundaryKey, PlanningOptions,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
};
use std::{collections::BTreeMap, path::Path, time::Duration};

#[derive(Default)]
struct Collector(Vec<ExternalForeDelivery>, Option<RunControl>);
impl ExternalForeOutputAdapter for Collector {
    fn deliver(&mut self, value: ExternalForeDelivery) -> Result<(), String> {
        self.0.push(value);
        if self.0.len() == 1 {
            if let Some(control) = &self.1 {
                control
                    .request_stop(RunControlRequestId::new("fixture/read-observed")?)
                    .map_err(|error| format!("readonly stop refused: {error:?}"))?;
            }
        }
        Ok(())
    }
}
#[derive(Default)]
struct Timer {
    started: bool,
}
impl TimerAdapter for Timer {
    fn wait(&mut self, _: Duration) {}
    fn observe_kernel_phase(&mut self, phase: crate::KernelTimingPhase) {
        if matches!(phase, crate::KernelTimingPhase::PlayStarted) {
            self.started = true;
        }
    }
}
fn fixture(write: bool, boot: &str) -> (StdHost, PlanFragment) {
    let ty = StructuredInfoType::leaf(kind_id(BOOL_INFO_ID)).unwrap();
    let initial = StructuredInfoValue::leaf(ty.clone(), InfoBool::FALSE.encode().to_vec()).unwrap();
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profile = conduit_plot::ProfileCatalog::new();
    conduit_semantic_catalog::state_value::install_state_value_kind(
        "Boolean",
        &ty,
        &initial,
        &mut startup,
        &mut profile,
    )
    .unwrap();
    startup
        .insert(conduit_plot::KindSignature {
            kind: "state/latest".into(),
            startup_parameters: vec![],
        })
        .unwrap();
    let input = if write { ">> write: Boolean\n" } else { "" };
    let cord = if write { "write >> cell\n" } else { "" };
    let source = format!("plot durable-cell (\n{input}current: $Boolean >>\n) {{\n cell: keep Boolean(false) for life\n{cord} cell >> current\n}}\n");
    let checked = check_syntax_document(&parse_syntax_document(&source), &startup).unwrap();
    let expanded = expand_canonical_plot_for_authoring(&checked, "durable-cell", &profile).unwrap();
    let mut host = StdHost::new_with_config(StdHostConfig {
        host_id: "host/durable-fore-fixture".into(),
        boot_id: boot.into(),
        offer_generation: OfferGeneration(1),
    });
    host.advertisement.capabilities.retain(|offer| {
        offer.kind_id.as_str() != conduit_semantic_catalog::state_value::STATE_VALUE_KIND
    });
    host.advertisement
        .capabilities
        .push(conduit_std_offers::state_value_durable_std_offer("Boolean", &ty, &initial).unwrap());
    host.advertisement
        .capabilities
        .sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
    host.advertisement.resources.push(resource_offer(
        "pool/durable-fore",
        conduit_std_offers::STATE_VALUE_DURABLE_RESOURCE_CLASS,
        1,
    ));
    host.advertisement
        .resources
        .sort_by(|a, b| a.pool_id.cmp(&b.pool_id));
    host.kernel_resources =
        crate::kernel_preparation::KernelResourceLedger::new(host.advertisement()).unwrap();
    let hosts = [host.advertisement().clone()];
    let placements = default_expanded_placements(&expanded.expanded, &hosts).unwrap();
    let mut boundaries = BTreeMap::new();
    for (port, direction) in [
        ("write", PortDirection::Input),
        ("current", PortDirection::Output),
    ] {
        if !write && port == "write" {
            continue;
        }
        boundaries.insert(
            ForeBoundaryKey {
                direction,
                front_port_id: port_id(port),
                track: ConnectionTrack::Payload,
            },
            ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: 1,
            },
        );
    }
    let empty = BTreeMap::new();
    let plan = plan_expanded_authoring_with_options(
        &expanded,
        &hosts,
        &placements,
        &["conduit.base/local@1".into()],
        PlanningOptions {
            connection_bases: &empty,
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 1,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )
    .unwrap();
    assert_eq!(plan.fragments.len(), 1);
    (host, plan.fragments[0].clone())
}
fn body(id: &str) -> conduit_body::BodyId {
    serde_json::from_value(serde_json::json!(id)).unwrap()
}
fn input() -> ExternalForeInput {
    ExternalForeInput {
        front_port_id: port_id("write"),
        track: ConnectionTrack::Payload,
        bytes: InfoBool::TRUE.encode().to_vec(),
    }
}
fn run(
    root: &Path,
    body: &conduit_body::BodyId,
    write: bool,
    boot: &str,
) -> Vec<ExternalForeDelivery> {
    let (mut host, fragment) = fixture(write, boot);
    let inputs = if write { vec![input()] } else { vec![] };
    let control = RunControl::default();
    let mut collector = Collector(Vec::new(), (!write).then(|| control.clone()));
    let report = host
        .run_body_durable_external_plot_controlled_to(
            body,
            root,
            fragment,
            &inputs,
            if write { 2 } else { 1 },
            &mut collector,
            &mut Vec::new(),
            &mut Timer::default(),
            &control,
        )
        .unwrap();
    assert!(report.kernel.is_some());
    assert_eq!(report.external_fore_deliveries, collector.0);
    assert!(!collector.0.is_empty());
    if !write {
        assert_eq!(
            collector.0.len(),
            1,
            "stop follows the first recovered output"
        );
        assert_eq!(report.control_receipts.len(), 1);
        assert_eq!(
            report.control_receipts[0].request_id.as_str(),
            "fixture/read-observed"
        );
        assert_eq!(
            report.control_receipts[0].disposition,
            crate::RunControlDisposition::Accepted
        );
        assert_eq!(
            report.control_receipts[0].active_play_id,
            report.kernel.as_ref().unwrap().active_play_id
        );
    }
    collector.0
}
fn last(values: &[ExternalForeDelivery]) -> &[u8] {
    values.last().unwrap().bytes.as_ref()
}

#[test]
fn fresh_process_state_write_and_new_boot_read_use_exact_body_identity() {
    const MODE: &str = "CONDUIT_DURABLE_FORE_TEST_MODE";
    const ROOT: &str = "CONDUIT_DURABLE_FORE_TEST_ROOT";
    if let Ok(mode) = std::env::var(MODE) {
        let root = std::path::PathBuf::from(std::env::var_os(ROOT).unwrap());
        let values = run(
            &root,
            &body("body/durable-fore"),
            mode == "write",
            &format!("boot/{mode}"),
        );
        assert_eq!(last(&values), InfoBool::TRUE.encode());
        return;
    }
    let root = tempfile::tempdir().unwrap();
    let (_, write) = fixture(true, "boot/identity-write");
    let (_, read) = fixture(false, "boot/identity-read");
    assert_eq!(write.states[0].state_id, read.states[0].state_id);
    assert_eq!(write.states[0].value_kind, read.states[0].value_kind);
    assert_eq!(write.states[0].initial_value, read.states[0].initial_value);
    assert_eq!(
        write.states[0].maximum_value_bytes,
        read.states[0].maximum_value_bytes
    );
    for mode in ["write", "read"] {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "host_execution::body_durable::tests::fresh_process_state_write_and_new_boot_read_use_exact_body_identity", "--nocapture"])
            .env(MODE, mode).env(ROOT, root.path()).status().unwrap();
        assert!(status.success(), "fresh {mode} process failed");
    }
    assert_eq!(
        last(&run(
            root.path(),
            &body("body/distinct"),
            false,
            "boot/distinct"
        )),
        InfoBool::FALSE.encode()
    );
    assert_eq!(
        last(&run(
            root.path(),
            &body("body/durable-fore"),
            false,
            "boot/read-again"
        )),
        InfoBool::TRUE.encode()
    );
}

#[test]
fn invalid_fore_refuses_before_start_and_precancel_preserves_committed_state() {
    let root = tempfile::tempdir().unwrap();
    let owner = body("body/durable-fore");
    run(root.path(), &owner, true, "boot/initial");
    for inputs in [
        vec![],
        vec![ExternalForeInput {
            front_port_id: port_id("foreign"),
            ..input()
        }],
        vec![ExternalForeInput {
            bytes: vec![0, 1],
            ..input()
        }],
        vec![input(), input()],
    ] {
        let (mut host, fragment) = fixture(true, "boot/refusal");
        let mut timer = Timer::default();
        assert!(host
            .run_body_durable_external_plot_controlled_to(
                &owner,
                root.path(),
                fragment,
                &inputs,
                2,
                &mut Collector::default(),
                &mut Vec::new(),
                &mut timer,
                &RunControl::default()
            )
            .is_err());
        assert!(!timer.started, "invalid Fore must refuse before Play");
    }
    let (mut host, fragment) = fixture(true, "boot/cancel");
    let control = RunControl::default();
    control
        .request_stop(RunControlRequestId::new("fixture/pre-cancel").unwrap())
        .unwrap();
    let mut collector = Collector::default();
    let mut replacement = input();
    replacement.bytes = InfoBool::FALSE.encode().to_vec();
    let report = host
        .run_body_durable_external_plot_controlled_to(
            &owner,
            root.path(),
            fragment,
            &[replacement],
            2,
            &mut collector,
            &mut Vec::new(),
            &mut Timer::default(),
            &control,
        )
        .expect("pre-cancel is an ordinary controlled terminal outcome");
    assert_eq!(report.control_receipts.len(), 1);
    assert_eq!(
        report.control_receipts[0].request_id.as_str(),
        "fixture/pre-cancel"
    );
    assert!(collector
        .0
        .iter()
        .all(|value| value.bytes.as_ref() != InfoBool::FALSE.encode()));
    assert_eq!(
        last(&run(root.path(), &owner, false, "boot/post-cancel")),
        InfoBool::TRUE.encode()
    );
}
