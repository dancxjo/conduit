//! Ordinary typed oscillator Source/Plan/Play proof without device effects.
use conduit_audio::*;
use conduit_core::{
    BaseImplementationId, ConfigurationValue, ConnectionTrack, PortDirection, StructuredInfoValue,
    StructuredInfoValueShape,
};
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    PortableExpressionProgram, ProfileCatalog, StartupCatalog,
};
use conduit_std_host::{
    ExternalForeDelivery, ExternalForeInput, ExternalForeOutputAdapter, StdHost, TimerAdapter,
};
use std::{collections::BTreeMap, time::Duration};
fn source() -> String {
    [
        include_str!("../../../semantics/audio/types.conduit"),
        include_str!("../../../semantics/audio/trajectory_types.conduit"),
        include_str!("../../../semantics/audio/rate_types.conduit"),
        include_str!("../../../semantics/audio/decibel_types.conduit"),
        include_str!("../../../semantics/audio/exact_control_types.conduit"),
        include_str!("../../../semantics/audio/time_analysis_types.conduit"),
        include_str!("../../../semantics/audio/dsp/periodic-types.conduit"),
        PERIODIC_FLOW_SOURCE,
        PERIODIC_OSCILLATOR_SOURCE,
        PERIODIC_VALIDATION_SOURCE,
    ]
    .join("\n")
}
fn anchor() -> AudioTrajectoryAnchor {
    AudioTrajectoryAnchor::new(
        AudioOriginIdentity::new(1).unwrap(),
        AudioTimelineIdentity::new(1).unwrap(),
    )
    .unwrap()
}
fn provenance() -> AudioTrajectoryProvenance {
    AudioTrajectoryProvenance::new(
        AudioTrajectoryProvenanceKind::Authored,
        "periodic fixture".into(),
        None,
    )
    .unwrap()
}
fn request(
    n: u64,
    d: u64,
    phase: i64,
    phase_den: u64,
    amp_n: u64,
    amp_d: u64,
    frames: u32,
) -> AudioPeriodicRequest {
    AudioPeriodicRequest::new(
        AudioRelativeAmplitude::new(amp_d, amp_n).unwrap(),
        AudioSampleRateBasis::new(anchor(), AudioFrameQuantization::Floor, 8000).unwrap(),
        frames,
        AudioFrequencyControl::new(anchor(), provenance(), AudioFrequencyHz::new(d, n).unwrap())
            .unwrap(),
        AudioPhaseReference::new(
            anchor(),
            AudioExactTimeOffset::new(1, 0).unwrap(),
            phase_den,
            phase,
            provenance(),
        )
        .unwrap(),
        AudioPeriodicReference::UnscaledCubicFlow,
    )
    .unwrap()
}
fn started(bytes: &[u8]) -> AudioPeriodicCursorCandidate {
    let value = StructuredInfoValue::from_canonical_bytes(bytes).unwrap();
    let StructuredInfoValueShape::Variant { tag, payload } = value.shape() else {
        panic!("start outcome")
    };
    assert_eq!(tag, "started");
    AudioPeriodicCursorCandidate::decode(&payload.canonical_bytes().unwrap()).unwrap()
}
struct NoTimer;
impl TimerAdapter for NoTimer {
    fn wait(&mut self, _: Duration) {
        panic!("oscillator has no timer/device effect");
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
fn prepare(source: &str, entry: &str, input_port: &str) -> (StdHost, conduit_core::Plan) {
    let checked =
        check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new()).unwrap();
    let authored =
        expand_canonical_plot_for_authoring(&checked, entry, &ProfileCatalog::new()).unwrap();
    let mut advertisement = StdHost::new().advertisement().clone();
    for gear in &authored.expanded.gears {
        let ConfigurationValue::Text(encoded) = &gear.configuration[0].value else {
            panic!("program")
        };
        let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
        let offer =
            conduit_std_offers::pure_expression_std_offer(&program, gear.inputs[0].temporal)
                .unwrap();
        assert!(gear.accepts_realization(&offer));
        if !advertisement
            .capabilities
            .iter()
            .any(|old| old.capability_id == offer.capability_id)
        {
            advertisement.capabilities.push(offer);
        }
    }
    advertisement
        .capabilities
        .sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
    let host = StdHost::from_advertisement(advertisement).unwrap();
    let hosts = [host.advertisement().clone()];
    let placements =
        conduit_planner::default_expanded_placements(&authored.expanded, &hosts).unwrap();
    let boundaries = [
        (PortDirection::Input, input_port),
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
                byte_capacity: 65536,
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
            connection_byte_capacity: 65536,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )
    .unwrap();
    assert!(conduit_core::verify_plan(&plan));
    (host, plan)
}
fn run(
    host: &mut StdHost,
    plan: &conduit_core::Plan,
    port: &str,
    inputs: Vec<Vec<u8>>,
) -> Vec<Vec<u8>> {
    let count = inputs.len();
    let inputs = inputs
        .into_iter()
        .map(|bytes| ExternalForeInput {
            front_port_id: port.into(),
            track: ConnectionTrack::Payload,
            bytes,
        })
        .collect::<Vec<_>>();
    let mut output = Outputs(Vec::with_capacity(count));
    for input in &inputs {
        // Each authored Item advances one finite ordinary Play. Caller batching
        // and fresh-host partitioning do not introduce a second runtime loop.
        let report = host
            .run_external_plot_to(
                plan.fragments[0].clone(),
                std::slice::from_ref(input),
                &mut output,
                &mut Vec::new(),
                &mut NoTimer,
            )
            .unwrap();
        let kernel = report.kernel.unwrap();
        assert_eq!(
            kernel.value_allocation_capacity_before,
            kernel.value_allocation_capacity_after
        );
        assert!(kernel
            .kernel_sign
            .iter()
            .any(|event| event.kind == conduit_kernel::KernelEventKind::HostCallRequested));
        assert!(kernel
            .kernel_sign
            .iter()
            .any(|event| event.kind == conduit_kernel::KernelEventKind::HostCallCompleted));
    }
    assert_eq!(output.0.len(), count);
    output.0
}

#[test]
fn ordinary_plans_preserve_partition_reset_and_explicit_cursor_bytes() {
    let source = source();
    let (mut prepare_host, prepare_plan) = prepare(&source, "dsp/oscillator-prepare", "request");
    let request = request(125, 1, 0, 1, 1, 1, 8);
    let initial = started(
        &run(
            &mut prepare_host,
            &prepare_plan,
            "request",
            vec![request.clone().encode().unwrap()],
        )[0],
    );
    assert_eq!(
        prepare_plan.source_document_id,
        parse_syntax_document(&source).source_document_id()
    );
    let (host, plan) = prepare(&source, "dsp/oscillator-step", "value");
    assert_eq!(plan.source_document_id, prepare_plan.source_document_id);
    assert_ne!(plan.plan_id, prepare_plan.plan_id);
    assert_eq!(plan.fragments.len(), 1);
    let sequence = |partitions: &[u32]| {
        let mut cursor = initial.clone();
        let mut all = Vec::new();
        for &count in partitions {
            let mut inputs = Vec::new();
            for _ in 0..count {
                inputs.push(
                    AudioPeriodicStepInput::new(false, cursor.clone())
                        .unwrap()
                        .encode()
                        .unwrap(),
                );
                // Independently construct the next input; installed Source must
                // validate and reproduce this finite phase/budget recurrence.
                cursor = AudioPeriodicCursorCandidate::new(
                    *cursor.emitted() + 1,
                    *cursor.initial_phase_q8(),
                    (*cursor.phase_q8() + 256) % 16384,
                    *cursor.phase_remainder(),
                    cursor.profile().clone(),
                    *cursor.remaining() - 1,
                )
                .unwrap();
            }
            let mut fresh = StdHost::from_advertisement(host.advertisement().clone()).unwrap();
            let outputs = run(&mut fresh, &plan, "value", inputs);
            for bytes in &outputs {
                let AudioPeriodicOutcome::Advanced(value) =
                    AudioPeriodicOutcome::decode(bytes).unwrap()
                else {
                    panic!("advanced")
                };
                let index = all.len();
                let flow = |frame: usize| {
                    let q = (frame as i128) * 256 / 48;
                    q * q * (256 - q) / 256
                };
                let previous = if index == 0 { 0 } else { flow(index - 1) };
                assert_eq!(*value.sample(), flow(index) - previous);
                assert_eq!(*value.cursor().emitted(), u32::try_from(index + 1).unwrap());
                let expected_cursor = AudioPeriodicCursorCandidate::new(
                    u32::try_from(index + 1).unwrap(),
                    0,
                    (i128::try_from(index + 1).unwrap() * 256) % 16384,
                    0,
                    initial.profile().clone(),
                    7 - u32::try_from(index).unwrap(),
                )
                .unwrap();
                assert_eq!(value.cursor(), &expected_cursor);
                all.push(bytes.clone());
            }
        }
        (all, cursor)
    };
    let whole = sequence(&[8]);
    assert_eq!(whole, sequence(&[3, 5]));
    assert_eq!(whole, sequence(&[8]));
    assert_eq!(*whole.1.remaining(), 0);
    let end = run(
        &mut StdHost::from_advertisement(host.advertisement().clone()).unwrap(),
        &plan,
        "value",
        vec![
            AudioPeriodicStepInput::new(false, whole.1)
                .unwrap()
                .encode()
                .unwrap(),
            AudioPeriodicStepInput::new(true, initial)
                .unwrap()
                .encode()
                .unwrap(),
        ],
    );
    assert!(matches!(
        AudioPeriodicOutcome::decode(&end[0]).unwrap(),
        AudioPeriodicOutcome::Exhausted(_)
    ));
    assert!(matches!(
        AudioPeriodicOutcome::decode(&end[1]).unwrap(),
        AudioPeriodicOutcome::Cancelled(_)
    ));
    assert_eq!(
        started(
            &run(
                &mut prepare_host,
                &prepare_plan,
                "request",
                vec![request.encode().unwrap()]
            )[0]
        )
        .emitted(),
        &0
    );
}
