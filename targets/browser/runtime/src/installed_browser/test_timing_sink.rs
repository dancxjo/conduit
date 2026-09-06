//! Typed timing observation fixture, excluded from production installations.
use super::factory::{BrowserHostResult, BrowserInstallation, BrowserManifestation};
use super::BrowserOperation;
use conduit_core::*;
use conduit_kernel::HostedValueStore;
pub(crate) const KIND: &str = "conduit-test/timing-sink";
pub(super) static SINK: BrowserInstallation = BrowserInstallation {
    implementation_id: KIND,
    offer,
    prepare,
    perform: Some(present),
};
pub(crate) fn offer() -> CapabilityOffer {
    let mut contract = conduit_semantic_catalog::normalize_relative_duration_definition();
    contract.inputs = contract.outputs;
    contract.inputs[0].direction = PortDirection::Input;
    CapabilityOffer {
        kind_id: KIND.into(),
        kind_contract_revision: "conduit-test/timing-sink@1".into(),
        capability_id: KIND.into(),
        startup_parameters: Vec::new(),
        shorthand: None,
        inputs: contract.inputs,
        outputs: Vec::new(),
        implementation: ImplementationOffer {
            execution_profile_id: "conduit-test/timing-sink@1".into(),
            implementation_id: KIND.into(),
            artifact_id: "conduit-test/timing-sink@1".into(),
        },
        host_operations: vec![HostOperationRequirement {
            contract_id: "conduit-test/timing-output".into(),
            target_kind: Some(KIND.into()),
            maximum_in_flight: 1,
            maximum_input_bytes: 4096,
            maximum_output_bytes: 0,
        }],
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: 4096,
        },
    }
}
fn prepare(placement: &PlannedGear, _: &mut HostedValueStore) -> Result<BrowserOperation, String> {
    super::factory::validate_placement(placement, &offer())?;
    Ok(BrowserOperation::presentation(4096, 1))
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
