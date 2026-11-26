//! Actual finite file effect, kernel State commit and fresh-process recovery.
//! Grants and Body identity are explicit CPU fixtures, not producer ACK policy.
use super::*;
use crate::json_snapshot_file::SelectedJsonSnapshotResidence;
use conduit_core::*;
use conduit_planner::{
    default_expanded_placements, plan_expanded_authoring_with_connection_limits,
    ConnectionQueueLimits, ForeBoundaryKey, PlanningOptions,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    PortableExpressionProgram, ProfileCatalog, StartupCatalog,
};
use std::{collections::BTreeMap, fs::File, path::Path, time::Duration};
const JSON: &[u8] = br#"{"value":1}"#;
const TEST: &str = "host_execution::json_snapshot::tests::authored_json_reference_state_recovers_in_fresh_process_then_reads_exact_generation";
mod fixture;
use fixture::*;

struct Collector {
    values: Vec<ExternalForeDelivery>,
    control: RunControl,
    optional: bool,
    minimum_deliveries: usize,
    stop_request: Option<RunControlRequestId>,
}
impl ExternalForeOutputAdapter for Collector {
    fn deliver(&mut self, delivery: ExternalForeDelivery) -> Result<(), String> {
        if delivery.front_port_id.as_str() == "overflow" {
            return Err("exactly-one reference Flow overflow".into());
        }
        let done = if self.optional {
            let value = validate_canonical_structured_value(delivery.bytes.as_ref())
                .map_err(|e| format!("optional reference: {e:?}"))?;
            value
                .variant_payload("some")
                .map_err(|e| format!("optional reference: {e:?}"))?
                .is_some()
        } else {
            true
        };
        self.values.push(delivery);
        if done && self.values.len() >= self.minimum_deliveries {
            self.control
                .request_stop(self.stop_request.take().expect("one prepared stop request"))
                .map_err(|e| format!("stop: {e:?}"))?;
        }
        Ok(())
    }
}
#[derive(Default)]
struct Timer;
impl TimerAdapter for Timer {
    fn wait(&mut self, _: Duration) {}
}
fn body() -> conduit_body::BodyId {
    serde_json::from_value(serde_json::json!("body/json-state-fixture")).unwrap()
}
fn attempt(
    root: &Path,
    mode: Mode,
    boot: &str,
    input: Option<Vec<u8>>,
) -> Result<(Vec<ExternalForeDelivery>, PlanFragment), String> {
    let files = root.join("snapshots");
    let state = root.join("state");
    std::fs::create_dir_all(&files).unwrap();
    std::fs::create_dir_all(&state).unwrap();
    let (mut host, fragment, selected) = fixture(&files, boot, mode);
    let input = if matches!(mode, Mode::Read) {
        Some(1_u64.to_le_bytes().to_vec())
    } else {
        input
    };
    let inputs = input
        .map(|bytes| ExternalForeInput {
            front_port_id: port_id(if matches!(mode, Mode::Publish) {
                "json"
            } else {
                "trigger"
            }),
            track: ConnectionTrack::Payload,
            bytes,
        })
        .into_iter()
        .collect::<Vec<_>>();
    let control = RunControl::default();
    let mut collector = Collector {
        values: Vec::with_capacity(3),
        control: control.clone(),
        optional: matches!(mode, Mode::Publish),
        minimum_deliveries: if matches!(mode, Mode::Publish) { 2 } else { 1 },
        stop_request: Some(RunControlRequestId::new("fixture/json-result").unwrap()),
    };
    let report = std::thread::scope(|scope| {
        let (done, waiting) = std::sync::mpsc::sync_channel::<()>(1);
        let stop = control.clone();
        scope.spawn(move || {
            if waiting.recv_timeout(Duration::from_secs(20)).is_err() {
                let _ =
                    stop.request_stop(RunControlRequestId::new("fixture/json-watchdog").unwrap());
            }
        });
        let result = if matches!(mode, Mode::Recover) {
            host.run_body_durable_external_plot_controlled_to(
                &body(),
                &state,
                fragment.clone(),
                &inputs,
                if matches!(mode, Mode::Publish) { 2 } else { 1 },
                &mut collector,
                &mut Vec::new(),
                &mut Timer,
                &control,
            )
        } else {
            host.run_body_durable_json_snapshot_to(
                &body(),
                &state,
                &selected,
                fragment.clone(),
                &inputs,
                if matches!(mode, Mode::Publish) { 2 } else { 1 },
                &mut collector,
                &mut Vec::new(),
                &mut Timer,
                &control,
            )
        };
        let _ = done.send(());
        result
    })?;
    assert_eq!(
        report.kernel.as_ref().unwrap().post_play_start_allocations,
        0,
        "selected JSON/State execution cannot allocate after Play starts"
    );
    if collector.values.is_empty() {
        return Err("no installed result".into());
    }
    Ok((collector.values, fragment))
}
fn execute(
    root: &Path,
    mode: Mode,
    boot: &str,
    input: Option<Vec<u8>>,
) -> (Vec<ExternalForeDelivery>, PlanFragment) {
    attempt(root, mode, boot, input).unwrap()
}
fn recovered_reference(values: &[ExternalForeDelivery]) -> Vec<u8> {
    let value =
        StructuredInfoValue::from_canonical_bytes(values.last().unwrap().bytes.as_ref()).unwrap();
    assert_eq!(value.value_type(), &optional().0);
    let StructuredInfoValueShape::Variant {
        tag: "some",
        payload,
    } = value.shape()
    else {
        panic!("committed optional reference")
    };
    let StructuredInfoValueShape::Leaf(bytes) = payload.shape() else {
        panic!("opaque ResourceRef leaf")
    };
    assert_eq!(BoundedResourceRef::decode(bytes).unwrap(), reference());
    bytes.to_vec()
}
#[test]
fn authored_json_reference_state_recovers_in_fresh_process_then_reads_exact_generation() {
    const MODE: &str = "CONDUIT_JSON_STATE_TEST_MODE";
    const ROOT: &str = "CONDUIT_JSON_STATE_TEST_ROOT";
    if let Ok(mode) = std::env::var(MODE) {
        let root = std::path::PathBuf::from(std::env::var_os(ROOT).unwrap());
        let (values, _) = execute(
            &root,
            if mode == "publish" {
                Mode::Publish
            } else {
                Mode::Recover
            },
            &format!("boot/{mode}"),
            (mode == "publish").then(|| JSON.to_vec()),
        );
        let _reference = recovered_reference(&values);
        if mode == "recover" {
            let (read, _) = execute(&root, Mode::Read, "boot/independent-read", None);
            assert_eq!(read.last().unwrap().bytes.as_ref(), JSON);
        }
        return;
    }
    let root = tempfile::tempdir().unwrap();
    let files = root.path().join("snapshots");
    std::fs::create_dir_all(&files).unwrap();
    let (_, write, _) = fixture(&files, "boot/check-write", Mode::Publish);
    let (_, read, _) = fixture(&files, "boot/check-read", Mode::Recover);
    assert_eq!(write.states[0].state_id, read.states[0].state_id);
    assert_eq!(write.states[0].value_kind, read.states[0].value_kind);
    assert_eq!(write.states[0].initial_value, read.states[0].initial_value);
    for mode in ["publish", "recover"] {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", TEST, "--nocapture"])
            .env(MODE, mode)
            .env(ROOT, root.path())
            .status()
            .unwrap();
        assert!(status.success(), "fresh {mode} process failed");
    }
}

#[test]
fn malformed_json_publication_cannot_replace_committed_reference_state() {
    let root = tempfile::tempdir().unwrap();
    assert!(attempt(
        root.path(),
        Mode::Publish,
        "boot/initial-refusal",
        Some(b"not JSON!!!".to_vec())
    )
    .is_err());
    let (empty, _) = execute(root.path(), Mode::Recover, "boot/empty-recovery", None);
    let empty_value =
        validate_canonical_structured_value(empty.last().unwrap().bytes.as_ref()).unwrap();
    assert!(empty_value.variant_payload("none").unwrap().is_some());
    assert!(
        attempt(
            root.path(),
            Mode::Read,
            "boot/read-before-publication",
            None
        )
        .is_err(),
        "sampled None must not become a fabricated reference or JSON output"
    );
    let (still_empty, _) = execute(root.path(), Mode::Recover, "boot/empty-after-read", None);
    let empty_value =
        validate_canonical_structured_value(still_empty.last().unwrap().bytes.as_ref()).unwrap();
    assert!(empty_value.variant_payload("none").unwrap().is_some());
    assert_eq!(
        std::fs::read_dir(root.path().join("snapshots"))
            .unwrap()
            .count(),
        0
    );
    let (published, _) = execute(
        root.path(),
        Mode::Publish,
        "boot/first",
        Some(JSON.to_vec()),
    );
    let accepted = recovered_reference(&published);
    let refused = attempt(
        root.path(),
        Mode::Publish,
        "boot/refused",
        Some(b"not JSON!!!".to_vec()),
    );
    assert!(
        refused.is_err(),
        "malformed JSON must terminate without another State output"
    );
    assert!(attempt(
        root.path(),
        Mode::Publish,
        "boot/occupied",
        Some(JSON.to_vec())
    )
    .is_err());
    let (recovered, _) = execute(root.path(), Mode::Recover, "boot/recovered", None);
    assert_eq!(recovered_reference(&recovered), accepted);
    let (read, _) = execute(root.path(), Mode::Read, "boot/read-after-refusal", None);
    assert_eq!(read.last().unwrap().bytes.as_ref(), JSON);
}

mod preparation;

mod admission;

mod source;
