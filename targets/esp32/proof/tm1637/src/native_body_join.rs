//! Root-owned private invitation and observation-only admission. Transport is
//! separate: this module never invents an authenticated rendezvous or a grant.
#[path = "native_body_join_owner.rs"]
pub(crate) mod owner;
use crate::native_restart::payload::wire as biography_wire;
#[path = "native_biography_custody_acceptance.rs"]
pub(crate) mod biography_acceptance;
#[path = "native_joined_host_recovery.rs"]
pub(crate) mod recovery;
#[path = "native_body_return_wire.rs"]
pub(crate) mod return_wire;
#[path = "native_body_return.rs"]
pub(crate) mod returning;
#[path = "native_body_join_wire.rs"]
pub(crate) mod wire;
pub(crate) enum Command {
    ReturnChallenge(return_wire::Begin),
    Return(return_wire::Complete),
    Admission(conduit_body::PortableSpawnAdmissionRequest),
    Recovery(recovery::Request),
}
pub(crate) fn decode_command(bytes: &[u8]) -> Result<Command, &'static str> {
    if bytes.len() > 1024 {
        return Err("body-join-request-capacity");
    }
    if let Ok(request) = return_wire::begin(bytes) {
        return Ok(Command::ReturnChallenge(request));
    }
    if let Ok(request) = return_wire::complete(bytes) {
        return Ok(Command::Return(request));
    }
    if let Ok(request) = serde_json::from_slice::<recovery::Request>(bytes) {
        if request.schema != recovery::REQUEST
            || request.nonce == [0; 32]
            || request.signature.len() != 64
        {
            return Err("body-recovery-request");
        }
        return Ok(Command::Recovery(request));
    }
    wire::decode(bytes).map(Command::Admission)
}
use crate::native_joined_host_wire::{JoinedHostReceipt, PROFILE};
use alloc::{string::String, vec::Vec};
use conduit_body::{
    PortableInvitation, PortableSpawnAdmissionRequest, SealedBodyLifecycleSession,
    SpawnInvitationSecret, INVITATION_SCHEMA,
};
use conduit_core::{BootId, HostId};

/// Private target bootstrap. Possession of `invitation.secret` authorizes one
/// canonical admission; never log it or serve it as an unauthenticated discovery
/// record. The Root key is pinned through the same private bootstrap channel.
#[derive(serde::Serialize)]
pub(crate) struct PrivateBodyInvitation {
    pub invitation: PortableInvitation,
    pub root_key: [u8; 32],
    pub producer_host: HostId,
    pub producer_boot: BootId,
}

/// Created only after canonical admission and the complete durable checkpoint.
/// No descriptor, claimed ID, raw verification key or deserializer creates it.
pub(crate) struct DurablyAdmittedHost {
    receipt: JoinedHostReceipt,
}
impl DurablyAdmittedHost {
    pub(crate) fn transcript(&self) -> [u8; 32] {
        crate::native_joined_host_wire::transcript(&self.receipt.scope())
    }
    pub(crate) fn signed(mut self, signature: [u8; 64]) -> JoinedHostReceipt {
        self.receipt.signature = signature.to_vec();
        self.receipt
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn issue(
    owner: &mut SealedBodyLifecycleSession,
    root_key: &[u8; 32],
    authority_host: &HostId,
    authority_boot: &BootId,
    secret: SpawnInvitationSecret,
    nonce: [u8; 32],
    now_millis: u64,
    expires_at_millis: u64,
) -> Result<PrivateBodyInvitation, &'static str> {
    let claim =
        crate::native_restart::admissions_transaction(owner, root_key, |owner, manager| {
            owner
                .issue_invitation(
                    manager,
                    secret.clone(),
                    nonce,
                    now_millis,
                    expires_at_millis,
                    authority_host,
                    authority_boot,
                )
                .map_err(|_| "body-join-invitation")
        })?;
    Ok(PrivateBodyInvitation {
        invitation: PortableInvitation {
            schema: String::from(INVITATION_SCHEMA),
            claim,
            secret: secret.copy_for_target_provisioning(),
            rendezvous: None,
        },
        root_key: *root_key,
        producer_host: authority_host.clone(),
        producer_boot: authority_boot.clone(),
    })
}

/// Admit only a new observer; no execution offer is installed. The current
/// Root's Wake/Plan/Play remains unchanged. BodyTime would pin the membership
/// revision, so that profile refuses an in-place membership transition.
pub(crate) fn admit(
    owner: &mut SealedBodyLifecycleSession,
    root_key: &[u8; 32],
    authority_host: &HostId,
    authority_boot: &BootId,
    request: &PortableSpawnAdmissionRequest,
    now_millis: u64,
) -> Result<DurablyAdmittedHost, &'static str> {
    let proof = request.admission_proof().map_err(|_| "body-join-request")?;
    let advertisement = &request.host_advertisement;
    if &advertisement.host_id == authority_host
        || !advertisement.bases.is_empty()
        || !advertisement.resources.is_empty()
        || !advertisement.capabilities.is_empty()
        || !advertisement.planner_capabilities.is_empty()
        || owner
            .realization()
            .is_some_and(|current| current.plan.body_time_requirement().is_some())
    {
        return Err("body-join-observer-profile");
    }
    let credential =
        crate::native_restart::admissions_transaction(owner, root_key, |owner, manager| {
            owner
                .admit_invited_host(
                    manager,
                    advertisement,
                    &proof,
                    now_millis,
                    authority_host,
                    authority_boot,
                )
                .map_err(|_| "body-join-admission")
        })?;
    Ok(DurablyAdmittedHost {
        receipt: JoinedHostReceipt {
            schema: String::from(PROFILE),
            credential,
            membership_revision: owner.evidence().membership.revision.0,
            producer_host: authority_host.clone(),
            producer_boot: authority_boot.clone(),
            producer_key: *root_key,
            invitation_id: proof.invitation_id,
            offer_generation: advertisement.offer_generation,
            signature: Vec::new(),
        },
    })
}
