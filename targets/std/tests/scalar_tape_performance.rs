//! Scoped scalar reference/tape release timing through ordinary Source/Plan/Play.
use conduit_core::{BaseImplementationId, ConfigurationValue, ConnectionTrack, PortDirection};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    PortableExpressionProgram, ProfileCatalog, StartupCatalog,
};
use conduit_std_host::{
    ExternalForeDelivery, ExternalForeInput, ExternalForeOutputAdapter, StdHost, TimerAdapter,
};
use std::{collections::BTreeMap, time::Duration};

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
fn prepare(
    source: &str,
    name: &str,
    tape: bool,
) -> (StdHost, conduit_core::Plan, PortableExpressionProgram) {
    let checked =
        check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new()).unwrap();
    let authored =
        expand_canonical_plot_for_authoring(&checked, name, &ProfileCatalog::new()).unwrap();
    let ConfigurationValue::Text(encoded) = &authored.expanded.gears[0].configuration[0].value
    else {
        panic!("program")
    };
    let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
    let mut advertisement = StdHost::new().advertisement().clone();
    let temporal = authored.expanded.gears[0].inputs[0].temporal;
    let offer = if tape {
        conduit_std_offers::pure_scalar_tape_std_offer(&program, temporal).unwrap()
    } else {
        conduit_std_offers::pure_expression_std_offer(&program, temporal).unwrap()
    };
    assert!(authored.expanded.gears[0].accepts_realization(&offer));
    advertisement.capabilities.push(offer);
    advertisement
        .capabilities
        .sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
    let host = StdHost::from_advertisement(advertisement).unwrap();
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
    (host, plan, program)
}

#[derive(Default)]
struct PhaseClock {
    marks: [Option<std::time::Instant>; 3],
}
impl TimerAdapter for PhaseClock {
    fn wait(&mut self, _: Duration) {
        panic!("scalar profile has no timer effect");
    }
    fn observe_kernel_phase(&mut self, phase: conduit_std_host::KernelTimingPhase) {
        let index = match phase {
            conduit_std_host::KernelTimingPhase::PreparationStarted => 0,
            conduit_std_host::KernelTimingPhase::PlayStarted => 1,
            conduit_std_host::KernelTimingPhase::PlayFinished => 2,
        };
        assert!(self.marks[index].is_none());
        self.marks[index] = Some(std::time::Instant::now());
    }
}
#[test]
#[ignore = "manual report-only release benchmark; timings are not correctness thresholds"]
fn source_pinned_scalar_profiles_measure_preparation_and_actual_kernel_play() {
    let source_identity =
        std::env::var("CONDUIT_SCALAR_TIMING_SOURCE").expect("explicit clean source pin");
    assert!(source_identity.len() == 40 && source_identity.bytes().all(|c| c.is_ascii_hexdigit()));
    let source =
        include_str!("../../../architecture/plot/tests/fixtures/expression_baseline.conduit");
    for operations in [1u64, 3, 8, 32] {
        for tape in [false, true] {
            for row in 0..8 {
                let authoring_started = std::time::Instant::now();
                let (mut host, plan, program) =
                    prepare(source, &format!("baseline-{operations}"), tape);
                let authoring_plan_wall_ns = authoring_started.elapsed().as_nanos();
                let evaluator_started = std::time::Instant::now();
                let mut evaluator = if tape {
                    conduit_plot::PreparedPortableExpressionEvaluator::new_scalar_tape(&program)
                        .unwrap()
                } else {
                    conduit_plot::PreparedPortableExpressionEvaluator::new(&program).unwrap()
                };
                let evaluator_preparation_wall_ns = evaluator_started.elapsed().as_nanos();
                let input = 7u64.to_le_bytes();
                let expected = program.evaluate(&input).unwrap();
                assert_eq!(evaluator.evaluate(&input).unwrap(), expected);
                let warm_started = std::time::Instant::now();
                for _ in 0..100_000 {
                    std::hint::black_box(evaluator.evaluate(std::hint::black_box(&input)).unwrap());
                }
                let warm_evaluator_wall_ns = warm_started.elapsed().as_nanos();
                let rust_started = std::time::Instant::now();
                for _ in 0..100_000 {
                    std::hint::black_box(
                        std::hint::black_box(7u64)
                            .checked_mul(std::hint::black_box(operations + 1))
                            .unwrap(),
                    );
                }
                let optimized_rust_wall_ns = rust_started.elapsed().as_nanos();
                println!("SCALAR_MICRO_SAMPLE source={source_identity} operations={operations} tape={tape} row={row} iterations=100000 authoring_plan_wall_ns={authoring_plan_wall_ns} evaluator_preparation_wall_ns={evaluator_preparation_wall_ns} warm_evaluator_wall_ns={warm_evaluator_wall_ns} optimized_rust_wall_ns={optimized_rust_wall_ns} limits={:?}",evaluator.scalar_tape_limits());
                let mut output = Outputs(Vec::with_capacity(1));
                let mut clock = PhaseClock::default();
                let report = host
                    .run_external_plot_to(
                        plan.fragments[0].clone(),
                        &[ExternalForeInput {
                            front_port_id: "value".into(),
                            track: ConnectionTrack::Payload,
                            bytes: input.to_vec(),
                        }],
                        &mut output,
                        &mut Vec::new(),
                        &mut clock,
                    )
                    .unwrap();
                assert_eq!(output.0, [expected]);
                let [Some(start), Some(play), Some(end)] = clock.marks else {
                    panic!("missing actual phase marker")
                };
                let kernel = report.kernel.unwrap();
                println!("SCALAR_PRODUCT_SAMPLE source={source_identity} operations={operations} tape={tape} row={row} preparation_wall_ns={} play_wall_ns={} plan={} play={} provider={} artifact={}",play.duration_since(start).as_nanos(),end.duration_since(play).as_nanos(),plan.plan_id.as_str(),kernel.active_play_id.as_str(),plan.fragments[0].placements[0].implementation_id.as_str(),plan.fragments[0].placements[0].artifact_id.as_str());
            }
        }
    }
}
