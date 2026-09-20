//! Two exact existing field selectors needed by the pointer controller.

use super::factory::{validate_placement, BrowserInstallation};
use super::BrowserOperation;
use conduit_core::{
    ArtifactId, Back, BackOfferBuilder, CapabilityId, CapabilityLimits, CapabilityOffer,
    ConfigurationValue, ExecutionProfileId, HostOperationRequirement, ImplementationId,
    PlannedGear, PortTemporal, StructuredCanonicalSelection, StructuredSelector,
};

pub(crate) const HOST_OPERATION: &str = "conduit.host/structured-selector@1";
pub(super) static POSITION: BrowserInstallation = BrowserInstallation {
    implementation_id: "browser/select-pointer-position@1",
    offer: position_offer,
    prepare,
    perform: None,
};
pub(super) static X: BrowserInstallation = BrowserInstallation {
    implementation_id: "browser/select-point-x@1",
    offer: x_offer,
    prepare,
    perform: None,
};

fn position() -> StructuredSelector {
    StructuredSelector::field(conduit_semantic_catalog::pointer_event_type(), "position")
        .expect("existing pointer position field")
}
fn x() -> StructuredSelector {
    StructuredSelector::field(position().output_type().clone(), "x")
        .expect("existing Point2 x field")
}

pub(super) fn install_types(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), String> {
    startup
        .insert_structured_type("Point2", position().output_type().clone())
        .map_err(debug)?;
    for selector in [position(), x()] {
        profile
            .insert(conduit_form::structured_selector_definition(
                &selector,
                PortTemporal::Value,
            ))
            .map_err(debug)?;
    }
    Ok(())
}
fn position_offer() -> CapabilityOffer {
    offer(&position(), POSITION.implementation_id)
}
fn x_offer() -> CapabilityOffer {
    offer(&x(), X.implementation_id)
}
fn offer(selector: &StructuredSelector, implementation: &str) -> CapabilityOffer {
    let contract =
        conduit_semantic_catalog::structured_selector_contract(selector, PortTemporal::Value);
    let target_kind = contract.kind_id.clone();
    BackOfferBuilder::new(
        contract,
        Back {
            capability_id: CapabilityId::from(implementation),
            implementation_id: ImplementationId::from(implementation),
            execution_profile_id: ExecutionProfileId::from(implementation),
            artifact_id: ArtifactId::from("conduit-browser-runtime/pointer-selectors@1"),
            host_operations: vec![HostOperationRequirement {
                contract_id: HOST_OPERATION.into(),
                target_kind: Some(target_kind),
                maximum_in_flight: 1,
                maximum_input_bytes: super::MAXIMUM_BROWSER_VALUE_BYTES as u32,
                maximum_output_bytes: super::MAXIMUM_BROWSER_VALUE_BYTES as u32,
            }],
            resource_requirements: Vec::new(),
            authority_requirements: Vec::new(),
        },
    )
    .narrow_capacity(CapabilityLimits {
        max_active_instances: 8,
        max_queue_items: 1,
        max_queue_bytes: super::MAXIMUM_BROWSER_VALUE_BYTES as u32,
    })
    .expect("browser pointer selector capacity narrows its exact dynamic contract")
    .build()
}

fn prepare(
    placement: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<BrowserOperation, String> {
    PreparedSelector::new(placement)?;
    Ok(BrowserOperation::unary(
        super::MAXIMUM_BROWSER_VALUE_BYTES as u32,
        1,
    ))
}

pub(crate) struct PreparedSelector {
    selector: StructuredSelector,
    input_type: Vec<u8>,
    output_type: Vec<u8>,
    output: Vec<u8>,
}

impl PreparedSelector {
    pub(crate) fn new(placement: &PlannedGear) -> Result<Self, String> {
        let expected = match placement.implementation_id.as_str() {
            "browser/select-pointer-position@1" => position(),
            "browser/select-point-x@1" => x(),
            _ => return Err("unsupported browser selector implementation".into()),
        };
        validate_placement(
            placement,
            &offer(&expected, placement.implementation_id.as_str()),
        )?;
        let [entry] = placement.configuration.as_slice() else {
            return Err("selector requires exactly one configuration".into());
        };
        let ConfigurationValue::Text(encoded) = &entry.value else {
            return Err("selector configuration must be canonical text".into());
        };
        if entry.key != "selector" || encoded != &expected.canonical_hex().map_err(debug)? {
            return Err("selector configuration differs from exact offered field".into());
        }
        Ok(Self {
            input_type: expected.input_type().canonical_bytes().map_err(debug)?,
            output_type: expected.output_type().canonical_bytes().map_err(debug)?,
            selector: expected,
            output: Vec::with_capacity(super::MAXIMUM_BROWSER_VALUE_BYTES),
        })
    }

    pub(crate) fn execute(&mut self, input: &[u8]) -> Result<&[u8], String> {
        match self
            .selector
            .select_canonical_into(input, &self.input_type, &self.output_type, &mut self.output)
            .map_err(debug)?
        {
            StructuredCanonicalSelection::Matched => Ok(&self.output),
            _ => Err("required pointer field selector did not match".into()),
        }
    }
}
fn debug(error: impl core::fmt::Debug) -> String {
    format!("{error:?}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pointer_selectors_preserve_their_dynamic_contracts_and_narrow_capacity() {
        for (selector, offer) in [(position(), position_offer()), (x(), x_offer())] {
            let semantic = conduit_semantic_catalog::structured_selector_contract(
                &selector,
                PortTemporal::Value,
            );
            assert_eq!(offer.startup_parameters, semantic.startup_parameters);
            assert_eq!(offer.shorthand, semantic.shorthand);
            assert_eq!(offer.kind_id, semantic.kind_id);
            assert_eq!(
                offer.kind_contract_revision,
                semantic.kind_contract_revision
            );
            assert_eq!(offer.inputs, semantic.inputs);
            assert_eq!(offer.outputs, semantic.outputs);
            assert_eq!(offer.limits.max_active_instances, 8);
            assert_eq!(offer.limits.max_queue_items, 1);
            assert!(offer.limits.max_queue_items < semantic.limits.max_queue_items);
            assert!(offer.limits.max_queue_bytes < semantic.limits.max_queue_bytes);
        }
    }
}
