use conduit_body::{
    Body, BodyBiographyEvidence, BodyFulfillment, BodyMembership, BodyWorkset,
    FulfillmentReadiness, derive_fulfillment_readiness,
};
use conduit_core::{AuthorityGrantId, SignId};
use conduit_tutorial_plot::{
    TutorialPlayback, presentation_from_evidence, purpose_state_from_evidence,
};

#[test]
fn explicit_fulfillment_is_distinct_from_completing_optional_tutorial_exercises() {
    let born = Body::born_with_plots(BodyWorkset::default(), 0, SignId::from("test/born")).unwrap();
    let membership = BodyMembership::new(born.body_id.clone()).unwrap();
    let mut evidence =
        BodyBiographyEvidence::born(born.clone(), membership, "Resting".into()).unwrap();
    let sign = SignId::from("test/fulfilled");
    let fulfilled = born
        .fulfill(
            BodyFulfillment {
                final_wake_id: None,
                authority_grant_id: AuthorityGrantId::from("test/operator-grant"),
                attribution: "test/operator".into(),
                settled_obligations: vec![],
            },
            sign.clone(),
        )
        .unwrap();
    evidence
        .append_body_lifecycle_events(fulfilled, &[(sign, 1)])
        .unwrap();
    let purpose = purpose_state_from_evidence(&evidence).unwrap();
    assert!(matches!(
        derive_fulfillment_readiness(&purpose).unwrap(),
        FulfillmentReadiness::NotReady { .. }
    ));
    let view = presentation_from_evidence(&evidence, 1, TutorialPlayback::Fulfilled)
        .unwrap()
        .lower()
        .unwrap();
    assert!(
        view.nodes
            .iter()
            .any(|node| node.text.contains("This Body is fulfilled."))
    );
    assert!(view.nodes.iter().all(|node| {
        !node.text.contains("Not yet fulfilled") && !node.text.contains("keep exploring")
    }));
    assert!(
        view.actions
            .iter()
            .any(|action| action.id == "body.inspect-lifecycle")
    );
}
