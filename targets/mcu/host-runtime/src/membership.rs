//! One authenticated spawn exchange shared by native MCU Host compositions.
use crate::transport::MessageTransport;
use conduit_body::{
    ambient_admission_transcript, validate_attested_admission_receipt_basis,
    BorrowedAdmissionReceiptAttestation, MemberBootstrap, MembershipCredential,
    PicoAdmissionAdvertisement, PicoSpawnJoinRequest, PortableAdmissionReceipt,
    PICO_ADMISSION_PROTOCOL, PICO_SPAWN_PROTOCOL,
};
use conduit_core::HostAdvertisement;
use ed25519_dalek::{Signer, SigningKey};
use sha2::{Digest, Sha256};

pub struct AcceptedMembership {
    pub credential: MembershipCredential,
    pub receipt_digest: [u8; 32],
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    RetainedMember,
    Scope,
    Encoding,
    Transport,
    Attestation,
    Receipt,
}

/// Existing Root possession is borrowed, never replaced. The platform supplies
/// its actual current Boot advertisement; neither carrier reachability nor a
/// successful send is membership. Only an independently verified owner receipt
/// completes this exchange. A retained member must use canonical Part return.
pub struct SpawnAdmission<'a, 's> {
    pub bootstrap: &'a MemberBootstrap<'s>,
    pub actual: &'a HostAdvertisement,
    /// Actual checked running IMAGE, supplied independently of the bootstrap.
    pub image_id: &'a str,
    pub key: &'a SigningKey,
    pub current: Option<&'a MembershipCredential>,
    pub freshness: u64,
    pub friendly_label: &'a str,
}
impl SpawnAdmission<'_, '_> {
    pub async fn exchange<T: MessageTransport>(
        &self,
        link: &mut T,
        input: &mut [u8],
        output: &mut [u8],
    ) -> Result<AcceptedMembership, Refusal> {
        if self.bootstrap.admission.credential().is_some() || self.current.is_some() {
            return Err(Refusal::RetainedMember);
        }
        let provision = self.bootstrap.admission.spawn().ok_or(Refusal::Scope)?;
        if self.freshness == 0
            || self.actual.host_id.as_str() != self.bootstrap.host_id
            || self.actual.profile.as_str() != self.bootstrap.configuration_id
            || self.actual.boot_id.as_str().is_empty()
            || self.actual.offer_generation.0 == 0
            || self.key.verifying_key()
                != SigningKey::from_bytes(self.bootstrap.admission.secret()).verifying_key()
            || !self.bootstrap.matches_native(
                self.actual.host_id.as_str(),
                self.image_id,
                self.actual.profile.as_str(),
            )
        {
            return Err(Refusal::Scope);
        }
        let arrival = PicoAdmissionAdvertisement {
            protocol: PICO_ADMISSION_PROTOCOL,
            advertisement: self.actual.clone(),
            friendly_label: self.friendly_label.into(),
            verifying_key: self.key.verifying_key().to_bytes().to_vec(),
            freshness_sequence: self.freshness,
        };
        let n = serde_json_core::to_slice(&arrival, output).map_err(|_| Refusal::Encoding)?;
        link.send(&output[..n])
            .await
            .map_err(|_| Refusal::Transport)?;
        let transcript = ambient_admission_transcript(
            "spawn-invitation-v1",
            provision.body_id,
            provision.invitation_id,
            self.actual.host_id.as_str(),
            self.actual.boot_id.as_str(),
            self.actual.offer_generation,
            &provision.nonce,
            provision.expires_at_millis,
        );
        let signature = self.key.sign(&transcript).to_bytes();
        let proof = PicoSpawnJoinRequest {
            protocol: PICO_SPAWN_PROTOCOL,
            spore_id: provision.spore_id,
            image_id: provision.image_id,
            invitation_id: provision.invitation_id,
            body_id: provision.body_id,
            host_id: self.actual.host_id.as_str(),
            boot_id: self.actual.boot_id.as_str(),
            offer_generation: self.actual.offer_generation.0,
            nonce: provision.nonce,
            signature: &signature,
        };
        let n = serde_json_core::to_slice(&proof, output).map_err(|_| Refusal::Encoding)?;
        link.send(&output[..n])
            .await
            .map_err(|_| Refusal::Transport)?;
        let bytes = link.receive(input).await.map_err(|_| Refusal::Transport)?;
        let (metadata, used) =
            serde_json_core::from_slice::<BorrowedAdmissionReceiptAttestation<'_>>(bytes)
                .map_err(|_| Refusal::Encoding)?;
        if used != bytes.len() {
            return Err(Refusal::Encoding);
        }
        let raw = link.receive(output).await.map_err(|_| Refusal::Transport)?;
        metadata
            .verify(
                raw,
                &self.bootstrap.owner,
                &self.key.verifying_key().to_bytes(),
            )
            .map_err(|_| Refusal::Attestation)?;
        let (receipt, used) = serde_json_core::from_slice::<PortableAdmissionReceipt>(raw)
            .map_err(|_| Refusal::Encoding)?;
        if used != raw.len() {
            return Err(Refusal::Encoding);
        }
        validate_attested_admission_receipt_basis(&receipt, &self.bootstrap.body_id, self.actual)
            .map_err(|_| Refusal::Receipt)?;
        let accepted = AcceptedMembership {
            credential: receipt.credential,
            receipt_digest: Sha256::digest(raw).into(),
        };
        input.fill(0);
        output.fill(0);
        Ok(accepted)
    }
}
