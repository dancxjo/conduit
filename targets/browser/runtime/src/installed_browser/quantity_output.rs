//! Browser realization of the exact Quantity leaf wrapper and existing presenter.

use super::factory::{
    validate_placement, BrowserHostResult, BrowserInstallation, BrowserManifestation,
};
use super::BrowserOperation;
use conduit_core::{
    CapabilityOffer, PlannedGear, Quantity, StructuredInfoValue, StructuredInfoValueShape,
};
use conduit_semantic_catalog::{
    quantity_info_prefix, wrapped_quantity_type, QUANTITY_INFO_MAXIMUM_BYTES,
};
use std::sync::OnceLock;

pub(crate) const WRAP_OPERATION: &str = "conduit.host/wrap-quantity@1";
const WRAP_IMPLEMENTATION: &str = "browser/kernel-wrap-quantity@1";
pub(crate) const PRESENTATION_IMPLEMENTATION: &str = "browser/presentation-quantity-leaf@1";
pub(crate) const DIRECT_PRESENTATION_IMPLEMENTATION: &str = "browser/presentation-quantity@1";
static PREFIX: OnceLock<Vec<u8>> = OnceLock::new();

pub(super) static WRAP: BrowserInstallation = BrowserInstallation {
    implementation_id: WRAP_IMPLEMENTATION,
    offer: wrap_offer,
    prepare: prepare_wrap,
    perform: None,
};
pub(super) static PRESENTATION: BrowserInstallation = BrowserInstallation {
    implementation_id: PRESENTATION_IMPLEMENTATION,
    offer: presentation_offer,
    prepare: prepare_presentation,
    perform: Some(present),
};
pub(super) static DIRECT_PRESENTATION: BrowserInstallation = BrowserInstallation {
    implementation_id: DIRECT_PRESENTATION_IMPLEMENTATION,
    offer: direct_presentation_offer,
    prepare: prepare_direct_presentation,
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
        vec![conduit_core::HostOperationRequirement {
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

pub(super) fn presentation_offer() -> CapabilityOffer {
    crate::structured_offers::structured_presentation_offer(
        "Quantity",
        &wrapped_quantity_type(),
        crate::structured_offers::BrowserOfferIdentity {
            capability: PRESENTATION_IMPLEMENTATION,
            profile: "browser/quantity-presentation@1",
            implementation: PRESENTATION_IMPLEMENTATION,
            artifact: "conduit-browser-runtime/quantity-presentation@1",
        },
    )
}

pub(super) fn direct_presentation_offer() -> CapabilityOffer {
    let mut offer = presentation_offer();
    let contract = conduit_semantic_catalog::quantity_presentation_definition();
    offer.kind_id = contract.kind_id.clone();
    offer.kind_contract_revision = contract.kind_contract_revision;
    offer.capability_id = DIRECT_PRESENTATION_IMPLEMENTATION.into();
    offer.implementation.execution_profile_id = DIRECT_PRESENTATION_IMPLEMENTATION.into();
    offer.implementation.implementation_id = DIRECT_PRESENTATION_IMPLEMENTATION.into();
    offer.implementation.artifact_id = "conduit-browser-runtime/quantity-presentation@1".into();
    offer.host_operations[0].contract_id = "conduit.host/presentation-quantity@1".into();
    offer.host_operations[0].target_kind = Some(contract.kind_id);
    offer
}

pub(super) fn install_catalogs(
    _: &mut conduit_form::StartupCatalog,
    _: &mut conduit_form::ProfileCatalog,
) -> Result<(), String> {
    Ok(())
}

fn prepare_wrap(
    placement: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<BrowserOperation, String> {
    validate_placement(placement, &wrap_offer())?;
    PREFIX.get_or_init(quantity_info_prefix);
    Ok(BrowserOperation::unary(
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

fn prepare_presentation(
    placement: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<BrowserOperation, String> {
    validate_placement(placement, &presentation_offer())?;
    Ok(BrowserOperation::presentation(
        conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        1,
    ))
}

fn prepare_direct_presentation(
    placement: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<BrowserOperation, String> {
    validate_placement(placement, &direct_presentation_offer())?;
    Ok(BrowserOperation::presentation(
        QUANTITY_INFO_MAXIMUM_BYTES as u32,
        1,
    ))
}

pub(crate) fn decode(input: &[u8]) -> Result<Quantity, String> {
    let value = StructuredInfoValue::from_canonical_bytes(input)
        .map_err(|error| format!("decode Quantity leaf: {error:?}"))?;
    if value.value_type() != &wrapped_quantity_type() {
        return Err("wrong exact Quantity presentation profile".into());
    }
    let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
        return Err("Quantity presentation is not a leaf".into());
    };
    Quantity::decode(bytes).map_err(|error| format!("decode presented Quantity: {error:?}"))
}

fn present(_: &PlannedGear, input: &[u8]) -> Result<BrowserHostResult, String> {
    decode(input)?;
    Ok(BrowserHostResult {
        output: None,
        manifestation: Some(BrowserManifestation {
            kind_id: conduit_semantic_catalog::STRUCTURED_PRESENTATION_KIND,
            canonical_value: input.to_vec(),
        }),
    })
}

fn present_direct(_: &PlannedGear, input: &[u8]) -> Result<BrowserHostResult, String> {
    decode(input)?;
    Ok(BrowserHostResult {
        output: None,
        manifestation: Some(BrowserManifestation {
            kind_id: conduit_semantic_catalog::QUANTITY_PRESENTATION_KIND,
            canonical_value: input.to_vec(),
        }),
    })
}
