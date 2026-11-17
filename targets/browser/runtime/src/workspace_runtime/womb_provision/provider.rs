//! One correlated provider attempt and retained kernel-visible result.
use super::*;
use conduit_body_make::womb_runtime::{WombDisposition, WombRequest};
use conduit_core::{ActivePlayId, PlacementId, PlanId};
use std::io::Write;

struct EvidenceWriter<'a>(&'a mut Vec<u8>);
impl Write for EvidenceWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if self
            .0
            .len()
            .checked_add(bytes.len())
            .is_none_or(|size| size > 4096)
        {
            return Err(std::io::Error::other(
                "Womb provider evidence exceeds its reserved bound",
            ));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[derive(Serialize)]
struct Evidence<'a> {
    schema: &'static str,
    reservation: &'a [u8; 32],
    request: &'a [u8; 32],
    play: &'a BodyPlayIdentity,
    expected_release_digest: &'a str,
    disposition: &'static str,
    artifact: Option<&'a conduit_body_make::JoiningHostPackageReceipt>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::workspace_runtime) struct ProviderCall {
    pub(super) host_id: HostId,
    pub(super) boot_id: BootId,
    pub(super) reservation_id: [u8; 32],
    pub(super) active_play_id: ActivePlayId,
    pub(super) placement_id: PlacementId,
    pub(super) request_sequence: u32,
    pub(super) now_millis: u64,
}

pub(in crate::workspace_runtime) fn run(
    body: &BodyLifecycleSession,
    call: ProviderCall,
) -> Result<Vec<u8>, Refusal> {
    require_issuer(body, &call.host_id, &call.boot_id)?;
    let now_millis = call.now_millis;
    PROVISION.with(|slot| {
        let mut slot = slot.borrow_mut();
        let provision = slot.as_mut().ok_or("Womb has no private provision")?;
        validate_current(body,&call,provision)?;
        if provision.target_job_pending {return Err(Refusal::new("Womb.ProviderPending","The selected browser producer has not drained"));}
        let binding=provision.binding.as_ref().expect("validated current binding");
        let started=binding.play.as_ref().expect("validated Started binding");
        if provision.provider_outcome.is_none() {
            if provision.provider_attempted {
                return Err(Refusal::new("Womb.ProviderFailed", "The one admitted provider attempt failed before retaining a result"));
            }
            provision.provider_attempted = true;
            // A digest selected during reservation is not verified builder
            // material. No artifact may be claimed without retained bytes.
            let (disposition, name) = if provision.invitation.claim().inspect(now_millis).is_err() {
                (WombDisposition::StaleInvitation, "stale-invitation")
            } else if let Some(failure)=provision.target_job_failure {
                (failure,"selected-browser-producer-terminal")
            } else if provision.material.as_ref().is_some_and(|material| material.derived.is_some()) {
                ADMISSIONS.with(|slot| {
                    let slot = slot.borrow();
                    let manager = slot.as_ref().ok_or("Womb issuer manager is absent")?;
                    manager.validate_owned_pending_spawn_invitation(&provision.invitation).map_err(|error|format!("Pending invitation custody: {error:?}"))?;
                    seed::prepare(provision.material.as_mut().expect("retained derived material"),body,&provision.configuration,&provision.invitation,now_millis)
                }).map(|()|(WombDisposition::Prepared,"prepared-private-seed"))
                    .unwrap_or((WombDisposition::Failed,"seed-preparation-failed"))
            } else {
                (WombDisposition::UnavailableBuilder, if provision.material.as_ref().is_some_and(|material| material.is_verified()) { "browser-image-validation-not-prepared" } else { "verified-release-not-staged" })
            };
            provision.provider_evidence.clear();
            serde_json::to_writer(EvidenceWriter(&mut provision.provider_evidence), &Evidence {
                schema: "conduit.womb/provider-evidence@1", reservation: &provision.id,
                request: &binding.request.0, play: started,
                expected_release_digest: &provision.expected_release_digest, disposition: name,
                artifact:provision.material.as_ref().and_then(|material|material.seed.as_ref()).and_then(|seed|seed.receipt.as_ref()),
            }).map_err(|error| Refusal::new("Womb.EvidenceBound", error.to_string()))?;
            provision.provider_outcome = Some(WombOutcome { request: binding.request, disposition, evidence_digest: Sha256::digest(&provision.provider_evidence).into() });
        }
        let outcome = provision.provider_outcome.expect("retained bounded provider result");
        encode(&serde_json::json!({"schema":"conduit.womb/provider-completion@1", "outcome":outcome.encode().as_slice()}))
    })
}

pub(crate) fn validate_completion(request: WombRequest, bytes: &[u8]) -> Result<(), String> {
    let outcome = WombOutcome::decode(bytes).ok_or("malformed Womb completion")?;
    PROVISION.with(|slot| {
        let slot = slot.borrow();
        let provision = slot
            .as_ref()
            .ok_or("Womb completion has no private owner")?;
        let binding = provision
            .binding
            .as_ref()
            .ok_or("Womb completion has no actual Body binding")?;
        if binding.request != request
            || outcome.request != request
            || binding.play.is_none()
            || provision.provider_outcome != Some(outcome)
        {
            return Err("Womb completion differs from retained provider evidence".into());
        }
        Ok(())
    })
}

pub(crate) fn record_kernel_outcome(
    plan: &PlanId,
    placement: &PlacementId,
    outcome: WombOutcome,
) -> Result<(), String> {
    PROVISION.with(|slot| {
        let mut slot = slot.borrow_mut();
        let provision = slot.as_mut().ok_or("Womb output has no private owner")?;
        let binding = provision
            .binding
            .as_ref()
            .ok_or("Womb output has no Body binding")?;
        if &binding.plan != plan
            || &binding.placement.placement_id != placement
            || provision.provider_outcome != Some(outcome)
        {
            return Err("Womb output differs from its exact retained provider result".into());
        }
        provision.kernel_outcome = Some(outcome);
        Ok(())
    })
}

pub(in crate::workspace_runtime) fn result(
    body: &BodyLifecycleSession,
    host: &HostId,
    boot: &BootId,
    id: [u8; 32],
) -> Result<Vec<u8>, Refusal> {
    require_issuer(body, host, boot)?;
    PROVISION.with(|slot| {
        let slot = slot.borrow();
        let provision = slot.as_ref().ok_or("Womb has no retained result")?;
        if provision.id != id { return Err(Refusal::new("Womb.StaleReservation", "Result names another provision")); }
        encode(&serde_json::json!({"schema":"conduit.womb/result@1", "provider_started":provision.provider_attempted, "provider_completed":provision.provider_outcome.is_some(), "kernel_outcome":provision.kernel_outcome.map(|outcome| outcome.encode().to_vec()), "artifact_prepared":provision.material.as_ref().and_then(|material|material.seed.as_ref()).is_some_and(|seed|seed.receipt.is_some()),"delivery_committed":provision.delivery_committed,"invitation_delivered":false,"membership_claimed":false}))
    })
}

pub(super) fn validate_current(
    body: &BodyLifecycleSession,
    call: &ProviderCall,
    provision: &Provision,
) -> Result<(), Refusal> {
    require_issuer(body, &call.host_id, &call.boot_id)?;
    if provision.id != call.reservation_id
        || provision.host != call.host_id
        || provision.boot != call.boot_id
    {
        return Err(Refusal::new(
            "Womb.StaleReservation",
            "Provider names another reservation",
        ));
    }
    let binding = provision
        .binding
        .as_ref()
        .ok_or_else(|| Refusal::new("Womb.NotStarted", "Womb has no prepared Body binding"))?;
    let started = binding.play.as_ref().ok_or_else(|| {
        Refusal::new(
            "Womb.NotStarted",
            "Record the actual kernel Started callback first",
        )
    })?;
    if started.active_play_id != call.active_play_id
        || binding.placement.placement_id != call.placement_id
        || body.realization().is_none_or(|actual| {
            actual.plan.plan_id != binding.plan || actual.play.as_ref() != Some(started)
        })
    {
        return Err(Refusal::new(
            "Womb.StalePlay",
            "Provider differs from the actual current Womb Play",
        ));
    }
    crate::plot_runner::womb_provider::validate_pending(
        started,
        &binding.placement,
        call.request_sequence,
        binding.request,
    )
    .map_err(|error| Refusal::new("Womb.StaleCall", error))?;
    Ok(())
}
