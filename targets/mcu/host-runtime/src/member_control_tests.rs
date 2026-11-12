//! Membership-only return must leave original execution/resource custody intact.
use super::*;
use crate::{
    root::{Configuration, Root},
    seed::Seed,
};
use conduit_core as c;

fn carrier(bootstrap: &MemberBootstrap<'_>) -> Vec<u8> {
    let payload = serde_json::to_vec(bootstrap).unwrap();
    let mut bytes = b"CONDUIT_SEED@2\0\0".to_vec();
    bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&payload);
    bytes
}
fn root<'s>(bytes: &'s [u8], mut actual: HostAdvertisement) -> Root<'s, 16384> {
    actual.resources.push(c::ResourceOffer {
        pool_id: "pool/fixture".into(),
        class_id: "resource/memory".into(),
        capacity_units: 16,
        compute: None,
        content: None,
    });
    let seed = Seed::parse(bytes, "host/1", "image/1", "configuration/1").unwrap();
    let mut root = Root::new(
        seed,
        actual.clone(),
        Configuration {
            image_id: "image/1",
            configuration_id: "configuration/1",
            friendly_label: "fixture",
            preparation: None,
            execution: None,
        },
    )
    .unwrap();
    root.resources
        .admit(
            c::ResourceAdmissionRequest {
                plan_id: "plan/original".into(),
                placement_id: "placement/original".into(),
                host_id: actual.host_id.clone(),
                boot_id: actual.boot_id.clone(),
                offer_generation: actual.offer_generation,
                items: vec![c::ResourceAdmissionItem {
                    requirement: c::ResourceRequirement {
                        class_id: "resource/memory".into(),
                        units: 1,
                        protected_role: None,
                        compute: None,
                        content: None,
                    },
                    binding: c::ResourceBinding {
                        pool_id: "pool/fixture".into(),
                        class_id: "resource/memory".into(),
                        units: 1,
                        protected: None,
                        compute: None,
                        content: None,
                    },
                }],
            },
            &[c::ResourceObservation {
                host_id: actual.host_id.clone(),
                boot_id: actual.boot_id.clone(),
                offer_generation: actual.offer_generation,
                pool_id: "pool/fixture".into(),
                class_id: "resource/memory".into(),
                health: c::ResourceHealth::Ready,
                unreserved_units: 16,
                utilized_units: 0,
                sign_id: "sign/current".into(),
            }],
        )
        .unwrap();
    root
}
fn replace_ack(
    link: &mut Messages,
    bootstrap: &MemberBootstrap<'_>,
    member: &SigningKey,
    owner: &SigningKey,
    change: impl FnOnce(&mut MemberReturnImageAcknowledgement),
) {
    use ed25519_dalek::Signer;
    let mut assembly = conduit_wire::BoundedDocumentAssembly::new(16384, 32);
    for frame in link.incoming.iter().skip(2) {
        assembly.push(frame).unwrap();
    }
    let mut ack: MemberReturnImageAcknowledgement =
        serde_json::from_slice(assembly.completed().unwrap().0).unwrap();
    change(&mut ack);
    let signed = attest_member_return_image_acknowledgement(
        &ack,
        bootstrap.owner.clone(),
        member.verifying_key().to_bytes(),
        |t| owner.sign(t).to_bytes(),
    )
    .unwrap();
    let document = signed.receipt_json.as_bytes();
    let digest = Sha256::digest(document).into();
    link.incoming.truncate(1);
    link.incoming
        .push_back(serde_json::to_vec(&signed.transport_metadata().unwrap()).unwrap());
    let mut offset = 0;
    while offset < document.len() {
        let mut frame = [0; 4096];
        let (n, next) =
            conduit_wire::encode_bounded_document_chunk(&mut frame, document, offset, &digest)
                .unwrap();
        link.incoming.push_back(frame[..n].to_vec());
        offset = next;
    }
}
#[test]
fn canonical_member_control_updates_only_verified_receipt_and_preserves_reservations() {
    let (bootstrap, actual, member, owner) = fixture();
    let bytes = carrier(&bootstrap);
    for case in 0..7 {
        let mut root = root(&bytes, actual.clone());
        // Establish a real accepted receipt first; refusal must preserve both
        // nonempty membership fields, not merely leave an empty option empty.
        let mut initial = messages(&bootstrap, root.actual(), &member, &owner);
        {
            let (mut control, input) = root.member_control_with_workspace().unwrap();
            ready(control.return_member(
                &mut initial,
                [IMAGE].into_iter(),
                &[] as &[c::AuthorityGrant],
                input,
                &mut [0; 4096],
            ))
            .unwrap();
        }
        assert!(root.has_current_member());
        assert!(root.receipt_digest().is_some());
        let mut link = messages(&bootstrap, root.actual(), &member, &owner);
        match case {
            1 => root.key = SigningKey::from_bytes(&[9; 32]),
            2 => {
                link = messages(
                    &bootstrap,
                    root.actual(),
                    &member,
                    &SigningKey::from_bytes(&[8; 32]),
                )
            }
            3 => {
                let mut stale = challenge(root.actual());
                stale.boot_id = "stale/boot";
                link.incoming[0] = serde_json::to_vec(&stale).unwrap();
            }
            4 => replace_ack(&mut link, &bootstrap, &member, &owner, |ack| {
                ack.credential.part_id = serde_json::from_str("\"part/other\"").unwrap()
            }),
            5 => replace_ack(&mut link, &bootstrap, &member, &owner, |ack| {
                ack.projection.context.profile = "foreign/profile".into()
            }),
            6 => link.incoming.truncate(1),
            _ => {}
        }
        let original_resources = serde_json::to_vec(&root.resources).unwrap();
        let original_receipt = serde_json::to_vec(&root.receipt).unwrap();
        let original_digest = root.receipt_digest();
        let result = {
            let (mut control, workspace) = root.member_control_with_workspace().unwrap();
            assert_eq!(workspace.len(), 16384);
            let result = ready(control.return_member(
                &mut link,
                [IMAGE].into_iter(),
                &[] as &[c::AuthorityGrant],
                workspace,
                &mut [0; 4096],
            ));
            result
        };
        let expected = match case {
            0 => Ok(()),
            1 => Err(crate::root::Refusal::MemberReturn(Refusal::Scope)),
            2 => Err(crate::root::Refusal::MemberReturn(Refusal::Attestation)),
            3 => Err(crate::root::Refusal::MemberReturn(Refusal::Challenge)),
            4 | 5 => Err(crate::root::Refusal::MemberReturn(Refusal::Receipt)),
            6 => Err(crate::root::Refusal::MemberReturn(Refusal::Transport)),
            _ => unreachable!(),
        };
        assert_eq!(result, expected, "case {case}");
        assert_eq!(
            serde_json::to_vec(&root.resources).unwrap(),
            original_resources
        );
        assert_eq!(root.resources.admissions().len(), 1);
        if case == 0 {
            assert_eq!(
                root.receipt.as_ref().unwrap().part_id,
                bootstrap.admission.credential().unwrap().part_id
            );
            assert_eq!(root.receipt.as_ref().unwrap().boot_id, actual.boot_id);
            assert!(root.receipt_digest().is_some());
        } else {
            assert_eq!(serde_json::to_vec(&root.receipt).unwrap(), original_receipt);
            assert_eq!(root.receipt_digest(), original_digest);
        }
    }
}
struct PendingOwner;
impl MessageTransport for PendingOwner {
    type Error = ();
    async fn send(&mut self, _: &[u8]) -> Result<(), ()> {
        Ok(())
    }
    async fn receive<'a>(&mut self, _: &'a mut [u8]) -> Result<&'a [u8], ()> {
        core::future::pending().await
    }
}
impl DocumentTransport for PendingOwner {
    async fn receive_document(
        &mut self,
        _: &mut conduit_wire::BorrowedBoundedDocumentAssembly<'_>,
    ) -> Result<(), ()> {
        core::future::pending().await
    }
}
#[test]
fn cancelled_member_return_preserves_receipt_and_original_reservations() {
    use core::{
        future::Future,
        task::{Context, Poll, Waker},
    };
    let (bootstrap, actual, _, _) = fixture();
    let bytes = carrier(&bootstrap);
    let mut root = root(&bytes, actual);
    let mut initial = messages(
        &bootstrap,
        root.actual(),
        &SigningKey::from_bytes(&[3; 32]),
        &SigningKey::from_bytes(&[7; 32]),
    );
    {
        let (mut control, input) = root.member_control_with_workspace().unwrap();
        ready(control.return_member(
            &mut initial,
            [IMAGE].into_iter(),
            &[] as &[c::AuthorityGrant],
            input,
            &mut [0; 4096],
        ))
        .unwrap();
    }
    let digest = root.receipt_digest();
    assert!(digest.is_some());
    let receipt = serde_json::to_vec(&root.receipt).unwrap();
    let resources = serde_json::to_vec(&root.resources).unwrap();
    {
        let (mut control, workspace) = root.member_control_with_workspace().unwrap();
        let mut transport = PendingOwner;
        let mut output = [0; 4096];
        let future = control.return_member(
            &mut transport,
            [IMAGE].into_iter(),
            &[] as &[c::AuthorityGrant],
            workspace,
            &mut output,
        );
        let mut pinned = core::pin::pin!(future);
        assert!(matches!(
            pinned
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop())),
            Poll::Pending
        ));
    }
    assert_eq!(serde_json::to_vec(&root.receipt).unwrap(), receipt);
    assert_eq!(serde_json::to_vec(&root.resources).unwrap(), resources);
    assert_eq!(root.receipt_digest(), digest);
    assert!(!root.retained_input());
    // Cancellation relinquishes the borrow of the same original backing.
    assert_eq!(root.member_control_with_workspace().unwrap().1.len(), 16384);
}
#[test]
fn member_workspace_refuses_retained_command_without_releasing_its_custody() {
    use ed25519_dalek::Signer;
    let (bootstrap, actual, _, owner) = fixture();
    let bytes = carrier(&bootstrap);
    let mut root = root(&bytes, actual);
    let mut member = bootstrap.admission.credential().unwrap().clone();
    member.boot_id = root.actual().boot_id.clone();
    let payload = b"retained-command";
    let slot = IssuedHostCommandChallenge::issue(
        &member,
        root.actual(),
        &bootstrap.owner,
        HostCommandReceiveBounds {
            payload_contract: "fixture/command",
            maximum_payload_bytes: 64,
            lifetime_millis: 100,
        },
        [4; 32],
        10,
    )
    .unwrap();
    let signature = owner
        .sign(&host_command_attribution_transcript(slot.challenge(), payload).unwrap())
        .to_bytes();
    let stamp = slot
        .authenticate(payload, &signature, 11)
        .unwrap()
        .into_retained_stamp()
        .unwrap();
    root.input.writable().unwrap()[..payload.len()].copy_from_slice(payload);
    root.input.retain(stamp).unwrap();
    let resources = serde_json::to_vec(&root.resources).unwrap();
    assert!(matches!(
        root.member_control_with_workspace(),
        Err(crate::root::Refusal::RetainedInput)
    ));
    assert_eq!(root.input.payload(), Some(payload.as_slice()));
    assert!(root.retained_input());
    assert_eq!(serde_json::to_vec(&root.resources).unwrap(), resources);
    // Membership-only borrowing with independently funded scratch remains legal;
    // it cannot make the retained command writable.
    assert_eq!(root.member_control().actual().host_id.as_str(), "host/1");
}
