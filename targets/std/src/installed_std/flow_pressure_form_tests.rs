use super::test_scalar_flow;
use conduit_core::{
    kind_id, BaseImplementationId, OfferGeneration, TerminalDisposition, SCALAR_ENCODED_LEN,
    SCALAR_INFO_ID,
};
use conduit_form::parse_with_startup;
use std::collections::BTreeMap;
use std::time::Duration;

struct Timer(Vec<Duration>);

impl crate::TimerAdapter for Timer {
    fn wait(&mut self, duration: Duration) {
        self.0.push(duration);
    }
}

#[test]
fn scalar_source_can_use_coalesce_latest_as_a_continuous_std_form() {
    let mut profile = super::test_support::test_catalog();
    let mut startup = profile.startup_catalog().unwrap();
    conduit_semantic_catalog::install_flow_pressure_kind(
        conduit_semantic_catalog::flow_coalesce_latest_contract(
            &kind_id(SCALAR_INFO_ID),
            SCALAR_ENCODED_LEN as u32,
        ),
        &mut startup,
        &mut profile,
    )
    .unwrap();

    let form = parse_with_startup(
        "form scalar_flow {\n    source: conduit-test/scalar-source\n    latest: flow/coalesce-latest\n    sink: conduit-test/scalar-sink(expected = 3)\n    source.value > latest.in\n    latest.out > sink.in\n}\n",
        &startup,
        &profile,
    )
    .unwrap();

    let host = crate::StdHost::new_with_config(crate::StdHostConfig {
        host_id: "flow-pressure-proof".into(),
        boot_id: "flow-pressure-proof/boot".into(),
        offer_generation: OfferGeneration(1),
    });
    let mut advertisement = host.advertisement().clone();
    advertisement.capabilities.extend([
        test_scalar_flow::source_offer(),
        test_scalar_flow::sink_offer(),
        conduit_std_offers::flow_coalesce_latest_std_offer(
            &kind_id(SCALAR_INFO_ID),
            SCALAR_ENCODED_LEN as u32,
        ),
    ]);
    advertisement
        .capabilities
        .sort_by(|left, right| left.capability_id.cmp(&right.capability_id));
    let hosts = [advertisement.clone()];
    let placements = conduit_planner::default_placements(&form, &hosts).unwrap();
    let plan = conduit_planner::plan_with_options(
        &form,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: SCALAR_ENCODED_LEN as u32,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .unwrap();

    assert!(plan.fragments[0].placements.iter().any(|placement| {
        placement.implementation_id.as_str()
            == conduit_std_offers::FLOW_COALESCE_LATEST_STD_IMPLEMENTATION
    }));
    let coalesced = plan.fragments[0]
        .connections
        .iter()
        .find(|connection| {
            connection.pressure_policy == conduit_core::DeliveryPressurePolicy::CoalesceLatest
        })
        .unwrap();
    assert_eq!(coalesced.byte_capacity, SCALAR_ENCODED_LEN as u32);

    let mut timer = Timer(Vec::with_capacity(3));
    let mut output = Vec::with_capacity(16_384);
    let report = super::run_fragment(
        super::InstalledRunHost {
            advertisement: &advertisement,
            playback: None,
            midi_input: None,
            midi_output: None,
            keyboard: None,
            local_model: None,
            vector_search: None,
            calendar: None,
            body_conversation_context: None,
        },
        &plan.fragments[0],
        0,
        &mut 0,
        &mut output,
        &mut timer,
        &crate::RunControl::default(),
    )
    .unwrap();

    assert_eq!(timer.0, [Duration::ZERO; 3]);
    assert!(matches!(
        report
            .observations
            .last()
            .map(|observation| &observation.kind),
        Some(conduit_core::ObservationKind::PlanTerminal {
            disposition: TerminalDisposition::Completed
        })
    ));
    assert_eq!(report.kernel.unwrap().post_play_start_allocations, 0);
}
