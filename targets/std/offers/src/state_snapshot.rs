//! Hosted std realization of one-shot current-value snapshots.

use conduit_core::{
    ArtifactId, Back, BackOfferBuilder, CapabilityId, CapabilityOffer, CheckedValueContract,
    ExecutionProfileId, ImplementationId,
};

pub const STATE_SNAPSHOT_EXECUTION_PROFILE: &str = "conduit.std/state-snapshot-prepared@1";
pub const STATE_SNAPSHOT_IMPLEMENTATION: &str = "std/kernel-state-snapshot@1";
pub const STATE_SNAPSHOT_ARTIFACT: &str = "conduit-std-host/state-snapshot@1";
pub const STATE_SNAPSHOT_MAXIMUM_VALUE_BYTES: u32 = 4 * 1024;

pub fn state_snapshot_offer(value: &CheckedValueContract) -> Result<CapabilityOffer, &'static str> {
    if value.maximum_bytes > STATE_SNAPSHOT_MAXIMUM_VALUE_BYTES {
        return Err("std state/snapshot specialization exceeds its prepared value bound");
    }
    let contract = conduit_semantic_catalog::state_snapshot_semantic_contract(value)?;
    Ok(BackOfferBuilder::new(
        contract,
        Back {
            capability_id: CapabilityId::from(format!(
                "state-snapshot-{}-{}",
                value.value_kind.as_str(),
                value.maximum_bytes
            )),
            execution_profile_id: ExecutionProfileId::from(STATE_SNAPSHOT_EXECUTION_PROFILE),
            implementation_id: ImplementationId::from(STATE_SNAPSHOT_IMPLEMENTATION),
            artifact_id: ArtifactId::from(STATE_SNAPSHOT_ARTIFACT),
            host_calls: Vec::new(),
            resource_requirements: Vec::new(),
            authority_requirements: Vec::new(),
        },
    )
    .build())
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{kind_id, TERMINAL_INFO_ID};

    #[test]
    fn offer_preserves_the_exact_4096_byte_text_fore_without_host_effects() {
        let text = CheckedValueContract::new(kind_id("value/text"), 4_096, vec![]).unwrap();
        let offer = state_snapshot_offer(&text).unwrap();
        assert!(offer.host_calls.is_empty());
        assert!(offer.resource_requirements.is_empty());
        assert_eq!(offer.inputs[0].value_kind, text.value_kind);
        assert_eq!(offer.outputs[0].value_kind, text.value_kind);
        assert_eq!(
            offer.outputs[0].abnormal_kind.as_ref().unwrap().as_str(),
            TERMINAL_INFO_ID
        );
    }
}
