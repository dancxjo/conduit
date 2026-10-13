use super::{KindTerminalBehavior, StandardKindContract};
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;
#[cfg(feature = "plot-catalog")]
use conduit_core::KindIdentity;
use conduit_core::{kind_id, port_id, CapabilityLimits, Kind, PortDescriptor, PortDirection};

pub const TICK_FACE_KIND: &str = "face/tick";
pub const TICK_FACE_CONTRACT_REVISION: &str = "conduit.std/face-tick@2";

pub fn tick_face_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(TICK_FACE_KIND),
        plain_name: "Tick face".to_string(),
        summary: "Present each exact typed tick while the play remains alive.".to_string(),
        inputs: vec![PortDescriptor {
            port_id: port_id("tick"),
            value_kind: kind_id(conduit_time::TICK_VALUE_KIND),
            direction: PortDirection::Input,
            temporal: conduit_core::PortTemporal::Flow { closes: false },
            abnormal_kind: None,
        }],
        outputs: Vec::new(),
        configuration: crate::data_label_configuration(),
        limits: CapabilityLimits {
            max_active_instances: 16,
            max_queue_items: 4,
            max_queue_bytes: 64,
        },
        terminal_behavior: KindTerminalBehavior::CompletesWhenInputsClose,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: "show: face/tick(\"Clock ticks\")".to_string(),
    }
}

pub fn tick_face_semantic_contract() -> Kind {
    tick_face_contract().into_semantic_contract(TICK_FACE_CONTRACT_REVISION)
}

#[cfg(feature = "plot-catalog")]
pub fn tick_face_kind_projection() -> conduit_plot::KindProjection {
    use conduit_plot::KindProjection;
    let contract = tick_face_contract();
    KindProjection {
        kind_id: contract.kind_id,
        kind_contract_revision: KindIdentity::from(TICK_FACE_CONTRACT_REVISION),
        inputs: contract.inputs,
        outputs: contract.outputs,
        configuration: contract.configuration,
    }
}

#[cfg(feature = "plot-catalog")]
pub fn install_tick_face_catalog(
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    use conduit_plot::KindSignature;
    startup.insert(KindSignature {
        kind: TICK_FACE_KIND.to_string(),
        startup_parameters: crate::data_label_startup_signature(),
    })?;
    profile
        .insert_kind(tick_face_semantic_contract())
        .map_err(|error| error.to_string())
}
