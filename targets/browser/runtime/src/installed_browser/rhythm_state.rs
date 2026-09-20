//! Finite browser source for an explicitly configured portable rhythm state.

use conduit_core::{
    ArtifactId, CapabilityId, CapabilityOffer, CapabilityOfferBuilder, CapabilityRealization,
    ConfigurationValue, ExecutionProfileId, ImplementationId,
};
use conduit_kernel::ValueStorage;

const PROFILE: &str = "browser/rhythm-state-source@1";
const IMPLEMENTATION: &str = "browser/kernel-rhythm-state-source@1";
const ARTIFACT: &str = "conduit-browser-runtime/rhythm-state-source@1";

fn offer() -> CapabilityOffer {
    CapabilityOfferBuilder::new(
        conduit_time::rhythm_state_source_semantic_contract(),
        CapabilityRealization {
            capability_id: CapabilityId::from("rhythm-state-source"),
            execution_profile_id: ExecutionProfileId::from(PROFILE),
            implementation_id: ImplementationId::from(IMPLEMENTATION),
            artifact_id: ArtifactId::from(ARTIFACT),
            host_operations: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build()
}

fn prepare(
    placement: &conduit_core::PlannedGear,
    values: &mut conduit_kernel::HostedValueStore,
) -> Result<super::BrowserOperation, String> {
    super::factory::validate_placement(placement, &offer())?;
    let get = |key: &str| {
        placement
            .configuration
            .iter()
            .find_map(|entry| match entry.value {
                ConfigurationValue::U64(value) if entry.key == key => Some(value),
                _ => None,
            })
            .ok_or_else(|| format!("rhythm state lacks {key}"))
    };
    let sequence = u32::try_from(get("sequence")?).map_err(|_| "rhythm sequence exceeds u32")?;
    let next_pulse_at_ms =
        u32::try_from(get("next-pulse-at-ms")?).map_err(|_| "next pulse instant exceeds u32")?;
    let period_ms = u16::try_from(get("period-ms")?).map_err(|_| "period exceeds u16")?;
    let expected_peer_sequence = u32::try_from(get("expected-peer-sequence")?)
        .map_err(|_| "expected peer sequence exceeds u32")?;
    let state = conduit_time::RhythmState {
        sequence,
        next_pulse_at_ms,
        period_ms,
        expected_peer_sequence,
    };
    conduit_time::decode_rhythm_state(&conduit_time::encode_rhythm_state(state))
        .map_err(|error| format!("invalid rhythm state: {error:?}"))?;
    let value = values
        .store(&conduit_time::encode_rhythm_state(state))
        .map_err(|error| format!("admit rhythm state: {error:?}"))?;
    Ok(super::BrowserOperation::source(value))
}

pub(super) static INSTALLATION: super::factory::BrowserInstallation =
    super::factory::BrowserInstallation {
        implementation_id: IMPLEMENTATION,
        offer,
        prepare,
        perform: None,
    };

#[cfg(test)]
mod tests {
    #[test]
    fn browser_rhythm_source_preserves_the_portable_contract() {
        let offer = super::offer();
        let semantic = conduit_time::rhythm_state_source_semantic_contract();
        assert_eq!(offer.startup_parameters, semantic.startup_parameters);
        assert_eq!(offer.kind_id, semantic.kind_id);
        assert_eq!(
            offer.kind_contract_revision,
            semantic.kind_contract_revision
        );
        assert_eq!(offer.inputs, semantic.inputs);
        assert_eq!(offer.outputs, semantic.outputs);
        assert_eq!(offer.limits, semantic.limits);
    }
}
