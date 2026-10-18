//! Browser realization of the exact Quantity leaf wrapper and existing presenter.

use super::factory::{
    validate_placement, BrowserHostResult, BrowserInstallation, BrowserManifestation,
};
use super::BrowserBack;
use conduit_core::{
    kind_id, present_host_call_requirement, resource_requirement, ArtifactId, Back,
    BackOfferBuilder, CapabilityId, CapabilityOffer, ExecutionProfileId, ImplementationId,
    PlannedGear, Quantity, StructuredInfoValue, StructuredInfoValueShape, FACE_RESOURCE_CLASS,
};
use conduit_semantic_catalog::{
    quantity_info_prefix, wrapped_quantity_type, QUANTITY_INFO_MAXIMUM_BYTES,
};
use std::sync::OnceLock;

pub(crate) const WRAP_OPERATION: &str = "conduit.host/wrap-quantity@1";
const WRAP_IMPLEMENTATION: &str = "browser/kernel-wrap-quantity@1";
pub(crate) const FACE_IMPLEMENTATION: &str = "browser/face-quantity-leaf@1";
pub(crate) const DIRECT_FACE_IMPLEMENTATION: &str = "browser/face-quantity@1";
static PREFIX: OnceLock<Vec<u8>> = OnceLock::new();

pub(super) static WRAP: BrowserInstallation = BrowserInstallation {
    implementation_id: WRAP_IMPLEMENTATION,
    offer: wrap_offer,
    prepare: prepare_wrap,
    perform: None,
};
pub(super) static FACE: BrowserInstallation = BrowserInstallation {
    implementation_id: FACE_IMPLEMENTATION,
    offer: face_offer,
    prepare: prepare_face,
    perform: Some(present),
};
pub(super) static DIRECT_FACE: BrowserInstallation = BrowserInstallation {
    implementation_id: DIRECT_FACE_IMPLEMENTATION,
    offer: direct_face_offer,
    prepare: prepare_direct_face,
    perform: Some(present_direct),
};

fn wrap_offer() -> CapabilityOffer {
    let contract = conduit_semantic_catalog::quantity_info_wrap_contract();
    let target_kind = Some(contract.kind_id.clone());
    conduit_semantic_catalog::realization_offer(
        contract,
        conduit_semantic_catalog::QUANTITY_INFO_WRAP_REVISION,
        conduit_semantic_catalog::RealizationOfferIdentity {
            capability: WRAP_IMPLEMENTATION,
            execution_profile: WRAP_IMPLEMENTATION,
            implementation: WRAP_IMPLEMENTATION,
            artifact: "conduit-browser-runtime/wrap-quantity@1",
        },
        vec![conduit_core::HostCallRequirement {
            contract_id: WRAP_OPERATION.into(),
            target_kind,
            maximum_in_flight: 1,
            maximum_input_bytes: conduit_core::QUANTITY_ENCODED_LEN as u32,
            maximum_output_bytes: QUANTITY_INFO_MAXIMUM_BYTES as u32,
        }],
        Vec::new(),
        Vec::new(),
    )
}

pub(super) fn face_offer() -> CapabilityOffer {
    crate::structured_offers::structured_face_offer(
        "Quantity",
        &wrapped_quantity_type(),
        crate::structured_offers::BrowserOfferIdentity {
            capability: FACE_IMPLEMENTATION,
            profile: "browser/quantity-face@1",
            implementation: FACE_IMPLEMENTATION,
            artifact: "conduit-browser-runtime/quantity-face@1",
        },
    )
}

pub(super) fn direct_face_offer() -> CapabilityOffer {
    BackOfferBuilder::new(
        conduit_semantic_catalog::quantity_face_semantic_contract(),
        Back {
            capability_id: CapabilityId::from(DIRECT_FACE_IMPLEMENTATION),
            execution_profile_id: ExecutionProfileId::from(DIRECT_FACE_IMPLEMENTATION),
            implementation_id: ImplementationId::from(DIRECT_FACE_IMPLEMENTATION),
            artifact_id: ArtifactId::from("conduit-browser-runtime/quantity-face@1"),
            host_calls: vec![present_host_call_requirement(
                kind_id(conduit_semantic_catalog::QUANTITY_FACE_KIND),
                conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
            )],
            resource_requirements: vec![resource_requirement(FACE_RESOURCE_CLASS, 1)],
            authority_requirements: Vec::new(),
        },
    )
    .build()
}

pub(super) fn install_catalogs(
    _: &mut conduit_plot::StartupCatalog,
    _: &mut conduit_plot::ProfileCatalog,
) -> Result<(), String> {
    Ok(())
}

fn prepare_wrap(
    placement: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<BrowserBack, String> {
    validate_placement(placement, &wrap_offer())?;
    PREFIX.get_or_init(quantity_info_prefix);
    Ok(BrowserBack::unary(
        conduit_core::QUANTITY_ENCODED_LEN as u32,
        1,
    ))
}

pub(crate) fn wrap(input: &[u8]) -> Result<([u8; QUANTITY_INFO_MAXIMUM_BYTES], usize), String> {
    Quantity::decode(input).map_err(|error| format!("wrap malformed Quantity: {error:?}"))?;
    let prefix = PREFIX
        .get()
        .ok_or("Quantity wrapper was not prepared before Play")?;
    let mut output = [0; QUANTITY_INFO_MAXIMUM_BYTES];
    let length = prefix.len() + input.len();
    output[..prefix.len()].copy_from_slice(prefix);
    output[prefix.len()..length].copy_from_slice(input);
    Ok((output, length))
}

fn prepare_face(
    placement: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<BrowserBack, String> {
    validate_placement(placement, &face_offer())?;
    Ok(BrowserBack::face(
        conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        1,
    ))
}

fn prepare_direct_face(
    placement: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<BrowserBack, String> {
    validate_placement(placement, &direct_face_offer())?;
    Ok(BrowserBack::face(QUANTITY_INFO_MAXIMUM_BYTES as u32, 1))
}

pub(crate) fn decode(input: &[u8]) -> Result<Quantity, String> {
    let value = StructuredInfoValue::from_canonical_bytes(input)
        .map_err(|error| format!("decode Quantity leaf: {error:?}"))?;
    if value.value_type() != &wrapped_quantity_type() {
        return Err("wrong exact Quantity face profile".into());
    }
    let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
        return Err("Quantity face is not a leaf".into());
    };
    Quantity::decode(bytes).map_err(|error| format!("decode presented Quantity: {error:?}"))
}

fn present(placement: &PlannedGear, input: &[u8]) -> Result<BrowserHostResult, String> {
    decode(input)?;
    Ok(BrowserHostResult {
        output: None,
        manifestation: Some(BrowserManifestation {
            data_label: super::factory::manifestation_label(placement)?,
            kind_id: conduit_semantic_catalog::STRUCTURED_FACE_KIND,
            canonical_value: input.to_vec(),
        }),
    })
}

fn present_direct(placement: &PlannedGear, input: &[u8]) -> Result<BrowserHostResult, String> {
    decode(input)?;
    Ok(BrowserHostResult {
        output: None,
        manifestation: Some(BrowserManifestation {
            data_label: super::factory::manifestation_label(placement)?,
            kind_id: conduit_semantic_catalog::QUANTITY_FACE_KIND,
            canonical_value: input.to_vec(),
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_quantity_realization_preserves_owner_issued_front() {
        let contract = conduit_semantic_catalog::quantity_face_semantic_contract();
        let offer = direct_face_offer();
        assert_eq!(offer.startup_parameters, contract.startup_parameters);
        assert_eq!(offer.shorthand, contract.shorthand);
        assert_eq!(offer.kind_id, contract.kind_id);
        assert_eq!(
            offer.kind_contract_revision,
            contract.kind_contract_revision
        );
        assert_eq!(offer.inputs, contract.inputs);
        assert_eq!(offer.outputs, contract.outputs);
        assert_eq!(offer.limits, contract.limits);
        assert_eq!(offer.host_calls.len(), 1);
        assert_eq!(offer.resource_requirements.len(), 1);
    }

    fn labelled_placement() -> PlannedGear {
        let offered = direct_face_offer();
        conduit_core::planned_gear_from_parts! {
            realization_properties: offered.realization_properties,
            semantic_contract: offered.semantic_contract,
            placement_id: "quantity-label-placement".into(),
            gear_id: "room-temperature".into(),
            kind_id: offered.kind_id,
            kind_contract_revision: offered.kind_contract_revision,
            execution_profile_id: offered.implementation.execution_profile_id,
            configuration: vec![conduit_core::ConfigurationEntry {
                key: "label".into(),
                value: conduit_core::ConfigurationValue::Text("Room temperature".into()),
            }],
            host_id: "browser/quantity-label".into(),
            boot_id: "browser-boot/quantity-label".into(),
            offer_generation: conduit_core::OfferGeneration(1),
            capability_id: offered.capability_id,
            implementation_id: offered.implementation.implementation_id,
            artifact_id: offered.implementation.artifact_id,
            base: None,
            realization_characteristics: Vec::new(),
            limits: offered.limits,
            inputs: offered.inputs,
            outputs: offered.outputs,
            terminal_transductions: Vec::new(),
            host_calls: offered.host_calls,
            resources: Vec::new(),
            authority: Vec::new(),
            pool_references: Vec::new(),
        }
    }

    #[test]
    fn quantity_face_preserves_declared_label_and_typed_canonical_value() {
        let placement = labelled_placement();
        let quantity = Quantity::new(21, conduit_core::Unit::Celsius);
        let bytes = StructuredInfoValue::leaf(wrapped_quantity_type(), quantity.encode().to_vec())
            .unwrap()
            .canonical_bytes()
            .unwrap();
        let result = present_direct(&placement, &bytes).unwrap();
        let manifestation = result.manifestation.unwrap();
        assert_eq!(
            manifestation.data_label.as_deref(),
            Some("Room temperature")
        );
        assert_eq!(manifestation.canonical_value, bytes);
        assert_eq!(decode(&manifestation.canonical_value).unwrap(), quantity);
    }

    #[test]
    fn quantity_face_refuses_missing_or_invalid_label_before_prepare() {
        let mut placement = labelled_placement();
        let mut values = conduit_kernel::HostedValueStore::new(2, 1024, 2048).unwrap();
        assert!(prepare_direct_face(&placement, &mut values).is_ok());
        placement.configuration.clear();
        assert!(prepare_direct_face(&placement, &mut values).is_err());
        for label in [" ", "Room\ntemperature"] {
            placement.configuration = vec![conduit_core::ConfigurationEntry {
                key: "label".into(),
                value: conduit_core::ConfigurationValue::Text(label.into()),
            }];
            assert!(prepare_direct_face(&placement, &mut values).is_err());
        }
    }
}
