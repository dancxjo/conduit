//! Exact bounded collection of one closing Flow into one sequence Value.

#[cfg(feature = "form-catalog")]
use alloc::string::ToString;
use alloc::{vec, vec::Vec};
use conduit_core::{
    kind_id, port_id, BoundedCollectSemanticLaw, CapabilityLimits, CheckedValueContract,
    FrontValueContract, FrontValueLocation, Kind, KindIdentity, KindSemanticLaw, PortDescriptor,
    PortDirection, PortTemporal, PreparedLeafSequenceEncoder, UNIT_INFO_ID,
};

pub const FLOW_COLLECT_KIND: &str = "flow/collect";
pub const FLOW_COLLECT_CONTRACT_REVISION: &str = "conduit.flow/collect@1";
pub const FLOW_COLLECT_INPUT_PORT: &str = "values";
pub const FLOW_COLLECT_OUTPUT_PORT: &str = "collected";

/// Specializes `flow/collect` over one exact element envelope, finite item
/// maximum, and typed overflow disposition.
pub fn flow_collect_semantic_contract(
    element: &CheckedValueContract,
    maximum_items: u16,
    overflow_disposition: &CheckedValueContract,
) -> Result<Kind, &'static str> {
    if element.maximum_bytes == 0 && element.value_kind.as_str() != UNIT_INFO_ID {
        return Err("flow/collect requires a finite canonical element envelope");
    }
    if overflow_disposition.maximum_bytes == 0
        && overflow_disposition.value_kind.as_str() != UNIT_INFO_ID
    {
        return Err("flow/collect requires a finite canonical overflow disposition");
    }
    let encoder = PreparedLeafSequenceEncoder::new(
        element.value_kind.clone(),
        element.maximum_bytes,
        maximum_items,
    )
    .map_err(|_| "flow/collect output exceeds structured Info bounds")?;
    let collection = CheckedValueContract::new(
        encoder
            .value_type()
            .map_err(|_| "flow/collect output type is invalid")?
            .profile()
            .map_err(|_| "flow/collect output profile is invalid")?
            .value_kind()
            .clone(),
        encoder.maximum_bytes(),
        vec![],
    )
    .map_err(|_| "flow/collect output contract is invalid")?;
    let input_port = port_id(FLOW_COLLECT_INPUT_PORT);
    let output_port = port_id(FLOW_COLLECT_OUTPUT_PORT);
    let law = BoundedCollectSemanticLaw {
        input_port_id: input_port.clone(),
        output_port_id: output_port.clone(),
        element: element.clone(),
        collection: collection.clone(),
        maximum_items,
        overflow_disposition: overflow_disposition.clone(),
    };
    let retained_element_bytes = element
        .maximum_bytes
        .checked_mul(
            u32::from(maximum_items)
                .checked_add(1)
                .ok_or("flow/collect retained item count overflows")?,
        )
        .ok_or("flow/collect retained element bytes overflow")?;
    Ok(Kind {
        startup_parameters: Vec::new(),
        shorthand: None,
        kind_id: kind_id(FLOW_COLLECT_KIND),
        kind_contract_revision: KindIdentity::from(FLOW_COLLECT_CONTRACT_REVISION),
        inputs: vec![PortDescriptor {
            port_id: input_port.clone(),
            value_kind: element.value_kind.clone(),
            direction: PortDirection::Input,
            temporal: PortTemporal::Flow { closes: true },
            abnormal_kind: None,
        }],
        outputs: vec![PortDescriptor {
            port_id: output_port.clone(),
            value_kind: collection.value_kind.clone(),
            direction: PortDirection::Output,
            temporal: PortTemporal::Value,
            abnormal_kind: Some(overflow_disposition.value_kind.clone()),
        }],
        configuration: Vec::new(),
        semantic_laws: vec![
            KindSemanticLaw::ValueContracts(vec![
                FrontValueContract {
                    location: FrontValueLocation::Input(input_port),
                    contract: element.clone(),
                },
                FrontValueContract {
                    location: FrontValueLocation::Output(output_port.clone()),
                    contract: collection,
                },
                FrontValueContract {
                    location: FrontValueLocation::OutputAbnormal(output_port),
                    contract: overflow_disposition.clone(),
                },
            ]),
            KindSemanticLaw::BoundedCollect(law),
        ],
        limits: CapabilityLimits {
            max_active_instances: 8,
            max_queue_items: maximum_items
                .checked_add(2)
                .ok_or("flow/collect queue item envelope overflows")?,
            max_queue_bytes: retained_element_bytes
                .checked_add(encoder.maximum_bytes())
                .and_then(|bytes| bytes.checked_add(overflow_disposition.maximum_bytes))
                .ok_or("flow/collect queue byte envelope overflows")?,
        },
    })
}

#[cfg(feature = "form-catalog")]
pub fn install_flow_collect_kind(
    element: &CheckedValueContract,
    maximum_items: u16,
    overflow_disposition: &CheckedValueContract,
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    startup.insert(conduit_form::KindSignature {
        kind: FLOW_COLLECT_KIND.to_string(),
        startup_parameters: Vec::new(),
    })?;
    profile
        .insert_kind(
            flow_collect_semantic_contract(element, maximum_items, overflow_disposition)
                .map_err(str::to_string)?,
        )
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value(kind: &str, bytes: u32) -> CheckedValueContract {
        CheckedValueContract::new(kind_id(kind), bytes, vec![]).unwrap()
    }

    #[test]
    fn contract_collects_one_closing_flow_into_one_exact_sequence_value() {
        let element = value("value/text", 32);
        let overflow = value("flow/collect-overflow@1", 1);
        let contract = flow_collect_semantic_contract(&element, 4, &overflow).unwrap();

        assert_eq!(contract.inputs.len(), 1);
        assert_eq!(contract.outputs.len(), 1);
        assert_eq!(
            contract.inputs[0].temporal,
            PortTemporal::Flow { closes: true }
        );
        assert_eq!(contract.outputs[0].temporal, PortTemporal::Value);
        assert_eq!(contract.outputs[0].abnormal_kind, Some(overflow.value_kind));
        let law = contract.bounded_collect().unwrap();
        assert_eq!(law.maximum_items, 4);
        assert_eq!(law.element, element);
        assert_eq!(law.collection, contract.value_contracts()[1].contract);
        contract.validate().unwrap();
    }

    #[test]
    fn specialization_identity_changes_with_bound_maximum_and_overflow_type() {
        let overflow = value("flow/collect-overflow@1", 1);
        let first = flow_collect_semantic_contract(&value("value/text", 32), 4, &overflow).unwrap();
        let bounded =
            flow_collect_semantic_contract(&value("value/text", 64), 4, &overflow).unwrap();
        let count = flow_collect_semantic_contract(&value("value/text", 32), 5, &overflow).unwrap();
        let refusal = flow_collect_semantic_contract(
            &value("value/text", 32),
            4,
            &value("flow/other-overflow@1", 1),
        )
        .unwrap();
        assert_ne!(first.semantic_contract(), bounded.semantic_contract());
        assert_ne!(first.semantic_contract(), count.semantic_contract());
        assert_ne!(first.semantic_contract(), refusal.semantic_contract());
    }

    #[test]
    fn zero_or_nonfinite_specializations_refuse() {
        let element = value("value/text", 32);
        let overflow = value("flow/collect-overflow@1", 1);
        assert!(flow_collect_semantic_contract(&element, 0, &overflow).is_err());
        assert!(
            flow_collect_semantic_contract(&value("value/unbounded", 0), 4, &overflow).is_err()
        );
        assert!(
            flow_collect_semantic_contract(&element, 4, &value("flow/unbounded-overflow", 0))
                .is_err()
        );
    }

    #[cfg(feature = "form-catalog")]
    #[test]
    fn installer_registers_the_exact_specialization() {
        let element = value("value/text", 32);
        let overflow = value("flow/collect-overflow@1", 1);
        let mut startup = conduit_form::StartupCatalog::new();
        let mut profile = conduit_form::ProfileCatalog::new();
        install_flow_collect_kind(&element, 4, &overflow, &mut startup, &mut profile).unwrap();

        let installed = profile.canonical_kind(&kind_id(FLOW_COLLECT_KIND)).unwrap();
        let expected = flow_collect_semantic_contract(&element, 4, &overflow).unwrap();
        assert_eq!(installed, &expected);
    }
}
