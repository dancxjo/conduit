//! Source-request binding and independently recomputed installed interval profile.
use super::back::{BackBudget, BackFactory, InstalledBack};
use crate::numerical_host::enclosure_step::{MathematicalEnclosureBack, ENCLOSURE_TOTAL_WORK};
use conduit_core::*;
use conduit_kernel::HostedValueStore;
use conduit_semantic_catalog::*;
use std::fmt::Write;
const IMPLEMENTATION: &str = "conduit.std/mathematical-enclosure@1";
const PROFILE: &str = "std/borrowed-exact-interval@1";
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
fn bounds(back: &MathematicalEnclosureBack) -> Result<StructuredConfigurationValue, String> {
    let integer = StructuredInfoType::leaf(kind_id("value/u64")).unwrap();
    let fields = [
        (
            "maximum-input-bytes",
            MATHEMATICAL_VALUE_MAX_ENCODED_BYTES as u64,
        ),
        (
            "maximum-output-bytes",
            u64::from(MATHEMATICAL_ENCLOSE_MAX_BYTES),
        ),
        ("maximum-numerical-limb-work", ENCLOSURE_TOTAL_WORK),
        ("maximum-numeric-bits", EXACT_NUMERIC_MAX_BITS as u64),
        (
            "retained-prepared-payload-bytes",
            (back.retained_capacity_bytes() + core::mem::size_of::<MathematicalEnclosureBack>())
                as u64,
        ),
    ];
    let build = || -> Result<_, StructuredInfoRefusal> {
        let ty = StructuredInfoType::record(
            kind_id("std/mathematical-enclosure-bounds@1"),
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
    let (profile, bytes) = build().map_err(|why| format!("enclosure bounds: {why:?}"))?;
    StructuredConfigurationValue::new(profile, bytes)
        .ok_or("numeric/enclose-bounds-invalid@1".into())
}
pub(crate) fn offer(configuration: &[ConfigurationEntry]) -> Result<CapabilityOffer, String> {
    let req = mathematical_enclose_requirements(configuration)?;
    let back = MathematicalEnclosureBack::new(req)?;
    let ConfigurationValue::Structured(binding) = &configuration[0].value else {
        unreachable!()
    };
    let request = hex(semantic_digest(
        "std/enclose-request@1",
        binding.canonical_value(),
    ));
    let artifact = hex(semantic_digest("std/enclose-artifact@1", concat!(
        include_str!("enclosure_back.rs"),
        include_str!("../numerical_host/enclosure_step.rs"),
        include_str!("../numerical_host/enclosure_encoding.rs"),
        include_str!("../../../../semantics/catalog/src/numerical_realization/enclosure.rs"),
        include_str!("../../../../semantics/catalog/numerical/types.conduit"),
        include_str!("../../../../architecture/core/src/exact_numeric/symbolic/enclosure.rs"),
        include_str!("../../../../architecture/core/src/exact_numeric/symbolic/enclosure/borrowed.rs"),
        include_str!("../../../../architecture/core/src/exact_numeric/symbolic/enclosure/borrowed_projection.rs"),
        include_str!("../../../../architecture/core/src/exact_numeric/symbolic/enclosure/evaluate.rs"),
        include_str!("../../../../architecture/core/src/exact_numeric/symbolic/enclosure/pi.rs"),
        include_str!("../../../../architecture/core/src/exact_numeric/symbolic/enclosure/interval.rs"),
        include_str!("../../../../architecture/core/src/exact_numeric/symbolic/enclosure/inverse_node.rs"),
        include_str!("../../../../architecture/core/src/exact_numeric/symbolic.rs"),
        include_str!("../../../../architecture/core/src/exact_numeric/symbolic/codec.rs"),
        include_str!("../../../../architecture/core/src/structured_info/prepared_composition.rs"),
        include_str!("../../../../architecture/core/src/exact_numeric/rational.rs"),
        include_str!("../../../../architecture/core/src/exact_numeric/natural.rs")
    ).as_bytes()));
    let mut result = BackOfferBuilder::new(
        mathematical_enclose_semantic_contract(),
        Back {
            capability_id: format!("std/source-enclose/{request}").into(),
            execution_profile_id: PROFILE.into(),
            implementation_id: IMPLEMENTATION.into(),
            artifact_id: format!("sha256:{artifact}").into(),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build();
    result.realization_properties = vec![binding.clone(), bounds(&back)?];
    result
        .realization_properties
        .sort_by(|a, b| a.profile().cmp(b.profile()));
    Ok(result)
}
pub(crate) fn owns_offer(offer: &CapabilityOffer) -> bool {
    offer
        .capability_id
        .as_str()
        .starts_with("std/source-enclose/")
        && offer.implementation.implementation_id.as_str() == IMPLEMENTATION
        && offer.implementation.execution_profile_id.as_str() == PROFILE
}
fn admitted(gear: &PlannedGear) -> Result<MathematicalEnclosureBack, String> {
    if gear.kind_id.as_str() != MATHEMATICAL_ENCLOSE_KIND {
        return Err("numeric/enclose-wrong-kind@1".into());
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
        return Err("numeric/enclose-planned-realization-differs@1".into());
    }
    MathematicalEnclosureBack::new(mathematical_enclose_requirements(&gear.configuration)?)
}
fn budget(gear: &PlannedGear) -> Result<BackBudget, String> {
    let _ = admitted(gear)?;
    Ok(BackBudget {
        value_items: 2,
        value_bytes: MATHEMATICAL_VALUE_MAX_ENCODED_BYTES as u32 + MATHEMATICAL_ENCLOSE_MAX_BYTES,
        host_requests: 0,
        sign_items: 8,
        maximum_value_bytes: MATHEMATICAL_ENCLOSE_MAX_BYTES,
    })
}
fn prepare(gear: &PlannedGear, _: &mut HostedValueStore) -> Result<InstalledBack, String> {
    Ok(InstalledBack::MathematicalEnclosure(admitted(gear)?))
}
#[cfg(test)]
#[path = "enclosure_back_tests.rs"]
mod tests;
