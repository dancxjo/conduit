//! Source-bound exact coordinates and independently recomputed installation.
use super::back::{BackBudget, BackFactory, InstalledBack};
#[cfg(test)]
use crate::numerical_host::coordinate_step::COORDINATE_WORK;
use crate::numerical_host::coordinate_step::{QuantityCoordinateBack, COORDINATE_TOTAL_WORK};
use conduit_core::*;
use conduit_kernel::HostedValueStore;
use conduit_semantic_catalog::*;
use std::fmt::Write;
const IMPLEMENTATION: &str = "conduit.std/quantity-coordinates@1";
const PROFILE: &str = "std/exact-quantity-coordinates@1";
pub(super) static FACTORY: BackFactory = BackFactory {
    implementation_id: IMPLEMENTATION,
    budget,
    prepare,
};
fn hex(digest: [u8; 32]) -> String {
    let mut out = String::with_capacity(64);
    for byte in digest {
        write!(&mut out, "{byte:02x}").unwrap();
    }
    out
}
fn bounds(back: &QuantityCoordinateBack) -> Result<StructuredConfigurationValue, String> {
    let integer = StructuredInfoType::leaf(kind_id("value/u64")).unwrap();
    let fields = [
        (
            "maximum-input-bytes",
            MATHEMATICAL_QUANTITY_MAX_ENCODED_BYTES as u64,
        ),
        (
            "maximum-output-bytes",
            u64::from(QUANTITY_COORDINATES_EVIDENCE_MAX_BYTES),
        ),
        (
            "maximum-result-bytes",
            MATHEMATICAL_QUANTITY_MAX_ENCODED_BYTES as u64,
        ),
        ("maximum-numerical-limb-work", COORDINATE_TOTAL_WORK),
        ("maximum-numeric-bits", EXACT_NUMERIC_MAX_BITS as u64),
        (
            "retained-prepared-payload-bytes",
            (back.retained_capacity_bytes() + core::mem::size_of::<QuantityCoordinateBack>())
                as u64,
        ),
    ];
    let build = || -> Result<_, StructuredInfoRefusal> {
        let ty = StructuredInfoType::record(
            kind_id("std/quantity-coordinate-bounds@1"),
            fields
                .iter()
                .map(|(name, _)| StructuredFieldType::new(*name, integer.clone()))
                .collect::<Result<_, _>>()?,
        )?;
        let value = StructuredInfoValue::record(
            ty,
            fields
                .iter()
                .map(|(name, value)| {
                    StructuredFieldValue::new(
                        *name,
                        StructuredInfoValue::leaf(integer.clone(), value.to_le_bytes().to_vec())?,
                    )
                })
                .collect::<Result<_, _>>()?,
        )?;
        Ok((
            value.value_type().profile()?.value_kind().clone(),
            value.canonical_bytes()?,
        ))
    };
    let (profile, bytes) = build().map_err(|why| format!("coordinate bounds: {why:?}"))?;
    StructuredConfigurationValue::new(profile, bytes)
        .ok_or("physical/coordinates-bounds-invalid@1".into())
}
pub(crate) fn offer(configuration: &[ConfigurationEntry]) -> Result<CapabilityOffer, String> {
    let req = quantity_coordinate_target(configuration)?;
    let back = QuantityCoordinateBack::new(req)?;
    let ConfigurationValue::Structured(binding) = &configuration[0].value else {
        unreachable!()
    };
    let request = hex(semantic_digest(
        "std/coordinates-request@1",
        binding.canonical_value(),
    ));
    let artifact = hex(semantic_digest("std/coordinate-artifact@1", concat!(
        include_str!("coordinate_back.rs"),
        include_str!("../numerical_host/coordinate_step.rs"),
        include_str!("../numerical_host/coordinate_step/refusal.rs"),
        include_str!("../numerical_host/coordinate_encoding.rs"),
        include_str!("../../../../semantics/catalog/src/numerical_realization/coordinates.rs"),
        include_str!("../../../../semantics/catalog/numerical/types.conduit"),
        include_str!("../../../../architecture/core/src/exact_numeric/quantity/coordinate_conversion.rs"),
        include_str!("../../../../architecture/core/src/exact_numeric/quantity/coordinate_conversion_projection.rs"),
        include_str!("../../../../architecture/core/src/exact_numeric/quantity/coordinate_conversion_evidence.rs"),
        include_str!("../../../../architecture/core/src/exact_numeric/quantity/coordinate_content_ids.rs"),
        include_str!("../../../../architecture/core/src/exact_numeric/quantity/mathematical_conversion.rs"),
        include_str!("../../../../architecture/core/src/exact_numeric/quantity/value_profile.rs"),
        include_str!("../../../../architecture/core/src/exact_numeric/quantity/value_view.rs"),
        include_str!("../../../../architecture/core/src/exact_numeric/quantity.rs"),
        include_str!("../../../../architecture/core/src/structured_info/prepared_composition.rs"),
        include_str!("../../../../architecture/core/src/exact_numeric/rational.rs"),
        include_str!("../../../../architecture/core/src/exact_numeric/natural.rs")
    ).as_bytes()));
    let mut result = BackOfferBuilder::new(
        quantity_coordinates_semantic_contract(),
        Back {
            capability_id: format!("std/source-coordinates/{request}").into(),
            execution_profile_id: PROFILE.into(),
            implementation_id: IMPLEMENTATION.into(),
            artifact_id: format!("sha256:{artifact}").into(),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build();
    result.realization_properties = vec![
        binding.clone(),
        bounds(&back)?,
        QuantityWireContract::new(
            vec![(port_id("value"), QuantityValueProfile::Mathematical)],
            vec![(port_id("result"), QuantityValueProfile::Mathematical)],
        )
        .map_err(|why| format!("coordinates/wire-profile@1: {why:?}"))?
        .to_property()
        .map_err(|why| format!("coordinates/wire-property@1: {why:?}"))?,
    ];
    result
        .realization_properties
        .sort_by(|a, b| a.profile().cmp(b.profile()));
    Ok(result)
}
pub(crate) fn owns_offer(offer: &CapabilityOffer) -> bool {
    offer
        .capability_id
        .as_str()
        .starts_with("std/source-coordinates/")
        && offer.implementation.implementation_id.as_str() == IMPLEMENTATION
        && offer.implementation.execution_profile_id.as_str() == PROFILE
}
fn admitted(gear: &PlannedGear) -> Result<QuantityCoordinateBack, String> {
    if gear.kind_id.as_str() != QUANTITY_COORDINATES_KIND {
        return Err("physical/coordinates-wrong-kind@1".into());
    }
    let expected = offer(&gear.configuration)?;
    if gear.kind_contract_revision != expected.kind_contract_revision
        || gear.capability_id != expected.capability_id
        || gear.execution_profile_id != expected.implementation.execution_profile_id
        || gear.implementation_id != expected.implementation.implementation_id
        || gear.artifact_id != expected.implementation.artifact_id
        || gear.inputs != expected.inputs
        || gear.outputs != expected.outputs
        || gear.limits != expected.limits
        || gear.semantic_contract != expected.semantic_contract
        || gear.realization_properties != expected.realization_properties
        || !gear.host_calls.is_empty()
        || !gear.resources.is_empty()
        || !gear.authority.is_empty()
        || gear.base.is_some()
        || !gear.realization_characteristics.is_empty()
        || !gear.pool_references.is_empty()
        || !gear.terminal_transductions.is_empty()
    {
        return Err("physical/coordinates-planned-realization-differs@1".into());
    }
    QuantityCoordinateBack::new(quantity_coordinate_target(&gear.configuration)?)
}
fn budget(gear: &PlannedGear) -> Result<BackBudget, String> {
    let _ = admitted(gear)?;
    Ok(BackBudget {
        value_items: 3,
        value_bytes: 2 * MATHEMATICAL_QUANTITY_MAX_ENCODED_BYTES as u32
            + QUANTITY_COORDINATES_EVIDENCE_MAX_BYTES,
        host_requests: 0,
        sign_items: 12,
        maximum_value_bytes: QUANTITY_COORDINATES_EVIDENCE_MAX_BYTES,
    })
}
fn prepare(gear: &PlannedGear, _: &mut HostedValueStore) -> Result<InstalledBack, String> {
    Ok(InstalledBack::QuantityCoordinate(admitted(gear)?))
}

#[cfg(test)]
#[path = "coordinate_back_tests.rs"]
mod tests;
