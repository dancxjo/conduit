//! Portable musical-input source contract.

use crate::{sound::event_limits, sound::music_ports, StandardKindContract, TerminalBehavior};
use alloc::{string::ToString, vec::Vec};
use conduit_core::{kind_id, PortDirection};

pub const MUSIC_INPUT_KIND: &str = "music/input";
pub const MUSIC_INPUT_REVISION: &str = "conduit.std/music-input@1";

pub fn music_input_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(MUSIC_INPUT_KIND),
        plain_name: "Musical input".to_string(),
        summary: "Produce portable note and typed expressive-control events.".to_string(),
        inputs: Vec::new(),
        outputs: music_ports(PortDirection::Output),
        configuration: Vec::new(),
        limits: event_limits(),
        terminal_behavior: TerminalBehavior::HostInputEndsOrFailsSource,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: "input: music/input".to_string(),
    }
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
        assert_eq!(
            sound_contract_revision(MUSIC_INPUT_KIND).unwrap().as_str(),
            MUSIC_INPUT_REVISION
        );
    }
}
