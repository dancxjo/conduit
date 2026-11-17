//! Hosted protocol peer only. It verifies signatures but performs no Core commit.
use super::*;
use conduit_body::{
    attest_member_return_image_acknowledgement, HostAdvertisementImageProjection,
    MemberReturnImageAcknowledgement, MemberReturnImagePresentation, PartReturnChallenge,
    SignedPortableAdmissionReceipt,
};
use conduit_wire::{BorrowedBoundedDocumentAssembly, BoundedDocumentAssembly};
use ed25519_dalek::Signature;
use serde::Deserialize;
#[derive(Clone, Copy, Default)]
pub enum Fault {
    #[default]
    None,
    Owner,
    Projection,
    Part,
    Boot,
    ChallengePart,
    ChallengeBoot,
    LostAck,
}
pub struct Fixture {
    pub bootstrap: MemberBootstrap<'static>,
    pub actual: HostAdvertisement,
    pub member: SigningKey,
    pub owner: SigningKey,
    pub retained: MembershipCredential,
    pub grants: Vec<AuthorityGrant>,
}
impl Fixture {
    pub fn new() -> Self {
        let (mut bootstrap, actual, member, owner) = crate::membership_tests::fixture();
        let mut retained = crate::membership_tests::receipt(&bootstrap, &actual).credential;
        retained.boot_id = "fixture/prior-boot".into();
        bootstrap.admission = conduit_body::MemberBootstrapMaterial::Retained {
            credential: retained.clone(),
            secret: [3; 32],
        };
        let grants = vec![AuthorityGrant {
            grant_id: "fixture/grant".into(),
            contract_id: "fixture/authority@1".into(),
            host_call_contract_id: "fixture/call@1".into(),
            subject_kind: "fixture/sample".into(),
            host_id: actual.host_id.clone(),
            boot_id: actual.boot_id.clone(),
            capability_id: "fixture/sampler".into(),
        }];
        Self {
            bootstrap,
            actual,
            member,
            owner,
            retained,
            grants,
        }
    }
    pub fn returning(&self) -> RetainedReturn<'_, 'static> {
        RetainedReturn {
            bootstrap: &self.bootstrap,
            actual: &self.actual,
            image_id: "image/1",
            key: &self.member,
            retained: &self.retained,
        }
    }
}
#[derive(Deserialize)]
struct Proof {
    admission_id: conduit_body::AdmissionId,
    body_id: conduit_body::BodyId,
    part_id: conduit_body::PartId,
    host_id: conduit_core::HostId,
    boot_id: conduit_core::BootId,
    nonce: [u8; 32],
    signature: Vec<u8>,
}
pub struct Owner<'a> {
    fixture: &'a Fixture,
    images: &'a [&'a [u8]],
    fault: Fault,
    incoming: BoundedDocumentAssembly,
    documents: Vec<Vec<u8>>,
    challenge: PartReturnChallenge,
    acknowledgement: SignedPortableAdmissionReceipt,
    pub sends: usize,
    receives: usize,
    pub proof_verified: bool,
}
impl<'a> Owner<'a> {
    pub fn new(fixture: &'a Fixture, images: &'a [&'a [u8]], fault: Fault) -> Self {
        let mut challenge = PartReturnChallenge {
            admission_id: serde_json::from_str(r#""fixture/admission""#).unwrap(),
            body_id: fixture.retained.body_id.clone(),
            part_id: fixture.retained.part_id.clone(),
            host_id: fixture.actual.host_id.clone(),
            boot_id: fixture.actual.boot_id.clone(),
            offer_generation: fixture.actual.offer_generation,
            nonce: [9; 32],
            issued_at_millis: 1000,
            expires_at_millis: 2000,
        };
        if matches!(fault, Fault::ChallengePart) {
            challenge.part_id = serde_json::from_str(r#""wrong/part""#).unwrap();
        }
        if matches!(fault, Fault::ChallengeBoot) {
            challenge.boot_id = fixture.retained.boot_id.clone();
        }
        let mut projection = HostAdvertisementImageProjection {
            context: fixture.actual.clone(),
            capability_images: images
                .iter()
                .map(|bytes| Sha256::digest(bytes).into())
                .collect(),
            authority_grants: fixture.grants.clone(),
        };
        if matches!(fault, Fault::Projection) {
            projection.capability_images.reverse();
        }
        let mut credential = fixture.retained.clone();
        credential.boot_id = fixture.actual.boot_id.clone();
        credential.credential_id =
            serde_json::from_str(r#""fixture/returned-credential""#).unwrap();
        credential.issued_at_millis = 1001;
        if matches!(fault, Fault::Part) {
            credential.part_id = serde_json::from_str(r#""wrong/part""#).unwrap();
        }
        if matches!(fault, Fault::Boot) {
            credential.boot_id = fixture.retained.boot_id.clone();
        }
        let receipt = MemberReturnImageAcknowledgement::new(credential, projection);
        let mut peer = fixture.bootstrap.owner.clone();
        if matches!(fault, Fault::Owner) {
            peer.boot_id = "wrong/owner-boot".into();
        }
        // Signing only authenticates these bytes; this fixture makes no durable
        // Core-admission or recording assertion.
        let acknowledgement = if matches!(fault, Fault::Boot) {
            // The normal attester rejects a stale Boot itself. Produce an exact
            // signed invalid document via the public transcript, so the receiver
            // still has to enforce the relationship after signature verification.
            signed_raw(&receipt, peer, fixture)
        } else {
            attest_member_return_image_acknowledgement(
                &receipt,
                peer,
                fixture.member.verifying_key().to_bytes(),
                |t| fixture.owner.sign(t).to_bytes(),
            )
            .unwrap()
        };
        Self {
            fixture,
            images,
            fault,
            incoming: BoundedDocumentAssembly::new(4096, 64),
            documents: Vec::new(),
            challenge,
            acknowledgement,
            sends: 0,
            receives: 0,
            proof_verified: false,
        }
    }
    fn verify_presentation(&self) {
        assert_eq!(self.documents.len(), self.images.len() + 1);
        let presentation: MemberReturnImagePresentation =
            serde_json::from_slice(&self.documents[0]).unwrap();
        presentation
            .verify(&self.fixture.member.verifying_key().to_bytes())
            .unwrap();
        assert_eq!(presentation.credential, self.fixture.retained);
        assert_eq!(presentation.projection.context, self.fixture.actual);
        assert_eq!(
            presentation.projection.authority_grants,
            self.fixture.grants
        );
        for (index, bytes) in self.images.iter().enumerate() {
            assert_eq!(self.documents[index + 1].as_slice(), *bytes);
            let digest: [u8; 32] = Sha256::digest(bytes).into();
            assert_eq!(presentation.projection.capability_images[index], digest);
        }
    }
}
fn signed_raw(
    receipt: &MemberReturnImageAcknowledgement,
    peer: conduit_body::FederationPeer,
    fixture: &Fixture,
) -> SignedPortableAdmissionReceipt {
    let receipt_json = serde_json::to_string(receipt).unwrap();
    let member_verifying_key = fixture.member.verifying_key().to_bytes();
    let transcript = conduit_body::admission_receipt_attestation_transcript(
        receipt_json.as_bytes(),
        peer.host_id.as_str(),
        peer.boot_id.as_str(),
        peer.offer_generation.0,
        &peer.verifying_key,
        &member_verifying_key,
    )
    .unwrap();
    SignedPortableAdmissionReceipt {
        schema: conduit_body::SIGNED_ADMISSION_RECEIPT_SCHEMA.into(),
        owner: peer,
        member_verifying_key,
        signature: fixture.owner.sign(&transcript).to_bytes().to_vec(),
        receipt_json,
    }
}
impl MessageTransport for Owner<'_> {
    type Error = ();
    async fn send(&mut self, bytes: &[u8]) -> Result<(), ()> {
        self.sends += 1;
        if bytes.starts_with(b"CBD1") {
            if self.incoming.push(bytes).map_err(|_| ())? {
                self.documents
                    .push(self.incoming.completed().unwrap().0.to_vec());
                self.incoming = BoundedDocumentAssembly::new(4096, 64);
            }
        } else {
            let proof: Proof = serde_json::from_slice(bytes).unwrap();
            assert_eq!(proof.admission_id, self.challenge.admission_id);
            assert_eq!(proof.body_id, self.challenge.body_id);
            assert_eq!(proof.part_id, self.challenge.part_id);
            assert_eq!(proof.host_id, self.challenge.host_id);
            assert_eq!(proof.boot_id, self.challenge.boot_id);
            assert_eq!(proof.nonce, self.challenge.nonce);
            // This fixture's owner clock decides expiry; the MCU never guesses
            // conversion of an owner's expiry into its unrelated local clock.
            assert!(
                self.challenge.issued_at_millis <= 1001 && 1001 < self.challenge.expires_at_millis
            );
            self.fixture
                .member
                .verifying_key()
                .verify_strict(
                    &self.challenge.signing_transcript(),
                    &Signature::from_slice(&proof.signature).unwrap(),
                )
                .unwrap();
            self.proof_verified = true;
        }
        Ok(())
    }
    async fn receive<'a>(&mut self, output: &'a mut [u8]) -> Result<&'a [u8], ()> {
        self.receives += 1;
        let bytes = if self.receives == 1 {
            self.verify_presentation();
            serde_json::to_vec(&self.challenge).unwrap()
        } else {
            assert!(self.proof_verified);
            serde_json::to_vec(&self.acknowledgement.transport_metadata().unwrap()).unwrap()
        };
        let n = bytes.len();
        output.get_mut(..n).ok_or(())?.copy_from_slice(&bytes);
        Ok(&output[..n])
    }
}
impl DocumentTransport for Owner<'_> {
    async fn receive_document(
        &mut self,
        document: &mut BorrowedBoundedDocumentAssembly<'_>,
    ) -> Result<(), ()> {
        if matches!(self.fault, Fault::LostAck) {
            return Err(());
        }
        let raw = self.acknowledgement.receipt_json.as_bytes();
        let digest = Sha256::digest(raw).into();
        let mut offset = 0;
        let mut frame = [0; 256];
        while offset < raw.len() {
            let (n, next) =
                conduit_wire::encode_bounded_document_chunk(&mut frame, raw, offset, &digest)
                    .unwrap();
            document.push(&frame[..n]).map_err(|_| ())?;
            offset = next;
        }
        Ok(())
    }
}
