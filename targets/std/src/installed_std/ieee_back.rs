//! Initialized IEEE realization identity and pre-Play bounds admission.
use super::back::{BackBudget, BackFactory, InstalledBack};
use crate::numerical_host::ieee_step::IeeeBinary64Back;
use conduit_core::*;
use conduit_kernel::HostedValueStore;
use conduit_semantic_catalog::{
    ieee_binary64_configuration, ieee_binary64_quiet_semantic_contract,
    ieee_binary64_semantic_contract, IeeeBinary64Configuration, IEEE_BINARY64_KIND,
    IEEE_BINARY64_QUIET_KIND,
};
use std::fmt::Write;
const IMPLEMENTATION: &str = "conduit.std/ieee-binary64@1";
const ARITHMETIC_PROFILE: &str = "std/ieee-binary64-x86_64-sse2@1";
const QUIET_PROFILE: &str = "std/ieee-binary64-quiet-integer@1";
const BOUNDS: &str = "std/ieee-binary64-bounds@1";
pub(super) static FACTORY: BackFactory = BackFactory {
    implementation_id: IMPLEMENTATION,
    budget,
    prepare,
};
fn contract(kind: &str) -> Result<Kind, String> {
    match kind {
        IEEE_BINARY64_KIND => Ok(ieee_binary64_semantic_contract()),
        IEEE_BINARY64_QUIET_KIND => Ok(ieee_binary64_quiet_semantic_contract()),
        _ => Err("ieee/unsupported-kind@1".into()),
    }
}
fn example_configuration(quiet: bool) -> IeeeBinary64Configuration {
    if quiet {
        IeeeBinary64Configuration::Quiet {
            predicate: IeeeBinary64QuietPredicate::Equal,
        }
    } else {
        IeeeBinary64Configuration::Arithmetic {
            operation: IeeeBinary64Operation::Add,
            rounding: Binary64Rounding::NearestTiesEven,
        }
    }
}
fn bounds(back: &IeeeBinary64Back) -> Result<StructuredConfigurationValue, String> {
    let integer = StructuredInfoType::leaf(kind_id("value/u64")).unwrap();
    let fields = [
        ("maximum-operations", 1usize),
        ("input-payload-bytes", 16),
        ("output-payload-bytes", back.output_bytes()),
        (
            "retained-encoder-heap-bytes",
            back.retained_capacity_bytes(),
        ),
    ];
    let build = || -> Result<_, StructuredInfoRefusal> {
        let ty = StructuredInfoType::record(
            kind_id(BOUNDS),
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
                        StructuredInfoValue::leaf(
                            integer.clone(),
                            (*value as u64).to_le_bytes().to_vec(),
                        )?,
                    )
                })
                .collect::<Result<_, _>>()?,
        )?;
        Ok((
            value.value_type().profile()?.value_kind().clone(),
            value.canonical_bytes()?,
        ))
    };
    let (profile, bytes) = build().map_err(|e| format!("IEEE bounds: {e:?}"))?;
    StructuredConfigurationValue::new(profile, bytes).ok_or("ieee/invalid-bounds@1".into())
}
pub(crate) fn offer(kind: &str) -> Result<CapabilityOffer, String> {
    let quiet = kind == IEEE_BINARY64_QUIET_KIND;
    let contract = contract(kind)?;
    let back = IeeeBinary64Back::new(example_configuration(quiet))?;
    let digest = semantic_digest(
        "std/ieee-binary64-artifact@1",
        concat!(
            include_str!("ieee_back.rs"),
            include_str!("../numerical_host/ieee_step.rs"),
            include_str!("../numerical_host/ieee_native.rs"),
            include_str!("../numerical_host/ieee_encoding.rs"),
            include_str!("../../../../semantics/catalog/src/numerical_realization/ieee.rs"),
            include_str!("../../../../semantics/catalog/numerical/types.conduit"),
            include_str!("../../../../architecture/core/src/exact_numeric/binary64/ieee.rs"),
            include_str!(
                "../../../../architecture/core/src/exact_numeric/binary64/ieee/special.rs"
            ),
            include_str!(
                "../../../../architecture/core/src/exact_numeric/binary64/ieee/comparison.rs"
            ),
            include_str!(
                "../../../../architecture/core/src/structured_info/prepared_composition.rs"
            )
        )
        .as_bytes(),
    );
    let mut artifact = String::from("sha256:");
    for byte in digest {
        write!(&mut artifact, "{byte:02x}").unwrap();
    }
    let mut offer = BackOfferBuilder::new(
        contract,
        Back {
            capability_id: format!("{IMPLEMENTATION}/{kind}").into(),
            execution_profile_id: if quiet {
                QUIET_PROFILE
            } else {
                ARITHMETIC_PROFILE
            }
            .into(),
            implementation_id: IMPLEMENTATION.into(),
            artifact_id: artifact.into(),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build();
    offer.realization_properties = vec![bounds(&back)?];
    Ok(offer)
}
pub(crate) fn owns_offer(offer: &CapabilityOffer) -> bool {
    offer.implementation.implementation_id.as_str() == IMPLEMENTATION
        && matches!(
            (
                offer.kind_id.as_str(),
                offer.implementation.execution_profile_id.as_str()
            ),
            (IEEE_BINARY64_KIND, ARITHMETIC_PROFILE) | (IEEE_BINARY64_QUIET_KIND, QUIET_PROFILE)
        )
}
fn admitted(gear: &PlannedGear) -> Result<IeeeBinary64Back, String> {
    let expected = offer(gear.kind_id.as_str())?;
    let contract = contract(gear.kind_id.as_str())?;
    if gear.kind_contract_revision != expected.kind_contract_revision
        || gear.capability_id != expected.capability_id
        || gear.execution_profile_id != expected.implementation.execution_profile_id
        || gear.implementation_id != expected.implementation.implementation_id
        || gear.artifact_id != expected.implementation.artifact_id
        || gear.inputs != expected.inputs
        || gear.outputs != expected.outputs
        || gear.limits != expected.limits
        || gear.semantic_contract != contract.semantic_contract()
        || gear.realization_properties != expected.realization_properties
        || !gear.host_calls.is_empty()
        || !gear.resources.is_empty()
        || !gear.authority.is_empty()
        || gear.base.is_some()
        || !gear.realization_characteristics.is_empty()
        || !gear.pool_references.is_empty()
        || !gear.terminal_transductions.is_empty()
    {
        return Err("ieee/planned-realization-differs@1".into());
    }
    IeeeBinary64Back::new(ieee_binary64_configuration(
        &gear.kind_id,
        &gear.configuration,
    )?)
}
fn budget(gear: &PlannedGear) -> Result<BackBudget, String> {
    let back = admitted(gear)?;
    let maximum = back.output_bytes() as u32;
    Ok(BackBudget {
        value_items: 3,
        value_bytes: 16 + maximum,
        host_requests: 0,
        sign_items: 8,
        maximum_value_bytes: maximum.max(8),
    })
}
fn prepare(gear: &PlannedGear, _: &mut HostedValueStore) -> Result<InstalledBack, String> {
    Ok(InstalledBack::IeeeBinary64(admitted(gear)?))
}

#[cfg(test)]
#[path = "ieee_back_tests.rs"]
mod tests;
