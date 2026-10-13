//! Explicit conversion from a Quantity into its structured leaf envelope.

use crate::{KindTerminalBehavior, StandardKindContract};
#[cfg(feature = "plot-catalog")]
use alloc::string::String;
use alloc::{vec, vec::Vec};
#[cfg(test)]
use conduit_core::{Quantity, StructuredInfoValue, Unit};
use conduit_core::{
    kind_id, port_id, CapabilityLimits, Kind, PortDescriptor, PortDirection, PortTemporal,
    StructuredInfoType,
};

pub const QUANTITY_INFO_WRAP_KIND: &str = "structured-info/wrap-quantity";
pub const QUANTITY_INFO_WRAP_REVISION: &str = "conduit.std/wrap-quantity@1";
pub const QUANTITY_PRESENTATION_KIND: &str = "presentation/quantity";
pub const QUANTITY_PRESENTATION_REVISION: &str = "conduit.std/presentation-quantity@1";
pub const QUANTITY_INFO_MAXIMUM_BYTES: usize = 4096;

pub fn quantity_info_leaf_kind() -> conduit_core::KindId {
    let coordinate = conduit_core::PortableNumericProfile {
        maximum_encoded_bytes: 512,
        ..conduit_core::PortableNumericProfile::MAXIMAL
    };
    let unit_law = conduit_core::PortableNumericProfile {
        maximum_encoded_bytes: 256,
        ..conduit_core::PortableNumericProfile::MAXIMAL
    };
    kind_id(
        &conduit_core::PhysicalNumericProfile {
            coordinate,
            unit_law,
        }
        .generic_info_id()
        .expect("finite physical profile"),
    )
}

pub fn wrapped_quantity_type() -> StructuredInfoType {
    StructuredInfoType::leaf(quantity_info_leaf_kind()).expect("Quantity is an exact leaf")
}

/// Prepare the canonical envelope before Play; subsequent wrapping only copies bytes.
pub fn quantity_info_prefix() -> Vec<u8> {
    // The leaf length is emitted separately for each variable capsule.
    let mut prefix = wrapped_quantity_type()
        .canonical_bytes()
        .expect("checked type");
    prefix.push(0);
    prefix
}

pub fn quantity_info_wrap_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(QUANTITY_INFO_WRAP_KIND),
        plain_name: "Wrap Quantity".into(),
        summary: "Preserve one Quantity as an exact structured Info leaf.".into(),
        inputs: vec![PortDescriptor {
            port_id: port_id("in"),
            value_kind: quantity_info_leaf_kind(),
            direction: PortDirection::Input,
            temporal: PortTemporal::Value,
            abnormal_kind: None,
        }],
        outputs: vec![PortDescriptor {
            port_id: port_id("out"),
            value_kind: wrapped_quantity_type()
                .profile()
                .unwrap()
                .value_kind()
                .clone(),
            direction: PortDirection::Output,
            temporal: PortTemporal::Value,
            abnormal_kind: None,
        }],
        configuration: Default::default(),
        limits: CapabilityLimits {
            max_active_instances: 16,
            max_queue_items: 1,
            max_queue_bytes: QUANTITY_INFO_MAXIMUM_BYTES as u32,
        },
        terminal_behavior:
            KindTerminalBehavior::EmitsOneDecisionOrCompletesWhenDecisionBecomesImpossible,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: "wrap: structured-info/wrap-quantity".into(),
    }
}

pub fn quantity_info_wrap_semantic_contract() -> Kind {
    let contract = quantity_info_wrap_contract();
    Kind {
        startup_parameters: Vec::new(),
        shorthand: None,
        kind_id: contract.kind_id,
        kind_contract_revision: QUANTITY_INFO_WRAP_REVISION.into(),
        inputs: contract.inputs,
        outputs: contract.outputs,
        configuration: contract.configuration,
        semantic_laws: alloc::vec![conduit_core::KindSemanticLaw::Terminal(
            contract.terminal_behavior
        )],
        limits: contract.limits,
    }
}

pub fn quantity_presentation_definition() -> conduit_plot::KindProjection {
    let contract = quantity_presentation_semantic_contract();
    conduit_plot::KindProjection {
        kind_id: contract.kind_id,
        kind_contract_revision: contract.kind_contract_revision,
        inputs: contract.inputs,
        outputs: contract.outputs,
        configuration: Default::default(),
    }
}

pub fn quantity_presentation_semantic_contract() -> Kind {
    Kind {
        startup_parameters: Vec::new(),
        shorthand: None,
        kind_id: kind_id(QUANTITY_PRESENTATION_KIND),
        kind_contract_revision: QUANTITY_PRESENTATION_REVISION.into(),
        inputs: vec![PortDescriptor {
            port_id: port_id("input"),
            value_kind: wrapped_quantity_type()
                .profile()
                .unwrap()
                .value_kind()
                .clone(),
            direction: PortDirection::Input,
            temporal: PortTemporal::Value,
            abnormal_kind: None,
        }],
        outputs: Vec::new(),
        configuration: Default::default(),
        semantic_laws: Default::default(),
        limits: CapabilityLimits {
            max_active_instances: 4,
            max_queue_items: 1,
            max_queue_bytes: conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        },
    }
}

#[cfg(feature = "plot-catalog")]
pub fn install_quantity_info_catalog(
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), String> {
    startup.insert(conduit_plot::KindSignature {
        kind: QUANTITY_INFO_WRAP_KIND.into(),
        startup_parameters: Vec::new(),
    })?;
    profile
        .insert_kind(quantity_info_wrap_semantic_contract())
        .map_err(|error| alloc::format!("{error}"))?;
    startup.insert(conduit_plot::KindSignature {
        kind: QUANTITY_PRESENTATION_KIND.into(),
        startup_parameters: Vec::new(),
    })?;
    profile
        .insert_kind(quantity_presentation_semantic_contract())
        .map_err(|error| alloc::format!("{error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepared_quantity_prefix_preserves_canonical_values_and_units() {
        let prefix = quantity_info_prefix();
        for unit in [
            Unit::Hertz,
            Unit::Percent,
            Unit::One,
            Unit::Nanosecond,
            Unit::Mebibyte,
        ] {
            for value in [i64::MIN, -1, 0, 1, i64::MAX] {
                let quantity = Quantity::new(value, unit.clone()).encode();
                let mut wrapped = prefix.clone();
                wrapped.extend_from_slice(&(quantity.len() as u32).to_le_bytes());
                wrapped.extend_from_slice(&quantity);
                let expected =
                    StructuredInfoValue::leaf(wrapped_quantity_type(), quantity.to_vec())
                        .unwrap()
                        .canonical_bytes()
                        .unwrap();
                assert_eq!(wrapped, expected);
                assert!(wrapped.len() <= QUANTITY_INFO_MAXIMUM_BYTES);
            }
        }
        let contract = quantity_info_wrap_contract();
        assert_eq!(contract.inputs[0].value_kind, quantity_info_leaf_kind());
        assert_ne!(
            contract.inputs[0].value_kind,
            contract.outputs[0].value_kind
        );
        let semantics = quantity_info_wrap_semantic_contract();
        assert_eq!(
            semantics.kind_contract_revision.as_str(),
            QUANTITY_INFO_WRAP_REVISION
        );
        assert_eq!(semantics.limits, contract.limits);
    }
}
