//! Read-only projection of an exact retained provider Sign; never submits or retries.
use crate::{validate_sign_artifact, SignArtifact};
use alloc::{format, string::String};
use conduit_core::{Observation, ObservationKind, Plan, PlannedGear};
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_process::{PrintLifecycle, PRINT_MAXIMUM_WIRE_BYTES};

/// Borrows authoritative evidence instead of keeping another job state machine.
pub struct PrintInspection<'a> {
    sign: &'a Observation,
    plan: &'a Plan,
    provider: &'a PlannedGear,
    lifecycle: PrintLifecycle,
}

pub fn inspect_print_sign(artifact: &SignArtifact) -> Result<PrintInspection<'_>, String> {
    validate_sign_artifact(artifact)?;
    let sign = &artifact.sign;
    let plan = artifact
        .plan
        .as_ref()
        .or_else(|| artifact.play.as_ref().map(|p| &p.plan))
        .ok_or("print Sign lacks exact Plan")?;
    inspect_provider_event(plan, sign)
}

fn inspect_provider_event<'a>(
    plan: &'a Plan,
    sign: &'a Observation,
) -> Result<PrintInspection<'a>, String> {
    if sign.plan_id.as_ref() != Some(&plan.plan_id) {
        return Err("print Sign names another Plan".into());
    }
    let placement_id = sign
        .placement_id
        .as_ref()
        .ok_or("print Sign lacks placement")?;
    let provider = plan
        .fragments
        .iter()
        .filter(|f| f.host_id == sign.host_id && f.boot_id == sign.boot_id)
        .flat_map(|f| &f.placements)
        .find(|p| {
            &p.placement_id == placement_id
                && p.host_id == sign.host_id
                && p.boot_id == sign.boot_id
        })
        .ok_or("print Sign provider does not match its Plan")?;
    let ObservationKind::ValueProduced { value } = &sign.kind else {
        return Err("print inspection requires a provider-produced lifecycle value".into());
    };
    if value.encoded.len() > PRINT_MAXIMUM_WIRE_BYTES {
        return Err("print lifecycle exceeds its finite envelope".into());
    }
    let ty = PrintLifecycle::semantic_type().map_err(|_| "print semantic Type unavailable")?;
    let profile = ty.profile().map_err(|_| "print Type lacks profile")?;
    let kind = profile.value_kind();
    if &value.value_kind != kind || !provider.outputs.iter().any(|p| &p.value_kind == kind) {
        return Err("print Sign is not an admitted lifecycle output".into());
    }
    let lifecycle =
        PrintLifecycle::decode(&value.encoded).map_err(|_| "invalid typed print lifecycle")?;
    Ok(PrintInspection {
        sign,
        plan,
        provider,
        lifecycle,
    })
}

impl PrintInspection<'_> {
    pub fn sign(&self) -> &Observation {
        self.sign
    }
    pub fn plan(&self) -> &Plan {
        self.plan
    }
    pub fn provider(&self) -> &PlannedGear {
        self.provider
    }
    pub fn lifecycle(&self) -> &PrintLifecycle {
        &self.lifecycle
    }

    /// Describes only this event's evidence. Missing ACK never grants retry permission.
    pub fn render_text(&self) -> String {
        let (state, job, attempt, provider_job) = match &self.lifecycle {
            PrintLifecycle::Queued(v) => ("queued", v.job_id(), v.attempt_id(), None),
            PrintLifecycle::Dispatched(v) => (
                "dispatched; ACK not observed",
                v.job_id(),
                v.attempt_id(),
                None,
            ),
            PrintLifecycle::Acknowledged(v) => (
                "provider acknowledged",
                v.operation().job_id(),
                v.operation().attempt_id(),
                Some(v.provider_job_id()),
            ),
            PrintLifecycle::Completed(v) => (
                "completed",
                v.operation().job_id(),
                v.operation().attempt_id(),
                Some(v.provider_job_id()),
            ),
            PrintLifecycle::Cancelled(v) => (
                "cancelled before dispatch",
                v.job_id(),
                v.attempt_id(),
                None,
            ),
            PrintLifecycle::Failed(v) => (
                "failed (provider evidence)",
                v.operation().job_id(),
                v.operation().attempt_id(),
                None,
            ),
            PrintLifecycle::Uncertain(v) => (
                "delivery uncertain; retry not authorized",
                v.operation().job_id(),
                v.operation().attempt_id(),
                v.provider_job_id().as_ref(),
            ),
        };
        let plan = self.plan;
        format!("Print: {state}\njob={job:?} attempt={attempt:?} provider_job={provider_job:?}\nsource={} checked={} expanded={} plan={}\nprovider={} implementation={} artifact={} host={} boot={} placement={} sign={}\nevent={:?}\n",
            plan.source_document_id.as_str(), plan.checked_plot_id.as_str(), plan.expanded_plot_id.as_str(), plan.plan_id.as_str(),
            self.provider.capability_id.as_str(), self.provider.implementation_id.as_str(), self.provider.artifact_id.as_str(), self.provider.host_id.as_str(), self.provider.boot_id.as_str(), self.provider.placement_id.as_str(), self.sign.sign_id.as_str(), self.lifecycle)
    }
}

/// Called after the ordinary snapshot validator, using its retained exact Plans.
pub(crate) fn snapshot_print_events(
    snapshot: &crate::ObservatorySnapshot,
) -> alloc::vec::Vec<String> {
    snapshot
        .observations
        .iter()
        .map(|sign| (sign, false))
        .chain(
            snapshot
                .historical_observations
                .iter()
                .map(|sign| (sign, true)),
        )
        .filter_map(|(sign, historical)| {
            let plan = snapshot
                .plans
                .iter()
                .find(|p| Some(&p.plan_id) == sign.plan_id.as_ref())?;
            inspect_provider_event(plan, sign).ok().map(|inspection| {
                format!(
                    "history={}\n{}",
                    if historical { "historical" } else { "current" },
                    inspection.render_text()
                )
            })
        })
        .collect()
}
