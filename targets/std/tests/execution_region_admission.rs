//! Sealed region promises must not silently fall back to the serial runner.
#[path = "replan_common/expression.rs"]
#[allow(dead_code)]
mod fixture;
use conduit_core::*;
use fixture::*;

fn region_plan(mut plan: Plan, lanes: u32, isolated: bool) -> Plan {
    let fragment = &mut plan.fragments[0];
    let mut admitted_placements = fragment
        .placements
        .iter()
        .map(|p| p.placement_id.clone())
        .collect::<Vec<_>>();
    admitted_placements.sort();
    fragment.execution_regions = vec![ExecutionRegion {
        region_id: "region/admission-control".into(),
        admitted_placements,
        execution_profile_id: "std/region-not-installed@1".into(),
        scheduling: ExecutionScheduling::CooperativeBoundedStep,
        lane_count: lanes,
        lane_resource: ResourceBinding {
            pool_id: "pool/not-an-installed-lane".into(),
            class_id: "conduit.resource/execution-lane@1".into(),
            units: lanes,
            protected: None,
            content: None,
            compute: Some(ComputeReservation {
                selected_lanes: lanes,
                service_guarantee: ComputeServiceGuarantee::Shared,
                architecture_base_id: "base/not-installed".into(),
                architecture_base_kind: ArchitectureBaseKind::HostedOs,
                topology_group_id: None,
                performance_class: None,
                nominal_clock_hz: None,
            }),
        },
        lane_base_id: "base/not-installed".into(),
        requirements: ExecutionRegionRequirements {
            runtime_memory_bytes: 0,
            timer_slots: 0,
            cord_item_capacity: 0,
            cord_byte_capacity: 0,
            mandatory_sign_items: fragment.sign_storage_budget.item_capacity,
            mandatory_sign_bytes: fragment.sign_storage_budget.byte_capacity,
        },
        preemption_required: isolated,
        isolation_required: isolated,
    }];
    seal_plan_with_realization_backs_and_completion(
        PlotIdentity {
            source_document_id: plan.source_document_id,
            checked_plot_id: plan.checked_plot_id,
            expanded_plot_id: plan.expanded_plot_id,
        },
        plan.completion_policy,
        plan.realization_backs,
        plan.fragments,
    )
}

#[test]
fn unsupported_region_and_lane_promises_refuse_before_output_and_leave_serial_path_usable() {
    let mut case = case();
    for lanes in [1, 2, 4] {
        for isolated in [false, true] {
            let plan = region_plan(case.previous.clone(), lanes, isolated);
            assert!(verify_plan(&plan), "a valid seal does not install a worker");
            let mut outputs = Outputs::default();
            let mut visible = Vec::new();
            let result = case.host.run_external_plot_to(
                plan.fragments[0].clone(),
                &inputs(7),
                &mut outputs,
                &mut visible,
                &mut NoTimer,
            );
            assert!(
                matches!(result, Err(ref reason) if reason.contains("execution regions are not installed")),
                "unsupported {lanes}-lane/isolation={isolated} contract: {result:?}"
            );
            assert!(outputs.0.is_empty(), "refusal must precede output delivery");
        }
    }
    // Every refusal leaves the unchanged ordinary single-lane reference usable.
    let report = old_run(&mut case);
    assert_eq!(report.kernel.unwrap().active_play.play_sequence, 0);
}
