//! Exact conversion executes during preparation. Play emits the admitted
//! immutable receipt through the existing fixed-storage structured Value Back.
use super::{
    back::{BackBudget, BackFactory, InstalledBack},
    structured_values_back::StructuredLiteralBack,
};
use conduit_core::*;
use conduit_kernel::{HostedValueStore, ValueStorage};
use conduit_plot::quantity_conversion as conversion;
use std::fmt::Write;

pub(super) const IMPLEMENTATION: &str = "conduit.std/exact-quantity-conversion@1";
const DIFFERENCE_IMPLEMENTATION: &str = "conduit.std/exact-temperature-difference-conversion@1";
const PROFILE: &str = "conduit.std/prepared-quantity-receipt@1";
pub(super) static FACTORY: BackFactory = BackFactory {
    implementation_id: IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) static DIFFERENCE_FACTORY: BackFactory = BackFactory {
    implementation_id: DIFFERENCE_IMPLEMENTATION,
    budget,
    prepare,
};
pub(crate) fn offers() -> [CapabilityOffer; 2] {
    [
        offer(),
        offer_for(conversion::temperature_difference::KIND).expect("reviewed difference Kind"),
    ]
}
fn offer() -> CapabilityOffer {
    offer_for(conversion::KIND).expect("reviewed quantity Kind")
}
fn offer_for(kind: &str) -> Option<CapabilityOffer> {
    let (contract, implementation) = match kind {
        conversion::KIND => (conversion::contract(), IMPLEMENTATION),
        conversion::temperature_difference::KIND => (
            conversion::temperature_difference::contract(),
            DIFFERENCE_IMPLEMENTATION,
        ),
        _ => return None,
    };
    // The installed Back, codec, resolver, reviewed transforms and receipt
    // schema participate in the artifact. No ambient file or external provider
    // substitutes for these compiled sources.
    let digest = semantic_digest(
        "quantity/compiled-conversion-back@1",
        concat!(
            include_str!("quantity_conversion_back.rs"),
            include_str!("../../../../architecture/plot/src/quantity_conversion.rs"),
            include_str!("../../../../architecture/plot/src/quantity_conversion/encoding.rs"),
            include_str!("../../../../architecture/plot/src/quantity_conversion/profile.rs"),
            include_str!(
                "../../../../architecture/plot/src/quantity_conversion/temperature_difference.rs"
            ),
            include_str!("../../../../architecture/core/src/quantity/temperature_difference.rs"),
            include_str!("../../../../architecture/core/src/quantity.rs"),
            include_str!("../../../../architecture/core/src/quantity_prefix.rs"),
            include_str!("../../../../architecture/core/src/quantity_suffix.rs"),
            include_str!("../../../../architecture/core/src/quantity/exact.rs"),
            include_str!("../../../../architecture/core/src/quantity/target.rs"),
            include_str!("../../../../architecture/core/src/quantity/receipt.rs"),
            include_str!("../../../../architecture/core/src/quantity/conversion_law.rs"),
            include_str!("../../../../architecture/core/src/quantity/wide_conversion.rs"),
            include_str!("../../../../architecture/core/src/quantity/magnitude.rs")
        )
        .as_bytes(),
    );
    let mut artifact = String::from("sha256:");
    for byte in digest {
        write!(&mut artifact, "{byte:02x}").expect("write digest");
    }
    Some(
        BackOfferBuilder::new(
            contract,
            Back {
                capability_id: CapabilityId::from(implementation),
                execution_profile_id: ExecutionProfileId::from(PROFILE),
                implementation_id: ImplementationId::from(implementation),
                artifact_id: ArtifactId::from(artifact),
                host_calls: vec![],
                resource_requirements: vec![],
                authority_requirements: vec![],
            },
        )
        .build(),
    )
}

fn admitted(placement: &PlannedGear) -> Result<Vec<u8>, String> {
    let expected =
        offer_for(placement.kind_id.as_str()).ok_or("unsupported quantity conversion Kind")?;
    let contract = if placement.kind_id.as_str() == conversion::KIND {
        conversion::contract()
    } else {
        conversion::temperature_difference::contract()
    };
    if placement.kind_id != expected.kind_id
        || placement.kind_contract_revision != expected.kind_contract_revision
        || placement.capability_id != expected.capability_id
        || placement.execution_profile_id != expected.implementation.execution_profile_id
        || placement.implementation_id != expected.implementation.implementation_id
        || placement.artifact_id != expected.implementation.artifact_id
        || placement.inputs != expected.inputs
        || placement.outputs != expected.outputs
        || placement.limits != expected.limits
        || placement.semantic_contract != contract.semantic_contract()
        || !placement.host_calls.is_empty()
        || !placement.resources.is_empty()
        || !placement.authority.is_empty()
        || placement.base.is_some()
        || !placement.realization_properties.is_empty()
        || !placement.realization_characteristics.is_empty()
        || !placement.pool_references.is_empty()
        || !placement.terminal_transductions.is_empty()
    {
        return Err("quantity conversion differs from its exact installed offer".into());
    }
    // Refuse malformed or oversized configuration before wide arithmetic.
    let fields = contract.configuration;
    if placement.configuration.len() != fields.len() {
        return Err("quantity conversion configuration differs".into());
    }
    for field in &fields {
        let mut entries = placement
            .configuration
            .iter()
            .filter(|entry| entry.key == field.key);
        let entry = entries
            .next()
            .ok_or("quantity conversion configuration field missing")?;
        if entries.next().is_some() {
            return Err("quantity conversion configuration field duplicated".into());
        }
        conduit_plot::validate_configuration_value(field, &entry.value)
            .map_err(|error| format!("quantity conversion configuration: {error}"))?;
    }
    let receipt = if placement.kind_id.as_str() == conversion::KIND {
        conversion::prepare_configuration(&placement.configuration)
    } else {
        conversion::temperature_difference::prepare_configuration(&placement.configuration)
    }
    .map_err(|error| format!("quantity conversion preparation: {error:?}"))?;
    if receipt
        .value_type()
        .profile()
        .map_err(|error| format!("quantity receipt Type: {error:?}"))?
        .value_kind()
        != &placement.outputs[0].value_kind
    {
        return Err("quantity receipt output Type differs".into());
    }
    receipt
        .canonical_bytes()
        .map_err(|error| format!("quantity receipt encoding: {error:?}"))
}

fn budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    let bytes = admitted(placement)?;
    let maximum = u32::try_from(bytes.len()).map_err(|_| "quantity receipt exceeds storage")?;
    if maximum > conversion::MAXIMUM_RECEIPT_BYTES {
        return Err("quantity receipt exceeds admitted profile".into());
    }
    Ok(BackBudget {
        value_items: 2,
        value_bytes: maximum
            .checked_mul(2)
            .ok_or("quantity receipt budget overflow")?,
        host_requests: 0,
        sign_items: 8,
        maximum_value_bytes: maximum,
    })
}
fn prepare(
    placement: &PlannedGear,
    values: &mut HostedValueStore,
) -> Result<InstalledBack, String> {
    let bytes = admitted(placement)?;
    let value = values
        .store(&bytes)
        .map_err(|error| format!("store quantity receipt: {error:?}"))?;
    Ok(InstalledBack::StructuredLiteral(
        StructuredLiteralBack::prepared(value),
    ))
}

#[cfg(test)]
#[path = "quantity_conversion_back_tests.rs"]
mod tests;
