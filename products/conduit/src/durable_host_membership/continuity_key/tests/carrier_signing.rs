//! Prepared and ordinary signing preserve the original credential/key scope.
use super::*;
#[test]
fn member_carrier_signer_uses_retained_key_without_replacing_membership() {
    let mut f = fixture();
    let local = SpawnInvitationSecret::from_csprng_bytes([7; 32]).unwrap();
    let mut manager =
        conduit_body::AdmissionManager::new(f.receipt.credential.body_id.clone()).unwrap();
    let mut membership =
        conduit_body::BodyMembership::new(f.receipt.credential.body_id.clone()).unwrap();
    let invitation = manager
        .issue_spawn_invitation(local.clone(), [3; 32], 1, 1000)
        .unwrap();
    let actual = &f.receipt.host_advertisement;
    let proof = conduit_body::SpawnAdmissionProof {
        invitation_id: invitation.invitation_id.clone(),
        body_id: invitation.body_id.clone(),
        host_id: actual.host_id.clone(),
        boot_id: actual.boot_id.clone(),
        nonce: invitation.nonce,
        signature: local.sign(&invitation.signing_transcript(
            &actual.host_id,
            &actual.boot_id,
            actual.offer_generation,
        )),
    };
    f.receipt.credential = manager
        .complete_spawn(
            &mut membership,
            actual,
            &proof,
            2,
            conduit_body::AdmissionSigns {
                part_admitted: "fixture/part".into(),
                host_attached: "fixture/attached".into(),
                candidate_admitted: "fixture/admitted".into(),
            },
        )
        .unwrap();
    f.pending.invitation.claim = invitation.claim();
    f.pending.request.invitation_id = invitation.invitation_id;
    f.pending.request.nonce = invitation.nonce;
    f.pending.request.signature = proof.signature.to_vec();
    durable_host::write_json_atomic(&f.root.join("body/pending-join.json"), &f.pending).unwrap();
    complete(&f).unwrap();
    let original_membership = membership.clone();
    let receipt_count = manager.receipts.len();
    let receiver_key = SpawnInvitationSecret::from_csprng_bytes([13; 32]).unwrap();
    let receiver = conduit_body::FederationPeer {
        host_id: "fixture/owner".into(),
        boot_id: "fixture/owner-boot".into(),
        offer_generation: OfferGeneration(1),
        verifying_key: receiver_key.verifying_key(),
    };
    let session = conduit_core::LinkBindingId::from("fixture/carrier");
    let line = conduit_core::LineId::from("fixture/authenticated-line");
    let context = MemberCarrierSigningContext {
        receiver: &receiver,
        session: &session,
        line: &line,
        security: conduit_core::LineSecurity::AuthenticatedEncrypted,
        maximum_frames: 128,
        maximum_millis: 1000,
    };
    let challenge = conduit_body::FederationChallenge {
        session_id: session.clone(),
        line_id: line.clone(),
        line_security: context.security,
        initiator: conduit_body::FederationPeer {
            host_id: f.receipt.host_advertisement.host_id.clone(),
            boot_id: f.receipt.host_advertisement.boot_id.clone(),
            offer_generation: OfferGeneration(1),
            verifying_key: local.verifying_key(),
        },
        responder: receiver.clone(),
        nonce: [19; 32],
        expires_at_millis: 500,
        maximum_frames: 64,
    };
    let installation = fs::read(f.root.join("installation.json")).unwrap();
    let private_key = fs::read(f.root.join("body").join(PATH)).unwrap();
    let credential_path = PathBuf::from(
        durable_host::read_installation(&f.root.join("installation.json"))
            .unwrap()
            .joined_body_state
            .unwrap()
            .credential_path,
    );
    let credential_bytes = fs::read(&credential_path).unwrap();
    for iteration in 0..8 {
        // Synthetic distinct issuance. Real nonce/session freshness belongs to
        // the authenticated receiver; this fixture is not a CSPRNG proof.
        let session = conduit_core::LinkBindingId::from(format!("fixture/carrier/{iteration}"));
        let context = MemberCarrierSigningContext {
            receiver: &receiver,
            session: &session,
            line: &line,
            security: conduit_core::LineSecurity::AuthenticatedEncrypted,
            maximum_frames: 128,
            maximum_millis: 1000,
        };
        let mut challenge = challenge.clone();
        challenge.session_id = session.clone();
        challenge.nonce = [19 + iteration; 32];
        let transcript: [u8; 32] = conduit_body::challenge_transcript(&challenge)
            .try_into()
            .unwrap();
        let signature = sign_member_carrier_challenge(
            &f.root,
            &f.receipt.credential,
            &f.receipt.host_advertisement,
            &context,
            &challenge,
            100,
        )
        .unwrap();
        let witness = manager
            .current_member_continuity(
                &membership,
                &f.receipt.credential.part_id,
                &f.receipt.host_advertisement,
            )
            .unwrap();
        let mut slot = conduit_body::CurrentMemberChallengeSlot::default();
        slot.issue(witness, &receiver, challenge.clone(), 99)
            .unwrap();
        let receiving = slot
            .complete(
                &membership,
                &receiver,
                &challenge,
                signature,
                receiver_key.sign(&transcript),
                100,
            )
            .unwrap();
        assert!(receiving.inspection(&membership).membership_current);
        assert!(slot
            .complete(
                &membership,
                &receiver,
                &challenge,
                signature,
                receiver_key.sign(&transcript),
                100
            )
            .is_err());
    }
    assert_eq!(membership, original_membership);
    assert_eq!(manager.receipts.len(), receipt_count);
    assert_eq!(
        fs::read(f.root.join("installation.json")).unwrap(),
        installation
    );
    assert_eq!(
        fs::read(f.root.join("body").join(PATH)).unwrap(),
        private_key
    );
    // Preparation is allowed to retain custody, not to outlive a changed
    // installation or credential. Recheck exact bytes without rebuilding the
    // equipment catalogue inside the authenticated network deadline.
    for changed in [&f.root.join("installation.json"), &credential_path] {
        let signer = prepare_member_carrier_signer(
            &f.root,
            &f.receipt.credential,
            &f.receipt.host_advertisement,
        )
        .unwrap();
        let original = fs::read(changed).unwrap();
        let mut replacement = original.clone();
        replacement.push(b' ');
        fs::write(changed, &replacement).unwrap();
        assert!(signer
            .sign(&context, &challenge, 100)
            .unwrap_err()
            .contains("custody changed"));
        fs::write(changed, &original).unwrap();
    }
    assert_eq!(fs::read(credential_path).unwrap(), credential_bytes);
    for change in 0..13 {
        let mut received = challenge.clone();
        let mut actual = f.receipt.host_advertisement.clone();
        let mut credential = f.receipt.credential.clone();
        match change {
            0 => received.responder.host_id = "redirected/host".into(),
            1 => received.responder.verifying_key = local.verifying_key(),
            2 => received.session_id = "another/session".into(),
            3 => received.line_id = "another/line".into(),
            4 => received.line_security = conduit_core::LineSecurity::PlaintextNetwork,
            5 => received.initiator.verifying_key = receiver_key.verifying_key(),
            6 => actual.boot_id = "another/boot".into(),
            7 => actual.offer_generation = OfferGeneration(2),
            8 => received.nonce = [0; 32],
            9 => received.expires_at_millis = 100,
            10 => received.maximum_frames = 129,
            11 => received.expires_at_millis = 1101,
            _ => credential.credential_id = id("another/credential"),
        }
        assert!(
            sign_member_carrier_challenge(&f.root, &credential, &actual, &context, &received, 100)
                .is_err(),
            "variant {change}"
        );
    }
    assert_eq!(
        fs::read(f.root.join("installation.json")).unwrap(),
        installation
    );
}
