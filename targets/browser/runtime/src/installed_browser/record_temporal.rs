//! Pure-kernel browser realizations of explicit framed-record temporal adapters.

use super::factory::{validate_placement, BrowserInstallation};
use super::BrowserOperation;
use conduit_core::{
    ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer, ExecutionProfileId,
    ImplementationId, ImplementationOffer, KindContractRevision, PlannedGear,
};
use conduit_kernel::HostedValueStore;

const SINGLETON_IMPLEMENTATION: &str = "browser/record-singleton-stream@1";
const EXACTLY_ONE_IMPLEMENTATION: &str = "browser/record-exactly-one@1";
const MAXIMUM: u32 = super::MAXIMUM_BROWSER_VALUE_BYTES as u32;

pub(super) static SINGLETON: BrowserInstallation = BrowserInstallation {
    implementation_id: SINGLETON_IMPLEMENTATION,
    offer: singleton_offer,
    prepare: prepare_singleton,
    perform: None,
};
pub(super) static EXACTLY_ONE: BrowserInstallation = BrowserInstallation {
    implementation_id: EXACTLY_ONE_IMPLEMENTATION,
    offer: exactly_one_offer,
    prepare: prepare_exactly_one,
    perform: None,
};

fn singleton_offer() -> CapabilityOffer {
    offer(
        conduit_net::record_singleton_stream_definition(),
        SINGLETON_IMPLEMENTATION,
    )
}

fn exactly_one_offer() -> CapabilityOffer {
    offer(
        conduit_net::record_exactly_one_definition(),
        EXACTLY_ONE_IMPLEMENTATION,
    )
}

fn offer(definition: conduit_form::KindDefinition, implementation: &str) -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: Some((
            definition.inputs[0].port_id.clone(),
            definition.outputs[0].port_id.clone(),
        )),
        capability_id: CapabilityId::from(implementation),
        kind_id: definition.kind_id,
        kind_contract_revision: KindContractRevision::from(
            conduit_net::RECORD_TEMPORAL_CONTRACT_REVISION,
        ),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from("browser/record-temporal@1"),
            implementation_id: ImplementationId::from(implementation),
            artifact_id: ArtifactId::from("conduit-net/record-temporal@1"),
        },
        inputs: definition.inputs,
        outputs: definition.outputs,
        host_operations: Vec::new(),
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: MAXIMUM,
        },
    }
}

fn prepare_singleton(
    placement: &PlannedGear,
    _: &mut HostedValueStore,
) -> Result<BrowserOperation, String> {
    validate_placement(placement, &singleton_offer())?;
    Ok(BrowserOperation::singleton_stream(MAXIMUM))
}

fn prepare_exactly_one(
    placement: &PlannedGear,
    _: &mut HostedValueStore,
) -> Result<BrowserOperation, String> {
    validate_placement(placement, &exactly_one_offer())?;
    Ok(BrowserOperation::exactly_one(MAXIMUM))
}
