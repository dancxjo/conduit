//! Portable pulse-observation face front.

use alloc::string::ToString;
use alloc::{vec, vec::Vec};
#[cfg(feature = "plot-catalog")]
use conduit_core::KindIdentity;
use conduit_core::{kind_id, port_id, CapabilityLimits, Kind, PortDescriptor, PortDirection};

use super::{KindTerminalBehavior, StandardKindContract};

pub const PULSE_FACE_KIND: &str = "face/pulse";
pub const PULSE_FACE_CONTRACT_REVISION: &str = "conduit.face/pulse-observation@2";
pub const PULSE_TONE_FACE_KIND: &str = "sound/pulse-tone";
pub const PULSE_TONE_FACE_CONTRACT_REVISION: &str = "conduit.sound/pulse-tone@1";

pub fn pulse_face_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(PULSE_FACE_KIND),
        plain_name: "Pulse face".to_string(),
        summary: "Manifest each exact pulse observation while its play remains alive.".to_string(),
        inputs: vec![PortDescriptor {
            port_id: port_id("pulse"),
            value_kind: kind_id(conduit_time::PULSE_OBSERVATION_VALUE_KIND),
            direction: PortDirection::Input,
            temporal: conduit_core::PortTemporal::Flow { closes: false },
            abnormal_kind: None,
        }],
        outputs: Vec::new(),
        configuration: crate::data_label_configuration(),
        limits: CapabilityLimits {
            max_active_instances: 16,
            max_queue_items: 1,
            max_queue_bytes: conduit_time::PULSE_OBSERVATION_ENCODED_LEN as u32,
        },
        terminal_behavior: KindTerminalBehavior::CompletesWhenInputsClose,
        hosted_implementation_required: true,
        browser_manifestation_honest: true,
        pico_manifestation_honest: false,
        example: "light: face/pulse(\"Pulse observations\")".to_string(),
    }
}

pub fn pulse_face_semantic_contract() -> Kind {
    pulse_face_contract().into_semantic_contract(PULSE_FACE_CONTRACT_REVISION)
}

pub fn pulse_tone_face_semantic_contract() -> Kind {
    pulse_tone_face_contract().into_semantic_contract(PULSE_TONE_FACE_CONTRACT_REVISION)
}

pub fn pulse_tone_face_contract() -> StandardKindContract {
    let mut contract = pulse_face_contract();
    contract.configuration.clear();
    contract.kind_id = kind_id(PULSE_TONE_FACE_KIND);
    contract.plain_name = "Pulse tone".to_string();
    contract.summary = "Manifest exact pulse observations as admitted tone events.".to_string();
    contract.browser_manifestation_honest = false;
    contract.example = "tone: sound/pulse-tone".to_string();
    contract
}

#[cfg(feature = "plot-catalog")]
pub fn pulse_face_kind_projection() -> conduit_plot::KindProjection {
    use conduit_plot::KindProjection;
    let contract = pulse_face_contract();
    KindProjection {
        kind_id: contract.kind_id,
        kind_contract_revision: KindIdentity::from(PULSE_FACE_CONTRACT_REVISION),
        inputs: contract.inputs,
        outputs: contract.outputs,
        configuration: contract.configuration,
    }
}

#[cfg(feature = "plot-catalog")]
pub fn pulse_tone_face_kind_projection() -> conduit_plot::KindProjection {
    let mut definition = pulse_face_kind_projection();
    definition.configuration.clear();
    definition.kind_id = kind_id(PULSE_TONE_FACE_KIND);
    definition.kind_contract_revision = KindIdentity::from(PULSE_TONE_FACE_CONTRACT_REVISION);
    definition
}

#[cfg(feature = "plot-catalog")]
pub fn install_pulse_face_catalog(
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    use conduit_plot::KindSignature;
    for (kind, definition) in [
        (PULSE_FACE_KIND, pulse_face_semantic_contract()),
        (PULSE_TONE_FACE_KIND, pulse_tone_face_semantic_contract()),
    ] {
        startup.insert(KindSignature {
            kind: kind.to_string(),
            startup_parameters: if kind == PULSE_FACE_KIND {
                crate::data_label_startup_signature()
            } else {
                Vec::new()
            },
        })?;
        profile
            .insert_kind(definition)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn light_and_tone_are_distinct_finite_face_contracts() {
        let light = pulse_face_contract();
        let tone = pulse_tone_face_contract();
        assert_ne!(light.kind_id, tone.kind_id);
        assert_eq!(light.inputs, tone.inputs);
        assert_eq!(light.limits, tone.limits);
        assert!(light.browser_manifestation_honest);
        assert!(!tone.browser_manifestation_honest);
        assert_eq!(tone.kind_id.as_str(), PULSE_TONE_FACE_KIND);
    }
}
