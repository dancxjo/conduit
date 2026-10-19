#![cfg(feature = "kernel-operation-owners")]
use conduit_ai::{
    fixed_numeric_catalog::fixed_numeric_contracts,
    fixed_numeric_value_capacity::value_capacity64_contract,
    operation_owners::fixed_numeric::{fixed_numeric_offer, fixed_numeric_offer_capacity64},
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
        .and_then(|eligibility| eligibility.require(WorkTransformation::Fusion))
        .is_ok()
}
#[test]
fn only_explicit_pointwise_kinds_gain_pure_semantics() {
    let contracts = fixed_numeric_contracts().unwrap();
    let mut eligible = 0;
    for kind in contracts {
        let offer = fixed_numeric_offer(kind.kind_id.as_str());
        let declared = kind
            .kind_contract_revision
            .as_str()
            .ends_with("/pure-value@1");
        if declared {
            eligible += 1;
            let offer = offer.unwrap();
            assert!(fusion(&kind, &back(&offer)));
            let larger = value_capacity64_contract(kind.clone()).unwrap();
            let larger_offer = fixed_numeric_offer_capacity64(kind.kind_id.as_str()).unwrap();
            assert!(fusion(&larger, &back(&larger_offer)));
            assert!(larger
                .kind_contract_revision
                .as_str()
                .contains(kind.kind_contract_revision.as_str()));
            assert_eq!(kind.semantic_laws, larger.semantic_laws);
            assert_eq!(kind.inputs, larger.inputs);
            assert_eq!(kind.outputs, larger.outputs);
        } else if let Ok(offer) = offer {
            assert!(!fusion(&kind, &back(&offer)), "{}", kind.kind_id.as_str());
        }
    }
    // Four elementwise operations at 40/128/160/192, sigmoid4, multiply320,
    // and tanh at 4/40/64/128/160/192/320.
    assert_eq!(eligible, 25);
}
#[test]
fn missing_duplicate_effect_and_realization_facts_refuse_fusion() {
    let kind = fixed_numeric_contracts()
        .unwrap()
        .into_iter()
        .find(|kind| kind.kind_id.as_str() == "numeric/add128")
        .unwrap();
    let offer = fixed_numeric_offer("numeric/add128").unwrap();
    assert!(fusion(&kind, &back(&offer)));
    for index in 1..kind.semantic_laws.len() {
        let mut missing = kind.clone();
        missing.semantic_laws.remove(index);
        assert!(!fusion(&missing, &back(&offer)));
        let mut duplicate = kind.clone();
        duplicate
            .semantic_laws
            .push(kind.semantic_laws[index].clone());
        assert!(!fusion(&duplicate, &back(&offer)));
    }
    let mut effectful = kind.clone();
    let fact = effectful
        .semantic_laws
        .iter_mut()
        .find(|law| matches!(law, KindSemanticLaw::ExternalEffects(_)))
        .unwrap();
    *fact = KindSemanticLaw::ExternalEffects(ExternalEffectBehavior::Observable);
    assert!(!fusion(&effectful, &back(&offer)));
    let mut stateful = kind.clone();
    let fact = stateful
        .semantic_laws
        .iter_mut()
        .find(|law| matches!(law, KindSemanticLaw::TemporalState(_)))
        .unwrap();
    *fact = KindSemanticLaw::TemporalState(TemporalStateBehavior::Retained);
    assert!(!fusion(&stateful, &back(&offer)));
    let mut realized = back(&offer);
    realized.authority_requirements.push(AuthorityRequirement {
        contract_id: "test/forbidden".into(),
        host_call_contract_id: "test/forbidden-call".into(),
        subject_kind: kind_id("test/forbidden-subject"),
    });
    assert!(!fusion(&kind, &realized));
}

#[test]
fn compact_offer_layout_requires_exact_owned_type_envelope_and_artifact() {
    use conduit_ai::operation_owners::fixed_numeric::compact_pointwise_offer_layout;
    let offer = fixed_numeric_offer_capacity64("numeric/add128").unwrap();
    let layout = compact_pointwise_offer_layout(&offer).unwrap();
    assert_eq!((layout.lanes, layout.logical_bytes), (128, 1372));
    let mut artifact = offer.clone();
    artifact.implementation.artifact_id = "unselected/artifact".into();
    assert!(compact_pointwise_offer_layout(&artifact).is_err());
    let mut port = offer.clone();
    port.outputs[0].value_kind = kind_id("forged/f32-looking-profile");
    assert!(compact_pointwise_offer_layout(&port).is_err());
    let mut laws = offer;
    laws.semantic_contract
        .laws
        .retain(|law| !matches!(law, KindSemanticLaw::Replay(_)));
    assert!(compact_pointwise_offer_layout(&laws).is_err());
    let linear = fixed_numeric_offer("numeric/linear128x128").unwrap();
    assert!(
        compact_pointwise_offer_layout(&linear).is_err(),
        "immutable tensor custody needs its own explicit realization proof"
    );
}
