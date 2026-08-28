//! Portable musical-input source contract.

use crate::{
    sound::event_limits, sound::music_ports, StandardConfigurationField, StandardConfigurationRule,
    StandardKindContract, TerminalBehavior,
};
use alloc::{string::ToString, vec, vec::Vec};
use conduit_core::{kind_id, ConfigurationValue, PortDirection};

pub const MUSIC_INPUT_KIND: &str = "music/input";
pub const MUSIC_INPUT_REVISION: &str = "conduit.std/music-input@1";
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
}
