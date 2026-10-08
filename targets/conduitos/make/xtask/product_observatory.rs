//! Correlate a bounded native product export with its current boot and Play.
use super::ConduitosError;
use conduit_observatory::{BootProofClass, ObservatorySnapshot, PlanLifecycle};
use serde_json::Value;

pub(super) fn capture(transcript: &str, product: &Value) -> Result<Option<Value>, ConduitosError> {
    let prefix = conduitos::observatory::EXPORT_PREFIX;
    let Some(json) = super::emitted_line::complete_json_line(transcript, prefix) else {
        return Ok(None);
    };
    if json.len() > conduitos::observatory::MAX_EXPORT_BYTES
        || transcript.matches(prefix).count() != 1
    {
        return Err(refusal("expected one bounded ordinary Observatory export"));
    }
    let snapshot: ObservatorySnapshot =
        serde_json::from_str(json).map_err(|error| refusal(error.to_string()))?;
    conduit_observatory::validate_snapshot(&snapshot).map_err(refusal)?;
    validate_correlation(&snapshot, product)?;
    serde_json::to_value(snapshot)
        .map(Some)
        .map_err(|error| refusal(error.to_string()))
}

fn validate_correlation(
    snapshot: &ObservatorySnapshot,
    product: &Value,
) -> Result<(), ConduitosError> {
    let exact = |field: &str, identity: &str| {
        !identity.is_empty() && product[field].as_str() == Some(identity)
    };
    let valid = snapshot.hosts.len() == 1
        && snapshot.plans.len() == 1
        && snapshot.plays.len() == 1
        && snapshot.sealed_boot_provenance.len() == 1
        && snapshot.retention.dropped_items == 0
        && snapshot.hosts.first().is_some_and(|host| {
            exact("host_id", host.advertisement.host_id.as_str())
                && exact("boot_id", host.advertisement.boot_id.as_str())
                && product["offer_generation"].as_u64()
                    == Some(host.advertisement.offer_generation.0)
        })
        && snapshot.plans.first().is_some_and(|plan| {
            exact(
                "ordinary_source_document_id",
                plan.source_document_id.as_str(),
            ) && exact("ordinary_checked_plot_id", plan.checked_plot_id.as_str())
                && exact("ordinary_expanded_plot_id", plan.expanded_plot_id.as_str())
                && exact("ordinary_plan_id", plan.plan_id.as_str())
        })
        && snapshot.plays.first().is_some_and(|play| {
            exact("ordinary_play_id", play.active_play_id.as_str())
                && exact("ordinary_plan_id", play.plan_id.as_str())
                && exact("host_id", play.host_id.as_str())
                && exact("boot_id", play.boot_id.as_str())
                && play.lifecycle == PlanLifecycle::Completed
        })
        && snapshot
            .sealed_boot_provenance
            .first()
            .is_some_and(|provenance| {
                exact("host_id", provenance.host_id.as_str())
                    && exact("boot_id", provenance.boot_id.as_str())
                    && exact("image_id", provenance.image_id.as_str())
                    && exact("build_id", provenance.build_id.as_str())
                    && provenance.proof_class == BootProofClass::FreestandingEmulator
            });
    if !valid {
        return Err(refusal(
            "sealed Source/Plan/Play and boot provenance disagree with the current product receipt",
        ));
    }
    Ok(())
}

fn refusal(detail: impl Into<String>) -> ConduitosError {
    ConduitosError::refusal("ordinary-product-observatory-invalid", detail)
}

#[cfg(test)]
mod tests;
