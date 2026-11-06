//! Finite local results for the owner receipt, not a durable recorder or Line ack.
use conduit_body::ResidentPlot;
use conduit_core::{
    ConnectionTrack, PortDirection, DEFAULT_CONNECTION_BYTE_CAPACITY,
    DEFAULT_CONNECTION_ITEM_CAPACITY,
};
use conduit_planner::{ConnectionQueueLimits, ForeBoundaryKey};
use conduit_std_host::{body_execution::BodyForeOutputAdapter, ExternalForeDelivery};
use std::collections::{BTreeMap, BTreeSet};

type Boundaries = BTreeMap<ResidentPlot, BTreeMap<ForeBoundaryKey, ConnectionQueueLimits>>;
const ITEMS: usize = DEFAULT_CONNECTION_ITEM_CAPACITY as usize;
pub(super) struct ReceiptFore {
    values: [Option<ExternalForeDelivery>; ITEMS],
    len: usize,
    refusal: Option<String>,
}
impl ReceiptFore {
    pub(super) fn new() -> Self {
        Self {
            values: std::array::from_fn(|_| None),
            len: 0,
            refusal: Some("owner receipt Fore capacity exhausted".into()),
        }
    }
    pub(super) fn snapshot(&self) -> serde_json::Value {
        serde_json::Value::Array(
            self.values
                .iter()
                .flatten()
                .map(|value| {
                    serde_json::json!({
                        "front_port_id": value.front_port_id, "track": value.track,
                        "value_kind": value.value_kind, "sequence": value.sequence,
                        "bytes": value.bytes,
                    })
                })
                .collect(),
        )
    }
}
impl BodyForeOutputAdapter for ReceiptFore {
    fn deliver(&mut self, value: &ExternalForeDelivery) -> Result<(), String> {
        if self.len == ITEMS || value.bytes.len() > DEFAULT_CONNECTION_BYTE_CAPACITY as usize {
            return Err(self.refusal.take().unwrap_or_default());
        }
        // Shared immutable identity/payload clones allocate nothing; IMAGE
        // payloads retain their original byte charge through publication.
        self.values[self.len] = Some(value.clone());
        self.len += 1;
        Ok(())
    }
}

pub(super) fn boundaries(
    sources: &[super::super::super::retained_source::RetainedPlotSource],
) -> Result<Boundaries, String> {
    let mut result = BTreeMap::new();
    let mut names = BTreeSet::<(String, ConnectionTrack)>::new();
    for source in sources {
        // The existing Crèche adapter owns its own exact boundary budgets.
        if source.plot.expanded.name == "creche/main" {
            continue;
        }
        if !source.plot.input_bindings.is_empty() {
            return Err(
                "service Fore ingress requires an explicitly selected input adapter".into(),
            );
        }
        let mut ports = BTreeMap::new();
        for output in &source.plot.output_bindings {
            if !names.insert((output.front_port_id.as_str().to_owned(), output.track)) {
                return Err(
                    "owner receipt Fore outputs require unique port/track identities".into(),
                );
            }
            ports.insert(
                ForeBoundaryKey {
                    direction: PortDirection::Output,
                    front_port_id: output.front_port_id.clone(),
                    track: output.track,
                },
                ConnectionQueueLimits {
                    item_capacity: DEFAULT_CONNECTION_ITEM_CAPACITY,
                    byte_capacity: DEFAULT_CONNECTION_BYTE_CAPACITY,
                },
            );
        }
        if !ports.is_empty() {
            result.insert(
                ResidentPlot::new(
                    source.plot.expanded.source_document_id.clone(),
                    source.plot.expanded.checked_plot_id.clone(),
                ),
                ports,
            );
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn value(sequence: u64, size: usize) -> ExternalForeDelivery {
        ExternalForeDelivery::new(
            "result".into(),
            ConnectionTrack::Payload,
            "fixture/exact-result@1".into(),
            sequence,
            vec![7; size],
        )
    }
    #[test]
    fn receipt_capture_shares_exact_values_and_refuses_pressure_without_mutation() {
        let mut capture = ReceiptFore::new();
        let output = value(0, DEFAULT_CONNECTION_BYTE_CAPACITY as usize);
        for _ in 0..ITEMS {
            capture.deliver(&output).unwrap();
        }
        assert_eq!(
            capture.values[0].as_ref().unwrap().bytes.as_ptr(),
            output.bytes.as_ptr()
        );
        let before = capture.snapshot();
        assert!(capture.deliver(&output).is_err());
        assert_eq!(capture.snapshot(), before);
        let mut capture = ReceiptFore::new();
        assert!(capture
            .deliver(&value(0, DEFAULT_CONNECTION_BYTE_CAPACITY as usize + 1))
            .is_err());
        assert_eq!(capture.len, 0);
    }
    fn source(text: &str) -> super::super::super::super::retained_source::RetainedPlotSource {
        let source = crate::plot_source::parse(text).unwrap();
        let plot = source.expand_entry_for_authoring().unwrap();
        super::super::super::super::retained_source::RetainedPlotSource { source, plot }
    }
    #[test]
    fn receipt_planning_refuses_implicit_ingress_and_ambiguous_workset_outputs() {
        let input = source(include_str!(
            "../../../../../make/body/tests/source_check_call/check.conduit"
        ));
        assert!(boundaries(&[input]).unwrap_err().contains("input adapter"));
        let one = "plot one (\n ticks: Tick...| >>\n) {\n clock: time/tick(count = 1, period-ms = 0)\n clock.tick >> ticks\n}\n";
        let sources = [source(one), source(&one.replace("plot one", "plot two"))];
        assert!(boundaries(&sources)
            .unwrap_err()
            .contains("unique port/track"));
    }
}
