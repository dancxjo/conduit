#![cfg(target_os = "linux")]
//! Real Source/Plan/Play/file boundary; no editor UI acceptance is implied.
use conduit_core::*;
use conduit_std_host::{ExternalForeDelivery, ExternalForeInput};
#[path = "text_file_play/plan.rs"]
mod fixture;

struct Output(Vec<ExternalForeDelivery>);
impl Default for Output {
    fn default() -> Self {
        Self(Vec::with_capacity(1))
    }
}
impl conduit_std_host::ExternalForeOutputAdapter for Output {
    fn deliver(&mut self, value: ExternalForeDelivery) -> Result<(), String> {
        self.0.push(value);
        Ok(())
    }
}

#[test]
fn ordinary_save_then_fresh_host_load_returns_the_committed_text_generation() {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(|| {
            let root = tempfile::tempdir().unwrap();
            let mut writer = fixture::host(root.path(), "boot/one", true);
            let plan = fixture::plan(&writer, true);
            let mut saved = Output::default();
            let report = writer
                .run_external_plot_to(
                    plan.fragments[0].clone(),
                    &[ExternalForeInput {
                        front_port_id: port_id("value"),
                        track: ConnectionTrack::Payload,
                        bytes: b"I was here.".to_vec(),
                    }],
                    &mut saved,
                    &mut Vec::new(),
                    &mut conduit_std_host::ThreadTimer,
                )
                .unwrap();
            assert!(matches!(
                report.observations.last().map(|event| &event.kind),
                Some(ObservationKind::PlanTerminal {
                    disposition: TerminalDisposition::Completed
                })
            ));
            assert_eq!(saved.0.len(), 1);
            assert_eq!(saved.0[0].front_port_id.as_str(), "saved");
            let reference = saved.0[0].bytes.to_vec();
            assert_eq!(
                conduit_data::DataReference::decode_for(&kind_id("value/text"), &reference)
                    .unwrap()
                    .reference(),
                &fixture::reference()
            );
            drop(writer);
            let mut reader = fixture::host(root.path(), "boot/two", false);
            let plan = fixture::plan(&reader, false);
            let mut loaded = Output::default();
            let report = reader
                .run_external_plot_to(
                    plan.fragments[0].clone(),
                    &[ExternalForeInput {
                        front_port_id: port_id("selected"),
                        track: ConnectionTrack::Payload,
                        bytes: reference,
                    }],
                    &mut loaded,
                    &mut Vec::new(),
                    &mut conduit_std_host::ThreadTimer,
                )
                .unwrap();
            assert!(matches!(
                report.observations.last().map(|event| &event.kind),
                Some(ObservationKind::PlanTerminal {
                    disposition: TerminalDisposition::Completed
                })
            ));
            assert_eq!(loaded.0.len(), 1);
            assert_eq!(loaded.0[0].front_port_id.as_str(), "value");
            assert_eq!(loaded.0[0].bytes.as_slice(), b"I was here.");
        })
        .unwrap()
        .join()
        .unwrap();
}

#[path = "text_file_play/body.rs"]
mod body;

#[test]
fn a_wrapper_cannot_hide_the_publication_terminal() {
    let root = tempfile::tempdir().unwrap();
    let writer = fixture::host(root.path(), "boot/current", true);
    assert!(fixture::try_plan(&writer, true, false).is_err());
}

#[test]
fn resealing_cannot_hide_a_split_publication_terminal_from_core_verification() {
    let root = tempfile::tempdir().unwrap();
    let writer = fixture::host(root.path(), "boot/current", true);
    let plan = fixture::plan(&writer, true);
    assert!(verify_plan(&plan));
    let mut fragments = plan.fragments.clone();
    fragments[0]
        .fore_ports
        .retain(|port| port.track != ConnectionTrack::AbnormalTerminal);
    let forged = seal_plan_with_activation_entries(
        PlotIdentity {
            source_document_id: plan.source_document_id,
            checked_plot_id: plan.checked_plot_id,
            expanded_plot_id: plan.expanded_plot_id,
        },
        plan.completion_policy,
        plan.realization_backs,
        plan.activations,
        fragments,
    );
    assert!(!verify_plan(&forged));
}
