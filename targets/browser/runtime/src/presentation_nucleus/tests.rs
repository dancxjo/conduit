use super::*;

#[test]
fn browser_offers_preserve_semantic_faces_but_own_realization_identity() {
    let offers = offers();
    assert_eq!(offers.len(), 13);
    for offer in offers {
        let canonical = offers::canonical_offer(offer.kind_id.as_str()).unwrap();
        assert_eq!(
            offer.kind_contract_revision,
            canonical.kind_contract_revision
        );
        assert_eq!(offer.inputs, canonical.inputs);
        assert_eq!(offer.outputs, canonical.outputs);
        assert_eq!(offer.host_operations, canonical.host_operations);
        assert_eq!(offer.limits, canonical.limits);
        assert_eq!(
            offer.implementation.execution_profile_id.as_str(),
            BROWSER_PRESENTATION_PROFILE
        );
        assert_eq!(
            offer.implementation.artifact_id.as_str(),
            BROWSER_PRESENTATION_ARTIFACT
        );
    }
    let browser_upper = browser_text_upper_offer();
    let canonical_upper = browser_text_upper_offer();
    assert_eq!(
        browser_upper.kind_contract_revision,
        canonical_upper.kind_contract_revision
    );
    assert_eq!(browser_upper.inputs, canonical_upper.inputs);
    assert_eq!(browser_upper.outputs, canonical_upper.outputs);
    assert_eq!(
        browser_upper.host_operations,
        canonical_upper.host_operations
    );
    assert_eq!(browser_upper.limits, canonical_upper.limits);
    assert_eq!(
        browser_upper.implementation.execution_profile_id.as_str(),
        BROWSER_TEXT_UPPER_PROFILE
    );
    assert_eq!(
        browser_upper.implementation.artifact_id.as_str(),
        BROWSER_TEXT_UPPER_ARTIFACT
    );
}

#[test]
fn ordinary_browser_plans_execute_layout_and_graphics_through_the_kernel() {
    let proof = execute_browser_nucleus().expect("browser nucleus executes");
    assert_eq!(proof.graphics.commands().len(), 3);
    assert_eq!(proof.layout.child_count, 3);
    assert_eq!(proof.text, "STRASSE");
    assert_ne!(proof.graphics_plan_id, proof.layout_plan_id);
    assert_ne!(proof.layout_plan_id, proof.text_plan_id);
    assert_ne!(proof.text_plan_id, proof.structured_plan_id);
    assert!(proof.structured.presentation.text.is_empty());
    assert!(proof
        .structured
        .presentation
        .properties
        .iter()
        .any(|property| {
            property.name == "record-schema"
                && property.value
                    == conduit_presentation::PresentationPropertyValue::Identity(
                        "education/feedback@1".into(),
                    )
        }));
    assert!(proof
        .structured
        .presentation
        .properties
        .iter()
        .any(|property| {
            property.name == "quantity-unit"
                && property.value
                    == conduit_presentation::PresentationPropertyValue::Identity(
                        "ratio/percent".into(),
                    )
        }));
}
