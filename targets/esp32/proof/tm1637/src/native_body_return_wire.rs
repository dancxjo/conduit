//! Bounded empty-offer return carrier, never a rich advertisement decoder.
use super::wire::{Advertisement, Bytes};
use alloc::string::String;
use conduit_body::{AdmissionId, BodyId, PartId, PartReturnProof};
use conduit_core::HostAdvertisement;
use serde::Deserialize;
pub(crate) const CHALLENGE_REQUEST: &str =
    "conduit.environment/observer-return-challenge-request@1";
pub(crate) const REQUEST: &str = "conduit.environment/observer-return-request@1";
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ChallengeRequest {
    pub schema: String,
    pub body_id: BodyId,
    pub part_id: PartId,
    pub host_advertisement: Advertisement,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Request {
    pub schema: String,
    pub body_id: BodyId,
    pub part_id: PartId,
    pub admission_id: AdmissionId,
    pub host_advertisement: Advertisement,
    pub nonce: Bytes<32>,
    pub signature: Bytes<64>,
}
pub(crate) struct Begin {
    pub body_id: BodyId,
    pub part_id: PartId,
    pub advertisement: HostAdvertisement,
}
pub(crate) struct Complete {
    pub advertisement: HostAdvertisement,
    pub proof: PartReturnProof,
}
pub(crate) fn begin(bytes: &[u8]) -> Result<Begin, &'static str> {
    let r: ChallengeRequest =
        serde_json::from_slice(bytes).map_err(|_| "body-return-challenge-encoding")?;
    if r.schema != CHALLENGE_REQUEST {
        return Err("body-return-challenge-schema");
    }
    Ok(Begin {
        body_id: r.body_id,
        part_id: r.part_id,
        advertisement: r.host_advertisement.into_host()?,
    })
}
pub(crate) fn complete(bytes: &[u8]) -> Result<Complete, &'static str> {
    let r: Request = serde_json::from_slice(bytes).map_err(|_| "body-return-encoding")?;
    if r.schema != REQUEST || r.nonce.0 == [0; 32] {
        return Err("body-return-schema");
    }
    let advertisement = r.host_advertisement.into_host()?;
    Ok(Complete {
        proof: PartReturnProof {
            admission_id: r.admission_id,
            body_id: r.body_id,
            part_id: r.part_id,
            host_id: advertisement.host_id.clone(),
            boot_id: advertisement.boot_id.clone(),
            nonce: r.nonce.0,
            signature: r.signature.0,
        },
        advertisement,
    })
}
