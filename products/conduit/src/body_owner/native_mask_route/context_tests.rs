//! Preparation refusal checks; no funded installation or physical Show claim.
use super::*;
use conduit_body::{MembershipCredential, PartId, ResidentPlot};
use conduit_std_host::{StdHost, StdHostConfig};

fn owner_fixture() -> (Owner, conduit_plot::ExpandedAuthoringPlot) {
    let source = crate::plot_source::parse(
        "plot fixture {\n display: face/text(\"Fixture\")\n \"Fixture\" >> display\n}.",
    )
    .unwrap();
    let plot = source.expand_entry_for_authoring().unwrap();
    let host = StdHost::new_with_config(StdHostConfig {
        host_id: "host/context-preparation-test".into(),
        boot_id: "boot/context-preparation-test".into(),
        offer_generation: conduit_core::OfferGeneration(1),
    });
    let resident = ResidentPlot::new(
        plot.expanded.source_document_id.clone(),
        plot.expanded.checked_plot_id.clone(),
    );
    (
        Owner::open(host, resident, None, "Preparation fixture").unwrap(),
        plot,
    )
}

#[test]
fn native_context_preparation_needs_idle_owner_and_current_admitted_member() {
    let (mut owner, _) = owner_fixture();
    let before = owner.session.evidence().clone();
    let offer = owner.host.advertisement().clone();
    let member = MemberPresentationBasis::new(
        MembershipCredential {
            // Wire-shaped negative fixture, never an admitted credential.
            credential_id: serde_json::from_value(serde_json::json!(
                "credential/context-test-unadmitted"
            ))
            .unwrap(),
            body_id: before.body_id.clone(),
            part_id: PartId::bind(&before.body_id, "unadmitted", 0).unwrap(),
            host_id: offer.host_id.clone(),
            boot_id: offer.boot_id.clone(),
            issued_at_millis: 0,
        },
        offer,
    )
    .unwrap();
    assert!(matches!(
        owner.prepare_native_ambient_contexts(&member, "clock", "weather", "image"),
        Err(error) if error == "native-mask-part-unavailable"
    ));
    // Host custody alone must fence preparation before inspecting the claim.
    // This fixture transfer is not an admitted application Start.
    let host = owner.host.take_for_play().unwrap();
    assert!(matches!(
        owner.prepare_native_ambient_contexts(&member, "clock", "weather", "image"),
        Err(error) if error == "native-mask-context-preparation-requires-idle-owner"
    ));
    owner.host.restore_after_play(host).unwrap();
    assert_eq!(owner.session.evidence(), &before);
    assert!(owner.session.realization().is_none());
    assert!(owner.pending_native_mask.is_none());
}

#[test]
fn streaming_offer_identity_matches_existing_json_digest_exactly() {
    let host = StdHost::new_with_config(StdHostConfig {
        host_id: "host/context/quoted-\"-unicode-λ".into(),
        boot_id: "boot/context/identity".into(),
        offer_generation: conduit_core::OfferGeneration(1),
    });
    let offer = host.advertisement();
    let prior = super::super::super::super::super::digest(&serde_json::to_vec(offer).unwrap());
    let actual = offer_identity(offer).unwrap();
    assert!(matches_digest(&actual, &prior));
    assert!(!matches_digest(
        &actual,
        prior.strip_prefix("sha256:").unwrap()
    ));
    assert!(!matches_digest(
        &actual,
        &prior.replace("sha256:", "sha512:")
    ));
    assert!(!matches_digest(&actual, &format!("{prior}0")));
    assert!(!matches_digest(&actual, ""));
    assert!(!matches_digest(&actual, &"0".repeat(64)));
}

#[test]
fn context_identity_requires_the_real_unstarted_proposal() {
    let (mut owner, plot) = owner_fixture();
    assert!(matches!(prospective_context_play(&owner), Err(error)
        if error == "native-mask-context-preparation-requires-unstarted-proposal"));
    owner.plan(&plot).unwrap();
    let proposal = owner.session.realization().unwrap();
    let expected = owner
        .host
        .prospective_body_play(&proposal.wake, &proposal.plan)
        .unwrap();
    let evidence = owner.session.evidence().clone();
    assert_eq!(prospective_context_play(&owner).unwrap(), expected);
    assert_eq!(owner.session.evidence(), &evidence);
    assert!(owner.session.realization().unwrap().play.is_none());
    let host = owner.host.take_for_play().unwrap();
    assert!(matches!(prospective_context_play(&owner), Err(error)
        if error == "native-mask-context-preparation-requires-idle-owner"));
    owner.host.restore_after_play(host).unwrap();
    owner.lull().unwrap();
    assert!(prospective_context_play(&owner).is_err());
}
