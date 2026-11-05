//! Canonical full membership, real signed proof, unchanged installed custody.
use super::*;

#[test]
fn full_membership_refuses_signed_browser_proof_with_a_public_capacity_code() {
    let (mut owner, root, _) = setup();
    let mut biography = owner.session.evidence().clone();
    while biography.membership.parts.len() < conduit_body::MAX_BODY_PARTS {
        let mut membership = biography.membership.clone();
        let index = membership.parts.len();
        let sequence = biography.records.last().unwrap().sequence + 1;
        let part = conduit_body::PartId::bind(
            &biography.body_id,
            &format!("fixture/retained-offline/{index}"),
            sequence,
        )
        .unwrap();
        let change = membership
            .admit(
                &biography.body_id,
                membership.revision,
                part,
                conduit_body::MembershipProofId::bind(&format!("proof/retained-offline/{index}"))
                    .unwrap(),
                format!("sign/retained-offline/{index}").into(),
            )
            .unwrap();
        biography
            .append_membership_events(membership, &[(change, sequence)])
            .unwrap();
    }
    let authority = owner.host.advertisement();
    owner.session = conduit_body::BodyLifecycleSession::open_admitted(
        biography.clone(),
        &authority.host_id,
        &authority.boot_id,
    )
    .unwrap();
    owner.persist(&root).unwrap();
    let authorized = owner
        .browser_authorize_window(Some("host/browser-test"), Some(BROWSER_KEY), 10_000)
        .unwrap();
    let challenge = advertise(&mut owner, &authorized.window_id);
    let secret = SpawnInvitationSecret::from_csprng_bytes([7; 32]).unwrap();
    let result = owner.browser_complete(
        &root,
        &authorized.window_id,
        In::AmbientProof {
            protocol: PROTOCOL,
            admission_id: challenge.admission_id.clone(),
            body_id: challenge.body_id.clone(),
            host_id: challenge.host_id.clone(),
            boot_id: challenge.boot_id.clone(),
            nonce: challenge.nonce.to_vec(),
            signature: secret.sign(&challenge.signing_transcript()).to_vec(),
        },
    );
    assert_eq!(result.err().unwrap(), "membership-part-capacity-exhausted");
    assert_eq!(owner.session.evidence(), &biography);
    assert_eq!(
        crate::durable_host::owner::state::load(&root)
            .unwrap()
            .unwrap(),
        biography
    );
    assert!(owner
        .admissions
        .as_ref()
        .is_none_or(|manager| manager.receipts.is_empty()));
    std::fs::remove_dir_all(root).unwrap();
}
