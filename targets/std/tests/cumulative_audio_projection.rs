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
struct Outputs(Vec<conduit_std_host::ExternalForeBytes>);
impl ExternalForeOutputAdapter for Outputs {
    fn deliver(&mut self, value: ExternalForeDelivery) -> Result<(), String> {
        if value.track == ConnectionTrack::Payload {
            self.0.push(value.bytes);
        }
        Ok(())
    }
}

fn execute_projection(
    kind: &str,
    input_type: &str,
    output_type: &str,
    bytes: Vec<u8>,
    expected: Option<&[u8]>,
) {
    let types = [
        include_str!("../../../semantics/audio/types.conduit"),
        include_str!("../../../semantics/audio/trajectory_types.conduit"),
        include_str!("../../../semantics/audio/rate_types.conduit"),
    ]
    .join("\n");
    let laws = include_str!("../../../semantics/audio/rate_projection.conduit");
    let source = format!("{types}\n{laws}\nplot projection (\n >> value: {input_type}\n result: {output_type} >>\n) {{\n operation: {kind}\n value >> operation.value\n operation.result >> result\n}}\n");
    let checked =
        check_syntax_document(&parse_syntax_document(&source), &StartupCatalog::new()).unwrap();
    let authored =
        expand_canonical_plot_for_authoring(&checked, "projection", &ProfileCatalog::new())
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
    let plan_id = plan.plan_id.clone();
    let mut outputs = Outputs::default();
    let report = host.run_external_plot_to(
        plan.fragments[0].clone(),
        &[ExternalForeInput {
            front_port_id: "value".into(),
            track: ConnectionTrack::Payload,
            bytes: bytes.clone(),
        }],
        &mut outputs,
        &mut vec![],
        &mut NoTimer,
    );
    let Some(expected) = expected else {
        assert!(report.is_err(), "malformed input must refuse");
        assert!(outputs.0.is_empty(), "refusal emits no partial output");
        return;
    };
    let report = report.unwrap();
    assert_eq!(outputs.0, [expected.to_vec()]);
    let kernel = report.kernel.unwrap();
    assert!(kernel
        .kernel_sign
        .iter()
        .any(|event| event.kind == conduit_kernel::KernelEventKind::HostCallCompleted));
    println!("AUDIO_CUMULATIVE_EVIDENCE kind={kind} source={:?} plan={:?} play={:?} input={:?} output={:?}", checked.source_document_id, plan_id, kernel.active_play_id, bytes, outputs.0);
    assert_eq!(
        kernel.value_allocation_capacity_before,
        kernel.value_allocation_capacity_after
    );
}

#[test]
fn cumulative_thirds_execute_installed_projection_at_three_sample_rates() {
    for rate in [8000u64, 16000, 48000] {
        let anchor = AudioTrajectoryAnchor::new(
            AudioOriginIdentity::new(1).unwrap(),
            AudioTimelineIdentity::new(1).unwrap(),
        )
        .unwrap();
        let basis = AudioSampleRateBasis::new(anchor, AudioFrameQuantization::Floor, rate).unwrap();
        let cumulative_basis = AudioCumulativeFrameBasis::new(basis.clone(), 3).unwrap();
        let chain = AudioSampleProjectionChain::new(
            cumulative_basis.clone(),
            conduit_plot::rust_binding::BoundedSequence::try_from_iter(
                vec![AudioSampleProjectionQuantity::duration(3, 1).unwrap(); 3],
            )
            .unwrap(),
        )
        .unwrap();
        let prepared = PreparedAudioSampleProjection::new()
            .unwrap()
            .chain(&chain.encode().unwrap())
            .unwrap();
        let mut whole = 0;
        let mut remainder = 0;
        for (index, step) in prepared.steps().iter().enumerate() {
            let cursor =
                AudioCumulativeFrameCursor::new(cumulative_basis.clone(), remainder, whole)
                    .unwrap();
            let request = AudioCumulativeFrameRequest::new(
                AudioSampleProjectionRequest::new(
                    basis.clone(),
                    AudioSampleProjectionQuantity::duration(3, 1).unwrap(),
                )
                .unwrap(),
                cursor,
            )
            .unwrap();
            assert_eq!(step.original(), &request);
            let numerator = rate * (index as u64 + 1);
            let next_whole = numerator / 3;
            let next_remainder = numerator % 3;
            assert_eq!(*step.result().raw().frame_count(), next_whole - whole);
            assert_eq!(*step.result().raw().whole_frames(), next_whole);
            assert_eq!(*step.result().raw().remainder_numerator(), next_remainder);
            assert_eq!(step.executions().len(), 4);
            let basis_check = &step.executions()[1];
            let append = &step.executions()[2];
            assert_eq!(basis_check.input_canonical(), append.input_canonical());
            execute_projection(
                "audio/cumulative-basis-equal",
                "AudioCumulativeFrameEligible",
                "Boolean",
                basis_check.input_canonical().to_vec(),
                Some(&[1]),
            );
            execute_projection(
                "audio/cumulative-append",
                "AudioCumulativeFrameEligible",
                "AudioCumulativeFrameRaw",
                append.input_canonical().to_vec(),
                Some(&step.result().raw().clone().encode().unwrap()),
            );
            whole = next_whole;
            remainder = next_remainder;
        }
        assert_eq!((whole, remainder), (rate, 0));
    }
}

#[test]
fn installed_projection_refuses_malformed_and_detects_foreign_anchor() {
    execute_projection(
        "audio/cumulative-append",
        "AudioCumulativeFrameEligible",
        "AudioCumulativeFrameRaw",
        vec![0, 1],
        None,
    );
    let anchor = |origin| {
        AudioTrajectoryAnchor::new(
            AudioOriginIdentity::new(origin).unwrap(),
            AudioTimelineIdentity::new(1).unwrap(),
        )
        .unwrap()
    };
    let basis = |origin| {
        AudioSampleRateBasis::new(anchor(origin), AudioFrameQuantization::Floor, 8000).unwrap()
    };
    let cursor =
        AudioCumulativeFrameCursor::new(AudioCumulativeFrameBasis::new(basis(1), 3).unwrap(), 0, 0)
            .unwrap();
    let request = AudioCumulativeFrameRequest::new(
        AudioSampleProjectionRequest::new(
            basis(2),
            AudioSampleProjectionQuantity::duration(3, 1).unwrap(),
        )
        .unwrap(),
        cursor,
    )
    .unwrap();
    assert!(matches!(
        PreparedAudioSampleProjection::new()
            .unwrap()
            .append(&request.clone().encode().unwrap()),
        Err(AudioRateProjectionRefusal::ForeignBasis)
    ));
}
