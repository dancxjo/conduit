//! Source-bound physical allowance and finite initialized numerical factories.
use super::back::{BackBudget, BackFactory, InstalledBack};
use crate::numerical_host::near_step::{NearQuantityBack, NEAR_TOTAL_NUMERICAL_WORK};
use conduit_core::*;
use conduit_kernel::HostedValueStore;
use conduit_semantic_catalog::{
    near_quantity_limit, near_quantity_semantic_contract, NearQuantityAlgorithm,
    NUMERICAL_NEAR_KIND,
};
use std::fmt::Write;
const IMPLEMENTATION: &str = "conduit.std/quantity-near@1";
const EXACT: &str = "std/quantity-near-exact-rational@1";
const FLOAT: &str = "std/quantity-near-binary32-single-round@1";
pub(super) static FACTORY: BackFactory = BackFactory {
    implementation_id: IMPLEMENTATION,
    budget,
    prepare,
};
fn requested(configuration: &[ConfigurationEntry]) -> Result<Quantity, String> {
    near_quantity_limit(configuration)?;
    let ConfigurationValue::Quantity(value) = &configuration[0].value else {
        unreachable!("checked near request")
    };
    Ok(value.value())
}
fn bounds(
    back: &NearQuantityBack,
    algorithm: NearQuantityAlgorithm,
) -> Result<StructuredConfigurationValue, String> {
    let integer = StructuredInfoType::leaf(kind_id("value/u64")).unwrap();
    let fields = [
        (
            "algorithm",
            if algorithm == NearQuantityAlgorithm::Binary32 {
                32
            } else {
                0
            },
        ),
        (
            "maximum-input-bytes",
            QUANTITY_VALUE_MAX_ENCODED_BYTES as u64,
        ),
        (
            "maximum-output-bytes",
            u64::from(NearQuantityBack::output_bound()),
        ),
        ("maximum-numerical-limb-work", NEAR_TOTAL_NUMERICAL_WORK),
        ("maximum-numeric-bits", EXACT_NUMERIC_MAX_BITS as u64),
        (
            "retained-prepared-payload-bytes",
            (back.retained_capacity_bytes() + core::mem::size_of::<NearQuantityBack>()) as u64,
        ),
    ];
    let build = || -> Result<_, StructuredInfoRefusal> {
        let ty = StructuredInfoType::record(
            kind_id("std/quantity-near-bounds@1"),
            fields
                .iter()
                .map(|(n, _)| StructuredFieldType::new(*n, integer.clone()))
                .collect::<Result<_, _>>()?,
        )?;
        let value = StructuredInfoValue::record(
            ty,
            fields
                .iter()
                .map(|(n, v)| {
                    StructuredFieldValue::new(
                        *n,
                        StructuredInfoValue::leaf(integer.clone(), v.to_le_bytes().to_vec())?,
                    )
                })
                .collect::<Result<_, _>>()?,
        )?;
        Ok((
            value.value_type().profile()?.value_kind().clone(),
            value.canonical_bytes()?,
        ))
    };
    let (profile, bytes) = build().map_err(|why| format!("near bounds: {why:?}"))?;
    StructuredConfigurationValue::new(profile, bytes).ok_or("numeric/near-invalid-bounds@1".into())
}
fn offer(limit: Quantity, algorithm: NearQuantityAlgorithm) -> Result<CapabilityOffer, String> {
    let back = NearQuantityBack::new(limit, algorithm)?;
    let value = QuantityConfigurationValue::from_value(limit)
        .map_err(|why| format!("near limit: {why:?}"))?;
    let binding = quantity_realization_property(&value)
        .map_err(|why| format!("near request binding: {why:?}"))?;
    let digest = semantic_digest("std/near-request@1", binding.canonical_value());
    let mut identity = String::with_capacity(64);
    for b in digest {
        write!(&mut identity, "{b:02x}").unwrap();
    }
    let artifact = semantic_digest(
        "std/near-artifact@1",
        concat!(
            include_str!("near_back.rs"),
            include_str!("../numerical_host/near_step.rs"),
            include_str!("../numerical_host/near_encoding.rs"),
            include_str!("../../../../semantics/catalog/src/numerical_realization/near.rs"),
            include_str!("../../../../semantics/catalog/numerical/types.conduit"),
            include_str!("../../../../architecture/core/src/exact_numeric/binary32.rs"),
            include_str!("../../../../architecture/core/src/exact_numeric/binary32/projection.rs"),
            include_str!("../../../../architecture/core/src/exact_numeric/rational/ieee.rs"),
            include_str!("../../../../architecture/core/src/exact_numeric/rational/binary32.rs"),
            include_str!("../../../../architecture/core/src/exact_numeric/quantity/binary32.rs"),
            include_str!("../../../../architecture/core/src/realization_properties.rs")
        )
        .as_bytes(),
    );
    let mut artifact_id = String::with_capacity(71);
    artifact_id.push_str("sha256:");
    for b in artifact {
        write!(&mut artifact_id, "{b:02x}").unwrap();
    }
    let mut offer = BackOfferBuilder::new(
        near_quantity_semantic_contract(),
        Back {
            capability_id: format!(
                "std/source-near/{}/{identity}",
                if algorithm == NearQuantityAlgorithm::Binary32 {
                    "10-binary32"
                } else {
                    "20-exact"
                }
            )
            .into(),
            execution_profile_id: if algorithm == NearQuantityAlgorithm::Binary32 {
                FLOAT
            } else {
                EXACT
            }
            .into(),
            implementation_id: IMPLEMENTATION.into(),
            artifact_id: artifact_id.into(),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build();
    let wire =
        QuantityWireContract::new(vec![("value".into(), QuantityValueProfile::Exact)], vec![])
            .map_err(|why| format!("near wire: {why:?}"))?
            .to_property()
            .map_err(|why| format!("near wire: {why:?}"))?;
    offer.realization_properties = vec![binding, wire, bounds(&back, algorithm)?];
    offer
        .realization_properties
        .sort_by(|a, b| a.profile().cmp(b.profile()));
    Ok(offer)
}
pub(crate) fn offers(configuration: &[ConfigurationEntry]) -> Result<Vec<CapabilityOffer>, String> {
    let limit = requested(configuration)?;
    let mut result = vec![offer(limit, NearQuantityAlgorithm::ExactRational)?];
    if limit.coefficient() != 0 {
        result.push(offer(limit, NearQuantityAlgorithm::Binary32)?);
    }
    Ok(result)
}
pub(crate) fn owns_offer(offer: &CapabilityOffer) -> bool {
    offer.capability_id.as_str().starts_with("std/source-near/")
        && offer.implementation.implementation_id.as_str() == IMPLEMENTATION
        && matches!(
            offer.implementation.execution_profile_id.as_str(),
            EXACT | FLOAT
        )
}
fn admitted(gear: &PlannedGear) -> Result<NearQuantityBack, String> {
    if gear.kind_id.as_str() != NUMERICAL_NEAR_KIND {
        return Err("numeric/near-wrong-kind@1".into());
    }
    let limit = requested(&gear.configuration)?;
    let algorithm = match gear.execution_profile_id.as_str() {
        EXACT => NearQuantityAlgorithm::ExactRational,
        FLOAT => NearQuantityAlgorithm::Binary32,
        _ => return Err("numeric/near-unavailable-profile@1".into()),
    };
    if algorithm == NearQuantityAlgorithm::Binary32 && limit.coefficient() == 0 {
        return Err("numeric/near-zero-limit-requires-exact-profile@1".into());
    }
    let expected = offer(limit, algorithm)?;
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
        return Err("numeric/near-planned-realization-differs@1".into());
    }
    NearQuantityBack::new(limit, algorithm)
}
fn budget(gear: &PlannedGear) -> Result<BackBudget, String> {
    let _ = admitted(gear)?;
    let maximum = NearQuantityBack::output_bound();
    Ok(BackBudget {
        value_items: 2,
        value_bytes: QUANTITY_VALUE_MAX_ENCODED_BYTES as u32 + maximum,
        host_requests: 0,
        sign_items: 8,
        maximum_value_bytes: maximum,
    })
}
fn prepare(gear: &PlannedGear, _: &mut HostedValueStore) -> Result<InstalledBack, String> {
    Ok(InstalledBack::NearQuantity(admitted(gear)?))
}

#[cfg(test)]
#[path = "near_back_tests.rs"]
mod tests;
