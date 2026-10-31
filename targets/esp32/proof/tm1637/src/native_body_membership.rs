//! Membership-only invitation completion; no equipment description or offer
//! becomes available through this path. Durable Root custody precedes signing.
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
    // Membership changes invalidate a BodyTime-pinned membership revision.
    // Preserve the existing active-Body safeguard, independently of equipment.
    if &request.host_id == authority_host
        || owner
            .realization()
            .is_some_and(|current| current.plan.body_time_requirement().is_some())
    {
        return Err("body-membership-profile");
    }
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
        receipt: PortableMembershipAdmissionReceipt {
            schema: MEMBERSHIP_ADMISSION_RECEIPT_SCHEMA.into(),
            protocol_version: conduit_core::PROTOCOL_VERSION,
            credential,
            membership_revision: owner.evidence().membership.revision.0,
            producer_host: authority_host.clone(),
            producer_boot: authority_boot.clone(),
            producer_key: *root_key,
            invitation_id: proof.invitation_id,
            offer_generation: request.offer_generation,
            signature: [0; 64],
            request_digest: request.digest(),
            membership_admitted: true,
            current_offers_available: false,
            plan_created: false,
            play_created: false,
        },
    })
}
