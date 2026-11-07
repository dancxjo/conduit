use crate::{
    member_return::{Refusal, RetainedReturn},
    tests::ready,
    transport::MessageTransport,
};
use conduit_body::*;
use conduit_core::{BootId, HostAdvertisement};
use ed25519_dalek::{Signature, SigningKey};
use sha2::{Digest, Sha256};
use std::{collections::VecDeque, vec, vec::Vec};
const IMAGE: &[u8] = b"immutable-offer-image";
struct Messages {
    incoming: VecDeque<Vec<u8>>,
    sent: Vec<Vec<u8>>,
}
impl MessageTransport for Messages {
    type Error = ();
    async fn receive<'a>(&mut self, output: &'a mut [u8]) -> Result<&'a [u8], ()> {
        let bytes = self.incoming.pop_front().ok_or(())?;
        let out = output.get_mut(..bytes.len()).ok_or(())?;
        out.copy_from_slice(&bytes);
        Ok(out)
    }
    async fn send(&mut self, bytes: &[u8]) -> Result<(), ()> {
        self.sent.push(bytes.to_vec());
        Ok(())
    }
}
fn fixture() -> (
    MemberBootstrap<'static>,
    HostAdvertisement,
    SigningKey,
    SigningKey,
) {
    let (mut bootstrap, actual, member, owner) = crate::membership_tests::fixture();
    let mut retained = crate::membership_tests::receipt(&bootstrap, &actual).credential;
    retained.boot_id = BootId::from("previous/boot");
    bootstrap.admission = MemberBootstrapMaterial::Retained {
        credential: retained,
        secret: [3; 32],
    };
    (bootstrap, actual, member, owner)
}
fn challenge<'a>(actual: &'a HostAdvertisement) -> BorrowedPartReturnChallenge<'a> {
    BorrowedPartReturnChallenge {
        admission_id: "admission/return",
        body_id: "body/1",
        part_id: "part/1",
        host_id: actual.host_id.as_str(),
        boot_id: actual.boot_id.as_str(),
        offer_generation: actual.offer_generation.0,
        nonce: [8; 32],
        issued_at_millis: 100,
        expires_at_millis: 200,
    }
}
fn messages(
    bootstrap: &MemberBootstrap<'_>,
    actual: &HostAdvertisement,
    member: &SigningKey,
    owner: &SigningKey,
) -> Messages {
    let retained = bootstrap.admission.credential().unwrap();
    let mut credential = retained.clone();
    credential.boot_id = actual.boot_id.clone();
    let projection = HostAdvertisementImageProjection {
        context: actual.clone(),
        capability_images: vec![Sha256::digest(IMAGE).into()],
        authority_grants: vec![],
    };
    use ed25519_dalek::Signer;
    let signed = attest_member_return_image_acknowledgement(
        &MemberReturnImageAcknowledgement::new(credential, projection),
        bootstrap.owner.clone(),
        member.verifying_key().to_bytes(),
        |t| owner.sign(t).to_bytes(),
    )
    .unwrap();
    Messages {
        incoming: [
            serde_json::to_vec(&challenge(actual)).unwrap(),
            serde_json::to_vec(&signed.transport_metadata().unwrap()).unwrap(),
            signed.receipt_json.into_bytes(),
        ]
        .into(),
        sent: vec![],
    }
}
#[test]
fn retained_return_preserves_part_and_signs_exact_expiry_bearing_challenge() {
    let (bootstrap, actual, member, owner) = fixture();
    let retained = bootstrap.admission.credential().unwrap();
    let mut link = messages(&bootstrap, &actual, &member, &owner);
    let exchange = RetainedReturn {
        bootstrap: &bootstrap,
        actual: &actual,
        image_id: "image/1",
        key: &member,
        retained,
    };
    let accepted = ready(exchange.exchange(
        &mut link,
        [IMAGE].into_iter(),
        || Ok(vec![]),
        &mut [0; 4096],
        &mut [0; 4096],
    ))
    .unwrap();
    assert_eq!(accepted.credential.body_id, retained.body_id);
    assert_eq!(accepted.credential.part_id, retained.part_id);
    assert_eq!(accepted.credential.boot_id, actual.boot_id);
    assert_eq!(retained.boot_id.as_str(), "previous/boot");
    let mut presentation_bytes = conduit_wire::BoundedDocumentAssembly::new(16 * 1024, 32);
    let mut frame = 0;
    loop {
        let complete = presentation_bytes.push(&link.sent[frame]).unwrap();
        frame += 1;
        if complete { break; }
    }
    assert_eq!(link.sent.len(), frame + 2);
    let presentation: MemberReturnImagePresentation =
        serde_json::from_slice(presentation_bytes.completed().unwrap().0).unwrap();
    presentation
        .verify(&member.verifying_key().to_bytes())
        .unwrap();
    let mut output = [0; 128];
    let mut document = conduit_wire::BorrowedBoundedDocumentAssembly::new(&mut output, 1);
    assert!(document.push(&link.sent[frame]).unwrap());
    assert_eq!(document.completed().unwrap().0, IMAGE);
    let proof: serde_json::Value = serde_json::from_slice(&link.sent[frame + 1]).unwrap();
    let signature: Vec<u8> = serde_json::from_value(proof["signature"].clone()).unwrap();
    member
        .verifying_key()
        .verify_strict(
            &challenge(&actual).signing_transcript(),
            &Signature::from_slice(&signature).unwrap(),
        )
        .unwrap();
    let mut changed = challenge(&actual);
    changed.expires_at_millis += 1;
    assert!(member
        .verifying_key()
        .verify_strict(
            &changed.signing_transcript(),
            &Signature::from_slice(&signature).unwrap()
        )
        .is_err());
}
#[test]
fn stale_challenge_or_invalid_expiry_refuses_before_return_proof() {
    let (bootstrap, actual, member, owner) = fixture();
    let retained = bootstrap.admission.credential().unwrap();
    let exchange = RetainedReturn {
        bootstrap: &bootstrap,
        actual: &actual,
        image_id: "image/1",
        key: &member,
        retained,
    };
    for stale_boot in [true, false] {
        let mut link = messages(&bootstrap, &actual, &member, &owner);
        let mut request = challenge(&actual);
        if stale_boot {
            request.boot_id = "old/boot";
        } else {
            request.expires_at_millis = request.issued_at_millis;
        }
        link.incoming[0] = serde_json::to_vec(&request).unwrap();
        assert!(matches!(
            ready(exchange.exchange(
                &mut link,
                [IMAGE].into_iter(),
                || Ok(vec![]),
                &mut [0; 4096],
                &mut [0; 4096]
            )),
            Err(Refusal::Challenge)
        ));
        assert_eq!(link.sent.len(), 2);
    }
}
#[test]
fn lost_owner_ack_or_wrong_owner_key_cannot_install_returned_membership() {
    let (bootstrap, actual, member, owner) = fixture();
    let retained = bootstrap.admission.credential().unwrap();
    let exchange = RetainedReturn {
        bootstrap: &bootstrap,
        actual: &actual,
        image_id: "image/1",
        key: &member,
        retained,
    };
    let mut interrupted = messages(&bootstrap, &actual, &member, &owner);
    interrupted.incoming.truncate(1);
    assert!(matches!(
        ready(exchange.exchange(
            &mut interrupted,
            [IMAGE].into_iter(),
            || Ok(vec![]),
            &mut [0; 4096],
            &mut [0; 4096]
        )),
        Err(Refusal::Transport)
    ));
    let wrong = SigningKey::from_bytes(&[6; 32]);
    let mut forged = messages(&bootstrap, &actual, &member, &wrong);
    assert!(matches!(
        ready(exchange.exchange(
            &mut forged,
            [IMAGE].into_iter(),
            || Ok(vec![]),
            &mut [0; 4096],
            &mut [0; 4096]
        )),
        Err(Refusal::Attestation)
    ));
    assert_eq!(retained.boot_id.as_str(), "previous/boot");
}
#[test]
fn changed_image_or_retained_body_refuses_before_carrier_effects() {
    let (bootstrap, actual, member, owner) = fixture();
    let retained = bootstrap.admission.credential().unwrap();
    let mut link = messages(&bootstrap, &actual, &member, &owner);
    let mut exchange = RetainedReturn {
        bootstrap: &bootstrap,
        actual: &actual,
        image_id: "other/image",
        key: &member,
        retained,
    };
    assert!(matches!(
        ready(exchange.exchange(
            &mut link,
            [IMAGE].into_iter(),
            || Ok(vec![]),
            &mut [0; 4096],
            &mut [0; 4096]
        )),
        Err(Refusal::Scope)
    ));
    let mut changed = retained.clone();
    changed.body_id = serde_json::from_str(r#""other/body""#).unwrap();
    exchange.image_id = "image/1";
    exchange.retained = &changed;
    assert!(matches!(
        ready(exchange.exchange(
            &mut link,
            [IMAGE].into_iter(),
            || Ok(vec![]),
            &mut [0; 4096],
            &mut [0; 4096]
        )),
        Err(Refusal::Scope)
    ));
    assert!(link.sent.is_empty());
}

#[test]
fn excess_images_refuse_before_native_grant_copy_or_transport() {
    let (bootstrap, actual, member, owner) = fixture();
    let retained = bootstrap.admission.credential().unwrap();
    let exchange = RetainedReturn {
        bootstrap: &bootstrap,
        actual: &actual,
        image_id: "image/1",
        key: &member,
        retained,
    };
    let mut link = messages(&bootstrap, &actual, &member, &owner);
    let copied = core::cell::Cell::new(false);
    assert!(matches!(
        ready(exchange.exchange(
            &mut link,
            [IMAGE; conduit_body::MAX_MEMBER_RETURN_IMAGES + 1].into_iter(),
            || {
                copied.set(true);
                Ok(vec![])
            },
            &mut [0; 4096],
            &mut [0; 4096]
        )),
        Err(Refusal::Bounds)
    ));
    assert!(!copied.get());
    assert!(link.sent.is_empty());
}
