//! One selected async JS producer, fenced by the actual pending Host Call.
use super::*;

pub(in crate::workspace_runtime) fn begin_job(
    body: &BodyLifecycleSession,
    call: ProviderCall,
) -> Result<Vec<u8>, Refusal> {
    PROVISION.with(|slot| {
        let mut slot=slot.borrow_mut();
        let provision=slot.as_mut().ok_or("Womb has no private provision")?;
        provider::validate_current(body,&call,provision)?;
        if provision.target_job_pending || provision.provider_attempted || provision.target_job_failure.is_some() {
            return Err(Refusal::new("Womb.ProviderBusy","One browser producer attempt is already retained"));
        }
        if !provision.material.as_ref().is_some_and(staging::Material::is_verified) {return Err(Refusal::new("Womb.UnavailableBuilder","Verified distribution bytes are absent"));}
        ADMISSIONS.with(|slot| {
            let slot=slot.borrow();
            slot.as_ref().ok_or("Womb issuer manager is absent")?.validate_owned_pending_spawn_invitation(&provision.invitation)
                .map_err(|error|Refusal::new(&format!("Womb.Custody.{error:?}"),"Original invitation is no longer pending"))
        })?;
        provision.invitation.claim().inspect(call.now_millis).map_err(|error|Refusal::new(&format!("Admission.{error:?}"),"Producer invitation is stale"))?;
        let pin=provision.release_pin.as_ref().ok_or("Womb release selection is unpinned")?;
        let checked=crate::creche::womb_configuration_review(&provision.configuration).map_err(|error|Refusal::new("Womb.Configuration",error))?;
        let response=encode(&serde_json::json!({"schema":"conduit.womb/private-host-job@1","reservation_id":provision.id,
            "release_manifest_digest":pin.manifest_digest,"release_source_identity":pin.source_identity,
            "distribution_digest":provision.expected_release_digest,"configuration_id":checked["configuration_id"],"profile_id":checked["profile_id"],"checked":checked}))?;
        provision.target_job_pending=true;
        provision.target_job_sequence=Some(call.request_sequence);
        Ok(response)
    })
}

pub(in crate::workspace_runtime) fn complete_job(
    body: &BodyLifecycleSession,
    call: ProviderCall,
    manifest: Vec<u8>,
    image: Vec<u8>,
) -> Result<Vec<u8>, Refusal> {
    PROVISION.with(|slot| {
        let mut slot = slot.borrow_mut();
        let provision = slot.as_mut().ok_or("Womb has no private provision")?;
        provider::validate_current(body, &call, provision)?;
        if !provision.target_job_pending || provision.provider_attempted {
            return Err(Refusal::new(
                "Womb.StaleCall",
                "Browser producer no longer owns this pending call",
            ));
        }
        // Reborrow actual issuer after the JS builder/validator awaits. No
        // manager reconstructed from a durable snapshot can authorize output.
        ADMISSIONS.with(|slot| {
            let slot = slot.borrow();
            slot.as_ref()
                .ok_or("Womb issuer manager is absent")?
                .validate_owned_pending_spawn_invitation(&provision.invitation)
                .map_err(|error| {
                    Refusal::new(
                        &format!("Womb.Custody.{error:?}"),
                        "Original invitation changed during browser preparation",
                    )
                })
        })?;
        provision
            .invitation
            .claim()
            .inspect(call.now_millis)
            .map_err(|error| {
                Refusal::new(
                    &format!("Admission.{error:?}"),
                    "Invitation expired during browser preparation",
                )
            })?;
        let material = provision
            .material
            .as_mut()
            .ok_or("Original distribution bytes are absent")?;
        let derived = browser_release::retain(material, &provision.configuration, manifest, image)
            .map_err(|error| Refusal::new("Womb.InvalidRelease", error))?;
        material.derived = Some(derived);
        provision.target_job_pending = false;
        Ok(b"{\"schema\":\"conduit.womb/private-host-output@1\",\"retained\":true}".to_vec())
    })
}

pub(in crate::workspace_runtime) fn fail_job(
    body: &BodyLifecycleSession,
    call: ProviderCall,
    cancelled: bool,
) -> Result<Vec<u8>, Refusal> {
    require_issuer(body, &call.host_id, &call.boot_id)?;
    PROVISION.with(|slot| {
        let mut slot = slot.borrow_mut();
        let provision = slot.as_mut().ok_or("Womb has no private producer")?;
        if provision.id != call.reservation_id
            || provision.host != call.host_id
            || provision.boot != call.boot_id
            || !provision.target_job_pending
        {
            return Err(Refusal::new(
                "Womb.StaleCall",
                "Cleanup names another producer owner",
            ));
        }
        if provision.target_job_sequence != Some(call.request_sequence)
            || provision.binding.as_ref().is_none_or(|binding| {
                binding.placement.placement_id != call.placement_id
                    || binding
                        .play
                        .as_ref()
                        .is_none_or(|play| play.active_play_id != call.active_play_id)
            })
        {
            return Err(Refusal::new(
                "Womb.StaleCall",
                "Cleanup differs from the exact original producer call",
            ));
        }
        provision.target_job_pending = false;
        provision.target_job_failure = Some(if cancelled {
            conduit_body_make::womb_runtime::WombDisposition::Cancelled
        } else {
            conduit_body_make::womb_runtime::WombDisposition::Failed
        });
        Ok(b"{\"schema\":\"conduit.womb/private-host-drained@1\"}".to_vec())
    })
}
