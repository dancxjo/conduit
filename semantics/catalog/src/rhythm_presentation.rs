//! Portable presentation of bounded rhythm-state updates.

use alloc::{string::ToString, vec, vec::Vec};
use conduit_core::{kind_id, port_id, CapabilityLimits, PortDescriptor, PortDirection};

use super::{KindTerminalBehavior, StandardKindContract};

pub const RHYTHM_PRESENTATION_KIND: &str = "presentation/rhythm";
pub const RHYTHM_PRESENTATION_CONTRACT_REVISION: &str = "conduit.presentation/rhythm-state@2";

pub fn rhythm_presentation_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(RHYTHM_PRESENTATION_KIND),
        plain_name: "Rhythm presentation".to_string(),
        summary: "Manifest each exact phase-following state while its play remains alive."
            .to_string(),
        inputs: vec![PortDescriptor {
            port_id: port_id("state"),
            value_kind: kind_id(conduit_time::RHYTHM_STATE_VALUE_KIND),
            direction: PortDirection::Input,
            temporal: conduit_core::PortTemporal::Flow { closes: false },
        }],
        outputs: Vec::new(),
        configuration: Default::default(),
        limits: CapabilityLimits {
            max_active_instances: 16,
            max_queue_items: 1,
            max_queue_bytes: conduit_time::RHYTHM_STATE_ENCODED_LEN as u32,
        },
        terminal_behavior: KindTerminalBehavior::CompletesWhenInputsClose,
        hosted_implementation_required: true,
        browser_manifestation_honest: true,
        pico_manifestation_honest: false,
        example: "show: presentation/rhythm".to_string(),
    }
}

#[cfg(feature = "form-catalog")]
pub fn install_rhythm_presentation_catalog(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    use conduit_form::{KindProjection, KindSignature};
    let contract = rhythm_presentation_contract();
    startup.insert(KindSignature {
        kind: RHYTHM_PRESENTATION_KIND.to_string(),
        startup_parameters: vec![],
    })?;
    profile
        .insert(KindProjection {
            kind_id: contract.kind_id,
            kind_contract_revision: conduit_core::KindIdentity::from(
                RHYTHM_PRESENTATION_CONTRACT_REVISION,
            ),
            inputs: contract.inputs,
            outputs: contract.outputs,
            configuration: vec![],
        })
        .map_err(|error| error.to_string())
}
