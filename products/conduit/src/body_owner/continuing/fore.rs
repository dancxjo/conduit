//! Finite local results for the owner receipt, not a durable recorder or Line ack.
use conduit_body::ResidentPlot;
use conduit_core::{
    ConnectionTrack, DEFAULT_CONNECTION_BYTE_CAPACITY, DEFAULT_CONNECTION_ITEM_CAPACITY,
    PortDirection,
};
use conduit_planner::{ConnectionQueueLimits, ForeBoundaryKey};
use conduit_std_host::{ExternalForeDelivery, body_execution::BodyForeOutputAdapter};
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

/// One finite Root-selected ingress. Construction requires original IMAGE input
/// custody; labels and queue limits grant no Source or operation authority.
pub(crate) struct ImageIngress {
    pub(crate) plot: ResidentPlot,
    pub(crate) input: conduit_std_host::body_execution::ImageForeInput,
    pub(crate) limits: ConnectionQueueLimits,
}

/// Same checked requirement used by Start and its availability projection.
/// The installed Creche owns its existing explicit adapter separately.
pub(in super::super) fn requires_selected_input_adapter(
    plot: &conduit_plot::ExpandedAuthoringPlot,
) -> bool {
    plot.expanded.name != "creche/main" && !plot.input_bindings.is_empty()
}

#[cfg(test)]
pub(super) fn boundaries(
    sources: &[super::super::super::retained_source::RetainedPlotSource],
) -> Result<Boundaries, String> {
    boundaries_with_ingress(sources, None)
}

#[cfg(test)]
pub(super) fn boundaries_with_ingress(
    sources: &[super::super::super::retained_source::RetainedPlotSource],
    ingress: Option<&ImageIngress>,
) -> Result<Boundaries, String> {
    boundaries_for_plots(sources.iter().map(|source| &source.plot), ingress)
}

pub(in super::super) fn boundaries_for_plots<'a>(
    plots: impl IntoIterator<Item = &'a conduit_plot::ExpandedAuthoringPlot>,
    ingress: Option<&ImageIngress>,
) -> Result<Boundaries, String> {
    boundaries_for_plots_with_local(plots, ingress, None)
}

pub(in super::super) fn boundaries_for_plots_with_local<'a>(
    plots: impl IntoIterator<Item = &'a conduit_plot::ExpandedAuthoringPlot>,
    ingress: Option<&ImageIngress>,
    local: Option<&crate::durable_host::local_fore_policy::SelectedLocalForeSet>,
) -> Result<Boundaries, String> {
    let mut selected = false;
    let mut result = BTreeMap::new();
    let mut names = BTreeSet::<(String, ConnectionTrack)>::new();
    for plot in plots {
        // The existing Crèche adapter owns its own exact boundary budgets.
        if plot.expanded.name == "creche/main" {
            continue;
        }
        let resident = ResidentPlot::new(
            plot.expanded.source_document_id.clone(),
            plot.expanded.checked_plot_id.clone(),
        );
        let local_ports = local.and_then(|set| set.boundaries(&resident));
        let mut ports = local_ports.cloned().unwrap_or_default();
        if requires_selected_input_adapter(plot) && local_ports.is_none() {
            let resident = ResidentPlot::new(
                plot.expanded.source_document_id.clone(),
                plot.expanded.checked_plot_id.clone(),
            );
            let selected_input = ingress
                .filter(|input| input.plot == resident)
                .ok_or("service Fore ingress requires an explicitly selected input adapter")?;
            let input = selected_input.input.input();
            if selected
                || plot.input_bindings.len() != 1
                || plot.input_bindings[0].front_port_id != input.front_port_id
                || plot.input_bindings[0].track != input.track
                || selected_input.limits.item_capacity != 1
                || selected_input.limits.byte_capacity == 0
                || input.bytes.len() > selected_input.limits.byte_capacity as usize
            {
                return Err("selected IMAGE ingress differs from exact finite Fore".into());
            }
            ports.insert(
                ForeBoundaryKey {
                    direction: PortDirection::Input,
                    front_port_id: input.front_port_id.clone(),
                    track: input.track,
                },
                selected_input.limits,
            );
            selected = true;
        }
        for output in &plot.output_bindings {
            if ports.contains_key(&ForeBoundaryKey {
                direction: PortDirection::Output,
                front_port_id: output.front_port_id.clone(),
                track: output.track,
            }) {
                continue;
            }
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
        if !ports.is_empty() || local_ports.is_some() {
            result.insert(
                ResidentPlot::new(
                    plot.expanded.source_document_id.clone(),
                    plot.expanded.checked_plot_id.clone(),
                ),
                ports,
            );
        }
    }
    if ingress.is_some() && !selected {
        return Err("selected IMAGE ingress is absent from complete workset".into());
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
        assert!(
            capture
                .deliver(&value(0, DEFAULT_CONNECTION_BYTE_CAPACITY as usize + 1))
                .is_err()
        );
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
        assert!(
            boundaries(&sources)
                .unwrap_err()
                .contains("unique port/track")
        );
    }
    #[test]
    fn explicit_ingress_requires_exact_resident_port_and_finite_capacity() {
        use conduit_core::{ConnectionTrack, DEFAULT_CONNECTION_BYTE_CAPACITY};
        let source = source(include_str!(
            "../../../../../make/body/tests/source_check_call/check.conduit"
        ));
        let resident = ResidentPlot::new(
            source.plot.expanded.source_document_id.clone(),
            source.plot.expanded.checked_plot_id.clone(),
        );
        let mut host = super::super::image_tests::host(64 * 1024 * 1024);
        let input = host
            .retain_selected_image_fore_input(conduit_std_host::ExternalForeInput {
                front_port_id: "request".into(),
                track: ConnectionTrack::Payload,
                bytes: vec![0; 16],
            })
            .unwrap();
        let mut ingress = ImageIngress {
            plot: resident.clone(),
            input,
            limits: ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: DEFAULT_CONNECTION_BYTE_CAPACITY,
            },
        };
        let sources = [source];
        assert!(boundaries(&sources).is_err());
        let selected = boundaries_with_ingress(&sources, Some(&ingress)).unwrap();
        assert_eq!(selected[&resident].len(), 2);
        ingress.limits.byte_capacity = 15;
        assert!(boundaries_with_ingress(&sources, Some(&ingress)).is_err());
        ingress.limits.byte_capacity = DEFAULT_CONNECTION_BYTE_CAPACITY;
        ingress.limits.item_capacity = 2;
        assert!(boundaries_with_ingress(&sources, Some(&ingress)).is_err());
        ingress.limits.item_capacity = 1;
        ingress.plot.source_document_id = "foreign/source".into();
        assert!(boundaries_with_ingress(&sources, Some(&ingress)).is_err());
        assert!(boundaries_with_ingress(&[], Some(&ingress)).is_err());
        host.verify_selected_image_fore_input(&ingress.input)
            .unwrap();
    }
}
