//! Typed measurement-decision observation fixture, excluded from production installations.

use super::factory::{BrowserHostResult, BrowserInstallation, BrowserManifestation};
use super::BrowserOperation;
use conduit_core::*;
use conduit_kernel::HostedValueStore;

pub(crate) const KIND: &str = "conduit-test/measurement-decision-sink";

pub(super) static SINK: BrowserInstallation = BrowserInstallation {
    implementation_id: KIND,
    offer,
    prepare,
    perform: Some(present),
};

pub(crate) fn offer() -> CapabilityOffer {
    CapabilityOffer {
        kind_id: KIND.into(),
        kind_contract_revision: "conduit-test/measurement-decision-sink@1".into(),
        capability_id: KIND.into(),
        startup_parameters: Vec::new(),
        shorthand: None,
        inputs: vec![PortDescriptor {
            port_id: port_id("decision"),
            value_kind: conduit_data::measurement_threshold_decision_type()
                .profile()
                .unwrap()
                .value_kind()
                .clone(),
            direction: PortDirection::Input,
            temporal: PortTemporal::Value,
        }],
        outputs: Vec::new(),
        implementation: ImplementationOffer {
            execution_profile_id: KIND.into(),
            implementation_id: KIND.into(),
            artifact_id: KIND.into(),
        },
        host_operations: vec![HostOperationRequirement {
            contract_id: "conduit-test/measurement-decision-output".into(),
            target_kind: Some(KIND.into()),
            maximum_in_flight: 1,
            maximum_input_bytes: super::MAXIMUM_BROWSER_VALUE_BYTES as u32,
            maximum_output_bytes: 0,
        }],
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: super::MAXIMUM_BROWSER_VALUE_BYTES as u32,
        },
    }
}

fn prepare(placement: &PlannedGear, _: &mut HostedValueStore) -> Result<BrowserOperation, String> {
    super::factory::validate_placement(placement, &offer())?;
    Ok(BrowserOperation::presentation(
        super::MAXIMUM_BROWSER_VALUE_BYTES as u32,
        1,
    ))
}

fn present(_: &PlannedGear, input: &[u8]) -> Result<BrowserHostResult, String> {
    Ok(BrowserHostResult {
        output: None,
        manifestation: Some(BrowserManifestation {
            kind_id: KIND,
            canonical_value: input.to_vec(),
        }),
    })
}
