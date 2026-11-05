//! A private-work class uses ordinary finite resource selection and refusal.
use super::*;
use conduit_core::COOPERATIVE_STEP_WORK_CLASS_ID as WORK;

#[test]
fn cooperative_work_is_unavailable_until_a_real_host_pool_funds_it() {
    let plot = plot();
    for capacity in [None, Some(2), Some(3)] {
        let mut host = host();
        host.capabilities[0]
            .resource_requirements
            .push(conduit_core::resource_requirement(WORK, 3));
        if let Some(units) = capacity {
            host.resources
                .push(conduit_core::resource_offer("test/step-work", WORK, units));
            host.resources.sort_by(|a, b| a.pool_id.cmp(&b.pool_id));
        }
        let choices = default_placements(&plot, std::slice::from_ref(&host)).unwrap();
        let result = plan(
            &plot,
            &[host],
            &choices,
            &[BaseImplementationId::from("conduit.base/local@1")],
        );
        match capacity {
            None => assert!(matches!(result, Err(PlannerError::UnavailableResource(_)))),
            Some(2) => assert!(
                matches!(result, Err(PlannerError::ResourceCapacityExceeded(_))),
                "{result:?}"
            ),
            Some(3) => {
                let plan = result.unwrap();
                let fragment = &plan.fragments[0];
                let placement = fragment
                    .placements
                    .iter()
                    .find(|p| p.resources.iter().any(|r| r.class_id.as_str() == WORK))
                    .unwrap();
                let reservation = placement
                    .resources
                    .iter()
                    .find(|r| r.class_id.as_str() == WORK)
                    .unwrap();
                assert_eq!(reservation.pool_id.as_str(), "test/step-work");
                assert_eq!(reservation.units, 3);
                let (lowered, _) = conduit_plan_lowering::lowering::lower_plan_fragment_from_plan(
                    &plan,
                    &fragment.fragment_id,
                )
                .unwrap();
                let index = fragment
                    .placements
                    .iter()
                    .position(|p| p.placement_id == placement.placement_id)
                    .unwrap();
                let base = 1
                    + placement.inputs.len()
                    + placement.outputs.len()
                    + placement.host_calls.len();
                assert_eq!(
                    usize::from(lowered.nodes[index].maximum_step_fuel),
                    base + 3
                );
                assert_eq!(
                    lowered.nodes[index].maximum_step_fuel,
                    lowered.node_specs[index].maximum_step_fuel
                );
            }
            _ => unreachable!(),
        }
    }
}
