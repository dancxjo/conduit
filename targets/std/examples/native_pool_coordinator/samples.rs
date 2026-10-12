//! Checked owner-clock sample import. Caller must own the producing pipe and
//! pin its Host advertisement and clock. This proves no coordinator-clock
//! freshness and creates neither a forecast nor a capacity/authority grant.
use conduit_core::*;
use conduit_planner::resource_cost::{
    CostCalibrationRevision, CostEvidenceSource, CostExecutionCondition, CostExecutionObservation,
    CostObservationWindow,
};
use conduit_std_host::hosted_local_model::{LocalModelRuntimeState, ModelResidencyState};
use serde::Deserialize;
use serde_json::json;
use std::io::{Read, Write};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Sample {
    schema: String,
    host_id: HostId,
    boot_id: BootId,
    offer_generation: OfferGeneration,
    capability_id: CapabilityId,
    implementation_id: ImplementationId,
    artifact_id: ArtifactId,
    resource_pool_id: Option<ResourcePoolId>,
    sign_id: SignId,
    calibration_identity: ArtifactId,
    calibration_revision: u64,
    observed: MonotonicInstant,
    expires: MonotonicInstant,
    validated_at: MonotonicInstant,
    sampling_duration: MonotonicDuration,
    runtime_state: LocalModelRuntimeState,
    execution_condition: String,
    resident_size: Option<ValuePayload>,
    vram_size: Option<ValuePayload>,
    attempt: u16,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    host: HostAdvertisement,
    owner_clock: MonotonicClockIdentity,
    sample: Sample,
    #[serde(default)]
    freshness: Option<super::freshness::Input>,
}
fn measurement(
    value: Option<&ValuePayload>,
    bytes: Option<u64>,
    work: &mut ExactNumericWork,
) -> Result<Option<RationalQuantity>, String> {
    match (value, bytes) {
        (None, None) => Ok(None),
        (Some(value), Some(bytes)) => {
            if value.value_kind.as_str() != RATIONAL_QUANTITY_ID {
                return Err("measurement semantic type changed".into());
            }
            let decoded = RationalQuantity::decode(&value.encoded, work)
                .map_err(|e| format!("measurement quantity: {e:?}"))?;
            if decoded.unit() != Unit::resolve("B").map_err(|e| format!("byte Unit: {e:?}"))?
                || decoded.role() != QuantityRole::Linear
                || decoded.coordinate() != ExactRational::from_i128(i128::from(bytes))
            {
                return Err("measurement dimension, role or exact coordinate changed".into());
            }
            Ok(Some(decoded))
        }
        _ => Err("measurement presence changed".into()),
    }
}
fn check(input: &Request) -> Result<serde_json::Value, String> {
    let host = &input.host;
    let sample = &input.sample;
    input
        .owner_clock
        .validate()
        .map_err(|e| format!("owner clock: {e:?}"))?;
    if sample.schema != "conduit.proof/model-cost-sample@1"
        || sample.host_id != host.host_id
        || sample.boot_id != host.boot_id
        || sample.offer_generation != host.offer_generation
        || input.owner_clock.host_id() != &host.host_id
        || input.owner_clock.boot_id() != &host.boot_id
        || input.owner_clock.scale() != TemporalScale::Microseconds
        || sample.observed.clock() != &input.owner_clock
    {
        return Err("sample owner, generation, schema or clock changed".into());
    }
    if !(1..=64).contains(&sample.attempt)
        || sample.sign_id
            != bind_sign(
                &host.host_id,
                &host.boot_id,
                None,
                u64::from(sample.attempt),
            )
            .sign_id
        || sample.calibration_identity.as_str() != "conduit.proof/model-runtime-sampling@1"
        || sample.calibration_revision != 1
    {
        return Err("sample producer or metric revision changed".into());
    }
    let offer = host
        .capabilities
        .iter()
        .find(|offer| offer.capability_id == sample.capability_id)
        .ok_or("sample capability absent from current owner")?;
    if offer.kind_id.as_str() != conduit_ai::LLM_GENERATE_KIND
        || offer.implementation.implementation_id != sample.implementation_id
        || offer.implementation.artifact_id != sample.artifact_id
    {
        return Err("sample initialized implementation changed".into());
    }
    let resource = host
        .resources
        .iter()
        .find(|resource| resource.class_id.as_str() == conduit_ai::LOCAL_MODEL_MEMORY_RESOURCE);
    if sample.resource_pool_id.as_ref() != resource.map(|resource| &resource.pool_id) {
        return Err("sample memory resource binding changed".into());
    }
    let window = CostObservationWindow::admit(sample.observed.clone(), sample.expires.clone())
        .map_err(|e| format!("sample window: {e:?}"))?;
    if sample
        .expires
        .elapsed_since(&sample.observed)
        .map_err(|e| format!("sample lifetime: {e:?}"))?
        .ticks()
        > 1_000_000
        || sample.sampling_duration.scale() != TemporalScale::Microseconds
        || sample.sampling_duration.ticks() > 500_000
        || sample
            .validated_at
            .elapsed_since(&sample.observed)
            .map_err(|e| format!("sample age: {e:?}"))?
            .ticks()
            < sample.sampling_duration.ticks()
    {
        return Err("sample acquisition or lifetime bound changed".into());
    }
    let calibration =
        CostCalibrationRevision::new(&sample.calibration_identity, sample.calibration_revision)
            .map_err(|e| format!("sample calibration: {e:?}"))?;
    let source =
        CostEvidenceSource::bind(host, offer, resource, &sample.sign_id, calibration, window)
            .map_err(|e| format!("sample source: {e:?}"))?;
    source
        .validate_at(host, calibration, &sample.validated_at)
        .map_err(|e| format!("sample owner validation: {e:?}"))?;
    let current_bound = input
        .freshness
        .as_ref()
        .map(|exchange| super::freshness::validate(exchange, source.window(), &sample.validated_at))
        .transpose()?;
    let expected = match sample.runtime_state.residency {
        ModelResidencyState::NotResident => CostExecutionCondition::Cold,
        ModelResidencyState::Unknown | ModelResidencyState::ResidentWithoutEpoch => {
            CostExecutionCondition::Unknown
        }
    };
    if sample.execution_condition != format!("{expected:?}") {
        return Err("sample invented an execution condition or residency epoch".into());
    }
    let execution = CostExecutionObservation::admit(&source, expected)
        .map_err(|e| format!("sample execution observation: {e:?}"))?;
    let mut work = ExactNumericWork::new(131_072);
    measurement(
        sample.resident_size.as_ref(),
        sample.runtime_state.resident_bytes,
        &mut work,
    )?;
    measurement(
        sample.vram_size.as_ref(),
        sample.runtime_state.vram_bytes,
        &mut work,
    )?;
    Ok(
        json!({"phase":"sample-checked","freshness_scope":if current_bound.is_some() { "coordinator_via_admitted_peer_exchange" } else { "reported_owner_validation_only" },
        "owner_current_interval":current_bound,
        "host_id":host.host_id,"boot_id":host.boot_id,"sign_id":source.sign(),
        "condition":format!("{:?}",execution.condition()),"numeric_work":work.spent(),
        "observed":source.window().observed(),"expires":source.window().expires()}),
    )
}
pub(super) fn run() -> Result<(), String> {
    const MAXIMUM: usize = 1024 * 1024;
    let mut input = std::io::stdin().lock();
    let mut prefix = [0; 4];
    input
        .read_exact(&mut prefix)
        .map_err(|_| "sample header unavailable")?;
    let length = u32::from_be_bytes(prefix) as usize;
    if length == 0 || length > MAXIMUM {
        return Err("sample document extent refused".into());
    }
    let mut bytes = vec![0; length];
    input
        .read_exact(&mut bytes)
        .map_err(|_| "sample document incomplete")?;
    let request: Request = serde_json::from_slice(&bytes).map_err(|_| "sample document invalid")?;
    let receipt = serde_json::to_vec(&check(&request)?).map_err(|_| "sample receipt invalid")?;
    let mut output = std::io::stdout().lock();
    output
        .write_all(&(receipt.len() as u32).to_be_bytes())
        .map_err(|_| "sample receipt header failed")?;
    output
        .write_all(&receipt)
        .map_err(|_| "sample receipt failed")?;
    output
        .flush()
        .map_err(|_| "sample receipt flush failed".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn payload(unit: &str, coordinate: i128, role: QuantityRole) -> ValuePayload {
        let value = RationalQuantity::new(
            ExactRational::from_i128(coordinate),
            Unit::resolve(unit).unwrap(),
            role,
        )
        .unwrap();
        let mut bytes = vec![0; RATIONAL_QUANTITY_MAX_ENCODED_BYTES];
        let length = value.encode(&mut bytes).unwrap();
        bytes.truncate(length);
        ValuePayload {
            value_kind: KindId::from(RATIONAL_QUANTITY_ID),
            encoded: bytes,
        }
    }
    #[test]
    fn canonical_time_quantity_cannot_masquerade_as_reported_memory() {
        let time = payload("us", 4096, QuantityRole::Linear);
        assert!(measurement(Some(&time), Some(4096), &mut ExactNumericWork::new(131_072)).is_err());
    }
    #[test]
    fn exact_reported_extent_and_presence_are_checked() {
        let bytes = payload("B", 4096, QuantityRole::Linear);
        assert!(measurement(
            Some(&bytes),
            Some(4096),
            &mut ExactNumericWork::new(131_072)
        )
        .is_ok());
        assert!(measurement(
            Some(&bytes),
            Some(4095),
            &mut ExactNumericWork::new(131_072)
        )
        .is_err());
        assert!(measurement(Some(&bytes), None, &mut ExactNumericWork::new(131_072)).is_err());
        assert!(measurement(None, Some(0), &mut ExactNumericWork::new(131_072)).is_err());
    }
    #[test]
    fn malformed_and_unfunded_numeric_import_refuse() {
        let mut bytes = payload("B", 4096, QuantityRole::Linear);
        assert!(measurement(Some(&bytes), Some(4096), &mut ExactNumericWork::new(0)).is_err());
        bytes.encoded.truncate(1);
        assert!(measurement(
            Some(&bytes),
            Some(4096),
            &mut ExactNumericWork::new(131_072)
        )
        .is_err());
    }
}
