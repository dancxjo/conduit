//! Existing portable Body admission documents, shared with no_std participants.
//!
//! These are the installed CLI's existing serde shapes, not a second admission
//! protocol. Callers bound input bytes before decoding and authenticate their
//! transport separately. Validation checks document consistency only: only
//! `AdmissionManager::complete_spawn` verifies and consumes invitation authority,
//! and a received receipt is meaningful only from the authenticated Body owner.

use alloc::{boxed::Box, string::String, vec::Vec};
use conduit_core::HostAdvertisement;
use serde::{Deserialize, Serialize};

use super::{
    AdmissionRefusal, MembershipCredential, SpawnAdmissionProof, SpawnInvitationClaim,
    SpawnInvitationSecret, ADMISSION_SIGNATURE_BYTES,
};
use crate::{BodyId, RendezvousDescriptorRefusal, SpawnInvitationId, SpawnRendezvousDescriptor};

pub const INVITATION_SCHEMA: &str = "conduit.body/spawn-invitation@1";
pub const ROUTED_INVITATION_SCHEMA: &str = "conduit.body/spawn-invitation@2";
pub const SPAWN_ADMISSION_REQUEST_SCHEMA: &str = "conduit.body/spawn-admission-request@1";
pub const SPAWN_ADMISSION_RECEIPT_SCHEMA: &str = "conduit.body/spawn-admission-receipt@1";
pub const ROUTED_ADMISSION_REQUEST_SCHEMA: &str = "conduit.body/routed-admission-request@1";
pub const ROUTED_ADMISSION_RESPONSE_SCHEMA: &str = "conduit.body/routed-admission-response@1";

/// Admission capability for exact target provisioning. Never log this document
/// or derive `Debug`: its serialized secret intentionally authorizes one proof.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PortableInvitation {
    pub schema: String,
    pub claim: SpawnInvitationClaim,
    pub secret: [u8; 32],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rendezvous: Option<SpawnRendezvousDescriptor>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PortableSpawnAdmissionRequest {
    pub schema: String,
    pub invitation_id: SpawnInvitationId,
    pub body_id: BodyId,
    pub host_advertisement: HostAdvertisement,
    pub nonce: [u8; 32],
    pub signature: Vec<u8>,
    pub membership_admitted: bool,
    pub plan_created: bool,
    pub play_created: bool,
}

// Preserve the existing receipt's serde compatibility, including acceptance of
// unknown extension fields. Exact accepted identities are checked separately.
#[derive(Clone, Serialize, Deserialize)]
pub struct PortableAdmissionReceipt {
    pub schema: String,
    pub credential: MembershipCredential,
    pub host_advertisement: HostAdvertisement,
    pub membership_admitted: bool,
    pub current_offers_available: bool,
    pub plan_created: bool,
    pub play_created: bool,
}

/// One existing signed request carried over an authenticated owner route.
/// The route authenticates the owner separately; this envelope grants nothing.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoutedAdmissionRequest {
    pub schema: String,
    pub invitation_id: String,
    pub request: PortableSpawnAdmissionRequest,
}

/// Exact owner outcome over that same route. A refusal is never membership.
#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "kebab-case", deny_unknown_fields)]
pub enum RoutedAdmissionResponse {
    Admitted {
        schema: String,
        receipt: Box<PortableAdmissionReceipt>,
    },
    Refused {
        schema: String,
        code: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdmissionDocumentRefusal {
    InvitationSchema,
    Invitation(AdmissionRefusal),
    Route(RendezvousDescriptorRefusal),
    RouteIdentity,
    RouteSchema,
    Secret(AdmissionRefusal),
    RequestClaims,
    SignatureBound,
    ReceiptBasis,
}

impl PortableInvitation {
    /// Inspect an invitation before electing to sign it. Legacy v1 remains
    /// available for diagnostic callers; public routed join requires v2.
    pub fn validate(&self, now_millis: u64) -> Result<(), AdmissionDocumentRefusal> {
        let routed = self.schema == ROUTED_INVITATION_SCHEMA;
        if self.schema != INVITATION_SCHEMA && !routed {
            return Err(AdmissionDocumentRefusal::InvitationSchema);
        }
        self.claim
            .inspect(now_millis)
            .map_err(AdmissionDocumentRefusal::Invitation)?;
        match (&self.rendezvous, routed) {
            (None, false) => {}
            (Some(rendezvous), true) => {
                rendezvous
                    .validate(now_millis)
                    .map_err(AdmissionDocumentRefusal::Route)?;
                if rendezvous.body_id != self.claim.body_id.as_str()
                    || rendezvous.invitation_id != self.claim.invitation_id.as_str()
                {
                    return Err(AdmissionDocumentRefusal::RouteIdentity);
                }
            }
            _ => return Err(AdmissionDocumentRefusal::RouteSchema),
        }
        SpawnInvitationSecret::from_csprng_bytes(self.secret)
            .map_err(AdmissionDocumentRefusal::Secret)?;
        Ok(())
    }
}

impl PortableSpawnAdmissionRequest {
    /// A request cannot assert successful membership or execution. Signature
    /// authenticity, expiry, current Host/Boot and replay remain manager checks.
    pub fn validate(&self) -> Result<(), AdmissionDocumentRefusal> {
        if self.schema != SPAWN_ADMISSION_REQUEST_SCHEMA
            || self.membership_admitted
            || self.plan_created
            || self.play_created
        {
            return Err(AdmissionDocumentRefusal::RequestClaims);
        }
        Ok(())
    }

    /// Decode the existing exact proof carrier without granting admission.
    pub fn admission_proof(&self) -> Result<SpawnAdmissionProof, AdmissionDocumentRefusal> {
        self.validate()?;
        let signature: [u8; ADMISSION_SIGNATURE_BYTES] = self
            .signature
            .as_slice()
            .try_into()
            .map_err(|_| AdmissionDocumentRefusal::SignatureBound)?;
        Ok(SpawnAdmissionProof {
            invitation_id: self.invitation_id.clone(),
            body_id: self.body_id.clone(),
            host_id: self.host_advertisement.host_id.clone(),
            boot_id: self.host_advertisement.boot_id.clone(),
            nonce: self.nonce,
            signature,
        })
    }
}

impl PortableAdmissionReceipt {
    /// Correlate an owner's receipt with the caller's retained pending request.
    /// This authenticates neither the owner nor the receipt; it creates no Plan
    /// or Play. Installation-specific current Host/Boot checks remain caller-owned.
    pub fn validate_against(
        &self,
        request: &PortableSpawnAdmissionRequest,
    ) -> Result<(), AdmissionDocumentRefusal> {
        if self.schema != SPAWN_ADMISSION_RECEIPT_SCHEMA
            || !self.membership_admitted
            || self.plan_created
            || self.play_created
            || self.current_offers_available != !request.host_advertisement.capabilities.is_empty()
            || self.host_advertisement != request.host_advertisement
            || self.credential.body_id != request.body_id
            || self.credential.host_id != request.host_advertisement.host_id
            || self.credential.boot_id != request.host_advertisement.boot_id
        {
            return Err(AdmissionDocumentRefusal::ReceiptBasis);
        }
        Ok(())
    }
}

impl core::fmt::Display for AdmissionDocumentRefusal {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvitationSchema => {
                formatter.write_str("Body invitation has an unsupported schema")
            }
            Self::Invitation(error) => write!(formatter, "Body invitation refused: {error:?}"),
            Self::Route(error) => write!(formatter, "Body invitation route refused: {error:?}"),
            Self::RouteIdentity => {
                formatter.write_str("Body invitation route lost its exact invitation identity")
            }
            Self::RouteSchema => formatter.write_str("Body invitation schema and route disagree"),
            Self::Secret(error) => write!(formatter, "Body invitation secret refused: {error:?}"),
            Self::RequestClaims => formatter
                .write_str("Body admission request has an unsupported schema or claims effects"),
            Self::SignatureBound => {
                formatter.write_str("Body admission request signature has the wrong bound")
            }
            Self::ReceiptBasis => formatter.write_str(
                "Body admission receipt lost its exact pending join identity or claims effects",
            ),
        }
    }
}
