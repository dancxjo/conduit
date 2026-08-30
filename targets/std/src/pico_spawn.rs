//! Exact local USB provisioning and observation for a self-joining Pico spore.

use std::time::Duration;

use conduit_body::{
    BodyId, PicoSpawnProvision, SpawnAdmissionProof, SpawnInvitation, SpawnInvitationId,
    MAX_PICO_ADMISSION_FRAME_BYTES, PICO_SPAWN_PROTOCOL,
};
use conduit_core::{BootId, HostAdvertisement, HostId, OfferGeneration};
use serde::Deserialize;

use crate::pico_admission::{decode_advertisement, PicoAdmissionTransportError};
use crate::usb_cdc::NativePathCdcLine;

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
    pub proof: SpawnAdmissionProof,
    pub spore_id: String,
    pub image_id: String,
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

    pub fn request_join(
        mut self,
        spore_id: &str,
        image_id: &str,
        invitation: &SpawnInvitation,
        timeout: Duration,
    ) -> Result<PicoSpawnObservation, PicoSpawnTransportError> {
        let mut secret = invitation.secret.copy_for_target_provisioning();
        let mut provision = PicoSpawnProvision {
            protocol: PICO_SPAWN_PROTOCOL,
            spore_id,
            image_id,
            invitation_id: invitation.invitation_id.as_str(),
            body_id: invitation.body_id.as_str(),
            nonce: invitation.nonce,
            expires_at_millis: invitation.expires_at_millis,
            secret,
        };
        let mut encoded = match serde_json::to_vec(&provision) {
            Ok(encoded) => encoded,
            Err(_) => {
                provision.secret.fill(0);
                secret.fill(0);
                return Err(PicoSpawnTransportError::Malformed);
            }
        };
        provision.secret.fill(0);
        secret.fill(0);
        if encoded.len() > MAX_PICO_ADMISSION_FRAME_BYTES {
            encoded.fill(0);
            return Err(PicoSpawnTransportError::Oversized);
        }
        let sent = self.line.send_raw_stream_frame(&encoded, timeout);
        encoded.fill(0);
        sent?;

        let mut bytes = [0u8; MAX_PICO_ADMISSION_FRAME_BYTES];
        let advertisement =
            decode_advertisement(self.line.receive_raw_stream_frame(&mut bytes, timeout)?)?
                .advertisement;
        let join = decode_join_request(self.line.receive_raw_stream_frame(&mut bytes, timeout)?)?;
        if join.spore_id != spore_id {
            return Err(PicoSpawnTransportError::WrongSpore);
        }
        if join.image_id != image_id {
            return Err(PicoSpawnTransportError::WrongImage);
        }
        if join.invitation_id != invitation.invitation_id {
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
