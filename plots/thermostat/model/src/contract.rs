use crate::{COMMAND_BYTES, STATE_BYTES};
use alloc::{string::String, string::ToString, vec, vec::Vec};
use conduit_core::{
    kind_id, port_id, CapabilityLimits, CheckedValueContract, FrontValueContract,
    FrontValueLocation, Kind, KindIdentity, KindSemanticLaw, KindTerminalBehavior, PortDescriptor,
    PortDirection, PortTemporal,
};
pub const THERMOSTAT_KIND: &str = "thermostat/combine";
pub const THERMOSTAT_REVISION: &str = "conduit.thermostat/combine@1";
pub const STATE_KIND: &str = "conduit.thermostat/state@1";
pub const COMMAND_KIND: &str = "conduit.thermostat/command@1";
pub fn install_catalogs(
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), String> {
    startup.insert(conduit_plot::KindSignature {
        kind: THERMOSTAT_KIND.into(),
        startup_parameters: vec![],
    })?;
    profile
        .insert_kind(thermostat_kind())
        .map_err(|e| e.to_string())
}
pub fn thermostat_kind() -> Kind {
    Kind {
        startup_parameters: Vec::new(),
        shorthand: None,
        kind_id: kind_id(THERMOSTAT_KIND),
        kind_contract_revision: KindIdentity::from(THERMOSTAT_REVISION),
        inputs: vec![
            port("accumulator", STATE_KIND, PortDirection::Input),
            port("item", COMMAND_KIND, PortDirection::Input),
        ],
        outputs: vec![port("combined", STATE_KIND, PortDirection::Output)],
        configuration: Vec::new(),
        semantic_laws: vec![
            KindSemanticLaw::ValueContracts(vec![
                value_contract("accumulator", STATE_KIND, STATE_BYTES, true),
                value_contract("item", COMMAND_KIND, COMMAND_BYTES, true),
                value_contract("combined", STATE_KIND, STATE_BYTES, false),
            ]),
            KindSemanticLaw::Terminal(KindTerminalBehavior::CompletesWhenInputsClose),
        ],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 3,
            max_queue_bytes: (2 * STATE_BYTES + COMMAND_BYTES) as u32,
        },
    }
}

fn value_contract(
    name: &str,
    value_kind: &str,
    maximum_bytes: usize,
    input: bool,
) -> FrontValueContract {
    let port = port_id(name);
    FrontValueContract {
        location: if input {
            FrontValueLocation::Input(port)
        } else {
            FrontValueLocation::Output(port)
        },
        contract: CheckedValueContract::new(kind_id(value_kind), maximum_bytes as u32, vec![])
            .expect("Thermostat's finite Info Form contract"),
    }
}

fn port(name: &str, value_kind: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(value_kind),
        direction,
        temporal: PortTemporal::Value,
        abnormal_kind: None,
    }
}
