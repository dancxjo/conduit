//! Explicit exposure of one private Seed; exposure is neither Boot nor Join.
use super::*;
use conduit_body_make::womb_runtime::WombDisposition;

pub(in crate::workspace_runtime) fn begin_delivery(
    body: &BodyLifecycleSession,
    host: &HostId,
    boot: &BootId,
    id: [u8; 32],
    now: u64,
) -> Result<Vec<u8>, Refusal> {
    require_issuer(body, host, boot)?;
    PROVISION.with(|slot| {
        let mut slot=slot.borrow_mut();
        let provision=slot.as_mut().ok_or("Womb has no private Seed")?;
        if provision.id!=id || &provision.host!=host || &provision.boot!=boot {return Err(Refusal::new("Womb.StaleReservation","Delivery names another provision"));}
        let binding=provision.binding.as_ref().ok_or("Womb has no actual Body binding")?;
        let play=binding.play.as_ref().ok_or("Womb has not Started")?;
        if body.realization().is_none_or(|actual|actual.plan.plan_id!=binding.plan || actual.play.as_ref()!=Some(play)) {
            return Err(Refusal::new("Womb.StalePlay","Delivery differs from the current completed Womb Play"));
        }
        if !provision.kernel_outcome.is_some_and(|outcome|outcome.disposition==WombDisposition::Prepared) {
            return Err(Refusal::new("Womb.NotPrepared","The kernel has not emitted a Prepared result"));
        }
        provision.invitation.claim().inspect(now).map_err(|error|Refusal::new(&format!("Admission.{error:?}"),"Delivery invitation is stale"))?;
        ADMISSIONS.with(|slot| {
            let slot=slot.borrow();
            slot.as_ref().ok_or("Womb issuer manager is absent")?.validate_owned_pending_spawn_invitation(&provision.invitation)
                .map_err(|error|Refusal::new(&format!("Womb.Custody.{error:?}"),"Original pending invitation is no longer current"))
        })?;
        let seed=provision.material.as_ref().and_then(|material|material.seed.as_ref()).ok_or("Private Seed candidate is absent")?;
        let receipt=seed.receipt.as_ref().ok_or("Private Seed candidate has no completed package receipt")?;
        let response=encode(&serde_json::json!({"schema":"conduit.womb/delivery-access@1","artifact":receipt,
            "delivery_committed":true,"invitation_delivered":false,"boot_observed":false,"membership_claimed":false}))?;
        // Commit before exposing the pointer: after this point another local
        // consumer may have copied the secret-bearing target provisioning.
        provision.delivery_committed=true;
        Ok(response)
    })
}

/// Release private provisioning only after the actual original manager proves
/// that this exact invitation was consumed. Other Plots may still be playing.
pub(in crate::workspace_runtime) fn release_after_admission(
    body: &BodyLifecycleSession,
    host: &HostId,
    boot: &BootId,
    id: [u8; 32],
) -> Result<Vec<u8>, Refusal> {
    require_issuer(body, host, boot)?;
    PROVISION.with(|slot| {
        let mut slot=slot.borrow_mut();
        let provision=slot.as_ref().ok_or("Womb has no retained Seed")?;
        if provision.id!=id || provision.host!=*host || provision.boot!=*boot {
            return Err(Refusal::new("Womb.StaleReservation","Release names another private provision"));
        }
        if provision.target_job_pending || !provision.delivery_committed
            || !provision.kernel_outcome.is_some_and(|outcome|outcome.disposition==WombDisposition::Prepared) {
            return Err(Refusal::new("Womb.NotAdmitted","A private or unfinished candidate cannot be retired as admitted"));
        }
        ADMISSIONS.with(|manager| {
            let manager=manager.borrow();
            match manager.as_ref().ok_or("Womb issuer manager is absent")?.validate_owned_pending_spawn_invitation(&provision.invitation) {
                Err(conduit_body::PendingInvitationRevocationRefusal::AlreadyUsed)=>Ok(()),
                Ok(())=>Err(Refusal::new("Womb.NotAdmitted","The original invitation is still unused")),
                Err(error)=>Err(Refusal::new(&format!("Womb.Custody.{error:?}"),"Original invitation consumption is unproven")),
            }
        })?;
        let response=encode(&serde_json::json!({"schema":"conduit.womb/admitted-retirement@1",
            "body_id":provision.invitation.body_id,"invitation_id":provision.invitation.invitation_id,
            "invitation_consumed":true,"candidate_released":true}))?;
        // The canonical manager marks an invitation used only after attaching
        // its Part and retaining the receipt. Dropping wipes the private ZIP;
        // no revocation or rollback of admitted membership is performed.
        *slot=None;
        Ok(response)
    })
}

#[no_mangle]
pub extern "C" fn conduit_womb_seed_output_ptr() -> usize {
    PROVISION.with(|slot| {
        let slot = slot.borrow();
        slot.as_ref()
            .filter(|provision| provision.delivery_committed)
            .and_then(|provision| provision.material.as_ref())
            .and_then(|material| material.seed.as_ref())
            .map_or(0, |seed| seed.bytes.as_ptr() as usize)
    })
}
#[no_mangle]
pub extern "C" fn conduit_womb_seed_output_len() -> usize {
    PROVISION.with(|slot| {
        let slot = slot.borrow();
        slot.as_ref()
            .filter(|provision| provision.delivery_committed)
            .and_then(|provision| provision.material.as_ref())
            .and_then(|material| material.seed.as_ref())
            .map_or(0, |seed| seed.bytes.len())
    })
}
