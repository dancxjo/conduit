//! Exact std realization of finite sampling for bounded structured payloads.
use conduit_core::*;

pub const CURRENT_SAMPLE_FINITE_IMPLEMENTATION: &str = "std/kernel-current-sample-finite@1";
pub const CURRENT_SAMPLE_FINITE_PROFILE: &str = "conduit.std/current-sample-finite-prepared@1";
pub const CURRENT_SAMPLE_FINITE_ARTIFACT: &str = "conduit-composite/current-sample@1";
pub const CURRENT_SAMPLE_FINITE_MAXIMUM_BYTES: u32 = 128 * 1024;
pub const CURRENT_SAMPLE_FINITE_MAXIMUM_TRIGGER_BYTES: u32 = 256;

pub fn current_sample_finite_offer(
    value: &CheckedValueContract,
    trigger: &CheckedValueContract,
) -> Result<CapabilityOffer, &'static str> {
    if value.maximum_bytes > CURRENT_SAMPLE_FINITE_MAXIMUM_BYTES
        || trigger.maximum_bytes > CURRENT_SAMPLE_FINITE_MAXIMUM_TRIGGER_BYTES
    {
        return Err("std finite sampler exceeds its prepared payload or trigger bound");
    }
    let kind = conduit_semantic_catalog::current_sample_finite_semantic_contract(value, trigger)?;
    Ok(BackOfferBuilder::new(
        kind,
        Back {
            capability_id: format!(
                "current-sample-finite/{}/{}/{}/{}@1",
                value.value_kind.as_str(),
                value.maximum_bytes,
                trigger.value_kind.as_str(),
                trigger.maximum_bytes
            )
            .into(),
            execution_profile_id: CURRENT_SAMPLE_FINITE_PROFILE.into(),
            implementation_id: CURRENT_SAMPLE_FINITE_IMPLEMENTATION.into(),
            artifact_id: CURRENT_SAMPLE_FINITE_ARTIFACT.into(),
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
    #[test]
    fn finite_sampler_offer_refuses_envelopes_beyond_its_prepared_profile() {
        let value = CheckedValueContract::new(
            kind_id("value/bytes"),
            CURRENT_SAMPLE_FINITE_MAXIMUM_BYTES + 1,
            vec![],
        )
        .unwrap();
        let tick = CheckedValueContract::new(kind_id(EMPTY_INFO_ID), 0, vec![]).unwrap();
        assert!(current_sample_finite_offer(&value, &tick).is_err());
        let value = CheckedValueContract::new(kind_id("value/bytes"), 16, vec![]).unwrap();
        let too_large = CheckedValueContract::new(
            kind_id("value/bytes"),
            CURRENT_SAMPLE_FINITE_MAXIMUM_TRIGGER_BYTES + 1,
            vec![],
        )
        .unwrap();
        assert!(current_sample_finite_offer(&value, &too_large).is_err());
    }
}
