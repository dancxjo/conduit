//! Immutable exact Value-activation contract and its canonical identity.
use super::*;
use crate::KernelCompositeDefinition;
use conduit_core::{semantic_digest, PlanId, PortDirection, PortTemporal};

const ACTIVATION_CONTRACT_INFO_ID: &str = "conduit.execution.bounded-activation-contract.v1";

/// The immutable checked and planned contract for repeatedly activating one
/// exact Value behavior. This is temporal lifting machinery; it does not alter
/// the selected behavior's own port modality.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedActivationContract {
    pub selected_plan_id: PlanId,
    pub input_port: PortId,
    pub input_value_kind: KindId,
    pub input_abnormal_kind: Option<KindId>,
    pub output_port: PortId,
    pub output_value_kind: KindId,
    pub output_abnormal_kind: Option<KindId>,
    pub maximum_active: u16,
    pub maximum_queue_items: u16,
    pub maximum_queue_bytes: u32,
    pub maximum_items: u16,
    pub identity: [u8; 32],
}

impl BoundedActivationContract {
    pub(super) fn for_definition(
        definition: &KernelCompositeDefinition,
        input_port: PortId,
        output_port: PortId,
        maximum_items: u16,
    ) -> Result<Self, BoundedActivationError> {
        let input = definition
            .boundary
            .input_fronts
            .iter()
            .find(|front| front.external_port.port_id == input_port)
            .ok_or_else(|| BoundedActivationError::UnknownInput(input_port.clone()))?;
        let output = definition
            .boundary
            .output_fronts
            .iter()
            .find(|front| front.external_port.port_id == output_port)
            .ok_or_else(|| BoundedActivationError::UnknownOutput(output_port.clone()))?;
        if input.external_port.direction != PortDirection::Input
            || input.external_port.temporal != PortTemporal::Value
        {
            return Err(BoundedActivationError::NotValueInput(input_port));
        }
        if output.external_port.direction != PortDirection::Output
            || output.external_port.temporal != PortTemporal::Value
        {
            return Err(BoundedActivationError::NotValueOutput(output_port));
        }

        let maximum_active = 1u16;
        let maximum_queue_items = 1u16;
        let maximum_queue_bytes = definition.external_capability.limits.max_queue_bytes;
        if maximum_items == 0 {
            return Err(BoundedActivationError::PlannedContractMismatch);
        }
        let mut encoded = Vec::new();
        encode_string(&mut encoded, definition.internal_plan.plan_id.as_str());
        encode_string(&mut encoded, input.external_port.port_id.as_str());
        encode_string(&mut encoded, input.external_port.value_kind.as_str());
        encode_optional_kind(&mut encoded, input.external_port.abnormal_kind.as_ref());
        encode_string(&mut encoded, output.external_port.port_id.as_str());
        encode_string(&mut encoded, output.external_port.value_kind.as_str());
        encode_optional_kind(&mut encoded, output.external_port.abnormal_kind.as_ref());
        encoded.extend_from_slice(&maximum_active.to_le_bytes());
        encoded.extend_from_slice(&maximum_queue_items.to_le_bytes());
        encoded.extend_from_slice(&maximum_queue_bytes.to_le_bytes());
        encoded.extend_from_slice(&maximum_items.to_le_bytes());
        Ok(Self {
            selected_plan_id: definition.internal_plan.plan_id.clone(),
            input_port: input.external_port.port_id.clone(),
            input_value_kind: input.external_port.value_kind.clone(),
            input_abnormal_kind: input.external_port.abnormal_kind.clone(),
            output_port: output.external_port.port_id.clone(),
            output_value_kind: output.external_port.value_kind.clone(),
            output_abnormal_kind: output.external_port.abnormal_kind.clone(),
            maximum_active,
            maximum_queue_items,
            maximum_queue_bytes,
            maximum_items,
            identity: semantic_digest(ACTIVATION_CONTRACT_INFO_ID, &encoded),
        })
    }
}

fn encode_string(encoded: &mut Vec<u8>, value: &str) {
    encoded.extend_from_slice(&(value.len() as u32).to_le_bytes());
    encoded.extend_from_slice(value.as_bytes());
}

fn encode_optional_kind(encoded: &mut Vec<u8>, value: Option<&KindId>) {
    match value {
        Some(kind) => {
            encoded.push(1);
            encode_string(encoded, kind.as_str());
        }
        None => encoded.push(0),
    }
}
