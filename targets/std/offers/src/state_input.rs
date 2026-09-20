//! Exact state and portable-input realizations owned by the hosted std Host.

use conduit_core::{
    kind_id, ArtifactId, CapabilityId, CapabilityOffer, CapabilityOfferBuilder,
    CapabilityRealization, ExecutionProfileId, HostOperationContractId, HostOperationRequirement,
    ImplementationId, SemanticCapabilityContract,
};
use conduit_human::{CHORD_ENCODED_LEN, KEY_EVENT_ENCODED_LEN};

pub const STATE_COUNT_EXECUTION_PROFILE: &str = "conduit.std/state-count-kernel-hosted@1";
pub const STATE_COUNT_IMPLEMENTATION: &str = "std/kernel-state-count@1";
pub const STATE_COUNT_ARTIFACT: &str = "conduit-std-host/state-count@1";
pub const STATE_COUNT_CAPABILITY: &str = "state-count-v1";
pub const STATE_TOGGLE_EXECUTION_PROFILE: &str = "conduit.std/state-toggle-kernel-hosted@1";
pub const STATE_TOGGLE_IMPLEMENTATION: &str = "std/kernel-state-toggle@1";
pub const STATE_TOGGLE_ARTIFACT: &str = "conduit-std-host/state-toggle@1";
pub const STATE_TOGGLE_CAPABILITY: &str = "state-toggle-v1";

pub const KEY_EVENT_TEE_PROFILE: &str = "conduit.input/key-tee-kernel@1";
pub const KEY_EVENT_TEE_IMPLEMENTATION: &str = "std/kernel-key-event-tee@1";
pub const KEY_EVENT_TEE_ARTIFACT: &str = "conduit-std-host/key-event-tee@1";
pub const KEY_EVENT_TEE_CAPABILITY: &str = "key-event-tee-v1";
pub const KEYMAP_PROFILE: &str = "conduit.input/keymap-kernel-hosted@1";
pub const KEYMAP_IMPLEMENTATION: &str = "std/kernel-keymap@1";
pub const KEYMAP_ARTIFACT: &str = "conduit-std-host/keymap@1";
pub const KEYMAP_CAPABILITY: &str = "input-keymap-v1";
pub const KEYMAP_HOST_OPERATION: &str = "conduit.host/input-keymap@1";
pub const KEYMAP_HOST_TARGET: &str = "input/keymap-text-fragment";
pub const CHORDS_PROFILE: &str = "conduit.input/chords-kernel-hosted@1";
pub const CHORDS_IMPLEMENTATION: &str = "std/kernel-chords@1";
pub const CHORDS_ARTIFACT: &str = "conduit-std-host/chords@1";
pub const CHORDS_CAPABILITY: &str = "input-chords-v1";
pub const CHORDS_HOST_OPERATION: &str = "conduit.host/input-chords@1";
pub const CHORDS_HOST_TARGET: &str = "input/chord-fragment";

pub fn state_count_offer() -> CapabilityOffer {
    offer(
        conduit_semantic_catalog::state_count_semantic_contract(),
        STATE_COUNT_CAPABILITY,
        STATE_COUNT_EXECUTION_PROFILE,
        STATE_COUNT_IMPLEMENTATION,
        STATE_COUNT_ARTIFACT,
        None,
    )
}

pub fn state_toggle_offer() -> CapabilityOffer {
    offer(
        conduit_semantic_catalog::state_toggle_semantic_contract(),
        STATE_TOGGLE_CAPABILITY,
        STATE_TOGGLE_EXECUTION_PROFILE,
        STATE_TOGGLE_IMPLEMENTATION,
        STATE_TOGGLE_ARTIFACT,
        None,
    )
}

pub fn key_event_tee_offer() -> CapabilityOffer {
    offer(
        conduit_semantic_catalog::key_event_tee_semantic_contract(),
        KEY_EVENT_TEE_CAPABILITY,
        KEY_EVENT_TEE_PROFILE,
        KEY_EVENT_TEE_IMPLEMENTATION,
        KEY_EVENT_TEE_ARTIFACT,
        None,
    )
}

pub fn keymap_offer() -> CapabilityOffer {
    offer(
        conduit_semantic_catalog::keymap_semantic_contract(),
        KEYMAP_CAPABILITY,
        KEYMAP_PROFILE,
        KEYMAP_IMPLEMENTATION,
        KEYMAP_ARTIFACT,
        Some((KEYMAP_HOST_OPERATION, KEYMAP_HOST_TARGET, 4)),
    )
}

pub fn chords_offer() -> CapabilityOffer {
    offer(
        conduit_semantic_catalog::chords_semantic_contract(),
        CHORDS_CAPABILITY,
        CHORDS_PROFILE,
        CHORDS_IMPLEMENTATION,
        CHORDS_ARTIFACT,
        Some((
            CHORDS_HOST_OPERATION,
            CHORDS_HOST_TARGET,
            CHORD_ENCODED_LEN as u32,
        )),
    )
}

#[allow(clippy::too_many_arguments)]
fn offer(
    contract: SemanticCapabilityContract,
    capability: &str,
    profile: &str,
    implementation: &str,
    artifact: &str,
    operation: Option<(&str, &str, u32)>,
) -> CapabilityOffer {
    let operations = operation
        .map(|(contract, target, output)| HostOperationRequirement {
            contract_id: HostOperationContractId::from(contract),
            target_kind: Some(kind_id(target)),
            maximum_in_flight: 1,
            maximum_input_bytes: KEY_EVENT_ENCODED_LEN as u32,
            maximum_output_bytes: output,
        })
        .into_iter()
        .collect();
    CapabilityOfferBuilder::new(
        contract,
        CapabilityRealization {
            capability_id: CapabilityId::from(capability),
            execution_profile_id: ExecutionProfileId::from(profile),
            implementation_id: ImplementationId::from(implementation),
            artifact_id: ArtifactId::from(artifact),
            host_operations: operations,
            resource_requirements: Vec::new(),
            authority_requirements: Vec::new(),
        },
    )
    .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_offer_preserves_its_portable_contract() {
        for (offer, contract) in [
            (
                state_count_offer(),
                conduit_semantic_catalog::state_count_contract(),
            ),
            (
                state_toggle_offer(),
                conduit_semantic_catalog::state_toggle_contract(),
            ),
            (
                key_event_tee_offer(),
                conduit_semantic_catalog::key_event_tee_contract(),
            ),
            (keymap_offer(), conduit_semantic_catalog::keymap_contract()),
            (chords_offer(), conduit_semantic_catalog::chords_contract()),
        ] {
            assert_eq!(offer.kind_id, contract.kind_id);
            assert_eq!(offer.inputs, contract.inputs);
            assert_eq!(offer.outputs, contract.outputs);
            assert_eq!(offer.limits, contract.limits);
        }
    }
}
