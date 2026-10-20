//! Genuine installed alternative Backs; fixed clocks/forecasts are conformance fixtures.
use conduit_core::*;
use conduit_planner::{resource_cost::*, *};
use conduit_plot::*;
use conduit_std_host::*;
use std::collections::BTreeMap;
pub struct Case {
    pub host: StdHost,
    pub previous: Plan,
    pub candidate: Plan,
    pub authored: ExpandedAuthoringPlot,
}
pub struct NoTimer;
impl TimerAdapter for NoTimer {
    fn wait(&mut self, _: std::time::Duration) {
        panic!("pure profile has no timer");
    }
}
pub struct Outputs(pub Vec<ExternalForeDelivery>);
impl Default for Outputs {
    fn default() -> Self {
        Self(Vec::with_capacity(1))
    }
}
impl ExternalForeOutputAdapter for Outputs {
    fn deliver(&mut self, value: ExternalForeDelivery) -> Result<(), String> {
        if self.0.len() == 1 {
            return Err("one-Value observer capacity exhausted".into());
        }
        self.0.push(value);
        Ok(())
    }
}
pub fn inputs(value: u64) -> [ExternalForeInput; 1] {
    [ExternalForeInput {
        front_port_id: "value".into(),
        track: ConnectionTrack::Payload,
        bytes: value.to_le_bytes().to_vec(),
    }]
}
pub fn case() -> Case {
    case_with_config(StdHostConfig {
        host_id: "host/replan-conformance".into(),
        boot_id: "boot/replan-conformance".into(),
        offer_generation: OfferGeneration(1),
    })
}
pub fn case_with_config(config: StdHostConfig) -> Case {
    case_with_profile(config, false)
}
/// `local` selects the genuine direct kernel Back instead of the scalar tape.
pub fn case_with_profile(config: StdHostConfig, local: bool) -> Case {
    let source =
        include_str!("../../../../architecture/plot/tests/fixtures/expression_baseline.conduit");
    let checked =
        check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new()).unwrap();
    let authored =
        expand_canonical_plot_for_authoring(&checked, "baseline-32", &ProfileCatalog::new())
            .unwrap();
    let gear = &authored.expanded.gears[0];
    let ConfigurationValue::Text(encoded) = &gear.configuration[0].value else {
        panic!("program")
    };
    let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
    assert_eq!(
        program.evaluate(&7u64.to_le_bytes()).unwrap(),
        231u64.to_le_bytes()
    );
    let generic =
        conduit_std_offers::pure_expression_std_offer(&program, PortTemporal::Value).unwrap();
    let tape = if local {
        conduit_std_offers::pure_expression_local_offer(&program).unwrap()
    } else {
        conduit_std_offers::pure_scalar_tape_std_offer(&program, PortTemporal::Value).unwrap()
    };
    let base = StdHost::new_with_composition(config, StdHostComposition::minimal());
    let mut ad = base.advertisement().clone();
    ad.capabilities.extend([generic.clone(), tape.clone()]);
    ad.capabilities
        .sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
    let host = StdHost::from_advertisement(ad).unwrap();
    let plan = |capability_id| {
        let placements = PlacementChoices {
            by_gear: BTreeMap::from([(
                gear.gear_id.clone(),
                PlacementChoice {
                    host_id: host.advertisement().host_id.clone(),
                    capability_id,
                },
            )]),
        };
        let boundaries = [
            (PortDirection::Input, "value"),
            (PortDirection::Output, "result"),
        ]
        .into_iter()
        .map(|(direction, port)| {
            (
                ForeBoundaryKey {
                    direction,
                    front_port_id: port.into(),
                    track: ConnectionTrack::Payload,
                },
                ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: 4096,
                },
            )
        })
        .collect();
        plan_expanded_authoring_with_options(
            &authored,
            &[host.advertisement().clone()],
            &placements,
            &["conduit.base/local@1".into()],
            PlanningOptions {
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
        .unwrap()
    };
    let previous = plan(generic.capability_id);
    let candidate = plan(tape.capability_id);
    Case {
        host,
        previous,
        candidate,
        authored,
    }
}
pub fn old_run(case: &mut Case) -> StdRunReport {
    let mut outputs = Outputs::default();
    let report = case
        .host
        .run_external_plot_to(
            case.previous.fragments[0].clone(),
            &inputs(7),
            &mut outputs,
            &mut Vec::new(),
            &mut NoTimer,
        )
        .unwrap();
    assert_eq!(outputs.0.len(), 1);
    assert_eq!(&*outputs.0[0].bytes, &231u64.to_le_bytes());
    report
}
fn work() -> ExactNumericWork {
    ExactNumericWork::new(1_000_000_000)
}
fn duration(ns: u64) -> CostDuration {
    CostDuration::admit(
        RationalQuantity::new(
            ExactRational::ratio(ns.into(), 1_000_000_000, &mut work()).unwrap(),
            Unit::resolve("s").unwrap(),
            QuantityRole::Linear,
        )
        .unwrap(),
        &mut work(),
    )
    .unwrap()
}
fn forecast(ns: u64) -> FulfillmentCostForecast {
    let range = |n| Some(CostDurationRange::admit(duration(n), duration(n), &mut work()).unwrap());
    FulfillmentCostForecast::sequential(
        CostForecastStages {
            setup: range(0),
            queue: range(0),
            service: range(ns),
            transfer: range(0),
        },
        &mut work(),
    )
    .unwrap()
}
pub fn with_request<R>(
    case: &mut Case,
    report: &StdRunReport,
    supplied: &[ExternalForeInput],
    old_ns: [u64; 3],
    now_tick: u64,
    run: impl FnOnce(&mut StdHost, StdReluctantReplanRequest<'_>) -> R,
) -> R {
    let ad = case.host.advertisement().clone();
    let clock = MonotonicClockIdentity::new(
        ad.host_id.clone(),
        ad.boot_id.clone(),
        "fixture/replan-clock".into(),
        TemporalScale::Nanoseconds,
        1,
        0,
    )
    .unwrap();
    let instant = |tick| MonotonicInstant::new(tick, clock.clone()).unwrap();
    let identity: ArtifactId = "fixture/replan-cost-profile".into();
    let revision = CostCalibrationRevision::new(&identity, 1).unwrap();
    let old = &case.previous.fragments[0].placements[0];
    let new = &case.candidate.fragments[0].placements[0];
    let source = |gear: &PlannedGear, sign, tick| {
        CostEvidenceSource::bind(
            &ad,
            ad.capabilities
                .iter()
                .find(|c| c.capability_id == gear.capability_id)
                .unwrap(),
            None,
            sign,
            revision,
            CostObservationWindow::admit(instant(tick), instant(10_000)).unwrap(),
        )
        .unwrap()
    };
    let stage_signs: Vec<SignId> = (0..5)
        .map(|i| format!("fixture/stage/{i}").into())
        .collect();
    let measurements: Vec<_> = (0..5)
        .map(|i| {
            CostMeasuredDuration::measure(
                &instant(100 + i * 10),
                &instant(101 + i * 10),
                &mut work(),
            )
            .unwrap()
        })
        .collect();
    let sources: Vec<_> = (0..5)
        .map(|i| {
            source(
                if i == 4 { old } else { new },
                &stage_signs[i],
                measurements[i].completed().ticks(),
            )
        })
        .collect();
    let stages = [
        ReplacementCostStage::Preparation,
        ReplacementCostStage::Transfer,
        ReplacementCostStage::Residency,
        ReplacementCostStage::Migration,
        ReplacementCostStage::Continuity,
    ];
    let parts: Vec<_> = (0..5)
        .map(|i| {
            ReplacementCostComponent::bind(
                stages[i],
                &sources[i],
                if i == 4 {
                    &case.previous
                } else {
                    &case.candidate
                },
                &measurements[i],
                None,
            )
            .unwrap()
        })
        .collect();
    let calibrations = [revision];
    let now = instant(now_tick);
    let context = CostEvaluationContext {
        host: &ad,
        calibrations: &calibrations,
        now: &now,
        execution_observations: &[],
    };
    let facts = ReplacementCostFacts::admit(
        &case.previous,
        old,
        &case.candidate,
        new,
        core::array::from_fn(|i| Some(&parts[i])),
        &[],
        std::slice::from_ref(&context),
        &mut work(),
    )
    .unwrap();
    let value = ValuePayload {
        value_kind: case.authored.expanded.gears[0].inputs[0].value_kind.clone(),
        encoded: 7u64.to_le_bytes().to_vec(),
    };
    let input = [CostInput {
        port: &case.authored.expanded.gears[0].inputs[0].port_id,
        value: &value,
    }];
    let demand = CostDemand::admit(&case.authored.expanded.gears[0], &input).unwrap();
    let signs: Vec<SignId> = (0..6)
        .map(|i| format!("fixture/forecast/{i}").into())
        .collect();
    let sampled: Vec<_> = (0..6)
        .map(|i| {
            source(
                if i % 2 == 0 { old } else { new },
                &signs[i],
                1000 + (i / 2) as u64 * 100,
            )
        })
        .collect();
    let models: Vec<_> = old_ns
        .into_iter()
        .flat_map(|n| [forecast(n), forecast(10)])
        .collect();
    let forecasts: Vec<_> = sampled
        .iter()
        .zip(&models)
        .map(|(s, m)| CostedForecast::bind(s, demand, m).unwrap())
        .collect();
    let observations: Vec<_> = forecasts
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| ReplanCostObservation {
            previous: &p[0],
            candidate: &p[1],
        })
        .collect();
    let policy = ReluctantReplanPolicy::admit(
        3,
        duration(100),
        duration(5000),
        duration(250),
        duration(1000),
        duration(10),
        1,
        &mut work(),
    )
    .unwrap();
    run(
        &mut case.host,
        StdReluctantReplanRequest {
            previous_report: report,
            policy,
            costs: &facts,
            observations: &observations,
            context: &context,
            inputs: supplied,
        },
    )
}
