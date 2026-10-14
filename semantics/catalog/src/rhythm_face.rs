//! Portable face of bounded rhythm-state updates.

use alloc::{string::ToString, vec, vec::Vec};
use conduit_core::{kind_id, port_id, CapabilityLimits, Kind, PortDescriptor, PortDirection};

use super::{KindTerminalBehavior, StandardKindContract};

pub const RHYTHM_FACE_KIND: &str = "face/rhythm";
pub const RHYTHM_FACE_CONTRACT_REVISION: &str = "conduit.face/rhythm-state@2";

pub fn rhythm_face_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(RHYTHM_FACE_KIND),
        plain_name: "Rhythm face".to_string(),
        summary: "Manifest each exact phase-following state while its play remains alive."
            .to_string(),
        inputs: vec![PortDescriptor {
            port_id: port_id("state"),
            value_kind: kind_id(conduit_time::RHYTHM_STATE_VALUE_KIND),
            direction: PortDirection::Input,
            temporal: conduit_core::PortTemporal::Flow { closes: false },
            abnormal_kind: None,
        }],
        outputs: Vec::new(),
        configuration: crate::data_label_configuration(),
        limits: CapabilityLimits {
            max_active_instances: 16,
            max_queue_items: 1,
            max_queue_bytes: conduit_time::RHYTHM_STATE_ENCODED_LEN as u32,
        },
        terminal_behavior: KindTerminalBehavior::CompletesWhenInputsClose,
        hosted_implementation_required: true,
        browser_manifestation_honest: true,
        pico_manifestation_honest: false,
        example: "show: face/rhythm(\"Rhythm state\")".to_string(),
    }
}

pub fn rhythm_face_semantic_contract() -> Kind {
    rhythm_face_contract().into_semantic_contract(RHYTHM_FACE_CONTRACT_REVISION)
}

#[cfg(feature = "plot-catalog")]
pub fn install_rhythm_face_catalog(
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    use conduit_plot::KindSignature;
    startup.insert(KindSignature {
        kind: RHYTHM_FACE_KIND.to_string(),
        startup_parameters: crate::data_label_startup_signature(),
    })?;
    profile
        .insert_kind(rhythm_face_semantic_contract())
        .map_err(|error| error.to_string())
}
