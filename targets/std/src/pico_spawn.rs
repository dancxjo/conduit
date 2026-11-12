//! Exact local USB provisioning and observation for a self-joining Pico spore.

use std::time::Duration;

use conduit_body::{
    BodyId, SpawnAdmissionProof, SpawnInvitationClaim, SpawnInvitationId,
    MAX_PICO_ADMISSION_FRAME_BYTES, PICO_SPAWN_PROTOCOL, PICO_SPAWN_REQUEST,
};
use conduit_core::{BootId, HostAdvertisement, HostId, OfferGeneration};
use serde::{Deserialize, Serialize};

use crate::pico_admission::{decode_advertisement, PicoAdmissionTransportError};
use crate::usb_cdc::NativePathCdcLine;
mod continuation;
pub use continuation::{ContinuationInstallTransportError, UnverifiedContinuationPrefix};
mod basis;
pub use basis::PicoSpawnAdmissionBasis;
mod fore_carriage;
mod member_return;
mod offer_images;
mod preparation;
pub use fore_carriage::{
    ForeCarriageBounds, ForeCarriageError, ForeCarriageFrame, PicoForeCarriage,
    PreparedForeCarriage,
};
pub use member_return::{MemberReturnObservation, MemberReturnPresentation, PicoMemberReturnBasis};
pub use offer_images::resolve_member_offer_images;
pub use preparation::{
    PreparationCommandBytes, PreparationExchange, PreparationTransportError,
    ReportedPreparationInput,
};

#[derive(Debug)]
pub enum PicoSpawnTransportError {
    Admission(PicoAdmissionTransportError),
    Malformed,
    WrongProtocol,
    WrongSpore,
    WrongImage,
    WrongInvitation,
    WrongBody,
    WrongAdvertisement,
    Oversized,
    InvalidReceipt,
}

impl From<PicoAdmissionTransportError> for PicoSpawnTransportError {
    fn from(error: PicoAdmissionTransportError) -> Self {
        Self::Admission(error)
    }
}

impl From<crate::usb_cdc::NativeUsbCdcError> for PicoSpawnTransportError {
    fn from(error: crate::usb_cdc::NativeUsbCdcError) -> Self {
        Self::Admission(error.into())
    }
}

pub struct PicoSpawnObservation {
    pub advertisement: HostAdvertisement,
    pub verifying_key: [u8; 32],
    pub proof: SpawnAdmissionProof,
    pub spore_id: String,
    pub image_id: String,
}

/// Locally selected physical carrier behavior; never membership authority.
#[derive(Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProvisioningCarrier {
    #[default]
    UsbCdc,
    ForwardedTty,
}
impl ProvisioningCarrier {
    pub fn open(self, path: &str) -> Result<PicoSpawnSocket, PicoSpawnTransportError> {
        let line = match self {
            Self::UsbCdc => NativePathCdcLine::open(path, MAX_PICO_ADMISSION_FRAME_BYTES)?,
            Self::ForwardedTty => {
                NativePathCdcLine::open_forwarded_tty(path, MAX_PICO_ADMISSION_FRAME_BYTES)?
            }
        };
        Ok(PicoSpawnSocket { line })
    }
}

pub struct PicoSpawnSocket {
    line: NativePathCdcLine,
}

impl PicoSpawnSocket {
    pub fn open(path: &str) -> Result<Self, PicoSpawnTransportError> {
        Ok(Self {
            line: NativePathCdcLine::open(path, MAX_PICO_ADMISSION_FRAME_BYTES)?,
        })
    }

    /// Deliver an already committed receipt over two bounded raw USB frames.
    /// Verification requires independent owner/member pins and actual observed
    /// equipment. Successful writes do not prove MCU acceptance or start Play.
    pub fn deliver_admission_receipt(
        &mut self,
        signed: &conduit_body::SignedPortableAdmissionReceipt,
        expected_owner: &conduit_body::FederationPeer,
        expected_member_key: &[u8; 32],
        expected_body: &BodyId,
        actual_advertisement: &HostAdvertisement,
        timeout: Duration,
    ) -> Result<(), PicoSpawnTransportError> {
        let metadata = checked_receipt_frames(
            signed,
            expected_owner,
            expected_member_key,
            expected_body,
            actual_advertisement,
        )?;
        self.line.send_raw_stream_frame(&metadata, timeout)?;
        self.line
            .send_raw_stream_frame(signed.receipt_json.as_bytes(), timeout)?;
        Ok(())
    }

    pub fn request_join(
        &mut self,
        spore_id: &str,
        image_id: &str,
        invitation: &SpawnInvitationClaim,
        timeout: Duration,
    ) -> Result<PicoSpawnObservation, PicoSpawnTransportError> {
        // The private provision is already in the checked Seed. Request only
        // its existing canonical proof; never transfer its signing secret here.
        self.line
            .send_raw_stream_frame(PICO_SPAWN_REQUEST, timeout)?;

        let mut bytes = [0u8; MAX_PICO_ADMISSION_FRAME_BYTES];
        let arrival =
            decode_advertisement(self.line.receive_raw_stream_frame(&mut bytes, timeout)?)?;
        let verifying_key = arrival
            .verifying_key
            .as_slice()
            .try_into()
            .map_err(|_| PicoSpawnTransportError::WrongAdvertisement)?;
        let advertisement = arrival.advertisement;
        let join = decode_join_request(self.line.receive_raw_stream_frame(&mut bytes, timeout)?)?;
        if join.spore_id != spore_id {
            return Err(PicoSpawnTransportError::WrongSpore);
        }
        if join.image_id != image_id {
            return Err(PicoSpawnTransportError::WrongImage);
        }
        if join.invitation_id != invitation.invitation_id || join.nonce != invitation.nonce {
            return Err(PicoSpawnTransportError::WrongInvitation);
        }
        if join.body_id != invitation.body_id {
            return Err(PicoSpawnTransportError::WrongBody);
        }
        if join.host_id != advertisement.host_id
            || join.boot_id != advertisement.boot_id
            || join.offer_generation != advertisement.offer_generation
        {
            return Err(PicoSpawnTransportError::WrongAdvertisement);
        }
        Ok(PicoSpawnObservation {
            advertisement,
            verifying_key,
            proof: SpawnAdmissionProof {
                invitation_id: join.invitation_id,
                body_id: join.body_id,
                host_id: join.host_id,
                boot_id: join.boot_id,
                nonce: join.nonce,
                signature: join
                    .signature
                    .try_into()
                    .map_err(|_| PicoSpawnTransportError::Malformed)?,
            },
            spore_id: join.spore_id,
            image_id: join.image_id,
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OwnedJoinRequest {
    protocol: u16,
    spore_id: String,
    image_id: String,
    invitation_id: SpawnInvitationId,
    body_id: BodyId,
    host_id: HostId,
    boot_id: BootId,
    offer_generation: OfferGeneration,
    nonce: [u8; 32],
    signature: Vec<u8>,
}

fn decode_join_request(encoded: &[u8]) -> Result<OwnedJoinRequest, PicoSpawnTransportError> {
    if encoded.is_empty() || encoded.len() > MAX_PICO_ADMISSION_FRAME_BYTES {
        return Err(PicoSpawnTransportError::Oversized);
    }
    let request: OwnedJoinRequest =
        serde_json::from_slice(encoded).map_err(|_| PicoSpawnTransportError::Malformed)?;
    if request.protocol != PICO_SPAWN_PROTOCOL {
        return Err(PicoSpawnTransportError::WrongProtocol);
    }
    Ok(request)
}

fn checked_receipt_frames(
    signed: &conduit_body::SignedPortableAdmissionReceipt,
    expected_owner: &conduit_body::FederationPeer,
    expected_member_key: &[u8; 32],
    expected_body: &BodyId,
    actual_advertisement: &HostAdvertisement,
) -> Result<Vec<u8>, PicoSpawnTransportError> {
    // Keep the existing Pico limit even though the generic attestation supports
    // larger peers. Refuse before either transport write.
    if signed.receipt_json.is_empty() || signed.receipt_json.len() > MAX_PICO_ADMISSION_FRAME_BYTES
    {
        return Err(PicoSpawnTransportError::Oversized);
    }
    signed
        .verify(
            expected_owner,
            expected_member_key,
            expected_body,
            actual_advertisement,
        )
        .map_err(|_| PicoSpawnTransportError::InvalidReceipt)?;
    let metadata = signed
        .transport_metadata()
        .map_err(|_| PicoSpawnTransportError::InvalidReceipt)?;
    let metadata = serde_json::to_vec(&metadata).map_err(|_| PicoSpawnTransportError::Malformed)?;
    if metadata.len() > MAX_PICO_ADMISSION_FRAME_BYTES {
        return Err(PicoSpawnTransportError::Oversized);
    }
    Ok(metadata)
}

#[cfg(test)]
mod receipt_tests {
    use super::*;
    #[test]
    fn receipt_frames_keep_exact_bytes_and_require_independent_pins() {
        let owner_key = conduit_body::SpawnInvitationSecret::from_csprng_bytes([7; 32]).unwrap();
        let owner = conduit_body::FederationPeer {
            host_id: "host/owner".into(),
            boot_id: "boot/owner".into(),
            offer_generation: OfferGeneration(1),
            verifying_key: owner_key.verifying_key(),
        };
        let mut ad = crate::StdHost::new().advertisement().clone();
        ad.bases.clear();
        ad.resources.clear();
        ad.capabilities.clear();
        ad.planner_capabilities.clear();
        let receipt: conduit_body::PortableAdmissionReceipt = serde_json::from_value(serde_json::json!({
            "schema": conduit_body::SPAWN_ADMISSION_RECEIPT_SCHEMA,
            "credential": {"credential_id":"credential/test", "body_id":"body/test", "part_id":"part/test",
                "host_id":ad.host_id, "boot_id":ad.boot_id, "issued_at_millis":1},
            "host_advertisement": ad, "membership_admitted":true, "current_offers_available":false,
            "plan_created":false, "play_created":false,
        })).unwrap();
        let member_key = [9; 32];
        let signed = conduit_body::attest_portable_admission_receipt(
            &receipt,
            owner.clone(),
            member_key,
            |t| owner_key.sign(t),
        )
        .unwrap();
        let frames = checked_receipt_frames(
            &signed,
            &owner,
            &member_key,
            &receipt.credential.body_id,
            &ad,
        )
        .unwrap();
        let metadata: conduit_body::BorrowedAdmissionReceiptAttestation<'_> =
            serde_json::from_slice(&frames).unwrap();
        metadata
            .verify(signed.receipt_json.as_bytes(), &owner, &member_key)
            .unwrap();
        let mut wrong = owner.clone();
        wrong.boot_id = "boot/foreign".into();
        assert!(matches!(
            checked_receipt_frames(
                &signed,
                &wrong,
                &member_key,
                &receipt.credential.body_id,
                &ad
            ),
            Err(PicoSpawnTransportError::InvalidReceipt)
        ));
        assert!(checked_receipt_frames(
            &signed,
            &owner,
            &[8; 32],
            &receipt.credential.body_id,
            &ad
        )
        .is_err());
        let mut changed = ad.clone();
        changed.boot_id = "boot/changed".into();
        assert!(checked_receipt_frames(
            &signed,
            &owner,
            &member_key,
            &receipt.credential.body_id,
            &changed
        )
        .is_err());
        let mut oversized = signed;
        oversized.receipt_json = " ".repeat(MAX_PICO_ADMISSION_FRAME_BYTES + 1);
        assert!(matches!(
            checked_receipt_frames(
                &oversized,
                &owner,
                &member_key,
                &receipt.credential.body_id,
                &ad
            ),
            Err(PicoSpawnTransportError::Oversized)
        ));
    }
}
