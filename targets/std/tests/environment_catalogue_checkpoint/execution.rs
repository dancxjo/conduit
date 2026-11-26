//! Ordinary finite runners and controlled stop, without a recorder ACK policy.
use super::*;
use conduit_std_host::{
    ExternalForeDelivery, ExternalForeInput, ExternalForeOutputAdapter, RunControl,
    RunControlRequestId,
};
use std::{path::Path, time::Duration};
struct Collector {
    values: Vec<ExternalForeDelivery>,
    control: RunControl,
    write: bool,
    stop: Option<RunControlRequestId>,
}
impl ExternalForeOutputAdapter for Collector {
    fn deliver(&mut self, value: ExternalForeDelivery) -> Result<(), String> {
        let canonical = c::validate_canonical_structured_value(value.bytes.as_ref())
            .map_err(|e| format!("optional reference: {e:?}"))?;
        let present = canonical
            .variant_payload("some")
            .map_err(|e| format!("optional reference: {e:?}"))?
            .is_some();
        self.values.push(value);
        if !self.write || (present && self.values.len() >= 2) {
            self.control
                .request_stop(self.stop.take().expect("one terminal stop"))
                .map_err(|e| format!("stop: {e:?}"))?;
        }
        Ok(())
    }
}
pub fn body(name: &str) -> conduit_body::BodyId {
    serde_json::from_value(serde_json::json!(name)).unwrap()
}
pub fn execute(
    root: &Path,
    boot: &str,
    body_name: &str,
    input: Option<Vec<u8>>,
    cancel: bool,
) -> Result<(Vec<ExternalForeDelivery>, c::PlanFragment), String> {
    let files = root.join("snapshots");
    let state = root.join("state");
    std::fs::create_dir_all(&files).unwrap();
    std::fs::create_dir_all(&state).unwrap();
    let write = input.is_some();
    let (mut host, fragment, selected) = host::prepare(&files, boot, write);
    let inputs = input
        .map(|bytes| ExternalForeInput {
            front_port_id: "catalog".into(),
            track: c::ConnectionTrack::Payload,
            bytes,
        })
        .into_iter()
        .collect::<Vec<_>>();
    let control = RunControl::default();
    if cancel {
        control
            .request_stop(RunControlRequestId::new("fixture/precancel").unwrap())
            .unwrap();
    }
    let mut collector = Collector {
        values: Vec::with_capacity(if write { 2 } else { 1 }),
        control: control.clone(),
        write,
        stop: Some(RunControlRequestId::new("fixture/catalogue-result").unwrap()),
    };
    let mut timer = allocation::PlayProbe::default();
    let report = std::thread::scope(|scope| {
        let (done, waiting) = std::sync::mpsc::sync_channel::<()>(1);
        let stop = control.clone();
        scope.spawn(move || {
            if waiting.recv_timeout(Duration::from_secs(20)).is_err() {
                let _ = stop
                    .request_stop(RunControlRequestId::new("fixture/catalogue-watchdog").unwrap());
            }
        });
        let result = if let Some(selected) = selected.as_ref() {
            host.run_body_durable_json_snapshot_to(
                &body(body_name),
                &state,
                selected,
                fragment.clone(),
                &inputs,
                2,
                &mut collector,
                &mut Vec::new(),
                &mut timer,
                &control,
            )
        } else {
            host.run_body_durable_external_plot_controlled_to(
                &body(body_name),
                &state,
                fragment.clone(),
                &inputs,
                1,
                &mut collector,
                &mut Vec::new(),
                &mut timer,
                &control,
            )
        };
        let _ = done.send(());
        result
    })?;
    if let Some(kernel) = report.kernel.as_ref() {
        timer.assert_finished_without_allocation();
        assert_eq!(
            kernel.value_allocation_capacity_before,
            kernel.value_allocation_capacity_after
        );
    } else {
        assert!(cancel, "successful execution requires a kernel receipt");
    }
    assert_eq!(
        control.accepted_stop_request().unwrap().as_str(),
        if cancel {
            "fixture/precancel"
        } else {
            "fixture/catalogue-result"
        }
    );
    if cancel {
        assert!(collector.values.is_empty());
    } else {
        assert!(!collector.values.is_empty());
    }
    Ok((collector.values, fragment))
}
pub fn reference_bytes(values: &[ExternalForeDelivery]) -> Option<Vec<u8>> {
    let value = c::StructuredInfoValue::from_canonical_bytes(values.last().unwrap().bytes.as_ref())
        .unwrap();
    let c::StructuredInfoValueShape::Variant { tag, payload } = value.shape() else {
        panic!("optional")
    };
    if tag == "none" {
        return None;
    }
    assert_eq!(tag, "some");
    let c::StructuredInfoValueShape::Leaf(bytes) = payload.shape() else {
        panic!("reference")
    };
    Some(bytes.to_vec())
}
