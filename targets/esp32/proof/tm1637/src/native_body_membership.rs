//! Membership-only invitation completion; no equipment description or offer
//! becomes available through this path. Durable Root custody precedes signing.
use alloc::boxed::Box;
use conduit_body::{
    PortableMembershipAdmissionReceipt, PortableMembershipAdmissionRequest,
    SealedBodyLifecycleSession, MEMBERSHIP_ADMISSION_RECEIPT_SCHEMA,
};
use conduit_core::{BootId, HostId};

/// Only a completed authoritative checkpoint can construct this signing input.
pub(crate) struct DurablyAdmittedMembership {
    receipt: PortableMembershipAdmissionReceipt,
}
impl DurablyAdmittedMembership {
    pub(crate) fn transcript(&self) -> [u8; 32] {
        self.receipt.transcript()
    }
    pub(crate) fn root(&self) -> &[u8; 32] {
        &self.receipt.producer_key
    }
    pub(crate) fn signed(mut self, signature: [u8; 64]) -> PortableMembershipAdmissionReceipt {
        self.receipt.signature = signature;
        self.receipt
    }
}

pub(crate) fn admit(
    owner: &mut SealedBodyLifecycleSession,
    root_key: &[u8; 32],
    authority_host: &HostId,
    authority_boot: &BootId,
    request: &PortableMembershipAdmissionRequest,
    now_millis: u64,
) -> Result<DurablyAdmittedMembership, &'static str> {
    let proof = request
        .admission_proof()
        .map_err(|_| "body-membership-request")?;
    validate_profile(owner, authority_host, request)?;
    let credential =
        crate::native_restart::admissions_transaction(owner, root_key, |owner, manager| {
            owner
                .admit_invited_host_scope(
                    manager,
                    request.host_scope(),
                    &proof,
                    now_millis,
                    authority_host,
                    authority_boot,
                )
                .map_err(|_| "body-membership-admission")
        })?;
    Ok(DurablyAdmittedMembership {
        receipt: pending(
            owner,
            root_key,
            authority_host,
            authority_boot,
            request,
            credential,
        )
        .receipt,
    })
}

fn pending(
    owner: &SealedBodyLifecycleSession,
    root_key: &[u8; 32],
    authority_host: &HostId,
    authority_boot: &BootId,
    request: &PortableMembershipAdmissionRequest,
    credential: conduit_body::MembershipCredential,
) -> PendingMembership {
    PendingMembership {
        receipt: PortableMembershipAdmissionReceipt {
            schema: MEMBERSHIP_ADMISSION_RECEIPT_SCHEMA.into(),
            protocol_version: conduit_core::PROTOCOL_VERSION,
            credential,
            membership_revision: owner.evidence().membership.revision.0,
            producer_host: authority_host.clone(),
            producer_boot: authority_boot.clone(),
            producer_key: *root_key,
            invitation_id: request.invitation_id.clone(),
            offer_generation: request.offer_generation,
            signature: [0; 64],
            request_digest: request.digest(),
            membership_admitted: true,
            current_offers_available: false,
            plan_created: false,
            play_created: false,
        },
    }
}

/// Unsigned pending custody is opaque and has no signing/transcript entrance.
pub(crate) struct PendingMembership {
    receipt: PortableMembershipAdmissionReceipt,
}
pub(crate) type PendingCheckpoint = crate::native_restart::owned_checkpoint::Job<PendingMembership>;
pub(crate) enum OwnedRefusal {
    Unstaged(Box<SealedBodyLifecycleSession>, &'static str),
    Staged(Box<PendingCheckpoint>, &'static str),
}
fn validate_profile(
    owner: &SealedBodyLifecycleSession,
    authority_host: &HostId,
    request: &PortableMembershipAdmissionRequest,
) -> Result<(), &'static str> {
    // Membership changes invalidate a BodyTime-pinned membership revision.
    // Preserve the existing active-Body safeguard, independently of equipment.
    if &request.host_id == authority_host
        || owner
            .realization()
            .is_some_and(|current| current.plan.body_time_requirement().is_some())
    {
        return Err("body-membership-profile");
    }
    Ok(())
}
pub(crate) fn begin_owned(
    owner: Box<SealedBodyLifecycleSession>,
    root_key: &[u8; 32],
    authority_host: &HostId,
    authority_boot: &BootId,
    request: &PortableMembershipAdmissionRequest,
    now_millis: u64,
) -> Result<Box<PendingCheckpoint>, OwnedRefusal> {
    let proof = match request.admission_proof() {
        Ok(p) => p,
        Err(_) => return Err(OwnedRefusal::Unstaged(owner, "body-membership-request")),
    };
    if let Err(reason) = validate_profile(&owner, authority_host, request) {
        return Err(OwnedRefusal::Unstaged(owner, reason));
    }
    let mut job = match PendingCheckpoint::reserve(owner, *root_key) {
        Ok(j) => j,
        Err((owner, e)) => return Err(OwnedRefusal::Unstaged(owner, e)),
    };
    if let Err(reason) = job.stage_invited_host_scope(
        request.host_scope(),
        &proof,
        now_millis,
        authority_host,
        authority_boot,
        |owner, credential| {
            pending(
                owner,
                root_key,
                authority_host,
                authority_boot,
                request,
                credential,
            )
        },
    ) {
        return match job.release_unstaged() {
            Ok(owner) => Err(OwnedRefusal::Unstaged(owner, reason)),
            Err(job) => Err(OwnedRefusal::Staged(job, reason)),
        };
    }
    Ok(job)
}
/// Only authenticated source completion can release the signing input.
pub(crate) fn finish_owned(
    job: Box<PendingCheckpoint>,
) -> Result<(Box<SealedBodyLifecycleSession>, DurablyAdmittedMembership), Box<PendingCheckpoint>> {
    let (owner, pending) = job.adopt()?;
    Ok((
        owner,
        DurablyAdmittedMembership {
            receipt: pending.receipt,
        },
    ))
}
