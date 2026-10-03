//! One explicitly authorized native join over an existing protected Line.
//!
//! This is preparation/admission, never Play. The caller supplies the actual
//! advertisement and an independently authorized owner/session binding. Neither
//! discovery nor the invitation secret authenticates the owner. The existing
//! protected handshake must use separately authorized key material. Its binding
//! has Host/Boot identities, but needs no Body membership, Part, Plan, or Cord.
//! This module neither establishes a carrier nor changes owner membership.

use alloc::boxed::Box;
use conduit_body::{
    AdmissionDocumentRefusal, PortableAdmissionReceipt, PortableInvitation,
    PortableSpawnAdmissionRequest, SPAWN_ADMISSION_REQUEST_SCHEMA, SpawnInvitationSecret,
};
use conduit_core::{HostAdvertisement, PROTOCOL_VERSION};
use conduit_protected_line::{
    ProtectedCarrier, ProtectedFrameCarrier, ProtectedLineError, Role, SessionBinding,
    SessionDisposition,
};

/// Admission-document bounds selected before preparing the exchange. These are
/// JSON payload bytes, excluding protected-Line and outer stream framing. The
/// existing protected profile accepts at most 65,519 plaintext bytes; its frame
/// overhead and the outer carrier's capacity must be admitted separately.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AdmissionByteLimits {
    pub maximum_request_bytes: usize,
    pub maximum_receipt_bytes: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeAdmissionRefusal {
    Bounds,
    Document(AdmissionDocumentRefusal),
    HostIdentity,
    StaleHost,
    Expired,
    OwnerSession,
    RequestPressure,
    ReceiptPressure,
    Encoding,
    ReceiptTime,
    Protected(ProtectedLineError),
}

/// A signed request is not membership. This value retains one exact request,
/// its encoded bytes, and public correlation state; no invitation secret.
/// Allocations occur during preparation and bounded receipt decoding, not Play.
/// Caller-owned invitation/advertisement allocations precede this boundary.
pub struct PreparedNativeAdmission<'a> {
    request: PortableSpawnAdmissionRequest,
    encoded: Box<[u8]>,
    owner_session: &'a SessionBinding,
    local_role: Role,
    prepared_at_millis: u64,
    expires_at_millis: u64,
    limits: AdmissionByteLimits,
}

impl<'a> PreparedNativeAdmission<'a> {
    /// Calling this method is the caller's explicit decision to use this
    /// invitation and independently authorized owner/session. Do not populate
    /// `owner_session` from an unauthenticated peer or serial reachability.
    pub fn prepare_authorized(
        invitation: &PortableInvitation,
        advertisement: HostAdvertisement,
        owner_session: &'a SessionBinding,
        local_role: Role,
        limits: AdmissionByteLimits,
        now_millis: u64,
    ) -> Result<Self, NativeAdmissionRefusal> {
        if limits.maximum_request_bytes == 0
            || limits.maximum_receipt_bytes == 0
            || limits.maximum_request_bytes > 65_519
            || limits.maximum_receipt_bytes > 65_519
        {
            return Err(NativeAdmissionRefusal::Bounds);
        }
        invitation
            .validate(now_millis)
            .map_err(NativeAdmissionRefusal::Document)?;
        let local = match local_role {
            Role::Initiator => &owner_session.initiator,
            Role::Responder => &owner_session.responder,
        };
        if advertisement.protocol_version != PROTOCOL_VERSION
            || advertisement.host_id.as_str().is_empty()
            || advertisement.boot_id.as_str().is_empty()
            || advertisement.offer_generation.0 == 0
            || local.host_id != advertisement.host_id.as_str()
            || local.boot_id != advertisement.boot_id.as_str()
        {
            return Err(NativeAdmissionRefusal::HostIdentity);
        }
        let secret =
            SpawnInvitationSecret::from_csprng_bytes(invitation.secret).map_err(|error| {
                NativeAdmissionRefusal::Document(AdmissionDocumentRefusal::Secret(error))
            })?;
        let signature = secret.sign(&invitation.claim.signing_transcript(
            &advertisement.host_id,
            &advertisement.boot_id,
            advertisement.offer_generation,
        ));
        let request = PortableSpawnAdmissionRequest {
            schema: SPAWN_ADMISSION_REQUEST_SCHEMA.into(),
            invitation_id: invitation.claim.invitation_id.clone(),
            body_id: invitation.claim.body_id.clone(),
            host_advertisement: advertisement,
            nonce: invitation.claim.nonce,
            signature: signature.to_vec(),
            membership_admitted: false,
            plan_created: false,
            play_created: false,
        };
        let encoded = serde_json::to_vec(&request).map_err(|_| NativeAdmissionRefusal::Encoding)?;
        if encoded.len() > limits.maximum_request_bytes {
            return Err(NativeAdmissionRefusal::RequestPressure);
        }
        Ok(Self {
            request,
            encoded: encoded.into_boxed_slice(),
            owner_session,
            local_role,
            prepared_at_millis: now_millis,
            expires_at_millis: invitation.claim.expires_at_millis,
            limits,
        })
    }

    pub fn request(&self) -> &PortableSpawnAdmissionRequest {
        &self.request
    }

    /// Exact existing JSON document, with no transport-specific action wrapper.
    pub fn request_bytes(&self) -> &[u8] {
        &self.encoded
    }

    /// Consume preparation into one pending exchange. The protected carrier is
    /// exclusively borrowed until receipt or cancellation, so another session
    /// cannot substitute a receipt. Any refusal retires this prepared request.
    pub fn send<'line, C: ProtectedFrameCarrier>(
        self,
        line: &'line mut ProtectedCarrier<C>,
        current: &HostAdvertisement,
        now_millis: u64,
    ) -> Result<PendingNativeAdmission<'a, 'line, C>, NativeAdmissionRefusal> {
        self.check_current(current, now_millis)?;
        self.check_session(line)?;
        let evidence = line.session().evidence();
        if evidence.sent_frames != 0 || evidence.received_frames != 0 {
            return Err(NativeAdmissionRefusal::OwnerSession);
        }
        line.send(&self.encoded)
            .map_err(NativeAdmissionRefusal::Protected)?;
        Ok(PendingNativeAdmission {
            prepared: self,
            line,
        })
    }

    /// Retire the unused request without admitting membership or sending input.
    pub fn cancel(self) {}

    fn check_current(
        &self,
        current: &HostAdvertisement,
        now: u64,
    ) -> Result<(), NativeAdmissionRefusal> {
        if current != &self.request.host_advertisement {
            return Err(NativeAdmissionRefusal::StaleHost);
        }
        if now < self.prepared_at_millis || now >= self.expires_at_millis {
            return Err(NativeAdmissionRefusal::Expired);
        }
        Ok(())
    }

    fn check_session<C: ProtectedFrameCarrier>(
        &self,
        line: &ProtectedCarrier<C>,
    ) -> Result<(), NativeAdmissionRefusal> {
        let evidence = line.session().evidence();
        if evidence.binding != self.owner_session
            || evidence.role != self.local_role
            || evidence.disposition != SessionDisposition::Open
        {
            return Err(NativeAdmissionRefusal::OwnerSession);
        }
        if (evidence.limits.maximum_payload_bytes as usize) < self.encoded.len()
            || (evidence.limits.maximum_payload_bytes as usize) < self.limits.maximum_receipt_bytes
        {
            return Err(NativeAdmissionRefusal::Bounds);
        }
        Ok(())
    }
}

/// One request, one authenticated receipt attempt. Consuming completion retires
/// pending state on success, stale identity, expiry, malformed input, or loss;
/// there is no retry, reconnect, or replay transition.
pub struct PendingNativeAdmission<'a, 'line, C> {
    prepared: PreparedNativeAdmission<'a>,
    line: &'line mut ProtectedCarrier<C>,
}

impl<C: ProtectedFrameCarrier> PendingNativeAdmission<'_, '_, C> {
    pub fn complete(
        self,
        current: &HostAdvertisement,
        now_millis: u64,
    ) -> Result<PortableAdmissionReceipt, NativeAdmissionRefusal> {
        self.prepared.check_current(current, now_millis)?;
        self.prepared.check_session(self.line)?;
        let bytes = self
            .line
            .receive()
            .map_err(NativeAdmissionRefusal::Protected)?;
        if bytes.len() > self.prepared.limits.maximum_receipt_bytes {
            return Err(NativeAdmissionRefusal::ReceiptPressure);
        }
        let receipt: PortableAdmissionReceipt =
            serde_json::from_slice(bytes).map_err(|_| NativeAdmissionRefusal::Encoding)?;
        receipt
            .validate_against(&self.prepared.request)
            .map_err(NativeAdmissionRefusal::Document)?;
        if receipt.credential.issued_at_millis < self.prepared.prepared_at_millis
            || receipt.credential.issued_at_millis > now_millis
        {
            return Err(NativeAdmissionRefusal::ReceiptTime);
        }
        Ok(receipt)
    }

    /// Cancel this pending exchange and close its protected session. No receipt
    /// or membership is manufactured; transport close failures remain explicit.
    pub fn cancel(self) -> Result<(), NativeAdmissionRefusal> {
        self.line.close().map_err(NativeAdmissionRefusal::Protected)
    }
}

#[cfg(test)]
#[path = "admission_tests.rs"]
mod tests;
