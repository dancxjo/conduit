//! Portable request to manifest one exact frequency as a bounded tone.
use crate::{KindTerminalBehavior, StandardKindContract};
#[cfg(feature = "plot-catalog")]
use alloc::string::ToString;
use alloc::{vec, vec::Vec};
use conduit_core::{
    kind_id, port_id, CapabilityLimits, Kind, PortDescriptor, PortDirection, PortTemporal,
    QUANTITY_ENCODED_LEN, QUANTITY_INFO_ID,
};

pub const PITCH_TONE_KIND: &str = "sound/pitch-tone";
pub const PITCH_TONE_REVISION: &str = "conduit.sound/pitch-tone@1";

/// Loudness, envelope, device, and activation policy belong to the selected
/// Host realization rather than the authored plot.
pub fn pitch_tone_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(PITCH_TONE_KIND),
        plain_name: "Bounded pitch tone".into(),
        summary: "Manifest one exact frequency quantity as a short, safely bounded audible tone; retain unavailable or denied playback without failing unrelated work.".into(),
        inputs: vec![PortDescriptor {
            port_id: port_id("pitch"),
            value_kind: kind_id(QUANTITY_INFO_ID),
            direction: PortDirection::Input,
            temporal: PortTemporal::Value,
            abnormal_kind: None,
        }],
        outputs: Vec::new(),
        configuration: Default::default(),
        limits: CapabilityLimits {
            max_active_instances: 16,
            max_queue_items: 1,
            max_queue_bytes: QUANTITY_ENCODED_LEN as u32,
        },
        terminal_behavior: KindTerminalBehavior::CompletesWhenInputsClose,
        hosted_implementation_required: true,
        browser_manifestation_honest: true,
        pico_manifestation_honest: false,
        example: "tone: sound/pitch-tone".into(),
    }
}

pub fn pitch_tone_semantic_contract() -> Kind {
    pitch_tone_contract().into_semantic_contract(PITCH_TONE_REVISION)
}

#[cfg(feature = "plot-catalog")]
pub fn install_pitch_tone_catalog(
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    startup.insert(conduit_plot::KindSignature {
        kind: PITCH_TONE_KIND.into(),
        startup_parameters: vec![],
    })?;
    profile
        .insert_kind(pitch_tone_semantic_contract())
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{PortDirection, PortTemporal, QuantityUnit, QUANTITY_INFO_ID};

    #[test]
    fn pitch_tone_is_an_exact_bounded_frequency_sink_without_platform_facts() {
        let contract = pitch_tone_contract();
        assert_eq!(contract.kind_id.as_str(), PITCH_TONE_KIND);
        assert_eq!(contract.inputs.len(), 1);
        assert_eq!(contract.inputs[0].port_id.as_str(), "pitch");
        assert_eq!(contract.inputs[0].value_kind.as_str(), QUANTITY_INFO_ID);
        assert_eq!(contract.inputs[0].direction, PortDirection::Input);
        assert_eq!(contract.inputs[0].temporal, PortTemporal::Value);
        assert_eq!(contract.limits.max_queue_items, 1);
        assert_eq!(contract.limits.max_queue_bytes, QUANTITY_ENCODED_LEN as u32);
        assert!(contract.outputs.is_empty());
        assert!(contract.configuration.is_empty());
        for forbidden in ["browser", "Web Audio", "AudioContext", "oscillator", "gain"] {
            assert!(!contract.summary.contains(forbidden));
        }
        assert_eq!(
            QuantityUnit::Hertz.dimension(),
            conduit_core::QuantityDimension::Frequency
        );
    }
}
