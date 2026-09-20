//! Explicit temporal adapters for one finite framed-record submission.

use alloc::{string::ToString, vec, vec::Vec};
use conduit_core::{
    kind_id, port_id, CapabilityLimits, KindContractRevision, PortDescriptor, PortDirection,
    PortTemporal, SemanticCapabilityContract, MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};
use conduit_form::{KindDefinition, KindSignature, ProfileCatalog, StartupCatalog};

use crate::framed_typed_record_type;

pub const RECORD_SINGLETON_STREAM_KIND: &str = "record/singleton-stream";
pub const RECORD_EXACTLY_ONE_KIND: &str = "record/exactly-one";
pub const RECORD_TEMPORAL_CONTRACT_REVISION: &str = "conduit.net/record-temporal@1";

pub fn install_record_temporal_catalogs(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), alloc::string::String> {
    for definition in definitions() {
        startup.insert(KindSignature {
            kind: definition.kind_id.as_str().to_string(),
            startup_parameters: Vec::new(),
        })?;
        profile
            .insert(definition)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

pub fn record_singleton_stream_definition() -> KindDefinition {
    definitions()[0].clone()
}

pub fn record_exactly_one_definition() -> KindDefinition {
    definitions()[1].clone()
}

pub fn record_singleton_stream_semantic_contract() -> SemanticCapabilityContract {
    semantic_contract(record_singleton_stream_definition())
}

pub fn record_exactly_one_semantic_contract() -> SemanticCapabilityContract {
    semantic_contract(record_exactly_one_definition())
}

fn semantic_contract(definition: KindDefinition) -> SemanticCapabilityContract {
    SemanticCapabilityContract {
        startup_parameters: Vec::new(),
        shorthand: Some((
            definition.inputs[0].port_id.clone(),
            definition.outputs[0].port_id.clone(),
        )),
        kind_id: definition.kind_id,
        kind_contract_revision: definition.kind_contract_revision,
        inputs: definition.inputs,
        outputs: definition.outputs,
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 4,
            max_queue_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32 * 4,
        },
    }
}

fn definitions() -> [KindDefinition; 2] {
    let kind = framed_typed_record_type()
        .profile()
        .expect("framed record profile is finite")
        .value_kind()
        .clone();
    [
        KindDefinition {
            kind_id: kind_id(RECORD_SINGLETON_STREAM_KIND),
            kind_contract_revision: KindContractRevision::from(RECORD_TEMPORAL_CONTRACT_REVISION),
            inputs: vec![port(
                "record",
                kind.clone(),
                PortDirection::Input,
                PortTemporal::Value,
            )],
            outputs: vec![port(
                "stream",
                kind.clone(),
                PortDirection::Output,
                PortTemporal::Flow { closes: true },
            )],
            configuration: Vec::new(),
        },
        KindDefinition {
            kind_id: kind_id(RECORD_EXACTLY_ONE_KIND),
            kind_contract_revision: KindContractRevision::from(RECORD_TEMPORAL_CONTRACT_REVISION),
            inputs: vec![port(
                "stream",
                kind.clone(),
                PortDirection::Input,
                PortTemporal::Flow { closes: true },
            )],
            outputs: vec![port(
                "record",
                kind,
                PortDirection::Output,
                PortTemporal::Value,
            )],
            configuration: Vec::new(),
        },
    ]
}

fn port(
    name: &str,
    value_kind: conduit_core::KindId,
    direction: PortDirection,
    temporal: PortTemporal,
) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind,
        direction,
        temporal,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adapters_make_the_value_flow_boundary_explicit() {
        let singleton = record_singleton_stream_definition();
        assert_eq!(singleton.inputs[0].temporal, PortTemporal::Value);
        assert_eq!(
            singleton.outputs[0].temporal,
            PortTemporal::Flow { closes: true }
        );
        let exactly_one = record_exactly_one_definition();
        assert_eq!(
            exactly_one.inputs[0].temporal,
            PortTemporal::Flow { closes: true }
        );
        assert_eq!(exactly_one.outputs[0].temporal, PortTemporal::Value);
    }

    #[test]
    fn semantic_contracts_own_shorthand_and_capacity() {
        for contract in [
            record_singleton_stream_semantic_contract(),
            record_exactly_one_semantic_contract(),
        ] {
            assert_eq!(
                contract.shorthand,
                Some((
                    contract.inputs[0].port_id.clone(),
                    contract.outputs[0].port_id.clone()
                ))
            );
            assert_eq!(contract.limits.max_queue_items, 4);
            assert_eq!(
                contract.limits.max_queue_bytes,
                MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32 * 4
            );
        }
    }
}
