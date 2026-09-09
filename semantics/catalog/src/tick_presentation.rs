use super::{StandardKindContract, TerminalBehavior};
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;
use conduit_core::{
    kind_id, port_id, CapabilityLimits, KindContractRevision, PortDescriptor, PortDirection,
};

pub const TICK_PRESENTATION_KIND: &str = "presentation/tick";
pub const TICK_PRESENTATION_CONTRACT_REVISION: &str = "conduit.std/presentation-tick@2";

pub fn tick_presentation_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(TICK_PRESENTATION_KIND),
        plain_name: "Tick presentation".to_string(),
        summary: "Present each exact typed tick while the Play remains alive.".to_string(),
        inputs: vec![PortDescriptor {
            port_id: port_id("tick"),
            value_kind: kind_id(conduit_time::TICK_VALUE_KIND),
            direction: PortDirection::Input,
            temporal: conduit_core::PortTemporal::Flow { closes: false },
        }],
        outputs: Vec::new(),
        configuration: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 16,
            max_queue_items: 1,
            max_queue_bytes: conduit_time::TICK_ENCODED_LEN,
        },
        terminal_behavior: TerminalBehavior::CompletesWhenInputsClose,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: "show: presentation/tick".to_string(),
    }
}

#[cfg(feature = "form-catalog")]
pub fn tick_presentation_kind_definition() -> conduit_form::KindDefinition {
    use conduit_form::KindDefinition;
    let contract = tick_presentation_contract();
    KindDefinition {
        kind_id: contract.kind_id,
        kind_contract_revision: KindContractRevision::from(TICK_PRESENTATION_CONTRACT_REVISION),
        inputs: contract.inputs,
        outputs: contract.outputs,
        configuration: Vec::new(),
    }
}

#[cfg(feature = "form-catalog")]
pub fn install_tick_presentation_catalog(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    use conduit_form::KindSignature;
    startup.insert(KindSignature {
        kind: TICK_PRESENTATION_KIND.to_string(),
        startup_parameters: Vec::new(),
    })?;
    profile
        .insert(tick_presentation_kind_definition())
        .map_err(|error| error.to_string())
}
