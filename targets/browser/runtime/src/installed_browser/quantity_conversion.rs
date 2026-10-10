//! Browser realization of the portable checked quantity operations. Arithmetic
//! and receipt encoding finish during preparation. The comparator shares the
//! portable finite Step Back and validates its runtime receipt without allocation.
use super::{BrowserBack, BrowserInstallation, MAXIMUM_BROWSER_VALUE_BYTES};
use conduit_core::*;
use conduit_kernel::{HostedValueStore, ValueStorage};
use conduit_plot::quantity_conversion as conversion;
use std::fmt::Write;

const CONVERTED_EQUALS_ID: &str = "browser/converted-equals@1";
const PROFILE: &str = "browser/prepared-quantity-receipt@1";
const COMPARATOR_PROFILE: &str = "browser/checked-receipt-comparator@1";
const CONVERT_ID: &str = "browser/exact-quantity-conversion@1";
const DIFFERENCE_ID: &str = "browser/exact-temperature-difference-conversion@1";
const COMPARE_ID: &str = "browser/exact-quantity-comparison@1";
const COMPARE_DIFFERENCE_ID: &str = "browser/exact-temperature-difference-comparison@1";

pub(super) static CONVERTED_EQUALS: BrowserInstallation = BrowserInstallation {
    implementation_id: CONVERTED_EQUALS_ID,
    offer: converted_equals_offer,
    prepare,
    perform: None,
};
fn converted_equals_offer() -> CapabilityOffer {
    offer_for(conversion::converted_equals::KIND).expect("reviewed receipt comparator")
}
pub(super) static CONVERT: BrowserInstallation = BrowserInstallation {
    implementation_id: CONVERT_ID,
    offer: conversion_offer,
    prepare,
    perform: None,
};
pub(super) static DIFFERENCE: BrowserInstallation = BrowserInstallation {
    implementation_id: DIFFERENCE_ID,
    offer: difference_offer,
    prepare,
    perform: None,
};
pub(super) static COMPARE: BrowserInstallation = BrowserInstallation {
    implementation_id: COMPARE_ID,
    offer: comparison_offer,
    prepare,
    perform: None,
};
pub(super) static COMPARE_DIFFERENCE: BrowserInstallation = BrowserInstallation {
    implementation_id: COMPARE_DIFFERENCE_ID,
    offer: difference_comparison_offer,
    prepare,
    perform: None,
};
fn conversion_offer() -> CapabilityOffer {
    offer_for(conversion::KIND).expect("reviewed conversion")
}
fn difference_offer() -> CapabilityOffer {
    offer_for(conversion::temperature_difference::KIND).expect("reviewed difference")
}
fn comparison_offer() -> CapabilityOffer {
    offer_for(conversion::comparison::KIND).expect("reviewed comparison")
}
fn difference_comparison_offer() -> CapabilityOffer {
    offer_for(conversion::comparison::DIFFERENCE_KIND).expect("reviewed difference comparison")
}
fn offer_for(kind: &str) -> Option<CapabilityOffer> {
    let contract = operation_contract(kind)?;
    let implementation = match kind {
        conversion::KIND => CONVERT_ID,
        conversion::converted_equals::KIND => CONVERTED_EQUALS_ID,
        conversion::temperature_difference::KIND => DIFFERENCE_ID,
        conversion::comparison::KIND => COMPARE_ID,
        conversion::comparison::DIFFERENCE_KIND => COMPARE_DIFFERENCE_ID,
        _ => return None,
    };
    let digest = semantic_digest(
        "quantity/compiled-browser-back@1",
        concat!(
            include_str!("quantity_conversion.rs"),
            include_str!("../../../../../semantics/catalog/src/converted_equals_back.rs"),
            include_str!("../../../../../architecture/plot/src/quantity_conversion/converted_equals.rs"),
            include_str!("../../../../../architecture/plot/src/quantity_conversion/converted_equals/validation.rs"),
            include_str!("../../../../../architecture/plot/src/quantity_conversion.rs"),
            include_str!("../../../../../architecture/plot/src/quantity_conversion/encoding.rs"),
            include_str!("../../../../../architecture/plot/src/quantity_conversion/comparison.rs"),
            include_str!("../../../../../architecture/plot/src/quantity_conversion/comparison/operand.rs"),
            include_str!("../../../../../architecture/plot/src/quantity_conversion/profile.rs"),
            include_str!("../../../../../architecture/plot/src/quantity_conversion/temperature_difference.rs"),
            include_str!("../../../../../architecture/core/src/quantity/temperature_difference.rs"),
            include_str!("../../../../../architecture/core/src/quantity.rs"),
            include_str!("../../../../../architecture/core/src/unit.rs"),
            include_str!("../../../../../architecture/core/definitions/physical.conduit"),
            include_str!("../../../../../architecture/core/src/physical_definition.rs"),
            include_str!("../../../../../architecture/core/src/physical_definition/dimension.rs"),
            include_str!("../../../../../architecture/core/src/physical_definition/family.rs"),
            include_str!("../../../../../architecture/core/src/physical_definition/generated.rs"),
            include_str!("../../../../../architecture/core/src/physical_definition/role_kind.rs"),
            include_str!("../../../../../architecture/core/src/physical_definition/scalar.rs"),
            include_str!("../../../../../architecture/core/src/physical_definition/unit.rs"),
            include_str!("../../../../../architecture/core/src/quantity_configuration.rs"),
            include_str!("../../../../../architecture/core/src/quantity_suffix.rs"),
            include_str!("../../../../../architecture/core/src/quantity/exact.rs"),
            include_str!("../../../../../architecture/core/src/quantity/target.rs"),
            include_str!("../../../../../architecture/core/src/quantity/receipt.rs"),
            include_str!("../../../../../architecture/core/src/quantity/conversion_law.rs"),
            include_str!("../../../../../architecture/core/src/quantity/wide_conversion.rs"),
            include_str!("../../../../../architecture/core/src/quantity/magnitude.rs")
        ).as_bytes(),
    );
    let mut artifact = String::from("sha256:");
    for byte in digest {
        write!(&mut artifact, "{byte:02x}").expect("write compiled artifact digest");
    }
    let mut offer = BackOfferBuilder::new(
        contract,
        Back {
            capability_id: implementation.into(),
            execution_profile_id: if kind == conversion::converted_equals::KIND {
                COMPARATOR_PROFILE
            } else {
                PROFILE
            }
            .into(),
            implementation_id: implementation.into(),
            artifact_id: artifact.into(),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build();
    // The portable receipt Type allows 8 KiB. This installed browser profile
    // admits the existing 4 KiB Cord limit and refuses larger actual receipts.
    offer.limits.max_queue_bytes = MAXIMUM_BROWSER_VALUE_BYTES as u32;
    Some(offer)
}
fn prepare(placement: &PlannedGear, values: &mut HostedValueStore) -> Result<BrowserBack, String> {
    let expected = offer_for(placement.kind_id.as_str()).ok_or("unsupported quantity Kind")?;
    let contract =
        operation_contract(placement.kind_id.as_str()).ok_or("unsupported quantity contract")?;
    super::factory::validate_placement(placement, &expected)?;
    if placement.capability_id != expected.capability_id
        || placement.limits != expected.limits
        || placement.semantic_contract != contract.semantic_contract()
        || !placement.resources.is_empty()
        || !placement.authority.is_empty()
        || placement.base.is_some()
        || !placement.realization_characteristics.is_empty()
        || !placement.pool_references.is_empty()
        || !placement.terminal_transductions.is_empty()
    {
        return Err("quantity operation differs from its exact browser offer".into());
    }
    if placement.configuration.len() != contract.configuration.len() {
        return Err("quantity configuration differs".into());
    }
    for field in &contract.configuration {
        let mut entries = placement
            .configuration
            .iter()
            .filter(|entry| entry.key == field.key);
        let entry = entries.next().ok_or("quantity configuration is absent")?;
        if entries.next().is_some() {
            return Err("quantity configuration is duplicated".into());
        }
        conduit_plot::validate_configuration_value(field, &entry.value)
            .map_err(|error| format!("quantity configuration: {error}"))?;
    }
    if placement.kind_id.as_str() == conversion::converted_equals::KIND {
        let [entry] = placement.configuration.as_slice() else {
            return Err("receipt comparator requires expected Quantity".into());
        };
        let ("expected", ConfigurationValue::Quantity(expected)) =
            (entry.key.as_str(), &entry.value)
        else {
            return Err("receipt comparator expected Type differs".into());
        };
        let comparison =
            conversion::converted_equals::PreparedConvertedEquals::new(expected.value())
                .map_err(|error| format!("receipt comparator configuration: {error:?}"))?;
        let no = values
            .store(&InfoBool::FALSE.encode())
            .map_err(|error| format!("store false: {error:?}"))?;
        let yes = values
            .store(&InfoBool::TRUE.encode())
            .map_err(|error| format!("store true: {error:?}"))?;
        return Ok(BrowserBack::installed_step(
            conduit_semantic_catalog::ConvertedEqualsBack::new(
                comparison,
                [no, yes],
                MAXIMUM_BROWSER_VALUE_BYTES as u32,
            ),
        ));
    }
    let receipt = conversion::prepare_operation_configuration(
        placement.kind_id.as_str(),
        &placement.configuration,
    )
    .map_err(|error| format!("quantity preparation: {error:?}"))?;
    if receipt
        .value_type()
        .profile()
        .map_err(|error| format!("receipt Type: {error:?}"))?
        .value_kind()
        != &placement.outputs[0].value_kind
    {
        return Err("quantity receipt output Type differs".into());
    }
    let bytes = receipt
        .canonical_bytes()
        .map_err(|error| format!("receipt encoding: {error:?}"))?;
    if bytes.len() > MAXIMUM_BROWSER_VALUE_BYTES {
        return Err("quantity receipt exceeds browser representation eligibility".into());
    }
    let value = values
        .store(&bytes)
        .map_err(|error| format!("store quantity receipt: {error:?}"))?;
    Ok(BrowserBack::source(value))
}

fn operation_contract(kind: &str) -> Option<Kind> {
    if kind == conversion::converted_equals::KIND {
        Some(conversion::converted_equals::contract())
    } else {
        conversion::operation_contract(kind)
    }
}
