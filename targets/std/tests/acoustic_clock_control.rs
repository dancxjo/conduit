//! Public catalog Source Back through normal installed Std Host Plan/Play.
//! Declared clock mathematics proves no physical clock calibration.
use conduit_audio::*;
use conduit_core::{BaseImplementationId, ConfigurationValue, ConnectionTrack, PortDirection};
use conduit_data::{ClockRelation, ClockRelationQuality};
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring_with_backs, parse_syntax_document,
    CanonicalBackCatalog, PortableExpressionProgram, ProfileCatalog, StartupCatalog,
};
use conduit_std_host::{
    ExternalForeDelivery, ExternalForeInput, ExternalForeOutputAdapter, StdHost, TimerAdapter,
};
use std::{collections::BTreeMap, time::Duration};
struct NoTimer;
impl TimerAdapter for NoTimer {
    fn wait(&mut self, _: Duration) {
        panic!("abstract acoustic control has no timer/device effect");
    }
}
#[derive(Default)]
struct Outputs(Vec<Vec<u8>>);
impl ExternalForeOutputAdapter for Outputs {
    fn deliver(&mut self, value: ExternalForeDelivery) -> Result<(), String> {
        if value.track == ConnectionTrack::Payload {
            self.0.push(value.bytes);
        }
        Ok(())
    }
}
fn prepare(source: &str, entry: &str, input_port: &str) -> (StdHost, conduit_core::Plan) {
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    conduit_semantic_catalog::install_acoustic_control_catalogs(&mut startup, &mut profiles)
        .unwrap();
    let mut backs = CanonicalBackCatalog::new();
    conduit_semantic_catalog::install_acoustic_control_backs(&profiles, &mut backs).unwrap();
    let package = conduit_semantic_catalog::prepare_acoustic_control_source_package().unwrap();
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let authored =
        expand_canonical_plot_for_authoring_with_backs(&checked, entry, &profiles, &backs).unwrap();
    assert!(!authored.expanded.gears.is_empty());
    println!(
        "clock-control-owner-source={:?}",
        package.checked.source_document_id
    );
    let mut advertisement = StdHost::new().advertisement().clone();
    for gear in &authored.expanded.gears {
        let ConfigurationValue::Text(encoded) = &gear.configuration[0].value else {
            panic!("program")
        };
        let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
        assert_eq!(program.canonical_hex().unwrap(), *encoded);
        let offer =
            conduit_std_offers::pure_expression_std_offer(&program, gear.inputs[0].temporal)
                .unwrap();
        println!("clock-control-program-offer={:?}", offer.capability_id);
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
        println!(
            "clock-control-source={:?} plan={:?} play={:?}",
            plan.source_document_id, plan.plan_id, kernel.active_play_id
        );
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

fn anchor() -> AudioTrajectoryAnchor {
    AudioTrajectoryAnchor::new(
        AudioOriginIdentity::new(9).unwrap(),
        AudioTimelineIdentity::new(4).unwrap(),
    )
    .unwrap()
}
fn binding(rate: u64, source_clock: &str) -> AudioClockBinding {
    let relation = ClockRelation::new(
        "declared-clock-map@1".into(),
        ClockRelationQuality::Exact,
        37,
        "authored-seconds".into(),
        1,
        900,
        "selected-clock".into(),
        rate,
    )
    .unwrap();
    AudioClockBinding::new(
        anchor(),
        relation,
        source_clock.into(),
        37,
        AudioClockSourceUnit::Seconds,
        "selected-clock".into(),
    )
    .unwrap()
}
fn query(n: u64, d: u64) -> AudioTrajectoryQuery {
    AudioTrajectoryQuery::new(anchor(), AudioExactTimeOffset::new(d, n).unwrap()).unwrap()
}
const EXAMPLE_SOURCE: &str =
    include_str!("../../../semantics/audio/examples/clock-control.conduit");

#[test]
fn installed_public_clock_projection_preserves_nonzero_anchors_at_three_rates() {
    let (mut host, plan) = prepare(EXAMPLE_SOURCE, "clock-project", "input");
    for rate in [8000, 16000, 48000] {
        let binding = binding(rate, "authored-seconds");
        let original = binding.clone().encode().unwrap();
        let input = AudioClockProjectionEligible::new(
            AudioClockProjectionRequest::new(binding.clone(), query(1, 3)).unwrap(),
        )
        .unwrap();
        let output = run(&mut host, &plan, "input", vec![input.encode().unwrap()]);
        let actual = AudioClockProjectionRaw::decode(&output[0]).unwrap();
        assert_eq!(*actual.whole_ticks(), 900 + i128::from(rate) / 3);
        assert_eq!(*actual.remainder_numerator(), i128::from(rate) % 3);
        assert_eq!(*actual.remainder_denominator(), 3);
        let native = PreparedAudioClockBinding::new(&original)
            .unwrap()
            .project(&query(1, 3).encode().unwrap())
            .unwrap();
        assert_eq!(native.result().raw(), &actual);
        assert_eq!(binding.clone().encode().unwrap(), original);
        assert_eq!(*binding.relation().source_anchor(), 37);
        assert!(!native.executions().is_empty());
    }
}

#[test]
fn installed_public_control_schedule_distinguishes_100_and_200_hz_queries() {
    let (mut host, plan) = prepare(EXAMPLE_SOURCE, "control-query-time", "input");
    for rate in [100, 200] {
        let schedule = AudioControlProjectionSchedule::new(
            anchor(),
            AudioControlCadence::new(rate).unwrap(),
            AudioControlReconstruction::PointSamplesOnly,
            AudioExactTimeOffset::new(3, 1).unwrap(),
            3,
        )
        .unwrap();
        let before = schedule.clone().encode().unwrap();
        let points = (0..3)
            .map(|index| {
                AudioControlProjectionPoint::new(index, schedule.clone())
                    .unwrap()
                    .encode()
                    .unwrap()
            })
            .collect();
        let outputs = run(&mut host, &plan, "input", points);
        for (index, bytes) in outputs.iter().enumerate() {
            let actual = AudioControlQueryTimeRaw::decode(bytes).unwrap();
            assert_eq!(*actual.numerator_seconds(), rate + index as u64 * 3);
            assert_eq!(*actual.denominator(), 3 * rate);
            // Field-copy admission is the same owner boundary as preparation;
            // the adapter performs no numerical projection.
            let admitted = AudioTrajectoryQuery::new(
                schedule.anchor().clone(),
                AudioExactTimeOffset::new(*actual.denominator(), *actual.numerator_seconds())
                    .unwrap(),
            )
            .unwrap();
            let admission = AudioControlQueryTimeAdmission::new(
                AudioControlProjectionPoint::new(index as u16, schedule.clone()).unwrap(),
                admitted.clone(),
                actual,
            )
            .unwrap();
            assert_eq!(admission.query(), &admitted);
            assert_eq!(admitted.anchor(), schedule.anchor());
            assert_eq!(
                admission.raw().numerator_seconds(),
                admitted.time().numerator_seconds()
            );
            assert_eq!(admission.raw().denominator(), admitted.time().denominator());
        }
        assert_eq!(schedule.encode().unwrap(), before);
    }
}

#[test]
fn installed_public_domain_gate_refuses_wrong_clock_without_mutating_original() {
    let (mut host, plan) = prepare(EXAMPLE_SOURCE, "clock-binding-valid", "input");
    let foreign = binding(100, "foreign-seconds");
    let before = foreign.clone().encode().unwrap();
    assert_eq!(
        run(&mut host, &plan, "input", vec![before.clone()]),
        vec![vec![0]]
    );
    assert_eq!(foreign.clone().encode().unwrap(), before);
    assert!(PreparedAudioClockBinding::new(&before).is_err());
    assert!(AudioControlCadence::new(0).is_err());
    let valid = binding(100, "authored-seconds").encode().unwrap();
    assert_eq!(run(&mut host, &plan, "input", vec![valid]), vec![vec![1]]);
}

#[test]
fn installed_public_query_profile_refuses_bounds_and_foreign_anchor() {
    let (mut host, plan) = prepare(EXAMPLE_SOURCE, "clock-query-eligible", "input");
    let binding = binding(100, "authored-seconds");
    let before = binding.clone().encode().unwrap();
    let foreign = AudioTrajectoryQuery::new(
        AudioTrajectoryAnchor::new(
            AudioOriginIdentity::new(10).unwrap(),
            AudioTimelineIdentity::new(4).unwrap(),
        )
        .unwrap(),
        AudioExactTimeOffset::new(3, 1).unwrap(),
    )
    .unwrap();
    let inputs = [query(u64::MAX, 3), foreign, query(1, 3)]
        .into_iter()
        .map(|query| {
            AudioClockProjectionRequest::new(binding.clone(), query)
                .unwrap()
                .encode()
                .unwrap()
        })
        .collect();
    assert_eq!(
        run(&mut host, &plan, "input", inputs),
        vec![vec![0], vec![0], vec![1]]
    );
    assert_eq!(binding.encode().unwrap(), before);
}
