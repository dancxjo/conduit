//! Workspace-owned semantic edges around the portable `llm/present` Front.

use alloc::{
    string::{String, ToString},
    vec,
    vec::Vec,
};
use conduit_core::{
    CapabilityLimits, Kind, KindSemanticLaw, KindTerminalBehavior, PortDescriptor, PortDirection,
    PortTemporal,
};

pub const REQUEST_KIND: &str = "workspace/tutorial-presenter-request";
pub const MANIFESTATION_KIND: &str = "workspace/tutorial-manifestation";
pub const CONTRACT_REVISION: &str = "conduit.workspace/tutorial-presenter@1";

pub fn request_contract() -> Kind {
    contract(
        REQUEST_KIND,
        Vec::new(),
        vec![PortDescriptor {
            port_id: conduit_core::port_id("request"),
            value_kind: conduit_core::kind_id(conduit_ai::GENERATIVE_PRESENTER_INPUT_VALUE_KIND),
            direction: PortDirection::Output,
            temporal: PortTemporal::Value,
        }],
        conduit_ai::MAXIMUM_LLM_INPUT_BYTES as u32,
        KindTerminalBehavior::EmitsOnce,
    )
}

pub fn manifestation_contract() -> Kind {
    contract(
        MANIFESTATION_KIND,
        vec![PortDescriptor {
            port_id: conduit_core::port_id("manifestation"),
            value_kind: conduit_core::kind_id(conduit_ai::GENERATED_MANIFESTATION_VALUE_KIND),
            direction: PortDirection::Input,
            temporal: PortTemporal::Value,
        }],
        Vec::new(),
        conduit_ai::MAXIMUM_LLM_OUTPUT_BYTES as u32,
        KindTerminalBehavior::CompletesAfterFixedCount { count: 1 },
    )
}

fn contract(
    kind: &str,
    inputs: Vec<PortDescriptor>,
    outputs: Vec<PortDescriptor>,
    bytes: u32,
    terminal: KindTerminalBehavior,
) -> Kind {
    Kind {
        startup_parameters: Vec::new(),
        shorthand: None,
        kind_id: conduit_core::kind_id(kind),
        kind_contract_revision: CONTRACT_REVISION.into(),
        inputs,
        outputs,
        configuration: Vec::new(),
        semantic_laws: vec![KindSemanticLaw::Terminal(terminal)],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: bytes,
        },
    }
}

pub fn install_tutorial_presenter_catalog(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), String> {
    for contract in [request_contract(), manifestation_contract()] {
        startup.insert(conduit_form::KindSignature {
            kind: contract.kind_id.as_str().to_string(),
            startup_parameters: Vec::new(),
        })?;
        profile
            .insert(conduit_form::KindProjection {
                kind_id: contract.kind_id,
                kind_contract_revision: contract.kind_contract_revision,
                inputs: contract.inputs,
                outputs: contract.outputs,
                configuration: Vec::new(),
            })
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}
