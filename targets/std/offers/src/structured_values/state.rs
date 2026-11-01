//! Explicit finite State offer; callers must install its implementation before advertising it.
use conduit_core::{
    ArtifactId, CapabilityId, CapabilityOffer, ExecutionProfileId, HostCallContractId,
    HostCallRequirement, ImplementationId, ImplementationOffer, ResourceClassId,
    ResourceRequirement, StateLifetime, StateRetentionSupport, StructuredInfoRefusal,
    StructuredInfoType,
};

pub const STATE_VALUE_STD_PROFILE: &str = "std/state-value-kernel-4096@1";
pub const STATE_VALUE_STD_IMPLEMENTATION: &str = "std/kernel-state-value-4096@1";
pub const STATE_VALUE_STD_ARTIFACT: &str = "conduit-kernel/state-delay@1";
pub const STATE_VALUE_STD_MAXIMUM_BYTES: u32 = 4096;
pub const STATE_VALUE_DURABLE_STD_MAXIMUM_BYTES: u32 = conduit_data::MAXIMUM_DATA_TEXT_BYTES;
pub const STATE_VALUE_DURABLE_STD_PROFILE: &str = "std/state-value-body-durable-4096@1";
pub const STATE_VALUE_DURABLE_STD_IMPLEMENTATION: &str = "std/state-value-body-durable@2";
pub const STATE_VALUE_DURABLE_STD_ARTIFACT: &str = "conduit-std-host/durable-state@1";
pub const STATE_VALUE_DURABLE_RECOVER_HOST_CALL: &str =
    "conduit.host/state-value-durable-recover@1";
pub const STATE_VALUE_DURABLE_COMMIT_HOST_CALL: &str = "conduit.host/state-value-durable-commit@1";
pub const STATE_VALUE_DURABLE_RESOURCE_CLASS: &str = "conduit.resource/body-durable-state-slot@1";
pub const STATE_VALUE_DURABLE_RECEIPT_BYTES: u32 = 1 + 8 + 32;
pub const STATE_VALUE_DURABLE_RECOVERY_METADATA_BYTES: u32 = 2 + 8 + 32;
pub const STATE_VALUE_DURABLE_RECOVERY_VALUE_BYTES: u32 = STATE_VALUE_DURABLE_STD_MAXIMUM_BYTES;

/// Construct the exact offer for the kernel's finite canonical-value envelope.
/// This does not register a capability or authorize an effect. The installation
/// must independently validate the sealed State contract before Play starts.
pub fn state_value_std_offer(
    type_name: &str,
    value_type: &StructuredInfoType,
    default_value: &conduit_core::StructuredInfoValue,
) -> Result<CapabilityOffer, StructuredInfoRefusal> {
    let mut contract = conduit_semantic_catalog::state_value::state_value_semantic_contract(
        type_name,
        value_type,
        default_value,
    )
    .map_err(|_| StructuredInfoRefusal::WrongType)?;
    contract.limits.max_queue_bytes = STATE_VALUE_STD_MAXIMUM_BYTES;
    let value_kind = contract.outputs[0].value_kind.as_str();
    let semantic_contract = contract.semantic_contract();
    conduit_core::capability_offer_from_parts! {
        capability_id: CapabilityId::from(format!("std-state-value-{value_kind}")),
        kind_id: contract.kind_id,
        kind_contract_revision: contract.kind_contract_revision,
        startup_parameters: contract.startup_parameters,
        shorthand: Some((
            conduit_core::port_id("next"),
            conduit_core::port_id("current"),
        )),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(STATE_VALUE_STD_PROFILE),
            implementation_id: ImplementationId::from(STATE_VALUE_STD_IMPLEMENTATION),
            artifact_id: ArtifactId::from(STATE_VALUE_STD_ARTIFACT),
        },
        inputs: contract.inputs,
        outputs: contract.outputs,
        semantic_contract,
        host_calls: Vec::new(),
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: contract.limits,
    }
    .with_state_retention(StateRetentionSupport {
        maximum_lifetime: StateLifetime::Play,
    })
    .map_err(|_| StructuredInfoRefusal::WrongType)
}

/// Construct the distinct Body-retention offer. The ordinary in-memory offer
/// remains Play-lived; selecting this offer seals durable residence and exact
/// recovery/commit operations into the Plan instead of silently strengthening
/// it during execution.
pub fn state_value_durable_std_offer(
    type_name: &str,
    value_type: &StructuredInfoType,
    default_value: &conduit_core::StructuredInfoValue,
) -> Result<CapabilityOffer, StructuredInfoRefusal> {
    let mut offer = state_value_std_offer(type_name, value_type, default_value)?;
    offer.capability_id = CapabilityId::from(format!(
        "std-state-value-body-durable-{}",
        offer.outputs[0].value_kind.as_str()
    ));
    offer.implementation = ImplementationOffer {
        execution_profile_id: ExecutionProfileId::from(STATE_VALUE_DURABLE_STD_PROFILE),
        implementation_id: ImplementationId::from(STATE_VALUE_DURABLE_STD_IMPLEMENTATION),
        artifact_id: ArtifactId::from(STATE_VALUE_DURABLE_STD_ARTIFACT),
    };
    offer.limits.max_queue_bytes = STATE_VALUE_DURABLE_STD_MAXIMUM_BYTES;
    offer.host_calls = vec![
        HostCallRequirement {
            contract_id: HostCallContractId::from(STATE_VALUE_DURABLE_COMMIT_HOST_CALL),
            target_kind: Some(offer.kind_id.clone()),
            maximum_in_flight: 1,
            maximum_input_bytes: STATE_VALUE_DURABLE_STD_MAXIMUM_BYTES,
            maximum_output_bytes: STATE_VALUE_DURABLE_RECEIPT_BYTES,
        },
        HostCallRequirement {
            contract_id: HostCallContractId::from(STATE_VALUE_DURABLE_RECOVER_HOST_CALL),
            target_kind: Some(offer.kind_id.clone()),
            maximum_in_flight: 1,
            maximum_input_bytes: STATE_VALUE_DURABLE_RECOVERY_METADATA_BYTES,
            maximum_output_bytes: STATE_VALUE_DURABLE_RECOVERY_VALUE_BYTES,
        },
    ];
    offer.resource_requirements = vec![ResourceRequirement {
        class_id: ResourceClassId::from(STATE_VALUE_DURABLE_RESOURCE_CLASS),
        units: 1,
        protected_role: None,
        compute: None,
        content: None,
    }];
    offer.state_retention = Some(StateRetentionSupport {
        maximum_lifetime: StateLifetime::Body,
    });
    offer
        .validate_state_retention()
        .map_err(|_| StructuredInfoRefusal::WrongType)?;
    Ok(offer)
}

/// Specialize the existing durable implementation to one independently selected
/// value ceiling. This narrows advertised queues and Host Calls; it grants no
/// residence, resources or authority and leaves the default factory unchanged.
pub fn state_value_durable_std_offer_with_bound(
    type_name: &str,
    value_type: &StructuredInfoType,
    default_value: &conduit_core::StructuredInfoValue,
    maximum_value_bytes: u32,
) -> Result<CapabilityOffer, StructuredInfoRefusal> {
    if !(1..=STATE_VALUE_DURABLE_STD_MAXIMUM_BYTES).contains(&maximum_value_bytes) {
        return Err(StructuredInfoRefusal::CanonicalEncodingTooLarge);
    }
    let mut offer = state_value_durable_std_offer(type_name, value_type, default_value)?;
    // Borrow the factory's already encoded canonical initializer rather than
    // allocating a second encoding just to count its exact bytes.
    let initial = offer
        .semantic_contract
        .configuration
        .iter()
        .find(|field| field.key == "initial")
        .ok_or(StructuredInfoRefusal::WrongType)?;
    let conduit_core::ConfigurationValue::Structured(initial) = &initial.default_value else {
        return Err(StructuredInfoRefusal::WrongType);
    };
    if initial.canonical_value().len() > maximum_value_bytes as usize {
        return Err(StructuredInfoRefusal::CanonicalEncodingTooLarge);
    }
    offer.limits.max_queue_bytes = maximum_value_bytes;
    offer.host_calls[0].maximum_input_bytes = maximum_value_bytes;
    offer.host_calls[1].maximum_output_bytes = maximum_value_bytes;
    Ok(offer)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn offer_preserves_typed_meaning_and_names_its_smaller_realization_bound() {
        let ty =
            StructuredInfoType::leaf(conduit_core::kind_id(conduit_core::BOOL_INFO_ID)).unwrap();
        let contract =
            conduit_semantic_catalog::state_value::state_value_contract("Cell", &ty).unwrap();
        let default = conduit_core::StructuredInfoValue::leaf(
            ty.clone(),
            conduit_core::InfoBool::FALSE.encode().to_vec(),
        )
        .unwrap();
        let offer = state_value_std_offer("Cell", &ty, &default).unwrap();
        assert_eq!(offer.inputs, contract.inputs);
        assert_eq!(offer.outputs, contract.outputs);
        assert_eq!(offer.startup_parameters, contract.startup_parameters);
        assert_eq!(
            offer.kind_contract_revision,
            contract.kind_contract_revision
        );
        assert_eq!(offer.limits.max_queue_bytes, STATE_VALUE_STD_MAXIMUM_BYTES);
        assert!(offer.limits.max_queue_bytes < contract.limits.max_queue_bytes);
        assert!(offer.host_calls.is_empty());
        assert!(offer.authority_requirements.is_empty());
        assert_eq!(
            offer.state_retention,
            Some(StateRetentionSupport {
                maximum_lifetime: StateLifetime::Play,
            })
        );
    }

    #[test]
    fn durable_offer_is_distinct_and_seals_body_retention_operations() {
        let ty =
            StructuredInfoType::leaf(conduit_core::kind_id(conduit_core::BOOL_INFO_ID)).unwrap();
        let default = conduit_core::StructuredInfoValue::leaf(
            ty.clone(),
            conduit_core::InfoBool::FALSE.encode().to_vec(),
        )
        .unwrap();
        let ordinary = state_value_std_offer("Cell", &ty, &default).unwrap();
        let durable = state_value_durable_std_offer("Cell", &ty, &default).unwrap();
        assert_eq!(durable.inputs, ordinary.inputs);
        assert_eq!(durable.outputs, ordinary.outputs);
        assert_ne!(durable.implementation, ordinary.implementation);
        assert_eq!(durable.host_calls.len(), 2);
        assert_eq!(durable.resource_requirements.len(), 1);
        assert_eq!(
            durable.limits.max_queue_bytes,
            conduit_data::MAXIMUM_DATA_TEXT_BYTES
        );
        assert_eq!(
            durable.host_calls[1].maximum_output_bytes,
            conduit_data::MAXIMUM_DATA_TEXT_BYTES
        );
        assert_eq!(
            durable.state_retention,
            Some(StateRetentionSupport {
                maximum_lifetime: StateLifetime::Body,
            })
        );
        assert_eq!(
            ordinary.state_retention,
            Some(StateRetentionSupport {
                maximum_lifetime: StateLifetime::Play,
            })
        );
    }
    #[test]
    fn bounded_durable_offer_narrows_only_selected_value_envelope() {
        let ty =
            StructuredInfoType::leaf(conduit_core::kind_id(conduit_core::BOOL_INFO_ID)).unwrap();
        let default = conduit_core::StructuredInfoValue::leaf(
            ty.clone(),
            conduit_core::InfoBool::FALSE.encode().to_vec(),
        )
        .unwrap();
        let original = state_value_durable_std_offer("Cell", &ty, &default).unwrap();
        assert_eq!(
            original,
            state_value_durable_std_offer_with_bound("Cell", &ty, &default, 4096).unwrap()
        );
        let selected =
            state_value_durable_std_offer_with_bound("Cell", &ty, &default, 1024).unwrap();
        assert_eq!(selected.semantic_contract, original.semantic_contract);
        assert_eq!(selected.inputs, original.inputs);
        assert_eq!(selected.outputs, original.outputs);
        assert_eq!(selected.implementation, original.implementation);
        assert_eq!(
            selected.authority_requirements,
            original.authority_requirements
        );
        assert_eq!(
            selected.resource_requirements,
            original.resource_requirements
        );
        assert_eq!(selected.limits.max_queue_bytes, 1024);
        assert_eq!(selected.host_calls[0].maximum_input_bytes, 1024);
        assert_eq!(
            selected.host_calls[0].maximum_output_bytes,
            STATE_VALUE_DURABLE_RECEIPT_BYTES
        );
        assert_eq!(
            selected.host_calls[1].maximum_input_bytes,
            STATE_VALUE_DURABLE_RECOVERY_METADATA_BYTES
        );
        assert_eq!(selected.host_calls[1].maximum_output_bytes, 1024);
    }
    #[test]
    fn bounded_durable_offer_refuses_outside_ceiling_or_unfit_canonical_default() {
        let ty =
            StructuredInfoType::leaf(conduit_core::kind_id(conduit_core::BOOL_INFO_ID)).unwrap();
        let default = conduit_core::StructuredInfoValue::leaf(
            ty.clone(),
            conduit_core::InfoBool::FALSE.encode().to_vec(),
        )
        .unwrap();
        let exact = default.canonical_bytes().unwrap().len() as u32;
        assert!(state_value_durable_std_offer_with_bound("Cell", &ty, &default, exact).is_ok());
        for maximum in [0, exact - 1, STATE_VALUE_DURABLE_STD_MAXIMUM_BYTES + 1] {
            assert_eq!(
                state_value_durable_std_offer_with_bound("Cell", &ty, &default, maximum),
                Err(StructuredInfoRefusal::CanonicalEncodingTooLarge)
            );
        }
    }
}
