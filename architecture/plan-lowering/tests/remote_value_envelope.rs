//! A sealed semantic envelope survives lowering and protects remote ingress.
use conduit_core::*;
use conduit_kernel::{
    scheduler::{
        FixedScheduler, RemoteIngressOutcome, SchedulerError, StepBack, StepInputBytes, StepIo,
        StepOutcome,
    },
    FixedRoutes, FixedSignLog, FixedValueStore, ValueStorage,
};
use conduit_plan_lowering::lowering::{
    lower_plan_fragment_from_plan, ForeValueRefusal, FIXED_KERNEL_STORAGE_PORTS_PER_NODE as PORTS,
};
#[path = "../../core/tests/common/sealed_state.rs"]
mod common;
#[derive(Clone, Copy)]
struct Waiting;
impl StepBack<PORTS> for Waiting {
    fn step(&mut self, _: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        StepOutcome::Await
    }
}
#[test]
fn sealed_input_value_envelope_is_preserved_at_kernel_ingress() {
    for policy in [
        DeliveryPressurePolicy::PreserveOrder,
        DeliveryPressurePolicy::CoalesceLatest,
    ] {
        let mut fragment = common::fragment();
        fragment.states.clear();
        fragment.placements[0].outputs.clear();
        fragment.placements[0].inputs[0].value_kind = kind_id("value/text");
        fragment.placements[0].limits.max_queue_bytes = 8;
        let contract = CheckedValueContract::new(kind_id("value/text"), 2, vec![]).unwrap();
        fragment.placements[0]
            .semantic_contract
            .laws
            .push(KindSemanticLaw::ValueContracts(vec![FrontValueContract {
                location: FrontValueLocation::Input(port_id("next")),
                contract: contract.clone(),
            }]));
        fragment.fore_ports.push(PlannedForePort {
            front_port_id: port_id("input"),
            direction: PortDirection::Input,
            placement_id: PlacementId::from("placement"),
            gear_port_id: port_id("next"),
            value_kind: kind_id("value/text"),
            value_contract: Some(contract),
            abnormal_kind: None,
            track: ConnectionTrack::Payload,
            temporal: PortTemporal::Value,
            pressure_policy: policy,
            item_capacity: 1,
            byte_capacity: 8,
            selected_line: None,
        });
        fragment.sign_storage_budget =
            mandatory_sign_storage_requirement(&fragment.expected_sign).unwrap();
        let plan = common::seal(fragment);
        assert!(verify_plan(&plan));
        let (lowered, activations) =
            lower_plan_fragment_from_plan(&plan, &plan.fragments[0].fragment_id).unwrap();
        assert!(activations.entries.is_empty());
        assert!(lowered.states.is_empty());
        assert!(lowered.host_calls.is_empty());
        let input = &lowered.fore_ports[0];
        assert_eq!(input.validate_value(b"ok"), Ok(()));
        assert_eq!(
            input.validate_value(b"big"),
            Err(ForeValueRefusal::Constraint(
                ValueConstraintRefusal::Oversize {
                    actual: 3,
                    maximum: 2
                }
            ))
        );
        assert_eq!(lowered.cords[0].spec.maximum_value_bytes, 2);
        assert_eq!(lowered.cords[0].spec.byte_capacity, 8);
        let mut routes = FixedRoutes::<1, 0>::new(PORTS as u16);
        routes.seal().unwrap();
        let mut scheduler = FixedScheduler::<_, _, _, 1, 1, PORTS, 1, 1, 0>::new(
            lowered.node_specs.try_into().unwrap(),
            [lowered.cords[0].spec],
            routes,
            [Waiting],
            FixedValueStore::<2, 8>::new(16).unwrap(),
            FixedSignLog::<32>::new_with_remote_storage(
                32 * core::mem::size_of::<conduit_kernel::KernelEvent>() as u32,
                32,
                conduit_kernel::remote_sign_storage_bytes(32).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        // Kernel admission independently enforces the lowered immutable bound.
        for sequence in 0..2 {
            let signs = scheduler.signs().events().count();
            let usage = scheduler.cord_usage(input.cord).unwrap();
            let items = scheduler.values().used_items();
            assert_eq!(
                scheduler.admit_remote_input(input.endpoint, input.cord, sequence, b"big"),
                Err(SchedulerError::SemanticValueBoundExceeded)
            );
            assert_eq!(scheduler.signs().events().count(), signs);
            assert_eq!(scheduler.cord_usage(input.cord).unwrap(), usage);
            assert_eq!(scheduler.values().used_items(), items);
            let admitted =
                scheduler.admit_remote_input(input.endpoint, input.cord, sequence, b"ok");
            if sequence == 0 || policy == DeliveryPressurePolicy::CoalesceLatest {
                assert_eq!(admitted, Ok(RemoteIngressOutcome::Accepted { sequence }));
            } else {
                assert_eq!(admitted, Ok(RemoteIngressOutcome::Full { sequence }));
            }
        }
        assert_eq!(scheduler.cord_usage(input.cord).unwrap(), (1, 2));
        assert_eq!(scheduler.values().used_items(), 1);
        scheduler.cancel().unwrap();
        assert_eq!(scheduler.values().used_items(), 0);
    }
}
