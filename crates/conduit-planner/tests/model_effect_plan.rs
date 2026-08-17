mod model_effect_common;

use conduit_ai::{
    EffectAuthority, EffectAuthorityDerivationError, ProposalDecisionOutcome, ProposalGate,
};
use conduit_core::{kind_id, port_id, verify_plan, PlacementId, PlanId};
use conduit_kernel::{
    BoundedValueRef, FixedHostOperationBindings, HostOperationBinding, HostOperationId, NodeId,
    OperationAction, RequestId, ValueRef,
};
use model_effect_common::{proposal, reseal, wired_plan, EFFECT_KIND};

#[test]
fn exact_plan_wiring_derives_authority_and_reaches_kernel_admission() {
    let plan = wired_plan();
    assert!(verify_plan(&plan));
    let authority = EffectAuthority::from_plan(
        &plan,
        &PlacementId::from("placement/proposer"),
        &kind_id(EFFECT_KIND),
    )
    .unwrap();
    assert_eq!(authority.authority_id, "grant/send-message");
    assert_eq!(authority.maximum_argument_bytes, 512);

    let mut gate = ProposalGate::new(Some(authority), 2).unwrap();
    let disposition = gate.submit(proposal(&plan)).unwrap();
    assert!(matches!(
        disposition.decision.outcome,
        ProposalDecisionOutcome::Authorized { .. }
    ));
    let request = disposition.request.unwrap();

    let byte_len = u32::try_from(request.canonical_arguments.len()).unwrap();
    let input = BoundedValueRef::new(
        ValueRef {
            slot: 0,
            generation: 1,
            byte_len,
        },
        512,
    )
    .unwrap();
    let mut bindings = FixedHostOperationBindings::<1>::new(1);
    bindings
        .install(
            NodeId(0),
            HostOperationBinding {
                operation: HostOperationId(0),
                maximum_input_bytes: 512,
                maximum_output_bytes: 512,
            },
        )
        .unwrap();
    bindings.seal().unwrap();
    assert!(bindings
        .admit(
            NodeId(0),
            OperationAction::RequestHostOperation {
                request: RequestId(1),
                operation: HostOperationId(0),
                input,
            },
        )
        .is_ok());
}

#[test]
fn missing_or_mutated_plan_truth_cannot_be_replaced_by_model_output() {
    let plan = wired_plan();
    let proposer = PlacementId::from("placement/proposer");
    let operation = kind_id(EFFECT_KIND);

    let mut unwired = plan.clone();
    unwired.fragments[0].connections.clear();
    let unwired = reseal(unwired);
    assert_eq!(
        EffectAuthority::from_plan(&unwired, &proposer, &operation),
        Err(EffectAuthorityDerivationError::EffectWiringMissing)
    );

    let mut invented_source_port = plan.clone();
    invented_source_port.fragments[0].connections[0].source_port_id = port_id("invented");
    let invented_source_port = reseal(invented_source_port);
    assert_eq!(
        EffectAuthority::from_plan(&invented_source_port, &proposer, &operation),
        Err(EffectAuthorityDerivationError::EffectWiringMissing)
    );

    let mut unavailable = plan.clone();
    unavailable.fragments[0].placements[1].authority.clear();
    let unavailable = reseal(unavailable);
    assert_eq!(
        EffectAuthority::from_plan(&unavailable, &proposer, &operation),
        Err(EffectAuthorityDerivationError::AuthorityMissing)
    );

    let mut wrong_subject = plan.clone();
    wrong_subject.fragments[0].placements[1].authority[0].subject_kind = kind_id("effect/other@1");
    let wrong_subject = reseal(wrong_subject);
    assert_eq!(
        EffectAuthority::from_plan(&wrong_subject, &proposer, &operation),
        Err(EffectAuthorityDerivationError::AuthorityMissing)
    );

    let mut corrupted = plan.clone();
    corrupted.plan_id = PlanId::from("model-supplied-plan");
    assert_eq!(
        EffectAuthority::from_plan(&corrupted, &proposer, &operation),
        Err(EffectAuthorityDerivationError::InvalidPlan)
    );

    let mut no_authority_gate = ProposalGate::new(None, 2).unwrap();
    assert!(no_authority_gate
        .submit(proposal(&plan))
        .unwrap()
        .request
        .is_none());
}
