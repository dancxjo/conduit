//! Explicit conversion from a Quantity into its structured leaf envelope.

use crate::{StandardKindContract, TerminalBehavior};
#[cfg(feature = "form-catalog")]
use alloc::string::String;
use alloc::{vec, vec::Vec};
use conduit_core::{
    kind_id, port_id, CapabilityLimits, PortDescriptor, PortDirection, PortTemporal, Quantity,
    QuantityUnit, StructuredInfoType, StructuredInfoValue, QUANTITY_ENCODED_LEN, QUANTITY_INFO_ID,
};

pub const QUANTITY_INFO_WRAP_KIND: &str = "structured-info/wrap-quantity";
pub const QUANTITY_INFO_WRAP_REVISION: &str = "conduit.std/wrap-quantity@1";
pub const QUANTITY_INFO_MAXIMUM_BYTES: usize = 128;

pub fn wrapped_quantity_type() -> StructuredInfoType {
    StructuredInfoType::leaf(kind_id(QUANTITY_INFO_ID)).expect("Quantity is an exact leaf")
}

/// Prepare the canonical envelope before Play; subsequent wrapping only copies bytes.
pub fn quantity_info_prefix() -> Vec<u8> {
    let quantity = Quantity::new(0, QuantityUnit::One).encode();
    let mut envelope = StructuredInfoValue::leaf(wrapped_quantity_type(), quantity.to_vec())
        .expect("canonical Quantity leaf")
        .canonical_bytes()
        .expect("finite Quantity envelope");
    assert!(envelope.ends_with(&quantity));
    envelope.truncate(envelope.len() - QUANTITY_ENCODED_LEN);
    assert!(envelope.len() + QUANTITY_ENCODED_LEN <= QUANTITY_INFO_MAXIMUM_BYTES);
    envelope
}

pub fn quantity_info_wrap_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(QUANTITY_INFO_WRAP_KIND),
        plain_name: "Wrap Quantity".into(),
        summary: "Preserve one Quantity as an exact structured Info leaf.".into(),
        inputs: vec![PortDescriptor {
            port_id: port_id("in"),
            value_kind: kind_id(QUANTITY_INFO_ID),
            direction: PortDirection::Input,
            temporal: PortTemporal::Value,
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
        }],
        configuration: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 16,
            max_queue_items: 1,
            max_queue_bytes: QUANTITY_INFO_MAXIMUM_BYTES as u32,
        },
        terminal_behavior:
            TerminalBehavior::EmitsOneDecisionOrCompletesWhenDecisionBecomesImpossible,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: "wrap: structured-info/wrap-quantity".into(),
    }
}

#[cfg(feature = "form-catalog")]
pub fn install_quantity_info_catalog(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), String> {
    let contract = quantity_info_wrap_contract();
    startup.insert(conduit_form::KindSignature {
        kind: QUANTITY_INFO_WRAP_KIND.into(),
        startup_parameters: Vec::new(),
    })?;
    profile
        .insert(conduit_form::KindDefinition {
            kind_id: contract.kind_id,
            kind_contract_revision: conduit_core::KindContractRevision::from(
                QUANTITY_INFO_WRAP_REVISION,
            ),
            inputs: contract.inputs,
            outputs: contract.outputs,
            configuration: Vec::new(),
        })
        .map_err(|error| alloc::format!("{error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepared_quantity_prefix_preserves_canonical_values_and_units() {
        let prefix = quantity_info_prefix();
        for unit in [
            QuantityUnit::Hertz,
            QuantityUnit::Percent,
            QuantityUnit::One,
            QuantityUnit::Nanosecond,
            QuantityUnit::Mebibyte,
        ] {
            for value in [i64::MIN, -1, 0, 1, i64::MAX] {
                let quantity = Quantity::new(value, unit).encode();
                let mut wrapped = prefix.clone();
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
        assert_eq!(contract.inputs[0].value_kind.as_str(), QUANTITY_INFO_ID);
        assert_ne!(
            contract.inputs[0].value_kind,
            contract.outputs[0].value_kind
        );
    }
}
