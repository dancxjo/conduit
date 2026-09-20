//! Pure-kernel browser realizations of explicit framed-record temporal adapters.

use super::factory::{validate_placement, BrowserInstallation};
use super::BrowserOperation;
use conduit_core::{
    ArtifactId, Back, BackOfferBuilder, CapabilityId, CapabilityOffer, ExecutionProfileId,
    ImplementationId, Kind, PlannedGear,
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
        conduit_net::record_singleton_stream_semantic_contract(),
        SINGLETON_IMPLEMENTATION,
    )
}

fn exactly_one_offer() -> CapabilityOffer {
    offer(
        conduit_net::record_exactly_one_semantic_contract(),
        EXACTLY_ONE_IMPLEMENTATION,
    )
}

fn offer(contract: Kind, implementation: &str) -> CapabilityOffer {
    BackOfferBuilder::new(
        contract,
        Back {
            capability_id: CapabilityId::from(implementation),
            execution_profile_id: ExecutionProfileId::from("browser/record-temporal@1"),
            implementation_id: ImplementationId::from(implementation),
            artifact_id: ArtifactId::from("conduit-net/record-temporal@1"),
            host_operations: Vec::new(),
            resource_requirements: Vec::new(),
            authority_requirements: Vec::new(),
        },
    )
    .build()
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
