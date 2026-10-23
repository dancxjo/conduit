//! Canonical protected selection only; current lowerers still refuse execution.
extern crate alloc;
use conduit_core::*;
#[allow(dead_code)]
#[path = "../../plan-lowering/tests/common/execution_regions.rs"]
mod fixture;
use conduit_planner::execution_regions::*;

fn input() -> (Plan, HostAdvertisement, Vec<ProtectedRegionSelection>) {
    let initial = fixture::plan(1);
    let (mut host, _) = fixture::lane_host(&initial, 1);
    let mut fragment = initial.fragments[0].clone();
    let old = fragment.execution_regions.remove(0);
    let mut lane = old.lane_resource;
    lane.compute.as_mut().unwrap().architecture_base_kind = ArchitectureBaseKind::BareMetal;
    let offered = host.resources[0].compute.as_mut().unwrap();
    offered.architecture_base_kind = ArchitectureBaseKind::BareMetal;
    offered.protected_preemption = Some(ProtectedPreemptionOffer {
        execution_profile_id: "fixture/protected@1".into(),
        policy: ProtectedEntryBudget {
            accounting: ProtectedEntryAccounting::SourceInterruptsV1,
            quantum_interrupts: 2,
            total_interrupts: 8,
        },
        context_bytes: 128,
        context_alignment: 16,
    });
    host.bases.push(BaseProviderAdvertisement {
        base_id: "base".into(),
        provider_instance_id: "base/instance".into(),
        provider_generation: 1,
        implementation_id: "base/kernel@1".into(),
        mechanism_family: "base/kernel".into(),
        enforcement_class: BaseEnforcementClass::ConduitOsKernelEnforced,
        lifecycle: BaseLifecycle::Ready,
        capability_ids: vec![],
        resource_pool_ids: vec!["lanes".into()],
    });
    let context_binding = ResourceBinding {
        pool_id: "continuation".into(),
        class_id: RUNTIME_MEMORY_RESOURCE_CLASS.into(),
        units: 128,
        compute: None,
        protected: None,
        content: None,
    };
    host.resources.push(ResourceOffer {
        pool_id: context_binding.pool_id.clone(),
        class_id: context_binding.class_id.clone(),
        capacity_units: 128,
        compute: None,
        content: None,
    });
    let selection = ProtectedRegionSelection {
        region_id: old.region_id,
        placements: old.admitted_placements,
        execution_profile_id: "fixture/protected@1".into(),
        lane_resource: lane,
        lane_base_id: old.lane_base_id,
        context_binding,
    };
    (fixture::common::seal(fragment), host, vec![selection])
}
fn select() -> Plan {
    let (plan, host, selections) = input();
    seal_protected_preemptible_execution(plan, &host, selections).unwrap()
}
#[test]
fn selected_policy_is_exact_offer_truth_and_changes_only_realization_identity() {
    let (original, host, selections) = input();
    let selected =
        seal_protected_preemptible_execution(original.clone(), &host, selections).unwrap();
    assert!(verify_plan(&selected));
    assert_ne!(selected.plan_id, original.plan_id);
    assert_eq!(selected.source_document_id, original.source_document_id);
    assert_eq!(selected.checked_plot_id, original.checked_plot_id);
    assert_eq!(selected.expanded_plot_id, original.expanded_plot_id);
    assert_eq!(
        selected.fragments[0].placements,
        original.fragments[0].placements
    );
    assert_eq!(
        selected.fragments[0].connections,
        original.fragments[0].connections
    );
    let region = &selected.fragments[0].execution_regions[0];
    assert!(region.preemption_required && region.isolation_required);
    let ExecutionScheduling::ProtectedPreemptibleStep { policy, context } = &region.scheduling
    else {
        panic!("protected profile")
    };
    let offered = host.resources[0]
        .compute
        .as_ref()
        .unwrap()
        .protected_preemption
        .as_ref()
        .unwrap();
    assert_eq!(policy, &offered.policy);
    assert_eq!(
        (context.bytes, context.alignment),
        (offered.context_bytes, offered.context_alignment)
    );
    for quantum in [1, 3] {
        let (plan, mut host, selections) = input();
        host.resources[0]
            .compute
            .as_mut()
            .unwrap()
            .protected_preemption
            .as_mut()
            .unwrap()
            .policy
            .quantum_interrupts = quantum;
        let changed = seal_protected_preemptible_execution(plan, &host, selections).unwrap();
        assert_ne!(changed.plan_id, selected.plan_id);
    }
}
#[test]
fn stale_host_boot_and_offer_generation_refuse() {
    for changed in 0..3 {
        let (plan, mut host, selections) = input();
        match changed {
            0 => host.host_id = "foreign".into(),
            1 => host.boot_id = "foreign".into(),
            _ => host.offer_generation = OfferGeneration(2),
        }
        assert_eq!(
            seal_protected_preemptible_execution(plan, &host, selections),
            Err(ProtectedRegionPlanningError::StaleHost)
        );
    }
}
#[test]
fn absence_wrong_profile_and_invalid_offered_policy_refuse() {
    for changed in 0..4 {
        let (plan, mut host, selections) = input();
        let offered = &mut host.resources[0]
            .compute
            .as_mut()
            .unwrap()
            .protected_preemption;
        match changed {
            0 => *offered = None,
            1 => offered.as_mut().unwrap().execution_profile_id = "foreign".into(),
            2 => offered.as_mut().unwrap().policy.quantum_interrupts = 0,
            _ => offered.as_mut().unwrap().policy.quantum_interrupts = 9,
        }
        assert_eq!(
            seal_protected_preemptible_execution(plan, &host, selections),
            Err(ProtectedRegionPlanningError::UnsupportedProfile)
        );
    }
}
#[test]
fn pool_capacity_binding_and_base_mismatches_refuse() {
    for changed in 0..8 {
        let (plan, mut host, mut selections) = input();
        let expected = match changed {
            0 => {
                host.resources[1].capacity_units = 127;
                ProtectedRegionPlanningError::CapacityExceeded
            }
            1 => {
                selections[0].context_binding.units = 127;
                ProtectedRegionPlanningError::BindingMismatch
            }
            2 => {
                let duplicate = host.resources[0].clone();
                host.resources.push(duplicate);
                ProtectedRegionPlanningError::AmbiguousPool
            }
            3 => {
                host.bases.clear();
                ProtectedRegionPlanningError::InvalidBase
            }
            4 => {
                host.bases[0].lifecycle = BaseLifecycle::Lost;
                ProtectedRegionPlanningError::InvalidBase
            }
            5 => {
                host.bases[0].enforcement_class = BaseEnforcementClass::Cooperative;
                ProtectedRegionPlanningError::InvalidBase
            }
            6 => {
                selections[0].lane_resource.units = 2;
                ProtectedRegionPlanningError::BindingMismatch
            }
            _ => {
                host.resources[0].capacity_units = 0;
                ProtectedRegionPlanningError::CapacityExceeded
            }
        };
        assert_eq!(
            seal_protected_preemptible_execution(plan, &host, selections),
            Err(expected),
            "mutation {changed}"
        );
    }
}
#[test]
fn flags_cannot_be_removed_from_a_resealed_protected_plan() {
    for preemption in [false, true] {
        let mut fragment = select().fragments.remove(0);
        if preemption {
            fragment.execution_regions[0].preemption_required = false;
        } else {
            fragment.execution_regions[0].isolation_required = false;
        }
        let altered = fixture::common::seal(fragment);
        assert!(!verify_plan(&altered));
    }
}
#[test]
fn current_lowering_explicitly_refuses_new_profile() {
    let plan = select();
    let result = conduit_plan_lowering::lowering::lower_plan_fragment_from_plan(
        &plan,
        &plan.fragments[0].fragment_id,
    );
    assert!(matches!(
        result,
        Err(conduit_plan_lowering::lowering::LoweringError::UnsupportedExecutionRegion(_))
    ));
}

#[test]
fn offered_context_extent_alignment_and_pool_are_plan_identity() {
    let baseline = select();
    for mutation in 0..3 {
        let (original, mut host, mut selections) = input();
        match mutation {
            0 => {
                host.resources[0]
                    .compute
                    .as_mut()
                    .unwrap()
                    .protected_preemption
                    .as_mut()
                    .unwrap()
                    .context_bytes = 256;
                host.resources[1].capacity_units = 256;
                selections[0].context_binding.units = 256;
            }
            1 => {
                host.resources[0]
                    .compute
                    .as_mut()
                    .unwrap()
                    .protected_preemption
                    .as_mut()
                    .unwrap()
                    .context_alignment = 32;
            }
            _ => {
                host.resources[1].pool_id = "continuation/other".into();
                selections[0].context_binding.pool_id = host.resources[1].pool_id.clone();
            }
        }
        let changed =
            seal_protected_preemptible_execution(original.clone(), &host, selections).unwrap();
        assert!(verify_plan(&changed));
        assert_ne!(
            changed.plan_id, baseline.plan_id,
            "context mutation {mutation}"
        );
        assert_eq!(changed.source_document_id, original.source_document_id);
        assert_eq!(changed.checked_plot_id, original.checked_plot_id);
        assert_eq!(changed.expanded_plot_id, original.expanded_plot_id);
        assert_eq!(
            changed.fragments[0].placements,
            original.fragments[0].placements
        );
        assert_eq!(
            changed.fragments[0].connections,
            original.fragments[0].connections
        );
    }
}

#[test]
fn independent_regions_share_capacity_without_reusing_the_same_context_bytes() {
    let (original, mut host, mut selections) = input();
    let mut fragment = original.fragments[0].clone();
    let mut second = fragment.placements[0].clone();
    second.placement_id = "placement/second".into();
    second.gear_id = "gear/second".into();
    fragment.startup_order.push(second.placement_id.clone());
    let first = &selections[0];
    let second_selection = ProtectedRegionSelection {
        region_id: "region/second".into(),
        placements: vec![second.placement_id.clone()],
        execution_profile_id: first.execution_profile_id.clone(),
        lane_resource: first.lane_resource.clone(),
        lane_base_id: first.lane_base_id.clone(),
        context_binding: first.context_binding.clone(),
    };
    fragment.placements.push(second);
    let plan = fixture::common::seal(fragment);
    assert!(
        verify_plan(&plan),
        "two independent placements are valid before protected selection"
    );
    host.resources[0].capacity_units = 2;
    selections.push(second_selection);
    assert_eq!(host.resources[1].capacity_units, 128);
    assert!(selections.iter().all(|s| s.context_binding.units == 128));
    assert_eq!(
        seal_protected_preemptible_execution(plan, &host, selections),
        Err(ProtectedRegionPlanningError::CapacityExceeded)
    );
}
