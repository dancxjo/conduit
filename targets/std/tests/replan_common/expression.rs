//! Genuine installed alternative Backs; fixed clocks/forecasts are conformance fixtures.
use conduit_core::*;
use conduit_planner::*;
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
