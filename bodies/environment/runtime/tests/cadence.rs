//! Check the finite authored overnight cadence through ordinary planning.
#[allow(dead_code)]
#[path = "support/plan.rs"]
mod plan;
#[allow(dead_code)]
#[path = "support/pulse.rs"]
mod pulse;
// The pulse fixture's unused assigned branch refers to these shared modules.
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[allow(dead_code)]
#[path = "support/assignment.rs"]
mod assignment;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
#[test]
fn overnight_workload_preserves_typed_failures_and_finite_cadence() {
    let contract =
        conduit_environment_runtime::observation::ObservationContract::prepare().unwrap();
    let (ready, effects) = pulse::ready(0);
    for (count, period) in [(0, 60000), (4, 3000), (960, 60000)] {
        let (plan, lowered) = plan::prepare_with_cadence(
            &ready,
            contract.kind().clone(),
            conduit_time::tick_semantic_contract(),
            count,
            period,
        );
        assert_eq!(lowered.nodes.len(), 15);
        assert_eq!(lowered.cords.len(), 14);
        let tick = plan.fragments[0]
            .placements
            .iter()
            .find(|g| g.kind_id.as_str() == "time/tick")
            .unwrap();
        assert_eq!(
            conduit_time::parse_tick_configuration(&tick.configuration).unwrap(),
            conduit_time::TickConfiguration {
                count,
                period_ms: period
            }
        );
        assert_eq!(tick.outputs.len(), 1);
        assert_eq!(
            tick.outputs[0].temporal,
            conduit_core::PortTemporal::Flow { closes: true }
        );
        assert_eq!(lowered.host_calls.len(), 6); // native pulse, wait, four selectors
        assert_eq!(effects.get(), 0); // checking and planning have no physical effects
    }
}
