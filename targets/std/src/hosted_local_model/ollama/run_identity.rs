//! Adapter counters are scoped to the exact selected Host Boot and placement.
use conduit_core::PlannedGear;
use sha2::{Digest, Sha256};
pub(super) fn identity(kind: &str, placement: &PlannedGear, sequence: u64) -> String {
    let mut digest = Sha256::new();
    digest.update(b"conduit-local-model-attempt-v1\0");
    for part in [
        kind,
        placement.host_id.as_str(),
        placement.boot_id.as_str(),
        placement.placement_id.as_str(),
    ] {
        digest.update((part.len() as u64).to_le_bytes());
        digest.update(part.as_bytes());
    }
    digest.update(sequence.to_le_bytes());
    format!("{kind}/local-model/{:x}", digest.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn placement() -> PlannedGear {
        let host = conduit_ai::llm_generate_base_fixtures()[0]
            .advertisement
            .clone();
        let offer = &host.capabilities[0];
        conduit_core::planned_gear_from_parts! {
            placement_id: "fixture/budget".into(), gear_id: "fixture/budget".into(),
            kind_id: offer.kind_id.clone(), kind_contract_revision: offer.kind_contract_revision.clone(),
            execution_profile_id: offer.implementation.execution_profile_id.clone(), configuration: Vec::new(),
            host_id: host.host_id, boot_id: host.boot_id, offer_generation: host.offer_generation,
            capability_id: offer.capability_id.clone(), implementation_id: offer.implementation.implementation_id.clone(),
            artifact_id: offer.implementation.artifact_id.clone(), base: None,
            realization_characteristics: Vec::new(), limits: offer.limits.clone(),
            inputs: offer.inputs.clone(), outputs: offer.outputs.clone(), semantic_contract: offer.semantic_contract.clone(),
            terminal_transductions: Vec::new(), host_calls: offer.host_calls.clone(),
            resources: Vec::new(), authority: Vec::new(), pool_references: Vec::new(),
        }
    }
    #[test]
    fn reset_adapter_counter_in_a_replacement_boot_cannot_reuse_a_run_identity() {
        let mut placement = placement();
        let original = identity("run", &placement, 1);
        placement.boot_id = "boot/replacement".into();
        assert_ne!(identity("run", &placement, 1), original);
        assert_ne!(
            identity("run", &placement, 1),
            identity("run", &placement, 2)
        );
        assert!(identity("run", &placement, u64::MAX).len() <= 128);
    }
}
