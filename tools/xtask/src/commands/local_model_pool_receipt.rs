//! Strict import boundary for one live multi-Host model-pool receipt.

use conduit_core::PoolRealizationEnvelope;
use serde::Deserialize;
use std::{collections::BTreeSet, path::Path};

const SCHEMA: &str = "conduit.proof/live-local-model-pool@1";
const MAXIMUM_REALIZATIONS: usize = 16;
const MAXIMUM_OPERATIONS: usize = 16;
const MAXIMUM_SIGNS_PER_OPERATION: usize = 128;
const MAXIMUM_IDENTITY_BYTES: usize = 192;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LiveLocalModelPoolReceipt {
    schema: String,
    body_id: String,
    wake_id: String,
    source_document_id: String,
    checked_form_id: String,
    expanded_form_id: String,
    plan_id: String,
    play_id: String,
    pool_id: String,
    member_kind_id: String,
    member_kind_contract_revision: String,
    realization_envelope: Vec<PoolRealizationEnvelope>,
    operations: Vec<LivePoolOperation>,
    prompt_content_retained: bool,
    physical_evidence: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LivePoolOperation {
    operation_id: String,
    realization: u16,
    host_id: String,
    boot_id: String,
    capability_id: String,
    member_key: Vec<u8>,
    member_slot: u16,
    member_epoch: u32,
    member_node: u16,
    member_play: u16,
    population_at_admission: u16,
    selection_sign_id: String,
    observation_sign_ids: Vec<String>,
    result_bytes: usize,
    disposition: String,
}

impl LiveLocalModelPoolReceipt {
    pub(super) fn plan_id(&self) -> &str {
        &self.plan_id
    }

    pub(super) fn play_id(&self) -> &str {
        &self.play_id
    }

    pub(super) fn operation_count(&self) -> usize {
        self.operations.len()
    }

    fn validate(&self) -> Result<(), String> {
        if self.schema != SCHEMA {
            return Err("live local-model receipt uses an unknown schema".into());
        }
        for (label, identity) in [
            ("Body", &self.body_id),
            ("Wake", &self.wake_id),
            ("source Form", &self.source_document_id),
            ("checked Form", &self.checked_form_id),
            ("expanded Form", &self.expanded_form_id),
            ("Plan", &self.plan_id),
            ("Play", &self.play_id),
            ("pool", &self.pool_id),
        ] {
            validate_identity(label, identity)?;
        }
        if self.prompt_content_retained {
            return Err("live local-model receipt retained prompt content".into());
        }
        if self.member_kind_id != "ai/generate-text"
            || self.member_kind_contract_revision != "conduit.ai/generate-text@1"
        {
            return Err("live receipt does not name the canonical ai/generate-text front".into());
        }
        if self.physical_evidence {
            return Err("Workspace receipt cannot self-assert attended physical evidence".into());
        }
        if !(2..=MAXIMUM_REALIZATIONS).contains(&self.realization_envelope.len()) {
            return Err("live local-model receipt requires two to sixteen realizations".into());
        }
        let first = &self.realization_envelope[0];
        let mut hosts = BTreeSet::new();
        for realization in &self.realization_envelope {
            if realization.member_capacity == 0 || realization.resources.is_empty() {
                return Err("live realization lacks finite member/resource capacity".into());
            }
            if realization.implementation_id != first.implementation_id
                || realization.artifact_id != first.artifact_id
            {
                return Err("live realizations do not expose the same exact model back".into());
            }
            if !hosts.insert((realization.host_id.clone(), realization.boot_id.clone())) {
                return Err("live realization envelope repeats a Host/Boot".into());
            }
        }
        if hosts.len() < 2 {
            return Err("live receipt does not contain two exact Hosts".into());
        }
        if !(2..=MAXIMUM_OPERATIONS).contains(&self.operations.len()) {
            return Err("live receipt requires two to sixteen completed operations".into());
        }
        let mut operation_ids = BTreeSet::new();
        let mut selected_realizations = BTreeSet::new();
        for operation in &self.operations {
            validate_identity("operation", &operation.operation_id)?;
            validate_identity("selection Sign", &operation.selection_sign_id)?;
            if !operation_ids.insert(operation.operation_id.clone()) {
                return Err("live receipt repeats an operation identity".into());
            }
            let realization = self
                .realization_envelope
                .get(usize::from(operation.realization))
                .ok_or_else(|| "live operation selected outside the Plan envelope".to_string())?;
            if operation.host_id != realization.host_id.as_str()
                || operation.boot_id != realization.boot_id.as_str()
                || operation.capability_id != realization.capability_id.as_str()
            {
                return Err("live operation identity differs from its selected realization".into());
            }
            if operation.member_key.len() != 32
                || operation.member_epoch == 0
                || operation.population_at_admission == 0
                || usize::from(operation.population_at_admission) > self.realization_envelope.len()
            {
                return Err("live operation has invalid finite member admission truth".into());
            }
            let _member_identity = (
                operation.member_slot,
                operation.member_node,
                operation.member_play,
            );
            if operation.observation_sign_ids.is_empty()
                || operation.observation_sign_ids.len() > MAXIMUM_SIGNS_PER_OPERATION
                || operation
                    .observation_sign_ids
                    .iter()
                    .any(|identity| validate_identity("observation Sign", identity).is_err())
            {
                return Err("live operation has invalid bounded observation Signs".into());
            }
            if operation.result_bytes == 0 || operation.disposition != "Completed" {
                return Err("live operation did not retain an exact successful terminal".into());
            }
            selected_realizations.insert(operation.realization);
        }
        if selected_realizations.len() < 2 {
            return Err("live operations do not prove work on two selected realizations".into());
        }
        if !self
            .operations
            .iter()
            .any(|operation| operation.population_at_admission >= 2)
        {
            return Err("live operations do not prove overlapping admitted members".into());
        }
        Ok(())
    }
}

fn validate_identity(label: &str, value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > MAXIMUM_IDENTITY_BYTES
        || value.chars().any(char::is_whitespace)
    {
        return Err(format!("live receipt {label} identity is invalid"));
    }
    Ok(())
}

pub(super) fn read(path: &Path) -> Result<(LiveLocalModelPoolReceipt, Vec<u8>), String> {
    let bytes = std::fs::read(path)
        .map_err(|error| format!("cannot read live receipt {}: {error}", path.display()))?;
    if bytes.is_empty() || bytes.len() > 512 * 1024 {
        return Err("live local-model receipt exceeds its finite file bound".into());
    }
    let receipt: LiveLocalModelPoolReceipt = serde_json::from_slice(&bytes)
        .map_err(|error| format!("invalid live local-model receipt JSON: {error}"))?;
    receipt.validate()?;
    Ok((receipt, bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn receipt() -> serde_json::Value {
        let realization = |index: u8| {
            serde_json::json!({
                "host_id": format!("host/{index}"),
                "boot_id": format!("boot/{index}"),
                "offer_generation": 1,
                "capability_id": format!("capability/{index}"),
                "implementation_id": "std/local-open-weight-model@1",
                "artifact_id": "model/shared-sha256",
                "member_capacity": 1,
                "resources": [{
                    "pool_id": format!("resource/{index}"),
                    "class_id": "resource/inference-slot",
                    "units": 1,
                    "protected": null,
                    "compute": null,
                    "content": null
                }]
            })
        };
        let operation = |index: u8| {
            serde_json::json!({
                "operation_id": format!("operation/{index}"),
                "realization": index,
                "host_id": format!("host/{index}"),
                "boot_id": format!("boot/{index}"),
                "capability_id": format!("capability/{index}"),
                "member_key": vec![index.saturating_add(1); 32],
                "member_slot": index,
                "member_epoch": 1,
                "member_node": 256 + u16::from(index),
                "member_play": 1,
                "population_at_admission": index + 1,
                "selection_sign_id": format!("sign/selection/{index}"),
                "observation_sign_ids": [format!("sign/observation/{index}")],
                "result_bytes": 12,
                "disposition": "Completed"
            })
        };
        serde_json::json!({
            "schema": SCHEMA,
            "body_id": "body/1", "wake_id": "wake/1",
            "source_document_id": "source/1", "checked_form_id": "checked/1",
            "expanded_form_id": "expanded/1", "plan_id": "plan/1", "play_id": "play/1",
            "pool_id": "pool/1",
            "member_kind_id": "ai/generate-text",
            "member_kind_contract_revision": "conduit.ai/generate-text@1",
            "realization_envelope": [realization(0), realization(1)],
            "operations": [operation(0), operation(1)],
            "prompt_content_retained": false, "physical_evidence": false
        })
    }

    #[test]
    fn accepts_two_exact_hosts_and_rejects_single_host_or_plaintext_claim() {
        let valid: LiveLocalModelPoolReceipt = serde_json::from_value(receipt()).unwrap();
        valid.validate().unwrap();

        let mut one = receipt();
        one["operations"][1]["realization"] = serde_json::json!(0);
        one["operations"][1]["host_id"] = serde_json::json!("host/0");
        one["operations"][1]["boot_id"] = serde_json::json!("boot/0");
        one["operations"][1]["capability_id"] = serde_json::json!("capability/0");
        let one: LiveLocalModelPoolReceipt = serde_json::from_value(one).unwrap();
        assert!(one.validate().unwrap_err().contains("two selected"));

        let mut retained = receipt();
        retained["prompt_content_retained"] = serde_json::json!(true);
        let retained: LiveLocalModelPoolReceipt = serde_json::from_value(retained).unwrap();
        assert!(retained.validate().unwrap_err().contains("prompt content"));
    }
}
