//! Hosted std realization of triggered current-value sampling.

use conduit_core::{
    ArtifactId, Back, BackOfferBuilder, CapabilityId, CapabilityOffer, CheckedValueContract,
    ExecutionProfileId, ImplementationId,
};

pub const CURRENT_SAMPLE_EXECUTION_PROFILE: &str = "conduit.std/current-sample-prepared@1";
pub const CURRENT_SAMPLE_IMPLEMENTATION: &str = "std/kernel-current-sample@1";
pub const CURRENT_SAMPLE_ARTIFACT: &str = "conduit-std-host/current-sample@1";
pub const CURRENT_SAMPLE_MAXIMUM_VALUE_BYTES: u32 = 4 * 1024;
pub const CURRENT_SAMPLE_MAXIMUM_TRIGGER_BYTES: u32 = 256;

pub fn current_sample_offer(
    value: &CheckedValueContract,
    trigger: &CheckedValueContract,
) -> Result<CapabilityOffer, &'static str> {
    if value.maximum_bytes > CURRENT_SAMPLE_MAXIMUM_VALUE_BYTES
        || trigger.maximum_bytes > CURRENT_SAMPLE_MAXIMUM_TRIGGER_BYTES
    {
        return Err("std current/sample specialization exceeds its prepared bounds");
    }
    let contract = conduit_semantic_catalog::current_sample_semantic_contract(value, trigger)?;
    Ok(BackOfferBuilder::new(
        contract,
        Back {
            capability_id: CapabilityId::from(format!(
                "current-sample-{}-{}-{}-{}",
                value.value_kind.as_str(),
                value.maximum_bytes,
                trigger.value_kind.as_str(),
                trigger.maximum_bytes
            )),
            execution_profile_id: ExecutionProfileId::from(CURRENT_SAMPLE_EXECUTION_PROFILE),
            implementation_id: ImplementationId::from(CURRENT_SAMPLE_IMPLEMENTATION),
            artifact_id: ArtifactId::from(CURRENT_SAMPLE_ARTIFACT),
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
    fn offer_preserves_exact_value_and_trigger_fores_without_host_effects() {
        let text = CheckedValueContract::new(kind_id("value/text"), 4_096, vec![]).unwrap();
        let request = CheckedValueContract::new(kind_id("data/save-request@1"), 0, vec![]).unwrap();
        let offer = current_sample_offer(&text, &request).unwrap();
        assert!(offer.host_calls.is_empty());
        assert!(offer.resource_requirements.is_empty());
        assert_eq!(offer.inputs[0].value_kind, text.value_kind);
        assert_eq!(offer.inputs[1].value_kind, request.value_kind);
        assert_eq!(offer.outputs[0].value_kind, text.value_kind);
        assert_eq!(
            offer.outputs[0].abnormal_kind.as_ref().unwrap().as_str(),
            TERMINAL_INFO_ID
        );
    }
}
