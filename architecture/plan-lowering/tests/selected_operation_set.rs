//! Opaque operation possession plus one actual capacity-one reservation.
extern crate alloc;
use conduit_core::*;
use conduit_kernel::{HostCallId, NodeId, PortId as NumericPortId, RouteRange};
use conduit_plan_lowering::{
    lowering::{lower_plan_fragment, LoweredPlanFragment, LoweredRoute},
    selected_operation::SelectedOperationPlan,
    selected_operation_set::*,
};
#[path = "selected_operation_set/fixture.rs"]
mod fixture;
#[path = "../../../targets/conduitos/src/machine_membrane/selection_fixture.rs"]
mod selection_fixture;
use fixture::*;

#[test]
fn different_targets_share_exactly_one_capacity_one_reservation() {
    let f = Fixture::new();
    let a = operation(f.scopes[0].clone());
    let r = operation(f.scopes[1].clone());
    let before = [a.inspection().clone(), r.inspection().clone()];
    let admitted = f.owner.admissions().to_vec();
    let result = f.bind(&[&r, &a]);
    assert!(result.is_err()); // Actual operations must follow trusted mapping.
    let result = f.bind(&[&a, &r]).unwrap();
    assert_eq!(result.node, NodeId(0));
    assert_eq!(result.routes[0].unwrap().call, HostCallId(0));
    assert_eq!(result.routes[1].unwrap().call, HostCallId(1));
    assert_eq!(result.routes[1].unwrap().input, NumericPortId(1));
    assert_eq!(result.routes[1].unwrap().output, NumericPortId(1));
    assert_eq!(f.owner.host().resources[0].capacity_units, 1);
    assert_eq!(f.owner.admissions().len(), 1);
    assert_eq!(f.owner.admissions()[0].items[0].binding.units, 1);
    assert_eq!([a.inspection().clone(), r.inspection().clone()], before);
    assert_eq!(f.owner.admissions(), admitted);
    // Trusted order can differ from lowering order; actual IDs remain exact.
    let mut calls = f.calls();
    calls.swap(0, 1);
    let reversed = f.bind_calls(&[&r, &a], &calls).unwrap();
    assert_eq!(reversed.routes[0].unwrap().call, HostCallId(1));
}
#[test]
fn altered_lowering_including_routes_is_rejected() {
    for edit in 0..3 {
        let mut f = Fixture::new();
        let a = operation(f.scopes[0].clone());
        let r = operation(f.scopes[1].clone());
        match edit {
            0 => f.lowered.host_calls[0].call = HostCallId(3),
            1 => f.lowered.node_specs[0].maximum_step_fuel += 1,
            _ => f.lowered.routes.push(LoweredRoute {
                source_node: NodeId(0),
                source_port: NumericPortId(0),
                range: RouteRange { start: 0, len: 0 },
                targets: vec![],
            }),
        }
        assert_eq!(
            f.bind(&[&a, &r]),
            Err(SelectedOperationSetRefusal::WrongBinding)
        );
    }
}
#[test]
fn absent_duplicate_wrong_target_ports_and_bounds_refuse() {
    let f = Fixture::new();
    let a = operation(f.scopes[0].clone());
    let r = operation(f.scopes[1].clone());
    assert_eq!(f.bind(&[&a]), Err(SelectedOperationSetRefusal::Bounds));
    for edit in 0..9 {
        let mut calls = f.calls();
        let absent = HostCallContractId::from("host/absent");
        let wrong_kind = KindId::from("wrong/target");
        let wrong_port = conduit_core::PortId::from("missing");
        match edit {
            0 => calls[1].contract = calls[0].contract,
            1 => calls[1].contract = &absent,
            2 => calls[1].target_kind = &wrong_kind,
            3 => calls[1].input_port = calls[0].input_port,
            4 => calls[1].output_port = &wrong_port,
            5 => calls[1].input_bytes += 1,
            6 => calls[1].output_bytes += 1,
            7 => calls[1].work_units += 1,
            _ => calls[1].maximum_operations += 1,
        }
        assert!(f.bind_calls(&[&a, &r], &calls).is_err(), "edit {edit}");
    }
    assert!(f.bind(&[&a, &a]).is_err());
}
#[test]
fn foreign_actual_scopes_cannot_replace_selected_authority() {
    let f = Fixture::new();
    let a = operation(f.scopes[0].clone());
    for edit in 0..12 {
        let mut scope = f.scopes[1].clone();
        match edit {
            0 => scope.boot_id = "other/boot".into(),
            1 => scope.base_provider_generation += 1,
            2 => scope.authority_grant_id = "other/grant".into(),
            3 => scope.resource_pool_id = "other/pool".into(),
            4 => scope.resource_generation_id = ResourceGenerationId("other/generation".into()),
            5 => scope.envelope_id = "other/envelope".into(),
            6 => scope.active_play_id = "other/play".into(),
            7 => scope.capability_id = "other/capability".into(),
            8 => scope.implementation_id = "other/implementation".into(),
            9 => scope.base_instance_id = "other/provider".into(),
            10 => scope.authority_contract_id = "other/authority".into(),
            _ => scope.subject_kind = "other/subject".into(),
        }
        let foreign = operation(scope);
        assert!(f.bind(&[&a, &foreign]).is_err(), "edit {edit}");
    }
}
#[test]
fn revoked_inflight_spent_and_unauthorized_operations_refuse_without_refunding() {
    let f = Fixture::new();
    let a = operation(f.scopes[0].clone());
    let mut r = operation(f.scopes[1].clone());
    r.revoke().unwrap();
    assert_eq!(
        f.bind(&[&a, &r]),
        Err(SelectedOperationSetRefusal::Possession)
    );
    let mut r = operation(f.scopes[1].clone());
    let lease = r.authorize().unwrap();
    assert_eq!(
        f.bind(&[&a, &r]),
        Err(SelectedOperationSetRefusal::Possession)
    );
    r.complete(lease, 8).unwrap();
    assert_eq!(
        f.bind(&[&a, &r]),
        Err(SelectedOperationSetRefusal::Possession)
    );
    for _ in 1..8 {
        let lease = r.authorize().unwrap();
        r.complete(lease, 8).unwrap();
    }
    let before = r.inspection().clone();
    assert_eq!(
        f.bind(&[&a, &r]),
        Err(SelectedOperationSetRefusal::Possession)
    );
    assert_eq!(r.inspection(), &before);
    assert!(matches!(
        operation_with_claim(f.scopes[1].clone(), |claim| claim.subject_kind =
            "unauthorized".into()),
        Err(BaseCapabilityRefusal::WrongScope)
    ));
}
#[test]
fn absent_or_wrong_actual_ledger_is_rejected() {
    for edit in 0..3 {
        let mut f = Fixture::new();
        let a = operation(f.scopes[0].clone());
        let r = operation(f.scopes[1].clone());
        match edit {
            0 => {
                f.owner.release(&f.fragment.plan_id, &f.placement).unwrap();
            }
            1 => {
                let mut host = f.owner.host().clone();
                host.boot_id = "wrong/boot".into();
                f.owner = ResourceAdmissionOwner::new(host);
            }
            _ => {
                f.resource.units = 2;
            }
        }
        assert!(f.bind(&[&a, &r]).is_err());
    }
}

#[test]
fn narrower_original_claim_is_not_proved_by_a_larger_inspection_scope() {
    let f = Fixture::new();
    let a = operation(f.scopes[0].clone());
    for narrow_work in [false, true] {
        let r = operation_with_claim(f.scopes[1].clone(), |claim| {
            if narrow_work {
                claim.work_units = 1;
            } else {
                claim.parameter_bytes = 1;
            }
        })
        .ok()
        .unwrap();
        assert_eq!(f.bind(&[&a, &r]), Err(SelectedOperationSetRefusal::Bounds));
    }
}
#[test]
fn different_reserved_binding_in_same_current_host_ledger_refuses() {
    let mut f = Fixture::new();
    let a = operation(f.scopes[0].clone());
    let r = operation(f.scopes[1].clone());
    let mut host = f.owner.host().clone();
    host.resources[0].pool_id = "other/residence".into();
    let mut binding = f.resource.clone();
    binding.pool_id = host.resources[0].pool_id.clone();
    let observation = ResourceObservation {
        host_id: host.host_id.clone(),
        boot_id: host.boot_id.clone(),
        offer_generation: host.offer_generation,
        pool_id: binding.pool_id.clone(),
        class_id: binding.class_id.clone(),
        health: ResourceHealth::Ready,
        unreserved_units: 1,
        utilized_units: 0,
        sign_id: "other/current".into(),
    };
    f.owner = ResourceAdmissionOwner::new(host);
    f.owner
        .admit(
            ResourceAdmissionRequest {
                plan_id: f.fragment.plan_id.clone(),
                placement_id: f.placement.clone(),
                host_id: f.fragment.host_id.clone(),
                boot_id: f.fragment.boot_id.clone(),
                offer_generation: f.fragment.offer_generation,
                items: vec![ResourceAdmissionItem {
                    requirement: f.requirement.clone(),
                    binding,
                }],
            },
            &[observation],
        )
        .unwrap();
    assert_eq!(
        f.bind(&[&a, &r]),
        Err(SelectedOperationSetRefusal::Reservation)
    );
}
#[test]
fn expected_source_and_current_play_cannot_be_supplied_by_incoming_plan() {
    for edit in 0..4 {
        let mut f = Fixture::new();
        let a = operation(f.scopes[0].clone());
        let r = operation(f.scopes[1].clone());
        match edit {
            0 => f.kind.kind_id = "different/service".into(),
            1 => f.kind.inputs[0].value_kind = "different/info".into(),
            2 => f.active.play_sequence += 1,
            _ => {
                f.active = bind_active_play(
                    &f.fragment.plan_id,
                    &f.fragment.host_id,
                    &f.fragment.boot_id,
                    1,
                )
            }
        }
        assert!(f.bind(&[&a, &r]).is_err());
    }
}

#[test]
fn resealed_plan_and_actual_offer_limits_must_match_retained_kind() {
    fn refresh(f: &mut Fixture, host: HostAdvertisement) {
        f.fragment = seal_plan(
            PlotIdentity {
                source_document_id: f.fragment.source_document_id.clone(),
                checked_plot_id: f.fragment.checked_plot_id.clone(),
                expanded_plot_id: f.fragment.expanded_plot_id.clone(),
            },
            vec![f.fragment.clone()],
        )
        .fragments
        .remove(0);
        f.active = bind_active_play(
            &f.fragment.plan_id,
            &f.fragment.host_id,
            &f.fragment.boot_id,
            0,
        );
        f.lowered = lower_plan_fragment(&f.fragment).unwrap();
        for scope in &mut f.scopes {
            scope.plan_id = f.fragment.plan_id.clone();
            scope.active_play_id = f.active.active_play_id.clone();
        }
        let observation = ResourceObservation {
            host_id: host.host_id.clone(),
            boot_id: host.boot_id.clone(),
            offer_generation: host.offer_generation,
            pool_id: f.resource.pool_id.clone(),
            class_id: f.resource.class_id.clone(),
            health: ResourceHealth::Ready,
            unreserved_units: 1,
            utilized_units: 0,
            sign_id: "current".into(),
        };
        f.owner = ResourceAdmissionOwner::new(host);
        f.owner
            .admit_planned_placement(
                f.fragment.plan_id.clone(),
                &f.fragment.placements[0],
                &[observation],
            )
            .unwrap();
    }
    for change_offer in [false, true] {
        for field in 0..3 {
            let mut f = Fixture::new();
            let mut host = f.owner.host().clone();
            let limits = if change_offer {
                &mut host.capabilities[0].limits
            } else {
                &mut f.fragment.placements[0].limits
            };
            match field {
                0 => limits.max_active_instances += 1,
                1 => limits.max_queue_items += 1,
                _ => limits.max_queue_bytes += 1,
            }
            // Every other identity, lowering, original operation and actual
            // capacity-one reservation is refreshed, so stale scope cannot
            // substitute for the limits check this test intends to exercise.
            refresh(&mut f, host);
            let a = operation(f.scopes[0].clone());
            let r = operation(f.scopes[1].clone());
            assert_eq!(
                f.bind(&[&a, &r]),
                Err(if change_offer {
                    SelectedOperationSetRefusal::Reservation
                } else {
                    SelectedOperationSetRefusal::WrongBinding
                }),
                "offer={change_offer}, field={field}"
            );
            f.fragment.placements[0].limits = f.kind.limits.clone();
            let mut host = f.owner.host().clone();
            host.capabilities[0].limits = f.kind.limits.clone();
            refresh(&mut f, host);
            let a = operation(f.scopes[0].clone());
            let r = operation(f.scopes[1].clone());
            assert!(f.bind(&[&a, &r]).is_ok());
        }
    }
}
