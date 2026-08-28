use conduit_ai::{
    EmbodiedModelReceipt, EmbodiedModelView, EmbodimentReceiptError, EmbodimentStage,
    ProposalDecisionOutcome, ProposalRefusal,
};
use conduit_core::{kind_id, ActivePlayId, CheckedFormId, ExpandedFormId, PlanId, SignId};

fn view(stage: EmbodimentStage, index: usize) -> EmbodiedModelView {
    let expressive = !matches!(stage, EmbodimentStage::PerceptionOnly);
    let authorized = matches!(stage, EmbodimentStage::AuthorizedEffect);
    EmbodiedModelView {
        stage,
        checked_form_id: CheckedFormId::from(format!("checked/embodied/{index}")),
        expanded_form_id: ExpandedFormId::from(format!("expanded/embodied/{index}")),
        plan_id: PlanId::from(format!("plan/embodied/{index}")),
        active_play_id: ActivePlayId::from(format!("play/embodied/{index}")),
        model_gear_identity: "gear/model".into(),
        model_implementation_identity: "implementation/local-model/exact-digest".into(),
        wired_inputs: vec![
            kind_id("perception/scene-summary@1"),
            kind_id("robotics/battery-permille-millivolts@1"),
        ],
        wired_outputs: if expressive {
            vec![kind_id("value/text@1")]
        } else {
            Vec::new()
        },
        expressive_output_wired: expressive,
        protected_effect_wired: authorized,
        authority_id: authorized.then(|| "authority/indicator-only".into()),
        proposal_id: format!("proposal/embodied/{index}"),
        decision: if authorized {
            ProposalDecisionOutcome::Authorized {
                request_id: "request/embodied/effect".into(),
            }
        } else {
            ProposalDecisionOutcome::Refused(ProposalRefusal::UnwiredOperation)
        },
        resulting_signs: if authorized {
            vec![SignId::from("sign/embodied/effect")]
        } else {
            Vec::new()
        },
    }
}

fn receipt() -> EmbodiedModelReceipt {
    EmbodiedModelReceipt {
        schema: "conduit.llm/embodied-body-receipt@1",
        proof_class: "deterministic-production-kernel",
        body_id: "body/embodied-model".into(),
        perception_value_kind: kind_id("perception/scene-summary@1"),
        state_value_kind: kind_id("robotics/battery-permille-millivolts@1"),
        expressive_value_kind: kind_id("value/text@1"),
        protected_effect_kind: kind_id("effect/indicator-set@1"),
        views: vec![
            view(EmbodimentStage::PerceptionOnly, 1),
            view(EmbodimentStage::Expressive, 2),
            view(EmbodimentStage::AuthorizedEffect, 3),
        ],
        ambient_host_access: false,
    }
}

#[test]
fn graph_wiring_alone_changes_model_context_and_power() {
    receipt().validate().unwrap();
}

#[test]
fn provider_swap_plan_reuse_and_ambient_ports_refuse() {
    let mut changed = receipt();
    changed.views[1].model_implementation_identity = "implementation/other".into();
    assert_eq!(
        changed.validate(),
        Err(EmbodimentReceiptError::ProviderChanged)
    );

    let mut reused = receipt();
    reused.views[1].plan_id = reused.views[0].plan_id.clone();
    assert_eq!(
        reused.validate(),
        Err(EmbodimentReceiptError::IdentityReused)
    );

    let mut ambient = receipt();
    ambient.views[0]
        .wired_inputs
        .push(kind_id("ambient/filesystem-tool@1"));
    assert_eq!(ambient.validate(), Err(EmbodimentReceiptError::AmbientPort));
}

#[test]
fn unwired_or_unauthorized_effect_cannot_be_reported_as_real() {
    let mut fabricated = receipt();
    fabricated.views[0].decision = ProposalDecisionOutcome::Authorized {
        request_id: "request/fabricated".into(),
    };
    fabricated.views[0].resulting_signs = vec![SignId::from("sign/fabricated")];
    assert_eq!(
        fabricated.validate(),
        Err(EmbodimentReceiptError::InvalidDecision)
    );

    let mut missing_authority = receipt();
    missing_authority.views[2].authority_id = None;
    assert_eq!(
        missing_authority.validate(),
        Err(EmbodimentReceiptError::MissingEffectWiring)
    );

    let mut model_claim = receipt();
    model_claim.views[2].resulting_signs.clear();
    assert_eq!(
        model_claim.validate(),
        Err(EmbodimentReceiptError::InvalidEffectEvidence)
    );
}
