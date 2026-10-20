#![cfg(feature = "kernel-operation-owners")]
use conduit_ai::{
    fixed_numeric_catalog::fixed_numeric_contracts,
    operation_owners::fixed_numeric::{compact_pointwise_offer_layout, fixed_numeric_offer},
};
use conduit_core::*;

fn back(offer: &CapabilityOffer) -> Back {
    Back {
        capability_id: offer.capability_id.clone(),
        execution_profile_id: offer.implementation.execution_profile_id.clone(),
        implementation_id: offer.implementation.implementation_id.clone(),
        artifact_id: offer.implementation.artifact_id.clone(),
        host_calls: offer.host_calls.clone(),
        resource_requirements: offer.resource_requirements.clone(),
        authority_requirements: offer.authority_requirements.clone(),
    }
}
fn fusion(kind: &Kind, back: &Back) -> bool {
    derive_transformation_eligibility(kind, back)
        .and_then(|e| e.require(WorkTransformation::Fusion))
        .is_ok()
}
#[test]
fn exact_linear_contracts_declare_explicit_resource_dependence_without_transport_grants() {
    let mut found = Vec::new();
    for kind in fixed_numeric_contracts().unwrap() {
        if !kind
            .kind_contract_revision
            .as_str()
            .ends_with("/read-immutable-input@1")
        {
            continue;
        }
        found.push(kind.kind_id.as_str().to_owned());
        let offer = fixed_numeric_offer(kind.kind_id.as_str()).unwrap();
        let facts = pure_expression_facts(&kind).unwrap();
        assert_eq!(facts.resource_dependence, SemanticDependence::ExplicitInput);
        assert_eq!(facts.temporal_state, TemporalStateBehavior::None);
        assert_eq!(facts.replay, ReplayBehavior::Exact);
        assert!(fusion(&kind, &back(&offer)));
        assert_eq!(kind.inputs.len(), 2);
        assert_eq!(kind.inputs[1].port_id, port_id("weights"));
        assert!(kind
            .inputs
            .iter()
            .chain(&kind.outputs)
            .all(|p| p.temporal == PortTemporal::Value));
        assert!(
            compact_pointwise_offer_layout(&offer).is_err(),
            "semantic facts are not compact realization admission"
        );
    }
    found.sort();
    let mut expected: Vec<_> = [
        "3x2", "161x18", "18x18", "80x1", "328x192", "192x192", "192x4", "272x480", "160x480",
        "240x384", "128x384", "208x384", "160x160", "128x128", "688x128", "128x40",
    ]
    .map(|shape| format!("numeric/linear{shape}"))
    .into();
    expected.sort();
    assert_eq!(found, expected);
}
#[test]
fn missing_ambiguous_ambient_stateful_or_authority_bearing_contracts_are_refused() {
    let kind = fixed_numeric_contracts()
        .unwrap()
        .into_iter()
        .find(|k| k.kind_id.as_str() == "numeric/linear80x1")
        .unwrap();
    let offer = fixed_numeric_offer("numeric/linear80x1").unwrap();
    let realized = back(&offer);
    assert!(fusion(&kind, &realized));
    for index in 1..kind.semantic_laws.len() {
        let mut missing = kind.clone();
        missing.semantic_laws.remove(index);
        assert!(!fusion(&missing, &realized));
        let mut duplicate = kind.clone();
        duplicate
            .semantic_laws
            .push(kind.semantic_laws[index].clone());
        assert!(!fusion(&duplicate, &realized));
    }
    for replacement in [
        KindSemanticLaw::ResourceDependence(SemanticDependence::Ambient),
        KindSemanticLaw::TemporalState(TemporalStateBehavior::Retained),
        KindSemanticLaw::ExternalEffects(ExternalEffectBehavior::Observable),
        KindSemanticLaw::Suspension(SuspensionBehavior::MaySuspend),
    ] {
        let mut changed = kind.clone();
        let fact = changed
            .semantic_laws
            .iter_mut()
            .find(|fact| std::mem::discriminant(*fact) == std::mem::discriminant(&replacement))
            .unwrap();
        *fact = replacement;
        assert!(!fusion(&changed, &realized));
    }
    let mut authority = realized;
    authority.authority_requirements.push(AuthorityRequirement {
        contract_id: "test/extra-authority".into(),
        host_call_contract_id: "test/extra-call".into(),
        subject_kind: kind_id("test/extra-subject"),
    });
    assert!(!fusion(&kind, &authority));
}
