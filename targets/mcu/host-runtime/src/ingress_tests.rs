//! Signed input custody over ordinary bounded chunks. Fake keys/nonces/time;
//! this does not prove current membership, a prepared workset, or execution.
use crate::{
    ingress::{self, Refusal},
    membership_tests::{fixture, receipt},
    tests::ready,
    transport::{DocumentTransport, MessageTransport},
};
use conduit_body::{
    check_host_workset_preparation_delegation, host_command_attribution_transcript,
    HostCommandChallenge, HostCommandInput, HostPreparationAuthorizer, HostPreparationDelegation,
    NativePreparationContext, PreparationDelegationRefusal, HOST_PREPARED_WORKSET_CONTRACT,
};
use conduit_wire::{
    encode_bounded_document_chunk, BorrowedBoundedDocumentAssembly, BOUNDED_DOCUMENT_HEADER_BYTES,
};
use core::{
    future::Future,
    task::{Context, Waker},
};
use ed25519_dalek::{Signer, SigningKey};
use sha2::{Digest, Sha256};
use std::{string::String, vec::Vec};
const PAYLOAD: &[u8] =
    b"opaque signed workset bytes: ingress authenticates but never decodes these";
const NOW: u64 = 100;
fn policy() -> HostPreparationDelegation {
    HostPreparationDelegation {
        authorizer: HostPreparationAuthorizer::SeedOwner,
        maximum_assignment_bytes: 2048,
        maximum_runtime_state_bytes: 4096,
    }
}
struct Link<'a> {
    owner: &'a SigningKey,
    payload: &'a [u8],
    signature: Option<[u8; 64]>,
    sent: Vec<Vec<u8>>,
    received: usize,
    documents: usize,
    bad_signature: bool,
    corrupt_document: bool,
    lose_ack: bool,
    pause_document: bool,
}
impl<'a> Link<'a> {
    fn new(owner: &'a SigningKey) -> Self {
        Self {
            owner,
            payload: PAYLOAD,
            signature: None,
            sent: Vec::new(),
            received: 0,
            documents: 0,
            bad_signature: false,
            corrupt_document: false,
            lose_ack: false,
            pause_document: false,
        }
    }
    fn effects(&self) -> (usize, usize, usize) {
        (self.sent.len(), self.received, self.documents)
    }
}
impl MessageTransport for Link<'_> {
    type Error = ();
    async fn send(&mut self, bytes: &[u8]) -> Result<(), ()> {
        self.sent.push(bytes.to_vec());
        if let Ok((challenge, used)) =
            serde_json_core::from_slice::<HostCommandChallenge<'_>>(bytes)
        {
            assert_eq!(used, bytes.len());
            assert_eq!(challenge.payload_contract, HOST_PREPARED_WORKSET_CONTRACT);
            assert_eq!(challenge.maximum_payload_bytes, 2048);
            assert_eq!(challenge.issued_at_millis, NOW);
            assert_eq!(
                challenge.expires_at_millis,
                NOW + u64::from(ingress::LIFETIME_MILLIS)
            );
            let transcript = host_command_attribution_transcript(&challenge, self.payload).unwrap();
            let mut signature = self.owner.sign(&transcript).to_bytes();
            if self.bad_signature {
                signature[0] ^= 1;
            }
            self.signature = Some(signature);
            Ok(())
        } else {
            let report: serde_json::Value = serde_json::from_slice(bytes).unwrap();
            assert_eq!(report["phase"], "input-retained");
            if self.lose_ack {
                Err(())
            } else {
                Ok(())
            }
        }
    }
    async fn receive<'a>(&mut self, output: &'a mut [u8]) -> Result<&'a [u8], ()> {
        self.received += 1;
        let signature = self.signature.ok_or(())?;
        output.get_mut(..64).ok_or(())?.copy_from_slice(&signature);
        Ok(&output[..64])
    }
}
impl DocumentTransport for Link<'_> {
    async fn receive_document(
        &mut self,
        document: &mut BorrowedBoundedDocumentAssembly<'_>,
    ) -> Result<(), ()> {
        self.documents += 1;
        let digest: [u8; 32] = Sha256::digest(self.payload).into();
        let mut offset = 0;
        let mut frame = [0; 64];
        while offset < self.payload.len() {
            let (length, next) =
                encode_bounded_document_chunk(&mut frame, self.payload, offset, &digest).unwrap();
            if self.corrupt_document && offset == 0 {
                frame[BOUNDED_DOCUMENT_HEADER_BYTES] ^= 1;
            }
            let complete = document.push(&frame[..length]).map_err(|_| ())?;
            if self.pause_document {
                std::future::pending::<()>().await;
            }
            offset = next;
            assert_eq!(complete, offset == self.payload.len());
        }
        Ok(())
    }
}
fn report(bytes: &[u8], digest: &[u8; 32]) {
    let value: serde_json::Value = serde_json::from_slice(bytes).unwrap();
    assert_eq!(value["phase"], "input-retained");
    assert_eq!(value["custody"], "ram");
    assert_eq!(value["resources"], "not-admitted");
    assert_eq!(value["start"], "not-started");
    let hex: String = digest.iter().map(|b| std::format!("{b:02x}")).collect();
    assert_eq!(value["payload_sha256"], hex);
}
#[test]
fn valid_signed_workset_and_lost_ack_keep_the_same_ram_custody() {
    let (bootstrap, actual, _, owner) = fixture();
    let admitted = receipt(&bootstrap, &actual); // Independently supplied test context only.
    let context = NativePreparationContext {
        bootstrap: &bootstrap,
        member: &admitted.credential,
        actual: &actual,
        image_id: "image/1",
        configuration_id: "configuration/1",
        now_millis: NOW,
    };
    let policy = policy();
    for lose_ack in [false, true] {
        let mut link = Link::new(&owner);
        link.lose_ack = lose_ack;
        let mut input = HostCommandInput::<2048>::new();
        assert_eq!(
            ready(ingress::receive(
                &mut link,
                &mut input,
                &policy,
                context,
                [9; 32],
                || NOW + 1
            )),
            if lose_ack {
                Err(Refusal::Carrier)
            } else {
                Ok(())
            }
        );
        assert!(input.is_retained());
        assert_eq!(input.payload(), Some(PAYLOAD));
        let digest: [u8; 32] = Sha256::digest(PAYLOAD).into();
        assert_eq!(input.payload_digest(), Some(&digest));
        assert!(input.writable().is_err());
        assert_eq!(link.effects(), (2, 1, 1));
        report(&link.sent[1], &digest);
        let original_ack = link.sent[1].clone();
        let effects = link.effects();
        assert_eq!(
            ready(ingress::receive(
                &mut link,
                &mut input,
                &policy,
                context,
                [10; 32],
                || NOW + 2
            )),
            Err(Refusal::Scope)
        );
        assert_eq!(link.effects(), effects);
        assert_eq!(input.payload(), Some(PAYLOAD));
        link.lose_ack = false;
        ready(ingress::report(&mut link, &input)).unwrap();
        assert_eq!(link.sent.last().unwrap(), &original_ack);
        assert_eq!(input.payload_digest(), Some(&digest));
        let command = input
            .reborrow(
                &admitted.credential,
                &actual,
                &bootstrap.owner,
                HOST_PREPARED_WORKSET_CONTRACT,
                NOW + 2,
            )
            .unwrap();
        assert_eq!(
            check_host_workset_preparation_delegation(None, context, &command),
            Err(PreparationDelegationRefusal::MissingDelegation)
        );
        let too_small = HostPreparationDelegation {
            maximum_assignment_bytes: 1,
            ..policy
        };
        assert_eq!(
            check_host_workset_preparation_delegation(Some(&too_small), context, &command),
            Err(PreparationDelegationRefusal::Bounds)
        );
    }
}
#[test]
fn signature_expiry_corruption_policy_and_cancellation_never_retain_input() {
    let (bootstrap, actual, _, owner) = fixture();
    let admitted = receipt(&bootstrap, &actual);
    let context = NativePreparationContext {
        bootstrap: &bootstrap,
        member: &admitted.credential,
        actual: &actual,
        image_id: "image/1",
        configuration_id: "configuration/1",
        now_millis: NOW,
    };
    let policy = policy();
    for (bad_signature, corrupt_document, checked_now, expected) in [
        (true, false, NOW + 1, Refusal::Authentication),
        (
            false,
            false,
            NOW + u64::from(ingress::LIFETIME_MILLIS),
            Refusal::Authentication,
        ),
        (false, false, NOW - 1, Refusal::Authentication),
        (false, true, NOW + 1, Refusal::Document),
    ] {
        let mut input = HostCommandInput::<2048>::new();
        let mut link = Link::new(&owner);
        link.bad_signature = bad_signature;
        link.corrupt_document = corrupt_document;
        assert_eq!(
            ready(ingress::receive(
                &mut link,
                &mut input,
                &policy,
                context,
                [9; 32],
                || checked_now
            )),
            Err(expected)
        );
        assert!(!input.is_retained());
        assert!(input.payload_digest().is_none());
        assert!(input.writable().is_ok());
        assert_eq!(link.sent.len(), 1);
    }
    for invalid in [
        HostPreparationDelegation {
            maximum_assignment_bytes: 0,
            ..policy
        },
        HostPreparationDelegation {
            maximum_runtime_state_bytes: 0,
            ..policy
        },
    ] {
        let mut input = HostCommandInput::<2048>::new();
        let mut link = Link::new(&owner);
        assert_eq!(
            ready(ingress::receive(
                &mut link,
                &mut input,
                &invalid,
                context,
                [9; 32],
                || NOW + 1
            )),
            Err(Refusal::Scope)
        );
        assert_eq!(link.effects(), (0, 0, 0));
        assert!(!input.is_retained());
    }
    let mut input = HostCommandInput::<2048>::new();
    let mut link = Link::new(&owner);
    let wrong_image = NativePreparationContext {
        image_id: "wrong-image",
        ..context
    };
    assert_eq!(
        ready(ingress::receive(
            &mut link,
            &mut input,
            &policy,
            wrong_image,
            [9; 32],
            || NOW + 1
        )),
        Err(Refusal::Policy)
    );
    assert!(!input.is_retained());
    assert_eq!(link.sent.len(), 1);
    let mut input = HostCommandInput::<2048>::new();
    let mut link = Link::new(&owner);
    link.pause_document = true;
    {
        let mut future = core::pin::pin!(ingress::receive(
            &mut link,
            &mut input,
            &policy,
            context,
            [9; 32],
            || NOW + 1
        ));
        assert!(future
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_pending());
    }
    assert!(!input.is_retained());
    assert!(input.writable().is_ok());
    assert_eq!(link.effects(), (1, 0, 1));
    // An explicit later receive can reuse unretained backing with a fresh nonce.
    let mut retry = Link::new(&owner);
    ready(ingress::receive(
        &mut retry,
        &mut input,
        &policy,
        context,
        [10; 32],
        || NOW + 1,
    ))
    .unwrap();
    assert_eq!(input.payload(), Some(PAYLOAD));
}

#[test]
fn signed_command_for_another_actual_profile_cannot_use_installed_policy() {
    let (bootstrap, mut actual, _, owner) = fixture();
    actual.profile = "configuration/other".into();
    let admitted = receipt(&bootstrap, &actual);
    let context = NativePreparationContext {
        bootstrap: &bootstrap,
        member: &admitted.credential,
        actual: &actual,
        image_id: "image/1",
        configuration_id: "configuration/1",
        now_millis: NOW,
    };
    // The signer signs the issued challenge for the actual profile correctly.
    // Signature validity must not retarget the installed configuration/policy.
    let mut input = HostCommandInput::<2048>::new();
    let mut link = Link::new(&owner);
    assert_eq!(
        ready(ingress::receive(
            &mut link,
            &mut input,
            &policy(),
            context,
            [9; 32],
            || NOW + 1,
        )),
        Err(Refusal::Policy)
    );
    assert!(!input.is_retained());
    assert!(input.payload_digest().is_none());
}
