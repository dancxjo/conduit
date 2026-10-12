//! Actual owner-side sampling outside Play. Canonical quantities retain their
//! exact Unit/role capsules; no warm epoch or zero measurement is invented.
use conduit_core::{
    ArtifactId, RationalQuantity, RATIONAL_QUANTITY_ID, RATIONAL_QUANTITY_MAX_ENCODED_BYTES,
};
use conduit_planner::resource_cost::CostCalibrationRevision;
use conduit_std_host::{
    hosted_local_model::{ModelCostSampler, ModelCostSamplingPolicy},
    StdHost, ThreadTimer, TimerAdapter,
};
use serde_json::{json, Value};

pub(super) fn prepare(host: &StdHost) -> Result<ModelCostSampler, String> {
    let offer = host
        .advertisement()
        .capabilities
        .iter()
        .find(|offer| offer.kind_id.as_str() == conduit_ai::LLM_GENERATE_KIND)
        .ok_or("initialized generation capability absent")?;
    ModelCostSampler::admit(
        host.advertisement(),
        &offer.capability_id,
        ModelCostSamplingPolicy {
            validity_microseconds: 1_000_000,
            maximum_sampling_microseconds: 500_000,
            minimum_interval_microseconds: 1_000,
            maximum_attempts: 64,
        },
    )
}
fn quantity(value: Option<RationalQuantity>) -> Result<Option<Value>, String> {
    value
        .map(|value| {
            let mut encoded = [0; RATIONAL_QUANTITY_MAX_ENCODED_BYTES];
            let length = value
                .encode(&mut encoded)
                .map_err(|e| format!("sample quantity encoding: {e:?}"))?;
            Ok(json!({"value_kind":RATIONAL_QUANTITY_ID,"encoded":&encoded[..length]}))
        })
        .transpose()
}
pub(super) fn observe(host: &StdHost, sampler: &mut ModelCostSampler) -> Result<Value, String> {
    let owner = host.advertisement();
    let sign = conduit_core::bind_sign(
        &owner.host_id,
        &owner.boot_id,
        None,
        u64::from(sampler.attempts()) + 1,
    )
    .sign_id;
    let metric = ArtifactId::from("conduit.proof/model-runtime-sampling@1");
    let revision =
        CostCalibrationRevision::new(&metric, 1).map_err(|e| format!("sample revision: {e:?}"))?;
    let mut timer = ThreadTimer;
    let sample = host.observe_local_model_execution(sampler, &sign, revision, &mut timer)?;
    let now = timer
        .monotonic_observation(&owner.host_id, &owner.boot_id)
        .ok_or("sample publication clock unavailable")?;
    sample
        .source()
        .validate_at(owner, revision, &now)
        .map_err(|e| format!("sample expired before publication: {e:?}"))?;
    let source = sample.source();
    let condition = sample
        .execution_observation()
        .map_err(|e| format!("sample condition: {e:?}"))?;
    Ok(json!({
        "schema":"conduit.proof/model-cost-sample@1",
        "host_id":owner.host_id,"boot_id":owner.boot_id,"offer_generation":owner.offer_generation,
        "capability_id":source.offer().capability_id,
        "implementation_id":source.offer().implementation.implementation_id,
        "artifact_id":source.offer().implementation.artifact_id,
        "resource_pool_id":source.resource().map(|resource| &resource.pool_id),
        "sign_id":source.sign(),"calibration_identity":revision.identity(),"calibration_revision":revision.revision(),
        "observed":source.window().observed(),"expires":source.window().expires(),"validated_at":now,
        "sampling_duration":sample.sampling_duration(),"runtime_state":sample.state(),
        "execution_condition":format!("{:?}",condition.condition()),
        "resident_size":quantity(sample.resident_size())?,"vram_size":quantity(sample.vram_size())?,
        "attempt":sampler.attempts(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{ExactNumericWork, ExactRational, QuantityRole, Unit};

    #[test]
    fn maximum_reported_byte_extent_retains_exact_unit_and_role_in_transport() {
        let original = RationalQuantity::new(
            ExactRational::from_i128(i128::from(u64::MAX)),
            Unit::resolve("B").unwrap(),
            QuantityRole::Linear,
        )
        .unwrap();
        let encoded = quantity(Some(original)).unwrap().unwrap();
        assert_eq!(encoded["value_kind"], RATIONAL_QUANTITY_ID);
        let bytes: Vec<u8> = serde_json::from_value(encoded["encoded"].clone()).unwrap();
        let decoded = RationalQuantity::decode(&bytes, &mut ExactNumericWork::new(65_536)).unwrap();
        assert_eq!(decoded, original);
        assert_eq!(decoded.unit(), Unit::resolve("B").unwrap());
        assert_eq!(decoded.role(), QuantityRole::Linear);
    }

    #[test]
    fn missing_memory_measurement_remains_absent() {
        assert!(quantity(None).unwrap().is_none());
    }
}
