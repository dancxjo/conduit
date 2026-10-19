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
            self.0.push(value.bytes.to_vec());
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
    cpu: [Option<u128>; 3],
}
// Diagnostic only: Linux calling-thread CPU, excluding scheduler descheduling.
// Unsupported targets report None rather than substituting elapsed wall time.
fn thread_cpu_ns() -> Option<u128> {
    #[cfg(target_os = "linux")]
    {
        let mut time = libc::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        // SAFETY: a valid writable timespec and a read-only thread clock id.
        if unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut time) } != 0 {
            return None;
        }
        Some(time.tv_sec as u128 * 1_000_000_000 + time.tv_nsec as u128)
    }
    #[cfg(not(target_os = "linux"))]
    None
}
fn cpu_elapsed(start: Option<u128>) -> Option<u128> {
    thread_cpu_ns()?.checked_sub(start?)
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
        self.cpu[index] = thread_cpu_ns();
    }
}
#[test]
#[ignore = "manual report-only release benchmark; timings are not correctness thresholds"]
fn source_pinned_scalar_profiles_measure_preparation_and_actual_kernel_play() {
    let source_identity =
        std::env::var("CONDUIT_SCALAR_TIMING_SOURCE").expect("explicit clean source pin");
    assert!(source_identity.len() == 40 && source_identity.bytes().all(|c| c.is_ascii_hexdigit()));
    for (domain, prefix, source) in [
        (
            "integer",
            "baseline",
            include_str!("../../../architecture/plot/tests/fixtures/expression_baseline.conduit"),
        ),
        (
            "boolean",
            "boolean",
            include_str!(
                "../../../architecture/plot/tests/fixtures/expression_boolean_baseline.conduit"
            ),
        ),
    ] {
        for operations in [1u64, 3, 8, 32] {
            for tape in [false, true] {
                for row in 0..8 {
                    let authoring_started = std::time::Instant::now();
                    let authoring_cpu = thread_cpu_ns();
                    let (mut host, plan, program) =
                        prepare(source, &format!("{prefix}-{operations}"), tape);
                    let authoring_plan_wall_ns = authoring_started.elapsed().as_nanos();
                    let authoring_plan_cpu_ns = cpu_elapsed(authoring_cpu);
                    let evaluator_started = std::time::Instant::now();
                    let evaluator_cpu = thread_cpu_ns();
                    let mut evaluator = if tape {
                        conduit_plot::PreparedPortableExpressionEvaluator::new_scalar_tape(&program)
                            .unwrap()
                    } else {
                        conduit_plot::PreparedPortableExpressionEvaluator::new(&program).unwrap()
                    };
                    let evaluator_preparation_wall_ns = evaluator_started.elapsed().as_nanos();
                    let evaluator_preparation_cpu_ns = cpu_elapsed(evaluator_cpu);
                    let integer_input = 7u64.to_le_bytes();
                    let boolean_input = [1u8];
                    let input = if domain == "integer" {
                        integer_input.as_slice()
                    } else {
                        boolean_input.as_slice()
                    };
                    let expected = program.evaluate(input).unwrap();
                    assert_eq!(evaluator.evaluate(input).unwrap(), expected);
                    let warm_started = std::time::Instant::now();
                    let warm_cpu = thread_cpu_ns();
                    for _ in 0..100_000 {
                        std::hint::black_box(
                            evaluator.evaluate(std::hint::black_box(input)).unwrap(),
                        );
                    }
                    let warm_evaluator_wall_ns = warm_started.elapsed().as_nanos();
                    let warm_evaluator_cpu_ns = cpu_elapsed(warm_cpu);
                    let rust_started = std::time::Instant::now();
                    let rust_cpu = thread_cpu_ns();
                    if domain == "integer" {
                        for _ in 0..100_000 {
                            std::hint::black_box(
                                std::hint::black_box(7u64)
                                    .checked_mul(std::hint::black_box(operations + 1))
                                    .unwrap(),
                            );
                        }
                    } else {
                        // Native optimization may collapse an idempotent conjunction
                        // tree to its input. This reference excludes validation,
                        // dispatch and kernel work, as the integer reference does.
                        for _ in 0..100_000 {
                            std::hint::black_box(std::hint::black_box(input[0] != 0));
                        }
                    }
                    let optimized_rust_wall_ns = rust_started.elapsed().as_nanos();
                    let optimized_rust_cpu_ns = cpu_elapsed(rust_cpu);
                    println!("SCALAR_CPU_SAMPLE source={source_identity} domain={domain} operations={operations} tape={tape} row={row} authoring_plan_cpu_ns={authoring_plan_cpu_ns:?} evaluator_preparation_cpu_ns={evaluator_preparation_cpu_ns:?} warm_evaluator_cpu_ns={warm_evaluator_cpu_ns:?} optimized_rust_cpu_ns={optimized_rust_cpu_ns:?}");
                    println!("SCALAR_MICRO_SAMPLE source={source_identity} domain={domain} operations={operations} tape={tape} row={row} iterations=100000 authoring_plan_wall_ns={authoring_plan_wall_ns} evaluator_preparation_wall_ns={evaluator_preparation_wall_ns} warm_evaluator_wall_ns={warm_evaluator_wall_ns} optimized_rust_wall_ns={optimized_rust_wall_ns} limits={:?}",evaluator.scalar_tape_limits());
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
                    assert_eq!(
                        kernel.value_allocation_capacity_before,
                        kernel.value_allocation_capacity_after
                    );
                    let preparation_cpu_ns = clock.cpu[1]
                        .zip(clock.cpu[0])
                        .map(|(end, start)| end - start);
                    let play_cpu_ns = clock.cpu[2]
                        .zip(clock.cpu[1])
                        .map(|(end, start)| end - start);
                    println!("SCALAR_PRODUCT_CPU_SAMPLE source={source_identity} domain={domain} operations={operations} tape={tape} row={row} preparation_cpu_ns={preparation_cpu_ns:?} play_cpu_ns={play_cpu_ns:?}");
                    println!("SCALAR_PRODUCT_SAMPLE source={source_identity} domain={domain} operations={operations} tape={tape} row={row} preparation_wall_ns={} play_wall_ns={} source_document={} checked={} expanded={} plan={} play={} provider={} artifact={}",play.duration_since(start).as_nanos(),end.duration_since(play).as_nanos(),plan.source_document_id.as_str(),plan.checked_plot_id.as_str(),plan.expanded_plot_id.as_str(),plan.plan_id.as_str(),kernel.active_play_id.as_str(),plan.fragments[0].placements[0].implementation_id.as_str(),plan.fragments[0].placements[0].artifact_id.as_str());
                }
            }
        }
    }
}
