//! Preserve exact evidence before releasing effect custody; distinguish success.
use super::effects::Custody;
use conduit::creche_encounter::{CrecheEncounterAdapter, CrecheEncounterController};
use conduit_body_make::deployment_runtime::{CrecheDisposition, CrecheOutcome};
use conduit_std_host::{hosted_creche::HostedCrecheAdapter, ExternalForeDelivery};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::Path;

pub(super) struct CompletionFacts {
    pub outcome_acknowledged: bool,
    pub stopped_after_outcome: bool,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn finish(
    controller: &CrecheEncounterController,
    adapter: &CrecheEncounterAdapter,
    delivery: Option<&ExternalForeDelivery>,
    execution: &Value,
    manufacture: &Custody,
    facts: CompletionFacts,
    directory: &Path,
    ui_error: Option<&str>,
    native_image_identity: &str,
) -> Result<(), String> {
    let mut errors = Vec::new();
    let mut make_refusal = None;
    let held = adapter.retained_outcome();
    let delivered = delivery.and_then(|d| CrecheOutcome::decode(&d.bytes));
    if delivery.is_some() && (delivered.is_none() || delivered != held) {
        errors.push("delivered outcome differs from retained adapter outcome".to_string());
    }
    if let Some(outcome) = held {
        match adapter.retained_evidence(&outcome) {
            Ok(evidence)
                if <[u8; 32]>::from(Sha256::digest(evidence)) == outcome.evidence_digest =>
            {
                if let Err(e) = super::super::write_bytes_atomic(
                    &directory.join("canonical-evidence.bin"),
                    evidence,
                ) {
                    errors.push(e);
                }
            }
            _ => errors.push("retained canonical outcome evidence digest differs".into()),
        }
    }
    let saved = controller.saved_receipts().unwrap_or_else(|e| {
        errors.push(e);
        vec![]
    });
    match manufacture.try_lock() {
        Ok(retained) => {
            make_refusal = retained.refusal.clone();
            let record = json!({"refusal":retained.refusal,
                "birth":retained.job.as_ref().map(|j| j.biography()),
                "selected_hosts":retained.job.as_ref().map(|j| j.selected_hosts()),
                "artifacts":retained.job.as_ref().map(|j| j.artifacts().iter().map(|a| json!({"prepared_birth":a.receipt(),"builder":a.builder_receipt()})).collect::<Vec<_>>()),
                "resume_authorized":false});
            if let Err(e) =
                super::super::write_json_atomic(&directory.join("manufacture.json"), &record)
            {
                errors.push(e);
            }
        }
        Err(_) => errors.push("manufacture custody remains busy or poisoned".into()),
    }
    match controller.take_rejected_products() {
        Ok(Some(rejected)) => {
            let record = json!({"reason":rejected.reason(), "birth":rejected.products().biography(), "artifacts":rejected.products().artifacts().iter().map(|a| json!({"prepared_birth":a.receipt(),"builder":a.builder_receipt()})).collect::<Vec<_>>(), "resume_authorized":false});
            if let Err(e) =
                super::super::write_json_atomic(&directory.join("rejected-products.json"), &record)
            {
                errors.push(e);
            }
        }
        Ok(None) => {}
        Err(e) => errors.push(e),
    }
    match controller.take_retired_products() {
        Ok(Some(products)) => {
            let record = json!({"birth":products.biography(), "artifacts":products.artifacts().iter().map(|a| json!({"prepared_birth":a.receipt(),"builder":a.builder_receipt()})).collect::<Vec<_>>(), "resume_authorized":false});
            if let Err(e) =
                super::super::write_json_atomic(&directory.join("retired-products.json"), &record)
            {
                errors.push(e);
            }
        }
        Ok(None) => {}
        Err(e) => errors.push(e),
    }
    let terminal =
        serde_json::from_value::<conduit_core::TerminalDisposition>(execution["terminal"].clone())
            .ok();
    let kernel_retired_cleanly = (terminal == Some(conduit_core::TerminalDisposition::Completed)
        || (facts.stopped_after_outcome
            && terminal
                == Some(conduit_core::TerminalDisposition::Cancelled {
                    reason: conduit_core::CancellationReason::OperatorRequested,
                })))
        && facts.outcome_acknowledged
        && execution["play"].is_object()
        && execution["failure"].is_null()
        && execution["cleanup_failure"].is_null()
        && execution["fore_pending_output"].is_null();
    let disposition = held.map(|outcome| match outcome.disposition {
        CrecheDisposition::Prepared => "prepared",
        CrecheDisposition::Realized => "realized",
        CrecheDisposition::Refused => "refused",
        CrecheDisposition::Cancelled => "cancelled",
    });
    // This application supplies make/save grants only, never a carrier operation.
    let prepared = disposition == Some("prepared")
        && delivered == held
        && held.is_some()
        && kernel_retired_cleanly
        && ui_error.is_none()
        && errors.is_empty();
    let record = json!({"schema":"conduit.install/native-creche-terminal@1", "ui_error":ui_error, "make_refusal":make_refusal,
        "native_image_identity":native_image_identity, "adapter_disposition":disposition,
        "outcome_fore_acknowledged":facts.outcome_acknowledged,
        "foreground_play_stopped_after_outcome":facts.stopped_after_outcome,
        "adapter_request":held.map(|o| o.request.0), "canonical_evidence_digest":held.map(|o| o.evidence_digest),
        "delivered_outcome_bytes":delivery.map(|d| &d.bytes), "execution":execution,
        "prepared":prepared,"saved":saved,"retention_errors":errors,
        "carrier_realized":false,"boot_observed":false,"join_observed":false});
    super::super::write_json_atomic(&directory.join("terminal.json"), &record)?;
    if prepared {
        Ok(())
    } else {
        Err(format!("native Crèche ended without successful preparation ({disposition:?}, {}); exact terminal evidence retained in {}", ui_error.or(make_refusal.as_deref()).unwrap_or("see retained execution and canonical evidence"), directory.display()))
    }
}

#[cfg(test)]
mod tests;
