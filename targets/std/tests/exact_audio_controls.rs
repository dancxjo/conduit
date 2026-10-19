//! Checked quantity preparation crosses an authored typed control Gear and Kernel.
use conduit_audio::*;
use conduit_core::*;
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_plot::*;
use conduit_std_host::{
    ExternalForeDelivery, ExternalForeInput, ExternalForeOutputAdapter, StdHost, TimerAdapter,
};
use std::collections::BTreeMap;
struct NoTimer;
impl TimerAdapter for NoTimer {
    fn wait(&mut self, _: std::time::Duration) {
        panic!("control has no timer effect")
    }
}
#[derive(Default)]
struct Outputs(Vec<Vec<u8>>);
impl ExternalForeOutputAdapter for Outputs {
    fn deliver(&mut self, value: ExternalForeDelivery) -> Result<(), String> {
        if value.track == ConnectionTrack::Payload {
            self.0.push(value.bytes.to_vec());
        }
        Ok(())
    }
}

#[test]
fn checked_exact_controls_cross_installed_source_plan_play_without_rounding() {
    for (literal, name, kind) in [
        ("440Hz", "AudioFrequencyControl", "audio/frequency-control"),
        ("1.25Hz", "AudioFrequencyControl", "audio/frequency-control"),
        ("250ms", "AudioTimeControl", "audio/time-control"),
    ] {
        let startup = StartupCatalog::new();
        let types = [
            include_str!("../../../semantics/audio/types.conduit"),
            include_str!("../../../semantics/audio/trajectory_types.conduit"),
            include_str!("../../../semantics/audio/rate_types.conduit"),
            include_str!("../../../semantics/audio/decibel_types.conduit"),
            include_str!("../../../semantics/audio/exact_control_types.conduit"),
        ]
        .join("\n");
        let laws = include_str!("../../../semantics/audio/exact_controls.conduit");
        let source = format!("{types}\n{laws}\nplot controls (\n >> value: {name} <=4096B\n result: {name} >>\n) {{\n authored = {literal}\n control: {kind}\n value >> control.value\n control.result >> result\n}}\n");
        let syntax = parse_syntax_document(&source);
        let checked = check_syntax_document(&syntax, &startup).unwrap();
        assert_eq!(syntax.source_document_id(), checked.source_document_id);
        let control_plot = checked
            .plots
            .iter()
            .find(|plot| plot.name == "controls")
            .unwrap();
        let CanonicalStartupValue::Quantity(quantity) = &control_plot.local_values[0].1 else {
            panic!("checked authored quantity")
        };
        let mut work = ExactNumericWork::new(1_000_000_000);
        let anchor = AudioTrajectoryAnchor::new(
            AudioOriginIdentity::new(17).unwrap(),
            AudioTimelineIdentity::new(31).unwrap(),
        )
        .unwrap();
        let provenance = AudioTrajectoryProvenance::new(
            AudioTrajectoryProvenanceKind::Authored,
            "checked literal".into(),
            None,
        )
        .unwrap();
        let bytes = if name == "AudioFrequencyControl" {
            let prepared =
                prepare_exact_audio_frequency(quantity.value(), quantity.source(), &mut work)
                    .unwrap();
            assert_eq!(prepared.receipt().original(), literal);
            AudioFrequencyControl::new(anchor.clone(), provenance.clone(), *prepared.value())
                .unwrap()
                .encode()
                .unwrap()
        } else {
            let prepared =
                prepare_exact_audio_time(quantity.value(), quantity.source(), &mut work).unwrap();
            assert_eq!(prepared.receipt().original(), literal);
            AudioTimeControl::new(anchor.clone(), provenance.clone(), prepared.value().clone())
                .unwrap()
                .encode()
                .unwrap()
        };
        assert!(bytes.len() <= 4096, "declared finite control envelope");
        let authored =
            expand_canonical_plot_for_authoring(&checked, "controls", &ProfileCatalog::new())
                .unwrap();
        let mut advertisement = StdHost::new().advertisement().clone();
        for gear in &authored.expanded.gears {
            let ConfigurationValue::Text(encoded) = &gear
                .configuration
                .iter()
                .find(|entry| entry.key == "program")
                .unwrap()
                .value
            else {
                panic!("authored control program")
            };
            let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
            advertisement.capabilities.push(
                conduit_std_offers::pure_expression_std_offer(&program, gear.inputs[0].temporal)
                    .unwrap(),
            );
        }
        advertisement
            .capabilities
            .sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
        let mut host = StdHost::from_advertisement(advertisement).unwrap();
        let hosts = [host.advertisement().clone()];
        let placements =
            conduit_planner::default_expanded_placements(&authored.expanded, &hosts).unwrap();
        let boundaries = [
            (PortDirection::Input, "value"),
            (PortDirection::Output, "result"),
        ]
        .into_iter()
        .map(|(direction, port)| {
            (
                conduit_planner::ForeBoundaryKey {
                    direction,
                    front_port_id: port.into(),
                    track: ConnectionTrack::Payload,
                },
                conduit_planner::ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: 4096,
                },
            )
        })
        .collect();
        let plan = conduit_planner::plan_expanded_authoring_with_options(
            &authored,
            &hosts,
            &placements,
            &[BaseImplementationId::from("conduit.base/local@1")],
            conduit_planner::PlanningOptions {
                connection_bases: &BTreeMap::new(),
                line_candidates: &BTreeMap::new(),
                connection_item_capacity: 1,
                connection_byte_capacity: 4096,
                authority_grants: &[],
                protected_resource_grants: &[],
                line_offers: &[],
            },
            &boundaries,
        )
        .unwrap();
        assert_eq!(plan.source_document_id, checked.source_document_id);
        let mut outputs = Outputs::default();
        let report = host
            .run_external_plot_to(
                plan.fragments[0].clone(),
                &[ExternalForeInput {
                    front_port_id: "value".into(),
                    track: ConnectionTrack::Payload,
                    bytes: bytes.clone(),
                }],
                &mut outputs,
                &mut vec![],
                &mut NoTimer,
            )
            .unwrap();
        assert_eq!(outputs.0, [bytes]);
        let kernel = report.kernel.unwrap();
        assert!(kernel
            .kernel_sign
            .iter()
            .any(|event| event.kind == conduit_kernel::KernelEventKind::HostCallCompleted));
        assert_eq!(
            kernel.value_allocation_capacity_before,
            kernel.value_allocation_capacity_after
        );
        if name == "AudioFrequencyControl" {
            let decoded = AudioFrequencyControl::decode(&outputs.0[0]).unwrap();
            assert_eq!(decoded.anchor(), &anchor);
        } else {
            let decoded = AudioTimeControl::decode(&outputs.0[0]).unwrap();
            assert_eq!(decoded.anchor(), &anchor);
        }
    }
}
