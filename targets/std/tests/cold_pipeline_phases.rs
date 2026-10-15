//! Linux repository-internal phase proof; actual kernel and explicit reference.
#![cfg(target_os = "linux")]
use conduit_core::{BaseImplementationId, ConfigurationValue, ConnectionTrack, PortDirection};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    PortableExpressionProgram, PreparedPortableExpressionEvaluator, ProfileCatalog, StartupCatalog,
};
use conduit_std_host::{
    ExternalForeDelivery, ExternalForeInput, ExternalForeOutputAdapter, KernelTimingPhase, StdHost,
    TimerAdapter,
};
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};
#[derive(Clone, Copy)]
struct Stamp {
    wall: Instant,
    cpu: u64,
}
impl Stamp {
    fn now() -> Self {
        let mut time = libc::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        // SAFETY: Linux's thread CPU clock writes to a valid initialized timespec.
        assert_eq!(
            unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut time) },
            0
        );
        Self {
            wall: Instant::now(),
            cpu: u64::try_from(time.tv_sec).unwrap() * 1_000_000_000
                + u64::try_from(time.tv_nsec).unwrap(),
        }
    }
    fn elapsed(self) -> (u128, u64) {
        let end = Self::now();
        (
            end.wall.duration_since(self.wall).as_nanos(),
            end.cpu - self.cpu,
        )
    }
}
#[derive(Default)]
struct Phases {
    marks: [Option<Stamp>; 3],
}
impl TimerAdapter for Phases {
    fn wait(&mut self, _: Duration) {
        panic!("pure fixture has no timer effect")
    }
    fn observe_kernel_phase(&mut self, phase: KernelTimingPhase) {
        let index = match phase {
            KernelTimingPhase::PreparationStarted => 0,
            KernelTimingPhase::PlayStarted => 1,
            KernelTimingPhase::PlayFinished => 2,
        };
        assert!(self.marks[index].replace(Stamp::now()).is_none());
    }
}
#[derive(Default)]
struct Output(Vec<Vec<u8>>);
impl ExternalForeOutputAdapter for Output {
    fn deliver(&mut self, value: ExternalForeDelivery) -> Result<(), String> {
        self.0.push(value.bytes);
        Ok(())
    }
}
fn source(nodes: usize) -> String {
    let mut text="plot double (\n >> value: U64\n result: U64 >>\n) = (. + .)\nplot pipeline (\n >> value: U64\n result: U64 >>\n) {\n".to_owned();
    for i in 0..nodes {
        text.push_str(&format!(" n{i}: double\n"));
    }
    text.push_str(" value >> n0.value\n");
    for i in 1..nodes {
        text.push_str(&format!(" n{}.result >> n{i}.value\n", i - 1));
    }
    text.push_str(&format!(" n{}.result >> result\n}}\n", nodes - 1));
    text
}
fn sample(nodes: usize, trial: usize, print: bool) {
    let text = source(nodes);
    let start = Stamp::now();
    let syntax = parse_syntax_document(&text);
    let parse = start.elapsed();
    let start = Stamp::now();
    let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
    let check = start.elapsed();
    let start = Stamp::now();
    let authored =
        expand_canonical_plot_for_authoring(&checked, "pipeline", &ProfileCatalog::new()).unwrap();
    let expand = start.elapsed();
    assert_eq!(authored.expanded.gears.len(), nodes);
    let start = Stamp::now();
    let mut ad = StdHost::new().advertisement().clone();
    for gear in &authored.expanded.gears {
        let ConfigurationValue::Text(encoded) = &gear.configuration[0].value else {
            panic!("program")
        };
        let p = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
        let offer =
            conduit_std_offers::pure_expression_std_offer(&p, gear.inputs[0].temporal).unwrap();
        if !ad
            .capabilities
            .iter()
            .any(|old| old.capability_id == offer.capability_id)
        {
            ad.capabilities.push(offer);
        }
    }
    ad.capabilities
        .sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
    let mut host = StdHost::from_advertisement(ad).unwrap();
    let hosts = [host.advertisement().clone()];
    let host_setup = start.elapsed();
    let start = Stamp::now();
    let placements =
        conduit_planner::default_expanded_placements(&authored.expanded, &hosts).unwrap();
    let boundaries = [
        (PortDirection::Input, "value"),
        (PortDirection::Output, "result"),
    ]
    .into_iter()
    .map(|(direction, name)| {
        (
            conduit_planner::ForeBoundaryKey {
                direction,
                front_port_id: name.into(),
                track: ConnectionTrack::Payload,
            },
            conduit_planner::ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: 8,
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
            connection_byte_capacity: 8,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )
    .unwrap();
    let planning = start.elapsed();
    let start = Stamp::now();
    let reference = format!(
        "plot reference (\n >> value: U64\n result: U64 >>\n) = (. * {})\n",
        1u64 << nodes
    );
    let c =
        check_syntax_document(&parse_syntax_document(&reference), &StartupCatalog::new()).unwrap();
    let a = expand_canonical_plot_for_authoring(&c, "reference", &ProfileCatalog::new()).unwrap();
    let ConfigurationValue::Text(encoded) = &a.expanded.gears[0].configuration[0].value else {
        panic!("program")
    };
    let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
    let reference_source_prepare = start.elapsed();
    let start = Stamp::now();
    let mut evaluator = PreparedPortableExpressionEvaluator::new(&program).unwrap();
    let evaluator_prepare = start.elapsed();
    let input = 7u64.to_le_bytes();
    let expected = 7u64.checked_mul(1u64 << nodes).unwrap().to_le_bytes();
    assert_eq!(evaluator.evaluate(&input).unwrap(), expected);
    let start = Stamp::now();
    for _ in 0..100_000 {
        assert_eq!(
            evaluator.evaluate(std::hint::black_box(&input)).unwrap(),
            expected
        );
    }
    let evaluator_warm = start.elapsed();
    let mut output = Output(Vec::with_capacity(1));
    let mut phases = Phases::default();
    let start = Stamp::now();
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
            &mut phases,
        )
        .unwrap();
    let cold_total = start.elapsed();
    assert_eq!(output.0, [expected]);
    let [Some(prep), Some(play), Some(end)] = phases.marks else {
        panic!("phase capture")
    };
    let kernel = report.kernel.unwrap();
    let mut warm_output = Output(Vec::with_capacity(1));
    let mut warm_phases = Phases::default();
    let start = Stamp::now();
    let warm_report = host
        .run_external_plot_to(
            plan.fragments[0].clone(),
            &[ExternalForeInput {
                front_port_id: "value".into(),
                track: ConnectionTrack::Payload,
                bytes: input.to_vec(),
            }],
            &mut warm_output,
            &mut Vec::new(),
            &mut warm_phases,
        )
        .unwrap();
    let warm_total = start.elapsed();
    assert_eq!(warm_output.0, [expected]);
    let [Some(warm_prep), Some(warm_play), Some(warm_end)] = warm_phases.marks else {
        panic!("warm phases")
    };
    let warm_kernel = warm_report.kernel.unwrap();
    assert_eq!(
        kernel
            .kernel_sign
            .iter()
            .map(|e| e.kind)
            .collect::<Vec<_>>(),
        warm_kernel
            .kernel_sign
            .iter()
            .map(|e| e.kind)
            .collect::<Vec<_>>()
    );
    if print {
        let source = std::env::var("CONDUIT_COLD_PIPELINE_SOURCE").unwrap();
        assert!(source.len() == 40 && source.bytes().all(|b| b.is_ascii_hexdigit()));
        println!(
            "COLD_PIPELINE_ROW {}",
            serde_json::json!({"source":source,"nodes":nodes,"trial":trial,"source_document_id":plan.source_document_id.as_str(),"checked_plot_id":plan.checked_plot_id.as_str(),"expanded_plot_id":plan.expanded_plot_id.as_str(),"plan_id":plan.plan_id.as_str(),"cold_play_id":kernel.active_play_id.as_str(),"warm_play_id":warm_kernel.active_play_id.as_str(),"providers":plan.fragments[0].placements.iter().map(|p| (p.implementation_id.as_str(),p.artifact_id.as_str())).collect::<Vec<_>>(),"clock":"monotonic wall and Linux thread CPU nanoseconds","parse":parse,"check":check,"expand":expand,"host_offer_setup":host_setup,"planning":planning,"reference_source_prepare":reference_source_prepare,"evaluator_prepare":evaluator_prepare,"evaluator_warm_100000":evaluator_warm,"cold_installed_prepare":(play.wall.duration_since(prep.wall).as_nanos(),play.cpu-prep.cpu),"cold_play":(end.wall.duration_since(play.wall).as_nanos(),end.cpu-play.cpu),"warm_installed_prepare":(warm_play.wall.duration_since(warm_prep.wall).as_nanos(),warm_play.cpu-warm_prep.cpu),"warm_play":(warm_end.wall.duration_since(warm_play.wall).as_nanos(),warm_end.cpu-warm_play.cpu),"cold_total_host_run":cold_total,"warm_total_host_run":warm_total,"cold_signs":kernel.kernel_sign.len(),"warm_signs":warm_kernel.kernel_sign.len(),"input":input,"output":expected,"queue_item_capacity":1,"queue_byte_capacity":8,"warmup":"warm Play follows one measured and correctness-checked cold Play; Back preparation repeats each Play"})
        );
    }
}
#[test]
fn checked_pipeline_has_exact_one_three_eight_thirty_two_gears_and_outputs() {
    for n in [1, 3, 8, 32] {
        sample(n, 0, false);
    }
}
#[test]
#[ignore = "manual optimized phase samples; report only, no latency threshold"]
fn measure_cold_and_warm_pipeline_phases() {
    for trial in 0..8 {
        for nodes in [1, 3, 8, 32] {
            sample(nodes, trial, true);
        }
    }
}
