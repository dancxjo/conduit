//! Portable musical-input source contract.

use crate::{
    sound::event_limits, sound::music_ports, StandardConfigurationField, StandardConfigurationRule,
    StandardKindContract, TerminalBehavior,
};
use alloc::{string::ToString, vec, vec::Vec};
use conduit_core::{
    kind_id, resource_requirement, ArtifactId, AuthorityContractId, AuthorityRequirement,
    CapabilityId, CapabilityOffer, ConfigurationValue, ExecutionProfileId, HostOperationContractId,
    HostOperationRequirement, ImplementationId, ImplementationOffer, KindContractRevision,
    PortDirection,
};

pub const MUSIC_INPUT_KIND: &str = "music/input";
pub const MUSIC_INPUT_REVISION: &str = "conduit.std/music-input@1";
pub const MUSIC_INPUT_MIDI_PROFILE: &str = "std/midi1-raw-input-monotonic-us@1";
pub const MUSIC_INPUT_MIDI_IMPLEMENTATION: &str = "std/kernel-music-input-midi1@1";
pub const MUSIC_INPUT_MIDI_ARTIFACT: &str = "conduit-std-host/music-input-midi1@1";
pub const MUSIC_INPUT_MIDI_OPERATION: &str = "conduit.host/midi1-input-next-observation@1";
pub const MIDI_INPUT_RESOURCE_CLASS: &str = "conduit.resource/midi-input@1";
pub const MIDI_INPUT_AUTHORITY_CONTRACT: &str = "conduit.authority/midi-input@1";
pub const MUSIC_INPUT_A4_REFERENCE_KEY: &str = "a4-reference-millihertz";
pub const MUSIC_INPUT_TRANSPOSE_KEY: &str = "transpose-semitones";

pub fn music_input_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(MUSIC_INPUT_KIND),
        plain_name: "Musical input".to_string(),
        summary: "Produce portable note and typed expressive-control events.".to_string(),
        inputs: Vec::new(),
        outputs: music_ports(PortDirection::Output),
        configuration: music_input_configuration(),
        limits: event_limits(),
        terminal_behavior: TerminalBehavior::HostInputEndsOrFailsSource,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: "input: music/input".to_string(),
    }
}

/// Exact MIDI 1.0 realization boundary for the portable source.
///
/// The host operation returns one fixed-size protocol-domain observation. The
/// installed implementation, not the platform adapter, maps it onto the exact
/// portable note or control output port while preserving stream order.
pub fn music_input_midi_offer() -> CapabilityOffer {
    let contract = music_input_contract();
    let operation = HostOperationRequirement {
        contract_id: HostOperationContractId::from(MUSIC_INPUT_MIDI_OPERATION),
        target_kind: Some(kind_id(conduit_midi::MIDI_INPUT_OBSERVATION_INFO_ID)),
        maximum_in_flight: 1,
        maximum_input_bytes: 0,
        maximum_output_bytes: conduit_midi::MIDI_INPUT_OBSERVATION_ENCODED_LEN as u32,
    };
    CapabilityOffer {
        startup_parameters: crate::startup_face(&contract.configuration),
        shorthand: None,
        capability_id: CapabilityId::from("music-input-midi1"),
        kind_id: contract.kind_id,
        kind_contract_revision: KindContractRevision::from(MUSIC_INPUT_REVISION),
        inputs: contract.inputs,
        outputs: contract.outputs,
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(MUSIC_INPUT_MIDI_PROFILE),
            implementation_id: ImplementationId::from(MUSIC_INPUT_MIDI_IMPLEMENTATION),
            artifact_id: ArtifactId::from(MUSIC_INPUT_MIDI_ARTIFACT),
        },
        host_operations: vec![operation.clone()],
        resource_requirements: vec![resource_requirement(MIDI_INPUT_RESOURCE_CLASS, 1)],
        authority_requirements: vec![AuthorityRequirement {
            contract_id: AuthorityContractId::from(MIDI_INPUT_AUTHORITY_CONTRACT),
            host_operation_contract_id: operation.contract_id,
            subject_kind: kind_id(conduit_midi::MIDI_INPUT_OBSERVATION_INFO_ID),
        }],
        limits: contract.limits,
    }
}

pub fn music_input_configuration() -> Vec<StandardConfigurationField> {
    vec![
        StandardConfigurationField {
            key: MUSIC_INPUT_A4_REFERENCE_KEY.to_string(),
            default_value: ConfigurationValue::U64(440_000),
            rule: StandardConfigurationRule::U64Range {
                minimum: conduit_audio::MINIMUM_A4_MILLIHERTZ,
                maximum: conduit_audio::MAXIMUM_A4_MILLIHERTZ,
            },
        },
        StandardConfigurationField {
            key: MUSIC_INPUT_TRANSPOSE_KEY.to_string(),
            default_value: ConfigurationValue::I64(0),
            rule: StandardConfigurationRule::I64Range {
                minimum: -48,
                maximum: 48,
            },
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sound_contract_revision;

    #[test]
    fn contract_is_a_host_neutral_typed_source() {
        let contract = music_input_contract();
        assert!(contract.inputs.is_empty());
        assert_eq!(contract.outputs, music_ports(PortDirection::Output));
        assert_eq!(
            contract.terminal_behavior,
            TerminalBehavior::HostInputEndsOrFailsSource
        );
        assert_eq!(contract.configuration, music_input_configuration());
        assert_eq!(
            sound_contract_revision(MUSIC_INPUT_KIND).unwrap().as_str(),
            MUSIC_INPUT_REVISION
        );
    }

    #[test]
    fn midi_offer_keeps_protocol_observation_below_portable_outputs() {
        let offer = music_input_midi_offer();
        assert_eq!(offer.kind_id.as_str(), MUSIC_INPUT_KIND);
        assert_eq!(offer.inputs, Vec::new());
        assert_eq!(offer.outputs, music_input_contract().outputs);
        assert_eq!(offer.host_operations.len(), 1);
        let operation = &offer.host_operations[0];
        assert_eq!(operation.maximum_in_flight, 1);
        assert_eq!(operation.maximum_input_bytes, 0);
        assert_eq!(
            operation.maximum_output_bytes,
            conduit_midi::MIDI_INPUT_OBSERVATION_ENCODED_LEN as u32
        );
        assert_eq!(
            operation.target_kind.as_ref().unwrap().as_str(),
            conduit_midi::MIDI_INPUT_OBSERVATION_INFO_ID
        );
        assert_eq!(offer.resource_requirements.len(), 1);
        assert_eq!(offer.authority_requirements.len(), 1);
        assert_eq!(offer.startup_parameters.len(), 2);
        assert_eq!(
            offer.authority_requirements[0].subject_kind,
            operation.target_kind.clone().unwrap()
        );
    }
}
