use super::*;
use alloc::vec;
use conduit_body::{Body, BodyPlotPlan, ResidentPlot};
use conduit_core::*;
#[path = "../../../../core/tests/common/sealed_state.rs"]
mod common;
fn fixture() -> (
    Wake,
    BodyPlan,
    BodyPlayIdentity,
    LoweredPlanFragment,
    BaseOperationClaim,
) {
    let mut fragment = common::fragment();
    fragment.states.clear();
    let kind = kind();
    let gear = &mut fragment.placements[0];
    gear.kind_id = kind.kind_id.clone();
    gear.kind_contract_revision = kind.kind_contract_revision.clone();
    gear.semantic_contract = kind.semantic_contract();
    gear.implementation_id = "independent/implementation".into();
    gear.capability_id = "independent/capability".into();
    gear.base = Some(BaseProviderBinding {
        base_id: "independent/base-id".into(),
        provider_instance_id: "independent/base".into(),
        provider_generation: 1,
        implementation_id: "independent/base-driver".into(),
        mechanism_family: "independent/base-kind".into(),
        enforcement_class: BaseEnforcementClass::Cooperative,
    });
    gear.host_calls = vec![HostCallRequirement {
        contract_id: "independent/call".into(),
        target_kind: Some(kind.kind_id.clone()),
        maximum_in_flight: 1,
        maximum_input_bytes: 1,
        maximum_output_bytes: 1,
    }];
    gear.resources = vec![ResourceBinding {
        pool_id: "independent/pool".into(),
        class_id: "independent/resource".into(),
        units: 1,
        protected: None,
        compute: None,
        content: None,
    }];
    gear.authority = vec![AuthorityBinding {
        grant_id: "independent/grant".into(),
        contract_id: "independent/authority".into(),
        host_call_contract_id: "independent/call".into(),
        subject_kind: kind.kind_id.clone(),
        host_id: fragment.host_id.clone(),
        boot_id: fragment.boot_id.clone(),
        capability_id: gear.capability_id.clone(),
    }];
    fragment.sign_storage_budget =
        mandatory_sign_storage_requirement(&fragment.expected_sign).unwrap();
    let plan = common::seal(fragment);
    let plot = ResidentPlot::new(
        plan.source_document_id.clone(),
        plan.checked_plot_id.clone(),
    );
    let body = Body::born_with_plots(
        conduit_body::BodyWorkset::from_plots([plot.clone()]).unwrap(),
        1,
        "born".into(),
    )
    .unwrap();
    let wake = body.wake(1, "woke".into()).unwrap().1;
    let plan = BodyPlan::seal(&wake, vec![BodyPlotPlan { plot, plan }]).unwrap();
    let play = BodyPlayIdentity::bind(&plan, 7);
    let fragment = &plan.plots[0].plan.fragments[0];
    let lowered = crate::lowering::lower_plan_fragment(fragment).unwrap();
    let claim = BaseOperationClaim {
        host_id: fragment.host_id.clone(),
        boot_id: fragment.boot_id.clone(),
        base_instance_id: "independent/base".into(),
        base_provider_generation: 1,
        plan_id: plan.plan_id.clone(),
        active_play_id: play.active_play_id.clone(),
        implementation_id: "independent/implementation".into(),
        operation_contract_id: "independent/call".into(),
        subject_kind: "independent/kind".into(),
        resource_pool_id: "independent/pool".into(),
        resource_generation_id: ResourceGenerationId("independent/generation".into()),
        envelope_id: "independent/envelope".into(),
        parameter_bytes: 1,
        work_units: 1,
    };
    (wake, plan, play, lowered, claim)
}
#[test]
fn body_scope_keeps_one_body_play_and_the_exact_selected_fragment() {
    let (wake, plan, play, lowered, claim) = fixture();
    let fragment = &plan.plots[0].plan.fragments[0];
    let selection = SelectedBodyOperationPlan {
        wake: &wake,
        plan: &plan,
        play: &play,
        fragment,
        lowered: &lowered,
        placement_id: &fragment.placements[0].placement_id,
    };
    assert_eq!(validate_scope(&claim, &selection), Ok(()));
    let child = bind_active_play(
        &fragment.plan_id,
        &fragment.host_id,
        &fragment.boot_id,
        play.play_sequence,
    );
    assert_ne!(child.active_play_id, play.active_play_id);
    let mut wrong = claim.clone();
    wrong.plan_id = fragment.plan_id.clone();
    wrong.active_play_id = child.active_play_id;
    assert_eq!(
        validate_scope(&wrong, &selection),
        Err(SelectedCallRefusal::WrongBinding)
    );
}
#[test]
fn stale_body_play_wake_host_and_substituted_fragment_refuse() {
    let (wake, plan, play, lowered, claim) = fixture();
    let fragment = &plan.plots[0].plan.fragments[0];
    for change in 0..6 {
        let mut wake = wake.clone();
        let mut plan = plan.clone();
        let mut play = play.clone();
        let mut fragment = fragment.clone();
        let mut claim = claim.clone();
        match change {
            0 => wake.workload_revision += 1,
            1 => play.play_sequence += 1,
            2 => plan.plan_id = "copied-body-label".into(),
            3 => fragment.boot_id = "another-boot".into(),
            4 => claim.host_id = "another-host".into(),
            _ => fragment.offer_generation.0 += 1,
        }
        let selection = SelectedBodyOperationPlan {
            wake: &wake,
            plan: &plan,
            play: &play,
            fragment: &fragment,
            lowered: &lowered,
            placement_id: &fragment.placements[0].placement_id,
        };
        assert_eq!(
            validate_scope(&claim, &selection),
            Err(SelectedCallRefusal::WrongBinding),
            "case {change}"
        );
    }
}

fn kind() -> Kind {
    let source = common::fragment();
    let gear = &source.placements[0];
    Kind {
        kind_id: "independent/kind".into(),
        kind_contract_revision: "independent/kind@1".into(),
        startup_parameters: vec![],
        shorthand: None,
        inputs: gear.inputs.clone(),
        outputs: gear.outputs.clone(),
        configuration: vec![],
        semantic_laws: vec![],
        limits: gear.limits.clone(),
    }
}
fn table(claim: &BaseOperationClaim) -> BaseCapabilityTable {
    let mut table = BaseCapabilityTable::new(
        claim.host_id.clone(),
        claim.boot_id.clone(),
        claim.base_instance_id.clone(),
        1,
        [7; 32],
        1,
    )
    .unwrap();
    let scope = BaseCapabilityScope {
        host_id: claim.host_id.clone(),
        boot_id: claim.boot_id.clone(),
        base_instance_id: claim.base_instance_id.clone(),
        base_provider_generation: 1,
        plan_id: claim.plan_id.clone(),
        active_play_id: claim.active_play_id.clone(),
        authority_grant_id: "independent/grant".into(),
        authority_contract_id: "independent/authority".into(),
        capability_id: "independent/capability".into(),
        implementation_id: claim.implementation_id.clone(),
        operation_contract_id: claim.operation_contract_id.clone(),
        subject_kind: claim.subject_kind.clone(),
        resource_pool_id: claim.resource_pool_id.clone(),
        resource_generation_id: claim.resource_generation_id.clone(),
        envelope_id: claim.envelope_id.clone(),
        maximum_parameter_bytes: 1,
        maximum_result_bytes: 1,
        maximum_work_units: 1,
        maximum_in_flight: 1,
        maximum_operations: 1,
    };
    let authority = BaseCapabilityAuthority {
        grant: AuthorityGrant {
            grant_id: scope.authority_grant_id.clone(),
            contract_id: scope.authority_contract_id.clone(),
            host_call_contract_id: scope.operation_contract_id.clone(),
            subject_kind: scope.subject_kind.clone(),
            host_id: scope.host_id.clone(),
            boot_id: scope.boot_id.clone(),
            capability_id: scope.capability_id.clone(),
        },
        base_instance_id: scope.base_instance_id.clone(),
        base_provider_generation: 1,
        resource_pool_id: scope.resource_pool_id.clone(),
        resource_generation_id: scope.resource_generation_id.clone(),
        operation_contract_id: scope.operation_contract_id.clone(),
        envelope_id: scope.envelope_id.clone(),
        maximum_parameter_bytes: 1,
        maximum_result_bytes: 1,
        maximum_work_units: 1,
        maximum_in_flight: 1,
        maximum_operations: 1,
    };
    let _bearer = table
        .issue(CapabilityIssueRequest { scope, authority })
        .unwrap();
    table
}
#[test]
fn full_body_binding_preserves_contract_tables_and_independent_possession() {
    let (wake, plan, play, lowered, claim) = fixture();
    let fragment = &plan.plots[0].plan.fragments[0];
    let kind = kind();
    let table = table(&claim);
    let before = table.inspections().next().unwrap().clone();
    let invoke = |lowered: &LoweredPlanFragment, input_bytes| {
        bind_body_selected_configured_operation(
            &table,
            &claim,
            SelectedOperationContract {
                kind: &kind,
                call: "independent/call",
                input_bytes,
                output_bytes: 1,
                resource_bytes: 1,
            },
            &[],
            SelectedBodyOperationPlan {
                wake: &wake,
                plan: &plan,
                play: &play,
                fragment,
                lowered,
                placement_id: &fragment.placements[0].placement_id,
            },
        )
    };
    assert_eq!(invoke(&lowered, 1), Ok(NodeId(0)));
    assert_eq!(invoke(&lowered, 2), Err(SelectedCallRefusal::WrongBinding));
    let mut substituted = lowered.clone();
    substituted.host_calls[0].binding.maximum_output_bytes = 2;
    assert_eq!(
        invoke(&substituted, 1),
        Err(SelectedCallRefusal::WrongBinding)
    );
    assert_eq!(table.inspections().next().unwrap(), &before);
}

#[test]
fn copied_plan_authority_labels_cannot_replace_actual_table_possession() {
    let (wake, plan, play, lowered, claim) = fixture();
    let fragment = &plan.plots[0].plan.fragments[0];
    let kind = kind();
    let empty = BaseCapabilityTable::new(
        claim.host_id.clone(),
        claim.boot_id.clone(),
        claim.base_instance_id.clone(),
        1,
        [7; 32],
        1,
    )
    .unwrap();
    let result = bind_body_selected_configured_operation(
        &empty,
        &claim,
        SelectedOperationContract {
            kind: &kind,
            call: "independent/call",
            input_bytes: 1,
            output_bytes: 1,
            resource_bytes: 1,
        },
        &[],
        SelectedBodyOperationPlan {
            wake: &wake,
            plan: &plan,
            play: &play,
            fragment,
            lowered: &lowered,
            placement_id: &fragment.placements[0].placement_id,
        },
    );
    assert_eq!(result, Err(SelectedCallRefusal::Possession));
    assert_eq!(empty.inspections().count(), 0);
}
