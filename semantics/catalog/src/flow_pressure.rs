//! Exact checked flow-pressure seams distinct from `state/latest`.

#[cfg(feature = "form-catalog")]
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;
use conduit_core::{
    kind_id, port_id, CapabilityLimits, DeliveryPressurePolicy, KindContractRevision, KindId,
    PortDescriptor, PortDirection, PortTemporal, SemanticCapabilityContract,
};

pub const FLOW_BACKPRESSURE_KIND: &str = "flow/backpressure";
pub const FLOW_BACKPRESSURE_REVISION: &str = "conduit.flow/backpressure@1";
pub const FLOW_COALESCE_LATEST_KIND: &str = "flow/coalesce-latest";
pub const FLOW_COALESCE_LATEST_REVISION: &str = "conduit.flow/coalesce-latest@1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowPressureContract {
    pub kind_id: KindId,
    pub kind_contract_revision: KindContractRevision,
    pub inputs: Vec<PortDescriptor>,
    pub outputs: Vec<PortDescriptor>,
    pub limits: CapabilityLimits,
}

impl From<FlowPressureContract> for SemanticCapabilityContract {
    fn from(contract: FlowPressureContract) -> Self {
        Self {
            startup_parameters: Vec::new(),
            shorthand: None,
            kind_id: contract.kind_id,
            kind_contract_revision: contract.kind_contract_revision,
            inputs: contract.inputs,
            outputs: contract.outputs,
            limits: contract.limits,
        }
    }
}

pub fn flow_backpressure_contract(
    value_kind: &KindId,
    maximum_item_bytes: u32,
) -> FlowPressureContract {
    FlowPressureContract {
        kind_id: kind_id(FLOW_BACKPRESSURE_KIND),
        kind_contract_revision: KindContractRevision::from(FLOW_BACKPRESSURE_REVISION),
        inputs: vec![port(
            value_kind,
            "in",
            PortDirection::Input,
            PortTemporal::Flow { closes: false },
        )],
        outputs: vec![port(
            value_kind,
            "out",
            PortDirection::Output,
            PortTemporal::Flow { closes: false },
        )],
        limits: limits(maximum_item_bytes),
    }
}

pub fn flow_coalesce_latest_contract(
    value_kind: &KindId,
    maximum_item_bytes: u32,
) -> FlowPressureContract {
    FlowPressureContract {
        kind_id: kind_id(FLOW_COALESCE_LATEST_KIND),
        kind_contract_revision: KindContractRevision::from(FLOW_COALESCE_LATEST_REVISION),
        inputs: vec![port(
            value_kind,
            "in",
            PortDirection::Input,
            PortTemporal::Flow { closes: false },
        )],
        outputs: vec![port(
            value_kind,
            "out",
            PortDirection::Output,
            PortTemporal::Current,
        )],
        limits: limits(maximum_item_bytes),
    }
}

pub fn reviewed_flow_pressure_policy(kind_id: &KindId) -> Option<DeliveryPressurePolicy> {
    match kind_id.as_str() {
        FLOW_BACKPRESSURE_KIND => Some(DeliveryPressurePolicy::PreserveOrder),
        FLOW_COALESCE_LATEST_KIND => Some(DeliveryPressurePolicy::CoalesceLatest),
        _ => None,
    }
}

fn limits(maximum_item_bytes: u32) -> CapabilityLimits {
    CapabilityLimits {
        max_active_instances: 4,
        max_queue_items: 1,
        max_queue_bytes: maximum_item_bytes,
    }
}

fn port(
    value_kind: &KindId,
    name: &str,
    direction: PortDirection,
    temporal: PortTemporal,
) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: value_kind.clone(),
        direction,
        temporal,
    }
}

#[cfg(feature = "form-catalog")]
pub fn install_flow_pressure_kind(
    contract: FlowPressureContract,
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    use conduit_form::{KindDefinition, KindSignature};

    startup.insert(KindSignature {
        kind: contract.kind_id.as_str().into(),
        startup_parameters: Vec::new(),
    })?;
    profile
        .insert(KindDefinition {
            kind_id: contract.kind_id,
            kind_contract_revision: contract.kind_contract_revision,
            inputs: contract.inputs,
            outputs: contract.outputs,
            configuration: Vec::new(),
        })
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reviewed_kind_ids_map_to_distinct_policies() {
        assert_eq!(
            reviewed_flow_pressure_policy(&kind_id(FLOW_BACKPRESSURE_KIND)),
            Some(DeliveryPressurePolicy::PreserveOrder)
        );
        assert_eq!(
            reviewed_flow_pressure_policy(&kind_id(FLOW_COALESCE_LATEST_KIND)),
            Some(DeliveryPressurePolicy::CoalesceLatest)
        );
        assert_eq!(
            reviewed_flow_pressure_policy(&kind_id("flow/unknown")),
            None
        );
    }

    #[test]
    fn coalescing_contract_stays_distinct_from_state_latest() {
        let value_kind = kind_id(conduit_core::SCALAR_INFO_ID);
        let latest =
            flow_coalesce_latest_contract(&value_kind, conduit_core::SCALAR_ENCODED_LEN as u32);
        assert_eq!(
            latest.inputs[0].temporal,
            PortTemporal::Flow { closes: false }
        );
        assert_eq!(latest.outputs[0].temporal, PortTemporal::Current);
        assert_ne!(latest.kind_id.as_str(), crate::LATEST_KIND);
    }

    #[cfg(feature = "form-catalog")]
    #[test]
    fn exact_contracts_install_for_checked_forms() {
        let mut startup = conduit_form::StartupCatalog::new();
        let mut profile = conduit_form::ProfileCatalog::new();
        let value_kind = kind_id(conduit_core::SCALAR_INFO_ID);
        install_flow_pressure_kind(
            flow_backpressure_contract(&value_kind, conduit_core::SCALAR_ENCODED_LEN as u32),
            &mut startup,
            &mut profile,
        )
        .unwrap();
        install_flow_pressure_kind(
            flow_coalesce_latest_contract(&value_kind, conduit_core::SCALAR_ENCODED_LEN as u32),
            &mut startup,
            &mut profile,
        )
        .unwrap();
        assert_eq!(
            profile
                .get(&kind_id(FLOW_BACKPRESSURE_KIND))
                .unwrap()
                .kind_contract_revision
                .as_str(),
            FLOW_BACKPRESSURE_REVISION
        );
        assert_eq!(
            profile
                .get(&kind_id(FLOW_COALESCE_LATEST_KIND))
                .unwrap()
                .kind_contract_revision
                .as_str(),
            FLOW_COALESCE_LATEST_REVISION
        );
    }
}
