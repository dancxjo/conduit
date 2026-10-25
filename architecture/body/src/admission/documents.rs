//! Existing portable Body admission documents, shared with no_std participants.
//!
//! Standalone request/receipt JSON retains the installed CLI's existing shape.
//! Routed version two losslessly carries complete advertisement JSON inside an
//! explicitly bounded profile; legacy routed version one remains supported.
//! Callers bound input bytes before decoding and authenticate transport separately. Validation checks document consistency only: only
//! `AdmissionManager::complete_spawn` verifies and consumes invitation authority,
//! and a received receipt is meaningful only from the authenticated Body owner.

use alloc::{boxed::Box, string::String, vec::Vec};
use conduit_core::HostAdvertisement;
use serde::{de::Error as _, ser::Error as _, Deserialize, Deserializer, Serialize, Serializer};

use super::{
    AdmissionRefusal, MembershipCredential, SpawnAdmissionProof, SpawnInvitationClaim,
    SpawnInvitationSecret, ADMISSION_SIGNATURE_BYTES,
};
use crate::{BodyId, RendezvousDescriptorRefusal, SpawnInvitationId, SpawnRendezvousDescriptor};

pub const INVITATION_SCHEMA: &str = "conduit.body/spawn-invitation@1";
pub const ROUTED_INVITATION_SCHEMA: &str = "conduit.body/spawn-invitation@2";
pub const SPAWN_ADMISSION_REQUEST_SCHEMA: &str = "conduit.body/spawn-admission-request@1";
pub const SPAWN_ADMISSION_RECEIPT_SCHEMA: &str = "conduit.body/spawn-admission-receipt@1";
pub const LEGACY_ROUTED_ADMISSION_REQUEST_SCHEMA: &str = "conduit.body/routed-admission-request@1";
pub const ROUTED_ADMISSION_REQUEST_SCHEMA: &str = LEGACY_ROUTED_ADMISSION_REQUEST_SCHEMA;
pub const COMPACT_ROUTED_ADMISSION_REQUEST_SCHEMA: &str = "conduit.body/routed-admission-request@2";
pub const LEGACY_ROUTED_ADMISSION_RESPONSE_SCHEMA: &str =
    "conduit.body/routed-admission-response@1";
pub const ROUTED_ADMISSION_RESPONSE_SCHEMA: &str = LEGACY_ROUTED_ADMISSION_RESPONSE_SCHEMA;
pub const COMPACT_ROUTED_ADMISSION_RESPONSE_SCHEMA: &str =
    "conduit.body/routed-admission-response@2";

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
#[derive(Clone)]
pub struct RoutedAdmissionRequest {
    pub schema: String,
    pub invitation_id: String,
    pub request: PortableSpawnAdmissionRequest,
}

// The outer version selects the advertisement profile. The nested signed
// request and signature transcript stay unchanged; receipts remain raw v1.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RequestWire<A> {
    schema: String,
    invitation_id: SpawnInvitationId,
    body_id: BodyId,
    host_advertisement: A,
    nonce: [u8; 32],
    signature: Vec<u8>,
    membership_admitted: bool,
    plan_created: bool,
    play_created: bool,
}

impl RequestWire<crate::admission_advertisement_wire::CompactAdvertisement> {
    fn from_request(request: &PortableSpawnAdmissionRequest) -> Self {
        Self {
            schema: request.schema.clone(),
            invitation_id: request.invitation_id.clone(),
            body_id: request.body_id.clone(),
            host_advertisement: crate::admission_advertisement_wire::CompactAdvertisement(
                request.host_advertisement.clone(),
            ),
            nonce: request.nonce,
            signature: request.signature.clone(),
            membership_admitted: request.membership_admitted,
            plan_created: request.plan_created,
            play_created: request.play_created,
        }
    }
    fn into_request(self) -> PortableSpawnAdmissionRequest {
        PortableSpawnAdmissionRequest {
            schema: self.schema,
            invitation_id: self.invitation_id,
            body_id: self.body_id,
            host_advertisement: self.host_advertisement.0,
            nonce: self.nonce,
            signature: self.signature,
            membership_admitted: self.membership_admitted,
            plan_created: self.plan_created,
            play_created: self.play_created,
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RoutedWire<R> {
    schema: String,
    invitation_id: String,
    request: R,
}

impl Serialize for RoutedAdmissionRequest {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if self.schema == LEGACY_ROUTED_ADMISSION_REQUEST_SCHEMA {
            RoutedWire {
                schema: self.schema.clone(),
                invitation_id: self.invitation_id.clone(),
                request: &self.request,
            }
            .serialize(serializer)
        } else if self.schema == COMPACT_ROUTED_ADMISSION_REQUEST_SCHEMA {
            RoutedWire {
                schema: self.schema.clone(),
                invitation_id: self.invitation_id.clone(),
                request: RequestWire::from_request(&self.request),
            }
            .serialize(serializer)
        } else {
            Err(S::Error::custom(
                "unsupported routed admission request schema",
            ))
        }
    }
}

impl<'de> Deserialize<'de> for RoutedAdmissionRequest {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Wire {
            Compact(
                RoutedWire<RequestWire<crate::admission_advertisement_wire::CompactAdvertisement>>,
            ),
            Legacy(RoutedWire<PortableSpawnAdmissionRequest>),
        }
        match Wire::deserialize(deserializer)? {
            Wire::Compact(value) if value.schema == COMPACT_ROUTED_ADMISSION_REQUEST_SCHEMA => {
                Ok(Self {
                    schema: value.schema,
                    invitation_id: value.invitation_id,
                    request: value.request.into_request(),
                })
            }
            Wire::Legacy(value) if value.schema == LEGACY_ROUTED_ADMISSION_REQUEST_SCHEMA => {
                Ok(Self {
                    schema: value.schema,
                    invitation_id: value.invitation_id,
                    request: value.request,
                })
            }
            _ => Err(D::Error::custom(
                "routed admission schema and advertisement profile disagree",
            )),
        }
    }
}

impl RoutedAdmissionRequest {
    pub fn supported_schema(&self) -> bool {
        self.schema == COMPACT_ROUTED_ADMISSION_REQUEST_SCHEMA
            || self.schema == LEGACY_ROUTED_ADMISSION_REQUEST_SCHEMA
    }
}

/// Exact owner outcome over that same route. A refusal is never membership.
#[derive(Clone)]
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

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CompactReceiptWire {
    schema: String,
    credential: MembershipCredential,
    host_advertisement: crate::admission_advertisement_wire::CompactAdvertisement,
    membership_admitted: bool,
    current_offers_available: bool,
    plan_created: bool,
    play_created: bool,
}
impl CompactReceiptWire {
    fn from_receipt(receipt: &PortableAdmissionReceipt) -> Self {
        Self {
            schema: receipt.schema.clone(),
            credential: receipt.credential.clone(),
            host_advertisement: crate::admission_advertisement_wire::CompactAdvertisement(
                receipt.host_advertisement.clone(),
            ),
            membership_admitted: receipt.membership_admitted,
            current_offers_available: receipt.current_offers_available,
            plan_created: receipt.plan_created,
            play_created: receipt.play_created,
        }
    }
    fn into_receipt(self) -> PortableAdmissionReceipt {
        PortableAdmissionReceipt {
            schema: self.schema,
            credential: self.credential,
            host_advertisement: self.host_advertisement.0,
            membership_admitted: self.membership_admitted,
            current_offers_available: self.current_offers_available,
            plan_created: self.plan_created,
            play_created: self.play_created,
        }
    }
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "kebab-case", deny_unknown_fields)]
enum ResponseWire<R> {
    Admitted { schema: String, receipt: Box<R> },
    Refused { schema: String, code: String },
}

pub fn supported_routed_admission_response_schema(schema: &str) -> bool {
    schema == COMPACT_ROUTED_ADMISSION_RESPONSE_SCHEMA
        || schema == LEGACY_ROUTED_ADMISSION_RESPONSE_SCHEMA
}

impl RoutedAdmissionRequest {
    /// Reply with the same advertisement profile the requester understands.
    pub fn response_schema(&self) -> &'static str {
        if self.schema == LEGACY_ROUTED_ADMISSION_REQUEST_SCHEMA {
            LEGACY_ROUTED_ADMISSION_RESPONSE_SCHEMA
        } else {
            COMPACT_ROUTED_ADMISSION_RESPONSE_SCHEMA
        }
    }
}

impl Serialize for RoutedAdmissionResponse {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Admitted { schema, receipt }
                if schema == LEGACY_ROUTED_ADMISSION_RESPONSE_SCHEMA =>
            {
                ResponseWire::Admitted {
                    schema: schema.clone(),
                    receipt: Box::new(receipt.as_ref()),
                }
                .serialize(serializer)
            }
            Self::Admitted { schema, receipt }
                if schema == COMPACT_ROUTED_ADMISSION_RESPONSE_SCHEMA =>
            {
                ResponseWire::Admitted {
                    schema: schema.clone(),
                    receipt: Box::new(CompactReceiptWire::from_receipt(receipt)),
                }
                .serialize(serializer)
            }
            Self::Refused { schema, code }
                if supported_routed_admission_response_schema(schema) =>
            {
                ResponseWire::<PortableAdmissionReceipt>::Refused {
                    schema: schema.clone(),
                    code: code.clone(),
                }
                .serialize(serializer)
            }
            _ => Err(S::Error::custom(
                "unsupported routed admission response schema",
            )),
        }
    }
}
impl<'de> Deserialize<'de> for RoutedAdmissionResponse {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Wire {
            Compact(ResponseWire<CompactReceiptWire>),
            Legacy(ResponseWire<PortableAdmissionReceipt>),
        }
        match Wire::deserialize(deserializer)? {
            Wire::Compact(ResponseWire::Admitted { schema, receipt })
                if schema == COMPACT_ROUTED_ADMISSION_RESPONSE_SCHEMA =>
            {
                Ok(Self::Admitted {
                    schema,
                    receipt: Box::new(receipt.into_receipt()),
                })
            }
            Wire::Legacy(ResponseWire::Admitted { schema, receipt })
                if schema == LEGACY_ROUTED_ADMISSION_RESPONSE_SCHEMA =>
            {
                Ok(Self::Admitted { schema, receipt })
            }
            Wire::Compact(ResponseWire::Refused { schema, code })
            | Wire::Legacy(ResponseWire::Refused { schema, code })
                if supported_routed_admission_response_schema(&schema) =>
            {
                Ok(Self::Refused { schema, code })
            }
            _ => Err(D::Error::custom(
                "routed response schema and advertisement profile disagree",
            )),
        }
    }
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
