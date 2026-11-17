use super::*;
use conduit_body::{Body, BodyPlan, BodyPlotPlan, FederatedReceivingSession, ResidentPlot};
use conduit_core::OfferGeneration;

fn fixture() -> (
    StartupOwnerIdentity,
    StartupOwnerChallenge,
    SpawnInvitationSecret,
) {
    let host = StdHost::new_with_config(crate::StdHostConfig {
        host_id: "host/startup-owner".into(),
        boot_id: "boot/startup-owner".into(),
        offer_generation: OfferGeneration(1),
    });
    let identity = host.prepare_startup_owner_identity().unwrap();
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profile = conduit_plot::ProfileCatalog::new();
    conduit_time::install_tick_catalog(&mut startup, &mut profile).unwrap();
    conduit_semantic_catalog::install_tick_face_catalog(&mut startup, &mut profile).unwrap();
    let document = conduit_plot::check_syntax_document(
        &conduit_plot::parse_syntax_document(
            "plot owner-auth {\nclock: time/tick(count = 1, period-ms = 0)\n}\n",
        ),
        &startup,
    )
    .unwrap();
    let expanded = conduit_plot::expand_canonical_plot(&document, "owner-auth", &profile).unwrap();
    let plan = host.plan_expanded_local(&expanded).unwrap();
    let resident = ResidentPlot::new(
        plan.source_document_id.clone(),
        plan.checked_plot_id.clone(),
    );
    let body = Body::born(
        resident.source_document_id.clone(),
        resident.checked_plot_id.clone(),
        1,
        "sign/owner-auth-born".into(),
    )
    .unwrap();
    let wake = body.wake(1, "sign/owner-auth-woke".into()).unwrap().1;
    let plan = BodyPlan::seal(
        &wake,
        vec![BodyPlotPlan {
            plot: resident,
            plan,
        }],
    )
    .unwrap();
    let member = SpawnInvitationSecret::from_csprng_bytes([7; 32]).unwrap();
    let challenge = StartupOwnerChallenge {
        body_id: plan.body_id.clone(),
        wake_id: plan.wake_id.clone(),
        plan_id: plan.plan_id.clone(),
        global_play: BodyPlayIdentity::bind(&plan, 0),
        part_id: PartId::bind(&plan.body_id, "fixture/member", 1).unwrap(),
        // A public test label is scope, never a membership acceptance proof.
        credential_id: serde_json::from_str("\"credential/owner-auth\"").unwrap(),
        member_offer_digest: [11; 32],
        transport_binding_sha256: [13; 32],
        federation: FederationChallenge {
            session_id: "pending".into(),
            line_id: "pending".into(),
            line_security: LineSecurity::AuthenticatedEncrypted,
            initiator: FederationPeer {
                host_id: "host/member".into(),
                boot_id: "boot/member".into(),
                offer_generation: OfferGeneration(1),
                verifying_key: member.verifying_key(),
            },
            responder: identity.peer().clone(),
            nonce: [17; 32],
            expires_at_millis: 20_000,
            maximum_frames: 256,
        },
    }
    .bind_session()
    .unwrap();
    (identity, challenge, member)
}

#[test]
fn exact_startup_scope_authenticates_through_the_existing_federation_verifier() {
    let (identity, challenge, member) = fixture();
    let probe = crate::allocation_probe::begin();
    let owner_signature = identity.sign_startup_challenge(&challenge, 1000).unwrap();
    assert_eq!(probe.finish(), 0);
    let digest = conduit_body::challenge_transcript_digest(&challenge.federation);
    FederatedReceivingSession::establish(
        challenge.federation.clone(),
        member.sign(&digest),
        owner_signature,
        1000,
    )
    .unwrap();
}

#[test]
fn changed_scope_receiver_transport_and_replay_basis_cannot_reuse_a_bound_challenge() {
    let (identity, challenge, _) = fixture();
    for case in 0..12 {
        let mut changed = challenge.clone();
        match case {
            0 => changed.credential_id = serde_json::from_str("\"credential/other\"").unwrap(),
            1 => changed.part_id = PartId::bind(&changed.body_id, "other", 2).unwrap(),
            2 => changed.global_play.play_sequence += 1,
            3 => changed.member_offer_digest[0] ^= 1,
            4 => changed.transport_binding_sha256[0] ^= 1,
            5 => changed.federation.initiator.boot_id = "boot/replacement".into(),
            6 => changed.federation.responder.verifying_key[0] ^= 1,
            7 => changed.federation.nonce[0] ^= 1,
            8 => changed.federation.maximum_frames += 1,
            9 => changed.federation.expires_at_millis += 1,
            10 => changed.federation.session_id = "startup/unrelated".into(),
            _ => changed.federation.line_id = "startup-tls/unrelated".into(),
        }
        assert!(
            identity.sign_startup_challenge(&changed, 1000).is_err(),
            "case {case}"
        );
    }
    assert!(identity.sign_startup_challenge(&challenge, 20_000).is_err());
    let other = StdHost::new_with_config(crate::StdHostConfig {
        host_id: "host/other-owner".into(),
        boot_id: "boot/other-owner".into(),
        offer_generation: OfferGeneration(1),
    })
    .prepare_startup_owner_identity()
    .unwrap();
    assert!(other.sign_startup_challenge(&challenge, 1000).is_err());
}
